// ---------------------------------------------------------------------------
// Ported from SMT, the Surrogate Modeling Toolbox.
//
//   Upstream repo:    https://github.com/SMTorg/smt
//   Upstream files:   smt/kernels/kernels.py  (PowExp, Matern52, Matern32:
//                       values and grad_ind derivatives)
//                     smt/surrogate_models/krg_based/likelihood_eval.py
//                       (LikelihoodEvaluator.evaluate, .gradient)
//                     smt/surrogate_models/krg_based/distances.py
//                       (cross_distances, differences, componentwise_distance,
//                        componentwise_distance_PLS, constant, linear,
//                        quadratic)
//                     smt/utils/misc.py  (standardization)
//                     smt/surrogate_models/krg_based/krg_based.py
//                       (_optimize_hyperparam: log10 space, bounds, starts)
//   Upstream commit:  151a3ff3c13733d168b0ca8844ff0a4ff072252a  (master)
//   Accessed:         2026-11-04
//
//   Copyright (c) 2017, SMT developers. All rights reserved.
//   BSD 3-Clause License; the full text is this crate's LICENSE-BSD-SMT, and
//   NOTICE records it. Redistributions of source code must retain the above
//   copyright notice, this list of conditions and the disclaimer in
//   LICENSE-BSD-SMT.
//
// This Rust translation is part of RAFFLES / Outram Park and is distributed
// under AGPL-3.0-only. BSD-3-Clause -> (A)GPLv3 is ONE-WAY: this file may NOT
// be contributed back to SMT or redistributed under BSD terms.
//
// Translation notes:
//   * Kernels are the enum `SmtCorrelation` (no `Kernel` class hierarchy, no
//     kernel algebra `Sum`/`Product`, no `pow_exp`/`squar_sin_exp`/`act_exp`).
//     Only the continuous path: no mixed-integer / hierarchical kernels.
//   * NumPy reductions are reproduced where the order is known (sequential
//     along axis 0 of an (n, d) array; pairwise for an (n, 1) array or a
//     contiguous row, `polynomial_chaos::special::pairwise_sum`).
//   * LAPACK (Cholesky, triangular solves, QR, SVD) is `linalg`'s plain
//     loops: round-off-level differences, measured in the V&V record.
//   * The gradient's trace tr(R^-1 dR) is formed from an explicit R^-1
//     (one set of n solves per evaluation) where SMT solves the full dR twice
//     per hyperparameter; equal in exact arithmetic, O(n^3) cheaper.
//   * OPTIMISER: SMT's TNC (scipy) is NOT ported. The multistart runs this
//     crate's bounded BFGS (`gaussian_process::bounded_bfgs`) on SMT's own
//     objective and gradient in log10(theta), to convergence. Starts are
//     SMT's recorded ones (`SmtStarts::Explicit`) or this crate's LCG draws.
//   * Noise estimation (`eval_noise`, `noise0`, heteroscedastic noise,
//     `is_ri`) and the Hessian are not ported.
// ---------------------------------------------------------------------------

//! The numerics every SMT kriging route shares: kernels, regression bases,
//! standardisation, the reduced likelihood and its gradient, and the
//! multistart optimiser over log10 θ.

use petir::mathf::RealMath;

use super::linalg::{backward_lt, backward_upper, columns, dot, forward, householder_qr, singular_value_ratio};
use crate::surrogate::polynomial_chaos::special::pairwise_sum;
use crate::{RafflesError, Result};

/// SMT's correlation functions (`corr`). Each is a product of 1-D kernels on
/// the componentwise distances `d_i` (already `|Δx_i|^power`, or their PLS
/// projection).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SmtCorrelation {
    /// `squar_exp`, SMT's default: `exp(−Σ θ_i d_i)`, `d_i = Δx_i²`.
    SquarExp,
    /// `abs_exp`: `exp(−Σ θ_i d_i)`, `d_i = |Δx_i|`.
    AbsExp,
    /// `matern52`: `Π (1 + √5 l_i + 5/3 l_i²) exp(−√5 Σ l_i)`, `l_i = θ_i |Δx_i|`.
    Matern52,
    /// `matern32`: `Π (1 + √3 l_i) exp(−√3 Σ l_i)`, `l_i = θ_i |Δx_i|`.
    Matern32,
}

impl SmtCorrelation {
    /// SMT's `_pow_exp_power`: 2 for `squar_exp`, 1 otherwise.
    #[must_use]
    pub fn power(self) -> f64 {
        match self {
            Self::SquarExp => 2.0,
            Self::AbsExp | Self::Matern52 | Self::Matern32 => 1.0,
        }
    }

    /// The correlation of one pair, from its componentwise distances `d`.
    #[must_use]
    pub fn value(self, theta: &[f64], d: &[f64]) -> f64 {
        match self {
            Self::SquarExp | Self::AbsExp => {
                let t: Vec<f64> = theta.iter().zip(d).map(|(a, b)| a * b).collect();
                (-pairwise_sum(&t)).r_exp()
            }
            Self::Matern52 => {
                let s5 = 5.0_f64.sqrt();
                let ll: Vec<f64> = theta.iter().zip(d).map(|(a, b)| a * b).collect();
                let mut prod = 1.0;
                for (i, l) in ll.iter().enumerate() {
                    let fac = (1.0 + s5 * l) + 5.0 / 3.0 * (l * l);
                    prod = if i == 0 { fac } else { prod * fac };
                }
                prod * (-s5 * pairwise_sum(&ll)).r_exp()
            }
            Self::Matern32 => {
                let s3 = 3.0_f64.sqrt();
                let ll: Vec<f64> = theta.iter().zip(d).map(|(a, b)| a * b).collect();
                let mut prod = 1.0;
                for (i, l) in ll.iter().enumerate() {
                    let fac = 1.0 + s3 * l;
                    prod = if i == 0 { fac } else { prod * fac };
                }
                prod * (-s3 * pairwise_sum(&ll)).r_exp()
            }
        }
    }

    /// `∂r/∂θ_k` given `r` (SMT's `corr(d, grad_ind=k)`).
    #[must_use]
    pub fn derivative(self, theta: &[f64], d: &[f64], k: usize, r: f64) -> f64 {
        let dk = d[k];
        match self {
            Self::SquarExp | Self::AbsExp => -dk * r,
            Self::Matern52 => {
                let s5 = 5.0_f64.sqrt();
                let f1 = s5 * dk + 10.0 / 3.0 * theta[k] * (dk * dk);
                let f2 = (1.0 + s5 * theta[k] * dk) + 5.0 / 3.0 * (theta[k] * theta[k]) * (dk * dk);
                let f3 = s5 * dk;
                (f1 / f2 - f3) * r
            }
            Self::Matern32 => {
                let s3 = 3.0_f64.sqrt();
                let f1 = 1.0 / (1.0 + s3 * theta[k] * dk) - 1.0;
                f1 * r * s3 * dk
            }
        }
    }
}

/// SMT's regression bases (`poly`, and MFK's `rho_regr`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SmtRegression {
    /// `[1]`: ordinary kriging.
    Constant,
    /// `[1, x_1, …, x_n]`.
    Linear,
    /// `[1, x_1, …, x_n, x_1 x_1, x_1 x_2, …, x_n x_n]` (upper triangle, row by row).
    Quadratic,
}

impl SmtRegression {
    /// The basis at a (standardised) point.
    #[must_use]
    pub fn basis(self, x: &[f64]) -> Vec<f64> {
        let mut f = vec![1.0];
        if self != Self::Constant {
            f.extend_from_slice(x);
        }
        if self == Self::Quadratic {
            for k in 0..x.len() {
                for j in k..x.len() {
                    f.push(x[k] * x[j]);
                }
            }
        }
        f
    }
}

/// `standardization`: column means and sample stds (`ddof = 1`) of X, a std
/// below 100 ε replaced by 1; the mean and sample std of y, a zero std
/// replaced by 1. Returns `(x_offset, x_scale, y_mean, y_std)`.
#[must_use]
pub fn standardization(x: &[Vec<f64>], y: &[f64]) -> (Vec<f64>, Vec<f64>, f64, f64) {
    let n = x.len() as f64;
    let d = x.first().map_or(0, Vec::len);
    // Axis 0 of an (n, d) array: a sequential sum per column.
    let mut off = vec![0.0; d];
    for r in x {
        for j in 0..d {
            off[j] += r[j];
        }
    }
    for v in &mut off {
        *v /= n;
    }
    let mut ss = vec![0.0; d];
    for r in x {
        for j in 0..d {
            let c = r[j] - off[j];
            ss[j] += c * c;
        }
    }
    let scale: Vec<f64> = ss
        .iter()
        .map(|s| {
            let v = (s / (n - 1.0)).sqrt();
            if v.abs() < 100.0 * f64::EPSILON {
                1.0
            } else {
                v
            }
        })
        .collect();
    // An (n, 1) array: pairwise.
    let y_mean = pairwise_sum(y) / n;
    let sq: Vec<f64> = y.iter().map(|v| (v - y_mean) * (v - y_mean)).collect();
    let mut y_std = (pairwise_sum(&sq) / (n - 1.0)).sqrt();
    if y_std == 0.0 {
        y_std = 1.0;
    }
    (off, scale, y_mean, y_std)
}

/// How componentwise distances are formed.
#[derive(Debug, Clone, PartialEq)]
pub enum Projection {
    /// `|Δx_i|^power` in every input dimension (KRG, MFK, KPLSK's second pass).
    Full,
    /// KPLS: `Δx² · coeff²` (squar_exp) or `|Δx| · |coeff|` (otherwise), one
    /// column per PLS component; `coeff` is `nx × n_comp`.
    Pls(Vec<Vec<f64>>),
}

impl Projection {
    /// The componentwise distances of one difference vector `dx`.
    #[must_use]
    pub fn distances(&self, corr: SmtCorrelation, dx: &[f64]) -> Vec<f64> {
        match self {
            Self::Full => dx
                .iter()
                .map(|v| if corr.power() == 2.0 { v.abs() * v.abs() } else { v.abs() })
                .collect(),
            Self::Pls(coeff) => {
                let nc = coeff.first().map_or(0, Vec::len);
                (0..nc)
                    .map(|l| {
                        dx.iter()
                            .zip(coeff)
                            .map(|(v, c)| {
                                if corr == SmtCorrelation::SquarExp {
                                    (v * v) * (c[l] * c[l])
                                } else {
                                    v.abs() * c[l].abs()
                                }
                            })
                            .fold(0.0, |s, t| s + t)
                    })
                    .collect()
            }
        }
    }
}

/// The training pairs `(i, j)`, `i < j`, in SMT's order, and their
/// componentwise distances.
#[derive(Debug, Clone, PartialEq)]
pub struct Pairs {
    /// Indices.
    pub ij: Vec<(usize, usize)>,
    /// Distances, one row per pair.
    pub d: Vec<Vec<f64>>,
}

impl Pairs {
    /// `cross_distances` then the componentwise transform.
    #[must_use]
    pub fn new(x: &[Vec<f64>], corr: SmtCorrelation, proj: &Projection) -> Self {
        let n = x.len();
        let mut ij = Vec::with_capacity(n * n.saturating_sub(1) / 2);
        let mut d = Vec::with_capacity(ij.capacity());
        for k in 0..n {
            for l in k + 1..n {
                ij.push((k, l));
                let dx: Vec<f64> = x[k].iter().zip(&x[l]).map(|(a, b)| a - b).collect();
                d.push(proj.distances(corr, &dx));
            }
        }
        Self { ij, d }
    }
}

/// One level's training problem: standardised inputs' pair distances, the
/// regression matrix `F`, the standardised outputs, and how many of `F`'s
/// columns are the regression (`p`) and the autoregressive part (`q`).
#[derive(Debug, Clone, PartialEq)]
pub struct Level {
    /// Pair distances of the standardised inputs.
    pub pairs: Pairs,
    /// `n × (q + p)`.
    pub f: Vec<Vec<f64>>,
    /// Standardised outputs.
    pub y: Vec<f64>,
    /// Regression columns.
    pub p: usize,
    /// Autoregressive (ρ) columns, 0 for a single-fidelity model.
    pub q: usize,
    /// SMT's `y_std` (scales σ² back to output units).
    pub y_std: f64,
    /// The nugget.
    pub nugget: f64,
    /// The correlation.
    pub corr: SmtCorrelation,
}

/// What `LikelihoodEvaluator.evaluate` puts in `par`.
#[derive(Debug, Clone, PartialEq)]
pub struct LevelPar {
    /// Process variance, **in output units** (SMT multiplies by `y_std²`).
    pub sigma2: f64,
    /// GLS weights.
    pub beta: Vec<f64>,
    /// `R⁻¹ (y − F β)`.
    pub gamma: Vec<f64>,
    /// Lower Cholesky factor of `R`.
    pub c: Vec<Vec<f64>>,
    /// `C⁻¹ F`, `n × (q + p)`.
    pub ft: Vec<Vec<f64>>,
    /// `Q` of `Ft = Q G`.
    pub q: Vec<Vec<f64>>,
    /// `G` of `Ft = Q G`.
    pub g: Vec<Vec<f64>>,
}

impl Level {
    fn n(&self) -> usize {
        self.y.len()
    }

    /// The correlation matrix at `theta` (`R = (1 + nugget) I`, off-diagonals
    /// the pair correlations), and the pair correlations.
    fn r_matrix(&self, theta: &[f64]) -> (Vec<Vec<f64>>, Vec<f64>) {
        let n = self.n();
        let r: Vec<f64> = self.pairs.d.iter().map(|d| self.corr.value(theta, d)).collect();
        let mut m = vec![vec![0.0; n]; n];
        for (i, row) in m.iter_mut().enumerate() {
            row[i] = 1.0 + self.nugget + 0.0;
        }
        for ((i, j), v) in self.pairs.ij.iter().zip(&r) {
            m[*i][*j] = *v;
            m[*j][*i] = *v;
        }
        (m, r)
    }

    /// `_reduced_likelihood_function`: the reduced log10 likelihood and the
    /// GLS parameters, or `None` where SMT returns `−inf` (a NaN correlation,
    /// a failed Cholesky, an ill-conditioned `G` with a well-conditioned `F`).
    ///
    /// # Errors
    /// SMT's "F is too ill conditioned" (cond(F) > 1e15).
    pub fn evaluate(&self, theta: &[f64]) -> Result<Option<(f64, LevelPar)>> {
        let n = self.n();
        let (rm, r) = self.r_matrix(theta);
        if r.iter().any(|v| v.is_nan()) {
            return Ok(None);
        }
        let Some(c) = crate::bayesian::transitional::cholesky(&rm, 0.0) else {
            return Ok(None);
        };
        let fcols = columns(&self.f);
        let ftc: Vec<Vec<f64>> = fcols.iter().map(|col| forward(&c, col)).collect();
        let ft: Vec<Vec<f64>> = (0..n).map(|i| ftc.iter().map(|col| col[i]).collect()).collect();
        let (q, g) = householder_qr(&ft);
        if singular_value_ratio(&g) < 1e-10 {
            if 1.0 / singular_value_ratio(&self.f) > 1e15 {
                return Err(RafflesError::InvalidParameter {
                    parameter: "regression".to_string(),
                    value: f64::NAN,
                    reason: "F is too ill conditioned. Poor combination of regression model and observations.".to_string(),
                });
            }
            return Ok(None);
        }
        let yt = forward(&c, &self.y);
        let qcols = columns(&q);
        let qty: Vec<f64> = qcols.iter().map(|col| dot(col, &yt)).collect();
        let beta = backward_upper(&g, &qty);
        let rho: Vec<f64> = (0..n).map(|i| yt[i] - dot(&ft[i], &beta)).collect();
        let dof = (n - self.p - self.q) as f64;
        let mut det_r = 1.0;
        for (i, row) in c.iter().enumerate() {
            let v = row[i].r_powf(2.0 / n as f64);
            det_r = if i == 0 { v } else { det_r * v };
        }
        let sq: Vec<f64> = rho.iter().map(|v| v * v).collect();
        let sigma2 = pairwise_sum(&sq) / dof;
        let mut rlf = -dof * sigma2.r_log10() - n as f64 * det_r.r_log10();
        if rlf > 1e15 {
            rlf = 1e15;
        }
        let gamma = backward_lt(&c, &rho);
        Ok(Some((
            rlf,
            LevelPar {
                sigma2: sigma2 * (self.y_std * self.y_std),
                beta,
                gamma,
                c,
                ft,
                q,
                g,
            },
        )))
    }

    /// `_reduced_likelihood_gradient`: `∂ rlf / ∂ θ` (linear θ), with SMT's
    /// `σ² + nugget` (finding 2 of the V&V record). `None` where
    /// [`evaluate`](Self::evaluate) is.
    ///
    /// # Errors
    /// As [`evaluate`](Self::evaluate).
    pub fn gradient(&self, theta: &[f64]) -> Result<Option<(f64, Vec<f64>)>> {
        let Some((rlf, par)) = self.evaluate(theta)? else {
            return Ok(None);
        };
        let n = self.n();
        let nf = n as f64;
        let sigma_2 = par.sigma2 + self.nugget;
        let (_, r) = self.r_matrix(theta);
        // R⁻¹ column by column, for the trace term.
        let mut rinv = vec![vec![0.0; n]; n];
        for col in 0..n {
            let mut e = vec![0.0; n];
            e[col] = 1.0;
            let x = backward_lt(&par.c, &forward(&par.c, &e));
            for (row, v) in x.iter().enumerate() {
                rinv[row][col] = *v;
            }
        }
        let qcols = columns(&par.q);
        let ln10 = 10.0_f64.r_ln();
        let mut grad = Vec::with_capacity(theta.len());
        for k in 0..theta.len() {
            let dr: Vec<f64> = self.pairs.d.iter().zip(&r).map(|(d, rv)| self.corr.derivative(theta, d, k, *rv)).collect();
            // dR γ, and tr(R⁻¹ dR) = 2 Σ_{pairs} R⁻¹_ij dR_ij (dR symmetric, zero diagonal).
            let mut drg = vec![0.0; n];
            let mut tr = 0.0;
            for ((i, j), v) in self.pairs.ij.iter().zip(&dr) {
                drg[*i] += v * par.gamma[*j];
                drg[*j] += v * par.gamma[*i];
                tr += 2.0 * rinv[*i][*j] * v;
            }
            let cinv_dr_gamma = forward(&par.c, &drg);
            let qt: Vec<f64> = qcols.iter().map(|col| dot(col, &cinv_dr_gamma)).collect();
            let dbeta: Vec<f64> = backward_upper(&par.g, &qt).iter().map(|v| -v).collect();
            let dmu: Vec<f64> = self.f.iter().map(|row| dot(row, &dbeta)).collect();
            let dmu_g = dot(&dmu, &par.gamma);
            let g_dr_g = dot(&par.gamma, &drg);
            let dsigma_2 = (1.0 / nf) * (-dmu_g - dmu_g - g_dr_g) * (self.y_std * self.y_std);
            grad.push(-nf / ln10 * (dsigma_2 / sigma_2 + tr / nf));
        }
        Ok(Some((rlf, grad)))
    }
}

/// How the hyperparameters are chosen.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SmtOptimizer {
    /// Maximise the reduced likelihood over log10 θ within the bounds, from
    /// every start (SMT's TNC multistart; here this crate's bounded BFGS).
    MaximiseLikelihood,
    /// Keep θ0 exactly (SMT's `hyper_opt = "NoOp"`, except that SMT returns
    /// `10**log10(θ0)`, which can differ from θ0 by an ulp).
    None,
}

/// Starting points of the multistart, in log10 θ.
#[derive(Debug, Clone, PartialEq)]
pub enum SmtStarts {
    /// SMT's arrangement: log10 θ0, one uniform draw in the log10 bounds, and
    /// an `n_start`-point Latin hypercube, drawn from this crate's
    /// `petir::rng::lcg` streams (`seed`), not numpy's PCG64: the points are
    /// not SMT's.
    Default {
        /// SMT's `n_start` (10).
        n_start: usize,
        /// Master seed of the two designs.
        seed: i64,
    },
    /// Given starts (log10 θ rows), e.g. the ones SMT recorded. For KPLSK,
    /// rows whose length is the input dimension are used in the second pass
    /// (after the projected θ0), the others in the first.
    Explicit(Vec<Vec<f64>>),
}

impl SmtStarts {
    /// SMT's default: `n_start = 10`, here with seed 41 (SMT's numpy seed,
    /// used for a different generator).
    #[must_use]
    pub fn smt_default() -> Self {
        Self::Default { n_start: 10, seed: 41 }
    }

    /// The start rows for `m` hyperparameters, `theta0_log` first; `lhs`
    /// false gives only θ0 and one uniform draw (SMT's `use_multistart =
    /// False`). For `Explicit`, the rows of length `m`.
    ///
    /// # Errors
    /// A zero-size design.
    pub fn rows(&self, theta0_log: &[f64], log_bounds: (f64, f64), lhs: bool) -> Result<Vec<Vec<f64>>> {
        use crate::samplers::{LatinHypercube, MonteCarlo, SamplingDesign};
        let m = theta0_log.len();
        match self {
            Self::Explicit(rows) => Ok(rows.iter().filter(|r| r.len() == m).cloned().collect()),
            Self::Default { n_start, seed } => {
                let (lo, hi) = log_bounds;
                let map = |u: f64| lo + u * (hi - lo);
                let mut out = vec![theta0_log.to_vec()];
                out.extend(MonteCarlo::new(1, m)?.generate(*seed).into_iter().map(|r| r.into_iter().map(map).collect::<Vec<f64>>()));
                if lhs && *n_start > 1 {
                    out.extend(LatinHypercube::new(*n_start, m)?.generate(seed.wrapping_add(1)).into_iter().map(|r| r.into_iter().map(map).collect::<Vec<f64>>()));
                }
                Ok(out)
            }
        }
    }
}

/// The multistart: from every start, the bounded BFGS on `−rlf(10^t)` with
/// SMT's gradient by the chain rule; the best end point (lowest `−rlf`, the
/// first on a tie, as SMT's `result["fun"] < best["fun"]`). Starts whose
/// likelihood is `−inf` are skipped. `None` if every start fails.
///
/// # Errors
/// [`Level::evaluate`]'s ill-conditioned-F error.
pub fn maximise(level: &Level, starts: &[Vec<f64>], log_bounds: (f64, f64)) -> Result<Option<Vec<f64>>> {
    let ln10 = 10.0_f64.r_ln();
    let bounds = vec![log_bounds; starts.first().map_or(0, Vec::len)];
    let failure = core::cell::RefCell::new(None);
    let eval = |t: &[f64]| -> Option<(f64, Vec<f64>)> {
        let th: Vec<f64> = t.iter().map(|v| 10.0_f64.r_powf(*v)).collect();
        match level.gradient(&th) {
            Ok(Some((rlf, g))) => Some((-rlf, g.iter().zip(&th).map(|(gi, ti)| -ln10 * ti * gi).collect())),
            Ok(None) => None,
            Err(e) => {
                *failure.borrow_mut() = Some(e);
                None
            }
        }
    };
    let mut best: Option<(f64, Vec<f64>)> = None;
    for s in starts {
        let mut s0 = s.clone();
        for v in &mut s0 {
            *v = v.clamp(log_bounds.0, log_bounds.1);
        }
        let Some(t) = crate::surrogate::gaussian_process::bounded_bfgs(s0, &bounds, &eval) else {
            continue;
        };
        if let Some((fv, _)) = eval(&t) {
            if best.as_ref().is_none_or(|b| fv < b.0) {
                best = Some((fv, t));
            }
        }
    }
    if let Some(e) = failure.into_inner() {
        if best.is_none() {
            return Err(e);
        }
    }
    Ok(best.map(|(_, t)| t.iter().map(|v| 10.0_f64.r_powf(*v)).collect()))
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Each kernel's θ-derivative against a central finite difference, and the
    /// likelihood gradient against finite differences of the likelihood, on a
    /// small 2-D design; the bases' widths; standardisation of a constant.
    #[test]
    fn derivatives_match_finite_differences() {
        let x: Vec<Vec<f64>> = (0..7).map(|i| vec![f64::from(i) * 0.3 - 1.0, (f64::from(i) * 1.7).sin()]).collect();
        let y: Vec<f64> = x.iter().map(|r| r[0] * r[0] + 0.5 * r[1]).collect();
        let (off, sc, ym, ys) = standardization(&x, &y);
        let xn: Vec<Vec<f64>> = x.iter().map(|r| r.iter().zip(&off).zip(&sc).map(|((v, o), s)| (v - o) / s).collect()).collect();
        let yn: Vec<f64> = y.iter().map(|v| (v - ym) / ys).collect();
        for corr in [SmtCorrelation::SquarExp, SmtCorrelation::AbsExp, SmtCorrelation::Matern52, SmtCorrelation::Matern32] {
            let theta = [0.7, 1.9];
            let d = [0.4, 1.3];
            let r = corr.value(&theta, &d);
            for k in 0..2 {
                let h = 1e-6;
                let mut tp = theta;
                let mut tm = theta;
                tp[k] += h;
                tm[k] -= h;
                let fd = (corr.value(&tp, &d) - corr.value(&tm, &d)) / (2.0 * h);
                assert!((fd - corr.derivative(&theta, &d, k, r)).abs() < 1e-8, "{corr:?} {k}");
            }
            let level = Level {
                pairs: Pairs::new(&xn, corr, &Projection::Full),
                f: xn.iter().map(|r| SmtRegression::Linear.basis(r)).collect(),
                y: yn.clone(),
                p: 3,
                q: 0,
                y_std: ys,
                nugget: 100.0 * f64::EPSILON,
                corr,
            };
            let (_, g) = level.gradient(&theta).unwrap().unwrap();
            for k in 0..2 {
                let h = 1e-6;
                let mut tp = theta.to_vec();
                let mut tm = theta.to_vec();
                tp[k] += h;
                tm[k] -= h;
                let fd = (level.evaluate(&tp).unwrap().unwrap().0 - level.evaluate(&tm).unwrap().unwrap().0) / (2.0 * h);
                assert!((fd - g[k]).abs() < 1e-5 * (1.0 + fd.abs()), "{corr:?} {k}: {fd} vs {}", g[k]);
            }
            let best = maximise(&level, &SmtStarts::smt_default().rows(&[-2.0, -2.0], (-6.0, 20f64.log10()), true).unwrap(), (-6.0, 20f64.log10())).unwrap().unwrap();
            assert!(level.evaluate(&best).unwrap().unwrap().0 >= level.evaluate(&theta).unwrap().unwrap().0);
        }
        assert_eq!(SmtRegression::Quadratic.basis(&[2.0, 3.0]), vec![1.0, 2.0, 3.0, 4.0, 6.0, 9.0]);
        assert_eq!(SmtRegression::Constant.basis(&[2.0]), vec![1.0]);
        let (_, sc, _, ys) = standardization(&[vec![1.0], vec![1.0]], &[2.0, 2.0]);
        assert_eq!((sc[0], ys), (1.0, 1.0));
        assert_eq!(SmtStarts::Explicit(vec![vec![0.0], vec![1.0, 2.0]]).rows(&[0.0], (-1.0, 1.0), true).unwrap().len(), 1);
        // PLS projection: Δx² · c² for squar_exp, |Δx| · |c| otherwise.
        let p = Projection::Pls(vec![vec![0.5], vec![-2.0]]);
        assert_eq!(p.distances(SmtCorrelation::SquarExp, &[2.0, 1.0]), vec![4.0 * 0.25 + 4.0]);
        assert_eq!(p.distances(SmtCorrelation::AbsExp, &[-2.0, 1.0]), vec![1.0 + 2.0]);
    }
}
