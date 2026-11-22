// SPDX-License-Identifier: AGPL-3.0-only
//! `RealMath::r_sin` against glibc's `sin` and the correctly rounded sine
//! (gh:#1455).
//!
//! # Methodology
//!
//! - **Reference:** `verification_and_validation/sin_correct_rounding/sin_ref.txt`,
//!   written by `sinq.c` (same folder): 8 000 deterministic arguments (xorshift64;
//!   quarter each in [0, 2 pi), [0, 100), [-500, 500), [0, 0.1)); per line the
//!   argument, glibc 2.39's `sin` (what gfortran calls), and the correctly
//!   rounded sine, `sinq` (libquadmath, binary128) rounded once to binary64.
//!   binary128 carries 113 bits, so the double rounding is wrong only for an
//!   argument whose sine lies within 2^-60 of a midpoint: none expected in 8 000.
//! - **This test:** `RealMath::r_sin` (the `libm` crate, a musl port) on every
//!   argument, counted against the correctly rounded value and against glibc.
//! - **Pass criterion:** the counts below, exactly (a regression in either
//!   direction is a change worth a reader's attention).
//!
//! # Prediction (before the run)
//!
//! gh:#1455 found 10 of 352 `csunr` values one ulp from glibc and that std's
//! `sin` (glibc) fixes them. glibc's `sin` is not correctly rounded in general
//! (IBM Accurate Mathematical Library claims < 1 ulp, in practice ~0.55 ulp),
//! so the question is which of the two is further from the correctly rounded
//! value. Predicted: both miss on a minority; musl's `sin` (<= 1 ulp, fdlibm)
//! misses more often than glibc.
//!
//! # Result (2026-11-11, glibc 2.39, gcc 13.3, x86-64)
//!
//! See the asserted numbers in the test body and
//! `verification_and_validation/sin_correct_rounding/README.md`.

use petir::mathf::RealMath;

fn load() -> Vec<(f64, f64, f64)> {
    include_str!("../verification_and_validation/sin_correct_rounding/sin_ref.txt")
        .lines()
        .map(|l| {
            let mut it = l.split_whitespace().map(|h| f64::from_bits(u64::from_str_radix(h, 16).unwrap()));
            (it.next().unwrap(), it.next().unwrap(), it.next().unwrap())
        })
        .collect()
}

/// `r_sin` against the correctly rounded sine and against glibc.
#[test]
fn r_sin_vs_correct_rounding_and_glibc() {
    let rows = load();
    assert_eq!(rows.len(), 8000);
    let (mut glibc_wrong, mut petir_wrong, mut petir_ne_glibc, mut petir_worse_than_glibc, mut petir_better) = (0, 0, 0, 0, 0);
    let mut max_ulp_petir = 0u64;
    for &(x, g, c) in &rows {
        let p = x.r_sin();
        let ulp = |a: f64, b: f64| (a.to_bits() as i64 - b.to_bits() as i64).unsigned_abs();
        if g.to_bits() != c.to_bits() {
            glibc_wrong += 1;
        }
        if p.to_bits() != c.to_bits() {
            petir_wrong += 1;
            max_ulp_petir = max_ulp_petir.max(ulp(p, c));
        }
        if p.to_bits() != g.to_bits() {
            petir_ne_glibc += 1;
        }
        if p.to_bits() != c.to_bits() && g.to_bits() == c.to_bits() {
            petir_worse_than_glibc += 1;
        }
        if p.to_bits() == c.to_bits() && g.to_bits() != c.to_bits() {
            petir_better += 1;
        }
    }
    println!(
        "n=8000  glibc != correct: {glibc_wrong}  r_sin != correct: {petir_wrong} (max {max_ulp_petir} ulp)  \
         r_sin != glibc: {petir_ne_glibc}  r_sin wrong where glibc right: {petir_worse_than_glibc}  \
         r_sin right where glibc wrong: {petir_better}"
    );
    assert_eq!(max_ulp_petir <= 1, true, "r_sin is within 1 ulp of the correctly rounded value");
    assert_eq!(
        (glibc_wrong, petir_wrong, petir_ne_glibc, petir_worse_than_glibc, petir_better),
        EXPECTED,
        "recorded counts changed"
    );
}

/// Recorded: (glibc wrong, r_sin wrong, r_sin != glibc, r_sin wrong where glibc
/// right, r_sin right where glibc wrong). Filled from the first run, 2026-11-11.
const EXPECTED: (i32, i32, i32, i32, i32) = (8, 199, 201, 196, 5);
