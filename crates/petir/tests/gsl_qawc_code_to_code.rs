// SPDX-License-Identifier: GPL-3.0-only
//! **V&V: `petir::integration::qawc` against compiled GSL 2.8** (gh:#1340).
//! Methodology, reference and prediction P5:
//! `verification_and_validation/integration_family_gh1340.md`.
//!
//! Replays `reference-data/gsl/qawc-gsl-2.8-reference.txt` (22 cases, driver
//! `qawc_reference_driver.c`, workspace of exactly `limit` entries). GSL's
//! integer status, the workspace size and the number of integrand calls must
//! match **exactly**; result and abserr are counted bit-identical and the
//! counts pinned. Covers both branches of `qc25c` (Clenshaw-Curtis with the
//! Cauchy moments for `|cc| <= 1.1`, the weighted 15-point rule otherwise),
//! reversed limits, a roundoff status (`sqabs_cusp`), an iteration-limit
//! status, and the argument errors.

use petir::integration::qawc_with_status;
use petir::PetirError;

const REFERENCE: &str = include_str!("../../../reference-data/gsl/qawc-gsl-2.8-reference.txt");

fn gsl_status(s: Result<(), PetirError>) -> i64 {
    match s {
        Ok(()) => 0,
        Err(PetirError::Invalid) => 4,
        Err(PetirError::MaxIterations) => 11,
        Err(PetirError::Tolerance) => 13,
        Err(PetirError::RoundOff) => 18,
        Err(PetirError::BadIntegrand) => 21,
        Err(PetirError::Failed) => 5,
        Err(e) => panic!("unexpected error {e:?}"),
    }
}

fn same(a: f64, b: f64) -> bool {
    a.to_bits() == b.to_bits() || (a.is_nan() && b.is_nan())
}

/// `(f, a, b, c, epsabs, epsrel, limit)` as the C driver has them.
type Case = (fn(f64) -> f64, f64, f64, f64, f64, f64, usize);

fn case(name: &str) -> Case {
    let one: fn(f64) -> f64 = |_| 1.0;
    let expx: fn(f64) -> f64 = |x| x.exp();
    let rough: fn(f64) -> f64 = |x| (200.0 * x).sin() + 2.0;
    match name {
        "one_centre" => (one, -1.0, 2.0, 0.0, 0.0, 1e-10, 100),
        "one_reversed" => (one, 2.0, -1.0, 0.0, 0.0, 1e-10, 100),
        "one_outside" => (one, 0.0, 1.0, 5.0, 0.0, 1e-10, 100),
        "one_near" => (one, 0.0, 1.0, 1.05, 0.0, 1e-10, 100),
        "lin_centre" => (|x| x, -1.0, 1.0, 0.3, 0.0, 1e-10, 100),
        "exp_centre" => (expx, 0.0, 2.0, 0.7, 0.0, 1e-10, 100),
        "exp_off_centre" => (expx, 0.0, 2.0, 1.99, 0.0, 1e-10, 100),
        "cos3" => (|x| (3.0 * x).cos(), -2.0, 3.0, 0.1, 0.0, 1e-8, 100),
        "sqabs_cusp" => (|x| x.abs().sqrt(), -1.0, 1.0, 0.0, 0.0, 1e-8, 100),
        "sqabs_off" => (|x| x.abs().sqrt(), -1.0, 1.0, 0.3, 0.0, 1e-12, 100),
        "gauss_wide" => (|x| (-x * x).exp(), -5.0, 5.0, 1.0, 0.0, 1e-10, 100),
        "peak_near_c" => (
            |x| 1.0 / (1e-3 + (x - 0.4) * (x - 0.4)),
            0.0,
            1.0,
            0.45,
            0.0,
            1e-8,
            200,
        ),
        "kink_at_c" => (|x| (x - 0.25).abs(), 0.0, 1.0, 0.25, 0.0, 1e-9, 100),
        "kink_off_c" => (|x| (x - 0.25).abs(), 0.0, 1.0, 0.6, 0.0, 1e-12, 100),
        "rough_limit3" => (rough, 0.0, 2.0, 0.7, 0.0, 1e-12, 3),
        "rough" => (rough, 0.0, 2.0, 0.7, 0.0, 1e-9, 400),
        "limit1" => (expx, 0.0, 2.0, 0.7, 0.0, 1e-14, 1),
        "abs_only" => (expx, 0.0, 2.0, 0.7, 1e-8, 0.0, 100),
        "c_eq_a" => (expx, 0.0, 2.0, 0.0, 0.0, 1e-8, 100),
        "c_eq_b" => (expx, 0.0, 2.0, 2.0, 0.0, 1e-8, 100),
        "badtol" => (expx, 0.0, 2.0, 0.7, 0.0, 1e-17, 100),
        "tight_roundoff" => (expx, 0.0, 2.0, 0.7, 0.0, 1e-15, 200),
        other => panic!("unknown case {other}"),
    }
}

#[test]
fn qawc_matches_compiled_gsl() {
    let (mut cases, mut res_exact, mut err_exact) = (0, 0, 0);
    for line in REFERENCE.lines().filter(|l| l.starts_with("QAWC ")) {
        let f: Vec<&str> = line.split_whitespace().collect();
        let name = f[1];
        let status: i64 = f[2].parse().unwrap();
        let result: f64 = f[3].parse().unwrap();
        let abserr: f64 = f[4].parse().unwrap();
        let size: usize = f[5].parse().unwrap();
        let neval: usize = f[6].parse().unwrap();
        let (g, a, b, c, epsabs, epsrel, limit) = case(name);
        let n = std::cell::Cell::new(0usize);
        let h = |x: f64| {
            n.set(n.get() + 1);
            g(x)
        };
        let o = qawc_with_status(h, a, b, c, epsabs, epsrel, limit);
        cases += 1;
        assert_eq!(gsl_status(o.status), status, "{name} status");
        assert_eq!(o.integral.subintervals, size, "{name} size");
        assert_eq!(n.get(), neval, "{name} evaluations");
        if same(o.integral.value, result) {
            res_exact += 1;
        } else {
            println!("  {name}: result {:e} vs GSL {result:e}", o.integral.value);
        }
        if same(o.integral.abserr, abserr) {
            err_exact += 1;
        } else {
            println!("  {name}: abserr {:e} vs GSL {abserr:e}", o.integral.abserr);
        }
    }
    println!("  qawc vs GSL 2.8: {cases} cases; results {res_exact}/{cases}; abserr {err_exact}/{cases} bit-identical");
    assert_eq!(cases, 22);
    assert_eq!(res_exact, 22, "results bit-identical (pinned)");
    assert_eq!(err_exact, 22, "abserr bit-identical (pinned)");
}
