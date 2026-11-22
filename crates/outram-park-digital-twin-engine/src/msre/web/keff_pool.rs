//! **The page side of the MSRE Core k_eff tab** (gh:#1571, #1574; moved
//! from the `msre_web` example into the library, gh:#1578): a pool of
//! k_eff workers for one study, the measured fuel/graphite split, and the
//! tab's accuracy presets. No drawing: a page draws from these.
//!
//! [`Pool`] holds the two state machines of `dhoby_ghaut::mc_web::pool`
//! (gh:#1572): `DataRelay` deals the nuclear-data jobs to the workers and
//! relays every product to every other worker, then each builds the model;
//! `KeffPool` runs the power iteration on them, one worker sampling the
//! source and every worker transporting chunks of each generation. The
//! workers are [`super::engine::Runner`]s that only ever get
//! `Request::Keff` ([`super::keff`]), separate from the plant's worker,
//! which keeps running.
//!
//! **The model's default sits outside the ±1000 pcm band (+1726 ± 23 pcm,
//! the V&V record)**; a page showing these runs should say so.
//!
//! What it composes is tested: the two state machines
//! (`dhoby_ghaut::mc_web::pool::tests`), the model
//! (`vv::msre_n1::tests::the_study_plans_the_recorded_default`), the worker
//! ([`super::keff`]'s tests), the split's arithmetic
//! (`session::tests`), and the whole pool on the real model, bit for bit
//! against `run_keff_csg_par` (the ignored test below, about 12 minutes).
//!
//! Research, education and V&V only. NOT VALIDATED.

use super::engine::{Event, Request};
use super::keff::{coordinator_settings, jobs, Assembly, KEv, KReq};
use crate::msre::first_criticality::{self as m, Study};
use crate::msre::session::{coefficient_from_pair, Coefficients, SPLIT_BASE_F, SPLIT_STEP_F};
use bugis::web_demo::link::Link;
use dhoby_ghaut::mc_web::keff::KeffConfig;
use dhoby_ghaut::mc_web::pool::{DataOut, DataRelay, KeffPool, Note, RunReq, RELAY_WORDS_PER_FRAME};
use dhoby_ghaut::web_demo::data_cache::Source;

/// ORNL-4233's critical state, rods at 0.08 % δk/k (`vv::msre_n1::REFERENCE_K`).
pub const K_REF: f64 = m::REFERENCE_K;
/// The maintainer's band for N-1, ±1000 pcm (gh:#1128, 2026-11-10).
pub const BAND_PCM: f64 = 1000.0;
/// One worker's memory for the pool planner, MB: an ESTIMATE (the HTR-10
/// core's worker holds about 545 MB; MSRE's 52 nuclides at one temperature
/// are of the same order). Not measured.
pub const WORKER_MB: f64 = 700.0;
/// The record's seed for the tab's runs (any is fine; this one is fixed so
/// a run can be repeated bit for bit).
pub const SEED: u64 = 1;

/// Accuracy presets: name, histories, inactive, active, and the σ each is
/// expected to give, pcm, scaled by 1/√(histories × active) from the
/// record's single run of 20 000 × 200 active (about 56 pcm, from the
/// 6-seed ±23 pcm). An ESTIMATE for choosing; each run shows its own σ.
pub const PRESETS: [(&str, usize, usize, usize, f64); 3] = [
    ("Quick", 4_000, 30, 140, 150.0),
    ("Standard", 10_000, 50, 200, 80.0),
    ("Careful", 20_000, 100, 400, 40.0),
];

/// A finished run, kept for the measured coefficients (gh:#1574).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Finished {
    pub study: Study,
    pub cfg: KeffConfig,
    pub k: f64,
    pub sigma: f64,
}

/// **The measured split** (gh:#1574): the N-3 record's three runs on this
/// tab's pool, one after another: A salt and graphite at
/// [`SPLIT_BASE_F`]; C the salt raised by [`SPLIT_STEP_F`]; D the graphite
/// raised. α_fuel from A and C, α_graphite from A and D
/// ([`coefficient_from_pair`]), each with its σ. Never sent to the plant
/// except by the reader's button.
#[derive(Clone, Debug, PartialEq)]
pub struct Split {
    pub cfg: KeffConfig,
    pub shield: bool,
    pub u235_scale: f64,
    /// A, C, D as they finish.
    pub runs: Vec<Finished>,
}

impl Split {
    /// The study of run `i` (0 A, 1 C, 2 D).
    pub fn study(&self, i: usize) -> Study {
        let (b, up) = (SPLIT_BASE_F, SPLIT_BASE_F + SPLIT_STEP_F);
        let (salt_f, graphite_f) = match i {
            0 => (b, b),
            1 => (up, b),
            _ => (b, up),
        };
        Study { salt_f, graphite_f, u235_scale: self.u235_scale, shield: self.shield, ..Study::default() }
    }

    /// The coefficients, once the three runs are in.
    pub fn coefficients(&self) -> Option<Coefficients> {
        let [a, c, d] = [self.runs.first()?, self.runs.get(1)?, self.runs.get(2)?];
        let (f, sf) = coefficient_from_pair(a.k, a.sigma, c.k, c.sigma, SPLIT_STEP_F);
        let (g, sg) = coefficient_from_pair(a.k, a.sigma, d.k, d.sigma, SPLIT_STEP_F);
        Some(Coefficients::Measured { fuel_per_f: f, fuel_sigma: sf, graphite_per_f: g, graphite_sigma: sg })
    }
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
    /// [`super::engine::worker::WORKER_JS`] in the browser, engine threads
    /// natively), each repainting `ctx` when it posts.
    ///
    /// # Errors
    /// A browser worker could not be started.
    pub fn start(ctx: &egui::Context, study: Study, workers: usize) -> Result<Self, String> {
        let mut links = Vec::new();
        for _ in 0..workers {
            #[cfg(target_arch = "wasm32")]
            links.push(bugis::web_demo::link::start_web::<Request, Event>(ctx.clone(), super::engine::worker::WORKER_JS, Event::Error)?);
            #[cfg(not(target_arch = "wasm32"))]
            {
                let c = ctx.clone();
                links.push(bugis::web_demo::link::start_native(super::engine::Runner::default(), move || c.request_repaint()));
            }
        }
        Ok(Self::with_links(study, links))
    }

    /// A pool for `study` on workers already started (each an
    /// [`super::engine::Runner`] behind its link). With no links the pool
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
                            log::info!("msre k_eff pool: worker {} built {} nuclides, {} materials in {:.1} s", w + 1, a.nuclides, a.materials, a.secs);
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
    let (_, n, ni, na, _) = PRESETS[i.min(PRESETS.len() - 1)];
    KeffConfig { n_particles: n, n_inactive: ni, n_active: na, seed: SEED, point_source: false, want_sites: false }
}

#[cfg(all(test, not(target_arch = "wasm32")))]
mod tests {
    use super::*;

    /// The presets are what the tab offers (on the fixed seed), a preset
    /// index past the list is the last one, and a forced worker count is
    /// honoured (clamped to 1-16 by `bugis::web_demo::pool::plan`).
    #[test]
    fn presets_and_worker_counts() {
        let q = preset_cfg(0);
        assert_eq!((q.n_particles, q.n_inactive, q.n_active, q.seed), (4_000, 30, 140, SEED));
        assert_eq!(preset_cfg(99), preset_cfg(PRESETS.len() - 1));
        assert_eq!(plan_workers(Some(3)), 3);
        assert_eq!(plan_workers(Some(0)), 1);
        assert!(plan_workers(None) >= 1);
    }

    /// The split's three studies are A, C and D at the record's
    /// temperatures, with the reader's shield and U-235; its coefficients
    /// are `coefficient_from_pair` on A-C and A-D once all three are in.
    #[test]
    fn the_split_runs_a_c_d_and_pairs_them() {
        let mut sp = Split { cfg: preset_cfg(0), shield: false, u235_scale: 1.02, runs: Vec::new() };
        let (b, up) = (SPLIT_BASE_F, SPLIT_BASE_F + SPLIT_STEP_F);
        assert_eq!((sp.study(0).salt_f, sp.study(0).graphite_f), (b, b));
        assert_eq!((sp.study(1).salt_f, sp.study(1).graphite_f), (up, b));
        assert_eq!((sp.study(2).salt_f, sp.study(2).graphite_f), (b, up));
        assert!(!sp.study(1).shield && sp.study(2).u235_scale == 1.02);
        let ks = [(1.02, 0.001), (1.01, 0.002), (1.015, 0.003)];
        for (i, &(k, sigma)) in ks.iter().enumerate() {
            assert_eq!(sp.coefficients(), None, "not before three runs");
            sp.runs.push(Finished { study: sp.study(i), cfg: sp.cfg, k, sigma });
        }
        let (f, sf) = coefficient_from_pair(ks[0].0, ks[0].1, ks[1].0, ks[1].1, SPLIT_STEP_F);
        let (g, sg) = coefficient_from_pair(ks[0].0, ks[0].1, ks[2].0, ks[2].1, SPLIT_STEP_F);
        assert_eq!(sp.coefficients(), Some(Coefficients::Measured { fuel_per_f: f, fuel_sigma: sf, graphite_per_f: g, graphite_sigma: sg }));
    }

    /// A pool on no workers says so (found by this test: it used to deal
    /// the jobs to a worker 0 that does not exist and panic), sends
    /// nothing, reports nothing, and holds a pending run without starting it.
    #[test]
    fn a_pool_without_workers_fails_cleanly() {
        let mut p = Pool::with_links(Study { shield: false, ..Study::default() }, Vec::new());
        assert!(matches!(p.phase, Phase::Failed(_)));
        assert!(!p.data_phase() && p.assembled.is_empty() && p.links.is_empty());
        p.pending = Some(preset_cfg(0));
        assert!(p.pump(1.0).is_empty());
        assert!(p.pending.is_some());
        p.terminate();
    }
    use super::super::keff::{entropy_mesh, native_tape, run_settings, KeffWorker};

    /// **gh:#1571 / #1572 / #1578: a pooled run on the real MSRE model is
    /// `run_keff_csg_par`'s, bit for bit.** The tab's pool (two native
    /// worker threads, the library's `DataRelay` and `KeffPool`) processes
    /// the default study's 54 jobs, relays them, builds the model on each
    /// worker and runs 500 × [2 + 3] on seed 7. Separately, one worker
    /// processes every job itself and `run_keff_csg_par` runs on its model
    /// (2 threads, the entropy mesh). Every generation's k must be equal to
    /// the bit. Prints the k values, the timings and the hardware.
    #[test]
    #[ignore = "processes the MSRE model's 54 jobs twice natively and runs k (many minutes); run with --ignored --nocapture"]
    fn a_pooled_run_is_run_keff_csg_par_bit_for_bit() {
        let study = Study::default();
        let cfg = KeffConfig { n_particles: 500, n_inactive: 2, n_active: 3, seed: 7, point_source: false, want_sites: false };
        let ctx = egui::Context::default();
        let t0 = std::time::Instant::now();
        let mut pool = Pool::start(&ctx, study, 2).expect("pool");
        pool.pending = Some(cfg);
        let mut ks = Vec::new();
        'run: loop {
            for n in pool.pump(bugis::web_demo::platform::now_s()) {
                match n {
                    Note::Generation(g) => ks.push(g.gen.k),
                    Note::RunDone => break 'run,
                }
            }
            if let Phase::Failed(m) = &pool.phase {
                panic!("{m}");
            }
            if let Some(f) = pool.keff.run.as_ref().and_then(|r| r.failed.clone()) {
                panic!("{f}");
            }
            assert!(t0.elapsed().as_secs() < 7200, "not done in two hours");
            std::thread::sleep(std::time::Duration::from_millis(10));
        }
        let pool_s = t0.elapsed().as_secs_f64();
        pool.terminate();
        let t1 = std::time::Instant::now();
        let mut w = KeffWorker::default();
        let list = jobs(&study);
        for (j, job) in list.iter().enumerate() {
            let b = job.tape().map(|t| native_tape(t).expect("tape")).unwrap_or_default();
            w.process(&study, j, &b).expect("process");
        }
        w.assemble(&study, native_tape, || 0.0).expect("assemble");
        let md = w.model.as_ref().expect("model");
        let r = outram_mc_libs::physics::transport_csg::run_keff_csg_par(
            &md.geometry,
            &md.materials,
            &md.nuclides,
            &[],
            Some(&entropy_mesh()),
            m::source_box(),
            &run_settings(&cfg, md.salt_k),
            None,
            &[],
            None,
            outram_mc_libs::physics::compute::ThreadCount::Fixed(2),
        );
        let ref_s = t1.elapsed().as_secs_f64();
        let bits = |v: &[f64]| v.iter().map(|k| k.to_bits()).collect::<Vec<_>>();
        println!("MSRE_POOL_K {:?}", ks.iter().map(|k| format!("{k:.17e}")).collect::<Vec<_>>());
        println!("MSRE_PAR_K  {:?} mean {:.5} ± {:.5}", r.k_by_generation.iter().map(|k| format!("{k:.17e}")).collect::<Vec<_>>(), r.k_mean, r.k_std);
        println!(
            "MSRE_TIMING pool (process + relay + build + run, 2 workers) {pool_s:.1} s; reference (process + build + run, one worker then 2 threads) {ref_s:.1} s; hardware: {}",
            outram_mc_libs::perf_report::HardwareInfo::detect().headline()
        );
        assert_eq!(bits(&ks), bits(&r.k_by_generation));
    }
}
