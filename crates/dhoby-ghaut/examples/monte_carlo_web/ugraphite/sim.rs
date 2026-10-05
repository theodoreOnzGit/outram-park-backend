//! What the `ugraphite` and `lumped` rungs compute. Nothing here
//! re-implements transport.
//!
//! **Watch, one neutron at a time** ([`Chain`]): each neutron is a call to
//! outram-mc-libs' [`run_fixed_source_traced`] with ONE source particle,
//! `max_secondaries = 0` (so a fission ends the history) and a
//! [`TrackRecorder`], exactly as the TRISO, Godiva and LCT-008 rungs do. The
//! collision physics is the transport's own: graphite S(α,β) below its
//! cutoff, free gas (with DBRC on U-238) below 400 kT, target at rest above,
//! URR probability tables in U-238's unresolved range.
//!
//! **Chaining is the illustration:**
//! - after a fission, the next neutron is born at that fission site, with
//!   energy drawn from the U-235 ENDF/B-VIII.0 fission spectrum at the
//!   incident energy (the track does not say which nuclide fissioned; in
//!   these thermal systems almost every fission is U-235);
//! - after a capture, the next neutron starts at a uniform point of the
//!   uranium-bearing region ([`Birth`]) with a thermal (0.0253 eV) incident
//!   fission spectrum.

// Headless output is native-only.
#![cfg_attr(target_arch = "wasm32", allow(dead_code))]

use crate::history::History;
use crate::triso::model::{splitmix, uniform};
use outram_mc_libs::geometry::geometry::Geometry;
use outram_mc_libs::geometry::position::Position;
use outram_mc_libs::material::material::Material;
use outram_mc_libs::material::nuclide::Nuclide;
use outram_mc_libs::physics::fixed_source::{run_fixed_source_traced, FixedSource, FixedSourceSettings};
use outram_mc_libs::physics::track_output::{TrackEvent, TrackRecorder};

/// Incident energy for a fresh chain's birth spectrum: thermal fission, eV.
const FRESH_INCIDENT_EV: f64 = 0.0253;
/// A thermal neutron in graphite scatters hundreds of times (and in a
/// natural-uranium cell often over a thousand) before it is absorbed; a cap
/// far above that makes a truncated track a reportable anomaly, not the norm.
const MAX_STATES: usize = 400_000;

/// Where a fresh neutron (not from the previous fission) is born: uniformly
/// in the region that holds the uranium.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Birth {
    /// Anywhere in the cube `|x|, |y|, |z| < half` (the homogeneous mixture).
    Cube { half: f64 },
    /// Anywhere in the ball `|r| < r` (the uranium lump).
    Ball { r: f64 },
}

pub struct Physics {
    pub geometry: Geometry,
    pub materials: Vec<Material>,
    pub nuclides: Vec<Nuclide>,
    pub birth: Birth,
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
        let u235 = &phys.nuclides[super::model::N_U235];
        let from_fission = self.pending_fission.is_some();
        let (birth, e_in) = match self.pending_fission.take() {
            Some(site) => site,
            None => (self.random_point(phys.birth), FRESH_INCIDENT_EV),
        };
        let birth_energy_ev = u235.sample_fission_energy(e_in, &mut self.rng);
        let transport_seed = splitmix(&mut self.rng);
        let settings = FixedSourceSettings {
            n_particles: 1,
            n_batches: 1,
            seed: transport_seed,
            max_secondaries: 0,
            temperature_k: super::model::TEMPERATURE_K,
            ..Default::default()
        };
        let mut recorder = TrackRecorder::new(1, MAX_STATES);
        run_fixed_source_traced(
            &phys.geometry,
            &phys.materials,
            &phys.nuclides,
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

    fn random_point(&mut self, b: Birth) -> Position {
        let (half, ball) = match b {
            Birth::Cube { half } => (half, None),
            Birth::Ball { r } => (r, Some(r)),
        };
        loop {
            // A hair inside the region, so a birth never sits on a surface.
            let p = [0; 3].map(|_| (2.0 * uniform(&mut self.rng) - 1.0) * half * (1.0 - 1.0e-9));
            match ball {
                Some(r) if p[0] * p[0] + p[1] * p[1] + p[2] * p[2] >= r * r * (1.0 - 1.0e-9) => continue,
                _ => return Position::new(p[0], p[1], p[2]),
            }
        }
    }
}
