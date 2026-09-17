// SPDX-License-Identifier: GPL-3.0-only
// Copyright (C) 2026 OUTRAM PARK contributors
//
// Derived from GeN-Foam (Generalized Nuclear Foam)
//   Upstream: https://gitlab.com/foam-for-nuclear/GeN-Foam
//   Upstream source: src/classes/thermalHydraulics/src/functionObjects/patchScalarFieldValue/
//   Upstream commit: 652b3da
//   Upstream copyright: (C) 2015-2022 EPFL; built on OpenFOAM v2506
//   Upstream license: GPL-3.0
//
// This file is part of OUTRAM PARK, distributed under GPL-3.0. See the crate
// root or <https://www.gnu.org/licenses/> for the full license text.

//! `patchScalarFieldValue` — per-face values of a scalar field on a boundary
//! patch, plus a selectable reduction over that patch.
//!
//! Literal upstream `functionObjects::patchScalarFieldValue`
//! (`patchScalarFieldValue.C`/`.H`) does **no reduction at all** — its
//! `write()` just dumps the raw per-face boundary values as an OpenFOAM list
//! (`"( t ( v0 v1 ... vn ))"`). [`patch_field_values`] ports that literally.
//!
//! This crate's porting brief additionally calls for a **selectable
//! reduction** (sum / average / min / max / integral) over the patch,
//! dispatched through a closed enum rather than upstream's raw dump —
//! [`PatchReduceOp`] and [`reduce_patch_scalar_field`] provide that. This is a
//! deliberate generalisation beyond upstream's `write()`, not a literal port
//! of it — documented here so the two are not confused.

use outram_foam_basic_lib::fields::VolScalarField;
use outram_foam_basic_lib::mesh::FvMesh;

/// Reduction applied over a boundary patch's face values by
/// [`reduce_patch_scalar_field`]. Closed enum — no `dyn` dispatch.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PatchReduceOp {
    /// Unweighted sum of face values: `sum(v)`.
    Sum,
    /// Area-weighted average: `sum(v * |Sf|) / sum(|Sf|)`.
    Average,
    /// Minimum face value.
    Min,
    /// Maximum face value.
    Max,
    /// Area integral: `sum(v * |Sf|)`.
    Integral,
}

/// Raw per-face boundary values of `field` on `patch_index`, in face order.
///
/// Literal port of upstream `patchScalarFieldValue::write()`'s output list
/// (minus the `"( t ( ... ))"` text framing, which is a file-format concern
/// out of scope for a pure computation).
///
/// # Panics
/// If `patch_index` is out of range for `field.boundary`.
pub fn patch_field_values(field: &VolScalarField, patch_index: usize) -> Vec<f64> {
    field.boundary[patch_index].values.as_slice().to_vec()
}

/// Reduce `field`'s values over boundary patch `patch_index` by `op`.
///
/// See [`PatchReduceOp`] for the exact formula per variant.
///
/// # Panics
/// If `patch_index` is out of range for `mesh.patches` / `field.boundary`, or
/// (`Min`/`Max`) the patch has zero faces (fold over an empty iterator
/// returns `+-infinity`, which is intentionally left un-guarded — an empty
/// patch is a caller configuration error, not a runtime condition to paper
/// over silently).
pub fn reduce_patch_scalar_field(
    mesh: &FvMesh,
    field: &VolScalarField,
    patch_index: usize,
    op: PatchReduceOp,
) -> f64 {
    let patch = &mesh.patches[patch_index];
    let vals = field.boundary[patch_index].values.as_slice();

    match op {
        PatchReduceOp::Sum => vals.iter().sum(),
        PatchReduceOp::Average => {
            let (num, den) = area_weighted_num_den(mesh, patch.start, vals);
            num / den
        }
        PatchReduceOp::Min => vals.iter().cloned().fold(f64::INFINITY, f64::min),
        PatchReduceOp::Max => vals.iter().cloned().fold(f64::NEG_INFINITY, f64::max),
        PatchReduceOp::Integral => area_weighted_num_den(mesh, patch.start, vals).0,
    }
}

/// `(sum(v*|Sf|), sum(|Sf|))` over `vals`, whose face `i` is global face
/// `patch_start + i`.
fn area_weighted_num_den(mesh: &FvMesh, patch_start: usize, vals: &[f64]) -> (f64, f64) {
    let mut num = 0.0;
    let mut den = 0.0;
    for (i, &v) in vals.iter().enumerate() {
        let a = mesh.face_areas[patch_start + i];
        num += v * a;
        den += a;
    }
    (num, den)
}

#[cfg(test)]
mod tests {
    use super::*;
    use outram_foam_basic_lib::fields::{BoundaryCondition, Field, PatchField, VolField};
    use outram_foam_basic_lib::mesh::{BoundaryPatch, FvMeshBuilder, PatchKind};
    use outram_foam_basic_lib::primitives::Vector3;
    use std::sync::Arc;

    /// One cell, three boundary faces (unit area each) all on patch
    /// "outlet", no internal faces — same fixture shape as
    /// `mass_flow::tests::single_cell_three_face_mesh`.
    fn single_cell_three_face_mesh() -> Arc<FvMesh> {
        Arc::new(
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
                .unwrap(),
        )
    }

    fn outlet_field(mesh: &Arc<FvMesh>, values: Vec<f64>) -> VolScalarField {
        VolField::new(
            "p",
            Arc::clone(mesh),
            Field::uniform(1, values[0]),
            vec![PatchField {
                bc: BoundaryCondition::FixedField(Field::new(values.clone())),
                values: Field::new(values),
            }],
        )
    }

    /// **V&V** — `patch_field_values` and every [`PatchReduceOp`] variant
    /// against hand-computed values on a tiny constructed mesh.
    ///
    /// Methodology: the 1-cell/3-boundary-face "outlet" patch fixture, unit
    /// area per face (so area-weighted reductions coincide with their
    /// unweighted counterparts, isolating the reduction logic from the area
    /// weighting already covered by `pressure_drop`'s and `mass_flow`'s V&V).
    /// Field values `[100, 200, 300]` (arbitrary scalar units — the pressure
    /// field is only illustrative here). Reference values by hand:
    /// `Sum = 100+200+300 = 600`, `Average = 600/3 = 200`, `Min = 100`,
    /// `Max = 300`, `Integral = sum(v*1) = 600` (numerically equal to `Sum`
    /// here because area = 1 per face — the formulas differ in general).
    /// Pass criterion: exact match to machine precision.
    ///
    /// Result (2026-07-15): `patch_field_values` returned `[100, 200, 300]`
    /// unchanged; `Sum = 600`, `Average = 200`, `Min = 100`, `Max = 300`,
    /// `Integral = 600` — all match the hand-computed references to
    /// `< 1e-12`.
    #[test]
    fn patch_scalar_value_reductions_match_hand_computed_values() {
        let mesh = single_cell_three_face_mesh();
        let p = outlet_field(&mesh, vec![100.0, 200.0, 300.0]);

        assert_eq!(patch_field_values(&p, 0), vec![100.0, 200.0, 300.0]);

        let sum = reduce_patch_scalar_field(&mesh, &p, 0, PatchReduceOp::Sum);
        let avg = reduce_patch_scalar_field(&mesh, &p, 0, PatchReduceOp::Average);
        let min = reduce_patch_scalar_field(&mesh, &p, 0, PatchReduceOp::Min);
        let max = reduce_patch_scalar_field(&mesh, &p, 0, PatchReduceOp::Max);
        let integral = reduce_patch_scalar_field(&mesh, &p, 0, PatchReduceOp::Integral);

        assert!((sum - 600.0).abs() < 1e-12, "Sum = {sum}");
        assert!((avg - 200.0).abs() < 1e-12, "Average = {avg}");
        assert!((min - 100.0).abs() < 1e-12, "Min = {min}");
        assert!((max - 300.0).abs() < 1e-12, "Max = {max}");
        assert!((integral - 600.0).abs() < 1e-12, "Integral = {integral}");
    }
}
