//! **Surrogate-accelerated parameter sweeps** — GitHub #499.
//!
//! # The rule that shapes this module
//!
//! **A surrogate value is never reported as a transport result.** It is a
//! [`SurrogatePrediction`] — its own type, with an interval, a distance to the
//! nearest real run and no `σ` field that could be mistaken for a Monte
//! Carlo standard error — never a `KeffResult` or a bare `f64`. Every sweep
//! keeps its full runs, and the surrogate's leave-one-out residuals on them
//! are its V&V.
//!
//! # What it does
//!
//! - [`SweepSurrogate::fit`]: a `raffles` [`PolynomialSurrogate`] of fixed
//!   degree through the full runs, with the `n` leave-one-out refits
//!   ([`raffles::surrogate::validation::LeaveOneOut`]).
//! - [`SweepSurrogate::predict`]: the full-data fit plus a **jackknife+**
//!   interval (coverage ≥ `1 − 2α` with no assumption on the polynomial being
//!   the right shape; Barber et al. 2021). The interval covers a new *noisy
//!   run* at `x`, so it can be no narrower than the run-to-run scatter.
//! - [`SweepSurrogate::loo_gate`]: the held-out comparison #499 asks for.
//! - [`SweepSurrogate::propose_next_runs`]: active sampling — rank candidate
//!   inputs by the spread of the leave-one-out models' predictions there
//!   (where the fit is least stable), weighted by distance to existing and
//!   already-proposed runs so proposals spread out. A heuristic, documented
//!   as one; it decides where to spend full runs, never what a result is.
//!
//! A categorical input (e.g. the nuclear-data library in the HTR-10 sweep)
//! is not a polynomial variable: fit one surrogate per category.
//!
//! # V&V gate (from #499, fixed in advance), and a caveat found writing it
//!
//! On the 22 runs of `nee_soon/verification_and_validation/htr10_seker_2026_10_01_10k`
//! (11 heights × 2 libraries, one surrogate per library): leave-out
//! cross-validation error within the per-run MC `σ`, i.e.
//! `loo_rmse ≤ rms(σ_i)`.
//!
//! **Caveat, stated before any measurement:** a held-out residual contains
//! the held-out run's own noise (variance `σ²`) *plus* the refit's error at
//! that point, so even a perfect surrogate has an expected LOO RMS of about
//! `σ·sqrt(1 + p/n)` (`p` coefficients) — **above** `σ`. Read literally, the
//! gate is expected to fail for a correct surrogate. [`LooGate`] therefore
//! reports the literal comparison, the noise-floor-aware one
//! (`χ²` per point against `1 + p/(n − p)`), and both verdicts; which one the
//! gate means is the maintainer's call, flagged on #499, not decided here.
//!
//! **Result: NOT YET MEASURED (testing deferred by maintainer, 2026-10-03).**
//! Protocol: `verification_and_validation/stats_epic_493/499_surrogate_sweeps.md`;
//! runner: `examples/stats_sweep_loo.rs` (reads the committed CSV, no
//! transport).

use raffles::surrogate::validation::LeaveOneOut;
use raffles::surrogate::PolynomialSurrogate;

/// One full transport run in a sweep: inputs, value, its MC `σ`.
#[derive(Debug, Clone, PartialEq)]
pub struct SweepRun {
    /// Input coordinates (e.g. `[height_cm]`).
    pub x: Vec<f64>,
    /// The transport result.
    pub value: f64,
    /// Its Monte Carlo 1σ.
    pub sigma: f64,
}

/// A surrogate's estimate at an input. **Not a transport result.**
#[derive(Debug, Clone, PartialEq)]
pub struct SurrogatePrediction {
    /// Where.
    pub x: Vec<f64>,
    /// The full-data polynomial at `x`.
    pub value: f64,
    /// Jackknife+ interval at miscoverage `alpha` (coverage ≥ `1 − 2α`);
    /// a bound is infinite when there are too few runs for it.
    pub interval: (f64, f64),
    /// The `α` used.
    pub alpha: f64,
    /// Euclidean distance from `x` to the nearest full run, in input units —
    /// a reader's cue for interpolation versus extrapolation.
    pub nearest_run_distance: f64,
}

/// The held-out comparison #499 asks for, both readings.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct LooGate {
    /// Number of full runs.
    pub n: usize,
    /// Polynomial coefficients `p`.
    pub terms: usize,
    /// RMS leave-one-out residual.
    pub loo_rmse: f64,
    /// RMS of the runs' own `σ`.
    pub rms_sigma: f64,
    /// Literal gate: `loo_rmse ≤ rms_sigma`.
    pub literal_pass: bool,
    /// `Σ (r_i/σ_i)² / n`.
    pub chi2_per_point: f64,
    /// Expected `chi2_per_point` for a perfect surrogate, `1 + p/(n − p)`
    /// (the noise floor; infinite when `n ≤ p`).
    pub noise_floor: f64,
    /// Noise-floor gate: `chi2_per_point / noise_floor` within the two-sided
    /// 95 % band of `χ²_n / n`. Approximate: LOO residuals are not
    /// independent, so the band is a guide, not an exact test.
    pub floor_aware_pass: bool,
}

/// A fitted sweep surrogate with its leave-one-out validation.
#[derive(Debug, Clone, PartialEq)]
pub struct SweepSurrogate {
    /// The full runs, as given.
    pub runs: Vec<SweepRun>,
    /// Full fit and LOO refits.
    pub loo: LeaveOneOut,
}

impl SweepSurrogate {
    /// Fit at total degree `degree` and ridge `ridge` (fixed for every
    /// refit).
    ///
    /// # Errors
    ///
    /// Fewer than three runs, ragged inputs, a non-positive `σ`, or a fit the
    /// polynomial module refuses.
    pub fn fit(runs: Vec<SweepRun>, degree: usize, ridge: f64) -> Result<Self, String> {
        if runs.iter().any(|r| !(r.sigma > 0.0)) {
            return Err("every run needs a positive MC sigma".into());
        }
        let xs: Vec<Vec<f64>> = runs.iter().map(|r| r.x.clone()).collect();
        let ys: Vec<f64> = runs.iter().map(|r| r.value).collect();
        let loo = LeaveOneOut::fit(&xs, &ys, degree, ridge).map_err(|e| e.to_string())?;
        Ok(Self { runs, loo })
    }

    /// The full-data polynomial.
    pub fn model(&self) -> &PolynomialSurrogate {
        &self.loo.full
    }

    /// Predict at `x` with a jackknife+ interval at miscoverage `alpha`.
    ///
    /// # Errors
    ///
    /// Wrong input length or `alpha` outside `(0, 0.5)`.
    pub fn predict(&self, x: &[f64], alpha: f64) -> Result<SurrogatePrediction, String> {
        let value = self.loo.full.predict(x).map_err(|e| e.to_string())?;
        let interval = self.loo.jackknife_plus(x, alpha).map_err(|e| e.to_string())?;
        Ok(SurrogatePrediction {
            x: x.to_vec(),
            value,
            interval,
            alpha,
            nearest_run_distance: self
                .runs
                .iter()
                .map(|r| distance(&r.x, x))
                .fold(f64::INFINITY, f64::min),
        })
    }

    /// The held-out gate, both readings (see the module docs).
    pub fn loo_gate(&self) -> LooGate {
        let n = self.runs.len();
        let terms = self.loo.full.terms();
        let sigmas: Vec<f64> = self.runs.iter().map(|r| r.sigma).collect();
        let rms_sigma = (sigmas.iter().map(|s| s * s).sum::<f64>() / n as f64).sqrt();
        let loo_rmse = self.loo.rmse();
        let chi2_per_point = self.loo.chi2_per_point(&sigmas).unwrap_or(f64::NAN);
        let noise_floor = if n > terms {
            1.0 + terms as f64 / (n - terms) as f64
        } else {
            f64::INFINITY
        };
        let (lo, hi) = raffles::estimators::seed_consistency::chi2_per_dof_band(n, 0.95);
        let scaled = chi2_per_point / noise_floor;
        LooGate {
            n,
            terms,
            loo_rmse,
            rms_sigma,
            literal_pass: loo_rmse <= rms_sigma,
            chi2_per_point,
            noise_floor,
            floor_aware_pass: scaled >= lo && scaled <= hi,
        }
    }

    /// Indices into `candidates` of up to `count` proposed next full runs.
    ///
    /// Score of a candidate: the sample sd of the `n` leave-one-out models'
    /// predictions there, times the distance to the nearest existing or
    /// already-proposed run (so a second proposal does not land on the
    /// first). Greedy, highest score first; candidates at an existing run
    /// score zero and are never proposed.
    pub fn propose_next_runs(&self, candidates: &[Vec<f64>], count: usize) -> Vec<usize> {
        let spread: Vec<f64> = candidates
            .iter()
            .map(|c| {
                let p: Vec<f64> = self
                    .loo
                    .models
                    .iter()
                    .filter_map(|m| m.predict(c).ok())
                    .collect();
                raffles::estimators::pooled(&p).1
            })
            .collect();
        let mut taken: Vec<Vec<f64>> = self.runs.iter().map(|r| r.x.clone()).collect();
        let mut chosen = Vec::new();
        for _ in 0..count {
            let mut best: Option<(usize, f64)> = None;
            for (i, c) in candidates.iter().enumerate() {
                if chosen.contains(&i) {
                    continue;
                }
                let d = taken.iter().map(|t| distance(t, c)).fold(f64::INFINITY, f64::min);
                let score = spread[i] * d;
                if score > 0.0 && best.is_none_or(|(_, b)| score > b) {
                    best = Some((i, score));
                }
            }
            match best {
                Some((i, _)) => {
                    chosen.push(i);
                    taken.push(candidates[i].clone());
                }
                None => break,
            }
        }
        chosen
    }
}

fn distance(a: &[f64], b: &[f64]) -> f64 {
    a.iter().zip(b).map(|(p, q)| (p - q).powi(2)).sum::<f64>().sqrt()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn runs(f: impl Fn(f64) -> f64) -> Vec<SweepRun> {
        (0..11)
            .map(|i| {
                let h = 100.0 + 10.0 * i as f64;
                SweepRun {
                    x: vec![h],
                    value: f(h),
                    sigma: 1e-3,
                }
            })
            .collect()
    }

    /// **Methodology.** Noise-free quadratic data at 11 heights, degree 2:
    /// LOO residuals are round-off, so the literal gate passes; a prediction
    /// between runs returns its interval and a nearest-run distance of 5.
    ///
    /// **Result:** NOT YET MEASURED (testing deferred by maintainer,
    /// 2026-10-03).
    #[test]
    fn exact_quadratic_sweep() {
        let s = SweepSurrogate::fit(runs(|h| 0.9 + 2e-3 * h - 4e-6 * h * h), 2, 0.0).unwrap();
        let g = s.loo_gate();
        assert!(g.literal_pass, "{g:?}");
        let p = s.predict(&[155.0], 0.2).unwrap();
        assert!((p.nearest_run_distance - 5.0).abs() < 1e-12);
        assert!(p.interval.0 <= p.value && p.value <= p.interval.1);
    }

    /// **Methodology.** Active sampling on a fit through 11 runs proposes the
    /// requested number of distinct candidates and never a candidate sitting
    /// on an existing run.
    ///
    /// **Result:** NOT YET MEASURED (testing deferred by maintainer,
    /// 2026-10-03).
    #[test]
    fn proposals_are_distinct_and_avoid_existing_runs() {
        let s = SweepSurrogate::fit(runs(|h| (h / 100.0).ln()), 2, 0.0).unwrap();
        let cands: Vec<Vec<f64>> = (0..41).map(|i| vec![95.0 + 2.5 * i as f64]).collect();
        let picks = s.propose_next_runs(&cands, 3);
        assert_eq!(picks.len(), 3);
        for &i in &picks {
            assert!(s.runs.iter().all(|r| (r.x[0] - cands[i][0]).abs() > 1e-9));
        }
        let mut u = picks.clone();
        u.sort_unstable();
        u.dedup();
        assert_eq!(u.len(), 3);
        assert!(SweepSurrogate::fit(
            vec![SweepRun { x: vec![1.0], value: 1.0, sigma: 0.0 }; 3],
            1,
            0.0
        )
        .is_err());
    }
}
