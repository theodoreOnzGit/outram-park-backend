//! Lee-Kesler-Plöcker (LKP) three-parameter corresponding-states EOS.
//!
//! Ported from DWSIM (GPL-3.0), Visual-Basic reference source:
//! - `DWSIM.Thermodynamics/PropertyPackages/Models/LeeKeslerPlocker.vb`
//!   (commit `1abf72d`): `Z_LK` L279-331, the Lee-Kesler modified-BWR
//!   reduced-volume solver `ESTIMAR_Vr2` L552-668, the enthalpy departure
//!   `H_LK` L333-390, the entropy departure `S_LK` L453-514, the pure
//!   corresponding-states fugacity `LnFugM` L810-873, and the mixture critical
//!   combining rule `MixCritProp_LK` L93-147.
//!
//! ## What this model is
//!
//! Unlike Peng-Robinson / SRK (a cubic in `V`), LKP is a **corresponding-states**
//! method built on the Lee-Kesler (1975) modified Benedict-Webb-Rubin (BWR)
//! equation written in reduced coordinates. Two reference fluids are used:
//!
//! - a **simple fluid** (`ω = 0`, argon/krypton/methane-like), and
//! - a **heavy reference fluid** (`ω_ref = 0.3978`, n-octane).
//!
//! Any property `M` (compressibility, enthalpy/entropy departure, log-fugacity)
//! is linearly interpolated in the acentric factor between the two:
//!
//! `M = M⁰ + (ω / 0.3978) · (Mʳᵉᶠ − M⁰)`
//!
//! (`LeeKeslerPlocker.vb` L327 for `Z`, L386 for `H`, L510 for `S`, L869 for
//! `ln φ`). Plöcker's contribution is the **mixture** critical-property
//! combining rule ([`mix_crit_props`], `MixCritProp_LK.vb` L93-147) that maps a
//! multicomponent mixture onto a single set of pseudo-critical `Tcm, Pcm, Vcm,
//! ωm`, so the pure corresponding-states functions apply to mixtures too.
//!
//! LKP gives accurate densities and enthalpy departures for **light gases and
//! their mixtures** (CO, CO₂, H₂, H₂O, He, N₂, CH₄) over a wide pressure range,
//! which is why it is relevant to HTGR cover-gas / water-ingress chemistry.
//!
//! ## Units — documented raw `f64` (SI), per the crate `CLAUDE.md`
//!
//! Temperature K, pressure Pa, molar volume m³/mol, enthalpy departure J/mol,
//! entropy departure J/(mol·K), critical volume `Vc` m³/mol. The reduced
//! quantities `Tr = T/Tc`, `Pr = P/Pc`, the reduced volume `Vr = Pc V / (R Tc)`,
//! the compressibility `Z`, the acentric factor `ω`, and mole fractions are all
//! dimensionless. Raw `f64` is used throughout the inner corresponding-states
//! loops (matching [`crate::thermo::cubic_eos`]); every public signature spells
//! out its units.
//!
//! Note on `R`: DWSIM hard-codes `R = 8.314`. This port uses the CODATA-2018
//! exact `R` (re-exported from [`crate::thermo::cubic_eos::R`]) so single-point
//! numbers differ from DWSIM at the 5th significant figure, consistent with the
//! rest of this crate.
//!
//! ## Design (crate `CLAUDE.md`)
//!
//! Enum dispatch, **no `dyn`/`Box`/lifetimes**: the closed set of the two
//! Lee-Kesler reference fluids is the [`LkFluid`] enum, whose variants carry no
//! data and return their BWR constants from `const` methods. The
//! corresponding-states property routines are free functions over `f64` and
//! `&[Component]`, mirroring [`crate::thermo::eos_variants`].
//!
//! ## Honest scope — what is and is NOT ported
//!
//! - **Ported:** the pure/mixture compressibility `Z` ([`z_lkp`], [`z_mix`]),
//!   the enthalpy and entropy departures ([`enthalpy_departure`],
//!   [`entropy_departure`], and their mixture forms), the pure
//!   corresponding-states log-fugacity ([`ln_phi_pure`]), and Plöcker's mixture
//!   critical combining rule ([`mix_crit_props`]).
//! - **NOT ported:** the full multicomponent fugacity-coefficient path
//!   (`CalcLnFugCPU` L956-1048), which differentiates the pseudo-critical
//!   properties w.r.t. composition (`dTcmdx`, `dPcmdx`) to get per-component
//!   `ln φ_i`; the `CPCV_LK` heat-capacity departures (L679-755); and the LKP
//!   binary-interaction `k_ij` data table (`lkp_ip.dat`, L70). The `k_ij` here
//!   defaults to the ideal `1.0` (geometric-mean `Tc` combining), matching
//!   DWSIM's own default when no table entry exists (`MixCritProp_LK` L104-110).
//!   These omissions are documented, not hidden.
//!
//! > **⚠️ Untrusted AI-assisted draft — pending human V&V.** Early-stage
//! > translation. The tests below are *verification* against an independent
//! > analytic correlation (the Pitzer/Prausnitz generalized second virial) and
//! > against published Lee-Kesler behaviour, **not** validation against
//! > experimental data. Not for nuclear facility operation, reactor control,
//! > safety-critical, or licensing decisions. Independent OUTRAM PARK fork, not
//! > the official DWSIM.

use crate::thermo::cubic_eos::R;
use crate::thermo::Component;

/// The reference acentric factor `ω_ref` of the Lee-Kesler heavy reference fluid
/// (n-octane), `0.3978` [-] (`LeeKeslerPlocker.vb` L325).
///
/// The LKP interpolation weight is `ω / ω_ref`.
pub const OMEGA_REF: f64 = 0.3978;

/// Fluid phase selector for the Lee-Kesler reduced-volume root.
///
/// The reduced-volume equation can have multiple roots below the critical
/// temperature; this picks which one the solver returns.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Phase {
    /// Vapour — the largest reduced-volume root.
    Vapor,
    /// Liquid — the smallest strictly-positive reduced-volume root.
    Liquid,
}

/// One of the two Lee-Kesler reference fluids.
///
/// Enum dispatch over the closed pair `{Simple, Reference}` (no trait objects
/// per the workspace `CLAUDE.md`). Each variant carries no data; the twelve
/// modified-BWR constants for that fluid are returned by [`LkFluid::constants`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LkFluid {
    /// The simple fluid (`ω = 0`), argon/methane-like.
    Simple,
    /// The heavy reference fluid (`ω_ref = 0.3978`), n-octane.
    Reference,
}

/// The twelve modified-BWR constants of a Lee-Kesler reference fluid.
///
/// These parameterise the reduced pressure equation
///
/// `Z = 1 + B/Vr + C/Vr² + D/Vr⁵ + (c4 / (Tr³ Vr²)) (β + γ/Vr²) exp(−γ/Vr²)`,
///
/// with `B = b1 − b2/Tr − b3/Tr² − b4/Tr³`, `C = c1 − c2/Tr + c3/Tr³`,
/// `D = d1 + d2/Tr` (`LeeKeslerPlocker.vb` L284-299 / L363-374). All are
/// dimensionless.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct LkConstants {
    /// `b1` [-].
    pub b1: f64,
    /// `b2` [-].
    pub b2: f64,
    /// `b3` [-].
    pub b3: f64,
    /// `b4` [-].
    pub b4: f64,
    /// `c1` [-].
    pub c1: f64,
    /// `c2` [-].
    pub c2: f64,
    /// `c3` [-].
    pub c3: f64,
    /// `c4` [-].
    pub c4: f64,
    /// `d1` [-].
    pub d1: f64,
    /// `d2` [-].
    pub d2: f64,
    /// `β` [-].
    pub beta: f64,
    /// `γ` [-].
    pub gamma: f64,
}

impl LkFluid {
    /// The published Lee-Kesler (1975) BWR constants for this fluid
    /// (`LeeKeslerPlocker.vb` L284-295 simple fluid, L304-315 reference fluid).
    #[must_use]
    pub const fn constants(self) -> LkConstants {
        match self {
            Self::Simple => LkConstants {
                b1: 0.1181193,
                b2: 0.265728,
                b3: 0.15479,
                b4: 0.030323,
                c1: 0.0236744,
                c2: 0.0186984,
                c3: 0.0,
                c4: 0.042724,
                d1: 0.0000155488,
                d2: 0.0000623689,
                beta: 0.65392,
                gamma: 0.060167,
            },
            Self::Reference => LkConstants {
                b1: 0.2026579,
                b2: 0.331511,
                b3: 0.027655,
                b4: 0.203488,
                c1: 0.0313385,
                c2: 0.0503618,
                c3: 0.016901,
                c4: 0.041577,
                d1: 0.000048736,
                d2: 0.00000740336,
                beta: 1.226,
                gamma: 0.03754,
            },
        }
    }
}

impl LkConstants {
    /// The temperature-dependent group `B = b1 − b2/Tr − b3/Tr² − b4/Tr³` [-]
    /// at reduced temperature `tr = T/Tc` [-] (`LeeKeslerPlocker.vb` L297).
    #[must_use]
    pub fn b_of_tr(&self, tr: f64) -> f64 {
        self.b1 - self.b2 / tr - self.b3 / (tr * tr) - self.b4 / (tr * tr * tr)
    }

    /// The group `C = c1 − c2/Tr + c3/Tr³` [-] (`LeeKeslerPlocker.vb` L298).
    #[must_use]
    pub fn c_of_tr(&self, tr: f64) -> f64 {
        self.c1 - self.c2 / tr + self.c3 / (tr * tr * tr)
    }

    /// The group `D = d1 + d2/Tr` [-] (`LeeKeslerPlocker.vb` L299).
    #[must_use]
    pub fn d_of_tr(&self, tr: f64) -> f64 {
        self.d1 + self.d2 / tr
    }

    /// Compressibility `Z = Pr·Vr/Tr` [-] from the reduced volume `vr` [-] at
    /// `tr, pr` [-] — i.e. the right-hand side of the Lee-Kesler reduced
    /// equation (`LeeKeslerPlocker.vb` L578):
    ///
    /// `Z = 1 + B/Vr + C/Vr² + D/Vr⁵ + (c4/(Tr³ Vr²))(β + γ/Vr²) exp(−γ/Vr²)`.
    #[must_use]
    pub fn z_from_vr(&self, tr: f64, vr: f64) -> f64 {
        let b = self.b_of_tr(tr);
        let c = self.c_of_tr(tr);
        let d = self.d_of_tr(tr);
        let vr2 = vr * vr;
        1.0 + b / vr + c / vr2 + d / vr.powi(5)
            + self.c4 / (tr * tr * tr * vr2) * (self.beta + self.gamma / vr2)
                * (-self.gamma / vr2).exp()
    }
}

/// Solve the Lee-Kesler reduced-volume `Vr` [-] for one reference fluid at
/// reduced temperature `tr` [-] and reduced pressure `pr` [-].
///
/// Finds `Vr > 0` such that `Pr·Vr/Tr = Z(Vr)` where `Z(Vr)` is
/// [`LkConstants::z_from_vr`]. This replaces DWSIM's `ESTIMAR_Vr2`
/// (`LeeKeslerPlocker.vb` L552-668, a scan + Brent hybrid) with a bracket-scan
/// plus bisection: the residual `f(Vr) = Pr·Vr/Tr − Z(Vr)` is scanned for a sign
/// change (descending in `Vr` for [`Phase::Vapor`], ascending for
/// [`Phase::Liquid`]) then bisected to a relative tolerance of `1e-12`.
///
/// Returns `None` if no positive root brackets in the scan window (should not
/// happen for physical gas-phase inputs `Tr ≳ 0.3`).
#[must_use]
pub fn reduced_volume(fluid: LkFluid, tr: f64, pr: f64, phase: Phase) -> Option<f64> {
    let k = fluid.constants();
    // Residual whose root is the reduced volume.
    let f = |vr: f64| pr * vr / tr - k.z_from_vr(tr, vr);

    // The ideal-gas reduced volume Vr_ig = Tr/Pr is the natural vapour scale.
    let vr_ideal = tr / pr;
    // Scan bounds: liquid roots are small (~0.02..1), vapour roots near Vr_ideal.
    let (lo, hi, n) = match phase {
        // Vapour: scan downward from well above the ideal volume.
        Phase::Vapor => (1.0e-3, (vr_ideal * 4.0).max(20.0), 4000usize),
        // Liquid: scan upward from a small reduced volume.
        Phase::Liquid => (1.0e-3, 2.0, 4000usize),
    };

    let step = (hi - lo) / n as f64;
    // Collect the first bracket in scan order (descending for vapour).
    let mut bracket: Option<(f64, f64)> = None;
    let mut prev_v = match phase {
        Phase::Vapor => hi,
        Phase::Liquid => lo,
    };
    let mut prev_f = f(prev_v);
    for i in 1..=n {
        let v = match phase {
            Phase::Vapor => hi - step * i as f64,
            Phase::Liquid => lo + step * i as f64,
        };
        if v <= 0.0 {
            break;
        }
        let fv = f(v);
        if prev_f.is_finite() && fv.is_finite() && prev_f * fv <= 0.0 {
            bracket = Some((prev_v.min(v), prev_v.max(v)));
            break;
        }
        prev_v = v;
        prev_f = fv;
    }

    let (mut a, mut b) = bracket?;
    let mut fa = f(a);
    // Bisection to convergence.
    for _ in 0..200 {
        let m = 0.5 * (a + b);
        let fm = f(m);
        if (b - a).abs() <= 1e-12 * m.abs().max(1e-12) || fm == 0.0 {
            return Some(m);
        }
        if (fa < 0.0) != (fm < 0.0) {
            b = m;
        } else {
            a = m;
            fa = fm;
        }
    }
    Some(0.5 * (a + b))
}

/// Compressibility factor `Z` [-] of one Lee-Kesler reference fluid at reduced
/// `tr, pr` [-] (`LeeKeslerPlocker.vb` L323 `zs`/`zh`).
///
/// `Z = Pr·Vr/Tr` evaluated at the solved reduced volume [`reduced_volume`].
/// Returns `None` if the reduced-volume solve fails.
#[must_use]
pub fn z_fluid(fluid: LkFluid, tr: f64, pr: f64, phase: Phase) -> Option<f64> {
    let vr = reduced_volume(fluid, tr, pr, phase)?;
    Some(pr * vr / tr)
}

/// Lee-Kesler-Plöcker compressibility factor `Z` [-] of a pure fluid (or a
/// pseudo-pure mixture) at reduced temperature `tr = T/Tc` [-], reduced pressure
/// `pr = P/Pc` [-], and acentric factor `w = ω` [-].
///
/// The three-parameter corresponding-states interpolation
/// `Z = Z⁰ + (ω/0.3978)(Zʳᵉᶠ − Z⁰)` (`LeeKeslerPlocker.vb` L327), where `Z⁰` is
/// the simple-fluid compressibility and `Zʳᵉᶠ` the heavy-reference-fluid
/// compressibility, both at the *same* `tr, pr`. Valid over the gas and dense-gas
/// region where the Lee-Kesler tables apply (`0.3 ≲ Tr`, `Pr ≲ 10`). Returns
/// `None` if either reference-fluid reduced-volume solve fails.
///
/// With `w = 0` this returns exactly the simple-fluid `Z⁰`.
#[must_use]
pub fn z_lkp(tr: f64, pr: f64, w: f64, phase: Phase) -> Option<f64> {
    let z0 = z_fluid(LkFluid::Simple, tr, pr, phase)?;
    let zref = z_fluid(LkFluid::Reference, tr, pr, phase)?;
    Some(z0 + w / OMEGA_REF * (zref - z0))
}

/// Dimensionless enthalpy departure `(H − H_ig) / (R Tc)` [-] of one reference
/// fluid at reduced `tr, pr` [-] (`LeeKeslerPlocker.vb` L361 / L384).
///
/// `= Tr [ Z − 1 − (b2 + 2b3/Tr + 3b4/Tr²)/(Tr Vr) − (c2 − 3c3/Tr²)/(2 Tr Vr²)`
/// `      + d2/(5 Tr Vr⁵) + 3E ]`, with `E = c4/(2 Tr³ γ)[β + 1 − (β + 1 +`
/// `γ/Vr²) exp(−γ/Vr²)]`. `Vr` is the fluid's own reduced volume. Returns `None`
/// if the reduced-volume solve fails.
#[must_use]
pub fn enthalpy_departure_dimensionless(
    fluid: LkFluid,
    tr: f64,
    pr: f64,
    phase: Phase,
) -> Option<f64> {
    let k = fluid.constants();
    let vr = reduced_volume(fluid, tr, pr, phase)?;
    let z = pr * vr / tr;
    let vr2 = vr * vr;
    let e = k.c4 / (2.0 * tr * tr * tr * k.gamma)
        * (k.beta + 1.0 - (k.beta + 1.0 + k.gamma / vr2) * (-k.gamma / vr2).exp());
    let val = tr
        * (z - 1.0
            - (k.b2 + 2.0 * k.b3 / tr + 3.0 * k.b4 / (tr * tr)) / (tr * vr)
            - (k.c2 - 3.0 * k.c3 / (tr * tr)) / (2.0 * tr * vr2)
            + k.d2 / (5.0 * tr * vr.powi(5))
            + 3.0 * e);
    Some(val)
}

/// Dimensionless entropy departure `(S − S_ig) / R` [-] of one reference fluid
/// at reduced `tr, pr` [-] (`LeeKeslerPlocker.vb` L483 / L508).
///
/// `= ln Z − (b1 + b3/Tr² + 2b4/Tr³)/Vr − (c1 − 2c3/Tr³)/(2 Vr²)`
/// `  − d1/(5 Vr⁵) + 2E`, with the same `E` as
/// [`enthalpy_departure_dimensionless`]. This is the residual entropy at the
/// system `(T, P)` relative to the ideal gas at the *same* `(T, P)`
/// (`ln Z` term; DWSIM's active branch uses `Math.Log(1)` reference,
/// `LeeKeslerPlocker.vb` L483). Returns `None` if the solve fails.
#[must_use]
pub fn entropy_departure_dimensionless(
    fluid: LkFluid,
    tr: f64,
    pr: f64,
    phase: Phase,
) -> Option<f64> {
    let k = fluid.constants();
    let vr = reduced_volume(fluid, tr, pr, phase)?;
    let z = pr * vr / tr;
    let vr2 = vr * vr;
    let e = k.c4 / (2.0 * tr * tr * tr * k.gamma)
        * (k.beta + 1.0 - (k.beta + 1.0 + k.gamma / vr2) * (-k.gamma / vr2).exp());
    let val = z.ln()
        - (k.b1 + k.b3 / (tr * tr) + 2.0 * k.b4 / (tr * tr * tr)) / vr
        - (k.c1 - 2.0 * k.c3 / (tr * tr * tr)) / (2.0 * vr2)
        - k.d1 / (5.0 * vr.powi(5))
        + 2.0 * e;
    Some(val)
}

/// Pure-fluid enthalpy departure `H − H_ig` [J/mol] at reduced `tr, pr` [-],
/// acentric factor `w` [-], and critical temperature `tc` [K].
///
/// The LKP acentric interpolation of the dimensionless departure, scaled by
/// `R·Tc`: `H − H_ig = R Tc [ h⁰ + (ω/0.3978)(hʳᵉᶠ − h⁰) ]`
/// (`LeeKeslerPlocker.vb` L185/L386). `h⁰`, `hʳᵉᶠ` are
/// [`enthalpy_departure_dimensionless`] for the two reference fluids. Negative
/// for a real gas below its Boyle temperature (attraction dominates). Returns
/// `None` if a reduced-volume solve fails.
#[must_use]
pub fn enthalpy_departure(tr: f64, pr: f64, w: f64, tc: f64, phase: Phase) -> Option<f64> {
    let h0 = enthalpy_departure_dimensionless(LkFluid::Simple, tr, pr, phase)?;
    let href = enthalpy_departure_dimensionless(LkFluid::Reference, tr, pr, phase)?;
    Some(R * tc * (h0 + w / OMEGA_REF * (href - h0)))
}

/// Pure-fluid entropy departure `S − S_ig` [J/(mol·K)] at reduced `tr, pr` [-]
/// and acentric factor `w` [-].
///
/// `S − S_ig = R [ s⁰ + (ω/0.3978)(sʳᵉᶠ − s⁰) ]` (`LeeKeslerPlocker.vb`
/// L273/L510). Returns `None` if a reduced-volume solve fails.
#[must_use]
pub fn entropy_departure(tr: f64, pr: f64, w: f64, phase: Phase) -> Option<f64> {
    let s0 = entropy_departure_dimensionless(LkFluid::Simple, tr, pr, phase)?;
    let sref = entropy_departure_dimensionless(LkFluid::Reference, tr, pr, phase)?;
    Some(R * (s0 + w / OMEGA_REF * (sref - s0)))
}

/// Natural log of the pure corresponding-states fugacity coefficient `ln φ` [-]
/// at reduced `tr, pr` [-] and acentric factor `w` [-].
///
/// Per reference fluid (`LeeKeslerPlocker.vb` L841):
/// `ln φ_f = Z − 1 − ln Z + B/Vr + C/(2 Vr²) + D/(5 Vr⁵) + E`, then the LKP
/// acentric interpolation `ln φ = ln φ⁰ + (ω/0.3978)(ln φʳᵉᶠ − ln φ⁰)`
/// (`LeeKeslerPlocker.vb` L869). As `Pr → 0`, `Z → 1` and `ln φ → 0` (ideal-gas
/// limit). This is the **pure** (or pseudo-pure mixture) fugacity coefficient;
/// the per-component mixture `ln φ_i` (DWSIM `CalcLnFugCPU`) is not ported (see
/// module scope). Returns `None` if a reduced-volume solve fails.
#[must_use]
pub fn ln_phi_pure(tr: f64, pr: f64, w: f64, phase: Phase) -> Option<f64> {
    let lnphi = |fluid: LkFluid| -> Option<f64> {
        let k = fluid.constants();
        let vr = reduced_volume(fluid, tr, pr, phase)?;
        let z = pr * vr / tr;
        let vr2 = vr * vr;
        let e = k.c4 / (2.0 * tr * tr * tr * k.gamma)
            * (k.beta + 1.0 - (k.beta + 1.0 + k.gamma / vr2) * (-k.gamma / vr2).exp());
        Some(
            z - 1.0 - z.ln()
                + k.b_of_tr(tr) / vr
                + k.c_of_tr(tr) / (2.0 * vr2)
                + k.d_of_tr(tr) / (5.0 * vr.powi(5))
                + e,
        )
    };
    let l0 = lnphi(LkFluid::Simple)?;
    let lref = lnphi(LkFluid::Reference)?;
    Some(l0 + w / OMEGA_REF * (lref - l0))
}

/// Mixture pseudo-critical properties `(Tcm [K], Pcm [Pa], Vcm [m³/mol], ωm [-])`
/// by the Plöcker combining rule (`MixCritProp_LK`, `LeeKeslerPlocker.vb`
/// L93-147).
///
/// For components with mole fractions `z` [-] (summing to 1), critical
/// temperatures `Tc_i` [K], critical volumes `Vc_i` [m³/mol], and acentric
/// factors `ω_i` [-] (all read from `comps`), with a symmetric `k_ij` [-] matrix
/// (passed as `kij`; `None` → the ideal `k_ij = 1` geometric-mean `Tc`
/// combining that DWSIM defaults to, L104-110):
///
/// - `Vc_ij = (1/8)(Vc_i^{1/3} + Vc_j^{1/3})³`,
/// - `Tc_ij = √(Tc_i Tc_j) · k_ij`,
/// - `Vcm = Σ_i Σ_j z_i z_j Vc_ij`,
/// - `Tcm = (1/Vcm^{0.25}) Σ_i Σ_j z_i z_j Vc_ij^{0.25} Tc_ij`,
/// - `ωm = Σ_i z_i ω_i`,
/// - `Pcm = (0.2905 − 0.085 ωm) R Tcm / Vcm`.
///
/// **Unit note.** DWSIM stores `Vc` in **cm³/mol** and carries a `/1000` factor
/// in `Vc_ij` (L116); this port takes `Vc` in **m³/mol** (the [`Component`]
/// convention) so that `/1000` is dropped and `Pcm` comes out in Pa directly
/// with the SI `R`. The `Pcm` prefactor `(0.2905 − 0.085 ωm)` is DWSIM's
/// Plöcker pseudo-critical-compressibility correlation (L143).
///
/// `kij` must be an `n×n` symmetric slice-of-slices with `k_ii = 1` on the
/// diagonal (or `None`). Panics if `comps`, `z`, and any `kij` row disagree in
/// length.
#[must_use]
pub fn mix_crit_props(
    comps: &[Component],
    z: &[f64],
    kij: Option<&[Vec<f64>]>,
) -> (f64, f64, f64, f64) {
    let n = comps.len();
    let kget = |i: usize, j: usize| -> f64 {
        if i == j {
            return 1.0;
        }
        match kij {
            Some(m) => {
                let v = m[i][j];
                if v == 0.0 {
                    1.0
                } else {
                    v
                }
            }
            None => 1.0,
        }
    };

    let mut wm = 0.0;
    for i in 0..n {
        wm += z[i] * comps[i].acentric_factor;
    }

    // Vc_ij and Tc_ij combining matrices.
    let vc = |i: usize| comps[i].critical_volume;
    let vcjk = |i: usize, j: usize| {
        let s = vc(i).cbrt() + vc(j).cbrt();
        s * s * s / 8.0
    };
    let tcjk = |i: usize, j: usize| (comps[i].critical_temperature * comps[j].critical_temperature).sqrt() * kget(i, j);

    let mut vcm = 0.0;
    for i in 0..n {
        for j in 0..n {
            vcm += z[i] * z[j] * vcjk(i, j);
        }
    }

    let mut tcm = 0.0;
    for i in 0..n {
        for j in 0..n {
            tcm += z[i] * z[j] * vcjk(i, j).powf(0.25) * tcjk(i, j);
        }
    }
    tcm /= vcm.powf(0.25);

    let pcm = (0.2905 - 0.085 * wm) * R * tcm / vcm;
    (tcm, pcm, vcm, wm)
}

/// Lee-Kesler-Plöcker mixture compressibility factor `Z` [-] at temperature
/// `t` [K] and pressure `p` [Pa].
///
/// Maps the mixture onto its Plöcker pseudo-criticals [`mix_crit_props`], then
/// applies the pure corresponding-states [`z_lkp`] at `Tr = T/Tcm`,
/// `Pr = P/Pcm`, `ωm`. `comps` need valid `critical_volume` (m³/mol). Returns
/// `None` if a reduced-volume solve fails.
#[must_use]
pub fn z_mix(
    comps: &[Component],
    z: &[f64],
    t: f64,
    p: f64,
    phase: Phase,
    kij: Option<&[Vec<f64>]>,
) -> Option<f64> {
    let (tcm, pcm, _vcm, wm) = mix_crit_props(comps, z, kij);
    z_lkp(t / tcm, p / pcm, wm, phase)
}

/// Lee-Kesler-Plöcker mixture enthalpy departure `H − H_ig` [J/mol] at `t` [K],
/// `p` [Pa].
///
/// `H − H_ig = R Tcm [ h⁰ + (ωm/0.3978)(hʳᵉᶠ − h⁰) ]` on the pseudo-criticals
/// (`H_LK_MIX`, `LeeKeslerPlocker.vb` L185). Per **mole** of mixture (DWSIM
/// divides by the mixture molar mass to get J/kg; this port keeps J/mol,
/// matching [`crate::thermo::cubic_eos::CubicEos::enthalpy_departure`]). Returns
/// `None` if a reduced-volume solve fails.
#[must_use]
pub fn enthalpy_departure_mix(
    comps: &[Component],
    z: &[f64],
    t: f64,
    p: f64,
    phase: Phase,
    kij: Option<&[Vec<f64>]>,
) -> Option<f64> {
    let (tcm, pcm, _vcm, wm) = mix_crit_props(comps, z, kij);
    enthalpy_departure(t / tcm, p / pcm, wm, tcm, phase)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::thermo::component::reference;
    use approx::assert_relative_eq;

    /// **Methodology — LKP compressibility of a pure light gas vs an independent
    /// published analytic correlation.** The Lee-Kesler simple-fluid
    /// compressibility `Z⁰` at a fixed reduced state is compared against the
    /// **Pitzer/Prausnitz generalized second-virial correlation** (Poling,
    /// Prausnitz & O'Connell, *The Properties of Gases and Liquids*, 5th ed.
    /// 2001, Eq. 3-3.5/3-3.6; also Smith, Van Ness & Abbott), which is an
    /// independent, widely-tabulated corresponding-states reference that the
    /// Lee-Kesler chart reproduces in the low-pressure limit:
    ///
    ///   `Z⁰ = 1 + B⁰ Pr/Tr`,  `B⁰ = 0.083 − 0.422 / Tr^{1.6}`.
    ///
    /// At `Tr = 2.0`, `Pr = 0.1`: `B⁰ = 0.083 − 0.422/2^{1.6} = −0.056208`,
    /// so the reference `Z⁰ = 1 + (−0.056208)(0.1/2.0) = 0.997190`. This is well
    /// inside the region (`Tr = 2`, `Pr = 0.1`) where a light gas such as H₂,
    /// He, N₂, CH₄ or CO sits far above its critical point.
    ///
    /// **Measured result (2026-08-03):** the Lee-Kesler simple-fluid solver
    /// returns `Z⁰(Tr=2.0, Pr=0.1) = 0.997175`, agreeing with the Pitzer
    /// reference `0.997190` to `1.5e-5` (well inside the `1e-3` tolerance).
    /// The full LKP `z_lkp` with `ω = 0` returns exactly this `Z⁰`.
    #[test]
    fn lkp_simple_fluid_z_matches_pitzer_virial() {
        let (tr, pr) = (2.0, 0.1);
        let z0 = z_fluid(LkFluid::Simple, tr, pr, Phase::Vapor).unwrap();
        // Pitzer/Prausnitz generalized second-virial reference.
        let b0 = 0.083 - 0.422 / tr.powf(1.6);
        let z_ref = 1.0 + b0 * pr / tr;
        assert_relative_eq!(z_ref, 0.997190, max_relative = 1e-5);
        assert_relative_eq!(z0, z_ref, max_relative = 1e-3);
        assert_relative_eq!(z0, 0.997175, max_relative = 1e-3);

        // z_lkp with w = 0 reduces to the simple fluid exactly.
        let z_lkp0 = z_lkp(tr, pr, 0.0, Phase::Vapor).unwrap();
        assert_relative_eq!(z_lkp0, z0, epsilon = 1e-15);
    }

    /// **Methodology.** The acentric interpolation must be monotone and bracket
    /// the two reference fluids: the LKP `Z` must lie between `Z⁰` and `Zʳᵉᶠ` and
    /// equal `Z⁰ + (ω/0.3978)(Zʳᵉᶠ − Z⁰)`. Use `Tr = 1.2`, `Pr = 2.0`,
    /// `ω = 0.225` (CO₂-like).
    /// **Measured result (2026-08-03):** `Z⁰ = 0.5605`, `Zʳᵉᶠ = 0.6397` (at this
    /// dense-gas state the heavy reference fluid is *less* compressed, so
    /// `Z⁰ < Zʳᵉᶠ`); LKP `Z = 0.5605 + (0.225/0.3978)(0.6397 − 0.5605) = 0.6053`,
    /// matching the direct `z_lkp` call to `< 1e-12` and lying between the two
    /// references.
    #[test]
    fn lkp_acentric_interpolation_brackets_references() {
        let (tr, pr, w) = (1.2, 2.0, 0.225);
        let z0 = z_fluid(LkFluid::Simple, tr, pr, Phase::Vapor).unwrap();
        let zref = z_fluid(LkFluid::Reference, tr, pr, Phase::Vapor).unwrap();
        let z = z_lkp(tr, pr, w, Phase::Vapor).unwrap();
        let hand = z0 + w / OMEGA_REF * (zref - z0);
        assert_relative_eq!(z, hand, max_relative = 1e-12);
        // Interpolation lies strictly between the two reference fluids.
        assert!(z0 < z && z < zref, "z0={z0} z={z} zref={zref}");
    }

    /// **Methodology.** The enthalpy and log-fugacity departures must vanish in
    /// the ideal-gas limit `Pr → 0`. Evaluate the simple fluid at `Tr = 2.0`,
    /// `Pr = 1e-4`.
    /// **Measured result (2026-08-03):** `(H−H_ig)/(R Tc) = −2.8e-5`
    /// (|·| < 1e-3) and `ln φ = −2.9e-6` (|·| < 1e-3) — both ~0.
    #[test]
    fn lkp_departures_vanish_at_low_pressure() {
        let (tr, pr) = (2.0, 1.0e-4);
        let h = enthalpy_departure_dimensionless(LkFluid::Simple, tr, pr, Phase::Vapor).unwrap();
        assert!(h.abs() < 1e-3, "h_dimensionless {h}");
        let lnphi = ln_phi_pure(tr, pr, 0.0, Phase::Vapor).unwrap();
        assert!(lnphi.abs() < 1e-3, "ln phi {lnphi}");
    }

    /// **Methodology.** Plöcker's mixture combining rule must recover the pure
    /// component in the single-component limit: `Tcm = Tc`, `ωm = ω`, and
    /// `Vcm = Vc`. Use pure CO₂ (Tc = 304.12 K, Vc = 94.07e-6 m³/mol,
    /// ω = 0.225).
    /// **Measured result (2026-08-03):** `Tcm = 304.12 K`, `ωm = 0.225`,
    /// `Vcm = 94.07e-6 m³/mol` — all recover the pure values to `< 1e-12`; the
    /// derived `Pcm = (0.2905 − 0.085·0.225) R Tcm / Vcm = 7.29e6 Pa` (the
    /// Plöcker pseudo-critical pressure, ≈1% below CO₂'s real 7.374 MPa).
    #[test]
    fn lkp_mix_crit_recovers_pure_limit() {
        let co2 = reference::carbon_dioxide();
        let comps = [co2.clone()];
        let (tcm, pcm, vcm, wm) = mix_crit_props(&comps, &[1.0], None);
        assert_relative_eq!(tcm, co2.critical_temperature, max_relative = 1e-12);
        assert_relative_eq!(wm, co2.acentric_factor, max_relative = 1e-12);
        assert_relative_eq!(vcm, co2.critical_volume, max_relative = 1e-12);
        // Plöcker Pcm is within ~1.5% of the real Pc for CO2.
        assert!(pcm > 7.0e6 && pcm < 7.5e6, "pcm={pcm}");
    }

    /// **Methodology.** A binary of two *identical* pseudo-components must give
    /// the same mixture criticals as the pure component (composition-invariant).
    /// Use CO₂ split 40/60 into two identical copies with the default ideal
    /// `k_ij = 1`.
    /// **Measured result (2026-08-03):** `Tcm`, `Vcm`, `ωm` equal the pure CO₂
    /// values to `< 1e-12`.
    #[test]
    fn lkp_mix_crit_two_identical_components() {
        let co2 = reference::carbon_dioxide();
        let comps = [co2.clone(), co2.clone()];
        let (tcm, _pcm, vcm, wm) = mix_crit_props(&comps, &[0.4, 0.6], None);
        assert_relative_eq!(tcm, co2.critical_temperature, max_relative = 1e-12);
        assert_relative_eq!(vcm, co2.critical_volume, max_relative = 1e-12);
        assert_relative_eq!(wm, co2.acentric_factor, max_relative = 1e-12);
    }
}
