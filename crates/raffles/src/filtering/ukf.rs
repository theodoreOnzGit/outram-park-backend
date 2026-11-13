// ---------------------------------------------------------------------------
// Ported from FilterPy.
//
//   Upstream project: FilterPy — Roger R. Labbe Jr
//   Upstream repo:    https://github.com/rlabbe/filterpy
//   Upstream files:   filterpy/kalman/UKF.py
//                     filterpy/kalman/unscented_transform.py
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
// Translation notes: UnscentedKalmanFilter.predict (including upstream's
// re-draw of sigmas_f from the predicted x, P, commit 94852b4), .update and
// .cross_variance, and unscented_transform with the default mean and residual
// (x = Wm . sigmas, P = y^T @ (y * Wc) + noise), are ported. fx and hx are
// passed to each call instead of stored on the object (no type parameter or
// lifetime on the struct). Not ported: custom x_mean_fn / z_mean_fn /
// residual / state_add callables, batch_filter, the UKF rts_smoother and the
// likelihood properties.
// ---------------------------------------------------------------------------

//! The unscented Kalman filter (FilterPy's `UnscentedKalmanFilter` and
//! `unscented_transform`).

use super::numpy_ops::{outer, vec_dot, Matrix};
use super::sigma_points::MerweScaledSigmaPoints;
use crate::{RafflesError, Result};

/// FilterPy's `unscented_transform(sigmas, Wm, Wc, noise_cov)` with the
/// default mean and residual: `x = Wm · sigmas`,
/// `P = (σ − x)ᵀ ((σ − x) ∘ Wc) + noise`.
///
/// # Errors
/// On a dimension mismatch.
pub fn unscented_transform(sigmas: &Matrix, wm: &[f64], wc: &[f64], noise_cov: Option<&Matrix>) -> Result<(Vec<f64>, Matrix)> {
    let (kmax, n) = (sigmas.rows(), sigmas.cols());
    if wc.len() != kmax {
        return Err(RafflesError::DimensionMismatch { expected: kmax, found: wc.len() });
    }
    let x = vec_dot(wm, sigmas)?;
    let mut y = Matrix::zeros(kmax, n);
    let mut yw = Matrix::zeros(kmax, n);
    for k in 0..kmax {
        for j in 0..n {
            let d = sigmas.get(k, j) - x[j];
            y.set(k, j, d);
            yw.set(k, j, d * wc[k]);
        }
    }
    let mut p = y.t().dot(&yw)?;
    if let Some(q) = noise_cov {
        p = p.add(q)?;
    }
    Ok((x, p))
}

/// FilterPy's `UnscentedKalmanFilter` with Merwe sigma points. `x` is 1-D,
/// as upstream.
#[derive(Debug, Clone, PartialEq)]
pub struct UnscentedKalmanFilter {
    /// State estimate.
    pub x: Vec<f64>,
    /// State covariance.
    pub p: Matrix,
    /// Process noise.
    pub q: Matrix,
    /// Measurement noise.
    pub r: Matrix,
    /// Default time step passed to `fx`.
    pub dt: f64,
    /// The sigma-point scheme.
    pub points: MerweScaledSigmaPoints,
    /// Sigma points after `predict` (re-drawn from the prior, as upstream).
    pub sigmas_f: Matrix,
    /// Sigma points through `hx` in the last `update`.
    pub sigmas_h: Matrix,
    /// Gain of the last update.
    pub k: Matrix,
    /// Residual of the last update.
    pub y: Vec<f64>,
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
    dim_z: usize,
}

impl UnscentedKalmanFilter {
    /// Upstream's constructor: `x = 0`, `P = Q = I`, `R = I`.
    ///
    /// # Errors
    /// [`RafflesError::DimensionMismatch`] if the points' n is not `dim_x`.
    pub fn new(dim_x: usize, dim_z: usize, dt: f64, points: MerweScaledSigmaPoints) -> Result<Self> {
        if points.n != dim_x || dim_z == 0 {
            return Err(RafflesError::DimensionMismatch { expected: dim_x, found: points.n });
        }
        let ns = points.num_sigmas();
        Ok(Self {
            x: vec![0.0; dim_x],
            p: Matrix::identity(dim_x),
            q: Matrix::identity(dim_x),
            r: Matrix::identity(dim_z),
            dt,
            points,
            sigmas_f: Matrix::zeros(ns, dim_x),
            sigmas_h: Matrix::zeros(ns, dim_z),
            k: Matrix::zeros(dim_x, dim_z),
            y: vec![0.0; dim_z],
            s: Matrix::zeros(dim_z, dim_z),
            si: Matrix::zeros(dim_z, dim_z),
            x_prior: vec![0.0; dim_x],
            p_prior: Matrix::identity(dim_x),
            x_post: vec![0.0; dim_x],
            p_post: Matrix::identity(dim_x),
            dim_z,
        })
    }

    /// Upstream's `predict()`: propagate the sigma points of `(x, P)`
    /// through `fx(σ, dt)`, take the unscented transform plus `Q`, then
    /// re-draw `sigmas_f` from the predicted `(x, P)`.
    ///
    /// # Errors
    /// From `fx`, a wrong-length `fx` result, or a non-positive-definite `P`.
    pub fn predict<Fx: Fn(&[f64], f64) -> Result<Vec<f64>>>(&mut self, fx: Fx) -> Result<()> {
        let sigmas = self.points.sigma_points(&self.x, &self.p)?;
        for i in 0..sigmas.rows() {
            let v = fx(sigmas.row(i), self.dt)?;
            self.sigmas_f.set_row(i, &v)?;
        }
        let (x, p) = unscented_transform(&self.sigmas_f, &self.points.wm, &self.points.wc, Some(&self.q))?;
        self.x = x;
        self.p = p;
        self.sigmas_f = self.points.sigma_points(&self.x, &self.p)?;
        self.x_prior = self.x.clone();
        self.p_prior = self.p.clone();
        Ok(())
    }

    /// Upstream's `update(z)`: `σ_h = hx(σ_f)`, `(ẑ, S) = UT(σ_h) + R`,
    /// `P_xz = Σ Wc_i (σ_f,i − x)(σ_h,i − ẑ)ᵀ`, `K = P_xz S⁻¹`,
    /// `x = x + K (z − ẑ)`, `P = P − K S Kᵀ`.
    ///
    /// # Errors
    /// From `hx`, a wrong-length `z` or `hx` result, or a singular `S`.
    pub fn update<Hx: Fn(&[f64]) -> Result<Vec<f64>>>(&mut self, z: &[f64], hx: Hx) -> Result<()> {
        if z.len() != self.dim_z {
            return Err(RafflesError::DimensionMismatch { expected: self.dim_z, found: z.len() });
        }
        for i in 0..self.sigmas_f.rows() {
            let v = hx(self.sigmas_f.row(i))?;
            self.sigmas_h.set_row(i, &v)?;
        }
        let (zp, s) = unscented_transform(&self.sigmas_h, &self.points.wm, &self.points.wc, Some(&self.r))?;
        self.s = s;
        self.si = self.s.inv()?;
        let pxz = self.cross_variance(&zp)?;
        self.k = pxz.dot(&self.si)?;
        self.y = z.iter().zip(&zp).map(|(a, b)| a - b).collect();
        let dx = self.k.dot_vec(&self.y)?;
        self.x = self.x.iter().zip(&dx).map(|(a, b)| a + b).collect();
        self.p = self.p.sub(&self.k.dot(&self.s.dot(&self.k.t())?)?)?;
        self.x_post = self.x.clone();
        self.p_post = self.p.clone();
        Ok(())
    }

    /// Upstream's `cross_variance(x, z, sigmas_f, sigmas_h)`, accumulated in
    /// sigma-point order.
    fn cross_variance(&self, zp: &[f64]) -> Result<Matrix> {
        let mut pxz = Matrix::zeros(self.sigmas_f.cols(), self.sigmas_h.cols());
        for i in 0..self.sigmas_f.rows() {
            let dx: Vec<f64> = self.sigmas_f.row(i).iter().zip(&self.x).map(|(a, b)| a - b).collect();
            let dz: Vec<f64> = self.sigmas_h.row(i).iter().zip(zp).map(|(a, b)| a - b).collect();
            pxz = pxz.add(&outer(&dx, &dz).scale(self.points.wc[i]))?;
        }
        Ok(pxz)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Error paths; the numbers are pinned against FilterPy and the KF in
    /// `tests/filterpy_gh1191.rs`.
    #[test]
    fn error_paths() {
        let pts = MerweScaledSigmaPoints::new(2, 0.5, 2.0, 1.0);
        assert!(UnscentedKalmanFilter::new(3, 1, 1.0, pts.clone()).is_err());
        let mut u = UnscentedKalmanFilter::new(2, 1, 1.0, pts).unwrap();
        u.predict(|x, _| Ok(x.to_vec())).unwrap();
        assert!(u.update(&[1.0, 2.0], |x| Ok(vec![x[0]])).is_err());
        assert!(u.predict(|_, _| Ok(vec![1.0])).is_err());
        assert!(unscented_transform(&Matrix::zeros(3, 1), &[1.0; 3], &[1.0; 2], None).is_err());
    }
}
