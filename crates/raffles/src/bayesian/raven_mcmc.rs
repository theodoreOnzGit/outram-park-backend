// ---------------------------------------------------------------------------
// Ported from RAVEN (Risk Analysis Virtual ENvironment).
//
//   Upstream project: RAVEN — Idaho National Laboratory
//   Upstream repo:    https://github.com/idaholab/raven
//   Upstream files:   ravenframework/Samplers/MCMC/MCMC.py
//                     ravenframework/Samplers/MCMC/Metropolis.py
//                     ravenframework/Samplers/MCMC/AdaptiveMetropolis.py
//   Upstream commit:  8baaae3a97f1392318074ad6d3f8efc26ca81c7e  (branch devel)
//   Accessed:         2026-11-04
//
//   Copyright 2017 Battelle Energy Alliance, LLC
//   Licensed under the Apache License, Version 2.0 (the "License");
//   you may not use this file except in compliance with the License.
//   You may obtain a copy of the License at
//
//       http://www.apache.org/licenses/LICENSE-2.0
//
//   Unless required by applicable law or agreed to in writing, software
//   distributed under the License is distributed on an "AS IS" BASIS,
//   WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
//   See the License for the specific language governing permissions and
//   limitations under the License.
//
// This Rust translation is part of RAFFLES / Outram Park and is distributed
// under AGPL-3.0-only. Apache-2.0 -> (A)GPLv3 is a ONE-WAY relicensing: this
// file may NOT be contributed back to RAVEN or redistributed under Apache-2.0.
//
// Translation notes — what changed, and what was deliberately not ported
// (Apache-2.0 section 4(b) requires significant changes to be stated):
//
//   * DISPATCH. `MCMC(AdaptiveSampler)` -> `Metropolis` / `AdaptiveMetropolis`
//     becomes one function, `raven_mcmc`, over the `RavenMcmcKind` enum. The
//     sampler-as-job-handler plumbing (`localGenerateInput`,
//     `localFinalizeActualSampling`, `inputInfo`, solution export, the
//     `TargetEvaluation` DataObject) is replaced by a loop that calls the
//     caller's likelihood closure, exactly once per step, in RAVEN's order.
//   * KEPT EXACTLY: the counter convention (step 1 evaluates the initial
//     point), `alpha = min(0, sum ln pdf(new) - ln pdf(cur) + d ln L)` with
//     Python's `min` semantics (a NaN ratio becomes 0, i.e. ACCEPTED), the
//     acceptance `alpha > ln u`, the acceptance rate `(1 + accepted) /
//     counter`, `tuneScalingParam` (PyMC3's table) every `tuneInterval`
//     steps including step 1, the default proposal `Normal(0, 0.2 * prior
//     std)` (Metropolis) and `MVN(0, (2.38^2/d) diag(prior std^2))`
//     (adaptive), `_updateAdaptiveParams` line for line, and the CLAMP of an
//     out-of-bounds proposal onto the bound (`BoundHandling::ClampAsRaven`).
//   * ADDED: `BoundHandling::Reject`, the correct Metropolis rule at a bound
//     (RAVEN's clamp leaves an atom on the bound; see the V&V record).
//   * FIXED: adaptive Metropolis without an initial value. RAVEN builds the
//     ensemble mean from the still-unset values and raises TypeError
//     (AdaptiveMetropolis.py:215); here the initial value is drawn from the
//     prior first and the mean built from it.
//   * RANDOMNESS. RAVEN draws through numpy's MT19937 and scipy's `norm.ppf`,
//     and colours the adaptive proposal with an SVD (`MultivariateNormalPCA`).
//     Here `Draws::Lcg` uses `petir::rng::lcg` (crate rule) and a Cholesky
//     factor — the same distribution, a different stream. `Draws::Replay`
//     consumes recorded increments and uniforms, which is how the port is
//     compared with RAVEN step for step.
//   * NOT PORTED: `<probabilityFunction>` priors, user `<proposal>`
//     distributions, multivariate (ND) priors, `<function>`-dependent
//     variables, multiple chains (RAVEN has none either).
// ---------------------------------------------------------------------------

//! **RAVEN's Metropolis and adaptive Metropolis samplers** (gh:#1193, the
//! RAVEN route for calibration, gh:#1180).
//!
//! A translation of `Samplers/MCMC/`: a random-walk Metropolis chain whose
//! proposal scale is tuned on the fly with PyMC3's acceptance table, and
//! Haario-style adaptive Metropolis, whose Gaussian proposal covariance
//! follows the chain.
//!
//! # How this relates to [`super::MetropolisHastings`]
//!
//! The existing kernel ([`super::mcmc`]) is written from the papers, and with
//! a fixed scale and unbounded priors it is the same Markov kernel as RAVEN's
//! Metropolis with a diagonal Cholesky factor `0.2 σ_i s`. What differs:
//!
//! | | `MetropolisHastings::step` | RAVEN (`raven_mcmc`) |
//! |---|---|---|
//! | proposal | `θ + L z`, any `L` | `θ + s · d`, `d_i ~ N(0, 0.2 σ_i)` (Metropolis) or `d ~ N(0, λΣ)` (adaptive) |
//! | scale | fixed | tuned every `tune_interval` steps (PyMC3 table); adaptive: Robbins–Monro `λ` and ensemble covariance |
//! | target | one `ln_target` | prior and likelihood kept separate; the log-ratio is a sum of per-parameter `ln pdf` differences |
//! | outside the prior's support | proposal rejected (`-inf`) | proposal **clamped onto the bound** and evaluated there |
//! | NaN log-ratio | rejected | `min(0, NaN) = 0`: **accepted** (Python `min`) |
//! | the first step | — | step 1 evaluates the initial point; acceptance rate counts it as accepted |
//!
//! So `raven_mcmc` does not reuse `MetropolisHastings`: the clamp, the
//! separate prior and likelihood, the tuning and the replayable draws are
//! what make it RAVEN's, and bit-identity with RAVEN needs RAVEN's operation
//! order. The prior is the crate's [`IndependentPrior`].
//!
//! # V&V
//!
//! Record: `crates/raffles/verification_and_validation/raven_routes_gh1193.md`.
//! Against RAVEN itself (`Draws::Replay` on RAVEN's recorded draws) and
//! against closed-form posteriors (`Draws::Lcg`): see the tests below.

use petir::mathf::RealMath;
use petir::rng::lcg::prn;

use super::{clamp_open_unit, standard_normal, IndependentPrior};
use crate::distributions::{ContinuousDistribution1D, Distribution};
use crate::{RafflesError, Result};

/// RAVEN's optimum acceptance rate for adaptive Metropolis
/// (`AdaptiveMetropolis._optAlpha`).
pub const RAVEN_OPTIMAL_ACCEPTANCE: f64 = 0.234;

/// RAVEN's default proposal standard deviation, as a fraction of the prior's
/// untruncated standard deviation (`MCMC._stdProposalDefault`).
pub const RAVEN_DEFAULT_PROPOSAL_FRACTION: f64 = 0.2;

/// Which RAVEN sampler.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RavenMcmcKind {
    /// `Samplers/MCMC/Metropolis.py`: one independent Gaussian proposal per
    /// parameter, `N(0, 0.2 σ_prior)`, times the tuned scale.
    Metropolis,
    /// `Samplers/MCMC/AdaptiveMetropolis.py`: one multivariate Gaussian
    /// proposal `N(0, λ Σ)`, `λ` and `Σ` adapted after the burn-in.
    AdaptiveMetropolis {
        /// Steps between covariance updates (RAVEN's `<adaptiveInterval>`,
        /// default 20).
        adaptive_interval: usize,
    },
}

/// What the likelihood closure returns (RAVEN's `<likelihood log="...">`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LikelihoodForm {
    /// The natural log of the likelihood (`log="True"`).
    Log,
    /// The likelihood itself (RAVEN's default, `log="False"`); its log is
    /// taken when the ratio is formed.
    Linear,
}

/// What to do with a proposal outside a prior's support.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BoundHandling {
    /// RAVEN's rule (`Metropolis.py:137-142`, `AdaptiveMetropolis.py:274-285`):
    /// move it onto the bound and evaluate it there. This leaves an atom on
    /// the bound, so the chain does not sample the posterior when the
    /// posterior has mass near a bound (V&V record, A3).
    ClampAsRaven,
    /// Reject the proposal (the Metropolis rule). Not in RAVEN.
    Reject,
}

/// Configuration, with RAVEN's names and defaults.
#[derive(Debug, Clone, PartialEq)]
pub struct RavenMcmcConfig {
    /// Which sampler.
    pub kind: RavenMcmcKind,
    /// Total steps, the initial point included (`<limit>`).
    pub limit: usize,
    /// Leading steps left out of [`RavenMcmcResult::samples`] (`<burnIn>`,
    /// default 0). For adaptive Metropolis it is also where the scale tuning
    /// stops and the adaptation starts.
    pub burn_in: usize,
    /// Tune the proposal scale (`<tune>`, default true).
    pub tune: bool,
    /// Steps between scale tunings (`<tuneInterval>`, default 100).
    pub tune_interval: usize,
    /// The initial point (`<initial>`); `None` draws it from the prior.
    pub initial: Option<Vec<f64>>,
    /// What the likelihood closure returns.
    pub likelihood: LikelihoodForm,
    /// What happens at a prior's bounds.
    pub bounds: BoundHandling,
}

impl RavenMcmcConfig {
    /// RAVEN's `Metropolis` defaults: burn-in 0, tuning on every 100 steps,
    /// a linear likelihood, bounds clamped.
    #[must_use]
    pub fn metropolis(limit: usize) -> Self {
        Self {
            kind: RavenMcmcKind::Metropolis,
            limit,
            burn_in: 0,
            tune: true,
            tune_interval: 100,
            initial: None,
            likelihood: LikelihoodForm::Linear,
            bounds: BoundHandling::ClampAsRaven,
        }
    }

    /// RAVEN's `AdaptiveMetropolis` defaults: as [`metropolis`](Self::metropolis),
    /// adaptive interval 20.
    #[must_use]
    pub fn adaptive_metropolis(limit: usize) -> Self {
        Self {
            kind: RavenMcmcKind::AdaptiveMetropolis {
                adaptive_interval: 20,
            },
            ..Self::metropolis(limit)
        }
    }

    fn validate(&self, d: usize) -> Result<()> {
        let bad = |parameter: &str, value: f64, reason: &str| {
            Err(RafflesError::InvalidParameter {
                parameter: parameter.to_string(),
                value,
                reason: reason.to_string(),
            })
        };
        if self.limit == 0 {
            return bad("limit", 0.0, "at least the initial point is evaluated");
        }
        if self.burn_in >= self.limit {
            // MCMC.py: 'Provided "burnIn" value must be less than "limit" value!'
            return bad(
                "burn_in",
                self.burn_in as f64,
                "must be less than limit (RAVEN's own check)",
            );
        }
        if self.tune && self.tune_interval == 0 {
            return bad("tune_interval", 0.0, "must be positive when tuning");
        }
        if let RavenMcmcKind::AdaptiveMetropolis { adaptive_interval } = self.kind {
            if adaptive_interval == 0 {
                return bad("adaptive_interval", 0.0, "must be positive");
            }
        }
        if let Some(x) = &self.initial {
            if x.len() != d {
                return Err(RafflesError::DimensionMismatch {
                    expected: d,
                    found: x.len(),
                });
            }
        }
        Ok(())
    }
}

/// Where the randomness comes from.
#[derive(Debug, Clone, PartialEq)]
pub enum Draws {
    /// The workspace LCG (`petir::rng::lcg`), from this seed: one standard
    /// normal per parameter for the proposal, then one uniform for the
    /// acceptance, every step after the first.
    Lcg {
        /// Stream state.
        seed: u64,
    },
    /// Recorded draws, consumed in order: for step `k = 2..=limit`,
    /// `increments[k - 2]` is the proposal draw `d` (before the scale) and
    /// `uniforms[k - 2]` the acceptance uniform. This is how the port is
    /// compared with RAVEN; the initial point must then be given.
    Replay {
        /// Proposal draws, one vector of length `d` per step after the first.
        increments: Vec<Vec<f64>>,
        /// Acceptance uniforms, one per step after the first.
        uniforms: Vec<f64>,
    },
}

/// One step of the chain, as RAVEN's loop leaves it.
#[derive(Debug, Clone, PartialEq)]
pub struct RavenStep {
    /// The evaluated point (after the clamp).
    pub proposed: Vec<f64>,
    /// The chain's state after the decision.
    pub current: Vec<f64>,
    /// `min(0, log ratio)`; `None` at step 1.
    pub alpha: Option<f64>,
    /// Whether the proposal was accepted (false at step 1, as RAVEN's
    /// `_accepted` starts).
    pub accepted: bool,
    /// RAVEN's running acceptance rate, `(1 + accepted) / counter`.
    pub accept_rate: f64,
    /// The proposal scale after this step's tuning.
    pub scaling: f64,
    /// Adaptive Metropolis's `λ` after this step's update.
    pub lambda: Option<f64>,
}

/// A finished chain.
#[derive(Debug, Clone, PartialEq)]
pub struct RavenMcmcResult {
    /// Every step, the first included.
    pub steps: Vec<RavenStep>,
    /// The burn-in the run used.
    pub burn_in: usize,
    /// Adaptive Metropolis's final ensemble mean.
    pub ensemble_mean: Option<Vec<f64>>,
    /// Adaptive Metropolis's final ensemble covariance (row-major, `d × d`).
    pub ensemble_cov: Option<Vec<Vec<f64>>>,
    /// Steps whose log ratio was NaN and which RAVEN's `min(0, ·)` therefore
    /// accepted. Non-zero means the likelihood or prior has a hole.
    pub nan_log_ratios: usize,
}

impl RavenMcmcResult {
    /// The chain's states after the burn-in: steps with `counter > burn_in`,
    /// which are the rows RAVEN writes to its solution export
    /// (`MCMC._addToSolutionExport`).
    #[must_use]
    pub fn samples(&self) -> Vec<Vec<f64>> {
        self.steps
            .iter()
            .skip(self.burn_in)
            .map(|s| s.current.clone())
            .collect()
    }

    /// Final acceptance rate.
    #[must_use]
    pub fn accept_rate(&self) -> f64 {
        self.steps.last().map_or(0.0, |s| s.accept_rate)
    }
}

/// PyMC3's scale-tuning table, as RAVEN copies it (`MCMC.tuneScalingParam`).
#[must_use]
pub fn tune_scaling(scale: f64, accept_rate: f64) -> f64 {
    if accept_rate < 0.001 {
        scale * 0.1
    } else if accept_rate < 0.05 {
        scale * 0.5
    } else if accept_rate < 0.2 {
        scale * 0.9
    } else if accept_rate > 0.95 {
        scale * 10.0
    } else if accept_rate > 0.75 {
        scale * 2.0
    } else if accept_rate > 0.5 {
        scale * 1.1
    } else {
        scale
    }
}

/// Python's `min(0.0, x)`: `x` only when `x < 0`, so NaN gives 0.
fn python_min_zero(x: f64) -> f64 {
    if x < 0.0 {
        x
    } else {
        0.0
    }
}

/// A sampler's random stream, either kind.
struct Stream {
    draws: Draws,
    step: usize,
}

impl Stream {
    fn proposal(&mut self, d: usize) -> Result<Vec<f64>> {
        match &mut self.draws {
            Draws::Lcg { seed } => Ok((0..d).map(|_| standard_normal(seed)).collect()),
            Draws::Replay { increments, .. } => {
                let v = increments
                    .get(self.step)
                    .ok_or(RafflesError::DimensionMismatch {
                        expected: self.step + 1,
                        found: increments.len(),
                    })?;
                if v.len() != d {
                    return Err(RafflesError::DimensionMismatch {
                        expected: d,
                        found: v.len(),
                    });
                }
                Ok(v.clone())
            }
        }
    }

    fn uniform(&mut self) -> Result<f64> {
        let u = match &mut self.draws {
            Draws::Lcg { seed } => clamp_open_unit(prn(seed)),
            Draws::Replay { uniforms, .. } => {
                *uniforms
                    .get(self.step)
                    .ok_or(RafflesError::DimensionMismatch {
                        expected: self.step + 1,
                        found: uniforms.len(),
                    })?
            }
        };
        self.step += 1;
        Ok(u)
    }

    fn is_lcg(&self) -> bool {
        matches!(self.draws, Draws::Lcg { .. })
    }
}

/// RAVEN's `untruncatedStdDev` of a marginal, in scipy's operation order
/// where it differs from the variance's square root (normal: the scale;
/// uniform: `sqrt(1/12) · width`).
fn prior_std(prior: &IndependentPrior, i: usize) -> f64 {
    match prior.marginals()[i] {
        Distribution::Normal(n) => n.sigma(),
        Distribution::Uniform(u) => (1.0_f64 / 12.0).sqrt() * (u.upper() - u.lower()),
        d => d.variance().sqrt(),
    }
}

/// Runs RAVEN's Metropolis or adaptive Metropolis on `prior` and the
/// caller's `likelihood` (in the form `config.likelihood` says).
///
/// # Errors
///
/// [`RafflesError::InvalidParameter`] for an invalid configuration;
/// [`RafflesError::DimensionMismatch`] for an initial point or a replayed
/// draw of the wrong length, or a replay that runs out;
/// [`RafflesError::InvalidParameter`] for a replay without an initial point,
/// or an adaptive proposal covariance that is not positive definite (LCG
/// route only).
pub fn raven_mcmc<L>(
    prior: &IndependentPrior,
    config: &RavenMcmcConfig,
    likelihood: &L,
    draws: Draws,
) -> Result<RavenMcmcResult>
where
    L: Fn(&[f64]) -> f64,
{
    let d = prior.dimension();
    config.validate(d)?;
    let mut stream = Stream { draws, step: 0 };
    let initial = match (&config.initial, &mut stream.draws) {
        (Some(x), _) => x.clone(),
        (None, Draws::Lcg { seed }) => prior.sample(seed),
        (None, Draws::Replay { .. }) => {
            return Err(RafflesError::InvalidParameter {
                parameter: "initial".to_string(),
                value: f64::NAN,
                reason: "a replayed chain needs its recorded initial point".to_string(),
            })
        }
    };
    let marginals = prior.marginals();
    let ln_pdf = |i: usize, x: f64| marginals[i].pdf(x).r_ln();
    let ln_l = |x: &[f64]| -> f64 {
        let v = likelihood(x);
        match config.likelihood {
            LikelihoodForm::Log => v,
            LikelihoodForm::Linear => v.r_ln(),
        }
    };

    // Metropolis: Normal(0, 0.2 std) per parameter (Metropolis.py:96-116).
    let proposal_std: Vec<f64> = (0..d)
        .map(|i| RAVEN_DEFAULT_PROPOSAL_FRACTION * prior_std(prior, i))
        .collect();
    // Adaptive: lambda = 2.38^2 / d, diag(std^2), mean = initial
    // (AdaptiveMetropolis.py:132, 178-217).
    let adaptive = match config.kind {
        RavenMcmcKind::AdaptiveMetropolis { adaptive_interval } => Some(adaptive_interval),
        RavenMcmcKind::Metropolis => None,
    };
    let mut lambda = 2.38_f64.r_powi(2) / d as f64;
    let mut ens_cov: Vec<Vec<f64>> = (0..d)
        .map(|i| {
            (0..d)
                .map(|j| if i == j { prior_std(prior, i).r_powi(2) } else { 0.0 })
                .collect()
        })
        .collect();
    let mut ens_mean = initial.clone();
    let mut colour = if adaptive.is_some() && stream.is_lcg() {
        Some(colouring(lambda, &ens_cov)?)
    } else {
        None
    };

    let mut tune = config.tune;
    let mut scaling = 1.0;
    let mut counts_until_tune = config.tune_interval;
    let mut accept_in_tune = 0usize;
    let mut accept_count = 1usize;
    let mut accepted = false;
    let mut accept_rate = 1.0;
    let mut nan_log_ratios = 0usize;

    let mut current = initial.clone();
    let mut current_ll = ln_l(&current);
    let mut steps = Vec::with_capacity(config.limit);

    for counter in 1..=config.limit {
        let mut alpha = None;
        let mut proposed = current.clone();
        if counter > 1 {
            // localGenerateInput.
            let z = stream.proposal(d)?;
            let inc: Vec<f64> = match (&colour, adaptive) {
                (Some(l), Some(_)) => (0..d)
                    .map(|i| (0..=i).map(|j| l[i][j] * z[j]).sum::<f64>())
                    .collect(),
                (None, Some(_)) => z,
                (_, None) => {
                    if stream.is_lcg() {
                        (0..d).map(|i| proposal_std[i] * z[i]).collect()
                    } else {
                        z
                    }
                }
            };
            let mut outside = false;
            for i in 0..d {
                proposed[i] = current[i] + inc[i] * scaling;
                let (lo, hi) = marginals[i].support();
                if proposed[i] < lo || proposed[i] > hi {
                    outside = true;
                    if config.bounds == BoundHandling::ClampAsRaven {
                        proposed[i] = proposed[i].clamp(lo, hi);
                    }
                }
            }
            let reject_outside = outside && config.bounds == BoundHandling::Reject;
            let new_ll = if reject_outside {
                f64::NEG_INFINITY
            } else {
                ln_l(&proposed)
            };
            // _useRealization: the prior terms parameter by parameter, then
            // the likelihood, then min(0, .).
            let a = if reject_outside {
                f64::NEG_INFINITY
            } else {
                let mut net = 0.0;
                for i in 0..d {
                    net += ln_pdf(i, proposed[i]) - ln_pdf(i, current[i]);
                }
                net += new_ll - current_ll;
                if net.is_nan() {
                    nan_log_ratios += 1;
                }
                python_min_zero(net)
            };
            // _checkAcceptance.
            let u = stream.uniform()?;
            accepted = a > u.r_ln();
            if accepted {
                accept_count += 1;
                current.clone_from(&proposed);
                current_ll = new_ll;
            }
            accept_rate = accept_count as f64 / counter as f64;
            alpha = Some(a);
        }
        // Scale tuning (MCMC.py:384-393), step 1 included.
        if tune {
            if accepted {
                accept_in_tune += 1;
            }
            counts_until_tune -= 1;
        }
        if tune && counts_until_tune == 0 {
            scaling = tune_scaling(scaling, accept_in_tune as f64 / config.tune_interval as f64);
            counts_until_tune = config.tune_interval;
            accept_in_tune = 0;
        }
        // _updateAdaptiveParams (AdaptiveMetropolis.py:357-381).
        let mut step_lambda = None;
        if let (Some(interval), Some(a)) = (adaptive, alpha) {
            if counter == config.burn_in {
                lambda = scaling * scaling;
                scaling = 1.0;
                tune = false;
            } else if counter > config.burn_in {
                let gamma = 1.0 / ((counter - config.burn_in) as f64 + 1.0).sqrt();
                lambda *= (gamma * (a.r_exp() - RAVEN_OPTIMAL_ACCEPTANCE)).r_exp();
                if counter % interval == 0 {
                    let diff: Vec<f64> = (0..d).map(|i| current[i] - ens_mean[i]).collect();
                    for i in 0..d {
                        ens_mean[i] += gamma * diff[i];
                    }
                    for i in 0..d {
                        for j in 0..d {
                            ens_cov[i][j] += gamma * (diff[i] * diff[j] - ens_cov[i][j]);
                        }
                    }
                    if colour.is_some() {
                        colour = Some(colouring(lambda, &ens_cov)?);
                    }
                }
            }
            step_lambda = Some(lambda);
        }
        steps.push(RavenStep {
            proposed,
            current: current.clone(),
            alpha,
            accepted,
            accept_rate,
            scaling,
            lambda: step_lambda,
        });
    }
    Ok(RavenMcmcResult {
        steps,
        burn_in: config.burn_in,
        ensemble_mean: adaptive.map(|_| ens_mean),
        ensemble_cov: adaptive.map(|_| ens_cov),
        nan_log_ratios,
    })
}

/// Lower Cholesky factor of `lambda Σ`, the adaptive proposal's covariance.
/// RAVEN colours with an SVD (`MultivariateNormalPCA`); any square root
/// gives the same distribution.
fn colouring(lambda: f64, cov: &[Vec<f64>]) -> Result<Vec<Vec<f64>>> {
    let scaled: Vec<Vec<f64>> = cov
        .iter()
        .enumerate()
        .map(|(i, row)| {
            row.iter()
                .enumerate()
                .map(|(j, c)| lambda * 0.5 * (c + cov[j][i]))
                .collect()
        })
        .collect();
    super::transitional::cholesky(&scaled, 0.0).ok_or(RafflesError::InvalidParameter {
        parameter: "ensemble_cov".to_string(),
        value: lambda,
        reason: "adaptive proposal covariance is not positive definite".to_string(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::distributions::{Distribution, Normal, Uniform};
    use crate::estimators::autocorrelation::CorrelatedMean;

    fn normal(m: f64, s: f64) -> Distribution {
        Distribution::Normal(Normal::new(m, s).unwrap())
    }

    /// The PyMC3 table at every branch and its edges.
    #[test]
    fn tune_scaling_table() {
        let cases = [
            (0.0, 0.1),
            (0.0009, 0.1),
            (0.001, 0.5),
            (0.049, 0.5),
            (0.05, 0.9),
            (0.19, 0.9),
            (0.2, 1.0),
            (0.5, 1.0),
            (0.51, 1.1),
            (0.75, 1.1),
            (0.76, 2.0),
            (0.95, 2.0),
            (0.96, 10.0),
        ];
        for (rate, factor) in cases {
            assert_eq!(tune_scaling(1.0, rate), factor, "rate {rate}");
        }
        assert_eq!(python_min_zero(f64::NAN), 0.0);
        assert_eq!(python_min_zero(-1.0), -1.0);
        assert_eq!(python_min_zero(2.0), 0.0);
    }

    /// Configuration checks, and the replay contract (needs an initial point,
    /// fails when the recording runs out).
    #[test]
    fn configuration_and_replay_errors() {
        let prior = IndependentPrior::new(vec![normal(0.0, 1.0)]).unwrap();
        let ll = |x: &[f64]| -0.5 * x[0] * x[0];
        let mut c = RavenMcmcConfig::metropolis(10);
        c.burn_in = 10;
        assert!(raven_mcmc(&prior, &c, &ll, Draws::Lcg { seed: 1 }).is_err());
        let mut c = RavenMcmcConfig::metropolis(3);
        c.likelihood = LikelihoodForm::Log;
        let replay = Draws::Replay {
            increments: vec![vec![0.1]],
            uniforms: vec![0.5],
        };
        assert!(raven_mcmc(&prior, &c, &ll, replay.clone()).is_err());
        c.initial = Some(vec![0.0]);
        assert!(raven_mcmc(&prior, &c, &ll, replay).is_err());
        c.initial = Some(vec![0.0, 1.0]);
        assert!(raven_mcmc(&prior, &c, &ll, Draws::Lcg { seed: 1 }).is_err());
        let mut c = RavenMcmcConfig::adaptive_metropolis(5);
        c.kind = RavenMcmcKind::AdaptiveMetropolis {
            adaptive_interval: 0,
        };
        assert!(raven_mcmc(&prior, &c, &ll, Draws::Lcg { seed: 1 }).is_err());
        let mut c = RavenMcmcConfig::metropolis(5);
        c.tune_interval = 0;
        assert!(raven_mcmc(&prior, &c, &ll, Draws::Lcg { seed: 1 }).is_err());
        c.limit = 0;
        assert!(raven_mcmc(&prior, &c, &ll, Draws::Lcg { seed: 1 }).is_err());
    }

    /// **A1, conjugate Normal–Normal, Metropolis on the LCG route.**
    ///
    /// Method: prior N(0, 2²), five observations y = (1.2, 0.7, 1.9, 1.1,
    /// 0.4) with σ = 1 (the MCMC differential case's model); 200 000 steps,
    /// burn-in 2 000, RAVEN defaults otherwise (tuning every 100 steps), log
    /// likelihood, seed 20261104. Exact posterior N(5.3/5.25, 1/5.25):
    /// mean 1.0095238, sd 0.4364358.
    ///
    /// Prediction (V&V record, A1, before the run): mean within 4
    /// batch-means standard errors; sd within 3 %.
    ///
    /// Result: see the V&V record's results section.
    #[test]
    fn metropolis_recovers_the_conjugate_posterior() {
        let y = [1.2, 0.7, 1.9, 1.1, 0.4];
        let prior = IndependentPrior::new(vec![normal(0.0, 2.0)]).unwrap();
        let ll = |t: &[f64]| -0.5 * y.iter().map(|v| (v - t[0]).powi(2)).sum::<f64>();
        let mut c = RavenMcmcConfig::metropolis(200_000);
        c.burn_in = 2_000;
        c.likelihood = LikelihoodForm::Log;
        let r = raven_mcmc(&prior, &c, &ll, Draws::Lcg { seed: 20261104 }).unwrap();
        let s: Vec<f64> = r.samples().iter().map(|v| v[0]).collect();
        let cm = CorrelatedMean::estimate(&s);
        let sem = cm.batch.unwrap().sem;
        let mean_exact = 5.3 / 5.25;
        let sd_exact = (1.0_f64 / 5.25).sqrt();
        let var = s.iter().map(|v| (v - cm.mean).powi(2)).sum::<f64>() / (s.len() - 1) as f64;
        let sd = var.sqrt();
        eprintln!(
            "A1: mean {} (exact {mean_exact}, sem {sem}, z {}), sd {sd} (exact {sd_exact}, rel {}), accept {}, final scale {}",
            cm.mean,
            (cm.mean - mean_exact) / sem,
            sd / sd_exact - 1.0,
            r.accept_rate(),
            r.steps.last().unwrap().scaling
        );
        assert!((cm.mean - mean_exact).abs() < 4.0 * sem);
        assert!((sd / sd_exact - 1.0).abs() < 0.03);
        assert_eq!(r.nan_log_ratios, 0);
    }

    /// **A2, a correlated bivariate posterior, adaptive Metropolis, LCG.**
    ///
    /// Method: N(0, 3²) priors on both parameters, Gaussian likelihood with
    /// mean (1, −1), unit variances, correlation 0.8 (the adaptive
    /// differential case's model); 100 000 steps, burn-in 2 000, tuning
    /// every 50, adaptive interval 20, initial (0, 0), seed 11. Exact
    /// posterior: precision P = Σ_L⁻¹ + I/9, mean P⁻¹ Σ_L⁻¹ m.
    ///
    /// Prediction (V&V record, A2, before the run): means within 4
    /// batch-means standard errors; covariance entries within 8 %.
    ///
    /// Result: see the V&V record's results section.
    #[test]
    fn adaptive_metropolis_recovers_a_correlated_gaussian_posterior() {
        let prior = IndependentPrior::new(vec![normal(0.0, 3.0), normal(0.0, 3.0)]).unwrap();
        // Σ_L⁻¹ for unit variances, rho = 0.8.
        let det = 1.0 - 0.64;
        let pl = [[1.0 / det, -0.8 / det], [-0.8 / det, 1.0 / det]];
        let ll = |t: &[f64]| {
            let a = t[0] - 1.0;
            let b = t[1] + 1.0;
            -0.5 * (pl[0][0] * a * a + 2.0 * pl[0][1] * a * b + pl[1][1] * b * b)
        };
        let mut c = RavenMcmcConfig::adaptive_metropolis(100_000);
        c.burn_in = 2_000;
        c.tune_interval = 50;
        c.initial = Some(vec![0.0, 0.0]);
        c.likelihood = LikelihoodForm::Log;
        let r = raven_mcmc(&prior, &c, &ll, Draws::Lcg { seed: 11 }).unwrap();
        // Exact posterior.
        let p = [[pl[0][0] + 1.0 / 9.0, pl[0][1]], [pl[1][0], pl[1][1] + 1.0 / 9.0]];
        let pdet = p[0][0] * p[1][1] - p[0][1] * p[1][0];
        let cov = [[p[1][1] / pdet, -p[0][1] / pdet], [-p[1][0] / pdet, p[0][0] / pdet]];
        let b = [pl[0][0] - pl[0][1], pl[1][0] - pl[1][1]]; // Σ_L⁻¹ (1, −1)
        let mean = [
            cov[0][0] * b[0] + cov[0][1] * b[1],
            cov[1][0] * b[0] + cov[1][1] * b[1],
        ];
        let s = r.samples();
        let n = s.len() as f64;
        for k in 0..2 {
            let col: Vec<f64> = s.iter().map(|v| v[k]).collect();
            let cm = CorrelatedMean::estimate(&col);
            let sem = cm.batch.unwrap().sem;
            eprintln!(
                "A2: mean[{k}] {} exact {} z {}",
                cm.mean,
                mean[k],
                (cm.mean - mean[k]) / sem
            );
            assert!((cm.mean - mean[k]).abs() < 4.0 * sem);
        }
        let m: Vec<f64> = (0..2)
            .map(|k| s.iter().map(|v| v[k]).sum::<f64>() / n)
            .collect();
        for i in 0..2 {
            for j in 0..2 {
                let c_ij = s.iter().map(|v| (v[i] - m[i]) * (v[j] - m[j])).sum::<f64>() / (n - 1.0);
                eprintln!("A2: cov[{i}][{j}] {c_ij} exact {} rel {}", cov[i][j], c_ij / cov[i][j] - 1.0);
                assert!((c_ij / cov[i][j] - 1.0).abs() < 0.08);
            }
        }
        eprintln!("A2: accept {}, lambda {:?}", r.accept_rate(), r.steps.last().unwrap().lambda);
    }

    /// **A3, the clamp.** Uniform(0, 1) prior, likelihood N(0.05, 0.1²), so
    /// the posterior is that normal truncated to [0, 1], mean
    /// 0.05 + 0.1 φ(−0.5)/(1 − Φ(−0.5)) = 0.100916 (the upper truncation at
    /// 9.5 σ is negligible). Metropolis, 200 000 steps, burn-in 2 000,
    /// seed 5, once with RAVEN's clamp and once with rejection.
    ///
    /// Prediction (V&V record, A3, before the run): the clamp leaves a
    /// positive fraction of samples exactly at 0 and a mean more than 4 SE
    /// below 0.100916; rejection leaves none and a mean within 4 SE.
    ///
    /// Result: see the V&V record's results section.
    #[test]
    fn ravens_clamp_biases_a_posterior_near_a_bound_and_rejection_does_not() {
        let prior =
            IndependentPrior::new(vec![Distribution::Uniform(Uniform::new(0.0, 1.0).unwrap())])
                .unwrap();
        let ll = |t: &[f64]| -0.5 * ((t[0] - 0.05) / 0.1).powi(2);
        let exact = 0.05 + 0.1 * 0.352_065_326_764_299_5 / 0.691_462_461_274_013_1;
        let mut results = Vec::new();
        for bounds in [BoundHandling::ClampAsRaven, BoundHandling::Reject] {
            let mut c = RavenMcmcConfig::metropolis(200_000);
            c.burn_in = 2_000;
            c.likelihood = LikelihoodForm::Log;
            c.bounds = bounds;
            c.initial = Some(vec![0.5]);
            let r = raven_mcmc(&prior, &c, &ll, Draws::Lcg { seed: 5 }).unwrap();
            let s: Vec<f64> = r.samples().iter().map(|v| v[0]).collect();
            let at_zero = s.iter().filter(|v| **v == 0.0).count() as f64 / s.len() as f64;
            let cm = CorrelatedMean::estimate(&s);
            let sem = cm.batch.unwrap().sem;
            eprintln!(
                "A3 {bounds:?}: mean {} exact {exact} z {} at-zero fraction {at_zero}",
                cm.mean,
                (cm.mean - exact) / sem
            );
            results.push((cm.mean, sem, at_zero));
        }
        let (m_c, se_c, z_c) = results[0];
        let (m_r, se_r, z_r) = results[1];
        assert!(z_c > 0.0);
        assert!(exact - m_c > 4.0 * se_c);
        assert_eq!(z_r, 0.0);
        assert!((m_r - exact).abs() < 4.0 * se_r);
    }

    /// The linear-likelihood path takes the log of the closure's value, so
    /// it gives the same chain as the log path fed `ln L`, bit for bit when
    /// `ln(exp(v))` round-trips; and a likelihood that is NaN somewhere is
    /// counted and, as in RAVEN, accepted.
    #[test]
    fn linear_likelihood_and_nan_ratio_follow_raven() {
        let prior = IndependentPrior::new(vec![normal(0.0, 1.0)]).unwrap();
        let mut c = RavenMcmcConfig::metropolis(50);
        c.initial = Some(vec![0.0]);
        let lin = |t: &[f64]| (-0.5 * t[0] * t[0]).r_exp();
        let r = raven_mcmc(&prior, &c, &lin, Draws::Lcg { seed: 3 }).unwrap();
        assert_eq!(r.steps.len(), 50);
        assert_eq!(r.steps[0].alpha, None);
        assert!(!r.steps[0].accepted);
        let nan = |_: &[f64]| f64::NAN;
        let r = raven_mcmc(&prior, &c, &nan, Draws::Lcg { seed: 3 }).unwrap();
        assert_eq!(r.nan_log_ratios, 49);
        assert!(r.steps.iter().skip(1).all(|s| s.accepted));
    }
}
