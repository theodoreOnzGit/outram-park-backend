// SPDX-License-Identifier: GPL-3.0-only
// Copyright (C) 2026 OUTRAM PARK contributors
//
// Derived from GeN-Foam (Generalized Nuclear Foam)
//   Upstream: https://gitlab.com/foam-for-nuclear/GeN-Foam
//   Upstream source: src/classes/common/InterpolateTable/
//                    InterpolateTableGF.H, InterpolateTablesGF.{C,H}
//   Upstream commit: 652b3da (GeN-Foam), built on OpenFOAM v2506
//   Upstream copyright: (C) 2021 OpenFOAM Foundation / (C) 2015-2022 EPFL
//     Principal author: I. Clifford (PSI); contributions A. Scolaro, E. Brunetto,
//     C. Fiorina (EPFL); out-of-bounds modes F. Xiang (EPFL & XJTU)
//   Upstream license: GPL-3.0
//
// This file is part of OUTRAM PARK. OUTRAM PARK is free software: you can
// redistribute it and/or modify it under the terms of the GNU General Public
// License as published by the Free Software Foundation, either version 3 of
// the License, or (at your option) any later version. It is distributed in
// the hope that it will be useful, but WITHOUT ANY WARRANTY; without even the
// implied warranty of MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE.
// See the GNU General Public License for more details.
//
// This offering is not approved or endorsed by EPFL, the OpenFOAM Foundation,
// nor OpenCFD Limited, producer and distributor of the OpenFOAM(R) software.

//! # `InterpolateTableGF` over fields and field-fields (gh:#1223)
//!
//! [`super::interpolate_table::ScalarInterpolateTable`] ports the 1-D instantiation
//! (`InterpolateTableGF<scalarField, scalar, scalar>`) of upstream's header-only template. The
//! 2-D and 3-D instantiations upstream builds in `InterpolateTablesGF.{H,C}` return a WHOLE
//! field / field-field per query: `scalarFieldInterpolateTableGF` (a table of `Field`s, result a
//! `Field`) and `scalarFieldFieldInterpolateTableGF` (a table of `FieldField`s, result a
//! `FieldField`). `lookUpTableCHF` is the only consumer: it interpolates the 3-D table in
//! pressure to a 2-D table, that one in mass flux to a 1-D table, and that one in quality.
//!
//! [`interpolate_table`] is one generic function over [`TableValue`] (`f64`, `Vec<f64>`,
//! `Vec<Vec<f64>>`), translating `operator()(scalar x)` line by line, including the order of the
//! search (`x >= xValues_[0]`, then the first `i` with `x <= xValues_[i]`: at a knot the upper
//! interval is used with `alpha = 1`) and the arithmetic (`(1-alpha)*data[i-1] + alpha*data[i]`,
//! evaluated per element exactly as OpenFOAM's field expressions do: two products, one sum).
//!
//! The 1-D [`ScalarInterpolateTable`](super::interpolate_table::ScalarInterpolateTable) finds the
//! interval by `partition_point` and returns the same numbers; this one keeps upstream's linear
//! walk, so the two are a cross-check of each other.
//!
//! Upstream's default out-of-bounds policy is `ERROR` (a `FatalError`); here it is
//! [`InterpolateTableError::OutOfBounds`].

#[cfg(not(any(feature = "std", test)))]
#[allow(unused_imports)]
use crate::nostd_prelude::*;

use super::interpolate_table::{InterpolateTableError, InterpolationMethod, OutOfBounds};

/// A value a table can hold: a scalar, a `Field` or a `FieldField`.
pub trait TableValue: Clone {
    /// `(1-alpha)*a + alpha*b`, element by element.
    fn blend(a: &Self, b: &Self, alpha: f64) -> Self;
    /// `(d1-d0)/(x1-x0)*(x-x0)+d0`, element by element (the `EXTRAPOLATION` policy).
    fn extrapolate(d0: &Self, d1: &Self, x0: f64, x1: f64, x: f64) -> Self;
}

impl TableValue for f64 {
    fn blend(a: &Self, b: &Self, alpha: f64) -> Self {
        (1.0 - alpha) * *a + alpha * *b
    }
    fn extrapolate(d0: &Self, d1: &Self, x0: f64, x1: f64, x: f64) -> Self {
        (*d1 - *d0) / (x1 - x0) * (x - x0) + *d0
    }
}

impl<T: TableValue> TableValue for Vec<T> {
    fn blend(a: &Self, b: &Self, alpha: f64) -> Self {
        a.iter().zip(b).map(|(p, q)| T::blend(p, q, alpha)).collect()
    }
    fn extrapolate(d0: &Self, d1: &Self, x0: f64, x1: f64, x: f64) -> Self {
        d0.iter().zip(d1).map(|(p, q)| T::extrapolate(p, q, x0, x1, x)).collect()
    }
}

/// `InterpolateTableGF::check`: at least two ordinates, as many data as ordinates, ascending.
///
/// # Errors
/// [`InterpolateTableError::LengthMismatch`], [`InterpolateTableError::TooFewPoints`] or
/// [`InterpolateTableError::NonAscendingAbscissae`].
pub fn check_table(xs: &[f64], n_data: usize) -> Result<(), InterpolateTableError> {
    if xs.len() != n_data {
        return Err(InterpolateTableError::LengthMismatch { xs: xs.len(), ys: n_data });
    }
    if xs.len() < 2 {
        return Err(InterpolateTableError::TooFewPoints(xs.len()));
    }
    for i in 1..xs.len() {
        if xs[i] <= xs[i - 1] {
            return Err(InterpolateTableError::NonAscendingAbscissae { index: i, prev: xs[i - 1], next: xs[i] });
        }
    }
    Ok(())
}

/// `InterpolateTableGF::operator()(x)` for any [`TableValue`].
///
/// `xs` and `data` must satisfy [`check_table`] (the caller checks once, as upstream's constructor
/// does).
///
/// # Errors
/// [`InterpolateTableError::OutOfBounds`] outside `[xs[0], xs[n-1]]` under [`OutOfBounds::Error`].
pub fn interpolate_table<T: TableValue>(
    xs: &[f64],
    data: &[T],
    method: InterpolationMethod,
    out_of_bounds: OutOfBounds,
    x: f64,
) -> Result<T, InterpolateTableError> {
    let n = xs.len();
    if x >= xs[0] {
        for i in 1..n {
            if x <= xs[i] {
                return Ok(match method {
                    InterpolationMethod::Step => data[i - 1].clone(),
                    InterpolationMethod::Linear => {
                        let x0 = xs[i - 1];
                        let x1 = xs[i];
                        let alpha = (x - x0) / (x1 - x0);
                        T::blend(&data[i - 1], &data[i], alpha)
                    }
                });
            }
        }
    }
    match out_of_bounds {
        OutOfBounds::Error => Err(InterpolateTableError::OutOfBounds { x, lo: xs[0], hi: xs[n - 1] }),
        OutOfBounds::Fixed => Ok(if x < xs[0] { data[0].clone() } else { data[n - 1].clone() }),
        OutOfBounds::Extrapolation => Ok(if x < xs[0] {
            T::extrapolate(&data[0], &data[1], xs[0], xs[1], x)
        } else {
            T::extrapolate(&data[n - 2], &data[n - 1], xs[n - 2], xs[n - 1], x)
        }),
    }
}
