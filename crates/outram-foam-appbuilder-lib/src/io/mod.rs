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
//! ## What actually reads and writes today
//!
//! **Only the mesh and field readers work.** The three `system/` dictionary
//! parsers and every writer are `todo!()` and panic if called, so a case is
//! configured by constructing the structs in Rust and its results are read off
//! the solver's public fields rather than from disk.
//!
//! | Module | Covers | Status |
//! |---|---|---|
//! | [`poly_mesh`] | `constant/polyMesh` (points/faces/owner/neighbour) | **Implemented** |
//! | [`field_reader`] | `0/<field>` internal fields, scalar and vector | **Implemented** |
//! | [`control_dict`] | `system/controlDict` time + write control | struct only; `read` is `todo!()` |
//! | [`fv_schemes`] | `system/fvSchemes` ddt/grad/div/laplacian choices | struct only; `read` is `todo!()` |
//! | [`fv_solution`] | `system/fvSolution` linear-solver + PIMPLE controls | struct only; `read` is `todo!()` |
//! | [`output`] | OpenFOAM-ASCII and legacy-VTK field writers | **all `todo!()`** |
//!
//! The "struct only" rows are still useful: the dictionaries they model are
//! typed enums, so a scheme or solver selection that OpenFOAM would accept
//! silently and misinterpret is instead a compile error here.

pub mod control_dict;
pub mod field_reader;
pub mod fv_schemes;
pub mod fv_solution;
pub mod output;
pub mod poly_mesh;
