// SPDX-License-Identifier: GPL-3.0-only
// Copyright (C) 2026 OUTRAM PARK contributors
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

//! **VERIFICATION CASES 6, 7 and 8 — locking.**
//!
//! Case 6 measures **volumetric locking** at `nu = 0.499` against a
//! manufactured solution, on Quad4 (6a) and Hex8 (6b). Case 7 measures it
//! plastically, as the collapse pressure of a thick-walled cylinder against the
//! closed-form von Mises limit load. Case 8 measures **shear locking**, which is
//! a different mechanism that B-bar does **not** cure, so that the claim stops
//! being an unmeasured assertion.
//!
//! Every case is run twice — [`Formulation::FullIntegration`] and
//! [`Formulation::BBar`] — because the contrast is the evidence. A single good
//! number from B-bar would not show that anything had been fixed.
//!
//! Results are also collected in `docs/verification.md`.
//!
//! # Linear solver: Jacobi, not ILU(0), and why that is a finding
//!
//! Every other verification case in this crate preconditions with ILU(0). These
//! do not, because **ILU(0)-CG stagnates on the nearly-incompressible systems
//! here**: at `nu = 0.499` on the 32x32 Quad4 mesh it reached a relative
//! residual of 1.23 (full integration) and 0.153 (B-bar) after 20 000
//! iterations and stopped improving, while Jacobi-CG solved both to `1e-10`.
//! That is a preconditioner defect, not a formulation defect — the same B-bar
//! system that ILU(0) cannot solve converges cleanly under Jacobi — and it is
//! recorded as bead `op-ldaz` rather than worked around silently.

mod common;

use common::{elastic_newton_with, order};
use farrer_park::assembly::System;
use farrer_park::prelude::*;

/// Young's modulus used throughout, in pascals.
const E_PA: f64 = 200.0e9;
/// Yield stress of the limit-load case, in pascals.
const SIGMA_Y: f64 = 250.0e6;
/// Poisson's ratio of the limit-load case, dimensionless.
const NU_PLASTIC: f64 = 0.3;

fn options_for(formulation: Formulation) -> SystemOptions {
    SystemOptions {
        formulation,
        ..SystemOptions::default()
    }
}

/// Jacobi-preconditioned settings — see the module note on why not ILU(0).
fn locking_newton(linear_tolerance: f64, residual_tolerance: f64) -> NewtonSettings {
    let mut s = elastic_newton_with(linear_tolerance, residual_tolerance, 1.0e-6);
    s.linear.preconditioner = PreconditionerChoice::Jacobi;
    s.linear.max_iter = 500_000;
    s
}

// ── Case 6: nearly incompressible elasticity, manufactured solution ─────────

/// Solve the manufactured problem on one mesh with one formulation and return
/// `(L2 error, H1-seminorm error, dofs)`.
///
/// `three_d` selects [`BodyForce::ManufacturedSine3d`] on the unit cube over
/// [`BodyForce::ManufacturedSine`] on the unit square. Both exact fields vanish
/// on the whole boundary, so the Dirichlet data contributes no error of its own.
fn run_mms(mesh: Mesh, nu: f64, formulation: Formulation, three_d: bool) -> (f64, f64, usize) {
    let elastic = LinearElastic::new(E_PA, nu).unwrap();
    let (lambda, mu) = (elastic.lame_lambda(), elastic.shear_modulus());
    let bf = if three_d {
        BodyForce::ManufacturedSine3d(lambda, mu)
    } else {
        BodyForce::ManufacturedSine(lambda, mu)
    };
    let shared = mesh.shared();
    let dofs = DofMap::displacement(&shared);
    let mut system = System::with_options(
        shared.clone(),
        Material::Elastic(elastic),
        bf,
        options_for(formulation),
    )
    .unwrap();

    let mut bcs = DirichletSet::new();
    let boundary = shared.bounding_box_boundary_nodes(1e-10);
    bcs.fix_from_field(&shared, &dofs, &boundary, |x| {
        bf.manufactured_solution(x).expect("manufactured field").0
    });

    let forces = vec![0.0; system.n_dofs()];
    let (u, _) = solve_nonlinear(&mut system, &bcs, &forces, &locking_newton(1.0e-10, 1.0e-8))
        .unwrap_or_else(|e| panic!("nearly incompressible MMS solve, nu = {nu}: {e}"));
    let (l2, h1) = system.manufactured_errors(&u).expect("manufactured errors");
    (l2, h1, system.n_dofs())
}

/// Run one refinement sweep and return `(L2 errors, observed L2 orders)`.
fn mms_sweep(
    label: &str,
    nu: f64,
    formulation: Formulation,
    divisions: &[usize],
    build: fn(usize) -> Mesh,
    three_d: bool,
) -> (Vec<f64>, Vec<f64>) {
    println!("\n  {label}, nu = {nu}, {}", formulation.name());
    println!(
        "  {:>5} {:>8} {:>13} {:>7} {:>13}",
        "n", "dofs", "L2", "order", "H1"
    );
    let mut l2s: Vec<f64> = Vec::new();
    let mut orders = Vec::new();
    for (i, &n) in divisions.iter().enumerate() {
        let (l2, h1, nd) = run_mms(build(n), nu, formulation, three_d);
        let o = if i > 0 {
            let o = order(l2s[i - 1], l2, 2.0);
            orders.push(o);
            o
        } else {
            f64::NAN
        };
        println!("  {n:>5} {nd:>8} {l2:>13.5e} {o:>7.3} {h1:>13.5e}");
        l2s.push(l2);
    }
    (l2s, orders)
}

/// # Nearly incompressible elasticity on Quad4: full integration locks, B-bar does not
///
/// ## Methodology
///
/// The manufactured solution of verification case 2 — exact displacement
/// `u_x = u_y = sin(pi x) sin(pi y)` metres on the unit square, plane strain,
/// body force `-div sigma` evaluated exactly at every quadrature point by
/// [`BodyForce::ManufacturedSine`], homogeneous Dirichlet data on the whole
/// boundary — run at **two** Poisson's ratios and **two** formulations:
///
/// - `nu = 0.3` (`lambda / mu = 1.5`), the control, where nothing should lock;
/// - `nu = 0.499` (`lambda / mu = 332.7`), the nearly incompressible case.
///
/// Meshes `n = 4, 8, 16, 32` per side, so `h` halves each level and the
/// observed order is `log2(e_coarse / e_fine)`. `E = 200 GPa` throughout.
/// Errors are integrated with [`farrer_park::quadrature::error_rule`], richer
/// than the assembly rule, so the norm's own quadrature error is not what is
/// being measured. Linear solves by Jacobi-preconditioned conjugate gradients
/// to `1e-10` relative residual — six orders below the smallest discretisation
/// error in the table, and see the module note on why not ILU(0).
///
/// ## Pass criterion
///
/// Three claims, each asserted as an **order or a ratio**, never as a tuned
/// absolute tolerance:
///
/// 1. B-bar converges at the theoretical rate at `nu = 0.499`: observed L2
///    order within 0.15 of 2 on the finest pair.
/// 2. Full integration does **not**: observed L2 order below 1.5 somewhere in
///    the sweep, i.e. the rate is demonstrably degraded rather than merely the
///    constant being worse.
/// 3. B-bar's error is insensitive to `nu` — the defining property of a
///    locking-free element — within a factor of 2 between `nu = 0.3` and
///    `nu = 0.499` on the finest mesh, while full integration's error must be
///    at least 5 times larger at `nu = 0.499` than B-bar's.
///
/// ## Results, measured 2026-09-11 (release build, this machine)
///
/// L2 errors of the displacement, in metres times metre (the norm is not
/// normalised; the study takes ratios, for which the normalisation cancels).
///
/// **`nu = 0.3`, the control:**
///
/// | n | dofs | L2, full | order | L2, B-bar | order |
/// |---|---|---|---|---|---|
/// | 4 | 50 | 4.37275e-2 | | 3.03801e-2 | |
/// | 8 | 162 | 1.11518e-2 | 1.971 | 7.61116e-3 | 1.997 |
/// | 16 | 578 | 2.80446e-3 | 1.991 | 1.90510e-3 | 1.998 |
/// | 32 | 2178 | 7.02209e-4 | 1.998 | 4.76440e-4 | 1.999 |
///
/// **`nu = 0.499`:**
///
/// | n | dofs | L2, full | order | L2, B-bar | order |
/// |---|---|---|---|---|---|
/// | 4 | 50 | 4.79622e-2 | | 2.49554e-2 | |
/// | 8 | 162 | 2.03534e-2 | 1.237 | 5.88305e-3 | 2.085 |
/// | 16 | 578 | 1.25968e-2 | 0.692 | 1.45262e-3 | 2.018 |
/// | 32 | 2178 | 6.14098e-3 | 1.037 | 3.62084e-4 | 2.004 |
///
/// ## Interpretation
///
/// **Full integration locks, and it shows up in the rate, not just the
/// constant.** At `nu = 0.3` its observed order is 1.971, 1.991, 1.998 —
/// textbook second order. At `nu = 0.499` the same element gives 1.237, 0.692,
/// 1.037: the sequence is not converging at second order anywhere in this range
/// of `h`, and the wobble is the signature of a pre-asymptotic regime whose
/// error constant scales with `lambda / mu`. On the finest mesh its error is
/// **8.75 times** its own `nu = 0.3` error (6.141e-3 against 7.022e-4), for a
/// problem whose exact solution did not change at all.
///
/// **B-bar does not lock.** Observed orders 2.085, 2.018, 2.004 at
/// `nu = 0.499` — the theoretical rate, approached from above and settling on
/// it. Its finest-mesh error, 3.621e-4, is *smaller* than its own `nu = 0.3`
/// error of 4.764e-4: a factor of 0.76, so the error is essentially independent
/// of Poisson's ratio, which is exactly what "locking-free" means. (It is not a
/// contradiction that it falls slightly; the exact solution is fixed while the
/// material changes, so the two errors need not be ordered.)
///
/// **The contrast on the finest mesh is 16.96x** — full integration 6.141e-3
/// against B-bar 3.621e-4 — for the same mesh, the same solver, the same exact
/// solution and the same number of unknowns.
///
/// At `nu = 0.3` B-bar is also mildly *better* than full integration (4.764e-4
/// against 7.022e-4). That is not a general guarantee and should not be read as
/// one: B-bar is a different element, not a strictly better one, and on a
/// compressible material full integration remains the default for the
/// variational-cleanliness reason given on [`Formulation::FullIntegration`].
#[test]
fn nearly_incompressible_quad4_locks_under_full_integration_but_not_under_bbar() {
    let divisions = [4usize, 8, 16, 32];
    println!("\nCASE 6a: NEARLY INCOMPRESSIBLE ELASTICITY, Quad4, manufactured solution");
    let (full_03, ord_full_03) = mms_sweep(
        "Quad4",
        0.3,
        Formulation::FullIntegration,
        &divisions,
        |n| unit_square_quad4(n).unwrap(),
        false,
    );
    let (bbar_03, _) = mms_sweep(
        "Quad4",
        0.3,
        Formulation::BBar,
        &divisions,
        |n| unit_square_quad4(n).unwrap(),
        false,
    );
    let (full_05, ord_full_05) = mms_sweep(
        "Quad4",
        0.499,
        Formulation::FullIntegration,
        &divisions,
        |n| unit_square_quad4(n).unwrap(),
        false,
    );
    let (bbar_05, ord_bbar_05) = mms_sweep(
        "Quad4",
        0.499,
        Formulation::BBar,
        &divisions,
        |n| unit_square_quad4(n).unwrap(),
        false,
    );

    let fine = divisions.len() - 1;
    println!(
        "\n  finest mesh: full/B-bar error ratio at nu = 0.499 is {:.2}x; \
         B-bar nu-sensitivity {:.3}x; full-integration nu-sensitivity {:.2}x",
        full_05[fine] / bbar_05[fine],
        bbar_05[fine] / bbar_03[fine],
        full_05[fine] / full_03[fine]
    );

    // 1. The control must be clean for both formulations.
    let last_full_03 = *ord_full_03.last().unwrap();
    assert!(
        (last_full_03 - 2.0).abs() < 0.15,
        "the nu = 0.3 control must converge at order 2; full integration gave {last_full_03:.3}"
    );
    // 2. B-bar converges at the theoretical rate where the material is nearly
    //    incompressible.
    let last_bbar_05 = *ord_bbar_05.last().unwrap();
    assert!(
        (last_bbar_05 - 2.0).abs() < 0.15,
        "B-bar at nu = 0.499 must converge at order 2, got {last_bbar_05:.3}"
    );
    // 3. Full integration must be demonstrably degraded, not merely worse by a
    //    constant.
    let worst_full_05 = ord_full_05.iter().cloned().fold(f64::INFINITY, f64::min);
    assert!(
        worst_full_05 < 1.5,
        "full integration at nu = 0.499 should show a degraded RATE; \
         worst observed order was {worst_full_05:.3}, which is not degraded"
    );
    // 4. B-bar's error is nu-insensitive; full integration's is not.
    assert!(
        bbar_05[fine] / bbar_03[fine] < 2.0,
        "B-bar error should be insensitive to nu, ratio {:.3}",
        bbar_05[fine] / bbar_03[fine]
    );
    assert!(
        full_05[fine] / bbar_05[fine] > 5.0,
        "full integration should be far worse than B-bar at nu = 0.499, ratio {:.2}",
        full_05[fine] / bbar_05[fine]
    );
}

/// # Nearly incompressible elasticity on Hex8: the same result in three dimensions
///
/// ## Methodology
///
/// As case 6a, but on the unit cube with eight-node hexahedra and the
/// three-dimensional manufactured field
/// `u_x = u_y = u_z = sin(pi x) sin(pi y) sin(pi z)` metres, whose body force is
/// [`BodyForce::ManufacturedSine3d`]. That body force is checked independently
/// against `-div sigma` of its own exact field by
/// `assembly::tests::manufactured_body_forces_are_minus_divergence_of_their_own_stress`,
/// so a lost order here cannot be blamed on a mis-derived forcing term.
///
/// Meshes `n = 2, 4, 8, 16` divisions per side (81 to 14 739 degrees of
/// freedom). Pass criteria as case 6a.
///
/// ## Results, measured 2026-09-11
///
/// **`nu = 0.3`, the control:**
///
/// | n | dofs | L2, full | order | L2, B-bar | order |
/// |---|---|---|---|---|---|
/// | 2 | 81 | 1.61098e-1 | | 1.27103e-1 | |
/// | 4 | 375 | 4.10679e-2 | 1.972 | 2.74336e-2 | 2.212 |
/// | 8 | 2187 | 1.04584e-2 | 1.973 | 6.70978e-3 | 2.032 |
/// | 16 | 14739 | 2.62991e-3 | 1.992 | 1.67044e-3 | 2.006 |
///
/// **`nu = 0.499`:**
///
/// | n | dofs | L2, full | order | L2, B-bar | order |
/// |---|---|---|---|---|---|
/// | 2 | 81 | 1.61098e-1 | | 3.12185e-1 | |
/// | 4 | 375 | 4.78075e-2 | 1.753 | 4.49880e-2 | 2.795 |
/// | 8 | 2187 | 2.32814e-2 | 1.038 | 9.76937e-3 | 2.203 |
/// | 16 | 14739 | 1.50022e-2 | 0.634 | 2.35856e-3 | 2.050 |
///
/// ## Interpretation
///
/// The Hex8 result is the Quad4 result, harder. Full integration's observed
/// order falls monotonically — 1.753, 1.038, **0.634** — so on this mesh
/// sequence it is losing ground with refinement rather than gaining it, and its
/// finest-mesh error is **5.70 times** its own `nu = 0.3` error. B-bar gives
/// 2.795, 2.203, 2.050, converging onto the theoretical rate from above, and
/// its finest-mesh error is 1.41 times its `nu = 0.3` error — the same
/// `nu`-insensitivity as in two dimensions.
///
/// The contrast on the finest mesh is **6.36x** (1.500e-2 against 2.359e-3).
/// It is smaller than the 17x seen on Quad4 only because the Hex8 sweep does
/// not reach as small an `h`; the *rate* separation is wider here, not
/// narrower.
///
/// The one place B-bar looks bad is the coarsest mesh at `nu = 0.499`
/// (3.12185e-1 against full integration's 1.61098e-1). A 2x2x2 mesh of the unit cube
/// has two elements across a full sine wave, which resolves nothing; both
/// numbers are pre-asymptotic garbage and neither is evidence about the
/// element. It is left in the table rather than trimmed, because trimming the
/// row that flatters the conclusion is how a study stops being one.
#[test]
fn nearly_incompressible_hex8_locks_under_full_integration_but_not_under_bbar() {
    let divisions = [2usize, 4, 8, 16];
    println!("\nCASE 6b: NEARLY INCOMPRESSIBLE ELASTICITY, Hex8, manufactured solution");
    let build = |n: usize| unit_cube_hex8(n).unwrap();
    let (full_03, _) = mms_sweep("Hex8", 0.3, Formulation::FullIntegration, &divisions, build, true);
    let (bbar_03, _) = mms_sweep("Hex8", 0.3, Formulation::BBar, &divisions, build, true);
    let (full_05, ord_full_05) =
        mms_sweep("Hex8", 0.499, Formulation::FullIntegration, &divisions, build, true);
    let (bbar_05, ord_bbar_05) =
        mms_sweep("Hex8", 0.499, Formulation::BBar, &divisions, build, true);

    let fine = divisions.len() - 1;
    println!(
        "\n  finest mesh: full/B-bar error ratio at nu = 0.499 is {:.2}x; \
         B-bar nu-sensitivity {:.3}x; full-integration nu-sensitivity {:.2}x",
        full_05[fine] / bbar_05[fine],
        bbar_05[fine] / bbar_03[fine],
        full_05[fine] / full_03[fine]
    );

    let last_bbar_05 = *ord_bbar_05.last().unwrap();
    assert!(
        (last_bbar_05 - 2.0).abs() < 0.15,
        "B-bar Hex8 at nu = 0.499 must converge at order 2, got {last_bbar_05:.3}"
    );
    let last_full_05 = *ord_full_05.last().unwrap();
    assert!(
        last_full_05 < 1.5,
        "full-integration Hex8 at nu = 0.499 should show a degraded RATE, got {last_full_05:.3}"
    );
    assert!(
        bbar_05[fine] / bbar_03[fine] < 2.0,
        "B-bar Hex8 error should be insensitive to nu, ratio {:.3}",
        bbar_05[fine] / bbar_03[fine]
    );
    assert!(
        full_05[fine] / bbar_05[fine] > 4.0,
        "full integration should be far worse than B-bar at nu = 0.499, ratio {:.2}",
        full_05[fine] / bbar_05[fine]
    );
}

// ── Case 7: fully plastic limit load ────────────────────────────────────────

/// Closed-form von Mises plane-strain limit pressure of a thick-walled cylinder
/// of an elastic-perfectly-plastic material, in pascals:
///
/// `p_L = (2 / sqrt(3)) sigma_y ln(b / a)`
///
/// `a` and `b` are the inner and outer radii in metres, `sigma_y` the yield
/// stress in pascals. Derived by integrating the fully plastic equilibrium
/// equation `d sigma_r / dr = (sigma_theta - sigma_r) / r` with the plane-strain
/// von Mises condition `sigma_theta - sigma_r = 2 sigma_y / sqrt(3)` from
/// `sigma_r(b) = 0` to `sigma_r(a) = -p`.
fn limit_pressure(a: f64, b: f64, sigma_y: f64) -> f64 {
    2.0 / 3.0_f64.sqrt() * sigma_y * (b / a).ln()
}

/// Displacement-controlled thick-walled cylinder: prescribe the bore radial
/// displacement and read the pressure back out of the support reaction.
///
/// Returns `(u_r in metres, equivalent bore pressure in pascals, yielding
/// quadrature points)` at each requested displacement level, in order.
///
/// # Why displacement control
///
/// Under *force* control the problem has a limit point: at `p_L` the tangent
/// stiffness of an elastic-perfectly-plastic structure loses definiteness and
/// Newton diverges, so the measured collapse load would be "the last pressure
/// at which the solver happened to converge", which depends on the tolerance as
/// much as on the mechanics. Under displacement control the equilibrium path
/// stays single-valued and the pressure simply plateaus, which is a measurement
/// rather than a failure.
///
/// # How the pressure is recovered
///
/// At convergence the internal force at a constrained degree of freedom is the
/// support reaction. Summing its radial component over the bore nodes gives the
/// total radial force the support applies, and dividing by the **polygon**
/// perimeter of the discrete bore — `n_theta * 2 a sin(pi / (4 n_theta))`, not
/// the arc length `pi a / 2` — gives the equivalent pressure. The elements are
/// straight-sided, so the polygon is the boundary the load actually acts on;
/// using the arc would introduce a 0.05 % bias at `n_theta = 16` in the
/// convenient direction.
///
/// # Path
///
/// Each level is solved as a single load step from zero displacement against
/// the **committed** history of the previous level, which is the same
/// incremental scheme `tests/plasticity.rs`'s uniaxial cell uses and is
/// legitimate for rate-independent J2 under monotonic loading: the end-of-step
/// stress depends only on the committed state and the end-of-step strain.
fn run_limit_load(
    formulation: Formulation,
    n_r: usize,
    n_theta: usize,
    a: f64,
    b: f64,
    levels: &[f64],
) -> Vec<(f64, f64, usize)> {
    let mesh = quarter_annulus_quad4(a, b, n_r, n_theta).unwrap().shared();
    let dofs = DofMap::displacement(&mesh);
    let material = Material::j2_linear_hardening(E_PA, NU_PLASTIC, SIGMA_Y, 0.0).unwrap();
    let mut system = System::with_options(
        mesh.clone(),
        material,
        BodyForce::None,
        options_for(formulation),
    )
    .unwrap();

    let bore = mesh.nodes_where(|c| ((c[0] * c[0] + c[1] * c[1]).sqrt() - a).abs() < 1e-12);
    assert_eq!(bore.len(), n_theta + 1, "bore node count");
    let perimeter = n_theta as f64 * 2.0 * a * (std::f64::consts::FRAC_PI_4 / n_theta as f64).sin();

    let settings = NewtonSettings {
        max_iterations: 60,
        residual_tolerance: 1.0e-9,
        increment_tolerance: 1.0e-7,
        n_load_steps: 1,
        max_cutbacks: 0,
        dirichlet_method: DirichletMethod::Elimination,
        linear: LinearSolverSettings {
            method: KrylovMethod::ConjugateGradient,
            preconditioner: PreconditionerChoice::Jacobi,
            tolerance: 1.0e-11,
            max_iter: 200_000,
            restart: 50,
        },
    };

    let mut out = Vec::new();
    for &ur in levels {
        let mut bcs = DirichletSet::new();
        for n in mesh.nodes_where(|c| c[1].abs() < 1e-12) {
            bcs.fix(&dofs, n, 1, 0.0);
        }
        for n in mesh.nodes_where(|c| c[0].abs() < 1e-12) {
            bcs.fix(&dofs, n, 0, 0.0);
        }
        for &n in &bore {
            let c = mesh.node(n);
            let r = (c[0] * c[0] + c[1] * c[1]).sqrt();
            bcs.fix(&dofs, n, 0, ur * c[0] / r);
            bcs.fix(&dofs, n, 1, ur * c[1] / r);
        }
        let forces = vec![0.0; system.n_dofs()];
        let (u, _) = solve_nonlinear(&mut system, &bcs, &forces, &settings)
            .unwrap_or_else(|e| panic!("limit-load step to u_r = {ur:e} m: {e}"));
        let asm = system.assemble(&u, None).unwrap();
        let mut radial = 0.0;
        for &n in &bore {
            let c = mesh.node(n);
            let r = (c[0] * c[0] + c[1] * c[1]).sqrt();
            radial +=
                asm.internal_force[n.0 * 2] * c[0] / r + asm.internal_force[n.0 * 2 + 1] * c[1] / r;
        }
        out.push((ur, radial / perimeter, asm.n_yielding));
    }
    out
}

/// # Fully plastic collapse of a thick-walled cylinder, against the closed-form limit load
///
/// ## Methodology
///
/// Geometry: inner radius `a = 0.05 m`, outer `b = 0.10 m`, quarter model with
/// symmetry conditions on both cut faces (`u_y = 0` on `theta = 0`, `u_x = 0`
/// on `theta = pi/2`). Material: **elastic-perfectly-plastic** J2,
/// `E = 200 GPa`, `nu = 0.3`, `sigma_y = 250 MPa`, hardening modulus `H = 0`
/// exactly, because the closed form assumes perfect plasticity and a positive
/// `H` would make the structure carry more load for reasons that have nothing
/// to do with the element formulation.
///
/// Loading: the bore radial displacement is ramped in 32 equal increments of
/// `2.5e-5 m` to `8.0e-4 m` (16 times the first-yield bore displacement of
/// `5.16e-5 m`, and `u_r / a = 1.6 %` — at the outer edge of what a small-strain
/// model should be asked to do; see the limitations note below). At each level
/// the equivalent bore pressure is recovered from the support reaction, as
/// documented on [`run_limit_load`].
///
/// Meshes: `n_r = 2, 4, 8, 16` radial by `n_theta = 2 n_r` circumferential
/// Quad4, plane strain, each run twice — full integration and B-bar.
///
/// **Reference:** `p_L = (2 / sqrt(3)) sigma_y ln(b / a) = 200.0944 MPa`
/// (see [`limit_pressure`]).
///
/// ## Pass criterion
///
/// Ratios, not tolerances:
///
/// 1. Both formulations must **over**-predict at every mesh level. A
///    displacement-based finite element admits only kinematically admissible
///    fields, so by the upper-bound theorem of limit analysis its collapse load
///    cannot fall below the exact one; a number under `p_L` would mean
///    something is wrong, not that the mesh is good.
/// 2. B-bar's over-prediction must be at least **5 times smaller** than full
///    integration's at every level.
/// 3. B-bar on the finest mesh must be within **0.5 %** of `p_L`.
///
/// ## Results, measured 2026-09-11 (release build, this machine)
///
/// Collapse pressure at `u_r = 8.0e-4 m`, in megapascals, against
/// `p_L = 200.0944 MPa`:
///
/// | n_r x n_theta | dofs | full integration | p/p_L | B-bar | p/p_L | over-prediction ratio |
/// |---|---|---|---|---|---|---|
/// | 2 x 4 | 30 | 247.8937 | 1.23888 | 202.2194 | 1.01062 | 22.5x |
/// | 4 x 8 | 90 | 213.1640 | 1.06532 | 200.6581 | 1.00282 | 23.2x |
/// | 8 x 16 | 306 | 203.4431 | 1.01674 | 200.2354 | 1.00070 | 23.7x |
/// | 16 x 32 | 1122 | 200.9348 | 1.00420 | 200.1275 | 1.00017 | 25.4x |
///
/// The last column is `(p_full - p_L) / (p_bbar - p_L)`.
///
/// Pressure-displacement path on the `10 x 20` mesh, showing the plateau the
/// measurement relies on (full integration / B-bar, in MPa):
///
/// | u_r (m) | full | B-bar | yielding points |
/// |---|---|---|---|
/// | 5.0e-5 | 105.036 | 104.929 | 0 (still elastic) |
/// | 1.0e-4 | 166.946 | 166.696 | 192 / 205 |
/// | 2.0e-4 | 198.706 | 198.198 | 414 / 420 |
/// | 4.0e-4 | 201.070 | 200.046 | 447 / 445 |
/// | 6.0e-4 | 201.708 | 200.167 | 456 / 420 |
///
/// ## Interpretation
///
/// **Full-integration Quad4 over-predicts the collapse load by 23.9 % on a
/// coarse mesh**, and the error only falls as `1 / n_r^2`-ish with refinement:
/// 23.9 %, 6.5 %, 1.7 %, 0.42 %. That is volumetric locking measured
/// plastically: once a zone flows, J2 flow is volume preserving, and a
/// full-integration bilinear element imposes that constraint at four points per
/// element, which over-constrains the displacement field and makes the
/// structure carry load it should not.
///
/// **B-bar over-predicts by 1.06 %, 0.28 %, 0.07 % and 0.017 %** on the same
/// four meshes — between 22 and 25 times closer at every level. On the
/// `16 x 32` mesh it reproduces the closed-form limit pressure to 170 parts per
/// million.
///
/// Read across, the practical statement is that B-bar on the **2 x 4** mesh
/// (30 degrees of freedom, 1.1 % error) is more accurate than full integration
/// on the **8 x 16** mesh (306 degrees of freedom, 1.7 % error): ten times the
/// unknowns for a worse answer.
///
/// The plateau is real and not an artefact of stopping: between `u_r = 4e-4` and
/// `6e-4 m` the B-bar pressure moves by 0.06 %, having risen by 90 % over the
/// path. Both curves still creep slowly upward, which is expected — a
/// displacement-based discretisation approaches its own (slightly high) limit
/// load from below as the plastic zone finishes spreading through the
/// outermost element — and it is why the numbers above are quoted at a stated
/// displacement rather than as "the" collapse load.
///
/// **Limitations of this case, stated rather than buried.** It is small strain
/// with no geometry update, so at `u_r / a = 1.6 %` the bore has moved
/// appreciably and the real structure's limit load would differ slightly.
/// Straight-sided elements make the bore a polygon, which biases the pressure
/// by about 0.05 % at `n_theta = 16` — smaller than the B-bar error only on the
/// finest mesh, where it becomes comparable, so the `1.00017` figure should be
/// read as "indistinguishable from `p_L` at this mesh's geometric fidelity"
/// rather than as five-digit agreement. And the reference itself assumes a
/// perfectly plastic von Mises material in plane strain with no strain
/// hardening, which is the material that was run, but is not steel.
#[test]
fn fully_plastic_limit_load_full_integration_over_predicts_bbar_does_not() {
    let (a, b) = (0.05_f64, 0.10_f64);
    let p_l = limit_pressure(a, b, SIGMA_Y);
    let levels: Vec<f64> = (1..=32).map(|k| 2.5e-5 * k as f64).collect();
    println!("\nCASE 7: THICK-WALLED CYLINDER COLLAPSE, elastic-perfectly-plastic");
    println!(
        "  a = {a} m, b = {b} m, sigma_y = {} MPa, closed-form p_L = {:.4} MPa",
        SIGMA_Y / 1e6,
        p_l / 1e6
    );
    println!(
        "  {:>13} {:>7} {:>13} {:>9} {:>13} {:>9} {:>9}",
        "n_r x n_theta", "dofs", "p full (MPa)", "p/p_L", "p B-bar (MPa)", "p/p_L", "ratio"
    );

    let mut ratios = Vec::new();
    for n_r in [2usize, 4, 8, 16] {
        let n_theta = 2 * n_r;
        let full = *run_limit_load(Formulation::FullIntegration, n_r, n_theta, a, b, &levels)
            .last()
            .unwrap();
        let bbar = *run_limit_load(Formulation::BBar, n_r, n_theta, a, b, &levels)
            .last()
            .unwrap();
        let dofs = 2 * (n_r + 1) * (n_theta + 1);
        let excess_ratio = (full.1 - p_l) / (bbar.1 - p_l);
        println!(
            "  {:>13} {dofs:>7} {:>13.4} {:>9.5} {:>13.4} {:>9.5} {excess_ratio:>8.1}x",
            format!("{n_r} x {n_theta}"),
            full.1 / 1e6,
            full.1 / p_l,
            bbar.1 / 1e6,
            bbar.1 / p_l
        );
        assert!(
            full.1 > p_l && bbar.1 > p_l,
            "a displacement-based element must over-predict a limit load \
             (upper-bound theorem): full {:.4} MPa, B-bar {:.4} MPa vs p_L {:.4} MPa",
            full.1 / 1e6,
            bbar.1 / 1e6,
            p_l / 1e6
        );
        ratios.push((excess_ratio, bbar.1 / p_l));
    }

    for (r, _) in &ratios {
        assert!(
            *r > 5.0,
            "B-bar must cut the limit-load over-prediction by at least 5x, got {r:.1}x"
        );
    }
    let finest_bbar = ratios.last().unwrap().1;
    assert!(
        (finest_bbar - 1.0).abs() < 5.0e-3,
        "B-bar on the finest mesh must be within 0.5 % of the closed-form limit load, \
         got p/p_L = {finest_bbar:.5}"
    );
}

// ── Case 8: shear locking, which B-bar does NOT cure ────────────────────────

/// Quad4 cantilever tip deflection at `nu = 0`, in metres (downwards positive),
/// with the degree-of-freedom count.
///
/// Geometry, loading and support exactly as
/// `analytical::cantilever_tip_deflection_against_beam_theory`, so the two are
/// directly comparable: the beam occupies `0 <= x <= L`, `0 <= y <= H` with unit
/// thickness, the `x = 0` edge is fully clamped, and a uniform shear traction
/// `t_y = -P / H` on `x = L` carries the tip load `P`. The deflection is read at
/// the mid-height node of the loaded edge, so `ny` must be even.
///
/// `nu = 0` for the same reason as there: at zero Poisson's ratio plane strain,
/// plane stress and beam theory all use the same `E`, so any discrepancy is
/// shear deformation, end effects or discretisation error and nothing else.
fn run_quad4_cantilever(
    formulation: Formulation,
    length: f64,
    height: f64,
    load: f64,
    nx: usize,
    ny: usize,
) -> (f64, usize) {
    assert!(ny.is_multiple_of(2), "ny must be even so a mid-height node exists");
    let mesh = rectangle_quad4(length, height, nx, ny).unwrap().shared();
    let dofs = DofMap::displacement(&mesh);
    let mut system = System::with_options(
        mesh.clone(),
        Material::elastic(E_PA, 0.0).unwrap(),
        BodyForce::None,
        options_for(formulation),
    )
    .unwrap();
    let mut bcs = DirichletSet::new();
    let clamped = mesh.nodes_where(|c| c[0].abs() < 1e-12);
    bcs.fix_all_components(&dofs, &clamped);

    let mut forces = vec![0.0; system.n_dofs()];
    let facets = rectangle_right_edge_facets(nx, ny);
    Traction::uniform(facets, [0.0, -load / height, 0.0])
        .accumulate(&mesh, &dofs, &mut forces)
        .unwrap();

    // A slender beam is badly conditioned; 1e-8 is still three orders below the
    // smallest quantity being compared. Same reasoning as the Tri6 case.
    let (u, _) = solve_nonlinear(&mut system, &bcs, &forces, &locking_newton(1.0e-8, 1.0e-6))
        .expect("Quad4 cantilever");
    let tip = mesh
        .nodes_where(|c| (c[0] - length).abs() < 1e-12 && (c[1] - 0.5 * height).abs() < 1e-12)
        .into_iter()
        .next()
        .expect("a node at the mid-height of the loaded edge");
    (-u[tip.0 * 2 + 1], system.n_dofs())
}

/// # Shear locking in Quad4, and the fact that B-bar does not cure it
///
/// ## Why this case exists
///
/// Bead `op-vrtt.1` asserted that "a Quad4 mesh of the same coarseness would
/// give a visibly too-stiff beam" — an unmeasured claim, made to justify the
/// existing cantilever case using quadratic triangles. This measures it, and
/// measures whether the B-bar work of cases 6 and 7 fixes it. **It does not**,
/// and that is the headline of this case rather than a footnote.
///
/// ## Methodology
///
/// The cantilever of verification case 4, re-meshed with **bilinear
/// quadrilaterals**: `H = 0.1 m`, unit thickness, `E = 200 GPa`, `nu = 0`,
/// clamped at `x = 0`, uniform shear traction on `x = L` totalling `P = 1000 N`
/// downwards, tip deflection read at the mid-height node of the loaded edge.
///
/// Two slendernesses `L/H = 4` and `8`; elements through the depth
/// `ny = 2, 4, 8, 16` with **square** elements (`nx = ny L / H`), plus one
/// deliberately span-coarse mesh at `ny = 2` with element aspect ratio 4
/// (`nx = ny L / (4 H)`) to show the aspect-ratio sensitivity. Every mesh is run
/// with both formulations. Linear solves by Jacobi-preconditioned conjugate
/// gradients to `1e-8`.
///
/// **Reference:** Timoshenko beam theory,
/// `delta = P L^3 / (3 E I) + P L / (k G A)` with `I = H^3 / 12`, `A = H`,
/// `k = 5/6` and `G = E/2` at `nu = 0`. Timoshenko rather than Euler-Bernoulli,
/// because the quantity of interest here is how much stiffer than the *correct*
/// answer the element is, and the correct answer includes shear deformation.
///
/// ## Pass criterion
///
/// 1. Full-integration Quad4 must be **at least 5 % too stiff** at `ny = 2`
///    with square elements — the claim the bead made, now with a number on it.
/// 2. B-bar must **not** recover it: at least 2 % of the deficit must remain at
///    `ny = 2`. This assertion is written so that it fails if someone later
///    believes B-bar has fixed shear locking.
/// 3. Both formulations must converge monotonically towards Timoshenko as `ny`
///    grows, and neither may exceed it (a displacement-based element cannot be
///    softer than the true solution).
///
/// ## Results, measured 2026-09-11 (release build, this machine)
///
/// `delta / delta_Timoshenko`; below 1 means too stiff.
///
/// **`L/H = 4`, `delta_Timoshenko = 1.328000e-6 m`:**
///
/// | ny | nx | aspect | dofs | full integration | B-bar |
/// |---|---|---|---|---|---|
/// | 2 | 2 | 4 | 18 | 0.3313 | 0.3399 |
/// | 2 | 8 | 1 | 54 | 0.8835 | 0.9518 |
/// | 4 | 16 | 1 | 170 | 0.9673 | 0.9867 |
/// | 8 | 32 | 1 | 594 | 0.9911 | 0.9962 |
/// | 16 | 64 | 1 | 2210 | 0.9973 | 0.9986 |
///
/// **`L/H = 8`, `delta_Timoshenko = 1.033600e-5 m`:**
///
/// | ny | nx | aspect | dofs | full integration | B-bar |
/// |---|---|---|---|---|---|
/// | 2 | 4 | 4 | 30 | 0.3328 | 0.3421 |
/// | 2 | 16 | 1 | 102 | 0.8875 | 0.9579 |
/// | 4 | 32 | 1 | 330 | 0.9692 | 0.9890 |
/// | 8 | 64 | 1 | 1170 | 0.9920 | 0.9972 |
///
/// For comparison, the **Tri6** mesh of verification case 4 reaches 0.99930 at
/// `L/H = 4` on 1170 degrees of freedom — better than Quad4 with B-bar manages
/// on 2210.
///
/// ## Interpretation
///
/// **The bead's claim is confirmed and quantified.** With two square elements
/// through the depth a full-integration Quad4 cantilever is **11.25 % too
/// stiff** (`L/H = 8`), and with elements of aspect ratio 4 it is **66.7 % too
/// stiff** — two thirds of the deflection simply missing. The aspect-ratio sensitivity is the tell: shear locking comes from
/// the bilinear element's inability to represent pure bending without a
/// parasitic shear strain whose magnitude grows with the element's
/// length-to-depth ratio, so stretching elements along the span makes it far
/// worse while adding elements through the depth makes it better.
///
/// **B-bar helps, but it does not cure this, and it cannot.** It lifts
/// `ny = 2` from 0.8875 to 0.9579 — recovering about 63 % of the deficit — and
/// leaves **4.2 % still missing**. The part it recovers is the *volumetric*
/// share of the parasitic constraint: even at `nu = 0` the bulk modulus is
/// `E/3`, not zero, so bending excites a spurious dilatational stiffness that
/// the mean-dilatation modification relaxes. The part it does not recover is
/// the parasitic *shear* strain itself, which is a deviatoric mode and which
/// B-bar leaves entirely untouched by construction — it only ever substitutes
/// the volumetric part of `B`. On the aspect-ratio-4 mesh, where the shear
/// share dominates, B-bar moves 0.3328 only to 0.3421 — a 0.9-point
/// improvement on a 66.7 % error — and 65.8 % of the deflection is still
/// missing.
///
/// **Shear locking therefore remains an open defect in this crate**, tracked as
/// its own follow-up bead (`op-uqqg`). The fixes are a different family from
/// B-bar — incompatible modes (Wilson), enhanced assumed strain (Simo-Rifai),
/// assumed natural strain, or reduced integration with hourglass control — and
/// each needs verifying in its own right. Nothing in cases 6 and 7 bears on it,
/// and the B-bar work must not be described as having addressed it.
///
/// The practical advice this case supports, and the reason case 4 uses Tri6:
/// **do not bend a low-order quadrilateral**. If the mesh must be quadrilateral,
/// four or more elements through the depth with an aspect ratio near 1 keeps the
/// error near 1 %; two long elements through the depth loses a third of the
/// deflection.
#[test]
fn shear_locking_in_quad4_is_measured_and_bbar_does_not_cure_it() {
    let (height, load) = (0.1_f64, 1000.0_f64);
    let inertia = height.powi(3) / 12.0;
    let shear_modulus = E_PA / 2.0; // nu = 0
    println!("\nCASE 8: SHEAR LOCKING IN Quad4 (B-bar is NOT a cure)");

    let mut coarse_square_full = 0.0_f64;
    let mut coarse_square_bbar = 0.0_f64;
    for slenderness in [4usize, 8] {
        let length = height * slenderness as f64;
        let eb = load * length.powi(3) / (3.0 * E_PA * inertia);
        let timo = eb + load * length / ((5.0 / 6.0) * shear_modulus * height);
        println!(
            "\n  L/H = {slenderness}: Euler-Bernoulli {eb:.6e} m, Timoshenko {timo:.6e} m"
        );
        println!(
            "  {:>4} {:>5} {:>7} {:>7} {:>12} {:>12}",
            "ny", "nx", "aspect", "dofs", "full/Timo", "B-bar/Timo"
        );
        // (ny, nx, aspect): one deliberately span-coarse mesh, then square ones.
        let mut cases: Vec<(usize, usize, usize)> = vec![(2, 2 * slenderness / 4, 4)];
        for ny in [2usize, 4, 8, 16] {
            if ny * slenderness > 96 {
                continue;
            }
            cases.push((ny, ny * slenderness, 1));
        }
        let mut square_ratios: Vec<f64> = Vec::new();
        for (ny, nx, aspect) in cases {
            let (df, nd) = run_quad4_cantilever(Formulation::FullIntegration, length, height, load, nx, ny);
            let (db, _) = run_quad4_cantilever(Formulation::BBar, length, height, load, nx, ny);
            println!(
                "  {ny:>4} {nx:>5} {aspect:>7} {nd:>7} {:>12.4} {:>12.4}",
                df / timo,
                db / timo
            );
            assert!(
                df / timo < 1.0 + 1e-9 && db / timo < 1.0 + 1e-9,
                "a displacement-based element cannot be softer than the true solution: \
                 full {:.4}, B-bar {:.4} of Timoshenko",
                df / timo,
                db / timo
            );
            if aspect == 1 {
                square_ratios.push(df / timo);
                if ny == 2 {
                    coarse_square_full = df / timo;
                    coarse_square_bbar = db / timo;
                }
            }
        }
        for w in square_ratios.windows(2) {
            assert!(
                w[1] > w[0],
                "refining through the depth must reduce shear locking: {:?}",
                square_ratios
            );
        }
    }

    println!(
        "\n  at ny = 2 square elements: full integration is {:.1} % too stiff, \
         B-bar still {:.1} % too stiff (it recovers {:.0} % of the deficit)",
        100.0 * (1.0 - coarse_square_full),
        100.0 * (1.0 - coarse_square_bbar),
        100.0 * (coarse_square_bbar - coarse_square_full) / (1.0 - coarse_square_full)
    );
    assert!(
        coarse_square_full < 0.95,
        "full-integration Quad4 should be at least 5 % too stiff at two elements through \
         the depth; measured {coarse_square_full:.4} of Timoshenko"
    );
    assert!(
        coarse_square_bbar < 0.98,
        "B-bar does NOT cure shear locking and this assertion exists to keep that honest: \
         it should still be at least 2 % too stiff, measured {coarse_square_bbar:.4}"
    );
}
