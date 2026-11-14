// SPDX-License-Identifier: GPL-3.0-only
// Copyright (C) 2026 OUTRAM PARK contributors
// Derived from OpenFOAM (www.openfoam.com), Copyright (C) 2011-2016 OpenFOAM
// Foundation, 2016-2026 OpenCFD Ltd (ESI), GPL-3.0-or-later.

//! (Moved from `outram-foam-mesh` to this crate on 2026-11-14 for gh:#1564; `outram_foam_mesh`
//! re-exports it, behaviour unchanged. `OctreeShapes` was split into `OctreeShapes` +
//! `NearestShapes` so a shape type that only needs `findLine` need not provide `findNearest`.)
//!
//! `indexedOctree` of OpenFOAM.com ESI v2506, translated so that **nearest-shape queries visit
//! the shapes in upstream's order** (gh:#1467 round 7).
//!
//! Why it exists: `snappyHexMesh`'s feature snapping asks `indexedOctree::findNearest` for the
//! nearest feature edge / surface triangle of a sample, and upstream keeps the FIRST shape at a
//! strictly smaller distance (`distSqr < nearestDistSqr`). When two shapes are at exactly the
//! same distance the answer is whichever the traversal visits first, so reproducing ESI
//! bit for bit needs the same tree and the same traversal. Round 6 brute-forced the shapes in
//! index order; four mirror-image valence-3 corner points chose the other edge than ESI
//! (README, "Round 7").
//!
//! Translated (upstream file : routine):
//! - `indexedOctree.C` : `divide`, `splitNodes`, the constructor level loop, `findNearest`
//!   (sample overload). `compactContents` only renumbers the leaf storage (the sub-node
//!   references follow), so it is not needed for the traversal.
//! - `treeBoundBox.C`, `treeBoundBoxI.H`, `boundBox.C`, `boundBoxI.H` : `subBbox`, `subOctant`,
//!   `searchOrder`, `subOverlaps` (sphere and box), `overlaps`, `intersects` (Sutherland
//!   segment clip and the separating-axis triangle test), `inflate`.
//! - `Random.C` / `Rand48.H` : the `lrand48`-compatible generator and libstdc++'s
//!   `uniform_real_distribution<double>` (`generate_canonical`, two 31-bit draws).
//! - `treeDataEdge.C`, `treeDataPrimitivePatch.C` : `overlaps(index, box)` and `findNearest`.
//!
//! Not translated: `findLine`, `findInside`, volume types, the node-type bit sets, the
//! `linePointRef` overload of `findNearest`.

/// A point or vector.
pub type V = [f64; 3];

const VSMALL: f64 = 1e-300;
const ROOTVSMALL: f64 = 1e-150;

// ---------------------------------------------------------------------------------------
// Random (Rand48 + std::uniform_real_distribution<double>)
// ---------------------------------------------------------------------------------------

/// `Foam::Random`: `Rand48` (`lrand48`) driving `std::uniform_real_distribution<double>` on
/// [0, 1).
#[derive(Debug, Clone)]
pub struct EsiRandom {
    state: u64,
}

impl EsiRandom {
    /// `Random(seed)`: `Rand48::convert`, `(seed << 16) | 0x330e`.
    pub fn new(seed: u32) -> Self {
        Self { state: ((seed as u64) << 16) | 0x330e }
    }

    /// `Rand48::operator()`: one 31-bit draw.
    fn next31(&mut self) -> u64 {
        self.state = (self.state.wrapping_mul(0x5DEE_CE66D).wrapping_add(0xB)) & ((1u64 << 48) - 1);
        self.state >> 17
    }

    /// `scalar01()`: libstdc++ `generate_canonical<double, 53>` with a 2^31-wide generator
    /// (`k = 2` draws, the first the low part), clamped below one.
    pub fn scalar01(&mut self) -> f64 {
        let r = 2147483648.0f64;
        let mut sum = 0.0f64;
        let mut tmp = 1.0f64;
        for _ in 0..2 {
            sum += (self.next31() as f64) * tmp;
            tmp *= r;
        }
        let ret = sum / tmp;
        if ret >= 1.0 {
            // `std::nextafter(1, 0)`
            f64::from_bits(1.0f64.to_bits() - 1)
        } else {
            ret
        }
    }

    /// `sample01<vector>()`: three draws, x then y then z.
    pub fn sample01_vector(&mut self) -> V {
        let x = self.scalar01();
        let y = self.scalar01();
        let z = self.scalar01();
        [x, y, z]
    }
}

// ---------------------------------------------------------------------------------------
// treeBoundBox
// ---------------------------------------------------------------------------------------

const RIGHTHALF: u8 = 1;
const TOPHALF: u8 = 2;
const FRONTHALF: u8 = 4;
const LEFTBIT: u8 = 1;
const RIGHTBIT: u8 = 2;
const BOTTOMBIT: u8 = 4;
const TOPBIT: u8 = 8;
const BACKBIT: u8 = 16;
const FRONTBIT: u8 = 32;

/// `treeBoundBox`.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct TreeBox {
    /// Minimum corner.
    pub min: V,
    /// Maximum corner.
    pub max: V,
}

pub fn sub(a: V, b: V) -> V {
    [a[0] - b[0], a[1] - b[1], a[2] - b[2]]
}

pub fn dot(a: V, b: V) -> f64 {
    a[0] * b[0] + a[1] * b[1] + a[2] * b[2]
}

pub fn cross(a: V, b: V) -> V {
    [a[1] * b[2] - a[2] * b[1], a[2] * b[0] - a[0] * b[2], a[0] * b[1] - a[1] * b[0]]
}

impl TreeBox {
    /// `treeBoundBox(points)`: the bounds of a point list.
    pub fn of_points(points: &[V]) -> Self {
        let mut min = [f64::MAX; 3];
        let mut max = [-f64::MAX; 3];
        for p in points {
            for c in 0..3 {
                min[c] = min[c].min(p[c]);
                max[c] = max[c].max(p[c]);
            }
        }
        Self { min, max }
    }

    /// `boundBox::centre`: `0.5*(min + max)`.
    pub fn centre(&self) -> V {
        [0.5 * (self.min[0] + self.max[0]), 0.5 * (self.min[1] + self.max[1]), 0.5 * (self.min[2] + self.max[2])]
    }

    /// `boundBox::inflate(rndGen, factor, delta)`.
    pub fn inflate(&mut self, rnd: &mut EsiRandom, factor: f64, delta: f64) {
        let span = sub(self.max, self.min);
        let mag = dot(span, span).sqrt();
        let min_span = factor * mag;
        let new_span = [span[0].max(min_span), span[1].max(min_span), span[2].max(min_span)];
        let r = rnd.sample01_vector();
        for c in 0..3 {
            self.min[c] -= (factor * r[c]) * new_span[c];
        }
        let r = rnd.sample01_vector();
        for c in 0..3 {
            self.max[c] += (factor * r[c]) * new_span[c];
        }
        for c in 0..3 {
            self.min[c] -= delta;
            self.max[c] += delta;
        }
    }

    /// The search domain of every tree of `refinementFeatures` and `triSurfaceSearch`:
    /// `treeBoundBox bb(points); Random rndGen(65431); bb.inflate(rndGen, 1e-4, ROOTVSMALL)`.
    pub fn search_domain(points: &[V]) -> Self {
        let mut bb = Self::of_points(points);
        let mut rnd = EsiRandom::new(65431);
        bb.inflate(&mut rnd, 1e-4, ROOTVSMALL);
        bb
    }

    fn corner(&self, octant: u8) -> V {
        [
            if octant & RIGHTHALF != 0 { self.max[0] } else { self.min[0] },
            if octant & TOPHALF != 0 { self.max[1] } else { self.min[1] },
            if octant & FRONTHALF != 0 { self.max[2] } else { self.min[2] },
        ]
    }

    /// `subBbox(mid, octant)`.
    pub fn sub_bbox(&self, mid: V, octant: u8) -> TreeBox {
        let mut bb = TreeBox { min: mid, max: mid };
        if octant & RIGHTHALF != 0 {
            bb.max[0] = self.max[0];
        } else {
            bb.min[0] = self.min[0];
        }
        if octant & TOPHALF != 0 {
            bb.max[1] = self.max[1];
        } else {
            bb.min[1] = self.min[1];
        }
        if octant & FRONTHALF != 0 {
            bb.max[2] = self.max[2];
        } else {
            bb.min[2] = self.min[2];
        }
        bb
    }

    /// `boundBox::box_box_overlaps`.
    fn box_box_overlaps(min_a: V, max_a: V, min_b: V, max_b: V) -> bool {
        min_a[0] <= max_b[0]
            && min_b[0] <= max_a[0]
            && min_a[1] <= max_b[1]
            && min_b[1] <= max_a[1]
            && min_a[2] <= max_b[2]
            && min_b[2] <= max_a[2]
    }

    /// `boundBox::box_sphere_overlaps`.
    fn box_sphere_overlaps(c0: V, c1: V, centre: V, radius_sqr: f64) -> bool {
        let mut dist_sqr = 0.0;
        for d in 0..3 {
            let d0 = c0[d] - centre[d];
            let d1 = c1[d] - centre[d];
            if (d0 > 0.0) != (d1 > 0.0) {
                // centre inside both extrema
            } else {
                dist_sqr += (d0 * d0).min(d1 * d1);
                if dist_sqr > radius_sqr {
                    return false;
                }
            }
        }
        true
    }

    /// `boundBox::overlaps(centre, radiusSqr)`.
    pub fn overlaps_sphere(&self, centre: V, radius_sqr: f64) -> bool {
        Self::box_sphere_overlaps(self.min, self.max, centre, radius_sqr)
    }

    /// `treeBoundBox::subOverlaps(octant, centre, radiusSqr)`.
    pub fn sub_overlaps_sphere(&self, octant: u8, centre: V, radius_sqr: f64) -> bool {
        Self::box_sphere_overlaps(self.centre(), self.corner(octant), centre, radius_sqr)
    }

    /// `boundBox::overlaps(bb)`.
    pub fn overlaps_box(&self, other: &TreeBox) -> bool {
        Self::box_box_overlaps(self.min, self.max, other.min, other.max)
    }

    /// `treeBoundBox::subOverlaps(octant, bb)`.
    pub fn sub_overlaps_box(&self, octant: u8, bb: &TreeBox) -> bool {
        let mut sub_min = self.centre();
        let mut sub_max = sub_min;
        if octant & RIGHTHALF != 0 {
            sub_max[0] = self.max[0];
        } else {
            sub_min[0] = self.min[0];
        }
        if octant & TOPHALF != 0 {
            sub_max[1] = self.max[1];
        } else {
            sub_min[1] = self.min[1];
        }
        if octant & FRONTHALF != 0 {
            sub_max[2] = self.max[2];
        } else {
            sub_min[2] = self.min[2];
        }
        Self::box_box_overlaps(sub_min, sub_max, bb.min, bb.max)
    }

    /// `treeBoundBox::searchOrder(pt)`: the octant containing `pt` first, then the face,
    /// edge and corner neighbours by increasing distance of `pt` to the dividing planes.
    pub fn search_order(&self, pt: V) -> [u8; 8] {
        let c = self.centre();
        let mut dist = [c[0] - pt[0], c[1] - pt[1], c[2] - pt[2]];
        let mut octant = 0u8;
        if dist[0] < 0.0 {
            octant |= RIGHTHALF;
            dist[0] *= -1.0;
        }
        if dist[1] < 0.0 {
            octant |= TOPHALF;
            dist[1] *= -1.0;
        }
        if dist[2] < 0.0 {
            octant |= FRONTHALF;
            dist[2] *= -1.0;
        }
        let (min, mid, max);
        if dist[0] < dist[1] {
            if dist[1] < dist[2] {
                min = RIGHTHALF;
                mid = TOPHALF;
                max = FRONTHALF;
            } else if dist[2] < dist[0] {
                min = FRONTHALF;
                mid = RIGHTHALF;
                max = TOPHALF;
            } else {
                min = RIGHTHALF;
                mid = FRONTHALF;
                max = TOPHALF;
            }
        } else if dist[2] < dist[1] {
            min = FRONTHALF;
            mid = TOPHALF;
            max = RIGHTHALF;
        } else if dist[0] < dist[2] {
            min = TOPHALF;
            mid = RIGHTHALF;
            max = FRONTHALF;
        } else {
            min = TOPHALF;
            mid = FRONTHALF;
            max = RIGHTHALF;
        }
        let mut o = [0u8; 8];
        o[0] = octant;
        o[1] = octant ^ min;
        o[2] = octant ^ mid;
        o[3] = octant ^ max;
        o[4] = o[1] ^ mid;
        o[5] = o[1] ^ max;
        o[6] = o[2] ^ max;
        o[7] = o[4] ^ max;
        o
    }

    fn pos_bits(&self, pt: V) -> u8 {
        let mut o = 0u8;
        if pt[0] < self.min[0] {
            o |= LEFTBIT;
        } else if pt[0] > self.max[0] {
            o |= RIGHTBIT;
        }
        if pt[1] < self.min[1] {
            o |= BOTTOMBIT;
        } else if pt[1] > self.max[1] {
            o |= TOPBIT;
        }
        if pt[2] < self.min[2] {
            o |= BACKBIT;
        } else if pt[2] > self.max[2] {
            o |= FRONTBIT;
        }
        o
    }

    /// `treeBoundBox::intersects(overallStart, overallVec, start, end, pt, ptOnFaces)`:
    /// Sutherland's segment clip, verbatim (including upstream's `pt.x() = min().y()` in the
    /// zero-y-component branch). Returns `(intersects, pt, ptOnFaces)`.
    pub fn clip(&self, overall_start: V, overall_vec: V, start: V, end: V) -> (bool, V, u8) {
        let end_bits = self.pos_bits(end);
        let mut pt = start;
        for _ in 0..4 {
            let pt_bits = self.pos_bits(pt);
            if pt_bits == 0 {
                return (true, pt, self.face_bits(pt));
            }
            if pt_bits & end_bits != 0 {
                return (false, pt, self.face_bits(pt));
            }
            if pt_bits & LEFTBIT != 0 {
                if overall_vec[0].abs() > VSMALL {
                    let s = (self.min[0] - overall_start[0]) / overall_vec[0];
                    pt[0] = self.min[0];
                    pt[1] = overall_start[1] + overall_vec[1] * s;
                    pt[2] = overall_start[2] + overall_vec[2] * s;
                } else {
                    pt[0] = self.min[0];
                }
            } else if pt_bits & RIGHTBIT != 0 {
                if overall_vec[0].abs() > VSMALL {
                    let s = (self.max[0] - overall_start[0]) / overall_vec[0];
                    pt[0] = self.max[0];
                    pt[1] = overall_start[1] + overall_vec[1] * s;
                    pt[2] = overall_start[2] + overall_vec[2] * s;
                } else {
                    pt[0] = self.max[0];
                }
            } else if pt_bits & BOTTOMBIT != 0 {
                if overall_vec[1].abs() > VSMALL {
                    let s = (self.min[1] - overall_start[1]) / overall_vec[1];
                    pt[0] = overall_start[0] + overall_vec[0] * s;
                    pt[1] = self.min[1];
                    pt[2] = overall_start[2] + overall_vec[2] * s;
                } else {
                    // upstream: `pt.x() = min().y();`
                    pt[0] = self.min[1];
                }
            } else if pt_bits & TOPBIT != 0 {
                if overall_vec[1].abs() > VSMALL {
                    let s = (self.max[1] - overall_start[1]) / overall_vec[1];
                    pt[0] = overall_start[0] + overall_vec[0] * s;
                    pt[1] = self.max[1];
                    pt[2] = overall_start[2] + overall_vec[2] * s;
                } else {
                    pt[1] = self.max[1];
                }
            } else if pt_bits & BACKBIT != 0 {
                if overall_vec[2].abs() > VSMALL {
                    let s = (self.min[2] - overall_start[2]) / overall_vec[2];
                    pt[0] = overall_start[0] + overall_vec[0] * s;
                    pt[1] = overall_start[1] + overall_vec[1] * s;
                    pt[2] = self.min[2];
                } else {
                    pt[2] = self.min[2];
                }
            } else if pt_bits & FRONTBIT != 0 {
                if overall_vec[2].abs() > VSMALL {
                    let s = (self.max[2] - overall_start[2]) / overall_vec[2];
                    pt[0] = overall_start[0] + overall_vec[0] * s;
                    pt[1] = overall_start[1] + overall_vec[1] * s;
                    pt[2] = self.max[2];
                } else {
                    pt[2] = self.max[2];
                }
            }
        }
        // can end up here if the end point is on the edge of the box
        (true, pt, self.face_bits(pt))
    }

    /// `treeBoundBox::intersects(start, end, pt)` as a yes/no.
    pub fn intersects_segment(&self, start: V, end: V) -> bool {
        self.clip(start, sub(end, start), start, end).0
    }

    /// `treeBoundBox::faceBits(pt)`: the faces the point lies exactly on.
    pub fn face_bits(&self, pt: V) -> u8 {
        let mut o = 0u8;
        if pt[0] == self.min[0] {
            o |= LEFTBIT;
        } else if pt[0] == self.max[0] {
            o |= RIGHTBIT;
        }
        if pt[1] == self.min[1] {
            o |= BOTTOMBIT;
        } else if pt[1] == self.max[1] {
            o |= TOPBIT;
        }
        if pt[2] == self.min[2] {
            o |= BACKBIT;
        } else if pt[2] == self.max[2] {
            o |= FRONTBIT;
        }
        o
    }

    /// `treeBoundBox::subOctant(pt)`: the octant (about the centre) a point lies in.
    pub fn sub_octant(&self, pt: V) -> u8 {
        let mid = self.centre();
        let mut o = 0u8;
        if pt[0] > mid[0] {
            o |= RIGHTHALF;
        }
        if pt[1] > mid[1] {
            o |= TOPHALF;
        }
        if pt[2] > mid[2] {
            o |= FRONTHALF;
        }
        o
    }

    /// `boundBox::contains(pt)`: inclusive.
    pub fn contains(&self, p: V) -> bool {
        self.min[0] <= p[0]
            && p[0] <= self.max[0]
            && self.min[1] <= p[1]
            && p[1] <= self.max[1]
            && self.min[2] <= p[2]
            && p[2] <= self.max[2]
    }

    /// `hexCorner<0..7>`.
    fn hex_corners(&self) -> [V; 8] {
        let (mn, mx) = (self.min, self.max);
        [
            mn,
            [mx[0], mn[1], mn[2]],
            [mx[0], mx[1], mn[2]],
            [mn[0], mx[1], mn[2]],
            [mn[0], mn[1], mx[2]],
            [mx[0], mn[1], mx[2]],
            mx,
            [mn[0], mx[1], mx[2]],
        ]
    }

    /// `boundBox::intersects(triPointRef)`: corner containment then the separating-axis
    /// test (box normals, triangle normal, nine edge cross products).
    pub fn intersects_triangle(&self, a: V, b: V, c: V) -> bool {
        if self.contains(a) || self.contains(b) || self.contains(c) {
            return true;
        }
        let corners = self.hex_corners();
        let project_box = |axis: V| -> (f64, f64) {
            let mut lo = dot(axis, corners[0]);
            let mut hi = lo;
            for k in corners.iter().skip(1) {
                let d = dot(axis, *k);
                lo = lo.min(d);
                hi = hi.max(d);
            }
            (lo, hi)
        };
        let overlaps = |x: (f64, f64), y: (f64, f64)| x.0 <= y.1 && y.0 <= x.1;
        for d in 0..3 {
            let lo = a[d].min(b[d]).min(c[d]);
            let hi = a[d].max(b[d]).max(c[d]);
            if !overlaps((lo, hi), (self.min[d], self.max[d])) {
                return false;
            }
        }
        // 0.5*((b - a) ^ (c - a))
        let n = cross(sub(b, a), sub(c, a));
        let axis = [0.5 * n[0], 0.5 * n[1], 0.5 * n[2]];
        let t = dot(axis, a);
        if !overlaps((t, t), project_box(axis)) {
            return false;
        }
        for edge_vec in [sub(c, b), sub(a, c), sub(b, a)] {
            for face_dir in 0..3 {
                let mut unit = [0.0; 3];
                unit[face_dir] = 1.0;
                let axis = cross(edge_vec, unit);
                let da = dot(axis, a);
                let db = dot(axis, b);
                let dc = dot(axis, c);
                let tri = (da.min(db).min(dc), da.max(db).max(dc));
                if !overlaps(tri, project_box(axis)) {
                    return false;
                }
            }
        }
        true
    }
}

// ---------------------------------------------------------------------------------------
// indexedOctree
// ---------------------------------------------------------------------------------------

/// The shape contract of `indexedOctree<Type>`: the two members the build and the nearest
/// search use.
pub trait OctreeShapes {
    /// Number of shapes.
    fn len(&self) -> usize;
    /// `overlaps(index, searchBox)`.
    fn overlaps_box(&self, index: usize, bb: &TreeBox) -> bool;
}

/// The extra member of the shapes a nearest-shape query (`findNearest`) needs.
pub trait NearestShapes: OctreeShapes {
    /// `findNearest(indices, sample, nearestDistSqr, minIndex, nearestPoint)`: strictly smaller
    /// distance wins, in the order of `indices`.
    fn find_nearest(&self, indices: &[usize], sample: V, nearest_dist_sqr: &mut f64, min_index: &mut Option<usize>, nearest_point: &mut V);
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Sub {
    Empty,
    Node(usize),
    Content(usize),
}

#[derive(Debug, Clone)]
struct Node {
    bb: TreeBox,
    subs: [Sub; 8],
    /// `node::parent_`: the node holding this one, `None` for the top node.
    parent: Option<usize>,
}

/// `indexedOctree<Type>`.
#[derive(Debug, Clone)]
pub struct Octree<S: OctreeShapes> {
    shapes: S,
    nodes: Vec<Node>,
    contents: Vec<Vec<usize>>,
}

fn divide_indices<S: OctreeShapes>(shapes: &S, indices: &[usize], bb: &TreeBox) -> [Vec<usize>; 8] {
    let mut out: [Vec<usize>; 8] = Default::default();
    let mid = bb.centre();
    for (octant, slot) in out.iter_mut().enumerate() {
        let sub_bb = bb.sub_bbox(mid, octant as u8);
        for &i in indices {
            if shapes.overlaps_box(i, &sub_bb) {
                slot.push(i);
            }
        }
    }
    out
}

/// `divide(bb, contents, contentIndex)`: the first non-empty subset replaces the current
/// content, the others are appended.
fn divide_node<S: OctreeShapes>(shapes: &S, bb: &TreeBox, contents: &mut Vec<Vec<usize>>, content_index: usize) -> Node {
    let indices = contents[content_index].clone();
    let divided = divide_indices(shapes, &indices, bb);
    let mut subs = [Sub::Empty; 8];
    let mut replace = true;
    let mut ci = content_index;
    for (octant, sub_indices) in divided.into_iter().enumerate() {
        if sub_indices.is_empty() {
            subs[octant] = Sub::Empty;
        } else {
            if replace {
                contents[ci] = sub_indices;
                replace = false;
            } else {
                ci = contents.len();
                contents.push(sub_indices);
            }
            subs[octant] = Sub::Content(ci);
        }
    }
    Node { bb: *bb, subs, parent: None }
}

impl<S: OctreeShapes> Octree<S> {
    /// `indexedOctree(shapes, bb, maxLevels, maxLeafRatio, maxDuplicity)`.
    pub fn new(shapes: S, bb: TreeBox, max_levels: usize, max_leaf_ratio: usize, max_duplicity: f64) -> Self {
        if shapes.len() == 0 {
            return Self { shapes, nodes: Vec::new(), contents: Vec::new() };
        }
        let mut nodes: Vec<Node> = Vec::new();
        let mut contents: Vec<Vec<usize>> = vec![(0..shapes.len()).collect()];
        let top = divide_node(&shapes, &bb, &mut contents, 0);
        nodes.push(top);
        let mut n_levels = 1;
        while n_levels < max_levels {
            let n_entries: usize = contents.iter().map(|c| c.len()).sum();
            if n_entries as f64 > max_duplicity * shapes.len() as f64 {
                break;
            }
            let n_old = nodes.len();
            // splitNodes: only the old nodes
            for node_i in 0..n_old {
                for octant in 0..8usize {
                    if let Sub::Content(content_i) = nodes[node_i].subs[octant] {
                        if contents[content_i].len() > max_leaf_ratio {
                            let sub_bb = nodes[node_i].bb.sub_bbox(nodes[node_i].bb.centre(), octant as u8);
                            let sub_node = divide_node(&shapes, &sub_bb, &mut contents, content_i);
                            let sz = nodes.len();
                            let mut sub_node = sub_node;
                            sub_node.parent = Some(node_i);
                            nodes.push(sub_node);
                            nodes[node_i].subs[octant] = Sub::Node(sz);
                        }
                    }
                }
            }
            if n_old == nodes.len() {
                break;
            }
            n_levels += 1;
        }
        Self { shapes, nodes, contents }
    }

    /// The shapes.
    pub fn shapes(&self) -> &S {
        &self.shapes
    }

    /// Number of nodes (a build check).
    pub fn n_nodes(&self) -> usize {
        self.nodes.len()
    }
}

impl<S: NearestShapes> Octree<S> {
    fn find_nearest_node(&self, node_i: usize, sample: V, nearest_dist_sqr: &mut f64, nearest_shape: &mut Option<usize>, nearest_point: &mut V) {
        let nod = &self.nodes[node_i];
        for octant in nod.bb.search_order(sample) {
            match nod.subs[octant as usize] {
                Sub::Node(sub_i) => {
                    if self.nodes[sub_i].bb.overlaps_sphere(sample, *nearest_dist_sqr) {
                        self.find_nearest_node(sub_i, sample, nearest_dist_sqr, nearest_shape, nearest_point);
                    }
                }
                Sub::Content(ci) => {
                    if nod.bb.sub_overlaps_sphere(octant, sample, *nearest_dist_sqr) {
                        self.shapes.find_nearest(&self.contents[ci], sample, nearest_dist_sqr, nearest_shape, nearest_point);
                    }
                }
                Sub::Empty => {}
            }
        }
    }

    /// `findNearest(sample, startDistSqr)`: the shape index and nearest point, when one is
    /// at a squared distance below `start_dist_sqr`.
    pub fn find_nearest(&self, sample: V, start_dist_sqr: f64) -> Option<(usize, V)> {
        let mut d = start_dist_sqr;
        let mut shape = None;
        let mut pt = [0.0; 3];
        if !self.nodes.is_empty() {
            self.find_nearest_node(0, sample, &mut d, &mut shape, &mut pt);
        }
        shape.map(|s| (s, pt))
    }
}


// ---------------------------------------------------------------------------------------
// findLine (gh:#1564)
// ---------------------------------------------------------------------------------------

/// `indexedOctreeBase::perturbTol_ = 10*SMALL`, the value `triSurfaceSearch` leaves in force while
/// it calls `findLine`.
pub const PERTURB_TOL: f64 = 10.0 * crate::primitives::SMALL;

/// The shapes a `findLine` query needs: upstream's `findIntersectOp`.
pub trait LineShapes: OctreeShapes {
    /// `findIntersectOp::operator()(index, start, end, pt)`: the intersection point of the
    /// segment `start .. end` with shape `index`, if any.
    fn find_intersection(&self, index: usize, start: V, end: V) -> Option<V>;
}

/// `pointIndexHit` of a `findLine`: the hit shape and point.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct LineHit {
    /// Index of the shape that was hit.
    pub index: usize,
    /// Intersection point on that shape.
    pub point: V,
}

/// Working `pointIndexHit` of the traversal.
struct Walk {
    hit: bool,
    point: V,
    index: usize,
}

fn perturb_vec(bb: &TreeBox) -> V {
    [
        PERTURB_TOL * (bb.max[0] - bb.min[0]),
        PERTURB_TOL * (bb.max[1] - bb.min[1]),
        PERTURB_TOL * (bb.max[2] - bb.min[2]),
    ]
}

/// `indexedOctree::pushPoint(bb, pt, pushInside)`.
fn push_point(bb: &TreeBox, pt: V, push_inside: bool) -> V {
    let perturb = perturb_vec(bb);
    let mut out = pt;
    for d in 0..3 {
        if (pt[d] - bb.min[d]).abs() < perturb[d].abs() {
            let dist = perturb[d] + ROOTVSMALL;
            out[d] = if push_inside { bb.min[d] + dist } else { bb.min[d] - dist };
        } else if (pt[d] - bb.max[d]).abs() < perturb[d].abs() {
            let dist = perturb[d] + ROOTVSMALL;
            out[d] = if push_inside { bb.max[d] - dist } else { bb.max[d] + dist };
        }
    }
    out
}

/// `indexedOctree::pushPoint(bb, faceID, pt, pushInside)`.
fn push_point_face(bb: &TreeBox, face_id: u8, pt: V, push_inside: bool) -> V {
    let perturb = perturb_vec(bb);
    let mut out = pt;
    let lo_bits = [LEFTBIT, BOTTOMBIT, BACKBIT];
    let hi_bits = [RIGHTBIT, TOPBIT, FRONTBIT];
    for d in 0..3 {
        if face_id & lo_bits[d] != 0 {
            out[d] = if push_inside { bb.min[d] + (perturb[d] + ROOTVSMALL) } else { bb.min[d] - (perturb[d] + ROOTVSMALL) };
        } else if face_id & hi_bits[d] != 0 {
            out[d] = if push_inside { bb.max[d] - (perturb[d] + ROOTVSMALL) } else { bb.max[d] + (perturb[d] + ROOTVSMALL) };
        }
    }
    out
}

/// `indexedOctree::pushPointIntoFace(bb, dir, pt)`: keep a point on the face most
/// perpendicular to the ray, away from edges and corners.
fn push_point_into_face(bb: &TreeBox, dir: V, pt: V) -> V {
    let pt_face = bb.face_bits(pt);
    // (axis, is_max_side) of the faces the point is on, in upstream's order x, y, z
    let mut faces: Vec<(usize, bool)> = Vec::with_capacity(3);
    if pt_face & LEFTBIT != 0 {
        faces.push((0, false));
    } else if pt_face & RIGHTBIT != 0 {
        faces.push((0, true));
    }
    if pt_face & BOTTOMBIT != 0 {
        faces.push((1, false));
    } else if pt_face & TOPBIT != 0 {
        faces.push((1, true));
    }
    if pt_face & BACKBIT != 0 {
        faces.push((2, false));
    } else if pt_face & FRONTBIT != 0 {
        faces.push((2, true));
    }
    if faces.is_empty() {
        return pt;
    }
    let mut keep = faces[0];
    if faces.len() > 1 {
        // mag(faceNormals[face] & dir): the normal is +-e_axis, so the product is +-dir[axis]
        let mut max_inproduct = dir[keep.0].abs();
        for &f in faces.iter().skip(1) {
            let s = dir[f.0].abs();
            if s > max_inproduct {
                max_inproduct = s;
                keep = f;
            }
        }
    }
    let mut face_point = push_point(bb, pt, true);
    face_point[keep.0] = if keep.1 { bb.max[keep.0] } else { bb.min[keep.0] };
    face_point
}

impl<S: OctreeShapes> Octree<S> {
    /// `subBbox(parentNodeI, octant)`: the stored box of a sub-node, else the octant of the node box.
    fn sub_bbox_of(&self, node_i: usize, octant: u8) -> TreeBox {
        let nod = &self.nodes[node_i];
        if let Sub::Node(child) = nod.subs[octant as usize] {
            return self.nodes[child].bb;
        }
        nod.bb.sub_bbox(nod.bb.centre(), octant)
    }

    /// `findNode(nodeI, sample)`: the lowest-level `(node, octant)` containing the sample.
    fn find_node(&self, node_i: usize, sample: V) -> (usize, u8) {
        let mut n = node_i;
        loop {
            let nod = &self.nodes[n];
            let octant = nod.bb.sub_octant(sample);
            match nod.subs[octant as usize] {
                Sub::Node(child) => n = child,
                _ => return (n, octant),
            }
        }
    }

    /// `walkToParent`: the parent of `node_i` and the octant of it that holds `node_i`.
    fn walk_to_parent(&self, node_i: usize) -> Option<(usize, u8)> {
        let parent = self.nodes[node_i].parent?;
        for i in 0..8u8 {
            if self.nodes[parent].subs[i as usize] == Sub::Node(node_i) {
                return Some((parent, i));
            }
        }
        // upstream: FatalError "no parent found"
        panic!("octree node {node_i} is not among the sub-nodes of its parent {parent}");
    }

    /// `walkToNeighbour(facePoint, faceID, nodeI, octant)`: false when the edge of the tree is hit.
    fn walk_to_neighbour(&self, face_point: V, face_id: u8, node_i: &mut usize, octant: &mut u8) -> bool {
        let x = RIGHTHALF;
        let y = TOPHALF;
        let z = FRONTHALF;
        let mut octant_mask = 0u8;
        let mut wanted = 0u8;
        if face_id & LEFTBIT != 0 {
            octant_mask |= x;
            wanted |= x;
        } else if face_id & RIGHTBIT != 0 {
            octant_mask |= x;
        }
        if face_id & BOTTOMBIT != 0 {
            octant_mask |= y;
            wanted |= y;
        } else if face_id & TOPBIT != 0 {
            octant_mask |= y;
        }
        if face_id & BACKBIT != 0 {
            octant_mask |= z;
            wanted |= z;
        } else if face_id & FRONTBIT != 0 {
            octant_mask |= z;
        }
        while wanted != (*octant & octant_mask) {
            for bit in [x, y, z] {
                if wanted & bit != 0 {
                    if *octant & bit != 0 {
                        octant_mask &= !bit;
                        wanted &= !bit;
                    }
                } else if *octant & bit == 0 {
                    octant_mask &= !bit;
                    wanted &= !bit;
                }
            }
            match self.walk_to_parent(*node_i) {
                None => return false,
                Some((p, po)) => {
                    *node_i = p;
                    *octant = po;
                }
            }
        }
        *octant ^= octant_mask;
        if let Sub::Node(child) = self.nodes[*node_i].subs[*octant as usize] {
            let (n, o) = self.find_node(child, face_point);
            *node_i = n;
            *octant = o;
        }
        true
    }
}

impl<S: LineShapes> Octree<S> {
    /// `traverseNode(findAny = false, ...)`: nearest intersection in one node, else the point
    /// where the ray leaves it and the box faces that point lies on.
    fn traverse_node(&self, start: V, end: V, node_i: usize, octant: u8, walk: &mut Walk, hit_bits: &mut u8) {
        let nod = &self.nodes[node_i];
        if let Sub::Content(ci) = nod.subs[octant as usize] {
            let indices = &self.contents[ci];
            if !indices.is_empty() {
                let octant_bb = self.sub_bbox_of(node_i, octant);
                let mut nearest_point = end;
                for &shape_i in indices {
                    if let Some(pt) = self.shapes.find_intersection(shape_i, start, nearest_point) {
                        // an intersection in a neighbouring box is skipped: it is found in that box
                        if octant_bb.contains(pt) {
                            nearest_point = pt;
                            walk.hit = true;
                            walk.point = pt;
                            walk.index = shape_i;
                        }
                    }
                }
                if walk.hit {
                    return;
                }
            }
        }
        // nothing intersected: ray trace back from `end` to the box of this node
        let octant_bb = self.sub_bbox_of(node_i, octant);
        let (intersected, pt, bits) = octant_bb.clip(end, sub(start, end), end, start);
        if intersected {
            walk.point = pt;
            *hit_bits = bits;
        } else {
            // end on a face/edge of the box: retrack with the end pushed out of the box
            let perturbed_end = push_point(&octant_bb, end, false);
            self.traverse_node(start, perturbed_end, node_i, octant, walk, hit_bits);
        }
    }

    /// `findLine(start, end, findIntersectOp)` (nearest intersection): the first shape hit along
    /// the segment, walking the leaves in ray order exactly as upstream.
    pub fn find_line(&self, start: V, end: V) -> Option<LineHit> {
        if self.nodes.is_empty() {
            return None;
        }
        let tree_bb = self.nodes[0].bb;
        let start_bit = tree_bb.pos_bits(start);
        let end_bit = tree_bb.pos_bits(end);
        if start_bit & end_bit != 0 {
            return None;
        }
        let mut track_start = start;
        let mut track_end = end;
        if start_bit != 0 {
            let (ok, pt, _) = tree_bb.clip(start, sub(end, start), start, end);
            if !ok {
                return None;
            }
            track_start = pt;
        }
        if end_bit != 0 {
            let (ok, pt, _) = tree_bb.clip(end, sub(track_start, end), end, track_start);
            if !ok {
                return None;
            }
            track_end = pt;
        }
        let (mut node_i, mut octant) = self.find_node(0, track_start);
        let tree_vec = sub(track_end, track_start);
        let mut walk = Walk { hit: false, point: track_start, index: 0 };
        for _ in 0..100000 {
            let octant_bb = self.sub_bbox_of(node_i, octant);
            let start_point = push_point_into_face(&octant_bb, tree_vec, walk.point);
            let mut hit_face = 0u8;
            self.traverse_node(start_point, track_end, node_i, octant, &mut walk, &mut hit_face);
            if walk.hit {
                break;
            }
            if hit_face == 0 || walk.point == track_end {
                break;
            }
            let perturbed = push_point_face(&octant_bb, hit_face, walk.point, false);
            if !self.walk_to_neighbour(perturbed, hit_face, &mut node_i, &mut octant) {
                break;
            }
        }
        if walk.hit {
            Some(LineHit { index: walk.index, point: walk.point })
        } else {
            None
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rand48_matches_lrand48_first_values() {
        // lrand48() after srand48(1): 89400484, 976015093, 1792756325 (glibc), the 31-bit
        // draws of the same linear congruential recurrence.
        let mut r = EsiRandom::new(1);
        assert_eq!(r.next31(), 89400484);
        assert_eq!(r.next31(), 976015093);
        assert_eq!(r.next31(), 1792756325);
    }

    #[test]
    fn scalar01_is_below_one_and_deterministic() {
        let mut a = EsiRandom::new(65431);
        let mut b = EsiRandom::new(65431);
        for _ in 0..100 {
            let x = a.scalar01();
            assert!((0.0..1.0).contains(&x));
            assert_eq!(x.to_bits(), b.scalar01().to_bits());
        }
    }

    #[test]
    fn search_order_visits_the_containing_octant_first() {
        let bb = TreeBox { min: [0.0; 3], max: [2.0; 3] };
        let o = bb.search_order([1.5, 0.2, 1.9]);
        assert_eq!(o[0], RIGHTHALF | FRONTHALF);
        let mut sorted = o.to_vec();
        sorted.sort();
        assert_eq!(sorted, (0..8).collect::<Vec<u8>>());
    }

    #[test]
    fn segment_and_triangle_box_tests() {
        let bb = TreeBox { min: [0.0; 3], max: [1.0; 3] };
        assert!(bb.intersects_segment([-1.0, 0.5, 0.5], [2.0, 0.5, 0.5]));
        assert!(!bb.intersects_segment([-1.0, 2.0, 0.5], [2.0, 2.0, 0.5]));
        assert!(bb.intersects_triangle([-1.0, -1.0, 0.5], [3.0, -1.0, 0.5], [-1.0, 3.0, 0.5]));
        assert!(!bb.intersects_triangle([2.0, 2.0, 2.0], [3.0, 2.0, 2.0], [2.0, 3.0, 2.0]));
    }


    /// Unit squares in the planes z = k (k = 0..30): a minimal `LineShapes` for `find_line`.
    struct Plates;

    impl OctreeShapes for Plates {
        fn len(&self) -> usize {
            30
        }
        fn overlaps_box(&self, index: usize, bb: &TreeBox) -> bool {
            let z = index as f64;
            bb.min[2] <= z && z <= bb.max[2] && bb.min[0] <= 1.2 && bb.min[1] <= 1.2 && bb.max[0] >= 0.2 && bb.max[1] >= 0.2
        }
    }

    impl LineShapes for Plates {
        fn find_intersection(&self, index: usize, start: V, end: V) -> Option<V> {
            let z = index as f64;
            let dz = end[2] - start[2];
            if dz == 0.0 {
                return None;
            }
            let t = (z - start[2]) / dz;
            if !(0.0..=1.0).contains(&t) {
                return None;
            }
            let p = [start[0] + t * (end[0] - start[0]), start[1] + t * (end[1] - start[1]), z];
            if (0.2..=1.2).contains(&p[0]) && (0.2..=1.2).contains(&p[1]) {
                Some(p)
            } else {
                None
            }
        }
    }

    #[test]
    fn find_line_returns_the_first_plate_along_the_ray_in_either_direction() {
        let bb = TreeBox { min: [-10.0, -10.0, -0.5], max: [10.0, 10.0, 30.5] };
        let tree = Octree::new(Plates, bb, 10, 10, 3.0);
        assert!(tree.n_nodes() > 1);
        let up = tree.find_line([0.7, 0.7, 2.5], [0.7, 0.7, 20.5]).unwrap();
        assert_eq!(up.index, 3);
        assert_eq!(up.point, [0.7, 0.7, 3.0]);
        let down = tree.find_line([0.7, 0.7, 20.5], [0.7, 0.7, 2.5]).unwrap();
        assert_eq!(down.index, 20);
        // misses: beside the plates, and a segment ending before the first plate
        assert!(tree.find_line([5.0, 0.7, 2.5], [5.0, 0.7, 20.5]).is_none());
        assert!(tree.find_line([0.7, 0.7, 2.1], [0.7, 0.7, 2.9]).is_none());
        // start outside the tree box is clipped to it
        let from_outside = tree.find_line([0.7, 0.7, -5.0], [0.7, 0.7, 20.5]).unwrap();
        assert_eq!(from_outside.index, 0);
    }
}
