// SPDX-License-Identifier: GPL-3.0-only
// Copyright (C) 2026 OUTRAM PARK contributors
//
// Derived from GeN-Foam (Generalized Nuclear Foam)
//   Upstream: https://gitlab.com/foam-for-nuclear/GeN-Foam
//   Upstream source: src/classes/thermalHydraulics/src/functionObjects/fieldDiffExtents/
//   Upstream commit: 652b3da
//   Upstream copyright: (C) 2015-2022 EPFL; built on OpenFOAM v2506
//   Upstream license: GPL-3.0
//
// This file is part of OUTRAM PARK, distributed under GPL-3.0. See the crate
// root or <https://www.gnu.org/licenses/> for the full license text.

//! `fieldDiffExtents` — spatial bounding-box extents of where one field
//! exceeds a "mask" field, at a single instant.
//!
//! Port of `functionObjects::fieldDiffExtents`
//! (`fieldDiffExtents.C`/`.H`/`fieldDiffExtentsTemplates.C`), **scalar fields
//! only** — upstream is templated over `scalar`/`vector`/`sphericalTensor`/
//! `symmTensor`/`tensor`; this port implements only the `scalar`
//! instantiation, the common case for TH monitoring fields like `T`/`p`.
//!
//! **Despite the "Diff" in the name, this does not compare a field to its own
//! value at a previous timestep.** It compares **two different fields**
//! (`field` vs `maskField`) at the *same* instant, cell-by-cell / face-by-face:
//!
//! ```text
//! mask_i = true   if field_i > maskField_i
//!        = false  otherwise
//! extents = bounding box of { C_i - C0 : mask_i }
//! ```
//!
//! where `C_i` is a cell centre (internal field) or patch face centre
//! (boundary), and `C0` is a user-chosen reference position (upstream's
//! `referencePosition`, default the origin).
//!
//! Note the comparison is **unsigned, no `mag()`** — this matches upstream's
//! **scalar specialisation** of `calcMask`
//! (`return pos((field/oneField) - (maskField/oneMaskField));` in
//! `fieldDiffExtents.C`), which differs from the generic vector/tensor
//! template in `fieldDiffExtentsTemplates.C`
//! (`return pos(mag(field/oneField) - mag(maskField/oneMaskField));`). Since
//! this port only implements the scalar case, only the unsigned form applies.
//!
//! If no cell/face satisfies the mask, upstream falls back to a degenerate
//! box at the reference position (`extents.add(point::zero)` after `C0`
//! subtraction) — [`internal_extents`] and [`patch_extents`] mirror that
//! fallback exactly, so an "empty" result is a well-defined zero-size box
//! rather than an unbounded/inverted one.

use outram_foam_basic_lib::fields::VolScalarField;
use outram_foam_basic_lib::mesh::FvMesh;
use outram_foam_basic_lib::primitives::Vector3;

/// Axis-aligned bounding box, `min`/`max` corner. Mirrors upstream's
/// `Foam::boundBox`.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Extents {
    pub min: Vector3,
    pub max: Vector3,
}

impl Extents {
    fn empty() -> Self {
        Self {
            min: Vector3::ZERO,
            max: Vector3::ZERO,
        }
    }

    fn grow(&mut self, p: Vector3, seeded: &mut bool) {
        if !*seeded {
            self.min = p;
            self.max = p;
            *seeded = true;
        } else {
            self.min = Vector3::new(
                self.min.x.min(p.x),
                self.min.y.min(p.y),
                self.min.z.min(p.z),
            );
            self.max = Vector3::new(
                self.max.x.max(p.x),
                self.max.y.max(p.y),
                self.max.z.max(p.z),
            );
        }
    }
}

/// Cell-wise (internal field) diff mask: `true` where `field_i > mask_field_i`.
///
/// Matches upstream's **scalar specialisation** of `calcMask` — unsigned
/// difference, no `mag()` (see module docs for why this differs from the
/// generic vector/tensor template).
///
/// # Panics
/// If `field.internal.len() != mask_field.internal.len()`.
pub fn cell_mask(field: &VolScalarField, mask_field: &VolScalarField) -> Vec<bool> {
    assert_eq!(
        field.internal.len(),
        mask_field.internal.len(),
        "field/maskField cell count mismatch"
    );
    (0..field.internal.len())
        .map(|c| field.internal[c] > mask_field.internal[c])
        .collect()
}

/// Patch-face diff mask on boundary patch `patch_index`: `true` where
/// `field_i > mask_field_i` at that face. Same semantics as [`cell_mask`], on
/// a patch's boundary values instead of the internal field.
///
/// # Panics
/// If the two fields' face counts on `patch_index` disagree, or `patch_index`
/// is out of range for either field's boundary.
pub fn patch_mask(
    field: &VolScalarField,
    mask_field: &VolScalarField,
    patch_index: usize,
) -> Vec<bool> {
    let fv = field.boundary[patch_index].values.as_slice();
    let mv = mask_field.boundary[patch_index].values.as_slice();
    assert_eq!(
        fv.len(),
        mv.len(),
        "field/maskField patch face count mismatch"
    );
    (0..fv.len()).map(|i| fv[i] > mv[i]).collect()
}

/// Bounding-box extents of `{ C_i - reference_position : mask[i] }` over cell
/// centres.
///
/// If no cell satisfies the mask, returns a degenerate box at the origin of
/// the reference-shifted frame (`min == max == Vector3::ZERO`) — mirrors
/// upstream's `extents.add(point::zero)` fallback.
///
/// # Panics
/// If `mask.len() != mesh.n_cells`.
pub fn internal_extents(mesh: &FvMesh, mask: &[bool], reference_position: Vector3) -> Extents {
    assert_eq!(
        mask.len(),
        mesh.n_cells,
        "mask length must equal mesh.n_cells"
    );
    grow_extents(
        mask.iter()
            .enumerate()
            .filter(|(_, &m)| m)
            .map(|(c, _)| mesh.cell_centres[c] - reference_position),
    )
}

/// Bounding-box extents of `{ Cf_i - reference_position : mask[i] }` over the
/// face centres of boundary patch `patch_index`. Same empty-mask fallback as
/// [`internal_extents`].
///
/// # Panics
/// If `mask.len()` does not equal `mesh.patches[patch_index].size`, or
/// `patch_index` is out of range for `mesh.patches`.
pub fn patch_extents(
    mesh: &FvMesh,
    patch_index: usize,
    mask: &[bool],
    reference_position: Vector3,
) -> Extents {
    let patch = &mesh.patches[patch_index];
    assert_eq!(
        mask.len(),
        patch.size,
        "mask length must equal patch face count"
    );
    grow_extents(
        mask.iter()
            .enumerate()
            .filter(|(_, &m)| m)
            .map(|(fi, _)| mesh.face_centres[patch.start + fi] - reference_position),
    )
}

fn grow_extents(points: impl Iterator<Item = Vector3>) -> Extents {
    let mut ext = Extents::empty();
    let mut seeded = false;
    for p in points {
        ext.grow(p, &mut seeded);
    }
    ext
}

#[cfg(test)]
mod tests {
    use super::*;
    use outram_foam_basic_lib::fields::{BoundaryCondition, Field, PatchField, VolField};
    use outram_foam_basic_lib::mesh::{BoundaryPatch, FvMeshBuilder, PatchKind};
    use std::sync::Arc;

    /// Three cells on a line at x = 0.5, 1.5, 2.5 (no boundary faces needed
    /// for this test — internal-field-only check).
    fn three_cell_line_mesh() -> Arc<FvMesh> {
        Arc::new(
            FvMeshBuilder::new()
                .n_cells(3)
                .n_internal_faces(0)
                .owner(vec![])
                .neighbour(vec![])
                .patches(vec![])
                .cell_volumes(vec![1.0, 1.0, 1.0])
                .cell_centres(vec![
                    Vector3::new(0.5, 0.0, 0.0),
                    Vector3::new(1.5, 0.0, 0.0),
                    Vector3::new(2.5, 0.0, 0.0),
                ])
                .face_area_vectors(vec![])
                .face_centres(vec![])
                .build()
                .unwrap(),
        )
    }

    #[test]
    fn mask_selects_cells_where_field_exceeds_mask_field_unsigned() {
        let mesh = three_cell_line_mesh();
        let field = VolField::new(
            "T",
            Arc::clone(&mesh),
            Field::new(vec![10.0, 5.0, -1.0]),
            vec![],
        );
        let mask_field = VolField::new(
            "mask",
            Arc::clone(&mesh),
            Field::new(vec![3.0, 3.0, 3.0]),
            vec![],
        );

        let mask = cell_mask(&field, &mask_field);
        // 10.0 > 3.0 -> true; 5.0 > 3.0 -> true; -1.0 > 3.0 -> false.
        assert_eq!(mask, vec![true, true, false]);

        let ext = internal_extents(&mesh, &mask, Vector3::ZERO);
        assert!((ext.min.x - 0.5).abs() < 1e-12);
        assert!((ext.max.x - 1.5).abs() < 1e-12);
    }

    #[test]
    fn empty_mask_falls_back_to_degenerate_box_at_reference() {
        let mesh = three_cell_line_mesh();
        let mask = vec![false, false, false];
        let ext = internal_extents(&mesh, &mask, Vector3::new(1.0, 0.0, 0.0));
        assert_eq!(ext.min, Vector3::ZERO);
        assert_eq!(ext.max, Vector3::ZERO);
    }

    #[test]
    fn patch_mask_and_extents_use_face_centres() {
        let mesh = Arc::new(
            FvMeshBuilder::new()
                .n_cells(1)
                .n_internal_faces(0)
                .owner(vec![0, 0])
                .neighbour(vec![])
                .patches(vec![BoundaryPatch::new("wall", 0, 2, PatchKind::Wall)])
                .cell_volumes(vec![1.0])
                .cell_centres(vec![Vector3::new(0.5, 0.0, 0.0)])
                .face_area_vectors(vec![
                    Vector3::new(1.0, 0.0, 0.0),
                    Vector3::new(1.0, 0.0, 0.0),
                ])
                .face_centres(vec![
                    Vector3::new(0.0, 0.0, 0.0),
                    Vector3::new(2.0, 0.0, 0.0),
                ])
                .build()
                .unwrap(),
        );
        let field = VolField::new(
            "T",
            Arc::clone(&mesh),
            Field::uniform(1, 0.0),
            vec![PatchField {
                bc: BoundaryCondition::FixedField(Field::new(vec![10.0, 1.0])),
                values: Field::new(vec![10.0, 1.0]),
            }],
        );
        let mask_field = VolField::new(
            "mask",
            Arc::clone(&mesh),
            Field::uniform(1, 0.0),
            vec![PatchField {
                bc: BoundaryCondition::FixedField(Field::new(vec![5.0, 5.0])),
                values: Field::new(vec![5.0, 5.0]),
            }],
        );

        let mask = patch_mask(&field, &mask_field, 0);
        assert_eq!(mask, vec![true, false]);

        let ext = patch_extents(&mesh, 0, &mask, Vector3::ZERO);
        assert_eq!(ext.min, Vector3::new(0.0, 0.0, 0.0));
        assert_eq!(ext.max, Vector3::new(0.0, 0.0, 0.0));
    }
}
