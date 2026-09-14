// Copyright (C) 2026 Theodore Ong and the outram-park contributors. GPL-3.0-only.

//! Deterministic float maths — the crate's transcendental calls, routed so the
//! backend can be chosen (`bn:op-chyp.5`).
//!
//! # The problem
//!
//! `std`'s `f64::exp`, `ln`, `cos` and friends are thin wrappers over the
//! **platform** libm. glibc, macOS and MSVC disagree in the last ulp, so a
//! k-eff computed on Windows can differ from Linux in its final digits for no
//! physical reason. This crate's drivers document thread-count-independent
//! results and its committed fixtures inherit the same fragility.
//!
//! # The shape, and why it is a feature rather than a straight swap
//!
//! Every transcendental call goes through [`RealMath`]. With the
//! **`deterministic-math`** feature on, it resolves to `petir::real`, a single
//! musl-derived implementation that is bit-identical on every platform. With
//! the feature off — the default — it resolves to `std`, exactly as before.
//!
//! `bn:op-chyp.5` originally asked for an unconditional swap. Measurement
//! argued against that, and the numbers are why this is a switch:
//!
//! ```text
//!         std vs libm          benchmark, 20M calls
//!   exp    9.64 % of calls     std 125 ms   libm 219 ms   1.74x SLOWER
//!   ln     1.68 %              std 121 ms   libm 174 ms   1.43x SLOWER
//!   cos    3.22 %              std 232 ms   libm 186 ms   0.80x (faster)
//! ```
//!
//! `ln` and `exp` are 68 of this crate's 126 transcendental sites and sit in
//! Monte Carlo inner loops. Paying ~1.5-1.7x on every run buys a property that
//! only matters when comparing results *between* platforms — so the cost is
//! opt-in, and a reproducibility or cross-platform-fixture run turns it on.
//!
//! # What is deliberately NOT routed
//!
//! `sqrt`, `abs`, `floor`, `ceil`, `round`, `powi`. IEEE-754 requires `sqrt`
//! to be correctly rounded and the others are exact, so they are **already**
//! bit-identical everywhere — 0.0000 % mismatch over 200 000 points. Routing
//! them would buy nothing and cost real time (`libm::sqrt` benchmarks 3.73x
//! slower than the hardware instruction). That is 562 of the ~700 float call
//! sites in this crate, left alone on purpose.

/// The transcendental functions this crate routes.
///
/// Method names are prefixed `r_` so the call sites read as a deliberate
/// choice rather than shadowing the inherent `f64` methods — and so the sweep
/// that introduced them was a mechanical rename rather than an expression
/// rewrite.
pub trait RealMath {
    /// Natural logarithm.
    fn r_ln(self) -> f64;
    /// `e^x`.
    fn r_exp(self) -> f64;
    /// Base-10 logarithm.
    fn r_log10(self) -> f64;
    /// Cube root.
    fn r_cbrt(self) -> f64;
    /// `x^y` for real `y`.
    fn r_powf(self, y: f64) -> f64;
    /// Cosine.
    fn r_cos(self) -> f64;
    /// Sine.
    fn r_sin(self) -> f64;
    /// Hyperbolic tangent.
    fn r_tanh(self) -> f64;
    /// Two-argument arctangent.
    fn r_atan2(self, x: f64) -> f64;
}

#[cfg(not(feature = "deterministic-math"))]
impl RealMath for f64 {
    #[inline]
    fn r_ln(self) -> f64 {
        self.ln()
    }
    #[inline]
    fn r_exp(self) -> f64 {
        self.exp()
    }
    #[inline]
    fn r_log10(self) -> f64 {
        self.log10()
    }
    #[inline]
    fn r_cbrt(self) -> f64 {
        self.cbrt()
    }
    #[inline]
    fn r_powf(self, y: f64) -> f64 {
        self.powf(y)
    }
    #[inline]
    fn r_cos(self) -> f64 {
        self.cos()
    }
    #[inline]
    fn r_sin(self) -> f64 {
        self.sin()
    }
    #[inline]
    fn r_tanh(self) -> f64 {
        self.tanh()
    }
    #[inline]
    fn r_atan2(self, x: f64) -> f64 {
        self.atan2(x)
    }
}

#[cfg(feature = "deterministic-math")]
impl RealMath for f64 {
    #[inline]
    fn r_ln(self) -> f64 {
        petir::real::ln(self)
    }
    #[inline]
    fn r_exp(self) -> f64 {
        petir::real::exp(self)
    }
    #[inline]
    fn r_log10(self) -> f64 {
        petir::real::log10(self)
    }
    #[inline]
    fn r_cbrt(self) -> f64 {
        petir::real::cbrt(self)
    }
    #[inline]
    fn r_powf(self, y: f64) -> f64 {
        petir::real::powf(self, y)
    }
    #[inline]
    fn r_cos(self) -> f64 {
        petir::real::cos(self)
    }
    #[inline]
    fn r_sin(self) -> f64 {
        petir::real::sin(self)
    }
    #[inline]
    fn r_tanh(self) -> f64 {
        petir::real::tanh(self)
    }
    #[inline]
    fn r_atan2(self, x: f64) -> f64 {
        petir::real::atan2(self, x)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Whichever backend is selected, the identities must hold.
    #[test]
    fn the_routed_functions_satisfy_their_identities() {
        for k in 1..=40 {
            let x = f64::from(k) * 0.37;
            assert!((x.r_ln().r_exp() - x).abs() < 1e-12 * x, "ln/exp at {x}");
            assert!((x.r_log10() - x.r_ln() / 10f64.r_ln()).abs() < 1e-13, "log10 at {x}");
            assert!((x.r_cbrt().r_powf(3.0) - x).abs() < 1e-11 * x, "cbrt at {x}");
            let s = x.r_sin();
            let c = x.r_cos();
            assert!((s * s + c * c - 1.0).abs() < 1e-14, "sin^2+cos^2 at {x}");
            assert!(x.r_tanh().abs() <= 1.0, "tanh range at {x}");
            assert!(x.r_atan2(1.0).is_finite(), "atan2 at {x}");
        }
    }

    /// The default build must be `std`-backed, i.e. free of the measured
    /// 1.4-1.7x cost. This pins the default rather than trusting the manifest.
    #[cfg(not(feature = "deterministic-math"))]
    #[test]
    fn the_default_backend_is_std() {
        let x = 0.7_f64;
        assert_eq!(x.r_exp().to_bits(), x.exp().to_bits());
        assert_eq!(x.r_ln().to_bits(), x.ln().to_bits());
    }

    /// With the feature on, results must match PETIR exactly — that is the
    /// whole point of turning it on.
    #[cfg(feature = "deterministic-math")]
    #[test]
    fn the_deterministic_backend_is_petir() {
        let x = 0.7_f64;
        assert_eq!(x.r_exp().to_bits(), petir::real::exp(x).to_bits());
        assert_eq!(x.r_ln().to_bits(), petir::real::ln(x).to_bits());
    }
}
