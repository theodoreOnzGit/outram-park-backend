// SPDX-License-Identifier: GPL-3.0-only
// Copyright (C) 2026 Theodore Ong and the outram-park contributors.

//! **Guard — PETIR's results must not depend on the build graph** (gh #1001).
//!
//! # The defect this guards against
//!
//! PETIR is `#![no_std]`, and its numerics called `real::Real` methods with
//! method syntax: `alpha.hypot(xnorm)`. Whenever ANY crate in the build graph
//! links `std` — `num-traits` with its `std` feature is the usual one, and
//! `uom` depends on `num-traits` — rustc can see std's inherent `f64`/`f32`
//! methods, and an inherent method beats a trait method of the same name. So
//! the same source called CORE-MATH / `libm` in one graph and the platform C
//! library (glibc here) in another.
//!
//! It surfaced when gh #994 added `faer` as a petir dev-dependency: `faer`
//! turns on `num-traits/std`, and `gsl_eigen_nonsymm_code_to_code` dropped
//! from 320/320 to 236/320 bit-identical, every miss in `rand25` — exactly
//! stock GSL's numbers, because `householder::transform` was now calling
//! glibc's `hypot`. gh #1001 first blamed `libm`'s `arch` feature; that was
//! measured and ruled out (`libm::fma` and `libm::sqrt` agree with the
//! hardware on 5 million inputs with `arch` on and off, and the eigen test
//! passes with `arch` on and `faer` absent). The compiled `rlib` showed the
//! mechanism directly: with `num-traits/std` in the graph it imported `hypot`,
//! `fma`, `exp`, `log`, `pow`, `sin`, `cos`, `atan2`, ... from the C library;
//! without it, none.
//!
//! The fix (gh #1001, 2026-10-20) writes every such call as UFCS,
//! `Real::hypot(alpha, xnorm)`, which std cannot shadow; `clippy.toml` forbids
//! the method form.
//!
//! # Methodology
//!
//! [`digest::digests`] hashes the exact bits of seven groups of public routes
//! that used the method form (see `std_shadowing/digest.rs`). This test pins
//! the digests. `cargo test -p petir` always has `std` in petir's graph (the
//! `faer` dev-dependency), so this is the std-present build; the std-absent
//! build cannot be made by `cargo test` at all, because dev-dependency
//! features always unify. `scripts/check-std-shadowing.sh` covers it: it
//! builds the same digest in a throwaway crate with and without
//! `num-traits/std`, and with `libm`'s `arch` on and with
//! `force-soft-floats`, and requires all four to print [`EXPECTED`]. It also
//! checks the compiled `rlib` for C maths imports and runs the clippy lint.
//!
//! Pass criterion: every group's digest equals [`EXPECTED`], bit for bit.
//!
//! # Prediction, written before the runs (2026-10-20)
//!
//! After the fix, the four graphs give identical digests, equal to what the
//! PRE-fix source gives with `std` absent (UFCS selects the same `Real`
//! method the method form selected there). Pre-fix with `std` present: every
//! group except `hypot` (which calls the free function `real::hypot`, never
//! shadowed) differs.
//!
//! # Results (2026-10-20, x86-64, glibc 2.44, Rust 1.99.1)
//!
//! The PRE-fix source (negative control): with `std` absent, all seven
//! finite-valued digests matched the post-fix ones; with `std` present, 6 of
//! 7 differed (all but `hypot`) and the rlib imported 95 C maths symbols
//! (`exp`, `log`, `pow`, `sin`, `cos`, `atan2`, `hypot`, `fma`, ...).
//!
//! The prediction held for finite values but MISSED one thing: `libm`'s
//! `arch` feature changes the SIGN BIT of the NaN that `libm::sqrt` (and
//! `libm::fma`) generate: `sqrt(-1)` is `0x7ff8...` in software and
//! `0xfff8...` from `sqrtsd`. The first four-graph run showed it as a
//! `specfunc` mismatch under `force-soft-floats` (the special functions
//! return NaN outside their domain). Per the maintainer's direction that
//! PETIR be bit-identical, not "equal up to NaN payload", `Real::sqrt` and
//! `Real::mul_add` now return one canonical NaN (x86's default, `0xfff8...`,
//! which the GSL references hold), and `sqrt` joined the UFCS
//! conversion (152 more sites). Final run, fixed source: all seven digests
//! bit-identical, NaNs included, in all four graphs; the std-present rlib
//! imports no inexact C maths routine.
//!
//! If this fails: do NOT update `EXPECTED` first. Run the script; if the four
//! builds disagree, a method-syntax call has come back (`cargo clippy` with
//! `--features uom/std` names it). Only a deliberate change to a routine's
//! arithmetic justifies new digests, recorded here with the date and reason.

//!
//! # Re-pinned 2026-10-21: `specfunc` and `transfer_fn` (gh:#985)
//!
//! A deliberate change to two groups' arithmetic. gh:#985 renamed the
//! transcendentals in the lifts' SOURCES (`outram-foam-basic-lib`, `chem-eng`)
//! to `RealMath`'s `r_` methods, and the lifts were re-lifted to match
//! (`tests/verbatim_provenance.rs`). `RealMath::r_exp`/`r_ln`/`r_powf` take the
//! ARM optimized-routines ports where `Real::exp`/`ln`/`powf` take `libm`, and
//! `r_sin_cos` is `(r_sin, r_cos)` where `Real::sin_cos` is `libm::sincos`, so
//! the special functions (`inc_gamma`, `inv_inc_gamma`, `erf_inv`) and the
//! transfer-function blocks moved: `specfunc` `0xa12b16066df916cf` ->
//! `0x88ec94e3202a1c7e`, `transfer_fn` `0x142dfcde5eb88a64` ->
//! `0x328eb18a7aceaede`. The other five groups did not move. `r_` methods
//! cannot be shadowed (std has none), so the graph independence this test
//! guards is unaffected; `scripts/check-std-shadowing.sh` was run on the new
//! pins (result in the commit that made this change).

#[path = "std_shadowing/digest.rs"]
mod digest;

/// The pinned digests. See the module docs before changing one.
const EXPECTED: &[(&str, u64)] = &[
    ("hypot", 0xc47bf45097971ff9),
    ("linalg", 0xd8c5ccef7949ea52),
    ("poly", 0x79cf17c03ec9892a),
    // Re-pinned 2026-10-21 (gh:#985 re-lift); was 0xa12b16066df916cf.
    ("specfunc", 0x88ec94e3202a1c7e),
    ("wgsl_mirrors", 0xea6f3ca0eb4f5ece),
    // Re-pinned 2026-10-21 (gh:#985 re-lift); was 0x142dfcde5eb88a64.
    ("transfer_fn", 0x328eb18a7aceaede),
    ("deriv_integration_ode", 0x22cff312c7e299cd),
];

#[test]
fn petir_output_does_not_depend_on_the_build_graph() {
    let got = digest::digests();
    for (name, value) in &got {
        println!("DIGEST {name} {value:#018x}");
    }
    let mismatches: Vec<String> = got
        .iter()
        .zip(EXPECTED)
        .filter(|((_, g), (_, e))| g != e)
        .map(|((n, g), (_, e))| format!("{n}: got {g:#018x}, expected {e:#018x}"))
        .collect();
    assert_eq!(got.len(), EXPECTED.len());
    assert!(
        mismatches.is_empty(),
        "PETIR output moved (gh #1001):\n{}",
        mismatches.join("\n")
    );
}
