//! Bayesian model updating — priors, likelihoods and the samplers that turn
//! them into a posterior.
//!
//! # What this module is for
//!
//! Given a prior belief about a model's parameters and a likelihood that says
//! how well a parameter vector explains measured data, produce samples from
//! the posterior
//!
//! ```text
//! p(theta | D)  =  p(D | theta) * p(theta) / p(D)
//! ```
//!
//! and an estimate of the evidence `p(D)` (the normalising constant, also
//! called the marginal likelihood). The evidence is what lets two competing
//! models be compared, so a sampler that produces it is worth more here than
//! one that only produces posterior samples.
//!
//! # Provenance — INDEPENDENT IMPLEMENTATIONS FROM THE PUBLISHED PAPERS
//!
//! **Read this before adding anything to this module.**
//!
//! The algorithms here were requested by way of Adolphus Lye's MATLAB
//! repositories (workspace issue #158). Of the seven repositories listed
//! there, exactly one — `Bayesian-Model-Updating-Tutorials` — carries a
//! licence (GPL-3.0). The other six carry **no licence file and no licence
//! statement**, which under copyright law means all rights reserved: their
//! code may not be copied, translated or adapted into this GPL-3.0 repository
//! without the author's permission.
//!
//! Everything in this module is therefore written **from the published
//! papers**, not from that MATLAB. Algorithms and mathematics are not
//! copyrightable; a specific expression of them in source code is. The rule
//! this module follows, which is the same one `crates/raffles/CLAUDE.md`
//! already states for RAVEN:
//!
//! - **Cite the paper** that defines the algorithm, in the item's doc comment,
//!   with a DOI.
//! - **Do not** carry a "ported from" attribution header pointing at an
//!   unlicensed repository — there is nothing to attribute, because nothing
//!   was taken.
//! - **Do not** transcribe an unlicensed file line by line, even with renamed
//!   variables.
//!
//! If Adolphus adds a GPL-compatible licence to those repositories, a
//! code-level port becomes possible and the attribution-header convention in
//! `CLAUDE.md` applies from that point on. Until then, the papers are the
//! specification.
//!
//! # What is here
//!
//! - [`IndependentPrior`] — a prior over several parameters, each with its own
//!   marginal from [`crate::distributions`], independent of the others.
//! - [`mcmc`] — the two Markov-chain moves everything else is built on:
//!   random-walk Metropolis–Hastings, and the affine-invariant ensemble
//!   ("stretch") move.
//! - [`transitional`] — Transitional MCMC (TMCMC) and Transitional Ensemble
//!   MCMC (TEMCMC): tempered sequential samplers that walk the posterior in
//!   stages and return the log-evidence as a by-product.
//! - [`case_studies`] — small problems with exactly known answers, ported from
//!   the one repository in that set that carries a licence. Includes a bimodal
//!   posterior whose two modes are computable in closed form, which is the
//!   failure a moment-based check cannot see.
//!
//! # Conventions used throughout
//!
//! - **Everything is in natural logarithms.** A likelihood evaluated at a
//!   badly-fitting parameter vector underflows `f64` long before the sampler
//!   is finished with it, so the API takes and returns `ln L(theta)`, never
//!   `L(theta)`. A log-likelihood of [`f64::NEG_INFINITY`] is legal and means
//!   "impossible"; a `NaN` is not, and is treated as impossible with the
//!   diagnostic counter [`SamplerDiagnostics::non_finite_likelihoods`]
//!   incremented so it cannot pass silently.
//! - **The caller supplies the likelihood as a closure**, `Fn(&[f64]) -> f64`,
//!   taking the parameter vector and returning its natural log-likelihood.
//!   Generic parameters rather than `Box<dyn Fn>` — the workspace design rules
//!   forbid trait objects, and a generic is also faster and keeps the closure's
//!   captured state on the caller's side.
//! - **Randomness is explicit.** Every sampler takes a `seed: i64` and uses
//!   [`crate::samplers::stream_seed`] over the workspace's OpenMC 64-bit LCG.
//!   The same seed and the same configuration produce a byte-identical result.
//!   There is no thread-local RNG and no time-seeded default anywhere here.
//! - **Parameters are plain `f64` vectors** in whatever units the caller's
//!   model uses; RAFFLES never interprets them physically. See the crate-level
//!   note on units.
//!
//! # Verification
//!
//! Each sampler's own doc comment states the methodology and the measured
//! results of its verification tests, as the workspace V&V rule requires. The
//! backbone test is the **conjugate Normal–Normal problem**, where the
//! posterior mean, the posterior variance *and* the evidence all have closed
//! forms — so a sampler can be checked on all three at once rather than on
//! posterior moments alone. See [`transitional`] for the numbers.
//!
//! None of this has been through human V&V. It is AI-assisted draft material
//! under the workspace `RESPONSIBLE_USE.md` rules until the maintainer and the
//! crate owner review it.

pub mod case_studies;
pub mod mcmc;
pub mod transitional;

use crate::distributions::{ContinuousDistribution1D, Distribution};
use crate::{RafflesError, Result};

pub use mcmc::{EnsembleMove, MetropolisHastings};
pub use transitional::{
    temcmc, tmcmc, StageReport, TemperingCriterion, TransitionKernel, TransitionalConfig,
    TransitionalResult,
};

/// A prior over `d` parameters whose marginals are mutually independent.
///
/// This is the prior shape that covers nearly every Bayesian model-updating
/// problem in the engineering literature: each parameter gets its own
/// distribution (a `Uniform` range from a physical bound, a `LogNormal` for a
/// positive stiffness, a `Gamma` for a noise precision) and no correlation is
/// asserted between them a priori. Correlation is what the *posterior* is
/// expected to discover.
///
/// A correlated prior is deliberately not offered yet: the samplers here only
/// ever need [`ln_pdf`](Self::ln_pdf) and [`sample`](Self::sample), so adding a
/// `Prior` enum with a multivariate-normal variant later is a pure extension
/// and needs no change at the call sites.
///
/// # Example
///
/// ```
/// use raffles::bayesian::IndependentPrior;
/// use raffles::distributions::{Distribution, Normal, Uniform};
///
/// let prior = IndependentPrior::new(vec![
///     Distribution::Uniform(Uniform::new(0.0, 10.0)?),
///     Distribution::Normal(Normal::new(1.0, 0.5)?),
/// ])?;
///
/// assert_eq!(prior.dimension(), 2);
/// // Inside the support: a finite log-density.
/// assert!(prior.ln_pdf(&[5.0, 1.0]).is_finite());
/// // Outside it: impossible, not an error.
/// assert_eq!(prior.ln_pdf(&[-1.0, 1.0]), f64::NEG_INFINITY);
/// # Ok::<(), raffles::RafflesError>(())
/// ```
#[derive(Debug, Clone, PartialEq)]
pub struct IndependentPrior {
    marginals: Vec<Distribution>,
}

impl IndependentPrior {
    /// Builds a prior from one marginal distribution per parameter.
    ///
    /// The order of `marginals` fixes the order of the parameter vector
    /// everywhere else in this module: `theta[i]` is distributed as
    /// `marginals[i]`.
    ///
    /// # Errors
    ///
    /// [`RafflesError::InvalidParameter`] if `marginals` is empty — a
    /// zero-dimensional posterior is not a problem this module can express,
    /// and accepting one would only defer the failure to a confusing place.
    pub fn new(marginals: Vec<Distribution>) -> Result<Self> {
        if marginals.is_empty() {
            return Err(RafflesError::InvalidParameter {
                parameter: "marginals".to_string(),
                value: 0.0,
                reason: "a prior needs at least one parameter".to_string(),
            });
        }
        Ok(Self { marginals })
    }

    /// Number of parameters `d`, i.e. the length of every parameter vector
    /// this prior describes.
    pub fn dimension(&self) -> usize {
        self.marginals.len()
    }

    /// The marginal distributions, in parameter order.
    pub fn marginals(&self) -> &[Distribution] {
        &self.marginals
    }

    /// Natural log of the joint prior density at `theta`.
    ///
    /// Because the marginals are independent this is the sum of the marginal
    /// log-densities. Returns [`f64::NEG_INFINITY`] — not an error — when
    /// `theta` falls outside any marginal's support, which is what a sampler
    /// needs: an impossible proposal is rejected, it is not a fault.
    ///
    /// A `theta` of the wrong length also returns [`f64::NEG_INFINITY`]. That
    /// is deliberate: this is called in the innermost loop of every sampler,
    /// where the length has already been established by construction, and
    /// returning a `Result` there would cost a branch per call for a condition
    /// that cannot occur.
    pub fn ln_pdf(&self, theta: &[f64]) -> f64 {
        if theta.len() != self.marginals.len() {
            return f64::NEG_INFINITY;
        }
        let mut acc = 0.0;
        for (value, marginal) in theta.iter().zip(self.marginals.iter()) {
            let density = marginal.pdf(*value);
            if !(density > 0.0) {
                return f64::NEG_INFINITY;
            }
            acc += density.ln();
        }
        if acc.is_nan() {
            f64::NEG_INFINITY
        } else {
            acc
        }
    }

    /// Draws one parameter vector from the prior, advancing `seed` in place.
    ///
    /// Uses inverse-transform sampling on each marginal, one uniform deviate
    /// per parameter, drawn from the workspace LCG. `seed` is the caller's
    /// stream state: pass the same starting state twice and the same vector
    /// comes back twice.
    ///
    /// The deviate is nudged off the open interval's ends before it reaches
    /// [`ContinuousDistribution1D::ppf`], so an unbounded marginal cannot
    /// return an infinite coordinate from a `u` that rounded to exactly 0 or 1.
    pub fn sample(&self, seed: &mut u64) -> Vec<f64> {
        self.marginals
            .iter()
            .map(|marginal| {
                let u = clamp_open_unit(outram_mc_libs::rng::lcg::prn(seed));
                // `ppf` only errors for u outside [0, 1]; `clamp_open_unit`
                // guarantees the interior, so this cannot fail.
                marginal.ppf(u).unwrap_or_else(|_| marginal.mean())
            })
            .collect()
    }
}

/// Nudges a uniform deviate into the open interval `(0, 1)`.
///
/// Inverse-transform sampling of an unbounded distribution maps `u = 0` to
/// `-inf` and `u = 1` to `+inf`. The LCG can return a value that rounds to
/// either end, and one infinite coordinate poisons an entire chain, so clamp
/// to the nearest representable interior point instead.
pub(crate) fn clamp_open_unit(u: f64) -> f64 {
    const EPS: f64 = 1.0e-12;
    if !u.is_finite() {
        0.5
    } else {
        u.clamp(EPS, 1.0 - EPS)
    }
}

/// Counters a sampler keeps about numerical trouble it met and handled.
///
/// These are **not** errors: every one of them describes something a sampler
/// can legitimately encounter and continue past. They are reported so that a
/// run which "worked" but leaned heavily on a fallback cannot look identical
/// to one that did not.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct SamplerDiagnostics {
    /// Number of likelihood evaluations that returned `NaN`.
    ///
    /// Treated as [`f64::NEG_INFINITY`] (impossible) so the chain keeps
    /// moving. A non-zero count means the caller's likelihood has a hole in
    /// it — a `0/0`, a `ln` of a negative, an unguarded `sqrt` — and the
    /// posterior should not be trusted until that is found.
    pub non_finite_likelihoods: usize,
    /// Number of times a stage's proposal covariance was not positive definite
    /// and had to be regularised by inflating its diagonal.
    ///
    /// Happens when the resampled population has collapsed onto fewer distinct
    /// points than there are parameters. A large count means the tempering is
    /// advancing too fast for the population size.
    pub covariance_regularisations: usize,
}

/// Evaluates a caller's log-likelihood, mapping `NaN` to "impossible".
///
/// Every sampler in this module goes through here rather than calling the
/// closure directly, so that the `NaN` policy is stated once and the
/// diagnostic counter cannot be forgotten at a call site.
pub(crate) fn eval_ln_likelihood<L>(
    ln_likelihood: &L,
    theta: &[f64],
    diagnostics: &mut SamplerDiagnostics,
) -> f64
where
    L: Fn(&[f64]) -> f64,
{
    let value = ln_likelihood(theta);
    if value.is_nan() {
        diagnostics.non_finite_likelihoods += 1;
        f64::NEG_INFINITY
    } else {
        value
    }
}

/// A standard normal deviate by inverse transform from one LCG draw.
///
/// Inverse transform rather than Box–Muller on purpose: it consumes exactly
/// one uniform per normal, which keeps the number of draws a sampler makes a
/// function of its configuration alone and therefore keeps runs reproducible
/// when a rejected proposal short-circuits.
pub(crate) fn standard_normal(seed: &mut u64) -> f64 {
    let u = clamp_open_unit(outram_mc_libs::rng::lcg::prn(seed));
    // The standard normal is constructible and its ppf is defined on (0, 1),
    // so neither step can fail; 0.0 is the median fallback.
    crate::distributions::Normal::new(0.0, 1.0)
        .and_then(|n| n.ppf(u))
        .unwrap_or(0.0)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::distributions::{Normal, Uniform};

    fn two_parameter_prior() -> IndependentPrior {
        IndependentPrior::new(vec![
            Distribution::Uniform(Uniform::new(0.0, 1.0).unwrap()),
            Distribution::Normal(Normal::new(0.0, 1.0).unwrap()),
        ])
        .unwrap()
    }

    /// **Methodology.** The joint log-density of an independent prior must be
    /// the sum of its marginal log-densities. Checked at an interior point
    /// against the two marginals evaluated separately.
    ///
    /// **Result.** Agreement to 1e-15 (2026-09-16).
    #[test]
    fn joint_log_density_is_the_sum_of_marginals() {
        let prior = two_parameter_prior();
        let theta = [0.25, 0.5];
        let expected = Uniform::new(0.0, 1.0).unwrap().pdf(0.25).ln()
            + Normal::new(0.0, 1.0).unwrap().pdf(0.5).ln();
        assert!((prior.ln_pdf(&theta) - expected).abs() < 1e-15);
    }

    /// **Methodology.** A parameter vector outside any marginal's support is
    /// impossible, not erroneous. Checked below and above a `Uniform(0, 1)`
    /// first coordinate, and for a vector of the wrong length.
    ///
    /// **Result.** All three return `-inf` (2026-09-16).
    #[test]
    fn outside_the_support_is_negative_infinity() {
        let prior = two_parameter_prior();
        assert_eq!(prior.ln_pdf(&[-0.1, 0.0]), f64::NEG_INFINITY);
        assert_eq!(prior.ln_pdf(&[1.1, 0.0]), f64::NEG_INFINITY);
        assert_eq!(prior.ln_pdf(&[0.5]), f64::NEG_INFINITY);
    }

    /// **Methodology.** Prior draws must reproduce the marginals they came
    /// from. 20 000 draws from `Uniform(0, 1) x Normal(0, 1)`, comparing the
    /// sample mean and standard deviation of each coordinate with the analytic
    /// values (0.5, 0.288675) and (0, 1). Tolerance is 4 standard errors of
    /// the mean, which for 20 000 draws is 0.008 and 0.028 respectively.
    ///
    /// **Result** (2026-09-16, seed 20260916, `--release`): uniform coordinate
    /// mean 0.50210, sd 0.28926 (analytic 0.5, 0.288675); normal coordinate
    /// mean -0.00730, sd 1.00503 (analytic 0, 1). Both inside tolerance.
    #[test]
    fn prior_draws_reproduce_the_marginals() {
        let prior = two_parameter_prior();
        let mut seed = crate::samplers::stream_seed(20_260_916, 0);
        let n = 20_000;
        let mut draws: Vec<Vec<f64>> = Vec::with_capacity(n);
        for _ in 0..n {
            draws.push(prior.sample(&mut seed));
        }

        for (column, (mean, sd)) in [(0.5, 0.288_675_134_594_813), (0.0, 1.0)]
            .into_iter()
            .enumerate()
        {
            let values: Vec<f64> = draws.iter().map(|row| row[column]).collect();
            let sample_mean = values.iter().sum::<f64>() / n as f64;
            let sample_var = values
                .iter()
                .map(|v| (v - sample_mean).powi(2))
                .sum::<f64>()
                / (n as f64 - 1.0);
            let tolerance = 4.0 * sd / (n as f64).sqrt();

            println!("prior column {column}: mean {sample_mean:.5} (ref {mean}) sd {:.5} (ref {sd})", sample_var.sqrt());
            assert!(
                (sample_mean - mean).abs() < tolerance,
                "column {column}: mean {sample_mean} vs {mean}"
            );
            assert!(
                (sample_var.sqrt() - sd).abs() < 0.05 * sd,
                "column {column}: sd {} vs {sd}",
                sample_var.sqrt()
            );
        }
    }

    /// **Methodology.** The `NaN` policy must be applied and counted. A
    /// likelihood that returns `NaN` is evaluated once through
    /// [`eval_ln_likelihood`].
    ///
    /// **Result.** Returns `-inf` and increments the counter (2026-09-16).
    #[test]
    fn nan_likelihood_is_impossible_and_counted() {
        let mut diagnostics = SamplerDiagnostics::default();
        let value = eval_ln_likelihood(&|_: &[f64]| f64::NAN, &[0.0], &mut diagnostics);
        assert_eq!(value, f64::NEG_INFINITY);
        assert_eq!(diagnostics.non_finite_likelihoods, 1);
    }

    /// **Methodology.** Standard normal draws by inverse transform must match
    /// `N(0, 1)`. 20 000 draws; sample mean and variance against 0 and 1.
    ///
    /// **Result** (2026-09-16, seed 7, `--release`): mean -0.00233, variance
    /// 1.00532.
    #[test]
    fn standard_normal_draws_are_standard_normal() {
        let mut seed = crate::samplers::stream_seed(7, 0);
        let n = 20_000;
        let values: Vec<f64> = (0..n).map(|_| standard_normal(&mut seed)).collect();
        let mean = values.iter().sum::<f64>() / n as f64;
        let var = values.iter().map(|v| (v - mean).powi(2)).sum::<f64>() / (n as f64 - 1.0);
        println!("standard normal draws: mean {mean:.5} variance {var:.5}");

        assert!(mean.abs() < 4.0 / (n as f64).sqrt(), "mean {mean}");
        assert!((var - 1.0).abs() < 0.05, "variance {var}");
    }
}
