// ---------------------------------------------------------------------------
// Ported from RAVEN (Risk Analysis Virtual ENvironment).
//
//   Upstream project: RAVEN — Idaho National Laboratory
//   Upstream repo:    https://github.com/idaholab/raven
//   Upstream files:   ravenframework/SupervisedLearning/GaussPolynomialRom.py
//                     ravenframework/Samplers/SparseGridCollocation.py (the
//                     wiring: default quadrature/polynomial per variable)
//   Upstream commit:  8baaae3a97f1392318074ad6d3f8efc26ca81c7e  (branch devel)
//   Accessed:         2026-11-04
//
//   Copyright 2017 Battelle Energy Alliance, LLC
//   Licensed under the Apache License, Version 2.0 (the "License");
//   you may not use this file except in compliance with the License.
//   You may obtain a copy of the License at
//
//       http://www.apache.org/licenses/LICENSE-2.0
//
//   Unless required by applicable law or agreed to in writing, software
//   distributed under the License is distributed on an "AS IS" BASIS,
//   WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
//   See the License for the specific language governing permissions and
//   limitations under the License.
//
// This Rust translation is part of RAFFLES / Outram Park and is distributed
// under AGPL-3.0-only. Apache-2.0 -> (A)GPLv3 is a ONE-WAY relicensing: this
// file may NOT be contributed back to RAVEN or redistributed under Apache-2.0.
//
// Translation notes:
//   * The ROM (`GaussPolynomialRom`) and its sampler
//     (`SparseGridCollocation`) were two entities tied through RAVEN's
//     assembler; here one `PolynomialChaosSpec` builds both the index set
//     and the grid, and `PolynomialChaos::fit` trains on rows that contain
//     the grid points.
//   * Kept exactly: `_train`'s matching of each grid point to a training row
//     within 1e-9 (RAVEN uses a `scipy.spatial.KDTree` query with
//     `distance_upper_bound=1e-9`; here a linear scan, same rule), the
//     training ROW (not the grid point) being converted to the standard
//     domain, the coefficient sum `Σ y·Π φ·w` in grid order times
//     `Π measureNorm`, `__mean__` (the zeroth coefficient), `__variance__`
//     (`Σ c² − mean²` in index-set order), `__evaluateLocal__`,
//     `getSensitivities` and `getTotalSensitivities`.
//   * Not ported: HDMRRom (deferred, gh:#1193 final report), the KDTree
//     itself, XML/pickle/printing, `__evaluateMoment__` for r > 2 (it
//     integrates the ROM over the grid; not needed for mean/variance).
// ---------------------------------------------------------------------------

//! **Polynomial chaos: RAVEN's `GaussPolynomialRom`** (gh:#1193), with its
//! index sets, Smolyak and tensor quadrature grids and orthonormal
//! Legendre/Hermite polynomials ([`grid`]), and the SciPy Gauss rules and
//! special functions it delegates to ([`special`]).
//!
//! The expansion is a **projection**, not a regression: each coefficient is
//! the quadrature of `y · Φ_k` on RAVEN's grid, so the training data must be
//! the model at the grid's points ([`PolynomialChaosSpec::grid`]). Mean,
//! variance and Sobol indices follow from the coefficients alone.
//!
//! # How this relates to [`super::PolynomialSurrogate`]
//!
//! The existing polynomial route is a least-squares fit in monomials on any
//! design; this one is RAVEN's spectral projection in orthonormal
//! polynomials on a quadrature design. Neither replaces the other; both are
//! exact for a polynomial inside their basis (the V&V record's A4).
//!
//! # V&V
//!
//! Record: `crates/raffles/verification_and_validation/raven_routes_gh1193.md`;
//! differential tests in `tests/raven_routes_gh1193.rs`.

pub mod grid;
pub mod special;

pub use grid::{index_set, quadrature_grid, IndexSetKind, PceVariable, QuadratureGrid, SparseGridKind};

use crate::{RafflesError, Result};

/// RAVEN's tolerance for matching a training row to a grid point
/// (`GaussPolynomialRom._train`, `distance_upper_bound=1e-9`).
pub const GRID_MATCH_TOLERANCE: f64 = 1e-9;

/// What to build: the inputs, the index set, the order and the grid.
#[derive(Debug, Clone, PartialEq)]
pub struct PolynomialChaosSpec {
    /// One entry per input, in input order.
    pub variables: Vec<PceVariable>,
    /// `<IndexSet>`.
    pub index_set: IndexSetKind,
    /// `<PolynomialOrder>` (at least 1, RAVEN's own check).
    pub order: usize,
    /// `<SparseGrid>` (RAVEN's default: Smolyak).
    pub grid: SparseGridKind,
    /// Per-input importance weights (RAVEN's `<Interpolation weight=...>`,
    /// default 1); `None` for all ones.
    pub importance: Option<Vec<f64>>,
}

impl PolynomialChaosSpec {
    /// A Smolyak, total-degree, isotropic spec (RAVEN's defaults apart from
    /// the index set, which RAVEN requires the user to name).
    #[must_use]
    pub fn total_degree(variables: Vec<PceVariable>, order: usize) -> Self {
        Self {
            variables,
            index_set: IndexSetKind::TotalDegree,
            order,
            grid: SparseGridKind::Smolyak,
            importance: None,
        }
    }

    fn validate(&self) -> Result<Vec<f64>> {
        if self.variables.is_empty() {
            return Err(RafflesError::InvalidParameter {
                parameter: "variables".to_string(),
                value: 0.0,
                reason: "at least one input".to_string(),
            });
        }
        if self.order < 1 {
            return Err(RafflesError::InvalidParameter {
                parameter: "order".to_string(),
                value: 0.0,
                reason: "Polynomial order cannot be less than 1 (RAVEN's own check)".to_string(),
            });
        }
        for v in &self.variables {
            v.validate()?;
        }
        let w = self
            .importance
            .clone()
            .unwrap_or_else(|| vec![1.0; self.variables.len()]);
        if w.len() != self.variables.len() {
            return Err(RafflesError::DimensionMismatch {
                expected: self.variables.len(),
                found: w.len(),
            });
        }
        if w.iter().any(|v| !(v.is_finite() && *v > 0.0)) {
            return Err(RafflesError::InvalidParameter {
                parameter: "importance".to_string(),
                value: f64::NAN,
                reason: "importance weights must be positive".to_string(),
            });
        }
        Ok(w)
    }

    /// The index set: the multi-indices of the polynomial terms, in RAVEN's
    /// order.
    ///
    /// # Errors
    ///
    /// An invalid spec.
    pub fn index_points(&self) -> Result<Vec<Vec<usize>>> {
        let w = self.validate()?;
        Ok(index_set(self.index_set, self.order, &w))
    }

    /// The quadrature grid the model must be run on, in RAVEN's order.
    ///
    /// # Errors
    ///
    /// An invalid spec, or a rule of more than 150 points.
    pub fn grid(&self) -> Result<QuadratureGrid> {
        let set = self.index_points()?;
        quadrature_grid(&self.variables, self.index_set, &set, self.grid)
    }
}

/// A trained expansion, one coefficient vector per output.
#[derive(Debug, Clone, PartialEq)]
pub struct PolynomialChaos {
    spec: PolynomialChaosSpec,
    index_set: Vec<Vec<usize>>,
    coefficients: Vec<Vec<f64>>,
}

impl PolynomialChaos {
    /// Trains on rows `x` / outputs `y` that contain every grid point
    /// (`GaussPolynomialRom._train`). Extra rows are ignored; a grid point
    /// with no row within [`GRID_MATCH_TOLERANCE`] is an error, as in RAVEN.
    ///
    /// # Errors
    ///
    /// An invalid spec, mismatched lengths, a missing grid point, or a
    /// non-finite output at a grid point.
    pub fn fit(spec: &PolynomialChaosSpec, x: &[Vec<f64>], y: &[Vec<f64>]) -> Result<Self> {
        if x.len() != y.len() {
            return Err(RafflesError::DimensionMismatch {
                expected: x.len(),
                found: y.len(),
            });
        }
        let set = spec.index_points()?;
        let grid = quadrature_grid(&spec.variables, spec.index_set, &set, spec.grid)?;
        let d = spec.variables.len();
        let n_out = y.first().map_or(0, Vec::len);
        if x.iter().any(|r| r.len() != d) || y.iter().any(|r| r.len() != n_out) || n_out == 0 {
            return Err(RafflesError::DimensionMismatch {
                expected: d,
                found: x.first().map_or(0, Vec::len),
            });
        }
        // Match each grid point to its nearest row within the tolerance.
        let mut rows = Vec::with_capacity(grid.points.len());
        for pt in &grid.points {
            let mut best: Option<(f64, usize)> = None;
            for (k, r) in x.iter().enumerate() {
                let dist = r
                    .iter()
                    .zip(pt)
                    .map(|(a, b)| (a - b) * (a - b))
                    .sum::<f64>()
                    .sqrt();
                if dist <= GRID_MATCH_TOLERANCE && best.is_none_or(|(bd, _)| dist < bd) {
                    best = Some((dist, k));
                }
            }
            let (_, k) = best.ok_or(RafflesError::InvalidParameter {
                parameter: "x".to_string(),
                value: pt[0],
                reason: "input values do not match required values (a grid point has no \
                         training row within 1e-9)"
                    .to_string(),
            })?;
            if y[k].iter().any(|v| !v.is_finite()) {
                return Err(RafflesError::InvalidParameter {
                    parameter: "y".to_string(),
                    value: f64::NAN,
                    reason: "non-finite output at a grid point".to_string(),
                });
            }
            rows.push(k);
        }
        let std_pts: Vec<Vec<f64>> = rows
            .iter()
            .map(|&k| {
                x[k].iter()
                    .zip(&spec.variables)
                    .map(|(v, var)| var.to_quad(*v))
                    .collect()
            })
            .collect();
        let norm = spec
            .variables
            .iter()
            .fold(1.0, |acc, v| acc * v.measure_norm());
        let mut coefficients = vec![vec![0.0; set.len()]; n_out];
        for (t, coef) in coefficients.iter_mut().enumerate() {
            for (c, idx) in coef.iter_mut().zip(&set) {
                let mut acc = 0.0;
                for (g, &k) in rows.iter().enumerate() {
                    acc += y[k][t] * basis(&spec.variables, idx, &std_pts[g]) * grid.weights[g];
                }
                *c = acc * norm;
            }
        }
        Ok(Self {
            spec: spec.clone(),
            index_set: set,
            coefficients,
        })
    }

    /// The spec it was built from.
    #[must_use]
    pub fn spec(&self) -> &PolynomialChaosSpec {
        &self.spec
    }

    /// The multi-indices, aligned with each output's coefficients.
    #[must_use]
    pub fn index_set(&self) -> &[Vec<usize>] {
        &self.index_set
    }

    /// The coefficients of output `k`, aligned with [`index_set`](Self::index_set).
    #[must_use]
    pub fn coefficients(&self, k: usize) -> &[f64] {
        &self.coefficients[k]
    }

    /// Number of outputs.
    #[must_use]
    pub fn outputs(&self) -> usize {
        self.coefficients.len()
    }

    /// `__evaluateLocal__`: every output at `x`.
    ///
    /// # Errors
    ///
    /// [`RafflesError::DimensionMismatch`] for a wrong-length `x`.
    pub fn predict(&self, x: &[f64]) -> Result<Vec<f64>> {
        if x.len() != self.spec.variables.len() {
            return Err(RafflesError::DimensionMismatch {
                expected: self.spec.variables.len(),
                found: x.len(),
            });
        }
        let std: Vec<f64> = x
            .iter()
            .zip(&self.spec.variables)
            .map(|(v, var)| var.to_quad(*v))
            .collect();
        Ok(self
            .coefficients
            .iter()
            .map(|c| {
                let mut tot = 0.0;
                for (ci, idx) in c.iter().zip(&self.index_set) {
                    tot += ci * basis(&self.spec.variables, idx, &std);
                }
                tot
            })
            .collect())
    }

    /// `__mean__`: the coefficient of the constant term.
    #[must_use]
    pub fn mean(&self, k: usize) -> f64 {
        self.index_set
            .iter()
            .position(|i| i.iter().all(|v| *v == 0))
            .map_or(0.0, |p| self.coefficients[k][p])
    }

    /// `__variance__`: `Σ c² − mean²`.
    #[must_use]
    pub fn variance(&self, k: usize) -> f64 {
        let m = self.mean(k);
        let s2 = self.coefficients[k].iter().fold(0.0, |a, c| a + c * c);
        s2 - m * m
    }

    /// `getSensitivities`: Sobol indices and partial variances per subset of
    /// inputs (each subset a sorted list of input positions), in order of
    /// first appearance in the index set.
    #[must_use]
    pub fn sobol(&self, k: usize) -> Vec<(Vec<usize>, f64, f64)> {
        let tot = self.variance(k);
        let mut parts: Vec<(Vec<usize>, f64)> = Vec::new();
        for (c, idx) in self.coefficients[k].iter().zip(&self.index_set) {
            let subset: Vec<usize> = idx
                .iter()
                .enumerate()
                .filter(|(_, p)| **p != 0)
                .map(|(i, _)| i)
                .collect();
            if subset.is_empty() {
                continue;
            }
            match parts.iter_mut().find(|(s, _)| *s == subset) {
                Some((_, v)) => *v += c * c,
                None => parts.push((subset, c * c)),
            }
        }
        parts
            .into_iter()
            .map(|(s, p)| (s, p / tot, p))
            .collect()
    }

    /// `getTotalSensitivities`: for each subset, the sum of the Sobol
    /// indices of every subset containing it.
    #[must_use]
    pub fn total_sobol(&self, k: usize) -> Vec<(Vec<usize>, f64)> {
        let s = self.sobol(k);
        s.iter()
            .map(|(sub, _, _)| {
                let mut t = 0.0;
                for (chk, idx, _) in &s {
                    if sub.iter().all(|v| chk.contains(v)) {
                        t += idx;
                    }
                }
                (sub.clone(), t)
            })
            .collect()
    }
}

/// `_multiDPolyBasisEval`: the product of the 1-D orthonormal polynomials.
fn basis(vars: &[PceVariable], orders: &[usize], pt: &[f64]) -> f64 {
    let mut tot = 1.0;
    for ((v, o), p) in vars.iter().zip(orders).zip(pt) {
        tot *= v.poly(*o, *p);
    }
    tot
}

#[cfg(test)]
mod tests {
    use super::*;

    fn uniform(lower: f64, upper: f64) -> PceVariable {
        PceVariable::Uniform { lower, upper }
    }

    /// **A4, polynomial reproduction.** Sudret's polynomial
    /// `Π (3 y_n² + 1) / 2³` on U(0, 1)³ at tensor-product order 2 (Smolyak):
    /// exact indices S_i = 25/91, S_ij = 5/91, S_123 = 1/91, mean 1,
    /// variance 0.728; and a cubic in a uniform and a normal input at total
    /// degree 3, reproduced at off-grid points.
    ///
    /// Prediction (V&V record A4, before the run): Sudret within 1e-13
    /// absolute; the cubic within 1e-12 relative.
    ///
    /// Result: see the V&V record.
    #[test]
    fn polynomial_chaos_reproduces_polynomials_exactly() {
        let spec = PolynomialChaosSpec {
            variables: vec![uniform(0.0, 1.0); 3],
            index_set: IndexSetKind::TensorProduct,
            order: 2,
            grid: SparseGridKind::Smolyak,
            importance: None,
        };
        let g = spec.grid().unwrap();
        let y: Vec<Vec<f64>> = g
            .points
            .iter()
            .map(|p| vec![p.iter().map(|v| 3.0 * v * v + 1.0).product::<f64>() / 8.0])
            .collect();
        let pce = PolynomialChaos::fit(&spec, &g.points, &y).unwrap();
        let mut worst: f64 = (pce.mean(0) - 1.0).abs().max((pce.variance(0) - 0.728).abs());
        for (s, idx, _) in pce.sobol(0) {
            let exact = match s.len() {
                1 => 25.0 / 91.0,
                2 => 5.0 / 91.0,
                _ => 1.0 / 91.0,
            };
            worst = worst.max((idx - exact).abs());
        }
        eprintln!("A4 Sudret worst abs error {worst:.3e}");
        assert!(worst < 1e-13);
        let totals = pce.total_sobol(0);
        let t1 = totals.iter().find(|(s, _)| *s == vec![0]).unwrap().1;
        assert!((t1 - (25.0 + 10.0 + 1.0) / 91.0).abs() < 1e-13);

        let cubic = |x: &[f64]| {
            1.0 + 2.0 * x[0] - 0.5 * x[1] + 0.75 * x[0] * x[1] - 0.3 * x[0].powi(3)
                + 0.2 * x[1] * x[1] * x[0]
        };
        let spec = PolynomialChaosSpec::total_degree(
            vec![
                uniform(-1.0, 3.0),
                PceVariable::Normal {
                    mean: 2.0,
                    std: 0.5,
                },
            ],
            3,
        );
        let g = spec.grid().unwrap();
        let y: Vec<Vec<f64>> = g.points.iter().map(|p| vec![cubic(p), -cubic(p)]).collect();
        let pce = PolynomialChaos::fit(&spec, &g.points, &y).unwrap();
        assert_eq!(pce.outputs(), 2);
        let mut worst: f64 = 0.0;
        for p in [[0.3, 1.7], [-0.9, 2.6], [2.5, 1.1], [0.0, 0.0]] {
            let pred = pce.predict(&p).unwrap();
            worst = worst.max(((pred[0] - cubic(&p)) / cubic(&p)).abs());
            assert_eq!(pred[1], -pred[0]);
        }
        eprintln!("A4 cubic worst rel error {worst:.3e}");
        assert!(worst < 1e-12);
        assert!(pce.predict(&[0.0]).is_err());
        assert_eq!(pce.index_set().len(), pce.coefficients(0).len());
        assert_eq!(pce.spec(), &spec);
    }

    /// Errors: a missing grid point (RAVEN's "input values do not match
    /// required values"), mismatched lengths, a non-finite output, an
    /// invalid spec.
    #[test]
    fn polynomial_chaos_refuses_bad_input() {
        let spec = PolynomialChaosSpec::total_degree(vec![uniform(0.0, 1.0)], 2);
        let g = spec.grid().unwrap();
        let y: Vec<Vec<f64>> = g.points.iter().map(|p| vec![p[0]]).collect();
        assert!(PolynomialChaos::fit(&spec, &g.points[1..], &y[1..]).is_err());
        assert!(PolynomialChaos::fit(&spec, &g.points, &y[1..]).is_err());
        let mut bad = y.clone();
        bad[0][0] = f64::NAN;
        assert!(PolynomialChaos::fit(&spec, &g.points, &bad).is_err());
        let mut s = spec.clone();
        s.order = 0;
        assert!(s.grid().is_err());
        s.order = 2;
        s.importance = Some(vec![1.0, 1.0]);
        assert!(s.grid().is_err());
        s.importance = Some(vec![-1.0]);
        assert!(s.grid().is_err());
        s.variables.clear();
        assert!(s.grid().is_err());
        // Rows slightly off the grid (within 1e-9) still match, as in RAVEN.
        let shifted: Vec<Vec<f64>> = g.points.iter().map(|p| vec![p[0] + 1e-12]).collect();
        assert!(PolynomialChaos::fit(&spec, &shifted, &y).is_ok());
    }
}
