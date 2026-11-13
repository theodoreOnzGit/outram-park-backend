// SPDX-License-Identifier: GPL-3.0-only
// Copyright (C) 2026 OUTRAM PARK contributors
//
// Ported from OpenMC (https://github.com/openmc-dev/openmc, MIT licence,
// Copyright (c) 2011-2026 Massachusetts Institute of Technology, UChicago
// Argonne LLC and OpenMC contributors; see LICENSE.openmc):
//   src/external/quartic_solver.cpp, include/openmc/external/quartic_solver.h
//   at commit 8f79d9d02483c90e4134f03a9df4b75ce6b062bf.
// That file is itself a C++ implementation of the algorithm of
//   Orellana, De Michele, "Algorithm 1010: Boosting Efficiency in Solving
//   Quartic Equations with No Compromise in Accuracy", ACM Trans. Math. Softw.
//   46(2), 2020, https://github.com/dsrd/quartic (the `oqs` namespace).
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

//! `oqs::quartic_solver`: OpenMC's quartic root finder for the torus ray
//! intersection (gh:#1386; code-to-code test
//! `outram-mc-libs/tests/surface_distance_vs_openmc_code_to_code.rs`).
//!
//! A line-by-line translation, same operation order, same branches. Translation
//! notes:
//!
//! - `std::abs/acos/cos/sqrt/pow/cbrt/copysign` map to the `f64` methods, which
//!   call the platform `libm` as the C++ does.
//! - Real roots (the only ones a torus distance uses) come out of the real
//!   branches (cases I and III with real coefficients), which this translation
//!   keeps bit-for-bit. The complex-coefficient branches (case II, and case III
//!   with `d3 > 0`) only ever yield roots with a non-zero imaginary part, which
//!   the caller discards; they are translated with a small private complex type
//!   whose `sqrt` and `/` use the textbook formulas rather than `libstdc++`'s
//!   `csqrt`/`__divdc3`, so those roots are not claimed bit-identical (and are
//!   never used).

use petir::mathf::RealMath;
use std::f64::consts::PI;

/// `pow(DBL_MAX, 1/3) / 1.618034`.
const CUBIC_RESCAL_FACT: f64 = 3.488062113727083e+102;
/// `pow(DBL_MAX, 1/4) / 1.618034`.
const QUART_RESCAL_FACT: f64 = 7.156344627944542e+76;
const MACHEPS: f64 = f64::EPSILON;

#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct Cx {
    pub re: f64,
    pub im: f64,
}

impl Cx {
    const fn new(re: f64, im: f64) -> Self {
        Self { re, im }
    }
    fn conj(self) -> Self {
        Self::new(self.re, -self.im)
    }
    fn abs(self) -> f64 {
        self.re.r_hypot(self.im)
    }
    fn mul(self, o: Self) -> Self {
        Self::new(self.re * o.re - self.im * o.im, self.re * o.im + self.im * o.re)
    }
    fn add(self, o: Self) -> Self {
        Self::new(self.re + o.re, self.im + o.im)
    }
    fn sub(self, o: Self) -> Self {
        Self::new(self.re - o.re, self.im - o.im)
    }
    fn scale(self, k: f64) -> Self {
        Self::new(self.re * k, self.im * k)
    }
    fn div_real(self, k: f64) -> Self {
        Self::new(self.re / k, self.im / k)
    }
    fn div(self, o: Self) -> Self {
        let den = o.re * o.re + o.im * o.im;
        Self::new(
            (self.re * o.re + self.im * o.im) / den,
            (self.im * o.re - self.re * o.im) / den,
        )
    }
    /// Principal square root (not libstdc++'s `csqrt`; see the module docs).
    fn sqrt(self) -> Self {
        let m = self.abs();
        let re = ((m + self.re) / 2.0).sqrt();
        let im = ((m - self.re) / 2.0).sqrt().copysign(self.im);
        Self::new(re, im)
    }
}

fn solve_cubic_analytic_depressed_handle_inf(b: f64, c: f64) -> f64 {
    // find analytically the dominant root of a depressed cubic x^3+b*x+c
    // where coefficients b and c are large (see sec. 2.2 in the manuscript)
    let q = -b / 3.0;
    let r = 0.5 * c;
    if r == 0.0 {
        return if b <= 0.0 { (-b).sqrt() } else { 0.0 };
    }
    let kk;
    if q.abs() < r.abs() {
        let qr = q / r;
        let qrsq = qr * qr;
        kk = 1.0 - q * qrsq;
    } else {
        let rq = r / q;
        kk = 1.0_f64.copysign(q) * (rq * rq / q - 1.0);
    }
    if kk < 0.0 {
        let sqrt_q = q.sqrt();
        let theta = ((r / q.abs()) / sqrt_q).r_acos();
        if 2.0 * theta < PI {
            -2.0 * sqrt_q * (theta / 3.0).r_cos()
        } else {
            -2.0 * sqrt_q * ((theta + 2.0 * PI) / 3.0).r_cos()
        }
    } else {
        let a;
        if q.abs() < r.abs() {
            a = -1.0_f64.copysign(r) * (r.abs() * (1.0 + kk.sqrt())).r_cbrt();
        } else {
            a = -1.0_f64.copysign(r) * (r.abs() + q.abs().sqrt() * q.abs() * kk.sqrt()).r_cbrt();
        }
        let b2 = if a == 0.0 { 0.0 } else { q / a };
        a + b2
    }
}

fn solve_cubic_analytic_depressed(b: f64, c: f64) -> f64 {
    // find analytically the dominant root of a depressed cubic x^3+b*x+c
    // (see sec. 2.2 in the manuscript)
    let q = -b / 3.0;
    let r = 0.5 * c;
    if q.abs() > 1e102 || r.abs() > 1e154 {
        return solve_cubic_analytic_depressed_handle_inf(b, c);
    }
    let q3 = q * q * q;
    let r2 = r * r;
    if r2 < q3 {
        let theta = (r / q3.sqrt()).r_acos();
        let sqrt_q = -2.0 * q.sqrt();
        if 2.0 * theta < PI {
            sqrt_q * (theta / 3.0).r_cos()
        } else {
            sqrt_q * ((theta + 2.0 * PI) / 3.0).r_cos()
        }
    } else {
        let a = -1.0_f64.copysign(r) * (r.abs() + (r2 - q3).sqrt()).r_powf(1.0 / 3.0);
        let b2 = if a == 0.0 { 0.0 } else { q / a };
        a + b2 // this is always largest root even if A=B
    }
}

fn calc_phi0(a: f64, b: f64, c: f64, d: f64, scaled: bool) -> f64 {
    // find phi0 as the dominant root of the depressed and shifted cubic
    // in eq. (79) (see also the discussion in sec. 2.2 of the manuscript)
    let mut diskr = 9.0 * a * a - 24.0 * b;
    // eq. (87)
    let s;
    if diskr > 0.0 {
        diskr = diskr.sqrt();
        s = -2.0 * b / (3.0 * a + diskr.copysign(a));
    } else {
        s = -a / 4.0;
    }
    // eqs. (83)
    let aq = a + 4.0 * s;
    let bq = b + 3.0 * s * (a + 2.0 * s);
    let cq = c + s * (2.0 * b + s * (3.0 * a + 4.0 * s));
    let dq = d + s * (c + s * (b + s * (a + s)));
    let mut gg = bq * bq / 9.0;
    let mut hh = aq * cq;

    let mut g = hh - 4.0 * dq - 3.0 * gg; // eq. (85)
    let mut h = (8.0 * dq + hh - 2.0 * gg) * bq / 3.0 - cq * cq - dq * aq * aq; // eq. (86)
    let mut rmax = solve_cubic_analytic_depressed(g, h);
    if rmax.is_nan() || rmax.is_infinite() {
        rmax = solve_cubic_analytic_depressed_handle_inf(g, h);
        if (rmax.is_nan() || rmax.is_infinite()) && scaled {
            // try harder: rescale also the depressed cubic if quartic has been
            // already rescaled
            let rfact = CUBIC_RESCAL_FACT;
            let rfactsq = rfact * rfact;
            let mut ggss = gg / rfactsq;
            let mut hhss = hh / rfactsq;
            let dqss = dq / rfactsq;
            let aqs = aq / rfact;
            let bqs = bq / rfact;
            let cqs = cq / rfact;
            // (upstream assigns ggss/hhss twice; the first values are dead)
            let _ = (ggss, hhss);
            ggss = bqs * bqs / 9.0;
            hhss = aqs * cqs;
            g = hhss - 4.0 * dqss - 3.0 * ggss;
            h = (8.0 * dqss + hhss - 2.0 * ggss) * bqs / 3.0
                - cqs * (cqs / rfact)
                - (dq / rfact) * aqs * aqs;
            rmax = solve_cubic_analytic_depressed(g, h);
            if rmax.is_nan() || rmax.is_infinite() {
                rmax = solve_cubic_analytic_depressed_handle_inf(g, h);
            }
            rmax *= rfact;
        }
    }
    let _ = (&mut gg, &mut hh);
    // Newton-Raphson used to refine phi0 (see end of sec. 2.2 in the manuscript)
    let mut x = rmax;
    let mut xsq = x * x;
    let xxx = x * xsq;
    let gx = g * x;
    let mut f = x * (xsq + g) + h;
    let mut maxtt = xxx.abs().max(gx.abs());
    if h.abs() > maxtt {
        maxtt = h.abs();
    }
    if f.abs() > MACHEPS * maxtt {
        for _iter in 0..8 {
            let df = 3.0 * xsq + g;
            if df == 0.0 {
                break;
            }
            let xold = x;
            x += -f / df;
            let fold = f;
            xsq = x * x;
            f = x * (xsq + g) + h;
            if f == 0.0 {
                break;
            }
            if f.abs() >= fold.abs() {
                x = xold;
                break;
            }
        }
    }
    x
}

fn calc_err_ldlt(b: f64, c: f64, d: f64, d2: f64, l1: f64, l2: f64, l3: f64) -> f64 {
    // Eqs. (29) and (30) in the manuscript
    let mut sum = if b == 0.0 {
        (d2 + l1 * l1 + 2.0 * l3).abs()
    } else {
        (((d2 + l1 * l1 + 2.0 * l3) - b) / b).abs()
    };
    sum += if c == 0.0 {
        (2.0 * d2 * l2 + 2.0 * l1 * l3).abs()
    } else {
        (((2.0 * d2 * l2 + 2.0 * l1 * l3) - c) / c).abs()
    };
    sum += if d == 0.0 {
        (d2 * l2 * l2 + l3 * l3).abs()
    } else {
        (((d2 * l2 * l2 + l3 * l3) - d) / d).abs()
    };
    sum
}

fn calc_err_abcd_cmplx(a: f64, b: f64, c: f64, d: f64, aq: Cx, bq: Cx, cq: Cx, dq: Cx) -> f64 {
    // Eqs. (68) and (69) in the manuscript for complex alpha1 (aq), beta1 (bq),
    // alpha2 (cq) and beta2 (dq)
    let real = |z: f64| Cx::new(z, 0.0);
    let _ = real;
    let mut sum = if d == 0.0 {
        bq.mul(dq).abs()
    } else {
        bq.mul(dq).sub(Cx::new(d, 0.0)).div_real(d).abs()
    };
    sum += if c == 0.0 {
        bq.mul(cq).add(aq.mul(dq)).abs()
    } else {
        bq.mul(cq).add(aq.mul(dq)).sub(Cx::new(c, 0.0)).div_real(c).abs()
    };
    sum += if b == 0.0 {
        bq.add(aq.mul(cq)).add(dq).abs()
    } else {
        bq.add(aq.mul(cq)).add(dq).sub(Cx::new(b, 0.0)).div_real(b).abs()
    };
    sum += if a == 0.0 {
        aq.add(cq).abs()
    } else {
        aq.add(cq).sub(Cx::new(a, 0.0)).div_real(a).abs()
    };
    sum
}

fn calc_err_abcd(a: f64, b: f64, c: f64, d: f64, aq: f64, bq: f64, cq: f64, dq: f64) -> f64 {
    // Eqs. (68) and (69) in the manuscript for real alpha1 (aq), beta1 (bq),
    // alpha2 (cq) and beta2 (dq)
    let mut sum = if d == 0.0 { (bq * dq).abs() } else { ((bq * dq - d) / d).abs() };
    sum += if c == 0.0 {
        (bq * cq + aq * dq).abs()
    } else {
        (((bq * cq + aq * dq) - c) / c).abs()
    };
    sum += if b == 0.0 {
        (bq + aq * cq + dq).abs()
    } else {
        (((bq + aq * cq + dq) - b) / b).abs()
    };
    sum += if a == 0.0 { (aq + cq).abs() } else { (((aq + cq) - a) / a).abs() };
    sum
}

fn calc_err_abc(a: f64, b: f64, c: f64, aq: f64, bq: f64, cq: f64, dq: f64) -> f64 {
    // Eqs. (48)-(51) in the manuscript
    let mut sum = if c == 0.0 {
        (bq * cq + aq * dq).abs()
    } else {
        (((bq * cq + aq * dq) - c) / c).abs()
    };
    sum += if b == 0.0 {
        (bq + aq * cq + dq).abs()
    } else {
        (((bq + aq * cq + dq) - b) / b).abs()
    };
    sum += if a == 0.0 { (aq + cq).abs() } else { (((aq + cq) - a) / a).abs() };
    sum
}

fn nr_abcd(a: f64, b: f64, c: f64, d: f64, aq: &mut f64, bq: &mut f64, cq: &mut f64, dq: &mut f64) {
    // Newton-Raphson described in sec. 2.3 of the manuscript for complex
    // coefficients a,b,c,d
    let mut x = [*aq, *bq, *cq, *dq];
    let vr = [d, c, b, a];
    let mut fvec = [
        x[1] * x[3] - d,
        x[1] * x[2] + x[0] * x[3] - c,
        x[1] + x[0] * x[2] + x[3] - b,
        x[0] + x[2] - a,
    ];
    let mut errf = 0.0;
    for k1 in 0..4 {
        errf += if vr[k1] == 0.0 { fvec[k1].abs() } else { (fvec[k1] / vr[k1]).abs() };
    }
    for _iter in 0..8 {
        let x02 = x[0] - x[2];
        let det = x[1] * x[1] + x[1] * (-x[2] * x02 - 2.0 * x[3]) + x[3] * (x[0] * x02 + x[3]);
        if det == 0.0 {
            break;
        }
        let mut jinv = [[0.0f64; 4]; 4];
        jinv[0][0] = x02;
        jinv[0][1] = x[3] - x[1];
        jinv[0][2] = x[1] * x[2] - x[0] * x[3];
        jinv[0][3] = -x[1] * jinv[0][1] - x[0] * jinv[0][2];
        jinv[1][0] = x[0] * jinv[0][0] + jinv[0][1];
        jinv[1][1] = -x[1] * jinv[0][0];
        jinv[1][2] = -x[1] * jinv[0][1];
        jinv[1][3] = -x[1] * jinv[0][2];
        jinv[2][0] = -jinv[0][0];
        jinv[2][1] = -jinv[0][1];
        jinv[2][2] = -jinv[0][2];
        jinv[2][3] = jinv[0][2] * x[2] + jinv[0][1] * x[3];
        jinv[3][0] = -x[2] * jinv[0][0] - jinv[0][1];
        jinv[3][1] = jinv[0][0] * x[3];
        jinv[3][2] = x[3] * jinv[0][1];
        jinv[3][3] = x[3] * jinv[0][2];
        let mut dx = [0.0f64; 4];
        for k1 in 0..4 {
            dx[k1] = 0.0;
            for k2 in 0..4 {
                dx[k1] += jinv[k1][k2] * fvec[k2];
            }
        }
        let xold = x;
        for k1 in 0..4 {
            x[k1] += -dx[k1] / det;
        }
        fvec[0] = x[1] * x[3] - d;
        fvec[1] = x[1] * x[2] + x[0] * x[3] - c;
        fvec[2] = x[1] + x[0] * x[2] + x[3] - b;
        fvec[3] = x[0] + x[2] - a;
        let errfold = errf;
        errf = 0.0;
        for k1 in 0..4 {
            errf += if vr[k1] == 0.0 { fvec[k1].abs() } else { (fvec[k1] / vr[k1]).abs() };
        }
        if errf == 0.0 {
            break;
        }
        if errf >= errfold {
            x = xold;
            break;
        }
    }
    *aq = x[0];
    *bq = x[1];
    *cq = x[2];
    *dq = x[3];
}

fn solve_quadratic(a: f64, b: f64) -> [Cx; 2] {
    let diskr = a * a - 4.0 * b;
    if diskr >= 0.0 {
        let div = -a - diskr.sqrt().copysign(a);
        let zmax = div / 2.0;
        let zmin = if zmax == 0.0 { 0.0 } else { b / zmax };
        [Cx::new(zmax, 0.0), Cx::new(zmin, 0.0)]
    } else {
        let sqrtd = (-diskr).sqrt();
        [Cx::new(-a / 2.0, sqrtd / 2.0), Cx::new(-a / 2.0, -sqrtd / 2.0)]
    }
}

/// `oqs::quartic_solver`: roots of
/// `coeff[4] x^4 + coeff[3] x^3 + coeff[2] x^2 + coeff[1] x + coeff[0] = 0`
/// (`coeff[4] != 0`), as four complex numbers.
pub(crate) fn quartic_solver(coeff: [f64; 5]) -> [Cx; 4] {
    let mut roots = [Cx::new(0.0, 0.0); 4];
    let (mut acx, mut bcx, mut ccx, mut dcx) = (Cx::new(0.0, 0.0), Cx::new(0.0, 0.0), Cx::new(0.0, 0.0), Cx::new(0.0, 0.0));
    let mut l2m = [0.0f64; 12];
    let mut d2m = [0.0f64; 12];
    let mut res = [0.0f64; 12];
    let mut errv = [0.0f64; 3];
    let mut aqv = [0.0f64; 3];
    let mut cqv = [0.0f64; 3];
    let mut realcase = [0i32; 2];

    let mut a = coeff[3] / coeff[4];
    let mut b = coeff[2] / coeff[4];
    let mut c = coeff[1] / coeff[4];
    let mut d = coeff[0] / coeff[4];
    let mut phi0 = calc_phi0(a, b, c, d, false);

    // simple polynomial rescaling
    let mut rfact = 1.0;
    if phi0.is_nan() || phi0.is_infinite() {
        rfact = QUART_RESCAL_FACT;
        a /= rfact;
        let rfactsq = rfact * rfact;
        b /= rfactsq;
        c /= rfactsq * rfact;
        d /= rfactsq * rfactsq;
        phi0 = calc_phi0(a, b, c, d, true);
    }
    let l1 = a / 2.0; // eq. (16)
    let l3 = b / 6.0 + phi0 / 2.0; // eq. (18)
    let del2 = c - a * l3; // defined just after eq. (27)
    let mut nsol = 0usize;
    let bl311 = 2.0 * b / 3.0 - phi0 - l1 * l1; // This is d2 as defined in eq. (20)
    let dml3l3 = d - l3 * l3; // dml3l3 is d3 as defined in eq. (15) with d2=0

    // Three possible solutions for d2 and l2 (see eqs. (28) and discussion which follows)
    if bl311 != 0.0 {
        d2m[nsol] = bl311;
        l2m[nsol] = del2 / (2.0 * d2m[nsol]);
        res[nsol] = calc_err_ldlt(b, c, d, d2m[nsol], l1, l2m[nsol], l3);
        nsol += 1;
    }
    if del2 != 0.0 {
        l2m[nsol] = 2.0 * dml3l3 / del2;
        if l2m[nsol] != 0.0 {
            d2m[nsol] = del2 / (2.0 * l2m[nsol]);
            res[nsol] = calc_err_ldlt(b, c, d, d2m[nsol], l1, l2m[nsol], l3);
            nsol += 1;
        }
        d2m[nsol] = bl311;
        l2m[nsol] = 2.0 * dml3l3 / del2;
        res[nsol] = calc_err_ldlt(b, c, d, d2m[nsol], l1, l2m[nsol], l3);
        nsol += 1;
    }

    let (l2, d2);
    if nsol == 0 {
        l2 = 0.0;
        d2 = 0.0;
    } else {
        // we select the (d2,l2) pair which minimizes errors
        let mut resmin = 0.0;
        let mut kmin = 0usize;
        for k1 in 0..nsol {
            if k1 == 0 || res[k1] < resmin {
                resmin = res[k1];
                kmin = k1;
            }
        }
        d2 = d2m[kmin];
        l2 = l2m[kmin];
    }
    let mut whichcase = 0usize;
    let (mut aq, mut bq, mut cq, mut dq) = (0.0f64, 0.0f64, 0.0f64, 0.0f64);
    if d2 < 0.0 {
        // Case I eqs. (37)-(40)
        let gamma = (-d2).sqrt();
        aq = l1 + gamma;
        bq = l3 + gamma * l2;

        cq = l1 - gamma;
        dq = l3 - gamma * l2;
        if dq.abs() < bq.abs() {
            dq = d / bq;
        } else if dq.abs() > bq.abs() {
            bq = d / dq;
        }
        if aq.abs() < cq.abs() {
            nsol = 0;
            if dq != 0.0 {
                aqv[nsol] = (c - bq * cq) / dq; // see eqs. (47)
                errv[nsol] = calc_err_abc(a, b, c, aqv[nsol], bq, cq, dq);
                nsol += 1;
            }
            if cq != 0.0 {
                aqv[nsol] = (b - dq - bq) / cq; // see eqs. (47)
                errv[nsol] = calc_err_abc(a, b, c, aqv[nsol], bq, cq, dq);
                nsol += 1;
            }
            aqv[nsol] = a - cq; // see eqs. (47)
            errv[nsol] = calc_err_abc(a, b, c, aqv[nsol], bq, cq, dq);
            nsol += 1;
            // we select the value of aq (i.e. alpha1 in the manuscript) which minimizes errors
            let mut errmin = 0.0;
            let mut kmin = 0usize;
            for k in 0..nsol {
                if k == 0 || errv[k] < errmin {
                    kmin = k;
                    errmin = errv[k];
                }
            }
            aq = aqv[kmin];
        } else {
            nsol = 0;
            if bq != 0.0 {
                cqv[nsol] = (c - aq * dq) / bq; // see eqs. (53)
                errv[nsol] = calc_err_abc(a, b, c, aq, bq, cqv[nsol], dq);
                nsol += 1;
            }
            if aq != 0.0 {
                cqv[nsol] = (b - bq - dq) / aq; // see eqs. (53)
                errv[nsol] = calc_err_abc(a, b, c, aq, bq, cqv[nsol], dq);
                nsol += 1;
            }
            cqv[nsol] = a - aq; // see eqs. (53)
            errv[nsol] = calc_err_abc(a, b, c, aq, bq, cqv[nsol], dq);
            nsol += 1;
            // we select the value of cq (i.e. alpha2 in the manuscript) which minimizes errors
            let mut errmin = 0.0;
            let mut kmin = 0usize;
            for k in 0..nsol {
                if k == 0 || errv[k] < errmin {
                    kmin = k;
                    errmin = errv[k];
                }
            }
            cq = cqv[kmin];
        }
        realcase[0] = 1;
    } else if d2 > 0.0 {
        // Case II eqs. (53)-(56)
        let gamma = d2.sqrt();
        acx = Cx::new(l1, gamma);
        bcx = Cx::new(l3, gamma * l2);
        ccx = acx.conj();
        dcx = bcx.conj();
        realcase[0] = 0;
    } else {
        realcase[0] = -1; // d2=0
    }
    // Case III: d2 is 0 or approximately 0 (in this case check which solution is better)
    if realcase[0] == -1
        || (d2.abs() <= MACHEPS * ((2.0 * b / 3.0).abs() + phi0.abs() + l1 * l1))
    {
        let d3 = d - l3 * l3;
        let mut err0 = 0.0;
        if realcase[0] == 1 {
            err0 = calc_err_abcd(a, b, c, d, aq, bq, cq, dq);
        } else if realcase[0] == 0 {
            err0 = calc_err_abcd_cmplx(a, b, c, d, acx, bcx, ccx, dcx);
        }
        let (mut aq1, mut bq1, mut cq1, mut dq1) = (0.0f64, 0.0f64, 0.0f64, 0.0f64);
        let (mut acx1, mut bcx1, mut ccx1, mut dcx1) = (Cx::new(0.0, 0.0), Cx::new(0.0, 0.0), Cx::new(0.0, 0.0), Cx::new(0.0, 0.0));
        let err1;
        if d3 <= 0.0 {
            realcase[1] = 1;
            aq1 = l1;
            bq1 = l3 + (-d3).sqrt();
            cq1 = l1;
            dq1 = l3 - (-d3).sqrt();
            if dq1.abs() < bq1.abs() {
                dq1 = d / bq1;
            } else if dq1.abs() > bq1.abs() {
                bq1 = d / dq1;
            }
            err1 = calc_err_abcd(a, b, c, d, aq1, bq1, cq1, dq1); // eq. (68)
        } else {
            // complex
            realcase[1] = 0;
            acx1 = Cx::new(l1, 0.0);
            bcx1 = Cx::new(l3, 0.0).add(Cx::new(0.0, d3.sqrt()));
            ccx1 = Cx::new(l1, 0.0);
            dcx1 = bcx1.conj();
            err1 = calc_err_abcd_cmplx(a, b, c, d, acx1, bcx1, ccx1, dcx1);
        }
        if realcase[0] == -1 || err1 < err0 {
            whichcase = 1; // d2 = 0
            if realcase[1] == 1 {
                aq = aq1;
                bq = bq1;
                cq = cq1;
                dq = dq1;
            } else {
                acx = acx1;
                bcx = bcx1;
                ccx = ccx1;
                dcx = dcx1;
            }
        }
    }
    if realcase[whichcase] == 1 {
        // if alpha1, beta1, alpha2 and beta2 are real first refine the
        // coefficient through a Newton-Raphson
        nr_abcd(a, b, c, d, &mut aq, &mut bq, &mut cq, &mut dq);
        // finally calculate the roots as roots of p1(x) and p2(x) (see end of sec. 2.1)
        let q = solve_quadratic(aq, bq);
        roots[0] = q[0];
        roots[1] = q[1];
        let q = solve_quadratic(cq, dq);
        roots[2] = q[0];
        roots[3] = q[1];
    } else if whichcase == 0 {
        // complex coefficients of p1 and p2; d2 != 0
        let cdiskr = acx.mul(acx).scale(0.25).sub(bcx);
        // calculate the roots as roots of p1(x) and p2(x) (see end of sec. 2.1)
        let zx1 = acx.scale(-0.5).add(cdiskr.sqrt());
        let zx2 = acx.scale(-0.5).sub(cdiskr.sqrt());
        let zxmax = if zx1.abs() > zx2.abs() { zx1 } else { zx2 };
        let zxmin = bcx.div(zxmax);
        roots[0] = zxmin;
        roots[1] = zxmin.conj();
        roots[2] = zxmax;
        roots[3] = zxmax.conj();
    } else {
        // d2 ~ 0 (upstream's comment: "never gets here!")
        let cdiskr = acx.mul(acx).sub(bcx.scale(4.0)).sqrt();
        let mut zx1 = acx.add(cdiskr).scale(-0.5);
        let mut zx2 = acx.sub(cdiskr).scale(-0.5);
        let mut zxmax = if zx1.abs() > zx2.abs() { zx1 } else { zx2 };
        let mut zxmin = bcx.div(zxmax);
        roots[0] = zxmax;
        roots[1] = zxmin;
        let cdiskr = ccx.mul(ccx).sub(dcx.scale(4.0)).sqrt();
        zx1 = ccx.add(cdiskr).scale(-0.5);
        zx2 = ccx.sub(cdiskr).scale(-0.5);
        zxmax = if zx1.abs() > zx2.abs() { zx1 } else { zx2 };
        zxmin = dcx.div(zxmax);
        roots[2] = zxmax;
        roots[3] = zxmin;
    }
    if rfact != 1.0 {
        for r in roots.iter_mut() {
            *r = r.scale(rfact);
        }
    }
    roots
}

#[cfg(test)]
mod tests {
    use super::*;

    /// (x-1)(x-2)(x-3)(x-4) = x^4 - 10x^3 + 35x^2 - 50x + 24: four real roots.
    #[test]
    fn four_real_roots() {
        let mut r: Vec<f64> =
            quartic_solver([24.0, -50.0, 35.0, -10.0, 1.0]).iter().map(|z| z.re).collect();
        r.sort_by(|a, b| a.partial_cmp(b).unwrap());
        for (got, want) in r.iter().zip([1.0, 2.0, 3.0, 4.0]) {
            assert!((got - want).abs() < 1e-12, "{got} vs {want}");
        }
        assert!(quartic_solver([24.0, -50.0, 35.0, -10.0, 1.0]).iter().all(|z| z.im == 0.0));
    }

    /// x^4 + 5x^2 + 4 = (x^2+1)(x^2+4): four imaginary roots.
    #[test]
    fn four_complex_roots() {
        let z = quartic_solver([4.0, 0.0, 5.0, 0.0, 1.0]);
        assert!(z.iter().all(|r| r.im != 0.0));
    }
}
