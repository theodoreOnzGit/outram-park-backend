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

//! Sparse LDU (lower-diagonal-upper) linear algebra for implicit FV solves.
//!
//! Holds the face-addressed sparse matrix representation used by the
//! finite-volume implicit operators and the iterative solvers that invert it:
//!
//! - [`ldu_matrix::LduMatrix`] — the raw sparse coefficients (diagonal + per-face
//!   lower/upper off-diagonals) and matrix–vector / residual kernels.
//! - [`FvMatrix`](crate::ldu_matrix::fv_matrix::FvMatrix) — a scalar equation `A·φ = b` for a `VolScalarField`,
//!   assembled by the Layer-3 `fvm::` operators.
//! - [`FvVectorMatrix`](crate::ldu_matrix::fv_vector_matrix::FvVectorMatrix) — the vector counterpart `A·U = b` with
//!   scalar LDU coefficients and a `Field<Vector3>` source.
//! - [`parallel`](crate::ldu_matrix::parallel) — the same sparse product and the
//!   per-iteration vector operations on the hybrid execution backend
//!   ([`HybridLdu`](crate::ldu_matrix::parallel::HybridLdu),
//!   [`LduTopology`](crate::ldu_matrix::parallel::LduTopology)): one entry point
//!   per operation taking a [`ComputeBackend`](crate::compute::ComputeBackend),
//!   serial or multi-CPU, bit-for-bit identical either way.
//! - [`solvers`](crate::ldu_matrix::solvers) — Gauss-Seidel, DIC-preconditioned conjugate gradient, GAMG
//!   (algebraic multigrid), and the [`krylov_solve`](crate::ldu_matrix::solvers::krylov_solve()) adapter onto the asymmetric
//!   BiCGStab / GMRES kernels in [`crate::krylov`].
//!
//! Belongs here: the sparse-matrix storage, its arithmetic, and the linear
//! solvers. Field types, meshes, and the differential operators that build these
//! matrices live in their own modules.

pub mod fv_matrix;
pub mod fv_vector_matrix;
pub mod ldu_matrix;
pub mod parallel;
pub mod solvers;

pub use fv_matrix::{FvMatrix, SolverPerformance, SolverSettings};
pub use fv_vector_matrix::FvVectorMatrix;
pub use ldu_matrix::LduMatrix;
pub use parallel::{HybridLdu, LduTopology};
pub use solvers::conjugate_gradient;
pub use solvers::gamg;
pub use solvers::gauss_seidel;
pub use solvers::krylov_solve::{
    krylov_solve, krylov_solve_prepared, KrylovMethod, KrylovOptions, PreconditionerKind,
};
