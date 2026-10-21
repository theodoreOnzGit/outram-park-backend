// SPDX-License-Identifier: GPL-3.0-only
// Copyright (C) 2026 Theodore Ong and the outram-park contributors.

//! **V&V — the remaining six GSL-derived modules, code-to-code against GSL
//! 2.8 compiled and run.**
//!
//! # Why this file exists
//!
//! When `petir` was declared mature on 2026-09-15, the declaration's bar was
//! "agrees with GSL" — but the evidence was uneven. `cheb`, `expint`,
//! `gamma_inc` and `linalg::qr` had references produced by *executing* GSL.
//! `roots`, `min`, `deriv`, `interp`, `integration` and `ode` were ported from
//! GSL and tested against GSL's own assertions and against analytical results,
//! which is good evidence but a different and weaker kind. This closes that
//! gap so the bar means one thing across the crate.
//!
//! # The iterate sequence, not the answer
//!
//! For the iterative routines the reference records **every iterate**, not the
//! converged result. That is the point of it. Any correct bisection converges
//! to `sqrt(5)`, and so does any correct Brent — comparing final answers would
//! pass against a port of an entirely different algorithm, or against one that
//! silently substituted a better method. The trajectory pins what is actually
//! implemented: the sequence of brackets a bisection walks is unique to
//! bisection.
//!
//! The same reasoning drives the ODE cases, which compare the **per-step
//! error estimate** alongside the state. Two integrators can agree on `y` and
//! disagree completely on `yerr`, and `yerr` is what a step controller acts
//! on.
//!
//! # Results (2026-09-15)
//!
//! ```text
//!   module                values  bit-identical      worst relative diff
//!   roots (bracketing)      270   270  (100.0 %)     0
//!   roots (polishing)        33    33  (100.0 %)     0
//!   min                     150   150  (100.0 %)     0
//!   interp                  244   244  (100.0 %)     0
//!   ode (RKF45)             120   120  (100.0 %)     0
//!   deriv                    78    76  ( 97.4 %)     3.27e-16
//!   integration              28    27  ( 96.4 %)     1.82e-16
//! ```
//!
//! Five of the seven are bit-identical throughout, including every iterate of
//! every root finder and minimiser. The two that are not differ in the last
//! ulp, which is what reaching the same BLAS-1 kernels through different
//! object code gives.
//!
//! Two deviations were found by these comparisons rather than by reading, and
//! both are documented where they live: the secant solver's refusal to divide
//! by zero (see `the_polishing_root_finders_follow_gsls_iterates`) and GSL's
//! step-doubled RK4 (see `ode_stepping_matches_gsl`).
//!
//! # Skipping
//!
//! Absent reference file means skip, not fail.

use std::collections::BTreeMap;
use std::fs;
use std::path::PathBuf;

use petir::deriv::{backward, central, forward};
use petir::integration::{kronrod, qag, QkRule};
use petir::interp::{Interp, InterpMethod};
use petir::min::{MinMethod, Minimizer};
use petir::ode::{rk4_step, rkf45_step};
use petir::roots::{BracketingMethod, BracketingSolver, PolishingMethod, PolishingSolver};

/// A case is a name and its rows, each row a tag plus numbers.
type Cases = BTreeMap<String, Vec<(String, Vec<f64>)>>;

fn reference_path() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../reference-data/gsl/numerics-gsl-2.8-reference.txt")
}

fn load() -> Option<Cases> {
    let text = fs::read_to_string(reference_path()).ok()?;
    let mut cases: Cases = BTreeMap::new();
    let mut current = String::new();
    for line in text.lines() {
        let f: Vec<&str> = line.split_whitespace().collect();
        match f.first().copied() {
            Some("CASE") => {
                current = f.get(1).copied().unwrap_or("?").to_string();
                cases.insert(current.clone(), Vec::new());
            }
            Some("END") => current.clear(),
            Some(tag) => {
                let nums: Vec<f64> = f
                    .iter()
                    .skip(1)
                    .filter_map(|v| v.parse::<f64>().ok())
                    .collect();
                if let Some(rows) = cases.get_mut(&current) {
                    rows.push((tag.to_string(), nums));
                }
            }
            None => {}
        }
    }
    Some(cases)
}

/// Relative difference, falling back to absolute near zero.
fn rel(a: f64, b: f64) -> f64 {
    let scale = a.abs().max(b.abs());
    if scale < 1e-300 {
        (a - b).abs()
    } else {
        (a - b).abs() / scale
    }
}

/// Accumulates agreement statistics so each test can report real numbers.
#[derive(Default)]
struct Tally {
    compared: usize,
    identical: usize,
    worst: f64,
    worst_where: String,
}

impl Tally {
    fn check(&mut self, label: &str, got: f64, expected: f64) {
        self.compared += 1;
        if got.to_bits() == expected.to_bits() {
            self.identical += 1;
            return;
        }
        let d = rel(got, expected);
        if d > self.worst {
            self.worst = d;
            self.worst_where = format!("{label}: petir {got:e}, gsl {expected:e}");
        }
    }

    fn report(&self, what: &str, tol: f64) {
        let pct = 100.0 * self.identical as f64 / self.compared as f64;
        eprintln!(
            "{what} vs GSL 2.8: {} values, {} bit-identical ({pct:.1} %), \
             worst relative difference {:e}",
            self.compared, self.identical, self.worst
        );
        assert!(self.compared > 0, "{what}: nothing compared");
        assert!(
            self.worst < tol,
            "{what}: worst relative difference {:e} at {}",
            self.worst,
            self.worst_where
        );
    }
}

/// `x^2 - 5`, the function GSL's own `roots/test.c` uses.
fn quad(x: f64) -> f64 {
    x * x - 5.0
}
fn dquad(x: f64) -> f64 {
    2.0 * x
}

/// Every bracketing root finder must walk GSL's exact bracket sequence.
///
/// # Methodology
///
/// Bisection, false position and Brent on `x^2 - 5` over `[0, 5]`, 30
/// iterations each, comparing the reported root and both bracket ends at
/// every step.
#[test]
fn the_bracketing_root_finders_follow_gsls_iterates() {
    let Some(cases) = load() else {
        eprintln!("skipping: numerics-gsl-2.8-reference.txt not present");
        return;
    };
    let mut t = Tally::default();

    for (name, method) in [
        ("roots_bisection", BracketingMethod::Bisection),
        ("roots_falsepos", BracketingMethod::FalsePosition),
        ("roots_brent", BracketingMethod::Brent),
    ] {
        let rows = cases.get(name).unwrap_or_else(|| panic!("no case {name}"));
        let mut s = BracketingSolver::new(method, quad, 0.0, 5.0).unwrap();
        for (tag, nums) in rows {
            assert_eq!(tag, "IT");
            s.iterate().unwrap();
            let (Some(&root), Some(&lo), Some(&hi)) = (nums.get(1), nums.get(2), nums.get(3))
            else {
                panic!("{name}: malformed row");
            };
            t.check(&format!("{name} root"), s.root(), root);
            t.check(&format!("{name} lower"), s.x_lower(), lo);
            t.check(&format!("{name} upper"), s.x_upper(), hi);
        }
    }
    t.report("roots (bracketing)", 1e-13);
}

/// Newton, secant and Steffenson must walk GSL's exact iterate sequence, for
/// as long as PETIR is willing to iterate.
///
/// # Why Steffenson is the one that matters here
///
/// It keeps two values — the unaccelerated iterate that drives the next step,
/// and the possibly Aitken-accelerated value it reports. Conflating them
/// produces a solver that still converges on easy problems, so only a
/// trajectory comparison catches it. All twelve of its iterates match GSL's.
///
/// # A DELIBERATE DEVIATION this test found, and pins
///
/// Once the iteration has landed on the root exactly, the secant update's
/// denominator `f(x_n) - f(x_{n-1})` becomes zero. GSL divides anyway and the
/// iterate simply repeats forever — `roots_secant` in the reference shows
/// `2.23606797749978981` from iteration 7 onward. PETIR returns
/// [`PetirError::ZeroDivide`] instead, at iteration 9.
///
/// PETIR's behaviour is the more useful one: a caller looping to a tolerance
/// learns that no further progress is possible, where against GSL they would
/// spin to their iteration cap. But it IS a divergence from upstream, so it is
/// asserted here rather than left to be rediscovered: every iterate PETIR
/// produces must match GSL's, and PETIR may only stop **after** reaching the
/// converged value, never before.
#[test]
fn the_polishing_root_finders_follow_gsls_iterates() {
    let Some(cases) = load() else {
        eprintln!("skipping: numerics-gsl-2.8-reference.txt not present");
        return;
    };
    let mut t = Tally::default();

    for (name, method) in [
        ("roots_newton", PolishingMethod::Newton),
        ("roots_secant", PolishingMethod::Secant),
        ("roots_steffenson", PolishingMethod::Steffenson),
    ] {
        let rows = cases.get(name).unwrap_or_else(|| panic!("no case {name}"));
        let mut s = PolishingSolver::new(method, quad, dquad, 5.0).unwrap();
        let mut produced = 0usize;
        let mut stopped_at: Option<usize> = None;

        for (i, (tag, nums)) in rows.iter().enumerate() {
            assert_eq!(tag, "IT");
            if s.iterate().is_err() {
                // See this test's doc comment: upstream divides by zero and
                // repeats; PETIR refuses. Legal only once converged.
                stopped_at = Some(i);
                break;
            }
            let Some(&root) = nums.get(1) else {
                panic!("{name}: malformed row");
            };
            t.check(&format!("{name} root"), s.root(), root);
            produced += 1;
        }

        // Whatever PETIR did produce must have matched, and it must have gone
        // far enough to be AT the root before refusing -- stopping early would
        // be a defect wearing a deviation's clothes.
        assert!(
            produced >= 6,
            "{name}: only {produced} iterates before stopping"
        );
        let converged = 5.0_f64.sqrt();
        assert!(
            (s.root() - converged).abs() < 1e-12,
            "{name}: stopped at {} which is not the root {converged}",
            s.root()
        );
        if let Some(i) = stopped_at {
            eprintln!(
                "  {name}: PETIR stopped at iterate {i} (ZeroDivide) where GSL \
                 repeats the root -- deviation documented in this test"
            );
        }
    }
    t.report("roots (polishing)", 1e-12);
}

/// Golden section and Brent minimisation must walk GSL's iterates.
#[test]
fn the_minimisers_follow_gsls_iterates() {
    let Some(cases) = load() else {
        eprintln!("skipping: numerics-gsl-2.8-reference.txt not present");
        return;
    };
    let mut t = Tally::default();

    for (name, method) in [
        ("min_goldensection", MinMethod::GoldenSection),
        ("min_brent", MinMethod::Brent),
    ] {
        let rows = cases.get(name).unwrap_or_else(|| panic!("no case {name}"));
        let mut s = Minimizer::new(method, |x: f64| x.cos() + 1.0, 3.0, 0.0, 6.0).unwrap();
        for (tag, nums) in rows {
            assert_eq!(tag, "IT");
            s.iterate().unwrap();
            let (Some(&xm), Some(&lo), Some(&hi)) = (nums.get(1), nums.get(2), nums.get(3)) else {
                panic!("{name}: malformed row");
            };
            t.check(&format!("{name} x"), s.x_minimum(), xm);
            t.check(&format!("{name} lower"), s.x_lower(), lo);
            t.check(&format!("{name} upper"), s.x_upper(), hi);
        }
    }
    t.report("min", 1e-12);
}

/// Central, forward and backward differentiation, value AND error estimate.
#[test]
fn numerical_differentiation_matches_gsl() {
    let Some(cases) = load() else {
        eprintln!("skipping: numerics-gsl-2.8-reference.txt not present");
        return;
    };
    let rows = cases.get("deriv_exp").expect("no case deriv_exp");
    let mut t = Tally::default();

    for (tag, nums) in rows {
        let (Some(&x), Some(&value), Some(&abserr)) = (nums.get(1), nums.get(2), nums.get(3))
        else {
            panic!("malformed deriv row");
        };
        let got = match tag.as_str() {
            "C" => central(|v: f64| v.exp(), x, 1e-4),
            "F" => forward(|v: f64| v.exp(), x, 1e-4),
            _ => backward(|v: f64| v.exp(), x, 1e-4),
        };
        t.check(&format!("deriv {tag} value"), got.value, value);
        t.check(&format!("deriv {tag} abserr"), got.abserr, abserr);
    }
    t.report("deriv", 1e-12);
}

/// Linear and natural cubic spline interpolation, value and derivative.
#[test]
fn interpolation_matches_gsl() {
    let Some(cases) = load() else {
        eprintln!("skipping: numerics-gsl-2.8-reference.txt not present");
        return;
    };
    let mut t = Tally::default();

    let n = 9usize;
    let xa: Vec<f64> = (0..n)
        .map(|i| -1.0 + 2.0 * i as f64 / (n - 1) as f64)
        .collect();
    let ya: Vec<f64> = xa.iter().map(|&x| 1.0 / (1.0 + 25.0 * x * x)).collect();

    for (name, method) in [
        ("interp_linear", InterpMethod::Linear),
        ("interp_cspline", InterpMethod::CubicSpline),
    ] {
        let rows = cases.get(name).unwrap_or_else(|| panic!("no case {name}"));
        let s = Interp::new(method, &xa, &ya).unwrap();
        for (tag, nums) in rows {
            assert_eq!(tag, "P");
            let (Some(&x), Some(&value), Some(&deriv)) = (nums.get(1), nums.get(2), nums.get(3))
            else {
                panic!("{name}: malformed row");
            };
            t.check(&format!("{name} value"), s.eval(x).unwrap(), value);
            t.check(&format!("{name} deriv"), s.eval_deriv(x).unwrap(), deriv);
        }
    }
    t.report("interp", 1e-12);
}

/// All six Kronrod rules and the QAG driver.
#[test]
fn quadrature_matches_gsl() {
    let Some(cases) = load() else {
        eprintln!("skipping: numerics-gsl-2.8-reference.txt not present");
        return;
    };
    let rows = cases.get("integration").expect("no case integration");
    let mut t = Tally::default();
    let runge = |x: f64| 1.0 / (1.0 + 25.0 * x * x);

    for (tag, nums) in rows {
        match tag.as_str() {
            "QK" => {
                let (Some(&points), Some(&result), Some(&abserr), Some(&resabs), Some(&resasc)) = (
                    nums.first(),
                    nums.get(1),
                    nums.get(2),
                    nums.get(3),
                    nums.get(4),
                ) else {
                    panic!("malformed QK row");
                };
                let rule = match points as u32 {
                    15 => QkRule::Qk15,
                    21 => QkRule::Qk21,
                    31 => QkRule::Qk31,
                    41 => QkRule::Qk41,
                    51 => QkRule::Qk51,
                    _ => QkRule::Qk61,
                };
                let r = kronrod(rule, runge, -1.0, 1.0);
                let p = points as u32;
                t.check(&format!("qk{p} result"), r.result, result);
                t.check(&format!("qk{p} abserr"), r.abserr, abserr);
                t.check(&format!("qk{p} resabs"), r.resabs, resabs);
                t.check(&format!("qk{p} resasc"), r.resasc, resasc);
            }
            "QAG" => {
                let (Some(&result), Some(&abserr)) = (nums.first(), nums.get(1)) else {
                    panic!("malformed QAG row");
                };
                // The driver emits sin over [0, pi] first, then Runge.
                let is_sin = result > 1.5;
                let got = if is_sin {
                    qag(
                        QkRule::Qk21,
                        |x: f64| x.sin(),
                        0.0,
                        core::f64::consts::PI,
                        0.0,
                        1e-10,
                        200,
                    )
                } else {
                    qag(QkRule::Qk21, runge, -1.0, 1.0, 0.0, 1e-10, 200)
                }
                .unwrap();
                t.check("qag value", got.value, result);
                t.check("qag abserr", got.abserr, abserr);
            }
            other => panic!("unexpected integration tag {other}"),
        }
    }
    t.report("integration", 1e-12);
}

/// RKF45 stepping, comparing the per-step ERROR ESTIMATE as well as the
/// state — see the module docs for why that is the sharper half.
///
/// # RK4 IS DELIBERATELY NOT COMPARED, and finding out why was the point
///
/// GSL's public `gsl_odeiv2_step_rk4` is **not** a classical RK4 step.
/// `rk4_apply` (`ode-initval2/rk4.c:247`) takes one full step, then **two
/// half-steps**, reports the half-step result as `y` and their difference as
/// `yerr` — step-doubling, to manufacture an error estimate the classical
/// method does not have. `rk4.c` also contains the plain single step, as a
/// private `rk4_step` helper, and **that** is what `petir::ode::rk4_step`
/// ports.
///
/// So the two are different quantities, not a port and its original. Comparing
/// them showed a discrepancy of 4.68e-8 at `h = 0.05` — which is not a defect
/// but exactly the `O(h^5)` local error you would expect to separate a full
/// step from two half-steps, and treating it as a tolerance to be loosened
/// would have buried a real difference under a wider bound.
///
/// GSL exposes no way to reach its inner helper, so there is nothing to
/// compare against through the public API. `gsl_ode.rs` checks PETIR's RK4
/// against its analytical convergence order instead, which is the right
/// instrument for it.
#[test]
fn ode_stepping_matches_gsl() {
    let Some(cases) = load() else {
        eprintln!("skipping: numerics-gsl-2.8-reference.txt not present");
        return;
    };
    let mut t = Tally::default();
    let h = 0.05_f64;

    // dy/dt = y, one component.
    {
        let rows = cases.get("ode_rkf45").expect("no case ode_rkf45");
        let mut y = vec![1.0_f64];
        let mut time = 0.0_f64;
        for (tag, nums) in rows {
            assert_eq!(tag, "S");
            let (Some(&y0), Some(&e0)) = (nums.get(2), nums.get(3)) else {
                panic!("ode_rkf45: malformed row");
            };
            let step = rkf45_step(|_t, s: &[f64], d: &mut [f64]| d[0] = s[0], time, &y, h).unwrap();
            t.check("rkf45 y", step.y.first().copied().unwrap_or(f64::NAN), y0);
            t.check(
                "rkf45 yerr",
                step.yerr.first().copied().unwrap_or(f64::NAN),
                e0,
            );
            y = step.y;
            time += h;
        }
    }

    // RK4 is deliberately not compared -- see this test's doc comment. What IS
    // asserted is that the difference has the size step-doubling predicts, so
    // the explanation above is measured rather than asserted.
    {
        let rows = cases.get("ode_rk4").expect("no case ode_rk4");
        let mut y = vec![1.0_f64];
        let mut time = 0.0_f64;
        let mut worst_gap = 0.0_f64;
        for (tag, nums) in rows {
            assert_eq!(tag, "S");
            let Some(&gsl_y) = nums.get(2) else {
                panic!("ode_rk4: malformed row");
            };
            y = rk4_step(|_t, s: &[f64], d: &mut [f64]| d[0] = s[0], time, &y, h).unwrap();
            worst_gap = worst_gap.max((y.first().copied().unwrap_or(f64::NAN) - gsl_y).abs());
            time += h;
        }
        eprintln!(
            "rk4: PETIR's single classical step vs GSL's step-DOUBLED stepper \
             differs by at most {worst_gap:e} over 20 steps at h = {h} -- \
             these are different quantities, see the doc comment"
        );
        assert!(
            worst_gap > 1e-12,
            "the two agreed, which contradicts the step-doubling reading of \
             rk4.c -- re-read ode-initval2/rk4.c before trusting this test"
        );
        assert!(
            worst_gap < 1e-5,
            "gap {worst_gap:e} is far larger than the O(h^5) a step-doubling \
             difference explains; something else is wrong"
        );
    }

    // The harmonic oscillator, two components -- exercises the coupling that
    // a one-component system cannot.
    let rows = cases.get("ode_rkf45osc").expect("no case ode_rkf45osc");
    let mut y = vec![1.0_f64, 0.0];
    let mut time = 0.0_f64;
    for (tag, nums) in rows {
        assert_eq!(tag, "S");
        let step = rkf45_step(
            |_t, s: &[f64], d: &mut [f64]| {
                d[0] = s[1];
                d[1] = -s[0];
            },
            time,
            &y,
            h,
        )
        .unwrap();
        for (k, offset) in [(0usize, 2usize), (1, 4)] {
            let (Some(&yk), Some(&ek)) = (nums.get(offset), nums.get(offset + 1)) else {
                panic!("malformed oscillator row");
            };
            t.check(
                &format!("rkf45osc y{k}"),
                step.y.get(k).copied().unwrap_or(f64::NAN),
                yk,
            );
            t.check(
                &format!("rkf45osc yerr{k}"),
                step.yerr.get(k).copied().unwrap_or(f64::NAN),
                ek,
            );
        }
        y = step.y;
        time += h;
    }

    t.report("ode", 1e-12);
}
