// SPDX-License-Identifier: GPL-3.0-only
// Copyright (C) 2026 Theodore Kay Chen Ong (OUTRAM PARK workspace)
//
// This program is free software: you can redistribute it and/or modify it
// under the terms of the GNU General Public License as published by the
// Free Software Foundation, version 3 of the License.
//
// This program is distributed in the hope that it will be useful, but
// WITHOUT ANY WARRANTY; without even the implied warranty of
// MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE.  See the GNU
// General Public License for more details.
//
// You should have received a copy of the GNU General Public License along
// with this program.  If not, see <https://www.gnu.org/licenses/>.

//! Real-coefficient polynomial helpers for the z-domain conversions.
//!
//! # Conventions
//!
//! Every polynomial is a `Vec<f64>` in **ascending** powers of the variable:
//! `p[k]` is the coefficient of `x^k`. Coefficients are dimensionless `f64`
//! (see the module-level unit note in [`super`]). The zero polynomial is an
//! empty vector after trimming.
//!
//! # What does not belong here
//!
//! General polynomial root-finding. Roots are computed analytically for
//! degree <= 2 only ([`roots_deg_le_2`]); higher degrees are the follow-up
//! bead's business, not a half-implemented eigensolver here.

use super::cplx::Cplx;

/// Removes trailing (highest-power) coefficients smaller in magnitude than
/// `1e-14 * max|coeff|`, so degrees are meaningful after arithmetic.
/// Returns an empty vector for the (near-)zero polynomial.
pub(crate) fn trim(mut p: Vec<f64>) -> Vec<f64> {
    let max_abs = p.iter().fold(0.0_f64, |m, c| m.max(c.abs()));
    if max_abs == 0.0 {
        return Vec::new();
    }
    let tol = 1e-14 * max_abs;
    while let Some(last) = p.last() {
        if last.abs() <= tol {
            p.pop();
        } else {
            break;
        }
    }
    p
}

/// Degree of a trimmed polynomial; `0` for constants and for the empty
/// (zero) polynomial.
pub(crate) fn degree(p: &[f64]) -> usize {
    p.len().saturating_sub(1)
}

/// Polynomial product (convolution). Ascending-power convention.
pub(crate) fn mul(a: &[f64], b: &[f64]) -> Vec<f64> {
    if a.is_empty() || b.is_empty() {
        return Vec::new();
    }
    let mut out = vec![0.0; a.len() + b.len() - 1];
    for (i, &ai) in a.iter().enumerate() {
        for (j, &bj) in b.iter().enumerate() {
            out[i + j] += ai * bj;
        }
    }
    out
}

/// Sum of two polynomials (ascending powers), padding the shorter one.
pub(crate) fn add(a: &[f64], b: &[f64]) -> Vec<f64> {
    let n = a.len().max(b.len());
    let mut out = vec![0.0; n];
    for (i, &ai) in a.iter().enumerate() {
        out[i] += ai;
    }
    for (i, &bi) in b.iter().enumerate() {
        out[i] += bi;
    }
    out
}

/// Multiplies every coefficient by a scalar.
pub(crate) fn scale(p: &[f64], k: f64) -> Vec<f64> {
    p.iter().map(|c| c * k).collect()
}

/// `base^n` by repeated multiplication (`n` is at most the system order, so
/// no fast exponentiation is warranted). `base^0 = [1]`.
pub(crate) fn pow(base: &[f64], n: usize) -> Vec<f64> {
    let mut out = vec![1.0];
    for _ in 0..n {
        out = mul(&out, base);
    }
    out
}

/// Evaluates the polynomial at a complex point by Horner's rule.
///
/// Currently exercised only by the verification tests (frequency-response
/// and DC-gain checks), hence the `dead_code` allowance for non-test
/// builds.
#[cfg_attr(not(test), allow(dead_code))]
pub(crate) fn eval(p: &[f64], x: Cplx) -> Cplx {
    let mut acc = Cplx::real(0.0);
    for &c in p.iter().rev() {
        acc = acc * x + Cplx::real(c);
    }
    acc
}

/// Analytic roots of a real polynomial of degree <= 2 (ascending powers).
///
/// Returns the roots as complex numbers (a conjugate pair for a negative
/// discriminant). A constant or empty polynomial has no roots. Callers must
/// have trimmed the polynomial first so the leading coefficient is nonzero.
///
/// Returns `None` if the degree exceeds 2 — the caller converts that into
/// [`super::ZDomainError::UnsupportedOrder`].
pub(crate) fn roots_deg_le_2(p: &[f64]) -> Option<Vec<Cplx>> {
    match p.len() {
        0 | 1 => Some(Vec::new()),
        2 => {
            // p0 + p1 x = 0
            Some(vec![Cplx::real(-p[0] / p[1])])
        }
        3 => {
            // p0 + p1 x + p2 x^2 = 0 -> x = (-b +- sqrt(b^2 - 4ac)) / (2a)
            let (c, b, a) = (p[0], p[1], p[2]);
            let disc = b * b - 4.0 * a * c;
            let sqrt_disc = Cplx::real(disc).sqrt();
            let two_a = Cplx::real(2.0 * a);
            let minus_b = Cplx::real(-b);
            Some(vec![
                (minus_b + sqrt_disc) / two_a,
                (minus_b - sqrt_disc) / two_a,
            ])
        }
        _ => None,
    }
}

/// Rebuilds a real polynomial (ascending powers) from its complex roots and
/// a real leading-coefficient gain: `k * prod (x - r_i)`.
///
/// The imaginary parts of the expanded coefficients are discarded; for root
/// sets that are closed under conjugation they are rounding noise. Callers
/// that can produce genuinely unpaired complex roots (matched `d2c` of a
/// negative-real discrete pole) must document that the real part is taken.
pub(crate) fn from_roots(roots: &[Cplx], k: f64) -> Vec<f64> {
    let mut acc = vec![Cplx::real(k)];
    for &r in roots {
        // multiply acc by (x - r)
        let mut next = vec![Cplx::real(0.0); acc.len() + 1];
        for (i, &c) in acc.iter().enumerate() {
            next[i] = next[i] + c * (-r); // constant term contribution
            next[i + 1] = next[i + 1] + c; // x * c
        }
        acc = next;
    }
    acc.into_iter().map(|c| c.re).collect()
}
