// ---------------------------------------------------------------------------
// Ported from FilterPy.
//
//   Upstream project: FilterPy — Roger R. Labbe Jr
//   Upstream repo:    https://github.com/rlabbe/filterpy
//   Upstream file:    filterpy/kalman/EKF.py
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
// Translation notes: predict (linear, x = F x + B u with upstream's B = 0)
// and update (the caller's HJacobian and Hx, Joseph form) are ported. The
// residual is z - h(x) (upstream's default np.subtract). predict_x_with is
// NOT upstream: FilterPy asks the user to override predict_x for a nonlinear
// process; here the caller passes it. Upstream inverts S with
// scipy.linalg.inv (getrf + getri), which rounds differently from numpy's
// inv for S larger than 1 x 1; the port uses numpy's order throughout (see
// the V&V record, section 2.3). predict_update and the likelihood properties
// are not ported.
// ---------------------------------------------------------------------------

//! The extended Kalman filter (FilterPy's `ExtendedKalmanFilter`).

use super::numpy_ops::Matrix;
use crate::{RafflesError, Result};

/// FilterPy's `ExtendedKalmanFilter`. `x` is a `dim_x × 1` column.
#[derive(Debug, Clone, PartialEq)]
pub struct ExtendedKalmanFilter {
    /// State dimension.
    pub dim_x: usize,
    /// Measurement dimension.
    pub dim_z: usize,
    /// State estimate.
    pub x: Matrix,
    /// State covariance.
    pub p: Matrix,
    /// State transition (or the process Jacobian, for [`Self::predict_x_with`]).
    pub f: Matrix,
    /// Process noise.
    pub q: Matrix,
    /// Measurement noise.
    pub r: Matrix,
    /// Gain of the last update.
    pub k: Matrix,
    /// Residual of the last update.
    pub y: Matrix,
    /// System uncertainty of the last update.
    pub s: Matrix,
    /// Its inverse.
    pub si: Matrix,
    /// Prior.
    pub x_prior: Matrix,
    /// Prior covariance.
    pub p_prior: Matrix,
    /// Posterior.
    pub x_post: Matrix,
    /// Posterior covariance.
    pub p_post: Matrix,
}

impl ExtendedKalmanFilter {
    /// Upstream's constructor: `x = 0`, `P = F = Q = I`, `R = I`.
    ///
    /// # Errors
    /// [`RafflesError::InvalidParameter`] if a dimension is 0.
    pub fn new(dim_x: usize, dim_z: usize) -> Result<Self> {
        if dim_x == 0 || dim_z == 0 {
            return Err(RafflesError::InvalidParameter { parameter: "dim".into(), value: 0.0, reason: "dimensions must be 1 or more".into() });
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
            f: i.clone(),
            q: i,
            r: Matrix::identity(dim_z),
            k: Matrix::zeros(dim_x, 1),
            y: Matrix::zeros(dim_z, 1),
            s: Matrix::zeros(dim_z, dim_z),
            si: Matrix::zeros(dim_z, dim_z),
        })
    }

    /// Upstream's `predict()`: `x = F x + 0` (its `B = 0`, `u = 0`), then
    /// `P = F P Fᵀ + Q`.
    ///
    /// # Errors
    /// On a dimension mismatch.
    pub fn predict(&mut self) -> Result<()> {
        let fx = self.f.dot(&self.x)?;
        self.x = fx.add(&Matrix::zeros(self.dim_x, 1))?;
        self.finish_predict()
    }

    /// A nonlinear prediction (not upstream, which asks for an overridden
    /// `predict_x`): `x = fx(x)`, then `P = F P Fᵀ + Q` with the caller's
    /// Jacobian already in `self.f`.
    ///
    /// # Errors
    /// If `fx` returns the wrong length, or on a dimension mismatch.
    pub fn predict_x_with<Fx: Fn(&[f64]) -> Result<Vec<f64>>>(&mut self, fx: Fx) -> Result<()> {
        let x = fx(&self.x.col(0))?;
        if x.len() != self.dim_x {
            return Err(RafflesError::DimensionMismatch { expected: self.dim_x, found: x.len() });
        }
        self.x = Matrix::column(&x);
        self.finish_predict()
    }

    fn finish_predict(&mut self) -> Result<()> {
        self.p = self.f.dot(&self.p)?.dot(&self.f.t())?.add(&self.q)?;
        self.x_prior = self.x.clone();
        self.p_prior = self.p.clone();
        Ok(())
    }

    /// Upstream's `update(z, HJacobian, Hx)`: `H = HJacobian(x)`,
    /// `S = H P Hᵀ + R`, `K = P Hᵀ S⁻¹`, `y = z − Hx(x)`, `x = x + K y`,
    /// Joseph-form `P`.
    ///
    /// # Errors
    /// On a dimension mismatch, a singular `S`, or an error from the closures.
    pub fn update<J, Hx>(&mut self, z: &[f64], h_jacobian: J, hx: Hx) -> Result<()>
    where
        J: Fn(&[f64]) -> Result<Matrix>,
        Hx: Fn(&[f64]) -> Result<Vec<f64>>,
    {
        if z.len() != self.dim_z {
            return Err(RafflesError::DimensionMismatch { expected: self.dim_z, found: z.len() });
        }
        let xv = self.x.col(0);
        let h = h_jacobian(&xv)?;
        let pht = self.p.dot(&h.t())?;
        self.s = h.dot(&pht)?.add(&self.r)?;
        self.si = self.s.inv()?;
        self.k = pht.dot(&self.si)?;
        let hxv = hx(&xv)?;
        if hxv.len() != self.dim_z {
            return Err(RafflesError::DimensionMismatch { expected: self.dim_z, found: hxv.len() });
        }
        self.y = Matrix::column(z).sub(&Matrix::column(&hxv))?;
        self.x = self.x.add(&self.k.dot(&self.y)?)?;
        let i_kh = Matrix::identity(self.dim_x).sub(&self.k.dot(&h)?)?;
        self.p = i_kh.dot(&self.p)?.dot(&i_kh.t())?.add(&self.k.dot(&self.r)?.dot(&self.k.t())?)?;
        self.x_post = self.x.clone();
        self.p_post = self.p.clone();
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A linear EKF equals the KF bit for bit (same algebra, same order),
    /// and the nonlinear predict and the error paths work.
    #[test]
    fn a_linear_ekf_is_the_kf() {
        let f = Matrix::from_rows(&[vec![1.0, 0.1], vec![0.0, 1.0]]).unwrap();
        let h = Matrix::from_rows(&[vec![1.0, 0.0]]).unwrap();
        let mut kf = super::super::kalman::KalmanFilter::new(2, 1).unwrap();
        let mut ekf = ExtendedKalmanFilter::new(2, 1).unwrap();
        kf.f = f.clone();
        kf.h = h.clone();
        ekf.f = f.clone();
        for z in [0.3, 0.9, 1.4, 2.2] {
            kf.predict(None).unwrap();
            kf.update(&[z]).unwrap();
            ekf.predict().unwrap();
            let hh = h.clone();
            ekf.update(&[z], |_| Ok(hh.clone()), |x| Ok(vec![x[0]])).unwrap();
            assert_eq!(kf.x, ekf.x);
            assert_eq!(kf.p, ekf.p);
        }
        let before = ekf.x.clone();
        ekf.f = Matrix::identity(2);
        ekf.predict_x_with(|x| Ok(x.to_vec())).unwrap();
        assert_eq!(ekf.x, before);
        assert!(ekf.predict_x_with(|_| Ok(vec![1.0])).is_err());
        assert!(ekf.update(&[1.0, 2.0], |_| Ok(h.clone()), |x| Ok(vec![x[0]])).is_err());
        assert!(ekf.update(&[1.0], |_| Ok(h.clone()), |_| Ok(vec![])).is_err());
        assert!(ExtendedKalmanFilter::new(0, 1).is_err());
    }
}
