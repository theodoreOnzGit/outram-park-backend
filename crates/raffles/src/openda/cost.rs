// ---------------------------------------------------------------------------
// Ported from OpenDA (open data assimilation toolbox).
//
//   Upstream project: OpenDA, OpenDA Association
//   Upstream repo:    https://github.com/OpenDA-Association/OpenDA
//   Upstream files:   algorithms/java/src/org/openda/algorithms/SimulationKwadraticCostFunction.java
//                       (prepare/evaluate: one model run with announced
//                        observations, Jo and the background term Jb, factor,
//                        best-so-far tracking)
//                     .../algorithms/SimpleCostFunction.java
//                     .../algorithms/RosenbrockCostFunction.java
//                     .../algorithms/SimpleLeastSquaresCostFunction.java
//                     .../algorithms/ICostFunction.java, LeastSquaresCostFunction.java
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
// Translation notes: ICostFunction / LeastSquaresCostFunction become the
// `CostFunction` trait (consumed generically by the optimisers), with
// `least_squares()` returning `Some` where Java tests `instanceof
// LeastSquaresCostFunction`. The model run of SimulationKwadraticCostFunction
// is the `Predictor` seam (a caller's model, or `SimulationPredictor` for an
// `OdaStochModel`); observation filters, bias/std removal, debug files, model
// cloning for parallel runs and result writing are not ported. Squared norms
// are OpenDA's `Math.pow(norm2, 2.0)` (= n·n, measured).
// ---------------------------------------------------------------------------

//! **OpenDA's cost functions**, including the weighted least-squares cost
//! its calibration algorithms minimise (gh:#1192).

use super::linalg::{axpy, norm2_squared, pointwise_divide};
use super::models::{Observations, OdaStochModel};
use super::noise::{DrawSource, StochVector};
use crate::{RafflesError, Result};

/// Every evaluation, in order, and the best so far (OpenDA's `allPars`,
/// `allCosts`, `allPreds`, `pMin`, `fMin`, `predMin`).
#[derive(Debug, Clone, Default, PartialEq)]
pub struct CostHistory {
    /// Evaluated parameters.
    pub parameters: Vec<Vec<f64>>,
    /// Their costs.
    pub costs: Vec<f64>,
    /// Their predictions (least-squares costs only).
    pub predictions: Vec<Vec<f64>>,
    best: Option<(f64, Vec<f64>, Option<Vec<f64>>)>,
}

impl CostHistory {
    /// Record one evaluation; the best is replaced only on a strict
    /// improvement (OpenDA starts from `Double.MAX_VALUE`).
    pub fn record(&mut self, p: &[f64], cost: f64, pred: Option<&[f64]>) {
        self.parameters.push(p.to_vec());
        self.costs.push(cost);
        if let Some(y) = pred {
            self.predictions.push(y.to_vec());
        }
        if cost < self.best.as_ref().map_or(f64::MAX, |b| b.0) {
            self.best = Some((cost, p.to_vec(), pred.map(<[f64]>::to_vec)));
        }
    }

    /// `getOptimalCost()`, `getOptimalParameters()`.
    #[must_use]
    pub fn optimum(&self) -> Option<(f64, &[f64])> {
        self.best.as_ref().map(|b| (b.0, b.1.as_slice()))
    }

    /// The predictions at the optimum.
    #[must_use]
    pub fn optimal_predictions(&self) -> Option<&[f64]> {
        self.best.as_ref().and_then(|b| b.2.as_deref())
    }
}

/// What a least-squares cost exposes to the optimisers.
#[derive(Debug, Clone, PartialEq)]
pub struct LeastSquaresView {
    /// Observations and their (independent) errors.
    pub observations: StochVector,
    /// The parameters' prior uncertainty (search directions, background).
    pub parameters: StochVector,
    /// Whether the background term is in the cost.
    pub background: bool,
    /// Predictions of the last evaluation.
    pub last_predictions: Option<Vec<f64>>,
}

/// OpenDA's `ICostFunction`, consumed generically by [`super::simplex`],
/// [`super::powell`] and [`super::dud`].
pub trait CostFunction {
    /// The cost at `p`, recorded in [`CostFunction::history`].
    ///
    /// # Errors
    /// The model's.
    fn evaluate(&mut self, p: &[f64]) -> Result<f64>;
    /// Every evaluation so far.
    fn history(&self) -> &CostHistory;
    /// `Some` for a least-squares cost (Java's `instanceof`).
    fn least_squares(&self) -> Option<LeastSquaresView> {
        None
    }
    /// `getMultiplicationFactor()`.
    fn multiplication_factor(&self) -> f64 {
        1.0
    }
}

/// OpenDA's `SimpleCostFunction`: `Σ ((p_i − 1)/w_i)²`, `w = (1, 2)`.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct SimpleCost {
    history: CostHistory,
}

impl CostFunction for SimpleCost {
    fn evaluate(&mut self, p: &[f64]) -> Result<f64> {
        let (means, width) = ([1.0, 1.0], [1.0, 2.0]);
        let mut r = 0.0;
        for i in 0..2 {
            let t = (p[i] - means[i]) / width[i];
            r += t * t;
        }
        self.history.record(p, r, None);
        Ok(r)
    }
    fn history(&self) -> &CostHistory {
        &self.history
    }
}

/// OpenDA's `RosenbrockCostFunction` (factor 100, pairs of parameters).
#[derive(Debug, Clone, Default, PartialEq)]
pub struct RosenbrockCost {
    history: CostHistory,
}

impl CostFunction for RosenbrockCost {
    fn evaluate(&mut self, p: &[f64]) -> Result<f64> {
        let mut r = 0.0;
        for i in (0..p.len()).step_by(2) {
            let (x1, x2) = (p[i], p[i + 1]);
            let mut s = x1 * x1 - x2;
            r += 100.0 * s * s;
            s = 1. - x1;
            r += s * s;
        }
        self.history.record(p, r, None);
        Ok(r)
    }
    fn history(&self) -> &CostHistory {
        &self.history
    }
}

/// A caller's model for the weighted least-squares cost: predictions of
/// the observations at parameters `p`.
pub trait Predictor {
    /// The predictions.
    ///
    /// # Errors
    /// The model's.
    fn predict(&self, p: &[f64]) -> Result<Vec<f64>>;
}

/// The identity model `y = p` of `SimpleLeastSquaresCostFunction`.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct IdentityPredictor;

impl Predictor for IdentityPredictor {
    fn predict(&self, p: &[f64]) -> Result<Vec<f64>> {
        Ok(p.to_vec())
    }
}

/// `SimulationKwadraticCostFunction`'s model run for an [`OdaStochModel`]:
/// a fresh copy of the template, `setParameters(p)`, observations
/// announced, run to the horizon's end without noise, then observed.
#[derive(Debug, Clone, PartialEq)]
pub struct SimulationPredictor<M> {
    template: M,
    observations: Observations,
}

impl<M: OdaStochModel> SimulationPredictor<M> {
    /// The observations kept are those in the model's time horizon
    /// `(t_start, t_stop]`, as OpenDA's `createSelection(timeHorizon)`:
    /// **one at `t_start` is dropped**.
    ///
    /// # Errors
    /// No observation in the horizon (OpenDA throws).
    pub fn new(template: M, observations: &Observations) -> Result<Self> {
        let (a, b) = template.time_horizon();
        let sel = observations.select_span(a, b);
        if sel.is_empty() {
            return Err(RafflesError::InvalidParameter { parameter: "observations".into(), value: 0.0, reason: "none within the simulation span".into() });
        }
        Ok(Self { template, observations: sel })
    }

    /// The observations used.
    #[must_use]
    pub fn observations(&self) -> &Observations {
        &self.observations
    }
}

impl<M: OdaStochModel> Predictor for SimulationPredictor<M> {
    fn predict(&self, p: &[f64]) -> Result<Vec<f64>> {
        let mut m = self.template.clone();
        m.set_parameters(p)?;
        m.set_automatic_noise(false);
        m.announce_observed_values(&self.observations);
        let end = m.time_horizon().1;
        m.compute(end, &mut DrawSource::injected(Vec::new(), Vec::new(), Vec::new()))?;
        m.observed_values(&self.observations)
    }
}

/// OpenDA's weighted cost `J = f·‖(y − h(p))/σ‖² + [f·‖L_p⁻¹(p − p̄)‖²]`,
/// `f = factor` (0.5 in OpenDA), each `‖·‖²` as `Math.pow(norm2, 2)`.
#[derive(Debug, Clone, PartialEq)]
pub struct WeightedLeastSquares<P> {
    predictor: P,
    observations: StochVector,
    parameters: StochVector,
    /// The factor on both terms.
    pub factor: f64,
    /// Add the background term.
    pub background: bool,
    /// Add Jb before Jo (`SimpleLeastSquaresCostFunction`'s order).
    pub background_first: bool,
    history: CostHistory,
    last: Option<Vec<f64>>,
}

impl<P: Predictor> WeightedLeastSquares<P> {
    /// From a predictor, observed values with σ, and the parameter prior.
    #[must_use]
    pub fn new(predictor: P, observations: StochVector, parameters: StochVector, factor: f64, background: bool) -> Self {
        Self { predictor, observations, parameters, factor, background, background_first: false, history: CostHistory::default(), last: None }
    }

    /// The predictor.
    #[must_use]
    pub fn predictor(&self) -> &P {
        &self.predictor
    }
}

impl<M: OdaStochModel> WeightedLeastSquares<SimulationPredictor<M>> {
    /// `new SimulationKwadraticCostFunction(factory, observer)` with
    /// `addBackgroundTerm = background`: factor 0.5, the template's parameter
    /// uncertainty, the observations in its horizon.
    ///
    /// # Errors
    /// As [`SimulationPredictor::new`], or a model without parameter
    /// uncertainty.
    pub fn simulation(template: M, observations: &Observations, background: bool) -> Result<Self> {
        let par = template.parameter_uncertainty().ok_or_else(|| RafflesError::InvalidParameter { parameter: "parameter uncertainty".into(), value: 0.0, reason: "the model has none".into() })?;
        let pred = SimulationPredictor::new(template, observations)?;
        let obs = pred.observations().uncertainty()?;
        Ok(Self::new(pred, obs, par, 0.5, background))
    }
}

impl<P: Predictor> CostFunction for WeightedLeastSquares<P> {
    fn evaluate(&mut self, p: &[f64]) -> Result<f64> {
        let prd = self.predictor.predict(p)?;
        if prd.len() != self.observations.len() {
            return Err(RafflesError::DimensionMismatch { expected: self.observations.len(), found: prd.len() });
        }
        let mut res = self.observations.mean.clone();
        axpy(&mut res, -1.0, &prd);
        pointwise_divide(&mut res, &self.observations.std);
        let mut obs_cost = norm2_squared(&res);
        obs_cost *= self.factor;
        let mut back_cost = 0.0;
        if self.background {
            if let Some((i, s)) = self.parameters.std.iter().enumerate().find(|(_, s)| **s < 1e-8) {
                return Err(RafflesError::InvalidParameter { parameter: format!("parameter {} standard deviation", i + 1), value: *s, reason: "zero or negative: no penalty term (OpenDA throws)".into() });
            }
            let mut d = p.to_vec();
            axpy(&mut d, -1.0, &self.parameters.mean);
            let t = self.parameters.sqrt_solve(&d)?;
            back_cost = norm2_squared(&t);
            back_cost *= self.factor;
        }
        // Addition order matters: the weighted cost adds Jo then Jb, the
        // simple least-squares test cost Jb then Jo.
        let mut total = 0.0;
        if self.background && self.background_first {
            total += back_cost;
            total += obs_cost;
        } else {
            total += obs_cost;
            if self.background {
                total += back_cost;
            }
        }
        self.history.record(p, total, Some(&prd));
        self.last = Some(prd);
        Ok(total)
    }
    fn history(&self) -> &CostHistory {
        &self.history
    }
    fn least_squares(&self) -> Option<LeastSquaresView> {
        Some(LeastSquaresView { observations: self.observations.clone(), parameters: self.parameters.clone(), background: self.background, last_predictions: self.last.clone() })
    }
    fn multiplication_factor(&self) -> f64 {
        self.factor
    }
}

/// OpenDA's `SimpleLeastSquaresCostFunction`: `y = p`, observations
/// (1, 1) ± (1, 2), parameter prior (0, 0) ± (1, 2), factor 1.
///
/// # Errors
/// Never (fixed, consistent shapes).
pub fn simple_least_squares(background: bool) -> Result<WeightedLeastSquares<IdentityPredictor>> {
    let mut c = WeightedLeastSquares::new(
        IdentityPredictor,
        StochVector::independent(vec![1.0, 1.0], vec![1.0, 2.0])?,
        StochVector::independent(vec![0.0, 0.0], vec![1.0, 2.0])?,
        1.0,
        background,
    );
    c.background_first = true;
    Ok(c)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::openda::models::{Dynamics, SimpleModelConfig, SimpleStochModel};

    /// The test costs at known points; history and best-so-far; the
    /// weighted cost's two terms; the simulation predictor drops t_start
    /// and reproduces the model; errors.
    #[test]
    fn costs() {
        let mut s = SimpleCost::default();
        assert_eq!(s.evaluate(&[1.0, 1.0]).unwrap(), 0.0);
        assert_eq!(s.evaluate(&[0.0, 0.0]).unwrap(), 1.25);
        assert_eq!(s.history().optimum().unwrap().0, 0.0);
        assert!(s.least_squares().is_none() && s.multiplication_factor() == 1.0);
        let mut r = RosenbrockCost::default();
        assert_eq!(r.evaluate(&[1.0, 1.0, 0.0, 0.0]).unwrap(), 1.0);
        assert_eq!(r.history().costs.len(), 1);
        let mut l = simple_least_squares(true).unwrap();
        // At p = (0, 0): Jb = 0, Jo = ((1−0)/1)² + ((1−0)/2)² = 1.25, taken as
        // OpenDA does, (√1.25)² = 1.2500000000000002.
        assert_eq!(l.evaluate(&[0.0, 0.0]).unwrap(), 1.25_f64.sqrt() * 1.25_f64.sqrt());
        assert!((l.evaluate(&[1.0, 2.0]).unwrap() - 2.25).abs() < 1e-15);
        assert_eq!(l.history().optimal_predictions().unwrap(), &[0.0, 0.0]);
        assert_eq!(l.least_squares().unwrap().last_predictions.unwrap(), vec![1.0, 2.0]);
        assert_eq!(*l.predictor(), IdentityPredictor);
        let mut z = WeightedLeastSquares::new(IdentityPredictor, StochVector::independent(vec![1.0], vec![1.0]).unwrap(), StochVector::independent(vec![0.0], vec![0.0]).unwrap(), 0.5, true);
        assert!(z.evaluate(&[1.0]).is_err());
        assert!(z.evaluate(&[1.0, 2.0]).is_err());
        let tpl = SimpleStochModel::new(Dynamics::oscillator(), &SimpleModelConfig::default()).unwrap();
        let obs = Observations::new(vec![0.0, 1.0, 2.0], vec![0.0; 3], vec![0.8, 0.0, -0.6], vec![0.1; 3], None).unwrap();
        let mut w = WeightedLeastSquares::simulation(tpl.clone(), &obs, false).unwrap();
        assert_eq!(w.predictor().observations().len(), 2);
        let c = w.evaluate(&[8.0, std::f64::consts::FRAC_PI_2]).unwrap();
        assert!(c > 0.0 && c < 10.0 && w.multiplication_factor() == 0.5);
        assert!(WeightedLeastSquares::simulation(tpl, &Observations::new(vec![0.0], vec![0.0], vec![0.8], vec![0.1], None).unwrap(), false).is_err());
    }
}
