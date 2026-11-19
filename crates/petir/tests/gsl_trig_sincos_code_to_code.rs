//! **V&V — `sin_gsl` / `cos_gsl` against compiled GSL 2.8 `gsl_sf_sin_e` /
//! `gsl_sf_cos_e`** (gh:#1421, maintainer approval 2026-11-12).
//!
//! # Methodology
//!
//! `reference-data/gsl/trig_sincos_reference_driver.c` calls GSL 2.8's
//! `gsl_sf_sin_e` and `gsl_sf_cos_e` (`specfunc/trig.c`, commit `cf180cd7`) on
//! 19 706 arguments: dense `k/4`, every `m*pi/4` octant boundary and its
//! neighbours, 18 000 pseudo-random arguments up to 1e14, round numbers up to
//! `DBL_MAX`, the small-argument cut and zeros, and writes the 64-bit patterns.
//! This test replays each row through `specfunc::trig::{sin_gsl, cos_gsl}` and
//! compares **bit patterns**.
//!
//! `gsl_sf_sin_e`/`cos_e` call no libm function (octant reduction plus a
//! Chebyshev series), so the reference does not depend on the platform libm and
//! needs no shim. Pass criterion: every row bit-identical.
//!
//! Prediction P1 (committed in `specfunc_gsl_gh1419_1420_1421.md` before this
//! ran): 100 %. Results (2026-11-12, Rust 1.99.1 release, GSL 2.8 `gcc -O2`
//! x86-64 glibc): printed by the test, recorded in the V&V record.
//!
//! The test also measures, for information, how far PETIR's own `Real::sin` /
//! `Real::cos` (the default) are from GSL's on the same rows, so the cost of
//! choosing `TrigKernel::Gsl` can be read off.

use petir::real::Real;
use petir::specfunc::trig::{cos_gsl, sin_gsl, TrigKernel};

const REFERENCE: &str = include_str!("../../../reference-data/gsl/trig-sincos-gsl-2.8-reference.txt");

fn hex(s: &str) -> f64 {
    f64::from_bits(u64::from_str_radix(s, 16).expect("hex"))
}

/// Ulp distance between two finite doubles of the same sign.
fn ulps(a: f64, b: f64) -> u64 {
    let (x, y) = (a.to_bits() as i64, b.to_bits() as i64);
    (x - y).unsigned_abs()
}

#[test]
fn sin_gsl_and_cos_gsl_are_bit_identical_to_gsl() {
    let (mut rows, mut exact_gsl, mut exact_petir, mut worst_petir) = (0usize, 0usize, 0usize, 0u64);
    let mut shown = 0;
    for line in REFERENCE.lines() {
        let mut it = line.split_whitespace();
        let name = it.next().unwrap();
        let x = hex(it.next().unwrap());
        let status: i32 = it.next().unwrap().parse().unwrap();
        let want = hex(it.next().unwrap());
        assert_eq!(status, 0, "GSL refused {name}({x:e})");
        let (ours, kernel) = match name {
            "sin" => (sin_gsl(x), Real::sin(x)),
            "cos" => (cos_gsl(x), Real::cos(x)),
            other => panic!("unknown row {other}"),
        };
        rows += 1;
        let same = ours.to_bits() == want.to_bits() || (ours.is_nan() && want.is_nan());
        exact_gsl += same as usize;
        if !same && shown < 8 {
            shown += 1;
            println!("  DIFF {name}({x:e}): gsl {want:e} ours {ours:e}");
        }
        // PETIR's own sin/cos, for information.
        if kernel.to_bits() == want.to_bits() {
            exact_petir += 1;
        } else if kernel.is_finite() && want.is_finite() && kernel.signum() == want.signum() {
            worst_petir = worst_petir.max(ulps(kernel, want));
        }
    }
    println!(
        "  sin_gsl/cos_gsl vs GSL 2.8: {exact_gsl}/{rows} bit-identical ({:.3} %); \
         PETIR's own Real::sin/cos vs GSL: {exact_petir}/{rows} bit-identical, worst same-sign gap {worst_petir} ulp",
        100.0 * exact_gsl as f64 / rows as f64
    );
    assert!(rows > 39_000, "reference truncated: {rows}");
    assert_eq!(exact_gsl, rows, "sin_gsl/cos_gsl must be bit-identical to GSL");
}

/// The kernel enum dispatches to the right routine, and the default is PETIR's
/// own (gh:#1421: the option must be explicit).
#[test]
fn trig_kernel_dispatch_and_default() {
    assert_eq!(TrigKernel::default(), TrigKernel::Petir);
    for x in [0.3_f64, 1.0, 2.5, 100.0, 12345.678, -7.25] {
        assert_eq!(TrigKernel::Gsl.sin(x).to_bits(), sin_gsl(x).to_bits());
        assert_eq!(TrigKernel::Gsl.cos(x).to_bits(), cos_gsl(x).to_bits());
        assert_eq!(TrigKernel::Petir.sin(x).to_bits(), Real::sin(x).to_bits());
        assert_eq!(TrigKernel::Petir.cos(x).to_bits(), Real::cos(x).to_bits());
    }
}
