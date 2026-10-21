//! **Seed-pooled ensembles as a library feature** — GitHub #494.
//!
//! # What this replaces
//!
//! Every pooled number in this crate's record came from an example that
//! re-implemented the same loop: spawn threads, chunk the seeds, run each,
//! pool with `vv::pooled`. This module is that loop, once, plus the check the
//! per-example copies never made: **does the seed-to-seed scatter agree with
//! each run's own internal `σ`?** (`χ²/dof` against a fixed two-sided 95 %
//! band, and leave-one-out outlier flags — the statistics are
//! [`raffles::estimators::seed_consistency`].)
//!
//! # What is run is the caller's business
//!
//! The runner takes a closure `Fn(u64) -> (value, sigma)` for one seed, so a
//! `k`-eigenvalue case, a fixed-source tally or anything else plugs in
//! unchanged. The closure is run exactly as the caller wrote it, so an
//! ensemble's per-seed values are bit-identical to running those seeds one at
//! a time.
//!
//! # Thread-count independence
//!
//! Seeds are split **in order** into `ceil(N / workers)`-sized chunks and each
//! seed writes its own slot, so the per-seed vector — not just the mean — is
//! independent of the worker count. This is exactly the scheme
//! `examples/godiva_keff_ensemble.rs` used before it was migrated here.
//!
//! # V&V gate (from #494, fixed in advance)
//!
//! On Godiva (and one TRISO case): the pooled `σ` shrinks as `1/√N` within its
//! own uncertainty — measured by [`EnsembleReport::group_scatter`], the
//! scatter of disjoint group means against `sd/√m` — and the between-seed
//! spread agrees with the internal `σ` (`χ²/dof` inside its 95 % band). If
//! not, the gap is the inter-cycle-correlation finding for #495.
//!
//! **Result: NOT YET MEASURED (testing deferred by maintainer, 2026-10-03).**
//! Protocol: `verification_and_validation/stats_epic_493/494_seed_ensembles.md`.

use raffles::estimators::seed_consistency::{
    group_mean_scatter, seed_consistency, GroupMeanScatter, SeedConsistency,
};
use std::sync::atomic::{AtomicUsize, Ordering};

/// Group size for [`EnsembleReport::group_scatter`], fixed in advance: 32
/// seeds (this workspace's standard) give 8 disjoint groups of 4.
pub const GROUP_SIZE: usize = 4;

/// One seed's result: its estimate and that run's own internal 1σ, in one
/// unit (the caller's — `k`, pcm, a tally unit).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct SeedRun {
    /// The master seed this run used.
    pub seed: u64,
    /// The run's estimate.
    pub value: f64,
    /// The run's own internal standard error (e.g. `KeffResult::k_std`).
    pub sigma: f64,
}

/// Worker threads from the machine, `available_parallelism` or 4.
///
/// The result never depends on it (see the module docs); only the wall clock
/// does.
pub fn default_workers() -> usize {
    std::thread::available_parallelism()
        .map(|n| n.get())
        .unwrap_or(4)
}

/// Run `run` once per seed on `workers` threads, returning results in seed
/// order. See [`run_seeds_with_progress`].
pub fn run_seeds<F>(seeds: &[u64], workers: usize, run: F) -> Vec<SeedRun>
where
    F: Fn(u64) -> (f64, f64) + Sync,
{
    run_seeds_with_progress(seeds, workers, run, |_| {})
}

/// Run `run` once per seed on `workers` threads; `progress(n)` is called after
/// each seed with the number finished so far (from whichever worker finished
/// it). Results come back in seed order.
///
/// `workers` is clamped to `1..=seeds.len()`. On `wasm32` (no threads) the
/// seeds run serially, in order, with identical results.
pub fn run_seeds_with_progress<F, P>(seeds: &[u64], workers: usize, run: F, progress: P) -> Vec<SeedRun>
where
    F: Fn(u64) -> (f64, f64) + Sync,
    P: Fn(usize) + Sync,
{
    let mut out: Vec<SeedRun> = seeds
        .iter()
        .map(|&seed| SeedRun {
            seed,
            value: f64::NAN,
            sigma: f64::NAN,
        })
        .collect();
    if seeds.is_empty() {
        return out;
    }
    let done = AtomicUsize::new(0);
    #[cfg(target_arch = "wasm32")]
    {
        let _ = workers;
        for slot in out.iter_mut() {
            let (v, s) = run(slot.seed);
            slot.value = v;
            slot.sigma = s;
            progress(done.fetch_add(1, Ordering::Relaxed) + 1);
        }
    }
    #[cfg(not(target_arch = "wasm32"))]
    {
        let chunk = seeds.len().div_ceil(workers.min(seeds.len()).max(1));
        let run = &run;
        let progress = &progress;
        let done = &done;
        std::thread::scope(|s| {
            for out_chunk in out.chunks_mut(chunk) {
                s.spawn(move || {
                    for slot in out_chunk.iter_mut() {
                        let (v, sg) = run(slot.seed);
                        slot.value = v;
                        slot.sigma = sg;
                        progress(done.fetch_add(1, Ordering::Relaxed) + 1);
                    }
                });
            }
        });
    }
    out
}

/// Pooled statistics and the consistency check for a finished ensemble.
#[derive(Debug, Clone, PartialEq)]
pub struct EnsembleReport {
    /// The per-seed results, in seed order.
    pub runs: Vec<SeedRun>,
    /// Unweighted pooled mean, `raffles::estimators::pooled` — the number this
    /// crate's ensembles have always quoted.
    pub mean: f64,
    /// Seed-to-seed sample sd: what ONE run scatters by.
    pub sd: f64,
    /// `sd/√N`: the uncertainty on [`Self::mean`].
    pub sem: f64,
    /// `χ²/dof` of the seeds against their internal `σ`, verdict and outliers.
    /// `None` for fewer than two seeds or any non-positive internal `σ`.
    pub consistency: Option<SeedConsistency>,
    /// Disjoint-group-mean scatter at [`GROUP_SIZE`] (the measured `1/√N`
    /// check). `None` with fewer than three groups.
    pub group_scatter: Option<GroupMeanScatter>,
}

impl EnsembleReport {
    /// Pool `runs`.
    pub fn from_runs(runs: Vec<SeedRun>) -> Self {
        let values: Vec<f64> = runs.iter().map(|r| r.value).collect();
        let sigmas: Vec<f64> = runs.iter().map(|r| r.sigma).collect();
        let (mean, sd, sem) = raffles::estimators::pooled(&values);
        let consistency = seed_consistency(&values, &sigmas).ok();
        let group_scatter = group_mean_scatter(&values, GROUP_SIZE);
        Self {
            runs,
            mean,
            sd,
            sem,
            consistency,
            group_scatter,
        }
    }

    /// The per-seed values, in seed order.
    pub fn values(&self) -> Vec<f64> {
        self.runs.iter().map(|r| r.value).collect()
    }

    /// A multi-line report of the consistency check, in `unit`. Report-only.
    pub fn consistency_summary(&self, unit: &str) -> String {
        let mut s = String::new();
        match &self.consistency {
            Some(c) => {
                s.push_str(&format!(
                    "  internal sigma  rms {:.4} {unit}; seed-to-seed sd / internal = {:.2}\n",
                    c.rms_internal_sigma, c.spread_ratio
                ));
                s.push_str(&format!(
                    "  chi2/dof        {:.2} (dof {}), 95 % band [{:.2}, {:.2}], p(>=) {:.3} => {:?}\n",
                    c.chi2_per_dof, c.dof, c.band_95.0, c.band_95.1, c.p_value_upper, c.verdict
                ));
                if c.outliers.is_empty() {
                    s.push_str(&format!(
                        "  outlier seeds   none (|z| > {:.2}, Bonferroni 5 %)\n",
                        c.outlier_threshold
                    ));
                } else {
                    for o in &c.outliers {
                        s.push_str(&format!(
                            "  outlier seed    #{} (seed {}) value {:.4} {unit}, z {:+.2} -- inspect, do not drop\n",
                            o.index, self.runs[o.index].seed, o.value, o.z
                        ));
                    }
                }
            }
            None => s.push_str("  chi2/dof        not computable (needs >= 2 seeds with sigma > 0)\n"),
        }
        match &self.group_scatter {
            Some(g) => s.push_str(&format!(
                "  1/sqrt(N) check {} groups of {}: observed sd of group means / (sd/sqrt(m)) = {:.2} +/- {:.2}\n",
                g.n_groups, g.group_size, g.ratio, g.ratio_sigma
            )),
            None => s.push_str("  1/sqrt(N) check needs >= 3 groups of 4 seeds\n"),
        }
        s
    }
}

/// Seeds `1..=n`, the convention `godiva_keff_ensemble` uses.
pub fn consecutive_seeds(n: usize) -> Vec<u64> {
    (1..=n as u64).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    /// **Methodology.** A deterministic closure (value = seed², σ = 1) run on
    /// 1, 3 and 7 workers must give identical per-seed vectors in seed order:
    /// the thread-count independence the migrated example relied on.
    ///
    /// **Result:** NOT YET MEASURED (testing deferred by maintainer,
    /// 2026-10-03).
    #[test]
    fn results_are_worker_count_independent_and_in_order() {
        let seeds = consecutive_seeds(10);
        let f = |s: u64| ((s * s) as f64, 1.0);
        let a = run_seeds(&seeds, 1, f);
        let b = run_seeds(&seeds, 3, f);
        let c = run_seeds(&seeds, 7, f);
        assert_eq!(a, b);
        assert_eq!(a, c);
        assert_eq!(a[9].seed, 10);
        assert_eq!(a[9].value, 100.0);
        let count = AtomicUsize::new(0);
        let _ = run_seeds_with_progress(&seeds, 4, f, |_| {
            count.fetch_add(1, Ordering::Relaxed);
        });
        assert_eq!(count.load(Ordering::Relaxed), 10);
    }

    /// **Methodology.** The report's pooled mean/sd/sem must be exactly
    /// `raffles::estimators::pooled` — the formula the per-example copies used
    /// — so migrating an example cannot move a printed number.
    ///
    /// **Result:** NOT YET MEASURED (testing deferred by maintainer,
    /// 2026-10-03).
    #[test]
    fn pooled_numbers_are_the_shared_formula() {
        let runs: Vec<SeedRun> = (0..12)
            .map(|i| SeedRun {
                seed: i,
                value: ((i * 37) % 11) as f64,
                sigma: 3.0,
            })
            .collect();
        let r = EnsembleReport::from_runs(runs.clone());
        let v: Vec<f64> = runs.iter().map(|r| r.value).collect();
        assert_eq!((r.mean, r.sd, r.sem), raffles::estimators::pooled(&v));
        assert!(r.consistency.is_some());
        assert_eq!(r.group_scatter.unwrap().n_groups, 3);
        assert!(r.consistency_summary("pcm").contains("chi2/dof"));
    }
}
