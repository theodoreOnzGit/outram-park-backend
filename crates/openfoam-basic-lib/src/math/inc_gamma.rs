// SPDX-License-Identifier: GPL-3.0-only
// Copyright (C) 2026 OUTRAM PARK contributors
// Derived from OpenFOAM (www.openfoam.com)
// Copyright (C) 2004-2023 OpenFOAM Foundation
// Copyright (C) 2016-2023 OpenCFD Ltd.
//
// This file is part of OUTRAM PARK.
//
// OUTRAM PARK is free software: you can redistribute it and/or modify it
// under the terms of the GNU General Public License as published by the
// Free Software Foundation, either version 3 of the License, or (at your
// option) any later version.
//
// OUTRAM PARK is distributed in the hope that it will be useful, but
// WITHOUT ANY WARRANTY; without even the implied warranty of
// MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE.  See the GNU
// General Public License for more details.
//
// You should have received a copy of the GNU General Public License along
// with OUTRAM PARK.  If not, see <https://www.gnu.org/licenses/>.

// Incomplete gamma functions.
// Implements Foam::Math::incGammaRatio_Q, incGammaRatio_P, incGamma_Q, incGamma_P
// from incGamma.C.
// Algorithm: DiDonato & Morris (DM) — ACM TOMS 12(4), 1986.

// erf / erfc are not in stable Rust std; call through C ABI.
extern "C" {
    fn erf(x: f64) -> f64;
    fn erfc(x: f64) -> f64;
    fn tgamma(x: f64) -> f64;
}

#[inline]
fn c_erf(x: f64) -> f64   { unsafe { erf(x) } }
#[inline]
fn c_erfc(x: f64) -> f64  { unsafe { erfc(x) } }
#[inline]
fn c_gamma(x: f64) -> f64 { unsafe { tgamma(x) } }

fn factorial(n: i32) -> f64 {
    (1..=n).fold(1.0_f64, |acc, i| acc * i as f64)
}

// (DM:Eq. 13) — continued-fraction expansion for Q
fn calc_qe11(a: f64, x: f64, e: i32) -> f64 {
    let mut a_2n   = 0.0_f64;
    let mut b_2n   = 1.0_f64;
    let mut a_2np1 = 1.0_f64;
    let mut b_2np1 = x;

    let mut n = 1;
    while 2 * n <= e {
        let a_2nm1 = a_2np1;
        let b_2nm1 = b_2np1;

        let n_minus_a = n as f64 - a;
        a_2n = a_2nm1 + n_minus_a * a_2n;
        b_2n = b_2nm1 + n_minus_a * b_2n;

        a_2np1 = x * a_2n + n as f64 * a_2nm1;
        b_2np1 = x * b_2n + n as f64 * b_2nm1;

        n += 1;
    }

    if 2 * (n - 1) < e {
        a_2np1 / b_2np1
    } else {
        a_2n / b_2n
    }
}

// (DM:Eq. 15) — series expansion for P
fn calc_pe15(a: f64, x: f64, nmax: i32) -> f64 {
    let mut prod = 1.0_f64;
    let mut sum  = 0.0_f64;
    for n in 1..=nmax {
        prod *= a + n as f64;
        sum  += x.powi(n) / prod;
    }
    let r = (-x).exp() * x.powf(a) / c_gamma(a);
    r / a * (1.0 + sum)
}

// (DM:Eq. 16) — asymptotic expansion for Q (large x)
fn calc_qe16(a: f64, x: f64, n_terms: i32) -> f64 {
    let mut an  = 1.0_f64;
    let mut sum = 0.0_f64;
    for n in 1..=(n_terms - 1) {
        an  *= a - n as f64;
        sum += an / x.powi(n);
    }
    let r = (-x).exp() * x.powf(a) / c_gamma(a);
    r / x * (1.0 + sum)
}

// (DM:Eq. 18) — Temme approximation for large a
fn calc_te18(a: f64, e0: f64, _x: f64, lambda: f64, sigma: f64, phi: f64) -> f64 {
    const D0_0: f64 = -0.333_333_333_333_333e+00;
    const D0_1: f64 =  0.833_333_333_333_333e-01;
    const D0_2: f64 = -0.148_148_148_148_148e-01;
    const D0_3: f64 =  0.115_740_740_740_741e-02;
    const D0_4: f64 =  0.352_733_686_067_019e-03;
    const D0_5: f64 = -0.178_755_144_032_922e-03;
    const D0_6: f64 =  0.391_926_317_852_244e-04;

    const D1_0: f64 = -0.185_185_185_185_185e-02;
    const D1_1: f64 = -0.347_222_222_222_222e-02;
    const D1_2: f64 =  0.264_550_264_550_265e-02;
    const D1_3: f64 = -0.990_226_337_448_560e-03;
    const D1_4: f64 =  0.205_761_316_872_428e-03;

    const D2_0: f64 =  0.413_359_788_359_788e-02;
    const D2_1: f64 = -0.268_132_716_049_383e-02;

    let u  = 1.0 / a;
    let mut z = (2.0 * phi).sqrt();
    if lambda < 1.0 { z = -z; }

    let c2 = D2_1 * z + D2_0;

    if sigma > e0 / a.sqrt() {
        let z2 = z * z;
        let z3 = z2 * z;
        let z4 = z2 * z2;
        let z5 = z4 * z;
        let z6 = z3 * z3;
        let c0 = D0_6*z6 + D0_5*z5 + D0_4*z4 + D0_3*z3 + D0_2*z2 + D0_1*z + D0_0;
        let c1 = D1_4*z4 + D1_3*z3 + D1_2*z2 + D1_1*z + D1_0;
        c2*u*u + c1*u + c0
    } else {
        let z2 = z * z;
        let c0 = D0_2*z2 + D0_1*z + D0_0;
        let c1 = D1_1*z + D1_0;
        c2*u*u + c1*u + c0
    }
}

/// Regularised upper incomplete gamma: `Q(a, x) = Γ(a, x) / Γ(a)`.
///
/// Selects from several branch formulas depending on `a` and `x` ranges,
/// exactly as in `Foam::Math::incGammaRatio_Q`.
pub fn inc_gamma_ratio_q(a: f64, x: f64) -> f64 {
    use std::f64::consts::PI;

    const BIG: f64 = 14.0;
    const X0:  f64 = 17.0;
    const E0:  f64 = 0.025;

    if a < 1.0 {
        if (a - 0.5).abs() < f64::EPSILON {
            // (DM:Eq. 8)
            if x < 0.25 {
                return 1.0 - c_erf(x.sqrt());
            } else {
                return c_erfc(x.sqrt());
            }
        } else if x < 1.1 {
            // (DM:Eq. 12)
            let alpha = if x < 0.5 {
                (0.765_f64.sqrt().ln()) / x.ln()
            } else {
                x / 2.59
            };

            let mut sum = 0.0;
            for n in 1i32..=10 {
                sum += (-x).powi(n) / ((a + n as f64) * factorial(n));
            }
            let j = -a * sum;

            if a >= alpha {
                // (DM:Eq. 9)
                return 1.0 - (x.powf(a) * (1.0 - j)) / c_gamma(a + 1.0);
            } else {
                // (DM:Eq. 10)
                let l = (a * x.ln()).exp() - 1.0;
                let h = 1.0 / c_gamma(a + 1.0) - 1.0;
                return (x.powf(a) * j - l) / c_gamma(a + 1.0) - h;
            }
        } else {
            // (DM:Eq. 11)
            let r = (-x).exp() * x.powf(a) / c_gamma(a);
            return r * calc_qe11(a, x, 30);
        }
    } else if a >= BIG {
        let sigma = (1.0 - x / a).abs();

        let lambda = x / a;
        let phi    = lambda - 1.0 - lambda.ln();
        let y      = a * phi;

        if sigma <= E0 / a.sqrt() {
            // (DM:Eq. 19)
            let e_val = 0.5 - (1.0 - y / 3.0) * (y / PI).sqrt();
            let te    = calc_te18(a, E0, x, lambda, sigma, phi);
            return if lambda <= 1.0 {
                1.0 - (e_val - (1.0 - y) / (2.0 * PI * a).sqrt() * te)
            } else {
                e_val + (1.0 - y) / (2.0 * PI * a).sqrt() * te
            };
        } else if sigma <= 0.4 {
            // (DM:Eq. 17)
            let te = calc_te18(a, E0, x, lambda, sigma, phi);
            return if lambda <= 1.0 {
                1.0 - (0.5 * c_erfc(y.sqrt())
                    - (-y).exp() / (2.0 * PI * a).sqrt() * te)
            } else {
                0.5 * c_erfc(y.sqrt())
                    + (-y).exp() / (2.0 * PI * a).sqrt() * te
            };
        } else if x <= a.max(10.0_f64.ln()) {
            return 1.0 - calc_pe15(a, x, 20);
        } else if x < X0 {
            let r = (-x).exp() * x.powf(a) / c_gamma(a);
            return r * calc_qe11(a, x, 30);
        } else {
            return calc_qe16(a, x, 20);
        }
    } else {
        // 1 ≤ a < BIG
        if a > x || x >= X0 {
            if x <= a.max(10.0_f64.ln()) {
                return 1.0 - calc_pe15(a, x, 20);
            } else if x < X0 {
                let r = (-x).exp() * x.powf(a) / c_gamma(a);
                return r * calc_qe11(a, x, 30);
            } else {
                return calc_qe16(a, x, 20);
            }
        } else {
            let a_floor = a.floor();
            let half_a  = (a * 2.0).floor();

            if half_a == a * 2.0 {
                // a is a multiple of 0.5
                if a_floor == a {
                    // a is a positive integer: (DM:Eq. 14) exponential sum
                    let mut sum = 0.0;
                    for n in 0..=(a as i64 - 1) {
                        sum += x.powi(n as i32) / factorial(n as i32);
                    }
                    return (-x).exp() * sum;
                } else {
                    // a = k + 0.5 for some integer k
                    let i = (a - 0.5) as i32;
                    let mut prod = 1.0_f64;
                    let mut sum  = 0.0_f64;
                    for n in 1..=i {
                        prod *= n as f64 - 0.5;
                        sum  += x.powi(n) / prod;
                    }
                    return c_erfc(x.sqrt())
                        + (-x).exp() / (PI * x).sqrt() * sum;
                }
            } else if x <= a.max(10.0_f64.ln()) {
                return 1.0 - calc_pe15(a, x, 20);
            } else if x < X0 {
                let r = (-x).exp() * x.powf(a) / c_gamma(a);
                return r * calc_qe11(a, x, 30);
            } else {
                return calc_qe16(a, x, 20);
            }
        }
    }
}

/// Regularised lower incomplete gamma: `P(a, x) = γ(a, x) / Γ(a) = 1 − Q(a, x)`.
#[inline]
pub fn inc_gamma_ratio_p(a: f64, x: f64) -> f64 {
    1.0 - inc_gamma_ratio_q(a, x)
}

/// Upper incomplete gamma: `Γ(a, x) = Q(a, x) · Γ(a)`.
#[inline]
pub fn inc_gamma_q(a: f64, x: f64) -> f64 {
    inc_gamma_ratio_q(a, x) * c_gamma(a)
}

/// Lower incomplete gamma: `γ(a, x) = P(a, x) · Γ(a)`.
#[inline]
pub fn inc_gamma_p(a: f64, x: f64) -> f64 {
    inc_gamma_ratio_p(a, x) * c_gamma(a)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn approx_eq(a: f64, b: f64, tol: f64) -> bool {
        (a - b).abs() < tol
    }

    // Reference: P(1, 1) = 1 − e⁻¹ ≈ 0.6321205588
    #[test]
    fn p_1_1() {
        let p = inc_gamma_ratio_p(1.0, 1.0);
        assert!(approx_eq(p, 1.0 - (-1.0_f64).exp(), 1e-6), "P(1,1)={p}");
    }

    // P + Q = 1
    #[test]
    fn pq_sum_to_one() {
        for &(a, x) in &[(0.5_f64, 1.0), (1.0, 2.0), (5.0, 3.0), (20.0, 15.0)] {
            let p = inc_gamma_ratio_p(a, x);
            let q = inc_gamma_ratio_q(a, x);
            assert!(approx_eq(p + q, 1.0, 1e-10), "P+Q≠1 for a={a} x={x}: {}", p+q);
        }
    }

    // Q(a, 0) = 1 for all a > 0
    #[test]
    fn q_at_zero() {
        for &a in &[0.5_f64, 1.0, 5.0, 15.0] {
            let q = inc_gamma_ratio_q(a, 1e-300);
            assert!(approx_eq(q, 1.0, 1e-4), "Q({a},0)={q}");
        }
    }

    // Γ(a,x) = Γ(a)·Q(a,x)
    #[test]
    fn unnormalised_consistency() {
        let a = 3.0;
        let x = 2.0;
        let gq = inc_gamma_q(a, x);
        let gp = inc_gamma_p(a, x);
        assert!(approx_eq(gq + gp, c_gamma(a), 1e-10), "γ+Γ≠Γ(a)");
    }
}
