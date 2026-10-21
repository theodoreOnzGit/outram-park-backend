// Copyright (C) 2026 Theodore Ong and the outram-park contributors. GPL-3.0-only.

//! Deterministic float maths: every transcendental call in this crate goes to
//! PETIR, in every build, with no feature and no opt-out.
//!
//! # Why
//!
//! `std`'s `f64::exp`, `ln`, `cos` and friends are thin wrappers over the
//! **platform** libm. glibc versions, macOS and MSVC disagree in the last ulp,
//! and in Monte Carlo one different last bit in a direction cosine changes the
//! random walk from that history on. OutramMC promises the same bits for the
//! same seed on every platform, so the platform libm is never called.
//!
//! # The shape
//!
//! [`RealMath`] is PETIR's own trait ([`petir::mathf::RealMath`]), re-exported
//! so the 26 modules that import `crate::mathf::RealMath` keep compiling, and so
//! this crate and every other OUTRAM PARK crate share one trait. Its routes:
//! PETIR's ports of ARM optimized-routines for `exp`/`ln`/`powf` (verified
//! bit-identical to upstream's compiled C), compiler-rt's `__powidf2` for
//! `powi`, and the pure-Rust `libm` port of musl for the rest. All are
//! platform-independent.
//!
//! `sqrt`, `abs`, `floor`, `ceil` and `round` stay `std`: IEEE-754 requires
//! `sqrt` to be correctly rounded and the others are exact, so they are already
//! bit-identical everywhere.
//!
//! # History
//!
//! ~~Six functions (`log10`, `cbrt`, `cos`, `sin`, `tanh`, `atan2`) went to the
//! platform libm unless the off-by-default `deterministic-math` feature was on,
//! and `powi`/`hypot` and many direct `std` calls bypassed the trait
//! entirely.~~ **CORRECTED 2026-10-20 (maintainer: "always enabled, never a
//! feature, never for the user to choose", gh:#985).** The feature was
//! removed and every direct call converted. Found because
//! `tests/delta_tracking_bit_identity.rs` gave different bits on the Linux Mint
//! host (Ubuntu glibc 2.35) than on the host that pinned it, with no commit
//! moving them: turning the PETIR route on for the six moved every
//! direction-dependent fingerprint and left the trig-free `majorants`
//! fingerprint bit-identical. The fingerprint gates were re-pinned once.
//!
//! ~~`tests/no_platform_libm.rs` fails if a direct `std` transcendental call
//! comes back.~~ **CORRECTED 2026-10-20:** that file never existed in this
//! crate. The gate is workspace-wide, `crates/petir/tests/no_platform_libm.rs`,
//! and covers this crate's `src/` (gh:#985).

pub use petir::mathf::RealMath;

#[cfg(test)]
mod tests {
    use super::*;

    /// The identities hold through the routed functions.
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

    /// Every routed function is PETIR's, bit for bit, in this (the only)
    /// build. Pins the default rather than trusting the module docs.
    #[test]
    fn every_route_is_petir() {
        for k in 1..=200 {
            let x = f64::from(k) * 0.031;
            assert_eq!(x.r_exp().to_bits(), petir::real::exp(x).to_bits(), "exp {x}");
            assert_eq!(x.r_ln().to_bits(), petir::real::ln(x).to_bits(), "ln {x}");
            assert_eq!(x.r_powf(2.5).to_bits(), petir::real::powf(x, 2.5).to_bits(), "powf {x}");
            assert_eq!(x.r_powi(3).to_bits(), petir::real::powi(x, 3).to_bits(), "powi {x}");
            assert_eq!(x.r_log10().to_bits(), petir::real::log10(x).to_bits(), "log10 {x}");
            assert_eq!(x.r_cbrt().to_bits(), petir::real::cbrt(x).to_bits(), "cbrt {x}");
            assert_eq!(x.r_cos().to_bits(), petir::real::cos(x).to_bits(), "cos {x}");
            assert_eq!(x.r_sin().to_bits(), petir::real::sin(x).to_bits(), "sin {x}");
            assert_eq!(x.r_tanh().to_bits(), petir::real::tanh(x).to_bits(), "tanh {x}");
            assert_eq!(x.r_atan2(0.7).to_bits(), petir::real::atan2(x, 0.7).to_bits(), "atan2 {x}");
        }
    }

    /// ...and PETIR's `exp`/`ln`/`powf` route really is the ARM port.
    #[test]
    fn petirs_route_is_the_arm_port() {
        for k in 1..=200 {
            let x = f64::from(k) * 0.31;
            assert_eq!(x.r_exp().to_bits(), petir::fast_exp::exp_ieee(x).to_bits(), "exp at {x}");
            assert_eq!(x.r_ln().to_bits(), petir::fast_log::ln_ieee(x).to_bits(), "ln at {x}");
            assert_eq!(
                x.r_powf(2.5).to_bits(),
                petir::fast_pow::powf_ieee(x, 2.5).to_bits(),
                "powf at {x}"
            );
        }
    }
}
