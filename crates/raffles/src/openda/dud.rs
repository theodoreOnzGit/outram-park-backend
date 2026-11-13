// ---------------------------------------------------------------------------
// Ported from OpenDA (open data assimilation toolbox).
//
//   Upstream project: OpenDA, OpenDA Association
//   Upstream repo:    https://github.com/OpenDA-Association/OpenDA
//   Upstream files:   algorithms/java/src/org/openda/algorithms/BaseDudCoreOptimizer.java
//                       (initialize(pInit), sortByCostAndCashObservationData,
//                        next, isThereMoreToDo, infNorm; the unconstrained path)
//                     .../algorithms/DudCoreOptimizer.java
//                       (InitialSearchStep, CalculateParsMatrix,
//                        CalculateGradSimu, BackgroundMatrix)
//                     core/java/src/org/openda/utils/SqrtCovariance.java (asVectorArray)
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
// Translation notes: DUD (Ralston & Jennrich 1978) as OpenDA implements it.
// Ported faithfully, NOT fixed: backtracking flips `inner_scale_fac`'s sign on
// the OPTIMISER at inner iteration `min_inner_negative_look + 2`, so every later
// outer iteration backtracks in the negative direction too; `isThereMoreToDo`
// returns true whenever an inner iteration happened and `imain < maxit`.
// Not ported: inequality constraints (`add_constraints`, an active-set QP),
// the restart initialiser, extra stop criteria, parallel evaluations, and
// correlated observations (OpenDA throws for them too).
// ---------------------------------------------------------------------------

//! **OpenDA's DUD** ("doesn't use derivatives"), a secant Gauss–Newton for
//! weighted least squares (gh:#1192).

use super::cost::{CostFunction, LeastSquaresView};
use super::jama::Rows;
use super::linalg::{self, axpy, pointwise_divide};
use super::simplex::sorted_index;
use crate::{RafflesError, Result};

/// OpenDA's `DudCoreOptimizer` (unconstrained).
#[derive(Debug, Clone, PartialEq)]
pub struct Dud {
    /// `initialStep` (1.0): the search steps are this times the prior
    /// uncertainty's columns.
    pub initial_step: f64,
    /// `maxit` (3).
    pub max_iterations: usize,
    /// `maxInnerIter` (6).
    pub max_inner_iterations: usize,
    /// `maxTotalEvaluations` (None = OpenDA's −1, unlimited).
    pub max_total_evaluations: Option<usize>,
    /// `innerScaleFac` (0.5). Its sign flips during backtracking.
    pub inner_scale_fac: f64,
    /// `minInnerNegativeLook` (3).
    pub min_inner_negative_look: usize,
    /// `maxStep` (10), in the basis of the stored parameter differences.
    pub max_step: f64,
    /// `absTol` (0.01).
    pub abs_tol: f64,
    /// `relTol` (0.01).
    pub rel_tol: f64,
    /// `relTolLinCost` (0.01).
    pub rel_tol_lin_cost: f64,
    /// `computeErrorEstimate` (true).
    pub compute_error_estimate: bool,
    pars: Vec<Vec<f64>>,
    preds: Vec<Vec<f64>>,
    costs: Vec<f64>,
    ls: Option<LeastSquaresView>,
    cost_factor: f64,
    iterations: usize,
    total_inner: usize,
    more_to_do: bool,
    std: Option<Vec<f64>>,
    sqrt_cov: Option<Rows>,
}

impl Default for Dud {
    fn default() -> Self {
        Self {
            initial_step: 1.0,
            max_iterations: 3,
            max_inner_iterations: 6,
            max_total_evaluations: None,
            inner_scale_fac: 0.5,
            min_inner_negative_look: 3,
            max_step: 10.0,
            abs_tol: 0.01,
            rel_tol: 0.01,
            rel_tol_lin_cost: 0.01,
            compute_error_estimate: true,
            pars: Vec::new(),
            preds: Vec::new(),
            costs: Vec::new(),
            ls: None,
            cost_factor: 1.0,
            iterations: 0,
            total_inner: 0,
            more_to_do: true,
            std: None,
            sqrt_cov: None,
        }
    }
}

fn not_ls() -> RafflesError {
    RafflesError::InvalidParameter { parameter: "cost function".into(), value: 0.0, reason: "DUD needs a least-squares cost".into() }
}

fn last_preds<C: CostFunction>(f: &C) -> Result<Vec<f64>> {
    f.least_squares().and_then(|v| v.last_predictions).ok_or_else(not_ls)
}

impl Dud {
    /// `initialize(pInit)`: `pInit` and `pInit + initial_step·L_j` for each
    /// column `L_j` of the prior's square root, evaluated in order, then
    /// sorted by cost.
    ///
    /// # Errors
    /// A cost that is not least squares, or the cost's.
    pub fn initialize<C: CostFunction>(&mut self, f: &mut C, p_init: &[f64]) -> Result<()> {
        let ls = f.least_squares().ok_or_else(not_ls)?;
        if ls.observations.correlated.is_some() {
            return Err(RafflesError::InvalidParameter { parameter: "observations".into(), value: 0.0, reason: "DUD is not implemented for correlated observations (OpenDA)".into() });
        }
        let dirs = linalg::columns(&ls.parameters.sqrt_matrix());
        let m = dirs.len();
        let mut pars = vec![p_init.to_vec()];
        for i in 0..m {
            let mut p = p_init.to_vec();
            for (j, d) in dirs.iter().enumerate() {
                axpy(&mut p, self.initial_step * if i == j { 1.0 } else { 0.0 }, d);
            }
            pars.push(p);
        }
        let mut costs = Vec::with_capacity(m + 1);
        let mut preds = Vec::with_capacity(m + 1);
        for p in &pars {
            costs.push(f.evaluate(p)?);
            preds.push(last_preds(f)?);
            self.total_inner += 1;
        }
        self.cost_factor = f.multiplication_factor();
        let ix = sorted_index(&costs);
        self.costs = ix.iter().map(|&i| costs[i]).collect();
        self.pars = ix.iter().map(|&i| pars[i].clone()).collect();
        self.preds = ix.iter().map(|&i| preds[i].clone()).collect();
        self.ls = f.least_squares();
        Ok(())
    }

    /// The stored points, best first.
    #[must_use]
    pub fn current(&self) -> (&[Vec<f64>], &[f64]) {
        (&self.pars, &self.costs)
    }

    /// Outer iterations done.
    #[must_use]
    pub fn iterations(&self) -> usize {
        self.iterations
    }

    /// The linearised standard deviations of the parameters at the last
    /// iteration (`getStdValues()`).
    #[must_use]
    pub fn std_values(&self) -> Option<&[f64]> {
        self.std.as_deref()
    }

    /// The linearised square root of the parameters' covariance
    /// (`getSqrtCovariance()`).
    #[must_use]
    pub fn sqrt_covariance(&self) -> Option<&Rows> {
        self.sqrt_cov.as_ref()
    }

    /// `optimize()`.
    ///
    /// # Errors
    /// The cost's, the SVD's, or a call before [`Dud::initialize`].
    pub fn optimize<C: CostFunction>(&mut self, f: &mut C) -> Result<()> {
        if self.pars.is_empty() {
            return Err(RafflesError::InvalidParameter { parameter: "dud".into(), value: 0.0, reason: "initialize first".into() });
        }
        while self.more_to_do {
            self.next(f)?;
        }
        Ok(())
    }

    #[allow(clippy::too_many_lines)]
    fn next<C: CostFunction>(&mut self, f: &mut C) -> Result<()> {
        let ls = self.ls.clone().ok_or_else(not_ls)?;
        let ndirs = self.pars.len() - 1;
        self.iterations += 1;
        let p0 = self.pars[0].clone();
        let diffs: Vec<Vec<f64>> = (0..ndirs)
            .map(|i| {
                let mut v = self.pars[i + 1].clone();
                axpy(&mut v, -1.0, &p0);
                v
            })
            .collect();
        let p_matrix = linalg::from_columns(&diffs);
        let mut rhs = ls.observations.mean.clone();
        axpy(&mut rhs, -1.0, &self.preds[0]);
        pointwise_divide(&mut rhs, &ls.observations.std);
        let grad: Vec<Vec<f64>> = (0..ndirs)
            .map(|i| {
                let mut v = self.preds[i + 1].clone();
                axpy(&mut v, -1.0, &self.preds[0]);
                pointwise_divide(&mut v, &ls.observations.std);
                v
            })
            .collect();
        let mut a = linalg::from_columns(&grad);
        if ls.background {
            let mut rb = ls.parameters.mean.clone();
            axpy(&mut rb, -1.0, &p0);
            let rb = ls.parameters.sqrt_solve(&rb)?;
            let back = diffs.iter().map(|d| ls.parameters.sqrt_solve(d)).collect::<Result<Vec<_>>>()?;
            a = linalg::concat_vertical(&a, &linalg::from_columns(&back));
            rhs.extend(rb);
        }
        let mut step = linalg::right_solve(&a, &rhs)?;
        let mut inf = 0.0;
        for v in &step {
            if v.abs() > inf {
                inf = v.abs();
            }
        }
        if inf > self.max_step {
            linalg::scale(&mut step, self.max_step / inf);
        }
        let mut res_lin = rhs.clone();
        linalg::right_multiply(&a, 1.0, &step, -1.0, &mut res_lin);
        let mut cost_lin = linalg::norm2(&res_lin);
        cost_lin = cost_lin * cost_lin * self.cost_factor;
        let mut inner = 0usize;
        let mut scale_step = 1.0;
        let mut rel_error_lin = 0.0;
        let (p_try, cost_try, pred_try) = loop {
            let mut p_try = p0.clone();
            linalg::right_multiply(&p_matrix, scale_step, &step, 1.0, &mut p_try);
            let cost_try = f.evaluate(&p_try)?;
            if inner == 0 {
                rel_error_lin = if (self.costs[0] - cost_lin).abs() <= 1e-14 { 0.0 } else { ((cost_try - cost_lin) / (self.costs[0] - cost_lin)).abs() };
            }
            let pred_try = last_preds(f)?;
            if cost_try <= self.costs[0] || inner > self.max_inner_iterations {
                break (p_try, cost_try, pred_try);
            }
            inner += 1;
            self.total_inner += 1;
            if inner == self.min_inner_negative_look + 2 && self.inner_scale_fac > 0.0 {
                self.inner_scale_fac *= -1.0;
            }
            scale_step *= self.inner_scale_fac;
        };
        if self.compute_error_estimate {
            let pinv = linalg::mult(&a, &a, true, false)?;
            let (u, mut s, v) = linalg::svd(&pinv)?;
            let np = a.first().map_or(0, Vec::len);
            for i in 0..np {
                s[i][i] = 1.0 / s[i][i];
            }
            let pu = linalg::mult(&p_matrix, &u, false, false)?;
            let pus = linalg::mult(&pu, &s, false, false)?;
            let pest = linalg::mult(&pus, &linalg::mult(&v, &p_matrix, true, true)?, false, false)?;
            self.std = Some(linalg::diag(&pest).iter().map(|x| x.sqrt()).collect());
            for i in 0..np {
                s[i][i] = s[i][i].sqrt();
            }
            let pus = linalg::mult(&pu, &s, false, false)?;
            self.sqrt_cov = Some(linalg::mult(&pus, &v, false, true)?);
        }
        self.preds[ndirs] = pred_try;
        self.costs[ndirs] = cost_try;
        self.pars[ndirs] = p_try;
        let ix = sorted_index(&self.costs);
        self.costs = ix.iter().map(|&i| self.costs[i]).collect();
        self.pars = ix.iter().map(|&i| self.pars[i].clone()).collect();
        self.preds = ix.iter().map(|&i| self.preds[i].clone()).collect();
        let diff = self.costs[1] - self.costs[0];
        let more = self.iterations < self.max_iterations
            && diff > self.abs_tol
            && diff > self.rel_tol * self.costs[0].abs()
            && rel_error_lin > self.rel_tol_lin_cost
            && self.max_total_evaluations.is_none_or(|m| self.total_inner < m);
        self.more_to_do = if inner > 0 && self.iterations < self.max_iterations { true } else { more };
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::openda::cost::{simple_least_squares, SimpleCost};

    /// On `y = p` DUD lands on the observations in one step without the
    /// background and on the posterior mean (½, ½) with it; its error
    /// estimate equals the exact posterior sd; errors for a non-LS cost and
    /// optimize before initialize.
    #[test]
    fn dud_on_identity() {
        let mut f = simple_least_squares(false).unwrap();
        let mut d = Dud { abs_tol: 1e-3, rel_tol: 1e-3, ..Dud::default() };
        d.initialize(&mut f, &[0.0, 0.0]).unwrap();
        assert_eq!(d.current().0.len(), 3);
        d.optimize(&mut f).unwrap();
        let (c, p) = f.history().optimum().unwrap();
        assert!(c < 1e-20 && (p[0] - 1.0).abs() < 1e-12 && (p[1] - 1.0).abs() < 1e-12, "{p:?}");
        let sd = d.std_values().unwrap();
        assert!((sd[0] - 1.0).abs() < 1e-12 && (sd[1] - 2.0).abs() < 1e-12, "{sd:?}");
        assert!(d.sqrt_covariance().is_some() && d.iterations() >= 1);
        let mut g = simple_least_squares(true).unwrap();
        let mut d = Dud::default();
        d.initialize(&mut g, &[0.0, 0.0]).unwrap();
        d.optimize(&mut g).unwrap();
        let (_, p) = g.history().optimum().unwrap();
        assert!((p[0] - 0.5).abs() < 1e-12 && (p[1] - 0.5).abs() < 1e-12, "{p:?}");
        assert!(Dud::default().initialize(&mut SimpleCost::default(), &[0.0, 0.0]).is_err());
        assert!(Dud::default().optimize(&mut SimpleCost::default()).is_err());
    }
}
