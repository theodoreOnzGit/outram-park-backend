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

//! Tutorial: pimpleFoam — lid-driven cavity (Re = 100)
//!
//! Case directory: `tutorials/cases/pimple_foam_cavity/`
//!
//! ## How to generate reference data
//!
//! ```bash
//! cd tutorials/cases/pimple_foam_cavity
//! blockMesh
//! pimpleFoam
//! ```
//!
//! ## Verification strategy
//!
//! The lid-driven cavity at Re = 100 has a well-known steady-state solution
//! (Ghia, Ghia & Shin 1982, tabulated U_x along the vertical centreline).
//!
//! Checks:
//!   1. Field comparison: max |U_rust − U_openfoam| / U_lid < 1 % at steady state.
//!   2. Ghia benchmark: U_x at the 17 tabulated y-positions matches to within
//!      2 % relative error (accounts for mesh resolution differences).
//!
//! ## TODO (before un-ignoring)
//!   - Drop polyMesh, 0/, and reference end-time files into the case directory
//!   - Implement `read_poly_mesh` and initial field readers
//!   - Fill in Ghia 1982 reference values (Re = 100 column)

use std::path::Path;

const CASE_DIR: &str = concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/tutorials/cases/pimple_foam_cavity"
);

// Lid velocity [m/s] and cavity side length [m] — must match blockMeshDict
const U_LID: f64 = 1.0;
const L: f64 = 1.0;
// Kinematic viscosity → Re = U_LID * L / NU = 100
const NU: f64 = 0.01;

// ── Ghia 1982 reference data, Re = 100, vertical centreline U_x ──────────────
// y/L positions (17 points) and corresponding U_x / U_lid
const GHIA_Y: [f64; 17] = [
    0.0000, 0.0547, 0.0625, 0.0703, 0.1016, 0.1719, 0.2813,
    0.4531, 0.5000, 0.6172, 0.7344, 0.8516, 0.9531, 0.9609,
    0.9688, 0.9766, 1.0000,
];
const GHIA_UX: [f64; 17] = [
     0.00000,  -0.03717, -0.04192, -0.04775, -0.06434, -0.10150, -0.15662,
    -0.21090,  -0.20581, -0.13641,  0.00332,  0.23151,  0.68717,  0.73722,
     0.78871,   0.84123,  1.00000,
];

fn case_dir() -> &'static Path { Path::new(CASE_DIR) }

fn poly_mesh_present() -> bool {
    let pm = case_dir().join("constant").join("polyMesh");
    ["points", "faces", "owner", "neighbour", "boundary"]
        .iter()
        .all(|f| pm.join(f).exists())
}

#[test]
#[ignore = "requires blockMesh-generated polyMesh in tutorials/cases/pimple_foam_cavity/constant/polyMesh/"]
fn cavity_mesh_loads() {
    assert!(poly_mesh_present(), "polyMesh files missing — run `blockMesh` in {}", CASE_DIR);
    // TODO: read_poly_mesh + validate
}

/// Steady-state field comparison against pimpleFoam reference output.
#[test]
#[ignore = "requires polyMesh, initial fields (0/), and pimpleFoam reference fields — see module doc"]
fn cavity_velocity_matches_openfoam() {
    // TODO: run PimpleFoam to steady state, compare U field
}

/// Ghia 1982 benchmark: U_x along the vertical centreline at Re = 100.
#[test]
#[ignore = "requires polyMesh, initial fields, and a completed steady-state run — see module doc"]
fn cavity_ghia_benchmark_re100() {
    // At each GHIA_Y[i], interpolate U_x from the Rust solver result.
    // Assert relative error < 2 % at all 17 points.
    for (y, ux_ref) in GHIA_Y.iter().zip(GHIA_UX.iter()) {
        let _ = (y, ux_ref, U_LID, L, NU); // suppress unused warnings
        // TODO: interpolate and compare
    }
}
