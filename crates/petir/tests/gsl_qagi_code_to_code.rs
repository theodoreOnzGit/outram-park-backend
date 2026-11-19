// SPDX-License-Identifier: GPL-3.0-only
//! **V&V: `petir::integration::{qagi, qagiu, qagil}` against compiled GSL 2.8**
//! (gh:#1340). Methodology, reference and predictions P1-P3:
//! `verification_and_validation/qagi_gh1340.md`.
//!
//! Replays `reference-data/gsl/qagi-gsl-2.8-reference.txt` (18 cases). Status,
//! workspace size and evaluation count must match exactly; the result and the
//! error estimate are counted bit-identical and bounded at 1e-12 / 1e-3
//! relative (the QAGS test's bars for a libm-only difference). Results are
//! recorded in the V&V record.

use petir::integration::{qagi_with_status, qagil_with_status, qagiu_with_status};
use petir::PetirError;

const REFERENCE: &str = include_str!("../../../reference-data/gsl/qagi-gsl-2.8-reference.txt");

fn gsl_status(s: Result<(), PetirError>) -> i64 {
    match s {
        Ok(()) => 0,
        Err(PetirError::MaxIterations) => 11,
        Err(PetirError::Tolerance) => 13,
        Err(PetirError::RoundOff) => 18,
        Err(PetirError::BadIntegrand) => 21,
        Err(PetirError::Diverged) => 22,
        Err(e) => panic!("unexpected error {e:?}"),
    }
}

/// The integrand, bound and tolerances exactly as the C driver has them.
#[allow(clippy::type_complexity)]
fn case(name: &str) -> (fn(f64) -> f64, f64, f64, f64, usize) {
    match name {
        "gauss" => (|x| (-x * x).exp(), 0.0, 0.0, 1e-10, 200),
        "cauchy" => (|x| 1.0 / (1.0 + x * x), 0.0, 0.0, 1e-10, 200),
        "quartic" => (|x| 1.0 / (1.0 + x * x * x * x), 0.0, 0.0, 1e-12, 200),
        "peak30" => (
            |x| 1.0 / (1.0 + (x - 30.0) * (x - 30.0) * 100.0),
            0.0,
            0.0,
            1e-8,
            500,
        ),
        "peak30_limit3" => (
            |x| 1.0 / (1.0 + (x - 30.0) * (x - 30.0) * 100.0),
            0.0,
            0.0,
            1e-12,
            3,
        ),
        "inv_sqrt1_diverge" => (|x| 1.0 / (1.0 + x * x).sqrt(), 0.0, 0.0, 1e-8, 100),
        "cauchy_badtol" => (|x| 1.0 / (1.0 + x * x), 0.0, 0.0, 1e-14, 50),
        "inv_x2_from1" => (|x| 1.0 / (x * x), 1.0, 0.0, 1e-10, 200),
        "x_m15_from1" => (|x| 1.0 / (x * x.sqrt()), 1.0, 0.0, 1e-10, 200),
        "x_m11_from1" => (|x| x.powf(-1.1), 1.0, 0.0, 1e-6, 500),
        "exp_from2" => (|x| (-x).exp(), 2.0, 0.0, 1e-10, 200),
        "cauchy_from0" => (|x| 1.0 / (1.0 + x * x), 0.0, 1e-12, 0.0, 200),
        "inv_x_from1_diverge" => (|x| 1.0 / x, 1.0, 0.0, 1e-8, 100),
        "ln_x2_from1" => (|x| x.ln() / (x * x), 1.0, 0.0, 1e-10, 200),
        "exp_to_m1" => (|x| x.exp(), -1.0, 0.0, 1e-10, 200),
        "cauchy_to0" => (|x| 1.0 / (1.0 + x * x), 0.0, 0.0, 1e-10, 200),
        "inv_x2_to_m1" => (|x| 1.0 / (x * x), -1.0, 0.0, 1e-10, 200),
        "gauss_to_m3" => (|x| (-x * x).exp(), -3.0, 1e-9, 1e-9, 100),
        other => panic!("unknown case {other}"),
    }
}

fn rel(a: f64, b: f64) -> f64 {
    if a.to_bits() == b.to_bits() {
        0.0
    } else if b == 0.0 {
        a.abs()
    } else {
        ((a - b) / b).abs()
    }
}

#[test]
fn qagi_family_matches_compiled_gsl() {
    let (mut cases, mut res_exact, mut err_exact) = (0, 0, 0);
    for line in REFERENCE.lines().filter(|l| l.starts_with("QAGI ")) {
        let f: Vec<&str> = line.split_whitespace().collect();
        let (name, kind) = (f[1], f[2]);
        let status: i64 = f[3].parse().unwrap();
        let result: f64 = f[4].parse().unwrap();
        let abserr: f64 = f[5].parse().unwrap();
        let size: usize = f[6].parse().unwrap();
        let neval: usize = f[7].parse().unwrap();
        let (g, lim, epsabs, epsrel, limit) = case(name);
        let n = std::cell::Cell::new(0usize);
        let h = |x: f64| {
            n.set(n.get() + 1);
            g(x)
        };
        let o = match kind {
            "i" => qagi_with_status(h, epsabs, epsrel, limit),
            "u" => qagiu_with_status(h, lim, epsabs, epsrel, limit),
            _ => qagil_with_status(h, lim, epsabs, epsrel, limit),
        };
        cases += 1;
        assert_eq!(gsl_status(o.status), status, "{name} status");
        assert_eq!(o.integral.subintervals, size, "{name} size");
        assert_eq!(n.get(), neval, "{name} evaluations");
        assert!(rel(o.integral.value, result) <= 1e-12, "{name} result");
        assert!(rel(o.integral.abserr, abserr) <= 1e-3, "{name} abserr");
        if o.integral.value.to_bits() == result.to_bits() {
            res_exact += 1;
        } else {
            println!("  {name}: result {:e} vs GSL {result:e}", o.integral.value);
        }
        if o.integral.abserr.to_bits() == abserr.to_bits() {
            err_exact += 1;
        } else {
            println!("  {name}: abserr {:e} vs GSL {abserr:e}", o.integral.abserr);
        }
    }
    println!(
        "  qagi family vs GSL 2.8: {cases} cases; results {res_exact}/{cases} bit-identical; \
         abserr {err_exact}/{cases} bit-identical"
    );
    assert_eq!(cases, 18);
}
