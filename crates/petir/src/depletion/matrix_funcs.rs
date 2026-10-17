// SPDX-License-Identifier: GPL-3.0-only
// Copyright (C) 2026 Theodore Ong and the outram-park contributors.
//
// ---------------------------------------------------------------------------
// PORTED from OpenMC `openmc/deplete/_matrix_funcs.py` (lines 1-80), commit
// d7d3284a1b13d7cae020e0d8fea9f1e60d91b18b, MIT -- "Copyright (c) 2011-2026
// Massachusetts Institute of Technology, UChicago Argonne LLC, and OpenMC
// contributors"; full notice in NOTICE. Ported 2026-10-17 (gh:#950).
// ---------------------------------------------------------------------------

//! The special burnup matrices the higher-order integrators exponentiate —
//! OpenMC `openmc/deplete/_matrix_funcs.py`.
//!
//! Upstream's functions take `(chain, rates, fission_yields)` and call
//! `chain.form_matrix` inside. Here the caller forms the matrices (that is
//! the operator's job, see [`super::TransportOperator::form_matrix`]) and
//! passes them in; the arithmetic on them is upstream's, expression for
//! expression and in the same left-to-right order, through
//! [`lincomb`](super::matrix::lincomb).
//!
//! Every scalar coefficient is computed exactly as Python computes it from
//! the source text (`5 / 12` is the double nearest 5/12, `-5 * dt / (12 *
//! dt_l)` is `((-5 * dt) / (12 * dt_l))`, and so on).
//!
//! **DEVIATION (`**` on floats):** upstream's `dt ** 2` and `dt_l ** 2` call
//! the C `pow`; these use `dt * dt`. `pow(x, 2.0)` and `x * x` agree whenever
//! the platform `pow` is correctly rounded at exponent 2, which glibc's is in
//! practice; `tests/depletion_vs_openmc.rs` checks the outcome bit for bit on
//! the steps it runs.

use super::matrix::{lincomb, DepletionMatrix, Sign};
use crate::Result;

/// `celi_f1` (`_matrix_funcs.py:4-6`): `5/12 * M0 + 1/12 * M1`.
///
/// # Errors
///
/// Matrices of different order.
pub fn celi_f1(m0: &DepletionMatrix, m1: &DepletionMatrix) -> Result<DepletionMatrix> {
    lincomb((5.0 / 12.0, m0), &[(Sign::Plus, 1.0 / 12.0, m1)])
}

/// `celi_f2` (`_matrix_funcs.py:9-11`): `1/12 * M0 + 5/12 * M1`.
///
/// # Errors
///
/// Matrices of different order.
pub fn celi_f2(m0: &DepletionMatrix, m1: &DepletionMatrix) -> Result<DepletionMatrix> {
    lincomb((1.0 / 12.0, m0), &[(Sign::Plus, 5.0 / 12.0, m1)])
}

/// `cf4_f1` (`_matrix_funcs.py:14-15`): `1/2 * M`.
pub fn cf4_f1(m: &DepletionMatrix) -> DepletionMatrix {
    m.scaled(1.0 / 2.0)
}

/// `cf4_f2` (`_matrix_funcs.py:18-20`): `-1/2 * M0 + M1`.
///
/// # Errors
///
/// Matrices of different order.
pub fn cf4_f2(m0: &DepletionMatrix, m1: &DepletionMatrix) -> Result<DepletionMatrix> {
    // `+ chain.form_matrix(rates[1])` is an unscaled add; `1.0 * v == v`
    // exactly, so the coefficient 1 changes no bit.
    lincomb((-1.0 / 2.0, m0), &[(Sign::Plus, 1.0, m1)])
}

/// `cf4_f3` (`_matrix_funcs.py:23-27`):
/// `1/4 * M0 + 1/6 * M1 + 1/6 * M2 - 1/12 * M3`.
///
/// # Errors
///
/// Matrices of different order.
pub fn cf4_f3(
    m0: &DepletionMatrix,
    m1: &DepletionMatrix,
    m2: &DepletionMatrix,
    m3: &DepletionMatrix,
) -> Result<DepletionMatrix> {
    lincomb(
        (1.0 / 4.0, m0),
        &[
            (Sign::Plus, 1.0 / 6.0, m1),
            (Sign::Plus, 1.0 / 6.0, m2),
            (Sign::Minus, 1.0 / 12.0, m3),
        ],
    )
}

/// `cf4_f4` (`_matrix_funcs.py:30-34`):
/// `-1/12 * M0 + 1/6 * M1 + 1/6 * M2 + 1/4 * M3`.
///
/// # Errors
///
/// Matrices of different order.
pub fn cf4_f4(
    m0: &DepletionMatrix,
    m1: &DepletionMatrix,
    m2: &DepletionMatrix,
    m3: &DepletionMatrix,
) -> Result<DepletionMatrix> {
    lincomb(
        (-1.0 / 12.0, m0),
        &[
            (Sign::Plus, 1.0 / 6.0, m1),
            (Sign::Plus, 1.0 / 6.0, m2),
            (Sign::Plus, 1.0 / 4.0, m3),
        ],
    )
}

/// `rk4_f1` (`_matrix_funcs.py:37-38`): `1/2 * M`.
pub fn rk4_f1(m: &DepletionMatrix) -> DepletionMatrix {
    m.scaled(1.0 / 2.0)
}

/// `rk4_f4` (`_matrix_funcs.py:41-45`):
/// `1/6 * M0 + 1/3 * M1 + 1/3 * M2 + 1/6 * M3`.
///
/// # Errors
///
/// Matrices of different order.
pub fn rk4_f4(
    m0: &DepletionMatrix,
    m1: &DepletionMatrix,
    m2: &DepletionMatrix,
    m3: &DepletionMatrix,
) -> Result<DepletionMatrix> {
    lincomb(
        (1.0 / 6.0, m0),
        &[
            (Sign::Plus, 1.0 / 3.0, m1),
            (Sign::Plus, 1.0 / 3.0, m2),
            (Sign::Plus, 1.0 / 6.0, m3),
        ],
    )
}

/// `leqi_f1` (`_matrix_funcs.py:48-52`), with `f1 = M(prev)`, `f2 = M(bos)`:
/// `-dt / (12 * dt_l) * f1 + (dt + 6 * dt_l) / (12 * dt_l) * f2`.
///
/// # Errors
///
/// Matrices of different order.
pub fn leqi_f1(
    f1: &DepletionMatrix,
    f2: &DepletionMatrix,
    dt_l: f64,
    dt: f64,
) -> Result<DepletionMatrix> {
    let c1 = -dt / (12.0 * dt_l);
    let c2 = (dt + 6.0 * dt_l) / (12.0 * dt_l);
    lincomb((c1, f1), &[(Sign::Plus, c2, f2)])
}

/// `leqi_f2` (`_matrix_funcs.py:55-59`):
/// `-5 * dt / (12 * dt_l) * f1 + (5 * dt + 6 * dt_l) / (12 * dt_l) * f2`.
///
/// # Errors
///
/// Matrices of different order.
pub fn leqi_f2(
    f1: &DepletionMatrix,
    f2: &DepletionMatrix,
    dt_l: f64,
    dt: f64,
) -> Result<DepletionMatrix> {
    let c1 = -5.0 * dt / (12.0 * dt_l);
    let c2 = (5.0 * dt + 6.0 * dt_l) / (12.0 * dt_l);
    lincomb((c1, f1), &[(Sign::Plus, c2, f2)])
}

/// `leqi_f3` (`_matrix_funcs.py:62-69`), with `f3 = M(predicted EOS)`:
///
/// ```text
/// -dt**2 / (12 * dt_l * (dt + dt_l)) * f1
///   + (dt**2 + 6 * dt * dt_l + 5 * dt_l**2) / (12 * dt_l * (dt + dt_l)) * f2
///   + dt_l / (12 * (dt + dt_l)) * f3
/// ```
///
/// # Errors
///
/// Matrices of different order.
pub fn leqi_f3(
    f1: &DepletionMatrix,
    f2: &DepletionMatrix,
    f3: &DepletionMatrix,
    dt_l: f64,
    dt: f64,
) -> Result<DepletionMatrix> {
    let dt2 = dt * dt;
    let dt_l2 = dt_l * dt_l;
    let c1 = -dt2 / (12.0 * dt_l * (dt + dt_l));
    let c2 = (dt2 + 6.0 * dt * dt_l + 5.0 * dt_l2) / (12.0 * dt_l * (dt + dt_l));
    let c3 = dt_l / (12.0 * (dt + dt_l));
    lincomb((c1, f1), &[(Sign::Plus, c2, f2), (Sign::Plus, c3, f3)])
}

/// `leqi_f4` (`_matrix_funcs.py:72-80`):
///
/// ```text
/// -dt**2 / (12 * dt_l * (dt + dt_l)) * f1
///   + (dt**2 + 2 * dt * dt_l + dt_l**2) / (12 * dt_l * (dt + dt_l)) * f2
///   + (4 * dt + 5 * dt_l) / (12 * (dt + dt_l)) * f3
/// ```
///
/// # Errors
///
/// Matrices of different order.
pub fn leqi_f4(
    f1: &DepletionMatrix,
    f2: &DepletionMatrix,
    f3: &DepletionMatrix,
    dt_l: f64,
    dt: f64,
) -> Result<DepletionMatrix> {
    let dt2 = dt * dt;
    let dt_l2 = dt_l * dt_l;
    let c1 = -dt2 / (12.0 * dt_l * (dt + dt_l));
    let c2 = (dt2 + 2.0 * dt * dt_l + dt_l2) / (12.0 * dt_l * (dt + dt_l));
    let c3 = (4.0 * dt + 5.0 * dt_l) / (12.0 * (dt + dt_l));
    lincomb((c1, f1), &[(Sign::Plus, c2, f2), (Sign::Plus, c3, f3)])
}

#[cfg(test)]
mod tests {
    use super::*;

    fn one(v: f64) -> DepletionMatrix {
        let mut m = DepletionMatrix::zeros(1);
        m.set(0, 0, v).unwrap();
        m
    }

    /// For a constant matrix every combination must sum to the right
    /// multiple of it: the coefficients of each two-exponential pair sum to
    /// 1 (CF4, CE/LI, LE/QI), RK4's to 1, CF4's stage 3 to 1/2.
    #[test]
    fn coefficient_sums() {
        let m = one(1.0);
        let s = |r: Result<DepletionMatrix>| r.unwrap().get(0, 0);
        assert!((s(celi_f1(&m, &m)) + s(celi_f2(&m, &m)) - 1.0).abs() < 1e-15);
        assert!((s(cf4_f3(&m, &m, &m, &m)) + s(cf4_f4(&m, &m, &m, &m)) - 1.0).abs() < 1e-15);
        assert!((s(rk4_f4(&m, &m, &m, &m)) - 1.0).abs() < 1e-15);
        assert_eq!(cf4_f1(&m).get(0, 0), 0.5);
        assert_eq!(rk4_f1(&m).get(0, 0), 0.5);
        assert_eq!(s(cf4_f2(&m, &m)), 0.5);
        for (dt_l, dt) in [(1.0, 1.0), (2.0, 0.5), (0.3, 7.0)] {
            assert!(
                (s(leqi_f1(&m, &m, dt_l, dt)) + s(leqi_f2(&m, &m, dt_l, dt)) - 1.0).abs() < 1e-14
            );
            assert!(
                (s(leqi_f3(&m, &m, &m, dt_l, dt)) + s(leqi_f4(&m, &m, &m, dt_l, dt)) - 1.0).abs()
                    < 1e-14
            );
        }
        assert!(celi_f1(&m, &DepletionMatrix::zeros(2)).is_err());
    }

    /// The LE/QI coefficients at `dt_l == dt` are the values the upstream
    /// docstring's formulas give with `h_{i-1} = h_i` (`integrators.py:330-342`).
    #[test]
    fn leqi_equal_steps() {
        let (a, b, c) = (one(1.0), one(0.0), one(0.0));
        assert!((leqi_f1(&a, &b, 1.0, 1.0).unwrap().get(0, 0) - (-1.0 / 12.0)).abs() < 1e-16);
        assert!((leqi_f2(&a, &b, 1.0, 1.0).unwrap().get(0, 0) - (-5.0 / 12.0)).abs() < 1e-16);
        assert!((leqi_f3(&a, &b, &c, 1.0, 1.0).unwrap().get(0, 0) - (-1.0 / 24.0)).abs() < 1e-16);
        assert!((leqi_f3(&b, &b, &a, 1.0, 1.0).unwrap().get(0, 0) - (1.0 / 24.0)).abs() < 1e-16);
        assert!((leqi_f4(&b, &a, &b, 1.0, 1.0).unwrap().get(0, 0) - (4.0 / 24.0)).abs() < 1e-16);
        assert!((leqi_f4(&b, &b, &a, 1.0, 1.0).unwrap().get(0, 0) - (9.0 / 24.0)).abs() < 1e-16);
    }
}
