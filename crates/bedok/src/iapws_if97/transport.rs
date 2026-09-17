//! Transport properties — viscosity and thermal conductivity.
//!
//! # Provenance — third-party, BSD-2-Clause
//!
//! As [`crate::iapws_if97`]: translated from `IAPWS_IF97.m` by Mark Mikofski,
//! Copyright (c) 2013, BSD-2-Clause, terms reproduced in the crate `NOTICE`.
//! Source functions `mu_pT`, `k_pT`, `pB23_T`.
//!
//! # These are separate IAPWS releases, not part of IF97 itself
//!
//! IF97 is a formulation for the *thermodynamic* properties. Viscosity and
//! thermal conductivity come from their own releases, which IF97 supplies the
//! density for:
//!
//! - **Viscosity** — IAPWS Formulation 2008 for the Viscosity of Ordinary
//!   Water Substance.
//! - **Thermal conductivity** — Revised Release on the IAPWS Formulation 1985
//!   for the Thermal Conductivity of Ordinary Water Substance (2008 revision).
//!
//! Both are written in terms of **reduced density and temperature**, so the
//! kernels here take `(rho, T)` and the `(p, T)` wrappers get `rho` from the
//! IF97 region equations. That split is not in the reference — it inlines the
//! kernel three times, once per region — but it matters for verification: the
//! published check tables are stated at given `(rho, T)`, so the kernel can be
//! checked against them *exactly*, which a `(p, T)` entry point cannot be.
//!
//! # The critical enhancement is absent, and that is the reference's choice
//!
//! The viscosity release defines `mu = mu0 * mu1 * mu2`, where `mu2` is a
//! critical-region enhancement that rises steeply near 647 K and 322 kg/m³.
//! The reference computes only `mu0 * mu1`, the "industrial" form the release
//! permits outside that region. Away from the critical point `mu2` is 1, so
//! this is exact for reactor conditions; within roughly 10 K and 100 kg/m³ of
//! critical it under-predicts. Preserved as written.
//!
//! # Units
//!
//! Pressure MPa, temperature K, density kg/m³, viscosity **Pa·s**, thermal
//! conductivity **W/(m·K)**. Note both are SI here, where the BEDOK callers
//! work in cm-g-s and convert at the call site.

use super::basic::{v1_pt, v2_pt};
use super::region4::psat_t;

/// `p = pB23_T(T)` — pressure on the region 2 / region 3 boundary, MPa, from
/// temperature in K.
///
/// The inverse of [`super::backward::tb23_p`], and a plain quadratic where that
/// one is a square root. Both are needed: the transport functions select their
/// region by pressure at a given temperature.
pub fn pb23_t(t: f64) -> f64 {
    const N: [f64; 3] = [
        0.348_051_856_289_69e3,
        -0.116_718_598_799_75e1,
        0.101_929_700_393_26e-2,
    ];
    N[0] + t * (N[1] + t * N[2])
}

/// Lower temperature bound for both correlations, K — the triple point.
pub const T_MIN: f64 = 273.16;
/// Region 1/3 boundary temperature, K.
pub const T_B13: f64 = 623.15;
/// Region 2/3 boundary temperature, K, as the transport releases use it.
///
/// Note this is **863.15 K**, not the 623.15 K that bounds region 1 — the
/// transport releases carry their own region map.
pub const T_B23: f64 = 863.15;
/// Upper temperature bound, K.
pub const T_MAX: f64 = 1073.15;
/// Upper pressure bound, MPa.
pub const P_MAX: f64 = 100.0;

/// `mu = mu0(T) * mu1(rho, T)` — dynamic viscosity, **Pa·s**, from density and
/// temperature.
///
/// # Arguments
///
/// - `rho` — density, **kg/m³**.
/// - `t` — temperature, **K**.
///
/// # Returns
///
/// Dynamic viscosity in **Pa·s**. Multiply by 1e6 for the µPa·s the published
/// tables use.
///
/// # What this omits
///
/// The critical enhancement `mu2`; see the module docs.
pub fn mu_rho_t(rho: f64, t: f64) -> f64 {
    const TC: f64 = 647.096;
    const RHOC: f64 = 322.0;
    const MUSTAR: f64 = 1.00e-6;
    const HI: [f64; 4] = [1.67752, 2.20462, 0.636_656_4, -0.241_605];
    // (I, J, H) — the 21 residual terms.
    const I: [i32; 21] = [0, 1, 2, 3, 0, 1, 2, 3, 5, 0, 1, 2, 3, 4, 0, 1, 0, 3, 4, 3, 5];
    const J: [i32; 21] = [0, 0, 0, 0, 1, 1, 1, 1, 1, 2, 2, 2, 2, 2, 3, 3, 4, 4, 5, 6, 6];
    const H: [f64; 21] = [
        0.520_094,
        0.085_089_5,
        -1.083_74,
        -0.289_555,
        0.222_531,
        0.999_115,
        1.887_97,
        1.266_13,
        0.120_573,
        -0.281_378,
        -0.906_851,
        -0.772_479,
        -0.489_837,
        -0.257_040,
        0.161_913,
        0.257_399,
        -0.032_537_2,
        0.069_845_2,
        0.008_721_02,
        -0.004_356_73,
        -0.000_593_264,
    ];

    let tbar = t / TC;
    let rhobar = rho / RHOC;

    // `mu0 = 100*sqrt(Tbar) / sum(Hi / Tbar^i)`, Horner as the reference writes it.
    let mu0 = 100.0 * tbar.sqrt() / (HI[0] + (HI[1] + (HI[2] + HI[3] / tbar) / tbar) / tbar);

    let sum: f64 = (0..21)
        .map(|k| rhobar * (1.0 / tbar - 1.0).powi(I[k]) * H[k] * (rhobar - 1.0).powi(J[k]))
        .sum();
    let mu1 = sum.exp();

    mu0 * mu1 * MUSTAR
}

/// `k = k0 + k1 + k2` — thermal conductivity, **W/(m·K)**, from density and
/// temperature.
///
/// # Arguments
///
/// - `rho` — density, **kg/m³**.
/// - `t` — temperature, **K**.
///
/// # The reducing constants are not the critical constants
///
/// This release reduces by `Tstar = 647.26 K` and `rhostar = 317.7 kg/m³`,
/// which are the 1985 formulation's own values and differ slightly from the
/// critical point (647.096 K, 322.0 kg/m³) that the viscosity release uses.
/// Mixing the two would be a small but systematic error, so they are declared
/// separately here rather than shared.
///
/// # The `S` term branches on temperature
///
/// `S` is `1/deltaTbar` at or above the reducing temperature and
/// `C6/deltaTbar^0.6` below it. The reference writes this as a sum of two
/// logical-mask products, which is the MATLAB idiom for a branch; it is an
/// `if` here.
pub fn k_rho_t(rho: f64, t: f64) -> f64 {
    const TSTAR: f64 = 647.26;
    const RHOSTAR: f64 = 317.7;
    const A: [f64; 4] = [0.010_281_1, 0.029_962_1, 0.015_614_6, -0.004_224_64];
    const B: [f64; 3] = [-0.397_070, 0.400_302, 1.060_000];
    const BB: [f64; 2] = [-0.171_587, 2.392_190];
    const D: [f64; 4] = [0.070_130_9, 0.011_852_0, 0.001_699_37, -1.020_0];
    const C: [f64; 6] = [
        0.642_857,
        -4.117_17,
        -6.179_37,
        0.003_089_76,
        0.082_299_4,
        10.093_2,
    ];

    let tbar = t / TSTAR;
    let rhobar = rho / RHOSTAR;

    let k0 = tbar.sqrt() * (A[0] + (A[1] + (A[2] + A[3] * tbar) * tbar) * tbar);
    let k1 = B[0] + B[1] * rhobar + B[2] * (BB[0] * (rhobar + BB[1]).powi(2)).exp();

    let delta_tbar = (tbar - 1.0).abs() + C[3];
    let q = 2.0 + C[4] / delta_tbar.powf(0.6);
    let s = if tbar >= 1.0 {
        1.0 / delta_tbar
    } else {
        C[5] / delta_tbar.powf(0.6)
    };

    let k2 = (D[0] / tbar.powi(10) + D[1]) * rhobar.powf(1.8) * (C[0] * (1.0 - rhobar.powf(2.8))).exp()
        + D[2] * s * rhobar.powf(q) * (q / (1.0 + q) * (1.0 - rhobar.powf(1.0 + q))).exp()
        + D[3] * (C[1] * tbar.powf(1.5) + C[2] / rhobar.powi(5)).exp();

    k0 + k1 + k2
}

/// Which IF97 region a `(p, T)` state falls in, for the transport releases'
/// own region map.
///
/// Returns `None` outside the validity envelope, where the reference leaves its
/// `NaN` initialisation in place.
fn region(p: f64, t: f64) -> Option<u8> {
    let p_min = psat_t(T_MIN);
    if !(T_MIN..=T_MAX).contains(&t) || p > P_MAX || p < p_min {
        return None;
    }
    let psat = psat_t(t);
    let pb23 = if (T_B13..=T_B23).contains(&t) {
        Some(pb23_t(t))
    } else {
        None
    };

    // Region 1: at or above saturation, up to the 1/3 boundary temperature.
    if t <= T_B13 && p >= psat {
        return Some(1);
    }
    // Region 2: below saturation, or below the 2/3 boundary above it.
    if (t <= T_B13 && p <= psat)
        || (t > T_B13 && t <= T_B23 && pb23.is_some_and(|b| p <= b))
        || (t > T_B23 && p <= P_MAX)
    {
        return Some(2);
    }
    if pb23.is_some_and(|b| p > b) && t > T_B13 && t < T_B23 {
        return Some(3);
    }
    None
}

/// `mu = mu_pT(p, T)` — dynamic viscosity, **Pa·s**, from pressure and
/// temperature.
///
/// # Arguments
///
/// - `p` — pressure, **MPa**, up to 100 MPa.
/// - `t` — temperature, **K**, from the triple point to 1073.15 K.
///
/// # Returns
///
/// Viscosity in **Pa·s**, or `NaN` outside the validity envelope, or in region
/// 3 — see below.
///
/// # Region 3 returns `NaN`
///
/// The reference gets region 3's density from `v_pT`, which dispatches into the
/// region-3 backward equations. Those are not translated, so this returns `NaN`
/// there rather than a wrong number — the same principled gap
/// [`super::basic::hl_p`] and [`super::backward::t_ph`] carry. Region 3 is
/// above 623.15 K and above the 2/3 boundary pressure; BEDOK's coolant does not
/// go there.
pub fn mu_pt(p: f64, t: f64) -> f64 {
    match region(p, t) {
        Some(1) => mu_rho_t(1.0 / v1_pt(p, t), t),
        Some(2) => mu_rho_t(1.0 / v2_pt(p, t), t),
        _ => f64::NAN,
    }
}

/// `k = k_pT(p, T)` — thermal conductivity, **W/(m·K)**, from pressure and
/// temperature.
///
/// Same arguments, envelope and region-3 gap as [`mu_pt`].
pub fn k_pt(p: f64, t: f64) -> f64 {
    match region(p, t) {
        Some(1) => k_rho_t(1.0 / v1_pt(p, t), t),
        Some(2) => k_rho_t(1.0 / v2_pt(p, t), t),
        _ => f64::NAN,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The 2008 viscosity release's own check table, at given `(rho, T)`.
    ///
    /// # Methodology
    ///
    /// The release tabulates `mu` for the industrial form `mu0 * mu1` — without
    /// the critical enhancement — at states specified by **density and
    /// temperature**, which is exactly what [`mu_rho_t`] takes. Testing the
    /// kernel directly rather than through [`mu_pt`] is what makes an exact
    /// comparison possible: routing through `(p, T)` would fold in the IF97
    /// density's own error and turn a check of this correlation into a check of
    /// two things at once.
    ///
    /// Values are in µPa·s, so the computed Pa·s is scaled by 1e6.
    ///
    /// **Pass criterion: half the last printed digit, absolutely.** Every
    /// published value is quoted to six decimal places, so the tightest
    /// meaningful check is `|computed - published| <= 5e-7 µPa·s`. A blanket
    /// *relative* tolerance is the wrong tool here and the first version of
    /// this test got it wrong: at 1e-8 relative it failed on
    /// `14.538324 µPa·s`, whose own printed half-ulp is already 3.4e-8
    /// relative — the criterion was tighter than the reference value's
    /// precision. Six decimals on a 4-digit number is simply fewer significant
    /// figures than on a 10-digit one, and an absolute tolerance handles both
    /// without pretending otherwise.
    ///
    /// # Results — measured 2026-08-17
    ///
    /// All eleven states agree, worst absolute deviation **4.858e-7 µPa·s** —
    /// just inside the 5e-7 criterion, i.e. **the computed values round to the
    /// published digits**.
    ///
    /// **Interpretation.** Sitting right at the printed precision rather than
    /// orders below it is the expected signature: the residual is the
    /// published values' own rounding, not error in this implementation. The
    /// 21-term residual sum and the four-term `mu0` denominator are verified
    /// against the standard across four decades of density (1 to 1200 kg/m³)
    /// and 875 K of temperature.
    #[test]
    fn viscosity_matches_the_published_check_values() {
        // (T [K], rho [kg/m^3], mu [uPa*s])
        let cases = [
            (298.15, 998.0, 889.735_100),
            (298.15, 1200.0, 1_437.649_467),
            (373.15, 1000.0, 307.883_622),
            (433.15, 1.0, 14.538_324),
            (433.15, 1000.0, 217.685_358),
            (873.15, 1.0, 32.619_287),
            (873.15, 100.0, 35.802_262),
            (873.15, 600.0, 77.430_195),
            (1173.15, 1.0, 44.217_245),
            (1173.15, 100.0, 47.640_433),
            (1173.15, 400.0, 64.154_608),
        ];
        // Half of the last printed digit: the values carry six decimals.
        const TOL: f64 = 5e-7;
        let mut worst: f64 = 0.0;
        for (t, rho, mu_ref) in cases {
            let mu = mu_rho_t(rho, t) * 1e6;
            let err = (mu - mu_ref).abs();
            worst = worst.max(err);
            eprintln!(
                "mu({t} K, {rho} kg/m3) = {mu:.6} uPa.s, expected {mu_ref}, abs_err {err:.2e}"
            );
            assert!(
                err <= TOL,
                "mu at ({t}, {rho}): got {mu}, want {mu_ref} (abs_err {err:.3e} > {TOL:.0e})"
            );
        }
        eprintln!("viscosity: worst absolute deviation {worst:.3e} uPa.s");
    }

    /// Viscosity at ordinary conditions through the `(p, T)` entry point —
    /// and the liquid/vapour switch that sits right at the boiling point.
    ///
    /// # Methodology
    ///
    /// This checks the **wrapper** — the region selection and the IF97 density
    /// — rather than the correlation, which the table test above verifies
    /// exactly. Reference values are the familiar textbook ones: about
    /// **1002 µPa·s** for liquid water at 20 °C, **~282 µPa·s** for liquid near
    /// 99 °C, and **~12 µPa·s** for atmospheric steam at 100 °C.
    ///
    /// **The liquid case at "100 °C" is taken at 372.0 K, not 373.15 K, and
    /// that is not a fudge.** At 1 atm water saturates at `Tsat = 373.1243 K`,
    /// so 373.15 K is 0.026 K *above* the saturation line and is vapour. The
    /// first version of this test asked for liquid viscosity at 373.15 K and
    /// got 12.2 µPa·s — the region selector was right and the test was wrong.
    /// Rather than just move the temperature, the vapour point is now asserted
    /// too, which turns the mistake into a check that the region switch fires
    /// where the saturation line says it should.
    ///
    /// Pass criteria: 1% relative at 20 °C, **2% at 372 K**, 3% for the vapour
    /// point. The 2% is not slack — 372 K is **98.85 °C**, and the 282 µPa·s
    /// figure is quoted at 100 °C. Water's viscosity falls about
    /// 2.6 µPa·s/K there, so 1.15 K of offset is worth ~3 µPa·s, or 1.1% —
    /// just outside a 1% band, for reasons that have nothing to do with the
    /// implementation. The band is set to cover the offset rather than the
    /// reference value being moved to something unverifiable.
    ///
    /// These tolerances come from the precision of recalled figures, not the
    /// formulation, and are ample to catch a density in the wrong units — a
    /// g/cm³ against kg/m³ slip moves the answer by orders of magnitude.
    ///
    /// # Results — measured 2026-08-17
    ///
    /// `Tsat(0.101325 MPa) = 373.1243 K`. Viscosities: **1001.597 µPa·s** at
    /// 20 °C (0.04% from 1002), **285.021 µPa·s** at 372 K (1.07% from the
    /// 100 °C figure of 282, as the temperature offset predicts), and
    /// **12.232 µPa·s** as vapour at 373.15 K (0.6% from 12.3).
    ///
    /// **Interpretation.** The liquid-to-vapour drop across 1.15 K is a factor
    /// of **23.3**, and it falls exactly where `Tsat` says it should. The
    /// region selector, the saturation line and the two IF97 density branches
    /// are therefore consistent with each other — which is what this test
    /// exists to check, the correlation itself being verified exactly against
    /// the published table above.
    #[test]
    fn viscosity_through_the_pt_entry_point_switches_phase_at_saturation() {
        use super::super::region4::tsat_p;
        let p = 0.101_325;
        let tsat = tsat_p(p);

        let liquid_20c = mu_pt(p, 293.15) * 1e6;
        let liquid_99c = mu_pt(p, 372.0) * 1e6;
        let vapour_100c = mu_pt(p, 373.15) * 1e6;
        eprintln!(
            "Tsat({p} MPa) = {tsat:.4} K | mu: liquid 20 C = {liquid_20c:.3}, liquid 372 K = {liquid_99c:.3}, vapour 373.15 K = {vapour_100c:.3} uPa.s"
        );

        // 372.0 K is below saturation, 373.15 K above it.
        assert!(372.0 < tsat && tsat < 373.15, "Tsat = {tsat}");

        assert!(
            (liquid_20c - 1002.0).abs() / 1002.0 < 0.01,
            "20 C liquid: got {liquid_20c}"
        );
        assert!(
            (liquid_99c - 282.0).abs() / 282.0 < 0.02,
            "372 K liquid: got {liquid_99c}"
        );
        assert!(
            (vapour_100c - 12.3).abs() / 12.3 < 0.03,
            "373.15 K vapour: got {vapour_100c}"
        );

        // The phase change is a ~23x drop across 1.15 K.
        assert!(
            liquid_99c > 20.0 * vapour_100c,
            "the liquid/vapour switch did not fire: {liquid_99c} vs {vapour_100c}"
        );
    }

    /// Thermal conductivity of liquid water and steam at ordinary conditions.
    ///
    /// # Methodology
    ///
    /// The 1985 conductivity release's check table is not reproduced here — no
    /// tabulated values were available to transcribe — so this is a
    /// **magnitude and shape check**, not a verification against the standard,
    /// and it is labelled as such deliberately. Reference figures are the
    /// familiar ones: liquid water about **0.598 W/(m·K) at 20 °C** rising to
    /// **0.679 at 100 °C**, and atmospheric steam about **0.025 W/(m·K)** at
    /// 400 K.
    ///
    /// Pass criterion 2% relative. That catches a mistranscribed coefficient
    /// large enough to matter and a units error, but it does **not** establish
    /// agreement with the release at its stated precision. Until the check
    /// table is transcribed, `k_pT` should be treated as less well verified
    /// than [`mu_rho_t`], which is checked exactly.
    ///
    /// The liquid rising and the vapour being ~25x lower are both physically
    /// required, and are asserted separately so a coefficient error that
    /// preserved magnitude but broke the trend would still fail.
    ///
    /// # Results — measured 2026-08-17
    ///
    /// Liquid **0.59953 W/(m·K)** at 20 °C (0.3% from 0.598) rising to
    /// **0.67728** at 372 K (0.2% from 0.679); steam **0.02685** at 400 K
    /// (0.9% from 0.0271). The liquid/vapour ratio is 22.3.
    ///
    /// **Interpretation.** Magnitude and trend are both right, which rules out
    /// a units error and a grossly mistranscribed coefficient. It does **not**
    /// establish agreement with the 1985 release at its stated precision —
    /// `k_pT` remains the least-verified function in this module, and should be
    /// treated as such until the release's check table is transcribed.
    #[test]
    fn thermal_conductivity_is_the_right_magnitude_and_trend() {
        let liq_20c = k_pt(0.101_325, 293.15);
        let liq_100c = k_pt(0.101_325, 372.0);
        let vap_400k = k_pt(0.101_325, 400.0);
        eprintln!(
            "k: liquid 20 C = {liq_20c:.5}, liquid ~99 C = {liq_100c:.5}, steam 400 K = {vap_400k:.5} W/m/K"
        );

        assert!((liq_20c - 0.598).abs() / 0.598 < 0.02, "20 C: {liq_20c}");
        assert!((liq_100c - 0.679).abs() / 0.679 < 0.02, "100 C: {liq_100c}");
        assert!((vap_400k - 0.0271).abs() / 0.0271 < 0.02, "steam: {vap_400k}");

        assert!(liq_100c > liq_20c, "liquid conductivity must rise with T");
        assert!(vap_400k < liq_20c / 10.0, "steam must be far less conductive");
    }

    /// The two BEDOK operating points give plausible coolant transport
    /// properties.
    ///
    /// # Methodology
    ///
    /// A PWR at 15.5 MPa and 580 K, and a BWR at 7 MPa and 550 K — both
    /// compressed liquid, both region 1. Hot pressurised water is far less
    /// viscous than at room temperature (roughly 90 µPa·s against 1000) and
    /// somewhat less conductive than at its peak near 130 °C.
    ///
    /// Pass criterion: viscosity in 70-120 µPa·s and conductivity in
    /// 0.45-0.70 W/(m·K), bands wide enough to be a sanity check on the region
    /// selection rather than an accuracy claim.
    ///
    /// # Results — measured 2026-08-17
    ///
    /// PWR (15.5 MPa, 580 K): **85.812 µPa·s**, **0.54673 W/(m·K)**.
    /// BWR (7 MPa, 550 K): **95.099 µPa·s**, **0.58395 W/(m·K)**.
    ///
    /// **Interpretation.** Both sit in region 1 and both are physically
    /// sensible: hot pressurised water is ~12x less viscous than at room
    /// temperature, and the hotter PWR coolant is both thinner and less
    /// conductive than the BWR's, conductivity having peaked near 130 °C and
    /// fallen away since. A units or region error would not preserve that
    /// ordering.
    #[test]
    fn the_benchmark_coolant_states_are_plausible() {
        for (label, p, t) in [("PWR", 15.5, 580.0), ("BWR", 7.0, 550.0)] {
            let mu = mu_pt(p, t) * 1e6;
            let k = k_pt(p, t);
            eprintln!("{label} ({p} MPa, {t} K): mu = {mu:.3} uPa.s, k = {k:.5} W/m/K");
            assert!((70.0..120.0).contains(&mu), "{label} viscosity {mu}");
            assert!((0.45..0.70).contains(&k), "{label} conductivity {k}");
        }
    }

    /// Region 3 is `NaN`, loudly, because its density equations are not
    /// translated.
    #[test]
    fn region_three_is_nan_pending_the_region_three_equations() {
        // Above 623.15 K and above the 2/3 boundary pressure.
        let t = 700.0;
        let p = pb23_t(t) + 5.0;
        assert!(mu_pt(p, t).is_nan(), "mu should be NaN in region 3");
        assert!(k_pt(p, t).is_nan(), "k should be NaN in region 3");
        // Outside the envelope entirely.
        assert!(mu_pt(0.1, 1200.0).is_nan(), "above Tmax");
        assert!(k_pt(200.0, 400.0).is_nan(), "above pmax");
    }

    /// `pB23_T` and `TB23_p` invert each other.
    ///
    /// # Methodology
    ///
    /// The two are separate transcriptions of the same boundary — a quadratic
    /// and a square root over the same five coefficients — so round-tripping
    /// one through the other is an independent check on both.
    ///
    /// Pass criterion 1e-9 relative over the boundary's temperature range.
    ///
    /// # Results — measured 2026-08-17
    ///
    /// Worst round-trip error **2.616e-13** relative — machine precision.
    /// Both transcriptions of the boundary are therefore consistent.
    #[test]
    fn the_region_23_boundary_inverts() {
        use super::super::backward::tb23_p;
        let mut worst: f64 = 0.0;
        let mut t = 623.15;
        while t <= 863.15 {
            let back = tb23_p(pb23_t(t));
            worst = worst.max((back - t).abs() / t);
            t += 20.0;
        }
        eprintln!("pB23_T/TB23_p round trip: worst rel_err = {worst:.3e}");
        assert!(worst < 1e-9, "worst {worst}");
    }
}
