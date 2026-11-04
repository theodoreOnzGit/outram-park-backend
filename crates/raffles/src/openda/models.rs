// ---------------------------------------------------------------------------
// Ported from OpenDA (open data assimilation toolbox).
//
//   Upstream project: OpenDA, OpenDA Association
//   Upstream repo:    https://github.com/OpenDA-Association/OpenDA
//   Upstream files:   models/java/src/org/openda/models/simpleModel/SimpleStochModelInstance.java
//                       (Initialize's defaults, compute (RK4 + system noise),
//                        axpyOnState, setParameters, getObservedValues,
//                        announceObservedValues, restoreInternalState,
//                        getObservedLocalization)
//                     models/java/src/org/openda/models/oscillator/OscillatorStochModelInstance.java
//                     models/java/src/org/openda/models/lorenz/LorenzStochModelInstance.java
//                     observers/java/src/org/openda/observers/CsvStochObserver.java
//                       (createSelection over a time span, getTimes, getRealizations,
//                        evaluatePDF; the in-memory column form)
//                     core/java/src/org/openda/utils/Time.java (getTimeStep, inSpan)
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
// Translation notes: these are OpenDA's TEST models, ported so the algorithms
// can be compared with OpenDA end to end; they are not physics models of
// anything. The model-wrapper framework (factories, exchange items, XML
// ConfigTree, result writers, restart files) is not ported: a configuration
// is the `SimpleModelConfig` struct. The parameter hashtable becomes the
// `Dynamics` enum's fields. `OdaStochModel` is the trait the ported
// algorithms consume (generically), written here after OpenDA's
// IStochModelInstance; it is not a port of that interface's full surface.
// ---------------------------------------------------------------------------

//! **OpenDA's test models** (oscillator, Lorenz-63), the observer, and the
//! model seam the sequential algorithms consume (gh:#1192).

use super::noise::{DrawSource, StochVector};
use super::linalg;
use crate::{RafflesError, Result};

/// OpenDA's time precision, one second in days: `1.0/24.0/60.0/60.0`.
pub const TIME_PRECISION: f64 = 1.0 / 24.0 / 60.0 / 60.0;

/// Java's `Math.round(double)` for the values a step count takes:
/// `⌊x + ½⌋` evaluated exactly.
#[must_use]
pub fn java_round(x: f64) -> i64 {
    let f = x.floor();
    (if x - f >= 0.5 { f + 1.0 } else { f }) as i64
}

/// Observations in OpenDA's `CsvStochObserver` column form, independent
/// Gaussian errors.
#[derive(Debug, Clone, PartialEq)]
pub struct Observations {
    /// Observation time, model time units.
    pub time: Vec<f64>,
    /// State index observed (OpenDA's `index` column).
    pub index: Vec<f64>,
    /// Observed value.
    pub value: Vec<f64>,
    /// Standard deviation.
    pub std: Vec<f64>,
    /// OpenDA's `transform` column: `2` observes the square of the state.
    pub transform: Option<Vec<f64>>,
}

impl Observations {
    /// From columns.
    ///
    /// # Errors
    /// Columns of different lengths.
    pub fn new(time: Vec<f64>, index: Vec<f64>, value: Vec<f64>, std: Vec<f64>, transform: Option<Vec<f64>>) -> Result<Self> {
        let n = time.len();
        for l in [index.len(), value.len(), std.len(), transform.as_ref().map_or(n, Vec::len)] {
            if l != n {
                return Err(RafflesError::DimensionMismatch { expected: n, found: l });
            }
        }
        Ok(Self { time, index, value, std, transform })
    }

    /// Number of observations.
    #[must_use]
    pub fn len(&self) -> usize {
        self.time.len()
    }

    /// True if none.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.time.is_empty()
    }

    /// `createSelection(span)`: the observations whose time stamp lies in
    /// `(begin, end]` (`Time.inSpan`; note the open lower end).
    #[must_use]
    pub fn select_span(&self, begin: f64, end: f64) -> Self {
        let keep: Vec<usize> = (0..self.len()).filter(|&i| self.time[i] > begin && self.time[i] <= end).collect();
        let pick = |v: &Vec<f64>| keep.iter().map(|&i| v[i]).collect::<Vec<f64>>();
        Self { time: pick(&self.time), index: pick(&self.index), value: pick(&self.value), std: pick(&self.std), transform: self.transform.as_ref().map(pick) }
    }

    /// The distinct observation times, sorted (`getTimes`).
    #[must_use]
    pub fn times(&self) -> Vec<f64> {
        let mut t = self.time.clone();
        t.sort_by(f64::total_cmp);
        t.dedup();
        t
    }

    /// The observation error as a `StochVector`.
    ///
    /// # Errors
    /// Never for a constructed `Observations` (same lengths).
    pub fn uncertainty(&self) -> Result<StochVector> {
        StochVector::independent(self.value.clone(), self.std.clone())
    }

    /// `getRealizations()`: one perturbed observation vector.
    ///
    /// # Errors
    /// The draw source's.
    pub fn realization(&self, draws: &mut DrawSource) -> Result<Vec<f64>> {
        self.uncertainty()?.realization(draws)
    }
}

/// The model seam OpenDA's sequential algorithms use (after
/// `IStochModelInstance`), consumed generically. RAFFLES implements it for
/// its test models ([`SimpleStochModel`]) and for a
/// [`crate::fidelity::StateSpaceModel`]; a downstream crate may implement it
/// for its own model.
pub trait OdaStochModel: Clone {
    /// The state vector.
    fn state(&self) -> Vec<f64>;
    /// `x = x + alpha·v`.
    fn axpy_on_state(&mut self, alpha: f64, v: &[f64]);
    /// Current model time.
    fn current_time(&self) -> f64;
    /// Start and end of the time horizon.
    fn time_horizon(&self) -> (f64, f64);
    /// Advance to `target`, drawing system noise from `draws` when automatic
    /// noise is on.
    ///
    /// # Errors
    /// The model's own.
    fn compute(&mut self, target: f64, draws: &mut DrawSource) -> Result<()>;
    /// Start storing states for `obs` (OpenDA's announceObservedValues).
    fn announce_observed_values(&mut self, obs: &Observations);
    /// The model's prediction of `obs`.
    ///
    /// # Errors
    /// The model's own (e.g. an observation outside the stored states).
    fn observed_values(&self, obs: &Observations) -> Result<Vec<f64>>;
    /// The initial-state uncertainty, if any.
    fn state_uncertainty(&self) -> Option<StochVector>;
    /// The parameter uncertainty, if any.
    fn parameter_uncertainty(&self) -> Option<StochVector>;
    /// Set the uncertain parameters.
    ///
    /// # Errors
    /// A wrong number of parameters.
    fn set_parameters(&mut self, p: &[f64]) -> Result<()>;
    /// Switch automatic system noise on or off.
    fn set_automatic_noise(&mut self, on: bool);
    /// Hamill's localisation mask, one state-length vector per observation
    /// (OpenDA's default, here too: all ones).
    fn observed_localization(&self, obs: &Observations) -> Vec<Vec<f64>> {
        vec![vec![1.0; self.state().len()]; obs.len()]
    }
    /// `restoreInternalState(saved)`: become a copy of `saved`'s state.
    fn restore_from(&mut self, saved: &Self) {
        *self = saved.clone();
    }
}

/// The right-hand side of a [`SimpleStochModel`].
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Dynamics {
    /// `dx/dt = u`, `du/dt = −ω²x − (2/t_damp)u`.
    Oscillator {
        /// Damping time scale \[s\].
        t_damp: f64,
        /// Angular frequency \[rad/s\].
        omega: f64,
    },
    /// Lorenz (1963).
    Lorenz63 {
        /// σ.
        sigma: f64,
        /// ρ.
        rho: f64,
        /// β.
        beta: f64,
    },
}

impl Dynamics {
    /// OpenDA's oscillator defaults: t_damp 8, ω = 0.25·2π.
    #[must_use]
    pub fn oscillator() -> Self {
        Self::Oscillator { t_damp: 8.0, omega: 0.25 * 2.0 * std::f64::consts::PI }
    }

    /// OpenDA's Lorenz defaults: σ 10, ρ 28, β 8/3.
    #[must_use]
    pub fn lorenz63() -> Self {
        Self::Lorenz63 { sigma: 10.0, rho: 28.0, beta: 8.0 / 3.0 }
    }

    fn dx(&self, x: &[f64]) -> Vec<f64> {
        match *self {
            Self::Oscillator { t_damp, omega } => vec![x[1], -(omega * omega) * x[0] - (2.0 / t_damp * x[1])],
            Self::Lorenz63 { sigma, rho, beta } => vec![sigma * (x[1] - x[0]), rho * x[0] - x[1] - x[0] * x[2], x[0] * x[1] - beta * x[2]],
        }
    }

    /// The uncertain parameters, in OpenDA's `stochParNames` order.
    #[must_use]
    pub fn parameters(&self) -> Vec<f64> {
        match *self {
            Self::Oscillator { t_damp, omega } => vec![t_damp, omega],
            Self::Lorenz63 { sigma, rho, beta } => vec![sigma, rho, beta],
        }
    }

    fn set(&mut self, p: &[f64]) -> Result<()> {
        let want = self.parameters().len();
        if p.len() != want {
            return Err(RafflesError::DimensionMismatch { expected: want, found: p.len() });
        }
        // OpenDA's setParameters: Math.max(value, 0.0).
        let c = |v: f64| v.max(0.0);
        match self {
            Self::Oscillator { t_damp, omega } => {
                *t_damp = c(p[0]);
                *omega = c(p[1]);
            }
            Self::Lorenz63 { sigma, rho, beta } => {
                *sigma = c(p[0]);
                *rho = c(p[1]);
                *beta = c(p[2]);
            }
        }
        Ok(())
    }

    fn default_state(&self) -> Vec<f64> {
        match self {
            Self::Oscillator { .. } => vec![0.8, 0.0],
            Self::Lorenz63 { .. } => vec![1.508870, -1.531271, 25.46091],
        }
    }

    fn default_timespan(&self) -> [f64; 3] {
        match self {
            Self::Oscillator { .. } => [0.0, 0.05, 10.0],
            Self::Lorenz63 { .. } => [0.0, 0.025, 30.0],
        }
    }

    fn default_parameter_std(&self) -> Vec<f64> {
        match self {
            Self::Oscillator { .. } => vec![1.0, 0.02 * 2.0 * std::f64::consts::PI],
            Self::Lorenz63 { .. } => vec![1.0, 2.0, 0.3],
        }
    }

    fn default_noise_std(&self) -> f64 {
        match self {
            Self::Oscillator { .. } => 0.3,
            Self::Lorenz63 { .. } => 0.01,
        }
    }

}

/// The XML configuration of OpenDA's simple models, as a struct: `None`
/// keeps the model's default (OpenDA's `Initialize`).
#[derive(Debug, Clone, Default, PartialEq)]
pub struct SimpleModelConfig {
    /// `simulationTimespan`: `[t_start, t_step, t_stop]`.
    pub timespan: Option<[f64; 3]>,
    /// `parameters` (all of them, in `stochParNames` order).
    pub parameters: Option<Vec<f64>>,
    /// `parameterUncertainty`: standard deviations (the mean is the
    /// parameters' values at that point).
    pub parameter_std: Option<Vec<f64>>,
    /// `systemNoise`: `(mean, std)`.
    pub system_noise: Option<(Vec<f64>, Vec<f64>)>,
    /// `initialState`.
    pub initial_state: Option<Vec<f64>>,
    /// `initialStateUncertainty` (only read with `initial_state`, as OpenDA).
    pub initial_state_std: Option<Vec<f64>>,
}

/// OpenDA's `SimpleStochModelInstance`: RK4 with step `t_step`, optional
/// system noise `√t_step·w` after each step, and stored states for
/// observations.
#[derive(Debug, Clone, PartialEq)]
pub struct SimpleStochModel {
    /// The right-hand side and its parameters.
    pub dynamics: Dynamics,
    state: Vec<f64>,
    t: f64,
    time_step: i64,
    timespan: [f64; 3],
    stoch_pars: StochVector,
    sys_noise: StochVector,
    initial_uncertainty: Option<StochVector>,
    auto_noise: bool,
    store_obs: bool,
    t_store: Vec<f64>,
    i_store: Vec<i64>,
    x_store: Vec<Vec<f64>>,
    localization_mask: Option<Vec<f64>>,
}

impl SimpleStochModel {
    /// A model with `dynamics`' defaults overridden by `config`.
    ///
    /// # Errors
    /// Lengths that do not match the dynamics.
    pub fn new(mut dynamics: Dynamics, config: &SimpleModelConfig) -> Result<Self> {
        let timespan = config.timespan.unwrap_or_else(|| dynamics.default_timespan());
        // The constructor's uncertainty is centred on the DEFAULT parameters;
        // `parameterUncertainty` (parsed after `parameters`) re-centres it.
        let mut stoch_pars = StochVector::independent(dynamics.parameters(), dynamics.default_parameter_std())?;
        if let Some(p) = &config.parameters {
            dynamics.set_unclamped(p)?;
        }
        if let Some(sd) = &config.parameter_std {
            stoch_pars = StochVector::independent(dynamics.parameters(), sd.clone())?;
        }
        let n = dynamics.default_state().len();
        let sys_noise = match &config.system_noise {
            Some((m, s)) => StochVector::independent(m.clone(), s.clone())?,
            None => StochVector::independent(vec![0.0; n], vec![dynamics.default_noise_std(); n])?,
        };
        let mut state = dynamics.default_state();
        // OpenDA's oscillator and Lorenz classes REDECLARE
        // `initialStateUncertainty`, so their constructor's default (std
        // `default_initial_std`) sets a shadowing field that
        // `getStateUncertainty()` never returns: without a configured
        // `initialStateUncertainty` the model has none.
        let mut initial_uncertainty = None;
        if let Some(s0) = &config.initial_state {
            state = s0.clone();
            if let Some(sd) = &config.initial_state_std {
                initial_uncertainty = Some(StochVector::independent(vec![0.0; sd.len()], sd.clone())?);
            }
        }
        if state.len() != n || sys_noise.len() != n {
            return Err(RafflesError::DimensionMismatch { expected: n, found: state.len().min(sys_noise.len()) });
        }
        Ok(Self {
            dynamics,
            state,
            t: timespan[0],
            time_step: 0,
            timespan,
            stoch_pars,
            sys_noise,
            initial_uncertainty,
            auto_noise: false,
            store_obs: false,
            t_store: Vec::new(),
            i_store: Vec::new(),
            x_store: Vec::new(),
            localization_mask: None,
        })
    }

    /// Replace Hamill's mask (all ones by default) by the same `mask` for
    /// every observation: a test device (OpenDA's oscillator has none).
    #[must_use]
    pub fn with_localization_mask(mut self, mask: Vec<f64>) -> Self {
        self.localization_mask = Some(mask);
        self
    }

    fn step_index(&self, t_obs: f64) -> i64 {
        // Time.getTimeStep: ceil((t − precision − t_start)/step).
        ((t_obs - TIME_PRECISION - self.timespan[0]) / self.timespan[1]).ceil() as i64
    }
}

impl Dynamics {
    fn set_unclamped(&mut self, p: &[f64]) -> Result<()> {
        let want = self.parameters().len();
        if p.len() != want {
            return Err(RafflesError::DimensionMismatch { expected: want, found: p.len() });
        }
        match self {
            Self::Oscillator { t_damp, omega } => {
                *t_damp = p[0];
                *omega = p[1];
            }
            Self::Lorenz63 { sigma, rho, beta } => {
                *sigma = p[0];
                *rho = p[1];
                *beta = p[2];
            }
        }
        Ok(())
    }
}

impl OdaStochModel for SimpleStochModel {
    fn state(&self) -> Vec<f64> {
        self.state.clone()
    }

    fn axpy_on_state(&mut self, alpha: f64, v: &[f64]) {
        linalg::axpy(&mut self.state, alpha, v);
        if self.store_obs {
            if let Some(last) = self.x_store.last_mut() {
                last.clone_from(&self.state);
            }
        }
    }

    fn current_time(&self) -> f64 {
        self.t
    }

    fn time_horizon(&self) -> (f64, f64) {
        (self.timespan[0], self.timespan[2])
    }

    fn compute(&mut self, target: f64, draws: &mut DrawSource) -> Result<()> {
        let dt = self.timespan[1];
        let nsteps = java_round((target - self.t) / dt);
        for _ in 0..nsteps.max(0) {
            let x = &self.state;
            let d0 = self.dynamics.dx(x);
            let mut xn = x.clone();
            linalg::axpy(&mut xn, 0.5 * dt, &d0);
            let d1 = self.dynamics.dx(&xn);
            let mut xn = x.clone();
            linalg::axpy(&mut xn, 0.5 * dt, &d1);
            let d2 = self.dynamics.dx(&xn);
            let mut xn = x.clone();
            linalg::axpy(&mut xn, 1.0 * dt, &d2);
            let d3 = self.dynamics.dx(&xn);
            let x = &mut self.state;
            linalg::axpy(x, 1.0 / 6.0 * dt, &d0);
            linalg::axpy(x, 2.0 / 6.0 * dt, &d1);
            linalg::axpy(x, 2.0 / 6.0 * dt, &d2);
            linalg::axpy(x, 1.0 / 6.0 * dt, &d3);
            if self.auto_noise {
                let w = self.sys_noise.realization(draws)?;
                linalg::axpy(&mut self.state, dt.sqrt(), &w);
            }
            self.t += dt;
            self.time_step += 1;
            if self.store_obs {
                self.t_store.push(self.t);
                self.x_store.push(self.state.clone());
                self.i_store.push(self.time_step);
            }
        }
        Ok(())
    }

    fn announce_observed_values(&mut self, _obs: &Observations) {
        self.t_store.clear();
        self.x_store.clear();
        self.i_store.clear();
        self.store_obs = true;
        self.t_store.push(self.t);
        self.x_store.push(self.state.clone());
        self.i_store.push(self.time_step);
    }

    fn observed_values(&self, obs: &Observations) -> Result<Vec<f64>> {
        let mut out = Vec::with_capacity(obs.len());
        for i in 0..obs.len() {
            let ix = obs.index[i] as i64;
            if ix < 0 || ix as usize >= self.state.len() {
                return Err(RafflesError::InvalidParameter { parameter: "observation index".into(), value: obs.index[i], reason: "outside the state".into() });
            }
            let mut v = if self.store_obs {
                let it = self.step_index(obs.time[i]);
                let k = self.i_store.iter().position(|&s| s == it).ok_or_else(|| RafflesError::InvalidParameter {
                    parameter: "observation time".into(),
                    value: obs.time[i],
                    reason: "outside the stored states".into(),
                })?;
                self.x_store[k][ix as usize]
            } else {
                self.state[ix as usize]
            };
            if obs.transform.as_ref().is_some_and(|t| t[i] == 2.0) {
                v *= v;
            }
            out.push(v);
        }
        Ok(out)
    }

    fn state_uncertainty(&self) -> Option<StochVector> {
        self.initial_uncertainty.clone()
    }

    fn parameter_uncertainty(&self) -> Option<StochVector> {
        Some(self.stoch_pars.clone())
    }

    fn set_parameters(&mut self, p: &[f64]) -> Result<()> {
        self.dynamics.set(p)
    }

    fn set_automatic_noise(&mut self, on: bool) {
        self.auto_noise = on;
    }

    fn observed_localization(&self, obs: &Observations) -> Vec<Vec<f64>> {
        let m = self.localization_mask.clone().unwrap_or_else(|| vec![1.0; self.state.len()]);
        vec![m; obs.len()]
    }

    /// OpenDA restores state, time and step and **stops storing** (it clears
    /// the stored states).
    fn restore_from(&mut self, saved: &Self) {
        self.state.clone_from(&saved.state);
        self.time_step = saved.time_step;
        self.t = saved.t;
        if self.store_obs {
            self.t_store.clear();
            self.x_store.clear();
            self.i_store.clear();
            self.store_obs = false;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The noise-free oscillator decays and oscillates at ω; stored states
    /// answer observations at their time; parameters clamp at 0; restore
    /// copies state and time; Lorenz runs; observation selection is
    /// `(begin, end]`; bad shapes are errors.
    #[test]
    fn models_and_observations() {
        let mut m = SimpleStochModel::new(Dynamics::oscillator(), &SimpleModelConfig::default()).unwrap();
        let obs = Observations::new(vec![0.0, 1.0, 2.0], vec![0.0; 3], vec![0.0; 3], vec![0.1; 3], None).unwrap();
        m.announce_observed_values(&obs);
        let mut d = DrawSource::lcg(1);
        m.compute(2.0, &mut d).unwrap();
        let y = m.observed_values(&obs).unwrap();
        // x(t) = 0.8 e^{-t/8}(cos ω_d t + sin(ω_d t)/(8ω_d)), ω_d = √(ω² − 1/64), ω = π/2.
        let w = (std::f64::consts::FRAC_PI_2.powi(2) - 1.0 / 64.0).sqrt();
        let exact = |t: f64| 0.8 * (-t / 8.0).exp() * ((w * t).cos() + (w * t).sin() / (8.0 * w));
        for (t, v) in [0.0, 1.0, 2.0].iter().zip(&y) {
            assert!((v - exact(*t)).abs() < 1e-6, "{t} {v}");
        }
        assert!((m.current_time() - 2.0).abs() < 1e-12 && m.time_horizon() == (0.0, 10.0));
        assert!(m.observed_values(&Observations::new(vec![5.0], vec![0.0], vec![0.0], vec![0.1], None).unwrap()).is_err());
        assert!(m.observed_values(&Observations::new(vec![1.0], vec![7.0], vec![0.0], vec![0.1], None).unwrap()).is_err());
        let sq = Observations::new(vec![2.0], vec![0.0], vec![0.0], vec![0.1], Some(vec![2.0])).unwrap();
        assert_eq!(m.observed_values(&sq).unwrap()[0], y[2] * y[2]);
        m.axpy_on_state(1.0, &[1.0, 0.0]);
        assert_eq!(m.observed_values(&sq).unwrap()[0], (y[2] + 1.0) * (y[2] + 1.0), "axpy updates the last stored state");
        m.set_parameters(&[-1.0, 2.0]).unwrap();
        assert_eq!(m.dynamics.parameters(), vec![0.0, 2.0]);
        assert!(m.set_parameters(&[1.0]).is_err());
        let fresh = SimpleStochModel::new(Dynamics::oscillator(), &SimpleModelConfig::default()).unwrap();
        m.restore_from(&fresh);
        assert_eq!((m.state(), m.current_time()), (vec![0.8, 0.0], 0.0));
        assert_eq!(m.observed_values(&sq).unwrap()[0], 0.8 * 0.8, "no longer storing");
        assert_eq!(m.observed_localization(&obs), vec![vec![1.0; 2]; 3]);
        let lm = fresh.clone().with_localization_mask(vec![1.0, 0.4]);
        assert_eq!(lm.observed_localization(&obs)[2], vec![1.0, 0.4]);
        assert!(m.state_uncertainty().is_none(), "shadowed field: no default state uncertainty");
        assert_eq!(m.parameter_uncertainty().unwrap().std[0], 1.0);
        let pc = SimpleModelConfig { parameters: Some(vec![9.0, 1.0]), ..Default::default() };
        assert_eq!(SimpleStochModel::new(Dynamics::oscillator(), &pc).unwrap().parameter_uncertainty().unwrap().mean[0], 8.0);
        let pc = SimpleModelConfig { parameter_std: Some(vec![2.0, 0.1]), ..pc };
        assert_eq!(SimpleStochModel::new(Dynamics::oscillator(), &pc).unwrap().parameter_uncertainty().unwrap().mean[0], 9.0);
        let mut noisy = fresh.clone();
        noisy.set_automatic_noise(true);
        noisy.compute(0.1, &mut d).unwrap();
        assert_ne!(noisy.state(), {
            let mut q = fresh.clone();
            q.compute(0.1, &mut d).unwrap();
            q.state()
        });
        let cfg = SimpleModelConfig { timespan: Some([0.0, 0.025, 1.0]), initial_state: Some(vec![1.0, 1.0, 1.0]), initial_state_std: Some(vec![0.5; 3]), ..Default::default() };
        let mut l = SimpleStochModel::new(Dynamics::lorenz63(), &cfg).unwrap();
        assert_eq!(l.state_uncertainty().unwrap().std, vec![0.5; 3]);
        l.compute(1.0, &mut d).unwrap();
        assert!(l.state().iter().all(|v| v.is_finite()) && l.state() != vec![1.0; 3]);
        assert!(SimpleStochModel::new(Dynamics::lorenz63(), &SimpleModelConfig { initial_state: Some(vec![1.0]), ..Default::default() }).is_err());
        assert!(SimpleStochModel::new(Dynamics::lorenz63(), &SimpleModelConfig { parameters: Some(vec![1.0]), ..Default::default() }).is_err());
        let all = Observations::new(vec![0.0, 1.0, 1.0, 2.0], vec![0.0; 4], vec![1.0, 2.0, 3.0, 4.0], vec![0.1; 4], Some(vec![1.0; 4])).unwrap();
        assert_eq!(all.select_span(0.0, 1.0).value, vec![2.0, 3.0]);
        assert_eq!(all.times(), vec![0.0, 1.0, 2.0]);
        assert!(!all.is_empty() && all.len() == 4);
        let mut d1 = DrawSource::injected(vec![1.0; 4], vec![], vec![]);
        assert_eq!(all.realization(&mut d1).unwrap()[0], 1.1);
        assert!(Observations::new(vec![0.0], vec![], vec![0.0], vec![0.1], None).is_err());
        assert_eq!((java_round(2.5), java_round(2.4999), java_round(-0.5)), (3, 2, 0));
    }
}
