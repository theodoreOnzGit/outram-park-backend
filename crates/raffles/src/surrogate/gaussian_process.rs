// ---------------------------------------------------------------------------
// Two upstreams, as in RAVEN itself: RAVEN owns the interface and defaults,
// scikit-learn owns the numerics.
//
// (1) Ported from RAVEN (Risk Analysis Virtual ENvironment) — the interface:
//   Upstream project: RAVEN — Idaho National Laboratory
//   Upstream repo:    https://github.com/idaholab/raven
//   Upstream file:    ravenframework/SupervisedLearning/ScikitLearn/
//                     GaussianProcess/GaussianProcessRegressor.py
//                     (`pickKernel`, `customKernel`, the input defaults)
//   Upstream commit:  8baaae3a97f1392318074ad6d3f8efc26ca81c7e  (branch devel)
//   Accessed:         2026-11-04
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
// (2) Ported from scikit-learn 1.3.2 (BSD-3-Clause) — the numerics:
//   Upstream repo:    https://github.com/scikit-learn/scikit-learn (tag 1.3.2)
//   Upstream files:   sklearn/gaussian_process/_gpr.py  (fit, predict,
//                     log_marginal_likelihood)
//                     sklearn/gaussian_process/kernels.py  (ConstantKernel,
//                     RBF, Matern nu in {0.5, 1.5, 2.5}, WhiteKernel, Sum,
//                     Product: values, diagonals, log-space gradients)
//                     sklearn/preprocessing/_data.py  (_handle_zeros_in_scale)
//   Copyright (c) 2007-2023 The scikit-learn developers. BSD-3-Clause; the
//   full text is this crate's LICENSE-BSD-SKLEARN, and NOTICE records it.
//
// This Rust translation is part of RAFFLES / Outram Park and is distributed
// under AGPL-3.0-only. Apache-2.0 and BSD-3-Clause -> (A)GPLv3 are ONE-WAY:
// this file may NOT be contributed back to RAVEN or scikit-learn, or
// redistributed under Apache-2.0 or BSD terms.
//
// Translation notes:
//   * sklearn's kernel object algebra (`Sum`, `Product`, operator
//     overloading) is a recursive tree; here it is the flat `GpKernel`
//     struct `[amplitude *] base [+ white]`, which covers RAVEN's kernel menu
//     for regression (Constant, RBF, Matern, WhiteNoise) and its custom
//     strings of the shapes `Constant*RBF`, `Constant*Matern`,
//     `RBF+WhiteNoise`, `Constant*RBF+WhiteNoise`. The hyperparameter vector
//     `theta` is in sklearn's order (amplitude, length scales, noise), in
//     log space, fixed ones omitted.
//   * Kept: `alpha` added to the diagonal, the lower Cholesky, `cho_solve`,
//     `normalize_y` (numpy mean and population std, a zero std replaced by
//     1), the predictive variance `k(x,x) − vᵀv` clipped at 0 and rescaled,
//     the LML and its gradient `0.5 Σ (ααᵀ − K⁻¹) ∘ ∂K`.
//   * Linear algebra is plain loops (no BLAS/LAPACK: crate rule), so the
//     operation order differs from LAPACK's blocked `dpotrf` and from
//     numpy's AVX-512 `exp`; the V&V record gives the measured gap.
//   * OPTIMISER: sklearn calls scipy's L-BFGS-B. It is NOT ported. Here a
//     bounded quasi-Newton (BFGS on the log hyperparameters, projected onto
//     the bounds, Armijo backtracking) maximises the same LML from the same
//     starting point. Results agree at the optimum, not along the path.
//     `n_restarts_optimizer > 0` (random restarts from sklearn's RNG) is not
//     ported.
//   * RAVEN wraps the GP in sklearn's MultiOutputRegressor by default
//     (`<multioutput>` True), which (a) fits one GP per output, kept here,
//     and (b) makes RAVEN drop the standard deviation, NOT kept: this port
//     always returns it.
// ---------------------------------------------------------------------------

//! **Gaussian-process regression, RAVEN's route**: RAVEN's
//! `GaussianProcessRegressor` ROM (its kernel menu and defaults) over
//! scikit-learn's numerics (gh:#1193). The SMT kriging port (gh:#1190) is
//! the second route; the two are to be cross-checked there.
//!
//! RAVEN's defaults, which [`GpConfig::raven_defaults`] reproduces: no kernel
//! given (so sklearn's `1.0 (fixed) * RBF(1.0, fixed)`, which leaves
//! **nothing to optimise**), `alpha = 1e-10`, `normalize_y = false`,
//! L-BFGS-B, no restarts. Pick a kernel with [`GpKernel::raven`] to get one
//! whose hyperparameters are fitted.
//!
//! The leave-one-out residuals use the closed form of Rasmussen and
//! Williams (2006), *Gaussian Processes for Machine Learning*, eq. 5.12, at
//! the fitted hyperparameters (an independent addition, not from either
//! upstream).
//!
//! # V&V
//!
//! Record: `crates/raffles/verification_and_validation/raven_routes_gh1193.md`;
//! differential tests in `tests/raven_routes_gh1193.rs`.

use petir::mathf::RealMath;

use crate::{RafflesError, Result};

/// sklearn's default hyperparameter bounds, `(1e-5, 1e5)`.
pub const SKLEARN_DEFAULT_BOUNDS: (f64, f64) = (1e-5, 1e5);

/// One scalar hyperparameter: its value, and its bounds (`None` = fixed).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Hyper {
    /// The value (not its log).
    pub value: f64,
    /// `(lower, upper)`, or `None` for a fixed hyperparameter.
    pub bounds: Option<(f64, f64)>,
}

impl Hyper {
    /// sklearn's default: value with bounds `(1e-5, 1e5)`.
    #[must_use]
    pub fn free(value: f64) -> Self {
        Self {
            value,
            bounds: Some(SKLEARN_DEFAULT_BOUNDS),
        }
    }

    /// A fixed hyperparameter.
    #[must_use]
    pub fn fixed(value: f64) -> Self {
        Self {
            value,
            bounds: None,
        }
    }
}

/// The stationary part of the kernel.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BaseKernel {
    /// Squared exponential, `exp(−d²/2)` on scaled distances.
    Rbf,
    /// Matérn ν = 1/2.
    Matern12,
    /// Matérn ν = 3/2.
    Matern32,
    /// Matérn ν = 5/2 (RAVEN's `Matern`).
    Matern52,
}

/// RAVEN's kernel names for regression (`pickKernel`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RavenKernel {
    /// `RBF`: `RBF(length_scale=1 or ones(d))`.
    Rbf,
    /// `Matern`: `Matern(length_scale, nu=2.5)`.
    Matern,
    /// `Constant*RBF`.
    ConstantRbf,
    /// `Constant*Matern`.
    ConstantMatern,
    /// `RBF+WhiteNoise`.
    RbfWhite,
    /// `Constant*RBF+WhiteNoise`.
    ConstantRbfWhite,
}

/// `[amplitude ×] base(length scales) [+ white noise]`.
#[derive(Debug, Clone, PartialEq)]
pub struct GpKernel {
    /// A `ConstantKernel` factor; `None` for none.
    pub amplitude: Option<Hyper>,
    /// The stationary kernel.
    pub base: BaseKernel,
    /// One length scale (isotropic) or one per input (anisotropic).
    pub length_scale: Vec<f64>,
    /// Bounds shared by every length scale; `None` = fixed.
    pub length_scale_bounds: Option<(f64, f64)>,
    /// A `WhiteKernel` term; `None` for none.
    pub white: Option<Hyper>,
}

impl GpKernel {
    /// sklearn's default when RAVEN passes no kernel:
    /// `1.0 (fixed) * RBF(1.0, fixed)`.
    #[must_use]
    pub fn raven_default() -> Self {
        Self {
            amplitude: Some(Hyper::fixed(1.0)),
            base: BaseKernel::Rbf,
            length_scale: vec![1.0],
            length_scale_bounds: None,
            white: None,
        }
    }

    /// RAVEN's `pickKernel`/`customKernel` for `name`, on `d` inputs, with
    /// RAVEN's `<anisotropic>` flag; every hyperparameter starts at 1 with
    /// sklearn's default bounds.
    #[must_use]
    pub fn raven(name: RavenKernel, d: usize, anisotropic: bool) -> Self {
        let (amp, base, white) = match name {
            RavenKernel::Rbf => (false, BaseKernel::Rbf, false),
            RavenKernel::Matern => (false, BaseKernel::Matern52, false),
            RavenKernel::ConstantRbf => (true, BaseKernel::Rbf, false),
            RavenKernel::ConstantMatern => (true, BaseKernel::Matern52, false),
            RavenKernel::RbfWhite => (false, BaseKernel::Rbf, true),
            RavenKernel::ConstantRbfWhite => (true, BaseKernel::Rbf, true),
        };
        Self {
            amplitude: amp.then(|| Hyper::free(1.0)),
            base,
            length_scale: vec![1.0; if anisotropic { d } else { 1 }],
            length_scale_bounds: Some(SKLEARN_DEFAULT_BOUNDS),
            white: white.then(|| Hyper::free(1.0)),
        }
    }

    /// The free hyperparameters, `ln` of each, in sklearn's order.
    #[must_use]
    pub fn theta(&self) -> Vec<f64> {
        let mut t = Vec::new();
        if let Some(Hyper {
            value,
            bounds: Some(_),
        }) = self.amplitude
        {
            t.push(value.r_ln());
        }
        if self.length_scale_bounds.is_some() {
            t.extend(self.length_scale.iter().map(|v| v.r_ln()));
        }
        if let Some(Hyper {
            value,
            bounds: Some(_),
        }) = self.white
        {
            t.push(value.r_ln());
        }
        t
    }

    /// Bounds of [`theta`](Self::theta), in log space.
    #[must_use]
    pub fn bounds(&self) -> Vec<(f64, f64)> {
        let lg = |(a, b): (f64, f64)| (a.r_ln(), b.r_ln());
        let mut t = Vec::new();
        if let Some(Hyper { bounds: Some(b), .. }) = self.amplitude {
            t.push(lg(b));
        }
        if let Some(b) = self.length_scale_bounds {
            t.extend(self.length_scale.iter().map(|_| lg(b)));
        }
        if let Some(Hyper { bounds: Some(b), .. }) = self.white {
            t.push(lg(b));
        }
        t
    }

    /// A copy with the free hyperparameters set from `theta` (log space,
    /// sklearn's `clone_with_theta`).
    ///
    /// # Errors
    ///
    /// [`RafflesError::DimensionMismatch`] for a wrong-length `theta`.
    pub fn with_theta(&self, theta: &[f64]) -> Result<Self> {
        if theta.len() != self.theta().len() {
            return Err(RafflesError::DimensionMismatch {
                expected: self.theta().len(),
                found: theta.len(),
            });
        }
        let mut k = self.clone();
        let mut it = theta.iter();
        if let Some(h) = k.amplitude.as_mut() {
            if h.bounds.is_some() {
                h.value = it.next().map_or(h.value, |v| v.r_exp());
            }
        }
        if k.length_scale_bounds.is_some() {
            for l in &mut k.length_scale {
                *l = it.next().map_or(*l, |v| v.r_exp());
            }
        }
        if let Some(h) = k.white.as_mut() {
            if h.bounds.is_some() {
                h.value = it.next().map_or(h.value, |v| v.r_exp());
            }
        }
        Ok(k)
    }

    fn scaled(&self, x: &[f64]) -> Vec<f64> {
        if self.length_scale.len() == 1 {
            x.iter().map(|v| v / self.length_scale[0]).collect()
        } else {
            x.iter().zip(&self.length_scale).map(|(v, l)| v / l).collect()
        }
    }

    /// The base kernel's value at scaled squared distance `d2` (RBF) or
    /// distance `sqrt(d2)` (Matérn), in sklearn's operation order.
    fn base_value(&self, d2: f64) -> f64 {
        match self.base {
            BaseKernel::Rbf => (-0.5 * d2).r_exp(),
            BaseKernel::Matern12 => (-d2.sqrt()).r_exp(),
            BaseKernel::Matern32 => {
                let k = d2.sqrt() * 3.0_f64.sqrt();
                (1.0 + k) * (-k).r_exp()
            }
            BaseKernel::Matern52 => {
                let k = d2.sqrt() * 5.0_f64.sqrt();
                (1.0 + k + k * k / 3.0) * (-k).r_exp()
            }
        }
    }

    /// `k(x, y)` for `x != y` rows (the `Y` path: no white noise).
    fn cross(&self, x: &[f64], y: &[f64]) -> f64 {
        let (a, b) = (self.scaled(x), self.scaled(y));
        let d2 = a.iter().zip(&b).fold(0.0, |s, (u, v)| {
            let d = u - v;
            s + d * d
        });
        let k = self.base_value(d2);
        match self.amplitude {
            Some(h) => h.value * k,
            None => k,
        }
    }

    /// `kernel.diag(x)`.
    fn diag(&self) -> f64 {
        let k = match self.amplitude {
            Some(h) => h.value * 1.0,
            None => 1.0,
        };
        match self.white {
            Some(w) => k + w.value,
            None => k,
        }
    }

    /// `kernel(X)` and, for the free hyperparameters, `∂K/∂θ` (log space),
    /// `grad[p][i][j]`.
    fn gram(&self, x: &[Vec<f64>]) -> (Vec<Vec<f64>>, Vec<Vec<Vec<f64>>>) {
        let n = x.len();
        let xs: Vec<Vec<f64>> = x.iter().map(|r| self.scaled(r)).collect();
        let mut base = vec![vec![0.0; n]; n];
        let mut d2m = vec![vec![0.0; n]; n];
        for i in 0..n {
            for j in 0..n {
                if i == j {
                    base[i][j] = 1.0;
                    continue;
                }
                let d2 = xs[i].iter().zip(&xs[j]).fold(0.0, |s, (u, v)| {
                    let d = u - v;
                    s + d * d
                });
                d2m[i][j] = d2;
                base[i][j] = self.base_value(d2);
            }
        }
        let amp = self.amplitude.map_or(1.0, |h| h.value);
        let mut grads = Vec::new();
        if let Some(Hyper { bounds: Some(_), .. }) = self.amplitude {
            // Product rule, the ConstantKernel factor: ∂(c k)/∂ ln c = c k.
            grads.push(base.iter().map(|r| r.iter().map(|v| amp * v).collect()).collect());
        }
        if self.length_scale_bounds.is_some() {
            let dims: Vec<Option<usize>> = if self.length_scale.len() == 1 {
                vec![None]
            } else {
                (0..self.length_scale.len()).map(Some).collect()
            };
            for dim in dims {
                let mut g = vec![vec![0.0; n]; n];
                for i in 0..n {
                    for j in 0..n {
                        if i == j {
                            continue;
                        }
                        let dd = match dim {
                            None => d2m[i][j],
                            Some(k) => {
                                let d = x[i][k] - x[j][k];
                                d * d / (self.length_scale[k] * self.length_scale[k])
                            }
                        };
                        let gb = match self.base {
                            BaseKernel::Rbf => match dim {
                                None => base[i][j] * dd,
                                Some(_) => dd * base[i][j],
                            },
                            BaseKernel::Matern12 => base[i][j] * (dd / d2m[i][j].sqrt()),
                            BaseKernel::Matern32 => {
                                3.0 * dd * (-(3.0 * d2m[i][j]).sqrt()).r_exp()
                            }
                            BaseKernel::Matern52 => {
                                let tmp = (5.0 * d2m[i][j]).sqrt();
                                5.0 / 3.0 * dd * (tmp + 1.0) * (-tmp).r_exp()
                            }
                        };
                        // Product rule, the base factor: c ∂k.
                        g[i][j] = match self.amplitude {
                            Some(_) => gb * amp,
                            None => gb,
                        };
                    }
                }
                grads.push(g);
            }
        }
        let mut k: Vec<Vec<f64>> = base
            .iter()
            .map(|r| {
                r.iter()
                    .map(|v| match self.amplitude {
                        Some(h) => h.value * v,
                        None => *v,
                    })
                    .collect()
            })
            .collect();
        if let Some(w) = self.white {
            for (i, row) in k.iter_mut().enumerate() {
                row[i] += w.value;
            }
            if w.bounds.is_some() {
                grads.push(
                    (0..n)
                        .map(|i| (0..n).map(|j| if i == j { w.value } else { 0.0 }).collect())
                        .collect(),
                );
            }
        }
        (k, grads)
    }
}

/// How the hyperparameters are chosen.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GpOptimizer {
    /// Maximise the log marginal likelihood (RAVEN's `fmin_l_bfgs_b`, here a
    /// bounded BFGS; see the module notes).
    MaximiseLml,
    /// Keep the kernel's hyperparameters (RAVEN's `optimizer = None`).
    None,
}

/// RAVEN's `GaussianProcessRegressor` settings.
#[derive(Debug, Clone, PartialEq)]
pub struct GpConfig {
    /// The kernel and its starting hyperparameters.
    pub kernel: GpKernel,
    /// Added to the kernel diagonal in the fit (`<alpha>`, default 1e-10).
    pub alpha: f64,
    /// Standardise the targets (`<normalize_y>`, default false).
    pub normalize_y: bool,
    /// `<optimizer>`.
    pub optimizer: GpOptimizer,
}

impl GpConfig {
    /// RAVEN's defaults: [`GpKernel::raven_default`], alpha 1e-10, no target
    /// normalisation, the LML maximised.
    #[must_use]
    pub fn raven_defaults() -> Self {
        Self {
            kernel: GpKernel::raven_default(),
            alpha: 1e-10,
            normalize_y: false,
            optimizer: GpOptimizer::MaximiseLml,
        }
    }

    /// RAVEN's defaults with a named kernel.
    #[must_use]
    pub fn raven_with_kernel(kernel: GpKernel) -> Self {
        Self {
            kernel,
            ..Self::raven_defaults()
        }
    }
}

/// A fitted single-output Gaussian process.
#[derive(Debug, Clone, PartialEq)]
pub struct GaussianProcess {
    kernel: GpKernel,
    alpha_noise: f64,
    x_train: Vec<Vec<f64>>,
    l: Vec<Vec<f64>>,
    weights: Vec<f64>,
    y_mean: f64,
    y_std: f64,
    lml: f64,
    y_train: Vec<f64>,
}

fn forward(l: &[Vec<f64>], b: &[f64]) -> Vec<f64> {
    let n = b.len();
    let mut z = vec![0.0; n];
    for i in 0..n {
        let mut s = b[i];
        for k in 0..i {
            s -= l[i][k] * z[k];
        }
        z[i] = s / l[i][i];
    }
    z
}

fn backward(l: &[Vec<f64>], z: &[f64]) -> Vec<f64> {
    let n = z.len();
    let mut x = vec![0.0; n];
    for i in (0..n).rev() {
        let mut s = z[i];
        for k in i + 1..n {
            s -= l[k][i] * x[k];
        }
        x[i] = s / l[i][i];
    }
    x
}

/// numpy's population mean and std of a column, a zero std replaced by 1
/// (`_handle_zeros_in_scale`).
fn standardise(y: &[f64]) -> (f64, f64) {
    use super::polynomial_chaos::special::numpy_sum;
    let n = y.len() as f64;
    let mean = numpy_sum(y) / n;
    let sq: Vec<f64> = y.iter().map(|v| (v - mean) * (v - mean)).collect();
    let std = (numpy_sum(&sq) / n).sqrt();
    (mean, if std < 10.0 * f64::EPSILON { 1.0 } else { std })
}

/// The LML at `kernel`, and its gradient in log hyperparameters; `None`
/// when the Gram matrix is not positive definite (sklearn returns −∞).
fn lml_and_grad(
    kernel: &GpKernel,
    x: &[Vec<f64>],
    y: &[f64],
    alpha: f64,
) -> Option<(f64, Vec<f64>, Vec<Vec<f64>>, Vec<f64>)> {
    let n = x.len();
    let (mut k, grads) = kernel.gram(x);
    for (i, row) in k.iter_mut().enumerate() {
        row[i] += alpha;
    }
    let l = crate::bayesian::transitional::cholesky(&k, 0.0)?;
    let a = backward(&l, &forward(&l, y));
    let fit = y.iter().zip(&a).fold(0.0, |s, (u, v)| s + u * v);
    let logdet: Vec<f64> = (0..n).map(|i| l[i][i].r_ln()).collect();
    let lml = -0.5 * fit
        - super::polynomial_chaos::special::numpy_sum(&logdet)
        - n as f64 / 2.0 * (2.0 * core::f64::consts::PI).r_ln();
    let mut grad = Vec::with_capacity(grads.len());
    if !grads.is_empty() {
        // K⁻¹ column by column.
        let mut kinv = vec![vec![0.0; n]; n];
        for c in 0..n {
            let mut e = vec![0.0; n];
            e[c] = 1.0;
            let col = backward(&l, &forward(&l, &e));
            for r in 0..n {
                kinv[r][c] = col[r];
            }
        }
        for g in &grads {
            let mut s = 0.0;
            for i in 0..n {
                for j in 0..n {
                    s += (a[i] * a[j] - kinv[i][j]) * g[j][i];
                }
            }
            grad.push(0.5 * s);
        }
    }
    Some((lml, grad, l, a))
}

impl GaussianProcess {
    /// Fits one output.
    ///
    /// # Errors
    ///
    /// Mismatched or empty data, a non-finite value, or a Gram matrix that
    /// is not positive definite at the chosen hyperparameters (sklearn's
    /// "Try gradually increasing the 'alpha' parameter").
    pub fn fit(config: &GpConfig, x: &[Vec<f64>], y: &[f64]) -> Result<Self> {
        let n = x.len();
        if n == 0 || y.len() != n {
            return Err(RafflesError::DimensionMismatch {
                expected: n,
                found: y.len(),
            });
        }
        let d = x[0].len();
        let ls = config.kernel.length_scale.len();
        if x.iter().any(|r| r.len() != d) || (ls != 1 && ls != d) {
            return Err(RafflesError::DimensionMismatch {
                expected: d,
                found: ls,
            });
        }
        if x.iter().flatten().chain(y).any(|v| !v.is_finite()) {
            return Err(RafflesError::InvalidParameter {
                parameter: "training data".to_string(),
                value: f64::NAN,
                reason: "non-finite value".to_string(),
            });
        }
        let (y_mean, y_std) = if config.normalize_y {
            standardise(y)
        } else {
            (0.0, 1.0)
        };
        let yn: Vec<f64> = y.iter().map(|v| (v - y_mean) / y_std).collect();
        let mut kernel = config.kernel.clone();
        if config.optimizer == GpOptimizer::MaximiseLml && !kernel.theta().is_empty() {
            let theta = maximise(&kernel, x, &yn, config.alpha)?;
            kernel = kernel.with_theta(&theta)?;
        }
        let (lml, _, l, weights) = lml_and_grad(&kernel, x, &yn, config.alpha).ok_or(
            RafflesError::InvalidParameter {
                parameter: "alpha".to_string(),
                value: config.alpha,
                reason: "the kernel is not returning a positive definite matrix; try \
                         increasing alpha"
                    .to_string(),
            },
        )?;
        Ok(Self {
            kernel,
            alpha_noise: config.alpha,
            x_train: x.to_vec(),
            l,
            weights,
            y_mean,
            y_std,
            lml,
            y_train: yn,
        })
    }

    /// The fitted kernel.
    #[must_use]
    pub fn kernel(&self) -> &GpKernel {
        &self.kernel
    }

    /// The log marginal likelihood at the fitted hyperparameters
    /// (sklearn's `log_marginal_likelihood_value_`).
    #[must_use]
    pub fn log_marginal_likelihood(&self) -> f64 {
        self.lml
    }

    /// The LML and its gradient at another `theta` (log space), on the
    /// training data (sklearn's `log_marginal_likelihood(theta,
    /// eval_gradient=True)`); `None` where the Gram matrix fails.
    ///
    /// # Errors
    ///
    /// A wrong-length `theta`.
    pub fn log_marginal_likelihood_at(&self, theta: &[f64]) -> Result<Option<(f64, Vec<f64>)>> {
        let k = self.kernel.with_theta(theta)?;
        Ok(lml_and_grad(&k, &self.x_train, &self.y_train, self.alpha_noise)
            .map(|(v, g, _, _)| (v, g)))
    }

    /// Predictive mean and standard deviation at `x`.
    ///
    /// # Errors
    ///
    /// A wrong-length `x`.
    pub fn predict(&self, x: &[f64]) -> Result<(f64, f64)> {
        if x.len() != self.x_train[0].len() {
            return Err(RafflesError::DimensionMismatch {
                expected: self.x_train[0].len(),
                found: x.len(),
            });
        }
        let kt: Vec<f64> = self.x_train.iter().map(|r| self.kernel.cross(x, r)).collect();
        let m = kt.iter().zip(&self.weights).fold(0.0, |s, (a, b)| s + a * b);
        let mean = self.y_std * m + self.y_mean;
        let v = forward(&self.l, &kt);
        let mut var = self.kernel.diag() - v.iter().fold(0.0, |s, a| s + a * a);
        if var < 0.0 {
            var = 0.0;
        }
        Ok((mean, (var * (self.y_std * self.y_std)).sqrt()))
    }

    /// Leave-one-out residuals `y_i − μ_{−i}` at the fitted hyperparameters,
    /// in output units (Rasmussen and Williams 2006, eq. 5.12:
    /// `[K⁻¹y]_i / [K⁻¹]_ii`).
    #[must_use]
    pub fn loo_residuals(&self) -> Vec<f64> {
        let n = self.x_train.len();
        (0..n)
            .map(|i| {
                let mut e = vec![0.0; n];
                e[i] = 1.0;
                let col = backward(&self.l, &forward(&self.l, &e));
                self.weights[i] / col[i] * self.y_std
            })
            .collect()
    }
}

/// Maximises the LML over the free log hyperparameters from the kernel's
/// starting point: BFGS projected onto the bounds, Armijo backtracking.
/// Not a port of L-BFGS-B (module notes).
fn maximise(kernel: &GpKernel, x: &[Vec<f64>], y: &[f64], alpha: f64) -> Result<Vec<f64>> {
    let bounds = kernel.bounds();
    let eval = |t: &[f64]| -> Option<(f64, Vec<f64>)> {
        let k = kernel.with_theta(t).ok()?;
        lml_and_grad(&k, x, y, alpha).map(|(v, g, _, _)| (-v, g.iter().map(|e| -e).collect()))
    };
    bounded_bfgs(kernel.theta(), &bounds, &eval).ok_or(RafflesError::InvalidParameter {
        parameter: "kernel".to_string(),
        value: f64::NAN,
        reason: "the starting hyperparameters give a non positive definite Gram matrix"
            .to_string(),
    })
}

/// Minimises `eval` (value and gradient; `None` where undefined) from `t0`
/// within `bounds`: BFGS projected onto the bounds, Armijo backtracking, a
/// steepest-descent restart when a line search fails. This crate's own
/// optimiser, not a port of L-BFGS-B or TNC. Shared by the RAVEN/sklearn GP
/// route (gh:#1193) and the SMT kriging routes (gh:#1190); `None` if `eval`
/// fails at the (projected) start.
pub(crate) fn bounded_bfgs<F>(t0: Vec<f64>, bounds: &[(f64, f64)], eval: &F) -> Option<Vec<f64>>
where
    F: Fn(&[f64]) -> Option<(f64, Vec<f64>)>,
{
    let m = bounds.len();
    let project = |t: &mut Vec<f64>| {
        for (v, (lo, hi)) in t.iter_mut().zip(bounds) {
            *v = v.clamp(*lo, *hi);
        }
    };
    let mut t = t0;
    project(&mut t);
    let (mut f, mut g) = eval(&t)?;
    let mut h = vec![vec![0.0; m]; m];
    for (i, row) in h.iter_mut().enumerate() {
        row[i] = 1.0;
    }
    let mut reset = false;
    for _ in 0..2000 {
        // Projected gradient norm (free components only).
        let pg: f64 = t
            .iter()
            .zip(&g)
            .zip(bounds)
            .map(|((v, gi), (lo, hi))| {
                let p = (v - gi).clamp(*lo, *hi) - v;
                p.abs()
            })
            .fold(0.0, f64::max);
        if pg < 1e-9 {
            break;
        }
        // Direction, with variables at an active bound frozen.
        let active: Vec<bool> = t
            .iter()
            .zip(&g)
            .zip(bounds)
            .map(|((v, gi), (lo, hi))| (*v <= *lo && *gi > 0.0) || (*v >= *hi && *gi < 0.0))
            .collect();
        let mut p: Vec<f64> = (0..m)
            .map(|i| {
                if active[i] {
                    0.0
                } else {
                    -(0..m).filter(|j| !active[*j]).map(|j| h[i][j] * g[j]).sum::<f64>()
                }
            })
            .collect();
        let mut slope: f64 = p.iter().zip(&g).map(|(a, b)| a * b).sum();
        if !(slope < 0.0) {
            for i in 0..m {
                p[i] = if active[i] { 0.0 } else { -g[i] };
                h[i] = (0..m).map(|j| if i == j { 1.0 } else { 0.0 }).collect();
            }
            slope = p.iter().zip(&g).map(|(a, b)| a * b).sum();
        }
        let mut step = 1.0;
        let mut accepted = None;
        for _ in 0..60 {
            let mut tn: Vec<f64> = t.iter().zip(&p).map(|(a, b)| a + step * b).collect();
            project(&mut tn);
            if let Some((fnew, gnew)) = eval(&tn) {
                let moved: f64 = tn.iter().zip(&t).map(|(a, b)| (a - b).abs()).sum();
                if fnew <= f + 1e-4 * step * slope.min(0.0) || moved == 0.0 {
                    accepted = Some((tn, fnew, gnew));
                    break;
                }
            }
            step *= 0.5;
        }
        let Some((tn, fnew, gnew)) = accepted else {
            if reset {
                break;
            }
            // Line search failed on a quasi-Newton direction: restart from
            // steepest descent once before giving up.
            reset = true;
            for (i, row) in h.iter_mut().enumerate() {
                *row = (0..m).map(|j| if i == j { 1.0 } else { 0.0 }).collect();
            }
            continue;
        };
        reset = false;
        let s: Vec<f64> = tn.iter().zip(&t).map(|(a, b)| a - b).collect();
        let yv: Vec<f64> = gnew.iter().zip(&g).map(|(a, b)| a - b).collect();
        let sy: f64 = s.iter().zip(&yv).map(|(a, b)| a * b).sum();
        let done = s.iter().all(|v| *v == 0.0);
        t = tn;
        f = fnew;
        g = gnew;
        if sy > 1e-12 {
            // BFGS inverse-Hessian update.
            let hy: Vec<f64> = (0..m).map(|i| (0..m).map(|j| h[i][j] * yv[j]).sum()).collect();
            let yhy: f64 = yv.iter().zip(&hy).map(|(a, b)| a * b).sum();
            for i in 0..m {
                for j in 0..m {
                    h[i][j] += (sy + yhy) * s[i] * s[j] / (sy * sy) - (hy[i] * s[j] + s[i] * hy[j]) / sy;
                }
            }
        }
        if done {
            break;
        }
    }
    Some(t)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn line() -> (Vec<Vec<f64>>, Vec<f64>) {
        let x: Vec<Vec<f64>> = [0.0, 0.45, 1.1, 1.6, 2.2, 2.9, 3.4, 4.0]
            .iter()
            .map(|v| vec![*v])
            .collect();
        let y = x.iter().map(|r| (2.0 * r[0]).sin() + 0.1 * r[0]).collect();
        (x, y)
    }

    /// **A5, noise-free interpolation** (V&V record A5, predicted before the
    /// run: `abs(mean − y)` ≤ 1e-6 · max abs(y) at every training point, std
    /// ≤ 1e-3), at RAVEN's default and with a fitted RBF; and the
    /// closed-form LOO residuals against brute-force refits.
    #[test]
    fn gp_interpolates_noise_free_data_and_loo_matches_refits() {
        let (x, y) = line();
        let ymax = y.iter().fold(0.0_f64, |a, b| a.max(b.abs()));
        for cfg in [
            GpConfig::raven_defaults(),
            GpConfig::raven_with_kernel(GpKernel::raven(RavenKernel::Rbf, 1, false)),
            GpConfig::raven_with_kernel(GpKernel::raven(RavenKernel::ConstantMatern, 1, true)),
        ] {
            let gp = GaussianProcess::fit(&cfg, &x, &y).unwrap();
            let mut worst: f64 = 0.0;
            let mut sd_worst: f64 = 0.0;
            for (xi, yi) in x.iter().zip(&y) {
                let (m, s) = gp.predict(xi).unwrap();
                worst = worst.max((m - yi).abs() / ymax);
                sd_worst = sd_worst.max(s);
            }
            eprintln!("A5 {:?}: worst rel residual {worst:.3e}, worst std {sd_worst:.3e}", cfg.kernel.base);
            assert!(worst <= 1e-6 && sd_worst <= 1e-3);
            // LOO closed form = refit without row i at fixed hyperparameters.
            let fixed = GpConfig {
                kernel: gp.kernel().clone(),
                optimizer: GpOptimizer::None,
                ..cfg.clone()
            };
            let loo = gp.loo_residuals();
            for i in [0usize, 3, 7] {
                let xr: Vec<Vec<f64>> = x.iter().enumerate().filter(|(j, _)| *j != i).map(|(_, r)| r.clone()).collect();
                let yr: Vec<f64> = y.iter().enumerate().filter(|(j, _)| *j != i).map(|(_, v)| *v).collect();
                let g = GaussianProcess::fit(&fixed, &xr, &yr).unwrap();
                let (m, _) = g.predict(&x[i]).unwrap();
                assert!((loo[i] - (y[i] - m)).abs() < 1e-6 * ymax, "LOO row {i}");
            }
        }
    }

    /// The LML gradient against central differences, for every kernel shape
    /// and both length-scale forms, and the kernel's theta round trip.
    #[test]
    fn lml_gradient_matches_finite_differences() {
        let x: Vec<Vec<f64>> = (0..9).map(|i| vec![0.4 * i as f64, (0.7 * i as f64).cos()]).collect();
        let y: Vec<f64> = x.iter().map(|r| r[0].sin() + r[1]).collect();
        for name in [
            RavenKernel::Rbf,
            RavenKernel::Matern,
            RavenKernel::ConstantRbf,
            RavenKernel::ConstantMatern,
            RavenKernel::RbfWhite,
            RavenKernel::ConstantRbfWhite,
        ] {
            for aniso in [false, true] {
                for base in [None, Some(BaseKernel::Matern12), Some(BaseKernel::Matern32)] {
                    let mut k = GpKernel::raven(name, 2, aniso);
                    if let Some(b) = base {
                        k.base = b;
                    }
                    if let Some(w) = k.white.as_mut() {
                        w.value = 0.05;
                    }
                    let theta: Vec<f64> = k.theta().iter().enumerate().map(|(i, _)| 0.1 * i as f64 - 0.2).collect();
                    let cfg = GpConfig {
                        kernel: k.with_theta(&theta).unwrap(),
                        alpha: 1e-6,
                        normalize_y: true,
                        optimizer: GpOptimizer::None,
                    };
                    let gp = GaussianProcess::fit(&cfg, &x, &y).unwrap();
                    assert_eq!(gp.kernel().theta().len(), theta.len());
                    let (_, g) = gp.log_marginal_likelihood_at(&theta).unwrap().unwrap();
                    for p in 0..theta.len() {
                        let h = 1e-6;
                        let mut tp = theta.clone();
                        tp[p] += h;
                        let mut tm = theta.clone();
                        tm[p] -= h;
                        let fp = gp.log_marginal_likelihood_at(&tp).unwrap().unwrap().0;
                        let fm = gp.log_marginal_likelihood_at(&tm).unwrap().unwrap().0;
                        let fd = (fp - fm) / (2.0 * h);
                        assert!((fd - g[p]).abs() < 1e-5 * (1.0 + g[p].abs()), "{name:?} {aniso} {base:?} p{p}: {fd} vs {}", g[p]);
                    }
                }
            }
        }
        assert!(GpKernel::raven_default().with_theta(&[1.0]).is_err());
    }

    /// Bad input is refused, and a non-positive-definite Gram is reported.
    #[test]
    fn gp_refuses_bad_input() {
        let (x, y) = line();
        let cfg = GpConfig::raven_defaults();
        assert!(GaussianProcess::fit(&cfg, &x, &y[1..]).is_err());
        assert!(GaussianProcess::fit(&cfg, &[], &[]).is_err());
        let mut bad = y.clone();
        bad[0] = f64::NAN;
        assert!(GaussianProcess::fit(&cfg, &x, &bad).is_err());
        let gp = GaussianProcess::fit(&cfg, &x, &y).unwrap();
        assert!(gp.predict(&[0.0, 1.0]).is_err());
        // Duplicate rows with alpha 0: singular.
        let dup = vec![vec![1.0], vec![1.0]];
        let cfg0 = GpConfig { alpha: 0.0, ..cfg };
        assert!(GaussianProcess::fit(&cfg0, &dup, &[1.0, 1.0]).is_err());
        assert_eq!(standardise(&[2.0, 2.0]).1, 1.0);
    }
}
