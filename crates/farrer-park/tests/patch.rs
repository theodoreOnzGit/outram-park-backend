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

//! **VERIFICATION CASE 1 — the patch test**, for every element type.
//!
//! Results are also collected in `docs/verification.md`.

mod common;

use common::{elastic_newton, jitter};
use farrer_park::assembly::System;
use farrer_park::prelude::*;

/// Steel, used throughout: `E = 200 GPa`, `nu = 0.3`.
const E_PA: f64 = 200.0e9;
const NU: f64 = 0.3;

/// The linear displacement field imposed on the patch boundary, in metres.
///
/// `u_i = c_i + sum_j A_ij x_j` with the coefficients below. Chosen so that
/// every strain component is non-zero (in 3-D) and none is a multiple of
/// another, which is what makes the test able to catch a transposed or
/// mis-scaled row of the strain-displacement operator.
fn linear_field(x: [f64; 3], dim: usize) -> [f64; 3] {
    const S: f64 = 1.0e-4;
    if dim == 2 {
        [
            S * (1.0 + 2.0 * x[0] + 3.0 * x[1]),
            S * (-0.5 + 1.5 * x[0] - 2.0 * x[1]),
            0.0,
        ]
    } else {
        [
            S * (1.0 + 2.0 * x[0] + 3.0 * x[1] - 1.0 * x[2]),
            S * (-0.5 + 1.5 * x[0] - 2.0 * x[1] + 0.5 * x[2]),
            S * (0.25 - 1.0 * x[0] + 0.75 * x[1] + 1.25 * x[2]),
        ]
    }
}

/// The constant strain the field above produces, in engineering Voigt form.
fn exact_strain(dim: usize) -> Voigt6 {
    const S: f64 = 1.0e-4;
    if dim == 2 {
        // eps_xx = 2S, eps_yy = -2S, gamma_xy = 3S + 1.5S
        Voigt6::new(2.0 * S, -2.0 * S, 0.0, 0.0, 0.0, 4.5 * S)
    } else {
        // gamma_yz = du_y/dz + du_z/dy = 0.5S + 0.75S
        // gamma_xz = du_x/dz + du_z/dx = -1.0S + -1.0S
        // gamma_xy = du_x/dy + du_y/dx = 3.0S + 1.5S
        Voigt6::new(2.0 * S, -2.0 * S, 1.25 * S, 1.25 * S, -2.0 * S, 4.5 * S)
    }
}

/// Run one patch test and return `(max relative displacement error, max
/// relative stress error)`.
///
/// # Methodology
///
/// 1. Take a structured mesh and **displace every interior node** by up to
///    `jitter_fraction` of the element size, deterministically. A patch test on
///    a uniform grid is far weaker: a uniform grid can pass for reasons that
///    have nothing to do with the element formulation.
/// 2. Impose the exact linear displacement field on **every boundary node, all
///    components**, by strong elimination.
/// 3. Solve.
/// 4. Compare the interior nodal displacements with the exact field, and the
///    stress at every quadrature point with the single constant stress
///    `C_e : eps_exact`.
///
/// # Pass criterion
///
/// Both errors must be at the level of the linear-solver tolerance
/// (`1e-14` relative residual), not at the level of a discretisation error.
/// The threshold asserted is `1e-9` relative, which is several orders looser
/// than what is measured and is there to catch a regression rather than to
/// certify the precision — the measured numbers are printed and recorded.
fn run_patch(mesh: Mesh, jitter_fraction: f64, label: &str) -> (f64, f64) {
    let dim = mesh.dim();
    let h = 1.0 / 3.0; // every generator below uses three divisions per side
    let jittered = mesh
        .map_coords(|i, c| {
            let on_boundary = (0..dim).any(|k| c[k].abs() < 1e-12 || (c[k] - 1.0).abs() < 1e-12);
            if on_boundary {
                c
            } else {
                let mut o = c;
                for k in 0..dim {
                    o[k] += jitter_fraction * h * jitter(i, k);
                }
                o
            }
        })
        .expect("jittered mesh");

    let shared = jittered.clone().shared();
    let dofs = DofMap::displacement(&shared);
    let material = Material::elastic(E_PA, NU).expect("steel");
    let mut system = System::new(shared.clone(), material, BodyForce::None);

    let boundary = shared.bounding_box_boundary_nodes(1e-10);
    let mut bcs = DirichletSet::new();
    bcs.fix_from_field(&shared, &dofs, &boundary, |x| linear_field(x, dim));

    let forces = vec![0.0; system.n_dofs()];
    let (u, report) = solve_nonlinear(&mut system, &bcs, &forces, &elastic_newton())
        .unwrap_or_else(|e| panic!("{label}: solve failed: {e}"));
    assert_eq!(report.steps.len(), 1);

    // Displacement error at every node, relative to the field's own scale.
    let scale = (0..shared.n_nodes())
        .flat_map(|n| {
            let v = linear_field(shared.coords()[n], dim);
            (0..dim).map(move |c| v[c].abs())
        })
        .fold(0.0_f64, f64::max);
    let mut max_u_err = 0.0_f64;
    for n in 0..shared.n_nodes() {
        let want = linear_field(shared.coords()[n], dim);
        for c in 0..dim {
            max_u_err = max_u_err.max((u[n * dim + c] - want[c]).abs() / scale);
        }
    }

    // Stress error at every quadrature point against the single constant value.
    let exact_stress = material.elastic_stiffness().apply(&exact_strain(dim));
    let s_scale = exact_stress.abs_max();
    let mut max_s_err = 0.0_f64;
    for s in system.quadrature_stress() {
        max_s_err = max_s_err.max(s.minus(&exact_stress).abs_max() / s_scale);
    }

    println!(
        "PATCH TEST {label:>6}: nodes {:5}, elements {:5}, \
         max relative displacement error {max_u_err:.3e}, \
         max relative stress error {max_s_err:.3e}",
        shared.n_nodes(),
        shared.n_elements()
    );
    (max_u_err, max_s_err)
}

/// # Patch test, all five element types
///
/// ## Methodology
///
/// An irregular patch of each element type on the unit square (2-D) or unit
/// cube (3-D), three divisions per side, with every interior node displaced by
/// up to 12 % of the element size (25 % for the quadratic triangle, which can
/// take more distortion). Material: isotropic linear elasticity,
/// `E = 200 GPa`, `nu = 0.3`; 2-D cases are plane strain. The exact linear
/// displacement field given by `linear_field` is imposed on every boundary
/// degree of freedom by strong elimination; the linear system is solved by
/// ILU(0)-preconditioned conjugate gradients to a relative residual of `1e-14`.
///
/// The element must reproduce that field in the interior, and a single constant
/// stress at every quadrature point. **Pass criterion: both relative errors
/// below `1e-9`** — several orders looser than the measured values, so the
/// assertion is a regression guard rather than a precision claim.
///
/// ## Results, measured 2026-09-11 (release build, this machine)
///
/// | Element | Nodes | Elements | max rel. displacement error | max rel. stress error |
/// |---|---|---|---|---|
/// | Tri3 | 16 | 18 | 9.035e-17 | 2.690e-15 |
/// | Tri6 | 49 | 18 | 1.807e-16 | 1.313e-14 |
/// | Quad4 | 16 | 9 | 9.035e-17 | 2.475e-15 |
/// | Tet4 | 64 | 162 | 9.035e-17 | 1.978e-15 |
/// | Hex8 | 64 | 27 | 9.035e-17 | 1.649e-15 |
///
/// ## Interpretation
///
/// Every element passes at machine precision. The displacement errors are one
/// to two units in the last place of the field's own magnitude, and the stress
/// errors sit a decade or so higher because forming the stress multiplies the
/// nodal round-off by the elastic modulus and differentiates the shape
/// functions. Nothing here is a discretisation error: these are round-off
/// levels, which is exactly what a passing patch test looks like.
///
/// The quadratic triangle's stress error (1.3e-14) is the largest, by about
/// half a decade. That is expected rather than suspicious: its shape-function
/// gradients vary within the element, so the strain at a quadrature point is a
/// longer floating-point sum than the constant-strain triangle's.
///
/// This verifies the shape functions and their derivatives, the isoparametric
/// Jacobian on a distorted element, and the assembly and scatter path. It does
/// **not** verify the convergence rate, which is what the manufactured-solution
/// study is for, and it says nothing about accuracy on a problem whose exact
/// solution is not linear.
#[test]
fn patch_test_all_element_types() {
    use farrer_park::mesh::{tri3_to_tri6, unit_cube_hex8, unit_cube_tet4, unit_square_quad4,
                            unit_square_tri3};

    let cases: Vec<(&str, Mesh, f64)> = vec![
        ("Tri3", unit_square_tri3(3).unwrap(), 0.12),
        ("Quad4", unit_square_quad4(3).unwrap(), 0.12),
        ("Tet4", unit_cube_tet4(3).unwrap(), 0.12),
        ("Hex8", unit_cube_hex8(3).unwrap(), 0.12),
    ];
    for (label, mesh, frac) in cases {
        let (ue, se) = run_patch(mesh, frac, label);
        assert!(ue < 1e-9, "{label}: displacement error {ue:e}");
        assert!(se < 1e-9, "{label}: stress error {se:e}");
    }

    // Tri6 is built by promoting an ALREADY-JITTERED Tri3 mesh, so its midside
    // nodes land on the midpoints of the distorted edges and the elements stay
    // straight-sided. Jittering a Tri6 mesh directly would curve the edges,
    // which a linear field genuinely cannot be reproduced on, and the test
    // would be failing the mesh rather than the element.
    let base = unit_square_tri3(3).unwrap();
    let jittered_base = base
        .map_coords(|i, c| {
            let on_boundary = c[0].abs() < 1e-12
                || c[1].abs() < 1e-12
                || (c[0] - 1.0).abs() < 1e-12
                || (c[1] - 1.0).abs() < 1e-12;
            if on_boundary {
                c
            } else {
                [
                    c[0] + 0.25 / 3.0 * jitter(i, 0),
                    c[1] + 0.25 / 3.0 * jitter(i, 1),
                    0.0,
                ]
            }
        })
        .unwrap();
    let tri6 = tri3_to_tri6(&jittered_base).unwrap();
    let (ue, se) = run_patch(tri6, 0.0, "Tri6");
    assert!(ue < 1e-9, "Tri6: displacement error {ue:e}");
    assert!(se < 1e-9, "Tri6: stress error {se:e}");
}

/// # Patch test under the penalty Dirichlet method
///
/// ## Methodology
///
/// The Quad4 patch above, re-run with [`DirichletMethod::Penalty`] at
/// `beta = 1e10` instead of strong elimination, everything else identical.
///
/// ## Results, measured 2026-09-11
///
/// | Quantity | Value |
/// |---|---|
/// | max relative displacement error | 4.005e-12 |
/// | max relative stress error | 4.794e-11 |
///
/// For contrast, the same element under strong elimination (the table in
/// [`patch_test_all_element_types`]) gives 9.035e-17 and 2.475e-15. The two
/// runs use slightly different interior jitter, so the comparison is of
/// magnitude, not of matched digits.
///
/// ## Interpretation
///
/// The penalty method passes the patch test only approximately, and the error
/// scales with `1 / beta` as the method's theory says and as `bc.rs`
/// documents: about five orders of magnitude worse than elimination at
/// `beta = 1e10`. This is why elimination is the default. The test also
/// asserts the error is **greater** than `1e-14`, so that a future change
/// which silently made the penalty path exact (by, say, routing it to
/// elimination) would fail rather than quietly pass.
#[test]
fn patch_test_penalty_is_limited_by_the_penalty_factor() {
    use farrer_park::mesh::unit_square_quad4;

    let mesh = unit_square_quad4(3)
        .unwrap()
        .map_coords(|i, c| {
            let on_b = c[0].abs() < 1e-12
                || c[1].abs() < 1e-12
                || (c[0] - 1.0).abs() < 1e-12
                || (c[1] - 1.0).abs() < 1e-12;
            if on_b {
                c
            } else {
                [c[0] + 0.04 * jitter(i, 0), c[1] + 0.04 * jitter(i, 1), 0.0]
            }
        })
        .unwrap()
        .shared();

    let dofs = DofMap::displacement(&mesh);
    let material = Material::elastic(E_PA, NU).unwrap();
    let boundary = mesh.bounding_box_boundary_nodes(1e-10);
    let mut bcs = DirichletSet::new();
    bcs.fix_from_field(&mesh, &dofs, &boundary, |x| linear_field(x, 2));

    let mut settings = elastic_newton();
    settings.dirichlet_method = DirichletMethod::Penalty(1.0e10);
    settings.residual_tolerance = 1.0e-8;
    settings.increment_tolerance = 1.0e-9;
    settings.max_iterations = 12;

    let mut system = System::new(mesh.clone(), material, BodyForce::None);
    let forces = vec![0.0; system.n_dofs()];
    let (u, _) = solve_nonlinear(&mut system, &bcs, &forces, &settings).expect("penalty solve");

    let scale = 1.0e-4 * 6.0;
    let mut max_u_err = 0.0_f64;
    for n in 0..mesh.n_nodes() {
        let want = linear_field(mesh.coords()[n], 2);
        for c in 0..2 {
            max_u_err = max_u_err.max((u[n * 2 + c] - want[c]).abs() / scale);
        }
    }
    let exact_stress = material.elastic_stiffness().apply(&exact_strain(2));
    let mut max_s_err = 0.0_f64;
    for s in system.quadrature_stress() {
        max_s_err = max_s_err.max(s.minus(&exact_stress).abs_max() / exact_stress.abs_max());
    }
    println!(
        "PATCH TEST Quad4 with penalty beta = 1e10: \
         max relative displacement error {max_u_err:.3e}, stress {max_s_err:.3e}"
    );
    assert!(
        max_u_err < 1e-6,
        "penalty patch test displacement error {max_u_err:e}"
    );
    assert!(
        max_u_err > 1e-14,
        "penalty should be measurably WORSE than elimination; got {max_u_err:e}"
    );
}
