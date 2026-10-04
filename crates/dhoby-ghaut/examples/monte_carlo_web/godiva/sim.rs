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
use outram_mc_libs::geometry::geometry::Geometry;
use outram_mc_libs::geometry::position::Position;
use outram_mc_libs::physics::fixed_source::{run_fixed_source_traced, FixedSource, FixedSourceSettings};
use outram_mc_libs::physics::keff::{GenerationReport, KeffSettings, PowerIteration};
use outram_mc_libs::physics::track_output::{TrackEvent, TrackRecorder};

/// Incident energy for the birth spectrum of a neutron not born at the last
/// fission site, eV.
const FRESH_INCIDENT_EV: f64 = 1.0e6;
/// A fast neutron in Godiva makes a handful of collisions; any cap far above
/// that makes a truncated track a reportable anomaly.
const MAX_STATES: usize = 20_000;
/// At most this many source sites cross to the page per generation (to draw).
pub const MAX_SITES_SHOWN: usize = 3000;

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

/// The settings a reader chooses, and where the source starts.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct KeffConfig {
    pub n_particles: usize,
    pub n_inactive: usize,
    pub n_active: usize,
    pub seed: u64,
    /// Start from every neutron at the centre (Watch) rather than spread
    /// uniformly through the sphere (Run, as the recorded result did).
    pub point_source: bool,
    /// Send the next generation's source sites to the page (Watch draws them).
    pub want_sites: bool,
}

impl KeffConfig {
    /// The defaults of the Run k_eff panel: **the recorded result's own
    /// settings**, 5000 neutrons x [40 inactive + 120 active], seed 1, so a
    /// reader's run is one seed of the 32 the record pools.
    ///
    /// Sized on a measurement, not a guess: natively (i9-13900K, 16 logical
    /// cores, 1 thread used, 62 GB, Linux, CPU only, shared with other
    /// builds) the transport of these 800 000 histories took **2.9 s** on
    /// 2026-10-04; processing the three tapes at tolerance 0.001 took 62 s.
    /// Transport is a few seconds even several times slower in a phone's
    /// browser; the data processing is the long part.
    ///
    /// That run (`--headless-keff 5000 40 120 1`) gave **k = 0.99942 ±
    /// 0.00183 (−58 ± 183 pcm)**: one draw, consistent with the record's
    /// −52 ± 27 pcm.
    pub const RUN_DEFAULT: KeffConfig =
        KeffConfig { n_particles: 5000, n_inactive: 40, n_active: 120, seed: 1, point_source: false, want_sites: false };
    /// The Watch-generations run: a small population started at the centre,
    /// enough generations for the entropy to rise and flatten.
    pub const WATCH_DEFAULT: KeffConfig =
        KeffConfig { n_particles: 1000, n_inactive: 15, n_active: 15, seed: 1, point_source: true, want_sites: true };

    pub fn settings(&self) -> KeffSettings {
        KeffSettings {
            n_particles: self.n_particles,
            n_inactive: self.n_inactive,
            n_active: self.n_active,
            seed: self.seed,
            temperature_k: model::TEMPERATURE_K,
            ..KeffSettings::default()
        }
    }
}

/// One generation as the page sees it: the library's report plus (Watch only)
/// a sample of where the next generation's neutrons start, projected on x-y.
#[derive(Clone, Debug, PartialEq)]
pub struct Generation {
    pub report: GenerationReport,
    pub sites: Vec<[f32; 2]>,
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
        let s = cfg.settings();
        let it = if cfg.point_source {
            PowerIteration::new_point_source(model::RADIUS_CM, &s, Position::ZERO)
        } else {
            PowerIteration::new(model::RADIUS_CM, &s)
        };
        Self { cfg, it: it.with_entropy_mesh(entropy_mesh()) }
    }

    /// The source before the first generation (Watch draws it).
    pub fn initial_sites(&self) -> Vec<[f32; 2]> {
        self.sites()
    }

    fn sites(&self) -> Vec<[f32; 2]> {
        let all = self.it.source_positions();
        let stride = all.len().div_ceil(MAX_SITES_SHOWN).max(1);
        all.iter().step_by(stride).map(|p| [p.x as f32, p.y as f32]).collect()
    }

    /// Run one generation. `None` once the run is over.
    pub fn step(&mut self, phys: &Physics) -> Option<Generation> {
        let report = self.it.step(&phys.data.material, &phys.data.nuclides)?;
        let sites = if self.cfg.want_sites { self.sites() } else { Vec::new() };
        Some(Generation { report, sites })
    }

    pub fn finished(&self) -> bool {
        self.it.finished()
    }
}

// ─── The openmc.run()-style console ──────────────────────────────────────────

/// The console header, as `openmc.run()` prints it for an eigenvalue run
/// with an entropy mesh.
pub const CONSOLE_HEADER: [&str; 2] = [
    " Bat./Gen.      k       Entropy         Average k",
    " =========   ========   ========   ====================",
];

/// One console line per generation, in `openmc.run()`'s layout: generation,
/// its `k`, the source entropy, and from the second active generation on the
/// running mean and its standard error.
pub fn console_line(r: &GenerationReport) -> String {
    let h = r.entropy.map_or("        ".to_string(), |h| format!("{h:8.5}"));
    let mut s = format!("{:>8}/1    {:7.5}   {h}", r.index + 1, r.k);
    if let Some((m, e)) = r.k_mean {
        if e > 0.0 {
            s.push_str(&format!("   {m:7.5} +/- {e:7.5}"));
        }
    }
    s
}

/// A headless Run k_eff: the console, then the counts and the result.
/// Deterministic for a given config and data. Native-only (the workspace's
/// headless-mode rule; it is also how the native live-run check is done).
pub fn headless_keff(phys: &Physics, cfg: KeffConfig, mut out: impl FnMut(&str)) -> outram_mc_libs::physics::keff::KeffResult {
    let mut k = Keff::new(cfg);
    for l in CONSOLE_HEADER {
        out(l);
    }
    let mut active = outram_mc_libs::physics::keff::HistoryCounts::default();
    let mut production = 0.0;
    while let Some(g) = k.step(phys) {
        out(&console_line(&g.report));
        if g.report.active {
            active.add(&g.report.counts);
            production += g.report.production;
        }
    }
    let r = k.it.result();
    for l in summary_lines(&active, production, cfg, r.k_mean, r.k_std) {
        out(&l);
    }
    r
}

/// The closing summary, shared by the headless run and the app: the analog
/// counts over the active generations, `k` by counting, and the result
/// against the experiment and the record.
pub fn summary_lines(
    c: &outram_mc_libs::physics::keff::HistoryCounts,
    production: f64,
    cfg: KeffConfig,
    k_mean: f64,
    k_std: f64,
) -> Vec<String> {
    let mut v = Vec::new();
    let t = c.tracked.max(1) as f64;
    let f = c.fissions();
    let pct = |x: u64| 100.0 * x as f64 / t;
    v.push(String::new());
    v.push(format!(" Neutrons followed (active): {} ({} source + {} (n,xn) secondaries)",
        c.tracked, cfg.n_particles * cfg.n_active, c.tracked as i64 - (cfg.n_particles * cfg.n_active) as i64));
    v.push(format!("   leaked     {:>9}  {:5.1} %", c.leaked, pct(c.leaked)));
    v.push(format!("   captured   {:>9}  {:5.1} %", c.captured, pct(c.captured)));
    v.push(format!("   fissioned  {:>9}  {:5.1} %", f, pct(f)));
    let [b0, b1, b2] = c.fissions_by_energy;
    v.push(format!("   fissions by incident energy: < 0.625 eV {b0}, 0.625 eV-100 keV {b1}, > 100 keV {b2}"));
    if f > 0 {
        let nu = production / f as f64;
        let p_nl = 1.0 - c.leaked as f64 / t;
        let k_inf = nu * f as f64 / (f + c.captured) as f64;
        v.push(format!("   nu-bar {nu:.4}   k_inf = nu F/(F+C) = {k_inf:.5}   P_NL = 1 - L/N = {p_nl:.5}"));
        v.push(format!("   k_inf x P_NL = {:.5}   (k = nu F / source neutrons = {:.5})",
            k_inf * p_nl, production / (cfg.n_particles * cfg.n_active) as f64));
    }
    v.push(String::new());
    v.push(format!(" k-effective = {k_mean:.5} +/- {k_std:.5}   ({:+.0} +/- {:.0} pcm from 1)", (k_mean - 1.0) * 1e5, k_std * 1e5));
    v.push(format!(" Experiment (ICSBEP HEU-MET-FAST-001): {:.4} +/- {:.4}",
        outram_mc_libs::vv::godiva::BENCHMARK_K, outram_mc_libs::vv::godiva::BENCHMARK_SIGMA));
    v.push(format!(" Recorded (32 seeds x 5000 x [40+120], 2026-09-30): {:.5} +/- {:.5} ({:+.0} +/- {:.0} pcm)",
        RECORDED_K, RECORDED_SEM, (RECORDED_K - 1.0) * 1e5, RECORDED_SEM * 1e5));
    v
}

/// The recorded result for this model and data, to compare a reader's run
/// with: **route 4** (outram-mc reading ENDF/B-VIII.0 directly) of
/// `crates/outram-mc-libs/verification_and_validation/icsbep/five_route_keff_2026_09_29.md`,
/// "Results — after the OpenMC-parity audit": **0.99948 ± 0.00027
/// (−52 ± 27 pcm)**, 32 seeds × 5000 histories × [40 inactive + 120 active],
/// 2026-09-30, commit `0414bc8277`. The ± is the standard error of the
/// 32-seed mean (seed-to-seed sd 151 pcm). Quoted, not re-measured here.
pub const RECORDED_K: f64 = 0.99948;
/// Standard error of [`RECORDED_K`].
pub const RECORDED_SEM: f64 = 0.00027;
/// Seed-to-seed standard deviation of one run at the recorded settings.
pub const RECORDED_SD_ONE_RUN: f64 = 0.00151;
