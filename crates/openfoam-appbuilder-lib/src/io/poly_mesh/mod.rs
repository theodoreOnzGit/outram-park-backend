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

use std::path::Path;
use std::sync::Arc;
use openfoam_basic_lib::prelude::FvMesh;
use crate::error::AppBuilderError;

/// Read an OpenFOAM `constant/polyMesh/` directory and return an `FvMesh`.
///
/// Expected files:
///   `points`     — one `(x y z)` vertex per line
///   `faces`      — one face per line: `N(v0 v1 … vN-1)`
///   `owner`      — owner cell index per face
///   `neighbour`  — neighbour cell index per internal face
///   `boundary`   — patch definitions (name, type, startFace, nFaces)
///
/// C++ source: `src/OpenFOAM/meshes/polyMesh/`
pub fn read_poly_mesh(poly_mesh_dir: &Path) -> Result<Arc<FvMesh>, AppBuilderError> {
    let _ = poly_mesh_dir;
    todo!("read_poly_mesh: parse points, faces, owner, neighbour, boundary files")
}

/// Parse the `points` file: lines of `(x y z)` into `Vec<[f64; 3]>`.
fn parse_points(_text: &str) -> Result<Vec<[f64; 3]>, AppBuilderError> {
    todo!("parse_points")
}

/// Parse the `faces` file: lines of `N(v0 v1 … vN-1)` into `Vec<Vec<usize>>`.
fn parse_faces(_text: &str) -> Result<Vec<Vec<usize>>, AppBuilderError> {
    todo!("parse_faces")
}

/// Parse the `owner` / `neighbour` files: one integer per line.
fn parse_index_list(_text: &str) -> Result<Vec<usize>, AppBuilderError> {
    todo!("parse_index_list")
}
