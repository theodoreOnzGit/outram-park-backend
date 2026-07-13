//! Fission neutron production.
//!
//! C++ source: `src/physics.cpp` — `fission()`, `create_fission_sites()`.
//!
//! For a k-eigenvalue calculation each fission collision banks an integer number
//! of secondary neutrons for the *next* generation. This module ports the
//! neutron-count sampler; the fission-site energy/direction come from the source
//! samplers ([`crate::rng::distributions::watt`],
//! [`crate::rng::distributions::isotropic_direction`]) and the banking itself is
//! driven by the eigenvalue loop ([`crate::physics::keff`]).
//!
//! **Delayed neutrons** are folded into the total ν̄ and treated as prompt — the
//! standard eigenvalue approximation (prompt + delayed born at the same instant).

use crate::rng::lcg::prn;

/// Sample the integer number of fission neutrons to bank from one fission event.
///
/// The expected yield per fission is ν̄; dividing by the running eigenvalue guess
/// `keff` keeps the fission bank's population stationary from generation to
/// generation (OpenMC's `create_fission_sites` normalisation). The expected
/// value `ν̄/keff` is split into a deterministic integer part plus a Bernoulli
/// draw on the fractional part:
///
/// `n = ⌊ν̄/keff⌋ + [ξ < frac(ν̄/keff)]`.
///
/// A non-positive or non-finite `keff` falls back to `keff = 1` so a diverging
/// first generation can't produce a nonsensical count.
pub fn sample_num_neutrons(nu_bar: f64, keff: f64, seed: &mut u64) -> usize {
    let k = if keff.is_finite() && keff > 0.0 { keff } else { 1.0 };
    let expected = nu_bar / k;
    let mut n = expected.floor();
    if prn(seed) < expected - n {
        n += 1.0;
    }
    n.max(0.0) as usize
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Over many draws the mean integer count converges to ν̄/keff.
    #[test]
    fn mean_count_matches_expected() {
        let mut seed = 0xf1_5510u64;
        let (nu_bar, keff) = (2.44, 1.03);
        let n = 200_000;
        let total: usize = (0..n).map(|_| sample_num_neutrons(nu_bar, keff, &mut seed)).sum();
        let mean = total as f64 / n as f64;
        let expected = nu_bar / keff;
        assert!((mean - expected).abs() < 0.01, "mean {mean} vs expected {expected}");
    }

    /// A degenerate keff is treated as 1 rather than producing garbage.
    #[test]
    fn non_positive_keff_is_safe() {
        let mut seed = 1u64;
        let counts: Vec<usize> =
            (0..100).map(|_| sample_num_neutrons(2.5, 0.0, &mut seed)).collect();
        assert!(counts.iter().all(|&c| c == 2 || c == 3), "ν̄=2.5 ⇒ 2 or 3");
    }
}
