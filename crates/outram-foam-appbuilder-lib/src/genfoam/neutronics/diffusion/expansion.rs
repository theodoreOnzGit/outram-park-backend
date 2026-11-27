// SPDX-License-Identifier: GPL-3.0-only
// Copyright (C) 2026 OUTRAM PARK contributors
//
// Derived from GeN-Foam (Generalized Nuclear Foam)
//   Upstream: https://gitlab.com/foam-for-nuclear/GeN-Foam
//   Upstream commit: 652b3da0cc55
//   Upstream source: src/classes/neutronics/XS/XS/include/setNeutronicsVariables.H (lines 85-115, the
//                    `radExp` and `axExp` branches), XS.C (`disp_`, `axialOrientation_`)
//   Upstream copyright: (C) 2015-2022 EPFL; built on OpenFOAM v2506
//   Upstream license: GPL-3.0
//
// This file is part of OUTRAM PARK (GPL-3.0-only, see the other files of this module).

//! The `disp`-driven expansion variables of `XS::correct()` (gh:#1898, README round 47): `radExp` and
//! `axExp` of the cross-section feedback, from a mesh displacement field and the fuel axial orientation.
//!
//! AI-written, not human-reviewed. Research, education and V&V only.
//!
//! Upstream (`setNeutronicsVariables.H`):
//!
//! * `radExp`: copy `disp_`, multiply component `i` by `1 - axialOrientation_[i]`,
//!   `correctBoundaryConditions()`, then `fvc::div(DNeutroRad)/2.0` (`divSchemes default Gauss linear`);
//! * `axExp`: the scalar `disp_ & axialOrientation_`, then `fvc::grad(...) & axialOrientation_`
//!   (`gradSchemes default Gauss linear`).
//!
//! The displacement is held with zero-gradient boundaries (upstream: `disp_` is read from the case if
//! present, with the file's boundary conditions; without a file it is zero, `zeroGradient`). Other `disp`
//! boundary types are not read here: the caller hands the cell values.

#[cfg(not(any(feature = "std", test)))]
#[allow(unused_imports)]
use crate::nostd_prelude::*;
use alloc::sync::Arc;

use outram_foam_basic_lib::fields::boundary::bc::PatchField;
use outram_foam_basic_lib::fields::field::Field;
use outram_foam_basic_lib::fields::vol_field::{VolScalarField, VolVectorField};
use outram_foam_basic_lib::fv_operators::fvc;
use outram_foam_basic_lib::mesh::fv_mesh::PatchKind;
use outram_foam_basic_lib::prelude::FvMesh;
use outram_foam_basic_lib::primitives::Vector3;

/// Per-cell expansion variables of one displacement field.
#[derive(Debug, Clone, PartialEq)]
pub struct ExpansionFields {
    /// `radExp`: `fvc::div(disp with the axial component removed)/2`.
    pub rad_exp: Vec<f64>,
    /// `axExp`: `fvc::grad(disp . axialOrientation) . axialOrientation`.
    pub ax_exp: Vec<f64>,
}

/// `radExp` and `axExp` of the cell displacement `disp` (one vector per cell) for the fuel axial
/// orientation `axial_orientation` (the `axialOrientation` entry of `nuclearData`, default `(0 0 1)`).
///
/// Translation of the two branches of `setNeutronicsVariables.H` (see the module documentation); the
/// `div` is `surfaceIntegrate(Sf & interpolate)` with `owner += / neighbour -=` in face order and the
/// boundary faces last, divided by the cell volume, as `fvc::surfaceIntegrate` does.
///
/// # Panics
///
/// If `disp.len() != mesh.n_cells`.
#[must_use]
pub fn expansion_from_displacement(mesh: &Arc<FvMesh>, disp: &[Vector3], axial_orientation: Vector3) -> ExpansionFields {
    assert_eq!(disp.len(), mesh.n_cells, "one displacement per cell");
    let boundary = || mesh.patches.iter().map(|p| PatchField::zero_gradient_vec(p.size)).collect();

    // radExp: DNeutroRad = disp with component i scaled by (1 - axialOrientation[i])
    let rad = VolVectorField::new(
        "disp",
        mesh.clone(),
        Field::from_fn(mesh.n_cells, |c| {
            let d = disp[c];
            Vector3::new(
                d.x * (1.0 - axial_orientation.x),
                d.y * (1.0 - axial_orientation.y),
                d.z * (1.0 - axial_orientation.z),
            )
        }),
        boundary(),
    );
    let rad_f = fvc::interpolate(&rad);
    let mut div = vec![0.0; mesh.n_cells];
    for f in 0..mesh.n_internal_faces {
        let flux = mesh.face_area_vectors[f].dot(rad_f.internal[f]);
        div[mesh.owner[f]] += flux;
        div[mesh.neighbour[f]] -= flux;
    }
    for (pi, patch) in mesh.patches.iter().enumerate() {
        if patch.kind == PatchKind::Empty {
            continue;
        }
        for fi in 0..patch.size {
            let gf = patch.start + fi;
            div[mesh.owner[gf]] += mesh.face_area_vectors[gf].dot(rad_f.boundary[pi].values[fi]);
        }
    }
    let rad_exp: Vec<f64> = (0..mesh.n_cells).map(|c| (div[c] / mesh.cell_volumes[c]) / 2.0).collect();

    // axExp: DNeutroAx = disp & axialOrientation, then grad(DNeutroAx) & axialOrientation
    let ax = VolScalarField::new(
        "disp",
        mesh.clone(),
        Field::from_fn(mesh.n_cells, |c| disp[c].dot(axial_orientation)),
        mesh.patches.iter().map(|p| PatchField::zero_gradient(p.size)).collect(),
    );
    let g = fvc::grad(&ax);
    let ax_exp: Vec<f64> = (0..mesh.n_cells).map(|c| g.internal[c].dot(axial_orientation)).collect();

    ExpansionFields { rad_exp, ax_exp }
}

/// The row-major `n_cells x n_variables` feedback-parameter matrix of [`super::XsEvolution::new`] for
/// the variables named in `variable_names` (the order of the `xsVariables` dictionary): the `radExp` and
/// `axExp` columns come from `expansion`, every other column from `others` (`(name, one value per cell)`).
///
/// # Panics
///
/// If a variable is neither `radExp`, `axExp` nor in `others`, or a column is not `n_cells` long.
#[must_use]
pub fn cell_parameters_with_expansion(
    variable_names: &[String],
    n_cells: usize,
    expansion: &ExpansionFields,
    others: &[(String, Vec<f64>)],
) -> Vec<f64> {
    let mut out = Vec::with_capacity(n_cells * variable_names.len());
    for c in 0..n_cells {
        for name in variable_names {
            out.push(match name.as_str() {
                "radExp" => expansion.rad_exp[c],
                "axExp" => expansion.ax_exp[c],
                other => others
                    .iter()
                    .find(|(n, _)| n == other)
                    .map(|(_, v)| v[c])
                    .unwrap_or_else(|| panic!("no values for the feedback variable {other}")),
            });
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use outram_foam_basic_lib::mesh::fv_mesh::{BoundaryPatch, FvMeshBuilder};

    /// Two cells along z (unit area, each 0.5 long): `disp = (0, 0, z)` gives `axExp` 1 (the gradient of
    /// the axial displacement) and `radExp` 0; `disp = (x, 0, 0)` with a z-slab mesh has no x-faces here,
    /// so a purely axial displacement checks the axial branch and the removal of the axial component.
    fn slab() -> Arc<FvMesh> {
        Arc::new(
            FvMeshBuilder::new()
                .n_cells(2)
                .n_internal_faces(1)
                .owner(vec![0, 1, 0])
                .neighbour(vec![1])
                .patches(vec![
                    BoundaryPatch::new("top", 1, 1, PatchKind::Wall),
                    BoundaryPatch::new("bottom", 2, 1, PatchKind::Wall),
                ])
                .cell_volumes(vec![0.5, 0.5])
                .cell_centres(vec![Vector3::new(0.0, 0.0, 0.25), Vector3::new(0.0, 0.0, 0.75)])
                .face_area_vectors(vec![
                    Vector3::new(0.0, 0.0, 1.0),
                    Vector3::new(0.0, 0.0, 1.0),
                    Vector3::new(0.0, 0.0, -1.0),
                ])
                .face_centres(vec![
                    Vector3::new(0.0, 0.0, 0.5),
                    Vector3::new(0.0, 0.0, 1.0),
                    Vector3::new(0.0, 0.0, 0.0),
                ])
                .build()
                .unwrap(),
        )
    }

    #[test]
    fn an_axial_displacement_has_no_radial_expansion_and_a_gradient_axial_one() {
        let mesh = slab();
        let disp = [Vector3::new(0.0, 0.0, 0.25), Vector3::new(0.0, 0.0, 0.75)];
        let e = expansion_from_displacement(&mesh, &disp, Vector3::new(0.0, 0.0, 1.0));
        assert_eq!(e.rad_exp, vec![0.0, 0.0]);
        // zero-gradient ends: the gradient of the two-cell field is (0.5 * 0.75 - 0.25 ... ) per cell
        assert!(e.ax_exp[0] > 0.0 && e.ax_exp[1] > 0.0);
        // mid-face value 0.5, ends equal the end cells: cell 0 (0.5 - 0.25)/0.5, cell 1 (0.75 - 0.5)/0.5
        assert!((e.ax_exp[0] - 0.5).abs() < 1e-15 && (e.ax_exp[1] - 0.5).abs() < 1e-15);
    }

    #[test]
    fn the_axial_orientation_selects_which_component_counts_as_radial() {
        let mesh = slab();
        let disp = [Vector3::new(0.0, 0.0, 0.25), Vector3::new(0.0, 0.0, 0.75)];
        // axis along x: the z displacement is "radial", its divergence along z is the radial expansion / 2
        let e = expansion_from_displacement(&mesh, &disp, Vector3::new(1.0, 0.0, 0.0));
        assert!((e.rad_exp[0] - 0.25).abs() < 1e-15 && (e.rad_exp[1] - 0.25).abs() < 1e-15);
        assert_eq!(e.ax_exp, vec![0.0, 0.0]);
    }

    #[test]
    fn the_parameter_matrix_is_row_major_in_the_dictionary_order() {
        let e = ExpansionFields { rad_exp: vec![1.0, 2.0], ax_exp: vec![3.0, 4.0] };
        let names = vec!["TFuel".to_string(), "axExp".to_string(), "radExp".to_string()];
        let m = cell_parameters_with_expansion(&names, 2, &e, &[("TFuel".to_string(), vec![900.0, 901.0])]);
        assert_eq!(m, vec![900.0, 3.0, 1.0, 901.0, 4.0, 2.0]);
    }
}
