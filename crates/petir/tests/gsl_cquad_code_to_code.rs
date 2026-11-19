// SPDX-License-Identifier: GPL-3.0-only
//! **V&V: `petir::integration::cquad` against compiled GSL 2.8** (gh:#1340,
//! round 3). Methodology, reference and predictions P14:
//! `verification_and_validation/integration_family_gh1340.md`.
//!
//! Replays `reference-data/gsl/cquad-gsl-2.8-reference.txt` (driver
//! `cquad_reference_driver.c`): the 25 test integrands of GSL's own
//! `integration/test.c` plus looser tolerances, small workspaces (intervals
//! dropped), non-finite integrands (the downdating path), divergent integrals
//! (`GSL_EDIVERGE`), refused tolerances and degenerate intervals. GSL's
//! status, `*nevals` and the number of integrand calls must match
//! **exactly**; the result and the error estimate are counted bit-identical.
//! `cquad.c` calls only `sqrt`, so no platform-libm allowance is expected; the
//! integrands call glibc in both programs.
//!
//! The 25 test integrands are also checked against GSL's own acceptance test
//! (`test.c:2437-2442`): relative error under 1e-12 and actual error under
//! 5 `abserr`.

use petir::integration::cquad_with_status;
use std::cell::Cell;
use std::f64::consts::PI;

const REFERENCE: &str = include_str!("../../../reference-data/gsl/cquad-gsl-2.8-reference.txt");

type Fx = fn(f64) -> f64;

fn cqf(i: usize) -> Fx {
    match i {
        1 => |x| x.exp(),
        2 => |x| f64::from(u8::from(x >= 0.3)),
        3 => |x| x.sqrt(),
        4 => |x| (23.0 / 25.0) * x.cosh() - x.cos(),
        5 => |x| {
            let x2 = x * x;
            1.0 / (x2 * (x2 + 1.0) + 0.9)
        },
        6 => |x| x * x.sqrt(),
        7 => |x| 1.0 / x.sqrt(),
        8 => |x| {
            let x2 = x * x;
            1.0 / (1.0 + x2 * x2)
        },
        9 => |x| 2.0 / (2.0 + (10.0 * PI * x).sin()),
        10 => |x| 1.0 / (1.0 + x),
        11 => |x| 1.0 / (1.0 + x.exp()),
        12 => |x| x / (x.exp() - 1.0),
        13 => |x| (100.0 * PI * x).sin() / (PI * x),
        14 => |x| 50.0_f64.sqrt() * (-50.0 * PI * x * x).exp(),
        15 => |x| 25.0 * (-25.0 * x).exp(),
        16 => |x| 50.0 / PI * (2500.0 * x * x + 1.0),
        17 => |x| {
            let t1 = 50.0 * PI * x;
            let t2 = t1.sin() / t1;
            50.0 * t2 * t2
        },
        18 => |x| {
            (x.cos() + 3.0 * x.sin() + 2.0 * (2.0 * x).cos() + 3.0 * (2.0 * x).sin()
                + 3.0 * (3.0 * x).cos())
            .cos()
        },
        19 => |x| x.ln(),
        20 => |x| 1.0 / (x * x + 1.005),
        21 => |x| {
            1.0 / (10.0 * (x - 0.2) * 2.0).cosh()
                + 1.0 / (100.0 * (x - 0.4) * 4.0).cosh()
                + 1.0 / (1000.0 * (x - 0.6) * 8.0).cosh()
        },
        22 => |x| 4.0 * PI * PI * x * (20.0 * PI * x).sin() * (2.0 * PI * x).cos(),
        23 => |x| {
            let t = 230.0 * x - 30.0;
            1.0 / (1.0 + t * t)
        },
        24 => |x| x.exp().floor(),
        25 => |x| {
            f64::from(u8::from(x < 1.0)) * (x + 1.0)
                + f64::from(u8::from(1.0 <= x && x <= 3.0)) * (3.0 - x)
                + f64::from(u8::from(x > 3.0)) * 2.0
        },
        other => panic!("no cqf{other}"),
    }
}

const RANGES: [(f64, f64); 25] = [
    (0.0, 1.0),
    (0.0, 1.0),
    (0.0, 1.0),
    (-1.0, 1.0),
    (-1.0, 1.0),
    (0.0, 1.0),
    (0.0, 1.0),
    (0.0, 1.0),
    (0.0, 1.0),
    (0.0, 1.0),
    (0.0, 1.0),
    (0.0, 1.0),
    (0.0, 1.0),
    (0.0, 10.0),
    (0.0, 10.0),
    (0.0, 10.0),
    (0.0, 1.0),
    (0.0, PI),
    (0.0, 1.0),
    (-1.0, 1.0),
    (0.0, 1.0),
    (0.0, 1.0),
    (0.0, 1.0),
    (0.0, 3.0),
    (0.0, 5.0),
];

const EXACT: [f64; 25] = [
    1.7182818284590452354,
    0.7,
    2.0 / 3.0,
    0.4794282266888016674,
    1.5822329637296729331,
    0.4,
    2.0,
    0.86697298733991103757,
    1.1547005383792515290,
    0.69314718055994530942,
    0.3798854930417224753,
    0.77750463411224827640,
    0.49898680869304550249,
    0.5,
    1.0,
    0.13263071079267703209e+08,
    0.49898680869304550249,
    0.83867634269442961454,
    -1.0,
    1.5643964440690497731,
    0.16349494301863722618,
    -0.63466518254339257343,
    0.013492485649467772692,
    17.664383539246514971,
    7.5,
];

/// `(integrand, a, b, epsabs, epsrel, size)` of a driver row.
fn case(name: &str) -> (Fx, f64, f64, f64, f64, usize) {
    if let Some(n) = name.strip_prefix('f') {
        if let Ok(i) = n.parse::<usize>() {
            let (a, b) = RANGES[i - 1];
            return (cqf(i), a, b, 0.0, 1.0e-12, 200);
        }
    }
    let sqrt_signed: Fx = |x| x.sqrt();
    let log_signed: Fx = |x| x.ln();
    let nan_band: Fx = |x| if (0.3..=0.35).contains(&x) { f64::NAN } else { x.exp() };
    let nan_mid: Fx = |x| if (x - 0.5).abs() < 0.2 { f64::NAN } else { x.exp() };
    let recip: Fx = |x| 1.0 / x;
    let cosh_peak: Fx = |x| 1.0 / (200.0 * (x - 0.123)).cosh();
    match name {
        "f1_1e-6" => (cqf(1), 0.0, 1.0, 0.0, 1e-6, 200),
        "f18_1e-8" => (cqf(18), 0.0, PI, 0.0, 1e-8, 200),
        "f13_1e-3" => (cqf(13), 0.0, 1.0, 0.0, 1e-3, 200),
        "f21_1e-14" => (cqf(21), 0.0, 1.0, 0.0, 1e-14, 200),
        "f15_abs" => (cqf(15), 0.0, 10.0, 1e-10, 0.0, 200),
        "f15_abs_rel" => (cqf(15), 0.0, 10.0, 1e-10, 1e-10, 200),
        "poly_exact" => (|x| x * x * x - x, -1.0, 2.0, 0.0, 1e-10, 50),
        "f21_ws3" => (cqf(21), 0.0, 1.0, 0.0, 1e-12, 3),
        "f21_ws4" => (cqf(21), 0.0, 1.0, 0.0, 1e-12, 4),
        "f21_ws5" => (cqf(21), 0.0, 1.0, 0.0, 1e-12, 5),
        "f21_ws8" => (cqf(21), 0.0, 1.0, 0.0, 1e-12, 8),
        "f13_ws5" => (cqf(13), 0.0, 1.0, 0.0, 1e-12, 5),
        "f7_ws3" => (cqf(7), 0.0, 1.0, 0.0, 1e-12, 3),
        "f23_ws10" => (cqf(23), 0.0, 1.0, 0.0, 1e-12, 10),
        "peak_ws6" => (cosh_peak, 0.0, 1.0, 0.0, 1e-12, 6),
        "sqrt_signed" => (sqrt_signed, -1.0, 1.0, 0.0, 1e-10, 200),
        "log_signed" => (log_signed, -1.0, 1.0, 0.0, 1e-10, 200),
        "nan_band" => (nan_band, 0.0, 1.0, 0.0, 1e-10, 200),
        "nan_mid" => (nan_mid, 0.0, 1.0, 0.0, 1e-8, 200),
        "inf_at_1" => (|x| 1.0 / (1.0 - x), 0.0, 1.0, 0.0, 1e-8, 200),
        "recip_0_1" => (recip, 0.0, 1.0, 0.0, 1e-9, 200),
        "recip2_0_1" => (|x| 1.0 / (x * x), 0.0, 1.0, 0.0, 1e-9, 200),
        "recip_sym" => (recip, -1.0, 1.0, 0.0, 1e-9, 200),
        "recip_pos" => (recip, 1.0, 10.0, 0.0, 1e-12, 200),
        "badtol_neg_abs" => (cqf(1), 0.0, 1.0, -1.0, 1e-9, 20),
        "badtol_neg_rel" => (cqf(1), 0.0, 1.0, 0.0, -1e-9, 20),
        "badtol_zero" => (cqf(1), 0.0, 1.0, 0.0, 0.0, 20),
        "badtol_tiny_rel" => (cqf(1), 0.0, 1.0, 0.0, 1e-17, 20),
        "ws2" => (cqf(1), 0.0, 1.0, 0.0, 1e-9, 2),
        "ws0" => (cqf(1), 0.0, 1.0, 0.0, 1e-9, 0),
        "reversed" => (cqf(1), 1.0, 0.0, 0.0, 1e-12, 200),
        "zero_width" => (cqf(1), 0.5, 0.5, 0.0, 1e-12, 200),
        "narrow" => (cqf(1), 1.0, 1.0 + 1e-15, 0.0, 1e-12, 200),
        "huge" => (cqf(1), 0.0, 1e6, 0.0, 1e-8, 200),
        other => panic!("unknown case {other}"),
    }
}

fn same(a: f64, b: f64) -> bool {
    a.to_bits() == b.to_bits() || (a.is_nan() && b.is_nan())
}

#[test]
fn cquad_matches_compiled_gsl() {
    let (mut rows, mut status_ok, mut ne_ok, mut calls_ok) = (0usize, 0usize, 0usize, 0usize);
    let (mut res_ok, mut err_ok, mut alloc_rows) = (0usize, 0usize, 0usize);
    let mut gsl_test_pass = 0usize;
    for line in REFERENCE.lines() {
        let f: Vec<&str> = line.split_whitespace().collect();
        match f[0] {
            "WSALLOC" => {
                // A workspace below 3 intervals is refused; the driver then
                // never calls cquad.
                let n: usize = f[1].parse().unwrap();
                assert!(n < 3 && f[2] == "0");
                let o = cquad_with_status(|x: f64| x, 0.0, 1.0, 0.0, 1e-9, n);
                assert_eq!(o.gsl_status, 1);
                alloc_rows += 1;
            }
            "CQUAD" => {
                let name = f[1];
                let status: i32 = f[2].parse().unwrap();
                let want = (f[3].parse::<f64>().unwrap(), f[4].parse::<f64>().unwrap());
                let (ne, want_calls): (usize, usize) = (f[5].parse().unwrap(), f[6].parse().unwrap());
                let (g, a, b, ea, er, size) = case(name);
                let calls = Cell::new(0usize);
                let o = cquad_with_status(
                    |x| {
                        calls.set(calls.get() + 1);
                        g(x)
                    },
                    a,
                    b,
                    ea,
                    er,
                    size,
                );
                rows += 1;
                if o.gsl_status == status {
                    status_ok += 1;
                } else {
                    println!("  {name}: status {} != GSL {status}", o.gsl_status);
                }
                if o.nevals == ne {
                    ne_ok += 1;
                } else {
                    println!("  {name}: nevals {} != GSL {ne}", o.nevals);
                }
                if calls.get() == want_calls {
                    calls_ok += 1;
                } else {
                    println!("  {name}: calls {} != GSL {want_calls}", calls.get());
                }
                if same(o.value, want.0) {
                    res_ok += 1;
                } else {
                    println!("  {name}: result {:e} vs GSL {:e}", o.value, want.0);
                }
                if same(o.abserr, want.1) {
                    err_ok += 1;
                } else {
                    println!("  {name}: abserr {:e} vs GSL {:e}", o.abserr, want.1);
                }
                // GSL's own acceptance test for the 25 integrands.
                if let Some(n) = name.strip_prefix('f').and_then(|n| n.parse::<usize>().ok()) {
                    let exact = EXACT[n - 1];
                    let rel = ((o.value - exact) / exact).abs();
                    if rel <= 1e-12 && (o.value - exact).abs() <= 5.0 * o.abserr && o.gsl_status == 0 {
                        gsl_test_pass += 1;
                    } else {
                        println!("  f{n}: GSL's test.c acceptance fails (rel {rel:e})");
                    }
                }
            }
            other => panic!("unknown row {other}"),
        }
    }
    println!(
        "CQUAD {rows} rows (+{alloc_rows} refused workspaces): status {status_ok} nevals {ne_ok} calls {calls_ok} result {res_ok} abserr {err_ok}; test.c acceptance {gsl_test_pass}/25"
    );
    assert_eq!(rows, 57);
    assert_eq!(alloc_rows, 2);
    // Pass criteria fixed before the run (P14).
    assert_eq!((status_ok, ne_ok, calls_ok), (rows, rows, rows));
    assert_eq!((res_ok, err_ok), (rows, rows));
    assert_eq!(gsl_test_pass, 25);
}
