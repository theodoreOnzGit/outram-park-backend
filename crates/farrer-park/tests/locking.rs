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
    let settings = elastic_newton_with(1.0e-10, 1.0e-8, 1.0e-6);
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
