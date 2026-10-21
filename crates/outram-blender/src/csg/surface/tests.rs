// SPDX-License-Identifier: GPL-3.0-only
// Copyright (C) 2026 OUTRAM PARK contributors
//
// Ported from OpenMC (https://github.com/openmc-dev/openmc, MIT licence,
// Copyright (c) 2011-2026 Massachusetts Institute of Technology, UChicago
// Argonne LLC and OpenMC contributors; see LICENSE.openmc):
//   src/surface.cpp (behaviour pinned against upstream)
// Unit tests of the surface kernels.
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

//! Surface unit tests (split out of the surface module, 2026-10-02).

use super::torus::*;
use super::*;
use crate::csg::position::{Direction, Position};

const TOL: f64 = 1.0e-9;
fn close(a: f64, b: f64) -> bool {
    (a - b).abs() < TOL
}

// ── general Plane ──────────────────────────────────────────────────────────
#[test]
fn plane_sense_distance_normal() {
    // Plane x + y = 0 (a=1,b=1,c=0,d=0); normal (1,1,0)/√2.
    let p = Plane {
        a: 1.0,
        b: 1.0,
        c: 0.0,
        d: 0.0,
        bc: BoundaryType::Transmissive,
    };
    assert!(p.evaluate(Position::new(1.0, 0.0, 0.0)) > 0.0); // outside (+)
    assert!(p.evaluate(Position::new(-1.0, 0.0, 0.0)) < 0.0); // inside (−)
    let n = p.normal(Position::ZERO);
    assert!(close(n.u, 1.0 / 2f64.sqrt()) && close(n.v, 1.0 / 2f64.sqrt()) && close(n.w, 0.0));
    // From (1,0,0) heading −x, reach x+y=0 at x=0 → distance 1.
    let d = p.distance(
        Position::new(1.0, 0.0, 0.0),
        Direction::new(-1.0, 0.0, 0.0),
        false,
    );
    assert!(close(d, 1.0), "plane distance {d}");
    // Parallel ray never hits.
    let d2 = p.distance(
        Position::new(1.0, 0.0, 0.0),
        Direction::new(1.0, -1.0, 0.0),
        false,
    );
    assert_eq!(d2, f64::INFINITY);
}

#[test]
fn plane_reflect() {
    let p = Plane {
        a: 1.0,
        b: 0.0,
        c: 0.0,
        d: 0.0,
        bc: BoundaryType::Reflective,
    }; // x=0
    let refl = p.reflect(Position::ZERO, Direction::new(0.6, 0.8, 0.0));
    assert!(close(refl.u, -0.6) && close(refl.v, 0.8) && close(refl.w, 0.0));
}

// ── X / Y cylinders ────────────────────────────────────────────────────────
#[test]
fn xcylinder_distance_and_axis_miss() {
    let cyl = XCylinder {
        y0: 0.0,
        z0: 0.0,
        r: 2.0,
        bc: BoundaryType::Transmissive,
    };
    assert!(cyl.evaluate(Position::new(9.0, 0.0, 0.0)) < 0.0); // on axis → inside
    assert!(cyl.evaluate(Position::new(0.0, 5.0, 0.0)) > 0.0); // outside
                                                               // From (0,-5,0) heading +y: hit near wall at y=-2 → distance 3.
    let d = cyl.distance(
        Position::new(0.0, -5.0, 0.0),
        Direction::new(0.0, 1.0, 0.0),
        false,
    );
    assert!(close(d, 3.0), "xcyl distance {d}");
    // Axis-parallel ray never hits.
    let d2 = cyl.distance(
        Position::new(0.0, 0.0, 0.0),
        Direction::new(1.0, 0.0, 0.0),
        false,
    );
    assert_eq!(d2, f64::INFINITY);
}

#[test]
fn ycylinder_distance() {
    let cyl = YCylinder {
        x0: 0.0,
        z0: 0.0,
        r: 2.0,
        bc: BoundaryType::Transmissive,
    };
    // From (-5,0,0) heading +x: hit near wall at x=-2 → distance 3.
    let d = cyl.distance(
        Position::new(-5.0, 0.0, 0.0),
        Direction::new(1.0, 0.0, 0.0),
        false,
    );
    assert!(close(d, 3.0), "ycyl distance {d}");
    assert!(cyl.evaluate(Position::new(0.0, 9.0, 0.0)) < 0.0); // on axis → inside
}

// ── cones (45° double-napped, r_sq = 1) ──────────────────────────────────────
#[test]
fn zcone_sense_and_distance() {
    let cone = ZCone {
        x0: 0.0,
        y0: 0.0,
        z0: 0.0,
        r_sq: 1.0,
        bc: BoundaryType::Transmissive,
    };
    assert!(cone.evaluate(Position::new(0.0, 0.0, 1.0)) < 0.0); // inside the nap
    assert!(cone.evaluate(Position::new(2.0, 0.0, 1.0)) > 0.0); // outside
    assert!(close(cone.evaluate(Position::new(1.0, 0.0, 1.0)), 0.0)); // on surface
                                                                      // From (2,0,1) heading −x: near nap at x=1 → distance 1 (far nap at x=−1 is 3).
    let d = cone.distance(
        Position::new(2.0, 0.0, 1.0),
        Direction::new(-1.0, 0.0, 0.0),
        false,
    );
    assert!(close(d, 1.0), "zcone distance {d}");
}

#[test]
fn zcone_coincident_skips_zero_root() {
    let cone = ZCone {
        x0: 0.0,
        y0: 0.0,
        z0: 0.0,
        r_sq: 1.0,
        bc: BoundaryType::Reflective,
    };
    // On the surface at (1,0,1), heading −x: skip d≈0, next crossing (far nap) at x=−1 → 2.
    let d = cone.distance(
        Position::new(1.0, 0.0, 1.0),
        Direction::new(-1.0, 0.0, 0.0),
        true,
    );
    assert!(close(d, 2.0), "coincident zcone distance {d}");
}

#[test]
fn xcone_and_ycone_distance() {
    let xc = XCone {
        x0: 0.0,
        y0: 0.0,
        z0: 0.0,
        r_sq: 1.0,
        bc: BoundaryType::Transmissive,
    };
    // From (1,2,0) heading −y: near nap at y=1 (x²=y²) → distance 1.
    let d = xc.distance(
        Position::new(1.0, 2.0, 0.0),
        Direction::new(0.0, -1.0, 0.0),
        false,
    );
    assert!(close(d, 1.0), "xcone distance {d}");
    assert!(xc.evaluate(Position::new(1.0, 0.0, 0.0)) < 0.0); // on axis → inside

    let yc = YCone {
        x0: 0.0,
        y0: 0.0,
        z0: 0.0,
        r_sq: 1.0,
        bc: BoundaryType::Transmissive,
    };
    // From (2,1,0) heading −x: near nap at x=1 → distance 1.
    let d2 = yc.distance(
        Position::new(2.0, 1.0, 0.0),
        Direction::new(-1.0, 0.0, 0.0),
        false,
    );
    assert!(close(d2, 1.0), "ycone distance {d2}");
    assert!(yc.evaluate(Position::new(0.0, 1.0, 0.0)) < 0.0); // on axis → inside
}

// ── general Quadric reproduces a sphere ─────────────────────────────────────
#[test]
fn quadric_matches_sphere() {
    // x² + y² + z² − 4 = 0  ⇔  Sphere(center 0, r=2).
    let q = Quadric {
        a: 1.0,
        b: 1.0,
        c: 1.0,
        d: 0.0,
        e: 0.0,
        f: 0.0,
        g: 0.0,
        h: 0.0,
        j: 0.0,
        k: -4.0,
        bc: BoundaryType::Transmissive,
    };
    let s = Sphere {
        x0: 0.0,
        y0: 0.0,
        z0: 0.0,
        r: 2.0,
        bc: BoundaryType::Transmissive,
    };
    for p in [
        Position::new(0.0, 0.0, 0.0),
        Position::new(3.0, 1.0, -2.0),
        Position::new(2.0, 0.0, 0.0),
    ] {
        assert!(
            close(q.evaluate(p), s.evaluate(p)),
            "quadric vs sphere eval at {:?}",
            (p.x, p.y, p.z)
        );
    }
    // Ray from (0,0,−5) heading +z: both give distance 3 (hit at z=−2).
    let r0 = Position::new(0.0, 0.0, -5.0);
    let u = Direction::new(0.0, 0.0, 1.0);
    assert!(close(q.distance(r0, u, false), s.distance(r0, u, false)));
    assert!(close(q.distance(r0, u, false), 3.0));
    // Normals agree (up to sign/normalisation) at a surface point.
    let sp = Position::new(2.0, 0.0, 0.0);
    let nq = q.normal(sp);
    let ns = s.normal(sp);
    assert!(close(nq.u, ns.u) && close(nq.v, ns.v) && close(nq.w, ns.w));
}

// ── SurfaceKind enum dispatch reaches the new variants ──────────────────────
#[test]
fn surfacekind_dispatch_new_variants() {
    let sk = SurfaceKind::Quadric(Quadric {
        a: 1.0,
        b: 1.0,
        c: 1.0,
        d: 0.0,
        e: 0.0,
        f: 0.0,
        g: 0.0,
        h: 0.0,
        j: 0.0,
        k: -4.0,
        bc: BoundaryType::Vacuum,
    });
    assert!(close(
        sk.distance(
            Position::new(0.0, 0.0, -5.0),
            Direction::new(0.0, 0.0, 1.0),
            false
        ),
        3.0
    ));
    let ur = Direction::new(1.0, 0.0, 0.0);
    assert!(sk.sense(Position::new(3.0, 0.0, 0.0), ur)); // outside r=2 sphere
    assert_eq!(sk.bc(), BoundaryType::Vacuum);
}

/// Is `target` (approximately) present among the first `n` roots?
fn contains(roots: &[f64], n: usize, target: f64) -> bool {
    roots[..n].iter().any(|&r| (r - target).abs() < 1.0e-6)
}

// ── Quartic solver, tested in isolation against polynomials with known roots.
//
// Methodology: build each polynomial from known factors, hand-expand its
// descending coefficients, feed them to `quartic_real_roots`, and assert the
// returned real roots (ascending) match the analytic factor roots. This
// exercises the solver *before* any torus geometry trusts it.

/// Four distinct simple roots.
/// Methodology: (d−1)(d−2)(d−3)(d−4) = d⁴ − 10d³ + 35d² − 50d + 24.
/// Result: solver returns exactly [1, 2, 3, 4].
#[test]
fn quartic_four_distinct_roots() {
    let mut r = [0.0; 4];
    let n = quartic_real_roots(1.0, -10.0, 35.0, -50.0, 24.0, &mut r);
    assert_eq!(n, 4, "expected 4 roots, got {n}: {:?}", &r[..n]);
    for (i, want) in [1.0, 2.0, 3.0, 4.0].into_iter().enumerate() {
        assert!(close(r[i], want), "root {i} = {} want {want}", r[i]);
    }
}

/// A double root plus two simple roots.
/// Methodology: (d−2)²(d−4)(d−6) = d⁴ − 14d³ + 68d² − 136d + 96.
/// Result: the three distinct real roots {2, 4, 6} are all found (the
/// double root at 2 is reported once).
#[test]
fn quartic_double_root() {
    let mut r = [0.0; 4];
    let n = quartic_real_roots(1.0, -14.0, 68.0, -136.0, 96.0, &mut r);
    assert!(contains(&r, n, 2.0), "missing double root 2: {:?}", &r[..n]);
    assert!(contains(&r, n, 4.0), "missing root 4: {:?}", &r[..n]);
    assert!(contains(&r, n, 6.0), "missing root 6: {:?}", &r[..n]);
}

/// No real roots.
/// Methodology: (d²+1)(d²+4) = d⁴ + 5d² + 4, whose roots are all imaginary.
/// Result: solver returns 0 real roots.
#[test]
fn quartic_no_real_roots() {
    let mut r = [0.0; 4];
    let n = quartic_real_roots(1.0, 0.0, 5.0, 0.0, 4.0, &mut r);
    assert_eq!(n, 0, "expected 0 real roots, got {n}: {:?}", &r[..n]);
}

/// Degenerate leading coefficient falls back to the cubic path.
/// Methodology: 0·d⁴ + (d−1)(d−2)(d−3) = 0·d⁴ + d³ − 6d² + 11d − 6.
/// Result: solver returns the cubic's roots [1, 2, 3].
#[test]
fn quartic_degenerate_leading_reduces_to_cubic() {
    let mut r = [0.0; 4];
    let n = quartic_real_roots(0.0, 1.0, -6.0, 11.0, -6.0, &mut r);
    assert_eq!(n, 3, "expected 3 roots, got {n}: {:?}", &r[..n]);
    for (i, want) in [1.0, 2.0, 3.0].into_iter().enumerate() {
        assert!(close(r[i], want), "root {i} = {} want {want}", r[i]);
    }
}

/// `smallest_positive_quartic_root` honours the `eps` gate.
/// Methodology: on (d−1)(d−2)(d−3)(d−4), eps=1e−10 skips nothing (→1);
/// eps=1.5 skips the root at 1 (→2). A no-real-root quartic → INFINITY.
#[test]
fn smallest_positive_quartic_root_gate() {
    let coeffs = [1.0, -10.0, 35.0, -50.0, 24.0];
    assert!(close(smallest_positive_quartic_root(coeffs, 1.0e-10), 1.0));
    assert!(close(smallest_positive_quartic_root(coeffs, 1.5), 2.0));
    assert_eq!(
        smallest_positive_quartic_root([1.0, 0.0, 5.0, 0.0, 4.0], 1.0e-10),
        f64::INFINITY
    );
}

// ── Z-torus (a=3, b=1, c=1): a circular tube of radius 1 whose centre-line
// is the circle x²+y²=9 in the z=0 plane. All reference numbers below are
// hand-derived from the implicit form ((R⊥−3))² + z² = 1 on R⊥=|x|, z lines.

/// Sense function at hand-verified points.
/// Methodology: evaluate `((R⊥−3)/1)² + (z/1)² − 1`.
/// Results (analytic): on-surface (4,0,0),(2,0,0),(3,0,±1) → 0; the tube
/// centre-line point (3,0,0) → −1 (deepest inside); (3,0,2) → 3, (5,0,0) → 3,
/// the central hole (0,0,0) → 8 (all outside).
/// Note: (3,0,0) is the tube *centre*, NOT on the surface — it evaluates to −1.
#[test]
fn ztorus_evaluate_analytic() {
    let t = ZTorus {
        x0: 0.0,
        y0: 0.0,
        z0: 0.0,
        a: 3.0,
        b: 1.0,
        c: 1.0,
        bc: BoundaryType::Transmissive,
    };
    assert!(close(t.evaluate(Position::new(4.0, 0.0, 0.0)), 0.0));
    assert!(close(t.evaluate(Position::new(2.0, 0.0, 0.0)), 0.0));
    assert!(close(t.evaluate(Position::new(3.0, 0.0, 1.0)), 0.0));
    assert!(close(t.evaluate(Position::new(3.0, 0.0, -1.0)), 0.0));
    assert!(close(t.evaluate(Position::new(3.0, 0.0, 0.0)), -1.0)); // tube centre, inside
    assert!(close(t.evaluate(Position::new(3.0, 0.0, 2.0)), 3.0)); // outside (above tube)
    assert!(close(t.evaluate(Position::new(5.0, 0.0, 0.0)), 3.0)); // outside (radially)
    assert!(close(t.evaluate(Position::new(0.0, 0.0, 0.0)), 8.0)); // central hole
}

/// Ray along the x-axis crosses the tube four times.
/// Methodology: from (10,0,0) heading −x, the tube walls on the x-axis are at
/// x = 4, 2, −2, −4 (where (|x|−3)² = 1), i.e. distances t = 6, 8, 12, 14.
/// The derived quartic is t⁴ − 40t³ + 580t² − 3600t + 8064 = 0.
/// Results (analytic): solver roots = {6, 8, 12, 14}; nearest distance = 6.
#[test]
fn ztorus_distance_four_roots() {
    // The quartic itself has exactly the four analytic roots.
    let mut r = [0.0; 4];
    let n = quartic_real_roots(1.0, -40.0, 580.0, -3600.0, 8064.0, &mut r);
    assert_eq!(n, 4, "roots: {:?}", &r[..n]);
    for (i, want) in [6.0, 8.0, 12.0, 14.0].into_iter().enumerate() {
        assert!(close(r[i], want), "root {i} = {} want {want}", r[i]);
    }
    // The surface's distance method returns the nearest crossing.
    let t = ZTorus {
        x0: 0.0,
        y0: 0.0,
        z0: 0.0,
        a: 3.0,
        b: 1.0,
        c: 1.0,
        bc: BoundaryType::Transmissive,
    };
    let d = t.distance(
        Position::new(10.0, 0.0, 0.0),
        Direction::new(-1.0, 0.0, 0.0),
        false,
    );
    assert!(close(d, 6.0), "nearest distance {d}");
}

/// Coincident start skips the d≈0 root.
/// Methodology: start ON the surface at (4,0,0) heading −x; crossings at
/// x = 4,2,−2,−4 ⇒ t = 0,2,6,8. With `coincident = true` the t≈0 root is
/// discarded. Result (analytic): nearest returned distance = 2.
#[test]
fn ztorus_coincident_skips_zero_root() {
    let t = ZTorus {
        x0: 0.0,
        y0: 0.0,
        z0: 0.0,
        a: 3.0,
        b: 1.0,
        c: 1.0,
        bc: BoundaryType::Reflective,
    };
    let d = t.distance(
        Position::new(4.0, 0.0, 0.0),
        Direction::new(-1.0, 0.0, 0.0),
        true,
    );
    assert!(close(d, 2.0), "coincident distance {d}");
}

/// Outward normals (gradient of the implicit form) at hand-verified points.
/// Methodology / results (analytic): at the outer equator (4,0,0) the normal
/// is +x; at the inner equator (2,0,0) it points toward the axis, −x; at the
/// top of the tube (3,0,1) it is +z.
#[test]
fn ztorus_normal_analytic() {
    let t = ZTorus {
        x0: 0.0,
        y0: 0.0,
        z0: 0.0,
        a: 3.0,
        b: 1.0,
        c: 1.0,
        bc: BoundaryType::Transmissive,
    };
    let n1 = t.normal(Position::new(4.0, 0.0, 0.0));
    assert!(
        close(n1.u, 1.0) && close(n1.v, 0.0) && close(n1.w, 0.0),
        "outer eq {:?}",
        (n1.u, n1.v, n1.w)
    );
    let n2 = t.normal(Position::new(2.0, 0.0, 0.0));
    assert!(
        close(n2.u, -1.0) && close(n2.v, 0.0) && close(n2.w, 0.0),
        "inner eq {:?}",
        (n2.u, n2.v, n2.w)
    );
    let n3 = t.normal(Position::new(3.0, 0.0, 1.0));
    assert!(
        close(n3.u, 0.0) && close(n3.v, 0.0) && close(n3.w, 1.0),
        "top {:?}",
        (n3.u, n3.v, n3.w)
    );
}

/// X-torus (axis = x, major circle in the y-z plane), a=3,b=1,c=1.
/// Methodology: on-surface point (0,4,0) → eval 0; a ray from (0,0,10)
/// heading −z crosses the tube on the z-axis at z = 4,2,−2,−4 ⇒ nearest
/// distance 6; the outer-equator normal at (0,4,0) is +y.
#[test]
fn xtorus_evaluate_distance_normal() {
    let t = XTorus {
        x0: 0.0,
        y0: 0.0,
        z0: 0.0,
        a: 3.0,
        b: 1.0,
        c: 1.0,
        bc: BoundaryType::Transmissive,
    };
    assert!(close(t.evaluate(Position::new(0.0, 4.0, 0.0)), 0.0));
    assert!(close(t.evaluate(Position::new(0.0, 3.0, 0.0)), -1.0)); // tube centre
    let d = t.distance(
        Position::new(0.0, 0.0, 10.0),
        Direction::new(0.0, 0.0, -1.0),
        false,
    );
    assert!(close(d, 6.0), "xtorus distance {d}");
    let n = t.normal(Position::new(0.0, 4.0, 0.0));
    assert!(
        close(n.u, 0.0) && close(n.v, 1.0) && close(n.w, 0.0),
        "xtorus normal {:?}",
        (n.u, n.v, n.w)
    );
}

/// Y-torus (axis = y, major circle in the x-z plane), a=3,b=1,c=1.
/// Methodology: on-surface point (4,0,0) → eval 0; a ray from (0,0,10)
/// heading −z crosses the tube on the z-axis at z = 4,2,−2,−4 ⇒ nearest
/// distance 6; the outer-equator normal at (4,0,0) is +x.
#[test]
fn ytorus_evaluate_distance_normal() {
    let t = YTorus {
        x0: 0.0,
        y0: 0.0,
        z0: 0.0,
        a: 3.0,
        b: 1.0,
        c: 1.0,
        bc: BoundaryType::Transmissive,
    };
    assert!(close(t.evaluate(Position::new(4.0, 0.0, 0.0)), 0.0));
    assert!(close(t.evaluate(Position::new(3.0, 0.0, 0.0)), -1.0)); // tube centre
    let d = t.distance(
        Position::new(0.0, 0.0, 10.0),
        Direction::new(0.0, 0.0, -1.0),
        false,
    );
    assert!(close(d, 6.0), "ytorus distance {d}");
    let n = t.normal(Position::new(4.0, 0.0, 0.0));
    assert!(
        close(n.u, 1.0) && close(n.v, 0.0) && close(n.w, 0.0),
        "ytorus normal {:?}",
        (n.u, n.v, n.w)
    );
}

/// A translated (off-origin) Z-torus tracks the centre.
/// Methodology: centre (5,0,0), a=3,b=1,c=1. From (15,0,0) heading −x the
/// translated start is X=10, so distances are again {6,8,12,14}; nearest 6.
#[test]
fn ztorus_translated_center() {
    let t = ZTorus {
        x0: 5.0,
        y0: 0.0,
        z0: 0.0,
        a: 3.0,
        b: 1.0,
        c: 1.0,
        bc: BoundaryType::Transmissive,
    };
    let d = t.distance(
        Position::new(15.0, 0.0, 0.0),
        Direction::new(-1.0, 0.0, 0.0),
        false,
    );
    assert!(close(d, 6.0), "translated ztorus distance {d}");
}

/// The enum-dispatched `SurfaceKind` path reaches the torus variants.
/// Methodology: wrap the a=3,b=1,c=1 Z-torus in `SurfaceKind::ZTorus`; the
/// same x-axis ray gives distance 6, sense at the central hole (0,0,0) is
/// `true` (outside), and the boundary condition round-trips.
#[test]
fn surfacekind_torus_dispatch() {
    let sk = SurfaceKind::ZTorus(ZTorus {
        x0: 0.0,
        y0: 0.0,
        z0: 0.0,
        a: 3.0,
        b: 1.0,
        c: 1.0,
        bc: BoundaryType::Vacuum,
    });
    let d = sk.distance(
        Position::new(10.0, 0.0, 0.0),
        Direction::new(-1.0, 0.0, 0.0),
        false,
    );
    assert!(close(d, 6.0), "dispatch distance {d}");
    let ur = Direction::new(1.0, 0.0, 0.0);
    assert!(sk.sense(Position::new(0.0, 0.0, 0.0), ur)); // central hole is outside
    assert!(!sk.sense(Position::new(3.0, 0.0, 0.0), ur)); // tube centre is inside
    assert_eq!(sk.bc(), BoundaryType::Vacuum);
    let n = sk.normal(Position::new(4.0, 0.0, 0.0));
    assert!(close(n.u, 1.0) && close(n.v, 0.0) && close(n.w, 0.0));
}
