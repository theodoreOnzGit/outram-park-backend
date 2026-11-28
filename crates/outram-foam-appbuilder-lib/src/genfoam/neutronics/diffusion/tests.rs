// SPDX-License-Identifier: GPL-3.0-only
// Copyright (C) 2026 OUTRAM PARK contributors
//
// Derived from GeN-Foam (Generalized Nuclear Foam)
//   Upstream: https://gitlab.com/foam-for-nuclear/GeN-Foam
//   Upstream commit: 652b3da
//   Upstream source: src/classes/neutronics/diffusion/ (V&V of the port)
//   Upstream copyright: (C) 2015-2022 EPFL; built on OpenFOAM v2506
//   Upstream license: GPL-3.0
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
//
// This offering is not approved or endorsed by EPFL, the OpenFOAM Foundation,
// nor OpenCFD Limited, producer and distributor of the OpenFOAM(R) software.

//! # Verification & validation — multigroup diffusion
//!
//! ## Methodology
//!
//! The multigroup diffusion solver is checked against two closed-form results
//! from one-group diffusion theory, on a homogeneous (single-zone) medium — the
//! only cases with an exact analytical `k_eff`, so any discrepancy is the
//! solver's, not a reference digitisation's:
//!
//! 1. **Infinite medium `k_inf`** (`kinf_infinite_medium`). Reflective
//!    (`ZeroGradient`) boundaries remove all leakage, so
//!    `k_inf = nu Sigma_f / Sigma_a` exactly (buckling-free; the discrete flux
//!    is uniform, so there is no discretisation error). Verifies the
//!    fission/removal balance and the power-iteration `k` update.
//!
//! 2. **Bare-slab critical buckling** (`keff_bare_slab_critical_buckling`).
//!    A slab of width `L` with zero-flux (`FixedValue(0)`) faces has the
//!    fundamental mode `sin(pi x / L)`, geometric buckling `B^2 = (pi/L)^2`, and
//!    analytical `k = nu Sigma_f / (Sigma_a + D B^2)`. This exercises the
//!    `fvm::laplacian` leakage operator and its sign; the measured `k` carries
//!    the expected O(dx^2) finite-volume discretisation error, which shrinks
//!    under mesh refinement (`keff_bare_slab_mesh_convergence`).
//!
//! 3. **Null transient** (`null_transient_holds_steady`). Starting from the
//!    converged eigenvector + equilibrium precursors with `k_eff` frozen at the
//!    eigenvalue, the backward-Euler transient reproduces the steady state, so
//!    the total power is invariant — the transient analogue of the
//!    point-kinetics null-transient check. Verifies the transient assembly is
//!    consistent with the eigenvalue assembly.
//!
//! ## Results (measured 2026-07-15, this port, `cargo test --release`, CG inner
//! solve)
//!
//! - `k_inf` = 3.000000000 vs analytical 3.0 (`|Δ| = 4.4e-16`, 2 outer
//!   iterations). PASS.
//! - Bare slab (L = 1 m, D = 0.02 m, Sigma_a = 0.1 m^-1, nu Sigma_f = 0.3 m^-1,
//!   400 cells): measured `k_eff = 1.0087727` vs analytical
//!   `k = 0.3/(0.1 + 0.02 pi^2) = 1.0087693` — relative deviation `3.4e-6`
//!   (+0.3 pcm), 11 outer iterations. PASS (criterion: within 0.5 %).
//! - Mesh convergence: |k_measured − k_analytic| falls 2.20e-4 (50 cells) →
//!   5.51e-5 (100) → 1.38e-5 (200), the expected O(dx^2) finite-volume rate.
//!   PASS.
//! - Null transient: total power `3.333333` → `3.333334` over 50 steps of
//!   `dt = 1 ms`, relative drift `1.4e-7`. PASS (criterion: `< 1e-6`).
//!
//! (The exact measured numbers are emitted to stderr by each test via
//! `--nocapture`; the assertions below encode the pass criteria.)

#[cfg(not(any(feature = "std", test)))]
#[allow(unused_imports)]
use crate::nostd_prelude::*;
use petir::mathf::RealMath;
use core::f64::consts::PI;
use alloc::sync::Arc;

use outram_foam_basic_lib::interface::one_dimensional_meshing::create_one_d_mesh;
use outram_foam_basic_lib::prelude::{BoundaryCondition, FvMesh};
use uom::si::area::square_meter;
use uom::si::f64::{Area, Length, Time};
use uom::si::length::meter;
use uom::si::time::second;

use super::{DiffusionNeutronics, DiffusionSettings};
use crate::genfoam::neutronics::xs::input::{
    NuclearDataInput, StateInput, ZoneConstantsInput, ZoneStateInput,
};
use crate::genfoam::neutronics::xs::CrossSectionData;

/// Physical group constants for a single-group homogeneous medium.
struct OneGroup {
    d: f64,
    nu_sigma_f: f64,
    sigma_a: f64,
}

/// Build a one-group, single-zone, one-precursor `CrossSectionData` with no
/// feedback variables. Self-scatter is zero (one group), so `sigma_removal`
/// equals the absorption cross section.
fn one_group_xs(gc: &OneGroup) -> CrossSectionData {
    let constants = ZoneConstantsInput {
        iv: vec![1.0e-6],       // 1/v (s/m); value irrelevant to the eigenvalue
        disc_factor: vec![1.0], // no discontinuity factors
        integral_flux: vec![1.0],
        beta: vec![0.0065],
        lambda: vec![0.08],
        ..ZoneConstantsInput::default()
    };
    let zone = ZoneStateInput {
        name: "core".to_string(),
        d: vec![gc.d],
        nu_sigma_eff: vec![gc.nu_sigma_f],
        sigma_pow: vec![1.0], // arbitrary positive energy-release weight
        sigma_removal: vec![gc.sigma_a],
        chi_prompt: vec![1.0],
        chi_delayed: vec![1.0],
        scattering: vec![vec![vec![0.0]]], // 1 moment, 1x1, no self-scatter
        constants: Some(constants),
    };
    let reference = StateInput {
        name: "reference".to_string(),
        parameters: Default::default(),
        zones: vec![zone],
    };
    let input = NuclearDataInput {
        energy_groups: 1,
        prec_groups: 1,
        legendre_moments: 1,
        poly_spline_mode: 1,
        fast_neutrons: false,
        xs_variables: Vec::new(),
        do_not_parametrize: Vec::new(),
        states: vec![reference],
    };
    CrossSectionData::from_input(&input).expect("valid one-group nuclear data")
}

fn slab_mesh(n: i64) -> Arc<FvMesh> {
    Arc::new(
        create_one_d_mesh(Length::new::<meter>(1.0), Area::new::<square_meter>(1.0), n)
            .expect("valid slab mesh"),
    )
}

/// Analytical bare-slab eigenvalue `k = nu Sigma_f / (Sigma_a + D (pi/L)^2)`.
fn analytic_slab_k(gc: &OneGroup, length: f64) -> f64 {
    let b2 = (PI / length).r_powi(2);
    gc.nu_sigma_f / (gc.sigma_a + gc.d * b2)
}

#[test]
fn kinf_infinite_medium() {
    let gc = OneGroup {
        d: 0.02,
        nu_sigma_f: 0.30,
        sigma_a: 0.10,
    };
    let xs = one_group_xs(&gc);
    let mesh = slab_mesh(4);
    // Reflective (no leakage) on both faces → infinite medium.
    let bc = vec![
        BoundaryCondition::ZeroGradient,
        BoundaryCondition::ZeroGradient,
    ];
    let zone_of_cell = vec![0usize; mesh.n_cells];
    let mut model = DiffusionNeutronics::new(
        mesh,
        &xs,
        &zone_of_cell,
        &[],
        &bc,
        DiffusionSettings::default(),
    )
    .expect("build diffusion model");

    let report = model.solve_eigenvalue().expect("eigenvalue converges");
    let k_inf_analytic = gc.nu_sigma_f / gc.sigma_a; // = 3.0
    eprintln!(
        "k_inf: measured {:.9}, analytic {:.9}, |Δ| {:.2e}, outer iters {}",
        report.k_eff,
        k_inf_analytic,
        (report.k_eff - k_inf_analytic).abs(),
        report.outer_iterations
    );
    assert!(report.converged);
    assert!(
        (report.k_eff - k_inf_analytic).abs() < 1e-7,
        "k_inf = {} != {}",
        report.k_eff,
        k_inf_analytic
    );
}

#[test]
fn keff_bare_slab_critical_buckling() {
    let gc = OneGroup {
        d: 0.02,
        nu_sigma_f: 0.30,
        sigma_a: 0.10,
    };
    let xs = one_group_xs(&gc);
    let n = 400;
    let mesh = slab_mesh(n);
    // Zero-flux (vacuum) on both faces.
    let bc = vec![
        BoundaryCondition::FixedValue(0.0),
        BoundaryCondition::FixedValue(0.0),
    ];
    let zone_of_cell = vec![0usize; mesh.n_cells];
    let mut model = DiffusionNeutronics::new(
        mesh,
        &xs,
        &zone_of_cell,
        &[],
        &bc,
        DiffusionSettings::default(),
    )
    .expect("build diffusion model");

    let report = model.solve_eigenvalue().expect("eigenvalue converges");
    let k_analytic = analytic_slab_k(&gc, 1.0);
    let rel = (report.k_eff - k_analytic).abs() / k_analytic;
    eprintln!(
        "bare slab (n={}): measured k_eff {:.7}, analytic {:.7}, rel dev {:.3e} ({:.1} pcm), outer iters {}",
        n,
        report.k_eff,
        k_analytic,
        rel,
        (report.k_eff - k_analytic) * 1e5,
        report.outer_iterations
    );
    assert!(report.converged);
    assert!(
        rel < 5e-3,
        "measured k_eff {} deviates from analytic {} by {:.3e} (> 0.5%)",
        report.k_eff,
        k_analytic,
        rel
    );
}

/// Round 40 (gh:#1890): the default linear solves are OpenFOAM's (`PCG`/`DIC`, OpenFOAM's normalised residual); the
/// deprecated basic-lib form (`with_basic_lib_cg`) gives the same eigenvalue to the solver tolerance.
#[test]
fn the_two_linear_solver_forms_give_the_same_bare_slab_keff() {
    let gc = OneGroup { d: 0.02, nu_sigma_f: 0.30, sigma_a: 0.10 };
    let xs = one_group_xs(&gc);
    let mesh = slab_mesh(100);
    let bc = vec![BoundaryCondition::FixedValue(0.0), BoundaryCondition::FixedValue(0.0)];
    let zone_of_cell = vec![0usize; mesh.n_cells];
    let run = |settings: DiffusionSettings| {
        let mut model = DiffusionNeutronics::new(mesh.clone(), &xs, &zone_of_cell, &[], &bc, settings).expect("model");
        model.solve_eigenvalue().expect("converges").k_eff
    };
    let upstream = run(DiffusionSettings::default());
    let old = run(DiffusionSettings::default().with_basic_lib_cg());
    assert!(DiffusionSettings::default().with_basic_lib_cg().form != DiffusionSettings::default().form);
    assert!((upstream - old).abs() / old < 1e-7, "{upstream} vs {old}");
}

#[test]
fn keff_bare_slab_mesh_convergence() {
    let gc = OneGroup {
        d: 0.02,
        nu_sigma_f: 0.30,
        sigma_a: 0.10,
    };
    let xs = one_group_xs(&gc);
    let k_analytic = analytic_slab_k(&gc, 1.0);
    let bc = vec![
        BoundaryCondition::FixedValue(0.0),
        BoundaryCondition::FixedValue(0.0),
    ];

    let mut last_dev = f64::INFINITY;
    for &n in &[50i64, 100, 200] {
        let mesh = slab_mesh(n);
        let zone_of_cell = vec![0usize; mesh.n_cells];
        let mut model = DiffusionNeutronics::new(
            mesh,
            &xs,
            &zone_of_cell,
            &[],
            &bc,
            DiffusionSettings::default(),
        )
        .unwrap();
        let report = model.solve_eigenvalue().unwrap();
        let dev = (report.k_eff - k_analytic).abs();
        eprintln!("  n={:>4}: k_eff {:.7}, |Δ| {:.3e}", n, report.k_eff, dev);
        assert!(
            dev < last_dev,
            "deviation did not shrink on refinement: {dev:.3e} !< {last_dev:.3e}"
        );
        last_dev = dev;
    }
}

#[test]
fn null_transient_holds_steady() {
    let gc = OneGroup {
        d: 0.02,
        nu_sigma_f: 0.30,
        sigma_a: 0.10,
    };
    let xs = one_group_xs(&gc);
    let mesh = slab_mesh(100);
    let bc = vec![
        BoundaryCondition::FixedValue(0.0),
        BoundaryCondition::FixedValue(0.0),
    ];
    let zone_of_cell = vec![0usize; mesh.n_cells];
    let mut model = DiffusionNeutronics::new(
        mesh,
        &xs,
        &zone_of_cell,
        &[],
        &bc,
        DiffusionSettings::default(),
    )
    .unwrap();

    // Converge the eigenvector; k_eff is now frozen for the transient.
    model.solve_eigenvalue().unwrap();
    let p0 = model.state().total_power_raw();
    assert!(p0 > 0.0, "eigenvalue power must be positive, got {p0}");

    let dt = Time::new::<second>(1.0e-3);
    for _ in 0..50 {
        model.step(dt).unwrap();
    }
    let p1 = model.state().total_power_raw();
    let rel = (p1 - p0).abs() / p0;
    eprintln!("null transient: P0 {p0:.6e}, P50 {p1:.6e}, rel drift {rel:.3e}");
    assert!(
        rel < 1e-6,
        "null transient drifted: relative change {rel:e}"
    );
}

/// A warm start from a converged solution (`set_initial_guess`) reaches the
/// same `k_eff` as the cold start, in far fewer outer iterations — what a
/// coupling loop that rebuilds the model each Picard pass relies on
/// (dhoby-ghaut Step 10, gh:#591). Measured 2026-10-05: see the message.
#[test]
fn warm_start_reaches_the_same_k_in_fewer_iterations() {
    let gc = OneGroup {
        d: 0.02,
        nu_sigma_f: 0.30,
        sigma_a: 0.10,
    };
    let xs = one_group_xs(&gc);
    let mesh = slab_mesh(200);
    let bc = vec![
        BoundaryCondition::FixedValue(0.0),
        BoundaryCondition::FixedValue(0.0),
    ];
    let zone_of_cell = vec![0usize; mesh.n_cells];
    let build = || {
        DiffusionNeutronics::new(
            mesh.clone(),
            &xs,
            &zone_of_cell,
            &[],
            &bc,
            DiffusionSettings::default(),
        )
        .unwrap()
    };
    let mut cold = build();
    let r0 = cold.solve_eigenvalue().unwrap();
    let flux: Vec<Vec<f64>> = cold
        .state()
        .flux()
        .iter()
        .map(|f| f.internal.as_slice().to_vec())
        .collect();
    let mut warm = build();
    warm.set_initial_guess(&flux, r0.k_eff);
    let r1 = warm.solve_eigenvalue().unwrap();
    eprintln!(
        "cold: k {:.8} in {} outers; warm: k {:.8} in {} outers",
        r0.k_eff, r0.outer_iterations, r1.k_eff, r1.outer_iterations
    );
    assert!(r1.converged);
    assert!((r1.k_eff - r0.k_eff).abs() < 1e-7);
    assert!(r1.outer_iterations * 5 < r0.outer_iterations);
}

/// gh:#1814: the eigenvalue -> transient chain (`genfoam::workflow`) on the bare one-group slab: the pseudo-time
/// stage reaches the static eigenvalue and normalises the flux to `pTarget`; the transient built from the
/// normalised state starts with `keff_` frozen and holds the target power under a null perturbation.
#[test]
fn workflow_chain_normalises_to_p_target_and_holds_it() {
    use crate::genfoam::neutronics::pseudo_time::PseudoTimeControls;
    use crate::genfoam::workflow::{EigenvalueStage, TransientStart};
    let gc = OneGroup { d: 0.02, nu_sigma_f: 0.30, sigma_a: 0.10 };
    let xs = one_group_xs(&gc);
    let mesh = slab_mesh(100);
    let bc = vec![BoundaryCondition::FixedValue(0.0), BoundaryCondition::FixedValue(0.0)];
    let zone_of_cell = vec![0usize; mesh.n_cells];
    let build = || {
        DiffusionNeutronics::new(mesh.clone(), &xs, &zone_of_cell, &[], &bc, DiffusionSettings::default()).unwrap()
    };
    let mut reference = build();
    let k_static = reference.solve_eigenvalue().unwrap().k_eff;

    let p_target = 3.0e6;
    let controls = PseudoTimeControls::guided(1.0, 300.0, 10.0, 10.0, 0.5, p_target);
    let normalised = EigenvalueStage::Diffusion { model: build(), controls }.run().expect("eigenvalue stage");
    let reactor = normalised.reactor_state();
    assert_eq!(reactor.p_target, p_target);
    assert!(((reactor.k_eff - k_static) / k_static).abs() < 1e-5, "{} vs {k_static}", reactor.k_eff);
    let mut run = normalised.start_transient(TransientStart::Plain).expect("transient");
    assert_eq!(run.k_eff(), reactor.k_eff, "keff_ is frozen into the transient");
    let p0 = run.power();
    assert!(((p0 - p_target) / p_target).abs() < 1e-9, "normalised to pTarget: {p0}");
    for _ in 0..50 {
        run.step_fixed(Time::new::<second>(1.0e-3)).expect("step");
    }
    assert!(((run.power() - p0) / p0).abs() < 1e-5, "null transient drifted: {} vs {p0}", run.power());
    assert_eq!(run.k_eff(), reactor.k_eff);
    // the coupled stepper refuses a stationary-fuel run, by name
    assert!(run.step_adjustable().is_err());
}

/// gh:#1856 item 1: a static mesh reduces to the old `ddt` bit for bit (`rebind_mesh` without `V0`), `V0` enters the `ddt` source on a moving
/// mesh (a control: `V0 = 2 V` changes the step), and a moved mesh of another topology is refused.
#[test]
fn rebind_without_v0_is_the_old_path_and_v0_weights_the_ddt_source() {
    use super::external_source::StationaryStepControls;
    let gc = OneGroup { d: 0.02, nu_sigma_f: 0.30, sigma_a: 0.10 };
    let xs = one_group_xs(&gc);
    let mesh = slab_mesh(20);
    let n = mesh.n_cells;
    let bc = vec![BoundaryCondition::FixedValue(0.0), BoundaryCondition::FixedValue(0.0)];
    let build = || {
        let mut m = DiffusionNeutronics::new(mesh.clone(), &xs, &vec![0usize; n], &[], &bc, DiffusionSettings::default()).unwrap();
        m.set_initial_guess(&[vec![1.0; n]], 1.0);
        m
    };
    let controls = StationaryStepControls::default();
    let (mut a, mut b, mut c) = (build(), build(), build());
    let v2: Vec<f64> = mesh.cell_volumes.iter().map(|v| 2.0 * v).collect();
    let mut first_a = Vec::new();
    for k in 1..=3 {
        let t = 0.01 * k as f64;
        a.upstream_stationary_step(t, 0.01, &controls).unwrap();
        b.rebind_mesh(mesh.clone(), None).unwrap();
        b.upstream_stationary_step(t, 0.01, &controls).unwrap();
        assert_eq!(a.state().flux()[0].internal.as_slice(), b.state().flux()[0].internal.as_slice(), "step {k}");
        assert!(!b.on_moving_mesh());
        if k == 1 {
            first_a = a.state().flux()[0].internal.as_slice().to_vec();
            c.rebind_mesh(mesh.clone(), Some(v2.clone())).unwrap();
            c.upstream_stationary_step(t, 0.01, &controls).unwrap();
            assert!(c.on_moving_mesh());
        }
    }
    assert_ne!(c.state().flux()[0].internal.as_slice(), first_a.as_slice(), "V0 != V must change the ddt source");
    assert!(a.rebind_mesh(slab_mesh(21), None).is_err(), "another topology is refused");
    assert!(a.rebind_mesh(mesh.clone(), Some(vec![1.0; 3])).is_err(), "V0 must be sized to the mesh");
}

/// gh:#1950 (README round 75): `multiPhysicsSolver` repeats a pass inside one time step with the OLD-TIME values of the previous time step. Without a hold a
/// second `upstream_stationary_step` integrates from the first pass's result; with it, the pass is the same step again (bit for bit, the solve starts from
/// the first pass's flux but `ddt` takes the held old values, so a CONVERGED pass is a fixed point), and release restores the default; the step reports
/// `solver_residual` (`getResidual()`), at least the last pass's `neutroResidual`.
#[test]
fn a_repeated_pass_keeps_the_old_time_values_of_the_time_step() {
    use super::external_source::StationaryStepControls;
    let gc = OneGroup { d: 0.02, nu_sigma_f: 0.30, sigma_a: 0.10 };
    let xs = one_group_xs(&gc);
    let mesh = slab_mesh(20);
    let n = mesh.n_cells;
    let bc = vec![BoundaryCondition::FixedValue(0.0), BoundaryCondition::FixedValue(0.0)];
    let build = || {
        let mut m = DiffusionNeutronics::new(mesh.clone(), &xs, &vec![0usize; n], &[], &bc, DiffusionSettings::default()).unwrap();
        m.set_initial_guess(&[vec![1.0; n]], 0.9);
        m
    };
    let controls = StationaryStepControls::default();
    // default: the second call integrates from the first call's result
    let mut free = build();
    free.upstream_stationary_step(0.01, 0.01, &controls).unwrap();
    let after_one: Vec<f64> = free.state().flux()[0].internal.as_slice().to_vec();
    free.upstream_stationary_step(0.01, 0.01, &controls).unwrap();
    let free_two: Vec<f64> = free.state().flux()[0].internal.as_slice().to_vec();
    // held: two passes of ONE step
    let mut held = build();
    held.hold_stationary_old_time();
    assert!(held.stationary_old_time_is_held());
    let s1 = held.upstream_stationary_step(0.01, 0.01, &controls).unwrap();
    assert_eq!(held.state().flux()[0].internal.as_slice(), after_one.as_slice(), "the first pass is the ordinary step");
    held.upstream_stationary_step(0.01, 0.01, &controls).unwrap();
    let held_two: Vec<f64> = held.state().flux()[0].internal.as_slice().to_vec();
    assert_ne!(held_two, free_two, "a repeated pass must not integrate from the previous pass");
    let drift = held_two.iter().zip(&after_one).fold(0.0_f64, |a, (x, y)| a.max(((x - y) / y).abs()));
    assert!(drift < 1e-3, "the second pass stays near the first (same old time): {drift:e}");
    assert!(s1.solver_residual >= *s1.residual_history.last().unwrap());
    // gh:#1966 (README round 86): two recorded flux solves per group and neutron iteration, the first one's initial residual the
    // iteration's `neutroResidual` (one group), and the solver's log is the last call's
    assert_eq!(s1.flux_solves.len(), 2 * s1.neutron_iterations);
    for (k, r) in s1.residual_history.iter().enumerate() {
        assert_eq!(s1.flux_solves[2 * k].initial_residual, *r);
        assert_eq!(s1.flux_solves[2 * k].field, "fluxStar0");
    }
    let s3 = match held.upstream_stationary_step(0.01, 0.01, &controls) {
        Ok(s) => s,
        Err(e) => panic!("third pass: {e}"),
    };
    assert_eq!(held.flux_solve_log(), s3.flux_solves.as_slice());
    held.release_stationary_old_time();
    assert!(!held.stationary_old_time_is_held());
}
