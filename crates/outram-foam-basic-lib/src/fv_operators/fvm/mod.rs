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

//! Implicit (`fvm`) finite-volume operators — each **assembles into a sparse
//! matrix** (`FvMatrix` for scalar unknowns, `FvVectorMatrix` for vector
//! unknowns) whose solve advances the field, rather than returning an explicit
//! field.
//!
//! Mirrors `Foam::fvm::` (`src/finiteVolume/finiteVolume/fvm/`). Contents:
//! implicit Euler time derivatives (`ddt`, `ddt_coeff`, `ddt_vec`,
//! `ddt_coeff_vec`) and the second time derivative (`d2dt2`, `d2dt2_coeff`),
//! first-order upwind convection (`div`, `div_vec`), the Gauss-orthogonal
//! Laplacian (`laplacian`, `laplacian_vec`), its **non-orthogonality-corrected**
//! counterpart (`laplacian_corrected`, `solve_laplacian_non_orthogonal`,
//! selected by the `NonOrthoScheme` enum — the orthogonal form is silently
//! first-order-wrong on any non-hex mesh), and implicit / explicit source
//! terms (`sp`, `su`, `su_sp` and their `_vec` forms). See each function's doc
//! and the `sup` module header for the LHS / RHS sign conventions that apply
//! when combining these matrices.

mod d2dt2;
mod ddt;
mod ddt_vec;
mod div;
mod div_vec;
mod laplacian;
mod laplacian_corrected;
mod laplacian_vec;
mod sup;

pub use d2dt2::{d2dt2, d2dt2_coeff};
pub use ddt::{ddt, ddt_coeff};
pub use ddt_vec::{ddt_coeff_vec, ddt_vec};
pub use div::div;
pub use div_vec::div_vec;
pub use laplacian::laplacian;
pub use laplacian_corrected::{
    laplacian_corrected, max_non_orthogonality_deg, non_ortho_geometry,
    solve_laplacian_non_orthogonal, NonOrthoGeometry, NonOrthoScheme,
};
pub use laplacian_vec::laplacian_vec;
pub use sup::{sp, sp_vec, su, su_sp, su_sp_vec, su_vec};
