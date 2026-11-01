// SPDX-License-Identifier: GPL-3.0-only
// Copyright (C) 2026 Theodore Ong and the outram-park contributors.

//! V&V for `petir::newton_krylov` (gh:#1066): JFNK on the 1-D Bratu problem
//! and a 1-D nonlinear (temperature-dependent conductivity) heat equation,
//! compared against an assembled-Jacobian Newton solution, with iteration
//! counts with and without preconditioning.
//!
//! Record: `crates/petir/verification_and_validation/implicit_ode_and_jfnk.md`.
//!
//! > Research, education and V&V only. AI draft, unreviewed.

use petir::linalg::{lu_decomp, lu_solve, Matrix};
use petir::multiroots::{fdjacobian, GSL_SQRT_DBL_EPSILON};
use petir::newton_krylov::{jfnk, jfnk_preconditioned, JfnkConfig, JfnkReport, KrylovSettings};

const N: usize = 100;

fn h() -> f64 {
    1.0 / (N as f64 + 1.0)
}

fn norm(v: &[f64]) -> f64 {
    v.iter().map(|x| x * x).sum::<f64>().sqrt()
}

/// The reference: plain Newton with the assembled analytic Jacobian, dense LU
/// (`petir::linalg::lu_decomp`), full steps, until the Newton step is below
/// `1e-12 max(||x||, 1)` (quadratic convergence: the error after that step is
/// far smaller still). A residual criterion was tried first (`||F|| < 1e-13
/// ||F_0||`) and never met: the `1/h^2`-scaled residual has a round-off floor
/// of ~1e-11 here. Returns the root and the Newton iteration count.
fn assembled_newton<F, J>(f: F, jac: J, x0: &[f64]) -> (Vec<f64>, usize)
where
    F: Fn(&[f64], &mut [f64]),
    J: Fn(&[f64], &mut Matrix),
{
    let n = x0.len();
    let mut x = x0.to_vec();
    let mut r = vec![0.0; n];
    f(&x, &mut r);
    let r0 = norm(&r);
    for it in 0..50 {
        let mut jm = Matrix::zeros(n, n).unwrap();
        jac(&x, &mut jm);
        let p = lu_decomp(&mut jm).unwrap();
        let rhs: Vec<f64> = r.iter().map(|v| -v).collect();
        let dx = lu_solve(&jm, &p, &rhs).unwrap();
        for (xi, di) in x.iter_mut().zip(&dx) {
            *xi += di;
        }
        f(&x, &mut r);
        if norm(&dx) < 1e-12 * norm(&x).max(1.0) {
            println!(
                "  reference Newton: ||F||/||F_0|| = {:.2e} after {} iterations, last ||dx|| = {:.2e}",
                norm(&r) / r0,
                it + 1,
                norm(&dx)
            );
            return (x, it + 1);
        }
    }
    panic!("reference Newton did not converge");
}

/// Check an analytic Jacobian against GSL's forward-difference one, so the
/// reference Newton is not resting on a hand-derivation error.
fn check_jacobian<F, J>(f: F, jac: J, x: &[f64])
where
    F: Fn(&[f64], &mut [f64]),
    J: Fn(&[f64], &mut Matrix),
{
    let n = x.len();
    let mut fx = vec![0.0; n];
    f(x, &mut fx);
    let mut fd = Matrix::zeros(n, n).unwrap();
    fdjacobian(&f, x, &fx, GSL_SQRT_DBL_EPSILON, &mut fd).unwrap();
    let mut an = Matrix::zeros(n, n).unwrap();
    jac(x, &mut an);
    let scale = an.as_slice().iter().fold(0.0_f64, |m, v| m.max(v.abs()));
    let worst = fd
        .as_slice()
        .iter()
        .zip(an.as_slice())
        .fold(0.0_f64, |m, (a, b)| m.max((a - b).abs()));
    assert!(worst < 1e-6 * scale, "analytic vs FD Jacobian: {worst} of {scale}");
}

/// JFNK settings. `eta` is the constant forcing term (GMRES relative
/// tolerance). The main measurement uses `ETA = 1e-4`; `1e-8` is run as well
/// and recorded, because it was the first choice and it failed (see
/// `ETA`).
fn config(eta: f64) -> JfnkConfig {
    JfnkConfig {
        max_iterations: 50,
        abs_tol: 1e-50,
        rel_tol: 1e-10,
        step_tol: 1e-14,
        linear: KrylovSettings {
            tolerance: eta,
            max_iter: 5000,
            restart: 200,
        },
        max_backtracks: 10,
        error_rel: petir::newton_krylov::PETSC_SQRT_MACHINE_EPSILON,
    }
}

/// The forcing term of the main measurement.
///
/// **Why 1e-4 and not the 1e-8 first written.** The first run (2026-11-01)
/// used `1e-8`. After the first Newton step every GMRES solve, in all six
/// variants, exhausted its 5000-iteration budget without reaching `1e-8`
/// (the drivers still converged, by the tolerated inexact steps, and agreed
/// with the reference to 1.2e-14 / 1.3e-12). Cause: the finite-difference
/// product's own error. These residuals are `1/h^2`-scaled differences of
/// O(1) values, so each component of `F` carries round-off ~`eps 4 |u| /
/// h^2` ~ 1e-11; divided by the differencing parameter (~1.5e-8 for a unit
/// direction) that is ~1e-3 of noise in `J v`, which puts the attainable
/// relative linear residual near `1e-3 ||x|| / ||F||` — far above `1e-8` once
/// `||F||` is small. A forcing term below the operator's noise floor buys
/// nothing but iterations. `1e-4` sits in the range Knoll & Keyes (J. Comput.
/// Phys. 193 (2004) 357, §2.2) give for a constant forcing term; it was fixed
/// before the second run, and the `1e-8` run is kept beside it as the
/// recorded failure.
const ETA: f64 = 1e-4;

/// Run JFNK unpreconditioned, with the approximate Jacobian and with the exact
/// one; print and return `(report, max |u - u_ref|)` for each.
fn three_ways<F, A, J>(
    name: &str,
    f: &F,
    approx: &A,
    exact: &J,
    u_ref: &[f64],
    eta: f64,
) -> [(JfnkReport, f64); 3]
where
    F: Fn(&[f64], &mut [f64]),
    A: Fn(&[f64], &mut Matrix),
    J: Fn(&[f64], &mut Matrix),
{
    let x0 = vec![0.0; u_ref.len()];
    let mut u = x0.clone();
    let none = jfnk(f, &mut u, &config(eta)).unwrap();
    let d_none = max_diff(&u, u_ref);
    report(&format!("{name} eta={eta:.0e} no precond"), &none, d_none);

    let mut u = x0.clone();
    let ap = jfnk_preconditioned(f, approx, &mut u, &config(eta)).unwrap();
    let d_ap = max_diff(&u, u_ref);
    report(&format!("{name} eta={eta:.0e} approx-J precond"), &ap, d_ap);

    let mut u = x0;
    let ex = jfnk_preconditioned(f, exact, &mut u, &config(eta)).unwrap();
    let d_ex = max_diff(&u, u_ref);
    report(&format!("{name} eta={eta:.0e} exact-J precond"), &ex, d_ex);

    [(none, d_none), (ap, d_ap), (ex, d_ex)]
}

fn report(label: &str, rep: &JfnkReport, diff: f64) {
    println!(
        "{label:<34} newton={:2} gmres/step={:?} total gmres={:5} F evals={:6} \
         ||F||={:.2e} reason={:?} max|u-u_ref|={:.2e}",
        rep.iterations,
        rep.linear_iterations,
        rep.total_linear_iterations(),
        rep.function_evaluations,
        rep.final_residual_norm,
        rep.reason,
        diff
    );
}

fn max_diff(a: &[f64], b: &[f64]) -> f64 {
    a.iter().zip(b).fold(0.0_f64, |m, (x, y)| m.max((x - y).abs()))
}

// ── Bratu ───────────────────────────────────────────────────────────────────

const LAMBDA: f64 = 1.0;

/// `-u'' = lambda e^u` on (0, 1), `u(0) = u(1) = 0`, second-order central
/// differences on `N` interior nodes.
fn bratu(u: &[f64], r: &mut [f64]) {
    let h2 = h() * h();
    for i in 0..N {
        let ul = if i == 0 { 0.0 } else { u[i - 1] };
        let ur = if i + 1 == N { 0.0 } else { u[i + 1] };
        r[i] = -(ul - 2.0 * u[i] + ur) / h2 - LAMBDA * u[i].exp();
    }
}

fn bratu_jac(u: &[f64], j: &mut Matrix) {
    let h2 = h() * h();
    for i in 0..N {
        j.set(i, i, 2.0 / h2 - LAMBDA * u[i].exp());
        if i > 0 {
            j.set(i, i - 1, -1.0 / h2);
        }
        if i + 1 < N {
            j.set(i, i + 1, -1.0 / h2);
        }
    }
}

/// The approximate Jacobian: the discrete Laplacian alone (drops the
/// `-lambda e^u` diagonal), constant over the solve.
fn bratu_laplacian(_u: &[f64], j: &mut Matrix) {
    let h2 = h() * h();
    for i in 0..N {
        j.set(i, i, 2.0 / h2);
        if i > 0 {
            j.set(i, i - 1, -1.0 / h2);
        }
        if i + 1 < N {
            j.set(i, i + 1, -1.0 / h2);
        }
    }
}

/// JFNK on 1-D Bratu, `lambda = 1`, against assembled-Jacobian Newton.
///
/// **Methodology.** `-u'' = e^u`, `u(0) = u(1) = 0`, `N = 100` interior nodes
/// (`h = 1/101`), initial guess `u = 0`. Reference: full-step Newton with the
/// analytic tridiagonal Jacobian assembled densely and solved by GSL's LU,
/// converged until the Newton step is below `1e-12 ||x||`; the analytic Jacobian is checked
/// against `gsl_multiroot_fdjacobian` first. JFNK: `rel_tol = 1e-10`,
/// `step_tol = 1e-14`, GMRES(200) to relative `1e-8`, Walker–Pernice
/// differencing with `error_rel = sqrt(eps)`, run three ways — no
/// preconditioner; right-preconditioned by the discrete Laplacian alone (an
/// approximate Jacobian, missing the `-e^u` diagonal); and by the exact
/// analytic Jacobian. Pass criteria: all three converge by the relative test;
/// `max |u - u_ref| < 1e-9`; preconditioning cuts total GMRES iterations by at
/// least 5x; exact-Jacobian preconditioning needs at most 3 GMRES iterations
/// per Newton step. (Iteration-count criteria apply to the `ETA = 1e-4` run.)
///
/// **Prediction (written before the run, 2026-11-01).** Reference Newton 3–4
/// iterations (`max u ≈ 0.14`). JFNK: 4–5 Newton steps in all three variants.
/// GMRES per step: unpreconditioned ~50–100 (the Laplacian's condition number
/// is ~4/(pi h)^2 ~ 4e3, and the 100-dimensional space bounds full GMRES at
/// 100); Laplacian-preconditioned 3–5 (the preconditioned operator is
/// `I - L^{-1} e^u`, eigenvalues in [0.88, 1]); exact-preconditioned 1–2.
/// Agreement with the reference ~1e-11.
///
/// **Second prediction, for `ETA = 1e-4` (written after the 1e-8 run and
/// before the 1e-4 run, 2026-11-01).** Newton 4–6 steps (linear convergence
/// at rate ~1e-4 once quadratic convergence is outpaced). GMRES per step:
/// unpreconditioned 20–60; Laplacian 2–3; exact 1. Agreement still ~1e-14.
///
/// **Results (2026-11-01, release build, x86-64).** PASS. Reference Newton:
/// 4 iterations, `max u = 0.140526506595`, final `||F||/||F_0|| = 1.3e-13`.
///
/// | forcing | preconditioner | Newton | GMRES per step | GMRES total | `F` evals | `max |u - u_ref|` |
/// |---|---|---|---|---|---|---|
/// | 1e-8 | none | 3 | 50, 5000, 5000 | 10050 | 10295 | 9.2e-15 |
/// | 1e-8 | Laplacian | 3 | 4, 5000, 5000 | 10004 | 22653 | 9.0e-15 |
/// | 1e-8 | exact J | 3 | 1, 5000, 5000 | 10001 | 30006 | 1.2e-14 |
/// | **1e-4** | none | 3 | 50, 50, 99 | **199** | 212 | 3.6e-14 |
/// | **1e-4** | Laplacian | 3 | 3, 2, 2 | **7** | 20 | 4.1e-13 |
/// | **1e-4** | exact J | 3 | 1, 1, 1 | **3** | 16 | 5.7e-15 |
///
/// At `1e-8` every GMRES solve after the first hit its 5000-iteration cap
/// (see [`ETA`]). At `1e-4`: the approximate (Laplacian) preconditioner cuts
/// GMRES iterations 28x and `F` evaluations 11x; the exact one 66x / 13x.
/// Newton steps 3 everywhere (predicted 4–6: one fewer than the reference
/// needs, because JFNK stops on the relative test at `1e-10` where the
/// reference iterates to a step criterion). Unpreconditioned GMRES needed
/// 50 per step (predicted 20–60 for `1e-4`); the 99 on the last step is
/// consistent with the finite-difference noise floor (see [`ETA`]) closing in
/// on `1e-4` as `||F||` shrinks, and was not investigated further.
#[test]
fn bratu_jfnk_matches_assembled_newton() {
    let x0 = vec![0.0; N];
    check_jacobian(bratu, bratu_jac, &vec![0.1; N]);
    let (u_ref, it_ref) = assembled_newton(bratu, bratu_jac, &x0);
    let umax = u_ref.iter().cloned().fold(f64::MIN, f64::max);
    println!("Bratu reference Newton: {it_ref} iterations, max u = {umax:.12}");

    // Recorded failure of the first protocol (see `ETA`): agreement only.
    for (rep, d) in three_ways("Bratu", &bratu, &bratu_laplacian, &bratu_jac, &u_ref, 1e-8) {
        assert!(d < 1e-9, "{d}");
        assert_eq!(rep.reason, petir::newton_krylov::JfnkConverged::RelativeResidual);
    }

    let [(none, d_none), (lap, d_lap), (ex, d_ex)] =
        three_ways("Bratu", &bratu, &bratu_laplacian, &bratu_jac, &u_ref, ETA);
    for (d, rep) in [(d_none, &none), (d_lap, &lap), (d_ex, &ex)] {
        assert!(d < 1e-9, "{d}");
        assert_eq!(rep.reason, petir::newton_krylov::JfnkConverged::RelativeResidual);
    }
    assert!(5 * lap.total_linear_iterations() <= none.total_linear_iterations());
    assert!(ex.linear_iterations.iter().all(|&k| k <= 3));
}

// ── nonlinear heat ──────────────────────────────────────────────────────────

const Q: f64 = 20.0;
const BETA: f64 = 1.0;

fn k(u: f64) -> f64 {
    1.0 + BETA * u * u
}

fn dk(u: f64) -> f64 {
    2.0 * BETA * u
}

/// Steady `-(k(u) u')' = q` on (0, 1), `u(0) = u(1) = 0`, `k = 1 + u^2`,
/// face conductivity the arithmetic mean of the two nodes.
fn heat(u: &[f64], r: &mut [f64]) {
    let h2 = h() * h();
    for i in 0..N {
        let ul = if i == 0 { 0.0 } else { u[i - 1] };
        let ur = if i + 1 == N { 0.0 } else { u[i + 1] };
        let kp = 0.5 * (k(u[i]) + k(ur));
        let km = 0.5 * (k(ul) + k(u[i]));
        r[i] = -(kp * (ur - u[i]) - km * (u[i] - ul)) / h2 - Q;
    }
}

fn heat_jac(u: &[f64], j: &mut Matrix) {
    let h2 = h() * h();
    for i in 0..N {
        let ul = if i == 0 { 0.0 } else { u[i - 1] };
        let ur = if i + 1 == N { 0.0 } else { u[i + 1] };
        let kp = 0.5 * (k(u[i]) + k(ur));
        let km = 0.5 * (k(ul) + k(u[i]));
        // F_i = -(A - B)/h^2 - q, A = kp (ur - ui), B = km (ui - ul).
        let da_dui = 0.5 * dk(u[i]) * (ur - u[i]) - kp;
        let db_dui = 0.5 * dk(u[i]) * (u[i] - ul) + km;
        j.set(i, i, -(da_dui - db_dui) / h2);
        if i + 1 < N {
            let da_dur = 0.5 * dk(ur) * (ur - u[i]) + kp;
            j.set(i, i + 1, -da_dur / h2);
        }
        if i > 0 {
            let db_dul = 0.5 * dk(ul) * (u[i] - ul) - km;
            j.set(i, i - 1, db_dul / h2);
        }
    }
}

/// The approximate (Picard) Jacobian: conductivities frozen at the current
/// iterate, the `k'(u)` terms dropped.
fn heat_picard(u: &[f64], j: &mut Matrix) {
    let h2 = h() * h();
    for i in 0..N {
        let ul = if i == 0 { 0.0 } else { u[i - 1] };
        let ur = if i + 1 == N { 0.0 } else { u[i + 1] };
        let kp = 0.5 * (k(u[i]) + k(ur));
        let km = 0.5 * (k(ul) + k(u[i]));
        j.set(i, i, (kp + km) / h2);
        if i + 1 < N {
            j.set(i, i + 1, -kp / h2);
        }
        if i > 0 {
            j.set(i, i - 1, -km / h2);
        }
    }
}

/// JFNK on a 1-D nonlinear heat equation against assembled-Jacobian Newton.
///
/// **Methodology.** Steady `-(k(u) u')' = 20`, `k = 1 + u^2`, `u(0) = u(1) =
/// 0`, `N = 100` interior nodes, arithmetic-mean face conductivity, initial
/// guess `u = 0`. Reference and JFNK settings as in the Bratu test (analytic
/// Jacobian checked against `gsl_multiroot_fdjacobian`). Preconditioners: none;
/// the Picard matrix (conductivities frozen at the iterate, `k'` terms
/// dropped — an approximate Jacobian); the exact Jacobian. Pass criteria as
/// for Bratu.
///
/// **Prediction (written before the run, 2026-11-01).** The linear-`k`
/// solution peaks at `q/8 = 2.5`; with `k = 1 + u^2` the peak drops to ~1.5.
/// Reference Newton 5–7 iterations from `u = 0` (stronger nonlinearity than
/// Bratu), possibly with JFNK needing a backtrack on the first step. GMRES
/// per step: unpreconditioned ~50–100; Picard-preconditioned 5–15 (the
/// dropped `k'` terms are first order in the gradient and not small);
/// exact-preconditioned 1–2. Agreement ~1e-11.
///
/// **Second prediction, for `ETA = 1e-4` (written after the 1e-8 run and
/// before the 1e-4 run, 2026-11-01).** Newton 6–8 steps; GMRES per step:
/// unpreconditioned 30–100; Picard 2–6; exact 1. Agreement ~1e-12.
///
/// **Results (2026-11-01, release build, x86-64).** PASS. Reference Newton:
/// 7 iterations, `max u = 1.460471950314`, final `||F||/||F_0|| = 1.9e-13`.
///
/// | forcing | preconditioner | Newton | GMRES per step | GMRES total | `F` evals | `max |u - u_ref|` |
/// |---|---|---|---|---|---|---|
/// | 1e-8 | none | 5 | 98, 5000 x4 | 20098 | 23330 | 1.3e-12 |
/// | 1e-8 | Picard | 5 | 1, 5000 x4 | 20001 | 45003 | 1.3e-12 |
/// | 1e-8 | exact J | 5 | 1, 5000 x4 | 20001 | 60007 | 1.3e-12 |
/// | **1e-4** | none | 5 | 50, 50, 50, 49, 50 | **249** | 271 | 1.3e-12 |
/// | **1e-4** | Picard | 6 | 1, 6, 5, 4, 4, 4 | **24** | 50 | 5.3e-15 |
/// | **1e-4** | exact J | 5 | 1, 1, 1, 1, 1 | **5** | 27 | 1.3e-12 |
///
/// Picard preconditioning cuts GMRES iterations 10x and `F` evaluations 5x;
/// the exact Jacobian 50x / 10x. Its first step needs 1 GMRES iteration
/// because at `u = 0` the Picard matrix IS the Jacobian (`k' = 0`). Newton
/// 5–6 steps (predicted 6–8; the reference needs 7 to its stricter step
/// criterion). The `F`-evaluation count of the exact-J run (27, one more than
/// 6 residuals + 5 x 3 GMRES products + 5 line-search trials) suggests one
/// backtrack; backtracks are not instrumented. The 1.3e-12 differences are the
/// solves that stopped at `||F|| ~ 8e-9` (relative test `2e-8`), against
/// 5.3e-15 for the one that happened to overshoot to `3.8e-11`.
#[test]
fn nonlinear_heat_jfnk_matches_assembled_newton() {
    let x0 = vec![0.0; N];
    let probe: Vec<f64> = (0..N).map(|i| 1.0 + 0.3 * (i as f64 * 0.1).sin()).collect();
    check_jacobian(heat, heat_jac, &probe);
    let (u_ref, it_ref) = assembled_newton(heat, heat_jac, &x0);
    let umax = u_ref.iter().cloned().fold(f64::MIN, f64::max);
    println!("Heat reference Newton: {it_ref} iterations, max u = {umax:.12}");

    // Recorded failure of the first protocol (see `ETA`): agreement only.
    for (rep, d) in three_ways("Heat", &heat, &heat_picard, &heat_jac, &u_ref, 1e-8) {
        assert!(d < 1e-9, "{d}");
        assert_eq!(rep.reason, petir::newton_krylov::JfnkConverged::RelativeResidual);
    }

    let [(none, d_none), (pic, d_pic), (ex, d_ex)] =
        three_ways("Heat", &heat, &heat_picard, &heat_jac, &u_ref, ETA);
    for (d, rep) in [(d_none, &none), (d_pic, &pic), (d_ex, &ex)] {
        assert!(d < 1e-9, "{d}");
        assert_eq!(rep.reason, petir::newton_krylov::JfnkConverged::RelativeResidual);
    }
    assert!(5 * pic.total_linear_iterations() <= none.total_linear_iterations());
    assert!(ex.linear_iterations.iter().all(|&k| k <= 3));
}
