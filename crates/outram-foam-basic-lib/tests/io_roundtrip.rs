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

//! Round-trip verification for the OpenFOAM ASCII case I/O (`io` module).
//!
//! # Methodology
//!
//! Each test constructs an in-memory value (a [`FoamFile`] dictionary, a
//! [`PolyMesh`], or a [`VolScalarField`]/[`VolVectorField`]), serialises it to
//! OpenFOAM ASCII, parses the text back, and asserts the recovered value
//! equals the original. This verifies that the writer and parser are inverse
//! at the AST level (the project's I/O round-trip requirement). The polyMesh
//! test additionally checks the divergence-theorem geometry: a two-unit-cube
//! mesh must reconstruct a total volume of 2.0 m^3, confirming the face
//! windings and pyramid decomposition are consistent.
//!
//! # Results (2026-07-17, against the crate as built)
//!
//! - `dict_roundtrip`: header + nested sub-dicts + scalar/word/tokens/list/
//!   dimensioned entries recovered exactly (`FoamFile` `PartialEq`).
//! - `poly_mesh_roundtrip`: 12-point / 11-face / 2-cell mesh recovered
//!   exactly; `to_fv_mesh` validates and `total_volume() == 2.0` within
//!   1e-12 m^3.
//! - `field_scalar_roundtrip`: dimensions `[0 2 -2 0 0 0 0]`, a nonuniform
//!   internal field, and fixedValue / zeroGradient / nonuniform-fixedValue
//!   boundary patches all recovered exactly.
//! - `field_vector_roundtrip`: dimensions `[0 1 -1 0 0 0 0]`, a uniform
//!   internal field, and fixedValue / zeroGradient boundary patches recovered
//!   exactly.

use std::sync::Arc;

use outram_foam_basic_lib::fields::boundary::bc::{BoundaryCondition, PatchField};
use outram_foam_basic_lib::fields::field::Field;
use outram_foam_basic_lib::fields::{VolScalarField, VolVectorField};
use outram_foam_basic_lib::io::dict::{Dimensioned, FoamDict, FoamEntry, FoamFile, FoamHeader, FoamValue};
use outram_foam_basic_lib::io::poly_mesh::{MeshFace, PolyMesh};
use outram_foam_basic_lib::io::{
    read_vol_scalar_field, read_vol_vector_field, write_vol_scalar_field, write_vol_vector_field,
};
use outram_foam_basic_lib::mesh::{BoundaryPatch, FvMesh, PatchKind};
use outram_foam_basic_lib::primitives::Vector3;

/// Unique scratch directory for a test, cleaned up on drop.
struct TmpDir {
    path: std::path::PathBuf,
}
impl TmpDir {
    fn new(tag: &str) -> Self {
        let nanos = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let path =
            std::env::temp_dir().join(format!("outram_io_{tag}_{}_{nanos}", std::process::id()));
        std::fs::create_dir_all(&path).unwrap();
        Self { path }
    }
}
impl Drop for TmpDir {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.path);
    }
}

// ───────────────────────────────────────────────────────────────────────────
//  Dictionary round-trip
// ───────────────────────────────────────────────────────────────────────────

#[test]
fn dict_roundtrip() {
    let mut body = FoamDict::new();
    body.insert("application", FoamEntry::Word("icoFoam".into()));
    body.insert("startTime", FoamEntry::Scalar(0.0));
    body.insert("deltaT", FoamEntry::Scalar(0.005));
    body.insert(
        "nu",
        FoamEntry::Dimensioned(Dimensioned {
            dims: [0.0, 2.0, -1.0, 0.0, 0.0, 0.0, 0.0],
            value: vec![FoamValue::Scalar(1e-5)],
        }),
    );

    let mut div = FoamDict::new();
    div.insert("default", FoamEntry::Word("none".into()));
    div.insert(
        "div(phi,U)",
        FoamEntry::Tokens(vec![
            FoamValue::Word("Gauss".into()),
            FoamValue::Word("linearUpwind".into()),
            FoamValue::Word("grad(U)".into()),
        ]),
    );
    body.insert("divSchemes", FoamEntry::SubDict(div));

    body.insert(
        "vertices",
        FoamEntry::List(vec![
            FoamValue::Scalar(0.0),
            FoamValue::Scalar(1.0),
            FoamValue::Scalar(2.0),
        ]),
    );

    let ff = FoamFile {
        header: Some(FoamHeader::standard("dictionary", "controlDict")),
        dict: body,
    };

    let text = ff.to_foam_string();
    let parsed = FoamFile::parse_named(&text, "dict_roundtrip").expect("parse must succeed");
    assert_eq!(parsed, ff, "dictionary must round-trip exactly");

    // Also confirm the header survives.
    assert_eq!(parsed.header.as_ref().unwrap().class(), Some("dictionary"));
    assert_eq!(parsed.header.as_ref().unwrap().object(), Some("controlDict"));
}

// ───────────────────────────────────────────────────────────────────────────
//  polyMesh round-trip
// ───────────────────────────────────────────────────────────────────────────

/// Two unit hex cells stacked along x (`x ∈ {0,1,2}`, `y,z ∈ {0,1}`).
///
/// 12 points, 11 faces (1 internal shared face + 10 boundary), 2 cells,
/// 3 boundary patches (`left`, `right`, `walls`). Face windings follow the
/// OpenFOAM hex `cellModel`, so every face normal points owner→neighbour
/// (or outward for boundary faces) and the two cells each have volume 1.
fn two_cube_poly_mesh() -> PolyMesh {
    // idx(i,j,k) = i + 3j + 6k
    let mut points = Vec::with_capacity(12);
    for k in 0..2 {
        for j in 0..2 {
            for i in 0..3 {
                points.push(Vector3::new(i as f64, j as f64, k as f64));
            }
        }
    }

    let face = |verts: [usize; 4], owner: usize, neighbour: Option<usize>| MeshFace {
        verts: verts.to_vec(),
        owner,
        neighbour,
    };

    let faces = vec![
        // internal: cell0 x-max == cell1 x-min, normal +x (owner 0 -> neighbour 1)
        face([1, 4, 10, 7], 0, Some(1)),
        // patch "left": cell0 x-min
        face([0, 6, 9, 3], 0, None),
        // patch "right": cell1 x-max
        face([2, 5, 11, 8], 1, None),
        // patch "walls": cell0 y-min, y-max, z-min, z-max
        face([0, 1, 7, 6], 0, None),
        face([3, 9, 10, 4], 0, None),
        face([0, 3, 4, 1], 0, None),
        face([6, 7, 10, 9], 0, None),
        // patch "walls": cell1 y-min, y-max, z-min, z-max
        face([1, 2, 8, 7], 1, None),
        face([4, 10, 11, 5], 1, None),
        face([1, 4, 5, 2], 1, None),
        face([7, 8, 11, 10], 1, None),
    ];

    let patches = vec![
        BoundaryPatch::new("left", 1, 1, PatchKind::Patch),
        BoundaryPatch::new("right", 2, 1, PatchKind::Patch),
        BoundaryPatch::new("walls", 3, 8, PatchKind::Wall),
    ];

    PolyMesh {
        points,
        faces,
        n_internal_faces: 1,
        n_cells: 2,
        patches,
    }
}

#[test]
fn poly_mesh_roundtrip() {
    let pm = two_cube_poly_mesh();

    // Geometry sanity: divergence-theorem volume of two unit cubes.
    assert!(
        (pm.total_volume() - 2.0).abs() < 1e-12,
        "expected total volume 2.0, got {}",
        pm.total_volume()
    );
    let fv = pm.to_fv_mesh().expect("to_fv_mesh must validate");
    assert_eq!(fv.n_cells, 2);
    assert_eq!(fv.n_internal_faces, 1);
    assert_eq!(fv.n_faces, 11);

    // Write, read, assert equal.
    let tmp = TmpDir::new("polymesh");
    pm.write(&tmp.path).expect("write must succeed");
    let read = PolyMesh::read(&tmp.path).expect("read must succeed");
    assert_eq!(read, pm, "polyMesh must round-trip exactly");
}

// ───────────────────────────────────────────────────────────────────────────
//  Field round-trip
// ───────────────────────────────────────────────────────────────────────────

fn mesh_arc() -> Arc<FvMesh> {
    Arc::new(two_cube_poly_mesh().to_fv_mesh().unwrap())
}

#[test]
fn field_scalar_roundtrip() {
    let mesh = mesh_arc();

    // Nonuniform internal field, mixed boundary conditions.
    let internal = Field::new(vec![1.5, 2.5]);
    let walls_vals: Vec<f64> = (0..8).map(|i| 0.1 * i as f64).collect();
    let boundary = vec![
        PatchField::fixed_value(1, 5.0),  // left
        PatchField::zero_gradient(1),     // right
        PatchField {                      // walls: nonuniform fixedValue
            bc: BoundaryCondition::FixedField(Field::new(walls_vals.clone())),
            values: Field::new(walls_vals.clone()),
        },
    ];
    let field = VolScalarField::new("p", Arc::clone(&mesh), internal, boundary);
    let dims = [0.0, 2.0, -2.0, 0.0, 0.0, 0.0, 0.0];

    let tmp = TmpDir::new("scalarfield");
    let path = tmp.path.join("p");
    write_vol_scalar_field(&path, &field, dims).expect("write must succeed");
    let (read, read_dims) =
        read_vol_scalar_field(&path, Arc::clone(&mesh)).expect("read must succeed");

    assert_eq!(read_dims, dims, "dimensions must round-trip");
    assert_eq!(read.internal.as_slice(), field.internal.as_slice());
    assert_eq!(read.name, "p");

    // left: fixedValue 5
    match &read.boundary[0].bc {
        BoundaryCondition::FixedValue(v) => assert_eq!(*v, 5.0),
        other => panic!("left patch: expected FixedValue, got {other:?}"),
    }
    // right: zeroGradient
    assert!(matches!(read.boundary[1].bc, BoundaryCondition::ZeroGradient));
    // walls: nonuniform fixedValue -> FixedField
    match &read.boundary[2].bc {
        BoundaryCondition::FixedField(f) => assert_eq!(f.as_slice(), walls_vals.as_slice()),
        other => panic!("walls patch: expected FixedField, got {other:?}"),
    }
    assert_eq!(read.boundary[2].values.as_slice(), walls_vals.as_slice());
}

#[test]
fn field_vector_roundtrip() {
    let mesh = mesh_arc();

    // Uniform internal field, fixedValue + zeroGradient boundaries.
    let internal = Field::new(vec![Vector3::new(1.0, 0.0, 0.0); mesh.n_cells]);
    let boundary = vec![
        PatchField::fixed_value_vec(1, Vector3::new(0.0, 0.0, 0.0)), // left
        PatchField::zero_gradient_vec(1),                            // right
        PatchField::fixed_value_vec(8, Vector3::new(2.0, 0.0, 0.0)), // walls
    ];
    let field = VolVectorField::new("U", Arc::clone(&mesh), internal, boundary);
    let dims = [0.0, 1.0, -1.0, 0.0, 0.0, 0.0, 0.0];

    let tmp = TmpDir::new("vectorfield");
    let path = tmp.path.join("U");
    write_vol_vector_field(&path, &field, dims).expect("write must succeed");
    let (read, read_dims) =
        read_vol_vector_field(&path, Arc::clone(&mesh)).expect("read must succeed");

    assert_eq!(read_dims, dims, "dimensions must round-trip");
    assert_eq!(read.internal.as_slice(), field.internal.as_slice());

    match &read.boundary[0].bc {
        BoundaryCondition::FixedValue(v) => assert_eq!(*v, Vector3::new(0.0, 0.0, 0.0)),
        other => panic!("left patch: expected FixedValue, got {other:?}"),
    }
    assert!(matches!(read.boundary[1].bc, BoundaryCondition::ZeroGradient));
    match &read.boundary[2].bc {
        BoundaryCondition::FixedValue(v) => assert_eq!(*v, Vector3::new(2.0, 0.0, 0.0)),
        other => panic!("walls patch: expected FixedValue, got {other:?}"),
    }
}
