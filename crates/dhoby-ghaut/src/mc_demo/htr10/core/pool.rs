//! **Since gh:#1572 the two halves of the protocol are the library's**
//! (`crate::mc_web::pool`: `DataRelay` and `KeffPool`, moved from
//! here); [`PoolState`] wraps them with the core's bed, rods, zone
//! temperatures and pour, and its tests below still pin the whole.
//!
//! **The page side of the core's worker pool** (gh:#786): deal the jobs, relay
//! every product to every other worker, start the assembly, then run the
//! power iteration and deal each generation's chunks to idle workers.
//!
//! [`PoolState`] is the whole protocol as a state machine with no I/O: it
//! takes workers' events ([`PoolState::on_event`]) and hands out the
//! requests to send ([`PoolState::outbox`]), so it is tested natively with
//! scripted workers. [`CorePool`] wires it to real links (Web Workers in the
//! browser, threads natively).
//!
//! **No lag.** The page only relays and reduces: a product is relayed
//! without being read (a `Float64Array` outside the page's wasm memory), at
//! most [`RELAY_WORDS_PER_FRAME`] words a frame so no frame stalls on a copy;
//! a generation's reduction is a sum over its histories and a resample of its
//! bank (a few milliseconds for thousands of neutrons).
//!
//! **A run can fail without the pool failing** (gh:#817, maintainer
//! 2026-10-09: "Give an option to restart serially or in parallel if race
//! conditions or other errors occur"; "I don't want to recompute nuclear
//! data and start again"). Three kinds of failure are kept apart:
//!
//! - **a run failure** (a reply that cannot be read, a generation that cannot
//!   be added up, a chunk a worker could not transport): recorded on the run
//!   ([`RunState::failed`]); every worker keeps its data and core, and
//!   [`PoolState::restart_run`] starts the run again, in parallel or on one
//!   worker ([`RestartKind`]);
//! - **a dead worker** (its `onerror`, or a panic: its data are gone): marked
//!   dead ([`PoolState::alive`]); the run it was serving fails, and the
//!   survivors carry on. Only when none survive is the pool failed;
//! - **a pool failure** ([`PoolPhase::Failed`]): while the data are still
//!   being processed or assembled, or with no worker left. The data must be
//!   reprocessed.
//!
//! **Stale replies.** Every run has an epoch, carried by each source and
//! chunk request and echoed by its reply. A restarted run has a new epoch,
//! so a reply still in flight from the replaced run is recognised, discarded
//! and logged, never added to the new run. Each busy worker has exactly one
//! request outstanding, and a restart leaves it marked busy until that reply
//! (stale) arrives, so the new run never gives it a second request meanwhile.

use super::{AssemblyReport, CoreEv, CoreReq};
use crate::mc_demo::keff::KeffConfig;
use crate::mc_web::pool::{DataOut, DataRelay, KeffPool, RunReq};
pub use crate::mc_web::pool::{
    GenSummary, Note, RestartKind, RunOrigin, RELAY_WORDS_PER_FRAME,
};

/// The model a run started on: the bed's layers and the rods the core held
/// (the pool may be rebuilt with others after it ends; what the run's k is
/// compared with follows these, not the pool's).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct HtrRun {
    pub layers: usize,
    pub rods: [f64; super::random_bed::N_RODS],
}

/// A run in progress on the core (the library's, gh:#1572, carrying the
/// bed and rods it started on).
pub type RunState = crate::mc_web::pool::RunState<HtrRun>;

/// Where the pool is.
#[derive(Clone, Debug, PartialEq)]
pub enum PoolPhase {
    /// Processing and relaying the jobs.
    Processing,
    /// Every worker building its nuclides, core and majorant.
    Assembling,
    /// Every live worker holds the model at `layers`.
    Ready,
    /// The data must be reprocessed (module docs).
    Failed(String),
}

/// The pool's protocol (module docs): the library's data relay and k_eff
/// run (gh:#1572), with the HTR-10 core's bed, rods, zone temperatures and
/// pour.
pub struct PoolState {
    pub workers: usize,
    /// Processing and relaying the jobs (the library's).
    pub data: DataRelay,
    pub assembled: Vec<Option<AssemblyReport>>,
    pub layers: usize,
    /// Each control rod's insertion the workers' core is built with (gh:#1025),
    /// `[0, 1]`, rod `i` as `Htr10CoreDesign::rod_insertions` numbers them.
    /// All withdrawn until [`Self::set_rods`].
    pub rods: [f64; super::random_bed::N_RODS],
    /// The eight zone temperatures, K, the workers process and build at
    /// (gh:#1142). The record's 300.15 K until [`Self::with_temperatures`].
    pub temps: [f64; 8],
    pub phase: PoolPhase,
    pub started_s: f64,
    pub data_ready_s: Option<f64>,
    /// The k_eff run on the workers (the library's): its run, the workers
    /// alive, the log of what the reader should see.
    pub keff: KeffPool<HtrRun>,
    /// Requests waiting to go out.
    out: Vec<(usize, CoreReq)>,
}

impl PoolState {
    /// A pool of `workers` for jobs with these labels and costs, the core
    /// built at `layers`.
    pub fn new(workers: usize, labels: Vec<String>, costs: Vec<f64>, layers: usize, now_s: f64) -> Self {
        let workers = workers.max(1);
        Self {
            workers,
            data: DataRelay::new(workers, labels, costs),
            assembled: vec![None; workers],
            layers,
            rods: super::random_bed::WITHDRAWN,
            temps: nee_soon::htr10_rmc::zones::ZoneTemperatures::record().to_kelvin(),
            phase: PoolPhase::Processing,
            started_s: now_s,
            data_ready_s: None,
            keff: KeffPool::new(workers, "about 545 MB of nuclear data".into(), super::run_settings, super::TRACED_PER_GENERATION),
            out: Vec::new(),
        }
    }

    /// This pool, processing and building at zone temperatures `temps` (K,
    /// in `Htr10Zone::ALL` order; gh:#1142). The job list must be
    /// `super::jobs_at` of the same temperatures.
    #[must_use]
    pub fn with_temperatures(mut self, temps: [f64; 8]) -> Self {
        self.temps = temps;
        self
    }

    /// **A pool whose workers already hold their model** (gh:#905, #907):
    /// each loaded a benchmark rung through the ordinary engine load, so
    /// there is nothing to process, relay or assemble. Only the run
    /// protocol is used: the same source and chunk requests, epochs,
    /// restarts and reduction as the HTR-10 core. `per_worker` says what a
    /// worker holds, for the message if one stops.
    pub fn ready(workers: usize, per_worker: String, now_s: f64) -> Self {
        let mut s = Self::new(workers, Vec::new(), Vec::new(), 0, now_s);
        s.data = DataRelay::none(s.workers);
        s.phase = PoolPhase::Ready;
        s.data_ready_s = Some(now_s);
        s.keff.per_worker = per_worker;
        s
    }

    /// Where the data came from, one line (gh:#818: Leak Before Break, the
    /// page shows it): how many products were read from this browser's
    /// cache and how many processed now.
    pub fn data_source(&self) -> String {
        self.data.data_source()
    }

    /// Workers still alive.
    pub fn live_workers(&self) -> usize {
        self.keff.live_workers()
    }

    /// Fraction of the data work done, by cost (assembly counts as the last 15 %).
    pub fn progress(&self) -> f64 {
        let built = self.assembled.iter().filter(|a| a.is_some()).count() as f64 / self.workers as f64;
        0.85 * self.data.processed() + 0.15 * built
    }

    /// Why the current run failed, if it did.
    pub fn keff_failed(&self) -> Option<String> {
        self.keff.run.as_ref().and_then(|r| r.failed.clone())
    }

    /// The model a run starting now is on.
    fn model(&self) -> HtrRun {
        HtrRun { layers: self.layers, rods: self.rods }
    }

    /// The data are still being processed or assembled.
    fn data_phase(&self) -> bool {
        matches!(self.phase, PoolPhase::Processing | PoolPhase::Assembling)
    }

    /// The live workers whose models are all at `layers`: ready.
    fn check_ready(&mut self, now_s: f64) {
        let all = (0..self.workers)
            .filter(|&v| self.keff.alive[v])
            .all(|v| self.assembled[v].is_some_and(|a| a.layers == self.layers));
        if all && self.live_workers() > 0 {
            self.phase = PoolPhase::Ready;
            self.data_ready_s.get_or_insert(now_s);
        }
    }

    /// A worker's event.
    pub fn on_event(&mut self, w: usize, ev: CoreEv, now_s: f64) -> Vec<Note> {
        let mut notes = Vec::new();
        if w >= self.workers {
            return notes;
        }
        match ev {
            CoreEv::JobStarted { job } => self.data.on_started(job),
            CoreEv::DataNote(m) => self.keff.note(format!("worker {}: {m}", w + 1)),
            CoreEv::Product { job, secs, data, source } => self.data.on_product(w, job, secs, data, source),
            CoreEv::Assembled(r) => {
                // One console line per assembly, for an unattended browser
                // check of time and memory (gh:#1141).
                log::info!(
                    "worker pool: worker {} assembled N = {} in {:.1} s (nuclides {:.1} s, geometry {:.1} s, majorant {:.1} s), wasm memory {:.0} MB, {:.0} s after the pool started",
                    w + 1,
                    r.layers,
                    r.tapes_s + r.nuclides_s + r.geometry_s + r.majorant_s,
                    r.nuclides_s,
                    r.geometry_s,
                    r.majorant_s,
                    r.memory_mb,
                    now_s - self.started_s
                );
                self.assembled[w] = Some(r);
                self.check_ready(now_s);
            }
            CoreEv::Source { sites, seed, mesh, epoch } => self.keff.on_source(w, &sites, seed, &mesh, epoch, now_s),
            CoreEv::ChunkDone { data, secs, epoch } => notes = self.keff.on_chunk_done(w, &data, secs, epoch, now_s),
            // Pool members never pour (the page's own worker does).
            CoreEv::Pour(_) | CoreEv::PourDiagnostics(_) => {}
            CoreEv::Failed { message, epoch } => {
                if self.data_phase() {
                    // Its data are incomplete: the pool cannot be ready.
                    self.keff.idle(w);
                    self.phase = PoolPhase::Failed(format!(
                        "Worker {} could not build its data: {message}. Reload the page to \
                         start again, and please report this message at {}.",
                        w + 1,
                        crate::mc_web::pool::REPORT
                    ));
                } else {
                    self.keff.on_failed(w, &message, epoch);
                }
            }
        }
        notes
    }

    /// A worker's error event. Either a reply that could not be read (the
    /// worker is alive; the run it belonged to fails) or the worker itself
    /// failing (its data are gone: it is marked dead, gh:#817).
    pub fn on_error(&mut self, w: usize, msg: String) {
        let data_phase = self.data_phase();
        if let Some(why) = self.keff.on_error(w, msg, data_phase) {
            self.phase = PoolPhase::Failed(why);
        }
    }

    /// Start a run (the pool must be ready, with no run going).
    pub fn start_run(&mut self, cfg: KeffConfig, now_s: f64) -> Result<(), String> {
        if self.phase != PoolPhase::Ready {
            return Err("the data are not ready".into());
        }
        let model = self.model();
        self.keff.start_run(cfg, model, now_s)
    }

    /// **Restart a failed run** (gh:#817) with the same settings, in parallel
    /// on every live worker or serially on the first, keeping every worker's
    /// data. Replies still in flight for the failed run are discarded as they
    /// arrive (module docs).
    pub fn restart_run(&mut self, kind: RestartKind, now_s: f64) -> Result<(), String> {
        if self.phase != PoolPhase::Ready {
            return Err("the data are not ready".into());
        }
        let model = self.model();
        self.keff.restart_run(kind, model, now_s)
    }

    /// **Stop the run** the reader started: no more chunks are cut, replies
    /// still in flight are discarded as they arrive (it is no longer going),
    /// and a new run may start. Its generations so far stay on screen.
    pub fn stop_run(&mut self) {
        self.keff.stop_run();
    }

    /// Pause or resume: a paused run finishes the generation in flight and
    /// cuts no more.
    pub fn set_paused(&mut self, paused: bool) {
        self.keff.set_paused(paused);
    }

    /// Rebuild the core at another layer count (only between runs).
    pub fn set_layers(&mut self, layers: usize) -> Result<(), String> {
        if self.phase != PoolPhase::Ready {
            return Err("the data are not ready".into());
        }
        if self.keff.going() {
            return Err("a run is going".into());
        }
        if layers != self.layers {
            self.layers = layers;
            self.phase = PoolPhase::Assembling;
            self.keff.run = None;
            for w in (0..self.workers).filter(|&w| self.keff.alive[w]) {
                self.out.push((w, CoreReq::Layers { layers }));
            }
        }
        Ok(())
    }

    /// **Hand every worker the reader's pour and rebuild the core on it**
    /// (gh:#1244; only between runs, like [`Self::set_layers`]): each live
    /// worker gets the bed, then the rebuild, in that order.
    ///
    /// # Errors
    /// The data are not ready, or a run is going.
    pub fn use_pour(&mut self, b: &super::pour::PouredBed) -> Result<(), String> {
        if self.phase != PoolPhase::Ready {
            return Err("the data are not ready".into());
        }
        if self.keff.going() {
            return Err("a run is going".into());
        }
        let layers = super::random_bed::USER_POUR;
        self.layers = layers;
        self.phase = PoolPhase::Assembling;
        self.keff.run = None;
        for w in (0..self.workers).filter(|&w| self.keff.alive[w]) {
            self.out.push((w, CoreReq::pour_bed(b)));
            self.out.push((w, CoreReq::Layers { layers }));
        }
        Ok(())
    }

    /// Before the first assembly: every worker gets the reader's pour, so a
    /// pool started on [`super::random_bed::USER_POUR`] builds it.
    pub fn send_pour_first(&mut self, b: &super::pour::PouredBed) {
        for w in 0..self.workers {
            self.out.push((w, CoreReq::pour_bed(b)));
        }
    }

    /// **Rebuild every worker's core with the control rods at `rods`**
    /// (gh:#1025; only between runs, like [`Self::set_layers`]). The pool is
    /// assembling until every live worker has reported its rebuilt core.
    /// Nothing changes, and nothing is sent, when the rods are already there.
    ///
    /// # Errors
    /// The data are not ready, a run is going, or an insertion is outside
    /// `[0, 1]`.
    pub fn set_rods(&mut self, rods: [f64; super::random_bed::N_RODS]) -> Result<(), String> {
        if self.phase != PoolPhase::Ready {
            return Err("the data are not ready".into());
        }
        if self.keff.going() {
            return Err("a run is going".into());
        }
        if let Some(f) = rods.iter().find(|f| !(0.0..=1.0).contains(*f)) {
            return Err(format!("rod insertion {f} is not in [0, 1]"));
        }
        if rods.map(f64::to_bits) != self.rods.map(f64::to_bits) {
            self.rods = rods;
            self.phase = PoolPhase::Assembling;
            for v in 0..self.workers {
                self.assembled[v] = None;
            }
            for w in (0..self.workers).filter(|&w| self.keff.alive[w]) {
                self.out.push((w, CoreReq::Rods { rods }));
            }
        }
        Ok(())
    }

    /// The requests to send now: the first time, each worker's jobs; then
    /// relays (at most `word_budget` words, but always at least one), each
    /// worker's assembly once everything has reached it, and chunks to idle
    /// live workers (only the serial worker, for a serial run).
    pub fn outbox(&mut self, word_budget: usize) -> Vec<(usize, CoreReq)> {
        let mut out = std::mem::take(&mut self.out);
        if matches!(self.phase, PoolPhase::Failed(_)) {
            return out;
        }
        let (data, all_assembling) = self.data.outbox(word_budget, self.phase == PoolPhase::Processing);
        for d in data {
            out.push(match d {
                DataOut::Process { worker, jobs } => (worker, CoreReq::Process { jobs, temps: self.temps }),
                DataOut::Relay { worker, job, data } => (worker, CoreReq::Product { job, data }),
                DataOut::Assemble { worker } => (worker, CoreReq::Assemble { layers: self.layers, temps: self.temps }),
            });
        }
        if all_assembling {
            self.phase = PoolPhase::Assembling;
        }
        if self.phase == PoolPhase::Ready {
            for (w, r) in self.keff.outbox() {
                out.push(match r {
                    RunReq::Source { cfg, epoch } => (w, CoreReq::Source { cfg, epoch }),
                    RunReq::Chunk { cfg, data, epoch } => (w, CoreReq::Chunk { cfg, data, epoch }),
                });
            }
        }
        out
    }
}

// ─── Wired to links ──────────────────────────────────────────────────────────

/// The pool with its links.
pub struct CorePool {
    pub links: Vec<crate::web_demo::link::Link<crate::mc_demo::engine::Request, crate::mc_demo::engine::Event>>,
    pub state: PoolState,
    pub plan: crate::web_demo::pool::Plan,
}

impl CorePool {
    /// Start the workers (Web Workers in the browser, threads natively).
    pub fn start(
        ctx: &egui::Context,
        plan: crate::web_demo::pool::Plan,
        layers: usize,
        temps: &nee_soon::htr10_rmc::zones::ZoneTemperatures,
    ) -> Result<Self, String> {
        temps.check()?;
        let jobs = super::jobs_at(temps)?;
        let labels = jobs.iter().map(|j| j.label()).collect();
        let costs = jobs.iter().map(super::job_cost).collect();
        let mut links = Vec::new();
        for _ in 0..plan.workers {
            #[cfg(target_arch = "wasm32")]
            links.push(crate::web_demo::link::start_web::<
                crate::mc_demo::engine::Request,
                crate::mc_demo::engine::Event,
            >(
                ctx.clone(), crate::mc_demo::worker_js(), crate::mc_demo::engine::Event::Error
            )?);
            #[cfg(not(target_arch = "wasm32"))]
            {
                let c = ctx.clone();
                links.push(crate::web_demo::link::start_native(
                    crate::mc_demo::engine::McEngine::default(),
                    move || c.request_repaint(),
                ));
            }
        }
        let state = PoolState::new(
            plan.workers,
            labels,
            costs,
            layers,
            crate::web_demo::platform::now_s(),
        )
        .with_temperatures(temps.to_kelvin());
        let mut state = state;
        if let Some(b) = super::random_bed::poured() {
            state.send_pour_first(&b);
        }
        Ok(Self { links, state, plan })
    }

    /// Each frame: take every worker's events, send what is due.
    pub fn pump(&mut self) -> Vec<Note> {
        let now = crate::web_demo::platform::now_s();
        let mut notes = Vec::new();
        for (w, link) in self.links.iter().enumerate() {
            for e in link.drain() {
                match e {
                    crate::mc_demo::engine::Event::Core(ev) => notes.extend(self.state.on_event(w, ev, now)),
                    crate::mc_demo::engine::Event::Error(m) => {
                        self.state.on_error(w, m);
                        if !self.state.keff.alive[w] {
                            // Its state cannot be trusted and its memory is
                            // what a phone needs back.
                            link.terminate();
                        }
                    }
                    _ => {}
                }
            }
        }
        for (w, req) in self.state.outbox(RELAY_WORDS_PER_FRAME) {
            if let Some(l) = self.links.get(w) {
                l.send(crate::mc_demo::engine::Request::Core(req));
            }
        }
        notes
    }
}


impl CorePool {
    /// Stop every worker and free their memory (the pool is being replaced,
    /// gh:#817/#818). A dropped `Link` would leave its Web Worker running.
    pub fn terminate(&self) {
        for l in &self.links {
            l.terminate();
        }
    }
}

#[cfg(all(test, not(target_arch = "wasm32")))]
mod tests {
    use super::*;
    // What the protocol used to import here, before it moved to the
    // library (gh:#1572).
    use crate::mc_web::pool::{not_added_up, unreadable, CHUNKS_PER_WORKER, REPORT};
    use crate::web_demo::data_cache::Source;
    use crate::web_demo::link::{Floats, BAD_MESSAGE};
    use outram_mc_libs::physics::transport_csg::distributed::{DistributedPowerIteration, GenerationChunk, SourceSite};
    use std::collections::VecDeque;
    use outram_mc_libs::geometry::cell::{Cell, HalfSpaceSense, RegionToken};
    use outram_mc_libs::geometry::geometry::Geometry;
    use outram_mc_libs::geometry::position::Position;
    use outram_mc_libs::geometry::surface::{BoundaryType, Sphere, SurfaceKind};
    use outram_mc_libs::geometry::universe::Universe;
    use outram_mc_libs::material::material::{Material, NuclideComponent};
    use outram_mc_libs::material::nuclide::Nuclide;
    use outram_mc_libs::physics::transport_csg::distributed::transport_chunk;
    use outram_mc_libs::physics::transport_csg::SourceBox;

    fn report(layers: usize) -> AssemblyReport {
        AssemblyReport {
            layers,
            tapes_s: 0.0,
            nuclides_s: 0.0,
            geometry_s: 0.0,
            majorant_s: 0.0,
            memory_mb: 1.0,
        }
    }

    /// Scripted workers through the data phase: every worker gets its own
    /// jobs once, every product reaches every other worker exactly once and
    /// never back to its maker, and a worker's assembly is sent only after
    /// every product has been sent to it.
    #[test]
    /// The reader's pour (gh:#1244): between runs every live worker gets the
    /// bed and then the rebuild, in that order; not while the pool is
    /// assembling; and a pool started with a pour sends it before anything.
    fn a_pour_reaches_every_worker_before_its_rebuild() {
        let b = super::super::pour::poured_bed(&[0.0, 0.0, -0.1, 0.0, 0.0, 0.03], 0.57, 3).expect("bed");
        let mut p = PoolState::ready(3, String::new(), 0.0);
        assert!(p.use_pour(&b).is_ok());
        assert_eq!(p.phase, PoolPhase::Assembling);
        assert_eq!(p.layers, super::super::random_bed::USER_POUR);
        let out = p.outbox(10);
        for w in 0..3 {
            let mine: Vec<&CoreReq> = out.iter().filter(|(v, _)| *v == w).map(|(_, r)| r).collect();
            assert!(matches!(mine.as_slice(), [CoreReq::PourBed { version: 3, .. }, CoreReq::Layers { layers: 1 }]), "worker {w}");
            assert_eq!(mine[0].poured_bed().expect("bed"), b);
        }
        assert!(p.use_pour(&b).is_err(), "not while assembling");
        let mut q = PoolState::new(2, Vec::new(), Vec::new(), 1, 0.0);
        q.send_pour_first(&b);
        let out = q.outbox(10);
        assert!(matches!(out.first(), Some((0, CoreReq::PourBed { .. }))));
        assert!(matches!(out.get(1), Some((1, CoreReq::PourBed { .. }))));
    }

    #[test]
    fn every_product_reaches_every_other_worker_before_its_assembly() {
        let costs = vec![5.0, 1.0, 3.0, 2.0, 2.0];
        let labels = (0..5).map(|i| format!("job {i}")).collect();
        let mut p = PoolState::new(3, labels, costs, 12, 0.0);
        let first = p.outbox(10);
        let processed: Vec<(usize, Vec<usize>)> = first
            .iter()
            .filter_map(|(w, r)| match r {
                CoreReq::Process { jobs, .. } => Some((*w, jobs.clone())),
                _ => None,
            })
            .collect();
        assert_eq!(processed.len(), 3);
        let mut got: Vec<Vec<usize>> = vec![Vec::new(); 3];
        let mut assembled_after: Vec<Option<usize>> = vec![None; 3];
        for (w, jobs) in &processed {
            for &j in jobs {
                p.on_event(*w, CoreEv::JobStarted { job: j }, 1.0);
                p.on_event(
                    *w,
                    CoreEv::Product {
                        job: j,
                        secs: 1.0,
                        data: Floats::from_vec(vec![j as f64; 4]),
                        source: Source::Processed,
                    },
                    1.0,
                );
                got[*w].push(j);
            }
        }
        assert_eq!(p.phase, PoolPhase::Processing);
        for _ in 0..50 {
            // A small budget: one relay per call at most (each is 4 words).
            for (v, r) in p.outbox(3) {
                match r {
                    CoreReq::Product { job, data } => {
                        assert_ne!(p.data.owner[job], v, "a product went back to its maker");
                        assert_eq!(data.to_vec(), vec![job as f64; 4]);
                        assert!(!data.is_empty());
                        assert!(assembled_after[v].is_none(), "a product after the assembly");
                        got[v].push(job);
                    }
                    CoreReq::Assemble { layers, .. } => {
                        assert_eq!(layers, 12);
                        assembled_after[v] = Some(got[v].len());
                    }
                    _ => panic!("unexpected request"),
                }
            }
        }
        for v in 0..3 {
            let mut g = got[v].clone();
            g.sort_unstable();
            assert_eq!(g, vec![0, 1, 2, 3, 4], "worker {v}");
            assert_eq!(assembled_after[v], Some(5));
        }
        assert_eq!(p.phase, PoolPhase::Assembling);
        assert!(p.progress() > 0.8 && p.progress() < 0.9);
        for v in 0..3 {
            p.on_event(v, CoreEv::Assembled(report(12)), 2.0);
        }
        assert_eq!(p.phase, PoolPhase::Ready);
        assert_eq!(p.data_ready_s, Some(2.0));
        // Layers: everyone rebuilds, ready again when all report.
        assert!(p.set_layers(14).is_ok());
        assert_eq!(p.phase, PoolPhase::Assembling);
        assert_eq!(
            p.outbox(10)
                .iter()
                .filter(|(_, r)| matches!(r, CoreReq::Layers { layers: 14 }))
                .count(),
            3
        );
        // Rods (gh:#1025): the same, with the rod positions; a second call
        // with the same rods sends nothing.
        for v in 0..3 {
            p.on_event(v, CoreEv::Assembled(report(14)), 3.0);
        }
        assert_eq!(p.phase, PoolPhase::Ready);
        let mut rods = super::super::random_bed::WITHDRAWN;
        rods[2] = 1.0;
        assert!(p.set_rods([1.5; 10]).is_err());
        assert!(p.set_rods(rods).is_ok());
        assert_eq!(p.phase, PoolPhase::Assembling);
        assert_eq!(p.outbox(10).iter().filter(|(_, r)| matches!(r, CoreReq::Rods { rods: q } if *q == rods)).count(), 3);
        for v in 0..3 {
            assert_eq!(p.phase, PoolPhase::Assembling, "ready before worker {v} reported");
            p.on_event(v, CoreEv::Assembled(report(14)), 4.0);
        }
        assert_eq!(p.phase, PoolPhase::Ready);
        assert!(p.set_rods(rods).is_ok());
        assert_eq!(p.phase, PoolPhase::Ready);
        assert!(p.outbox(10).iter().all(|(_, r)| !matches!(r, CoreReq::Rods { .. })));
        assert!(p.set_layers(12).is_ok());
        // A worker that fails while the data are being (re)built fails the
        // pool: its share is missing.
        p.on_error(1, "boom".into());
        match &p.phase {
            // Informative in the browser: which worker, why it is likely, what to do.
            PoolPhase::Failed(m) => {
                assert!(
                    m.contains("Worker 2 of 3 stopped: boom") && m.contains("processed again"),
                    "{m}"
                );
            }
            other => panic!("{other:?}"),
        }
        assert!(!p.keff.alive[1] && p.keff.died[1].is_some());
        let m = not_added_up(1, "a result for generation 0 while reducing 1", 6, 2);
        assert!(
            m.contains("generation 1") && m.contains("not in the physics") && m.contains(REPORT),
            "{m}"
        );
        assert!(unreadable("a chunk result", 0, "short").contains("worker 1"));
        // A worker-side failure while building fails the pool too.
        let mut q = PoolState::new(2, vec!["a".into()], vec![1.0], 12, 0.0);
        let failed = CoreEv::Failed {
            message: "download".into(),
            epoch: super::super::NO_RUN,
        };
        q.on_event(0, failed, 0.0);
        assert!(matches!(&q.phase, PoolPhase::Failed(m) if m.contains("download")));
    }

    fn model() -> (Geometry, Vec<Material>, Vec<Nuclide>) {
        let nucs = vec![
            Nuclide::from_core("U235").expect("U235"),
            Nuclide::from_core("U238").expect("U238"),
        ];
        let mats = vec![Material {
            id: 1,
            name: "heu".into(),
            components: vec![
                NuclideComponent {
                    nuclide_idx: 0,
                    atom_density: 4.4994e-2,
                },
                NuclideComponent {
                    nuclide_idx: 1,
                    atom_density: 2.4984e-3,
                },
            ],
            temperature: 293.6,
        }];
        let geom = Geometry {
            surfaces: vec![SurfaceKind::Sphere(Sphere {
                x0: 0.0,
                y0: 0.0,
                z0: 0.0,
                r: 8.7,
                bc: BoundaryType::Vacuum,
            })],
            cells: vec![Cell::material(
                1,
                vec![RegionToken::HalfSpace {
                    surface_idx: 0,
                    sense: HalfSpaceSense::Inside,
                }],
                0,
                293.6,
            )],
            universes: vec![Universe {
                id: 0,
                cell_indices: vec![0],
            }],
            lattices: vec![],
            root_universe: 0,
        };
        (geom, mats, nucs)
    }

    /// A bare HEU sphere and scripted workers that answer with the real
    /// `transport_chunk`; replies are queued, so a test chooses when (and
    /// whether) each arrives.
    struct Bench {
        geom: Geometry,
        mats: Vec<Material>,
        nucs: Vec<Nuclide>,
        box_: SourceBox,
        mesh: [f64; 9],
        cfg: KeffConfig,
    }

    /// Replies sent but not yet delivered, oldest first.
    type Inflight = VecDeque<(usize, CoreEv)>;

    impl Bench {
        fn new() -> Self {
            let (geom, mats, nucs) = model();
            Self {
                geom,
                mats,
                nucs,
                box_: SourceBox {
                    lower: Position::new(-8.0, -8.0, -8.0),
                    upper: Position::new(8.0, 8.0, 8.0),
                },
                mesh: [-9.0, -9.0, -9.0, 9.0, 9.0, 9.0, 2.0, 2.0, 2.0],
                cfg: KeffConfig {
                    n_particles: 200,
                    n_inactive: 2,
                    n_active: 3,
                    seed: 77,
                    point_source: false,
                    want_sites: false,
                },
            }
        }

        /// One coordinator, one chunk per generation: the answer every
        /// pool must give, bit for bit.
        fn reference(&self) -> Vec<u64> {
            let settings = super::super::run_settings(&self.cfg);
            let mut it = DistributedPowerIteration::new(
                &self.geom, &self.mats, &self.nucs, self.box_, &settings,
            );
            while !it.finished() {
                let r: Vec<_> = it
                    .chunks(1, 0)
                    .iter()
                    .map(|c| transport_chunk(&self.geom, &self.mats, &self.nucs, &[], &settings, c))
                    .collect();
                it.finish_generation(r, Some(&super::super::mesh_from_words(&self.mesh)))
                    .expect("reduce");
            }
            it.k_by_generation().iter().map(|k| k.to_bits()).collect()
        }

        /// A worker's reply to a run request.
        fn reply(&self, req: CoreReq) -> Option<CoreEv> {
            Some(match req {
                CoreReq::Source { cfg, epoch } => {
                    let (s, seed) = DistributedPowerIteration::initial_source(
                        &self.geom,
                        &self.mats,
                        &self.nucs,
                        self.box_,
                        &super::super::run_settings(&cfg),
                    );
                    CoreEv::Source {
                        sites: Floats::from_vec(SourceSite::encode(&s)),
                        seed,
                        mesh: self.mesh,
                        epoch,
                    }
                }
                CoreReq::Chunk { cfg, data, epoch } => {
                    let c = GenerationChunk::from_f64s(&data.to_vec()).expect("chunk");
                    let r = transport_chunk(
                        &self.geom,
                        &self.mats,
                        &self.nucs,
                        &[],
                        &super::super::run_settings(&cfg),
                        &c,
                    );
                    CoreEv::ChunkDone {
                        data: Floats::from_vec(r.to_f64s()),
                        secs: 0.0,
                        epoch,
                    }
                }
                _ => return None,
            })
        }

        /// A pool of `workers`, data ready.
        fn ready_pool(&self, workers: usize) -> PoolState {
            let mut p = PoolState::new(workers, vec!["only".into()], vec![1.0], 12, 0.0);
            p.on_event(
                0,
                CoreEv::Product {
                    job: 0,
                    secs: 0.0,
                    data: Floats::from_vec(vec![1.0]),
                    source: Source::Cached { created_ms: 0.0, code: "test".into() },
                },
                0.0,
            );
            let _ = p.outbox(1 << 20);
            for v in 0..workers {
                p.on_event(v, CoreEv::Assembled(report(12)), 0.0);
            }
            assert_eq!(p.phase, PoolPhase::Ready);
            p
        }

        /// Send what the pool asks, queue the replies, deliver the oldest,
        /// until the run is done or `until(k so far)` says stop. Returns the
        /// k of each generation delivered, the workers asked, and whether
        /// the run finished.
        fn drive(
            &self,
            p: &mut PoolState,
            inflight: &mut Inflight,
            until: impl Fn(&[u64]) -> bool,
        ) -> (Vec<u64>, Vec<usize>, bool) {
            let (mut ks, mut asked, mut done) = (Vec::new(), Vec::new(), false);
            for _ in 0..5000 {
                for (w, req) in p.outbox(1 << 20) {
                    asked.push(w);
                    if let Some(ev) = self.reply(req) {
                        inflight.push_back((w, ev));
                    }
                }
                let Some((w, ev)) = inflight.pop_front() else {
                    break;
                };
                for n in p.on_event(w, ev, 1.0) {
                    match n {
                        Note::Generation(g) => ks.push(g.gen.k.to_bits()),
                        Note::RunDone => done = true,
                    }
                }
                if done || until(&ks) {
                    break;
                }
            }
            (ks, asked, done)
        }

        /// Send the requests due now and queue their replies.
        fn send_due(&self, p: &mut PoolState, inflight: &mut Inflight) {
            for (w, req) in p.outbox(1 << 20) {
                if let Some(ev) = self.reply(req) {
                    inflight.push_back((w, ev));
                }
            }
        }
    }

    /// A run through the pool with scripted workers (the transport is the
    /// real `transport_chunk` on a small sphere) gives, for 1 and for 3
    /// workers, the same `k` in every generation bit for bit as one
    /// `DistributedPowerIteration` cut in one chunk: the reduction does not
    /// depend on the pool.
    #[test]
    fn the_pool_answer_does_not_depend_on_the_worker_count() {
        let b = Bench::new();
        let reference = b.reference();
        for workers in [1, 3] {
            let mut p = b.ready_pool(workers);
            p.start_run(b.cfg, 0.0).expect("start");
            // Regression (phone, 2026-10-09): a second Start while the
            // initial source is still being sampled is refused.
            let again = p.start_run(b.cfg, 0.5).unwrap_err();
            assert!(again.contains("already going"), "{again}");
            // The source request, answered twice: the duplicate must be
            // discarded, not queue generation 0 a second time.
            let out = p.outbox(1 << 20);
            assert_eq!(out.len(), 1);
            let (w, req) = out.into_iter().next().expect("source request");
            let ev = b.reply(req).expect("source");
            let CoreEv::Source {
                sites,
                seed,
                mesh,
                epoch,
            } = &ev
            else {
                panic!("not a source")
            };
            let dup = CoreEv::Source {
                sites: sites.clone(),
                seed: *seed,
                mesh: *mesh,
                epoch: *epoch,
            };
            let mut inflight = VecDeque::from([(w, ev), (w, dup)]);
            let (ks, _, done) = b.drive(&mut p, &mut inflight, |_| false);
            assert!(done, "{workers} workers: the run did not finish");
            assert_eq!(ks, reference, "{workers} workers");
            assert_eq!(p.keff.stale_discarded, 1, "the duplicate source is discarded, visibly");
            assert!(p.keff.run.as_ref().is_some_and(RunState::finished));
            // Pausing a finished run does nothing; a run that has not failed
            // cannot be restarted; a new run can start.
            p.set_paused(true);
            p.set_paused(false);
            assert!(p.restart_run(RestartKind::Parallel, 2.0).is_err());
            assert!(p.start_run(b.cfg, 2.0).is_ok());
        }
    }

    /// **A restart with chunks in flight** (gh:#817): generation 2's chunks
    /// are out on three workers when one reply arrives garbled. The run
    /// fails and the pool stays ready. A parallel restart begins while two
    /// workers are still busy with the old run; their replies, arriving
    /// late, are discarded and logged. The restarted run gives the reference
    /// `k` in every generation, bit for bit.
    #[test]
    fn a_restart_discards_stale_replies_and_gives_the_same_k() {
        let b = Bench::new();
        let reference = b.reference();
        let mut p = b.ready_pool(3);
        p.start_run(b.cfg, 0.0).expect("start");
        let mut inflight = Inflight::new();
        let (ks, _, _) = b.drive(&mut p, &mut inflight, |ks| ks.len() == 2);
        assert_eq!(ks, reference[..2]);
        b.send_due(&mut p, &mut inflight);
        assert!(inflight.len() >= 3, "generation 2 is out on every worker");
        // One reply arrives garbled.
        let (w0, _) = inflight.pop_front().expect("a reply");
        let bad = CoreEv::ChunkDone {
            data: Floats::from_vec(vec![1.0, 2.0]),
            secs: 0.0,
            epoch: 1,
        };
        assert!(p.on_event(w0, bad, 1.0).is_empty());
        assert_eq!(p.phase, PoolPhase::Ready, "a run failure is not a pool failure");
        let why = p.keff.run.as_ref().and_then(|r| r.failed.clone()).expect("failed");
        assert!(why.contains("could not be read") && why.contains("restart"), "{why}");
        assert!(p.outbox(1 << 20).is_empty(), "a failed run deals nothing");
        p.restart_run(RestartKind::Parallel, 1.0).expect("restart");
        let run = p.keff.run.as_ref().expect("run");
        assert_eq!(run.epoch, 2);
        assert!(
            run.describe().contains("restart in parallel on 3 workers"),
            "{}",
            run.describe()
        );
        assert!(
            p.keff.log.iter().any(|l| l.contains("restart in parallel") && l.contains("still busy")),
            "{:?}",
            p.keff.log
        );
        // The old run's replies are still queued ahead of the new run's.
        let before = p.keff.stale_discarded;
        let (ks, _, done) = b.drive(&mut p, &mut inflight, |_| false);
        assert!(done);
        assert!(p.keff.stale_discarded >= before + 2, "{} discarded", p.keff.stale_discarded - before);
        assert!(
            p.keff.log.iter().any(|l| l.contains("discarded a chunk result")),
            "{:?}",
            p.keff.log
        );
        assert_eq!(ks, reference, "the restarted run");
    }

    /// **A serial restart** (gh:#817): after a worker reports a failed
    /// chunk, "Restart run serially" sends every request to one worker, and
    /// the run gives the reference `k` bit for bit.
    #[test]
    fn a_serial_restart_uses_one_worker_and_gives_the_same_k() {
        let b = Bench::new();
        let reference = b.reference();
        let mut p = b.ready_pool(3);
        p.start_run(b.cfg, 0.0).expect("start");
        let mut inflight = Inflight::new();
        b.drive(&mut p, &mut inflight, |ks| ks.len() == 1);
        let failed = CoreEv::Failed {
            message: "core worker: out of memory".into(),
            epoch: 1,
        };
        p.on_event(2, failed, 1.0);
        assert_eq!(p.phase, PoolPhase::Ready);
        assert!(p
            .keff.run
            .as_ref()
            .and_then(|r| r.failed.as_deref())
            .is_some_and(|m| m.contains("out of memory")));
        p.restart_run(RestartKind::Serial, 1.0).expect("restart");
        assert!(p.keff.run.as_ref().is_some_and(|r| r.describe().contains("serial restart")));
        // The old run's replies arrive first: all stale.
        while let Some((w, ev)) = inflight.pop_front() {
            assert!(p.on_event(w, ev, 1.0).is_empty());
        }
        let (ks, asked, done) = b.drive(&mut p, &mut inflight, |_| false);
        assert!(done);
        assert!(!asked.is_empty() && asked.iter().all(|&w| w == 0), "serial: {asked:?}");
        assert_eq!(ks, reference, "the serial restart");
    }

    /// **A dead worker** (gh:#817): worker 2 stops while it holds a chunk.
    /// It is marked dead, the run fails, the pool stays ready; a parallel
    /// restart uses the two survivors only and gives the reference `k`. A
    /// reply that could not be read is not a death. With no survivor, the
    /// pool says the data must be processed again.
    #[test]
    fn a_dead_worker_leaves_the_survivors_to_restart() {
        let b = Bench::new();
        let reference = b.reference();
        let mut p = b.ready_pool(3);
        p.start_run(b.cfg, 0.0).expect("start");
        let mut inflight = Inflight::new();
        b.drive(&mut p, &mut inflight, |ks| ks.len() == 1);
        b.send_due(&mut p, &mut inflight);
        // Worker 2's reply never comes: it died.
        inflight.retain(|(w, _)| *w != 1);
        p.on_error(1, "physics worker: out of memory".into());
        assert_eq!(p.phase, PoolPhase::Ready);
        assert_eq!(p.keff.alive, vec![true, false, true]);
        assert_eq!(p.live_workers(), 2);
        let why = p.keff.run.as_ref().and_then(|r| r.failed.clone()).expect("failed");
        assert!(
            why.contains("worker 2 of 3 stopped") && why.contains("2 workers still hold"),
            "{why}"
        );
        p.restart_run(RestartKind::Parallel, 1.0).expect("restart");
        assert!(p.keff.run.as_ref().is_some_and(|r| r.describe().contains("on 2 workers")));
        let (ks, asked, done) = b.drive(&mut p, &mut inflight, |_| false);
        assert!(done);
        assert!(!asked.contains(&1), "a dead worker was asked: {asked:?}");
        assert_eq!(ks, reference, "the restart on the survivors");
        // An unreadable reply fails a run, not the worker.
        p.start_run(b.cfg, 2.0).expect("start");
        let _ = p.outbox(1 << 20);
        p.on_error(0, format!("{BAD_MESSAGE}: garbled"));
        assert!(p.keff.alive[0]);
        assert!(p.keff.run.as_ref().is_some_and(|r| r.failed.is_some()));
        // No survivor: the data must be processed again.
        p.on_error(0, "physics worker panicked: x".into());
        assert_eq!(p.phase, PoolPhase::Ready);
        p.on_error(2, "physics worker panicked: y".into());
        match &p.phase {
            PoolPhase::Failed(m) => assert!(
                m.contains("No worker is left") && m.contains("processed again"),
                "{m}"
            ),
            other => panic!("{other:?}"),
        }
        assert!(p.restart_run(RestartKind::Serial, 3.0).is_err());
    }

    /// **Stop, and what a run keeps** (2026-10-15, for runs of up to 5000
    /// generations): every generation's note carries its traced tracks for
    /// the view, but the run keeps its generations WITHOUT them, so its
    /// memory does not grow by tracks per generation; the time-left line is
    /// there from the first generation; Stop ends the run at once without
    /// failing it, the replies in flight are discarded, its generations stay,
    /// and a new run may start.
    #[test]
    fn a_stopped_run_frees_the_pool_and_keeps_no_tracks() {
        let b = Bench::new();
        let mut p = b.ready_pool(2);
        p.start_run(b.cfg, 0.0).expect("start");
        let mut inflight = Inflight::new();
        let (mut noted, mut clock) = (Vec::new(), 0.0);
        for _ in 0..1000 {
            b.send_due(&mut p, &mut inflight);
            let Some((w, ev)) = inflight.pop_front() else {
                break;
            };
            clock += 1.0;
            for n in p.on_event(w, ev, clock) {
                if let Note::Generation(g) = n {
                    noted.push(g.gen.tracks.len());
                }
            }
            if noted.len() >= 2 {
                break;
            }
        }
        assert_eq!(noted.len(), 2);
        assert!(noted.iter().all(|&t| t > 0), "the view still gets the tracks: {noted:?}");
        let run = p.keff.run.as_ref().expect("run");
        assert_eq!(run.gens.len(), 2);
        assert!(run.gens.iter().all(|g| g.gen.tracks.is_empty()), "the run keeps no tracks");
        assert!(super::super::screen::run_eta(run).is_some_and(|l| l.contains("left on this device")));
        p.stop_run();
        let run = p.keff.run.as_ref().expect("run");
        assert!(!run.going() && run.stopped && run.failed.is_none());
        b.send_due(&mut p, &mut inflight);
        while let Some((w, ev)) = inflight.pop_front() {
            assert!(p.on_event(w, ev, 2.0).is_empty(), "a reply of the stopped run is discarded");
        }
        assert_eq!(p.keff.run.as_ref().map(|r| r.gens.len()), Some(2));
        assert!(p.restart_run(RestartKind::Parallel, 3.0).is_err(), "a stopped run is not a failed one");
        assert!(p.start_run(b.cfg, 3.0).is_ok());
    }

    /// **Measurement, not a gate: the PAGE's share of a 100 000-neutron
    /// generation.** The pool's coordinator runs on the page's thread: it
    /// cuts and encodes each generation's chunks (`outbox`), decodes every
    /// chunk result and reduces the generation (concatenate the banks,
    /// Shannon entropy, resample the next source; `on_event`). Times those
    /// calls (not the transport, which the scripted workers do) for 4
    /// workers × 3 chunks on the bench sphere at 100 000 neutrons, two
    /// generations, and prints the total and the longest single call: the
    /// longest is what one frame can be held by.
    ///
    /// Result, 2026-10-15, Intel Xeon @ 2.80 GHz (Cascade Lake, KVM, 4
    /// vCPUs, 15 GiB), native release, one thread: **78.6 ms for three
    /// generations (26.2 ms per generation), longest single call 16.9 ms**.
    /// Interpretation: about one frame per generation on this desktop; in
    /// the browser (wasm 1.3-2.5× slower, per the htr10_full_core_web
    /// record) and on a phone core (an assumed 2-4× slower, NOT measured)
    /// that is an estimated 45-170 ms hitch on the page once per generation,
    /// against minutes of transport per 100 000-neutron generation there.
    /// Below 100 000 neutrons it shrinks in proportion. Moving the
    /// coordinator into a worker would remove it; not done here.
    #[test]
    #[ignore = "timing measurement (prints); run with --ignored --nocapture"]
    fn page_side_cost_of_a_100k_generation() {
        let mut b = Bench::new();
        b.cfg.n_particles = 100_000;
        b.cfg.n_inactive = 1;
        b.cfg.n_active = 2;
        let mut p = b.ready_pool(4);
        p.start_run(b.cfg, 0.0).expect("start");
        let (mut page, mut longest, mut gens, mut clock) = (0.0_f64, 0.0_f64, 0, 0.0);
        let mut inflight = Inflight::new();
        for _ in 0..1000 {
            let t = std::time::Instant::now();
            let out = p.outbox(1 << 20);
            let dt = t.elapsed().as_secs_f64();
            (page, longest) = (page + dt, longest.max(dt));
            for (w, req) in out {
                if let Some(ev) = b.reply(req) {
                    inflight.push_back((w, ev));
                }
            }
            let Some((w, ev)) = inflight.pop_front() else {
                break;
            };
            clock += 1.0;
            let t = std::time::Instant::now();
            let notes = p.on_event(w, ev, clock);
            let dt = t.elapsed().as_secs_f64();
            (page, longest) = (page + dt, longest.max(dt));
            gens += notes.iter().filter(|n| matches!(n, Note::Generation(_))).count();
            if notes.iter().any(|n| matches!(n, Note::RunDone)) {
                break;
            }
        }
        assert_eq!(gens, 3);
        eprintln!(
            "  page side, 100 000 neutrons x {gens} generations on 4 workers x {CHUNKS_PER_WORKER} chunks: {:.1} ms in all ({:.1} ms per generation), longest single call {:.1} ms",
            1e3 * page,
            1e3 * page / gens as f64,
            1e3 * longest
        );
    }

    /// The pool's links start and stop: a terminated pool takes no work and
    /// delivers nothing.
    #[test]
    fn a_pool_starts_and_terminates() {
        let plan = crate::web_demo::pool::Plan {
            workers: 2,
            reason: "test".into(),
        };
        let pool = CorePool::start(&egui::Context::default(), plan, 12, &nee_soon::htr10_rmc::zones::ZoneTemperatures::record()).expect("start");
        assert_eq!(pool.links.len(), 2);
        assert_eq!(pool.state.data.labels.len(), 36);
        pool.terminate();
        assert!(pool.links.iter().all(|l| l.drain().is_empty()));
    }

    /// **gh:#1572: the real core's pool k on a fixed seed**, for comparing
    /// the pool before and after its protocol moved to the library: all 36
    /// HTR-10 jobs processed on two worker threads at the record's 300.15 K,
    /// the N = 12 lattice core, then 300 × [2 + 3] on the record seed. Prints
    /// every generation's k as bits (`HTR10_POOL_K`).
    #[test]
    #[ignore = "processes all 36 HTR-10 jobs on two threads and runs k (minutes); run with --ignored --nocapture"]
    fn the_real_core_pool_k_on_a_fixed_seed() {
        let plan = crate::web_demo::pool::Plan { workers: 2, reason: "test".into() };
        let mut pool = CorePool::start(&egui::Context::default(), plan, 12, &nee_soon::htr10_rmc::zones::ZoneTemperatures::record()).expect("start");
        let t0 = std::time::Instant::now();
        while pool.state.phase != PoolPhase::Ready {
            pool.pump();
            if let PoolPhase::Failed(m) = &pool.state.phase {
                panic!("{m}");
            }
            assert!(t0.elapsed().as_secs() < 3600, "not ready in an hour");
            std::thread::sleep(std::time::Duration::from_millis(20));
        }
        let cfg = KeffConfig { n_particles: 300, n_inactive: 2, n_active: 3, seed: super::super::RECORD_SEED, point_source: false, want_sites: false };
        pool.state.start_run(cfg, crate::web_demo::platform::now_s()).expect("start run");
        let mut ks = Vec::new();
        'run: loop {
            for n in pool.pump() {
                match n {
                    Note::Generation(g) => ks.push(g.gen.k),
                    Note::RunDone => break 'run,
                }
            }
            assert!(pool.state.keff_failed().is_none(), "{:?}", pool.state.keff_failed());
            std::thread::sleep(std::time::Duration::from_millis(5));
        }
        pool.terminate();
        println!("HTR10_POOL_K {:?}", ks.iter().map(|k| format!("{k:.17e}={:016x}", k.to_bits())).collect::<Vec<_>>());
        assert_eq!(ks.len(), 5);
    }
}
