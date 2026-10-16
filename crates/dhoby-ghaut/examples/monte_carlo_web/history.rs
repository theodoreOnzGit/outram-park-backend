//! One finished neutron history, as every animated rung draws it.
//!
//! The rungs differ in geometry and data, not in what a history is: a
//! [`Track`] recorded by outram-mc-libs' own transport, plus the numbers the
//! panel shows. Moved here from the TRISO rung's `sim.rs` (2026-10-04, gh:#521)
//! when the Godiva rung started producing the same thing.

// Headless output is native-only; the browser build does not call it.
#![cfg_attr(target_arch = "wasm32", allow(dead_code))]

use outram_mc_libs::geometry::position::Position;
use outram_mc_libs::physics::keff::HistoryCounts;
use outram_mc_libs::physics::track_output::{Track, TrackEvent};

/// The conventional thermal / epithermal boundary (the cadmium cut-off), eV.
/// Used to count "thermalised" histories and to bin fissions.
pub const THERMAL_CUTOFF_EV: f64 = 0.625;

/// One finished history and the numbers derived from its track.
#[derive(Clone)]
pub struct History {
    pub index: u64,
    pub birth: Position,
    pub birth_energy_ev: f64,
    pub track: Track,
    pub outcome: Option<TrackEvent>,
    pub scatters: usize,
    pub crossings: usize,
    pub path_cm: f64,
    pub time_of_flight_s: f64,
    pub final_energy_ev: f64,
    pub min_energy_ev: f64,
    /// Whether this neutron was born at the previous neutron's fission site.
    pub from_fission: bool,
}

impl History {
    /// Derive the panel's numbers from a recorded track.
    pub fn from_track(index: u64, birth: Position, birth_energy_ev: f64, from_fission: bool, track: Track) -> Self {
        let outcome = track.outcome();
        let count = |ev: TrackEvent| track.states.iter().filter(|s| s.event == ev).count();
        let last = track.states.last();
        Self {
            index,
            birth,
            birth_energy_ev,
            outcome,
            scatters: count(TrackEvent::Scatter),
            crossings: count(TrackEvent::SurfaceCrossing),
            path_cm: track.path_length().unwrap_or(0.0),
            time_of_flight_s: last.map_or(0.0, |s| s.time),
            final_energy_ev: last.map_or(f64::NAN, |s| s.energy),
            min_energy_ev: track.states.iter().map(|s| s.energy).fold(f64::INFINITY, f64::min),
            from_fission,
            track,
        }
    }

    pub fn thermalised(&self) -> bool {
        self.min_energy_ev < THERMAL_CUTOFF_EV
    }

    /// The state indices where a flight at a new energy starts: the birth
    /// (state 0) and every scatter. A surface crossing (a reflective wall, a
    /// cell boundary) changes neither the energy nor the flight, so it is
    /// not one; the history's end (capture, fission, leak) starts nothing.
    pub fn flight_starts(&self) -> Vec<usize> {
        let st = &self.track.states;
        (0..st.len()).filter(|&i| i == 0 || st[i].event == TrackEvent::Scatter).collect()
    }

    /// The energy the neutron flies at after each collision, eV: element 0 is
    /// the birth energy, element `n` the energy after its `n`-th scatter
    /// (each state carries the energy the neutron leaves it with). The σ(E)
    /// panel's collision strip draws this series (gh:#906).
    pub fn collision_energies(&self) -> Vec<f64> {
        self.flight_starts().into_iter().map(|i| self.track.states[i].energy).collect()
    }
}

/// Running totals over every history shown.
#[derive(Default, Clone)]
pub struct Stats {
    pub histories: u64,
    pub fissions: u64,
    pub captures: u64,
    pub leaks: u64,
    pub other: u64,
    pub thermalised: u64,
    pub scatters: u64,
    pub path_cm: f64,
    /// The same leaked / captured / fissioned tally the Run k_eff mode reads
    /// from the power iteration ([`HistoryCounts`]), here counted from the
    /// recorded tracks, with fissions binned by incident energy.
    pub counts: HistoryCounts,
}

impl Stats {
    pub fn add(&mut self, h: &History) {
        self.histories += 1;
        self.counts.tracked += 1;
        match h.outcome {
            Some(TrackEvent::Fission) => {
                self.fissions += 1;
                // The incident energy of the fissioning neutron: the energy it
                // flew in, recorded on the last state.
                let e = h.final_energy_ev;
                let [lo, hi] = HistoryCounts::FISSION_BIN_EDGES_EV;
                self.counts.fissions_by_energy[if e < lo { 0 } else if e < hi { 1 } else { 2 }] += 1;
            }
            Some(TrackEvent::Absorption) => {
                self.captures += 1;
                self.counts.captured += 1;
            }
            Some(TrackEvent::Leak) => {
                self.leaks += 1;
                self.counts.leaked += 1;
            }
            _ => self.other += 1,
        }
        self.thermalised += h.thermalised() as u64;
        self.scatters += h.scatters as u64;
        self.path_cm += h.path_cm;
    }
}

pub fn outcome_name(o: Option<TrackEvent>) -> &'static str {
    match o {
        Some(TrackEvent::Fission) => "fission",
        Some(TrackEvent::Absorption) => "capture",
        Some(TrackEvent::Leak) => "leak",
        Some(TrackEvent::Lost) => "lost",
        Some(TrackEvent::Rouletted) => "rouletted",
        Some(TrackEvent::Born) | Some(TrackEvent::SurfaceCrossing) | Some(TrackEvent::Scatter) => "unterminated",
        None => "empty",
    }
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;
    use outram_mc_libs::geometry::position::Direction;
    use outram_mc_libs::physics::track_output::TrackState;

    /// A toy capture history along x, one cm per state: born at 2 MeV, a
    /// wall crossing (no energy change), scatters to 1 MeV and 1 keV, then
    /// captured at 1 keV.
    pub(crate) fn toy() -> History {
        let s = |x: f64, e: f64, ev: TrackEvent| TrackState {
            r: Position::new(x, 0.0, 0.0),
            u: Direction::new(1.0, 0.0, 0.0),
            energy: e,
            time: x,
            weight: 1.0,
            cell: 0,
            material: Some(0),
            event: ev,
        };
        let track = Track {
            states: vec![
                s(0.0, 2.0e6, TrackEvent::Born),
                s(1.0, 2.0e6, TrackEvent::SurfaceCrossing),
                s(2.0, 1.0e6, TrackEvent::Scatter),
                s(3.0, 1.0e3, TrackEvent::Scatter),
                s(4.0, 1.0e3, TrackEvent::Absorption),
            ],
            dropped_states: 0,
        };
        History::from_track(0, Position::new(0.0, 0.0, 0.0), 2.0e6, false, track)
    }

    /// Flights start at the birth and at each scatter, never at a crossing or
    /// at the end; the energies are the ones each flight is flown at.
    #[test]
    fn collision_energies_skip_crossings_and_the_end() {
        let h = toy();
        assert_eq!(h.flight_starts(), vec![0, 2, 3]);
        assert_eq!(h.collision_energies(), vec![2.0e6, 1.0e6, 1.0e3]);
        assert_eq!(h.scatters, h.collision_energies().len() - 1);
    }
}

// ─── Headless (workspace hard rule) ──────────────────────────────────────────

pub const CSV_HEADER: &str = "history,from_fission,birth_x_cm,birth_y_cm,birth_energy_eV,outcome,\
scatters,crossings,path_cm,time_of_flight_s,final_energy_eV,min_energy_eV,states,dropped_states";

pub fn csv_row(h: &History) -> String {
    format!(
        "{},{},{:.6e},{:.6e},{:.6e},{},{},{},{:.6e},{:.6e},{:.6e},{:.6e},{},{}",
        h.index, h.from_fission as u8, h.birth.x, h.birth.y, h.birth_energy_ev,
        outcome_name(h.outcome), h.scatters, h.crossings, h.path_cm, h.time_of_flight_s,
        h.final_energy_ev, h.min_energy_ev, h.track.states.len(), h.track.dropped_states,
    )
}
