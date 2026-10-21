// SPDX-License-Identifier: AGPL-3.0-only
// Copyright (C) 2026 Theodore Ong and the outram-park contributors.
//
// PORTED from faer 0.24.4, src/linalg/triangular_solve.rs (MIT, sarah
// quiñones and the faer contributors), read from the published crates.io
// source on 2026-10-20; the full MIT notice is in `super`'s header
// (`dense/mod.rs`) and applies to this file:
//
//   solve_unit_lower_triangular_in_place_imp      -> unit_lower
//   solve_lower_triangular_in_place_imp           -> lower
//   solve_unit_upper_triangular_in_place_imp      -> unit_upper
//   solve_upper_triangular_in_place_imp           -> upper
//   solve_{unit_,}lower_triangular_in_place_base_case_generic_imp
//                                                 -> base_unit_lower, base_lower
//   block_size, recursion_threshold (= 4)
// Real `f64`, so faer's `conj` is the identity and is dropped. The base
// cases are plain (unfused) scalar arithmetic in faer too, evaluated left to
// right exactly as written there.
// Subscripts are checked accesses (see `super`).

//! Dense triangular solves `T X = B` in place, faer's recursive blocking:
//! split the right-hand side's columns in half while it has more than 64 of
//! them and `T` is at most 128 wide, else split `T` at faer's `block_size`,
//! solve, update the bottom with one [`matmul`](super::matmul::matmul), and
//! solve again; at most 4 rows, the closed-form base case.

use super::matmul::matmul;
use super::{rd, rdo, wr, Accum, View};
use crate::{PetirError, Result};

/// faer's `recursion_threshold()`.
const RECURSION_THRESHOLD: usize = 4;

/// faer's `block_size(n)`.
fn block_size(n: usize) -> usize {
    let base_rem = n / 2;
    n - if n >= 32 {
        (base_rem + 15) / 16 * 16
    } else if n >= 16 {
        (base_rem + 7) / 8 * 8
    } else if n >= 8 {
        (base_rem + 3) / 4 * 4
    } else {
        base_rem
    }
}

fn check(tri: View, rhs: View) -> Result<()> {
    if tri.nrows != tri.ncols || rhs.nrows != tri.nrows {
        return Err(PetirError::Invalid);
    }
    Ok(())
}

/// The base case of the unit-lower solve (`n <= 4`), per column.
fn base_unit_lower(d: &mut [f64], tsrc: Option<&[f64]>, tri: View, rhs: View) -> Result<()> {
    let n = tri.nrows;
    let l = |i: usize, j: usize| -> Result<f64> { Ok(-rdo(d, tsrc, tri.at(i, j))?) };
    match n {
        0 | 1 => Ok(()),
        2 => {
            let nl10 = l(1, 0)?;
            for j in 0..rhs.ncols {
                let x0 = rd(d, rhs.at(0, j))?;
                let x1 = wr(d, rhs.at(1, j))?;
                *x1 = *x1 + nl10 * x0;
            }
            Ok(())
        }
        3 => {
            let (nl10, nl20, nl21) = (l(1, 0)?, l(2, 0)?, l(2, 1)?);
            for j in 0..rhs.ncols {
                let y0 = rd(d, rhs.at(0, j))?;
                let mut y1 = rd(d, rhs.at(1, j))?;
                let mut y2 = rd(d, rhs.at(2, j))?;
                y1 = y1 + nl10 * y0;
                y2 = y2 + nl20 * y0 + nl21 * y1;
                *wr(d, rhs.at(1, j))? = y1;
                *wr(d, rhs.at(2, j))? = y2;
            }
            Ok(())
        }
        4 => {
            let (nl10, nl20, nl21) = (l(1, 0)?, l(2, 0)?, l(2, 1)?);
            let (nl30, nl31, nl32) = (l(3, 0)?, l(3, 1)?, l(3, 2)?);
            for j in 0..rhs.ncols {
                let y0 = rd(d, rhs.at(0, j))?;
                let mut y1 = rd(d, rhs.at(1, j))?;
                let mut y2 = rd(d, rhs.at(2, j))?;
                let mut y3 = rd(d, rhs.at(3, j))?;
                y1 = y1 + nl10 * y0;
                y2 = y2 + nl20 * y0 + nl21 * y1;
                y3 = y3 + nl30 * y0 + nl31 * y1 + nl32 * y2;
                *wr(d, rhs.at(1, j))? = y1;
                *wr(d, rhs.at(2, j))? = y2;
                *wr(d, rhs.at(3, j))? = y3;
            }
            Ok(())
        }
        _ => Err(PetirError::Invalid),
    }
}

/// The base case of the lower solve (`n <= 4`): `l_ii^-1 = 1 / l_ii` and
/// `nl_ij = (-l_ij) * l_ii^-1`, per column.
fn base_lower(d: &mut [f64], tsrc: Option<&[f64]>, tri: View, rhs: View) -> Result<()> {
    let n = tri.nrows;
    let t = |i: usize, j: usize| -> Result<f64> { rdo(d, tsrc, tri.at(i, j)) };
    match n {
        0 => Ok(()),
        1 => {
            let inv = 1.0 / t(0, 0)?;
            for j in 0..rhs.ncols {
                let x0 = wr(d, rhs.at(0, j))?;
                *x0 = *x0 * inv;
            }
            Ok(())
        }
        2 => {
            let l00 = 1.0 / t(0, 0)?;
            let l11 = 1.0 / t(1, 1)?;
            let nl10 = -t(1, 0)? * l11;
            for j in 0..rhs.ncols {
                let x0 = rd(d, rhs.at(0, j))? * l00;
                *wr(d, rhs.at(0, j))? = x0;
                let x1 = wr(d, rhs.at(1, j))?;
                *x1 = *x1 * l11 + nl10 * x0;
            }
            Ok(())
        }
        3 => {
            let l00 = 1.0 / t(0, 0)?;
            let l11 = 1.0 / t(1, 1)?;
            let l22 = 1.0 / t(2, 2)?;
            let nl10 = -t(1, 0)? * l11;
            let nl20 = -t(2, 0)? * l22;
            let nl21 = -t(2, 1)? * l22;
            for j in 0..rhs.ncols {
                let mut y0 = rd(d, rhs.at(0, j))?;
                let mut y1 = rd(d, rhs.at(1, j))?;
                let mut y2 = rd(d, rhs.at(2, j))?;
                y0 = y0 * l00;
                y1 = y1 * l11 + nl10 * y0;
                y2 = y2 * l22 + nl20 * y0 + nl21 * y1;
                *wr(d, rhs.at(0, j))? = y0;
                *wr(d, rhs.at(1, j))? = y1;
                *wr(d, rhs.at(2, j))? = y2;
            }
            Ok(())
        }
        4 => {
            let l00 = 1.0 / t(0, 0)?;
            let l11 = 1.0 / t(1, 1)?;
            let l22 = 1.0 / t(2, 2)?;
            let l33 = 1.0 / t(3, 3)?;
            let nl10 = -t(1, 0)? * l11;
            let nl20 = -t(2, 0)? * l22;
            let nl21 = -t(2, 1)? * l22;
            let nl30 = -t(3, 0)? * l33;
            let nl31 = -t(3, 1)? * l33;
            let nl32 = -t(3, 2)? * l33;
            for j in 0..rhs.ncols {
                let mut y0 = rd(d, rhs.at(0, j))?;
                let mut y1 = rd(d, rhs.at(1, j))?;
                let mut y2 = rd(d, rhs.at(2, j))?;
                let mut y3 = rd(d, rhs.at(3, j))?;
                y0 = y0 * l00;
                y1 = y1 * l11 + nl10 * y0;
                y2 = y2 * l22 + nl20 * y0 + nl21 * y1;
                y3 = y3 * l33 + nl30 * y0 + nl31 * y1 + nl32 * y2;
                *wr(d, rhs.at(0, j))? = y0;
                *wr(d, rhs.at(1, j))? = y1;
                *wr(d, rhs.at(2, j))? = y2;
                *wr(d, rhs.at(3, j))? = y3;
            }
            Ok(())
        }
        _ => Err(PetirError::Invalid),
    }
}

/// Which of the two lower solves.
#[derive(Clone, Copy)]
enum Diag {
    Unit,
    Generic,
}

fn lower_imp(d: &mut [f64], tsrc: Option<&[f64]>, tri: View, rhs: View, diag: Diag) -> Result<()> {
    check(tri, rhs)?;
    let n = tri.nrows;
    let k = rhs.ncols;
    if k > 64 && n <= 128 {
        let mid = k / 2;
        lower_imp(d, tsrc, tri, rhs.cols(0, mid), diag)?;
        return lower_imp(d, tsrc, tri, rhs.cols(mid, k - mid), diag);
    }
    if n <= RECURSION_THRESHOLD {
        return match diag {
            Diag::Unit => base_unit_lower(d, tsrc, tri, rhs),
            Diag::Generic => base_lower(d, tsrc, tri, rhs),
        };
    }
    let bs = block_size(n);
    let top_left = tri.sub(0, 0, bs, bs);
    let bot_left = tri.sub(bs, 0, n - bs, bs);
    let bot_right = tri.sub(bs, bs, n - bs, n - bs);
    let top = rhs.rows(0, bs);
    let bot = rhs.rows(bs, n - bs);
    lower_imp(d, tsrc, top_left, top, diag)?;
    matmul(d, bot, Accum::Add, tsrc, bot_left, None, top, -1.0)?;
    lower_imp(d, tsrc, bot_right, bot, diag)
}

/// `L X = B` with a unit diagonal (faer
/// `solve_unit_lower_triangular_in_place`). `tsrc = None` when `L` lives in
/// the same buffer as `X`.
pub(crate) fn unit_lower(d: &mut [f64], tsrc: Option<&[f64]>, tri: View, rhs: View) -> Result<()> {
    lower_imp(d, tsrc, tri, rhs, Diag::Unit)
}

/// `L X = B` (faer `solve_lower_triangular_in_place`).
pub(crate) fn lower(d: &mut [f64], tsrc: Option<&[f64]>, tri: View, rhs: View) -> Result<()> {
    lower_imp(d, tsrc, tri, rhs, Diag::Generic)
}

/// `U X = B` with a unit diagonal: faer reverses rows and columns of `U` and
/// the rows of `X` and runs the unit-lower solve. No sparse path calls it
/// (faer's supernodal LU and `LLᵀ` solves use the other three); it is kept
/// because `vs_faer` pins it against faer with the other three.
#[allow(dead_code)]
pub(crate) fn unit_upper(d: &mut [f64], tsrc: Option<&[f64]>, tri: View, rhs: View) -> Result<()> {
    lower_imp(d, tsrc, tri.rev_both(), rhs.rev_rows(), Diag::Unit)
}

/// `U X = B`, by the same reversal.
pub(crate) fn upper(d: &mut [f64], tsrc: Option<&[f64]>, tri: View, rhs: View) -> Result<()> {
    lower_imp(d, tsrc, tri.rev_both(), rhs.rev_rows(), Diag::Generic)
}

#[cfg(test)]
mod tests {
    use super::*;
    use alloc::vec::Vec;

    #[test]
    fn solves_lower_and_upper_systems_of_every_blocking() {
        for &n in &[1usize, 3, 4, 5, 9, 17, 40] {
            // T: column-major, well conditioned.
            let mut t = alloc::vec![0.0; n * n];
            for j in 0..n {
                for i in 0..n {
                    t[i + j * n] = if i == j {
                        2.0 + i as f64 * 0.1
                    } else {
                        ((i * 3 + j * 5) % 7) as f64 * 0.05 - 0.15
                    };
                }
            }
            let x: Vec<f64> = (0..n).map(|i| 1.0 + i as f64 * 0.25).collect();
            let tv = View::col_major(0, n, n, n);
            for (which, unit) in [(0, false), (1, true), (2, false), (3, true)] {
                let lowerish = which < 2;
                let mut b = alloc::vec![0.0; n];
                for i in 0..n {
                    for k in 0..n {
                        let inside = if lowerish { k <= i } else { k >= i };
                        if !inside {
                            continue;
                        }
                        let tik = if k == i && unit { 1.0 } else { t[i + k * n] };
                        b[i] += tik * x[k];
                    }
                }
                let bv = View::col_major(0, n, 1, n);
                match which {
                    0 => lower(&mut b, Some(&t), tv, bv).unwrap(),
                    1 => unit_lower(&mut b, Some(&t), tv, bv).unwrap(),
                    2 => upper(&mut b, Some(&t), tv, bv).unwrap(),
                    _ => unit_upper(&mut b, Some(&t), tv, bv).unwrap(),
                }
                for i in 0..n {
                    assert!((b[i] - x[i]).abs() < 1e-12, "n={n} case {which}");
                }
            }
        }
        assert_eq!(
            lower(
                &mut [0.0; 2],
                Some(&[1.0; 4]),
                View::col_major(0, 2, 2, 2),
                View::col_major(0, 3, 1, 3)
            ),
            Err(PetirError::Invalid)
        );
    }
}
