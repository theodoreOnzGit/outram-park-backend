// ---------------------------------------------------------------------------
// Ported from OpenDA (open data assimilation toolbox).
//
//   Upstream project: OpenDA, OpenDA Association
//   Upstream repo:    https://github.com/OpenDA-Association/OpenDA
//   Upstream file:    algorithms/java/src/org/openda/algorithms/SimplexCoreOptimizer.java
//                       (initialize, sortSimplexByCost, next, sortedIndex,
//                        the least-squares error estimate)
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
// Translation notes: the cost function is passed to each call rather than
// held (no lifetime). Ported faithfully, NOT fixed: `initialize` sorts only a
// copy of the costs (for its stopping test), so the first iteration runs on
// the simplex in evaluation order; the stopping test at `initialize` uses the
// tolerances in effect at that moment (OpenDA's tests change them after);
// the centroid includes the worst vertex and is corrected in the reflection
// coefficients. Extra stop criteria and restart initialisers are not ported.
// ---------------------------------------------------------------------------

//! **OpenDA's Nelder–Mead simplex** (gh:#1192).

use super::cost::CostFunction;
use super::linalg::{self, axpy, scale};
use crate::{RafflesError, Result};

/// Indices that sort `v` ascending, stably (`Collections.sort` with
/// `Double.compare`).
#[must_use]
pub fn sorted_index(v: &[f64]) -> Vec<usize> {
    let mut ix: Vec<usize> = (0..v.len()).collect();
    ix.sort_by(|a, b| v[*a].total_cmp(&v[*b]));
    ix
}

fn apply<T: Clone>(v: &[T], ix: &[usize]) -> Vec<T> {
    ix.iter().map(|&i| v[i].clone()).collect()
}

/// OpenDA's `SimplexCoreOptimizer`.
#[derive(Debug, Clone, PartialEq)]
pub struct Simplex {
    /// `maxitSimplex` (100).
    pub max_iterations: usize,
    /// `absTolSimplex` (0.01).
    pub abs_tol: f64,
    /// `relTolSimplex` (0.01).
    pub rel_tol: f64,
    vertices: Vec<Vec<f64>>,
    costs: Vec<f64>,
    preds: Vec<Option<Vec<f64>>>,
    n: usize,
    iterations: usize,
    more_to_do: bool,
    std: Option<Vec<f64>>,
}

impl Default for Simplex {
    fn default() -> Self {
        Self { max_iterations: 100, abs_tol: 0.01, rel_tol: 0.01, vertices: Vec::new(), costs: Vec::new(), preds: Vec::new(), n: 0, iterations: 0, more_to_do: true, std: None }
    }
}

fn eval<C: CostFunction>(f: &mut C, p: &[f64]) -> Result<(f64, Option<Vec<f64>>)> {
    let c = f.evaluate(p)?;
    Ok((c, f.least_squares().and_then(|v| v.last_predictions)))
}

impl Simplex {
    /// `initialize(pInit, width)`: `pInit` and `pInit + width·e_i`.
    ///
    /// # Errors
    /// The cost's.
    pub fn initialize<C: CostFunction>(&mut self, f: &mut C, p_init: &[f64], width: f64) -> Result<()> {
        let n = p_init.len();
        self.vertices = vec![p_init.to_vec()];
        let (c, p) = eval(f, p_init)?;
        self.costs = vec![c];
        self.preds = vec![p];
        for i in 0..n {
            let mut q = p_init.to_vec();
            q[i] += width;
            let (c, p) = eval(f, &q)?;
            self.vertices.push(q);
            self.costs.push(c);
            self.preds.push(p);
        }
        self.n = n;
        let sorted = apply(&self.costs, &sorted_index(&self.costs));
        let diff = sorted[n] - sorted[0];
        self.more_to_do = self.iterations < self.max_iterations && diff > self.abs_tol && diff > self.rel_tol * sorted[0].abs();
        Ok(())
    }

    /// The vertices and their costs.
    #[must_use]
    pub fn current(&self) -> (&[Vec<f64>], &[f64]) {
        (&self.vertices, &self.costs)
    }

    /// Outer iterations done.
    #[must_use]
    pub fn iterations(&self) -> usize {
        self.iterations
    }

    /// The least-squares standard deviations OpenDA estimates from the
    /// final simplex (`getStdValues()`), for a least-squares cost.
    #[must_use]
    pub fn std_values(&self) -> Option<&[f64]> {
        self.std.as_deref()
    }

    /// `optimize()`: iterate until a stopping test holds.
    ///
    /// # Errors
    /// The cost's, or a call before [`Simplex::initialize`].
    pub fn optimize<C: CostFunction>(&mut self, f: &mut C) -> Result<()> {
        if self.vertices.is_empty() {
            return Err(RafflesError::InvalidParameter { parameter: "simplex".into(), value: 0.0, reason: "initialize first".into() });
        }
        while self.more_to_do {
            self.next(f)?;
        }
        Ok(())
    }

    #[allow(clippy::too_many_lines)]
    fn next<C: CostFunction>(&mut self, f: &mut C) -> Result<()> {
        let (r, e, c, s) = (-1.0_f64, 2.0_f64, 0.5_f64, 0.5_f64);
        let n = self.n;
        let nf = n as f64;
        let mut sparam = std::mem::take(&mut self.vertices);
        let mut costs = std::mem::take(&mut self.costs);
        let mut preds = std::mem::take(&mut self.preds);
        let mut flag = false;
        self.iterations += 1;
        let mut centroid = sparam[0].clone();
        for v in &sparam[1..] {
            axpy(&mut centroid, 1.0, v);
        }
        scale(&mut centroid, 1.0 / nf);
        let reflect = vertex_point(&centroid, &sparam[n], r, nf);
        let (cost_r, pred_r) = eval(f, &reflect)?;
        if cost_r >= costs[0] && cost_r < costs[n - 1] {
            flag = true;
            sparam[n] = reflect.clone();
            costs[n] = cost_r;
            preds[n] = pred_r.clone();
        }
        if !flag && cost_r < costs[0] {
            let expand = vertex_point(&centroid, &sparam[n], r * e, nf);
            let (cost_e, pred_e) = eval(f, &expand)?;
            if cost_e < cost_r {
                flag = true;
                sparam[n] = expand;
                costs[n] = cost_e;
                preds[n] = pred_e;
            }
            // A separate test, as OpenDA: a NaN cost takes neither branch.
            if cost_e >= cost_r {
                flag = true;
                sparam[n] = reflect.clone();
                costs[n] = cost_r;
                preds[n] = pred_r.clone();
            }
        }
        if !flag && cost_r >= costs[n - 1] && cost_r < costs[n] {
            let contract = vertex_point(&centroid, &sparam[n], r * c, nf);
            let (cost_c, pred_c) = eval(f, &contract)?;
            if cost_c <= cost_r {
                flag = true;
                sparam[n] = contract;
                costs[n] = cost_c;
                preds[n] = pred_c;
            }
        }
        if !flag && cost_r >= costs[n] {
            let contract = vertex_point(&centroid, &sparam[n], c, nf);
            let (cost_c, pred_c) = eval(f, &contract)?;
            if cost_c < costs[n] {
                flag = true;
                sparam[n] = contract;
                costs[n] = cost_c;
                preds[n] = pred_c;
            }
        }
        if !flag {
            for iv in 1..=n {
                let mut shrink = sparam[0].clone();
                scale(&mut shrink, 1.0 - s);
                axpy(&mut shrink, s, &sparam[iv]);
                let (cost_s, pred_s) = eval(f, &shrink)?;
                sparam[iv] = shrink;
                costs[iv] = cost_s;
                preds[iv] = pred_s;
            }
        }
        let ix = sorted_index(&costs);
        let (costs, sparam, preds) = (apply(&costs, &ix), apply(&sparam, &ix), apply(&preds, &ix));
        let diff = costs[n] - costs[0];
        self.more_to_do = self.iterations < self.max_iterations && diff > self.abs_tol && diff > self.rel_tol * costs[0].abs();
        if !self.more_to_do {
            if let Some(ls) = f.least_squares() {
                self.std = Some(error_estimate(&sparam, &preds, &ls)?);
            }
        }
        self.vertices = sparam;
        self.costs = costs;
        self.preds = preds;
        Ok(())
    }
}

/// `centroid·(1 − a) − ((1 − a)/n − a)·worst`, OpenDA's reflection,
/// expansion and contraction points (the centroid includes the worst).
fn vertex_point(centroid: &[f64], worst: &[f64], a: f64, nf: f64) -> Vec<f64> {
    let mut x = centroid.to_vec();
    scale(&mut x, 1.0 - a);
    axpy(&mut x, -((1.0 - a) / nf - a), worst);
    x
}

/// The error estimate at the end of `next()`: the least-squares system of
/// the simplex's differences relative to its best vertex.
fn error_estimate(sparam: &[Vec<f64>], preds: &[Option<Vec<f64>>], ls: &super::cost::LeastSquaresView) -> Result<Vec<f64>> {
    let n = sparam.len() - 1;
    let missing = || RafflesError::InvalidParameter { parameter: "simplex predictions".into(), value: 0.0, reason: "a vertex has none".into() };
    let p0 = preds[0].as_ref().ok_or_else(missing)?;
    let mut preds_norm = Vec::with_capacity(n);
    let mut pars_rel = Vec::with_capacity(n);
    for i in 0..n {
        let mut pn = preds[i + 1].as_ref().ok_or_else(missing)?.clone();
        axpy(&mut pn, -1.0, p0);
        linalg::pointwise_divide(&mut pn, &ls.observations.std);
        preds_norm.push(pn);
        let mut pr = sparam[i + 1].clone();
        axpy(&mut pr, -1.0, &sparam[0]);
        pars_rel.push(pr);
    }
    let a_obs = linalg::from_columns(&preds_norm);
    let a = if ls.background {
        let pars_norm = pars_rel.iter().map(|p| ls.parameters.sqrt_solve(p)).collect::<Result<Vec<_>>>()?;
        linalg::concat_vertical(&a_obs, &linalg::from_columns(&pars_norm))
    } else {
        a_obs
    };
    let np = a.first().map_or(0, Vec::len);
    let mut pinv = linalg::zeros(np, np);
    linalg::multiply(&mut pinv, 1.0, &a, &a, 0.0, true, false)?;
    let p = linalg::inverse(&pinv)?;
    let xi = linalg::from_columns(&pars_rel);
    let mut pest = linalg::zeros(np, np);
    linalg::multiply(&mut pest, 1.0, &p, &xi, 0.0, false, true)?;
    let pest = linalg::mult(&xi, &pest, false, false)?;
    Ok(linalg::diag(&pest).iter().map(|v| v.sqrt()).collect())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::openda::cost::{simple_least_squares, RosenbrockCost, SimpleCost};

    /// It descends on the quadratic and on Rosenbrock; the least-squares
    /// error estimate equals the exact posterior sd for a linear model;
    /// optimize before initialize is an error.
    #[test]
    fn simplex_minimises() {
        let mut f = SimpleCost::default();
        let mut s = Simplex::default();
        s.initialize(&mut f, &[0.0, 0.0], 1.5).unwrap();
        assert_eq!(s.current().0[1], vec![1.5, 0.0]);
        s.abs_tol = 1e-12;
        s.rel_tol = 1e-12;
        s.max_iterations = 500;
        s.optimize(&mut f).unwrap();
        let (c, p) = f.history().optimum().unwrap();
        assert!(c < 1e-9 && (p[0] - 1.0).abs() < 1e-4 && s.iterations() > 10, "{p:?}");
        let mut r = RosenbrockCost::default();
        let mut s = Simplex::default();
        s.initialize(&mut r, &[0.0, 0.0], 1.5).unwrap();
        s.optimize(&mut r).unwrap();
        assert!(r.history().optimum().unwrap().0 < 0.5);
        let mut l = simple_least_squares(false).unwrap();
        // OpenDA's default tolerances: with tight ones the simplex shrinks until
        // AᵀA falls below inverse()'s absolute 1e-7 cut-off and the estimate
        // collapses (upstream behaviour).
        let mut s = Simplex::default();
        s.initialize(&mut l, &[0.0, 0.0], 1.0).unwrap();
        s.optimize(&mut l).unwrap();
        // y = p: P = Xi (Xiᵀ diag(1/σ²) Xi)⁻¹ Xiᵀ = diag(σ²), sd (1, 2).
        let sd = s.std_values().unwrap();
        assert!((sd[0] - 1.0).abs() < 1e-6 && (sd[1] - 2.0).abs() < 1e-6, "{sd:?}");
        assert!(Simplex::default().optimize(&mut SimpleCost::default()).is_err());
        assert_eq!(sorted_index(&[3.0, 1.0, 2.0, 1.0]), vec![1, 3, 2, 0]);
    }
}
