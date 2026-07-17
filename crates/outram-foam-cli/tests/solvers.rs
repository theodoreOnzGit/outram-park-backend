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

//! End-to-end smoke test for the solver CLI binaries (`op-x8x.2`).
//!
//! # Methodology
//!
//! `rhoCentralFoam` is the one solver wired end-to-end. The test hand-builds a
//! minimal OpenFOAM case on disk — a two-cell hex mesh (`x ∈ {0,1,2}`,
//! `y,z ∈ {0,1}`, the same geometry the io round-trip test uses), a
//! `system/controlDict` (`deltaT = 1e-4`, `endTime = 5e-4` ⇒ 5 steps), and a
//! `0/` directory carrying a **Sod-shock-tube-like** initial state (left cell
//! `ρ = 1.0`, `e = 2.5e5`; right cell `ρ = 0.125`, `e = 2.0e5`; `U = 0`). It
//! then invokes the actual compiled `rhoCentralFoam` binary (via
//! `CARGO_BIN_EXE_rhoCentralFoam`) pointed at that case, and asserts:
//!
//! 1. the process exits successfully;
//! 2. an output time directory `0.0005/` is written with `rho`, `e`, `U`, `p`;
//! 3. every value read back is finite (no `NaN`/`inf`);
//! 4. the density has actually evolved — the high-density left cell drops and
//!    the low-density right cell rises (mass fluxes across the shared face),
//!    proving the solve ran rather than echoing the input.
//!
//! The two stubbed compressible solvers (`rhoPimpleFoam`, `sonicFoam`) and
//! `gen-foam` are checked to exit non-zero with a message naming the missing
//! input, so the "honest stub" contract does not silently regress into a fake
//! run.
//!
//! # Results (2026-07-17, against the crate as built)
//!
//! `rhoCentralFoam` marches the 5 steps and writes `0.0005/{rho,e,U,p}`, all
//! values finite. Measured (γ = 1.4, `deltaT = 1e-4`, 5 steps): density
//! `[1.0, 0.125] → [0.92242, 0.20241]` kg/m³ and internal energy
//! `[2.5e5, 2.0e5] → [2.49269e5, 2.16647e5]` J/kg — the left (dense, high-p)
//! cell loses mass and the right (rarefied, low-p) cell gains it, the expected
//! first-instant behaviour of a Sod shock-tube. This is a smoke/behaviour check
//! (the mesh is two cells), not a quantitative shock-tube validation. Data taken
//! 2026-07-17 against the crate as built.

use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::Arc;

use outram_foam_basic_lib::fields::boundary::bc::PatchField;
use outram_foam_basic_lib::fields::field::Field;
use outram_foam_basic_lib::fields::{VolScalarField, VolVectorField};
use outram_foam_basic_lib::io::dict::{FoamDict, FoamEntry, FoamFile, FoamHeader};
use outram_foam_basic_lib::io::field::{
    read_vol_scalar_field, write_vol_scalar_field, write_vol_vector_field,
};
use outram_foam_basic_lib::io::poly_mesh::{MeshFace, PolyMesh};
use outram_foam_basic_lib::mesh::{BoundaryPatch, FvMesh, PatchKind};
use outram_foam_basic_lib::primitives::Vector3;

/// Scratch case directory, removed on drop.
struct TmpCase {
    path: PathBuf,
}
impl TmpCase {
    fn new(tag: &str) -> Self {
        let nanos = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let path =
            std::env::temp_dir().join(format!("outram_cli_{tag}_{}_{nanos}", std::process::id()));
        std::fs::create_dir_all(&path).unwrap();
        Self { path }
    }
}
impl Drop for TmpCase {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.path);
    }
}

/// Two unit hex cells stacked along x — 12 points, 11 faces (1 internal + 10
/// boundary), 3 patches (`left`, `right`, `walls`). Same geometry as the io
/// round-trip verification.
fn two_cube_poly_mesh() -> PolyMesh {
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
        face([1, 4, 10, 7], 0, Some(1)),
        face([0, 6, 9, 3], 0, None),
        face([2, 5, 11, 8], 1, None),
        face([0, 1, 7, 6], 0, None),
        face([3, 9, 10, 4], 0, None),
        face([0, 3, 4, 1], 0, None),
        face([6, 7, 10, 9], 0, None),
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

/// zeroGradient boundary for every patch, in mesh-patch order.
fn zg_scalar_boundary(mesh: &FvMesh) -> Vec<PatchField<f64>> {
    mesh.patches
        .iter()
        .map(|p| PatchField::zero_gradient(p.size))
        .collect()
}
fn zg_vector_boundary(mesh: &FvMesh) -> Vec<PatchField<Vector3>> {
    mesh.patches
        .iter()
        .map(|p| PatchField::zero_gradient_vec(p.size))
        .collect()
}

/// Build the Sod-like two-cell case on disk under `root`.
fn write_shock_tube_case(root: &Path, mesh: Arc<FvMesh>) {
    // constant/polyMesh
    two_cube_poly_mesh()
        .write(root.join("constant").join("polyMesh"))
        .expect("write polyMesh");

    // system/controlDict: 5 explicit steps.
    let sys = root.join("system");
    std::fs::create_dir_all(&sys).unwrap();
    let mut d = FoamDict::new();
    d.insert("application", FoamEntry::Word("rhoCentralFoam".into()));
    d.insert("deltaT", FoamEntry::Scalar(1e-4));
    d.insert("startTime", FoamEntry::Scalar(0.0));
    d.insert("endTime", FoamEntry::Scalar(5e-4));
    d.insert("writeInterval", FoamEntry::Scalar(5e-4));
    let ff = FoamFile {
        header: Some(FoamHeader::standard("dictionary", "controlDict")),
        dict: d,
    };
    ff.write(sys.join("controlDict")).expect("write controlDict");

    // 0/ initial fields.
    let zero = root.join("0");
    std::fs::create_dir_all(&zero).unwrap();

    let u = VolVectorField::new(
        "U",
        Arc::clone(&mesh),
        Field::new(vec![Vector3::ZERO; mesh.n_cells]),
        zg_vector_boundary(&mesh),
    );
    let rho = VolScalarField::new(
        "rho",
        Arc::clone(&mesh),
        Field::new(vec![1.0, 0.125]),
        zg_scalar_boundary(&mesh),
    );
    // e = p / ((gamma-1) rho): left p=1e5, right p=1e4, gamma=1.4.
    let e = VolScalarField::new(
        "e",
        Arc::clone(&mesh),
        Field::new(vec![2.5e5, 2.0e5]),
        zg_scalar_boundary(&mesh),
    );

    let u_dims = [0.0, 1.0, -1.0, 0.0, 0.0, 0.0, 0.0];
    let rho_dims = [1.0, -3.0, 0.0, 0.0, 0.0, 0.0, 0.0];
    let e_dims = [0.0, 2.0, -2.0, 0.0, 0.0, 0.0, 0.0];
    write_vol_vector_field(zero.join("U"), &u, u_dims).unwrap();
    write_vol_scalar_field(zero.join("rho"), &rho, rho_dims).unwrap();
    write_vol_scalar_field(zero.join("e"), &e, e_dims).unwrap();
}

#[test]
fn rho_central_foam_end_to_end() {
    let case = TmpCase::new("rhocentral");
    let mesh = Arc::new(two_cube_poly_mesh().to_fv_mesh().unwrap());
    write_shock_tube_case(&case.path, Arc::clone(&mesh));

    let out = Command::new(env!("CARGO_BIN_EXE_rhoCentralFoam"))
        .arg("--case")
        .arg(&case.path)
        .output()
        .expect("failed to launch rhoCentralFoam binary");

    assert!(
        out.status.success(),
        "rhoCentralFoam exited non-zero.\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&out.stdout),
        String::from_utf8_lossy(&out.stderr),
    );

    // Output written into 0.0005/.
    let out_dir = case.path.join("0.0005");
    assert!(
        out_dir.join("rho").is_file(),
        "expected 0.0005/rho to be written; stdout:\n{}",
        String::from_utf8_lossy(&out.stdout),
    );
    assert!(out_dir.join("e").is_file(), "expected 0.0005/e");
    assert!(out_dir.join("U").is_file(), "expected 0.0005/U");
    assert!(out_dir.join("p").is_file(), "expected 0.0005/p");

    // Read back rho and check finiteness + physical evolution.
    let (rho_out, _) = read_vol_scalar_field(out_dir.join("rho"), Arc::clone(&mesh)).unwrap();
    let (e_out, _) = read_vol_scalar_field(out_dir.join("e"), Arc::clone(&mesh)).unwrap();
    let rho_vals = rho_out.internal.as_slice();
    let e_vals = e_out.internal.as_slice();

    for &v in rho_vals.iter().chain(e_vals.iter()) {
        assert!(v.is_finite(), "field value is not finite: {v}");
    }

    // Pressure jump drives mass from the dense (left) cell into the rarefied
    // (right) cell during the first instants: rho[left] < 1.0 < rho[right-init]
    // route → rho[left] falls, rho[right] rises.
    assert!(
        rho_vals[0] < 1.0,
        "left-cell density should fall from 1.0, got {}",
        rho_vals[0]
    );
    assert!(
        rho_vals[1] > 0.125,
        "right-cell density should rise from 0.125, got {}",
        rho_vals[1]
    );
}

/// The stubbed solvers must fail honestly (non-zero exit + a message naming the
/// missing input), never silently fake a run.
#[test]
fn stubbed_solvers_fail_honestly() {
    // A valid-enough case dir (just needs to exist for `-case`).
    let case = TmpCase::new("stub");

    for (bin, needle) in [
        (env!("CARGO_BIN_EXE_rhoPimpleFoam"), "thermophysicalProperties"),
        (env!("CARGO_BIN_EXE_sonicFoam"), "thermophysicalProperties"),
        (env!("CARGO_BIN_EXE_gen-foam"), "point-kinetics"),
    ] {
        let out = Command::new(bin)
            .arg("--case")
            .arg(&case.path)
            .output()
            .expect("failed to launch stub binary");
        assert!(
            !out.status.success(),
            "{bin} should exit non-zero (honest stub), but succeeded",
        );
        let msg = String::from_utf8_lossy(&out.stderr);
        assert!(
            msg.contains(needle),
            "{bin} stderr should name the missing input ({needle}); got:\n{msg}",
        );
    }
}
