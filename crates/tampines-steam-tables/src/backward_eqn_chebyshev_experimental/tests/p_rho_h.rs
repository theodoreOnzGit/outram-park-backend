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
//! Two generators are used, because a `(T,p)` pair on the saturation line is
//! underdetermined without a quality:
//!
//! - [`generate_sample_points`] sweeps `(p, T)` for the **single-phase**
//!   regions, labelling each state with the crate's own dispatcher.
//! - [`generate_two_phase_sample_points`] sweeps `(T, x)` for **Region 4**,
//!   taking the reference pressure from `sat_pressure_4(T_sat)` and the
//!   mixture properties from the crate's two-phase `(T,p,x)` flashes, with
//!   quality running from the bubble point to the dew point inclusive.

use uom::si::{
    available_energy::kilojoule_per_kilogram, f64::*, mass_density::kilogram_per_cubic_meter,
    pressure::megapascal, specific_volume::cubic_meter_per_kilogram,
    thermodynamic_temperature::kelvin,
};

use crate::backward_eqn_chebyshev_experimental::{
    p_rho_h_in_region, rho_h_region_candidate, rho_h_region_scores, RhoHRegion,
};
use crate::interfaces::functional_programming::pt_flash_eqm::{
    h_tp_eqm_single_phase, h_tp_eqm_two_phase, region_fwd_eqn_single_phase, v_tp_eqm_single_phase,
    v_tp_eqm_two_phase, FwdEqnRegion,
};
use crate::region_4_vap_liq_equilibrium::sat_pressure_4;

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

/// Prints the Region 1 error distribution as percentages, binned by pressure
/// decade, to show where the ill-conditioning actually bites.
///
/// # Methodology
///
/// Diagnostic only — asserts nothing. Takes the Region 1 subset of the
/// standard sweep and reports the relative error as a percentage, both overall
/// and per pressure decade, since the conditioning of the `rho -> p` inversion
/// depends strongly on pressure.
///
/// Run with `--nocapture` to see the numbers.
#[test]
fn diagnose_p_rho_h_region_1_percentage_error() {
    let points = generate_sample_points(60, 60);
    let region_1: Vec<&SamplePoint> = points
        .iter()
        .filter(|point| point.region == RhoHRegion::Region1)
        .collect();
    assert!(!region_1.is_empty(), "no Region 1 states generated");

    let percentiles = |mut errors: Vec<f64>| -> (f64, f64, f64, f64) {
        errors.sort_by(|a, b| a.partial_cmp(b).unwrap());
        let pick = |q: f64| errors[((errors.len() - 1) as f64 * q) as usize];
        (pick(0.50), pick(0.90), pick(0.99), errors[errors.len() - 1])
    };

    let as_percent: Vec<f64> = region_1
        .iter()
        .map(|point| 100.0 * point.relative_error())
        .collect();
    let (p50, p90, p99, max) = percentiles(as_percent);
    eprintln!(
        "Region 1 p(rho,h) error [%], all {} states: p50 = {p50:.4}%, \
         p90 = {p90:.3}%, p99 = {p99:.1}%, max = {max:.1}%",
        region_1.len()
    );

    eprintln!("  by pressure decade:");
    for (lo, hi, label) in [
        (1.0e-3, 1.0e-2, "1e-3..1e-2 MPa"),
        (1.0e-2, 1.0e-1, "1e-2..1e-1 MPa"),
        (1.0e-1, 1.0e0, "1e-1..1    MPa"),
        (1.0e0, 1.0e1, "1   ..10   MPa"),
        (1.0e1, 1.0e2, "10  ..100  MPa"),
    ] {
        let bin: Vec<f64> = region_1
            .iter()
            .filter(|point| point.p_reference_mpa >= lo && point.p_reference_mpa < hi)
            .map(|point| 100.0 * point.relative_error())
            .collect();
        if bin.is_empty() {
            eprintln!("    {label}: no states");
            continue;
        }
        let n = bin.len();
        let (p50, p90, _p99, max) = percentiles(bin);
        eprintln!("    {label}: n = {n:4}, p50 = {p50:9.4}%, p90 = {p90:9.3}%, max = {max:9.1}%");
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
/// The aggregate hides how strongly this depends on pressure. As percentages,
/// by decade (see `diagnose_p_rho_h_region_1_percentage_error`):
///
/// | Pressure | n | median | 90th pct | max |
/// |---|---|---|---|---|
/// | 1e-3 – 1e-2 MPa | 15 | 88.2% | 220.7% | 318.5% |
/// | 1e-2 – 1e-1 MPa | 32 | 6.77% | 20.3% | 29.4% |
/// | 1e-1 – 1 MPa | 54 | 0.699% | 1.83% | 2.7% |
/// | 1 – 10 MPa | 100 | 0.054% | 0.173% | 0.3% |
/// | 10 – 100 MPa | 98 | 0.0072% | 0.017% | ~0.0% |
///
/// **Consequence for callers:** the limitation is low *pressure*, not liquid
/// as such. Above ~1 MPa Region 1 recovers pressure to better than 0.3%, and
/// to 0.017% above 10 MPa. Below ~0.1 MPa it should not be used — carry
/// pressure as a state variable there instead.
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

/// Generates genuine two-phase (Region 4) states by a `(T, p, x)` forward
/// flash, sweeping quality from the bubble point to the dew point.
///
/// For each saturation temperature the reference pressure is the IAPWS
/// `sat_pressure_4(T_sat)`, and the mixture's specific volume and enthalpy
/// come from the crate's own two-phase `(T,p,x)` flashes. Density is the
/// reciprocal of specific volume. Every reference value is therefore
/// IAPWS-traceable, and the quality sweep spans the full dome rather than a
/// fitted approximation of it.
fn generate_two_phase_sample_points(n_t: usize, n_x: usize) -> Vec<SamplePoint> {
    // Region 4 spans the triple point to the critical point; stop just short of
    // T_c = 647.096 K, where the two phases merge and the flash degenerates.
    const T_MIN_K: f64 = 280.0;
    const T_MAX_K: f64 = 645.0;

    let mut out = Vec::new();

    for i in 0..n_t {
        let frac_t = i as f64 / (n_t - 1) as f64;
        let t_kelvin = T_MIN_K + frac_t * (T_MAX_K - T_MIN_K);
        let t = ThermodynamicTemperature::new::<kelvin>(t_kelvin);

        let p = sat_pressure_4(t);
        let p_reference_mpa = p.get::<megapascal>();
        if !p_reference_mpa.is_finite() || p_reference_mpa <= 0.0 {
            continue;
        }

        for j in 0..n_x {
            // x = 0 is the bubble point, x = 1 the dew point; both included
            let quality = j as f64 / (n_x - 1) as f64;

            let v = v_tp_eqm_two_phase(t, p, quality).get::<cubic_meter_per_kilogram>();
            if !v.is_finite() || v <= 0.0 {
                continue;
            }
            let rho_kg_m3 = 1.0 / v;

            let h_kj_kg = h_tp_eqm_two_phase(t, p, quality).get::<kilojoule_per_kilogram>();
            if !h_kj_kg.is_finite() {
                continue;
            }

            let p_fitted_mpa = p_rho_h_in_region(
                RhoHRegion::Region4,
                MassDensity::new::<kilogram_per_cubic_meter>(rho_kg_m3),
                AvailableEnergy::new::<kilojoule_per_kilogram>(h_kj_kg),
            )
            .get::<megapascal>();

            out.push(SamplePoint {
                region: RhoHRegion::Region4,
                t_kelvin,
                rho_kg_m3,
                h_kj_kg,
                p_reference_mpa,
                p_fitted_mpa,
            });
        }
    }

    out
}

/// Prints the Region 4 `p(rho,h)` error distribution over the two-phase dome.
///
/// # Methodology
///
/// Diagnostic only — asserts nothing beyond having generated states. Sweeps
/// saturation temperature 280–645 K against quality 0 (bubble point) to 1 (dew
/// point) via the `(T,p,x)` forward flash, and reports the relative deviation
/// of the fitted pressure from `sat_pressure_4(T_sat)`. Also breaks the error
/// down by quality band, since the bubble- and dew-point edges are where a
/// two-phase fit is most likely to struggle.
///
/// Run with `--nocapture` to see the numbers.
#[test]
fn diagnose_p_rho_h_region_4_over_the_dome() {
    let points = generate_two_phase_sample_points(50, 21);
    assert!(!points.is_empty(), "no two-phase states generated");

    let percentiles = |mut errors: Vec<f64>| -> (f64, f64, f64, f64) {
        errors.sort_by(|a, b| a.partial_cmp(b).unwrap());
        let pick = |q: f64| errors[((errors.len() - 1) as f64 * q) as usize];
        (pick(0.50), pick(0.90), pick(0.99), errors[errors.len() - 1])
    };

    let all: Vec<f64> = points.iter().map(SamplePoint::relative_error).collect();
    let (p50, p90, p99, max) = percentiles(all);
    eprintln!(
        "Region 4 p(rho,h) over the dome, {} states: p50 = {p50:.3e}, \
         p90 = {p90:.3e}, p99 = {p99:.3e}, max = {max:.3e}",
        points.len()
    );

    if let Some(worst) = points
        .iter()
        .max_by(|a, b| a.relative_error().partial_cmp(&b.relative_error()).unwrap())
    {
        eprintln!(
            "  worst: T_sat = {:.2} K, rho = {:.5} kg/m3, h = {:.2} kJ/kg, \
             p_ref = {:.6} MPa, p_fit = {:.6} MPa",
            worst.t_kelvin,
            worst.rho_kg_m3,
            worst.h_kj_kg,
            worst.p_reference_mpa,
            worst.p_fitted_mpa
        );
    }
}

/// Verifies the Region 4 `p(rho,h)` surfaces across the two-phase dome, from
/// the bubble point to the dew point.
///
/// # Methodology
///
/// Closes the gap left by [`p_rho_h_round_trips_forward_equations_given_region`],
/// which cannot reach Region 4 because two-phase states are not representable
/// through the single-phase `(T,p)` flash. States are generated here with the
/// crate's two-phase `(T,p,x)` flashes instead, sweeping saturation
/// temperature 280–645 K against quality 0 to 1 inclusive, so both saturation
/// boundaries are exercised. The reference pressure is the IAPWS
/// `sat_pressure_4(T_sat)`.
///
/// # Results
///
/// Measured 2026-08-31 — see the asserted envelope below, and run
/// `diagnose_p_rho_h_region_4_over_the_dome --nocapture` for the distribution.
#[test]
fn p_rho_h_round_trips_over_the_two_phase_dome() {
    let points = generate_two_phase_sample_points(50, 21);
    assert!(!points.is_empty(), "no two-phase states generated");

    let n = points.len();
    let mut errors: Vec<f64> = points.iter().map(SamplePoint::relative_error).collect();
    errors.sort_by(|a, b| a.partial_cmp(b).unwrap());

    let p50 = errors[(n - 1) / 2];
    let p99 = errors[((n - 1) as f64 * 0.99) as usize];

    assert!(
        p50 < P_RHO_H_REGION_4_P50_REL_TOLERANCE,
        "Region 4 p(rho,h) median relative error {p50:.6e} over {n} two-phase \
         states exceeded the recorded envelope of \
         {P_RHO_H_REGION_4_P50_REL_TOLERANCE:e}"
    );
    assert!(
        p99 < P_RHO_H_REGION_4_P99_REL_TOLERANCE,
        "Region 4 p(rho,h) 99th-percentile relative error {p99:.6e} over {n} \
         two-phase states exceeded the recorded envelope of \
         {P_RHO_H_REGION_4_P99_REL_TOLERANCE:e}"
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
/// Measured median relative-error envelope for Region 4 `p(rho,h)` over the
/// two-phase dome.
///
/// Measured 2026-08-31 over 1050 states swept bubble point to dew point:
/// p50 = 2.20e-4, p90 = 1.34e-3, p99 = 4.73e-3, max = 1.77e-1. The maximum
/// sits at the 280 K bubble point, the low-pressure liquid-like corner where
/// density is least sensitive to pressure — the same conditioning problem that
/// dominates Region 1.
const P_RHO_H_REGION_4_P50_REL_TOLERANCE: f64 = 1.0e-3;
/// Measured 99th-percentile relative-error envelope for Region 4 `p(rho,h)`
/// over the two-phase dome (measured 2026-08-31, see above).
const P_RHO_H_REGION_4_P99_REL_TOLERANCE: f64 = 1.0e-2;
/// Measured agreement floor for the statistical region classifier against the
/// crate's forward dispatcher, over single-phase states only.
const CLASSIFIER_ACCURACY_FLOOR: f64 = 0.90;
