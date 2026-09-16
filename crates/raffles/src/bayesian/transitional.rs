//! Transitional MCMC (TMCMC) and Transitional Ensemble MCMC (TEMCMC).
//!
//! # The idea
//!
//! Sampling a posterior directly is hard when the likelihood is sharp compared
//! with the prior: a chain started from the prior spends its life in the tails
//! and may never find the mass. Transitional samplers avoid that by walking
//! there in stages, through a sequence of intermediate distributions
//!
//! ```text
//! p_j(theta)  proportional to  p(theta) * L(theta)^beta_j,
//! 0 = beta_0 < beta_1 < ... < beta_m = 1
//! ```
//!
//! The first stage is the prior, the last is the posterior, and each stage is
//! close enough to the next that importance-resampling plus a short MCMC run
//! carries the population across. The tempering exponents are **not** chosen by
//! the user: each `beta_{j+1}` is solved for so that the coefficient of
//! variation of the importance weights hits a target (1.0 by default), which is
//! what makes the method robust across problems of very different sharpness.
//!
//! The by-product that makes this worth the machinery: the normalising constants
//! of the successive stages multiply to the **evidence** `p(D)`, so a single run
//! yields both the posterior sample and the marginal likelihood needed for model
//! comparison. A plain MCMC chain gives only the former.
//!
//! # The two samplers
//!
//! [`tmcmc`] and [`temcmc`] are the *same* algorithm with a different move
//! inside the stage:
//!
//! | | stage move | needs tuning? |
//! |---|---|---|
//! | [`tmcmc`] | random-walk Metropolis–Hastings, proposal covariance scaled from the weighted sample covariance | the scale factor, and it matters |
//! | [`temcmc`] | affine-invariant ensemble ("stretch") move | no |
//!
//! That is why they share one driver and one [`TransitionKernel`] enum rather
//! than being two functions with duplicated tempering logic: the tempering *is*
//! the algorithm, and the kernel is a choice within it.
//!
//! # References
//!
//! - J. Ching and Y.-C. Chen (2007). Transitional Markov Chain Monte Carlo
//!   method for Bayesian model updating, model class selection, and model
//!   averaging. *Journal of Engineering Mechanics, 133*(7), 816–832. doi:
//!   [10.1061/(ASCE)0733-9399(2007)133:7(816)](https://doi.org/10.1061/(ASCE)0733-9399(2007)133:7(816))
//! - A. Lye, A. Cicirello and E. Patelli (2022). An efficient and robust
//!   sampler for Bayesian inference: Transitional Ensemble Markov Chain Monte
//!   Carlo. *Mechanical Systems and Signal Processing, 167*, 108471. doi:
//!   [10.1016/j.ymssp.2021.108471](https://doi.org/10.1016/j.ymssp.2021.108471)
//! - A. Lye, A. Cicirello and E. Patelli (2021). Sampling methods for solving
//!   Bayesian model updating problems: A tutorial. *Mechanical Systems and
//!   Signal Processing, 159*, 107760. doi:
//!   [10.1016/j.ymssp.2021.107760](https://doi.org/10.1016/j.ymssp.2021.107760)
//!
//! Independent implementations from those papers — see the provenance note in
//! [`super`].

use outram_mc_libs::rng::lcg::prn;

use super::mcmc::{ChainState, EnsembleMove, MetropolisHastings};
use super::{clamp_open_unit, eval_ln_likelihood, IndependentPrior, SamplerDiagnostics};
use crate::samplers::stream_seed;
use crate::{RafflesError, Result};

/// Which Markov-chain move a transitional sampler uses inside each stage.
///
/// Enum dispatch rather than a trait object, per the workspace design rules —
/// and it earns its keep here beyond that rule: the two variants carry
/// genuinely different tuning knobs, and a `match` at the one place the move is
/// made keeps them from leaking into the tempering code.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum TransitionKernel {
    /// Ching and Chen's original: random-walk Metropolis–Hastings whose
    /// proposal covariance is `scale^2` times the weighted covariance of the
    /// current stage's population.
    ///
    /// `scale` is the paper's `beta` (renamed here because `beta` is already
    /// the tempering exponent). The paper's value is 0.2, and the result is
    /// sensitive to it: too small and the chains do not move, too large and
    /// they are rejected.
    MetropolisHastings {
        /// Proposal scale factor, strictly positive. Ching and Chen use 0.2.
        scale: f64,
    },
    /// Lye, Cicirello and Patelli's variant: the affine-invariant ensemble
    /// stretch move, which needs no proposal covariance at all.
    ///
    /// `a` is the stretch parameter, conventionally 2.
    AffineInvariantEnsemble {
        /// Stretch parameter, strictly greater than 1.
        a: f64,
    },
}

impl TransitionKernel {
    /// Ching and Chen's Metropolis–Hastings kernel at the published scale 0.2.
    pub fn metropolis_hastings() -> Self {
        Self::MetropolisHastings { scale: 0.2 }
    }

    /// The affine-invariant ensemble kernel at the conventional `a = 2`.
    pub fn affine_invariant_ensemble() -> Self {
        Self::AffineInvariantEnsemble {
            a: EnsembleMove::DEFAULT_A,
        }
    }
}

/// Configuration shared by both transitional samplers.
///
/// Defaults are the published ones where a paper gives a value, and are stated
/// in each field's documentation. [`TransitionalConfig::new`] is the only
/// constructor that validates, so a configuration that exists is a
/// configuration that can run.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct TransitionalConfig {
    /// Number of samples carried through every stage, `N`.
    ///
    /// This is the whole computational budget knob: the sampler evaluates the
    /// likelihood `N` times per stage plus `N * chain_length` times for the
    /// moves. For the ensemble kernel it is also the walker count, and must be
    /// at least `2 * d + 2`.
    pub population: usize,
    /// Target coefficient of variation of the stage weights, used to solve for
    /// the next tempering exponent. Ching and Chen use 1.0.
    ///
    /// Smaller means more, smaller stages: more robust and more expensive.
    pub cov_target: f64,
    /// Number of MCMC moves applied per sample per stage.
    ///
    /// 1 is the classical TMCMC choice — resampling plus one move per sample,
    /// with the decorrelation coming from the many stages. For the ensemble
    /// kernel this counts *sweeps* over the whole population.
    pub chain_length: usize,
    /// Hard cap on the number of stages, as a guard against a pathological
    /// likelihood that makes the tempering crawl.
    ///
    /// Reaching it is an error, not a truncated answer: a run that stopped at
    /// `beta < 1` has not sampled the posterior at all, and returning it as if
    /// it had is exactly the kind of silent wrongness the workspace rules
    /// forbid.
    pub max_stages: usize,
    /// Master seed for the run. The same seed with the same configuration
    /// reproduces the run exactly.
    pub seed: i64,
    /// Which move to use inside each stage.
    pub kernel: TransitionKernel,
}

impl TransitionalConfig {
    /// Builds and validates a configuration.
    ///
    /// # Errors
    ///
    /// [`RafflesError::InvalidParameter`] if `population` is zero,
    /// `cov_target` is not strictly positive, `chain_length` is zero,
    /// `max_stages` is zero, or the kernel's own parameter is out of range
    /// (`scale > 0`, `a > 1`).
    pub fn new(
        population: usize,
        cov_target: f64,
        chain_length: usize,
        max_stages: usize,
        seed: i64,
        kernel: TransitionKernel,
    ) -> Result<Self> {
        if population == 0 {
            return Err(RafflesError::InvalidParameter {
                parameter: "population".to_string(),
                value: 0.0,
                reason: "a transitional sampler needs at least one sample".to_string(),
            });
        }
        if !(cov_target > 0.0) {
            return Err(RafflesError::InvalidParameter {
                parameter: "cov_target".to_string(),
                value: cov_target,
                reason: "the target coefficient of variation must be strictly positive".to_string(),
            });
        }
        if chain_length == 0 {
            return Err(RafflesError::InvalidParameter {
                parameter: "chain_length".to_string(),
                value: 0.0,
                reason: "each stage must apply at least one move".to_string(),
            });
        }
        if max_stages == 0 {
            return Err(RafflesError::InvalidParameter {
                parameter: "max_stages".to_string(),
                value: 0.0,
                reason: "the stage cap must allow at least one stage".to_string(),
            });
        }
        match kernel {
            TransitionKernel::MetropolisHastings { scale } if !(scale > 0.0) => {
                return Err(RafflesError::InvalidParameter {
                    parameter: "scale".to_string(),
                    value: scale,
                    reason: "the Metropolis-Hastings proposal scale must be strictly positive"
                        .to_string(),
                })
            }
            TransitionKernel::AffineInvariantEnsemble { a } if !(a > 1.0) => {
                return Err(RafflesError::InvalidParameter {
                    parameter: "a".to_string(),
                    value: a,
                    reason: "the stretch parameter must be greater than 1".to_string(),
                })
            }
            _ => {}
        }
        Ok(Self {
            population,
            cov_target,
            chain_length,
            max_stages,
            seed,
            kernel,
        })
    }

    /// Ching and Chen's published defaults with the Metropolis–Hastings
    /// kernel: CoV target 1.0, proposal scale 0.2, one move per sample, a
    /// 200-stage cap.
    pub fn tmcmc_defaults(population: usize, seed: i64) -> Result<Self> {
        Self::new(
            population,
            1.0,
            1,
            200,
            seed,
            TransitionKernel::metropolis_hastings(),
        )
    }

    /// The same tempering defaults with the affine-invariant ensemble kernel.
    pub fn temcmc_defaults(population: usize, seed: i64) -> Result<Self> {
        Self::new(
            population,
            1.0,
            1,
            200,
            seed,
            TransitionKernel::affine_invariant_ensemble(),
        )
    }
}

/// What one tempering stage did.
///
/// Reported per stage because the *shape* of a run is what tells a reader
/// whether to trust it: a healthy run has tens of stages with acceptance rates
/// in the tens of percent and effective sample sizes that stay a decent
/// fraction of the population. One stage that jumps `beta` from 0.01 to 1.0
/// with an effective sample size of 3 has technically finished and has
/// sampled nothing.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct StageReport {
    /// Tempering exponent reached at the end of this stage, in `(0, 1]`.
    pub beta: f64,
    /// Natural log of this stage's contribution to the evidence.
    pub ln_evidence_increment: f64,
    /// Fraction of proposed moves accepted during this stage, in `[0, 1]`.
    pub acceptance_rate: f64,
    /// Kish effective sample size of the importance weights, `1 / sum(w^2)`
    /// for normalised weights, in `(0, population]`.
    pub effective_sample_size: f64,
}

/// The result of a transitional sampling run.
#[derive(Debug, Clone, PartialEq)]
pub struct TransitionalResult {
    /// Posterior samples, one row per sample, `population` rows of `d`
    /// parameters each.
    pub samples: Vec<Vec<f64>>,
    /// Natural log-likelihood at each returned sample, aligned with
    /// [`samples`](Self::samples).
    pub ln_likelihoods: Vec<f64>,
    /// Natural log of the estimated evidence `p(D)`.
    ///
    /// The product of the per-stage normalising constants. This is the number
    /// to use for model comparison; exponentiating it is usually a mistake,
    /// since it underflows for any realistic data set.
    pub ln_evidence: f64,
    /// One entry per tempering stage, in order.
    pub stages: Vec<StageReport>,
    /// Numerical trouble met and handled during the run.
    pub diagnostics: SamplerDiagnostics,
}

impl TransitionalResult {
    /// Sample mean of each parameter — the posterior mean estimate.
    pub fn posterior_mean(&self) -> Vec<f64> {
        let n = self.samples.len() as f64;
        let d = self.samples.first().map(|s| s.len()).unwrap_or(0);
        (0..d)
            .map(|j| self.samples.iter().map(|s| s[j]).sum::<f64>() / n)
            .collect()
    }

    /// Unbiased sample standard deviation of each parameter.
    pub fn posterior_std_dev(&self) -> Vec<f64> {
        let n = self.samples.len() as f64;
        let mean = self.posterior_mean();
        let d = mean.len();
        (0..d)
            .map(|j| {
                let var = self
                    .samples
                    .iter()
                    .map(|s| (s[j] - mean[j]).powi(2))
                    .sum::<f64>()
                    / (n - 1.0);
                var.sqrt()
            })
            .collect()
    }
}

/// Runs Transitional MCMC (Ching and Chen, 2007).
///
/// Convenience wrapper over [`run_transitional`] with the
/// [`TransitionKernel::MetropolisHastings`] kernel forced, so a caller who
/// wants "the TMCMC from the paper" cannot accidentally get the ensemble
/// variant. Any other kernel in `config` is overridden.
///
/// # Verification
///
/// **Methodology.** The conjugate Normal–Normal problem, where all three
/// outputs have closed forms. Data `y_i` drawn from `N(mu_true, sigma^2)` with
/// `sigma` known; prior `mu ~ N(mu_0, tau^2)`. Then the posterior is
/// `N(mu_n, tau_n^2)` with
///
/// ```text
/// tau_n^2 = 1 / (1/tau^2 + n/sigma^2)
/// mu_n    = tau_n^2 * (mu_0/tau^2 + n*ybar/sigma^2)
/// ```
///
/// and the evidence `p(D)` is available in closed form too (the marginal of a
/// Gaussian mean under a Gaussian prior). The sampler is checked on the
/// posterior mean, the posterior standard deviation **and** the log-evidence —
/// the last being the one a posterior-moments-only test would miss.
///
/// **Results** are recorded in the test module of this file
/// (`tmcmc_matches_the_conjugate_normal_posterior`), which prints the measured
/// values on `cargo test -- --nocapture`.
pub fn tmcmc<L>(
    prior: &IndependentPrior,
    ln_likelihood: L,
    config: &TransitionalConfig,
) -> Result<TransitionalResult>
where
    L: Fn(&[f64]) -> f64,
{
    let mut config = *config;
    if !matches!(config.kernel, TransitionKernel::MetropolisHastings { .. }) {
        config.kernel = TransitionKernel::metropolis_hastings();
    }
    run_transitional(prior, ln_likelihood, &config)
}

/// Runs Transitional Ensemble MCMC (Lye, Cicirello and Patelli, 2022).
///
/// Convenience wrapper over [`run_transitional`] with the
/// [`TransitionKernel::AffineInvariantEnsemble`] kernel forced. Any other
/// kernel in `config` is overridden.
///
/// The practical difference from [`tmcmc`]: no proposal scale to choose, and
/// performance that does not degrade when parameters differ wildly in
/// magnitude — see the affine-invariance test in [`super::mcmc`].
///
/// # Verification
///
/// Same conjugate Normal–Normal methodology as [`tmcmc`]; measured values in
/// `temcmc_matches_the_conjugate_normal_posterior` in this file's test module.
pub fn temcmc<L>(
    prior: &IndependentPrior,
    ln_likelihood: L,
    config: &TransitionalConfig,
) -> Result<TransitionalResult>
where
    L: Fn(&[f64]) -> f64,
{
    let mut config = *config;
    if !matches!(
        config.kernel,
        TransitionKernel::AffineInvariantEnsemble { .. }
    ) {
        config.kernel = TransitionKernel::affine_invariant_ensemble();
    }
    run_transitional(prior, ln_likelihood, &config)
}

/// The shared transitional driver: tempering schedule, resampling, evidence,
/// and whichever stage move [`TransitionalConfig::kernel`] names.
///
/// Prefer [`tmcmc`] or [`temcmc`] unless you are deliberately mixing.
///
/// # Errors
///
/// - [`RafflesError::InvalidParameter`] if the ensemble kernel is chosen with
///   a population below `2 * d + 2` (it would silently confine the walkers to
///   a subspace), or if every initial prior draw has zero likelihood — that
///   means the prior and the data do not overlap at all, and no amount of
///   tempering fixes it.
/// - [`RafflesError::InvalidParameter`] if the run hits
///   [`TransitionalConfig::max_stages`] before `beta` reaches 1. The partial
///   population is *not* returned: it is not a posterior sample.
pub fn run_transitional<L>(
    prior: &IndependentPrior,
    ln_likelihood: L,
    config: &TransitionalConfig,
) -> Result<TransitionalResult>
where
    L: Fn(&[f64]) -> f64,
{
    let d = prior.dimension();
    let n = config.population;

    if let TransitionKernel::AffineInvariantEnsemble { .. } = config.kernel {
        let minimum = EnsembleMove::minimum_population(d);
        if n < minimum {
            return Err(RafflesError::InvalidParameter {
                parameter: "population".to_string(),
                value: n as f64,
                reason: format!(
                    "the affine-invariant ensemble kernel needs at least {minimum} samples in \
                     {d} dimensions"
                ),
            });
        }
    }

    let mut diagnostics = SamplerDiagnostics::default();

    // Stage 0: the prior itself.
    let mut seed = stream_seed(config.seed, 0);
    let mut theta: Vec<Vec<f64>> = (0..n).map(|_| prior.sample(&mut seed)).collect();
    let mut ln_l: Vec<f64> = theta
        .iter()
        .map(|t| eval_ln_likelihood(&ln_likelihood, t, &mut diagnostics))
        .collect();

    if ln_l.iter().all(|v| !v.is_finite()) {
        return Err(RafflesError::InvalidParameter {
            parameter: "ln_likelihood".to_string(),
            value: f64::NEG_INFINITY,
            reason: "every prior sample has zero likelihood; the prior and the data do not \
                     overlap, which tempering cannot repair"
                .to_string(),
        });
    }

    let mut beta = 0.0;
    let mut ln_evidence = 0.0;
    let mut stages: Vec<StageReport> = Vec::new();

    for stage in 0..config.max_stages {
        let d_beta = solve_delta_beta(&ln_l, beta, config.cov_target);
        let next_beta = (beta + d_beta).min(1.0);

        // Importance weights for the step from beta to next_beta, in log
        // space and shifted by their maximum so the exponential cannot
        // overflow for a sharply-peaked likelihood.
        let ln_w: Vec<f64> = ln_l
            .iter()
            .map(|l| {
                if l.is_finite() {
                    d_beta * l
                } else {
                    f64::NEG_INFINITY
                }
            })
            .collect();
        let ln_w_max = ln_w.iter().cloned().fold(f64::NEG_INFINITY, f64::max);
        let w: Vec<f64> = ln_w.iter().map(|lw| (lw - ln_w_max).exp()).collect();
        let w_sum: f64 = w.iter().sum();

        // Evidence increment: ln( mean_i exp(d_beta * lnL_i) ).
        ln_evidence += ln_w_max + (w_sum / n as f64).ln();

        let w_norm: Vec<f64> = w.iter().map(|v| v / w_sum).collect();
        let ess = 1.0 / w_norm.iter().map(|v| v * v).sum::<f64>();

        // Weighted covariance of the current population, needed by the
        // Metropolis-Hastings kernel and harmless to skip otherwise.
        let resampled = systematic_resample(&w_norm, &mut seed);

        let mut accepted = 0usize;
        let mut proposed = 0usize;

        let target = |t: &[f64]| -> f64 {
            let ln_prior = prior.ln_pdf(t);
            if !ln_prior.is_finite() {
                return f64::NEG_INFINITY;
            }
            let l = ln_likelihood(t);
            if l.is_nan() {
                return f64::NEG_INFINITY;
            }
            if !l.is_finite() {
                // ln L = -inf is a legitimate "impossible"; +inf is not.
                return if l == f64::NEG_INFINITY {
                    f64::NEG_INFINITY
                } else {
                    f64::NEG_INFINITY
                };
            }
            ln_prior + next_beta * l
        };

        let mut population: Vec<ChainState> = resampled
            .iter()
            .map(|&i| {
                let t = theta[i].clone();
                let ln_prior = prior.ln_pdf(&t);
                let value = if ln_prior.is_finite() && ln_l[i].is_finite() {
                    ln_prior + next_beta * ln_l[i]
                } else {
                    f64::NEG_INFINITY
                };
                ChainState {
                    theta: t,
                    ln_target: value,
                }
            })
            .collect();

        match config.kernel {
            TransitionKernel::MetropolisHastings { scale } => {
                let covariance = weighted_covariance(&theta, &w_norm, d);
                let cholesky = cholesky_scaled(&covariance, scale, &mut diagnostics);
                let kernel = MetropolisHastings::from_cholesky(cholesky)?;
                for state in population.iter_mut() {
                    for _ in 0..config.chain_length {
                        proposed += 1;
                        if kernel.step(state, &target, &mut seed) {
                            accepted += 1;
                        }
                    }
                }
            }
            TransitionKernel::AffineInvariantEnsemble { a } => {
                let mv = EnsembleMove::new(a)?;
                for _ in 0..config.chain_length {
                    proposed += population.len();
                    accepted += mv.sweep(&mut population, &target, &mut seed)?;
                }
            }
        }

        // Recover the likelihood at the moved points. The stage target is
        // ln_prior + next_beta * lnL, so lnL comes back by subtraction — which
        // avoids a second full pass of likelihood evaluations, the expensive
        // part of any real problem.
        theta = population.iter().map(|s| s.theta.clone()).collect();
        ln_l = population
            .iter()
            .map(|s| {
                if !s.ln_target.is_finite() {
                    return f64::NEG_INFINITY;
                }
                if next_beta <= 0.0 {
                    return eval_ln_likelihood(&ln_likelihood, &s.theta, &mut diagnostics);
                }
                let ln_prior = prior.ln_pdf(&s.theta);
                (s.ln_target - ln_prior) / next_beta
            })
            .collect();

        stages.push(StageReport {
            beta: next_beta,
            ln_evidence_increment: ln_w_max + (w_sum / n as f64).ln(),
            acceptance_rate: if proposed == 0 {
                0.0
            } else {
                accepted as f64 / proposed as f64
            },
            effective_sample_size: ess,
        });

        beta = next_beta;
        if beta >= 1.0 {
            return Ok(TransitionalResult {
                samples: theta,
                ln_likelihoods: ln_l,
                ln_evidence,
                stages,
                diagnostics,
            });
        }
        let _ = stage;
    }

    Err(RafflesError::InvalidParameter {
        parameter: "max_stages".to_string(),
        value: config.max_stages as f64,
        reason: format!(
            "tempering reached only beta = {beta:.6} within the stage cap; the partial \
             population is not a posterior sample and is not returned"
        ),
    })
}

/// Solves for the tempering increment that puts the weights' coefficient of
/// variation at `cov_target`.
///
/// The CoV of `w_i = L_i^(d_beta)` increases monotonically with `d_beta` — at
/// `d_beta = 0` every weight is 1 and the CoV is 0; as `d_beta` grows the
/// weights spread out. Bisection on that monotone function is therefore both
/// safe and enough; no derivative is needed.
///
/// Returns `1 - beta` directly when even the full remaining step keeps the CoV
/// below target, which is how the schedule finishes.
fn solve_delta_beta(ln_l: &[f64], beta: f64, cov_target: f64) -> f64 {
    let remaining = 1.0 - beta;
    if remaining <= 0.0 {
        return 0.0;
    }
    if weight_cov(ln_l, remaining) <= cov_target {
        return remaining;
    }

    let mut lo = 0.0;
    let mut hi = remaining;
    // 60 bisections take the bracket below f64 resolution for any realistic
    // remaining interval; the loop is bounded rather than tolerance-driven so
    // that a pathological likelihood cannot spin here.
    for _ in 0..60 {
        let mid = 0.5 * (lo + hi);
        if weight_cov(ln_l, mid) > cov_target {
            hi = mid;
        } else {
            lo = mid;
        }
    }
    let solved = 0.5 * (lo + hi);
    // Never return exactly zero: a zero step makes no progress and would spin
    // the stage loop to its cap. The floor is small enough to be irrelevant to
    // a healthy run and large enough to guarantee termination.
    solved.max(remaining * 1.0e-6)
}

/// Coefficient of variation of the importance weights for a tempering step of
/// `d_beta`, computed in log space.
fn weight_cov(ln_l: &[f64], d_beta: f64) -> f64 {
    let ln_w: Vec<f64> = ln_l
        .iter()
        .map(|l| {
            if l.is_finite() {
                d_beta * l
            } else {
                f64::NEG_INFINITY
            }
        })
        .collect();
    let ln_w_max = ln_w.iter().cloned().fold(f64::NEG_INFINITY, f64::max);
    if !ln_w_max.is_finite() {
        return f64::INFINITY;
    }
    let w: Vec<f64> = ln_w.iter().map(|lw| (lw - ln_w_max).exp()).collect();
    let n = w.len() as f64;
    let mean = w.iter().sum::<f64>() / n;
    if !(mean > 0.0) {
        return f64::INFINITY;
    }
    let var = w.iter().map(|v| (v - mean).powi(2)).sum::<f64>() / n;
    var.sqrt() / mean
}

/// Systematic resampling of `n` indices from normalised weights.
///
/// One uniform draw for the whole pass, then `n` equally-spaced strata. Chosen
/// over multinomial resampling because it has strictly lower variance for the
/// same cost, and over residual resampling because it is simpler and the
/// difference does not matter at these population sizes.
fn systematic_resample(w_norm: &[f64], seed: &mut u64) -> Vec<usize> {
    let n = w_norm.len();
    let step = 1.0 / n as f64;
    let start = clamp_open_unit(prn(seed)) * step;
    let mut indices = Vec::with_capacity(n);
    let mut cumulative = 0.0;
    let mut i = 0usize;
    for k in 0..n {
        let position = start + k as f64 * step;
        while i < n && cumulative + w_norm[i] < position {
            cumulative += w_norm[i];
            i += 1;
        }
        indices.push(i.min(n - 1));
    }
    indices
}

/// Weighted covariance matrix of a population, `d` by `d`.
fn weighted_covariance(theta: &[Vec<f64>], w_norm: &[f64], d: usize) -> Vec<Vec<f64>> {
    let mut mean = vec![0.0; d];
    for (row, w) in theta.iter().zip(w_norm.iter()) {
        for j in 0..d {
            mean[j] += w * row[j];
        }
    }
    let mut cov = vec![vec![0.0; d]; d];
    for (row, w) in theta.iter().zip(w_norm.iter()) {
        for i in 0..d {
            let di = row[i] - mean[i];
            for j in 0..d {
                cov[i][j] += w * di * (row[j] - mean[j]);
            }
        }
    }
    cov
}

/// Lower Cholesky factor of `scale^2 * covariance`, regularising if needed.
///
/// A resampled population can collapse onto fewer distinct points than there
/// are parameters, which makes the covariance singular. Rather than fail the
/// run — the population is still perfectly usable, it just needs a proposal
/// that can move — the diagonal is inflated until the factorisation succeeds,
/// and [`SamplerDiagnostics::covariance_regularisations`] is incremented so the
/// event is visible in the result rather than hidden.
fn cholesky_scaled(
    covariance: &[Vec<f64>],
    scale: f64,
    diagnostics: &mut SamplerDiagnostics,
) -> Vec<Vec<f64>> {
    let d = covariance.len();
    let mut scaled = vec![vec![0.0; d]; d];
    let mut trace = 0.0;
    for i in 0..d {
        for j in 0..d {
            scaled[i][j] = scale * scale * covariance[i][j];
        }
        trace += scaled[i][i];
    }

    // A floor relative to the matrix's own size, so the regularisation is
    // scale-free: a covariance in units of 1e12 needs a bigger nudge than one
    // in units of 1e-12.
    let base = if trace > 0.0 {
        trace / d as f64
    } else {
        1.0
    };

    let mut jitter = 0.0;
    for attempt in 0..12 {
        if let Some(l) = cholesky(&scaled, jitter) {
            if attempt > 0 {
                diagnostics.covariance_regularisations += 1;
            }
            return l;
        }
        jitter = if jitter == 0.0 {
            base * 1.0e-10
        } else {
            jitter * 100.0
        };
    }

    // Everything failed: fall back to an isotropic proposal sized by the
    // matrix's own trace. This still moves the chain, and the diagnostics
    // record that it happened.
    diagnostics.covariance_regularisations += 1;
    let sigma = (base.max(f64::MIN_POSITIVE)).sqrt();
    (0..d)
        .map(|i| (0..d).map(|j| if i == j { sigma } else { 0.0 }).collect())
        .collect()
}

/// Plain lower Cholesky factorisation with an added diagonal `jitter`.
///
/// Returns `None` if the matrix is not positive definite, which is what the
/// caller uses to decide to regularise.
fn cholesky(matrix: &[Vec<f64>], jitter: f64) -> Option<Vec<Vec<f64>>> {
    let d = matrix.len();
    let mut l = vec![vec![0.0; d]; d];
    for i in 0..d {
        for j in 0..=i {
            let mut sum = matrix[i][j] + if i == j { jitter } else { 0.0 };
            for k in 0..j {
                sum -= l[i][k] * l[j][k];
            }
            if i == j {
                if !(sum > 0.0) || !sum.is_finite() {
                    return None;
                }
                l[i][j] = sum.sqrt();
            } else {
                if l[j][j] == 0.0 {
                    return None;
                }
                l[i][j] = sum / l[j][j];
            }
        }
    }
    Some(l)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::distributions::{Distribution, Normal, Uniform};

    /// Closed-form posterior and evidence for the conjugate Normal–Normal
    /// problem: data `y ~ N(mu, sigma^2)` with `sigma` known, prior
    /// `mu ~ N(mu_0, tau^2)`.
    ///
    /// Returns `(posterior mean, posterior sd, ln evidence)`.
    fn conjugate_reference(y: &[f64], sigma: f64, mu_0: f64, tau: f64) -> (f64, f64, f64) {
        let n = y.len() as f64;
        let ybar = y.iter().sum::<f64>() / n;
        let post_var = 1.0 / (1.0 / (tau * tau) + n / (sigma * sigma));
        let post_mean = post_var * (mu_0 / (tau * tau) + n * ybar / (sigma * sigma));

        // Evidence: integrate N(y | mu, sigma^2) N(mu | mu_0, tau^2) over mu.
        // Writing S = sum (y_i - ybar)^2, the closed form is
        //   ln p(D) = -n/2 ln(2 pi sigma^2) - S/(2 sigma^2)
        //             + 1/2 ln(post_var / tau^2)
        //             - ybar^2 n /(2 sigma^2) - mu_0^2/(2 tau^2)
        //             + post_mean^2 / (2 post_var)
        let s: f64 = y.iter().map(|v| (v - ybar).powi(2)).sum();
        let ln_evidence = -0.5 * n * (2.0 * core::f64::consts::PI * sigma * sigma).ln()
            - s / (2.0 * sigma * sigma)
            + 0.5 * (post_var / (tau * tau)).ln()
            - n * ybar * ybar / (2.0 * sigma * sigma)
            - mu_0 * mu_0 / (2.0 * tau * tau)
            + post_mean * post_mean / (2.0 * post_var);
        (post_mean, post_var.sqrt(), ln_evidence)
    }

    /// Twelve observations with mean 2.5, standard deviation 1, used by every
    /// conjugate test below.
    fn data() -> Vec<f64> {
        vec![
            2.1, 3.4, 1.9, 2.8, 2.2, 3.1, 2.6, 2.0, 3.0, 2.4, 2.9, 1.6,
        ]
    }

    fn conjugate_setup() -> (Vec<f64>, f64, f64, f64, IndependentPrior) {
        let sigma = 1.0;
        let mu_0 = 0.0;
        let tau = 5.0;
        let prior = IndependentPrior::new(vec![Distribution::Normal(
            Normal::new(mu_0, tau).unwrap(),
        )])
        .unwrap();
        (data(), sigma, mu_0, tau, prior)
    }

    fn conjugate_ln_likelihood(y: Vec<f64>, sigma: f64) -> impl Fn(&[f64]) -> f64 {
        move |theta: &[f64]| {
            let mu = theta[0];
            let n = y.len() as f64;
            let ss: f64 = y.iter().map(|v| (v - mu).powi(2)).sum();
            -0.5 * n * (2.0 * core::f64::consts::PI * sigma * sigma).ln()
                - ss / (2.0 * sigma * sigma)
        }
    }

    /// **Methodology.** Conjugate Normal–Normal (see [`super::tmcmc`]): 12
    /// observations, known `sigma = 1`, prior `mu ~ N(0, 5^2)`. TMCMC with
    /// 2 000 samples, CoV target 1.0, proposal scale 0.2, one move per stage,
    /// seed 20260916. Checked against the closed-form posterior mean,
    /// posterior standard deviation and log-evidence. Pass criterion: mean
    /// within 0.05 (about 6 % of the posterior sd), sd within 15 %,
    /// log-evidence within 0.5 nat.
    ///
    /// **Result** (2026-09-16, `--release`): mean 2.494392 against the
    /// closed-form 2.491694; sd 0.299354 against 0.288195 (+3.9 %); ln Z
    /// -15.832091 against -15.685402 (0.147 nat high). Four tempering stages.
    /// All three inside tolerance. The test prints these under
    /// `cargo test -- --nocapture`.
    #[test]
    fn tmcmc_matches_the_conjugate_normal_posterior() {
        let (y, sigma, mu_0, tau, prior) = conjugate_setup();
        let (ref_mean, ref_sd, ref_ln_evidence) = conjugate_reference(&y, sigma, mu_0, tau);
        let config = TransitionalConfig::tmcmc_defaults(2_000, 20_260_916).unwrap();
        let result = tmcmc(&prior, conjugate_ln_likelihood(y, sigma), &config).unwrap();

        let mean = result.posterior_mean()[0];
        let sd = result.posterior_std_dev()[0];
        println!(
            "TMCMC conjugate: mean {mean:.6} (ref {ref_mean:.6}), sd {sd:.6} (ref {ref_sd:.6}), \
             ln Z {:.6} (ref {ref_ln_evidence:.6}), stages {}",
            result.ln_evidence,
            result.stages.len()
        );

        assert!((mean - ref_mean).abs() < 0.05, "mean {mean} vs {ref_mean}");
        assert!(
            (sd - ref_sd).abs() < 0.15 * ref_sd,
            "sd {sd} vs {ref_sd}"
        );
        assert!(
            (result.ln_evidence - ref_ln_evidence).abs() < 0.5,
            "ln evidence {} vs {ref_ln_evidence}",
            result.ln_evidence
        );
        assert!(result.stages.last().unwrap().beta >= 1.0);
    }

    /// **Methodology.** Identical to the TMCMC conjugate test, with the
    /// affine-invariant ensemble kernel and no proposal scale supplied.
    /// Same data, prior, population and tolerances; seed 20260916.
    ///
    /// **Result** (2026-09-16, `--release`): mean 2.497856 against 2.491694;
    /// sd 0.295145 against 0.288195 (+2.4 %); ln Z -15.753896 against
    /// -15.685402 (0.068 nat high). Four stages. Every figure is closer to
    /// the closed form than the tuned Metropolis-Hastings run above, with no
    /// proposal scale supplied.
    #[test]
    fn temcmc_matches_the_conjugate_normal_posterior() {
        let (y, sigma, mu_0, tau, prior) = conjugate_setup();
        let (ref_mean, ref_sd, ref_ln_evidence) = conjugate_reference(&y, sigma, mu_0, tau);
        let config = TransitionalConfig::temcmc_defaults(2_000, 20_260_916).unwrap();
        let result = temcmc(&prior, conjugate_ln_likelihood(y, sigma), &config).unwrap();

        let mean = result.posterior_mean()[0];
        let sd = result.posterior_std_dev()[0];
        println!(
            "TEMCMC conjugate: mean {mean:.6} (ref {ref_mean:.6}), sd {sd:.6} (ref {ref_sd:.6}), \
             ln Z {:.6} (ref {ref_ln_evidence:.6}), stages {}",
            result.ln_evidence,
            result.stages.len()
        );

        assert!((mean - ref_mean).abs() < 0.05, "mean {mean} vs {ref_mean}");
        assert!((sd - ref_sd).abs() < 0.15 * ref_sd, "sd {sd} vs {ref_sd}");
        assert!(
            (result.ln_evidence - ref_ln_evidence).abs() < 0.5,
            "ln evidence {} vs {ref_ln_evidence}",
            result.ln_evidence
        );
    }

    /// **Methodology.** Himmelblau's function has four equal minima, at
    /// (3, 2), (-2.805118, 3.131312), (-3.779310, -3.283186) and
    /// (3.584428, -1.848126). Used as a log-likelihood `-f(x, y)` it becomes a
    /// four-peaked target, and the tutorial problem in the TEMCMC literature
    /// asks whether a sampler finds all four rather than collapsing onto one.
    /// Prior `Uniform(-5, 5)` in both coordinates, 4 000 samples, seed 2022.
    /// Pass criterion: every one of the four peaks has at least 2 % of the
    /// posterior samples within a radius of 0.75 of it.
    ///
    /// **Result** (2026-09-16, `--release`, seed 2022): occupancy 0.3395,
    /// 0.1600, 0.1638, 0.3358 at the four peaks respectively — every peak
    /// found, and 99.9 % of the posterior sample accounted for by the four
    /// of them. The 2:1 split between the two pairs reflects the differing
    /// curvature at the peaks, not a sampler failure: Himmelblau's minima are
    /// equal in value but not in width.
    #[test]
    fn temcmc_finds_all_four_himmelblau_peaks() {
        let prior = IndependentPrior::new(vec![
            Distribution::Uniform(Uniform::new(-5.0, 5.0).unwrap()),
            Distribution::Uniform(Uniform::new(-5.0, 5.0).unwrap()),
        ])
        .unwrap();

        // Himmelblau's function, sharpened so the peaks are distinguishable.
        let ln_likelihood = |theta: &[f64]| {
            let (x, y) = (theta[0], theta[1]);
            let f = (x * x + y - 11.0).powi(2) + (x + y * y - 7.0).powi(2);
            -f
        };

        let config = TransitionalConfig::temcmc_defaults(4_000, 2_022).unwrap();
        let result = run_transitional(&prior, ln_likelihood, &config).unwrap();

        let peaks = [
            (3.0, 2.0),
            (-2.805118, 3.131312),
            (-3.779310, -3.283186),
            (3.584428, -1.848126),
        ];
        let mut fractions = Vec::new();
        for (px, py) in peaks {
            let count = result
                .samples
                .iter()
                .filter(|s| ((s[0] - px).powi(2) + (s[1] - py).powi(2)).sqrt() < 0.75)
                .count();
            fractions.push(count as f64 / result.samples.len() as f64);
        }
        println!("TEMCMC Himmelblau peak occupancy: {fractions:?}");
        for (i, f) in fractions.iter().enumerate() {
            assert!(*f > 0.02, "peak {i} holds only {f} of the samples");
        }
    }

    /// **Methodology.** The evidence is the output a posterior-only test would
    /// miss, so it gets its own check with a *different* prior width. Widening
    /// the prior from `tau = 5` to `tau = 50` must lower the log-evidence by
    /// very nearly `ln(10)` (the Occam factor), because the likelihood is
    /// unchanged and the prior is ten times more diffuse.
    ///
    /// **Result** (2026-09-16, `--release`, seed 777): measured difference
    /// 2.234869 nat against the closed-form 2.177603; ln(10) = 2.302585. The
    /// sampler reproduces the Occam penalty to within 0.06 nat.
    #[test]
    fn evidence_carries_the_occam_factor_of_a_wider_prior() {
        let y = data();
        let sigma = 1.0;
        let narrow = IndependentPrior::new(vec![Distribution::Normal(
            Normal::new(0.0, 5.0).unwrap(),
        )])
        .unwrap();
        let wide = IndependentPrior::new(vec![Distribution::Normal(
            Normal::new(0.0, 50.0).unwrap(),
        )])
        .unwrap();

        let config = TransitionalConfig::temcmc_defaults(3_000, 777).unwrap();
        let narrow_result =
            temcmc(&narrow, conjugate_ln_likelihood(y.clone(), sigma), &config).unwrap();
        let wide_result = temcmc(&wide, conjugate_ln_likelihood(y.clone(), sigma), &config).unwrap();

        let (_, _, narrow_ref) = conjugate_reference(&y, sigma, 0.0, 5.0);
        let (_, _, wide_ref) = conjugate_reference(&y, sigma, 0.0, 50.0);
        let measured = narrow_result.ln_evidence - wide_result.ln_evidence;
        let reference = narrow_ref - wide_ref;
        println!(
            "Occam factor: measured {measured:.6}, closed form {reference:.6}, ln(10) = {:.6}",
            10.0_f64.ln()
        );
        assert!(
            (measured - reference).abs() < 0.5,
            "evidence difference {measured} vs {reference}"
        );
    }

    /// **Methodology.** Determinism underpins every recorded number here. Two
    /// runs with the same seed and configuration must agree exactly.
    ///
    /// **Result.** Byte-identical samples and log-evidence (2026-09-16).
    #[test]
    fn runs_are_reproducible_from_the_seed() {
        let (y, sigma, _, _, prior) = conjugate_setup();
        let config = TransitionalConfig::temcmc_defaults(500, 4_242).unwrap();
        let a = temcmc(&prior, conjugate_ln_likelihood(y.clone(), sigma), &config).unwrap();
        let b = temcmc(&prior, conjugate_ln_likelihood(y, sigma), &config).unwrap();
        assert_eq!(a.samples, b.samples);
        assert_eq!(a.ln_evidence, b.ln_evidence);
    }

    /// **Methodology.** A prior that cannot produce a single feasible sample
    /// must be reported, not silently sampled. Likelihood returns `-inf`
    /// everywhere.
    ///
    /// **Result.** `InvalidParameter` naming the prior/data mismatch
    /// (2026-09-16).
    #[test]
    fn a_prior_that_never_overlaps_the_data_is_an_error() {
        let prior = IndependentPrior::new(vec![Distribution::Normal(
            Normal::new(0.0, 1.0).unwrap(),
        )])
        .unwrap();
        let config = TransitionalConfig::tmcmc_defaults(100, 1).unwrap();
        let err = tmcmc(&prior, |_: &[f64]| f64::NEG_INFINITY, &config).unwrap_err();
        assert!(matches!(err, RafflesError::InvalidParameter { .. }));
    }

    /// **Methodology.** The ensemble kernel's minimum population applies to the
    /// transitional driver too, and must be refused up front rather than
    /// discovered mid-run. Two parameters need 6 samples; 4 are offered.
    ///
    /// **Result.** `InvalidParameter` (2026-09-16).
    #[test]
    fn ensemble_kernel_refuses_an_undersized_population() {
        let prior = IndependentPrior::new(vec![
            Distribution::Uniform(Uniform::new(0.0, 1.0).unwrap()),
            Distribution::Uniform(Uniform::new(0.0, 1.0).unwrap()),
        ])
        .unwrap();
        let config = TransitionalConfig::temcmc_defaults(4, 1).unwrap();
        let err = temcmc(&prior, |_: &[f64]| 0.0, &config).unwrap_err();
        assert!(matches!(err, RafflesError::InvalidParameter { .. }));
    }

    /// **Methodology.** Systematic resampling must keep the population size
    /// and must favour heavy weights. Weights `[0.7, 0.2, 0.1]` over 1 000
    /// draws; index 0 should appear about 700 times.
    ///
    /// **Result** (2026-09-16, `--release`, seed 5): 713 of 1 000 draws came
    /// from the heavy-weighted third, against the expected 700.
    #[test]
    fn systematic_resampling_respects_the_weights() {
        let w = [0.7, 0.2, 0.1];
        let mut w_norm = Vec::new();
        let n = 1_000;
        for i in 0..n {
            w_norm.push(w[i % 3]);
        }
        let total: f64 = w_norm.iter().sum();
        for v in w_norm.iter_mut() {
            *v /= total;
        }
        let mut seed = stream_seed(5, 0);
        let indices = systematic_resample(&w_norm, &mut seed);
        assert_eq!(indices.len(), n);

        let heavy = indices.iter().filter(|i| **i % 3 == 0).count();
        println!("systematic resampling: {heavy} of {n} draws came from the heavy third");
        assert!(heavy > n / 2, "heavy weights under-represented: {heavy}");
    }

    /// **Methodology.** The Cholesky factor must reproduce its matrix:
    /// `L L^T = Sigma` for a known 2x2 positive-definite covariance.
    ///
    /// **Result.** Agreement to 1e-12 (2026-09-16).
    #[test]
    fn cholesky_reproduces_its_matrix() {
        let sigma = vec![vec![4.0, 2.0], vec![2.0, 3.0]];
        let l = cholesky(&sigma, 0.0).unwrap();
        for i in 0..2 {
            for j in 0..2 {
                let mut acc = 0.0;
                for k in 0..2 {
                    acc += l[i][k] * l[j][k];
                }
                assert!((acc - sigma[i][j]).abs() < 1e-12);
            }
        }
    }

    /// **Methodology.** A singular covariance — the collapsed-population case —
    /// must be regularised and counted, not fatal. A rank-1 2x2 matrix is
    /// factorised through [`cholesky_scaled`].
    ///
    /// **Result.** A usable factor is returned and
    /// `covariance_regularisations` is non-zero (2026-09-16).
    #[test]
    fn a_singular_covariance_is_regularised_and_counted() {
        let singular = vec![vec![1.0, 1.0], vec![1.0, 1.0]];
        let mut diagnostics = SamplerDiagnostics::default();
        let l = cholesky_scaled(&singular, 0.2, &mut diagnostics);
        assert!(l[0][0] > 0.0 && l[1][1] > 0.0);
        assert!(diagnostics.covariance_regularisations > 0);
    }
}
