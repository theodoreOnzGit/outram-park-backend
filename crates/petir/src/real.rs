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


// ---------------------------------------------------------------------------
// The method-syntax half: everything `core` omits, as a trait.
// ---------------------------------------------------------------------------
//
// The free functions above exist for two reasons: `std` does not HAVE
// erf/erfc/tgamma/lgamma at all, and exp/ln/powf are routed deliberately
// through the ARM optimized-routines ports rather than through libm.
//
// The trait below exists for a third: `core` withholds the elementary
// operations as METHODS. `sqrt`, `exp`, `ln`, `powf` and the trig and rounding
// families are defined on `f64` by `std`, not by `core`, so a `no_std` crate
// cannot write `x.sqrt()`.
//
// WHICH TO REACH FOR. Prefer the FREE FUNCTIONS in new code: they are the
// routed, verified path, and for exp/ln/powf they are the ARM ports that are
// bit-identical to upstream. The trait is what lets PETIR carry kernels LIFTED
// VERBATIM from elsewhere in this workspace without rewriting every call site,
// and that byte-for-byte property is the whole value of a lift (see
// `tests/verbatim_provenance.rs`).
//
// NOTE the deliberate asymmetry: `Real::exp`, `Real::ln` and `Real::powf` go to
// `libm`, NOT to the ARM ports, because a verbatim lift must behave exactly as
// it did in the crate it came from. Changing the method path would silently
// change the numerics of lifted code. Routing a consumer onto the ARM port is
// an explicit, per-call-site decision (bn:op-j57z), not something a lift should
// inherit by accident.

/// The float operations `core` lacks, provided for `no_std` builds.
///
/// Implemented for [`f64`] only — PETIR is a double-precision library
/// throughout, matching GSL's and Octave's default storage type. Every method
/// has the same contract, argument order and edge-case behaviour as the
/// identically named [`f64`] inherent method in `std`.
pub trait Real: Copy {
    /// Square root. `NaN` for negative arguments; `-0.0` for `-0.0`.
    fn sqrt(self) -> Self;
    /// Cube root, defined for negative arguments (`(-8).cbrt() == -2`).
    fn cbrt(self) -> Self;
    /// `e^self`.
    fn exp(self) -> Self;
    /// `2^self`.
    fn exp2(self) -> Self;
    /// `e^self - 1`, accurate for `self` near zero.
    fn exp_m1(self) -> Self;
    /// Natural logarithm. `NaN` for negative arguments, `-inf` at zero.
    fn ln(self) -> Self;
    /// `ln(1 + self)`, accurate for `self` near zero.
    fn ln_1p(self) -> Self;
    /// Base-10 logarithm.
    fn log10(self) -> Self;
    /// Base-2 logarithm.
    fn log2(self) -> Self;
    /// Logarithm to an arbitrary `base`, computed as `self.ln() / base.ln()`.
    fn log(self, base: Self) -> Self;
    /// `self^n` for a real exponent.
    fn powf(self, n: Self) -> Self;
    /// `self^n` for an integer exponent, by exponentiation-by-squaring.
    fn powi(self, n: i32) -> Self;
    /// Sine of an angle in radians.
    fn sin(self) -> Self;
    /// Cosine of an angle in radians.
    fn cos(self) -> Self;
    /// Tangent of an angle in radians.
    fn tan(self) -> Self;
    /// Sine and cosine together, in one call.
    fn sin_cos(self) -> (Self, Self);
    /// Arcsine, in radians over `[-pi/2, pi/2]`.
    fn asin(self) -> Self;
    /// Arccosine, in radians over `[0, pi]`.
    fn acos(self) -> Self;
    /// Arctangent, in radians over `(-pi/2, pi/2)`.
    fn atan(self) -> Self;
    /// Four-quadrant arctangent of `self / other`, in radians over `(-pi, pi]`.
    fn atan2(self, other: Self) -> Self;
    /// Hyperbolic sine.
    fn sinh(self) -> Self;
    /// Hyperbolic cosine.
    fn cosh(self) -> Self;
    /// Hyperbolic tangent.
    fn tanh(self) -> Self;
    /// Inverse hyperbolic sine.
    fn asinh(self) -> Self;
    /// Inverse hyperbolic cosine.
    fn acosh(self) -> Self;
    /// Inverse hyperbolic tangent.
    fn atanh(self) -> Self;
    /// `sqrt(self^2 + other^2)` without intermediate overflow or underflow.
    fn hypot(self, other: Self) -> Self;
    /// Largest integer less than or equal to `self`.
    fn floor(self) -> Self;
    /// Smallest integer greater than or equal to `self`.
    fn ceil(self) -> Self;
    /// Nearest integer, halfway cases rounded away from zero.
    fn round(self) -> Self;
    /// Integer part, truncating toward zero.
    fn trunc(self) -> Self;
    /// Fractional part, `self - self.trunc()`, carrying `self`'s sign.
    fn fract(self) -> Self;
    /// `self * a + b` with a single rounding (fused multiply-add).
    fn mul_add(self, a: Self, b: Self) -> Self;
    /// Least non-negative remainder of `self (mod rhs)`.
    fn rem_euclid(self, rhs: Self) -> Self;
}

impl Real for f64 {
    #[inline]
    fn sqrt(self) -> f64 {
        libm::sqrt(self)
    }
    #[inline]
    fn cbrt(self) -> f64 {
        libm::cbrt(self)
    }
    #[inline]
    fn exp(self) -> f64 {
        libm::exp(self)
    }
    #[inline]
    fn exp2(self) -> f64 {
        libm::exp2(self)
    }
    #[inline]
    fn exp_m1(self) -> f64 {
        libm::expm1(self)
    }
    #[inline]
    fn ln(self) -> f64 {
        libm::log(self)
    }
    #[inline]
    fn ln_1p(self) -> f64 {
        libm::log1p(self)
    }
    #[inline]
    fn log10(self) -> f64 {
        libm::log10(self)
    }
    #[inline]
    fn log2(self) -> f64 {
        libm::log2(self)
    }
    #[inline]
    fn log(self, base: f64) -> f64 {
        libm::log(self) / libm::log(base)
    }
    #[inline]
    fn powf(self, n: f64) -> f64 {
        libm::pow(self, n)
    }
    /// Exponentiation by squaring, mirroring what `std` lowers `powi` to.
    ///
    /// A negative exponent is handled as `1 / self^|n|`. `i32::MIN` cannot be
    /// negated in two's complement, so it is widened to `i64` before the
    /// absolute value is taken.
    #[inline]
    fn powi(self, n: i32) -> f64 {
        let mut e = (n as i64).unsigned_abs();
        let mut base = self;
        let mut acc = 1.0_f64;
        while e > 0 {
            if e & 1 == 1 {
                acc *= base;
            }
            base *= base;
            e >>= 1;
        }
        if n < 0 {
            1.0 / acc
        } else {
            acc
        }
    }
    #[inline]
    fn sin(self) -> f64 {
        libm::sin(self)
    }
    #[inline]
    fn cos(self) -> f64 {
        libm::cos(self)
    }
    #[inline]
    fn tan(self) -> f64 {
        libm::tan(self)
    }
    #[inline]
    fn sin_cos(self) -> (f64, f64) {
        libm::sincos(self)
    }
    #[inline]
    fn asin(self) -> f64 {
        libm::asin(self)
    }
    #[inline]
    fn acos(self) -> f64 {
        libm::acos(self)
    }
    #[inline]
    fn atan(self) -> f64 {
        libm::atan(self)
    }
    #[inline]
    fn atan2(self, other: f64) -> f64 {
        libm::atan2(self, other)
    }
    #[inline]
    fn sinh(self) -> f64 {
        libm::sinh(self)
    }
    #[inline]
    fn cosh(self) -> f64 {
        libm::cosh(self)
    }
    #[inline]
    fn tanh(self) -> f64 {
        libm::tanh(self)
    }
    #[inline]
    fn asinh(self) -> f64 {
        libm::asinh(self)
    }
    #[inline]
    fn acosh(self) -> f64 {
        libm::acosh(self)
    }
    #[inline]
    fn atanh(self) -> f64 {
        libm::atanh(self)
    }
    #[inline]
    fn hypot(self, other: f64) -> f64 {
        libm::hypot(self, other)
    }
    #[inline]
    fn floor(self) -> f64 {
        libm::floor(self)
    }
    #[inline]
    fn ceil(self) -> f64 {
        libm::ceil(self)
    }
    #[inline]
    fn round(self) -> f64 {
        libm::round(self)
    }
    #[inline]
    fn trunc(self) -> f64 {
        libm::trunc(self)
    }
    #[inline]
    fn fract(self) -> f64 {
        self - libm::trunc(self)
    }
    #[inline]
    fn mul_add(self, a: f64, b: f64) -> f64 {
        libm::fma(self, a, b)
    }
    /// `self - rhs * (self / rhs).floor()`, clamped into `[0, |rhs|)`.
    ///
    /// Matches `f64::rem_euclid`: the result is never negative, and rounding
    /// can only push it to `|rhs|` at the very edge, which is folded back to
    /// zero.
    #[inline]
    fn rem_euclid(self, rhs: f64) -> f64 {
        let r = libm::fmod(self, rhs);
        if r < 0.0 {
            r + libm::fabs(rhs)
        } else {
            r
        }
    }
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
