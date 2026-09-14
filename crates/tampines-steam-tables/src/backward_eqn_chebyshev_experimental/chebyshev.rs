//! Chebyshev basis evaluation shared by the experimental backward
//! correlations in this module tree.
//!
//! Every correlation here is a tensor-product Chebyshev polynomial in two
//! scaled variables. The independent variables are mapped onto `[-1, 1]` by
//! [`scale`] before evaluation; outside that interval the Chebyshev recurrence
//! grows without bound, so callers must respect each correlation's stated fit
//! domain.
//!
//! # These are now PETIR's implementations
//!
//! The bodies that used to live here were lifted into `petir`
//! (`petir::cheb_slice`) per `bn:op-chyp.2`, which named this module as
//! "existing evaluation code to lift rather than rewrite". This file is now a
//! thin re-export so the call sites and their coefficient tables are
//! untouched.
//!
//! Two things are worth knowing about the move:
//!
//! - **Convention.** These tables are in the *plain* convention, where `c[0]`
//!   is the coefficient of `T_0`. GSL — and therefore `petir::ChebSeries` —
//!   doubles `c[0]` and halves it again on evaluation. [`cheb1`] maps to
//!   [`petir::eval_plain`], which keeps this crate's convention; routing these
//!   tables through the GSL-convention evaluator instead would shift every
//!   result by `c[0]/2`, which on `HF_COEFFS` is about 904 kJ/kg, silently.
//!   `petir`'s `tests/tampines_interop.rs` pins that trap.
//! - **Scaling.** [`scale`] now uses GSL's `(2v - lo - hi)/(hi - lo)` rather
//!   than the algebraically identical `2(v - lo)/(hi - lo) - 1` this file used
//!   before. They differ in the last ulp. The correlations' own regression
//!   tests cover the change.
//!
//! Nothing here allocates: `petir::cheb_slice` works on borrowed slices
//! precisely so these hot-path lookups do not have to.

pub(crate) use petir::{basis as cheb_basis, eval_plain as cheb1, scale};

/// Evaluates a dense tensor-product Chebyshev series `sum_ij c[i][j] T_i(x) T_j(y)`.
///
/// Both `x` and `y` are expected on `[-1, 1]` (see [`scale`]).
#[inline]
pub(crate) fn cheb2_dense<const M: usize, const N: usize>(
    x: f64,
    y: f64,
    c: &[[f64; N]; M],
) -> f64 {
    petir::eval2_dense(x, y, c)
}

/// Evaluates a sparse tensor-product Chebyshev series stored as
/// `(i, j, c_ij)` triples: `sum c_ij T_i(x) T_j(y)`.
///
/// Both `x` and `y` are expected on `[-1, 1]` (see [`scale`]). Degrees are
/// capped at 8 in each direction, which covers every fit stored here.
#[inline]
pub(crate) fn cheb2_sparse(x: f64, y: f64, coeffs: &[(usize, usize, f64)]) -> f64 {
    petir::eval2_sparse::<9>(x, y, coeffs)
}
