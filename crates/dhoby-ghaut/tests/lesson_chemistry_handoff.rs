//! Reaches every function of the chemistry and hand-off lessons' generator
//! (`examples/lesson_chemistry_handoff/chem.rs`) and checks each against
//! something it can fail on: boon-lay's own transcription checks (6.8877e-6
//! /s for air at 650 °C, 2.004984e-6 /s for steam at 1050 °C and 5 kPa,
//! f = 0.296875 for hydrolysis at 1000 °C and 1000 Pa), the validity flags at
//! the edges of each fitted range, the supply-limit crossover against a hand
//! solution of the Arrhenius law, the clamp pressure against a hand solution
//! of Eq. (5-2), and the recorded source term against the record it is
//! copied from. The generated page data are checked by
//! `docs/site/science/fission-products/check-chemhand.js`.
//!
//! Methodology: each assertion compares the crate's output through the
//! generator's helper with an independent value (a printed check, a closed
//! form solved by hand, or a recorded constant), to the tolerance stated at
//! the line. Results 2026-11-04: see the bottom of this doc, filled after the
//! run.
#![cfg(not(any(target_arch = "wasm32", target_os = "android")))]

#[path = "../examples/lesson_chemistry_handoff/chem.rs"]
mod chem;

use chem::*;

/// Air: transcription value, validity edges, supply crossover. The
/// crossover's hand solution: k(T) m / M_C = supply gives
/// 1/T = 1/923.15 - ln(k*/6.8877e-6) R / E_a with k* = 1 mol/s x 12.011e-3 /
/// 5000 kg x (21.27825 / p).
#[test]
fn air_law_flags_and_crossover() {
    let r = air_curve(21.27825, &[650.0, 590.0, 600.0, 690.0, 700.0]);
    assert!((r[0].0 / 6.8877e-6 - 1.0).abs() < 1e-4, "{}", r[0].0);
    assert_eq!(r.iter().map(|x| x.1).collect::<Vec<_>>(), vec![true, false, true, true, false]);
    // First order in O2: halving the pressure halves the rate.
    assert!((air_curve(10.639125, &[650.0])[0].0 / r[0].0 - 0.5).abs() < 1e-12);
    let hand = |p: f64| {
        let k = ILLUSTRATIVE_O2_SUPPLY_MOL_PER_S * 12.011e-3 / ILLUSTRATIVE_GRAPHITE_KG * 21.27825 / p;
        let inv_t = 1.0 / 923.15 - (k / 6.8877e-6).ln() * 8.314 / 191.0e3;
        1.0 / inv_t - 273.15
    };
    for p in [21.27825, 1.0] {
        let x = air_crossover_c(p, 300.0, 1600.0).expect("the flag changes");
        assert!((x - hand(p)).abs() < 0.1, "{p} kPa: {x} vs {}", hand(p));
    }
    assert_eq!(air_crossover_c(21.27825, 1000.0, 1600.0), None, "supply binds across the bracket");
    let (mol, supply) = air_gasified(900.0, 21.27825);
    assert!(supply && mol == ILLUSTRATIVE_O2_SUPPLY_MOL_PER_S);
    assert!(air_kinetic_mol(900.0, 21.27825) > 100.0 * mol);
    assert!(!air_gasified(500.0, 21.27825).1);
}

/// Steam: transcription value, hydrogen inhibition, the measured box.
#[test]
fn steam_law_and_box() {
    let g = steam_curve(5.0, 0.0, &[1050.0, 840.0, 850.0, 1100.0, 1110.0]);
    assert!((g[0].0 / 2.004984e-6 - 1.0).abs() < 1e-4, "{}", g[0].0);
    assert_eq!(g.iter().map(|x| x.1).collect::<Vec<_>>(), vec![true, false, true, true, false]);
    assert!(steam_curve(5.0, 1.0, &[1050.0])[0].0 < g[0].0);
    assert!(!steam_curve(300.0, 0.0, &[1050.0])[0].1 && !steam_curve(0.1, 0.0, &[1050.0])[0].1);
    assert_eq!(steam_curve(0.0, 0.0, &[1050.0])[0].0, 0.0);
}

/// Hydrolysis: transcription value, flags, and the clamp pressure against
/// Eq. (5-2) solved for f = 1 by hand:
/// ln P = (ln 1 - ln 2.13e13 + 47257/T) / (-4.353 + 6503/T).
#[test]
fn hydrolysis_flags_and_clamp_pressure() {
    let h = hydrolysis_curve(1.0, &[1000.0, 1100.0, 600.0]);
    assert!((h[0].0 / 0.296875 - 1.0).abs() < 1e-4, "{}", h[0].0);
    assert_eq!(h[0].1, 0);
    assert_eq!(h[1].1, 1, "1100 degC is outside the fit");
    assert_eq!(hydrolysis_curve(300.0, &[1000.0])[0], (1.0, 2));
    for t_c in [770.0, 1000.0] {
        let t = t_c + 273.15;
        let hand_kpa = ((-(2.13e13f64).ln() + 47257.0 / t) / (-4.353 + 6503.0 / t)).exp() / 1e3;
        let got = hydrolysis_whole_inventory_kpa(t_c).expect("clamps inside the bracket");
        assert!((got / hand_kpa - 1.0).abs() < 1e-4, "{t_c}: {got} vs {hand_kpa}");
    }
    // Above 1494 K the pressure exponent is negative (more steam, less
    // release), so at 1500 degC the fit is clamped already at the bracket's
    // lowest pressure (1 Pa: f = 2.13e13 exp(-47257/1773.15) = 57) and the
    // search reports None.
    assert_eq!(hydrolysis_whole_inventory_kpa(1500.0), None);
}

/// The source term is the dispersion demo's record, unchanged.
#[test]
fn the_source_term_is_the_record() {
    let r = released();
    assert_eq!(r.len(), recorded::RELEASE.len());
    for (a, b) in r.iter().zip(recorded::RELEASE.iter()) {
        assert_eq!((a.name, a.released_bq, a.inventory_bq), (b.0, b.1, b.2));
        assert_eq!(a.fraction, b.1 / b.2);
    }
    let sum: f64 = r.iter().map(|x| x.released_bq).sum();
    assert!(sum < recorded::RELEASE_TOTAL_BQ);
    assert!((temp(0.0).get::<uom::si::thermodynamic_temperature::kelvin>() - 273.15).abs() < 1e-12);
    assert_eq!(kpa(2.0).get::<uom::si::pressure::pascal>(), 2000.0);
}
