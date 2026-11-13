// ---------------------------------------------------------------------------
// Ported from FilterPy.
//
//   Upstream project: FilterPy — Roger R. Labbe Jr
//   Upstream repo:    https://github.com/rlabbe/filterpy
//   Upstream files:   filterpy/kalman/ensemble_kalman_filter.py
//                     filterpy/common/helpers.py (outer_product_sum)
//   Upstream commit:  5df4e095fac07684a05c495bd72d3424f7c68c9c  (master, 2.0.0)
//   Accessed:         2026-11-04
//
//   The MIT License (MIT)
//   Copyright (c) 2015 Roger R. Labbe Jr
//
//   Permission is hereby granted, free of charge, to any person obtaining a
//   copy of this software and associated documentation files (the
//   "Software"), to deal in the Software without restriction, including
//   without limitation the rights to use, copy, modify, merge, publish,
//   distribute, sublicense, and/or sell copies of the Software, and to permit
//   persons to whom the Software is furnished to do so, subject to the
//   following conditions: The above copyright notice and this permission
//   notice shall be included in all copies or substantial portions of the
//   Software. (Full text: crates/raffles/LICENSE-MIT-FILTERPY.)
//
// This Rust translation is part of RAFFLES / Outram Park and is distributed
// under AGPL-3.0-only. MIT -> AGPLv3 is a ONE-WAY flow: this file may not be
// contributed back to FilterPy or redistributed under MIT terms.
//
// Translation notes: EnsembleKalmanFilter.initialize, .predict, .update and
// outer_product_sum are ported. Upstream draws with
// numpy.random.multivariate_normal; here every draw is an argument (the
// *_with methods: the bit-identity path), or comes from `gaussian_draws`
// (petir LCG, Box-Muller, Cholesky; NOT numpy's generator, compared only
// statistically). Upstream's P after update is P - K S K^T, not the updated
// ensemble's covariance; kept, with ensemble_covariance() offered beside it.
// fx and hx are passed to each call.
// ---------------------------------------------------------------------------

//! The ensemble Kalman filter (FilterPy's `EnsembleKalmanFilter`).

use super::numpy_ops::{mean_rows, outer, sum_matrices, Matrix};
use crate::distributions::seeded::sample_normal;
use crate::{RafflesError, Result};

/// FilterPy's `outer_product_sum(A, B)`: `Σ_i A_i B_iᵀ` over rows
/// (an `einsum`, then numpy's sequential sum over axis 0).
///
/// # Errors
/// If `A` and `B` have different row counts, or no rows.
pub fn outer_product_sum(a: &Matrix, b: &Matrix) -> Result<Matrix> {
    if a.rows() != b.rows() {
        return Err(RafflesError::DimensionMismatch { expected: a.rows(), found: b.rows() });
    }
    let stack: Vec<Matrix> = (0..a.rows()).map(|i| outer(a.row(i), b.row(i))).collect();
    sum_matrices(&stack)
}

/// A lower factor `L` with `L Lᵀ = cov` for a symmetric positive
/// **semi**-definite `cov` (an outer-product Cholesky that sets a column to
/// zero where its pivot is zero to round-off, `≤ 1e-12 ·` the largest
/// diagonal). numpy's `multivariate_normal` accepts such covariances (it
/// factors by SVD), and FilterPy's kinematic `Q` is one: rank 1 for
/// `Q_discrete_white_noise(2)`.
fn psd_factor(cov: &Matrix) -> Result<Matrix> {
    let n = cov.rows();
    let scale = cov.diag().iter().fold(0.0f64, |a, v| a.max(v.abs()));
    let mut l = Matrix::zeros(n, n);
    for j in 0..n {
        let mut d = cov.get(j, j);
        for k in 0..j {
            d -= l.get(j, k) * l.get(j, k);
        }
        if d < -1e-12 * scale {
            return Err(RafflesError::InvalidParameter { parameter: "cov".into(), value: d, reason: "not positive semi-definite".into() });
        }
        if d <= 1e-12 * scale {
            continue;
        }
        let ljj = d.sqrt();
        l.set(j, j, ljj);
        for i in (j + 1)..n {
            let mut v = cov.get(i, j);
            for k in 0..j {
                v -= l.get(i, k) * l.get(j, k);
            }
            l.set(i, j, v / ljj);
        }
    }
    Ok(l)
}

/// `n` draws from `N(mean, cov)` as rows: `mean + L ξ`, `L` a lower factor
/// of the (semi-definite) covariance, `ξ` Box–Muller standard normals from the workspace LCG
/// (`distributions::seeded::sample_normal`). **Not** numpy's
/// `multivariate_normal` (which factors by SVD with its own generator):
/// the port's own draws are compared with FilterPy only statistically.
///
/// # Errors
/// If `cov` is not square of `mean`'s size, or not positive definite.
pub fn gaussian_draws(mean: &[f64], cov: &Matrix, n: usize, seed: &mut u64) -> Result<Matrix> {
    let d = mean.len();
    if cov.rows() != d || cov.cols() != d {
        return Err(RafflesError::DimensionMismatch { expected: d, found: cov.rows() });
    }
    let l = psd_factor(cov)?;
    let mut out = Matrix::zeros(n, d);
    for i in 0..n {
        let xi: Vec<f64> = (0..d).map(|_| sample_normal(seed)).collect();
        let lx = l.dot_vec(&xi)?;
        let row: Vec<f64> = mean.iter().zip(&lx).map(|(m, v)| m + v).collect();
        out.set_row(i, &row)?;
    }
    Ok(out)
}

/// FilterPy's `EnsembleKalmanFilter`.
#[derive(Debug, Clone, PartialEq)]
pub struct EnsembleKalmanFilter {
    /// State dimension.
    pub dim_x: usize,
    /// Measurement dimension.
    pub dim_z: usize,
    /// Ensemble size.
    pub n: usize,
    /// Time step passed to `fx`.
    pub dt: f64,
    /// State estimate (the ensemble mean after predict / update).
    pub x: Vec<f64>,
    /// Covariance (upstream's: sample after predict, `P − K S Kᵀ` after update).
    pub p: Matrix,
    /// Process noise.
    pub q: Matrix,
    /// Measurement noise.
    pub r: Matrix,
    /// The ensemble, one member per row.
    pub sigmas: Matrix,
    /// Gain of the last update.
    pub k: Matrix,
    /// Innovation covariance of the last update.
    pub s: Matrix,
    /// Its inverse.
    pub si: Matrix,
    /// Prior.
    pub x_prior: Vec<f64>,
    /// Prior covariance.
    pub p_prior: Matrix,
    /// Posterior.
    pub x_post: Vec<f64>,
    /// Posterior covariance.
    pub p_post: Matrix,
}

impl EnsembleKalmanFilter {
    /// Upstream's constructor and `initialize(x, P)`, with the initial
    /// ensemble supplied (upstream draws it from `N(x, P)`); `Q = I`,
    /// `R = I`.
    ///
    /// # Errors
    /// If `dim_z` or the ensemble size is 0, or the shapes disagree.
    pub fn new(x: &[f64], p: &Matrix, dim_z: usize, dt: f64, initial_ensemble: Matrix) -> Result<Self> {
        let (n, dim_x) = (initial_ensemble.rows(), x.len());
        if dim_z == 0 || n == 0 {
            return Err(RafflesError::InvalidParameter { parameter: "dim_z / N".into(), value: 0.0, reason: "must be greater than zero".into() });
        }
        if initial_ensemble.cols() != dim_x || p.rows() != dim_x || p.cols() != dim_x {
            return Err(RafflesError::DimensionMismatch { expected: dim_x, found: initial_ensemble.cols() });
        }
        Ok(Self {
            dim_x,
            dim_z,
            n,
            dt,
            x: x.to_vec(),
            p: p.clone(),
            q: Matrix::identity(dim_x),
            r: Matrix::identity(dim_z),
            sigmas: initial_ensemble,
            k: Matrix::zeros(dim_x, dim_z),
            s: Matrix::zeros(dim_z, dim_z),
            si: Matrix::zeros(dim_z, dim_z),
            x_prior: x.to_vec(),
            p_prior: p.clone(),
            x_post: x.to_vec(),
            p_post: p.clone(),
        })
    }

    /// [`Self::new`] with the initial ensemble drawn by [`gaussian_draws`].
    ///
    /// # Errors
    /// As [`Self::new`] and [`gaussian_draws`].
    pub fn new_drawn(x: &[f64], p: &Matrix, dim_z: usize, dt: f64, n: usize, seed: &mut u64) -> Result<Self> {
        let e = gaussian_draws(x, p, n, seed)?;
        Self::new(x, p, dim_z, dt, e)
    }

    fn deviations(m: &Matrix, mean: &[f64]) -> Matrix {
        let mut d = m.clone();
        for i in 0..m.rows() {
            for (j, v) in mean.iter().enumerate() {
                d.set(i, j, m.get(i, j) - v);
            }
        }
        d
    }

    /// Upstream's `predict()` with the process draws supplied (`N × dim_x`):
    /// `σ_i = fx(σ_i, dt) + e_i`, `x = mean σ`, `P = Σ (σ_i − x)(σ_i − x)ᵀ / (N − 1)`.
    ///
    /// # Errors
    /// From `fx`, or on a shape mismatch.
    pub fn predict_with<Fx: Fn(&[f64], f64) -> Result<Vec<f64>>>(&mut self, fx: Fx, process_draws: &Matrix) -> Result<()> {
        for i in 0..self.n {
            let v = fx(self.sigmas.row(i), self.dt)?;
            self.sigmas.set_row(i, &v)?;
        }
        self.sigmas = self.sigmas.add(process_draws)?;
        self.x = mean_rows(&self.sigmas)?;
        let d = Self::deviations(&self.sigmas, &self.x);
        self.p = outer_product_sum(&d, &d)?.div_scalar((self.n - 1) as f64);
        self.x_prior = self.x.clone();
        self.p_prior = self.p.clone();
        Ok(())
    }

    /// [`Self::predict_with`], drawing the process noise from `N(0, Q)`.
    ///
    /// # Errors
    /// As [`Self::predict_with`] and [`gaussian_draws`].
    pub fn predict<Fx: Fn(&[f64], f64) -> Result<Vec<f64>>>(&mut self, fx: Fx, seed: &mut u64) -> Result<()> {
        let e = gaussian_draws(&vec![0.0; self.dim_x], &self.q, self.n, seed)?;
        self.predict_with(fx, &e)
    }

    /// Upstream's `update(z)` with the measurement draws supplied
    /// (`N × dim_z`): `P_zz = Σ (h_i − h̄)(h_i − h̄)ᵀ/(N − 1) + R`,
    /// `P_xz = Σ (σ_i − x)(h_i − h̄)ᵀ/(N − 1)`, `K = P_xz P_zz⁻¹`,
    /// `σ_i += K (z + e_i − h_i)`, `x = mean σ`, `P = P − K S Kᵀ`.
    ///
    /// # Errors
    /// From `hx`, on a shape mismatch, or a singular `P_zz`.
    pub fn update_with<Hx: Fn(&[f64]) -> Result<Vec<f64>>>(&mut self, z: &[f64], hx: Hx, measurement_draws: &Matrix) -> Result<()> {
        if z.len() != self.dim_z {
            return Err(RafflesError::DimensionMismatch { expected: self.dim_z, found: z.len() });
        }
        let nm1 = (self.n - 1) as f64;
        let mut sigmas_h = Matrix::zeros(self.n, self.dim_z);
        for i in 0..self.n {
            sigmas_h.set_row(i, &hx(self.sigmas.row(i))?)?;
        }
        let z_mean = mean_rows(&sigmas_h)?;
        let dh = Self::deviations(&sigmas_h, &z_mean);
        let p_zz = outer_product_sum(&dh, &dh)?.div_scalar(nm1).add(&self.r)?;
        let dx = Self::deviations(&self.sigmas, &self.x);
        let p_xz = outer_product_sum(&dx, &dh)?.div_scalar(nm1);
        self.s = p_zz;
        self.si = self.s.inv()?;
        self.k = p_xz.dot(&self.si)?;
        for i in 0..self.n {
            let v: Vec<f64> = (0..self.dim_z).map(|j| (z[j] + measurement_draws.get(i, j)) - sigmas_h.get(i, j)).collect();
            let kv = self.k.dot_vec(&v)?;
            let row: Vec<f64> = self.sigmas.row(i).iter().zip(&kv).map(|(a, b)| a + b).collect();
            self.sigmas.set_row(i, &row)?;
        }
        self.x = mean_rows(&self.sigmas)?;
        self.p = self.p.sub(&self.k.dot(&self.s)?.dot(&self.k.t())?)?;
        self.x_post = self.x.clone();
        self.p_post = self.p.clone();
        Ok(())
    }

    /// [`Self::update_with`], drawing the measurement perturbations from
    /// `N(0, R)`.
    ///
    /// # Errors
    /// As [`Self::update_with`] and [`gaussian_draws`].
    pub fn update<Hx: Fn(&[f64]) -> Result<Vec<f64>>>(&mut self, z: &[f64], hx: Hx, seed: &mut u64) -> Result<()> {
        let e = gaussian_draws(&vec![0.0; self.dim_z], &self.r, self.n, seed)?;
        self.update_with(z, hx, &e)
    }

    /// The current ensemble's own sample covariance (divisor N − 1). Not
    /// upstream: after an update FilterPy's `P` is `P − K S Kᵀ` instead.
    ///
    /// # Errors
    /// On an empty ensemble.
    pub fn ensemble_covariance(&self) -> Result<Matrix> {
        let m = mean_rows(&self.sigmas)?;
        let d = Self::deviations(&self.sigmas, &m);
        Ok(outer_product_sum(&d, &d)?.div_scalar((self.n - 1) as f64))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The LCG draws have the requested moments (N = 20000, 4σ bounds), and
    /// the error paths.
    #[test]
    fn draws_have_their_moments() {
        let cov = Matrix::from_rows(&[vec![2.0, 0.6], vec![0.6, 0.5]]).unwrap();
        let mut seed = 42u64;
        let e = gaussian_draws(&[1.0, -2.0], &cov, 20_000, &mut seed).unwrap();
        let m = mean_rows(&e).unwrap();
        assert!((m[0] - 1.0).abs() < 4.0 * (2.0f64 / 20_000.0).sqrt() && (m[1] + 2.0).abs() < 4.0 * (0.5f64 / 20_000.0).sqrt(), "{m:?}");
        let f = EnsembleKalmanFilter::new(&[1.0, -2.0], &cov, 1, 1.0, e).unwrap();
        let c = f.ensemble_covariance().unwrap();
        for (i, j, v) in [(0, 0, 2.0), (0, 1, 0.6), (1, 1, 0.5)] {
            assert!((c.get(i, j) - v).abs() < 0.06 * v.max(0.5), "{i}{j} {}", c.get(i, j));
        }
        assert!(gaussian_draws(&[0.0], &cov, 2, &mut seed).is_err());
        // Semi-definite (rank 1) works, and its draws stay on the line.
        let rank1 = Matrix::from_rows(&[vec![1.0, 2.0], vec![2.0, 4.0]]).unwrap();
        let e1 = gaussian_draws(&[0.0, 0.0], &rank1, 5, &mut seed).unwrap();
        for i in 0..5 {
            assert!((e1.get(i, 1) - 2.0 * e1.get(i, 0)).abs() < 1e-12);
        }
        let indefinite = Matrix::from_rows(&[vec![1.0, 2.0], vec![2.0, 1.0]]).unwrap();
        assert!(gaussian_draws(&[0.0, 0.0], &indefinite, 1, &mut seed).is_err());
        assert!(EnsembleKalmanFilter::new(&[0.0], &Matrix::identity(1), 0, 1.0, Matrix::zeros(3, 1)).is_err());
        assert!(EnsembleKalmanFilter::new(&[0.0], &Matrix::identity(1), 1, 1.0, Matrix::zeros(3, 2)).is_err());
        assert!(outer_product_sum(&Matrix::zeros(2, 1), &Matrix::zeros(3, 1)).is_err());
    }
}
