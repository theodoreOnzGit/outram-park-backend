// SPDX-License-Identifier: GPL-3.0-only
//! **V&V: `petir::integration::romberg` against compiled GSL 2.8**
//! (gh:#1340). Methodology, reference and prediction P6:
//! `verification_and_validation/integration_family_gh1340.md`.
//!
//! Replays `reference-data/gsl/romberg-gsl-2.8-reference.txt` (20 cases,
//! driver `romberg_reference_driver.c`). GSL's integer status (`GSL_EDOM` 1,
//! `GSL_EMAXITER` 11), `*neval` and the integrand call count must match
//! exactly; the result is counted bit-identical and the count pinned.

use petir::integration::romberg_with_status;
use petir::PetirError;

const REFERENCE: &str = include_str!("../../../reference-data/gsl/romberg-gsl-2.8-reference.txt");

fn gsl_status(s: Result<(), PetirError>) -> i64 {
    match s {
        Ok(()) => 0,
        Err(PetirError::Domain) => 1,
        Err(PetirError::MaxIterations) => 11,
        Err(e) => panic!("unexpected error {e:?}"),
    }
}

/// `(f, a, b, epsabs, epsrel, n)` as the C driver has them.
type Case = (fn(f64) -> f64, f64, f64, f64, f64, usize);

fn case(name: &str) -> Case {
    let sq: fn(f64) -> f64 = |x| x * x;
    let expx: fn(f64) -> f64 = |x| x.exp();
    let runge: fn(f64) -> f64 = |x| 1.0 / (1.0 + 25.0 * x * x);
    let kink: fn(f64) -> f64 = |x| (x - 0.3).abs();
    match name {
        "sq" => (sq, 0.0, 3.0, 0.0, 1e-12, 10),
        "exp" => (expx, 0.0, 1.0, 0.0, 1e-12, 20),
        "exp_abs" => (expx, 0.0, 1.0, 1e-8, 0.0, 20),
        "sin" => (|x| x.sin(), 0.0, 3.14159265358979323846, 0.0, 1e-14, 20),
        "gauss" => (|x| (-x * x).exp(), -3.0, 3.0, 0.0, 1e-13, 25),
        "runge" => (runge, -1.0, 1.0, 0.0, 1e-10, 30),
        "runge_n6" => (runge, -1.0, 1.0, 0.0, 1e-10, 6),
        "kink_n6" => (kink, 0.0, 1.0, 0.0, 1e-14, 6),
        "kink_n12" => (kink, 0.0, 1.0, 0.0, 1e-8, 12),
        "sqrt_n14" => (|x| x.sqrt(), 0.0, 1.0, 0.0, 1e-9, 14),
        "poly8" => (|x| x.powf(8.0) - x, -1.0, 2.0, 0.0, 1e-13, 12),
        "osc" => (|x| (30.0 * x).cos(), 0.0, 5.0, 0.0, 1e-10, 16),
        "reversed" => (expx, 1.0, 0.0, 0.0, 1e-12, 20),
        "zero_width" => (expx, 0.5, 0.5, 1e-12, 0.0, 10),
        "n1" => (sq, 0.0, 1.0, 0.0, 1e-8, 1),
        "n2" => (sq, 0.0, 1.0, 0.0, 1e-8, 2),
        "n31_converges" => (expx, 0.0, 1.0, 0.0, 1e-12, 31),
        "eps_zero_n8" => (expx, 0.0, 1.0, 0.0, 0.0, 8),
        "neg_epsabs" => (sq, 0.0, 1.0, -1.0, 0.0, 5),
        "neg_epsrel" => (sq, 0.0, 1.0, 0.0, -1.0, 5),
        other => panic!("unknown case {other}"),
    }
}

#[test]
fn romberg_matches_compiled_gsl() {
    let (mut cases, mut res_exact) = (0, 0);
    for line in REFERENCE.lines().filter(|l| l.starts_with("ROMBERG ")) {
        let f: Vec<&str> = line.split_whitespace().collect();
        let name = f[1];
        let status: i64 = f[2].parse().unwrap();
        let result: f64 = f[3].parse().unwrap();
        let neval: usize = f[4].parse().unwrap();
        let counted: usize = f[5].parse().unwrap();
        let (g, a, b, epsabs, epsrel, n) = case(name);
        let c = std::cell::Cell::new(0usize);
        let h = |x: f64| {
            c.set(c.get() + 1);
            g(x)
        };
        let o = romberg_with_status(h, a, b, epsabs, epsrel, n);
        cases += 1;
        assert_eq!(gsl_status(o.status), status, "{name} status");
        assert_eq!(o.neval, neval, "{name} neval");
        assert_eq!(c.get(), counted, "{name} integrand calls");
        if o.value.to_bits() == result.to_bits() {
            res_exact += 1;
        } else {
            println!("  {name}: result {:e} vs GSL {result:e}", o.value);
        }
    }
    println!("  romberg vs GSL 2.8: {cases} cases; results {res_exact}/{cases} bit-identical");
    assert_eq!(cases, 20);
    assert_eq!(res_exact, 20, "results bit-identical (pinned)");
}
