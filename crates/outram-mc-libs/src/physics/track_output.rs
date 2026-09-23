// SPDX-License-Identifier: GPL-3.0

//! **Particle track capture** — every phase-space state along a history.
//! GitHub #271 scope item 2.
//!
//! Ported in structure from `src/track_output.cpp` and `TrackState`
//! (`include/openmc/particle_data.h:81`) at OpenMC `afa7a14`.
//!
//! # What this is for, and why it is not a logging feature
//!
//! This crate's history is a sequence of hunts — the MT=91 kinematic-bound
//! defect (#192), the inelastic anisotropy diagnosis (`op-tm9f`), the ring-RPT
//! hunt (#185, #186) — each of which came down to what *one history* did, and
//! each of which was resolved by re-deriving that from aggregate statistics
//! because there was no way to look. A recorded track is the direct answer.
//!
//! # Memory is bounded by construction, not by hoping
//!
//! A single fast history on Godiva is tens of events; a generation is
//! thousands of histories; a run is a hundred generations. Recording
//! everything is gigabytes. [`TrackRecorder`] therefore caps **both** the
//! number of tracks (`max_tracks`, upstream's `settings::max_tracks`) and the
//! states per track, and it reports what it dropped rather than silently
//! truncating — a truncated track that looks complete is worse than no track,
//! because the missing events are exactly the end of the history one is
//! usually looking for.
//!
//! # Recording consumes no randomness
//!
//! Nothing here draws from the RNG, so a run with capture on gives the **same
//! eigenvalue, bit for bit**, as one without.
//! `tests/variance_reduction_is_bit_identical_when_analog.rs` is the pattern;
//! `track_capture_does_not_perturb_the_run` is the check for this feature.

use crate::geometry::position::{Direction, Position};

/// What happened at a recorded state — upstream has no equivalent field, and
/// it is added here because a track without it is a list of points that a
/// reader has to guess the meaning of.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TrackEvent {
    /// The particle's birth state.
    Born,
    /// A surface was crossed.
    SurfaceCrossing,
    /// A scattering collision (elastic, inelastic, (n,xn)).
    Scatter,
    /// A fission, which terminates the history in the analog path.
    Fission,
    /// Capture.
    Absorption,
    /// Killed by Russian roulette or a weight-window cutoff. **Not** a
    /// physical event: distinguishing it from `Absorption` is what stops a
    /// track reader from counting variance-reduction kills as captures.
    Rouletted,
    /// Left the geometry through a vacuum boundary, or streamed to infinity.
    Leak,
    /// The particle was lost — a `locate` failure or the event-count cap.
    /// A track that ends here is a defect report, not a history.
    Lost,
}

/// One phase-space state — `TrackState` (`include/openmc/particle_data.h:81`),
/// plus [`Self::event`].
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct TrackState {
    /// Position \[cm\].
    pub r: Position,
    /// Direction (unit).
    pub u: Direction,
    /// Energy \[eV\].
    pub energy: f64,
    /// Time since birth \[s\].
    pub time: f64,
    /// Statistical weight.
    pub weight: f64,
    /// Cell index, or `usize::MAX` where it is not known.
    pub cell: usize,
    /// Material index, or `None` in a void — upstream's `material_id = -1`.
    pub material: Option<usize>,
    /// What produced this state.
    pub event: TrackEvent,
}

/// One particle's full track — `TrackStateHistory`
/// (`include/openmc/particle_data.h:93`).
#[derive(Debug, Clone, PartialEq, Default)]
pub struct Track {
    /// States in the order they occurred.
    pub states: Vec<TrackState>,
    /// States dropped because [`TrackRecorder::max_states_per_track`] was hit.
    ///
    /// Non-zero means this track is **incomplete at the end**, which is
    /// usually where the interesting part is.
    pub dropped_states: usize,
}

impl Track {
    /// Total path length \[cm\] along the recorded states.
    ///
    /// Meaningless when [`Self::dropped_states`] is non-zero, and the method
    /// says so rather than returning a short answer that looks like a result.
    pub fn path_length(&self) -> Option<f64> {
        if self.dropped_states > 0 {
            return None;
        }
        Some(
            self.states
                .windows(2)
                .map(|w| {
                    let (a, b) = (w[0].r, w[1].r);
                    ((b.x - a.x).powi(2) + (b.y - a.y).powi(2) + (b.z - a.z).powi(2)).sqrt()
                })
                .sum(),
        )
    }

    /// How the history ended, or `None` for a track that was cut short.
    pub fn outcome(&self) -> Option<TrackEvent> {
        if self.dropped_states > 0 {
            return None;
        }
        self.states.last().map(|s| s.event)
    }
}

/// Collects tracks under a hard memory bound —
/// `settings::max_tracks` / `settings::write_all_tracks`.
#[derive(Debug, Clone, PartialEq)]
pub struct TrackRecorder {
    /// Completed tracks.
    pub tracks: Vec<Track>,
    /// Cap on the number of tracks kept.
    pub max_tracks: usize,
    /// Cap on states within one track.
    pub max_states_per_track: usize,
    /// Tracks not started because [`Self::max_tracks`] was reached.
    pub dropped_tracks: usize,
    /// The track currently being filled.
    current: Option<Track>,
}

impl TrackRecorder {
    /// A recorder bounded at `max_tracks` histories and
    /// `max_states_per_track` states each.
    ///
    /// Upstream's default `max_tracks` is 1000; the per-track cap has no
    /// upstream equivalent and is added because a single history in a
    /// reflective, weakly-absorbing region can reach this crate's own
    /// `MAX_EVENTS` of 100 000.
    pub fn new(max_tracks: usize, max_states_per_track: usize) -> Self {
        Self {
            tracks: Vec::new(),
            max_tracks,
            max_states_per_track,
            dropped_tracks: 0,
            current: None,
        }
    }

    /// Whether the track budget is spent.
    ///
    /// # This is NOT a licence to stop calling [`Self::begin`]
    ///
    /// The first version of the transport wiring used this to skip `begin`
    /// entirely once full — which meant [`Self::dropped_tracks`] never
    /// incremented and always read zero, so a recorder that had seen 64 of
    /// 2000 histories reported that it had refused none. The counter was dead
    /// and the number it produced was wrong in the reassuring direction.
    /// `track_capture_does_not_perturb_the_run` caught it.
    ///
    /// `begin` is cheap and counts correctly when full, so the loop calls it
    /// unconditionally. This predicate is for a *caller* deciding whether to
    /// bother assembling a state, not for skipping the bookkeeping.
    pub fn is_full(&self) -> bool {
        self.current.is_none() && self.tracks.len() >= self.max_tracks
    }

    /// Begin a new track. Silently a no-op once the budget is spent, with the
    /// count kept in [`Self::dropped_tracks`].
    pub fn begin(&mut self) {
        self.finish();
        if self.tracks.len() >= self.max_tracks {
            self.dropped_tracks += 1;
            return;
        }
        self.current = Some(Track::default());
    }

    /// Record one state on the current track.
    pub fn record(&mut self, state: TrackState) {
        let Some(t) = self.current.as_mut() else {
            return;
        };
        if t.states.len() >= self.max_states_per_track {
            t.dropped_states += 1;
            return;
        }
        t.states.push(state);
    }

    /// Close the current track, if any.
    pub fn finish(&mut self) {
        if let Some(t) = self.current.take() {
            if !t.states.is_empty() {
                self.tracks.push(t);
            }
        }
    }

    /// Total states held, for reporting memory.
    pub fn n_states(&self) -> usize {
        self.tracks.iter().map(|t| t.states.len()).sum()
    }

    /// Every track that ended in the given way — the query a defect hunt
    /// actually makes ("show me the ones that got lost").
    pub fn ending_in(&self, event: TrackEvent) -> Vec<&Track> {
        self.tracks
            .iter()
            .filter(|t| t.outcome() == Some(event))
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn state(x: f64, event: TrackEvent) -> TrackState {
        TrackState {
            r: Position::new(x, 0.0, 0.0),
            u: Direction::new(1.0, 0.0, 0.0),
            energy: 1.0e6,
            time: 0.0,
            weight: 1.0,
            cell: 0,
            material: Some(0),
            event,
        }
    }

    /// The track budget is a hard cap, and what it refused is counted.
    #[test]
    fn the_track_budget_is_a_hard_cap_and_reports_what_it_dropped() {
        let mut r = TrackRecorder::new(2, 10);
        for i in 0..5 {
            r.begin();
            r.record(state(i as f64, TrackEvent::Born));
            r.record(state(i as f64 + 1.0, TrackEvent::Leak));
        }
        r.finish();
        assert_eq!(r.tracks.len(), 2, "the cap must hold");
        assert_eq!(r.dropped_tracks, 3, "and must say how many it refused");
        assert!(r.is_full());
    }

    /// **A truncated track must not look complete.** `path_length` and
    /// `outcome` both refuse rather than returning a short answer — the
    /// dropped events are the end of the history, which is where a defect hunt
    /// is looking.
    #[test]
    fn a_truncated_track_refuses_to_report_a_length_or_an_outcome() {
        let mut r = TrackRecorder::new(1, 3);
        r.begin();
        for i in 0..6 {
            r.record(state(i as f64, TrackEvent::Scatter));
        }
        r.record(state(99.0, TrackEvent::Leak));
        r.finish();

        let t = &r.tracks[0];
        assert_eq!(t.states.len(), 3);
        assert_eq!(t.dropped_states, 4);
        assert_eq!(
            t.path_length(),
            None,
            "a path length from a truncated track is a wrong number that looks right"
        );
        assert_eq!(t.outcome(), None, "the last event was dropped, so it is unknown");
    }

    /// A complete track reports its length and how it ended.
    #[test]
    fn a_complete_track_reports_its_length_and_outcome() {
        let mut r = TrackRecorder::new(4, 100);
        r.begin();
        r.record(state(0.0, TrackEvent::Born));
        r.record(state(3.0, TrackEvent::Scatter));
        r.record(state(7.0, TrackEvent::Leak));
        r.finish();
        let t = &r.tracks[0];
        assert_eq!(t.path_length(), Some(7.0));
        assert_eq!(t.outcome(), Some(TrackEvent::Leak));
    }

    /// Roulette kills are distinguishable from captures, so a track reader
    /// cannot count variance-reduction kills as physical absorptions.
    #[test]
    fn a_roulette_kill_is_not_an_absorption() {
        let mut r = TrackRecorder::new(4, 100);
        r.begin();
        r.record(state(0.0, TrackEvent::Born));
        r.record(state(1.0, TrackEvent::Rouletted));
        r.begin();
        r.record(state(0.0, TrackEvent::Born));
        r.record(state(1.0, TrackEvent::Absorption));
        r.finish();
        assert_eq!(r.ending_in(TrackEvent::Rouletted).len(), 1);
        assert_eq!(r.ending_in(TrackEvent::Absorption).len(), 1);
        assert_ne!(TrackEvent::Rouletted, TrackEvent::Absorption);
    }

    /// An empty track is not stored — a `begin` with no events would otherwise
    /// inflate the count with nothing in it.
    #[test]
    fn an_empty_track_is_not_stored() {
        let mut r = TrackRecorder::new(4, 100);
        r.begin();
        r.begin();
        r.finish();
        assert!(r.tracks.is_empty());
        assert_eq!(r.n_states(), 0);
    }
}
