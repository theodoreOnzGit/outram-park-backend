//! One neutron at a time.
//!
//! Each history is a call to outram-mc-libs' own
//! [`run_fixed_source_traced`] with ONE source particle and
//! `max_secondaries = 0` — so a fission ends the history instead of spawning a
//! chain inside the call — and a [`TrackRecorder`] capturing every
//! phase-space state. Recording draws no randomness (pinned upstream by
//! `track_capture_does_not_perturb_the_run`), so the trace drawn is exactly
//! the history transported. Nothing here re-implements transport.
//!
//! The demo then chains generations by hand: if neutron *n* ends in fission,
//! neutron *n+1* is born at that fission site, with an energy drawn from the
//! U-235 ENDF/B-VIII.0 fission spectrum at neutron *n*'s incident energy. If it
//! ends in capture, the next neutron starts afresh in a random kernel. The
//! U-235 spectrum is used for every fission, including the rare U-238 fast
//! fissions — the track does not record which nuclide fissioned. That is an
//! approximation of the birth spectrum only, and it is listed as one.

// Headless output and data preparation are native-only; the browser build
// does not call them.
#![cfg_attr(target_arch = "wasm32", allow(dead_code))]

use super::model::{self, NuclearData, N_U235};
use crate::history::{csv_row, History, CSV_HEADER};
use outram_mc_libs::geometry::geometry::Geometry;
use outram_mc_libs::geometry::position::Position;
use outram_mc_libs::physics::fixed_source::{run_fixed_source_traced, FixedSource, FixedSourceSettings};
use outram_mc_libs::physics::track_output::{TrackEvent, TrackRecorder};

/// Incident energy for a fresh chain's birth spectrum: thermal fission, eV.
const FRESH_INCIDENT_EV: f64 = 0.0253;
/// A thermal neutron in graphite scatters hundreds of times; cap states far
/// above that so a truncated track is a reportable anomaly, not the norm.
const MAX_STATES: usize = 200_000;

pub struct Physics {
    pub geometry: Geometry,
    pub data: NuclearData,
    pub centres: Vec<(f64, f64)>,
}

impl Physics {
    pub fn new(data: NuclearData) -> Self {
        let centres = model::particle_centres(model::LAYOUT_SEED);
        let geometry = model::build_geometry(&centres);
        Self { geometry, data, centres }
    }
}

pub struct Chain {
    rng: u64,
    next_index: u64,
    pending_fission: Option<(Position, f64)>,
}

impl Chain {
    pub fn new(seed: u64) -> Self {
        Self { rng: seed, next_index: 0, pending_fission: None }
    }

    pub fn run_next(&mut self, phys: &Physics) -> History {
        let u235 = &phys.data.nuclides[N_U235];
        let from_fission = self.pending_fission.is_some();
        let (birth, e_in) = match self.pending_fission.take() {
            Some(site) => site,
            None => (self.random_kernel_point(&phys.centres), FRESH_INCIDENT_EV),
        };
        let birth_energy_ev = u235.sample_fission_energy(e_in, &mut self.rng);

        // Each history gets its own transport seed, derived — not shared — so
        // histories are independent and the sequence is reproducible.
        let transport_seed = model::splitmix(&mut self.rng);
        let settings = FixedSourceSettings {
            n_particles: 1,
            n_batches: 1,
            seed: transport_seed,
            max_secondaries: 0,
            ..Default::default()
        };
        let mut recorder = TrackRecorder::new(1, MAX_STATES);
        run_fixed_source_traced(
            &phys.geometry,
            &phys.data.materials,
            &phys.data.nuclides,
            &FixedSource::Point { r: birth, energy_ev: birth_energy_ev },
            &settings,
            None,
            Some(&mut recorder),
            None,
            None,
        );
        let track = recorder.tracks.into_iter().next().unwrap_or_default();
        let h = History::from_track(self.next_index, birth, birth_energy_ev, from_fission, track);
        self.next_index += 1;
        if h.outcome == Some(TrackEvent::Fission) {
            if let Some(s) = h.track.states.last() {
                self.pending_fission = Some((s.r, s.energy));
            }
        }
        h
    }

    /// Uniform point in a uniformly chosen kernel (at z = 0).
    fn random_kernel_point(&mut self, centres: &[(f64, f64)]) -> Position {
        let i = ((model::uniform(&mut self.rng) * centres.len() as f64) as usize).min(centres.len() - 1);
        let (cx, cy) = centres[i];
        let r = model::KERNEL_R * (1.0 - 1.0e-9) * model::uniform(&mut self.rng).sqrt();
        let t = std::f64::consts::TAU * model::uniform(&mut self.rng);
        Position::new(cx + r * t.cos(), cy + r * t.sin(), 0.0)
    }
}

// ─── Headless (workspace hard rule) ──────────────────────────────────────────

/// The whole headless trace: header plus one row per history. Deterministic —
/// no clock, no I/O — so the same `(n, seed)` always gives the same bytes.
pub fn headless_csv(phys: &Physics, n: usize, seed: u64) -> String {
    let mut chain = Chain::new(seed);
    let mut out = String::from(CSV_HEADER);
    out.push('\n');
    for _ in 0..n {
        out.push_str(&csv_row(&chain.run_next(phys)));
        out.push('\n');
    }
    out
}
