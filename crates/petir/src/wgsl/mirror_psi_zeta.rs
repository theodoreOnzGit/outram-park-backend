//! `f32` mirrors of `shaders/psi_zeta.wgsl` — GSL's digamma and zeta
//! families.
//!
//! Generated from the same parse of [`crate::specfunc::psi`] and
//! [`crate::specfunc::zeta`] that produced the shader, so the two cannot
//! drift in their 136 coefficients. See [`crate::wgsl::mirror`] for why a
//! mirror exists at all.
//!
//! # Three entry points the shader deliberately lacks
//!
//! The integer-argument lookups (`psi_int`, `zeta_int`, `eta_int` and
//! friends) are 101-entry tables that WGSL would have to rebuild on the stack
//! per invocation; `zeta(s)` for `s <= -34` needs a `Gamma` that overflows
//! `f32`; and `psi_n` for `n >= 2` needs an `n!` that does the same. The
//! shader header says so in full, and the `f64` modules carry all three.
//!
//! # Where the error is — and a `pow` story the measurement refuted
//!
//! The expectation written here first was that `pow` would dominate: `hzeta`
//! calls it eleven times per evaluation with exponent `-s`, WGSL defines
//! `pow(x, y)` as `exp2(y * log2(x))`, and at `s = 20` that is a `log2`/`exp2`
//! round trip through a value of order `10^{-20}`.
//!
//! **It does not, and the sign is backwards.** `hzeta(s, 1)` against the `f64`
//! original:
//!
//! | `s` | 1.5 | 5 | 10 | 20 | 30 |
//! |---|---|---|---|---|---|
//! | relative | 3.418e-08 | 5.314e-08 | 1.201e-08 | 2.877e-10 | 9.313e-10 |
//!
//! It gets *better* as `s` grows, which is where `pow` works hardest. Its
//! worst point over the whole sweep is 2.763e-07 at `(s, q) = (5.5, 0.25)` —
//! one `f32` ulp, the same as everything else on the positive axis.
//!
//! # What actually costs digits: the reflection branch
//!
//! | function | worst | at | window |
//! |---|---|---|---|
//! | `psi_1piy` | 7.540e-08 | 195 | `(0, 2000]` |
//! | `psi` | 1.152e-07 | 10.9 | `(0, 30]` |
//! | `zetam1` | 2.090e-07 | 15.6 | `(5, 40]` |
//! | `psi_1` | 2.559e-07 | 23.2 | `(0, 30]` |
//! | `hzeta` | 2.763e-07 | (5.5, 0.25) | `s` in `(1, 31]` |
//! | `eta` | **6.775e-06** | -15.7 | `[-20, 40]` |
//! | `zeta` | **9.391e-06** | -29.475 | `[-30, 40]` |
//!
//! `zeta` and `eta` are two orders worse than the rest, and only below zero:
//! that is the functional equation, which multiplies `Gamma(1 - s)` by
//! `(2 pi)^s` and a sine. [`crate::wgsl::mirror_gamma`] measures its own
//! distance from `f64` at 7.839e-06 in the Lanczos branch, so `zeta`'s
//! 9.391e-06 is very nearly just `Gamma`'s error passed through. Nothing
//! here would improve it short of a better `f32` gamma.

// Under a std-linked build (`cargo test`) f32's inherent exp/ln/... shadow
// these trait methods, leaving the import formally unused. See crate::real.
#[allow(unused_imports)]
use crate::real::Real;

use core::f32::consts::PI;

#[rustfmt::skip]
const PSI_CS: [f32; 23] = [
    -0.0380570808, 0.491415393, -0.0568157478, 0.00835782123, -0.00133323286,
    0.000220313287, -3.70402382e-05, 6.28379365e-06, -1.07126391e-06, 1.83128395e-07,
    -3.13535094e-08, 5.37280878e-09, -9.21168141e-10, 1.57981265e-10, -2.7098646e-11,
    4.648722e-12, -7.97527e-13, 1.36827e-13, -2.3475e-14, 4.027e-15, -6.91e-16, 1.18e-16,
    -2e-17
];

#[rustfmt::skip]
const APSI_CS: [f32; 16] = [
    -0.0204749045, -0.0101801272, 5.59718725e-05, -1.29171766e-06, 5.72858606e-08,
    -3.8213539e-09, 3.397434e-10, -3.74838e-11, 4.899e-12, -7.344e-13, 1.233e-13,
    -2.28e-14, 4.5e-15, -9e-16, 2e-16, -0.0
];

#[rustfmt::skip]
const R1PY: [f32; 30] = [
    1.59888328, 0.679056254, -0.068485803, -0.00578818418, 0.00851125817, -0.00404265613,
    0.00135232841, -0.000311646564, 1.85075638e-05, 2.83487054e-05, -1.9487536e-05,
    8.07097887e-06, -2.29835643e-06, 3.05066296e-07, 1.30422386e-07, -1.23086572e-07,
    5.77108557e-08, -1.82755593e-08, 3.10204713e-09, 6.89893275e-10, -8.71822903e-10,
    4.40691477e-10, -1.47273111e-10, 2.75896825e-11, 4.18718268e-12, -6.56734605e-12,
    3.44879009e-12, -1.18072514e-12, 2.37983143e-13, 2.16636304e-15
];

#[rustfmt::skip]
const ZETA_XLT1: [f32; 14] = [
    1.48018677, 0.250120625, 0.00991137502, -0.000120847597, -4.75858664e-06,
    2.22299467e-07, -2.22374965e-09, -1.01732265e-10, 4.37566435e-12, -6.22296326e-14,
    -6.6116201e-16, 4.94772795e-17, -1.04298191e-18, 6.99252162e-21
];

#[rustfmt::skip]
const ZETA_XGT1: [f32; 30] = [
    19.3918516, 9.15253297, 0.242789766, -0.133900069, 0.0577827064, -0.0187625984,
    0.00394030143, -5.81508273e-05, -0.000375614891, 0.000189253055, -5.490322e-05,
    8.7086484e-06, 6.46094779e-07, -9.67497739e-07, 3.65854008e-07, -8.45925164e-08,
    9.99567861e-09, 1.42600364e-09, -1.17619688e-09, 3.71145759e-10, -7.47568552e-11,
    7.85369342e-12, 9.98271823e-13, -7.5276687e-13, 2.19550264e-13, -4.19348599e-14,
    4.63411496e-15, 2.37424885e-16, -2.72765164e-16, 7.84735701e-17
];

#[rustfmt::skip]
const ZETAM1_INTER: [f32; 23] = [
    -21.7509436, -5.63036878, 0.0528041359, -0.0156381809, 0.00408218474, -0.00102648673,
    0.00026046988, -6.76175847e-05, 1.79284473e-05, -4.83238651e-06, 1.31913789e-06,
    -3.63760501e-07, 1.01146848e-07, -2.83215225e-08, 7.9773371e-09, -2.25850169e-09,
    6.42269393e-10, -1.83363862e-10, 5.25309764e-11, -1.50958687e-11, 4.34997546e-12,
    -1.25597783e-12, 3.6128074e-13
];

#[rustfmt::skip]
const HZETA_C: [f32; 15] = [
    1.0, 0.0833333333, -0.00138888889, 3.30687831e-05, -8.26719577e-07, 2.0876757e-08,
    -5.28419014e-10, 1.33825365e-11, -3.3896803e-13, 8.58606206e-15, -2.1748687e-16,
    5.50900283e-18, -1.39544647e-19, 3.53470704e-21, -8.95351743e-23
];

/// `(2 pi)^{10 n}` for `n = 0 ..= 3` — GSL's `twopi_pow` truncated to what
/// `f32` holds. Thirteen of its eighteen entries are past `f32::MAX`.
const TWOPI_POW: [f32; 4] = [1.0, 9.589_560_1e7, 9.195_966e15, 8.818_527e23];

const LN2: f32 = 0.693_147_2;
const EULER: f32 = 0.577_215_7;
const ROOT5_EPS: f32 = 7.400_96e-4;
/// `f32`'s smallest normal. Stands in for GSL's `2 * GSL_DBL_MIN` guard on
/// `|sin(pi x)|`, which flushes to zero in `f32` and could never fire.
const SQRT_MIN: f32 = 1.1754944e-38;

/// Clenshaw in GSL's convention. Mirrors every `petir_cheb_*` in the shader.
fn cheb(x: f32, c: &[f32]) -> f32 {
    let Some((&c0, rest)) = c.split_first() else {
        return 0.0;
    };
    let (mut d, mut dd) = (0.0_f32, 0.0_f32);
    let y2 = 2.0 * x;
    for &ci in rest.iter().rev() {
        let temp = d;
        d = y2 * d - dd + ci;
        dd = temp;
    }
    x * d - dd + 0.5 * c0
}

/// `psi(x)` in `f32`. Mirrors `petir_psi`.
pub fn psi(x: f32) -> f32 {
    let y = x.abs();
    if x == 0.0 || x == -1.0 || x == -2.0 {
        return f32::NAN;
    }
    if y >= 2.0 {
        let c = cheb(8.0 / (y * y) - 1.0, &APSI_CS);
        if x < 0.0 {
            let s = (PI * x).sin();
            let co = (PI * x).cos();
            if s.abs() < 2.0 * SQRT_MIN {
                return f32::NAN;
            }
            return y.ln() - 0.5 / x + c - PI * co / s;
        }
        return y.ln() - 0.5 / x + c;
    }
    if x < -1.0 {
        let v = x + 2.0;
        return -(1.0 / x + 1.0 / (x + 1.0) + 1.0 / v) + cheb(2.0 * v - 1.0, &PSI_CS);
    }
    if x < 0.0 {
        let v = x + 1.0;
        return -(1.0 / x + 1.0 / v) + cheb(2.0 * v - 1.0, &PSI_CS);
    }
    if x < 1.0 {
        return -1.0 / x + cheb(2.0 * x - 1.0, &PSI_CS);
    }
    cheb(2.0 * (x - 1.0) - 1.0, &PSI_CS)
}

/// `Re psi(1 + iy)` in `f32`. Mirrors `petir_psi_1piy`.
pub fn psi_1piy(y: f32) -> f32 {
    let ay = y.abs();
    if ay > 1000.0 {
        let yi2 = 1.0 / (ay * ay);
        return ay.ln() + yi2 * (1.0 / 12.0 + 1.0 / 120.0 * yi2 + 1.0 / 252.0 * yi2 * yi2);
    }
    if ay > 10.0 {
        let yi2 = 1.0 / (ay * ay);
        let sum = yi2
            * (1.0 / 12.0
                + yi2
                    * (1.0 / 120.0
                        + yi2
                            * (1.0 / 252.0
                                + yi2
                                    * (1.0 / 240.0
                                        + yi2 * (1.0 / 132.0 + 691.0 / 32760.0 * yi2)))));
        return ay.ln() + sum;
    }
    if ay > 1.0 {
        let y2 = ay * ay;
        let v = y2 * (1.0 / (1.0 + y2) + 0.5 / (4.0 + y2));
        return cheb((2.0 * ay - 11.0) / 9.0, &R1PY) - EULER + v;
    }
    let y2 = y * y;
    let p = 1.960_399_9e-4 + y2 * (-3.842_666e-8 + y2 * (1.004_159_3e-11 - y2 * 2.951_674_4e-15));
    let mut sum = 0.0_f32;
    for n in 1..=50u32 {
        let nf = n as f32;
        sum += 1.0 / (nf * (nf * nf + y2));
    }
    -EULER + y2 * (sum + p)
}

/// `zeta(s, q)` in `f32`, `s > 1` and `q > 0`. Mirrors `petir_hzeta`.
pub fn hzeta(s: f32, q: f32) -> f32 {
    if !(s > 1.0) || !(q > 0.0) {
        return f32::NAN;
    }
    let max_bits = 54.0;
    if (s > max_bits && q < 1.0) || (s > 0.5 * max_bits && q < 0.25) {
        return q.powf(-s);
    }
    if s > 0.5 * max_bits && q < 1.0 {
        return q.powf(-s) * (1.0 + (q / (1.0 + q)).powf(s) + (q / (2.0 + q)).powf(s));
    }
    let kmax = 10.0_f32;
    let pmax = (kmax + q).powf(-s);
    let mut scp = s;
    let mut pcp = pmax / (kmax + q);
    let mut ans = pmax * ((kmax + q) / (s - 1.0) + 0.5);
    for k in 0..10u32 {
        ans += (k as f32 + q).powf(-s);
    }
    for (j, &cj) in (0..=12u32).zip(HZETA_C.iter().skip(1)) {
        let delta = cj * scp * pcp;
        ans += delta;
        if (delta / ans).abs() < 0.5 * f32::EPSILON {
            break;
        }
        let jf = j as f32;
        scp *= (s + 2.0 * jf + 1.0) * (s + 2.0 * jf + 2.0);
        pcp /= (kmax + q) * (kmax + q);
    }
    ans
}

/// `zeta(s)` for `s >= 0`. Mirrors `petir_zeta_sgt0`.
fn zeta_sgt0(s: f32) -> f32 {
    if s < 1.0 {
        return cheb(2.0 * s - 1.0, &ZETA_XLT1) / (s - 1.0);
    }
    if s <= 20.0 {
        return cheb((2.0 * s - 21.0) / 19.0, &ZETA_XGT1) / (s - 1.0);
    }
    let f2 = 1.0 - 2.0_f32.powf(-s);
    let f3 = 1.0 - 3.0_f32.powf(-s);
    let f5 = 1.0 - 5.0_f32.powf(-s);
    let f7 = 1.0 - 7.0_f32.powf(-s);
    1.0 / (f2 * f3 * f5 * f7)
}

/// `zeta(1 - s)` for `s < 0`. Mirrors `petir_zeta_1ms_slt0`.
fn zeta_1ms_slt0(s: f32) -> f32 {
    if s > -19.0 {
        return cheb((-19.0 - 2.0 * s) / 19.0, &ZETA_XGT1) / (-s);
    }
    let f2 = 1.0 - 2.0_f32.powf(-(1.0 - s));
    let f3 = 1.0 - 3.0_f32.powf(-(1.0 - s));
    let f5 = 1.0 - 5.0_f32.powf(-(1.0 - s));
    let f7 = 1.0 - 7.0_f32.powf(-(1.0 - s));
    1.0 / (f2 * f3 * f5 * f7)
}

/// `zeta(s)` in `f32`. Mirrors `petir_zeta`.
///
/// `NaN` at `s = 1` and for `s <= -34`, where the reflection branch needs a
/// `Gamma` that `f32` cannot hold.
pub fn zeta(s: f32) -> f32 {
    if s == 1.0 {
        return f32::NAN;
    }
    if s >= 0.0 {
        return zeta_sgt0(s);
    }
    let m = s % 2.0;
    let sin_term = if m == 0.0 {
        0.0
    } else {
        (0.5 * PI * (s % 4.0)).sin() / PI
    };
    if sin_term == 0.0 {
        return 0.0;
    }
    if s <= -34.0 {
        return f32::NAN;
    }
    let n = ((-s) / 10.0).floor() as usize;
    let fs = s + 10.0 * n as f32;
    let Some(&tp) = TWOPI_POW.get(n) else {
        return f32::NAN;
    };
    let p = (2.0 * PI).powf(fs) / tp;
    p * crate::wgsl::mirror_gamma::gamma(1.0 - s) * sin_term * zeta_1ms_slt0(s)
}

/// `zeta(s) - 1` in `f32`. Mirrors `petir_zetam1`.
pub fn zetam1(s: f32) -> f32 {
    if s <= 5.0 {
        return zeta(s) - 1.0;
    }
    if s < 15.0 {
        return cheb((s - 10.0) / 5.0, &ZETAM1_INTER).exp() + 2.0_f32.powf(-s);
    }
    let a = 2.0_f32.powf(-s);
    let b = 3.0_f32.powf(-s);
    let c = 5.0_f32.powf(-s);
    let d = 7.0_f32.powf(-s);
    let e = 11.0_f32.powf(-s);
    let f = 13.0_f32.powf(-s);
    let t1 = a + b + c + d + e + f;
    let t2 = a * (b + c + d + e + f) + b * (c + d + e + f) + c * (d + e + f) + d * (e + f) + e * f;
    let z = 1.0 / ((1.0 - a) * (1.0 - b) * (1.0 - c) * (1.0 - d) * (1.0 - e) * (1.0 - f));
    (t1 - t2) * z
}

/// `eta(s)` in `f32`. Mirrors `petir_eta`.
pub fn eta(s: f32) -> f32 {
    if s > 100.0 {
        return 1.0;
    }
    if (s - 1.0).abs() < 10.0 * ROOT5_EPS {
        let del = s - 1.0;
        let c1 = LN2 * (EULER - 0.5 * LN2);
        return LN2
            + del
                * (c1 + del * (-0.032_686_297 + del * (0.001_568_991_7 + del * 0.000_749_872_4)));
    }
    (1.0 - ((1.0 - s) * LN2).exp()) * zeta(s)
}

/// `psi(1, x)` in `f32`. Mirrors `petir_psi_1`.
pub fn psi_1(x: f32) -> f32 {
    if x == 0.0 || x == -1.0 || x == -2.0 {
        return f32::NAN;
    }
    if x > 0.0 {
        return hzeta(2.0, x);
    }
    if x > -5.0 {
        let m = -x.floor();
        let fx = x + m;
        if fx == 0.0 {
            return f32::NAN;
        }
        let mut sum = 0.0_f32;
        for k in 0..(m as u32) {
            let t = x + k as f32;
            sum += 1.0 / (t * t);
        }
        return hzeta(2.0, fx) + sum;
    }
    let sp = (PI * x).sin();
    PI * PI / (sp * sp) - hzeta(2.0, 1.0 - x)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::specfunc::{psi as f64_psi, zeta as f64_zeta};

    /// Worst relative difference between a mirror function and its `f64`
    /// original over a sweep, with the location.
    fn worst_vs_f64(
        mirror: impl Fn(f32) -> f32,
        exact: impl Fn(f64) -> f64,
        points: impl Iterator<Item = f32>,
    ) -> (f32, f32) {
        let (mut worst, mut at) = (0.0_f32, 0.0_f32);
        for x in points {
            let r = exact(x as f64);
            let m = mirror(x);
            if r == 0.0 || !r.is_finite() || !m.is_finite() {
                continue;
            }
            let e = (((m as f64) - r) / r).abs() as f32;
            if e > worst {
                worst = e;
                at = x;
            }
        }
        (worst, at)
    }

    /// The `f32` mirror against PETIR's `f64` originals — what `f32` costs.
    ///
    /// # Results, measured 2026-09-19
    ///
    /// | function | worst | at | window |
    /// |---|---|---|---|
    /// | `psi` | 1.152e-07 | 10.9 | `(0, 30]` |
    /// | `zeta` | 9.391e-06 | -29.475 | `[-30, 40]` |
    /// | `eta` | 6.775e-06 | -15.7 | `[-20, 40]` |
    /// | `zetam1` | 2.090e-07 | 15.6 | `(5, 40]` |
    /// | `psi_1piy` | 7.540e-08 | 195 | `(0, 2000]` |
    ///
    /// The two outliers are both the reflection branch — see
    /// `the_reflection_branch_is_what_costs_digits`.
    #[test]
    fn the_f32_cost_is_what_it_was_measured_to_be() {
        // psi over (0, 30], avoiding its zero at 1.4616.
        let (w, at) = worst_vs_f64(
            psi,
            f64_psi::psi,
            (1..=300)
                .map(|k| 0.1 * k as f32)
                .filter(|x| (x - 1.46).abs() > 0.1),
        );
        assert!(w < 1e-5, "psi: {w:e} at {at}");

        // zeta over [-30, 0) and (1, 40], away from the pole and the zeros.
        let (w, at) = worst_vs_f64(
            zeta,
            f64_zeta::zeta,
            (1..=400)
                .map(|k| -30.0 + 0.175 * k as f32)
                .filter(|s| (s - 1.0).abs() > 0.2),
        );
        assert!(w < 1e-4, "zeta: {w:e} at {at}");

        // eta over [-20, 40], which has no pole at all.
        let (w, at) = worst_vs_f64(
            eta,
            f64_zeta::eta,
            (0..=600).map(|k| -20.0 + 0.1 * k as f32),
        );
        assert!(w < 1e-4, "eta: {w:e} at {at}");

        // zetam1 over (5, 40], its two dedicated branches.
        let (w, at) = worst_vs_f64(
            zetam1,
            f64_zeta::zetam1,
            (1..=350).map(|k| 5.0 + 0.1 * k as f32),
        );
        assert!(w < 1e-4, "zetam1: {w:e} at {at}");

        // psi_1piy over (0, 2000], spanning all four branches.
        let (w, at) = worst_vs_f64(
            psi_1piy,
            f64_psi::psi_1piy,
            (1..=400).map(|k| 5.0 * k as f32),
        );
        assert!(w < 1e-4, "psi_1piy: {w:e} at {at}");
    }

    /// `pow` is **not** what costs digits in `hzeta`, contradicting the
    /// prediction recorded in this module's documentation.
    ///
    /// `hzeta` calls `pow` eleven times per evaluation with exponent `-s`, so
    /// the expectation was that accuracy would degrade as `s` grew. Measured
    /// at `q = 1`, it improves: 3.418e-08 at `s = 1.5` against 2.877e-10 at
    /// `s = 20`. Keeping the assertion means the documented explanation stays
    /// falsifiable.
    ///
    /// The worst `hzeta` point over the whole sweep is 2.763e-07 at
    /// `(s, q) = (5.5, 0.25)` — one `f32` ulp, the same as `psi` and
    /// `zetam1`. What is genuinely worse here is `zeta` below zero, and
    /// `the_reflection_branch_is_what_costs_digits` measures that instead.
    #[test]
    fn pow_is_not_what_costs_hzeta_its_digits() {
        let mut worst = 0.0_f32;
        let mut at = (0.0_f32, 0.0_f32);
        for i in 1..=60 {
            let s = 1.0 + 0.5 * i as f32;
            for q in [0.25_f32, 0.5, 1.0, 2.0, 7.5] {
                let r = f64_zeta::hzeta(s as f64, q as f64);
                if !r.is_finite() || r == 0.0 {
                    continue;
                }
                let e = (((hzeta(s, q) as f64) - r) / r).abs() as f32;
                if e > worst {
                    worst = e;
                    at = (s, q);
                }
            }
        }
        assert!(worst < 1e-6, "hzeta: {worst:e} at (s, q) = {at:?}");

        // The prediction, stated as the assertion that would catch it coming
        // back: large s is where pow does the most work, and it is BETTER
        // there than at small s.
        let rel = |s: f32| {
            let r = f64_zeta::hzeta(s as f64, 1.0);
            (((hzeta(s, 1.0) as f64) - r) / r).abs()
        };
        assert!(
            rel(20.0) < rel(1.5),
            "hzeta at s = 20 ({:e}) is documented as MORE accurate than at \
             s = 1.5 ({:e}), which is what refuted the pow hypothesis; \
             re-measure and rewrite the module docs rather than deleting this",
            rel(20.0),
            rel(1.5)
        );
    }

    /// `zeta` and `eta` below zero are two orders worse than everything else
    /// in this module, and the cause is `Gamma` in the functional equation.
    ///
    /// Measured: `zeta` 9.391e-06 at `s = -29.475`, `eta` 6.775e-06 at
    /// `s = -15.7`, against 1.152e-07 for `psi` and 2.090e-07 for `zetam1` on
    /// the positive axis. [`crate::wgsl::mirror_gamma`] independently measures
    /// its own Lanczos branch at 7.839e-06, so `zeta` is very nearly just
    /// `Gamma`'s error passed through a multiplication.
    #[test]
    fn the_reflection_branch_is_what_costs_digits() {
        let (worst_neg, at_neg) = worst_vs_f64(
            zeta,
            f64_zeta::zeta,
            (1..=200).map(|k| -30.0 + 0.145 * k as f32),
        );
        let (worst_pos, _) = worst_vs_f64(
            zeta,
            f64_zeta::zeta,
            (1..=200).map(|k| 1.2 + 0.2 * k as f32),
        );
        assert!(
            worst_neg > 10.0 * worst_pos,
            "zeta below zero ({worst_neg:e} at {at_neg}) is documented as far \
             worse than above it ({worst_pos:e}) because the functional \
             equation carries Gamma's f32 error; re-measure and rewrite the \
             docs rather than deleting this assertion"
        );
        assert!(worst_neg < 1e-4, "zeta below zero: {worst_neg:e}");
    }

    /// The `f32` identities: `psi`'s recurrence and `psi_1`'s, which cross
    /// the branch cuts and cannot be satisfied by a single mistyped table.
    #[test]
    fn the_recurrences_hold_in_f32() {
        let mut worst_psi = 0.0_f32;
        let mut worst_psi1 = 0.0_f32;
        for k in 1..=300 {
            let x = 0.1 * k as f32;
            worst_psi = worst_psi.max((((psi(x + 1.0) - psi(x)) - 1.0 / x) * x).abs());
            let d = psi_1(x + 1.0) - psi_1(x);
            worst_psi1 = worst_psi1.max(((d + 1.0 / (x * x)) * x * x).abs());
        }
        assert!(worst_psi < 1e-4, "psi recurrence in f32: {worst_psi:e}");
        assert!(worst_psi1 < 1e-3, "psi_1 recurrence in f32: {worst_psi1:e}");
    }

    /// `hzeta(s, 1) = zeta(s)` — two different branch structures.
    #[test]
    fn hzeta_reduces_to_zeta_in_f32() {
        let mut worst = 0.0_f32;
        for k in 1..=60 {
            let s = 1.0 + 0.5 * k as f32;
            let r = zeta(s);
            if r != 0.0 {
                worst = worst.max(((hzeta(s, 1.0) - r) / r).abs());
            }
        }
        assert!(worst < 1e-4, "hzeta(s,1) vs zeta(s) in f32: {worst:e}");
    }

    /// `eta(1) = ln 2` exactly, and `zeta` is exactly zero at the negative
    /// even integers down to the `f32` cut.
    #[test]
    fn the_exact_values_are_still_exact_in_f32() {
        assert_eq!(eta(1.0), LN2);
        for k in 1..=16 {
            let n = -2.0 * k as f32;
            assert_eq!(zeta(n), 0.0, "zeta({n})");
        }
    }

    /// The documented `f32`-only refusal: `zeta(s)` for `s <= -34` returns
    /// `NaN` because the reflection branch needs a `Gamma` past `f32::MAX`,
    /// while the `f64` module answers there.
    ///
    /// The value is often representable even though the route is not —
    /// `zeta(-35)` is about 8.4e14 — which is why this is a refusal rather
    /// than an overflow, and why it is recorded rather than left to surprise
    /// someone.
    #[test]
    fn zeta_refuses_below_minus_34_where_f32_cannot_carry_the_route() {
        // NOT -34 or -100: those are negative EVEN integers, so the
        // trivial-zero branch returns 0.0 before the refusal is reached.
        // That ordering is upstream's and is correct -- the zeros are exact
        // at any precision.
        assert_eq!(zeta(-34.0), 0.0);
        assert_eq!(zeta(-100.0), 0.0);
        assert!(zeta(-34.5).is_nan());
        assert!(zeta(-35.0).is_nan());
        assert!(zeta(-99.5).is_nan());
        // Just inside, it still works.
        assert!(zeta(-33.5).is_finite() && zeta(-33.5) != 0.0);
        // The f64 module has no such limit, and the answer is representable.
        let exact = f64_zeta::zeta(-35.0);
        assert!(exact.is_finite());
        assert!(
            (exact.abs() as f32).is_finite(),
            "zeta(-35) = {exact:e} is documented as representable in f32 even \
             though the route to it is not"
        );
        // The trivial zeros below the cut are still exact -- they are
        // returned before the refusal.
        assert_eq!(zeta(-36.0), 0.0);
    }

    /// The domain errors, spelled `!(x > 0.0)` in both shader and mirror so
    /// that `NaN` propagates rather than falling into a branch.
    #[test]
    fn the_domain_errors_match_the_f64_modules() {
        for x in [0.0_f32, -1.0, -2.0] {
            assert!(psi(x).is_nan(), "psi({x})");
            assert!(psi_1(x).is_nan(), "psi_1({x})");
        }
        assert!(zeta(1.0).is_nan());
        assert!(hzeta(1.0, 1.0).is_nan());
        assert!(hzeta(0.5, 1.0).is_nan());
        assert!(hzeta(2.0, 0.0).is_nan());
        assert!(hzeta(2.0, -1.0).is_nan());
        assert!(hzeta(f32::NAN, 1.0).is_nan());
        assert!(hzeta(2.0, f32::NAN).is_nan());
    }

    /// `psi_1piy` is even in `y`, which the shader carries through `abs`.
    #[test]
    fn psi_1piy_is_even_in_f32() {
        for k in 1..=100 {
            let y = 0.5 * k as f32;
            assert_eq!(psi_1piy(y), psi_1piy(-y), "psi_1piy parity at {y}");
        }
    }
}
