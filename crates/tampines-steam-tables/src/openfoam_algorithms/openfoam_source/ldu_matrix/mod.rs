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

//! LDU (Lower-Diagonal-Upper) sparse matrix layer for FV implicit operators.
//!
//! Provides the assembled linear-system representation ([`LduMatrix`]) that
//! `fvm::` operators build into, the scalar- and vector-field wrappers around
//! it ([`FvMatrix`], [`FvVectorMatrix`]), and the linear solvers that act on
//! it ([`solvers`]: Gauss-Seidel, PCG, GAMG). All coefficients here are plain
//! dimensionless `f64` — physical units live in the field the matrix was
//! assembled from (the upstream `fvm::` call), not in the matrix itself.

/// Scalar-field implicit equation `A·φ = b`, plus its solve entry points.
pub mod fv_matrix;
/// Vector-field implicit equation `A·U = b`, solved component-wise.
pub mod fv_vector_matrix;
/// Sparse LDU matrix storage and matrix–vector primitives (`A·x`, residual).
pub mod ldu_matrix;
/// Linear solvers over [`LduMatrix`]: Gauss-Seidel, PCG, and GAMG.
pub mod solvers;

pub use fv_matrix::*;
pub use fv_vector_matrix::*;
pub use ldu_matrix::*;
pub use solvers::*;
