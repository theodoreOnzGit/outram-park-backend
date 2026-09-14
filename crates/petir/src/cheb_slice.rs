// Copyright (C) 2026 Theodore Ong and the outram-park contributors. GPL-3.0-only.
//
// `eval_plain` / `eval_gsl` are the Clenshaw body of GSL 2.8 `cheb/eval.c`
// (`gsl_cheb_eval`), Copyright (C) 1996-2000 Gerard Jungman, GPL-3.0-or-later,
// read at commit cf180cd7fbd06039a577f9c9ff0b428784765ac1 -- the same
// arithmetic as `ChebSeries::eval`, over borrowed coefficients.
//
// `basis`, `eval2_dense` and `eval2_sparse` are LIFTED from this workspace's
// own `tampines-steam-tables`
// (src/backward_eqn_chebyshev_experimental/chebyshev.rs), per bn:op-chyp.2,
// which names that module as "existing evaluation code to lift rather than
// rewrite". GSL's cheb module is one-dimensional and has no tensor-product
// evaluator, so there is nothing upstream to port for those.

//! Allocation-free Chebyshev evaluation over borrowed coefficients, and the
//! two-dimensional tensor-product evaluators.
//!
//! # Why this exists next to [`ChebSeries`](crate::ChebSeries)
//!
//! [`ChebSeries`](crate::ChebSeries) owns a `Vec` because that is what
//! *fitting* needs — `gsl_cheb_init` sizes its coefficient array at runtime.
//! **Evaluating a table that is already known** is a different job: steam-table
//! backward correlations, ACE-format tabulations and published fits all hold
//! their coefficients as `const [f64; N]`, and are evaluated inside solver
//! loops where a per-call allocation is a regression.
//!
//! These functions take `&[f64]`, allocate nothing, and are usable in `const`
//! contexts' neighbourhood — the same arithmetic, different storage.
//!
//! # The two conventions, again
//!
//! [`eval_gsl`] closes Clenshaw with `0.5 * c[0]`, matching `gsl_cheb_eval` and
//! the coefficients `gsl_cheb_init` produces. [`eval_plain`] closes with
//! `c[0]`, matching most published tables. They differ by exactly `c[0]/2` and
//! mixing them is silent — see
//! [`ChebSeries::from_plain_coefficients`](crate::ChebSeries::from_plain_coefficients).

/// Map `v` from `[lo, hi]` onto the Chebyshev interval `[-1, 1]`.
///
/// Does **not** clamp: outside `[lo, hi]` the recurrence extrapolates and
/// diverges quickly, which is the caller's business to avoid.
///
/// # The expression is GSL's, deliberately
///
/// This is written as `(2v - lo - hi) / (hi - lo)`, which is
/// `gsl_cheb_eval`'s own line (`cheb/eval.c:35`), **not** the algebraically
/// identical `2(v - lo)/(hi - lo) - 1` that `tampines-steam-tables`' `scale`
/// uses. The two differ in the last ulp, and picking the other one makes
/// [`eval_gsl`] disagree with
/// [`ChebSeries::eval`](crate::ChebSeries::eval) by 1 ulp — which
/// `eval_gsl_matches_the_owning_series_bit_for_bit` catches. Since this crate
/// is a GSL port, GSL's form is the one that belongs here.
#[inline]
pub fn scale(v: f64, lo: f64, hi: f64) -> f64 {
    (2.0 * v - lo - hi) / (hi - lo)
}

/// Chebyshev polynomials of the first kind `T_0(x) ..= T_{N-1}(x)` by the
/// recurrence `T_k = 2x T_{k-1} - T_{k-2}`.
///
/// `x` is expected on `[-1, 1]` (see [`scale`]).
#[inline]
pub fn basis<const N: usize>(x: f64) -> [f64; N] {
    let mut t = [0.0_f64; N];
    if N == 0 {
        return t;
    }
    t[0] = 1.0;
    if N > 1 {
        t[1] = x;
    }
    for k in 2..N {
        t[k] = 2.0 * x * t[k - 1] - t[k - 2];
    }
    t
}

/// Evaluate `sum_k c[k] T_k(x)` — the **plain** convention.
///
/// `x` is expected on `[-1, 1]`. Returns `0.0` for an empty `c`.
#[inline]
pub fn eval_plain(x: f64, c: &[f64]) -> f64 {
    let Some((&c0, rest)) = c.split_first() else {
        return 0.0;
    };
    let (mut b1, mut b2) = (0.0, 0.0);
    for &ck in rest.iter().rev() {
        let b = 2.0 * x * b1 - b2 + ck;
        b2 = b1;
        b1 = b;
    }
    c0 + x * b1 - b2
}

/// Evaluate `0.5 c[0] + sum_{k>=1} c[k] T_k(x)` — **GSL's** convention, the
/// body of `gsl_cheb_eval` (`cheb/eval.c:29-46`) over a borrowed slice.
///
/// `x` is expected on `[-1, 1]`, i.e. already mapped by [`scale`]. Returns
/// `0.0` for an empty `c`.
#[inline]
pub fn eval_gsl(x: f64, c: &[f64]) -> f64 {
    if c.is_empty() {
        return 0.0;
    }
    let (mut d1, mut d2) = (0.0, 0.0);
    let y2 = 2.0 * x;
    for i in (1..c.len()).rev() {
        let temp = d1;
        d1 = y2 * d1 - d2 + c[i];
        d2 = temp;
    }
    x * d1 - d2 + 0.5 * c[0]
}

/// Dense tensor product `sum_ij c[i][j] T_i(x) T_j(y)`, plain convention.
///
/// Both `x` and `y` are expected on `[-1, 1]`.
#[inline]
pub fn eval2_dense<const M: usize, const N: usize>(x: f64, y: f64, c: &[[f64; N]; M]) -> f64 {
    let tx = basis::<M>(x);
    let ty = basis::<N>(y);
    let mut out = 0.0;
    for i in 0..M {
        for j in 0..N {
            out += c[i][j] * tx[i] * ty[j];
        }
    }
    out
}

/// Sparse tensor product from `(i, j, c_ij)` triples, plain convention.
///
/// Both `x` and `y` are expected on `[-1, 1]`. `MAX` bounds the degree in each
/// direction and must exceed every `i` and `j` present.
///
/// # Panics
/// If any `i` or `j` is `>= MAX`.
#[inline]
pub fn eval2_sparse<const MAX: usize>(x: f64, y: f64, coeffs: &[(usize, usize, f64)]) -> f64 {
    let tx = basis::<MAX>(x);
    let ty = basis::<MAX>(y);
    let mut out = 0.0;
    for &(i, j, c) in coeffs {
        out += c * tx[i] * ty[j];
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    extern crate alloc;
    use crate::ChebSeries;

    /// The slice evaluator must agree with [`ChebSeries::eval`] exactly — it is
    /// the same arithmetic, so "agrees" here means bit-identical.
    #[test]
    fn eval_gsl_matches_the_owning_series_bit_for_bit() {
        let cs = ChebSeries::new(24, -1.0, 2.0, |x| (x * x).exp()).unwrap();
        for k in 0..=100 {
            let v = -1.0 + 3.0 * f64::from(k) / 100.0;
            let owned = cs.eval(v);
            let borrowed = eval_gsl(scale(v, -1.0, 2.0), cs.coefficients());
            assert_eq!(
                owned.to_bits(),
                borrowed.to_bits(),
                "at v = {v}: {owned:e} vs {borrowed:e}"
            );
        }
    }

    /// `eval_plain` and `eval_gsl` differ by exactly `c[0]/2`.
    #[test]
    fn the_two_conventions_differ_by_half_the_constant_term() {
        let c = [3.0, 2.0, 1.0, -0.5];
        for x in [-0.9, -0.1, 0.0, 0.37, 1.0] {
            let d = eval_plain(x, &c) - eval_gsl(x, &c);
            assert!((d - 0.5 * c[0]).abs() < 1e-15, "at x = {x}: {d}");
        }
    }

    /// A dense tensor product of separable factors must factorise.
    #[test]
    fn dense_tensor_product_factorises() {
        // c[i][j] = a_i * b_j  =>  sum_ij = (sum_i a_i T_i)(sum_j b_j T_j)
        let a = [1.0, -2.0, 0.5];
        let b = [2.0, 1.0, -1.0, 0.25];
        let mut c = [[0.0; 4]; 3];
        for i in 0..3 {
            for j in 0..4 {
                c[i][j] = a[i] * b[j];
            }
        }
        for (x, y) in [(-0.8, 0.3), (0.0, 0.0), (0.55, -0.91)] {
            let got = eval2_dense(x, y, &c);
            let want = eval_plain(x, &a) * eval_plain(y, &b);
            assert!((got - want).abs() < 1e-14, "({x},{y}): {got} vs {want}");
        }
    }

    /// Sparse and dense must agree when the sparse triples enumerate the dense
    /// table.
    #[test]
    fn sparse_agrees_with_dense() {
        let c = [[1.0, -0.5, 0.25], [2.0, 0.0, -1.0], [0.0, 3.0, 0.5]];
        let triples: alloc::vec::Vec<(usize, usize, f64)> = (0..3)
            .flat_map(|i| (0..3).map(move |j| (i, j, c[i][j])))
            .collect();
        for (x, y) in [(-0.7, 0.2), (0.1, -0.4), (0.9, 0.9)] {
            let d = eval2_dense(x, y, &c);
            let s = eval2_sparse::<3>(x, y, &triples);
            assert!((d - s).abs() < 1e-14, "({x},{y}): {d} vs {s}");
        }
    }
}
