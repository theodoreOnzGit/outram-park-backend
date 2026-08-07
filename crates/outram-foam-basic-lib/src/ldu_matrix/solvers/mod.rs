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

//! Iterative linear solvers for the sparse LDU systems `A·x = b`.
//!
//! Each solver takes an [`LduMatrix`](super::ldu_matrix::LduMatrix) and a
//! right-hand side and returns the solution together with the iteration count
//! and final normalised residual:
//!
//! - [`gauss_seidel`](crate::ldu_matrix::solvers::gauss_seidel()) — a robust smoother that also handles the asymmetric
//!   (convection-bearing) momentum matrix.
//! - [`conjugate_gradient`](crate::ldu_matrix::solvers::conjugate_gradient()) — DIC-preconditioned CG for symmetric SPD systems
//!   (the pressure Poisson equation).
//! - [`gamg`](crate::ldu_matrix::solvers::gamg()) — algebraic multigrid for the same symmetric SPD systems, with
//!   near mesh-independent convergence on fine grids.
//! - [`krylov_solve`](fn@crate::ldu_matrix::solvers::krylov_solve) — the adapter onto the **asymmetric** Krylov kernels in
//!   [`crate::krylov`] (BiCGStab / restarted GMRES with identity, Jacobi or
//!   ILU(0) preconditioning), for the convection-bearing matrices where PCG and
//!   GAMG do not apply and Gauss-Seidel is slow.
//!
//! Belongs here: the linear-solver kernels only. The matrix assembly and the
//! `FvMatrix`/`FvVectorMatrix` wrappers that call them live one level up.

pub mod conjugate_gradient;
pub mod gamg;
pub mod gauss_seidel;
pub mod krylov_solve;

pub use conjugate_gradient::conjugate_gradient;
pub use gamg::gamg;
pub use gauss_seidel::gauss_seidel;
pub use krylov_solve::{krylov_solve, KrylovMethod, KrylovOptions, PreconditionerKind};
