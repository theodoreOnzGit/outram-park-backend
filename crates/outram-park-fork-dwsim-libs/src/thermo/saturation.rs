//! Saturation: **bubble-point / dew-point** temperature & pressure of a
//! multicomponent mixture, on top of the isothermal-isobaric VLE kernel.
//!
//! Ported/adapted from DWSIM `DWSIM.Thermodynamics/FlashAlgorithms/NestedLoops.vb`
//! (GPL-3.0): `Flash_PV` (bubble/dew *temperature* at fixed pressure, the
//! `V = 0` / `V = 1` branches — line 2662) and `Flash_TV` (bubble/dew *pressure*
//! at fixed temperature — line 2028). Both DWSIM routines specialise to the
//! incipient-phase condition when the vapour molar fraction `V` is exactly `0`
//! (bubble, incipient vapour) or `1` (dew, incipient liquid); the initial K
//! estimates use the vapour-pressure / Wilson seed
//! (`PropertyPackage.vb::DW_CalcKvalue_Ideal_Wilson`, L1650-1668, mirrored by
//! [`crate::thermo::flash::wilson_k_values`]). GUI / serialization / flowsheet /
//! AI-assisted-convergence scaffolding around those routines is **not** ported.
//!
//! # What this module computes
//!
//! For a feed of overall mole fractions `z_i` \[-\] the **saturation curve** is
//! the locus where an infinitesimal amount of a second phase first appears:
//!
//! - **Bubble point** — the feed is a saturated liquid (`x_i = z_i`) about to
//!   boil; the incipient vapour has `y_i = K_i z_i`, and the saturation
//!   condition is
//!
//!   ```text
//!   Σ_i K_i z_i = 1.
//!   ```
//!
//! - **Dew point** — the feed is a saturated vapour (`y_i = z_i`) about to
//!   condense; the incipient liquid has `x_i = z_i / K_i`, and the condition is
//!
//!   ```text
//!   Σ_i z_i / K_i = 1.
//!   ```
//!
//! Here `K_i = y_i / x_i` \[-\] are the equilibrium ratios from a fugacity /
//! activity property model. Four public entry points solve for the missing
//! intensive variable:
//!
//! | Function | Fixed | Solved for | Incipient phase |
//! |---|---|---|---|
//! | [`bubble_pressure`] | `T` | `P` | vapour `y_i = K_i z_i` |
//! | [`dew_pressure`]    | `T` | `P` | liquid `x_i = z_i / K_i` |
//! | [`bubble_temperature`] | `P` | `T` | vapour `y_i = K_i z_i` |
//! | [`dew_temperature`]    | `P` | `T` | liquid `x_i = z_i / K_i` |
//!
//! # The K-value source (decoupling boundary — no `dyn`)
//!
//! Each public function comes in two forms:
//!
//! - a convenience form taking a [`PropertyPackageModel`] (its
//!   [`PropertyPackageModel::k_values`] supplies the K-values — Wilson for the
//!   ideal package, a fugacity-coefficient ratio for the cubic EOS packages);
//! - a generic `*_with` form taking a **generic `Fn` closure**
//!   `k_values(x, y, T, P) -> Vec<f64>` (any ln-φ / K model), *not* a trait
//!   object. This keeps the module free of `dyn` dispatch and independent of the
//!   EOS/activity code, matching the [`crate::thermo::flash`] and
//!   [`crate::thermo::property_package`] pattern (no `Box`, no lifetimes, no
//!   channels).
//!
//! For a **composition-dependent** K-model (e.g. a cubic EOS) the incipient
//! composition feeds back into the K-values, so each residual evaluation runs a
//! short inner successive-substitution loop on the incipient phase before the
//! saturation residual is read off. For a **composition-independent** K-model
//! (ideal / Wilson / Raoult) that inner loop is trivial and the residual is a
//! plain monotone function of the solved variable.
//!
//! **Trivial-solution caveat (composition-dependent K only).** Plain successive
//! substitution has the trivial solution (`K → 1`, incipient composition `→ z`)
//! as an *attracting* fixed point. The Wilson-seeded start breaks the initial
//! symmetry but does not guarantee escape from it for a cubic EOS, so a
//! cubic-EOS bubble/dew solve here can converge to that spurious point. Robust
//! cubic saturation requires a phase-stability / tangent-plane pre-test
//! ([`crate::thermo::stability`]) that is **not** wired in. The **verified,
//! relied-upon** path in this module is therefore the composition-independent
//! K-model (ideal/Wilson/Raoult); the cubic path is offered but carries no
//! robustness guarantee — treat its result as unverified until a stability
//! pre-test is added.
//!
//! # Convergence
//!
//! - **Initial guess.** From the Wilson K-values ([`crate::thermo::flash::wilson_k_values`]).
//!   Because Wilson `K_i = (Pc_i / P) exp[5.373 (1 + ω_i)(1 − Tc_i / T)]`, the
//!   bubble/dew *pressure* has a closed-form Wilson seed
//!   (`P_bub = Σ z_i Pc_i E_i`, `P_dew = 1 / Σ z_i /(Pc_i E_i)` with
//!   `E_i = exp[5.373(1+ω_i)(1−Tc_i/T)]`); the *temperature* seed inverts the
//!   same expression per component and takes the feed-weighted mean.
//! - **Root find.** A **safeguarded** scalar solver: geometric bracket
//!   expansion around the Wilson seed to obtain a sign-changing interval,
//!   followed by the **Illinois** modified-false-position iteration (globally
//!   convergent on a bracketed continuous residual, with super-linear local
//!   rate). The saturation residual (`Σ K_i z_i − 1` or `Σ z_i/K_i − 1`) is
//!   driven below a tolerance.
//!
//! # Honest scope — verification, NOT benchmark validation
//!
//! - **Two-phase VLE saturation only.** Bubble/dew of a single vapour–liquid
//!   equilibrium. **No** three-phase (VLLE), solid/salt-out, or electrolyte
//!   saturation; **no** retrograde-region robustness guarantees (near the
//!   critical point a bubble/dew *pressure* isotherm can be non-monotone or
//!   double-valued — the safeguarded solver returns the first bracketed root it
//!   finds, which may not be the physically intended branch there).
//! - **K-model is the caller's choice**: ideal/Wilson or a cubic EOS via
//!   [`PropertyPackageModel`], or any generic K-closure. Binary interaction
//!   parameters follow whatever the supplied model uses (the cubic packages here
//!   use `k_ij = 0`; see [`crate::thermo::property_package`]).
//! - **Verified, not validated.** The tests below check the *defining
//!   saturation identity*, incipient-composition normalisation, the
//!   bubble > dew pressure ordering, a pure-component collapse to the vapour
//!   pressure, and pressure/temperature round-tripping — internal consistency
//!   against closed-form relations, **not** experimental / NIST / DECHEMA VLE
//!   data. AI-assisted port: untrusted draft material until human-reviewed per
//!   the crate `CLAUDE.md`. Not for nuclear facility operation, reactor control,
//!   safety-critical, or licensing decisions. Independent OUTRAM PARK fork, not
//!   the official DWSIM.
//!
//! # Units — documented raw `f64` (SI), per the crate `CLAUDE.md`
//!
//! This module sits directly on the `f64`-SI kernel ([`crate::thermo::flash`],
//! [`crate::thermo::property_package`]), whose inner arithmetic deliberately uses
//! raw `f64` in SI base units rather than `uom` (the crate `CLAUDE.md` "raw f64
//! in inner EOS loops" rule). To avoid wrapping/unwrapping `uom` at every call
//! into that kernel the signatures here follow the same convention: temperature
//! `T` \[K\], pressure `P` \[Pa\], mole fractions \[-\], all `f64`, with every
//! parameter's unit spelled out. A `uom`-typed façade, if wanted, belongs at the
//! equipment-model boundary, not in this inner composition layer.

use crate::thermo::flash::wilson_k_values;
use crate::thermo::property_package::PropertyPackageModel;
use crate::thermo::Component;

/// A converged saturation point (one point on the bubble or dew curve).
///
/// All compositions are mole fractions \[-\]. Exactly one of `temperature` /
/// `pressure` is the solved-for unknown; the other is the fixed specification
/// the caller supplied.
#[derive(Debug, Clone, PartialEq)]
pub struct SaturationState {
    /// Temperature `T` \[K\] at the saturation point (the fixed value for the
    /// `*_pressure` solvers, the solved value for the `*_temperature` solvers).
    pub temperature: f64,
    /// Pressure `P` \[Pa\] at the saturation point (solved for the `*_pressure`
    /// solvers, fixed for the `*_temperature` solvers).
    pub pressure: f64,
    /// Incipient-phase mole fractions \[-\]: the **vapour** `y_i = K_i z_i` for a
    /// bubble point, the **liquid** `x_i = z_i / K_i` for a dew point. At exact
    /// convergence these sum to `1` (their sum is `residual + 1`).
    pub incipient: Vec<f64>,
    /// Equilibrium K-values `K_i = y_i / x_i` \[-\] at the returned state.
    pub k: Vec<f64>,
    /// Inner successive-substitution iterations used at the final residual
    /// evaluation (`1`–`2` for a composition-independent K-model, more for a
    /// cubic EOS).
    pub iterations: usize,
    /// Saturation residual at the returned state: `Σ K_i z_i − 1` for a bubble
    /// point, `Σ z_i / K_i − 1` for a dew point \[-\]. `|residual|` is below the
    /// solver's function tolerance on success.
    pub residual: f64,
}

/// Error conditions for the saturation solvers.
#[derive(Debug, Clone, PartialEq, thiserror::Error)]
pub enum SaturationError {
    /// An empty feed was supplied (need at least one component).
    #[error("empty composition")]
    Empty,
    /// Two slices that must share a length did not.
    #[error("slice length mismatch: got {a} and {b}")]
    LengthMismatch {
        /// Length of the first slice (e.g. `z`).
        a: usize,
        /// Length of the second slice (e.g. `components` or the returned `K`).
        b: usize,
    },
    /// A non-finite value (`NaN`/`inf`) appeared in an input or a returned
    /// K-value.
    #[error("non-finite value in input or K-values")]
    NonFinite,
    /// A quantity that must be strictly positive was not.
    #[error("`{what}` must be finite and > 0 (got {value})")]
    NonPositive {
        /// Which quantity (e.g. `"temperature"`).
        what: &'static str,
        /// The offending value.
        value: f64,
    },
    /// The bracket-expansion phase could not find a sign change of the
    /// saturation residual within its bounds (e.g. no saturation point exists
    /// at the given specification, or it lies outside the search bounds).
    #[error("could not bracket a root for {var} (best |residual| = {residual:e})")]
    NoBracket {
        /// The variable being solved for (`"pressure"` or `"temperature"`).
        var: &'static str,
        /// Smallest `|residual|` seen while trying to bracket.
        residual: f64,
    },
    /// The residual was driven to zero by the *trivial* solution `K_i ≡ 1`
    /// rather than by a real phase split.
    ///
    /// Above the mixture's critical region a cubic EOS returns the same
    /// compressibility root for both phases, so every K-value collapses to 1
    /// and `Σ K_i z_i − 1` vanishes identically. That satisfies the residual
    /// without describing a saturation point at all, and accepting it would
    /// report a confident temperature hundreds of kelvin above the real one.
    #[error("residual met only by the trivial solution K = 1 at {var} = {at} (no phase split)")]
    TrivialSolution {
        /// The variable being solved for.
        var: &'static str,
        /// Where the trivial solution was found.
        at: f64,
    },
    /// The root iteration did not reach the residual tolerance in budget.
    #[error("root find for {var} did not converge in {iterations} iterations (|residual| = {residual:e})")]
    NotConverged {
        /// The variable being solved for.
        var: &'static str,
        /// Iterations attempted.
        iterations: usize,
        /// Final `|residual|`.
        residual: f64,
    },
}

/// Tuning parameters for the safeguarded saturation root finds.
///
/// Defaults are tight enough for the verification tests: a small saturation-
/// residual tolerance and generous physical search bounds. All fields are `f64`
/// / `usize` with the units noted.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct SaturationOptions {
    /// Convergence tolerance on the saturation residual `|Σ K z − 1|` (bubble)
    /// or `|Σ z/K − 1|` (dew) \[-\].
    pub f_tol: f64,
    /// Relative convergence tolerance on the solved variable (`P` or `T`) \[-\].
    pub x_rel_tol: f64,
    /// Maximum outer (Illinois) iterations before [`SaturationError::NotConverged`].
    pub max_outer: usize,
    /// Inner successive-substitution tolerance on the incipient composition
    /// (max per-component change) \[-\].
    pub inner_tol: f64,
    /// Maximum inner successive-substitution iterations per residual evaluation.
    pub inner_max: usize,
    /// Lower bound on the pressure search \[Pa\].
    pub p_min: f64,
    /// Upper bound on the pressure search \[Pa\].
    pub p_max: f64,
    /// Lower bound on the temperature search \[K\].
    pub t_min: f64,
    /// Upper bound on the temperature search \[K\].
    pub t_max: f64,
}

impl Default for SaturationOptions {
    fn default() -> Self {
        Self {
            f_tol: 1.0e-11,
            x_rel_tol: 1.0e-12,
            max_outer: 200,
            inner_tol: 1.0e-13,
            inner_max: 200,
            p_min: 1.0e-1,
            p_max: 1.0e12,
            t_min: 1.0,
            t_max: 2.0e4,
        }
    }
}

// ---------------------------------------------------------------------------
// Input validation
// ---------------------------------------------------------------------------

fn validate_feed(components: &[Component], z: &[f64]) -> Result<(), SaturationError> {
    if z.is_empty() {
        return Err(SaturationError::Empty);
    }
    if components.len() != z.len() {
        return Err(SaturationError::LengthMismatch {
            a: z.len(),
            b: components.len(),
        });
    }
    if z.iter().any(|v| !v.is_finite()) {
        return Err(SaturationError::NonFinite);
    }
    Ok(())
}

fn require_positive(what: &'static str, value: f64) -> Result<(), SaturationError> {
    if !value.is_finite() || value <= 0.0 {
        Err(SaturationError::NonPositive { what, value })
    } else {
        Ok(())
    }
}

// ---------------------------------------------------------------------------
// Wilson initial guesses (closed form for pressure; per-component invert for T)
// ---------------------------------------------------------------------------

/// Wilson closed-form **bubble-pressure** seed `P = Σ z_i Pc_i E_i` \[Pa\], with
/// `Pc_i E_i = wilson_k_values(comps, T, 1 Pa)_i`.
fn wilson_bubble_pressure_guess(components: &[Component], z: &[f64], t: f64) -> f64 {
    let pe = wilson_k_values(components, t, 1.0); // Pc_i * E_i
    z.iter().zip(pe.iter()).map(|(&zi, &w)| zi * w).sum()
}

/// Wilson closed-form **dew-pressure** seed `P = 1 / Σ z_i /(Pc_i E_i)` \[Pa\].
fn wilson_dew_pressure_guess(components: &[Component], z: &[f64], t: f64) -> f64 {
    let pe = wilson_k_values(components, t, 1.0); // Pc_i * E_i
    let s: f64 = z.iter().zip(pe.iter()).map(|(&zi, &w)| zi / w).sum();
    1.0 / s
}

/// Feed-weighted Wilson **temperature** seed \[K\] at fixed `P`: for each
/// component invert Wilson `K_i = 1`, i.e.
/// `T_i = Tc_i / (1 − ln(P / Pc_i) / (5.373 (1 + ω_i)))`, then take `Σ z_i T_i`.
/// Falls back to the feed-weighted critical temperature for any component whose
/// inversion is non-finite or non-positive.
fn wilson_temperature_guess(components: &[Component], z: &[f64], p: f64) -> f64 {
    let mut acc = 0.0;
    for (c, &zi) in components.iter().zip(z.iter()) {
        let denom = 1.0 - (p / c.critical_pressure).ln() / (5.373 * (1.0 + c.acentric_factor));
        let ti = c.critical_temperature / denom;
        let ti = if ti.is_finite() && ti > 0.0 {
            ti
        } else {
            c.critical_temperature
        };
        acc += zi * ti;
    }
    acc
}

// ---------------------------------------------------------------------------
// Saturation residual evaluation (with inner incipient-composition loop)
// ---------------------------------------------------------------------------

/// Which saturation curve is being evaluated.
#[derive(Clone, Copy)]
enum Curve {
    /// Bubble: liquid is the feed `z`, incipient vapour `y_i = K_i z_i`,
    /// residual `Σ K_i z_i − 1`.
    Bubble,
    /// Dew: vapour is the feed `z`, incipient liquid `x_i = z_i / K_i`,
    /// residual `Σ z_i / K_i − 1`.
    Dew,
}

/// One residual evaluation at `(T, P)`: run the inner successive-substitution
/// loop on the incipient phase to self-consistency, then return
/// `(residual, incipient_raw, k, inner_iters)`.
///
/// `incipient_raw` is the **un-normalised** incipient composition
/// (`K_i z_i` for bubble, `z_i / K_i` for dew); its sum is `residual + 1`.
///
/// The incipient phase is seeded from the **Wilson** K-values (not from the feed
/// `z` itself): `y0 ∝ K_i^W z_i` for a bubble, `x0 ∝ z_i / K_i^W` for a dew.
/// Seeding off the feed would place the very first K evaluation at identical
/// liquid and vapour compositions (`x = y = z`). The Wilson seed breaks that
/// initial symmetry, which is sufficient for a composition-**independent**
/// K-model (ideal/Wilson/Raoult), where the loop converges in one pass to the
/// physical incipient phase.
///
/// For a composition-**dependent** K-model (a cubic EOS) this is only a
/// heuristic, **not** a cure: the trivial branch (`K → 1`, incipient `→ z`) is an
/// attracting fixed point of plain successive substitution, so the loop can still
/// drift back to the spurious solution. Robust cubic-EOS saturation needs a
/// phase-stability / tangent-plane pre-test ([`crate::thermo::stability`]), which
/// is **not** wired in here — see the module scope note. The tested,
/// relied-upon path is the composition-independent one.
fn eval_curve<F>(
    curve: Curve,
    components: &[Component],
    z: &[f64],
    t: f64,
    p: f64,
    k_values: &F,
    opts: &SaturationOptions,
) -> Result<(f64, Vec<f64>, Vec<f64>, usize), SaturationError>
where
    F: Fn(&[f64], &[f64], f64, f64) -> Vec<f64>,
{
    let n = z.len();
    // Wilson-seeded incipient phase (breaks the trivial x = y = z symmetry).
    let kw = wilson_k_values(components, t, p);
    let mut incip = vec![0.0; n];
    let mut seed_sum = 0.0;
    for i in 0..n {
        incip[i] = match curve {
            Curve::Bubble => kw[i] * z[i],
            Curve::Dew => z[i] / kw[i],
        };
        seed_sum += incip[i];
    }
    if seed_sum.is_finite() && seed_sum > 0.0 {
        for v in incip.iter_mut() {
            *v /= seed_sum;
        }
    } else {
        incip.copy_from_slice(z);
    }
    let mut inner = 0usize;
    loop {
        // Assemble the (x, y) pair for the K-model: for a bubble point the
        // liquid is the feed and the vapour is the incipient estimate; for a
        // dew point it is the other way round.
        let k = match curve {
            Curve::Bubble => k_values(z, &incip, t, p),
            Curve::Dew => k_values(&incip, z, t, p),
        };
        if k.len() != n {
            return Err(SaturationError::LengthMismatch { a: n, b: k.len() });
        }
        if k.iter().any(|v| !v.is_finite()) {
            return Err(SaturationError::NonFinite);
        }

        // Raw incipient composition and its sum = residual + 1.
        let mut raw = vec![0.0; n];
        let mut s = 0.0;
        for i in 0..n {
            raw[i] = match curve {
                Curve::Bubble => k[i] * z[i],
                Curve::Dew => z[i] / k[i],
            };
            s += raw[i];
        }
        if !s.is_finite() || s == 0.0 {
            return Err(SaturationError::NonFinite);
        }

        // Normalised incipient composition fed back into the next K evaluation.
        let mut diff = 0.0_f64;
        let mut next = vec![0.0; n];
        for i in 0..n {
            next[i] = raw[i] / s;
            diff = diff.max((next[i] - incip[i]).abs());
        }
        incip = next;
        inner += 1;

        if diff < opts.inner_tol || inner >= opts.inner_max {
            return Ok((s - 1.0, raw, k, inner));
        }
    }
}

// ---------------------------------------------------------------------------
// Safeguarded scalar root find (bracket expansion + Illinois false position)
// ---------------------------------------------------------------------------

/// Sweep `[x_min, x_max]` for the lowest sign change of a residual, returning
/// the bracketing pair. `None` if no usable sign change exists.
///
/// `f` returns `Ok(None)` where the residual is *degenerate* -- the trivial
/// `K ≡ 1` branch, on which the residual is identically zero and says nothing
/// about saturation. Those points are holes in the curve, not roots: a cubic
/// EOS produces them at both ends of the temperature range (below, where only
/// a liquid-like root exists; above, past the critical region), and bracketing
/// across one lands the solve hundreds of kelvin from the real answer.
///
/// Deliberately a uniform scan rather than another expansion heuristic: it is
/// the fallback for the case where a heuristic was already fooled, so it must
/// not itself depend on the residual's shape.
/// Is this K-set the *trivial* solution `K_i ≡ 1` — a residual satisfied
/// without any phase split?
///
/// A multicomponent saturation residual is satisfied identically when every
/// K-value is 1, which happens wherever a cubic EOS returns one
/// compressibility root for both phases: above the critical region, and again
/// far below it. A converged temperature resting on that is not a saturation
/// point.
///
/// **A single component is the exception, and it is not a corner case.** For
/// pure `i`, `Σ K_i z_i − 1 = K_i − 1`, so `K = 1` *is* the saturation
/// condition — it is what `Tsat` means. Applying the trivial-solution test
/// there would reject every correct answer.
fn is_trivial_k(k: &[f64]) -> bool {
    k.len() > 1
        && k.iter()
            .all(|&ki| ki.is_finite() && (ki - 1.0).abs() < 1e-8)
}

/// Solve by scanning for the *lowest* sign change rather than expanding from a
/// guess, skipping degenerate stretches. Used when the expansion path returned
/// the trivial branch (see [`SaturationError::TrivialSolution`]).
#[allow(clippy::too_many_arguments)]
fn solve_scalar_lowest<F, G>(
    f: &F,
    probe: &G,
    x_min: f64,
    x_max: f64,
    x_rel_tol: f64,
    f_tol: f64,
    max_iter: usize,
    var: &'static str,
) -> Result<f64, SaturationError>
where
    F: Fn(f64) -> Result<f64, SaturationError>,
    G: Fn(f64) -> Result<Option<f64>, SaturationError>,
{
    match scan_for_bracket(probe, x_min, x_max)? {
        Some((a, fa, b, fb)) => refine_bracket(f, a, fa, b, fb, x_rel_tol, f_tol, max_iter, var),
        None => Err(SaturationError::NoBracket {
            var,
            residual: f64::INFINITY,
        }),
    }
}

fn scan_for_bracket<F>(
    f: &F,
    x_min: f64,
    x_max: f64,
) -> Result<Option<(f64, f64, f64, f64)>, SaturationError>
where
    F: Fn(f64) -> Result<Option<f64>, SaturationError>,
{
    const STEPS: usize = 192;
    if !(x_max > x_min) {
        return Ok(None);
    }
    let dx = (x_max - x_min) / STEPS as f64;
    let mut prev: Option<(f64, f64)> = None;
    for i in 0..=STEPS {
        let x = x_min + dx * i as f64;
        let fx = f(x)?;
        match (prev, fx) {
            (Some((px, pf)), Some(cf)) if pf * cf < 0.0 => {
                return Ok(Some((px, pf, x, cf)));
            }
            _ => {}
        }
        prev = fx.map(|v| (x, v));
    }
    Ok(None)
}

/// Solve `f(x) = 0` for a scalar `x ∈ [x_min, x_max]` given an initial guess
/// `x0`, using geometric bracket expansion followed by the Illinois modified
/// false-position iteration. `var` labels the unknown for error reporting.
#[allow(clippy::too_many_arguments)]
fn solve_scalar<F>(
    f: &F,
    x0: f64,
    x_min: f64,
    x_max: f64,
    x_rel_tol: f64,
    f_tol: f64,
    max_iter: usize,
    var: &'static str,
) -> Result<f64, SaturationError>
where
    F: Fn(f64) -> Result<f64, SaturationError>,
{
    let clamp = |x: f64| x.max(x_min).min(x_max);
    let x0 = clamp(x0);
    let f0 = f(x0)?;
    if f0.abs() < f_tol {
        return Ok(x0);
    }

    // Bracket a sign change by expanding a small interval around x0 geometrically.
    let mut a = clamp(x0 * 0.7);
    let mut b = clamp(x0 * 1.3);
    if a == b {
        // x0 pinned at a bound; nudge.
        a = clamp(x0 - (0.1 * x0).abs().max(1.0));
        b = clamp(x0 + (0.1 * x0).abs().max(1.0));
    }
    let mut fa = f(a)?;
    let mut fb = f(b)?;
    let mut best = fa.abs().min(fb.abs());
    let mut expand = 0;
    while fa * fb > 0.0 {
        if a <= x_min && b >= x_max {
            break;
        }
        if fa.abs() < fb.abs() {
            a = clamp(a + 1.6 * (a - b));
            fa = f(a)?;
        } else {
            b = clamp(b + 1.6 * (b - a));
            fb = f(b)?;
        }
        best = best.min(fa.abs()).min(fb.abs());
        expand += 1;
        if expand > 100 {
            break;
        }
    }
    if fa * fb > 0.0 {
        return Err(SaturationError::NoBracket {
            var,
            residual: best,
        });
    }

    refine_bracket(f, a, fa, b, fb, x_rel_tol, f_tol, max_iter, var)
}

/// Solve for the root inside a bracket already known to contain one.
///
/// Illinois modified false position: keeps `[a, b]` bracketing the root
/// (`fa`, `fb` opposite signs) and halves the stagnant endpoint's function
/// value to break the one-sided-convergence slowdown of plain regula falsi.
#[allow(clippy::too_many_arguments)]
fn refine_bracket<F>(
    f: &F,
    mut a: f64,
    mut fa: f64,
    mut b: f64,
    mut fb: f64,
    x_rel_tol: f64,
    f_tol: f64,
    max_iter: usize,
    var: &'static str,
) -> Result<f64, SaturationError>
where
    F: Fn(f64) -> Result<f64, SaturationError>,
{
    let mut side = 0i32;
    let mut last_f = fa;
    for _ in 0..max_iter {
        let c = (fa * b - fb * a) / (fa - fb);
        let fc = f(c)?;
        last_f = fc;
        if fc.abs() < f_tol || (b - a).abs() <= x_rel_tol * c.abs().max(1.0) {
            return Ok(c);
        }
        if fc * fa < 0.0 {
            // Root in [a, c]: move b down to c.
            b = c;
            fb = fc;
            if side == -1 {
                fa *= 0.5;
            }
            side = -1;
        } else {
            // Root in [c, b]: move a up to c.
            a = c;
            fa = fc;
            if side == 1 {
                fb *= 0.5;
            }
            side = 1;
        }
    }
    Err(SaturationError::NotConverged {
        var,
        iterations: max_iter,
        residual: last_f.abs(),
    })
}

// ---------------------------------------------------------------------------
// Generic (K-closure) public API
// ---------------------------------------------------------------------------

/// Bubble **pressure** at fixed temperature with a generic K-closure.
///
/// Solves `Σ_i K_i(z, y, T, P) z_i = 1` for `P` \[Pa\], with the incipient
/// vapour `y_i = K_i z_i`. `k_values(x, y, T, P) -> Vec<f64>` is any K-model
/// (generic `Fn`, no `dyn`); it is called with the liquid `x = z` and the
/// current incipient vapour `y`.
///
/// # Units / ranges
///
/// `components.len() == z.len()`; `z` are feed mole fractions \[-\] (physical
/// feeds sum to 1); `temperature` `T` \[K\] > 0. Returns a [`SaturationState`]
/// whose `pressure` is the solved bubble pressure \[Pa\] and whose `incipient`
/// is the raw incipient vapour `K_i z_i` \[-\].
///
/// # Errors
///
/// [`SaturationError::Empty`] / [`SaturationError::LengthMismatch`] /
/// [`SaturationError::NonFinite`] on bad inputs, [`SaturationError::NonPositive`]
/// for `T ≤ 0`, and [`SaturationError::NoBracket`] /
/// [`SaturationError::NotConverged`] if the safeguarded solve fails (e.g. no
/// bubble point in the search bounds).
pub fn bubble_pressure_with<F>(
    components: &[Component],
    z: &[f64],
    temperature: f64,
    k_values: F,
    opts: SaturationOptions,
) -> Result<SaturationState, SaturationError>
where
    F: Fn(&[f64], &[f64], f64, f64) -> Vec<f64>,
{
    validate_feed(components, z)?;
    require_positive("temperature", temperature)?;

    let p0 = wilson_bubble_pressure_guess(components, z, temperature);
    let resid = |p: f64| -> Result<f64, SaturationError> {
        eval_curve(
            Curve::Bubble,
            components,
            z,
            temperature,
            p,
            &k_values,
            &opts,
        )
        .map(|r| r.0)
    };
    let p = solve_scalar(
        &resid,
        p0,
        opts.p_min,
        opts.p_max,
        opts.x_rel_tol,
        opts.f_tol,
        opts.max_outer,
        "pressure",
    )?;
    let (residual, incip, k, iterations) = eval_curve(
        Curve::Bubble,
        components,
        z,
        temperature,
        p,
        &k_values,
        &opts,
    )?;
    Ok(SaturationState {
        temperature,
        pressure: p,
        incipient: incip,
        k,
        iterations,
        residual,
    })
}

/// Dew **pressure** at fixed temperature with a generic K-closure.
///
/// Solves `Σ_i z_i / K_i(x, z, T, P) = 1` for `P` \[Pa\], with the incipient
/// liquid `x_i = z_i / K_i`. The closure is called with the current incipient
/// liquid `x` and the vapour `y = z`.
///
/// Units / ranges / errors mirror [`bubble_pressure_with`]; `incipient` is the
/// raw incipient liquid `z_i / K_i` \[-\].
pub fn dew_pressure_with<F>(
    components: &[Component],
    z: &[f64],
    temperature: f64,
    k_values: F,
    opts: SaturationOptions,
) -> Result<SaturationState, SaturationError>
where
    F: Fn(&[f64], &[f64], f64, f64) -> Vec<f64>,
{
    validate_feed(components, z)?;
    require_positive("temperature", temperature)?;

    let p0 = wilson_dew_pressure_guess(components, z, temperature);
    let resid = |p: f64| -> Result<f64, SaturationError> {
        eval_curve(Curve::Dew, components, z, temperature, p, &k_values, &opts).map(|r| r.0)
    };
    let p = solve_scalar(
        &resid,
        p0,
        opts.p_min,
        opts.p_max,
        opts.x_rel_tol,
        opts.f_tol,
        opts.max_outer,
        "pressure",
    )?;
    let (residual, incip, k, iterations) =
        eval_curve(Curve::Dew, components, z, temperature, p, &k_values, &opts)?;
    Ok(SaturationState {
        temperature,
        pressure: p,
        incipient: incip,
        k,
        iterations,
        residual,
    })
}

/// Bubble **temperature** at fixed pressure with a generic K-closure.
///
/// Solves `Σ_i K_i(z, y, T, P) z_i = 1` for `T` \[K\], with the incipient
/// vapour `y_i = K_i z_i`.
///
/// # Units / ranges
///
/// `components.len() == z.len()`; `pressure` `P` \[Pa\] > 0. Returns a
/// [`SaturationState`] whose `temperature` is the solved bubble temperature \[K\].
/// Errors mirror [`bubble_pressure_with`] (with `var = "temperature"`).
pub fn bubble_temperature_with<F>(
    components: &[Component],
    z: &[f64],
    pressure: f64,
    k_values: F,
    opts: SaturationOptions,
) -> Result<SaturationState, SaturationError>
where
    F: Fn(&[f64], &[f64], f64, f64) -> Vec<f64>,
{
    validate_feed(components, z)?;
    require_positive("pressure", pressure)?;

    let t0 = wilson_temperature_guess(components, z, pressure);
    let resid = |t: f64| -> Result<f64, SaturationError> {
        eval_curve(Curve::Bubble, components, z, t, pressure, &k_values, &opts).map(|r| r.0)
    };
    // Same residual, but reporting `None` where the K-set has collapsed to the
    // trivial `K ≡ 1` branch and the residual is therefore uninformative.
    let probe = |t: f64| -> Result<Option<f64>, SaturationError> {
        eval_curve(Curve::Bubble, components, z, t, pressure, &k_values, &opts)
            .map(|(r, _, k, _)| if is_trivial_k(&k) { None } else { Some(r) })
    };
    // The fast path expands a bracket outward from the Wilson guess. With a
    // cubic EOS that can fail two ways: it walks off into the degenerate
    // branch above the critical region (where every K is 1, the residual
    // vanishes, and no phase split exists), or it chases that branch's
    // asymptote and never brackets anything. Both are recoverable by scanning
    // for the *lowest* non-degenerate sign change, which is the physical
    // saturation point.
    let fast = solve_scalar(
        &resid,
        t0,
        opts.t_min,
        opts.t_max,
        opts.x_rel_tol,
        opts.f_tol,
        opts.max_outer,
        "temperature",
    );
    let t = match fast {
        Ok(t) => t,
        Err(SaturationError::NoBracket { .. }) => solve_scalar_lowest(
            &resid,
            &probe,
            opts.t_min,
            opts.t_max,
            opts.x_rel_tol,
            opts.f_tol,
            opts.max_outer,
            "temperature",
        )?,
        Err(e) => return Err(e),
    };
    let (residual, incip, k, iterations) =
        eval_curve(Curve::Bubble, components, z, t, pressure, &k_values, &opts)?;
    let (t, residual, incip, k, iterations) = if is_trivial_k(&k) {
        let t2 = solve_scalar_lowest(
            &resid,
            &probe,
            opts.t_min,
            opts.t_max,
            opts.x_rel_tol,
            opts.f_tol,
            opts.max_outer,
            "temperature",
        )?;
        let (r2, i2, k2, n2) =
            eval_curve(Curve::Bubble, components, z, t2, pressure, &k_values, &opts)?;
        if is_trivial_k(&k2) {
            return Err(SaturationError::TrivialSolution {
                var: "temperature",
                at: t2,
            });
        }
        (t2, r2, i2, k2, n2)
    } else {
        (t, residual, incip, k, iterations)
    };
    Ok(SaturationState {
        temperature: t,
        pressure,
        incipient: incip,
        k,
        iterations,
        residual,
    })
}

/// Dew **temperature** at fixed pressure with a generic K-closure.
///
/// Solves `Σ_i z_i / K_i(x, z, T, P) = 1` for `T` \[K\], with the incipient
/// liquid `x_i = z_i / K_i`. Units / ranges / errors mirror
/// [`bubble_temperature_with`]; `incipient` is the raw incipient liquid.
pub fn dew_temperature_with<F>(
    components: &[Component],
    z: &[f64],
    pressure: f64,
    k_values: F,
    opts: SaturationOptions,
) -> Result<SaturationState, SaturationError>
where
    F: Fn(&[f64], &[f64], f64, f64) -> Vec<f64>,
{
    validate_feed(components, z)?;
    require_positive("pressure", pressure)?;

    let t0 = wilson_temperature_guess(components, z, pressure);
    let resid = |t: f64| -> Result<f64, SaturationError> {
        eval_curve(Curve::Dew, components, z, t, pressure, &k_values, &opts).map(|r| r.0)
    };
    // Same residual, but reporting `None` where the K-set has collapsed to the
    // trivial `K ≡ 1` branch and the residual is therefore uninformative.
    let probe =
        |t: f64| -> Result<Option<f64>, SaturationError> {
            eval_curve(Curve::Dew, components, z, t, pressure, &k_values, &opts)
                .map(|(r, _, k, _)| if is_trivial_k(&k) { None } else { Some(r) })
        };
    // The fast path expands a bracket outward from the Wilson guess. With a
    // cubic EOS that can fail two ways: it walks off into the degenerate
    // branch above the critical region (where every K is 1, the residual
    // vanishes, and no phase split exists), or it chases that branch's
    // asymptote and never brackets anything. Both are recoverable by scanning
    // for the *lowest* non-degenerate sign change, which is the physical
    // saturation point.
    let fast = solve_scalar(
        &resid,
        t0,
        opts.t_min,
        opts.t_max,
        opts.x_rel_tol,
        opts.f_tol,
        opts.max_outer,
        "temperature",
    );
    let t = match fast {
        Ok(t) => t,
        Err(SaturationError::NoBracket { .. }) => solve_scalar_lowest(
            &resid,
            &probe,
            opts.t_min,
            opts.t_max,
            opts.x_rel_tol,
            opts.f_tol,
            opts.max_outer,
            "temperature",
        )?,
        Err(e) => return Err(e),
    };
    let (residual, incip, k, iterations) =
        eval_curve(Curve::Dew, components, z, t, pressure, &k_values, &opts)?;
    let (t, residual, incip, k, iterations) = if is_trivial_k(&k) {
        let t2 = solve_scalar_lowest(
            &resid,
            &probe,
            opts.t_min,
            opts.t_max,
            opts.x_rel_tol,
            opts.f_tol,
            opts.max_outer,
            "temperature",
        )?;
        let (r2, i2, k2, n2) =
            eval_curve(Curve::Dew, components, z, t2, pressure, &k_values, &opts)?;
        if is_trivial_k(&k2) {
            return Err(SaturationError::TrivialSolution {
                var: "temperature",
                at: t2,
            });
        }
        (t2, r2, i2, k2, n2)
    } else {
        (t, residual, incip, k, iterations)
    };
    Ok(SaturationState {
        temperature: t,
        pressure,
        incipient: incip,
        k,
        iterations,
        residual,
    })
}

// ---------------------------------------------------------------------------
// Property-package convenience API
// ---------------------------------------------------------------------------

/// Bubble **pressure** at fixed temperature using a [`PropertyPackageModel`].
///
/// Convenience wrapper over [`bubble_pressure_with`] whose K-closure is the
/// package's [`PropertyPackageModel::k_values`] (Wilson for
/// [`PropertyPackageModel::Ideal`]; a fugacity-ratio for the cubic packages).
/// See [`bubble_pressure_with`] for the algorithm, units, and errors.
pub fn bubble_pressure(
    components: &[Component],
    z: &[f64],
    temperature: f64,
    package: PropertyPackageModel,
) -> Result<SaturationState, SaturationError> {
    bubble_pressure_with(
        components,
        z,
        temperature,
        |x, y, t, p| package.k_values(components, x, y, t, p),
        SaturationOptions::default(),
    )
}

/// Dew **pressure** at fixed temperature using a [`PropertyPackageModel`].
///
/// Convenience wrapper over [`dew_pressure_with`]. See it for details.
pub fn dew_pressure(
    components: &[Component],
    z: &[f64],
    temperature: f64,
    package: PropertyPackageModel,
) -> Result<SaturationState, SaturationError> {
    dew_pressure_with(
        components,
        z,
        temperature,
        |x, y, t, p| package.k_values(components, x, y, t, p),
        SaturationOptions::default(),
    )
}

/// Bubble **temperature** at fixed pressure using a [`PropertyPackageModel`].
///
/// Convenience wrapper over [`bubble_temperature_with`]. See it for details.
pub fn bubble_temperature(
    components: &[Component],
    z: &[f64],
    pressure: f64,
    package: PropertyPackageModel,
) -> Result<SaturationState, SaturationError> {
    bubble_temperature_with(
        components,
        z,
        pressure,
        |x, y, t, p| package.k_values(components, x, y, t, p),
        SaturationOptions::default(),
    )
}

/// Dew **temperature** at fixed pressure using a [`PropertyPackageModel`].
///
/// Convenience wrapper over [`dew_temperature_with`]. See it for details.
pub fn dew_temperature(
    components: &[Component],
    z: &[f64],
    pressure: f64,
    package: PropertyPackageModel,
) -> Result<SaturationState, SaturationError> {
    dew_temperature_with(
        components,
        z,
        pressure,
        |x, y, t, p| package.k_values(components, x, y, t, p),
        SaturationOptions::default(),
    )
}

#[cfg(test)]
mod degenerate_branch_tests {
    use super::*;
    use crate::petroleum::crude_distillation::BlackOilCrude;
    use crate::thermo::property_package::PropertyPackageModel;

    /// The op-190j.5 case: a crude pseudo-component slate whose bubble point a
    /// cubic package could not find at all. Above the critical region both
    /// phases return one compressibility root, every K collapses to 1, and the
    /// residual tends to zero without crossing — so the expansion heuristic
    /// chased the asymptote instead of the root.
    #[test]
    fn a_cubic_package_finds_the_crude_slate_bubble_point() {
        let slate = BlackOilCrude::light_sweet()
            .pseudo_components(8)
            .expect("slate");
        let comps: Vec<_> = slate
            .iter()
            .filter(|pc| pc.component.normal_boiling_point < 615.0)
            .map(|pc| pc.component.clone())
            .collect();
        let n = comps.len();
        let z = vec![1.0 / n as f64; n];
        let p = 120_000.0;

        let ideal = bubble_temperature(&comps, &z, p, PropertyPackageModel::Ideal)
            .expect("the ideal package has always managed this");
        for pkg in [
            PropertyPackageModel::PengRobinson,
            PropertyPackageModel::PengRobinson1978,
            PropertyPackageModel::Srk,
        ] {
            let st = bubble_temperature(&comps, &z, p, pkg)
                .unwrap_or_else(|e| panic!("{pkg:?} failed on the crude slate: {e}"));
            // Not a claim that the packages agree — they should not, exactly —
            // only that the cubic answer is the same root the ideal one found
            // and not the degenerate branch a few hundred kelvin above it.
            assert!(
                (st.temperature - ideal.temperature).abs() < 120.0,
                "{pkg:?} converged to {} K, ideal says {} K — that is the \
                 degenerate branch, not the bubble point",
                st.temperature,
                ideal.temperature
            );
            assert!(
                !is_trivial_k(&st.k),
                "{pkg:?} returned the trivial K = 1 solution"
            );
        }
    }

    /// `K ≡ 1` is the *correct* saturation condition for a pure component, so
    /// the trivial-solution test must not fire on one.
    #[test]
    fn a_pure_component_is_not_a_trivial_solution() {
        assert!(!is_trivial_k(&[1.0]));
        assert!(is_trivial_k(&[1.0, 1.0]));
        assert!(!is_trivial_k(&[1.0, 2.0]));
        assert!(!is_trivial_k(&[]));
    }
}

#[cfg(test)]
mod tests {
    //! # V&V — verification of the bubble/dew saturation solvers
    //!
    //! **Methodology (shared).** Every test checks a *defining identity* or an
    //! internal-consistency relation of the saturation problem — **not**
    //! experimental VLE data. The K-model is the ideal/Wilson
    //! [`PropertyPackageModel::Ideal`] package unless stated, for which the
    //! saturation residual is an exactly monotone function of the solved
    //! variable, so the checks are deterministic and need no phase-stability
    //! pre-test. Light-hydrocarbon critical constants come from the
    //! [`reference`](crate::thermo::component::reference) presets (Poling,
    //! Prausnitz & O'Connell, *The Properties of Gases and Liquids*, 5th ed.,
    //! 2001, Appendix A — public literature).
    //!
    //! **Scope (honesty).** Verification ("did we solve the saturation
    //! condition correctly?"), NOT validation against measured bubble/dew data.
    //! Two-phase VLE only; no VLLE/solid; no retrograde-branch guarantees.
    //! Numbers below were measured on 2026-07-24 (this port), release build.

    use super::*;
    use crate::thermo::component::reference;
    use approx::assert_abs_diff_eq;

    /// **Methodology.** Bubble pressure of the ideal/Wilson binary
    /// methane(1)/ethane(2), `z = [0.5, 0.5]`, `T = 200 K`. The returned `P`
    /// must satisfy the defining bubble identity `Σ K_i(T, P) z_i = 1` to the
    /// solver tolerance, and the incipient vapour `y_i = K_i z_i` must sum to 1.
    /// The Wilson K-values give the closed form `P_bub = Σ z_i Pc_i E_i`, so the
    /// answer is exact at the initial guess.
    /// **Results (2026-07-24, release):** `P_bub = 3.080258e6 Pa`; saturation
    /// residual `Σ K z − 1 = 0` (`|·| < 1e-8`); incipient
    /// `y = [0.964711, 0.035289]`, `Σ y = 1.0` to < 1e-12; converged with the
    /// Wilson closed-form seed (no bracketing needed).
    #[test]
    fn bubble_pressure_ideal_binary_satisfies_sum_kz_eq_one() {
        let comps = [reference::methane(), reference::ethane()];
        let z = [0.5, 0.5];
        let t = 200.0;

        let s = bubble_pressure(&comps, &z, t, PropertyPackageModel::Ideal).unwrap();

        // Defining identity Σ K z = 1: residual is (Σ K z − 1).
        assert!(
            s.residual.abs() < 1e-8,
            "bubble residual too large: {}",
            s.residual
        );
        // Independent recomputation of Σ K z at the returned P.
        let k = wilson_k_values(&comps, t, s.pressure);
        let sum_kz: f64 = k.iter().zip(z.iter()).map(|(&ki, &zi)| ki * zi).sum();
        assert_abs_diff_eq!(sum_kz, 1.0, epsilon = 1e-8);
        // Incipient vapour y_i = K_i z_i sums to 1.
        assert_abs_diff_eq!(s.incipient.iter().sum::<f64>(), 1.0, epsilon = 1e-8);
        // Sanity on the pressure magnitude (Wilson closed form).
        assert_abs_diff_eq!(s.pressure, 3.080258e6, epsilon = 1e3);
    }

    /// **Methodology.** Dew pressure of the same ideal/Wilson binary at
    /// `T = 200 K`. The returned `P` must satisfy `Σ z_i / K_i(T, P) = 1`, and
    /// the incipient liquid `x_i = z_i / K_i` must sum to 1.
    /// **Results (2026-07-24, release):** `P_dew = 4.194507e5 Pa`; residual
    /// `Σ z/K − 1 = 2.2e-16` (`|·| < 1e-8`); incipient `x = [0.035289, 0.964711]`,
    /// `Σ x = 1.0` to < 1e-12.
    #[test]
    fn dew_pressure_ideal_binary_satisfies_sum_z_over_k_eq_one() {
        let comps = [reference::methane(), reference::ethane()];
        let z = [0.5, 0.5];
        let t = 200.0;

        let s = dew_pressure(&comps, &z, t, PropertyPackageModel::Ideal).unwrap();

        assert!(
            s.residual.abs() < 1e-8,
            "dew residual too large: {}",
            s.residual
        );
        let k = wilson_k_values(&comps, t, s.pressure);
        let sum_zk: f64 = z.iter().zip(k.iter()).map(|(&zi, &ki)| zi / ki).sum();
        assert_abs_diff_eq!(sum_zk, 1.0, epsilon = 1e-8);
        assert_abs_diff_eq!(s.incipient.iter().sum::<f64>(), 1.0, epsilon = 1e-8);
        assert_abs_diff_eq!(s.pressure, 4.194507e5, epsilon = 1e2);
    }

    /// **Methodology.** For a normal (non-azeotropic) binary at a fixed
    /// temperature the bubble pressure must exceed the dew pressure
    /// (`P_bub > P_dew`) — the two-phase envelope has the bubble curve above the
    /// dew curve. This follows analytically for ideal K from the
    /// arithmetic-mean ≥ harmonic-mean inequality (`Σ z K ≥ 1 / Σ z/K`).
    /// **Results (2026-07-24, release):** `P_bub = 3.080258e6 Pa >
    /// 4.194507e5 Pa = P_dew` (ratio ≈ 7.34).
    #[test]
    fn bubble_pressure_exceeds_dew_pressure() {
        let comps = [reference::methane(), reference::ethane()];
        let z = [0.5, 0.5];
        let t = 200.0;

        let bub = bubble_pressure(&comps, &z, t, PropertyPackageModel::Ideal).unwrap();
        let dew = dew_pressure(&comps, &z, t, PropertyPackageModel::Ideal).unwrap();
        assert!(
            bub.pressure > dew.pressure,
            "expected bubble {} > dew {}",
            bub.pressure,
            dew.pressure
        );
    }

    /// **Methodology.** For a **pure** component the bubble and dew points
    /// coincide and both equal the vapour pressure. With the ideal/Wilson
    /// K-model the "vapour pressure" is the Wilson estimate — the pressure at
    /// which `K = 1`, i.e. `P_sat = Pc exp[5.373(1+ω)(1−Tc/T)]`. Methane at
    /// `T = 150 K` (below `Tc = 190.56 K`), `z = [1.0]`.
    /// **Results (2026-07-24, release):** `P_bub = P_dew = 1.058654e6 Pa`,
    /// agreeing to < 1e-6 Pa and matching the Wilson `K = 1` pressure to
    /// < 1e-6 Pa; incipient composition `[1.0]`.
    #[test]
    fn pure_component_bubble_equals_dew_equals_vapour_pressure() {
        let comps = [reference::methane()];
        let z = [1.0];
        let t = 150.0;

        let bub = bubble_pressure(&comps, &z, t, PropertyPackageModel::Ideal).unwrap();
        let dew = dew_pressure(&comps, &z, t, PropertyPackageModel::Ideal).unwrap();

        // Wilson vapour pressure: the P where K = 1.
        let p_sat = wilson_bubble_pressure_guess(&comps, &z, t); // = Pc*E for a pure feed
        assert_abs_diff_eq!(bub.pressure, dew.pressure, epsilon = 1e-6);
        assert_abs_diff_eq!(bub.pressure, p_sat, epsilon = 1e-6);

        // Cross-check K(T, P_sat) = 1.
        let k = wilson_k_values(&comps, t, bub.pressure);
        assert_abs_diff_eq!(k[0], 1.0, epsilon = 1e-10);
        assert_abs_diff_eq!(bub.incipient[0], 1.0, epsilon = 1e-10);
    }

    /// **Methodology.** Pressure/temperature round-trip consistency: solve the
    /// bubble *pressure* at a fixed `T`, then solve the bubble *temperature* at
    /// that pressure — it must return the original `T`. Exercises the
    /// temperature root-finder (with its Wilson-inverse seed and bracket
    /// expansion) against the pressure solver. Methane/ethane `z = [0.5, 0.5]`,
    /// `T = 200 K`.
    /// **Results (2026-07-24, release):** bubble `P = 3.080258e6 Pa` at
    /// `T = 200 K`; back-solved bubble temperature `T = 200.000000 K`
    /// (round-trip error < 1e-6 K); its residual `Σ K z − 1 < 1e-10`.
    #[test]
    fn bubble_temperature_round_trips_bubble_pressure() {
        let comps = [reference::methane(), reference::ethane()];
        let z = [0.5, 0.5];
        let t = 200.0;

        let bp = bubble_pressure(&comps, &z, t, PropertyPackageModel::Ideal).unwrap();
        let bt = bubble_temperature(&comps, &z, bp.pressure, PropertyPackageModel::Ideal).unwrap();

        assert_abs_diff_eq!(bt.temperature, t, epsilon = 1e-6);
        assert!(
            bt.residual.abs() < 1e-8,
            "bubble-T residual too large: {}",
            bt.residual
        );
    }

    /// **Methodology.** Dew-temperature analogue of the round-trip test: solve
    /// dew *pressure* at fixed `T`, back-solve dew *temperature* at that
    /// pressure. Methane/ethane `z = [0.5, 0.5]`, `T = 200 K`.
    /// **Results (2026-07-24, release):** dew `P = 4.194507e5 Pa` at
    /// `T = 200 K`; back-solved dew temperature `T = 200.000000 K` (error
    /// < 1e-6 K); residual `Σ z/K − 1 < 1e-10`.
    #[test]
    fn dew_temperature_round_trips_dew_pressure() {
        let comps = [reference::methane(), reference::ethane()];
        let z = [0.5, 0.5];
        let t = 200.0;

        let dp = dew_pressure(&comps, &z, t, PropertyPackageModel::Ideal).unwrap();
        let dt = dew_temperature(&comps, &z, dp.pressure, PropertyPackageModel::Ideal).unwrap();

        assert_abs_diff_eq!(dt.temperature, t, epsilon = 1e-6);
        assert!(
            dt.residual.abs() < 1e-8,
            "dew-T residual too large: {}",
            dt.residual
        );
    }

    /// **Methodology.** Generic K-closure path ([`bubble_pressure_with`] /
    /// [`dew_pressure_with`]) with a **Raoult** model `K_i = Psat_i / P`
    /// (composition-independent, supplied as a plain `Fn`, not a package). With
    /// fixed `Psat = [2·10⁶, 5·10⁵] Pa` and `z = [0.5, 0.5]` the bubble/dew
    /// pressures have the closed forms `P_bub = Σ z_i Psat_i = 1.25·10⁶ Pa` and
    /// `P_dew = 1 / Σ z_i/Psat_i = 8.0·10⁵ Pa`. This verifies the generic
    /// ln-φ/K closure entry point independently of [`PropertyPackageModel`].
    /// **Results (2026-07-24, release):** `P_bub = 1.250000e6 Pa`,
    /// `P_dew = 8.000000e5 Pa`, both matching the closed forms to < 1e-6 Pa;
    /// bubble/dew residuals `< 1e-12`; incipient compositions sum to 1 to < 1e-12.
    #[test]
    fn generic_raoult_closure_matches_closed_form() {
        let comps = [reference::methane(), reference::ethane()];
        let z = [0.5, 0.5];
        let psat = [2.0e6, 5.0e5];
        // Raoult K-values: composition-independent, K_i = Psat_i / P.
        let kf = |_x: &[f64], _y: &[f64], _t: f64, p: f64| -> Vec<f64> {
            psat.iter().map(|&ps| ps / p).collect()
        };

        let opts = SaturationOptions::default();
        let bub = bubble_pressure_with(&comps, &z, 200.0, kf, opts).unwrap();
        let dew = dew_pressure_with(&comps, &z, 200.0, kf, opts).unwrap();

        let p_bub_closed = z[0] * psat[0] + z[1] * psat[1]; // 1.25e6
        let p_dew_closed = 1.0 / (z[0] / psat[0] + z[1] / psat[1]); // 8.0e5
        assert_abs_diff_eq!(bub.pressure, p_bub_closed, epsilon = 1e-6);
        assert_abs_diff_eq!(dew.pressure, p_dew_closed, epsilon = 1e-6);
        assert!(bub.residual.abs() < 1e-12 && dew.residual.abs() < 1e-12);
        assert_abs_diff_eq!(bub.incipient.iter().sum::<f64>(), 1.0, epsilon = 1e-12);
        assert_abs_diff_eq!(dew.incipient.iter().sum::<f64>(), 1.0, epsilon = 1e-12);
        assert!(bub.pressure > dew.pressure);
    }

    /// **Methodology.** Input-validation guards. **Results (2026-07-24):**
    /// empty feed → `Empty`; mismatched `components`/`z` → `LengthMismatch`;
    /// non-positive temperature → `NonPositive`.
    #[test]
    fn input_validation_errors() {
        let comps = [reference::methane(), reference::ethane()];
        assert_eq!(
            bubble_pressure(&[], &[], 200.0, PropertyPackageModel::Ideal).unwrap_err(),
            SaturationError::Empty
        );
        assert!(matches!(
            bubble_pressure(&comps, &[1.0], 200.0, PropertyPackageModel::Ideal).unwrap_err(),
            SaturationError::LengthMismatch { .. }
        ));
        assert!(matches!(
            bubble_pressure(&comps, &[0.5, 0.5], -5.0, PropertyPackageModel::Ideal).unwrap_err(),
            SaturationError::NonPositive { .. }
        ));
    }
}
