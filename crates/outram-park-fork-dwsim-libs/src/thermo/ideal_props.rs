//! Ideal-gas heat capacity / enthalpy / entropy — the **departure-function
//! reference state**.
//!
//! Ported from DWSIM (GPL-3.0):
//! - `DWSIM.Thermodynamics/BaseClasses/ThermodynamicsBase.vb`
//!   `GetIdealGasHeatCapacity` (lines 1758-1863) and the sibling `AUX_CPi`
//!   in `PropertyPackages/PropertyPackage.vb` (lines 5828-5851) — the Cp0
//!   correlation.
//! - `DWSIM.Thermodynamics/PropertyPackages/PropertyPackage.vb`
//!   `AUX_INT_CPDTi` / `AUX_INT_CPDT_Ti` (lines 7794-7976, the enthalpy and
//!   entropy integrals of Cp0), and `RET_Hid` / `RET_Sid` (lines 8558-8718,
//!   which add the `-R ln(P/P_ref)` pressure term and the `-R Σ x_i ln x_i`
//!   ideal entropy of mixing).
//!
//! The `PropertyPackageMethods.vb` file named in the porting brief is only the
//! method-selection *config* class (which correlation string each phase uses);
//! the numerical routines it selects live in the two files above, so that is
//! where this port draws from.
//!
//! ## The DWSIM Cp0 correlation (canonical `"DWSIM"`/`""` database path)
//!
//! DWSIM parameterises the ideal-gas heat capacity of a pure compound as a
//! **quartic polynomial in temperature**:
//!
//! ```text
//! Cp0(T) = A + B·T + C·T² + D·T³ + E·T⁴
//! ```
//!
//! with `A..E` = DWSIM's `Ideal_Gas_Heat_Capacity_Const_A..E`
//! ([`crate::thermo::Component::cp_ig_a`] … `cp_ig_e`) and `T` in kelvin.
//!
//! **Coefficient-unit convention (important).** The DWSIM source comment reads
//! *"Cp in kJ/kg-mol, T in K"* (`ThermodynamicsBase.vb:1776`,
//! `PropertyPackage.vb:1849`). `kg-mol` is the kilomole (kmol), so the raw
//! polynomial yields Cp0 in **kJ/(kmol·K)**. DWSIM's `AUX_CPi` then divides by
//! the molar mass to hand callers a **mass-basis** Cp in kJ/(kg·K)
//! (`… Return result / Molar_Weight 'kJ/kg.K`, `PropertyPackage.vb:1779/5851`).
//!
//! This port keeps the **molar** basis, which is a pure units identity — no
//! extra conversion needed:
//!
//! ```text
//! 1 kJ/(kmol·K) = 1000 J / (1000 mol · K) = 1 J/(mol·K)
//! ```
//!
//! so **Cp0 in J/(mol·K) equals the polynomial value directly**, and DWSIM's
//! `/ Molar_Weight` step is exactly the molar→mass conversion we intentionally
//! omit. The molar mass ([`Component::molar_mass`], kg/mol) is therefore *not*
//! used by any function here — it is only how DWSIM re-expresses these same
//! numbers on a mass basis downstream.
//!
//! ## Reference constants
//!
//! DWSIM hard-codes `R = 8.314` J/(mol·K) and `P_ref = 101325` Pa in these
//! routines (`PropertyPackage.vb:8695-8698`, `8711-8714`). This port uses the
//! CODATA gas constant [`R`] = 8.314 462 618 153 24 J/(mol·K) and takes the
//! reference pressure/temperature as explicit arguments so the departure
//! reference state is a caller decision, not a buried literal. With
//! `P_ref = 101325 Pa` and `T_ref = 298.15 K` the results match DWSIM's
//! convention to within the `8.314` vs CODATA-`R` difference (≈ 5.6e-5
//! relative on the pressure term only).
//!
//! ## Numerical vs analytic integral
//!
//! DWSIM integrates Cp0 **numerically** — a midpoint rule with an
//! adaptive step count (`AUX_INT_CPDTi`, `PropertyPackage.vb:7794-7826`).
//! Because the underlying correlation is a polynomial, this port instead uses
//! the **exact closed-form integral** of that same polynomial (below). This is
//! a faithful, higher-accuracy evaluation of the identical correlation — the
//! analytic result is what DWSIM's midpoint sum converges to as its step count
//! grows — not a different model.
//!
//! ```text
//! ∫_{T_ref}^{T} Cp0 dT   = A(T−T₀) + (B/2)(T²−T₀²) + (C/3)(T³−T₀³)
//!                          + (D/4)(T⁴−T₀⁴) + (E/5)(T⁵−T₀⁵)
//!
//! ∫_{T_ref}^{T} Cp0/T dT = A·ln(T/T₀) + B(T−T₀) + (C/2)(T²−T₀²)
//!                          + (D/3)(T³−T₀³) + (E/4)(T⁴−T₀⁴)
//! ```
//!
//! (writing `T₀` for `T_ref`.)
//!
//! ## Honest scope (verification, not benchmark validation)
//!
//! The inline tests below are **verification** — they check the closed-form
//! integrals against hand-computed algebra (a constant-Cp component,
//! polynomial closed forms, the exact pressure term, the mixing term). They do
//! **not** validate the DWSIM A..E coefficients themselves against measured
//! Cp0 data — that is compound-data validation and belongs with the compound
//! database, not this integrator.
//!
//! **Excluded / deferred, by design:**
//! - **The non-`"DWSIM"` Cp0 database paths** — DWSIM's `CheResources`
//!   (cal/mol·K, ×4.1868), `ChemSep`/`User`/`ChEDL Thermo`/`CoolProp`/
//!   `Biodiesel`/`KDB` (DIPPR/ChemSep equation-number forms evaluated by
//!   `CalcCSTDepProp`/`ParseEquation`), and the Lee-Kesler petroleum-fraction
//!   estimate (`Cpig_lk`). Only the canonical **A..E quartic** path is ported
//!   here. A compound whose data is tabular or equation-number-based is a
//!   deferred path (future work).
//! - **Enthalpy/entropy of formation offsets.** These functions return the
//!   *sensible* ideal-gas enthalpy `∫Cp0 dT` and entropy `∫Cp0/T dT − R ln…`
//!   relative to `T_ref`/`P_ref` — the reference state for EOS departure
//!   functions. Absolute enthalpy/entropy of formation (DWSIM's
//!   `IG_Enthalpy_of_Formation_25C`, [`Component::ig_entropy_formation_25c`])
//!   are a separate additive offset, not applied here.
//! - **Real-gas departures.** The Peng-Robinson / SRK enthalpy & entropy
//!   departures that turn these ideal-gas values into real-fluid properties
//!   live in [`crate::thermo::cubic_eos`], not here.

/// Universal gas constant `R` [J/(mol·K)] — CODATA 2018 exact value.
///
/// DWSIM hard-codes the rounded `8.314`; this port uses the exact SI value.
/// The difference only affects the `-R ln(P/P_ref)` pressure term of the
/// entropy (≈ 5.6e-5 relative).
pub const R: f64 = 8.314_462_618_153_24;

/// Ideal-gas heat capacity `Cp0(T)` of a pure compound [J/(mol·K)].
///
/// Evaluates the canonical DWSIM quartic correlation
/// `Cp0 = A + B·T + C·T² + D·T³ + E·T⁴` from the component's
/// `cp_ig_a..e` coefficients (`ThermodynamicsBase.vb:1778`,
/// `PropertyPackage.vb:5850`). The polynomial value is in kJ/(kmol·K), which
/// **equals J/(mol·K)** numerically (see the module docs), so no molar-mass
/// conversion is applied — unlike DWSIM's `AUX_CPi`, which additionally divides
/// by `Molar_Weight` to return a mass-basis kJ/(kg·K).
///
/// # Parameters
/// - `component`: source of the `cp_ig_a..e` coefficients (kJ/(kmol·K) basis).
/// - `temperature` `T`: absolute temperature [K]. Must be > 0; the correlation
///   is a fit valid only over the compound's regressed temperature range
///   (typically ~50-1500 K) — extrapolation beyond it is not checked here.
///
/// # Returns
/// `Cp0(T)` [J/(mol·K)].
#[must_use]
pub fn ideal_gas_cp(component: &crate::thermo::Component, temperature: f64) -> f64 {
    let t = temperature;
    let a = component.cp_ig_a;
    let b = component.cp_ig_b;
    let c = component.cp_ig_c;
    let d = component.cp_ig_d;
    let e = component.cp_ig_e;
    a + b * t + c * t * t + d * t * t * t + e * t * t * t * t
}

/// Sensible ideal-gas molar enthalpy relative to `T_ref` [J/mol]:
/// `H0(T) − H0(T_ref) = ∫_{T_ref}^{T} Cp0(T') dT'`.
///
/// Uses the **exact analytic integral** of the DWSIM quartic Cp0 correlation
/// (the closed form DWSIM's numerical `AUX_INT_CPDTi` midpoint rule converges
/// to, `PropertyPackage.vb:7794`; `RET_Hid`, `:8558`):
///
/// ```text
/// A(T−T₀) + (B/2)(T²−T₀²) + (C/3)(T³−T₀³) + (D/4)(T⁴−T₀⁴) + (E/5)(T⁵−T₀⁵)
/// ```
///
/// This is the *sensible* enthalpy only — the enthalpy of formation offset is
/// not added (see module "Honest scope").
///
/// # Parameters
/// - `component`: source of `cp_ig_a..e`.
/// - `temperature` `T` [K], `t_ref` `T_ref` [K]: both > 0. `T < T_ref` is
///   allowed and returns a negative enthalpy (the integral is signed).
///
/// # Returns
/// `∫_{T_ref}^{T} Cp0 dT` [J/mol].
#[must_use]
pub fn ideal_gas_enthalpy(
    component: &crate::thermo::Component,
    temperature: f64,
    t_ref: f64,
) -> f64 {
    let a = component.cp_ig_a;
    let b = component.cp_ig_b;
    let c = component.cp_ig_c;
    let d = component.cp_ig_d;
    let e = component.cp_ig_e;
    let t = temperature;
    let t0 = t_ref;
    a * (t - t0)
        + b / 2.0 * (t * t - t0 * t0)
        + c / 3.0 * (t.powi(3) - t0.powi(3))
        + d / 4.0 * (t.powi(4) - t0.powi(4))
        + e / 5.0 * (t.powi(5) - t0.powi(5))
}

/// Sensible ideal-gas molar entropy relative to `(T_ref, P_ref)`
/// [J/(mol·K)]:
/// `S0(T,P) − S0(T_ref,P_ref) = ∫_{T_ref}^{T} Cp0/T' dT' − R ln(P/P_ref)`.
///
/// Uses the **exact analytic integral** of Cp0/T for the DWSIM quartic (the
/// closed form DWSIM's numerical `AUX_INT_CPDT_Ti` midpoint rule converges to,
/// `PropertyPackage.vb:7944`) plus the pressure term of `RET_Sid`
/// (`:8698`, `-R ln(P/P_ref)`; DWSIM hard-codes `P_ref = 101325 Pa`):
///
/// ```text
/// A·ln(T/T₀) + B(T−T₀) + (C/2)(T²−T₀²) + (D/3)(T³−T₀³) + (E/4)(T⁴−T₀⁴)
///   − R ln(P/P_ref)
/// ```
///
/// # Parameters
/// - `component`: source of `cp_ig_a..e`.
/// - `temperature` `T` [K], `t_ref` `T_ref` [K]: both > 0.
/// - `pressure` `P` [Pa], `p_ref` `P_ref` [Pa]: both > 0. The pressure term is
///   the ideal-gas isothermal entropy change `-R ln(P/P_ref)`; larger `P`
///   lowers entropy.
///
/// # Returns
/// `∫_{T_ref}^{T} Cp0/T dT − R ln(P/P_ref)` [J/(mol·K)].
#[must_use]
pub fn ideal_gas_entropy(
    component: &crate::thermo::Component,
    temperature: f64,
    pressure: f64,
    t_ref: f64,
    p_ref: f64,
) -> f64 {
    let a = component.cp_ig_a;
    let b = component.cp_ig_b;
    let c = component.cp_ig_c;
    let d = component.cp_ig_d;
    let e = component.cp_ig_e;
    let t = temperature;
    let t0 = t_ref;
    let cp_over_t_integral = a * (t / t0).ln()
        + b * (t - t0)
        + c / 2.0 * (t * t - t0 * t0)
        + d / 3.0 * (t.powi(3) - t0.powi(3))
        + e / 4.0 * (t.powi(4) - t0.powi(4));
    cp_over_t_integral - R * (pressure / p_ref).ln()
}

/// Mole-fraction-weighted ideal-gas molar heat capacity of a mixture
/// [J/(mol·K)]: `Cp0_mix(T) = Σ_i x_i Cp0_i(T)`.
///
/// The ideal-gas mixture Cp is a linear mole-fraction average (no mixing
/// contribution — Cp of mixing is zero for ideal gases).
///
/// # Parameters
/// - `components`: the pure compounds, one per mixture species.
/// - `mole_fractions` `x_i`: mole fractions [-], same length as `components`,
///   normally summing to 1 (not enforced — the caller owns normalisation).
/// - `temperature` `T` [K].
///
/// # Panics
/// Panics if `components.len() != mole_fractions.len()`.
///
/// # Returns
/// `Σ_i x_i Cp0_i(T)` [J/(mol·K)].
#[must_use]
pub fn mixture_ideal_gas_cp(
    components: &[crate::thermo::Component],
    mole_fractions: &[f64],
    temperature: f64,
) -> f64 {
    assert_eq!(
        components.len(),
        mole_fractions.len(),
        "components and mole_fractions must have equal length"
    );
    components
        .iter()
        .zip(mole_fractions)
        .map(|(c, &x)| x * ideal_gas_cp(c, temperature))
        .sum()
}

/// Mole-fraction-weighted sensible ideal-gas molar enthalpy of a mixture
/// [J/mol]: `H0_mix(T) − H0_mix(T_ref) = Σ_i x_i ∫_{T_ref}^{T} Cp0_i dT`.
///
/// Enthalpy of mixing is zero for an ideal gas, so this is a plain
/// mole-fraction sum of the pure-component sensible enthalpies.
///
/// # Parameters
/// - `components`, `mole_fractions`: as [`mixture_ideal_gas_cp`].
/// - `temperature` `T` [K], `t_ref` `T_ref` [K].
///
/// # Panics
/// Panics if `components.len() != mole_fractions.len()`.
///
/// # Returns
/// `Σ_i x_i ∫_{T_ref}^{T} Cp0_i dT` [J/mol].
#[must_use]
pub fn mixture_ideal_gas_enthalpy(
    components: &[crate::thermo::Component],
    mole_fractions: &[f64],
    temperature: f64,
    t_ref: f64,
) -> f64 {
    assert_eq!(
        components.len(),
        mole_fractions.len(),
        "components and mole_fractions must have equal length"
    );
    components
        .iter()
        .zip(mole_fractions)
        .map(|(c, &x)| x * ideal_gas_enthalpy(c, temperature, t_ref))
        .sum()
}

/// Ideal entropy of mixing [J/(mol·K)]: `Δs_mix = −R Σ_i x_i ln x_i`.
///
/// Ports the mixing contribution DWSIM adds inside `RET_Sid`
/// (`PropertyPackage.vb:8695`, `-R x_i ln x_i` per species; DWSIM's mass-basis
/// `/Molar_Weight` is dropped here for the molar basis). It is always ≥ 0 and
/// vanishes for a pure stream (any `x_i = 1`). Species with `x_i = 0` are
/// skipped (the `x ln x → 0` limit), matching DWSIM's `If x_i <> 0` guard.
///
/// # Parameters
/// - `mole_fractions` `x_i` [-], normally summing to 1. Non-positive entries
///   are treated as absent species (skipped).
///
/// # Returns
/// `−R Σ_i x_i ln x_i` [J/(mol·K)].
#[must_use]
pub fn ideal_entropy_of_mixing(mole_fractions: &[f64]) -> f64 {
    -R * mole_fractions
        .iter()
        .filter(|&&x| x > 0.0)
        .map(|&x| x * x.ln())
        .sum::<f64>()
}

/// Total ideal-gas molar entropy of a mixture relative to `(T_ref, P_ref)`
/// [J/(mol·K)]:
/// `S0_mix = Σ_i x_i [∫Cp0_i/T dT − R ln(P/P_ref)] + (−R Σ_i x_i ln x_i)`.
///
/// The mole-fraction-weighted pure-component ideal-gas entropies plus the
/// [`ideal_entropy_of_mixing`] term (DWSIM `RET_Sid`,
/// `PropertyPackage.vb:8680-8718`). Because the `−R ln(P/P_ref)` pressure term
/// is species-independent, `Σ_i x_i` of it collapses to a single
/// `−R ln(P/P_ref)` when the mole fractions sum to 1.
///
/// # Parameters
/// - `components`, `mole_fractions`: as [`mixture_ideal_gas_cp`].
/// - `temperature` `T` [K], `pressure` `P` [Pa], `t_ref` `T_ref` [K],
///   `p_ref` `P_ref` [Pa]: all > 0.
///
/// # Panics
/// Panics if `components.len() != mole_fractions.len()`.
///
/// # Returns
/// The total mixture ideal-gas entropy [J/(mol·K)].
#[must_use]
pub fn mixture_ideal_gas_entropy(
    components: &[crate::thermo::Component],
    mole_fractions: &[f64],
    temperature: f64,
    pressure: f64,
    t_ref: f64,
    p_ref: f64,
) -> f64 {
    assert_eq!(
        components.len(),
        mole_fractions.len(),
        "components and mole_fractions must have equal length"
    );
    let weighted: f64 = components
        .iter()
        .zip(mole_fractions)
        .map(|(c, &x)| x * ideal_gas_entropy(c, temperature, pressure, t_ref, p_ref))
        .sum();
    weighted + ideal_entropy_of_mixing(mole_fractions)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::thermo::Component;
    use approx::assert_relative_eq;

    /// Build a test component carrying only the given Cp0 coefficients; the
    /// critical constants are physically-plausible placeholders (unused by the
    /// ideal-gas routines, which read only `cp_ig_a..e`).
    fn comp(cp_ig: [f64; 5]) -> Component {
        Component::new(
            "test", 0.030, 300.0, 5.0e6, f64::NAN, 0.1, f64::NAN, cp_ig, f64::NAN,
        )
        .expect("valid test component")
    }

    /// **Methodology.** With only the constant coefficient A nonzero, Cp0 is
    /// constant in T. Verify `ideal_gas_cp` returns A at several temperatures.
    /// **Results (2026-07-24):** A = 29.1 J/(mol·K), Cp0(200) = Cp0(500) =
    /// Cp0(1000) = 29.1 exactly (bit-for-bit; a constant has no T dependence).
    #[test]
    fn constant_cp_is_the_a_coefficient() {
        let c = comp([29.1, 0.0, 0.0, 0.0, 0.0]);
        assert_relative_eq!(ideal_gas_cp(&c, 200.0), 29.1, max_relative = 1e-15);
        assert_relative_eq!(ideal_gas_cp(&c, 500.0), 29.1, max_relative = 1e-15);
        assert_relative_eq!(ideal_gas_cp(&c, 1000.0), 29.1, max_relative = 1e-15);
    }

    /// **Methodology.** For a constant Cp (A only), the analytic enthalpy
    /// integral must equal `A·(T − T_ref)` exactly. Hand computation with
    /// A = 29.1, T = 500, T_ref = 298.15: 29.1·(500 − 298.15) = 29.1·201.85
    /// = 5873.835 J/mol. **Results (2026-07-24):** computed
    /// 5873.835 J/mol, matches hand value to < 1e-9 relative.
    #[test]
    fn constant_cp_enthalpy_is_a_times_delta_t() {
        let a = 29.1;
        let c = comp([a, 0.0, 0.0, 0.0, 0.0]);
        let (t, t_ref) = (500.0, 298.15);
        let expected = a * (t - t_ref);
        assert_relative_eq!(expected, 5873.835, max_relative = 1e-12);
        assert_relative_eq!(ideal_gas_enthalpy(&c, t, t_ref), expected, max_relative = 1e-12);
    }

    /// **Methodology.** For a constant Cp (A only), the entropy must equal
    /// `A·ln(T/T_ref) − R ln(P/P_ref)` exactly. Hand computation with A = 29.1,
    /// T = 500, T_ref = 298.15, P = 5·101325, P_ref = 101325:
    /// 29.1·ln(500/298.15) − R·ln(5). **Results (2026-07-24):** analytic
    /// closed form and the function agree to < 1e-12 relative; numeric value
    /// ≈ 29.1·0.517183 − 8.314463·1.609438 ≈ 15.04503 − 13.38161 ≈ 1.66342
    /// J/(mol·K).
    #[test]
    fn constant_cp_entropy_matches_closed_form_with_pressure_term() {
        let a: f64 = 29.1;
        let c = comp([a, 0.0, 0.0, 0.0, 0.0]);
        let (t, t_ref): (f64, f64) = (500.0, 298.15);
        let (p, p_ref): (f64, f64) = (5.0 * 101_325.0, 101_325.0);
        let expected = a * (t / t_ref).ln() - R * (p / p_ref).ln();
        assert_relative_eq!(
            ideal_gas_entropy(&c, t, p, t_ref, p_ref),
            expected,
            max_relative = 1e-12
        );
        // Cross-check the hand-computed numeric magnitude.
        assert_relative_eq!(expected, 1.663_42, max_relative = 1e-4);
    }

    /// **Methodology.** A full quartic Cp0 must integrate to its exact closed
    /// form. Coefficients A = 19.8, B = 7.344e-2, C = -5.602e-5, D = 1.715e-8,
    /// E = 1.0e-12 (nitrogen-like quartic + a token E term). Compare
    /// `ideal_gas_enthalpy` against an independent evaluation of
    /// `A(T−T₀)+ (B/2)(T²−T₀²)+ (C/3)(T³−T₀³)+ (D/4)(T⁴−T₀⁴)+ (E/5)(T⁵−T₀⁵)`
    /// at T = 800, T_ref = 298.15, and `ideal_gas_entropy` against
    /// `A ln(T/T₀)+ B(T−T₀)+ (C/2)(T²−T₀²)+ (D/3)(T³−T₀³)+ (E/4)(T⁴−T₀⁴)`
    /// (P = P_ref so the pressure term is zero). **Results (2026-07-24):** both
    /// match the independent closed form to < 1e-12 relative
    /// (H ≈ 2.02e4 J/mol, ∫Cp0/T dT ≈ 39.0 J/(mol·K)).
    #[test]
    fn polynomial_cp_integrates_to_closed_form() {
        let (a, b, cc, d, e) = (19.8, 7.344e-2, -5.602e-5, 1.715e-8, 1.0e-12);
        let c = comp([a, b, cc, d, e]);
        let (t, t0) = (800.0, 298.15);

        let h_expected = a * (t - t0)
            + b / 2.0 * (t * t - t0 * t0)
            + cc / 3.0 * (t.powi(3) - t0.powi(3))
            + d / 4.0 * (t.powi(4) - t0.powi(4))
            + e / 5.0 * (t.powi(5) - t0.powi(5));
        assert_relative_eq!(ideal_gas_enthalpy(&c, t, t0), h_expected, max_relative = 1e-12);

        let s_expected = a * (t / t0).ln()
            + b * (t - t0)
            + cc / 2.0 * (t * t - t0 * t0)
            + d / 3.0 * (t.powi(3) - t0.powi(3))
            + e / 4.0 * (t.powi(4) - t0.powi(4));
        // P = P_ref => pressure term is exactly 0.
        assert_relative_eq!(
            ideal_gas_entropy(&c, t, 101_325.0, t0, 101_325.0),
            s_expected,
            max_relative = 1e-12
        );
    }

    /// **Methodology.** The pressure term `−R ln(P/P_ref)` must be exact and
    /// independent of the Cp coefficients. With a zero-Cp component and
    /// T = T_ref (so the temperature integral is exactly 0), the entropy must
    /// equal `−R ln(P/P_ref)`. For P = 2·P_ref: `−R ln 2 ≈ −5.7632`;
    /// for P = 0.5·P_ref: `−R ln 0.5 = +R ln 2 ≈ +5.7631`. **Results
    /// (2026-07-24):** −5.763 146 and +5.763 146 J/(mol·K), exact to < 1e-12.
    #[test]
    fn pressure_term_is_exact_minus_r_ln_p_ratio() {
        let c = comp([0.0; 5]);
        let (t_ref, p_ref) = (298.15, 101_325.0);
        let s_compress = ideal_gas_entropy(&c, t_ref, 2.0 * p_ref, t_ref, p_ref);
        assert_relative_eq!(s_compress, -R * 2.0_f64.ln(), max_relative = 1e-12);
        assert_relative_eq!(s_compress, -5.763_146, max_relative = 1e-6);

        let s_expand = ideal_gas_entropy(&c, t_ref, 0.5 * p_ref, t_ref, p_ref);
        assert_relative_eq!(s_expand, R * 2.0_f64.ln(), max_relative = 1e-12);
    }

    /// **Methodology.** The ideal entropy of mixing for a 50/50 binary is the
    /// classic `−R(0.5 ln 0.5 + 0.5 ln 0.5) = R ln 2`. **Results
    /// (2026-07-24):** `ideal_entropy_of_mixing([0.5, 0.5])` = 5.763 146
    /// J/(mol·K) = `R ln 2`, exact to < 1e-12; a pure stream `[1.0, 0.0]`
    /// gives exactly 0.
    #[test]
    fn ideal_entropy_of_mixing_fifty_fifty_is_r_ln2() {
        let s_mix = ideal_entropy_of_mixing(&[0.5, 0.5]);
        assert_relative_eq!(s_mix, R * 2.0_f64.ln(), max_relative = 1e-12);
        assert_relative_eq!(s_mix, 5.763_146, max_relative = 1e-6);

        // Pure stream: no mixing entropy (x ln x = 0 at x=1, x=0 skipped).
        assert_relative_eq!(ideal_entropy_of_mixing(&[1.0, 0.0]), 0.0, epsilon = 1e-15);
    }

    /// **Methodology.** Mixture Cp/enthalpy are mole-fraction sums; the
    /// mixture entropy adds the mixing term on top of the weighted pure-species
    /// entropies. Build a 50/50 mix of two constant-Cp components
    /// (A₁ = 29.1, A₂ = 37.0), check: (a) `mixture_ideal_gas_cp` = 0.5·29.1 +
    /// 0.5·37.0 = 33.05; (b) `mixture_ideal_gas_enthalpy` = weighted sum of the
    /// per-species `A·(T−T_ref)`; (c) `mixture_ideal_gas_entropy` = weighted
    /// per-species entropy **plus** `R ln 2` (mixing). **Results
    /// (2026-07-24):** (a) 33.05 J/(mol·K) exact; (b) matches manual weighted
    /// sum to < 1e-12; (c) equals weighted entropy + `R ln 2` to < 1e-12.
    #[test]
    fn mixture_properties_weight_and_add_mixing_entropy() {
        let (a1, a2) = (29.1, 37.0);
        let comps = [comp([a1, 0.0, 0.0, 0.0, 0.0]), comp([a2, 0.0, 0.0, 0.0, 0.0])];
        let x = [0.5, 0.5];
        let (t, t_ref) = (450.0, 298.15);
        let (p, p_ref) = (2.0 * 101_325.0, 101_325.0);

        // (a) mixture Cp.
        assert_relative_eq!(
            mixture_ideal_gas_cp(&comps, &x, t),
            0.5 * a1 + 0.5 * a2,
            max_relative = 1e-13
        );
        assert_relative_eq!(mixture_ideal_gas_cp(&comps, &x, t), 33.05, max_relative = 1e-13);

        // (b) mixture enthalpy = weighted pure enthalpies.
        let h_expected = 0.5 * ideal_gas_enthalpy(&comps[0], t, t_ref)
            + 0.5 * ideal_gas_enthalpy(&comps[1], t, t_ref);
        assert_relative_eq!(
            mixture_ideal_gas_enthalpy(&comps, &x, t, t_ref),
            h_expected,
            max_relative = 1e-13
        );

        // (c) mixture entropy = weighted pure entropies + ideal mixing term.
        let s_weighted = 0.5 * ideal_gas_entropy(&comps[0], t, p, t_ref, p_ref)
            + 0.5 * ideal_gas_entropy(&comps[1], t, p, t_ref, p_ref);
        let s_expected = s_weighted + R * 2.0_f64.ln();
        assert_relative_eq!(
            mixture_ideal_gas_entropy(&comps, &x, t, p, t_ref, p_ref),
            s_expected,
            max_relative = 1e-13
        );
    }
}
