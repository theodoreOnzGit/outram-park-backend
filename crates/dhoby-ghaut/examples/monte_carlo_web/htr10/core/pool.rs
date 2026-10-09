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

use super::{schedule, AssemblyReport, CoreEv, CoreReq};
use crate::keff::KeffConfig;
use dhoby_ghaut::web_demo::link::Floats;
use outram_mc_libs::physics::transport_csg::distributed::{
    ChunkResult, DistributedGeneration, DistributedPowerIteration, GenerationChunk, SourceSite,
};
use outram_mc_libs::tally::mesh::RegularMesh;
use std::collections::VecDeque;

/// At most this many `f64`s are relayed per frame (8 MB): one large product
/// (U-235's or U-238's, millions of words) still goes whole, but never two.
pub const RELAY_WORDS_PER_FRAME: usize = 1 << 20;
/// Chunks per worker per generation (more than one, so a worker that draws
/// short histories takes another chunk instead of idling).
pub const CHUNKS_PER_WORKER: usize = 3;

/// Where the pool is.
#[derive(Clone, Debug, PartialEq)]
pub enum PoolPhase {
    /// Processing and relaying the jobs.
    Processing,
    /// Every worker building its nuclides, core and majorant.
    Assembling,
    /// Every worker holds the model at `layers`.
    Ready,
    Failed(String),
}

/// One finished generation as the page shows it.
#[derive(Clone, Debug)]
pub struct GenSummary {
    pub gen: DistributedGeneration,
    /// Wall time of the generation, s, and the workers' summed busy time.
    pub wall_s: f64,
    pub busy_s: f64,
}

/// A run in progress.
pub struct RunState {
    pub cfg: KeffConfig,
    pub it: Option<DistributedPowerIteration>,
    mesh: Option<RegularMesh>,
    queue: VecDeque<GenerationChunk>,
    expected: usize,
    results: Vec<ChunkResult>,
    pub gens: Vec<GenSummary>,
    pub paused: bool,
    pub started_s: f64,
    gen_started_s: f64,
    gen_busy_s: f64,
    /// Waiting for the initial source.
    asked_source: bool,
}

impl RunState {
    pub fn finished(&self) -> bool {
        self.it
            .as_ref()
            .is_some_and(DistributedPowerIteration::finished)
    }
}

/// What the screen hears from the pool.
pub enum Note {
    Generation(GenSummary),
    /// The run's last generation is in.
    RunDone,
}

/// The pool's protocol (module docs).
pub struct PoolState {
    pub workers: usize,
    pub labels: Vec<String>,
    pub costs: Vec<f64>,
    pub assignment: Vec<Vec<usize>>,
    /// Per job: the worker doing it, whether it started, its seconds once done.
    pub owner: Vec<usize>,
    pub started: Vec<bool>,
    pub secs: Vec<Option<f64>>,
    /// Products awaiting relay, dropped once relayed to every other worker.
    products: Vec<Option<(Floats, usize)>>,
    relay: VecDeque<(usize, usize)>,
    process_sent: bool,
    assemble_sent: Vec<bool>,
    pub assembled: Vec<Option<AssemblyReport>>,
    pub layers: usize,
    pub phase: PoolPhase,
    pub started_s: f64,
    pub data_ready_s: Option<f64>,
    pub run: Option<RunState>,
    busy: Vec<bool>,
    /// Requests waiting to go out.
    out: Vec<(usize, CoreReq)>,
}

impl PoolState {
    /// A pool of `workers` for jobs with these labels and costs, the core
    /// built at `layers`.
    pub fn new(
        workers: usize,
        labels: Vec<String>,
        costs: Vec<f64>,
        layers: usize,
        now_s: f64,
    ) -> Self {
        let workers = workers.max(1);
        let assignment = schedule(&costs, workers);
        let mut owner = vec![0; costs.len()];
        for (w, js) in assignment.iter().enumerate() {
            for &j in js {
                owner[j] = w;
            }
        }
        let n = costs.len();
        Self {
            workers,
            labels,
            costs,
            assignment,
            owner,
            started: vec![false; n],
            secs: vec![None; n],
            products: (0..n).map(|_| None).collect(),
            relay: VecDeque::new(),
            process_sent: false,
            assemble_sent: vec![false; workers],
            assembled: vec![None; workers],
            layers,
            phase: PoolPhase::Processing,
            started_s: now_s,
            data_ready_s: None,
            run: None,
            busy: vec![false; workers],
            out: Vec::new(),
        }
    }

    /// Fraction of the data work done, by cost (assembly counts as the last 15 %).
    pub fn progress(&self) -> f64 {
        let total: f64 = self.costs.iter().sum::<f64>().max(1e-9);
        let done: f64 = self
            .costs
            .iter()
            .zip(&self.secs)
            .filter(|(_, s)| s.is_some())
            .map(|(c, _)| c)
            .sum();
        let built =
            self.assembled.iter().filter(|a| a.is_some()).count() as f64 / self.workers as f64;
        0.85 * done / total + 0.15 * built
    }

    /// A worker's event.
    pub fn on_event(&mut self, w: usize, ev: CoreEv, now_s: f64) -> Vec<Note> {
        let mut notes = Vec::new();
        match ev {
            CoreEv::JobStarted { job } => {
                if let Some(s) = self.started.get_mut(job) {
                    *s = true;
                }
            }
            CoreEv::Product { job, secs, data } => {
                if job < self.secs.len() {
                    self.secs[job] = Some(secs);
                    let others = (0..self.workers).filter(|&v| v != w).count();
                    for v in (0..self.workers).filter(|&v| v != w) {
                        self.relay.push_back((v, job));
                    }
                    self.products[job] = (others > 0).then_some((data, others));
                }
            }
            CoreEv::Assembled(r) => {
                if w < self.workers {
                    self.assembled[w] = Some(r);
                    if self
                        .assembled
                        .iter()
                        .all(|a| a.is_some_and(|a| a.layers == self.layers))
                    {
                        self.phase = PoolPhase::Ready;
                        self.data_ready_s.get_or_insert(now_s);
                    }
                }
            }
            CoreEv::Source { sites, seed, mesh } => {
                // The worker that sampled it is idle again.
                if let Some(b) = self.busy.get_mut(w) {
                    *b = false;
                }
                if let Some(run) = self.run.as_mut().filter(|r| r.it.is_none()) {
                    match SourceSite::decode(&sites.to_vec()) {
                        Ok(s) => {
                            run.it = Some(DistributedPowerIteration::from_initial_source(
                                &super::run_settings(&run.cfg),
                                &s,
                                seed,
                            ));
                            run.mesh = Some(super::mesh_from_words(&mesh));
                            run.gen_started_s = now_s;
                            Self::cut(run, self.workers);
                        }
                        Err(e) => {
                            self.phase = PoolPhase::Failed(unreadable("the initial source", w, &e));
                        }
                    }
                } else {
                    log::warn!("htr10 core: ignored an initial source from worker {} that no run is waiting for", w + 1);
                }
            }
            CoreEv::ChunkDone { data, secs } => {
                if let Some(b) = self.busy.get_mut(w) {
                    *b = false;
                }
                if let Some(run) = self.run.as_mut() {
                    match ChunkResult::from_f64s(&data.to_vec()) {
                        Ok(r) => {
                            run.results.push(r);
                            run.gen_busy_s += secs;
                        }
                        Err(e) => {
                            self.phase = PoolPhase::Failed(unreadable("a chunk result", w, &e));
                        }
                    }
                    if run.expected > 0 && run.results.len() == run.expected {
                        let got = run.results.len();
                        let results = std::mem::take(&mut run.results);
                        let mesh = run.mesh.clone();
                        if let Some(it) = run.it.as_mut() {
                            match it.finish_generation(results, mesh.as_ref()) {
                                Ok(gen) => {
                                    let s = GenSummary {
                                        gen,
                                        wall_s: now_s - run.gen_started_s,
                                        busy_s: run.gen_busy_s,
                                    };
                                    run.gens.push(s.clone());
                                    // Full precision in the console, so runs on
                                    // different pool sizes can be compared bit for bit.
                                    log::info!(
                                        "htr10 core: generation {} k = {:e} ({} workers)",
                                        s.gen.index,
                                        s.gen.k,
                                        self.workers
                                    );
                                    notes.push(Note::Generation(s));
                                    run.expected = 0;
                                    run.gen_busy_s = 0.0;
                                    run.gen_started_s = now_s;
                                    if it.finished() {
                                        notes.push(Note::RunDone);
                                    } else if !run.paused {
                                        Self::cut(run, self.workers);
                                    }
                                }
                                Err(e) => {
                                    self.phase = PoolPhase::Failed(not_added_up(
                                        it.generations_done(),
                                        &e,
                                        got,
                                        self.workers,
                                    ));
                                }
                            }
                        }
                    }
                }
            }
        }
        notes
    }

    /// A worker failed: the pool stops.
    pub fn on_error(&mut self, w: usize, msg: String) {
        self.phase = PoolPhase::Failed(format!(
            "Worker {} of {} stopped: {msg}. On a phone this is usually the browser reclaiming \
             memory (each worker holds about 545 MB of nuclear data). Close other tabs, or \
             open the page with ?workers=1, then reload the page.",
            w + 1,
            self.workers
        ));
    }

    /// Queue the current generation's chunks.
    fn cut(run: &mut RunState, workers: usize) {
        let Some(it) = run.it.as_ref() else { return };
        let chunks = it.chunks(workers * CHUNKS_PER_WORKER, super::TRACED_PER_GENERATION);
        run.expected = chunks.len();
        run.queue.extend(chunks);
    }

    /// Start a run (the pool must be ready, with no run going).
    pub fn start_run(&mut self, cfg: KeffConfig, now_s: f64) -> Result<(), String> {
        if self.phase != PoolPhase::Ready {
            return Err("the data are not ready".into());
        }
        // A run is going from the moment it starts, including while worker 1
        // samples its initial source (`it` is still None then). Accepting a
        // second start in that window sent two source requests, and each reply
        // queued generation 0's chunks: the duplicates came back while
        // generation 1 was being reduced (seen on a phone, 2026-10-09).
        if self.run.as_ref().is_some_and(|r| !r.finished()) {
            return Err("a run is already going (it may still be sampling its initial source); wait for it to finish".into());
        }
        self.run = Some(RunState {
            cfg,
            it: None,
            mesh: None,
            queue: VecDeque::new(),
            expected: 0,
            results: Vec::new(),
            gens: Vec::new(),
            paused: false,
            started_s: now_s,
            gen_started_s: now_s,
            gen_busy_s: 0.0,
            asked_source: false,
        });
        Ok(())
    }

    /// Pause or resume: a paused run finishes the generation in flight and
    /// cuts no more.
    pub fn set_paused(&mut self, paused: bool) {
        let workers = self.workers;
        if let Some(run) = self.run.as_mut() {
            let was = run.paused;
            run.paused = paused;
            if was && !paused && run.expected == 0 && !run.finished() {
                Self::cut(run, workers);
            }
        }
    }

    /// Rebuild the core at another layer count (only between runs).
    pub fn set_layers(&mut self, layers: usize) -> Result<(), String> {
        if self.phase != PoolPhase::Ready {
            return Err("the data are not ready".into());
        }
        if self
            .run
            .as_ref()
            .is_some_and(|r| !r.finished() && r.it.is_some())
        {
            return Err("a run is going".into());
        }
        if layers != self.layers {
            self.layers = layers;
            self.phase = PoolPhase::Assembling;
            self.run = None;
            for w in 0..self.workers {
                self.out.push((w, CoreReq::Layers { layers }));
            }
        }
        Ok(())
    }

    /// The requests to send now: the first time, each worker's jobs; then
    /// relays (at most `word_budget` words, but always at least one), each
    /// worker's assembly once everything has reached it, and chunks to idle
    /// workers.
    pub fn outbox(&mut self, word_budget: usize) -> Vec<(usize, CoreReq)> {
        let mut out = std::mem::take(&mut self.out);
        if matches!(self.phase, PoolPhase::Failed(_)) {
            return out;
        }
        if !self.process_sent {
            self.process_sent = true;
            for (w, js) in self.assignment.iter().enumerate() {
                out.push((w, CoreReq::Process { jobs: js.clone() }));
            }
        }
        let mut words = 0usize;
        while let Some(&(v, job)) = self.relay.front() {
            let Some((data, _)) = self.products[job].as_ref() else {
                self.relay.pop_front();
                continue;
            };
            if words > 0 && words + data.len() > word_budget {
                break;
            }
            words += data.len();
            out.push((
                v,
                CoreReq::Product {
                    job,
                    data: data.clone(),
                },
            ));
            self.relay.pop_front();
            if let Some((_, left)) = self.products[job].as_mut() {
                *left -= 1;
                if *left == 0 {
                    self.products[job] = None;
                }
            }
        }
        if self.phase == PoolPhase::Processing && self.secs.iter().all(Option::is_some) {
            for v in 0..self.workers {
                if !self.assemble_sent[v] && !self.relay.iter().any(|(w, _)| *w == v) {
                    self.assemble_sent[v] = true;
                    out.push((
                        v,
                        CoreReq::Assemble {
                            layers: self.layers,
                        },
                    ));
                }
            }
            if self.assemble_sent.iter().all(|&s| s) {
                self.phase = PoolPhase::Assembling;
            }
        }
        if self.phase == PoolPhase::Ready {
            if let Some(run) = self.run.as_mut() {
                if !run.asked_source {
                    run.asked_source = true;
                    self.busy[0] = true;
                    out.push((0, CoreReq::Source { cfg: run.cfg }));
                } else {
                    for w in 0..self.workers {
                        if !self.busy[w] {
                            let Some(c) = run.queue.pop_front() else {
                                break;
                            };
                            self.busy[w] = true;
                            out.push((
                                w,
                                CoreReq::Chunk {
                                    cfg: run.cfg,
                                    data: Floats::from_vec(c.to_f64s()),
                                },
                            ));
                        }
                    }
                }
            }
        }
        out
    }
}

/// Where a failure the reader cannot fix should be reported.
const REPORT: &str = "github.com/theodoreOnzGit/outram-park-backend/issues";

/// The message when a generation's results cannot be added up: what
/// happened, that it is the demo's bookkeeping and not the physics, and what
/// to do (maintainer, 2026-10-09: errors must be informative in the browser).
fn not_added_up(generation: usize, e: &str, got: usize, workers: usize) -> String {
    format!(
        "Run stopped at generation {generation}: the {got} results the {workers} workers sent \
         back could not be added up ({e}). This is a bug in the demo's bookkeeping, not in \
         the physics. Reload the page to start again, and please report this message at {REPORT}."
    )
}

/// The message when a worker's reply cannot be decoded.
fn unreadable(what: &str, w: usize, e: &str) -> String {
    format!(
        "Run stopped: {what} from worker {} could not be read ({e}). Reload the page to start \
         again, and please report this message at {REPORT}.",
        w + 1
    )
}

// ─── Wired to links ──────────────────────────────────────────────────────────

/// The pool with its links.
pub struct CorePool {
    pub links: Vec<dhoby_ghaut::web_demo::link::Link<crate::engine::Request, crate::engine::Event>>,
    pub state: PoolState,
    pub plan: dhoby_ghaut::web_demo::pool::Plan,
}

impl CorePool {
    /// Start the workers (Web Workers in the browser, threads natively).
    pub fn start(
        ctx: &egui::Context,
        plan: dhoby_ghaut::web_demo::pool::Plan,
        layers: usize,
    ) -> Result<Self, String> {
        let jobs = super::jobs()?;
        let labels = jobs.iter().map(|j| j.label()).collect();
        let costs = jobs.iter().map(super::job_cost).collect();
        let mut links = Vec::new();
        for _ in 0..plan.workers {
            #[cfg(target_arch = "wasm32")]
            links.push(dhoby_ghaut::web_demo::link::start_web::<
                crate::engine::Request,
                crate::engine::Event,
            >(
                ctx.clone(), "./worker.js", crate::engine::Event::Error
            )?);
            #[cfg(not(target_arch = "wasm32"))]
            {
                let c = ctx.clone();
                links.push(dhoby_ghaut::web_demo::link::start_native(
                    crate::engine::McEngine::default(),
                    move || c.request_repaint(),
                ));
            }
        }
        let state = PoolState::new(
            plan.workers,
            labels,
            costs,
            layers,
            dhoby_ghaut::web_demo::platform::now_s(),
        );
        Ok(Self { links, state, plan })
    }

    /// Each frame: take every worker's events, send what is due.
    pub fn pump(&mut self) -> Vec<Note> {
        let now = dhoby_ghaut::web_demo::platform::now_s();
        let mut notes = Vec::new();
        for (w, link) in self.links.iter().enumerate() {
            for e in link.drain() {
                match e {
                    crate::engine::Event::Core(ev) => notes.extend(self.state.on_event(w, ev, now)),
                    crate::engine::Event::Error(m) => self.state.on_error(w, m),
                    _ => {}
                }
            }
        }
        for (w, req) in self.state.outbox(RELAY_WORDS_PER_FRAME) {
            if let Some(l) = self.links.get(w) {
                l.send(crate::engine::Request::Core(req));
            }
        }
        notes
    }
}

#[cfg(all(test, not(target_arch = "wasm32")))]
mod tests {
    use super::*;
    use outram_mc_libs::geometry::cell::{Cell, HalfSpaceSense, RegionToken};
    use outram_mc_libs::geometry::geometry::Geometry;
    use outram_mc_libs::geometry::position::Position;
    use outram_mc_libs::geometry::surface::{BoundaryType, Sphere, SurfaceKind};
    use outram_mc_libs::geometry::universe::Universe;
    use outram_mc_libs::material::material::{Material, NuclideComponent};
    use outram_mc_libs::material::nuclide::Nuclide;
    use outram_mc_libs::physics::transport_csg::distributed::transport_chunk;
    use outram_mc_libs::physics::transport_csg::SourceBox;

    /// Scripted workers through the data phase: every worker gets its own
    /// jobs once, every product reaches every other worker exactly once and
    /// never back to its maker, and a worker's assembly is sent only after
    /// every product has been sent to it.
    #[test]
    fn every_product_reaches_every_other_worker_before_its_assembly() {
        let costs = vec![5.0, 1.0, 3.0, 2.0, 2.0];
        let labels = (0..5).map(|i| format!("job {i}")).collect();
        let mut p = PoolState::new(3, labels, costs, 12, 0.0);
        let first = p.outbox(10);
        let processed: Vec<(usize, Vec<usize>)> = first
            .iter()
            .filter_map(|(w, r)| match r {
                CoreReq::Process { jobs } => Some((*w, jobs.clone())),
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
                        assert_ne!(p.owner[job], v, "a product went back to its maker");
                        assert_eq!(data.to_vec(), vec![job as f64; 4]);
                        assert!(!data.is_empty());
                        assert!(assembled_after[v].is_none(), "a product after the assembly");
                        got[v].push(job);
                    }
                    CoreReq::Assemble { layers } => {
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
            let r = AssemblyReport {
                layers: 12,
                tapes_s: 0.0,
                nuclides_s: 0.0,
                geometry_s: 0.0,
                majorant_s: 0.0,
                memory_mb: 1.0,
            };
            p.on_event(v, CoreEv::Assembled(r), 2.0);
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
        p.on_error(1, "boom".into());
        match &p.phase {
            // Informative in the browser: which worker, why it is likely, what to do.
            PoolPhase::Failed(m) => {
                assert!(m.contains("Worker 2 of 3 stopped: boom") && m.contains("reload"), "{m}");
            }
            other => panic!("{other:?}"),
        }
        let m = not_added_up(1, "a result for generation 0 while reducing 1", 6, 2);
        assert!(m.contains("generation 1") && m.contains("not in the physics") && m.contains(REPORT), "{m}");
        assert!(unreadable("a chunk result", 0, "short").contains("worker 1"));
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

    /// A run through the pool with scripted workers (the transport is the
    /// real `transport_chunk` on a small sphere) gives, for 1 and for 3
    /// workers, the same `k` in every generation bit for bit as one
    /// `DistributedPowerIteration` cut in one chunk: the reduction does not
    /// depend on the pool.
    #[test]
    fn the_pool_answer_does_not_depend_on_the_worker_count() {
        let (geom, mats, nucs) = model();
        let cfg = KeffConfig {
            n_particles: 200,
            n_inactive: 2,
            n_active: 3,
            seed: 77,
            point_source: false,
            want_sites: false,
        };
        let box_ = SourceBox {
            lower: Position::new(-8.0, -8.0, -8.0),
            upper: Position::new(8.0, 8.0, 8.0),
        };
        let mesh = [-9.0, -9.0, -9.0, 9.0, 9.0, 9.0, 2.0, 2.0, 2.0];
        let settings = super::super::run_settings(&cfg);
        // The reference: one coordinator, one chunk per generation.
        let reference = {
            let mut it = DistributedPowerIteration::new(&geom, &mats, &nucs, box_, &settings);
            while !it.finished() {
                let r: Vec<_> = it
                    .chunks(1, 0)
                    .iter()
                    .map(|c| transport_chunk(&geom, &mats, &nucs, &[], &settings, c))
                    .collect();
                it.finish_generation(r, Some(&super::super::mesh_from_words(&mesh)))
                    .expect("reduce");
            }
            it.k_by_generation()
                .iter()
                .map(|k| k.to_bits())
                .collect::<Vec<_>>()
        };
        for workers in [1, 3] {
            let mut p = PoolState::new(workers, vec!["only".into()], vec![1.0], 12, 0.0);
            p.on_event(
                0,
                CoreEv::Product {
                    job: 0,
                    secs: 0.0,
                    data: Floats::from_vec(vec![1.0]),
                },
                0.0,
            );
            let _ = p.outbox(1 << 20);
            for v in 0..workers {
                p.on_event(
                    v,
                    CoreEv::Assembled(AssemblyReport {
                        layers: 12,
                        tapes_s: 0.0,
                        nuclides_s: 0.0,
                        geometry_s: 0.0,
                        majorant_s: 0.0,
                        memory_mb: 0.0,
                    }),
                    0.0,
                );
            }
            assert_eq!(p.phase, PoolPhase::Ready);
            p.start_run(cfg, 0.0).expect("start");
            // Regression (phone, 2026-10-09): a second Start while the
            // initial source is still being sampled is refused.
            let again = p.start_run(cfg, 0.5).unwrap_err();
            assert!(again.contains("already going"), "{again}");
            let mut ks = Vec::new();
            let mut done = false;
            for _ in 0..1000 {
                for (w, req) in p.outbox(1 << 20) {
                    let ev = match req {
                        CoreReq::Source { cfg } => {
                            let (s, seed) = DistributedPowerIteration::initial_source(
                                &geom,
                                &mats,
                                &nucs,
                                box_,
                                &super::super::run_settings(&cfg),
                            );
                            // Delivered twice: the duplicate must be ignored,
                            // not queue generation 0 a second time.
                            let dup = CoreEv::Source {
                                sites: Floats::from_vec(SourceSite::encode(&s)),
                                seed,
                                mesh,
                            };
                            assert!(p.on_event(w, dup, 1.0).is_empty());
                            CoreEv::Source {
                                sites: Floats::from_vec(SourceSite::encode(&s)),
                                seed,
                                mesh,
                            }
                        }
                        CoreReq::Chunk { cfg, data } => {
                            let c = GenerationChunk::from_f64s(&data.to_vec()).expect("chunk");
                            let r = transport_chunk(
                                &geom,
                                &mats,
                                &nucs,
                                &[],
                                &super::super::run_settings(&cfg),
                                &c,
                            );
                            CoreEv::ChunkDone {
                                data: Floats::from_vec(r.to_f64s()),
                                secs: 0.0,
                            }
                        }
                        _ => continue,
                    };
                    for n in p.on_event(w, ev, 1.0) {
                        match n {
                            Note::Generation(g) => ks.push(g.gen.k.to_bits()),
                            Note::RunDone => done = true,
                        }
                    }
                }
                if done {
                    break;
                }
            }
            assert!(done, "{workers} workers: the run did not finish");
            assert_eq!(ks, reference, "{workers} workers");
            assert!(p.run.as_ref().is_some_and(RunState::finished));
            // Pausing a finished run does nothing; a new run can start.
            p.set_paused(true);
            p.set_paused(false);
            assert!(p.start_run(cfg, 2.0).is_ok());
        }
    }
}
