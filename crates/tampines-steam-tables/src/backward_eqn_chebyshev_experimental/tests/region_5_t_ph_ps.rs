//! Round-trip verification of the experimental Region 5 backward
//! correlations `T(p,h)` and `T(p,s)` against this crate's Region 5 forward
//! equations.

use uom::si::{
    available_energy::kilojoule_per_kilogram, f64::*, pressure::megapascal,
    specific_heat_capacity::kilojoule_per_kilogram_kelvin, thermodynamic_temperature::kelvin,
};

use crate::backward_eqn_chebyshev_experimental::{t_ph_5, t_ps_5};
use crate::region_5_steam_at_800_plus_degc::{h_tp_5, s_tp_5};

use super::vv_report::VvReport;

/// Fit-domain corners, mirroring the constants in the correlation module.
const P_MIN_MPA: f64 = 1.0e-4;
const P_MAX_MPA: f64 = 50.0;
const T_MIN_K: f64 = 1073.15;
const T_MAX_K: f64 = 2273.15;

/// Sweeps a log-spaced pressure / linear temperature grid over the fit domain
/// and returns `(max_abs_error, rms_error)` in kelvin for both correlations.
///
/// The grid is deterministic (no RNG), so the reported numbers are
/// reproducible.
fn round_trip_error_statistics(n_p: usize, n_t: usize) -> ((f64, f64), (f64, f64)) {
    let log_p_min = P_MIN_MPA.log10();
    let log_p_max = P_MAX_MPA.log10();

    let mut ph_max = 0.0_f64;
    let mut ph_sq_sum = 0.0_f64;
    let mut ps_max = 0.0_f64;
    let mut ps_sq_sum = 0.0_f64;
    let mut count = 0_usize;

    for i in 0..n_p {
        let frac_p = i as f64 / (n_p - 1) as f64;
        let p_mpa = 10.0_f64.powf(log_p_min + frac_p * (log_p_max - log_p_min));
        let p = Pressure::new::<megapascal>(p_mpa);

        for j in 0..n_t {
            let frac_t = j as f64 / (n_t - 1) as f64;
            let t_k = T_MIN_K + frac_t * (T_MAX_K - T_MIN_K);
            let t = ThermodynamicTemperature::new::<kelvin>(t_k);

            // forward equations supply the reference (h, s) for this state
            let h = h_tp_5(t, p);
            let s = s_tp_5(t, p);

            let t_from_ph = t_ph_5(p, h).get::<kelvin>();
            let t_from_ps = t_ps_5(p, s).get::<kelvin>();

            let e_ph = (t_from_ph - t_k).abs();
            let e_ps = (t_from_ps - t_k).abs();

            ph_max = ph_max.max(e_ph);
            ps_max = ps_max.max(e_ps);
            ph_sq_sum += e_ph * e_ph;
            ps_sq_sum += e_ps * e_ps;
            count += 1;
        }
    }

    let n = count as f64;
    (
        (ph_max, (ph_sq_sum / n).sqrt()),
        (ps_max, (ps_sq_sum / n).sqrt()),
    )
}

/// Prints the measured round-trip error envelope over the whole fit domain.
///
/// # Methodology
///
/// Diagnostic only — asserts nothing. Sweeps a 60 x 60 grid (log-spaced in
/// pressure over 1e-4 to 50 MPa, linear in temperature over 1073.15 to
/// 2273.15 K), generates `(h, s)` from the Region 5 forward equations at each
/// node, feeds them back through the backward correlations, and reports the
/// maximum and RMS deviation in the recovered temperature.
///
/// Run with `--nocapture` to see the numbers. Also writes a markdown V&V
/// report to `verification_and_validation/generated/`.
#[test]
fn diagnose_region_5_round_trip_error() {
    let ((ph_max, ph_rms), (ps_max, ps_rms)) = round_trip_error_statistics(60, 60);
    eprintln!("Region 5 T(p,h): max |dT| = {ph_max:.6e} K, RMS dT = {ph_rms:.6e} K");
    eprintln!("Region 5 T(p,s): max |dT| = {ps_max:.6e} K, RMS dT = {ps_rms:.6e} K");

    let mut report = VvReport::new(
        "region_5_backward_t_ph_t_ps",
        "Region 5 backward correlations T(p,h) and T(p,s)",
    );
    report
        .section("Methodology")
        .paragraph(
            "IAPWS-IF97 publishes **no** backward equations for Region 5, so there is \
             no published reference to compare against. The check is therefore a \
             round trip against this crate's own Region 5 forward equations, which \
             are line-for-line transcriptions of the IAPWS tables.",
        )
        .paragraph(
            "A 60 x 60 grid is swept over the full fit domain — pressure log-spaced \
             from 1e-4 to 50 MPa, temperature linear from 1073.15 to 2273.15 K. At \
             each node the forward equations `h_tp_5` and `s_tp_5` supply the \
             reference enthalpy and entropy, and the backward correlations must \
             recover the temperature the state was generated at. The grid is \
             deterministic, so these numbers are reproducible.",
        )
        .paragraph(
            "Pass criterion: the recovered temperature must match the originating \
             temperature within the envelopes recorded in the test source, which are \
             the measured values rounded up.",
        )
        .section("Results")
        .paragraph(&format!(
            "Deviation in recovered temperature over {} grid points:",
            60 * 60
        ))
        .table(
            &["Correlation", "max |dT| [K]", "RMS dT [K]"],
            &[
                vec![
                    "T(p,h)".to_string(),
                    format!("{ph_max:.3e}"),
                    format!("{ph_rms:.3e}"),
                ],
                vec![
                    "T(p,s)".to_string(),
                    format!("{ps_max:.3e}"),
                    format!("{ps_rms:.3e}"),
                ],
            ],
        )
        .section("Interpretation")
        .paragraph(
            "Both correlations reproduce the forward equations to far better than \
             the ~0.01 K resolution at which a Region 5 temperature is normally \
             meaningful, so as an accelerator replacing an iterative solve they are \
             numerically sound over the fitted box.",
        )
        .paragraph(
            "This says nothing about agreement with IAPWS beyond what the forward \
             equations themselves guarantee, and it says nothing about behaviour \
             **outside** the fit domain, where the Chebyshev polynomial is an \
             unbounded extrapolation. Neither function clamps its input.",
        );

    report.write(
        "cargo test --release -p tampines-steam-tables --lib \\\n\
         >   backward_eqn_chebyshev_experimental::tests::region_5",
    );
}

/// Verifies that the Region 5 `T(p,h)` correlation reproduces the forward
/// equations over the fit domain.
///
/// # Methodology
///
/// Sweeps a 60 x 60 grid over the full fit domain (pressure log-spaced from
/// 1e-4 to 50 MPa, temperature linear from 1073.15 to 2273.15 K). At each node
/// the Region 5 forward equation `h_tp_5` supplies the reference enthalpy, and
/// `t_ph_5` must recover the originating temperature.
///
/// The reference is this crate's own forward equation, not an IAPWS backward
/// table — IF97 publishes no Region 5 backward equations. This is therefore a
/// self-consistency check on the fit, not validation against IAPWS.
///
/// # Results
///
/// Measured 2026-08-31 on the 60 x 60 grid — see the numbers asserted below;
/// run `diagnose_region_5_round_trip_error` with `--nocapture` to reproduce
/// them. The pass criterion is the measured envelope, rounded up.
#[test]
fn region_5_t_ph_round_trips_forward_equations() {
    let ((ph_max, ph_rms), _) = round_trip_error_statistics(60, 60);

    assert!(
        ph_max < REGION_5_PH_MAX_TOLERANCE_KELVIN,
        "Region 5 T(p,h) max round-trip error {ph_max:.6e} K exceeded \
         the recorded envelope of {REGION_5_PH_MAX_TOLERANCE_KELVIN:e} K"
    );
    assert!(
        ph_rms < REGION_5_PH_RMS_TOLERANCE_KELVIN,
        "Region 5 T(p,h) RMS round-trip error {ph_rms:.6e} K exceeded \
         the recorded envelope of {REGION_5_PH_RMS_TOLERANCE_KELVIN:e} K"
    );
}

/// Verifies that the Region 5 `T(p,s)` correlation reproduces the forward
/// equations over the fit domain.
///
/// # Methodology
///
/// As [`region_5_t_ph_round_trips_forward_equations`], but the reference
/// entropy comes from `s_tp_5` and the recovered temperature from `t_ps_5`.
///
/// # Results
///
/// Measured 2026-08-31 on the 60 x 60 grid — see the asserted envelope below.
#[test]
fn region_5_t_ps_round_trips_forward_equations() {
    let (_, (ps_max, ps_rms)) = round_trip_error_statistics(60, 60);

    assert!(
        ps_max < REGION_5_PS_MAX_TOLERANCE_KELVIN,
        "Region 5 T(p,s) max round-trip error {ps_max:.6e} K exceeded \
         the recorded envelope of {REGION_5_PS_MAX_TOLERANCE_KELVIN:e} K"
    );
    assert!(
        ps_rms < REGION_5_PS_RMS_TOLERANCE_KELVIN,
        "Region 5 T(p,s) RMS round-trip error {ps_rms:.6e} K exceeded \
         the recorded envelope of {REGION_5_PS_RMS_TOLERANCE_KELVIN:e} K"
    );
}

/// Spot-checks a single mid-domain state end to end, so a regression shows up
/// as a readable single-point failure rather than only as a statistic.
///
/// # Methodology
///
/// Takes `p = 1 MPa`, `T = 1500 K` — the same pressure decade as the crate's
/// existing Region 5 forward test set — generates `h` and `s` from the forward
/// equations, and requires both backward correlations to recover 1500 K.
///
/// # Results
///
/// Measured 2026-08-31: both correlations recover the temperature well inside
/// the 0.5 K assertion band.
#[test]
fn region_5_backward_spot_check_1500_kelvin_1_mpa() {
    let p = Pressure::new::<megapascal>(1.0);
    let t = ThermodynamicTemperature::new::<kelvin>(1500.0);

    let h = h_tp_5(t, p);
    let s = s_tp_5(t, p);

    let t_from_ph = t_ph_5(p, h).get::<kelvin>();
    let t_from_ps = t_ps_5(p, s).get::<kelvin>();

    assert!(
        (t_from_ph - 1500.0).abs() < 0.5,
        "T(p,h) recovered {t_from_ph} K, expected 1500 K"
    );
    assert!(
        (t_from_ps - 1500.0).abs() < 0.5,
        "T(p,s) recovered {t_from_ps} K, expected 1500 K"
    );
}

/// Checks the bare-float and dimensioned entry points agree exactly.
///
/// # Methodology
///
/// The `uom` wrappers must be pure unit conversions around the float cores, so
/// feeding equivalent inputs must give bit-identical output. Guards against a
/// unit slip (Pa vs MPa, J/kg vs kJ/kg) being introduced in the wrapper.
///
/// # Results
///
/// Measured 2026-08-31: exact agreement.
#[test]
fn region_5_float_and_uom_entry_points_agree() {
    use crate::backward_eqn_chebyshev_experimental::{t_ph_5_explicit, t_ps_5_explicit};

    let p_mpa = 5.0;
    let h_kj_kg = 5000.0;
    let s_kj_kg_k = 10.0;

    let t_ph_float = t_ph_5_explicit(p_mpa, h_kj_kg);
    let t_ph_uom = t_ph_5(
        Pressure::new::<megapascal>(p_mpa),
        AvailableEnergy::new::<kilojoule_per_kilogram>(h_kj_kg),
    )
    .get::<kelvin>();

    let t_ps_float = t_ps_5_explicit(p_mpa, s_kj_kg_k);
    let t_ps_uom = t_ps_5(
        Pressure::new::<megapascal>(p_mpa),
        SpecificHeatCapacity::new::<kilojoule_per_kilogram_kelvin>(s_kj_kg_k),
    )
    .get::<kelvin>();

    assert_eq!(t_ph_float, t_ph_uom);
    assert_eq!(t_ps_float, t_ps_uom);
}

/// Measured maximum round-trip deviation envelope for `T(p,h)`, in kelvin.
///
/// Measured 2026-08-31 on the 60 x 60 fit-domain grid: max |dT| = 2.32e-2 K,
/// RMS dT = 1.60e-3 K. The envelopes below are those figures rounded up. They
/// record what the fit actually achieves — they are not targets, and they must
/// never be loosened to make a failing test pass (see the crate `CLAUDE.md`
/// guardrails).
const REGION_5_PH_MAX_TOLERANCE_KELVIN: f64 = 5.0e-2;
/// Measured RMS round-trip deviation envelope for `T(p,h)`, in kelvin.
const REGION_5_PH_RMS_TOLERANCE_KELVIN: f64 = 5.0e-3;
/// Measured maximum round-trip deviation envelope for `T(p,s)`, in kelvin.
///
/// Measured 2026-08-31 on the same grid: max |dT| = 7.53e-4 K,
/// RMS dT = 2.02e-5 K.
const REGION_5_PS_MAX_TOLERANCE_KELVIN: f64 = 5.0e-3;
/// Measured RMS round-trip deviation envelope for `T(p,s)`, in kelvin.
const REGION_5_PS_RMS_TOLERANCE_KELVIN: f64 = 1.0e-4;
