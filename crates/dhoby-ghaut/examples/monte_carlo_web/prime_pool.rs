//! **A rung's nuclear data processed on several workers** (maintainer,
//! 2026-10-16: "data processing should also use multiple workers").
//!
//! Before 2026-10-16 every benchmark rung processed its tapes one after
//! another in the page's single engine worker; only the HTR-10 whole core
//! (`htr10::core::pool`) spread them over a pool. Now a load with two or more
//! jobs first runs here:
//!
//! 1. the jobs are dealt longest first to `web_demo::pool` workers
//!    ([`crate::htr10::core::schedule`], the whole core's own dealer, by the
//!    rung's progress weights);
//! 2. each worker makes only its jobs' products, through the same builder
//!    code a load runs ([`crate::rungs::RungBuilder::prime`]), and saves each
//!    to the shared cache (IndexedDB, gh:#818), posting the load's own
//!    `JobStarted` / `JobDone` events;
//! 3. once every worker is done, the page stops them and sends the ordinary
//!    [`Request::Load`] to its engine, which reads every product back from
//!    the cache.
//!
//! **Nothing new is computed and nothing is computed twice:** a product made
//! here is the product a load would have made (the same `make`), and its
//! cache key is the tape's contents, the settings and the code version.
//! **Failure is visible and safe** (Leak Before Break): a worker that fails
//! is named in the cache notes and the load goes on in the engine, which
//! processes whatever the cache lacks. Leaving the view stops the workers at
//! once (gh:#876 item 4); finished tapes stay cached, so coming back resumes.

// The browser build drives it from the app; the native build from tests.
#![cfg_attr(not(target_arch = "wasm32"), allow(dead_code))]

use crate::engine::{Event, Request, Tier};
use crate::table::Rung;
use dhoby_ghaut::web_demo::link::Link;
use dhoby_ghaut::web_demo::pool::Plan;

/// What [`PrimePool::pump`] found this frame.
#[derive(Debug, PartialEq)]
pub enum Outcome {
    /// Workers are still processing.
    Working,
    /// Every worker finished: send the engine its load.
    Done,
    /// A worker failed (named); the engine should load now and process what
    /// the cache lacks.
    Failed(String),
}

/// The workers processing one load's jobs.
pub struct PrimePool {
    /// The load this pool processes for: when it is done, the page sends
    /// the engine `Request::Load { id, rung, tier }`.
    pub id: u32,
    pub rung: Rung,
    pub tier: Tier,
    links: Vec<Link<Request, Event>>,
    /// Each worker's jobs, as dealt.
    pub deal: Vec<Vec<usize>>,
    /// Workers that have posted `Ready`.
    ready: Vec<bool>,
    pub plan: Plan,
}

/// One processing worker's memory for [`dhoby_ghaut::web_demo::pool::plan`],
/// MB: the rung's own Run-pool figure where it has one, else 400 (an
/// estimate, NOT measured: the largest tapes, U-235 and U-238, are what one
/// worker holds while it processes; measuring it is gh:#907's open item).
pub fn worker_mb(rung: Rung) -> f64 {
    rung.run_pool_mb().unwrap_or(400.0)
}

/// Each job's processing cost for the deal: the rung's progress weights, or
/// equal weights where it gives none for this tier.
pub fn costs(rung: Rung, tier: Tier) -> Vec<f64> {
    let n = rung.jobs_for(rung.tier(tier)).len();
    let w = rung.job_weights(rung.tier(tier));
    if w.len() == n {
        w.iter().map(|&x| x.max(0.01)).collect()
    } else {
        vec![1.0; n]
    }
}

/// How many workers to deal `jobs` jobs to: no more than there are jobs,
/// and none at all (`0`) for a load the engine should simply do itself (one
/// job or one worker gains nothing).
pub fn workers_for(plan_workers: usize, jobs: usize) -> usize {
    let w = plan_workers.min(jobs);
    if w >= 2 {
        w
    } else {
        0
    }
}

impl PrimePool {
    /// Start the workers for load `id` of `rung`, or `None` when the engine
    /// should process the load alone ([`workers_for`]).
    pub fn start(ctx: &egui::Context, plan: Plan, rung: Rung, tier: Tier, id: u32) -> Option<Result<Self, String>> {
        let costs = costs(rung, tier);
        let n = workers_for(plan.workers, costs.len());
        if n == 0 {
            return None;
        }
        let deal = crate::htr10::core::schedule(&costs, n);
        let mut links = Vec::new();
        for jobs in &deal {
            #[cfg(target_arch = "wasm32")]
            let link = match dhoby_ghaut::web_demo::link::start_web::<Request, Event>(ctx.clone(), "./worker.js", Event::Error) {
                Ok(l) => l,
                Err(e) => {
                    for l in &links {
                        Link::terminate(l);
                    }
                    return Some(Err(e));
                }
            };
            #[cfg(not(target_arch = "wasm32"))]
            let link = {
                let c = ctx.clone();
                dhoby_ghaut::web_demo::link::start_native(crate::engine::McEngine::default(), move || c.request_repaint())
            };
            link.send(Request::Prime { id, rung, tier, jobs: jobs.clone() });
            links.push(link);
        }
        let ready = vec![false; links.len()];
        Some(Ok(Self { id, rung, tier, links, deal, ready, plan }))
    }

    /// Take every worker's events: the load's progress events (and data
    /// notes) to hand to the page as if its engine had sent them, and the
    /// pool's outcome. A worker's `Ready` is the pool's business, never the
    /// page's (it would end the page's load early).
    pub fn pump(&mut self) -> (Vec<Event>, Outcome) {
        let mut out = Vec::new();
        let mut failed = None;
        for (w, link) in self.links.iter().enumerate() {
            for e in link.drain() {
                match e {
                    Event::Ready { .. } => self.ready[w] = true,
                    Event::Error(m) => failed = Some(format!("data worker {} of {} failed ({m}); the page's engine processes what the cache lacks", w + 1, self.links.len())),
                    e @ (Event::JobStarted { .. } | Event::JobDone { .. }) => out.push(e),
                    Event::DataNote(m) => out.push(Event::DataNote(format!("data worker {}: {m}", w + 1))),
                    _ => {}
                }
            }
        }
        let outcome = match failed {
            Some(m) => Outcome::Failed(m),
            None if self.ready.iter().all(|&r| r) => Outcome::Done,
            None => Outcome::Working,
        };
        (out, outcome)
    }

    /// Workers in the pool.
    pub fn workers(&self) -> usize {
        self.links.len()
    }

    /// Stop every worker (a dropped `Link` would leave its Web Worker
    /// running). Products already saved stay in the cache.
    pub fn terminate(&self) {
        for l in &self.links {
            l.terminate();
        }
    }
}

#[cfg(all(test, not(target_arch = "wasm32")))]
mod tests {
    use super::*;

    /// One job, or one worker, is the engine's alone; otherwise no more
    /// workers than jobs; every rung's costs cover its jobs, and the deal
    /// gives every job to exactly one worker.
    #[test]
    fn a_load_is_dealt_once_each_and_only_when_it_pays() {
        assert_eq!(workers_for(8, 1), 0);
        assert_eq!(workers_for(1, 12), 0);
        assert_eq!(workers_for(4, 3), 3);
        assert_eq!(workers_for(4, 35), 4);
        for &rung in crate::table::ALL {
            let c = costs(rung, Tier::Exact);
            assert_eq!(c.len(), rung.jobs_for(rung.tier(Tier::Exact)).len(), "{}", rung.info().name);
            assert!(c.iter().all(|&x| x > 0.0));
            let n = workers_for(4, c.len());
            if n == 0 {
                continue;
            }
            let deal = crate::htr10::core::schedule(&c, n);
            let mut all: Vec<usize> = deal.iter().flatten().copied().collect();
            all.sort_unstable();
            assert_eq!(all, (0..c.len()).collect::<Vec<_>>(), "{}", rung.info().name);
        }
    }

    /// **Methodology.** Jemima's three tapes on a pool of 2 native workers:
    /// each primes only its dealt jobs, posting the load's `JobStarted` /
    /// `JobDone` for exactly those, then `Ready`, which the pool keeps to
    /// itself. Pass: every job is reported done exactly once, the outcome is
    /// `Done`, no `Ready` reaches the page. (Natively each worker's cache is
    /// its own memory, so this checks the protocol and the dealing, not the
    /// shared IndexedDB, which only a browser has.)
    #[test]
    fn a_prime_pool_processes_every_job_once_then_is_done() {
        let ctx = egui::Context::default();
        let plan = Plan { workers: 2, reason: "test".into() };
        let mut p = PrimePool::start(&ctx, plan, Rung::Jemima, Tier::Exact, 7).expect("pays").expect("starts");
        assert_eq!(p.workers(), 2);
        let mut done = Vec::new();
        let t = std::time::Instant::now();
        loop {
            let (evs, outcome) = p.pump();
            for e in evs {
                match e {
                    Event::JobDone { id, index, .. } => {
                        assert_eq!(id, 7);
                        done.push(index);
                    }
                    Event::Ready { .. } => panic!("a worker's Ready reached the page"),
                    _ => {}
                }
            }
            match outcome {
                Outcome::Done => break,
                Outcome::Failed(m) => panic!("{m}"),
                Outcome::Working => {}
            }
            assert!(t.elapsed().as_secs() < 600, "timed out");
            std::thread::sleep(std::time::Duration::from_millis(20));
        }
        p.terminate();
        done.sort_unstable();
        assert_eq!(done, vec![0, 1, 2]);
    }
}
