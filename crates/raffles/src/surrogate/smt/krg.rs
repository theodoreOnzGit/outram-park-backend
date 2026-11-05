// ---------------------------------------------------------------------------
// Ported from SMT, the Surrogate Modeling Toolbox.
//
//   Upstream repo:    https://github.com/SMTorg/smt
//   Upstream files:   smt/surrogate_models/krg_based/krg_based.py
//                       (KrgBased: options and defaults, _new_train,
//                        _prepare_training_data, _optimize_hyperparam's start
//                        and bound handling, _predict_values,
//                        _predict_variances)
//                     smt/surrogate_models/krg.py      (KRG)
//                     smt/surrogate_models/kpls.py     (KPLS, fixed n_comp)
//                     smt/surrogate_models/kplsk.py    (KPLSK, two passes)
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
//   * KRG, KPLS and KPLSK are one struct, `SmtKriging`, with `SmtPls` saying
//     which (SMT's subclass hooks `_use_pls`, `_is_kplsk_style` become a
//     match). One output per model, as SMT's KRG (`ny = 1`).
//   * Kept faithfully: SMT's variance for universal kriging, which solves
//     with only the diagonal of G (`solve_triangular(G.T, ...)` without
//     `lower=True`, V&V record finding 1): `TrendVariance::AsSmt`, the
//     default. `TrendVariance::Exact` is the full lower-triangular solve,
//     offered beside it, not in place of it.
//   * A theta0 outside the bounds is an error here (SMT draws a random
//     replacement for KRG/KPLS); KPLSK's second pass clamps to the bound
//     -/+ 1e-10 as SMT does.
//   * Not ported: `eval_n_comp` (Wold / exhaustive PRESS selection of
//     n_comp), noise estimation, derivatives, mixed-integer design spaces,
//     `update_model`.
// ---------------------------------------------------------------------------

//! **SMT's KRG, KPLS and KPLSK** (gh:#1190): ordinary / universal kriging
//! with SMT's kernels, trends, nugget, θ bounds and normalisation, and its
//! partial-least-squares reductions. One fitted model per output.

use petir::mathf::RealMath;

use super::core::{maximise, Level, LevelPar, Pairs, Projection, SmtCorrelation, SmtOptimizer, SmtRegression, SmtStarts};
use super::linalg::{columns, dot, forward, forward_ut};
use crate::{RafflesError, Result};

/// Which partial-least-squares reduction, if any.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SmtPls {
    /// KRG: one θ per input.
    None,
    /// KPLS: one θ per PLS component (`squar_exp` or `abs_exp`).
    Kpls {
        /// Number of components (SMT's `n_comp`, default 1).
        n_comp: usize,
    },
    /// KPLSK: KPLS's optimum projected to one θ per input, then a full KRG
    /// optimisation from it (`squar_exp` only).
    Kplsk {
        /// Number of components of the first pass.
        n_comp: usize,
    },
}

/// The trend term of the predictive variance.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TrendVariance {
    /// SMT's KRG: `u = diag(G)⁻¹ (Ftᵀ rt − f)`, exact for a constant trend,
    /// not for `linear` / `quadratic` (V&V record, finding 1).
    AsSmt,
    /// `u = G⁻ᵀ (Ftᵀ rt − f)`, the universal-kriging variance.
    Exact,
}

/// SMT's KRG-family options.
#[derive(Debug, Clone, PartialEq)]
pub struct SmtKrgConfig {
    /// `corr` (default `squar_exp`).
    pub corr: SmtCorrelation,
    /// `poly` (default `constant`).
    pub poly: SmtRegression,
    /// `theta0` (default `[1e-2]`, broadcast); with [`SmtOptimizer::None`]
    /// the θ used, exactly.
    pub theta0: Vec<f64>,
    /// `theta_bounds` (default `[1e-6, 20]`).
    pub theta_bounds: (f64, f64),
    /// `nugget` (default `100 ε`).
    pub nugget: f64,
    /// Optimise or keep θ0.
    pub optimizer: SmtOptimizer,
    /// Multistart points.
    pub starts: SmtStarts,
    /// KRG, KPLS or KPLSK.
    pub pls: SmtPls,
    /// The variance's trend term.
    pub trend_variance: TrendVariance,
}

impl SmtKrgConfig {
    /// SMT's `KRG()` defaults.
    #[must_use]
    pub fn smt_defaults() -> Self {
        Self {
            corr: SmtCorrelation::SquarExp,
            poly: SmtRegression::Constant,
            theta0: vec![1e-2],
            theta_bounds: (1e-6, 20.0),
            nugget: 100.0 * f64::EPSILON,
            optimizer: SmtOptimizer::MaximiseLikelihood,
            starts: SmtStarts::smt_default(),
            pls: SmtPls::None,
            trend_variance: TrendVariance::AsSmt,
        }
    }

    /// The defaults at a fixed θ (SMT's `hyper_opt = "NoOp"`).
    #[must_use]
    pub fn at_theta(theta: Vec<f64>) -> Self {
        Self {
            theta0: theta,
            optimizer: SmtOptimizer::None,
            ..Self::smt_defaults()
        }
    }
}

/// A fitted SMT kriging model of one output.
#[derive(Debug, Clone, PartialEq)]
pub struct SmtKriging {
    corr: SmtCorrelation,
    poly: SmtRegression,
    trend_variance: TrendVariance,
    x_offset: Vec<f64>,
    x_scale: Vec<f64>,
    y_mean: f64,
    y_std: f64,
    x_norma: Vec<Vec<f64>>,
    projection: Projection,
    theta: Vec<f64>,
    rlf: f64,
    par: LevelPar,
    level: Level,
    coeff_pls: Option<Vec<Vec<f64>>>,
}

/// Broadcasts a length-1 θ0 to `m`.
pub(crate) fn broadcast(theta0: &[f64], m: usize) -> Result<Vec<f64>> {
    match theta0.len() {
        1 => Ok(vec![theta0[0]; m]),
        l if l == m => Ok(theta0.to_vec()),
        l => Err(RafflesError::DimensionMismatch { expected: m, found: l }),
    }
}

/// Checks θ0 against the bounds (an error, where SMT re-draws at random).
pub(crate) fn check_in_bounds(theta0: &[f64], b: (f64, f64)) -> Result<()> {
    match theta0.iter().find(|t| !(**t >= b.0 && **t <= b.1)) {
        Some(t) => Err(RafflesError::InvalidParameter {
            parameter: "theta0".to_string(),
            value: *t,
            reason: format!("outside theta_bounds [{}, {}]", b.0, b.1),
        }),
        None => Ok(()),
    }
}

/// Fits θ on `level` per `optimizer`/`starts`, returning θ, the likelihood
/// and the GLS parameters.
pub(crate) fn fit_level(level: &Level, theta0: &[f64], bounds: (f64, f64), optimizer: SmtOptimizer, starts: &SmtStarts, lhs: bool) -> Result<(Vec<f64>, f64, LevelPar)> {
    let theta = match optimizer {
        SmtOptimizer::None => theta0.to_vec(),
        SmtOptimizer::MaximiseLikelihood => {
            let lb = (bounds.0.r_log10(), bounds.1.r_log10());
            let t0: Vec<f64> = theta0.iter().map(|t| t.r_log10()).collect();
            let rows = starts.rows(&t0, lb, lhs)?;
            maximise(level, &rows, lb)?.ok_or(RafflesError::InvalidParameter {
                parameter: "nugget".to_string(),
                value: level.nugget,
                reason: "every start gives a non positive definite correlation matrix; try increasing the nugget".to_string(),
            })?
        }
    };
    let (rlf, par) = level.evaluate(&theta)?.ok_or(RafflesError::InvalidParameter {
        parameter: "theta".to_string(),
        value: f64::NAN,
        reason: "the reduced likelihood is -inf at theta (correlation matrix not positive definite, or G ill-conditioned)".to_string(),
    })?;
    Ok((theta, rlf, par))
}

/// Standardises `x` with `offset`/`scale`.
pub(crate) fn normalise(x: &[f64], offset: &[f64], scale: &[f64]) -> Vec<f64> {
    x.iter().zip(offset).zip(scale).map(|((v, o), s)| (v - o) / s).collect()
}

impl SmtKriging {
    /// Fits one output `y` on inputs `x` (`n × nx`), as SMT's `train()`.
    ///
    /// # Errors
    /// Mismatched or empty data, a non-finite value, too few rows for the
    /// trend, an invalid θ0, a PLS that cannot be formed, or a likelihood that
    /// is `−inf` everywhere.
    pub fn fit(config: &SmtKrgConfig, x: &[Vec<f64>], y: &[f64]) -> Result<Self> {
        let n = x.len();
        if n == 0 || y.len() != n {
            return Err(RafflesError::DimensionMismatch { expected: n, found: y.len() });
        }
        let nx = x[0].len();
        if x.iter().any(|r| r.len() != nx) {
            return Err(RafflesError::DimensionMismatch { expected: nx, found: 0 });
        }
        if x.iter().flatten().chain(y).any(|v| !v.is_finite()) {
            return Err(RafflesError::InvalidParameter {
                parameter: "training data".to_string(),
                value: f64::NAN,
                reason: "non-finite value".to_string(),
            });
        }
        let (n_comp, pls) = match config.pls {
            SmtPls::None => (nx, false),
            SmtPls::Kpls { n_comp } => (n_comp, true),
            SmtPls::Kplsk { n_comp } => (n_comp, true),
        };
        if matches!(config.pls, SmtPls::Kplsk { .. }) && config.corr != SmtCorrelation::SquarExp
            || matches!(config.pls, SmtPls::Kpls { .. }) && !matches!(config.corr, SmtCorrelation::SquarExp | SmtCorrelation::AbsExp)
        {
            return Err(RafflesError::InvalidParameter {
                parameter: "corr".to_string(),
                value: f64::NAN,
                reason: "KPLS takes squar_exp or abs_exp, KPLSK squar_exp only (SMT)".to_string(),
            });
        }
        let coeff_pls = if pls {
            if nx == 1 {
                Some(vec![vec![1.0]])
            } else {
                Some(super::pls::x_rotations(x, y, n_comp)?.into_iter().map(|r| r.into_iter().map(f64::abs).collect()).collect())
            }
        } else {
            None
        };
        let (x_offset, x_scale, y_mean, y_std) = super::core::standardization(x, y);
        let x_norma: Vec<Vec<f64>> = x.iter().map(|r| normalise(r, &x_offset, &x_scale)).collect();
        let y_norma: Vec<f64> = y.iter().map(|v| (v - y_mean) / y_std).collect();
        let f: Vec<Vec<f64>> = x_norma.iter().map(|r| config.poly.basis(r)).collect();
        let p = f[0].len();
        if p > n {
            return Err(RafflesError::InvalidParameter {
                parameter: "poly".to_string(),
                value: p as f64,
                reason: format!("ordinary least squares is undetermined: n_samples = {n} must be at least the regression size p = {p}"),
            });
        }
        let projection = match &coeff_pls {
            Some(c) => Projection::Pls(c.clone()),
            None => Projection::Full,
        };
        let theta0 = broadcast(&config.theta0, n_comp)?;
        check_in_bounds(&theta0, config.theta_bounds)?;
        let mut level = Level {
            pairs: Pairs::new(&x_norma, config.corr, &projection),
            f,
            y: y_norma,
            // KRG's `_get_pq()` is (0, 0): its likelihood and σ² divide by n,
            // not n − p (only MFK subtracts p + q; V&V record, finding 7).
            p: 0,
            q: 0,
            y_std,
            nugget: config.nugget,
            corr: config.corr,
        };
        let (mut theta, mut rlf, mut par) = fit_level(&level, &theta0, config.theta_bounds, config.optimizer, &config.starts, true)?;
        let mut projection = projection;
        if let (SmtPls::Kplsk { .. }, Some(coeff)) = (config.pls, &coeff_pls) {
            // Second pass: θ_full,i = Σ_l θ_l coeff_il², clamped into the
            // bounds by 1e-10, then a full KRG optimisation from it (θ0 and
            // one uniform draw, no LHS; SMT's maxfun has no counterpart).
            let b = config.theta_bounds;
            let full: Vec<f64> = coeff
                .iter()
                .map(|row| {
                    let t: Vec<f64> = theta.iter().zip(row).map(|(a, c)| a * (c * c)).collect();
                    let s = crate::surrogate::polynomial_chaos::special::pairwise_sum(&t);
                    if s > b.1 {
                        b.1 - 1e-10
                    } else if s < b.0 {
                        b.0 + 1e-10
                    } else {
                        s
                    }
                })
                .collect();
            projection = Projection::Full;
            level.pairs = Pairs::new(&x_norma, config.corr, &projection);
            (theta, rlf, par) = fit_level(&level, &full, b, config.optimizer, &config.starts, false)?;
        }
        Ok(Self {
            corr: config.corr,
            poly: config.poly,
            trend_variance: config.trend_variance,
            x_offset,
            x_scale,
            y_mean,
            y_std,
            x_norma,
            projection,
            theta,
            rlf,
            par,
            level,
            coeff_pls,
        })
    }

    /// The hyperparameters (SMT's `optimal_theta`).
    #[must_use]
    pub fn theta(&self) -> &[f64] {
        &self.theta
    }

    /// The reduced log10 likelihood at [`theta`](Self::theta)
    /// (`optimal_rlf_value`).
    #[must_use]
    pub fn reduced_likelihood(&self) -> f64 {
        self.rlf
    }

    /// σ² in output units, β, γ (`optimal_par`).
    #[must_use]
    pub fn gls(&self) -> (f64, &[f64], &[f64]) {
        (self.par.sigma2, &self.par.beta, &self.par.gamma)
    }

    /// `(X_offset, X_scale, y_mean, y_std)`.
    #[must_use]
    pub fn standardisation(&self) -> (&[f64], &[f64], f64, f64) {
        (&self.x_offset, &self.x_scale, self.y_mean, self.y_std)
    }

    /// KPLS's `coeff_pls` (`|x_rotations_|`), if a PLS route.
    #[must_use]
    pub fn coeff_pls(&self) -> Option<&[Vec<f64>]> {
        self.coeff_pls.as_deref()
    }

    fn correlations(&self, xn: &[f64]) -> Vec<f64> {
        self.x_norma
            .iter()
            .map(|t| {
                let dx: Vec<f64> = xn.iter().zip(t).map(|(a, b)| a - b).collect();
                self.corr.value(&self.theta, &self.projection.distances(self.corr, &dx))
            })
            .collect()
    }

    /// The predictive mean and variance (SMT's `predict_values`,
    /// `predict_variances`; the variance clipped at 0).
    ///
    /// # Errors
    /// A wrong-length `x`.
    pub fn predict(&self, x: &[f64]) -> Result<(f64, f64)> {
        if x.len() != self.x_offset.len() {
            return Err(RafflesError::DimensionMismatch { expected: self.x_offset.len(), found: x.len() });
        }
        let xn = normalise(x, &self.x_offset, &self.x_scale);
        let r = self.correlations(&xn);
        let f = self.poly.basis(&xn);
        let y_ = dot(&f, &self.par.beta) + dot(&r, &self.par.gamma);
        let mean = self.y_mean + self.y_std * y_;
        let rt = forward(&self.par.c, &r);
        let ftc = columns(&self.par.ft);
        let rhs: Vec<f64> = ftc.iter().zip(&f).map(|(col, fv)| dot(col, &rt) - fv).collect();
        let u = match self.trend_variance {
            TrendVariance::AsSmt => rhs.iter().enumerate().map(|(i, v)| v / self.par.g[i][i]).collect::<Vec<f64>>(),
            TrendVariance::Exact => forward_ut(&self.par.g, &rhs),
        };
        let mut rt2 = 0.0;
        for v in &rt {
            rt2 += v * v;
        }
        let mut u2 = 0.0;
        for v in &u {
            u2 += v * v;
        }
        let b = 1.0 - rt2 + u2;
        let mut s2 = self.par.sigma2 * b;
        if s2 < 0.0 {
            s2 = 0.0;
        }
        Ok((mean, s2))
    }

    /// The reduced likelihood and its gradient at another θ, on the training
    /// data (SMT's `_reduced_likelihood_gradient`), in the model's final
    /// projection; `None` where it is `−inf`.
    ///
    /// # Errors
    /// A wrong-length θ; ill-conditioned F.
    pub fn reduced_likelihood_gradient_at(&self, theta: &[f64]) -> Result<Option<(f64, Vec<f64>)>> {
        if theta.len() != self.theta.len() {
            return Err(RafflesError::DimensionMismatch { expected: self.theta.len(), found: theta.len() });
        }
        self.level.gradient(theta)
    }
}
