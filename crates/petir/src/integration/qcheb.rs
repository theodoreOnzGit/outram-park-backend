//! `gsl_integration_qcheb`: the 12th- and 24th-order Chebyshev
//! approximations of `f` on `[a, b]` (GSL 2.8 `integration/qcheb.c`, lines
//! 28-229, commit `cf180cd7`), used by QAWC and QAWS. Copyright (C) 1996-2000,
//! 2007 Brian Gough (GPL-3.0-or-later). gh:#1340.
//!
//! The arithmetic is upstream's, statement for statement; only `x[k]` of the
//! `cos(pi k / 24)` table is a `const` array with literal indices (allowed by
//! the subscript gate) and every other array goes through [`g`] / [`s`].

/// `cos(pi k / 24)` for `k = 1..=11`, qcheb.c lines 36-46.
const X: [f64; 11] = [
    0.9914448613738104,
    0.9659258262890683,
    0.9238795325112868,
    0.8660254037844386,
    0.7933533402912352,
    0.7071067811865475,
    0.6087614290087206,
    0.5000000000000000,
    0.3826834323650898,
    0.2588190451025208,
    0.1305261922200516,
];

/// Checked read; every index below is a constant in range.
fn g(v: &[f64], i: usize) -> f64 {
    v.get(i).copied().unwrap_or(0.0)
}

/// Checked write.
fn s(v: &mut [f64], i: usize, x: f64) {
    if let Some(p) = v.get_mut(i) {
        *p = x;
    }
}

/// `(cheb12, cheb24)` of `f` on `[a, b]`.
#[allow(clippy::too_many_lines)]
pub(super) fn qcheb<F>(f: &F, a: f64, b: f64) -> ([f64; 13], [f64; 25])
where
    F: Fn(f64) -> f64,
{
    let mut cheb12 = [0.0_f64; 13];
    let mut cheb24 = [0.0_f64; 25];
    let mut fval = [0.0_f64; 25];
    let mut v = [0.0_f64; 12];

    let center = 0.5 * (b + a);
    let half_length = 0.5 * (b - a);

    s(&mut fval, 0, 0.5 * f(b));
    s(&mut fval, 12, f(center));
    s(&mut fval, 24, 0.5 * f(a));

    for i in 1..12 {
        let j = 24 - i;
        let u = half_length * g(&X, i - 1);
        s(&mut fval, i, f(center + u));
        s(&mut fval, j, f(center - u));
    }

    for i in 0..12 {
        let j = 24 - i;
        s(&mut v, i, g(&fval, i) - g(&fval, j));
        let t = g(&fval, i) + g(&fval, j);
        s(&mut fval, i, t);
    }

    {
        let alam1 = g(&v, 0) - g(&v, 8);
        let alam2 = X[5] * (g(&v, 2) - g(&v, 6) - g(&v, 10));
        s(&mut cheb12, 3, alam1 + alam2);
        s(&mut cheb12, 9, alam1 - alam2);
    }

    {
        let alam1 = g(&v, 1) - g(&v, 7) - g(&v, 9);
        let alam2 = g(&v, 3) - g(&v, 5) - g(&v, 11);
        {
            let alam = X[2] * alam1 + X[8] * alam2;
            s(&mut cheb24, 3, g(&cheb12, 3) + alam);
            s(&mut cheb24, 21, g(&cheb12, 3) - alam);
        }
        {
            let alam = X[8] * alam1 - X[2] * alam2;
            s(&mut cheb24, 9, g(&cheb12, 9) + alam);
            s(&mut cheb24, 15, g(&cheb12, 9) - alam);
        }
    }

    {
        let part1 = X[3] * g(&v, 4);
        let part2 = X[7] * g(&v, 8);
        let part3 = X[5] * g(&v, 6);
        {
            let alam1 = g(&v, 0) + part1 + part2;
            let alam2 = X[1] * g(&v, 2) + part3 + X[9] * g(&v, 10);
            s(&mut cheb12, 1, alam1 + alam2);
            s(&mut cheb12, 11, alam1 - alam2);
        }
        {
            let alam1 = g(&v, 0) - part1 + part2;
            let alam2 = X[9] * g(&v, 2) - part3 + X[1] * g(&v, 10);
            s(&mut cheb12, 5, alam1 + alam2);
            s(&mut cheb12, 7, alam1 - alam2);
        }
    }

    {
        let alam = X[0] * g(&v, 1)
            + X[2] * g(&v, 3)
            + X[4] * g(&v, 5)
            + X[6] * g(&v, 7)
            + X[8] * g(&v, 9)
            + X[10] * g(&v, 11);
        s(&mut cheb24, 1, g(&cheb12, 1) + alam);
        s(&mut cheb24, 23, g(&cheb12, 1) - alam);
    }

    {
        let alam = X[10] * g(&v, 1) - X[8] * g(&v, 3) + X[6] * g(&v, 5) - X[4] * g(&v, 7)
            + X[2] * g(&v, 9)
            - X[0] * g(&v, 11);
        s(&mut cheb24, 11, g(&cheb12, 11) + alam);
        s(&mut cheb24, 13, g(&cheb12, 11) - alam);
    }

    {
        let alam = X[4] * g(&v, 1) - X[8] * g(&v, 3) - X[0] * g(&v, 5) - X[10] * g(&v, 7)
            + X[2] * g(&v, 9)
            + X[6] * g(&v, 11);
        s(&mut cheb24, 5, g(&cheb12, 5) + alam);
        s(&mut cheb24, 19, g(&cheb12, 5) - alam);
    }

    {
        let alam = X[6] * g(&v, 1) - X[2] * g(&v, 3) - X[10] * g(&v, 5) + X[0] * g(&v, 7)
            - X[8] * g(&v, 9)
            - X[4] * g(&v, 11);
        s(&mut cheb24, 7, g(&cheb12, 7) + alam);
        s(&mut cheb24, 17, g(&cheb12, 7) - alam);
    }

    for i in 0..6 {
        let j = 12 - i;
        s(&mut v, i, g(&fval, i) - g(&fval, j));
        let t = g(&fval, i) + g(&fval, j);
        s(&mut fval, i, t);
    }

    {
        let alam1 = g(&v, 0) + X[7] * g(&v, 4);
        let alam2 = X[3] * g(&v, 2);
        s(&mut cheb12, 2, alam1 + alam2);
        s(&mut cheb12, 10, alam1 - alam2);
    }

    s(&mut cheb12, 6, g(&v, 0) - g(&v, 4));

    {
        let alam = X[1] * g(&v, 1) + X[5] * g(&v, 3) + X[9] * g(&v, 5);
        s(&mut cheb24, 2, g(&cheb12, 2) + alam);
        s(&mut cheb24, 22, g(&cheb12, 2) - alam);
    }

    {
        let alam = X[5] * (g(&v, 1) - g(&v, 3) - g(&v, 5));
        s(&mut cheb24, 6, g(&cheb12, 6) + alam);
        s(&mut cheb24, 18, g(&cheb12, 6) - alam);
    }

    {
        let alam = X[9] * g(&v, 1) - X[5] * g(&v, 3) + X[1] * g(&v, 5);
        s(&mut cheb24, 10, g(&cheb12, 10) + alam);
        s(&mut cheb24, 14, g(&cheb12, 10) - alam);
    }

    for i in 0..3 {
        let j = 6 - i;
        s(&mut v, i, g(&fval, i) - g(&fval, j));
        let t = g(&fval, i) + g(&fval, j);
        s(&mut fval, i, t);
    }

    s(&mut cheb12, 4, g(&v, 0) + X[7] * g(&v, 2));
    s(&mut cheb12, 8, g(&fval, 0) - X[7] * g(&fval, 2));

    {
        let alam = X[3] * g(&v, 1);
        s(&mut cheb24, 4, g(&cheb12, 4) + alam);
        s(&mut cheb24, 20, g(&cheb12, 4) - alam);
    }

    {
        let alam = X[7] * g(&fval, 1) - g(&fval, 3);
        s(&mut cheb24, 8, g(&cheb12, 8) + alam);
        s(&mut cheb24, 16, g(&cheb12, 8) - alam);
    }

    s(&mut cheb12, 0, g(&fval, 0) + g(&fval, 2));

    {
        let alam = g(&fval, 1) + g(&fval, 3);
        s(&mut cheb24, 0, g(&cheb12, 0) + alam);
        s(&mut cheb24, 24, g(&cheb12, 0) - alam);
    }

    s(&mut cheb12, 12, g(&v, 0) - g(&v, 2));
    s(&mut cheb24, 12, g(&cheb12, 12));

    for i in 1..12 {
        let t = g(&cheb12, i) * (1.0 / 6.0);
        s(&mut cheb12, i, t);
    }

    let t = g(&cheb12, 0) * (1.0 / 12.0);

    s(&mut cheb12, 0, t);
    let t = g(&cheb12, 12) * (1.0 / 12.0);
    s(&mut cheb12, 12, t);

    for i in 1..24 {
        let t = g(&cheb24, i) * (1.0 / 12.0);
        s(&mut cheb24, i, t);
    }

    let t = g(&cheb24, 0) * (1.0 / 24.0);

    s(&mut cheb24, 0, t);
    let t = g(&cheb24, 24) * (1.0 / 24.0);
    s(&mut cheb24, 24, t);

    (cheb12, cheb24)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn expansion_of_x_squared_has_known_coefficients() {
        // x^2 = (T0 + T2)/2, so on [-1, 1] c0 (halved by qcheb's convention)
        // is 1/2 .. only check that T2's coefficient is 1/2 and T1, T3 vanish.
        let (c12, c24) = qcheb(&|x: f64| x * x, -1.0, 1.0);
        assert!((g(&c12, 2) - 0.5).abs() < 1e-14);
        assert!(g(&c12, 1).abs() < 1e-14 && g(&c12, 3).abs() < 1e-14);
        assert!((g(&c24, 2) - 0.5).abs() < 1e-14);
    }
}
