//! Markov-chain moves: random-walk Metropolis–Hastings and the
//! affine-invariant ensemble ("stretch") move.
//!
//! Both kernels here are *moves*, not samplers: each advances a state (or a
//! population of states) one step against a target the caller supplies as a
//! log-density. [`super::transitional`] builds the actual samplers on top of
//! them. Keeping the move separate from the tempering schedule is what makes
//! TMCMC and TEMCMC one algorithm with two kernels rather than two algorithms.
//!
//! # The target
//!
//! Every function here takes the target as `ln_target: Fn(&[f64]) -> f64`,
//! the natural log of a density known only up to a constant. That is all a
//! Metropolis acceptance ratio needs, and it is what a tempered posterior
//! `ln p(theta) + beta * ln L(theta)` naturally is.
//!
//! `-inf` means "impossible" and is always rejected. `NaN` from the caller's
//! target is treated as `-inf`: see [`super::eval_ln_likelihood`] for why that
//! policy exists and how it is counted.
//!
//! # Why both kernels
//!
//! **Metropolis–Hastings** with a multivariate-normal proposal is the
//! classical choice and is what Ching and Chen's TMCMC specifies. Its weakness
//! is that its performance depends entirely on the proposal covariance
//! matching the target's shape — on an anisotropic or strongly correlated
//! target with a poorly-scaled proposal it crawls.
//!
//! **The affine-invariant ensemble move** (Goodman and Weare, 2010) removes
//! that tuning problem: a population of walkers proposes moves along the lines
//! joining pairs of walkers, so the proposal inherits the target's own shape.
//! Its performance is *invariant* under any affine transformation of the
//! parameter space, which is exactly the property an engineering model-updating
//! problem needs, where one parameter may be a stiffness of order 1e9 and the
//! next a damping ratio of order 1e-3.
//!
//! # References
//!
//! - J. Goodman and J. Weare (2010). Ensemble samplers with affine invariance.
//!   *Communications in Applied Mathematics and Computational Science, 5*(1),
//!   65–80. doi: [10.2140/camcos.2010.5.65](https://doi.org/10.2140/camcos.2010.5.65)
//! - W. K. Hastings (1970). Monte Carlo sampling methods using Markov chains
//!   and their applications. *Biometrika, 57*(1), 97–109. doi:
//!   [10.1093/biomet/57.1.97](https://doi.org/10.1093/biomet/57.1.97)
//! - A. Lye, A. Cicirello and E. Patelli (2021). Sampling methods for solving
//!   Bayesian model updating problems: A tutorial. *Mechanical Systems and
//!   Signal Processing, 159*, 107760. doi:
//!   [10.1016/j.ymssp.2021.107760](https://doi.org/10.1016/j.ymssp.2021.107760)
//!
//! These are independent implementations from the published algorithms. See
//! the provenance note in [`super`] for why that matters here.

use outram_mc_libs::rng::lcg::prn;

use super::{clamp_open_unit, standard_normal};
use crate::{RafflesError, Result};

/// One state of a Markov chain: a parameter vector and the target's log-density
/// there.
///
/// Carrying the log-density with the point is not an optimisation detail, it is
/// what makes the chain correct in the presence of an expensive likelihood:
/// re-evaluating the target at the current point every step would double the
/// cost and, for a stochastic likelihood, would also quietly change the
/// invariant distribution.
#[derive(Debug, Clone, PartialEq)]
pub struct ChainState {
    /// The parameter vector, in the order fixed by the prior.
    pub theta: Vec<f64>,
    /// Natural log of the target density at `theta`, up to the target's own
    /// unknown constant. May be [`f64::NEG_INFINITY`].
    pub ln_target: f64,
}

impl ChainState {
    /// Builds a state by evaluating `ln_target` at `theta`.
    pub fn new<T>(theta: Vec<f64>, ln_target: &T) -> Self
    where
        T: Fn(&[f64]) -> f64,
    {
        let value = ln_target(&theta);
        Self {
            ln_target: if value.is_nan() {
                f64::NEG_INFINITY
            } else {
                value
            },
            theta,
        }
    }
}

/// Random-walk Metropolis–Hastings with a multivariate-normal proposal.
///
/// The proposal is `theta* ~ N(theta, Sigma)` with `Sigma` supplied as its
/// lower Cholesky factor, so the caller (in practice the TMCMC stage loop)
/// controls the scale and the correlation of the step. Because the proposal is
/// symmetric, the Hastings ratio reduces to the target ratio.
///
/// # Why the Cholesky factor and not the covariance
///
/// A sampler takes thousands of steps from one covariance matrix. Factorising
/// once per stage instead of once per step is the obvious saving, but the real
/// reason is correctness: the factorisation is where a non-positive-definite
/// covariance is *detected*, and detecting it once per stage (where it can be
/// regularised and counted) is far better than discovering it in the middle of
/// a chain.
#[derive(Debug, Clone, PartialEq)]
pub struct MetropolisHastings {
    /// Lower-triangular Cholesky factor `L` of the proposal covariance, so that
    /// `Sigma = L L^T`, stored row-major as `d` rows of `d` entries.
    cholesky: Vec<Vec<f64>>,
}

impl MetropolisHastings {
    /// Builds the kernel from the lower Cholesky factor of the proposal
    /// covariance.
    ///
    /// # Errors
    ///
    /// [`RafflesError::DimensionMismatch`] if `cholesky` is not square.
    /// [`RafflesError::InvalidParameter`] if it is empty or has a
    /// non-positive diagonal entry — a zero diagonal would make that parameter
    /// unmovable, which is a silent failure rather than a slow one.
    pub fn from_cholesky(cholesky: Vec<Vec<f64>>) -> Result<Self> {
        let d = cholesky.len();
        if d == 0 {
            return Err(RafflesError::InvalidParameter {
                parameter: "cholesky".to_string(),
                value: 0.0,
                reason: "proposal covariance must have at least one dimension".to_string(),
            });
        }
        for row in &cholesky {
            if row.len() != d {
                return Err(RafflesError::DimensionMismatch {
                    expected: d,
                    found: row.len(),
                });
            }
        }
        for (i, row) in cholesky.iter().enumerate() {
            if !(row[i] > 0.0) {
                return Err(RafflesError::InvalidParameter {
                    parameter: "cholesky".to_string(),
                    value: row[i],
                    reason: format!("diagonal entry {i} must be strictly positive"),
                });
            }
        }
        Ok(Self { cholesky })
    }

    /// Number of parameters the kernel proposes in.
    pub fn dimension(&self) -> usize {
        self.cholesky.len()
    }

    /// Advances `state` one Metropolis–Hastings step, in place.
    ///
    /// Returns `true` if the proposal was accepted. `seed` is the caller's LCG
    /// stream state and is advanced by `d + 1` draws per call — `d` normals for
    /// the proposal and one uniform for the accept/reject test — whether or not
    /// the proposal is accepted, so that the number of draws a run consumes
    /// depends only on its configuration.
    pub fn step<T>(&self, state: &mut ChainState, ln_target: &T, seed: &mut u64) -> bool
    where
        T: Fn(&[f64]) -> f64,
    {
        let d = self.cholesky.len();
        let z: Vec<f64> = (0..d).map(|_| standard_normal(seed)).collect();

        let mut proposal = state.theta.clone();
        for (i, item) in proposal.iter_mut().enumerate().take(d) {
            let mut step = 0.0;
            for (j, z_j) in z.iter().enumerate().take(i + 1) {
                step += self.cholesky[i][j] * z_j;
            }
            *item += step;
        }

        let ln_target_proposal = {
            let value = ln_target(&proposal);
            if value.is_nan() {
                f64::NEG_INFINITY
            } else {
                value
            }
        };

        // Draw the acceptance deviate unconditionally: see the note above on
        // keeping the draw count configuration-determined.
        let u = clamp_open_unit(prn(seed));
        let ln_ratio = ln_target_proposal - state.ln_target;
        let accept = if ln_target_proposal == f64::NEG_INFINITY {
            false
        } else if state.ln_target == f64::NEG_INFINITY || ln_ratio >= 0.0 {
            // A chain started at an impossible point must be able to escape it,
            // so any finite proposal is accepted from there.
            true
        } else {
            u.ln() < ln_ratio
        };

        if accept {
            state.theta = proposal;
            state.ln_target = ln_target_proposal;
        }
        accept
    }
}

/// The affine-invariant ensemble ("stretch") move of Goodman and Weare (2010).
///
/// A population of `n` walkers explores the target together. To move walker
/// `k`, a partner `j != k` is drawn from the *complementary* half of the
/// population and the proposal is
///
/// ```text
/// Y = X_j + z * (X_k - X_j),     z ~ g(z) on [1/a, a],  g(z) proportional to 1/sqrt(z)
/// ```
///
/// accepted with probability `min(1, z^(d-1) * q(Y) / q(X_k))`. Because the
/// proposal is built only from differences of walker positions, the whole
/// kernel commutes with any affine map of the parameter space: rescaling a
/// parameter from metres to millimetres, or rotating a correlated pair, leaves
/// the sampler's behaviour unchanged. That is the property the name refers to,
/// and it is why this kernel needs no proposal covariance and no tuning.
///
/// # The complementary-halves split matters
///
/// Updating a walker using a partner from the *same* half that is itself being
/// updated in the same sweep breaks detailed balance. The population is
/// therefore split in two: the first half is updated using partners from the
/// second, then the second using the (already updated) first. Goodman and
/// Weare's paper and the `emcee` implementation both do this; it is not an
/// optimisation, and removing it silently biases the answer.
///
/// # Population size
///
/// `n` must be at least `2 * d + 2` for the ensemble to span the parameter
/// space with a complementary half of size at least `d + 1`. A smaller
/// population confines the walkers to a lower-dimensional affine subspace of
/// the parameter space, permanently — this is the classic failure mode of
/// ensemble samplers and it fails *silently*, producing a plausible-looking
/// chain that never explores some directions. [`EnsembleMove::new`] rejects it
/// rather than letting it happen.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct EnsembleMove {
    /// The stretch parameter `a > 1`; proposals scale by `z` in `[1/a, a]`.
    a: f64,
}

impl EnsembleMove {
    /// Goodman and Weare's recommended stretch parameter, `a = 2`.
    ///
    /// The paper reports sampler performance to be insensitive to it over a
    /// broad range, and `emcee` has used 2 as its default since 2013.
    pub const DEFAULT_A: f64 = 2.0;

    /// Builds the move with stretch parameter `a`.
    ///
    /// # Errors
    ///
    /// [`RafflesError::InvalidParameter`] unless `a > 1`. At `a = 1` the
    /// proposal density collapses to a point mass at `z = 1`, i.e. the walker
    /// never moves.
    pub fn new(a: f64) -> Result<Self> {
        if !(a > 1.0) {
            return Err(RafflesError::InvalidParameter {
                parameter: "a".to_string(),
                value: a,
                reason: "the stretch parameter must be greater than 1".to_string(),
            });
        }
        Ok(Self { a })
    }

    /// The stretch parameter.
    pub fn a(&self) -> f64 {
        self.a
    }

    /// Smallest population size that can explore `dimension` parameters.
    ///
    /// `2 * d + 2`: each half must hold at least `d + 1` walkers, which is the
    /// smallest set whose pairwise differences can span `d` dimensions.
    pub fn minimum_population(dimension: usize) -> usize {
        2 * dimension + 2
    }

    /// Advances an entire population one sweep, in place.
    ///
    /// Every walker is offered exactly one move. Returns the number accepted,
    /// so the caller can report an acceptance rate over the sweep.
    ///
    /// # Errors
    ///
    /// [`RafflesError::InvalidParameter`] if the population is smaller than
    /// [`minimum_population`](Self::minimum_population) for the dimension the
    /// walkers live in, or if the walkers disagree on that dimension.
    pub fn sweep<T>(
        &self,
        population: &mut [ChainState],
        ln_target: &T,
        seed: &mut u64,
    ) -> Result<usize>
    where
        T: Fn(&[f64]) -> f64,
    {
        let n = population.len();
        let d = population.first().map(|w| w.theta.len()).unwrap_or(0);
        if d == 0 {
            return Err(RafflesError::InvalidParameter {
                parameter: "population".to_string(),
                value: 0.0,
                reason: "the ensemble is empty or its walkers are zero-dimensional".to_string(),
            });
        }
        for walker in population.iter() {
            if walker.theta.len() != d {
                return Err(RafflesError::DimensionMismatch {
                    expected: d,
                    found: walker.theta.len(),
                });
            }
        }
        if n < Self::minimum_population(d) {
            return Err(RafflesError::InvalidParameter {
                parameter: "population".to_string(),
                value: n as f64,
                reason: format!(
                    "an affine-invariant ensemble in {d} dimensions needs at least {} walkers; \
                     fewer confines the walkers to a subspace, silently",
                    Self::minimum_population(d)
                ),
            });
        }

        let split = n / 2;
        let mut accepted = 0;
        // First half moved against the second, then the second against the
        // (now updated) first — the complementary-halves rule above.
        for (start, end, partner_start, partner_end) in [(0, split, split, n), (split, n, 0, split)]
        {
            for k in start..end {
                let partner_count = partner_end - partner_start;
                let offset = (clamp_open_unit(prn(seed)) * partner_count as f64) as usize;
                let j = partner_start + offset.min(partner_count - 1);

                let z = self.draw_stretch(seed);
                let mut proposal = Vec::with_capacity(d);
                for i in 0..d {
                    proposal.push(population[j].theta[i] + z * (population[k].theta[i] - population[j].theta[i]));
                }

                let ln_target_proposal = {
                    let value = ln_target(&proposal);
                    if value.is_nan() {
                        f64::NEG_INFINITY
                    } else {
                        value
                    }
                };

                let u = clamp_open_unit(prn(seed));
                let accept = if ln_target_proposal == f64::NEG_INFINITY {
                    false
                } else if population[k].ln_target == f64::NEG_INFINITY {
                    true
                } else {
                    let ln_q = (d as f64 - 1.0) * z.ln() + ln_target_proposal
                        - population[k].ln_target;
                    ln_q >= 0.0 || u.ln() < ln_q
                };

                if accept {
                    population[k].theta = proposal;
                    population[k].ln_target = ln_target_proposal;
                    accepted += 1;
                }
            }
        }
        Ok(accepted)
    }

    /// Draws the stretch factor `z` from `g(z) ∝ 1/sqrt(z)` on `[1/a, a]`.
    ///
    /// Inverse transform: with `u ~ U(0, 1)`,
    /// `z = ((a - 1) u + 1)^2 / a` has exactly that density, which is the
    /// standard closed form and needs no rejection step.
    fn draw_stretch(&self, seed: &mut u64) -> f64 {
        let u = clamp_open_unit(prn(seed));
        ((self.a - 1.0) * u + 1.0).powi(2) / self.a
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::samplers::stream_seed;

    /// Log-density of an uncorrelated 2-D standard normal, up to a constant.
    fn ln_standard_normal_2d(theta: &[f64]) -> f64 {
        -0.5 * (theta[0] * theta[0] + theta[1] * theta[1])
    }

    /// Log-density of a strongly correlated, badly scaled 2-D Gaussian.
    ///
    /// Covariance `[[1e6, 0.9999e6 * 1e-3], [..., 1e-6]]` in effect: the first
    /// coordinate has standard deviation 1e3, the second 1e-3, and they are
    /// 0.99 correlated. This is the shape that defeats an untuned random walk
    /// and that an affine-invariant sampler is supposed to be indifferent to.
    fn ln_anisotropic_2d(theta: &[f64]) -> f64 {
        let (sx, sy, rho) = (1.0e3, 1.0e-3, 0.99);
        let (x, y) = (theta[0] / sx, theta[1] / sy);
        let quad = (x * x - 2.0 * rho * x * y + y * y) / (1.0 - rho * rho);
        -0.5 * quad
    }

    fn mean_and_sd(values: &[f64]) -> (f64, f64) {
        let n = values.len() as f64;
        let mean = values.iter().sum::<f64>() / n;
        let var = values.iter().map(|v| (v - mean).powi(2)).sum::<f64>() / (n - 1.0);
        (mean, var.sqrt())
    }

    /// **Methodology.** A Metropolis–Hastings chain with a well-scaled
    /// proposal must reproduce its target. 60 000 steps against a 2-D standard
    /// normal from a proposal with standard deviation 1 in each coordinate;
    /// the first 10 000 discarded as burn-in. Sample mean and standard
    /// deviation of each coordinate against the analytic (0, 1). Pass
    /// criterion: mean within 0.05, standard deviation within 5 %.
    ///
    /// **Result** (2026-09-16, seed 20260916, `--release`): coordinate 0 mean
    /// -0.02001, sd 0.98865; coordinate 1 mean -0.00744, sd 1.00218.
    /// Acceptance rate 0.5518. Inside tolerance. The test prints these under
    /// `cargo test -- --nocapture`, so the recorded values can be re-checked
    /// rather than taken on trust.
    #[test]
    fn metropolis_hastings_reproduces_a_standard_normal() {
        let kernel =
            MetropolisHastings::from_cholesky(vec![vec![1.0, 0.0], vec![0.0, 1.0]]).unwrap();
        let mut seed = stream_seed(20_260_916, 0);
        let mut state = ChainState::new(vec![0.0, 0.0], &ln_standard_normal_2d);

        let burn_in = 10_000;
        let kept = 50_000;
        let mut x = Vec::with_capacity(kept);
        let mut y = Vec::with_capacity(kept);
        let mut accepted = 0;
        for step in 0..(burn_in + kept) {
            if kernel.step(&mut state, &ln_standard_normal_2d, &mut seed) {
                accepted += 1;
            }
            if step >= burn_in {
                x.push(state.theta[0]);
                y.push(state.theta[1]);
            }
        }

        let acceptance = accepted as f64 / (burn_in + kept) as f64;


        let (mx, sx) = mean_and_sd(&x);


        let (my, sy) = mean_and_sd(&y);


        println!("MH standard normal: mean_x {mx:.5} sd_x {sx:.5} mean_y {my:.5} sd_y {sy:.5} acceptance {acceptance:.4}");
        assert!(
            acceptance > 0.1 && acceptance < 0.9,
            "acceptance rate {acceptance} is implausible for this proposal"
        );
        for values in [&x, &y] {
            let (mean, sd) = mean_and_sd(values);
            assert!(mean.abs() < 0.05, "mean {mean}");
            assert!((sd - 1.0).abs() < 0.05, "sd {sd}");
        }
    }

    /// **Methodology.** The ensemble move must reproduce the same 2-D standard
    /// normal, with no proposal covariance supplied at all. 20 walkers,
    /// 3 000 sweeps, first 500 discarded; every kept walker position pooled.
    /// Pass criterion as above.
    ///
    /// **Result** (2026-09-16, seed 11, `--release`): coordinate 0 mean
    /// -0.01602, sd 0.97902; coordinate 1 mean -0.00929, sd 0.99996.
    /// Acceptance rate 0.7177.
    #[test]
    fn ensemble_move_reproduces_a_standard_normal() {
        let mv = EnsembleMove::new(EnsembleMove::DEFAULT_A).unwrap();
        let mut seed = stream_seed(11, 0);
        let mut population: Vec<ChainState> = (0..20)
            .map(|_| {
                let theta = vec![standard_normal(&mut seed), standard_normal(&mut seed)];
                ChainState::new(theta, &ln_standard_normal_2d)
            })
            .collect();

        let burn_in = 500;
        let sweeps = 3_000;
        let mut x = Vec::new();
        let mut y = Vec::new();
        let mut accepted = 0;
        for sweep in 0..sweeps {
            accepted += mv
                .sweep(&mut population, &ln_standard_normal_2d, &mut seed)
                .unwrap();
            if sweep >= burn_in {
                for walker in &population {
                    x.push(walker.theta[0]);
                    y.push(walker.theta[1]);
                }
            }
        }

        let acceptance = accepted as f64 / (sweeps * population.len()) as f64;


        let (mx, sx) = mean_and_sd(&x);


        let (my, sy) = mean_and_sd(&y);


        println!("ensemble standard normal: mean_x {mx:.5} sd_x {sx:.5} mean_y {my:.5} sd_y {sy:.5} acceptance {acceptance:.4}");
        assert!(
            acceptance > 0.1 && acceptance < 0.9,
            "acceptance rate {acceptance}"
        );
        for values in [&x, &y] {
            let (mean, sd) = mean_and_sd(values);
            assert!(mean.abs() < 0.05, "mean {mean}");
            assert!((sd - 1.0).abs() < 0.05, "sd {sd}");
        }
    }

    /// **Methodology — the affine-invariance property itself.** This is the
    /// claim the kernel is chosen for, so it is tested directly rather than
    /// assumed. The same ensemble sampler is run on a 2-D Gaussian whose
    /// coordinates differ in scale by a factor of 1e6 and are 0.99 correlated,
    /// with no proposal information supplied. It must recover both marginal
    /// standard deviations (1e3 and 1e-3) and the correlation, to within 10 %
    /// — the same tolerance the isotropic case meets, since affine invariance
    /// says the difficulty should be identical.
    ///
    /// For contrast the test also runs a random-walk Metropolis–Hastings chain
    /// with an isotropic unit proposal — the natural "untuned" choice — over
    /// the same number of target evaluations, and asserts that it does *not*
    /// recover the first coordinate's scale. That asymmetry is the point of
    /// the whole kernel; if MH ever passed, this test would be measuring
    /// nothing.
    ///
    /// **Result** (2026-09-16, seed 4242, `--release`): ensemble sd_x
    /// 1.001731e3 (target 1.0e3), sd_y 1.000389e-3 (target 1.0e-3),
    /// correlation 0.98981 (target 0.99) — all well inside 10 %. Untuned MH
    /// over a comparable budget: sd_x 2.084, i.e. 0.2 % of the true 1e3,
    /// having explored essentially none of the first coordinate. Affine
    /// invariance confirmed, and the contrast case is emphatic rather than
    /// marginal.
    #[test]
    fn ensemble_move_is_affine_invariant_where_metropolis_hastings_is_not() {
        let mv = EnsembleMove::new(EnsembleMove::DEFAULT_A).unwrap();
        let mut seed = stream_seed(4242, 0);
        let mut population: Vec<ChainState> = (0..40)
            .map(|_| {
                let theta = vec![
                    1.0e3 * standard_normal(&mut seed),
                    1.0e-3 * standard_normal(&mut seed),
                ];
                ChainState::new(theta, &ln_anisotropic_2d)
            })
            .collect();

        let mut x = Vec::new();
        let mut y = Vec::new();
        for sweep in 0..4_000 {
            mv.sweep(&mut population, &ln_anisotropic_2d, &mut seed)
                .unwrap();
            if sweep >= 500 {
                for walker in &population {
                    x.push(walker.theta[0]);
                    y.push(walker.theta[1]);
                }
            }
        }

        let (mean_x, sd_x) = mean_and_sd(&x);
        let (mean_y, sd_y) = mean_and_sd(&y);
        assert!(
            (sd_x - 1.0e3).abs() < 0.1e3,
            "ensemble sd_x {sd_x} should be near 1e3"
        );
        assert!(
            (sd_y - 1.0e-3).abs() < 0.1e-3,
            "ensemble sd_y {sd_y} should be near 1e-3"
        );
        let covariance: f64 = x
            .iter()
            .zip(y.iter())
            .map(|(a, b)| (a - mean_x) * (b - mean_y))
            .sum::<f64>()
            / (x.len() as f64 - 1.0);
        let correlation = covariance / (sd_x * sd_y);

        println!("ensemble anisotropic: sd_x {sd_x:.6e} sd_y {sd_y:.6e} correlation {correlation:.5}");
        assert!(
            (correlation - 0.99).abs() < 0.05,
            "ensemble correlation {correlation}"
        );

        // The contrast case: an untuned isotropic random walk over a
        // comparable evaluation budget cannot see the 1e3 direction at all.
        let kernel =
            MetropolisHastings::from_cholesky(vec![vec![1.0, 0.0], vec![0.0, 1.0]]).unwrap();
        let mut mh_seed = stream_seed(4242, 1);
        let mut state = ChainState::new(vec![0.0, 0.0], &ln_anisotropic_2d);
        let mut mh_x = Vec::new();
        for step in 0..140_000 {
            kernel.step(&mut state, &ln_anisotropic_2d, &mut mh_seed);
            if step >= 20_000 {
                mh_x.push(state.theta[0]);
            }
        }
        let (_, mh_sd_x) = mean_and_sd(&mh_x);

        println!("untuned MH on the same target: sd_x {mh_sd_x:.5}");
        assert!(
            mh_sd_x < 0.5e3,
            "untuned MH unexpectedly recovered sd_x {mh_sd_x}; this test's contrast is gone"
        );
    }

    /// **Methodology.** The stretch density `g(z) ∝ 1/sqrt(z)` on `[1/a, a]`
    /// has mean `(a + 1 + 1/a) / 3`, which for `a = 2` is 1.1666…. 200 000
    /// draws; sample mean against that, and every draw checked to lie inside
    /// `[1/a, a]`.
    ///
    /// **Result** (2026-09-16, seed 99, `--release`): mean 1.166128 against
    /// 1.166667, all draws in range.
    #[test]
    fn stretch_draws_match_their_density() {
        let mv = EnsembleMove::new(2.0).unwrap();
        let mut seed = stream_seed(99, 0);
        let n = 200_000;
        let mut sum = 0.0;
        for _ in 0..n {
            let z = mv.draw_stretch(&mut seed);
            assert!(z >= 0.5 - 1e-12 && z <= 2.0 + 1e-12, "z out of range: {z}");
            sum += z;
        }
        let mean = sum / n as f64;
        let expected = (2.0 + 1.0 + 0.5) / 3.0;

        println!("stretch density: mean {mean:.6} vs {expected:.6}");
        assert!((mean - expected).abs() < 0.005, "mean {mean} vs {expected}");
    }

    /// **Methodology.** A population too small for the dimension must be
    /// rejected, because the failure it causes is silent. `2 * 2 + 2 = 6`
    /// walkers are required in two dimensions; 5 are offered.
    ///
    /// **Result.** `InvalidParameter`, naming the required size (2026-09-16).
    #[test]
    fn undersized_population_is_rejected() {
        let mv = EnsembleMove::new(2.0).unwrap();
        let mut seed = stream_seed(1, 0);
        let mut population: Vec<ChainState> = (0..5)
            .map(|_| ChainState::new(vec![0.1, 0.2], &ln_standard_normal_2d))
            .collect();
        let err = mv
            .sweep(&mut population, &ln_standard_normal_2d, &mut seed)
            .unwrap_err();
        assert!(matches!(err, RafflesError::InvalidParameter { .. }));
    }

    /// **Methodology.** A chain started at an impossible point must escape it,
    /// otherwise a prior-support violation in the initial population would
    /// freeze that walker forever. One MH step from a `-inf` state against a
    /// finite target.
    ///
    /// **Result.** The step is accepted and the state becomes finite
    /// (2026-09-16).
    #[test]
    fn a_chain_can_escape_an_impossible_start() {
        let kernel =
            MetropolisHastings::from_cholesky(vec![vec![1.0, 0.0], vec![0.0, 1.0]]).unwrap();
        let mut seed = stream_seed(5, 0);
        let mut state = ChainState {
            theta: vec![0.0, 0.0],
            ln_target: f64::NEG_INFINITY,
        };
        assert!(kernel.step(&mut state, &ln_standard_normal_2d, &mut seed));
        assert!(state.ln_target.is_finite());
    }

    /// **Methodology.** Determinism is what every committed result in this
    /// module depends on. The same seed and configuration must give the same
    /// chain. Two independent 500-step runs compared element-wise.
    ///
    /// **Result.** Byte-identical (2026-09-16).
    #[test]
    fn runs_are_reproducible_from_the_seed() {
        let kernel =
            MetropolisHastings::from_cholesky(vec![vec![0.7, 0.0], vec![0.2, 0.5]]).unwrap();
        let run = || {
            let mut seed = stream_seed(31_337, 0);
            let mut state = ChainState::new(vec![0.3, -0.2], &ln_standard_normal_2d);
            let mut path = Vec::new();
            for _ in 0..500 {
                kernel.step(&mut state, &ln_standard_normal_2d, &mut seed);
                path.push(state.theta.clone());
            }
            path
        };
        assert_eq!(run(), run());
    }
}
