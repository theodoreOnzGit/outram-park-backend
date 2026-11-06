// SPDX-License-Identifier: AGPL-3.0-only
// Copyright (C) 2026 OUTRAM PARK contributors
//
// NOT ported from FilterPy (gh:#1191): the standard filter-consistency
// statistics (Bar-Shalom, Li & Kirubarajan 2001, *Estimation with
// Applications to Tracking and Navigation*, section 5.4).

//! Filter consistency: NEES and NIS.
//!
//! - **NEES** `ε = (x̂ − x)ᵀ P⁻¹ (x̂ − x)` needs the truth, so it exists only
//!   in a twin experiment. For a consistent filter its time average is χ²
//!   with `n` degrees of freedom divided by the number of steps.
//! - **NIS** `ν = yᵀ S⁻¹ y` needs only the innovation. It is the check an
//!   operator could actually run, and it **cannot see** an error that the
//!   observations do not carry (the double-pipe pilot, gh:#1191, shows one).

use super::numpy_ops::{ddot, Matrix};
use crate::{RafflesError, Result};

fn quadratic(v: &[f64], m: &Matrix) -> Result<f64> {
    if m.rows() != v.len() {
        return Err(RafflesError::DimensionMismatch { expected: m.rows(), found: v.len() });
    }
    let w = m.inv()?.dot_vec(v)?;
    Ok(ddot(v, &w))
}

/// NEES of an estimate `x_hat` with covariance `p` against the truth.
///
/// # Errors
/// On a size mismatch or a singular `p`.
pub fn nees(x_hat: &[f64], truth: &[f64], p: &Matrix) -> Result<f64> {
    if x_hat.len() != truth.len() {
        return Err(RafflesError::DimensionMismatch { expected: x_hat.len(), found: truth.len() });
    }
    let e: Vec<f64> = x_hat.iter().zip(truth).map(|(a, b)| a - b).collect();
    quadratic(&e, p)
}

/// NIS of an innovation `y` with covariance `s`.
///
/// # Errors
/// On a size mismatch or a singular `s`.
pub fn nis(y: &[f64], s: &Matrix) -> Result<f64> {
    quadratic(y, s)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn quadratic_forms() {
        let p = Matrix::diagonal(&[4.0, 0.25]);
        assert!((nees(&[2.0, 1.0], &[0.0, 0.5], &p).unwrap() - 2.0).abs() < 1e-15);
        assert!((nis(&[3.0], &Matrix::diagonal(&[9.0])).unwrap() - 1.0).abs() < 1e-15);
        assert!(nees(&[1.0], &[1.0, 2.0], &p).is_err() && nis(&[1.0], &p).is_err());
    }
}
