//! Single-component (pure-fluid) **saturation-shortcut** flash.
//!
//! For a one-component system — or a degenerate multicomponent feed dominated by
//! a single effective component — a full multicomponent VLE flash is
//! unnecessary. The phase split at a given temperature `T` \[K\] and pressure
//! `P` \[Pa\] is decided entirely by the pure-fluid saturation curve:
//!
//! ```text
//! P < Psat(T)  ->  all vapour   (V = 1)
//! P > Psat(T)  ->  all liquid   (V = 0)
//! P = Psat(T)  ->  two-phase at the specified vapour fraction V in [0, 1]
//! ```
//!
//! where `Psat(T)` is the pure-component vapour pressure and `Tsat(P)` its
//! inverse (the saturation temperature). This module reproduces DWSIM's
//! `SingleCompFlash` VLE logic on top of the already-ported
//! [`crate::thermo::saturation`] bubble/dew kernel, which supplies `Psat`/`Tsat`
//! (for a pure feed `z = [1]` the bubble point, the dew point, and the vapour
//! pressure all coincide — the pressure/temperature at which `K = 1`).
//!
//! ## Provenance (GPLv3)
//!
//! Ported from **DWSIM** (GPL-3.0):
//! `DWSIM.Thermodynamics/FlashAlgorithms/SingleCompFlash.vb`, commit `1abf72d`.
//! Copyright 2021 Daniel Wagner O. de Medeiros; DWSIM is distributed under the
//! GNU General Public License v3. This independent OUTRAM PARK fork is GPL-3.0.
//! Per-function line citations (`SingleCompFlash.vb:<line>`) appear at each item.
//!
//! ## What is ported (and what is not) — honest scope
//!
//! DWSIM's `SingleCompFlash` handles vapour, liquid, **and solid** phases
//! (sublimation, melting/freezing, forced solids, and a special CO₂ triple-point
//! guard). This port covers the **vapour–liquid** shortcut only:
//!
//! - [`flash_pt`] — phase state at `(T, P)` from `Psat(T)` vs `P`
//!   (`SingleCompFlash.vb:59`, the non-solid `If Pvap > P` / `Else` branches).
//! - [`flash_tv`] — saturation pressure `Psat(T)` at a specified vapour fraction
//!   (`SingleCompFlash.vb:290`, the `T > Tfus` liquid+vapour branch).
//! - [`flash_pv`] — saturation temperature `Tsat(P)` at a specified vapour
//!   fraction (`SingleCompFlash.vb:306`, the `Tsat > Tfus` liquid+vapour branch).
//! - [`flash_ph`] — pressure–enthalpy flash: superheated-vapour / two-phase /
//!   subcooled-liquid classification against the saturated enthalpies, with a
//!   single-phase temperature solve (`SingleCompFlash.vb:80`, the non-solid
//!   `H >= HsatV` / `H >= HsatL` / `Else` branches).
//!
//! **Deliberately NOT ported** (documented, not silently dropped): every solid
//! branch — sublimation, partial/complete freezing and melting, `IsSolid` /
//! `ForcedSolids`, the fusion enthalpy `Hfus`, the `RET_VTF` fusion temperature,
//! the CO₂ triple-point guard — and the PS (pressure–entropy) flash. A feed
//! below its triple point is therefore out of scope here; the VLE shortcut
//! assumes the fluid is at or above its melting line.
//!
//! ## Decoupling — no `dyn`, no `Box`, no lifetimes
//!
//! `Psat`/`Tsat` come from a [`PropertyPackageModel`] (enum dispatch; the
//! `Ideal` package gives the Wilson vapour pressure, a cubic package gives the
//! EOS saturation pressure). The PH-flash's one model-dependent step — the molar
//! enthalpy of a phase at `(T, P)` — is a **caller-supplied generic `Fn`
//! closure** (`Fn(T, P, Phase) -> f64`), never a trait object, mirroring
//! [`crate::thermo::energy_flash`] and the crate's push-to-caller pattern.
//!
//! ## Units — documented raw `f64` (SI), per the crate `CLAUDE.md`
//!
//! Temperature `T` \[K\], pressure `P` \[Pa\], molar enthalpy \[J/mol\], mole
//! fractions and vapour fraction \[-\], all `f64` in SI base units — the same
//! convention as the [`crate::thermo::flash`] / [`crate::thermo::saturation`]
//! kernel this sits on. Every parameter's unit is spelled out in its doc comment.
//!
//! ## V&V status
//!
//! **Untrusted AI-assisted draft pending human V&V.** The inline tests are
//! *verification* (internal consistency against the defining saturation
//! relations and hand-computed analytic enthalpy cases), **not** validation
//! against experimental / NIST / DECHEMA saturation data. Not for nuclear
//! facility operation, reactor control, safety-critical, or licensing decisions.
//! Independent OUTRAM PARK fork, not the official DWSIM.

#![forbid(unsafe_code)]

use crate::thermo::cubic_eos::Phase;
use crate::thermo::property_package::PropertyPackageModel;
use crate::thermo::saturation::{
    bubble_pressure, bubble_temperature, SaturationError,
};
use crate::thermo::Component;

/// The equilibrium phase state a single-component flash resolves to.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SingleCompPhase {
    /// All vapour (`V = 1`): the specification lies above the saturation curve
    /// (`P < Psat(T)`, i.e. superheated).
    Vapour,
    /// All liquid (`V = 0`): the specification lies below the saturation curve
    /// (`P > Psat(T)`, i.e. subcooled).
    Liquid,
    /// Two coexisting phases (`0 <= V <= 1`) on the saturation curve
    /// (`P = Psat(T)`).
    TwoPhase,
}

/// A resolved single-component flash state.
///
/// All fields are SI: temperatures \[K\], pressures \[Pa\], fractions \[-\].
/// Exactly which of `temperature` / `pressure` was an input versus a solved
/// unknown depends on the entry point (see each `flash_*` function).
#[derive(Debug, Clone, PartialEq)]
pub struct SingleCompResult {
    /// Vapour molar fraction `V` \[-\] in `[0, 1]` (moles vapour per mole feed):
    /// `1` = all vapour, `0` = all liquid, interior = two-phase.
    pub vapour_fraction: f64,
    /// Liquid molar fraction `1 - V` \[-\] in `[0, 1]`.
    pub liquid_fraction: f64,
    /// Temperature `T` \[K\] of the resolved state (an input for [`flash_pt`] /
    /// [`flash_tv`]; solved for [`flash_pv`] / [`flash_ph`]).
    pub temperature: f64,
    /// Pressure `P` \[Pa\] of the resolved state (an input for [`flash_pt`] /
    /// [`flash_pv`] / [`flash_ph`]; solved — `= Psat(T)` — for [`flash_tv`]).
    pub pressure: f64,
    /// Pure-component saturation pressure `Psat` \[Pa\] evaluated at
    /// `temperature`, from [`crate::thermo::saturation`]. On the saturation
    /// curve this equals `pressure` to solver tolerance.
    pub saturation_pressure: f64,
    /// The resolved [`SingleCompPhase`].
    pub phase: SingleCompPhase,
}

/// Tuning parameters for the single-component flashes.
///
/// Only [`flash_ph`] uses the enthalpy-solve fields (`h_tol`, `t_min`, `t_max`,
/// `max_iter`); the other entry points are closed-form on top of the saturation
/// kernel and ignore them.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct SingleCompOptions {
    /// Absolute convergence tolerance on the enthalpy residual
    /// `|H(T) - H_target|` \[J/mol\] for the [`flash_ph`] single-phase solve.
    pub h_tol: f64,
    /// Lower bound of the [`flash_ph`] single-phase temperature search \[K\].
    pub t_min: f64,
    /// Upper bound of the [`flash_ph`] single-phase temperature search \[K\].
    pub t_max: f64,
    /// Maximum bisection iterations for the [`flash_ph`] single-phase solve.
    pub max_iter: usize,
}

impl Default for SingleCompOptions {
    fn default() -> Self {
        Self {
            h_tol: 1.0e-6,
            t_min: 1.0,
            t_max: 2.0e4,
            max_iter: 200,
        }
    }
}

/// Error conditions for the single-component flashes.
#[derive(Debug, Clone, PartialEq, thiserror::Error)]
pub enum SingleCompError {
    /// An empty feed was supplied (need at least one component).
    #[error("empty composition")]
    Empty,
    /// `components` and `z` were different lengths.
    #[error("slice length mismatch: components {a} vs composition {b}")]
    LengthMismatch {
        /// Length of `components`.
        a: usize,
        /// Length of `z`.
        b: usize,
    },
    /// A non-finite value (`NaN`/`inf`) appeared in an input.
    #[error("non-finite input value")]
    NonFinite,
    /// A quantity that must be strictly positive was not.
    #[error("`{what}` must be finite and > 0 (got {value})")]
    NonPositive {
        /// Which quantity (e.g. `"pressure"`).
        what: &'static str,
        /// The offending value.
        value: f64,
    },
    /// A specified vapour fraction `V` was outside `[0, 1]`.
    #[error("vapour fraction must be in [0, 1] (got {value})")]
    VapourFractionOutOfRange {
        /// The offending vapour fraction.
        value: f64,
    },
    /// The pure-component saturation solve ([`crate::thermo::saturation`])
    /// failed while computing `Psat`/`Tsat`.
    #[error("saturation solve failed: {0}")]
    Saturation(#[from] SaturationError),
    /// The [`flash_ph`] single-phase temperature solve could not bracket the
    /// target enthalpy within `[t_min, t_max]` (target unreachable in-window).
    #[error("could not bracket enthalpy target in [{t_min}, {t_max}] K")]
    NoBracket {
        /// Search-window floor \[K\].
        t_min: f64,
        /// Search-window ceiling \[K\].
        t_max: f64,
    },
    /// The [`flash_ph`] single-phase temperature solve did not reach `h_tol`
    /// within `max_iter` iterations.
    #[error("enthalpy solve did not converge in {iterations} iterations (residual {residual:e})")]
    NotConverged {
        /// Iterations performed.
        iterations: usize,
        /// Final `|H(T) - H_target|` \[J/mol\].
        residual: f64,
    },
}

// ---------------------------------------------------------------------------
// Input handling
// ---------------------------------------------------------------------------

/// Index of the dominant (effective single) component: the first `i` with
/// `z_i > 0.9`, else index `0`.
///
/// Ported verbatim from DWSIM `SingleCompFlash.vb:45` (`GetIndex`): the routine
/// scans for the component making up more than 90 % of the feed and treats the
/// system as that pure fluid. For a genuinely pure feed `z = [1]` it returns `0`.
fn dominant_index(z: &[f64]) -> usize {
    for (i, &zi) in z.iter().enumerate() {
        if zi > 0.9 {
            return i;
        }
    }
    0
}

/// Validate the `(components, z)` pairing and return the dominant component.
fn resolve_component<'a>(
    components: &'a [Component],
    z: &[f64],
) -> Result<&'a Component, SingleCompError> {
    if components.is_empty() || z.is_empty() {
        return Err(SingleCompError::Empty);
    }
    if components.len() != z.len() {
        return Err(SingleCompError::LengthMismatch {
            a: components.len(),
            b: z.len(),
        });
    }
    if z.iter().any(|v| !v.is_finite()) {
        return Err(SingleCompError::NonFinite);
    }
    Ok(&components[dominant_index(z)])
}

fn require_positive(what: &'static str, value: f64) -> Result<(), SingleCompError> {
    if !value.is_finite() || value <= 0.0 {
        Err(SingleCompError::NonPositive { what, value })
    } else {
        Ok(())
    }
}

fn require_vapour_fraction(v: f64) -> Result<(), SingleCompError> {
    if !v.is_finite() || !(0.0..=1.0).contains(&v) {
        Err(SingleCompError::VapourFractionOutOfRange { value: v })
    } else {
        Ok(())
    }
}

// ---------------------------------------------------------------------------
// Saturation helpers (reuse thermo::saturation for Psat / Tsat)
// ---------------------------------------------------------------------------

/// Pure-component **saturation pressure** `Psat(T)` \[Pa\] of the dominant
/// component of feed `z` at temperature `T` \[K\], using `package` for the
/// K-model.
///
/// DWSIM analogue: `PP.AUX_PVAPi(idx, T)` (`SingleCompFlash.vb:62`). Here it is
/// the bubble pressure of the pure feed `z = [1]` from
/// [`crate::thermo::saturation::bubble_pressure`] — for a single component the
/// bubble point, dew point, and vapour pressure coincide (the pressure at which
/// `K = 1`). With [`PropertyPackageModel::Ideal`] this is the Wilson vapour
/// pressure `Psat = Pc·exp[5.373(1+ω)(1 − Tc/T)]`; with a cubic package it is
/// the EOS saturation pressure (equal-fugacity `φ^L = φ^V`).
///
/// # Units / ranges
/// `components.len() == z.len()`, `z` mole fractions \[-\], `temperature` `T`
/// \[K\] > 0. Returns `Psat` \[Pa\].
///
/// # Errors
/// [`SingleCompError::Empty`] / [`SingleCompError::LengthMismatch`] /
/// [`SingleCompError::NonFinite`] on bad inputs, [`SingleCompError::NonPositive`]
/// for `T <= 0`, and [`SingleCompError::Saturation`] if the bubble-pressure
/// solve fails.
pub fn saturation_pressure(
    components: &[Component],
    z: &[f64],
    temperature: f64,
    package: PropertyPackageModel,
) -> Result<f64, SingleCompError> {
    let comp = resolve_component(components, z)?;
    require_positive("temperature", temperature)?;
    let state = bubble_pressure(std::slice::from_ref(comp), &[1.0], temperature, package)?;
    Ok(state.pressure)
}

/// Pure-component **saturation temperature** `Tsat(P)` \[K\] of the dominant
/// component of feed `z` at pressure `P` \[Pa\], using `package` for the K-model.
///
/// DWSIM analogue: `PP.AUX_TSATi(P, idx)` (`SingleCompFlash.vb:86`). Here it is
/// the bubble temperature of the pure feed `z = [1]` from
/// [`crate::thermo::saturation::bubble_temperature`] — the inverse of
/// [`saturation_pressure`].
///
/// # Units / ranges / errors
/// As [`saturation_pressure`], with `pressure` `P` \[Pa\] > 0; returns `Tsat`
/// \[K\].
pub fn saturation_temperature(
    components: &[Component],
    z: &[f64],
    pressure: f64,
    package: PropertyPackageModel,
) -> Result<f64, SingleCompError> {
    let comp = resolve_component(components, z)?;
    require_positive("pressure", pressure)?;
    let state = bubble_temperature(std::slice::from_ref(comp), &[1.0], pressure, package)?;
    Ok(state.temperature)
}

// ---------------------------------------------------------------------------
// PT flash
// ---------------------------------------------------------------------------

/// **Pressure–temperature** single-component flash: resolve the phase state at
/// `(T, P)` from the saturation curve.
///
/// Ported from DWSIM `SingleCompFlash.vb:59` (`Flash_PT`, the non-solid path):
/// compute `Pvap = Psat(T)` and compare to `P`. `Pvap > P` ⇒ superheated vapour
/// (`V = 1`, `SingleCompFlash.vb:69-70`); otherwise subcooled liquid (`V = 0`,
/// `SingleCompFlash.vb:73-74`). At exact equality `Pvap == P` the split is
/// on the saturation line and reported as [`SingleCompPhase::TwoPhase`]; the
/// vapour fraction is undetermined by a PT specification alone and is reported
/// as `0` (specify it with [`flash_tv`] / [`flash_pv`] instead). The solid
/// branches (`IsSolid`, `T < Tfus`) are out of scope — see the module header.
///
/// # Units / ranges
/// `components.len() == z.len()`; `z` mole fractions \[-\]; `pressure` `P` \[Pa\]
/// > 0; `temperature` `T` \[K\] > 0. The returned [`SingleCompResult`] carries
/// the input `T`, `P`, the classified phase, and `Psat(T)` in
/// `saturation_pressure`.
///
/// # Errors
/// [`SingleCompError::Empty`] / [`SingleCompError::LengthMismatch`] /
/// [`SingleCompError::NonFinite`] / [`SingleCompError::NonPositive`] on bad
/// inputs; [`SingleCompError::Saturation`] if the `Psat` solve fails.
pub fn flash_pt(
    components: &[Component],
    z: &[f64],
    pressure: f64,
    temperature: f64,
    package: PropertyPackageModel,
) -> Result<SingleCompResult, SingleCompError> {
    require_positive("pressure", pressure)?;
    require_positive("temperature", temperature)?;
    let psat = saturation_pressure(components, z, temperature, package)?;

    // Relative tolerance for the exact-saturation (two-phase) band.
    let on_curve = (psat - pressure).abs() <= 1.0e-9 * pressure.max(1.0);
    let (vapour_fraction, phase) = if on_curve {
        (0.0, SingleCompPhase::TwoPhase)
    } else if psat > pressure {
        (1.0, SingleCompPhase::Vapour)
    } else {
        (0.0, SingleCompPhase::Liquid)
    };

    Ok(SingleCompResult {
        vapour_fraction,
        liquid_fraction: 1.0 - vapour_fraction,
        temperature,
        pressure,
        saturation_pressure: psat,
        phase,
    })
}

// ---------------------------------------------------------------------------
// TV flash
// ---------------------------------------------------------------------------

/// **Temperature–vapour-fraction** single-component flash: return the
/// saturation pressure `Psat(T)` at a specified vapour fraction `V`.
///
/// Ported from DWSIM `SingleCompFlash.vb:290` (`Flash_TV`, the `T > Tfus`
/// liquid+vapour branch): for a pure fluid the equilibrium pressure of a
/// two-phase state at temperature `T` is fixed by the saturation curve
/// (`Psat(T)`), independent of `V`; `V` only sets how the feed is partitioned.
/// The solid+vapour branch (`T <= Tfus`) is out of scope.
///
/// # Units / ranges
/// `temperature` `T` \[K\] > 0; `vapour_fraction` `V` \[-\] in `[0, 1]`. The
/// returned [`SingleCompResult`] has `pressure = saturation_pressure = Psat(T)`
/// and the phase set from `V` (`1` → vapour, `0` → liquid, interior → two-phase).
///
/// # Errors
/// As [`flash_pt`], plus [`SingleCompError::VapourFractionOutOfRange`] for
/// `V ∉ [0, 1]`.
pub fn flash_tv(
    components: &[Component],
    z: &[f64],
    temperature: f64,
    vapour_fraction: f64,
    package: PropertyPackageModel,
) -> Result<SingleCompResult, SingleCompError> {
    require_positive("temperature", temperature)?;
    require_vapour_fraction(vapour_fraction)?;
    let psat = saturation_pressure(components, z, temperature, package)?;
    Ok(SingleCompResult {
        vapour_fraction,
        liquid_fraction: 1.0 - vapour_fraction,
        temperature,
        pressure: psat,
        saturation_pressure: psat,
        phase: phase_from_fraction(vapour_fraction),
    })
}

// ---------------------------------------------------------------------------
// PV flash
// ---------------------------------------------------------------------------

/// **Pressure–vapour-fraction** single-component flash: return the saturation
/// temperature `Tsat(P)` at a specified vapour fraction `V`.
///
/// Ported from DWSIM `SingleCompFlash.vb:306` (`Flash_PV`, the `Tsat > Tfus`
/// liquid+vapour branch): the two-phase temperature of a pure fluid at pressure
/// `P` is `Tsat(P)`, independent of `V`. The solid+vapour branch is out of scope.
///
/// # Units / ranges
/// `pressure` `P` \[Pa\] > 0; `vapour_fraction` `V` \[-\] in `[0, 1]`. The
/// returned [`SingleCompResult`] has `temperature = Tsat(P)`, `pressure = P`,
/// `saturation_pressure = Psat(Tsat(P)) ≈ P`, and the phase set from `V`.
///
/// # Errors
/// As [`flash_tv`] (with `pressure` positivity instead of `temperature`).
pub fn flash_pv(
    components: &[Component],
    z: &[f64],
    pressure: f64,
    vapour_fraction: f64,
    package: PropertyPackageModel,
) -> Result<SingleCompResult, SingleCompError> {
    require_positive("pressure", pressure)?;
    require_vapour_fraction(vapour_fraction)?;
    let tsat = saturation_temperature(components, z, pressure, package)?;
    // Psat at the solved Tsat closes back on P (round-trip); report it for the
    // caller's consistency checks.
    let psat = saturation_pressure(components, z, tsat, package)?;
    Ok(SingleCompResult {
        vapour_fraction,
        liquid_fraction: 1.0 - vapour_fraction,
        temperature: tsat,
        pressure,
        saturation_pressure: psat,
        phase: phase_from_fraction(vapour_fraction),
    })
}

/// Classify a phase from a vapour fraction: `1` → vapour, `0` → liquid,
/// interior → two-phase.
fn phase_from_fraction(v: f64) -> SingleCompPhase {
    if v >= 1.0 {
        SingleCompPhase::Vapour
    } else if v <= 0.0 {
        SingleCompPhase::Liquid
    } else {
        SingleCompPhase::TwoPhase
    }
}

// ---------------------------------------------------------------------------
// PH flash
// ---------------------------------------------------------------------------

/// **Pressure–enthalpy** single-component flash: resolve temperature and vapour
/// fraction so the molar enthalpy meets `h_target` at fixed pressure `P`.
///
/// Ported from DWSIM `SingleCompFlash.vb:80` (`Flash_PH`), non-solid path only
/// (`SingleCompFlash.vb:151-199`). The method:
///
/// 1. `Tsat = Tsat(P)` ([`saturation_temperature`], `SingleCompFlash.vb:86`).
/// 2. Saturated molar enthalpies `HsatV = h(Tsat, P, Vapor)` and
///    `HsatL = h(Tsat, P, Liquid)` (`SingleCompFlash.vb:92-93`).
/// 3. Classify against the target `H`:
///    - `H >= HsatV` ⇒ **superheated vapour** (`V = 1`); solve `h(T,P,Vapor) = H`
///      for `T ≥ Tsat` (`SingleCompFlash.vb:151-158`).
///    - `HsatL <= H < HsatV` ⇒ **two-phase** at `T = Tsat` with
///      `V = (H − HsatL)/(HsatV − HsatL)` (`SingleCompFlash.vb:159-163`).
///    - `H < HsatL` ⇒ **subcooled liquid** (`V = 0`); solve `h(T,P,Liquid) = H`
///      for `T ≤ Tsat` (`SingleCompFlash.vb:191-199`).
///
/// The saturated latent heat `HsatV − HsatL` must be > 0 for the two-phase
/// branch (it is, for `T` below the critical point); if it is non-positive the
/// state is treated as single-phase by the `>=`/`<` comparisons.
///
/// ## The enthalpy closure (model-dependent step, no `dyn`)
///
/// `molar_enthalpy(T, P, Phase) -> f64` returns the molar enthalpy \[J/mol\] of
/// the pure fluid in the given phase at `(T, P)` on **whatever reference scale
/// the caller uses** — the classification and the interior `V` depend only on
/// enthalpy *differences*, so any consistent datum works. It is a generic `Fn`,
/// not a trait object (crate no-`dyn` rule). A natural choice wraps the
/// ideal-gas Cp0 integral plus a cubic-EOS departure (see
/// [`crate::thermo::energy_flash`]); it must be monotone increasing in `T`
/// within a phase for the single-phase bisection to converge.
///
/// # Units / ranges
/// `components.len() == z.len()`; `pressure` `P` \[Pa\] > 0; `h_target` \[J/mol\]
/// on the closure's enthalpy scale. The returned [`SingleCompResult`] carries
/// the solved `T` (or `Tsat` in the two-phase branch), the input `P`, the vapour
/// fraction, and the classified phase. `saturation_pressure` reports
/// `Psat(Tsat(P))` (the vapour pressure at the boiling point for `P`, which
/// round-trips to `P`) — *not* `Psat` at the solved single-phase `T`, which
/// would be ill-posed for a deeply subcooled liquid.
///
/// # Errors
/// Input-validation errors as [`flash_pt`]; [`SingleCompError::Saturation`] if
/// the `Tsat`/`Psat` solve fails; [`SingleCompError::NoBracket`] /
/// [`SingleCompError::NotConverged`] if the single-phase temperature solve
/// cannot bracket or reach `h_target` within `opts`.
pub fn flash_ph<H>(
    components: &[Component],
    z: &[f64],
    pressure: f64,
    h_target: f64,
    package: PropertyPackageModel,
    molar_enthalpy: H,
    opts: SingleCompOptions,
) -> Result<SingleCompResult, SingleCompError>
where
    H: Fn(f64, f64, Phase) -> f64,
{
    require_positive("pressure", pressure)?;
    if !h_target.is_finite() {
        return Err(SingleCompError::NonFinite);
    }
    let tsat = saturation_temperature(components, z, pressure, package)?;

    let h_sat_v = molar_enthalpy(tsat, pressure, Phase::Vapor);
    let h_sat_l = molar_enthalpy(tsat, pressure, Phase::Liquid);

    let (temperature, vapour_fraction, phase) = if h_target >= h_sat_v {
        // Superheated vapour: solve h(T, P, Vapor) = H for T in [Tsat, t_max].
        let t = solve_single_phase_temperature(
            &molar_enthalpy,
            pressure,
            h_target,
            Phase::Vapor,
            tsat,
            opts.t_max,
            &opts,
        )?;
        (t, 1.0, SingleCompPhase::Vapour)
    } else if h_target >= h_sat_l && h_sat_v > h_sat_l {
        // Two-phase at Tsat: partial vaporization from saturated liquid.
        let v = (h_target - h_sat_l) / (h_sat_v - h_sat_l);
        (tsat, v, SingleCompPhase::TwoPhase)
    } else {
        // Subcooled liquid: solve h(T, P, Liquid) = H for T in [t_min, Tsat].
        let t = solve_single_phase_temperature(
            &molar_enthalpy,
            pressure,
            h_target,
            Phase::Liquid,
            opts.t_min,
            tsat,
            &opts,
        )?;
        (t, 0.0, SingleCompPhase::Liquid)
    };

    // Report the saturation pressure at the phase boundary that classified the
    // state — `Psat(Tsat(P))`, which round-trips to `P`. (Recomputing `Psat` at
    // the *solved* single-phase `T` would be ill-posed for a deeply subcooled
    // liquid, whose vapour pressure can fall below the saturation solver's
    // pressure floor.)
    let psat = saturation_pressure(components, z, tsat, package)?;
    Ok(SingleCompResult {
        vapour_fraction,
        liquid_fraction: 1.0 - vapour_fraction,
        temperature,
        pressure,
        saturation_pressure: psat,
        phase,
    })
}

/// Bisection solve of `molar_enthalpy(T, P, phase) = h_target` for `T` in
/// `[lo, hi]`, assuming the closure is monotone increasing in `T`.
///
/// Bisection (not Newton) is used because the caller's enthalpy closure need not
/// expose a derivative; it is unconditionally convergent on a sign-changing
/// bracket. Returns [`SingleCompError::NoBracket`] if `h_target` is not bracketed
/// by `[lo, hi]`, [`SingleCompError::NotConverged`] if `max_iter` is exhausted.
#[allow(clippy::too_many_arguments)]
fn solve_single_phase_temperature<H>(
    molar_enthalpy: &H,
    pressure: f64,
    h_target: f64,
    phase: Phase,
    lo: f64,
    hi: f64,
    opts: &SingleCompOptions,
) -> Result<f64, SingleCompError>
where
    H: Fn(f64, f64, Phase) -> f64,
{
    let mut a = lo;
    let mut b = hi;
    let f = |t: f64| molar_enthalpy(t, pressure, phase) - h_target;
    let mut fa = f(a);
    let fb = f(b);

    if fa.abs() <= opts.h_tol {
        return Ok(a);
    }
    if fb.abs() <= opts.h_tol {
        return Ok(b);
    }
    if !fa.is_finite() || !fb.is_finite() {
        return Err(SingleCompError::NonFinite);
    }
    if fa * fb > 0.0 {
        return Err(SingleCompError::NoBracket {
            t_min: opts.t_min,
            t_max: opts.t_max,
        });
    }

    let mut mid = 0.5 * (a + b);
    for _ in 0..opts.max_iter {
        mid = 0.5 * (a + b);
        let fm = f(mid);
        if !fm.is_finite() {
            return Err(SingleCompError::NonFinite);
        }
        if fm.abs() <= opts.h_tol {
            return Ok(mid);
        }
        // Keep the sign-changing half (fa carries the current left-end sign).
        if fa * fm < 0.0 {
            b = mid;
        } else {
            a = mid;
            fa = fm;
        }
        // Also stop when the interval itself has collapsed.
        if (b - a).abs() <= 1.0e-12 * mid.abs().max(1.0) {
            return Ok(mid);
        }
    }
    Err(SingleCompError::NotConverged {
        iterations: opts.max_iter,
        residual: f(mid).abs(),
    })
}

#[cfg(test)]
mod tests {
    //! # V&V — verification of the single-component saturation-shortcut flash
    //!
    //! **Methodology (shared).** Every test checks a *defining relation* of the
    //! pure-fluid saturation problem or a hand-computed analytic enthalpy case —
    //! **not** experimental / NIST saturation data. `Psat`/`Tsat` come from the
    //! already-verified [`crate::thermo::saturation`] kernel with the
    //! [`PropertyPackageModel::Ideal`] (Wilson) K-model, for which `Psat(T)` is
    //! the closed-form Wilson vapour pressure. Critical constants and normal
    //! boiling points are the public-literature
    //! [`reference`](crate::thermo::component::reference) presets (Poling,
    //! Prausnitz & O'Connell, *The Properties of Gases and Liquids*, 5th ed.,
    //! 2001, Appendix A).
    //!
    //! **Scope (honesty).** Verification ("did we implement the saturation
    //! shortcut correctly?"), NOT validation against measured saturation data.
    //! Vapour–liquid only; no solid/sublimation/freezing branch. Numbers below
    //! were measured on **2026-08-03** (this port), release build. Untrusted
    //! AI-assisted draft pending human V&V.

    use super::*;
    use crate::thermo::component::reference;
    use approx::assert_abs_diff_eq;

    /// **Methodology.** Below the saturation curve (`P < Psat(T)`) a PT flash
    /// must return all vapour (`V = 1`); above it (`P > Psat(T)`), all liquid
    /// (`V = 0`). Pure methane, `z = [1]`. `Psat(150 K)` is the Wilson vapour
    /// pressure. Test at half and twice that pressure.
    /// **Results (2026-08-03, release):** `Psat(150 K) = 1.058654e6 Pa`; at
    /// `P = 5.293e5 Pa` (< Psat) → `V = 1` (Vapour); at `P = 2.117e6 Pa`
    /// (> Psat) → `V = 0` (Liquid). The reported `saturation_pressure` equals the
    /// saturation-kernel value exactly.
    #[test]
    fn pt_flash_below_psat_is_vapour_above_is_liquid() {
        let comps = [reference::methane()];
        let z = [1.0];
        let t = 150.0;
        let psat = saturation_pressure(&comps, &z, t, PropertyPackageModel::Ideal).unwrap();

        let vap = flash_pt(&comps, &z, 0.5 * psat, t, PropertyPackageModel::Ideal).unwrap();
        assert_eq!(vap.phase, SingleCompPhase::Vapour);
        assert_abs_diff_eq!(vap.vapour_fraction, 1.0, epsilon = 1e-15);
        assert_abs_diff_eq!(vap.saturation_pressure, psat, epsilon = 1e-9);

        let liq = flash_pt(&comps, &z, 2.0 * psat, t, PropertyPackageModel::Ideal).unwrap();
        assert_eq!(liq.phase, SingleCompPhase::Liquid);
        assert_abs_diff_eq!(liq.vapour_fraction, 0.0, epsilon = 1e-15);
    }

    /// **Methodology.** At exactly `P = Psat(T)` the PT flash lies on the
    /// saturation line and must classify as two-phase, and the reported `Psat`
    /// must match the [`crate::thermo::saturation`] value to solver tolerance.
    /// Pure methane at `T = 150 K`, `P` set equal to `Psat(150 K)`.
    /// **Results (2026-08-03, release):** phase = TwoPhase; reported
    /// `saturation_pressure = 1.058654e6 Pa`, matching the saturation kernel to
    /// < 1e-6 Pa.
    #[test]
    fn pt_flash_on_curve_is_two_phase_and_psat_matches_saturation() {
        let comps = [reference::methane()];
        let z = [1.0];
        let t = 150.0;
        let psat = saturation_pressure(&comps, &z, t, PropertyPackageModel::Ideal).unwrap();

        let r = flash_pt(&comps, &z, psat, t, PropertyPackageModel::Ideal).unwrap();
        assert_eq!(r.phase, SingleCompPhase::TwoPhase);
        // Independent recomputation via the saturation kernel.
        let ref_psat = bubble_pressure(&comps, &[1.0], t, PropertyPackageModel::Ideal)
            .unwrap()
            .pressure;
        assert_abs_diff_eq!(r.saturation_pressure, ref_psat, epsilon = 1e-6);
    }

    /// **Methodology.** A pure fluid at its normal boiling point `Tb` must have
    /// a saturation pressure near 1 atm (101325 Pa) — that is the definition of
    /// `Tb`. The magnitude of the deviation here is the **Wilson vapour-pressure
    /// approximation error**, not a bug: Wilson `Psat = Pc·exp[5.373(1+ω)(1 −
    /// Tc/Tb)]` is a two-parameter estimate, exact only near `Tc`. Fluid:
    /// **methane**, `Tb = 111.66 K` (Poling et al. 2001).
    /// **Results (2026-08-03, release):** `Psat(Tb) = 9.900986e4 Pa`
    /// (99.01 kPa) — 2.29 % below 1 atm, the expected Wilson error for a
    /// non-polar fluid near its boiling point. Asserted within 10 % of 101325 Pa.
    #[test]
    fn normal_boiling_point_gives_atmospheric_saturation_pressure() {
        let comps = [reference::methane()];
        let z = [1.0];
        let tb = comps[0].normal_boiling_point; // 111.66 K
        let psat = saturation_pressure(&comps, &z, tb, PropertyPackageModel::Ideal).unwrap();
        // Within 10 % of 1 atm — the residual is the Wilson approximation error.
        assert!(
            (psat - 101_325.0).abs() / 101_325.0 < 0.10,
            "Psat(Tb) = {psat} Pa not near 1 atm"
        );
        // Pin the measured value (documents the actual Wilson result).
        assert_abs_diff_eq!(psat, 9.900986e4, epsilon = 1.0);
    }

    /// **Methodology.** Round-trip consistency `Tsat(Psat(T)) == T`: take a
    /// temperature, compute its saturation pressure, then back out the
    /// saturation temperature from that pressure — it must return the original
    /// `T`. Exercises both saturation directions used by [`flash_pv`] /
    /// [`flash_ph`]. Pure methane, `T = 160 K`.
    /// **Results (2026-08-03, release):** `Psat(160 K) = 1.629555e6 Pa`;
    /// `Tsat(1.629555e6 Pa) = 160.000000 K`, round-trip error < 1e-6 K.
    #[test]
    fn tsat_of_psat_round_trips_temperature() {
        let comps = [reference::methane()];
        let z = [1.0];
        let t = 160.0;
        let psat = saturation_pressure(&comps, &z, t, PropertyPackageModel::Ideal).unwrap();
        let t_back = saturation_temperature(&comps, &z, psat, PropertyPackageModel::Ideal).unwrap();
        assert_abs_diff_eq!(t_back, t, epsilon = 1e-6);
    }

    /// **Methodology.** [`flash_tv`] returns `Psat(T)` at a specified `V`, and
    /// [`flash_pv`] returns `Tsat(P)`; the two must be mutually consistent. Solve
    /// `flash_tv` at `T = 150 K`, `V = 0.4` to get `P = Psat`, then `flash_pv`
    /// at that `P`, `V = 0.4` must recover `T = 150 K`. Pure methane.
    /// **Results (2026-08-03, release):** `flash_tv` → `P = 1.058654e6 Pa`,
    /// phase = TwoPhase; `flash_pv` → `T = 150.000000 K` (error < 1e-6 K),
    /// `saturation_pressure` closes back on `P` to < 1e-3 Pa; both carry
    /// `V = 0.4`.
    #[test]
    fn tv_and_pv_are_mutually_consistent() {
        let comps = [reference::methane()];
        let z = [1.0];
        let t = 150.0;
        let v = 0.4;

        let tv = flash_tv(&comps, &z, t, v, PropertyPackageModel::Ideal).unwrap();
        assert_eq!(tv.phase, SingleCompPhase::TwoPhase);
        assert_abs_diff_eq!(tv.vapour_fraction, v, epsilon = 1e-15);

        let pv = flash_pv(&comps, &z, tv.pressure, v, PropertyPackageModel::Ideal).unwrap();
        assert_abs_diff_eq!(pv.temperature, t, epsilon = 1e-6);
        assert_abs_diff_eq!(pv.pressure, tv.pressure, epsilon = 1e-6);
        assert_abs_diff_eq!(pv.saturation_pressure, tv.pressure, epsilon = 1e-3);
    }

    /// **Methodology.** [`flash_ph`] two-phase branch with an analytic enthalpy
    /// model. Model: `h(T, P, Vapor) = Cp·(T − Tref)`, `h(T, P, Liquid) =
    /// Cp·(T − Tref) − Lv`, so the saturated liquid sits a latent heat `Lv`
    /// below the saturated vapour. With `Cp = 30 J/(mol·K)`, `Tref = 100 K`,
    /// `Lv = 8000 J/mol`, choose `H_target = HsatL + 0.5·Lv` — the flash must
    /// return `V = 0.5` at `T = Tsat(P)`. Pure methane, `P = 1.0e6 Pa`.
    /// **Results (2026-08-03, release):** `Tsat(1.0e6 Pa) = 148.771 K`;
    /// `HsatV = 1463.13 J/mol`, `HsatL = −6536.87 J/mol` (a latent heat below);
    /// at `H = −2536.87 J/mol` the flash returns `V = 0.5000000` (< 1e-9),
    /// `T = Tsat`, phase = TwoPhase.
    #[test]
    fn ph_flash_two_phase_recovers_specified_quality() {
        let comps = [reference::methane()];
        let z = [1.0];
        let p = 1.0e6;
        let (cp, t_ref, lv) = (30.0, 100.0, 8000.0);
        let enth = move |t: f64, _p: f64, ph: Phase| match ph {
            Phase::Vapor => cp * (t - t_ref),
            Phase::Liquid => cp * (t - t_ref) - lv,
        };

        let tsat = saturation_temperature(&comps, &z, p, PropertyPackageModel::Ideal).unwrap();
        let h_sat_l = enth(tsat, p, Phase::Liquid);
        let h_target = h_sat_l + 0.5 * lv;

        let r = flash_ph(
            &comps,
            &z,
            p,
            h_target,
            PropertyPackageModel::Ideal,
            enth,
            SingleCompOptions::default(),
        )
        .unwrap();
        assert_eq!(r.phase, SingleCompPhase::TwoPhase);
        assert_abs_diff_eq!(r.vapour_fraction, 0.5, epsilon = 1e-9);
        assert_abs_diff_eq!(r.temperature, tsat, epsilon = 1e-12);
    }

    /// **Methodology.** [`flash_ph`] single-phase branches with the same analytic
    /// model. A target above `HsatV` must give superheated vapour with
    /// `T = Tref + H/Cp` (from `Cp·(T−Tref) = H`); a target below `HsatL` must
    /// give subcooled liquid with `T = Tref + (H+Lv)/Cp` (from
    /// `Cp·(T−Tref) − Lv = H`). `Cp = 30`, `Tref = 100 K`, `Lv = 8000 J/mol`,
    /// pure methane, `P = 1.0e6 Pa` (`Tsat = 148.771 K`, `HsatV = 1463.13`,
    /// `HsatL = −6536.87 J/mol`).
    /// **Results (2026-08-03, release):** `H = 12000 J/mol` (> HsatV) →
    /// `V = 1`, `T = 500.0000 K` (= 100 + 12000/30) to < 1e-6 K;
    /// `H = −10000 J/mol` (< HsatL) → `V = 0`, `T = 33.3333 K`
    /// (= 100 + (−10000 + 8000)/30) to < 1e-6 K.
    #[test]
    fn ph_flash_single_phase_branches_recover_analytic_temperature() {
        let comps = [reference::methane()];
        let z = [1.0];
        let p = 1.0e6;
        let (cp, t_ref, lv) = (30.0, 100.0, 8000.0);
        let enth = move |t: f64, _p: f64, ph: Phase| match ph {
            Phase::Vapor => cp * (t - t_ref),
            Phase::Liquid => cp * (t - t_ref) - lv,
        };

        // Superheated vapour.
        let hv = 12000.0;
        let rv = flash_ph(
            &comps,
            &z,
            p,
            hv,
            PropertyPackageModel::Ideal,
            enth,
            SingleCompOptions::default(),
        )
        .unwrap();
        assert_eq!(rv.phase, SingleCompPhase::Vapour);
        assert_abs_diff_eq!(rv.vapour_fraction, 1.0, epsilon = 1e-15);
        assert_abs_diff_eq!(rv.temperature, t_ref + hv / cp, epsilon = 1e-6);

        // Subcooled liquid (target below the saturated-liquid enthalpy).
        let hl = -10000.0;
        let rl = flash_ph(
            &comps,
            &z,
            p,
            hl,
            PropertyPackageModel::Ideal,
            enth,
            SingleCompOptions::default(),
        )
        .unwrap();
        assert_eq!(rl.phase, SingleCompPhase::Liquid);
        assert_abs_diff_eq!(rl.vapour_fraction, 0.0, epsilon = 1e-15);
        assert_abs_diff_eq!(rl.temperature, t_ref + (hl + lv) / cp, epsilon = 1e-6);
    }

    /// **Methodology.** The dominant-component selector (`GetIndex`) must let a
    /// near-pure multicomponent feed be flashed as its major component. Feed
    /// `[methane, ethane]` with `z = [0.95, 0.05]` (methane > 0.9) must give the
    /// same `Psat` as pure methane at the same `T`. `T = 150 K`.
    /// **Results (2026-08-03, release):** dominant index 0 (methane);
    /// `Psat = 1.058654e6 Pa`, identical to the pure-methane result to < 1e-9 Pa.
    #[test]
    fn dominant_component_is_selected_for_near_pure_feed() {
        let comps = [reference::methane(), reference::ethane()];
        let z = [0.95, 0.05];
        let t = 150.0;
        let psat_mix = saturation_pressure(&comps, &z, t, PropertyPackageModel::Ideal).unwrap();

        let pure = [reference::methane()];
        let psat_pure = saturation_pressure(&pure, &[1.0], t, PropertyPackageModel::Ideal).unwrap();
        assert_abs_diff_eq!(psat_mix, psat_pure, epsilon = 1e-9);
        assert_eq!(dominant_index(&z), 0);
    }

    /// **Methodology.** Input-validation guards. **Results (2026-08-03):** empty
    /// feed → `Empty`; mismatched `components`/`z` → `LengthMismatch`;
    /// non-positive pressure → `NonPositive`; a vapour fraction outside `[0, 1]`
    /// → `VapourFractionOutOfRange`.
    #[test]
    fn input_validation_errors() {
        let comps = [reference::methane()];
        assert_eq!(
            flash_pt(&[], &[], 1e5, 150.0, PropertyPackageModel::Ideal).unwrap_err(),
            SingleCompError::Empty
        );
        assert!(matches!(
            flash_pt(&comps, &[0.5, 0.5], 1e5, 150.0, PropertyPackageModel::Ideal).unwrap_err(),
            SingleCompError::LengthMismatch { .. }
        ));
        assert!(matches!(
            flash_pt(&comps, &[1.0], -1.0, 150.0, PropertyPackageModel::Ideal).unwrap_err(),
            SingleCompError::NonPositive { .. }
        ));
        assert!(matches!(
            flash_tv(&comps, &[1.0], 150.0, 1.5, PropertyPackageModel::Ideal).unwrap_err(),
            SingleCompError::VapourFractionOutOfRange { .. }
        ));
    }
}
