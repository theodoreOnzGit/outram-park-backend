// Copyright (C) 2026 Theodore Ong and the outram-park contributors. GPL-3.0-only.

//! Real-valued elementary and special functions, in pure Rust.
//!
//! # What this module is for
//!
//! Thin re-exports of the [`libm`] crate — a pure-Rust port of musl's libm —
//! so the workspace has **one** place to get float maths that does not depend
//! on a platform C library.
//!
//! Two problems this solves:
//!
//! 1. **`erf`, `erfc`, `tgamma` and `lgamma` are not in Rust's `std`.** Three
//!    crates here reached them through `extern "C"` blocks, which is the last
//!    real C in the workspace and a silent dependency on a system libm. On
//!    `wasm32-unknown-unknown` there is no system libc at all.
//! 2. **Platform libms disagree in the last ulp.** `sin`, `exp`, `tgamma` and
//!    the rest return different final bits on glibc, macOS and MSVC. One fixed
//!    implementation makes results **bit-identical across platforms**, which
//!    matters for the Monte Carlo drivers that document thread-count-
//!    independent results and for committed regression fixtures.
//!
//! # How far these differ from glibc — measured, not assumed
//!
//! Swapping an implementation changes bits, so the difference was measured
//! against glibc on x86-64 over 801 points spanning `[-16.4, 16.4]` (400 for
//! the gamma pair, which needs `x > 0`), on 2026-09-14:
//!
//! ```text
//!   erf      801 pts,  793 bit-identical (99.0 %), worst rel 1.4e-16
//!   erfc     801 pts,  752 bit-identical (93.9 %), worst rel 3.6e-16
//!   lgamma   400 pts,  383 bit-identical (95.8 %), worst rel 4.2e-16
//!   tgamma   400 pts,  100 bit-identical (25.0 %), worst rel 9.1e-16
//! ```
//!
//! Everything is inside a few ulp, so no result changes meaningfully — but
//! note **`tgamma` differs in the last bits three times out of four**. A
//! fixture asserting `tgamma`-derived values to full `f64` precision will move.
//! That is a real consequence of the swap and is why the numbers are here
//! rather than left to be discovered.
//!
//! The comparison is reproduced as a test in `tests/libm_vs_platform.rs`.
//!
//! # Three functions take the fast route, by default
//!
//! [`exp`], [`ln`] and [`powf`] do **not** go to `libm`. They are routed to
//! [`crate::fast_exp`], [`crate::fast_log`] and [`crate::fast_pow`] — ports of
//! ARM optimized-routines, which is the implementation glibc itself ships.
//! This is the default, not an opt-in.
//!
//! Why those three and not the rest: they are the hot ones (68 of
//! `outram-mc-libs`' 126 transcendental sites are `exp` and `ln`, in Monte
//! Carlo inner loops), they are the ones where `libm` costs the most, and they
//! are the three ARM publishes as scalar `f64`. Measured from Rust over
//! 2 000 000 calls each (`tests/fast_math_speed.rs`):
//!
//! ```text
//!            old libm route / ARM route      ARM route / platform
//!   exp            1.75 - 1.86x                   0.70 - 0.75x
//!   ln             1.09 - 1.18x                   1.04 - 1.25x
//!   powf           2.09 - 2.18x                   1.41 - 1.57x
//! ```
//!
//! So the default got **faster** as well as staying deterministic — and `exp`
//! is now faster than the *platform*, because it inlines instead of calling
//! through a dynamic symbol. `ln` gains only ~1.1x and that is reported as
//! such: the `libm` crate's `log` was already good. Determinism is unchanged
//! in kind —
//! one fixed implementation on every platform — but the bits **moved**,
//! because ARM's `exp` and musl's are different implementations. Anything
//! pinned to `petir::real::{exp, ln, powf}` output at full `f64` precision
//! needs re-baselining once, in the same way the `libm` swap itself did.
//!
//! Everything else here stays on `libm`: ARM publishes no scalar double
//! `log10`, `cbrt`, `cos`, `sin`, `tanh` or `atan2`, and the special functions
//! (`erf`, `erfc`, `tgamma`, `lgamma`) are not in its scope at all.

/// The error function `erf(x)`. Not in Rust's `std`.
#[inline]
pub fn erf(x: f64) -> f64 {
    libm::erf(x)
}

/// The complementary error function `erfc(x) = 1 - erf(x)`, accurate for large
/// `x` where `1 - erf(x)` would cancel. Not in Rust's `std`.
#[inline]
pub fn erfc(x: f64) -> f64 {
    libm::erfc(x)
}

/// The gamma function `Γ(x)` (C's `tgamma`). Not in Rust's `std`.
///
/// See the module docs: this is the one of the four that most often differs
/// from glibc in the last bits.
#[inline]
pub fn tgamma(x: f64) -> f64 {
    libm::tgamma(x)
}

/// `ln |Γ(x)|` (C's `lgamma`). Not in Rust's `std`.
#[inline]
pub fn lgamma(x: f64) -> f64 {
    libm::lgamma(x)
}

// ─────────────────────────────────────────────────────────────────────────────
// Elementary transcendentals.
//
// These DO exist in Rust's `std` (as `f64` methods) — the reason to route
// through here is not availability but DETERMINISM: `std` dispatches to the
// platform libm, and glibc, macOS and MSVC disagree in the last ulp. Measured
// over 200 000 points, `std` vs `libm` differ on 1.68 % of `ln` calls, 9.64 %
// of `exp` and 3.22 % of `cos`.
//
// Note what is NOT here, deliberately: `sqrt`, `abs`, `floor`, `ceil`,
// `round`. IEEE-754 requires `sqrt` to be correctly rounded and the rest are
// exact, so they are ALREADY bit-identical everywhere — 0.0000 % mismatch over
// the same 200 000 points. Routing them through `libm` would buy nothing and
// cost real time: `libm::sqrt` benchmarks 3.73x slower than the hardware
// instruction. Use the `std` methods for those.
// ─────────────────────────────────────────────────────────────────────────────

/// Natural logarithm. (`std`: `f64::ln`.)
///
/// **Routed through [`crate::fast_log`]**, the ARM optimized-routines port —
/// see the "Three functions take the fast route" note above.
#[inline]
pub fn ln(x: f64) -> f64 {
    crate::fast_log::ln_ieee(x)
}

/// `e^x`. (`std`: `f64::exp`.)
///
/// **Routed through [`crate::fast_exp`]**, the ARM optimized-routines port —
/// see the "Three functions take the fast route" note above.
#[inline]
pub fn exp(x: f64) -> f64 {
    crate::fast_exp::exp_ieee(x)
}

/// Base-10 logarithm. (`std`: `f64::log10`.)
#[inline]
pub fn log10(x: f64) -> f64 {
    libm::log10(x)
}

/// Cube root. (`std`: `f64::cbrt`.)
#[inline]
pub fn cbrt(x: f64) -> f64 {
    libm::cbrt(x)
}

/// `x^y` for real `y`. (`std`: `f64::powf`.)
///
/// **Routed through [`crate::fast_pow`]**, the ARM optimized-routines port —
/// see the "Three functions take the fast route" note above.
#[inline]
pub fn powf(x: f64, y: f64) -> f64 {
    crate::fast_pow::powf_ieee(x, y)
}

/// Cosine. (`std`: `f64::cos`.)
#[inline]
pub fn cos(x: f64) -> f64 {
    libm::cos(x)
}

/// Sine. (`std`: `f64::sin`.)
#[inline]
pub fn sin(x: f64) -> f64 {
    libm::sin(x)
}

/// Hyperbolic tangent. (`std`: `f64::tanh`.)
#[inline]
pub fn tanh(x: f64) -> f64 {
    libm::tanh(x)
}

/// Two-argument arctangent. (`std`: `f64::atan2`.)
#[inline]
pub fn atan2(y: f64, x: f64) -> f64 {
    libm::atan2(y, x)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Anchor values that do not depend on which libm is underneath.
    #[test]
    fn known_values() {
        assert!((erf(0.0)).abs() < 1e-300);
        assert!((erfc(0.0) - 1.0).abs() < 1e-15);
        // Γ(1) = 1, Γ(5) = 4! = 24, Γ(1/2) = √π
        assert!((tgamma(1.0) - 1.0).abs() < 1e-15);
        assert!((tgamma(5.0) - 24.0).abs() < 1e-13);
        assert!((tgamma(0.5) - core::f64::consts::PI.sqrt()).abs() < 1e-15);
        // ln Γ(1) = 0, ln Γ(5) = ln 24
        assert!(lgamma(1.0).abs() < 1e-15);
        assert!((lgamma(5.0) - libm::log(24.0)).abs() < 1e-14);
    }

    /// `erf` and `erfc` must stay complementary.
    #[test]
    fn erf_and_erfc_are_complementary() {
        for k in -30..=30 {
            let x = f64::from(k) * 0.17;
            assert!(
                (erf(x) + erfc(x) - 1.0).abs() < 2.0e-16,
                "erf({x}) + erfc({x}) != 1"
            );
        }
    }
}
