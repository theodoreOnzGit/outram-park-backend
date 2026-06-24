/// Tally definition — filter composition and accumulator.
///
/// C++ source: `src/tallies/tally.cpp`, `include/openmc/tallies/tally.h`.
///
/// A `Tally` accumulates scores over a user-defined subset of the phase space,
/// filtered by a conjunction of `Filter`s (cell, energy bin, material, mesh, …).
///
/// Score types: flux, total reaction rate, fission, absorption, current, etc.
/// Multiple scores can be accumulated per tally.

use super::filter::Filter;

/// Score type.  Maps to `openmc::TallyScore`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ScoreType {
    Flux,
    Total,
    Fission,
    Absorption,
    NuFission,
    ScatterN,        // (n,xn) scatter
    Current,
    Events,
}

/// A single tally accumulator bin: running sum + sum-of-squares for statistics.
#[derive(Debug, Default, Clone)]
pub struct TallyBin {
    pub sum: f64,
    pub sum_sq: f64,
    pub count: u64,
}

impl TallyBin {
    pub fn score(&mut self, value: f64) {
        self.sum    += value;
        self.sum_sq += value * value;
        self.count  += 1;
    }

    /// Mean over `n_realizations` active batches.
    pub fn mean(&self, n_realizations: u64) -> f64 {
        if n_realizations == 0 { return 0.0; }
        self.sum / n_realizations as f64
    }

    /// Relative standard deviation (as fraction of mean).
    pub fn rel_std_dev(&self, n_realizations: u64) -> f64 {
        if n_realizations < 2 { return f64::INFINITY; }
        let n = n_realizations as f64;
        let mean = self.sum / n;
        if mean == 0.0 { return f64::INFINITY; }
        let variance = (self.sum_sq / n - mean * mean) / (n - 1.0);
        variance.sqrt() / mean.abs()
    }
}

/// A tally.  Maps to `openmc::Tally`.
pub struct Tally {
    pub id: i32,
    pub name: String,
    pub filters: Vec<Box<dyn Filter>>,
    pub scores: Vec<ScoreType>,
    /// Accumulated bins, indexed `[filter_bin * n_scores + score_idx]`.
    pub bins: Vec<TallyBin>,
}

impl Tally {
    /// Total number of bins = product of each filter's bin count × number of scores.
    pub fn n_bins(&self) -> usize {
        self.filters.iter().map(|f| f.n_bins()).product::<usize>() * self.scores.len()
    }
}
