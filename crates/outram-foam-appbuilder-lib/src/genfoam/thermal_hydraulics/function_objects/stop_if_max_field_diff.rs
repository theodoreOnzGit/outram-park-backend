// SPDX-License-Identifier: GPL-3.0-only
// Copyright (C) 2026 OUTRAM PARK contributors
//
// Derived from GeN-Foam (Generalized Nuclear Foam)
//   Upstream: https://gitlab.com/foam-for-nuclear/GeN-Foam
//   Upstream source: src/classes/thermalHydraulics/src/functionObjects/stopIfMaxFieldDiff/
//   Upstream commit: 652b3da
//   Upstream copyright: (C) 2015-2022 EPFL; built on OpenFOAM v2506
//   Upstream license: GPL-3.0
//
// This file is part of OUTRAM PARK, distributed under GPL-3.0. See the crate
// root or <https://www.gnu.org/licenses/> for the full license text.

//! `stopIfMaxFieldDiff` — stop-criterion decision: true once
//! `max_cell(field1 - field2) > 0`.
//!
//! Port of `functionObjects::stopIfMaxFieldDiff` (`.C`/`.H`). Upstream's
//! `write()` calls `time.writeAndEnd()` directly when the criterion trips;
//! **this port only computes the decision** ([`StopDecision`]) — wiring a
//! stop signal into an actual run/timestep loop is a solver-driver concern,
//! deliberately out of scope for a post-processing function object (per the
//! porting brief: "return a bool/enum decision; don't wire into any run
//! loop"). The caller decides what to do with [`StopDecision::stop`].
//!
//! Compares only the **internal** field, matching upstream's
//! `max(field1-field2)` (an OpenFOAM `Foam::max` reduction over a
//! `GeometricField`'s internal field).

use outram_foam_basic_lib::fields::VolScalarField;

/// Outcome of evaluating the `stopIfMaxFieldDiff` criterion.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct StopDecision {
    /// `true` once `max_diff > 0.0` — upstream's trigger condition.
    pub stop: bool,
    /// `max_c (field1[c] - field2[c])` over all internal cells.
    pub max_diff: f64,
}

/// Evaluate the `stopIfMaxFieldDiff` criterion for `field1`/`field2`.
///
/// Matches upstream `stopIfMaxFieldDiff::write()`:
/// `if (max(field1-field2).value() > 0.0) { ... stop ... }`.
///
/// # Panics
/// If `field1.internal.len() != field2.internal.len()`, or either field has
/// zero cells (the max of an empty field is undefined).
pub fn evaluate_stop_if_max_field_diff(
    field1: &VolScalarField,
    field2: &VolScalarField,
) -> StopDecision {
    assert_eq!(
        field1.internal.len(),
        field2.internal.len(),
        "field1/field2 cell count mismatch"
    );
    assert!(
        !field1.internal.is_empty(),
        "fields must have at least one cell"
    );

    let max_diff = (0..field1.internal.len())
        .map(|c| field1.internal[c] - field2.internal[c])
        .fold(f64::NEG_INFINITY, f64::max);

    StopDecision {
        stop: max_diff > 0.0,
        max_diff,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use outram_foam_basic_lib::fields::{Field, VolField};
    use outram_foam_basic_lib::mesh::FvMeshBuilder;
    use outram_foam_basic_lib::primitives::Vector3;
    use std::sync::Arc;

    fn three_cell_mesh() -> Arc<outram_foam_basic_lib::mesh::FvMesh> {
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
    fn triggers_stop_when_field1_exceeds_field2_anywhere() {
        let mesh = three_cell_mesh();
        let f1 = VolField::new(
            "f1",
            Arc::clone(&mesh),
            Field::new(vec![1.0, 2.0, 3.0]),
            vec![],
        );
        let f2 = VolField::new(
            "f2",
            Arc::clone(&mesh),
            Field::new(vec![1.0, 5.0, 2.9]),
            vec![],
        );
        // cell 2: 3.0 - 2.9 = 0.1 > 0 -> stop.
        let decision = evaluate_stop_if_max_field_diff(&f1, &f2);
        assert!(decision.stop);
        assert!((decision.max_diff - 0.1).abs() < 1e-12);
    }

    #[test]
    fn does_not_trigger_when_field1_never_exceeds_field2() {
        let mesh = three_cell_mesh();
        let f1 = VolField::new(
            "f1",
            Arc::clone(&mesh),
            Field::new(vec![1.0, 2.0, 3.0]),
            vec![],
        );
        let f2 = VolField::new(
            "f2",
            Arc::clone(&mesh),
            Field::new(vec![1.0, 2.0, 3.0]),
            vec![],
        );
        // Equal fields: max diff = 0.0, not > 0.0 -> no stop.
        let decision = evaluate_stop_if_max_field_diff(&f1, &f2);
        assert!(!decision.stop);
        assert!(decision.max_diff.abs() < 1e-12);
    }
}
