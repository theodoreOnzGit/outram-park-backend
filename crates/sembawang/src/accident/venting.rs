// SPDX-License-Identifier: GPL-3.0

//! The venting selection, held as indices so it cannot be mispaired.
//!
//! # The upstream defect this type exists to make unwriteable
//!
//! `coolant_release` returns the released fraction at each sample where the
//! hot-node temperature is **rising** (`mean_dtdt >= 0`), together with the
//! times of those samples. That is a **gather**: an arbitrary subset of the
//! full time axis, in order.
//!
//! Upstream then pairs it with a **prefix** of the temperature array:
//!
//! ```python
//! frac, times_short = calc.coolant_release(times, accident_temp)
//! rmv = np.size(times) - np.size(times_short)
//! times = times_short                         # a SELECTION
//! accident_temp = accident_temp[:, :-rmv, :]  # a PREFIX
//! ```
//!
//! A selection of `k` elements and the first `k` elements are the same thing
//! **only when the venting mask is a contiguous run starting at index 0.** For
//! a monotonically heating transient it always is, which is why the defect
//! survives: the reference case never exercises it.
//!
//! On a transient that heats, cools and reheats, the mask is gappy and the two
//! diverge — every venting sample after the gap gets paired with the
//! temperature of an earlier, cooler one. Measured on a heat-cool-reheat case,
//! one node's Xe-133 release differs by tens of per cent between the two
//! pairings.
//!
//! # The fix is structural, not care
//!
//! Being careful is not a fix, because the next caller has to be careful too.
//! So: [`VentingWindow`] owns the **indices**, [`VentingWindow::times`] and
//! [`VentingWindow::gather`] both go through them, and **no method on this type
//! returns a prefix.** There is no API here that can produce upstream's
//! pairing; a caller that wants it has to slice an array by hand, which is
//! visible in a diff.
//!
//! # Why the indices are recovered by walking, not re-derived
//!
//! The obvious alternative is to re-evaluate `mean_dtdt >= 0` here and take the
//! indices where it holds. That would be a **second copy of the predicate**,
//! and two copies drift — a change to the venting condition in `boon-lay` would
//! silently stop matching this one, and the failure would be a subtle
//! mispairing rather than a compile error.
//!
//! Instead [`VentingWindow::from_coolant_release`] walks the returned venting
//! times against the full axis in lockstep and asserts every one is consumed.
//! It carries no opinion about *why* a sample vented, so it cannot disagree
//! about it.

use uom::si::f64::Time;
use uom::si::time::second;

use crate::error::{Error, Result};

/// Which samples of a transient vented, and how much each released.
///
/// Constructed from `boon-lay`'s `coolant_release` output. See the module docs
/// for why it stores indices and why it has no prefix accessor.
#[derive(Debug, Clone, PartialEq)]
pub struct VentingWindow {
    indices: Vec<usize>,
    fractions: Vec<f64>,
    full_len: usize,
}

impl VentingWindow {
    /// Recover the venting indices from `coolant_release`'s output.
    ///
    /// # Arguments
    /// - `full_times` — the complete time axis the transient was computed on.
    /// - `vent_times` — the second element of `coolant_release`'s return: the
    ///   times of the samples that vented, a subsequence of `full_times`.
    /// - `fractions` — the first element: the released fraction at each of
    ///   those samples.
    ///
    /// Matching is by **exact** `f64` equality, which is correct rather than
    /// fragile here: `coolant_release` pushes elements of `times` through
    /// unmodified, so the values are bit-identical copies, not recomputations.
    ///
    /// # Errors
    /// [`Error::VentingTimeNotOnAxis`] if a venting time is not found in
    /// `full_times` at or after the previous match — which would mean the two
    /// have drifted apart and no pairing is trustworthy.
    ///
    /// # Panics
    /// Panics if `vent_times` and `fractions` differ in length.
    pub fn from_coolant_release(
        full_times: &[Time],
        vent_times: &[Time],
        fractions: Vec<f64>,
    ) -> Result<Self> {
        assert_eq!(
            vent_times.len(),
            fractions.len(),
            "coolant_release returned {} venting times and {} fractions",
            vent_times.len(),
            fractions.len()
        );

        let mut indices = Vec::with_capacity(vent_times.len());
        let mut cursor = 0usize;
        for vt in vent_times {
            let target = vt.get::<second>();
            while cursor < full_times.len()
                && full_times[cursor].get::<second>() != target
            {
                cursor += 1;
            }
            if cursor >= full_times.len() {
                return Err(Error::VentingTimeNotOnAxis { time_s: target });
            }
            indices.push(cursor);
            cursor += 1;
        }

        Ok(Self {
            indices,
            fractions,
            full_len: full_times.len(),
        })
    }

    /// How many samples vented.
    #[must_use]
    pub fn len(&self) -> usize {
        self.indices.len()
    }

    /// Whether nothing vented.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.indices.is_empty()
    }

    /// The length of the full time axis this was gathered from.
    #[must_use]
    pub const fn full_len(&self) -> usize {
        self.full_len
    }

    /// The indices into the full axis, ascending and without repeats.
    #[must_use]
    pub fn indices(&self) -> &[usize] {
        &self.indices
    }

    /// The released fraction at each venting sample.
    ///
    /// The **first entry is always exactly 1**, because upstream's
    /// `coolant_release` hard-codes `frac[0] = 1` regardless of what its
    /// integral produced. That is faithful to upstream and is surfaced through
    /// [`crate::error::Caveats::first_sample_forced_fully_vented`] rather than
    /// corrected here.
    #[must_use]
    pub fn fractions(&self) -> &[f64] {
        &self.fractions
    }

    /// Whether the venting samples form a contiguous run.
    ///
    /// **This is the discriminator for whether upstream's pairing is valid.**
    /// When true, a prefix and this selection coincide and a comparison against
    /// upstream is meaningful. When false, they do not, and any number computed
    /// upstream on this transient is paired with the wrong temperatures.
    #[must_use]
    pub fn is_contiguous(&self) -> bool {
        self.indices
            .windows(2)
            .all(|w| w[1] == w[0] + 1)
    }

    /// The venting samples' times, gathered from the full axis.
    ///
    /// # Panics
    /// Panics if `full_times` is not the axis this was built from.
    #[must_use]
    pub fn times(&self, full_times: &[Time]) -> Vec<Time> {
        self.gather(full_times)
    }

    /// Any per-sample quantity, gathered at the venting indices.
    ///
    /// This is the **only** way to line another array up with this window, and
    /// it is deliberately generic so that temperatures, rates and per-node
    /// slices all go through the same path. There is no prefix counterpart.
    ///
    /// # Panics
    /// Panics if `full` is not the same length as the axis this was built
    /// from — which is what catches a caller who has already truncated
    /// something before getting here.
    #[must_use]
    pub fn gather<T: Copy>(&self, full: &[T]) -> Vec<T> {
        assert_eq!(
            full.len(),
            self.full_len,
            "this window indexes a {}-sample axis but was handed {} samples. If the \
             array was already truncated to the venting count, that is upstream's \
             prefix pairing and it is wrong whenever is_contiguous() is false.",
            self.full_len,
            full.len()
        );
        self.indices.iter().map(|&i| full[i]).collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn times(v: &[f64]) -> Vec<Time> {
        v.iter().map(|t| Time::new::<second>(*t)).collect()
    }

    #[test]
    fn a_monotonic_heat_up_vents_contiguously_and_matches_a_prefix() {
        let full = times(&[0.0, 100.0, 200.0, 300.0, 400.0]);
        let vent = times(&[0.0, 100.0, 200.0]);
        let w = VentingWindow::from_coolant_release(&full, &vent, vec![1.0, 0.4, 0.7]).unwrap();

        assert!(w.is_contiguous());
        assert_eq!(w.indices(), &[0, 1, 2]);

        // In this regime -- and ONLY this regime -- upstream's prefix agrees.
        let temps = [900.0, 1000.0, 1100.0, 1050.0, 1200.0];
        assert_eq!(w.gather(&temps), temps[..w.len()].to_vec());
    }

    /// The defect, demonstrated. A heat-cool-reheat transient vents at 0, 100
    /// and then again at 300 and 400 -- indices {0, 1, 3, 4}, not {0, 1, 2, 3}.
    ///
    /// **This test asserts the two pairings DIFFER.** A test that only checked
    /// `gather` returned the right values would pass under the bug too, because
    /// the bug is in what upstream pairs with, not in the gather.
    #[test]
    fn a_gappy_mask_makes_the_prefix_pairing_wrong_and_the_gather_right() {
        let full = times(&[0.0, 100.0, 200.0, 300.0, 400.0]);
        // 200 s is a cooling sample, so it does not vent.
        let vent = times(&[0.0, 100.0, 300.0, 400.0]);
        let w =
            VentingWindow::from_coolant_release(&full, &vent, vec![1.0, 0.4, 0.6, 0.9]).unwrap();

        assert!(
            !w.is_contiguous(),
            "this is the regime where upstream's pairing goes wrong"
        );
        assert_eq!(w.indices(), &[0, 1, 3, 4]);

        let temps = [900.0, 1000.0, 950.0, 1100.0, 1300.0];

        // What this type produces: each venting sample's OWN temperature.
        let correct = w.gather(&temps);
        assert_eq!(correct, vec![900.0, 1000.0, 1100.0, 1300.0]);

        // What upstream's `accident_temp[:, :-rmv, :]` produces: the first
        // `len` temperatures, regardless of which samples actually vented.
        let upstream_prefix = temps[..w.len()].to_vec();
        assert_eq!(upstream_prefix, vec![900.0, 1000.0, 950.0, 1100.0]);

        assert_ne!(
            correct, upstream_prefix,
            "if these agreed, this test would not be exercising the defect"
        );
        // Every sample after the gap is paired with an earlier, cooler one.
        assert!(correct[2] > upstream_prefix[2]);
        assert!(correct[3] > upstream_prefix[3]);
    }

    /// Handing in an already-truncated array is exactly upstream's mistake, so
    /// it is rejected rather than silently gathered from the wrong thing.
    #[test]
    #[should_panic(expected = "prefix pairing")]
    fn gathering_from_an_already_truncated_array_is_rejected() {
        let full = times(&[0.0, 100.0, 200.0, 300.0, 400.0]);
        let vent = times(&[0.0, 100.0, 300.0]);
        let w = VentingWindow::from_coolant_release(&full, &vent, vec![1.0, 0.4, 0.6]).unwrap();
        let already_truncated = [900.0, 1000.0, 950.0];
        let _ = w.gather(&already_truncated);
    }

    #[test]
    fn a_venting_time_off_the_axis_is_an_error_not_a_silent_mispairing() {
        let full = times(&[0.0, 100.0, 200.0]);
        let vent = times(&[0.0, 150.0]);
        let err = VentingWindow::from_coolant_release(&full, &vent, vec![1.0, 0.5]).unwrap_err();
        assert!(matches!(err, Error::VentingTimeNotOnAxis { time_s } if time_s == 150.0));
    }

    #[test]
    fn a_single_venting_sample_counts_as_contiguous() {
        let full = times(&[0.0, 100.0]);
        let vent = times(&[100.0]);
        let w = VentingWindow::from_coolant_release(&full, &vent, vec![1.0]).unwrap();
        assert!(w.is_contiguous());
        assert_eq!(w.len(), 1);
        assert_eq!(w.full_len(), 2);
    }

    #[test]
    fn the_times_accessor_gathers_rather_than_truncating() {
        let full = times(&[0.0, 100.0, 200.0, 300.0]);
        let vent = times(&[0.0, 300.0]);
        let w = VentingWindow::from_coolant_release(&full, &vent, vec![1.0, 0.8]).unwrap();
        let got: Vec<f64> = w.times(&full).iter().map(|t| t.get::<second>()).collect();
        assert_eq!(got, vec![0.0, 300.0]);
    }
}
