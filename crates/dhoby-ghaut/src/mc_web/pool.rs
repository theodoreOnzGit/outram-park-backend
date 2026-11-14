//! **The page side of a Monte Carlo worker pool** (gh:#1572; promoted from
//! the `monte_carlo_web` example's HTR-10 whole core, `htr10/core/pool.rs`,
//! gh:#786, #817, #905, #907): two state machines with no I/O, which a page
//! drives with its workers' events and whose requests it sends.
//!
//! - [`DataRelay`]: deal per-nuclide processing jobs to the workers (longest
//!   first, [`schedule`]), relay each finished product to every other worker
//!   (a [`Floats`] the page never reads, at most [`RELAY_WORDS_PER_FRAME`]
//!   words a frame), then tell each worker to build its model once every
//!   product has reached it.
//! - [`KeffPool`]: a k-eigenvalue run on workers that each hold the model:
//!   one worker samples the initial source, each generation's histories go
//!   out in chunks to idle workers, and the page reduces them in history
//!   order with `transport_csg::distributed`'s coordinator. The answer is
//!   `run_keff_csg_par`'s, bit for bit, for any number of workers (the
//!   example's pool tests pin it).
//!
//! **A run can fail without the pool failing** (gh:#817, maintainer
//! 2026-10-09: "Give an option to restart serially or in parallel if race
//! conditions or other errors occur"; "I don't want to recompute nuclear
//! data and start again"). Three kinds of failure are kept apart:
//!
//! - **a run failure** (a reply that cannot be read, a generation that cannot
//!   be added up, a chunk a worker could not transport): recorded on the run
//!   ([`RunState::failed`]); every worker keeps its data, and
//!   [`KeffPool::restart_run`] starts the run again, in parallel or on one
//!   worker ([`RestartKind`]);
//! - **a dead worker** (its `onerror`, or a panic: its data are gone): marked
//!   dead ([`KeffPool::alive`]); the run it was serving fails, and the
//!   survivors carry on;
//! - **a pool failure**: while the data are still being processed or
//!   assembled, or with no worker left. [`KeffPool::on_error`] returns the
//!   message; the caller marks its pool failed. The data must be reprocessed.
//!
//! **Stale replies.** Every run has an epoch, carried by each source and
//! chunk request and echoed by its reply. A restarted run has a new epoch,
//! so a reply still in flight from the replaced run is recognised, discarded
//! and logged, never added to the new run. Each busy worker has exactly one
//! request outstanding, and a restart leaves it marked busy until that reply
//! (stale) arrives, so the new run never gives it a second request meanwhile.
//!
//! **What a model adds.** A run may carry the model's own parameters (`T`:
//! the HTR-10 core's bed and rods; `()` for none), set by the caller when it
//! starts the run, so what a finished run's `k` is compared with follows the
//! model the run was on, not the pool's present one.
//!
//! **The worker side** of a CSG model is [`csg_source`] and [`csg_chunk`]:
//! surface tracking on the model a worker holds.
//!
//! Moved, not rewritten: the code is the example's, with the HTR-10 bed,
//! rods, zone temperatures and pour left in the example's `PoolState`, which
//! wraps these two machines. Research, education and V&V only.

use super::keff::KeffConfig;
use crate::web_demo::data_cache::Source;
use crate::web_demo::link::{Floats, BAD_MESSAGE};
use outram_mc_libs::geometry::geometry::Geometry;
use outram_mc_libs::material::material::Material;
use outram_mc_libs::material::nuclide::Nuclide;
use outram_mc_libs::physics::keff::KeffSettings;
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
/// The epoch of a failure that belongs to no run.
pub const NO_RUN: u32 = 0;

/// Deal jobs (by `costs`) to `workers`: longest first, each to the least
/// loaded worker so far (LPT). Ties go to the lower worker index.
pub fn schedule(costs: &[f64], workers: usize) -> Vec<Vec<usize>> {
    let workers = workers.max(1);
    let mut order: Vec<usize> = (0..costs.len()).collect();
    order.sort_by(|&a, &b| costs[b].total_cmp(&costs[a]).then(a.cmp(&b)));
    let mut load = vec![0.0_f64; workers];
    let mut out = vec![Vec::new(); workers];
    for j in order {
        let w = (0..workers)
            .min_by(|&a, &b| load[a].total_cmp(&load[b]).then(a.cmp(&b)))
            .unwrap_or(0);
        load[w] += costs[j];
        out[w].push(j);
    }
    out
}

/// An entropy mesh as a source reply's words (`[lower×3, upper×3, dims×3]`);
/// [`mesh_from_words`] reads them back.
pub fn mesh_words(mesh: &RegularMesh) -> [f64; 9] {
    let mut w = [0.0; 9];
    w[..3].copy_from_slice(&mesh.lower_left);
    w[3..6].copy_from_slice(&mesh.upper_right);
    for k in 0..3 {
        w[6 + k] = mesh.dimension[k] as f64;
    }
    w
}

/// The entropy mesh from a source reply's words.
pub fn mesh_from_words(w: &[f64; 9]) -> RegularMesh {
    RegularMesh {
        lower_left: [w[0], w[1], w[2]],
        upper_right: [w[3], w[4], w[5]],
        dimension: [w[6] as usize, w[7] as usize, w[8] as usize],
    }
}

/// Thin a result's tracks to what is drawn: surface crossings removed (the
/// flight is straight between collisions), the rest kept.
pub fn thin_tracks(r: &mut ChunkResult) {
    use outram_mc_libs::physics::track_output::TrackEvent;
    for (_, t) in &mut r.tracks {
        t.states.retain(|s| s.event != TrackEvent::SurfaceCrossing);
    }
}

// ─── The worker side of a CSG model ─────────────────────────────────────────

/// A pooled run's initial source, as a worker samples it.
pub struct PoolSource {
    pub sites: Vec<SourceSite>,
    /// The sequential source stream's state after sampling them.
    pub seed: u64,
    pub mesh: RegularMesh,
}

/// A pool member's answer to "sample the initial source" on a CSG model:
/// `n_particles` sites in `start`, the entropy mesh on `entropy`.
pub fn csg_source(
    phys: (&Geometry, &[Material], &[Nuclide]),
    start: outram_mc_libs::physics::transport_csg::SourceBox,
    entropy: RegularMesh,
    settings: &KeffSettings,
) -> PoolSource {
    let (sites, seed) = DistributedPowerIteration::initial_source(phys.0, phys.1, phys.2, start, settings);
    PoolSource { sites, seed, mesh: entropy }
}

/// A pool member's answer to "transport this chunk" on a CSG model: surface
/// tracking (no majorants), its tracks thinned to what is drawn
/// ([`thin_tracks`]).
pub fn csg_chunk(phys: (&Geometry, &[Material], &[Nuclide]), settings: &KeffSettings, chunk: &[f64]) -> Result<Vec<f64>, String> {
    use outram_mc_libs::physics::transport_csg::distributed::transport_chunk;
    let c = GenerationChunk::from_f64s(chunk)?;
    let mut r = transport_chunk(phys.0, phys.1, phys.2, &[], settings, &c);
    thin_tracks(&mut r);
    Ok(r.to_f64s())
}

// ─── Data: process, relay, assemble ─────────────────────────────────────────

/// What [`DataRelay::outbox`] asks the caller to send.
pub enum DataOut {
    /// Worker `worker`: process these jobs.
    Process { worker: usize, jobs: Vec<usize> },
    /// Worker `worker`: take job `job`'s product, made by another worker.
    Relay { worker: usize, job: usize, data: Floats },
    /// Worker `worker` holds every product: build the model.
    Assemble { worker: usize },
}

/// The data half of a pool (module docs).
pub struct DataRelay {
    pub workers: usize,
    pub labels: Vec<String>,
    pub costs: Vec<f64>,
    pub assignment: Vec<Vec<usize>>,
    /// Per job: the worker doing it, whether it started, its seconds once done.
    pub owner: Vec<usize>,
    pub started: Vec<bool>,
    pub secs: Vec<Option<f64>>,
    /// Per job: processed now, or read from the browser's cache (gh:#818).
    pub sources: Vec<Option<Source>>,
    /// Products awaiting relay, dropped once relayed to every other worker.
    products: Vec<Option<(Floats, usize)>>,
    relay: VecDeque<(usize, usize)>,
    process_sent: bool,
    assemble_sent: Vec<bool>,
}

impl DataRelay {
    /// `workers` workers for jobs with these labels and costs.
    pub fn new(workers: usize, labels: Vec<String>, costs: Vec<f64>) -> Self {
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
            sources: vec![None; n],
            products: (0..n).map(|_| None).collect(),
            relay: VecDeque::new(),
            process_sent: false,
            assemble_sent: vec![false; workers],
        }
    }

    /// Workers that already hold their model: nothing to process or relay.
    pub fn none(workers: usize) -> Self {
        let mut d = Self::new(workers, Vec::new(), Vec::new());
        d.process_sent = true;
        d
    }

    /// Where the data came from, one line (gh:#818: Leak Before Break, the
    /// page shows it): how many products were read from this browser's
    /// cache and how many processed now.
    pub fn data_source(&self) -> String {
        Source::summarize(self.sources.iter().flatten())
    }

    /// Fraction of the processing done, by cost.
    pub fn processed(&self) -> f64 {
        let total: f64 = self.costs.iter().sum::<f64>().max(1e-9);
        let done: f64 = self.costs.iter().zip(&self.secs).filter(|(_, s)| s.is_some()).map(|(c, _)| c).sum();
        done / total
    }

    /// Every job's product is in.
    pub fn all_processed(&self) -> bool {
        self.secs.iter().all(Option::is_some)
    }

    /// A worker started job `job`.
    pub fn on_started(&mut self, job: usize) {
        if let Some(s) = self.started.get_mut(job) {
            *s = true;
        }
    }

    /// Worker `w` finished job `job`: queue its product for every other worker.
    pub fn on_product(&mut self, w: usize, job: usize, secs: f64, data: Floats, source: Source) {
        if job < self.secs.len() {
            self.secs[job] = Some(secs);
            log::info!(
                "worker pool: {} {} ({secs:.1} s, worker {})",
                self.labels.get(job).map_or("?", String::as_str),
                source.describe(),
                w + 1
            );
            self.sources[job] = Some(source);
            let others: Vec<usize> = (0..self.workers).filter(|&v| v != w).collect();
            for &v in &others {
                self.relay.push_back((v, job));
            }
            self.products[job] = (!others.is_empty()).then_some((data, others.len()));
        }
    }

    /// The requests to send now: the first time, each worker's jobs; then
    /// relays (at most `word_budget` words, but always at least one); and,
    /// while `processing` and once every product is in, each worker's
    /// assembly once everything has reached it. The flag is `true` once
    /// every worker has been told to assemble (the caller's pool is then
    /// assembling).
    pub fn outbox(&mut self, word_budget: usize, processing: bool) -> (Vec<DataOut>, bool) {
        let mut out = Vec::new();
        if !self.process_sent {
            self.process_sent = true;
            for (w, js) in self.assignment.iter().enumerate() {
                out.push(DataOut::Process { worker: w, jobs: js.clone() });
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
            out.push(DataOut::Relay { worker: v, job, data: data.clone() });
            self.relay.pop_front();
            if let Some((_, left)) = self.products[job].as_mut() {
                *left -= 1;
                if *left == 0 {
                    self.products[job] = None;
                }
            }
        }
        let mut all_sent = false;
        if processing && self.all_processed() {
            for v in 0..self.workers {
                if !self.assemble_sent[v] && !self.relay.iter().any(|(w, _)| *w == v) {
                    self.assemble_sent[v] = true;
                    out.push(DataOut::Assemble { worker: v });
                }
            }
            all_sent = self.assemble_sent.iter().all(|&s| s);
        }
        (out, all_sent)
    }
}

// ─── The run: k_eff on the workers ──────────────────────────────────────────

/// One finished generation as the page shows it.
#[derive(Clone, Debug)]
pub struct GenSummary {
    pub gen: DistributedGeneration,
    /// Wall time of the generation, s, and the workers' summed busy time.
    pub wall_s: f64,
    pub busy_s: f64,
}

impl GenSummary {
    /// The same generation with no traced tracks: what a run keeps per
    /// generation (a few hundred bytes, against the tracks' tens to
    /// hundreds of kB).
    pub fn without_tracks(&self) -> Self {
        let g = &self.gen;
        Self {
            gen: DistributedGeneration {
                index: g.index,
                active: g.active,
                k: g.k,
                k_mean: g.k_mean,
                entropy: g.entropy,
                bank_size: g.bank_size,
                counts: g.counts,
                tracks: Vec::new(),
            },
            wall_s: self.wall_s,
            busy_s: self.busy_s,
        }
    }
}

/// How a failed run is started again (gh:#817). Both give the same `k` in
/// every generation, bit for bit: the reduction does not depend on how the
/// histories were dealt out.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RestartKind {
    /// On every live worker.
    Parallel,
    /// Every chunk to one worker (the first live one): slower, and nothing
    /// is ever in flight on two workers at once.
    Serial,
}

/// Where a run came from: what its summary records.
#[derive(Clone, Debug, PartialEq)]
pub enum RunOrigin {
    /// Started by the reader (or `?autostart`).
    Fresh,
    /// A restart of a failed run: how, on how many workers, and the failure
    /// it replaced.
    Restart { kind: RestartKind, workers: usize, after: String },
}

/// A run in progress. `T`: the model parameters it started on (module docs).
pub struct RunState<T> {
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
    /// The run's epoch: requests carry it and replies echo it.
    pub epoch: u32,
    /// Why the run stopped, if it failed. The pool is still ready.
    pub failed: Option<String>,
    pub origin: RunOrigin,
    /// The one worker every request goes to, for a serial run.
    pub serial: Option<usize>,
    /// Stopped by the reader: not going, not failed.
    pub stopped: bool,
    /// The model the run started on.
    pub model: T,
}

impl<T> RunState<T> {
    /// Bins of the run's entropy mesh, once the source has set it: the
    /// entropy's ceiling is `log2` of this.
    pub fn entropy_bins(&self) -> Option<usize> {
        self.mesh.as_ref().map(|m| m.dimension.iter().product())
    }
    fn new(cfg: KeffConfig, now_s: f64, epoch: u32, origin: RunOrigin, serial: Option<usize>, model: T) -> Self {
        Self {
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
            epoch,
            failed: None,
            origin,
            serial,
            stopped: false,
            model,
        }
    }

    pub fn finished(&self) -> bool {
        self.it.as_ref().is_some_and(DistributedPowerIteration::finished)
    }

    /// Going: started, neither finished, failed nor stopped (including while
    /// the initial source is sampled, a8b9ad7833).
    pub fn going(&self) -> bool {
        !self.finished() && self.failed.is_none() && !self.stopped
    }

    /// What kind of run this is, for its summary and the console.
    pub fn describe(&self) -> String {
        match &self.origin {
            RunOrigin::Fresh => format!("run {}", self.epoch),
            RunOrigin::Restart { kind: RestartKind::Parallel, workers, .. } => {
                format!("run {}, a restart in parallel on {workers} workers", self.epoch)
            }
            RunOrigin::Restart { kind: RestartKind::Serial, .. } => {
                format!("run {}, a serial restart on 1 worker", self.epoch)
            }
        }
    }

    /// Stop the run: it cuts no more chunks; replies still in flight for it
    /// are discarded when they arrive (it is not going).
    fn fail(&mut self, why: String) {
        log::error!("worker pool: {} failed: {why}", self.describe());
        self.failed = Some(why);
        self.queue.clear();
        self.results.clear();
        self.expected = 0;
    }
}

/// What the screen hears from the pool.
pub enum Note {
    Generation(GenSummary),
    /// The run's last generation is in.
    RunDone,
}

/// What [`KeffPool::outbox`] asks the caller to send.
pub enum RunReq {
    /// Sample the initial source.
    Source { cfg: KeffConfig, epoch: u32 },
    /// Transport this chunk (`GenerationChunk::to_f64s`).
    Chunk { cfg: KeffConfig, data: Floats, epoch: u32 },
}

/// The run half of a pool (module docs).
pub struct KeffPool<T> {
    pub workers: usize,
    busy: Vec<bool>,
    /// Per worker: alive, or why it died (gh:#817).
    pub alive: Vec<bool>,
    pub died: Vec<Option<String>>,
    /// The last run epoch handed out (0: none yet).
    epoch: u32,
    /// Stale replies discarded so far (each also in the console).
    pub stale_discarded: usize,
    /// What the pool did that the reader should be able to see: restarts,
    /// discarded replies, dead workers (also in the console). Newest last.
    pub log: Vec<String>,
    pub run: Option<RunState<T>>,
    /// What each worker holds, for the message when one stops ("about 545
    /// MB of nuclear data" for the HTR-10 core).
    pub per_worker: String,
    /// The transport settings of a run's coordinator (the workers' own).
    settings: fn(&KeffConfig) -> KeffSettings,
    /// Histories traced (every state kept) at the start of each generation.
    traced_per_generation: usize,
}

impl<T> KeffPool<T> {
    /// `workers` workers, the coordinator's transport settings from
    /// `settings`, `traced` histories traced per generation.
    pub fn new(workers: usize, per_worker: String, settings: fn(&KeffConfig) -> KeffSettings, traced: usize) -> Self {
        let workers = workers.max(1);
        Self {
            workers,
            busy: vec![false; workers],
            alive: vec![true; workers],
            died: vec![None; workers],
            epoch: 0,
            stale_discarded: 0,
            log: Vec::new(),
            run: None,
            per_worker,
            settings,
            traced_per_generation: traced,
        }
    }

    /// Workers still alive.
    pub fn live_workers(&self) -> usize {
        self.alive.iter().filter(|&&a| a).count()
    }

    /// Worker `w` has a request outstanding.
    pub fn is_busy(&self, w: usize) -> bool {
        self.busy.get(w).copied().unwrap_or(false)
    }

    /// Worker `w` has no request outstanding any more (a failure that is
    /// not a run's, such as while the data are built).
    pub fn idle(&mut self, w: usize) {
        if let Some(b) = self.busy.get_mut(w) {
            *b = false;
        }
    }

    /// A run is going (module docs).
    pub fn going(&self) -> bool {
        self.run.as_ref().is_some_and(RunState::going)
    }

    /// Record something the reader should see, in the panel and the console.
    pub fn note(&mut self, line: String) {
        log::warn!("worker pool: {line}");
        self.log.push(line);
        if self.log.len() > 40 {
            self.log.remove(0);
        }
    }

    /// Discard a reply no going run is waiting for, visibly.
    fn discard(&mut self, w: usize, what: &str, epoch: u32) {
        self.stale_discarded += 1;
        let now = self.run.as_ref().map_or(0, |r| r.epoch);
        self.note(format!(
            "discarded {what} from worker {} for run {epoch} (current run {now}): no run is \
             waiting for it (it was in flight when its run stopped, or is a duplicate)",
            w + 1
        ));
    }

    /// The run a reply tagged `epoch` belongs to, if it is still going;
    /// otherwise the reply is stale: discarded and logged.
    fn current_run(&mut self, epoch: u32, w: usize, what: &str) -> Option<&mut RunState<T>> {
        if !self.run.as_ref().is_some_and(|r| r.epoch == epoch && r.going()) {
            self.discard(w, what, epoch);
            return None;
        }
        self.run.as_mut()
    }

    /// Worker `w`'s initial source for run `epoch`: `sites`
    /// (`SourceSite::encode`), the stream state `seed`, the entropy mesh.
    pub fn on_source(&mut self, w: usize, sites: &Floats, seed: u64, mesh: &[f64; 9], epoch: u32, now_s: f64) {
        if w >= self.workers {
            return;
        }
        // The worker that sampled it is idle again.
        self.busy[w] = false;
        let waiting = self.run.as_ref().is_some_and(|r| r.epoch == epoch && r.going() && r.it.is_none());
        let live = self.live_workers();
        if !waiting {
            // A duplicate (a8b9ad7833) or one for a replaced run.
            self.discard(w, "an initial source", epoch);
        } else if let Some(run) = self.run.as_mut() {
            match SourceSite::decode(&sites.to_vec()) {
                Ok(s) => {
                    run.it = Some(DistributedPowerIteration::from_initial_source(&(self.settings)(&run.cfg), &s, seed));
                    run.mesh = Some(mesh_from_words(mesh));
                    run.gen_started_s = now_s;
                    let n = Self::dealing(run, live);
                    Self::cut(run, n, self.traced_per_generation);
                }
                Err(e) => run.fail(unreadable("the initial source", w, &e)),
            }
        }
    }

    /// Worker `w`'s chunk result for run `epoch`, `secs` of its time.
    pub fn on_chunk_done(&mut self, w: usize, data: &Floats, secs: f64, epoch: u32, now_s: f64) -> Vec<Note> {
        let mut notes = Vec::new();
        if w >= self.workers {
            return notes;
        }
        self.busy[w] = false;
        let workers = self.live_workers();
        let traced = self.traced_per_generation;
        let Some(run) = self.current_run(epoch, w, "a chunk result") else {
            return notes;
        };
        match ChunkResult::from_f64s(&data.to_vec()) {
            Ok(r) => {
                run.results.push(r);
                run.gen_busy_s += secs;
            }
            Err(e) => {
                run.fail(unreadable("a chunk result", w, &e));
                return notes;
            }
        }
        if run.expected > 0 && run.results.len() == run.expected {
            let got = run.results.len();
            let results = std::mem::take(&mut run.results);
            let mesh = run.mesh.clone();
            let (label, deal) = (run.describe(), Self::dealing(run, workers));
            let Some(it) = run.it.as_mut() else {
                return notes;
            };
            let reduced = it.finish_generation(results, mesh.as_ref());
            let (done, mean, at) = (it.finished(), it.k_mean(), it.generations_done());
            match reduced {
                Ok(gen) => {
                    let s = GenSummary { gen, wall_s: now_s - run.gen_started_s, busy_s: run.gen_busy_s };
                    // Kept for the plot and the console WITHOUT its traced
                    // tracks: those go to the view once, in the note. Kept,
                    // they grew the page's memory by every generation's
                    // tracks (up to 5000 of them since 2026-10-15).
                    run.gens.push(s.without_tracks());
                    // Full precision in the console, so runs on different
                    // pool sizes can be compared bit for bit.
                    log::info!("worker pool: {label}: generation {} k = {:e} ({deal} workers)", s.gen.index, s.gen.k);
                    notes.push(Note::Generation(s));
                    run.expected = 0;
                    run.gen_busy_s = 0.0;
                    run.gen_started_s = now_s;
                    if done {
                        if let Some((m, e)) = mean {
                            log::info!("worker pool: {label} done: k = {m:.6} ± {e:.6}");
                        }
                        notes.push(Note::RunDone);
                    } else if !run.paused {
                        Self::cut(run, deal, traced);
                    }
                }
                Err(e) => run.fail(not_added_up(at, &e, got, workers)),
            }
        }
        notes
    }

    /// Worker `w` could not finish a request of run `epoch` (`NO_RUN`: a
    /// request of no run, only noted). The worker is idle again; the run
    /// fails, the pool does not.
    pub fn on_failed(&mut self, w: usize, message: &str, epoch: u32) {
        if w >= self.workers {
            return;
        }
        self.busy[w] = false;
        if epoch == NO_RUN {
            self.note(format!("worker {}: {message}", w + 1));
        } else if let Some(run) = self.current_run(epoch, w, "a failure") {
            run.fail(format!(
                "Run stopped: worker {} could not finish its request ({message}). It still holds \
                 its data: restart the run below.",
                w + 1
            ));
        }
    }

    /// A worker's error event. Either a reply that could not be read (the
    /// worker is alive; the run it belonged to fails) or the worker itself
    /// failing (its data are gone: it is marked dead, gh:#817). Returns the
    /// message the pool fails with, if it must (no worker left, or a worker
    /// died while `data_phase`: its share of the data is lost).
    pub fn on_error(&mut self, w: usize, msg: String, data_phase: bool) -> Option<String> {
        if w >= self.workers || !self.alive[w] {
            return None;
        }
        if msg.starts_with(BAD_MESSAGE) {
            // Its one outstanding reply was lost: it is idle again.
            self.busy[w] = false;
            if let Some(run) = self.run.as_mut().filter(|r| r.going()) {
                run.fail(unreadable("a reply", w, &msg));
            } else {
                self.note(format!("an unreadable reply from worker {}: {msg}", w + 1));
            }
            return None;
        }
        let was_busy = self.busy[w];
        self.alive[w] = false;
        self.busy[w] = false;
        let why = format!(
            "{msg}. On a phone this is usually the browser reclaiming memory (each worker holds {})",
            self.per_worker
        );
        self.died[w] = Some(why.clone());
        let left = self.live_workers();
        self.note(format!("worker {} of {} stopped ({left} left): {why}", w + 1, self.workers));
        if left == 0 || data_phase {
            return Some(format!(
                "Worker {} of {} stopped: {why}. {} The nuclear data must be processed again: \
                 close other tabs, or open the page with ?workers=1, then press Reprocess \
                 (data already in this browser's cache are read back, not processed).",
                w + 1,
                self.workers,
                if left == 0 { "No worker is left." } else { "Its share of the data was not finished." }
            ));
        }
        if let Some(run) = self.run.as_mut().filter(|r| r.going()) {
            // Its chunk (or the source it was sampling) is lost with it; a
            // serial run on it has no worker.
            if was_busy || run.serial == Some(w) {
                run.fail(format!(
                    "Run stopped: worker {} of {} stopped while it held part of the run ({why}). \
                     {left} worker{} still hold{} the data: restart the run below.",
                    w + 1,
                    self.workers,
                    if left == 1 { "" } else { "s" },
                    if left == 1 { "s" } else { "" },
                ));
            }
        }
        None
    }

    /// How many workers a run deals to.
    fn dealing(run: &RunState<T>, live: usize) -> usize {
        if run.serial.is_some() {
            1
        } else {
            live.max(1)
        }
    }

    /// Queue the current generation's chunks for `workers` workers.
    fn cut(run: &mut RunState<T>, workers: usize, traced: usize) {
        let Some(it) = run.it.as_ref() else { return };
        let chunks = it.chunks(workers * CHUNKS_PER_WORKER, traced);
        run.expected = chunks.len();
        run.queue.extend(chunks);
    }

    /// A new run's epoch.
    fn next_epoch(&mut self) -> u32 {
        self.epoch += 1;
        self.epoch
    }

    /// Start a run on `model` (the caller has checked its pool is ready).
    pub fn start_run(&mut self, cfg: KeffConfig, model: T, now_s: f64) -> Result<(), String> {
        // A run is going from the moment it starts, including while worker 1
        // samples its initial source (`it` is still None then). Accepting a
        // second start in that window sent two source requests, and each reply
        // queued generation 0's chunks: the duplicates came back while
        // generation 1 was being reduced (seen on a phone, 2026-10-09).
        if self.going() {
            return Err("a run is already going (it may still be sampling its initial source); wait for it to finish".into());
        }
        let epoch = self.next_epoch();
        self.run = Some(RunState::new(cfg, now_s, epoch, RunOrigin::Fresh, None, model));
        Ok(())
    }

    /// **Restart a failed run** (gh:#817) with the same settings, on `model`,
    /// in parallel on every live worker or serially on the first, keeping
    /// every worker's data. Replies still in flight for the failed run are
    /// discarded as they arrive (module docs).
    pub fn restart_run(&mut self, kind: RestartKind, model: T, now_s: f64) -> Result<(), String> {
        let Some(old) = self.run.as_ref() else {
            return Err("there is no run to restart".into());
        };
        let Some(after) = old.failed.clone() else {
            return Err("the run has not failed".into());
        };
        let cfg = old.cfg;
        let first = (0..self.workers).find(|&v| self.alive[v]).ok_or("no worker is alive")?;
        let (serial, workers) = match kind {
            RestartKind::Parallel => (None, self.live_workers()),
            RestartKind::Serial => (Some(first), 1),
        };
        let epoch = self.next_epoch();
        let busy = (0..self.workers).filter(|&v| self.busy[v]).count();
        let run = RunState::new(cfg, now_s, epoch, RunOrigin::Restart { kind, workers, after }, serial, model);
        let line = format!(
            "{} (replacing run {}); {busy} worker{} still busy with the old run: their replies will be discarded",
            run.describe(),
            epoch - 1,
            if busy == 1 { " is" } else { "s are" }
        );
        self.note(line);
        self.run = Some(run);
        Ok(())
    }

    /// **Stop the run** the reader started: no more chunks are cut, replies
    /// still in flight are discarded as they arrive (it is no longer going),
    /// and a new run may start. Its generations so far stay on screen.
    pub fn stop_run(&mut self) {
        if let Some(run) = self.run.as_mut().filter(|r| r.going()) {
            run.stopped = true;
            run.queue.clear();
            run.results.clear();
            run.expected = 0;
        }
    }

    /// Pause or resume: a paused run finishes the generation in flight and
    /// cuts no more.
    pub fn set_paused(&mut self, paused: bool) {
        let workers = self.live_workers();
        let traced = self.traced_per_generation;
        if let Some(run) = self.run.as_mut().filter(|r| r.going()) {
            let was = run.paused;
            run.paused = paused;
            if was && !paused && run.expected == 0 {
                let n = Self::dealing(run, workers);
                Self::cut(run, n, traced);
            }
        }
    }

    /// The run's requests to send now (call only while the pool is ready):
    /// the initial source to the first usable idle worker, then chunks to
    /// idle live workers (only the serial worker, for a serial run).
    pub fn outbox(&mut self) -> Vec<(usize, RunReq)> {
        let mut out = Vec::new();
        let (alive, busy) = (&self.alive, &mut self.busy);
        if let Some(run) = self.run.as_mut().filter(|r| r.going()) {
            let serial = run.serial;
            let usable = |w: usize| alive[w] && serial.is_none_or(|s| s == w);
            if !run.asked_source {
                // The first usable idle worker samples it; a worker still
                // busy with a replaced run's chunk is waited for.
                if let Some(w) = (0..alive.len()).find(|&w| usable(w) && !busy[w]) {
                    run.asked_source = true;
                    busy[w] = true;
                    out.push((w, RunReq::Source { cfg: run.cfg, epoch: run.epoch }));
                }
            } else {
                for w in 0..alive.len() {
                    if usable(w) && !busy[w] {
                        let Some(c) = run.queue.pop_front() else {
                            break;
                        };
                        busy[w] = true;
                        out.push((w, RunReq::Chunk { cfg: run.cfg, data: Floats::from_vec(c.to_f64s()), epoch: run.epoch }));
                    }
                }
            }
        }
        out
    }
}

/// Where a failure the reader cannot fix should be reported.
pub const REPORT: &str = "github.com/theodoreOnzGit/outram-park-backend/issues";

/// The message when a generation's results cannot be added up: what
/// happened, that it is the demo's bookkeeping and not the physics, and what
/// to do (maintainer, 2026-10-09: errors must be informative in the browser).
pub fn not_added_up(generation: usize, e: &str, got: usize, workers: usize) -> String {
    format!(
        "Run stopped at generation {generation}: the {got} results the {workers} workers sent \
         back could not be added up ({e}). This is a bug in the demo's bookkeeping, not in \
         the physics, and the workers still hold their data: restart the run below (serially \
         rules out a race between workers), and please report this message at {REPORT}."
    )
}

/// The message when a worker's reply cannot be decoded.
pub fn unreadable(what: &str, w: usize, e: &str) -> String {
    format!(
        "Run stopped: {what} from worker {} could not be read ({e}). The workers still hold \
         their data: restart the run below, and please report this message at {REPORT}.",
        w + 1
    )
}

#[cfg(test)]
mod tests;
