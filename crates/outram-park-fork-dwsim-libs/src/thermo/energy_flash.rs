//! Isenthalpic (PH) / energy flash: solve the temperature `T` at which a
//! mixture's total molar enthalpy equals a target `H`, at fixed pressure `P`.
//!
//! Ported/composed from **DWSIM** (GPL-3.0). The reference is the PH-flash idea
//! in DWSIM's flash algorithms:
//! - `DWSIM.Thermodynamics/FlashAlgorithms/BaseFlashAlgorithm.vb` `Flash_PH`
//!   (the generic energy-flash driver template), and
//! - `DWSIM.Thermodynamics/FlashAlgorithms/NestedLoops.vb` `Flash_PH`
//!   (the concrete nested-loops implementation): an outer temperature loop that
//!   drives `H(T) − H_target → 0` with an inner TP (pressure-temperature) flash
//!   supplying the phase split at each candidate `T`, using the mixture heat
//!   capacity as the Newton derivative.
//!
//! ## The physics this module implements
//!
//! At a candidate temperature `T` and the fixed pressure `P`, the mixture's
//! total molar enthalpy is the ideal-gas part plus the phase-weighted real-fluid
//! departures:
//!
//! ```text
//! H(T,P) = H_ideal_mix(T)                         (Σ_i z_i ∫_{T_ref}^{T} Cp0_i dT)
//!        + β · H_dep(y, T, P, vapour)             (β   = vapour molar fraction)
//!        + (1 − β) · H_dep(x, T, P, liquid)
//! ```
//!
//! - The **ideal-gas part** is [`crate::thermo::ideal_props::mixture_ideal_gas_enthalpy`]
//!   evaluated at the *feed* composition `z`. The ideal-gas enthalpy of mixing
//!   is zero and ideal-gas enthalpy is pressure-independent, so the phase split
//!   does not change this term — moles are conserved across the split.
//! - The **departures** are [`crate::thermo::cubic_eos::CubicEos::enthalpy_departure`]
//!   evaluated at each phase's own composition (`y` for vapour, `x` for liquid)
//!   and weighted by that phase's molar fraction.
//!
//! ## Decoupling (the crate's push-to-caller pattern, no `dyn`)
//!
//! The two model-dependent steps are **caller-supplied generic `Fn` closures**,
//! never trait objects — mirroring [`crate::thermo::flash::nested_loops_flash`]'s
//! K-value closure and the `expander::isentropic` pattern, and obeying the
//! workspace no-`Box<dyn>`/no-`dyn` rule:
//!
//! - a **TP-flash closure** `Fn(T, P) -> FlashResult` gives the phase split
//!   `(β, x, y)` at `(T, P)` — this is where the property model (fugacity /
//!   activity) enters, so keeping it a closure keeps this module independent of
//!   any particular EOS/activity choice;
//! - an **enthalpy-departure closure** `Fn(&[f64], T, P, Phase) -> f64` returns
//!   the molar enthalpy departure `[J/mol]` of a phase of the given composition.
//!   The intended argument wraps [`crate::thermo::cubic_eos::CubicEos::enthalpy_departure`]
//!   (`.unwrap_or(0.0)` for the ideal-gas / no-root limit), but any real-fluid
//!   departure model satisfies the signature.
//!
//! ## Solver: safeguarded Newton / bisection
//!
//! `H(T)` is monotonically increasing in `T` (the mixture `Cp > 0`), so
//! `f(T) = H(T) − H_target` has exactly one root. The driver:
//!
//! 1. **Brackets** the root outward from an initial guess using a
//!    `Cp`-scaled, geometrically-growing step, until a sign change of `f` is
//!    found (documented bounds `[t_min, t_max]`; failure → [`EnergyFlashError::NoBracket`]).
//! 2. **Solves** inside the bracket with a Numerical-Recipes-style `rtsafe`
//!    **safeguarded Newton**: a Newton step `ΔT = f / (dH/dT)` with
//!    `dH/dT ≈ Cp_mix(T)` (the *ideal-gas* mixture Cp — the departure's
//!    temperature derivative is intentionally neglected, an approximation the
//!    bisection safeguard makes robust), falling back to bisection whenever the
//!    Newton step would leave the bracket or is not reducing the interval fast
//!    enough. This cannot diverge because the root stays bracketed throughout.
//!
//! ## Honest scope (verification, NOT benchmark validation)
//!
//! - The inline tests are **verification** — they check the driver against
//!   hand-computed analytic cases (constant-`Cp` ideal gas), an internal
//!   round-trip, phase-weighting algebra, and monotonicity. They do **not**
//!   validate the enthalpy model against measured PH-flash data or the DWSIM
//!   reference outputs; that is benchmark validation and is future work.
//! - **Enthalpy model = ideal-gas Cp0 integral + cubic-EOS departure only.**
//!   No enthalpy-of-formation offsets are applied (consistent with
//!   [`crate::thermo::ideal_props`]'s *sensible*-enthalpy convention), so
//!   `H_target` must be expressed on that same sensible-enthalpy scale relative
//!   to `T_ref`. Mixing across the two product phases beyond the mole-weighted
//!   departures is not modelled.
//! - **Two-phase PH flash is supported** through the TP-flash closure's `β`,
//!   but the *quality* of the two-phase result is only as good as the supplied
//!   TP flash; this module owns the temperature root-find, not the VLE.
//! - **`flash_ps` (entropy / PS flash)** is implemented on the same skeleton
//!   with `dS/dT = Cp/T`, but its two-phase entropy is a documented
//!   approximation (see [`flash_ps`]); it is verified only for the single-phase
//!   case here.

use crate::thermo::cubic_eos::Phase;
use crate::thermo::flash::FlashResult;
use crate::thermo::ideal_props::{
    mixture_ideal_gas_cp, mixture_ideal_gas_enthalpy, mixture_ideal_gas_entropy,
};
use crate::thermo::Component;

/// Tuning parameters for the safeguarded Newton/bisection temperature solve
/// used by [`flash_ph`] and [`flash_ps`].
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct EnergyFlashOptions {
    /// Maximum safeguarded-Newton iterations inside the bracket before
    /// returning [`EnergyFlashError::NotConverged`].
    pub max_iter: usize,
    /// Absolute convergence tolerance on the objective value
    /// `|H(T) − H_target|` \[J/mol\] (for [`flash_ph`]) or
    /// `|S(T) − S_target|` \[J/(mol·K)\] (for [`flash_ps`]).
    pub value_tol: f64,
    /// Absolute convergence tolerance on the temperature step `|ΔT|` \[K\].
    pub temperature_tol: f64,
    /// Lower bound of the temperature search \[K\] (bracket floor).
    pub t_min: f64,
    /// Upper bound of the temperature search \[K\] (bracket ceiling).
    pub t_max: f64,
    /// Maximum geometric bracket-expansion steps before giving up with
    /// [`EnergyFlashError::NoBracket`].
    pub bracket_expansions: usize,
}

impl Default for EnergyFlashOptions {
    /// Defaults sized for the verification tests: `1e-6` value tolerance,
    /// `1e-9 K` temperature tolerance, a `1..20000 K` search window, and up to
    /// 200 iterations / bracket expansions.
    fn default() -> Self {
        Self {
            max_iter: 200,
            value_tol: 1.0e-6,
            temperature_tol: 1.0e-9,
            t_min: 1.0,
            t_max: 20_000.0,
            bracket_expansions: 200,
        }
    }
}

/// A converged energy-flash result (from [`flash_ph`] or [`flash_ps`]).
#[derive(Debug, Clone, PartialEq)]
pub struct EnergyFlashResult {
    /// The converged temperature `T` \[K\] at which the mixture enthalpy
    /// (or entropy) equals the target, at the fixed input pressure.
    pub temperature: f64,
    /// Vapour molar fraction `β` \[-\] at the converged state, in `[0, 1]`:
    /// `0` = all liquid, `1` = all vapour, interior = two coexisting phases.
    /// Taken from the TP-flash closure evaluated at the converged `T`.
    pub vapour_fraction: f64,
    /// Liquid-phase mole fractions `x_i` \[-\] at the converged `T` (sum to 1).
    pub x: Vec<f64>,
    /// Vapour-phase mole fractions `y_i` \[-\] at the converged `T` (sum to 1).
    pub y: Vec<f64>,
    /// Number of safeguarded-Newton iterations performed inside the bracket.
    pub iterations: usize,
    /// Achieved absolute residual of the objective at convergence:
    /// `|H(T) − H_target|` \[J/mol\] for [`flash_ph`], or
    /// `|S(T) − S_target|` \[J/(mol·K)\] for [`flash_ps`].
    pub residual: f64,
}

/// Error conditions for the energy-flash routines.
#[derive(Debug, Clone, PartialEq, thiserror::Error)]
pub enum EnergyFlashError {
    /// An empty composition was supplied (need at least one component).
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
    /// A non-finite objective value (`NaN`/`inf`) was produced by the enthalpy
    /// (or entropy) evaluation at a candidate temperature.
    #[error("non-finite objective value at temperature {temperature} K")]
    NonFinite {
        /// The temperature at which the non-finite value appeared \[K\].
        temperature: f64,
    },
    /// The root could not be bracketed within `[t_min, t_max]` in the allowed
    /// number of expansions — the target lies outside the reachable enthalpy
    /// range, or the objective is non-monotone within the window.
    #[error("could not bracket the root in [{t_min}, {t_max}] K after expansion")]
    NoBracket {
        /// Search-window floor \[K\].
        t_min: f64,
        /// Search-window ceiling \[K\].
        t_max: f64,
    },
    /// The derivative `dH/dT` (or `dS/dT`) was ~0 at the initial guess, so no
    /// Newton predictor could be formed (a zero-`Cp` mixture).
    #[error("degenerate derivative (Cp ~ 0) at temperature {temperature} K")]
    DegenerateDerivative {
        /// The temperature at which the derivative vanished \[K\].
        temperature: f64,
    },
    /// The safeguarded Newton loop did not reach tolerance in `max_iter`.
    #[error("energy flash did not converge in {iterations} iterations (residual {residual:e})")]
    NotConverged {
        /// Iterations performed.
        iterations: usize,
        /// Final objective residual.
        residual: f64,
    },
}

/// Total molar enthalpy `H(T, P)` \[J/mol\] of the mixture at temperature `T`
/// \[K\] and pressure `P` \[Pa\]: the ideal-gas part plus the phase-weighted
/// real-fluid enthalpy departures.
///
/// ```text
/// H = Σ_i z_i ∫_{T_ref}^{T} Cp0_i dT
///   + β · H_dep(y, T, P, Vapor) + (1 − β) · H_dep(x, T, P, Liquid)
/// ```
///
/// The ideal-gas term uses the **feed** composition `z` (moles are conserved
/// across the phase split and ideal-gas enthalpy is composition-linear and
/// pressure-independent, so the split does not affect it). Each departure is
/// evaluated at its own phase composition and weighted by the phase's molar
/// fraction; a phase with zero fraction contributes nothing (its departure
/// closure is not called).
///
/// # Parameters
/// - `components`, `z`: pure compounds and their feed mole fractions \[-\]
///   (same length; `z` normally sums to 1).
/// - `temperature` `T` \[K\] > 0, `pressure` `P` \[Pa\] > 0.
/// - `t_ref` `T_ref` \[K\]: the ideal-gas enthalpy reference temperature (the
///   `H = 0` datum for the sensible-enthalpy scale).
/// - `tp_flash`: `Fn(T, P) -> FlashResult` — the phase split at `(T, P)`.
/// - `enthalpy_departure`: `Fn(&[f64], T, P, Phase) -> f64` — molar enthalpy
///   departure \[J/mol\] of a phase of the given composition.
///
/// # Panics
/// Panics (via [`mixture_ideal_gas_enthalpy`]) if
/// `components.len() != z.len()`.
///
/// # Returns
/// Total molar enthalpy `H(T, P)` \[J/mol\] on the sensible-enthalpy scale
/// (no enthalpy-of-formation offset).
pub fn mixture_enthalpy<Flash, Dep>(
    components: &[Component],
    z: &[f64],
    temperature: f64,
    pressure: f64,
    t_ref: f64,
    tp_flash: Flash,
    enthalpy_departure: Dep,
) -> f64
where
    Flash: Fn(f64, f64) -> FlashResult,
    Dep: Fn(&[f64], f64, f64, Phase) -> f64,
{
    let h_ideal = mixture_ideal_gas_enthalpy(components, z, temperature, t_ref);
    let split = tp_flash(temperature, pressure);
    let beta = split.beta;
    let mut h_dep = 0.0;
    if beta > 0.0 {
        h_dep += beta * enthalpy_departure(&split.y, temperature, pressure, Phase::Vapor);
    }
    if beta < 1.0 {
        h_dep += (1.0 - beta) * enthalpy_departure(&split.x, temperature, pressure, Phase::Liquid);
    }
    h_ideal + h_dep
}

/// Total molar entropy `S(T, P)` \[J/(mol·K)\] of the mixture: the ideal-gas
/// entropy plus the phase-weighted real-fluid entropy departures.
///
/// ```text
/// S = S_ideal_mix(z, T, P)                        (incl. −R ln(P/P_ref) and mixing)
///   + β · S_dep(y, T, P, Vapor) + (1 − β) · S_dep(x, T, P, Liquid)
/// ```
///
/// The ideal-gas term is [`crate::thermo::ideal_props::mixture_ideal_gas_entropy`]
/// at the **feed** composition and the total pressure.
///
/// **Two-phase caveat (documented approximation).** Unlike enthalpy, ideal-gas
/// entropy is *not* independent of composition or pressure, so evaluating the
/// ideal part at the feed composition and total pressure is exact only in the
/// single-phase limit (`β = 0` or `β = 1`). In the two-phase region it omits
/// the entropy of separating the feed into distinct-composition phases at their
/// partial pressures. Full two-phase entropy accounting is deferred; treat
/// [`flash_ps`] as verified for single-phase only.
///
/// # Parameters
/// As [`mixture_enthalpy`], plus `p_ref` `P_ref` \[Pa\] for the ideal-gas
/// pressure term, and an `entropy_departure` closure returning molar entropy
/// departure \[J/(mol·K)\].
///
/// # Panics
/// Panics if `components.len() != z.len()`.
///
/// # Returns
/// Total molar entropy `S(T, P)` \[J/(mol·K)\].
#[allow(clippy::too_many_arguments)]
pub fn mixture_entropy<Flash, Dep>(
    components: &[Component],
    z: &[f64],
    temperature: f64,
    pressure: f64,
    t_ref: f64,
    p_ref: f64,
    tp_flash: Flash,
    entropy_departure: Dep,
) -> f64
where
    Flash: Fn(f64, f64) -> FlashResult,
    Dep: Fn(&[f64], f64, f64, Phase) -> f64,
{
    let s_ideal = mixture_ideal_gas_entropy(components, z, temperature, pressure, t_ref, p_ref);
    let split = tp_flash(temperature, pressure);
    let beta = split.beta;
    let mut s_dep = 0.0;
    if beta > 0.0 {
        s_dep += beta * entropy_departure(&split.y, temperature, pressure, Phase::Vapor);
    }
    if beta < 1.0 {
        s_dep += (1.0 - beta) * entropy_departure(&split.x, temperature, pressure, Phase::Liquid);
    }
    s_ideal + s_dep
}

/// Isenthalpic (PH) flash: find the temperature `T` \[K\] at which the mixture's
/// total molar enthalpy equals `h_target` \[J/mol\], at the fixed pressure `P`.
///
/// Drives `f(T) = H(T, P) − H_target → 0` (with [`mixture_enthalpy`]) by the
/// safeguarded Newton/bisection scheme documented at the module level:
/// bracket outward with a `Cp`-scaled growing step, then `rtsafe` inside the
/// bracket using `dH/dT ≈ Cp_mix(T)` (ideal-gas mixture heat capacity) as the
/// Newton derivative, with bisection safeguarding every step so it cannot
/// diverge.
///
/// # Parameters
/// - `components`, `z`: pure compounds and feed mole fractions \[-\]
///   (same non-zero length).
/// - `pressure` `P` \[Pa\] > 0 (held fixed).
/// - `h_target` `H_target` \[J/mol\]: the target total molar enthalpy on the
///   sensible-enthalpy scale relative to `t_ref` (no formation offset).
/// - `t_ref` `T_ref` \[K\]: ideal-gas enthalpy reference (the `H = 0` datum).
/// - `t_initial` \[K\]: initial temperature guess (must satisfy
///   `t_min ≤ t_initial ≤ t_max`); the bracket is grown outward from here.
/// - `tp_flash`: `Fn(T, P) -> FlashResult` — phase split at each candidate `T`.
/// - `enthalpy_departure`: `Fn(&[f64], T, P, Phase) -> f64` — molar enthalpy
///   departure \[J/mol\] (wrap [`crate::thermo::cubic_eos::CubicEos::enthalpy_departure`]
///   with `.unwrap_or(0.0)`).
/// - `opts`: solver tolerances and bounds ([`EnergyFlashOptions`]).
///
/// # Errors
/// - [`EnergyFlashError::Empty`] / [`EnergyFlashError::LengthMismatch`] for a
///   malformed composition.
/// - [`EnergyFlashError::NoBracket`] if the target enthalpy is unreachable in
///   `[t_min, t_max]`.
/// - [`EnergyFlashError::DegenerateDerivative`] if `Cp ~ 0` at `t_initial`.
/// - [`EnergyFlashError::NonFinite`] / [`EnergyFlashError::NotConverged`] on a
///   non-finite evaluation or failure to reach tolerance in `max_iter`.
///
/// # Returns
/// The converged [`EnergyFlashResult`] (temperature, vapour fraction, phase
/// compositions, iteration count, and achieved enthalpy residual).
#[allow(clippy::too_many_arguments)]
pub fn flash_ph<Flash, Dep>(
    components: &[Component],
    z: &[f64],
    pressure: f64,
    h_target: f64,
    t_ref: f64,
    t_initial: f64,
    tp_flash: Flash,
    enthalpy_departure: Dep,
    opts: EnergyFlashOptions,
) -> Result<EnergyFlashResult, EnergyFlashError>
where
    Flash: Fn(f64, f64) -> FlashResult,
    Dep: Fn(&[f64], f64, f64, Phase) -> f64,
{
    validate(components, z)?;

    // Objective f(T) = H(T,P) − H_target and its (ideal-gas) derivative Cp(T).
    let f = |t: f64| {
        mixture_enthalpy(
            components,
            z,
            t,
            pressure,
            t_ref,
            &tp_flash,
            &enthalpy_departure,
        ) - h_target
    };
    let dfdt = |t: f64| mixture_ideal_gas_cp(components, z, t);

    let (t, iterations, residual) = safeguarded_solve(&f, &dfdt, t_initial, opts)?;
    let split = tp_flash(t, pressure);
    Ok(EnergyFlashResult {
        temperature: t,
        vapour_fraction: split.beta,
        x: split.x,
        y: split.y,
        iterations,
        residual,
    })
}

/// Isentropic-target (PS) flash: find the temperature `T` \[K\] at which the
/// mixture's total molar entropy equals `s_target` \[J/(mol·K)\], at fixed `P`.
///
/// Same safeguarded Newton/bisection skeleton as [`flash_ph`], driving
/// `S(T, P) − S_target → 0` (with [`mixture_entropy`]) using the ideal-gas
/// derivative `dS/dT = Cp_mix(T) / T`.
///
/// **Scope / honesty.** Because [`mixture_entropy`]'s two-phase term is a
/// documented approximation (it evaluates the ideal entropy at the feed
/// composition and total pressure, omitting the entropy of inter-phase
/// separation), this routine is **verified only for the single-phase case**.
/// Two-phase PS flash with rigorous entropy accounting is deferred future work.
///
/// # Parameters
/// As [`flash_ph`], but `s_target` \[J/(mol·K)\] and an additional
/// `p_ref` `P_ref` \[Pa\] for the ideal-gas pressure term, and an
/// `entropy_departure` closure returning molar entropy departure
/// \[J/(mol·K)\] (wrap [`crate::thermo::cubic_eos::CubicEos::entropy_departure`]).
///
/// # Errors
/// As [`flash_ph`] (the entropy analogue of each condition).
///
/// # Returns
/// The converged [`EnergyFlashResult`]; its `residual` is `|S(T) − S_target|`
/// \[J/(mol·K)\].
#[allow(clippy::too_many_arguments)]
pub fn flash_ps<Flash, Dep>(
    components: &[Component],
    z: &[f64],
    pressure: f64,
    s_target: f64,
    t_ref: f64,
    p_ref: f64,
    t_initial: f64,
    tp_flash: Flash,
    entropy_departure: Dep,
    opts: EnergyFlashOptions,
) -> Result<EnergyFlashResult, EnergyFlashError>
where
    Flash: Fn(f64, f64) -> FlashResult,
    Dep: Fn(&[f64], f64, f64, Phase) -> f64,
{
    validate(components, z)?;

    let f = |t: f64| {
        mixture_entropy(
            components,
            z,
            t,
            pressure,
            t_ref,
            p_ref,
            &tp_flash,
            &entropy_departure,
        ) - s_target
    };
    // dS/dT = Cp/T for the ideal-gas contribution.
    let dfdt = |t: f64| mixture_ideal_gas_cp(components, z, t) / t;

    let (t, iterations, residual) = safeguarded_solve(&f, &dfdt, t_initial, opts)?;
    let split = tp_flash(t, pressure);
    Ok(EnergyFlashResult {
        temperature: t,
        vapour_fraction: split.beta,
        x: split.x,
        y: split.y,
        iterations,
        residual,
    })
}

/// Validate the `(components, z)` pairing shared by both drivers.
fn validate(components: &[Component], z: &[f64]) -> Result<(), EnergyFlashError> {
    if components.is_empty() || z.is_empty() {
        return Err(EnergyFlashError::Empty);
    }
    if components.len() != z.len() {
        return Err(EnergyFlashError::LengthMismatch {
            a: components.len(),
            b: z.len(),
        });
    }
    Ok(())
}

/// Safeguarded Newton/bisection root-find of a monotone objective `f(T)` with
/// derivative `dfdt(T)`, returning `(root_T, iterations, |f(root)|)`.
///
/// This is the shared engine behind [`flash_ph`] and [`flash_ps`]. It first
/// **brackets** the root outward from `t_initial` (a derivative-scaled,
/// geometrically-growing step, clamped to `[t_min, t_max]`), then runs a
/// Numerical-Recipes `rtsafe`: a Newton step is taken only when it stays inside
/// the current bracket and is shrinking the interval fast enough, otherwise the
/// step bisects. The root is bracketed at all times, so the iteration cannot
/// diverge even though `dfdt` neglects the departure's temperature dependence.
fn safeguarded_solve<F, D>(
    f: &F,
    dfdt: &D,
    t_initial: f64,
    opts: EnergyFlashOptions,
) -> Result<(f64, usize, f64), EnergyFlashError>
where
    F: Fn(f64) -> f64,
    D: Fn(f64) -> f64,
{
    let f0 = f(t_initial);
    if !f0.is_finite() {
        return Err(EnergyFlashError::NonFinite {
            temperature: t_initial,
        });
    }
    if f0.abs() <= opts.value_tol {
        return Ok((t_initial, 0, f0.abs()));
    }

    let d0 = dfdt(t_initial);
    if !d0.is_finite() || d0.abs() < 1.0e-30 {
        return Err(EnergyFlashError::DegenerateDerivative {
            temperature: t_initial,
        });
    }

    // ---- Bracket the root: f is increasing in T. --------------------------
    // Initial step magnitude from the Newton predictor distance, floored so a
    // near-zero-slope guess still moves.
    let mut step = (f0.abs() / d0.abs()).max(10.0);
    let (mut lo, mut hi) = (t_initial, t_initial);

    if f0 < 0.0 {
        // Need a higher T where f ≥ 0.
        let mut t = t_initial;
        let mut bracketed = false;
        for _ in 0..opts.bracket_expansions {
            t = (t + step).min(opts.t_max);
            let ft = f(t);
            if !ft.is_finite() {
                return Err(EnergyFlashError::NonFinite { temperature: t });
            }
            if ft.abs() <= opts.value_tol {
                return Ok((t, 0, ft.abs()));
            }
            if ft >= 0.0 {
                hi = t;
                bracketed = true;
                break;
            }
            lo = t;
            if t >= opts.t_max {
                break;
            }
            step *= 2.0;
        }
        if !bracketed {
            return Err(EnergyFlashError::NoBracket {
                t_min: opts.t_min,
                t_max: opts.t_max,
            });
        }
    } else {
        // Need a lower T where f ≤ 0.
        let mut t = t_initial;
        let mut bracketed = false;
        for _ in 0..opts.bracket_expansions {
            t = (t - step).max(opts.t_min);
            let ft = f(t);
            if !ft.is_finite() {
                return Err(EnergyFlashError::NonFinite { temperature: t });
            }
            if ft.abs() <= opts.value_tol {
                return Ok((t, 0, ft.abs()));
            }
            if ft <= 0.0 {
                lo = t;
                bracketed = true;
                break;
            }
            hi = t;
            if t <= opts.t_min {
                break;
            }
            step *= 2.0;
        }
        if !bracketed {
            return Err(EnergyFlashError::NoBracket {
                t_min: opts.t_min,
                t_max: opts.t_max,
            });
        }
    }

    // ---- rtsafe inside [lo, hi] (f(lo) < 0 < f(hi)). ----------------------
    // xl is the f<0 end, xh the f>0 end.
    let (mut xl, mut xh) = (lo, hi);
    let mut rts = 0.5 * (lo + hi);
    let mut dx_old = (hi - lo).abs();
    let mut dx = dx_old;
    let mut fv = f(rts);
    let mut df = dfdt(rts);

    for iter in 1..=opts.max_iter {
        let out_of_range = ((rts - xh) * df - fv) * ((rts - xl) * df - fv) > 0.0;
        let slow = (2.0 * fv).abs() > (dx_old * df).abs();
        if !df.is_finite() || df.abs() < 1.0e-30 || out_of_range || slow {
            // Bisection step.
            dx_old = dx;
            dx = 0.5 * (xh - xl);
            rts = xl + dx;
        } else {
            // Newton step.
            dx_old = dx;
            dx = fv / df;
            rts -= dx;
        }

        if dx.abs() < opts.temperature_tol {
            let r = f(rts).abs();
            return Ok((rts, iter, r));
        }

        fv = f(rts);
        if !fv.is_finite() {
            return Err(EnergyFlashError::NonFinite { temperature: rts });
        }
        df = dfdt(rts);

        if fv.abs() <= opts.value_tol {
            return Ok((rts, iter, fv.abs()));
        }

        if fv < 0.0 {
            xl = rts;
        } else {
            xh = rts;
        }
    }

    Err(EnergyFlashError::NotConverged {
        iterations: opts.max_iter,
        residual: fv.abs(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::thermo::cubic_eos::CubicEos;
    use approx::assert_relative_eq;

    /// Build a test component carrying only the given Cp0 coefficients plus
    /// physically-plausible critical constants (needed for the cubic-departure
    /// round-trip; the ideal-gas tests read only `cp_ig_a..e`). Nitrogen-like
    /// constants (Poling et al. 2001, Appendix A) are used as neutral defaults.
    fn comp(cp_ig: [f64; 5]) -> Component {
        Component::new(
            "test",
            0.028014,
            126.20,
            3.398e6,
            90.1e-6,
            0.037,
            77.35,
            cp_ig,
            f64::NAN,
        )
        .expect("valid test component")
    }

    /// An all-vapour single-phase TP-flash closure: `β = 1`, `x = y = z`.
    fn all_vapour(z: &[f64]) -> impl Fn(f64, f64) -> FlashResult + '_ {
        move |_t, _p| FlashResult {
            beta: 1.0,
            x: z.to_vec(),
            y: z.to_vec(),
            k: vec![1.0; z.len()],
            iterations: 0,
        }
    }

    /// **Methodology.** Single-phase ideal-gas PH flash with a constant Cp
    /// (only the A coefficient nonzero) and zero departures. For a pure stream
    /// `z = [1]` the enthalpy is exactly `H(T) = A·(T − T_ref)`, so the flash
    /// must recover the analytic `T = T_ref + H_target/A`. With A = 29.1
    /// J/(mol·K), T_ref = 298.15 K, H_target = 5000 J/mol the exact answer is
    /// `298.15 + 5000/29.1 = 469.971306…` K.
    /// **Results (2026-07-24):** flash_ph returns T = 469.971306 K, matching
    /// the closed form to < 1e-9 relative; the achieved enthalpy residual is
    /// below the 1e-6 J/mol tolerance.
    #[test]
    fn single_phase_ideal_gas_recovers_analytic_temperature() {
        let a = 29.1;
        let c = [comp([a, 0.0, 0.0, 0.0, 0.0])];
        let z = [1.0];
        let (t_ref, p) = (298.15, 1.0e5);
        let h_target = 5000.0;

        let flash = all_vapour(&z);
        let dep = |_frac: &[f64], _t: f64, _p: f64, _ph: Phase| 0.0;

        let r = flash_ph(
            &c,
            &z,
            p,
            h_target,
            t_ref,
            300.0,
            flash,
            dep,
            EnergyFlashOptions::default(),
        )
        .expect("flash_ph converges");

        let expected = t_ref + h_target / a;
        assert_relative_eq!(r.temperature, expected, max_relative = 1e-9);
        assert_relative_eq!(r.temperature, 469.971_306, max_relative = 1e-6);
        assert!(r.residual <= 1e-6);
        assert_eq!(r.vapour_fraction, 1.0);
    }

    /// **Methodology.** Binary constant-Cp mixture, ideal gas, zero departures.
    /// The mixture Cp is the mole-weighted average `Cp = 0.4·29.1 + 0.6·37.0 =
    /// 33.84 J/(mol·K)`, and the enthalpy is `Cp·(T − T_ref)`. flash_ph must
    /// recover `T = T_ref + H_target/Cp`. With H_target = 8000 J/mol,
    /// T_ref = 300 K: `300 + 8000/33.84 = 536.406619…` K.
    /// **Results (2026-07-24):** flash_ph returns T = 536.406619 K to < 1e-9
    /// relative.
    #[test]
    fn binary_ideal_gas_recovers_weighted_cp_temperature() {
        let comps = [
            comp([29.1, 0.0, 0.0, 0.0, 0.0]),
            comp([37.0, 0.0, 0.0, 0.0, 0.0]),
        ];
        let z = [0.4, 0.6];
        let (t_ref, p) = (300.0, 1.0e5);
        let h_target = 8000.0;
        let cp_mix = 0.4 * 29.1 + 0.6 * 37.0;

        let flash = all_vapour(&z);
        let dep = |_f: &[f64], _t: f64, _p: f64, _ph: Phase| 0.0;

        let r = flash_ph(
            &comps,
            &z,
            p,
            h_target,
            t_ref,
            400.0,
            flash,
            dep,
            EnergyFlashOptions::default(),
        )
        .expect("converges");

        let expected = t_ref + h_target / cp_mix;
        assert_relative_eq!(r.temperature, expected, max_relative = 1e-9);
        assert_relative_eq!(r.temperature, 536.406_619, max_relative = 1e-6);
    }

    /// **Methodology.** Round-trip with real Peng-Robinson enthalpy departures.
    /// Build a pure nitrogen-like stream with a quartic Cp0 (A = 19.8,
    /// B = 7.344e-2, C = −5.602e-5, D = 1.715e-8, E = 1e-12), compute the total
    /// molar enthalpy `H` at a known T_known = 350 K, P = 1e6 Pa (all-vapour
    /// single phase, PR departure), then flash_ph back from a deliberately
    /// wrong initial guess of 250 K. Because the driver solves `H(T) = H_target`
    /// with the *same* enthalpy function, the safeguarded Newton must return
    /// exactly T_known.
    /// **Results (2026-07-24):** the forward enthalpy at 350 K was
    /// H = 1933.1977 J/mol (ideal-gas part 1985.9930 J/mol + PR departure
    /// −52.7953 J/mol at 1 MPa); flash_ph recovered T = 350.000000 K to < 1e-7
    /// relative, residual < 1e-6 J/mol.
    #[test]
    fn round_trip_with_peng_robinson_departure() {
        let comps = [comp([19.8, 7.344e-2, -5.602e-5, 1.715e-8, 1.0e-12])];
        let z = [1.0];
        let (t_ref, p) = (298.15, 1.0e6);
        let t_known = 350.0;

        let eos = CubicEos::PengRobinson;
        let flash = all_vapour(&z);
        // Capture `comps` by reference; wrap PR departure, ideal limit on None.
        let dep = |frac: &[f64], t: f64, p: f64, ph: Phase| {
            eos.enthalpy_departure(&comps, frac, t, p, ph, None)
                .unwrap_or(0.0)
        };
        // Shadow as `Copy` references so both calls below can reuse them.
        let flash = &flash;
        let dep = &dep;

        let h_known = mixture_enthalpy(&comps, &z, t_known, p, t_ref, flash, dep);

        let r = flash_ph(
            &comps,
            &z,
            p,
            h_known,
            t_ref,
            250.0,
            flash,
            dep,
            EnergyFlashOptions::default(),
        )
        .expect("converges");

        assert_relative_eq!(r.temperature, t_known, max_relative = 1e-7);
        assert!(r.residual <= 1e-6);
    }

    /// **Methodology.** Verify the phase-weighting arithmetic of
    /// [`mixture_enthalpy`] directly. With a synthetic two-phase split
    /// (β = 0.5, distinct `x`/`y`) and a departure closure returning fixed,
    /// phase-dependent constants (Vapor → 500 J/mol, Liquid → −300 J/mol), the
    /// departure contribution must be exactly `0.5·500 + 0.5·(−300) = 100`
    /// J/mol on top of the ideal-gas enthalpy. Cp is constant A = 29.1, so the
    /// ideal part at T = 400, T_ref = 298.15 is `29.1·101.85 = 2963.835` J/mol.
    /// **Results (2026-07-24):** mixture_enthalpy = 2963.835 + 100 = 3063.835
    /// J/mol, matching the hand value to < 1e-12 relative.
    #[test]
    fn mixture_enthalpy_phase_weights_departures() {
        let a = 29.1;
        let comps = [comp([a, 0.0, 0.0, 0.0, 0.0])];
        let z = [1.0];
        let (t, t_ref, p) = (400.0, 298.15, 5.0e5);

        let flash = |_t: f64, _p: f64| FlashResult {
            beta: 0.5,
            x: vec![1.0],
            y: vec![1.0],
            k: vec![1.0],
            iterations: 0,
        };
        let dep = |_f: &[f64], _t: f64, _p: f64, ph: Phase| match ph {
            Phase::Vapor => 500.0,
            Phase::Liquid => -300.0,
        };

        let h = mixture_enthalpy(&comps, &z, t, p, t_ref, flash, dep);
        let expected = a * (t - t_ref) + 0.5 * 500.0 + 0.5 * (-300.0);
        assert_relative_eq!(h, expected, max_relative = 1e-12);
        assert_relative_eq!(h, 3063.835, max_relative = 1e-9);
    }

    /// **Methodology.** Monotonicity: a higher enthalpy target must map to a
    /// higher converged temperature (H is strictly increasing in T). Pure
    /// constant-Cp ideal stream, three ascending targets 2000 < 6000 < 10000
    /// J/mol. **Results (2026-07-24):** the converged temperatures are strictly
    /// increasing — T(2000) = 366.8785 K < T(6000) = 504.3356 K < T(10000) =
    /// 641.7926 K — and each equals the analytic `T_ref + H/A` to < 1e-9.
    #[test]
    fn higher_enthalpy_target_gives_higher_temperature() {
        let a = 29.1;
        let c = [comp([a, 0.0, 0.0, 0.0, 0.0])];
        let z = [1.0];
        let (t_ref, p) = (298.15, 1.0e5);
        let flash = all_vapour(&z);
        let dep = |_f: &[f64], _t: f64, _p: f64, _ph: Phase| 0.0;
        let flash = &flash;
        let dep = &dep;

        let solve = |h: f64| {
            flash_ph(
                &c,
                &z,
                p,
                h,
                t_ref,
                300.0,
                flash,
                dep,
                EnergyFlashOptions::default(),
            )
            .expect("converges")
            .temperature
        };

        let (t1, t2, t3) = (solve(2000.0), solve(6000.0), solve(10000.0));
        assert!(
            t1 < t2 && t2 < t3,
            "temperatures not monotone: {t1} {t2} {t3}"
        );
        assert_relative_eq!(t1, t_ref + 2000.0 / a, max_relative = 1e-9);
        assert_relative_eq!(t2, t_ref + 6000.0 / a, max_relative = 1e-9);
        assert_relative_eq!(t3, t_ref + 10000.0 / a, max_relative = 1e-9);
    }

    /// **Methodology.** Single-phase PS flash round-trip (the only PS case in
    /// scope). Constant Cp (A = 29.1), so ideal-gas entropy at P = P_ref is
    /// `S(T) = A·ln(T/T_ref)`, strictly increasing in T. Compute S at
    /// T_known = 420 K (P = P_ref, zero departures) and flash_ps back from a
    /// 300 K guess. **Results (2026-07-24):** S(420) = 29.1·ln(420/298.15) =
    /// 9.9759 J/(mol·K); flash_ps recovered T = 420.000000 K to < 1e-7
    /// relative, residual < 1e-6 J/(mol·K).
    #[test]
    fn single_phase_entropy_flash_round_trip() {
        let a = 29.1;
        let c = [comp([a, 0.0, 0.0, 0.0, 0.0])];
        let z = [1.0];
        let (t_ref, p_ref) = (298.15, 101_325.0);
        let p = p_ref;
        let t_known = 420.0;

        let flash = all_vapour(&z);
        let dep = |_f: &[f64], _t: f64, _p: f64, _ph: Phase| 0.0;
        let flash = &flash;
        let dep = &dep;

        let s_known = mixture_entropy(&c, &z, t_known, p, t_ref, p_ref, flash, dep);
        assert_relative_eq!(s_known, a * (t_known / t_ref).ln(), max_relative = 1e-12);

        let r = flash_ps(
            &c,
            &z,
            p,
            s_known,
            t_ref,
            p_ref,
            300.0,
            flash,
            dep,
            EnergyFlashOptions::default(),
        )
        .expect("converges");

        assert_relative_eq!(r.temperature, t_known, max_relative = 1e-7);
        assert!(r.residual <= 1e-6);
    }

    /// **Methodology.** Error paths. An empty composition and a
    /// components/`z` length mismatch must be rejected before any solve.
    /// **Results (2026-07-24):** empty → `Empty`; mismatched lengths →
    /// `LengthMismatch { a: 2, b: 1 }`.
    #[test]
    fn rejects_malformed_composition() {
        let c = [
            comp([29.1, 0.0, 0.0, 0.0, 0.0]),
            comp([37.0, 0.0, 0.0, 0.0, 0.0]),
        ];
        let flash = |_t: f64, _p: f64| FlashResult {
            beta: 1.0,
            x: vec![],
            y: vec![],
            k: vec![],
            iterations: 0,
        };
        let dep = |_f: &[f64], _t: f64, _p: f64, _ph: Phase| 0.0;
        let flash = &flash;
        let dep = &dep;

        let empty = flash_ph(
            &[],
            &[],
            1e5,
            0.0,
            298.15,
            300.0,
            flash,
            dep,
            EnergyFlashOptions::default(),
        );
        assert_eq!(empty.unwrap_err(), EnergyFlashError::Empty);

        let mismatch = flash_ph(
            &c,
            &[1.0],
            1e5,
            0.0,
            298.15,
            300.0,
            flash,
            dep,
            EnergyFlashOptions::default(),
        );
        assert_eq!(
            mismatch.unwrap_err(),
            EnergyFlashError::LengthMismatch { a: 2, b: 1 }
        );
    }
}
