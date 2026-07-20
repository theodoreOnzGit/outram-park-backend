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

//! Sparse linear-system layer: the `LduMatrix` (lower/diagonal/upper
//! face-addressed storage), the `FvMatrix`/`FvVectorMatrix` wrappers the `fvm`
//! operators assemble into, and the `solvers` submodule (Gauss–Seidel, CG,
//! GAMG). Mirrors OpenFOAM's `lduMatrix`/`fvMatrix` from
//! `src/OpenFOAM/matrices/` and `src/finiteVolume/fvMatrices/`.

pub mod ldu_matrix;
pub mod fv_matrix;
pub mod fv_vector_matrix;
pub mod solvers;

pub use ldu_matrix::*;
pub use fv_matrix::*;
pub use fv_vector_matrix::*;
pub use solvers::*;
