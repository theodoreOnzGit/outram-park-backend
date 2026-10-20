// SPDX-License-Identifier: GPL-3.0-only
// Copyright (C) 2026 Theodore Ong and the outram-park contributors.
//
// The method SIGNATURES in this file mirror uom 0.38.0 (MIT OR Apache-2.0,
// Copyright (c) the uom contributors, https://github.com/iliekturtles/uom):
// `src/system.rs` (Quantity::hypot, cbrt, mul_add, powi, sqrt, inside its
// `std!` blocks), `src/si/ratio.rs` (`impl Ratio`, `#[cfg(feature = "std")]`)
// and `src/si/angle.rs` (`impl Angle` and `Quantity::atan2`, same gate). The
// typenum bounds and the result-construction shape are uom's; the value
// arithmetic is PETIR's. Permissive into GPL-3.0, one-way.

//! The 27 quantity methods `no_std` `uom` drops, restored on PETIR maths.
//!
//! # Why this exists (gh:#985)
//!
//! The workspace builds `uom` **without** its `std` feature (maintainer
//! direction 2026-10-20). With `std` off, uom 0.38 compiles none of:
//!
//! | on | methods | trait here |
//! |---|---|---|
//! | every `Quantity` | `sqrt`, `cbrt`, `powi`, `hypot`, `mul_add`, `atan2` | [`QuantityMath`] |
//! | `Ratio` | `exp`, `exp2`, `exp_m1`, `ln`, `ln_1p`, `log`, `log2`, `log10`, `acos`, `acosh`, `asin`, `asinh`, `atan`, `atanh` | [`RatioMath`] |
//! | `Angle` | `sin`, `cos`, `tan`, `sinh`, `cosh`, `tanh`, `sin_cos` | [`AngleMath`] |
//!
//! With `std` ON, those methods went to the platform libm (`f64::cbrt` →
//! glibc / macOS / MSVC), which differ in the last ulp — the determinism leak
//! #985 closes. Here each one computes on `.value` through PETIR.
//!
//! # Names are `r_`-prefixed, on purpose
//!
//! `x.r_sqrt()`, never `x.sqrt()`. If `uom/std` were ever unified back on,
//! uom's inherent `sqrt` would win method resolution over a same-named trait
//! method and the call would silently return to the platform. A distinct name
//! cannot be shadowed. This is the same convention as
//! [`crate::mathf::RealMath`].
//!
//! # Which PETIR route each method takes
//!
//! | method | value computed by | vs `std`'s result |
//! |---|---|---|
//! | `r_sqrt` | [`Real::sqrt`] (`libm::sqrt`) | identical: IEEE sqrt is correctly rounded |
//! | `r_mul_add` | [`Real::mul_add`] (`libm::fma`) | identical: fma is correctly rounded |
//! | `r_powi` | [`RealMath::r_powi`] (`__powidf2`'s algorithm) | identical to compiler-rt's `powi`; `P2` is exactly `x*x` |
//! | `r_exp`, `r_ln` | [`RealMath`] — ARM optimized-routines ports | may differ by an ulp |
//! | `r_log` | `r_ln(x) / r_ln(base)` — `std`'s own definition, on the ARM `ln` | may differ by an ulp |
//! | `r_asinh`, `r_acosh`, `r_atanh` | [`Real`] (`libm`) — `RealMath` has no route | may differ by an ulp |
//! | everything else | [`RealMath`]'s `libm` row | may differ by an ulp |
//!
//! The routes are `RealMath`'s wherever it has one, so a quantity call site
//! and a bare-`f64` call site converted by #985 land on the same bits.
//!
//! # Fidelity to uom, and the one deviation
//!
//! Dimension bookkeeping is uom's, copied: `r_sqrt` requires every exponent
//! divisible by two (`PartialDiv<P2>`) and halves it, so `Area` → `Length` and
//! `Length.r_sqrt()` does not compile. Results are built the way uom builds
//! them — dimensioned results by writing `value` directly, `Ratio` → `Ratio`
//! / `Angle` through `new::<ratio>` / `new::<radian>` (which turns `-0.0`
//! into `+0.0`, exactly as uom does), `Angle` → `Ratio` through `From<V>`.
//!
//! **DEVIATION:** `r_hypot` takes `other: Self`, uom's `not_autoconvert`
//! signature. uom's `autoconvert` variant also accepts other base units and
//! converts with its private `change_base`; every workspace call is SI-to-SI,
//! where the two are the same.
//!
//! uom's `mul_add` uses `a.value` and `b.value` without converting their base
//! units `Ua`, `Ub`; that is mirrored, not fixed.

use uom::num::Num;
use uom::si::angle::{radian, Angle};
use uom::si::ratio::{ratio, Ratio};
use uom::si::{Dimension, Quantity, Units, ISQ};
use uom::typenum::{Integer, PartialDiv, PartialQuot, Prod, Sum, P2, P3};
use uom::Conversion;

use crate::mathf::RealMath;
use crate::real::Real;

/// `sqrt`, `cbrt`, `powi`, `hypot`, `mul_add` and `atan2` for any
/// [`Quantity`], on PETIR maths. See the [module docs](self).
///
/// `use petir::uom_math::QuantityMath;` and rename `.sqrt()` to `.r_sqrt()`.
#[diagnostic::on_unimplemented(
    message = "`{Self}` is not a uom `Quantity` with a PETIR-supported storage type",
    note = "QuantityMath is implemented for uom::si::Quantity<D, U, V> with V = f64 or f32"
)]
pub trait QuantityMath<D, U, V>: Sized
where
    D: Dimension + ?Sized,
    U: Units<V> + ?Sized,
    V: Num + Conversion<V> + Real + RealMath,
{
    /// Square root; halves every dimension exponent. `NaN` for a negative
    /// value. Mirrors uom's `Quantity::sqrt`.
    #[allow(clippy::type_complexity)]
    fn r_sqrt(
        self,
    ) -> Quantity<
        ISQ<
            PartialQuot<D::L, P2>,
            PartialQuot<D::M, P2>,
            PartialQuot<D::T, P2>,
            PartialQuot<D::I, P2>,
            PartialQuot<D::Th, P2>,
            PartialQuot<D::N, P2>,
            PartialQuot<D::J, P2>,
        >,
        U,
        V,
    >
    where
        D::L: PartialDiv<P2>,
        <D::L as PartialDiv<P2>>::Output: Integer,
        D::M: PartialDiv<P2>,
        <D::M as PartialDiv<P2>>::Output: Integer,
        D::T: PartialDiv<P2>,
        <D::T as PartialDiv<P2>>::Output: Integer,
        D::I: PartialDiv<P2>,
        <D::I as PartialDiv<P2>>::Output: Integer,
        D::Th: PartialDiv<P2>,
        <D::Th as PartialDiv<P2>>::Output: Integer,
        D::N: PartialDiv<P2>,
        <D::N as PartialDiv<P2>>::Output: Integer,
        D::J: PartialDiv<P2>,
        <D::J as PartialDiv<P2>>::Output: Integer,
        D::Kind: uom::marker::Div;

    /// Cube root; divides every dimension exponent by three. Mirrors uom's
    /// `Quantity::cbrt`.
    #[allow(clippy::type_complexity)]
    fn r_cbrt(
        self,
    ) -> Quantity<
        ISQ<
            PartialQuot<D::L, P3>,
            PartialQuot<D::M, P3>,
            PartialQuot<D::T, P3>,
            PartialQuot<D::I, P3>,
            PartialQuot<D::Th, P3>,
            PartialQuot<D::N, P3>,
            PartialQuot<D::J, P3>,
        >,
        U,
        V,
    >
    where
        D::L: PartialDiv<P3>,
        <D::L as PartialDiv<P3>>::Output: Integer,
        D::M: PartialDiv<P3>,
        <D::M as PartialDiv<P3>>::Output: Integer,
        D::T: PartialDiv<P3>,
        <D::T as PartialDiv<P3>>::Output: Integer,
        D::I: PartialDiv<P3>,
        <D::I as PartialDiv<P3>>::Output: Integer,
        D::Th: PartialDiv<P3>,
        <D::Th as PartialDiv<P3>>::Output: Integer,
        D::N: PartialDiv<P3>,
        <D::N as PartialDiv<P3>>::Output: Integer,
        D::J: PartialDiv<P3>,
        <D::J as PartialDiv<P3>>::Output: Integer,
        D::Kind: uom::marker::Div;

    /// Integer power by a `typenum` integer, e.g. `x.r_powi(P2::new())`;
    /// multiplies every dimension exponent by `E`. Mirrors uom's
    /// `Quantity::powi`.
    #[allow(clippy::type_complexity)]
    fn r_powi<E>(
        self,
        e: E,
    ) -> Quantity<
        ISQ<
            Prod<D::L, E>,
            Prod<D::M, E>,
            Prod<D::T, E>,
            Prod<D::I, E>,
            Prod<D::Th, E>,
            Prod<D::N, E>,
            Prod<D::J, E>,
        >,
        U,
        V,
    >
    where
        D::L: core::ops::Mul<E>,
        <D::L as core::ops::Mul<E>>::Output: Integer,
        D::M: core::ops::Mul<E>,
        <D::M as core::ops::Mul<E>>::Output: Integer,
        D::T: core::ops::Mul<E>,
        <D::T as core::ops::Mul<E>>::Output: Integer,
        D::I: core::ops::Mul<E>,
        <D::I as core::ops::Mul<E>>::Output: Integer,
        D::Th: core::ops::Mul<E>,
        <D::Th as core::ops::Mul<E>>::Output: Integer,
        D::N: core::ops::Mul<E>,
        <D::N as core::ops::Mul<E>>::Output: Integer,
        D::J: core::ops::Mul<E>,
        <D::J as core::ops::Mul<E>>::Output: Integer,
        D::Kind: uom::marker::Mul,
        E: Integer;

    /// Hypotenuse `sqrt(self² + other²)` without undue overflow. Mirrors
    /// uom's `not_autoconvert` `Quantity::hypot` (see the module's
    /// DEVIATION note).
    fn r_hypot(self, other: Self) -> Self;

    /// Fused `(self * a) + b` with one rounding. Mirrors uom's
    /// `Quantity::mul_add`, including its use of `a.value` / `b.value`
    /// without base-unit conversion.
    #[allow(clippy::type_complexity)]
    fn r_mul_add<Da, Ua, Ub>(
        self,
        a: Quantity<Da, Ua, V>,
        b: Quantity<
            ISQ<
                Sum<D::L, Da::L>,
                Sum<D::M, Da::M>,
                Sum<D::T, Da::T>,
                Sum<D::I, Da::I>,
                Sum<D::Th, Da::Th>,
                Sum<D::N, Da::N>,
                Sum<D::J, Da::J>,
            >,
            Ub,
            V,
        >,
    ) -> Quantity<
        ISQ<
            Sum<D::L, Da::L>,
            Sum<D::M, Da::M>,
            Sum<D::T, Da::T>,
            Sum<D::I, Da::I>,
            Sum<D::Th, Da::Th>,
            Sum<D::N, Da::N>,
            Sum<D::J, Da::J>,
        >,
        U,
        V,
    >
    where
        D::L: core::ops::Add<Da::L>,
        <D::L as core::ops::Add<Da::L>>::Output: Integer,
        D::M: core::ops::Add<Da::M>,
        <D::M as core::ops::Add<Da::M>>::Output: Integer,
        D::T: core::ops::Add<Da::T>,
        <D::T as core::ops::Add<Da::T>>::Output: Integer,
        D::I: core::ops::Add<Da::I>,
        <D::I as core::ops::Add<Da::I>>::Output: Integer,
        D::Th: core::ops::Add<Da::Th>,
        <D::Th as core::ops::Add<Da::Th>>::Output: Integer,
        D::N: core::ops::Add<Da::N>,
        <D::N as core::ops::Add<Da::N>>::Output: Integer,
        D::J: core::ops::Add<Da::J>,
        <D::J as core::ops::Add<Da::J>>::Output: Integer,
        D::Kind: uom::marker::Mul,
        Da: Dimension + ?Sized,
        Da::Kind: uom::marker::Mul,
        Ua: Units<V> + ?Sized,
        Ub: Units<V> + ?Sized;

    /// Four-quadrant arctangent of `self` (y) and `other` (x), as an
    /// [`Angle`]. Mirrors uom's `Quantity::atan2`.
    fn r_atan2(self, other: Self) -> Angle<U, V>
    where
        radian: Conversion<V, T = V::T>;
}

impl<D, U, V> QuantityMath<D, U, V> for Quantity<D, U, V>
where
    D: Dimension + ?Sized,
    U: Units<V> + ?Sized,
    V: Num + Conversion<V> + Real + RealMath,
{
    #[inline(always)]
    fn r_sqrt(
        self,
    ) -> Quantity<
        ISQ<
            PartialQuot<D::L, P2>,
            PartialQuot<D::M, P2>,
            PartialQuot<D::T, P2>,
            PartialQuot<D::I, P2>,
            PartialQuot<D::Th, P2>,
            PartialQuot<D::N, P2>,
            PartialQuot<D::J, P2>,
        >,
        U,
        V,
    >
    where
        D::L: PartialDiv<P2>,
        <D::L as PartialDiv<P2>>::Output: Integer,
        D::M: PartialDiv<P2>,
        <D::M as PartialDiv<P2>>::Output: Integer,
        D::T: PartialDiv<P2>,
        <D::T as PartialDiv<P2>>::Output: Integer,
        D::I: PartialDiv<P2>,
        <D::I as PartialDiv<P2>>::Output: Integer,
        D::Th: PartialDiv<P2>,
        <D::Th as PartialDiv<P2>>::Output: Integer,
        D::N: PartialDiv<P2>,
        <D::N as PartialDiv<P2>>::Output: Integer,
        D::J: PartialDiv<P2>,
        <D::J as PartialDiv<P2>>::Output: Integer,
        D::Kind: uom::marker::Div,
    {
        Quantity {
            dimension: core::marker::PhantomData,
            units: core::marker::PhantomData,
            value: Real::sqrt(self.value),
        }
    }

    #[inline(always)]
    fn r_cbrt(
        self,
    ) -> Quantity<
        ISQ<
            PartialQuot<D::L, P3>,
            PartialQuot<D::M, P3>,
            PartialQuot<D::T, P3>,
            PartialQuot<D::I, P3>,
            PartialQuot<D::Th, P3>,
            PartialQuot<D::N, P3>,
            PartialQuot<D::J, P3>,
        >,
        U,
        V,
    >
    where
        D::L: PartialDiv<P3>,
        <D::L as PartialDiv<P3>>::Output: Integer,
        D::M: PartialDiv<P3>,
        <D::M as PartialDiv<P3>>::Output: Integer,
        D::T: PartialDiv<P3>,
        <D::T as PartialDiv<P3>>::Output: Integer,
        D::I: PartialDiv<P3>,
        <D::I as PartialDiv<P3>>::Output: Integer,
        D::Th: PartialDiv<P3>,
        <D::Th as PartialDiv<P3>>::Output: Integer,
        D::N: PartialDiv<P3>,
        <D::N as PartialDiv<P3>>::Output: Integer,
        D::J: PartialDiv<P3>,
        <D::J as PartialDiv<P3>>::Output: Integer,
        D::Kind: uom::marker::Div,
    {
        Quantity {
            dimension: core::marker::PhantomData,
            units: core::marker::PhantomData,
            value: RealMath::r_cbrt(self.value),
        }
    }

    #[inline(always)]
    fn r_powi<E>(
        self,
        _e: E,
    ) -> Quantity<
        ISQ<
            Prod<D::L, E>,
            Prod<D::M, E>,
            Prod<D::T, E>,
            Prod<D::I, E>,
            Prod<D::Th, E>,
            Prod<D::N, E>,
            Prod<D::J, E>,
        >,
        U,
        V,
    >
    where
        D::L: core::ops::Mul<E>,
        <D::L as core::ops::Mul<E>>::Output: Integer,
        D::M: core::ops::Mul<E>,
        <D::M as core::ops::Mul<E>>::Output: Integer,
        D::T: core::ops::Mul<E>,
        <D::T as core::ops::Mul<E>>::Output: Integer,
        D::I: core::ops::Mul<E>,
        <D::I as core::ops::Mul<E>>::Output: Integer,
        D::Th: core::ops::Mul<E>,
        <D::Th as core::ops::Mul<E>>::Output: Integer,
        D::N: core::ops::Mul<E>,
        <D::N as core::ops::Mul<E>>::Output: Integer,
        D::J: core::ops::Mul<E>,
        <D::J as core::ops::Mul<E>>::Output: Integer,
        D::Kind: uom::marker::Mul,
        E: Integer,
    {
        Quantity {
            dimension: core::marker::PhantomData,
            units: core::marker::PhantomData,
            value: RealMath::r_powi(self.value, E::to_i32()),
        }
    }

    #[inline(always)]
    fn r_hypot(self, other: Self) -> Self {
        Quantity {
            dimension: core::marker::PhantomData,
            units: core::marker::PhantomData,
            value: RealMath::r_hypot(self.value, other.value),
        }
    }

    #[inline(always)]
    fn r_mul_add<Da, Ua, Ub>(
        self,
        a: Quantity<Da, Ua, V>,
        b: Quantity<
            ISQ<
                Sum<D::L, Da::L>,
                Sum<D::M, Da::M>,
                Sum<D::T, Da::T>,
                Sum<D::I, Da::I>,
                Sum<D::Th, Da::Th>,
                Sum<D::N, Da::N>,
                Sum<D::J, Da::J>,
            >,
            Ub,
            V,
        >,
    ) -> Quantity<
        ISQ<
            Sum<D::L, Da::L>,
            Sum<D::M, Da::M>,
            Sum<D::T, Da::T>,
            Sum<D::I, Da::I>,
            Sum<D::Th, Da::Th>,
            Sum<D::N, Da::N>,
            Sum<D::J, Da::J>,
        >,
        U,
        V,
    >
    where
        D::L: core::ops::Add<Da::L>,
        <D::L as core::ops::Add<Da::L>>::Output: Integer,
        D::M: core::ops::Add<Da::M>,
        <D::M as core::ops::Add<Da::M>>::Output: Integer,
        D::T: core::ops::Add<Da::T>,
        <D::T as core::ops::Add<Da::T>>::Output: Integer,
        D::I: core::ops::Add<Da::I>,
        <D::I as core::ops::Add<Da::I>>::Output: Integer,
        D::Th: core::ops::Add<Da::Th>,
        <D::Th as core::ops::Add<Da::Th>>::Output: Integer,
        D::N: core::ops::Add<Da::N>,
        <D::N as core::ops::Add<Da::N>>::Output: Integer,
        D::J: core::ops::Add<Da::J>,
        <D::J as core::ops::Add<Da::J>>::Output: Integer,
        D::Kind: uom::marker::Mul,
        Da: Dimension + ?Sized,
        Da::Kind: uom::marker::Mul,
        Ua: Units<V> + ?Sized,
        Ub: Units<V> + ?Sized,
    {
        Quantity {
            dimension: core::marker::PhantomData,
            units: core::marker::PhantomData,
            value: Real::mul_add(self.value, a.value, b.value),
        }
    }

    #[inline(always)]
    fn r_atan2(self, other: Self) -> Angle<U, V>
    where
        radian: Conversion<V, T = V::T>,
    {
        Angle::new::<radian>(RealMath::r_atan2(self.value, other.value))
    }
}

/// `exp`, `ln`, the logarithms and the inverse trig/hyperbolic functions for
/// a dimensionless [`Ratio`], on PETIR maths. See the [module docs](self).
pub trait RatioMath: Sized {
    /// The storage type (`f64` or `f32`), the type of [`RatioMath::r_log`]'s base.
    type Value;
    /// The [`Angle`] an inverse trig function returns.
    type Angle;
    /// Inverse cosine, as an angle.
    fn r_acos(self) -> Self::Angle;
    /// Inverse hyperbolic cosine, as an angle (uom's own typing).
    fn r_acosh(self) -> Self::Angle;
    /// Inverse sine, as an angle.
    fn r_asin(self) -> Self::Angle;
    /// Inverse hyperbolic sine, as an angle (uom's own typing).
    fn r_asinh(self) -> Self::Angle;
    /// Inverse tangent, as an angle.
    fn r_atan(self) -> Self::Angle;
    /// Inverse hyperbolic tangent, as an angle (uom's own typing).
    fn r_atanh(self) -> Self::Angle;
    /// `e^self`.
    fn r_exp(self) -> Self;
    /// `2^self`.
    fn r_exp2(self) -> Self;
    /// `e^self - 1`, accurate near zero.
    fn r_exp_m1(self) -> Self;
    /// Natural logarithm.
    fn r_ln(self) -> Self;
    /// `ln(1 + self)`, accurate near zero.
    fn r_ln_1p(self) -> Self;
    /// Logarithm to an arbitrary base, `ln(self) / ln(base)` as `std` defines it.
    fn r_log(self, base: Self::Value) -> Self;
    /// Base-2 logarithm.
    fn r_log2(self) -> Self;
    /// Base-10 logarithm.
    fn r_log10(self) -> Self;
}

impl<U, V> RatioMath for Ratio<U, V>
where
    U: Units<V> + ?Sized,
    V: uom::num::Float + Conversion<V> + Real + RealMath,
    radian: Conversion<V, T = V::T>,
    ratio: Conversion<V, T = V::T>,
{
    type Value = V;
    type Angle = Angle<U, V>;

    #[inline(always)]
    fn r_acos(self) -> Angle<U, V> {
        Angle::new::<radian>(RealMath::r_acos(self.value))
    }
    #[inline(always)]
    fn r_acosh(self) -> Angle<U, V> {
        Angle::new::<radian>(Real::acosh(self.value))
    }
    #[inline(always)]
    fn r_asin(self) -> Angle<U, V> {
        Angle::new::<radian>(RealMath::r_asin(self.value))
    }
    #[inline(always)]
    fn r_asinh(self) -> Angle<U, V> {
        Angle::new::<radian>(Real::asinh(self.value))
    }
    #[inline(always)]
    fn r_atan(self) -> Angle<U, V> {
        Angle::new::<radian>(RealMath::r_atan(self.value))
    }
    #[inline(always)]
    fn r_atanh(self) -> Angle<U, V> {
        Angle::new::<radian>(Real::atanh(self.value))
    }
    #[inline(always)]
    fn r_exp(self) -> Self {
        Ratio::new::<ratio>(RealMath::r_exp(self.value))
    }
    #[inline(always)]
    fn r_exp2(self) -> Self {
        Ratio::new::<ratio>(RealMath::r_exp2(self.value))
    }
    #[inline(always)]
    fn r_exp_m1(self) -> Self {
        Ratio::new::<ratio>(RealMath::r_exp_m1(self.value))
    }
    #[inline(always)]
    fn r_ln(self) -> Self {
        Ratio::new::<ratio>(RealMath::r_ln(self.value))
    }
    #[inline(always)]
    fn r_ln_1p(self) -> Self {
        Ratio::new::<ratio>(RealMath::r_ln_1p(self.value))
    }
    #[inline(always)]
    fn r_log(self, base: V) -> Self {
        Ratio::new::<ratio>(RealMath::r_ln(self.value) / RealMath::r_ln(base))
    }
    #[inline(always)]
    fn r_log2(self) -> Self {
        Ratio::new::<ratio>(RealMath::r_log2(self.value))
    }
    #[inline(always)]
    fn r_log10(self) -> Self {
        Ratio::new::<ratio>(RealMath::r_log10(self.value))
    }
}

/// Trigonometric and hyperbolic functions of an [`Angle`], as a [`Ratio`], on
/// PETIR maths. See the [module docs](self).
pub trait AngleMath: Sized {
    /// The [`Ratio`] the functions return.
    type Ratio;
    /// Cosine.
    fn r_cos(self) -> Self::Ratio;
    /// Hyperbolic cosine.
    fn r_cosh(self) -> Self::Ratio;
    /// Sine.
    fn r_sin(self) -> Self::Ratio;
    /// Hyperbolic sine.
    fn r_sinh(self) -> Self::Ratio;
    /// `(sin, cos)`, each bit-identical to `r_sin` / `r_cos`.
    fn r_sin_cos(self) -> (Self::Ratio, Self::Ratio);
    /// Tangent.
    fn r_tan(self) -> Self::Ratio;
    /// Hyperbolic tangent.
    fn r_tanh(self) -> Self::Ratio;
}

impl<U, V> AngleMath for Angle<U, V>
where
    U: Units<V> + ?Sized,
    V: uom::num::Float + Conversion<V> + Real + RealMath,
{
    type Ratio = Ratio<U, V>;

    #[inline(always)]
    fn r_cos(self) -> Ratio<U, V> {
        RealMath::r_cos(self.value).into()
    }
    #[inline(always)]
    fn r_cosh(self) -> Ratio<U, V> {
        RealMath::r_cosh(self.value).into()
    }
    #[inline(always)]
    fn r_sin(self) -> Ratio<U, V> {
        RealMath::r_sin(self.value).into()
    }
    #[inline(always)]
    fn r_sinh(self) -> Ratio<U, V> {
        RealMath::r_sinh(self.value).into()
    }
    #[inline(always)]
    fn r_sin_cos(self) -> (Ratio<U, V>, Ratio<U, V>) {
        let (s, c) = RealMath::r_sin_cos(self.value);
        (s.into(), c.into())
    }
    #[inline(always)]
    fn r_tan(self) -> Ratio<U, V> {
        RealMath::r_tan(self.value).into()
    }
    #[inline(always)]
    fn r_tanh(self) -> Ratio<U, V> {
        RealMath::r_tanh(self.value).into()
    }
}

#[cfg(test)]
mod tests {
    //! Every method is pinned BIT-IDENTICAL to the PETIR function it forwards
    //! to (so the quantity spelling and the `f64` spelling can never mean
    //! different things), and the dimension bookkeeping is checked by the
    //! result TYPES: each `let x: Length = ...` fails to compile if the
    //! exponent arithmetic is wrong.
    use super::*;
    use uom::si::angle::radian;
    use uom::si::area::square_meter;
    use uom::si::f64::{Angle, Area, Length, Ratio, Time, Velocity, Volume};
    use uom::si::length::meter;
    use uom::si::ratio::ratio;
    use uom::si::time::second;
    use uom::si::velocity::meter_per_second;
    use uom::si::volume::cubic_meter;
    use uom::typenum::{N1, P2, P3};

    const XS: [f64; 7] = [0.0, 0.125, 0.5, 0.9, 1.0, 2.5, 1.0e6];

    fn same(a: f64, b: f64) -> bool {
        a.to_bits() == b.to_bits() || (a.is_nan() && b.is_nan())
    }

    #[test]
    fn sqrt_halves_dimensions_and_is_libm_sqrt() {
        for &x in &XS {
            let l: Length = Area::new::<square_meter>(x).r_sqrt();
            assert!(same(l.get::<meter>(), libm::sqrt(x)));
        }
        let r: Ratio = Ratio::new::<ratio>(-1.0).r_sqrt();
        assert!(r.value.is_nan());
    }

    #[test]
    fn cbrt_thirds_dimensions_and_is_r_cbrt() {
        for &x in &XS {
            let l: Length = Volume::new::<cubic_meter>(x).r_cbrt();
            assert!(same(l.get::<meter>(), x.r_cbrt()));
        }
    }

    #[test]
    fn powi_multiplies_dimensions_and_p2_is_x_times_x() {
        for &x in &XS {
            let a: Area = Length::new::<meter>(x).r_powi(P2::new());
            assert!(same(a.get::<square_meter>(), x * x));
            let v: Volume = Length::new::<meter>(x).r_powi(P3::new());
            assert!(same(v.get::<cubic_meter>(), x.r_powi(3)));
        }
        let inv = Time::new::<second>(4.0).r_powi(N1::new());
        let f: uom::si::f64::Frequency = inv;
        assert_eq!(f.value, 0.25);
    }

    #[test]
    fn hypot_is_r_hypot() {
        for &x in &XS {
            let h = Length::new::<meter>(x).r_hypot(Length::new::<meter>(3.0));
            assert!(same(h.get::<meter>(), x.r_hypot(3.0)));
        }
    }

    #[test]
    fn mul_add_is_fma_and_types_the_product() {
        for &x in &XS {
            let t = Time::new::<second>(x);
            let v = Velocity::new::<meter_per_second>(3.0);
            let l0 = Length::new::<meter>(0.1);
            let l: Length = v.r_mul_add(t, l0);
            assert!(same(l.get::<meter>(), libm::fma(3.0, x, 0.1)));
        }
    }

    #[test]
    fn atan2_is_r_atan2_in_radians() {
        for &x in &XS {
            let a: Angle = Length::new::<meter>(x).r_atan2(Length::new::<meter>(-2.0));
            assert!(same(a.get::<radian>(), x.r_atan2(-2.0)));
        }
    }

    #[test]
    fn ratio_methods_forward_bit_identically() {
        for &x in &XS {
            let r = Ratio::new::<ratio>(x);
            let g = |q: Ratio| q.get::<ratio>();
            let ga = |q: Angle| q.get::<radian>();
            // `+ 0.0` reproduces uom's `new::<unit>` conversion, which maps
            // `-0.0` to `+0.0`; for every other value it is exact.
            assert!(same(g(r.r_exp()), x.r_exp() + 0.0));
            assert!(same(g(r.r_exp2()), x.r_exp2() + 0.0));
            assert!(same(g(r.r_exp_m1()), x.r_exp_m1() + 0.0));
            assert!(same(g(r.r_ln()), x.r_ln() + 0.0));
            assert!(same(g(r.r_ln_1p()), x.r_ln_1p() + 0.0));
            assert!(same(g(r.r_log(3.0)), x.r_ln() / 3.0_f64.r_ln() + 0.0));
            assert!(same(g(r.r_log2()), x.r_log2() + 0.0));
            assert!(same(g(r.r_log10()), x.r_log10() + 0.0));
            assert!(same(ga(r.r_acos()), x.r_acos() + 0.0));
            assert!(same(ga(r.r_asin()), x.r_asin() + 0.0));
            assert!(same(ga(r.r_atan()), x.r_atan() + 0.0));
            assert!(same(ga(r.r_asinh()), libm::asinh(x) + 0.0));
            assert!(same(ga(r.r_acosh()), libm::acosh(x) + 0.0));
            assert!(same(ga(r.r_atanh()), libm::atanh(x) + 0.0));
        }
    }

    #[test]
    fn angle_methods_forward_bit_identically() {
        for &x in &XS {
            let a = Angle::new::<radian>(x);
            assert!(same(a.r_sin().value, x.r_sin()));
            assert!(same(a.r_cos().value, x.r_cos()));
            assert!(same(a.r_tan().value, x.r_tan()));
            assert!(same(a.r_sinh().value, x.r_sinh()));
            assert!(same(a.r_cosh().value, x.r_cosh()));
            assert!(same(a.r_tanh().value, x.r_tanh()));
            let (s, c) = a.r_sin_cos();
            assert!(same(s.value, x.r_sin()) && same(c.value, x.r_cos()));
        }
    }
}
