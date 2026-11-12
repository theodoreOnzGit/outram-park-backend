// SPDX-License-Identifier: AGPL-3.0-only
// Copyright (C) 2026 OUTRAM PARK contributors
//
// Ported from OpenMC (https://github.com/openmc-dev/openmc, MIT licence,
// Copyright (c) 2011-2026 Massachusetts Institute of Technology, UChicago
// Argonne LLC and OpenMC contributors; see LICENSE.openmc):
//   openmc/model/surface_composite.py at commit 8f79d9d0: CylinderSector
//   (lines 71-237), IsogonalOctagon (240-395), CruciformPrism (1372-1454),
//   RectangularPrism and its `_plane` helper (1457-1585).
//
// This file is part of OUTRAM PARK. AGPL-3.0-only; see LICENSE.

//! Infinite prisms: [`CylinderSector`], [`IsogonalOctagon`],
//! [`CruciformPrism`] and the rounded-corner [`RectangularPrism`].

use super::{
    and, flip_normal, neg, not, or, plane_from_points, pos, remap, Axis, Builder, CompositeError,
    CompositeRegion, T,
};
use crate::geometry::surface::{SurfaceKind, XCylinder, XPlane, YCylinder, YPlane, ZCylinder, ZPlane};

/// An axis-aligned plane `coord = value` on `axis` (upstream `_plane`, line 1457).
pub(super) fn axis_plane(axis: Axis, value: f64) -> SurfaceKind {
    match axis {
        Axis::X => SurfaceKind::XPlane(XPlane { x0: value, bc: T }),
        Axis::Y => SurfaceKind::YPlane(YPlane { y0: value, bc: T }),
        Axis::Z => SurfaceKind::ZPlane(ZPlane { z0: value, bc: T }),
    }
}

/// A cylinder parallel to `axis`, centred at `(c1, c2)` in the other two
/// coordinates taken in x, y, z order (`(y, z)` for X, `(x, z)` for Y, `(x, y)`
/// for Z), as `openmc.XCylinder(y0, z0, r)` etc. take them.
pub(super) fn axis_cylinder(axis: Axis, c1: f64, c2: f64, r: f64) -> SurfaceKind {
    match axis {
        Axis::X => SurfaceKind::XCylinder(XCylinder {
            y0: c1,
            z0: c2,
            r,
            bc: T,
        }),
        Axis::Y => SurfaceKind::YCylinder(YCylinder {
            x0: c1,
            z0: c2,
            r,
            bc: T,
        }),
        Axis::Z => SurfaceKind::ZCylinder(ZCylinder {
            x0: c1,
            y0: c2,
            r,
            bc: T,
        }),
    }
}

// ── CylinderSector ───────────────────────────────────────────────────────────

/// **Infinite cylindrical sector**: the annulus `r1 < r < r2` between the
/// half-planes at angles `theta1` and `theta2` (degrees, counter-clockwise
/// from the first basis axis: +y for an X axis, +x for Y and Z, as upstream
/// documents it; see the coordinate map in [`Self::new`]).
///
/// Port of `CylinderSector` (`openmc/model/surface_composite.py`, lines
/// 71-237). Surfaces in `_surface_names` order: `outer_cyl`, `inner_cyl`,
/// `plane1`, `plane2`. Inside is
/// `-outer & +inner & -plane1 & +plane2`, and for a **reflex** sector
/// (`theta2 - theta1 > 180`) `-outer & +inner & (-plane1 | +plane2)`
/// (`__neg__`, lines 233-237).
#[derive(Debug, Clone)]
pub struct CylinderSector {
    region: CompositeRegion,
}

impl CylinderSector {
    /// Build the sector. `center` is the axis position in the two
    /// perpendicular coordinates, in x, y, z order. Errors as upstream when
    /// `r2 <= r1` or `theta2 <= theta1`.
    pub fn new(
        r1: f64,
        r2: f64,
        theta1: f64,
        theta2: f64,
        center: [f64; 2],
        axis: Axis,
    ) -> Result<Self, CompositeError> {
        if r2 <= r1 {
            return Err(CompositeError::new(
                "CylinderSector",
                "r2 must be greater than r1.",
            ));
        }
        if theta2 <= theta1 {
            return Err(CompositeError::new(
                "CylinderSector",
                "theta2 must be greater than theta1.",
            ));
        }
        let reflex = theta2 - theta1 > 180.0;
        let pi = std::f64::consts::PI;
        // `phi1 = pi / 180 * theta1`, evaluated left to right.
        let phi1 = pi / 180.0 * theta1;
        let phi2 = pi / 180.0 * theta2;

        let p1 = [center[0], center[1], 1.0];
        let p2_plane1 = [
            r1 * phi1.cos() + center[0],
            r1 * phi1.sin() + center[1],
            0.0,
        ];
        let p3_plane1 = [
            r2 * phi1.cos() + center[0],
            r2 * phi1.sin() + center[1],
            0.0,
        ];
        let p2_plane2 = [
            r1 * phi2.cos() + center[0],
            r1 * phi2.sin() + center[1],
            0.0,
        ];
        let p3_plane2 = [
            r2 * phi2.cos() + center[0],
            r2 * phi2.sin() + center[1],
            0.0,
        ];

        let coord_map = match axis {
            Axis::Z => [0, 1, 2],
            Axis::Y => [0, 2, 1],
            Axis::X => [2, 0, 1],
        };
        let inner = axis_cylinder(axis, center[0], center[1], r1);
        let outer = axis_cylinder(axis, center[0], center[1], r2);

        let [p1, p2_plane1, p3_plane1, p2_plane2, p3_plane2] =
            [p1, p2_plane1, p3_plane1, p2_plane2, p3_plane2].map(|p| remap(p, coord_map));

        let line = || {
            CompositeError::new(
                "CylinderSector",
                "All three points appear to lie along a line.",
            )
        };
        let mut plane1 = plane_from_points(p1, p2_plane1, p3_plane1).ok_or_else(line)?;
        let mut plane2 = plane_from_points(p1, p2_plane2, p3_plane2).ok_or_else(line)?;
        if axis == Axis::Y {
            flip_normal(&mut plane1);
            flip_normal(&mut plane2);
        }

        let mut b = Builder::default();
        let i_outer = b.add(outer);
        let i_inner = b.add(inner);
        let i_p1 = b.add(SurfaceKind::Plane(plane1));
        let i_p2 = b.add(SurfaceKind::Plane(plane2));
        let negative = if reflex {
            and(vec![
                neg(i_outer),
                pos(i_inner),
                or(vec![neg(i_p1), pos(i_p2)]),
            ])
        } else {
            and(vec![neg(i_outer), pos(i_inner), neg(i_p1), pos(i_p2)])
        };
        Ok(Self {
            region: b.finish(negative),
        })
    }

    /// Upstream's alternate constructor `from_theta_alpha` (lines 182-231):
    /// central angle `theta` and offset `alpha`, so `theta1 = alpha` and
    /// `theta2 = alpha + theta`. `theta` must be in `(0, 360)`.
    pub fn from_theta_alpha(
        r1: f64,
        r2: f64,
        theta: f64,
        alpha: f64,
        center: [f64; 2],
        axis: Axis,
    ) -> Result<Self, CompositeError> {
        if theta >= 360.0 || theta <= 0.0 {
            return Err(CompositeError::new(
                "CylinderSector",
                "theta must be less than 360 and greater than 0.",
            ));
        }
        Self::new(r1, r2, alpha, alpha + theta, center, axis)
    }

    /// The expanded surfaces and regions.
    pub fn region(&self) -> &CompositeRegion {
        &self.region
    }
}

// ── IsogonalOctagon ─────────────────────────────────────────────────────────

/// **Infinite isogonal octagon**: four faces parallel to the basis axes at
/// half-width `r1`, four at 45 degrees at half-width `r2`.
///
/// Port of `IsogonalOctagon` (`surface_composite.py`, lines 240-395).
/// Surfaces in `_surface_names` order: `top`, `bottom`, `upper_right`,
/// `lower_left`, `right`, `left`, `lower_right`, `upper_left`. Both `__neg__`
/// and `__pos__` are overridden upstream (lines 374-395) and both are ported:
/// `+octagon` is the union of the eight outward half-spaces, not a complement.
#[derive(Debug, Clone)]
pub struct IsogonalOctagon {
    region: CompositeRegion,
}

impl IsogonalOctagon {
    /// Build the octagon centred at `center` (the two perpendicular
    /// coordinates in x, y, z order). Errors as upstream when
    /// `r2 > r1 sqrt(2)` or `r1 > r2 sqrt(2)`.
    pub fn new(center: [f64; 2], r1: f64, r2: f64, axis: Axis) -> Result<Self, CompositeError> {
        let [c1, c2] = center;
        let cright = c1 + r1;
        let cleft = c1 - r1;
        let ctop = c2 + r1;
        let cbottom = c2 - r1;
        let sqrt2 = 2.0_f64.sqrt();
        if r2 > r1 * sqrt2 {
            return Err(CompositeError::new(
                "IsogonalOctagon",
                "r2 is greater than sqrt(2) * r1. Octagon may be erroneous.",
            ));
        }
        if r1 > r2 * sqrt2 {
            return Err(CompositeError::new(
                "IsogonalOctagon",
                "r1 is greater than sqrt(2) * r2. Octagon may be erroneous.",
            ));
        }
        let l = r2 * sqrt2 - r1;

        let p1_ur = [l, r1, 0.0];
        let p2_ur = [r1, l, 0.0];
        let p3_ur = [r1, l, 1.0];
        let p1_lr = [r1, -l, 0.0];
        let p2_lr = [l, -r1, 0.0];
        let p3_lr = [l, -r1, 1.0];
        let negate = |p: [f64; 3]| [-p[0], -p[1], -p[2]];
        let p1_ll = negate(p1_ur);
        let p2_ll = negate(p2_ur);
        let p3_ll = negate(p3_ur);
        let p1_ul = negate(p1_lr);
        let p2_ul = negate(p2_lr);
        let p3_ul = negate(p3_lr);

        let (coord_map, top, bottom, right, left) = match axis {
            Axis::Z => (
                [0, 1, 2],
                axis_plane(Axis::Y, ctop),
                axis_plane(Axis::Y, cbottom),
                axis_plane(Axis::X, cright),
                axis_plane(Axis::X, cleft),
            ),
            Axis::Y => (
                [0, 2, 1],
                axis_plane(Axis::Z, ctop),
                axis_plane(Axis::Z, cbottom),
                axis_plane(Axis::X, cright),
                axis_plane(Axis::X, cleft),
            ),
            Axis::X => (
                [2, 0, 1],
                axis_plane(Axis::Z, ctop),
                axis_plane(Axis::Z, cbottom),
                axis_plane(Axis::Y, cright),
                axis_plane(Axis::Y, cleft),
            ),
        };
        // `p[0] += c1; p[1] += c2; p[:] = p[coord_map]`.
        let place = |p: [f64; 3]| remap([p[0] + c1, p[1] + c2, p[2]], coord_map);
        let line = || {
            CompositeError::new(
                "IsogonalOctagon",
                "All three points appear to lie along a line.",
            )
        };
        let upper_right =
            plane_from_points(place(p1_ur), place(p2_ur), place(p3_ur)).ok_or_else(line)?;
        let lower_right =
            plane_from_points(place(p1_lr), place(p2_lr), place(p3_lr)).ok_or_else(line)?;
        let lower_left =
            plane_from_points(place(p1_ll), place(p2_ll), place(p3_ll)).ok_or_else(line)?;
        let upper_left =
            plane_from_points(place(p1_ul), place(p2_ul), place(p3_ul)).ok_or_else(line)?;

        let mut b = Builder::default();
        let top = b.add(top);
        let bottom = b.add(bottom);
        let ur = b.add(SurfaceKind::Plane(upper_right));
        let ll = b.add(SurfaceKind::Plane(lower_left));
        let right = b.add(right);
        let left = b.add(left);
        let lr = b.add(SurfaceKind::Plane(lower_right));
        let ul = b.add(SurfaceKind::Plane(upper_left));

        let (negative, positive) = if axis == Axis::Y {
            (
                and(vec![
                    neg(top),
                    pos(bottom),
                    neg(right),
                    pos(left),
                    neg(ur),
                    neg(lr),
                    pos(ll),
                    pos(ul),
                ]),
                or(vec![
                    pos(top),
                    neg(bottom),
                    pos(right),
                    neg(left),
                    pos(ur),
                    pos(lr),
                    neg(ll),
                    neg(ul),
                ]),
            )
        } else {
            (
                and(vec![
                    neg(top),
                    pos(bottom),
                    neg(right),
                    pos(left),
                    pos(ur),
                    pos(lr),
                    neg(ll),
                    neg(ul),
                ]),
                or(vec![
                    pos(top),
                    neg(bottom),
                    pos(right),
                    neg(left),
                    neg(ur),
                    neg(lr),
                    pos(ll),
                    pos(ul),
                ]),
            )
        };
        Ok(Self {
            region: b.finish_with(negative, positive),
        })
    }

    /// The expanded surfaces and regions.
    pub fn region(&self) -> &CompositeRegion {
        &self.region
    }
}

// ── CruciformPrism ──────────────────────────────────────────────────────────

/// **Generalised cruciform prism** (Serpent's `gcross`): the union of
/// rectangles `|h| < d_i`, `|v| < d_{n-1-i}` about `center`.
///
/// Port of `CruciformPrism` (`surface_composite.py`, lines 1372-1454).
/// Surfaces are `hmin_i, hmax_i, vmin_i, vmax_i` for each distance in turn
/// (the upstream `_surfnames` order). For an X axis "horizontal" is y and
/// "vertical" z; for Y they are x and z; for Z, x and y.
#[derive(Debug, Clone)]
pub struct CruciformPrism {
    region: CompositeRegion,
}

impl CruciformPrism {
    /// Build the prism. `distances` must all be positive and strictly
    /// monotonic, increasing or decreasing (upstream's `distances` setter).
    pub fn new(distances: &[f64], center: [f64; 2], axis: Axis) -> Result<Self, CompositeError> {
        if !distances.iter().all(|&d| d > 0.0) {
            return Err(CompositeError::new(
                "CruciformPrism",
                "distances must be positive",
            ));
        }
        let increasing = distances.windows(2).all(|w| w[1] > w[0]);
        let decreasing = distances.windows(2).all(|w| w[1] < w[0]);
        if !(increasing || decreasing) {
            return Err(CompositeError::new(
                "CruciformPrism",
                "distances must be monotonic",
            ));
        }
        let (h_axis, v_axis) = match axis {
            Axis::X => (Axis::Y, Axis::Z),
            Axis::Y => (Axis::X, Axis::Z),
            Axis::Z => (Axis::X, Axis::Y),
        };
        let [x0, y0] = center;
        let mut b = Builder::default();
        let mut idx = Vec::with_capacity(distances.len());
        for &d in distances {
            let hmin = b.add(axis_plane(h_axis, x0 - d));
            let hmax = b.add(axis_plane(h_axis, x0 + d));
            let vmin = b.add(axis_plane(v_axis, y0 - d));
            let vmax = b.add(axis_plane(v_axis, y0 + d));
            idx.push([hmin, hmax, vmin, vmax]);
        }
        let n = distances.len();
        let regions = (0..n)
            .map(|i| {
                and(vec![
                    pos(idx[i][0]),
                    neg(idx[i][1]),
                    pos(idx[n - 1 - i][2]),
                    neg(idx[n - 1 - i][3]),
                ])
            })
            .collect();
        Ok(Self {
            region: b.finish(or(regions)),
        })
    }

    /// The expanded surfaces and regions.
    pub fn region(&self) -> &CompositeRegion {
        &self.region
    }
}

// ── RectangularPrism ────────────────────────────────────────────────────────

/// **Infinite rectangular prism, optionally with rounded corners.**
///
/// Port of `RectangularPrism` (`surface_composite.py`, lines 1464-1585).
/// `width` runs along the first perpendicular axis and `height` along the
/// second (x and y for a Z prism; x and z for Y; y and z for X); `origin` is
/// the centre in those two. With `corner_radius > 0` the four corners are cut
/// by quarter cylinders: `prism & ~(corner_1 | ... | corner_4)` (`__neg__`,
/// lines 1572-1585).
///
/// Surfaces: `min_x1, max_x1, min_x2, max_x2`, then for rounded corners
/// `x1_min_x2_min, x1_min_x2_max, x1_max_x2_min, x1_max_x2_max, x1_min,
/// x1_max, x2_min, x2_max`.
#[derive(Debug, Clone)]
pub struct RectangularPrism {
    region: CompositeRegion,
}

impl RectangularPrism {
    /// Build the prism. `corner_radius = 0` gives square corners.
    pub fn new(
        width: f64,
        height: f64,
        axis: Axis,
        origin: [f64; 2],
        corner_radius: f64,
    ) -> Result<Self, CompositeError> {
        let (x1, x2) = match axis {
            Axis::X => (Axis::Y, Axis::Z),
            Axis::Y => (Axis::X, Axis::Z),
            Axis::Z => (Axis::X, Axis::Y),
        };
        let mut b = Builder::default();
        let min_x1 = b.add(axis_plane(x1, -width / 2.0 + origin[0]));
        let max_x1 = b.add(axis_plane(x1, width / 2.0 + origin[0]));
        let min_x2 = b.add(axis_plane(x2, -height / 2.0 + origin[1]));
        let max_x2 = b.add(axis_plane(x2, height / 2.0 + origin[1]));
        let mut prism = and(vec![pos(min_x1), neg(max_x1), pos(min_x2), neg(max_x2)]);

        if corner_radius > 0.0 {
            let rc = corner_radius;
            let c1_lo = origin[0] - width / 2.0 + rc;
            let c1_hi = origin[0] + width / 2.0 - rc;
            let c2_lo = origin[1] - height / 2.0 + rc;
            let c2_hi = origin[1] + height / 2.0 - rc;
            let x1_min_x2_min = b.add(axis_cylinder(axis, c1_lo, c2_lo, rc));
            let x1_min_x2_max = b.add(axis_cylinder(axis, c1_lo, c2_hi, rc));
            let x1_max_x2_min = b.add(axis_cylinder(axis, c1_hi, c2_lo, rc));
            let x1_max_x2_max = b.add(axis_cylinder(axis, c1_hi, c2_hi, rc));
            let x1_min = b.add(axis_plane(x1, -width / 2.0 + origin[0] + rc));
            let x1_max = b.add(axis_plane(x1, width / 2.0 + origin[0] - rc));
            let x2_min = b.add(axis_plane(x2, -height / 2.0 + origin[1] + rc));
            let x2_max = b.add(axis_plane(x2, height / 2.0 + origin[1] - rc));
            let corners = or(vec![
                and(vec![pos(x1_min_x2_min), neg(x1_min), neg(x2_min)]),
                and(vec![pos(x1_min_x2_max), neg(x1_min), pos(x2_max)]),
                and(vec![pos(x1_max_x2_min), pos(x1_max), neg(x2_min)]),
                and(vec![pos(x1_max_x2_max), pos(x1_max), pos(x2_max)]),
            ]);
            prism = and(vec![prism, not(corners)]);
        }
        Ok(Self {
            region: b.finish(prism),
        })
    }

    /// The expanded surfaces and regions.
    pub fn region(&self) -> &CompositeRegion {
        &self.region
    }
}
