// SPDX-License-Identifier: GPL-3.0-only
//! **V&V: `petir::integration::qagp` against compiled GSL 2.8** (gh:#1340).
//! Methodology, reference and predictions (P1, P4):
//! `verification_and_validation/integration_family_gh1340.md`.
//!
//! Replays `reference-data/gsl/qagp-gsl-2.8-reference.txt` (31 cases, driver
//! `qagp_reference_driver.c`, workspace of exactly `limit` entries). GSL's
//! integer status, the workspace size and the number of integrand calls must
//! match **exactly**; result and abserr are counted bit-identical (two NaNs
//! count as identical) and the counts pinned.
//!
//! **Not discriminated, stated plainly:** qagp.c line 297 tests `i > 10` with
//! a stale loop index (`i == nint`), where qags.c tests the iteration count.
//! The port keeps upstream's `nint > 10`. A negative control (2026-11-13)
//! replaced it by `iteration > 10` and ALL 31 cases still passed, including 9
//! cases with 3 sub-ranges and 600 iterations and 4 noise-level integrands
//! chosen to provoke error increases. So this file does not pin that line; it
//! stands on reading `qagp.c`, and a case that does discriminate (one with
//! `roundoff_type3` reaching 20) has not been found.

use petir::integration::qagp_with_status;
use petir::PetirError;

const REFERENCE: &str = include_str!("../../../reference-data/gsl/qagp-gsl-2.8-reference.txt");

fn gsl_status(s: Result<(), PetirError>) -> i64 {
    match s {
        Ok(()) => 0,
        Err(PetirError::Invalid) => 4,
        Err(PetirError::MaxIterations) => 11,
        Err(PetirError::Tolerance) => 13,
        Err(PetirError::RoundOff) => 18,
        Err(PetirError::BadIntegrand) => 21,
        Err(PetirError::Diverged) => 22,
        Err(PetirError::Failed) => 5,
        Err(e) => panic!("unexpected error {e:?}"),
    }
}

fn same(a: f64, b: f64) -> bool {
    a.to_bits() == b.to_bits() || (a.is_nan() && b.is_nan())
}

type Case = (fn(f64) -> f64, Vec<f64>, f64, f64, usize);

fn case(name: &str) -> Case {
    let absk: fn(f64) -> f64 = |x| (x - 1.0).abs();
    let invx: fn(f64) -> f64 = |x| 1.0 / x;
    let pw09: fn(f64) -> f64 = |x| (x - 0.5).abs().powf(-0.9);
    let invsqx: fn(f64) -> f64 = |x| 1.0 / x.sqrt();
    let p12: Vec<f64> = (0..12).map(f64::from).collect();
    let cos50: fn(f64) -> f64 = |x| (50.0 * x).cos();
    match name {
        "abs_kink" => (absk, vec![0.0, 1.0, 3.0], 0.0, 1e-10, 50),
        "ln_abs" => (
            |x| (x - 1.0).abs().ln(),
            vec![0.0, 1.0, 2.0],
            0.0,
            1e-10,
            100,
        ),
        "inv_sqrt_abs" => (
            |x| 1.0 / (x - 1.0).abs().sqrt(),
            vec![0.0, 1.0, 3.0],
            0.0,
            1e-10,
            100,
        ),
        "step" => (
            |x| if x < 1.0 { 1.0 } else { 2.0 },
            vec![0.0, 1.0, 2.0],
            0.0,
            1e-10,
            50,
        ),
        "two_peaks" => (
            |x| 1.0 / (1e-4 + (x - 0.3) * (x - 0.3)) + 1.0 / (1e-4 + (x - 0.7) * (x - 0.7)),
            vec![0.0, 0.3, 0.7, 1.0],
            0.0,
            1e-8,
            200,
        ),
        "exp_12pts" => (|x| (-x).exp(), p12, 0.0, 1e-13, 100),
        "wavy_12pts" => (
            |x| (40.0 * x).cos() / ((x - 5.5).abs() + 1e-3).sqrt(),
            vec![0.0, 1.0, 2.0, 3.0, 4.0, 5.0, 6.0, 7.0, 8.0, 9.0, 10.0, 11.5],
            0.0,
            1e-9,
            400,
        ),
        "sharp_12pts" => (
            |x| (-1e4 * (x - 6.0) * (x - 6.0)).exp() + (-1e4 * (x - 9.0) * (x - 9.0)).exp(),
            p12,
            1e-12,
            1e-12,
            400,
        ),
        "pow09_limit4" => (pw09, vec![0.0, 0.5, 1.0], 0.0, 1e-10, 4),
        "pow09" => (pw09, vec![0.0, 0.5, 1.0], 0.0, 1e-8, 200),
        "pole_diverge" => (invx, vec![-1.0, 0.0, 1.0], 0.0, 1e-8, 100),
        "sqrt_abs" => (
            |x| (x - 1.0).abs().sqrt(),
            vec![0.0, 1.0, 2.0],
            0.0,
            1e-10,
            100,
        ),
        "cos50" => (cos50, vec![0.0, 1.0, 2.0], 0.0, 1e-10, 100),
        "cos50_limit5" => (cos50, vec![0.0, 1.0, 2.0], 0.0, 1e-13, 5),
        "inv_sqrt_x" => (invsqx, vec![0.0, 1.0], 0.0, 1e-10, 100),
        "inv_sqrt_x_tight" => (invsqx, vec![0.0, 1.0], 0.0, 1e-14, 30),
        "unordered" => (absk, vec![0.0, 2.0, 1.0], 0.0, 1e-10, 50),
        "npts_gt_limit" => (absk, vec![0.0, 1.0, 3.0], 0.0, 1e-10, 2),
        "badtol" => (absk, vec![0.0, 1.0, 3.0], 0.0, 1e-16, 50),
        "zero_width" => (absk, vec![0.0, 0.0, 1.0], 0.0, 1e-10, 50),
        "abs_only" => (absk, vec![0.0, 2.0], 1e-9, 0.0, 50),
        "limit2_two_pts" => (pw09, vec![0.0, 1.0], 0.0, 1e-12, 2),
        "fastosc" => (|x| (5000.0 * x).sin(), vec![0.0, 1.0, 2.0], 0.0, 1e-10, 600),
        "chirp" => (
            |x| (1e5 * x * x).sin(),
            vec![0.0, 1.0, 2.0],
            0.0,
            1e-10,
            600,
        ),
        "absosc" => (
            |x| (3000.0 * x).sin().abs(),
            vec![0.0, 1.0, 2.0],
            0.0,
            1e-10,
            600,
        ),
        "sininv" => (
            |x| (1.0 / (x + 1e-9)).sin(),
            vec![0.0, 1.0, 2.0],
            0.0,
            1e-8,
            600,
        ),
        "chirp_4pts" => (
            |x| (1e5 * x * x).sin(),
            vec![0.0, 0.3, 0.7, 1.0],
            0.0,
            1e-10,
            600,
        ),
        "noise1" => (
            |x| 1.0 + 1e-12 * (1e12 * x).sin(),
            vec![0.0, 1.0, 2.0],
            0.0,
            1e-13,
            600,
        ),
        "noise2" => (|x| (1e9 * x).sin(), vec![0.0, 1.0, 2.0], 0.0, 1e-10, 600),
        "saw" => (
            |x| 1e6 * x - (1e6 * x).floor(),
            vec![0.0, 1.0, 2.0],
            0.0,
            1e-10,
            600,
        ),
        "noise3" => (
            |x| (-x).exp() + 1e-13 * (7e13 * x).sin(),
            vec![0.0, 1.0, 2.0],
            0.0,
            1e-13,
            600,
        ),
        other => panic!("unknown case {other}"),
    }
}

#[test]
fn qagp_matches_compiled_gsl() {
    let (mut cases, mut res_exact, mut err_exact) = (0, 0, 0);
    for line in REFERENCE.lines().filter(|l| l.starts_with("QAGP ")) {
        let f: Vec<&str> = line.split_whitespace().collect();
        let name = f[1];
        let status: i64 = f[2].parse().unwrap();
        let result: f64 = f[3].parse().unwrap();
        let abserr: f64 = f[4].parse().unwrap();
        let size: usize = f[5].parse().unwrap();
        let neval: usize = f[6].parse().unwrap();
        let (g, pts, epsabs, epsrel, limit) = case(name);
        let n = std::cell::Cell::new(0usize);
        let h = |x: f64| {
            n.set(n.get() + 1);
            g(x)
        };
        let o = qagp_with_status(h, &pts, epsabs, epsrel, limit);
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
    println!("  qagp vs GSL 2.8: {cases} cases; results {res_exact}/{cases}; abserr {err_exact}/{cases} bit-identical");
    assert_eq!(cases, 31);
    assert_eq!(res_exact, 31, "results bit-identical (pinned)");
    assert_eq!(err_exact, 31, "abserr bit-identical (pinned)");
}
