// SPDX-License-Identifier: AGPL-3.0-only
// Copyright (C) 2026 Theodore Ong and the outram-park contributors.
//
// PORTED from the `faer` crate, version 0.24.4, file
// `src/sparse/linalg/triangular_solve.rs`, read from the published crates.io
// source on 2026-10-20.
//
//     https://codeberg.org/sarah-quinones/faer
//     Copyright (c) sarah quiñones <sarah@veganb.tw> and the faer contributors
//     Licensed MIT (faer's Cargo.toml `license = "MIT"`; the published crate
//     ships no LICENSE file, so the standard MIT text is reproduced here).
//
// Its notice, retained as that licence requires of a redistribution in source
// form:
//
//   Permission is hereby granted, free of charge, to any person obtaining a
//   copy of this software and associated documentation files (the
//   "Software"), to deal in the Software without restriction, including
//   without limitation the rights to use, copy, modify, merge, publish,
//   distribute, sublicense, and/or sell copies of the Software, and to permit
//   persons to whom the Software is furnished to do so, subject to the
//   following conditions:
//
//   The above copyright notice and this permission notice shall be included in
//   all copies or substantial portions of the Software.
//
//   THE SOFTWARE IS PROVIDED "AS IS", WITHOUT WARRANTY OF ANY KIND, EXPRESS OR
//   IMPLIED, INCLUDING BUT NOT LIMITED TO THE WARRANTIES OF MERCHANTABILITY,
//   FITNESS FOR A PARTICULAR PURPOSE AND NONINFRINGEMENT. IN NO EVENT SHALL
//   THE AUTHORS OR COPYRIGHT HOLDERS BE LIABLE FOR ANY CLAIM, DAMAGES OR OTHER
//   LIABILITY, WHETHER IN AN ACTION OF CONTRACT, TORT OR OTHERWISE, ARISING
//   FROM, OUT OF OR IN CONNECTION WITH THE SOFTWARE OR THE USE OR OTHER
//   DEALINGS IN THE SOFTWARE.
//
// MIT into AGPL-3.0-only is a ONE-WAY flow. Maintainer direction for this
// port: gh #994, 2026-10-20. See the crate NOTICE.
//
// Ported items: the single right-hand-side arm (`(_, _, _) =>`) of
//   solve_lower_triangular_in_place_impl            -> lower_in_place
//   solve_lower_triangular_transpose_in_place_impl  -> lower_transpose_in_place
//   solve_upper_triangular_in_place_impl            -> upper_in_place
// faer's 2-, 3- and 4-column arms do the same per-column arithmetic on several
// columns at once (no cross-column operation), so solving a k-column right
// hand side one column at a time is bit-identical to them.

//! **Sparse triangular solves**, one right-hand side at a time, ported from
//! `faer` 0.24.4. The factor is CSC with the diagonal stored **first** in each
//! column of a lower factor and **last** in each column of an upper one, as
//! faer's simplicial factors are. A non-unit diagonal is applied as a multiply
//! by its reciprocal (`x * (1 / d)`), which is what faer does — not `x / d`.

use super::{g, m, sl};
use crate::{PetirError, Result};

/// One column of a CSC factor: its row indices and values.
fn column<'a>(
    col_ptr: &[usize],
    row_idx: &'a [usize],
    val: &'a [f64],
    j: usize,
) -> Result<(&'a [usize], &'a [f64])> {
    let range = g(col_ptr, j)?..g(col_ptr, j + 1)?;
    Ok((sl(row_idx, range.clone())?, sl(val, range)?))
}

/// Solve `L x = b` in place (`x` holds `b` on entry). `unit` skips the stored
/// diagonal (treats it as 1).
///
/// # Errors
/// [`PetirError::Invalid`] if the factor's structure does not fit `x`, or a
/// column has no stored diagonal.
pub fn lower_in_place(
    col_ptr: &[usize],
    row_idx: &[usize],
    val: &[f64],
    unit: bool,
    x: &mut [f64],
) -> Result<()> {
    let n = x.len();
    for j in 0..n {
        let (rows, vals) = column(col_ptr, row_idx, val, j)?;
        let (Some((_, rest_rows)), Some((&d, rest_vals))) =
            (rows.split_first(), vals.split_first())
        else {
            return Err(PetirError::Invalid);
        };
        let xj = if unit {
            g(x, j)?
        } else {
            let d = 1.0 / d;
            let xj = g(x, j)? * d;
            *m(x, j)? = xj;
            xj
        };
        for (&i, &lij) in rest_rows.iter().zip(rest_vals) {
            *m(x, i)? -= lij * xj;
        }
    }
    Ok(())
}

/// Solve `Lᵀ x = b` in place, with `L` stored as for [`lower_in_place`].
///
/// # Errors
/// As [`lower_in_place`].
pub fn lower_transpose_in_place(
    col_ptr: &[usize],
    row_idx: &[usize],
    val: &[f64],
    unit: bool,
    x: &mut [f64],
) -> Result<()> {
    let n = x.len();
    for j in (0..n).rev() {
        let (rows, vals) = column(col_ptr, row_idx, val, j)?;
        let (Some((_, rest_rows)), Some((&d, rest_vals))) =
            (rows.split_first(), vals.split_first())
        else {
            return Err(PetirError::Invalid);
        };
        let mut acc = 0.0f64;
        for (&i, &lij) in rest_rows.iter().zip(rest_vals).rev() {
            acc += lij * g(x, i)?;
        }
        let mut xj = g(x, j)? - acc;
        if !unit {
            let d = 1.0 / d;
            xj *= d;
        }
        *m(x, j)? = xj;
    }
    Ok(())
}

/// Solve `U x = b` in place, with the diagonal stored last in each column.
///
/// # Errors
/// As [`lower_in_place`].
pub fn upper_in_place(
    col_ptr: &[usize],
    row_idx: &[usize],
    val: &[f64],
    unit: bool,
    x: &mut [f64],
) -> Result<()> {
    let n = x.len();
    for j in (0..n).rev() {
        let (rows, vals) = column(col_ptr, row_idx, val, j)?;
        let (Some((_, rest_rows)), Some((&d, rest_vals))) = (rows.split_last(), vals.split_last())
        else {
            return Err(PetirError::Invalid);
        };
        let xj = if unit {
            g(x, j)?
        } else {
            let d = 1.0 / d;
            let xj = g(x, j)? * d;
            *m(x, j)? = xj;
            xj
        };
        for (&i, &uij) in rest_rows.iter().zip(rest_vals).rev() {
            *m(x, i)? -= uij * xj;
        }
    }
    Ok(())
}
