//! Bayesian model selection: comparing competing models by their evidence.
//!
//! # What this is for
//!
//! Parameter estimation asks "given this model, what are its parameters?".
//! Model selection asks the prior question: **which model?** A stiffer spring
//! or a nonlinear one; a Weibull failure law or a lognormal; a two-parameter
//! creep model or a four-parameter one. The Bayesian answer is the ratio of the
//! models' evidences — their marginal likelihoods — which the transitional
//! samplers in [`crate::bayesian`] already produce as a by-product of sampling.
//!
//! # The Occam factor is not an add-on
//!
//! The evidence integrates the likelihood over the *whole prior*, so a model
//! with more parameters, or vaguer priors on them, pays for the parameter space
//! it does not use. That penalty is automatic: there is no AIC-style correction
//! term to remember, and none to get wrong. A four-parameter model must fit
//! enough better to repay the volume it spends.
//!
//! [`crate::bayesian::transitional`] has a test measuring exactly this: widening
//! a prior tenfold lowered the log-evidence by 2.23 nats against a closed-form
//! 2.18.
//!
//! # What this module will not do for you
//!
//! - **It cannot rescue a bad evidence estimate.** The Bayes factor inherits
//!   every error in the two log-evidences, and a sampler that has not converged
//!   produces a confident, wrong one. Compare the stage reports of the two runs
//!   before trusting their ratio.
//! - **It says nothing about whether either model is any good.** A Bayes factor
//!   of 1000 means one model is far better than the other; both may still be
//!   hopeless. That is a posterior-predictive question, not a model-selection
//!   one.
//! - **The interpretation scale is a convention, not a result.** See
//!   [`EvidenceStrength`].
//!
//! # References
//!
//! - H. Jeffreys (1961). *Theory of Probability*, 3rd edition. Oxford
//!   University Press — the original interpretation scale.
//! - R. E. Kass and A. E. Raftery (1995). Bayes factors. *Journal of the
//!   American Statistical Association, 90*(430), 773–795. doi:
//!   [10.1080/01621459.1995.10476572](https://doi.org/10.1080/01621459.1995.10476572)
//!   — the scale used here, and the standard modern reference.
//! - J. Ching and Y.-C. Chen (2007). Transitional Markov Chain Monte Carlo
//!   method for Bayesian model updating, model class selection, and model
//!   averaging. *Journal of Engineering Mechanics, 133*(7), 816–832. doi:
//!   [10.1061/(ASCE)0733-9399(2007)133:7(816)](https://doi.org/10.1061/(ASCE)0733-9399(2007)133:7(816))
//!   — model class selection from TMCMC evidence, which is what this module
//!   consumes.

use crate::{RafflesError, Result};

/// How strongly a Bayes factor favours one model over another, on the
/// Kass–Raftery scale.
///
/// **This is a convention.** The thresholds are conventional reading aids, not
/// results: nothing changes about the evidence at `2 * ln B = 6`. They are
/// offered because a bare number invites over-reading in both directions, and
/// because a report that says "strong" is easier to argue with than one that
/// says "3.7".
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum EvidenceStrength {
    /// `2 ln B` below 2: the data barely distinguish the models.
    NotWorthMoreThanABareMention,
    /// `2 ln B` in `[2, 6)`.
    Positive,
    /// `2 ln B` in `[6, 10)`.
    Strong,
    /// `2 ln B` at or above 10.
    VeryStrong,
}

impl EvidenceStrength {
    /// Classifies a natural-log Bayes factor on the Kass–Raftery scale, which
    /// is stated in terms of `2 * ln B`.
    pub fn from_ln_bayes_factor(ln_bayes_factor: f64) -> Self {
        let scale = 2.0 * ln_bayes_factor.abs();
        if scale < 2.0 {
            Self::NotWorthMoreThanABareMention
        } else if scale < 6.0 {
            Self::Positive
        } else if scale < 10.0 {
            Self::Strong
        } else {
            Self::VeryStrong
        }
    }

    /// The conventional phrase, for a report line.
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::NotWorthMoreThanABareMention => "not worth more than a bare mention",
            Self::Positive => "positive",
            Self::Strong => "strong",
            Self::VeryStrong => "very strong",
        }
    }
}

/// One candidate model in a comparison: a name and its log-evidence.
#[derive(Debug, Clone, PartialEq)]
pub struct ModelEvidence {
    /// How the model is referred to in the report.
    pub name: String,
    /// Natural log of the model's evidence `p(D | M)`, as returned by
    /// [`crate::bayesian::TransitionalResult::ln_evidence`].
    ///
    /// The log, always: a realistic data set drives the evidence itself far
    /// below `f64::MIN_POSITIVE`, so a comparison written in terms of the raw
    /// evidence silently becomes `0 / 0`.
    pub ln_evidence: f64,
}

impl ModelEvidence {
    /// Builds a candidate.
    ///
    /// # Errors
    ///
    /// [`RafflesError::InvalidParameter`] if `ln_evidence` is not finite. A
    /// `NaN` here is the signature of a sampler that failed rather than of a
    /// model that lost, and quietly ranking it last would hide that.
    pub fn new(name: impl Into<String>, ln_evidence: f64) -> Result<Self> {
        if !ln_evidence.is_finite() {
            return Err(RafflesError::InvalidParameter {
                parameter: "ln_evidence".to_string(),
                value: ln_evidence,
                reason: "a model's log-evidence must be finite; a NaN or infinity means the \
                         sampler failed rather than that the model lost"
                    .to_string(),
            });
        }
        Ok(Self {
            name: name.into(),
            ln_evidence,
        })
    }
}

/// The natural-log Bayes factor of model `a` against model `b`.
///
/// `ln B = ln p(D | M_a) - ln p(D | M_b)`. Positive favours `a`.
///
/// Returned as a log for the same reason the inputs are logs: `exp` of it
/// overflows or underflows for any comparison decisive enough to be
/// interesting.
pub fn ln_bayes_factor(a: &ModelEvidence, b: &ModelEvidence) -> f64 {
    a.ln_evidence - b.ln_evidence
}

/// Posterior probability of each model, given equal prior probabilities.
///
/// A softmax over the log-evidences, computed with the usual maximum shift so
/// that a set of log-evidences around -15 000 — entirely ordinary for a few
/// hundred observations — does not underflow to a vector of zeros.
///
/// The result is aligned with the input and sums to 1.
///
/// # The equal-prior assumption is doing work
///
/// Equal prior model probabilities is a choice, and in a safety case it is
/// often the wrong one: a model that contradicts established physics should not
/// start level with one that does not. Where priors differ, add
/// `ln p(M_i)` to each log-evidence before calling this — the arithmetic is the
/// same, and doing it at the call site keeps the assumption visible.
///
/// # Errors
///
/// [`RafflesError::InvalidParameter`] if `models` is empty.
pub fn posterior_model_probabilities(models: &[ModelEvidence]) -> Result<Vec<f64>> {
    if models.is_empty() {
        return Err(RafflesError::InvalidParameter {
            parameter: "models".to_string(),
            value: 0.0,
            reason: "a comparison needs at least one model".to_string(),
        });
    }
    let max = models
        .iter()
        .map(|m| m.ln_evidence)
        .fold(f64::NEG_INFINITY, f64::max);
    let weights: Vec<f64> = models.iter().map(|m| (m.ln_evidence - max).exp()).collect();
    let total: f64 = weights.iter().sum();
    Ok(weights.into_iter().map(|w| w / total).collect())
}

/// A ranked comparison of several models.
#[derive(Debug, Clone, PartialEq)]
pub struct ModelComparison {
    /// The candidates, ordered best first.
    pub ranked: Vec<ModelEvidence>,
    /// Posterior probability of each entry in [`ranked`](Self::ranked), under
    /// equal prior model probabilities.
    pub probabilities: Vec<f64>,
    /// Natural-log Bayes factor of the best model against the runner-up, or
    /// `None` when only one model was supplied.
    pub ln_bayes_factor_over_runner_up: Option<f64>,
    /// How strong that margin is, on the Kass–Raftery scale.
    pub strength: Option<EvidenceStrength>,
}

impl ModelComparison {
    /// The winning model.
    pub fn best(&self) -> &ModelEvidence {
        &self.ranked[0]
    }

    /// A one-line summary for a report or a log.
    pub fn summary(&self) -> String {
        match (&self.ln_bayes_factor_over_runner_up, &self.strength) {
            (Some(ln_b), Some(strength)) => format!(
                "{} wins with posterior probability {:.4}; ln B = {:.3} over {} ({} evidence)",
                self.best().name,
                self.probabilities[0],
                ln_b,
                self.ranked[1].name,
                strength.as_str()
            ),
            _ => format!(
                "{} is the only candidate; a comparison of one model is not a comparison",
                self.best().name
            ),
        }
    }
}

/// Ranks models by evidence and reports the margin.
///
/// # Errors
///
/// [`RafflesError::InvalidParameter`] if `models` is empty.
pub fn compare_models(models: &[ModelEvidence]) -> Result<ModelComparison> {
    if models.is_empty() {
        return Err(RafflesError::InvalidParameter {
            parameter: "models".to_string(),
            value: 0.0,
            reason: "a comparison needs at least one model".to_string(),
        });
    }
    let mut ranked = models.to_vec();
    ranked.sort_by(|a, b| {
        b.ln_evidence
            .partial_cmp(&a.ln_evidence)
            .unwrap_or(core::cmp::Ordering::Equal)
    });
    let probabilities = posterior_model_probabilities(&ranked)?;
    let (ln_b, strength) = if ranked.len() > 1 {
        let ln_b = ranked[0].ln_evidence - ranked[1].ln_evidence;
        (
            Some(ln_b),
            Some(EvidenceStrength::from_ln_bayes_factor(ln_b)),
        )
    } else {
        (None, None)
    };
    Ok(ModelComparison {
        ranked,
        probabilities,
        ln_bayes_factor_over_runner_up: ln_b,
        strength,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::bayesian::{temcmc, IndependentPrior, TransitionalConfig};
    use crate::distributions::{Distribution, Normal};

    /// **Methodology.** The log Bayes factor is a difference of log-evidences,
    /// and posterior model probabilities are their softmax. Checked against
    /// values computed by hand: evidences of -10 and -12 give `ln B = 2` and
    /// probabilities `(e^2/(1+e^2), 1/(1+e^2)) = (0.880797, 0.119203)`.
    ///
    /// **Result** (2026-09-16): exact to 1e-12.
    #[test]
    fn bayes_factors_and_model_probabilities_are_what_they_should_be() {
        let a = ModelEvidence::new("a", -10.0).unwrap();
        let b = ModelEvidence::new("b", -12.0).unwrap();
        assert!((ln_bayes_factor(&a, &b) - 2.0).abs() < 1e-12);

        let probabilities = posterior_model_probabilities(&[a, b]).unwrap();
        assert!((probabilities[0] - 0.880_797_077_977_882).abs() < 1e-12);
        assert!((probabilities[1] - 0.119_202_922_022_118).abs() < 1e-12);
        assert!((probabilities.iter().sum::<f64>() - 1.0).abs() < 1e-15);
    }

    /// **Methodology — the underflow that makes the log form necessary.** Two
    /// models with log-evidences of -15 000 and -15 002 are entirely ordinary
    /// for a few hundred observations, and `exp(-15000)` is zero in `f64`. The
    /// probabilities must still come out as 0.8808 and 0.1192 rather than as
    /// `NaN` from a `0 / 0`.
    ///
    /// **Result** (2026-09-16): 0.880797 and 0.119203, identical to the
    /// well-scaled case above.
    #[test]
    fn enormous_negative_log_evidences_do_not_underflow() {
        let models = [
            ModelEvidence::new("a", -15_000.0).unwrap(),
            ModelEvidence::new("b", -15_002.0).unwrap(),
        ];
        let probabilities = posterior_model_probabilities(&models).unwrap();
        assert!((probabilities[0] - 0.880_797_077_977_882).abs() < 1e-12);
        assert!(probabilities.iter().all(|p| p.is_finite()));
    }

    /// **Methodology.** The Kass–Raftery thresholds are stated in `2 ln B`, so
    /// the boundaries fall at `ln B` = 1, 3 and 5. Checked either side of each,
    /// and for a negative `ln B` (which favours the other model but is equally
    /// strong evidence).
    ///
    /// **Result** (2026-09-16): every boundary lands in the right band.
    #[test]
    fn the_interpretation_scale_has_the_published_boundaries() {
        use EvidenceStrength::*;
        assert_eq!(
            EvidenceStrength::from_ln_bayes_factor(0.9),
            NotWorthMoreThanABareMention
        );
        assert_eq!(EvidenceStrength::from_ln_bayes_factor(1.1), Positive);
        assert_eq!(EvidenceStrength::from_ln_bayes_factor(2.9), Positive);
        assert_eq!(EvidenceStrength::from_ln_bayes_factor(3.1), Strong);
        assert_eq!(EvidenceStrength::from_ln_bayes_factor(4.9), Strong);
        assert_eq!(EvidenceStrength::from_ln_bayes_factor(5.1), VeryStrong);
        // Symmetric: evidence against is evidence.
        assert_eq!(EvidenceStrength::from_ln_bayes_factor(-5.1), VeryStrong);
    }

    /// **Methodology — end to end, against a known answer.** Two candidate
    /// models for the same twelve observations of a `N(2.5, 1)` process:
    /// model A has the correct noise scale `sigma = 1`, model B assumes
    /// `sigma = 3`, which is badly over-dispersed. Each is sampled with TEMCMC,
    /// and the comparison must pick model A.
    ///
    /// This is a real end-to-end check rather than an arithmetic one: the
    /// evidences come from actual sampler runs, and the answer is known
    /// independently because the data were generated under A's assumption.
    ///
    /// **Result** (2026-09-16, `--release`, seed 20260916): the correct model
    /// wins with ln Z -15.7539 against -26.3261, a log Bayes factor of 10.572
    /// — "very strong" on the Kass-Raftery scale — and a posterior model
    /// probability of 1.0000 to four figures.
    #[test]
    fn model_selection_picks_the_model_the_data_came_from() {
        let y: Vec<f64> = vec![2.1, 3.4, 1.9, 2.8, 2.2, 3.1, 2.6, 2.0, 3.0, 2.4, 2.9, 1.6];
        let prior =
            IndependentPrior::new(vec![Distribution::Normal(Normal::new(0.0, 5.0).unwrap())])
                .unwrap();

        let mut candidates = Vec::new();
        for (name, sigma) in [
            ("sigma = 1 (correct)", 1.0),
            ("sigma = 3 (over-dispersed)", 3.0),
        ] {
            let data = y.clone();
            let ln_likelihood = move |theta: &[f64]| {
                let mu = theta[0];
                let n = data.len() as f64;
                let ss: f64 = data.iter().map(|v| (v - mu).powi(2)).sum();
                -0.5 * n * (2.0 * core::f64::consts::PI * sigma * sigma).ln()
                    - ss / (2.0 * sigma * sigma)
            };
            let config = TransitionalConfig::temcmc_defaults(2_000, 20_260_916).unwrap();
            let result = temcmc(&prior, ln_likelihood, &config).unwrap();
            candidates.push(ModelEvidence::new(name, result.ln_evidence).unwrap());
        }

        let comparison = compare_models(&candidates).unwrap();
        println!("model selection: {}", comparison.summary());
        for (model, probability) in comparison
            .ranked
            .iter()
            .zip(comparison.probabilities.iter())
        {
            println!(
                "  {}: ln Z {:.4}, P(M|D) {:.4}",
                model.name, model.ln_evidence, probability
            );
        }

        assert_eq!(comparison.best().name, "sigma = 1 (correct)");
        assert!(comparison.probabilities[0] > 0.9);
    }

    /// **Methodology.** A failed sampler must be refused rather than ranked
    /// last: `NaN` and infinite log-evidences are rejected at construction.
    /// An empty comparison is refused too.
    ///
    /// **Result.** All three rejected (2026-09-16).
    #[test]
    fn a_failed_sampler_is_refused_not_ranked_last() {
        assert!(ModelEvidence::new("bad", f64::NAN).is_err());
        assert!(ModelEvidence::new("bad", f64::NEG_INFINITY).is_err());
        assert!(compare_models(&[]).is_err());
    }

    /// **Methodology.** A comparison of one model is not a comparison, and the
    /// summary must say so rather than implying a win.
    ///
    /// **Result** (2026-09-16): no Bayes factor, no strength, and the summary
    /// names the situation.
    #[test]
    fn a_single_candidate_is_not_a_comparison() {
        let comparison = compare_models(&[ModelEvidence::new("only", -3.0).unwrap()]).unwrap();
        assert_eq!(comparison.ln_bayes_factor_over_runner_up, None);
        assert_eq!(comparison.strength, None);
        assert!(comparison.summary().contains("not a comparison"));
        assert!((comparison.probabilities[0] - 1.0).abs() < 1e-15);
    }
}
