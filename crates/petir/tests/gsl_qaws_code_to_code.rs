// SPDX-License-Identifier: GPL-3.0-only
//! **V&V: `petir::integration::{QawsTable, qaws}` against compiled GSL 2.8**
//! (gh:#1340). Methodology, reference and prediction P10:
//! `verification_and_validation/integration_family_gh1340.md`.
//!
//! Replays `reference-data/gsl/qaws-gsl-2.8-reference.txt` (driver
//! `qaws_reference_driver.c`, workspace of exactly `limit` entries): 12 table
//! parameter sets (8 accepted, 4 refused), the Chebyshev moment arrays of the
//! 8 accepted tables, and 17 integrations. GSL's integer status, workspace
//! size and `neval` must match **exactly**; moments, results and abserr are
//! counted bit-identical and the counts pinned. The weights call `pow` and
//! `log`, which are glibc in GSL and PETIR's in the port.

use petir::integration::{qaws_with_status, QawsTable};
use petir::PetirError;

const REFERENCE: &str = include_str!("../../../reference-data/gsl/qaws-gsl-2.8-reference.txt");

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

type Case = (
    fn(f64) -> f64,
    f64,
    f64,
    (f64, f64, i32, i32),
    f64,
    f64,
    usize,
);

fn case(name: &str) -> Case {
    let one: fn(f64) -> f64 = |_| 1.0;
    let cos10: fn(f64) -> f64 = |x| (10.0 * x).cos();
    let expx: fn(f64) -> f64 = |x| x.exp();
    match name {
        "log_only" => (one, 0.0, 1.0, (0.0, 0.0, 1, 0), 0.0, 1e-10, 100),
        "sqrt_ends" => (one, 0.0, 1.0, (-0.5, -0.5, 0, 0), 0.0, 1e-9, 100),
        "alg_left" => (|x| x.cos(), 0.0, 2.0, (-0.5, 0.0, 0, 0), 0.0, 1e-10, 100),
        "alg_right" => (expx, 0.0, 1.0, (0.0, -0.7, 0, 0), 0.0, 1e-10, 100),
        "both_log" => (
            |x| 1.0 / (1.0 + x),
            0.0,
            3.0,
            (0.5, 0.5, 1, 1),
            0.0,
            1e-10,
            100,
        ),
        "nu_only" => (|x| x, 0.5, 2.0, (0.0, 0.0, 0, 1), 0.0, 1e-10, 100),
        "plain" => (
            |x| (5.0 * x).sin(),
            0.0,
            2.0,
            (0.0, 0.0, 0, 0),
            0.0,
            1e-10,
            100,
        ),
        "mix" => (
            |x| (-x).exp(),
            -1.0,
            1.0,
            (1.5, -0.3, 1, 0),
            0.0,
            1e-10,
            100,
        ),
        "hard" => (
            |x| 1.0 + x * x,
            0.0,
            1.0,
            (-0.9, -0.9, 0, 0),
            0.0,
            1e-12,
            200,
        ),
        "limit3" => (cos10, 0.0, 1.0, (-0.5, -0.5, 0, 0), 0.0, 1e-12, 3),
        "limit4" => (cos10, 0.0, 1.0, (-0.5, -0.5, 0, 0), 0.0, 1e-12, 4),
        "limit5" => (cos10, 0.0, 1.0, (-0.5, -0.5, 0, 0), 0.0, 1e-12, 5),
        "abs_only" => (expx, 0.0, 1.0, (-0.5, 0.0, 0, 0), 1e-8, 0.0, 100),
        "b_lt_a" => (one, 1.0, 0.0, (0.0, 0.0, 0, 0), 0.0, 1e-8, 10),
        "b_eq_a" => (one, 1.0, 1.0, (0.0, 0.0, 0, 0), 0.0, 1e-8, 10),
        "badtol" => (one, 0.0, 1.0, (0.0, 0.0, 0, 0), 0.0, 1e-17, 10),
        "both_sing_hard" => (cos10, 0.0, 3.0, (-0.95, -0.95, 1, 1), 0.0, 1e-13, 300),
        other => panic!("unknown case {other}"),
    }
}

#[test]
fn qaws_matches_compiled_gsl() {
    let (mut terr, mut tables, mut mom, mut mom_exact) = (0, 0, 0, 0);
    let (mut cases, mut res_exact, mut err_exact) = (0, 0, 0);
    for line in REFERENCE.lines() {
        let f: Vec<&str> = line.split_whitespace().collect();
        match f[0] {
            "TERR" => {
                let (alpha, beta): (f64, f64) = (f[1].parse().unwrap(), f[2].parse().unwrap());
                let (mu, nu): (i32, i32) = (f[3].parse().unwrap(), f[4].parse().unwrap());
                let ok = f[5] == "1";
                assert_eq!(QawsTable::new(alpha, beta, mu, nu).is_ok(), ok, "{line}");
                terr += 1;
            }
            "TBL" => {
                let (alpha, beta): (f64, f64) = (f[1].parse().unwrap(), f[2].parse().unwrap());
                let (mu, nu): (i32, i32) = (f[3].parse().unwrap(), f[4].parse().unwrap());
                let want: Vec<f64> = f[5..].iter().map(|s| s.parse().unwrap()).collect();
                assert_eq!(want.len(), 100);
                let t = QawsTable::new(alpha, beta, mu, nu).unwrap();
                let (ri, rj, rg, rh) = t.moments();
                let got = ri.iter().chain(rj).chain(rg).chain(rh);
                let mut row_exact = 0;
                for (g, w) in got.zip(want.iter()) {
                    mom += 1;
                    if g.to_bits() == w.to_bits() {
                        mom_exact += 1;
                        row_exact += 1;
                    }
                }
                println!("  table ({alpha}, {beta}, {mu}, {nu}): moments {row_exact}/100");
                tables += 1;
            }
            "QAWS" => {
                let name = f[1];
                let status: i64 = f[2].parse().unwrap();
                let result: f64 = f[3].parse().unwrap();
                let abserr: f64 = f[4].parse().unwrap();
                let size: usize = f[5].parse().unwrap();
                let neval: usize = f[6].parse().unwrap();
                let (g, a, b, (alpha, beta, mu, nu), epsabs, epsrel, limit) = case(name);
                let t = QawsTable::new(alpha, beta, mu, nu).unwrap();
                let n = std::cell::Cell::new(0usize);
                let h = |x: f64| {
                    n.set(n.get() + 1);
                    g(x)
                };
                let o = qaws_with_status(h, a, b, &t, epsabs, epsrel, limit);
                cases += 1;
                assert_eq!(gsl_status(o.status), status, "{name} status");
                assert_eq!(o.integral.subintervals, size, "{name} size");
                assert_eq!(n.get(), neval, "{name} evaluations");
                if same(o.integral.value, result) {
                    res_exact += 1;
                } else {
                    println!("  {name}: result {:e} vs GSL {result:e}", o.integral.value);
                    assert!(
                        ((o.integral.value - result) / result).abs() < 1e-12,
                        "{name} result"
                    );
                }
                if same(o.integral.abserr, abserr) {
                    err_exact += 1;
                } else {
                    println!("  {name}: abserr {:e} vs GSL {abserr:e}", o.integral.abserr);
                    // abserr is a difference of large numbers and can sit at the
                    // rounding floor, so a last-bit libm difference moves it by
                    // percents (`mix`: 4.7 %); only its order is checked here, and
                    // the shim control (see the V&V record) shows it identical.
                    let ratio = o.integral.abserr / abserr;
                    assert!((0.5..=2.0).contains(&ratio), "{name} abserr");
                }
            }
            _ => {}
        }
    }
    println!(
        "  qaws vs GSL 2.8: {terr} TERR rows; {tables} tables, moments {mom_exact}/{mom}; \
         {cases} cases, results {res_exact}/{cases}, abserr {err_exact}/{cases} bit-identical"
    );
    assert_eq!((terr, tables, cases), (12, 8, 17));
    // Pinned, measured 2026-11-13 against stock GSL (glibc `pow`/`log`). The
    // prediction that abserr agrees to 1e-6 FAILED (both_log 8.9e-6, mix 4.7 %).
    // DEMONSTRATED to be libm only: GSL's driver relinked with PETIR's `pow`
    // and `log` (diagnostic shim, not committed) is 800/800 moments, 17/17
    // results and 17/17 abserr bit-identical to this port.
    assert_eq!((mom_exact, mom), (675, 800));
    assert_eq!((res_exact, err_exact), (13, 9));
}
