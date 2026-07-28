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

//! # `io` — OpenFOAM case input/output
//!
//! Purpose-built Rust parsers and writers for the OpenFOAM ASCII case files, so
//! a case can be read into typed structs (invalid keys become `Result` errors,
//! not silent runtime fallbacks) and results written back out. No C++/FFI is
//! used — see `poly_mesh`'s header for the rationale.
//!
//! - [`control_dict`] — `system/controlDict` (time control, write control).
//! - [`fv_schemes`] — `system/fvSchemes` (ddt/grad/div/laplacian scheme choices).
//! - [`fv_solution`] — `system/fvSolution` (linear-solver + PIMPLE controls).
//! - [`poly_mesh`] — `constant/polyMesh` reader (points/faces/owner/neighbour).
//! - [`field_reader`] — `0/<field>` internal-field readers (scalar and vector).
//! - [`output`] — OpenFOAM-ASCII / VTK field writers (currently unimplemented).

pub mod control_dict;
pub mod field_reader;
pub mod fv_schemes;
pub mod fv_solution;
pub mod output;
pub mod poly_mesh;
