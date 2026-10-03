//! **Source-convergence diagnostics and an inactive-cycle recommendation** —
//! GitHub #496.
//!
//! # The question
//!
//! How many inactive generations does *this* case need? The usual answer is a
//! number copied from somewhere else — the HTR-10 reference used 5 on a 1.8 m
//! pebble core, which `htr10_rmc_keff.rs` flags as a possible bias. A source
//! still relaxing while `k` is scored is a **bias**, not a variance: it does
//! not shrink with more seeds, and no `σ` reports it.
//!
//! # What this does
//!
//! [`SourceConvergence::from_result`] reads a finished
//! [`KeffResult`](crate::physics::keff::KeffResult) — its Shannon-entropy
//! trace (when an entropy mesh was supplied) and its `k` trace, **all**
//! generations including the inactive ones, since that is where the
//! transient is — and runs three independent tests from
//! [`raffles::estimators::stationarity`] on each:
//!
//! 1. **MSER-5 truncation** — where the transient ends; fails (trace "not
//!    converged") if the optimum lies in the second half of the trace.
//! 2. **Geweke two-window test** on what remains after that truncation, with
//!    batch-means errors — `|z| > 1.96` is drift.
//! 3. **A Bayesian single change-point** — where the mean most probably
//!    shifted, and how strongly BIC favours a shift at all.
//!
//! The **recommended inactive count** is the larger of the entropy and `k`
//! MSER truncations, and is given only when every analysed trace passes
//! (MSER optimum in the first half and no Geweke drift after it). Otherwise
//! the recommendation is `None` with the failing evidence — a number is never
//! invented for a trace that has not settled.
//!
//! Entropy is the primary signal (it sees the spatial shape of the source,
//! which `k` integrates away); `k` is analysed too, because a case with no
//! entropy mesh has nothing else.
//!
//! # Report-only
//!
//! Nothing here changes transport or any result. No driver calls it; the
//! caller decides what to do with a recommendation, and changing `n_inactive`
//! is a new run.
//!
//! # V&V gate (from #496, fixed in advance)
//!
//! On a case with a deliberately bad initial source, the diagnostic must flag
//! non-convergence where entropy is still drifting and pass once it is flat;
//! the recommended count is compared against a long reference run.
//!
//! **Result: NOT YET MEASURED (testing deferred by maintainer, 2026-10-03).**
//! Protocol: `verification_and_validation/stats_epic_493/496_source_convergence.md`.

use crate::physics::keff::KeffResult;
use raffles::estimators::stationarity::{
    change_point, geweke, mser_truncation, ChangePoint, GewekeTest, MserTruncation, MSER_BATCH,
};

/// The three tests on one trace.
#[derive(Debug, Clone, PartialEq)]
pub struct TraceDiagnosis {
    /// `"entropy"` or `"k"`.
    pub name: &'static str,
    /// Generations in the trace.
    pub n: usize,
    /// MSER-5 truncation; `None` if the trace is too short (< 20 values).
    pub mser: Option<MserTruncation>,
    /// Geweke on the trace after the MSER truncation.
    pub geweke_after_truncation: Option<GewekeTest>,
    /// Change-point posterior over the whole trace (batch means of 5).
    pub change_point: Option<ChangePoint>,
}

impl TraceDiagnosis {
    /// Run the three tests on `trace`.
    pub fn analyse(name: &'static str, trace: &[f64]) -> Self {
        let mser = mser_truncation(trace, MSER_BATCH);
        let geweke_after_truncation = mser
            .as_ref()
            .and_then(|m| geweke(&trace[m.truncation.min(trace.len())..]));
        Self {
            name,
            n: trace.len(),
            mser,
            geweke_after_truncation,
            change_point: change_point(trace, MSER_BATCH),
        }
    }

    /// Converged: MSER found its optimum in the first half **and** Geweke sees
    /// no drift after it. `false` when either test could not run.
    pub fn converged(&self) -> bool {
        match (&self.mser, &self.geweke_after_truncation) {
            (Some(m), Some(g)) => m.in_first_half && !g.drift,
            _ => false,
        }
    }

    /// The generations to discard by this trace's MSER rule, when converged.
    pub fn discard(&self) -> Option<usize> {
        if self.converged() {
            self.mser.as_ref().map(|m| m.truncation)
        } else {
            None
        }
    }

    fn summary_line(&self) -> String {
        let mser = match &self.mser {
            Some(m) => format!(
                "MSER-5 discard {}{}",
                m.truncation,
                if m.in_first_half { "" } else { " (SECOND HALF: not converged)" }
            ),
            None => "MSER-5 n/a (too short)".into(),
        };
        let gw = match &self.geweke_after_truncation {
            Some(g) => format!(
                "Geweke z {:+.2}{}{}",
                g.z,
                if g.drift { " DRIFT" } else { "" },
                if g.batch_errors { "" } else { " (naive errors)" }
            ),
            None => "Geweke n/a".into(),
        };
        let cp = match &self.change_point {
            Some(c) => format!(
                "change-point at gen ~{} (ln B {:+.1}, {})",
                c.map_sample,
                c.ln_bayes_factor_change,
                c.strength.as_str()
            ),
            None => "change-point n/a".into(),
        };
        format!(
            "    {:<7} [{} gens] {mser}; {gw}; {cp} => {}\n",
            self.name,
            self.n,
            if self.converged() { "converged" } else { "NOT converged" }
        )
    }
}

/// Source-convergence evidence for one run, and a recommendation.
#[derive(Debug, Clone, PartialEq)]
pub struct SourceConvergence {
    /// The inactive count the run actually used.
    pub n_inactive_used: usize,
    /// Entropy trace diagnosis; `None` when the run carried no entropy.
    pub entropy: Option<TraceDiagnosis>,
    /// `k` trace diagnosis.
    pub k: TraceDiagnosis,
    /// `max` of the MSER discards over the analysed traces, only if every
    /// analysed trace converged.
    pub recommended_inactive: Option<usize>,
}

impl SourceConvergence {
    /// Analyse `r`, which ran with `n_inactive` inactive generations.
    pub fn from_result(r: &KeffResult, n_inactive: usize) -> Self {
        Self::from_traces(&r.k_by_generation, &r.entropy, n_inactive)
    }

    /// As [`Self::from_result`], from the two traces (an empty `entropy`
    /// means none was computed).
    pub fn from_traces(k_by_generation: &[f64], entropy: &[f64], n_inactive: usize) -> Self {
        let k = TraceDiagnosis::analyse("k", k_by_generation);
        let entropy = if entropy.is_empty() {
            None
        } else {
            Some(TraceDiagnosis::analyse("entropy", entropy))
        };
        let mut rec = k.discard();
        if let Some(e) = &entropy {
            rec = match (rec, e.discard()) {
                (Some(a), Some(b)) => Some(a.max(b)),
                _ => None,
            };
        }
        Self {
            n_inactive_used: n_inactive,
            entropy,
            k,
            recommended_inactive: rec,
        }
    }

    /// `Some(true)` when a recommendation exists and the run discarded at
    /// least that many generations; `Some(false)` when it discarded fewer;
    /// `None` when there is no recommendation.
    pub fn inactive_sufficient(&self) -> Option<bool> {
        self.recommended_inactive.map(|r| self.n_inactive_used >= r)
    }

    /// Multi-line report. Report-only.
    pub fn summary(&self, label: &str) -> String {
        let mut s = format!("  [source convergence, #496] {label}\n");
        match &self.entropy {
            Some(e) => s.push_str(&e.summary_line()),
            None => s.push_str("    entropy NOT COMPUTED for this run (no entropy mesh)\n"),
        }
        s.push_str(&self.k.summary_line());
        match (self.recommended_inactive, self.inactive_sufficient()) {
            (Some(r), Some(ok)) => s.push_str(&format!(
                "    => recommended inactive >= {r}; this run used {} ({})\n",
                self.n_inactive_used,
                if ok { "sufficient" } else { "TOO FEW" }
            )),
            _ => s.push_str(
                "    => no recommendation: at least one trace has not settled; run longer\n",
            ),
        }
        s
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// An entropy-like trace that rises from `h0` to `h_inf` with time
    /// constant `tau`, plus deterministic small noise from the PETIR LCG.
    fn relaxing(n: usize, h0: f64, h_inf: f64, tau: f64, seed: u64) -> Vec<f64> {
        let mut s = seed;
        (0..n)
            .map(|g| {
                h_inf + (h0 - h_inf) * (-(g as f64) / tau).exp()
                    + 0.01 * raffles::distributions::seeded::sample_normal(&mut s)
            })
            .collect()
    }

    /// **Methodology.** A synthetic "bad initial source": entropy rising from
    /// 2 to 6 bits with a 15-generation time constant. Over 300 generations
    /// the trace settles, so it must be judged converged with a discard
    /// between 30 and 150 (fixed band around `15 ln(4/0.01) ≈ 90`); a run that
    /// used 20 inactive must be reported as too few. Cut at 40 generations
    /// (still drifting) it must NOT be judged converged.
    ///
    /// **Result:** NOT YET MEASURED (testing deferred by maintainer,
    /// 2026-10-03).
    #[test]
    fn drifting_entropy_is_flagged_and_settled_entropy_passes() {
        let h = relaxing(300, 2.0, 6.0, 15.0, 9);
        let k = relaxing(300, 0.9, 1.0, 5.0, 10);
        let c = SourceConvergence::from_traces(&k, &h, 20);
        let e = c.entropy.as_ref().unwrap();
        assert!(e.converged(), "{}", c.summary("settled"));
        let r = c.recommended_inactive.unwrap();
        assert!((30..=150).contains(&r), "recommended {r}");
        assert_eq!(c.inactive_sufficient(), Some(false));

        let c = SourceConvergence::from_traces(&k[..40], &h[..40], 20);
        assert!(!c.entropy.as_ref().unwrap().converged(), "{}", c.summary("cut"));
        assert_eq!(c.recommended_inactive, None);
    }

    /// No entropy: the `k` trace alone decides, and the report says entropy
    /// was not computed.
    #[test]
    fn missing_entropy_is_reported() {
        let k = relaxing(200, 1.0, 1.0, 1.0, 3);
        let c = SourceConvergence::from_traces(&k, &[], 50);
        assert!(c.entropy.is_none());
        assert!(c.summary("x").contains("NOT COMPUTED"));
    }
}
