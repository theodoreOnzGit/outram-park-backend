// SPDX-License-Identifier: GPL-3.0-only
// Copyright (C) 2026 OUTRAM PARK contributors
//
// Derived from code_aster (https://gitlab.com/codeaster/src)
//   Copyright (C) 1991 - 2026 - EDF R&D
//   Licence: GPL-3.0-or-later
//   Upstream commit: b504ea08c2f49575e04644cee2e39a63ea45c16e
//   Corresponds to the `algo_inte` local-integration algorithms declared across
//   `code_aster/Behaviours/*.py`: NEWTON, NEWTON_1D, NEWTON_PERT, SECANTE,
//   BRENT, RUNGE_KUTTA, ANALYTIQUE.
//
// This file is part of OUTRAM PARK. See `src/lib.rs` for the full licence
// notice.

//! Local integration algorithms shared by every constitutive law.
//!
//! # What a "local solve" is
//!
//! A rate-dependent constitutive law cannot be evaluated in closed form. Given a
//! strain increment, the stress at the end of the step depends on the inelastic
//! increment, which itself depends on the end-of-step stress. That circularity
//! is resolved per integration point, per timestep, by a **scalar root find** —
//! typically on the equivalent plastic or creep increment.
//!
//! Every law in code_aster's catalogue declares which algorithm it wants in its
//! `algo_inte` field, so this handful of solvers is shared machinery: porting it
//! once unblocks all 151 mechanical laws, which is why it precedes them.
//!
//! # Coverage of upstream's `algo_inte`
//!
//! Counting across the 229 catalogue declarations:
//!
//! | Upstream `algo_inte` | Count | Here |
//! |---|---|---|
//! | `ANALYTIQUE` | 68 | [`ScalarAlgorithm::Analytic`] — no local solve; the law inverts in closed form |
//! | `SANS_OBJET` | 52 | no local solve needed at all |
//! | `SPECIFIQUE` | 32 | law-specific; each law brings its own |
//! | `NEWTON` | 23 | [`newton_safeguarded`] |
//! | `NEWTON_PERT` | 16 | [`newton_perturbed`] |
//! | `SECANTE` | 12 | [`secant`] |
//! | `BRENT` | 10 | [`brent`] |
//! | `NEWTON_1D` | 8 | [`newton_safeguarded`] — the scalar case is the same solve |
//! | `RUNGE_KUTTA` | few | **not here**: reuses `outram_foam_basic_lib::ode` |
//!
//! `RUNGE_KUTTA` is deliberately absent. `outram-foam-basic-lib` already carries
//! `Euler`, `Rkf45` (adaptive Runge-Kutta-Fehlberg) and `Rosenbrock23` (stiff),
//! driven by an `OdeSystem` trait. A law integrating its internal variables as
//! an ODE system should use those; re-porting a Runge-Kutta here would duplicate
//! a Layer-1 primitive at Layer 5, the same mistake as hand-rolling an
//! eigensolver rather than porting OpenFOAM's into the primitive crate.
//!
//! # Why safeguarding is not optional
//!
//! Plain Newton is the obvious choice and the wrong default here. This crate has
//! already met the failure mode once: upstream's `LimbackCreepModel` omits the
//! primary-creep derivative from its Jacobian, so the Newton direction is
//! systematically wrong and the iteration oscillates without converging. The
//! rheology port had to wrap it in a bracketed safeguard to get an answer at
//! all.
//!
//! That is not an isolated defect — an inconsistent or deliberately simplified
//! Jacobian is common in legacy constitutive code, because the exact tangent is
//! tedious to derive and the author only needed the residual to vanish. So
//! [`newton_safeguarded`] keeps a bracket and falls back to bisection whenever
//! the Newton step would leave it or fails to reduce the interval. That costs
//! nothing when the Jacobian is good and rescues the iteration when it is not.
//!
//! # Convergence, and what these are checked against
//!
//! The tests verify the *published orders of convergence* — quadratic for
//! Newton, the golden ratio for the secant method — on a classical benchmark
//! whose root is known analytically. Those orders are theorems, not fitted
//! constants, which makes them a genuine external reference rather than a
//! self-consistency check.

use crate::error::{OffbeatError, Result};

/// Convergence control for a local solve.
///
/// # Defaults
///
/// 100 iterations to a residual tolerance of 1e-10 and a step tolerance of
/// 1e-14. The residual tolerance is the one that usually binds; the step
/// tolerance stops the iteration when the bracket or increment can no longer
/// be refined in floating point, which is the honest termination criterion when
/// the residual scale is large.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct SolverControl {
    /// Maximum iterations before reporting non-convergence.
    pub max_iter: usize,
    /// Absolute tolerance on the residual `|f(x)|`.
    pub residual_tol: f64,
    /// Absolute tolerance on the step `|x_{n+1} - x_n|`.
    pub step_tol: f64,
}

impl Default for SolverControl {
    fn default() -> Self {
        Self {
            max_iter: 100,
            residual_tol: 1.0e-10,
            step_tol: 1.0e-14,
        }
    }
}

/// Outcome of a local solve.
///
/// Returned rather than logged so a constitutive law can decide what to do —
/// usually to surface
/// [`ConstitutiveNotConverged`](crate::error::OffbeatError::ConstitutiveNotConverged)
/// with the cell index it knows and this solver does not.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct LocalSolution {
    /// The root.
    pub root: f64,
    /// Residual `f(root)` actually achieved.
    pub residual: f64,
    /// Iterations performed.
    pub iterations: usize,
    /// How many of those fell back to bisection because the Newton step was
    /// rejected.
    ///
    /// Zero means the Jacobian was good throughout. A large count on a law that
    /// claims an exact tangent is evidence the tangent is wrong — worth
    /// surfacing rather than hiding, since it is otherwise invisible.
    pub bisection_steps: usize,
}

/// Which local algorithm a law asks for.
///
/// Mirrors upstream's `algo_inte` field. Enum dispatch, not trait objects, per
/// the workspace rule — the set is closed and known at compile time.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ScalarAlgorithm {
    /// `ANALYTIQUE` — the law inverts in closed form and needs no iteration.
    ///
    /// Present so a law's declared algorithm can be represented faithfully;
    /// attempting to *run* it returns
    /// [`NotImplemented`](crate::error::OffbeatError::NotImplemented) rather
    /// than silently iterating, because a closed-form law that reaches an
    /// iterative solver has been mis-wired.
    Analytic,
    /// `NEWTON` / `NEWTON_1D` — Newton with a bisection safeguard.
    Newton,
    /// `NEWTON_PERT` — Newton with a numerically perturbed derivative.
    NewtonPerturbed,
    /// `SECANTE` — the secant method.
    Secant,
    /// `BRENT` — Brent's method: bisection, secant and inverse quadratic
    /// interpolation combined.
    Brent,
}

impl ScalarAlgorithm {
    /// The upstream `algo_inte` token this corresponds to.
    #[must_use]
    pub const fn aster_name(self) -> &'static str {
        match self {
            Self::Analytic => "ANALYTIQUE",
            Self::Newton => "NEWTON",
            Self::NewtonPerturbed => "NEWTON_PERT",
            Self::Secant => "SECANTE",
            Self::Brent => "BRENT",
        }
    }

    /// Whether this algorithm needs an analytic derivative from the caller.
    #[must_use]
    pub const fn needs_derivative(self) -> bool {
        matches!(self, Self::Newton)
    }
}

/// Newton's method with a bisection safeguard on a bracket.
///
/// # Arguments
///
/// - `f` — the residual. A root is a value where it vanishes.
/// - `df` — its derivative. If this is inconsistent with `f` the safeguard is
///   what saves the solve; see the module documentation.
/// - `bracket` — `(a, b)` with `f(a)` and `f(b)` of opposite sign. Checked.
/// - `control` — iteration and tolerance limits.
///
/// # Method
///
/// Each iteration proposes a Newton step. It is accepted only if it lands
/// inside the current bracket and is actually reducing the interval; otherwise
/// the step is replaced by a bisection of the bracket. The bracket is then
/// tightened using the sign of the new residual, so it always contains a root.
/// This is the classical safeguarded-Newton arrangement, and it converges
/// quadratically when the derivative is good while never being worse than
/// bisection when it is not.
///
/// # Errors
///
/// [`OffbeatError::Unphysical`] if the supplied bracket does not straddle a
/// root — that is a caller error, and iterating anyway would return a
/// confident wrong answer. [`OffbeatError::ConstitutiveNotConverged`] if the
/// iteration budget is exhausted.
pub fn newton_safeguarded<F, D>(
    f: F,
    df: D,
    bracket: (f64, f64),
    control: &SolverControl,
) -> Result<LocalSolution>
where
    F: Fn(f64) -> f64,
    D: Fn(f64) -> f64,
{
    let (mut lo, mut hi) = (bracket.0, bracket.1);
    let (mut f_lo, mut f_hi) = (f(lo), f(hi));

    if let Some(sol) = exact_endpoint(lo, f_lo, hi, f_hi) {
        return Ok(sol);
    }
    check_bracket(f_lo, f_hi)?;

    // Orient so that f(lo) < 0 < f(hi); the step-acceptance test below reads
    // more simply in that orientation.
    if f_lo > 0.0 {
        core::mem::swap(&mut lo, &mut hi);
        core::mem::swap(&mut f_lo, &mut f_hi);
    }

    let mut x = 0.5 * (lo + hi);
    let mut bisection_steps = 0;

    for iteration in 1..=control.max_iter {
        let fx = f(x);
        if fx.abs() <= control.residual_tol {
            return Ok(LocalSolution {
                root: x,
                residual: fx,
                iterations: iteration,
                bisection_steps,
            });
        }

        // Re-bracket on the sign of the residual.
        if fx < 0.0 {
            lo = x;
        } else {
            hi = x;
        }

        let dfx = df(x);
        let newton = if dfx.abs() > f64::MIN_POSITIVE {
            x - fx / dfx
        } else {
            f64::NAN
        };

        let inside = newton.is_finite() && (newton - lo) * (newton - hi) < 0.0;
        let next = if inside {
            newton
        } else {
            bisection_steps += 1;
            0.5 * (lo + hi)
        };

        if (next - x).abs() <= control.step_tol {
            let fnext = f(next);
            return Ok(LocalSolution {
                root: next,
                residual: fnext,
                iterations: iteration,
                bisection_steps,
            });
        }
        x = next;
    }

    Err(OffbeatError::ConstitutiveNotConverged {
        cell: usize::MAX,
        residual: f(x).abs(),
        iterations: control.max_iter,
    })
}

/// Newton with a numerically perturbed derivative — upstream's `NEWTON_PERT`.
///
/// For laws whose analytic tangent is unavailable or known to be inconsistent.
/// The derivative is approximated by a central difference with step
/// `perturbation * max(|x|, 1)`, so the step scales with the magnitude of the
/// unknown rather than being absolute.
///
/// # Choosing `perturbation`
///
/// Central differencing has truncation error `O(h²)` and round-off error
/// `O(eps/h)`, which balance near `h = eps^(1/3)`, about 6e-6 for `f64`. That
/// is the default in [`perturbed_default`]. A much smaller step is not more
/// accurate — it is noisier.
///
/// # Errors
///
/// As [`newton_safeguarded`].
pub fn newton_perturbed<F>(
    f: F,
    bracket: (f64, f64),
    perturbation: f64,
    control: &SolverControl,
) -> Result<LocalSolution>
where
    F: Fn(f64) -> f64,
{
    let fr = &f;
    let df = |x: f64| {
        let h = perturbation * x.abs().max(1.0);
        (fr(x + h) - fr(x - h)) / (2.0 * h)
    };
    newton_safeguarded(&f, df, bracket, control)
}

/// The perturbation step that balances truncation against round-off for a
/// central difference in `f64`: `eps^(1/3)`, about 6.06e-6.
#[must_use]
pub fn perturbed_default() -> f64 {
    f64::EPSILON.cbrt()
}

/// The secant method — upstream's `SECANTE`.
///
/// Replaces Newton's derivative with the slope through the last two iterates,
/// so it needs no derivative at all. Its order of convergence is the golden
/// ratio, about 1.618 — superlinear but slower than Newton, which is the price
/// of not knowing the tangent.
///
/// Unlike [`newton_safeguarded`] this keeps no bracket, so it can diverge on a
/// badly-chosen pair. It is offered because upstream declares it for 12 laws;
/// prefer [`brent`] when robustness matters more than simplicity.
///
/// # Errors
///
/// [`OffbeatError::ConstitutiveNotConverged`] if the budget is exhausted or the
/// secant slope collapses to zero.
pub fn secant<F>(f: F, x0: f64, x1: f64, control: &SolverControl) -> Result<LocalSolution>
where
    F: Fn(f64) -> f64,
{
    let (mut a, mut b) = (x0, x1);
    let (mut fa, mut fb) = (f(a), f(b));

    for iteration in 1..=control.max_iter {
        if fb.abs() <= control.residual_tol {
            return Ok(LocalSolution {
                root: b,
                residual: fb,
                iterations: iteration,
                bisection_steps: 0,
            });
        }

        let denom = fb - fa;
        if denom.abs() < f64::MIN_POSITIVE {
            return Err(OffbeatError::ConstitutiveNotConverged {
                cell: usize::MAX,
                residual: fb.abs(),
                iterations: iteration,
            });
        }

        let next = b - fb * (b - a) / denom;
        a = b;
        fa = fb;
        b = next;
        fb = f(b);

        if (b - a).abs() <= control.step_tol {
            return Ok(LocalSolution {
                root: b,
                residual: fb,
                iterations: iteration,
                bisection_steps: 0,
            });
        }
    }

    Err(OffbeatError::ConstitutiveNotConverged {
        cell: usize::MAX,
        residual: fb.abs(),
        iterations: control.max_iter,
    })
}

/// Brent's method — upstream's `BRENT`.
///
/// Combines bisection, the secant method and inverse quadratic interpolation:
/// it takes the fast interpolating step when that step is behaving, and falls
/// back to bisection when it is not. The result is superlinear convergence with
/// bisection's guarantee — it cannot fail to converge on a valid bracket.
///
/// This is the right default for a constitutive local solve whose residual is
/// awkward, and the reason upstream offers it alongside Newton.
///
/// # Errors
///
/// [`OffbeatError::Unphysical`] if the bracket does not straddle a root;
/// [`OffbeatError::ConstitutiveNotConverged`] if the budget is exhausted.
pub fn brent<F>(f: F, bracket: (f64, f64), control: &SolverControl) -> Result<LocalSolution>
where
    F: Fn(f64) -> f64,
{
    let (mut a, mut b) = (bracket.0, bracket.1);
    let (mut fa, mut fb) = (f(a), f(b));

    if let Some(sol) = exact_endpoint(a, fa, b, fb) {
        return Ok(sol);
    }
    check_bracket(fa, fb)?;

    // `b` is kept as the best estimate, `a` as the contrapoint.
    if fa.abs() < fb.abs() {
        core::mem::swap(&mut a, &mut b);
        core::mem::swap(&mut fa, &mut fb);
    }

    let mut c = a;
    let mut fc = fa;
    let mut d = b - a;
    let mut e = d;
    let mut bisection_steps = 0;

    for iteration in 1..=control.max_iter {
        if fb.abs() <= control.residual_tol {
            return Ok(LocalSolution {
                root: b,
                residual: fb,
                iterations: iteration,
                bisection_steps,
            });
        }

        if (fb > 0.0) == (fc > 0.0) {
            c = a;
            fc = fa;
            d = b - a;
            e = d;
        }
        if fc.abs() < fb.abs() {
            a = b;
            b = c;
            c = a;
            fa = fb;
            fb = fc;
            fc = fa;
        }

        let tol = 2.0 * f64::EPSILON * b.abs() + 0.5 * control.step_tol;
        let m = 0.5 * (c - b);
        if m.abs() <= tol {
            return Ok(LocalSolution {
                root: b,
                residual: fb,
                iterations: iteration,
                bisection_steps,
            });
        }

        if e.abs() < tol || fa.abs() <= fb.abs() {
            // Interpolation is not making progress; bisect.
            d = m;
            e = m;
            bisection_steps += 1;
        } else {
            let s = fb / fa;
            let (mut p, mut q);
            if (a - c).abs() < f64::EPSILON {
                // Two points only: secant step.
                p = 2.0 * m * s;
                q = 1.0 - s;
            } else {
                // Three points: inverse quadratic interpolation.
                let qq = fa / fc;
                let r = fb / fc;
                p = s * (2.0 * m * qq * (qq - r) - (b - a) * (r - 1.0));
                q = (qq - 1.0) * (r - 1.0) * (s - 1.0);
            }
            if p > 0.0 {
                q = -q;
            } else {
                p = -p;
            }
            if 2.0 * p < (3.0 * m * q - (tol * q).abs()).min((e * q).abs()) {
                e = d;
                d = p / q;
            } else {
                d = m;
                e = m;
                bisection_steps += 1;
            }
        }

        a = b;
        fa = fb;
        b += if d.abs() > tol {
            d
        } else if m > 0.0 {
            tol
        } else {
            -tol
        };
        fb = f(b);
    }

    Err(OffbeatError::ConstitutiveNotConverged {
        cell: usize::MAX,
        residual: fb.abs(),
        iterations: control.max_iter,
    })
}

/// A bracket endpoint that is already a root, if either is.
fn exact_endpoint(a: f64, fa: f64, b: f64, fb: f64) -> Option<LocalSolution> {
    if fa == 0.0 {
        return Some(LocalSolution {
            root: a,
            residual: 0.0,
            iterations: 0,
            bisection_steps: 0,
        });
    }
    if fb == 0.0 {
        return Some(LocalSolution {
            root: b,
            residual: 0.0,
            iterations: 0,
            bisection_steps: 0,
        });
    }
    None
}

/// Reject a bracket whose endpoints share a sign.
fn check_bracket(fa: f64, fb: f64) -> Result<()> {
    if (fa > 0.0) == (fb > 0.0) {
        return Err(OffbeatError::Unphysical {
            quantity: "root-finding bracket",
            value: fa * fb,
            unit: "-",
            reason: "the residual has the same sign at both bracket endpoints, so \
                     no root is guaranteed inside; iterating would return a \
                     confident wrong answer",
        });
    }
    Ok(())
}

#[cfg(test)]
mod tests;
