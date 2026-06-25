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

//! Tutorial: rhoCentralFoam — 1-D Sod shock tube
//!
//! Case directory: `tutorials/cases/rho_central_foam_shock_tube/`
//!
//! ## How to generate reference data
//!
//! ```bash
//! cd tutorials/cases/rho_central_foam_shock_tube
//! blockMesh
//! rhoCentralFoam
//! ```
//!
//! This produces field files under `0.007/` (or whichever end time you set).
//! The test reads:
//!   - `constant/polyMesh/`   → mesh via `read_poly_mesh`
//!   - `0/U`, `0/p`, `0/T`   → initial conditions
//!   - `<endTime>/p`          → reference pressure profile from OpenFOAM
//!
//! ## Verification strategy
//!
//! The Sod shock tube (Sod 1978) has an exact Riemann solution consisting of
//! four constant states separated by a left rarefaction, contact discontinuity,
//! and right shock.  Two checks are made:
//!   1. Field comparison: max |p_rust − p_openfoam| / max |p_openfoam| < 2 %
//!      (tolerates KNP numerical diffusion at the shock).
//!   2. Wave-speed check: the shock position at t_end is within one cell width
//!      of the theoretical value x_shock = x0 + S_shock * t_end.
//!
//! ## TODO (before un-ignoring)
//!   - Drop `constant/polyMesh/` files into the case directory (run blockMesh)
//!   - Drop `0/U`, `0/p`, `0/T` initial-condition files
//!   - Drop the reference end-time directory (e.g. `0.007/p`, `0.007/U`)
//!   - Implement `read_poly_mesh` in `io::poly_mesh`
//!   - Implement initial field readers in `io::field_reader`
//!   - Fill in the `REFERENCE_*` constants below from the OpenFOAM run

use std::path::Path;

const CASE_DIR: &str = concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/tutorials/cases/rho_central_foam_shock_tube"
);

// ── Sod shock tube initial conditions (SI) ───────────────────────────────────
// Left state: high pressure driver
const P_LEFT:   f64 = 1.0e5;   // Pa
const RHO_LEFT: f64 = 1.0;     // kg/m³
// Right state: low pressure driven
const P_RIGHT:   f64 = 1.0e4;  // Pa
const RHO_RIGHT: f64 = 0.125;  // kg/m³
// Diaphragm position
const X_DIAPHRAGM: f64 = 5.0;  // m  (case-dependent — check blockMeshDict)
// End time (must match controlDict)
const T_END: f64 = 0.007;      // s

// ── Reference values from OpenFOAM run (fill in after running the case) ──────
// Set REFERENCE_AVAILABLE to true once the reference files are present.
const REFERENCE_AVAILABLE: bool = false;

// ── helpers ──────────────────────────────────────────────────────────────────

fn case_dir() -> &'static Path {
    Path::new(CASE_DIR)
}

fn poly_mesh_dir() -> std::path::PathBuf {
    case_dir().join("constant").join("polyMesh")
}

fn poly_mesh_present() -> bool {
    poly_mesh_dir().join("points").exists()
        && poly_mesh_dir().join("faces").exists()
        && poly_mesh_dir().join("owner").exists()
        && poly_mesh_dir().join("neighbour").exists()
        && poly_mesh_dir().join("boundary").exists()
}

// ── tests ─────────────────────────────────────────────────────────────────────

/// Smoke test: read the polyMesh and verify basic mesh consistency.
///
/// Requires: `constant/polyMesh/` files (run `blockMesh` to generate).
#[test]
#[ignore = "requires blockMesh-generated polyMesh in tutorials/cases/rho_central_foam_shock_tube/constant/polyMesh/"]
fn shock_tube_mesh_loads() {
    assert!(
        poly_mesh_present(),
        "polyMesh files missing — run `blockMesh` in {}",
        CASE_DIR
    );

    // TODO: uncomment once read_poly_mesh is implemented
    // let mesh = openfoam_appbuilder_lib::io::poly_mesh::read_poly_mesh(&poly_mesh_dir())
    //     .expect("polyMesh should load without error");
    // mesh.validate().expect("mesh should pass consistency checks");
    // assert!(mesh.n_cells > 0, "mesh must have at least one cell");
}

/// Field comparison: run rhoCentralFoam for T_END seconds and compare p, U
/// against the OpenFOAM reference solution at the same time.
///
/// Requires: polyMesh + initial fields + OpenFOAM reference end-time fields.
#[test]
#[ignore = "requires polyMesh, initial fields (0/), and OpenFOAM reference fields — see module doc"]
fn shock_tube_pressure_matches_openfoam() {
    assert!(REFERENCE_AVAILABLE, "set REFERENCE_AVAILABLE = true and fill in reference constants");

    // TODO: implement once read_poly_mesh and field readers are done
    //
    // let mesh = read_poly_mesh(&poly_mesh_dir()).unwrap();
    // let mut solver = RhoCentralFoam::new(mesh, control, schemes, solution);
    // // set initial conditions from 0/ files
    // solver.run().unwrap();
    // // load reference p from <T_END>/p
    // // assert max relative error < 2 %
}

/// Wave-speed check: the shock position at T_END should be within one cell
/// width of the theoretical value from the exact Riemann solution.
///
/// Requires: polyMesh + initial fields + completed solver run.
#[test]
#[ignore = "requires polyMesh, initial fields (0/), and a completed solver run — see module doc"]
fn shock_tube_shock_position() {
    // Exact Riemann solution for Sod tube with gamma = 1.4:
    // shock speed S ≈ 1.752 * sqrt(gamma * P_RIGHT / RHO_RIGHT) (approximate)
    // Theoretical shock position at T_END:
    let gamma = 1.4_f64;
    let c_right = (gamma * P_RIGHT / RHO_RIGHT).sqrt();
    let s_shock = 1.752 * c_right;  // approximate; use exact Riemann solver for tighter bound
    let _x_shock_theory = X_DIAPHRAGM + s_shock * T_END;

    // TODO: compare against the shock position found in the Rust solver output
    // (locate the cell where p drops from ~p_right_post_shock to p_right within one cell width)
}
