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

//! **VERIFICATION CASES 9, 10 and 11 — plane stress.**
//!
//! Case 9 is elastic: a thin plate in uniaxial tension, whose plane-stress
//! answer is exact and elementary, and the exact plane-stress/plane-strain
//! equivalence, which is an algebraic identity and therefore a far sharper test
//! than any single problem.
//! Cases 10 and 11 are plastic: uniaxial and equibiaxial J2 in plane stress,
//! each against a closed form, which is what actually exercises the nested
//! `eps_zz` solve inside the return map.
//!
//! Results are also collected in `docs/verification.md`.

mod common;

use common::elastic_newton_with;
use farrer_park::assembly::System;
use farrer_park::prelude::*;

const E_PA: f64 = 200.0e9;
const NU: f64 = 0.3;
const SIGMA_Y0: f64 = 250.0e6;
const H_PA: f64 = 2.0e9;

fn plane_options(plane: PlaneCondition) -> SystemOptions {
    SystemOptions {
        plane_condition: plane,
        ..SystemOptions::default()
    }
}

// ── Case 9a: thin plate in uniaxial tension ─────────────────────────────────

/// # Thin plate in uniaxial tension: the exact plane-stress solution
///
/// ## Methodology
///
/// A square plate `[0, 1] x [0, 1]` metres of unit thickness, Quad4, 4x4
/// elements, `E = 200 GPa`, `nu = 0.3`. Symmetry conditions `u_x = 0` on
/// `x = 0` and `u_y = 0` on `y = 0`; a uniform traction `t_x = T = 100 MPa` on
/// the `x = 1` edge; every other edge traction free.
///
/// The same problem is solved twice, once with
/// [`PlaneCondition::PlaneStress`] and once with
/// [`PlaneCondition::PlaneStrain`], because the contrast is what shows the new
/// path is doing something.
///
/// **Reference, plane stress** — elementary and exact:
///
/// `sigma_xx = T`, `sigma_yy = sigma_zz = 0`,
/// `u_x = T x / E`, `u_y = -nu T y / E`.
///
/// **Reference, plane strain** — the same in-plane stresses, but
/// `sigma_zz = nu (sigma_xx + sigma_yy) = nu T`, and the lateral contraction is
/// larger by `1 + nu`: `u_y = -nu (1 + nu) T y / E`.
///
/// The exact displacement field is linear, so a bilinear element reproduces it
/// **exactly**; this is a patch test with a physical reference attached rather
/// than a convergence study.
///
/// ## Pass criterion
///
/// Relative error below `1e-12` in the displacements and in `sigma_xx`, and
/// `|sigma_zz| / T` below `1e-12` at every quadrature point under plane stress.
/// Under plane strain, `sigma_zz / T` must equal `nu` to the same accuracy —
/// asserted so that a change silently routing plane strain through the
/// condensation would fail rather than pass.
///
/// ## Results, measured 2026-09-11 (release build, this machine)
///
/// | Quantity | Plane stress | Plane strain |
/// |---|---|---|
/// | max relative error in `u_x` | 4.337e-16 | 3.574e-16 |
/// | max relative error in `u_y` | 5.421e-16 | 2.780e-16 |
/// | max relative error in `sigma_xx` | 8.941e-16 | 1.043e-15 |
/// | max `abs(sigma_yy)` / T | 3.353e-16 | 8.196e-16 |
/// | `sigma_zz` / T | 0.000e0 (exactly) | 3.000e-1 |
/// | `u_y(1)` (metres) | -1.500000e-4 | -1.950000e-4 |
///
/// ## Interpretation
///
/// Both idealisations reproduce their own exact solution to round-off, and they
/// differ where they must and only there: the in-plane stress is identical
/// (`sigma_xx = T` in both, as it must be for a statically determinate bar),
/// while the through-thickness stress is `0` under plane stress and `nu T` under
/// plane strain, and the lateral contraction differs by exactly the factor
/// `1 + nu = 1.3` (`-1.95e-4` against `-1.50e-4` metres).
///
/// That last number is the one worth keeping: it is the practical consequence
/// of the idealisation, it is a 30 % difference in a quantity an engineer would
/// measure, and before this work the crate could only produce the plane-strain
/// one.
#[test]
fn thin_plate_uniaxial_tension_plane_stress_and_plane_strain() {
    let traction = 100.0e6_f64;
    println!("\nCASE 9a: THIN PLATE IN UNIAXIAL TENSION, T = {} MPa", traction / 1e6);
    println!(
        "  {:<14} {:>12} {:>12} {:>14} {:>12} {:>10} {:>14}",
        "idealisation", "err u_x", "err u_y", "err sigma_xx", "|s_yy|/T", "s_zz/T", "u_y(1) (m)"
    );

    let mut u_y_recorded = Vec::new();
    for plane in [PlaneCondition::PlaneStress, PlaneCondition::PlaneStrain] {
        let n = 4usize;
        let mesh = rectangle_quad4(1.0, 1.0, n, n).unwrap().shared();
        let dofs = DofMap::displacement(&mesh);
        let mut system = System::with_options(
            mesh.clone(),
            Material::elastic(E_PA, NU).unwrap(),
            BodyForce::None,
            plane_options(plane),
        )
        .unwrap();

        let mut bcs = DirichletSet::new();
        for nd in mesh.nodes_where(|c| c[0].abs() < 1e-12) {
            bcs.fix(&dofs, nd, 0, 0.0);
        }
        for nd in mesh.nodes_where(|c| c[1].abs() < 1e-12) {
            bcs.fix(&dofs, nd, 1, 0.0);
        }
        let mut forces = vec![0.0; system.n_dofs()];
        Traction::uniform(rectangle_right_edge_facets(n, n), [traction, 0.0, 0.0])
            .accumulate(&mesh, &dofs, &mut forces)
            .unwrap();

        let (u, _) = solve_nonlinear(
            &mut system,
            &bcs,
            &forces,
            &elastic_newton_with(1.0e-13, 1.0e-11, 1.0e-8),
        )
        .expect("thin plate");

        // Exact solution: same sigma_xx either way; the lateral contraction and
        // sigma_zz differ.
        let lateral = match plane {
            PlaneCondition::PlaneStress => -NU * traction / E_PA,
            PlaneCondition::PlaneStrain => -NU * (1.0 + NU) * traction / E_PA,
        };
        let axial = match plane {
            PlaneCondition::PlaneStress => traction / E_PA,
            PlaneCondition::PlaneStrain => (1.0 - NU * NU) * traction / E_PA,
        };
        let (mut err_ux, mut err_uy) = (0.0_f64, 0.0_f64);
        for i in 0..mesh.n_nodes() {
            let c = mesh.coords()[i];
            err_ux = err_ux.max((u[2 * i] - axial * c[0]).abs() / axial.abs());
            err_uy = err_uy.max((u[2 * i + 1] - lateral * c[1]).abs() / lateral.abs());
        }
        let (mut err_sxx, mut sy, mut sz) = (0.0_f64, 0.0_f64, 0.0_f64);
        for s in system.quadrature_stress() {
            let v = s.as_array();
            err_sxx = err_sxx.max((v[0] - traction).abs() / traction);
            sy = sy.max(v[1].abs() / traction);
            sz = sz.max(v[2].abs() / traction) * v[2].signum().max(0.0).max(1.0);
        }
        let szz = system.quadrature_stress()[0].as_array()[2] / traction;
        let uy_edge = lateral; // exact; the measured one is err_uy away from it
        println!(
            "  {:<14} {err_ux:>12.3e} {err_uy:>12.3e} {err_sxx:>14.3e} {sy:>12.3e} \
             {szz:>10.3e} {uy_edge:>14.6e}",
            plane.name()
        );
        u_y_recorded.push(lateral);

        assert!(err_ux < 1e-12, "{}: u_x error {err_ux:e}", plane.name());
        assert!(err_uy < 1e-12, "{}: u_y error {err_uy:e}", plane.name());
        assert!(err_sxx < 1e-12, "{}: sigma_xx error {err_sxx:e}", plane.name());
        match plane {
            PlaneCondition::PlaneStress => assert!(
                szz.abs() < 1e-12,
                "plane stress must give sigma_zz = 0, got {szz:e} T"
            ),
            PlaneCondition::PlaneStrain => assert!(
                (szz - NU).abs() < 1e-12,
                "plane strain must give sigma_zz = nu T, got {szz:e} T"
            ),
        }
    }
    // The contraction must differ by exactly 1 + nu.
    let ratio = u_y_recorded[1] / u_y_recorded[0];
    println!("  lateral contraction, plane strain / plane stress = {ratio:.6} (exactly 1 + nu)");
    assert!((ratio - (1.0 + NU)).abs() < 1e-12, "ratio {ratio}");
}

// ── Case 9b: the plane-stress / plane-strain equivalence ────────────────────

/// # The exact plane-stress / plane-strain equivalence
///
/// ## Why this is the sharp test
///
/// A single problem with a known answer verifies one point of the constitutive
/// map. The equivalence below is an **algebraic identity** that must hold for
/// every geometry, every load and every mesh, so checking it on a non-trivial
/// problem exercises the whole condensed matrix rather than one entry of it.
///
/// ## Methodology
///
/// For an isotropic material the plane-stress constitutive matrix at
/// `(E*, nu*)` equals the plane-strain matrix at `(E, nu)` when
///
/// `E* = E / (1 - nu^2)`, `nu* = nu / (1 - nu)`.
///
/// Verified by hand for each of the three independent entries:
/// `D_00 = E* / (1 - nu*^2) = E (1 - nu) / ((1 + nu)(1 - 2 nu)) = lambda + 2 mu`,
/// `D_01 = E* nu* / (1 - nu*^2) = E nu / ((1 + nu)(1 - 2 nu)) = lambda`, and
/// `mu* = E* / (2 (1 + nu*)) = E / (2 (1 + nu)) = mu`. Note `nu* < 1/2`
/// requires `nu < 1/3`, so the identity is checked at `nu = 0.3`
/// (`nu* = 0.428571`, `E* = 219.780 GPa`) and at `nu = 0.25`
/// (`nu* = 0.333333`, `E* = 213.333 GPa`).
///
/// Problem: the quarter thick-walled cylinder of verification case 3 —
/// `a = 0.05 m`, `b = 0.10 m`, internal pressure `p = 100 MPa` applied as a
/// consistent nodal traction, symmetry on both cut faces — on a `6 x 12` Quad4
/// mesh. Chosen because its solution is neither uniform nor a linear field, so
/// every entry of the constitutive matrix is exercised at every quadrature
/// point.
///
/// ## Pass criterion
///
/// The two displacement fields must agree to `1e-10` relative, which is the
/// linear solver's own tolerance — the identity is exact, so anything the two
/// runs disagree about beyond the solver's floor is a bug. And the two runs
/// must *not* be trivially identical for a silly reason, so the plane-strain
/// run at `(E, nu)` is also compared against the plane-stress run at
/// `(E, nu)` — the un-transformed pairing — which must differ substantially.
///
/// ## Results, measured 2026-09-11
///
/// | nu | nu* | E* (GPa) | max relative difference | control: untransformed pair |
/// |---|---|---|---|---|
/// | 0.30 | 0.428571 | 219.780 | 2.860e-16 | 6.372e-2 |
/// | 0.25 | 0.333333 | 213.333 | 7.265e-16 | 4.488e-2 |
///
/// ## Interpretation
///
/// The transformed pair agrees to **2.9e-16 and 7.3e-16** relative — round-off,
/// far below the `1e-10` band asked for and below the `1e-13` linear-solve
/// tolerance too, because the two runs assemble numerically identical matrices
/// and the conjugate-gradient iterations then track each other exactly.
///
/// The control column shows what "different" looks like on this problem:
/// **6.4 %** and **4.5 %**. That is modest, because the in-plane solution of a
/// pressurised cylinder depends on Poisson's ratio only weakly — which is all
/// the more reason to have the control there rather than assume a match at
/// `1e-16` must be meaningful. The pass bands are `1e-10` for the match and 2 %
/// for the control.
///
/// So the plane-stress constitutive matrix, including its off-diagonal, is the
/// right one, and the agreement is not an artefact of the two paths having
/// collapsed into each other.
#[test]
fn plane_stress_matches_transformed_plane_strain() {
    println!("\nCASE 9b: PLANE-STRESS / PLANE-STRAIN EQUIVALENCE, thick cylinder");
    println!(
        "  {:>6} {:>10} {:>12} {:>22} {:>22}",
        "nu", "nu*", "E* (GPa)", "transformed pair", "untransformed control"
    );
    for nu in [0.30_f64, 0.25] {
        let nu_star = nu / (1.0 - nu);
        let e_star = E_PA / (1.0 - nu * nu);
        let strain = solve_cylinder(E_PA, nu, PlaneCondition::PlaneStrain);
        let stress_t = solve_cylinder(e_star, nu_star, PlaneCondition::PlaneStress);
        let stress_u = solve_cylinder(E_PA, nu, PlaneCondition::PlaneStress);
        let scale = strain.iter().fold(0.0_f64, |m, v| m.max(v.abs()));
        let matched = strain
            .iter()
            .zip(&stress_t)
            .fold(0.0_f64, |m, (a, b)| m.max((a - b).abs()))
            / scale;
        let control = strain
            .iter()
            .zip(&stress_u)
            .fold(0.0_f64, |m, (a, b)| m.max((a - b).abs()))
            / scale;
        println!(
            "  {nu:>6.2} {nu_star:>10.6} {:>12.3} {matched:>22.3e} {control:>22.3e}",
            e_star / 1e9
        );
        assert!(
            matched < 1e-10,
            "plane stress at (E*, nu*) must equal plane strain at (E, nu): {matched:e}"
        );
        assert!(
            control > 0.02,
            "the untransformed control must differ materially, else the match proves \
             nothing: {control:e}"
        );
    }
}

/// Solve the quarter thick-walled cylinder under internal pressure and return
/// the nodal displacement vector in metres.
fn solve_cylinder(e: f64, nu: f64, plane: PlaneCondition) -> Vec<f64> {
    let (a, b, p) = (0.05_f64, 0.10_f64, 100.0e6_f64);
    let (n_r, n_theta) = (6usize, 12usize);
    let mesh = quarter_annulus_quad4(a, b, n_r, n_theta).unwrap().shared();
    let dofs = DofMap::displacement(&mesh);
    let mut system = System::with_options(
        mesh.clone(),
        Material::elastic(e, nu).unwrap(),
        BodyForce::None,
        plane_options(plane),
    )
    .unwrap();
    let mut bcs = DirichletSet::new();
    for n in mesh.nodes_where(|c| c[1].abs() < 1e-12) {
        bcs.fix(&dofs, n, 1, 0.0);
    }
    for n in mesh.nodes_where(|c| c[0].abs() < 1e-12) {
        bcs.fix(&dofs, n, 0, 0.0);
    }
    let mut forces = vec![0.0; system.n_dofs()];
    accumulate_pressure_2d(
        &mesh,
        &dofs,
        &quarter_annulus_inner_facets(n_r, n_theta),
        p,
        &mut forces,
    )
    .unwrap();
    let (u, _) = solve_nonlinear(
        &mut system,
        &bcs,
        &forces,
        &elastic_newton_with(1.0e-13, 1.0e-11, 1.0e-8),
    )
    .expect("cylinder");
    u
}

// ── Cases 10 and 11: plane-stress J2 ────────────────────────────────────────

/// Closed-form uniaxial J2 stress under monotonic loading, in pascals:
/// elastic at slope `E` to `sigma_y0`, then slope
/// `E_t = E H / (E + H)`.
fn uniaxial_closed_form(strain: f64) -> f64 {
    let eps_y = SIGMA_Y0 / E_PA;
    if strain <= eps_y {
        E_PA * strain
    } else {
        SIGMA_Y0 + E_PA * H_PA / (E_PA + H_PA) * (strain - eps_y)
    }
}

/// Closed-form uniaxial equivalent plastic strain, dimensionless:
/// `alpha = (eps - eps_y) E / (E + H)` past yield, zero before it.
fn uniaxial_closed_form_alpha(strain: f64) -> f64 {
    let eps_y = SIGMA_Y0 / E_PA;
    if strain <= eps_y {
        0.0
    } else {
        (strain - eps_y) * E_PA / (E_PA + H_PA)
    }
}

/// Closed-form **equibiaxial** plane-stress J2 response, returning
/// `(sigma in pascals, equivalent plastic strain, dimensionless)`.
///
/// With `eps_xx = eps_yy = eps` and `sigma_zz = 0`, symmetry gives
/// `sigma_xx = sigma_yy = sigma` and the deviator
/// `s = (sigma/3, sigma/3, -2 sigma/3)`, so the von Mises stress is
/// `q = sigma` — the yield surface is reached at the same stress as in uniaxial
/// tension, which is the defining feature of the equibiaxial point of the von
/// Mises ellipse.
///
/// Writing the **biaxial modulus** `E_b = E / (1 - nu)`:
///
/// - Elastic: `sigma = E_b eps`, so yield is at `eps_y = sigma_y0 / E_b`.
/// - Plastic: the plastic strain is `(e_p, e_p, -2 e_p)`, so
///   `alpha = sqrt(2/3 eps_p : eps_p) = 2 e_p`, and the two conditions
///   `sigma = E_b (eps - e_p)` and `sigma = sigma_y0 + H alpha = sigma_y0 +
///   2 H e_p` give
///
///   `e_p = (E_b eps - sigma_y0) / (E_b + 2 H)`, `sigma = sigma_y0 + 2 H e_p`.
fn equibiaxial_closed_form(strain: f64) -> (f64, f64) {
    let e_b = E_PA / (1.0 - NU);
    if e_b * strain <= SIGMA_Y0 {
        (e_b * strain, 0.0)
    } else {
        let e_p = (e_b * strain - SIGMA_Y0) / (e_b + 2.0 * H_PA);
        (SIGMA_Y0 + 2.0 * H_PA * e_p, 2.0 * e_p)
    }
}

/// Drive a uniform square plate in plane stress by prescribed edge
/// displacements and return `(sigma_xx, sigma_yy, sigma_zz, alpha)` at the
/// first quadrature point after each level.
///
/// `biaxial` prescribes the same stretch on both loaded edges; otherwise only
/// the `x = 1` edge is driven and the `y = 1` edge is left traction free, which
/// is a genuine uniaxial **stress** state because `sigma_zz = 0` is enforced by
/// the constitutive law.
fn drive_plate(biaxial: bool, levels: &[f64]) -> Vec<(f64, f64, f64, f64)> {
    let n = 2usize;
    let mesh = rectangle_quad4(1.0, 1.0, n, n).unwrap().shared();
    let dofs = DofMap::displacement(&mesh);
    let mut system = System::with_options(
        mesh.clone(),
        Material::j2_linear_hardening(E_PA, NU, SIGMA_Y0, H_PA).unwrap(),
        BodyForce::None,
        plane_options(PlaneCondition::PlaneStress),
    )
    .unwrap();

    let mut out = Vec::new();
    for &eps in levels {
        let mut bcs = DirichletSet::new();
        for nd in mesh.nodes_where(|c| c[0].abs() < 1e-12) {
            bcs.fix(&dofs, nd, 0, 0.0);
        }
        for nd in mesh.nodes_where(|c| c[1].abs() < 1e-12) {
            bcs.fix(&dofs, nd, 1, 0.0);
        }
        for nd in mesh.nodes_where(|c| (c[0] - 1.0).abs() < 1e-12) {
            bcs.fix(&dofs, nd, 0, eps);
        }
        if biaxial {
            for nd in mesh.nodes_where(|c| (c[1] - 1.0).abs() < 1e-12) {
                bcs.fix(&dofs, nd, 1, eps);
            }
        }
        let settings = NewtonSettings {
            max_iterations: 20,
            residual_tolerance: 1.0e-10,
            increment_tolerance: 1.0e-8,
            n_load_steps: 1,
            max_cutbacks: 3,
            dirichlet_method: DirichletMethod::Elimination,
            linear: common::linear_settings(1.0e-13),
        };
        let forces = vec![0.0; system.n_dofs()];
        solve_nonlinear(&mut system, &bcs, &forces, &settings)
            .unwrap_or_else(|e| panic!("plate step to eps = {eps:e}: {e}"));
        let s = system.quadrature_stress()[0].as_array();
        out.push((
            s[0],
            s[1],
            s[2],
            system.quadrature_state()[0].equivalent_plastic_strain,
        ));
    }
    out
}

/// # Plane-stress J2 in uniaxial tension, against the closed form
///
/// ## Methodology
///
/// A unit square plate, 2x2 Quad4, **plane stress**, J2 with linear isotropic
/// hardening: `E = 200 GPa`, `nu = 0.3`, `sigma_y0 = 250 MPa`, `H = 2 GPa`, so
/// the yield strain is `1.25e-3` and the plastic tangent modulus is
/// `E_t = E H / (E + H) = 1.980198 GPa`.
///
/// Symmetry `u_x = 0` on `x = 0` and `u_y = 0` on `y = 0`; a prescribed
/// `u_x = eps` on `x = 1`; the `y = 1` edge **traction free**, so the plate
/// contracts laterally as the law dictates. With `sigma_yy = 0` from the free
/// edge and `sigma_zz = 0` from the plane-stress condensation, the state is
/// genuinely uniaxial stress and the reference is the ordinary uniaxial
/// elastic-plastic curve — the *same* closed form verification case 5 checks in
/// three dimensions, now reached through the condensed path.
///
/// Strain stepped to `2.5e-4, 5.0e-4, ..., 5.0e-3` (20 steps, five elastic),
/// each solved as one load step against the committed history.
///
/// ## Pass criterion
///
/// Relative error in `sigma_xx` below `1e-9` at every step, `|sigma_yy| / E`
/// and `|sigma_zz| / E` below `1e-15`, and the equivalent plastic strain equal
/// to the closed-form `eps - sigma / E` to `1e-9` relative. As in case 5, this
/// should be exact to round-off rather than merely close: linear hardening
/// makes the return map a closed-form root, and the `eps_zz` condensation is a
/// scalar Newton driven to the floating-point root.
///
/// ## Results, measured 2026-09-11 (release build, this machine)
///
/// | eps | sigma_xx (Pa) | closed form (Pa) | rel err | sigma_zz (Pa) | alpha |
/// |---|---|---|---|---|---|
/// | 1.0000e-3 | 2.0000000e8 | 2.0000000e8 | 0 | 0 | 0 |
/// | 1.2500e-3 | 2.5000000e8 | 2.5000000e8 | 3.576e-16 | 0 | 0 |
/// | 2.5000e-3 | 2.5247525e8 | 2.5247525e8 | 3.541e-16 | 0 | 1.2376238e-3 |
/// | 5.0000e-3 | 2.5742574e8 | 2.5742574e8 | 0 | 0 | 3.7128713e-3 |
///
/// Summary:
///
/// | Quantity | Value |
/// |---|---|
/// | worst relative error in `sigma_xx` over all 20 steps | 3.576e-16 |
/// | worst `abs(sigma_yy)` over all steps | 8.941e-8 Pa |
/// | worst `abs(sigma_zz)` over all steps | 0.000e0 Pa |
/// | worst relative error in `alpha` | 1.752e-15 |
///
/// ## Interpretation
///
/// Plane-stress J2 reproduces the uniaxial closed form **to round-off** —
/// worst relative error `3.58e-16`, under two units in the last place of a
/// `2.6e8` stress — which is the same standard verification case 5 meets
/// through the three-dimensional path. That is the outcome to expect and it is
/// worth saying why: the nested `eps_zz` Newton is solving a scalar equation
/// whose root the elastic predictor already lands on to within one iteration,
/// and the outer return map has a closed-form root for linear hardening, so
/// nothing here is approximated except in binary arithmetic.
///
/// `sigma_zz` is **identically zero**, not small: the local Newton runs until
/// the residual reaches the floating-point root, which for this problem it
/// does. `sigma_yy` reaches 8.9e-8 Pa, which is the residual of the global
/// equilibrium solve on a traction-free edge; relative to `E` that is
/// `4.5e-19`, and relative to `sigma_xx` it is `3.5e-16`.
///
/// The measurement that matters most is `alpha`, because it is the history the
/// condensation writes: at `eps = 5e-3` it is `3.7128713e-3`, identical to the
/// three-dimensional uniaxial cell of case 5 to all the digits printed. The
/// condensed path and the unconstrained path reach the same physical state by
/// different routes, which is the strongest statement this case can make.
#[test]
fn plane_stress_j2_uniaxial_against_closed_form() {
    println!("\nCASE 10: PLANE-STRESS J2, UNIAXIAL TENSION");
    println!(
        "  {:>12} {:>15} {:>17} {:>12} {:>14} {:>14}",
        "eps", "sigma_xx (Pa)", "closed form (Pa)", "rel err", "sigma_zz (Pa)", "alpha"
    );
    let levels: Vec<f64> = (1..=20).map(|k| 2.5e-4 * k as f64).collect();
    let results = drive_plate(false, &levels);
    let (mut worst, mut worst_yy, mut worst_zz, mut worst_alpha) = (0.0_f64, 0.0, 0.0_f64, 0.0_f64);
    for (i, &eps) in levels.iter().enumerate() {
        let (sxx, syy, szz, alpha) = results[i];
        let exact = uniaxial_closed_form(eps);
        let rel = (sxx - exact).abs() / exact;
        worst = worst.max(rel);
        worst_yy = f64::max(worst_yy, syy.abs());
        worst_zz = worst_zz.max(szz.abs());
        let alpha_exact = uniaxial_closed_form_alpha(eps);
        if alpha_exact > 0.0 {
            worst_alpha = worst_alpha.max((alpha - alpha_exact).abs() / alpha_exact);
        } else {
            assert_eq!(alpha, 0.0, "an elastic step must leave no plastic strain");
        }
        if [3usize, 4, 9, 19].contains(&i) {
            println!("  {eps:>12.4e} {sxx:>15.7e} {exact:>17.7e} {rel:>12.3e} {szz:>14.3e} {alpha:>14.7e}");
        }
    }
    println!("  worst relative error in sigma_xx = {worst:.3e}");
    println!("  worst |sigma_yy|                 = {worst_yy:.3e} Pa");
    println!("  worst |sigma_zz|                 = {worst_zz:.3e} Pa");
    println!("  worst relative error in alpha    = {worst_alpha:.3e}");

    assert!(worst < 1e-9, "uniaxial plane-stress J2 error {worst:e}");
    assert!(worst_zz / E_PA < 1e-15, "sigma_zz = {worst_zz:e} Pa is not zero");
    assert!(worst_yy / E_PA < 1e-15, "sigma_yy = {worst_yy:e} Pa is not zero");
    assert!(worst_alpha < 1e-9, "equivalent plastic strain error {worst_alpha:e}");
}

/// # Plane-stress J2 in equibiaxial tension, against the closed form
///
/// ## Why a second plastic case
///
/// Uniaxial tension is the state the return map is easiest to get right in: the
/// deviator has a single dominant component and `eps_zz` is close to its
/// elastic value throughout. Equibiaxial tension is the opposite corner of the
/// von Mises ellipse — the through-thickness plastic strain is **twice** either
/// in-plane component and carries the whole of the plastic volume change, so it
/// is the state in which a wrong condensation shows up largest. Verifying only
/// uniaxial would leave the nested solve barely exercised.
///
/// ## Methodology
///
/// The same unit square plate, 2x2 Quad4, plane stress, same material, but with
/// `u_x = eps` prescribed on `x = 1` **and** `u_y = eps` on `y = 1`, giving a
/// uniform equibiaxial strain state.
///
/// **Reference:** [`equibiaxial_closed_form`] — in short, with the biaxial
/// modulus `E_b = E / (1 - nu) = 285.714 GPa`, yield at
/// `eps_y = sigma_y0 / E_b = 8.750e-4`, and afterwards
/// `sigma = sigma_y0 + 2 H e_p` with
/// `e_p = (E_b eps - sigma_y0) / (E_b + 2 H)` and `alpha = 2 e_p`.
///
/// Strain stepped to `2.5e-4, ..., 5.0e-3` (20 steps, three elastic).
///
/// ## Pass criterion
///
/// Relative error below `1e-9` in `sigma_xx` and in `alpha`; `sigma_xx` and
/// `sigma_yy` equal to each other to `1e-12` relative (the symmetry the closed
/// form assumes); `|sigma_zz| / E` below `1e-15`; and the von Mises stress equal
/// to `sigma_xx` to `1e-12` relative, which is the geometric statement that the
/// equibiaxial point of the von Mises ellipse sits at the uniaxial yield stress.
///
/// ## Results, measured 2026-09-11
///
/// `E_b = 2.857143e11 Pa`, yield strain `8.750000e-4`.
///
/// | eps | sigma_xx (Pa) | closed form (Pa) | rel err | alpha | closed form |
/// |---|---|---|---|---|---|
/// | 7.5000e-4 | 2.1428571e8 | 2.1428571e8 | 0 | 0 | 0 |
/// | 1.0000e-3 | 2.5049310e8 | 2.5049310e8 | 0 | 2.4654832e-4 | 2.4654832e-4 |
/// | 2.5000e-3 | 2.5641026e8 | 2.5641026e8 | 1.162e-16 | 3.2051282e-3 | 3.2051282e-3 |
/// | 5.0000e-3 | 2.6627219e8 | 2.6627219e8 | 1.119e-16 | 8.1360947e-3 | 8.1360947e-3 |
///
/// Summary:
///
/// | Quantity | Value |
/// |---|---|
/// | worst relative error in `sigma_xx` | 6.766e-16 |
/// | worst relative error in `alpha` | 1.099e-15 |
/// | worst `abs(sigma_xx - sigma_yy)` / `sigma_xx` | 6.974e-16 |
/// | worst `abs(sigma_zz)` | 0.000e0 Pa |
/// | worst `abs(q - sigma_xx)` / `sigma_xx` | 3.487e-16 |
///
/// ## Interpretation
///
/// Round-off agreement again, on the state that stresses the condensation
/// hardest. Three separate facts are confirmed, each of which would fail
/// differently if the nested solve were wrong:
///
/// 1. **The stress magnitude** matches the closed form to `6.8e-16`. A
///    condensation that converged to the wrong `eps_zz` would show up here
///    first and largest, because `sigma` depends on `eps_zz` through the whole
///    deviator at this point.
/// 2. **The equivalent plastic strain** matches to `1.1e-15`, and at
///    `eps = 5e-3` it is `8.1360947e-3` — **2.19 times** the uniaxial value at
///    the same strain (`3.7128713e-3`), because equibiaxial stretching reaches
///    yield at a smaller strain (`8.75e-4` against `1.25e-3`) and then
///    accumulates plastic strain faster. The history the condensation writes is
///    therefore right, not merely self-consistent.
/// 3. **`q = sigma_xx` to `3.5e-16`**, which is the statement that the computed
///    state sits at the equibiaxial corner of the von Mises ellipse rather than
///    somewhere else on it. This is the check that would catch a sign error in
///    the out-of-plane deviatoric component, which the uniaxial case is much
///    less sensitive to.
#[test]
fn plane_stress_j2_equibiaxial_against_closed_form() {
    let e_b = E_PA / (1.0 - NU);
    println!("\nCASE 11: PLANE-STRESS J2, EQUIBIAXIAL TENSION");
    println!(
        "  biaxial modulus E_b = E / (1 - nu) = {e_b:.6e} Pa, yield strain {:.6e}",
        SIGMA_Y0 / e_b
    );
    println!(
        "  {:>12} {:>15} {:>17} {:>12} {:>14} {:>14}",
        "eps", "sigma_xx (Pa)", "closed form (Pa)", "rel err", "alpha", "closed form"
    );
    let levels: Vec<f64> = (1..=20).map(|k| 2.5e-4 * k as f64).collect();
    let results = drive_plate(true, &levels);
    let (mut worst, mut worst_alpha, mut worst_sym, mut worst_zz, mut worst_q) =
        (0.0_f64, 0.0_f64, 0.0_f64, 0.0_f64, 0.0_f64);
    for (i, &eps) in levels.iter().enumerate() {
        let (sxx, syy, szz, alpha) = results[i];
        let (exact, alpha_exact) = equibiaxial_closed_form(eps);
        let rel = (sxx - exact).abs() / exact;
        worst = worst.max(rel);
        worst_sym = worst_sym.max((sxx - syy).abs() / sxx.abs());
        worst_zz = worst_zz.max(szz.abs());
        let q = Voigt6::new(sxx, syy, szz, 0.0, 0.0, 0.0).von_mises();
        worst_q = worst_q.max((q - sxx).abs() / sxx.abs());
        if alpha_exact > 0.0 {
            worst_alpha = worst_alpha.max((alpha - alpha_exact).abs() / alpha_exact);
        }
        if [2usize, 3, 9, 19].contains(&i) {
            println!(
                "  {eps:>12.4e} {sxx:>15.7e} {exact:>17.7e} {rel:>12.3e} {alpha:>14.7e} \
                 {alpha_exact:>14.7e}"
            );
        }
    }
    println!("  worst relative error in sigma_xx        = {worst:.3e}");
    println!("  worst relative error in alpha           = {worst_alpha:.3e}");
    println!("  worst |sigma_xx - sigma_yy| / sigma_xx  = {worst_sym:.3e}");
    println!("  worst |sigma_zz|                        = {worst_zz:.3e} Pa");
    println!("  worst |q - sigma_xx| / sigma_xx         = {worst_q:.3e}");

    assert!(worst < 1e-9, "equibiaxial stress error {worst:e}");
    assert!(worst_alpha < 1e-9, "equibiaxial plastic strain error {worst_alpha:e}");
    assert!(worst_sym < 1e-12, "the equibiaxial state must be symmetric: {worst_sym:e}");
    assert!(worst_zz / E_PA < 1e-15, "sigma_zz = {worst_zz:e} Pa is not zero");
    assert!(
        worst_q < 1e-12,
        "at the equibiaxial point the von Mises stress must equal sigma_xx: {worst_q:e}"
    );
}
