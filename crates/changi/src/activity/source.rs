// SPDX-License-Identifier: GPL-3.0

//! The released source term: how much of what, over which windows.
//!
//! This is the **input** to [`super::survey`], and the boundary at which
//! `sembawang` hands over. `changi` computes none of it — what gets out of the
//! fuel and out of the building is a severe-accident question, not a dispersion
//! one.
//!
//! # This module holds no nuclide data, deliberately
//!
//! There is no half-life table, no decay-constant table and no atomic-number
//! table here, and none may be added — the crate rule is that nuclide data
//! comes from `boon-lay` so the two cannot drift. A [`NuclideRelease`]
//! therefore carries its decay constant and its deposition group **as supplied
//! by the caller**, and its `label` is a printing label with no meaning to any
//! lookup.
//!
//! The caller should take the decay constant from `boon-lay`'s
//! `TrisoAtopsNuclide::decay_constant()`, and **not** from
//! [`crate::flexpart::decay::decay_constant`], which carries upstream
//! FLEXPART's truncated `0.693147` in place of `ln 2`.
//!
//! # Why windows carry an activity and not a rate
//!
//! A release rate in Bq/s has dimension T^-2, because a becquerel is already
//! s^-1. `uom` will name that type, but nothing human reads it as a release
//! rate. So a window carries **how much was released during it**, and any
//! consumer that wants a rate divides by the window's own duration.

use uom::si::f64::{Frequency, Radioactivity, Time};
use uom::si::radioactivity::becquerel;
use uom::si::time::second;

use super::deposition::DepositionGroup;

/// One release window: activity leaves the building at some unspecified
/// profile between `start` and `end`.
///
/// The model treats the release as **uniform over the window** — puffs are
/// emitted at a constant rate through it. A release that is strongly peaked
/// inside its window should be split into more, shorter windows rather than
/// described by one long one.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ReleaseWindow {
    /// Start of the window, measured from the start of the dispersion run.
    pub start: Time,
    /// End of the window. Must be strictly after `start`.
    pub end: Time,
}

impl ReleaseWindow {
    /// A window from `start` to `end`.
    ///
    /// # Panics
    /// Panics if `end` is not strictly after `start`.
    #[must_use]
    pub fn new(start: Time, end: Time) -> Self {
        assert!(
            end.get::<second>() > start.get::<second>(),
            "a release window must have positive duration; got {} s to {} s",
            start.get::<second>(),
            end.get::<second>()
        );
        Self { start, end }
    }

    /// How long the window lasts.
    #[must_use]
    pub fn duration(self) -> Time {
        self.end - self.start
    }
}

/// One nuclide's release, resolved by window.
///
/// `released[i]` is the activity that left during `SourceTerm::windows[i]`, so
/// the two must be the same length.
#[derive(Debug, Clone, PartialEq)]
pub struct NuclideRelease {
    /// A printing label, e.g. `"I-131"`. Carries no meaning to any lookup —
    /// see the module docs on why there is no nuclide table here.
    pub label: String,
    /// Radioactive decay constant, `ln(2) / half_life`. Supplied by the caller,
    /// normally from `boon-lay`.
    pub decay_constant: Frequency,
    /// How this nuclide behaves on meeting the ground. Note this is a
    /// *deposition* grouping and differs from `boon-lay`'s transport grouping
    /// for Se and Te — see [`DepositionGroup`].
    pub deposition_group: DepositionGroup,
    /// Activity released in each window, in the same order as
    /// [`SourceTerm::windows`].
    pub released: Vec<Radioactivity>,
}

impl NuclideRelease {
    /// Total activity released across every window.
    #[must_use]
    pub fn total_released(&self) -> Radioactivity {
        Radioactivity::new::<becquerel>(
            self.released.iter().map(|a| a.get::<becquerel>()).sum::<f64>(),
        )
    }
}

/// A complete released source term: the windows, and every nuclide's release
/// resolved across them.
#[derive(Debug, Clone, PartialEq)]
pub struct SourceTerm {
    /// Release windows, ascending and **contiguous** — see [`Self::validate`].
    pub windows: Vec<ReleaseWindow>,
    /// One entry per nuclide.
    pub nuclides: Vec<NuclideRelease>,
}

impl SourceTerm {
    /// Build and validate in one step.
    ///
    /// # Panics
    /// Panics on anything [`Self::validate`] rejects.
    #[must_use]
    pub fn new(windows: Vec<ReleaseWindow>, nuclides: Vec<NuclideRelease>) -> Self {
        let term = Self { windows, nuclides };
        term.validate();
        term
    }

    /// Check the invariants the dispersion driver relies on.
    ///
    /// Windows must be non-empty, **contiguous** (each starting exactly where
    /// the previous ended) and ascending; every nuclide must carry one activity
    /// per window; no activity may be negative; no decay constant may be
    /// negative.
    ///
    /// **Contiguity is required, not merely tidy.** [`Self::segment_boundaries`]
    /// hands the windows to [`super::chi_over_q::dilution_factors`] as a list of
    /// boundaries, which partitions the whole run — so a *gap* between two
    /// windows would silently become part of the neighbouring segment rather
    /// than a quiet period. To express a pause, insert a window with zero
    /// released activity.
    ///
    /// # Panics
    /// Panics with a message naming the violated invariant.
    pub fn validate(&self) {
        assert!(!self.windows.is_empty(), "a source term needs at least one release window");
        assert!(!self.nuclides.is_empty(), "a source term needs at least one nuclide");
        for pair in self.windows.windows(2) {
            let (a, b) = (pair[0], pair[1]);
            assert!(
                (b.start.get::<second>() - a.end.get::<second>()).abs() < 1e-9,
                "release windows must be contiguous: window ending at {} s is followed by one \
                 starting at {} s. Insert a zero-activity window to express a pause.",
                a.end.get::<second>(),
                b.start.get::<second>()
            );
        }
        for n in &self.nuclides {
            assert_eq!(
                n.released.len(),
                self.windows.len(),
                "nuclide {} carries {} activities for {} windows",
                n.label,
                n.released.len(),
                self.windows.len()
            );
            assert!(
                n.decay_constant.get::<uom::si::frequency::hertz>() >= 0.0,
                "nuclide {} has a negative decay constant",
                n.label
            );
            for (i, a) in n.released.iter().enumerate() {
                assert!(
                    a.get::<becquerel>() >= 0.0,
                    "nuclide {} has a negative release in window {i}",
                    n.label
                );
            }
        }
    }

    /// The window boundaries, as [`super::chi_over_q::dilution_factors`] wants
    /// them: `n + 1` ascending times for `n` windows.
    ///
    /// Relies on contiguity, which [`Self::validate`] enforces.
    #[must_use]
    pub fn segment_boundaries(&self) -> Vec<Time> {
        let mut bounds = Vec::with_capacity(self.windows.len() + 1);
        bounds.push(self.windows[0].start);
        for w in &self.windows {
            bounds.push(w.end);
        }
        bounds
    }

    /// When the last window closes.
    #[must_use]
    pub fn end(&self) -> Time {
        self.windows[self.windows.len() - 1].end
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use uom::si::frequency::hertz;

    fn window(a: f64, b: f64) -> ReleaseWindow {
        ReleaseWindow::new(Time::new::<second>(a), Time::new::<second>(b))
    }

    fn nuclide(label: &str, activities: &[f64]) -> NuclideRelease {
        NuclideRelease {
            label: label.to_string(),
            decay_constant: Frequency::new::<hertz>(0.0),
            deposition_group: DepositionGroup::Aerosol,
            released: activities
                .iter()
                .map(|a| Radioactivity::new::<becquerel>(*a))
                .collect(),
        }
    }

    #[test]
    fn boundaries_are_one_longer_than_the_windows() {
        let term = SourceTerm::new(
            vec![window(0.0, 100.0), window(100.0, 250.0)],
            vec![nuclide("I-131", &[1.0e12, 2.0e12])],
        );
        let b: Vec<f64> = term
            .segment_boundaries()
            .iter()
            .map(|t| t.get::<second>())
            .collect();
        assert_eq!(b, vec![0.0, 100.0, 250.0]);
        assert_eq!(term.end().get::<second>(), 250.0);
    }

    #[test]
    fn totals_sum_across_windows() {
        let n = nuclide("Cs-137", &[1.0e12, 3.0e12, 6.0e12]);
        assert!((n.total_released().get::<becquerel>() - 1.0e13).abs() < 1.0);
    }

    /// A gap would silently be absorbed into a neighbouring segment, so it is
    /// rejected rather than tolerated.
    #[test]
    #[should_panic(expected = "must be contiguous")]
    fn a_gap_between_windows_is_rejected() {
        let _ = SourceTerm::new(
            vec![window(0.0, 100.0), window(150.0, 250.0)],
            vec![nuclide("I-131", &[1.0e12, 2.0e12])],
        );
    }

    #[test]
    #[should_panic(expected = "activities for")]
    fn a_nuclide_must_carry_one_activity_per_window() {
        let _ = SourceTerm::new(
            vec![window(0.0, 100.0), window(100.0, 250.0)],
            vec![nuclide("I-131", &[1.0e12])],
        );
    }

    #[test]
    #[should_panic(expected = "positive duration")]
    fn a_zero_length_window_is_rejected() {
        let _ = window(100.0, 100.0);
    }

    #[test]
    #[should_panic(expected = "negative release")]
    fn a_negative_release_is_rejected() {
        let _ = SourceTerm::new(vec![window(0.0, 100.0)], vec![nuclide("I-131", &[-1.0])]);
    }
}
