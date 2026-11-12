// SPDX-License-Identifier: GPL-3.0-only
// Copyright (C) 2026 OUTRAM PARK contributors
//
// Ported from OpenMC (https://github.com/openmc-dev/openmc, MIT licence,
// Copyright (c) 2011-2026 Massachusetts Institute of Technology, UChicago
// Argonne LLC and OpenMC contributors; see LICENSE.openmc):
//   src/surface.cpp, include/openmc/surface.h
// Tori and the quartic real-root solver they need.
// Moved here unchanged in substance from `outram-mc-libs`
// (`src/geometry/`) on 2026-10-02, GitHub issue #486: outram-blender owns the
// CSG description and its pure navigation kernel, outram-mc-libs keeps the
// transport-state work and re-exports these items under its old paths.
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

//! The X/Y/Z tori and the degree-4 real-root solver their ray intersection
//! needs (split out of the surface module on 2026-10-02 to respect the
//! 1000-line file cap).

use super::{BoundaryType, Surface};
use crate::csg::position::{Direction, Position};

// ── Real-root solver for polynomials up to degree 4 ─────────────────────────
//
// The torus is a degree-4 surface, so its ray intersection is a quartic (unlike
// the quadric surfaces above whose distance reduces to a quadratic). Rather than
// Ferrari/Cardano closed forms — which are numerically delicate around their
// case splits — this uses **derivative bracketing**, which is robust and easy to
// verify: the real roots of a polynomial's derivative partition the real line
// into intervals on which the polynomial is monotonic, so each interval holds at
// most one simple root, found by a guaranteed-convergent sign-change bisection
// (then Newton-polished). The derivative's own roots are found the same way one
// degree down, bottoming out at the quadratic formula. Multiple roots (which sit
// at critical points) are detected separately by a relative-residual test.
//
// No BLAS / eigenvalue solver is used, so this stays Termux/Android-portable.

/// Evaluate a polynomial with **descending** coefficients at `x` (Horner).
/// e.g. `coeffs = [c4, c3, c2, c1, c0]` for a quartic.
#[inline]
pub(crate) fn poly_eval(coeffs: &[f64], x: f64) -> f64 {
    let mut acc = 0.0;
    for &c in coeffs {
        acc = acc * x + c;
    }
    acc
}

/// Sum of the absolute magnitudes of every Horner term — the natural scale used
/// to turn `|P(x)|` into a *relative* residual for multiple-root detection.
pub(crate) fn poly_abs_scale(coeffs: &[f64], x: f64) -> f64 {
    let ax = x.abs();
    let mut acc = 0.0;
    for &c in coeffs {
        acc = acc * ax + c.abs();
    }
    acc
}

/// Cauchy bound: every real root of `coeffs` (descending, `coeffs[0] != 0`)
/// satisfies `|root| <= 1 + max_i |coeffs[i] / coeffs[0]|`.
pub(crate) fn root_bound(coeffs: &[f64]) -> f64 {
    let lead = coeffs[0];
    let mut m = 0.0;
    for &c in &coeffs[1..] {
        let a = (c / lead).abs();
        if a > m {
            m = a;
        }
    }
    1.0 + m
}

/// Insert `r` into `out[0..n]` unless a near-duplicate is already present.
/// Returns the new count. Dedup tolerance is relative (1e-7).
pub(crate) fn push_unique(out: &mut [f64], n: usize, r: f64) -> usize {
    for &existing in &out[..n] {
        if (existing - r).abs() <= 1.0e-7 * (1.0 + r.abs()) {
            return n;
        }
    }
    if n < out.len() {
        out[n] = r;
        n + 1
    } else {
        n
    }
}

/// Ascending insertion sort of a tiny slice (n <= 4).
pub(crate) fn sort_small(s: &mut [f64]) {
    for i in 1..s.len() {
        let mut j = i;
        while j > 0 && s[j - 1] > s[j] {
            s.swap(j - 1, j);
            j -= 1;
        }
    }
}

/// Locate the single root of `coeffs` inside `[lo, hi]`, where `P(lo)` and
/// `P(hi)` are known to straddle zero. Bisection (guaranteed to converge) then
/// a few Newton steps for full-precision polish. `deriv` is `P'` (descending).
pub(crate) fn bisect_root(coeffs: &[f64], deriv: &[f64], lo: f64, hi: f64) -> f64 {
    let mut a = lo;
    let mut b = hi;
    let mut fa = poly_eval(coeffs, a);
    for _ in 0..100 {
        let mid = 0.5 * (a + b);
        let fm = poly_eval(coeffs, mid);
        if fm == 0.0 {
            return mid;
        }
        // Keep the sub-interval that still straddles the root.
        if (fm < 0.0) == (fa < 0.0) {
            a = mid;
            fa = fm;
        } else {
            b = mid;
        }
        if (b - a).abs() <= 1.0e-15 * (1.0 + mid.abs()) {
            break;
        }
    }
    let mut x = 0.5 * (a + b);
    for _ in 0..6 {
        let f = poly_eval(coeffs, x);
        let df = poly_eval(deriv, x);
        if df == 0.0 {
            break;
        }
        let step = f / df;
        x -= step;
        if step.abs() <= 1.0e-16 * (1.0 + x.abs()) {
            break;
        }
    }
    x
}

/// Collect the real roots of `coeffs` given its derivative `deriv` and the
/// **sorted** real critical points `crits` (roots of `deriv`). Writes them into
/// `out` (ascending) and returns the count.
///
/// The critical points split the real line into monotonic intervals; a simple
/// root lives wherever consecutive breakpoints straddle zero. Roots of even
/// multiplicity sit *at* a critical point (no sign change), so they are picked
/// up by a separate relative-residual test on each critical point.
pub(crate) fn collect_real_roots(
    coeffs: &[f64],
    deriv: &[f64],
    crits: &[f64],
    out: &mut [f64],
) -> usize {
    // Outer bound enclosing every root and every critical point.
    let mut m = root_bound(coeffs);
    for &c in crits {
        m = m.max(c.abs());
    }
    m += 1.0;

    // Breakpoints: -m, crits (sorted), +m.  crits.len() <= 3 for a quartic.
    let mut bp = [0.0f64; 5];
    let mut nb = 0;
    bp[nb] = -m;
    nb += 1;
    for &c in crits {
        bp[nb] = c;
        nb += 1;
    }
    bp[nb] = m;
    nb += 1;

    let mut n = 0usize;
    for i in 0..nb - 1 {
        let lo = bp[i];
        let hi = bp[i + 1];
        let flo = poly_eval(coeffs, lo);
        let fhi = poly_eval(coeffs, hi);
        if (flo < 0.0 && fhi > 0.0) || (flo > 0.0 && fhi < 0.0) {
            let r = bisect_root(coeffs, deriv, lo, hi);
            n = push_unique(out, n, r);
        }
    }
    // Even-multiplicity roots coincide with critical points.
    for &c in crits {
        let residual = poly_eval(coeffs, c).abs();
        let scale = poly_abs_scale(coeffs, c);
        if residual <= 1.0e-9 * scale + 1.0e-300 {
            n = push_unique(out, n, c);
        }
    }
    sort_small(&mut out[..n]);
    n
}

/// Real roots of `a x² + b x + c = 0`, ascending, into `out` (len >= 2).
/// Degenerates to the linear solve when `a ≈ 0`.
pub(crate) fn quadratic_real_roots(a: f64, b: f64, c: f64, out: &mut [f64]) -> usize {
    if a.abs() < 1.0e-12 * (1.0 + b.abs() + c.abs()) {
        if b.abs() < 1.0e-12 * (1.0 + c.abs()) {
            return 0;
        }
        out[0] = -c / b;
        return 1;
    }
    let disc = b * b - 4.0 * a * c;
    if disc < 0.0 {
        return 0;
    }
    if disc == 0.0 {
        out[0] = -b / (2.0 * a);
        return 1;
    }
    let sq = disc.sqrt();
    let r1 = (-b - sq) / (2.0 * a);
    let r2 = (-b + sq) / (2.0 * a);
    out[0] = r1.min(r2);
    out[1] = r1.max(r2);
    2
}

/// Real roots of `a x³ + b x² + c x + d = 0`, ascending, into `out` (len >= 3).
/// Degenerates to [`quadratic_real_roots`] when `a ≈ 0`.
pub(crate) fn cubic_real_roots(a: f64, b: f64, c: f64, d: f64, out: &mut [f64]) -> usize {
    if a.abs() < 1.0e-12 * (1.0 + b.abs() + c.abs() + d.abs()) {
        return quadratic_real_roots(b, c, d, out);
    }
    let coeffs = [a, b, c, d];
    let deriv = [3.0 * a, 2.0 * b, c];
    let mut crit = [0.0f64; 2];
    let nc = quadratic_real_roots(3.0 * a, 2.0 * b, c, &mut crit); // already ascending
    collect_real_roots(&coeffs, &deriv, &crit[..nc], out)
}

/// Real roots of `a x⁴ + b x³ + c x² + d x + e = 0`, ascending, into `out`
/// (len >= 4). Degenerates to [`cubic_real_roots`] when the leading coefficient
/// `a ≈ 0` (the promised lower-degree fallback).
#[allow(dead_code)]
pub(crate) fn quartic_real_roots(a: f64, b: f64, c: f64, d: f64, e: f64, out: &mut [f64]) -> usize {
    if a.abs() < 1.0e-12 * (1.0 + b.abs() + c.abs() + d.abs() + e.abs()) {
        return cubic_real_roots(b, c, d, e, out);
    }
    let coeffs = [a, b, c, d, e];
    let deriv = [4.0 * a, 3.0 * b, 2.0 * c, d];
    let mut crit = [0.0f64; 3];
    let nc = cubic_real_roots(4.0 * a, 3.0 * b, 2.0 * c, d, &mut crit);
    sort_small(&mut crit[..nc]);
    collect_real_roots(&coeffs, &deriv, &crit[..nc], out)
}

/// Smallest root `d > eps` of the quartic `coeffs[0] d⁴ + … + coeffs[4] = 0`,
/// or `INFINITY` if none.
///
/// (The tori below need one extra step beyond this — filtering the *spurious*
/// roots introduced by clearing the surface equation's square root — so they
/// call [`torus_distance`] rather than this directly; this is the plain
/// smallest-positive-root entry point, exercised by the solver's own tests.)
//
// Currently referenced only from the test suite (the tori use `torus_distance`,
// which additionally filters spurious roots); kept as the documented plain
// quartic entry point and to guard the solver's public-facing contract.
#[allow(dead_code)]
pub(crate) fn smallest_positive_quartic_root(coeffs: [f64; 5], eps: f64) -> f64 {
    let mut roots = [0.0f64; 4];
    let n = quartic_real_roots(
        coeffs[0], coeffs[1], coeffs[2], coeffs[3], coeffs[4], &mut roots,
    );
    let mut best = f64::INFINITY;
    for &r in &roots[..n] {
        if r > eps && r < best {
            best = r;
        }
    }
    best
}

// ── Torus surfaces (Torus{X,Y,Z}) ───────────────────────────────────────────
//
// **Field meaning, CORRECTED 2026-11-10 (gh:#701, #1386).** ~~`b` in-plane minor
// radius, `c` axial minor radius~~ — that was the opposite of OpenMC. A torus
// about an axis, centred at `(x0, y0, z0)`, has (OpenMC `include/openmc/surface.h`
// and `openmc/surface.py`):
//   - `a` — **major** radius (radius of the circle of revolution),
//   - `b` — minor radius/half-axis **parallel** to the axis of revolution,
//   - `c` — minor radius **perpendicular** to the axis of revolution (radial).
//
// Writing the radial pair as `(r1, r2)` and the axial displacement as `ax`, every
// torus shares one implicit "sense" form (`SurfaceZTorus::evaluate`,
// `src/surface.cpp:1198`, OpenMC `8f79d9d0`):
//
//     f = ax² / b² + (sqrt(r1² + r2²) − a)² / c² − 1
//
// The three structs differ only in which Cartesian axis is the torus axis:
//   Z-torus → radial pair (x, y), axial z;
//   X-torus → radial pair (y, z), axial x;
//   Y-torus → radial pair (x, z), axial y.

/// `SurfaceXTorus/YTorus/ZTorus::evaluate` (`src/surface.cpp:1073/1137/1198`),
/// same operation order.
#[inline]
pub(crate) fn torus_evaluate(r1: f64, r2: f64, ax: f64, a: f64, b: f64, c: f64) -> f64 {
    let t = (r1 * r1 + r2 * r2).sqrt() - a;
    (ax * ax) / (b * b) + (t * t) / (c * c) - 1.0
}

/// The unnormalised gradient direction of the `*Torus::normal` routines
/// (`src/surface.cpp:1101/1162/1223`): `f`'s gradient multiplied by
/// `g c² b² / 2` (`g = sqrt(r1² + r2²)`), returned as `(n_r1, n_r2, n_ax)`.
#[inline]
pub(crate) fn torus_gradient(r1: f64, r2: f64, ax: f64, a: f64, b: f64, c: f64) -> (f64, f64, f64) {
    let g = (r1 * r1 + r2 * r2).sqrt();
    (r1 * (g - a) * b * b, r2 * (g - a) * b * b, c * c * g * ax)
}

/// `torus_distance` (`src/surface.cpp:1007`): the quartic's coefficients, the
/// coincident `e = 0` rule, the `TORUS_TOL` cutoff and the interior-root check
/// are upstream's, in upstream's order. `(p1, p2, pax)` is the position relative
/// to the centre with `pax` along the axis of revolution; `a`, `b`, `c` as above.
///
/// The quartic is solved by `oqs::quartic_solver` exactly as upstream does
/// (Orellana & De Michele; `quartic_oqs.rs`). ~~This port solved it by derivative
/// bracketing and bisection ([`quartic_real_roots`]), so roots agreed to
/// round-off at best~~ **CORRECTED 2026-11-10** (gh:#1386).
#[allow(clippy::too_many_arguments)]
#[inline]
pub(crate) fn torus_distance(
    x1: f64,
    x2: f64,
    x3: f64,
    u1: f64,
    u2: f64,
    u3: f64,
    a: f64,
    b: f64,
    c: f64,
    coincident: bool,
) -> f64 {
    const TORUS_TOL: f64 = 1.0e-10; // include/openmc/constants.h:58
    // Coefficients for equation: (c2 t^2 + c1 t + c0)^2 = c2' t^2 + c1' t + c0'
    let dd = (c * c) / (b * b);
    let c2 = u1 * u1 + u2 * u2 + dd * u3 * u3;
    let c1 = 2.0 * (u1 * x1 + u2 * x2 + dd * u3 * x3);
    let c0 = x1 * x1 + x2 * x2 + dd * x3 * x3 + a * a - c * c;
    let four_a2 = 4.0 * a * a;
    let c2p = four_a2 * (u1 * u1 + u2 * u2);
    let c1p = 2.0 * four_a2 * (u1 * x1 + u2 * x2);
    let c0p = four_a2 * (x1 * x1 + x2 * x2);

    // a t^4 + b t^3 + c t^2 + d t + e = 0; when coincident, e is set to zero.
    let coeff_e = if coincident { 0.0 } else { c0 * c0 - c0p };
    let coeff_d = 2.0 * c0 * c1 - c1p;
    let coeff_c = c1 * c1 + 2.0 * c0 * c2 - c2p;
    let coeff_b = 2.0 * c1 * c2;
    let coeff_a = c2 * c2;

    // `oqs::quartic_solver` (src/external/quartic_solver.cpp), ported in quartic_oqs.rs.
    let roots = super::quartic_oqs::quartic_solver([coeff_e, coeff_d, coeff_c, coeff_b, coeff_a]);

    // Smallest positive real root; for a coincident point one root is ~0 and a
    // tolerance discards it.
    let mut distance = f64::INFINITY;
    let cutoff = if coincident { TORUS_TOL } else { 0.0 };
    for z in &roots {
        if z.im != 0.0 {
            continue;
        }
        let root = z.re;
        if root > cutoff && root < distance {
            // Avoid roots corresponding to internal surfaces.
            let s1 = x1 + u1 * root;
            let s2 = x2 + u2 * root;
            let s3 = x3 + u3 * root;
            let check = dd * s3 * s3 + s1 * s1 + s2 * s2 + a * a - c * c;
            if check >= 0.0 {
                distance = root;
            }
        }
    }
    distance
}

#[derive(Debug, Clone)]
/// Torus about the **Z** axis, centred at `(x0, y0, z0)`.
///
/// Radial pair `(x, y)`, axial `z`:
/// `(z−z0)² / b² + (sqrt((x−x0)² + (y−y0)²) − a)² / c² − 1 = 0`.
///
/// `a` = major radius, `b` = minor radius **along the axis**, `c` = minor radius
/// **perpendicular** to the axis (all cm) — OpenMC's meaning since 2026-11-10
/// (gh:#701; it was the other way round). Maps to OpenMC `SurfaceZTorus`
/// (`src/surface.cpp:1198`).
pub struct ZTorus {
    pub x0: f64,
    pub y0: f64,
    pub z0: f64,
    pub a: f64,
    pub b: f64,
    pub c: f64,
    pub bc: BoundaryType,
}

impl Surface for ZTorus {
    #[inline]
    fn evaluate(&self, r: Position) -> f64 {
        torus_evaluate(r.x - self.x0, r.y - self.y0, r.z - self.z0, self.a, self.b, self.c)
    }
    #[inline]
    fn normal(&self, r: Position) -> Direction {
        let (g1, g2, gax) =
            torus_gradient(r.x - self.x0, r.y - self.y0, r.z - self.z0, self.a, self.b, self.c);
        Direction::from_unnormalised(g1, g2, gax)
    }
    #[inline]
    fn distance(&self, r: Position, u: Direction, coincident: bool) -> f64 {
        torus_distance(
            r.x - self.x0,
            r.y - self.y0,
            r.z - self.z0,
            u.u,
            u.v,
            u.w,
            self.a,
            self.b,
            self.c,
            coincident,
        )
    }
}

#[derive(Debug, Clone)]
/// Torus about the **X** axis, centred at `(x0, y0, z0)`.
///
/// Radial pair `(y, z)`, axial `x`; `b` along the axis, `c` perpendicular (see
/// [`ZTorus`]). Maps to OpenMC `SurfaceXTorus` (`src/surface.cpp:1073`).
pub struct XTorus {
    pub x0: f64,
    pub y0: f64,
    pub z0: f64,
    pub a: f64,
    pub b: f64,
    pub c: f64,
    pub bc: BoundaryType,
}

impl Surface for XTorus {
    #[inline]
    fn evaluate(&self, r: Position) -> f64 {
        // radial pair (y, z), axial x
        torus_evaluate(r.y - self.y0, r.z - self.z0, r.x - self.x0, self.a, self.b, self.c)
    }
    #[inline]
    fn normal(&self, r: Position) -> Direction {
        let (g1, g2, gax) =
            torus_gradient(r.y - self.y0, r.z - self.z0, r.x - self.x0, self.a, self.b, self.c);
        // (g1, g2) belong to (y, z); gax to x → reorder to (x, y, z).
        Direction::from_unnormalised(gax, g1, g2)
    }
    #[inline]
    fn distance(&self, r: Position, u: Direction, coincident: bool) -> f64 {
        torus_distance(
            r.y - self.y0,
            r.z - self.z0,
            r.x - self.x0,
            u.v,
            u.w,
            u.u,
            self.a,
            self.b,
            self.c,
            coincident,
        )
    }
}

#[derive(Debug, Clone)]
/// Torus about the **Y** axis, centred at `(x0, y0, z0)`.
///
/// Radial pair `(x, z)`, axial `y`; `b` along the axis, `c` perpendicular (see
/// [`ZTorus`]). Maps to OpenMC `SurfaceYTorus` (`src/surface.cpp:1137`).
pub struct YTorus {
    pub x0: f64,
    pub y0: f64,
    pub z0: f64,
    pub a: f64,
    pub b: f64,
    pub c: f64,
    pub bc: BoundaryType,
}

impl Surface for YTorus {
    #[inline]
    fn evaluate(&self, r: Position) -> f64 {
        // radial pair (x, z), axial y
        torus_evaluate(r.x - self.x0, r.z - self.z0, r.y - self.y0, self.a, self.b, self.c)
    }
    #[inline]
    fn normal(&self, r: Position) -> Direction {
        let (g1, g2, gax) =
            torus_gradient(r.x - self.x0, r.z - self.z0, r.y - self.y0, self.a, self.b, self.c);
        // (g1, g2) belong to (x, z); gax to y → reorder to (x, y, z).
        Direction::from_unnormalised(g1, gax, g2)
    }
    #[inline]
    fn distance(&self, r: Position, u: Direction, coincident: bool) -> f64 {
        torus_distance(
            r.x - self.x0,
            r.z - self.z0,
            r.y - self.y0,
            u.u,
            u.w,
            u.v,
            self.a,
            self.b,
            self.c,
            coincident,
        )
    }
}
