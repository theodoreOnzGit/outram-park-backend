// SPDX-License-Identifier: GPL-3.0-only
// Copyright (C) 2026 OUTRAM PARK contributors
//
// NOT ported from RAVEN. Moved here 2026-10-02 from `outram-mc-libs` (GitHub
// issue #500) at the WORKSPACE MAINTAINER's direction; the crate owner's review
// is outstanding — see this crate's CLAUDE.md, "Scope boundaries". Each
// function names the outram-mc file it came from, and where that file was
// itself a port (OpenMC `src/tallies/trigger.cpp`, `src/tallies/tally.cpp`,
// `openmc/tally.py`) the upstream is named too. Every expression is moved
// byte-for-byte: the order of operations is part of the contract, because
// outram-mc pins k-eff results to the bit.

//! **Basic estimators** — the generic statistics a Monte Carlo code needs:
//! sample mean and standard error, seed pooling, Shannon entropy over counts,
//! running-sum (batch) uncertainty and batch prediction, and first-order error
//! propagation.
//!
//! # Why these live in RAFFLES
//!
//! The rule from GitHub issue #500: **the maths on numbers lives here; deciding
//! what is counted stays with the physics code.** `outram-mc-libs` still
//! chooses the active generations, bins the fission bank on its mesh, decides
//! which tally a trigger watches and which tallies are combined — and calls
//! these functions for the arithmetic. Before the move the sample mean/stderr
//! existed in four identical private copies in outram-mc's drivers; it now
//! exists once.
//!
//! # Bit-identity is a requirement, not a nicety
//!
//! Every function here reproduces its outram-mc original **bit for bit**: the
//! same summation order, the same `(n - 1)` placement, the same `sqrt` of a
//! quotient rather than a quotient of `sqrt`s. Two functions that compute "the
//! same" statistic are therefore kept apart where their rounding differs:
//! [`mean_and_stderr`] returns `sqrt(var / n)` and [`pooled`] returns
//! `sqrt(var) / sqrt(n)`, which are not bit-identical. Merging them would move
//! recorded results. `outram-mc-libs/tests/stats_move_fingerprints.rs` pins
//! k-eff runs through all of them.
//!
//! # Status
//!
//! No human V&V, like the rest of this crate. The formulas are the textbook
//! ones and each carries a unit test against a hand-computed value.

// ---------------------------------------------------------------------------
// Submodules added 2026-10-03 under epic #493 (GitHub #494, #495, #496), at the
// workspace maintainer's direction; the crate owner's review is outstanding
// (CLAUDE.md, "Scope boundaries"). They ADD functions and change nothing above,
// so the bit-identity contract on the moved functions is untouched.
// ---------------------------------------------------------------------------

/// Seed-ensemble consistency: `χ²/dof` of seed values against their internal
/// `σ`, a fixed 95 % band, leave-one-out outlier flags (GitHub #494).
pub mod seed_consistency;
/// Autocorrelation-corrected standard errors for a correlated sequence: batch
/// means, integrated autocorrelation time, effective sample size (GitHub #495).
pub mod autocorrelation;

/// Sample mean and **standard error of the mean** (1σ) of `x`.
///
/// ```text
/// mean = Σx / n
/// var  = Σ(x - mean)² / (n - 1)
/// sem  = sqrt(var / n)
/// ```
///
/// Returns `(0, 0)` for an empty slice and `(mean, 0)` for one sample — a
/// single realisation has no measurable spread.
///
/// **Moved from** `outram-mc-libs`, where four identical private copies
/// (`physics/keff.rs`, `physics/transport_csg.rs`, `physics/physics_mg.rs`,
/// `pebble_beds/keff_delta.rs`) computed the eigenvalue estimate over the
/// active generations. The four were diffed before merging and were
/// character-identical.
///
/// ```
/// let (m, s) = raffles::estimators::mean_and_stderr(&[1.0, 2.0, 3.0]);
/// assert_eq!(m, 2.0);
/// assert!((s - (1.0_f64 / 3.0).sqrt()).abs() < 1e-15);
/// ```
pub fn mean_and_stderr(x: &[f64]) -> (f64, f64) {
    let n = x.len();
    if n == 0 {
        return (0.0, 0.0);
    }
    let mean = x.iter().sum::<f64>() / n as f64;
    if n < 2 {
        return (mean, 0.0);
    }
    let var = x.iter().map(|v| (v - mean).powi(2)).sum::<f64>() / (n as f64 - 1.0);
    (mean, (var / n as f64).sqrt())
}

/// Pool independent samples — typically one result per RNG seed — into
/// `(mean, sd, sem)`.
///
/// - **`sd`** is the sample standard deviation (`n - 1`): the spread of one
///   realisation, i.e. what a single reproduction will give.
/// - **`sem = sd / √n`** is the uncertainty on the pooled mean; quote it when
///   comparing with a benchmark or another code.
///
/// Returns `(NaN, 0, 0)` for no samples and `(x, 0, 0)` for one.
///
/// **Moved from** `outram_mc_libs::vv::pooled`, which re-exports this. Note
/// `sem` is `sd / sqrt(n)`, not [`mean_and_stderr`]'s `sqrt(var / n)`: equal in
/// exact arithmetic, different in the last bit, and kept so.
///
/// ```
/// let (mean, sd, sem) = raffles::estimators::pooled(&[100.0, 200.0, 300.0]);
/// assert!((mean - 200.0).abs() < 1e-9);
/// assert!((sd - 100.0).abs() < 1e-9);
/// assert!((sem - 100.0 / 3.0_f64.sqrt()).abs() < 1e-9);
/// ```
pub fn pooled(x: &[f64]) -> (f64, f64, f64) {
    let n = x.len();
    if n == 0 {
        return (f64::NAN, 0.0, 0.0);
    }
    let mean = x.iter().sum::<f64>() / n as f64;
    if n == 1 {
        return (mean, 0.0, 0.0);
    }
    let var = x.iter().map(|v| (v - mean).powi(2)).sum::<f64>() / (n - 1) as f64;
    let sd = var.sqrt();
    (mean, sd, sd / (n as f64).sqrt())
}

/// Shannon entropy, **in bits**, of the distribution given by non-negative
/// `counts` (weights per bin): `H = -Σ p log₂ p` with `p = c / Σc`, empty bins
/// contributing nothing.
///
/// Ranges from 0 (everything in one bin) to `log₂(bins)` (uniform). Returns
/// `None` when the total is not positive — there is no distribution to take
/// an entropy of (OpenMC divides unconditionally and yields NaN there).
///
/// **Moved from** `outram_mc_libs::tally::mesh::RegularMesh::shannon_entropy`
/// (a port of OpenMC `src/mesh.cpp`'s entropy, base 2 as upstream's
/// `std::log2`), which still bins the fission bank and calls this.
///
/// ```
/// let h = raffles::estimators::shannon_entropy_bits(&[1.0, 1.0, 1.0, 1.0]).unwrap();
/// assert_eq!(h, 2.0);
/// assert_eq!(raffles::estimators::shannon_entropy_bits(&[0.0, 0.0]), None);
/// ```
pub fn shannon_entropy_bits(counts: &[f64]) -> Option<f64> {
    let total: f64 = counts.iter().sum();
    if !(total > 0.0) {
        return None;
    }
    let mut h = 0.0;
    for c in counts {
        let p = c / total;
        if p > 0.0 {
            h -= p * p.log2();
        }
    }
    Some(h)
}

/// Running sums for one quantity across realisations (batches).
///
/// **Moved from** `outram_mc_libs::tally::trigger::BinStats`, which re-exports
/// it.
#[derive(Debug, Clone, Copy, Default)]
pub struct BinStats {
    /// Sum of the per-realisation values.
    pub sum: f64,
    /// Sum of their squares.
    pub sum_sq: f64,
}

/// Mean, standard deviation **of the mean**, and relative error over `n`
/// realisations, from running sums:
///
/// ```text
/// mean    = sum / n
/// std_dev = sqrt( max(0, sum_sq / n - mean²) / (n - 1) )
/// rel_err = std_dev / |mean|
/// ```
///
/// The `(n - 1)` is outside the parenthesis, so this is the uncertainty on the
/// mean, not the spread of the realisations. The variance is clamped at 0 so
/// a cancelled variance reads as zero rather than NaN.
///
/// Returns `None` for `n < 2` (no estimate) or a mean of exactly zero ("no
/// contributions", which OpenMC signals with a `(-1, -1)` sentinel and which
/// must not be confused with "converged to zero").
///
/// **Moved from** `outram_mc_libs::tally::trigger::bin_uncertainty` (port of
/// OpenMC `get_tally_uncertainty`, `src/tallies/trigger.cpp:30`), which
/// re-exports it; the trigger *policy* (metrics, thresholds, which bins) stays
/// in outram-mc.
pub fn bin_uncertainty(stats: BinStats, n: usize) -> Option<(f64, f64, f64)> {
    if n < 2 {
        return None;
    }
    let nf = n as f64;
    let mean = stats.sum / nf;
    if mean == 0.0 {
        return None;
    }
    let std_dev = ((stats.sum_sq / nf - mean * mean) / (nf - 1.0))
        .max(0.0)
        .sqrt();
    let rel_err = std_dev / mean.abs();
    Some((mean, std_dev, rel_err))
}

/// Predicted total batches for an uncertainty that is `ratio` times its
/// target, assuming variance falls as `1/N`:
///
/// ```text
/// n_pred = floor(n_active * ratio²) + n_inactive + 1,  n_active = current - n_inactive
/// ```
///
/// `None` for a non-finite ratio (no basis for an estimate).
///
/// **Moved from** `outram_mc_libs::tally::trigger::predict_batches` (OpenMC
/// `src/tallies/trigger.cpp:209-215`), which re-exports it.
pub fn predict_batches(current_batch: usize, n_inactive: usize, ratio: f64) -> Option<usize> {
    if !ratio.is_finite() {
        return None;
    }
    let n_active = current_batch.saturating_sub(n_inactive) as f64;
    Some((n_active * ratio * ratio) as usize + n_inactive + 1)
}

/// First-order, **uncorrelated** error propagation for `a ± b`:
/// `σ = sqrt(σa² + σb²)`.
///
/// **Moved from** `outram_mc_libs::tally::arithmetic` (`DerivedTally::add` /
/// `sub`, after `openmc/tally.py`). Two operands that are in fact correlated
/// (`a + a`) get the uncorrelated answer, by design.
#[inline]
pub fn sigma_sum(sa: f64, sb: f64) -> f64 {
    (sa * sa + sb * sb).sqrt()
}

/// Relative-variance term `(σ/x)²`, taken as `0` for `x == 0` so an exact-zero
/// operand carries no relative uncertainty instead of a NaN.
#[inline]
fn rel_var(x: f64, s: f64) -> f64 {
    if x == 0.0 {
        0.0
    } else {
        (s / x).powi(2)
    }
}

/// `a · b` and its first-order uncorrelated σ, `|a·b| · sqrt((σa/a)² + (σb/b)²)`.
///
/// **Moved from** `outram_mc_libs::tally::arithmetic::DerivedTally::mul`.
#[inline]
pub fn product_with_sigma(a: f64, sa: f64, b: f64, sb: f64) -> (f64, f64) {
    let v = a * b;
    (v, v.abs() * (rel_var(a, sa) + rel_var(b, sb)).sqrt())
}

/// `a / b` and its first-order uncorrelated σ, `|a/b| · sqrt((σa/a)² + (σb/b)²)`.
/// A zero `b` gives a non-finite value; guarding against it is the caller's job.
///
/// **Moved from** `outram_mc_libs::tally::arithmetic::DerivedTally::div`.
#[inline]
pub fn quotient_with_sigma(a: f64, sa: f64, b: f64, sb: f64) -> (f64, f64) {
    let v = a / b;
    (v, v.abs() * (rel_var(a, sa) + rel_var(b, sb)).sqrt())
}

/// Sum of values and the quadrature sum of their σ, `(Σv, sqrt(Σσ²))`.
///
/// **Moved from** `outram_mc_libs::tally::arithmetic::DerivedTally::sum`
/// (`openmc.Tally.summation`).
pub fn sum_with_sigma(values: &[f64], sigmas: &[f64]) -> (f64, f64) {
    let s: f64 = values.iter().sum();
    let var: f64 = sigmas.iter().map(|x| x * x).sum();
    (s, var.sqrt())
}

#[cfg(test)]
mod tests {
    use super::*;

    /// **Methodology.** Hand-computed values. `{1, 2, 3, 4, 5}`: mean 3,
    /// sample variance 2.5, so `sem = sqrt(2.5 / 5)`; the running-sum form
    /// gives the population variance 2 over `n - 1 = 4`, i.e. `sqrt(0.5)`,
    /// the same number by a different association.
    ///
    /// **Result** (2026-10-02): all within 1e-15.
    #[test]
    fn hand_computed_values() {
        let x = [1.0, 2.0, 3.0, 4.0, 5.0];
        let (m, s) = mean_and_stderr(&x);
        assert_eq!(m, 3.0);
        assert!((s - 0.5_f64.sqrt()).abs() < 1e-15);
        let (pm, psd, psem) = pooled(&x);
        assert_eq!(pm, 3.0);
        assert!((psd - 2.5_f64.sqrt()).abs() < 1e-15);
        assert!((psem - 0.5_f64.sqrt()).abs() < 1e-15);
        let st = BinStats {
            sum: 15.0,
            sum_sq: 55.0,
        };
        let (bm, bsd, brel) = bin_uncertainty(st, 5).unwrap();
        assert_eq!(bm, 3.0);
        assert!((bsd - 0.5_f64.sqrt()).abs() < 1e-15);
        assert!((brel - 0.5_f64.sqrt() / 3.0).abs() < 1e-15);
    }

    /// Degenerate inputs give the documented sentinels, never a panic.
    #[test]
    fn degenerate_inputs() {
        assert_eq!(mean_and_stderr(&[]), (0.0, 0.0));
        assert_eq!(mean_and_stderr(&[4.0]), (4.0, 0.0));
        assert!(pooled(&[]).0.is_nan());
        assert_eq!(pooled(&[4.0]), (4.0, 0.0, 0.0));
        assert!(bin_uncertainty(
            BinStats {
                sum: 1.0,
                sum_sq: 1.0
            },
            1
        )
        .is_none());
        assert!(bin_uncertainty(BinStats::default(), 10).is_none());
        assert_eq!(predict_batches(120, 20, f64::INFINITY), None);
        assert_eq!(predict_batches(120, 20, 2.0), Some(421));
        assert_eq!(shannon_entropy_bits(&[]), None);
        assert_eq!(shannon_entropy_bits(&[3.0, 0.0]), Some(0.0));
    }

    /// Error propagation against hand-computed values; a zero operand is
    /// finite.
    #[test]
    fn propagation() {
        assert!((sigma_sum(3.0, 4.0) - 5.0).abs() < 1e-15);
        let (v, s) = product_with_sigma(2.0, 0.2, 4.0, 0.8);
        assert_eq!(v, 8.0);
        assert!((s - 8.0 * 0.05_f64.sqrt()).abs() < 1e-12);
        let (q, qs) = quotient_with_sigma(2.0, 0.2, 4.0, 0.8);
        assert_eq!(q, 0.5);
        assert!((qs - 0.5 * 0.05_f64.sqrt()).abs() < 1e-12);
        assert!(product_with_sigma(2.0, 0.2, 0.0, 0.0).1.is_finite());
        let (t, ts) = sum_with_sigma(&[1.0, 2.0], &[3.0, 4.0]);
        assert_eq!((t, ts), (3.0, 5.0));
    }
}
