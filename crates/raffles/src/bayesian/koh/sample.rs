// SPDX-License-Identifier: AGPL-3.0-only
// Copyright (C) 2026 OUTRAM PARK contributors
//
// NOT a port. SEPIA's own MCMC (per-parameter Metropolis with its
// "Uniform", "BetaRho" and "PropMH" steps and step-size tuning) is not
// translated: raffles' TMCMC (`bayesian::transitional`) samples SEPIA's
// posterior instead, on the coordinates SEPIA's steps act on (θ, ρ, λ), so
// both target the same density. Written for gh:#1180.

//! Sampling SEPIA's Kennedy–O'Hagan posterior with TMCMC.
//!
//! TMCMC tempers a likelihood against a proper prior it can draw from. The
//! target here is SEPIA's log posterior, so the sampler is handed a simple
//! **base** distribution per coordinate — uniform on (0, 1) for θ and for
//! ρ = exp(−β/4), uniform on the bounds for a precision with finite bounds,
//! the prior's own Gamma (shifted to its lower bound) otherwise — and the
//! "likelihood" `ln post − ln base`. At β = 1 the tempered target is
//! exactly SEPIA's posterior; only the tempering path differs.

use petir::mathf::RealMath;
use super::model::{EmulatorFactor, KohForm, KohModel, KohParams, ParamPrior, PriorDist};
use crate::bayesian::transitional::{tmcmc, TemperingCriterion, TransitionKernel, TransitionalConfig};
use crate::bayesian::IndependentPrior;
use crate::distributions::{Distribution, Gamma, Uniform};
use crate::{RafflesError, Result};

/// Which parameters TMCMC samples; the rest stay at the start values.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum KohFree {
    /// Every parameter of the model's form (SEPIA's joint posterior).
    All,
    /// The emulator's (β_U, λ_Uz, λ_Ws, λ_WOs) held at the start values,
    /// θ, β_V, λ_Vz and λ_Os sampled: Kennedy & O'Hagan's plug-in of the
    /// emulator hyperparameters (KOH 2001, §4.5). Its W block is factored
    /// once.
    CalibrationOnly,
}

/// The sampler's settings.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct KohSampleConfig {
    /// Which parameters move.
    pub free: KohFree,
    /// TMCMC population.
    pub population: usize,
    /// Metropolis moves per sample per stage.
    pub chain_length: usize,
    /// Master seed.
    pub seed: i64,
}

/// Posterior samples.
#[derive(Debug, Clone, PartialEq)]
pub struct KohPosterior {
    /// One parameter set per TMCMC particle.
    pub samples: Vec<KohParams>,
    /// SEPIA's log posterior at each sample.
    pub log_post: Vec<f64>,
    /// TMCMC's log evidence of the tempered "likelihood" (relative to the
    /// base, not SEPIA's normalisation).
    pub ln_evidence: f64,
    /// Tempering stages taken.
    pub stages: usize,
}

impl KohPosterior {
    /// The sample with the highest log posterior.
    ///
    /// # Panics
    /// Never for a posterior `sample` returned (it has `population ≥ 1`
    /// samples). Justified: an empty posterior cannot be constructed here.
    #[must_use]
    pub fn best(&self) -> &KohParams {
        let i = self.log_post.iter().enumerate().fold(0, |b, (i, v)| if *v > self.log_post[b] { i } else { b });
        &self.samples[i]
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Group {
    Theta,
    BetaU,
    LamUz,
    LamWs,
    LamWos,
    BetaV,
    LamVz,
    LamOs,
}

fn groups(form: KohForm, free: KohFree) -> Vec<Group> {
    let emu = [Group::BetaU, Group::LamUz, Group::LamWs, Group::LamWos];
    let mut g = Vec::new();
    if form != KohForm::SimulationOnly {
        g.push(Group::Theta);
    }
    if free == KohFree::All {
        g.extend(emu);
    }
    if form == KohForm::WithDiscrepancy {
        g.extend([Group::BetaV, Group::LamVz]);
    }
    if form != KohForm::SimulationOnly {
        g.push(Group::LamOs);
    }
    g
}

fn base(prior: &ParamPrior) -> Result<Distribution> {
    match prior.dist {
        PriorDist::BetaRho { .. } => Ok(Distribution::Uniform(Uniform::new(0.0, 1.0)?)),
        PriorDist::Gamma { a, b } if !(prior.upper.is_finite() && prior.lower.is_finite()) => Ok(Distribution::Gamma(Gamma::new(a, b, prior.lower)?)),
        _ if prior.lower.is_finite() && prior.upper.is_finite() => Ok(Distribution::Uniform(Uniform::new(prior.lower, prior.upper)?)),
        _ => Err(RafflesError::InvalidParameter { parameter: "prior".to_string(), value: prior.lower, reason: "an unbounded non-Gamma prior has no TMCMC base here".to_string() }),
    }
}

impl KohModel {
    fn group_prior(&self, g: Group) -> &ParamPrior {
        let p = &self.priors;
        match g {
            Group::Theta => &p.theta,
            Group::BetaU => &p.beta_u,
            Group::LamUz => &p.lam_uz,
            Group::LamWs => &p.lam_ws,
            Group::LamWos => &p.lam_wos,
            Group::BetaV => &p.beta_v,
            Group::LamVz => &p.lam_vz,
            Group::LamOs => &p.lam_os,
        }
    }

    /// Writes TMCMC coordinates `phi` into a copy of `start`.
    fn decode(gs: &[Group], start: &KohParams, phi: &[f64]) -> KohParams {
        let mut out = start.clone();
        let mut k = 0;
        let rho = |v: f64| -4.0 * v.r_ln();
        for g in gs {
            match g {
                Group::Theta => {
                    for t in &mut out.theta {
                        *t = phi[k];
                        k += 1;
                    }
                }
                Group::BetaU => {
                    for b in &mut out.beta_u {
                        *b = rho(phi[k]);
                        k += 1;
                    }
                }
                Group::BetaV => {
                    for b in &mut out.beta_v {
                        *b = rho(phi[k]);
                        k += 1;
                    }
                }
                Group::LamUz | Group::LamWs | Group::LamWos | Group::LamVz | Group::LamOs => {
                    let v = phi[k];
                    k += 1;
                    match g {
                        Group::LamUz => out.lam_uz = v,
                        Group::LamWs => out.lam_ws = v,
                        Group::LamWos => out.lam_wos = v,
                        Group::LamVz => out.lam_vz = v,
                        _ => out.lam_os = v,
                    }
                }
            }
        }
        out
    }

    /// Samples SEPIA's posterior with TMCMC (module docs), parameters not
    /// freed by `config.free` held at `start`.
    ///
    /// # Errors
    /// The TMCMC driver's; [`RafflesError::InvalidParameter`] if
    /// `CalibrationOnly` is asked of a simulation-only model, if the
    /// emulator at `start` is not positive definite, or for a prior with no
    /// base.
    pub fn sample(&self, start: &KohParams, config: &KohSampleConfig) -> Result<KohPosterior> {
        if self.form == KohForm::SimulationOnly && config.free == KohFree::CalibrationOnly {
            return Err(RafflesError::InvalidParameter { parameter: "free".to_string(), value: 0.0, reason: "a simulation-only model has only emulator parameters".to_string() });
        }
        let gs = groups(self.form, config.free);
        let mut marginals = Vec::new();
        for g in &gs {
            let width = match g {
                Group::Theta => start.theta.len(),
                Group::BetaU => start.beta_u.len(),
                Group::BetaV => start.beta_v.len(),
                _ => 1,
            };
            let b = base(self.group_prior(*g))?;
            marginals.extend(std::iter::repeat(b).take(width));
        }
        let prior = IndependentPrior::new(marginals)?;
        let factor: Option<EmulatorFactor> = match config.free {
            KohFree::All => None,
            KohFree::CalibrationOnly => Some(self.emulator_factor(start).ok_or_else(|| RafflesError::InvalidParameter { parameter: "start".to_string(), value: f64::NAN, reason: "the emulator covariance is not positive definite".to_string() })?),
        };
        let post = |phi: &[f64]| -> f64 {
            let p = Self::decode(&gs, start, phi);
            let lp = self.log_prior(&p);
            if lp == f64::NEG_INFINITY {
                return lp;
            }
            let ll = match &factor {
                Some(f) => self.log_lik_with(&p, f),
                None => self.log_lik(&p),
            };
            ll + lp
        };
        let tc = TransitionalConfig::new(config.population, TemperingCriterion::weight_cov(), config.chain_length, 200, config.seed, TransitionKernel::metropolis_hastings())?;
        let res = tmcmc(&prior, |phi| post(phi) - prior.ln_pdf(phi), &tc)?;
        let samples: Vec<KohParams> = res.samples.iter().map(|phi| Self::decode(&gs, start, phi)).collect();
        let log_post = res.samples.iter().map(|phi| post(phi)).collect();
        Ok(KohPosterior { samples, log_post, ln_evidence: res.ln_evidence, stages: res.stages.len() })
    }
}
