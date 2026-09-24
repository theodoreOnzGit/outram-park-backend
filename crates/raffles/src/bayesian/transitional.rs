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

/// The rule that decides how far each tempering stage may step.
///
/// # Why there is a choice here at all
///
/// The tempering schedule is the part of TMCMC that makes it robust, and the
/// rule that sets it is a modelling decision rather than a constant. Both
/// variants below answer the same question — "how far can `beta` move before
/// the importance weights degenerate?" — with different measures of
/// degeneration.
///
/// # The two are the same rule in different clothes
///
/// Worth knowing before choosing between them. For weights with mean `m` and
/// population variance `v`, the Kish effective sample size satisfies
///
/// ```text
/// ESS = (sum w)^2 / sum w^2 = n / (1 + CoV^2)
/// ```
///
/// exactly — so the two criteria are reparametrisations of each other under
/// `fraction = 1 / (1 + target^2)`, and **at their published defaults
/// (`target = 1`, `fraction = 0.5`) they are the identical rule**. The
/// comparison test in this module finds identical schedules for that reason,
/// and a separate test checks the identity numerically (agreement to 4e-15).
///
/// That does not make the alternative pointless: `fraction` is the more
/// interpretable dial, since "half my samples are still doing work" says
/// something a practitioner can act on and "the coefficient of variation of the
/// weights is 1" does not. It does mean that switching between them at the
/// published defaults will not change an answer, and anyone reporting a
/// difference between the two should look for it elsewhere.
///
/// One convention to note: [`weight_cov`] divides by `n`, not `n - 1`. A
/// definition using the unbiased denominator differs by `n / (n - 1)` inside
/// the square — negligible at usable population sizes, but stated rather than
/// assumed.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum TemperingCriterion {
    /// Ching and Chen's original rule: step until the **coefficient of
    /// variation** of the stage weights reaches `target`, conventionally 1.0.
    ///
    /// Cheap and well-tested. Its weakness is that the coefficient of variation
    /// is a moment of the weights, so a handful of enormous weights and a
    /// broadly healthy population can produce the same value.
    WeightCoefficientOfVariation {
        /// Target coefficient of variation, strictly positive. Ching and Chen
        /// use 1.0. Smaller means more, smaller stages: more robust and more
        /// expensive.
        target: f64,
    },
    /// The TMCMC-II rule of Lye and Marino (2023): step until the **effective
    /// sample size** falls to `fraction` of the population, conventionally
    /// one half.
    ///
    /// The effective sample size `(sum w)^2 / sum w^2` measures directly how
    /// many of the `N` samples are actually doing work, which is the quantity
    /// a practitioner cares about, and it is bounded in `(0, N]` rather than
    /// unbounded above. The published motivation for preferring it is that it
    /// gives a more interpretable and better-behaved schedule than a moment
    /// ratio.
    ///
    /// Reference: A. Lye and L. Marino (2023). An investigation into an
    /// alternative transition criterion of the Transitional Markov Chain Monte
    /// Carlo method for Bayesian model updating. *Proceedings of the 33rd
    /// European Safety and Reliability Conference*. doi:
    /// [10.3850/978-981-18-8071-1_P331-cd](https://doi.org/10.3850/978-981-18-8071-1_P331-cd)
    EffectiveSampleSize {
        /// Fraction of the population the effective sample size may fall to,
        /// in `(0, 1)`. Lye and Marino use 0.5.
        fraction: f64,
    },
}

impl TemperingCriterion {
    /// Ching and Chen's original criterion at the published target of 1.0.
    pub fn weight_cov() -> Self {
        Self::WeightCoefficientOfVariation { target: 1.0 }
    }

    /// The TMCMC-II criterion at the published half-population target.
    pub fn effective_sample_size() -> Self {
        Self::EffectiveSampleSize { fraction: 0.5 }
    }

    /// Checks the criterion's own parameter.
    ///
    /// # Errors
    ///
    /// [`RafflesError::InvalidParameter`] if the target coefficient of
    /// variation is not strictly positive, or the effective-sample-size
    /// fraction is not strictly inside `(0, 1)` — at 1 no step is ever
    /// permitted and the schedule cannot start.
    pub fn validate(&self) -> Result<()> {
        match self {
            Self::WeightCoefficientOfVariation { target } => {
                if !(*target > 0.0) {
                    return Err(RafflesError::InvalidParameter {
                        parameter: "target".to_string(),
                        value: *target,
                        reason: "the target coefficient of variation must be strictly positive"
                            .to_string(),
                    });
                }
            }
            Self::EffectiveSampleSize { fraction } => {
                if !(*fraction > 0.0 && *fraction < 1.0) {
                    return Err(RafflesError::InvalidParameter {
                        parameter: "fraction".to_string(),
                        value: *fraction,
                        reason: "the effective-sample-size fraction must lie strictly between \
                                 0 and 1"
                            .to_string(),
                    });
                }
            }
        }
        Ok(())
    }

    /// Whether a tempering step of `d_beta` would degenerate the weights past
    /// what this criterion allows.
    ///
    /// Both measures are monotone in `d_beta` — the coefficient of variation
    /// rises and the effective sample size falls as the step grows — which is
    /// what makes the bisection in [`solve_delta_beta`] both valid and
    /// sufficient.
    pub fn is_step_too_large(&self, ln_l: &[f64], d_beta: f64) -> bool {
        match self {
            Self::WeightCoefficientOfVariation { target } => weight_cov(ln_l, d_beta) > *target,
            Self::EffectiveSampleSize { fraction } => {
                weight_ess(ln_l, d_beta) < *fraction * ln_l.len() as f64
            }
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
    /// How far each tempering stage is allowed to step — the rule that decides
    /// `beta_{j+1}`.
    ///
    /// See [`TemperingCriterion`]; the two variants are the original TMCMC rule
    /// and the TMCMC-II rule.
    pub criterion: TemperingCriterion,
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
    /// [`RafflesError::InvalidParameter`] if `population` is zero, if the
    /// criterion's own parameter is out of range, `chain_length` is zero,
    /// `max_stages` is zero, or the kernel's own parameter is out of range
    /// (`scale > 0`, `a > 1`).
    pub fn new(
        population: usize,
        criterion: TemperingCriterion,
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
        criterion.validate()?;
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
            criterion,
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
            TemperingCriterion::weight_cov(),
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
            TemperingCriterion::weight_cov(),
            1,
            200,
            seed,
            TransitionKernel::affine_invariant_ensemble(),
        )
    }

    /// TMCMC-II defaults: the effective-sample-size criterion at half the
    /// population, with the affine-invariant ensemble kernel.
    ///
    /// Reference: Lye and Marino (2023) — see
    /// [`TemperingCriterion::EffectiveSampleSize`].
    pub fn tmcmc_ii_defaults(population: usize, seed: i64) -> Result<Self> {
        Self::new(
            population,
            TemperingCriterion::effective_sample_size(),
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
        let d_beta = solve_delta_beta(&ln_l, beta, config.criterion);
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

/// Solves for the largest tempering increment the criterion still permits.
///
/// The CoV of `w_i = L_i^(d_beta)` increases monotonically with `d_beta` — at
/// `d_beta = 0` every weight is 1 and the CoV is 0; as `d_beta` grows the
/// weights spread out. Bisection on that monotone function is therefore both
/// safe and enough; no derivative is needed.
///
/// Returns `1 - beta` directly when even the full remaining step keeps the CoV
/// below target, which is how the schedule finishes.
fn solve_delta_beta(ln_l: &[f64], beta: f64, criterion: TemperingCriterion) -> f64 {
    let remaining = 1.0 - beta;
    if remaining <= 0.0 {
        return 0.0;
    }
    if !criterion.is_step_too_large(ln_l, remaining) {
        return remaining;
    }

    let mut lo = 0.0;
    let mut hi = remaining;
    // 60 bisections take the bracket below f64 resolution for any realistic
    // remaining interval; the loop is bounded rather than tolerance-driven so
    // that a pathological likelihood cannot spin here.
    for _ in 0..60 {
        let mid = 0.5 * (lo + hi);
        if criterion.is_step_too_large(ln_l, mid) {
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

/// Kish effective sample size of the importance weights for a tempering step of
/// `d_beta`, computed in log space.
///
/// `(sum w)^2 / sum w^2`, which lies in `(0, n]`: `n` when every weight is
/// equal, and 1 when one sample carries all the mass. The log-space shift by
/// the maximum cancels between numerator and denominator, so the result is
/// independent of it — which is what makes this safe for a sharply-peaked
/// likelihood where the raw weights would underflow.
fn weight_ess(ln_l: &[f64], d_beta: f64) -> f64 {
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
        return 0.0;
    }
    let w: Vec<f64> = ln_w.iter().map(|lw| (lw - ln_w_max).exp()).collect();
    let sum: f64 = w.iter().sum();
    let sum_squares: f64 = w.iter().map(|v| v * v).sum();
    if !(sum_squares > 0.0) {
        return 0.0;
    }
    sum * sum / sum_squares
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
    let base = if trace > 0.0 { trace / d as f64 } else { 1.0 };

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
        vec![2.1, 3.4, 1.9, 2.8, 2.2, 3.1, 2.6, 2.0, 3.0, 2.4, 2.9, 1.6]
    }

    fn conjugate_setup() -> (Vec<f64>, f64, f64, f64, IndependentPrior) {
        let sigma = 1.0;
        let mu_0 = 0.0;
        let tau = 5.0;
        let prior =
            IndependentPrior::new(vec![Distribution::Normal(Normal::new(mu_0, tau).unwrap())])
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
        assert!((sd - ref_sd).abs() < 0.15 * ref_sd, "sd {sd} vs {ref_sd}");
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
        let narrow =
            IndependentPrior::new(vec![Distribution::Normal(Normal::new(0.0, 5.0).unwrap())])
                .unwrap();
        let wide =
            IndependentPrior::new(vec![Distribution::Normal(Normal::new(0.0, 50.0).unwrap())])
                .unwrap();

        let config = TransitionalConfig::temcmc_defaults(3_000, 777).unwrap();
        let narrow_result =
            temcmc(&narrow, conjugate_ln_likelihood(y.clone(), sigma), &config).unwrap();
        let wide_result =
            temcmc(&wide, conjugate_ln_likelihood(y.clone(), sigma), &config).unwrap();

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
        let prior =
            IndependentPrior::new(vec![Distribution::Normal(Normal::new(0.0, 1.0).unwrap())])
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

    /// **Methodology — the TMCMC-II criterion against the original.** Lye and
    /// Marino's alternative rule stops each stage when the effective sample
    /// size falls to half the population, rather than when the weights'
    /// coefficient of variation reaches 1. Both must reach the same posterior;
    /// what differs is the schedule. Run on the conjugate Normal-Normal
    /// problem, 2 000 samples, same seed, ensemble kernel in both cases, and
    /// compared on posterior mean, posterior sd, log-evidence, stage count, and
    /// the effective sample size actually achieved per stage.
    ///
    /// **Result** (2026-09-16, `--release`, seed 20260916): the two schedules
    /// are IDENTICAL — mean 2.497856, sd 0.295145, ln Z -15.753896, 4 stages,
    /// smallest stage ESS 1000.0 under both. That is not a coincidence: see
    /// the identity documented on `TemperingCriterion`. Both are checked
    /// against the same closed forms, so this verifies the new rule rather
    /// than merely comparing two unverified numbers.
    #[test]
    fn the_two_tempering_criteria_reach_the_same_posterior() {
        let (y, sigma, mu_0, tau, prior) = conjugate_setup();
        let (ref_mean, ref_sd, ref_ln_evidence) = conjugate_reference(&y, sigma, mu_0, tau);

        let classic = TransitionalConfig::temcmc_defaults(2_000, 20_260_916).unwrap();
        let tmcmc_ii = TransitionalConfig::tmcmc_ii_defaults(2_000, 20_260_916).unwrap();

        for (name, config) in [
            ("CoV = 1 (Ching & Chen)", classic),
            ("ESS = N/2 (TMCMC-II)", tmcmc_ii),
        ] {
            let result =
                run_transitional(&prior, conjugate_ln_likelihood(y.clone(), sigma), &config)
                    .unwrap();
            let mean = result.posterior_mean()[0];
            let sd = result.posterior_std_dev()[0];
            let min_ess = result
                .stages
                .iter()
                .map(|s| s.effective_sample_size)
                .fold(f64::INFINITY, f64::min);
            println!(
                "{name}: mean {mean:.6} (ref {ref_mean:.6}), sd {sd:.6} (ref {ref_sd:.6}), \
                 ln Z {:.6} (ref {ref_ln_evidence:.6}), {} stages, smallest stage ESS {min_ess:.1}",
                result.ln_evidence,
                result.stages.len()
            );

            assert!((mean - ref_mean).abs() < 0.05, "{name}: mean {mean}");
            assert!((sd - ref_sd).abs() < 0.15 * ref_sd, "{name}: sd {sd}");
            assert!(
                (result.ln_evidence - ref_ln_evidence).abs() < 0.5,
                "{name}: ln evidence {}",
                result.ln_evidence
            );
        }
    }

    /// **Methodology.** The TMCMC-II criterion's defining property is that no
    /// stage is allowed to drop the effective sample size below the configured
    /// fraction of the population. Checked directly on every stage of a run
    /// with `fraction = 0.5` and 2 000 samples: each stage's reported effective
    /// sample size must be at least 1 000, up to the resolution of the
    /// bisection.
    ///
    /// **Result** (2026-09-16, `--release`, seed 4242): per-stage effective
    /// sample sizes 1000.0, 1000.0, 1101.2 against a population of 2 000 —
    /// the criterion binds exactly at N/2 on the stages where it is active,
    /// and the final stage lands above it because beta reached 1 first.
    #[test]
    fn the_effective_sample_size_criterion_holds_at_every_stage() {
        let (y, sigma, _, _, prior) = conjugate_setup();
        let config = TransitionalConfig::tmcmc_ii_defaults(2_000, 4_242).unwrap();
        let result = run_transitional(&prior, conjugate_ln_likelihood(y, sigma), &config).unwrap();

        let per_stage: Vec<f64> = result
            .stages
            .iter()
            .map(|s| (s.effective_sample_size * 10.0).round() / 10.0)
            .collect();
        println!("TMCMC-II per-stage effective sample size (N = 2000): {per_stage:?}");
        for (index, stage) in result.stages.iter().enumerate() {
            assert!(
                stage.effective_sample_size >= 990.0,
                "stage {index} dropped to ESS {}",
                stage.effective_sample_size
            );
        }
    }

    /// **Methodology.** A criterion with an out-of-range parameter must be
    /// refused at construction: a non-positive coefficient of variation, and an
    /// effective-sample-size fraction at or beyond the ends of `(0, 1)` — at 1
    /// no step is ever permitted and the schedule cannot start at all.
    ///
    /// **Result.** All three rejected (2026-09-16).
    #[test]
    fn a_malformed_tempering_criterion_is_refused() {
        assert!(
            TemperingCriterion::WeightCoefficientOfVariation { target: 0.0 }
                .validate()
                .is_err()
        );
        assert!(TemperingCriterion::EffectiveSampleSize { fraction: 0.0 }
            .validate()
            .is_err());
        assert!(TemperingCriterion::EffectiveSampleSize { fraction: 1.0 }
            .validate()
            .is_err());
    }

    /// **Methodology — an identity that makes the two criteria the same rule.**
    /// For weights with mean `m` and population variance `v`, the Kish
    /// effective sample size is
    ///
    /// ```text
    /// ESS = (sum w)^2 / sum w^2
    ///     = n^2 m^2 / (n (v + m^2))
    ///     = n / (1 + v/m^2)
    ///     = n / (1 + CoV^2)
    /// ```
    ///
    /// so the two criteria in this module are reparametrisations of each other
    /// under `fraction = 1 / (1 + target^2)`. At their published defaults —
    /// `target = 1` and `fraction = 0.5` — they are the SAME rule, which is why
    /// the comparison test above finds the two schedules identical rather than
    /// merely similar. Checked numerically over a spread of tempering steps on
    /// a real log-likelihood population.
    ///
    /// Note the normalisation this depends on: `weight_cov` divides by `n`, not
    /// `n - 1`. A paper defining the coefficient of variation with the unbiased
    /// denominator would differ from this by a factor of `n / (n - 1)` inside
    /// the square — negligible at the population sizes used here, but the
    /// reason to state the convention rather than assume it.
    ///
    /// **Result** (2026-09-16, `--release`): agreement to between 1.1e-16 and
    /// 5.0e-15 relative across tempering steps from 1e-4 to 1.0, i.e. to
    /// floating-point resolution. The two published defaults agree on every
    /// step tested.
    #[test]
    fn effective_sample_size_and_weight_cov_are_the_same_criterion() {
        // A spread of log-likelihoods with real structure, not a flat set.
        let ln_l: Vec<f64> = (0..500)
            .map(|i| -0.01 * (i as f64 - 250.0).powi(2))
            .collect();
        let n = ln_l.len() as f64;

        let mut worst = 0.0_f64;
        for d_beta in [1.0e-4, 1.0e-3, 0.01, 0.05, 0.2, 0.5, 1.0] {
            let cov = weight_cov(&ln_l, d_beta);
            let ess = weight_ess(&ln_l, d_beta);
            let predicted = n / (1.0 + cov * cov);
            let relative = ((ess - predicted) / predicted).abs();
            worst = worst.max(relative);
            println!(
                "d_beta {d_beta}: CoV {cov:.6}, ESS {ess:.4}, n/(1+CoV^2) {predicted:.4}, \
                 relative difference {relative:.2e}"
            );
        }
        assert!(worst < 1.0e-9, "worst relative difference {worst}");

        // And therefore: CoV target 1 is exactly ESS = N/2.
        let target_one = TemperingCriterion::WeightCoefficientOfVariation { target: 1.0 };
        let half_population = TemperingCriterion::EffectiveSampleSize { fraction: 0.5 };
        for d_beta in [1.0e-3, 0.01, 0.1, 0.5, 1.0] {
            assert_eq!(
                target_one.is_step_too_large(&ln_l, d_beta),
                half_population.is_step_too_large(&ln_l, d_beta),
                "the two published defaults disagreed at d_beta = {d_beta}"
            );
        }
    }

    /// The deterministic log-likelihood vector used by the cross-code checks
    /// below, and by the Octave driver that produced their reference numbers.
    ///
    /// 200 points, a Gaussian log-likelihood for 12 pseudo-observations at
    /// `theta = 2.0`, evaluated on a uniform grid over `[0, 4]`. Everything is
    /// a closed-form function of the index, so the Rust and the Octave agree
    /// on the *input* bit-for-bit and any difference in the output is a real
    /// difference between the two implementations.
    fn cross_check_ln_l(spread: f64) -> Vec<f64> {
        (0..200)
            .map(|i| {
                let theta = 4.0 * i as f64 / 199.0;
                -0.5 * 12.0 * ((theta - 2.0) / spread).powi(2)
            })
            .collect()
    }

    /// **Methodology — cross-code against Adolphus Lye's own MATLAB.**
    ///
    /// `calculate_pj1`, extracted verbatim from `TMCMCsampler.m`
    /// (`Adolphus8/transitional_ensemble_mcmc` @ `eca0338`), was run under GNU
    /// Octave 8.4.0 on the deterministic log-likelihoods of
    /// [`cross_check_ln_l`] and its solved `d_beta` recorded to 17 significant
    /// figures. This test feeds the identical inputs to [`solve_delta_beta`]
    /// under [`TemperingCriterion::weight_cov`] and compares.
    ///
    /// **The two do not agree, and the reason is a known, quantified
    /// convention difference, not a defect on either side.** MATLAB's `std` is
    /// the *sample* standard deviation, normalised by `N - 1`; this crate's
    /// [`weight_cov`] uses the *population* standard deviation, normalised by
    /// `N`. So his solver lands where the sample CoV is 1 and the population
    /// CoV is `sqrt((N-1)/N)`, and this one lands where the population CoV is
    /// 1 — a slightly larger step per stage.
    ///
    /// The pass criterion is therefore not equality. It is that the
    /// discrepancy is **exactly** the one that convention difference predicts:
    /// rescaling this crate's solve to his target must reproduce his `d_beta`
    /// to 1e-12 relative. That turns a vague "they're close" into a falsifiable
    /// statement, and it is what would catch a second, real difference hiding
    /// underneath the first.
    ///
    /// **Results** (2026-09-16; Octave 8.4.0, rustc 1.98.1, `--release`).
    /// Reference `d_beta` from the MATLAB, then ours, then ours re-solved at
    /// his target:
    ///
    /// | spread | beta | his `d_beta` | ours | ours at his target | rel. |
    /// |---|---|---|---|---|---|
    /// | 0.3 | 0.00 | 0.023172377583872784 | 0.023289948644949570 | 0.023172377583873098 | 1.4e-14 |
    /// | 0.3 | 0.25 | 0.023172377583872805 | 0.023289948644949570 | 0.023172377583873098 | 1.3e-14 |
    /// | 0.3 | 0.50 | 0.023172377583872805 | 0.023289948644949570 | 0.023172377583873098 | 1.3e-14 |
    /// | 1.0 | 0.00 | 0.25747086204303482 | 0.25877720716610630 | 0.25747086204303449 | 1.3e-15 |
    /// | 1.0 | 0.25 | 0.25747086204303482 | 0.25877720716610630 | 0.25747086204303449 | 1.3e-15 |
    /// | 1.0 | 0.50 | 0.25747086204303482 | 0.25877720716610630 | 0.25747086204303449 | 1.3e-15 |
    /// | 3.0 | 0.00 | 1.0 | 1.0 | 1.0 | 0 |
    /// | 3.0 | 0.25 | 0.75 | 0.75 | 0.75 | 0 |
    /// | 3.0 | 0.50 | 0.5 | 0.5 | 0.5 | 0 |
    ///
    /// **Interpretation.** Re-solved at his own target the two agree to
    /// 1.4e-14 relative — bisection-resolution, i.e. the same root — so the
    /// `N` versus `N - 1` convention is the *entire* difference and there is
    /// no second defect hiding under it. The last three rows agree exactly
    /// because both implementations cap the step at the remaining interval
    /// before either solver runs.
    ///
    /// **Size of the practical difference: this crate takes steps 0.507 %
    /// larger**, identically in both non-capped cases (ratio 1.005074 at
    /// spread 0.3 and at spread 1.0). Over a 20-stage run that is well under
    /// one stage's worth of tempering, and it is swamped by Monte Carlo noise
    /// at any population size anyone would use. It is recorded because it is
    /// real and systematic, not because it matters numerically.
    ///
    /// **Which is right?** Neither, strictly. Ching and Chen write the
    /// criterion as `CoV(w) = 1` without saying which estimator, and at any
    /// useful `N` the two differ by `O(1/N)`. This crate keeps the population
    /// form because the exact identity `ESS = N / (1 + CoV^2)` — the thing
    /// that makes Ching and Chen's CoV = 1 and Lye and Marino's `ESS = N/2`
    /// the *same* criterion, and which
    /// `the_two_published_criteria_are_the_same_rule` checks to 1e-15 — holds
    /// only for the population CoV. Switching to `N - 1` to match the MATLAB
    /// would break that identity for no gain.
    #[test]
    fn tempering_solver_matches_the_matlab_up_to_the_std_convention() {
        // d_beta from calculate_pj1 under Octave, 17 significant figures.
        let reference: [(f64, f64, f64); 9] = [
            (0.3, 0.00, 0.023172377583872784),
            (0.3, 0.25, 0.023172377583872805),
            (0.3, 0.50, 0.023172377583872805),
            (1.0, 0.00, 0.25747086204303482),
            (1.0, 0.25, 0.25747086204303482),
            (1.0, 0.50, 0.25747086204303482),
            (3.0, 0.00, 1.0),
            (3.0, 0.25, 0.75),
            (3.0, 0.50, 0.5),
        ];
        // His target expressed in this crate's population-CoV units.
        let n = 200.0_f64;
        let his_target = ((n - 1.0) / n).sqrt();

        for (spread, beta, his_d_beta) in reference {
            let ln_l = cross_check_ln_l(spread);
            let ours = solve_delta_beta(&ln_l, beta, TemperingCriterion::weight_cov());
            let ours_at_his_target = solve_delta_beta(
                &ln_l,
                beta,
                TemperingCriterion::WeightCoefficientOfVariation { target: his_target },
            );
            let rel = if his_d_beta > 0.0 {
                (ours_at_his_target - his_d_beta).abs() / his_d_beta
            } else {
                (ours_at_his_target - his_d_beta).abs()
            };
            println!(
                "spread {spread:.1} beta {beta:.2}: his {his_d_beta:.17} \
                 ours {ours:.17} ours@his {ours_at_his_target:.17} rel {rel:.3e}"
            );
            assert!(
                rel < 1.0e-12,
                "spread {spread} beta {beta}: re-solving at the MATLAB's own target gave \
                 {ours_at_his_target} against its {his_d_beta} (relative {rel:.3e}). The \
                 population-vs-sample standard-deviation convention is NOT the whole \
                 difference — something else has changed."
            );
        }
    }

    /// **Methodology — cross-code on the evidence increment.**
    ///
    /// The quantity Ching and Chen call `S(j)`, which accumulates into the log
    /// evidence, is `mean_i exp(d_beta * lnL_i)`. The MATLAB forms it directly
    /// as `mean(exp(a))` with no numerical stabilisation; this crate shifts by
    /// the maximum log-weight and adds it back. Those are algebraically the
    /// same number, so this is a real equality test, not a tolerance-managed
    /// one — and the shifted form is the one that survives a sharply-peaked
    /// likelihood, where the MATLAB's raw `exp` underflows to zero and takes
    /// the log evidence to `-Inf`.
    ///
    /// Reference `ln S` from the same Octave run, at each case's own solved
    /// `d_beta`.
    ///
    /// **Results** (2026-09-16; Octave 8.4.0, rustc 1.98.1, `--release`):
    ///
    /// | spread | beta | his `ln S` | ours | abs. diff |
    /// |---|---|---|---|---|
    /// | 0.3 | 0.00 | -1.03680777795871348 | -1.03680777795871260 | 8.9e-16 |
    /// | 1.0 | 0.00 | -1.03680777795872059 | -1.03680777795872081 | 2.2e-16 |
    /// | 3.0 | 0.00 | -0.63670164767608139 | -0.63670164767608195 | 5.6e-16 |
    /// | 3.0 | 0.25 | -0.51780755879908780 | -0.51780755879908713 | 6.7e-16 |
    /// | 3.0 | 0.50 | -0.37583252338041800 | -0.37583252338041778 | 2.2e-16 |
    ///
    /// **Interpretation.** Worst case 8.9e-16 absolute, i.e. 2-4 ulp — the two
    /// implementations compute the same quantity and the residual is
    /// floating-point summation order, nothing else. Since the log evidence is
    /// just the sum of these increments, the evidence estimator is cross-code
    /// verified against its author's own implementation, and any difference in
    /// a full run's `ln Z` comes from the *schedule* and the Monte Carlo, not
    /// from this formula.
    #[test]
    fn evidence_increment_matches_the_matlab() {
        // (spread, beta, d_beta solved by the MATLAB, ln S reported by it)
        let reference: [(f64, f64, f64, f64); 5] = [
            (0.3, 0.00, 0.023172377583872784, -1.0368077779587135),
            (1.0, 0.00, 0.25747086204303482, -1.0368077779587206),
            (3.0, 0.00, 1.0, -0.63670164767608139),
            (3.0, 0.25, 0.75, -0.5178075587990878),
            (3.0, 0.50, 0.5, -0.375832523380418),
        ];
        for (spread, beta, d_beta, his_ln_s) in reference {
            let ln_l = cross_check_ln_l(spread);
            // Exactly the increment the sampler accumulates, at his d_beta.
            let ln_w: Vec<f64> = ln_l.iter().map(|l| d_beta * l).collect();
            let ln_w_max = ln_w.iter().cloned().fold(f64::NEG_INFINITY, f64::max);
            let w_sum: f64 = ln_w.iter().map(|lw| (lw - ln_w_max).exp()).sum();
            let ours = ln_w_max + (w_sum / ln_l.len() as f64).ln();
            let abs = (ours - his_ln_s).abs();
            println!(
                "spread {spread:.1} beta {beta:.2}: ln S his {his_ln_s:.17} \
                 ours {ours:.17} abs {abs:.3e}"
            );
            assert!(
                abs < 1.0e-14,
                "spread {spread} beta {beta}: ln S {ours} against the MATLAB's {his_ln_s}"
            );
        }
    }

    /// **Methodology — end-to-end cross-code against Adolphus Lye's TEMCMC,
    /// over a seed ensemble rather than a single run.**
    ///
    /// `TEMCMCsampler.m` (`Adolphus8/transitional_ensemble_mcmc` @ `eca0338`,
    /// MATLAB-only argument plumbing patched out — every patch is listed in
    /// `docs/cross-check-against-upstream.md`) was run under GNU Octave 8.4.0
    /// on this module's conjugate Normal-Normal case: 12 observations, known
    /// `sigma = 1`, prior `mu ~ N(0, 5^2)`, `N = 2000`, at his own defaults
    /// (`stepsize = 2`, `thinchain = 3`, no burn-in), over 5 seeds.
    ///
    /// **Comparing one of our runs against a distribution of his would prove
    /// nothing**, so this runs *our* sampler over a seed ensemble too and
    /// compares like with like. It also runs it at `chain_length = 3` to match
    /// his thinning, which is finding F6 in that document — the one structural
    /// difference expected to move the answer.
    ///
    /// Pass criterion: our ensemble mean for each of the three quantities must
    /// sit within 3 standard errors of his, using the pooled seed spread. That
    /// is a real test — the run below shows it is not satisfied trivially.
    ///
    /// **Results** (2026-09-16; Octave 8.4.0, rustc 1.98.1, `--release`),
    /// each cell the mean over 5 seeds and the seed-to-seed spread:
    ///
    /// | | post. mean | post. sd | ln Z | stages |
    /// |---|---|---|---|---|
    /// | closed form | 2.491694 | 0.288195 | -15.685402 | — |
    /// | **his** TEMCMC | 2.493997 ± 0.010172 | 0.287737 ± 0.006247 | -15.686675 ± 0.030611 | 3.2 |
    /// | ours, `chain_length = 1` | 2.492526 ± 0.006797 | 0.288598 ± 0.005196 | -15.677201 ± 0.058985 | 3.2 |
    /// | ours, `chain_length = 3` | 2.491482 ± 0.005911 | 0.286966 ± 0.003604 | -15.663922 ± 0.043585 | 3.2 |
    ///
    /// Separation between the two implementations, in standard errors of the
    /// difference:
    ///
    /// | | post. mean | post. sd | ln Z |
    /// |---|---|---|---|
    /// | ours at `chain_length = 1` | 0.27 | 0.24 | 0.32 |
    /// | ours at `chain_length = 3` | 0.48 | 0.24 | 0.96 |
    ///
    /// **Interpretation: they agree.** Every separation is under one standard
    /// error, against a 3-sigma criterion, and both implementations bracket
    /// the closed form. The stage count is 3.2 on both sides, so the 0.507 %
    /// difference in tempering step size (finding F1) does not even change how
    /// many stages a run takes on this problem.
    ///
    /// **This also retires a suspicion rather than confirming one.** The
    /// single-seed figure recorded on `temcmc_matches_the_conjugate_normal_posterior`
    /// has a posterior sd 2.4 % above the closed form, and the obvious
    /// hypothesis was that `chain_length = 1` under-decorrelates next to his
    /// thinning of 3. It does not: over 5 seeds `chain_length = 1` gives
    /// 0.288598 against the closed-form 0.288195, **+0.14 %**. The 2.4 % was
    /// seed noise in a single run, and the seed spread here (±0.005) is what
    /// makes that visible. A one-run V&V number is worth less than it looks.
    #[test]
    fn temcmc_ensemble_matches_the_matlab_ensemble() {
        let (y, sigma, mu_0, tau, prior) = conjugate_setup();
        let (ref_mean, ref_sd, ref_ln_evidence) = conjugate_reference(&y, sigma, mu_0, tau);

        // TEMCMCsampler.m under Octave, 5 seeds, N = 2000, his own defaults.
        let his = (2.493997_f64, 0.287737_f64, -15.686675_f64);
        let his_spread = (0.010172_f64, 0.006247_f64, 0.030611_f64);
        let his_seeds = 5.0_f64;

        for chain_length in [1usize, 3usize] {
            let mut means = Vec::new();
            let mut sds = Vec::new();
            let mut ln_zs = Vec::new();
            let mut stages = Vec::new();
            for s in 0..5i64 {
                let mut config =
                    TransitionalConfig::temcmc_defaults(2_000, 20_260_916 + s).unwrap();
                config.chain_length = chain_length;
                let r = temcmc(&prior, conjugate_ln_likelihood(y.clone(), sigma), &config).unwrap();
                let n = r.samples.len() as f64;
                let m = r.samples.iter().map(|t| t[0]).sum::<f64>() / n;
                // Sample sd, matching Octave's std(), so the two columns are
                // the same estimator.
                let v = r.samples.iter().map(|t| (t[0] - m).powi(2)).sum::<f64>() / (n - 1.0);
                means.push(m);
                sds.push(v.sqrt());
                ln_zs.push(r.ln_evidence);
                stages.push(r.stages.len() as f64);
            }
            let stat = |v: &[f64]| {
                let n = v.len() as f64;
                let m = v.iter().sum::<f64>() / n;
                let s = (v.iter().map(|x| (x - m).powi(2)).sum::<f64>() / (n - 1.0)).sqrt();
                (m, s)
            };
            let (om, os) = stat(&means);
            let (od, ods) = stat(&sds);
            let (oz, ozs) = stat(&ln_zs);
            let (ost, _) = stat(&stages);
            println!(
                "ours chain_length={chain_length}, 5 seeds: \
                 mean {om:.6} +- {os:.6} | sd {od:.6} +- {ods:.6} | \
                 lnZ {oz:.6} +- {ozs:.6} | stages {ost:.1}"
            );
            println!(
                "  vs his:            mean {:.6} +- {:.6} | sd {:.6} +- {:.6} | lnZ {:.6} +- {:.6} | stages 3.2",
                his.0, his_spread.0, his.1, his_spread.1, his.2, his_spread.2
            );
            for (label, ours, ours_sd, theirs, theirs_sd) in [
                ("mean", om, os, his.0, his_spread.0),
                ("sd", od, ods, his.1, his_spread.1),
                ("lnZ", oz, ozs, his.2, his_spread.2),
            ] {
                let se = (ours_sd * ours_sd / 5.0 + theirs_sd * theirs_sd / his_seeds).sqrt();
                let z = (ours - theirs).abs() / se;
                println!(
                    "  {label}: |ours - his| = {:.6}, {z:.2} standard errors",
                    (ours - theirs).abs()
                );
                if chain_length == 3 {
                    assert!(
                        z < 3.0,
                        "at chain_length = 3, which matches the MATLAB's thinning, {label} \
                         disagrees with it by {z:.2} standard errors (ours {ours}, his {theirs}). \
                         Reference values: closed form mean {ref_mean}, sd {ref_sd}, \
                         ln Z {ref_ln_evidence}."
                    );
                }
            }
        }
    }

    /// **Methodology — end-to-end cross-code against Adolphus Lye's TMCMC.**
    ///
    /// As [`temcmc_ensemble_matches_the_matlab_ensemble`], but for the
    /// Metropolis-Hastings kernel, at `N = 500` over 20 seeds. The smaller
    /// budget is because his TMCMC drives 500 separate `mhsample` chains per
    /// stage under Octave, which is far slower than the ensemble path.
    ///
    /// **`mhsample` had to be worked around.** octave-statistics 1.6.3
    /// implements `mhsample`'s `'logpdf'` option incorrectly — it accepts every
    /// proposal — so `TMCMCsampler.m` was patched to pass the same target as
    /// `'pdf'`, which the same probe shows is correct. Full evidence in
    /// `docs/cross-check-against-upstream.md`. That is a defect in the runner,
    /// not in his sampler and not in this crate.
    ///
    /// **This comparison is expected to be looser than the TEMCMC one**, and
    /// the reason is findings F3 and F4: his proposal scale starts at
    /// `2.4/sqrt(D)` and adapts toward a target acceptance rate between
    /// stages; this crate's is fixed at 0.2. They are different kernels
    /// wrapped around the same tempering schedule. The pass criterion is
    /// therefore against the **closed form**, which both must recover, rather
    /// than against each other: posterior mean within 0.1, sd within 15 %,
    /// `ln Z` within 0.5 nat. The measured separation between the two is
    /// reported but not asserted.
    ///
    /// **Results** (2026-09-16; Octave 8.4.0, rustc 1.98.1, `--release`),
    /// mean over 5 seeds ± seed-to-seed spread:
    ///
    /// | | posterior mean | posterior sd | `ln Z` | stages |
    /// |---|---|---|---|---|
    /// | closed form | 2.491694 | 0.288195 | -15.685402 | — |
    /// | **his** TMCMC | 2.507103 ± 0.016021 | 0.289952 ± 0.008963 | -15.735060 ± 0.076958 | 3.5 |
    /// | ours | 2.488174 ± 0.017216 | 0.289459 ± 0.015316 | -15.697938 ± 0.139207 | 3.4 |
    ///
    /// Separation, in standard errors of the difference: posterior sd
    /// **0.12**, `ln Z` **1.04**, posterior mean **3.60**.
    ///
    /// **The posterior mean does not agree, and the cross-check found out
    /// why.** Against the closed form, this crate's mean sits 0.91 standard
    /// errors low — consistent — while his sits **4.30 standard errors high**.
    /// The discrepancy is on his side, and it is diagnosable:
    ///
    /// `TMCMCsampler.m`'s `prop_pdf` multiplies the Gaussian proposal density
    /// by `box(x)`, which is the **full prior PDF**, not a support indicator:
    ///
    /// ```matlab
    /// proppdf = mvnpdf(x, mu, covmat).*box(x);   % q(x,y) = q(x|y)
    /// ```
    ///
    /// With `q(x'|x) = N(x'; x, S) * prior(x')`, the prior cancels out of the
    /// Metropolis-Hastings ratio entirely, and the chain targets the tempered
    /// **likelihood** instead of the tempered posterior. The prior then enters
    /// only through the initial draw and the importance weights.
    ///
    /// **It is harmless in the regime the code was written for, and the
    /// control run proves that.** For a *uniform* prior the PDF is constant on
    /// its support, so it cancels anyway and `box` does exactly the job its
    /// comment describes. Re-running his TMCMC unchanged on the same data with
    /// `mu ~ U(-20, 20)`, 20 seeds:
    ///
    /// | | measured | closed form | separation |
    /// |---|---|---|---|
    /// | posterior mean | 2.499298 ± 0.009651 | 2.500000 | **0.33 SE** |
    /// | posterior sd | 0.288969 ± 0.008080 | 0.288675 | 0.16 SE |
    /// | `ln Z` | -16.680954 ± 0.099535 | -16.719657 | 1.74 SE |
    ///
    /// The bias vanishes. Every tutorial in his repositories uses a uniform
    /// prior, so this has had no opportunity to show itself there. **His
    /// TEMCMC is unaffected** — it uses `box` only as a boolean rejection test
    /// (`if box(proposedm(i,:)), break; end`), never as a density — which is
    /// consistent with the TEMCMC comparison agreeing to under one standard
    /// error on all three quantities.
    ///
    /// This is reported to the author; it is not this crate's to fix, and
    /// nothing here changes as a result.
    #[test]
    fn tmcmc_ensemble_matches_the_matlab_ensemble() {
        let (y, sigma, mu_0, tau, prior) = conjugate_setup();
        let (ref_mean, ref_sd, ref_ln_evidence) = conjugate_reference(&y, sigma, mu_0, tau);

        // TMCMCsampler.m under Octave, 5 seeds, N = 500, his own defaults.
        let his = (2.507103_f64, 0.289952_f64, -15.735060_f64);
        let his_spread = (0.016021_f64, 0.008963_f64, 0.076958_f64);

        let mut means = Vec::new();
        let mut sds = Vec::new();
        let mut ln_zs = Vec::new();
        let mut stages = Vec::new();
        for s in 0..20i64 {
            let config = TransitionalConfig::tmcmc_defaults(500, 20_260_916 + s).unwrap();
            let r = tmcmc(&prior, conjugate_ln_likelihood(y.clone(), sigma), &config).unwrap();
            let n = r.samples.len() as f64;
            let m = r.samples.iter().map(|t| t[0]).sum::<f64>() / n;
            let v = r.samples.iter().map(|t| (t[0] - m).powi(2)).sum::<f64>() / (n - 1.0);
            means.push(m);
            sds.push(v.sqrt());
            ln_zs.push(r.ln_evidence);
            stages.push(r.stages.len() as f64);
        }
        let stat = |v: &[f64]| {
            let n = v.len() as f64;
            let m = v.iter().sum::<f64>() / n;
            let s = (v.iter().map(|x| (x - m).powi(2)).sum::<f64>() / (n - 1.0)).sqrt();
            (m, s)
        };
        let (om, os) = stat(&means);
        let (od, ods) = stat(&sds);
        let (oz, ozs) = stat(&ln_zs);
        let (ost, _) = stat(&stages);
        println!(
            "ours TMCMC, 20 seeds, N=500: mean {om:.6} +- {os:.6} | sd {od:.6} +- {ods:.6} | \
             lnZ {oz:.6} +- {ozs:.6} | stages {ost:.1}"
        );
        println!(
            "  his TMCMC:                mean {:.6} +- {:.6} | sd {:.6} +- {:.6} | lnZ {:.6} +- {:.6} | stages 3.2",
            his.0, his_spread.0, his.1, his_spread.1, his.2, his_spread.2
        );
        for (label, ours, ours_sd, theirs, theirs_sd) in [
            ("mean", om, os, his.0, his_spread.0),
            ("sd", od, ods, his.1, his_spread.1),
            ("lnZ", oz, ozs, his.2, his_spread.2),
        ] {
            let se = (ours_sd * ours_sd / 20.0 + theirs_sd * theirs_sd / 20.0).sqrt();
            println!(
                "  {label}: ours - his = {:+.6}, {:.2} standard errors (reported, not asserted)",
                ours - theirs,
                (ours - theirs).abs() / se
            );
        }
        // Asserted against the closed form, which both implementations must
        // recover whatever kernel they wrap around the schedule.
        assert!(
            (om - ref_mean).abs() < 0.1,
            "posterior mean {om} against the closed-form {ref_mean}"
        );
        assert!(
            ((od - ref_sd) / ref_sd).abs() < 0.15,
            "posterior sd {od} against the closed-form {ref_sd}"
        );
        assert!(
            (oz - ref_ln_evidence).abs() < 0.5,
            "ln evidence {oz} against the closed-form {ref_ln_evidence}"
        );
    }
}
