//! What the Godiva rung computes. Nothing here re-implements transport.
//!
//! - **Watch, neutrons** ([`Chain`]): one neutron at a time, each a call to
//!   outram-mc-libs' [`run_fixed_source_traced`] on the sphere, with ONE
//!   source particle, `max_secondaries = 0` (a fission ends the history) and
//!   a [`TrackRecorder`], exactly as the TRISO rung does. The first neutron
//!   is born at the centre. After a fission the next one is born at that
//!   fission site from the U-235 ENDF/B-VIII.0 fission spectrum at the
//!   incident energy (the track does not say which nuclide fissioned; in
//!   Godiva it is U-235 ~94 % of the time by atoms). After a leak or a capture
//!   the next one starts at a uniformly random point in the sphere with a
//!   1 MeV-incident fission spectrum. That chaining is the illustration; each
//!   track is one real history.
//! - **Watch, generations** and **Run k_eff** ([`Keff`]): outram-mc-libs'
//!   [`PowerIteration`], the single-thread reference power iteration that
//!   `run_keff` itself is (bit for bit, pinned by
//!   `stepping_the_power_iteration_is_the_single_thread_run_bit_for_bit`),
//!   stepped one generation at a time so each generation can be printed and
//!   plotted as it finishes. Watch starts it from a point source at the
//!   centre to show the source spreading; Run starts it as
//!   `godiva_keff_endf_local.rs` does.

// Headless output is native-only.
#![cfg_attr(target_arch = "wasm32", allow(dead_code))]

use super::model::{self, NuclearData, N_U235};
use crate::history::History;
use crate::keff::{console_line, sample_sites, summary_lines, Generation, KeffConfig, CONSOLE_HEADER};
use outram_mc_libs::geometry::geometry::Geometry;
use outram_mc_libs::geometry::position::Position;
use outram_mc_libs::physics::fixed_source::{run_fixed_source_traced, FixedSource, FixedSourceSettings};
use outram_mc_libs::physics::keff::{HistoryCounts, KeffResult, KeffSettings, PowerIteration};
use outram_mc_libs::physics::track_output::{TrackEvent, TrackRecorder};

/// Incident energy for the birth spectrum of a neutron not born at the last
/// fission site, eV.
const FRESH_INCIDENT_EV: f64 = 1.0e6;
/// A fast neutron in Godiva makes a handful of collisions; any cap far above
/// that makes a truncated track a reportable anomaly.
const MAX_STATES: usize = 20_000;

pub struct Physics {
    pub geometry: Geometry,
    pub data: NuclearData,
    pub materials: Vec<outram_mc_libs::material::material::Material>,
}

impl Physics {
    pub fn new(data: NuclearData) -> Self {
        let materials = vec![data.material.clone()];
        Self { geometry: model::build_geometry(), data, materials }
    }
}

// ─── Watch: one neutron at a time ────────────────────────────────────────────

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
            None if self.next_index == 0 => (Position::ZERO, FRESH_INCIDENT_EV),
            None => (self.random_point(), FRESH_INCIDENT_EV),
        };
        let birth_energy_ev = u235.sample_fission_energy(e_in, &mut self.rng);
        let transport_seed = crate::triso::model::splitmix(&mut self.rng);
        let settings = FixedSourceSettings {
            n_particles: 1,
            n_batches: 1,
            seed: transport_seed,
            max_secondaries: 0,
            temperature_k: model::TEMPERATURE_K,
            ..Default::default()
        };
        let mut recorder = TrackRecorder::new(1, MAX_STATES);
        run_fixed_source_traced(
            &phys.geometry,
            &phys.materials,
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

    /// Uniform point in the sphere.
    fn random_point(&mut self) -> Position {
        use crate::triso::model::uniform;
        loop {
            let p = [0; 3].map(|_| (2.0 * uniform(&mut self.rng) - 1.0) * model::RADIUS_CM);
            if p[0] * p[0] + p[1] * p[1] + p[2] * p[2] < model::RADIUS_CM * model::RADIUS_CM {
                return Position::new(p[0], p[1], p[2]);
            }
        }
    }
}

// ─── Generations: the real power iteration ───────────────────────────────────

fn settings(cfg: &KeffConfig) -> KeffSettings {
    KeffSettings {
        n_particles: cfg.n_particles,
        n_inactive: cfg.n_inactive,
        n_active: cfg.n_active,
        seed: cfg.seed,
        temperature_k: model::TEMPERATURE_K,
        ..KeffSettings::default()
    }
}

/// The entropy mesh: 5 x 5 x 5 on the sphere's bounding box, the mesh of the
/// Shannon-entropy verification against OpenMC.
fn entropy_mesh() -> outram_mc_libs::tally::mesh::RegularMesh {
    PowerIteration::bounding_box_mesh(model::RADIUS_CM, 5)
}

pub struct Keff {
    pub cfg: KeffConfig,
    it: PowerIteration,
}

impl Keff {
    pub fn new(cfg: KeffConfig) -> Self {
        let s = settings(&cfg);
        let it = if cfg.point_source {
            PowerIteration::new_point_source(model::RADIUS_CM, &s, Position::ZERO)
        } else {
            PowerIteration::new(model::RADIUS_CM, &s)
        };
        Self { cfg, it: it.with_entropy_mesh(entropy_mesh()) }
    }

    /// The source before the first generation (Watch draws it).
    pub fn initial_sites(&self) -> Vec<[f32; 2]> {
        sample_sites(&self.it.source_positions())
    }

    /// Run one generation. `None` once the run is over.
    pub fn step(&mut self, phys: &Physics) -> Option<Generation> {
        let report = self.it.step(&phys.data.material, &phys.data.nuclides)?;
        let sites = if self.cfg.want_sites { sample_sites(&self.it.source_positions()) } else { Vec::new() };
        Some(Generation { report, sites })
    }

    pub fn finished(&self) -> bool {
        self.it.finished()
    }

    pub fn result(&self) -> KeffResult {
        self.it.result()
    }
}

/// A headless Run k_eff: the console, then the counts and the result.
/// Deterministic for a given config and data. Native-only (the workspace's
/// headless-mode rule; it is also how the native live-run check is done).
pub fn headless_keff(phys: &Physics, cfg: KeffConfig, mut out: impl FnMut(&str)) -> KeffResult {
    let mut k = Keff::new(cfg);
    for l in CONSOLE_HEADER {
        out(l);
    }
    let mut active = HistoryCounts::default();
    let mut production = 0.0;
    while let Some(g) = k.step(phys) {
        out(&console_line(&g.report));
        if g.report.active {
            active.add(&g.report.counts);
            production += g.report.production;
        }
    }
    let r = k.result();
    for l in summary_lines(&active, production, cfg, r.k_mean, r.k_std, Some(super::REFERENCE)) {
        out(&l);
    }
    r
}

/// The rung once its data are processed: what the engine serves.
pub struct Loaded {
    pub phys: Physics,
    pub chain: Chain,
    pub keff: Option<Keff>,
}

impl crate::rungs::LoadedRung for Loaded {
    fn run_next(&mut self) -> History {
        self.chain.run_next(&self.phys)
    }
    fn keff_start(&mut self, cfg: KeffConfig) -> Result<Vec<[f32; 2]>, String> {
        let k = Keff::new(cfg);
        let sites = if cfg.want_sites { k.initial_sites() } else { Vec::new() };
        self.keff = Some(k);
        Ok(sites)
    }
    fn keff_step(&mut self) -> Option<Generation> {
        let phys = &self.phys;
        self.keff.as_mut().and_then(|k| k.step(phys))
    }
    fn keff_finished(&self) -> bool {
        self.keff.as_ref().is_none_or(|k| k.finished())
    }
}
