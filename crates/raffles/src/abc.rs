//! Approximate Bayesian Computation — inference when there is no likelihood.
//!
//! # The problem it solves
//!
//! Bayesian updating needs `p(D | theta)`. A great many engineering models
//! cannot supply one: the model is a black box, or its output is stochastic in
//! a way nobody has written down, or the thing being matched is a *scatter of
//! measurements* rather than a point with a known error distribution. What such
//! a model can always do is **run**.
//!
//! ABC replaces the likelihood with a simulation and a distance:
//!
//! 1. propose `theta`,
//! 2. run the model at `theta` to get a synthetic sample,
//! 3. measure how far that sample is from the observed one, with one of
//!    [`crate::distance::DistanceMetric`],
//! 4. turn that distance into a likelihood with a kernel of width `epsilon`.
//!
//! As `epsilon` shrinks the approximation approaches the true posterior and the
//! computation gets harder. That trade-off is the whole method, and
//! [`AbcKernel::epsilon`] is where it is made explicit.
//!
//! # Two ways to run it
//!
//! - [`rejection_abc`] — draw from the prior, simulate, keep the close ones.
//!   Transparent, embarrassingly parallel, and hopeless when the prior is
//!   broad: the acceptance rate falls off a cliff. Use it as a reference and a
//!   sanity check, which is exactly what this module uses it for.
//! - [`abc_ln_likelihood`] with [`crate::bayesian::temcmc`] — build an
//!   approximate log-likelihood and hand it to the transitional sampler. This
//!   is the distance-based stochastic model updating framework of the recent
//!   literature (Bhattacharyya-, Hellinger- and Jensen–Shannon-based variants
//!   are just different choices of [`crate::distance::DistanceMetric`]), and it
//!   is what makes the method usable on a real problem.
//!
//! # Draw as many samples as you have observations
//!
//! **The simulator should return about as many rows as the observed data
//! has.** This looks like a performance detail and is not: it decides whether
//! the posterior's *width* means anything.
//!
//! A distribution-to-distribution distance compares the model's output
//! distribution with the **empirical** distribution of the data, and treats
//! that empirical distribution as if it were the truth. Simulate far more
//! points than you observed and the model side becomes essentially exact, so
//! the distance is minimised by whichever parameter best matches the
//! particular sampling noise in your handful of measurements. Shrink `epsilon`
//! and the posterior concentrates on that minimiser — reporting a confidence
//! the data cannot support.
//!
//! Measured on this module's own conjugate test problem (12 observations of
//! `N(2.5, 1)`, exact posterior sd 0.288195):
//!
//! | simulated rows | tolerance | posterior sd | verdict |
//! |---|---|---|---|
//! | 200 | 0.1 | 0.118810 | 2.4x **too tight** — over-confident |
//! | 12 | 0.2 | 0.366628 | 1.27x wider than exact — the honest direction |
//!
//! With the sample sizes matched, tightening the tolerance walks the posterior
//! sd down towards the exact value and stops there rather than sailing past it:
//! 0.5677 at `epsilon = 0.5`, 0.3464 at 0.2, 0.3110 at 0.05, against the exact
//! 0.288195. That saturation is what a correctly-posed ABC problem looks like.
//!
//! Both rows of that table are pinned by tests in this module, so the failure
//! mode cannot quietly come back.
//!
//! # Frozen randomness, and why it is not optional
//!
//! A stochastic simulator makes the ABC likelihood stochastic too: call it
//! twice at the same `theta` and you get two different numbers. Dropping such a
//! function into a Metropolis acceptance ratio silently changes what the chain
//! converges to — it becomes a pseudo-marginal method, valid only under
//! conditions most users never check, and a chain that lands on a lucky draw
//! can stick there indefinitely.
//!
//! This module therefore **derives the simulator's seed from `theta` itself**,
//! so the approximate likelihood is an honest deterministic function: the same
//! parameter vector always produces the same synthetic sample and the same
//! distance. The cost is that the noise becomes a fixed, rough surface rather
//! than fresh noise each call; the benefit is that the sampler is doing what
//! its convergence theory says it is doing. [`AbcLikelihood::simulations`] sets
//! how many model runs are averaged into each evaluation, which is the knob for
//! smoothing that surface.
//!
//! # References
//!
//! - M. A. Beaumont, W. Zhang and D. J. Balding (2002). Approximate Bayesian
//!   computation in population genetics. *Genetics, 162*(4), 2025–2035. doi:
//!   [10.1093/genetics/162.4.2025](https://doi.org/10.1093/genetics/162.4.2025)
//! - A. Lye, S. Ferson and S. Xiao (2024). Comparison between distance
//!   functions for Approximate Bayesian Computation towards stochastic model
//!   updating and model validation under limited data. *ASCE-ASME Journal of
//!   Risk and Uncertainty in Engineering Systems Part A: Civil Engineering,
//!   10*, 03124001. doi:
//!   [10.1061/AJRUA6.RUENG-1223](https://doi.org/10.1061/AJRUA6.RUENG-1223)
//! - S. Bi, M. Broggi and M. Beer (2019). The role of the Bhattacharyya
//!   distance in stochastic model updating. *Mechanical Systems and Signal
//!   Processing, 117*, 437–452. doi:
//!   [10.1016/j.ymssp.2018.08.017](https://doi.org/10.1016/j.ymssp.2018.08.017)
//!
//! Independent implementations from the published definitions — see the
//! provenance note in [`crate::bayesian`].

use crate::bayesian::IndependentPrior;
use crate::distance::DistanceMetric;
use crate::samplers::stream_seed;
use crate::{RafflesError, Result};

/// How a distance is turned into an approximate log-likelihood.
///
/// All three are the standard ABC kernels. They differ in how sharply they
/// punish a model run that misses, and — more importantly for a sampler —
/// in whether they can return `-inf`.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum AbcKernel {
    /// The textbook ABC indicator: `ln L = 0` if `distance <= epsilon`, and
    /// `-inf` otherwise.
    ///
    /// The definition every ABC derivation starts from, and the worst choice
    /// for a gradient-free sampler: the approximate likelihood is flat inside
    /// the ball and impossible outside it, so there is nothing to climb. A
    /// transitional sampler whose whole initial population is outside the ball
    /// cannot start. Use it with [`rejection_abc`], not with MCMC.
    Uniform {
        /// Tolerance radius, strictly positive, in the distance's own units.
        epsilon: f64,
    },
    /// Gaussian kernel: `ln L = -0.5 * (distance / epsilon)^2`.
    ///
    /// Never `-inf`, so there is always a gradient to follow back towards the
    /// data. This is the kernel the distance-based stochastic model updating
    /// frameworks use, and the default choice here.
    Gaussian {
        /// Kernel width, strictly positive, in the distance's own units.
        epsilon: f64,
    },
    /// Epanechnikov kernel: `ln L = ln(1 - (distance / epsilon)^2)` inside the
    /// ball, `-inf` outside.
    ///
    /// The minimum-variance kernel in the classical density-estimation sense,
    /// and a middle ground: it falls off smoothly like the Gaussian but has the
    /// Uniform's hard cut-off. Same starting problem as `Uniform` if the
    /// initial population is all outside the ball.
    Epanechnikov {
        /// Tolerance radius, strictly positive, in the distance's own units.
        epsilon: f64,
    },
}

impl AbcKernel {
    /// The kernel's width or tolerance.
    pub fn epsilon(&self) -> f64 {
        match self {
            Self::Uniform { epsilon }
            | Self::Gaussian { epsilon }
            | Self::Epanechnikov { epsilon } => *epsilon,
        }
    }

    /// Whether this kernel can return [`f64::NEG_INFINITY`] for a finite
    /// distance, i.e. whether it has a hard cut-off.
    ///
    /// Worth asking before pairing one with a transitional sampler: see the
    /// warning on [`AbcKernel::Uniform`].
    pub fn has_hard_cutoff(&self) -> bool {
        match self {
            Self::Uniform { .. } | Self::Epanechnikov { .. } => true,
            Self::Gaussian { .. } => false,
        }
    }

    /// Converts a distance into an approximate natural log-likelihood.
    ///
    /// A non-finite distance — which [`DistanceMetric::Bhattacharyya`]
    /// legitimately produces for disjoint samples — maps to
    /// [`f64::NEG_INFINITY`] under every kernel.
    ///
    /// # Errors
    ///
    /// [`RafflesError::InvalidParameter`] if `epsilon` is not strictly
    /// positive, or if `distance` is negative.
    pub fn ln_likelihood(&self, distance: f64) -> Result<f64> {
        let epsilon = self.epsilon();
        if !(epsilon > 0.0) {
            return Err(RafflesError::InvalidParameter {
                parameter: "epsilon".to_string(),
                value: epsilon,
                reason: "the ABC tolerance must be strictly positive".to_string(),
            });
        }
        if distance < 0.0 {
            return Err(RafflesError::InvalidParameter {
                parameter: "distance".to_string(),
                value: distance,
                reason: "a distance cannot be negative".to_string(),
            });
        }
        if !distance.is_finite() {
            return Ok(f64::NEG_INFINITY);
        }
        let ratio = distance / epsilon;
        Ok(match self {
            Self::Uniform { .. } => {
                if ratio <= 1.0 {
                    0.0
                } else {
                    f64::NEG_INFINITY
                }
            }
            Self::Gaussian { .. } => -0.5 * ratio * ratio,
            Self::Epanechnikov { .. } => {
                if ratio < 1.0 {
                    (1.0 - ratio * ratio).ln()
                } else {
                    f64::NEG_INFINITY
                }
            }
        })
    }
}

/// Everything needed to turn a simulator into an approximate log-likelihood.
///
/// Built once and handed to [`abc_ln_likelihood`], which produces the closure a
/// sampler consumes.
#[derive(Debug, Clone, PartialEq)]
pub struct AbcLikelihood {
    /// The observed data: rows of equal width, one row per measurement.
    pub observed: Vec<Vec<f64>>,
    /// How the synthetic sample is compared with [`observed`](Self::observed).
    pub metric: DistanceMetric,
    /// How the resulting distance becomes a log-likelihood.
    pub kernel: AbcKernel,
    /// How many independent model runs are pooled into each evaluation.
    ///
    /// One is enough when the simulator itself returns a whole sample. Raise it
    /// when the simulator returns a single realisation per call, or to smooth
    /// the frozen-noise surface described in the module documentation, at
    /// proportional cost.
    pub simulations: usize,
    /// Base seed for the simulator. Mixed with a hash of the parameter vector,
    /// so a given `theta` always gets the same simulator stream — see the
    /// module documentation on frozen randomness.
    pub seed: i64,
}

impl AbcLikelihood {
    /// Builds and validates the configuration.
    ///
    /// # Errors
    ///
    /// [`RafflesError::InvalidParameter`] if `observed` is empty, if
    /// `simulations` is zero, or if the kernel's `epsilon` is not strictly
    /// positive.
    /// [`RafflesError::DimensionMismatch`] if the observed rows are ragged.
    pub fn new(
        observed: Vec<Vec<f64>>,
        metric: DistanceMetric,
        kernel: AbcKernel,
        simulations: usize,
        seed: i64,
    ) -> Result<Self> {
        if observed.is_empty() {
            return Err(RafflesError::InvalidParameter {
                parameter: "observed".to_string(),
                value: 0.0,
                reason: "ABC needs at least one observation".to_string(),
            });
        }
        let width = observed[0].len();
        if width == 0 {
            return Err(RafflesError::InvalidParameter {
                parameter: "observed".to_string(),
                value: 0.0,
                reason: "observations must have at least one coordinate".to_string(),
            });
        }
        for row in &observed {
            if row.len() != width {
                return Err(RafflesError::DimensionMismatch {
                    expected: width,
                    found: row.len(),
                });
            }
        }
        if simulations == 0 {
            return Err(RafflesError::InvalidParameter {
                parameter: "simulations".to_string(),
                value: 0.0,
                reason: "each evaluation needs at least one model run".to_string(),
            });
        }
        // Surfaces a bad epsilon here rather than on the first evaluation.
        kernel.ln_likelihood(0.0)?;
        Ok(Self {
            observed,
            metric,
            kernel,
            simulations,
            seed,
        })
    }

    /// Width of one observation row.
    pub fn width(&self) -> usize {
        self.observed[0].len()
    }

    /// The simulator stream seed for a given parameter vector.
    ///
    /// Deterministic in `theta` — this is the mechanism behind the frozen
    /// randomness described in the module documentation. Built by mixing the
    /// raw bits of every coordinate through SplitMix64, so two parameter
    /// vectors that differ in the last bit get unrelated streams.
    pub fn simulator_seed(&self, theta: &[f64], run: usize) -> u64 {
        let mut hash = 0x9E37_79B9_7F4A_7C15_u64 ^ (run as u64).wrapping_mul(0xBF58_476D_1CE4_E5B9);
        for value in theta {
            hash ^= value.to_bits();
            hash = split_mix64(hash);
        }
        stream_seed(self.seed, (hash % (1 << 20)) as usize)
    }
}

/// One round of SplitMix64, used to fold a parameter vector into a stream index.
fn split_mix64(mut z: u64) -> u64 {
    z = z.wrapping_add(0x9E37_79B9_7F4A_7C15);
    z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
    z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
    z ^ (z >> 31)
}

/// Builds an approximate log-likelihood closure from a simulator.
///
/// `simulator` takes a parameter vector and a mutable LCG stream state, and
/// returns a sample of model outputs with the same row width as the observed
/// data. The returned closure has the `Fn(&[f64]) -> f64` shape that
/// [`crate::bayesian::tmcmc`] and [`crate::bayesian::temcmc`] consume, so the
/// whole distance-based stochastic model updating framework is:
///
/// ```no_run
/// # use raffles::abc::{abc_ln_likelihood, AbcKernel, AbcLikelihood};
/// # use raffles::bayesian::{temcmc, IndependentPrior, TransitionalConfig};
/// # use raffles::distance::DistanceMetric;
/// # fn run(prior: &IndependentPrior, observed: Vec<Vec<f64>>) -> raffles::Result<()> {
/// let abc = AbcLikelihood::new(
///     observed,
///     DistanceMetric::JensenShannon { bins: None },
///     AbcKernel::Gaussian { epsilon: 0.05 },
///     1,
///     20_260_916,
/// )?;
/// let ln_likelihood = abc_ln_likelihood(abc, |theta, seed| {
///     // run your model at `theta`, using `seed` for any randomness
///     # let _ = (theta, seed);
///     vec![vec![0.0]]
/// });
/// let config = TransitionalConfig::temcmc_defaults(2_000, 20_260_916)?;
/// let posterior = temcmc(prior, ln_likelihood, &config)?;
/// # let _ = posterior;
/// # Ok(())
/// # }
/// ```
///
/// A simulator whose output rows are the wrong width, or which returns no rows
/// at all, yields [`f64::NEG_INFINITY`] for that parameter vector — the
/// sampler treats it as an impossible point rather than failing the run, which
/// is the right behaviour for a model that legitimately has no solution in part
/// of the parameter space.
pub fn abc_ln_likelihood<S>(config: AbcLikelihood, simulator: S) -> impl Fn(&[f64]) -> f64
where
    S: Fn(&[f64], &mut u64) -> Vec<Vec<f64>>,
{
    move |theta: &[f64]| {
        let mut pooled: Vec<Vec<f64>> = Vec::new();
        for run in 0..config.simulations {
            let mut seed = config.simulator_seed(theta, run);
            let sample = simulator(theta, &mut seed);
            pooled.extend(sample);
        }
        if pooled.is_empty() || pooled.iter().any(|row| row.len() != config.width()) {
            return f64::NEG_INFINITY;
        }
        match config.metric.distance(&pooled, &config.observed) {
            Ok(distance) => config
                .kernel
                .ln_likelihood(distance)
                .unwrap_or(f64::NEG_INFINITY),
            Err(_) => f64::NEG_INFINITY,
        }
    }
}

/// The result of a rejection-ABC run.
#[derive(Debug, Clone, PartialEq)]
pub struct RejectionAbcResult {
    /// Accepted parameter vectors — the approximate posterior sample.
    pub samples: Vec<Vec<f64>>,
    /// The distance achieved by each accepted vector, aligned with
    /// [`samples`](Self::samples).
    pub distances: Vec<f64>,
    /// Proposals drawn from the prior, accepted or not.
    pub proposals: usize,
    /// Fraction of proposals accepted, in `[0, 1]`.
    ///
    /// The number that decides whether rejection ABC was the right tool. Below
    /// about 1 % the answer is no: move to [`abc_ln_likelihood`] with a
    /// transitional sampler.
    pub acceptance_rate: f64,
}

/// Plain rejection ABC: draw from the prior, simulate, keep the close ones.
///
/// Every proposal is independent, so the result is an exact sample from the
/// ABC posterior for the given tolerance — no burn-in, no autocorrelation, no
/// convergence question. That makes it the right *reference* against which to
/// check an MCMC-based ABC run, which is what this module's tests use it for,
/// even though its acceptance rate makes it impractical on a real problem.
///
/// The tolerance comes from `config.kernel.epsilon()`, and acceptance is the
/// kernel's own rule: a `Gaussian` kernel accepts stochastically with
/// probability `exp(ln L)`, while `Uniform` and `Epanechnikov` have their hard
/// cut-offs.
///
/// # Errors
///
/// [`RafflesError::InvalidParameter`] if `proposals` is zero, or if the prior
/// and the kernel are inconsistent in a way the constructors already reject.
pub fn rejection_abc<S>(
    prior: &IndependentPrior,
    config: &AbcLikelihood,
    simulator: S,
    proposals: usize,
    seed: i64,
) -> Result<RejectionAbcResult>
where
    S: Fn(&[f64], &mut u64) -> Vec<Vec<f64>>,
{
    if proposals == 0 {
        return Err(RafflesError::InvalidParameter {
            parameter: "proposals".to_string(),
            value: 0.0,
            reason: "rejection ABC needs at least one proposal".to_string(),
        });
    }

    let mut prior_seed = stream_seed(seed, 0);
    let mut accept_seed = stream_seed(seed, 1);
    let mut samples = Vec::new();
    let mut distances = Vec::new();

    for _ in 0..proposals {
        let theta = prior.sample(&mut prior_seed);

        let mut pooled: Vec<Vec<f64>> = Vec::new();
        for run in 0..config.simulations {
            let mut simulator_seed = config.simulator_seed(&theta, run);
            pooled.extend(simulator(&theta, &mut simulator_seed));
        }
        // Draw the acceptance deviate unconditionally so that the stream
        // advances by a fixed amount per proposal, keeping runs reproducible.
        let u = outram_mc_libs::rng::lcg::prn(&mut accept_seed);
        if pooled.is_empty() || pooled.iter().any(|row| row.len() != config.width()) {
            continue;
        }

        let distance = match config.metric.distance(&pooled, &config.observed) {
            Ok(d) => d,
            Err(_) => continue,
        };
        let ln_l = config.kernel.ln_likelihood(distance)?;
        let accept = if ln_l == f64::NEG_INFINITY {
            false
        } else if ln_l >= 0.0 {
            true
        } else {
            u.max(f64::MIN_POSITIVE).ln() < ln_l
        };
        if accept {
            samples.push(theta);
            distances.push(distance);
        }
    }

    let acceptance_rate = samples.len() as f64 / proposals as f64;
    Ok(RejectionAbcResult {
        samples,
        distances,
        proposals,
        acceptance_rate,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::bayesian::{temcmc, TransitionalConfig};
    use crate::distributions::{ContinuousDistribution1D, Distribution, Normal, Uniform};
    use outram_mc_libs::rng::lcg::prn;

    /// Twelve observations of a `N(2.5, 1)` process, the same data the
    /// conjugate tests in [`crate::bayesian::transitional`] use, so the two
    /// modules' answers are directly comparable.
    fn observed() -> Vec<Vec<f64>> {
        [
            2.1, 3.4, 1.9, 2.8, 2.2, 3.1, 2.6, 2.0, 3.0, 2.4, 2.9, 1.6,
        ]
        .iter()
        .map(|v| vec![*v])
        .collect()
    }

    /// Closed-form posterior for the conjugate problem: `mu ~ N(0, 5^2)`,
    /// data `N(mu, 1)`. Returns `(mean, sd)`.
    fn conjugate_posterior() -> (f64, f64) {
        let y: Vec<f64> = observed().iter().map(|row| row[0]).collect();
        let n = y.len() as f64;
        let ybar = y.iter().sum::<f64>() / n;
        let var = 1.0 / (1.0 / 25.0 + n);
        (var * (n * ybar), var.sqrt())
    }

    /// Draws `n` samples from `N(theta[0], 1)`.
    fn draw(theta: &[f64], seed: &mut u64, n: usize) -> Vec<Vec<f64>> {
        let dist = Normal::new(theta[0], 1.0).unwrap();
        (0..n)
            .map(|_| {
                let u = prn(seed).clamp(1e-12, 1.0 - 1e-12);
                vec![dist.ppf(u).unwrap()]
            })
            .collect()
    }

    /// Simulator drawing **as many samples as there are observations** (12).
    ///
    /// This is the correct pairing — see
    /// [`abc_matching_the_observed_sample_size_recovers_the_right_width`] and
    /// the module documentation on sample-size matching.
    fn simulator(theta: &[f64], seed: &mut u64) -> Vec<Vec<f64>> {
        draw(theta, seed, 12)
    }

    /// Simulator drawing far more samples than there are observations (200).
    ///
    /// Kept deliberately, to demonstrate the over-confidence failure mode.
    fn oversampled_simulator(theta: &[f64], seed: &mut u64) -> Vec<Vec<f64>> {
        draw(theta, seed, 200)
    }

    fn prior() -> IndependentPrior {
        IndependentPrior::new(vec![Distribution::Normal(Normal::new(0.0, 5.0).unwrap())]).unwrap()
    }

    fn mean_and_sd(samples: &[Vec<f64>]) -> (f64, f64) {
        let n = samples.len() as f64;
        let mean = samples.iter().map(|s| s[0]).sum::<f64>() / n;
        let var = samples.iter().map(|s| (s[0] - mean).powi(2)).sum::<f64>() / (n - 1.0);
        (mean, var.sqrt())
    }

    /// **Methodology.** The kernels' defining formulas, checked at the points
    /// where they are easiest to get wrong: distance 0, distance exactly
    /// `epsilon`, and beyond. Gaussian at `d = epsilon` must be `-0.5`;
    /// Uniform must be 0 inside and `-inf` outside; Epanechnikov must be 0 at
    /// `d = 0` and `-inf` at `d = epsilon`.
    ///
    /// **Result** (2026-09-16): all exact.
    #[test]
    fn kernels_match_their_definitions() {
        let g = AbcKernel::Gaussian { epsilon: 0.5 };
        assert_eq!(g.ln_likelihood(0.0).unwrap(), 0.0);
        assert!((g.ln_likelihood(0.5).unwrap() + 0.5).abs() < 1e-15);
        assert!((g.ln_likelihood(1.0).unwrap() + 2.0).abs() < 1e-15);
        assert!(!g.has_hard_cutoff());

        let u = AbcKernel::Uniform { epsilon: 0.5 };
        assert_eq!(u.ln_likelihood(0.4).unwrap(), 0.0);
        assert_eq!(u.ln_likelihood(0.5).unwrap(), 0.0);
        assert_eq!(u.ln_likelihood(0.6).unwrap(), f64::NEG_INFINITY);
        assert!(u.has_hard_cutoff());

        let e = AbcKernel::Epanechnikov { epsilon: 0.5 };
        assert_eq!(e.ln_likelihood(0.0).unwrap(), 0.0);
        assert_eq!(e.ln_likelihood(0.5).unwrap(), f64::NEG_INFINITY);
        assert!(e.ln_likelihood(0.25).unwrap() < 0.0);

        // An infinite distance — what Bhattacharyya gives for disjoint
        // samples — is impossible under every kernel.
        for kernel in [g, u, e] {
            assert_eq!(
                kernel.ln_likelihood(f64::INFINITY).unwrap(),
                f64::NEG_INFINITY
            );
        }
        // A non-positive epsilon is refused rather than producing NaN.
        assert!(AbcKernel::Gaussian { epsilon: 0.0 }.ln_likelihood(1.0).is_err());
    }

    /// **Methodology — frozen randomness.** The module claims the approximate
    /// likelihood is a deterministic function of `theta`. Evaluated twice at
    /// the same point, and at a neighbouring point, with a stochastic
    /// simulator.
    ///
    /// **Result** (2026-09-16): identical to the last bit at the same `theta`,
    /// and different at a neighbouring one — so the determinism is not coming
    /// from the simulator ignoring its seed.
    #[test]
    fn the_approximate_likelihood_is_deterministic_in_theta() {
        let config = AbcLikelihood::new(
            observed(),
            DistanceMetric::Wasserstein1,
            AbcKernel::Gaussian { epsilon: 0.2 },
            1,
            20_260_916,
        )
        .unwrap();
        let ln_l = abc_ln_likelihood(config, simulator);
        let a = ln_l(&[2.5]);
        let b = ln_l(&[2.5]);
        let c = ln_l(&[2.6]);
        assert_eq!(a, b, "the same theta gave two different log-likelihoods");
        assert!(a != c, "neighbouring thetas gave identical log-likelihoods");
    }

    /// **Methodology — against the closed-form posterior.** Twelve
    /// observations of `N(2.5, 1)`, prior `N(0, 5^2)`, Wasserstein-1 distance,
    /// Gaussian kernel, TEMCMC with 1 000 samples, and a simulator drawing
    /// **twelve** samples per evaluation — the same number as there are
    /// observations, per the sample-size rule in the module documentation.
    ///
    /// Pass criterion: posterior mean within 0.15 of the closed-form 2.491694,
    /// and posterior sd within a factor of two of the closed-form 0.288195 in
    /// either direction. ABC is an approximation, so exact agreement is not
    /// expected; being in the right place with roughly the right width is.
    ///
    /// **Result** (2026-09-16, `--release`, seed 20260916): mean 2.462622
    /// against the closed-form 2.491694; sd 0.366628 against 0.288195, i.e.
    /// 27 % wider — the direction an approximation should err in. Four
    /// tempering stages.
    #[test]
    fn abc_matching_the_observed_sample_size_recovers_the_right_width() {
        let (ref_mean, ref_sd) = conjugate_posterior();
        let config = AbcLikelihood::new(
            observed(),
            DistanceMetric::Wasserstein1,
            AbcKernel::Gaussian { epsilon: 0.2 },
            1,
            20_260_916,
        )
        .unwrap();
        let ln_l = abc_ln_likelihood(config, simulator);
        let sampler_config = TransitionalConfig::temcmc_defaults(1_000, 20_260_916).unwrap();
        let result = temcmc(&prior(), ln_l, &sampler_config).unwrap();

        let (mean, sd) = mean_and_sd(&result.samples);
        println!(
            "ABC+TEMCMC (matched sample size, 12 vs 12): mean {mean:.6} \
             (conjugate {ref_mean:.6}), sd {sd:.6} (conjugate {ref_sd:.6}), stages {}",
            result.stages.len()
        );
        assert!(
            (mean - ref_mean).abs() < 0.15,
            "mean {mean} vs conjugate {ref_mean}"
        );
        assert!(
            sd > 0.5 * ref_sd && sd < 2.0 * ref_sd,
            "sd {sd} is not within a factor of two of the conjugate {ref_sd}"
        );
    }

    /// **Methodology — the over-confidence trap, measured rather than
    /// asserted.** The same problem with a simulator that draws 200 samples
    /// against 12 observations, and a tolerance (0.1) below the data's own
    /// statistical uncertainty (0.288).
    ///
    /// The expectation encoded here is the *opposite* of the naive one. A
    /// distribution-to-distribution distance compares the model's CDF with the
    /// **empirical** CDF of the data, and treats that empirical CDF as if it
    /// were the truth. With 200 simulated points the model side is essentially
    /// exact, so the distance is minimised by whatever parameter matches the
    /// 12 points' own sampling noise — and shrinking `epsilon` concentrates the
    /// posterior onto that minimiser instead of spreading it by the uncertainty
    /// those 12 points actually carry. The posterior therefore comes out
    /// *tighter* than the exact one, which is over-confidence, not accuracy.
    ///
    /// This test exists because the first version of the test above asserted
    /// that ABC can only ever inflate a posterior, and failed: the measured
    /// sd was 0.118810 against the exact 0.288195, i.e. 2.4 times too tight.
    /// Rather than loosen that assertion, the failure mode is pinned here, so
    /// that a future change which accidentally fixes or worsens it is visible.
    ///
    /// **Result** (2026-09-16, `--release`, seed 20260916): mean 2.491123,
    /// sd 0.118810 against the exact 0.288195 — a ratio of 0.412, i.e. the
    /// reported posterior is 2.4 times too confident.
    #[test]
    fn an_oversampled_simulator_makes_the_posterior_over_confident() {
        let (_, ref_sd) = conjugate_posterior();
        let config = AbcLikelihood::new(
            observed(),
            DistanceMetric::Wasserstein1,
            AbcKernel::Gaussian { epsilon: 0.1 },
            1,
            20_260_916,
        )
        .unwrap();
        let ln_l = abc_ln_likelihood(config, oversampled_simulator);
        let sampler_config = TransitionalConfig::temcmc_defaults(1_000, 20_260_916).unwrap();
        let result = temcmc(&prior(), ln_l, &sampler_config).unwrap();

        let (mean, sd) = mean_and_sd(&result.samples);
        println!(
            "ABC+TEMCMC (oversampled, 200 vs 12): mean {mean:.6}, sd {sd:.6} \
             against the conjugate {ref_sd:.6} — ratio {:.3}",
            sd / ref_sd
        );
        assert!(
            sd < 0.75 * ref_sd,
            "the over-confidence this test documents has changed: sd {sd} vs conjugate {ref_sd}"
        );
    }

    /// **Methodology — the tolerance does what it claims.** Shrinking
    /// `epsilon` must tighten the ABC posterior towards the exact one. Three
    /// runs at `epsilon` = 0.5, 0.2 and 0.05, everything else fixed. The
    /// posterior standard deviation must decrease monotonically.
    ///
    /// **Result** (2026-09-16, `--release`, seed 7): posterior sd 0.567720 at
    /// epsilon 0.5, 0.346416 at 0.2, 0.311012 at 0.05. Monotone, and
    /// saturating just above the exact 0.288195 rather than continuing past
    /// it — which is what a correctly-posed ABC problem should do.
    #[test]
    fn a_smaller_tolerance_tightens_the_posterior() {
        let mut previous = f64::INFINITY;
        let mut row = Vec::new();
        for epsilon in [0.5, 0.2, 0.05] {
            let config = AbcLikelihood::new(
                observed(),
                DistanceMetric::Wasserstein1,
                AbcKernel::Gaussian { epsilon },
                1,
                7,
            )
            .unwrap();
            let ln_l = abc_ln_likelihood(config, simulator);
            let sampler_config = TransitionalConfig::temcmc_defaults(1_000, 7).unwrap();
            let result = temcmc(&prior(), ln_l, &sampler_config).unwrap();
            let (_, sd) = mean_and_sd(&result.samples);
            row.push((epsilon, sd));
            assert!(
                sd <= previous + 1e-9,
                "sd grew from {previous} to {sd} when epsilon fell to {epsilon}"
            );
            previous = sd;
        }
        println!("ABC posterior sd against tolerance: {row:?}");
    }

    /// **Methodology — MCMC-free cross-check.** Rejection ABC produces an exact
    /// sample from the ABC posterior with no convergence question attached, so
    /// it is the right reference for the sampler-based route. Both are run on
    /// the same problem with the same tolerance, and their posterior means must
    /// agree to within their combined Monte Carlo error.
    ///
    /// The prior here is deliberately narrowed to `Uniform(1.5, 3.5)` — with
    /// the `N(0, 5^2)` prior of the other tests, rejection ABC's acceptance
    /// rate is too low to give a usable reference, which is itself the point
    /// the module documentation makes about when to use it.
    ///
    /// **Result** (2026-09-16, `--release`, seed 99): rejection ABC gave mean
    /// 2.496747, sd 0.345839 from 1 763 acceptances out of 20 000 proposals
    /// (8.8 % acceptance, on a deliberately narrow prior); ABC+TEMCMC gave
    /// mean 2.523686, sd 0.350981. The means agree to 0.027 and the widths to
    /// 1.5 %, on two routes that share no sampling machinery.
    #[test]
    fn rejection_abc_agrees_with_the_sampler_based_route() {
        let narrow =
            IndependentPrior::new(vec![Distribution::Uniform(Uniform::new(1.5, 3.5).unwrap())])
                .unwrap();
        let config = AbcLikelihood::new(
            observed(),
            DistanceMetric::Wasserstein1,
            AbcKernel::Gaussian { epsilon: 0.2 },
            1,
            99,
        )
        .unwrap();

        let rejection = rejection_abc(&narrow, &config, simulator, 20_000, 99).unwrap();
        let (rejection_mean, rejection_sd) = mean_and_sd(&rejection.samples);

        let ln_l = abc_ln_likelihood(config, simulator);
        let sampler_config = TransitionalConfig::temcmc_defaults(2_000, 99).unwrap();
        let sampled = temcmc(&narrow, ln_l, &sampler_config).unwrap();
        let (sampler_mean, sampler_sd) = mean_and_sd(&sampled.samples);

        println!(
            "rejection ABC: mean {rejection_mean:.6} sd {rejection_sd:.6} from \
             {} accepted of {} proposals (acceptance {:.4}); \
             ABC+TEMCMC: mean {sampler_mean:.6} sd {sampler_sd:.6}",
            rejection.samples.len(),
            rejection.proposals,
            rejection.acceptance_rate
        );

        assert!(
            rejection.samples.len() > 100,
            "only {} acceptances; the reference is too noisy to compare against",
            rejection.samples.len()
        );
        assert!(
            (rejection_mean - sampler_mean).abs() < 0.1,
            "rejection {rejection_mean} vs sampler {sampler_mean}"
        );
    }

    /// **Methodology.** A simulator that returns nothing, or rows of the wrong
    /// width, must make that parameter vector impossible rather than crashing
    /// the run — a model with no solution in part of its parameter space is
    /// ordinary.
    ///
    /// **Result** (2026-09-16): both cases give `-inf`.
    #[test]
    fn a_failing_simulator_makes_a_point_impossible() {
        let config = AbcLikelihood::new(
            observed(),
            DistanceMetric::Wasserstein1,
            AbcKernel::Gaussian { epsilon: 0.2 },
            1,
            1,
        )
        .unwrap();
        let empty = abc_ln_likelihood(config.clone(), |_: &[f64], _: &mut u64| Vec::new());
        assert_eq!(empty(&[2.5]), f64::NEG_INFINITY);

        let ragged = abc_ln_likelihood(config, |_: &[f64], _: &mut u64| {
            vec![vec![1.0, 2.0], vec![3.0, 4.0]]
        });
        assert_eq!(ragged(&[2.5]), f64::NEG_INFINITY);
    }

    /// **Methodology.** Malformed configuration must be refused at
    /// construction, where the caller can see it, not at the first evaluation
    /// inside a sampler.
    ///
    /// **Result.** Empty data, ragged data, zero simulations and a
    /// non-positive epsilon are all rejected (2026-09-16).
    #[test]
    fn malformed_configuration_is_refused() {
        let metric = DistanceMetric::Wasserstein1;
        let kernel = AbcKernel::Gaussian { epsilon: 0.1 };
        assert!(AbcLikelihood::new(Vec::new(), metric, kernel, 1, 0).is_err());
        assert!(
            AbcLikelihood::new(vec![vec![1.0], vec![1.0, 2.0]], metric, kernel, 1, 0).is_err()
        );
        assert!(AbcLikelihood::new(observed(), metric, kernel, 0, 0).is_err());
        assert!(AbcLikelihood::new(
            observed(),
            metric,
            AbcKernel::Gaussian { epsilon: -1.0 },
            1,
            0
        )
        .is_err());
    }

    /// **Methodology.** Rejection ABC must be reproducible from its seed, since
    /// it is used as the reference in the cross-check above.
    ///
    /// **Result.** Identical accepted sets across two runs (2026-09-16).
    #[test]
    fn rejection_abc_is_reproducible() {
        let narrow =
            IndependentPrior::new(vec![Distribution::Uniform(Uniform::new(2.0, 3.0).unwrap())])
                .unwrap();
        let config = AbcLikelihood::new(
            observed(),
            DistanceMetric::Wasserstein1,
            AbcKernel::Gaussian { epsilon: 0.3 },
            1,
            5,
        )
        .unwrap();
        let a = rejection_abc(&narrow, &config, simulator, 2_000, 5).unwrap();
        let b = rejection_abc(&narrow, &config, simulator, 2_000, 5).unwrap();
        assert_eq!(a.samples, b.samples);
    }
}
