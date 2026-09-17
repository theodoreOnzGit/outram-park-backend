//! V&V — the incompressible backend's re-ported auxiliary correlations
//! (`incompressibles`, bead op-kbc.19): freezing temperature `T_freeze`,
//! saturation pressure `p_sat`, and the derived specific entropy / internal
//! energy. These four quantities were dropped at the original port even though
//! CoolProp's own JSON carries the coefficients; this file verifies the
//! restored evaluators against physical anchors and against a hand-evaluation
//! of the same JSON fit.
//!
//! Fixture fluids:
//! - **MEG** — CoolProp's aqueous Ethylene-Glycol brine (mass-based,
//!   `x ∈ [0.0, 0.6]`), which carries a real `polynomial` `T_freeze(x)` fit but
//!   no `saturation_pressure`.
//! - **T66** — Therminol 66 heat-transfer oil (pure), which carries a real
//!   `exponential` `saturation_pressure(T)` fit (`TminPsat = 343.15 K`,
//!   `reference: "Therminol2014"`) but no `T_freeze`. Its `specific_heat`
//!   `polynomial` fit drives the entropy / internal-energy checks.
//!
//! No CoolProp/`rfluids` runtime is available in this environment (op-kbc.3),
//! so the "reference" is (a) a physical anchor independent of the fit where one
//! exists, and (b) an independent Python hand-evaluation of the same upstream
//! JSON coefficients otherwise — a code-vs-formula check of the port's
//! `eval_t_freeze` / `eval_fit` / `entropy_raw_integral` engine.

use outram_park_fork_coolprop::incompressibles::{Incompressible, IncompressibleError};

fn rel(a: f64, b: f64) -> f64 {
    (a - b).abs() / b.abs()
}

/// METHODOLOGY: freezing temperature of CoolProp's aqueous ethylene-glycol
/// brine `MEG` at two compositions, via `Incompressible::MEG.t_freeze(x)`
/// (a `polynomial` fit in mass fraction `x`, centred on `xbase = 0.308462`,
/// evaluated at pressure 0 since the fit is single-row in pressure).
///
/// - x = 0.0 (pure water): the reference is the *physical* freezing point of
///   water, 273.15 K (0 °C) — an anchor independent of the fit. Tolerance 5e-4
///   relative (≈ 0.14 K), i.e. the correlation must reproduce the pure-water
///   limit.
/// - x = 0.30 (30 mass-% EG): reference is the independent Python evaluation of
///   the same JSON coeffs, 258.574222 K, cross-checked against the ASHRAE
///   ethylene-glycol phase diagram (30 mass-% freezes near −14 °C ≈ 259 K).
///   Tolerance 1e-6 relative (code-vs-formula).
///
/// RESULTS (2026-07-16, this port): T_freeze(x=0) = 273.150285 K (Δ = +0.0003 K
/// from the 273.15 K water anchor — the fit's own pure-water intercept);
/// T_freeze(x=0.30) = 258.574222 K = −14.58 °C, matching the hand evaluation to
/// < 1e-12 relative and consistent with the published ≈ −14 °C EG-30% freeze
/// depression. Interpretation: the restored `T_freeze` correlation is faithful
/// to CoolProp's fit and physically correct at the pure-water limit.
#[test]
fn meg_freezing_point_water_anchor_and_30_mass_percent() {
    let tf_pure = Incompressible::MEG.t_freeze(0.0).unwrap();
    let tf_30 = Incompressible::MEG.t_freeze(0.30).unwrap();
    println!(
        "MEG T_freeze: x=0 -> {tf_pure:.6} K, x=0.30 -> {tf_30:.6} K ({:.2} C)",
        tf_30 - 273.15
    );

    // Physical anchor: pure water freezes at 273.15 K.
    assert!(
        rel(tf_pure, 273.15) < 5e-4,
        "pure-water freeze point = {tf_pure}"
    );
    // Code-vs-formula against the upstream JSON coeffs (Python hand-eval).
    assert!(
        rel(tf_30, 258.574222) < 1e-6,
        "30 mass-% EG freeze point = {tf_30}"
    );
    // Sanity: adding antifreeze must depress the freezing point.
    assert!(tf_30 < tf_pure, "freeze point must drop with concentration");
}

/// METHODOLOGY: `T_freeze` is defined only for solutions that carry a real fit.
/// A pure oil (T66) must report the property as unavailable rather than return
/// a wrong number, matching the module's honest-failure style for viscosity /
/// conductivity. Also checks the composition guard on `MEG`.
///
/// RESULTS (2026-07-16): `T66.t_freeze(0.0)` returns
/// `Err(PropertyUnavailable)`; `MEG.t_freeze(0.9)` (above `xmax = 0.6`) returns
/// `Err(OutOfRange)`. Interpretation: unavailable / out-of-range inputs are
/// rejected, never silently mis-evaluated.
#[test]
fn t_freeze_unavailable_on_pure_oil_and_out_of_range_composition() {
    assert_eq!(
        Incompressible::T66.t_freeze(0.0),
        Err(IncompressibleError::PropertyUnavailable)
    );
    assert_eq!(
        Incompressible::MEG.t_freeze(0.9),
        Err(IncompressibleError::OutOfRange)
    );
}

/// METHODOLOGY: saturation (vapour) pressure of Therminol 66 (`T66`) via
/// `Incompressible::T66.saturation_pressure(T, 0.0)` — an `exponential` fit
/// `p_sat = exp(c0/(T + c1) − c2)` with `TminPsat = 343.15 K`.
///
/// - At T = 573.15 K (300 °C): reference is the independent Python evaluation
///   of the same JSON coeffs, 30718.4846 Pa (≈ 0.307 bar), cross-checked
///   against Eastman's published Therminol 66 vapour-pressure data (≈ 0.31 bar
///   at 300 °C). Tolerance 1e-6 relative.
/// - Normal boiling point: the temperature at which `p_sat = 101325 Pa` is a
///   physical anchor. Solving the fit gives 632.094 K = 358.94 °C; Eastman
///   lists the Therminol 66 atmospheric boiling point as 359 °C. We assert the
///   port evaluates `p_sat(632.094 K) = 101325 Pa` to 1e-6 relative and that
///   358.94 °C is within 1 °C of the published 359 °C.
///
/// RESULTS (2026-07-16, this port): p_sat(573.15 K) = 30718.4846 Pa (matches
/// hand-eval to < 1e-12 rel, ≈ published 0.31 bar); p_sat(632.094 K) =
/// 101325.0 Pa, i.e. an atmospheric boiling point of 358.94 °C vs Eastman's
/// published 359 °C (Δ ≈ 0.06 °C). Interpretation: the restored `p_sat`
/// correlation reproduces CoolProp's fit and the published Therminol 66 vapour
/// pressure / boiling point.
#[test]
fn t66_saturation_pressure_at_300c_and_normal_boiling_point() {
    let ps_300c = Incompressible::T66
        .saturation_pressure(573.15, 0.0)
        .unwrap();
    println!(
        "T66 p_sat(300 C) = {ps_300c:.4} Pa = {:.4} bar",
        ps_300c / 1e5
    );
    assert!(rel(ps_300c, 30718.4846) < 1e-6, "p_sat(300 C) = {ps_300c}");

    // Published atmospheric boiling point 359 C -> 632.094 K should sit at 1 atm.
    const T_NBP: f64 = 632.0938755546402;
    let ps_nbp = Incompressible::T66.saturation_pressure(T_NBP, 0.0).unwrap();
    println!("T66 p_sat(NBP {:.2} C) = {ps_nbp:.2} Pa", T_NBP - 273.15);
    assert!(
        rel(ps_nbp, 101_325.0) < 1e-6,
        "p_sat at published NBP = {ps_nbp}"
    );
    assert!(
        (T_NBP - 273.15 - 359.0).abs() < 1.0,
        "NBP within 1 C of Eastman 359 C"
    );
}

/// METHODOLOGY: `p_sat` must reject temperatures below `TminPsat` (the fit is
/// not valid there — CoolProp #2209) and report unavailable for a fluid with no
/// saturation correlation (MEG), rather than return a wrong number.
///
/// RESULTS (2026-07-16): `T66.saturation_pressure(300 K, 0.0)` (below
/// `TminPsat = 343.15 K`) returns `Err(OutOfRange)`; `MEG.saturation_pressure`
/// returns `Err(PropertyUnavailable)`. Interpretation: the `TminPsat` guard and
/// the missing-correlation guard both propagate as errors.
#[test]
fn saturation_pressure_guards_below_tminpsat_and_when_absent() {
    assert_eq!(
        Incompressible::T66.saturation_pressure(300.0, 0.0),
        Err(IncompressibleError::OutOfRange)
    );
    assert_eq!(
        Incompressible::MEG.saturation_pressure(300.0, 0.0),
        Err(IncompressibleError::PropertyUnavailable)
    );
}

/// METHODOLOGY: specific entropy of Therminol 66 from the same `specific_heat`
/// polynomial the module already integrates for enthalpy, evaluated at
/// T = 500 K via `Incompressible::T66.entropy(T, P, 0.0)`. The port's closed
/// form (CoolProp `Polynomial2DFrac::integral` with `x_exp = −1`, referenced to
/// zero at `t_range.0 = Tmin = 273.15 K`) is compared to a fine numerical
/// quadrature of `∫_{Tmin}^{T} c_p(T')/T' dT'` computed independently in Python
/// (400 000-panel midpoint rule), reference 1117.18669 J/(kg·K). Also checks
/// entropy is zero at the reference temperature and increases with T.
/// Tolerance 1e-6 relative.
///
/// RESULTS (2026-07-16, this port): entropy(500 K) = 1117.186691 J/(kg·K),
/// matching the numerical integral (1117.186691) to < 1e-9 relative; entropy
/// (273.15 K) = 0 exactly, and d s/d T > 0. Interpretation: the derived entropy
/// is analytically consistent with the c_p fit (an internally-consistent
/// reference, not CoolProp's absolute convention).
#[test]
fn t66_entropy_matches_numerical_cp_over_t_integral() {
    const P: f64 = 101_325.0;
    let s500 = Incompressible::T66.entropy(500.0, P, 0.0).unwrap();
    let s_ref = Incompressible::T66.entropy(273.15, P, 0.0).unwrap();
    let s400 = Incompressible::T66.entropy(400.0, P, 0.0).unwrap();
    println!("T66 entropy: s(500)={s500:.6}, s(273.15)={s_ref:.6}, s(400)={s400:.6}");

    assert!(rel(s500, 1117.186691) < 1e-6, "entropy(500 K) = {s500}");
    assert!(
        s_ref.abs() < 1e-9,
        "entropy at reference Tmin must be 0, got {s_ref}"
    );
    assert!(s400 > s_ref && s500 > s400, "entropy must increase with T");
}

/// METHODOLOGY: specific internal energy of Therminol 66 via
/// `Incompressible::T66.internal_energy(T, p, 0.0)`, which implements CoolProp's
/// `calc_umass = h − p·v = h − p/ρ`. Verified against `enthalpy − p/density`
/// evaluated from the port's own `enthalpy` and `density` at T = 500 K,
/// p = 1e5 Pa (self-consistency of the thermodynamic identity). The p/ρ
/// correction is small for a dense oil (≈ 116 J/kg), so `u` sits just below
/// `h`. Tolerance 1e-9 relative.
///
/// RESULTS (2026-07-16, this port): at (500 K, 1e5 Pa): h = 81996.0972 J/kg,
/// ρ = 865.4999 kg/m³, p/ρ = 115.5402 J/kg, so u = 81880.5570 J/kg — the method
/// reproduces h − p/ρ to < 1e-12 relative. Interpretation: internal energy is
/// consistent with the enthalpy/density fits and correctly pressure-dependent
/// (unlike the other incompressible properties).
#[test]
fn t66_internal_energy_equals_enthalpy_minus_p_over_rho() {
    const T: f64 = 500.0;
    const P: f64 = 1.0e5;
    let u = Incompressible::T66.internal_energy(T, P, 0.0).unwrap();
    let h = Incompressible::T66.enthalpy(T, P, 0.0).unwrap();
    let rho = Incompressible::T66.density(T, P, 0.0).unwrap();
    let expected = h - P / rho;
    println!(
        "T66 @500K,1e5Pa: u={u:.4} h={h:.4} rho={rho:.4} p/rho={:.4}",
        P / rho
    );

    assert!(
        rel(u, expected) < 1e-9,
        "u = {u}, expected h - p/rho = {expected}"
    );
    assert!(rel(u, 81880.5570) < 1e-6, "u(500 K, 1e5 Pa) = {u}");
    assert!(
        u < h,
        "p/rho correction must lower u below h for a dense liquid"
    );
}
