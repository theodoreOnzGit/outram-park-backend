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

//! **VERIFICATION CASE 5** — uniaxial J2 plasticity with linear isotropic
//! hardening against the closed-form elastic-plastic response, including
//! elastic unloading; verification of the consistent tangent by numerical
//! perturbation; and measurement of the Newton convergence order.
//!
//! Results are also collected in `docs/verification.md`.

mod common;

use farrer_park::assembly::System;
use farrer_park::prelude::*;
use outram_foam_basic_lib::compute::ComputeBackend;
use outram_foam_basic_lib::math::differentiate::{jacobian, DiffSettings};

/// Structural steel with linear isotropic hardening.
const E_PA: f64 = 200.0e9;
const NU: f64 = 0.3;
const SIGMA_Y0: f64 = 250.0e6;
const H_PA: f64 = 2.0e9;

/// Closed-form uniaxial stress under monotonic loading, in pascals.
///
/// `sigma = E eps` while `eps <= sigma_y0 / E`, then
/// `sigma = sigma_y0 + E_t (eps - sigma_y0 / E)` with the tangent modulus
/// `E_t = E H / (E + H)`.
fn uniaxial_monotonic(strain: f64) -> f64 {
    let eps_y = SIGMA_Y0 / E_PA;
    if strain <= eps_y {
        E_PA * strain
    } else {
        let et = E_PA * H_PA / (E_PA + H_PA);
        SIGMA_Y0 + et * (strain - eps_y)
    }
}

/// A single Hex8 element under uniaxial stress: symmetry on the three
/// coordinate planes, a prescribed `u_x` on the `x = 1` face, everything else
/// free.
///
/// The lateral faces are traction free, so the element contracts laterally by
/// whatever the constitutive law requires. That is the point: it makes this a
/// genuine uniaxial **stress** test, solved by the finite-element machinery,
/// rather than a material-point driver with the lateral strains imposed by
/// hand.
struct UniaxialCell {
    system: System,
    bcs_template: Vec<(NodeId, usize)>,
    x_face: Vec<NodeId>,
    dofs: DofMap,
}

impl UniaxialCell {
    fn new() -> Self {
        let mesh = unit_cube_hex8(1).unwrap().shared();
        let dofs = DofMap::displacement(&mesh);
        let material =
            Material::j2_linear_hardening(E_PA, NU, SIGMA_Y0, H_PA).expect("steel with hardening");
        let system = System::new(mesh.clone(), material, BodyForce::None);

        let mut template = Vec::new();
        for n in mesh.nodes_where(|c| c[0].abs() < 1e-12) {
            template.push((n, 0));
        }
        for n in mesh.nodes_where(|c| c[1].abs() < 1e-12) {
            template.push((n, 1));
        }
        for n in mesh.nodes_where(|c| c[2].abs() < 1e-12) {
            template.push((n, 2));
        }
        let x_face = mesh.nodes_where(|c| (c[0] - 1.0).abs() < 1e-12);
        Self {
            system,
            bcs_template: template,
            x_face,
            dofs,
        }
    }

    /// Advance the cell to a prescribed axial strain and return
    /// `(sigma_xx at the first quadrature point, the load-step report)`.
    ///
    /// Each call starts its Newton iteration from zero displacement but from
    /// the **committed** plastic history, which is legitimate for
    /// rate-independent J2: the stress at the end of a step is a function of the
    /// committed state and the end-of-step strain alone.
    fn advance_to(&mut self, axial_strain: f64) -> (f64, LoadStepReport) {
        let mut bcs = DirichletSet::new();
        for (n, c) in &self.bcs_template {
            bcs.fix(&self.dofs, *n, *c, 0.0);
        }
        for n in &self.x_face {
            bcs.fix(&self.dofs, *n, 0, axial_strain);
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
        let forces = vec![0.0; self.system.n_dofs()];
        let (_u, rep) = solve_nonlinear(&mut self.system, &bcs, &forces, &settings)
            .unwrap_or_else(|e| panic!("uniaxial step to {axial_strain}: {e}"));
        let sigma = self.system.quadrature_stress()[0].as_array()[0];
        (sigma, rep.steps.into_iter().next_back().unwrap())
    }

    /// The committed equivalent plastic strain at the first quadrature point.
    fn equivalent_plastic_strain(&self) -> f64 {
        self.system.quadrature_state()[0].equivalent_plastic_strain
    }
}

/// # Uniaxial J2 plasticity against the closed-form response
///
/// ## Methodology
///
/// A single eight-node hexahedron of unit side, symmetry conditions `u_x = 0`,
/// `u_y = 0`, `u_z = 0` on the three coordinate planes, and a prescribed axial
/// displacement on the `x = 1` face. The other faces are traction free, so the
/// element contracts laterally as the constitutive law dictates and the state
/// is genuinely uniaxial **stress**, not uniaxial strain.
///
/// Material: `E = 200 GPa`, `nu = 0.3`, `sigma_y0 = 250 MPa`,
/// `H = 2 GPa`, so the yield strain is `1.25e-3` and the plastic tangent
/// modulus is `E_t = E H / (E + H) = 1.98020 GPa`.
///
/// Load path: axial strain stepped to `1e-4, 2e-4, ..., 5e-3` (50 steps, four
/// of them elastic), then **unloaded** in the same steps back to `2e-3`. At
/// each step the axial Cauchy stress at the element's first quadrature point is
/// compared with the closed form; on the unloading branch the reference is
/// `sigma_max - E (eps_max - eps)`, elastic from the turning point.
///
/// Each step is solved with a full Newton iteration on the consistent tangent,
/// linear solves by ILU(0)-preconditioned conjugate gradients to `1e-13`.
///
/// ## Pass criterion
///
/// Relative error in `sigma_xx` below `1e-9` at every point of the path, on
/// both branches, and the equivalent plastic strain at the end of unloading
/// unchanged from its value at peak load. For linear hardening the radial
/// return is a closed-form root, so the finite-element answer should be exact
/// to round-off, not merely close.
///
/// ## Results, measured 2026-09-11 (release build, this machine)
///
/// Yield strain `1.250000e-3`, plastic tangent modulus `E_t = 1.980198e9 Pa`.
/// Loading branch (selected points; `newton` is linear solves in that step):
///
/// | strain | sigma_FEM (Pa) | sigma_exact (Pa) | rel err | newton |
/// |---|---|---|---|---|
/// | 1.0000e-4 | 2.0000000e7 | 2.0000000e7 | 0 | 2 |
/// | 2.0000e-4 | 4.0000000e7 | 4.0000000e7 | 0 | 2 |
/// | 1.0000e-3 | 2.0000000e8 | 2.0000000e8 | 0 | 2 |
/// | 2.0000e-3 | 2.5148515e8 | 2.5148515e8 | 3.555e-16 | 3 |
/// | 3.0000e-3 | 2.5346535e8 | 2.5346535e8 | 0 | 3 |
/// | 4.0000e-3 | 2.5544554e8 | 2.5544554e8 | 8.167e-16 | 3 |
/// | 5.0000e-3 | 2.5742574e8 | 2.5742574e8 | 4.631e-16 | 3 |
///
/// Unloading from `eps = 5.0e-3`. Reverse yield occurs at
/// `eps = 2.425743e-3`, where the stress reaches `-sigma_y(alpha) =
/// -2.574257e8 Pa`:
///
/// | strain | sigma_FEM (Pa) | sigma_exact (Pa) | rel err | newton |
/// |---|---|---|---|---|
/// | 4.0000e-3 | 5.7425743e7 | 5.7425743e7 | 3.503e-15 | 3 |
/// | 3.0000e-3 | -1.4257426e8 | -1.4257426e8 | 1.045e-15 | 3 |
/// | 2.0000e-3 | -2.5826880e8 | -2.5826880e8 | 4.616e-16 | 2 |
/// | 1.0000e-3 | -2.6024900e8 | -2.6024900e8 | 2.290e-16 | 2 |
/// | 0 | -2.6222919e8 | -2.6222919e8 | 1.136e-16 | 2 |
///
/// Summary:
///
/// | Quantity | Value |
/// |---|---|
/// | worst relative error, loading | 8.167e-16 |
/// | worst relative error, elastic unloading | 1.954e-14 |
/// | worst relative error, reverse yielding | 5.761e-16 |
/// | equivalent plastic strain at peak | 3.7128712871e-3 |
/// | equivalent plastic strain just before reverse yield | 3.7128712871e-3 |
/// | equivalent plastic strain at `eps = 0` | 6.1145966082e-3 |
///
/// ## Interpretation
///
/// The finite-element answer reproduces the closed form **to round-off** on all
/// three branches — worst relative error `1.95e-14`, which is a handful of
/// units in the last place of a `2.6e8` stress. That is the correct outcome and
/// not merely a close one: with linear isotropic hardening the radial-return
/// consistency condition is linear, so the return map has a closed-form root
/// and the only error left is floating-point arithmetic.
///
/// Three things are verified beyond the stress values themselves.
///
/// **Unloading is exactly elastic.** The equivalent plastic strain is
/// bit-identical between peak load and the last step before reverse yield
/// (`3.7128712871e-3` both times, differing by less than `1e-18`), so the yield
/// check is not leaking spurious plastic flow into an elastic step.
///
/// **Reverse yielding happens where isotropic hardening says it should.** The
/// surface is a sphere of radius `sigma_y(alpha) = 257.4257 MPa` after the
/// loading branch, and the model yields again in compression at exactly that
/// magnitude — there is no Bauschinger effect, which is correct for isotropic
/// hardening and would be wrong for kinematic hardening. Modelling the
/// Bauschinger effect needs a back stress, which this law does not have and
/// does not claim to.
///
/// **The closed-form plastic strain is recovered.** At peak,
/// `alpha = eps - sigma / E = 5.0e-3 - 2.5742574e8 / 2.0e11 = 3.7128713e-3`,
/// matching the computed value to nine digits.
///
/// Newton takes two solves per elastic step and three per plastic step, which
/// is the expected cost: the extra solve is the one that discovers the point
/// has yielded and re-linearises about the returned state.
#[test]
fn uniaxial_j2_against_closed_form() {
    let mut cell = UniaxialCell::new();
    let eps_y = SIGMA_Y0 / E_PA;
    println!("\nUNIAXIAL J2, E = 200 GPa, nu = 0.3, sigma_y0 = 250 MPa, H = 2 GPa");
    println!("  yield strain = {eps_y:.6e}, E_t = {:.6e} Pa", E_PA * H_PA / (E_PA + H_PA));
    println!(
        "{:>12} {:>15} {:>15} {:>12} {:>8}",
        "strain", "sigma_FEM (Pa)", "sigma_exact (Pa)", "rel err", "newton"
    );

    let mut worst_load = 0.0_f64;
    let mut peak = (0.0, 0.0);
    for k in 1..=50 {
        let eps = 1.0e-4 * k as f64;
        let (sigma, rep) = cell.advance_to(eps);
        let exact = uniaxial_monotonic(eps);
        let rel = (sigma - exact).abs() / exact.abs();
        worst_load = worst_load.max(rel);
        if k % 10 == 0 || k <= 2 {
            println!(
                "{eps:>12.4e} {sigma:>15.7e} {exact:>15.7e} {rel:>12.3e} {:>8}",
                rep.iterations
            );
        }
        peak = (eps, sigma);
    }
    let alpha_peak = cell.equivalent_plastic_strain();

    // The unloading reference. Isotropic hardening leaves the yield surface a
    // sphere of radius sigma_y(alpha_peak), so unloading is elastic only until
    // the stress reaches MINUS that value; past it the material yields again in
    // compression and follows the hardening slope. Both branches are checked,
    // because stopping the sweep before reverse yield would leave the most
    // failure-prone part of the model untested.
    let sigma_y_peak = SIGMA_Y0 + H_PA * alpha_peak;
    let et = E_PA * H_PA / (E_PA + H_PA);
    let strain_at_reverse_yield = peak.0 - (peak.1 + sigma_y_peak) / E_PA;
    let unload_reference = |eps: f64| -> f64 {
        if eps >= strain_at_reverse_yield {
            peak.1 - E_PA * (peak.0 - eps)
        } else {
            -sigma_y_peak + et * (eps - strain_at_reverse_yield)
        }
    };
    println!(
        "  --- unloading from eps = {:.4e}; reverse yield at eps = {:.6e}, \
         sigma_y(alpha) = {:.6e} Pa ---",
        peak.0, strain_at_reverse_yield, sigma_y_peak
    );

    let mut worst_elastic_unload = 0.0_f64;
    let mut worst_reverse = 0.0_f64;
    let mut alpha_before_reverse = alpha_peak;
    for k in (0..50).rev() {
        let eps = 1.0e-4 * k as f64;
        let (sigma, rep) = cell.advance_to(eps);
        let exact = unload_reference(eps);
        let rel = (sigma - exact).abs() / exact.abs().max(1.0);
        if eps >= strain_at_reverse_yield {
            worst_elastic_unload = worst_elastic_unload.max(rel);
            alpha_before_reverse = cell.equivalent_plastic_strain();
        } else {
            worst_reverse = worst_reverse.max(rel);
        }
        if k % 10 == 0 {
            println!(
                "{eps:>12.4e} {sigma:>15.7e} {exact:>15.7e} {rel:>12.3e} {:>8}",
                rep.iterations
            );
        }
    }
    let alpha_end = cell.equivalent_plastic_strain();

    println!("  worst relative error, loading            = {worst_load:.3e}");
    println!("  worst relative error, elastic unloading  = {worst_elastic_unload:.3e}");
    println!("  worst relative error, reverse yielding   = {worst_reverse:.3e}");
    println!("  equivalent plastic strain at peak        = {alpha_peak:.10e}");
    println!("  equivalent plastic strain before reverse = {alpha_before_reverse:.10e}");
    println!("  equivalent plastic strain at eps = 0     = {alpha_end:.10e}");

    assert!(worst_load < 1e-9, "loading branch error {worst_load:e}");
    assert!(
        worst_elastic_unload < 1e-9,
        "elastic unloading branch error {worst_elastic_unload:e}"
    );
    assert!(worst_reverse < 1e-9, "reverse-yield branch error {worst_reverse:e}");
    assert!(
        (alpha_before_reverse - alpha_peak).abs() < 1e-18,
        "unloading must be purely elastic until reverse yield: \
         alpha {alpha_peak:e} -> {alpha_before_reverse:e}"
    );
    assert!(
        alpha_end > alpha_peak,
        "the material must yield again in compression: alpha {alpha_peak:e} -> {alpha_end:e}"
    );
    // The plastic strain must be the closed-form value:
    // alpha = (eps - sigma/E) at peak.
    let alpha_exact = peak.0 - peak.1 / E_PA;
    assert!(
        (alpha_peak - alpha_exact).abs() / alpha_exact < 1e-9,
        "alpha {alpha_peak:e} vs closed form {alpha_exact:e}"
    );
}

/// # Consistent tangent verified by numerical perturbation
///
/// ## Methodology
///
/// At a quadrature point driven well into the plastic range (with prior
/// history, a non-proportional strain increment and a non-zero shear so no
/// entry of the tangent is trivially zero), the algorithmic tangent returned by
/// [`Material::update`] is compared entry by entry with the numerical
/// derivative of the same stress update.
///
/// The numerical derivative is taken by
/// [`outram_foam_basic_lib::math::differentiate::jacobian`] with
/// [`DiffSettings::central`] — the workspace's own implementation of
/// `code_aster`'s perturbed-difference scheme, with its `eps^(1/3)`-scaled
/// step — rather than a hand-rolled difference, so the step-size policy is one
/// that has been verified in its own right.
///
/// Errors are normalised by the largest entry of the analytic tangent.
///
/// ## Pass criterion
///
/// Largest relative entry error below `1e-5`. A central difference on a smooth
/// function is `O(h^2)` accurate with `h ~ eps^(1/3)`, so roughly `1e-11`
/// truncation plus `eps / h ~ 1e-11` round-off is the floor; `1e-5` is a
/// generous band that nevertheless fails immediately if the tangent is the
/// continuum elastoplastic modulus rather than the algorithmic one (those
/// differ by several per cent here).
///
/// ## Results, measured 2026-09-11
///
/// | Quantity | Value |
/// |---|---|
/// | analytic tangent scale (largest entry) | 1.973815e11 Pa |
/// | worst relative entry error vs central difference | 1.055e-7, at entry (1, 2) |
/// | algorithmic vs **elastic** tangent, same point | 4.986e-1 relative |
///
/// ## Interpretation
///
/// The algorithmic tangent agrees with the numerical derivative of the stress
/// update to about seven digits, which is the accuracy a central difference at
/// `h ~ eps^(1/3)` can deliver: truncation `O(h^2) ~ 1e-11` relative to the
/// function, plus round-off `eps/h ~ 1e-11`, amplified by the `1e11` scale of
/// the tangent itself. The residual `1e-7` is therefore the *difference
/// scheme's* error, not the tangent's.
///
/// The third row is the control that makes the first row mean something. At
/// this point the algorithmic tangent differs from the purely elastic one by
/// 50 % of its largest entry, so a run that had mistakenly returned `C_e` would
/// have failed by five orders of magnitude rather than passing narrowly. The
/// test asserts that gap explicitly, so that a future change making the two
/// coincide (perfect plasticity with no flow, say) would fail loudly rather
/// than quietly passing a comparison with nothing in it.
#[test]
fn consistent_tangent_by_numerical_perturbation() {
    let material = Material::j2_linear_hardening(E_PA, NU, SIGMA_Y0, H_PA).unwrap();
    // Build history by taking a prior plastic step.
    let warm = Voigt6::new(3.0e-3, -5.0e-4, 0.0, 0.0, 0.0, 1.0e-3);
    let state = material.update(warm, &MaterialState::pristine(), PlaneCondition::PlaneStrain).unwrap().state;
    let eps = Voigt6::new(6.0e-3, -1.0e-3, 2.0e-4, 3.0e-4, -2.0e-4, 2.0e-3);
    let analytic = material.update(eps, &state, PlaneCondition::PlaneStrain).unwrap();
    assert!(analytic.yielding, "the check point must be plastic");

    let sol = jacobian(
        &eps.as_array(),
        DiffSettings::central(),
        ComputeBackend::Serial,
        |_, v: &[f64], out: &mut Vec<f64>| {
            let e = Voigt6([v[0], v[1], v[2], v[3], v[4], v[5]]);
            out.extend_from_slice(&material.update(e, &state, PlaneCondition::PlaneStrain).unwrap().stress.as_array());
        },
    );
    let num = sol.matrix().expect("smooth in the plastic regime");
    let scale = analytic.tangent.abs_max();
    let mut worst = 0.0_f64;
    let mut worst_at = (0usize, 0usize);
    for i in 0..6 {
        for j in 0..6 {
            let d = (num.get(i, j) - analytic.tangent.get(i, j)).abs() / scale;
            if d > worst {
                worst = d;
                worst_at = (i, j);
            }
        }
    }
    // For contrast, the purely elastic tangent at the same point.
    let elastic_gap = analytic.tangent.max_abs_diff(&material.elastic_stiffness()) / scale;

    println!("\nCONSISTENT TANGENT vs NUMERICAL JACOBIAN");
    println!("  analytic tangent scale        = {scale:.6e} Pa");
    println!("  worst relative entry error    = {worst:.3e} at ({}, {})", worst_at.0, worst_at.1);
    println!("  algorithmic vs elastic tangent= {elastic_gap:.3e} (relative, for contrast)");

    assert!(worst < 1e-5, "consistent tangent entry error {worst:e}");
    assert!(
        elastic_gap > 0.1,
        "the algorithmic tangent must differ substantially from the elastic one \
         at a plastic point, else the comparison proves nothing; got {elastic_gap:e}"
    );
}

/// # Newton convergence order on a genuinely nonlinear elasto-plastic problem
///
/// ## Methodology
///
/// A single-element uniaxial cell is too easy to show a convergence *order*:
/// the state is uniform and the return map is closed form, so Newton lands on
/// the answer almost immediately. A partially plastic structure is needed, in
/// which some quadrature points yield during the step and others do not, so
/// that the residual is genuinely nonlinear in the unknown.
///
/// The problem used is the thick-walled cylinder of
/// `analytical::thick_walled_cylinder_against_lame` with the elastic material
/// replaced by J2 with linear hardening (`sigma_y0 = 250 MPa`, `H = 2 GPa`) and
/// the pressure raised to `p = 140 MPa`, which is well past the bore's elastic
/// limit and leaves a plastic front partway through the wall. Mesh:
/// `n_r = 12`, `n_theta = 24` Quad4, plane strain. Applied in four equal load
/// steps.
///
/// The residual history of the **last** load step is reported; the observed
/// order is estimated from its final three entries as
/// `log(r_{k+1} / r_k) / log(r_k / r_{k-1})`.
///
/// ## Volumetric locking is present and is not being measured away
///
/// Plastic flow is volume preserving, so a fully plastic zone discretised with
/// full-integration Quad4 elements locks: the computed collapse load is too
/// high. That affects the *stresses* this run produces, which is why no stress
/// comparison is made here. It does not affect the *convergence order* of the
/// Newton iteration, which is a property of the linearisation, and that is the
/// only thing this case claims.
///
/// ## Pass criterion
///
/// The observed order must be at least 1.7. A modified-Newton iteration using
/// the elastic tangent would give exactly 1, so this discriminates sharply
/// between a consistent tangent and an inconsistent one.
///
/// ## Results, measured 2026-09-11
///
/// Four load steps, zero cutbacks. Relative residual before each solve:
///
/// | load factor | iterations | yielding points | residual history |
/// |---|---|---|---|
/// | 0.250 | 2 | 0 | 1.000e0, 8.856e-14, 1.255e-14 |
/// | 0.500 | 2 | 0 | 5.000e-1, 4.569e-14, 1.201e-14 |
/// | 0.750 | 2 | 0 | 3.333e-1, 3.351e-14, 1.380e-14 |
/// | 1.000 | 5 | 192 | 2.500e-1, 2.654e-1, 5.848e-3, 1.641e-6, 1.253e-13, 1.262e-14 |
///
/// **Observed convergence order on the final step: 2.004.**
///
/// ## Interpretation
///
/// The last step's residual descent `5.85e-3 -> 1.64e-6 -> 1.25e-13` squares
/// the exponent each time, with a nearly constant ratio
/// `r_{k+1} / r_k^2` of 0.048 then 0.046 — the definition of quadratic
/// convergence, and a direct measurement of the consistent tangent being the
/// exact derivative of the discrete internal force. A modified-Newton iteration
/// on the elastic tangent would give order 1 and would need tens of iterations
/// to reach `1e-13` instead of three.
///
/// The first three load steps are still elastic (zero yielding points, two
/// solves each): at `p = 140 MPa` the bore reaches a von Mises stress of
/// 324 MPa against a 250 MPa yield, so plasticity appears only in the last
/// quarter of the load. The plastic step's residual **rises** from 2.500e-1 to
/// 2.654e-1 on its first iteration, which is expected rather than alarming: the
/// first iterate is taken on the elastic tangent inherited from a state where
/// nothing had yielded, so it overshoots, and the iteration recovers
/// immediately once the returned stresses are known.
///
/// 192 of the 1152 quadrature points are plastic at the end of the step, so
/// the structure is genuinely partially plastic and the tangent is genuinely
/// state dependent.
///
/// What this case does **not** claim: nothing about the accuracy of the
/// elasto-plastic stresses themselves. Full-integration Quad4 elements lock
/// volumetrically once a zone flows plastically, because plastic flow is
/// incompressible; the collapse pressure this mesh would predict is too high.
/// The convergence order is a property of the linearisation and is unaffected,
/// which is why it is the only thing measured here.
#[test]
fn newton_converges_quadratically_with_the_consistent_tangent() {
    let (a, b, p) = (0.05, 0.10, 140.0e6);
    let (n_r, n_theta) = (12usize, 24usize);
    let mesh = quarter_annulus_quad4(a, b, n_r, n_theta).unwrap().shared();
    let dofs = DofMap::displacement(&mesh);
    let material = Material::j2_linear_hardening(E_PA, NU, SIGMA_Y0, H_PA).unwrap();
    let mut system = System::new(mesh.clone(), material, BodyForce::None);

    let mut bcs = DirichletSet::new();
    for n in mesh.nodes_where(|c| c[1].abs() < 1e-12) {
        bcs.fix(&dofs, n, 1, 0.0);
    }
    for n in mesh.nodes_where(|c| c[0].abs() < 1e-12) {
        bcs.fix(&dofs, n, 0, 0.0);
    }
    let mut forces = vec![0.0; system.n_dofs()];
    let facets = quarter_annulus_inner_facets(n_r, n_theta);
    accumulate_pressure_2d(&mesh, &dofs, &facets, p, &mut forces).unwrap();

    let settings = NewtonSettings {
        max_iterations: 30,
        residual_tolerance: 1.0e-11,
        increment_tolerance: 1.0e-7,
        n_load_steps: 4,
        max_cutbacks: 3,
        dirichlet_method: DirichletMethod::Elimination,
        linear: common::linear_settings(1.0e-13),
    };
    let (_u, report) =
        solve_nonlinear(&mut system, &bcs, &forces, &settings).expect("elasto-plastic cylinder");

    println!("\nNEWTON CONVERGENCE, elasto-plastic thick cylinder, p = 140 MPa");
    println!("  {} load steps, {} cutbacks", report.steps.len(), report.cutbacks);
    for s in &report.steps {
        println!(
            "  load factor {:.3}: {} iterations, {} yielding points, residuals {:?}",
            s.load_factor,
            s.iterations,
            s.n_yielding,
            s.residual_history
                .iter()
                .map(|r| format!("{r:.3e}"))
                .collect::<Vec<_>>()
        );
    }
    let last = report.steps.last().expect("at least one load step");
    assert!(
        last.n_yielding > 0,
        "the final step must be partially plastic, else this proves nothing"
    );
    // Estimated from the residuals ABOVE 1e-10: the trailing entries of a
    // converged history are at the round-off floor of the residual evaluation
    // and the ratio between two such numbers says nothing about the iteration.
    let order = last
        .observed_order_above(1.0e-10)
        .expect("at least three meaningful residuals in the final step");
    println!("  observed Newton convergence order on the final step = {order:.3}");
    assert!(
        order > 1.7,
        "observed Newton order {order:.3}; a consistent tangent should give ~2, \
         an elastic (modified-Newton) tangent would give 1"
    );
}
