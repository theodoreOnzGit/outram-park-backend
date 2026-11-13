// ---------------------------------------------------------------------------
// Ported from FilterPy.
//
//   Upstream project: FilterPy — Roger R. Labbe Jr
//   Upstream repo:    https://github.com/rlabbe/filterpy
//   Upstream file:    filterpy/kalman/kalman_filter.py
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
// Translation notes: KalmanFilter.predict, .update (Joseph form, as
// upstream), .batch_filter (predict-then-update order) and .rts_smoother are
// ported, each numpy operation in numpy's own order (numpy_ops). Not ported:
// update_steadystate, update_correlated, update_sequential, the fading-memory
// alpha setter (alpha_sq stays 1 unless set), the likelihood / Mahalanobis
// properties (consistency.rs computes NIS instead), get_prediction,
// get_update, residual_of, measurement_of_state, test_matrix_dimensions and
// the module-level functional API. x is FilterPy's 2-D column (dim_x x 1).
// ---------------------------------------------------------------------------

//! The linear Kalman filter and the Rauch–Tung–Striebel smoother (FilterPy's
//! `KalmanFilter`).

use super::numpy_ops::Matrix;
use crate::{RafflesError, Result};

/// FilterPy's `KalmanFilter`: state `x` (a `dim_x × 1` column) and
/// covariance `P`, with `F`, `H`, `Q`, `R` and an optional control matrix
/// `B`, and the update's by-products `K`, `y`, `S`, `SI` kept as upstream
/// keeps them.
#[derive(Debug, Clone, PartialEq)]
pub struct KalmanFilter {
    /// State dimension.
    pub dim_x: usize,
    /// Measurement dimension.
    pub dim_z: usize,
    /// State estimate (`dim_x × 1`).
    pub x: Matrix,
    /// State covariance.
    pub p: Matrix,
    /// Process noise covariance.
    pub q: Matrix,
    /// Control matrix (`None`, upstream's default).
    pub b: Option<Matrix>,
    /// State transition.
    pub f: Matrix,
    /// Measurement function.
    pub h: Matrix,
    /// Measurement noise covariance.
    pub r: Matrix,
    /// Fading-memory factor squared (1, upstream's default).
    pub alpha_sq: f64,
    /// Kalman gain of the last update.
    pub k: Matrix,
    /// Residual of the last update.
    pub y: Matrix,
    /// System uncertainty of the last update.
    pub s: Matrix,
    /// Its inverse.
    pub si: Matrix,
    /// Prior (after `predict`).
    pub x_prior: Matrix,
    /// Prior covariance.
    pub p_prior: Matrix,
    /// Posterior (after `update`).
    pub x_post: Matrix,
    /// Posterior covariance.
    pub p_post: Matrix,
}

/// What [`KalmanFilter::batch_filter`] returns (upstream's tuple).
#[derive(Debug, Clone, PartialEq)]
pub struct BatchOutput {
    /// Posterior means.
    pub means: Vec<Matrix>,
    /// Posterior covariances.
    pub covariances: Vec<Matrix>,
    /// Prior means.
    pub means_prior: Vec<Matrix>,
    /// Prior covariances.
    pub covariances_prior: Vec<Matrix>,
}

/// What [`KalmanFilter::rts_smoother`] returns (upstream's `(x, P, K, Pp)`).
#[derive(Debug, Clone, PartialEq)]
pub struct RtsOutput {
    /// Smoothed means.
    pub x: Vec<Matrix>,
    /// Smoothed covariances.
    pub p: Vec<Matrix>,
    /// Smoother gains (the last is zero).
    pub k: Vec<Matrix>,
    /// Predicted covariances (the last is the filter's own).
    pub pp: Vec<Matrix>,
}

impl KalmanFilter {
    /// Upstream's constructor: `x = 0`, `P = Q = F = I`, `H = 0`, `R = I`.
    ///
    /// # Errors
    /// [`RafflesError::InvalidParameter`] if either dimension is 0.
    pub fn new(dim_x: usize, dim_z: usize) -> Result<Self> {
        for (name, d) in [("dim_x", dim_x), ("dim_z", dim_z)] {
            if d < 1 {
                return Err(RafflesError::InvalidParameter { parameter: name.into(), value: d as f64, reason: "must be 1 or more".into() });
            }
        }
        let x = Matrix::zeros(dim_x, 1);
        let i = Matrix::identity(dim_x);
        Ok(Self {
            dim_x,
            dim_z,
            x_prior: x.clone(),
            p_prior: i.clone(),
            x_post: x.clone(),
            p_post: i.clone(),
            x,
            p: i.clone(),
            q: i.clone(),
            b: None,
            f: i,
            h: Matrix::zeros(dim_z, dim_x),
            r: Matrix::identity(dim_z),
            alpha_sq: 1.0,
            k: Matrix::zeros(dim_x, dim_z),
            y: Matrix::zeros(dim_z, 1),
            s: Matrix::zeros(dim_z, dim_z),
            si: Matrix::zeros(dim_z, dim_z),
        })
    }

    /// `predict(u)` with the filter's own `B`, `F`, `Q`:
    /// `x = F x (+ B u)`, `P = α² F P Fᵀ + Q`.
    ///
    /// # Errors
    /// On a dimension mismatch.
    pub fn predict(&mut self, u: Option<&Matrix>) -> Result<()> {
        let (f, q, b) = (self.f.clone(), self.q.clone(), self.b.clone());
        self.predict_with(u, b.as_ref(), &f, &q)
    }

    /// `predict(u, B, F, Q)` with explicit matrices.
    ///
    /// # Errors
    /// On a dimension mismatch.
    pub fn predict_with(&mut self, u: Option<&Matrix>, b: Option<&Matrix>, f: &Matrix, q: &Matrix) -> Result<()> {
        self.x = match (b, u) {
            (Some(b), Some(u)) => f.dot(&self.x)?.add(&b.dot(u)?)?,
            _ => f.dot(&self.x)?,
        };
        self.p = f.dot(&self.p)?.dot(&f.t())?.scale(self.alpha_sq).add(q)?;
        self.x_prior = self.x.clone();
        self.p_prior = self.p.clone();
        Ok(())
    }

    /// `update(z)` with the filter's own `R`, `H` (Joseph form).
    ///
    /// # Errors
    /// On a dimension mismatch or a singular `S`.
    pub fn update(&mut self, z: &[f64]) -> Result<()> {
        let (r, h) = (self.r.clone(), self.h.clone());
        self.update_with(z, &r, &h)
    }

    /// `update(z, R, H)`: `y = z − H x`, `S = H P Hᵀ + R`, `K = P Hᵀ S⁻¹`,
    /// `x = x + K y`, `P = (I − K H) P (I − K H)ᵀ + K R Kᵀ`.
    ///
    /// # Errors
    /// On a dimension mismatch or a singular `S`.
    pub fn update_with(&mut self, z: &[f64], r: &Matrix, h: &Matrix) -> Result<()> {
        if z.len() != self.dim_z {
            return Err(RafflesError::DimensionMismatch { expected: self.dim_z, found: z.len() });
        }
        let z = Matrix::column(z);
        self.y = z.sub(&h.dot(&self.x)?)?;
        let pht = self.p.dot(&h.t())?;
        self.s = h.dot(&pht)?.add(r)?;
        self.si = self.s.inv()?;
        self.k = pht.dot(&self.si)?;
        self.x = self.x.add(&self.k.dot(&self.y)?)?;
        let i_kh = Matrix::identity(self.dim_x).sub(&self.k.dot(h)?)?;
        self.p = i_kh.dot(&self.p)?.dot(&i_kh.t())?.add(&self.k.dot(r)?.dot(&self.k.t())?)?;
        self.x_post = self.x.clone();
        self.p_post = self.p.clone();
        Ok(())
    }

    /// `batch_filter(zs, us=…)` with upstream's default order (predict,
    /// then update), the filter's own matrices at every step.
    ///
    /// # Errors
    /// As [`Self::predict`] and [`Self::update`].
    pub fn batch_filter(&mut self, zs: &[Vec<f64>], us: Option<&[Matrix]>) -> Result<BatchOutput> {
        let mut out = BatchOutput { means: vec![], covariances: vec![], means_prior: vec![], covariances_prior: vec![] };
        for (i, z) in zs.iter().enumerate() {
            self.predict(us.map(|u| &u[i]))?;
            out.means_prior.push(self.x.clone());
            out.covariances_prior.push(self.p.clone());
            self.update(z)?;
            out.means.push(self.x.clone());
            out.covariances.push(self.p.clone());
        }
        Ok(out)
    }

    /// `rts_smoother(Xs, Ps)` with the filter's own `F`, `Q` at every step.
    ///
    /// # Errors
    /// As [`rts_smoother`].
    pub fn rts_smoother(&self, xs: &[Matrix], ps: &[Matrix]) -> Result<RtsOutput> {
        let n = xs.len();
        rts_smoother(xs, ps, &vec![self.f.clone(); n], &vec![self.q.clone(); n])
    }
}

/// FilterPy's `KalmanFilter.rts_smoother(Xs, Ps, Fs, Qs)`: for
/// `k = n − 2 … 0`, `Pp[k] = F P[k] Fᵀ + Q`, `K[k] = P[k] Fᵀ Pp[k]⁻¹`,
/// `x[k] += K[k] (x[k+1] − F x[k])`, `P[k] += K[k] (P[k+1] − Pp[k]) K[k]ᵀ`,
/// with `F = Fs[k+1]`, `Q = Qs[k+1]` as upstream.
///
/// # Errors
/// [`RafflesError::DimensionMismatch`] if the lists differ in length (upstream's
/// `ValueError`) or on a matrix mismatch; a singular `Pp`.
pub fn rts_smoother(xs: &[Matrix], ps: &[Matrix], fs: &[Matrix], qs: &[Matrix]) -> Result<RtsOutput> {
    let n = xs.len();
    for len in [ps.len(), fs.len(), qs.len()] {
        if len != n {
            return Err(RafflesError::DimensionMismatch { expected: n, found: len });
        }
    }
    let dim_x = xs.first().map_or(0, Matrix::rows);
    let mut x = xs.to_vec();
    let mut p = ps.to_vec();
    let mut pp = ps.to_vec();
    let mut k = vec![Matrix::zeros(dim_x, dim_x); n];
    for i in (0..n.saturating_sub(1)).rev() {
        let (f, q) = (&fs[i + 1], &qs[i + 1]);
        pp[i] = f.dot(&p[i])?.dot(&f.t())?.add(q)?;
        k[i] = p[i].dot(&f.t())?.dot(&pp[i].inv()?)?;
        let dx = k[i].dot(&x[i + 1].sub(&f.dot(&x[i])?)?)?;
        x[i] = x[i].add(&dx)?;
        let dp = k[i].dot(&p[i + 1].sub(&pp[i])?)?.dot(&k[i].t())?;
        p[i] = p[i].add(&dp)?;
    }
    Ok(RtsOutput { x, p, k, pp })
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Constructor defaults and the error paths; the numbers are pinned
    /// against FilterPy in `tests/filterpy_gh1191.rs`.
    #[test]
    fn defaults_and_errors() {
        assert!(KalmanFilter::new(0, 1).is_err() && KalmanFilter::new(1, 0).is_err());
        let mut kf = KalmanFilter::new(2, 1).unwrap();
        assert_eq!(kf.p, Matrix::identity(2));
        assert!(kf.update(&[1.0, 2.0]).is_err());
        kf.h = Matrix::from_rows(&[vec![1.0, 0.0]]).unwrap();
        let out = kf.batch_filter(&[vec![1.0], vec![2.0]], None).unwrap();
        assert_eq!(out.means.len(), 2);
        let sm = kf.rts_smoother(&out.means, &out.covariances).unwrap();
        assert_eq!(sm.x.last(), out.means.last());
        assert!(rts_smoother(&out.means, &out.covariances[..1], &[], &[]).is_err());
    }
}
