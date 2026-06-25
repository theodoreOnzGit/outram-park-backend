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

//! Tutorial: rhoPimpleFoam — NACA 0012 aerofoil (RAS, subsonic)
//!
//! Mirrors the OpenFOAM tutorial at:
//!   `tutorials/compressible/rhoPimpleFoam/RAS/aerofoilNACA0012/`
//!
//! Case directory: `tutorials/cases/rho_pimple_foam_aerofoil_naca0012/`
//!
//! ## Flow conditions
//!
//! | Quantity | Value |
//! |---|---|
//! | Inlet velocity U | (200, 0, 0) m/s |
//! | Outlet pressure p | 1×10⁵ Pa |
//! | Inlet temperature T | 298 K |
//! | Mach number (air, γ=1.4) | ≈ 0.58 (subsonic) |
//! | End time | 0.15 s |
//! | Δt | 2×10⁻⁵ s |
//! | Turbulence | k-ω SST (RAS) |
//!
//! ## How to generate reference data
//!
//! ```bash
//! cd tutorials/cases/rho_pimple_foam_aerofoil_naca0012
//! # copy all files from the OpenFOAM tutorial:
//! # tutorials/compressible/rhoPimpleFoam/RAS/aerofoilNACA0012/
//! blockMesh
//! extrudeMesh        # extrudes the 2-D mesh to one cell thick
//! rhoPimpleFoam
//! ```
//!
//! The reference fields live in the end-time directory (e.g. `0.15/p`, `0.15/U`).
//!
//! ## Verification strategy
//!
//! Since this is a RAS case with k-ω SST, there is no simple analytical
//! solution.  Verification is by field comparison against the OpenFOAM
//! reference run on the *same* mesh and with the *same* numerical schemes:
//!
//!   1. **Pressure coefficient Cp** on the aerofoil wall faces:
//!      Cp = (p − p_inf) / (0.5 · ρ_inf · U_inf²)
//!      Assert max |Cp_rust − Cp_openfoam| < 0.05 (absolute) at all wall faces.
//!
//!   2. **Lift coefficient CL** integrated over the wall patch:
//!      CL = (1/(0.5·ρ·U²·c)) · Σ_f  (p_f · Sf · ĵ)
//!      Assert |CL_rust − CL_openfoam| / |CL_openfoam| < 2 %.
//!
//!   3. **Mass conservation**: |Σ_f φ_f| / (ρ_inf · U_inf · A_inlet) < 1×10⁻⁶
//!      (global mass imbalance should be near machine precision).
//!
//! ## TODO (before un-ignoring)
//!   - Copy case files from the OpenFOAM tutorial directory and run the mesh steps
//!   - Drop `constant/polyMesh/` output into the case directory
//!   - Drop `0/` (or `0.orig/`) initial-condition files
//!   - Drop the reference end-time directory (`0.15/p`, `0.15/U`, etc.)
//!   - Implement `read_poly_mesh` in `io::poly_mesh`
//!   - Implement initial and result field readers in `io::field_reader`
//!   - Wire k-ω SST turbulence model into RhoPimpleFoam (currently a stub)
//!   - Fill in REFERENCE_CL below from the OpenFOAM run

use std::path::Path;

const CASE_DIR: &str = concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/tutorials/cases/rho_pimple_foam_aerofoil_naca0012"
);

// ── Flow conditions (must match 0.orig/ files) ────────────────────────────────
const U_INF: f64 = 200.0;   // m/s  (x-direction)
const P_INF: f64 = 1.0e5;   // Pa
const T_INF: f64 = 298.0;   // K
const GAMMA: f64 = 1.4;
const R_AIR: f64 = 287.058; // J/(kg·K)

// Derived
fn rho_inf() -> f64 { P_INF / (R_AIR * T_INF) }
fn c_inf()   -> f64 { (GAMMA * R_AIR * T_INF).sqrt() }
fn mach_inf() -> f64 { U_INF / c_inf() }
fn q_inf()   -> f64 { 0.5 * rho_inf() * U_INF * U_INF }

// Chord length [m] — must match blockMeshDict / extrudeMeshDict
const CHORD: f64 = 1.0;

// ── Reference values from the OpenFOAM run (fill in after running the case) ──
// Set REFERENCE_AVAILABLE = true and fill in REFERENCE_CL once the run is done.
const REFERENCE_AVAILABLE: bool = false;
const REFERENCE_CL: f64 = 0.0; // placeholder

// ── helpers ───────────────────────────────────────────────────────────────────

fn case_dir() -> &'static Path { Path::new(CASE_DIR) }

fn poly_mesh_present() -> bool {
    let pm = case_dir().join("constant").join("polyMesh");
    ["points", "faces", "owner", "neighbour", "boundary"]
        .iter()
        .all(|f| pm.join(f).exists())
}

// ── tests ─────────────────────────────────────────────────────────────────────

/// Smoke test: read the polyMesh and verify basic mesh consistency.
///
/// Requires: `constant/polyMesh/` (run `blockMesh && extrudeMesh`).
#[test]
#[ignore = "requires blockMesh+extrudeMesh output in tutorials/cases/rho_pimple_foam_aerofoil_naca0012/constant/polyMesh/"]
fn aerofoil_mesh_loads() {
    assert!(
        poly_mesh_present(),
        "polyMesh missing — run `blockMesh && extrudeMesh` in {}",
        CASE_DIR
    );
    // TODO: uncomment once read_poly_mesh is implemented
    // let mesh = openfoam_appbuilder_lib::io::poly_mesh::read_poly_mesh(
    //     &case_dir().join("constant").join("polyMesh"))
    //     .expect("polyMesh should load");
    // mesh.validate().expect("mesh consistency check");
    // assert!(mesh.n_cells > 0);
}

/// Pressure coefficient Cp on the aerofoil wall vs OpenFOAM reference.
///
/// Requires: polyMesh + initial fields + completed OpenFOAM run in 0.15/.
#[test]
#[ignore = "requires polyMesh, initial fields (0/), and OpenFOAM reference fields (0.15/) — see module doc"]
fn aerofoil_cp_matches_openfoam() {
    assert!(REFERENCE_AVAILABLE, "set REFERENCE_AVAILABLE = true and populate 0.15/ reference fields");

    // Cp = (p - p_inf) / q_inf
    let _ = (q_inf(), mach_inf(), CHORD);

    // TODO:
    // let mesh = read_poly_mesh(...).unwrap();
    // let mut solver = RhoPimpleFoam::new(mesh, control, schemes, solution);
    // // load 0/ initial conditions
    // solver.run().unwrap();
    // // load 0.15/p from OpenFOAM reference
    // // for each wall face: assert |Cp_rust - Cp_ref| < 0.05
}

/// Lift coefficient CL integrated over the wall patch vs OpenFOAM reference.
///
/// Requires: polyMesh + initial fields + completed solver run.
#[test]
#[ignore = "requires polyMesh, initial fields (0/), and a completed solver run — see module doc"]
fn aerofoil_cl_matches_openfoam() {
    assert!(REFERENCE_AVAILABLE, "set REFERENCE_AVAILABLE = true and fill in REFERENCE_CL");

    let _ = (REFERENCE_CL, rho_inf(), U_INF, CHORD);

    // TODO:
    // Integrate pressure over wall faces:
    //   CL = (1 / q_inf / CHORD) * Σ_f p_f * (Sf · ĵ)
    // Assert |CL_rust - REFERENCE_CL| / REFERENCE_CL < 0.02
}

/// Global mass conservation: net mass flux should be near zero.
///
/// Requires: polyMesh + completed solver run.
#[test]
#[ignore = "requires polyMesh, initial fields (0/), and a completed solver run — see module doc"]
fn aerofoil_mass_conservation() {
    let _ = (rho_inf(), U_INF);

    // TODO:
    // After solver.run(), sum phi over all boundary faces.
    // The net flux should satisfy:
    //   |Σ phi_f| / (rho_inf * U_INF * A_inlet) < 1e-6
}
