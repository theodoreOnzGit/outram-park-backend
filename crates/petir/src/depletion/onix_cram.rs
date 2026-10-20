// SPDX-License-Identifier: AGPL-3.0-only
// Copyright (C) 2026 Theodore Ong and the outram-park contributors.
//
// ---------------------------------------------------------------------------
// PORTED from ONIX (NOT from GSL, which has no CRAM; NOT from OpenMC, whose
// CRAM16 is the different IPF form in `cram.rs`).
//
//   Project : ONIX, https://github.com/jlanversin/ONIX
//   File    : onix/salameche/cram.py (`CRAM16`, lines 6-59: theta 22-30,
//             alpha_0 32, alpha 34-42, solve loop 44-59)
//   Commit  : 7328dc6acf705827470819a4367ef2a98c642b74
//   Licence : MIT, reproduced in full below and in NOTICE (section
//             "DERIVED WORK -- ONIX"). Thanks to ONIX's author.
//
//   MIT License
//
//   Copyright (c) 2019-2020 Julien de Troullioud de Lanversin
//
//   Permission is hereby granted, free of charge, to any person obtaining a
//   copy of this software and associated documentation files (the
//   "Software"), to deal in the Software without restriction, including
//   without limitation the rights to use, copy, modify, merge, publish,
//   distribute, sublicense, and/or sell copies of the Software, and to
//   permit persons to whom the Software is furnished to do so, subject to
//   the following conditions:
//
//   The above copyright notice and this permission notice shall be included
//   in all copies or substantial portions of the Software.
//
//   THE SOFTWARE IS PROVIDED "AS IS", WITHOUT WARRANTY OF ANY KIND, EXPRESS
//   OR IMPLIED, INCLUDING BUT NOT LIMITED TO THE WARRANTIES OF
//   MERCHANTABILITY, FITNESS FOR A PARTICULAR PURPOSE AND NONINFRINGEMENT.
//   IN NO EVENT SHALL THE AUTHORS OR COPYRIGHT HOLDERS BE LIABLE FOR ANY
//   CLAIM, DAMAGES OR OTHER LIABILITY, WHETHER IN AN ACTION OF CONTRACT,
//   TORT OR OTHERWISE, ARISING FROM, OUT OF OR IN CONNECTION WITH THE
//   SOFTWARE OR THE USE OR OTHER DEALINGS IN THE SOFTWARE.
//
// First ported in `outram-park-fork-onix/src/cram.rs` (2026-08-04) and MOVED
// here 2026-10-20 (gh:#999, maintainer direction "The cram from Onix can go
// to petir and attribute onix"). The arithmetic is unchanged, operation for
// operation: `outram-park-fork-onix/tests/cram16_move_fingerprints.rs` pins
// it against fingerprints printed on the pre-move code. What changed:
// `num_complex::Complex64` became PETIR's `depletion::Complex64` (the same
// formulas for `*`, `/`, `-` and `|z|^2`, checked against num-complex 0.4.6's
// source), `Complex64::norm` became `crate::real::hypot`, subscripts became
// `get` access (PETIR's no-panic gate) and errors became `PetirError`s.
// ---------------------------------------------------------------------------

//! ONIX's order-16 CRAM in **partial-fraction** form.
//!
//! Computes `n(dt) = exp(A dt) n0` for a burnup matrix `A` (`1/s`) as ONIX's
//! `CRAM16` does (`onix/salameche/cram.py:6-59`):
//!
//! ```text
//!     N = 2 Re( sum_{k=1..8} solve(A dt - theta_k I, alpha_k n0) ) + alpha_0 n0
//! ```
//!
//! Every pole's solve sees the ORIGINAL `n0` and the eight results are
//! summed. That is the parallel partial-fraction sum, and it is a different
//! algorithm from [`super::cram::cram16`], OpenMC's incomplete partial
//! factorisation (IPF), in which each pole acts on the vector the previous
//! pole produced.
//!
//! # Why this is a second routine and not [`super::cram::cram16`]
//!
//! Compared 2026-10-20 (gh:#999):
//!
//! * **Poles:** the same eight Pusa poles, in a different order. ONIX holds
//!   20 significant digits, OpenMC 16; as `f64` they round identically in 12
//!   of 16 components and differ by 1 to 3 ulps in the other four (pole 1
//!   `re` 1 ulp; pole 5 `im` 2; pole 6 `im` 2; pole 8 `re` 1 and `im` 3, in
//!   ONIX's order).
//! * **`alpha_0`:** 1 ulp apart (`2.1248537104952237488e-16` against
//!   `2.124853710495224e-16`).
//! * **Residues:** unrelated. ONIX's `alpha_k` are partial-fraction residues
//!   (`|alpha_k|` from 2.4e-5 to 2.3e2); OpenMC's are IPF coefficients
//!   (up to 1.2e5). Each set is only correct for its own algorithm.
//! * **Pivoting:** ONIX's port picks the pivot by `|z|` (`hypot`), PETIR's
//!   IPF by `|z|^2`. Neither is upstream's: NumPy's `solve` calls LAPACK
//!   `zgesv`, whose `izamax` compares `|re| + |im|`.
//! * **Result:** agree to round-off, not to the bit: worst difference
//!   6.87e-15 on unit-scale inventories, 0 of 9 entries bit-equal
//!   (`tests::agrees_with_the_ipf_form_to_round_off`).
//!
//! So both stay, each bit-faithful to its own upstream. The OpenMC IPF form
//! is the one PETIR's integrators call; this one serves
//! `outram-park-fork-onix`.
//!
//! # Deviations from upstream, all flagged
//!
//! * **DEVIATION (precision):** ONIX stores the coefficients as
//!   `np.complex256` and multiplies `alpha_k * N_0` in extended precision,
//!   then solves in `complex128`. This holds `f64` throughout. The literals
//!   are kept to ONIX's 20 digits; `f64` rounds them.
//! * **DEVIATION (linear solver):** `np.linalg.solve` (LAPACK `zgesv`) is
//!   replaced by dense complex Gaussian elimination with partial pivoting on
//!   `hypot(re, im)`; see the pivoting note above.
//! * **DEVIATION (singular pivot):** a column with no nonzero entry returns
//!   [`PetirError::Singular`]; `zgesv` would report `info > 0` and NumPy
//!   raises `LinAlgError`.
//! * **Not ported:** ONIX's `print` and timing calls (`cram.py:17-18, 57`),
//!   and `CRAM_density_check` (`cram.py:127-163`), which is a physicality
//!   filter on the result, not part of the approximation; it stays in
//!   `outram-park-fork-onix` as `clamp_nonnegative`.
//! * **No degenerate early return:** unlike [`super::cram::cram16`],
//!   `dt == 0` or an all-zero matrix is evaluated like any other input, as
//!   ONIX does, and returns `n0` only to `alpha_0`-scale round-off.
//!
//! # Units
//!
//! * `a` — burnup matrix, every coefficient `1/s`.
//! * `dt` — step length, seconds.
//! * `n0` / return value — number densities in any consistent unit.

use alloc::vec;
use alloc::vec::Vec;

use super::cram::Complex64;
use super::matrix::DepletionMatrix;
use crate::{PetirError, Result};

/// The 8 upper-half-plane poles `theta_k`, `cram.py:22-30`, digit for digit.
///
/// `excessive_precision` is allowed on purpose: these are ONIX's
/// `complex256` literals reproduced verbatim; `f64` rounds them to the same
/// values truncated literals would give.
#[allow(clippy::excessive_precision)]
const CRAM16_PFD_THETA: [Complex64; 8] = [
    Complex64::new(-1.0843917078696988026e1, 1.9277446167181652284e1),
    Complex64::new(-5.2649713434426468895, 1.6220221473167927305e1),
    Complex64::new(5.9481522689511774808, 3.5874573620183222829),
    Complex64::new(3.5091036084149180974, 8.4361989858843750826),
    Complex64::new(6.4161776990994341923, 1.1941223933701386874),
    Complex64::new(1.4193758971856659786, 1.0925363484496722585e1),
    Complex64::new(4.9931747377179963991, 5.9968817136039422260),
    Complex64::new(-1.4139284624888862114, 1.3497725698892745389e1),
];

/// The 8 partial-fraction residues `alpha_k`, `cram.py:34-42`, digit for
/// digit (see [`CRAM16_PFD_THETA`] on the precision allowance).
#[allow(clippy::excessive_precision)]
const CRAM16_PFD_ALPHA: [Complex64; 8] = [
    Complex64::new(-5.0901521865224915650e-7, -2.4220017652852287970e-5),
    Complex64::new(2.1151742182466030907e-4, 4.3892969647380673918e-3),
    Complex64::new(1.1339775178483930527e2, 1.0194721704215856450e2),
    Complex64::new(1.5059585270023467528e1, -5.7514052776421819979),
    Complex64::new(-6.4500878025539646595e1, -2.2459440762652096056e2),
    Complex64::new(-1.4793007113557999718, 1.7686588323782937906),
    Complex64::new(-6.2518392463207918892e1, -1.1190391094283228480e1),
    Complex64::new(4.1023136835410021273e-2, -1.5743466173455468191e-1),
];

/// The limit at infinity `alpha_0`, `cram.py:32`, digit for digit.
#[allow(clippy::excessive_precision)]
const CRAM16_PFD_ALPHA0: f64 = 2.1248537104952237488e-16;

/// `|z|` as `num_complex::Complex64::norm` computed it: `hypot(re, im)`.
#[inline]
fn modulus(z: Complex64) -> f64 {
    crate::real::hypot(z.re, z.im)
}

/// Entry `(row, col)` of a row-stored complex matrix.
fn entry(m: &[Vec<Complex64>], row: usize, col: usize) -> Result<Complex64> {
    m.get(row)
        .and_then(|r| r.get(col))
        .copied()
        .ok_or(PetirError::Invalid)
}

/// Swap entries `a` and `b` of `v` without a subscript.
fn swap<T>(v: &mut [T], a: usize, b: usize) -> Result<()> {
    if a == b {
        return Ok(());
    }
    let (lo, hi) = if a < b { (a, b) } else { (b, a) };
    let (head, tail) = v.split_at_mut_checked(hi).ok_or(PetirError::Invalid)?;
    let x = head.get_mut(lo).ok_or(PetirError::Invalid)?;
    let y = tail.first_mut().ok_or(PetirError::Invalid)?;
    core::mem::swap(x, y);
    Ok(())
}

/// Solve `m x = b` by Gaussian elimination with partial pivoting, the
/// pre-move `complex_solve` of `outram-park-fork-onix` operation for
/// operation: the pivot is the FIRST row at or below `col` with the largest
/// `hypot(re, im)`; a zero multiplier skips its row; the update runs over
/// columns `col..n`; back substitution subtracts left to right.
///
/// # Errors
///
/// [`PetirError::Singular`] when a column's largest modulus is `0.0`
/// (checked before any swap, as the pre-move code did), and
/// [`PetirError::Invalid`] for inconsistent shapes.
fn solve_pfd(mut m: Vec<Vec<Complex64>>, mut b: Vec<Complex64>) -> Result<Vec<Complex64>> {
    let n = b.len();
    if m.len() != n || m.iter().any(|r| r.len() != n) {
        return Err(PetirError::Invalid);
    }
    for col in 0..n {
        let mut pivot = col;
        let mut best = modulus(entry(&m, col, col)?);
        for (row, r) in m.iter().enumerate().skip(col + 1) {
            let mag = modulus(*r.get(col).ok_or(PetirError::Invalid)?);
            if mag > best {
                best = mag;
                pivot = row;
            }
        }
        if best == 0.0 {
            return Err(PetirError::Singular { col });
        }
        swap(&mut m, col, pivot)?;
        swap(&mut b, col, pivot)?;

        let (upper, lower) = m.split_at_mut_checked(col + 1).ok_or(PetirError::Invalid)?;
        let prow = upper.last().ok_or(PetirError::Invalid)?;
        let diag = *prow.get(col).ok_or(PetirError::Invalid)?;
        let (bu, bl) = b.split_at_mut_checked(col + 1).ok_or(PetirError::Invalid)?;
        let bcol = *bu.last().ok_or(PetirError::Invalid)?;
        for (row, brow) in lower.iter_mut().zip(bl.iter_mut()) {
            let factor = row.get(col).ok_or(PetirError::Invalid)?.div(diag);
            if factor.re == 0.0 && factor.im == 0.0 {
                continue;
            }
            for (rk, pk) in row.iter_mut().zip(prow.iter()).skip(col) {
                *rk = rk.sub(factor.mul(*pk));
            }
            *brow = brow.sub(factor.mul(bcol));
        }
    }
    let mut x = vec![Complex64::from_real(0.0); n];
    for i in (0..n).rev() {
        let mrow = m.get(i).ok_or(PetirError::Invalid)?;
        let mut acc = *b.get(i).ok_or(PetirError::Invalid)?;
        for (mij, xj) in mrow.iter().zip(x.iter()).skip(i + 1) {
            acc = acc.sub(mij.mul(*xj));
        }
        let d = *mrow.get(i).ok_or(PetirError::Invalid)?;
        *x.get_mut(i).ok_or(PetirError::Invalid)? = acc.div(d);
    }
    Ok(x)
}

/// ONIX's order-16 CRAM, `n(dt) = exp(A dt) n0`, in partial-fraction form
/// (`onix/salameche/cram.py:6-59`). See the module docs for how it differs
/// from [`super::cram::cram16`].
///
/// * `a` — burnup matrix (`1/s`).
/// * `n0` — initial densities, length `a.order()`.
/// * `dt` — step, seconds.
///
/// Negative entries (a CRAM artefact for species that should be exactly
/// zero) are returned as computed, not clamped.
///
/// # Errors
///
/// [`PetirError::LengthMismatch`] if `n0.len() != a.order()`;
/// [`PetirError::Singular`] if a resolvent `A dt - theta_k I` has a column
/// of zeros during elimination.
pub fn cram16_pfd(a: &DepletionMatrix, n0: &[f64], dt: f64) -> Result<Vec<f64>> {
    let n = a.order();
    if n0.len() != n {
        return Err(PetirError::LengthMismatch {
            expected: n,
            found: n0.len(),
        });
    }
    let src = a.as_slice();
    // Sum of the real parts of the eight solves (cram.py:47-51); the
    // imaginary parts are discarded by `2*_N.real` (cram.py:53).
    let mut acc = vec![0.0_f64; n];
    for (th, al) in CRAM16_PFD_THETA.iter().zip(CRAM16_PFD_ALPHA.iter()) {
        // term1 = At - theta[i]*I (cram.py:48)
        let mut m: Vec<Vec<Complex64>> = src
            .chunks_exact(n.max(1))
            .take(n)
            .map(|r| r.iter().map(|&v| Complex64::new(v * dt, 0.0)).collect())
            .collect();
        for (d, row) in m.iter_mut().enumerate() {
            if let Some(v) = row.get_mut(d) {
                *v = v.sub(*th);
            }
        }
        // term2 = alpha[i]*N_0 (cram.py:49)
        let rhs: Vec<Complex64> = n0.iter().map(|&v| al.mul(Complex64::new(v, 0.0))).collect();
        let y = solve_pfd(m, rhs)?;
        for (s, yi) in acc.iter_mut().zip(&y) {
            *s += yi.re;
        }
    }
    // N = 2*_N.real; N = N + alpha_0*N_0 (cram.py:53-54)
    Ok(acc
        .iter()
        .zip(n0)
        .map(|(s, &v)| 2.0 * s + CRAM16_PFD_ALPHA0 * v)
        .collect())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn matrix_from(n: usize, entries: &[f64]) -> DepletionMatrix {
        let mut a = DepletionMatrix::zeros(n);
        a.as_mut_slice().copy_from_slice(entries);
        a
    }

    /// `dn/dt = -lambda n`, lambda = 1e-3 /s, dt = 500 s: CRAM16 must give
    /// `exp(-0.5)` to round-off (the pre-move unit test, unchanged gate).
    #[test]
    fn single_nuclide_matches_the_exponential() {
        let a = matrix_from(1, &[-1e-3]);
        let n = cram16_pfd(&a, &[1.0], 500.0).unwrap();
        let reference = crate::real::exp(-0.5);
        assert!((n[0] - reference).abs() < 1e-12, "{} vs {reference}", n[0]);
    }

    #[test]
    fn wrong_length_inventory_is_reported() {
        let a = DepletionMatrix::zeros(2);
        assert_eq!(
            cram16_pfd(&a, &[1.0], 1.0),
            Err(PetirError::LengthMismatch {
                expected: 2,
                found: 1
            })
        );
    }

    /// With `dt = 1` and `A` the real 2x2 rotation block of the first pole,
    /// `A - theta_1 I = [[-ib, -b], [b, -ib]]`, which the elimination
    /// reduces to an exact zero in column 1.
    #[test]
    fn a_singular_resolvent_is_an_error_not_a_nan() {
        let th = CRAM16_PFD_THETA[0];
        let a = matrix_from(2, &[th.re, -th.im, th.im, th.re]);
        assert_eq!(
            cram16_pfd(&a, &[1.0, 1.0], 1.0),
            Err(PetirError::Singular { col: 1 })
        );
    }

    /// Cross-check against OpenMC's IPF CRAM16 ([`super::super::cram::cram16`]).
    ///
    /// Methodology: the two are different rational-approximation
    /// evaluations of the same order-16 Pusa approximant, so they must agree
    /// to round-off, and need not agree to the bit (module docs). Cases: an
    /// A -> B -> C chain (lambda 1e-2, 3e-3 /s) at 200 s, a stiff diagonal
    /// (-1, -1e-6 /s) at 5 s, and a stiff 4-nuclide chain with a capture loop
    /// at one day. Criterion: `|pfd - ipf| <= 1e-13` per entry (every `n0`
    /// entry is at most 1, so this is relative to the inventory).
    ///
    /// Result (2026-10-20, x86_64 Linux): worst absolute difference
    /// **6.87e-15**, and **0 of 9** entries bit-equal. Equal to round-off,
    /// not bit-identical, as the module docs predict.
    #[test]
    fn agrees_with_the_ipf_form_to_round_off() {
        let cases: [(usize, &[f64], &[f64], f64); 3] = [
            (
                3,
                &[-1e-2, 0.0, 0.0, 1e-2, -3e-3, 0.0, 0.0, 3e-3, 0.0],
                &[1.0, 0.0, 0.0],
                200.0,
            ),
            (2, &[-1.0, 0.0, 0.0, -1e-6], &[1.0, 1.0], 5.0),
            (
                4,
                &[
                    -2.9e-5, 0.0, 0.0, 7.0e-3, //
                    2.9e-5, -4.9e-4, 0.0, 0.0, //
                    0.0, 4.9e-4, -3.4e-6, 0.0, //
                    0.0, 0.0, 3.4e-6, -7.3e-3,
                ],
                &[1.0, 1e-3, 0.5, 0.0],
                86400.0,
            ),
        ];
        let mut worst = 0.0_f64;
        let mut equal = 0;
        let mut total = 0;
        for (n, entries, n0, dt) in cases {
            let a = matrix_from(n, entries);
            let pfd = cram16_pfd(&a, n0, dt).unwrap();
            let ipf = super::super::cram::cram16(&a, n0, dt).unwrap();
            for (p, q) in pfd.iter().zip(&ipf) {
                worst = worst.max((p - q).abs());
                equal += usize::from(p.to_bits() == q.to_bits());
                total += 1;
            }
        }
        std::println!("pfd vs ipf: worst |diff| {worst:.2e}, bit-equal {equal}/{total}");
        assert!(worst <= 1e-13, "worst |pfd - ipf| = {worst:e}");
    }
}
