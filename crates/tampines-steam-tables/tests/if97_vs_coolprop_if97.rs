//! Differential comparison of `tampines-steam-tables` against CoolProp's `IF97.h`
//! and rust-steam (gh:#1346, under epic gh:#1336).
//!
//! # Methodology
//!
//! * **Reference data.** `verification_and_validation/if97_differential/` holds two
//!   gzipped CSVs produced by upstream code actually run, not transcribed:
//!   `coolprop_if97_reference.csv.gz` (inputs + CoolProp IF97.h outputs, written by
//!   `coolprop_if97_driver.cpp`) and `rusteam_reference.csv.gz` (same inputs, rust-steam
//!   outputs, written by `rusteam_driver/`). Provenance, licences, versions and the exact
//!   regeneration commands are in that folder's `README.md`.
//! * **Inputs.** (T,p): T every 30 K 273.15-1073.15, every 100 K to 2273.15, plus
//!   +-1 ulp around 273.15, 623.15, 647.096, 863.15, 1073.15, 2273.15 K, and a dense block
//!   near the critical point; p log-spaced 611.213 Pa-100 MPa plus 50/100 MPa, p_sat(T)
//!   and B23(T) with +-1 ulp and +-1e-9 neighbours. (p,h), (p,s): log-spaced p, a linear h/s
//!   sweep through the dome plus saturated-liquid/vapour values +-1e-9..1e-4. (h,s): a
//!   coarse rectangle, many points of which are outside IF97.
//! * **Measure.** Relative difference |a-b|/max(|a|,|b|), reported as count compared, count
//!   bit-identical, count within 1 ulp, worst relative difference and where it occurs.
//! * **Pass criterion.** This test PINS the measured numbers (counts exact, worst relative
//!   difference bounded just above what was measured). The pins are a regression record,
//!   not a claim that the differences are physically acceptable; the interpretation is in
//!   `verification_and_validation/if97_differential/RESULTS.md`, and the predictions that
//!   were written before the first run are in `PREDICTIONS.md` in the same folder.
//!
//! Results, dates and interpretation: see `RESULTS.md` (measured 2026-11-13).

use miniz_oxide::inflate::decompress_to_vec;
use std::collections::BTreeMap;
use std::panic::{catch_unwind as raw_catch, AssertUnwindSafe};
use tampines_steam_tables::interfaces::functional_programming::pt_flash_eqm::FwdEqnRegion;
use tampines_steam_tables::prelude::*;
use tampines_steam_tables::region_1_subcooled_liquid::{h_tp_1, s_tp_1};
use tampines_steam_tables::region_2_vapour::{h_tp_2, s_tp_2};
use tampines_steam_tables::region_3_single_phase_plus_supercritical_steam::p_boundary_2_3;
use tampines_steam_tables::surface_tension::water_surf_tension;
use tampines_steam_tables::thermal_conductivity::lambda_tp_eqm_single_phase;
use tampines_steam_tables::dynamic_viscosity::mu_tp_eqm_single_phase;
use uom::si::available_energy::joule_per_kilogram;
use uom::si::dynamic_viscosity::pascal_second;
use uom::si::f64::*;
use uom::si::pressure::pascal;
use uom::si::radiant_exposure::joule_per_square_meter;
use uom::si::ratio::ratio;
use uom::si::specific_heat_capacity::joule_per_kilogram_kelvin;
use uom::si::specific_volume::cubic_meter_per_kilogram;
use uom::si::thermal_conductivity::watt_per_meter_kelvin;
use uom::si::thermodynamic_temperature::kelvin;
use uom::si::velocity::meter_per_second;

const COOLPROP_GZ: &[u8] = include_bytes!(
    "../verification_and_validation/if97_differential/coolprop_if97_reference.csv.gz"
);
const RUSTEAM_GZ: &[u8] = include_bytes!(
    "../verification_and_validation/if97_differential/rusteam_reference.csv.gz"
);

/// Inflate a gzip member written with `gzip -n` / Python `mtime=0` (10-byte header,
/// 8-byte trailer).
fn gunzip(bytes: &[u8]) -> String {
    assert!(bytes.len() > 18 && bytes[0] == 0x1f && bytes[1] == 0x8b && bytes[3] == 0);
    let raw = decompress_to_vec(&bytes[10..bytes.len() - 8]).expect("inflate");
    String::from_utf8(raw).expect("utf8")
}

fn parse(s: &str) -> f64 {
    match s {
        "nan" | "panic" => f64::NAN,
        _ => s.parse::<f64>().expect("float"),
    }
}

fn rows(text: &str) -> Vec<(String, Vec<String>)> {
    text.lines()
        .map(|l| {
            let mut it = l.split(',');
            let tag = it.next().unwrap().to_string();
            (tag, it.map(|x| x.to_string()).collect())
        })
        .collect()
}

/// Statistics for one (group, quantity) cell.
#[derive(Default, Clone)]
struct Stat {
    n: usize,
    bit: usize,
    ulp1: usize,
    max_rel: f64,
    worst: String,
    /// the three worst (rel, location, tampines, upstream)
    top: Vec<(f64, String, f64, f64)>,
}

fn ulp_apart(a: f64, b: f64) -> bool {
    if a.is_sign_negative() != b.is_sign_negative() {
        return a == b;
    }
    let (x, y) = (a.to_bits() as i64, b.to_bits() as i64);
    (x - y).abs() <= 1
}

fn rel(a: f64, b: f64) -> f64 {
    if a == b {
        0.0
    } else {
        (a - b).abs() / a.abs().max(b.abs())
    }
}

#[derive(Default)]
struct Report {
    cells: BTreeMap<String, Stat>,
    /// notes such as region mismatches, one-sided failures (capped per key)
    notes: BTreeMap<String, (usize, Vec<String>)>,
}

impl Report {
    fn cmp(&mut self, key: &str, tampines: f64, upstream: f64, at: &str) {
        let c = self.cells.entry(key.to_string()).or_default();
        c.n += 1;
        if tampines.to_bits() == upstream.to_bits() {
            c.bit += 1;
        }
        if ulp_apart(tampines, upstream) {
            c.ulp1 += 1;
        }
        let r = rel(tampines, upstream);
        if r > c.max_rel {
            c.max_rel = r;
            c.worst = at.to_string();
        }
        if r > 0.0 {
            c.top.push((r, at.to_string(), tampines, upstream));
            c.top.sort_by(|a, b| b.0.partial_cmp(&a.0).unwrap());
            c.top.truncate(3);
        }
    }
    fn note(&mut self, key: &str, at: String) {
        let e = self.notes.entry(key.to_string()).or_insert((0, vec![]));
        e.0 += 1;
        if e.1.len() < 40 {
            e.1.push(at);
        }
    }
}

/// Run a TAMPINES call, turning a panic into `None`.
fn catch_unwind<R, F: FnOnce() -> R>(f: F) -> std::thread::Result<R> {
    raw_catch(AssertUnwindSafe(f))
}

fn attempt<F: FnOnce() -> f64>(f: F) -> Option<f64> {
    catch_unwind(f).ok()
}

fn region_num(r: FwdEqnRegion) -> i32 {
    match r {
        FwdEqnRegion::Region1 => 1,
        FwdEqnRegion::Region2 => 2,
        FwdEqnRegion::Region3 => 3,
        FwdEqnRegion::Region4 => 4,
        FwdEqnRegion::Region5 => 5,
    }
}

const TC: f64 = 647.096;
const PC: f64 = 22.064e6;

/// On a boundary point CoolProp is not compared, but rust-steam (same region logic as this
/// crate) still is: both take the same side or the difference is itself a finding.
fn continue_after_boundary(rep: &mut Report, rrow: Option<&(String, Vec<String>)>, tp: &Tp, at: &str) {
    if let Some((_, rr)) = rrow {
        let rv: Vec<f64> = (2..9).map(|i| parse(&rr[i])).collect();
        let rus_ok = !rv[1].is_nan();
        if rus_ok && !tp.h.is_nan() {
            let g = format!("R{}", tp.region);
            let names = ["v", "h", "s", "u", "cp", "cv", "w"];
            let tv = [tp.v, tp.h, tp.s, tp.u, tp.cp, tp.cv, tp.w];
            for k in 0..7 {
                rep.cmp(&format!("RUS/{}/{}", g, names[k]), tv[k], rv[k], at);
            }
        } else if rus_ok != !tp.h.is_nan() {
            rep.note("RUS/boundary_one_sided", at.to_string());
        }
    }
}

fn near_critical(t: f64, p: f64) -> bool {
    (t - TC).abs() < 10.0 && (p - PC).abs() < 3.0e6
}

fn tp_group(region: i32, t: f64, p: f64) -> String {
    if region == 3 && near_critical(t, p) {
        "R3crit".to_string()
    } else {
        format!("R{}", region)
    }
}

struct Tp {
    v: f64,
    h: f64,
    s: f64,
    u: f64,
    cp: f64,
    cv: f64,
    w: f64,
    mu: f64,
    lam: f64,
    region: i32,
}

fn tampines_tp(t: f64, p: f64) -> Tp {
    let tt = ThermodynamicTemperature::new::<kelvin>(t);
    let pp = Pressure::new::<pascal>(p);
    let nan = f64::NAN;
    let region = catch_unwind(|| region_num(region_fwd_eqn_single_phase(tt, pp))).unwrap_or(-1);
    Tp {
        region,
        v: attempt(|| v_tp_eqm_single_phase(tt, pp).get::<cubic_meter_per_kilogram>()).unwrap_or(nan),
        h: attempt(|| h_tp_eqm_single_phase(tt, pp).get::<joule_per_kilogram>()).unwrap_or(nan),
        s: attempt(|| s_tp_eqm_single_phase(tt, pp).get::<joule_per_kilogram_kelvin>()).unwrap_or(nan),
        u: attempt(|| u_tp_eqm_single_phase(tt, pp).get::<joule_per_kilogram>()).unwrap_or(nan),
        cp: attempt(|| cp_tp_eqm_single_phase(tt, pp).get::<joule_per_kilogram_kelvin>()).unwrap_or(nan),
        cv: attempt(|| cv_tp_eqm_single_phase(tt, pp).get::<joule_per_kilogram_kelvin>()).unwrap_or(nan),
        w: attempt(|| w_tp_eqm_single_phase(tt, pp).get::<meter_per_second>()).unwrap_or(nan),
        mu: attempt(|| mu_tp_eqm_single_phase(tt, pp).get::<pascal_second>()).unwrap_or(nan),
        lam: attempt(|| lambda_tp_eqm_single_phase(tt, pp).get::<watt_per_meter_kelvin>()).unwrap_or(nan),
    }
}

/// Run the whole comparison. Pure function of the two committed reference files.
fn run_comparison() -> Report {
    let cool = rows(&gunzip(COOLPROP_GZ));
    let rus = rows(&gunzip(RUSTEAM_GZ));
    let mut rep = Report::default();
    // rust-steam rows are in the same order as the CoolProp rows of the tags it answers
    let mut rus_iter = rus.iter();
    for (tag, c) in &cool {
        let n = |i: usize| parse(&c[i]);
        let answers_rus = matches!(tag.as_str(), "TP" | "SATT" | "SATP" | "PH" | "PS");
        let rrow = if answers_rus { rus_iter.next() } else { None };
        if let Some((rtag, r)) = rrow {
            assert_eq!(rtag, tag, "row order drift between the two reference files");
            assert_eq!(parse(&r[0]), n(0), "input drift (first input)");
            if tag == "TP" || tag == "PH" || tag == "PS" {
                assert_eq!(parse(&r[1]), n(1), "input drift (second input)");
            }
        }
        let r = |i: usize| rrow.map(|(_, r)| parse(&r[i])).unwrap_or(f64::NAN);
        match tag.as_str() {
            "TP" => {
                let (t, p) = (n(0), n(1));
                let at = format!("T={:.17e} p={:.17e}", t, p);
                let cool_region = c[2].parse::<i32>().unwrap();
                let tp = tampines_tp(t, p);
                // Points within 1e-8 (relative) of the saturation curve or of B23 are
                // classified as boundary points: the two codes round psat/B23 differently, so
                // which side they land on is round-off, not a physics difference. They are
                // counted but their VALUES are not compared (liquid vs vapour would differ 1000x).
                let tt = ThermodynamicTemperature::new::<kelvin>(t);
                let psat = if (273.15..=TC).contains(&t) { Some(sat_pressure_4(tt).get::<pascal>()) } else { None };
                let b23 = if (623.15..=863.15).contains(&t) { Some(p_boundary_2_3(tt).get::<pascal>()) } else { None };
                let on_psat = psat.map(|x| (p - x).abs() / x < 1e-8).unwrap_or(false);
                let on_b23 = b23.map(|x| (p - x).abs() / x < 1e-8).unwrap_or(false);
                if on_psat || on_b23 {
                    let class = if on_psat { "psat" } else { "B23" };
                    rep.note(&format!("TP/boundary_point_{}", class), at.clone());
                    if cool_region > 0 && !tp.h.is_nan() && tp.region != cool_region {
                        rep.note(&format!("TP/boundary_region_flip_{}", class), format!("{} cool=R{} tampines=R{}", at, cool_region, tp.region));
                    }
                    if cool_region > 0 && tp.h.is_nan() {
                        rep.note(&format!("TP/boundary_coolprop_ok_tampines_refuses_{}", class), format!("{} cool=R{}", at, cool_region));
                    }
                    if cool_region <= 0 && !tp.h.is_nan() {
                        rep.note(&format!("TP/boundary_tampines_ok_coolprop_refuses_{}", class), format!("{} tampines=R{}", at, tp.region));
                    }
                    continue_after_boundary(&mut rep, rrow, &tp, &at);
                    continue;
                }
                // region agreement
                let cool_ok = cool_region > 0 && !n(3).is_nan();
                let tam_ok = !tp.h.is_nan();
                match (cool_ok, tam_ok) {
                    (true, true) => {
                        rep.cells.entry("TP/region_agree".into()).or_default().n += 1;
                        if tp.region != cool_region {
                            let class = if (t - 623.15).abs() < 1e-9 {
                                "T623.15"
                            } else if (t - 1073.15).abs() < 1e-9 {
                                "T1073.15"
                            } else {
                                "other"
                            };
                            rep.note(&format!("TP/region_mismatch_{}", class), format!("{} cool=R{} tampines=R{}", at, cool_region, tp.region));
                            if class != "other" {
                                // size of the shared-boundary ambiguity: both sides are valid
                                // IF97 states at the boundary, so quantify how far apart they are
                                for (name, tv, cv) in [
                                    ("v", tp.v, 1.0 / n(3)),
                                    ("h", tp.h, n(4)),
                                    ("s", tp.s, n(5)),
                                    ("cp", tp.cp, n(7)),
                                ] {
                                    rep.cmp(&format!("TP/{}_other_side/{}", class, name), tv, cv, &at);
                                }
                            }
                        } else {
                            rep.cells.entry("TP/region_agree".into()).or_default().bit += 1;
                        }
                    }
                    (true, false) => rep.note("TP/coolprop_ok_tampines_refuses", format!("{} cool=R{}", at, cool_region)),
                    (false, true) => rep.note("TP/tampines_ok_coolprop_refuses", format!("{} tampines=R{}", at, tp.region)),
                    (false, false) => {
                        rep.note("TP/both_refuse", at.clone());
                    }
                }
                if cool_ok && tam_ok && tp.region == cool_region {
                    let g = tp_group(cool_region, t, p);
                    // CoolProp gives rho; compare v with 1/rho
                    rep.cmp(&format!("TP/{}/v", g), tp.v, 1.0 / n(3), &at);
                    rep.cmp(&format!("TP/{}/h", g), tp.h, n(4), &at);
                    rep.cmp(&format!("TP/{}/s", g), tp.s, n(5), &at);
                    rep.cmp(&format!("TP/{}/u", g), tp.u, n(6), &at);
                    rep.cmp(&format!("TP/{}/cp", g), tp.cp, n(7), &at);
                    rep.cmp(&format!("TP/{}/cv", g), tp.cv, n(8), &at);
                    rep.cmp(&format!("TP/{}/w", g), tp.w, n(9), &at);
                    if !tp.mu.is_nan() && !n(10).is_nan() {
                        rep.cmp(&format!("TP/{}/mu", g), tp.mu, n(10), &at);
                    } else {
                        rep.note("TP/transport_one_sided", format!("{} mu tampines={} cool={}", at, tp.mu, n(10)));
                    }
                    if !tp.lam.is_nan() && !n(11).is_nan() {
                        rep.cmp(&format!("TP/{}/lambda", g), tp.lam, n(11), &at);
                    } else {
                        rep.note("TP/transport_one_sided", format!("{} lambda tampines={} cool={}", at, tp.lam, n(11)));
                    }
                }
                // rust-steam vs TAMPINES (rust-steam columns: v h s u cp cv w at 2..)
                if let Some((_, rr)) = rrow {
                    let rv: Vec<f64> = (2..9).map(|i| parse(&rr[i])).collect();
                    let rus_ok = !rv[1].is_nan();
                    if rus_ok && tam_ok {
                        let g = format!("R{}", tp.region);
                        let names = ["v", "h", "s", "u", "cp", "cv", "w"];
                        let tv = [tp.v, tp.h, tp.s, tp.u, tp.cp, tp.cv, tp.w];
                        for k in 0..7 {
                            // the driver multiplied kJ outputs by 1000.0; the same is done there
                            rep.cmp(&format!("RUS/{}/{}", g, names[k]), tv[k], rv[k], &at);
                        }
                    } else if rus_ok != tam_ok {
                        let who = if rus_ok { "rusteam_ok_tampines_refuses" } else { "tampines_ok_rusteam_refuses" };
                        rep.note(&format!("RUS/{}", who), format!("{} tampines_region=R{}", at, tp.region));
                    }
                }
            }
            "SATT" => {
                let t = n(0);
                let at = format!("T={:.17e}", t);
                let tt = ThermodynamicTemperature::new::<kelvin>(t);
                let ps = sat_pressure_4(tt).get::<pascal>();
                rep.cmp("SAT/psat", ps, n(1), &at);
                if !n(2).is_nan() {
                    let sg = water_surf_tension(tt).get::<joule_per_square_meter>();
                    rep.cmp("SAT/sigma", sg, n(2), &at);
                }
                if rrow.is_some() {
                    rep.cmp("RUS/SAT/psat", ps, r(1), &at);
                }
            }
            "SATP" => {
                let p = n(0);
                let at = format!("p={:.17e}", p);
                let ts = sat_temp_4(Pressure::new::<pascal>(p)).get::<kelvin>();
                rep.cmp("SAT/Tsat", ts, n(1), &at);
                if rrow.is_some() {
                    rep.cmp("RUS/SAT/Tsat", ts, r(1), &at);
                }
            }
            "PH" | "PS" => {
                let (p, x_in) = (n(0), n(1));
                let at = format!("p={:.17e} {}={:.17e}", p, if tag == "PH" { "h" } else { "s" }, x_in);
                let pp = Pressure::new::<pascal>(p);
                let cool_region = n(6) as i32; // NaN -> 0
                let t_c = n(2);
                let (t_t, v_t, x_t, o_t, reg_t);
                if tag == "PH" {
                    let hh = AvailableEnergy::new::<joule_per_kilogram>(x_in);
                    t_t = attempt(|| t_ph_eqm(pp, hh).get::<kelvin>());
                    v_t = attempt(|| v_ph_eqm(pp, hh).get::<cubic_meter_per_kilogram>());
                    x_t = attempt(|| x_ph_flash(pp, hh));
                    o_t = attempt(|| s_ph_eqm(pp, hh).get::<joule_per_kilogram_kelvin>());
                    reg_t = catch_unwind(|| region_num(ph_flash_region(pp, hh))).ok();
                } else {
                    let ss = SpecificHeatCapacity::new::<joule_per_kilogram_kelvin>(x_in);
                    t_t = attempt(|| t_ps_eqm(pp, ss).get::<kelvin>());
                    v_t = attempt(|| v_ps_eqm(pp, ss).get::<cubic_meter_per_kilogram>());
                    x_t = attempt(|| x_ps_flash(pp, ss));
                    o_t = attempt(|| h_ps_eqm(pp, ss).get::<joule_per_kilogram>());
                    reg_t = catch_unwind(|| region_num(ps_flash_region(pp, ss))).ok();
                }
                let cool_ok = !t_c.is_nan();
                // saturated-liquid / saturated-vapour edge, from this crate's own Region 1/2
                // values at Tsat(p): within 1e-8 relative the side is round-off
                let dedge: f64 = if p < PC && p >= 611.2 && cool_ok && t_t.is_some() {
                    catch_unwind(|| {
                        let ts = sat_temp_4(pp);
                        let (xl, xv) = if tag == "PH" {
                            (h_tp_1(ts, pp).get::<joule_per_kilogram>(), h_tp_2(ts, pp).get::<joule_per_kilogram>())
                        } else {
                            (s_tp_1(ts, pp).get::<joule_per_kilogram_kelvin>(), s_tp_2(ts, pp).get::<joule_per_kilogram_kelvin>())
                        };
                        let d = |a: f64, b: f64| (a - b).abs() / b.abs().max(1.0);
                        d(x_in, xl).min(d(x_in, xv))
                    })
                    .unwrap_or(f64::INFINITY)
                } else {
                    f64::INFINITY
                };
                let edge = dedge < 1e-8;
                if edge {
                    rep.note(&format!("{}/edge_point", tag), at.clone());
                    if reg_t.map(|r| r != cool_region).unwrap_or(true) {
                        rep.note(&format!("{}/edge_region_flip", tag), format!("{} cool=R{} tampines={:?}", at, cool_region, reg_t));
                    }
                    continue;
                }
                match (cool_ok, t_t.is_some()) {
                    (true, false) => rep.note(&format!("{}/coolprop_ok_tampines_refuses", tag), format!("{} cool=R{}", at, cool_region)),
                    (false, true) => rep.note(&format!("{}/tampines_ok_coolprop_refuses", tag), at.clone()),
                    (false, false) => rep.note(&format!("{}/both_refuse", tag), at.clone()),
                    (true, true) => {
                        let tsat = catch_unwind(|| sat_temp_4(pp).get::<kelvin>()).unwrap_or(f64::NAN);
                        let near = p < PC && (t_c - tsat).abs() < 0.05;
                        let g = if p < 612.0 {
                            // p = Pmin = 611.213 Pa row: CoolProp's Q and s misbehave there
                            format!("R{}pmin", cool_region)
                        } else if cool_region == 4 && dedge < 2e-3 {
                            // within 2e-3 (relative) of the saturated liquid/vapour state
                            "R4nearedge".to_string()
                        } else if cool_region == 4 {
                            "R4".to_string()
                        } else if near {
                            format!("R{}near_sat", cool_region)
                        } else {
                            format!("R{}", cool_region)
                        };
                        if let Some(rg) = reg_t {
                            rep.cells.entry(format!("{}/region_agree", tag)).or_default().n += 1;
                            if rg == cool_region {
                                rep.cells.entry(format!("{}/region_agree", tag)).or_default().bit += 1;
                            } else {
                                rep.note(&format!("{}/region_mismatch", tag), format!("{} cool=R{} tampines=R{}", at, cool_region, rg));
                            }
                        }
                        // values are compared only where both codes put the state in the same region
                        if reg_t != Some(cool_region) {
                            continue;
                        }
                        rep.cmp(&format!("{}/{}/T", tag, g), t_t.unwrap(), t_c, &at);
                        if let Some(v) = v_t {
                            rep.cmp(&format!("{}/{}/v", tag, g), v, 1.0 / n(3), &at);
                        }
                        if cool_region == 4 {
                            if let Some(x) = x_t {
                                rep.cmp(&format!("{}/{}/x", tag, g), x, n(4), &at);
                            }
                        }
                        if let Some(o) = o_t {
                            let name = if tag == "PH" { "s" } else { "h" };
                            rep.cmp(&format!("{}/{}/{}", tag, g, name), o, n(5), &at);
                        }
                        if let Some((_, rr)) = rrow {
                            let rt = parse(&rr[2]);
                            if !rt.is_nan() {
                                // rust-steam only implements the Region 1 and 2 backward
                                // equations, so only R1/R2 states can be compared; where it
                                // still answers for a Region 3 / dome state, that is counted
                                if cool_region == 1 || cool_region == 2 {
                                    rep.cmp(&format!("RUS/{}/T", tag), t_t.unwrap(), rt, &at);
                                } else {
                                    rep.note(&format!("RUS/{}_answers_for_R{}", tag, cool_region), at.clone());
                                }
                            }
                        }
                    }
                }
                // rust-steam answers where TAMPINES errored or vice versa
                if let Some((_, rr)) = rrow {
                    let rt = parse(&rr[2]);
                    if !rt.is_nan() && t_t.is_none() {
                        rep.note(&format!("RUS/{}_rusteam_ok_tampines_refuses", tag), at.clone());
                    }
                }
            }
            "HS" => {
                let (h, s) = (n(0), n(1));
                let at = format!("h={:.17e} s={:.17e}", h, s);
                let hh = AvailableEnergy::new::<joule_per_kilogram>(h);
                let ss = SpecificHeatCapacity::new::<joule_per_kilogram_kelvin>(s);
                let t_t = attempt(|| t_hs_eqm(hh, ss).get::<kelvin>());
                let p_t = attempt(|| p_hs_eqm(hh, ss).get::<pascal>());
                let x_t = attempt(|| x_hs_eqm(hh, ss).get::<ratio>());
                let cool_ok = !n(2).is_nan() && !n(3).is_nan();
                match (cool_ok, t_t.is_some() && p_t.is_some()) {
                    (true, false) => rep.note("HS/coolprop_ok_tampines_refuses", at.clone()),
                    (false, true) => rep.note("HS/tampines_ok_coolprop_refuses", at.clone()),
                    (false, false) => rep.note("HS/both_refuse", at.clone()),
                    (true, true) => {
                        let g = format!("R{}", n(5) as i32);
                        rep.cmp(&format!("HS/{}/T", g), t_t.unwrap(), n(2), &at);
                        rep.cmp(&format!("HS/{}/p", g), p_t.unwrap(), n(3), &at);
                        if n(5) as i32 == 4 {
                            if let Some(x) = x_t {
                                rep.cmp("HS/R4/x", x, n(4), &at);
                            }
                        }
                    }
                }
            }
            other => panic!("unknown tag {}", other),
        }
    }
    assert!(rus_iter.next().is_none(), "rust-steam file has surplus rows");
    rep
}

fn print_report(rep: &Report) {
    println!("{:<28} {:>6} {:>6} {:>6} {:>11}  worst at", "cell", "n", "bit", "<=1ulp", "max_rel");
    for (k, c) in &rep.cells {
        println!("{:<28} {:>6} {:>6} {:>6} {:>11.3e}  {}", k, c.n, c.bit, c.ulp1, c.max_rel, c.worst);
        if c.max_rel > 1e-9 && std::env::var("IF97_VERBOSE").is_ok() {
            for (r, at, a, b) in &c.top {
                println!("      top {:.3e} {} tampines={:.17e} upstream={:.17e}", r, at, a, b);
            }
        }
    }
    for (k, (n, ex)) in &rep.notes {
        println!("NOTE {} : {}", k, n);
        let show = if std::env::var("IF97_VERBOSE").is_ok() { 40 } else { 3 };
        for e in ex.iter().take(show) {
            println!("     {}", e);
        }
    }
}

#[test]
fn tampines_vs_coolprop_if97_and_rust_steam() {
    // panics are expected (single-phase flashes refuse states on the saturation line);
    // keep the log readable. The hook is process-global, so this file holds ONE test.
    std::panic::set_hook(Box::new(|_| {}));
    let rep = run_comparison();
    print_report(&rep);
    pins::check(&rep);
}

mod pins {
    use super::Report;
    /// (cell, n, bit_identical, within_1ulp, max_rel upper bound). Filled from the
    /// 2026-11-13 run; counts are exact, the bound sits just above the measurement.
    pub const CELLS: &[(&str, usize, usize, usize, f64)] = &[
        ("HS/R1/T", 109, 69, 77, 3.541000e-15),
        ("HS/R1/p", 109, 109, 109, 0.000000e+00),
        ("HS/R2/T", 192, 173, 188, 5.690000e-16),
        ("HS/R2/p", 192, 110, 168, 2.913000e-16),
        ("HS/R3/T", 20, 13, 13, 5.992000e-14),
        ("HS/R3/p", 20, 20, 20, 0.000000e+00),
        ("HS/R4/T", 69, 34, 34, 5.482000e-05),
        ("HS/R4/p", 69, 7, 9, 9.333000e-04),
        ("HS/R4/x", 69, 16, 25, 3.245000e-03),
        ("PH/R1/T", 230, 149, 176, 3.816000e-15),
        ("PH/R1/s", 230, 134, 153, 9.164000e-14),
        ("PH/R1/v", 230, 87, 183, 1.170000e-14),
        ("PH/R1near_sat/T", 38, 10, 12, 7.696000e-05),
        ("PH/R1near_sat/s", 38, 31, 33, 8.100000e-16),
        ("PH/R1near_sat/v", 38, 19, 36, 2.084000e-15),
        ("PH/R1pmin/T", 2, 2, 2, 0.000000e+00),
        ("PH/R1pmin/s", 2, 1, 1, 5.705000e-16),
        ("PH/R1pmin/v", 2, 0, 2, 2.170000e-16),
        ("PH/R2/T", 401, 365, 385, 3.088000e-15),
        ("PH/R2/s", 401, 260, 354, 1.923000e-15),
        ("PH/R2/v", 401, 188, 356, 5.966000e-15),
        ("PH/R2near_sat/T", 20, 3, 7, 2.656000e-05),
        ("PH/R2near_sat/s", 20, 11, 17, 4.733000e-15),
        ("PH/R2near_sat/v", 20, 6, 13, 1.677000e-14),
        ("PH/R2pmin/T", 19, 18, 18, 1.656000e-05),
        ("PH/R2pmin/s", 19, 7, 14, 3.407000e-06),
        ("PH/R2pmin/v", 19, 13, 18, 1.662000e-05),
        ("PH/R3/T", 78, 40, 40, 7.688000e-14),
        ("PH/R3/s", 78, 0, 0, 2.334000e-04),
        ("PH/R3/v", 78, 0, 0, 1.214000e-03),
        ("PH/R3near_sat/T", 6, 3, 3, 7.730000e-14),
        ("PH/R3near_sat/s", 6, 0, 0, 3.713000e-06),
        ("PH/R3near_sat/v", 6, 0, 0, 1.428000e-05),
        ("PH/R4/T", 414, 204, 228, 9.933000e-15),
        ("PH/R4/s", 414, 0, 0, 7.596000e-04),
        ("PH/R4/v", 414, 0, 0, 8.331000e-05),
        ("PH/R4/x", 414, 106, 180, 1.562000e-05),
        ("PH/R4nearedge/T", 79, 36, 40, 9.933000e-15),
        ("PH/R4nearedge/s", 79, 0, 0, 1.349000e-03),
        ("PH/R4nearedge/v", 79, 0, 0, 9.011000e-05),
        ("PH/R4nearedge/x", 79, 18, 22, 6.564000e-03),
        ("PH/R4pmin/T", 29, 29, 29, 0.000000e+00),
        ("PH/R4pmin/s", 29, 0, 0, 6.836000e-01),
        ("PH/R4pmin/v", 29, 0, 0, 1.470000e-06),
        ("PH/R4pmin/x", 29, 0, 0, 1.001000e+00),
        ("PH/region_agree", 1322, 1316, 0, 0.000000e+00),
        ("PS/R1/T", 233, 222, 233, 1.848000e-16),
        ("PS/R1/h", 233, 101, 185, 1.378000e-15),
        ("PS/R1/v", 233, 95, 221, 2.792000e-15),
        ("PS/R1near_sat/T", 38, 26, 26, 3.275000e-05),
        ("PS/R1near_sat/h", 38, 20, 32, 4.123000e-15),
        ("PS/R1near_sat/v", 38, 13, 32, 2.233000e-15),
        ("PS/R1pmin/T", 2, 2, 2, 0.000000e+00),
        ("PS/R1pmin/h", 2, 1, 2, 1.248000e-16),
        ("PS/R1pmin/v", 2, 1, 2, 2.170000e-16),
        ("PS/R2/T", 217, 217, 217, 0.000000e+00),
        ("PS/R2/h", 217, 97, 184, 6.629000e-16),
        ("PS/R2/v", 217, 115, 205, 3.805000e-16),
        ("PS/R2near_sat/T", 20, 8, 8, 2.804000e-05),
        ("PS/R2near_sat/h", 20, 11, 19, 3.344000e-16),
        ("PS/R2near_sat/v", 20, 13, 20, 2.039000e-16),
        ("PS/R2pmin/T", 7, 6, 6, 1.390000e-05),
        ("PS/R2pmin/h", 7, 6, 6, 2.861000e-06),
        ("PS/R2pmin/v", 7, 3, 6, 1.395000e-05),
        ("PS/R3/T", 43, 43, 43, 0.000000e+00),
        ("PS/R3/h", 43, 0, 0, 2.957000e-04),
        ("PS/R3/v", 43, 0, 0, 1.205000e-03),
        ("PS/R3near_sat/T", 6, 5, 5, 2.076000e-07),
        ("PS/R3near_sat/h", 6, 0, 0, 1.993000e-06),
        ("PS/R3near_sat/v", 6, 0, 0, 8.966000e-06),
        ("PS/R4/T", 442, 234, 265, 9.933000e-15),
        ("PS/R4/h", 442, 0, 0, 2.088000e-04),
        ("PS/R4/v", 442, 0, 0, 1.426000e-04),
        ("PS/R4/x", 442, 232, 278, 1.158000e-05),
        ("PS/R4nearedge/T", 81, 36, 40, 9.933000e-15),
        ("PS/R4nearedge/h", 81, 0, 0, 6.138000e-04),
        ("PS/R4nearedge/v", 81, 0, 0, 1.449000e-04),
        ("PS/R4nearedge/x", 81, 34, 39, 5.206000e-03),
        ("PS/R4pmin/T", 41, 41, 41, 0.000000e+00),
        ("PS/R4pmin/h", 41, 0, 0, 2.707000e-01),
        ("PS/R4pmin/v", 41, 0, 0, 2.488000e-07),
        ("PS/R4pmin/x", 41, 0, 0, 1.001000e+00),
        ("PS/region_agree", 1136, 1130, 0, 0.000000e+00),
        ("RUS/PH/T", 650, 645, 646, 1.931000e-01),
        ("RUS/PS/T", 483, 374, 419, 3.507000e-01),
        ("RUS/R1/cp", 514, 351, 512, 2.264000e-16),
        ("RUS/R1/cv", 514, 332, 514, 2.216000e-16),
        ("RUS/R1/h", 514, 298, 486, 4.025000e-16),
        ("RUS/R1/s", 514, 351, 514, 2.189000e-16),
        ("RUS/R1/u", 514, 303, 503, 2.704000e-16),
        ("RUS/R1/v", 514, 514, 514, 0.000000e+00),
        ("RUS/R1/w", 514, 514, 514, 0.000000e+00),
        ("RUS/R2/cp", 1870, 1012, 1817, 4.445000e-16),
        ("RUS/R2/cv", 1870, 607, 1427, 1.098000e-15),
        ("RUS/R2/h", 1870, 928, 1739, 4.400000e-16),
        ("RUS/R2/s", 1870, 1279, 1870, 2.222000e-16),
        ("RUS/R2/u", 1870, 1037, 1833, 3.461000e-16),
        ("RUS/R2/v", 1870, 1870, 1870, 0.000000e+00),
        ("RUS/R2/w", 1870, 1870, 1870, 0.000000e+00),
        ("RUS/R3/cp", 356, 232, 354, 1.332000e-03),
        ("RUS/R3/cv", 356, 227, 353, 4.183000e-04),
        ("RUS/R3/h", 356, 202, 346, 3.248000e-06),
        ("RUS/R3/s", 356, 197, 354, 3.167000e-06),
        ("RUS/R3/u", 356, 204, 343, 1.956000e-08),
        ("RUS/R3/v", 356, 354, 354, 1.771000e-05),
        ("RUS/R3/w", 356, 291, 354, 1.160000e-03),
        ("RUS/R5/cp", 752, 460, 751, 3.292000e-16),
        ("RUS/R5/cv", 752, 349, 680, 4.299000e-16),
        ("RUS/R5/h", 752, 427, 750, 2.528000e-16),
        ("RUS/R5/s", 752, 492, 752, 2.218000e-16),
        ("RUS/R5/u", 752, 473, 729, 2.358000e-16),
        ("RUS/R5/v", 752, 752, 752, 0.000000e+00),
        ("RUS/R5/w", 752, 752, 752, 0.000000e+00),
        ("RUS/SAT/Tsat", 120, 111, 113, 2.616000e-15),
        ("RUS/SAT/psat", 374, 374, 374, 0.000000e+00),
        ("SAT/Tsat", 120, 62, 71, 7.102000e-15),
        ("SAT/psat", 374, 85, 86, 6.663000e-15),
        ("SAT/sigma", 374, 226, 369, 3.255000e-16),
        ("TP/R1/cp", 481, 233, 420, 2.887000e-13),
        ("TP/R1/cv", 481, 324, 373, 2.743000e-12),
        ("TP/R1/h", 481, 254, 359, 1.533000e-12),
        ("TP/R1/lambda", 481, 0, 2, 4.639000e-12),
        ("TP/R1/mu", 481, 23, 62, 3.295000e-14),
        ("TP/R1/s", 481, 361, 381, 1.432000e-12),
        ("TP/R1/u", 481, 314, 334, 1.152000e-12),
        ("TP/R1/v", 481, 236, 445, 9.599000e-15),
        ("TP/R1/w", 481, 355, 415, 1.477000e-12),
        ("TP/R2/cp", 1804, 265, 606, 3.455000e-15),
        ("TP/R2/cv", 1804, 258, 324, 1.095000e-14),
        ("TP/R2/h", 1804, 853, 1516, 5.454000e-16),
        ("TP/R2/lambda", 1804, 644, 993, 3.136000e-11),
        ("TP/R2/mu", 1804, 629, 1438, 6.511000e-16),
        ("TP/R2/s", 1804, 1228, 1663, 5.894000e-16),
        ("TP/R2/u", 1804, 1260, 1565, 6.403000e-16),
        ("TP/R2/v", 1804, 927, 1708, 4.394000e-16),
        ("TP/R2/w", 1804, 1053, 1676, 4.824000e-15),
        ("TP/R3/cp", 210, 0, 0, 3.037000e-05),
        ("TP/R3/cv", 210, 0, 0, 4.068000e-06),
        ("TP/R3/h", 210, 0, 0, 2.045000e-06),
        ("TP/R3/lambda", 210, 0, 0, 6.158000e-06),
        ("TP/R3/mu", 210, 0, 0, 7.136000e-06),
        ("TP/R3/s", 210, 0, 0, 2.072000e-06),
        ("TP/R3/u", 210, 0, 0, 2.763000e-06),
        ("TP/R3/v", 210, 0, 0, 6.335000e-06),
        ("TP/R3/w", 210, 0, 0, 1.454000e-05),
        ("TP/R3crit/cp", 87, 0, 0, 9.830000e-01),
        ("TP/R3crit/cv", 87, 0, 0, 4.454000e-03),
        ("TP/R3crit/h", 87, 0, 0, 3.620000e-03),
        ("TP/R3crit/lambda", 87, 0, 0, 8.547000e-01),
        ("TP/R3crit/mu", 87, 0, 0, 1.029000e-02),
        ("TP/R3crit/s", 87, 0, 0, 2.650000e-03),
        ("TP/R3crit/u", 87, 0, 0, 3.265000e-03),
        ("TP/R3crit/v", 87, 0, 0, 1.393000e-02),
        ("TP/R3crit/w", 87, 0, 0, 2.392000e-03),
        ("TP/R5/cp", 705, 193, 377, 5.826000e-16),
        ("TP/R5/cv", 705, 194, 354, 8.734000e-16),
        ("TP/R5/h", 705, 425, 607, 3.317000e-16),
        ("TP/R5/lambda", 705, 413, 673, 6.604000e-16),
        ("TP/R5/mu", 705, 158, 520, 5.083000e-16),
        ("TP/R5/s", 705, 482, 655, 3.523000e-16),
        ("TP/R5/u", 705, 578, 623, 5.015000e-16),
        ("TP/R5/v", 705, 364, 684, 3.523000e-16),
        ("TP/R5/w", 705, 548, 702, 2.898000e-16),
        ("TP/T1073.15_other_side/cp", 47, 0, 0, 7.381000e-04),
        ("TP/T1073.15_other_side/h", 47, 0, 0, 2.288000e-05),
        ("TP/T1073.15_other_side/s", 47, 0, 0, 1.943000e-05),
        ("TP/T1073.15_other_side/v", 47, 0, 0, 1.239000e-04),
        ("TP/T623.15_other_side/cp", 17, 0, 0, 1.332000e-03),
        ("TP/T623.15_other_side/h", 17, 0, 0, 1.776000e-05),
        ("TP/T623.15_other_side/s", 17, 0, 0, 1.054000e-05),
        ("TP/T623.15_other_side/v", 17, 0, 0, 3.439000e-05),
        ("TP/region_agree", 3351, 3287, 0, 0.000000e+00),
    ];
    /// (note key, count)
    pub const NOTES: &[(&str, usize)] = &[
        ("HS/both_refuse", 391),
        ("HS/coolprop_ok_tampines_refuses", 2),
        ("HS/tampines_ok_coolprop_refuses", 1),
        ("PH/both_refuse", 38),
        ("PH/edge_point", 120),
        ("PH/edge_region_flip", 25),
        ("PH/region_mismatch", 6),
        ("PH/tampines_ok_coolprop_refuses", 158),
        ("PS/both_refuse", 92),
        ("PS/edge_point", 120),
        ("PS/edge_region_flip", 16),
        ("PS/region_mismatch", 6),
        ("PS/tampines_ok_coolprop_refuses", 122),
        ("RUS/PH_answers_for_R3", 10),
        ("RUS/PH_answers_for_R4", 38),
        ("RUS/PS_answers_for_R3", 4),
        ("RUS/PS_answers_for_R4", 53),
        ("RUS/tampines_ok_rusteam_refuses", 68),
        ("TP/both_refuse", 98),
        ("TP/boundary_coolprop_ok_tampines_refuses_psat", 6),
        ("TP/boundary_point_B23", 79),
        ("TP/boundary_point_psat", 138),
        ("TP/boundary_region_flip_B23", 24),
        ("TP/boundary_region_flip_psat", 25),
        ("TP/boundary_tampines_ok_coolprop_refuses_B23", 4),
        ("TP/boundary_tampines_ok_coolprop_refuses_psat", 15),
        ("TP/region_mismatch_T1073.15", 47),
        ("TP/region_mismatch_T623.15", 17),
    ];
    pub fn check(rep: &Report) {
        for (k, n, bit, ulp1, bound) in CELLS {
            let c = rep.cells.get(*k).unwrap_or_else(|| panic!("missing cell {}", k));
            assert_eq!(c.n, *n, "{} n", k);
            assert_eq!(c.bit, *bit, "{} bit-identical", k);
            assert_eq!(c.ulp1, *ulp1, "{} within 1ulp", k);
            assert!(c.max_rel <= *bound, "{} max_rel {:e} > pinned {:e} at {}", k, c.max_rel, bound, c.worst);
        }
        for (k, n) in NOTES {
            let got = rep.notes.get(*k).map(|x| x.0).unwrap_or(0);
            assert_eq!(got, *n, "note {}", k);
        }
        // every cell/note present must be pinned, so a new category cannot slip in unseen
        for k in rep.cells.keys() {
            assert!(CELLS.iter().any(|c| c.0 == k), "unpinned cell {}", k);
        }
        for k in rep.notes.keys() {
            assert!(NOTES.iter().any(|c| c.0 == k), "unpinned note {}", k);
        }
    }
}
