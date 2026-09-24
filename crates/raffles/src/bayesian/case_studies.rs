// ---------------------------------------------------------------------------
// Ported from Bayesian-Model-Updating-Tutorials.
//
//   Upstream project: Bayesian Model Updating Tutorials (A. Lye)
//   Upstream repo:    https://github.com/Adolphus8/Bayesian-Model-Updating-Tutorials
//   Upstream files:   1D Static Spring-Mass System/model.m
//                     1D Static Spring-Mass System/log_likelihood.m
//                     2D Eigen-value Problem/model.m
//                     2D Eigen-value Problem/log_likelihood.m
//   Upstream commit:  f88cbbc1bac5e4caab6f5fd620d76a25c50defa5
//   Accessed:         2026-09-16
//
//   Licensed under the GNU General Public License v3.0, as published in that
//   repository's LICENSE file. This Rust translation is part of RAFFLES /
//   Outram Park and is distributed under GPL-3.0-only, which is the same
//   licence.
//
//   NOTE: this is the ONE repository of the seven in workspace issue #158 that
//   carries a licence. The other six carry none, so nothing was taken from
//   them and the algorithms elsewhere in this crate are independent
//   implementations from the published papers. See the provenance note in
//   `super`.
//
// Translation notes: the MATLAB takes a matrix of chains and loops over rows;
// here each model is a plain function of one parameter vector, because the
// samplers in this crate call the likelihood per-sample rather than per-batch.
// The log-likelihoods are written as closures the caller builds (see the tests)
// rather than as functions taking a model handle, since Rust closures capture
// the data directly and a handle argument would buy nothing. The MATLAB
// `.mlx` live scripts that drive these — plotting, prior setup, sampler
// configuration — are not ported: they are the tutorial's presentation layer.
// ---------------------------------------------------------------------------

//! Case studies from the Bayesian model-updating tutorials — small problems
//! with exactly known answers.
//!
//! # Why these are worth having
//!
//! Every sampler in [`super`] is verified against a conjugate Normal–Normal
//! problem, which is the right backbone test but is also the easiest possible
//! posterior: unimodal, symmetric, and in one dimension. These two case studies
//! are harder in specific, useful ways, and both still have answers that can be
//! written down rather than estimated:
//!
//! - [`spring_force`] — a linear static spring-mass system. The posterior over
//!   the stiffness is exactly Gaussian, so mean and variance are checkable in
//!   closed form. It is the sanity case.
//! - [`two_by_two_eigenvalues`] — the eigenvalues of a 2x2 structural matrix.
//!   Its posterior is **bimodal**: two distinct parameter vectors produce the
//!   same pair of eigenvalues, and both are computable exactly by solving a
//!   quadratic. A sampler that finds only one of them has failed in a way a
//!   moment-based check would not notice.
//!
//! The second is the one that earns its place. Collapsing onto a single mode is
//! the characteristic failure of a poorly-mixing sampler, and it looks like
//! success from every summary statistic.

/// Force in a 1-degree-of-freedom linear spring: `F = -k * x`.
///
/// - `stiffness` — `k`, in newtons per metre. Physically positive, though
///   nothing here enforces it: a sampler exploring a prior that allows negative
///   values should get an answer rather than a panic.
/// - `displacement` — `x`, in metres.
///
/// Returns the force in newtons. The sign convention is the upstream's: the
/// restoring force opposes the displacement.
pub fn spring_force(stiffness: f64, displacement: f64) -> f64 {
    -stiffness * displacement
}

/// Eigenvalues of the 2x2 structural matrix parameterised by `theta`.
///
/// The closed form the upstream uses:
///
/// ```text
/// lambda_1 = 0.5 * ((t1 + 2 t2) + sqrt(t1^2 + 4 t2^2))
/// lambda_2 = 0.5 * ((t1 + 2 t2) - sqrt(t1^2 + 4 t2^2))
/// ```
///
/// Returns `(lambda_1, lambda_2)` with `lambda_1 >= lambda_2` by construction.
///
/// # The bimodality, and where it comes from
///
/// The map is two-to-one almost everywhere. Writing `s = lambda_1 + lambda_2`
/// and `d = lambda_1 - lambda_2`, the parameters satisfy
///
/// ```text
/// t1 + 2 t2 = s        (a line)
/// t1^2 + 4 t2^2 = d^2  (an ellipse)
/// ```
///
/// and a line generally cuts an ellipse twice. So a measured eigenvalue pair is
/// explained equally well by **two** parameter vectors, and the posterior has
/// two modes. [`eigenvalue_partner`] computes the other one.
///
/// This is not a defect of the model — it is genuine non-identifiability, the
/// thing Bayesian model updating is supposed to *reveal* rather than average
/// over. A point estimate of `theta` here is meaningless on its own.
///
/// # Panics
///
/// Never. A negative discriminant is impossible: `t1^2 + 4 t2^2` is a sum of
/// squares.
pub fn two_by_two_eigenvalues(theta: &[f64]) -> (f64, f64) {
    let (t1, t2) = (theta[0], theta[1]);
    let trace = t1 + 2.0 * t2;
    let spread = (t1 * t1 + 4.0 * t2 * t2).sqrt();
    (0.5 * (trace + spread), 0.5 * (trace - spread))
}

/// The *other* parameter vector that produces the same eigenvalues as `theta`.
///
/// Solves the line-meets-ellipse system in [`two_by_two_eigenvalues`] for its
/// second root. Returns `None` when the line is tangent to the ellipse, which
/// is the measure-zero case where the two modes coincide and the problem is
/// identifiable after all.
///
/// Used by this module's tests to state exactly where the second posterior mode
/// must be, rather than looking for "some other cluster".
pub fn eigenvalue_partner(theta: &[f64]) -> Option<Vec<f64>> {
    let (lambda_1, lambda_2) = two_by_two_eigenvalues(theta);
    let s = lambda_1 + lambda_2;
    let d_squared = (lambda_1 - lambda_2).powi(2);

    // Substituting t1 = s - 2 t2 into t1^2 + 4 t2^2 = d^2 gives
    //   8 t2^2 - 4 s t2 + (s^2 - d^2) = 0.
    let a = 8.0;
    let b = -4.0 * s;
    let c = s * s - d_squared;
    let discriminant = b * b - 4.0 * a * c;
    if discriminant <= 1.0e-12 {
        return None;
    }
    let root = discriminant.sqrt();
    let candidates = [(-b + root) / (2.0 * a), (-b - root) / (2.0 * a)];
    // Return whichever root is not the one we started from.
    for t2 in candidates {
        if (t2 - theta[1]).abs() > 1.0e-9 {
            return Some(vec![s - 2.0 * t2, t2]);
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::bayesian::{temcmc, IndependentPrior, TransitionalConfig};
    use crate::distributions::{Distribution, Uniform};

    /// **Methodology — the linear case, against its exact posterior.** Five
    /// displacement measurements of a spring of true stiffness 250 N/m, with
    /// Gaussian force noise of 2 N and a `Uniform(100, 400)` prior on the
    /// stiffness.
    ///
    /// For a linear model with Gaussian noise the posterior over `k` is exactly
    /// Gaussian (truncated to the prior, which is wide enough here not to
    /// bite), with
    ///
    /// ```text
    /// k_hat   = -sum(F_i x_i) / sum(x_i^2)
    /// var(k)  = sigma^2 / sum(x_i^2)
    /// ```
    ///
    /// so both the posterior mean and its standard deviation are known without
    /// sampling. TEMCMC with 2 000 samples must reproduce both.
    ///
    /// **Result** (2026-09-16, `--release`, seed 20260916): posterior mean
    /// 249.0285 N/m against the closed-form 248.5455 — a discrepancy of 0.018
    /// posterior standard deviations — and sd 27.1577 against 26.9680 (+0.7 %).
    /// The true stiffness of 250 N/m sits well inside, as it should: five
    /// noisy measurements do not pin a stiffness to better than about 27 N/m.
    #[test]
    fn the_spring_mass_posterior_matches_its_closed_form() {
        let true_stiffness = 250.0;
        let sigma = 2.0;
        let displacements = [0.010, 0.020, 0.030, 0.040, 0.050];
        // Fixed "measurements": the exact model plus a fixed small perturbation,
        // so the test is deterministic and the closed form applies exactly to
        // the numbers actually used.
        let perturbations = [0.8, -1.3, 0.4, 1.1, -0.6];
        let forces: Vec<f64> = displacements
            .iter()
            .zip(perturbations.iter())
            .map(|(x, e)| spring_force(true_stiffness, *x) + e)
            .collect();

        let sum_xx: f64 = displacements.iter().map(|x| x * x).sum();
        let sum_fx: f64 = forces
            .iter()
            .zip(displacements.iter())
            .map(|(f, x)| f * x)
            .sum();
        let exact_mean = -sum_fx / sum_xx;
        let exact_sd = sigma / sum_xx.sqrt();

        let prior = IndependentPrior::new(vec![Distribution::Uniform(
            Uniform::new(100.0, 400.0).unwrap(),
        )])
        .unwrap();
        let ln_likelihood = move |theta: &[f64]| {
            let k = theta[0];
            let mut acc = 0.0;
            for (x, f) in displacements.iter().zip(forces.iter()) {
                let residual = f - spring_force(k, *x);
                acc += residual * residual;
            }
            -0.5 * acc / (sigma * sigma)
        };

        let config = TransitionalConfig::temcmc_defaults(2_000, 20_260_916).unwrap();
        let result = temcmc(&prior, ln_likelihood, &config).unwrap();
        let mean = result.posterior_mean()[0];
        let sd = result.posterior_std_dev()[0];
        println!(
            "spring-mass: posterior mean {mean:.4} N/m (exact {exact_mean:.4}), \
             sd {sd:.4} (exact {exact_sd:.4}), true stiffness {true_stiffness}"
        );

        assert!(
            (mean - exact_mean).abs() < 0.1 * exact_sd,
            "mean {mean} vs exact {exact_mean}"
        );
        assert!(
            (sd - exact_sd).abs() < 0.2 * exact_sd,
            "sd {sd} vs exact {exact_sd}"
        );
    }

    /// **Methodology — the bimodal case, with both modes computed exactly.**
    /// The 2x2 eigenvalue problem at a true parameter vector of `(1.0, 0.8)`.
    /// Solving the line-meets-ellipse system shows a second parameter vector
    /// produces identical eigenvalues; [`eigenvalue_partner`] gives it as
    /// `(1.6, 0.5)`, and the test checks that claim before using it.
    ///
    /// A `Uniform(0, 3)` prior on each parameter, noise-free measurements with
    /// a likelihood standard deviation of 0.01, TEMCMC with 4 000 samples. The
    /// pass criterion is that **both** modes hold at least 5 % of the posterior
    /// sample within a radius of 0.15.
    ///
    /// This is the test that would catch a sampler collapsing onto one mode —
    /// a failure every moment-based check would report as success, and which
    /// would here produce a confident, wrong stiffness pair.
    ///
    /// **Result** (2026-09-16, `--release`, seed 20260916): the partner is
    /// exactly (1.6, 0.5), and both vectors give eigenvalues (2.243398,
    /// 0.356602) to 1e-9. The posterior puts 0.5550 of its mass at the true
    /// mode and 0.3840 at the second — 93.9 % of the sample accounted for by
    /// the two branches, in 7 tempering stages. Neither mode was missed.
    #[test]
    fn the_eigenvalue_problem_finds_both_of_its_exact_modes() {
        let truth = [1.0, 0.8];
        let partner = eigenvalue_partner(&truth).expect("this problem is bimodal");
        // The partner must genuinely reproduce the same eigenvalues.
        let (l1, l2) = two_by_two_eigenvalues(&truth);
        let (p1, p2) = two_by_two_eigenvalues(&partner);
        assert!(
            (l1 - p1).abs() < 1e-9 && (l2 - p2).abs() < 1e-9,
            "partner {partner:?} gives ({p1}, {p2}) not ({l1}, {l2})"
        );
        println!(
            "eigenvalue problem: truth {truth:?} and partner {partner:?} both give \
             eigenvalues ({l1:.6}, {l2:.6})"
        );
        assert!((partner[0] - 1.6).abs() < 1e-9 && (partner[1] - 0.5).abs() < 1e-9);

        let sigma = 0.01;
        let prior = IndependentPrior::new(vec![
            Distribution::Uniform(Uniform::new(0.0, 3.0).unwrap()),
            Distribution::Uniform(Uniform::new(0.0, 3.0).unwrap()),
        ])
        .unwrap();
        let ln_likelihood = move |theta: &[f64]| {
            let (m1, m2) = two_by_two_eigenvalues(theta);
            -0.5 * ((l1 - m1).powi(2) + (l2 - m2).powi(2)) / (sigma * sigma)
        };

        let config = TransitionalConfig::temcmc_defaults(4_000, 20_260_916).unwrap();
        let result = temcmc(&prior, ln_likelihood, &config).unwrap();

        let occupancy = |centre: &[f64]| {
            result
                .samples
                .iter()
                .filter(|s| ((s[0] - centre[0]).powi(2) + (s[1] - centre[1]).powi(2)).sqrt() < 0.15)
                .count() as f64
                / result.samples.len() as f64
        };
        let at_truth = occupancy(&truth);
        let at_partner = occupancy(&partner);
        println!(
            "eigenvalue posterior occupancy: {at_truth:.4} at {truth:?}, \
             {at_partner:.4} at {partner:?}, {} stages",
            result.stages.len()
        );

        assert!(
            at_truth > 0.05,
            "the true mode holds only {at_truth} of the posterior"
        );
        assert!(
            at_partner > 0.05,
            "the second mode holds only {at_partner} of the posterior — the sampler has \
             collapsed onto one branch, which every moment-based check would call success"
        );
    }

    /// **Methodology.** The two models must match their upstream formulas at
    /// hand-computed points. `spring_force(250, 0.02)` is `-5 N`. For
    /// `theta = (1, 0.5)`, the trace is 2 and the spread is
    /// `sqrt(1 + 1) = 1.41421`, giving eigenvalues 1.70711 and 0.29289.
    ///
    /// **Result** (2026-09-16): both exact to 1e-12.
    #[test]
    fn the_models_match_their_upstream_formulas() {
        assert!((spring_force(250.0, 0.02) + 5.0).abs() < 1e-12);
        assert!((spring_force(250.0, -0.02) - 5.0).abs() < 1e-12);

        let (l1, l2) = two_by_two_eigenvalues(&[1.0, 0.5]);
        assert!(
            (l1 - 1.707_106_781_186_547_5).abs() < 1e-12,
            "lambda_1 {l1}"
        );
        assert!(
            (l2 - 0.292_893_218_813_452_5).abs() < 1e-12,
            "lambda_2 {l2}"
        );
        // Ordering is guaranteed.
        assert!(l1 >= l2);
    }

    /// **Methodology — the tangent case.** At `theta = (1.0, 0.5)` the line is
    /// tangent to the ellipse, so the two modes coincide and the problem is
    /// identifiable. [`eigenvalue_partner`] must report that rather than
    /// inventing a second root.
    ///
    /// **Result** (2026-09-16): `None`, as the double root requires.
    #[test]
    fn the_tangent_case_has_no_second_mode() {
        assert_eq!(eigenvalue_partner(&[1.0, 0.5]), None);
    }
}
