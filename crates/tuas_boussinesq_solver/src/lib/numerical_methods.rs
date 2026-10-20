//! Root finding, quadrature and table interpolation for TUAS, all on PETIR.
//!
//! Added 2026-10-20 for gh:#989 (and the `roots` half of gh:#991), when TUAS
//! moved off the `peroxide` and `roots` crates onto `petir`, whose routines
//! are ports of GSL verified bit-for-bit against compiled GSL (see
//! `crates/petir/docs/verification-summary.md`). Everything here is an
//! **adapter**: the numerics are PETIR's; what this module adds is the
//! *convergence semantics* the replaced calls had, so the swap changes the
//! library underneath without silently changing what "converged" means.
//!
//! | replaced call | adapter | what is kept identical | where it differs |
//! |---|---|---|---|
//! | `peroxide` `bisection!(f, (a, b), n, tol)` / `BisectionMethod { max_iter, tol }` | [`bisection_residual`] | the probe sequence, the residual test `\|f(c)\| < tol` at the probed midpoint `c`, the returned point, the endpoint checks and the cap | a non-finite `f` is an error (GSL's `SAFE_FUNC_CALL`); peroxide carried on |
//! | `roots` `find_root_brent` / `find_root_regula_falsi` / `find_root_inverse_quadratic` with `SimpleConvergency { eps, max_iter }` | [`bracketed_root`] | bracket (either order), the two-part test (`\|f\| < eps` at the iterate, or bracket width `< eps`), the cap, the no-bracket error | the ALGORITHM: GSL's Brent / false position, not `roots`' own. See the function docs |
//! | `peroxide` `integrate(f, (a, b), Integral::GaussLegendre(20))` | [`gauss_legendre_20`] | the rule and its arithmetic (PETIR's tables were ported from peroxide 0.41.2; order 20 is unchanged) | none measured |
//! | `peroxide` `CubicSpline::from_nodes` / `.eval` / `.integrate` | [`NaturalCubicSpline`] | the interpolant (natural cubic spline through the same knots) | out of the table it is an error, where peroxide extrapolated; the second-derivative solve is GSL's tridiagonal one, not peroxide's dense `inv()`, so values move at round-off |

use std::cell::Cell;

use petir::interp::{Interp, InterpMethod};
use petir::roots::{test_interval, BracketingMethod, BracketingSolver};
use petir::PetirError;

use crate::tuas_lib_error::TuasLibError;

fn petir_error(context: &str, err: PetirError) -> TuasLibError {
    TuasLibError::GenericStringError(format!("{context}: petir error {err:?}"))
}

/// Records the last two points at which a wrapped function was evaluated, so
/// the adapters can apply a residual test to a point PETIR's solver probed
/// without evaluating the function a second time.
struct ProbeLog {
    previous: Cell<(f64, f64)>,
    last: Cell<(f64, f64)>,
}

impl ProbeLog {
    fn new() -> Self {
        Self {
            previous: Cell::new((f64::NAN, f64::NAN)),
            last: Cell::new((f64::NAN, f64::NAN)),
        }
    }
    fn record(&self, x: f64, y: f64) {
        self.previous.set(self.last.get());
        self.last.set((x, y));
    }
}

/// Bisection with `peroxide` 0.41.2's convergence semantics, run on PETIR's
/// GSL bisection (`petir::roots::BracketingSolver`, `BracketingMethod::Bisection`).
///
/// # What peroxide did, read from its source
///
/// `peroxide-0.41.2/src/numerical/root.rs`, `impl RootFinder for
/// BisectionMethod` (lines 547-589), which the `bisection!` macro (lines
/// 233-259) wraps:
///
/// 1. evaluate `f(a)`, `f(b)`; return `a` if `|f(a)| < tol`, else `b` if
///    `|f(b)| < tol`; error if `f(a) * f(b) > 0`;
/// 2. up to `max_iter` times: `c = (a + b) / 2`; return `c` if
///    `|f(c)| < tol` (an absolute RESIDUAL test, in the function's units);
///    otherwise keep the half whose ends differ in sign;
/// 3. error (`NotConverge`) after `max_iter` halvings.
///
/// GSL's bisection probes exactly the same midpoints `(x_lower + x_upper) / 2`
/// and keeps the same half, so the probe sequence is identical. What GSL does
/// NOT have is the residual test: its `root()` is the midpoint of the *new*
/// bracket, and its stock convergence test is the bracket width. So this
/// adapter logs each probe and applies peroxide's residual test to it,
/// returning the probed point — **bit-identical to peroxide** on every case
/// pinned in this module's tests, with one evaluation per iteration as before.
///
/// # Where it differs from peroxide
///
/// - a non-finite `f` at an endpoint or a probe is an error here
///   (`petir::PetirError::Domain`, GSL's `SAFE_FUNC_CALL`); peroxide carried on;
/// - "no root" is `f(a)` and `f(b)` of strictly the same sign (GSL), where
///   peroxide tested the product `f(a) * f(b) > 0`. They differ only when
///   that product underflows to zero;
/// - `a > b` is an error (GSL); every TUAS call passes `a < b`.
///
/// # Errors
///
/// [`TuasLibError::GenericStringError`] naming `context` when the bracket
/// does not straddle a root, `f` is not finite, or `max_iter` halvings do not
/// meet `tol` — where peroxide returned an error that every TUAS call site
/// `unwrap()`ed into a panic.
pub(crate) fn bisection_residual<F: Fn(f64) -> f64>(
    f: F,
    (a, b): (f64, f64),
    max_iter: usize,
    tol: f64,
    context: &str,
) -> Result<f64, TuasLibError> {
    let log = ProbeLog::new();
    let g = |x: f64| {
        let y = f(x);
        log.record(x, y);
        y
    };
    // BracketingSolver::new evaluates f(a) then f(b), and nothing else.
    let solver = BracketingSolver::new(BracketingMethod::Bisection, &g, a, b);
    let (xa, fa) = log.previous.get();
    let (xb, fb) = log.last.get();
    if xa == a && fa.abs() < tol {
        return Ok(a);
    }
    if xb == b && fb.abs() < tol {
        return Ok(b);
    }
    let mut solver = solver.map_err(|e| petir_error(context, e))?;
    for _ in 0..max_iter {
        solver.iterate().map_err(|e| petir_error(context, e))?;
        let (x, y) = log.last.get();
        if y.abs() < tol {
            return Ok(x);
        }
    }
    Err(TuasLibError::GenericStringError(format!(
        "{context}: bisection did not reach |f| < {tol} in {max_iter} iterations \
         (bracket [{}, {}])",
        solver.x_lower(),
        solver.x_upper()
    )))
}

/// A bracketed root with the `roots` 0.0.8 crate's `SimpleConvergency`
/// semantics, run on a PETIR (GSL) bracketing solver.
///
/// # What `roots` did, read from its source
///
/// `roots-0.0.8/src/numerical/{brent,regula_falsi,inverse_quadratic,simple_convergency}.rs`:
/// the bracket may be given in either order; `SimpleConvergency { eps,
/// max_iter }` declares convergence when `|f(x)| < eps` at the current
/// iterate (`is_root_found`) **or** when the bracket ends are within `eps`
/// of each other (`is_converged`), one `eps` serving both, absolute; a
/// non-straddling bracket is `NoBracketing`; reaching `max_iter` iterations
/// is `NoConvergency`.
///
/// This keeps all of that: the bracket is sorted, the endpoints are tested
/// for `|f| < eps` first, and after every GSL iteration the iterate is
/// tested for `|f| < eps` (when the solver evaluated `f` there, which GSL's
/// Brent and false position do on every iteration except the degenerate
/// ones) and the bracket for `width < eps`
/// (`petir::roots::test_interval(lo, hi, eps, 0.0)`).
///
/// # Where it differs from `roots`
///
/// - **The algorithm is GSL's, not `roots`'.** `roots`' Brent is a textbook
///   variant (`brent.rs`, inverse quadratic with the five bisection
///   conditions); GSL's `brent.c` differs in its acceptance tests and its
///   internal tolerance. `roots`' regula falsi is the Illinois variant
///   (halving the retained end's `f` after two same-side steps); GSL's false
///   position instead bisects when the secant step stalls. `roots`'
///   `find_root_inverse_quadratic` has no GSL counterpart; GSL's Brent, an
///   inverse-quadratic method with a bisection safeguard, replaces it. So
///   iterates differ, and converged roots agree to within `eps`, not bit for
///   bit.
/// - `roots` tests convergence *before* each step and gives up after the
///   `max_iter`-th step without testing it; this tests after each step, so
///   it may accept on the `max_iter`-th step where `roots` erred.
/// - `roots`' width test returned its previous iterate `c`; this returns the
///   solver's current best estimate.
/// - A non-finite `f` is an error here; `roots` carried on with the NaN.
///
/// # Errors
///
/// [`TuasLibError::GenericStringError`] naming `context` for no bracket, a
/// non-finite `f`, or no convergence in `max_iter` iterations. Callers that
/// used `roots`' error to fall back to a wider bracket still can.
pub(crate) fn bracketed_root<F: Fn(f64) -> f64>(
    method: BracketingMethod,
    f: F,
    a: f64,
    b: f64,
    eps: f64,
    max_iter: usize,
    context: &str,
) -> Result<f64, TuasLibError> {
    let (lo, hi) = if a <= b { (a, b) } else { (b, a) };
    let log = ProbeLog::new();
    let g = |x: f64| {
        let y = f(x);
        log.record(x, y);
        y
    };
    let solver = BracketingSolver::new(method, &g, lo, hi);
    let (x_lo, f_lo) = log.previous.get();
    let (x_hi, f_hi) = log.last.get();
    if x_lo == lo && f_lo.abs() < eps {
        return Ok(lo);
    }
    if x_hi == hi && f_hi.abs() < eps {
        return Ok(hi);
    }
    let mut solver = solver.map_err(|e| petir_error(context, e))?;
    for _ in 0..max_iter {
        solver.iterate().map_err(|e| petir_error(context, e))?;
        let (x, y) = log.last.get();
        if x == solver.root() && y.abs() < eps {
            return Ok(x);
        }
        if test_interval(solver.x_lower(), solver.x_upper(), eps, 0.0)
            .map_err(|e| petir_error(context, e))?
            .is_converged()
        {
            return Ok(solver.root());
        }
    }
    Err(TuasLibError::GenericStringError(format!(
        "{context}: {method:?} did not converge to eps = {eps} in {max_iter} iterations \
         (bracket [{}, {}])",
        solver.x_lower(),
        solver.x_upper()
    )))
}

/// The `roots` crate's `SimpleConvergency`, kept field for field so the
/// flow-network solvers that build and re-tune one keep their shape:
/// `eps` serves as both the residual and the bracket-width tolerance
/// (absolute), `max_iter` caps the iterations. See [`bracketed_root`].
#[derive(Debug, Clone, Copy)]
pub(crate) struct SimpleConvergency {
    /// Absolute tolerance on `|f|` and on the bracket width.
    pub eps: f64,
    /// Iteration cap.
    pub max_iter: usize,
}

/// Brent on PETIR (GSL `brent.c`) with `SimpleConvergency` semantics.
/// Replaces `roots::find_root_brent` and, having no GSL counterpart,
/// `roots::find_root_inverse_quadratic`; see [`bracketed_root`] for every
/// difference. The bracket may be given in either order.
pub(crate) fn brent_root<F: Fn(f64) -> f64>(
    a: f64,
    b: f64,
    f: F,
    convergency: &SimpleConvergency,
) -> Result<f64, TuasLibError> {
    bracketed_root(
        BracketingMethod::Brent,
        f,
        a,
        b,
        convergency.eps,
        convergency.max_iter,
        "Brent (PETIR)",
    )
}

/// False position on PETIR (GSL `falsepos.c`) with `SimpleConvergency`
/// semantics. Replaces `roots::find_root_regula_falsi`, which was the
/// Illinois variant; GSL's safeguards a stalled secant step with a bisection
/// instead. See [`bracketed_root`].
pub(crate) fn false_position_root<F: Fn(f64) -> f64>(
    a: f64,
    b: f64,
    f: F,
    convergency: &SimpleConvergency,
) -> Result<f64, TuasLibError> {
    bracketed_root(
        BracketingMethod::FalsePosition,
        f,
        a,
        b,
        convergency.eps,
        convergency.max_iter,
        "false position (PETIR)",
    )
}

/// 20-point Gauss-Legendre quadrature of `f` over `[a, b]`, PETIR's
/// `integration::gauss_legendre::gauss_legendre` (ported from peroxide
/// 0.41.2, whose `Integral::GaussLegendre(20)` TUAS used for the enthalpy of
/// custom materials since v0.1.1; see `docs/notes.md`).
///
/// Why this route and not `qag`/`qags`: the peroxide call was a FIXED,
/// non-adaptive 20-point rule, chosen in v0.1.1 because the adaptive
/// `G20K41` hung on an unreachable absolute tolerance and `G20K41R` cost
/// three orders of magnitude in run time. PETIR's port of the same rule is
/// the like-for-like replacement: same nodes and weights at order 20 (the
/// six corrected table values are at orders 11 and 12), same affine map and
/// summation order. A reversed interval returns the negated integral, as
/// peroxide's did.
pub(crate) fn gauss_legendre_20<F: Fn(f64) -> f64>(
    f: F,
    a: f64,
    b: f64,
    context: &str,
) -> Result<f64, TuasLibError> {
    petir::integration::gauss_legendre::gauss_legendre(f, a, b, 20)
        .map_err(|e| petir_error(context, e))
}

/// A natural cubic spline over a property table, on PETIR's
/// `petir::interp::Interp` (`InterpMethod::CubicSpline`, GSL's
/// `gsl_interp_cspline`), replacing `peroxide`'s `CubicSpline`.
///
/// # Out of the table is an error, never a guess
///
/// peroxide evaluated outside the knots with the end polynomial; PETIR
/// refuses with `PetirError::Domain`, and so does this: [`Self::eval`] and
/// [`Self::integral`] return [`TuasLibError::CorrelationRangeError`] naming
/// the table, the value and the table's range. Nothing is clamped.
pub(crate) struct NaturalCubicSpline {
    interp: Interp,
    name: &'static str,
    units: &'static str,
}

impl NaturalCubicSpline {
    /// Build over strictly increasing `x`. `name` and `units` (of `x`)
    /// label the out-of-range error.
    pub(crate) fn new(
        x: &[f64],
        y: &[f64],
        name: &'static str,
        units: &'static str,
    ) -> Result<Self, TuasLibError> {
        let interp =
            Interp::new(InterpMethod::CubicSpline, x, y).map_err(|e| petir_error(name, e))?;
        Ok(Self {
            interp,
            name,
            units,
        })
    }

    fn range_error(&self, value: f64) -> TuasLibError {
        TuasLibError::CorrelationRangeError {
            parameter: format!("{} (natural cubic spline table)", self.name),
            value,
            lower_bound: self.interp.x_min(),
            upper_bound: self.interp.x_max(),
            units: self.units.to_string(),
        }
    }

    fn map(&self, value: f64, err: PetirError) -> TuasLibError {
        match err {
            PetirError::Domain => self.range_error(value),
            other => petir_error(self.name, other),
        }
    }

    /// The spline at `x`, or a range error outside the table.
    pub(crate) fn eval(&self, x: f64) -> Result<f64, TuasLibError> {
        self.interp.eval(x).map_err(|e| self.map(x, e))
    }

    /// The signed integral of the spline from `a` to `b`
    /// (`petir::interp::Interp::eval_integ`, GSL's `gsl_interp_eval_integ`).
    ///
    /// GSL refuses `a > b`; the integral of the same interpolant with the
    /// limits reversed is its negative, so for `a > b` this returns
    /// `-eval_integ(b, a)`. That is the identity, not an extrapolation: both
    /// limits must still lie inside the table.
    ///
    /// **Correction to peroxide, found 2026-10-20.** peroxide 0.41.2's
    /// `CubicSpline::integrate` (`src/numerical/spline.rs` lines 671-690)
    /// handles `a > b` only when both limits fall in one segment: it skips
    /// every segment whose end lies below `a` and stops at the first whose
    /// start lies above `b`, so for `b` below the segment containing `a` it
    /// sums nothing and returns 0. Copper's enthalpy (reference 273.15 K,
    /// table from 200 K) was therefore exactly 0 J/kg for every T in
    /// [200, 250) K under peroxide.
    pub(crate) fn integral(&self, a: f64, b: f64) -> Result<f64, TuasLibError> {
        let (lo, hi, sign) = if a <= b { (a, b, 1.0) } else { (b, a, -1.0) };
        let value = self.interp.eval_integ(lo, hi).map_err(|e| {
            let outside = if self.interp.eval(lo).is_err() {
                lo
            } else {
                hi
            };
            self.map(outside, e)
        })?;
        Ok(sign * value)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// peroxide 0.41.2's `bisection!` printed these, bit for bit, before
    /// peroxide was removed (2026-10-20, run against the crate as then
    /// vendored in `~/.cargo/registry`). `bisection_residual` must return
    /// the same `f64` — that is the claim "identical probe sequence and
    /// test" made checkable.
    #[test]
    fn bisection_residual_reproduces_peroxide_bit_for_bit() {
        for &(a, b, tol, max_iter, expected_bits) in PEROXIDE_BISECTION_CASES {
            let got = bisection_residual(
                |x: f64| x * x * x - 2.0 * x - 5.0,
                (a, b),
                max_iter,
                tol,
                "test",
            )
            .unwrap();
            assert_eq!(
                got.to_bits(),
                expected_bits,
                "[{a}, {b}] tol {tol}: got {got:e}, peroxide {:e}",
                f64::from_bits(expected_bits)
            );
        }
    }

    /// (a, b, tol, max_iter, peroxide's root as bits) for x^3 - 2x - 5.
    const PEROXIDE_BISECTION_CASES: &[(f64, f64, f64, usize, u64)] = &[
        (2.0, 3.0, 1e-8, 100, 0x4000c1a435000000), // 2.094551481306553e0
        (0.0, 5.0, 1e-8, 100, 0x4000c1a434f00000), // 2.0945514808408916e0
        (-1.0, 4.0, 1e-12, 100, 0x4000c1a435081980), // 2.0945514815422825e0
        (2.0, 2.5, 1e-6, 100, 0x4000c1a440000000), // 2.0945515632629395e0
        (1.5, 2.2, 1e-10, 100, 0x4000c1a435086666), // 2.094551481551025e0
    ];

    #[test]
    fn bisection_residual_reports_no_bracket_and_non_convergence() {
        assert!(bisection_residual(|x| x * x + 1.0, (0.0, 1.0), 100, 1e-8, "t").is_err());
        // 3 halvings cannot reach 1e-12 on [0, 5].
        assert!(bisection_residual(|x| x * x - 5.0, (0.0, 5.0), 3, 1e-12, "t").is_err());
        // An endpoint root is returned without iterating, as peroxide did.
        assert_eq!(
            bisection_residual(|x| x - 2.0, (2.0, 5.0), 0, 1e-8, "t").unwrap(),
            2.0
        );
    }

    #[test]
    fn bracketed_root_meets_its_tolerance_in_either_bracket_order() {
        for method in [BracketingMethod::Brent, BracketingMethod::FalsePosition] {
            for (a, b) in [(0.0, 5.0), (5.0, 0.0)] {
                let r = bracketed_root(method, |x| x * x - 5.0, a, b, 1e-8, 70, "t").unwrap();
                assert!((r - 5.0_f64.sqrt()).abs() < 1e-8, "{method:?}: {r}");
            }
        }
        assert!(bracketed_root(
            BracketingMethod::Brent,
            |x| x * x + 1.0,
            0.0,
            1.0,
            1e-8,
            70,
            "t"
        )
        .is_err());
    }

    #[test]
    fn spline_refuses_out_of_table_and_integrates_signed() {
        let s = NaturalCubicSpline::new(&[0.0, 1.0, 2.0, 3.0], &[1.0, 1.0, 1.0, 1.0], "flat", "-")
            .unwrap();
        assert!((s.integral(0.5, 2.5).unwrap() - 2.0).abs() < 1e-14);
        assert!((s.integral(2.5, 0.5).unwrap() + 2.0).abs() < 1e-14);
        assert!(matches!(
            s.eval(3.5),
            Err(TuasLibError::CorrelationRangeError { value, .. }) if value == 3.5
        ));
        assert!(matches!(
            s.integral(1.0, -0.5),
            Err(TuasLibError::CorrelationRangeError { value, .. }) if value == -0.5
        ));
    }

    #[test]
    fn gauss_legendre_20_is_exact_for_a_cubic() {
        let v = gauss_legendre_20(|x| x * x * x, 0.0, 2.0, "t").unwrap();
        assert!((v - 4.0).abs() < 1e-13);
    }
}
