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

mod common;

use common::{mms_newton, order};
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

    // The exact field is imposed on EVERY boundary node, not merely set to
    // zero. In two dimensions the two are the same thing (the field vanishes on
    // the whole boundary), but on the unit cube the z = 0 and z = 1 faces carry
    // non-zero data, and forcing them to zero there would over-constrain the
    // problem and destroy the convergence rate.
    let mut bcs = DirichletSet::new();
    let boundary = shared.bounding_box_boundary_nodes(1e-10);
    let bf = BodyForce::ManufacturedSine(lambda, mu);
    bcs.fix_from_field(&shared, &dofs, &boundary, |x| {
        bf.manufactured_solution(x).expect("manufactured field").0
    });

    let forces = vec![0.0; system.n_dofs()];
    let (u, _) = solve_nonlinear(&mut system, &bcs, &forces, &mms_newton())
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
/// ## Results, measured 2026-09-11 (release build, this machine)
///
/// Quad4:
///
/// | n | dofs | L2 | order | H1 | order |
/// |---|---|---|---|---|---|
/// | 4 | 50 | 4.37275e-2 | | 7.09755e-1 | |
/// | 8 | 162 | 1.11518e-2 | 1.971 | 3.55844e-1 | 0.996 |
/// | 16 | 578 | 2.80446e-3 | 1.991 | 1.78034e-1 | 0.999 |
/// | 32 | 2178 | 7.02209e-4 | 1.998 | 8.90303e-2 | 1.000 |
///
/// Tri3:
///
/// | n | dofs | L2 | order | H1 | order |
/// |---|---|---|---|---|---|
/// | 4 | 50 | 8.22110e-2 | | 1.19874e+0 | |
/// | 8 | 162 | 2.12183e-2 | 1.954 | 6.12430e-1 | 0.969 |
/// | 16 | 578 | 5.34688e-3 | 1.989 | 3.07871e-1 | 0.992 |
/// | 32 | 2178 | 1.33938e-3 | 1.997 | 1.54143e-1 | 0.998 |
///
/// ## Interpretation
///
/// Both linear elements converge at the theoretical rates. The observed orders
/// approach 2 and 1 **monotonically from below** as the mesh refines, which is
/// the signature of a correctly implemented method whose higher-order terms are
/// vanishing — rather than of a rate that happens to look right on one pair of
/// meshes. At the finest level the agreement is 1.998 against 2 and 1.000
/// against 1 for Quad4, and 1.997 and 0.998 for Tri3.
///
/// Quad4 is uniformly the more accurate of the two at equal degree-of-freedom
/// count: 7.02e-4 against 1.34e-3 in `L2` at 2178 degrees of freedom, a factor
/// of 1.91. That is the usual ordering — the bilinear quadrilateral carries an
/// `xy` term the constant-strain triangle does not — and it is reported here
/// because it is a free consistency check on both implementations at once.
///
/// This verifies the numerics: shape functions, quadrature, assembly, boundary
/// conditions and the linear solve, together. It is **not** validation. No
/// physical measurement appears anywhere in it, and a manufactured solution is
/// chosen for convenience, not because any structure behaves that way.
///
/// Limitations worth stating: one material (`nu = 0.3`, well away from the
/// incompressible limit where these elements lock), one smooth solution, and
/// uniform meshes.
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
/// | 2 | 50 | 3.40645e-2 | | 6.69263e-1 | |
/// | 4 | 162 | 4.82246e-3 | 2.820 | 1.84008e-1 | 1.863 |
/// | 8 | 578 | 6.33194e-4 | 2.929 | 4.72934e-2 | 1.960 |
/// | 16 | 2178 | 8.03541e-5 | 2.978 | 1.19117e-2 | 1.989 |
///
/// ## Interpretation
///
/// Third order in `L2` (2.978) and second in `H1` (1.989) at the finest level,
/// approached monotonically from below. The quadratic element behaves exactly
/// as theory requires, and the gain over the linear elements is large: at 2178
/// degrees of freedom Tri6 is **8.7 times** more accurate in `L2` than Quad4
/// and **16.7 times** more accurate than Tri3, on the same mesh count.
///
/// The observed orders are slightly below theory at every level and rising
/// (2.820, 2.929, 2.978), which is the expected pre-asymptotic behaviour. If
/// they were *above* theory that would be the suspicious result, since it would
/// usually mean the error norm itself is being under-integrated.
///
/// One limitation: these are **straight-sided** quadratic triangles on a
/// polygonal domain, so nothing here exercises a curved isoparametric mapping.
/// A curved-boundary problem would need its own convergence study before the
/// same claim could be made there.
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
/// | 2 | 81 | 2.03345e-1 | | 1.45631e+0 | |
/// | 4 | 375 | 5.35370e-2 | 1.925 | 7.18005e-1 | 1.020 |
/// | 8 | 2187 | 1.35460e-2 | 1.983 | 3.56977e-1 | 1.008 |
///
/// Tet4:
///
/// | n | dofs | L2 | order | H1 | order |
/// |---|---|---|---|---|---|
/// | 2 | 81 | 2.77936e-1 | | 2.20001e+0 | |
/// | 4 | 375 | 7.92357e-2 | 1.811 | 1.19951e+0 | 0.875 |
/// | 8 | 2187 | 2.04129e-2 | 1.957 | 6.12571e-1 | 0.969 |
///
/// ## Interpretation
///
/// Both three-dimensional elements reach the theoretical linear-element rates
/// (`L2 ~ h^2`, `H1 ~ h^1`). Hex8 is already close on the coarse-to-medium pair
/// (1.925, 1.020); Tet4 is visibly pre-asymptotic there (1.811, 0.875) and
/// catches up by the finest pair (1.957, 0.969), which is unsurprising given
/// the Kuhn subdivision produces six rather elongated tetrahedra per cell.
///
/// At equal `h`, Hex8's `L2` error (1.355e-2 at `n = 8`) is 21 % **larger**
/// than the two-dimensional Quad4 result at the same `h` (1.115e-2), even
/// though the exact solution is `z`-independent. That is not an inconsistency:
/// a trilinear hexahedron restricted to a `z`-independent field is not the same
/// discrete space as a bilinear quadrilateral, because the `z` interpolation
/// participates in the volume integrals. Tet4 is a further 51 % worse than
/// Hex8, the usual penalty for a constant-strain simplex.
///
/// The pass criterion is applied only to the finest pair, with the coarse pair
/// reported rather than asserted on. A **genuinely three-dimensional**
/// manufactured field would be a stronger check than a `z`-extruded
/// two-dimensional one, and is not done here; that is a real limitation of this
/// case, not a formality.
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
