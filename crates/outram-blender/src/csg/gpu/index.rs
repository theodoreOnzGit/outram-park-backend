// SPDX-License-Identifier: GPL-3.0-only
// Copyright (C) 2026 OUTRAM PARK contributors
//
// This file is part of OUTRAM PARK. GPL-3.0-only; see LICENSE.
//
// NEW WORK (gh:#587), no upstream counterpart: a uniform-grid spatial index
// over a universe's cells for the GPU tracer. The cell bounding boxes are the
// OpenMC port's (`csg::plot::model_plot::cell_bounding_box`,
// `openmc.Cell.bounding_box`, MIT), tightened where those are unbounded by a
// vertex enumeration over the plane half-spaces and a cone-radius bound
// (new work, `tight_box`). Restricting a CSG region to the surfaces
// that pass through a voxel is the standard "local surface list" idea of
// voxelised CSG ray casting; written here from that description, not ported.

//! **A uniform grid over a universe's cells, with each cell's region cut
//! down to the surfaces that pass through each voxel**: the spatial index the
//! GPU tracer ([`super::render`]) uses so that a step through HTR-10's
//! reflector tests a handful of half-spaces instead of a 124-token region.
//!
//! For every voxel (padded by [`GridIndex::pad`] on each side) and every
//! cell whose bounding box reaches it:
//!
//! - each surface of the cell is classified against the padded box: either
//!   it **passes through** the box, or the whole box lies on **one side** of
//!   it (exact tests for planes, spheres, axis cylinders and axis cones; a
//!   general quadric is always taken to pass through);
//! - the cell's region is then constant-folded with those sides: a
//!   half-space whose surface does not pass through is `true` or `false`
//!   throughout the box. A region that folds to `false` drops the cell from
//!   the voxel; one that folds to `true` leaves an empty token list (the
//!   whole voxel is that cell); otherwise the voxel keeps the cell with the
//!   **surviving tokens only**.
//!
//! Inside the voxel the folded region is the same set as the full one, so
//! `contains` and the distance to the cell's boundary within the voxel are
//! unchanged; the tracer adds the voxel's own faces as "silent" boundaries
//! (no material change, nothing reported) and re-reads the next voxel's
//! list when it crosses one. The candidate order is the universe's search
//! order, so the first cell found is [`crate::csg::universe::Universe::find_cell`]'s.
//!
//! Lists are de-duplicated (a voxel inside one reflector block shares its
//! list with every other such voxel). A cell with a malformed region (one
//! [`crate::csg::cell::Cell::contains`] would reject on its stack discipline)
//! is never folded: it is kept, whole, in every voxel its box reaches.

use std::collections::HashMap;

use crate::csg::cell::{HalfSpaceSense, RegionToken};
use crate::csg::geometry::Geometry;
use crate::csg::plot::model_plot::{cell_bounding_box, region_tree, surface_bounding_box, Node};
use crate::csg::surface::SurfaceKind;

use super::flat::{TOK_COMPLEMENT, TOK_INTERSECTION, TOK_UNION};

/// A universe is indexed when its cells hold at least this many region
/// tokens in all (HTR-10's root universe holds 1 675) and it has at least
/// [`MIN_CELLS`] cells. Smaller universes are scanned whole, as before
/// (with the crossing hints). Measured 2026-10-05 on HTR-10: indexing the
/// 1 500 bed tiles too (125 tokens each, threshold 96) changed no frame
/// time and grew the buffer from 4.6 to 32.8 MB, so the threshold is high.
pub const MIN_TOKENS: usize = 512;
/// The grid index may add at most this many words to the flat buffer (16
/// MB); universes are indexed root first, then by token count, until it is
/// spent. Keeps a large geometry (a DEM bed) under the device's 128 MiB
/// storage-buffer binding limit.
pub const BUDGET_WORDS: usize = 4 << 20;
/// See [`MIN_TOKENS`].
pub const MIN_CELLS: usize = 4;
/// Voxels per indexed universe: about this many tokens' worth...
pub const VOXELS_PER_TOKEN: usize = 24;
/// ...between these bounds.
pub const MIN_VOXELS: usize = 64;
/// See [`MIN_VOXELS`].
pub const MAX_VOXELS: usize = 1 << 17;

/// The index of one universe, before encoding.
#[derive(Debug, Clone, PartialEq)]
pub struct GridIndex {
    /// Lower corner of the grid \[cm, universe frame\].
    pub lo: [f64; 3],
    /// Voxel edge per axis \[cm\].
    pub d: [f64; 3],
    /// Voxels per axis.
    pub n: [usize; 3],
    /// Some cell's box is unbounded: points outside the grid may still lie
    /// in a cell, so the tracer scans the whole universe there.
    pub open: bool,
    /// The cells that make the grid open: unbounded even by [`tight_box`].
    pub open_cells: Vec<usize>,
    /// Padding of each voxel when classifying surfaces \[cm\]: larger than the
    /// tracer's `f32` position error and its nudge past a crossing.
    pub pad: f64,
    /// Per voxel (x fastest): index into [`Self::lists`], or `None` when no
    /// cell reaches the voxel.
    pub voxels: Vec<Option<u32>>,
    /// The distinct candidate lists.
    pub lists: Vec<Vec<Candidate>>,
}

/// One cell in one voxel: its region cut down to the voxel.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct Candidate {
    /// Cell index (global).
    pub cell: u32,
    /// The folded region, encoded as in [`super::flat`].
    pub tokens: Vec<u32>,
    /// The folded region is a plain AND of half-spaces (or empty).
    pub simple: bool,
}

/// What a surface does across a box.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Side {
    /// Passes through the box (or may: the conservative answer).
    Through,
    /// The whole box lies on the positive (`Outside`) side.
    Positive,
    /// The whole box lies on the negative (`Inside`) side.
    Negative,
}

fn side_of(fmin: f64, fmax: f64) -> Side {
    if fmin > 0.0 {
        Side::Positive
    } else if fmax < 0.0 {
        Side::Negative
    } else {
        Side::Through
    }
}

/// Range of `(x - c)^2` over `x` in `[lo, hi]`.
fn sq_range(c: f64, lo: f64, hi: f64) -> (f64, f64) {
    let near = c.clamp(lo, hi) - c;
    let far = (c - lo).abs().max((c - hi).abs());
    (near * near, far * far)
}

/// Range of `a x` over `x` in `[lo, hi]` (bounds may be infinite).
fn lin_range(a: f64, lo: f64, hi: f64) -> (f64, f64) {
    if a == 0.0 {
        return (0.0, 0.0);
    }
    let (p, q) = (a * lo, a * hi);
    (p.min(q), p.max(q))
}

/// Classify surface `s` against the box `[lo, hi]`.
fn classify(s: &SurfaceKind, lo: [f64; 3], hi: [f64; 3]) -> Side {
    let ax = |k: usize, c: f64| -> Side { side_of(lo[k] - c, hi[k] - c) };
    // `sum (x_k - c_k)^2 - r^2` over the listed axes, minus `scale` times
    // the square along `cone` (a cone's axis), if any.
    let quad = |axes: &[(usize, f64)], cone: Option<(usize, f64, f64)>, r2: f64| -> Side {
        let (mut a, mut b) = (0.0, 0.0);
        for &(k, c) in axes {
            let (p, q) = sq_range(c, lo[k], hi[k]);
            a += p;
            b += q;
        }
        let (mut cmin, mut cmax) = (0.0, 0.0);
        if let Some((k, c, scale)) = cone {
            let (p, q) = sq_range(c, lo[k], hi[k]);
            if scale >= 0.0 {
                cmin = scale * p;
                cmax = scale * q;
            } else {
                cmin = scale * q;
                cmax = scale * p;
            }
        }
        side_of(a - cmax - r2, b - cmin - r2)
    };
    match s {
        SurfaceKind::XPlane(p) => ax(0, p.x0),
        SurfaceKind::YPlane(p) => ax(1, p.y0),
        SurfaceKind::ZPlane(p) => ax(2, p.z0),
        SurfaceKind::Plane(p) => {
            let r = [
                lin_range(p.a, lo[0], hi[0]),
                lin_range(p.b, lo[1], hi[1]),
                lin_range(p.c, lo[2], hi[2]),
            ];
            side_of(
                r[0].0 + r[1].0 + r[2].0 - p.d,
                r[0].1 + r[1].1 + r[2].1 - p.d,
            )
        }
        SurfaceKind::Sphere(p) => quad(&[(0, p.x0), (1, p.y0), (2, p.z0)], None, p.r * p.r),
        SurfaceKind::XCylinder(p) => quad(&[(1, p.y0), (2, p.z0)], None, p.r * p.r),
        SurfaceKind::YCylinder(p) => quad(&[(0, p.x0), (2, p.z0)], None, p.r * p.r),
        SurfaceKind::ZCylinder(p) => quad(&[(0, p.x0), (1, p.y0)], None, p.r * p.r),
        SurfaceKind::XCone(p) => quad(&[(1, p.y0), (2, p.z0)], Some((0, p.x0, p.r_sq)), 0.0),
        SurfaceKind::YCone(p) => quad(&[(0, p.x0), (2, p.z0)], Some((1, p.y0, p.r_sq)), 0.0),
        SurfaceKind::ZCone(p) => quad(&[(0, p.x0), (1, p.y0)], Some((2, p.z0, p.r_sq)), 0.0),
        SurfaceKind::Quadric(_)
        | SurfaceKind::XTorus(_)
        | SurfaceKind::YTorus(_)
        | SurfaceKind::ZTorus(_) => Side::Through,
    }
}

/// A sub-expression of a region while it is folded.
enum Folded {
    Const(bool),
    Tokens(Vec<u32>, bool),
}

/// Fold `region` with the sides `side(surface)` gives; `None` if the region
/// is malformed (the caller keeps it whole).
fn fold(region: &[RegionToken], mut side: impl FnMut(usize) -> Side) -> Option<Folded> {
    let mut st: Vec<Folded> = Vec::new();
    for t in region {
        match *t {
            RegionToken::HalfSpace { surface_idx, sense } => {
                let out = sense == HalfSpaceSense::Outside;
                st.push(match side(surface_idx) {
                    Side::Through => {
                        let s = u32::try_from(surface_idx).ok()?;
                        Folded::Tokens(vec![(s << 1) | u32::from(out)], true)
                    }
                    Side::Positive => Folded::Const(out),
                    Side::Negative => Folded::Const(!out),
                });
            }
            RegionToken::Intersection | RegionToken::Union => {
                let b = st.pop()?;
                let a = st.pop()?;
                let and = matches!(t, RegionToken::Intersection);
                st.push(match (a, b) {
                    (Folded::Const(x), other) | (other, Folded::Const(x)) => {
                        if x == and {
                            other
                        } else {
                            Folded::Const(x)
                        }
                    }
                    (Folded::Tokens(mut ta, sa), Folded::Tokens(tb, sb)) => {
                        ta.extend(tb);
                        ta.push(if and { TOK_INTERSECTION } else { TOK_UNION });
                        Folded::Tokens(ta, and && sa && sb)
                    }
                });
            }
            RegionToken::Complement => {
                let a = st.pop()?;
                st.push(match a {
                    Folded::Const(x) => Folded::Const(!x),
                    Folded::Tokens(mut ta, _) => {
                        ta.push(TOK_COMPLEMENT);
                        Folded::Tokens(ta, false)
                    }
                });
            }
        }
    }
    match st.len() {
        0 => Some(Folded::Const(true)),
        1 => st.pop(),
        _ => None,
    }
}

/// Coordinates beyond this are "unbounded" in [`tight_box`]'s vertex search.
const BIG: f64 = 1.0e7;

/// An axis-aligned box `(lo, hi)`; infinite bounds allowed, `lo > hi` empty.
type Aabb = ([f64; 3], [f64; 3]);

fn and_box(a: Aabb, b: Aabb) -> Aabb {
    (
        [0, 1, 2].map(|k| a.0[k].max(b.0[k])),
        [0, 1, 2].map(|k| a.1[k].min(b.1[k])),
    )
}

fn or_box(a: Aabb, b: Aabb) -> Aabb {
    (
        [0, 1, 2].map(|k| a.0[k].min(b.0[k])),
        [0, 1, 2].map(|k| a.1[k].max(b.1[k])),
    )
}

fn is_empty(b: &Aabb) -> bool {
    (0..3).any(|k| b.0[k] > b.1[k])
}

/// `n . x <= d` for a half-space bounded by a plane, if `s` is one.
fn plane_constraint(s: &SurfaceKind, positive: bool) -> Option<([f64; 3], f64)> {
    let (n, d) = match s {
        SurfaceKind::XPlane(p) => ([1.0, 0.0, 0.0], p.x0),
        SurfaceKind::YPlane(p) => ([0.0, 1.0, 0.0], p.y0),
        SurfaceKind::ZPlane(p) => ([0.0, 0.0, 1.0], p.z0),
        SurfaceKind::Plane(p) => ([p.a, p.b, p.c], p.d),
        _ => return None,
    };
    // Negative side: n.x - d < 0; positive side: -n.x <= -d.
    Some(if positive {
        (n.map(|v| -v), -d)
    } else {
        (n, d)
    })
}

/// The box of `{x in b : n_i . x <= d_i}`, by enumerating the vertices of
/// the polytope (every three of its planes and `b`'s faces, clipped at
/// `BIG`); the empty box if there is none.
fn clip_by_planes(b: Aabb, planes: &[([f64; 3], f64)]) -> Aabb {
    let mut all: Vec<([f64; 3], f64)> = planes.to_vec();
    for k in 0..3 {
        let mut e = [0.0; 3];
        e[k] = 1.0;
        all.push((e, b.1[k].min(BIG)));
        e[k] = -1.0;
        all.push((e, -(b.0[k].max(-BIG))));
    }
    let scale = all.iter().map(|p| p.1.abs()).fold(1.0, f64::max);
    let tol = 1e-9 * scale;
    let mut out: Aabb = ([f64::INFINITY; 3], [f64::NEG_INFINITY; 3]);
    let n = all.len();
    for i in 0..n {
        for j in i + 1..n {
            for k in j + 1..n {
                let (a, b2, c) = (all[i].0, all[j].0, all[k].0);
                let det = a[0] * (b2[1] * c[2] - b2[2] * c[1])
                    - a[1] * (b2[0] * c[2] - b2[2] * c[0])
                    + a[2] * (b2[0] * c[1] - b2[1] * c[0]);
                if det.abs() < 1e-12 {
                    continue;
                }
                // Cramer's rule.
                let d = [all[i].1, all[j].1, all[k].1];
                let col = |m: usize| -> f64 {
                    let r = |row: [f64; 3], dv: f64| -> [f64; 3] {
                        let mut v = row;
                        v[m] = dv;
                        v
                    };
                    let (p, q, s) = (r(a, d[0]), r(b2, d[1]), r(c, d[2]));
                    (p[0] * (q[1] * s[2] - q[2] * s[1]) - p[1] * (q[0] * s[2] - q[2] * s[0])
                        + p[2] * (q[0] * s[1] - q[1] * s[0]))
                        / det
                };
                let x = [col(0), col(1), col(2)];
                if all
                    .iter()
                    .all(|(nn, dd)| nn[0] * x[0] + nn[1] * x[1] + nn[2] * x[2] <= dd + tol)
                {
                    out = or_box(out, (x, x));
                }
            }
        }
    }
    if is_empty(&out) {
        return out;
    }
    // A vertex on the BIG clipping box means unbounded that way.
    let lo = [0, 1, 2].map(|k| {
        if out.0[k] <= -0.999 * BIG {
            f64::NEG_INFINITY
        } else {
            out.0[k] - tol
        }
    });
    let hi = [0, 1, 2].map(|k| {
        if out.1[k] >= 0.999 * BIG {
            f64::INFINITY
        } else {
            out.1[k] + tol
        }
    });
    and_box(b, (lo, hi))
}

/// **A bounding box of a region at least as tight as OpenMC's**
/// (`Region.bounding_box`, the port in `csg::plot::model_plot`), and still
/// conservative. OpenMC intersects the boxes of the half-spaces of an
/// intersection one by one, so a half-space with no box of its own (an
/// oblique plane, a cone) bounds nothing, even where the intersection is
/// bounded: a slot between two pairs of oblique planes, a cone cut by two
/// z-planes. Here an intersection's box is also clipped by the polytope of
/// its plane half-spaces (exact vertex enumeration) and by each cone's
/// radius over the axial range found so far.
///
/// Why: such a cell has an infinite OpenMC box, which left HTR-10's root
/// grid "open", and then every ray outside the model scanned all 128 root
/// cells at every step through the extended surfaces (0.2-0.4 s a frame).
fn tight_box(g: &Geometry, nodes: &[Node], i: usize, negate: bool) -> Aabb {
    match nodes[i] {
        Node::Half(s, sense) => {
            let negative = matches!(sense, HalfSpaceSense::Inside) != negate;
            let b = surface_bounding_box(&g.surfaces[s], negative);
            (b.lower_left, b.upper_right)
        }
        Node::Not(a) => tight_box(g, nodes, a, !negate),
        Node::And(a, b) | Node::Or(a, b) => {
            let intersect = matches!(nodes[i], Node::And(..)) != negate;
            if !intersect {
                return or_box(
                    tight_box(g, nodes, a, negate),
                    tight_box(g, nodes, b, negate),
                );
            }
            // The whole chain of intersections under this node.
            let mut leaves: Vec<(usize, bool)> = Vec::new();
            let mut others: Vec<(usize, bool)> = Vec::new();
            let mut stack = vec![(a, negate), (b, negate)];
            while let Some((j, ng)) = stack.pop() {
                match nodes[j] {
                    Node::Half(s, sense) => {
                        leaves.push((s, matches!(sense, HalfSpaceSense::Outside) != ng));
                    }
                    Node::Not(c) => stack.push((c, !ng)),
                    Node::And(p, q) | Node::Or(p, q) if matches!(nodes[j], Node::And(..)) != ng => {
                        stack.push((p, ng));
                        stack.push((q, ng));
                    }
                    _ => others.push((j, ng)),
                }
            }
            let inf = f64::INFINITY;
            let mut bx: Aabb = ([-inf; 3], [inf; 3]);
            for &(s, positive) in &leaves {
                let sb = surface_bounding_box(&g.surfaces[s], !positive);
                bx = and_box(bx, (sb.lower_left, sb.upper_right));
            }
            for &(j, ng) in &others {
                bx = and_box(bx, tight_box(g, nodes, j, ng));
            }
            let planes: Vec<([f64; 3], f64)> = leaves
                .iter()
                .filter_map(|&(s, positive)| plane_constraint(&g.surfaces[s], positive))
                .collect();
            for _ in 0..2 {
                if is_empty(&bx) {
                    return bx;
                }
                if !planes.is_empty() {
                    bx = clip_by_planes(bx, &planes);
                }
                // Inside a cone: the radius is at most sqrt(r_sq) |axial - apex|.
                for &(s, positive) in &leaves {
                    let (axis, apex, r_sq) = match &g.surfaces[s] {
                        SurfaceKind::XCone(c) if !positive => (0, [c.x0, c.y0, c.z0], c.r_sq),
                        SurfaceKind::YCone(c) if !positive => (1, [c.x0, c.y0, c.z0], c.r_sq),
                        SurfaceKind::ZCone(c) if !positive => (2, [c.x0, c.y0, c.z0], c.r_sq),
                        _ => continue,
                    };
                    let reach = (bx.0[axis] - apex[axis])
                        .abs()
                        .max((bx.1[axis] - apex[axis]).abs());
                    if !reach.is_finite() || r_sq < 0.0 {
                        continue;
                    }
                    let r = r_sq.sqrt() * reach;
                    for k in (0..3).filter(|&k| k != axis) {
                        bx.0[k] = bx.0[k].max(apex[k] - r);
                        bx.1[k] = bx.1[k].min(apex[k] + r);
                    }
                }
            }
            bx
        }
    }
}

/// [`tight_box`] of a cell (infinite for a cell with no region).
fn cell_tight_box(g: &Geometry, c: usize) -> Aabb {
    match region_tree(&g.cells[c].region) {
        Some((nodes, root)) => tight_box(g, &nodes, root, false),
        None => ([f64::NEG_INFINITY; 3], [f64::INFINITY; 3]),
    }
}

/// Whether universe `u` is worth indexing ([`MIN_TOKENS`], [`MIN_CELLS`]).
#[must_use]
pub fn wants_index(g: &Geometry, u: usize) -> bool {
    let cells = &g.universes[u].cell_indices;
    let tokens: usize = cells.iter().map(|&c| g.cells[c].region.len()).sum();
    cells.len() >= MIN_CELLS && tokens >= MIN_TOKENS
}

/// **Build the grid index of universe `u`**, or `None` when no cell has a
/// bounded box (nothing to grid).
#[must_use]
pub fn build(g: &Geometry, u: usize) -> Option<GridIndex> {
    let cells = &g.universes[u].cell_indices;
    let boxes: Vec<([f64; 3], [f64; 3])> = cells
        .iter()
        .map(|&c| {
            let b = cell_bounding_box(g, &g.cells[c]);
            let ob = (b.lower_left, b.upper_right);
            if (0..3).all(|k| ob.0[k].is_finite() && ob.1[k].is_finite()) {
                ob
            } else {
                // OpenMC's box is unbounded: try the tighter one.
                and_box(ob, cell_tight_box(g, c))
            }
        })
        .collect();
    let finite = |b: &([f64; 3], [f64; 3])| {
        (0..3).all(|k| b.0[k].is_finite() && b.1[k].is_finite() && b.0[k] <= b.1[k])
    };
    let empty = |b: &([f64; 3], [f64; 3])| (0..3).any(|k| b.0[k] > b.1[k] || b.0[k].is_nan());
    let mut lo = [f64::INFINITY; 3];
    let mut hi = [f64::NEG_INFINITY; 3];
    let mut unbounded = Vec::new();
    for (j, b) in boxes.iter().enumerate() {
        if finite(b) {
            for k in 0..3 {
                lo[k] = lo[k].min(b.0[k]);
                hi[k] = hi[k].max(b.1[k]);
            }
        } else if !empty(b) {
            unbounded.push(cells[j]);
        }
    }
    if !(0..3).all(|k| lo[k].is_finite() && hi[k] > lo[k]) {
        return None;
    }
    let ext: [f64; 3] = [0, 1, 2].map(|k| hi[k] - lo[k]);
    let diag = (ext[0] * ext[0] + ext[1] * ext[1] + ext[2] * ext[2]).sqrt();
    // Pad: well past the tracer's nudge (1e-5 cm + 4e-7 of the distance
    // travelled) and the f32 rounding of a position of this size.
    let pad = 0.01 + 2.0e-5 * diag;
    // Grow the grid by the pad, so a point the tracer places just outside
    // a bounded cell's box is still in the grid.
    for k in 0..3 {
        lo[k] -= pad;
        hi[k] += pad;
    }
    let open_cells = unbounded;
    let open = !open_cells.is_empty();
    let ext: [f64; 3] = [0, 1, 2].map(|k| hi[k] - lo[k]);
    let tokens: usize = cells.iter().map(|&c| g.cells[c].region.len()).sum();
    let target = (tokens * VOXELS_PER_TOKEN).clamp(MIN_VOXELS, MAX_VOXELS) as f64;
    let side = (ext[0] * ext[1] * ext[2] / target).cbrt();
    let n = ext.map(|e| ((e / side).ceil() as usize).clamp(1, 1024));
    let d = [0, 1, 2].map(|k| ext[k] / n[k] as f64);

    let n_surf = g.surfaces.len();
    let mut stamp: Vec<u32> = vec![u32::MAX; n_surf];
    let mut memo: Vec<Side> = vec![Side::Through; n_surf];
    let mut dedup: HashMap<Vec<Candidate>, u32> = HashMap::new();
    let mut lists: Vec<Vec<Candidate>> = Vec::new();
    let mut voxels = Vec::with_capacity(n[0] * n[1] * n[2]);
    let mut v: u32 = 0;
    for iz in 0..n[2] {
        for iy in 0..n[1] {
            for ix in 0..n[0] {
                let i = [ix, iy, iz];
                let blo: [f64; 3] = [0, 1, 2].map(|k| lo[k] + i[k] as f64 * d[k] - pad);
                let bhi: [f64; 3] = [0, 1, 2].map(|k| lo[k] + (i[k] + 1) as f64 * d[k] + pad);
                let mut list = Vec::new();
                for (j, &c) in cells.iter().enumerate() {
                    let b = &boxes[j];
                    if (0..3).any(|k| b.1[k] < blo[k] || b.0[k] > bhi[k] || b.0[k].is_nan()) {
                        continue;
                    }
                    let cell = u32::try_from(c).ok()?;
                    let mut side = |s: usize| {
                        if stamp[s] != v {
                            stamp[s] = v;
                            memo[s] = classify(&g.surfaces[s], blo, bhi);
                        }
                        memo[s]
                    };
                    match fold(&g.cells[c].region, &mut side) {
                        Some(Folded::Const(false)) => {}
                        Some(Folded::Const(true)) => list.push(Candidate {
                            cell,
                            tokens: Vec::new(),
                            simple: true,
                        }),
                        Some(Folded::Tokens(tokens, simple)) => list.push(Candidate {
                            cell,
                            tokens,
                            simple,
                        }),
                        // Malformed: kept whole (the full region's tokens).
                        None => list.push(Candidate {
                            cell,
                            tokens: full_tokens(&g.cells[c].region)?,
                            simple: false,
                        }),
                    }
                }
                if list.is_empty() {
                    voxels.push(None);
                } else {
                    let next = u32::try_from(lists.len()).ok()?;
                    let id = *dedup.entry(list.clone()).or_insert_with(|| {
                        lists.push(list);
                        next
                    });
                    voxels.push(Some(id));
                }
                v += 1;
            }
        }
    }
    Some(GridIndex {
        lo,
        d,
        n,
        open,
        open_cells,
        pad,
        voxels,
        lists,
    })
}

/// A region's tokens unfolded, in the flat encoding.
fn full_tokens(region: &[RegionToken]) -> Option<Vec<u32>> {
    region
        .iter()
        .map(|t| match *t {
            RegionToken::HalfSpace { surface_idx, sense } => u32::try_from(surface_idx)
                .ok()
                .map(|s| (s << 1) | u32::from(sense == HalfSpaceSense::Outside)),
            RegionToken::Intersection => Some(TOK_INTERSECTION),
            RegionToken::Union => Some(TOK_UNION),
            RegionToken::Complement => Some(TOK_COMPLEMENT),
        })
        .collect()
}

impl GridIndex {
    /// Voxels in all.
    #[must_use]
    pub fn n_voxels(&self) -> usize {
        self.n[0] * self.n[1] * self.n[2]
    }

    /// Mean candidates and mean folded tokens per non-empty voxel.
    #[must_use]
    pub fn mean_load(&self) -> (f64, f64) {
        let (mut nv, mut nc, mut nt) = (0usize, 0usize, 0usize);
        for l in self.voxels.iter().flatten() {
            let l = &self.lists[*l as usize];
            nv += 1;
            nc += l.len();
            nt += l.iter().map(|c| c.tokens.len()).sum::<usize>();
        }
        let nv = nv.max(1) as f64;
        (nc as f64 / nv, nt as f64 / nv)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::csg::cell::{Cell, SurfaceToken};
    use crate::csg::position::{Direction, Position};
    use crate::csg::surface::{BoundaryType, Plane, Sphere, ZCone, ZCylinder, ZPlane};
    use crate::csg::universe::Universe;

    fn hs(s: usize, out: bool) -> RegionToken {
        RegionToken::HalfSpace {
            surface_idx: s,
            sense: if out {
                HalfSpaceSense::Outside
            } else {
                HalfSpaceSense::Inside
            },
        }
    }

    /// A block of 9 bored rods in a cylinder, plus the bores: the root
    /// universe of a small reflector.
    fn bored() -> Geometry {
        let mut surfaces = vec![
            SurfaceKind::ZCylinder(ZCylinder {
                x0: 0.0,
                y0: 0.0,
                r: 50.0,
                bc: BoundaryType::Vacuum,
            }),
            SurfaceKind::ZPlane(ZPlane {
                z0: -40.0,
                bc: BoundaryType::Vacuum,
            }),
            SurfaceKind::ZPlane(ZPlane {
                z0: 40.0,
                bc: BoundaryType::Vacuum,
            }),
        ];
        let mut block = vec![hs(0, false), hs(1, true), RegionToken::Intersection];
        block.extend([hs(2, false), RegionToken::Intersection]);
        let mut cells = Vec::new();
        for i in 0..3 {
            for j in 0..3 {
                let s = surfaces.len();
                surfaces.push(SurfaceKind::ZCylinder(ZCylinder {
                    x0: -30.0 + 30.0 * i as f64,
                    y0: -30.0 + 30.0 * j as f64,
                    r: 5.0,
                    bc: BoundaryType::Transmissive,
                }));
                block.extend([hs(s, true), RegionToken::Intersection]);
                cells.push(Cell::material(
                    10 + s as i32,
                    vec![
                        hs(s, false),
                        hs(1, true),
                        RegionToken::Intersection,
                        hs(2, false),
                        RegionToken::Intersection,
                    ],
                    1,
                    293.6,
                ));
            }
        }
        surfaces.push(SurfaceKind::Sphere(Sphere {
            x0: 0.0,
            y0: 0.0,
            z0: 0.0,
            r: 2.0,
            bc: BoundaryType::Transmissive,
        }));
        cells.insert(0, Cell::material(1, block, 0, 293.6));
        let n = cells.len();
        Geometry {
            surfaces,
            cells,
            universes: vec![Universe {
                id: 0,
                cell_indices: (0..n).collect(),
            }],
            lattices: vec![],
            root_universe: 0,
        }
    }

    /// Folding agrees with `Cell::contains` at random points: the first cell
    /// of the point's voxel list that holds the point (folded tokens) is the
    /// one `find_cell` returns (full regions), everywhere in the grid.
    #[test]
    fn folded_lists_find_the_same_cell() {
        let g = bored();
        let idx = build(&g, 0).expect("bounded");
        assert!(!idx.open);
        let u = Direction::new(0.6, 0.0, 0.8);
        let mut seed: u64 = 12345;
        let mut rnd = || {
            seed = seed
                .wrapping_mul(6364136223846793005)
                .wrapping_add(1442695040888963407);
            (seed >> 11) as f64 / (1u64 << 53) as f64
        };
        let contains = |tokens: &[u32], p: Position| -> bool {
            // Evaluate the encoded tokens as Cell::contains would.
            let region: Vec<RegionToken> = tokens
                .iter()
                .map(|&t| match t {
                    TOK_INTERSECTION => RegionToken::Intersection,
                    TOK_UNION => RegionToken::Union,
                    TOK_COMPLEMENT => RegionToken::Complement,
                    t => hs((t >> 1) as usize, t & 1 == 1),
                })
                .collect();
            Cell::material(0, region, 0, 0.0).contains(p, u, &g.surfaces, SurfaceToken::NONE)
        };
        let mut checked = 0;
        for _ in 0..20_000 {
            let p = Position::new(
                -60.0 + 120.0 * rnd(),
                -60.0 + 120.0 * rnd(),
                -50.0 + 100.0 * rnd(),
            );
            let want = g.universes[0]
                .cell_indices
                .iter()
                .copied()
                .find(|&c| g.cells[c].contains(p, u, &g.surfaces, SurfaceToken::NONE));
            let i = [p.x, p.y, p.z];
            let vi: Option<[usize; 3]> = (0..3)
                .map(|k| {
                    let f = ((i[k] - idx.lo[k]) / idx.d[k]).floor();
                    (f >= 0.0 && (f as usize) < idx.n[k]).then_some(f as usize)
                })
                .collect::<Option<Vec<_>>>()
                .map(|v| [v[0], v[1], v[2]]);
            let got = vi.and_then(|v| {
                let lin = v[0] + idx.n[0] * (v[1] + idx.n[1] * v[2]);
                let l = idx.voxels[lin]?;
                idx.lists[l as usize]
                    .iter()
                    .find(|c| contains(&c.tokens, p))
                    .map(|c| c.cell as usize)
            });
            assert_eq!(got, want, "at {p:?}");
            checked += usize::from(want.is_some());
        }
        assert!(checked > 5_000, "{checked} points in a cell");
        let (cands, toks) = idx.mean_load();
        // The block cell holds 23 tokens in full; a voxel sees a few.
        assert!(toks < 10.0, "mean folded tokens per voxel {toks}");
        assert!(cands < 3.0, "mean candidates per voxel {cands}");
    }

    /// A slot between two pairs of oblique planes and a cone cut by two
    /// z-planes have unbounded OpenMC boxes; [`cell_tight_box`] bounds them
    /// (and still holds every point of the cell), so a universe of them gets
    /// a closed grid.
    #[test]
    fn unbounded_openmc_boxes_are_tightened() {
        let t = BoundaryType::Transmissive;
        let (c, sn) = (0.6_f64, 0.8_f64);
        let surfaces = vec![
            SurfaceKind::Plane(Plane {
                a: c,
                b: sn,
                c: 0.0,
                d: 95.0,
                bc: t,
            }),
            SurfaceKind::Plane(Plane {
                a: c,
                b: sn,
                c: 0.0,
                d: 101.0,
                bc: t,
            }),
            SurfaceKind::Plane(Plane {
                a: -sn,
                b: c,
                c: 0.0,
                d: -5.0,
                bc: t,
            }),
            SurfaceKind::Plane(Plane {
                a: -sn,
                b: c,
                c: 0.0,
                d: 5.0,
                bc: t,
            }),
            SurfaceKind::ZPlane(ZPlane { z0: -10.0, bc: t }),
            SurfaceKind::ZPlane(ZPlane { z0: 10.0, bc: t }),
            SurfaceKind::ZCone(ZCone {
                x0: 0.0,
                y0: 0.0,
                z0: -30.0,
                r_sq: 3.0,
                bc: t,
            }),
        ];
        let and = |v: Vec<RegionToken>| {
            let n = v.len();
            let mut v = v;
            v.extend(std::iter::repeat_n(RegionToken::Intersection, n - 1));
            v
        };
        let slot = and(vec![
            hs(0, true),
            hs(1, false),
            hs(2, true),
            hs(3, false),
            hs(4, true),
            hs(5, false),
        ]);
        let cone = and(vec![hs(6, false), hs(4, true), hs(5, false)]);
        let g = Geometry {
            surfaces,
            cells: vec![
                Cell::material(1, slot, 0, 293.6),
                Cell::material(2, cone, 1, 293.6),
            ],
            universes: vec![Universe {
                id: 0,
                cell_indices: vec![0, 1],
            }],
            lattices: vec![],
            root_universe: 0,
        };
        for c in 0..2 {
            let o = cell_bounding_box(&g, &g.cells[c]);
            assert!(!o.lower_left[0].is_finite(), "OpenMC's box is unbounded");
            let (lo, hi) = cell_tight_box(&g, c);
            assert!(
                (0..3).all(|k| lo[k].is_finite() && hi[k].is_finite()),
                "{lo:?} {hi:?}"
            );
            // Conservative: every sampled point of the cell is in the box.
            let u = Direction::new(1.0, 0.0, 0.0);
            let mut n_in = 0;
            for i in 0..200 {
                for j in 0..200 {
                    for k in 0..5 {
                        let p = Position::new(
                            -150.0 + 1.5 * f64::from(i),
                            -150.0 + 1.5 * f64::from(j),
                            -12.0 + 6.0 * f64::from(k),
                        );
                        if g.cells[c].contains(p, u, &g.surfaces, SurfaceToken::NONE) {
                            n_in += 1;
                            let q = [p.x, p.y, p.z];
                            assert!((0..3).all(|k| lo[k] <= q[k] && q[k] <= hi[k]), "{q:?}");
                        }
                    }
                }
            }
            assert!(n_in > 20, "cell {c}: {n_in} points");
        }
        // The cone cell's radius at z = 10 is sqrt(3) * 40 = 69.3 cm.
        let (lo, hi) = cell_tight_box(&g, 1);
        assert!((hi[0] - 3.0_f64.sqrt() * 40.0).abs() < 1e-6 && (lo[0] + hi[0]).abs() < 1e-6);
        assert!(
            !build(&g, 0).expect("bounded").open,
            "both cells bounded: closed grid"
        );
    }

    /// Exact side tests: a sphere's box corners, a cone and a plane.
    #[test]
    fn surfaces_are_classified_exactly() {
        let s = SurfaceKind::Sphere(Sphere {
            x0: 0.0,
            y0: 0.0,
            z0: 0.0,
            r: 1.0,
            bc: BoundaryType::Transmissive,
        });
        // Box wholly inside the ball, wholly outside, and straddling.
        assert_eq!(classify(&s, [-0.5; 3], [0.5; 3]), Side::Negative);
        assert_eq!(classify(&s, [0.7; 3], [2.0; 3]), Side::Positive);
        assert_eq!(classify(&s, [0.5; 3], [2.0; 3]), Side::Through);
        let p = SurfaceKind::ZPlane(ZPlane {
            z0: 3.0,
            bc: BoundaryType::Transmissive,
        });
        assert_eq!(classify(&p, [0.0; 3], [1.0; 3]), Side::Negative);
        assert_eq!(
            classify(&p, [0.0, 0.0, 3.5], [1.0, 1.0, 4.0]),
            Side::Positive
        );
    }
}
