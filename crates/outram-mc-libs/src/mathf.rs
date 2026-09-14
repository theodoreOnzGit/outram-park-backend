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
//! # The shape: `exp`/`ln`/`powf` are always deterministic; the rest is a feature
//!
//! Every transcendental call goes through [`RealMath`], and the three hot ones
//! — [`RealMath::r_exp`], [`RealMath::r_ln`] and [`RealMath::r_powf`] — go to
//! PETIR **unconditionally, in every build**. The remaining six (`log10`,
//! `cbrt`, `cos`, `sin`, `tanh`, `atan2`) still switch on the
//! **`deterministic-math`** feature: `std` by default, `petir::real` when it
//! is on.
//!
//! ## Why the split moved (2026-09-14)
//!
//! `bn:op-chyp.5` originally asked for an unconditional swap to `petir::real`.
//! Measurement argued against it, because `petir::real` was then a pure `libm`
//! (musl-derived) route and `libm` is slower than the platform:
//!
//! ```text
//!         std vs libm          benchmark, 20M calls
//!   exp    9.64 % of calls     std 125 ms   libm 219 ms   1.74x SLOWER
//!   ln     1.68 %              std 121 ms   libm 174 ms   1.43x SLOWER
//!   cos    3.22 %              std 232 ms   libm 186 ms   0.80x (faster)
//! ```
//!
//! `ln` and `exp` are 68 of this crate's 126 transcendental sites and sit in
//! Monte Carlo inner loops, so paying 1.5-1.7x on every run for a property
//! that only matters *between* platforms was the wrong default.
//!
//! **That trade-off no longer exists for those three.** `petir::real::{exp,
//! ln, powf}` now route to PETIR's ports of ARM optimized-routines — the
//! implementation glibc itself ships — each verified 100.000 % bit-identical
//! to upstream's own compiled C. Measured from Rust over 2 000 000 calls per
//! route, five runs:
//!
//! ```text
//!            old libm route / ARM route      ARM route / platform
//!   exp            1.75 - 1.86x                   0.70 - 0.75x
//!   ln             1.09 - 1.18x                   1.04 - 1.25x
//!   powf           2.09 - 2.18x                   1.41 - 1.57x
//! ```
//!
//! `exp` is now *faster than the platform*, and `ln` is within a few per cent
//! of it. `powf` is the one that still costs — about 1.4-1.6x the platform —
//! but it is a small minority of this crate's sites and 2.1x better than the
//! route it replaces. So determinism on the three hot functions is no longer
//! something to opt into and pay for; it is simply the default.
//!
//! ## What this means for existing results
//!
//! **The bits moved once.** ARM's `exp` and musl's are different
//! implementations, and glibc's build of ARM's is FMA-contracted where this
//! port is not, so a default build's `exp`/`ln`/`powf` output is no longer
//! bit-identical to what it was. Anything pinned to full `f64` precision
//! needs re-baselining once. Nothing is *less* accurate: all three are within
//! about 1 ulp of the platform, and now identical on every platform, which
//! they were not before.
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
    /// Natural logarithm. **Always** PETIR's ARM optimized-routines port,
    /// in every build — see the module docs.
    fn r_ln(self) -> f64;
    /// `e^x`. **Always** PETIR's ARM optimized-routines port, in every build.
    fn r_exp(self) -> f64;
    /// Base-10 logarithm.
    fn r_log10(self) -> f64;
    /// Cube root.
    fn r_cbrt(self) -> f64;
    /// `x^y` for real `y`. **Always** PETIR's ARM optimized-routines port,
    /// in every build.
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
        // Not `self.ln()`: the ARM port is the default for these three in
        // every build. See the module docs.
        petir::real::ln(self)
    }
    #[inline]
    fn r_exp(self) -> f64 {
        petir::real::exp(self)
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
        petir::real::powf(self, y)
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
            assert!(
                (x.r_log10() - x.r_ln() / 10f64.r_ln()).abs() < 1e-13,
                "log10 at {x}"
            );
            assert!(
                (x.r_cbrt().r_powf(3.0) - x).abs() < 1e-11 * x,
                "cbrt at {x}"
            );
            let s = x.r_sin();
            let c = x.r_cos();
            assert!((s * s + c * c - 1.0).abs() < 1e-14, "sin^2+cos^2 at {x}");
            assert!(x.r_tanh().abs() <= 1.0, "tanh range at {x}");
            assert!(x.r_atan2(1.0).is_finite(), "atan2 at {x}");
        }
    }

    /// Even in a default build, `exp`/`ln`/`powf` must be PETIR's — that is
    /// the 2026-09-14 change, and this pins it rather than trusting the
    /// manifest or the module docs.
    #[test]
    fn exp_ln_and_powf_are_petir_in_every_build() {
        let x = 0.7_f64;
        assert_eq!(x.r_exp().to_bits(), petir::real::exp(x).to_bits());
        assert_eq!(x.r_ln().to_bits(), petir::real::ln(x).to_bits());
        assert_eq!(x.r_powf(3.1).to_bits(), petir::real::powf(x, 3.1).to_bits());
    }

    /// ...and that PETIR's route really is the ARM port, not the `libm` one it
    /// replaced. Checked here as well as in PETIR because this crate is what
    /// actually depends on the property.
    #[test]
    fn petirs_route_is_the_arm_port() {
        for k in 1..=200 {
            let x = f64::from(k) * 0.31;
            assert_eq!(
                x.r_exp().to_bits(),
                petir::fast_exp::exp_ieee(x).to_bits(),
                "exp at {x}"
            );
            assert_eq!(
                x.r_ln().to_bits(),
                petir::fast_log::ln_ieee(x).to_bits(),
                "ln at {x}"
            );
            assert_eq!(
                x.r_powf(2.5).to_bits(),
                petir::fast_pow::powf_ieee(x, 2.5).to_bits(),
                "powf at {x}"
            );
        }
    }

    /// The six functions that still switch must follow the feature.
    #[cfg(not(feature = "deterministic-math"))]
    #[test]
    fn the_remaining_functions_default_to_std() {
        let x = 0.7_f64;
        assert_eq!(x.r_cos().to_bits(), x.cos().to_bits());
        assert_eq!(x.r_sin().to_bits(), x.sin().to_bits());
        assert_eq!(x.r_cbrt().to_bits(), x.cbrt().to_bits());
    }

    /// With the feature on, the remaining six must match PETIR exactly — that
    /// is now the whole point of turning it on, since the three hot ones no
    /// longer depend on it.
    #[cfg(feature = "deterministic-math")]
    #[test]
    fn the_deterministic_feature_covers_the_remaining_functions() {
        let x = 0.7_f64;
        assert_eq!(x.r_cos().to_bits(), petir::real::cos(x).to_bits());
        assert_eq!(x.r_sin().to_bits(), petir::real::sin(x).to_bits());
        assert_eq!(x.r_cbrt().to_bits(), petir::real::cbrt(x).to_bits());
        assert_eq!(x.r_tanh().to_bits(), petir::real::tanh(x).to_bits());
    }
}
