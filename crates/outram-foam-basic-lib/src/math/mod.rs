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

//! Special mathematical functions used by the thermophysics and statistics
//! kernels.
//!
//! Ports the OpenFOAM `primitives/functions/Math` helpers: the inverse error
//! function ([`erf_inv`](crate::math::erf_inv::erf_inv)), the regularised lower/upper incomplete gamma
//! functions and their unnormalised forms
//! ([`inc_gamma_ratio_p`](crate::math::inc_gamma::inc_gamma_ratio_p),
//! [`inc_gamma_ratio_q`](crate::math::inc_gamma::inc_gamma_ratio_q),
//! [`inc_gamma_p`](crate::math::inc_gamma::inc_gamma_p),
//! [`inc_gamma_q`](crate::math::inc_gamma::inc_gamma_q)), and the inverse of
//! the regularised lower incomplete gamma
//! ([`inv_inc_gamma`](crate::math::inv_inc_gamma::inv_inc_gamma)). All arguments and
//! results are dimensionless `f64`.

//! It also hosts [`parallel`], the **batched** root finders — bisection, Brent,
//! bracket-safeguarded Newton, and batched closed-form polynomial roots — each
//! dispatched across [`crate::compute::ComputeBackend`]. They live here rather
//! than under `polynomial/` because the general case solves a caller-supplied
//! residual such as `h(T) - h_target`, which is not a polynomial at all, and
//! because this module is already where the crate's iterative inversions live.

pub mod differentiate;
pub mod erf_inv;
pub mod inc_gamma;
pub mod inv_inc_gamma;
pub mod minimise;
pub mod parallel;

pub use differentiate::{
    derivative, derivative_backend_for, derivative_batch, jacobian, jacobian_batch,
    jacobian_batch_backend_for, jacobian_column_backend_for, ode_system_jacobian, DerivativeBatch,
    DerivativeSolution, DiffBatchFailure, DiffScheme, DiffSettings, DiffStatus, JacobianBatch,
    JacobianSolution, NumericalJacobian, CBRT_EPSILON, DERIVATIVE_BATCH_MIN_POINTS,
    FIFTH_ROOT_EPSILON, JACOBIAN_BATCH_MIN_PROBLEMS, JACOBIAN_COLUMN_MIN_DIMENSION,
};
pub use erf_inv::erf_inv;
pub use inc_gamma::{inc_gamma_p, inc_gamma_q, inc_gamma_ratio_p, inc_gamma_ratio_q};
pub use inv_inc_gamma::inv_inc_gamma;
pub use minimise::{
    golden_section_batch, minimise_backend_for, MinBatch, MinBatchFailure, MinProblem, MinSettings,
    MinSolution, MinStatus, Sense, GOLDEN_RATIO, MINIMISE_BATCH_MIN_PROBLEMS, SQRT_EPSILON,
};
pub use parallel::{
    cubic_roots_batch, linear_roots_batch, poly_roots_backend_for, quadratic_roots_batch,
    root_batch_backend_for, solve_bracketed_batch, solve_newton_batch, RootBatch, RootBatchFailure,
    RootMethod, RootProblem, RootSettings, RootSolution, RootStatus,
};
