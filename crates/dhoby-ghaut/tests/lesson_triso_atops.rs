//! Reaches every function of chapter 5's generator
//! (`examples/lesson_triso_atops/atops.rs`) and checks each against
//! something it can fail on: the Case A constants, the groups, a clamp read
//! from the output, the Booth floor of gh:#385, the release model per group,
//! linearity in the failure fractions, the live pools against the closed
//! form, and the venting rule. The generated page data are checked by
//! `docs/site/science/fission-products/check-fission.js` (checks 45-52).
//!
//! Results 2026-10-22: all 5 pass (`cargo test --release -p dhoby-ghaut
//! --test lesson_triso_atops`).
#![cfg(not(any(target_arch = "wasm32", target_os = "android")))]

#[path = "../examples/lesson_triso_atops/atops.rs"]
mod atops;

use atops::*;
use uom::si::frequency::hertz;
use uom::si::time::second;

#[test]
fn constants_groups_and_clamps() {
    let p = case_a();
    assert_eq!(p.irradiation_time.get::<second>(), 3.0 * 365.0 * 86400.0);
    assert!((years(1.0).get::<second>() - 31_536_000.0).abs() < 1e-6);
    assert!((a_booth() - (2.0 * 1e-5 * 2.13e-4f64).sqrt()).abs() < 1e-18);
    let n = nuclides();
    assert_eq!(n.len(), 84);
    let cs = n.iter().find(|x| x.0.name == "Cs-137").unwrap();
    assert_eq!(group_name(cs.0.element_group()), "SpecialMetal");
    assert!(!cs.1, "Cs-137 is long-lived against 3 years");
    assert!(n.iter().find(|x| x.0.name == "Kr-88").unwrap().1);
    let grid: Vec<f64> = (0..=40).map(|i| 300.0 + 25.0 * i as f64).collect();
    assert_eq!(flat_below(&grid, |t| d(55, t).0), Some(700.0));
    assert_eq!(flat_below(&grid, |t| d(36, t).0), None);
    assert!(d_sic_ag(1200.0) > d_sic_ag(1000.0));
    assert_eq!(nuclide("Ag-110m").z, 47);
}

#[test]
fn release_models() {
    // gh:#385: the 5000-term series has a floor near 1.216e-4.
    assert!((booth_long(1e-16) / 1.215976e-4 - 1.0).abs() < 1e-3);
    assert!((booth_trans(1e-16) / 1.215976e-4 - 1.0).abs() < 1e-3);
    assert!(booth_long(10.0) > 0.999999 && booth_terms() == 5000);
    let n = nuclides();
    let zr = n.iter().find(|x| x.0.name == "Zr-95").unwrap();
    assert_eq!(rb(&zr.0, zr.1, 1000.0), 1e-5, "other: nominal");
    let kr = n.iter().find(|x| x.0.name == "Kr-88").unwrap();
    assert!(rb(&kr.0, kr.1, 1200.0) > rb(&kr.0, kr.1, 900.0));
    assert!(breakthrough(1600.0) > breakthrough(1000.0));
    let t = years(3.0);
    assert!(attenuation(55, 700.0, t) > attenuation(55, 1200.0, t) && attenuation(55, 1200.0, t) >= 1.0);
    // "Other" metals get a fixed 1e8 in base_activities, not from attenuation_factor.
    assert_eq!(boon_lay::triso_atops_fork::activities::source_terms::OTHER_ATTENUATION_FACTOR, 1e8);
}

#[test]
fn normal_operation_is_linear_in_the_fractions() {
    let n = nuclides();
    let (cs, sl) = n.iter().find(|x| x.0.name == "Cs-137").unwrap().clone();
    let f = fractions_a();
    let mut g = f;
    g.heavy_metal *= 2.0;
    g.sic *= 2.0;
    g.incremental *= 2.0;
    g.incremental_sic *= 2.0;
    let a = node(&cs, sl, 900.0, f, false);
    let b = node(&cs, sl, 900.0, g, false);
    assert!((b.release_rate / a.release_rate - 2.0).abs() < 1e-12);
    assert_eq!(node(&cs, sl, 900.0, f, true).clean_up_activity, 0.0, "no clean-up of metals");
    // The live pools (not a port) reproduce the circulating pool from empty.
    let lam = cs.decay_constant().get::<hertz>();
    let (c, _, _, leaked) = live(a.source_rate, lam, case_a().k_plate.get::<hertz>(), 0.0, 0.0, case_a().run_time.get::<second>(), 50);
    assert!((c - a.circulating_activity).abs() <= 1e-9 * a.circulating_activity && leaked == 0.0);
    let acc = accident_fractions_a();
    assert!(acc.total() > f.sum());
    assert!(released_atoms(&cs, &a, 0.5) > 0.0);
}

#[test]
fn venting_only_while_heating() {
    let (h, t) = heatup(900.0, 1600.0);
    assert_eq!(h.len(), 61);
    assert_eq!(t[20], 1600.0);
    let cs = nuclide("Cs-137");
    let a = accident(&cs, &h, &t);
    assert!(a.vent[..=20].iter().all(|v| v.is_some()));
    assert!(a.vent[21..].iter().all(|v| v.is_none()));
    assert!(a.rf_kernel[20] > 0.99 && a.rf_graphite[20] > 0.0);
    assert_eq!(a.hours.len(), a.t_c.len());
    let kr = nuclide("Kr-85");
    assert!(accident(&kr, &h, &t).rf_graphite.iter().all(|&g| g == 0.0), "gases are not held in graphite");
}

#[test]
fn small_helpers() {
    assert!((temp(0.0).get::<uom::si::thermodynamic_temperature::kelvin>() - 273.15).abs() < 1e-12);
    assert_eq!(m(2.0).get::<uom::si::length::meter>(), 2.0);
}
