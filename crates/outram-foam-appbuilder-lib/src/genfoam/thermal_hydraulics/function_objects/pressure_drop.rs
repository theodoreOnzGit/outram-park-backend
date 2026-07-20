// SPDX-License-Identifier: GPL-3.0-only
// Copyright (C) 2026 OUTRAM PARK contributors
//
// Derived from GeN-Foam (Generalized Nuclear Foam)
//   Upstream: https://gitlab.com/foam-for-nuclear/GeN-Foam
//   Upstream source: src/classes/thermalHydraulics/src/functionObjects/pressureDrop/
//   Upstream commit: 652b3da
//   Upstream copyright: (C) 2015-2022 EPFL; built on OpenFOAM v2506
//   Upstream license: GPL-3.0
//
// This file is part of OUTRAM PARK, distributed under GPL-3.0. See the crate
// root or <https://www.gnu.org/licenses/> for the full license text.

//! `pressureDrop` — area-weighted-average pressure difference between two
//! boundary patches (or a patch average vs. a fixed reference pressure).
//!
//! Port of `functionObjects::pressureDrop` (`pressureDrop.C`/`.H`), **patch
//! region kind only**. Upstream additionally supports a `faceSet` region kind
//! by face-interpolating the cell pressure field (`fvc::interpolate(p)`) onto
//! an arbitrary internal-face subset; that needs an interpolation weight
//! scheme for arbitrary faces, which basic-lib does not expose as a
//! general-purpose helper outside the `fvm`/`fvc` matrix-assembly path. That
//! variant is intentionally out of scope for this port.
//! // TODO(genfoam): add a `Faces` region kind once a standalone
//! // face-interpolation helper exists for an arbitrary face subset.
//!
//! Upstream:
//! ```text
//! p1 = sum_faces(p_f * |Sf_f|) / sum_faces(|Sf_f|)    (region 1, "upstream")
//! p2 = sum_faces(p_f * |Sf_f|) / sum_faces(|Sf_f|)    (region 2, "downstream")
//! deltaP = p1 - p2
//! ```
//! For a patch region, `p_f` is the boundary patch field value
//! (`p.boundaryField()[patchID]`), i.e. already a per-face boundary value —
//! no interpolation needed, unlike the `faceSet` case above.

use outram_foam_basic_lib::fields::VolScalarField;
use outram_foam_basic_lib::mesh::FvMesh;
use uom::si::area::square_meter;
use uom::si::f64::Pressure;
use uom::si::pressure::pascal;

use super::patch_geometry::patch_area;

/// Area-weighted average of `p` over boundary patch `patch_index`.
///
/// `p_avg = sum_faces(p_f * |Sf_f|) / sum_faces(|Sf_f|)` — the same
/// area-weighted average upstream computes for each side of a `pressureDrop`
/// function object (`pressureDrop::write()`, `regionType::patch` branch).
///
/// # Panics
/// If `patch_index` is out of range for `mesh.patches` / `p.boundary`, or the
/// patch has zero total area (division by zero).
pub fn patch_average_pressure(mesh: &FvMesh, p: &VolScalarField, patch_index: usize) -> Pressure {
    let patch = &mesh.patches[patch_index];
    let bvals = &p.boundary[patch_index].values;
    let mut weighted = 0.0;
    for fi in 0..patch.size {
        weighted += bvals[fi] * mesh.face_areas[patch.start + fi];
    }
    let area_m2 = patch_area(mesh, patch_index).get::<square_meter>();
    Pressure::new::<pascal>(weighted / area_m2)
}

/// Pressure drop `p1 - p2` between two boundary patches.
///
/// `upstream_patch`/`downstream_patch` are patch indices into `mesh.patches`;
/// `p1`/`p2` are each computed by [`patch_average_pressure`]. Matches
/// upstream `pressureDrop::write()`'s `deltaP = p1 - p2` for the
/// `patch`/`patch` region-type combination.
///
/// # Panics
/// Same conditions as [`patch_average_pressure`], for either patch.
pub fn pressure_drop(
    mesh: &FvMesh,
    p: &VolScalarField,
    upstream_patch: usize,
    downstream_patch: usize,
) -> Pressure {
    patch_average_pressure(mesh, p, upstream_patch)
        - patch_average_pressure(mesh, p, downstream_patch)
}

/// Pressure drop between a patch's area-weighted average and a fixed
/// reference pressure (`p1 - p_ref`).
///
/// This is the "patch avg vs reference" mode named in this crate's porting
/// brief; it is not a literal upstream mode (upstream's `pressureDrop`
/// function object always compares two regions), but a direct specialisation
/// of the same area-weighted average with region 2 replaced by a supplied
/// constant.
pub fn pressure_drop_vs_reference(
    mesh: &FvMesh,
    p: &VolScalarField,
    patch_index: usize,
    p_reference: Pressure,
) -> Pressure {
    patch_average_pressure(mesh, p, patch_index) - p_reference
}

#[cfg(test)]
mod tests {
    use super::*;
    use outram_foam_basic_lib::fields::{BoundaryCondition, Field, PatchField, VolField};
    use outram_foam_basic_lib::mesh::{BoundaryPatch, FvMeshBuilder, PatchKind};
    use outram_foam_basic_lib::primitives::Vector3;
    use std::sync::Arc;

    /// Two single-face patches ("inlet", "outlet") on a one-cell mesh, unit
    /// area each, so an area-weighted average degenerates to a plain value.
    fn two_patch_mesh() -> Arc<FvMesh> {
        Arc::new(
            FvMeshBuilder::new()
                .n_cells(1)
                .n_internal_faces(0)
                .owner(vec![0, 0])
                .neighbour(vec![])
                .patches(vec![
                    BoundaryPatch::new("inlet", 0, 1, PatchKind::Patch),
                    BoundaryPatch::new("outlet", 1, 1, PatchKind::Patch),
                ])
                .cell_volumes(vec![1.0])
                .cell_centres(vec![Vector3::new(0.5, 0.0, 0.0)])
                .face_area_vectors(vec![
                    Vector3::new(-1.0, 0.0, 0.0),
                    Vector3::new(1.0, 0.0, 0.0),
                ])
                .face_centres(vec![
                    Vector3::new(0.0, 0.0, 0.0),
                    Vector3::new(1.0, 0.0, 0.0),
                ])
                .build()
                .unwrap(),
        )
    }

    #[test]
    fn patch_average_of_single_face_is_the_face_value() {
        let mesh = two_patch_mesh();
        let p = VolField::new(
            "p",
            Arc::clone(&mesh),
            Field::uniform(1, 500_000.0),
            vec![
                PatchField {
                    bc: BoundaryCondition::FixedValue(700_000.0),
                    values: Field::new(vec![700_000.0]),
                },
                PatchField {
                    bc: BoundaryCondition::FixedValue(100_000.0),
                    values: Field::new(vec![100_000.0]),
                },
            ],
        );

        let p_in = patch_average_pressure(&mesh, &p, 0);
        let p_out = patch_average_pressure(&mesh, &p, 1);
        assert!((p_in.get::<pascal>() - 700_000.0).abs() < 1e-9);
        assert!((p_out.get::<pascal>() - 100_000.0).abs() < 1e-9);

        let dp = pressure_drop(&mesh, &p, 0, 1);
        assert!((dp.get::<pascal>() - 600_000.0).abs() < 1e-9);
    }

    #[test]
    fn pressure_drop_vs_reference_matches_patch_minus_constant() {
        let mesh = two_patch_mesh();
        let p = VolField::new(
            "p",
            Arc::clone(&mesh),
            Field::uniform(1, 500_000.0),
            vec![
                PatchField {
                    bc: BoundaryCondition::FixedValue(700_000.0),
                    values: Field::new(vec![700_000.0]),
                },
                PatchField {
                    bc: BoundaryCondition::FixedValue(100_000.0),
                    values: Field::new(vec![100_000.0]),
                },
            ],
        );

        let dp = pressure_drop_vs_reference(&mesh, &p, 0, Pressure::new::<pascal>(650_000.0));
        assert!((dp.get::<pascal>() - 50_000.0).abs() < 1e-9);
    }
}
