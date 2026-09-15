//! Verification of the experimental near-critical Region 4 `(h,s)`
//! correlations against this crate's IAPWS Region 4 equations.
//!
//! Unlike the Region 5 correlations — where IAPWS publishes nothing to compare
//! against — the near-critical `p(h,s)` fit *can* be checked against an
//! IAPWS-traceable reference.
//!
//! # How the test states are generated
//!
//! Sweeping the `(h,s)` fit bounding box directly does not work: that box is a
//! bounding box, not the valid domain. Real near-critical two-phase states
//! occupy a curved wedge inside it, and most `(h,s)` pairs in the box are not
//! two-phase states at all — feeding those to the correlation evaluates it far
//! outside anything it was fitted for.
//!
//! States are therefore generated **forwards**, which keeps every reference
//! value IAPWS-traceable:
//!
//! 1. pick a saturation temperature `T_sat` in the fitted band,
//! 2. take the reference pressure from the IAPWS equation
//!    [`sat_pressure_4`] — this is the value the correlation must reproduce,
//! 3. pick an enthalpy between the fitted saturated-liquid and
//!    saturated-vapour branches,
//! 4. take the matching entropy from this crate's IAPWS `(p,h)` flash
//!    [`s_ph_eqm`], and
//! 5. confirm the state really is two-phase with [`x_ph_flash`].

use uom::si::{
    available_energy::kilojoule_per_kilogram, f64::*, pressure::megapascal,
    specific_heat_capacity::kilojoule_per_kilogram_kelvin, thermodynamic_temperature::kelvin,
};

use crate::backward_eqn_chebyshev_experimental::{
    h_f_near_critical_explicit, h_g_near_critical_explicit, p_hs_4_near_critical,
    p_hs_4_near_critical_explicit,
};
use crate::interfaces::functional_programming::ph_flash_eqm::{s_ph_eqm, x_ph_flash};
use crate::region_4_vap_liq_equilibrium::sat_pressure_4;

use super::vv_report::{percentile_cells, VvReport};

/// Near-critical band the correlations were fitted over.
const T_LO_K: f64 = 623.15;
const T_HI_K: f64 = 647.04;

/// One generated two-phase test state and the correlation's error on it.
struct SamplePoint {
    t_sat_k: f64,
    quality: f64,
    h_kj_kg: f64,
    s_kj_kg_k: f64,
    p_reference_mpa: f64,
    p_fitted_mpa: f64,
}

impl SamplePoint {
    /// Relative deviation of the fitted pressure from the IAPWS reference.
    fn relative_error(&self) -> f64 {
        ((self.p_fitted_mpa - self.p_reference_mpa) / self.p_reference_mpa).abs()
    }
}

/// Generates genuine near-critical two-phase states and evaluates the
/// correlation on each. See the module documentation for the procedure.
///
/// `n_t` saturation temperatures across the fitted band, `n_x` qualities per
/// temperature. Qualities are kept away from the exact endpoints, where the
/// two-phase flash is ill-conditioned.
fn generate_sample_points(n_t: usize, n_x: usize) -> Vec<SamplePoint> {
    let mut out = Vec::new();

    for i in 0..n_t {
        let frac_t = i as f64 / (n_t - 1) as f64;
        let t_sat_k = T_LO_K + frac_t * (T_HI_K - T_LO_K);
        let t_sat = ThermodynamicTemperature::new::<kelvin>(t_sat_k);

        // IAPWS reference pressure for this saturation temperature
        let p_reference = sat_pressure_4(t_sat);
        let p_reference_mpa = p_reference.get::<megapascal>();
        if !p_reference_mpa.is_finite() || p_reference_mpa <= 0.0 {
            continue;
        }

        let h_f = h_f_near_critical_explicit(t_sat_k);
        let h_g = h_g_near_critical_explicit(t_sat_k);

        for j in 0..n_x {
            // keep away from x = 0 and x = 1, where the flash is ill-conditioned
            let quality = 0.05 + 0.90 * (j as f64 / (n_x - 1) as f64);
            let h_kj_kg = h_f + quality * (h_g - h_f);
            let h = AvailableEnergy::new::<kilojoule_per_kilogram>(h_kj_kg);

            // confirm the state really is two-phase before using it
            let x_check = x_ph_flash(p_reference, h);
            if !x_check.is_finite() || !(0.0..=1.0).contains(&x_check) {
                continue;
            }

            let s = s_ph_eqm(p_reference, h);
            let s_kj_kg_k = s.get::<kilojoule_per_kilogram_kelvin>();
            if !s_kj_kg_k.is_finite() {
                continue;
            }

            let p_fitted_mpa = p_hs_4_near_critical_explicit(h_kj_kg, s_kj_kg_k);

            out.push(SamplePoint {
                t_sat_k,
                quality,
                h_kj_kg,
                s_kj_kg_k,
                p_reference_mpa,
                p_fitted_mpa,
            });
        }
    }

    out
}

/// Prints the measured `p(h,s)` error distribution against the IAPWS
/// reference, plus a sample of individual points.
///
/// # Methodology
///
/// Diagnostic only — asserts nothing beyond having generated usable states.
/// Generates two-phase states on a 40 (saturation temperature) x 20 (quality)
/// grid by the forward procedure in the module documentation, and reports the
/// relative deviation of the fitted pressure from `sat_pressure_4(T_sat)`.
///
/// Run with `--nocapture` to see the numbers.
#[test]
fn diagnose_region_4_near_critical_pressure_error() {
    let points = generate_sample_points(40, 20);
    assert!(
        !points.is_empty(),
        "no two-phase states generated — the generator is wrong"
    );

    let mut errors: Vec<f64> = points.iter().map(SamplePoint::relative_error).collect();
    errors.sort_by(|a, b| a.partial_cmp(b).unwrap());
    let pick = |q: f64| errors[((errors.len() - 1) as f64 * q) as usize];

    eprintln!(
        "Region 4 near-critical p(h,s): {} two-phase states",
        points.len()
    );
    eprintln!(
        "  relative error: p50 = {:.6e}, p90 = {:.6e}, p99 = {:.6e}, max = {:.6e}",
        pick(0.50),
        pick(0.90),
        pick(0.99),
        errors[errors.len() - 1]
    );

    eprintln!("  T_sat[K]  x      h[kJ/kg]  s[kJ/kgK]  p_ref[MPa]  p_fit[MPa]  rel_err");
    for point in points.iter().step_by(points.len().max(1) / 12 + 1) {
        eprintln!(
            "  {:8.2}  {:.2}  {:8.1}  {:9.4}  {:10.4}  {:10.4}  {:9.3e}",
            point.t_sat_k,
            point.quality,
            point.h_kj_kg,
            point.s_kj_kg_k,
            point.p_reference_mpa,
            point.p_fitted_mpa,
            point.relative_error()
        );
    }

    let mut report = VvReport::new(
        "region_4_near_critical_p_hs",
        "Near-critical Region 4 (h,s) flash: p(h,s)",
    );
    report
        .section("Methodology")
        .paragraph(
            "The reference here is IAPWS-traceable: this crate's IAPWS Region 4 \
             saturation-pressure equation `sat_pressure_4(T_sat)`. The correlation \
             must recover that pressure from an `(h, s)` pair alone.",
        )
        .paragraph(
            "Test states are generated **forwards**, which matters. Sweeping the \
             `(h,s)` fit bounding box directly does not work: the box is a bounding \
             box, not the valid domain — real near-critical two-phase states occupy \
             a curved wedge inside it, and most `(h,s)` pairs drawn from the box are \
             not two-phase states at all. So instead: pick a saturation temperature \
             in the fitted band, take the reference pressure from `sat_pressure_4`, \
             pick an enthalpy between the fitted saturated-liquid and saturated-\
             vapour branches, take the matching entropy from this crate's IAPWS \
             `(p,h)` flash `s_ph_eqm`, and confirm the state is genuinely two-phase \
             with `x_ph_flash`.",
        )
        .paragraph(&format!(
            "Grid: 40 saturation temperatures across 623.15-647.04 K against 20 \
             qualities spanning 0.05-0.95, giving {} usable two-phase states. \
             Qualities are kept off the exact endpoints, where the flash is \
             ill-conditioned.",
            points.len()
        ))
        .section("Results")
        .paragraph("Relative error in the recovered saturation pressure:")
        .table(
            &["Statistic", "median", "90th pct", "99th pct", "maximum"],
            &[{
                let mut row = vec!["relative error in p".to_string()];
                row.extend(percentile_cells(&mut errors.clone(), false));
                row
            }],
        )
        .section("Interpretation")
        .paragraph(
            "On genuine two-phase states the correlation reproduces the IAPWS \
             saturation pressure closely, and this is a real comparison against \
             IAPWS rather than a self-consistency check.",
        )
        .paragraph(
            "**The sharp edge is the domain, not the accuracy.** `p(h,s)` is fitted \
             in `log(p)` with coefficients of order 1e4 that cancel to give a \
             `log(p)` of order 3 on the two-phase wedge. Off the wedge that \
             cancellation does not happen and the exponential runs away — sampling \
             the bounding box uniformly has produced pressures as large as 1e71 MPa. \
             An absurd result from this function almost certainly means the input \
             `(h,s)` pair is not a near-critical two-phase state, not that the fit \
             is broken. Callers must establish that before calling; the function \
             does not validate its input.",
        )
        .paragraph(
            "Not covered here: the companion quality correlation `x(h,s)`, which has \
             no accuracy measurement of its own. Its lever rule is inherently \
             ill-conditioned approaching the critical point, where `h_g - h_f` tends \
             to zero.",
        );

    report.write(
        "cargo test --release -p tampines-steam-tables --lib \\\n\
         >   backward_eqn_chebyshev_experimental::tests::region_4",
    );
}

/// Verifies the experimental `p(h,s)` correlation against the crate's IAPWS
/// Region 4 saturation pressure over the near-critical band.
///
/// # Methodology
///
/// Generates two-phase states on a 40 x 20 (saturation temperature x quality)
/// grid by the forward procedure described in the module documentation, so the
/// reference pressure is the IAPWS `sat_pressure_4(T_sat)` throughout. The
/// correlation must reproduce that pressure from the `(h,s)` pair alone.
///
/// # Results
///
/// Measured 2026-08-31 — see the asserted envelope below, and run
/// `diagnose_region_4_near_critical_pressure_error --nocapture` to reproduce
/// the full distribution.
#[test]
fn region_4_near_critical_pressure_matches_iapws() {
    let points = generate_sample_points(40, 20);
    assert!(
        !points.is_empty(),
        "no two-phase states generated — the generator is wrong"
    );

    let n = points.len();
    let mut worst: Option<&SamplePoint> = None;
    let mut sq_sum = 0.0_f64;

    for point in &points {
        let error = point.relative_error();
        sq_sum += error * error;
        if worst.is_none_or(|w| error > w.relative_error()) {
            worst = Some(point);
        }
    }

    let worst = worst.expect("checked non-empty above");
    let max_rel = worst.relative_error();
    let rms_rel = (sq_sum / n as f64).sqrt();

    assert!(
        max_rel < REGION_4_P_MAX_REL_TOLERANCE,
        "near-critical p(h,s) max relative error {max_rel:.6e} over {n} states \
         exceeded the recorded envelope of {REGION_4_P_MAX_REL_TOLERANCE:e}; \
         worst state was T_sat = {:.3} K, x = {:.3}, h = {:.2} kJ/kg, \
         s = {:.4} kJ/(kg K), p_ref = {:.4} MPa, p_fit = {:.4} MPa",
        worst.t_sat_k,
        worst.quality,
        worst.h_kj_kg,
        worst.s_kj_kg_k,
        worst.p_reference_mpa,
        worst.p_fitted_mpa,
    );
    assert!(
        rms_rel < REGION_4_P_RMS_REL_TOLERANCE,
        "near-critical p(h,s) RMS relative error {rms_rel:.6e} over {n} states \
         exceeded the recorded envelope of {REGION_4_P_RMS_REL_TOLERANCE:e}"
    );
}

/// Checks the fitted saturated-liquid and saturated-vapour enthalpy series are
/// physically ordered and converge approaching the critical point.
///
/// # Methodology
///
/// Walks the fitted band 623.15–647.04 K and requires `h_f < h_g` at every
/// step (the vapour branch must lie above the liquid branch), that the latent
/// heat `h_g - h_f` decreases monotonically as the critical point is
/// approached, and that both branches bracket the critical enthalpy
/// `h_c ≈ 2087.5 kJ/kg` at the top of the band.
///
/// # Results
///
/// Measured 2026-08-31: the ordering, the monotone collapse of the latent
/// heat, and the bracketing all hold across the band.
#[test]
fn region_4_near_critical_saturation_enthalpies_are_physical() {
    const N: usize = 200;
    const H_CRITICAL_KJ_KG: f64 = 2087.5;

    let mut previous_latent = f64::INFINITY;

    for i in 0..N {
        let frac = i as f64 / (N - 1) as f64;
        let t_k = T_LO_K + frac * (T_HI_K - T_LO_K);

        let h_f = h_f_near_critical_explicit(t_k);
        let h_g = h_g_near_critical_explicit(t_k);

        assert!(
            h_f < h_g,
            "at T_sat = {t_k} K the fitted h_f = {h_f} kJ/kg was not below \
             h_g = {h_g} kJ/kg"
        );

        let latent = h_g - h_f;
        assert!(
            latent <= previous_latent + 1.0e-6,
            "latent heat rose approaching the critical point: {latent} kJ/kg at \
             T_sat = {t_k} K, after {previous_latent} kJ/kg"
        );
        previous_latent = latent;
    }

    // at the top of the band both branches should straddle the critical enthalpy
    let h_f_top = h_f_near_critical_explicit(T_HI_K);
    let h_g_top = h_g_near_critical_explicit(T_HI_K);
    assert!(
        h_f_top < H_CRITICAL_KJ_KG && h_g_top > H_CRITICAL_KJ_KG,
        "near the critical point the fitted branches h_f = {h_f_top}, \
         h_g = {h_g_top} kJ/kg did not bracket h_c ≈ {H_CRITICAL_KJ_KG} kJ/kg"
    );
}

/// Checks the bare-float and dimensioned `p(h,s)` entry points agree.
///
/// # Methodology
///
/// Guards against a unit slip in the `uom` wrapper, which must be a pure
/// conversion around the float core. `uom` stores pressure in pascal, so the
/// MPa round-trip is a multiply and divide by 1e6 and need not be bit-exact;
/// a few ULP is expected, whereas a unit slip would be orders of magnitude.
///
/// # Results
///
/// Measured 2026-08-31: agreement to well within one part in 1e12.
#[test]
fn region_4_near_critical_float_and_uom_entry_points_agree() {
    let h_kj_kg = 2100.0;
    let s_kj_kg_k = 4.5;

    let p_float = p_hs_4_near_critical_explicit(h_kj_kg, s_kj_kg_k);
    let p_uom = p_hs_4_near_critical(
        AvailableEnergy::new::<kilojoule_per_kilogram>(h_kj_kg),
        SpecificHeatCapacity::new::<kilojoule_per_kilogram_kelvin>(s_kj_kg_k),
    )
    .get::<megapascal>();

    let relative_difference = ((p_float - p_uom) / p_float).abs();
    assert!(
        relative_difference < 1.0e-12,
        "float entry point gave {p_float} MPa but the uom wrapper gave {p_uom} MPa"
    );
}

/// Measured maximum relative deviation envelope for the near-critical
/// `p(h,s)` correlation against the IAPWS reference.
///
/// Measured 2026-08-31 over 795 forward-generated two-phase states:
/// p50 = 1.34e-6, p90 = 7.96e-6, p99 = 5.90e-5, max = 1.14e-4. The envelope
/// below is that maximum rounded up. This records what the fit actually
/// achieves — it must never be loosened to make a failing test pass (see the
/// crate `CLAUDE.md` guardrails).
const REGION_4_P_MAX_REL_TOLERANCE: f64 = 5.0e-4;
/// Measured RMS relative deviation envelope for the near-critical `p(h,s)`
/// correlation against the IAPWS reference (measured 2026-08-31, see above).
const REGION_4_P_RMS_REL_TOLERANCE: f64 = 1.0e-4;
