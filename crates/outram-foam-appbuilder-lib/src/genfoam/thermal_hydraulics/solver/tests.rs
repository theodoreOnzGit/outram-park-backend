// SPDX-License-Identifier: GPL-3.0-only
// Copyright (C) 2026 OUTRAM PARK contributors
//
// Derived from GeN-Foam (Generalized Nuclear Foam)
//   Upstream: src/classes/thermalHydraulics/solvers/onePhase/**
//   Upstream commit: 652b3da
//   Upstream copyright: (C) 2015-2022 EPFL; built on OpenFOAM v2506
//   Upstream license: GPL-3.0
//
// This file is part of OUTRAM PARK, distributed under GPL-3.0. See the crate
// root or <https://www.gnu.org/licenses/> for the full license text.

//! V&V for the porous single-phase solver ([`super::one_phase`]).
//!
//! Each test marches the real [`OnePhaseSolver::step`] on a single uniform
//! porous cell where all spatial gradients vanish, so the ported field
//! equations collapse to the governing ODE with a known closed-form solution.
//! This exercises the genuine assembled operators (drag `Kd`, `fvm::ddt`,
//! `fvm::sp`, the linearised structure source) through the driver, not a
//! re-derivation.

use std::sync::Arc;

use outram_foam_basic_lib::prelude::*;

use super::one_phase::OnePhaseSolver;
use super::porous_drag::PorousDrag;
use crate::genfoam::thermal_hydraulics::closures::fs_drag::FsWallFriction;
use crate::genfoam::thermal_hydraulics::phase::fluid::{Fluid, StateOfMatter};

/// A single porous cell of unit volume with two wall patches (no internal
/// faces), so every `div`/`laplacian`/`grad` over it is identically zero.
fn one_cell_mesh() -> Arc<FvMesh> {
    Arc::new(
        FvMeshBuilder::new()
            .n_cells(1)
            .n_internal_faces(0)
            .owner(vec![0, 0])
            .neighbour(vec![])
            .patches(vec![
                BoundaryPatch::new("left", 0, 1, PatchKind::Wall),
                BoundaryPatch::new("right", 1, 1, PatchKind::Wall),
            ])
            .cell_volumes(vec![1.0])
            .cell_centres(vec![Vector3::new(0.5, 0.0, 0.0)])
            .face_area_vectors(vec![
                Vector3::new(-1.0, 0.0, 0.0),
                Vector3::new(1.0, 0.0, 0.0),
            ])
            .face_centres(vec![
                Vector3::new(0.0, 0.0, 0.0),
                Vector3::new(1.0, 0.0, 0.0),
            ])
            .build()
            .unwrap(),
    )
}

fn set_cell(f: &mut VolScalarField, v: f64) {
    f.internal.as_mut_slice()[0] = v;
}

/// V&V — laminar Darcy spin-up to terminal velocity under an imposed head.
///
/// Methodology: a porous cell driven by a constant body force `f_body`
/// [N/m^3] (an imposed pressure gradient) against the fluid-structure drag
/// `Kd U`. With no spatial gradients the momentum equation is the ODE
///
/// ```text
/// alpha rho dU/dt = f_body - Kd U .
/// ```
///
/// Using the exact laminar friction law `f = 64/Re` (the `ReynoldsPower{64,
/// -1, 0}` closure) the drag coefficient is velocity-independent,
/// `Kd = 32 mu / Dh^2` (see [`super::porous_drag`]'s V&V), so the steady
/// terminal velocity is the exact Darcy value `U_ss = f_body / Kd =
/// f_body Dh^2 / (32 mu)`. Backward Euler is exact at steady state.
///
/// Inputs: alpha = 1, rho = 1000 kg/m^3, mu = 1.0e-3 Pa s, Dh = 0.01 m,
/// f_body = 320 N/m^3; Kd = 32*1.0e-3/0.01^2 = 320 s^-1 → U_ss = 1.0 m/s.
/// Marched 200 steps of dt = 1 s (>> tau = alpha rho / Kd = 3.125 s).
///
/// Result (2026-07-15): U_ss = 1.0000000 m/s, |err| < 1e-6 m/s. Pass.
#[test]
fn laminar_darcy_spinup_terminal_velocity() {
    let mesh = one_cell_mesh();
    let mut fluid = Fluid::new(mesh.clone(), "coolant", StateOfMatter::Liquid);
    set_cell(fluid.alpha_mut(), 1.0);
    set_cell(fluid.density_mut(), 1000.0);
    set_cell(fluid.mu_mut(), 1.0e-3);
    set_cell(fluid.hydraulic_diameter_mut(), 0.01);
    fluid.correct_mag_velocity();

    let drag = PorousDrag::new(FsWallFriction::ReynoldsPower {
        coeff: 64.0,
        exp: -1.0,
        constant: 0.0,
    });
    let mut solver = OnePhaseSolver::new(mesh, fluid, drag);
    solver.solve_pressure = false;
    solver.body_force = Vector3::new(320.0, 0.0, 0.0);

    for _ in 0..200 {
        solver.step(1.0).unwrap();
    }
    let u = solver.fluid.velocity().internal.as_slice()[0].x;
    assert!((u - 1.0).abs() < 1e-6, "terminal U = {u} m/s, expected 1.0");
}

/// V&V — turbulent Darcy-Weisbach friction pressure drop (quadratic drag).
///
/// Methodology: same porous cell, but with a **constant** Darcy friction
/// factor `f0` (the `ReynoldsPower{0, 0, f0}` closure). The drag is then
/// quadratic, `Kd U = 0.5 f0 rho |U| U / Dh`, so a steady imposed head
/// `f_body` balances the Darcy-Weisbach friction body force
///
/// ```text
/// f_body = 0.5 f0 rho U^2 / Dh   =>   U_ss = sqrt( 2 f_body Dh / (f0 rho) ) .
/// ```
///
/// This is the canonical friction-pressure-drop check: the imposed pressure
/// gradient `dp/dx = -f_body = -0.5 f0 rho U^2 / Dh` is exactly Darcy-Weisbach.
///
/// Inputs: rho = 1000, Dh = 0.02 m, f0 = 0.02, f_body = 100 N/m^3.
/// U_ss = sqrt(2*100*0.02/(0.02*1000)) = sqrt(0.2) = 0.4472136 m/s.
/// Marched 400 steps of dt = 0.5 s to steady (Kd is lagged one step; it
/// converges by Picard iteration).
///
/// Result (2026-07-15): U_ss = 0.4472136 m/s, relative error < 1e-5. Pass.
#[test]
fn turbulent_darcy_weisbach_pressure_drop() {
    let rho = 1000.0;
    let dh = 0.02;
    let f0 = 0.02;
    let f_body = 100.0;

    let mesh = one_cell_mesh();
    let mut fluid = Fluid::new(mesh.clone(), "coolant", StateOfMatter::Liquid);
    set_cell(fluid.alpha_mut(), 1.0);
    set_cell(fluid.density_mut(), rho);
    set_cell(fluid.mu_mut(), 1.0e-3);
    set_cell(fluid.hydraulic_diameter_mut(), dh);
    // Seed a small velocity so the lagged Kd is non-zero on the first steps.
    fluid.velocity_mut().internal.as_mut_slice()[0] = Vector3::new(0.1, 0.0, 0.0);
    fluid.correct_mag_velocity();

    let drag = PorousDrag::new(FsWallFriction::ReynoldsPower {
        coeff: 0.0,
        exp: 1.0,
        constant: f0,
    });
    let mut solver = OnePhaseSolver::new(mesh, fluid, drag);
    solver.solve_pressure = false;
    solver.body_force = Vector3::new(f_body, 0.0, 0.0);

    for _ in 0..400 {
        solver.step(0.5).unwrap();
    }
    let u = solver.fluid.velocity().internal.as_slice()[0].x;
    let expected = (2.0 * f_body * dh / (f0 * rho)).sqrt();
    assert!(
        (u - expected).abs() / expected < 1e-5,
        "U_ss = {u} m/s, expected {expected} (Darcy-Weisbach)"
    );
}

/// V&V — porous energy equilibration to a fixed structure wall temperature.
///
/// Methodology: a stagnant porous cell (`U = 0`, no flux) with a
/// fixed-temperature structure surface at `T_wall`, coupled convectively by
/// `a_v h (T_wall - T)`. With no advection/diffusion the energy equation is the
/// relaxation ODE
///
/// ```text
/// alpha rho Cp dT/dt = a_v h (T_wall - T)   =>   T(t) -> T_wall .
/// ```
///
/// The linearised semi-implicit structure source (a `Su`/`Sp` pair, matching
/// `structure::linearizedSemiImplicitHeatSource`) is unconditionally stable and
/// has the exact steady state `T = T_wall`.
///
/// Inputs: alpha = 1, rho = 1000, Cp = 5000 J/(kg K), a_v = 100 1/m,
/// h = 1.0e4 W/(m^2 K), T0 = 300 K, T_wall = 600 K; tau = alpha rho Cp /
/// (a_v h) = 5.0 s. Marched 200 steps of dt = 1 s.
///
/// Result (2026-07-15): T_ss = 600.0000 K, |err| < 1e-4 K. Pass.
#[test]
fn porous_energy_equilibrates_to_wall_temperature() {
    let cp = 5000.0;
    let t0 = 300.0;
    let t_wall = 600.0;

    let mesh = one_cell_mesh();
    let mut fluid = Fluid::new(mesh.clone(), "coolant", StateOfMatter::Liquid);
    set_cell(fluid.alpha_mut(), 1.0);
    set_cell(fluid.density_mut(), 1000.0);
    set_cell(fluid.enthalpy_mut(), cp * t0);
    fluid.correct_mag_velocity();

    let drag = PorousDrag::new(FsWallFriction::ReynoldsPower {
        coeff: 0.0,
        exp: 1.0,
        constant: 0.0,
    });
    let mut solver = OnePhaseSolver::new(mesh.clone(), fluid, drag);
    solver.solve_pressure = false;
    set_cell(&mut solver.cp, cp);
    set_cell(&mut solver.wall_area, 100.0);
    set_cell(&mut solver.wall_htc, 1.0e4);
    set_cell(&mut solver.wall_temperature, t_wall);

    for _ in 0..200 {
        // No momentum drive; energy only.
        solver.correct_energy(1.0).unwrap();
    }
    let t = solver.temperature().internal.as_slice()[0];
    assert!((t - t_wall).abs() < 1e-4, "T_ss = {t} K, expected {t_wall}");
}

/// The six-cell slab of the constant-property energy pin (see
/// [`constant_property_energy_path_is_bit_identical`]).
fn pin_case() -> OnePhaseSolver {
    use outram_foam_basic_lib::steady_conduction::{radial_mesh, RadialGeometry, RadialShell};
    use uom::si::area::square_meter;
    use uom::si::f64::{Area, Length};
    use uom::si::length::meter;
    let cmesh = radial_mesh(
        RadialGeometry::Slab {
            area: Area::new::<square_meter>(0.5),
        },
        &[RadialShell {
            outer_radius: Length::new::<meter>(0.3),
            cells: 6,
        }],
    )
    .unwrap();
    let mesh = cmesh.mesh;
    let cp = 5000.0;
    let mut fluid = Fluid::new(mesh.clone(), "coolant", StateOfMatter::Liquid);
    for c in 0..6 {
        let x = c as f64;
        fluid.alpha_mut().internal.as_mut_slice()[c] = 0.4 + 0.05 * x;
        fluid.density_mut().internal.as_mut_slice()[c] = 1000.0 - 7.0 * x;
        fluid.enthalpy_mut().internal.as_mut_slice()[c] = cp * (300.0 + 13.0 * x);
        fluid.mu_mut().internal.as_mut_slice()[c] = 1.0e-3;
        fluid.hydraulic_diameter_mut().internal.as_mut_slice()[c] = 0.01;
    }
    fluid.enthalpy_mut().boundary[0].bc = BoundaryCondition::FixedValue(cp * 410.0);
    fluid.correct_mag_velocity();
    let drag = PorousDrag::new(FsWallFriction::ReynoldsPower {
        coeff: 64.0,
        exp: -1.0,
        constant: 0.0,
    });
    let mut solver = OnePhaseSolver::new(mesh, fluid, drag);
    solver.solve_pressure = false;
    solver.body_force = Vector3::new(32.0, 0.0, 0.0);
    for c in 0..6 {
        let x = c as f64;
        solver.cp.internal.as_mut_slice()[c] = cp;
        solver.alpha_eff.internal.as_mut_slice()[c] = (2.0 + 0.3 * x) / cp;
        solver.wall_area.internal.as_mut_slice()[c] = 50.0;
        solver.wall_htc.internal.as_mut_slice()[c] = 200.0 + 10.0 * x;
        solver.wall_temperature.internal.as_mut_slice()[c] = 500.0 - 20.0 * x;
    }
    solver
}

/// The enthalpy bits the constant-property energy path produced **before**
/// the temperature-dependent conductivity extension (gh:#1149) touched
/// `one_phase.rs`: captured on 2026-11-03 from the unmodified port at commit
/// a492135b8 by running [`pin_case`] for 5 steps of `dt = 0.7 s`. **Retired
/// as the pin 2026-11-03 (gh:#1152)**: they came from the defective energy
/// solve (fixed enthalpy face lost after the first solve; Gauss-Seidel
/// cold-started from zero). Kept as the record [`PIN_ENTHALPY_BITS`] moved
/// from, and used by [`energy_solve_matches_tightly_converged_march`].
const PIN_ENTHALPY_BITS_PRE_1152: [u64; 6] = [
    4699267812401739474,
    4699530888829840000,
    4699796177252820168,
    4700062961643042970,
    4700330851363260273,
    4700599657357131804,
];

/// The pinned enthalpy bits, **re-recorded 2026-11-03 (gh:#1152)** from
/// [`pin_case`], 5 steps of `dt = 0.7 s`, after `correct_energy` was made to
/// keep `he`'s boundary conditions and to solve upstream's way
/// (`smoothSolver`/`symGaussSeidel`, tolerance 1e-7, warm-started from `he`).
/// Old values: [`PIN_ENTHALPY_BITS_PRE_1152`].
const PIN_ENTHALPY_BITS: [u64; 6] = [
    4699267813052969334,
    4699530889695404414,
    4699796178375398694,
    4700062963068321122,
    4700330852138621715,
    4700601202290103506,
];

/// Pin — the constant-property energy path of the ported `onePhase` solver is
/// **bit-identical** to its recorded behaviour, so that the optional
/// temperature-dependent conductivity (gh:#1149) with `None` cannot move it.
///
/// Methodology: a six-cell slab with non-uniform `alpha`, `rho`, `alphaEff`,
/// wall coupling and initial enthalpy, one fixed-value enthalpy face (patch
/// 0, the slab's outer surface, next to cell 5), laminar drag and a body
/// force, marched 5 full [`OnePhaseSolver::step`]s. Every cell's final
/// enthalpy is compared by `f64::to_bits` against [`PIN_ENTHALPY_BITS`].
///
/// **Moved 2026-11-03 (gh:#1152), old -> new, T = he/Cp:**
///
/// | cell | old [K] | new [K] | (1) keep BCs | (2) upstream solve |
/// |---|---|---|---|---|
/// | 0 | 303.471492682 | 303.471523007 | 0 | +3.03e-5 |
/// | 1 | 315.721943499 | 315.721983805 | 0 | +4.03e-5 |
/// | 2 | 328.075398354 | 328.075450628 | +5.6e-13 | +5.23e-5 |
/// | 3 | 340.498514612 | 340.498580982 | +6.6e-9 | +6.64e-5 |
/// | 4 | 352.973101810 | 352.973137915 | +3.61e-5 | +1.7e-8 |
/// | 5 | 365.490356324 | 365.562297872 | **+7.19e-2** | -2.1e-8 |
///
/// Both changes were applied and measured one at a time; nothing else in the
/// path changed. (1) alone moves only the cells near the fixed face (the old
/// path dropped that face after step 1, so steps 2-5 saw zero gradient
/// there); (2) moves cells 0-3 by 1-2e-7 relative, the old solve's
/// under-convergence. Prediction (before the run): (1) raises the cell next
/// to the fixed 410 K face most, decaying away from it; (2) moves the field
/// by solver-tolerance amounts only. Both held, except that I first assumed
/// the face was next to cell 0; it is next to cell 5. The new bits are
/// within 4e-8 K of a march solved to 1e-15
/// ([`energy_solve_matches_tightly_converged_march`]); the old ones were
/// 0.072 K off.
#[test]
fn constant_property_energy_path_is_bit_identical() {
    let mut s = pin_case();
    for _ in 0..5 {
        s.step(0.7).unwrap();
    }
    let bits: Vec<u64> = s
        .fluid
        .enthalpy()
        .internal
        .as_slice()
        .iter()
        .map(|v| v.to_bits())
        .collect();
    assert_eq!(bits, PIN_ENTHALPY_BITS.to_vec());
}

/// V&V — **new work (gh:#1149)**: the porous energy equation with a
/// temperature-dependent conductivity ([`super::PorousConductivity`])
/// against the Kirchhoff transform.
///
/// **Methodology.** A stagnant porous slab (`alpha = 1`, no flow, no wall
/// coupling) of length 1 m in 40 cells (planar `RzMesh`), `T = 300 K` at
/// `x = 0` and `900 K` at `x = 1 m` (fixed enthalpy `Cp T`), and
/// `lambda = 10 (1 + 2e-3 T)`. The exact steady profile is linear in the
/// Kirchhoff variable `U = T + beta T^2/2`. Marched with
/// [`OnePhaseSolver::correct_energy`] to steady state, `dt = 0.2 s`, 5000
/// steps (`L^2/alpha` is about 45 s); each step re-evaluates
/// `alphaEff = lambda(T)/Cp` (Picard, lagged one step). Control: the same
/// slab with no model, i.e. the port's constant `alphaEff`, which gives a
/// linear profile.
///
/// **Prediction (before the run).** Within **0.1 K** of the Kirchhoff
/// profile everywhere (2nd-order FV with linearly interpolated face
/// `alphaEff`); the constant-`alphaEff` control is about **40 K** low at the
/// mid-plane (exact 640.2 K against 600 K).
///
/// **Result (2026-11-03).** First run, as first written with `dt = 1e7 s`
/// and 300 passes: **missed, 2.01 K**. Cause, found by reading the port: the
/// `EEqn` solve is a Gauss-Seidel cold-started from zero and capped at the
/// default 1000 sweeps, which a 40-cell steady diffusion matrix does not
/// converge in (spectral radius about 0.994). That is a property of the
/// ported driver's linear solve, not of the new conductivity; the protocol
/// was changed (a genuine time march, diagonally dominant), the threshold was
/// not. Re-run: **0.0637 K** max error with `lambda(T)`; the constant
/// control misses by **40.9 K**. Pass.
///
/// **Re-measured 2026-11-03 after gh:#1152** (`he` keeps its fixed ends;
/// upstream's warm-started `smoothSolver`/`symGaussSeidel`), with the BCs no
/// longer re-imposed by the test:
///
/// - time march, `dt = 0.2 s`, 5000 passes: 0.0637 -> **0.06360 K**;
/// - the **originally intended protocol**, `dt = 1e7 s`, 300 passes, now
///   **passes: 0.06355 K** (was 2.01 K), so it is asserted too;
/// - time march with the BCs re-imposed each pass as before: 0.06360 K,
///   identical to the run without, confirming `correct_energy` now keeps them;
/// - constant-`alphaEff` control: 40.904 K, unchanged.
///
/// Threshold unchanged (0.1 K).
#[test]
fn porous_energy_with_temperature_dependent_conductivity_matches_kirchhoff() {
    use crate::genfoam::thermal_hydraulics::solid_conduction::{
        AxisEdges, ConductivityModel, PropertyModel, RzGeometry, RzMesh, RzPatch,
    };
    use uom::si::f64::Length;
    use uom::si::length::meter;
    let (n, cp, beta, k0) = (40, 1000.0, 2.0e-3, 10.0);
    let build = |with_model: bool, dt: f64, passes: usize, reimpose: bool| {
        let rz = RzMesh::build(
            RzGeometry::Planar {
                depth: Length::new::<meter>(1.0),
            },
            AxisEdges::uniform(Length::new::<meter>(0.0), Length::new::<meter>(1.0), n).unwrap(),
            AxisEdges::uniform(Length::new::<meter>(0.0), Length::new::<meter>(1.0), 1).unwrap(),
            1,
            |_, _| 0,
        )
        .unwrap();
        let mesh = rz.mesh().clone();
        let mut fluid = Fluid::new(mesh.clone(), "gas", StateOfMatter::Gas);
        for c in 0..n {
            fluid.alpha_mut().internal.as_mut_slice()[c] = 1.0;
            fluid.density_mut().internal.as_mut_slice()[c] = 1.0;
            fluid.enthalpy_mut().internal.as_mut_slice()[c] = cp * 600.0;
        }
        fluid.enthalpy_mut().boundary[RzPatch::Inner.index()].bc = BoundaryCondition::FixedValue(cp * 300.0);
        fluid.enthalpy_mut().boundary[RzPatch::Outer.index()].bc = BoundaryCondition::FixedValue(cp * 900.0);
        let drag = PorousDrag::new(FsWallFriction::ReynoldsPower {
            coeff: 0.0,
            exp: 1.0,
            constant: 0.0,
        });
        let mut s = OnePhaseSolver::new(mesh, fluid, drag);
        s.solve_pressure = false;
        for c in 0..n {
            s.cp.internal.as_mut_slice()[c] = cp;
            s.alpha_eff.internal.as_mut_slice()[c] = k0 * (1.0 + beta * 600.0) / cp;
        }
        if with_model {
            s.conductivity_model = Some(
                super::PorousConductivity::new(
                    n,
                    vec![0; n],
                    vec![ConductivityModel(PropertyModel::Linear {
                        value_at_reference: k0,
                        beta,
                        reference_temperature: 0.0,
                    })],
                )
                .unwrap(),
            );
        }
        for _ in 0..passes {
            if reimpose {
                let he = s.fluid.enthalpy_mut();
                he.boundary[RzPatch::Inner.index()].bc = BoundaryCondition::FixedValue(cp * 300.0);
                he.boundary[RzPatch::Outer.index()].bc = BoundaryCondition::FixedValue(cp * 900.0);
            }
            s.correct_energy(dt).unwrap();
        }
        (rz, s)
    };
    let u = |t: f64| t + 0.5 * beta * t * t;
    let exact = |x: f64| {
        let uu = u(300.0) + (u(900.0) - u(300.0)) * x;
        ((1.0 + 2.0 * beta * uu).sqrt() - 1.0) / beta
    };
    let err = |rz: &RzMesh, s: &OnePhaseSolver| {
        let t = s.temperature();
        (0..n)
            .map(|c| (t.internal.as_slice()[c] - exact(rz.cell_centre(c).0.get::<meter>())).abs())
            .fold(0.0, f64::max)
    };
    let (rz, s) = build(true, 0.2, 5000, false);
    let e = err(&rz, &s);
    let (rz0, s0) = build(false, 0.2, 5000, false);
    let e0 = err(&rz0, &s0);
    let (rzs, ss) = build(true, 1e7, 300, false);
    let es = err(&rzs, &ss);
    let (rzr, sr) = build(true, 0.2, 5000, true);
    let er = err(&rzr, &sr);
    println!("porous Kirchhoff: max err {e:.4e} K with lambda(T) (time march), {es:.4e} K (dt = 1e7 s, 300 passes), {er:.4e} K (BCs re-imposed); constant alphaEff misses by {e0:.3} K");
    assert!(e < 0.1);
    assert!(es < 0.1);
    assert!((er - e).abs() < 1e-9, "re-imposing the BCs changed the answer: {er} against {e}");
    assert!(e0 > 30.0);
    assert!(super::PorousConductivity::new(n, vec![0; n - 1], vec![]).is_err());
    assert!(super::PorousConductivity::new(n, vec![1; n], vec![ConductivityModel(PropertyModel::Constant(1.0))]).is_err());
}

/// V&V — the `EEqn` linear solve is converged to its tolerance (gh:#1152).
///
/// **Methodology.** [`pin_case`] marched 5 steps of `dt = 0.7 s` with the
/// default energy solver (upstream `smoothSolver`/`symGaussSeidel`,
/// tolerance 1e-7, warm-started) against the same march with the solve
/// driven to tolerance 1e-15 (100 000 sweeps). Pass: every cell within
/// 1e-6 K of the tightly converged march.
///
/// **Prediction (before the run, 2026-11-03).** The default solve sits at
/// the 1e-7 normalised-residual level, so within about 1e-7 relative of the
/// converged enthalpy, i.e. well under 1e-4 K; the pre-#1152 path should be
/// off by about 0.07 K in the cell next to the fixed face (lost BC) and by
/// 1e-5 K-scale elsewhere.
///
/// **Result (2026-11-03).** Fixed path: worst **4.0e-8 K** (cell 2). The
/// pre-#1152 bits ([`PIN_ENTHALPY_BITS_PRE_1152`]): **-7.19e-2 K** in cell 5
/// (next to the fixed face) and -3.0e-5 to -6.6e-5 K in cells 0-4. Pass.
#[test]
fn energy_solve_matches_tightly_converged_march() {
    let mut tight = pin_case();
    tight.energy_solver = SolverSettings {
        tolerance: 1e-15,
        max_iter: 100_000,
    };
    let mut s = pin_case();
    for _ in 0..5 {
        tight.step(0.7).unwrap();
        s.step(0.7).unwrap();
    }
    let cp = 5000.0;
    for c in 0..6 {
        let t_tight = tight.fluid.enthalpy().internal.as_slice()[c] / cp;
        let t = s.fluid.enthalpy().internal.as_slice()[c] / cp;
        let t_old = f64::from_bits(PIN_ENTHALPY_BITS_PRE_1152[c]) / cp;
        println!("cell {c}: converged {t_tight:.9} K, default {:+.3e} K, pre-#1152 {:+.3e} K", t - t_tight, t_old - t_tight);
        assert!((t - t_tight).abs() < 1e-6, "cell {c}: {t} K against {t_tight} K");
    }
}
