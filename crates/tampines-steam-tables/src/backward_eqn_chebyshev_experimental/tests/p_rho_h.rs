//! Verification of the experimental `p(rho,h)` correlations and of the
//! statistical `(rho,h)` region classifier.
//!
//! # How the test states are generated
//!
//! States are generated **forwards** so every reference value is
//! IAPWS-traceable. For a `(p, T)` node the crate's own dispatcher
//! [`region_fwd_eqn_single_phase`] labels the region, and its forward flashes
//! supply specific volume and enthalpy; density is the reciprocal of specific
//! volume. The correlation must then recover the originating pressure from
//! `(rho, h)` alone.
//!
//! Only single-phase regions are swept here. Region 4 states are not
//! reachable through the single-phase `(T,p)` flash — a `(T,p)` pair on the
//! saturation line is underdetermined without a quality — so the Region 4
//! `p(rho,h)` surfaces are not covered by these tests. That gap is stated in
//! the module documentation rather than papered over.

use uom::si::{
    available_energy::kilojoule_per_kilogram, f64::*, mass_density::kilogram_per_cubic_meter,
    pressure::megapascal, specific_volume::cubic_meter_per_kilogram,
    thermodynamic_temperature::kelvin,
};

use crate::backward_eqn_chebyshev_experimental::{
    p_rho_h_in_region, rho_h_region_candidate, rho_h_region_scores, RhoHRegion,
};
use crate::interfaces::functional_programming::pt_flash_eqm::{
    h_tp_eqm_single_phase, region_fwd_eqn_single_phase, v_tp_eqm_single_phase, FwdEqnRegion,
};

/// One generated single-phase state, with the correlation's error on it.
struct SamplePoint {
    region: RhoHRegion,
    t_kelvin: f64,
    rho_kg_m3: f64,
    h_kj_kg: f64,
    p_reference_mpa: f64,
    p_fitted_mpa: f64,
}

impl SamplePoint {
    /// Relative deviation of the fitted pressure from the IAPWS reference.
    fn relative_error(&self) -> f64 {
        ((self.p_fitted_mpa - self.p_reference_mpa) / self.p_reference_mpa).abs()
    }
}

/// Translates the crate's forward-equation region label into this module's.
fn to_rho_h_region(region: FwdEqnRegion) -> Option<RhoHRegion> {
    match region {
        FwdEqnRegion::Region1 => Some(RhoHRegion::Region1),
        FwdEqnRegion::Region2 => Some(RhoHRegion::Region2),
        FwdEqnRegion::Region3 => Some(RhoHRegion::Region3),
        FwdEqnRegion::Region5 => Some(RhoHRegion::Region5),
        // two-phase: not reachable through the single-phase (T,p) flash
        FwdEqnRegion::Region4 => None,
    }
}

/// Generates single-phase states across the `(p, T)` envelope and evaluates
/// the correlation on each, using the region the crate's own dispatcher
/// assigns.
///
/// The sweep is deterministic, so the reported statistics are reproducible.
fn generate_sample_points(n_p: usize, n_t: usize) -> Vec<SamplePoint> {
    // pressure log-spaced over the IF97 envelope, temperature linear
    const P_MIN_MPA: f64 = 1.0e-3;
    const P_MAX_MPA: f64 = 50.0;
    const T_MIN_K: f64 = 280.0;
    const T_MAX_K: f64 = 2200.0;

    let log_p_min = P_MIN_MPA.log10();
    let log_p_max = P_MAX_MPA.log10();

    let mut out = Vec::new();

    for i in 0..n_p {
        let frac_p = i as f64 / (n_p - 1) as f64;
        let p_mpa = 10.0_f64.powf(log_p_min + frac_p * (log_p_max - log_p_min));
        let p = Pressure::new::<megapascal>(p_mpa);

        for j in 0..n_t {
            let frac_t = j as f64 / (n_t - 1) as f64;
            let t_kelvin = T_MIN_K + frac_t * (T_MAX_K - T_MIN_K);
            let t = ThermodynamicTemperature::new::<kelvin>(t_kelvin);

            let Some(region) = to_rho_h_region(region_fwd_eqn_single_phase(t, p)) else {
                continue;
            };

            let v = v_tp_eqm_single_phase(t, p).get::<cubic_meter_per_kilogram>();
            if !v.is_finite() || v <= 0.0 {
                continue;
            }
            let rho_kg_m3 = 1.0 / v;

            let h_kj_kg = h_tp_eqm_single_phase(t, p).get::<kilojoule_per_kilogram>();
            if !h_kj_kg.is_finite() {
                continue;
            }

            let p_fitted_mpa = p_rho_h_in_region(
                region,
                MassDensity::new::<kilogram_per_cubic_meter>(rho_kg_m3),
                AvailableEnergy::new::<kilojoule_per_kilogram>(h_kj_kg),
            )
            .get::<megapascal>();

            out.push(SamplePoint {
                region,
                t_kelvin,
                rho_kg_m3,
                h_kj_kg,
                p_reference_mpa: p_mpa,
                p_fitted_mpa,
            });
        }
    }

    out
}

/// Prints the measured `p(rho,h)` error distribution, overall and per region.
///
/// # Methodology
///
/// Diagnostic only — asserts nothing beyond having generated usable states.
/// Sweeps a 60 x 60 `(p, T)` grid (pressure log-spaced 1e-3 to 50 MPa,
/// temperature linear 280 to 2200 K), keeps the single-phase points, and
/// reports the relative deviation of the fitted pressure from the originating
/// pressure.
///
/// Run with `--nocapture` to see the numbers.
#[test]
fn diagnose_p_rho_h_error() {
    let points = generate_sample_points(60, 60);
    assert!(
        !points.is_empty(),
        "no states generated — the sweep is wrong"
    );

    let report = |label: &str, mut errors: Vec<f64>| {
        if errors.is_empty() {
            eprintln!("  {label:>10}: no points");
            return;
        }
        errors.sort_by(|a, b| a.partial_cmp(b).unwrap());
        let pick = |q: f64| errors[((errors.len() - 1) as f64 * q) as usize];
        eprintln!(
            "  {label:>10}: n = {:5}, p50 = {:.3e}, p90 = {:.3e}, p99 = {:.3e}, max = {:.3e}",
            errors.len(),
            pick(0.50),
            pick(0.90),
            pick(0.99),
            errors[errors.len() - 1]
        );
    };

    eprintln!("p(rho,h) relative pressure error, {} states", points.len());
    report(
        "overall",
        points.iter().map(SamplePoint::relative_error).collect(),
    );
    for region in [
        RhoHRegion::Region1,
        RhoHRegion::Region2,
        RhoHRegion::Region3,
        RhoHRegion::Region5,
    ] {
        report(
            &format!("{region:?}"),
            points
                .iter()
                .filter(|point| point.region == region)
                .map(SamplePoint::relative_error)
                .collect(),
        );
    }

    // worst offender, to make a regression diagnosable
    if let Some(worst) = points
        .iter()
        .max_by(|a, b| a.relative_error().partial_cmp(&b.relative_error()).unwrap())
    {
        eprintln!(
            "  worst: {:?} at T = {:.2} K, rho = {:.4} kg/m3, h = {:.2} kJ/kg, \
             p_ref = {:.5} MPa, p_fit = {:.5} MPa",
            worst.region,
            worst.t_kelvin,
            worst.rho_kg_m3,
            worst.h_kj_kg,
            worst.p_reference_mpa,
            worst.p_fitted_mpa
        );
    }
}

/// Verifies the `p(rho,h)` correlations reproduce the originating pressure for
/// single-phase states, given the correct region.
///
/// # Methodology
///
/// As the diagnostic above: a 60 x 60 `(p, T)` sweep, single-phase points
/// only, with the region taken from the crate's own forward dispatcher so that
/// the statistical classifier is not in the loop. The reference pressure is
/// the pressure the state was generated at.
///
/// Region 4 is not covered — see the module documentation.
///
/// Envelopes are asserted **per region**, not as one aggregate, because the
/// regions behave very differently: Regions 2, 3 and 5 recover pressure
/// tightly, while Region 1 is intrinsically ill-conditioned (see
/// [`p_rho_h_region_1_is_ill_conditioned_as_documented`]). An aggregate
/// percentile would hide that.
///
/// # Results
///
/// Measured 2026-08-31 over 3600 single-phase states, maximum relative error
/// in recovered pressure: Region 2 1.55e-4 (n = 1185), Region 3 3.69e-4
/// (n = 16), Region 5 1.31e-3 (n = 2100). Run `diagnose_p_rho_h_error
/// --nocapture` to reproduce the full distribution.
#[test]
fn p_rho_h_round_trips_forward_equations_given_region() {
    let points = generate_sample_points(60, 60);
    assert!(
        !points.is_empty(),
        "no states generated — the sweep is wrong"
    );

    // Region 1 is excluded here and covered by its own test, which documents
    // why it cannot meet these envelopes.
    for (region, tolerance) in [
        (RhoHRegion::Region2, 5.0e-4),
        (RhoHRegion::Region3, 1.0e-3),
        (RhoHRegion::Region5, 5.0e-3),
    ] {
        let in_region: Vec<&SamplePoint> = points
            .iter()
            .filter(|point| point.region == region)
            .collect();
        assert!(
            !in_region.is_empty(),
            "the sweep produced no {region:?} states, so its envelope is untested"
        );

        let worst = in_region
            .iter()
            .max_by(|a, b| a.relative_error().partial_cmp(&b.relative_error()).unwrap())
            .expect("checked non-empty above");

        assert!(
            worst.relative_error() < tolerance,
            "p(rho,h) max relative error in {region:?} was {:.6e} over {} states, \
             exceeding the recorded envelope of {tolerance:e}; worst state was \
             T = {:.2} K, rho = {:.4} kg/m3, h = {:.2} kJ/kg, p_ref = {:.5} MPa, \
             p_fit = {:.5} MPa",
            worst.relative_error(),
            in_region.len(),
            worst.t_kelvin,
            worst.rho_kg_m3,
            worst.h_kj_kg,
            worst.p_reference_mpa,
            worst.p_fitted_mpa,
        );
    }
}

/// Records that `p(rho,h)` is intrinsically ill-conditioned in Region 1, and
/// pins how bad it is so a regression is still visible.
///
/// # Methodology
///
/// Liquid water is very nearly incompressible: along an isotherm in Region 1,
/// density changes only marginally as pressure changes by orders of magnitude.
/// Recovering pressure *from* density is therefore ill-conditioned there — a
/// small error in `rho` maps to a large error in `p` — and this is a property
/// of the state variables, not a defect in the fit. No `(rho,h)` correlation,
/// however well fitted, can avoid it.
///
/// This test asserts the median stays reasonable while explicitly allowing the
/// large tail, so that the limitation is recorded in the suite rather than
/// hidden by an aggregate statistic.
///
/// # Results
///
/// Measured 2026-08-31 over 299 Region 1 states: p50 = 5.62e-4,
/// p90 = 7.58e-2, p99 = 1.84, max = 3.19 (i.e. the worst state recovers a
/// pressure a factor of ~4 out). The worst case sits at the low-pressure,
/// cold corner — T = 280 K, rho = 999.86 kg/m3, p_ref = 1.0e-3 MPa — exactly
/// where the isotherms are most tightly packed in density.
///
/// **Consequence for callers:** do not use `p(rho,h)` to recover pressure in
/// subcooled liquid. Use an equation of state parameterised the other way, or
/// carry pressure as a state variable there.
#[test]
fn p_rho_h_region_1_is_ill_conditioned_as_documented() {
    let points = generate_sample_points(60, 60);
    let mut errors: Vec<f64> = points
        .iter()
        .filter(|point| point.region == RhoHRegion::Region1)
        .map(SamplePoint::relative_error)
        .collect();
    assert!(
        !errors.is_empty(),
        "the sweep produced no Region 1 states, so this limitation is untested"
    );
    errors.sort_by(|a, b| a.partial_cmp(b).unwrap());

    let p50 = errors[(errors.len() - 1) / 2];
    assert!(
        p50 < P_RHO_H_REGION_1_P50_REL_TOLERANCE,
        "p(rho,h) median relative error in Region 1 was {p50:.6e} over {} states, \
         exceeding the recorded envelope of {P_RHO_H_REGION_1_P50_REL_TOLERANCE:e}",
        errors.len()
    );
}

/// Measures the statistical region classifier against the crate's own
/// forward-equation region labels.
///
/// # Methodology
///
/// Uses the same single-phase sweep, treating `region_fwd_eqn_single_phase` as
/// ground truth, and reports the per-region and overall agreement rate of
/// [`rho_h_region_candidate`]. This is the check on the classifier's claimed
/// ~97.3% overall accuracy.
///
/// Region 4 is absent from this sweep, so the classifier's weakest case
/// (~91.6% self-reported) is **not** exercised here.
///
/// # Results
///
/// Measured 2026-08-31 — see the asserted floor below and run with
/// `--nocapture` for the per-region breakdown.
#[test]
fn rho_h_region_classifier_agrees_with_forward_dispatcher() {
    let points = generate_sample_points(60, 60);
    assert!(
        !points.is_empty(),
        "no states generated — the sweep is wrong"
    );

    let mut total = 0_usize;
    let mut correct = 0_usize;

    for region in [
        RhoHRegion::Region1,
        RhoHRegion::Region2,
        RhoHRegion::Region3,
        RhoHRegion::Region5,
    ] {
        let in_region: Vec<&SamplePoint> = points.iter().filter(|p| p.region == region).collect();
        if in_region.is_empty() {
            continue;
        }
        let hits = in_region
            .iter()
            .filter(|point| rho_h_region_candidate(point.rho_kg_m3, point.h_kj_kg) == point.region)
            .count();

        eprintln!(
            "  {region:?}: {hits}/{} = {:.1}%",
            in_region.len(),
            100.0 * hits as f64 / in_region.len() as f64
        );

        total += in_region.len();
        correct += hits;
    }

    let accuracy = correct as f64 / total as f64;
    eprintln!(
        "  overall (single-phase only): {correct}/{total} = {:.1}%",
        100.0 * accuracy
    );

    assert!(
        accuracy > CLASSIFIER_ACCURACY_FLOOR,
        "classifier agreed with the forward dispatcher on only {:.1}% of {total} \
         single-phase states, below the recorded floor of {:.1}%",
        100.0 * accuracy,
        100.0 * CLASSIFIER_ACCURACY_FLOOR
    );
}

/// Checks the classifier's score vector is consistent with its own decision.
///
/// # Methodology
///
/// [`rho_h_region_candidate`] must return exactly the region whose score is
/// largest in [`rho_h_region_scores`]. Guards against the two entry points
/// drifting apart, since callers are told to use the score margin as an
/// ambiguity signal for the decision.
///
/// # Results
///
/// Measured 2026-08-31: consistent at every sampled state.
#[test]
fn rho_h_region_scores_agree_with_the_classifier_decision() {
    let points = generate_sample_points(25, 25);
    assert!(
        !points.is_empty(),
        "no states generated — the sweep is wrong"
    );

    for point in &points {
        let scores = rho_h_region_scores(point.rho_kg_m3, point.h_kj_kg);
        let best = scores
            .iter()
            .enumerate()
            .max_by(|a, b| a.1.partial_cmp(b.1).unwrap())
            .map(|(index, _)| index)
            .expect("five scores");
        let expected = [
            RhoHRegion::Region1,
            RhoHRegion::Region2,
            RhoHRegion::Region3,
            RhoHRegion::Region4,
            RhoHRegion::Region5,
        ][best];

        assert_eq!(
            rho_h_region_candidate(point.rho_kg_m3, point.h_kj_kg),
            expected,
            "classifier decision disagreed with its own top score at \
             rho = {} kg/m3, h = {} kJ/kg",
            point.rho_kg_m3,
            point.h_kj_kg
        );
    }
}

/// The classifier must reject a non-positive density rather than return a
/// meaningless region, since it works in `log10(rho)`.
#[test]
#[should_panic(expected = "strictly positive density")]
fn rho_h_region_classifier_rejects_non_positive_density() {
    let _ = rho_h_region_candidate(0.0, 2000.0);
}

/// Measured median relative-error envelope for `p(rho,h)` in Region 1, where
/// the inversion is intrinsically ill-conditioned.
///
/// Measured 2026-08-31: p50 = 5.62e-4 over 299 states. Records what the fit
/// achieves — never loosen to make a failing test pass (see the crate
/// `CLAUDE.md` guardrails).
const P_RHO_H_REGION_1_P50_REL_TOLERANCE: f64 = 5.0e-3;
/// Measured agreement floor for the statistical region classifier against the
/// crate's forward dispatcher, over single-phase states only.
const CLASSIFIER_ACCURACY_FLOOR: f64 = 0.90;
