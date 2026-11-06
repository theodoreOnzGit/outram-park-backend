// ---------------------------------------------------------------------------
// Ported from SciPy and NumPy (BSD-3-Clause) — the numerics RAVEN delegates to.
//
//   SciPy 1.14.1 (https://github.com/scipy/scipy, tag v1.14.1):
//     scipy/special/_orthogonal.py      _gen_roots_and_weights, roots_legendre,
//                                       roots_hermitenorm (n <= 150 branch)
//     scipy/special/orthogonal_eval.pxd eval_legendre_l, eval_hermitenorm
//     scipy/special/special/cephes/gamma.h  Gamma (|x| <= 33 branch)
//     scipy/special/special/cephes/beta.h   beta (the Gamma-ratio branch)
//     scipy/special/special/cephes/polevl.h polevl
//   Copyright (c) 2001-2002 Enthought, Inc. 2003-2024, SciPy Developers.
//
//   NumPy 1.26.4 (https://github.com/numpy/numpy, tag v1.26.4):
//     numpy/core/src/umath/loops_utils.h.src  pairwise_sum (the add reduction)
//     numpy/core/src/multiarray/calculation.c PyArray_Round (decimals > 0)
//   Copyright (c) 2005-2023, NumPy Developers.
//
//   Both BSD-3-Clause; the full texts are this crate's LICENSE-BSD-SCIPY and
//   LICENSE-BSD-NUMPY, and the NOTICE records the attribution. BSD-3-Clause
//   is compatible with (A)GPLv3; this translation is distributed under
//   AGPL-3.0-only as part of RAFFLES / Outram Park.
//
// Translation notes: the vectorised numpy expressions are written as loops in
// numpy's own operation order (so `x -= y/dy` is elementwise, `w.sum()` is
// numpy's pairwise sum). The eigenvalues of the Jacobi matrix come from a
// Sturm-sequence bisection where scipy calls LAPACK (`eigvals_banded` ->
// `dsbevd` -> `dsterf`, not ported); scipy's single Newton step polishes the
// roots, but the weights use the derivative at the UNPOLISHED roots, so they
// carry the eigensolver's last-bit differences (V&V record). `np.log`/`np.exp` are `petir`'s, not numpy's AVX-512 kernels, which
// differ from each other (and from glibc) in the last bit; the V&V record
// gives the measured effect. Only the branches RAVEN's Legendre and Hermite
// rules reach are ported.
// ---------------------------------------------------------------------------

//! The special functions and Gauss rules RAVEN takes from SciPy, ported in
//! SciPy's operation order so the differential test can compare bits.

use petir::mathf::RealMath;

use crate::{RafflesError, Result};

/// `polevl` (cephes): Horner from the leading coefficient.
fn polevl(x: f64, coef: &[f64]) -> f64 {
    let mut ans = coef[0];
    for c in &coef[1..] {
        ans = ans * x + c;
    }
    ans
}

const GAMMA_P: [f64; 7] = [
    1.601_195_224_767_518_614_07E-4,
    1.191_351_470_065_863_849_13E-3,
    1.042_137_975_617_615_699_35E-2,
    4.763_678_004_571_372_314_64E-2,
    2.074_482_276_484_359_751_50E-1,
    4.942_148_268_014_971_007_53E-1,
    9.999_999_999_999_999_967_96E-1,
];

const GAMMA_Q: [f64; 8] = [
    -2.315_818_733_241_201_298_19E-5,
    5.396_055_804_933_033_978_42E-4,
    -4.456_419_138_517_972_404_94E-3,
    1.181_397_852_220_604_355_52E-2,
    3.582_363_986_054_986_533_73E-2,
    -2.345_917_957_182_433_485_68E-1,
    7.143_049_170_302_730_740_85E-2,
    1.000_000_000_000_000_003_20E0,
];

/// cephes `Gamma` for `|x| <= 33` (the range `eval_legendre`'s series asks
/// for); `NaN` outside it rather than an unported Stirling branch.
pub(crate) fn cephes_gamma(mut x: f64) -> f64 {
    if !x.is_finite() || x.abs() > 33.0 {
        return f64::NAN;
    }
    let mut z = 1.0;
    while x >= 3.0 {
        x -= 1.0;
        z *= x;
    }
    while x < 0.0 {
        if x > -1.0e-9 {
            return small(x, z);
        }
        z /= x;
        x += 1.0;
    }
    while x < 2.0 {
        if x < 1.0e-9 {
            return small(x, z);
        }
        z /= x;
        x += 1.0;
    }
    if x == 2.0 {
        return z;
    }
    x -= 2.0;
    let p = polevl(x, &GAMMA_P);
    let q = polevl(x, &GAMMA_Q);
    z * p / q
}

fn small(x: f64, z: f64) -> f64 {
    if x == 0.0 {
        f64::NAN
    } else {
        z / ((1.0 + 0.577_215_664_901_532_9 * x) * x)
    }
}

/// cephes `beta(a, b)`, the Gamma-ratio branch only (both arguments small
/// and neither a non-positive integer), which is all `eval_legendre` uses.
pub(crate) fn cephes_beta(mut a: f64, mut b: f64) -> f64 {
    if a.abs() < b.abs() {
        core::mem::swap(&mut a, &mut b);
    }
    let y = cephes_gamma(a + b);
    let a = cephes_gamma(a);
    let b = cephes_gamma(b);
    if (a.abs() - y.abs()).abs() > (b.abs() - y.abs()).abs() {
        (b / y) * a
    } else {
        (a / y) * b
    }
}

/// `scipy.special.eval_legendre(n, x)` for integer `n` (`eval_legendre_l`).
#[must_use]
pub fn eval_legendre(n: usize, x: f64) -> f64 {
    if n == 0 {
        return 1.0;
    }
    if n == 1 {
        return x;
    }
    if x.abs() < 1e-5 {
        // Power series about 0.
        let a = n / 2;
        let mut d: f64 = if a % 2 == 0 { 1.0 } else { -1.0 };
        if n == 2 * a {
            d *= -2.0 / cephes_beta(a as f64 + 1.0, -0.5);
        } else {
            d *= 2.0 * x / cephes_beta(a as f64 + 1.0, 0.5);
        }
        let mut p = 0.0;
        let (n, a) = (n as i64, a as i64);
        for kk in 0..=a {
            p += d;
            let num = (2 * n + 1 - 2 * a + 2 * kk) as f64;
            let den = ((n + 1 - 2 * a + 2 * kk) * (n + 2 - 2 * a + 2 * kk)) as f64;
            d *= -2.0 * (x * x) * (a - kk) as f64 * num / den;
            // Upstream's (sic) equality test.
            if d.abs() == 1e-20 * p.abs() {
                break;
            }
        }
        return p;
    }
    let mut d = x - 1.0;
    let mut p = x;
    for kk in 0..n - 1 {
        let k = kk as f64 + 1.0;
        d = ((2.0 * k + 1.0) / (k + 1.0)) * (x - 1.0) * p + (k / (k + 1.0)) * d;
        p = d + p;
    }
    p
}

/// `scipy.special.eval_hermitenorm(n, x)`: probabilists' Hermite `He_n`.
#[must_use]
pub fn eval_hermitenorm(n: usize, x: f64) -> f64 {
    if x.is_nan() {
        return x;
    }
    match n {
        0 => 1.0,
        1 => x,
        _ => {
            let mut y3 = 0.0;
            let mut y2 = 1.0;
            for k in (2..=n).rev() {
                let y1 = x * y2 - k as f64 * y3;
                y3 = y2;
                y2 = y1;
            }
            x * y2 - y3
        }
    }
}

/// NumPy's sum of a contiguous `f64` array: the first element plus the
/// pairwise sum of the rest (the add reduction of `loops_utils.h.src`).
#[must_use]
pub fn numpy_sum(a: &[f64]) -> f64 {
    match a.split_first() {
        None => 0.0,
        Some((first, rest)) => first + pairwise_sum(rest),
    }
}

/// NumPy's pairwise summation of a contiguous `f64` run (`pairwise_sum` in
/// `loops_utils.h.src`): eight accumulators up to 128 elements, halving above.
/// Also what an `axis=0` reduction of an `(n, 1)` array uses (measured, gh:#1190).
pub(crate) fn pairwise_sum(a: &[f64]) -> f64 {
    let n = a.len();
    if n < 8 {
        let mut res = 0.0;
        for v in a {
            res += v;
        }
        res
    } else if n <= 128 {
        let mut r = [0.0; 8];
        r.copy_from_slice(&a[..8]);
        let mut i = 8;
        while i < n - (n % 8) {
            for j in 0..8 {
                r[j] += a[i + j];
            }
            i += 8;
        }
        let mut res = ((r[0] + r[1]) + (r[2] + r[3])) + ((r[4] + r[5]) + (r[6] + r[7]));
        while i < n {
            res += a[i];
            i += 1;
        }
        res
    } else {
        let mut n2 = n / 2;
        n2 -= n2 % 8;
        pairwise_sum(&a[..n2]) + pairwise_sum(&a[n2..])
    }
}

/// `np.around(x, decimals)` for `decimals > 0`: multiply, round half to
/// even, divide.
#[must_use]
pub fn numpy_around(x: f64, decimals: i32) -> f64 {
    let f = 10.0_f64.r_powi(decimals);
    (x * f).round_ties_even() / f
}

/// Eigenvalues, ascending, of the symmetric tridiagonal Jacobi matrix with
/// zero diagonal and off-diagonal `b`: Sturm-sequence bisection, each to
/// adjacent floats. Used in place of scipy's LAPACK `eigvals_banded`
/// (`dsbevd` -> `dsterf`), which is not ported; it agrees with LAPACK to
/// ≤ 8e-16 absolute on n ≤ 20 (the V&V record), closer than
/// `petir::linalg::eigen_symm` (≤ 2.1e-15), whose Householder pass the
/// already-tridiagonal matrix does not need.
fn jacobi_eigenvalues(b: &[f64]) -> Vec<f64> {
    let n = b.len() + 1;
    let radius = (0..n)
        .map(|i| {
            let left = if i > 0 { b[i - 1].abs() } else { 0.0 };
            let right = if i + 1 < n { b[i].abs() } else { 0.0 };
            left + right
        })
        .fold(0.0, f64::max);
    // Number of eigenvalues below `l`.
    let count = |l: f64| -> usize {
        let mut c = 0;
        let mut q = -l;
        if q < 0.0 {
            c += 1;
        }
        for bi in b {
            let prev = if q == 0.0 { f64::MIN_POSITIVE } else { q };
            q = -l - bi * bi / prev;
            if q < 0.0 {
                c += 1;
            }
        }
        c
    };
    (0..n)
        .map(|k| {
            let (mut lo, mut hi) = (-radius - 1.0, radius + 1.0);
            loop {
                let mid = 0.5 * (lo + hi);
                if mid <= lo || mid >= hi {
                    break;
                }
                if count(mid) > k {
                    hi = mid;
                } else {
                    lo = mid;
                }
            }
            0.5 * (lo + hi)
        })
        .collect()
}

/// Which Gauss rule.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GaussRule {
    /// `roots_legendre`: weight 1 on [−1, 1], weights sum to 2.
    Legendre,
    /// `roots_hermitenorm`: weight `exp(−x²/2)`, weights sum to `√(2π)`.
    HermiteNorm,
}

/// `scipy.special.roots_legendre(n)` / `roots_hermitenorm(n)`, `1 <= n <=
/// 150`: Golub–Welsch eigenvalues, one Newton step, scipy's log-normalised
/// weights, symmetrisation, then the weights scaled to `mu0`.
///
/// # Errors
///
/// [`RafflesError::InvalidParameter`] for `n` outside `1..=150`.
pub fn gauss_rule(rule: GaussRule, n: usize) -> Result<(Vec<f64>, Vec<f64>)> {
    if n == 0 || n > 150 {
        return Err(RafflesError::InvalidParameter {
            parameter: "n".to_string(),
            value: n as f64,
            reason: "Gauss rules are ported for 1 <= n <= 150 (scipy's recurrence branch)"
                .to_string(),
        });
    }
    let (mu0, bn): (f64, fn(f64) -> f64) = match rule {
        GaussRule::Legendre => (2.0, |k| k * (1.0 / (4.0 * k * k - 1.0)).sqrt()),
        GaussRule::HermiteNorm => ((2.0 * core::f64::consts::PI).sqrt(), |k| k.sqrt()),
    };
    let f = |m: usize, x: f64| match rule {
        GaussRule::Legendre => eval_legendre(m, x),
        GaussRule::HermiteNorm => eval_hermitenorm(m, x),
    };
    let df = |m: usize, x: f64| match rule {
        GaussRule::Legendre => {
            let mf = m as f64;
            (-mf * x * eval_legendre(m, x) + mf * eval_legendre(m - 1, x)) / (1.0 - x * x)
        }
        GaussRule::HermiteNorm => m as f64 * eval_hermitenorm(m - 1, x),
    };
    let b: Vec<f64> = (1..n).map(|k| bn(k as f64)).collect();
    let mut x = jacobi_eigenvalues(&b);
    let dy: Vec<f64> = x.iter().map(|&xi| df(n, xi)).collect();
    for (xi, dyi) in x.iter_mut().zip(&dy) {
        let y = f(n, *xi);
        *xi -= y / dyi;
    }
    let mut fm: Vec<f64> = x.iter().map(|&xi| f(n - 1, xi)).collect();
    let mut dy = dy;
    let norm = |v: &[f64]| {
        let logs: Vec<f64> = v.iter().map(|e| e.abs().r_ln()).collect();
        let mx = logs.iter().copied().fold(f64::NEG_INFINITY, f64::max);
        let mn = logs.iter().copied().fold(f64::INFINITY, f64::min);
        ((mx + mn) / 2.0).r_exp()
    };
    let (sf, sd) = (norm(&fm), norm(&dy));
    fm.iter_mut().for_each(|v| *v /= sf);
    dy.iter_mut().for_each(|v| *v /= sd);
    let w0: Vec<f64> = fm.iter().zip(&dy).map(|(a, b)| 1.0 / (a * b)).collect();
    let mut w: Vec<f64> = (0..n).map(|i| (w0[i] + w0[n - 1 - i]) / 2.0).collect();
    let xs: Vec<f64> = (0..n).map(|i| (x[i] - x[n - 1 - i]) / 2.0).collect();
    let scale = mu0 / numpy_sum(&w);
    w.iter_mut().for_each(|v| *v *= scale);
    Ok((xs, w))
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Closed forms: Γ at integers and half-integers, B(a, b), P_n(0), He_n,
    /// numpy's sum on lengths around its block edges, `np.around`, and the
    /// Gauss rules' exactness (an n-point rule integrates degree 2n − 1).
    #[test]
    fn special_functions_and_rules_against_closed_forms() {
        assert_eq!(cephes_gamma(5.0), 24.0);
        assert!((cephes_gamma(0.5) - core::f64::consts::PI.sqrt()).abs() < 1e-15);
        assert!((cephes_gamma(-0.5) + 2.0 * core::f64::consts::PI.sqrt()).abs() < 1e-14);
        assert!(cephes_gamma(0.0).is_nan() && cephes_gamma(40.0).is_nan());
        assert!((cephes_gamma(1e-10) - 1e10).abs() / 1e10 < 1e-9);
        assert!((cephes_beta(2.0, 3.0) - 1.0 / 12.0).abs() < 1e-16);
        // P_2(0) = −1/2, P_4(0) = 3/8, P_3 odd.
        assert!((eval_legendre(2, 0.0) + 0.5).abs() < 1e-15);
        assert!((eval_legendre(4, 0.0) - 0.375).abs() < 1e-15);
        assert_eq!(eval_legendre(3, 0.0), 0.0);
        assert!((eval_legendre(3, 0.5) - (2.5 * 0.125 - 0.75)).abs() < 1e-15);
        assert_eq!(eval_hermitenorm(3, 2.0), 8.0 - 6.0);
        assert!(eval_hermitenorm(4, f64::NAN).is_nan());
        for n in [0usize, 1, 7, 8, 9, 16, 129, 130, 300] {
            let a: Vec<f64> = (0..n).map(|i| 0.1 * i as f64).collect();
            let exact = 0.1 * (n * n.saturating_sub(1) / 2) as f64;
            assert!((numpy_sum(&a) - exact).abs() < 1e-9, "n = {n}");
        }
        assert_eq!(numpy_around(0.123_456_789_012_345_67, 15), 0.123_456_789_012_346);
        assert!(gauss_rule(GaussRule::Legendre, 0).is_err());
        assert!(gauss_rule(GaussRule::Legendre, 151).is_err());
        for n in 1..=12 {
            let (x, w) = gauss_rule(GaussRule::Legendre, n).unwrap();
            for deg in 0..2 * n {
                let q: f64 = x.iter().zip(&w).map(|(xi, wi)| wi * xi.powi(deg as i32)).sum();
                let exact = if deg % 2 == 1 { 0.0 } else { 2.0 / (deg as f64 + 1.0) };
                assert!((q - exact).abs() < 1e-13, "Legendre n {n} deg {deg}");
            }
            let (x, w) = gauss_rule(GaussRule::HermiteNorm, n).unwrap();
            let mut moment = (2.0 * core::f64::consts::PI).sqrt();
            for deg in (0..2 * n).step_by(2) {
                let q: f64 = x.iter().zip(&w).map(|(xi, wi)| wi * xi.powi(deg as i32)).sum();
                assert!((q - moment).abs() < 1e-10 * moment, "Hermite n {n} deg {deg}");
                moment *= deg as f64 + 1.0; // E[x^(k+2)] = (k+1) E[x^k]
            }
        }
    }
}
