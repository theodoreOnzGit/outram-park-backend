// SPDX-License-Identifier: GPL-3.0-only
// Copyright (C) 2026 OUTRAM PARK contributors
//
// This file is part of OUTRAM PARK. See `src/lib.rs` for the full licence
// notice.

//! Verification of the small-strain mechanics solver.
//!
//! # What is verified here, and what is not
//!
//! These are **verification** tests in the workspace's sense — "is the equation
//! implemented correctly?" — checked against closed-form elasticity solutions.
//! They are *not* validation: nothing here compares the port to OFFBEAT output
//! or to fuel-irradiation data, and no test in this file may be cited as
//! evidence that the port reproduces experiment. Validation against
//! `Cases/Verification` is separate work under bead `op-6sl.1` and is the
//! maintainer's to sign off.

use std::sync::Arc;

use approx::assert_relative_eq;
use outram_foam_basic_lib::fields::boundary::bc::{BoundaryCondition, PatchField};
use outram_foam_basic_lib::mesh::{BoundaryPatch, FvMesh, FvMeshBuilder, PatchKind};
use outram_foam_basic_lib::primitives::Vector3;

use super::*;

/// A 1-D column of `n` cells spanning `length` metres along **x**, with
/// cross-sectional area `area` and one patch at each end (`0` at `x = 0`,
/// `1` at `x = L`).
///
/// Built with the same `FvMeshBuilder` pattern the multiphase V&V cases use, so
/// these tests run on the same class of mesh the rest of the workspace verifies
/// against.
///
/// Note there are **no lateral patches**: the y and z directions carry no faces,
/// so `∇D` has no transverse component and the column is implicitly confined.
/// That is exactly the oedometric condition the stress tests below expect, and
/// it is why they compare against `(2μ+λ)ε` rather than `Eε`.
fn line_mesh(n: usize, length: f64, area: f64) -> Arc<FvMesh> {
    let dx = length / n as f64;
    let n_int = n - 1;
    let v3 = Vector3::new;

    let mut owner: Vec<usize> = (0..n_int).collect();
    let neighbour: Vec<usize> = (1..n).collect();
    owner.push(0);
    owner.push(n - 1);

    let cell_volumes = vec![dx * area; n];
    let cell_centres: Vec<Vector3> = (0..n)
        .map(|i| v3((i as f64 + 0.5) * dx, 0.0, 0.0))
        .collect();

    let mut face_area_vectors: Vec<Vector3> = (0..n_int).map(|_| v3(area, 0.0, 0.0)).collect();
    face_area_vectors.push(v3(-area, 0.0, 0.0));
    face_area_vectors.push(v3(area, 0.0, 0.0));

    let mut face_centres: Vec<Vector3> = (0..n_int)
        .map(|i| v3((i as f64 + 1.0) * dx, 0.0, 0.0))
        .collect();
    face_centres.push(v3(0.0, 0.0, 0.0));
    face_centres.push(v3(length, 0.0, 0.0));

    Arc::new(
        FvMeshBuilder::new()
            .n_cells(n)
            .n_internal_faces(n_int)
            .owner(owner)
            .neighbour(neighbour)
            .patches(vec![
                BoundaryPatch::new("left", n_int, 1, PatchKind::Wall),
                BoundaryPatch::new("right", n_int + 1, 1, PatchKind::Wall),
            ])
            .cell_volumes(cell_volumes)
            .cell_centres(cell_centres)
            .face_area_vectors(face_area_vectors)
            .face_centres(face_centres)
            .build()
            .expect("1-D line mesh must build"),
    )
}

/// Displacement boundary conditions clamping every patch to zero.
fn clamped_all(mesh: &FvMesh) -> Vec<PatchField<Vector3>> {
    mesh.patches
        .iter()
        .map(|p| PatchField::fixed_value_vec(p.size, Vector3::new(0.0, 0.0, 0.0)))
        .collect()
}

/// Reference material: a stand-in elastic solid, not a specific fuel.
///
/// `E = 200 GPa`, `ν = 0.3` — round numbers chosen so the closed-form results
/// below can be checked by hand.
fn reference_material() -> LinearElastic {
    LinearElastic::new(200.0e9, 0.3).expect("reference material is admissible")
}

/// **Lamé parameter identities.**
///
/// *Methodology:* evaluate `μ`, `λ` and `3K` for `E = 200 GPa`, `ν = 0.3`
/// against the textbook closed forms `μ = E/(2(1+ν))`, `λ = Eν/((1+ν)(1−2ν))`,
/// `3K = E/(1−2ν)`. Pass criterion: relative error below 1e-12.
///
/// *Result:* `μ = 76.923 GPa`, `λ = 115.385 GPa`, `3K = 500.0 GPa`, all matching
/// the closed forms to machine precision (measured 2026-07-29).
#[test]
fn lame_parameters_match_closed_form() {
    let m = reference_material();
    let (e, nu) = (m.young, m.poisson);

    assert_relative_eq!(
        m.shear_modulus(),
        e / (2.0 * (1.0 + nu)),
        max_relative = 1e-12
    );
    assert_relative_eq!(
        m.lame_lambda(),
        e * nu / ((1.0 + nu) * (1.0 - 2.0 * nu)),
        max_relative = 1e-12
    );
    assert_relative_eq!(m.three_k(), e / (1.0 - 2.0 * nu), max_relative = 1e-12);

    // 3K = 3λ + 2μ is the identity tying the three together; if any one of the
    // formulas above were wrong this would catch it.
    assert_relative_eq!(
        m.three_k(),
        3.0 * m.lame_lambda() + 2.0 * m.shear_modulus(),
        max_relative = 1e-12
    );
}

/// **Incompressible and unphysical constants are rejected.**
///
/// `ν = 0.5` makes `λ` and `3K` infinite; a solver that silently accepted it
/// would produce `NaN` fields several steps later, far from the cause.
#[test]
fn unphysical_elastic_constants_are_rejected() {
    assert!(LinearElastic::new(200.0e9, 0.5).is_err());
    assert!(LinearElastic::new(200.0e9, -1.0).is_err());
    assert!(LinearElastic::new(0.0, 0.3).is_err());
    assert!(LinearElastic::new(-1.0, 0.3).is_err());
}

/// **Fully constrained uniform eigenstrain — the analytic acceptance case.**
///
/// *Methodology:* a 20-cell 1-D column, every patch clamped to zero
/// displacement, subjected to a spatially uniform linear eigenstrain
/// `ε* = α ΔT = 1e-5 × 300 = 3e-3`. With all boundaries fixed and the load
/// uniform, the exact solution is `D ≡ 0` (the interior body force `∇(3K ε*)`
/// vanishes for uniform `ε*`, and zero displacement satisfies every boundary
/// condition). The constitutive law then gives a purely hydrostatic stress
///
/// `σ_xx = σ_yy = σ_zz = −3K ε*`,  off-diagonals zero.
///
/// This is the textbook result for a body prevented from expanding: it goes into
/// hydrostatic compression, and the magnitude depends on the bulk modulus alone.
/// Pass criterion: relative error below 1e-8 on the normal components,
/// off-diagonals below 1e-6 Pa absolute.
///
/// *Result:* `σ_xx = −1.500 GPa` against the closed-form `−3K ε* = −500 GPa ×
/// 3e-3 = −1.500 GPa`; agreement to better than 1e-10 relative, off-diagonals
/// zero to machine precision (measured 2026-07-29, `E = 200 GPa`, `ν = 0.3`).
/// Interpretation: the eigenstrain load, the `3K` factor and the stress update
/// are mutually consistent, and the sign convention is tension-positive as
/// documented.
#[test]
fn fully_constrained_eigenstrain_gives_hydrostatic_compression() {
    let mesh = line_mesh(20, 0.01, 1.0e-4);
    let material = reference_material();
    let mut solver = MechanicsSolver::new(mesh.clone(), material, clamped_all(&mesh));

    let alpha = 1.0e-5;
    let delta_t = 300.0;
    let eig = Eigenstrain::thermal(alpha, delta_t);
    solver.set_uniform_eigenstrain(eig);

    let report = solver.solve_quasi_static();
    assert!(
        report.converged,
        "quasi-static solve must converge; final change {:e} after {} iterations",
        report.final_change, report.iterations
    );

    let expected = -material.three_k() * eig.total();
    let sigma = solver.stress();

    // Check away from the clamped ends, where the one-sided gradient stencil is
    // exact for this constant-field solution but the patch treatment is not
    // what the analytic result is about.
    for c in 2..(mesh.n_cells - 2) {
        let s = sigma.internal[c];
        assert_relative_eq!(s.xx, expected, max_relative = 1e-8);
        assert_relative_eq!(s.yy, expected, max_relative = 1e-8);
        assert_relative_eq!(s.zz, expected, max_relative = 1e-8);
        assert!(s.xy.abs() < 1e-6, "shear xy should vanish, got {}", s.xy);
        assert!(s.xz.abs() < 1e-6, "shear xz should vanish, got {}", s.xz);
        assert!(s.yz.abs() < 1e-6, "shear yz should vanish, got {}", s.yz);
    }

    // The displacement itself must be (essentially) zero.
    assert!(
        report.max_displacement < 1.0e-12,
        "clamped uniform eigenstrain must not displace material, got {:e} m",
        report.max_displacement
    );
}

/// **Linearity in the eigenstrain.**
///
/// *Methodology:* run the clamped case above at `ε*` and at `2ε*` and compare
/// the resulting stress. Linear elasticity with an eigenstrain load is a linear
/// operator, so doubling the load must double the stress exactly. Pass
/// criterion: relative error below 1e-9.
///
/// *Result:* doubling `ΔT` from 300 K to 600 K doubles `σ_xx` from −1.500 GPa to
/// −3.000 GPa, to better than 1e-12 relative (measured 2026-07-29). This is a
/// self-consistency check on the implementation, not a comparison with an
/// external reference.
#[test]
fn stress_is_linear_in_eigenstrain() {
    let mesh = line_mesh(12, 0.01, 1.0e-4);
    let material = reference_material();

    let solve_at = |delta_t: f64| {
        let mut s = MechanicsSolver::new(mesh.clone(), material, clamped_all(&mesh));
        s.set_uniform_eigenstrain(Eigenstrain::thermal(1.0e-5, delta_t));
        let r = s.solve_quasi_static();
        assert!(r.converged);
        s.stress().internal[mesh.n_cells / 2].xx
    };

    let single = solve_at(300.0);
    let double = solve_at(600.0);
    assert_relative_eq!(double, 2.0 * single, max_relative = 1e-9);
}

/// **Eigenstrain sources are interchangeable.**
///
/// *Methodology:* the momentum balance sees only [`Eigenstrain::total`], so a
/// linear thermal strain of 3e-3 and a swelling strain of 3e-3 must produce
/// identical stress; and a swelling of +3e-3 combined with a densification of
/// −3e-3 must produce none. Pass criterion: identical stress to 1e-12 relative,
/// and below 1e-3 Pa absolute for the cancelling case.
///
/// *Result:* both held at machine precision (measured 2026-07-29). This
/// verifies the documented sign convention — densification opposes swelling —
/// which is the error this test exists to catch. Self-consistency check, not
/// external validation.
#[test]
fn eigenstrain_sources_superpose_and_cancel() {
    let mesh = line_mesh(12, 0.01, 1.0e-4);
    let material = reference_material();

    let solve_with = |eig: Eigenstrain| {
        let mut s = MechanicsSolver::new(mesh.clone(), material, clamped_all(&mesh));
        s.set_uniform_eigenstrain(eig);
        assert!(s.solve_quasi_static().converged);
        s.stress().internal[mesh.n_cells / 2].xx
    };

    let thermal_only = solve_with(Eigenstrain {
        thermal: 3.0e-3,
        ..Eigenstrain::default()
    });
    let swelling_only = solve_with(Eigenstrain {
        swelling: 3.0e-3,
        ..Eigenstrain::default()
    });
    assert_relative_eq!(thermal_only, swelling_only, max_relative = 1e-12);

    let cancelling = solve_with(Eigenstrain {
        swelling: 3.0e-3,
        densification: -3.0e-3,
        ..Eigenstrain::default()
    });
    assert!(
        cancelling.abs() < 1.0e-3,
        "swelling and equal densification must cancel, got {cancelling} Pa"
    );
}

/// **Imposed uniform strain reproduces the oedometric stiffness.**
///
/// *Methodology:* clamp `x = 0` at zero displacement and impose `u_x = u_L` on
/// the far end of a column of length `L`, with no eigenstrain and the lateral
/// directions constrained by the 1-D mesh. The exact solution is a uniform
/// strain `ε_xx = u_L / L` with every other strain component zero, giving
///
/// `σ_xx = (2μ + λ) ε_xx`
///
/// — the oedometric (confined) stiffness, *not* `E ε_xx`, because lateral
/// contraction is prevented. Pass criterion: relative error below 1e-6 in the
/// interior.
///
/// *Result:* for `u_L = 1e-6 m` over `L = 0.01 m` (`ε_xx = 1e-4`), `σ_xx =
/// 26.923 MPa` against the closed-form `(2μ+λ) ε_xx = 269.23 GPa × 1e-4 =
/// 26.923 MPa`; agreement to 3.3e-14 relative (measured 2026-07-29).
/// Interpretation: the implicit Laplacian coefficient and the explicit stress
/// correction combine to the correct confined stiffness, which is the property
/// the segregated split is most likely to get wrong.
#[test]
fn imposed_uniform_strain_gives_oedometric_stress() {
    let n = 40;
    let length = 0.01;
    let mesh = line_mesh(n, length, 1.0e-4);
    let material = reference_material();
    let u_end = 1.0e-6;

    // Patch 0 is the x = 0 end, patch 1 the x = L end, per `line_1d`.
    let boundary: Vec<PatchField<Vector3>> = mesh
        .patches
        .iter()
        .enumerate()
        .map(|(i, p)| match i {
            0 => PatchField::fixed_value_vec(p.size, Vector3::new(0.0, 0.0, 0.0)),
            1 => PatchField::fixed_value_vec(p.size, Vector3::new(u_end, 0.0, 0.0)),
            _ => PatchField {
                bc: BoundaryCondition::Symmetry,
                values: outram_foam_basic_lib::fields::Field::new(vec![
                    Vector3::new(0.0, 0.0, 0.0);
                    p.size
                ]),
            },
        })
        .collect();

    let mut solver = MechanicsSolver::new(mesh.clone(), material, boundary);
    solver.set_corrector_control(500, 1.0e-14);
    let report = solver.solve_quasi_static();
    assert!(
        report.converged,
        "solve must converge; final change {:e} after {} iterations",
        report.final_change, report.iterations
    );

    let strain = u_end / length;
    let expected = material.two_mu_plus_lambda() * strain;
    let sigma = solver.stress();

    for c in 5..(n - 5) {
        assert_relative_eq!(sigma.internal[c].xx, expected, max_relative = 1e-6);
    }
}

/// **Displacement is linear between the clamped ends.**
///
/// *Methodology:* the same imposed-strain case as above; the exact solution is
/// `u_x(x) = u_L · x / L`. Compare the solved cell values against that line.
/// Pass criterion: relative error below 1e-6.
///
/// *Result:* the solved profile matches the analytic line to within 1.2e-19 m
/// absolute across the whole column (measured 2026-07-29, 40 cells). Together with the stress
/// test above this establishes that both the field and the derived stress are
/// right, not merely one compensating for the other.
#[test]
fn displacement_profile_is_linear() {
    let n = 40;
    let length = 0.01;
    let mesh = line_mesh(n, length, 1.0e-4);
    let u_end = 1.0e-6;

    let boundary: Vec<PatchField<Vector3>> = mesh
        .patches
        .iter()
        .enumerate()
        .map(|(i, p)| match i {
            0 => PatchField::fixed_value_vec(p.size, Vector3::new(0.0, 0.0, 0.0)),
            1 => PatchField::fixed_value_vec(p.size, Vector3::new(u_end, 0.0, 0.0)),
            _ => PatchField {
                bc: BoundaryCondition::Symmetry,
                values: outram_foam_basic_lib::fields::Field::new(vec![
                    Vector3::new(0.0, 0.0, 0.0);
                    p.size
                ]),
            },
        })
        .collect();

    let mut solver = MechanicsSolver::new(mesh.clone(), reference_material(), boundary);
    solver.set_corrector_control(500, 1.0e-14);
    assert!(solver.solve_quasi_static().converged);

    let disp = solver.displacement();
    for c in 0..n {
        let x = mesh.cell_centres[c].x;
        let expected = u_end * x / length;
        assert_relative_eq!(disp.internal[c].x, expected, max_relative = 1e-6);
    }
}

/// **Grid independence of the imposed-strain solution.**
///
/// *Methodology:* solve the imposed-uniform-strain case above on 10, 20, 40 and
/// 80 cells and compare the mid-column `σ_xx` with the closed-form
/// `(2μ+λ) ε_xx` at each resolution. The exact solution is linear in `x`, so a
/// correct finite-volume discretisation reproduces it exactly on **every** mesh;
/// an error that grows with refinement means the linear solve is
/// under-converged, not that the scheme is inaccurate. Pass criterion: relative
/// error below 1e-10 at every resolution, and no systematic growth with `n`.
///
/// *Result (measured 2026-07-29, `E = 200 GPa`, `ν = 0.3`, `u_L = 1e-6 m`,
/// `L = 0.01 m`):*
///
/// | cells | relative error in `σ_xx` | max displacement error \[m\] |
/// |---|---|---|
/// | 10 | 2.4e-14 | 2.7e-20 |
/// | 20 | 2.9e-14 | 6.1e-20 |
/// | 40 | 3.3e-14 | 1.2e-19 |
/// | 80 | 4.3e-14 | 2.6e-19 |
///
/// Errors sit at round-off and grow only in proportion to the cell count, as
/// accumulated floating-point error should. Interpretation: the discretisation
/// is exact for the linear solution at every resolution.
///
/// This test earns its keep as a regression guard on the **linear-solver
/// settings**. With the library default (1e-7 tolerance, 1000 sweeps) the same
/// case gives 2.7e-7 relative error at 10 cells but 1.3e-2 at 80 — a 1% stress
/// error that grows under refinement and would be easy to misread as a
/// discretisation problem. See the note in `MechanicsSolver::new`.
#[test]
fn imposed_strain_solution_is_grid_independent() {
    let length = 0.01;
    let u_end = 1.0e-6;
    let material = reference_material();
    let expected = material.two_mu_plus_lambda() * u_end / length;

    for n in [10usize, 20, 40, 80] {
        let mesh = line_mesh(n, length, 1.0e-4);
        let boundary: Vec<PatchField<Vector3>> = mesh
            .patches
            .iter()
            .enumerate()
            .map(|(i, p)| {
                let v = if i == 0 {
                    Vector3::new(0.0, 0.0, 0.0)
                } else {
                    Vector3::new(u_end, 0.0, 0.0)
                };
                PatchField::fixed_value_vec(p.size, v)
            })
            .collect();

        let mut solver = MechanicsSolver::new(mesh.clone(), material, boundary);
        let report = solver.solve_quasi_static();
        assert!(report.converged, "n = {n} must converge");

        let got = solver.stress().internal[n / 2].xx;
        assert_relative_eq!(got, expected, max_relative = 1e-10);

        // The displacement profile must be the exact analytic line at every
        // resolution too, not merely the stress derived from it.
        for c in 0..n {
            let x = mesh.cell_centres[c].x;
            let want = u_end * x / length;
            assert!(
                (solver.displacement().internal[c].x - want).abs() < 1e-15,
                "n = {n}, cell {c}: displacement error too large"
            );
        }
    }
}

/// **Methodology.** MATPRO fits Zircaloy's Young's and shear moduli as two
/// independent lines, so `ν = E/(2G) − 1` climbs with temperature and crosses
/// the incompressible limit `0.5`. Bead `op-6sl.7` asked which of three
/// policies [`LinearElastic::from_models`] should take — clamp `ν`, fall back
/// to a constant, or refuse — and the answer is **refuse**. This pins that
/// decision so a later "helpful" clamp cannot be added silently.
///
/// Three checks. Below the crossover the constructor succeeds and its `λ` is
/// finite and positive. Above it the constructor returns
/// [`OffbeatError::Unphysical`] rather than any number at all. And the
/// arithmetic that motivates the refusal is exhibited directly: `λ` is
/// evaluated at a sequence of `ν` approaching `0.5` to show it diverges, so a
/// clamp at `0.5 − ε` would report a stiffness fixed by `ε` rather than by the
/// material.
///
/// Inputs: `YoungModulusModel::MatproZircaloy` and
/// `PoissonRatioModel::MatproZircaloy` on a fresh (zero cold-work, zero burnup)
/// state. Pass criterion: `Ok` at 600 K with finite positive `λ`; `Err` at
/// 1500 K; and `λ(ν)` rising by at least three decades as `ε` falls three
/// decades.
///
/// **Results (measured 2026-08-05, release).** At 600 K the correlation gives
/// `ν = 0.386353680`, `E = 7.595e10` Pa, `λ = 9.312224092515265e10` Pa — an
/// ordinary Zircaloy stiffness. At 1500 K it gives `ν = 0.5674999999999999`,
/// and the constructor refuses.
///
/// The clamp sweep is the argument in one table:
///
/// | clamp | `λ` \[Pa\] |
/// |---|---|
/// | `ν = 0.5 − 1e-3` | 1.2641444296197453e13 |
/// | `ν = 0.5 − 1e-4` | 1.2656645443030928e14 |
/// | `ν = 0.5 − 1e-5` | 1.2658164554417703e15 |
/// | `ν = 0.5 − 1e-6` | 1.2658316455882982e16 |
///
/// Exactly one decade of `λ` per decade of `ε`, as `λ ≈ E/(3ε)` predicts. Set
/// against the material's own `9.31e10` Pa at 600 K, a clamp at `1e-6` would
/// report a bulk stiffness **five orders of magnitude** too large — and the
/// figure would be traceable to nothing but whoever picked `ε`. That is the
/// whole case for refusing.
#[test]
fn the_matpro_zircaloy_poisson_crossover_refuses_the_case_rather_than_clamping() {
    use crate::materials::properties::poisson_ratio::PoissonRatioModel;
    use crate::materials::properties::young_modulus::YoungModulusModel;

    let young = YoungModulusModel::MatproZircaloy;
    let poisson = PoissonRatioModel::MatproZircaloy;

    let cold = MaterialState::fresh(600.0);
    let admissible = LinearElastic::from_models(young, poisson, &cold)
        .expect("600 K is well below the 1354.84 K crossover");
    println!(
        "600 K:  nu = {:.9}  E = {:e} Pa  lambda = {:e} Pa",
        admissible.poisson,
        admissible.young,
        admissible.lame_lambda()
    );
    assert!(admissible.lame_lambda().is_finite() && admissible.lame_lambda() > 0.0);

    let hot = MaterialState::fresh(1500.0);
    let refused = LinearElastic::from_models(young, poisson, &hot);
    println!("1500 K: nu = {:.9}  ->  {refused:?}", poisson.value(&hot));
    assert!(
        matches!(refused, Err(OffbeatError::Unphysical { .. })),
        "above the crossover the case must be refused, not clamped"
    );

    // Why clamping would be worse than refusing: lambda is unbounded as
    // nu -> 0.5, so the clamp tolerance would set the answer.
    let e = admissible.young;
    let mut previous = f64::NAN;
    for decade in 3..=6 {
        let epsilon = 10.0_f64.powi(-decade);
        let nu = 0.5 - epsilon;
        let lambda = e * nu / ((1.0 + nu) * (1.0 - 2.0 * nu));
        println!("clamp at nu = 0.5 - 1e-{decade}  ->  lambda = {lambda:e} Pa");
        if previous.is_finite() {
            assert!(
                lambda > 9.0 * previous,
                "lambda must grow ~10x per decade of clamp tolerance"
            );
        }
        previous = lambda;
    }
}
