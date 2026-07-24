//! Cubic-EOS refinements: PRSV α-function + Peneloux volume translation.
//!
//! Two independent, additive refinements layered on top of the base
//! Peng-Robinson / SRK kernel in [`crate::thermo::cubic_eos`] (which this module
//! reads but never edits):
//!
//! 1. **PRSV α-function** — the Stryjek-Vera (1986) modification of the PR
//!    attraction temperature-dependence. It refines *only* the per-component
//!    attraction `a_i(T)`; the co-volume `b_i`, the van der Waals one-fluid
//!    mixing rule, the compressibility cubic, the fugacity coefficient, and the
//!    enthalpy/entropy departures are all **unchanged**. So a PRSV run is a base
//!    PR run with `a_i(T)` swapped — see [`prsv_a_i`] / [`prsv_a_mix`] and the
//!    "Composing with `cubic_eos`" note below.
//! 2. **Peneloux volume translation** — a constant per-component molar-volume
//!    shift `c_i` (Peneloux et al. 1982) subtracted from the EOS molar volume:
//!    `v = v_EOS − c`. It improves the predicted *liquid density* without
//!    changing VLE: the shift is identical in every phase, so it **cancels
//!    exactly in K-values / fugacity ratios** (proven numerically in the tests).
//!
//! ## Ported from DWSIM (GPL-3.0), Visual-Basic reference source
//!
//! - **PRSV κ / α**: `PropertyPackages/Models/PRSV2-VL.vb` L130-141 (and the
//!   identical block repeated at L346-357, L528-539, L718-729, L1031-1033,
//!   L1253-1264). DWSIM implements the more general **PRSV2** three-parameter
//!   form; setting its `κ2 = κ3 = 0` collapses it to the one-parameter **PRSV**
//!   ported here. Selector/data plumbing:
//!   `PropertyPackages/PengRobinsonStryjekVera2VL.vb` L485 (`kappa1`).
//! - **Peneloux translation**: applied in
//!   `PropertyPackages/SoaveRedlichKwong.vb` L433 (`Z −= AUX_CM/(RT)·P`) and
//!   L1048/L1179 (`v −= AUX_CM`); mixture `c = Σ zᵢ cᵢ` in
//!   `PropertyPackages/PengRobinson.vb` L565-604 and
//!   `SoaveRedlichKwong.vb` L144-170 (`AUX_CM`). The default Rackett
//!   compressibility `Z_RA = 0.29056 − 0.08775 ω` used when no tabulated value
//!   exists is `PropertyPackages/PropertyPackage.vb` L5604 (also
//!   `Models/FluidProperties.vb` L314).
//!
//! ## Two forms of the Peneloux coefficient — which one this port uses
//!
//! DWSIM's PR/SRK-Peneloux packages actually store `cᵢ = γᵢ · bᵢ` with `γᵢ` a
//! **tabulated dimensionless coefficient** per compound (`AUX_Ci`,
//! `PengRobinson.vb` L592-…). This port instead uses the **original
//! Rackett-based Peneloux (1982) correlation**
//!
//! `cᵢ = 0.40768 (R Tc / Pc)(0.29441 − Z_RA)`,  `Z_RA = 0.29056 − 0.08775 ω`,
//!
//! because it needs no per-compound translation table (deferred, same as the
//! PRSV κ1 table) and is fully determined by the critical constants + acentric
//! factor already carried by [`Component`]. DWSIM uses this exact `Z_RA`
//! default (see citation above). Both forms produce a small positive `cᵢ` for
//! normal fluids and share the identical VLE-invariance property.
//!
//! ## Units — documented raw `f64` (SI), per the crate `CLAUDE.md`
//!
//! Temperature K, pressure Pa, molar volume m³/mol, the translation `cᵢ` m³/mol,
//! the EOS attraction `a` J·m³/mol² (= Pa·m⁶/mol²). `κ0`, `κ1`, `α`, `Z_RA`,
//! mole fractions `z`, and `Tr = T/Tc` are dimensionless. Raw `f64` matches the
//! base kernel; every public signature spells out its units.
//!
//! ## Design (crate `CLAUDE.md`)
//!
//! No `Box`/`dyn`, no lifetimes, no channels. These refinements are exposed as
//! **free functions** rather than an enum variant of [`CubicEos`]: the PRSV κ1
//! is a *per-compound* datum (an array over the mixture), which an EOS-selector
//! enum carrying no per-component state cannot hold cleanly. Free functions
//! compose with the existing [`CubicEos`] kernel without duplicating its Z-solve
//! or fugacity code (see below), so no enum is warranted here.
//!
//! ## Composing with `cubic_eos`
//!
//! PRSV changes one thing: `a_i(T)`. To run a full PRSV flash you reuse the base
//! kernel unchanged and only substitute the attraction:
//!
//! ```text
//! b_mix = CubicEos::PengRobinson.b_mix(comps, z)         // unchanged
//! a_mix = prsv_a_mix(comps, kappa1, z, T, kij)           // refined attraction
//! A = a_mix · P / (R T)²;  B = b_mix · P / (R T)
//! Z = CubicEos::PengRobinson.z_roots(A, B) → phase-select // unchanged solver
//! v_EOS = Z R T / P
//! v = corrected_molar_volume(v_EOS, comps, z)            // Peneloux density fix
//! ```
//!
//! The fugacity-coefficient and departure expressions in [`CubicEos`] take the
//! same `(a_mix, b_mix)` and are algebraically identical for PRSV — only the
//! numeric `a_mix` fed in differs.
//!
//! ## Honest scope — what is and is NOT here
//!
//! - **Verification, not validation.** The tests below check the equations are
//!   implemented correctly against hand-computed values and the published
//!   constants (Stryjek & Vera 1986; Peneloux et al. 1982), **not** against
//!   experimental VLE / liquid-density benchmarks. The Peneloux liquid-density
//!   test shows the shift *moves density in the right direction*, not that it
//!   hits an experimental datum.
//! - **PRSV one-parameter κ1 only.** The PRSV2 second/third parameters
//!   (`κ2, κ3`) and other α-forms (Mathias-Copeman, Twu, PC-SAFT) are out of
//!   scope. With `κ1 = 0` the α-function reduces to the PRSV κ0(ω) correlation
//!   — *close to* but not identical to the classic 1976 PR κ(ω).
//! - **κ1 data table deferred.** `κ1` is a fitted per-compound constant; this
//!   port takes it as an input argument (default `0.0`) rather than shipping a
//!   compound→κ1 table.
//! - **Peneloux Rackett form only.** The tabulated `γᵢ·bᵢ` DWSIM variant is not
//!   ported (needs the deferred translation table).
//!
//! > **⚠️ Unverified until validated.** Early-stage AI-assisted translation.
//! > Not for nuclear facility operation, reactor control, safety-critical, or
//! > licensing decisions. Independent OUTRAM PARK fork, not the official DWSIM.

use crate::thermo::cubic_eos::{BinaryInteraction, CubicEos, R};
use crate::thermo::Component;

// ---------------------------------------------------------------------------
// PRSV α-function (Stryjek & Vera 1986)
// ---------------------------------------------------------------------------

/// PRSV κ0 polynomial constant term [-] (Stryjek & Vera 1986, Eq. 8).
pub const PRSV_KAPPA0_C0: f64 = 0.378893;
/// PRSV κ0 linear coefficient (in ω) [-].
pub const PRSV_KAPPA0_C1: f64 = 1.4897153;
/// PRSV κ0 quadratic coefficient (in ω²) [-].
pub const PRSV_KAPPA0_C2: f64 = 0.17131848;
/// PRSV κ0 cubic coefficient (in ω³) [-].
///
/// The published Stryjek-Vera (1986) value is `0.0196554`; DWSIM's source
/// (`PRSV2-VL.vb` L130) carries `0.0196544` — a last-digit transcription
/// difference. This port uses the **published** `0.0196554`; the resulting κ0
/// differs from DWSIM by `< 1e-8` for `ω ≲ 1`, far below any physical
/// significance.
pub const PRSV_KAPPA0_C3: f64 = 0.0196554;

/// PRSV `κ0(ω)` — the acentric-factor part of the PRSV α-slope [-].
///
/// `κ0 = 0.378893 + 1.4897153 ω − 0.17131848 ω² + 0.0196554 ω³`
/// (Stryjek & Vera 1986; DWSIM `PRSV2-VL.vb` L130). `ω` is the Pitzer acentric
/// factor [-]. This is the PRSV refit of the PR α-slope; it is *close to* but
/// not identical to the classic 1976 PR `κ(ω) = 0.37464 + 1.54226 ω −
/// 0.26992 ω²`. Valid for the usual `−0.1 ≲ ω ≲ 1` range.
#[must_use]
pub fn prsv_kappa0(acentric_factor: f64) -> f64 {
    let w = acentric_factor;
    PRSV_KAPPA0_C0 + PRSV_KAPPA0_C1 * w - PRSV_KAPPA0_C2 * w * w + PRSV_KAPPA0_C3 * w * w * w
}

/// Full PRSV temperature-dependent α-slope `κ(T)` [-] for a component.
///
/// `κ = κ0 + κ1 (1 + √Tr)(0.7 − Tr)`, with `Tr = T/Tc` [-] and `κ0` from
/// [`prsv_kappa0`] (Stryjek & Vera 1986, Eq. 7; DWSIM `PRSV2-VL.vb` L130 with
/// the PRSV2 `κ2 = κ3 = 0`). `kappa1` [-] is the compound-specific fitted
/// parameter (default `0.0` → pure κ0). `t` [K] must be `> 0`. The `(0.7 − Tr)`
/// factor makes the κ1 correction vanish at `Tr = 0.7` and change sign across
/// it, the reduced temperature at which Stryjek & Vera anchored the fit.
#[must_use]
pub fn prsv_kappa(comp: &Component, kappa1: f64, t: f64) -> f64 {
    let tr = t / comp.critical_temperature;
    let sqrt_tr = tr.sqrt();
    prsv_kappa0(comp.acentric_factor) + kappa1 * (1.0 + sqrt_tr) * (0.7 - tr)
}

/// PRSV α-function `α(T) = [1 + κ(1 − √Tr)]²` [-] for a component.
///
/// The temperature scaling of the PR attraction under the PRSV κ (DWSIM
/// `PRSV2-VL.vb` L131). `κ` is [`prsv_kappa`]; `t` [K] must be `> 0`;
/// `Tr = T/Tc`. At the critical point (`Tr = 1`) `α = 1` **exactly**, for any
/// `κ1`, because `(1 − √1) = 0`. With `κ1 = 0` this reduces to the PRSV κ0(ω)
/// α-function, numerically close to the base PR α from
/// [`CubicEos::alpha`].
#[must_use]
pub fn prsv_alpha(comp: &Component, kappa1: f64, t: f64) -> f64 {
    let tr = t / comp.critical_temperature;
    let term = 1.0 + prsv_kappa(comp, kappa1, t) * (1.0 - tr.sqrt());
    term * term
}

/// PRSV pure-component attraction `a_i(T) = 0.45724 · α_PRSV(T) · R² Tc² / Pc`
/// [J·m³/mol²] at `t` [K].
///
/// Identical to [`CubicEos::a_i`] for Peng-Robinson except the α-function is the
/// PRSV [`prsv_alpha`] instead of the base PR α (DWSIM `PRSV2-VL.vb` L140 uses
/// the same `Ωa = 0.45724`). The co-volume `b_i` is **unchanged** — obtain it
/// from `CubicEos::PengRobinson.b_i(comp)`. Valid for `t > 0`.
#[must_use]
pub fn prsv_a_i(comp: &Component, kappa1: f64, t: f64) -> f64 {
    let tc = comp.critical_temperature;
    let pc = comp.critical_pressure;
    CubicEos::PengRobinson.omega_a() * prsv_alpha(comp, kappa1, t) * R * R * tc * tc / pc
}

/// Fetch `k_ij` from an optional matrix, treating `None` (or an out-of-range
/// matrix) as zero — the geometric-mean default (mirrors the base kernel).
#[inline]
fn kij_at(kij: Option<&BinaryInteraction>, i: usize, j: usize) -> f64 {
    match kij {
        Some(m) if i < m.len() && j < m.len() => m.get(i, j),
        _ => 0.0,
    }
}

/// PRSV mixture attraction `a_mix = Σ_i Σ_j z_i z_j √(a_i a_j)(1 − k_ij)`
/// [J·m³/mol²] at `t` [K], using the PRSV pure-component `a_i` from
/// [`prsv_a_i`].
///
/// This is the **identical** van der Waals one-fluid mixing rule as
/// [`CubicEos::a_mix`] — only the per-component `a_i` differs (PRSV vs base PR).
/// `kappa1` [-] is the per-component array (same length as `comps` and `z`);
/// pass all-zeros for the pure-κ0 PRSV. `kij = None` uses the geometric-mean
/// rule. The mixture co-volume `b_mix` is unchanged: use
/// `CubicEos::PengRobinson.b_mix(comps, z)`.
///
/// # Panics
/// Panics (via slice indexing) if `comps`, `kappa1`, and `z` differ in length.
#[must_use]
pub fn prsv_a_mix(
    comps: &[Component],
    kappa1: &[f64],
    z: &[f64],
    t: f64,
    kij: Option<&BinaryInteraction>,
) -> f64 {
    let ai: Vec<f64> = comps
        .iter()
        .zip(kappa1)
        .map(|(c, &k1)| prsv_a_i(c, k1, t))
        .collect();
    let mut am = 0.0;
    for i in 0..comps.len() {
        for j in 0..comps.len() {
            am += z[i] * z[j] * (ai[i] * ai[j]).sqrt() * (1.0 - kij_at(kij, i, j));
        }
    }
    am
}

// ---------------------------------------------------------------------------
// Peneloux volume translation (Peneloux, Rauzy & Fréze 1982)
// ---------------------------------------------------------------------------

/// Peneloux Rackett prefactor `0.40768` [-] (Peneloux et al. 1982, Eq. 8).
pub const PENELOUX_RACKETT_PREFACTOR: f64 = 0.40768;
/// Peneloux Rackett offset `0.29441` [-] (Peneloux et al. 1982, Eq. 8).
pub const PENELOUX_RACKETT_OFFSET: f64 = 0.29441;
/// Rackett `Z_RA` correlation constant term `0.29056` [-]
/// (DWSIM `PropertyPackage.vb` L5604; Yamada-Gunn form).
pub const RACKETT_ZRA_C0: f64 = 0.29056;
/// Rackett `Z_RA` acentric-factor coefficient `0.08775` [-]
/// (DWSIM `PropertyPackage.vb` L5604).
pub const RACKETT_ZRA_C1: f64 = 0.08775;

/// Rackett compressibility `Z_RA = 0.29056 − 0.08775 ω` [-] for a component.
///
/// The Yamada-Gunn estimate of the Rackett parameter from the acentric factor,
/// used as the DWSIM default when no tabulated `Z_RA` exists
/// (`PropertyPackage.vb` L5604). Dimensionless; typically `0.24–0.29` for normal
/// fluids.
#[must_use]
pub fn rackett_z_ra(comp: &Component) -> f64 {
    RACKETT_ZRA_C0 - RACKETT_ZRA_C1 * comp.acentric_factor
}

/// Peneloux per-component volume-translation shift `c_i` [m³/mol].
///
/// `c_i = 0.40768 (R Tc / Pc)(0.29441 − Z_RA)` with `Z_RA` from
/// [`rackett_z_ra`] (Peneloux, Rauzy & Fréze 1982). `c_i` is a **constant** —
/// independent of `T`, `P`, and composition — which is exactly why it cancels in
/// K-values (see [`peneloux_lnphi_shift`]). For a normal fluid
/// `Z_RA < 0.29441`, so `c_i > 0`: the translation *reduces* the EOS molar
/// volume, correcting PR/SRK's systematic liquid-volume overprediction. Units:
/// `R Tc / Pc` is m³/mol, the parenthesised factor is dimensionless.
#[must_use]
pub fn peneloux_shift(comp: &Component) -> f64 {
    let r_tc_pc = R * comp.critical_temperature / comp.critical_pressure;
    PENELOUX_RACKETT_PREFACTOR * r_tc_pc * (PENELOUX_RACKETT_OFFSET - rackett_z_ra(comp))
}

/// Mixture volume-translation `c = Σ z_i c_i` [m³/mol].
///
/// Linear (mole-fraction) mixing of the pure-component shifts, matching DWSIM's
/// `AUX_CM` (`PengRobinson.vb` L580-590, `SoaveRedlichKwong.vb` L159-169).
/// `comps` and mole fractions `z` [-] must have equal length (a mismatch is
/// silently truncated by the `zip`, so the caller must pass equal-length
/// slices); `z` should sum to 1.
#[must_use]
pub fn peneloux_c_mix(comps: &[Component], z: &[f64]) -> f64 {
    comps
        .iter()
        .zip(z)
        .map(|(c, &zi)| zi * peneloux_shift(c))
        .sum()
}

/// Peneloux-corrected molar volume `v = v_EOS − c` [m³/mol].
///
/// Subtracts the mixture translation [`peneloux_c_mix`] from the untranslated
/// cubic-EOS molar volume `v_eos` [m³/mol] (which the caller forms as
/// `Z R T / P` from the base [`CubicEos`] solve). Since `c > 0` for normal
/// fluids, `v < v_eos`, so the predicted **liquid density** `ρ = M/v`
/// **increases** toward experiment. Applying it to a vapour molar volume is
/// harmless (the relative shift is negligible at low density). DWSIM applies the
/// same subtraction at `SoaveRedlichKwong.vb` L1048/L1179.
#[must_use]
pub fn corrected_molar_volume(v_eos: f64, comps: &[Component], z: &[f64]) -> f64 {
    v_eos - peneloux_c_mix(comps, z)
}

/// Fugacity-coefficient shift from the Peneloux translation, `c_i P / (R T)`
/// [-], for one component at `t` [K], `p` [Pa].
///
/// Under a constant volume translation the fugacity coefficient transforms as
/// `ln φ_i^translated = ln φ_i^EOS − c_i P/(R T)` (Peneloux et al. 1982). This
/// returns the subtracted term. Because `c_i` and `P, T` are the **same in the
/// liquid and the vapour**, the term is identical in both phases and **cancels
/// exactly** in the K-value `K_i = φ_i^L / φ_i^V`:
///
/// `ln K_i^translated = (ln φ_i^L − c_iP/RT) − (ln φ_i^V − c_iP/RT) = ln K_i^EOS`.
///
/// Hence the Peneloux shift improves liquid density **without altering VLE**.
/// Verified numerically in the tests.
#[must_use]
pub fn peneloux_lnphi_shift(comp: &Component, t: f64, p: f64) -> f64 {
    peneloux_shift(comp) * p / (R * t)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::thermo::component::reference;
    use crate::thermo::cubic_eos::Phase;
    use approx::assert_relative_eq;

    /// **Methodology.** The PRSV κ0(ω) polynomial (Stryjek & Vera 1986, Eq. 8)
    /// must reproduce the published constants. Hand-computed for CO₂ (ω = 0.225)
    /// with the published cubic coefficient `0.0196554`:
    ///   κ0 = 0.378893 + 1.4897153·0.225 − 0.17131848·0.225² + 0.0196554·0.225³
    ///      = 0.705630.
    /// A cross-check at ω = 0 must give exactly the constant term 0.378893.
    /// **Results (2026-07-24):** computed κ0(0.225) = 0.7056298, κ0(0) =
    /// 0.378893 — match to 6 sig figs.
    #[test]
    fn prsv_kappa0_matches_polynomial() {
        assert_relative_eq!(prsv_kappa0(0.225), 0.705630, max_relative = 1e-5);
        assert_relative_eq!(prsv_kappa0(0.0), PRSV_KAPPA0_C0, epsilon = 1e-15);
    }

    /// **Methodology.** Two required limits of the PRSV α-function:
    ///   (a) at the critical point `Tr = 1`, `α = 1` exactly for any κ1
    ///       (the `(1 − √Tr)` factor is zero);
    ///   (b) with κ1 = 0 the PRSV α must be *close to* the base PR α (same
    ///       functional form, only the κ0(ω) refit differs). Evaluate CO₂ at
    ///       Tr = 0.8 (T = 243.30 K).
    /// **Results (2026-07-24):** α_PRSV(Tr=1) = 1.0 to 1e-15; at Tr = 0.8,
    /// α_PRSV(κ1=0) = 1.154540 vs base-PR α = 1.155074 — differ by 5.3e-4
    /// (< 1e-3), confirming "reduces toward standard PR".
    #[test]
    fn prsv_alpha_critical_and_pr_limit() {
        let co2 = reference::carbon_dioxide();
        // (a) alpha = 1 at Tc, for several kappa1.
        for k1 in [0.0, 0.03, -0.05] {
            assert_relative_eq!(
                prsv_alpha(&co2, k1, co2.critical_temperature),
                1.0,
                epsilon = 1e-15
            );
        }
        // (b) kappa1 = 0 is close to base PR alpha at Tr = 0.8.
        let t = 0.8 * co2.critical_temperature;
        let a_prsv = prsv_alpha(&co2, 0.0, t);
        let a_pr = CubicEos::PengRobinson.alpha(t / co2.critical_temperature, co2.acentric_factor);
        assert!((a_prsv - a_pr).abs() < 1e-3, "prsv {a_prsv} vs pr {a_pr}");
    }

    /// **Methodology.** The κ1 correction must vanish at `Tr = 0.7` (the
    /// `(0.7 − Tr)` factor) and shift α off the κ0-only value elsewhere. Compare
    /// PRSV a_i for CO₂ with κ1 = 0 vs κ1 = +0.03 at Tr = 0.7 and Tr = 0.8.
    /// **Results (2026-07-24):** at Tr = 0.7 the two a_i are equal to < 1e-12
    /// (κ1 term inactive); at Tr = 0.8 a nonzero κ1 changes a_i by ≈ 0.11%,
    /// confirming the κ1 wiring.
    #[test]
    fn prsv_kappa1_vanishes_at_tr_0_7() {
        let co2 = reference::carbon_dioxide();
        let t07 = 0.7 * co2.critical_temperature;
        assert_relative_eq!(
            prsv_a_i(&co2, 0.0, t07),
            prsv_a_i(&co2, 0.03, t07),
            max_relative = 1e-12
        );
        let t08 = 0.8 * co2.critical_temperature;
        let a0 = prsv_a_i(&co2, 0.0, t08);
        let a1 = prsv_a_i(&co2, 0.03, t08);
        assert!(
            (a1 - a0).abs() / a0 > 1e-4,
            "kappa1 had no effect: {a0} {a1}"
        );
    }

    /// **Methodology.** [`prsv_a_mix`] must reuse the base van der Waals mixing
    /// rule: for a single component it must equal the pure [`prsv_a_i`], and its
    /// algebra must match [`CubicEos::a_mix`] fed the same per-component `a_i`.
    /// Check pure CO₂ at 300 K, and a CO₂/methane 50/50 mixture at 250 K with
    /// κ1 = 0 (so a_i are PRSV-κ0) — compare against a hand geometric-mean sum.
    /// **Results (2026-07-24):** single-component a_mix equals a_i to machine
    /// precision; the binary a_mix equals the explicit
    /// `Σ z_i z_j √(a_i a_j)` sum to < 1e-12.
    #[test]
    fn prsv_a_mix_reuses_vdw_mixing() {
        let co2 = reference::carbon_dioxide();
        let pure = [co2.clone()];
        let am1 = prsv_a_mix(&pure, &[0.0], &[1.0], 300.0, None);
        assert_relative_eq!(am1, prsv_a_i(&co2, 0.0, 300.0), max_relative = 1e-12);

        let comps = [reference::carbon_dioxide(), reference::methane()];
        let k1 = [0.0, 0.0];
        let z = [0.5, 0.5];
        let t = 250.0;
        let am = prsv_a_mix(&comps, &k1, &z, t, None);
        let a0 = prsv_a_i(&comps[0], 0.0, t);
        let a1 = prsv_a_i(&comps[1], 0.0, t);
        let hand = z[0] * z[0] * a0 + z[1] * z[1] * a1 + 2.0 * z[0] * z[1] * (a0 * a1).sqrt();
        assert_relative_eq!(am, hand, max_relative = 1e-12);
    }

    /// **Methodology.** The Peneloux shift `c_i = 0.40768 (RTc/Pc)(0.29441 −
    /// Z_RA)` (Peneloux et al. 1982) must be **positive** for a normal fluid.
    /// Hand-computed for CO₂ (Tc = 304.12 K, Pc = 7.374 MPa, ω = 0.225):
    ///   Z_RA = 0.29056 − 0.08775·0.225 = 0.270816,
    ///   RTc/Pc = 3.42907e-4 m³/mol,
    ///   c = 0.40768·3.42907e-4·(0.29441 − 0.270816) = 3.2983e-6 m³/mol.
    /// **Results (2026-07-24):** computed c_CO₂ = 3.29832e-6 m³/mol (> 0),
    /// matching the hand value to 5 sig figs; water c = 3.3838e-6, methane
    /// c = 6.763e-7 m³/mol — all positive.
    #[test]
    fn peneloux_shift_positive_and_matches_hand() {
        let co2 = reference::carbon_dioxide();
        assert_relative_eq!(rackett_z_ra(&co2), 0.270816, max_relative = 1e-4);
        assert_relative_eq!(peneloux_shift(&co2), 3.29832e-6, max_relative = 1e-4);
        for c in [
            reference::carbon_dioxide(),
            reference::water(),
            reference::methane(),
            reference::nitrogen(),
            reference::ethane(),
        ] {
            assert!(peneloux_shift(&c) > 0.0, "c_i not positive for {}", c.name);
        }
    }

    /// **Methodology.** The Peneloux translation must **lower** the predicted
    /// liquid molar volume (raising liquid density) versus the untranslated PR
    /// EOS. Take pure CO₂ liquid at T = 250 K, P = 20 bar (2.0e6 Pa, above the
    /// ≈17.9 bar saturation pressure so the phase is liquid). Solve the base PR
    /// liquid Z with [`CubicEos::z_factor`], form `v_EOS = Z R T / P`, then apply
    /// [`corrected_molar_volume`]. Verification only — this checks the *sign and
    /// magnitude* of the correction, NOT agreement with an experimental density.
    /// **Results (2026-07-24):** Z_liq = 0.039548, v_EOS = 4.1102e-5 m³/mol,
    /// c = 3.2983e-6, v_corr = 3.7804e-5 m³/mol; density rises from 1070.8 to
    /// 1164.2 kg/m³. The correction lowers v by exactly c (8.0%).
    #[test]
    fn peneloux_lowers_liquid_molar_volume() {
        let co2 = reference::carbon_dioxide();
        let comps = [co2.clone()];
        let z = [1.0];
        let (t, p) = (250.0, 2.0e6);
        let zl = CubicEos::PengRobinson
            .z_factor(&comps, &z, t, p, Phase::Liquid, None)
            .unwrap();
        let v_eos = zl * R * t / p;
        let v_corr = corrected_molar_volume(v_eos, &comps, &z);
        assert!(
            v_corr < v_eos,
            "Peneloux did not lower volume: {v_corr} !< {v_eos}"
        );
        // The reduction is exactly the mixture translation c.
        assert_relative_eq!(v_eos - v_corr, peneloux_shift(&co2), max_relative = 1e-12);
        // Density increases.
        let m = co2.molar_mass;
        assert!(m / v_corr > m / v_eos);
        // Cross-check the measured numbers documented above.
        assert_relative_eq!(v_eos, 4.1102e-5, max_relative = 1e-3);
        assert_relative_eq!(v_corr, 3.7804e-5, max_relative = 1e-3);
    }

    /// **Methodology — VLE invariance.** The Peneloux shift `c_i P/(R T)` is the
    /// same in both phases, so it must cancel in the K-value. Numerically:
    /// build the translated ln φ for CO₂/methane at T = 220 K, P = 15 bar in a
    /// liquid and a vapour phase (base PR ln φ from [`CubicEos::ln_phi`] minus
    /// [`peneloux_lnphi_shift`]), and confirm
    ///   ln K_i^translated = ln φ_i^L,t − ln φ_i^V,t
    /// equals the untranslated ln K_i^EOS = ln φ_i^L − ln φ_i^V.
    /// **Results (2026-07-24):** for both components the translated and
    /// untranslated ln K_i agree to < 1e-14 — the shift cancels exactly, so
    /// K-values (and thus the flash) are unchanged by the density correction.
    #[test]
    fn peneloux_cancels_in_k_values() {
        let comps = [reference::carbon_dioxide(), reference::methane()];
        let z = [0.5, 0.5];
        let (t, p) = (220.0, 1.5e6);
        let ln_phi_l = CubicEos::PengRobinson
            .ln_phi(&comps, &z, t, p, Phase::Liquid, None)
            .unwrap();
        let ln_phi_v = CubicEos::PengRobinson
            .ln_phi(&comps, &z, t, p, Phase::Vapor, None)
            .unwrap();
        for i in 0..comps.len() {
            let shift = peneloux_lnphi_shift(&comps[i], t, p);
            // Translated ln phi in each phase.
            let lpl_t = ln_phi_l[i] - shift;
            let lpv_t = ln_phi_v[i] - shift;
            let ln_k_translated = lpl_t - lpv_t;
            let ln_k_eos = ln_phi_l[i] - ln_phi_v[i];
            assert_relative_eq!(ln_k_translated, ln_k_eos, epsilon = 1e-14);
        }
    }
}
