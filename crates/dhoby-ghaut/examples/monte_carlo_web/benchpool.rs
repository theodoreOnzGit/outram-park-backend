//! **Run k_eff of a benchmark rung on a pool of workers** (gh:#905, #907):
//! Jemima, HEU-SOL-THERM-009 and LEU-COMP-THERM-008, which ran on one Web
//! Worker until 2026-10-16 ("it is singlethreaded now, too slow",
//! maintainer).
//!
//! **Nothing here is new physics or a new protocol.**
//! - Each worker is an ordinary engine ([`crate::engine::McEngine`]) that
//!   loads the rung with an ordinary [`Request::Load`]. In the browser every
//!   worker after the first reads the processed data back from the shared
//!   cache (gh:#818) instead of processing again.
//! - The run is the HTR-10 whole core's pool protocol
//!   ([`crate::htr10::core::pool::PoolState`], entered ready): the page holds
//!   `transport_csg::distributed`'s coordinator, deals each generation's
//!   histories in chunks to idle workers, and reduces them in history order.
//!   Each worker serves the chunks on its loaded rung
//!   ([`crate::engine::serve_pool`], [`crate::csgrung::pool_chunk`]).
//!
//! **What the answer is.** `run_keff_csg_par`'s, bit for bit, for any number
//! of workers: the driver the five-route records were measured with
//! (`icsbep_five_route_keff`, `ComputeType::CpuMultiThread`). The serial
//! `CsgPowerIteration` the page used before draws from one sequential stream,
//! so the same seed gives a different (statistically equivalent) `k`.
//! `tests::a_pooled_run_is_run_keff_csg_par_on_any_number_of_workers` pins
//! both claims.

// The browser build drives it from the app; the native build from tests.
#![cfg_attr(not(target_arch = "wasm32"), allow(dead_code))]

use crate::engine::{Event, Request, Tier};
use crate::htr10::core::pool::{GenSummary, Note, PoolState, RELAY_WORDS_PER_FRAME};
use crate::keff::{Generation, KeffConfig};
use crate::table::Rung;
use dhoby_ghaut::web_demo::link::Link;
use dhoby_ghaut::web_demo::pool::Plan;
use outram_mc_libs::physics::keff::{GenerationReport, HistoryCounts};

/// The load id every pool worker uses (each worker loads once).
const LOAD_ID: u32 = 1;

/// One worker's load.
#[derive(Clone, Debug, PartialEq)]
pub enum Load {
    /// Tapes done so far.
    Loading(usize),
    Ready,
    Failed(String),
}

/// The pool: its workers, their loads, then the run state.
pub struct BenchPool {
    pub links: Vec<Link<Request, Event>>,
    pub rung: Rung,
    pub plan: Plan,
    pub loads: Vec<Load>,
    /// Data notes from the workers' loads (cache hits and misses), newest last.
    pub notes: Vec<String>,
    /// Set once every surviving worker has loaded.
    pub state: Option<PoolState>,
    pub started_s: f64,
}

impl BenchPool {
    /// Start `plan.workers` workers and ask each to load `rung` at `tier`.
    pub fn start(ctx: &egui::Context, plan: Plan, rung: Rung, tier: Tier) -> Result<Self, String> {
        let mut links = Vec::new();
        for _ in 0..plan.workers.max(1) {
            #[cfg(target_arch = "wasm32")]
            links.push(dhoby_ghaut::web_demo::link::start_web::<Request, Event>(ctx.clone(), "./worker.js", Event::Error)?);
            #[cfg(not(target_arch = "wasm32"))]
            {
                let c = ctx.clone();
                links.push(dhoby_ghaut::web_demo::link::start_native(crate::engine::McEngine::default(), move || c.request_repaint()));
            }
        }
        for l in &links {
            l.send(Request::Load { id: LOAD_ID, rung, tier });
        }
        let n = links.len();
        Ok(Self { links, rung, plan, loads: vec![Load::Loading(0); n], notes: Vec::new(), state: None, started_s: dhoby_ghaut::web_demo::platform::now_s() })
    }

    /// Workers whose data are loaded (or, before that, still loading).
    pub fn workers(&self) -> usize {
        self.links.len()
    }

    /// Tapes processed or read so far, over every worker, as a fraction of
    /// all of them (the loading bar).
    pub fn load_progress(&self) -> f64 {
        let per = self.rung.jobs_for(self.rung.tier(Tier::Exact)).len().max(1) as f64;
        let done: f64 = self
            .loads
            .iter()
            .map(|l| match l {
                Load::Loading(k) => *k as f64,
                Load::Ready | Load::Failed(_) => per,
            })
            .sum();
        done / (per * self.loads.len().max(1) as f64)
    }

    /// Each frame: take every worker's events, send what is due. Returns the
    /// run's notes (generations, the end of a run).
    pub fn pump(&mut self) -> Vec<Note> {
        let now = dhoby_ghaut::web_demo::platform::now_s();
        let mut out = Vec::new();
        for (w, link) in self.links.iter().enumerate() {
            for e in link.drain() {
                match (e, self.state.as_mut()) {
                    (Event::JobDone { .. }, None) => {
                        if let Load::Loading(k) = &mut self.loads[w] {
                            *k += 1;
                        }
                    }
                    (Event::Ready { .. }, None) => self.loads[w] = Load::Ready,
                    (Event::DataNote(m), _) => self.notes.push(format!("worker {}: {m}", w + 1)),
                    (Event::Error(m), None) => {
                        self.loads[w] = Load::Failed(m);
                        link.terminate();
                    }
                    (Event::Core(ev), Some(s)) => out.extend(s.on_event(w, ev, now)),
                    (Event::Error(m), Some(s)) => {
                        s.on_error(w, m);
                        if !s.alive[w] {
                            link.terminate();
                        }
                    }
                    _ => {}
                }
            }
        }
        let loading = self.loads.iter().any(|l| matches!(l, Load::Loading(_)));
        if self.state.is_none() && !loading && self.loads.contains(&Load::Ready) {
            let per = format!("the {} rung's processed nuclear data", self.rung.info().name);
            let mut s = PoolState::ready(self.links.len(), per, now);
            for (w, l) in self.loads.iter().enumerate() {
                if let Load::Failed(m) = l {
                    s.alive[w] = false;
                    s.died[w] = Some(m.clone());
                }
            }
            self.state = Some(s);
        }
        if let Some(s) = self.state.as_mut() {
            for (w, req) in s.outbox(RELAY_WORDS_PER_FRAME) {
                if let Some(l) = self.links.get(w) {
                    l.send(Request::Core(req));
                }
            }
        }
        out
    }

    /// Why the pool cannot run, if every worker failed to load.
    pub fn failed(&self) -> Option<String> {
        (!self.loads.is_empty() && self.loads.iter().all(|l| matches!(l, Load::Failed(_)))).then(|| match &self.loads[0] {
            Load::Failed(m) => format!("no worker could load the data: {m}"),
            _ => unreachable!(),
        })
    }

    /// Stop every worker (a dropped `Link` would leave its Web Worker running).
    pub fn terminate(&self) {
        for l in &self.links {
            l.terminate();
        }
    }
}

/// A pooled generation as the page's shared [`Generation`], the shape the
/// serial run's console and plot read: no history counts (a CSG run reports
/// none) and no source sites (Run mode draws its own neutrons).
pub fn generation(s: &GenSummary, cfg: KeffConfig) -> Generation {
    let g = &s.gen;
    Generation {
        report: GenerationReport {
            index: g.index,
            active: g.active,
            k: g.k,
            entropy: g.entropy,
            k_mean: g.k_mean,
            bank_size: g.bank_size,
            production: g.k * cfg.n_particles as f64,
            counts: HistoryCounts::default(),
        },
        sites: Vec::new(),
    }
}

#[cfg(all(test, not(target_arch = "wasm32")))]
mod tests {
    use super::*;
    use crate::table::Loaded;
    use outram_mc_libs::physics::compute::{ComputeType, ThreadCount};
    use outram_mc_libs::physics::transport_csg::run_keff_csg_par;

    /// Drive a pool of `n` native workers through one run; the `k` of every
    /// generation and the final mean.
    fn pooled(n: usize, cfg: KeffConfig) -> (Vec<f64>, Option<(f64, f64)>) {
        let (ks, mean, _) = pooled_timed(n, cfg);
        (ks, mean)
    }

    /// [`pooled`], with the run's wall time from its start to its last
    /// generation (the data loads excluded), s.
    fn pooled_timed(n: usize, cfg: KeffConfig) -> (Vec<f64>, Option<(f64, f64)>, f64) {
        let ctx = egui::Context::default();
        let plan = Plan { workers: n, reason: "test".into() };
        let mut p = BenchPool::start(&ctx, plan, Rung::Jemima, Tier::Exact).expect("start");
        let t0 = std::time::Instant::now();
        let mut ks = Vec::new();
        let mut mean = None;
        let mut started = false;
        let mut run_t0 = std::time::Instant::now();
        loop {
            for note in p.pump() {
                match note {
                    Note::Generation(s) => {
                        ks.push(s.gen.k);
                        mean = s.gen.k_mean;
                    }
                    Note::RunDone => {
                        p.terminate();
                        return (ks, mean, run_t0.elapsed().as_secs_f64());
                    }
                    _ => {}
                }
            }
            if let Some(s) = p.state.as_mut().filter(|_| !started) {
                s.start_run(cfg, 0.0).expect("start the run");
                started = true;
                run_t0 = std::time::Instant::now();
            }
            assert!(p.failed().is_none(), "{:?}", p.failed());
            assert!(t0.elapsed().as_secs() < 1200, "the pooled run did not finish");
            std::thread::sleep(std::time::Duration::from_millis(2));
        }
    }

    /// **Methodology.** Jemima at NJOY's tolerance 0.001 (~~the Watch tier, 0.01~~ until 2026-10-16, gh:#876 item 3), 400
    /// neutrons × [3 inactive + 5 active], seed 7, run three ways: on a pool
    /// of 1 native worker, on a pool of 3 (each loading the data itself, as
    /// Web Workers do), and by outram-mc-libs' `run_keff_csg_par` directly
    /// on the same data and settings. Pass: every generation's `k` is the
    /// same `f64` in all three. This is the claim the module docs make, that
    /// the pool reproduces the driver the five-route records used and does
    /// not depend on the worker count. ~~**Result (2026-10-16, on fc6d02257
    /// plus this change):** all three agree bit for bit in all 8
    /// generations; mean k = 0.97397 ± 0.01815 (a harness check at 400
    /// neutrons, not a benchmark result). 87 s, most of it the four native
    /// loads of the three tapes.~~ (at 0.01) **Result at 0.001 (2026-10-16,
    /// gh:#876 item 3):** all three agree bit for bit; mean k = 0.97397 ±
    /// 0.01815, the 0.01 value to the printed digits (a fast system: these
    /// 400 neutrons take the same paths). 141 s on an i9-13900K, the loads
    /// now at 0.001.
    #[test]
    fn a_pooled_run_is_run_keff_csg_par_on_any_number_of_workers() {
        let cfg = KeffConfig { n_particles: 400, n_inactive: 3, n_active: 5, seed: 7, point_source: false, want_sites: false };
        let (one, m1) = pooled(1, cfg);
        let (three, m3) = pooled(3, cfg);
        assert_eq!(one.len(), 8);
        assert_eq!(one, three, "1 worker against 3");
        assert_eq!(m1, m3);

        let l = match crate::load_native(Rung::Jemima, Tier::Exact, |_, _| {}).expect("load") {
            Loaded::Jemima(l) => l,
            _ => unreachable!(),
        };
        let p = &l.phys;
        let mut settings = crate::csgrung::run_settings(cfg, p.temperature_k);
        settings.compute = ComputeType::CpuMultiThread(ThreadCount::Fixed(2));
        let mesh = crate::csgrung::pool_source((&p.geometry, &p.materials, &p.nuclides), p.source_box, <crate::jemima::Jemima as crate::csgrung::CsgCase>::entropy_box(), p.temperature_k, cfg).mesh;
        let r = run_keff_csg_par(&p.geometry, &p.materials, &p.nuclides, &[], Some(&mesh), p.source_box, &settings, None, &[], None, ThreadCount::Fixed(2));
        assert_eq!(r.k_by_generation, one, "the pool against run_keff_csg_par");
        eprintln!("  pooled Jemima 400 x [3 + 5], seed 7: k = {:?} (1 and 3 workers and run_keff_csg_par agree bit for bit)", m1);
    }

    /// **Methodology.** The speed-up the pool exists for: Jemima at the
    /// Watch tier, 4000 neutrons × [2 inactive + 8 active], seed 1, on 1 and
    /// on 4 native workers (threads standing in for Web Workers), timing
    /// the run alone from its start to its last generation (the loads are
    /// excluded). Pass: the two give the same `k` bit for bit. The times
    /// are printed and recorded here; they are native threads on the
    /// development machine, not a browser. Ignored by default (about a
    /// minute, mostly the loads): `cargo test --release -p dhoby-ghaut
    /// --example monte_carlo_web -- --ignored pool_speedup --nocapture`.
    /// **Result (2026-10-16, 36-thread desktop):** 1 worker 1.9 s, 4 workers
    /// 0.7 s, a speed-up of 2.7 (below 4: a generation this small is
    /// 1 s of transport split into 12 chunks, and the page's reduction and
    /// resample are serial); k = 0.97713 ± 0.00352 on both, bit for bit. Not
    /// yet measured in a browser (gh:#907).
    #[test]
    #[ignore = "a timing measurement, about two minutes"]
    fn pool_speedup() {
        let cfg = KeffConfig { n_particles: 4000, n_inactive: 2, n_active: 8, seed: 1, point_source: false, want_sites: false };
        let (k1, m1, t1) = pooled_timed(1, cfg);
        let (k4, m4, t4) = pooled_timed(4, cfg);
        assert_eq!(k1, k4);
        eprintln!("  Jemima 4000 x [2 + 8]: 1 worker {t1:.1} s, 4 workers {t4:.1} s, speed-up {:.2}; k = {:?} both", t1 / t4, m1);
        assert_eq!(m1, m4);
    }
}
