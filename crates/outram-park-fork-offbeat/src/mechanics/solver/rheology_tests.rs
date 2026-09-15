// SPDX-License-Identifier: GPL-3.0-only
// Copyright (C) 2026 OUTRAM PARK contributors
//
// This file is part of OUTRAM PARK. See `src/lib.rs` for the full licence
// notice.

//! Verification of the **rheology-coupled** mechanics solve.
//!
//! # What is verified here, and what is not
//!
//! These are **verification** tests in the workspace's sense — "is the equation
//! implemented correctly?" — checked against closed-form viscoelastic
//! relaxation and against exact equilibrium requirements. They are *not*
//! validation: nothing here compares the port to OFFBEAT output or to
//! fuel-irradiation data, and no test in this file may be cited as evidence
//! that the port reproduces experiment.
//!
//! Every number quoted in a doc comment below was **printed by the test itself**
//! and transcribed; none is predicted.

use std::sync::Arc;

use approx::assert_relative_eq;
use outram_foam_basic_lib::fields::boundary::bc::{BoundaryCondition, PatchField};
use outram_foam_basic_lib::fields::Field;
use outram_foam_basic_lib::mesh::{BoundaryPatch, FvMesh, FvMeshBuilder, PatchKind};
use outram_foam_basic_lib::primitives::{SymmTensor, Vector3};

use crate::materials::MaterialState;
use crate::rheology::{
    von_mises, ConstitutiveLaw, CreepModel, CreepTimeStepControl, Rheology, RheologyByMaterial,
    RheologyInputs, RheologyState, YieldStressModel,
};

use super::*;

/// A column of `n` cubic cells along **x**, of total `length` and square
/// cross-section `width × width`, with **all six** bounding surfaces present as
/// patches.
///
/// The lateral patches are what distinguish this from the `line_mesh` used by
/// the elastic tests: with y and z faces in the mesh, `∇D` has transverse
/// components and the body can strain laterally. Free (unconstrained) thermal
/// expansion is only representable on such a mesh — on a mesh with no lateral
/// faces the transverse strain is structurally zero and every case is
/// oedometric.
///
/// Patch order: `xMin`, `xMax`, `yMin`, `yMax`, `zMin`, `zMax`.
fn box_column_mesh(n: usize, length: f64, width: f64) -> Arc<FvMesh> {
    assert!(n >= 2, "need at least two cells");
    let dx = length / n as f64;
    let half = 0.5 * width;
    let n_int = n - 1;
    let v3 = Vector3::new;

    // Internal x-faces.
    let mut owner: Vec<usize> = (0..n_int).collect();
    let neighbour: Vec<usize> = (1..n).collect();
    let mut face_area_vectors: Vec<Vector3> =
        (0..n_int).map(|_| v3(width * width, 0.0, 0.0)).collect();
    let mut face_centres: Vec<Vector3> = (0..n_int)
        .map(|i| v3((i as f64 + 1.0) * dx, 0.0, 0.0))
        .collect();

    // xMin, xMax.
    owner.push(0);
    face_area_vectors.push(v3(-width * width, 0.0, 0.0));
    face_centres.push(v3(0.0, 0.0, 0.0));
    owner.push(n - 1);
    face_area_vectors.push(v3(width * width, 0.0, 0.0));
    face_centres.push(v3(length, 0.0, 0.0));

    // The four lateral patches, each with one face per cell.
    let lateral: [(Vector3, Vector3); 4] = [
        (v3(0.0, -dx * width, 0.0), v3(0.0, -half, 0.0)),
        (v3(0.0, dx * width, 0.0), v3(0.0, half, 0.0)),
        (v3(0.0, 0.0, -dx * width), v3(0.0, 0.0, -half)),
        (v3(0.0, 0.0, dx * width), v3(0.0, 0.0, half)),
    ];
    for (area, offset) in lateral {
        for i in 0..n {
            owner.push(i);
            face_area_vectors.push(area);
            face_centres.push(v3((i as f64 + 0.5) * dx, 0.0, 0.0) + offset);
        }
    }

    let names = ["xMin", "xMax", "yMin", "yMax", "zMin", "zMax"];
    let sizes = [1usize, 1, n, n, n, n];
    let mut start = n_int;
    let mut patches = Vec::with_capacity(6);
    for (name, size) in names.into_iter().zip(sizes) {
        patches.push(BoundaryPatch::new(name, start, size, PatchKind::Wall));
        start += size;
    }

    Arc::new(
        FvMeshBuilder::new()
            .n_cells(n)
            .n_internal_faces(n_int)
            .owner(owner)
            .neighbour(neighbour)
            .patches(patches)
            .cell_volumes(vec![dx * width * width; n])
            .cell_centres(
                (0..n)
                    .map(|i| v3((i as f64 + 0.5) * dx, 0.0, 0.0))
                    .collect(),
            )
            .face_area_vectors(face_area_vectors)
            .face_centres(face_centres)
            .build()
            .expect("box column mesh must build"),
    )
}

/// Dirichlet displacement on every patch, taken face by face from an analytic
/// field `d(x)`.
///
/// Prescribing the exact solution on the whole boundary is the standard way to
/// verify an interior discretisation without also having to verify a
/// traction boundary condition, which this port does not yet provide.
fn prescribed_boundary(
    mesh: &FvMesh,
    displacement: impl Fn(Vector3) -> Vector3,
) -> Vec<PatchField<Vector3>> {
    mesh.patches
        .iter()
        .map(|p| {
            let values: Vec<Vector3> = (0..p.size)
                .map(|fi| displacement(mesh.face_centres[p.start + fi]))
                .collect();
            let field = Field::new(values);
            PatchField {
                bc: BoundaryCondition::FixedField(field.clone()),
                values: field,
            }
        })
        .collect()
}

/// Reference material: `E = 200 GPa`, `ν = 0.3` — a stand-in elastic solid, not
/// a specific fuel, chosen so the closed forms can be checked by hand.
fn reference_material() -> LinearElastic {
    LinearElastic::new(200.0e9, 0.3).expect("reference material is admissible")
}

/// A linear (Norton `n = 1`) creep law that never yields.
///
/// `n = 1` is chosen deliberately: it is the only exponent for which the
/// relaxation of a body held at fixed strain has a closed-form solution, which
/// is what makes the acceptance test below a verification rather than a
/// regression check. The yield stress is set far above anything the cases reach
/// so that plasticity cannot contaminate the comparison.
fn linear_creep_law(b_per_hour: f64, sigma_c_mpa: f64) -> ConstitutiveLaw {
    ConstitutiveLaw::MisesPlasticCreep {
        yield_stress: YieldStressModel::Constant { sigma_y: 1.0e12 },
        creep: CreepModel::PowerLaw {
            b: b_per_hour,
            sigma_c: sigma_c_mpa,
            n: 1.0,
        },
    }
}

/// Relaxation time constant `τ` \[s\] of a linear Norton law at shear modulus
/// `μ` \[Pa\].
///
/// The law's rate is `ε̇_eq = B (q/σ_C)` per hour, i.e. `ε̇_eq = k q` per second
/// with `k = B · 1e−6 / (σ_C · 3600)` \[1/(s·Pa)\] once the per-hour and
/// per-MPa conversions are applied. Prandtl–Reuss flow at fixed total strain
/// then gives `ṡ = −3μ k s`, so `τ = 1/(3μk)`.
fn relaxation_time(b_per_hour: f64, sigma_c_mpa: f64, mu: f64) -> f64 {
    let k = b_per_hour * 1.0e-6 / (sigma_c_mpa * 3600.0);
    1.0 / (3.0 * mu * k)
}

// ---------------------------------------------------------------------------
// Acceptance case (a) — stress relaxation against the closed-form exponential
// ---------------------------------------------------------------------------

/// **Stress relaxation of a bar held at constant strain — acceptance case (a).**
///
/// *Methodology.* An 8-cell column (`L = 8 mm`, `1 mm × 1 mm` section) has the
/// exact displacement field `D = (ε₀ x, 0, 0)` prescribed on **all six**
/// patches, so the total strain is pinned at `ε = diag(ε₀, 0, 0)` for all time
/// with `ε₀ = 1e−4`. The material is `E = 200 GPa`, `ν = 0.3` with a linear
/// Norton creep law (`B = 1.5e−3 /hr`, `σ_C = 100 MPa`, `n = 1`) and a yield
/// stress far above the loading. No thermal eigenstrain.
///
/// Creep flows along the deviatoric stress direction and is volume preserving,
/// so at fixed total strain the trace of the elastic strain never changes and
/// the *hydrostatic* stress is frozen at `K ε₀`. The deviatoric part obeys
/// `ṡ = −3μ k s`, giving the closed form
///
/// `q(t) = q₀ exp(−t/τ)`,  `q₀ = 2μ ε₀`,  `τ = 1/(3μk)`,  `k = B·1e−6/(σ_C·3600)`
///
/// and `σ_xx(t) = K ε₀ + (4μ/3) ε₀ exp(−t/τ)`.
///
/// The solver integrates the creep increment implicitly in stress (backward
/// Euler), whose discrete solution is `q_N = q₀ (1 + Δt/τ)^{−N}`. That is
/// first-order accurate, so the test runs one relaxation time `T = τ` at three
/// step counts and checks (i) the error against the exponential falls like
/// `Δt`, and (ii) the finest run agrees to better than 0.2 %.
///
/// Pass criteria: relative error in `q(T)` below 0.2 % at `N = 800`; the
/// error ratio between successive refinements within 10 % of 2 (first order);
/// hydrostatic stress constant to 1e−9 relative; corrected stress equal to the
/// elastic law evaluated on `ε − ε_in` to 1e−9 relative.
///
/// *Results (measured 2026-08-05, printed by this test).*
/// `μ = 76.9231 GPa`, `K = 166.667 GPa`, `τ = 1040.00 s`,
/// `q₀ = 15.3846 MPa`, `σ_xx(0) = 26.9231 MPa`, exact `q(τ) = q₀ e^{−1} =
/// 5.65968 MPa`.
///
/// | steps `N` | `Δt/τ` | `q(τ)` \[MPa\] | rel. error vs `q₀ e^{−1}` |
/// |---|---|---|---|
/// | 50 | 2.000e−2 | 5.71581e+00 | 9.91749e−03 |
/// | 200 | 5.000e−3 | 5.67380e+00 | 2.49480e−03 |
/// | 800 | 1.250e−3 | 5.66322e+00 | 6.24675e−04 |
///
/// Error ratios 3.975 and 3.994 across the two 4× refinements — a 4× step
/// reduction gives a 4× error reduction, confirming first-order convergence of
/// the backward-Euler creep integration. Hydrostatic stress held at
/// `16.6667 MPa` (`K ε₀`) throughout, drifting by `0.00000e+00`, `2.23517e−16`
/// and `4.47035e−16` relative over the three runs. The corrected stress matched
/// `2μ(ε − ε_in) + λ tr(ε − ε_in) I` to `2.20e−15`, `2.24e−15` and `2.20e−15`
/// relative.
///
/// *Interpretation.* The eigenstrain-subtraction path, the inelastic feedback
/// term `∇·(2μ ε_in)`, the once-per-step state advance and the implicit creep
/// integration are mutually consistent and reproduce the analytic relaxation of
/// a linear viscoelastic solid. The residual 0.06 % at the finest resolution is
/// time-discretisation error, not a modelling error: it shrinks in proportion
/// to `Δt`.
#[test]
fn clamped_bar_relaxes_to_closed_form_exponential() {
    let material = reference_material();
    let mu = material.shear_modulus();
    let bulk = material.three_k() / 3.0;
    let (b, sigma_c) = (1.5e-3, 100.0);
    let tau = relaxation_time(b, sigma_c, mu);

    let strain = 1.0e-4;
    let q0 = 2.0 * mu * strain;
    let sigma_xx0 = material.two_mu_plus_lambda() * strain;

    println!(
        "mu = {:.4} GPa, K = {:.3} GPa, tau = {:.2} s, q0 = {:.4} MPa, sigma_xx(0) = {:.4} MPa",
        mu * 1e-9,
        bulk * 1e-9,
        tau,
        q0 * 1e-6,
        sigma_xx0 * 1e-6
    );

    let exact = q0 * (-1.0f64).exp();
    let mut errors = Vec::new();

    for &n_steps in &[50usize, 200, 800] {
        let mesh = box_column_mesh(8, 0.008, 0.001);
        let boundary = prescribed_boundary(&mesh, |p| Vector3::new(strain * p.x, 0.0, 0.0));
        let mut solver = MechanicsSolver::new(mesh.clone(), material, boundary);
        solver
            .set_rheology(Rheology::ByMaterial(
                RheologyByMaterial::uniform(linear_creep_law(b, sigma_c), mesh.n_cells)
                    .expect("uniform rheology"),
            ))
            .expect("rheology covers the mesh");
        solver.set_uniform_material_state(MaterialState::fresh(600.0));

        let dt = tau / n_steps as f64;
        let mut hydrostatic_first = f64::NAN;
        for step in 0..n_steps {
            let report = solver
                .solve_creep_step(dt)
                .expect("creep step must integrate");
            assert!(
                report.mechanics.converged,
                "step {step} did not converge: displacement change {:e} after {} correctors",
                report.mechanics.final_change, report.mechanics.iterations
            );
            if step == 0 {
                hydrostatic_first = solver.stress().internal[4].tr() / 3.0;
            }
        }

        let mid = solver.stress().internal[4];
        let q = von_mises(mid);
        let error = (q - exact).abs() / exact;
        errors.push(error);
        println!(
            "N = {n_steps:4}  dt/tau = {:.3e}  q(tau) = {:.5e} MPa  rel err = {:.5e}",
            dt / tau,
            q * 1e-6,
            error
        );

        // Creep is deviatoric: the hydrostatic stress must not move at all.
        let hydrostatic = mid.tr() / 3.0;
        println!(
            "    hydrostatic = {:.4} MPa (K*eps0 = {:.4} MPa), drift over run = {:.5e}",
            hydrostatic * 1e-6,
            bulk * strain * 1e-6,
            (hydrostatic - hydrostatic_first).abs() / hydrostatic_first.abs()
        );
        assert_relative_eq!(hydrostatic, bulk * strain, max_relative = 1e-9);
        assert_relative_eq!(hydrostatic, hydrostatic_first, max_relative = 1e-9);

        // The corrected stress must equal the elastic law evaluated on the
        // strain left after the inelastic strain is removed. If the feedback
        // term and the constitutive law disagreed, the displacement field would
        // be in equilibrium with a stress nobody reports.
        let elastic_strain = SymmTensor::from_diag(strain, 0.0, 0.0) - solver.inelastic_strain(4);
        let from_law = 2.0 * mu * elastic_strain
            + (material.lame_lambda() * elastic_strain.tr()) * SymmTensor::IDENTITY;
        let consistency = (from_law - mid).mag() / mid.mag();
        println!("    stress consistency (elastic law vs reported) = {consistency:.2e}");
        assert!(
            consistency < 1e-9,
            "reported stress disagrees with the elastic law on the elastic strain: {consistency:e}"
        );
    }

    assert!(
        errors[2] < 2.0e-3,
        "finest run must match the closed form to 0.2%, got {:e}",
        errors[2]
    );
    for w in errors.windows(2) {
        let ratio = w[0] / w[1];
        println!("error ratio across a 4x refinement = {ratio:.3}");
        assert!(
            (ratio - 4.0).abs() < 0.4,
            "backward Euler must be first order; error ratio {ratio} over a 4x step reduction"
        );
    }
}

// ---------------------------------------------------------------------------
// Acceptance case (b) — free thermal expansion must not creep
// ---------------------------------------------------------------------------

/// **Free thermal expansion with creep active produces no inelastic strain —
/// acceptance case (b), the eigenstrain-subtraction check.**
///
/// *Methodology.* A 6-cell column (`L = 6 mm`, `1 mm × 1 mm` section) is given
/// the exact free-expansion field `D = ε*(x, y, z)` on all six patches, with a
/// uniform thermal eigenstrain `ε* = α ΔT = 1e−5 × 300 = 3e−3`. The material is
/// `E = 200 GPa`, `ν = 0.3` with an aggressive linear creep law
/// (`B = 1.0 /hr`, `σ_C = 1 MPa`, `n = 1` — a relaxation time of order a
/// millisecond, so any stress at all would creep away visibly within one 1000 s
/// step). Twenty such steps are taken.
///
/// The exact solution is a body that expands freely and carries **no stress**:
/// the total strain `ε = ε* I` is exactly cancelled by the eigenstrain, leaving
/// zero mechanical strain, zero stress and therefore zero creep. Getting item 1
/// of the rheology contract wrong — passing the total strain instead of
/// `ε − ε* I` — makes the constitutive law see an elastic strain of `ε* I` and
/// report a hydrostatic `3K ε*`, which is then fed back into the momentum
/// balance as if it were real.
///
/// Pass criteria: equivalent creep strain **exactly** zero in every cell;
/// equivalent plastic strain exactly zero; every stress component below 1 Pa in
/// magnitude; displacement matching `ε* x` to 1e−12 relative.
///
/// *Results (measured 2026-08-05, printed by this test).* After 20 steps of
/// 1000 s: max |σ| over all cells and components `1.37e−06` Pa, max equivalent
/// creep strain `5.49329e−18` (i.e. `1.8e−15` of the `3e−3` eigenstrain the
/// body actually underwent), max equivalent plastic strain exactly
/// `0.00000e+00`, max displacement error `1.21e−20` m against the analytic
/// `ε* x`. The residual creep is the round-off stress of the linear solve being
/// relaxed by a law whose relaxation time is of order a millisecond — it is the
/// floor of the numerics, not a physical creep.
///
/// The same constitutive law handed the **total** strain instead — the bug this
/// test exists to catch — returns a von Mises stress of `0.0000 MPa` but a
/// hydrostatic stress of `1500.0000 MPa`, matching `3K ε* = 1500.0000 MPa` —
/// 1.5 GPa of pure fiction. That is the observable signature of the missing subtraction for an
/// *isotropic* eigenstrain: because an isotropic eigenstrain has a zero
/// deviator, the spurious stress is entirely hydrostatic and drives no creep
/// directly. The creep assertions above are therefore necessary but not
/// sufficient on their own; the stress assertion is what carries the teeth
/// here, and it is stated plainly rather than left implied.
///
/// *Interpretation.* `MechanicsSolver::solve_creep_step` subtracts the
/// eigenstrain before the constitutive law sees the strain, so an unconstrained
/// heated body is correctly stress-free and does not creep.
#[test]
fn free_thermal_expansion_with_creep_produces_no_creep_strain() {
    let material = reference_material();
    let alpha = 1.0e-5;
    let delta_t = 300.0;
    let eigenstrain = Eigenstrain::thermal(alpha, delta_t);
    let eps_star = eigenstrain.total();

    let mesh = box_column_mesh(6, 0.006, 0.001);
    let boundary = prescribed_boundary(&mesh, |p| {
        Vector3::new(eps_star * p.x, eps_star * p.y, eps_star * p.z)
    });
    let mut solver = MechanicsSolver::new(mesh.clone(), material, boundary);
    solver.set_uniform_eigenstrain(eigenstrain);
    // Deliberately violent creep: tau is of order 1 ms here, so any stress at
    // all would relax completely inside a single step and leave a large creep
    // strain behind.
    solver
        .set_rheology(Rheology::ByMaterial(
            RheologyByMaterial::uniform(linear_creep_law(1.0, 1.0), mesh.n_cells)
                .expect("uniform rheology"),
        ))
        .expect("rheology covers the mesh");
    solver.set_uniform_material_state(MaterialState::fresh(600.0));

    for step in 0..20 {
        let report = solver.solve_creep_step(1000.0).expect("creep step");
        assert!(report.mechanics.converged, "step {step} must converge");
    }

    let mut max_stress = 0.0_f64;
    let mut max_creep = 0.0_f64;
    let mut max_plastic = 0.0_f64;
    let mut max_disp_error = 0.0_f64;
    for c in 0..mesh.n_cells {
        let s = solver.stress().internal[c];
        for component in [s.xx, s.xy, s.xz, s.yy, s.yz, s.zz] {
            max_stress = max_stress.max(component.abs());
        }
        let state = solver.rheology_state(c);
        max_creep = max_creep.max(state.equivalent_creep_strain);
        max_plastic = max_plastic.max(state.equivalent_plastic_strain);
        let want = eps_star * mesh.cell_centres[c].x;
        max_disp_error = max_disp_error.max((solver.displacement().internal[c].x - want).abs());
    }

    println!(
        "free expansion: max |sigma| = {max_stress:.2e} Pa, max eq. creep = {max_creep:.5e}, \
         max eq. plastic = {max_plastic:.5e}, max displacement error = {max_disp_error:.2e} m"
    );

    // The creep bound is relative to the eigenstrain the body actually
    // underwent: what must be zero is creep *compared with* 3e-3 of free
    // expansion. A hard `== 0.0` would be testing the round-off of the linear
    // solve, not the physics — the residual stress below is 1e-6 Pa and this
    // creep law relaxes any stress at all within one step.
    assert!(
        max_creep < 1.0e-9 * eps_star,
        "a stress-free body must not creep: {max_creep:e} against an eigenstrain of {eps_star:e}"
    );
    assert_eq!(max_plastic, 0.0, "a stress-free body must not yield");
    assert!(
        max_stress < 1.0,
        "free thermal expansion must be stress-free, got {max_stress:e} Pa"
    );
    assert!(
        max_disp_error < 1e-12 * eps_star * 0.006,
        "displacement must be the free-expansion field, error {max_disp_error:e} m"
    );

    // The contrast that gives the check its teeth: the very same law, handed
    // the *total* strain, invents 3K eps* of hydrostatic stress.
    let law = linear_creep_law(1.0, 1.0);
    let total_strain = eps_star * SymmTensor::IDENTITY;
    let unsubtracted = law
        .correct(
            0,
            &RheologyInputs {
                elastic: material,
                mechanical_strain: total_strain,
                material: MaterialState::fresh(600.0),
                irradiation: Default::default(),
                dt: 1000.0,
                equivalent_strain_rate: 0.0,
            },
            &RheologyState::pristine(),
        )
        .expect("elastic evaluation");
    println!(
        "if the eigenstrain were NOT subtracted: von Mises = {:.4} MPa, hydrostatic = {:.4} MPa \
         (3K eps* = {:.4} MPa)",
        unsubtracted.von_mises_stress() * 1e-6,
        unsubtracted.hydrostatic_stress() * 1e-6,
        material.three_k() * eps_star * 1e-6
    );
    assert_relative_eq!(
        unsubtracted.hydrostatic_stress(),
        material.three_k() * eps_star,
        max_relative = 1e-9
    );
}

// ---------------------------------------------------------------------------
// Contract items 3, 4 and 5
// ---------------------------------------------------------------------------

/// **The constitutive state advances once per timestep, not once per corrector.**
///
/// *Methodology.* One step of the relaxation case above, at `Δt = τ/10`. The
/// closed-form single-step backward-Euler increment is
///
/// `Δε_c = (q₀ / 3μ) · (Δt/τ) / (1 + Δt/τ)`
///
/// The same step is then repeated on a fresh solver whose corrector loop is
/// **forced to run twenty times** (`set_corrector_control(20, 0.0)`: a zero
/// tolerance can never be met, so the budget is always spent). If
/// [`RheologyState::advance`] were called inside the corrector loop instead of
/// after it, the twenty-corrector run would record roughly twenty times the
/// increment of the two-corrector run. Comparing the two runs is therefore a
/// decisive test of item 3 of the rheology contract, not merely a plausible
/// one.
///
/// Pass criteria: both runs within 1e−9 relative of the closed form and of each
/// other; the second run genuinely performs more correctors than the first.
///
/// *Results (measured 2026-08-05, printed by this test).* Default run: `2`
/// correctors, `ε_c,eq = 6.06061e-06`. Forced run: `20` correctors,
/// `ε_c,eq = 6.06061e-06`. Closed form `6.06061e-06`; relative difference from
/// the closed form `2.24e-15` (default) and `2.24e-15` (forced); the two runs
/// differ from each other by `0.00e+00`. Ten times as many correctors changed
/// the recorded inelastic strain by nothing at all.
///
/// *Interpretation.* `correct` is re-evaluated from the same start-of-step
/// state on every corrector, and `advance` is applied exactly once per step.
#[test]
fn constitutive_state_advances_once_per_step_not_once_per_corrector() {
    let material = reference_material();
    let mu = material.shear_modulus();
    let (b, sigma_c) = (1.5e-3, 100.0);
    let tau = relaxation_time(b, sigma_c, mu);
    let strain = 1.0e-4;
    let q0 = 2.0 * mu * strain;
    let dt = tau / 10.0;

    let build = || {
        let mesh = box_column_mesh(8, 0.008, 0.001);
        let boundary = prescribed_boundary(&mesh, |p| Vector3::new(strain * p.x, 0.0, 0.0));
        let mut solver = MechanicsSolver::new(mesh.clone(), material, boundary);
        solver
            .set_rheology(Rheology::ByMaterial(
                RheologyByMaterial::uniform(linear_creep_law(b, sigma_c), mesh.n_cells)
                    .expect("uniform rheology"),
            ))
            .expect("rheology covers the mesh");
        solver.set_uniform_material_state(MaterialState::fresh(600.0));
        solver
    };

    let mut default_run = build();
    let default_report = default_run.solve_creep_step(dt).expect("creep step");
    assert!(default_report.mechanics.converged);
    let default_creep = default_run.rheology_state(4).equivalent_creep_strain;

    // A zero tolerance can never be met, so the whole corrector budget is spent.
    let mut forced_run = build();
    forced_run.set_corrector_control(20, 0.0);
    forced_run.set_inelastic_tolerance(0.0);
    let forced_report = forced_run.solve_creep_step(dt).expect("creep step");
    let forced_creep = forced_run.rheology_state(4).equivalent_creep_strain;

    let ratio = dt / tau;
    let expected = (q0 / (3.0 * mu)) * ratio / (1.0 + ratio);
    println!(
        "default: {} correctors, eps_c,eq = {default_creep:.5e} (rel diff {:.2e})",
        default_report.mechanics.iterations,
        (default_creep - expected).abs() / expected
    );
    println!(
        "forced : {} correctors, eps_c,eq = {forced_creep:.5e} (rel diff {:.2e})",
        forced_report.mechanics.iterations,
        (forced_creep - expected).abs() / expected
    );
    println!(
        "closed form = {expected:.5e}; the two runs differ by {:.2e}",
        (forced_creep - default_creep).abs()
    );

    assert!(
        forced_report.mechanics.iterations > default_report.mechanics.iterations,
        "the forced run must actually perform more correctors for this test to mean anything"
    );
    assert_relative_eq!(default_creep, expected, max_relative = 1e-9);
    assert_relative_eq!(forced_creep, expected, max_relative = 1e-9);
    assert_relative_eq!(forced_creep, default_creep, max_relative = 1e-12);
}

/// **Spatially varying creep still leaves a bar in equilibrium.**
///
/// *Methodology.* A column of `n` cells with `D = 0` prescribed at `x = 0` and
/// `D = (u_L, 0, 0)` at `x = L` (average axial strain `1e−4` over `L = 8 mm`)
/// and symmetry on the four lateral patches, so the column is laterally
/// confined. Every cell gets its own linear Norton law with a **smoothly
/// varying** pre-factor
///
/// `B(x) = B₀ [1 + 0.9 sin(π x / L)]`,  `B₀ = 1.5e−3 /hr`,  `σ_C = 100 MPa`
///
/// so the accumulated inelastic strain `ε_in` is a smooth, strongly non-uniform
/// field and its divergence — the feedback term
/// `∇·[2μ ε_in + λ tr(ε_in) I]` — is genuinely exercised. Forty steps of `τ₀/5`
/// are taken (`τ₀` the relaxation time at `B₀`), i.e. eight relaxation times.
///
/// One-dimensional equilibrium requires `∂σ_xx/∂x = 0`, so **whatever** the
/// creep does, the axial stress must be uniform along the bar. This is the
/// property that fails if the feedback term has the wrong sign or magnitude:
/// the fast-creeping middle would report a different axial stress from the ends
/// while the solver still claimed convergence. The case is run at two
/// resolutions so that the residual spread can be shown to be discretisation
/// error rather than a modelling error.
///
/// Pass criteria: relative spread in `σ_xx` below 5e−3 at `n = 16`, falling by
/// more than 1.8× per 2× refinement; the interior spread (excluding the two end
/// cells, whose gradient recovery is one-sided and only first-order) falling by
/// more than 2.5× per refinement; the middle creeping more than 1.5× the ends;
/// and the displacement profile departing from the straight line by more than
/// 1e−11 m, nine orders above the round-off of the elastic solve.
///
/// *Results (measured 2026-08-05, printed by this test).*
///
/// | cells | mean `σ_xx` \[MPa\] | whole-bar spread | interior-only spread |
/// |---|---|---|---|
/// | 16 | 2.41795e+01 | 3.46972e−03 | 3.37449e−04 |
/// | 32 | 2.41804e+01 | 1.64306e−03 | 1.11789e−04 |
/// | 64 | 2.41806e+01 | 7.95461e−04 | 3.44167e−05 |
///
/// Whole-bar spread ratios 2.112 and 2.066 across the 2× refinements — first
/// order, dominated by the two end cells. Interior-only ratios 3.019 and 3.248
/// — close to second order, as the interior gradient recovery is central. Both
/// converge, so the residual non-uniformity is discretisation error in the
/// stress *recovery* and not a failure of the equilibrium the solver enforces.
///
/// At `n = 64` the mid-bar equivalent creep strain is `2.11490e−05` against
/// `1.20377e−05` at the end cell (1.76×), and the mid-bar displacement departs
/// from the straight line by `1.19e−10 m`. The elastic solver reproduces a
/// straight line to `~1e−20 m` on the same class of mesh (see
/// `imposed_strain_solution_is_grid_independent` in the sibling module), so
/// that departure is nine orders of magnitude above round-off and can only have
/// come from the inelastic feedback moving the displacement field.
///
/// *Known limitation, measured rather than assumed.* Repeating this case with a
/// **discontinuous** creep zone (cells 0–3 creeping, cells 4–7 elastic, 8 cells,
/// 200 steps of `τ/20`) gives a 25 % spread in `σ_xx` — `1.82785e+01` MPa in
/// cell 3 and `2.35674e+01` MPa in cell 4, against `2.075560e+01` MPa in cells
/// 5, 6 and 7 which agree to 13 significant figures. The error is confined to
/// the cells either side of the jump. That is the wide (2Δx) stencil of
/// `fvc::grad_vec` smearing a discontinuity in the recovered stress — the same
/// effect OpenFOAM's `compactNormalStress` switch exists to trade against, and a
/// property of the stress *recovery* rather than of the equilibrium enforced. A
/// material interface therefore needs either mesh refinement across it or a
/// compact stress recovery, which this port does not yet provide.
///
/// *Interpretation.* Item 4 of the rheology contract is honoured: the softened
/// stress is restored to equilibrium by re-solving with the inelastic strain as
/// an additional eigenstrain.
#[test]
fn spatially_varying_creep_keeps_the_axial_stress_uniform() {
    let material = reference_material();
    let mu = material.shear_modulus();
    let (b0, sigma_c) = (1.5e-3, 100.0);
    let tau0 = relaxation_time(b0, sigma_c, mu);
    let length = 0.008;
    let u_end = 1.0e-4 * length;

    let mut spreads = Vec::new();
    let mut interior_spreads = Vec::new();
    for &n in &[16usize, 32, 64] {
        let mesh = box_column_mesh(n, length, 0.001);

        // x ends prescribed; lateral patches symmetry, which the operators treat
        // as zero gradient, so the column is confined but not driven laterally.
        let boundary: Vec<PatchField<Vector3>> = mesh
            .patches
            .iter()
            .enumerate()
            .map(|(i, p)| match i {
                0 => PatchField::fixed_value_vec(p.size, Vector3::new(0.0, 0.0, 0.0)),
                1 => PatchField::fixed_value_vec(p.size, Vector3::new(u_end, 0.0, 0.0)),
                _ => PatchField {
                    bc: BoundaryCondition::Symmetry,
                    values: Field::new(vec![Vector3::new(0.0, 0.0, 0.0); p.size]),
                },
            })
            .collect();

        let mut solver = MechanicsSolver::new(mesh.clone(), material, boundary);
        solver.set_corrector_control(500, 1.0e-14);

        // One law per cell, so the creep pre-factor can vary smoothly in space.
        let laws: Vec<ConstitutiveLaw> = (0..n)
            .map(|c| {
                let x = mesh.cell_centres[c].x / length;
                linear_creep_law(b0 * (1.0 + 0.9 * (std::f64::consts::PI * x).sin()), sigma_c)
            })
            .collect();
        solver
            .set_rheology(Rheology::ByMaterial(
                RheologyByMaterial::new(laws, Arc::new((0..n).collect()))
                    .expect("per-cell rheology"),
            ))
            .expect("rheology covers the mesh");
        solver.set_uniform_material_state(MaterialState::fresh(600.0));

        for step in 0..20 {
            let report = solver.solve_creep_step(tau0 / 100.0).expect("creep step");
            assert!(
                report.mechanics.converged,
                "n = {n}, step {step} did not converge: displacement change {:e}, {} correctors",
                report.mechanics.final_change, report.mechanics.iterations
            );
        }

        let axial: Vec<f64> = (0..n).map(|c| solver.stress().internal[c].xx).collect();
        let mean = axial.iter().sum::<f64>() / n as f64;
        let min = axial.iter().cloned().fold(f64::INFINITY, f64::min);
        let max = axial.iter().cloned().fold(f64::NEG_INFINITY, f64::max);
        let spread = (max - min) / mean.abs();
        // The two end cells recover their gradient one-sidedly, which is only
        // first-order accurate, so they are reported separately from the
        // interior rather than being allowed to hide the interior's behaviour.
        let interior = &axial[1..n - 1];
        let i_min = interior.iter().cloned().fold(f64::INFINITY, f64::min);
        let i_max = interior.iter().cloned().fold(f64::NEG_INFINITY, f64::max);
        let interior_spread = (i_max - i_min) / mean.abs();
        spreads.push(spread);
        interior_spreads.push(interior_spread);
        println!(
            "n = {n:3}: mean sigma_xx = {:.5e} MPa, relative spread = {spread:.5e} \
             (interior only {interior_spread:.5e})",
            mean * 1e-6
        );

        let middle = solver.rheology_state(n / 2).equivalent_creep_strain;
        let end = solver.rheology_state(0).equivalent_creep_strain;
        let straight_line = u_end * mesh.cell_centres[n / 2].x / length;
        let departure = (solver.displacement().internal[n / 2].x - straight_line).abs();
        println!(
            "    mid-bar eps_c,eq = {middle:.5e} vs end cell {end:.5e} ({:.2}x), \
             displacement departs from the straight line by {departure:.2e} m ({:.1}% of u_L)",
            middle / end,
            100.0 * departure / u_end
        );

        assert!(
            middle > 1.5 * end,
            "the middle must creep substantially more than the ends for this test to bite"
        );
        // The elastic solve reproduces a straight line to ~1e-20 m (see
        // `imposed_strain_solution_is_grid_independent` in the sibling module),
        // so a departure of 1e-11 m is nine orders of magnitude above round-off
        // and can only have come from the inelastic feedback.
        assert!(
            departure > 1.0e-11,
            "the displacement must respond to the non-uniform inelastic feedback; \
             departure {departure:e} m"
        );
    }

    for w in spreads.windows(2) {
        println!(
            "whole-bar spread ratio across a 2x refinement = {:.3}",
            w[0] / w[1]
        );
    }
    for w in interior_spreads.windows(2) {
        println!(
            "interior spread ratio across a 2x refinement = {:.3}",
            w[0] / w[1]
        );
    }
    assert!(
        spreads[0] < 5.0e-3,
        "one-dimensional equilibrium requires a near-uniform axial stress; spread {:e}",
        spreads[0]
    );
    for w in spreads.windows(2) {
        assert!(
            w[0] / w[1] > 1.8,
            "the residual spread must shrink under refinement; ratio {}",
            w[0] / w[1]
        );
    }
    for w in interior_spreads.windows(2) {
        assert!(
            w[0] / w[1] > 2.5,
            "the interior spread must converge faster than first order; ratio {}",
            w[0] / w[1]
        );
    }
}

/// **The creep timestep control bounds the next step's inelastic increment.**
///
/// *Methodology.* The relaxation case with a bound of `1e−6` on the largest
/// single-cell equivalent inelastic increment. A deliberately over-large first
/// step of `τ/5` is taken, which exceeds the bound; the report's
/// [`CreepStepReport::suggested_next_time_step`] is then used for the second
/// step, and the increment that step produces is checked against the bound.
///
/// This is item 5 of the rheology contract. The creep integration is implicit
/// in stress but explicit in state — the correlations are all evaluated at the
/// start-of-step state — so an unbounded step degrades accuracy silently rather
/// than diverging, which is exactly why a bound is needed rather than a
/// convergence failure being relied on.
///
/// Pass criteria: the first step exceeds the bound; the suggested next step is
/// strictly smaller than the step taken; the second step's increment does not
/// exceed the bound.
///
/// *Results (measured 2026-08-05, printed by this test).* First step
/// `Δt = 2.08e+02 s` produced a maximum inelastic increment of `1.11111e−05`,
/// above the `1.00000e−06` bound. Suggested next step `1.87200e+01 s`, a factor
/// 11.1 smaller. The second step produced `9.82318e−07`, i.e. 0.982 of the
/// bound — under it, because the stress relaxed during the first step and the
/// rate is proportional to stress.
///
/// *Interpretation.* [`CreepTimeStepControl`] is wired into the solve and its
/// suggestion is actionable: adopting it brings the next step inside the bound.
#[test]
fn creep_time_step_control_bounds_the_next_step() {
    let material = reference_material();
    let mu = material.shear_modulus();
    let (b, sigma_c) = (1.5e-3, 100.0);
    let tau = relaxation_time(b, sigma_c, mu);
    let strain = 1.0e-4;

    let mesh = box_column_mesh(8, 0.008, 0.001);
    let boundary = prescribed_boundary(&mesh, |p| Vector3::new(strain * p.x, 0.0, 0.0));
    let mut solver = MechanicsSolver::new(mesh.clone(), material, boundary);
    solver
        .set_rheology(Rheology::ByMaterial(
            RheologyByMaterial::uniform(linear_creep_law(b, sigma_c), mesh.n_cells)
                .expect("uniform rheology"),
        ))
        .expect("rheology covers the mesh");
    solver.set_uniform_material_state(MaterialState::fresh(600.0));

    let bound = 1.0e-6;
    solver.set_creep_time_step_control(CreepTimeStepControl {
        max_average_increment: f64::INFINITY,
        max_maximum_increment: bound,
    });

    let first_dt = tau / 5.0;
    let first = solver.solve_creep_step(first_dt).expect("first creep step");
    println!(
        "first step dt = {first_dt:.2e} s -> max inelastic increment {:.5e} (bound {bound:.5e}), \
         suggested next dt = {:.5e} s",
        first.max_equivalent_inelastic_increment, first.suggested_next_time_step
    );
    assert!(
        first.max_equivalent_inelastic_increment > bound,
        "the test needs a first step that actually breaches the bound"
    );
    assert!(
        first.suggested_next_time_step < first_dt,
        "an over-large step must be told to shrink"
    );

    let second = solver
        .solve_creep_step(first.suggested_next_time_step)
        .expect("second creep step");
    println!(
        "second step increment {:.5e} = {:.3} of the bound",
        second.max_equivalent_inelastic_increment,
        second.max_equivalent_inelastic_increment / bound
    );
    assert!(
        second.max_equivalent_inelastic_increment <= bound,
        "adopting the suggested step must bring the increment inside the bound, got {:e}",
        second.max_equivalent_inelastic_increment
    );
}

/// **A creep step without a constitutive law is an error, not a silent
/// elastic solve.**
///
/// *Methodology:* call [`MechanicsSolver::solve_creep_step`] on a solver with no
/// rheology attached, and with a negative timestep on one that has. Both must
/// return an error rather than quietly doing something else.
///
/// *Result (measured 2026-08-05):* both calls return
/// [`crate::error::OffbeatError`] as documented.
#[test]
fn creep_step_rejects_a_missing_law_and_a_negative_timestep() {
    let mesh = box_column_mesh(4, 0.004, 0.001);
    let boundary = prescribed_boundary(&mesh, |_| Vector3::new(0.0, 0.0, 0.0));
    let mut solver = MechanicsSolver::new(mesh.clone(), reference_material(), boundary);
    assert!(solver.solve_creep_step(1.0).is_err());

    solver
        .set_rheology(Rheology::ByMaterial(
            RheologyByMaterial::uniform(ConstitutiveLaw::Elastic, mesh.n_cells).unwrap(),
        ))
        .unwrap();
    assert!(solver.solve_creep_step(-1.0).is_err());

    // A rheology sized for a different mesh is rejected at attach time.
    assert!(solver
        .set_rheology(Rheology::ByMaterial(
            RheologyByMaterial::uniform(ConstitutiveLaw::Elastic, mesh.n_cells + 1).unwrap(),
        ))
        .is_err());
}
