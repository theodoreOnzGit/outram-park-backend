// SPDX-License-Identifier: GPL-3.0-only
// Copyright (C) 2026 OUTRAM PARK contributors
// Port of OpenFOAM-dev (OpenFOAM Foundation) commit 852527377c7e87e33d51b19ca8974b5db60af28b:
//   src/mesh/blockMesh/blockEdges/{blockEdge,lineEdge,arcEdge,polyLineEdge
//     (polyLine),splineEdge,BSplineEdge,lineDivide}
//   src/OpenFOAM/geometry/splines/{Spline,CatmullRomSpline,BSpline}
//   src/OpenFOAM/primitives/Tensor/TensorI.H (`det`, `inv`, `operator&`)
// Copyright (C) 2011-2025 OpenFOAM Foundation
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

//! Curved block edges for `blockMesh` (gh:#1461, epic gh:#1458).
//!
//! A `blockMeshDict` `edges` entry replaces the straight line between two block
//! vertices by a curve. Upstream registers `line`, `arc`, `polyLine`, `spline`
//! (Catmull-Rom) and `BSpline` in dev `852527377c7e`. ~~`project` and
//! `projectCurve` need `geometry` and are reported as not implemented~~
//! **CORRECTED 2026-11-11 (gh:#1462)**: they are ported, resolved against the
//! `geometry` by [`BlockEdge::resolve_with`] ([`crate::block_projection`]).
//!
//! Every curve answers one question, [`EdgeCurve::position`]: the point at the
//! edge parameter `lambda` in `[0, 1]`. The caller places the `n + 1`
//! parameters by the edge's grading as `lineDivide` does, and
//! [`crate::block_mesh`] blends the edge points into the block interior as
//! `block::createPoints` does.
//!
//! Coordinates are the **raw dictionary coordinates** (before `convertToMeters`
//! / `units`): upstream builds edges from the unscaled `vertices_` and scales
//! the finished points (`blockMeshCreate.C:223`).
//!
//! Transcendentals (`cos`, `sin`, `tan`, `acos`) go through
//! `petir::mathf::RealMath`; `sqrt` and `floor` are exact on `std`.
//!
//! ## Upstream quirks kept on purpose
//!
//! - `arc v0 v1 angle (axis)` converts `angle` to radians and *then* checks
//!   `0 < theta < 360` (`arcEdge.C`, `calc(theta, axis)`), so the bound is 360
//!   **radians**, not degrees. Kept; a larger angle is rejected as upstream does.
//! - The `spline` / `BSpline` position is not the raw segment polynomial but an
//!   iteration (`Spline::position`, at most 16 iterations, tolerance
//!   `rootSmall`) that re-parameterises so that `lambda` is the fraction of
//!   straight-line distance from the start, as the grading requires.
//! - Optional trailing start/end tangents of `spline` / `BSpline` are read and
//!   discarded by the parser, as upstream does.

use std::sync::Arc;

use petir::mathf::RealMath;

use outram_foam_basic_lib::primitives::Vector3;

use crate::block_projection::{Geometry, ProjectEdge, ProjectionFlavour};
use crate::MeshError;

/// OpenFOAM `vSmall` for `double` (`numeric_limits<double>::min()`).
const V_SMALL: f64 = f64::MIN_POSITIVE;
/// OpenFOAM `small` for `double` (`numeric_limits<double>::epsilon()`).
const SMALL: f64 = f64::EPSILON;
/// OpenFOAM `vGreat` for `double` (`max()/10`).
const V_GREAT: f64 = f64::MAX / 10.0;

/// What an `edges` entry says about the curve, before the vertex positions are
/// known.
#[derive(Debug, Clone, PartialEq)]
pub enum BlockEdgeKind {
    /// `line v0 v1`: a straight edge. Upstream still treats it as a listed
    /// (curved-counted) edge in `block::createPoints`.
    Line,
    /// `arc v0 v1 (x y z)`: the circle through both vertices and this point.
    ArcThroughPoint(Vector3),
    /// `arc v0 v1 angle (ax ay az)`: angle in degrees about an axis (a helix
    /// when the chord has an axial component).
    ArcAngleAxis {
        /// Angle `[deg]`.
        angle_deg: f64,
        /// Axis (not necessarily unit length in the dictionary).
        axis: Vector3,
    },
    /// `polyLine v0 v1 ((x y z) ...)`: straight segments through the points.
    PolyLine(Vec<Vector3>),
    /// `spline v0 v1 ((x y z) ...)`: Catmull-Rom spline through the points.
    Spline(Vec<Vector3>),
    /// `BSpline v0 v1 ((x y z) ...)`: uniform cubic B-spline.
    BSpline(Vec<Vector3>),
    /// `project v0 v1 (surface ...)`: the straight edge projected onto the
    /// named `geometry` surfaces (gh:#1462).
    Project(Vec<String>),
    /// `projectCurve v0 v1 (surface ...)` (same algorithm as `project` unless
    /// an `extrudedCircle` is listed, which is not ported; gh:#1462).
    ProjectCurve(Vec<String>),
}

/// One parsed `edges` entry: the two block-vertex labels and the curve kind.
#[derive(Debug, Clone, PartialEq)]
pub struct BlockEdge {
    /// Start vertex label (index into the `vertices` list).
    pub start: usize,
    /// End vertex label.
    pub end: usize,
    /// The curve.
    pub kind: BlockEdgeKind,
}

impl BlockEdge {
    /// `blockEdge::compare(start, end)`: `1` when this edge runs `start` ->
    /// `end`, `-1` when it runs the other way, `0` when it is a different edge.
    pub fn compare(&self, start: usize, end: usize) -> i32 {
        if self.start == start && self.end == end {
            1
        } else if self.start == end && self.end == start {
            -1
        } else {
            0
        }
    }

    /// Resolve the curve against the (raw) vertex positions.
    ///
    /// # Errors
    /// [`MeshError::Construction`] for a vertex label out of range, an arc point
    /// on the line of the edge (`arcEdge::calc`), or an arc angle outside
    /// `(0, 360)` (see the module docs on the radian quirk).
    pub fn resolve(&self, vertices: &[Vector3]) -> Result<EdgeCurve, MeshError> {
        self.resolve_with(vertices, &Arc::new(Geometry::default()))
    }

    /// As [`resolve`](Self::resolve), with the `geometry` that `project` /
    /// `projectCurve` edges name their surfaces in (gh:#1462).
    ///
    /// # Errors
    /// As [`resolve`](Self::resolve), plus [`MeshError::DictParse`] for a
    /// surface name not in `geometry`.
    pub fn resolve_with(
        &self,
        vertices: &[Vector3],
        geometry: &Arc<Geometry>,
    ) -> Result<EdgeCurve, MeshError> {
        let get = |i: usize| -> Result<Vector3, MeshError> {
            vertices.get(i).copied().ok_or_else(|| {
                MeshError::Construction(format!(
                    "edge references vertex {i} but only {} vertices defined",
                    vertices.len()
                ))
            })
        };
        let p0 = get(self.start)?;
        let p1 = get(self.end)?;
        let with_ends = |inner: &[Vector3]| -> Vec<Vector3> {
            let mut all = Vec::with_capacity(inner.len() + 2);
            all.push(p0);
            all.extend_from_slice(inner);
            all.push(p1);
            all
        };
        Ok(match &self.kind {
            BlockEdgeKind::Line => EdgeCurve::Line { p0, p1 },
            BlockEdgeKind::ArcThroughPoint(pa) if geometry.flavour == ProjectionFlavour::Esi => {
                EdgeCurve::ArcEsi(ArcEdgeEsi::through_point(self.start, self.end, p0, p1, *pa)?)
            }
            BlockEdgeKind::ArcThroughPoint(pa) => {
                EdgeCurve::Arc(ArcEdge::through_point(self.start, self.end, p0, p1, *pa)?)
            }
            BlockEdgeKind::ArcAngleAxis { angle_deg, axis } => EdgeCurve::Arc(ArcEdge::from_angle(
                self.start,
                self.end,
                p0,
                p1,
                deg_to_rad(*angle_deg),
                normalised(*axis),
            )?),
            BlockEdgeKind::PolyLine(inner) => EdgeCurve::PolyLine(PolyLine::new(with_ends(inner))),
            BlockEdgeKind::Spline(inner) if geometry.flavour == ProjectionFlavour::Esi => {
                EdgeCurve::CatmullRomEsi(PolyLine::new(with_ends(inner)))
            }
            BlockEdgeKind::BSpline(inner) if geometry.flavour == ProjectionFlavour::Esi => {
                EdgeCurve::BSplineEsi(PolyLine::new(with_ends(inner)))
            }
            BlockEdgeKind::Spline(inner) => EdgeCurve::CatmullRom(with_ends(inner)),
            BlockEdgeKind::BSpline(inner) => EdgeCurve::BSpline(with_ends(inner)),
            BlockEdgeKind::Project(names) | BlockEdgeKind::ProjectCurve(names) => {
                let ids = geometry.surface_ids(names)?;
                EdgeCurve::Project(ProjectEdge::new(geometry.clone(), ids, p0, p1))
            }
        })
    }
}

/// `units::degrees.toStandard(deg)`: `deg * (pi/180)`.
fn deg_to_rad(deg: f64) -> f64 {
    deg * (std::f64::consts::PI / 180.0)
}

/// OpenFOAM `normalised(v)`: `v/|v|`, or zero for a zero vector.
fn normalised(v: Vector3) -> Vector3 {
    let m = v.mag();
    if m > 0.0 {
        v / m
    } else {
        Vector3::ZERO
    }
}

/// A resolved edge curve (enum dispatch, no trait objects).
#[derive(Debug, Clone, PartialEq)]
pub enum EdgeCurve {
    /// `lineEdge`: `p0 + lambda*(p1 - p0)`.
    Line {
        /// Start point.
        p0: Vector3,
        /// End point.
        p1: Vector3,
    },
    /// `arcEdge`.
    Arc(ArcEdge),
    /// `polyLineEdge`.
    PolyLine(PolyLine),
    /// `splineEdge` (Catmull-Rom); all knots including both end points.
    CatmullRom(Vec<Vector3>),
    /// `BSplineEdge`; all knots including both end points.
    BSpline(Vec<Vector3>),
    /// `projectEdge` / `projectCurveEdge`.
    Project(ProjectEdge),
    /// `splineEdge` as OpenFOAM.com v2506 evaluates it (gh:#1364): the raw
    /// `CatmullRomSpline::position(mu)` on the `polyLine` chord-length
    /// parameter, with no distance re-parameterisation. Chosen by
    /// [`BlockEdgeKind::resolve_with`] when the geometry flavour is
    /// [`ProjectionFlavour::Esi`]; the default (dev) form stays
    /// [`EdgeCurve::CatmullRom`].
    CatmullRomEsi(PolyLine),
    /// `BSplineEdge` as OpenFOAM.com v2506 evaluates it (see
    /// [`EdgeCurve::CatmullRomEsi`]).
    BSplineEsi(PolyLine),
    /// `arcEdge` through a mid point as OpenFOAM.com v2506 builds it (see
    /// [`ArcEdgeEsi`]); the default (dev) form stays [`EdgeCurve::Arc`].
    ArcEsi(ArcEdgeEsi),
}

impl EdgeCurve {
    /// The point at edge parameter `lambda` (`blockEdge::position(lambda)`).
    pub fn position(&self, lambda: f64) -> Vector3 {
        match self {
            EdgeCurve::Line { p0, p1 } => *p0 + (*p1 - *p0) * lambda,
            EdgeCurve::Arc(a) => a.position(lambda),
            EdgeCurve::PolyLine(p) => p.position(lambda),
            EdgeCurve::CatmullRom(k) => spline_position(k, SplineKind::CatmullRom, lambda),
            EdgeCurve::BSpline(k) => spline_position(k, SplineKind::BSpline, lambda),
            EdgeCurve::Project(p) => p.position(lambda),
            EdgeCurve::CatmullRomEsi(p) => p.spline_position_esi(SplineKind::CatmullRom, lambda),
            EdgeCurve::BSplineEsi(p) => p.spline_position_esi(SplineKind::BSpline, lambda),
            EdgeCurve::ArcEsi(a) => a.position(lambda),
        }
    }

    /// `blockEdge::position(const scalarList&)`: the points at each parameter.
    /// A projected edge iterates over the whole list (`projectEdge`).
    pub fn positions(&self, lambdas: &[f64]) -> Vec<Vector3> {
        match self {
            EdgeCurve::Project(p) => p.positions(lambdas),
            _ => lambdas.iter().map(|&l| self.position(l)).collect(),
        }
    }
}

// ---------------------------------------------------------------------------
//  arcEdge
// ---------------------------------------------------------------------------

/// A circular (or helical) arc: `arcEdge.C`.
#[derive(Debug, Clone, PartialEq)]
pub struct ArcEdge {
    p0: Vector3,
    centre: Vector3,
    axis: Vector3,
    /// Swept angle `[rad]`.
    theta: f64,
    /// Axial advance over the whole arc (zero for a planar arc).
    length: f64,
}

/// Row-major 3x3 tensor `[[xx, xy, xz], [yx, yy, yz], [zx, zy, zz]]`.
type Tensor = [[f64; 3]; 3];

fn row(t: &Tensor, i: usize) -> Vector3 {
    Vector3::new(t[i][0], t[i][1], t[i][2])
}

/// `det(tensor)` exactly as `TensorI.H`.
fn det(t: &Tensor) -> f64 {
    let (xx, xy, xz) = (t[0][0], t[0][1], t[0][2]);
    let (yx, yy, yz) = (t[1][0], t[1][1], t[1][2]);
    let (zx, zy, zz) = (t[2][0], t[2][1], t[2][2]);
    xx * yy * zz + xy * yz * zx + xz * yx * zy - xx * yz * zy - xy * yx * zz - xz * yy * zx
}

/// `inv(tensor, det)` exactly as `TensorI.H`.
fn inv(t: &Tensor, dett: f64) -> Tensor {
    let (xx, xy, xz) = (t[0][0], t[0][1], t[0][2]);
    let (yx, yy, yz) = (t[1][0], t[1][1], t[1][2]);
    let (zx, zy, zz) = (t[2][0], t[2][1], t[2][2]);
    [
        [
            (yy * zz - zy * yz) / dett,
            (xz * zy - xy * zz) / dett,
            (xy * yz - xz * yy) / dett,
        ],
        [
            (zx * yz - yx * zz) / dett,
            (xx * zz - xz * zx) / dett,
            (yx * xz - xx * yz) / dett,
        ],
        [
            (yx * zy - yy * zx) / dett,
            (xy * zx - xx * zy) / dett,
            (xx * yy - yx * xy) / dett,
        ],
    ]
}

/// `tensor & vector` as `TensorI.H`.
fn tensor_dot(t: &Tensor, v: Vector3) -> Vector3 {
    Vector3::new(
        t[0][0] * v.x + t[0][1] * v.y + t[0][2] * v.z,
        t[1][0] * v.x + t[1][1] * v.y + t[1][2] * v.z,
        t[2][0] * v.x + t[2][1] * v.y + t[2][2] * v.z,
    )
}

impl ArcEdge {
    /// `arcEdge::calc(const point& pA)`: the arc through `p0`, `pa`, `p1`.
    ///
    /// # Errors
    /// [`MeshError::Construction`] when `pa` lies on the line of the edge.
    pub fn through_point(
        start: usize,
        end: usize,
        p0: Vector3,
        p1: Vector3,
        pa: Vector3,
    ) -> Result<Self, MeshError> {
        let dx = pa - p0;
        let dy = pa - p1;
        let dz = dx.cross(dy);
        let dps: Tensor = [[dx.x, dx.y, dx.z], [dy.x, dy.y, dy.z], [dz.x, dz.y, dz.z]];
        let pm_x = (p0 + pa) / 2.0;
        let pm_y = (p1 + pa) / 2.0;
        let d = det(&dps);
        if d < V_SMALL {
            return Err(MeshError::Construction(format!(
                "arc point ({} {} {}) lies on the line of edge ({start} {end})",
                pa.x, pa.y, pa.z
            )));
        }
        let rhs = Vector3::new(
            row(&dps, 0).dot(pm_x),
            row(&dps, 1).dot(pm_y),
            row(&dps, 2).dot(pa),
        );
        let c = tensor_dot(&inv(&dps, d), rhs);

        let r0 = p0 - c;
        let r1 = p1 - c;
        let cos_t = r0.dot(r1) / (r0.mag() * r1.mag());
        let mut t = cos_t.min(1.0).max(-1.0).r_acos();
        if r0.cross(r1).dot(dz) > 0.0 {
            t = 2.0 * std::f64::consts::PI - t;
        }
        Ok(ArcEdge {
            p0,
            centre: c,
            axis: -normalised(dz),
            theta: t,
            length: 0.0,
        })
    }

    /// `arcEdge::calc(theta, axis)`; `theta` in radians, `axis` a unit vector.
    ///
    /// # Errors
    /// [`MeshError::Construction`] unless `0 < theta < 360` (the upstream
    /// check, applied to the radian value; see the module docs).
    pub fn from_angle(
        start: usize,
        end: usize,
        p0: Vector3,
        p1: Vector3,
        theta: f64,
        axis: Vector3,
    ) -> Result<Self, MeshError> {
        if 0.0 >= theta || theta >= 360.0 {
            return Err(MeshError::Construction(format!(
                "arc angle for edge ({start} {end}) must take a value between 0 and 360 degrees"
            )));
        }
        let dp = p1 - p0;
        let p_m = (p0 + p1) / 2.0;
        let r_m = normalised(dp.cross(axis));
        let l = dp.dot(axis);
        let chord = dp - axis * l;
        let mag_chord = chord.mag();
        let centre = p_m - (axis * l) / 2.0 - ((r_m * mag_chord) / 2.0) / (theta / 2.0).r_tan();
        Ok(ArcEdge {
            p0,
            centre,
            axis,
            theta,
            length: l,
        })
    }

    /// `arcEdge::position(lambda)`.
    pub fn position(&self, lambda: f64) -> Vector3 {
        let t = self.theta * lambda;
        let r1 = self.p0 - self.centre;
        let r2 = self.axis.cross(r1);
        self.centre + r1 * t.r_cos() + r2 * t.r_sin() + (self.axis * self.length) * lambda
    }

    /// Arc centre (raw dictionary coordinates).
    pub fn centre(&self) -> Vector3 {
        self.centre
    }

    /// Swept angle `[rad]`.
    pub fn theta(&self) -> f64 {
        self.theta
    }
}

/// OpenFOAM.com v2506 `SMALL` for `double`.
const ESI_SMALL: f64 = 1e-15;
/// OpenFOAM.com v2506 `ROOTVSMALL` for `double`.
const ESI_ROOT_V_SMALL: f64 = 1e-150;

/// `arcEdge` as OpenFOAM.com v2506 builds it from a mid point (gh:#1364):
/// `calcFromMidPoint` (centre from the three points, `acos` angle, axis from
/// `r1 ^ r3`) and a `coordSystem::cylindrical` whose rotation tensor has the
/// columns `(ax2, ax1 ^ ax2, ax1)` (`coordinateRotations::axes::rotation`,
/// `E3_E1`). The dev form ([`ArcEdge`], swept angle plus axis) is a different
/// construction of the same circle: equal to round-off, not bit for bit.
#[derive(Debug, Clone, PartialEq)]
pub struct ArcEdgeEsi {
    p0: Vector3,
    p1: Vector3,
    centre: Vector3,
    radius: f64,
    angle: f64,
    /// Rotation tensor of the cylindrical system (columns are the local axes).
    rot: Tensor,
}

impl ArcEdgeEsi {
    /// `arcEdge::calcFromMidPoint(p1, p3, p2)` of v2506.
    ///
    /// # Errors
    /// [`MeshError::Construction`] when the three points are collinear
    /// (`|denom| < rootVSmall`, upstream aborts).
    pub fn through_point(
        start: usize,
        end: usize,
        p1: Vector3,
        p3: Vector3,
        p2: Vector3,
    ) -> Result<Self, MeshError> {
        let a = p2 - p1;
        let b = p3 - p1;
        let asqr = a.dot(a);
        let bsqr = b.dot(b);
        let adotb = a.dot(b);
        let denom = asqr * bsqr - adotb * adotb;
        if denom.abs() < ESI_ROOT_V_SMALL {
            return Err(MeshError::Construction(format!(
                "arc ({start} {end}): the three points are collinear (denominator {denom})"
            )));
        }
        let fact = 0.5 * (bsqr - adotb) / denom;
        let centre = p1 + a * 0.5 + a.cross(b).cross(a) * fact;
        let r1 = p1 - centre;
        let r2 = p2 - centre;
        let r3 = p3 - centre;
        let mag1 = r1.mag();
        let mag3 = r3.mag();
        let mut arc_axis = r1.cross(r3);
        let radius = r3.mag();
        let mut angle = ((r1.dot(r3)) / (mag1 * mag3)).r_acos();
        if r1.cross(r2).dot(r1.cross(r3)) < 0.0 {
            angle = 2.0 * std::f64::consts::PI - angle;
        }
        if angle <= std::f64::consts::PI {
            if arc_axis.mag() / (mag1 * mag3) < 0.001 {
                arc_axis = r1.cross(r2);
            }
        } else {
            arc_axis = -arc_axis;
        }
        // coordinateRotations::axes::rotation(axis, r1, E3_E1)
        let mag_axis1 = arc_axis.mag();
        let ax1 = arc_axis / mag_axis1;
        let mut ax2 = r1;
        ax2 = ax2 - ax1 * ax2.dot(ax1);
        let mag_axis2 = ax2.mag();
        let ax2 = ax2 / mag_axis2;
        let ax3 = ax1.cross(ax2);
        let rot: Tensor = [[ax2.x, ax3.x, ax1.x], [ax2.y, ax3.y, ax1.y], [ax2.z, ax3.z, ax1.z]];
        Ok(ArcEdgeEsi { p0: p1, p1: p3, centre, radius, angle, rot })
    }

    /// `arcEdge::position(lambda)` of v2506: the end tests, then
    /// `cs.globalPosition(vector(radius, lambda*angle, 0))` =
    /// `(R & (r cos t, r sin t, 0)) + origin`.
    pub fn position(&self, lambda: f64) -> Vector3 {
        if lambda < ESI_SMALL {
            return self.p0;
        } else if lambda >= 1.0 - ESI_SMALL {
            return self.p1;
        }
        let theta = lambda * self.angle;
        let local = Vector3::new(self.radius * theta.r_cos(), self.radius * theta.r_sin(), 0.0);
        tensor_dot(&self.rot, local) + self.centre
    }
}

// ---------------------------------------------------------------------------
//  polyLine
// ---------------------------------------------------------------------------

/// `polyLine`: straight segments through the knots, parameterised by
/// normalised cumulative length.
#[derive(Debug, Clone, PartialEq)]
pub struct PolyLine {
    points: Vec<Vector3>,
    param: Vec<f64>,
}

impl PolyLine {
    /// `polyLine::calcParam` over all knots (end points included).
    pub fn new(points: Vec<Vector3>) -> Self {
        let mut param = vec![0.0; points.len()];
        if !param.is_empty() {
            for i in 1..param.len() {
                param[i] = param[i - 1] + (points[i] - points[i - 1]).mag();
            }
            let last = param.len() - 1;
            let line_length = param[last];
            for p in param.iter_mut().take(last).skip(1) {
                *p /= line_length;
            }
            param[last] = 1.0;
        }
        PolyLine { points, param }
    }

    /// `polyLine::position(mu)`.
    pub fn position(&self, mu: f64) -> Vector3 {
        if mu < SMALL {
            return self.points[0];
        } else if mu > 1.0 - SMALL {
            return self.points[self.points.len() - 1];
        }
        // localParameter(lambda)
        let mut segment = 1usize;
        while self.param[segment] < mu {
            segment += 1;
        }
        segment -= 1;
        let lambda = (mu - self.param[segment]) / (self.param[segment + 1] - self.param[segment]);
        self.position_in_segment(segment, lambda)
    }

    /// OpenFOAM.com v2506 `CatmullRomSpline::position(mu)` /
    /// `BSpline::position(mu)` (both derive from `polyLine`): the end-point
    /// tests, `localParameter`, then the segment polynomial in v2506's
    /// operation order. There is no distance iteration, so the parameter is the
    /// chord-length fraction of the knot polygon, not the straight-line
    /// distance fraction of the dev form ([`spline_position`]).
    fn spline_position_esi(&self, kind: SplineKind, mu: f64) -> Vector3 {
        let n = self.points.len();
        if mu < ESI_SMALL {
            return self.points[0];
        } else if mu > 1.0 - ESI_SMALL {
            return self.points[n - 1];
        }
        let (segment, lam) = self.local_parameter(mu);
        let n_segments = n - 1;
        let p0 = self.points[segment];
        let p1 = self.points[segment + 1];
        match kind {
            SplineKind::CatmullRom => {
                if lam <= 0.0 {
                    return p0;
                } else if lam >= 1.0 {
                    return p1;
                }
            }
            SplineKind::BSpline => {
                if segment == 0 && lam <= 0.0 {
                    return p0;
                } else if segment == n_segments - 1 && lam >= 1.0 {
                    return p1;
                }
            }
        }
        let e0 = if segment == 0 { p0 * 2.0 - p1 } else { self.points[segment - 1] };
        let e1 = if segment + 1 == n_segments { p1 * 2.0 - p0 } else { self.points[segment + 2] };
        let cubic = e0 * -1.0 + p0 * 3.0 - p1 * 3.0 + e1;
        match kind {
            SplineKind::CatmullRom => {
                let t2 = (e0 * 2.0 - p0 * 5.0 + p1 * 4.0 - e1) + cubic * lam;
                let t1 = (e0 * -1.0 + p1) + t2 * lam;
                ((p0 * 2.0) + t1 * lam) * 0.5
            }
            SplineKind::BSpline => {
                let t2 = (e0 * 3.0 - p0 * 6.0 + p1 * 3.0) + cubic * lam;
                let t1 = (e0 * -3.0 + p1 * 3.0) + t2 * lam;
                ((e0 + p0 * 4.0 + p1) + t1 * lam) * (1.0 / 6.0)
            }
        }
    }

    /// `polyLine::localParameter(lambda)`: the segment and the local `[0, 1]`
    /// parameter (callers have excluded the end points).
    fn local_parameter(&self, mu: f64) -> (usize, f64) {
        let mut segment = 1usize;
        while self.param[segment] < mu {
            segment += 1;
        }
        segment -= 1;
        (segment, (mu - self.param[segment]) / (self.param[segment + 1] - self.param[segment]))
    }

    fn position_in_segment(&self, segment: usize, mu: f64) -> Vector3 {
        if segment > self.points.len() - 1 {
            return self.points[self.points.len() - 1];
        }
        let p0 = self.points[segment];
        let p1 = self.points[segment + 1];
        if mu <= 0.0 {
            p0
        } else if mu >= 1.0 {
            p1
        } else {
            self.points[segment] + (p1 - p0) * mu
        }
    }
}

// ---------------------------------------------------------------------------
//  Spline (Catmull-Rom and B-spline)
// ---------------------------------------------------------------------------

#[derive(Clone, Copy)]
enum SplineKind {
    CatmullRom,
    BSpline,
}

/// `CatmullRomSpline::position(segmenti, segmentLambda)` /
/// `BSpline::position(segmenti, segmentLambda)`.
fn spline_segment(knots: &[Vector3], kind: SplineKind, segmenti: i64, lambda_arg: f64) -> Vector3 {
    let n_segments = (knots.len() - 1) as i64;
    if segmenti < 0 {
        return knots[0];
    }
    if segmenti >= n_segments {
        return knots[knots.len() - 1];
    }
    let s = segmenti as usize;
    let p0 = knots[s];
    let p1 = knots[s + 1];
    let p_minus1 = if segmenti == 0 {
        p0 * 2.0 - p1
    } else {
        knots[s - 1]
    };
    let p2 = if segmenti == n_segments - 1 {
        p1 * 2.0 - p0
    } else {
        knots[s + 2]
    };
    let l = lambda_arg.max(0.0).min(1.0);
    match kind {
        SplineKind::CatmullRom => {
            let a = -p_minus1 + p0 * 3.0 - p1 * 3.0 + p2;
            let b = p_minus1 * 2.0 - p0 * 5.0 + p1 * 4.0 - p2;
            let c = -p_minus1 + p1;
            let d = p0 * 2.0;
            ((((a * l) + b) * l + c) * l + d) / 2.0
        }
        SplineKind::BSpline => {
            let a = -p_minus1 + p0 * 3.0 - p1 * 3.0 + p2;
            let b = p_minus1 * 3.0 - p0 * 6.0 + p1 * 3.0;
            let c = -p_minus1 * 3.0 + p1 * 3.0;
            let d = p_minus1 + p0 * 4.0 + p1;
            ((((a * l) + b) * l + c) * l + d) / 6.0
        }
    }
}

/// `Spline<Derived>::position(lambda)`: iterate the segment parameter until the
/// straight-line distance fraction from the start matches `lambda`.
fn spline_position(knots: &[Vector3], kind: SplineKind, lambda: f64) -> Vector3 {
    let start = knots[0];
    let end = knots[knots.len() - 1];
    if lambda <= 0.0 {
        return start;
    }
    if lambda >= 1.0 {
        return end;
    }
    let n_segments = (knots.len() - 1) as f64;
    let evaluate = |lam: f64| -> Vector3 {
        let l = lam * n_segments;
        let segmenti = l.floor();
        let segment_lambda = l - segmenti;
        spline_segment(knots, kind, segmenti as i64, segment_lambda)
    };
    let tol = SMALL.sqrt();
    let mut lambda_star = lambda;
    let mut error = V_GREAT;
    let mut iter = 0;
    while error > tol && iter < 16 {
        let p = evaluate(lambda_star);
        let dist0 = (p - start).mag();
        let dist1 = (p - end).mag();
        let lambda_dist = dist0 / (dist0 + dist1);
        error = (lambda - lambda_dist).abs();
        let below = lambda < lambda_dist;
        let lambda_star0 = if below { 0.0 } else { lambda_star };
        let lambda_star1 = if below { lambda_star } else { 1.0 };
        let lambda_dist0 = if below { 0.0 } else { lambda_dist };
        let lambda_dist1 = if below { lambda_dist } else { 1.0 };
        let x = (lambda - lambda_dist0) / (lambda_dist1 - lambda_dist0).max(V_SMALL);
        lambda_star = (1.0 - x) * lambda_star0 + x * lambda_star1;
        iter += 1;
    }
    evaluate(lambda_star)
}
