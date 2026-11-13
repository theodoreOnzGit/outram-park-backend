// ---------------------------------------------------------------------------
// Ported from OpenDA (open data assimilation toolbox).
//
//   Upstream project: OpenDA, OpenDA Association
//   Upstream repo:    https://github.com/OpenDA-Association/OpenDA
//   Upstream files:   algorithms/java/src/org/openda/algorithms/kalmanFilter/AbstractSequentialAlgorithm.java
//                       (setStochComponents, next: analysis times from the
//                        observer, forecast then analysis, final forecast)
//                     .../kalmanFilter/AbstractSequentialEnsembleAlgorithm.java
//                       (setStochComponents: stochInit / stochParameter /
//                        stochForcing per member, forecast)
//   Upstream commit:  e51565c2c8def7d35f8b378a9ac118edbd98a94d
//   Accessed:         2026-11-04
//
//   Copyright (c) 2012 OpenDA Association. All rights reserved.
//   OpenDA is free software: you can redistribute it and/or modify it under
//   the terms of the GNU Lesser General Public License as published by the
//   Free Software Foundation, either version 3 of the License, or (at your
//   option) any later version. OpenDA is distributed in the hope that it will
//   be useful, but WITHOUT ANY WARRANTY; without even the implied warranty of
//   MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE. See the GNU Lesser
//   General Public License for more details (LICENSE-LGPL-3.0-OPENDA).
//
// This Rust translation is part of RAFFLES / Outram Park, conveyed under
// AGPL-3.0-only as a combined work (GPLv3 section 13 / AGPLv3 section 13).
//
// Translation notes: OpenDA's algorithm class hierarchy (EnKF, EnSR,
// ParticleFilter, SteadyStateFilter under AbstractSequential(Ensemble)
// Algorithm) becomes the `SequentialFilter` enum. Only the default
// "analysis times from the observer" mode is ported; fixed analysis times,
// skipAtInitialTime/FinalTime, restart, result writers and timers are not.
// An analysis time's observations are those within OpenDA's one-second time
// precision of it (OpenDA selects through tolerance spans; the observation
// times here are distinct to far more than that). The main model runs without
// noise (OpenDA's mainModel defaults); members draw in member order.
// ---------------------------------------------------------------------------

//! **OpenDA's sequential assimilation loop** and the filter enum (gh:#1192).

use super::enkf::{enkf_analysis, Localization};
use super::ensr::{ensr_analysis, particle_filter_analysis, steady_state_analysis, SteadyStateGain};
use super::models::{Observations, OdaStochModel, TIME_PRECISION};
use super::noise::DrawSource;
use crate::{RafflesError, Result};

/// OpenDA's `ensembleModel@stochInit/stochParameter/stochForcing`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct EnsembleOptions {
    /// Add a realisation of the state uncertainty to each member.
    pub stoch_init: bool,
    /// Draw each member's parameters from their uncertainty.
    pub stoch_parameter: bool,
    /// Members add system noise while computing.
    pub stoch_forcing: bool,
}

impl Default for EnsembleOptions {
    /// OpenDA's defaults: init and forcing on, parameters off.
    fn default() -> Self {
        Self { stoch_init: true, stoch_parameter: false, stoch_forcing: true }
    }
}

/// The sequential filters RAFFLES ports from OpenDA.
#[derive(Debug, Clone, PartialEq)]
pub enum SequentialFilter {
    /// `EnKF`.
    EnKf {
        /// Members.
        ensemble_size: usize,
        /// Localisation option.
        localization: Localization,
        /// Ensemble generation.
        options: EnsembleOptions,
    },
    /// `EnSR`.
    EnSr {
        /// Members.
        ensemble_size: usize,
        /// Ensemble generation.
        options: EnsembleOptions,
    },
    /// `ParticleFilter` (residual resampling).
    ParticleFilter {
        /// Particles.
        ensemble_size: usize,
        /// Ensemble generation.
        options: EnsembleOptions,
    },
    /// `SteadyStateFilter`: a stored gain on the main model, no ensemble.
    SteadyState(SteadyStateGain),
}

/// One analysis time.
#[derive(Debug, Clone, PartialEq)]
pub struct SequentialStep {
    /// The analysis time.
    pub time: f64,
    /// The observations assimilated.
    pub observations: Observations,
    /// Member states before the analysis (the main model for the
    /// steady-state filter).
    pub forecast: Vec<Vec<f64>>,
    /// Member states after it.
    pub analysis: Vec<Vec<f64>>,
    /// The main model's state after it.
    pub main_after: Vec<f64>,
    /// EnKF only: gain columns before and after localisation.
    pub gain: Option<(Vec<Vec<f64>>, Vec<Vec<f64>>)>,
}

/// A whole run.
#[derive(Debug, Clone, PartialEq)]
pub struct SequentialRun<M> {
    /// Each analysis.
    pub steps: Vec<SequentialStep>,
    /// The main model at the end (OpenDA's current state after the last
    /// analysis and the final forecast).
    pub main: M,
    /// The members at the end.
    pub members: Vec<M>,
}

fn create_members<M: OdaStochModel>(template: &M, n: usize, o: EnsembleOptions, draws: &mut DrawSource) -> Result<Vec<M>> {
    if n < 2 {
        return Err(RafflesError::InvalidParameter { parameter: "ensemble size".into(), value: n as f64, reason: "must be at least 2 (OpenDA)".into() });
    }
    let mut out = Vec::with_capacity(n);
    for _ in 0..n {
        let mut m = template.clone();
        if o.stoch_init {
            if let Some(e) = m.state_uncertainty() {
                let r = e.realization(draws)?;
                m.axpy_on_state(1.0, &r);
            }
        }
        if o.stoch_parameter {
            let p = m.parameter_uncertainty().ok_or_else(|| RafflesError::InvalidParameter { parameter: "stoch_parameter".into(), value: 1.0, reason: "the model has no parameter uncertainty".into() })?;
            let r = p.realization(draws)?;
            m.set_parameters(&r)?;
        }
        m.set_automatic_noise(o.stoch_forcing);
        out.push(m);
    }
    Ok(out)
}

fn forecast<M: OdaStochModel>(members: &mut [M], obs: &Observations, target: f64, draws: &mut DrawSource) -> Result<()> {
    for m in members.iter_mut() {
        if !obs.is_empty() {
            m.announce_observed_values(obs);
        }
        m.compute(target, draws)?;
    }
    Ok(())
}

/// Run `filter` on `template` (a fresh model) against `obs`: OpenDA's
/// `setStochComponents`, then `next()` at each distinct observation time
/// inside the model's horizon, then the final forecast.
///
/// # Errors
/// From the models, the analyses or the draws.
pub fn run_sequential<M: OdaStochModel>(template: &M, obs: &Observations, filter: &SequentialFilter, draws: &mut DrawSource) -> Result<SequentialRun<M>> {
    let mut main = template.clone();
    main.set_automatic_noise(false);
    let mut members = match filter {
        SequentialFilter::EnKf { ensemble_size, options, .. } | SequentialFilter::EnSr { ensemble_size, options } | SequentialFilter::ParticleFilter { ensemble_size, options } => {
            create_members(template, *ensemble_size, *options, draws)?
        }
        SequentialFilter::SteadyState(_) => Vec::new(),
    };
    let (t0, t1) = main.time_horizon();
    let times = obs.select_span(t0 - TIME_PRECISION, t1 + TIME_PRECISION).times();
    let mut current = main.current_time();
    let mut steps = Vec::with_capacity(times.len());
    for &t in &times {
        let sel = obs.select_span(t - TIME_PRECISION, t + TIME_PRECISION);
        if t > current {
            main.announce_observed_values(&sel);
            main.compute(t, draws)?;
            forecast(&mut members, &sel, t, draws)?;
        }
        current = t;
        if sel.is_empty() {
            continue;
        }
        let snap = |members: &[M], main: &M| if members.is_empty() { vec![main.state()] } else { members.iter().map(OdaStochModel::state).collect() };
        let fc = snap(&members, &main);
        let gain = match filter {
            SequentialFilter::EnKf { localization, .. } => {
                let up = enkf_analysis(&mut members, &mut main, &sel, *localization, draws)?;
                Some((up.gain_pre, up.gain_post))
            }
            SequentialFilter::EnSr { .. } => {
                ensr_analysis(&mut members, &mut main, &sel)?;
                None
            }
            SequentialFilter::ParticleFilter { .. } => {
                particle_filter_analysis(&mut members, &mut main, &sel, draws)?;
                None
            }
            SequentialFilter::SteadyState(g) => {
                steady_state_analysis(g, &mut main, &sel)?;
                None
            }
        };
        steps.push(SequentialStep { time: t, observations: sel, forecast: fc, analysis: snap(&members, &main), main_after: main.state(), gain });
    }
    if t1 > current {
        let sel = obs.select_span(current, t1);
        main.compute(t1, draws)?;
        forecast(&mut members, &sel, t1, draws)?;
    }
    Ok(SequentialRun { steps, main, members })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::openda::models::{Dynamics, SimpleModelConfig, SimpleStochModel};

    /// Each filter runs on the oscillator and pulls the final state toward
    /// the observed truth; the steady-state filter runs on the main model;
    /// too small an ensemble and a missing parameter uncertainty are
    /// errors; the final forecast runs past the last observation.
    #[test]
    fn filters_run() {
        let cfg = SimpleModelConfig { initial_state: Some(vec![0.0, 0.0]), initial_state_std: Some(vec![0.8, 0.8]), ..Default::default() };
        let tpl = SimpleStochModel::new(Dynamics::oscillator(), &cfg).unwrap();
        let truth = SimpleStochModel::new(Dynamics::oscillator(), &SimpleModelConfig::default()).unwrap();
        let mut tr = truth.clone();
        let times: Vec<f64> = (0..9).map(f64::from).collect();
        let mut vals = Vec::new();
        let mut d = DrawSource::lcg(3);
        for &t in &times {
            tr.compute(t, &mut d).unwrap();
            vals.push(tr.state()[0]);
        }
        let obs = Observations::new(times.clone(), vec![0.0; 9], vals, vec![0.1; 9], None).unwrap();
        let o = EnsembleOptions::default();
        let filters = [
            SequentialFilter::EnKf { ensemble_size: 20, localization: Localization::None, options: o },
            SequentialFilter::EnSr { ensemble_size: 20, options: o },
            SequentialFilter::ParticleFilter { ensemble_size: 50, options: o },
            SequentialFilter::SteadyState(SteadyStateGain { columns: vec![(0.0, vec![0.9, 0.3])], skip_factor: f64::INFINITY }),
        ];
        for f in &filters {
            let r = run_sequential(&tpl, &obs, f, &mut DrawSource::lcg(11)).unwrap();
            assert_eq!(r.steps.len(), 9);
            let last = &r.steps[8];
            assert!((last.main_after[0] - obs.value[8]).abs() < 0.3, "{f:?}: {:?}", last.main_after);
            assert!((r.main.current_time() - 10.0).abs() < 1e-9, "final forecast to the horizon");
        }
        assert!(run_sequential(&tpl, &obs, &SequentialFilter::EnKf { ensemble_size: 1, localization: Localization::None, options: o }, &mut DrawSource::lcg(1)).is_err());
        let po = EnsembleOptions { stoch_parameter: true, ..o };
        let r = run_sequential(&tpl, &obs, &SequentialFilter::EnKf { ensemble_size: 4, localization: Localization::Hamill, options: po }, &mut DrawSource::lcg(1)).unwrap();
        assert_ne!(r.members[0].dynamics, r.members[1].dynamics, "parameters drawn per member");
    }
}
