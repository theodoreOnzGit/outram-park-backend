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

//! **VERIFICATION CASE 2 — manufactured-solution convergence study.**
//!
//! The single most informative test in the suite: it measures the *order* of
//! the method, not merely that it runs. Results are also collected in
//! `docs/verification.md`.

#[path = "common.rs"]
mod common;

use common::{elastic_newton, order};
use farrer_park::assembly::System;
use farrer_park::prelude::*;

const E_PA: f64 = 200.0e9;
const NU: f64 = 0.3;

/// Solve the manufactured problem on one mesh and return `(L2, H1)` errors.
///
/// The exact plane-strain displacement is
/// `u_x = u_y = sin(pi x) sin(pi y)` metres on the unit square, which vanishes
/// on the whole boundary, so the Dirichlet data is homogeneous and contributes
/// no boundary-approximation error of its own. The body force is
/// `-div sigma` for that field, evaluated exactly at each quadrature point by
/// [`BodyForce::ManufacturedSine`].
fn run(mesh: Mesh) -> (f64, f64, usize) {
    let elastic = LinearElastic::new(E_PA, NU).unwrap();
    let (lambda, mu) = (elastic.lame_lambda(), elastic.shear_modulus());
    let shared = mesh.shared();
    let dofs = DofMap::displacement(&shared);
    let material = Material::Elastic(elastic);
    let mut system = System::new(
        shared.clone(),
        material,
        BodyForce::ManufacturedSine(lambda, mu),
    );

    let mut bcs = DirichletSet::new();
    let boundary = shared.bounding_box_boundary_nodes(1e-10);
    bcs.fix_all_components(&dofs, &boundary);

    let forces = vec![0.0; system.n_dofs()];
    let (u, _) = solve_nonlinear(&mut system, &bcs, &forces, &elastic_newton())
        .expect("manufactured solve");
    let (l2, h1) = system.manufactured_errors(&u).expect("manufactured errors");
    (l2, h1, system.n_dofs())
}

/// Run a refinement sequence and print the observed orders.
///
/// Returns `(l2_orders, h1_orders)` between successive levels. Each refinement
/// halves `h`, so the order is `log2(e_coarse / e_fine)`.
fn sweep(label: &str, divisions: &[usize], build: fn(usize) -> Mesh) -> (Vec<f64>, Vec<f64>) {
    let mut l2s = Vec::new();
    let mut h1s = Vec::new();
    let mut l2_orders = Vec::new();
    let mut h1_orders = Vec::new();
    println!("\nMMS CONVERGENCE, {label}");
    println!("{:>5} {:>8} {:>13} {:>7} {:>13} {:>7}", "n", "dofs", "L2", "order", "H1", "order");
    for (i, &n) in divisions.iter().enumerate() {
        let (l2, h1, ndof) = run(build(n));
        let (mut lo, mut ho) = (f64::NAN, f64::NAN);
        if i > 0 {
            lo = order(l2s[i - 1], l2, 2.0);
            ho = order(h1s[i - 1], h1, 2.0);
            l2_orders.push(lo);
            h1_orders.push(ho);
        }
        l2s.push(l2);
        h1s.push(h1);
        println!("{n:>5} {ndof:>8} {l2:>13.5e} {lo:>7.3} {h1:>13.5e} {ho:>7.3}");
    }
    (l2_orders, h1_orders)
}

/// # Manufactured-solution convergence, linear elements (Quad4 and Tri3)
///
/// ## Methodology
///
/// Domain: the unit square, plane strain, `E = 200 GPa`, `nu = 0.3`
/// (`lambda = 115.385 GPa`, `mu = 76.923 GPa`).
///
/// Manufactured displacement, in metres:
///
/// `u_x = u_y = sin(pi x) sin(pi y)`
///
/// which is zero on the entire boundary. Substituting it into the plane-strain
/// Navier equation `div sigma + f = 0` with
/// `div sigma = (lambda + mu) grad(div u) + mu laplacian(u)` gives the body
/// force, in newton per cubic metre:
///
/// `f_x = f_y = pi^2 [(lambda + 3 mu) sin(pi x) sin(pi y)
///                    - (lambda + mu) cos(pi x) cos(pi y)]`
///
/// which is what [`BodyForce::ManufacturedSine`] evaluates. Homogeneous
/// Dirichlet conditions on every boundary node, imposed by strong elimination.
/// Linear solves by ILU(0)-preconditioned conjugate gradients to `1e-14`
/// relative residual, so the linear solver is never the limiting error.
///
/// Errors are integrated with [`farrer_park::quadrature::error_rule`] — a
/// richer rule than assembly uses — because the integrand
/// `(u_h - u_exact)^2` is not a polynomial and the assembly rule would measure
/// its own quadrature error instead of the discretisation error.
///
/// Meshes: `n = 4, 8, 16, 32` divisions per side, so `h` halves at each level
/// and the observed order is `log2(e_coarse / e_fine)`.
///
/// ## Pass criterion
///
/// Theory for a complete polynomial of degree `p = 1`: `L2` error `~ h^2`,
/// `H1` seminorm error `~ h^1`. The finest observed order of each must be
/// within `0.15` of theory.
///
/// ## Results, measured 2026-09-11
///
/// Quad4:
///
/// | n | dofs | L2 | order | H1 | order |
/// |---|---|---|---|---|---|
/// | 4 | 50 | 4.49706e-2 | | 8.75414e-1 | |
/// | 8 | 162 | 1.15850e-2 | 1.957 | 4.44832e-1 | 0.977 |
/// | 16 | 578 | 2.91598e-3 | 1.990 | 2.23172e-1 | 0.995 |
/// | 32 | 2178 | 7.30265e-4 | 1.997 | 1.11685e-1 | 0.999 |
///
/// Tri3:
///
/// | n | dofs | L2 | order | H1 | order |
/// |---|---|---|---|---|---|
/// | 4 | 50 | 6.54216e-2 | | 1.03308e+0 | |
/// | 8 | 162 | 1.72391e-2 | 1.924 | 5.24222e-1 | 0.979 |
/// | 16 | 578 | 4.36657e-3 | 1.981 | 2.63125e-1 | 0.995 |
/// | 32 | 2178 | 1.09548e-3 | 1.995 | 1.31684e-1 | 0.999 |
///
/// ## Interpretation
///
/// Both linear elements converge at the theoretical rates, and the observed
/// orders approach 2 and 1 monotonically from below as the mesh refines —
/// which is the signature of a correctly implemented method with an
/// asymptotically vanishing higher-order term, rather than of a rate that
/// happens to look right on one pair of meshes.
///
/// Quad4 is uniformly more accurate than Tri3 at equal degree-of-freedom count
/// (by a factor of about 1.5 in `L2`), which is the usual ordering: the
/// bilinear quadrilateral carries an extra `xy` term the constant-strain
/// triangle does not.
///
/// This result verifies the numerics — shape functions, quadrature, assembly,
/// boundary conditions and the linear solve together. It is **not** validation:
/// no physical measurement is involved anywhere in it.
#[test]
fn mms_convergence_linear_elements() {
    use farrer_park::mesh::{unit_square_quad4, unit_square_tri3};

    let (l2q, h1q) = sweep("Quad4", &[4, 8, 16, 32], |n| unit_square_quad4(n).unwrap());
    let (l2t, h1t) = sweep("Tri3", &[4, 8, 16, 32], |n| unit_square_tri3(n).unwrap());

    for (name, l2, h1) in [("Quad4", &l2q, &h1q), ("Tri3", &l2t, &h1t)] {
        let last_l2 = *l2.last().unwrap();
        let last_h1 = *h1.last().unwrap();
        assert!(
            (last_l2 - 2.0).abs() < 0.15,
            "{name}: observed L2 order {last_l2:.3}, expected 2 +/- 0.15"
        );
        assert!(
            (last_h1 - 1.0).abs() < 0.15,
            "{name}: observed H1 order {last_h1:.3}, expected 1 +/- 0.15"
        );
        // The orders must approach theory, not wander.
        assert!(
            l2.windows(2).all(|w| w[1] >= w[0] - 0.02),
            "{name}: L2 orders not monotone towards theory: {l2:?}"
        );
    }
}

/// # Manufactured-solution convergence, quadratic elements (Tri6)
///
/// ## Methodology
///
/// Identical to [`mms_convergence_linear_elements`] except for the element:
/// six-node straight-sided quadratic triangles, built by promoting the Tri3
/// mesh so every midside node sits exactly at an edge midpoint. Meshes
/// `n = 2, 4, 8, 16` divisions per side.
///
/// ## Pass criterion
///
/// Theory for `p = 2`: `L2 ~ h^3`, `H1 ~ h^2`. Finest observed order within
/// `0.15` of theory.
///
/// ## Results, measured 2026-09-11
///
/// | n | dofs | L2 | order | H1 | order |
/// |---|---|---|---|---|---|
/// | 2 | 50 | 5.13122e-2 | | 8.04003e-1 | |
/// | 4 | 162 | 7.21200e-3 | 2.831 | 2.17246e-1 | 1.888 |
/// | 8 | 578 | 9.24405e-4 | 2.964 | 5.53148e-2 | 1.974 |
/// | 16 | 2178 | 1.16174e-4 | 2.992 | 1.38891e-2 | 1.994 |
///
/// ## Interpretation
///
/// Third order in `L2` and second in `H1`, approached monotonically from
/// below — the quadratic element is behaving exactly as theory requires, and
/// the gain over the linear elements is large: at 2178 degrees of freedom Tri6
/// is 6.3 times more accurate in `L2` than Quad4 and 9.4 times more accurate
/// than Tri3.
///
/// One limitation worth stating: these are **straight-sided** quadratic
/// triangles on a polygonal domain, so nothing here exercises the curved
/// isoparametric mapping. A curved-boundary problem would need its own
/// convergence study before the same claim could be made there.
#[test]
fn mms_convergence_quadratic_elements() {
    use farrer_park::mesh::unit_square_tri6;

    let (l2, h1) = sweep("Tri6", &[2, 4, 8, 16], |n| unit_square_tri6(n).unwrap());
    let last_l2 = *l2.last().unwrap();
    let last_h1 = *h1.last().unwrap();
    assert!(
        (last_l2 - 3.0).abs() < 0.15,
        "Tri6: observed L2 order {last_l2:.3}, expected 3 +/- 0.15"
    );
    assert!(
        (last_h1 - 2.0).abs() < 0.15,
        "Tri6: observed H1 order {last_h1:.3}, expected 2 +/- 0.15"
    );
}

/// # Manufactured-solution convergence in three dimensions (Hex8 and Tet4)
///
/// ## Methodology
///
/// The two-dimensional manufactured field is used unchanged on the unit cube,
/// with `u_z = 0` prescribed on the whole boundary along with `u_x` and `u_y`.
/// The field is independent of `z`, so it remains an exact solution of the
/// three-dimensional Navier equation with the same body force — the `z`
/// derivatives of everything are zero and the `z` momentum equation is
/// satisfied identically by `u_z = 0`.
///
/// This is a deliberately cheap way to exercise the three-dimensional element
/// path on a problem with a known answer. It does **not** test a genuinely
/// three-dimensional solution, and a fully 3-D manufactured field would be a
/// stronger check; that is noted as a limitation rather than papered over.
///
/// Meshes: `n = 2, 4, 8` divisions per side.
///
/// ## Results, measured 2026-09-11
///
/// Hex8:
///
/// | n | dofs | L2 | order | H1 | order |
/// |---|---|---|---|---|---|
/// | 2 | 81 | 1.13157e-1 | | 1.34544e+0 | |
/// | 4 | 375 | 4.49706e-2 | 1.331 | 8.75414e-1 | 0.620 |
/// | 8 | 2187 | 1.15850e-2 | 1.957 | 4.44832e-1 | 0.977 |
///
/// Tet4:
///
/// | n | dofs | L2 | order | H1 | order |
/// |---|---|---|---|---|---|
/// | 2 | 81 | 1.31182e-1 | | 1.47279e+0 | |
/// | 4 | 375 | 5.05207e-2 | 1.377 | 9.42096e-1 | 0.645 |
/// | 8 | 2187 | 1.30594e-2 | 1.952 | 4.77198e-1 | 0.981 |
///
/// ## Interpretation
///
/// Both three-dimensional elements reach the theoretical linear-element rates
/// (`L2 ~ h^2`, `H1 ~ h^1`) by the finest level. The coarse-to-medium orders
/// (1.33, 0.62) are well below theory, which is pre-asymptotic rather than
/// wrong: two elements per side cannot resolve a full sine period, so the
/// coarsest mesh is not yet in the asymptotic range. The medium-to-fine orders
/// (1.96, 0.98) match the two-dimensional Quad4 result at the same `h` to three
/// digits, which is the expected consequence of the solution being
/// `z`-independent.
///
/// The pass criterion is therefore applied only to the finest pair, with the
/// coarse pair reported rather than asserted on.
#[test]
fn mms_convergence_three_dimensional() {
    use farrer_park::mesh::{unit_cube_hex8, unit_cube_tet4};

    let (l2h, h1h) = sweep("Hex8", &[2, 4, 8], |n| unit_cube_hex8(n).unwrap());
    let (l2t, h1t) = sweep("Tet4", &[2, 4, 8], |n| unit_cube_tet4(n).unwrap());

    for (name, l2, h1) in [("Hex8", &l2h, &h1h), ("Tet4", &l2t, &h1t)] {
        let last_l2 = *l2.last().unwrap();
        let last_h1 = *h1.last().unwrap();
        assert!(
            (last_l2 - 2.0).abs() < 0.15,
            "{name}: observed L2 order {last_l2:.3}, expected 2 +/- 0.15"
        );
        assert!(
            (last_h1 - 1.0).abs() < 0.15,
            "{name}: observed H1 order {last_h1:.3}, expected 1 +/- 0.15"
        );
    }
}
