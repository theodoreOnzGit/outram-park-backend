// SPDX-License-Identifier: GPL-3.0-only
// Copyright (C) 2026 OUTRAM PARK contributors
//
// Ported from OpenMC (https://github.com/openmc-dev/openmc, MIT licence,
// Copyright (c) 2011-2026 Massachusetts Institute of Technology, UChicago
// Argonne LLC and OpenMC contributors; see LICENSE.openmc):
//   src/surface.cpp, include/openmc/surface.h
// Planes, spheres, cylinders, cones and the general quadric.
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

//! Planes, sphere, cylinders, cones and the general quadric (split out of
//! the surface module on 2026-10-02 to respect the 1000-line file cap).

use super::{BoundaryType, Surface};
use crate::csg::position::{Direction, Position};

// ── Concrete surfaces ─────────────────────────────────────────────────────────

#[derive(Debug, Clone)]
/// Infinite plane perpendicular to the X axis: x = x0.
pub struct XPlane {
    pub x0: f64,
    pub bc: BoundaryType,
}

impl Surface for XPlane {
    #[inline]
    fn evaluate(&self, r: Position) -> f64 {
        r.x - self.x0
    }
    #[inline]
    fn normal(&self, _r: Position) -> Direction {
        Direction::new(1.0, 0.0, 0.0)
    }
    #[inline]
    fn distance(&self, r: Position, u: Direction, coincident: bool) -> f64 {
        axis_aligned_plane_distance(r.x, u.u, coincident, self.x0)
    }
}

#[derive(Debug, Clone)]
/// Infinite plane perpendicular to the Y axis: y = y0.
pub struct YPlane {
    pub y0: f64,
    pub bc: BoundaryType,
}

impl Surface for YPlane {
    #[inline]
    fn evaluate(&self, r: Position) -> f64 {
        r.y - self.y0
    }
    #[inline]
    fn normal(&self, _r: Position) -> Direction {
        Direction::new(0.0, 1.0, 0.0)
    }
    #[inline]
    fn distance(&self, r: Position, u: Direction, coincident: bool) -> f64 {
        axis_aligned_plane_distance(r.y, u.v, coincident, self.y0)
    }
}

#[derive(Debug, Clone)]
/// Infinite plane perpendicular to the Z axis: z = z0.
pub struct ZPlane {
    pub z0: f64,
    pub bc: BoundaryType,
}

impl Surface for ZPlane {
    #[inline]
    fn evaluate(&self, r: Position) -> f64 {
        r.z - self.z0
    }
    #[inline]
    fn normal(&self, _r: Position) -> Direction {
        Direction::new(0.0, 0.0, 1.0)
    }
    #[inline]
    fn distance(&self, r: Position, u: Direction, coincident: bool) -> f64 {
        axis_aligned_plane_distance(r.z, u.w, coincident, self.z0)
    }
}

#[derive(Debug, Clone)]
/// Sphere: (x-x0)² + (y-y0)² + (z-z0)² = r²
pub struct Sphere {
    pub x0: f64,
    pub y0: f64,
    pub z0: f64,
    pub r: f64,
    pub bc: BoundaryType,
}

impl Surface for Sphere {
    #[inline]
    fn evaluate(&self, r: Position) -> f64 {
        let dx = r.x - self.x0;
        let dy = r.y - self.y0;
        let dz = r.z - self.z0;
        dx * dx + dy * dy + dz * dz - self.r * self.r
    }
    #[inline]
    fn normal(&self, r: Position) -> Direction {
        Direction::from_unnormalised(r.x - self.x0, r.y - self.y0, r.z - self.z0)
    }
    /// Smallest positive distance from `r` along `u` to the sphere.
    ///
    /// Solves |o + d·u|² = R² with o = r − center. Since |u| = 1 the quadratic
    /// is d² + 2(o·u)d + (o·o − R²) = 0, so d = −k ± √(k² − c) where k = o·u and
    /// c = o·o − R². Returns `INFINITY` if the ray misses (discriminant < 0).
    /// Three cases, as `SurfaceSphere::distance` (`src/surface.cpp:652`): on the
    /// sphere (`coincident` or `|c| < FP_COINCIDENT`), the sign of `k` says whether
    /// the ray faces out (no hit) or in (`−k + √quad`); inside (`c < 0`) the `+√`
    /// root; outside the `−√` root, if not behind. ~~`coincident` forced c = 0 and
    /// roots below a 1e-10 floor were dropped~~ **CORRECTED 2026-11-10** (gh:#1386:
    /// that differed from upstream).
    // ANCHOR: sphere_distance (mdBook include, Monte Carlo tutorial gh:#521)
    #[inline]
    fn distance(&self, r: Position, u: Direction, coincident: bool) -> f64 {
        // `SurfaceSphere::distance`, src/surface.cpp:652.
        let x = r.x - self.x0;
        let y = r.y - self.y0;
        let z = r.z - self.z0;
        let k = x * u.u + y * u.v + z * u.w;
        let c = x * x + y * y + z * z - self.r * self.r;
        let quad = k * k - c;
        if quad < 0.0 {
            f64::INFINITY
        } else if coincident || c.abs() < FP_COINCIDENT {
            // On the sphere: the sign of k says whether we face in or out.
            if k >= 0.0 {
                f64::INFINITY
            } else {
                -k + quad.sqrt()
            }
        } else if c < 0.0 {
            -k + quad.sqrt()
        } else {
            let d = -k - quad.sqrt();
            if d < 0.0 {
                f64::INFINITY
            } else {
                d
            }
        }
    }
    // ANCHOR_END: sphere_distance
}

#[derive(Debug, Clone)]
/// Infinite cylinder along the Z axis: (x-x0)² + (y-y0)² = r²
pub struct ZCylinder {
    pub x0: f64,
    pub y0: f64,
    pub r: f64,
    pub bc: BoundaryType,
}

impl Surface for ZCylinder {
    #[inline]
    fn evaluate(&self, r: Position) -> f64 {
        let dx = r.x - self.x0;
        let dy = r.y - self.y0;
        dx * dx + dy * dy - self.r * self.r
    }
    #[inline]
    fn normal(&self, r: Position) -> Direction {
        Direction::from_unnormalised(r.x - self.x0, r.y - self.y0, 0.0)
    }
    /// Smallest positive distance from `r` along `u` to the infinite Z cylinder.
    ///
    /// Ported from OpenMC `axis_aligned_cylinder_distance<2,0,1>`
    /// (`src/surface.cpp:401`). With `a = u.u² + u.v²`, `k = Δx·u + Δy·v` and
    /// `c = Δx² + Δy² − R²` the intersections are `d = (−k ± √(k²−a·c))/a`.
    /// `coincident` (or `|c|` tiny) means the particle sits on the surface: the
    /// sign of `k` says whether it faces out (no hit) or in.
    #[inline]
    fn distance(&self, r: Position, u: Direction, coincident: bool) -> f64 {
        axis_aligned_cylinder_distance(
            u.w,
            (r.x - self.x0, r.y - self.y0),
            (u.u, u.v),
            coincident,
            self.r,
        )
    }
}

/// `FP_COINCIDENT` (`include/openmc/constants.h:55`), shared with the module root.
use super::FP_COINCIDENT;

// The three helpers below are line-by-line ports of OpenMC `8f79d9d0`
// `src/surface.cpp` (gh:#1386; code-to-code test
// `tests/surface_distance_vs_openmc_code_to_code.rs`). Upstream returns
// `INFTY` (= `DBL_MAX`) for "no hit"; this port returns `f64::INFINITY`
// (same meaning; the test counts the two as agreeing misses).

/// `axis_aligned_plane_distance<i>` (`src/surface.cpp:209`): distance along `u_i`
/// from coordinate `r_i` to the plane at `offset`.
#[inline]
fn axis_aligned_plane_distance(r_i: f64, u_i: f64, coincident: bool, offset: f64) -> f64 {
    let f = offset - r_i;
    if coincident || f.abs() < FP_COINCIDENT || u_i == 0.0 {
        return f64::INFINITY;
    }
    let d = f / u_i;
    if d < 0.0 {
        return f64::INFINITY;
    }
    d
}

/// `axis_aligned_cylinder_distance<i1,i2,i3>` (`src/surface.cpp:446`).
/// `u1` is the direction cosine along the cylinder axis, `(r2, r3)` the position
/// relative to the axis in the other two coordinates and `(u2, u3)` the matching
/// direction cosines.
#[inline]
fn axis_aligned_cylinder_distance(
    u1: f64,
    (r2, r3): (f64, f64),
    (u2, u3): (f64, f64),
    coincident: bool,
    radius: f64,
) -> f64 {
    let a = 1.0 - u1 * u1; // u^2 + v^2
    if a == 0.0 {
        return f64::INFINITY;
    }
    let k = r2 * u2 + r3 * u3;
    let c = r2 * r2 + r3 * r3 - radius * radius;
    let quad = k * k - a * c;
    if quad < 0.0 {
        f64::INFINITY
    } else if coincident || c.abs() < FP_COINCIDENT {
        // On the cylinder: the sign of k says whether we face out or in.
        if k >= 0.0 {
            f64::INFINITY
        } else {
            (-k + quad.sqrt()) / a
        }
    } else if c < 0.0 {
        (-k + quad.sqrt()) / a
    } else {
        let d = (-k - quad.sqrt()) / a;
        if d < 0.0 {
            f64::INFINITY
        } else {
            d
        }
    }
}

/// `axis_aligned_cone_distance<i1,i2,i3>` (`src/surface.cpp:734`); `(r1, r2, r3)`
/// is the position relative to the apex with `1` the cone axis.
#[inline]
fn axis_aligned_cone_distance(
    (r1, r2, r3): (f64, f64, f64),
    (u1, u2, u3): (f64, f64, f64),
    coincident: bool,
    radius_sq: f64,
) -> f64 {
    let a = u2 * u2 + u3 * u3 - radius_sq * u1 * u1;
    let k = r2 * u2 + r3 * u3 - radius_sq * r1 * u1;
    let c = r2 * r2 + r3 * r3 - radius_sq * r1 * r1;
    let mut quad = k * k - a * c;
    let mut d;
    if quad < 0.0 {
        return f64::INFINITY;
    } else if coincident || c.abs() < FP_COINCIDENT {
        if k >= 0.0 {
            d = (-k - quad.sqrt()) / a;
        } else {
            d = (-k + quad.sqrt()) / a;
        }
    } else {
        quad = quad.sqrt();
        d = (-k - quad) / a;
        let b = (-k + quad) / a;
        if d < 0.0 {
            if b > 0.0 {
                d = b;
            }
        } else if b > 0.0 && b < d {
            d = b;
        }
    }
    if d <= 0.0 {
        f64::INFINITY
    } else {
        d
    }
}

#[derive(Debug, Clone)]
/// General plane: A·x + B·y + C·z = D.
///
/// The unrestricted-orientation plane (the axis-aligned [`XPlane`]/[`YPlane`]/
/// [`ZPlane`] are the cheap special cases). Maps to OpenMC `SurfacePlane`
/// (`src/surface.cpp`). `(A, B, C)` need not be unit — [`Surface::normal`]
/// normalises them.
pub struct Plane {
    pub a: f64,
    pub b: f64,
    pub c: f64,
    pub d: f64,
    pub bc: BoundaryType,
}

impl Surface for Plane {
    #[inline]
    fn evaluate(&self, r: Position) -> f64 {
        self.a * r.x + self.b * r.y + self.c * r.z - self.d
    }
    #[inline]
    fn normal(&self, _r: Position) -> Direction {
        Direction::from_unnormalised(self.a, self.b, self.c)
    }
    #[inline]
    fn distance(&self, r: Position, u: Direction, coincident: bool) -> f64 {
        // `SurfacePlane::distance`, src/surface.cpp:400.
        let f = self.a * r.x + self.b * r.y + self.c * r.z - self.d;
        let projection = self.a * u.u + self.b * u.v + self.c * u.w;
        if coincident || f.abs() < FP_COINCIDENT || projection == 0.0 {
            f64::INFINITY
        } else {
            let d = -f / projection;
            if d < 0.0 {
                f64::INFINITY
            } else {
                d
            }
        }
    }
}

#[derive(Debug, Clone)]
/// Infinite cylinder along the X axis: (y-y0)² + (z-z0)² = r².
///
/// The X-axis twin of [`ZCylinder`]; same intersection algebra with the radial
/// pair `(y, z)` and the parallel axis `x`. Ported from OpenMC
/// `axis_aligned_cylinder_distance<0,1,2>` (`src/surface.cpp`).
pub struct XCylinder {
    pub y0: f64,
    pub z0: f64,
    pub r: f64,
    pub bc: BoundaryType,
}

impl Surface for XCylinder {
    #[inline]
    fn evaluate(&self, r: Position) -> f64 {
        let dy = r.y - self.y0;
        let dz = r.z - self.z0;
        dy * dy + dz * dz - self.r * self.r
    }
    #[inline]
    fn normal(&self, r: Position) -> Direction {
        Direction::from_unnormalised(0.0, r.y - self.y0, r.z - self.z0)
    }
    #[inline]
    fn distance(&self, r: Position, u: Direction, coincident: bool) -> f64 {
        axis_aligned_cylinder_distance(
            u.u,
            (r.y - self.y0, r.z - self.z0),
            (u.v, u.w),
            coincident,
            self.r,
        )
    }
}

#[derive(Debug, Clone)]
/// Infinite cylinder along the Y axis: (x-x0)² + (z-z0)² = r².
///
/// The Y-axis twin of [`ZCylinder`]; radial pair `(x, z)`, parallel axis `y`.
/// Ported from OpenMC `axis_aligned_cylinder_distance<1,0,2>` (`src/surface.cpp`).
pub struct YCylinder {
    pub x0: f64,
    pub z0: f64,
    pub r: f64,
    pub bc: BoundaryType,
}

impl Surface for YCylinder {
    #[inline]
    fn evaluate(&self, r: Position) -> f64 {
        let dx = r.x - self.x0;
        let dz = r.z - self.z0;
        dx * dx + dz * dz - self.r * self.r
    }
    #[inline]
    fn normal(&self, r: Position) -> Direction {
        Direction::from_unnormalised(r.x - self.x0, 0.0, r.z - self.z0)
    }
    #[inline]
    fn distance(&self, r: Position, u: Direction, coincident: bool) -> f64 {
        axis_aligned_cylinder_distance(
            u.v,
            (r.x - self.x0, r.z - self.z0),
            (u.u, u.w),
            coincident,
            self.r,
        )
    }
}

#[derive(Debug, Clone)]
/// Double-napped cone about the Z axis: (x-x0)² + (y-y0)² = r_sq·(z-z0)².
///
/// `r_sq` is the **square of the slope** (tan² of the half-opening-angle), the
/// same parameterisation OpenMC `SurfaceZCone` stores (`src/surface.cpp`). The
/// surface is the full double cone (both naps); a single nap is selected in CSG
/// by intersecting with a half-space (e.g. `z > z0`).
pub struct ZCone {
    pub x0: f64,
    pub y0: f64,
    pub z0: f64,
    pub r_sq: f64,
    pub bc: BoundaryType,
}

impl Surface for ZCone {
    #[inline]
    fn evaluate(&self, r: Position) -> f64 {
        let dx = r.x - self.x0;
        let dy = r.y - self.y0;
        let dz = r.z - self.z0;
        dx * dx + dy * dy - self.r_sq * dz * dz
    }
    #[inline]
    fn normal(&self, r: Position) -> Direction {
        Direction::from_unnormalised(
            2.0 * (r.x - self.x0),
            2.0 * (r.y - self.y0),
            -2.0 * self.r_sq * (r.z - self.z0),
        )
    }
    #[inline]
    fn distance(&self, r: Position, u: Direction, coincident: bool) -> f64 {
        axis_aligned_cone_distance(
            (r.z - self.z0, r.x - self.x0, r.y - self.y0),
            (u.w, u.u, u.v),
            coincident,
            self.r_sq,
        )
    }
}

#[derive(Debug, Clone)]
/// Double-napped cone about the X axis: (y-y0)² + (z-z0)² = r_sq·(x-x0)².
///
/// X-axis twin of [`ZCone`]; `r_sq` is the slope². Ported from OpenMC
/// `SurfaceXCone` (`src/surface.cpp`).
pub struct XCone {
    pub x0: f64,
    pub y0: f64,
    pub z0: f64,
    pub r_sq: f64,
    pub bc: BoundaryType,
}

impl Surface for XCone {
    #[inline]
    fn evaluate(&self, r: Position) -> f64 {
        let dx = r.x - self.x0;
        let dy = r.y - self.y0;
        let dz = r.z - self.z0;
        dy * dy + dz * dz - self.r_sq * dx * dx
    }
    #[inline]
    fn normal(&self, r: Position) -> Direction {
        Direction::from_unnormalised(
            -2.0 * self.r_sq * (r.x - self.x0),
            2.0 * (r.y - self.y0),
            2.0 * (r.z - self.z0),
        )
    }
    #[inline]
    fn distance(&self, r: Position, u: Direction, coincident: bool) -> f64 {
        axis_aligned_cone_distance(
            (r.x - self.x0, r.y - self.y0, r.z - self.z0),
            (u.u, u.v, u.w),
            coincident,
            self.r_sq,
        )
    }
}

#[derive(Debug, Clone)]
/// Double-napped cone about the Y axis: (x-x0)² + (z-z0)² = r_sq·(y-y0)².
///
/// Y-axis twin of [`ZCone`]; `r_sq` is the slope². Ported from OpenMC
/// `SurfaceYCone` (`src/surface.cpp`).
pub struct YCone {
    pub x0: f64,
    pub y0: f64,
    pub z0: f64,
    pub r_sq: f64,
    pub bc: BoundaryType,
}

impl Surface for YCone {
    #[inline]
    fn evaluate(&self, r: Position) -> f64 {
        let dx = r.x - self.x0;
        let dy = r.y - self.y0;
        let dz = r.z - self.z0;
        dx * dx + dz * dz - self.r_sq * dy * dy
    }
    #[inline]
    fn normal(&self, r: Position) -> Direction {
        Direction::from_unnormalised(
            2.0 * (r.x - self.x0),
            -2.0 * self.r_sq * (r.y - self.y0),
            2.0 * (r.z - self.z0),
        )
    }
    #[inline]
    fn distance(&self, r: Position, u: Direction, coincident: bool) -> f64 {
        axis_aligned_cone_distance(
            (r.y - self.y0, r.x - self.x0, r.z - self.z0),
            (u.v, u.u, u.w),
            coincident,
            self.r_sq,
        )
    }
}

#[derive(Debug, Clone)]
/// General quadric: A x² + B y² + C z² + D xy + E yz + F xz + G x + H y + J z + K = 0.
///
/// The most general second-order surface — every other surface here is a special
/// case, but the explicit forms above are cheaper and are preferred when the
/// geometry allows. Maps to OpenMC `SurfaceQuadric` (`src/surface.cpp`).
pub struct Quadric {
    pub a: f64,
    pub b: f64,
    pub c: f64,
    pub d: f64,
    pub e: f64,
    pub f: f64,
    pub g: f64,
    pub h: f64,
    pub j: f64,
    pub k: f64,
    pub bc: BoundaryType,
}

impl Surface for Quadric {
    #[inline]
    fn evaluate(&self, r: Position) -> f64 {
        // `SurfaceQuadric::evaluate`, src/surface.cpp:907 (this operation order).
        let (x, y, z) = (r.x, r.y, r.z);
        x * (self.a * x + self.d * y + self.g)
            + y * (self.b * y + self.e * z + self.h)
            + z * (self.c * z + self.f * x + self.j)
            + self.k
    }
    #[inline]
    fn normal(&self, r: Position) -> Direction {
        let (x, y, z) = (r.x, r.y, r.z);
        Direction::from_unnormalised(
            2.0 * self.a * x + self.d * y + self.f * z + self.g,
            2.0 * self.b * y + self.d * x + self.e * z + self.h,
            2.0 * self.c * z + self.e * y + self.f * x + self.j,
        )
    }
    #[inline]
    fn distance(&self, r: Position, ang: Direction, coincident: bool) -> f64 {
        // `SurfaceQuadric::distance`, src/surface.cpp:916.
        let (x, y, z) = (r.x, r.y, r.z);
        let (u, v, w) = (ang.u, ang.v, ang.w);
        let a = self.a * u * u
            + self.b * v * v
            + self.c * w * w
            + self.d * u * v
            + self.e * v * w
            + self.f * u * w;
        let k = self.a * u * x
            + self.b * v * y
            + self.c * w * z
            + 0.5
                * (self.d * (u * y + v * x)
                    + self.e * (v * z + w * y)
                    + self.f * (w * x + u * z)
                    + self.g * u
                    + self.h * v
                    + self.j * w);
        let c = self.a * x * x
            + self.b * y * y
            + self.c * z * z
            + self.d * x * y
            + self.e * y * z
            + self.f * x * z
            + self.g * x
            + self.h * y
            + self.j * z
            + self.k;
        let mut quad = k * k - a * c;
        let mut d;
        if quad < 0.0 {
            return f64::INFINITY;
        } else if coincident || c.abs() < FP_COINCIDENT {
            // On the surface; a == 0 means plane-like along this ray.
            if a == 0.0 {
                d = f64::INFINITY;
            } else if k >= 0.0 {
                d = (-k - quad.sqrt()) / a;
            } else {
                d = (-k + quad.sqrt()) / a;
            }
        } else if a == 0.0 {
            // Locally a plane: one solution, a = 0 in a d^2 + 2 k d + c = 0.
            d = -0.5 * c / k;
        } else {
            quad = quad.sqrt();
            d = (-k - quad) / a;
            let b = (-k + quad) / a;
            if d < 0.0 {
                if b > 0.0 {
                    d = b;
                }
            } else if b > 0.0 && b < d {
                d = b;
            }
        }
        if d <= 0.0 {
            f64::INFINITY
        } else {
            d
        }
    }
}
