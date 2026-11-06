// ---------------------------------------------------------------------------
// Ported from FilterPy.
//
//   Upstream project: FilterPy — Roger R. Labbe Jr
//   Upstream repo:    https://github.com/rlabbe/filterpy
//   Upstream file:    filterpy/kalman/sigma_points.py
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
// Translation notes: MerweScaledSigmaPoints with upstream's defaults (scipy's
// upper Cholesky as the square root, np.subtract as the subtraction) is
// ported; Python's alpha**2 is libm pow, as here. Custom sqrt_method /
// subtract callables, JulierSigmaPoints and SimplexSigmaPoints are not ported.
// ---------------------------------------------------------------------------

//! Van der Merwe's scaled sigma points (FilterPy's `MerweScaledSigmaPoints`).

use petir::mathf::RealMath;
use super::numpy_ops::Matrix;
use crate::{RafflesError, Result};

/// FilterPy's `MerweScaledSigmaPoints(n, alpha, beta, kappa)`.
#[derive(Debug, Clone, PartialEq)]
pub struct MerweScaledSigmaPoints {
    /// State dimension.
    pub n: usize,
    /// Spread α.
    pub alpha: f64,
    /// Prior-knowledge β (2 for a Gaussian).
    pub beta: f64,
    /// Secondary scaling κ (often 3 − n).
    pub kappa: f64,
    /// Mean weights.
    pub wm: Vec<f64>,
    /// Covariance weights.
    pub wc: Vec<f64>,
}

impl MerweScaledSigmaPoints {
    /// Upstream's constructor and `_compute_weights`:
    /// `λ = α²(n + κ) − n`, `Wm = Wc = 0.5/(n + λ)`, `Wm₀ = λ/(n + λ)`,
    /// `Wc₀ = Wm₀ + (1 − α² + β)`.
    #[must_use]
    pub fn new(n: usize, alpha: f64, beta: f64, kappa: f64) -> Self {
        let nf = n as f64;
        let a2 = alpha.r_powf(core::hint::black_box(2.0));
        let lambda = a2 * (nf + kappa) - nf;
        let c = 0.5 / (nf + lambda);
        let mut wc = vec![c; 2 * n + 1];
        let mut wm = vec![c; 2 * n + 1];
        wc[0] = lambda / (nf + lambda) + (1.0 - a2 + beta);
        wm[0] = lambda / (nf + lambda);
        Self { n, alpha, beta, kappa, wm, wc }
    }

    /// `2n + 1`.
    #[must_use]
    pub fn num_sigmas(&self) -> usize {
        2 * self.n + 1
    }

    /// Upstream's `sigma_points(x, P)`: `U = cholesky((λ + n) P)` (upper),
    /// row 0 is `x`, row `k+1` is `x − (−U[k])`, row `n+k+1` is `x − U[k]`.
    ///
    /// # Errors
    /// [`RafflesError::DimensionMismatch`] if `x` or `P` is not of size n;
    /// [`RafflesError::InvalidParameter`] if `(λ + n) P` is not positive
    /// definite.
    pub fn sigma_points(&self, x: &[f64], p: &Matrix) -> Result<Matrix> {
        let n = self.n;
        if x.len() != n || p.rows() != n || p.cols() != n {
            return Err(RafflesError::DimensionMismatch { expected: n, found: x.len() });
        }
        let nf = n as f64;
        let lambda = self.alpha.r_powf(core::hint::black_box(2.0)) * (nf + self.kappa) - nf;
        let u = p.scale(lambda + nf).cholesky_upper()?;
        let mut s = Matrix::zeros(2 * n + 1, n);
        s.set_row(0, x)?;
        for k in 0..n {
            let row = u.row(k);
            let plus: Vec<f64> = x.iter().zip(row).map(|(a, b)| a - (-b)).collect();
            let minus: Vec<f64> = x.iter().zip(row).map(|(a, b)| a - b).collect();
            s.set_row(k + 1, &plus)?;
            s.set_row(n + k + 1, &minus)?;
        }
        Ok(s)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Weights sum to 1 and the points reproduce the mean and covariance
    /// they were drawn from (the unscented transform's defining property).
    #[test]
    fn points_reproduce_their_moments() {
        let pts = MerweScaledSigmaPoints::new(2, 0.5, 2.0, 1.0);
        assert!((pts.wm.iter().sum::<f64>() - 1.0).abs() < 1e-14);
        assert_eq!(pts.num_sigmas(), 5);
        let p = Matrix::from_rows(&[vec![2.0, 0.3], vec![0.3, 1.0]]).unwrap();
        let s = pts.sigma_points(&[1.0, -1.0], &p).unwrap();
        for j in 0..2 {
            let m: f64 = (0..5).map(|i| pts.wm[i] * s.get(i, j)).sum();
            assert!((m - [1.0, -1.0][j]).abs() < 1e-13);
        }
        let mut c = 0.0;
        for i in 0..5 {
            c += pts.wc[i] * (s.get(i, 0) - 1.0) * (s.get(i, 1) + 1.0);
        }
        assert!((c - 0.3).abs() < 1e-13, "{c}");
        assert!(pts.sigma_points(&[1.0], &p).is_err());
    }
}
