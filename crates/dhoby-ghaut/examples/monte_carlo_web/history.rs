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
