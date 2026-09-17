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

//! Iterative linear solvers for the `LduMatrix`: Gauss–Seidel smoothing,
//! (preconditioned) conjugate gradient for symmetric systems, and a geometric
//! agglomerated multigrid (GAMG). Mirrors OpenFOAM's `lduMatrix::solver`
//! hierarchy.

pub mod gauss_seidel;
pub mod conjugate_gradient;
pub mod gamg;

pub use gauss_seidel::gauss_seidel;
pub use conjugate_gradient::conjugate_gradient;
pub use gamg::gamg;
