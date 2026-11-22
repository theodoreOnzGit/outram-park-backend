// SPDX-License-Identifier: GPL-3.0-only
// Copyright (C) 2026 OUTRAM PARK contributors
//! # GeN-Foam `common/` (RBF, InterpolateTable, timeProfile) against a run of upstream (gh:#1361, step 2)
//!
//! AI-written, not human-reviewed. Research, education and V&V only.
//!
//! ## Methodology
//!
//! Reference: `verification_and_validation/genfoam_vs_upstream_gh1361/refs/common_dump.ref.txt.gz`,
//! produced by GeN-Foam `652b3da0cc55`'s own `libCommon` (unmodified
//! sources) on OpenFOAM-ESI v2506, gcc 13.3 `-O3`, no `-march`, through the
//! driver `upstream_driver/common_dump/genfoamCommonDump.C`. The file holds the
//! inputs (17 digits, exact doubles) and upstream's results. This test runs the
//! port on the same doubles and compares **bit for bit**, counting every
//! difference and its size in ulps. Regenerate with
//! `scripts/build_and_run_common_dump.sh`.
//!
//! Pass criterion: every quantity bit-identical, except where the cause is
//! the platform `libm` (`ln`, `pow`: PETIR vs glibc), which PETIR's AGENTS.md
//! accepts at 1 ulp; those are counted and pinned below with their exact
//! numbers, so a change in either direction fails.
//!
//! Results, predictions and causes: the `README.md` beside the reference.

use miniz_oxide::inflate::decompress_to_vec;
use outram_foam_appbuilder_lib::genfoam::common::interpolate_table::{
    InterpolationMethod, OutOfBounds, ScalarInterpolateTable,
};
use outram_foam_appbuilder_lib::genfoam::common::rbf::{
    polyharmonic_spline, polyharmonic_spline_inverse, solve_polyharmonic_spline,
    solve_polyharmonic_spline_direct,
};
use outram_foam_appbuilder_lib::genfoam::common::time_profile::TimeProfile;
use std::path::PathBuf;
use uom::si::f64::Time;
use uom::si::time::second;

fn reference_text() -> String {
    let path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("verification_and_validation/genfoam_vs_upstream_gh1361/refs/common_dump.ref.txt.gz");
    let bytes = std::fs::read(&path).expect("reference present");
    // gzip -n: 10-byte header, 8-byte trailer.
    String::from_utf8(decompress_to_vec(&bytes[10..bytes.len() - 8]).expect("inflate"))
        .expect("utf8")
}

/// `(tag, words after the tag)` for each reference line.
fn lines() -> Vec<(String, Vec<String>)> {
    reference_text()
        .lines()
        .filter(|l| !l.starts_with('#'))
        .map(|l| {
            let mut it = l.split_whitespace().map(str::to_string);
            let tag = it.next().unwrap_or_default();
            (tag, it.collect())
        })
        .collect()
}

fn nums(words: &[String]) -> Vec<f64> {
    // words[0] is the count
    words[1..].iter().map(|w| w.parse::<f64>().expect("f64")).collect()
}

/// Distance in representable doubles (0 = bit-identical). Different signs or
/// non-finite values give `u64::MAX`.
fn ulps(a: f64, b: f64) -> u64 {
    if a.to_bits() == b.to_bits() {
        return 0;
    }
    if !a.is_finite() || !b.is_finite() || (a < 0.0) != (b < 0.0) {
        // +0 / -0 pair
        return if a == b { 0 } else { u64::MAX };
    }
    a.to_bits().abs_diff(b.to_bits())
}

/// Running tally: values compared, values not bit-identical, largest ulp
/// distance, largest relative difference.
#[derive(Default, Debug, Clone, Copy, PartialEq)]
struct Tally {
    n: usize,
    differ: usize,
    max_ulp: u64,
    max_rel: f64,
}

impl Tally {
    fn add(&mut self, port: f64, up: f64) {
        self.n += 1;
        let u = ulps(port, up);
        if u != 0 {
            self.differ += 1;
            self.max_ulp = self.max_ulp.max(u);
            let rel = ((port - up) / up).abs();
            if rel.is_finite() {
                self.max_rel = self.max_rel.max(rel);
            }
        }
    }
    fn add_all(&mut self, port: &[f64], up: &[f64]) {
        assert_eq!(port.len(), up.len());
        for (a, b) in port.iter().zip(up) {
            self.add(*a, *b);
        }
    }
}

struct RbfCase {
    mode: usize,
    p: usize,
    n: usize,
    x: Vec<Vec<f64>>, // x[param][data]
    q: Vec<Vec<f64>>, // q[query][param]
    inv: Vec<f64>,
    w: Vec<f64>,
    v: Vec<f64>,
    f: Vec<f64>,
}

fn rbf_cases() -> Vec<(String, RbfCase)> {
    let ls = lines();
    let mut out = Vec::new();
    let mut i = 0;
    while i < ls.len() {
        if ls[i].0 == "RBF" {
            let h = &ls[i].1;
            let name = h[0].clone();
            let get = |key: &str| -> usize {
                let pos = h.iter().position(|w| w == key).expect("key");
                h[pos + 1].parse().expect("usize")
            };
            let (mode, p, n, nq) = (get("mode"), get("p"), get("n"), get("nq"));
            let flat_x = nums(&ls[i + 1].1);
            let v = nums(&ls[i + 2].1);
            let flat_q = nums(&ls[i + 3].1);
            let inv = nums(&ls[i + 4].1);
            let w = nums(&ls[i + 5].1);
            let f = nums(&ls[i + 6].1);
            assert_eq!(ls[i + 4].0, "INV");
            assert_eq!(ls[i + 6].0, "F");
            let x: Vec<Vec<f64>> = flat_x.chunks(n).map(<[f64]>::to_vec).collect();
            let q: Vec<Vec<f64>> = flat_q.chunks(p).map(<[f64]>::to_vec).collect();
            assert_eq!(q.len(), nq);
            out.push((name, RbfCase { mode, p, n, x, q, inv, w, v, f }));
            i += 7;
        } else {
            i += 1;
        }
    }
    out
}

/// All three tallies of the RBF comparison.
struct RbfResult {
    inverse: Tally,
    weights: Tally,
    values_port_weights: Tally,
    values_upstream_weights: Tally,
    values_upstream_weights_modes_1_3: Tally,
    values_upstream_weights_modes_2_4: Tally,
    cases: usize,
    weight_vectors_identical: usize,
    /// Largest `max_i |w_i - w_up_i| / max_i |w_up_i|` over the cases.
    weights_norm_max: f64,
}

fn run_rbf(direct: bool) -> RbfResult {
    let mut r = RbfResult {
        inverse: Tally::default(),
        weights: Tally::default(),
        values_port_weights: Tally::default(),
        values_upstream_weights: Tally::default(),
        values_upstream_weights_modes_1_3: Tally::default(),
        values_upstream_weights_modes_2_4: Tally::default(),
        cases: 0,
        weight_vectors_identical: 0,
        weights_norm_max: 0.0,
    };
    for (name, c) in rbf_cases() {
        r.cases += 1;
        assert_eq!(c.x.len(), c.p);
        assert_eq!(c.w.len(), c.n + c.p + 1, "{name}");
        if !c.inv.is_empty() {
            let inv = polyharmonic_spline_inverse(&c.x, c.mode).expect("nonsingular");
            let m = inv.n();
            let flat: Vec<f64> = (0..m)
                .flat_map(|i| (0..m).map(move |j| (i, j)))
                .map(|(i, j)| inv.get(i, j))
                .collect();
            r.inverse.add_all(&flat, &c.inv);
        }
        let w = if direct {
            solve_polyharmonic_spline_direct(&c.x, &c.v, c.mode)
        } else {
            solve_polyharmonic_spline(&c.x, &c.v, c.mode)
        }
        .expect("nonsingular");
        let before = r.weights.differ;
        r.weights.add_all(&w, &c.w);
        let scale = c.w.iter().fold(0.0_f64, |m, x| m.max(x.abs()));
        let worst = w.iter().zip(&c.w).fold(0.0_f64, |m, (a, b)| m.max((a - b).abs()));
        r.weights_norm_max = r.weights_norm_max.max(worst / scale);
        if r.weights.differ == before {
            r.weight_vectors_identical += 1;
        }
        for (qi, q) in c.q.iter().enumerate() {
            let fp = polyharmonic_spline(&w, &c.x, q, c.mode);
            r.values_port_weights.add(fp, c.f[qi]);
            let fu = polyharmonic_spline(&c.w, &c.x, q, c.mode);
            r.values_upstream_weights.add(fu, c.f[qi]);
            if c.mode == 1 || c.mode == 3 {
                r.values_upstream_weights_modes_1_3.add(fu, c.f[qi]);
            } else {
                r.values_upstream_weights_modes_2_4.add(fu, c.f[qi]);
            }
        }
    }
    r
}

#[test]
fn rbf_against_upstream() {
    let r = run_rbf(false);
    assert_eq!(r.cases, 34);
    // 8193 inverse entries, 562 weights, 280 values, every one bit-identical
    assert_eq!(r.inverse.n, 8193);
    assert_eq!(r.inverse.differ, 0);
    assert_eq!(r.weights.n, 562);
    assert_eq!(r.weights.differ, 0);
    assert_eq!(r.weight_vectors_identical, 34);
    assert_eq!(r.values_port_weights.n, 280);
    assert_eq!(r.values_port_weights.differ, 0);
    // The evaluator alone, given upstream's weights: no ulp from ln either
    // (modes 2 and 4 use `ln`, PETIR vs glibc; none of the 140 differs).
    assert_eq!(r.values_upstream_weights.differ, 0);
    assert_eq!(r.values_upstream_weights_modes_1_3.n, 140);
    assert_eq!(r.values_upstream_weights_modes_2_4.n, 140);
    assert_eq!(r.values_upstream_weights_modes_2_4.differ, 0);
}

/// The pre-gh:#1361 direct back-substitution, kept callable, is pinned at
/// the numbers measured against upstream on 2026-11-12: it rounds
/// differently from `inv(A) * v` in 528 of 562 weights.
#[test]
fn direct_solve_is_the_old_form_and_differs_from_upstream() {
    let r = run_rbf(true);
    assert_eq!(r.weights.n, 562);
    assert_eq!(r.weights.differ, 528);
    assert_eq!(r.weight_vectors_identical, 5);
    assert_eq!(r.values_port_weights.differ, 240);
    assert_eq!(r.values_port_weights.max_ulp, 26039);
    assert!(r.weights_norm_max > 6.4e-14 && r.weights_norm_max < 6.5e-14, "{:e}", r.weights_norm_max);
}

struct Tbl {
    method: InterpolationMethod,
    ob: OutOfBounds,
    xs: Vec<f64>,
    ys: Vec<f64>,
    qin: Vec<f64>,
    vin: Vec<f64>,
    qout: Vec<f64>,
    vout: Vec<f64>,
    ks: Vec<f64>,
    int: Vec<f64>,
    nq: Vec<f64>,
    np: Vec<f64>,
}

fn tbl_cases() -> Vec<Tbl> {
    let ls = lines();
    let mut out = Vec::new();
    let mut i = 0;
    while i < ls.len() {
        if ls[i].0 == "TBL" {
            let h = &ls[i].1;
            let method = if h[2] == "0" { InterpolationMethod::Step } else { InterpolationMethod::Linear };
            let ob = match h[4].as_str() {
                "0" => OutOfBounds::Error,
                "1" => OutOfBounds::Extrapolation,
                _ => OutOfBounds::Fixed,
            };
            let g = |k: usize| nums(&ls[i + k].1);
            out.push(Tbl {
                method,
                ob,
                xs: g(1),
                ys: g(2),
                qin: g(3),
                vin: g(4),
                qout: g(5),
                vout: g(6),
                ks: g(7),
                int: g(8),
                nq: g(9),
                np: g(10),
            });
            i += 11;
        } else {
            i += 1;
        }
    }
    out
}

struct TblResult {
    values: Tally,
    integral_k1: Tally,
    integral_other: Tally,
    next_point: Tally,
    tables: usize,
}

fn run_tables() -> TblResult {
    let mut r = TblResult {
        values: Tally::default(),
        integral_k1: Tally::default(),
        integral_other: Tally::default(),
        next_point: Tally::default(),
        tables: 0,
    };
    for t in tbl_cases() {
        r.tables += 1;
        let table =
            ScalarInterpolateTable::new(t.xs.clone(), t.ys.clone(), t.method, t.ob).expect("table");
        for (q, v) in t.qin.iter().zip(&t.vin) {
            r.values.add(table.interpolate(*q).expect("in range"), *v);
        }
        for (q, v) in t.qout.iter().zip(&t.vout) {
            r.values.add(table.interpolate(*q).expect("policy"), *v);
        }
        for (k, v) in t.ks.iter().zip(&t.int) {
            let tally = if *k == 1.0 { &mut r.integral_k1 } else { &mut r.integral_other };
            tally.add(table.integral(*k), *v);
        }
        for (q, v) in t.nq.iter().zip(&t.np) {
            r.next_point.add(table.next_point(*q).expect("next"), *v);
        }
    }
    r
}

#[test]
fn interpolate_table_against_upstream() {
    let r = run_tables();
    assert_eq!(r.tables, 18);
    // 618 values over {step, linear} x {error, extrapolation, fixed}, 90
    // integrals (k = 1, 2, 3, 0.5, 1.5) and 504 next points: all identical.
    assert_eq!(r.values.n, 618);
    assert_eq!(r.values.differ, 0);
    assert_eq!(r.integral_k1.n, 18);
    assert_eq!(r.integral_k1.differ, 0);
    assert_eq!(r.integral_other.n, 72);
    assert_eq!(r.integral_other.differ, 0);
    assert_eq!(r.next_point.n, 504);
    assert_eq!(r.next_point.differ, 0);
}

struct Tp {
    constant: Option<f64>,
    start: f64,
    table: Vec<(f64, f64)>,
    q: Vec<f64>,
    v: Vec<f64>,
}

fn parse_dict(dict: &[String]) -> (Option<f64>, f64, Vec<(f64, f64)>) {
    let text = dict.join(" ");
    if text.contains("type constant") {
        let c = text
            .split("constant ")
            .last()
            .unwrap()
            .trim_end_matches(';')
            .trim()
            .parse::<f64>()
            .unwrap();
        return (Some(c), 0.0, vec![]);
    }
    let start = text
        .split("startTime ")
        .nth(1)
        .unwrap()
        .split(';')
        .next()
        .unwrap()
        .trim()
        .parse::<f64>()
        .unwrap();
    let body = text.split("table table (").nth(1).unwrap().trim_end_matches(';').trim_end_matches(')');
    let table = body
        .split('(')
        .filter(|s| !s.trim().is_empty())
        .map(|pair| {
            let mut it = pair.trim_end_matches(')').split_whitespace();
            (it.next().unwrap().parse().unwrap(), it.next().unwrap().parse().unwrap())
        })
        .collect();
    (None, start, table)
}

fn tp_cases() -> Vec<Tp> {
    let ls = lines();
    let mut out = Vec::new();
    let mut i = 0;
    while i < ls.len() {
        if ls[i].0 == "TP" {
            let (constant, start, table) = parse_dict(&ls[i + 1].1);
            out.push(Tp { constant, start, table, q: nums(&ls[i + 2].1), v: nums(&ls[i + 3].1) });
            i += 4;
        } else {
            i += 1;
        }
    }
    out
}

struct TpResult {
    constant: Tally,
    inside: Tally,
    clamped: Tally,
    profiles: usize,
}

fn run_time_profiles(foundation: bool) -> TpResult {
    let mut r = TpResult {
        constant: Tally::default(),
        inside: Tally::default(),
        clamped: Tally::default(),
        profiles: 0,
    };
    for t in tp_cases() {
        r.profiles += 1;
        let profile = match t.constant {
            Some(c) => TimeProfile::constant(c),
            None => TimeProfile::table(
                t.table.iter().map(|p| Time::new::<second>(p.0)).collect(),
                t.table.iter().map(|p| p.1).collect(),
            )
            .expect("table")
            .with_start_time(Time::new::<second>(t.start)),
        };
        for (q, v) in t.q.iter().zip(&t.v) {
            let tq = Time::new::<second>(*q);
            let got = if foundation { profile.value_foundation(tq) } else { profile.value(tq) };
            if t.constant.is_some() {
                r.constant.add(got, *v);
            } else {
                let shifted = *q - t.start;
                let (lo, hi) = (t.table[0].0, t.table[t.table.len() - 1].0);
                if shifted <= lo || shifted >= hi {
                    r.clamped.add(got, *v);
                } else {
                    r.inside.add(got, *v);
                }
            }
        }
    }
    r
}

#[test]
fn time_profile_against_upstream() {
    let r = run_time_profiles(false);
    assert_eq!(r.profiles, 13);
    assert_eq!((r.constant.n, r.constant.differ), (4, 0));
    assert_eq!((r.clamped.n, r.clamped.differ), (41, 0));
    assert_eq!((r.inside.n, r.inside.differ), (667, 0));
}

/// The Foundation-lineage `interpolateXY` form of a table, kept callable
/// (`TimeProfile::value_foundation`), is pinned at the numbers measured
/// against GeN-Foam on 2026-11-12: 242 of 667 interior values differ, by up
/// to 24 ulp (4.7e-15 relative), constant and clamped values agree.
#[test]
fn foundation_form_of_a_table_differs_from_upstream() {
    let r = run_time_profiles(true);
    assert_eq!((r.constant.differ, r.clamped.differ), (0, 0));
    assert_eq!((r.inside.n, r.inside.differ), (667, 242));
    assert_eq!(r.inside.max_ulp, 24);
    assert!(r.inside.max_rel > 4.7e-15 && r.inside.max_rel < 4.71e-15, "{:e}", r.inside.max_rel);
}
