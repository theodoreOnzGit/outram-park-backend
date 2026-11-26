// SPDX-License-Identifier: GPL-3.0-only
// Copyright (C) 2026 OUTRAM PARK contributors
//
// This file is part of OUTRAM PARK, distributed under GPL-3.0. See the crate
// root or <https://www.gnu.org/licenses/> for the full license text.

//! Unit tests of the two-phase driver's FV layer and case reader. The
//! end-to-end runs on upstream's tutorial cases are integration tests in
//! `tests/genfoam_two_phase_1d_boiling.rs` (they need the upstream clone).

#[cfg(not(any(feature = "std", test)))]
#[allow(unused_imports)]
use crate::nostd_prelude::*;
use super::case::TwoPhaseCase;
use super::fv::{self, Bc, Eqn, Geometry, Scalar};
use outram_foam_basic_lib::interface::one_dimensional_meshing::create_one_d_mesh;
use outram_foam_basic_lib::prelude::*;
use uom::si::area::square_meter;
use uom::si::f64::{Area, Length};
use uom::si::length::meter;

fn line(n: i64) -> Geometry {
    let mesh = create_one_d_mesh(
        Length::new::<meter>(1.0),
        Area::new::<square_meter>(1e-2),
        n,
    )
    .expect("1-D mesh");
    Geometry::new(alloc::sync::Arc::new(mesh))
}

/// `fvc::reconstruct` of the flux of a uniform field returns that field in
/// the resolved direction and zero in the empty ones.
#[test]
fn reconstruct_recovers_a_uniform_velocity() {
    let g = line(8);
    let u = fv::Vector::uniform(&g, Vector3::new(0.7, 0.0, 0.0));
    let phi = fv::flux(&g, &u);
    for v in fv::reconstruct(&g, &phi) {
        assert!((v.x - 0.7).abs() < 1e-12 && v.y.abs() < 1e-12 && v.z.abs() < 1e-12, "{v:?}");
    }
}

/// `-laplacian(1, T)` with fixed ends reproduces the linear conduction
/// profile exactly (second-order operator on a linear solution).
#[test]
fn neg_laplacian_with_fixed_ends_gives_the_linear_profile() {
    let g = line(10);
    let mut bc = g.default_bcs();
    for (pi, p) in g.mesh.patches.iter().enumerate() {
        if p.kind != PatchKind::Empty {
            let v = if g.mesh.face_centres[p.start].x < 0.5 { 300.0 } else { 400.0 };
            bc[pi] = Bc::Fixed(vec![v; p.size]);
        }
    }
    let t = Scalar::uniform(&g, 350.0, bc);
    let mut e = Eqn::new(&g);
    let gamma = vec![1.0; g.n_faces()];
    e.neg_laplacian(&g, &gamma, &t);
    let mut x = vec![350.0; g.n_cells()];
    let _ = super::solve_krylov(&e, &mut x, 0.0);
    for c in 0..g.n_cells() {
        let exact = 300.0 + 100.0 * g.mesh.cell_centres[c].x;
        assert!((x[c] - exact).abs() < 1e-8, "cell {c}: {} vs {exact}", x[c]);
    }
}

#[cfg(feature = "std")]
/// The case reader rejects a directory that is not a case.
#[test]
fn reading_a_missing_case_is_an_error() {
    let r = TwoPhaseCase::read(std::path::Path::new("/nonexistent/genfoam/case"));
    assert!(r.is_err());
}

#[cfg(feature = "std")]
/// The vendored upstream `1D_boiling` case reads, builds, and steps through
/// the `ThermalHydraulicsSolver` enum; the fluid fractions fill the moving
/// volume exactly (`twoPhase.C` normalisation).
#[test]
fn one_d_boiling_case_steps_through_the_solver_enum() {
    use super::super::ThermalHydraulicsSolver;
    let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/data/genfoam_1d_boiling");
    let case = TwoPhaseCase::read(&dir).expect("read vendored 1D_boiling");
    assert!(case.deviations.is_empty(), "MULES is ported: the shipped case runs as shipped");
    assert!(matches!(case.controls.alpha, super::AlphaSolver::Mules(_)));
    // The port's earlier behaviour stays callable and is reported.
    let old = case.clone().with_alpha_solver_implicit_upwind().expect("implicitUpwind");
    assert_eq!(old.deviations.len(), 1, "MULES -> implicitUpwind is the only deviation");
    assert_eq!(old.controls.alpha, super::AlphaSolver::ImplicitUpwind);
    let mut th = ThermalHydraulicsSolver::TwoPhase(super::TwoPhaseSolver::new(case));
    for _ in 0..5 {
        th.step(1e-3).expect("step");
    }
    let ThermalHydraulicsSolver::TwoPhase(s) = th else {
        unreachable!("constructed as TwoPhase")
    };
    for c in 0..s.geom.n_cells() {
        let sum = s.fluids[0].alpha.c[c] + s.fluids[1].alpha.c[c] + s.structure.alpha.c[c];
        assert!((sum - 1.0).abs() < 1e-12, "cell {c}: {sum}");
        assert!(s.fluids[0].t.c[c].is_finite() && s.p.c[c].is_finite());
    }
    assert!((s.time - 5e-3).abs() < 1e-12);
}

// ───────────────────────── myMULES (gh:#1361 round 7) ─────────────────────────

use super::mules::{self, AlphaConvection, LimiterControls};

/// A zeroGradient scalar on `g` with the given cell values.
fn scalar_on(g: &Geometry, c: &[f64]) -> Scalar {
    let mut s = Scalar::uniform(g, 0.0, g.default_bcs());
    s.c = c.to_vec();
    s.correct_bc(g);
    s
}

#[test]
fn convection_scheme_words_are_parsed() {
    assert_eq!(AlphaConvection::from_words(&["Gauss", "vanLeer"]), Some(AlphaConvection::VanLeer));
    assert_eq!(AlphaConvection::from_words(&["Gauss", "upwind"]), Some(AlphaConvection::Upwind));
    assert_eq!(AlphaConvection::from_words(&["Gauss", "linear"]), Some(AlphaConvection::Linear));
    assert_eq!(AlphaConvection::from_words(&["Gauss", "QUICK"]), None);
}

/// `Gauss linear` gradient of a linear field is its slope in the interior cells (uniform mesh, `x` direction).
#[test]
fn gauss_linear_gradient_is_exact_for_a_linear_field_in_the_interior() {
    let g = line(10);
    let c: Vec<f64> = (0..10).map(|i| 2.0 * g.mesh.cell_centres[i].x + 1.0).collect();
    let grad = mules::gauss_linear_grad(&g, &scalar_on(&g, &c));
    for (i, v) in grad.iter().enumerate().take(9).skip(1) {
        assert!((v[0] - 2.0).abs() < 1e-12, "cell {i}: {v:?}");
        assert!(v[1].abs() < 1e-14 && v[2].abs() < 1e-14);
    }
}

/// vanLeer: on a linear profile the limiter is 1 (central differencing, the average of the two cells on a uniform
/// mesh); in front of a step the limiter is 0 (the upwind value); `upwind` and `linear` are the selections they name.
#[test]
fn van_leer_is_central_on_a_linear_profile_and_upwind_at_a_step() {
    let g = line(10);
    let flux = vec![1.0; g.n_faces()];
    let lin: Vec<f64> = (0..10).map(|i| 0.1 * i as f64).collect();
    let f = mules::interpolate(&g, AlphaConvection::VanLeer, &flux, &scalar_on(&g, &lin));
    for i in 2..7 {
        assert!((f[i] - 0.5 * (lin[i] + lin[i + 1])).abs() < 1e-12, "face {i}: {}", f[i]);
    }
    let step: Vec<f64> = (0..10).map(|i| if i < 5 { 0.0 } else { 1.0 }).collect();
    let f = mules::interpolate(&g, AlphaConvection::VanLeer, &flux, &scalar_on(&g, &step));
    assert_eq!(f[4], 0.0, "face between the last 0 cell and the first 1 cell takes the upwind (0) value");
    let up = mules::interpolate(&g, AlphaConvection::Upwind, &flux, &scalar_on(&g, &step));
    assert_eq!(up[4], 0.0);
    let fl = mules::interpolate(&g, AlphaConvection::Linear, &flux, &scalar_on(&g, &step));
    assert!((fl[4] - 0.5).abs() < 1e-12);
    let flx = mules::flux(&g, AlphaConvection::Upwind, &flux, &scalar_on(&g, &step));
    assert_eq!(flx[5], 1.0);
}

/// MULES explicit step: `psi = (psi0 r + Su - div(phiPsi)) / (r - Sp)`, checked by hand on a 3-cell line.
#[test]
fn explicit_update_follows_the_formula() {
    let g = line(3);
    let v = g.mesh.cell_volumes[0];
    let mut phi = vec![0.0; g.n_faces()];
    phi[0] = 2.0 * v; // face 0|1
    phi[1] = 1.0 * v; // face 1|2
    let psi0 = [1.0, 0.5, 0.25];
    let sp = [0.0, -2.0, 0.0];
    let su = [0.0, 0.0, 0.3];
    let mut psi = psi0.to_vec();
    mules::explicit_update(&g, 4.0, &mut psi, &psi0, &phi, &sp, &su);
    let want = [(1.0 * 4.0 - 2.0) / 4.0, (0.5 * 4.0 + (2.0 - 1.0)) / (4.0 + 2.0), (0.25 * 4.0 + 0.3 + 1.0) / 4.0];
    for c in 0..3 {
        assert!((psi[c] - want[c]).abs() < 1e-12, "cell {c}: {} vs {}", psi[c], want[c]);
    }
}

/// `limit(returnCorr)` with a flux that IS the upwind flux has nothing to limit: the correction is exactly 0.
/// With a central (anti-diffusive) flux across a step, the limited correction keeps the explicit update inside the
/// bounds [0, 1], which the unlimited central flux does not.
#[test]
fn mules_limiter_returns_a_zero_correction_for_upwind_and_keeps_a_step_bounded() {
    let g = line(10);
    let nf = g.n_faces();
    let area = g.mesh.face_areas[0];
    let u = 1.0;
    let phi: Vec<f64> = (0..nf).map(|f| if f < g.n_internal() { u * area } else { 0.0 }).collect();
    let step: Vec<f64> = (0..10).map(|i| if i < 5 { 1.0 } else { 0.0 }).collect();
    let psi = scalar_on(&g, &step);
    let ctl = LimiterControls::default();
    let dt = 0.5 * g.mesh.cell_volumes[0] / (u * area); // Courant 0.5
    let r = 1.0 / dt;
    let up = mules::interpolate(&g, AlphaConvection::Upwind, &phi, &psi);
    let mut phi_up: Vec<f64> = (0..nf).map(|f| phi[f] * up[f]).collect();
    let ones = vec![1.0; 10];
    mules::limit_return_corr(&g, &ctl, r, &psi, &step, &phi, &mut phi_up, &ones);
    assert!(phi_up.iter().all(|v| *v == 0.0), "{phi_up:?}");
    // a central flux: unlimited, the explicit update overshoots; limited (BD + correction) it stays in [0, 1]
    let lin = mules::interpolate(&g, AlphaConvection::Linear, &phi, &psi);
    let central: Vec<f64> = (0..nf).map(|f| phi[f] * lin[f]).collect();
    let mut x = step.clone();
    let zero = vec![0.0; 10];
    mules::explicit_update(&g, r, &mut x, &step, &central, &zero, &zero);
    assert!(x.iter().any(|v| *v > 1.0 + 1e-9 || *v < -1e-9), "the unlimited central step must violate the bounds");
    let mut corr = central.clone();
    mules::limit_return_corr(&g, &ctl, r, &psi, &step, &phi, &mut corr, &ones);
    let upf: Vec<f64> = (0..nf).map(|f| phi[f] * up[f]).collect();
    let limited: Vec<f64> = (0..nf).map(|f| upf[f] + corr[f]).collect();
    let mut y = step.clone();
    mules::explicit_update(&g, r, &mut y, &step, &limited, &zero, &zero);
    assert!(y.iter().all(|v| *v >= -1e-12 && *v <= 1.0 + 1e-12), "{y:?}");
}

/// The sub-cycle path of `solveAlphaMULES.H` (forced here: `adjustSubCycles false`, `nSubCycles 3`, because upstream's
/// own run first needs 2 sub-cycles at step 637, after its trajectory and the port's have separated): the count is
/// used, the fractions keep filling the moving volume, stay in [0, 1], and `alpha.oldTime()` is restored to the value
/// at the start of the step (upstream resets it to `alpha10` after the cycles).
#[cfg(feature = "std")]
#[test]
fn mules_sub_cycles_restore_old_time_and_keep_the_fractions_bounded() {
    let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/data/genfoam_1d_boiling");
    let mut case = TwoPhaseCase::read(&dir).expect("read vendored 1D_boiling");
    match &mut case.controls.alpha {
        super::AlphaSolver::Mules(mc) => {
            mc.adjust_sub_cycles = false;
            mc.n_sub_cycles = 3;
        }
        super::AlphaSolver::ImplicitUpwind => panic!("the shipped case selects MULES"),
    }
    let mut s = super::TwoPhaseSolver::new(case);
    for _ in 0..6 {
        let before: Vec<Vec<f64>> = (0..2).map(|k| s.fluids[k].alpha.c.clone()).collect();
        s.step().expect("step");
        assert_eq!(s.alpha_sub_cycles, 3);
        for k in 0..2 {
            assert_eq!(s.fluids[k].alpha_old, before[k], "alpha.oldTime() is the start-of-step value after the cycles");
        }
        for c in 0..s.geom.n_cells() {
            let sum = s.fluids[0].alpha.c[c] + s.fluids[1].alpha.c[c] + s.structure.alpha.c[c];
            assert!((sum - 1.0).abs() < 1e-12, "cell {c}: {sum}");
            for k in 0..2 {
                assert!((0.0..=1.0).contains(&s.fluids[k].alpha.c[c]));
            }
        }
    }
}

/// The boundary patch of `line(n)` that faces `+x` (the outlet) and the one that faces `-x` (the inlet).
fn end_patches(g: &Geometry) -> (usize, usize) {
    let mut inlet = usize::MAX;
    let mut outlet = usize::MAX;
    for (pi, p) in g.mesh.patches.iter().enumerate() {
        if p.kind != PatchKind::Empty {
            if g.mesh.face_centres[p.start].x < 0.5 {
                inlet = pi;
            } else {
                outlet = pi;
            }
        }
    }
    (inlet, outlet)
}

/// gh:#1801: `fixedGradient` (`fixedFluxPressure`) evaluates `c + g/deltaCoeffs`, assembles `+gamma |Sf| g` into the
/// `-laplacian` source and gives the boundary flux `-gamma |Sf| g` exactly.
#[test]
fn fixed_gradient_patch_evaluates_assembles_and_fluxes() {
    let g = line(4);
    let (inlet, _) = end_patches(&g);
    let mut bc = g.default_bcs();
    bc[inlet] = Bc::FixedGradient(vec![2.5]);
    let mut p = Scalar::uniform(&g, 10.0, bc);
    let f = g.mesh.patches[inlet].start;
    assert!((p.b[f - g.n_internal()] - (10.0 + 2.5 / g.delta[f])).abs() < 1e-12);
    p.correct_bc(&g);
    assert!(p.is_fixed_gradient(&g, f) && !p.is_fixed(&g, f));
    let gamma = vec![3.0; g.n_faces()];
    let mut e = Eqn::new(&g);
    e.neg_laplacian(&g, &gamma, &p);
    assert!((e.source[0] - 3.0 * g.mesh.face_areas[f] * 2.5).abs() < 1e-14);
    let flux = Eqn::laplacian_flux(&g, &gamma, &p);
    assert!((flux[f] + 3.0 * g.mesh.face_areas[f] * 2.5).abs() < 1e-14);
}

/// gh:#1801: `inletOutlet` takes `inletValue` where the NAMED flux is negative and the owner value otherwise, and its
/// convection coefficients follow (`valueFraction = neg(phi)`).
#[test]
fn inlet_outlet_patch_follows_the_sign_of_its_flux() {
    let g = line(4);
    let (inlet, _) = end_patches(&g);
    let mut bc = g.default_bcs();
    bc[inlet] = Bc::InletOutlet { inlet_value: vec![0.25], phi_index: 1 };
    let mut a = Scalar { c: vec![0.5; 4], b: vec![0.5; g.n_faces() - g.n_internal()], bc };
    let f = g.mesh.patches[inlet].start;
    let mut inflow = vec![0.0; g.n_faces()];
    inflow[f] = -1.0;
    let mut outflow = vec![0.0; g.n_faces()];
    outflow[f] = 1.0;
    a.correct_bc_with_flux(&g, [&[], &inflow]);
    assert_eq!(a.b[f - g.n_internal()], 0.25);
    assert_eq!(a.convection_coeffs(&g, f, [&[], &inflow]), (0.0, 0.25));
    a.correct_bc_with_flux(&g, [&[], &outflow]);
    assert_eq!(a.b[f - g.n_internal()], 0.5);
    assert_eq!(a.convection_coeffs(&g, f, [&[], &outflow]), (1.0, 0.0));
}

/// gh:#1801: `slip` removes the normal component, `fixedValue` imposes its value and sets a fixedValue flux patch,
/// `pressureInletOutletVelocity` is `zeroGradient` on outflow and keeps only the normal component on inflow; their matrix
/// coefficients follow `transformFvPatchField` (`1 - |n|`, `-deltaCoeffs |n|`).
#[test]
fn velocity_patches_evaluate_like_the_esi_classes() {
    use super::fv::{Vector, VectorBc};
    let g = line(4);
    let (inlet, outlet) = end_patches(&g);
    let mut u = Vector::uniform(&g, Vector3::new(2.0, 3.0, 0.0));
    u.bc[inlet] = VectorBc::Slip;
    u.bc[outlet] = VectorBc::PressureInletOutletVelocity { phi_index: 0 };
    let fi = g.mesh.patches[inlet].start - g.n_internal();
    let fo = g.mesh.patches[outlet].start - g.n_internal();
    let mut out = vec![0.0; g.n_faces()];
    out[g.mesh.patches[outlet].start] = 1.0;
    let mut inn = vec![0.0; g.n_faces()];
    inn[g.mesh.patches[outlet].start] = -1.0;
    u.correct_bc_with_flux(&g, [&out, &[]]);
    assert!((u.b[fi].x).abs() < 1e-14 && (u.b[fi].y - 3.0).abs() < 1e-14, "slip: {:?}", u.b[fi]);
    assert_eq!(u.b[fo], Vector3::new(2.0, 3.0, 0.0), "outflow is zeroGradient");
    u.correct_bc_with_flux(&g, [&inn, &[]]);
    assert!((u.b[fo].x - 2.0).abs() < 1e-14 && u.b[fo].y.abs() < 1e-14, "inflow keeps the normal part: {:?}", u.b[fo]);
    let c = u.patch_coeffs(&g, g.mesh.patches[inlet].start, [&out, &[]]);
    assert_eq!(c.value_internal, [0.0, 1.0, 1.0]);
    assert_eq!(c.gradient_internal_diag, [1.0, 0.0, 0.0]);
    assert!(u.flux_is_fixed(&g, g.mesh.patches[inlet].start) && !u.flux_is_fixed(&g, g.mesh.patches[outlet].start));
    let mut fx = Vector::uniform(&g, Vector3::new(1.0, 0.0, 0.0));
    fx.bc[inlet] = VectorBc::Fixed(vec![Vector3::new(4.0, 0.0, 0.0)]);
    fx.correct_bc(&g);
    assert_eq!(fx.b[fi].x, 4.0);
    assert!(fx.has_non_zero_gradient_patch());
    let cf = fx.patch_coeffs(&g, g.mesh.patches[inlet].start, [&[], &[]]);
    assert_eq!((cf.value_internal, cf.value_boundary[0]), ([0.0; 3], 4.0));
}
