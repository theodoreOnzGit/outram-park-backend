// SPDX-License-Identifier: GPL-3.0

//! Errors, and the caveat channel.
//!
//! # Why known upstream defects are returned as DATA, not logged
//!
//! `boon-lay`'s TRISO-ATOPS fork faithfully reproduces several upstream
//! behaviours that a caller would otherwise have to know about already. Logging
//! them would put them somewhere a caller does not read; burying them in a doc
//! comment puts them somewhere a caller does not look at run time. So they come
//! back attached to the result, as [`Caveats`], and a caller that reports a
//! number without reporting these is reporting half of it.

use thiserror::Error;

/// Anything that can go wrong assembling a source term.
#[derive(Debug, Error, Clone, PartialEq)]
pub enum Error {
    /// The prescribed temperature transient and its time axis disagree.
    #[error("the temperature transient has {temperatures} samples for {times} times")]
    TransientLengthMismatch {
        /// Number of time samples supplied.
        times: usize,
        /// Number of temperature samples supplied.
        temperatures: usize,
    },

    /// A nuclide was asked for that the TRISO-ATOPS supported table does not
    /// contain. The table has 84 entries; see
    /// `boon_lay::triso_atops_fork::nuclide_model::nuclide_database`.
    #[error("nuclide {0} is not in the TRISO-ATOPS supported table")]
    UnknownNuclide(String),

    /// The transient is too short to integrate.
    #[error("need at least two transient samples, got {0}")]
    TransientTooShort(usize),

    /// A venting time came back from `coolant_release` that is not in the full
    /// time axis it was derived from. This should be impossible and means the
    /// two have drifted apart — see [`crate::accident::venting`].
    #[error(
        "venting time {time_s} s is not present in the full time axis; the venting \
         selection and the axis it was gathered from have drifted apart"
    )]
    VentingTimeNotOnAxis {
        /// The offending time, in seconds.
        time_s: f64,
    },
}

/// This crate's result type.
pub type Result<T> = core::result::Result<T, Error>;

/// Known upstream behaviours that affected a result, returned alongside it.
///
/// Every field records a **real, deliberate** upstream behaviour that
/// `boon-lay` reproduces faithfully. None is a bug in this workspace, and none
/// should be silently corrected here — but a reader who does not know about
/// them will misread the numbers.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct Caveats {
    /// Upstream's `coolant_release` hard-codes `frac[0] = 1`, so the **first
    /// sample is always treated as fully vented** regardless of what the
    /// integral says. Always true when a venting calculation ran; recorded so
    /// the first window's release is not read as a physical result.
    pub first_sample_forced_fully_vented: bool,

    /// A release somewhere in the chain went **negative**. Two different things
    /// set this, and **they push the total in opposite directions**, so the
    /// flag alone does not tell a reader which way the answer is wrong.
    ///
    /// ~~If this is set, at least one node-nuclide pair went negative and the
    /// total is correspondingly under-stated.~~ **CORRECTED 2026-09-24** — that
    /// was right for one of the two paths and wrong for the other:
    ///
    /// 1. **`release_activity` returning a negative atom count.** Upstream
    ///    deliberately does not clamp it, and neither does this crate, so the
    ///    negative propagates and the total is **under-stated**. This path
    ///    needs a non-empty normal-operation pool to fire at all: with
    ///    [`crate::accident::release::zero_pools`] every subtracted term is
    ///    zero, so it cannot.
    /// 2. **A negative per-window first difference**, i.e. a *non-monotonic*
    ///    cumulative release. That one is floored to zero at the
    ///    [`changi::activity::source::SourceTerm`] boundary, which **raises**
    ///    the sum of the windows above the cumulative endpoint — the total is
    ///    **over-stated**.
    ///
    /// Path 2 is reachable and is not hypothetical. **Silver** is the case:
    /// the transient breakthrough release fraction
    /// (`boon_lay::triso_atops_fork::release_models::transient::breakthrough_model_transient`)
    /// rises, is then driven negative by its `−a/(2r)` time-lag term and
    /// clamped to zero until breakthrough, and only then grows — so the
    /// cumulative curie series falls over that stretch. Measured on the HTR-10
    /// DLOFC case (`crate::htr10`, 2026-09-24): Ag-110m had **13 of 30 windows
    /// negative**, and the windows sum to `2.503345e-2 Ci` against a cumulative
    /// endpoint of `2.500569e-2 Ci` — **over-stated by a factor 1.0011**. No
    /// other nuclide in that run had a single negative window.
    pub negative_atom_count_seen: bool,

    /// The Arrhenius diffusion coefficient is **clamped, never extrapolated**,
    /// outside roughly 700-2400 degrees Celsius. If this is set, the transient
    /// spent time outside the fitted range and the release there is governed by
    /// a held-constant `D`, not by the correlation.
    pub diffusion_coefficient_clamped: bool,

    /// The transient Booth solution **floors at about 1.216e-4** rather than
    /// reaching zero, so a nuclide that should release essentially nothing
    /// still shows a small release fraction.
    ///
    /// **NOT WIRED — nothing in this crate ever sets this field, so it is
    /// always `false` on a computed result.** Stated here because a reader
    /// finding it in a caveat struct would reasonably assume the condition is
    /// detected, and it is not: verified 2026-09-24 by searching the workspace
    /// for writes to it, which occur only in this module's own tests. It is
    /// kept rather than deleted because the underlying behaviour is real —
    /// `boon_lay::...::release_models::transient::booth_transient` snaps below
    /// `BOOTH_TRANSIENT_ZERO_FLOOR = 1e-6` — and detecting it needs the
    /// release-fraction values, which
    /// [`crate::accident::release::accident_release`] does not currently keep.
    pub booth_transient_floored: bool,

    /// The venting mask was **not contiguous**. This is the condition under
    /// which upstream's own prefix-versus-selection pairing goes wrong — see
    /// [`crate::accident::venting`]. This crate does not have that defect, but
    /// a result computed on a gappy mask cannot be compared against upstream's.
    pub venting_mask_was_gappy: bool,
}

impl Caveats {
    /// Whether anything worth reporting happened. Always true in practice —
    /// [`Self::first_sample_forced_fully_vented`] is set on every venting run —
    /// which is the point.
    #[must_use]
    pub const fn any(self) -> bool {
        self.first_sample_forced_fully_vented
            || self.negative_atom_count_seen
            || self.diffusion_coefficient_clamped
            || self.booth_transient_floored
            || self.venting_mask_was_gappy
    }

    /// One line per caveat that fired, for printing beside a result.
    #[must_use]
    pub fn lines(self) -> Vec<&'static str> {
        let mut v = Vec::new();
        if self.first_sample_forced_fully_vented {
            v.push(
                "upstream forces the first sample to be fully vented (frac[0] = 1); \
                 the first window's release is not a computed result",
            );
        }
        if self.negative_atom_count_seen {
            v.push(
                "at least one release went negative: an unclamped negative atom count \
                 under-states the total, while a negative per-window difference is \
                 floored to zero and OVER-states it (silver does the latter)",
            );
        }
        if self.diffusion_coefficient_clamped {
            v.push(
                "the transient left the ~700-2400 C fitted range, where D is clamped \
                 rather than extrapolated",
            );
        }
        if self.booth_transient_floored {
            v.push(
                "the transient Booth solution floors near 1.216e-4 rather than zero, so \
                 a near-zero release still reports a small fraction (NOTE: nothing sets \
                 this flag on a computed result -- see its field docs)",
            );
        }
        if self.venting_mask_was_gappy {
            v.push(
                "the venting mask was not contiguous; upstream's prefix pairing is wrong \
                 in this regime, so this result cannot be compared against it",
            );
        }
        v
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_default_caveats_reports_nothing() {
        assert!(!Caveats::default().any());
        assert!(Caveats::default().lines().is_empty());
    }

    #[test]
    fn every_caveat_has_a_line() {
        let all = Caveats {
            first_sample_forced_fully_vented: true,
            negative_atom_count_seen: true,
            diffusion_coefficient_clamped: true,
            booth_transient_floored: true,
            venting_mask_was_gappy: true,
        };
        assert!(all.any());
        assert_eq!(all.lines().len(), 5, "every field must produce a line");
    }
}
