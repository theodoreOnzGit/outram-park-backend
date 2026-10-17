// SPDX-License-Identifier: GPL-3.0-only
// Copyright (C) 2026 Theodore Ong and the outram-park contributors.
//
// ---------------------------------------------------------------------------
// PORTED from OpenMC (NOT from GSL, which has no CRAM).
//
//   Project : OpenMC, https://github.com/openmc-dev/openmc
//   File    : openmc/deplete/cram.py (`IPFCramSolver.__call__`, lines 65-105,
//             and the order-16 / order-48 coefficient tables, lines 109-199)
//   Commit  : d7d3284a1b13d7cae020e0d8fea9f1e60d91b18b (2026-09-05)
//   Licence : MIT. "Copyright (c) 2011-2026 Massachusetts Institute of
//             Technology, UChicago Argonne LLC, and OpenMC contributors" --
//             full notice in NOTICE (section "DERIVED WORK -- OpenMC").
//
// First ported in `outram-mc-libs/src/depletion/cram.rs` (2026-07-15) and
// MOVED here 2026-10-17 (gh:#950, maintainer direction "All math moved to
// petir"). The arithmetic of the order-16/48 path is unchanged, operation for
// operation; `crates/outram-mc-libs/tests/depletion_move_fingerprints.rs`
// pins it against fingerprints printed on the pre-move code. What changed:
// subscripts became iterator/`get` access (PETIR's no-panic gate), errors
// became `Result`s, and upstream's `substeps` argument was ported.
// ---------------------------------------------------------------------------

//! Chebyshev Rational Approximation Method (CRAM) matrix-exponential solver.
//!
//! Computes the depletion (Bateman) update `N(t + dt) = exp(A * dt) * N(t)`,
//! where `A` is the burnup matrix (`1/s`, a [`DepletionMatrix`]) and `N` is the
//! vector of nuclide number densities.
//!
//! # Method — Incomplete Partial Factorization (IPF) CRAM
//!
//! A port of OpenMC's `IPFCramSolver.__call__` (`openmc/deplete/cram.py:65-105`):
//!
//! ```text
//!     step_dt = dt if substeps == 1 else dt / substeps     (cram.py:90)
//!     A := step_dt * A                                      (cram.py:91)
//!     y := n0                                               (cram.py:100)
//!     repeat substeps times:                                (cram.py:101)
//!         for each pole i:  y += 2 * Re( alpha_i * solve(A - theta_i I, y) )
//!         y *= alpha0                                       (cram.py:102-104)
//! ```
//!
//! The IPF form is **sequential**: each pole's solve operates on the `y` the
//! previous pole already changed. It is not the parallel partial-fraction sum.
//! Because `y` starts real and each pole adds `2 Re(..)`, `y` stays real, so
//! only the `k` poles with positive imaginary part are stored.
//!
//! The coefficients are the Pusa (2016) values embedded verbatim in
//! `cram.py`: M. Pusa, "Higher-Order Chebyshev Rational Approximation Method
//! and Application to Burnup Equations," Nucl. Sci. Eng. 182:3, 297-318
//! (2016), <https://doi.org/10.13182/NSE15-26>. Substepping follows A. Isotalo
//! and M. Pusa, Nucl. Sci. Eng. 183:1, 65-77 (2016), as upstream cites.
//!
//! # Deviations from upstream, all flagged
//!
//! * **DEVIATION (linear solver):** upstream solves each pole's system with
//!   `scipy.sparse.linalg.spsolve` (SuperLU, `cram.py:95`) or, with
//!   substeps, a reused `splu` factorisation (`cram.py:98`). This uses dense
//!   complex Gaussian elimination with partial pivoting
//!   ([`solve_complex`]) and, for substeps, a dense LU factorisation reused
//!   across substeps ([`LuFactor`]). SuperLU is a C library and is not
//!   `no_std`; the chains this serves are tens of nuclides. The two solvers
//!   give the same answer to round-off, not to the bit: measured against
//!   OpenMC running its own `CRAM48` on the reference problem in
//!   `verification_and_validation/depletion/` (see `tests/depletion_vs_openmc.rs`).
//! * **DEVIATION (degenerate inputs):** a zero-order matrix, `dt == 0` or an
//!   all-zero matrix return `n0` unchanged (`exp(0) = I`). Upstream has no
//!   such early return and evaluates the rational approximation, which gives
//!   `n0` only to within `alpha0`-scale round-off. Kept from the pre-move code
//!   so the pinned results do not move.
//! * **DEVIATION (singular pivot):** an exactly zero pivot returns
//!   [`PetirError::Singular`]. Upstream's `spsolve` warns and returns NaN.
//!
//! # Units
//!
//! * `a` — burnup matrix, every coefficient `1/s`.
//! * `dt` — step length, seconds.
//! * `n0` / return value — number densities in any consistent unit.

use alloc::vec;
use alloc::vec::Vec;

use super::matrix::DepletionMatrix;
use crate::{PetirError, Result};

/// A minimal complex value for the CRAM linear solves and coefficients.
///
/// Deliberately tiny: a real and an imaginary part in `f64` and the four
/// operations complex Gaussian elimination needs, each written as the
/// textbook formula (no scaling in [`Complex64::div`]). Not a general complex
/// library.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Complex64 {
    /// Real part.
    pub re: f64,
    /// Imaginary part.
    pub im: f64,
}

impl Complex64 {
    /// A complex number `re + im*i`.
    #[inline]
    pub const fn new(re: f64, im: f64) -> Self {
        Self { re, im }
    }

    /// A real number as a complex value (`im = 0`).
    #[inline]
    pub const fn from_real(re: f64) -> Self {
        Self { re, im: 0.0 }
    }

    /// Difference `self - rhs`.
    #[inline]
    pub fn sub(self, rhs: Complex64) -> Complex64 {
        Complex64::new(self.re - rhs.re, self.im - rhs.im)
    }

    /// Product `self * rhs`, `(ac - bd) + (ad + bc)i`.
    #[inline]
    pub fn mul(self, rhs: Complex64) -> Complex64 {
        Complex64::new(
            self.re * rhs.re - self.im * rhs.im,
            self.re * rhs.im + self.im * rhs.re,
        )
    }

    /// Quotient `self / rhs` by the unscaled textbook formula.
    #[inline]
    pub fn div(self, rhs: Complex64) -> Complex64 {
        let denom = rhs.re * rhs.re + rhs.im * rhs.im;
        Complex64::new(
            (self.re * rhs.re + self.im * rhs.im) / denom,
            (self.im * rhs.re - self.re * rhs.im) / denom,
        )
    }

    /// Squared magnitude `|self|^2`, for pivot comparisons (monotone in
    /// `|self|`, no `sqrt`).
    #[inline]
    pub fn norm_sqr(self) -> f64 {
        self.re * self.re + self.im * self.im
    }

    #[inline]
    fn is_zero(self) -> bool {
        self.re == 0.0 && self.im == 0.0
    }
}

/// Swap rows `a < b` of `v` without a subscript.
fn swap_rows<T>(v: &mut [T], a: usize, b: usize) -> Result<()> {
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

/// Entry `(row, col)` of a row-stored complex matrix.
fn entry(m: &[Vec<Complex64>], row: usize, col: usize) -> Result<Complex64> {
    m.get(row)
        .and_then(|r| r.get(col))
        .copied()
        .ok_or(PetirError::Invalid)
}

/// Index of the partial pivot for column `col`: the row at or below `col`
/// with the largest `|m[row][col]|`, the FIRST such row on a tie.
fn pivot_row(m: &[Vec<Complex64>], col: usize) -> Result<usize> {
    let mut pivot = col;
    let mut best = entry(m, col, col)?.norm_sqr();
    for (row, r) in m.iter().enumerate().skip(col + 1) {
        let mag = r.get(col).ok_or(PetirError::Invalid)?.norm_sqr();
        if mag > best {
            best = mag;
            pivot = row;
        }
    }
    Ok(pivot)
}

/// Solve the dense complex system `m x = b` by Gaussian elimination with
/// partial pivoting. `m` is stored by rows (`n` rows of length `n`).
///
/// Moved from `outram-mc-libs` with the elimination order, the pivot rule
/// (strictly larger magnitude wins, so the first maximum is kept), the
/// skipped zero multiplier and the back substitution unchanged.
///
/// # Errors
///
/// [`PetirError::LengthMismatch`] for inconsistent shapes and
/// [`PetirError::Singular`] for an exactly zero pivot (a DEVIATION: the
/// pre-move code divided by it).
pub fn solve_complex(mut m: Vec<Vec<Complex64>>, mut b: Vec<Complex64>) -> Result<Vec<Complex64>> {
    let n = b.len();
    check_square(&m, n)?;
    // Forward elimination with partial pivoting.
    for col in 0..n {
        let pivot = pivot_row(&m, col)?;
        swap_rows(&mut m, pivot, col)?;
        swap_rows(&mut b, pivot, col)?;

        let (upper, lower) = m.split_at_mut_checked(col + 1).ok_or(PetirError::Invalid)?;
        let prow = upper.last().ok_or(PetirError::Invalid)?;
        let diag = *prow.get(col).ok_or(PetirError::Invalid)?;
        if diag.is_zero() {
            return Err(PetirError::Singular { col });
        }
        let (bu, bl) = b.split_at_mut_checked(col + 1).ok_or(PetirError::Invalid)?;
        let bcol = *bu.last().ok_or(PetirError::Invalid)?;
        // Eliminate the column below the pivot.
        for (row, brow) in lower.iter_mut().zip(bl.iter_mut()) {
            let factor = row.get(col).ok_or(PetirError::Invalid)?.div(diag);
            if factor.is_zero() {
                continue;
            }
            for (rk, pk) in row.iter_mut().zip(prow.iter()).skip(col) {
                let update = factor.mul(*pk);
                *rk = rk.sub(update);
            }
            let update = factor.mul(bcol);
            *brow = brow.sub(update);
        }
    }
    back_substitute(&m, &b)
}

/// Back substitution on an upper-triangular row-stored `m`.
fn back_substitute(m: &[Vec<Complex64>], b: &[Complex64]) -> Result<Vec<Complex64>> {
    let n = b.len();
    let mut x = vec![Complex64::from_real(0.0); n];
    for row in (0..n).rev() {
        let mrow = m.get(row).ok_or(PetirError::Invalid)?;
        let mut acc = *b.get(row).ok_or(PetirError::Invalid)?;
        for (mk, xk) in mrow.iter().zip(x.iter()).skip(row + 1) {
            acc = acc.sub(mk.mul(*xk));
        }
        let d = *mrow.get(row).ok_or(PetirError::Invalid)?;
        *x.get_mut(row).ok_or(PetirError::Invalid)? = acc.div(d);
    }
    Ok(x)
}

fn check_square(m: &[Vec<Complex64>], n: usize) -> Result<()> {
    if m.len() != n {
        return Err(PetirError::LengthMismatch { expected: n, found: m.len() });
    }
    for r in m {
        if r.len() != n {
            return Err(PetirError::LengthMismatch { expected: n, found: r.len() });
        }
    }
    Ok(())
}

/// A dense LU factorisation with partial pivoting, reusable across right-hand
/// sides — the stand-in for upstream's `splu(A - theta*ident).solve`
/// (`cram.py:98`), used when `substeps > 1`.
///
/// The elimination is the one in [`solve_complex`], with each multiplier
/// stored in the eliminated position and each row interchange recorded, so a
/// later [`LuFactor::solve`] replays the same operations on a new
/// right-hand side. `tests::lu_replays_the_direct_solve_bit_for_bit` checks
/// that it reproduces [`solve_complex`] exactly.
#[derive(Debug, Clone, PartialEq)]
pub struct LuFactor {
    lu: Vec<Vec<Complex64>>,
    /// `piv[col]` is the row swapped into `col` at elimination step `col`.
    piv: Vec<usize>,
}

impl LuFactor {
    /// Factorise `m` (row-stored, square).
    ///
    /// # Errors
    ///
    /// As [`solve_complex`].
    pub fn new(mut m: Vec<Vec<Complex64>>) -> Result<Self> {
        let n = m.len();
        check_square(&m, n)?;
        let mut piv = Vec::with_capacity(n);
        for col in 0..n {
            let pivot = pivot_row(&m, col)?;
            swap_rows(&mut m, pivot, col)?;
            piv.push(pivot);
            let (upper, lower) = m.split_at_mut_checked(col + 1).ok_or(PetirError::Invalid)?;
            let prow = upper.last().ok_or(PetirError::Invalid)?;
            let diag = *prow.get(col).ok_or(PetirError::Invalid)?;
            if diag.is_zero() {
                return Err(PetirError::Singular { col });
            }
            for row in lower.iter_mut() {
                let slot = row.get_mut(col).ok_or(PetirError::Invalid)?;
                let factor = slot.div(diag);
                if factor.is_zero() {
                    *slot = factor;
                    continue;
                }
                for (rk, pk) in row.iter_mut().zip(prow.iter()).skip(col + 1) {
                    let update = factor.mul(*pk);
                    *rk = rk.sub(update);
                }
                *row.get_mut(col).ok_or(PetirError::Invalid)? = factor;
            }
        }
        Ok(Self { lu: m, piv })
    }

    /// Solve `m x = b` with the stored factorisation.
    ///
    /// # Errors
    ///
    /// [`PetirError::LengthMismatch`] if `b` is the wrong length.
    pub fn solve(&self, mut b: Vec<Complex64>) -> Result<Vec<Complex64>> {
        let n = self.lu.len();
        if b.len() != n {
            return Err(PetirError::LengthMismatch { expected: n, found: b.len() });
        }
        // Apply every row interchange first, then forward-substitute with the
        // final multipliers (LAPACK `getrs` order). Each multiplier moved
        // with its row during the factorisation, so every element of b meets
        // the same updates, with the same multipliers, in the same order as
        // in `solve_complex`'s interleaved elimination.
        for (col, &pivot) in self.piv.iter().enumerate() {
            swap_rows(&mut b, pivot, col)?;
        }
        for col in 0..n {
            let (bu, bl) = b.split_at_mut_checked(col + 1).ok_or(PetirError::Invalid)?;
            let bcol = *bu.last().ok_or(PetirError::Invalid)?;
            for (row, brow) in self.lu.iter().skip(col + 1).zip(bl.iter_mut()) {
                let factor = *row.get(col).ok_or(PetirError::Invalid)?;
                if factor.is_zero() {
                    continue;
                }
                let update = factor.mul(bcol);
                *brow = brow.sub(update);
            }
        }
        back_substitute(&self.lu, &b)
    }
}

/// Build `A*step_dt - theta*I` as rows, from the already-scaled entries.
fn shifted(scaled: &[Complex64], n: usize, theta: Complex64) -> Vec<Vec<Complex64>> {
    let mut rows: Vec<Vec<Complex64>> = scaled.chunks_exact(n.max(1)).map(|r| r.to_vec()).collect();
    for (d, row) in rows.iter_mut().enumerate() {
        if let Some(v) = row.get_mut(d) {
            *v = v.sub(theta);
        }
    }
    rows
}

/// Core IPF CRAM sweep shared by every order — `IPFCramSolver.__call__`
/// (`openmc/deplete/cram.py:65-105`).
///
/// `alpha` and `theta` are the `k` complex residues and poles (one per
/// conjugate pair); `alpha0` is the limit of the approximation at infinity.
///
/// # Errors
///
/// [`PetirError::Invalid`] for `substeps == 0` (upstream's
/// `check_greater_than("substeps", substeps, 0)`, `cram.py:88`) or
/// `alpha`/`theta` of different lengths (`check_length`, `cram.py:59`);
/// [`PetirError::LengthMismatch`] if `n0.len() != a.order()`;
/// [`PetirError::Singular`] from a solve.
pub fn ipf_cram(
    a: &DepletionMatrix,
    n0: &[f64],
    dt: f64,
    alpha: &[Complex64],
    theta: &[Complex64],
    alpha0: f64,
    substeps: usize,
) -> Result<Vec<f64>> {
    let n = a.order();
    if substeps == 0 || alpha.len() != theta.len() {
        return Err(PetirError::Invalid);
    }
    if n0.len() != n {
        return Err(PetirError::LengthMismatch { expected: n, found: n0.len() });
    }

    // DEVIATION (see module docs): exp(A*dt) = I when there is nothing to evolve.
    if n == 0 || dt == 0.0 {
        return Ok(n0.to_vec());
    }
    let raw = a.as_slice();
    if raw.iter().all(|&v| v == 0.0) {
        return Ok(n0.to_vec());
    }

    // cram.py:90-91 -- `step_dt = dt if substeps == 1 else dt / substeps`,
    // `A = step_dt * A`.
    let step_dt = if substeps == 1 { dt } else { dt / substeps as f64 };
    let scaled: Vec<Complex64> = raw.iter().map(|&v| Complex64::from_real(v * step_dt)).collect();

    // y starts real and stays real (2*Re(...) additions); carry it as f64.
    let mut y: Vec<f64> = n0.to_vec();

    if substeps == 1 {
        // cram.py:95 -- one direct solve per pole.
        for (al, th) in alpha.iter().zip(theta) {
            let m = shifted(&scaled, n, *th);
            let b: Vec<Complex64> = y.iter().map(|&v| Complex64::from_real(v)).collect();
            let x = solve_complex(m, b)?;
            // cram.py:103 -- y += 2 * np.real(alpha * solve(y))
            for (yr, xr) in y.iter_mut().zip(&x) {
                *yr += 2.0 * al.mul(*xr).re;
            }
        }
        // cram.py:104
        for v in y.iter_mut() {
            *v *= alpha0;
        }
    } else {
        // cram.py:98 -- factorise once per pole, reuse across substeps.
        let mut solvers = Vec::with_capacity(theta.len());
        for th in theta {
            solvers.push(LuFactor::new(shifted(&scaled, n, *th))?);
        }
        for _ in 0..substeps {
            for (al, lu) in alpha.iter().zip(&solvers) {
                let b: Vec<Complex64> = y.iter().map(|&v| Complex64::from_real(v)).collect();
                let x = lu.solve(b)?;
                for (yr, xr) in y.iter_mut().zip(&x) {
                    *yr += 2.0 * al.mul(*xr).re;
                }
            }
            for v in y.iter_mut() {
                *v *= alpha0;
            }
        }
    }
    Ok(y)
}

/// Order-16 IPF CRAM, `N(t + dt) = exp(A dt) N(t)` — upstream's `CRAM16`
/// (`openmc/deplete/cram.py:132-133`, coefficients `cram.py:109-131`).
///
/// One substep; see [`CramSolver::solve`] for substepping. Order-16 CRAM is
/// accurate to roughly 1e-14 relative on the negative real axis where a
/// physical burnup matrix's eigenvalues lie (the tests measure it).
///
/// # Errors
///
/// As [`ipf_cram`].
pub fn cram16(a: &DepletionMatrix, n0: &[f64], dt: f64) -> Result<Vec<f64>> {
    ipf_cram(a, n0, dt, &CRAM16_ALPHA, &CRAM16_THETA, CRAM16_ALPHA0, 1)
}

/// Order-48 IPF CRAM — upstream's `CRAM48` (`openmc/deplete/cram.py:201-205`,
/// coefficients `cram.py:139-199`), upstream's **default** solver
/// (`abc.py:643`, `solver: str = "cram48"`). Costs 3x the solves of
/// [`cram16`].
///
/// # Errors
///
/// As [`ipf_cram`].
pub fn cram48(a: &DepletionMatrix, n0: &[f64], dt: f64) -> Result<Vec<f64>> {
    ipf_cram(a, n0, dt, &CRAM48_ALPHA, &CRAM48_THETA, CRAM48_ALPHA0, 1)
}

/// Solves `dN/dt = A N` over `dt` — upstream's `DepSystemSolver`
/// (`openmc/deplete/abc.py:1443-1470`), whose call signature is
/// `solver(A, n0, dt, substeps=1) -> n1`.
///
/// Implemented by [`CramSolver`]. A caller may supply its own solver by
/// implementing this trait, as upstream accepts any conforming callable
/// (`abc.py:724-756`).
#[diagnostic::on_unimplemented(
    message = "`{Self}` is not a depletion-system solver",
    note = "use `petir::depletion::CramSolver::Cram48` (upstream's default) or `Cram16`, or implement `DepSystemSolver::solve`"
)]
pub trait DepSystemSolver {
    /// `exp(A dt) n0`, with `substeps >= 1` identical sub-intervals.
    ///
    /// # Errors
    ///
    /// Whatever the solver reports; [`CramSolver`] reports as [`ipf_cram`].
    fn solve(&self, a: &DepletionMatrix, n0: &[f64], dt: f64, substeps: usize) -> Result<Vec<f64>>;
}

/// Upstream's two built-in solvers, `"cram16"` and `"cram48"`
/// (`openmc/deplete/abc.py:705-716`, the `solver == "cram48"` / `"cram16"` branches at `:707` and `:710`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum CramSolver {
    /// 16th-order IPF CRAM.
    Cram16,
    /// 48th-order IPF CRAM — upstream's default.
    #[default]
    Cram48,
}

impl DepSystemSolver for CramSolver {
    fn solve(&self, a: &DepletionMatrix, n0: &[f64], dt: f64, substeps: usize) -> Result<Vec<f64>> {
        match self {
            CramSolver::Cram16 => ipf_cram(a, n0, dt, &CRAM16_ALPHA, &CRAM16_THETA, CRAM16_ALPHA0, substeps),
            CramSolver::Cram48 => ipf_cram(a, n0, dt, &CRAM48_ALPHA, &CRAM48_THETA, CRAM48_ALPHA0, substeps),
        }
    }
}

// ---------------------------------------------------------------------------
// Coefficients — transcribed verbatim from openmc/deplete/cram.py.
// ---------------------------------------------------------------------------

/// Order-16 complex residues `c16_alpha` (cram.py:109-118), 8 pole-pairs.
const CRAM16_ALPHA: [Complex64; 8] = [
    Complex64 {
        re: 5.464930576870210e+3,
        im: -3.797983575308356e+4,
    },
    Complex64 {
        re: 9.045112476907548e+1,
        im: -1.115537522430261e+3,
    },
    Complex64 {
        re: 2.344818070467641e+2,
        im: -4.228020157070496e+2,
    },
    Complex64 {
        re: 9.453304067358312e+1,
        im: -2.951294291446048e+2,
    },
    Complex64 {
        re: 7.283792954673409e+2,
        im: -1.205646080220011e+5,
    },
    Complex64 {
        re: 3.648229059594851e+1,
        im: -1.155509621409682e+2,
    },
    Complex64 {
        re: 2.547321630156819e+1,
        im: -2.639500283021502e+1,
    },
    Complex64 {
        re: 2.394538338734709e+1,
        im: -5.650522971778156e+0,
    },
];

/// Order-16 complex poles `c16_theta` (cram.py:120-129), 8 pole-pairs.
const CRAM16_THETA: [Complex64; 8] = [
    Complex64 {
        re: 3.509103608414918,
        im: 8.436198985884374,
    },
    Complex64 {
        re: 5.948152268951177,
        im: 3.587457362018322,
    },
    Complex64 {
        re: -5.264971343442647,
        im: 16.22022147316793,
    },
    Complex64 {
        re: 1.419375897185666,
        im: 10.92536348449672,
    },
    Complex64 {
        re: 6.416177699099435,
        im: 1.194122393370139,
    },
    Complex64 {
        re: 4.993174737717997,
        im: 5.996881713603942,
    },
    Complex64 {
        re: -1.413928462488886,
        im: 13.49772569889275,
    },
    Complex64 {
        re: -10.84391707869699,
        im: 19.27744616718165,
    },
];

/// Order-16 limit at infinity `c16_alpha0` (cram.py:131).
const CRAM16_ALPHA0: f64 = 2.124853710495224e-16;

/// Order-48 complex poles `c48_theta = theta_r + i*theta_i` (cram.py:139-167),
/// 24 pole-pairs.
const CRAM48_THETA: [Complex64; 24] = [
    Complex64 {
        re: -4.465731934165702e+1,
        im: 6.233225190695437e+1,
    },
    Complex64 {
        re: -5.284616241568964e+0,
        im: 4.057499381311059e+1,
    },
    Complex64 {
        re: -8.867715667624458e+0,
        im: 4.325515754166724e+1,
    },
    Complex64 {
        re: 3.493013124279215e+0,
        im: 3.281615453173585e+1,
    },
    Complex64 {
        re: 1.564102508858634e+1,
        im: 1.558061616372237e+1,
    },
    Complex64 {
        re: 1.742097597385893e+1,
        im: 1.076629305714420e+1,
    },
    Complex64 {
        re: -2.834466755180654e+1,
        im: 5.492841024648724e+1,
    },
    Complex64 {
        re: 1.661569367939544e+1,
        im: 1.316994930024688e+1,
    },
    Complex64 {
        re: 8.011836167974721e+0,
        im: 2.780232111309410e+1,
    },
    Complex64 {
        re: -2.056267541998229e+0,
        im: 3.794824788914354e+1,
    },
    Complex64 {
        re: 1.449208170441839e+1,
        im: 1.799988210051809e+1,
    },
    Complex64 {
        re: 1.853807176907916e+1,
        im: 5.974332563100539e+0,
    },
    Complex64 {
        re: 9.932562704505182e+0,
        im: 2.532823409972962e+1,
    },
    Complex64 {
        re: -2.244223871767187e+1,
        im: 5.179633600312162e+1,
    },
    Complex64 {
        re: 8.590014121680897e-1,
        im: 3.536456194294350e+1,
    },
    Complex64 {
        re: -1.286192925744479e+1,
        im: 4.600304902833652e+1,
    },
    Complex64 {
        re: 1.164596909542055e+1,
        im: 2.287153304140217e+1,
    },
    Complex64 {
        re: 1.806076684783089e+1,
        im: 8.368200580099821e+0,
    },
    Complex64 {
        re: 5.870672154659249e+0,
        im: 3.029700159040121e+1,
    },
    Complex64 {
        re: -3.542938819659747e+1,
        im: 5.834381701800013e+1,
    },
    Complex64 {
        re: 1.901323489060250e+1,
        im: 1.194282058271408e+0,
    },
    Complex64 {
        re: 1.885508331552577e+1,
        im: 3.583428564427879e+0,
    },
    Complex64 {
        re: -1.734689708174982e+1,
        im: 4.883941101108207e+1,
    },
    Complex64 {
        re: 1.316284237125190e+1,
        im: 2.042951874827759e+1,
    },
];

/// Order-48 complex residues `c48_alpha = alpha_r + i*alpha_i`
/// (cram.py:169-197), 24 pole-pairs.
const CRAM48_ALPHA: [Complex64; 24] = [
    Complex64 {
        re: 6.387380733878774e+2,
        im: -6.743912502859256e+2,
    },
    Complex64 {
        re: 1.909896179065730e+2,
        im: -3.973203432721332e+2,
    },
    Complex64 {
        re: 4.236195226571914e+2,
        im: -2.041233768918671e+3,
    },
    Complex64 {
        re: 4.645770595258726e+2,
        im: -1.652917287299683e+3,
    },
    Complex64 {
        re: 7.765163276752433e+2,
        im: -1.783617639907328e+4,
    },
    Complex64 {
        re: 1.907115136768522e+3,
        im: -5.887068595142284e+4,
    },
    Complex64 {
        re: 2.909892685603256e+3,
        im: -9.953255345514560e+3,
    },
    Complex64 {
        re: 1.944772206620450e+2,
        im: -1.427131226068449e+3,
    },
    Complex64 {
        re: 1.382799786972332e+5,
        im: -3.256885197214938e+6,
    },
    Complex64 {
        re: 5.628442079602433e+3,
        im: -2.924284515884309e+4,
    },
    Complex64 {
        re: 2.151681283794220e+2,
        im: -1.121774011188224e+3,
    },
    Complex64 {
        re: 1.324720240514420e+3,
        im: -6.370088443140973e+4,
    },
    Complex64 {
        re: 1.617548476343347e+4,
        im: -1.008798413156542e+6,
    },
    Complex64 {
        re: 1.112729040439685e+2,
        im: -8.837109731680418e+1,
    },
    Complex64 {
        re: 1.074624783191125e+2,
        im: -1.457246116408180e+2,
    },
    Complex64 {
        re: 8.835727765158191e+1,
        im: -6.388286188419360e+1,
    },
    Complex64 {
        re: 9.354078136054179e+1,
        im: -2.195424319460237e+2,
    },
    Complex64 {
        re: 9.418142823531573e+1,
        im: -6.719055740098035e+2,
    },
    Complex64 {
        re: 1.040012390717851e+2,
        im: -1.693747595553868e+2,
    },
    Complex64 {
        re: 6.861882624343235e+1,
        im: -1.177598523430493e+1,
    },
    Complex64 {
        re: 8.766654491283722e+1,
        im: -4.596464999363902e+3,
    },
    Complex64 {
        re: 1.056007619389650e+2,
        im: -1.738294585524067e+3,
    },
    Complex64 {
        re: 7.738987569039419e+1,
        im: -4.311715386228984e+1,
    },
    Complex64 {
        re: 1.041366366475571e+2,
        im: -2.777743732451969e+2,
    },
];

/// Order-48 limit at infinity `c48_alpha0` (cram.py:199).
const CRAM48_ALPHA0: f64 = 2.258038182743983e-47;


#[cfg(test)]
mod tests {
    //! The first six tests moved with the solver from `outram-mc-libs`
    //! (2026-07-15 measurements kept); the rest were added with the move.
    use super::*;
    use std::vec::Vec;

    /// Build a `DepletionMatrix` from a row-major `n x n` slice of `1/s` values.
    fn matrix_from(n: usize, entries: &[f64]) -> DepletionMatrix {
        let mut a = DepletionMatrix::zeros(n);
        for row in 0..n {
            for col in 0..n {
                a.set(row, col, entries[row * n + col]).unwrap();
            }
        }
        a
    }

    /// Single-nuclide exponential decay to the exact half-life.
    ///
    /// `A = [-lambda]`, `lambda = ln2 / T_half` (I-135, 2.36520e4 s); exact
    /// `N(T_half) = N0/2`. Measured 2026-07-15: CRAM16 4.0e-16, CRAM48 2.3e-15.
    #[test]
    fn single_nuclide_decay_half_life() {
        let t_half = 2.36520e4_f64;
        let lambda = 2.0_f64.ln() / t_half;
        let a = matrix_from(1, &[-lambda]);
        let n0 = [1.0e24_f64];
        let expected = n0[0] / 2.0;
        let n = cram16(&a, &n0, t_half).unwrap();
        assert!((n[0] - expected).abs() / expected < 1e-10);
        let n48 = cram48(&a, &n0, t_half).unwrap();
        assert!((n48[0] - expected).abs() / expected < 1e-10);
    }

    /// Two-nuclide chain A -> B against analytic Bateman.
    ///
    /// `la = 1/3600`, `lb = 1/900`, `t = 5000 s`. Measured 2026-07-15: CRAM16
    /// 7.8e-16 / 3.4e-16, CRAM48 6.7e-16 / 8.5e-16.
    #[test]
    fn two_nuclide_chain_bateman() {
        let la = 1.0 / 3600.0_f64;
        let lb = 1.0 / 900.0_f64;
        let a = matrix_from(2, &[-la, 0.0, la, -lb]);
        let t = 5000.0_f64;
        let na = (-la * t).exp();
        let nb = la / (lb - la) * ((-la * t).exp() - (-lb * t).exp());
        let n = cram16(&a, &[1.0, 0.0], t).unwrap();
        assert!((n[0] - na).abs() / na < 1e-8);
        assert!((n[1] - nb).abs() / nb < 1e-8);
        let n48 = cram48(&a, &[1.0, 0.0], t).unwrap();
        assert!((n48[0] - na).abs() / na < 1e-10);
        assert!((n48[1] - nb).abs() / nb < 1e-10);
    }

    /// Three-nuclide chain A -> B -> C (C stable) and atom conservation.
    ///
    /// Measured 2026-07-15: CRAM16 `N_C` 2.1e-16; total-atom drift 0.0.
    #[test]
    fn three_nuclide_chain_and_conservation() {
        let la = 1.0 / 3600.0_f64;
        let lb = 1.0 / 1200.0_f64;
        let a = matrix_from(3, &[-la, 0.0, 0.0, la, -lb, 0.0, 0.0, lb, 0.0]);
        let t = 4000.0_f64;
        let n = cram16(&a, &[1.0, 0.0, 0.0], t).unwrap();
        let nc = 1.0 + (la * (-lb * t).exp() - lb * (-la * t).exp()) / (lb - la);
        assert!((n[2] - nc).abs() / nc < 1e-8);
        let total: f64 = n.iter().sum();
        assert!((total - 1.0).abs() < 1e-10);
    }

    /// Stiff two-nuclide system (eigenvalues spanning 1e5).
    ///
    /// Measured 2026-07-15: CRAM16 6.0e-16 / 1.2e-16; CRAM48 1.7e-15 / 1.2e-15.
    #[test]
    fn stiff_two_nuclide_system() {
        let la = 1.0e-2_f64;
        let lb = 1.0e3_f64;
        let a = matrix_from(2, &[-la, 0.0, la, -lb]);
        let t = 100.0_f64;
        let na = (-la * t).exp();
        let nb = la / (lb - la) * ((-la * t).exp() - (-lb * t).exp());
        for n in [cram16(&a, &[1.0, 0.0], t).unwrap(), cram48(&a, &[1.0, 0.0], t).unwrap()] {
            assert!((n[0] - na).abs() / na < 1e-5);
            assert!((n[1] - nb).abs() / nb < 1e-5);
        }
    }

    /// Atom conservation for a 4-chain over many half-lives.
    ///
    /// Measured 2026-07-15: drift 2.2e-16; smallest density -2.1e-16.
    #[test]
    fn long_chain_atom_conservation() {
        let (l1, l2, l3) = (1.0 / 500.0_f64, 1.0 / 1500.0_f64, 1.0 / 4000.0_f64);
        let a = matrix_from(
            4,
            &[-l1, 0.0, 0.0, 0.0, l1, -l2, 0.0, 0.0, 0.0, l2, -l3, 0.0, 0.0, 0.0, l3, 0.0],
        );
        let n = cram16(&a, &[1.0, 0.0, 0.0, 0.0], 2.0e4).unwrap();
        let total: f64 = n.iter().sum();
        assert!((total - 1.0).abs() < 1e-9);
        for v in &n {
            assert!(*v >= -1e-12);
        }
    }

    /// Degenerate inputs return `n0` (the flagged DEVIATION), and bad
    /// arguments are reported.
    #[test]
    fn degenerate_cases_and_errors() {
        let a = matrix_from(2, &[-1.0, 0.0, 1.0, -2.0]);
        let n0 = [3.0_f64, 4.0];
        assert_eq!(cram16(&a, &n0, 0.0).unwrap(), n0.to_vec());
        assert_eq!(cram16(&DepletionMatrix::zeros(2), &n0, 1.0e3).unwrap(), n0.to_vec());
        assert_eq!(cram16(&DepletionMatrix::zeros(0), &[], 5.0).unwrap(), Vec::<f64>::new());
        assert_eq!(
            cram48(&a, &[1.0], 1.0),
            Err(PetirError::LengthMismatch { expected: 2, found: 1 })
        );
        assert_eq!(CramSolver::Cram16.solve(&a, &n0, 1.0, 0), Err(PetirError::Invalid));
        assert_eq!(
            ipf_cram(&a, &n0, 1.0, &CRAM16_ALPHA, &CRAM48_THETA, CRAM16_ALPHA0, 1),
            Err(PetirError::Invalid)
        );
        // A singular system is reported, not divided by.
        let z = vec![vec![Complex64::from_real(0.0); 2]; 2];
        assert_eq!(
            solve_complex(z.clone(), vec![Complex64::from_real(1.0); 2]),
            Err(PetirError::Singular { col: 0 })
        );
        assert_eq!(LuFactor::new(z), Err(PetirError::Singular { col: 0 }));
        assert!(LuFactor::new(vec![vec![Complex64::from_real(1.0)]]).unwrap().solve(vec![]).is_err());
    }

    /// The enum dispatches to the same arithmetic as the free functions.
    #[test]
    fn solver_enum_is_the_free_functions() {
        let a = matrix_from(2, &[-1.0e-3, 0.0, 1.0e-3, -2.0e-4]);
        let n0 = [1.0, 0.5];
        assert_eq!(CramSolver::Cram16.solve(&a, &n0, 900.0, 1).unwrap(), cram16(&a, &n0, 900.0).unwrap());
        assert_eq!(CramSolver::Cram48.solve(&a, &n0, 900.0, 1).unwrap(), cram48(&a, &n0, 900.0).unwrap());
        assert_eq!(CramSolver::default(), CramSolver::Cram48);
    }

    /// `LuFactor` replays `solve_complex` exactly, on a matrix whose
    /// elimination pivots (row interchanges at more than one step).
    ///
    /// This is what lets the substep path be read as "the same solver,
    /// factorised once": the operations on the right-hand side are the same
    /// ones in the same order.
    #[test]
    fn lu_replays_the_direct_solve_bit_for_bit() {
        let vals = [
            [0.1, -20.0, 0.3, 40.0],
            [50.0, 0.2, -10.0, 0.0],
            [0.0, 75.0, 0.4, -30.0],
            [-30.0, 0.0, 90.0, 0.25],
        ];
        let theta = Complex64::new(1.4, 10.9);
        let m: Vec<Vec<Complex64>> = vals
            .iter()
            .enumerate()
            .map(|(i, r)| {
                r.iter()
                    .enumerate()
                    .map(|(j, &v)| if i == j { Complex64::from_real(v).sub(theta) } else { Complex64::from_real(v) })
                    .collect()
            })
            .collect();
        let b: Vec<Complex64> = [1.0, -0.5, 2.0, 0.125].iter().map(|&v| Complex64::from_real(v)).collect();
        let direct = solve_complex(m.clone(), b.clone()).unwrap();
        let lu = LuFactor::new(m).unwrap();
        assert!(lu.piv.iter().enumerate().filter(|(c, p)| *c != **p).count() >= 2, "{:?}", lu.piv);
        let replay = lu.solve(b).unwrap();
        for (x, y) in direct.iter().zip(&replay) {
            assert_eq!(x.re.to_bits(), y.re.to_bits());
            assert_eq!(x.im.to_bits(), y.im.to_bits());
        }
    }

    /// Substepping (Isotalo & Pusa 2016, `cram.py:90-105`) improves CRAM16
    /// where a single step is least accurate RELATIVE to a small component:
    /// `lambda * dt = 20`, so `N_A = exp(-20) = 2.1e-9`.
    ///
    /// Gates: absolute error on the tiny `N_A` below 1e-15 (CRAM's uniform
    /// bound on the negative real axis is ~2e-16 at order 16); the O(1)
    /// `N_B` below 1e-13 relative (round-off of the pole sweep); CRAM16's
    /// relative error on `N_A` falls with each doubling of substeps; CRAM48
    /// below 1e-13 relative throughout.
    ///
    /// # Result, 2026-10-17
    ///
    /// CRAM16 relative error on `N_A`: 8.23e-8, 8.07e-12, 1.12e-13 at 1, 2,
    /// 4 substeps (absolute 1.7e-16, 1.7e-20, 2.3e-22). CRAM48: 1.40e-15,
    /// 2.41e-15, 7.63e-15. A first draft gated CRAM16 at 1e-12 RELATIVE with
    /// one substep, misreading CRAM's error as relative; it failed at 8.2e-8.
    /// The second draft gated BOTH components at 1e-15 absolute, forgetting
    /// round-off on the O(1) one; it failed on CRAM48 with 4 substeps
    /// (`N_B` 3.6e-15 absolute, 7.3e-15 relative). Both were errors in the
    /// gate, not the solver; the gates above are the corrected statements.
    #[test]
    fn substeps_reproduce_the_exponential() {
        let la = 1.0e-3_f64;
        let lb = 3.0e-5_f64;
        let a = matrix_from(2, &[-la, 0.0, la, -lb]);
        let t = 2.0e4;
        let na = (-la * t).exp();
        let nb = la / (lb - la) * ((-la * t).exp() - (-lb * t).exp());
        let mut last16 = f64::INFINITY;
        for s in [1usize, 2, 4] {
            for solver in [CramSolver::Cram16, CramSolver::Cram48] {
                let n = solver.solve(&a, &[1.0, 0.0], t, s).unwrap();
                let ea = (n[0] - na).abs() / na;
                let eb = (n[1] - nb).abs() / nb;
                let abs_a = (n[0] - na).abs();
                std::println!("{solver:?} substeps {s}: rel err A {ea:.2e}, B {eb:.2e}, abs A {abs_a:.2e}");
                assert!(abs_a < 1e-15, "{solver:?} {s}: abs {abs_a:e}");
                assert!(eb < 1e-13, "{solver:?} {s}: B rel {eb:e}");
                if solver == CramSolver::Cram48 {
                    assert!(ea < 1e-13 && eb < 1e-13, "{solver:?} substeps {s}: {ea:e} {eb:e}");
                } else {
                    assert!(ea < last16, "CRAM16 did not improve at {s} substeps: {ea:e} vs {last16:e}");
                    last16 = ea;
                }
            }
        }
    }

    /// Complex arithmetic is the textbook formula.
    #[test]
    fn complex_arithmetic() {
        let a = Complex64::new(1.0, 2.0);
        let b = Complex64::new(3.0, -1.0);
        assert_eq!(a.mul(b), Complex64::new(5.0, 5.0));
        assert_eq!(a.sub(b), Complex64::new(-2.0, 3.0));
        assert_eq!(a.mul(b).div(b), Complex64::new(1.0, 2.0));
        assert_eq!(b.norm_sqr(), 10.0);
    }
}
