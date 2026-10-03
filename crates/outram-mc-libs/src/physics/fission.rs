//! Fission neutron production.
//!
//! C++ source: `src/physics.cpp` — `fission()`, `create_fission_sites()`.
//!
//! For a k-eigenvalue calculation each fission collision banks an integer number
//! of secondary neutrons for the *next* generation. This module ports the
//! neutron-count sampler ~~; the fission-site energy/direction come from the source
//! samplers ([`crate::rng::distributions::watt`],
//! [`crate::rng::distributions::isotropic_direction`]) and the banking itself is
//! driven by the eigenvalue loop ([`crate::physics::keff`]).~~
//! and the fission-bank resampler ([`comb_resample`]).
//!
//! **CORRECTED 2026-10-03.** In the continuous-energy kernels a fission site's
//! direction is [`crate::rng::distributions::isotropic_direction`], but its
//! energy is drawn by
//! [`Nuclide::sample_fission_energy_below`](crate::material::nuclide::Nuclide::sample_fission_energy_below)
//! from the nuclide's own χ (ENDF MF=5, or the Watt stand-in only where no law
//! is carried). [`crate::rng::distributions::watt`] seeds only the *initial*
//! source of the [`crate::physics::keff`] and
//! [`crate::pebble_beds::keff_delta`] drivers. The banking is done in
//! [`crate::physics::keff`] and in
//! [`crate::physics::transport_csg`]'s history loop.
//!
//! **Delayed neutrons** are folded into the total ν̄ — the
//! standard eigenvalue approximation (prompt + delayed born at the same instant).
//! ~~and treated as prompt~~ **CORRECTED 2026-10-03:** they are *counted* in ν̄
//! and born at the same instant, but where the nuclide carries delayed-neutron
//! spectra a delayed neutron's energy is drawn from its group's spectrum, not
//! the prompt χ (`Nuclide::sample_fission_energy_below`, GitHub #365).

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
    let k = if keff.is_finite() && keff > 0.0 {
        keff
    } else {
        1.0
    };
    let expected = nu_bar / k;
    let mut n = expected.floor();
    if prn(seed) < expected - n {
        n += 1.0;
    }
    n.max(0.0) as usize
}

/// Draw the next generation's `n` source sites from a fission bank by
/// **uniform combing**, as OpenMC's `synchronize_bank` does
/// (`src/eigenvalue.cpp:150-175`, Uniform Combing method,
/// doi:10.1080/00295639.2022.2091906).
///
/// The teeth are `total/n` apart with one random offset in `[0, total/n)`, and
/// tooth `i` takes site `floor(offset + i * total/n)`. Every site is therefore
/// taken `floor(n/total)` or `ceil(n/total)` times, and exactly `n` sites come
/// back.
///
/// GitHub #460: this crate used to draw `n` sites independently with
/// replacement. That is unbiased per generation, but the multinomial noise it
/// adds enlarges the finite-N population-control bias, which is negative and
/// O(1/N). One variate is consumed, where the old sampler consumed `n`.
///
/// An empty bank returns an empty source; callers already guard against it,
/// as OpenMC treats it as fatal.
pub fn comb_resample<T: Copy>(bank: &[T], n: usize, seed: &mut u64) -> Vec<T> {
    let total = bank.len();
    if total == 0 || n == 0 {
        return Vec::new();
    }
    let teeth = total as f64 / n as f64;
    let offset = prn(seed) * teeth;
    (0..n)
        .map(|i| {
            let idx = ((offset + i as f64 * teeth).floor() as usize).min(total - 1);
            bank[idx]
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The comb takes every site `floor(n/total)` or `ceil(n/total)` times and
    /// returns exactly `n`. Sampling with replacement fails this: its
    /// multiplicities are multinomial, so some site is almost surely taken 0 or
    /// 3+ times. GitHub #460.
    #[test]
    fn comb_multiplicities_are_floor_or_ceil() {
        for &(total, n) in &[(1000usize, 1000usize), (1234, 1000), (700, 1000), (5000, 1000)] {
            let bank: Vec<usize> = (0..total).collect();
            let mut seed = 0xC0B_u64 + total as u64;
            let out = comb_resample(&bank, n, &mut seed);
            assert_eq!(out.len(), n);
            let mut count = vec![0usize; total];
            for i in out {
                count[i] += 1;
            }
            let lo = n / total;
            let hi = lo + usize::from(n % total != 0);
            assert!(
                count.iter().all(|&c| c == lo || c == hi),
                "total {total}, n {n}: multiplicities outside {{{lo}, {hi}}}"
            );
        }
    }

    /// Over many draws the mean integer count converges to ν̄/keff.
    #[test]
    fn mean_count_matches_expected() {
        let mut seed = 0xf1_5510u64;
        let (nu_bar, keff) = (2.44, 1.03);
        let n = 200_000;
        let total: usize = (0..n)
            .map(|_| sample_num_neutrons(nu_bar, keff, &mut seed))
            .sum();
        let mean = total as f64 / n as f64;
        let expected = nu_bar / keff;
        assert!(
            (mean - expected).abs() < 0.01,
            "mean {mean} vs expected {expected}"
        );
    }

    /// A degenerate keff is treated as 1 rather than producing garbage.
    #[test]
    fn non_positive_keff_is_safe() {
        let mut seed = 1u64;
        let counts: Vec<usize> = (0..100)
            .map(|_| sample_num_neutrons(2.5, 0.0, &mut seed))
            .collect();
        assert!(counts.iter().all(|&c| c == 2 || c == 3), "ν̄=2.5 ⇒ 2 or 3");
    }
}
