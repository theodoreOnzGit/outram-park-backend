// SPDX-License-Identifier: GPL-3.0-only
// Copyright (C) 2026 OUTRAM PARK contributors

//! **Verification of `petir::ssprk`: observed order, and the SSP property.**
//!
//! # 1. Order of convergence
//!
//! **Methodology.** Integrate the linear, non-autonomous system
//! `y' = -y + cos t`, `y(0) = 1`, whose exact solution is
//! `y = (cos t + sin t)/2 + e^{-t}/2`, together with `z' = -2z`, `z(0) = 1`.
//! The non-autonomous forcing exercises the stage times as well as the stage
//! weights. Run to `T = 1` with `N = 10, 20, 40, 80, 160` steps. Take the max
//! error over both components at `T`. The observed order is
//! `log2(e_N / e_{2N})`.
//!
//! **Pass criterion.** The finest observed order is within ±0.1 of the formal
//! order: 2 for SSP22, 3 for SSP33, 4 for SSP104.
//!
//! **Results (2026-10-16, x86_64-linux, release).**
//!
//! | method | N=10→20 | 20→40 | 40→80 | 80→160 | error at N=160 |
//! |---|---|---|---|---|---|
//! | SSP22 | 2.1166 | 2.0562 | 2.0276 | **2.0137** | 7.12e-6 |
//! | SSP33 | 3.1155 | 3.0578 | 3.0289 | **3.0144** | 2.22e-8 |
//! | SSP104 | 4.0474 | 4.0239 | 4.0120 | **4.0077** | 3.07e-12 |
//!
//! Each converges to its formal order from above, as the leading error term
//! takes over. This verifies the tableau and the stage times. It does not
//! show bit-faithfulness to upstream; `ssprk_vs_pyclaw.rs` does that.
//!
//! # 2. Strong stability: total variation does not increase
//!
//! **Methodology.** Use first-order upwind advection, `q_t + q_x = 0`, on 100
//! periodic cells with a square-wave initial condition. The forward-Euler step
//! of this operator is TVD for CFL `nu <= 1`. The SSP property then promises TVD
//! for `nu <= C`, where C is 1 for SSP22 and SSP33 and 6 for SSP104. Take 300
//! steps and check `TV(q^{n+1}) <= TV(q^n) + 1e-12` at every step.
//!
//! **Negative control.** The same check at `nu` above `C` must catch a TV
//! *increase*. Without this, a check that could never fail would prove
//! nothing (root `CLAUDE.md`, "Get the PROCESS right").
//!
//! **Results (2026-10-16, x86_64-linux, release).** Worst per-step TV change
//! over 300 steps:
//!
//! | method | CFL ≤ C (must be ≤ 1e-12) | CFL just above C (control) | far above C (control) |
//! |---|---|---|---|
//! | SSP22 | 0.25: 1.1e-15 · 0.5: 0 · 1.0: 1.6e-15 | 1.05: **+1.1e12** | 1.5: +2.0e119 |
//! | SSP33 | 0.25: 8.9e-16 · 0.5: 8.9e-16 · 1.0: 4.4e-16 | 1.05: **+0.110** | 1.5: +1.5e90 |
//! | SSP104 | 1: 1.1e-15 · 3: 8.9e-16 · 6: 2.2e-15 | 6.3: **+1.3e69** | 7.0: +7.1e234 |
//!
//! Within `C`, every change is round-off (≤ 10 ulp of TV = 2). Above `C` the
//! check fires, including SSP33 at CFL 1.05, where the run stays bounded and
//! the TV still grows by 0.11 in one step. So the check can fail just past
//! the coefficient, not only once the scheme blows up.
//!
use petir::ssprk::{ssp_step, SspMethod, SspWorkspace};

fn exact(t: f64) -> [f64; 2] {
    [0.5 * (t.cos() + t.sin()) + 0.5 * (-t).exp(), (-2.0 * t).exp()]
}

fn ode(t: f64, y: &[f64], d: &mut [f64]) {
    d[0] = -y[0] + t.cos();
    d[1] = -2.0 * y[1];
}

fn error_at_one(m: SspMethod, n: usize) -> f64 {
    let dt = 1.0 / n as f64;
    let mut y = [1.0, 1.0];
    let mut w = SspWorkspace::new(2);
    let mut t = 0.0;
    for _ in 0..n {
        ssp_step(m, ode, t, &mut y, dt, &mut w).unwrap();
        t += dt;
    }
    let e = exact(1.0);
    (y[0] - e[0]).abs().max((y[1] - e[1]).abs())
}

/// Measured 2026-10-16: finest observed orders 2.0137, 3.0144 and 4.0077
/// (table in the module docs).
#[test]
fn observed_order_matches_formal_order() {
    for (m, p) in [(SspMethod::Ssp22, 2.0), (SspMethod::Ssp33, 3.0), (SspMethod::Ssp104, 4.0)] {
        let ns = [10, 20, 40, 80, 160];
        let errs: Vec<f64> = ns.iter().map(|&n| error_at_one(m, n)).collect();
        let orders: Vec<f64> = errs.windows(2).map(|w| (w[0] / w[1]).log2()).collect();
        println!("{m:?}: errors {errs:?}  observed orders {orders:.4?}");
        let finest = *orders.last().unwrap();
        assert!((finest - p).abs() < 0.1, "{m:?}: observed order {finest} vs formal {p}");
    }
}

const NCELL: usize = 100;

fn upwind(nu: f64) -> impl FnMut(f64, &[f64], &mut [f64]) {
    // dt = 1, dx = 1/nu-scaled: L(q)_i = -nu (q_i - q_{i-1}), so forward Euler
    // with dt = 1 runs at CFL nu.
    move |_t, q, d| {
        let mut qm = *q.last().unwrap();
        for (di, &qi) in d.iter_mut().zip(q.iter()) {
            *di = -nu * (qi - qm);
            qm = qi;
        }
    }
}

fn tv(q: &[f64]) -> f64 {
    let mut s = (q[0] - q[NCELL - 1]).abs();
    for w in q.windows(2) {
        s += (w[1] - w[0]).abs();
    }
    s
}

/// Largest single-step TV increase over 300 steps at CFL `nu` (<= 0 means TVD).
fn worst_tv_increase(m: SspMethod, nu: f64) -> f64 {
    let mut q: Vec<f64> = (0..NCELL).map(|i| if (20..50).contains(&i) { 1.0 } else { 0.0 }).collect();
    let mut w = SspWorkspace::new(NCELL);
    let mut worst = f64::NEG_INFINITY;
    let mut t = 0.0;
    for _ in 0..300 {
        let before = tv(&q);
        ssp_step(m, upwind(nu), t, &mut q, 1.0, &mut w).unwrap();
        t += 1.0;
        worst = worst.max(tv(&q) - before);
    }
    worst
}

/// The SSP promise at and below `C`. Measured 2026-10-16: worst per-step TV
/// change 2.2e-15 across all nine cases (table in the module docs).
#[test]
fn total_variation_does_not_increase_up_to_the_ssp_coefficient() {
    for (m, nus) in [
        (SspMethod::Ssp22, [0.25, 0.5, 1.0]),
        (SspMethod::Ssp33, [0.25, 0.5, 1.0]),
        (SspMethod::Ssp104, [1.0, 3.0, 6.0]),
    ] {
        assert!(nus.iter().all(|&nu| nu <= m.ssp_coefficient()));
        for nu in nus {
            let inc = worst_tv_increase(m, nu);
            println!("{m:?} CFL {nu}: worst per-step TV change {inc:e}");
            assert!(inc <= 1e-12, "{m:?} at CFL {nu}: TV grew by {inc:e}");
        }
    }
}

/// Negative control: above `C` the same check must fire, or it could not
/// have failed above. Measured 2026-10-16: it fires in all six cases, the
/// smallest growth being SSP33 at CFL 1.05 (+0.110).
#[test]
fn negative_control_tv_grows_above_the_ssp_coefficient() {
    for (m, nu) in [
        (SspMethod::Ssp22, 1.05),
        (SspMethod::Ssp33, 1.05),
        (SspMethod::Ssp104, 6.3),
        (SspMethod::Ssp22, 1.5),
        (SspMethod::Ssp33, 1.5),
        (SspMethod::Ssp104, 7.0),
    ] {
        let inc = worst_tv_increase(m, nu);
        println!("{m:?} CFL {nu} (> C = {}): worst per-step TV change {inc:e}", m.ssp_coefficient());
        assert!(inc > 1e-6, "{m:?} at CFL {nu}: the TV check failed to detect growth ({inc:e})");
    }
}
