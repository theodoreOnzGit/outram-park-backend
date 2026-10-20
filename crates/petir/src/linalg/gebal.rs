// PORTED from Reference-LAPACK 3.12.1,
// https://github.com/Reference-LAPACK/lapack, tag v3.12.1, commit
// 6ec7f2bc4ecf4c4a93496aa2fa519575bc0e39ca (2024-12-03), read 2026-10-20:
//   SRC/dgebal.f       -> `dgebal`
//   SRC/dgebak.f       -> `dgebak`
//   BLAS/SRC/dnrm2.f90 -> `dnrm2`  (reference BLAS, called by DGEBAL)
//   BLAS/SRC/idamax.f  -> `idamax` (reference BLAS, called by DGEBAL)
// and from GNU Octave 11.3.0, liboctave/numeric/aepbalance.h
// (`permuting_vector`, `scaling_vector`) and aepbalance.cc (the job letter),
// Copyright (C) 1994-2026 The Octave Project Developers, GPL-3.0-or-later,
// used here under version 3 -> `octave_balance`.
//
// Upstream notice for the LAPACK/BLAS parts, reproduced as the licence
// requires (modified BSD, LAPACK's LICENSE file):
//
//   Copyright (c) 1992-2023 The University of Tennessee and The University
//                           of Tennessee Research Foundation.  All rights
//                           reserved.
//   Copyright (c) 2000-2023 The University of California Berkeley. All
//                           rights reserved.
//   Copyright (c) 2006-2023 The University of Colorado Denver.  All rights
//                           reserved.
//
//   Redistribution and use in source and binary forms, with or without
//   modification, are permitted provided that the following conditions are
//   met:
//
//   - Redistributions of source code must retain the above copyright
//     notice, this list of conditions and the following disclaimer.
//
//   - Redistributions in binary form must reproduce the above copyright
//     notice, this list of conditions and the following disclaimer listed
//     in this license in the documentation and/or other materials
//     provided with the distribution.
//
//   - Neither the name of the copyright holders nor the names of its
//     contributors may be used to endorse or promote products derived from
//     this software without specific prior written permission.
//
//   The copyright holders provide no reassurances that the source code
//   provided does not infringe any patent, copyright, or any other
//   intellectual property rights of third parties.  The copyright holders
//   disclaim any liability to any recipient for claims brought against
//   recipient by any third party for infringement of that parties
//   intellectual property rights.
//
//   THIS SOFTWARE IS PROVIDED BY THE COPYRIGHT HOLDERS AND CONTRIBUTORS
//   "AS IS" AND ANY EXPRESS OR IMPLIED WARRANTIES, INCLUDING, BUT NOT
//   LIMITED TO, THE IMPLIED WARRANTIES OF MERCHANTABILITY AND FITNESS FOR
//   A PARTICULAR PURPOSE ARE DISCLAIMED. IN NO EVENT SHALL THE COPYRIGHT
//   OWNER OR CONTRIBUTORS BE LIABLE FOR ANY DIRECT, INDIRECT, INCIDENTAL,
//   SPECIAL, EXEMPLARY, OR CONSEQUENTIAL DAMAGES (INCLUDING, BUT NOT
//   LIMITED TO, PROCUREMENT OF SUBSTITUTE GOODS OR SERVICES; LOSS OF USE,
//   DATA, OR PROFITS; OR BUSINESS INTERRUPTION) HOWEVER CAUSED AND ON ANY
//   THEORY OF LIABILITY, WHETHER IN CONTRACT, STRICT LIABILITY, OR TORT
//   (INCLUDING NEGLIGENCE OR OTHERWISE) ARISING IN ANY WAY OUT OF THE USE
//   OF THIS SOFTWARE, EVEN IF ADVISED OF THE POSSIBILITY OF SUCH DAMAGE.
//
// Copyright (C) 2026 Theodore Ong and the outram-park contributors (this
// port). Modified BSD into AGPL-3.0-only is one-way; see NOTICE.

//! Balancing of a general real matrix — LAPACK `DGEBAL` and its
//! back-transformation `DGEBAK` (Reference-LAPACK 3.12.1), and Octave's
//! `[d, p, aa] = balance (A)` built on them.
//!
//! Maintainer direction, gh:#998 (2026-10-20): "Dgebal also port", so that
//! [`crate::linalg::expm`] carries out GNU Octave's balancing step exactly.
//!
//! # What `DGEBAL` does
//!
//! 1. **Permute** (`job` `P` or `B`): repeatedly move a row whose
//!    off-diagonal part (within the active block) is zero to the bottom, and
//!    a column whose off-diagonal part is zero to the left, isolating
//!    eigenvalues. The active block is rows/columns `ilo..=ihi` (1-based).
//! 2. **Scale** (`job` `S` or `B`): for each `i` in the active block, find
//!    a power of two `f` that brings the 2-norms of column `i` and row `i`
//!    of the block closer, guarded against overflow and underflow, and
//!    apply `D^-1 A D`; repeat until no factor changes.
//!
//! `scale` returns, LAPACK's way, the permutation indices (as `f64`,
//! 1-based) outside `ilo..=ihi` and the scaling factors inside it.
//!
//! # How this relates to [`crate::linalg::balance_matrix`] (GSL)
//!
//! GSL's `gsl_linalg_balance_matrix` (`balancemat.c`) is the EISPACK
//! `BALANC` scaling loop: OFF-diagonal 1-norms, no permutation, no overflow
//! guards, and the column norm scaled by `radix^2` per step. LAPACK 3.12's
//! `DGEBAL` uses 2-norms through `DNRM2` (including the diagonal), the
//! largest entries through `IDAMAX`, the safe-range guards, and permutes
//! first. **The arithmetic is different, so the two are NOT merged**: GSL's
//! eigensolver stays bit-identical to GSL with GSL's balancing, and `expm`
//! becomes Octave's with LAPACK's.
//!
//! # Verification
//!
//! `tests/lapack_gebal_code_to_code.rs` against the system Reference-LAPACK
//! 3.12.1 (`reference-data/lapack/`); see its doc comment.

use alloc::vec::Vec;

#[allow(unused_imports)]
use crate::real::Real;

use crate::linalg::elem::{rd, wr};
use crate::linalg::Matrix;
use crate::{PetirError, Result};

/// `JOB` of `DGEBAL` / `DGEBAK`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BalanceJob {
    /// `'N'`: neither permute nor scale.
    None,
    /// `'P'`: permute only.
    Permute,
    /// `'S'`: scale only.
    Scale,
    /// `'B'`: both. Octave's `balance (A)` default, and `expm.m`'s.
    Both,
}

/// `SIDE` of `DGEBAK`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BackSide {
    /// `'R'`: right eigenvectors (`V := D V`, then permute rows).
    Right,
    /// `'L'`: left eigenvectors (`V := D^-1 V`, then permute rows).
    Left,
}

/// The output of [`dgebal`] besides the balanced matrix, in LAPACK's
/// conventions (1-based `ilo`, `ihi`; `scale` holds 1-based permutation
/// indices outside `ilo..=ihi` and scaling factors inside it).
#[derive(Debug, Clone, PartialEq)]
pub struct Balancing {
    /// The job that produced it.
    pub job: BalanceJob,
    /// `ILO`, 1-based.
    pub ilo: usize,
    /// `IHI`, 1-based (`0` for `n = 0`).
    pub ihi: usize,
    /// `SCALE(1..N)`.
    pub scale: Vec<f64>,
}

/// Reference BLAS `DNRM2` (`BLAS/SRC/dnrm2.f90`, 3.12.1): the Euclidean norm
/// by Blue's algorithm with three accumulators (Anderson, ACM TOMS 2017).
/// The constants are the `real64` instances of upstream's expressions:
/// `tsml = 2^-511`, `tbig = 2^486`, `ssml = 2^537`, `sbig = 2^-538`.
fn dnrm2<I: Iterator<Item = f64>>(x: I) -> f64 {
    let tsml = libm::ldexp(1.0, -511);
    let tbig = libm::ldexp(1.0, 486);
    let ssml = libm::ldexp(1.0, 537);
    let sbig = libm::ldexp(1.0, -538);
    let max_n = f64::MAX;
    let mut n = 0usize;
    let mut notbig = true;
    let mut asml = 0.0_f64;
    let mut amed = 0.0_f64;
    let mut abig = 0.0_f64;
    for xi in x {
        n += 1;
        let ax = xi.abs();
        if ax > tbig {
            let t = ax * sbig;
            abig += t * t;
            notbig = false;
        } else if ax < tsml {
            if notbig {
                let t = ax * ssml;
                asml += t * t;
            }
        } else {
            amed += ax * ax;
        }
    }
    if n == 0 {
        return 0.0;
    }
    let scl;
    let sumsq;
    #[allow(clippy::eq_op)]
    let amed_nan = amed != amed;
    if abig > 0.0 {
        if amed > 0.0 || amed > max_n || amed_nan {
            abig += (amed * sbig) * sbig;
        }
        scl = 1.0 / sbig;
        sumsq = abig;
    } else if asml > 0.0 {
        if amed > 0.0 || amed > max_n || amed_nan {
            let amed_r = Real::sqrt(amed);
            let asml_r = Real::sqrt(asml) / ssml;
            let (ymin, ymax) = if asml_r > amed_r {
                (amed_r, asml_r)
            } else {
                (asml_r, amed_r)
            };
            scl = 1.0;
            let q = ymin / ymax;
            sumsq = ymax * ymax * (1.0 + q * q);
        } else {
            scl = 1.0 / ssml;
            sumsq = asml;
        }
    } else {
        scl = 1.0;
        sumsq = amed;
    }
    scl * Real::sqrt(sumsq)
}

/// Reference BLAS `IDAMAX` (`BLAS/SRC/idamax.f`): the 1-based index of the
/// first entry of largest absolute value; `0` for an empty vector.
fn idamax<I: Iterator<Item = f64>>(x: I) -> usize {
    let mut idx = 0usize;
    let mut dmax = 0.0_f64;
    for (k, v) in x.enumerate() {
        let a = v.abs();
        if k == 0 {
            idx = 1;
            dmax = a;
        } else if a > dmax {
            idx = k + 1;
            dmax = a;
        }
    }
    idx
}

// 1-based accessors, so the loops below read like the Fortran.
#[inline]
fn g(a: &Matrix, i: usize, j: usize) -> f64 {
    a.get(i - 1, j - 1)
}
#[inline]
fn s(a: &mut Matrix, i: usize, j: usize, v: f64) {
    a.set(i - 1, j - 1, v);
}

/// `DSWAP( L, A(1,I), 1, A(1,L), 1 )`: rows `1..=len` of columns `c1`, `c2`.
fn swap_cols(a: &mut Matrix, len: usize, c1: usize, c2: usize) {
    for r in 1..=len {
        let t = g(a, r, c1);
        s(a, r, c1, g(a, r, c2));
        s(a, r, c2, t);
    }
}

/// `DSWAP( N-K+1, A(I,K), LDA, A(L,K), LDA )`: columns `k..=n` of rows
/// `r1`, `r2`.
fn swap_rows(a: &mut Matrix, k: usize, n: usize, r1: usize, r2: usize) {
    for c in k..=n {
        let t = g(a, r1, c);
        s(a, r1, c, g(a, r2, c));
        s(a, r2, c, t);
    }
}

/// LAPACK `DGEBAL` (`SRC/dgebal.f`, 3.12.1): balance the square matrix `a`
/// in place (permute and/or scale, per `job`).
///
/// # Errors
///
/// - [`PetirError::NotSquare`] if `a` is not square.
/// - [`PetirError::Invalid`] where `DGEBAL` returns `INFO = -3`: a NaN
///   appears in a norm during scaling. `a` is then partly balanced, as in
///   LAPACK.
pub fn dgebal(job: BalanceJob, a: &mut Matrix) -> Result<Balancing> {
    let n = a.rows();
    if n != a.cols() {
        return Err(PetirError::NotSquare {
            rows: n,
            cols: a.cols(),
        });
    }
    let mut scale = alloc::vec![0.0_f64; n];
    // Quick returns.
    if n == 0 {
        return Ok(Balancing {
            job,
            ilo: 1,
            ihi: 0,
            scale,
        });
    }
    if job == BalanceJob::None {
        for v in scale.iter_mut() {
            *v = 1.0;
        }
        return Ok(Balancing {
            job,
            ilo: 1,
            ihi: n,
            scale,
        });
    }
    // Permutation to isolate eigenvalues if possible.
    let mut k = 1usize;
    let mut l = n;
    if job != BalanceJob::Scale {
        // Row and column exchange.
        let mut noconv = true;
        while noconv {
            // Search for rows isolating an eigenvalue and push them down.
            noconv = false;
            let mut i = l;
            while i >= 1 {
                let mut canswap = true;
                for j in 1..=l {
                    if i != j && g(a, i, j) != 0.0 {
                        canswap = false;
                        break;
                    }
                }
                if canswap {
                    wr(&mut scale, l - 1, i as f64);
                    if i != l {
                        swap_cols(a, l, i, l);
                        swap_rows(a, k, n, i, l);
                    }
                    noconv = true;
                    if l == 1 {
                        return Ok(Balancing {
                            job,
                            ilo: 1,
                            ihi: 1,
                            scale,
                        });
                    }
                    l -= 1;
                }
                i -= 1;
            }
        }
        noconv = true;
        while noconv {
            // Search for columns isolating an eigenvalue and push them left.
            noconv = false;
            // Fortran evaluates DO bounds once, at loop entry: the outer
            // `DO J = K, L` keeps the K of entry while the inner
            // `DO I = K, L` sees the current K.
            let (k0, l0) = (k, l);
            for j in k0..=l0 {
                let mut canswap = true;
                for i in k..=l {
                    if i != j && g(a, i, j) != 0.0 {
                        canswap = false;
                        break;
                    }
                }
                if canswap {
                    wr(&mut scale, k - 1, j as f64);
                    if j != k {
                        swap_cols(a, l, j, k);
                        swap_rows(a, k, n, j, k);
                    }
                    noconv = true;
                    k += 1;
                }
            }
        }
    }
    // Initialize SCALE for non-permuted submatrix.
    for v in scale.iter_mut().take(l).skip(k - 1) {
        *v = 1.0;
    }
    // If we only had to permute, we are done.
    if job == BalanceJob::Permute {
        return Ok(Balancing {
            job,
            ilo: k,
            ihi: l,
            scale,
        });
    }
    // Balance the submatrix in rows K to L. Iterative loop for norm
    // reduction. SFMIN1 = DLAMCH('S') / DLAMCH('P') = 2^-1022 / 2^-52.
    const SCLFAC: f64 = 2.0;
    const FACTOR: f64 = 0.95;
    let sfmin1 = f64::MIN_POSITIVE / f64::EPSILON;
    let sfmax1 = 1.0 / sfmin1;
    let sfmin2 = sfmin1 * SCLFAC;
    let sfmax2 = 1.0 / sfmin2;
    let mut noconv = true;
    while noconv {
        noconv = false;
        for i in k..=l {
            let mut c = dnrm2((k..=l).map(|r| g(a, r, i)));
            let mut r = dnrm2((k..=l).map(|cc| g(a, i, cc)));
            let ica = idamax((1..=l).map(|rr| g(a, rr, i)));
            let mut ca = g(a, ica, i).abs();
            let ira = idamax((k..=n).map(|cc| g(a, i, cc)));
            let mut ra = g(a, i, ira + k - 1).abs();
            // Guard against zero C or R due to underflow.
            if c == 0.0 || r == 0.0 {
                continue;
            }
            // Exit if NaN to avoid infinite loop.
            if (c + ca + r + ra).is_nan() {
                return Err(PetirError::Invalid);
            }
            let mut gg = r / SCLFAC;
            let mut f = 1.0_f64;
            let s0 = c + r;
            while c < gg && f.max(c).max(ca) < sfmax2 && r.min(gg).min(ra) > sfmin2 {
                f *= SCLFAC;
                c *= SCLFAC;
                ca *= SCLFAC;
                r /= SCLFAC;
                gg /= SCLFAC;
                ra /= SCLFAC;
            }
            gg = c / SCLFAC;
            while gg >= r && r.max(ra) < sfmax2 && f.min(c).min(gg).min(ca) > sfmin2 {
                f /= SCLFAC;
                c /= SCLFAC;
                gg /= SCLFAC;
                ca /= SCLFAC;
                r *= SCLFAC;
                ra *= SCLFAC;
            }
            // Now balance.
            if (c + r) >= FACTOR * s0 {
                continue;
            }
            let si = rd(&scale, i - 1);
            if f < 1.0 && si < 1.0 && f * si <= sfmin1 {
                continue;
            }
            if f > 1.0 && si > 1.0 && si >= sfmax1 / f {
                continue;
            }
            gg = 1.0 / f;
            wr(&mut scale, i - 1, si * f);
            noconv = true;
            // DSCAL( N-K+1, G, A( I, K ), LDA ); DSCAL( L, F, A( 1, I ), 1 )
            for cc in k..=n {
                let v = g(a, i, cc);
                s(a, i, cc, gg * v);
            }
            for rr in 1..=l {
                let v = g(a, rr, i);
                s(a, rr, i, f * v);
            }
        }
    }
    Ok(Balancing {
        job,
        ilo: k,
        ihi: l,
        scale,
    })
}

/// LAPACK `DGEBAK` (`SRC/dgebak.f`, 3.12.1): back-transform the eigenvectors
/// `v` (`n x m`) of a matrix balanced by [`dgebal`] into those of the
/// original matrix, in place. `bal.job` is the job `DGEBAL` ran with.
///
/// # Errors
///
/// [`PetirError::LengthMismatch`] if `v` does not have `bal.scale.len()`
/// rows, and [`PetirError::Invalid`] where LAPACK's argument checks fail
/// (`ilo`/`ihi` out of range).
pub fn dgebak(bal: &Balancing, side: BackSide, v: &mut Matrix) -> Result<()> {
    let n = bal.scale.len();
    let m = v.cols();
    if v.rows() != n {
        return Err(PetirError::LengthMismatch {
            expected: n,
            found: v.rows(),
        });
    }
    let (ilo, ihi) = (bal.ilo, bal.ihi);
    if ilo < 1 || ilo > n.max(1) || ihi < ilo.min(n) || ihi > n {
        return Err(PetirError::Invalid);
    }
    // Quick returns.
    if n == 0 || m == 0 || bal.job == BalanceJob::None {
        return Ok(());
    }
    let job = bal.job;
    // Backward balance.
    if ilo != ihi && (job == BalanceJob::Scale || job == BalanceJob::Both) {
        for i in ilo..=ihi {
            let sc = match side {
                BackSide::Right => rd(&bal.scale, i - 1),
                BackSide::Left => 1.0 / rd(&bal.scale, i - 1),
            };
            // DSCAL( M, S, V( I, 1 ), LDV )
            for j in 1..=m {
                let x = g(v, i, j);
                s(v, i, j, sc * x);
            }
        }
    }
    // Backward permutation: for I = ILO-1 step -1 until 1, IHI+1 step 1
    // until N. (The loop is the same for both sides.)
    if job == BalanceJob::Permute || job == BalanceJob::Both {
        for ii in 1..=n {
            let mut i = ii;
            if i >= ilo && i <= ihi {
                continue;
            }
            if i < ilo {
                i = ilo - ii;
            }
            // K = INT( SCALE( I ) )
            let k = rd(&bal.scale, i - 1) as usize;
            if k == i {
                continue;
            }
            // DSWAP( M, V( I, 1 ), LDV, V( K, 1 ), LDV )
            for j in 1..=m {
                let t = g(v, i, j);
                s(v, i, j, g(v, k, j));
                s(v, k, j, t);
            }
        }
    }
    Ok(())
}

/// GNU Octave's `[d, p, aa] = balance (A)` for a real matrix: `DGEBAL` with
/// job `'B'` (`aepbalance.cc`, `get_job (false, false)`), then
/// `scaling_vector` and `permuting_vector` (`aepbalance.h`). Returns `(d, p,
/// aa)` with `p` 1-based, as Octave does, so that `aa = (D \ A(p, p)) * D`
/// with `D = diag (d)`.
///
/// # Errors
///
/// As [`dgebal`].
pub fn octave_balance(a: &Matrix) -> Result<(Vec<f64>, Vec<usize>, Matrix)> {
    let mut aa = a.clone();
    let bal = dgebal(BalanceJob::Both, &mut aa)?;
    let n = aa.rows();
    let (ilo, ihi) = (bal.ilo, bal.ihi);
    // permuting_vector ()
    let mut pv: Vec<usize> = (1..=n).collect();
    let mut i = n;
    while i > ihi {
        // for (i = n-1; i >= m_ihi; i--) in 0-based C++
        let j = (rd(&bal.scale, i - 1) as usize).saturating_sub(1);
        if j < n {
            pv.swap(i - 1, j);
        }
        i -= 1;
    }
    for i0 in 0..ilo.saturating_sub(1) {
        let j = (rd(&bal.scale, i0) as usize).saturating_sub(1);
        if j < n {
            pv.swap(i0, j);
        }
    }
    // scaling_vector ()
    let mut scv = alloc::vec![1.0_f64; n];
    for i0 in ilo.saturating_sub(1)..ihi {
        wr(&mut scv, i0, rd(&bal.scale, i0));
    }
    Ok((scv, pv, aa))
}

#[cfg(test)]
mod tests {
    use super::*;
    use alloc::vec;

    #[test]
    fn dnrm2_matches_the_plain_norm_and_survives_extremes() {
        assert_eq!(dnrm2([3.0, 4.0].into_iter()), 5.0);
        assert_eq!(dnrm2(core::iter::empty()), 0.0);
        let big = libm::ldexp(1.0, 600);
        assert_eq!(dnrm2([3.0 * big, 4.0 * big].into_iter()), 5.0 * big);
        let tiny = libm::ldexp(1.0, -600);
        assert_eq!(dnrm2([3.0 * tiny, 4.0 * tiny].into_iter()), 5.0 * tiny);
        // mixed small and medium
        let v = dnrm2([1.0, tiny].into_iter());
        assert_eq!(v, 1.0);
    }

    #[test]
    fn idamax_takes_the_first_largest() {
        assert_eq!(idamax([1.0, -3.0, 3.0].into_iter()), 2);
        assert_eq!(idamax(core::iter::empty()), 0);
    }

    #[test]
    fn upper_triangular_is_fully_permuted_and_scale_is_noop_on_symmetric() {
        let mut a = Matrix::from_row_major(3, 3, vec![1.0, 2.0, 3.0, 0.0, 4.0, 5.0, 0.0, 0.0, 6.0])
            .unwrap();
        let b = dgebal(BalanceJob::Both, &mut a).unwrap();
        assert_eq!((b.ilo, b.ihi), (1, 1));
        let mut s = Matrix::from_row_major(2, 2, vec![2.0, 1.0, 1.0, 2.0]).unwrap();
        let b = dgebal(BalanceJob::Both, &mut s).unwrap();
        assert_eq!(b.scale, vec![1.0, 1.0]);
        assert!(dgebal(BalanceJob::Both, &mut Matrix::zeros(2, 3).unwrap()).is_err());
    }
}
