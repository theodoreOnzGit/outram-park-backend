//! Generic bounded time-history ring buffer for plot panels.
//!
//! Every simulator built on this crate ends up needing a "keep the last `N`
//! samples for a trend graph" buffer, and as of the 2026-08-17 survey behind
//! bead `op-wqk.22.1` this workspace had grown **three** near-duplicate,
//! independently-bugged implementations of exactly that:
//!
//! - `HtgrPlotData` (`examples/htgr_sim_v1/app/state.rs`) -- a `Vec<[f64; 2]>`
//!   per series, capped with a free function that does `buf.remove(0)` on
//!   overflow: correct output, but an O(n) shift of the whole buffer on every
//!   single sample once it is full.
//! - `PagePlotData`, shared verbatim between `ciet_educational_simulator_v2`
//!   (`src/bin/ciet_educational_simulator_v2/ciet_simulator_v2/app/
//!   panels_and_pages/ciet_data.rs`) and `fhr_sim_v2`
//!   (`examples/fhr_sim_v2/app/graph_data/mod.rs`), both descended from the
//!   original CIET v1 GUI -- `Vec::insert(0, sample)` (newest-first) followed
//!   by allocating a fresh `Vec` of a fixed `NUM_DATA_PTS_IN_PLOTS = 4000`
//!   entries and copying the retained prefix back in, **every sample**. Worse
//!   than the HTGR version, not better.
//!
//! [`PlotHistory`] replaces all of these with one generic, `VecDeque`-backed
//! ring buffer: `push` is O(1) amortised (push the new sample, pop the
//! oldest only once capacity is exceeded -- no shifting, no reallocating,
//! no rebuilding), and iteration is oldest-first, matching the convention
//! `HtgrPlotData` already uses and that `egui_plot::PlotPoints` expects.
//!
//! **This module is additive.** As of this pass no existing call site has
//! been migrated onto it -- `examples/htgr_sim_v1/app/state.rs` is being
//! edited concurrently by another session (a fast-forward-control change),
//! and rewriting `ciet_educational_simulator_v2`'s / `fhr_sim_v2`'s plot
//! state is entangled with their own pacing loops (see `op-wqk.22.3`).
//! Migrating an existing simulator onto [`PlotHistory`] is tracked as
//! follow-up work once this type has landed and been reviewed.
//!
//! # What belongs in this module
//!
//! A generic, physics-free, plotting-library-free bounded history buffer,
//! and nothing that assumes a particular series shape, sampling cadence, or
//! GUI plotting crate. Deciding *what* to sample, *how often*, and *which
//! plotting widget* renders it is the calling simulator's job -- exactly as
//! it is today for `HtgrPlotData`'s sampler thread in
//! `examples/htgr_sim_v1/app/mod.rs::start_simulation`.

use std::collections::VecDeque;

/// A time-value sample pair `[t_seconds, value]`.
///
/// This is the exact shape `egui_plot::PlotPoints` accepts from a
/// `Vec<[f64; 2]>` and the shape `HtgrPlotData` already uses for each of its
/// six series -- a convenience alias for callers whose samples are a plain
/// `(time, value)` pair in SI units with no `uom` dimension attached (the
/// dimension has already been divided out, e.g. `power.get::<megawatt>()`,
/// before the sample is pushed).
pub type XySample = [f64; 2];

/// A bounded, oldest-first time-history buffer holding at most `N` samples
/// of `T`.
///
/// Once [`push`](Self::push) has been called more than `N` times, each
/// further push drops the single oldest retained sample -- a fixed-size
/// sliding window over the most recent `N` samples, with no unbounded
/// growth and no per-push reallocation.
///
/// `T` is left fully generic: an [`XySample`] for a single trend line, a
/// `uom`-typed tuple like `(Time, Power)` for a caller that wants to keep
/// dimensioned data as long as possible, or any other per-sample payload.
/// `N` is a `const` generic, not a runtime field, so the capacity of a given
/// history is fixed at its type and cannot silently drift between the
/// buffer and whatever reads it.
///
/// # Example
///
/// ```
/// use outram_park_digital_twin_engine::app_scaffold::plot_history::{PlotHistory, XySample};
///
/// let mut power_history: PlotHistory<XySample, 3> = PlotHistory::new();
/// power_history.push([0.0, 10.0]);
/// power_history.push([1.0, 12.0]);
/// power_history.push([2.0, 14.0]);
/// power_history.push([3.0, 16.0]); // oldest sample [0.0, 10.0] is dropped
///
/// let samples: Vec<XySample> = power_history.iter().copied().collect();
/// assert_eq!(samples, vec![[1.0, 12.0], [2.0, 14.0], [3.0, 16.0]]);
/// ```
pub struct PlotHistory<T, const N: usize> {
    buf: VecDeque<T>,
}

impl<T, const N: usize> PlotHistory<T, N> {
    /// An empty history with room for up to `N` samples.
    ///
    /// # Panics
    ///
    /// Panics if `N == 0` -- a zero-capacity history can never retain a
    /// sample, which is always a caller mistake (an unset/forgotten const
    /// generic) rather than a meaningful empty-on-purpose buffer.
    pub fn new() -> Self {
        assert!(N > 0, "PlotHistory capacity N must be at least 1");
        Self {
            buf: VecDeque::with_capacity(N),
        }
    }

    /// Push the newest `sample`, dropping the oldest retained sample first
    /// if the buffer is already at capacity `N`. O(1) amortised -- no shift,
    /// no reallocation of the retained samples.
    pub fn push(&mut self, sample: T) {
        if self.buf.len() >= N {
            self.buf.pop_front();
        }
        self.buf.push_back(sample);
    }

    /// Iterate the retained samples oldest-first -- the order
    /// `egui_plot::PlotPoints::from(Vec<[f64; 2]>)` and `HtgrPlotData`'s
    /// existing series both expect.
    pub fn iter(&self) -> impl Iterator<Item = &T> {
        self.buf.iter()
    }

    /// Number of samples currently retained (`0..=N`).
    pub fn len(&self) -> usize {
        self.buf.len()
    }

    /// `true` if no sample has been pushed yet (or [`clear`](Self::clear)
    /// was just called).
    pub fn is_empty(&self) -> bool {
        self.buf.is_empty()
    }

    /// The fixed maximum retained-sample count -- always `N`, exposed as a
    /// method so callers do not need to name the const generic themselves.
    pub const fn capacity(&self) -> usize {
        N
    }

    /// Drop every retained sample, e.g. when a simulator restarts a run from
    /// defaults (see [`super::crash`]'s "restart means a clean swap" rule --
    /// a fresh run should start plotting from an empty history, not carry
    /// over samples from the crashed one).
    pub fn clear(&mut self) {
        self.buf.clear();
    }

    /// The most recently pushed sample, or `None` if the history is empty.
    pub fn last(&self) -> Option<&T> {
        self.buf.back()
    }

    /// Clone the retained samples out as an oldest-first `Vec<T>` -- the
    /// shape most plotting widgets (`egui_plot::PlotPoints::from`, a CSV
    /// row writer) actually want to consume.
    pub fn to_vec(&self) -> Vec<T>
    where
        T: Clone,
    {
        self.buf.iter().cloned().collect()
    }
}

impl<T, const N: usize> Default for PlotHistory<T, N> {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn new_history_is_empty() {
        let history: PlotHistory<XySample, 4> = PlotHistory::new();
        assert!(history.is_empty());
        assert_eq!(history.len(), 0);
        assert_eq!(history.capacity(), 4);
        assert_eq!(history.last(), None);
    }

    #[test]
    #[should_panic(expected = "capacity N must be at least 1")]
    fn zero_capacity_panics() {
        let _: PlotHistory<XySample, 0> = PlotHistory::new();
    }

    #[test]
    fn push_below_capacity_retains_everything_in_order() {
        let mut history: PlotHistory<XySample, 5> = PlotHistory::new();
        history.push([0.0, 1.0]);
        history.push([1.0, 2.0]);
        assert_eq!(history.len(), 2);
        assert_eq!(history.to_vec(), vec![[0.0, 1.0], [1.0, 2.0]]);
        assert_eq!(history.last(), Some(&[1.0, 2.0]));
    }

    #[test]
    fn push_beyond_capacity_drops_the_oldest_sample_only() {
        let mut history: PlotHistory<XySample, 3> = PlotHistory::new();
        for i in 0..5 {
            history.push([i as f64, (i * 10) as f64]);
        }
        // Only the last 3 of 5 pushes survive, oldest-first.
        assert_eq!(history.len(), 3);
        assert_eq!(
            history.to_vec(),
            vec![[2.0, 20.0], [3.0, 30.0], [4.0, 40.0]]
        );
    }

    #[test]
    fn iter_matches_to_vec_order() {
        let mut history: PlotHistory<XySample, 3> = PlotHistory::new();
        history.push([0.0, 0.0]);
        history.push([1.0, 1.0]);
        let via_iter: Vec<XySample> = history.iter().copied().collect();
        assert_eq!(via_iter, history.to_vec());
    }

    #[test]
    fn clear_empties_the_history_for_a_clean_restart() {
        let mut history: PlotHistory<XySample, 3> = PlotHistory::new();
        history.push([0.0, 1.0]);
        history.push([1.0, 2.0]);
        assert!(!history.is_empty());
        history.clear();
        assert!(history.is_empty());
        assert_eq!(history.len(), 0);
        // Capacity is a type-level property and survives a clear.
        assert_eq!(history.capacity(), 3);
    }

    #[test]
    fn works_with_a_non_xy_sample_type() {
        // A history of a caller-defined struct, not just [f64; 2] --
        // PlotHistory does not assume a particular series shape.
        #[derive(Debug, Clone, Copy, PartialEq)]
        struct Reading {
            t_seconds: f64,
            fuel_temp_k: f64,
            outlet_temp_k: f64,
        }

        let mut history: PlotHistory<Reading, 2> = PlotHistory::new();
        history.push(Reading {
            t_seconds: 0.0,
            fuel_temp_k: 900.0,
            outlet_temp_k: 850.0,
        });
        history.push(Reading {
            t_seconds: 1.0,
            fuel_temp_k: 901.0,
            outlet_temp_k: 851.0,
        });
        history.push(Reading {
            t_seconds: 2.0,
            fuel_temp_k: 902.0,
            outlet_temp_k: 852.0,
        });

        assert_eq!(history.len(), 2);
        assert_eq!(history.last().unwrap().t_seconds, 2.0);
    }

    #[test]
    fn default_matches_new() {
        let history: PlotHistory<XySample, 7> = PlotHistory::default();
        assert!(history.is_empty());
        assert_eq!(history.capacity(), 7);
    }
}
