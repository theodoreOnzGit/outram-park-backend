// SPDX-License-Identifier: GPL-3.0-only
//! **V&V: `petir::integration::qng` against compiled GSL 2.8** (gh:#1340).
//! Methodology, reference and predictions P1-P2:
//! `verification_and_validation/integration_family_gh1340.md`.
//!
//! Replays `reference-data/gsl/qng-gsl-2.8-reference.txt` (27 cases, driver
//! `qng_reference_driver.c`). GSL's integer status, `*neval`, and the count
//! of integrand calls must match **exactly**; result and abserr are counted
//! bit-identical and the counts pinned. The integrands are the driver's, and
//! this is a `std` build, so `exp`/`cos`/`log` reach the same glibc as the
//! driver; the only libm-dependent step inside PETIR is `pow(x, 1.5)` in
//! `rescale_error`.

use petir::integration::qng_with_status;

const REFERENCE: &str = include_str!("../../../reference-data/gsl/qng-gsl-2.8-reference.txt");

type Case = (fn(f64) -> f64, f64, f64, f64, f64);

fn case(name: &str) -> Case {
    let cubic: fn(f64) -> f64 = |x| x * x * x;
    let expx: fn(f64) -> f64 = |x| x.exp();
    let cos10: fn(f64) -> f64 = |x| (10.0 * x).cos();
    let sqrtx: fn(f64) -> f64 = |x| x.sqrt();
    let gauss: fn(f64) -> f64 = |x| (-x * x).exp();
    let runge: fn(f64) -> f64 = |x| 1.0 / (1.0 + 25.0 * x * x);
    let sin5: fn(f64) -> f64 = |x| (5.0 * x).sin() * x;
    match name {
        "cubic" => (cubic, 0.0, 2.0, 0.0, 1e-10),
        "exp_1e10" => (expx, 0.0, 1.0, 0.0, 1e-10),
        "exp_1e13" => (expx, 0.0, 1.0, 0.0, 1e-13),
        "exp_1e15" => (expx, 0.0, 1.0, 0.0, 1e-15),
        "exp_abs" => (expx, 0.0, 1.0, 1e-9, 0.0),
        "cos10_1e8" => (cos10, 0.0, 1.0, 0.0, 1e-8),
        "cos10_1e12" => (cos10, 0.0, 1.0, 0.0, 1e-12),
        "cos10_1e6" => (cos10, 0.0, 1.0, 0.0, 1e-6),
        "sqrt_1e14" => (sqrtx, 0.0, 1.0, 0.0, 1e-14),
        "sqrt_1e4" => (sqrtx, 0.0, 1.0, 0.0, 1e-4),
        "invsqrt_1e8" => (|x| 1.0 / x.sqrt(), 0.0, 1.0, 0.0, 1e-8),
        "gauss_m3_3" => (gauss, -3.0, 3.0, 0.0, 1e-9),
        "gauss_m3_3_1e12" => (gauss, -3.0, 3.0, 1e-12, 1e-12),
        "runge" => (runge, -1.0, 1.0, 0.0, 1e-8),
        "runge_1e3" => (runge, -1.0, 1.0, 0.0, 1e-3),
        "kink" => (|x| (x - 0.3).abs(), 0.0, 1.0, 0.0, 1e-8),
        "log_0_1" => (|x| x.ln(), 0.0, 1.0, 0.0, 1e-6),
        "log_1_2" => (|x| x.ln(), 1.0, 2.0, 0.0, 1e-12),
        "reversed" => (expx, 1.0, 0.0, 0.0, 1e-10),
        "zero_length" => (expx, 0.5, 0.5, 0.0, 1e-10),
        "const_one" => (|_| 1.0, 0.0, 3.0, 0.0, 1e-10),
        "xsin5" => (sin5, 0.0, 4.0, 0.0, 1e-10),
        "xsin5_wide" => (sin5, -10.0, 10.0, 1e-3, 0.0),
        "badtol_zero" => (cubic, 0.0, 1.0, 0.0, 0.0),
        "badtol_tiny" => (cubic, 0.0, 1.0, 0.0, 1e-15),
        "badtol_neg" => (cubic, 0.0, 1.0, -1.0, 1e-16),
        "tol_abs_only" => (cubic, 0.0, 1.0, 1e-8, 0.0),
        other => panic!("unknown case {other}"),
    }
}

#[test]
fn qng_matches_compiled_gsl() {
    let (mut cases, mut res_exact, mut err_exact) = (0, 0, 0);
    for line in REFERENCE.lines().filter(|l| l.starts_with("QNG ")) {
        let f: Vec<&str> = line.split_whitespace().collect();
        let name = f[1];
        let status: i32 = f[2].parse().unwrap();
        let result: f64 = f[3].parse().unwrap();
        let abserr: f64 = f[4].parse().unwrap();
        let neval: usize = f[5].parse().unwrap();
        let counted: usize = f[6].parse().unwrap();
        let (g, a, b, epsabs, epsrel) = case(name);
        let n = std::cell::Cell::new(0usize);
        let h = |x: f64| {
            n.set(n.get() + 1);
            g(x)
        };
        let o = qng_with_status(h, a, b, epsabs, epsrel);
        cases += 1;
        assert_eq!(o.gsl_status, status, "{name} status");
        assert_eq!(o.neval, neval, "{name} neval");
        assert_eq!(n.get(), counted, "{name} integrand calls");
        if o.value.to_bits() == result.to_bits() {
            res_exact += 1;
        } else {
            println!("  {name}: result {:e} vs GSL {result:e}", o.value);
        }
        if o.abserr.to_bits() == abserr.to_bits() {
            err_exact += 1;
        } else {
            println!("  {name}: abserr {:e} vs GSL {abserr:e}", o.abserr);
        }
    }
    println!("  qng vs GSL 2.8: {cases} cases; results {res_exact}/{cases}; abserr {err_exact}/{cases} bit-identical");
    assert_eq!(cases, 27);
    assert_eq!(res_exact, 27, "results bit-identical (pinned)");
    // One miss, `log_0_1`, 1 ulp in abserr (3.599909489090861e-3 against GSL's
    // 3.5999094890908604e-3). DEMONSTRATED to be `pow` in `rescale_error`:
    // GSL's driver relinked with PETIR's `pow` alone (a diagnostic shim, not
    // committed) prints 3.59990948909086079e-03, PETIR's value. Accepted per
    // AGENTS.md ("a 1-ulp difference from the platform libm"); pinned.
    assert_eq!(err_exact, 26, "abserr bit-identical (pinned)");
}
