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

//! **This is OUTRAM PARK's independent Rust translation of selected
//! OpenFOAM® primitive/finite-volume algorithms — it is not the official
//! OpenFOAM® software and is not affiliated with, endorsed by, or
//! sanctioned by OpenCFD Ltd. or the ESI Group.** OpenFOAM® is a registered
//! trademark of OpenCFD Limited. See `TRADEMARKS.md` (this crate's
//! directory, mirrored from the workspace root) for the full attribution
//! and non-affiliation notice.

pub mod fields;
pub mod fluid_thermo;
pub mod fv_operators;
pub mod interpolation;
pub mod ldu_matrix;
pub mod math;
pub mod matrix;
pub mod mesh;
pub mod ode;
pub mod polynomial;
pub mod prelude;
pub mod primitives;
pub mod thermophysics;

/// this part is extension in Rust
/// Now under here, I want to expose the openfoam primitives to something
/// that can be human readable
///
/// Also useful add-ons for the underlying libraries are put here,
/// eg. generating one dimensional meshes for system code type simulations
/// in TAMPINES
pub mod interface;
