// ---------------------------------------------------------------------------
// Ported from RAVEN (Risk Analysis Virtual ENvironment).
//
//   Upstream project: RAVEN — Idaho National Laboratory
//   Upstream repo:    https://github.com/idaholab/raven
//   Upstream files:   ravenframework/IndexSets.py
//                     ravenframework/Quadratures.py
//                     ravenframework/OrthoPolynomials.py
//                     ravenframework/Distributions.py (Uniform / Normal point
//                     conversions and measure norms only)
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
//   * The `IndexSet`, `SparseGrid`, `QuadratureSet` and `OrthogonalPolynomial`
//     class families become the enums `IndexSetKind`, `SparseGridKind` and
//     `PceVariable` (which fixes the quadrature and polynomial pairing the
//     way `SparseGridCollocation._generateQuadsAndPolys` defaults it:
//     Uniform -> Legendre/Legendre, untruncated Normal -> Hermite/Hermite).
//   * Kept exactly: `generateMultiIndex`'s recursion and its rules evaluated
//     on partial indices, the importance-weight normalisation, the Smolyak
//     combination coefficients (`smarterMakeCoeffs`), `GaussQuadRule`
//     (order + 1 points), `np.around(pts, 15)` on every rule, the tensor
//     product order (last variable fastest), the per-point weight product
//     and the merge of coincident points in insertion order.
//   * Not ported: Clenshaw–Curtis, the CDF quadratures, Laguerre and Jacobi,
//     the `Custom` and `Adaptive` index sets, the parallel (job-handler)
//     grid construction, and the pickling/XML methods.
// ---------------------------------------------------------------------------

//! RAVEN's index sets, quadrature sparse grids and orthonormal polynomials
//! for polynomial chaos.

use petir::mathf::RealMath;
use std::collections::HashMap;

use super::special::{eval_hermitenorm, eval_legendre, gauss_rule, numpy_around, GaussRule};
use crate::{RafflesError, Result};

/// One input of a polynomial chaos expansion, with RAVEN's default
/// quadrature and polynomial family for it.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum PceVariable {
    /// `Uniform(lower, upper)`: Gauss–Legendre points, Legendre polynomials.
    Uniform {
        /// Lower bound.
        lower: f64,
        /// Upper bound.
        upper: f64,
    },
    /// Untruncated `Normal(mean, std)`: Gauss–Hermite (probabilists') points,
    /// Hermite polynomials.
    Normal {
        /// Mean.
        mean: f64,
        /// Standard deviation.
        std: f64,
    },
}

impl PceVariable {
    pub(crate) fn validate(&self) -> Result<()> {
        let bad = |p: &str, v: f64, r: &str| {
            Err(RafflesError::InvalidParameter {
                parameter: p.to_string(),
                value: v,
                reason: r.to_string(),
            })
        };
        match *self {
            Self::Uniform { lower, upper } => {
                if !(lower.is_finite() && upper.is_finite() && upper > lower) {
                    return bad("upper", upper, "a uniform needs finite lower < upper");
                }
            }
            Self::Normal { mean, std } => {
                if !(mean.is_finite() && std.is_finite() && std > 0.0) {
                    return bad("std", std, "a normal needs a finite mean and std > 0");
                }
            }
        }
        Ok(())
    }

    fn rule(&self) -> GaussRule {
        match self {
            Self::Uniform { .. } => GaussRule::Legendre,
            Self::Normal { .. } => GaussRule::HermiteNorm,
        }
    }

    /// `untruncatedMean`, as scipy computes it.
    fn mean(&self) -> f64 {
        match *self {
            Self::Uniform { lower, upper } => lower + (upper - lower) * 0.5,
            Self::Normal { mean, .. } => mean,
        }
    }

    /// `convertToDistr`: standard quadrature point to the variable.
    pub(crate) fn to_distr(&self, x: f64) -> f64 {
        match *self {
            Self::Uniform { lower, upper } => (upper - lower) / 2.0 * x + self.mean(),
            Self::Normal { std, .. } => std * x + self.mean(),
        }
    }

    /// `convertToQuad`: the variable to the standard quadrature point.
    pub(crate) fn to_quad(&self, y: f64) -> f64 {
        match *self {
            Self::Uniform { lower, upper } => (y - self.mean()) / ((upper - lower) / 2.0),
            Self::Normal { std, .. } => (y - self.mean()) / std,
        }
    }

    /// `measureNorm`: 1/2 for Legendre on a uniform, 1/√(2π) for Hermite.
    pub(crate) fn measure_norm(&self) -> f64 {
        match self {
            Self::Uniform { .. } => 0.5,
            Self::Normal { .. } => 1.0 / (2.0 * core::f64::consts::PI).sqrt(),
        }
    }

    /// The orthonormal polynomial of `order` at standard point `x`
    /// (`OrthogonalPolynomial.__call__`: the scipy value times `norm`).
    pub(crate) fn poly(&self, order: usize, x: f64) -> f64 {
        match self {
            Self::Uniform { .. } => eval_legendre(order, x) * (2.0 * order as f64 + 1.0).sqrt(),
            Self::Normal { .. } => eval_hermitenorm(order, x) * (1.0 / factorial(order).sqrt()),
        }
    }
}

/// `math.gamma(1 + n)` for integer `n`: exact up to 22!.
fn factorial(n: usize) -> f64 {
    (1..=n).fold(1.0, |acc, k| acc * k as f64)
}

/// Which index set (`<IndexSet>`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum IndexSetKind {
    /// `max_j(p_j w_j) <= L · mean(w)`.
    TensorProduct,
    /// `sum_j p_j w_j <= L · mean(w)`.
    TotalDegree,
    /// `prod_j (p_j + 1)^(w_j) <= (L + 1)^(mean(w))`.
    HyperbolicCross,
}

/// Which grid (`<SparseGrid>`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SparseGridKind {
    /// Smolyak's combination of tensor rules (RAVEN's default).
    Smolyak,
    /// The full tensor grid at each dimension's largest order.
    Tensor,
}

/// `IndexSet.initialize` + the subclass rule + `generateMultiIndex`.
/// `importance` is RAVEN's per-variable `weight` (default 1).
#[must_use]
pub fn index_set(kind: IndexSetKind, order: usize, importance: &[f64]) -> Vec<Vec<usize>> {
    let max = importance.iter().copied().fold(f64::NEG_INFINITY, f64::max);
    let w: Vec<f64> = importance.iter().map(|v| 1.0 / (v / max)).collect();
    let n = w.len();
    let wsum = w.iter().fold(0.0, |a, b| a + b);
    let l = order as f64;
    let rule = |i: &[usize]| -> bool {
        match kind {
            IndexSetKind::TensorProduct => {
                let target = wsum / n as f64 * l;
                let mut big = 0.0_f64;
                for (j, p) in i.iter().enumerate() {
                    let v = *p as f64 * w[j];
                    if v > big {
                        big = v;
                    }
                }
                big <= target
            }
            IndexSetKind::TotalDegree => {
                let target = wsum / n as f64 * l;
                let mut tot = 0.0;
                for (j, p) in i.iter().enumerate() {
                    tot += *p as f64 * w[j];
                }
                tot <= target
            }
            IndexSetKind::HyperbolicCross => {
                let target = (l + 1.0).r_powf(wsum / (n as f64).max(1.0));
                let mut tot = 1.0;
                for (e, v) in i.iter().enumerate() {
                    tot *= (*v as f64 + 1.0).r_powf(w[e]);
                }
                tot <= target
            }
        }
    };
    let mut out = Vec::new();
    let mut prefix = Vec::new();
    generate(n, &rule, &mut prefix, &mut out);
    out
}

fn generate<R: Fn(&[usize]) -> bool>(
    n: usize,
    rule: &R,
    prefix: &mut Vec<usize>,
    out: &mut Vec<Vec<usize>>,
) {
    if prefix.len() == n {
        out.push(prefix.clone());
        return;
    }
    let mut i = 0;
    loop {
        prefix.push(i);
        if !rule(prefix) {
            prefix.pop();
            break;
        }
        generate(n, rule, prefix, out);
        prefix.pop();
        i += 1;
    }
}

/// A quadrature grid: points in the variables' own space, and weights, in
/// RAVEN's insertion order.
#[derive(Debug, Clone, PartialEq)]
pub struct QuadratureGrid {
    /// Points, one row per point.
    pub points: Vec<Vec<f64>>,
    /// Weights (on the quadrature measures, before `measureNorm`).
    pub weights: Vec<f64>,
}

/// `SparseGrid.tensorGrid(m)`: the tensor product of `m[n]`-point rules.
fn tensor_grid(vars: &[PceVariable], m: &[usize]) -> Result<(Vec<Vec<f64>>, Vec<f64>)> {
    let mut lists = Vec::with_capacity(vars.len());
    for (v, &mn) in vars.iter().zip(m) {
        let (x, w) = gauss_rule(v.rule(), mn)?;
        let pts: Vec<f64> = x.iter().map(|xi| v.to_distr(numpy_around(*xi, 15))).collect();
        lists.push((pts, w));
    }
    let mut points = vec![Vec::new()];
    let mut weights = vec![Vec::new()];
    for (pts, ws) in &lists {
        let mut np = Vec::with_capacity(points.len() * pts.len());
        let mut nw = Vec::with_capacity(points.len() * pts.len());
        for (p, w) in points.iter().zip(&weights) {
            for (x, wx) in pts.iter().zip(ws) {
                let mut q = p.clone();
                q.push(*x);
                np.push(q);
                let mut r: Vec<f64> = w.clone();
                r.push(*wx);
                nw.push(r);
            }
        }
        points = np;
        weights = nw;
    }
    // np.product over each weight tuple: sequential.
    let weights = weights
        .iter()
        .map(|t| t.iter().fold(1.0, |a, b| a * b))
        .collect();
    Ok((points, weights))
}

/// Points key for the merge: `-0.0` and `0.0` are one Python dict key.
fn key(p: &[f64]) -> Vec<u64> {
    p.iter()
        .map(|v| if *v == 0.0 { 0 } else { v.to_bits() })
        .collect()
}

/// `SmolyakSparseGrid.initialize` / `TensorGrid.initialize`.
///
/// # Errors
///
/// A Gauss rule out of range (more than 150 points in a dimension).
pub fn quadrature_grid(
    vars: &[PceVariable],
    kind: IndexSetKind,
    set: &[Vec<usize>],
    grid: SparseGridKind,
) -> Result<QuadratureGrid> {
    let mut points: Vec<Vec<f64>> = Vec::new();
    let mut weights: Vec<f64> = Vec::new();
    let mut index: HashMap<Vec<u64>, usize> = HashMap::new();
    match grid {
        SparseGridKind::Tensor => {
            let mut largest = vec![0usize; vars.len()];
            for idx in set {
                for (l, i) in largest.iter_mut().zip(idx) {
                    *l = (*l).max(*i);
                }
            }
            let m: Vec<usize> = largest.iter().map(|v| v + 1).collect();
            let (p, w) = tensor_grid(vars, &m)?;
            for (pt, wt) in p.into_iter().zip(w) {
                match index.get(&key(&pt)) {
                    Some(&k) => weights[k] = wt,
                    None => {
                        index.insert(key(&pt), points.len());
                        points.push(pt);
                        weights.push(wt);
                    }
                }
            }
        }
        SparseGridKind::Smolyak => {
            let (c, idxs): (Vec<f64>, Vec<Vec<usize>>) = if kind == IndexSetKind::TensorProduct {
                (vec![1.0], set.last().into_iter().cloned().collect())
            } else {
                let n = set.len();
                let mut c = vec![1.0; n];
                for i in 0..n {
                    for j in i + 1..n {
                        let d: Vec<i64> = set[j]
                            .iter()
                            .zip(&set[i])
                            .map(|(a, b)| *a as i64 - *b as i64)
                            .collect();
                        if d.iter().all(|v| (0..=1).contains(v)) {
                            if d.iter().sum::<i64>() % 2 == 0 {
                                c[i] += 1.0;
                            } else {
                                c[i] -= 1.0;
                            }
                        }
                    }
                }
                c.into_iter()
                    .zip(set.iter().cloned())
                    .filter(|(c, _)| *c != 0.0)
                    .unzip()
            };
            for (cof, idx) in c.iter().zip(&idxs) {
                let m: Vec<usize> = idx.iter().map(|v| v + 1).collect();
                let (p, w) = tensor_grid(vars, &m)?;
                for (pt, wt) in p.into_iter().zip(w) {
                    let wt = wt * cof;
                    match index.get(&key(&pt)) {
                        Some(&k) => weights[k] += wt,
                        None => {
                            index.insert(key(&pt), points.len());
                            points.push(pt);
                            weights.push(wt);
                        }
                    }
                }
            }
        }
    }
    Ok(QuadratureGrid { points, weights })
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Index-set counts against their closed forms (total degree L in d
    /// dimensions has C(L + d, d) members; tensor product (L + 1)^d), the
    /// Smolyak grid's weights summing to the product of the rule masses, and
    /// the point conversions round-tripping.
    #[test]
    fn index_sets_grids_and_conversions() {
        assert_eq!(index_set(IndexSetKind::TotalDegree, 3, &[1.0, 1.0]).len(), 10);
        assert_eq!(index_set(IndexSetKind::TensorProduct, 2, &[1.0; 3]).len(), 27);
        let hc = index_set(IndexSetKind::HyperbolicCross, 3, &[1.0, 1.0]);
        assert!(hc.iter().all(|i| (i[0] + 1) * (i[1] + 1) <= 4));
        // Anisotropic: the more important variable reaches a higher order.
        let an = index_set(IndexSetKind::TotalDegree, 3, &[2.0, 1.0]);
        assert!(an.iter().any(|i| i[0] > an.iter().map(|j| j[1]).max().unwrap()));
        let vars = [
            PceVariable::Uniform {
                lower: 0.0,
                upper: 2.0,
            },
            PceVariable::Normal {
                mean: 1.0,
                std: 3.0,
            },
        ];
        for grid in [SparseGridKind::Smolyak, SparseGridKind::Tensor] {
            let set = index_set(IndexSetKind::TotalDegree, 4, &[1.0, 1.0]);
            let g = quadrature_grid(&vars, IndexSetKind::TotalDegree, &set, grid).unwrap();
            let total: f64 = g.weights.iter().sum();
            let mass = 2.0 * (2.0 * core::f64::consts::PI).sqrt();
            assert!((total - mass).abs() < 1e-12 * mass, "{grid:?}");
            assert_eq!(g.points.len(), g.weights.len());
        }
        for v in vars {
            assert!(v.validate().is_ok());
            assert!((v.to_quad(v.to_distr(0.3)) - 0.3).abs() < 1e-15);
        }
        assert!(PceVariable::Uniform {
            lower: 1.0,
            upper: 1.0
        }
        .validate()
        .is_err());
        assert!(PceVariable::Normal {
            mean: 0.0,
            std: 0.0
        }
        .validate()
        .is_err());
        assert_eq!(factorial(5), 120.0);
    }
}
