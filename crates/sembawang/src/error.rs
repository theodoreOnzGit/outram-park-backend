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

    /// Upstream's `release_activity` can return a **negative** atom count and
    /// deliberately does not clamp it. If this is set, at least one node-nuclide
    /// pair went negative and the total is correspondingly under-stated.
    pub negative_atom_count_seen: bool,

    /// The Arrhenius diffusion coefficient is **clamped, never extrapolated**,
    /// outside roughly 700-2400 degrees Celsius. If this is set, the transient
    /// spent time outside the fitted range and the release there is governed by
    /// a held-constant `D`, not by the correlation.
    pub diffusion_coefficient_clamped: bool,

    /// The transient Booth solution **floors at about 1.216e-4** rather than
    /// reaching zero, so a nuclide that should release essentially nothing
    /// still shows a small release fraction.
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
                "at least one release went negative and was left unclamped, as upstream \
                 does; the total is under-stated",
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
                 a near-zero release still reports a small fraction",
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
