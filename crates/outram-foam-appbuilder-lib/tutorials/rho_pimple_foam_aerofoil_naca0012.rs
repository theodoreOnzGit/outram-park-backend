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
//! ## Status (corrected 2026-08-07 — the previous note was wrong on two counts)
//!
//! `aerofoil_mesh_loads` is active and passing — the 16 000-cell extruded mesh
//! (with cellZones/faceZones, which the reader ignores) loads and validates.
//!
//! `aerofoil_cp_matches_openfoam`, `aerofoil_cl_matches_openfoam`, and
//! `aerofoil_mass_conservation` remain `#[ignore]`, but **not for the reason
//! previously recorded here.** That note said the k-ω SST model "is still a stub
//! in `RhoPimpleFoam`" and that "the verification bodies below are wired and
//! ready; only the turbulence model is missing". Both statements were false:
//!
//! 1. **k-ω SST is not a stub.** It is fully implemented and unit-tested in
//!    `outram-foam-turbulence-lib` (`k_omega_sst/mod.rs`, ~490 lines: F1/F2
//!    blending, the Bradshaw stress limiter, and the k and ω transport
//!    equations). It was merely never *connected* to any solver. It is now:
//!    `RhoPimpleFoam` carries a
//!    [`outram_foam_appbuilder_lib::turbulence::TurbulenceClosure`] field, and
//!    `tests/turbulence_coupling.rs` verifies the coupling against the analytic
//!    homogeneous-decay solution.
//! 2. **The verification bodies were never wired.** All three functions below
//!    are empty `let _ = (…);` placeholders followed by a `// TODO:` comment —
//!    they compute nothing and compare nothing. Un-ignoring them would have made
//!    three vacuous tests pass.
//!
//! **The real blockers, as of 2026-08-07:**
//!
//! - **No turbulence wall functions.** The closures use zero-gradient near-wall
//!   boundary conditions, so ω is never driven to its `6ν/(β y²)` near-wall
//!   asymptote and ν_t = k/ω is unbounded near a wall. Measured consequence
//!   (`tests/turbulence_coupling.rs`): a Re = 100 lid-driven cavity develops
//!   ν_t/ν ≈ 260–330. On this aerofoil, whose whole Cp/CL signature is a
//!   boundary-layer phenomenon, that is disqualifying — a Cp comparison run this
//!   way would be numerology, not verification.
//! - **The test bodies still have to be written**, including reading the
//!   OpenFOAM reference fields from `0.15/` (which *are* present in the case
//!   directory) and filling in `REFERENCE_CL`/`REFERENCE_AVAILABLE`.
//! - **Cost.** 16 000 cells × 7 500 steps (t_end = 0.15 s, Δt = 2e-5 s) is not a
//!   test-suite-sized run and will need its own harness or a coarsened case.
//!
//! Do not un-ignore these until a body actually asserts something and wall
//! functions exist.

use outram_foam_appbuilder_lib::io::poly_mesh::read_poly_mesh;
use outram_foam_basic_lib::prelude::PatchKind;
use std::path::Path;

const CASE_DIR: &str = concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/tutorials/cases/rho_pimple_foam_aerofoil_naca0012"
);

// ── Flow conditions (must match 0.orig/ files) ────────────────────────────────
const U_INF: f64 = 200.0; // m/s  (x-direction)
const P_INF: f64 = 1.0e5; // Pa
const T_INF: f64 = 298.0; // K
const GAMMA: f64 = 1.4;
const R_AIR: f64 = 287.058; // J/(kg·K)

// Derived
fn rho_inf() -> f64 {
    P_INF / (R_AIR * T_INF)
}
fn c_inf() -> f64 {
    (GAMMA * R_AIR * T_INF).sqrt()
}
fn mach_inf() -> f64 {
    U_INF / c_inf()
}
fn q_inf() -> f64 {
    0.5 * rho_inf() * U_INF * U_INF
}

// Chord length [m] — must match blockMeshDict / extrudeMeshDict
const CHORD: f64 = 1.0;

// ── Reference values from the OpenFOAM run (fill in after running the case) ──
// Set REFERENCE_AVAILABLE = true and fill in REFERENCE_CL once the run is done.
const REFERENCE_AVAILABLE: bool = false;
const REFERENCE_CL: f64 = 0.0; // placeholder

// ── helpers ───────────────────────────────────────────────────────────────────

fn case_dir() -> &'static Path {
    Path::new(CASE_DIR)
}

fn poly_mesh_present() -> bool {
    let pm = case_dir().join("constant").join("polyMesh");
    ["points", "faces", "owner", "neighbour", "boundary"]
        .iter()
        .all(|f| pm.join(f).exists())
}

// ── tests ─────────────────────────────────────────────────────────────────────

/// Smoke test: read the polyMesh and verify basic mesh consistency.
///
/// The extruded C-grid has 16 000 cells and ships with cellZones/faceZones/
/// pointZones, which the reader ignores (it reads points/faces/owner/neighbour/
/// boundary only).
#[test]
fn aerofoil_mesh_loads() {
    assert!(
        poly_mesh_present(),
        "polyMesh missing — run `blockMesh && extrudeMesh` in {CASE_DIR}"
    );
    let mesh = read_poly_mesh(&case_dir().join("constant").join("polyMesh"))
        .expect("polyMesh should load");
    mesh.validate().expect("mesh consistency check");
    assert_eq!(
        mesh.n_cells, 16_000,
        "extruded NACA0012 C-grid has 16 000 cells"
    );
    assert_eq!(mesh.n_internal_faces, 31_760);
    assert_eq!(
        mesh.patches.len(),
        5,
        "patches: aerofoil, inlet, outlet, back, front"
    );

    let aerofoil = mesh
        .patches
        .iter()
        .find(|p| p.name == "aerofoil")
        .expect("mesh must have an 'aerofoil' patch");
    assert_eq!(aerofoil.kind, PatchKind::Wall, "aerofoil patch is a wall");
    assert!(
        aerofoil.size > 0,
        "aerofoil wall must have faces to integrate Cp/CL over"
    );
}

/// Pressure coefficient Cp on the aerofoil wall vs OpenFOAM reference.
///
/// Requires: polyMesh + initial fields + completed OpenFOAM run in 0.15/.
#[test]
#[ignore = "test body is an unimplemented TODO (asserts nothing), and a boundary-layer Cp comparison additionally needs turbulence wall functions — see module doc"]
fn aerofoil_cp_matches_openfoam() {
    // Cp = (p - p_inf) / q_inf
    let _ = (q_inf(), mach_inf(), CHORD, REFERENCE_AVAILABLE);

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
#[ignore = "test body is an unimplemented TODO (asserts nothing), REFERENCE_CL is a placeholder, and CL needs turbulence wall functions — see module doc"]
fn aerofoil_cl_matches_openfoam() {
    let _ = (REFERENCE_CL, REFERENCE_AVAILABLE, rho_inf(), U_INF, CHORD);

    // TODO:
    // Integrate pressure over wall faces:
    //   CL = (1 / q_inf / CHORD) * Σ_f p_f * (Sf · ĵ)
    // Assert |CL_rust - REFERENCE_CL| / REFERENCE_CL < 0.02
}

/// Global mass conservation: net mass flux should be near zero.
///
/// Requires: polyMesh + completed solver run.
#[test]
#[ignore = "test body is an unimplemented TODO (asserts nothing); also needs a 16 000-cell x 7 500-step run, not a test-suite-sized case — see module doc"]
fn aerofoil_mass_conservation() {
    let _ = (rho_inf(), U_INF);

    // TODO:
    // After solver.run(), sum phi over all boundary faces.
    // The net flux should satisfy:
    //   |Σ phi_f| / (rho_inf * U_INF * A_inlet) < 1e-6
}
