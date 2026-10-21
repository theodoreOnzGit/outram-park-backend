// SPDX-License-Identifier: GPL-3.0-only
// Copyright (C) 2026 OUTRAM PARK contributors
//
// Layout ported from this workspace's `outram_foam_basic_lib::interface::
// one_dimensional_meshing::create_one_d_mesh` (GPL-3.0-only), which carries no
// point coordinates; this file builds the same column with points.
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

//! The **1-D mesher**: a column of `n` equal hexahedra along `x`, the neutral
//! counterpart of `outram-foam-basic-lib`'s `create_one_d_mesh`.
//!
//! # Why this is not a call to `create_one_d_mesh`
//!
//! Checked and rejected (2026-10-03): `create_one_d_mesh` returns an `FvMesh`,
//! which stores cell/face geometry but **no points**, so no neutral mesh can be
//! rebuilt from it; and this crate's core cannot depend on
//! `outram-foam-basic-lib` (outram-mc-libs takes the core without features).
//! So the column is built here with points, and the
//! `one_d_column_matches_create_one_d_mesh` test (feature `foam-export`) pins
//! it to the original: same cell volumes and centres, same internal-face area
//! vectors, same `right`/`left` patches in the same order. The only addition
//! is the lateral surface, which an `FvMesh` column simply omits and this mesh
//! carries as an `empty` patch named `sides` (the OpenFOAM way to say "no
//! flux in this direction").

use super::element::{BoundarySpec, Element, ElementKind, LengthUnit, PatchKind};
use super::mesh::{UnstructuredMesh, UnstructuredMeshError};

/// A column of `n_cells` equal [`ElementKind::Hex8`] cells over
/// `x in [0, length]`, square cross-section of area `area`, centred on the
/// `x` axis (so cell centres sit at `y = z = 0`, as in `create_one_d_mesh`).
///
/// Patches, in order: `right` (`x = length`, [`PatchKind::Patch`]), `left`
/// (`x = 0`, [`PatchKind::Patch`]), `sides` (the lateral faces,
/// [`PatchKind::Empty`]).
///
/// # Arguments
/// - `unit` — unit of `length` (and of `area`, squared).
/// - `length` — column length, `> 0`.
/// - `area` — cross-sectional area, `> 0`.
/// - `n_cells` — number of cells, `>= 1`.
///
/// # Errors
/// [`UnstructuredMeshError::Empty`] for `n_cells == 0` or a non-positive
/// length or area.
pub fn one_d_column(
    unit: LengthUnit,
    length: f64,
    area: f64,
    n_cells: usize,
) -> Result<UnstructuredMesh, UnstructuredMeshError> {
    if n_cells == 0 || !(length > 0.0) || !(area > 0.0) {
        return Err(UnstructuredMeshError::Empty(
            "one_d_column: need n_cells >= 1 and a positive length and area",
        ));
    }
    let h = 0.5 * area.sqrt();
    let dx = length / n_cells as f64;
    // Station i holds the four corners (y, z) = (-h,-h), (h,-h), (h,h), (-h,h):
    // counter-clockwise seen from +x, so with zeta = x they are the Hex8
    // bottom-face ordering.
    let corners = [[-h, -h], [h, -h], [h, h], [-h, h]];
    let mut points = Vec::with_capacity(4 * (n_cells + 1));
    for i in 0..=n_cells {
        let x = if i == n_cells { length } else { i as f64 * dx };
        for c in corners {
            points.push([x, c[0], c[1]]);
        }
    }
    let mut elements = Vec::with_capacity(n_cells);
    let mut sides = Vec::with_capacity(4 * n_cells);
    for i in 0..n_cells {
        let nodes: Vec<usize> = (0..8).map(|k| 4 * i + k).collect();
        for lf in &ElementKind::Hex8.local_faces()[2..] {
            sides.push(lf.iter().map(|&k| nodes[k]).collect());
        }
        elements.push(Element { kind: ElementKind::Hex8, nodes });
    }
    let last = 4 * n_cells;
    let boundaries = vec![
        BoundarySpec {
            name: "right".into(),
            kind: PatchKind::Patch,
            facets: vec![vec![last, last + 1, last + 2, last + 3]],
        },
        BoundarySpec { name: "left".into(), kind: PatchKind::Patch, facets: vec![vec![0, 1, 2, 3]] },
        BoundarySpec { name: "sides".into(), kind: PatchKind::Empty, facets: sides },
    ];
    UnstructuredMesh::from_elements(unit, points, elements, boundaries, Vec::new())
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Ten cells over 2 m with 0.04 m^2 cross-section: volumes are
    /// `L A / n`, centres at `(i + 1/2) dx`, nine internal faces.
    #[test]
    fn column_geometry() {
        let m = one_d_column(LengthUnit::Metre, 2.0, 0.04, 10).unwrap();
        assert_eq!(m.n_cells(), 10);
        assert_eq!(m.n_internal_faces(), 9);
        for c in 0..10 {
            assert!((m.cell_volume(c) - 0.008).abs() < 1e-15);
            let x = m.cell_centre(c);
            assert!((x[0] - (c as f64 + 0.5) * 0.2).abs() < 1e-14);
            assert!(x[1].abs() < 1e-15 && x[2].abs() < 1e-15);
        }
        let names: Vec<&str> = m.patches().iter().map(|p| p.name.as_str()).collect();
        assert_eq!(names, ["right", "left", "sides"]);
        assert_eq!(m.patches()[2].faces.len(), 40);
        // Every cell is a recognised Hex8 with outward-oriented faces.
        assert!((0..10).all(|c| m.cell_kind(c) == ElementKind::Hex8));
        assert!((0..m.n_faces()).all(|f| m.face_is_oriented(f)));
    }

    #[test]
    fn rejects_zero_cells() {
        assert!(one_d_column(LengthUnit::Metre, 1.0, 1.0, 0).is_err());
    }
}
