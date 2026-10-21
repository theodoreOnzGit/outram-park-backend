// SPDX-License-Identifier: GPL-3.0

//! Hand a released source term to `changi` for dispersion and deposition.
//!
//! [`crate::accident::release::accident_release`] produces a
//! [`SourceTerm`] whose windows span the accident transient. `changi`'s
//! [`changi::activity::chi_over_q::dilution_factors`] needs something slightly
//! different: segment boundaries that partition the **whole dispersion run**,
//! starting at `t = 0` and ending when the run ends. The run must also carry on
//! after the last release, so the last puffs have time to reach the far
//! receptors.
//!
//! [`pad_for_dispersion`] closes that gap and does nothing else. It prepends a
//! zero-release window if the release starts after `t = 0`, and appends one
//! zero-release window covering the post-release tail. **No activity is added,
//! removed or moved**, so the per-nuclide totals are unchanged. That is
//! asserted in `tests/chain_handoff.rs`.
//!
//! This module has **no upstream**. It is plumbing between two crates, checked
//! by consistency tests, not verification.

use changi::activity::source::{NuclideRelease, ReleaseWindow, SourceTerm};
use uom::si::f64::{Radioactivity, Time};
use uom::si::radioactivity::becquerel;
use uom::si::time::second;

/// Pad `term` so its windows partition a dispersion run from `t = 0` to
/// `term.end() + tail`.
///
/// # Arguments
/// - `term` — the released source term, e.g. `AccidentRelease::source_term`.
/// - `tail` — how long the dispersion run continues after the last release,
///   in seconds of simulated time. Choose at least the puff lifetime
///   (`RunConfig::puff_duration`), or the last puffs are cut off before
///   reaching the far receptors.
///
/// # Returns
/// A new [`SourceTerm`] with the same nuclides, labels, decay constants and
/// deposition groups. Its windows are, in order: an optional leading
/// zero-release window, the original windows, and one trailing zero-release
/// window of length `tail`. Use its [`SourceTerm::segment_boundaries`] as the
/// segment list for `dilution_factors`, and its [`SourceTerm::end`] as the
/// run duration.
///
/// # Panics
/// Panics if `tail` is not positive, or if the first window starts before
/// `t = 0`.
#[must_use]
pub fn pad_for_dispersion(term: &SourceTerm, tail: Time) -> SourceTerm {
    assert!(
        tail.get::<second>() > 0.0,
        "the post-release tail must be positive; got {} s",
        tail.get::<second>()
    );
    let first_start = term.windows[0].start;
    assert!(
        first_start.get::<second>() >= 0.0,
        "the release cannot start before t = 0; got {} s",
        first_start.get::<second>()
    );

    let zero = Radioactivity::new::<becquerel>(0.0);
    let lead = first_start.get::<second>() > 0.0;

    let mut windows = Vec::with_capacity(term.windows.len() + 2);
    if lead {
        windows.push(ReleaseWindow::new(Time::new::<second>(0.0), first_start));
    }
    windows.extend(term.windows.iter().copied());
    let end = term.end();
    windows.push(ReleaseWindow::new(end, end + tail));

    let nuclides = term
        .nuclides
        .iter()
        .map(|n| {
            let mut released = Vec::with_capacity(windows.len());
            if lead {
                released.push(zero);
            }
            released.extend(n.released.iter().copied());
            released.push(zero);
            NuclideRelease {
                label: n.label.clone(),
                decay_constant: n.decay_constant,
                deposition_group: n.deposition_group,
                released,
            }
        })
        .collect();

    SourceTerm::new(windows, nuclides)
}
