// SPDX-License-Identifier: GPL-3.0-only
// Copyright (C) 2026 Theodore Ong and the outram-park contributors.

//! Code-to-code verification of `petir::integration::qags` against
//! `gsl_integration_qags` compiled from GSL 2.8 (commit `cf180cd7`).
//!
//! # Methodology
//!
//! `reference-data/gsl/qags_reference_driver.c` calls GSL's QAGS on 21 cases
//! and prints, per case, the status code, the result, the error estimate, the
//! number of sub-intervals left in the workspace and the number of integrand
//! evaluations. The build recipe (GSL's own `integration/` and `err/` sources
//! from the vendored tree, `gcc -O2`, glibc `libm`) is in the driver header and
//! in `reference-data/gsl/README.md`. This test replays every case with the
//! same integrand, written with the same libm calls (Rust's `f64` methods reach
//! glibc under a `std` build, as the C driver does), and compares:
//!
//! - status, sub-interval count and evaluation count **exactly** — these
//!   identify the path taken through the routine, which a port of a
//!   different algorithm would not reproduce even when it converges to the
//!   same value;
//! - the result to a relative 1e-12, the bar the other GSL numerics
//!   comparisons in this crate use;
//! - the error estimate to a relative **1e-3**, upstream's own criterion for
//!   it (`integration/test.c` checks QAGS `abserr` to 1e-6 for `f1` and 1e-3
//!   for `f11`, results to 1e-15). See "Why the error estimate gets a looser
//!   bar" below.
//!
//! Bit-identity is counted for both. Two NaNs, or two equal infinities,
//! compare equal.
//!
//! # Why the error estimate gets a looser bar — and when that was decided
//!
//! **This criterion was set after the first run failed at 1e-12**, so the
//! reason is recorded rather than asserted. On the first run every status,
//! count and result agreed bit for bit, and one error estimate did not:
//! `peak_limit50`/`peak_limit500` gave `abserr` 7.969761e-8 against GSL's
//! 7.969750e-8 (1.4e-6 relative). The final `abserr` is QUADPACK's running
//! `errsum + error12 - e_i`, which here cancels from an initial estimate of
//! order 1e4 down to 1e-7, so a last-ulp difference in any per-interval error
//! is amplified about 1e11-fold. The per-interval error comes from
//! `rescale_error`'s `pow(200 err / resasc, 1.5)`, which petir evaluates with
//! the `libm` crate (a `no_std` lib build) and GSL with glibc.
//!
//! That mechanism was **tested, not assumed**: rebuilding GSL itself with
//! `nextafter(pow(...), +inf)` in `integration/err.c` (a one-ulp nudge, every
//! call) leaves every status, count and result unchanged and moves GSL's own
//! `abserr` by 1.4e-6 on `peak`, 4e-10 on `f1`, 5.9e-4 on `f11` and 1.6e-6 on
//! the 1.49e-8 kernel case. So `abserr` agreement past about 1e-3 is a
//! property of the libm, not of the port, which is what upstream's own test
//! tolerance says too.
//!
//! The cases reach every exit of upstream's `return_error` switch except
//! `GSL_EFAILED`, which no probed integrand reached: the first-rule return,
//! plain convergence, convergence by extrapolation (endpoint singularities
//! `x^2.6 ln(1/x)`, `1/sqrt(x)`, `ln x`, `x^-0.9`), the iteration limit,
//! `GSL_EDIVERGE`, `GSL_EROUND` (an interior `1/sqrt|x - 1/3|`), `GSL_ESING`
//! (a pole, whose result is `inf` and error `NaN`) and `GSL_EBADTOL`; plus the
//! finite-cloud point kernel of `buangkok`'s plume shine along one line,
//! at pyDOSEIA's tolerance 1.49e-3 and at 1.49e-8.
//!
//! # Results (2026-09-28)
//!
//! 21 cases: status, sub-interval count and evaluation count identical in all
//! 21; results **21 of 21 bit-identical**; error estimates 19 of 21
//! bit-identical, the other two (the same `peak` integrand at two limits)
//! 1.43e-6 relative. Interpretation: the port follows GSL's QAGS iterate for
//! iterate, including the `qpsrt` ordering, the round-off counters and the
//! `qelg` table shifts, on every exit the reference reaches; the only
//! difference is the libm `pow` ulp described above.

use std::fs;
use std::path::PathBuf;

use petir::integration::qags_with_status;
use petir::PetirError;

fn reference_path() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../reference-data/gsl/qags-gsl-2.8-reference.txt")
}

/// The GSL status code for a petir status.
fn gsl_status(s: Result<(), PetirError>) -> i64 {
    match s {
        Ok(()) => 0,
        Err(PetirError::Failed) => 5,
        Err(PetirError::MaxIterations) => 11,
        Err(PetirError::Tolerance) => 13,
        Err(PetirError::RoundOff) => 18,
        Err(PetirError::BadIntegrand) => 21,
        Err(PetirError::Diverged) => 22,
        Err(e) => panic!("unexpected qags error {e:?}"),
    }
}

/// The case's integrand and arguments, exactly as the C driver has them.
#[allow(clippy::type_complexity)]
fn case(name: &str) -> (fn(f64) -> f64, f64, f64, f64, f64, usize) {
    let kernel = |x: f64| {
        let r2 = (x - 500.0) * (x - 500.0) + 0.01;
        let r = r2.sqrt();
        (1.0 + 1.2 * 0.01 * r) / (4.0 * core::f64::consts::PI * r2) * (-0.01 * r).exp()
    };
    let runge = |x: f64| 1.0 / (1.0 + 25.0 * x * x);
    let f1 = |x: f64| x.powf(2.6) * (1.0 / x).ln();
    let peak = |x: f64| 1.0 / ((x - 0.3) * (x - 0.3) + 1e-8);
    let osc = |x: f64| (100.0 * x).cos() * x.ln();
    match name {
        "f1" => (f1, 0.0, 1.0, 0.0, 1e-10, 1000),
        "f1_reverse" => (f1, 1.0, 0.0, 0.0, 1e-10, 1000),
        "f11" => (
            |x| (1.0 / x).ln().powf(2.0 - 1.0),
            1.0,
            1000.0,
            1e-7,
            0.0,
            1000,
        ),
        "inv_sqrt" => (|x| 1.0 / x.sqrt(), 0.0, 1.0, 0.0, 1e-10, 200),
        "log_x" => (|x| x.ln(), 0.0, 1.0, 0.0, 1e-12, 200),
        "x_pow_m09" => (|x| x.powf(-0.9), 0.0, 1.0, 0.0, 1e-10, 200),
        "runge" => (runge, -1.0, 1.0, 0.0, 1e-10, 200),
        "sin_first_rule" => (|x| x.sin(), 0.0, 1.0, 1e-6, 1e-6, 50),
        "peak_limit50" => (peak, 0.0, 1.0, 0.0, 1e-10, 50),
        "peak_limit500" => (peak, 0.0, 1.0, 0.0, 1e-10, 500),
        "inv_x_divergent" => (|x| 1.0 / x, 0.0, 1.0, 0.0, 1e-10, 200),
        "osc_limit3" => (osc, 0.0, 1.0, 0.0, 1e-10, 3),
        "osc" => (osc, 0.0, 1.0, 0.0, 1e-10, 500),
        "kernel_1e-3" => (kernel, 400.0, 600.0, 1.49e-3, 1.49e-3, 50),
        "kernel_1e-8" => (kernel, 400.0, 600.0, 0.0, 1.49e-8, 50),
        "inv_x2_diverge" => (|x| 1.0 / (x * x), 0.0, 1.0, 0.0, 1e-6, 50),
        "sin_inv_x_diverge" => (|x| (1.0 / x).sin() / x, 0.0, 1.0, 0.0, 1e-6, 1000),
        "interior_sqrt_round" => (
            |x| {
                if x == 0.0 {
                    0.0
                } else {
                    1.0 / (x - 0.33333).abs().sqrt()
                }
            },
            0.0,
            1.0,
            0.0,
            1e-10,
            50,
        ),
        "pole_sing" => (|x| 1.0 / (x - 0.5), 0.0, 1.0, 0.0, 1e-10, 1000),
        "log_x_over_x_maxiter" => (|x| x.ln() / x, 0.0, 1.0, 0.0, 1e-10, 50),
        "badtol" => (runge, 0.0, 1.0, 0.0, 1e-14, 50),
        other => panic!("unknown case {other}"),
    }
}

fn same(a: f64, b: f64) -> Option<f64> {
    if a.to_bits() == b.to_bits() || (a.is_nan() && b.is_nan()) {
        return Some(0.0);
    }
    if !a.is_finite() || !b.is_finite() {
        return None;
    }
    Some(if b == 0.0 {
        a.abs()
    } else {
        ((a - b) / b).abs()
    })
}

#[test]
fn qags_matches_compiled_gsl() {
    let Ok(text) = fs::read_to_string(reference_path()) else {
        eprintln!("skipping: qags-gsl-2.8-reference.txt not present");
        return;
    };
    let (mut cases, mut values, mut identical) = (0, 0, 0);
    let (mut worst_result, mut worst_abserr) = (0.0_f64, 0.0_f64);
    let mut failures = Vec::new();
    for line in text.lines().filter(|l| l.starts_with("QAGS ")) {
        let f: Vec<&str> = line.split_whitespace().collect();
        let name = f[1];
        let status: i64 = f[2].parse().unwrap();
        let result: f64 = f[3].parse().unwrap();
        let abserr: f64 = f[4].parse().unwrap();
        let size: usize = f[5].parse().unwrap();
        let neval: usize = f[6].parse().unwrap();

        let (g, a, b, epsabs, epsrel, limit) = case(name);
        let n = std::cell::Cell::new(0usize);
        let o = qags_with_status(
            |x| {
                n.set(n.get() + 1);
                g(x)
            },
            a,
            b,
            epsabs,
            epsrel,
            limit,
        );
        cases += 1;
        if gsl_status(o.status) != status {
            failures.push(format!("{name}: status {:?}, GSL {status}", o.status));
        }
        if o.integral.subintervals != size {
            failures.push(format!(
                "{name}: size {}, GSL {size}",
                o.integral.subintervals
            ));
        }
        if n.get() != neval {
            failures.push(format!("{name}: neval {}, GSL {neval}", n.get()));
        }
        for (what, got, want, tol) in [
            ("result", o.integral.value, result, 1e-12),
            ("abserr", o.integral.abserr, abserr, 1e-3),
        ] {
            values += 1;
            match same(got, want) {
                Some(d) => {
                    if d == 0.0 {
                        identical += 1;
                    }
                    if what == "result" {
                        worst_result = worst_result.max(d);
                    } else {
                        worst_abserr = worst_abserr.max(d);
                    }
                    if d > tol {
                        failures.push(format!("{name} {what}: {got:e} vs GSL {want:e}"));
                    }
                }
                None => failures.push(format!("{name} {what}: {got:e} vs GSL {want:e}")),
            }
        }
    }
    println!(
        "qags: {cases} cases, {identical}/{values} values bit-identical, \
         worst rel diff result {worst_result:e}, abserr {worst_abserr:e}"
    );
    assert_eq!(cases, 21, "every reference case replayed");
    assert!(failures.is_empty(), "{failures:#?}");
}
