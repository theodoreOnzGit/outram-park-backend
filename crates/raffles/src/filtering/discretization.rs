// ---------------------------------------------------------------------------
// Ported from FilterPy.
//
//   Upstream project: FilterPy — Roger R. Labbe Jr
//   Upstream repo:    https://github.com/rlabbe/filterpy
//   Upstream file:    filterpy/common/discretization.py
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
// Translation notes: Q_discrete_white_noise, Q_continuous_white_noise and
// order_by_derivative are ported with Python's `dt**k` as `powf` (both are
// libm `pow`) and scipy's block_diag inlined. van_loan_discretization and
// linear_ode_discretation are NOT ported (they need a matrix exponential).
// ---------------------------------------------------------------------------

//! Process-noise matrices for kinematic models (FilterPy's
//! `Q_discrete_white_noise`, `Q_continuous_white_noise`).

use petir::mathf::RealMath;
use super::numpy_ops::Matrix;
use crate::{RafflesError, Result};

fn table(dim: usize, dt: f64, continuous: bool) -> Result<Vec<Vec<f64>>> {
    // Python's `dt**k` is C `pow(dt, k)`; `black_box` keeps LLVM from
    // folding a constant exponent into multiplies. ~~so this is libm's pow
    // too~~ Since 2026-11-05 (gh:#985) this is PETIR's `powf`, its port of
    // ARM optimized-routines, the `pow` glibc ships: the same bits as the
    // platform on glibc, and the same on every platform.
    let p = |k: f64| dt.r_powf(core::hint::black_box(k));
    let q = match (dim, continuous) {
        (2, false) => vec![vec![0.25 * p(4.0), 0.5 * p(3.0)], vec![0.5 * p(3.0), p(2.0)]],
        (3, false) => vec![
            vec![0.25 * p(4.0), 0.5 * p(3.0), 0.5 * p(2.0)],
            vec![0.5 * p(3.0), p(2.0), dt],
            vec![0.5 * p(2.0), dt, 1.0],
        ],
        (4, false) => vec![
            vec![p(6.0) / 36.0, p(5.0) / 12.0, p(4.0) / 6.0, p(3.0) / 6.0],
            vec![p(5.0) / 12.0, p(4.0) / 4.0, p(3.0) / 2.0, p(2.0) / 2.0],
            vec![p(4.0) / 6.0, p(3.0) / 2.0, p(2.0), dt],
            vec![p(3.0) / 6.0, p(2.0) / 2.0, dt, 1.0],
        ],
        (2, true) => vec![vec![p(3.0) / 3.0, p(2.0) / 2.0], vec![p(2.0) / 2.0, dt]],
        (3, true) => vec![
            vec![p(5.0) / 20.0, p(4.0) / 8.0, p(3.0) / 6.0],
            vec![p(4.0) / 8.0, p(3.0) / 3.0, p(2.0) / 2.0],
            vec![p(3.0) / 6.0, p(2.0) / 2.0, dt],
        ],
        (4, true) => vec![
            vec![p(7.0) / 252.0, p(6.0) / 72.0, p(5.0) / 30.0, p(4.0) / 24.0],
            vec![p(6.0) / 72.0, p(5.0) / 20.0, p(4.0) / 8.0, p(3.0) / 6.0],
            vec![p(5.0) / 30.0, p(4.0) / 8.0, p(3.0) / 3.0, p(2.0) / 2.0],
            vec![p(4.0) / 24.0, p(3.0) / 6.0, p(2.0) / 2.0, dt],
        ],
        _ => {
            return Err(RafflesError::InvalidParameter {
                parameter: "dim".into(),
                value: dim as f64,
                reason: "dim must be between 2 and 4".into(),
            })
        }
    };
    Ok(q)
}

/// `scipy.linalg.block_diag(*[q] * block_size)`.
fn block_diag(q: &[Vec<f64>], block_size: usize) -> Matrix {
    let d = q.len();
    let mut m = Matrix::zeros(d * block_size, d * block_size);
    for b in 0..block_size {
        for (i, row) in q.iter().enumerate() {
            for (j, v) in row.iter().enumerate() {
                m.set(b * d + i, b * d + j, *v);
            }
        }
    }
    m
}

/// FilterPy's `order_by_derivative(Q, dim, block_size)`: `Q`'s entry
/// `(i, j)` becomes the `block_size × block_size` block `eye · Q[i][j]`,
/// so the state is ordered by derivative (x, y, ẋ, ẏ, …).
#[must_use]
pub fn order_by_derivative(q: &[Vec<f64>], dim: usize, block_size: usize) -> Matrix {
    let n = dim * block_size;
    let mut d = Matrix::zeros(n, n);
    for (i, x) in q.iter().flatten().enumerate() {
        let (ix, iy) = ((i / dim) * block_size, (i % dim) * block_size);
        for b in 0..block_size {
            for c in 0..block_size {
                let eye = if b == c { 1.0 } else { 0.0 };
                d.set(ix + b, iy + c, eye * x);
            }
        }
    }
    d
}

fn assemble(q: &[Vec<f64>], dim: usize, block_size: usize, order_by_dim: bool, scale: f64) -> Matrix {
    if order_by_dim {
        block_diag(q, block_size).scale(scale)
    } else {
        order_by_derivative(q, dim, block_size).scale(scale)
    }
}

/// FilterPy's `Q_discrete_white_noise(dim, dt, var, block_size,
/// order_by_dim)`: the piecewise white-noise model of a kinematic state of
/// `dim` derivatives (2, 3 or 4), repeated over `block_size` axes.
/// FilterPy's defaults are `dt = 1`, `var = 1`, `block_size = 1`,
/// `order_by_dim = true`.
///
/// # Errors
/// [`RafflesError::InvalidParameter`] unless `dim ∈ {2, 3, 4}` (upstream's
/// `ValueError`).
pub fn q_discrete_white_noise(dim: usize, dt: f64, var: f64, block_size: usize, order_by_dim: bool) -> Result<Matrix> {
    Ok(assemble(&table(dim, dt, false)?, dim, block_size, order_by_dim, var))
}

/// FilterPy's `Q_continuous_white_noise(dim, dt, spectral_density,
/// block_size, order_by_dim)`.
///
/// # Errors
/// As [`q_discrete_white_noise`].
pub fn q_continuous_white_noise(dim: usize, dt: f64, spectral_density: f64, block_size: usize, order_by_dim: bool) -> Result<Matrix> {
    Ok(assemble(&table(dim, dt, true)?, dim, block_size, order_by_dim, spectral_density))
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The closed forms at dt = 1, var = 2, and the error for dim 5. The
    /// bit-identity with FilterPy over 108 calls is in
    /// `tests/filterpy_gh1191.rs`.
    #[test]
    fn closed_forms_and_ordering() {
        let q = q_discrete_white_noise(2, 1.0, 2.0, 1, true).unwrap();
        assert_eq!(q.to_rows(), vec![vec![0.5, 1.0], vec![1.0, 2.0]]);
        let q2 = q_discrete_white_noise(2, 1.0, 1.0, 2, false).unwrap();
        assert_eq!(q2.get(0, 2), 0.5);
        assert_eq!(q2.get(1, 3), 0.5);
        assert_eq!(q2.get(0, 1), 0.0);
        assert!(q_discrete_white_noise(5, 1.0, 1.0, 1, true).is_err());
        assert!(q_continuous_white_noise(1, 1.0, 1.0, 1, true).is_err());
        assert_eq!(q_continuous_white_noise(2, 1.0, 1.0, 1, true).unwrap().get(1, 1), 1.0);
    }
}
