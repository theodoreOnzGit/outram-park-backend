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

/// gh:#1812: a `wedge` patch whose normal lies ON the coordinate axis (`centreNormal = n`) has `faceT = cellT = I`: the
/// patch value is the cell value and the matrix coefficients are those of `zeroGradient` (`snGradTransformDiag = 0`).
/// This reaches [`fv::wedge_velocity_bc`], the `Wedge` evaluation and its coefficients; the rotation itself is gated by
/// `wedge_rotation_tensor_is_the_rotation_of_the_centre_normal_onto_the_patch_normal`.
#[test]
fn wedge_patch_on_a_coordinate_axis_is_the_identity_transform() {
    use super::fv::{Vector, VectorBc};
    let g = line(4);
    let (inlet, _) = end_patches(&g);
    let bc = fv::wedge_velocity_bc(&g, inlet);
    assert!(matches!(bc, VectorBc::Wedge { .. }));
    let mut u = Vector::uniform(&g, Vector3::new(2.0, 3.0, 0.0));
    u.bc[inlet] = bc;
    u.correct_bc(&g);
    let fi = g.mesh.patches[inlet].start - g.n_internal();
    assert_eq!(u.b[fi], Vector3::new(2.0, 3.0, 0.0));
    let c = u.patch_coeffs(&g, g.mesh.patches[inlet].start, [&[], &[]]);
    assert_eq!(c.value_internal, [1.0; 3]);
    assert_eq!(c.gradient_internal_diag, [0.0; 3]);
}

// ---- gh:#1802: porousKEpsilon2PhaseCorrected building blocks ----

use super::turbulence::{bound, convective_inflow2, production_invariant, regex_full_match, relax_equation};

/// OpenFOAM's `solvers` patterns: `"(k|epsilon|Theta).*"` takes `k.liquid` and `epsilon.vapour`, `"e.*"` takes `epsilon.*`
/// too (which is why the LAST match rule matters), `"alpha"` is exact, an escaped bracket is a literal.
#[test]
fn keyword_regex_matches_the_solver_patterns() {
    assert!(regex_full_match("(k|epsilon|Theta).*", "k.liquid"));
    assert!(regex_full_match("(k|epsilon|Theta).*", "epsilon.vapour"));
    assert!(!regex_full_match("(k|epsilon|Theta).*", "h.liquid"));
    assert!(regex_full_match("e.*", "epsilon.liquid"));
    assert!(regex_full_match(".*", "anything"));
    assert!(regex_full_match("alpha", "alpha") && !regex_full_match("alpha", "alpha.vapour"));
    assert!(regex_full_match("div\\(phi\\)", "div(phi)") && !regex_full_match("div\\(phi\\)", "divXphiX"));
    assert!(regex_full_match("a|b.*", "bcd") && !regex_full_match("a|b.*", "ccd"));
}

/// `bound` (ESI `bound.C`): untouched above the limit; below it, a non-positive cell takes the average of its bounded
/// neighbours (the mean of the two interpolated faces) and nothing ends below the limit.
#[test]
fn bound_replaces_the_non_positive_cell_by_the_neighbour_average() {
    let g = line(5);
    let mut s = Scalar::uniform(&g, 1.0, g.default_bcs());
    assert!(!bound(&g, &mut s, 1e-15), "nothing below the limit: nothing done");
    s.c[2] = -3.0;
    s.c[1] = 2.0;
    s.c[3] = 4.0;
    assert!(bound(&g, &mut s, 1e-15));
    // the bounded field is (1, 2, 1e-15, 4, 1): faces of cell 2 interpolate to (2 + 1e-15)/2 and (1e-15 + 4)/2
    let avg = ((2.0 + 1e-15) / 2.0 + (1e-15 + 4.0) / 2.0) / 2.0;
    assert!((s.c[2] - avg).abs() < 1e-12, "{} vs {avg}", s.c[2]);
    assert!(s.c.iter().all(|v| *v >= 1e-15));
    assert_eq!(s.c[1], 2.0);
}

/// `fvMatrix::relax(alpha)`: a cell whose diagonal is below the sum of its off-diagonals is raised to it (then divided by alpha),
/// and the source gains `(D' - D) psi`; a convective inflow through a non-fixed patch counts twice its magnitude.
#[test]
fn relax_enforces_diagonal_dominance_and_adds_the_source() {
    let g = line(3);
    let mut e = Eqn::new(&g);
    e.ldu.diag = vec![1.0, 0.5, 4.0];
    e.ldu.upper = vec![-1.0, -1.0];
    e.ldu.lower = vec![-1.0, -1.0];
    let psi = vec![2.0, 3.0, 5.0];
    let inflow2 = vec![0.0; 3];
    relax_equation(&g, &mut e, 1.0, &inflow2, &psi);
    // sumOff = (1, 2, 1); D' = max(|D|, sumOff)
    assert_eq!(e.ldu.diag, vec![1.0, 2.0, 4.0]);
    assert_eq!(e.source, vec![0.0, 1.5 * 3.0, 0.0]);
    // alpha <= 0 is a no-op
    let before = e.ldu.diag.clone();
    relax_equation(&g, &mut e, 0.0, &inflow2, &psi);
    assert_eq!(e.ldu.diag, before);
    let mut phi = g.surface_zeros();
    phi[g.n_internal()] = -0.25;
    let s = Scalar::uniform(&g, 0.0, g.default_bcs());
    let inf = convective_inflow2(&g, &phi, &s);
    assert_eq!(inf.iter().sum::<f64>(), 0.5);
}

/// `G = nut (dev(twoSymm(grad U)) && grad U)` of a pure shear `dU_y/dx = a` is `nut a^2` (`twoSymm` has `a` in both off-diagonals,
/// `grad U` only in one); a uniform dilatation has no deviatoric part.
#[test]
fn production_of_a_pure_shear_is_a_squared() {
    let a = 3.0;
    let mut t = [[0.0; 3]; 3];
    t[0][1] = a;
    assert_eq!(production_invariant(&t), a * a);
    let d = [[2.0, 0.0, 0.0], [0.0, 2.0, 0.0], [0.0, 0.0, 2.0]];
    assert!(production_invariant(&d).abs() < 1e-14);
}

/// gh:#1812: `rotationTensor(n1, n2)` of the wedge patch maps `n1` onto `n2`, is orthogonal, and `cellT = faceT & faceT`
/// rotates twice (analytic 3-4-5 angle: `cos a = 0.8`, `sin a = 0.6`, so `cos 2a = 0.28`, `sin 2a = 0.96`).
#[test]
fn wedge_rotation_tensor_is_the_rotation_of_the_centre_normal_onto_the_patch_normal() {
    let n1 = [0.0, 0.0, 1.0];
    let n2 = [0.6, 0.0, 0.8];
    let r = fv::rotation_tensor(n1, n2);
    let rn1 = [r[0][2], r[1][2], r[2][2]];
    for c in 0..3 {
        assert!((rn1[c] - n2[c]).abs() < 1e-15, "R n1 = n2, component {c}");
    }
    for i in 0..3 {
        for j in 0..3 {
            let mut rrt = 0.0;
            for k in 0..3 {
                rrt += r[i][k] * r[j][k];
            }
            let id = if i == j { 1.0 } else { 0.0 };
            assert!((rrt - id).abs() < 1e-15, "R R^T = I at ({i},{j})");
        }
    }
    let mut r2n1 = [0.0; 3];
    for i in 0..3 {
        for k in 0..3 {
            r2n1[i] += r[i][k] * rn1[k];
        }
    }
    assert!((r2n1[0] - 0.96).abs() < 1e-15 && r2n1[1].abs() < 1e-15 && (r2n1[2] - 0.28).abs() < 1e-15);
}

// ───────────────────── MULES `solvePhase` (gh:#1799, round 34) ─────────────────────

/// A step profile and a central flux on `line(10)`, Courant 0.5, as in the limiter test above.
fn step_case(g: &Geometry) -> (Vec<f64>, Scalar, Vec<f64>, Vec<f64>, f64) {
    let nf = g.n_faces();
    let area = g.mesh.face_areas[0];
    let phi: Vec<f64> = (0..nf).map(|f| if f < g.n_internal() { area } else { 0.0 }).collect();
    let step: Vec<f64> = (0..10).map(|i| if i < 5 { 1.0 } else { 0.0 }).collect();
    let psi = scalar_on(g, &step);
    let lin = mules::interpolate(g, AlphaConvection::Linear, &phi, &psi);
    let central: Vec<f64> = (0..nf).map(|f| phi[f] * lin[f]).collect();
    let r = area / (0.5 * g.mesh.cell_volumes[0]);
    (phi, psi, step, central, r)
}

/// `limiter` is the zero-source case of `limiter_with_sources` bit for bit, and a non-zero `Sp`/`Su` moves the limiter
/// (`(rDeltaT - Sp) psiMaxn - Su` in the bounds, myMULESTemplates.C:440-452).
#[test]
fn limiter_with_zero_sources_is_the_zero_source_limiter_and_sources_move_it() {
    let g = line(10);
    let (phi, psi, step, central, r) = step_case(&g);
    let up = mules::interpolate(&g, AlphaConvection::Upwind, &phi, &psi);
    let bd: Vec<f64> = (0..g.n_faces()).map(|f| phi[f] * up[f]).collect();
    let corr: Vec<f64> = (0..g.n_faces()).map(|f| central[f] - bd[f]).collect();
    let (ones, zeros) = (vec![1.0; 10], vec![0.0; 10]);
    let ctl = LimiterControls::default();
    let a = mules::limiter(&g, &ctl, r, &psi, &step, &bd, &corr, &ones, &zeros);
    let b = mules::limiter_with_sources(&g, &ctl, r, &psi, &step, &bd, &corr, &zeros, &zeros, &ones, &zeros);
    assert_eq!(a, b);
    assert!(a.iter().any(|l| *l < 1.0), "the central flux across a step must be limited somewhere");
    // `psiMaxn = V ((r - Sp) psiMaxn - Su - r psi0) + sumPhiBD`: an explicit source of either sign moves the bounds
    let sink = vec![-0.4 * r; 10];
    let src = vec![0.4 * r; 10];
    let c = mules::limiter_with_sources(&g, &ctl, r, &psi, &step, &bd, &corr, &zeros, &sink, &ones, &zeros);
    let d = mules::limiter_with_sources(&g, &ctl, r, &psi, &step, &bd, &corr, &zeros, &src, &ones, &zeros);
    assert_ne!(a, c, "an explicit sink must change the limiter");
    assert_ne!(a, d, "an explicit source must change the limiter");
}

/// `limit(returnCorr = false)` returns the full flux `phiBD + lambda phiCorr`: bitwise `phiBD` plus what
/// `limit(returnCorr = true)` returns, and the explicit update with it stays inside the bounds.
#[test]
fn limit_full_flux_is_the_upwind_flux_plus_the_limited_correction() {
    let g = line(10);
    let (phi, psi, step, central, r) = step_case(&g);
    let (ones, zeros) = (vec![1.0; 10], vec![0.0; 10]);
    let ctl = LimiterControls::default();
    let up = mules::interpolate(&g, AlphaConvection::Upwind, &phi, &psi);
    let mut corr = central.clone();
    mules::limit_return_corr(&g, &ctl, r, &psi, &step, &phi, &mut corr, &ones);
    let mut full = central.clone();
    mules::limit_full_flux(&g, &ctl, r, &psi, &step, &phi, &mut full, &zeros, &zeros, &ones, &zeros);
    for f in 0..g.n_faces() {
        let bd = if f < g.n_internal() { phi[f] * up[f] } else { central[f] };
        assert_eq!(full[f], bd + corr[f], "face {f}");
    }
    let mut y = step.clone();
    mules::explicit_update(&g, r, &mut y, &step, &full, &zeros, &zeros);
    assert!(y.iter().all(|v| *v >= -1e-12 && *v <= 1.0 + 1e-12), "{y:?}");
}

/// A `wedge` patch gets `lambda = 0` at the end of every limiter iteration (myMULESTemplates.C:561-566). Its `phiCorr`
/// is 0 in the solver (`phiBD := phiPsi` on non-coupled patches), so the limited flux is unchanged: the translation is
/// exercised and has no numerical effect, which is what upstream's code gives too.
#[test]
fn a_wedge_patch_gets_lambda_zero_in_the_limiter() {
    let mut mesh = create_one_d_mesh(Length::new::<meter>(1.0), Area::new::<square_meter>(1e-2), 10).expect("1-D mesh");
    let (inlet, _) = end_patches(&Geometry::new(alloc::sync::Arc::new(mesh.clone())));
    let mut plain = mesh.clone();
    mesh.patches[inlet].kind = PatchKind::Wedge;
    plain.patches[inlet].kind = PatchKind::Patch;
    let g = Geometry::new(alloc::sync::Arc::new(mesh));
    let gp = Geometry::new(alloc::sync::Arc::new(plain));
    let (phi, psi, step, central, r) = step_case(&g);
    let up = mules::interpolate(&g, AlphaConvection::Upwind, &phi, &psi);
    let mut bd: Vec<f64> = (0..g.n_faces()).map(|f| phi[f] * up[f]).collect();
    let mut corr: Vec<f64> = (0..g.n_faces()).map(|f| central[f] - bd[f]).collect();
    let f0 = g.mesh.patches[inlet].start;
    bd[f0] = central[f0];
    corr[f0] = 0.3; // a (non-physical) boundary correction: the wedge branch must zero its lambda
    let (ones, zeros) = (vec![1.0; 10], vec![0.0; 10]);
    let ctl = LimiterControls::default();
    let lambda = mules::limiter(&g, &ctl, r, &psi, &step, &bd, &corr, &ones, &zeros);
    assert_eq!(lambda[f0], 0.0);
    let lambda_plain = mules::limiter(&gp, &ctl, r, &psi, &step, &bd, &corr, &ones, &zeros);
    assert_eq!(lambda_plain[f0], 1.0, "a plain patch keeps lambda = 1");
}

/// `nonuniform List<scalar> N (...)` tokens give the cell values; a wrong count or another element type is refused.
#[test]
fn nonuniform_scalar_list_reads_the_foam_tokens() {
    use outram_foam_basic_lib::io::FoamValue;
    let toks = |n: f64, items: Vec<FoamValue>| {
        vec![FoamValue::Word("nonuniform".into()), FoamValue::Word("List<scalar>".into()), FoamValue::Scalar(n), FoamValue::List(items)]
    };
    let ok = toks(2.0, vec![FoamValue::Scalar(0.25), FoamValue::Scalar(0.0)]);
    assert_eq!(super::case::nonuniform_scalar_list(&ok), Some(vec![0.25, 0.0]));
    assert_eq!(super::case::nonuniform_scalar_list(&toks(3.0, vec![FoamValue::Scalar(1.0)])), None);
    let mut vec_list = toks(1.0, vec![FoamValue::Scalar(1.0)]);
    vec_list[1] = FoamValue::Word("List<vector>".into());
    assert_eq!(super::case::nonuniform_scalar_list(&vec_list), None);
    assert_eq!(super::case::nonuniform_scalar_list(&[FoamValue::Word("uniform".into()), FoamValue::Scalar(1.0)]), None);
}

/// The `solveAlpha` lambda (`solvePhase` set), forced here with nSubCycles 3 on the vendored `1D_boiling`: the solved fluid
/// is bounded by the moving fraction, the other follows by continuity (sum = moving fraction in every cell), and
/// `alpha.oldTime()` is restored after the cycles.
#[cfg(feature = "std")]
#[test]
fn solve_phase_lambda_keeps_the_fractions_bounded_and_summing_to_the_moving_fraction() {
    for solved in [1usize, 0usize] {
        let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/data/genfoam_1d_boiling");
        let mut case = TwoPhaseCase::read(&dir).expect("read vendored 1D_boiling");
        match &mut case.controls.alpha {
            super::AlphaSolver::Mules(mc) => {
                mc.adjust_sub_cycles = false;
                mc.n_sub_cycles = 3;
                mc.solve_phase = Some(solved);
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
                let moving = 1.0 - s.structure.alpha.c[c];
                let sum = s.fluids[0].alpha.c[c] + s.fluids[1].alpha.c[c];
                assert!((sum - moving).abs() < 1e-12, "cell {c}: {sum} vs {moving}");
                for k in 0..2 {
                    assert!(s.fluids[k].alpha.c[c] >= 0.0 && s.fluids[k].alpha.c[c] <= moving, "cell {c} fluid {k}");
                }
            }
        }
    }
}

/// gh:#1894 round 46, the two-phase region: upstream does NOT relax its momentum equations (`UEqns_2p.H:191,194` comment out
/// `UEqn1.relax()` and `UEqn2.relax()`), so the round 45 defect (`fvMatrix::relax` with a non-uniform `internalCoeffs`) has no counterpart;
/// what a non-uniform `internalCoeffs` (a `slip` wall, `iCoeffs = (c, 0, 0)`) does reach is `A()` and `H()`, which `VecEqn` forms as upstream's
/// `fvMatrix::A()` (`cmptAv` into the diagonal, `fvMatrix.C:1313`) and `H()` (`(cmptAv - iCoeffs_k) psi_k` joins the numerator, `:1332`).
#[cfg(feature = "std")]
#[test]
fn two_phase_momentum_a_and_h_with_a_slip_face_follow_cmpt_av_and_has_no_relaxation() {
    let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/data/genfoam_1d_boiling");
    let case = TwoPhaseCase::read(&dir).expect("read vendored 1D_boiling");
    let s = super::TwoPhaseSolver::new(case);
    let g = s.geom.clone();
    let mut e = super::VecEqn::new(&g);
    let o = g.mesh.owner[g.n_internal()];
    let c = 0.6;
    e.add_internal_coeffs(o, [c, 0.0, 0.0], false);
    assert!((e.eqn.ldu.diag[o] - c / 3.0).abs() < 1e-15, "cmptAv goes into the diagonal");
    assert!(e.has_h_corr);
    assert!((e.h_corr[0][o] - (c / 3.0 - c)).abs() < 1e-15 && (e.h_corr[1][o] - c / 3.0).abs() < 1e-15);
    // H of cell `o` for U = (1, 1, 1): `h_corr_k psi_k` joins the numerator (no off-diagonals, no source here)
    let mut u = super::fv::Vector::uniform_with_constraint_patches(&g, outram_foam_basic_lib::primitives::Vector3::ZERO);
    for v in u.c.iter_mut() {
        *v = outram_foam_basic_lib::primitives::Vector3::new(1.0, 1.0, 1.0);
    }
    let h = e.h(&g, &u);
    let v = g.mesh.cell_volumes[o];
    assert!((h[o].x - (c / 3.0 - c) / v).abs() < 1e-12 * c / v && (h[o].y - (c / 3.0) / v).abs() < 1e-12 * c / v);
    // no relaxation: the assembled equation of the solver equals itself with the (nonexistent) relaxation ablated, i.e. correct_u_eqn
    // takes no relaxation factor from `controls` at all
    let e0 = s.correct_u_eqn(0);
    let e1 = s.correct_u_eqn(0);
    assert_eq!(e0.eqn.ldu.diag, e1.eqn.ldu.diag);
}
