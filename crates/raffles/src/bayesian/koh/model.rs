// ---------------------------------------------------------------------------
// Ported from SEPIA (Simulation-Enabled Prediction, Inference, and Analysis).
//
//   Upstream project: SEPIA — Los Alamos National Laboratory
//   Upstream repo:    https://github.com/lanl/SEPIA
//   Upstream files:   sepia/SepiaDistCov.py (compute_cov_mat)
//                     sepia/SepiaLogLik.py (compute_log_lik)
//                     sepia/SepiaPrior.py (compute_log_prior, is_in_bounds)
//                     sepia/SepiaModel.py (set_params_sim_only / _noD / _full)
//                     sepia/SepiaPredict.py (uvPred)
//   Upstream commit:  309890423a121919dc09a678c65a21748bbefb4b  (master)
//   Accessed:         2026-11-04
//
//   (c) 2020. Triad National Security, LLC. All rights reserved.
//   Licensed under the BSD 3-Clause License; the full text is preserved
//   verbatim in crates/raffles/LICENSE-BSD-SEPIA.
//
// This Rust translation is part of RAFFLES / Outram Park and is distributed
// under AGPL-3.0-only. BSD-3 -> AGPLv3 is ONE-WAY: this file may not be
// contributed back to SEPIA or redistributed under BSD terms.
//
// Translation notes:
//   * Scalar output only, pu = 1, LamSim = 1, SigObs = I / (1 + 1e-8) (what
//     SepiaModel computes for scalar output with no Sigy). The three model
//     forms are the enum `KohForm`.
//   * The likelihood keeps SEPIA's W / (VU | W) factorisation, its additions
//     in its order, and np.linalg.inv for SigWi (here through a Cholesky,
//     so it agrees to round-off, not bit for bit).
//   * The discrepancy branch is the GPMSA scalar branch SEPIA's likelihood
//     and dense predictor carry but its setup cannot reach (V&V record,
//     "Upstream findings"); the predictor follows upstream's own comment
//     "SigCross=[SigVVx SigUUx; zeros SigWUx]", correcting the defect at
//     SepiaPredict.py line 704.
//   * SEPIA's incremental recomputation (`cvar`) is not ported: every call
//     computes everything. `EmulatorFactor` instead caches the W block for
//     a caller holding the emulator's hyperparameters fixed.
//   * Not ported: multivariate output, mean basis, separable designs,
//     categorical inputs, hierarchical / shared theta, SEPIA's MCMC.
// ---------------------------------------------------------------------------

//! SEPIA's scalar-output Kennedy–O'Hagan model: parameters, priors, the
//! squared-exponential covariance, the log-likelihood, the log prior and the
//! predictive distribution of (δ, η) at new x.

use petir::mathf::RealMath;
use super::data::{pairwise_sum, KohData};
use crate::bayesian::transitional::cholesky;
use crate::{RafflesError, Result};

/// Which SEPIA model.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum KohForm {
    /// The emulator alone (`sim_only`): β_U, λ_Uz, λ_Ws, λ_WOs.
    SimulationOnly,
    /// Calibration without discrepancy (SEPIA's supported univariate model,
    /// `set_params_noD`): adds θ and λ_Os.
    NoDiscrepancy,
    /// Kennedy–O'Hagan with a GP discrepancy δ(x) (`set_params_full`): adds
    /// β_V and λ_Vz.
    WithDiscrepancy,
}

/// SEPIA's parameters for a scalar model, on SEPIA's scales: θ on the unit
/// cube, β the squared-exponential coefficients (ρ = exp(−β/4)), λ
/// precisions on the standardised output.
#[derive(Debug, Clone, PartialEq)]
pub struct KohParams {
    /// θ on the unit scale, length q (empty for a simulation-only model).
    pub theta: Vec<f64>,
    /// Emulator β, length p + q.
    pub beta_u: Vec<f64>,
    /// Emulator marginal precision.
    pub lam_uz: f64,
    /// Emulator nugget precision.
    pub lam_ws: f64,
    /// Simulator-residual precision.
    pub lam_wos: f64,
    /// Discrepancy β, length p (empty without discrepancy).
    pub beta_v: Vec<f64>,
    /// Discrepancy marginal precision.
    pub lam_vz: f64,
    /// Observation precision.
    pub lam_os: f64,
}

impl KohParams {
    /// SEPIA's initial values (`SepiaParam(val=...)` in `set_params_*`):
    /// θ 0.5, β 0.1, λ_Uz 1, λ_Ws 1000, λ_WOs 1000, λ_Vz 20, λ_Os 1000 (for
    /// scalar output `max(20, a/b)` and `max(100, a/b)` both give 1000).
    #[must_use]
    pub fn sepia_init(form: KohForm, p: usize, q: usize) -> Self {
        let with_t = form != KohForm::SimulationOnly;
        Self {
            theta: if with_t { vec![0.5; q] } else { Vec::new() },
            beta_u: vec![0.1; p + q],
            lam_uz: 1.0,
            lam_ws: 1000.0,
            lam_wos: 1000.0,
            beta_v: if form == KohForm::WithDiscrepancy { vec![0.1; p] } else { Vec::new() },
            lam_vz: 20.0,
            lam_os: 1000.0,
        }
    }
}

/// A SEPIA prior family (`SepiaPrior.dist`). Its log density is SEPIA's,
/// **unnormalised**.
#[derive(Debug, Clone, PartialEq)]
pub enum PriorDist {
    /// `−½ Σ ((x − mean)/sd)²`, per element.
    Normal { mean: Vec<f64>, sd: Vec<f64> },
    /// `Σ (a − 1) ln x − b x`.
    Gamma { a: f64, b: f64 },
    /// On ρ = exp(−x/4), capped at 0.999: `Σ (a − 1) ln ρ + (b − 1) ln(1 − ρ)`.
    BetaRho { a: f64, b: f64 },
    /// 0 inside the bounds.
    Uniform,
}

/// One parameter's prior with SEPIA's open bounds `lower < x < upper`.
#[derive(Debug, Clone, PartialEq)]
pub struct ParamPrior {
    /// The family.
    pub dist: PriorDist,
    /// Lower bound (strict).
    pub lower: f64,
    /// Upper bound (strict).
    pub upper: f64,
}

impl ParamPrior {
    /// `compute_log_prior` for one parameter's values `x`.
    #[must_use]
    pub fn log_prior(&self, x: &[f64]) -> f64 {
        if !x.iter().all(|v| *v > self.lower && *v < self.upper) {
            return f64::NEG_INFINITY;
        }
        let mut lp = 0.0;
        match &self.dist {
            PriorDist::Normal { mean, sd } => {
                for (i, v) in x.iter().enumerate() {
                    let z = (v - mean[i]) / sd[i];
                    lp += z * z;
                }
                -0.5 * lp
            }
            PriorDist::Gamma { a, b } => {
                for v in x {
                    lp += (a - 1.0) * v.r_ln() - b * v;
                }
                lp
            }
            PriorDist::BetaRho { a, b } => {
                for v in x {
                    let rho = (-0.25 * v).r_exp().min(0.999);
                    lp += (a - 1.0) * rho.r_ln() + (b - 1.0) * (1.0 - rho).r_ln();
                }
                lp
            }
            PriorDist::Uniform => 0.0,
        }
    }
}

/// Every parameter's prior.
#[derive(Debug, Clone, PartialEq)]
pub struct KohPriors {
    /// θ (unit scale).
    pub theta: ParamPrior,
    /// Emulator β.
    pub beta_u: ParamPrior,
    /// λ_Uz.
    pub lam_uz: ParamPrior,
    /// λ_Ws.
    pub lam_ws: ParamPrior,
    /// λ_WOs.
    pub lam_wos: ParamPrior,
    /// Discrepancy β.
    pub beta_v: ParamPrior,
    /// λ_Vz.
    pub lam_vz: ParamPrior,
    /// λ_Os.
    pub lam_os: ParamPrior,
}

impl KohPriors {
    /// SEPIA's defaults for scalar output (`set_params_sim_only`, `_noD`,
    /// `_full` with every prior correction zero): θ N(0.5, 10²) on (0, 1);
    /// β Beta(1, 0.1) on ρ, β in (0, ∞); λ_Uz Gamma(5, 5) on (0.3, ∞); λ_Ws
    /// Gamma(3, 3e-3) and λ_WOs Gamma(5, 5e-3) on (60, 1e5); λ_Vz and λ_Os
    /// Gamma(1, 1e-3) on (0, ∞).
    #[must_use]
    pub fn sepia_defaults(q: usize) -> Self {
        let g = |a: f64, b: f64, lower: f64, upper: f64| ParamPrior { dist: PriorDist::Gamma { a, b }, lower, upper };
        let rho = ParamPrior { dist: PriorDist::BetaRho { a: 1.0, b: 0.1 }, lower: 0.0, upper: f64::INFINITY };
        Self {
            theta: ParamPrior { dist: PriorDist::Normal { mean: vec![0.5; q], sd: vec![10.0; q] }, lower: 0.0, upper: 1.0 },
            beta_u: rho.clone(),
            lam_uz: g(5.0, 5.0, 0.3, f64::INFINITY),
            lam_ws: g(3.0, 3e-3, 60.0, 1e5),
            lam_wos: g(5.0, 5e-3, 60.0, 1e5),
            beta_v: rho,
            lam_vz: g(1.0, 1e-3, 0.0, f64::INFINITY),
            lam_os: g(1.0, 1e-3, 0.0, f64::INFINITY),
        }
    }
}

/// `SepiaDistCov(a).compute_cov_mat(beta, lamz)`: `exp(−Σ_k β_k d_k²)/λ` off
/// the diagonal, `1/λ` on it.
#[must_use]
pub fn cov_self(a: &[Vec<f64>], beta: &[f64], lamz: f64) -> Vec<Vec<f64>> {
    let n = a.len();
    let mut c = vec![vec![0.0; n]; n];
    for i in 0..n {
        c[i][i] = 1.0 / lamz;
        for j in i + 1..n {
            let v = (-sq_dot(&a[i], &a[j], beta)).r_exp() / lamz;
            c[i][j] = v;
            c[j][i] = v;
        }
    }
    c
}

/// `SepiaDistCov(a, b).compute_cov_mat(beta, lamz)`: the cross covariance.
#[must_use]
pub fn cov_cross(a: &[Vec<f64>], b: &[Vec<f64>], beta: &[f64], lamz: f64) -> Vec<Vec<f64>> {
    a.iter().map(|ai| b.iter().map(|bj| (-sq_dot(ai, bj, beta)).r_exp() / lamz).collect()).collect()
}

fn sq_dot(a: &[f64], b: &[f64], beta: &[f64]) -> f64 {
    let mut s = 0.0;
    for k in 0..beta.len() {
        let d = a[k] - b[k];
        s += d * d * beta[k];
    }
    s
}

fn chol(a: &[Vec<f64>]) -> Option<Vec<Vec<f64>>> {
    cholesky(a, 0.0)
}

fn chol_solve(l: &[Vec<f64>], b: &[f64]) -> Vec<f64> {
    let n = b.len();
    let mut z = vec![0.0; n];
    for i in 0..n {
        let mut s = b[i];
        for k in 0..i {
            s -= l[i][k] * z[k];
        }
        z[i] = s / l[i][i];
    }
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

/// `doLogLik`: `−Σ ln diag(L) − ½ wᵀ C⁻¹ w`, or −∞ if `C` is not positive
/// definite (SEPIA's "chol error").
fn do_log_lik(c: &[Vec<f64>], w: &[f64]) -> f64 {
    let Some(l) = chol(c) else {
        return f64::NEG_INFINITY;
    };
    let log_det = pairwise_sum(&l.iter().enumerate().map(|(i, r)| r[i].r_ln()).collect::<Vec<f64>>());
    let p1 = chol_solve(&l, w);
    let q = pairwise_sum(&p1.iter().zip(w).map(|(a, b)| a * b).collect::<Vec<f64>>());
    -log_det - 0.5 * q
}

fn inverse(c: &[Vec<f64>]) -> Option<Vec<Vec<f64>>> {
    let l = chol(c)?;
    let n = c.len();
    let cols: Vec<Vec<f64>> = (0..n)
        .map(|j| {
            let mut e = vec![0.0; n];
            e[j] = 1.0;
            chol_solve(&l, &e)
        })
        .collect();
    Some((0..n).map(|i| (0..n).map(|j| cols[j][i]).collect()).collect())
}

fn matmul(a: &[Vec<f64>], b: &[Vec<f64>]) -> Vec<Vec<f64>> {
    let k = b.len();
    let m = b.first().map_or(0, Vec::len);
    a.iter()
        .map(|ai| {
            let mut r = vec![0.0; m];
            for t in 0..k {
                let s = ai[t];
                for j in 0..m {
                    r[j] += s * b[t][j];
                }
            }
            r
        })
        .collect()
}

/// The emulator's W block, `SigW = C_zt(β_U, λ_Uz) + (1/λ_WOs + 1/λ_Ws) I`,
/// with its log-likelihood term and inverse: fixed while the emulator's
/// hyperparameters are.
#[derive(Debug, Clone, PartialEq)]
pub struct EmulatorFactor {
    /// `doLogLik(SigW, w)`, SEPIA's `SigWl`.
    pub log_lik_w: f64,
    /// `inv(SigW)`, SEPIA's `SigWi`.
    pub sig_w_inv: Vec<Vec<f64>>,
    beta_u: Vec<f64>,
    lam_uz: f64,
    lam_ws: f64,
    lam_wos: f64,
}

/// A scalar-output SEPIA model: data, form and priors.
#[derive(Debug, Clone, PartialEq)]
pub struct KohModel {
    /// The transformed data.
    pub data: KohData,
    /// Which model.
    pub form: KohForm,
    /// Priors.
    pub priors: KohPriors,
}

/// The predictive mean and covariance of `[v(x_pred); u(x_pred)]` (with
/// discrepancy) or `u(x_pred)` (without) on the standardised scale, one
/// parameter sample: SEPIA's `pred.mu[ii]`, `pred.sigma[ii]`.
#[derive(Debug, Clone, PartialEq)]
pub struct KohPredictive {
    /// Mean, `npred (pv + pu)`.
    pub mean: Vec<f64>,
    /// Covariance.
    pub cov: Vec<Vec<f64>>,
}

/// SigObs for scalar output: `inv(I + 1e-8 I)`.
const SIG_OBS: f64 = 1.0 / (1.0 + 1e-8);

impl KohModel {
    /// A model over `data`. A simulation-only `form` needs no observations;
    /// the others need them.
    ///
    /// # Errors
    /// [`RafflesError::InvalidParameter`] if the form and the data disagree.
    pub fn new(data: KohData, form: KohForm, priors: KohPriors) -> Result<Self> {
        if (form == KohForm::SimulationOnly) != (data.n() == 0) {
            return Err(RafflesError::InvalidParameter {
                parameter: "form".to_string(),
                value: data.n() as f64,
                reason: "a simulation-only model takes no observations; a calibration needs them".to_string(),
            });
        }
        Ok(Self { data, form, priors })
    }

    /// The emulator factor at `params`' emulator hyperparameters, or `None`
    /// if `SigW` is not positive definite.
    #[must_use]
    pub fn emulator_factor(&self, params: &KohParams) -> Option<EmulatorFactor> {
        let mut cg = cov_self(&self.data.zt, &params.beta_u, params.lam_uz);
        for (i, r) in cg.iter_mut().enumerate() {
            r[i] = r[i] + 1.0 / params.lam_wos + 1.0 / params.lam_ws;
        }
        let log_lik_w = do_log_lik(&cg, &self.data.w);
        if !log_lik_w.is_finite() {
            return None;
        }
        let sig_w_inv = if self.form == KohForm::SimulationOnly { Vec::new() } else { inverse(&cg)? };
        Some(EmulatorFactor { log_lik_w, sig_w_inv, beta_u: params.beta_u.clone(), lam_uz: params.lam_uz, lam_ws: params.lam_ws, lam_wos: params.lam_wos })
    }

    /// `compute_log_lik(model, 'all')`.
    #[must_use]
    pub fn log_lik(&self, params: &KohParams) -> f64 {
        match self.emulator_factor(params) {
            Some(f) => self.log_lik_with(params, &f),
            None => f64::NEG_INFINITY,
        }
    }

    /// The log-likelihood reusing `factor` for the W block. `factor` must
    /// have been built at `params`' emulator hyperparameters; a mismatch
    /// gives −∞ rather than a wrong number.
    #[must_use]
    pub fn log_lik_with(&self, params: &KohParams, factor: &EmulatorFactor) -> f64 {
        if factor.beta_u != params.beta_u || factor.lam_uz != params.lam_uz || factor.lam_ws != params.lam_ws || factor.lam_wos != params.lam_wos {
            return f64::NEG_INFINITY;
        }
        if self.form == KohForm::SimulationOnly {
            return factor.log_lik_w;
        }
        let d = &self.data;
        let n = d.n();
        let xt: Vec<Vec<f64>> = d.x_obs.iter().map(|x| [x.as_slice(), params.theta.as_slice()].concat()).collect();
        let mut sig_u = cov_self(&xt, &params.beta_u, params.lam_uz);
        for (i, r) in sig_u.iter_mut().enumerate() {
            r[i] = r[i] + 1.0 / params.lam_ws;
        }
        let sig_uw = cov_cross(&xt, &d.zt, &params.beta_u, params.lam_uz);
        let w_mat = matmul(&sig_uw, &factor.sig_w_inv);
        // SigUgW = SigU - W SigUW'
        let mut big = vec![vec![0.0; n]; n];
        for i in 0..n {
            for j in 0..n {
                let s: f64 = w_mat[i].iter().zip(&sig_uw[j]).map(|(a, b)| a * b).sum();
                big[i][j] = sig_u[i][j] - s;
            }
        }
        // SigVUgW = SigObs/lamOs (+ SigV) + SigUgW, in SEPIA's order.
        let mut svu = vec![vec![0.0; n]; n];
        for (i, r) in svu.iter_mut().enumerate() {
            r[i] = SIG_OBS / params.lam_os;
        }
        if self.form == KohForm::WithDiscrepancy {
            let sig_v = cov_self(&d.x_obs, &params.beta_v, params.lam_vz);
            for i in 0..n {
                for j in 0..n {
                    svu[i][j] = svu[i][j] + sig_v[i][j];
                }
            }
        }
        for i in 0..n {
            for j in 0..n {
                svu[i][j] = svu[i][j] + big[i][j];
            }
        }
        let mu_diff: Vec<f64> = (0..n).map(|i| d.u[i] - w_mat[i].iter().zip(&d.w).map(|(a, b)| a * b).sum::<f64>()).collect();
        do_log_lik(&svu, &mu_diff) + factor.log_lik_w
    }

    /// `log_prior()`: the sum over SEPIA's `mcmcList`, in its order.
    #[must_use]
    pub fn log_prior(&self, params: &KohParams) -> f64 {
        let p = &self.priors;
        let emu = |lp: f64| lp + p.beta_u.log_prior(&params.beta_u) + p.lam_uz.log_prior(&[params.lam_uz]) + p.lam_ws.log_prior(&[params.lam_ws]) + p.lam_wos.log_prior(&[params.lam_wos]);
        match self.form {
            KohForm::SimulationOnly => emu(0.0),
            KohForm::NoDiscrepancy => emu(0.0 + p.theta.log_prior(&params.theta)) + p.lam_os.log_prior(&[params.lam_os]),
            KohForm::WithDiscrepancy => {
                let lp = 0.0 + p.theta.log_prior(&params.theta) + p.beta_v.log_prior(&params.beta_v) + p.beta_u.log_prior(&params.beta_u) + p.lam_vz.log_prior(&[params.lam_vz]);
                lp + p.lam_uz.log_prior(&[params.lam_uz]) + p.lam_ws.log_prior(&[params.lam_ws]) + p.lam_wos.log_prior(&[params.lam_wos]) + p.lam_os.log_prior(&[params.lam_os])
            }
        }
    }

    /// `logPost()`: log-likelihood plus log prior.
    #[must_use]
    pub fn log_post(&self, params: &KohParams) -> f64 {
        let lp = self.log_prior(params);
        if lp == f64::NEG_INFINITY {
            return lp;
        }
        self.log_lik(params) + lp
    }

    /// `uvPred` for one parameter sample at unit-scale `x_pred` (θ from the
    /// sample, `addResidVar = False`): the conditional Gaussian of
    /// `[v(x_pred); u(x_pred)]` given `[u; w]`.
    ///
    /// # Errors
    /// [`RafflesError::InvalidParameter`] for a simulation-only model or a
    /// joint covariance that is not positive definite.
    pub fn predict(&self, params: &KohParams, x_pred: &[Vec<f64>]) -> Result<KohPredictive> {
        if self.form == KohForm::SimulationOnly {
            return Err(RafflesError::InvalidParameter { parameter: "form".to_string(), value: 0.0, reason: "prediction needs a calibration model".to_string() });
        }
        let d = &self.data;
        let (n, m, np) = (d.n(), d.m(), x_pred.len());
        let with_d = self.form == KohForm::WithDiscrepancy;
        let pv = usize::from(with_d);
        let xpt: Vec<Vec<f64>> = x_pred.iter().map(|x| [x.as_slice(), params.theta.as_slice()].concat()).collect();
        let xt: Vec<Vec<f64>> = d.x_obs.iter().map(|x| [x.as_slice(), params.theta.as_slice()].concat()).collect();
        let (bu, lu) = (&params.beta_u, params.lam_uz);
        let mut sig_u = cov_self(&xt, bu, lu);
        for (i, r) in sig_u.iter_mut().enumerate() {
            r[i] = r[i] + 1.0 / params.lam_ws;
        }
        let mut sig_w = cov_self(&d.zt, bu, lu);
        for (i, r) in sig_w.iter_mut().enumerate() {
            r[i] = r[i] + 1.0 / params.lam_wos + 1.0 / params.lam_ws;
        }
        let sig_uw = cov_cross(&xt, &d.zt, bu, lu);
        let sig_v = if with_d { cov_self(&d.x_obs, &params.beta_v, params.lam_vz) } else { Vec::new() };
        // SigData = [SigU + SigV + SigObs/lamOs, SigUW; SigUW', SigW]
        let nd = n + m;
        let mut data = vec![vec![0.0; nd]; nd];
        for i in 0..n {
            for j in 0..n {
                let mut v = sig_u[i][j];
                if with_d {
                    v = v + sig_v[i][j];
                }
                if i == j {
                    v = v + SIG_OBS * 1.0 / params.lam_os;
                }
                data[i][j] = v;
            }
            for j in 0..m {
                data[i][n + j] = sig_uw[i][j];
                data[n + j][i] = sig_uw[i][j];
            }
        }
        for i in 0..m {
            for j in 0..m {
                data[n + i][n + j] = sig_w[i][j];
            }
        }
        // SigPred = blockdiag(SigVp, SigUp); SigCross = [SigVVx SigUUx; 0 SigWUx].
        let npp = np * (pv + 1);
        let mut pred = vec![vec![0.0; npp]; npp];
        if with_d {
            let vp = cov_self(x_pred, &params.beta_v, params.lam_vz);
            for i in 0..np {
                pred[i][..np].copy_from_slice(&vp[i]);
            }
        }
        let mut up = cov_self(&xpt, bu, lu);
        for (i, r) in up.iter_mut().enumerate() {
            r[i] = r[i] + 1.0 / params.lam_ws;
        }
        for i in 0..np {
            pred[pv * np + i][pv * np..].copy_from_slice(&up[i]);
        }
        let mut cross = vec![vec![0.0; npp]; nd];
        if with_d {
            let vvx = cov_cross(&d.x_obs, x_pred, &params.beta_v, params.lam_vz);
            for i in 0..n {
                cross[i][..np].copy_from_slice(&vvx[i]);
            }
        }
        let uux = cov_cross(&xt, &xpt, bu, lu);
        let wux = cov_cross(&d.zt, &xpt, bu, lu);
        for i in 0..n {
            cross[i][pv * np..].copy_from_slice(&uux[i]);
        }
        for i in 0..m {
            cross[n + i][pv * np..].copy_from_slice(&wux[i]);
        }
        let l = chol(&data).ok_or_else(|| RafflesError::InvalidParameter { parameter: "SigData".to_string(), value: f64::NAN, reason: "not positive definite".to_string() })?;
        // W = SigData \ SigCross, column by column.
        let wcols: Vec<Vec<f64>> = (0..npp).map(|c| chol_solve(&l, &cross.iter().map(|r| r[c]).collect::<Vec<f64>>())).collect();
        let uw: Vec<f64> = d.u.iter().chain(&d.w).copied().collect();
        let mean = wcols.iter().map(|wc| wc.iter().zip(&uw).map(|(a, b)| a * b).sum()).collect();
        let cov = (0..npp)
            .map(|i| (0..npp).map(|j| pred[i][j] - wcols[i].iter().zip(cross.iter()).map(|(a, r)| a * r[j]).sum::<f64>()).collect())
            .collect();
        Ok(KohPredictive { mean, cov })
    }
}
