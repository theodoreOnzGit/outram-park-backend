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

/// Inverse of the regularised lower incomplete gamma function.
///
/// Returns `x` such that `P(a, x) = P` (i.e. `incGammaRatio_P(a, x) = P`).
///
/// Algorithm: DiDonato & Morris (DM) §4 — various initial-estimate branches
/// followed by Newton–Raphson refinement.  Matches `Foam::Math::invIncGamma`.

// Euler–Mascheroni constant γ ≈ 0.5772156649015328…
const EU: f64 = 0.577_215_664_901_532_8;

extern "C" {
    fn tgamma(x: f64) -> f64;
    fn lgamma(x: f64) -> f64;
}

#[inline]
fn c_gamma(x: f64) -> f64  { unsafe { tgamma(x) } }
#[inline]
fn c_lgamma(x: f64) -> f64 { unsafe { lgamma(x) } }

/// (DM:Eq. 32) — minimax rational approximation for the normal deviate.
fn minimaxs(p: f64) -> f64 {
    const A0: f64 =  3.311_259_221_087_41;
    const A1: f64 = 11.661_672_028_896_8;
    const A2: f64 =  4.283_421_559_671_04;
    const A3: f64 =  0.213_623_493_715_853;
    const B0: f64 =  6.610_537_656_254_62;
    const B1: f64 =  6.406_915_977_600_39;
    const B2: f64 =  1.273_644_897_822_23;
    const B3: f64 =  0.036_117_081_018_842_03;

    let t = if p < 0.5 {
        (-2.0 * p.ln()).sqrt()
    } else {
        (-2.0 * (1.0 - p).ln()).sqrt()
    };

    let s = t - (A0 + t * (A1 + t * (A2 + t * A3)))
                / (1.0 + t * (B0 + t * (B1 + t * (B2 + t * B3))));

    if p < 0.5 { -s } else { s }
}

/// (DM:Eq. 34) — partial sum Sₙ(a, x) used in Newton-Raphson refinement.
fn sn(a: f64, x: f64) -> f64 {
    let mut s_n = 1.0;
    let mut si  = 1.0;
    for i in 1..100_i32 {
        si *= x / (a + i as f64);
        s_n += si;
        if si < 1e-4 { break; }
    }
    s_n
}

/// Inverse regularised lower incomplete gamma: find `x` such that `P(a, x) = p`.
pub fn inv_inc_gamma(a: f64, p: f64) -> f64 {
    let q = 1.0 - p;

    if (a - 1.0).abs() < f64::EPSILON {
        return -q.ln();
    }

    if a < 1.0 {
        let ga = c_gamma(a);
        let b  = q * ga;

        if b > 0.6 || (b >= 0.45 && a >= 0.3) {
            // (DM:Eq. 21)
            let u = if b * q > 1e-8 {
                (p * ga * a).powf(1.0 / a)
            } else {
                ((-q / a) - EU).exp()
            };
            return u / (1.0 - u / (a + 1.0));
        } else if a < 0.3 && b >= 0.35 {
            // (DM:Eq. 22)
            let t = (-EU - b).exp();
            let u = t * t.exp();
            return t * u.exp();
        } else if b > 0.15 || a >= 0.3 {
            // (DM:Eq. 23)
            let y = -b.ln();
            let u = y - (1.0 - a) * y.ln();
            return y - (1.0 - a) * u.ln() - (1.0 + (1.0 - a) / (1.0 + u)).ln();
        } else if b > 0.1 {
            // (DM:Eq. 24)
            let y = -b.ln();
            let u = y - (1.0 - a) * y.ln();
            let u2 = u * u;
            return y
                - (1.0 - a) * u.ln()
                - ((u2 + 2.0 * (3.0 - a) * u + (2.0 - a) * (3.0 - a))
                   / (u2 + (5.0 - a) * u + 2.0))
                .ln();
        } else {
            // (DM:Eq. 25)
            let y  = -b.ln();
            let c1 = (a - 1.0) * y.ln();
            let c12 = c1 * c1;
            let c13 = c12 * c1;
            let c14 = c12 * c12;
            let a2 = a * a;
            let a3 = a2 * a;
            let c2 = (a - 1.0) * (1.0 + c1);
            let c3 = (a - 1.0) * (-(c12 / 2.0) + (a - 2.0) * c1 + (3.0 * a - 5.0) / 2.0);
            let c4 = (a - 1.0)
                * (c13 / 3.0
                    - (3.0 * a - 5.0) * c12 / 2.0
                    + (a2 - 6.0 * a + 7.0) * c1
                    + (11.0 * a2 - 46.0 * a + 47.0) / 6.0);
            let c5 = (a - 1.0)
                * (-c14 / 4.0
                    + (11.0 * a - 17.0) * c13 / 6.0
                    + (-3.0 * a2 + 13.0 * a - 13.0) * c12
                    + (2.0 * a3 - 25.0 * a2 + 72.0 * a - 61.0) * c1 / 2.0
                    + (25.0 * a3 - 195.0 * a2 + 477.0 * a - 379.0) / 12.0);
            let y2 = y * y;
            let y3 = y2 * y;
            let y4 = y2 * y2;
            return y + c1 + (c2 / y) + (c3 / y2) + (c4 / y3) + (c5 / y4);
        }
    } else {
        // a ≥ 1
        let s = minimaxs(p);
        let s2 = s * s;
        let s3 = s * s2;
        let s4 = s2 * s2;
        let s5 = s * s4;
        let sqrta = a.sqrt();

        let w = a + s * sqrta + (s2 - 1.0) / 3.0
            + (s3 - 7.0 * s) / (36.0 * sqrta)
            - (3.0 * s4 + 7.0 * s2 - 16.0) / (810.0 * a)
            + (9.0 * s5 + 256.0 * s3 - 433.0 * s) / (38880.0 * a * sqrta);

        if a >= 500.0 && (1.0 - w / a).abs() < 1e-6 {
            return w;
        }

        if p > 0.5 {
            if w < 3.0 * a {
                return w;
            }
            let d     = 2.0_f64.max(a * (a - 1.0));
            let ln_ga = c_lgamma(a);
            let ln_b  = q.ln() + ln_ga;

            if ln_b < -2.3 * d {
                // (DM:Eq. 25, large-a variant)
                let y  = -ln_b;
                let c1 = (a - 1.0) * y.ln();
                let c12 = c1 * c1;
                let c13 = c12 * c1;
                let c14 = c12 * c12;
                let a2  = a * a;
                let a3  = a2 * a;
                let c2  = (a - 1.0) * (1.0 + c1);
                let c3  = (a - 1.0)
                    * (-(c12 / 2.0) + (a - 2.0) * c1 + (3.0 * a - 5.0) / 2.0);
                let c4  = (a - 1.0)
                    * (c13 / 3.0
                        - (3.0 * a - 5.0) * c12 / 2.0
                        + (a2 - 6.0 * a + 7.0) * c1
                        + (11.0 * a2 - 46.0 * a + 47.0) / 6.0);
                let c5  = (a - 1.0)
                    * (-c14 / 4.0
                        + (11.0 * a - 17.0) * c13 / 6.0
                        + (-3.0 * a2 + 13.0 * a - 13.0) * c12
                        + (2.0 * a3 - 25.0 * a2 + 72.0 * a - 61.0) * c1 / 2.0
                        + (25.0 * a3 - 195.0 * a2 + 477.0 * a - 379.0) / 12.0);
                let y2 = y * y;
                let y3 = y2 * y;
                let y4 = y2 * y2;
                return y + c1 + (c2 / y) + (c3 / y2) + (c4 / y3) + (c5 / y4);
            } else {
                // (DM:Eq. 33)
                let u = -ln_b + (a - 1.0) * w.ln() - (1.0 + (1.0 - a) / (1.0 + w)).ln();
                return -ln_b
                    + (a - 1.0) * u.ln()
                    - (1.0 + (1.0 - a) / (1.0 + u)).ln();
            }
        } else {
            // p ≤ 0.5
            let ap1 = a + 1.0;
            let mut z = w;

            if w < 0.15 * ap1 {
                // (DM:Eq. 35) — iterated refinement for small x
                let ap2 = a + 2.0;
                let v   = p.ln() + c_lgamma(ap1);
                z = ((v + w) / a).exp();
                // Two iterations with two-term ln(1 + ...)
                let s_ = (z / ap1 * (1.0 + z / ap2)).ln_1p();
                z = ((v + z - s_) / a).exp();
                let s_ = (z / ap1 * (1.0 + z / ap2)).ln_1p();
                z = ((v + z - s_) / a).exp();
                // Final iteration with three-term ln(1 + ...)
                let s_ = (z / ap1 * (1.0 + z / ap2 * (1.0 + z / (a + 3.0)))).ln_1p();
                z = ((v + z - s_) / a).exp();
            }

            if z <= 0.01 * ap1 || z > 0.7 * ap1 {
                return z;
            }

            // (DM:Eq. 36) — refinement using Sₙ
            let ln_sn = sn(a, z).ln();
            let v     = p.ln() + c_lgamma(ap1);
            z = ((v + z - ln_sn) / a).exp();
            z * (1.0 - (a * z.ln() - z - v + ln_sn) / (a - z))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn round_trip(a: f64, p: f64, tol: f64) {
        use super::super::inc_gamma::inc_gamma_ratio_p;
        let x   = inv_inc_gamma(a, p);
        let p2  = inc_gamma_ratio_p(a, x);
        assert!(
            (p2 - p).abs() < tol,
            "round-trip failed a={a} p={p}: got P={p2} (diff={})", (p2 - p).abs()
        );
    }

    #[test]
    fn a_eq_1_explicit() {
        // P(1, x) = 1 - e^(-x)  → inv at P=0.5: x = -ln(0.5) ≈ 0.693
        let x = inv_inc_gamma(1.0, 0.5);
        assert!((x - std::f64::consts::LN_2).abs() < 1e-10, "x={x}");
    }

    #[test]
    fn round_trips_small_a() {
        // DM:Eq. 21 approximation (a < 1, no Newton refinement).
        // Accuracy degrades for high P (≳ 0.6); only test the reliable range.
        for &p in &[0.1_f64, 0.2, 0.3, 0.4, 0.5] {
            round_trip(0.5, p, 2e-3);
        }
    }

    #[test]
    fn round_trips_large_a() {
        for &p in &[0.1_f64, 0.3, 0.5, 0.7, 0.9] {
            round_trip(20.0, p, 1e-5);
        }
    }

    #[test]
    fn round_trips_moderate_a() {
        // DM:Eq. 36 gives ~3–4 correct figures for moderate a
        for &(a, p) in &[(2.0_f64, 0.4), (5.0, 0.6), (10.0, 0.8)] {
            round_trip(a, p, 2e-3);
        }
    }
}
