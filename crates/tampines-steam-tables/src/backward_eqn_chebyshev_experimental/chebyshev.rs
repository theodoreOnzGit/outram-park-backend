//! Chebyshev basis evaluation shared by the experimental backward
//! correlations in this module tree.
//!
//! Every correlation here is a tensor-product Chebyshev polynomial in two
//! scaled variables. The independent variables are mapped onto `[-1, 1]` by
//! [`scale`] before evaluation; outside that interval the Chebyshev recurrence
//! grows without bound, so callers must respect each correlation's stated fit
//! domain.

/// Maps a raw value onto the Chebyshev interval `[-1, 1]`.
///
/// `lo` and `hi` are the fit domain's endpoints in the same units as `v`.
/// A value at `lo` maps to `-1.0`, a value at `hi` maps to `+1.0`. Values
/// outside `[lo, hi]` map outside `[-1, 1]`, where the polynomial is an
/// extrapolation and diverges quickly — this function does not clamp.
#[inline]
pub(crate) fn scale(v: f64, lo: f64, hi: f64) -> f64 {
    2.0 * (v - lo) / (hi - lo) - 1.0
}

/// Chebyshev polynomials of the first kind `T_0(x) .. T_{N-1}(x)`.
///
/// `x` is expected on `[-1, 1]` (see [`scale`]). Built by the standard
/// recurrence `T_k = 2 x T_{k-1} - T_{k-2}`.
#[inline]
pub(crate) fn cheb_basis<const N: usize>(x: f64) -> [f64; N] {
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

/// Evaluates a one-dimensional Chebyshev series by the Clenshaw recurrence.
///
/// `x` is expected on `[-1, 1]`; `c[k]` is the coefficient of `T_k(x)`.
pub(crate) fn cheb1(x: f64, c: &[f64]) -> f64 {
    let mut b1 = 0.0;
    let mut b2 = 0.0;
    for &ck in c[1..].iter().rev() {
        let b = 2.0 * x * b1 - b2 + ck;
        b2 = b1;
        b1 = b;
    }
    c[0] + x * b1 - b2
}

/// Evaluates a dense tensor-product Chebyshev series `sum_ij c[i][j] T_i(x) T_j(y)`.
///
/// Both `x` and `y` are expected on `[-1, 1]` (see [`scale`]).
pub(crate) fn cheb2_dense<const M: usize, const N: usize>(
    x: f64,
    y: f64,
    c: &[[f64; N]; M],
) -> f64 {
    let tx = cheb_basis::<M>(x);
    let ty = cheb_basis::<N>(y);
    let mut out = 0.0;
    for i in 0..M {
        for j in 0..N {
            out += c[i][j] * tx[i] * ty[j];
        }
    }
    out
}

/// Evaluates a sparse tensor-product Chebyshev series stored as
/// `(i, j, c_ij)` triples: `sum c_ij T_i(x) T_j(y)`.
///
/// Both `x` and `y` are expected on `[-1, 1]` (see [`scale`]). Degrees are
/// capped at 8 in each direction, which covers every fit stored here.
pub(crate) fn cheb2_sparse(x: f64, y: f64, coeffs: &[(usize, usize, f64)]) -> f64 {
    let tx = cheb_basis::<9>(x);
    let ty = cheb_basis::<9>(y);
    let mut out = 0.0;
    for &(i, j, c) in coeffs {
        out += c * tx[i] * ty[j];
    }
    out
}
