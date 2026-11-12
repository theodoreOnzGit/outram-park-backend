// SPDX-License-Identifier: AGPL-3.0-only
// Copyright (C) 2026 OUTRAM PARK contributors
//
// Ported from OpenMC (https://github.com/openmc-dev/openmc, MIT licence,
// Copyright (c) 2011-2026 Massachusetts Institute of Technology, UChicago
// Argonne LLC and OpenMC contributors; see LICENSE.openmc):
//   openmc/model/surface_composite.py at commit 8f79d9d0: Polygon
//   (lines 900-1369): `__init__`, `_normals`, `_equations`,
//   `_validate_points`, `_group_simplices`, `_get_convex_hull_surfs`,
//   `_decompose_polygon_into_convex_sets`.
//
// This file is part of OUTRAM PARK. AGPL-3.0-only; see LICENSE.

//! [`Polygon`]: a closed path of points, concave allowed, extruded along the
//! missing axis (or revolved, for the `rz` basis).

use std::collections::BTreeMap;

use super::{and, neg, not, or, pos, Builder, CompositeError, CompositeRegion, T};
use crate::geometry::cell::RegionToken;
use crate::geometry::surface::{Plane, SurfaceKind, XPlane, YPlane, ZCone, ZCylinder, ZPlane};

/// The 2-D basis of a [`Polygon`] (upstream `basis`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PolygonBasis {
    /// Points are `(x, y)`; infinite along z.
    Xy,
    /// Points are `(y, z)`; infinite along x.
    Yz,
    /// Points are `(x, z)`; infinite along y.
    Xz,
    /// Points are `(r, z)` with `r = sqrt(x^2 + y^2)`; revolved about z.
    Rz,
}

/// **Polygon** from a closed path of points, concave allowed.
///
/// Port of `Polygon` (`openmc/model/surface_composite.py`, lines 900-1369).
/// The polygon is split into convex pieces; each piece is the intersection of
/// the half-spaces of its hull facets, and the polygon is their union
/// (`__init__`, lines 929-959; `__neg__`, line 961). Facets become planes for
/// the Cartesian bases and, for `rz`, z-planes, z-cylinders and one-sided
/// z-cones (`_get_convex_hull_surfs`, lines 1204-1270, ported line by line).
///
/// # One deliberate departure: the triangulation
///
/// Upstream triangulates with qhull (`scipy.spatial.Delaunay(points,
/// qhull_options='QJ')`, a joggled Delaunay triangulation), inserts edge
/// midpoints until every polygon edge is in it, and keeps the triangles whose
/// centroid is inside the polygon (`_constrain_triangulation`,
/// `_decompose_polygon_into_convex_sets`). qhull is not ported. This port
/// triangulates by **ear clipping**, which is constrained by construction:
/// every polygon edge is a triangle edge and every triangle is inside, so the
/// midpoint loop and the centroid test have nothing to do. The convex grouping
/// (`_group_simplices`, lines 1149-1202) and the facet-to-surface mapping are
/// then upstream's. **Consequence:** the convex pieces, and so the list of
/// internal facets, can differ from upstream's; the region (the set of points
/// inside) does not, because both are unions of triangles covering exactly
/// the polygon. `tests/composite_surfaces_vs_openmc.rs` checks that set
/// against OpenMC's own Python.
#[derive(Debug, Clone)]
pub struct Polygon {
    region: CompositeRegion,
    points: Vec<[f64; 2]>,
    pieces: usize,
}

impl Polygon {
    /// Build the polygon. Errors as upstream for fewer than three points,
    /// duplicate points or a self-intersecting path; a path given clockwise
    /// is reversed, and a closing point equal to the first is dropped.
    pub fn new(points: &[[f64; 2]], basis: PolygonBasis) -> Result<Self, CompositeError> {
        let points = validate_points(points)?;
        let triangles = ear_clip(&points)?;
        let groups = decompose(&points, &triangles);

        let boundary_eqns = boundary_equations(&points);
        let mut b = Builder::default();
        let mut pieces = Vec::with_capacity(groups.len());
        for group in &groups {
            let mut idx: Vec<usize> = group.iter().flat_map(|&t| triangles[t]).collect();
            idx.sort_unstable();
            idx.dedup();
            let pts: Vec<[f64; 2]> = idx.iter().map(|&i| points[i]).collect();
            let mut halves = Vec::new();
            for eq in hull_equations(&pts) {
                let on_boundary = boundary_eqns.iter().any(|e| allclose(&eq, e));
                halves.push(facet_half_space(&mut b, eq, basis, on_boundary)?);
            }
            pieces.push(and(halves));
        }
        Ok(Self {
            pieces: pieces.len(),
            region: b.finish(or(pieces)),
            points,
        })
    }

    /// The validated points, counter-clockwise.
    pub fn points(&self) -> &[[f64; 2]] {
        &self.points
    }

    /// Number of convex pieces the polygon was split into.
    pub fn convex_piece_count(&self) -> usize {
        self.pieces
    }

    /// The expanded surfaces and regions.
    pub fn region(&self) -> &CompositeRegion {
        &self.region
    }
}

/// `np.allclose(a, b)`: `|a - b| <= 1e-8 + 1e-5 |b|` element-wise.
fn allclose(a: &[f64], b: &[f64]) -> bool {
    a.iter()
        .zip(b)
        .all(|(x, y)| (x - y).abs() <= 1e-8 + 1e-5 * y.abs())
}

/// z-component of `a x b` for 2-D vectors (`np.cross(...)[-1]` on padded points).
fn cross2(a: [f64; 2], b: [f64; 2]) -> f64 {
    a[0] * b[1] - a[1] * b[0]
}

fn sub2(a: [f64; 2], b: [f64; 2]) -> [f64; 2] {
    [a[0] - b[0], a[1] - b[1]]
}

/// `_validate_points` (lines 1022-1131).
fn validate_points(input: &[[f64; 2]]) -> Result<Vec<[f64; 2]>, CompositeError> {
    let err = |m: &str| CompositeError::new("Polygon", m);
    let mut points = input.to_vec();
    if points.len() >= 2 && allclose(&points[0], &points[points.len() - 1]) {
        points.pop();
    }
    if points.len() < 3 {
        return Err(err("points must have at least 3 points"));
    }
    for i in 0..points.len() {
        for j in (i + 1)..points.len() {
            if points[i] == points[j] {
                return Err(err("Duplicate points were detected in the Polygon input"));
            }
        }
    }
    let n = points.len();
    let mut signed = 0.0;
    for i in 0..n {
        let prev = points[(i + n - 1) % n][0];
        let next = points[(i + 1) % n][0];
        signed += points[i][1] * (prev - next);
    }
    if signed < 0.0 {
        points.reverse();
    }

    let near_zero = |v: f64| v.abs() <= 1e-8; // np.isclose(v, 0)
    for i in 0..n {
        let p0 = points[i];
        let p1 = points[(i + 1) % n];
        for j in (i + 1)..n {
            let p2 = points[j];
            let p3 = points[(j + 1) % n];
            let cp0 = cross2(sub2(p3, p0), sub2(p2, p0));
            let cp1 = cross2(sub2(p3, p1), sub2(p2, p1));
            let cp2 = cross2(sub2(p1, p2), sub2(p0, p2));
            let cp3 = cross2(sub2(p1, p3), sub2(p0, p3));
            let cps = [[cp0, cp1], [cp2, cp3]];
            let zeros = cps.map(|row| row.map(|v| usize::from(near_zero(v))));
            let num_zeros: usize = zeros.iter().flatten().sum();
            let bad = match num_zeros {
                0 => cps.iter().all(|row| row[0] * row[1] < 0.0),
                1 => {
                    // The row with no zero is the segment the other's
                    // collinear point is tested against.
                    let row = if zeros[0][0] + zeros[0][1] == 0 {
                        cps[0]
                    } else {
                        cps[1]
                    };
                    row[0] * row[1] < 0.0
                }
                // 2: segments share an end point; 3: upstream only warns.
                2 | 3 => false,
                _ => {
                    let (xmin1, xmax1) = (p0[0].min(p1[0]), p0[0].max(p1[0]));
                    let (ymin1, ymax1) = (p0[1].min(p1[1]), p0[1].max(p1[1]));
                    let (xmin2, xmax2) = (p2[0].min(p3[0]), p2[0].max(p3[0]));
                    let (ymin2, ymax2) = (p2[1].min(p3[1]), p2[1].max(p3[1]));
                    let xlap = xmin1 < xmax2 && xmin2 < xmax1;
                    let ylap = ymin1 < ymax2 && ymin2 < ymax1;
                    xlap || ylap
                }
            };
            if bad {
                return Err(err("Polygon cannot be self-intersecting"));
            }
        }
    }
    Ok(points)
}

/// Orientation of `c` relative to `a -> b`: positive when counter-clockwise.
fn orient(a: [f64; 2], b: [f64; 2], c: [f64; 2]) -> f64 {
    cross2(sub2(b, a), sub2(c, a))
}

/// Whether `p` is inside or on the counter-clockwise triangle `a, b, c`.
fn in_closed_triangle(p: [f64; 2], a: [f64; 2], b: [f64; 2], c: [f64; 2]) -> bool {
    orient(a, b, p) >= 0.0 && orient(b, c, p) >= 0.0 && orient(c, a, p) >= 0.0
}

/// **Ear-clipping triangulation** of a simple counter-clockwise polygon (NEW
/// WORK standing in for qhull's Delaunay, see [`Polygon`]). Returns
/// counter-clockwise triangles as vertex indices. A vertex collinear with
/// both neighbours is dropped from the chain when no ear remains; it lies on
/// an edge and bounds no area.
fn ear_clip(points: &[[f64; 2]]) -> Result<Vec<[usize; 3]>, CompositeError> {
    let mut chain: Vec<usize> = (0..points.len()).collect();
    let mut tris = Vec::with_capacity(points.len());
    while chain.len() > 3 {
        let m = chain.len();
        let mut ear = None;
        for k in 0..m {
            let (ia, ib, ic) = (chain[(k + m - 1) % m], chain[k], chain[(k + 1) % m]);
            let (a, b, c) = (points[ia], points[ib], points[ic]);
            if orient(a, b, c) <= 0.0 {
                continue;
            }
            let blocked = chain
                .iter()
                .any(|&q| q != ia && q != ib && q != ic && in_closed_triangle(points[q], a, b, c));
            if !blocked {
                ear = Some((k, [ia, ib, ic]));
                break;
            }
        }
        match ear {
            Some((k, t)) => {
                tris.push(t);
                chain.remove(k);
            }
            None => {
                let flat = (0..m).find(|&k| {
                    let (ia, ib, ic) = (chain[(k + m - 1) % m], chain[k], chain[(k + 1) % m]);
                    orient(points[ia], points[ib], points[ic]) == 0.0
                });
                match flat {
                    Some(k) => {
                        chain.remove(k);
                    }
                    None => {
                        return Err(CompositeError::new(
                            "Polygon",
                            "Could not create a valid triangulation",
                        ))
                    }
                }
            }
        }
    }
    let (a, b, c) = (points[chain[0]], points[chain[1]], points[chain[2]]);
    if orient(a, b, c) > 0.0 {
        tris.push([chain[0], chain[1], chain[2]]);
    }
    if tris.is_empty() {
        return Err(CompositeError::new("Polygon", "Polygon has no area"));
    }
    Ok(tris)
}

/// The neighbour map of `_decompose_polygon_into_convex_sets` (lines
/// 1290-1300): triangle -> neighbours sharing an edge, listed as qhull lists
/// them, the neighbour opposite vertex 0, 1, 2 in turn. Every triangle is
/// inside the polygon here, so none is filtered out.
fn neighbour_map(tris: &[[usize; 3]]) -> BTreeMap<usize, Vec<usize>> {
    let mut by_edge: BTreeMap<(usize, usize), Vec<usize>> = BTreeMap::new();
    let key = |a: usize, b: usize| (a.min(b), a.max(b));
    for (t, v) in tris.iter().enumerate() {
        for k in 0..3 {
            by_edge
                .entry(key(v[(k + 1) % 3], v[(k + 2) % 3]))
                .or_default()
                .push(t);
        }
    }
    let mut map = BTreeMap::new();
    for (t, v) in tris.iter().enumerate() {
        let mut list = Vec::new();
        for k in 0..3 {
            if let Some(owners) = by_edge.get(&key(v[(k + 1) % 3], v[(k + 2) % 3])) {
                list.extend(owners.iter().copied().filter(|&o| o != t));
            }
        }
        map.insert(t, list);
    }
    map
}

/// `_decompose_polygon_into_convex_sets`'s grouping loop (lines 1302-1307).
fn decompose(points: &[[f64; 2]], tris: &[[usize; 3]]) -> Vec<Vec<usize>> {
    let mut map = neighbour_map(tris);
    let mut groups = Vec::new();
    while !map.is_empty() {
        groups.push(group_simplices(&mut map, None, points, tris));
    }
    groups
}

/// `_group_simplices` (lines 1149-1202), recursion included. A `BTreeMap`
/// keyed by triangle index iterates in the order upstream's `dict` was built.
fn group_simplices(
    map: &mut BTreeMap<usize, Vec<usize>>,
    group: Option<Vec<usize>>,
    points: &[[f64; 2]],
    tris: &[[usize; 3]],
) -> Vec<usize> {
    if map.is_empty() {
        return group.unwrap_or_default();
    }
    let Some(mut group) = group else {
        // `sorted(..., key=len)[0][0]`: the first of the shortest lists.
        let mut best = None;
        for (&k, v) in map.iter() {
            if best.is_none_or(|(_, len)| v.len() < len) {
                best = Some((k, v.len()));
            }
        }
        let sidx = best.map(|(k, _)| k).unwrap_or(0);
        return group_simplices(map, Some(vec![sidx]), points, tris);
    };
    let sidx = *group.last().unwrap_or(&0);
    let neighbours = map.remove(&sidx).unwrap_or_default();
    for n in neighbours {
        if group.contains(&n) || !map.contains_key(&n) {
            continue;
        }
        let mut test_group = group.clone();
        test_group.push(n);
        let mut idx: Vec<usize> = test_group.iter().flat_map(|&t| tris[t]).collect();
        idx.sort_unstable();
        idx.dedup();
        let pts: Vec<[f64; 2]> = idx.iter().map(|&i| points[i]).collect();
        if all_on_hull(&pts) {
            group = group_simplices(map, Some(test_group), points, tris);
        }
    }
    group
}

/// Strict convex hull (collinear points dropped), counter-clockwise, by
/// Andrew's monotone chain. Stands in for `scipy.spatial.ConvexHull`'s
/// vertices.
fn hull(points: &[[f64; 2]]) -> Vec<[f64; 2]> {
    let mut p = points.to_vec();
    p.sort_by(|a, b| a[0].total_cmp(&b[0]).then(a[1].total_cmp(&b[1])));
    p.dedup();
    if p.len() < 3 {
        return p;
    }
    let mut lower: Vec<[f64; 2]> = Vec::new();
    for &q in &p {
        while lower.len() >= 2 && orient(lower[lower.len() - 2], lower[lower.len() - 1], q) <= 0.0 {
            lower.pop();
        }
        lower.push(q);
    }
    let mut upper: Vec<[f64; 2]> = Vec::new();
    for &q in p.iter().rev() {
        while upper.len() >= 2 && orient(upper[upper.len() - 2], upper[upper.len() - 1], q) <= 0.0 {
            upper.pop();
        }
        upper.push(q);
    }
    lower.pop();
    upper.pop();
    lower.extend(upper);
    lower
}

/// Facet equations `(dx, dy, c)` of the hull, `dx x + dy y + c = 0` on the
/// facet with `(dx, dy)` the outward unit normal (`ConvexHull.equations`).
fn hull_equations(points: &[[f64; 2]]) -> Vec<[f64; 3]> {
    let h = hull(points);
    let n = h.len();
    (0..n)
        .map(|i| edge_equation(h[i], h[(i + 1) % n]))
        .collect()
}

/// Outward unit normal and offset of the counter-clockwise edge `a -> b`:
/// the tangent turned 90 degrees clockwise (upstream `_normals`, lines
/// 990-1002) and `c = -n . a` (`_equations`, lines 1004-1010).
fn edge_equation(a: [f64; 2], b: [f64; 2]) -> [f64; 3] {
    let t = sub2(b, a);
    let len = (t[0] * t[0] + t[1] * t[1]).sqrt();
    let t = [t[0] / len, t[1] / len];
    let nrm = [t[1], -t[0]];
    [nrm[0], nrm[1], -(nrm[0] * a[0] + nrm[1] * a[1])]
}

/// The polygon's own edge equations (`_equations`).
fn boundary_equations(points: &[[f64; 2]]) -> Vec<[f64; 3]> {
    let n = points.len();
    (0..n)
        .map(|i| edge_equation(points[i], points[(i + 1) % n]))
        .collect()
}

/// Whether every point is a hull vertex or on a hull facet: upstream's
/// convexity test `len(test_points) == len(hull.vertices) + len(hull.coplanar)`
/// with qhull option `Qc`. "On a facet" is within `1e-12` of the largest
/// coordinate magnitude (qhull's own coplanarity tolerance is round-off
/// sized; this one is too).
fn all_on_hull(points: &[[f64; 2]]) -> bool {
    let eqs = hull_equations(points);
    let scale = points
        .iter()
        .flat_map(|p| [p[0].abs(), p[1].abs()])
        .fold(1.0_f64, f64::max);
    let tol = 1e-12 * scale;
    points.iter().all(|p| {
        eqs.iter()
            .map(|e| e[0] * p[0] + e[1] * p[1] + e[2])
            .fold(f64::NEG_INFINITY, f64::max)
            >= -tol
    })
}

/// One hull facet as a half-space, `_get_convex_hull_surfs` (lines
/// 1204-1270): horizontal, vertical or slanted, per basis.
///
/// An `rz` facet on the axis (`r = 0`) is refused, as upstream refuses it:
/// `openmc.ZCylinder(r=-c/dx)` fails its `r > 0` coefficient check
/// (`openmc/surface.py`, `SurfaceCoefficient.__set__`, line 63).
fn facet_half_space(
    b: &mut Builder,
    eq: [f64; 3],
    basis: PolygonBasis,
    on_boundary: bool,
) -> Result<Vec<RegionToken>, CompositeError> {
    let [dx, dy, c] = eq;
    // math.isclose(v, 0, abs_tol=1e-8)
    let is_zero = |v: f64| v.abs() <= 1e-8_f64.max(1e-9 * v.abs());
    if is_zero(dx) {
        let s = match basis {
            PolygonBasis::Xz | PolygonBasis::Yz | PolygonBasis::Rz => {
                SurfaceKind::ZPlane(ZPlane { z0: -c / dy, bc: T })
            }
            PolygonBasis::Xy => SurfaceKind::YPlane(YPlane { y0: -c / dy, bc: T }),
        };
        let i = b.add_with(s, on_boundary);
        return Ok(if dy < 0.0 { pos(i) } else { neg(i) });
    }
    if is_zero(dy) {
        if basis == PolygonBasis::Rz && -c / dx <= 0.0 {
            return Err(CompositeError::new(
                "Polygon",
                "an rz facet at r <= 0 makes a ZCylinder with r <= 0, which OpenMC refuses",
            ));
        }
        let s = match basis {
            PolygonBasis::Xy | PolygonBasis::Xz => {
                SurfaceKind::XPlane(XPlane { x0: -c / dx, bc: T })
            }
            PolygonBasis::Yz => SurfaceKind::YPlane(YPlane { y0: -c / dx, bc: T }),
            PolygonBasis::Rz => SurfaceKind::ZCylinder(ZCylinder {
                x0: 0.0,
                y0: 0.0,
                r: -c / dx,
                bc: T,
            }),
        };
        let i = b.add_with(s, on_boundary);
        return Ok(if dx < 0.0 { pos(i) } else { neg(i) });
    }
    let plane = |a: f64, bb: f64, cc: f64, d: f64| {
        SurfaceKind::Plane(Plane {
            a,
            b: bb,
            c: cc,
            d,
            bc: T,
        })
    };
    Ok(match basis {
        PolygonBasis::Xy => neg(b.add_with(plane(dx, dy, 0.0, -c), on_boundary)),
        PolygonBasis::Yz => neg(b.add_with(plane(0.0, dx, dy, -c), on_boundary)),
        PolygonBasis::Xz => neg(b.add_with(plane(dx, 0.0, dy, -c), on_boundary)),
        PolygonBasis::Rz => {
            // A slanted (r, z) facet revolves into a one-sided z-cone.
            let y0 = -c / dy;
            let r2 = (dy * dy) / (dx * dx);
            let up = dy / dx < 0.0;
            let cone = b.add_with(
                SurfaceKind::ZCone(ZCone {
                    x0: 0.0,
                    y0: 0.0,
                    z0: y0,
                    r_sq: r2,
                    bc: T,
                }),
                on_boundary,
            );
            let plane = b.add_with(SurfaceKind::ZPlane(ZPlane { z0: y0, bc: T }), false);
            let inside = and(vec![neg(cone), if up { pos(plane) } else { neg(plane) }]);
            if (up && dx - dy < 0.0) || (!up && dx + dy < 0.0) {
                not(inside)
            } else {
                inside
            }
        }
    })
}
