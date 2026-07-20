// SPDX-License-Identifier: GPL-3.0-only
// Copyright (C) 2026 OUTRAM PARK contributors
//
// Derived from GeN-Foam (Generalized Nuclear Foam)
//   Upstream: https://gitlab.com/foam-for-nuclear/GeN-Foam
//   Upstream commit: 652b3da
//   Upstream copyright: (C) 2015-2022 EPFL; built on OpenFOAM v2506
//   Upstream license: GPL-3.0
//
// This file is part of OUTRAM PARK, distributed under GPL-3.0. See the crate
// root or <https://www.gnu.org/licenses/> for the full license text.

//! Shared patch-geometry helper for this module's function objects.
//!
//! Not itself one of the seven ported GeN-Foam function objects — factored
//! out to avoid duplicating the same `sum(|Sf|)` loop that upstream repeats
//! nearly verbatim in `massFlow::write()`, `pressureDrop::read()`, and
//! `TBulk::write()` (`forAll(magSf, i) { S += magSf[i]; }`).

use outram_foam_basic_lib::mesh::FvMesh;
use uom::si::area::square_meter;
use uom::si::f64::Area;

/// Total area of boundary patch `patch_index`: `sum_faces(|Sf|)`.
///
/// Mirrors the `S1_`/`S2_` accumulation in upstream `pressureDrop::read()`
/// and the (log-only) `S` accumulation in `massFlow::write()`.
///
/// # Panics
/// If `patch_index` is out of range for `mesh.patches`.
pub(crate) fn patch_area(mesh: &FvMesh, patch_index: usize) -> Area {
    let patch = &mesh.patches[patch_index];
    let sum: f64 = mesh.face_areas[patch.start..patch.end()].iter().sum();
    Area::new::<square_meter>(sum)
}

#[cfg(test)]
mod tests {
    use super::*;
    use outram_foam_basic_lib::mesh::{BoundaryPatch, FvMeshBuilder, PatchKind};
    use outram_foam_basic_lib::primitives::Vector3;

    /// One cell, three boundary faces (unit area each) all on patch
    /// "outlet", no internal faces. Shared fixture shape used across this
    /// module's V&V tests.
    fn single_cell_three_face_mesh() -> FvMesh {
        FvMeshBuilder::new()
            .n_cells(1)
            .n_internal_faces(0)
            .owner(vec![0, 0, 0])
            .neighbour(vec![])
            .patches(vec![BoundaryPatch::new("outlet", 0, 3, PatchKind::Patch)])
            .cell_volumes(vec![1.0])
            .cell_centres(vec![Vector3::new(0.5, 0.0, 0.0)])
            .face_area_vectors(vec![
                Vector3::new(1.0, 0.0, 0.0),
                Vector3::new(1.0, 0.0, 0.0),
                Vector3::new(1.0, 0.0, 0.0),
            ])
            .face_centres(vec![
                Vector3::new(1.0, 0.0, 0.0),
                Vector3::new(1.0, 0.5, 0.0),
                Vector3::new(1.0, -0.5, 0.0),
            ])
            .build()
            .unwrap()
    }

    #[test]
    fn three_unit_faces_sum_to_area_three() {
        let mesh = single_cell_three_face_mesh();
        let a = patch_area(&mesh, 0);
        assert!((a.get::<square_meter>() - 3.0).abs() < 1e-12);
    }
}
