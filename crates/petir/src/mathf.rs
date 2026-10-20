// Copyright (C) 2026 Theodore Ong and the outram-park contributors. GPL-3.0-only.

//! `f64` (and, since 2026-10-20, `f32`) transcendentals as methods, so a
//! crate can adopt the deterministic route by renaming rather than rewriting.
//!
//! **`f32` ADDED 2026-10-20** (maintainer direction, gh:#985): `RealMath` is
//! implemented for `f32` too, delegating every method to
//! [`crate::real::Real`]'s `f32` impl, i.e. `libm`'s single-precision
//! functions (`expf`, `logf`, `powf`, `sinf`, ...), and to its
//! exponentiation-by-squaring `powi`. There is no `f32` ARM port, so `r_exp`,
//! `r_ln` and `r_powf` take `libm` on `f32`. This is the route the
//! [`crate::wgsl`] CPU mirrors already use, so GPU CPU-mirror code converted
//! by the `.r_*` rename stays on one deterministic, platform-independent
//! route. The table below describes the `f64` impl.
//!
//! # Why a trait and not just [`crate::real`]
//!
//! [`crate::real::exp`] and friends are free functions, and calling them
//! directly is perfectly correct. The problem is *migration cost*: a crate
//! that already writes `x.exp()` in 300 places has to have each expression
//! turned inside out, `(a * b + c).exp()` becoming `exp(a * b + c)`. Every one
//! of those is a chance to move a paren and change the maths silently.
//!
//! [`RealMath`] makes the same migration a mechanical rename — `.exp()` to
//! `.r_exp()` — which a `sed` can do and a reviewer can check by eye. The
//! `r_` prefix is deliberate: it does not shadow the inherent `f64` methods,
//! so a site that was missed still compiles and still calls the platform, and
//! a site that was converted reads as a deliberate choice rather than looking
//! like ordinary `std`.
//!
//! This shape is `outram-mc-libs`' (`bn:op-chyp.5`), lifted here so the other
//! consumers share one trait instead of copying it four times (`bn:op-j57z`).
//!
//! # What it covers, and what it does not
//!
//! ~~**Exactly the three functions PETIR has a fast, deterministic port of** —
//! [`RealMath::r_exp`], [`RealMath::r_ln`], [`RealMath::r_powf`].~~
//! **EXTENDED 2026-10-16** (maintainer decision D6, gh:#896 / gh:#903): the
//! trait now also carries the `libm`-delegated functions and an integer power,
//! because migrating a consumer by rename only works if every call site it has
//! can be renamed. There are now **three routes behind one trait**, and a
//! caller should know which one a method takes:
//!
//! | route | methods | implementation | speed vs the platform |
//! |---|---|---|---|
//! | **ARM** | [`r_exp`](RealMath::r_exp), [`r_ln`](RealMath::r_ln), [`r_powf`](RealMath::r_powf) | [`crate::fast_exp`], [`crate::fast_log`], [`crate::fast_pow`] — ports of ARM optimized-routines, verified bit-identical to upstream's own compiled C | `exp` faster, `ln` comparable, `powf` ~1.5x slower (measured, see [`crate::real`]) |
//! | **integer power** | [`r_powi`](RealMath::r_powi) | [`crate::real::powi`] — libgcc's `__powidf2` algorithm, plain `*` and one `/` | not separately benchmarked |
//! | **`libm`** | `r_log10`, `r_cbrt`, `r_exp_m1`, `r_ln_1p`, `r_sin`, `r_cos`, `r_tan`, `r_asin`, `r_acos`, `r_atan`, `r_atan2`, ~~`r_hypot`~~, `r_sinh`, `r_cosh`, `r_tanh`, `r_log2`, `r_exp2`, `r_sin_cos` | the [`libm`] crate (a pure-Rust port of musl), through [`crate::real`]'s free functions where one exists | **slower than the platform**; by how much is not measured here |
//! | **CORE-MATH** (since 2026-10-20, gh:#997) | [`r_hypot`](RealMath::r_hypot) | [`crate::hypot`] — CORE-MATH's correctly rounded `cr_hypot` / `cr_hypotf`, moved off `libm` because musl's `hypot` is off by one ulp on some inputs | measured in `tests/hypot_correct_rounding.rs` |
//!
//! **EXTENDED 2026-10-20** (gh:#985): `r_log2`, `r_exp2` and `r_sin_cos` join
//! the `libm` row, so the workspace-wide rename has a target for every `log2`,
//! `exp2` and `sin_cos` call site. `r_sin_cos` is `(r_sin, r_cos)`, not
//! `libm::sincos`, so it is bit-identical to the two separate methods.
//!
//! **Only `exp`, `ln` and `powf` take the ARM route.** ARM optimized-routines
//! publishes no scalar-double `log10`, `cbrt`, trig or hyperbolic functions,
//! so the rest are `libm`: deterministic — one fixed implementation on every
//! target — but not fast, and not glibc's bits. They are here so that a crate
//! choosing determinism can get it by a rename; that is a trade a crate makes
//! knowingly, which is why the `r_` prefix stays and nothing shadows `std`.
//!
//! Each new method is a pure re-spelling: a test pins every one bit-identical
//! to the function it forwards to, so the method and the free function can
//! never mean different things.
//!
//! # There is no escape hatch, and the one that existed never worked
//!
//! ~~A `platform-libm` feature re-pointed all three methods at the platform's
//! own `f64::exp` / `f64::ln` / `f64::powf`, for the single job of rebuilding a
//! golden reference recorded before the ARM ports were adopted.~~
//! **REMOVED 2026-09-17**, by maintainer decision, for two reasons.
//!
//! It **could not compile in any configuration**. `platform-libm` pulled in a
//! `std` feature, but `lib.rs` was `#![no_std]` unconditionally and its
//! `extern crate std;` was gated on `cfg(test)` alone -- and `cargo test`
//! builds the library *without* `cfg(test)` first. So `f64::ln` never
//! resolved, and the hatch had been dead since this crate became `no_std`. It
//! was found by a `cargo check --all-features --all-targets` sweep
//! (`bn:op-7kwt`), not by anyone trying to use it.
//!
//! Gating it on `cfg(test)` instead was considered and rejected: its only
//! sanctioned use was a DOWNSTREAM test
//! (`cargo test -p tampines-steam-tables --features platform-libm`), where
//! `petir` compiles as a dependency and `cfg(test)` is false. That gate would
//! have silently supplied the default ARM-ports impl and produced a fixture
//! labelled `platform-libm` that was not built with it -- a silent wrong
//! answer in place of a compile error.
//!
//! **Consequence to be honest about:** the pre-adoption Edwards-O'Brien golden
//! reference (`tampines-steam-tables`'
//! `verification_and_validation/edwards_blowdown_platform_libm_golden.md`)
//! can no longer be regenerated from this tree. It was already unregenerable
//! in practice, since the hatch did not build; removing the feature makes that
//! explicit rather than leaving a route that appears to exist. Regenerating it
//! requires checking out a pre-`no_std` commit.
//!
//! # A 1 ulp change does not stay 1 ulp in a chained transient
//!
//! Adopting the ports in `tampines-steam-tables` was verified by its 1009
//! library tests being unchanged and its five generated V&V reports
//! regenerating character-identical. That evidence is sound for *single-state
//! property evaluation*, where a last-ulp difference is invisible against a
//! 1e-8 verification tolerance, and it was blind to chained integration. The
//! Edwards-O'Brien blowdown case -- roughly 20 000 timesteps x 24 cells x
//! several flashes per cell per corrector, order 1e9 chained evaluations
//! through a stiff pressure-velocity coupling -- passed before the sweep and
//! panicked after it. Bisected to a single commit; see `bn:op-ppmk` and
//! `bn:op-s2dc`.
//!
//! That is **not** evidence against the ports, which sit within about 1 ulp of
//! glibc and are identical on every platform, where the platform route is
//! neither. It is evidence that "unit tests unchanged" is the wrong acceptance
//! check for a long chained solve, and it is why this escape hatch earns its
//! keep: it lets the pre-adoption trajectory be re-derived and diffed rather
//! than merely remembered.
//!
//! ~~It deliberately does **not** cover `log10`, `cbrt`, `cos`, `sin`, `tanh` or
//! `atan2`. Those have no ARM scalar-double implementation, so PETIR can only
//! offer the `libm` (musl-derived) versions, which are deterministic but
//! measurably slower than the platform. That is a trade-off a crate should opt
//! into knowingly, so reach for [`crate::real`]'s free functions and mean it,
//! rather than getting it by importing a trait.~~
//! **SUPERSEDED 2026-10-16** by maintainer decision D6 — those are now methods
//! (see the table above). The trade-off itself is unchanged: they are `libm`,
//! deterministic and slower than the platform, and the "slower" has still not
//! been quantified for them (`tests/fast_math_speed.rs` covers only the three
//! ARM routes).
//!
//! It still does **not** cover `sqrt`, `abs`, `floor`, `ceil` or `round`.
//! IEEE-754 requires `sqrt` to be correctly rounded and ~~the others are exact,
//! so they are **already** bit-identical on every platform~~ `abs`, `floor`,
//! `ceil` and `round` are exact, so those are **already** bit-identical on every
//! platform. Routing them would buy nothing and cost real time — `libm::sqrt`
//! benchmarks 3.73x slower than the hardware instruction. (**Qualified
//! 2026-10-20, gh #1001:** that holds for finite `sqrt` results; the NaN that
//! `sqrt(-x)` generates differs in sign between the instruction and libm's
//! software path, so PETIR's own library calls `Real::sqrt`, which returns
//! one canonical NaN.)
//!
//! **CORRECTED 2026-10-16 — `powi` is NOT exact.** `f64::powi` is lowered by
//! each target's backend and its last bit differs on `x86_64-pc-windows-msvc`
//! (measured; see [`crate::real::powi`]). Use [`crate::real::powi`] where the
//! bits must match across platforms. ~~Whether `RealMath` should gain an
//! `r_powi` is open (gh:#896).~~ **Settled the same day** (D6):
//! [`RealMath::r_powi`] forwards to [`crate::real::powi`].
//!
//! # The bits move once
//!
//! Adopting this changes results in the last ulp: ARM's `exp` and the platform's
//! are different implementations. Nothing becomes less accurate — all three are
//! within about 1 ulp of glibc, and identical on *every* platform, which they
//! were not before — but a fixture pinned to full `f64` precision needs
//! re-baselining once. Do that deliberately, and record the move.
//!
//! ```
//! use petir::mathf::RealMath;
//!
//! let x = 2.0_f64;
//! // Deterministic on every platform, and the same value glibc gives to ~1 ulp.
//! assert!((x.r_ln().r_exp() - x).abs() < 1e-15);
//! ```

use crate::real::Real;

/// `f64` and `f32` maths as `r_`-prefixed methods, on PETIR's deterministic
/// routes. The routes below are the `f64` impl's; the `f32` impl is `libm`'s
/// single-precision functions throughout (see the module docs).
///
/// Method names carry an `r_` prefix so adoption is a rename, not an
/// expression rewrite, and so a converted call site reads as deliberate.
/// `r_exp`, `r_ln` and `r_powf` take the ARM optimized-routines ports;
/// `r_powi` takes [`crate::real::powi`]; every other method is `libm`. See the
/// module docs for the table and for what is deliberately *not* here.
pub trait RealMath: Copy {
    /// Natural logarithm, `ln x`.
    ///
    /// Returns `-inf` at `x == ±0` and a NaN for `x < 0`, as the platform
    /// does. Routed to [`crate::fast_log`].
    fn r_ln(self) -> Self;

    /// `e^x`.
    ///
    /// Returns `+inf` on overflow and `+0.0` on underflow, as the platform
    /// does. Routed to [`crate::fast_exp`].
    fn r_exp(self) -> Self;

    /// `x^y` for real `y`.
    ///
    /// Full IEEE-754 semantics, including the sign of a result from a negative
    /// base with an odd-integer exponent. Routed to [`crate::fast_pow`].
    fn r_powf(self, y: Self) -> Self;

    /// `x^n` for an integer `n`.
    ///
    /// Routed to [`crate::real::powi`] (libgcc's `__powidf2` algorithm), **not**
    /// to `f64::powi`, whose last bit is backend-dependent.
    fn r_powi(self, n: i32) -> Self;

    /// Base-10 logarithm. `libm`, via [`crate::real::log10`].
    fn r_log10(self) -> Self;

    /// Cube root, defined for negative arguments. `libm`, via [`crate::real::cbrt`].
    fn r_cbrt(self) -> Self;

    /// `e^x - 1`, accurate near zero. `libm::expm1`.
    fn r_exp_m1(self) -> Self;

    /// `ln(1 + x)`, accurate near zero. `libm::log1p`.
    fn r_ln_1p(self) -> Self;

    /// Sine, radians. `libm`, via [`crate::real::sin`].
    fn r_sin(self) -> Self;

    /// Cosine, radians. `libm`, via [`crate::real::cos`].
    fn r_cos(self) -> Self;

    /// Tangent, radians. `libm::tan`.
    fn r_tan(self) -> Self;

    /// Arcsine over `[-pi/2, pi/2]`. `libm::asin`.
    fn r_asin(self) -> Self;

    /// Arccosine over `[0, pi]`. `libm::acos`.
    fn r_acos(self) -> Self;

    /// Arctangent over `(-pi/2, pi/2)`. `libm::atan`.
    fn r_atan(self) -> Self;

    /// Four-quadrant arctangent of `self / x` (`self` is `y`, as in
    /// `f64::atan2`). `libm`, via [`crate::real::atan2`].
    fn r_atan2(self, x: Self) -> Self;

    /// `sqrt(self^2 + other^2)` without spurious overflow, correctly rounded.
    /// ~~`libm::hypot`.~~ **CHANGED 2026-10-20** (gh:#997): CORE-MATH's
    /// `cr_hypot` / `cr_hypotf` through [`crate::real::hypot`].
    fn r_hypot(self, other: Self) -> Self;

    /// Hyperbolic sine. `libm::sinh`.
    fn r_sinh(self) -> Self;

    /// Hyperbolic cosine. `libm::cosh`.
    fn r_cosh(self) -> Self;

    /// Hyperbolic tangent. `libm`, via [`crate::real::tanh`].
    fn r_tanh(self) -> Self;

    /// Base-2 logarithm. `libm::log2`.
    fn r_log2(self) -> Self;

    /// `2^x`. `libm::exp2`.
    fn r_exp2(self) -> Self;

    /// `(sin x, cos x)`, radians, in the order `f64::sin_cos` returns them.
    ///
    /// Computed as [`RealMath::r_sin`] and [`RealMath::r_cos`] (`libm`, via
    /// [`crate::real::sin`] and [`crate::real::cos`]), not `libm::sincos`, so
    /// `x.r_sin_cos()` is bit-identical to `(x.r_sin(), x.r_cos())` and a call
    /// site's meaning does not depend on which spelling it used.
    fn r_sin_cos(self) -> (Self, Self);
}

/// The default, deterministic route: ARM ports for `exp`/`ln`/`powf`,
/// `__powidf2` for `powi`, `libm` for the rest.
impl RealMath for f64 {
    #[inline]
    fn r_ln(self) -> f64 {
        crate::real::ln(self)
    }

    #[inline]
    fn r_exp(self) -> f64 {
        crate::real::exp(self)
    }

    #[inline]
    fn r_powf(self, y: f64) -> f64 {
        crate::real::powf(self, y)
    }

    #[inline]
    fn r_powi(self, n: i32) -> f64 {
        crate::real::powi(self, n)
    }

    #[inline]
    fn r_log10(self) -> f64 {
        crate::real::log10(self)
    }

    #[inline]
    fn r_cbrt(self) -> f64 {
        crate::real::cbrt(self)
    }

    #[inline]
    fn r_exp_m1(self) -> f64 {
        libm::expm1(self)
    }

    #[inline]
    fn r_ln_1p(self) -> f64 {
        libm::log1p(self)
    }

    #[inline]
    fn r_sin(self) -> f64 {
        crate::real::sin(self)
    }

    #[inline]
    fn r_cos(self) -> f64 {
        crate::real::cos(self)
    }

    #[inline]
    fn r_tan(self) -> f64 {
        libm::tan(self)
    }

    #[inline]
    fn r_asin(self) -> f64 {
        libm::asin(self)
    }

    #[inline]
    fn r_acos(self) -> f64 {
        libm::acos(self)
    }

    #[inline]
    fn r_atan(self) -> f64 {
        libm::atan(self)
    }

    #[inline]
    fn r_atan2(self, x: f64) -> f64 {
        crate::real::atan2(self, x)
    }

    #[inline]
    fn r_hypot(self, other: f64) -> f64 {
        crate::real::hypot(self, other)
    }

    #[inline]
    fn r_sinh(self) -> f64 {
        libm::sinh(self)
    }

    #[inline]
    fn r_cosh(self) -> f64 {
        libm::cosh(self)
    }

    #[inline]
    fn r_tanh(self) -> f64 {
        crate::real::tanh(self)
    }

    #[inline]
    fn r_log2(self) -> f64 {
        libm::log2(self)
    }

    #[inline]
    fn r_exp2(self) -> f64 {
        libm::exp2(self)
    }

    #[inline]
    fn r_sin_cos(self) -> (f64, f64) {
        (crate::real::sin(self), crate::real::cos(self))
    }
}

/// The `f32` route: [`crate::real::Real`]'s `f32` impl for every method, which
/// is `libm`'s single-precision functions plus exponentiation by squaring for
/// `powi`. Called through UFCS (`<f32 as Real>::exp`), never `self.exp()`:
/// under a `std`-linked build `f32`'s inherent method would win silently.
impl RealMath for f32 {
    #[inline]
    fn r_ln(self) -> f32 {
        <f32 as Real>::ln(self)
    }

    #[inline]
    fn r_exp(self) -> f32 {
        <f32 as Real>::exp(self)
    }

    #[inline]
    fn r_powf(self, y: f32) -> f32 {
        <f32 as Real>::powf(self, y)
    }

    #[inline]
    fn r_powi(self, n: i32) -> f32 {
        <f32 as Real>::powi(self, n)
    }

    #[inline]
    fn r_log10(self) -> f32 {
        <f32 as Real>::log10(self)
    }

    #[inline]
    fn r_cbrt(self) -> f32 {
        <f32 as Real>::cbrt(self)
    }

    #[inline]
    fn r_exp_m1(self) -> f32 {
        <f32 as Real>::exp_m1(self)
    }

    #[inline]
    fn r_ln_1p(self) -> f32 {
        <f32 as Real>::ln_1p(self)
    }

    #[inline]
    fn r_sin(self) -> f32 {
        <f32 as Real>::sin(self)
    }

    #[inline]
    fn r_cos(self) -> f32 {
        <f32 as Real>::cos(self)
    }

    #[inline]
    fn r_tan(self) -> f32 {
        <f32 as Real>::tan(self)
    }

    #[inline]
    fn r_asin(self) -> f32 {
        <f32 as Real>::asin(self)
    }

    #[inline]
    fn r_acos(self) -> f32 {
        <f32 as Real>::acos(self)
    }

    #[inline]
    fn r_atan(self) -> f32 {
        <f32 as Real>::atan(self)
    }

    #[inline]
    fn r_atan2(self, x: f32) -> f32 {
        <f32 as Real>::atan2(self, x)
    }

    #[inline]
    fn r_hypot(self, other: f32) -> f32 {
        <f32 as Real>::hypot(self, other)
    }

    #[inline]
    fn r_sinh(self) -> f32 {
        <f32 as Real>::sinh(self)
    }

    #[inline]
    fn r_cosh(self) -> f32 {
        <f32 as Real>::cosh(self)
    }

    #[inline]
    fn r_tanh(self) -> f32 {
        <f32 as Real>::tanh(self)
    }

    #[inline]
    fn r_log2(self) -> f32 {
        <f32 as Real>::log2(self)
    }

    #[inline]
    fn r_exp2(self) -> f32 {
        <f32 as Real>::exp2(self)
    }

    #[inline]
    fn r_sin_cos(self) -> (f32, f32) {
        (<f32 as Real>::sin(self), <f32 as Real>::cos(self))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The trait must be a pure re-spelling of [`crate::real`] — bit for bit,
    /// not approximately. If these ever diverge, a call site's meaning depends
    /// on which spelling it used, which is the whole thing this trait exists to
    /// avoid.
    #[test]
    fn the_methods_are_bit_identical_to_the_free_functions() {
        for k in -400..=400 {
            let x = f64::from(k) * 0.37;
            assert_eq!(
                x.r_exp().to_bits(),
                crate::real::exp(x).to_bits(),
                "exp at {x}"
            );
            if x > 0.0 {
                assert_eq!(
                    x.r_ln().to_bits(),
                    crate::real::ln(x).to_bits(),
                    "ln at {x}"
                );
                assert_eq!(
                    x.r_powf(2.5).to_bits(),
                    crate::real::powf(x, 2.5).to_bits(),
                    "powf at {x}"
                );
            }
        }
    }

    /// ...and that the route really is the ARM port, not `libm`. This is the
    /// property adopters are buying; pin it here rather than trusting the
    /// module docs to stay true.
    #[test]
    fn the_route_is_the_arm_port() {
        for k in 1..=300 {
            let x = f64::from(k) * 0.29;
            assert_eq!(x.r_exp().to_bits(), crate::fast_exp::exp_ieee(x).to_bits());
            assert_eq!(x.r_ln().to_bits(), crate::fast_log::ln_ieee(x).to_bits());
            assert_eq!(
                x.r_powf(1.7).to_bits(),
                crate::fast_pow::powf_ieee(x, 1.7).to_bits()
            );
        }
    }

    /// The 2026-10-16 additions (D6) are pure re-spellings too. Each is pinned
    /// bit-identical to the function it is documented to forward to, over a
    /// sweep wide enough to cross the argument-reduction branches of the trig
    /// and hyperbolic families.
    #[test]
    fn the_libm_and_powi_methods_are_bit_identical_to_what_they_forward_to() {
        let eq = |a: f64, b: f64, what: &str, x: f64| {
            assert_eq!(a.to_bits(), b.to_bits(), "{what} at {x:e}");
        };
        for k in -2000..=2000 {
            let x = f64::from(k) * 0.0137 + 1.0e-3;
            eq(x.r_cbrt(), libm::cbrt(x), "cbrt", x);
            eq(x.r_cbrt(), crate::real::cbrt(x), "cbrt/real", x);
            eq(x.r_sin(), libm::sin(x), "sin", x);
            eq(x.r_cos(), libm::cos(x), "cos", x);
            eq(x.r_tan(), libm::tan(x), "tan", x);
            eq(x.r_atan(), libm::atan(x), "atan", x);
            eq(x.r_sinh(), libm::sinh(x), "sinh", x);
            eq(x.r_cosh(), libm::cosh(x), "cosh", x);
            eq(x.r_tanh(), libm::tanh(x), "tanh", x);
            eq(x.r_exp_m1(), libm::expm1(x), "expm1", x);
            eq(x.r_atan2(0.7 - x), libm::atan2(x, 0.7 - x), "atan2", x);
            eq(
                x.r_hypot(3.0 * x - 2.0),
                crate::real::hypot(x, 3.0 * x - 2.0),
                "hypot",
                x,
            );
            eq(x.r_exp2(), libm::exp2(x), "exp2", x);
            let (s, c) = x.r_sin_cos();
            eq(s, x.r_sin(), "sin_cos.0", x);
            eq(c, x.r_cos(), "sin_cos.1", x);
            let u = x / 30.0; // inside [-1, 1]
            eq(u.r_asin(), libm::asin(u), "asin", u);
            eq(u.r_acos(), libm::acos(u), "acos", u);
            if x > -1.0 {
                eq(x.r_ln_1p(), libm::log1p(x), "log1p", x);
            }
            if x > 0.0 {
                eq(x.r_log10(), libm::log10(x), "log10", x);
                eq(x.r_log2(), libm::log2(x), "log2", x);
            }
            for n in [-23, -7, -2, -1, 0, 1, 2, 3, 5, 12, 23] {
                eq(x.r_powi(n), crate::real::powi(x, n), "powi", x);
            }
        }
        // powi edge exponents, including the one whose negation overflows i32.
        for n in [i32::MIN, i32::MIN + 1, i32::MAX] {
            let x = 1.000_000_1_f64;
            eq(x.r_powi(n), crate::real::powi(x, n), "powi edge", x);
        }
    }

    /// The case that motivated `r_powi`: `10**(-23)` differed between Linux
    /// and `x86_64-pc-windows-msvc` through `f64::powi`. `r_powi` must be the
    /// `__powidf2` route, and its exact small cases must stay exact.
    #[test]
    fn r_powi_is_the_powidf2_route() {
        assert_eq!(
            10.0_f64.r_powi(-23).to_bits(),
            crate::real::powi(10.0, -23).to_bits()
        );
        assert_eq!(2.0_f64.r_powi(10), 1024.0);
        assert_eq!(2.0_f64.r_powi(-2), 0.25);
        assert_eq!(0.0_f64.r_powi(-1), f64::INFINITY);
        assert_eq!(3.0_f64.r_powi(0), 1.0);
    }

    /// The IEEE special values must survive the extra hop.
    /// The 2026-10-20 additions (gh:#985) keep their exact small cases.
    #[test]
    fn r_log2_r_exp2_and_r_sin_cos_exact_cases() {
        assert_eq!(8.0_f64.r_log2(), 3.0);
        assert_eq!(0.0_f64.r_log2(), f64::NEG_INFINITY);
        assert!((-1.0_f64).r_log2().is_nan());
        assert_eq!(3.0_f64.r_exp2(), 8.0);
        assert_eq!((-1.0_f64).r_exp2(), 0.5);
        assert_eq!(0.0_f64.r_sin_cos(), (0.0, 1.0));
    }

    #[test]
    fn the_special_values_pass_through() {
        assert_eq!(0.0_f64.r_ln(), f64::NEG_INFINITY);
        assert!((-1.0_f64).r_ln().is_nan());
        assert_eq!(1000.0_f64.r_exp(), f64::INFINITY);
        assert_eq!((-1000.0_f64).r_exp(), 0.0);
        assert_eq!(2.0_f64.r_powf(0.0), 1.0);
        assert_eq!((-2.0_f64).r_powf(3.0), -8.0);
        assert!((-2.0_f64).r_powf(0.5).is_nan());
    }

    /// **Methodology.** The `f32` impl (2026-10-20, gh:#985) must be a pure
    /// re-spelling of `libm`'s single-precision functions — bit for bit — so
    /// `f32` code converted by the `.r_*` rename lands on the same route as
    /// the [`crate::wgsl`] CPU mirrors. Swept over 4001 points crossing the
    /// trig and hyperbolic argument-reduction branches. Pass: every result's
    /// bits equal the `libm` function's (and `powi`'s equal
    /// `<f32 as Real>::powi`'s). **Result** (2026-10-20): all equal.
    #[test]
    fn the_f32_methods_are_bit_identical_to_libm_single_precision() {
        let eq = |a: f32, b: f32, what: &str, x: f32| {
            assert_eq!(a.to_bits(), b.to_bits(), "{what} at {x:e}");
        };
        for k in -2000_i16..=2000 {
            let x = f32::from(k) * 0.0137 + 1.0e-3;
            eq(x.r_exp(), libm::expf(x), "expf", x);
            eq(x.r_exp2(), libm::exp2f(x), "exp2f", x);
            eq(x.r_exp_m1(), libm::expm1f(x), "expm1f", x);
            eq(x.r_cbrt(), libm::cbrtf(x), "cbrtf", x);
            eq(x.r_sin(), libm::sinf(x), "sinf", x);
            eq(x.r_cos(), libm::cosf(x), "cosf", x);
            eq(x.r_tan(), libm::tanf(x), "tanf", x);
            eq(x.r_atan(), libm::atanf(x), "atanf", x);
            eq(x.r_sinh(), libm::sinhf(x), "sinhf", x);
            eq(x.r_cosh(), libm::coshf(x), "coshf", x);
            eq(x.r_tanh(), libm::tanhf(x), "tanhf", x);
            eq(x.r_atan2(0.7 - x), libm::atan2f(x, 0.7 - x), "atan2f", x);
            eq(
                x.r_hypot(3.0 * x - 2.0),
                crate::real::hypotf(x, 3.0 * x - 2.0),
                "hypotf",
                x,
            );
            let (s, c) = x.r_sin_cos();
            eq(s, x.r_sin(), "sin_cos.0", x);
            eq(c, x.r_cos(), "sin_cos.1", x);
            let u = x / 30.0; // inside [-1, 1]
            eq(u.r_asin(), libm::asinf(u), "asinf", u);
            eq(u.r_acos(), libm::acosf(u), "acosf", u);
            if x > -1.0 {
                eq(x.r_ln_1p(), libm::log1pf(x), "log1pf", x);
            }
            if x > 0.0 {
                eq(x.r_ln(), libm::logf(x), "logf", x);
                eq(x.r_log10(), libm::log10f(x), "log10f", x);
                eq(x.r_log2(), libm::log2f(x), "log2f", x);
                eq(x.r_powf(2.5), libm::powf(x, 2.5), "powf", x);
            }
            for n in [-7, -2, -1, 0, 1, 2, 3, 5, 12] {
                eq(x.r_powi(n), <f32 as Real>::powi(x, n), "powi", x);
            }
        }
    }

    /// The `f32` exact small cases and special values survive the hop.
    #[test]
    fn the_f32_exact_cases_and_special_values_pass_through() {
        assert_eq!(8.0_f32.r_log2(), 3.0);
        assert_eq!(3.0_f32.r_exp2(), 8.0);
        assert_eq!(2.0_f32.r_powi(10), 1024.0);
        assert_eq!(2.0_f32.r_powi(-2), 0.25);
        assert_eq!(0.0_f32.r_sin_cos(), (0.0, 1.0));
        assert_eq!(0.0_f32.r_ln(), f32::NEG_INFINITY);
        assert!((-1.0_f32).r_ln().is_nan());
        assert_eq!(1000.0_f32.r_exp(), f32::INFINITY);
    }
}
