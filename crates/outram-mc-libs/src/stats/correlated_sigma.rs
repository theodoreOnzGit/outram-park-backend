//! **Autocorrelation-corrected uncertainties for `k_eff` and tallies** —
//! GitHub #495.
//!
//! # Why the reported `σ` is too small
//!
//! Power iteration feeds generation `g`'s fission sites into generation
//! `g + 1` as its source, so successive generation estimates of `k` (and of
//! every tally) are positively correlated. The `k_std` this crate's drivers
//! report is the textbook `s/√n` over the active generations
//! (`raffles::estimators::mean_and_stderr`), which assumes independence and
//! therefore **under-states** the true uncertainty by `sqrt(τ_int)`. The
//! effect is largest in loosely coupled cores — a 1.8 m pebble bed such as the
//! HTR-10 — where the dominance ratio is close to 1 (Ueki et al. 2004).
//!
//! # What this adds, and what it never does
//!
//! [`KeffUncertainty`] reads a finished
//! [`KeffResult`](crate::physics::keff::KeffResult) and reports the corrected
//! `σ` by batch means (default batch `⌊√n⌋`, fixed) and by the integrated
//! autocorrelation time / effective sample size — **next to the naive
//! `k_std`, which it carries unchanged**. It never writes `k_std` and no
//! driver calls it, so no recorded result moves.
//!
//! For tallies, the drivers accumulate running sums and keep no per-batch
//! history, so a correlated `σ` needs the per-batch realisations recorded by
//! the caller. [`TallyBatchRecorder`] does that by differencing a
//! [`Tally`](crate::tally::tally::Tally)'s bin sums between batches. **No
//! driver exposes a per-batch hook yet**, so today it serves a caller that
//! steps batches itself; wiring a report-only hook into the drivers is a
//! follow-up (filed against #495), not done here, because it would touch the
//! drivers' inner loop.
//!
//! # V&V gate (from #495, fixed in advance)
//!
//! On a loosely coupled case (HTR-10 at 10k × [5 + 135]) the corrected `σ`
//! must agree with the seed-to-seed spread from the ensemble (#494); the naive
//! `σ` is expected to under-predict it, and the result is recorded either
//! way. Agreement criterion, fixed here before measuring: the ratio
//! `seed-to-seed sd / mean corrected σ` lies within `1 ± 2·u`, with `u` the
//! combined relative uncertainty of the two `σ` estimates
//! (`u² = 1/(2(N_seeds − 1)) + 1/(2(a − 1))/N_seeds`, `a` the number of
//! batches).
//!
//! **Result: NOT YET MEASURED (testing deferred by maintainer, 2026-10-03).**
//! Protocol: `verification_and_validation/stats_epic_493/495_correlated_sigma.md`.

use crate::physics::keff::KeffResult;
use crate::tally::tally::Tally;
use raffles::estimators::autocorrelation::CorrelatedMean;

/// The naive and corrected uncertainties of a run's `k_eff`.
#[derive(Debug, Clone, PartialEq)]
pub struct KeffUncertainty {
    /// Active generations the estimate covers.
    pub n_active: usize,
    /// The run's own `k_mean`, carried unchanged.
    pub k_mean: f64,
    /// The run's own `k_std` (naive, assumes independent generations),
    /// carried unchanged.
    pub naive_sigma: f64,
    /// Batch means, autocorrelation time and ESS over the active generations.
    pub estimate: CorrelatedMean,
}

impl KeffUncertainty {
    /// Analyse the active part of `r.k_by_generation`, i.e. everything after
    /// the first `n_inactive` generations. `None` with fewer than four active
    /// generations.
    pub fn from_result(r: &KeffResult, n_inactive: usize) -> Option<Self> {
        Self::from_parts(&r.k_by_generation, r.k_mean, r.k_std, n_inactive)
    }

    /// As [`Self::from_result`], from the three fields it reads — for results
    /// held in another form (a state point, a CSV).
    pub fn from_parts(
        k_by_generation: &[f64],
        k_mean: f64,
        k_std: f64,
        n_inactive: usize,
    ) -> Option<Self> {
        let k = k_by_generation;
        let active = &k[n_inactive.min(k.len())..];
        if active.len() < 4 {
            return None;
        }
        Some(Self {
            n_active: active.len(),
            k_mean,
            naive_sigma: k_std,
            estimate: CorrelatedMean::estimate(active),
        })
    }

    /// Batch-means `σ`, if at least two batches fit.
    pub fn batch_sigma(&self) -> Option<f64> {
        self.estimate.batch.map(|b| b.sem)
    }

    /// One report line per estimator, in pcm. Report-only.
    pub fn summary(&self) -> String {
        let mut s = format!(
            "  k sigma (pcm): naive {:.1}  [{} active generations, assumes independence]\n",
            self.naive_sigma * 1e5,
            self.n_active
        );
        match self.estimate.batch {
            Some(b) => s.push_str(&format!(
                "                 batch means {:.1} +/- {:.0}%  ({} batches of {}, {} leading dropped)  x{:.2} naive\n",
                b.sem * 1e5,
                100.0 * b.sem_relative_uncertainty,
                b.n_batches,
                b.batch_size,
                b.dropped_leading,
                if self.naive_sigma > 0.0 { b.sem / self.naive_sigma } else { f64::NAN }
            )),
            None => s.push_str("                 batch means: too few generations\n"),
        }
        match (self.estimate.tau, self.estimate.ess_sem) {
            (Some(t), Some(e)) => s.push_str(&format!(
                "                 tau_int {:.2} (window {}{}), ESS {:.0}, sigma_ESS {:.1}\n",
                t.tau_int,
                t.window,
                if t.window_converged { "" } else { ", NOT converged: lower bound" },
                t.effective_sample_size,
                e * 1e5
            )),
            _ => s.push_str("                 tau_int: not computable\n"),
        }
        s
    }
}

/// Records per-batch realisations of every bin of a [`Tally`] by differencing
/// its running sums between batches, so a correlated `σ` can be computed per
/// bin.
///
/// Call [`Self::new`] on the tally before the first active batch and
/// [`Self::record`] after each active batch. Nothing here mutates the tally.
#[derive(Debug, Clone, PartialEq)]
pub struct TallyBatchRecorder {
    previous: Vec<f64>,
    /// `series[bin][batch]`: that bin's score in that batch.
    pub series: Vec<Vec<f64>>,
}

impl TallyBatchRecorder {
    /// Snapshot `tally`'s current bin sums as the baseline.
    pub fn new(tally: &Tally) -> Self {
        Self {
            previous: tally.bins.iter().map(|b| b.sum).collect(),
            series: vec![Vec::new(); tally.bins.len()],
        }
    }

    /// Append one batch: each bin's `sum` minus its value at the previous
    /// snapshot.
    ///
    /// # Errors
    ///
    /// The tally's bin count changed since [`Self::new`].
    pub fn record(&mut self, tally: &Tally) -> Result<(), String> {
        if tally.bins.len() != self.previous.len() {
            return Err(format!(
                "tally has {} bins, the recorder was built for {}",
                tally.bins.len(),
                self.previous.len()
            ));
        }
        for (i, b) in tally.bins.iter().enumerate() {
            self.series[i].push(b.sum - self.previous[i]);
            self.previous[i] = b.sum;
        }
        Ok(())
    }

    /// Naive and corrected uncertainty of bin `bin`'s per-batch mean.
    pub fn bin_estimate(&self, bin: usize) -> Option<CorrelatedMean> {
        self.series.get(bin).map(|s| CorrelatedMean::estimate(s))
    }
}

/// Corrected uncertainty of any per-batch series a caller already has (a
/// tally bin, a leakage, a derived quantity) — the series route when no
/// [`Tally`] is involved.
pub fn series_estimate(per_batch: &[f64]) -> CorrelatedMean {
    CorrelatedMean::estimate(per_batch)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tally::tally::{ScoreType, TallyBin};

    /// **Methodology.** The naive `σ` is carried bit for bit from the
    /// `KeffResult`, never recomputed, and inactive generations are excluded.
    ///
    /// **Result:** NOT YET MEASURED (testing deferred by maintainer,
    /// 2026-10-03).
    #[test]
    fn naive_sigma_is_carried_unchanged() {
        let k: Vec<f64> = (0..50).map(|g| 1.0 + 1e-3 * ((g * 7) % 5) as f64).collect();
        let (k_mean, k_std) = (1.0021, 0.000_123_456_789_f64);
        let u = KeffUncertainty::from_parts(&k, k_mean, k_std, 10).unwrap();
        assert_eq!(u.naive_sigma.to_bits(), k_std.to_bits());
        assert_eq!(u.k_mean.to_bits(), k_mean.to_bits());
        assert_eq!(u.n_active, 40);
        assert!(u.batch_sigma().is_some());
        assert!(u.summary().contains("naive"));
        assert!(KeffUncertainty::from_parts(&k, k_mean, k_std, 48).is_none());
    }

    /// **Methodology.** Differencing bookkeeping: three batches scoring 1, 2,
    /// 4 into one bin must be recorded as `[1, 2, 4]`.
    ///
    /// **Result:** NOT YET MEASURED (testing deferred by maintainer,
    /// 2026-10-03).
    #[test]
    fn recorder_differences_running_sums() {
        let mut t = Tally {
            id: 1,
            name: "t".into(),
            filters: Vec::new(),
            scores: vec![ScoreType::Flux],
            bins: vec![TallyBin::default()],
        };
        let mut rec = TallyBatchRecorder::new(&t);
        for v in [1.0, 2.0, 4.0] {
            t.bins[0].score(v);
            rec.record(&t).unwrap();
        }
        assert_eq!(rec.series[0], vec![1.0, 2.0, 4.0]);
        assert_eq!(rec.bin_estimate(0).unwrap().n, 3);
        t.bins.push(TallyBin::default());
        assert!(rec.record(&t).is_err());
    }
}
