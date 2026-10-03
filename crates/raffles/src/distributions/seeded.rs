// SPDX-License-Identifier: GPL-3.0-only
// Copyright (C) 2026 OUTRAM PARK contributors
//
// NOT ported from RAVEN (unlike the parent `distributions` module). These are
// OpenMC-style samplers (`src/random_dist.cpp`) moved here 2026-10-02 from
// `outram-mc-libs/src/rng/distributions.rs` (GitHub issue #500) at the
// workspace maintainer's direction; owner review outstanding — see CLAUDE.md
// "Scope boundaries".

//! **Seed-driven generic samplers** on the workspace LCG
//! ([`petir::rng::lcg`]): a uniform on `[low, high)`, a standard normal by
//! Box-Muller, and an exponential by inverse CDF.
//!
//! # How these relate to the rest of [`crate::distributions`]
//!
//! The RAVEN-derived distributions sample by **inverse CDF from a supplied
//! uniform** (`Distribution::sample(u)`), so the caller owns the stream. These
//! take the LCG **seed** and advance it themselves, which is what a Monte
//! Carlo transport history does. They are not a second implementation of the
//! same thing: `sample_normal` is Box-Muller (two draws per deviate), not the
//! inverse normal CDF, and changing either into the other would change every
//! stream that consumes them. Both shapes now live in one module tree, and
//! there is one copy of each.
//!
//! # Exactly the stream they always drew
//!
//! Moved byte-for-byte from `outram_mc_libs::rng::distributions`, which
//! re-exports them, so `outram-mc-libs`, `boon-lay` and `nee_soon` call sites
//! see the same numbers. `ln` is always `petir::real::ln` (as it was through
//! outram-mc's `RealMath::r_ln`). `cos` follows outram-mc's routing: the
//! platform `f64::cos` by default, and [`petir::real::cos`] under this crate's
//! **`deterministic-math`** feature, which `outram-mc-libs`' own
//! `deterministic-math` feature forwards. Physics samplers (`maxwell`, `watt`,
//! `isotropic_direction`) stay in `outram-mc-libs`.

use std::f64::consts::PI;

use petir::rng::lcg::prn;

#[inline]
fn ln(x: f64) -> f64 {
    petir::real::ln(x)
}

#[cfg(not(feature = "deterministic-math"))]
#[inline]
fn cos(x: f64) -> f64 {
    x.cos()
}

#[cfg(feature = "deterministic-math")]
#[inline]
fn cos(x: f64) -> f64 {
    petir::real::cos(x)
}

/// A uniform deviate on `[low, high)`, one LCG draw.
#[inline]
pub fn uniform(seed: &mut u64, low: f64, high: f64) -> f64 {
    low + (high - low) * prn(seed)
}

/// A standard normal deviate `N(0, 1)` by the Box-Muller transform: two draws
/// `u1`, `u2`, returning `sqrt(-2 ln u1) · cos(2π u2)`. For `N(μ, σ²)` use
/// `μ + σ · sample_normal(seed)`.
///
/// `u1` is clamped to the smallest positive `f64` so `ln(0)` cannot occur.
#[inline]
pub fn sample_normal(seed: &mut u64) -> f64 {
    let u1 = prn(seed).max(f64::MIN_POSITIVE);
    let u2 = prn(seed);
    (-2.0 * ln(u1)).sqrt() * cos(2.0 * PI * u2)
}

/// A 3-vector of independent `N(0, σ²)` deviates (three [`sample_normal`]
/// calls, x then y then z).
#[inline]
pub fn sample_normal_3d(seed: &mut u64, sigma: f64) -> (f64, f64, f64) {
    (
        sigma * sample_normal(seed),
        sigma * sample_normal(seed),
        sigma * sample_normal(seed),
    )
}

/// An exponential deviate with rate `λ` (mean `1/λ`) by inverse CDF,
/// `-ln(u) / λ`, one draw.
#[inline]
pub fn sample_exp(seed: &mut u64, rate: f64) -> f64 {
    -ln(prn(seed)) / rate
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sample_normal_mean_and_variance() {
        let mut seed = 0xc0ffee_u64;
        let n = 100_000;
        let samples: Vec<f64> = (0..n).map(|_| sample_normal(&mut seed)).collect();
        let mean = samples.iter().sum::<f64>() / n as f64;
        let var = samples.iter().map(|x| (x - mean).powi(2)).sum::<f64>() / n as f64;
        assert!(mean.abs() < 0.02, "mean = {mean:.4}, expected ~0");
        assert!((var - 1.0).abs() < 0.02, "variance = {var:.4}, expected ~1");
    }

    #[test]
    fn sample_exp_mean() {
        let mut seed = 0xdeadbeef_u64;
        let rate = 2.5;
        let n = 100_000;
        let mean = (0..n).map(|_| sample_exp(&mut seed, rate)).sum::<f64>() / n as f64;
        let expected = 1.0 / rate;
        assert!(
            (mean - expected).abs() / expected < 0.01,
            "mean = {mean:.4}"
        );
    }

    #[test]
    fn uniform_stays_in_range() {
        let mut seed = 42_u64;
        for _ in 0..10_000 {
            let x = uniform(&mut seed, -5.0, 3.0);
            assert!((-5.0..3.0).contains(&x));
        }
    }
}
