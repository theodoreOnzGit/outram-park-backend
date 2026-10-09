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

use super::{schedule, AssemblyReport, CoreEv, CoreReq};
use crate::keff::KeffConfig;
use dhoby_ghaut::web_demo::data_cache::Source;
use dhoby_ghaut::web_demo::link::{Floats, BAD_MESSAGE};
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
    /// Every live worker holds the model at `layers`.
    Ready,
    /// The data must be reprocessed (module docs).
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
    Restart {
        kind: RestartKind,
        workers: usize,
        after: String,
    },
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
    /// The run's epoch: requests carry it and replies echo it.
    pub epoch: u32,
    /// Why the run stopped, if it failed. The pool is still ready.
    pub failed: Option<String>,
    pub origin: RunOrigin,
    /// The one worker every request goes to, for a serial run.
    pub serial: Option<usize>,
}

impl RunState {
    fn new(cfg: KeffConfig, now_s: f64, epoch: u32, origin: RunOrigin, serial: Option<usize>) -> Self {
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
        }
    }

    pub fn finished(&self) -> bool {
        self.it
            .as_ref()
            .is_some_and(DistributedPowerIteration::finished)
    }

    /// Going: started, neither finished nor failed (including while the
    /// initial source is sampled, a8b9ad7833).
    pub fn going(&self) -> bool {
        !self.finished() && self.failed.is_none()
    }

    /// What kind of run this is, for its summary and the console.
    pub fn describe(&self) -> String {
        match &self.origin {
            RunOrigin::Fresh => format!("run {}", self.epoch),
            RunOrigin::Restart {
                kind: RestartKind::Parallel,
                workers,
                ..
            } => format!("run {}, a restart in parallel on {workers} workers", self.epoch),
            RunOrigin::Restart {
                kind: RestartKind::Serial,
                ..
            } => format!("run {}, a serial restart on 1 worker", self.epoch),
        }
    }

    /// Stop the run: it cuts no more chunks; replies still in flight for it
    /// are discarded when they arrive (it is not going).
    fn fail(&mut self, why: String) {
        log::error!("htr10 core: {} failed: {why}", self.describe());
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
    /// Per job: processed now, or read from the browser's cache (gh:#818).
    pub sources: Vec<Option<Source>>,
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
            sources: vec![None; n],
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
            alive: vec![true; workers],
            died: vec![None; workers],
            epoch: 0,
            stale_discarded: 0,
            log: Vec::new(),
            out: Vec::new(),
        }
    }

    /// Where the data came from, one line (gh:#818: Leak Before Break, the
    /// page shows it): how many products were read from this browser's
    /// cache and how many processed now.
    pub fn data_source(&self) -> String {
        Source::summarize(self.sources.iter().flatten())
    }

    /// Workers still alive.
    pub fn live_workers(&self) -> usize {
        self.alive.iter().filter(|&&a| a).count()
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

    /// Record something the reader should see, in the panel and the console.
    fn note(&mut self, line: String) {
        log::warn!("htr10 core: {line}");
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
    fn current_run(&mut self, epoch: u32, w: usize, what: &str) -> Option<&mut RunState> {
        if !self.run.as_ref().is_some_and(|r| r.epoch == epoch && r.going()) {
            self.discard(w, what, epoch);
            return None;
        }
        self.run.as_mut()
    }

    /// The live workers whose models are all at `layers`: ready.
    fn check_ready(&mut self, now_s: f64) {
        let all = (0..self.workers)
            .filter(|&v| self.alive[v])
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
            CoreEv::JobStarted { job } => {
                if let Some(s) = self.started.get_mut(job) {
                    *s = true;
                }
            }
            CoreEv::DataNote(m) => self.note(format!("worker {}: {m}", w + 1)),
            CoreEv::Product {
                job,
                secs,
                data,
                source,
            } => {
                if job < self.secs.len() {
                    self.secs[job] = Some(secs);
                    log::info!(
                        "htr10 core: {} {} ({secs:.1} s, worker {})",
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
            CoreEv::Assembled(r) => {
                self.assembled[w] = Some(r);
                self.check_ready(now_s);
            }
            CoreEv::Source {
                sites,
                seed,
                mesh,
                epoch,
            } => {
                // The worker that sampled it is idle again.
                self.busy[w] = false;
                let waiting = self
                    .run
                    .as_ref()
                    .is_some_and(|r| r.epoch == epoch && r.going() && r.it.is_none());
                let live = self.live_workers();
                if !waiting {
                    // A duplicate (a8b9ad7833) or one for a replaced run.
                    self.discard(w, "an initial source", epoch);
                } else if let Some(run) = self.run.as_mut() {
                    match SourceSite::decode(&sites.to_vec()) {
                        Ok(s) => {
                            run.it = Some(DistributedPowerIteration::from_initial_source(
                                &super::run_settings(&run.cfg),
                                &s,
                                seed,
                            ));
                            run.mesh = Some(super::mesh_from_words(&mesh));
                            run.gen_started_s = now_s;
                            let n = Self::dealing(run, live);
                            Self::cut(run, n);
                        }
                        Err(e) => run.fail(unreadable("the initial source", w, &e)),
                    }
                }
            }
            CoreEv::ChunkDone { data, secs, epoch } => {
                self.busy[w] = false;
                let workers = self.live_workers();
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
                            let s = GenSummary {
                                gen,
                                wall_s: now_s - run.gen_started_s,
                                busy_s: run.gen_busy_s,
                            };
                            run.gens.push(s.clone());
                            // Full precision in the console, so runs on
                            // different pool sizes can be compared bit for bit.
                            log::info!(
                                "htr10 core: {label}: generation {} k = {:e} ({deal} workers)",
                                s.gen.index,
                                s.gen.k,
                            );
                            notes.push(Note::Generation(s));
                            run.expected = 0;
                            run.gen_busy_s = 0.0;
                            run.gen_started_s = now_s;
                            if done {
                                if let Some((m, e)) = mean {
                                    log::info!("htr10 core: {label} done: k = {m:.6} ± {e:.6}");
                                }
                                notes.push(Note::RunDone);
                            } else if !run.paused {
                                Self::cut(run, deal);
                            }
                        }
                        Err(e) => run.fail(not_added_up(at, &e, got, workers)),
                    }
                }
            }
            CoreEv::Failed { message, epoch } => {
                self.busy[w] = false;
                if matches!(self.phase, PoolPhase::Processing | PoolPhase::Assembling) {
                    // Its data are incomplete: the pool cannot be ready.
                    self.phase = PoolPhase::Failed(format!(
                        "Worker {} could not build its data: {message}. Reload the page to \
                         start again, and please report this message at {REPORT}.",
                        w + 1
                    ));
                } else if epoch == super::NO_RUN {
                    self.note(format!("worker {}: {message}", w + 1));
                } else if let Some(run) = self.current_run(epoch, w, "a failure") {
                    run.fail(format!(
                        "Run stopped: worker {} could not finish its request ({message}). It \
                         still holds its data: restart the run below.",
                        w + 1
                    ));
                }
            }
        }
        notes
    }

    /// A worker's error event. Either a reply that could not be read (the
    /// worker is alive; the run it belonged to fails) or the worker itself
    /// failing (its data are gone: it is marked dead, gh:#817).
    pub fn on_error(&mut self, w: usize, msg: String) {
        if w >= self.workers || !self.alive[w] {
            return;
        }
        if msg.starts_with(BAD_MESSAGE) {
            // Its one outstanding reply was lost: it is idle again.
            self.busy[w] = false;
            if let Some(run) = self.run.as_mut().filter(|r| r.going()) {
                run.fail(unreadable("a reply", w, &msg));
            } else {
                self.note(format!("an unreadable reply from worker {}: {msg}", w + 1));
            }
            return;
        }
        let was_busy = self.busy[w];
        self.alive[w] = false;
        self.busy[w] = false;
        let why = format!(
            "{msg}. On a phone this is usually the browser reclaiming memory (each worker \
             holds about 545 MB of nuclear data)"
        );
        self.died[w] = Some(why.clone());
        let left = self.live_workers();
        self.note(format!("worker {} of {} stopped ({left} left): {why}", w + 1, self.workers));
        if left == 0 || matches!(self.phase, PoolPhase::Processing | PoolPhase::Assembling) {
            self.phase = PoolPhase::Failed(format!(
                "Worker {} of {} stopped: {why}. {} The nuclear data must be processed again: \
                 close other tabs, or open the page with ?workers=1, then press Reprocess \
                 (data already in this browser's cache are read back, not processed).",
                w + 1,
                self.workers,
                if left == 0 {
                    "No worker is left."
                } else {
                    "Its share of the data was not finished."
                }
            ));
            return;
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
    }

    /// How many workers a run deals to.
    fn dealing(run: &RunState, live: usize) -> usize {
        if run.serial.is_some() {
            1
        } else {
            live.max(1)
        }
    }

    /// Queue the current generation's chunks for `workers` workers.
    fn cut(run: &mut RunState, workers: usize) {
        let Some(it) = run.it.as_ref() else { return };
        let chunks = it.chunks(workers * CHUNKS_PER_WORKER, super::TRACED_PER_GENERATION);
        run.expected = chunks.len();
        run.queue.extend(chunks);
    }

    /// A new run's epoch.
    fn next_epoch(&mut self) -> u32 {
        self.epoch += 1;
        self.epoch
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
        if self.run.as_ref().is_some_and(RunState::going) {
            return Err("a run is already going (it may still be sampling its initial source); wait for it to finish".into());
        }
        let epoch = self.next_epoch();
        self.run = Some(RunState::new(cfg, now_s, epoch, RunOrigin::Fresh, None));
        Ok(())
    }

    /// **Restart a failed run** (gh:#817) with the same settings, in parallel
    /// on every live worker or serially on the first, keeping every worker's
    /// data. Replies still in flight for the failed run are discarded as they
    /// arrive (module docs).
    pub fn restart_run(&mut self, kind: RestartKind, now_s: f64) -> Result<(), String> {
        if self.phase != PoolPhase::Ready {
            return Err("the data are not ready".into());
        }
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
        let run = RunState::new(
            cfg,
            now_s,
            epoch,
            RunOrigin::Restart {
                kind,
                workers,
                after,
            },
            serial,
        );
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

    /// Pause or resume: a paused run finishes the generation in flight and
    /// cuts no more.
    pub fn set_paused(&mut self, paused: bool) {
        let workers = self.live_workers();
        if let Some(run) = self.run.as_mut().filter(|r| r.going()) {
            let was = run.paused;
            run.paused = paused;
            if was && !paused && run.expected == 0 {
                let n = Self::dealing(run, workers);
                Self::cut(run, n);
            }
        }
    }

    /// Rebuild the core at another layer count (only between runs).
    pub fn set_layers(&mut self, layers: usize) -> Result<(), String> {
        if self.phase != PoolPhase::Ready {
            return Err("the data are not ready".into());
        }
        if self.run.as_ref().is_some_and(RunState::going) {
            return Err("a run is going".into());
        }
        if layers != self.layers {
            self.layers = layers;
            self.phase = PoolPhase::Assembling;
            self.run = None;
            for w in (0..self.workers).filter(|&w| self.alive[w]) {
                self.out.push((w, CoreReq::Layers { layers }));
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
                        out.push((
                            w,
                            CoreReq::Source {
                                cfg: run.cfg,
                                epoch: run.epoch,
                            },
                        ));
                    }
                } else {
                    for w in 0..alive.len() {
                        if usable(w) && !busy[w] {
                            let Some(c) = run.queue.pop_front() else {
                                break;
                            };
                            busy[w] = true;
                            out.push((
                                w,
                                CoreReq::Chunk {
                                    cfg: run.cfg,
                                    data: Floats::from_vec(c.to_f64s()),
                                    epoch: run.epoch,
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
         the physics, and the workers still hold their data: restart the run below (serially \
         rules out a race between workers), and please report this message at {REPORT}."
    )
}

/// The message when a worker's reply cannot be decoded.
fn unreadable(what: &str, w: usize, e: &str) -> String {
    format!(
        "Run stopped: {what} from worker {} could not be read ({e}). The workers still hold \
         their data: restart the run below, and please report this message at {REPORT}.",
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
                    crate::engine::Event::Error(m) => {
                        self.state.on_error(w, m);
                        if !self.state.alive[w] {
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
                l.send(crate::engine::Request::Core(req));
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
        assert!(!p.alive[1] && p.died[1].is_some());
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
            assert_eq!(p.stale_discarded, 1, "the duplicate source is discarded, visibly");
            assert!(p.run.as_ref().is_some_and(RunState::finished));
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
        let why = p.run.as_ref().and_then(|r| r.failed.clone()).expect("failed");
        assert!(why.contains("could not be read") && why.contains("restart"), "{why}");
        assert!(p.outbox(1 << 20).is_empty(), "a failed run deals nothing");
        p.restart_run(RestartKind::Parallel, 1.0).expect("restart");
        let run = p.run.as_ref().expect("run");
        assert_eq!(run.epoch, 2);
        assert!(
            run.describe().contains("restart in parallel on 3 workers"),
            "{}",
            run.describe()
        );
        assert!(
            p.log.iter().any(|l| l.contains("restart in parallel") && l.contains("still busy")),
            "{:?}",
            p.log
        );
        // The old run's replies are still queued ahead of the new run's.
        let before = p.stale_discarded;
        let (ks, _, done) = b.drive(&mut p, &mut inflight, |_| false);
        assert!(done);
        assert!(p.stale_discarded >= before + 2, "{} discarded", p.stale_discarded - before);
        assert!(
            p.log.iter().any(|l| l.contains("discarded a chunk result")),
            "{:?}",
            p.log
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
            .run
            .as_ref()
            .and_then(|r| r.failed.as_deref())
            .is_some_and(|m| m.contains("out of memory")));
        p.restart_run(RestartKind::Serial, 1.0).expect("restart");
        assert!(p.run.as_ref().is_some_and(|r| r.describe().contains("serial restart")));
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
        assert_eq!(p.alive, vec![true, false, true]);
        assert_eq!(p.live_workers(), 2);
        let why = p.run.as_ref().and_then(|r| r.failed.clone()).expect("failed");
        assert!(
            why.contains("worker 2 of 3 stopped") && why.contains("2 workers still hold"),
            "{why}"
        );
        p.restart_run(RestartKind::Parallel, 1.0).expect("restart");
        assert!(p.run.as_ref().is_some_and(|r| r.describe().contains("on 2 workers")));
        let (ks, asked, done) = b.drive(&mut p, &mut inflight, |_| false);
        assert!(done);
        assert!(!asked.contains(&1), "a dead worker was asked: {asked:?}");
        assert_eq!(ks, reference, "the restart on the survivors");
        // An unreadable reply fails a run, not the worker.
        p.start_run(b.cfg, 2.0).expect("start");
        let _ = p.outbox(1 << 20);
        p.on_error(0, format!("{BAD_MESSAGE}: garbled"));
        assert!(p.alive[0]);
        assert!(p.run.as_ref().is_some_and(|r| r.failed.is_some()));
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

    /// The pool's links start and stop: a terminated pool takes no work and
    /// delivers nothing.
    #[test]
    fn a_pool_starts_and_terminates() {
        let plan = dhoby_ghaut::web_demo::pool::Plan {
            workers: 2,
            reason: "test".into(),
        };
        let pool = CorePool::start(&egui::Context::default(), plan, 12).expect("start");
        assert_eq!(pool.links.len(), 2);
        assert_eq!(pool.state.labels.len(), 36);
        pool.terminate();
        assert!(pool.links.iter().all(|l| l.drain().is_empty()));
    }
}
