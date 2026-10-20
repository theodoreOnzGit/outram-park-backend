// SPDX-License-Identifier: GPL-3.0-only
// Copyright (C) 2026 Theodore Ong and the outram-park contributors.
//
// PORTED from GNU Octave 11.3.0, scripts/linear-algebra/expm.m
// (installed as /usr/share/octave/11.3.0/m/linear-algebra/expm.m,
// sha256 3f97a52a9c9ef91340d72f80438577fadc53438db7e6b6b914ddc6b65bef7345),
// read 2026-10-20.
// Copyright (C) 2008-2026 The Octave Project Developers.
// GPL-3.0-or-later (verified from the file's own header); used here under
// version 3. See NOTICE.

//! The matrix exponential `exp(A)` — GNU Octave's `expm`.
//!
//! # Algorithm (Octave's, step by step)
//!
//! Ward's diagonal Padé approximation with three-step preconditioning
//! (*SIAM J. Numer. Anal.* 14, 1977):
//!
//! 1. special cases: a `1 x 1` is `exp(a)`; a diagonal matrix is
//!    `diag(exp(diag(A)))`;
//! 2. `-Inf` entries become `-realmax`;
//! 3. trace reduction: if `trace(A)/n > 0`, subtract it from the diagonal;
//! 4. ~~balancing~~ — see the deviation below;
//! 5. scaling: `[~, e] = log2(norm(A, Inf))`, `s = min(max(0, e), 1023)`,
//!    `A *= 2^-s`;
//! 6. the degree-8 diagonal Padé approximant with Octave's eight
//!    coefficients, `r = (x - y) \ (x + y)` with `x` the even and `y` the odd
//!    part, both by Horner in `A^2`;
//! 7. `s` squarings; then `r *= exp(trshift)`.
//!
//! Every matrix operation goes through the Octave vocabulary in
//! [`crate::linalg::octave`], so this reads like `expm.m`.
//!
//! # DEVIATION: no balancing (Octave's step 4)
//!
//! Octave calls `[d, p, aa] = balance(A)`, which is LAPACK `dgebal` with
//! job `'B'` (permute, then scale by powers of two to equalise row and
//! column 2-norms), and undoes it at the end (`r = d * r / d; r(p, p) = r`).
//! LAPACK is not in PETIR, and GSL's `gsl_linalg_balance_matrix` (ported in
//! [`crate::linalg::balance_matrix`]) is a *different* algorithm (1-norms,
//! no permutation), so substituting it would not be Octave's `expm` either.
//! **PETIR's `expm` skips the step** (`d = 1`, `p = 1:n`). In exact
//! arithmetic balancing is a similarity transform and changes nothing; in
//! floating point it changes the scaling exponent `s` and the conditioning
//! of `x - y`.
//!
//! The effect was **measured** against Octave
//! (`tests/octave_expm_code_to_code.rs`): against `expm.m` with only the
//! balance line removed, and against stock `expm.m`. See that test for the
//! numbers. Porting `dgebal` (BSD-licensed) would remove the deviation; it is
//! a non-GSL source, which this crate's `CLAUDE.md` reserves for the
//! maintainer.
//!
//! # Other differences from Octave, not deviations in the algorithm
//!
//! Octave evaluates `\`, `*` and `^` with LAPACK/BLAS; PETIR with its own
//! ports: `\` is the Crout LU with scaled partial pivoting
//! ([`crate::linalg::SquareMatrix`]; Octave's `dgesv` is Doolittle, and for a
//! symmetric `x - y` with a positive diagonal Octave tries Cholesky first),
//! `*` is `cblas_dgemm`'s reference loop (OpenBLAS blocks and may use FMA),
//! and `exp` is the `libm` crate's. These change the last bits only; the
//! test measures them separately from the balancing.
//!
//! # Units
//!
//! Bare dimensionless `f64`; for `exp(A t)` scale `A` by `t` first.

#[allow(unused_imports)]
use crate::real::Real;

use crate::linalg::{Matrix, Norm};
use crate::{PetirError, Result};

/// Octave's Padé coefficients, `expm.m`, verbatim.
const C: [f64; 8] = [
    5.0000000000000000e-1,
    1.1666666666666667e-1,
    1.6666666666666667e-2,
    1.6025641025641026e-3,
    1.0683760683760684e-4,
    4.8562548562548563e-6,
    1.3875013875013875e-7,
    1.9270852604185938e-9,
];

/// The matrix exponential of the square matrix `a`, Octave's `expm`
/// without its balancing step (see the module docs).
///
/// # Errors
///
/// - [`PetirError::NotSquare`] if `a` is not square (Octave: "expm: A must
///   be a square matrix").
/// - [`PetirError::Singular`] if the Padé denominator `x - y` is exactly
///   singular (Octave would warn and return `Inf`/`NaN`).
///
/// # Example
///
/// ```
/// use petir::linalg::{expm, Matrix};
/// // expm.m's own test: expm([1 -1; 0 1]) = [e -e; 0 e]
/// let a = Matrix::from_row_major(2, 2, vec![1.0, -1.0, 0.0, 1.0]).unwrap();
/// let r = expm(&a).unwrap();
/// let e = std::f64::consts::E;
/// assert!((r.get(0, 0) - e).abs() < 1e-14 && (r.get(0, 1) + e).abs() < 1e-14);
/// assert!(r.get(1, 0).abs() < 1e-15 && (r.get(1, 1) - e).abs() < 1e-14);
/// ```
pub fn expm(a: &Matrix) -> Result<Matrix> {
    let n = a.rows();
    if n != a.cols() {
        return Err(PetirError::NotSquare {
            rows: n,
            cols: a.cols(),
        });
    }

    // isscalar (A): r = exp (A)
    if n == 1 {
        return Matrix::from_row_major(1, 1, alloc::vec![Real::exp(a.get(0, 0))]);
    }
    // isdiag (A): r = diag (exp (diag (A)))
    if a.is_diag() {
        let d: alloc::vec::Vec<f64> = a.diag_vec().into_iter().map(Real::exp).collect();
        return Matrix::diag(&d);
    }

    let id = Matrix::eye(n, n)?;
    // A(A == -Inf) = -realmax ();
    let mut a = a.clone();
    for i in 0..n {
        for j in 0..n {
            if a.get(i, j) == f64::NEG_INFINITY {
                a.set(i, j, -f64::MAX);
            }
        }
    }
    // Trace reduction.
    let trshift = a.trace()? / n as f64;
    if trshift > 0.0 {
        a = a.sub(&id.scale(trshift))?;
    }
    // Balancing: DEVIATION, skipped (d = 1, p = 1:n). See the module docs.
    let aa = a;
    // [~, e] = log2 (norm (aa, "inf")); s = min (max (0, e), 1023);
    let (_, e) = libm::frexp(aa.norm(Norm::Inf)?);
    let s = e.clamp(0, 1023);
    let aa = aa.scale(libm::ldexp(1.0, -s));

    // Pade approximation for exp(A).
    let [c1, c2, c3, c4, c5, c6, c7, c8] = C;
    let a2 = aa.mul(&aa)?;
    // x = (((c(8) * a2 + c(6) * id) * a2 + c(4) * id) * a2 + c(2) * id) * a2 + id;
    let x = a2
        .scale(c8)
        .add(&id.scale(c6))?
        .mul(&a2)?
        .add(&id.scale(c4))?
        .mul(&a2)?
        .add(&id.scale(c2))?
        .mul(&a2)?
        .add(&id)?;
    // y = (((c(7) * a2 + c(5) * id) * a2 + c(3) * id) * a2 + c(1) * id) * aa;
    let y = a2
        .scale(c7)
        .add(&id.scale(c5))?
        .mul(&a2)?
        .add(&id.scale(c3))?
        .mul(&a2)?
        .add(&id.scale(c1))?
        .mul(&aa)?;

    // r = (x - y) \ (x + y);
    let mut r = x.sub(&y)?.solve_mat(&x.add(&y)?)?;

    // Undo scaling by repeated squaring: for k = 1:s, r ^= 2;
    for _ in 0..s {
        r = r.mul(&r)?;
    }

    // Inverse balancing: skipped with the balancing.
    // Inverse trace reduction.
    if trshift > 0.0 {
        r = r.scale(Real::exp(trshift));
    }
    Ok(r)
}

#[cfg(test)]
mod tests {
    use super::*;
    use alloc::vec;

    /// Octave's own `%!assert` tests from `expm.m`.
    #[test]
    fn octaves_own_assertions() {
        let e = core::f64::consts::E;
        let r = expm(&Matrix::from_row_major(2, 2, vec![1.0, -1.0, 0.0, 1.0]).unwrap()).unwrap();
        let want = [e, -e, 0.0, e];
        let err = r
            .sub(&Matrix::from_row_major(2, 2, want.to_vec()).unwrap())
            .unwrap();
        assert!(err.norm(Norm::Two).unwrap() < 1e-5);

        let a3 = Matrix::from_row_major(3, 3, vec![1.0, -1.0, -1.0, 0.0, 1.0, -1.0, 0.0, 0.0, 1.0])
            .unwrap();
        let r3 = expm(&a3).unwrap();
        let w3 = [e, -e, -e / 2.0, 0.0, e, -e, 0.0, 0.0, e];
        for (p, q) in r3.as_slice().iter().zip(w3) {
            assert!((p - q).abs() < 1e-5);
        }
        let ten = Matrix::from_row_major(1, 1, vec![10.0]).unwrap();
        assert_eq!(expm(&ten).unwrap().get(0, 0), Real::exp(10.0));
        let z = Matrix::zeros(3, 3).unwrap();
        assert_eq!(expm(&z).unwrap(), Matrix::eye(3, 3).unwrap());
        assert!(expm(&Matrix::zeros(2, 3).unwrap()).is_err());
    }

    /// The trace-shift branch, the `-Inf` replacement, and squaring.
    #[test]
    fn trace_shift_and_neg_inf_paths() {
        // positive trace: exp([[2,1],[0,2]]) = e^2 [[1,1],[0,1]]
        let a = Matrix::from_row_major(2, 2, vec![2.0, 1.0, 0.0, 2.0]).unwrap();
        let r = expm(&a).unwrap();
        let e2 = Real::exp(2.0);
        assert!((r.get(0, 0) - e2).abs() < 1e-13 && (r.get(0, 1) - e2).abs() < 1e-13);
        // large norm: many squarings, exp(t [[0,1],[-1,0]]) is a rotation
        let t = 40.0;
        let rot = Matrix::from_row_major(2, 2, vec![0.0, t, -t, 0.0]).unwrap();
        let r = expm(&rot).unwrap();
        assert!((r.get(0, 0) - Real::cos(t)).abs() < 1e-10);
        assert!((r.get(0, 1) - Real::sin(t)).abs() < 1e-10);
        // -Inf becomes -realmax rather than poisoning the arithmetic with NaN
        let m = Matrix::from_row_major(2, 2, vec![0.0, f64::NEG_INFINITY, 1.0, 0.0]).unwrap();
        assert!(expm(&m).is_ok());
    }
}
