// SPDX-License-Identifier: AGPL-3.0-only
// Copyright (C) 2026 OUTRAM PARK contributors
//
// Ported from OpenMC (https://github.com/openmc-dev/openmc, MIT licence,
// Copyright (c) 2011-2026 Massachusetts Institute of Technology, UChicago
// Argonne LLC and OpenMC contributors; see LICENSE.openmc):
//   openmc/model/surface_composite.py at commit 8f79d9d0:
//   X/Y/ZConeOneSided (lines 737-897), `_rotation_matrix` (1752-1794),
//   ConicalFrustum (1797-1869), Vessel (1872-1962);
//   openmc/surface.py at the same commit: PlaneMixin.translate/rotate
//   (lines 599-654), QuadricMixin.translate/rotate (1083-1172),
//   Cylinder._get_base_coeffs (1281-1311), Cone._get_base_coeffs (1858-1887).
//
// This file is part of OUTRAM PARK. AGPL-3.0-only; see LICENSE.

//! Cones and vessels: [`ConeOneSided`], [`ConicalFrustum`], [`Vessel`].

use super::prisms::{axis_cylinder, axis_plane};
use super::{and, dot, neg, norm, or, pos, Axis, Builder, CompositeError, CompositeRegion, T};
use crate::geometry::surface::{Plane, Quadric, SurfaceKind, XCone, YCone, ZCone};

// ── X/Y/ZConeOneSided ───────────────────────────────────────────────────────

/// **One nap of a cone**: the two-sided cone with apex `(x0, y0, z0)` and
/// squared slope `r2`, cut by the disambiguation plane through the apex.
///
/// Port of `XConeOneSided`, `YConeOneSided` and `ZConeOneSided`
/// (`surface_composite.py`, lines 737-897), one type with the axis as a
/// parameter. Surfaces: `cone`, `plane`. Inside is
/// `-cone & +plane` when `up`, `-cone & -plane` otherwise (`__neg__`, lines
/// 788-789). The plane does not take the composite's boundary condition, as
/// upstream's setter skips it.
#[derive(Debug, Clone)]
pub struct ConeOneSided {
    region: CompositeRegion,
}

impl ConeOneSided {
    /// Build the one-sided cone. `up` selects the nap that runs to `+inf`
    /// along `axis`. `r2` must be positive (`check_greater_than`).
    pub fn new(axis: Axis, apex: [f64; 3], r2: f64, up: bool) -> Result<Self, CompositeError> {
        let name = match axis {
            Axis::X => "XConeOneSided",
            Axis::Y => "YConeOneSided",
            Axis::Z => "ZConeOneSided",
        };
        if r2 <= 0.0 {
            return Err(CompositeError::new(
                name,
                "cone R^2 must be greater than 0.0",
            ));
        }
        let [x0, y0, z0] = apex;
        let (cone, plane) = match axis {
            Axis::X => (
                SurfaceKind::XCone(XCone {
                    x0,
                    y0,
                    z0,
                    r_sq: r2,
                    bc: T,
                }),
                axis_plane(Axis::X, x0),
            ),
            Axis::Y => (
                SurfaceKind::YCone(YCone {
                    x0,
                    y0,
                    z0,
                    r_sq: r2,
                    bc: T,
                }),
                axis_plane(Axis::Y, y0),
            ),
            Axis::Z => (
                SurfaceKind::ZCone(ZCone {
                    x0,
                    y0,
                    z0,
                    r_sq: r2,
                    bc: T,
                }),
                axis_plane(Axis::Z, z0),
            ),
        };
        let mut b = Builder::default();
        let i_cone = b.add(cone);
        let i_plane = b.add_with(plane, false);
        let side = if up { pos(i_plane) } else { neg(i_plane) };
        Ok(Self {
            region: b.finish(and(vec![neg(i_cone), side])),
        })
    }

    /// The expanded surfaces and regions.
    pub fn region(&self) -> &CompositeRegion {
        &self.region
    }
}

// ── ConicalFrustum ──────────────────────────────────────────────────────────

/// **Conical frustum** (MCNP's TRC): base centre `center_base`, axis vector
/// `axis` (its length is the height), radii `r1` at the base and `r2` at the
/// top.
///
/// Port of `ConicalFrustum` (`surface_composite.py`, lines 1797-1869).
/// Upstream builds the frustum along +z and rotates it onto `axis` about
/// `center_base` with `_rotation_matrix` (Rodrigues, lines 1752-1794). A
/// rotated `ZCone` (or `ZCylinder` when `r1 == r2`) becomes OpenMC's general
/// `Cone` (`Cylinder`), which upstream exports to the transport code as a
/// `Quadric` (`Cone.to_xml_element`); a rotated `ZPlane` becomes a general
/// `Plane`. So the surfaces here are `cone` as a [`Quadric`] and
/// `plane_bottom`, `plane_top` as [`Plane`]s, with coefficients computed in
/// upstream's order. Inside is `+plane_bottom & -plane_top & -cone`.
#[derive(Debug, Clone)]
pub struct ConicalFrustum {
    region: CompositeRegion,
}

/// A 3x3 matrix, row major.
type Mat3 = [[f64; 3]; 3];

/// `R @ v`.
fn mat_vec(r: &Mat3, v: [f64; 3]) -> [f64; 3] {
    [dot(r[0], v), dot(r[1], v), dot(r[2], v)]
}

/// `_rotation_matrix(v1, v2)` (`surface_composite.py`, lines 1752-1794): the
/// rotation taking `v1` onto `v2`, `sign(cos) I` when they are parallel or
/// anti-parallel to `rel_tol = 1e-8`, Rodrigues' formula otherwise.
fn rotation_matrix(v1: [f64; 3], v2: [f64; 3]) -> Mat3 {
    let n1 = norm(v1);
    let n2 = norm(v2);
    let u1 = [v1[0] / n1, v1[1] / n1, v1[2] / n1];
    let u2 = [v2[0] / n2, v2[1] / n2, v2[2] / n2];
    let cos_angle = dot(u1, u2);
    let identity: Mat3 = [[1.0, 0.0, 0.0], [0.0, 1.0, 0.0], [0.0, 0.0, 1.0]];
    // math.isclose(|cos|, 1.0, rel_tol=1e-8)
    let a = cos_angle.abs();
    if (a - 1.0).abs() <= 1e-8 * a.max(1.0) {
        let s = if cos_angle > 0.0 {
            1.0
        } else if cos_angle < 0.0 {
            -1.0
        } else {
            0.0
        };
        return identity.map(|row| row.map(|x| s * x));
    }
    let sin_angle = (1.0 - cos_angle * cos_angle).sqrt();
    let mut axis = super::cross(u1, u2);
    let an = norm(axis);
    axis = [axis[0] / an, axis[1] / an, axis[2] / an];
    let [kx, ky, kz] = axis;
    let k: Mat3 = [[0.0, -kz, ky], [kz, 0.0, -kx], [-ky, kx, 0.0]];
    let mut kk = [[0.0; 3]; 3];
    for i in 0..3 {
        for j in 0..3 {
            kk[i][j] = k[i][0] * k[0][j] + k[i][1] * k[1][j] + k[i][2] * k[2][j];
        }
    }
    let mut r = [[0.0; 3]; 3];
    for i in 0..3 {
        for j in 0..3 {
            r[i][j] = identity[i][j] + k[i][j] * sin_angle + kk[i][j] * (1.0 - cos_angle);
        }
    }
    r
}

/// A z-plane `z = z0` rotated by `r` about `pivot`, as
/// `PlaneMixin.rotate` does it (`openmc/surface.py`, lines 628-654):
/// translate by `-pivot` (`d += n . (-pivot)`), rotate `(a, b, c)`, keep `d`,
/// translate back by `+pivot`.
fn rotated_zplane(z0: f64, r: &Mat3, pivot: [f64; 3]) -> Plane {
    let (a, b, c) = (0.0, 0.0, 1.0);
    let minus = [-pivot[0], -pivot[1], -pivot[2]];
    let d = z0 + dot([a, b, c], minus);
    let n = mat_vec(r, [a, b, c]);
    let d = d + dot(n, pivot);
    Plane {
        a: n[0],
        b: n[1],
        c: n[2],
        d,
        bc: T,
    }
}

/// `_axis`: the axis normalised by its own norm.
fn unit(v: [f64; 3]) -> [f64; 3] {
    let n = norm(v);
    [v[0] / n, v[1] / n, v[2] / n]
}

/// `Cone._get_base_coeffs` (`openmc/surface.py`, lines 1858-1887): the general
/// cone with apex `o`, axis `axis` (normalised here as `_axis` does) and
/// `r2 = tan^2(theta)` as `Quadric` coefficients.
fn cone_quadric(o: [f64; 3], axis: [f64; 3], r2: f64) -> Quadric {
    let [x0, y0, z0] = o;
    let [dx, dy, dz] = unit(axis);
    let cos2 = 1.0 / (1.0 + r2);
    let a = cos2 - dx * dx;
    let b = cos2 - dy * dy;
    let c = cos2 - dz * dz;
    let d = -2.0 * dx * dy;
    let e = -2.0 * dy * dz;
    let f = -2.0 * dx * dz;
    let g = 2.0 * (dx * (dy * y0 + dz * z0) - a * x0);
    let h = 2.0 * (dy * (dx * x0 + dz * z0) - b * y0);
    let j = 2.0 * (dz * (dx * x0 + dy * y0) - c * z0);
    let k = a * x0 * x0 + b * y0 * y0 + c * z0 * z0
        - 2.0 * (dx * dy * x0 * y0 + dy * dz * y0 * z0 + dx * dz * x0 * z0);
    Quadric {
        a,
        b,
        c,
        d,
        e,
        f,
        g,
        h,
        j,
        k,
        bc: T,
    }
}

/// `Cylinder._get_base_coeffs` (`openmc/surface.py`, lines 1281-1311): the
/// general cylinder through `o` along `axis` (normalised) of radius `r`.
fn cylinder_quadric(o: [f64; 3], axis: [f64; 3], r: f64) -> Quadric {
    let [x1, y1, z1] = o;
    let u = unit(axis);
    let [x2, y2, z2] = [o[0] + u[0], o[1] + u[1], o[2] + u[2]];
    let dx = x2 - x1;
    let dy = y2 - y1;
    let dz = z2 - z1;
    let cx = y1 * z2 - y2 * z1;
    let cy = x2 * z1 - x1 * z2;
    let cz = x1 * y2 - x2 * y1;
    Quadric {
        a: dy * dy + dz * dz,
        b: dx * dx + dz * dz,
        c: dx * dx + dy * dy,
        d: -2.0 * dx * dy,
        e: -2.0 * dy * dz,
        f: -2.0 * dx * dz,
        g: 2.0 * (cy * dz - cz * dy),
        h: 2.0 * (cz * dx - cx * dz),
        j: 2.0 * (cx * dy - cy * dx),
        k: cx * cx + cy * cy + cz * cz - (dx * dx + dy * dy + dz * dz) * r * r,
        bc: T,
    }
}

impl ConicalFrustum {
    /// Build the frustum. A zero-length `axis` is refused (upstream divides
    /// by its norm).
    pub fn new(
        center_base: [f64; 3],
        axis: [f64; 3],
        r1: f64,
        r2: f64,
    ) -> Result<Self, CompositeError> {
        let h = norm(axis);
        if h == 0.0 || !h.is_finite() {
            return Err(CompositeError::new(
                "ConicalFrustum",
                "axis must be a non-zero vector",
            ));
        }
        let [x0, y0, z0] = center_base;
        let rot = rotation_matrix([0.0, 0.0, 1.0], axis);
        let pivot = center_base;
        let z_axis = mat_vec(&rot, [0.0, 0.0, 1.0]);

        let cone = if r1 != r2 {
            let apex = z0 + r1 * h / (r1 - r2);
            let q = (r1 - r2) / h;
            let r_sq = q * q;
            // ZCone.translate(-pivot): every coordinate moves.
            let local = [x0 - pivot[0], y0 - pivot[1], apex - pivot[2]];
            let o = mat_vec(&rot, local);
            let o = [o[0] + pivot[0], o[1] + pivot[1], o[2] + pivot[2]];
            cone_quadric(o, z_axis, r_sq)
        } else {
            // ZCylinder.translate(-pivot): z0 is a read-only 0 on a
            // ZCylinder, so only x0 and y0 move (`QuadricMixin.translate`).
            let local = [x0 - pivot[0], y0 - pivot[1], 0.0];
            let o = mat_vec(&rot, local);
            let o = [o[0] + pivot[0], o[1] + pivot[1], o[2] + pivot[2]];
            cylinder_quadric(o, z_axis, r1)
        };
        let plane_bottom = rotated_zplane(z0, &rot, pivot);
        let plane_top = rotated_zplane(z0 + h, &rot, pivot);

        let mut b = Builder::default();
        let i_cone = b.add(SurfaceKind::Quadric(cone));
        let i_bot = b.add(SurfaceKind::Plane(plane_bottom));
        let i_top = b.add(SurfaceKind::Plane(plane_top));
        Ok(Self {
            region: b.finish(and(vec![pos(i_bot), neg(i_top), neg(i_cone)])),
        })
    }

    /// The expanded surfaces and regions.
    pub fn region(&self) -> &CompositeRegion {
        &self.region
    }
}

// ── Vessel ──────────────────────────────────────────────────────────────────

/// **Vessel**: a cylinder of radius `r` from `p1` to `p2` along `axis`, closed
/// by semi-ellipsoidal heads of height `h1` (bottom) and `h2` (top). Serpent's
/// `vessel`; the reactor-pressure-vessel shape.
///
/// Port of `Vessel` (`surface_composite.py`, lines 1872-1962). Surfaces in
/// `_surface_names` order: `cyl`, `plane_bottom`, `plane_top`, `bottom`, `top`;
/// the heads are general [`Quadric`]s with upstream's coefficients
/// (`(x-x0)^2 + (y-y0)^2 + s^2 (z-z0)^2 = r^2`, `s = r/h`). Inside is
/// `(-cyl & +plane_bottom & -plane_top) | (-bottom & -plane_bottom) |
/// (-top & +plane_top)`. The two planes do not take the boundary condition.
#[derive(Debug, Clone)]
pub struct Vessel {
    region: CompositeRegion,
}

impl Vessel {
    /// Build the vessel; `center` is the axis position in the two
    /// perpendicular coordinates, x, y, z order. Errors when `p1 >= p2`.
    pub fn new(
        r: f64,
        p1: f64,
        p2: f64,
        h1: f64,
        h2: f64,
        center: [f64; 2],
        axis: Axis,
    ) -> Result<Self, CompositeError> {
        if p1 >= p2 {
            return Err(CompositeError::new("Vessel", "p1 must be less than p2"));
        }
        let [c1, c2] = center;
        let cyl = axis_cylinder(axis, c1, c2, r);
        let plane_bottom = axis_plane(axis, p1);
        let plane_top = axis_plane(axis, p2);

        let sb = r / h1;
        let st = r / h2;
        let sb2 = sb * sb;
        let st2 = st * st;
        let mut bottom = Quadric {
            a: 1.0,
            b: 1.0,
            c: 1.0,
            d: 0.0,
            e: 0.0,
            f: 0.0,
            g: 0.0,
            h: 0.0,
            j: 0.0,
            k: c1 * c1 + c2 * c2 + p1 * p1 * sb2 - r * r,
            bc: T,
        };
        let mut top = Quadric {
            k: c1 * c1 + c2 * c2 + p2 * p2 * st2 - r * r,
            ..bottom.clone()
        };
        match axis {
            Axis::X => {
                bottom.a *= sb2;
                top.a *= st2;
                bottom.g = -2.0 * p1 * sb2;
                top.g = -2.0 * p2 * st2;
                bottom.h = -2.0 * c1;
                top.h = -2.0 * c1;
                bottom.j = -2.0 * c2;
                top.j = -2.0 * c2;
            }
            Axis::Y => {
                bottom.b *= sb2;
                top.b *= st2;
                bottom.g = -2.0 * c1;
                top.g = -2.0 * c1;
                bottom.h = -2.0 * p1 * sb2;
                top.h = -2.0 * p2 * st2;
                bottom.j = -2.0 * c2;
                top.j = -2.0 * c2;
            }
            Axis::Z => {
                bottom.c *= sb2;
                top.c *= st2;
                bottom.g = -2.0 * c1;
                top.g = -2.0 * c1;
                bottom.h = -2.0 * c2;
                top.h = -2.0 * c2;
                bottom.j = -2.0 * p1 * sb2;
                top.j = -2.0 * p2 * st2;
            }
        }

        let mut b = Builder::default();
        let i_cyl = b.add(cyl);
        let i_pb = b.add_with(plane_bottom, false);
        let i_pt = b.add_with(plane_top, false);
        let i_bottom = b.add(SurfaceKind::Quadric(bottom));
        let i_top = b.add(SurfaceKind::Quadric(top));
        let negative = or(vec![
            and(vec![neg(i_cyl), pos(i_pb), neg(i_pt)]),
            and(vec![neg(i_bottom), neg(i_pb)]),
            and(vec![neg(i_top), pos(i_pt)]),
        ]);
        Ok(Self {
            region: b.finish(negative),
        })
    }

    /// The expanded surfaces and regions.
    pub fn region(&self) -> &CompositeRegion {
        &self.region
    }
}
