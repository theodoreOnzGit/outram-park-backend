// SPDX-License-Identifier: GPL-3.0-only
// Copyright (C) 2026 OUTRAM PARK contributors
// Derived from OpenFOAM (www.openfoam.com)
// Copyright (C) 2004-2023 OpenFOAM Foundation
// Copyright (C) 2016-2023 OpenCFD Ltd.
//
// This file is part of OUTRAM PARK.
//
// OUTRAM PARK is free software: you can redistribute it and/or modify it
// under the terms of the GNU General Public License as published by the
// Free Software Foundation, either version 3 of the License, or (at your
// option) any later version.
//
// OUTRAM PARK is distributed in the hope that it will be useful, but
// WITHOUT ANY WARRANTY; without even the implied warranty of
// MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE.  See the GNU
// General Public License for more details.
//
// You should have received a copy of the GNU General Public License along
// with OUTRAM PARK.  If not, see <https://www.gnu.org/licenses/>.

//! Generic adaptive ODE integrators (`dy/dx = f(x, y)`) ported from
//! `Foam::ODESolver`/`Foam::adaptiveSolver` and concrete solvers. The system
//! being integrated is abstracted behind [`OdeSystem`]; the physical meaning
//! of `y` (e.g. species concentrations, reactor state) is entirely up to the
//! caller's `OdeSystem` implementation — this module only sees plain `f64`
//! state vectors.

/// Explicit first-order adaptive Euler solver.
pub mod euler;
/// Adaptive embedded Runge-Kutta-Fehlberg 4(5) solver.
pub mod rkf45;
/// W-method Rosenbrock23 semi-implicit stiff solver.
pub mod rosenbrock23;

pub use euler::Euler;
pub use rkf45::Rkf45;
pub use rosenbrock23::Rosenbrock23;

use crate::openfoam_algorithms::openfoam_source::SquareMatrix;

// ── ODE system trait ─────────────────────────────────────────────────────────

/// Abstract ODE system `dy/dx = f(x, y)`. Maps to `Foam::ODESystem`.
pub trait OdeSystem {
    /// Number of equations (length of the `y` state vector).
    fn n_eqns(&self) -> usize;

    /// Fill `dydx` with the derivatives at `(x, y)`.
    fn derivatives(&self, x: f64, y: &[f64], dydx: &mut Vec<f64>);

    /// Fill `dfdx` and `dfdy` with the Jacobian at `(x, y)`.
    ///
    /// Required only by stiff solvers (Rosenbrock23). The default panics.
    fn jacobian(&self, _x: f64, _y: &[f64], _dfdx: &mut Vec<f64>, _dfdy: &mut SquareMatrix) {
        unimplemented!("jacobian not implemented for this ODE system");
    }
}

// ── Solver configuration ─────────────────────────────────────────────────────

/// Common parameters for the adaptive step-size controller.
/// Defaults match `Foam::adaptiveSolver` and `Foam::ODESolver`.
#[derive(Debug, Clone)]
pub struct OdeSolverConfig {
    /// Absolute per-equation tolerance.
    pub abs_tol: f64,
    /// Relative per-equation tolerance.
    pub rel_tol: f64,
    /// Safety factor on the step-size scaling (0 < safeScale < 1).
    pub safe_scale: f64,
    /// Exponent for step *increase*.
    pub alpha_inc: f64,
    /// Exponent for step *decrease*.
    pub alpha_dec: f64,
    /// Minimum scale factor applied per step.
    pub min_scale: f64,
    /// Maximum scale factor applied per step.
    pub max_scale: f64,
    /// Maximum sub-steps for one `integrate()` call.
    pub max_steps: usize,
}

impl Default for OdeSolverConfig {
    fn default() -> Self {
        Self {
            abs_tol: 1e-6,
            rel_tol: 1e-4,
            safe_scale: 0.9,
            alpha_inc: 0.2,
            alpha_dec: 0.25,
            min_scale: 0.2,
            max_scale: 10.0,
            max_steps: 10_000,
        }
    }
}

// ── Error type ───────────────────────────────────────────────────────────────

/// Failure modes of the adaptive ODE solvers in this module.
#[derive(Debug, Clone, PartialEq)]
pub enum OdeError {
    /// The adaptive step-size controller shrank `dx` below `f64::EPSILON`
    /// while trying to satisfy the error tolerance without converging.
    StepSizeUnderflow,
    /// `integrate()` did not reach `x_end` within `OdeSolverConfig::max_steps`
    /// sub-steps; the wrapped value is the step count reached.
    MaxStepsExceeded(usize),
    /// The system produced a non-finite (NaN or infinite) error estimate, so
    /// the state cannot be trusted and integration stopped.
    ///
    /// The usual cause is `OdeSystem::derivatives` or `OdeSystem::jacobian`
    /// returning a non-finite value — for example an evaluation that left the
    /// model's valid range (an IF97 flash outside its region, a property
    /// correlation past its bounds).
    ///
    /// # Why this variant exists
    ///
    /// Before bead `op-zwk0` this case did not error at all: it returned
    /// `Ok(())` with a NaN state, because the per-equation error fold in
    /// [`normalize_error`] used `f64::max`, which follows IEEE-754 `maxNum`
    /// and discards a NaN operand. A wrong answer reported as success is the
    /// worst failure mode available to a solver, so it is now a distinct error
    /// rather than being folded into
    /// [`StepSizeUnderflow`](Self::StepSizeUnderflow), which would have named
    /// the wrong cause. Fixed upstream in `outram-foam-basic-lib` as bead
    /// `op-ad6h`; this is the same fix in this vendored copy.
    NonFiniteState,
}

impl std::fmt::Display for OdeError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::StepSizeUnderflow => write!(f, "ODE step size underflow"),
            Self::MaxStepsExceeded(n) => write!(f, "ODE exceeded {n} steps"),
            Self::NonFiniteState => write!(
                f,
                "ODE produced a non-finite error estimate (NaN or infinite state)"
            ),
        }
    }
}

impl std::error::Error for OdeError {}

// ── Shared utilities ─────────────────────────────────────────────────────────

/// Normalised scalar error — max over all equations.
/// Maps to `Foam::ODESolver::normalizeError`.
///
/// Returns `f64::INFINITY` if any per-equation term is non-finite; see the
/// comment on the fold below for why that, and not `NaN`, is the right answer.
pub(crate) fn normalize_error(
    y0: &[f64],
    y: &[f64],
    err: &[f64],
    abs_tol: f64,
    rel_tol: f64,
) -> f64 {
    err.iter()
        .zip(y0)
        .zip(y)
        .map(|((e, &a), &b)| {
            let tol = abs_tol + rel_tol * a.abs().max(b.abs());
            e.abs() / tol
        })
        // NB: `fold(0.0, f64::max)` here was a silent-wrong-answer bug
        // (bead `op-zwk0`; fixed upstream in `outram-foam-basic-lib` as
        // `op-ad6h`). Rust's `f64::max` follows IEEE-754 `maxNum` and DISCARDS
        // a NaN operand, so `f64::max(0.0, NaN) == 0.0`: a NaN error read as
        // *zero* error, the step was accepted as perfectly converged, and
        // `integrate` returned `Ok(())` with a NaN state. Returning NaN
        // instead would not fix it either, because the caller's accept test is
        // `err <= 1.0` and every comparison against NaN is false — the step
        // would be rejected, but `dx` would then shrink to underflow with no
        // indication of why.
        //
        // So a non-finite error is reported as `INFINITY`, which is both true
        // ("this step is unboundedly bad") and actionable: the caller rejects
        // it and, per the checks in `adaptive_step` and
        // `Rosenbrock23::solve_step`, returns `OdeError::NonFiniteState`
        // rather than a misleading `StepSizeUnderflow`.
        .fold(0.0_f64, |acc, e| {
            if e.is_nan() || acc.is_nan() {
                f64::INFINITY
            } else {
                acc.max(e)
            }
        })
}

/// Adaptive step-size loop shared by all explicit solvers.
///
/// Calls `inner_step(x0, y0, dydx0, dx, y_out) -> err`, retrying with a
/// smaller `dx` whenever `err > 1`. Updates `x`, `y`, and `dx_try`.
/// Matches `Foam::adaptiveSolver::solve`.
pub(crate) fn adaptive_step(
    cfg: &OdeSolverConfig,
    mut inner_step: impl FnMut(f64, &[f64], &[f64], f64, &mut Vec<f64>) -> f64,
    ode: &dyn OdeSystem,
    x: &mut f64,
    y: &mut Vec<f64>,
    dydx0: &mut Vec<f64>,
    y_temp: &mut Vec<f64>,
    dx_try: &mut f64,
) -> Result<(), OdeError> {
    let mut dx = *dx_try;
    ode.derivatives(*x, y, dydx0);

    let err = loop {
        let err = inner_step(*x, y, dydx0, dx, y_temp);
        // A non-finite error means the system produced NaN or an infinite
        // value; shrinking `dx` cannot recover from that, so fail immediately
        // and name the real cause instead of grinding down to a misleading
        // `StepSizeUnderflow`. See `OdeError::NonFiniteState` and bead
        // `op-zwk0`.
        if !err.is_finite() {
            return Err(OdeError::NonFiniteState);
        }
        if err <= 1.0 {
            break err;
        }
        let scale = (cfg.safe_scale * err.powf(-cfg.alpha_dec)).max(cfg.min_scale);
        dx *= scale;
        if dx.abs() < f64::EPSILON {
            return Err(OdeError::StepSizeUnderflow);
        }
    };

    *x += dx;
    std::mem::swap(y, y_temp);

    let threshold = (cfg.max_scale / cfg.safe_scale).powf(-1.0 / cfg.alpha_inc);
    *dx_try = if err > threshold {
        let scale = (cfg.safe_scale * err.powf(-cfg.alpha_inc)).clamp(cfg.min_scale, cfg.max_scale);
        dx * scale
    } else {
        dx * cfg.safe_scale * cfg.max_scale
    };

    Ok(())
}

/// Integrate from `x_start` to `x_end` using repeated adaptive steps.
pub(crate) fn integrate_interval(
    cfg: &OdeSolverConfig,
    step_fn: &mut dyn FnMut(&mut f64, &mut Vec<f64>, &mut f64) -> Result<(), OdeError>,
    x_start: f64,
    x_end: f64,
    y: &mut Vec<f64>,
    dx_est: &mut f64,
) -> Result<(), OdeError> {
    let mut x = x_start;
    let mut dx = *dx_est;
    let mut n_steps = 0usize;

    while x < x_end {
        let dx_try = (x + dx).min(x_end) - x;
        let mut dx_limited = dx_try;
        step_fn(&mut x, y, &mut dx_limited)?;
        dx = dx_limited;
        n_steps += 1;
        if n_steps > cfg.max_steps {
            return Err(OdeError::MaxStepsExceeded(n_steps));
        }
    }

    *dx_est = dx;
    Ok(())
}

// ── Regression tests: non-finite state must not be reported as success ───────

/// Verification that a NaN produced by an `OdeSystem` is reported as an error
/// rather than as a converged solution (bead `op-zwk0`; the same defect was
/// fixed upstream in `outram-foam-basic-lib` as `op-ad6h`).
///
/// # Methodology
///
/// The system under test, [`NanDerivatives`], returns `dydx = NaN` from
/// `derivatives` at every point while supplying a perfectly finite Jacobian,
/// so the NaN can only enter through `f(x, y)`. It is integrated from
/// `x = 0` to `x = 1` from `y0 = [1.0]` with `abs_tol = 1e-6`,
/// `rel_tol = 1e-4` and an initial `dx = 0.1`, by each of the three steppers
/// in this module. [`normalize_error`] is additionally exercised directly on
/// NaN, infinite and ordinary finite inputs.
///
/// Pass criterion: every `integrate` call returns
/// `Err(OdeError::NonFiniteState)`, and `normalize_error` returns
/// `f64::INFINITY` for any non-finite component while leaving finite inputs
/// numerically unchanged.
///
/// # Results (measured 2026-08-13, release mode, this machine)
///
/// **Before the fix**, the probe printed:
///
/// ```text
/// normalize_error(y0=[1.0], y=[NaN], err=[NaN]) = 0
/// f64::max(0.0, NaN) = 0
/// Euler        -> Ok(()), y = [NaN], y[0].is_nan() = true
/// Rkf45        -> Ok(()), y = [NaN], y[0].is_nan() = true
/// Rosenbrock23 -> Ok(()), y = [NaN], y[0].is_nan() = true
/// ```
///
/// i.e. all three steppers returned **success with a NaN state** — the
/// silent-wrong-answer failure mode. `f64::max` follows IEEE-754 `maxNum`,
/// which discards a NaN operand, so the fold `fold(0.0, f64::max)` read a NaN
/// error as *zero* error and the very first step was accepted as perfectly
/// converged.
///
/// **After the fix** all three return `Err(OdeError::NonFiniteState)` and the
/// tests below assert exactly that. Note that `Rosenbrock23` carries its own
/// copy of the adaptive retry loop (`Rosenbrock23::solve_step`) instead of
/// calling [`adaptive_step`], so it needed the guard independently — a partial
/// fix touching only `adaptive_step` would have left the stiff solver broken.
///
/// # Interpretation and limitations
///
/// This is a **verification** test (is the error path implemented correctly?),
/// not a validation of any physics. It says nothing about whether a real
/// thermophysical model will produce a NaN, only that if one does, the solver
/// now says so instead of returning a plausible-looking number. `INFINITY`,
/// not `NaN`, is returned by the fold on purpose: the accept test is
/// `err <= 1.0` and every comparison against NaN is false, so a NaN error
/// would reject the step and then grind `dx` down to a misleading
/// `StepSizeUnderflow` that names the wrong cause.
#[cfg(test)]
mod non_finite_state_regression {
    use super::*;

    /// A system whose `derivatives` are NaN everywhere. The Jacobian is
    /// deliberately finite so that the NaN can only enter through `f(x, y)`.
    struct NanDerivatives;
    impl OdeSystem for NanDerivatives {
        fn n_eqns(&self) -> usize {
            1
        }
        fn derivatives(&self, _x: f64, _y: &[f64], dydx: &mut Vec<f64>) {
            dydx[0] = f64::NAN;
        }
        fn jacobian(&self, _x: f64, _y: &[f64], dfdx: &mut Vec<f64>, dfdy: &mut SquareMatrix) {
            dfdx[0] = 0.0;
            dfdy.set(0, 0, -1.0);
        }
    }

    /// A system with finite derivatives but a NaN Jacobian — only the stiff
    /// solver reads the Jacobian, so this isolates `Rosenbrock23`'s own
    /// duplicated retry loop.
    struct NanJacobian;
    impl OdeSystem for NanJacobian {
        fn n_eqns(&self) -> usize {
            1
        }
        fn derivatives(&self, _x: f64, y: &[f64], dydx: &mut Vec<f64>) {
            dydx[0] = -y[0];
        }
        fn jacobian(&self, _x: f64, _y: &[f64], dfdx: &mut Vec<f64>, dfdy: &mut SquareMatrix) {
            dfdx[0] = f64::NAN;
            dfdy.set(0, 0, f64::NAN);
        }
    }

    /// Well-behaved control: `y' = -y`, `y(0) = 1`, so `y(1) = e^-1`.
    struct DecayControl;
    impl OdeSystem for DecayControl {
        fn n_eqns(&self) -> usize {
            1
        }
        fn derivatives(&self, _x: f64, y: &[f64], dydx: &mut Vec<f64>) {
            dydx[0] = -y[0];
        }
        fn jacobian(&self, _x: f64, _y: &[f64], dfdx: &mut Vec<f64>, dfdy: &mut SquareMatrix) {
            dfdx[0] = 0.0;
            dfdy.set(0, 0, -1.0);
        }
    }

    #[test]
    fn normalize_error_is_infinite_for_a_nan_component() {
        // The bug in one line: `f64::max` discards NaN, so the old fold
        // returned 0.0 here — "perfectly converged".
        assert_eq!(0.0_f64.max(f64::NAN), 0.0, "f64::max still discards NaN");

        let got = normalize_error(&[1.0], &[f64::NAN], &[f64::NAN], 1e-6, 1e-4);
        assert!(
            got.is_infinite() && got > 0.0,
            "expected +inf for a NaN component, got {got}"
        );
    }

    #[test]
    fn normalize_error_is_infinite_for_an_infinite_component() {
        let got = normalize_error(&[1.0], &[f64::INFINITY], &[f64::INFINITY], 1e-6, 1e-4);
        assert!(
            got.is_infinite() && got > 0.0,
            "expected +inf for an infinite component, got {got}"
        );
    }

    #[test]
    fn normalize_error_is_unchanged_for_finite_input() {
        // tol = abs_tol + rel_tol * max(|y0|, |y|) = 1e-6 + 1e-4 * 1.0
        let tol = 1e-6 + 1e-4;
        let expected = 4e-6 / tol; // the larger of the two normalised errors
        let got = normalize_error(&[1.0, 1.0], &[1.0, 1.0], &[2e-6, 4e-6], 1e-6, 1e-4);
        assert!(
            (got - expected).abs() < 1e-12,
            "finite behaviour changed: got {got}, expected {expected}"
        );
    }

    #[test]
    fn euler_reports_nan_instead_of_returning_ok() {
        let mut solver = Euler::new(1, 1e-6, 1e-4);
        let mut y = vec![1.0_f64];
        let mut dx = 0.1;
        let result = solver.integrate(&NanDerivatives, 0.0, 1.0, &mut y, &mut dx);
        assert_eq!(result, Err(OdeError::NonFiniteState), "y = {y:?}");
    }

    #[test]
    fn rkf45_reports_nan_instead_of_returning_ok() {
        let mut solver = Rkf45::new(1, 1e-6, 1e-4);
        let mut y = vec![1.0_f64];
        let mut dx = 0.1;
        let result = solver.integrate(&NanDerivatives, 0.0, 1.0, &mut y, &mut dx);
        assert_eq!(result, Err(OdeError::NonFiniteState), "y = {y:?}");
    }

    #[test]
    fn rosenbrock23_reports_nan_derivatives_instead_of_returning_ok() {
        // Rosenbrock23 duplicates the adaptive retry loop rather than calling
        // `adaptive_step`, so this exercises a second, separate guard.
        let mut solver = Rosenbrock23::new(1, 1e-6, 1e-4);
        let mut y = vec![1.0_f64];
        let mut dx = 0.1;
        let result = solver.integrate(&NanDerivatives, 0.0, 1.0, &mut y, &mut dx);
        assert_eq!(result, Err(OdeError::NonFiniteState), "y = {y:?}");
    }

    #[test]
    fn rosenbrock23_reports_a_nan_jacobian() {
        let mut solver = Rosenbrock23::new(1, 1e-6, 1e-4);
        let mut y = vec![1.0_f64];
        let mut dx = 0.1;
        let result = solver.integrate(&NanJacobian, 0.0, 1.0, &mut y, &mut dx);
        assert_eq!(result, Err(OdeError::NonFiniteState), "y = {y:?}");
    }

    #[test]
    fn well_behaved_systems_still_integrate_after_the_fix() {
        let expected = (-1.0_f64).exp();

        // Euler is 1st order — it needs loose tolerances to keep the step
        // count sane, exactly as the existing `euler_exponential_decay` test
        // in `euler.rs` does.
        let mut e = Euler::new(1, 1e-3, 1e-2);
        let mut y = vec![1.0_f64];
        let mut dx = 0.1;
        e.integrate(&DecayControl, 0.0, 1.0, &mut y, &mut dx)
            .expect("Euler on y' = -y");
        assert!((y[0] - expected).abs() < 1e-2, "Euler y = {}", y[0]);

        let mut r = Rkf45::new(1, 1e-8, 1e-6);
        let mut y = vec![1.0_f64];
        let mut dx = 0.1;
        r.integrate(&DecayControl, 0.0, 1.0, &mut y, &mut dx)
            .expect("Rkf45 on y' = -y");
        assert!((y[0] - expected).abs() < 1e-6, "Rkf45 y = {}", y[0]);

        let mut rb = Rosenbrock23::new(1, 1e-8, 1e-6);
        let mut y = vec![1.0_f64];
        let mut dx = 0.1;
        rb.integrate(&DecayControl, 0.0, 1.0, &mut y, &mut dx)
            .expect("Rosenbrock23 on y' = -y");
        assert!((y[0] - expected).abs() < 1e-6, "Rosenbrock23 y = {}", y[0]);
    }
}
