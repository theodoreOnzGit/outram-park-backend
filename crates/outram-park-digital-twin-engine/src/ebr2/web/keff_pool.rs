//! **The page side of the EBR-II simulator's MC page** (gh:#1762): a pool
//! of k_eff workers for one study of the simplified full core, after MSRE's
//! (`crate::msre::web::keff_pool`, gh:#1571/#1578; the same protocol,
//! without the measured coefficient split). The data phase deals the
//! nuclides out and relays the products (`mc_web::pool::DataRelay`); the
//! run phase is `mc_web::pool::KeffPool`.
//!
//! Research, education and V&V only. A SIMPLIFIED FRESH CORE, NOT VALIDATED.

use super::keff::{coordinator_settings, jobs, Assembly, KEv, KReq};
use super::{Event, Request};
use bugis::web_demo::link::Link;
use dhoby_ghaut::mc_web::keff::KeffConfig;
use dhoby_ghaut::mc_web::pool::{DataOut, DataRelay, KeffPool, Note, RunReq, RELAY_WORDS_PER_FRAME};
use dhoby_ghaut::web_demo::data_cache::Source;
use nee_soon::ebr2_core::Study;

/// One worker's memory for the pool planner, MB: an ESTIMATE (41 nuclides
/// at one temperature, fewer than MSRE's 52, and no S(a,b)). Not measured.
pub const WORKER_MB: f64 = 600.0;
/// The fixed seed of the page's runs (so a run can be repeated bit for bit).
pub const SEED: u64 = 1;

/// Accuracy presets: name, histories, inactive, active. The σ each gives is
/// not known before the first record; each run shows its own.
pub const PRESETS: [(&str, usize, usize, usize); 3] = [("Quick", 4_000, 20, 80), ("Standard", 10_000, 40, 160), ("Careful", 20_000, 80, 320)];

/// A finished run.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Finished {
    pub study: Study,
    pub cfg: KeffConfig,
    pub k: f64,
    pub sigma: f64,
}

/// Where a pool is.
#[derive(Clone, Debug, PartialEq)]
pub enum Phase {
    Processing,
    Assembling,
    Ready,
    Failed(String),
}

/// A pool of k_eff workers built for one study: the page side of the
/// tab's Monte Carlo (module docs).
pub struct Pool {
    /// The study whose data the workers hold.
    pub study: Study,
    /// One link per worker (a Web Worker in the browser, a thread natively).
    pub links: Vec<Link<Request, Event>>,
    /// The data phase: jobs dealt, products relayed, assembly.
    pub data: DataRelay,
    /// The run phase: the power iteration on the workers.
    pub keff: KeffPool<Study>,
    pub phase: Phase,
    /// What each worker reported once its model was built.
    pub assembled: Vec<Option<Assembly>>,
    /// When the pool started and when its data were ready, s
    /// (`bugis::web_demo::platform::now_s`).
    pub started_s: f64,
    pub ready_s: Option<f64>,
    /// A run waiting for the data.
    pub pending: Option<KeffConfig>,
}

impl Pool {
    /// Start `workers` k_eff workers for `study` (Web Workers on
    /// [`super::worker::WORKER_JS`] in the browser, engine threads
    /// natively), each repainting `ctx` when it posts.
    ///
    /// # Errors
    /// A browser worker could not be started.
    pub fn start(ctx: &egui::Context, study: Study, workers: usize) -> Result<Self, String> {
        let mut links = Vec::new();
        for _ in 0..workers {
            #[cfg(target_arch = "wasm32")]
            links.push(bugis::web_demo::link::start_web::<Request, Event>(ctx.clone(), super::worker::WORKER_JS, Event::Error)?);
            #[cfg(not(target_arch = "wasm32"))]
            {
                let c = ctx.clone();
                links.push(bugis::web_demo::link::start_native(super::Runner::default(), move || c.request_repaint()));
            }
        }
        Ok(Self::with_links(study, links))
    }

    /// A pool for `study` on workers already started (each an
    /// [`super::Runner`] behind its link). With no links the pool
    /// starts [`Phase::Failed`] (there is nobody to deal the jobs to).
    pub fn with_links(study: Study, links: Vec<Link<Request, Event>>) -> Self {
        let list = jobs(&study);
        let labels = list.iter().map(|j| j.label()).collect();
        let costs = list.iter().map(|j| j.cost()).collect();
        let n = links.len();
        Self {
            study,
            links,
            data: DataRelay::new(n, labels, costs),
            keff: KeffPool::new(n, format!("about {WORKER_MB:.0} MB of nuclear data (an estimate)"), coordinator_settings, super::keff::TRACED),
            phase: if n == 0 { Phase::Failed("no workers in the pool".into()) } else { Phase::Processing },
            assembled: vec![None; n],
            started_s: bugis::web_demo::platform::now_s(),
            ready_s: None,
            pending: None,
        }
    }

    /// Close every worker.
    pub fn terminate(&self) {
        for l in &self.links {
            l.terminate();
        }
    }

    /// The data are being processed or assembled.
    pub fn data_phase(&self) -> bool {
        matches!(self.phase, Phase::Processing | Phase::Assembling)
    }

    /// Take every worker's events and send what is due (call each frame;
    /// never blocks). Returns the generations and run ends seen.
    pub fn pump(&mut self, now: f64) -> Vec<Note> {
        let mut notes = Vec::new();
        for w in 0..self.links.len() {
            for e in self.links[w].drain() {
                match e {
                    Event::Keff(k) => match k {
                        KEv::Started { job } => self.data.on_started(job),
                        KEv::Product { job, secs, data, cached_ms } => {
                            let source = match cached_ms {
                                Some(created_ms) => Source::Cached { created_ms, code: dhoby_ghaut::mc_web::processed_cache::CODE.to_string() },
                                None => Source::Processed,
                            };
                            self.data.on_product(w, job, secs, data, source);
                        }
                        KEv::Assembled(a) => {
                            // The console line an unattended browser check reads.
                            #[cfg(target_arch = "wasm32")]
                            log::info!("ebr2 k_eff pool: worker {} built {} nuclides, {} materials in {:.1} s", w + 1, a.nuclides, a.materials, a.secs);
                            self.assembled[w] = Some(a);
                            let all = (0..self.links.len()).filter(|&v| self.keff.alive[v]).all(|v| self.assembled[v].is_some());
                            if all && self.keff.live_workers() > 0 {
                                self.phase = Phase::Ready;
                                self.ready_s.get_or_insert(now);
                            }
                        }
                        KEv::Source { sites, seed, mesh, epoch } => self.keff.on_source(w, &sites, seed, &mesh, epoch, now),
                        KEv::ChunkDone { data, secs, epoch } => notes.extend(self.keff.on_chunk_done(w, &data, secs, epoch, now)),
                        KEv::Failed { message, epoch } => {
                            if self.data_phase() {
                                self.keff.idle(w);
                                self.phase = Phase::Failed(format!("Worker {} could not build its data: {message}.", w + 1));
                            } else {
                                self.keff.on_failed(w, &message, epoch);
                            }
                        }
                        KEv::Raster { .. } => {}
                    },
                    Event::Error(msg) => {
                        let dp = self.data_phase();
                        if let Some(why) = self.keff.on_error(w, msg, dp) {
                            self.phase = Phase::Failed(why);
                        }
                        if !self.keff.alive[w] {
                            self.links[w].terminate();
                        }
                    }
                    _ => {}
                }
            }
        }
        if !matches!(self.phase, Phase::Failed(_)) {
            let (out, all) = self.data.outbox(RELAY_WORDS_PER_FRAME, self.phase == Phase::Processing);
            for d in out {
                let (w, r) = match d {
                    DataOut::Process { worker, jobs } => (worker, KReq::Process { study: self.study, jobs }),
                    DataOut::Relay { worker, job, data } => (worker, KReq::Product { job, data }),
                    DataOut::Assemble { worker } => (worker, KReq::Assemble { study: self.study }),
                };
                self.links[w].send(Request::Keff(r));
            }
            if all && self.phase == Phase::Processing {
                self.phase = Phase::Assembling;
            }
            if self.phase == Phase::Ready {
                if let Some(cfg) = self.pending.take() {
                    if let Err(e) = self.keff.start_run(cfg, self.study, now) {
                        self.keff.note(e);
                    }
                }
                for (w, r) in self.keff.outbox() {
                    let r = match r {
                        RunReq::Source { cfg, epoch } => KReq::Source { cfg, epoch },
                        RunReq::Chunk { cfg, data, epoch } => KReq::Chunk { cfg, data, epoch },
                    };
                    self.links[w].send(Request::Keff(r));
                }
            }
        }
        notes
    }
}

/// How many pool workers to start: `forced` (a page's `?workers=N`), else
/// the device's plan for [`WORKER_MB`] each (`bugis::web_demo::pool`).
pub fn plan_workers(forced: Option<usize>) -> usize {
    bugis::web_demo::pool::plan(bugis::web_demo::pool::Device::probe(), WORKER_MB, forced).workers
}

/// The settings of preset `i` (clamped) on [`SEED`].
pub fn preset_cfg(i: usize) -> KeffConfig {
    let (_, n, ni, na) = PRESETS[i.min(PRESETS.len() - 1)];
    KeffConfig { n_particles: n, n_inactive: ni, n_active: na, seed: SEED, point_source: false, want_sites: false }
}

#[cfg(all(test, not(target_arch = "wasm32")))]
mod tests {
    use super::*;

    /// The presets on the fixed seed, a preset past the list is the last,
    /// a forced worker count is honoured, and a pool with no workers fails
    /// rather than waiting forever.
    #[test]
    fn presets_workers_and_an_empty_pool() {
        let q = preset_cfg(0);
        assert_eq!((q.n_particles, q.n_inactive, q.n_active, q.seed), (4_000, 20, 80, SEED));
        assert_eq!(preset_cfg(9), preset_cfg(PRESETS.len() - 1));
        assert_eq!(plan_workers(Some(2)), 2);
        let p = Pool::with_links(Study::default(), Vec::new());
        assert!(matches!(p.phase, Phase::Failed(_)));
    }
}
