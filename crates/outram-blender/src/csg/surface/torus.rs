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
// A torus about an axis, centred at `(x0, y0, z0)`, has three radii:
//   - `a` — **major** radius (radius of the circle of revolution),
//   - `b` — **in-plane minor** radius (tube radius in the radial direction),
//   - `c` — **axial minor** radius (tube radius along the axis).
//
// Writing the radial pair as `(r1, r2)` and the axial displacement as `ax`, and
// `R⊥ = sqrt(r1² + r2²)`, every torus shares one implicit "sense" form
// (OpenMC `SurfaceZTorus`/`SurfaceXTorus`/`SurfaceYTorus`, `src/surface.cpp`):
//
//     f = ((R⊥ − a) / b)² + (ax / c)² − 1
//
// The three structs differ only in which Cartesian axis is the torus axis:
//   Z-torus → radial pair (x, y), axial z;
//   X-torus → radial pair (y, z), axial x;
//   Y-torus → radial pair (x, z), axial y.

/// Torus sense in the axis-independent `(r1, r2, ax)` frame (radial pair +
/// axial). Negative inside the tube, positive outside, zero on the surface.
#[inline]
pub(crate) fn torus_evaluate(r1: f64, r2: f64, ax: f64, a: f64, b: f64, c: f64) -> f64 {
    let rperp = (r1 * r1 + r2 * r2).sqrt();
    let t = (rperp - a) / b;
    let s = ax / c;
    t * t + s * s - 1.0
}

/// Gradient of the torus sense in the `(r1, r2, ax)` frame (un-normalised; the
/// caller maps the components back to Cartesian and normalises).
///
/// From `f = (R⊥ − a)²/b² + ax²/c² − 1` with `R⊥ = sqrt(r1² + r2²)`:
///   ∂f/∂r1 = 2(R⊥ − a)/b² · r1/R⊥,  ∂f/∂r2 = 2(R⊥ − a)/b² · r2/R⊥,
///   ∂f/∂ax = 2·ax/c².
/// The shared factor 2 is dropped (normalisation removes it).
#[inline]
pub(crate) fn torus_gradient(r1: f64, r2: f64, ax: f64, a: f64, b: f64, c: f64) -> (f64, f64, f64) {
    let rperp = (r1 * r1 + r2 * r2).sqrt();
    let coeff = (rperp - a) / (b * b * rperp);
    (coeff * r1, coeff * r2, ax / (c * c))
}

/// Smallest positive ray-torus distance in the `(r1, r2, ax)` frame.
///
/// **Deriving the quartic.** Substitute the ray `P(t) = start + t·u` into the
/// cleared surface equation. `f = 0` multiplied through by `b²` and rearranged
/// isolates the square root:
///
/// ```text
/// ρ²(t) + a² − b² + (b²/c²)·ax²(t)  =  2a·R⊥(t)          (call the LHS G(t))
/// ```
///
/// where `ρ²(t) = r1² + r2²` is quadratic in `t`. Squaring to remove `R⊥`
/// (`R⊥² = ρ²`) gives the quartic
///
/// ```text
/// F(t) = G(t)² − 4a²·ρ²(t) = 0.
/// ```
///
/// With `ρ²(t) = α t² + 2β t + γ`, `ax²(t) = ax_α t² + 2 ax_β t + ax_γ`,
/// `k = b²/c²`, and `G(t) = Ga t² + 2Gb t + Gc` where
///
/// ```text
/// Ga = α + k·ax_α,  Gb = β + k·ax_β,  Gc = γ + a² − b² + k·ax_γ,
/// ```
///
/// expanding `G² − 4a²ρ²` yields the coefficients below.
///
/// **Spurious roots.** Squaring also admits the branch `G(t) = −2a·R⊥(t) ≤ 0`,
/// which is not a geometric intersection. A genuine hit has `G(t) = +2a·R⊥ ≥ 0`,
/// so any real root with `G(t) < 0` is discarded. (For a ring torus, `a > b`,
/// this branch is unreachable and the filter never fires; it is exact cover for
/// horn/spindle tori where `a ≤ b`.)
///
/// `coincident` (`start` sits on the surface) discards roots at/below a small
/// positive `eps` so round-off cannot report `d ≈ 0`.
#[allow(clippy::too_many_arguments)]
#[inline]
pub(crate) fn torus_distance(
    p1: f64,
    p2: f64,
    pax: f64,
    u1: f64,
    u2: f64,
    uax: f64,
    a: f64,
    b: f64,
    c: f64,
    coincident: bool,
) -> f64 {
    let k = (b * b) / (c * c);
    let alpha = u1 * u1 + u2 * u2;
    let beta = p1 * u1 + p2 * u2;
    let gamma = p1 * p1 + p2 * p2;
    let ax_alpha = uax * uax;
    let ax_beta = pax * uax;
    let ax_gamma = pax * pax;

    let ga = alpha + k * ax_alpha;
    let gb = beta + k * ax_beta;
    let gc = gamma + a * a - b * b + k * ax_gamma;
    let a2 = a * a;

    // F(t) = (Ga t² + 2Gb t + Gc)² − 4a²(α t² + 2β t + γ)
    let c4 = ga * ga;
    let c3 = 4.0 * ga * gb;
    let c2 = 4.0 * gb * gb + 2.0 * ga * gc - 4.0 * a2 * alpha;
    let c1 = 4.0 * gb * gc - 8.0 * a2 * beta;
    let c0 = gc * gc - 4.0 * a2 * gamma;

    let mut roots = [0.0f64; 4];
    let n = quartic_real_roots(c4, c3, c2, c1, c0, &mut roots);

    let eps = if coincident { 1.0e-8 } else { 1.0e-10 };
    let mut best = f64::INFINITY;
    for &t in &roots[..n] {
        if t <= eps {
            continue;
        }
        // Reject the spurious branch G(t) = −2a·R⊥.
        let gt = ga * t * t + 2.0 * gb * t + gc;
        let tol = 1.0e-9 * (1.0 + ga * t * t + (2.0 * gb * t).abs() + gc.abs());
        if gt >= -tol && t < best {
            best = t;
        }
    }
    best
}

#[derive(Debug, Clone)]
/// Torus about the **Z** axis, centred at `(x0, y0, z0)`.
///
/// Radial pair `(x, y)`, axial `z`:
///   `((sqrt((x−x0)² + (y−y0)²) − a) / b)² + ((z−z0) / c)² − 1 = 0`.
///
/// `a` = major radius, `b` = in-plane minor radius, `c` = axial minor radius
/// (all cm). Maps to OpenMC `SurfaceZTorus` (`src/surface.cpp`).
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
        torus_evaluate(
            r.x - self.x0,
            r.y - self.y0,
            r.z - self.z0,
            self.a,
            self.b,
            self.c,
        )
    }
    #[inline]
    fn normal(&self, r: Position) -> Direction {
        let (g1, g2, gax) = torus_gradient(
            r.x - self.x0,
            r.y - self.y0,
            r.z - self.z0,
            self.a,
            self.b,
            self.c,
        );
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
/// Radial pair `(y, z)`, axial `x`:
///   `((sqrt((y−y0)² + (z−z0)²) − a) / b)² + ((x−x0) / c)² − 1 = 0`.
/// X-axis twin of [`ZTorus`]; maps to OpenMC `SurfaceXTorus` (`src/surface.cpp`).
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
        torus_evaluate(
            r.y - self.y0,
            r.z - self.z0,
            r.x - self.x0,
            self.a,
            self.b,
            self.c,
        )
    }
    #[inline]
    fn normal(&self, r: Position) -> Direction {
        let (g1, g2, gax) = torus_gradient(
            r.y - self.y0,
            r.z - self.z0,
            r.x - self.x0,
            self.a,
            self.b,
            self.c,
        );
        // (g1, g2) are ∂f/∂y, ∂f/∂z; gax is ∂f/∂x → reorder to (x, y, z).
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
/// Radial pair `(x, z)`, axial `y`:
///   `((sqrt((x−x0)² + (z−z0)²) − a) / b)² + ((y−y0) / c)² − 1 = 0`.
/// Y-axis twin of [`ZTorus`]; maps to OpenMC `SurfaceYTorus` (`src/surface.cpp`).
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
        torus_evaluate(
            r.x - self.x0,
            r.z - self.z0,
            r.y - self.y0,
            self.a,
            self.b,
            self.c,
        )
    }
    #[inline]
    fn normal(&self, r: Position) -> Direction {
        let (g1, g2, gax) = torus_gradient(
            r.x - self.x0,
            r.z - self.z0,
            r.y - self.y0,
            self.a,
            self.b,
            self.c,
        );
        // (g1, g2) are ∂f/∂x, ∂f/∂z; gax is ∂f/∂y → reorder to (x, y, z).
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
