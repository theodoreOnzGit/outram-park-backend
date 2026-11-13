// SPDX-License-Identifier: GPL-3.0-only
// Copyright (C) 2026 OUTRAM PARK contributors
//
//! # Thermal expansion (14 upstream models) vs OFFBEAT 80e84450 (gh:#1393, round 2)
//!
//! ## Methodology
//!
//! Reference: `reference_thermal_expansion.txt`, written by
//! `dump_thermal_expansion.cpp` (verbatim `correct()` bodies of OFFBEAT's
//! `thermalExpansion*.C`; transcription, not a run of the compiled library).
//! Predictions and the derived gates: `PREDICTIONS_round2.md` section A.
//!
//! Gate G1: `|port - upstream| <= 8 eps * scale`, `scale` = sum of |additive
//! terms| of both operands (emitted by the driver).  Gate G2: relative
//! difference < 1e-14 where `|result| >= 0.2 scale`.  PARFUME buffer / PyC:
//! upstream's dead zone omits `mag()`; the port deliberately does not
//! reproduce that, so the gates are applied against the driver's `mag()`-fixed
//! twin, and the divergence from upstream-as-written is counted and pinned.
//!
//! ## Results
//!
//! See `verification_and_validation/thermal_expansion_vs_offbeat.md`.

mod offbeat_differential_support;

use offbeat_differential_support::{parse_rows, report, Row, Stats};
use outram_park_fork_offbeat::materials::properties::thermal_expansion::ThermalExpansionModel;
use outram_park_fork_offbeat::materials::MaterialState;
use std::collections::BTreeMap;

const FIXTURE: &str = include_str!(
    "../verification_and_validation/offbeat_differential_gh1393/reference_thermal_expansion.txt"
);
const EPS: f64 = f64::EPSILON;

/// Columns: T, Tref, a, b, c, d, scale.  `None` when outside the port's window.
fn port_value(name: &str, c: &[f64]) -> Option<f64> {
    let (t, tref, a, b, cc) = (c[0], c[1], c[2], c[3], c[4]);
    let mut s = MaterialState::fresh(t);
    let model = match name {
        "Constant" => ThermalExpansionModel::Constant { alpha: a, t_ref: tref },
        "PARFUMESiC" => ThermalExpansionModel::PARFUMESiC { alpha: a, t_ref: tref },
        "RelapUO2" => ThermalExpansionModel::RelapUO2 { t_ref: tref },
        "MatproUPuO2" => {
            s.pu_fraction = a;
            ThermalExpansionModel::MatproUPuO2
        }
        "MartinUPuO2" => {
            s.oxygen_deviation = a - 2.0;
            ThermalExpansionModel::MartinUPuO2 { t_ref: tref }
        }
        "LemehovUPuO2" => {
            s.pu_fraction = a;
            s.oxygen_deviation = b - 2.0;
            s.burnup = cc / 1000.0;
            ThermalExpansionModel::LemehovUPuO2
        }
        "MAMOX" => {
            s.pu_fraction = a;
            s.oxygen_deviation = b - 2.0;
            ThermalExpansionModel::MAMOX { t_ref: tref }
        }
        "MatproZy" => ThermalExpansionModel::MatproZy { t_ref: tref },
        "Gehr1515Ti" => ThermalExpansionModel::Gehr1515Ti,
        "Molybdenum" => ThermalExpansionModel::Molybdenum,
        "SneadSiC" => ThermalExpansionModel::SneadSiC { t_stress_free: tref },
        "SwindemanHastelloyN" => ThermalExpansionModel::SwindemanHastelloyN { t_ref: tref },
        n if n.starts_with("PARFUMEBuffer") => ThermalExpansionModel::PARFUMEBuffer { t_ref: tref },
        n if n.starts_with("PARFUMEPyC") => {
            ThermalExpansionModel::PARFUMEPyC { t_ref: tref, anisotropy_factor: a }
        }
        _ => return None,
    };
    let (lo, hi) = model.validity_range();
    if t < lo || t > hi {
        return None;
    }
    let p = model.principal_strains(&s);
    Some(if name.starts_with("PARFUMEPyC_t") { p[1] } else { p[0] })
}

/// Fixture rows gated against the port: everything except the `_up` twins.
fn gated_name(name: &str) -> Option<String> {
    if name.ends_with("_up") {
        return None;
    }
    Some(name.trim_end_matches("_fix").to_string())
}

fn model_key(name: &str) -> String {
    name.trim_end_matches("_up").trim_end_matches("_fix").to_string()
}

#[derive(Default)]
struct Extra {
    g1_fail: usize,
    g2_fail: usize,
    g2_checked: usize,
    worst_cond_rel: f64,
}

struct Outcome {
    stats: BTreeMap<String, Stats>,
    extra: BTreeMap<String, Extra>,
    pairs: usize,
    diverge: usize,
    diverge_port_nonzero: usize,
}

fn run() -> Outcome {
    let rows = parse_rows(FIXTURE);
    let mut stats: BTreeMap<String, Stats> = BTreeMap::new();
    let mut extra: BTreeMap<String, Extra> = BTreeMap::new();
    let mut by_key: BTreeMap<String, (Option<&Row>, Option<&Row>)> = BTreeMap::new();
    for r in &rows {
        if let Some(g) = gated_name(&r.name) {
            let scale = *r.col.last().unwrap();
            match port_value(&r.name, &r.col) {
                None => {
                    stats.entry(g).or_default().skipped_outside_window += 1;
                }
                Some(port) => {
                    stats.entry(g.clone()).or_default().add(port, r.reference, &format!("{:?}", r.col));
                    let e = extra.entry(g).or_default();
                    let d = (port - r.reference).abs();
                    if d > 8.0 * EPS * scale {
                        e.g1_fail += 1;
                    }
                    if r.reference != 0.0 && r.reference.abs() >= 0.2 * scale {
                        e.g2_checked += 1;
                        if d / r.reference.abs() >= 1e-14 {
                            e.g2_fail += 1;
                        }
                    } else if r.reference != 0.0 {
                        e.worst_cond_rel = e.worst_cond_rel.max(d / r.reference.abs());
                    }
                }
            }
        }
        let key = format!("{}|{:?}", model_key(&r.name), r.col);
        let slot = by_key.entry(key).or_insert((None, None));
        if r.name.ends_with("_up") {
            slot.0 = Some(r);
        } else if r.name.ends_with("_fix") {
            slot.1 = Some(r);
        }
    }
    let (mut pairs, mut diverge, mut diverge_port_nonzero) = (0, 0, 0);
    for (up, fix) in by_key.values() {
        if let (Some(u), Some(f)) = (up, fix) {
            pairs += 1;
            if u.reference != f.reference {
                diverge += 1;
                assert_eq!(u.reference, 0.0, "upstream-as-written must be exactly zero when it diverges");
                assert!(u.col[0] < u.col[1], "divergence only below Tref");
                if port_value(&f.name, &f.col).map_or(false, |p| p != 0.0) {
                    diverge_port_nonzero += 1;
                }
            }
        }
    }
    Outcome { stats, extra, pairs, diverge, diverge_port_nonzero }
}

#[test]
fn thermal_expansion_agrees_with_the_transcribed_offbeat_kernels() {
    let o = run();
    report(&o.stats);
    for (n, e) in &o.extra {
        println!(
            "{n:<22} G1 fail {} | G2 checked {} fail {} | worst rel near cancellation {:.3e}",
            e.g1_fail, e.g2_checked, e.g2_fail, e.worst_cond_rel
        );
    }
    println!(
        "up/fix pairs {}, upstream-as-written diverges (zero below Tref) {}, port non-zero there {}",
        o.pairs, o.diverge, o.diverge_port_nonzero
    );
    for (n, s) in &o.stats {
        assert!(s.compared > 0, "{n}: nothing compared");
    }
    for (n, e) in &o.extra {
        assert_eq!(e.g1_fail, 0, "{n}: G1 violated");
        assert_eq!(e.g2_fail, 0, "{n}: G2 violated");
    }
    assert_eq!(o.diverge, o.diverge_port_nonzero, "port must keep its documented non-zero contraction");
}
