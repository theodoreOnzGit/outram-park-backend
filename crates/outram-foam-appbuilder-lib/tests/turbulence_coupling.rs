// SPDX-License-Identifier: GPL-3.0-only
// Copyright (C) 2026 OUTRAM PARK contributors
// Derived from OpenFOAM (www.openfoam.com)
// Copyright (C) 2004-2023 OpenFOAM Foundation
// Copyright (C) 2016-2023 OpenCFD Ltd.
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

//! # V&V — turbulence closure driven from inside the PIMPLE time loop
//!
//! These tests exercise the Layer-4 ↔ Layer-5 coupling wired in
//! [`outram_foam_appbuilder_lib::turbulence`]: a `TurbulenceClosure` attached to
//! `PimpleFoam` must be synchronised with the live velocity/flux/viscosity,
//! corrected once per time step after the pressure correctors, and must feed
//! ν_t back into the momentum equation.
//!
//! ## What is verified here, and what is not
//!
//! **Verified.** That a closure advanced inside the real solver loop reproduces
//! the *analytic solution of its own transport equations* for the decay of
//! homogeneous turbulence — a method-of-exact-solution verification with no free
//! parameters — and that switching a closure on changes the momentum solution in
//! the way the effective viscosity dictates.
//!
//! **NOT verified, anywhere in this stack.** Agreement with a published
//! turbulence benchmark or a wall-friction correlation. The closures in
//! `outram-foam-turbulence-lib` use zero-gradient near-wall boundary conditions,
//! **not wall functions** — that crate says so itself, and ships
//! `wall_functions::{y_plus, u_tau, nu_t_wall}` as unwired standalone helpers.
//! A law-of-the-wall or Blasius/Moody friction-factor comparison is therefore
//! **blocked on wall functions**, not merely unwritten. The second test below
//! measures how bad this is in practice: a Re = 100 cavity run with Wilcox k-ω
//! develops ν_t/ν ≈ 260–330, which is physically absurd and is exactly the
//! symptom of a missing ω wall treatment. Do not describe any wall-bounded RAS
//! result from this stack as validated.

use outram_foam_appbuilder_lib::io::control_dict::{ControlDict, StartControl, StopControl};
use outram_foam_appbuilder_lib::io::fv_schemes::FvSchemes;
use outram_foam_appbuilder_lib::io::fv_solution::FvSolution;
use outram_foam_appbuilder_lib::io::poly_mesh::read_poly_mesh;
use outram_foam_appbuilder_lib::solvers::pimple_foam::PimpleFoam;
use outram_foam_appbuilder_lib::turbulence::TurbulenceClosure;
use outram_foam_basic_lib::prelude::{FvMesh, VolScalarField};
use std::path::Path;
use std::sync::Arc;

// Wilcox k-ω closure coefficients, written out here rather than imported so the
// reference solution below is an independent statement of the model, not a
// restatement of the code under test.
const BETA: f64 = 0.072; // ω destruction coefficient β  (dimensionless)
const BETA_STAR: f64 = 0.09; // k destruction coefficient β* (dimensionless)

/// The 20×20×1 lid-driven-cavity polyMesh shipped with the pimpleFoam tutorial
/// (400 cells, 0.1 m × 0.1 m × 0.01 m). Used here only as a real, non-trivial
/// mesh — each case below imposes its own initial and boundary state.
fn cavity_mesh() -> Arc<FvMesh> {
    let case = Path::new(env!("CARGO_MANIFEST_DIR")).join("tutorials/cases/pimple_foam_cavity");
    read_poly_mesh(&case.join("constant").join("polyMesh")).expect("read cavity polyMesh")
}

/// Advance the quiescent decay case `n_steps` × `dt` and return
/// `(k, ω)` in cell 0 [m²/s², 1/s].
fn run_decay(dt: f64, n_steps: usize, k0: f64, w0: f64) -> (f64, f64) {
    let mesh = cavity_mesh();
    let control = ControlDict {
        start: StartControl::StartTime(0.0),
        stop: StopControl::EndTime(dt * n_steps as f64),
        delta_t: dt,
        ..ControlDict::default()
    };
    let mut solution = FvSolution::default();
    solution.pimple.n_outer_correctors = 1;
    solution.pimple.n_correctors = 2;

    // `PimpleFoam::new` starts from U = 0, p = 0, φ = 0 with zero-gradient
    // boundaries: quiescent fluid, so the strain rate is identically zero and
    // the turbulence production G = ν_t |S|² vanishes. Pure decay.
    let mut solver = PimpleFoam::new(mesh.clone(), control, FvSchemes::default(), solution);
    solver.nu = VolScalarField::uniform("nu", mesh.clone(), 1.0e-5);
    solver.turbulence = TurbulenceClosure::k_omega(mesh.clone());
    solver.turbulence.set_k_omega_uniform(k0, w0);

    for _ in 0..n_steps {
        solver.step().expect("pimpleFoam step");
    }

    match &solver.turbulence {
        TurbulenceClosure::KOmega(m) => (m.k.internal[0], m.omega.internal[0]),
        _ => unreachable!("closure was set to k-omega"),
    }
}

/// V&V (verification) — decay of homogeneous turbulence driven from inside the
/// PIMPLE loop, against the analytic solution of the Wilcox k-ω equations.
///
/// # Methodology
///
/// **What is computed.** A `PimpleFoam` run on the 400-cell cavity mesh with
/// quiescent fluid (U ≡ 0, φ ≡ 0, p ≡ 0, ν = 1e-5 m²/s) and a Wilcox k-ω closure
/// attached through [`TurbulenceClosure`]. With zero strain the production term
/// G = ν_t |S|² vanishes and the model's transport equations reduce to the
/// homogeneous-decay ODE pair
///
/// ```text
///   dω/dt = −β ω²          β  = 0.072
///   dk/dt = −β* k ω        β* = 0.09
/// ```
///
/// **Reference (analytical, exact — no benchmark data, no tuning).**
///
/// ```text
///   ω(t) = ω0 / (1 + β ω0 t)
///   k(t) = k0 (1 + β ω0 t)^(−β*/β)     with β*/β = 0.09/0.072 = 1.25
/// ```
///
/// This is the classical k-ω decay law; the exponent 1.25 is the value Wilcox's
/// coefficients were calibrated to give for decaying isotropic grid turbulence.
///
/// The **ω discretisation is algebraically exact**: the model assembles
/// `fvm::ddt(ω) + Sp(β ω_old, ω)`, i.e. `ω_{n+1} = ω_n/(1 + β ω_n Δt)`, which
/// rearranges to `1/ω_{n+1} = 1/ω_n + βΔt` — the same arithmetic progression in
/// 1/ω that the analytic solution has, for *any* Δt. Any residual error in ω is
/// therefore linear-solver noise, not truncation. The **k equation is first
/// order** in Δt because it uses ω at the new time level, so its error must
/// halve when Δt halves.
///
/// **Inputs.** k0 = 1.0e-2 m²/s², ω0 = 100 s⁻¹ (so ν_t0 = k0/ω0 = 1e-4 m²/s),
/// t_end = 0.5 s. Growth factor 1 + βω0t = 4.6, giving analytic targets
/// ω(0.5 s) = 21.739130434783 s⁻¹ and k(0.5 s) = 1.484406032e-3 m²/s².
/// Two grids in time: Δt = 1e-3 s (500 steps) and Δt = 5e-4 s (1000 steps).
///
/// **Pass criteria.** (a) relative error in ω < 1e-4 at both Δt — a
/// solver-noise floor, not a convergence claim; (b) relative error in k < 1 % at
/// Δt = 1e-3 s; (c) observed order of convergence in k, log2(err_coarse /
/// err_fine), in [0.9, 1.1] — i.e. the first order the discretisation predicts,
/// rather than a coincidental single-point match.
///
/// # Results (run 2026-08-07, release mode, this machine)
///
/// Full Δt sweep, all through `PimpleFoam::step`:
///
/// | Δt [s] | steps | ω(0.5 s) [1/s] | rel. err. ω | k(0.5 s) [m²/s²] | rel. err. k |
/// |---|---|---|---|---|---|
/// | 1.0e-1 | 5 | 21.739123707826 | 3.09e-7 | 2.534139e-3 | 7.072e-1 |
/// | 5.0e-2 | 10 | 21.739122541760 | 3.63e-7 | 2.039734e-3 | 3.741e-1 |
/// | 1.0e-2 | 50 | 21.739080164881 | 2.31e-6 | 1.600680e-3 | 7.833e-2 |
/// | 5.0e-3 | 100 | 21.739057676489 | 3.35e-6 | 1.542875e-3 | 3.939e-2 |
/// | 2.5e-3 | 200 | 21.739107286172 | 1.06e-6 | 1.513726e-3 | 1.975e-2 |
/// | 1.0e-3 | 500 | 21.738747892342 | 1.76e-5 | 1.496134e-3 | 7.901e-3 |
/// | 5.0e-4 | 1000 | 21.739034629220 | 4.41e-6 | 1.490279e-3 | 3.956e-3 |
///
/// Analytic: ω = 21.739130434783 s⁻¹, k = 1.484406032e-3 m²/s².
///
/// **Interpretation.**
///
/// - **ω** matches the analytic solution to between 3e-7 and 2e-5 relative, and
///   the error is *non-monotonic in Δt over a 200× range* (3.1e-7 at Δt = 0.1 s,
///   1.8e-5 at Δt = 1e-3 s). It therefore does not scale as any power of Δt,
///   confirming it is the iterative floor of the model's internal Gauss-Seidel
///   solve (`SolverSettings::default()`, tolerance 1e-7 on the normalised
///   residual) rather than discretisation error — as the exactness argument
///   above predicts.
/// - **k** converges cleanly at first order across three decades of Δt: the
///   error ratios are 4.78× for a 5× refinement (1e-2 ← 5e-2), then 1.99×,
///   1.99×, 2.50× (for a 2.5× refinement) and 2.00× — observed order 1.00 at the
///   two grids asserted here, against the theoretical 1.
///
/// This verifies the k/ω source and sink assembly and the k↔ω coupling over the
/// **full Layer-4/Layer-5 path** — `PimpleFoam::step` → `TurbulenceClosure::
/// sync_inputs` → `TurbulenceClosure::correct` — including that `correct()` is
/// invoked exactly once per solver step with the solver's own Δt. It is
/// **verification of the model's own equations**, not validation against
/// measured grid-turbulence data.
#[test]
fn homogeneous_decay_matches_analytic_k_omega_solution() {
    let k0 = 1.0e-2_f64; // m²/s²
    let w0 = 100.0_f64; // 1/s
    let t_end = 0.5_f64; // s

    let growth = 1.0 + BETA * w0 * t_end; // 1 + β ω0 t = 4.6
    let w_exact = w0 / growth;
    let k_exact = k0 * growth.powf(-BETA_STAR / BETA);

    let dt_coarse = 1.0e-3;
    let (k_c, w_c) = run_decay(dt_coarse, (t_end / dt_coarse).round() as usize, k0, w0);
    let dt_fine = 5.0e-4;
    let (k_f, w_f) = run_decay(dt_fine, (t_end / dt_fine).round() as usize, k0, w0);

    let err_w_c = ((w_c - w_exact) / w_exact).abs();
    let err_w_f = ((w_f - w_exact) / w_exact).abs();
    let err_k_c = ((k_c - k_exact) / k_exact).abs();
    let err_k_f = ((k_f - k_exact) / k_exact).abs();
    let order = (err_k_c / err_k_f).log2();

    println!("omega exact {w_exact:.12} | dt={dt_coarse:.1e}: {w_c:.12} (rel {err_w_c:.3e}) | dt={dt_fine:.1e}: {w_f:.12} (rel {err_w_f:.3e})");
    println!("k     exact {k_exact:.9e} | dt={dt_coarse:.1e}: {k_c:.9e} (rel {err_k_c:.3e}) | dt={dt_fine:.1e}: {k_f:.9e} (rel {err_k_f:.3e})");
    println!("observed order of convergence in k = {order:.2} (theoretical 1)");

    assert!(
        err_w_c < 1e-4 && err_w_f < 1e-4,
        "omega must match the analytic decay law to the linear-solver floor: \
         rel err {err_w_c:e} (dt={dt_coarse}), {err_w_f:e} (dt={dt_fine})"
    );
    assert!(
        err_k_c < 1.0e-2,
        "k must track the analytic decay law: rel err {err_k_c:e} at dt = {dt_coarse}"
    );
    assert!(
        (0.9..=1.1).contains(&order),
        "k must converge at the first order the discretisation predicts; observed {order:.3} \
         (errors {err_k_c:e} -> {err_k_f:e})"
    );
}

/// V&V (verification) — attaching a closure changes the momentum solution by the
/// amount the effective viscosity dictates, and the laminar default does not
/// change it at all.
///
/// # Methodology
///
/// **What is computed.** The lid-driven cavity (400 cells, U_lid = 1 m/s along
/// the top wall, ν = 1e-3 m²/s → Re = U·L/ν = 100 at L = 0.1 m) advanced 100
/// steps at Δt = 5e-3 s, three ways: (a) the *default* closure; (b) an
/// explicitly constructed [`TurbulenceClosure::Laminar`]; (c) Wilcox k-ω started
/// from k = 1e-3 m²/s², ω = 10 s⁻¹ (ν_t0 = k/ω = 1e-4 m²/s, i.e. 10 % of ν).
///
/// **References — both exact, neither empirical.** (a) ≡ (b) must be
/// bit-identical, because the default *is* the laminar closure; and (c) must
/// differ from (a) by far more than solver round-off, because ν_t > 0 adds
/// diffusion to the momentum equation. A wiring bug that silently dropped ν_t
/// would pass the first check and fail the second; a wiring bug that perturbed
/// the laminar path would fail the first.
///
/// **Pass criteria.** max cell-wise |U_default − U_laminar| < 1e-14 m/s;
/// max |U_kω − U_laminar| > 1e-6 m/s; every velocity finite; ν_t > 0 and finite
/// in every cell at the end of the k-ω run.
///
/// # Results (run 2026-08-07, release mode, this machine)
///
/// - max |U_default − U_explicit-laminar| = 0.0 m/s — bit-identical, so adding
///   the closure layer perturbs no existing laminar result.
/// - max |U_kω − U_laminar| = 2.249334e-1 m/s (22.5 % of U_lid).
/// - peak interior |U|: laminar 0.799965 m/s, k-ω 0.852983 m/s.
/// - ν_t after 0.5 s: min 2.5736e-1, max 3.3015e-1 m²/s.
///
/// **Interpretation — and a blunt caveat.** The turbulent viscosity produced by
/// the Layer-4 closure demonstrably reaches the Layer-5 momentum predictor: it
/// moves the interior velocity field by 22 % of the lid speed. That is what this
/// test verifies, and all it verifies.
///
/// The *magnitude* is a red flag, not a result: ν_t/ν ≈ 260–330 in a Re = 100
/// cavity, where the true flow is laminar and ν_t should be ≈ 0. The cause is
/// the missing ω wall treatment — with zero-gradient ω at the walls, ω is not
/// driven to its `6ν/(β y²)` near-wall asymptote, so ν_t = k/ω is unbounded from
/// above near the wall. This is the concrete, measured consequence of the
/// `outram-foam-turbulence-lib` limitation recorded in this file's module
/// documentation, and it is why no wall-bounded RAS result from this stack may
/// be compared against a friction correlation and called validated.
#[test]
fn attaching_a_closure_changes_the_momentum_solution() {
    use outram_foam_appbuilder_lib::io::field_reader::{
        read_vol_scalar_field_full, read_vol_vector_field_full,
    };
    use outram_foam_basic_lib::prelude::Vector3;

    let case = Path::new(env!("CARGO_MANIFEST_DIR")).join("tutorials/cases/pimple_foam_cavity");
    let mesh = cavity_mesh();
    let n_steps = 100;
    let dt = 5.0e-3;

    let build = |closure: TurbulenceClosure| -> (Vec<Vector3>, Option<Vec<f64>>) {
        let control = ControlDict {
            start: StartControl::StartTime(0.0),
            stop: StopControl::EndTime(dt * n_steps as f64),
            delta_t: dt,
            ..ControlDict::default()
        };
        let mut solution = FvSolution::default();
        solution.pimple.n_outer_correctors = 1;
        solution.pimple.n_correctors = 2;
        let mut s = PimpleFoam::new(mesh.clone(), control, FvSchemes::default(), solution);
        s.u = read_vol_vector_field_full(&case.join("0").join("U"), &mesh).expect("read 0/U");
        s.p = read_vol_scalar_field_full(&case.join("0").join("p"), &mesh).expect("read 0/p");
        s.nu = VolScalarField::uniform("nu", mesh.clone(), 1.0e-3);
        s.turbulence = closure;
        for _ in 0..n_steps {
            s.step().expect("pimpleFoam step");
        }
        let u: Vec<Vector3> = s.u.internal.as_slice().to_vec();
        let nut = s.turbulence.nu_t().map(|f| f.internal.as_slice().to_vec());
        (u, nut)
    };

    // (a) default closure vs (b) explicit laminar — must be identical.
    let (u_default, nut_default) = build(TurbulenceClosure::default());
    let (u_laminar, _) = build(TurbulenceClosure::Laminar);
    assert!(
        nut_default.is_none(),
        "laminar closure exposes no nu_t field"
    );
    let d_lam = u_default
        .iter()
        .zip(&u_laminar)
        .map(|(a, b)| (*a - *b).mag())
        .fold(0.0_f64, f64::max);
    assert!(
        d_lam < 1e-14,
        "the default closure must be exactly laminar: max |ΔU| = {d_lam:e}"
    );

    // (c) Wilcox k-ω — must differ, and must produce a positive finite ν_t.
    let mut closure = TurbulenceClosure::k_omega(mesh.clone());
    closure.set_k_omega_uniform(1.0e-3, 10.0);
    let (u_turb, nut_turb) = build(closure);
    let nut = nut_turb.expect("k-omega closure exposes nu_t");

    let d_turb = u_turb
        .iter()
        .zip(&u_laminar)
        .map(|(a, b)| (*a - *b).mag())
        .fold(0.0_f64, f64::max);
    let nut_min = nut.iter().cloned().fold(f64::INFINITY, f64::min);
    let nut_max = nut.iter().cloned().fold(0.0_f64, f64::max);
    let peak_lam = u_laminar.iter().map(|v| v.mag()).fold(0.0_f64, f64::max);
    let peak_turb = u_turb.iter().map(|v| v.mag()).fold(0.0_f64, f64::max);
    println!(
        "max |U_default - U_laminar| = {d_lam:.6e} m/s\n\
         max |U_kOmega  - U_laminar| = {d_turb:.6e} m/s\n\
         nu_t in [{nut_min:.4e}, {nut_max:.4e}] m^2/s (nu = 1.0e-3)\n\
         peak interior |U|: laminar {peak_lam:.6} m/s, k-omega {peak_turb:.6} m/s"
    );

    assert!(
        u_turb.iter().all(|v| v.mag().is_finite()),
        "the turbulent run must stay finite"
    );
    assert!(
        nut.iter().all(|v| v.is_finite() && *v > 0.0),
        "k-omega must produce a positive finite nu_t everywhere"
    );
    assert!(
        d_turb > 1e-6,
        "nu_t must reach the momentum equation: max |ΔU| = {d_turb:e}"
    );
}
