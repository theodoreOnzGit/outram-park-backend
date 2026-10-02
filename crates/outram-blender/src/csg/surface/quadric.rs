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
        let dist_hint = if coincident { 1e-14 } else { 0.0 };
        if u.u.abs() < 1e-14 {
            return f64::INFINITY;
        }
        let d = (self.x0 - r.x) / u.u;
        if d > dist_hint {
            d
        } else {
            f64::INFINITY
        }
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
        let dist_hint = if coincident { 1e-14 } else { 0.0 };
        if u.v.abs() < 1e-14 {
            return f64::INFINITY;
        }
        let d = (self.y0 - r.y) / u.v;
        if d > dist_hint {
            d
        } else {
            f64::INFINITY
        }
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
        let dist_hint = if coincident { 1e-14 } else { 0.0 };
        if u.w.abs() < 1e-14 {
            return f64::INFINITY;
        }
        let d = (self.z0 - r.z) / u.w;
        if d > dist_hint {
            d
        } else {
            f64::INFINITY
        }
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
    /// c = o·o − R². Returns the nearest root with `d > ε`, or `INFINITY` if the
    /// ray misses (discriminant < 0) or both roots are behind the particle.
    ///
    /// `coincident` (the particle is sitting on this surface, e.g. just after a
    /// boundary crossing) forces c = 0 so round-off can't reflect the tangent
    /// root back inside — the standard OpenMC treatment.
    #[inline]
    fn distance(&self, r: Position, u: Direction, coincident: bool) -> f64 {
        const EPS: f64 = 1.0e-10;
        let ox = r.x - self.x0;
        let oy = r.y - self.y0;
        let oz = r.z - self.z0;
        let k = ox * u.u + oy * u.v + oz * u.w; // o·u
        let c = if coincident {
            0.0
        } else {
            ox * ox + oy * oy + oz * oz - self.r * self.r
        };
        let disc = k * k - c;
        if disc < 0.0 {
            return f64::INFINITY;
        }
        let sq = disc.sqrt();
        // Roots in increasing order: (−k − sq) ≤ (−k + sq).
        let d_near = -k - sq;
        if d_near > EPS {
            d_near
        } else {
            let d_far = -k + sq;
            if d_far > EPS {
                d_far
            } else {
                f64::INFINITY
            }
        }
    }
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
        const FP_COINCIDENT: f64 = 1.0e-12;
        let a = u.u * u.u + u.v * u.v;
        if a == 0.0 {
            return f64::INFINITY; // travelling parallel to the axis
        }
        let dx = r.x - self.x0;
        let dy = r.y - self.y0;
        let k = dx * u.u + dy * u.v;
        let c = dx * dx + dy * dy - self.r * self.r;
        let quad = k * k - a * c;
        if quad < 0.0 {
            return f64::INFINITY; // ray misses the cylinder
        }
        let sq = quad.sqrt();
        if coincident || c.abs() < FP_COINCIDENT {
            // On the surface: one root is ~0. Facing out (k≥0) ⇒ no forward hit.
            if k >= 0.0 {
                f64::INFINITY
            } else {
                (-k + sq) / a
            }
        } else if c < 0.0 {
            // Inside: exactly one positive root, the +√ branch.
            (-k + sq) / a
        } else {
            // Outside: nearest forward root is the −√ branch, if positive.
            let d = (-k - sq) / a;
            if d < 0.0 {
                f64::INFINITY
            } else {
                d
            }
        }
    }
}

/// Smallest root `d > eps` of `a·d² + b·d + c = 0`, or `INFINITY` if none.
///
/// The shared ray-quadric intersection solver for the surfaces below whose
/// distance reduces to a general quadratic (cones and the general [`Quadric`]).
/// Degenerates to the linear solve `b·d + c = 0` when `a ≈ 0` (a ray parallel
/// to a cone's generator, or a quadric that is locally planar along `u`).
#[inline]
fn smallest_positive_root(a: f64, b: f64, c: f64, eps: f64) -> f64 {
    if a.abs() < 1.0e-14 {
        if b.abs() < 1.0e-14 {
            return f64::INFINITY;
        }
        let d = -c / b;
        return if d > eps { d } else { f64::INFINITY };
    }
    let disc = b * b - 4.0 * a * c;
    if disc < 0.0 {
        return f64::INFINITY;
    }
    let sq = disc.sqrt();
    let inv = 0.5 / a;
    let d1 = (-b - sq) * inv;
    let d2 = (-b + sq) * inv;
    let (lo, hi) = if d1 <= d2 { (d1, d2) } else { (d2, d1) };
    if lo > eps {
        lo
    } else if hi > eps {
        hi
    } else {
        f64::INFINITY
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
        let dist_hint = if coincident { 1.0e-14 } else { 0.0 };
        let denom = self.a * u.u + self.b * u.v + self.c * u.w;
        if denom.abs() < 1.0e-14 {
            return f64::INFINITY; // ray parallel to the plane
        }
        let d = -(self.a * r.x + self.b * r.y + self.c * r.z - self.d) / denom;
        if d > dist_hint {
            d
        } else {
            f64::INFINITY
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
        const FP_COINCIDENT: f64 = 1.0e-12;
        let a = u.v * u.v + u.w * u.w;
        if a == 0.0 {
            return f64::INFINITY;
        }
        let dy = r.y - self.y0;
        let dz = r.z - self.z0;
        let k = dy * u.v + dz * u.w;
        let c = dy * dy + dz * dz - self.r * self.r;
        let quad = k * k - a * c;
        if quad < 0.0 {
            return f64::INFINITY;
        }
        let sq = quad.sqrt();
        if coincident || c.abs() < FP_COINCIDENT {
            if k >= 0.0 {
                f64::INFINITY
            } else {
                (-k + sq) / a
            }
        } else if c < 0.0 {
            (-k + sq) / a
        } else {
            let d = (-k - sq) / a;
            if d < 0.0 {
                f64::INFINITY
            } else {
                d
            }
        }
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
        const FP_COINCIDENT: f64 = 1.0e-12;
        let a = u.u * u.u + u.w * u.w;
        if a == 0.0 {
            return f64::INFINITY;
        }
        let dx = r.x - self.x0;
        let dz = r.z - self.z0;
        let k = dx * u.u + dz * u.w;
        let c = dx * dx + dz * dz - self.r * self.r;
        let quad = k * k - a * c;
        if quad < 0.0 {
            return f64::INFINITY;
        }
        let sq = quad.sqrt();
        if coincident || c.abs() < FP_COINCIDENT {
            if k >= 0.0 {
                f64::INFINITY
            } else {
                (-k + sq) / a
            }
        } else if c < 0.0 {
            (-k + sq) / a
        } else {
            let d = (-k - sq) / a;
            if d < 0.0 {
                f64::INFINITY
            } else {
                d
            }
        }
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
        let dx = r.x - self.x0;
        let dy = r.y - self.y0;
        let dz = r.z - self.z0;
        let a = u.u * u.u + u.v * u.v - self.r_sq * u.w * u.w;
        let k = dx * u.u + dy * u.v - self.r_sq * dz * u.w; // half the linear coeff
        let c = if coincident {
            0.0
        } else {
            dx * dx + dy * dy - self.r_sq * dz * dz
        };
        smallest_positive_root(a, 2.0 * k, c, 1.0e-10)
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
        let dx = r.x - self.x0;
        let dy = r.y - self.y0;
        let dz = r.z - self.z0;
        let a = u.v * u.v + u.w * u.w - self.r_sq * u.u * u.u;
        let k = dy * u.v + dz * u.w - self.r_sq * dx * u.u;
        let c = if coincident {
            0.0
        } else {
            dy * dy + dz * dz - self.r_sq * dx * dx
        };
        smallest_positive_root(a, 2.0 * k, c, 1.0e-10)
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
        let dx = r.x - self.x0;
        let dy = r.y - self.y0;
        let dz = r.z - self.z0;
        let a = u.u * u.u + u.w * u.w - self.r_sq * u.v * u.v;
        let k = dx * u.u + dz * u.w - self.r_sq * dy * u.v;
        let c = if coincident {
            0.0
        } else {
            dx * dx + dz * dz - self.r_sq * dy * dy
        };
        smallest_positive_root(a, 2.0 * k, c, 1.0e-10)
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
        let (x, y, z) = (r.x, r.y, r.z);
        self.a * x * x
            + self.b * y * y
            + self.c * z * z
            + self.d * x * y
            + self.e * y * z
            + self.f * x * z
            + self.g * x
            + self.h * y
            + self.j * z
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
    fn distance(&self, r: Position, u: Direction, coincident: bool) -> f64 {
        let (x, y, z) = (r.x, r.y, r.z);
        // Substitute r + d·u into the quadric → a_q·d² + b_q·d + c_q = 0.
        let a_q = self.a * u.u * u.u
            + self.b * u.v * u.v
            + self.c * u.w * u.w
            + self.d * u.u * u.v
            + self.e * u.v * u.w
            + self.f * u.u * u.w;
        let b_q = 2.0 * self.a * x * u.u
            + 2.0 * self.b * y * u.v
            + 2.0 * self.c * z * u.w
            + self.d * (x * u.v + y * u.u)
            + self.e * (y * u.w + z * u.v)
            + self.f * (x * u.w + z * u.u)
            + self.g * u.u
            + self.h * u.v
            + self.j * u.w;
        let c_q = if coincident { 0.0 } else { self.evaluate(r) };
        smallest_positive_root(a_q, b_q, c_q, 1.0e-10)
    }
}
