// Copyright (C) 2026 Theodore Ong and the outram-park contributors. GPL-3.0-only.

//! `f64` transcendentals as methods, so a crate can adopt the deterministic
//! route by renaming rather than rewriting.
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
//! **Exactly the three functions PETIR has a fast, deterministic port of** —
//! [`RealMath::r_exp`], [`RealMath::r_ln`], [`RealMath::r_powf`] — which route
//! to [`crate::fast_exp`], [`crate::fast_log`] and [`crate::fast_pow`], ports
//! of ARM optimized-routines verified bit-identical to upstream's own compiled
//! C. There is no feature gate: this *is* the default route, and it is at
//! worst comparable to the platform and at best faster than it.
//!
//! It deliberately does **not** cover `log10`, `cbrt`, `cos`, `sin`, `tanh` or
//! `atan2`. Those have no ARM scalar-double implementation, so PETIR can only
//! offer the `libm` (musl-derived) versions, which are deterministic but
//! measurably slower than the platform. That is a trade-off a crate should opt
//! into knowingly, so reach for [`crate::real`]'s free functions and mean it,
//! rather than getting it by importing a trait.
//!
//! Nor does it cover `sqrt`, `abs`, `floor`, `ceil`, `round` or `powi`.
//! IEEE-754 requires `sqrt` to be correctly rounded and the others are exact,
//! so they are **already** bit-identical on every platform. Routing them would
//! buy nothing and cost real time — `libm::sqrt` benchmarks 3.73x slower than
//! the hardware instruction.
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

/// `f64` transcendentals routed through PETIR's ARM optimized-routines ports.
///
/// Method names carry an `r_` prefix so adoption is a rename, not an
/// expression rewrite, and so a converted call site reads as deliberate. See
/// the module docs for what is deliberately *not* here.
pub trait RealMath {
    /// Natural logarithm, `ln x`.
    ///
    /// Returns `-inf` at `x == ±0` and a NaN for `x < 0`, as the platform
    /// does. Routed to [`crate::fast_log`].
    fn r_ln(self) -> f64;

    /// `e^x`.
    ///
    /// Returns `+inf` on overflow and `+0.0` on underflow, as the platform
    /// does. Routed to [`crate::fast_exp`].
    fn r_exp(self) -> f64;

    /// `x^y` for real `y`.
    ///
    /// Full IEEE-754 semantics, including the sign of a result from a negative
    /// base with an odd-integer exponent. Routed to [`crate::fast_pow`].
    fn r_powf(self, y: f64) -> f64;
}

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
            assert_eq!(x.r_exp().to_bits(), crate::real::exp(x).to_bits(), "exp at {x}");
            if x > 0.0 {
                assert_eq!(x.r_ln().to_bits(), crate::real::ln(x).to_bits(), "ln at {x}");
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
            assert_eq!(x.r_powf(1.7).to_bits(), crate::fast_pow::powf_ieee(x, 1.7).to_bits());
        }
    }

    /// The IEEE special values must survive the extra hop.
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
}
