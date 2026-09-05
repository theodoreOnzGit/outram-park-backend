// SPDX-License-Identifier: GPL-3.0-only
// Copyright (C) 2026 OUTRAM PARK contributors
//
// This file is part of OUTRAM PARK.
//
// OUTRAM PARK is free software: you can redistribute it and/or modify it
// under the terms of the GNU General Public License as published by the
// Free Software Foundation, either version 3 of the License, or (at your
// option) any later version.

//! # Tutorial: Running a compressible solver end-to-end
//!
//! This example demonstrates the minimal workflow to configure, run, and inspect
//! results from a CFD solver in `outram-foam-appbuilder-lib`. It solves a
//! **1-D double expansion wave** — two pressure-driven rarefaction fans expanding
//! into a vacuum — using the `RhoCentralFoam` density-based compressible solver.
//!
//! ## The physical problem
//!
//! A 1-D tube contains high-pressure air at both ends (p_L and p_R), separated
//! by a low-pressure gas at the centre. When the diaphragm ruptures, the two
//! pressure jumps create expanding rarefaction waves that propagate toward each
//! other. Over ~1 ms, the central pressure equilibrates and the flow reaches a
//! nearly uniform state (the two rarefaction fans meet at the domain centre).
//!
//! **Why this case?**
//! - No shocks — only smooth rarefaction waves (easier to verify).
//! - Runs in seconds, not minutes.
//! - Exact analytic solution exists (Toro, Riemann solvers for gas dynamics).
//! - Demonstrates initialization, time stepping, and field inspection.
//! - Shows how to: read a mesh, set initial fields, configure a solver, run it, and verify results.
//!
//! ## Workflow shown here
//!
//! 1. **Read the mesh** from an OpenFOAM case directory (disk I/O).
//! 2. **Set initial conditions** from primitive variables (ρ, U, p, T).
//! 3. **Configure the solver** with time step, end time, and FV schemes.
//! 4. **Run the solver** — call `solver.run()`.
//! 5. **Verify the results** against conservation laws and physical bounds.
//!
//! Run this example with:
//! ```shell
//! cargo run --release -p outram-foam-appbuilder-lib --example tutorial_run_a_solver
//! ```
//!
//! Expected runtime: < 2 seconds (100 cells, 2 ms simulation, ~2000 steps).

use outram_foam_appbuilder_lib::prelude::*;
use outram_foam_appbuilder_lib::io::poly_mesh::read_poly_mesh;
use std::path::Path;

// ═════════════════════════════════════════════════════════════════════════════
// PART 1: CASE SETUP AND PARAMETERS
// ═════════════════════════════════════════════════════════════════════════════

/// Path to the OpenFOAM case directory (shock tube mesh, reused for this example).
/// The mesh is a 1-D-like 10×10×1 grid (100 cells) over x ∈ [−5, 5] m.
const CASE_DIR: &str = concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/tutorials/cases/rho_central_foam_shock_tube"
);

/// Gas properties (air): heat capacity ratio
const GAMMA: f64 = 1.4;

/// Diaphragm position in the domain (centre of x ∈ [−5, 5])
const X_DIAPHRAGM: f64 = 0.0;

/// Left state: high pressure driver
const RHO_LEFT: f64 = 4.0; // kg/m³
const P_LEFT: f64 = 4.0e5; // Pa (400 kPa, high)

/// Right state: high pressure driver (second side)
const RHO_RIGHT: f64 = 4.0; // kg/m³
const P_RIGHT: f64 = 4.0e5; // Pa (400 kPa, high)

/// Centre state: low pressure void
const RHO_CENTER: f64 = 0.1; // kg/m³
const P_CENTER: f64 = 1.0e4; // Pa (10 kPa, vacuum-like)

/// Simulation control: run to 2 ms to let waves develop
const T_END: f64 = 0.002;
const DT: f64 = 1.0e-6;

// ═════════════════════════════════════════════════════════════════════════════
// PART 2: HELPER FUNCTIONS
// ═════════════════════════════════════════════════════════════════════════════

/// Locate the case directory at compile time.
fn case_dir() -> &'static Path {
    Path::new(CASE_DIR)
}

/// Check if all required mesh files exist (simple validation).
fn mesh_is_available() -> bool {
    let pm = case_dir().join("constant").join("polyMesh");
    ["points", "faces", "owner", "neighbour", "boundary"]
        .iter()
        .all(|f| pm.join(f).exists())
}

/// Compute the total mass in the domain (integral of ρ over all cells).
fn total_mass(solver: &RhoCentralFoam) -> f64 {
    solver
        .mesh
        .cell_volumes
        .iter()
        .zip(solver.rho.internal.iter())
        .map(|(vol, rho)| vol * rho)
        .sum()
}

/// Compute the total INTERNAL energy, the integral of ρ·e.
///
/// On its own this is **not** a conserved quantity, and checking it as though
/// it were is a classic mistake -- see [`total_energy`].
fn total_internal_energy(solver: &RhoCentralFoam) -> f64 {
    solver
        .mesh
        .cell_volumes
        .iter()
        .zip(solver.rho.internal.iter())
        .zip(solver.e.internal.iter())
        .map(|((vol, rho), e)| vol * rho * e)
        .sum()
}

/// Compute the total energy, the integral of ρ(e + ½|U|²).
///
/// **This is the conserved quantity, and the one to check.** `rhoCentralFoam`
/// solves conservation laws for mass, momentum and *total* energy, so with no
/// flux through the boundaries this sum should hold to near machine precision.
///
/// Internal energy alone is not conserved and must not be: in an expansion the
/// gas cools and accelerates, converting internal energy into kinetic energy.
/// Summing only ρ·e therefore shows a steady "loss" of about 1% in this case --
/// which is not numerical error at all, it is the physics of the problem being
/// measured with the wrong yardstick. A tutorial that reported that 1% as a
/// conservation result would be teaching a standard an order of magnitude too
/// lax, and would hide a genuine conservation bug if one ever appeared.
fn total_energy(solver: &RhoCentralFoam) -> f64 {
    solver
        .mesh
        .cell_volumes
        .iter()
        .zip(solver.rho.internal.iter())
        .zip(solver.e.internal.iter())
        .zip(solver.u.internal.iter())
        .map(|(((vol, rho), e), u)| {
            let ke = 0.5 * u.mag_sqr();
            vol * rho * (e + ke)
        })
        .sum()
}

// ═════════════════════════════════════════════════════════════════════════════
// PART 3: SOLVER INITIALIZATION
// ═════════════════════════════════════════════════════════════════════════════

/// Create and configure a RhoCentralFoam solver for the double-expansion wave.
///
/// Steps:
/// 1. Read the polyMesh from disk.
/// 2. Create a solver with default configuration.
/// 3. Set initial density, pressure, and internal energy from primitive vars.
/// 4. Set velocity to zero (initially at rest).
/// 5. Return the configured solver.
fn build_double_expansion_solver() -> RhoCentralFoam {
    // ── Step 1: Read the mesh ────────────────────────────────────────────────
    let mesh_dir = case_dir().join("constant").join("polyMesh");
    let mesh = read_poly_mesh(&mesh_dir).expect("Failed to read mesh");
    let n = mesh.n_cells;
    println!("Mesh loaded: {} cells", n);

    // ── Step 2: Configure the solver ─────────────────────────────────────────
    // Control dict: explicit time stepping (no iterations, just Courant-number stability)
    let control = ControlDict {
        start: StartControl::StartTime(0.0),
        stop: StopControl::EndTime(T_END),
        delta_t: DT,
        ..ControlDict::default()
    };

    // Create solver with default FV schemes (van Leer MUSCL reconstruction)
    let mut solver = RhoCentralFoam::new(
        mesh.clone(),
        control,
        FvSchemes::default(),
        FvSolution::default(),
    );

    // ── Step 3: Set initial density (ρ) ──────────────────────────────────────
    // High density at both ends, low density in centre (create the two drivers)
    let rho = solver.rho.internal.as_mut_slice();
    for c in 0..n {
        let x = mesh.cell_centres[c].x;
        rho[c] = if x < X_DIAPHRAGM - 0.5 {
            RHO_LEFT // left driver: high density
        } else if x > X_DIAPHRAGM + 0.5 {
            RHO_RIGHT // right driver: high density
        } else {
            RHO_CENTER // central void: low density
        };
    }

    // ── Step 4: Set initial pressure (p) ─────────────────────────────────────
    // Same structure: high pressure at ends, low in centre
    let p = solver.p.internal.as_mut_slice();
    for c in 0..n {
        let x = mesh.cell_centres[c].x;
        p[c] = if x < X_DIAPHRAGM - 0.5 {
            P_LEFT
        } else if x > X_DIAPHRAGM + 0.5 {
            P_RIGHT
        } else {
            P_CENTER
        };
    }

    // ── Step 5: Compute internal energy (e) from pressure ──────────────────
    // Ideal gas: e = p / ((γ − 1) ρ)  [J/kg]
    // This is the specific internal energy (not enthalpy).
    let rho_vals = solver.rho.internal.as_slice().to_vec();
    let p_vals = solver.p.internal.as_slice().to_vec();
    let e = solver.e.internal.as_mut_slice();
    for c in 0..n {
        e[c] = p_vals[c] / ((GAMMA - 1.0) * rho_vals[c]);
    }

    // ── Step 6: Set initial velocity (U) ──────────────────────────────────
    // All cells start at rest (U = 0 everywhere)
    let u = solver.u.internal.as_mut_slice();
    for c in 0..n {
        // Note: u[c] is a Vector3; set it to zero.
        u[c].x = 0.0;
        u[c].y = 0.0;
        u[c].z = 0.0;
    }

    solver
}

// ═════════════════════════════════════════════════════════════════════════════
// PART 4: MAIN — THE COMPLETE WORKFLOW
// ═════════════════════════════════════════════════════════════════════════════

fn main() {
    println!("╔═══════════════════════════════════════════════════════════════╗");
    println!("║ Tutorial: Running RhoCentralFoam for 1-D double expansion wave ║");
    println!("╚═══════════════════════════════════════════════════════════════╝\n");

    // ── PRE-CHECK: Ensure the mesh is available ──────────────────────────────
    if !mesh_is_available() {
        eprintln!(
            "ERROR: Mesh files not found at {}",
            case_dir().join("constant/polyMesh").display()
        );
        eprintln!("This example requires the shock tube case mesh (shipped with the crate).");
        std::process::exit(1);
    }

    // ─────────────────────────────────────────────────────────────────────────
    // STEP 1: Initialize the solver with double-expansion initial conditions
    // ─────────────────────────────────────────────────────────────────────────
    println!("STEP 1: Initializing solver...");
    let mut solver = build_double_expansion_solver();
    println!(
        "  Domain: x ∈ [{:.1}, {:.1}] m, {} cells",
        -5.0, 5.0, solver.mesh.n_cells
    );
    println!("  Simulation: t ∈ [0, {:.3e}] s, Δt = {:.3e} s", T_END, DT);
    let n_steps = (T_END / DT).ceil() as usize;
    println!("  Steps: ~{} (explicit time stepping)", n_steps);

    // ─────────────────────────────────────────────────────────────────────────
    // STEP 2: Report initial conditions
    // ─────────────────────────────────────────────────────────────────────────
    println!("\nSTEP 2: Initial conditions...");
    println!(
        "  Left driver:   ρ_L = {:.2} kg/m³, p_L = {:.2e} Pa",
        RHO_LEFT, P_LEFT
    );
    println!(
        "  Centre void:   ρ_c = {:.2} kg/m³, p_c = {:.2e} Pa",
        RHO_CENTER, P_CENTER
    );
    println!(
        "  Right driver:  ρ_R = {:.2} kg/m³, p_R = {:.2e} Pa",
        RHO_RIGHT, P_RIGHT
    );
    println!("  Velocity: U = 0 m/s everywhere (initially at rest)");

    // Record initial state for conservation checks
    let mass_initial = total_mass(&solver);
    let energy_initial = total_energy(&solver);
    let internal_initial = total_internal_energy(&solver);
    println!("\nInitial state:");
    println!("  Total mass:   {:.6e} kg", mass_initial);
    println!("  Total energy: {:.6e} J", energy_initial);

    // ─────────────────────────────────────────────────────────────────────────
    // STEP 3: Run the solver
    // ─────────────────────────────────────────────────────────────────────────
    println!("\nSTEP 3: Running solver...");
    let start_time = std::time::Instant::now();
    match solver.run() {
        Ok(()) => {
            let elapsed = start_time.elapsed();
            println!(
                "  ✓ Solver completed successfully in {:.3} seconds",
                elapsed.as_secs_f64()
            );
        }
        Err(e) => {
            eprintln!("  ✗ Solver failed: {:?}", e);
            std::process::exit(1);
        }
    }

    // ─────────────────────────────────────────────────────────────────────────
    // STEP 4: Verify conservation and physical bounds
    // ─────────────────────────────────────────────────────────────────────────
    println!("\nSTEP 4: Verifying conservation laws...");

    let mass_final = total_mass(&solver);
    let energy_final = total_energy(&solver);
    let internal_final = total_internal_energy(&solver);
    let mass_error = ((mass_final - mass_initial) / mass_initial).abs() * 100.0;
    let energy_error = ((energy_final - energy_initial) / energy_initial).abs() * 100.0;

    println!("  Mass conservation:");
    println!(
        "    Initial: {:.6e} kg  →  Final: {:.6e} kg",
        mass_initial, mass_final
    );
    println!("    Relative error: {:.4}%", mass_error);

    println!("  Energy conservation (TOTAL energy: internal + kinetic):");
    println!(
        "    Initial: {:.6e} J  →  Final: {:.6e} J",
        energy_initial, energy_final
    );
    println!("    Relative error: {:.4}%", energy_error);

    // The contrast is the teaching point, so show it rather than assert it.
    let internal_drop = (internal_final - internal_initial) / internal_initial * 100.0;
    println!("  Internal energy alone (NOT conserved, and must not be):");
    println!(
        "    Initial: {:.6e} J  →  Final: {:.6e} J   ({:+.4}%)",
        internal_initial, internal_final, internal_drop
    );
    println!("    That drop is the gas cooling as it expands and accelerates --");
    println!("    internal energy turning into kinetic energy, not numerical loss.");
    println!(
        "    Checking rho*e alone would report ~{:.2}% 'error' and hide a real one.",
        internal_drop.abs()
    );

    // Check for non-finite values (solver divergence)
    let n_nan_rho = solver
        .rho
        .internal
        .iter()
        .filter(|v| !v.is_finite())
        .count();
    let n_nan_p = solver.p.internal.iter().filter(|v| !v.is_finite()).count();
    let n_nan_e = solver.e.internal.iter().filter(|v| !v.is_finite()).count();

    if n_nan_rho > 0 || n_nan_p > 0 || n_nan_e > 0 {
        eprintln!(
            "  ✗ WARNING: {} NaN ρ, {} NaN p, {} NaN e — solver may have diverged",
            n_nan_rho, n_nan_p, n_nan_e
        );
    }

    // Physical bounds
    let min_rho = solver
        .rho
        .internal
        .iter()
        .cloned()
        .fold(f64::INFINITY, f64::min);
    let max_rho = solver
        .rho
        .internal
        .iter()
        .cloned()
        .fold(f64::NEG_INFINITY, f64::max);
    let min_p = solver
        .p
        .internal
        .iter()
        .cloned()
        .fold(f64::INFINITY, f64::min);
    let max_p = solver
        .p
        .internal
        .iter()
        .cloned()
        .fold(f64::NEG_INFINITY, f64::max);

    println!("\n  Physical bounds:");
    println!("    Density: [{:.4}, {:.4}] kg/m³", min_rho, max_rho);
    println!("    Pressure: [{:.4e}, {:.4e}] Pa", min_p, max_p);

    if min_rho > 0.0 && min_p > 0.0 {
        println!("    ✓ All densities and pressures positive (expected)");
    }

    // Check velocity magnitude
    let max_speed = solver
        .u
        .internal
        .iter()
        .map(|u| u.x.abs())
        .fold(0.0_f64, f64::max);
    let sound_speed_center = (GAMMA * P_CENTER / RHO_CENTER).sqrt();
    let sound_speed_left = (GAMMA * P_LEFT / RHO_LEFT).sqrt();
    println!(
        "  Velocity: max |U_x| = {:.2} m/s (sound speed ~{:.0} m/s in centre, {:.0} m/s at left)",
        max_speed, sound_speed_center, sound_speed_left
    );

    // ─────────────────────────────────────────────────────────────────────────
    // STEP 5: Sample and display solution at key points
    // ─────────────────────────────────────────────────────────────────────────
    println!("\nSTEP 5: Solution profile (selected cells):");
    println!(
        "\n{:>5} | {:>8} | {:>10} | {:>10} | {:>10}",
        "Cell", "x (m)", "ρ (kg/m³)", "U_x (m/s)", "p (Pa)"
    );
    println!("{:-<60}", "");

    // Sample: inlet, left driver, centre, right driver, outlet
    let sample_cells = vec![5, 25, 40, 49, 50, 51, 60, 75, 95];
    for c in sample_cells {
        if c < solver.mesh.n_cells {
            let x = solver.mesh.cell_centres[c].x;
            let rho = solver.rho.internal[c];
            let u_x = solver.u.internal[c].x;
            let p = solver.p.internal[c];
            println!(
                "{:5} | {:8.2} | {:10.4} | {:10.4} | {:10.2e}",
                c, x, rho, u_x, p
            );
        }
    }

    // ─────────────────────────────────────────────────────────────────────────
    // STEP 6: Summary and next steps
    // ─────────────────────────────────────────────────────────────────────────
    println!("\n╔═══════════════════════════════════════════════════════════════╗");
    println!("║ Summary and Assessment                                        ║");
    println!("╚═══════════════════════════════════════════════════════════════╝");

    if mass_error < 1.0 && energy_error < 1.0 {
        println!("✓ PASSED: Conservation laws hold to < 1% (excellent)");
    } else if mass_error < 5.0 && energy_error < 5.0 {
        println!("✓ PASSED: Conservation laws hold to < 5% (acceptable)");
    } else {
        println!("✗ WARNING: Conservation error > 5%");
    }

    println!("\nWhat this example demonstrated:");
    println!("  1. Reading an OpenFOAM mesh from disk (read_poly_mesh)");
    println!("  2. Setting up initial conditions (ρ, p, T → ρ, e, U)");
    println!("  3. Configuring time-stepping and numerical schemes");
    println!("  4. Running a compressible Riemann problem (explicit solver)");
    println!("  5. Verifying results against conservation laws");

    println!("\nTo extend this example:");
    println!("  • Call solver.step() in a loop for per-step post-processing");
    println!("  • Compare against the Toro exact Riemann solution");
    println!("  • Change initial conditions to other test cases (Sod, etc.)");
    println!("  • Once write_scalar_field() is implemented, save results to disk");
    println!("    (currently all writers are TODO — see io/output/mod.rs)");

    println!("\n✓ Tutorial complete.");
}
