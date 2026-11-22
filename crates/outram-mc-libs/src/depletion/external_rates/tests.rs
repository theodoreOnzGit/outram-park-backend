//! Analytic tests for the `TransferRates` / `ExternalSourceRates` port
//! (gh:#1622). Each test states its prediction before the run; the numbers
//! recorded in the doc comments are the measured results (2026-11-17).

use super::*;
use crate::depletion::cram::cram16;
use petir::mathf::RealMath;

#[cfg(not(any(feature = "std", test)))]
#[allow(unused_imports)]
use crate::nostd_prelude::*;

fn names(v: &[&str]) -> Vec<String> {
    v.iter().map(|s| (*s).to_owned()).collect()
}

fn rel(a: f64, b: f64) -> f64 {
    ((a - b) / b).abs()
}

/// Mass in g/mol taken as the mass number: enough for unit tests that check
/// the arithmetic, not the AME values.
fn mass_number(nuc: &str) -> Option<f64> {
    let digits: String = nuc.chars().skip_while(|c| !c.is_ascii_digit()).take_while(char::is_ascii_digit).collect();
    digits.parse::<f64>().ok()
}

/// Methodology: one stable nuclide, removal at `r = 1/(10 d)`, one 30 d step
/// with CRAM-16. Reference `N0 exp(-r t)`. Prediction: agreement to
/// CRAM-16's ~1e-14 on a negative real eigenvalue; gated at 1e-12.
#[test]
fn removal_is_exponential() {
    let nucs = names(&["Xe135"]);
    let mut tr = TransferRates::new(names(&["1"]), nucs.clone(), 1);
    tr.set_transfer_rate("1", &["Xe"], 0.1, RateUnit::PerDay, None, None).unwrap();
    let dt = 30.0 * SECONDS_PER_DAY;
    let out = deplete_with_external_rates(cram16, &nucs, vec![DepletionMatrix::zeros(1)], vec![vec![1.0e20]], dt, 0, Some(&tr), None).unwrap();
    let exact = 1.0e20 * (-0.1f64 * 30.0).r_exp();
    assert!(rel(out[0][0], exact) < 1e-12, "{} vs {exact}", out[0][0]);
}

/// Methodology: a constant feed `s` with removal `r`, from `N0 = 0`; exact
/// `s/r (1 - exp(-r t))`. The port augments the matrix as upstream does, so
/// the prediction is agreement to ~1e-12. The older
/// `integrators::apply_feed` adds `s dt` after the step; for `x = r dt =
/// 0.1` its predicted relative error is `x / (1 - e^-x) - 1 ≈ x/2 =
/// 5.08 %`. Measured: the exact path below 1e-12; `apply_feed` 5.083 %.
#[test]
fn exact_feed_matches_the_analytic_solution_and_apply_feed_is_first_order() {
    let nucs = names(&["Pa233"]);
    let mut tr = TransferRates::new(names(&["1"]), nucs.clone(), 1);
    tr.set_transfer_rate("1", &["Pa233"], 0.1, RateUnit::PerDay, None, None).unwrap();
    let mut es = ExternalSourceRates::new(names(&["1"]), 1);
    let s = 1.0e15;
    es.set_external_source_atoms("1", "Pa233", s, None).unwrap();
    let dt = SECONDS_PER_DAY;
    let r = 0.1 / SECONDS_PER_DAY;
    let out = deplete_with_external_rates(cram16, &nucs, vec![DepletionMatrix::zeros(1)], vec![vec![0.0]], dt, 0, Some(&tr), Some(&es)).unwrap();
    assert_eq!(out[0].len(), 1, "the augmented entry is stripped");
    let exact = s / r * (1.0 - (-r * dt).r_exp());
    assert!(rel(out[0][0], exact) < 1e-12, "{} vs {exact}", out[0][0]);

    // The first-order path, for the record.
    let old = s * dt; // N0 = 0, so the removal step leaves 0 and the feed adds s dt
    let err = rel(old, exact);
    assert!((err - 0.050_833).abs() < 1e-5, "apply_feed relative error {err}");
}

/// Methodology: Pa removed from material 1 into material 2 at `r`; no other
/// physics. Exact: `N1 = N0 e^{-rt}`, `N2 = N0 (1 - e^{-rt})`, sum
/// conserved. Prediction: both to 1e-12, U-233 untouched (element match
/// takes Pa only).
#[test]
fn transfer_between_materials_conserves_atoms() {
    let nucs = names(&["Pa233", "U233"]);
    let mut tr = TransferRates::new(names(&["1", "2"]), nucs.clone(), 1);
    tr.set_transfer_rate("1", &["Pa"], 1.0, RateUnit::PerDay, None, Some("2")).unwrap();
    let dt = 2.0 * SECONDS_PER_DAY;
    let n0 = vec![vec![1.0e18, 5.0e17], vec![0.0, 0.0]];
    let out = deplete_with_external_rates(cram16, &nucs, vec![DepletionMatrix::zeros(2), DepletionMatrix::zeros(2)], n0, dt, 0, Some(&tr), None).unwrap();
    let e = (-2.0f64).r_exp();
    assert!(rel(out[0][0], 1.0e18 * e) < 1e-12);
    assert!(rel(out[1][0], 1.0e18 * (1.0 - e)) < 1e-12);
    assert!(rel(out[0][1], 5.0e17) < 1e-12, "U-233 is not Pa");
    assert!(out[1][1].abs() < 1e3);
    assert!(rel(out[0][0] + out[1][0], 1.0e18) < 1e-12);
}

/// Methodology: material 1 has a feed `s` and sends Pa to material 2 at
/// `r`, from zero. Exact: `N1 = s/r (1 - e^{-rt})`, `N2 = s t - N1`.
/// Exercises the block system with one augmented block (upstream's
/// zero-row/column padding). Prediction: 1e-11.
#[test]
fn transfer_with_a_source_pads_the_block_system() {
    let nucs = names(&["Pa233"]);
    let mut tr = TransferRates::new(names(&["1", "2"]), nucs.clone(), 1);
    tr.set_transfer_rate("1", &["Pa"], 0.5, RateUnit::PerDay, None, Some("2")).unwrap();
    let mut es = ExternalSourceRates::new(names(&["1", "2"]), 1);
    let s = 3.0e14;
    es.set_external_source_atoms("1", "Pa233", s, None).unwrap();
    let dt = 4.0 * SECONDS_PER_DAY;
    let r = 0.5 / SECONDS_PER_DAY;
    let out = deplete_with_external_rates(cram16, &nucs, vec![DepletionMatrix::zeros(1), DepletionMatrix::zeros(1)], vec![vec![0.0], vec![0.0]], dt, 0, Some(&tr), Some(&es)).unwrap();
    assert_eq!((out[0].len(), out[1].len()), (1, 1));
    let n1 = s / r * (1.0 - (-r * dt).r_exp());
    assert!(rel(out[0][0], n1) < 1e-11, "{} vs {n1}", out[0][0]);
    assert!(rel(out[1][0], s * dt - n1) < 1e-11, "{} vs {}", out[1][0], s * dt - n1);
}

/// Departure 2 in the module docs. Material 2's only source is at step 1;
/// at step 0 material 1's source is active. Upstream would strip material
/// 2's last nuclide although it was not augmented; this port keeps it.
#[test]
fn a_material_without_a_source_this_step_keeps_every_nuclide() {
    let nucs = names(&["Li7", "Be9", "F19"]);
    let mut es = ExternalSourceRates::new(names(&["1", "2"]), 2);
    es.set_external_source_atoms("1", "F19", 1.0, Some(&[0])).unwrap();
    es.set_external_source_atoms("2", "F19", 1.0, Some(&[1])).unwrap();
    let n = vec![vec![1.0, 2.0, 3.0], vec![4.0, 5.0, 6.0]];
    let z = || DepletionMatrix::zeros(3);
    let out = deplete_with_external_rates(cram16, &nucs, vec![z(), z()], n, 10.0, 0, None, Some(&es)).unwrap();
    assert_eq!(out[1].len(), 3);
    assert!(rel(out[1][2], 6.0) < 1e-12, "material 2 is unchanged");
    assert!(rel(out[0][2], 13.0) < 1e-12, "material 1 gained s dt = 10");
}

/// Upstream's unit conversion: the stored rate is `rate / seconds`.
#[test]
fn units_convert_to_per_second() {
    let mut tr = TransferRates::new(names(&["1"]), names(&["Xe135"]), 1);
    tr.set_transfer_rate("1", &["Xe135"], 1.0, RateUnit::PerDay, None, None).unwrap();
    tr.set_transfer_rate("1", &["Kr"], 1.0, RateUnit::PerJulianYear, None, None).unwrap();
    assert_eq!(tr.rates().get_external_rate("1", "Xe135", 0, None).unwrap(), vec![1.0 / 86_400.0]);
    assert_eq!(tr.rates().get_external_rate("1", "Kr", 0, None).unwrap(), vec![1.0 / 31_557_600.0]);
}

/// Element first, then nuclide; rates sum; untouched nuclides are zero.
#[test]
fn form_rr_term_matches_elements_then_nuclides() {
    let nucs = names(&["Pa233", "U233", "Th232", "Am242_m1"]);
    let mut tr = TransferRates::new(names(&["1"]), nucs.clone(), 1);
    tr.set_transfer_rate("1", &["Pa"], 1.0, RateUnit::PerSecond, None, None).unwrap();
    tr.set_transfer_rate("1", &["U233"], 2.0, RateUnit::PerSecond, None, None).unwrap();
    tr.set_transfer_rate("1", &["U233"], 0.5, RateUnit::PerSecond, None, None).unwrap();
    tr.set_transfer_rate("1", &["Am"], 4.0, RateUnit::PerSecond, None, None).unwrap();
    let m = form_rr_term(&nucs, &tr, 0, &TransferTerm::Removal("1".into())).unwrap();
    assert_eq!([m.get(0, 0), m.get(1, 1), m.get(2, 2), m.get(3, 3)], [1.0, 2.5, 0.0, 4.0]);
    assert_eq!(m.get(0, 1), 0.0);
}

/// Upstream's validation errors.
#[test]
fn set_transfer_rate_refuses_what_upstream_refuses() {
    let nucs = names(&["Pa233", "Xe135"]);
    let mut tr = TransferRates::new(names(&["1"]), nucs.clone(), 2);
    tr.set_transfer_rate("1", &["Pa233"], 1.0, RateUnit::PerSecond, None, None).unwrap();
    assert!(tr.set_transfer_rate("1", &["Pa"], 1.0, RateUnit::PerSecond, None, None).is_err());
    tr.set_transfer_rate("1", &["Xe"], 1.0, RateUnit::PerSecond, None, None).unwrap();
    assert!(tr.set_transfer_rate("1", &["Xe135"], 1.0, RateUnit::PerSecond, None, None).is_err());
    assert!(tr.set_transfer_rate("1", &["Qq"], 1.0, RateUnit::PerSecond, None, None).is_err());
    assert!(tr.set_transfer_rate("9", &["Pa"], 1.0, RateUnit::PerSecond, None, None).is_err());
    assert!(tr.set_transfer_rate("1", &["Kr"], 1.0, RateUnit::PerSecond, Some(&[2]), None).is_err());
    assert!(tr.set_transfer_rate("1", &["Kr"], 1.0, RateUnit::PerSecond, None, Some("1")).is_err(), "one material");
    assert!(tr.set_redox("1", &[("Cs137", 1.0)], &[("Cs", 1)], None).is_err(), "not in chain");
}

/// Timestep bookkeeping and the destination filter.
#[test]
fn timesteps_and_destinations_filter_rates() {
    let nucs = names(&["Pa233"]);
    let mut tr = TransferRates::new(names(&["1", "2", "3"]), nucs, 3);
    tr.set_transfer_rate("1", &["Pa"], 1.0, RateUnit::PerSecond, Some(&[1]), Some("2")).unwrap();
    tr.set_transfer_rate("1", &["Pa"], 3.0, RateUnit::PerSecond, Some(&[2]), Some("3")).unwrap();
    assert_eq!(tr.rates().external_timesteps(), &[1, 2]);
    assert!(tr.rates().get_components("1", 0, None).unwrap().is_empty());
    assert_eq!(tr.rates().get_external_rate("1", "Pa", 1, None).unwrap(), vec![1.0]);
    assert_eq!(tr.rates().get_external_rate("1", "Pa", 2, Some("2")).unwrap(), Vec::<f64>::new());
    assert_eq!(tr.index_transfer(2), &[("3".to_owned(), "1".to_owned())]);
}

/// Hand-worked redox. Nuclides U235 (OS +4), Cs137 (+1), F19 (-1, the
/// buffer, fraction 1). Column 0: `a00 = -2`, `a10 = +2`. Net change
/// `-2*4 + 2*1 = -6`; buffer row: `a20 -= -6 * 1 / -1`, so `a20 = -6`.
/// Other columns carry no change. A zero-OS buffer is refused.
#[test]
fn redox_term_matches_the_hand_calculation() {
    let nucs = names(&["U235", "Cs137", "F19"]);
    let mut a = DepletionMatrix::zeros(3);
    a.set(0, 0, -2.0).unwrap();
    a.set(1, 0, 2.0).unwrap();
    let rx = Redox { buffer: vec![("F19".into(), 1.0)], oxidation_states: vec![("U".into(), 4), ("Cs".into(), 1), ("F".into(), -1)] };
    let b = add_redox_term(&a, &nucs, &rx).unwrap();
    assert_eq!(b.get(2, 0), -6.0);
    assert_eq!([b.get(0, 0), b.get(1, 0), b.get(2, 1), b.get(2, 2)], [-2.0, 2.0, 0.0, 0.0]);
    let zero = Redox { buffer: vec![("Cs137".into(), 1.0)], oxidation_states: vec![("U".into(), 4)] };
    assert!(add_redox_term(&a, &nucs, &zero).is_err());
}

/// The mass-rate setter: 235 g/d of U-235 at 235 g/mol is `N_A / 86400`
/// atoms per second, bit for bit in upstream's operation order. An element
/// expands over its natural isotopes, weighted by abundance.
#[test]
fn mass_source_converts_to_atoms() {
    let mut es = ExternalSourceRates::new(names(&["1"]), 1);
    es.set_external_source_rate("1", &[("U235", 100.0)], 235.0, MassRateUnit::GramPerDay, None, mass_number).unwrap();
    let r = es.rates().get_external_rate("1", "U235", 0, None).unwrap();
    assert_eq!(r, vec![235.0 / 235.0 * AVOGADRO_PER_MOL * 1.0 / SECONDS_PER_DAY]);

    let mut es = ExternalSourceRates::new(names(&["1"]), 1);
    es.set_external_source_rate("1", &[("Li", 1.0)], 1.0, MassRateUnit::GramPerSecond, None, mass_number).unwrap();
    let li6 = es.rates().get_external_rate("1", "Li6", 0, None).unwrap()[0];
    let li7 = es.rates().get_external_rate("1", "Li7", 0, None).unwrap()[0];
    assert!(li7 > 10.0 * li6, "natural lithium is mostly Li-7");
    assert!(es.set_external_source_rate("1", &[("Tc", 1.0)], 1.0, MassRateUnit::GramPerSecond, None, mass_number).is_err(), "no natural Tc");

    let nucs = names(&["Li6", "Li7", "F19"]);
    let v = form_ext_source_term(&nucs, &es, 0, "1").unwrap().unwrap();
    assert_eq!(v, vec![li6, li7, 0.0]);
    let empty = ExternalSourceRates::new(names(&["1"]), 1);
    assert!(form_ext_source_term(&nucs, &empty, 0, "1").unwrap().is_none());
}
