// SPDX-License-Identifier: GPL-3.0-only
// Copyright (C) 2026 OUTRAM PARK contributors.

//! **V&V — `petir::ode::implicit` code-to-code against GSL 2.8 compiled and
//! run (gh:#1338 case 5, epic gh:#1336).**
//!
//! # What is ported, so what is compared
//!
//! `ode::implicit` ports GSL's `modnewton1` (the modified-Newton stage solve),
//! the single implicit-Euler step of `rk1imp_apply`, `rksubs` and the
//! `std_control_errlevel` error level. It does **not** port `rk2imp`,
//! `rk4imp`, `bsimp` or `msbdf` (module docs), so those are not compared.
//! Two things it does differently from `rk1imp_apply` by design: no step
//! doubling (so no `yerr`, and no two-half-step result) and no retried steps.
//!
//! - **Backward Euler** is compared against `rk1imp_apply` itself: the driver
//!   `#include`s `rk1imp.c` and reads its file-static `y_onestep`, the
//!   single step petir ports (the `TWO`/`ERR` rows, GSL's returned two-half-
//!   step state and error estimate, are in the reference but petir has no
//!   counterpart, and the half steps reuse the first step's Jacobian and
//!   error levels, so they cannot be reproduced from petir's public step).
//! - **BDF2** has no GSL counterpart (its coefficients are OpenFOAM's). Its
//!   Newton solve *is* `modnewton1` with `A = [[1/coefft]]`, so the driver
//!   runs GSL's `modnewton1_init/_solve` + `rksubs` with that `A`; the C
//!   `ybar` formula is the driver's transcription of the Rust one, so this
//!   checks the Newton/LU/`rksubs` composition, not the coefficients.
//!
//! Problems from `ode-initval2/test.c`: the stiff linear system, Van der Pol
//! (mu = 10) and Robertson, with FIXED steps (no controller in the loop), each
//! step started from the single-step result so every step is compared from
//! its own state; tolerances 1e-10/1e-8, 1e-14/1e-12 and 1e-6/1e-4. The
//! reference includes steps where GSL's Newton FAILS (`GSL_FAILURE`, divergence
//! or 7 iterations); petir must fail at the same step.
//!
//! # Predicted agreement (written before running, 2026-11-10)
//!
//! Bit-identical in every state component. Same expressions in the same
//! order (the port kept `-h*a*J`, `-1.0*Yk + y0 + h*sum`, `rksubs`'s
//! `0.0 += b fY; *= h; += y0`), LU already bit-identical to GSL's, `nrm2`
//! the same GSL kernel, and `pow(eeta_prev, 0.8)` through the same glibc
//! (`Real::powf` in petir is `libm` -- a possible 1-ulp difference that only
//! matters if it flips the `eeta*|dscal| < 1` test). Failures are predicted
//! to occur at the same step. Divergence between GSL status codes (all
//! `GSL_FAILURE` = -1) and petir's `Diverged`/`MaxIterations` is by design.
//!
//! # Results
//!
//! See the per-test docs and `crates/petir/docs/verification-summary.md`.

use std::collections::BTreeMap;

use petir::linalg::Matrix;
use petir::ode::implicit::{backward_euler_step, bdf2_step, ImplicitTolerance};

const REFERENCE: &str = include_str!("../../../reference-data/gsl/ode-implicit-gsl-2.8-reference.txt");

struct Case {
    n: usize,
    nsteps: usize,
    h: f64,
    eabs: f64,
    erel: f64,
    y0: Vec<f64>,
    one: Vec<Vec<f64>>,
    fail: Option<usize>,
}

fn bits(s: &str) -> f64 {
    f64::from_bits(u64::from_str_radix(s, 16).unwrap())
}

fn load() -> BTreeMap<String, Case> {
    let mut out = BTreeMap::new();
    let mut cur: Option<(String, Case)> = None;
    for line in REFERENCE.lines() {
        let f: Vec<&str> = line.split_whitespace().collect();
        match f[0] {
            "CASE" => {
                cur = Some((
                    f[1].to_string(),
                    Case {
                        n: f[2].parse().unwrap(),
                        nsteps: f[3].parse().unwrap(),
                        h: f[4].parse().unwrap(),
                        eabs: f[5].parse().unwrap(),
                        erel: f[6].parse().unwrap(),
                        y0: vec![],
                        one: vec![],
                        fail: None,
                    },
                ));
            }
            "END" => {
                let (name, c) = cur.take().unwrap();
                out.insert(name, c);
            }
            "Y" => cur.as_mut().unwrap().1.y0 = f[2..].iter().map(|s| bits(s)).collect(),
            "ONE" => cur.as_mut().unwrap().1.one.push(f[2..].iter().map(|s| bits(s)).collect()),
            "FAIL" => cur.as_mut().unwrap().1.fail = Some(f[1].parse().unwrap()),
            _ => {}
        }
    }
    out
}

type Rhs = fn(f64, &[f64], &mut [f64]);
type Jac = fn(f64, &[f64], &mut Matrix);

fn rhs_stiff(_t: f64, y: &[f64], f: &mut [f64]) {
    f[0] = 998.0 * y[0] + 1998.0 * y[1];
    f[1] = -999.0 * y[0] - 1999.0 * y[1];
}
fn jac_stiff(_t: f64, _y: &[f64], d: &mut Matrix) {
    d.set(0, 0, 998.0);
    d.set(0, 1, 1998.0);
    d.set(1, 0, -999.0);
    d.set(1, 1, -1999.0);
}
fn rhs_vdp(_t: f64, y: &[f64], f: &mut [f64]) {
    let mu = 10.0;
    f[0] = y[1];
    f[1] = -y[0] + mu * y[1] * (1.0 - y[0] * y[0]);
}
fn jac_vdp(_t: f64, y: &[f64], d: &mut Matrix) {
    let mu = 10.0;
    d.set(0, 0, 0.0);
    d.set(0, 1, 1.0);
    d.set(1, 0, -2.0 * mu * y[0] * y[1] - 1.0);
    d.set(1, 1, mu * (1.0 - y[0] * y[0]));
}
fn rhs_rob(_t: f64, y: &[f64], f: &mut [f64]) {
    let (a, b, c) = (0.04, 1.0e4, 3.0e7);
    f[0] = -a * y[0] + b * y[1] * y[2];
    f[2] = c * y[1] * y[1];
    f[1] = -f[0] - f[2];
}
fn jac_rob(_t: f64, y: &[f64], d: &mut Matrix) {
    let (a, b, c) = (0.04, 1.0e4, 3.0e7);
    let rows = [
        [-a, b * y[2], b * y[1]],
        [a, -b * y[2] - 2.0 * c * y[1], -b * y[1]],
        [0.0, 2.0 * c * y[1], 0.0],
    ];
    for (i, r) in rows.iter().enumerate() {
        for (j, v) in r.iter().enumerate() {
            d.set(i, j, *v);
        }
    }
}

fn problem(suffix: &str) -> (Rhs, Jac) {
    if suffix.starts_with("stiff") {
        (rhs_stiff, jac_stiff)
    } else if suffix.starts_with("vdp") {
        (rhs_vdp, jac_vdp)
    } else {
        (rhs_rob, jac_rob)
    }
}

#[derive(Default)]
struct Tally {
    n: usize,
    same: usize,
    steps_ok: usize,
    fails_matched: usize,
    fails_total: usize,
    worst: f64,
}

/// March one reference case with petir; `bdf2` selects the stepper.
fn compare(name: &str, c: &Case, bdf2: bool, t: &mut Tally) {
    let suffix = name.split_once('_').unwrap().1;
    let (f, j) = problem(suffix);
    let tol = ImplicitTolerance { eps_abs: c.eabs, eps_rel: c.erel };
    let mut y = c.y0.clone();
    let mut y_old: Vec<f64> = vec![];
    let mut tt = 0.0_f64;
    let mut h_old = 0.0_f64;
    let ff = |t: f64, y: &[f64], out: &mut [f64]| f(t, y, out);
    let jj = |t: f64, y: &[f64], m: &mut Matrix| j(t, y, m);
    for k in 0..c.nsteps {
        let r = if bdf2 && k > 0 {
            bdf2_step(&ff, &jj, tt, &y, &y_old, c.h, h_old, &tol)
        } else {
            backward_euler_step(&ff, &jj, tt, &y, c.h, &tol)
        };
        if c.fail == Some(k) {
            t.fails_total += 1;
            assert!(r.is_err(), "{name}: GSL failed at step {k} but petir returned {r:?}");
            t.fails_matched += 1;
            return;
        }
        let ynew = r.unwrap_or_else(|e| panic!("{name}: petir failed at step {k} ({e:?}), GSL did not"));
        let want = &c.one[k];
        for i in 0..c.n {
            t.n += 1;
            if ynew[i].to_bits() == want[i].to_bits() {
                t.same += 1;
            } else {
                let d = (ynew[i] - want[i]).abs() / ynew[i].abs().max(want[i].abs());
                t.worst = t.worst.max(d);
                eprintln!("DIFF {name} step {} y[{i}]: petir {:e} gsl {:e}", k + 1, ynew[i], want[i]);
            }
        }
        t.steps_ok += 1;
        // next step starts from GSL's single-step result (== petir's when identical)
        y_old = std::mem::replace(&mut y, want.clone());
        h_old = c.h;
        tt += c.h;
    }
    if let Some(k) = c.fail {
        panic!("{name}: GSL failed at step {k} (never reached)");
    }
}

fn run_family(prefix: &str, bdf2: bool) -> Tally {
    let cases = load();
    let mut t = Tally::default();
    let mut ncase = 0;
    for (name, c) in &cases {
        if name.starts_with(prefix) {
            ncase += 1;
            compare(name, c, bdf2, &mut t);
        }
    }
    assert!(ncase > 0);
    eprintln!(
        "{prefix}: {ncase} cases, {} steps, {}/{} state values bit-identical, failures matched {}/{}, worst rel {:e}",
        t.steps_ok, t.same, t.n, t.fails_matched, t.fails_total, t.worst
    );
    t
}

fn assert_identical(t: &Tally) {
    assert_eq!(t.same, t.n, "worst relative difference {:e}", t.worst);
    assert_eq!(t.fails_matched, t.fails_total);
}

/// Backward Euler vs `rk1imp_apply`'s single step on the stiff linear,
/// Van der Pol and Robertson problems (`be_*` cases): state bit-identical at
/// every step, and every GSL Newton failure reproduced at the same step.
#[test]
fn backward_euler_step_matches_rk1imp_single_step() {
    let t = run_family("be_", false);
    assert!(t.fails_total >= 3, "reference should include Newton failures");
    assert_identical(&t);
}

/// BDF2 (Euler first step, then the variable-step BDF2 coefficients) vs
/// GSL's `modnewton1` + `rksubs` with `A = [[1/coefft]]` (`bdf2_*` cases).
#[test]
fn bdf2_step_matches_modnewton1_with_bdf2_coefficients() {
    let t = run_family("bdf2_", true);
    assert!(t.fails_total >= 3);
    assert_identical(&t);
}

/// Integration: a whole marched solve on the stiff system through
/// `solve_backward_euler` / `solve_bdf2` (whole time grids), against the
/// reference trajectories of the 20-step stiff cases.
///
/// The grid API takes time points, so each step's `h` is `t_{k+1} - t_k`,
/// which differs from the driver's `h` by grid rounding unless `h` is a
/// power of two. So: `h = 0.5` (exact grid) must be **bit-identical**;
/// `h = 0.01` (inexact grid, `h` off by up to ~1e-18) is only required to be
/// within 1e-10 relative, and the measured worst is printed. That difference
/// is the grid, not the algorithm: the step-level tests above are
/// bit-identical at `h = 0.01`.
#[test]
fn whole_stiff_solves_match_gsl_trajectories() {
    use petir::ode::implicit::{solve_backward_euler, solve_bdf2};
    let cases = load();
    let ff = |t: f64, y: &[f64], out: &mut [f64]| rhs_stiff(t, y, out);
    let jj = |t: f64, y: &[f64], m: &mut Matrix| jac_stiff(t, y, m);
    for (cname, bdf2, exact) in [
        ("be_stiff_h05", false, true),
        ("bdf2_stiff_h05", true, true),
        ("be_stiff_h001", false, false),
        ("bdf2_stiff_h001", true, false),
    ] {
        let c = &cases[cname];
        let tol = ImplicitTolerance { eps_abs: c.eabs, eps_rel: c.erel };
        let mut grid = vec![0.0_f64];
        for _ in 0..c.nsteps {
            let last = *grid.last().unwrap();
            grid.push(last + c.h);
        }
        let sol = if bdf2 {
            solve_bdf2(&ff, &jj, &grid, &c.y0, &tol).unwrap()
        } else {
            solve_backward_euler(&ff, &jj, &grid, &c.y0, &tol).unwrap()
        };
        let (mut n, mut same, mut worst) = (0usize, 0usize, 0.0_f64);
        for k in 0..c.nsteps {
            for i in 0..c.n {
                n += 1;
                let (a, b) = (sol.points[k + 1].y[i], c.one[k][i]);
                if a.to_bits() == b.to_bits() {
                    same += 1;
                } else {
                    worst = worst.max((a - b).abs() / a.abs().max(b.abs()));
                }
            }
        }
        eprintln!("whole solve {cname}: {same}/{n} bit-identical, worst rel {worst:e}");
        if exact {
            assert_eq!(same, n, "{cname}: worst {worst:e}");
        } else {
            assert!(worst < 1e-10, "{cname}: worst {worst:e}");
        }
    }
}
