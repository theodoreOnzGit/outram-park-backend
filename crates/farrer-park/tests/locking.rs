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

//! **VERIFICATION CASES 6, 7 and 8** — locking.

mod common;

use common::{elastic_newton_with, order};
use farrer_park::assembly::System;
use farrer_park::prelude::*;

const E_PA: f64 = 200.0e9;

fn bbar_options() -> SystemOptions {
    SystemOptions {
        formulation: Formulation::BBar,
        ..SystemOptions::default()
    }
}

fn options_for(formulation: Formulation) -> SystemOptions {
    SystemOptions {
        formulation,
        ..SystemOptions::default()
    }
}

/// Solve the manufactured problem on one mesh with one formulation and return
/// `(L2 error, H1 seminorm error, dofs)`.
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
    let mut settings = elastic_newton_with(1.0e-10, 1.0e-8, 1.0e-6);
    settings.linear.preconditioner = PreconditionerChoice::Jacobi;
    settings.linear.max_iter = 200_000;
    let r = solve_nonlinear(&mut system, &bcs, &forces, &settings);
    match r {
        Ok((u, _)) => {
            let (l2, h1) = system.manufactured_errors(&u).expect("manufactured errors");
            (l2, h1, system.n_dofs())
        }
        Err(e) => { println!("    SOLVE FAILED: {e}"); (f64::NAN, f64::NAN, system.n_dofs()) }
    }
}

#[test]
fn scratch_incompressible_hex8() {
    for nu in [0.3_f64, 0.499] {
        for f in [Formulation::FullIntegration, Formulation::BBar] {
            println!("\nHEX8 nu = {nu}, {}", f.name());
            let mut prev: Option<f64> = None;
            for n in [2usize, 4, 8, 16] {
                let t = std::time::Instant::now();
                let (l2, h1, nd) = run_mms(unit_cube_hex8(n).unwrap(), nu, f, true);
                let o = prev.map_or(f64::NAN, |p| order(p, l2, 2.0));
                println!("  n={n:>3} dofs={nd:>6} L2={l2:.5e} order={o:>6.3} H1={h1:.5e} [{:.1}s]", t.elapsed().as_secs_f64());
                prev = Some(l2);
            }
        }
    }
}

#[test]
fn scratch_incompressible_quad4() {
    for nu in [0.3_f64, 0.499] {
        for f in [Formulation::FullIntegration, Formulation::BBar] {
            println!("\nnu = {nu}, {}", f.name());
            let mut prev: Option<f64> = None;
            for n in [4usize, 8, 16, 32] {
                let (l2, h1, nd) = run_mms(unit_square_quad4(n).unwrap(), nu, f, false);
                let o = prev.map_or(f64::NAN, |p| order(p, l2, 2.0));
                println!("  n={n:>3} dofs={nd:>6} L2={l2:.5e} order={o:>6.3} H1={h1:.5e}");
                prev = Some(l2);
            }
        }
    }
}

const SIGMA_Y: f64 = 250.0e6;
const NU: f64 = 0.3;

/// Displacement-controlled thick-walled cylinder: ramp the bore radial
/// displacement and read back the pressure from the support reaction.
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
    let material = Material::j2_linear_hardening(E_PA, NU, SIGMA_Y, 0.0).unwrap();
    let mut system =
        System::with_options(mesh.clone(), material, BodyForce::None, options_for(formulation))
            .unwrap();

    let bore: Vec<NodeId> = mesh
        .nodes_where(|c| ((c[0] * c[0] + c[1] * c[1]).sqrt() - a).abs() < 1e-12 * a.max(1.0))
        .into_iter()
        .collect();
    assert_eq!(bore.len(), n_theta + 1, "bore node count");
    let perimeter = n_theta as f64 * 2.0 * a * (std::f64::consts::FRAC_PI_4 / n_theta as f64).sin();

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
        let forces = vec![0.0; system.n_dofs()];
        match solve_nonlinear(&mut system, &bcs, &forces, &settings) {
            Ok((u, _)) => {
                let asm = system.assemble(&u, None).unwrap();
                let mut radial = 0.0;
                for &n in &bore {
                    let c = mesh.node(n);
                    let r = (c[0] * c[0] + c[1] * c[1]).sqrt();
                    radial += asm.internal_force[n.0 * 2] * c[0] / r
                        + asm.internal_force[n.0 * 2 + 1] * c[1] / r;
                }
                out.push((ur, radial / perimeter, asm.n_yielding));
            }
            Err(e) => {
                println!("    u_r = {ur:.3e} FAILED: {e}");
                break;
            }
        }
    }
    out
}

#[test]
fn scratch_limit_load() {
    let (a, b) = (0.05_f64, 0.10_f64);
    let p_l = 2.0 / 3.0_f64.sqrt() * SIGMA_Y * (b / a).ln();
    println!("\nclosed-form limit pressure p_L = {:.4} MPa", p_l / 1e6);
    let levels: Vec<f64> = (1..=32).map(|k| 2.5e-5 * k as f64).collect();
    for nr in [2usize, 4, 8, 16] {
        for f in [Formulation::FullIntegration, Formulation::BBar] {
            let r = run_limit_load(f, nr, 2 * nr, a, b, &levels);
            let (ur, p, ny) = *r.last().unwrap();
            println!("  n_r={nr:>3} {:<17} u_r={ur:.2e} p={:.4} MPa p/p_L={:.5} yield={ny}",
                     f.name(), p / 1e6, p / p_l);
        }
    }
}

/// Quad4 cantilever tip deflection, in metres, at `nu = 0`.
fn run_quad4_cantilever(
    formulation: Formulation,
    length: f64,
    height: f64,
    load: f64,
    nx: usize,
    ny: usize,
) -> (f64, usize) {
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

    let mut settings = elastic_newton_with(1.0e-10, 1.0e-8, 1.0e-5);
    settings.linear.preconditioner = PreconditionerChoice::Jacobi;
    settings.linear.max_iter = 500_000;
    let (u, _) = solve_nonlinear(&mut system, &bcs, &forces, &settings).expect("quad4 cantilever");
    let tip = mesh
        .nodes_where(|c| (c[0] - length).abs() < 1e-12 && (c[1] - 0.5 * height).abs() < 1e-12)
        .into_iter()
        .next()
        .expect("mid-height node on the loaded edge");
    (-u[tip.0 * 2 + 1], system.n_dofs())
}

#[test]
fn scratch_shear_locking() {
    let (h, load) = (0.1_f64, 1000.0_f64);
    let inertia = h.powi(3) / 12.0;
    for slenderness in [4usize, 8] {
        let l = h * slenderness as f64;
        let eb = load * l.powi(3) / (3.0 * E_PA * inertia);
        let timo = eb + load * l / ((5.0 / 6.0) * (E_PA / 2.0) * h);
        println!("\nL/H = {slenderness}: EB = {eb:.6e} m, Timoshenko = {timo:.6e} m");
        for ny in [1usize, 2, 4, 8, 16] {
            let nx = ny * slenderness;
            let mut row = format!("  ny={ny:>3} nx={nx:>4}");
            for f in [Formulation::FullIntegration, Formulation::BBar] {
                let (d, nd) = run_quad4_cantilever(f, l, h, load, nx, ny.max(2));
                row += &format!("  {}: d={d:.5e} d/Timo={:.4} (dofs {nd})", f.name(), d / timo);
            }
            println!("{row}");
        }
    }
}
