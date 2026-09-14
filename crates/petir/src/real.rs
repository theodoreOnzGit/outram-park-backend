// SPDX-License-Identifier: GPL-3.0-only
// Copyright (C) 2026 OUTRAM PARK contributors
//
// This file is part of PETIR, a component of OUTRAM PARK.
//
// PETIR is free software: you can redistribute it and/or modify it under the
// terms of the GNU General Public License as published by the Free Software
// Foundation, version 3 of the License.
//
// PETIR is distributed in the hope that it will be useful, but WITHOUT ANY
// WARRANTY; without even the implied warranty of MERCHANTABILITY or FITNESS
// FOR A PARTICULAR PURPOSE.  See the GNU General Public License for more
// details.
//
// You should have received a copy of the GNU General Public License along
// with PETIR.  If not, see <https://www.gnu.org/licenses/>.

//! The `no_std` float-math shim: every transcendental `core` does not carry.
//!
//! # Why this module exists
//!
//! `core` provides only the float operations that need no libm call:
//! [`f64::abs`], [`f64::signum`], [`f64::copysign`], [`f64::min`],
//! [`f64::max`], [`f64::clamp`], [`f64::recip`], [`f64::to_radians`] and the
//! classification predicates. Everything else a numerical library lives on —
//! `sqrt`, `exp`, `ln`, `powf`, the trigonometric and hyperbolic families,
//! `floor`/`ceil`/`round`/`trunc` — is defined by `std`, not `core`, and is
//! therefore unavailable to a `no_std` crate.
//!
//! [`Real`] supplies exactly that missing set, backed by [`libm`] (a pure-Rust
//! translation of the same fdlibm/msun lineage the platform libm descends
//! from). Importing it restores ordinary method syntax:
//!
//! ```
//! use petir::real::Real;
//! let x: f64 = 2.0;
//! assert!((x.sqrt() - 1.414_213_562_373_095_1).abs() < 1e-15);
//! ```
//!
//! # Why the trait, rather than free functions
//!
//! Because it lets code be *lifted verbatim*. Large parts of PETIR are exact
//! transcriptions of kernels that already exist and are already tested
//! elsewhere in this workspace (`outram-foam-basic-lib`'s dense matrix,
//! polynomial and special-function layers) or upstream (GSL, GNU Octave).
//! Those bodies are written in `x.sqrt()` method form. Rewriting every call to
//! `sqrt(x)` would mean the port could no longer be diffed against its source,
//! which is the single most valuable property a translation has — see the
//! workspace CLAUDE.md "Debugging a port: read upstream first" rule.
//!
//! # Interaction with `std`
//!
//! When something else in the build links `std` (a `cargo test` run, for
//! instance), `f64` gains `std`'s *inherent* `sqrt`/`exp`/… methods. Rust
//! resolves inherent methods before trait methods, so those calls silently bind
//! to `std` instead of to [`Real`], and the `use` then reads as unused. That is
//! harmless — `std`'s implementations and `libm`'s are the same algorithms —
//! but it is why every `use crate::real::Real;` in this crate carries an
//! `#[allow(unused_imports)]`.
//!
//! # Accuracy
//!
//! `libm` targets < 1 ulp for the elementary functions, the same contract as
//! the C library it is translated from. PETIR does not add error on top: every
//! method below is a direct forward, except [`Real::powi`] (exponentiation by
//! squaring, which is what `std` does too), [`Real::fract`], and
//! [`Real::rem_euclid`], whose definitions are given inline.

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
