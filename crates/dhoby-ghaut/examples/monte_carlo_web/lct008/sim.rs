//! What the LCT-008 rung computes. Nothing here re-implements transport.
//!
//! **Watch, one neutron at a time** ([`Chain`]): each neutron is a call to
//! outram-mc-libs' [`run_fixed_source_traced`] on the assembled core, with
//! ONE source particle, `max_secondaries = 0` (so a fission ends the
//! history) and a [`TrackRecorder`], exactly as the TRISO and Godiva rungs do.
//! The tracks go through the same nested-lattice locate and surface crossing
//! that the k-eigenvalue runs use.
//!
//! **Chaining is the illustration:**
//! - after a fission, the next neutron is born at that fission site, with
//!   energy drawn from the U-235 ENDF/B-VIII.0 fission spectrum at the
//!   incident energy. The track does not say which nuclide fissioned; at
//!   2.459 w/o almost all fissions here are U-235.
//! - after a leak or a capture, the next neutron starts in a uniformly chosen
//!   fuel rod, at a uniform point of its pellet (any height), with a thermal
//!   (0.0253 eV) incident fission spectrum.

// Headless output is native-only.
#![cfg_attr(target_arch = "wasm32", allow(dead_code))]

use super::model::{self, NuclearData, PinKind, N_U235};
use crate::history::History;
use crate::triso::model::{splitmix, uniform};
use outram_mc_libs::geometry::geometry::Geometry;
use outram_mc_libs::geometry::position::Position;
use outram_mc_libs::physics::fixed_source::{run_fixed_source_traced, FixedSource, FixedSourceSettings};
use outram_mc_libs::physics::track_output::{TrackEvent, TrackRecorder};

/// Incident energy for a fresh chain's birth spectrum: thermal fission, eV.
const FRESH_INCIDENT_EV: f64 = 0.0253;
/// A thermal neutron in water scatters a few hundred times at most before it
/// is absorbed; a cap far above that makes a truncated track a reportable
/// anomaly, not the norm.
const MAX_STATES: usize = 200_000;

pub struct Physics {
    pub geometry: Geometry,
    pub data: NuclearData,
    /// Fuel-rod centres (x, y), cm, read from the assembled geometry.
    pub fuel: Vec<(f64, f64)>,
}

impl Physics {
    pub fn new(data: NuclearData) -> Self {
        let geometry = model::build_geometry(&data.materials);
        let fuel = model::pins(&geometry).into_iter().filter(|p| p.2 == PinKind::Fuel).map(|p| (p.0, p.1)).collect();
        Self { geometry, data, fuel }
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
            None => (self.random_fuel_point(&phys.fuel), FRESH_INCIDENT_EV),
        };
        let birth_energy_ev = u235.sample_fission_energy(e_in, &mut self.rng);
        let transport_seed = splitmix(&mut self.rng);
        let settings = FixedSourceSettings {
            n_particles: 1,
            n_batches: 1,
            seed: transport_seed,
            max_secondaries: 0,
            temperature_k: model::TEMP_K,
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

    /// A uniform point in the pellet of a uniformly chosen fuel rod, at a
    /// uniform height inside the core.
    fn random_fuel_point(&mut self, fuel: &[(f64, f64)]) -> Position {
        let i = ((uniform(&mut self.rng) * fuel.len() as f64) as usize).min(fuel.len() - 1);
        let (cx, cy) = fuel[i];
        let r = model::R_FUEL * (1.0 - 1.0e-9) * uniform(&mut self.rng).sqrt();
        let t = std::f64::consts::TAU * uniform(&mut self.rng);
        let z = model::Z_LO + (model::Z_HI - model::Z_LO) * (0.001 + 0.998 * uniform(&mut self.rng));
        Position::new(cx + r * t.cos(), cy + r * t.sin(), z)
    }
}
