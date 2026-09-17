// SPDX-License-Identifier: GPL-3.0-only
// Copyright (C) 2026 OUTRAM PARK contributors
//
// Derived from GeN-Foam (Generalized Nuclear Foam)
//   Upstream: https://gitlab.com/foam-for-nuclear/GeN-Foam
//   Upstream source: src/classes/thermalHydraulics/src/functionObjects/fieldIntegralToFMU/
//   Upstream commit: 652b3da
//   Upstream copyright: (C) 2015-2022 EPFL; built on OpenFOAM v2506
//   Upstream license: GPL-3.0
//
// This file is part of OUTRAM PARK, distributed under GPL-3.0. See the crate
// root or <https://www.gnu.org/licenses/> for the full license text.

//! `fieldIntegralToFMU` — volume integral `sum_cells(field_c * V_c)`.
//!
//! Port of `functionObjects::fieldIntegralToFMU`'s core computation
//! (`fieldIntegralToFMU.C`/`.H`: `result = gSum(fieldZone * volZone)`).
//!
//! // TODO(genfoam): the FMU/co-simulation export plumbing
//! // (`commDataLayer::storeObj`/`getObj`, `externalIOObject`) is
//! // deliberately **not** ported — it is a network/process-boundary
//! // concern (FMI-standard scalar exchange with an external co-simulation
//! // master) with no analog in this crate. Only the volume-integral
//! // computation itself is ported; wiring its result out to an FMU is a
//! // separate, later concern (outside `outram-foam-appbuilder-lib`).
//!
//! The physical unit of the result is `[field units] * m^3` — generic to
//! whatever quantity the caller integrates, so this returns a plain `f64`
//! rather than a named `uom` quantity (there is no single named quantity that
//! covers every field this can be called with). Upstream's own usage example
//! integrates a field named `heatFlux.structure` into a `hxPowerOut` FMU
//! scalar; that only comes out as a power if `heatFlux.structure` is actually
//! a volumetric power density (W/m^3), not a surface flux (W/m^2) despite the
//! name — that unit choice is the caller's responsibility, not this
//! function's.

use outram_foam_basic_lib::fields::VolScalarField;
use outram_foam_basic_lib::mesh::FvMesh;

/// Volume integral of `field` over an explicit list of cell indices
/// (upstream's `cellZone`).
///
/// `sum_{c in cells}(field[c] * V[c])`.
///
/// # Panics
/// If any index in `cells` is out of range for `mesh.cell_volumes` /
/// `field.internal`.
pub fn volume_integral_over_cells(mesh: &FvMesh, field: &VolScalarField, cells: &[usize]) -> f64 {
    cells
        .iter()
        .map(|&c| field.internal[c] * mesh.cell_volumes[c])
        .sum()
}

/// Volume integral of `field` over the whole domain (all cells).
///
/// `sum_c(field[c] * V[c])`.
///
/// # Panics
/// If `field.internal.len() != mesh.n_cells`.
pub fn volume_integral(mesh: &FvMesh, field: &VolScalarField) -> f64 {
    assert_eq!(
        field.internal.len(),
        mesh.n_cells,
        "field cell count must match mesh.n_cells"
    );
    (0..mesh.n_cells)
        .map(|c| field.internal[c] * mesh.cell_volumes[c])
        .sum()
}

#[cfg(test)]
mod tests {
    use super::*;
    use outram_foam_basic_lib::fields::{Field, VolField};
    use outram_foam_basic_lib::mesh::FvMeshBuilder;
    use outram_foam_basic_lib::primitives::Vector3;
    use std::sync::Arc;

    /// Three cells with distinct volumes `[1.0, 2.0, 3.0]` m^3, no faces.
    fn three_cell_mesh() -> Arc<outram_foam_basic_lib::mesh::FvMesh> {
        Arc::new(
            FvMeshBuilder::new()
                .n_cells(3)
                .n_internal_faces(0)
                .owner(vec![])
                .neighbour(vec![])
                .patches(vec![])
                .cell_volumes(vec![1.0, 2.0, 3.0])
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
    fn volume_integral_matches_hand_computed_sum() {
        let mesh = three_cell_mesh();
        let field = VolField::new(
            "q",
            Arc::clone(&mesh),
            Field::new(vec![10.0, 20.0, 30.0]),
            vec![],
        );
        // sum(field*V) = 10*1 + 20*2 + 30*3 = 10 + 40 + 90 = 140.
        let total = volume_integral(&mesh, &field);
        assert!((total - 140.0).abs() < 1e-12);
    }

    #[test]
    fn volume_integral_over_cells_selects_a_subset() {
        let mesh = three_cell_mesh();
        let field = VolField::new(
            "q",
            Arc::clone(&mesh),
            Field::new(vec![10.0, 20.0, 30.0]),
            vec![],
        );
        // Only cells 0 and 2: 10*1 + 30*3 = 10 + 90 = 100.
        let total = volume_integral_over_cells(&mesh, &field, &[0, 2]);
        assert!((total - 100.0).abs() < 1e-12);
    }
}
