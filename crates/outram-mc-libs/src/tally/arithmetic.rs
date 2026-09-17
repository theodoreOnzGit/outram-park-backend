//! Derived-tally arithmetic — combine tally results with uncertainty propagation.
//!
//! # Provenance: mirroring OpenMC's *Python* tally algebra
//!
//! Unlike the transport/geometry/scoring kernels, tally arithmetic does **not**
//! exist in the OpenMC C++ core (`src/tallies/*.cpp`). In OpenMC it lives entirely
//! in the Python post-processing layer — `openmc/tally.py` overloads `+ - * /` on
//! `Tally` objects and produces a derived tally whose `mean`/`std_dev` arrays carry
//! the propagated statistics (see `openmc.Tally.__add__`/`__sub__`/`__mul__`/
//! `__div__` and `Tally.get_slice`/`Tally.summation`). So per the crate's porting
//! rule this module is the **sanctioned scaffold-new-work path**: there is no C++
//! function to mirror, so we mirror the *documented Python semantics* instead, and
//! flag it as new work rather than a port.
//!
//! # What it computes
//!
//! A [`DerivedTally`] is a small value-with-uncertainty vector: parallel arrays of
//! bin **means** and **absolute standard deviations**, read out of a scored
//! [`Tally`] via [`DerivedTally::from_tally`] / [`DerivedTally::from_tally_score`].
//! Elementwise `+ - * /`, scalar multiply, bin summation, slicing and indexing are
//! provided, each propagating uncertainty under the **standard uncorrelated
//! (first-order) error-propagation** rules OpenMC's Python layer uses:
//!
//! - add / sub: `σ = sqrt(σa² + σb²)`
//! - mul:       `σ = |a·b|·sqrt((σa/a)² + (σb/b)²)`
//! - div:       `σ = |a/b|·sqrt((σa/a)² + (σb/b)²)`
//! - scalar·k:  `σ = |k|·σa`
//! - sum:       `σ = sqrt(Σ σᵢ²)`
//!
//! These assume the operands are **uncorrelated**, exactly as OpenMC's tally
//! arithmetic does; combining a tally with itself (e.g. `a + a`) therefore reports
//! a larger σ than the exact `2·a` (`scalar_mul`), which is the correct behaviour
//! for the independent-samples assumption and is asserted in the verification test.
//!
//! No `Box`, no trait objects, no lifetime parameters — a plain owned struct of two
//! `Vec<f64>` (per the crate's Rust design rules).

use super::tally::{Tally, TallyBin};

/// A derived tally result: parallel arrays of per-bin **mean** and **absolute
/// standard deviation**.
///
/// Units follow whatever score produced it (flux in track-length units, a reaction
/// rate in `w·d·Σ_x`, etc. — this layer is unit-agnostic and only carries the
/// numbers). `values[i]` is bin `i`'s mean estimate and `std_devs[i]` its 1σ
/// absolute uncertainty; the two vectors always have equal length.
#[derive(Debug, Clone, PartialEq)]
pub struct DerivedTally {
    /// Per-bin mean estimate.
    pub values: Vec<f64>,
    /// Per-bin absolute standard deviation (1σ), same length as `values`.
    pub std_devs: Vec<f64>,
}

/// Read one tally bin's `(mean, absolute_std_dev)` over `n_realizations` batches.
///
/// The absolute std dev is `mean · rel_std_dev`. [`TallyBin::rel_std_dev`] returns
/// `∞` for a degenerate bin (fewer than two realizations, or a zero mean); in that
/// case there is no meaningful spread to report, so the absolute std dev is taken
/// as `0.0` rather than propagating a `NaN`.
fn bin_mean_std(bin: &TallyBin, n_realizations: u64) -> (f64, f64) {
    let mean = bin.mean(n_realizations);
    let rel = bin.rel_std_dev(n_realizations);
    let std = if rel.is_finite() {
        mean.abs() * rel
    } else {
        0.0
    };
    (mean, std)
}

/// Relative-variance term `(σ/x)²`, guarded so a zero value contributes `0`
/// instead of a `NaN`/`∞` (the value's own magnitude then dominates the product
/// in [`DerivedTally::mul`]/[`DerivedTally::div`], matching OpenMC's convention of
/// treating an exact-zero bin as carrying no relative uncertainty).
fn rel_var(x: f64, s: f64) -> f64 {
    if x == 0.0 {
        0.0
    } else {
        (s / x).powi(2)
    }
}

impl DerivedTally {
    /// Build a derived tally from **all** of a tally's bins, flat, in stored order.
    ///
    /// Use this for a single-score tally (one score type), where each bin is a
    /// distinct filter bin. For a multi-score tally use
    /// [`DerivedTally::from_tally_score`] to extract one score across the filter
    /// bins. `n_realizations` is the number of active batches the tally accumulated
    /// (its per-bin `count`).
    pub fn from_tally(tally: &Tally, n_realizations: u64) -> DerivedTally {
        let mut values = Vec::with_capacity(tally.bins.len());
        let mut std_devs = Vec::with_capacity(tally.bins.len());
        for bin in &tally.bins {
            let (m, s) = bin_mean_std(bin, n_realizations);
            values.push(m);
            std_devs.push(s);
        }
        DerivedTally { values, std_devs }
    }

    /// Extract a single score (`score_idx`) across every filter bin of a
    /// multi-score tally.
    ///
    /// Tally bins are laid out `[filter_bin * n_scores + score_idx]` (see
    /// [`Tally`]); this strides over the filter bins picking one score, giving a
    /// per-filter-bin derived tally for that score. `n_realizations` is the number
    /// of active batches. Panics if `score_idx >= tally.scores.len()`.
    pub fn from_tally_score(tally: &Tally, score_idx: usize, n_realizations: u64) -> DerivedTally {
        let n_scores = tally.scores.len().max(1);
        assert!(
            score_idx < n_scores,
            "score_idx {score_idx} out of range (n_scores {n_scores})"
        );
        let n_fbins = tally.bins.len() / n_scores;
        let mut values = Vec::with_capacity(n_fbins);
        let mut std_devs = Vec::with_capacity(n_fbins);
        for fb in 0..n_fbins {
            let (m, s) = bin_mean_std(&tally.bins[fb * n_scores + score_idx], n_realizations);
            values.push(m);
            std_devs.push(s);
        }
        DerivedTally { values, std_devs }
    }

    /// Construct directly from parallel mean / std-dev arrays (must be equal length).
    pub fn new(values: Vec<f64>, std_devs: Vec<f64>) -> DerivedTally {
        assert_eq!(
            values.len(),
            std_devs.len(),
            "values and std_devs must have equal length"
        );
        DerivedTally { values, std_devs }
    }

    /// Number of bins.
    pub fn len(&self) -> usize {
        self.values.len()
    }

    /// Whether the derived tally has no bins.
    pub fn is_empty(&self) -> bool {
        self.values.is_empty()
    }

    /// The `(mean, std_dev)` of bin `i`. Panics if out of range.
    pub fn get(&self, i: usize) -> (f64, f64) {
        (self.values[i], self.std_devs[i])
    }

    /// A contiguous sub-range `[start, end)` of bins as a new derived tally
    /// (mirrors `openmc.Tally.get_slice`). Panics if the range is out of bounds.
    pub fn slice(&self, start: usize, end: usize) -> DerivedTally {
        DerivedTally {
            values: self.values[start..end].to_vec(),
            std_devs: self.std_devs[start..end].to_vec(),
        }
    }

    /// Elementwise sum `a + b` with `σ = sqrt(σa² + σb²)`. Panics on length mismatch.
    pub fn add(&self, other: &DerivedTally) -> DerivedTally {
        self.zip_map(other, |a, sa, b, sb| (a + b, (sa * sa + sb * sb).sqrt()))
    }

    /// Elementwise difference `a - b` with `σ = sqrt(σa² + σb²)`. Panics on length
    /// mismatch.
    pub fn sub(&self, other: &DerivedTally) -> DerivedTally {
        self.zip_map(other, |a, sa, b, sb| (a - b, (sa * sa + sb * sb).sqrt()))
    }

    /// Elementwise product `a · b` with `σ = |a·b|·sqrt((σa/a)² + (σb/b)²)`.
    /// Panics on length mismatch.
    pub fn mul(&self, other: &DerivedTally) -> DerivedTally {
        self.zip_map(other, |a, sa, b, sb| {
            let v = a * b;
            (v, v.abs() * (rel_var(a, sa) + rel_var(b, sb)).sqrt())
        })
    }

    /// Elementwise quotient `a / b` with `σ = |a/b|·sqrt((σa/a)² + (σb/b)²)`.
    /// Panics on length mismatch. A zero denominator yields a non-finite value —
    /// the caller must guard against zero-flux bins (the verification test does).
    pub fn div(&self, other: &DerivedTally) -> DerivedTally {
        self.zip_map(other, |a, sa, b, sb| {
            let v = a / b;
            (v, v.abs() * (rel_var(a, sa) + rel_var(b, sb)).sqrt())
        })
    }

    /// Scale every bin by an **exact** scalar `k`: value `k·a`, `σ = |k|·σa`.
    ///
    /// Because `k` is exact (no uncertainty), this is *not* the same as adding a
    /// tally to itself `k` times — `scalar_mul(2)` gives `σ = 2σa`, whereas
    /// `add(a, a)` gives `σ = σa·√2` under the uncorrelated assumption.
    pub fn scalar_mul(&self, k: f64) -> DerivedTally {
        DerivedTally {
            values: self.values.iter().map(|a| a * k).collect(),
            std_devs: self.std_devs.iter().map(|s| s.abs() * k.abs()).collect(),
        }
    }

    /// Reduce all bins to a single `(sum, σ)` with `σ = sqrt(Σ σᵢ²)` (mirrors
    /// `openmc.Tally.summation`).
    pub fn sum(&self) -> (f64, f64) {
        let s: f64 = self.values.iter().sum();
        let var: f64 = self.std_devs.iter().map(|x| x * x).sum();
        (s, var.sqrt())
    }

    /// Shared elementwise combinator: apply `f(a, σa, b, σb) -> (value, σ)` bin by
    /// bin. Panics if the two derived tallies differ in length.
    fn zip_map(
        &self,
        other: &DerivedTally,
        f: impl Fn(f64, f64, f64, f64) -> (f64, f64),
    ) -> DerivedTally {
        assert_eq!(self.len(), other.len(), "derived-tally length mismatch");
        let mut values = Vec::with_capacity(self.len());
        let mut std_devs = Vec::with_capacity(self.len());
        for i in 0..self.len() {
            let (v, s) = f(
                self.values[i],
                self.std_devs[i],
                other.values[i],
                other.std_devs[i],
            );
            values.push(v);
            std_devs.push(s);
        }
        DerivedTally { values, std_devs }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn dt(v: &[f64], s: &[f64]) -> DerivedTally {
        DerivedTally::new(v.to_vec(), s.to_vec())
    }

    /// add/sub propagate σ in quadrature; `(a+b)-b` recovers `a`'s means and grows σ.
    #[test]
    fn add_sub_roundtrip() {
        let a = dt(&[4.0, 9.0], &[0.2, 0.3]);
        let b = dt(&[1.0, 2.0], &[0.1, 0.15]);
        let back = a.add(&b).sub(&b);
        for i in 0..a.len() {
            assert!((back.values[i] - a.values[i]).abs() < 1e-12);
            assert!(back.std_devs[i] > a.std_devs[i], "σ must grow, not shrink");
        }
    }

    /// `a*2 == a+a` in the mean; σ differs (2σ vs σ√2) — the uncorrelated assumption.
    #[test]
    fn scalar_vs_self_add() {
        let a = dt(&[3.0], &[0.5]);
        let doubled = a.scalar_mul(2.0);
        let added = a.add(&a);
        assert!((doubled.values[0] - added.values[0]).abs() < 1e-12);
        assert!((doubled.std_devs[0] - 1.0).abs() < 1e-12); // 2·0.5
        assert!((added.std_devs[0] - 0.5 * 2f64.sqrt()).abs() < 1e-12);
    }

    /// mul/div relative errors add in quadrature; a zero operand contributes none.
    #[test]
    fn mul_div_propagation() {
        let a = dt(&[2.0], &[0.2]); // 10% rel
        let b = dt(&[4.0], &[0.8]); // 20% rel
        let p = a.mul(&b);
        assert!((p.values[0] - 8.0).abs() < 1e-12);
        let expect = 8.0 * (0.01f64 + 0.04).sqrt();
        assert!((p.std_devs[0] - expect).abs() < 1e-12);
        // zero-value operand: rel term guarded to 0.
        let z = dt(&[0.0], &[0.0]);
        let pz = a.mul(&z);
        assert_eq!(pz.values[0], 0.0);
        assert!(pz.std_devs[0].is_finite());
    }

    /// sum reduces bins with σ in quadrature.
    #[test]
    fn sum_reduction() {
        let a = dt(&[1.0, 2.0, 3.0], &[0.1, 0.2, 0.2]);
        let (s, sig) = a.sum();
        assert!((s - 6.0).abs() < 1e-12);
        assert!((sig - (0.01f64 + 0.04 + 0.04).sqrt()).abs() < 1e-12);
    }
}
