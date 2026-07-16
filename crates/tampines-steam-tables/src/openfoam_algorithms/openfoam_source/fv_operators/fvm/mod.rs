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

//! `fvm` — implicit finite-volume method operators. Each function assembles
//! sparse LDU matrix coefficients (time-derivative, convection, or diffusion
//! terms) into an `FvMatrix`/`FvVectorMatrix` for a caller to solve, as
//! opposed to `fvc`, which evaluates an explicit field value directly. The
//! matrix itself is a dimensionless numeric container — physical units are
//! carried by the `uom`-typed fields/coefficients passed in. Mirrors
//! `Foam::fvm::` from `src/finiteVolume/finiteVolume/fvm/`.

mod ddt;
mod ddt_vec;
mod div;
mod div_vec;
mod laplacian;
mod laplacian_vec;

pub use ddt::{ddt, ddt_coeff};
pub use ddt_vec::{ddt_coeff_vec, ddt_vec};
pub use div::div;
pub use div_vec::div_vec;
pub use laplacian::laplacian;
pub use laplacian_vec::laplacian_vec;
