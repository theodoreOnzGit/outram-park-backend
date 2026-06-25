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

pub mod euler;
pub mod rkf45;
pub mod rosenbrock23;

pub use euler::Euler;
pub use rkf45::Rkf45;
pub use rosenbrock23::Rosenbrock23;

use crate::matrix::SquareMatrix;

// ── ODE system trait ─────────────────────────────────────────────────────────

/// Abstract ODE system `dy/dx = f(x, y)`. Maps to `Foam::ODESystem`.
pub trait OdeSystem {
    fn n_eqns(&self) -> usize;

    /// Fill `dydx` with the derivatives at `(x, y)`.
    fn derivatives(&self, x: f64, y: &[f64], dydx: &mut Vec<f64>);

    /// Fill `dfdx` and `dfdy` with the Jacobian at `(x, y)`.
    ///
    /// Required only by stiff solvers (Rosenbrock23). The default panics.
    fn jacobian(
        &self,
        _x: f64,
        _y: &[f64],
        _dfdx: &mut Vec<f64>,
        _dfdy: &mut SquareMatrix,
    ) {
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

#[derive(Debug, Clone, PartialEq)]
pub enum OdeError {
    StepSizeUnderflow,
    MaxStepsExceeded(usize),
}

impl std::fmt::Display for OdeError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::StepSizeUnderflow => write!(f, "ODE step size underflow"),
            Self::MaxStepsExceeded(n) => write!(f, "ODE exceeded {n} steps"),
        }
    }
}

impl std::error::Error for OdeError {}

// ── Shared utilities ─────────────────────────────────────────────────────────

/// Normalised scalar error — max over all equations.
/// Maps to `Foam::ODESolver::normalizeError`.
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
        .fold(0.0_f64, f64::max)
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
        let scale = (cfg.safe_scale * err.powf(-cfg.alpha_inc))
            .clamp(cfg.min_scale, cfg.max_scale);
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
