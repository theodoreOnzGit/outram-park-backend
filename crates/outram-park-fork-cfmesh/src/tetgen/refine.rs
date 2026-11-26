// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright (C) 2026 OUTRAM PARK contributors
//
// Port of TetGen 1.6.0 (`tetgen.cxx` section `refine_cxx`: the facet and
// segment maps `makefacetverticesmap`, `create_segment_info_list`,
// `create_segment_facet_map`, `get_min_diahedral_angle`,
// `get_min_angle_at_ridge_vertex`, `ridge_vertices_adjacent`,
// `facet_ridge_vertex_adjacent`, `segfacetadjacent`, `is_sharp_segment`,
// `does_seg_contain_acute_vertex`, `set_ridge_vertex_protecting_ball`; the
// encroachment tests `check_encroachment`, `check_enc_segment`,
// `get_subface_ccent`, `check_enc_subface`, `check_subface`,
// `checktet4split`; the splits `get_steiner_on_segment`, `split_segment`,
// `parallel_shift`, `locate_on_surface`, `split_subface`,
// `locate_point_walk`, `split_tetrahedron`; the queues `enqueuesubface`,
// `enqueue_subface`, `top_subface`, `dequeue_subface`; the repair loops
// `repairencsegs`, `repairencfacs`, `repairbadtets`; and
// `delaunayrefinement`; with `projpt2edge`, `projpt2face` and the
// three-point `circumsphere` of `geom_cxx`):
//   TetGen — A Quality Tetrahedral Mesh Generator and 3D Delaunay Triangulator
//   https://github.com/libigl/tetgen, commit e05aca7df74e (TetGen 1.6.0)
//   Copyright (C) 2002-2020 Hang Si, Weierstrass Institute for Applied
//   Analysis and Stochastics (WIAS), Berlin.
//   Licence: GNU Affero General Public License v3.0 or later.
//
// This file is part of OUTRAM PARK. It is free software: you can redistribute
// it and/or modify it under the terms of the GNU Affero General Public License
// as published by the Free Software Foundation, either version 3 of the
// License, or (at your option) any later version. It is distributed WITHOUT
// ANY WARRANTY; see the GNU Affero General Public License for details.

//! Delaunay refinement, `-q` and `-a` (stage 6, gh:#1795).
//!
//! `delaunayrefinement` runs after `recoverdelaunay` when `-q` is given
//! (`-a#` implies `-q`). Three passes, each a queue drained by inserting
//! Steiner points with `insertpoint` (Bowyer-Watson, the cavity validated
//! against the boundary, Lawson flips after):
//!
//! 1. **encroached segments** (`repairencsegs`): a segment with a vertex
//!    inside its diametral sphere is split, at its midpoint or, near an
//!    adjacent segment, where the sphere about the shared vertex cuts it
//!    (`get_steiner_on_segment`);
//! 2. **encroached and bad subfaces** (`repairencfacs`): a subface with a
//!    vertex inside its diametral sphere, or whose circumradius to shortest
//!    edge exceeds `-q#`, is split at its circumcentre (or at the
//!    encroaching vertex's projection); a point that would encroach upon a
//!    segment is rejected and the segment split instead;
//! 3. **bad tetrahedra** (`repairbadtets`): a tetrahedron above the
//!    radius-edge bound, below the `-q/#` dihedral bound or above the `-a#`
//!    volume is split at its circumcentre, picked at random from the list
//!    (glibc `rand()`); a circumcentre that encroaches upon a segment or a
//!    subface is rejected and that boundary split instead.
//!
//! Insertion radii (`pointinsradius`) grade the mesh: a point closer to a
//! cavity vertex than that vertex's radius is rejected (`ENCVERTEX`).
//!
//! **Branches left out, and why:** `-r` with an `.elem` list (no `-r`
//! here); `-m` mesh sizes (`rejflag & 4`, the `pointmtrindex` tests of
//! `checktet4split`: without `-m` every size is 0, so these never fire);
//! `-aa` per-region volumes (`varvolume`, `volumebound`); `-S#` Steiner
//! limits and `-r#` element limits (`steinerleft` stays -1, `elem_limit`
//! 0: the branches are kept, unreached); `-c` (`b->convex`);
//! `in->tetunsuitable` (a library callback); the progress printing
//! (`refine_progress_ratio`: printf only); `check_tetrahedron` (its one
//! call is commented out upstream).

#[cfg(not(any(feature = "std", test)))]
#[allow(unused_imports)]
use crate::nostd_prelude::*;

use super::delaunay::{LocateResult, TetgenError};
use super::flip::{sesym, FlipConstraints};
use super::mesh::{
    eprevesymself, eprevself, esymself, enextesymself, enextself, sencode, Face, Point, ShPool,
    TetgenMesh, TriFace, VertType, DUMMYPOINT, EDGE2VER, NULL_POINT, NULL_TET, SNEXTPIVOT,
};
use super::steiner::InsertPointFlags;
use alloc::collections::VecDeque;
use alloc::vec::Vec;
use core::f64::consts::PI;

// ---------------------------------------------------------------------------
// Small vector helpers (tetgen.h inlines).
// ---------------------------------------------------------------------------

#[inline]
fn dot(v1: &[f64; 3], v2: &[f64; 3]) -> f64 {
    v1[0] * v2[0] + v1[1] * v2[1] + v1[2] * v2[2]
}

#[inline]
fn cross(v1: &[f64; 3], v2: &[f64; 3]) -> [f64; 3] {
    [
        v1[1] * v2[2] - v2[1] * v1[2],
        -(v1[0] * v2[2] - v2[0] * v1[2]),
        v1[0] * v2[1] - v2[0] * v1[1],
    ]
}

#[inline]
fn distance(p1: &[f64; 3], p2: &[f64; 3]) -> f64 {
    ((p2[0] - p1[0]) * (p2[0] - p1[0])
        + (p2[1] - p1[1]) * (p2[1] - p1[1])
        + (p2[2] - p1[2]) * (p2[2] - p1[2]))
        .sqrt()
}

#[inline]
fn senext(f: &Face) -> Face {
    Face {
        shver: SNEXTPIVOT[f.shver],
        ..*f
    }
}

#[inline]
fn senext2(f: &Face) -> Face {
    Face {
        shver: SNEXTPIVOT[SNEXTPIVOT[f.shver]],
        ..*f
    }
}

/// `facenormal(pa, pb, pc, n, 1, &lav)`: the normal and the average edge
/// length (`super::surface::facenormal` without `lav`).
fn facenormal_lav(pa: &[f64; 3], pb: &[f64; 3], pc: &[f64; 3]) -> ([f64; 3], f64) {
    let v1 = [pb[0] - pa[0], pb[1] - pa[1], pb[2] - pa[2]]; // a->b
    let v2 = [pa[0] - pc[0], pa[1] - pc[1], pa[2] - pc[2]]; // c->a
    let v3 = [pc[0] - pb[0], pc[1] - pb[1], pc[2] - pb[2]]; // b->c
    let l1 = dot(&v1, &v1);
    let l2 = dot(&v2, &v2);
    let l3 = dot(&v3, &v3);
    let (pv1, pv2);
    if l1 < l2 {
        if l2 < l3 {
            pv1 = v1;
            pv2 = v2;
        } else {
            pv1 = v3;
            pv2 = v1;
        }
    } else if l1 < l3 {
        pv1 = v1;
        pv2 = v2;
    } else {
        pv1 = v2;
        pv2 = v3;
    }
    let lav = (l1.sqrt() + l2.sqrt() + l3.sqrt()) / 3.0;
    let n = cross(&pv1, &pv2);
    ([-n[0], -n[1], -n[2]], lav)
}

/// `projpt2edge(p, e1, e2, prj)`: the projection of `p` on the line
/// `e1 e2`.
pub fn projpt2edge(p: &[f64; 3], e1: &[f64; 3], e2: &[f64; 3]) -> [f64; 3] {
    let mut v1 = [e2[0] - e1[0], e2[1] - e1[1], e2[2] - e1[2]];
    let v2 = [p[0] - e1[0], p[1] - e1[1], p[2] - e1[2]];
    let len = dot(&v1, &v1).sqrt();
    v1[0] /= len;
    v1[1] /= len;
    v1[2] /= len;
    let l_p = dot(&v1, &v2);
    [
        e1[0] + l_p * v1[0],
        e1[1] + l_p * v1[1],
        e1[2] + l_p * v1[2],
    ]
}

/// `projpt2face(p, f1, f2, f3, prj)`: the projection of `p` on the plane
/// `f1 f2 f3`.
pub fn projpt2face(p: &[f64; 3], f1: &[f64; 3], f2: &[f64; 3], f3: &[f64; 3]) -> [f64; 3] {
    let mut fnormal = super::surface::facenormal(f1, f2, f3, 1);
    let len = (fnormal[0] * fnormal[0] + fnormal[1] * fnormal[1] + fnormal[2] * fnormal[2]).sqrt();
    fnormal[0] /= len;
    fnormal[1] /= len;
    fnormal[2] /= len;
    let v1 = [p[0] - f1[0], p[1] - f1[1], p[2] - f1[2]];
    let dist = dot(&fnormal, &v1);
    [
        p[0] - dist * fnormal[0],
        p[1] - dist * fnormal[1],
        p[2] - dist * fnormal[2],
    ]
}

/// `circumsphere(pa, pb, pc, NULL, cent, NULL)`: the centre of the
/// diametral sphere of a triangle; `None` if degenerate.
pub fn circumsphere3(pa: &[f64; 3], pb: &[f64; 3], pc: &[f64; 3]) -> Option<[f64; 3]> {
    let mut a = [[0.0f64; 4]; 4];
    let r0 = [pb[0] - pa[0], pb[1] - pa[1], pb[2] - pa[2]];
    let r1 = [pc[0] - pa[0], pc[1] - pa[1], pc[2] - pa[2]];
    let r2 = cross(&r0, &r1);
    a[0][..3].copy_from_slice(&r0);
    a[1][..3].copy_from_slice(&r1);
    a[2][..3].copy_from_slice(&r2);
    let mut rhs = [0.5 * dot(&r0, &r0), 0.5 * dot(&r1, &r1), 0.0, 0.0];
    let mut indx = [0usize; 4];
    let mut d = 0.0;
    if !super::surface::lu_decmp(&mut a, 3, &mut indx, &mut d) {
        return None;
    }
    super::surface::lu_solve(&a, 3, &indx, &mut rhs);
    Some([pa[0] + rhs[0], pa[1] + rhs[1], pa[2] + rhs[2]])
}

/// `check_encroachment(pa, pb, checkpt)`: does `checkpt` lie inside the
/// diametral sphere of `[pa, pb]` (the angle at it above 90 degrees).
#[inline]
pub fn check_encroachment(pa: &[f64; 3], pb: &[f64; 3], checkpt: &[f64; 3]) -> bool {
    let d = (pa[0] - checkpt[0]) * (pb[0] - checkpt[0])
        + (pa[1] - checkpt[1]) * (pb[1] - checkpt[1])
        + (pa[2] - checkpt[2]) * (pb[2] - checkpt[2]);
    d < 0.0
}

// ---------------------------------------------------------------------------
// The refinement state.
// ---------------------------------------------------------------------------

/// `badface` as Delaunay refinement uses it (the split queues, the
/// unsplit lists, `encseglist` / `encshlist`).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct RefineFace {
    /// `tt`.
    pub tt: TriFace,
    /// `ss`.
    pub ss: Face,
    /// `key`.
    pub key: f64,
    /// `cent[6]`: a circumcentre and the `param` values.
    pub cent: [f64; 6],
    /// `forg`.
    pub forg: Point,
    /// `fdest`.
    pub fdest: Point,
    /// `fapex`.
    pub fapex: Point,
    /// `foppo`.
    pub foppo: Point,
    /// `noppo`: the encroaching vertex.
    pub noppo: Point,
}

impl Default for RefineFace {
    /// `badface::init()`.
    fn default() -> Self {
        RefineFace {
            tt: TriFace::default(),
            ss: Face::default(),
            key: 0.0,
            cent: [0.0; 6],
            forg: NULL_POINT,
            fdest: NULL_POINT,
            fapex: NULL_POINT,
            foppo: NULL_POINT,
            noppo: NULL_POINT,
        }
    }
}

/// `split_subfaces_pool` with `stack_enc_subfaces` and the 64 priority
/// queues (`queuefront[64]`, ...): encroached subfaces on a stack, bad
/// ones in FIFO buckets by radius-edge ratio; `top` is the stack top, else
/// the front of the highest non-empty bucket, as upstream's linked buckets
/// give.
#[derive(Debug, Clone, PartialEq)]
pub struct SubfaceQueue {
    stack: Vec<RefineFace>,
    buckets: Vec<VecDeque<RefineFace>>,
    recentq: i32,
}

impl Default for SubfaceQueue {
    fn default() -> Self {
        SubfaceQueue {
            stack: Vec::new(),
            buckets: (0..64).map(|_| VecDeque::new()).collect(),
            recentq: -1,
        }
    }
}

impl SubfaceQueue {
    /// `split_subfaces_pool->items`.
    pub fn items(&self) -> usize {
        self.stack.len() + self.buckets.iter().map(|b| b.len()).sum::<usize>()
    }
    /// The queue part of `enqueue_subface` (the badface is filled).
    fn enqueue(&mut self, bf: RefineFace) {
        if bf.noppo != NULL_POINT {
            // Push it into the encroaching stack.
            self.stack.push(bf);
        } else {
            // Push it into the priority queue.
            let mut qual = 1.0;
            if bf.cent[4] > 1.0 {
                qual = 1.0 / bf.cent[4]; // 1 / radius_edge_ratio.
            }
            let mut queuenumber = 0usize;
            if qual < 1.0 {
                queuenumber = (64.0 * (1.0 - qual)) as i32 as usize;
                if queuenumber > 63 {
                    queuenumber = 63;
                }
            }
            self.buckets[queuenumber].push_back(bf);
        }
    }
    /// `top_subface()`.
    fn top(&mut self) -> Option<RefineFace> {
        if let Some(bf) = self.stack.last() {
            return Some(*bf);
        }
        self.recentq = -1;
        for q in (0..64).rev() {
            if let Some(f) = self.buckets[q].front() {
                self.recentq = q as i32;
                return Some(*f);
            }
        }
        None
    }
    /// `dequeue_subface()`.
    fn dequeue(&mut self) {
        if self.stack.pop().is_none() && self.recentq >= 0 {
            self.buckets[self.recentq as usize].pop_front();
        }
    }
    /// `restart()` of the pool and `stack_enc_subfaces = NULL` (the
    /// buckets are left as upstream leaves `queuefront[]`).
    fn restart(&mut self) {
        self.stack.clear();
        for b in self.buckets.iter_mut() {
            b.clear();
        }
    }
}

/// What `delaunayrefinement` keeps in `tetgenmesh` members: its pools and
/// queues and the facet and segment maps. Upstream's are `NULL` outside
/// refinement; here they are empty (the maps `None` until built).
#[derive(Debug, Clone, PartialEq, Default)]
pub struct RefineState {
    /// `segment_info_list` (4 values per segment: the smallest dihedral
    /// angle at it, the smallest insertion radius on it, the smallest face
    /// angles at its two endpoints).
    pub segment_info_list: Vec<f64>,
    /// `idx_ridge_vertex_facet_list` (`None` is `NULL`).
    pub idx_ridge_vertex_facet_list: Option<Vec<i32>>,
    /// `ridge_vertex_facet_list`.
    pub ridge_vertex_facet_list: Vec<i32>,
    /// `idx2facetlist`.
    pub idx2facetlist: Vec<i32>,
    /// `facetverticeslist`.
    pub facetverticeslist: Vec<Point>,
    /// `number_of_facets`.
    pub number_of_facets: i32,
    /// `idx_segment_facet_list` (`None` is `NULL`).
    pub idx_segment_facet_list: Option<Vec<i32>>,
    /// `segment_facet_list`.
    pub segment_facet_list: Vec<i32>,
    /// `badsubsegs` (`enqueuesubface`'s pool for segments).
    pub badsubsegs: Vec<Face>,
    /// `badsubfacs`.
    pub badsubfacs: Vec<Face>,
    /// `split_segments_pool` with `stack_enc_segments` (top last).
    pub stack_enc_segments: Vec<RefineFace>,
    /// `split_subfaces_pool`, `stack_enc_subfaces` and the queues.
    pub subfaces: SubfaceQueue,
    /// `check_tets_list`.
    pub check_tets_list: Vec<TriFace>,
    /// `unsplit_subfaces`.
    pub unsplit_subfaces: Vec<RefineFace>,
    /// `unsplit_badtets`.
    pub unsplit_badtets: Vec<RefineFace>,
    /// `encseglist` (`insertpoint`'s `rejflag & 1`).
    pub encseglist: Vec<RefineFace>,
    /// `encshlist` (`insertpoint`'s `rejflag & 2`).
    pub encshlist: Vec<RefineFace>,
    /// `cos_facet_separate_ang_tol`.
    pub cos_facet_separate_ang_tol: f64,
    /// `cosmindihed`.
    pub cosmindihed: f64,
    /// `smallest_insradius`.
    pub smallest_insradius: f64,
    /// `insert_point_count`.
    pub insert_point_count: i64,
    /// `elem_limit` (`-r#`; 0 here).
    pub elem_limit: i64,
}

impl TetgenMesh {
    #[inline]
    fn xr(&self, p: Point) -> [f64; 3] {
        self.points[p].xyz
    }

    /// `sunmarktest2(s)`.
    #[inline]
    pub fn sunmarktest2(&mut self, s: &Face) {
        self.shr_mut(s).flags &= !4;
    }

    /// Is the shell face alive (`s.sh != NULL && s.sh[3] != NULL`).
    #[inline]
    fn sh_alive(&self, s: &Face) -> bool {
        !s.is_null() && self.shr(s).v[0] != NULL_POINT
    }

    // -- the maps -----------------------------------------------------------

    /// `set_ridge_vertex_protecting_ball(ridge_pt)`: give a ridge vertex an
    /// insertion radius (0.95 of its distance to the nearest vertex it
    /// shares a segment with) unless it has one.
    pub fn set_ridge_vertex_protecting_ball(&mut self, ridge_pt: Point) -> f64 {
        let mut rv = self.points[ridge_pt].insradius;
        if rv == 0.0 {
            self.reach6[38] += 1;
            let mut mindist = 1.0e30;
            let idx = self.pointmark(ridge_pt) as usize;
            let lo = self.idx_segment_ridge_vertex_list[idx] as usize;
            let hi = self.idx_segment_ridge_vertex_list[idx + 1] as usize;
            let x = self.xr(ridge_pt);
            for i in lo..hi {
                let dist = distance(&x, &self.xr(self.segment_ridge_vertex_list[i]));
                if mindist > dist {
                    mindist = dist;
                }
            }
            rv = mindist * 0.95;
            self.points[ridge_pt].insradius = rv;
        }
        rv
    }

    /// `get_min_diahedral_angle(seg)`: the smallest interior dihedral angle
    /// (degrees) between the facets at a segment.
    pub fn get_min_diahedral_angle(&self, seg: &Face) -> f64 {
        let pa = self.sorg(seg);
        let pb = self.sdest(seg);
        let mut startsh = self.spivot(seg);
        let adjtet;
        if startsh.is_null() {
            // This segment is not connected by any facet.
            adjtet = self.sstpivot1(seg);
            if adjtet.tet != NULL_TET {
                return 360.0; // This segment is completely inside the volume.
            }
        } else {
            if self.sorg(&startsh) != pa {
                startsh = sesym(&startsh);
            }
            adjtet = self.stpivot(&startsh);
        }
        if adjtet.tet == NULL_TET {
            return 0.0; // This segment is not inserted (recovered) yet.
        }
        let (xa, xb) = (self.xr(pa), self.xr(pb));
        let mut sum_theta = 0.0;
        let mut minang = 2.0 * PI;
        let mut spintet = adjtet;
        loop {
            if !self.ishulltet(spintet.tet) {
                let pc1 = self.apex(&spintet);
                let pc2 = self.oppo(&spintet);
                let n1 = super::surface::facenormal(&xa, &xb, &self.xr(pc1), 1);
                let n2 = super::surface::facenormal(&xa, &xb, &self.xr(pc2), 1);
                let n1len = dot(&n1, &n1).sqrt();
                let n2len = dot(&n2, &n2).sqrt();
                let mut costheta = dot(&n1, &n2) / (n1len * n2len);
                if costheta > 1.0 {
                    costheta = 1.0;
                } else if costheta < -1.0 {
                    costheta = -1.0;
                }
                sum_theta += costheta.acos();
            }
            self.fnextself(&mut spintet);
            let neighsh = self.tspivot(&spintet);
            if !neighsh.is_null() && sum_theta > 0.0 {
                if sum_theta < minang {
                    minang = sum_theta;
                }
                sum_theta = 0.0;
            }
            if spintet.tet == adjtet.tet {
                break;
            }
        }
        minang / PI * 180.0
    }

    /// `get_min_angle_at_ridge_vertex(seg)`: the smallest angle (degrees)
    /// at `sorg(seg)` summed over the triangles of each facet at the
    /// segment.
    pub fn get_min_angle_at_ridge_vertex(&self, seg: &Face) -> f64 {
        let pa = self.sorg(seg);
        let mut startsh = self.spivot(seg);
        if startsh.is_null() {
            return 360.0; // This segment does not belong to any facet.
        } else if self.sorg(&startsh) != pa {
            startsh = sesym(&startsh);
        }
        let xa = self.xr(pa);
        let mut minang = 2.0 * PI;
        let mut spinsh = startsh;
        while !spinsh.is_null() {
            let mut sum_theta = 0.0;
            let mut neighsh = spinsh;
            loop {
                let pb = self.sdest(&neighsh);
                let pc = self.sapex(&neighsh);
                sum_theta += self.interiorangle(&xa, &self.xr(pb), &self.xr(pc), None);
                neighsh = senext2(&neighsh);
                if self.isshsubseg(&neighsh) {
                    break;
                }
                neighsh = self.spivot(&neighsh);
                if self.sorg(&neighsh) != pa {
                    neighsh = sesym(&neighsh);
                }
            }
            if sum_theta < minang {
                minang = sum_theta;
            }
            // Go to the next facet at this segment.
            spinsh = self.spivot(&spinsh);
            if spinsh.same_sh(&startsh) {
                break;
            }
            if spinsh.is_null() {
                break; // A single facet may happen.
            }
            if self.sorg(&spinsh) != pa {
                spinsh = sesym(&spinsh);
            }
        }
        minang / PI * 180.0
    }

    /// `create_segment_info_list()`: the smallest dihedral angle at every
    /// segment and the smallest face angles at its endpoints.
    pub fn create_segment_info_list(&mut self) {
        if self.subsegpool.items == 0 {
            return; // There is no segments.
        }
        let count = (self.segmentendpointslist.len() / 2 + 1) * 4;
        self.rf.segment_info_list = alloc::vec![0.0; count];
        // The loop variable's version is carried from one segment to the
        //   next, as upstream's `segloop.shver`.
        let mut shver = 0usize;
        self.subsegpool.traversalinit();
        while let Some(slot) = self.shellfacetraverse(ShPool::Seg) {
            let mut segloop = Face {
                pool: ShPool::Seg,
                sh: slot,
                shver,
            };
            let segidx = self.getfacetindex(&segloop) as usize;
            if self.rf.segment_info_list[segidx * 4] == 0.0 {
                // Get the smallest dihedral angle at this segment.
                self.rf.segment_info_list[segidx * 4] = self.get_min_diahedral_angle(&segloop);
            }
            for k in 0..2 {
                segloop.shver = 0;
                if self.rf.segment_info_list[segidx * 4 + 2 + k] == 0.0 {
                    let endpt = self.segmentendpointslist[segidx * 2 + k];
                    if self.sorg(&segloop) != endpt {
                        segloop = sesym(&segloop);
                    }
                    if self.sorg(&segloop) == endpt {
                        // Get the min face angle at vertex endpts[k].
                        self.rf.segment_info_list[segidx * 4 + 2 + k] =
                            self.get_min_angle_at_ridge_vertex(&segloop);
                    }
                }
            }
            shver = segloop.shver;
        }
    }

    /// `makefacetverticesmap()`: index every facet (its subfaces get the
    /// facet index) and build the facet-to-ridge-vertex and
    /// ridge-vertex-to-facet maps.
    pub fn makefacetverticesmap(&mut self) {
        let items = self.pointpool.items;
        let mut idx = alloc::vec![0i32; items + 2];
        let mut facetvertexlist: Vec<Vec<Point>> = Vec::new();
        let mut facetindex = 0i32;
        let mut totalvertices = 0usize;

        self.subfacepool.traversalinit();
        while let Some(slot) = self.shellfacetraverse(ShPool::Sub) {
            let subloop = Face {
                pool: ShPool::Sub,
                sh: slot,
                shver: 0,
            };
            if self.sinfected(&subloop) {
                continue;
            }
            // A new facet. Create its vertices list.
            let mut vertlist: Vec<Point> = Vec::new();
            let ppt = self.subfaces[slot].v;
            for p in ppt {
                if self.pointtype(p) == VertType::Ridge {
                    self.pinfect(p);
                    vertlist.push(p);
                    idx[self.pointmark(p) as usize] += 1;
                }
            }
            self.sinfect(&subloop);
            self.caveshlist.push(subloop);
            let mut i = 0;
            while i < self.caveshlist.len() {
                let mut parysh = self.caveshlist[i];
                self.setfacetindex(&parysh, facetindex);
                for _ in 0..3 {
                    if !self.isshsubseg(&parysh) {
                        let neighsh = self.spivot(&parysh);
                        if !self.sinfected(&neighsh) {
                            let pa = self.sapex(&neighsh);
                            if !self.pinfected(pa) && self.pointtype(pa) == VertType::Ridge {
                                self.pinfect(pa);
                                vertlist.push(pa);
                                idx[self.pointmark(pa) as usize] += 1;
                            }
                            self.sinfect(&neighsh);
                            self.caveshlist.push(neighsh);
                        }
                    }
                    parysh = senext(&parysh);
                }
                i += 1;
            }
            totalvertices += vertlist.len();
            for &p in &vertlist {
                self.puninfect(p);
            }
            self.caveshlist.clear();
            facetvertexlist.push(vertlist);
            facetindex += 1;
        }

        // All subfaces are infected. Uninfect them.
        self.subfacepool.traversalinit();
        while let Some(slot) = self.shellfacetraverse(ShPool::Sub) {
            self.subfaces[slot].flags &= !1;
        }

        self.rf.number_of_facets = facetindex;
        let mut idx2facetlist = alloc::vec![0i32; facetindex as usize + 1];
        let mut facetverticeslist = alloc::vec![NULL_POINT; totalvertices];

        // Create ridge_vertex-to-facet map.
        let mut j = idx[0];
        idx[0] = 0;
        let mut i = 0;
        while i < items + 1 {
            let k = idx[i + 1];
            idx[i + 1] = idx[i] + j;
            j = k;
            i += 1;
        }
        let total_count = idx[i] as usize + 1;
        let mut ridge_vertex_facet_list = alloc::vec![0i32; total_count];

        let mut k = 0usize;
        for (fi, vertlist) in facetvertexlist.iter().enumerate() {
            idx2facetlist[fi + 1] = idx2facetlist[fi] + vertlist.len() as i32;
            for &p in vertlist {
                facetverticeslist[k] = p;
                k += 1;
                let ridge_idx = self.pointmark(p) as usize;
                ridge_vertex_facet_list[idx[ridge_idx] as usize] = fi as i32;
                idx[ridge_idx] += 1;
            }
        }

        // Counters in idx_ridge_vertex_facet_list[] are shifted by 1.
        let mut i = items as isize;
        while i >= 0 {
            idx[i as usize + 1] = idx[i as usize];
            i -= 1;
        }
        idx[0] = 0;

        self.rf.idx2facetlist = idx2facetlist;
        self.rf.facetverticeslist = facetverticeslist;
        self.rf.idx_ridge_vertex_facet_list = Some(idx);
        self.rf.ridge_vertex_facet_list = ridge_vertex_facet_list;
    }

    /// `create_segment_facet_map()`: the facets at every segment.
    pub fn create_segment_facet_map(&mut self) {
        let n = self.segmentendpointslist.len() / 2;
        let mut idx = alloc::vec![0i32; n + 1];
        let mut totalcount = 0i32;

        self.subsegpool.traversalinit();
        while let Some(slot) = self.shellfacetraverse(ShPool::Seg) {
            let segloop = Face {
                pool: ShPool::Seg,
                sh: slot,
                shver: 0,
            };
            let segindex = self.getfacetindex(&segloop) as usize;
            if idx[segindex] == 0 {
                // Count the number of facets at this segment.
                let startsh = self.spivot(&segloop);
                let mut spinsh = startsh;
                while !spinsh.is_null() {
                    idx[segindex] += 1;
                    spinsh = self.spivot(&spinsh);
                    if spinsh.same_sh(&startsh) {
                        break;
                    }
                }
                totalcount += idx[segindex];
            }
        }

        // Fill the starting indices into the list.
        let mut bflags = alloc::vec![false; n + 1];
        let mut j = idx[0];
        idx[0] = 0;
        for i in 0..n {
            let k = idx[i + 1];
            idx[i + 1] = idx[i] + j;
            j = k;
            bflags[i] = false;
        }

        let mut segment_facet_list = alloc::vec![0i32; totalcount as usize + 1];
        self.subsegpool.traversalinit();
        while let Some(slot) = self.shellfacetraverse(ShPool::Seg) {
            let segloop = Face {
                pool: ShPool::Seg,
                sh: slot,
                shver: 0,
            };
            let segindex = self.getfacetindex(&segloop) as usize;
            if !bflags[segindex] {
                let startsh = self.spivot(&segloop);
                let mut spinsh = startsh;
                while !spinsh.is_null() {
                    let facetidx = self.getfacetindex(&spinsh);
                    segment_facet_list[idx[segindex] as usize] = facetidx;
                    idx[segindex] += 1;
                    spinsh = self.spivot(&spinsh);
                    if spinsh.same_sh(&startsh) {
                        break;
                    }
                }
                bflags[segindex] = true;
            }
        }

        // Counters in idx_segment_facet_list[] are shifted by 1.
        let mut i = n as isize - 1;
        while i >= 0 {
            idx[i as usize + 1] = idx[i as usize];
            i -= 1;
        }
        idx[0] = 0;
        self.rf.idx_segment_facet_list = Some(idx);
        self.rf.segment_facet_list = segment_facet_list;
    }

    /// `ridge_vertices_adjacent(e1, e2)`: are the two ridge vertices the
    /// endpoints of one input segment.
    pub fn ridge_vertices_adjacent(&self, e1: Point, e2: Point) -> bool {
        let idx = self.pointmark(e1) as usize;
        let lo = self.idx_segment_ridge_vertex_list[idx] as usize;
        let hi = self.idx_segment_ridge_vertex_list[idx + 1] as usize;
        (lo..hi).any(|i| self.segment_ridge_vertex_list[i] == e2)
    }

    /// `facet_ridge_vertex_adjacent(chkfac, chkpt)`: is the ridge vertex a
    /// vertex of the facet. `InternalError` where upstream reads the `NULL`
    /// map (before `makefacetverticesmap`, e.g. boundary recovery).
    pub fn facet_ridge_vertex_adjacent(
        &self,
        chkfac: &Face,
        chkpt: Point,
    ) -> Result<bool, TetgenError> {
        let idx = self
            .rf
            .idx_ridge_vertex_facet_list
            .as_ref()
            .ok_or(TetgenError::InternalError)?;
        let ridge_idx = self.pointmark(chkpt) as usize;
        let facet_idx = self.getfacetindex(chkfac);
        let lo = idx[ridge_idx] as usize;
        let hi = idx[ridge_idx + 1] as usize;
        Ok((lo..hi).any(|i| self.rf.ridge_vertex_facet_list[i] == facet_idx))
    }

    /// `segfacetadjacent(subseg, subsh)`: is the segment on the facet.
    /// `InternalError` where upstream reads the `NULL` map.
    pub fn segfacetadjacent(&self, subseg: &Face, subsh: &Face) -> Result<bool, TetgenError> {
        let idx = self
            .rf
            .idx_segment_facet_list
            .as_ref()
            .ok_or(TetgenError::InternalError)?;
        let seg_idx = self.getfacetindex(subseg) as usize;
        let facet_idx = self.getfacetindex(subsh);
        let lo = idx[seg_idx] as usize;
        let hi = idx[seg_idx + 1] as usize;
        Ok((lo..hi).any(|i| self.rf.segment_facet_list[i] == facet_idx))
    }

    /// `is_sharp_segment(seg)`: the smallest dihedral angle at it is below
    /// 72 degrees.
    pub fn is_sharp_segment(&self, seg: &Face) -> bool {
        let segidx = self.getfacetindex(seg) as usize;
        self.rf.segment_info_list[segidx * 4] < 72.0
    }

    /// `does_seg_contain_acute_vertex(seg)`: an endpoint of the segment it
    /// belongs to, which is also an endpoint of this subsegment, has a face
    /// angle below 60 degrees.
    pub fn does_seg_contain_acute_vertex(&self, seg: &Face) -> bool {
        let segidx = self.getfacetindex(seg) as usize;
        let mut ang = 180.0;
        for i in 0..2 {
            let p = self.segmentendpointslist[segidx * 2 + i];
            if (p == self.sorg(seg) || p == self.sdest(seg))
                && self.rf.segment_info_list[segidx * 4 + 2 + i] < ang
            {
                ang = self.rf.segment_info_list[segidx * 4 + 2 + i];
            }
        }
        ang < 60.0
    }

    // -- queues ---------------------------------------------------------------

    /// `enqueuesubface(pool, chkface)`: queue a subsegment (`badsubsegs`)
    /// or a subface (`badsubfacs`) once for the encroachment check.
    pub fn enqueuesubface(&mut self, chkface: &Face) {
        if !self.smarktest2ed(chkface) {
            self.smarktest2(chkface); // Only queue it once.
            match chkface.pool {
                ShPool::Seg => self.rf.badsubsegs.push(*chkface),
                ShPool::Sub => self.rf.badsubfacs.push(*chkface),
            }
        }
    }

    /// `insertpoint`'s `rejflag & 1` test: every segment of the cavity
    /// (`cavetetseglist`) that `insertpt` encroaches upon goes to
    /// `encseglist`; true if any did.
    pub(super) fn reject_encroached_segments(&mut self, insertpt: Point) -> bool {
        let xi = self.xr(insertpt);
        for i in 0..self.cavetetseglist.len() {
            let seg = self.cavetetseglist[i];
            let v = self.shr(&seg).v;
            if check_encroachment(&self.xr(v[0]), &self.xr(v[1]), &xi) {
                let bf = RefineFace {
                    ss: seg,
                    forg: self.sorg(&seg),
                    fdest: self.sdest(&seg),
                    ..RefineFace::default()
                };
                self.rf.encseglist.push(bf);
            }
        }
        if !self.rf.encseglist.is_empty() {
            self.reach6[33] += 1;
            return true;
        }
        false
    }

    /// `insertpoint`'s `rejflag & 2` test: every subface of the cavity
    /// (`cavetetshlist`) whose diametral sphere holds `insertpt` goes to
    /// `encshlist`; true if any did.
    pub(super) fn reject_encroached_subfaces(
        &mut self,
        insertpt: Point,
    ) -> Result<bool, TetgenError> {
        for i in 0..self.cavetetshlist.len() {
            let parysh = self.cavetetshlist[i];
            let ccent = self.get_subface_ccent(&parysh)?;
            let mut encpt = insertpt;
            let mut radius = 0.0;
            if self.check_enc_subface(&parysh, &mut encpt, &ccent, &mut radius) {
                let mut bface = RefineFace {
                    ss: parysh,
                    forg: self.sorg(&parysh),
                    fdest: self.sdest(&parysh),
                    fapex: self.sapex(&parysh),
                    noppo: NULL_POINT, // no existing encroaching vertex.
                    key: radius,
                    ..RefineFace::default()
                };
                bface.cent[..3].copy_from_slice(&ccent);
                self.rf.encshlist.push(bface);
            }
        }
        if !self.rf.encshlist.is_empty() {
            self.reach6[34] += 1;
            return Ok(true);
        }
        Ok(false)
    }

    /// `insertpoint`'s `chkencflag & 1` and `& 2` queues: the segments
    /// (and the two new subsegments of a split one) and the subfaces (and
    /// the new subfaces) at the cavity.
    pub(super) fn queue_cavity_boundary(&mut self, chkencflag: i32, has_splitseg: bool) {
        if chkencflag & 1 != 0 {
            // Queue all segment outside C(p).
            for i in 0..self.cavetetseglist.len() {
                let s = self.cavetetseglist[i];
                if !self.sinfected(&s) {
                    self.enqueuesubface(&s);
                }
            }
            if has_splitseg {
                // Queue the two new subsegments inside C(p).
                for i in 0..self.cavesegshlist.len() {
                    let s = self.cavesegshlist[i];
                    self.enqueuesubface(&s);
                }
            }
        }
        if chkencflag & 2 != 0 {
            // Queue all subfaces outside C(p).
            for i in 0..self.cavetetshlist.len() {
                let s = self.cavetetshlist[i];
                if !self.sinfected(&s) {
                    self.enqueuesubface(&s);
                }
            }
            // Queue all new subfaces inside C(p).
            for i in 0..self.caveshbdlist.len() {
                let parysh = self.caveshbdlist[i];
                let checksh = self.spivot(&parysh); // a new subface [a, b, p].
                if self.shr(&checksh).v[0] != NULL_POINT {
                    self.enqueuesubface(&checksh);
                }
            }
        }
    }

    // -- segments -------------------------------------------------------------

    /// `check_enc_segment(chkseg, pencpt)`: is the segment encroached by
    /// `*pencpt` (if given), else by an apex of a tetrahedron at it (the
    /// one nearest the segment is returned in `*pencpt`).
    pub fn check_enc_segment(&self, chkseg: &Face, pencpt: &mut Point) -> bool {
        let v = self.shr(chkseg).v;
        let (x0, x1) = (self.xr(v[0]), self.xr(v[1]));
        if *pencpt != NULL_POINT {
            return check_encroachment(&x0, &x1, &self.xr(*pencpt));
        }
        let mut encpt = NULL_POINT;
        let mut minprjdist = 0.0;
        let searchtet = self.sstpivot1(chkseg);
        let mut spintet = searchtet;
        loop {
            let tapex = self.apex(&spintet);
            if tapex != DUMMYPOINT {
                let xt = self.xr(tapex);
                if check_encroachment(&x0, &x1, &xt) {
                    // Find one encroaching vertex. Its projection distance.
                    let prjpt = projpt2edge(&xt, &x0, &x1);
                    let prjdist = distance(&xt, &prjpt);
                    if encpt == NULL_POINT || prjdist < minprjdist {
                        encpt = tapex;
                        minprjdist = prjdist;
                    }
                }
            }
            self.fnextself(&mut spintet);
            if spintet.tet == searchtet.tet {
                break;
            }
        }
        if encpt != NULL_POINT {
            *pencpt = encpt;
            return true;
        }
        false
    }

    /// `get_steiner_on_segment(seg, refpt, steinpt)`: the point that splits
    /// the subsegment: its midpoint, or (encroached by `refpt`) the
    /// projection of `refpt`, or, when `refpt` lies on an adjacent segment
    /// or is an adjacent ridge vertex, where the sphere about the shared
    /// vertex through `refpt` cuts the segment. True if an acute angle was
    /// met.
    pub fn get_steiner_on_segment(&mut self, seg: &Face, refpt: Point, steinpt: Point) -> bool {
        let ei = self.sorg(seg);
        let ej = self.sdest(seg);
        let (xei, xej) = (self.xr(ei), self.xr(ej));
        let mut acute_flag = false;
        let mut sp = [0.0f64; 3];
        let midpoint = |sp: &mut [f64; 3]| {
            for i in 0..3 {
                sp[i] = xei[i] + 0.5 * (xej[i] - xei[i]);
            }
        };
        if refpt != NULL_POINT {
            // This segment is encroched by an existing vertex.
            let xr = self.xr(refpt);
            let rt = self.pointtype(refpt);
            // `adj`: Some((far vertex, near vertex, check ei)) if adjacent.
            let mut adj: Option<(Point, Point, bool)> = None;
            if rt == VertType::FreeSeg {
                let parentseg = super::mesh::sdecode(self.point2sh(refpt));
                let sidx1 = self.getfacetindex(&parentseg) as usize;
                let far_pi = self.segmentendpointslist[sidx1 * 2];
                let far_pj = self.segmentendpointslist[sidx1 * 2 + 1];
                let sidx2 = self.getfacetindex(seg) as usize;
                let far_ei = self.segmentendpointslist[sidx2 * 2];
                let far_ej = self.segmentendpointslist[sidx2 * 2 + 1];
                if far_pi == far_ei || far_pj == far_ei {
                    adj = Some((far_ei, far_ej, false));
                } else if far_pi == far_ej || far_pj == far_ej {
                    adj = Some((far_ej, far_ei, true));
                }
                self.reach6[if adj.is_some() { 5 } else { 7 }] += 1;
            } else if rt == VertType::Ridge {
                let sidx2 = self.getfacetindex(seg) as usize;
                let far_ei = self.segmentendpointslist[sidx2 * 2];
                let far_ej = self.segmentendpointslist[sidx2 * 2 + 1];
                if self.ridge_vertices_adjacent(far_ei, refpt) {
                    adj = Some((far_ei, far_ej, false));
                } else if self.ridge_vertices_adjacent(far_ej, refpt) {
                    adj = Some((far_ej, far_ei, true));
                }
                self.reach6[if adj.is_some() { 6 } else { 7 }] += 1;
            } else {
                self.reach6[7] += 1;
            }
            if let Some((near, far, check_ei)) = adj {
                // Two segments are adjacent at 'near'. Create a Steiner point
                //   at the intersection of the segment [near, far] and the
                //   sphere centered at 'near' with radius |near - refpt|.
                let (xn, xf) = (self.xr(near), self.xr(far));
                let l = distance(&xn, &xf);
                let l1 = distance(&xn, &xr);
                let t = l1 / l;
                for i in 0..3 {
                    sp[i] = xn[i] + t * (xf[i] - xn[i]);
                }
                let lfs_at_steiner = distance(&xr, &sp);
                let d = if check_ei {
                    distance(&sp, &xei)
                } else {
                    distance(&sp, &xej)
                };
                if d < lfs_at_steiner {
                    // Split the point at the middle.
                    midpoint(&mut sp);
                }
                self.set_ridge_vertex_protecting_ball(near);
                acute_flag = true;
            } else {
                // Cut the segment by the projection point of refpt.
                sp = projpt2edge(&xr, &xei, &xej);
                let lfs_at_steiner = distance(&xr, &sp);
                let dist_to_ei = distance(&sp, &xei);
                let dist_to_ej = distance(&sp, &xej);
                if dist_to_ei < lfs_at_steiner || dist_to_ej < lfs_at_steiner {
                    // Split the point at the middle.
                    midpoint(&mut sp);
                }
            }
        } else {
            // Split the point at the middle.
            self.reach6[8] += 1;
            midpoint(&mut sp);
        }
        self.points[steinpt].xyz = sp;
        acute_flag
    }

    /// `split_segment(splitseg, encpt, param, qflag, chkencflag, iloc)`:
    /// split a subsegment by `insertpoint`. A sharp segment (or one with an
    /// acute endpoint) is not split for a rejected circumcentre
    /// (`SHARPCORNER`).
    #[allow(clippy::too_many_arguments)]
    pub fn split_segment(
        &mut self,
        splitseg: &mut Face,
        encpt: Point,
        param: &mut [f64; 6],
        qflag: i32,
        chkencflag: i32,
        iloc: &mut LocateResult,
    ) -> Result<bool, TetgenError> {
        self.reach6[2] += 1;
        self.rf.insert_point_count += 1;
        // Is this segment shared by two facets form an acute dihedral angle?
        let segidx = self.getfacetindex(splitseg) as usize;
        let is_sharp = self.is_sharp_segment(splitseg);

        if qflag == 0 && encpt == NULL_POINT {
            // The split of this segment is due to a rejected ccent of a bad
            //   quality subface or a tetrahedron.
            if is_sharp || self.does_seg_contain_acute_vertex(splitseg) {
                self.reach6[4] += 1;
                *iloc = LocateResult::SharpCorner;
                return Ok(false);
            }
        }

        let newpt = self.makepoint(VertType::FreeSeg);
        self.get_steiner_on_segment(splitseg, encpt, newpt);
        // For create_a_shorter_edge() called in insertpoint().
        self.setpoint2sh(newpt, sencode(splitseg));

        // Split the segment by the Bowyer-Watson algorithm.
        let mut searchtet = self.sstpivot1(splitseg);
        let mut searchsh = Face::default();
        let mut ivf = InsertPointFlags {
            iloc: LocateResult::OnEdge,
            bowywat: 3, // Preserve subsegments and subfaces.
            validflag: true,
            lawson: 2,
            rejflag: 0,
            chkencflag,
            sloc: LocateResult::InStar,
            sbowywat: 3,
            splitbdflag: true,
            respectbdflag: true,
            smlenflag: true, // useinsertradius
            check_insert_radius: qflag == 0,
            parentpt: NULL_POINT,
            ..InsertPointFlags::default()
        };

        if self.insertpoint(
            newpt,
            &mut searchtet,
            Some(&mut searchsh),
            Some(splitseg),
            &mut ivf,
        )? {
            self.reach6[3] += 1;
            self.st_segref_count += 1;
            if self.steinerleft > 0 {
                self.steinerleft -= 1;
            }
            // useinsertradius.
            let mut rv = 0.0;
            if is_sharp {
                // A Steiner point on a sharp segment needs insertion radius.
                let l = ivf.smlen * 0.95;
                rv = if param[3] > l { param[3] } else { l };
                // Record the minimum insertion radius for this segment.
                let mut minradius = self.rf.segment_info_list[segidx * 4 + 1];
                if minradius == 0.0 || rv < minradius {
                    minradius = rv;
                }
                self.rf.segment_info_list[segidx * 4 + 1] = minradius;
            }
            self.points[newpt].insradius = rv;
            self.points[newpt].point2ppt = ivf.parentpt;
            if ivf.smlen < self.rf.smallest_insradius {
                self.rf.smallest_insradius = ivf.smlen;
            }
            self.refine_lawson(chkencflag)?;
            if self.later_unflip_queue.len() as i64 > self.b.unflip_queue_limit {
                self.reach6[39] += 1;
                self.recoverdelaunay()?;
            }
            *iloc = ivf.iloc;
            Ok(true)
        } else {
            // Point is not inserted.
            if ivf.iloc == LocateResult::NearVertex {
                return Err(TetgenError::InternalError); // report a bug.
            }
            self.reach6[44] += 1;
            self.pointdealloc(newpt);
            *iloc = ivf.iloc;
            Ok(false)
        }
    }

    /// The `if (flipstack != NULL) { lawsonflip3d(...) }` after every
    /// refinement insertion (`fc.enqflag = 2`).
    fn refine_lawson(&mut self, chkencflag: i32) -> Result<(), TetgenError> {
        if self.flippool_items() > 0 {
            self.reach6[40] += 1;
            let mut fc = FlipConstraints {
                chkencflag,
                enqflag: 2,
                ..FlipConstraints::default()
            };
            self.lawsonflip3d(&mut fc)?;
        }
        Ok(())
    }

    /// `repairencsegs(param, qflag, chkencflag)`: split the encroached
    /// segments (the queued ones checked first) until none is left.
    pub fn repairencsegs(
        &mut self,
        param: &mut [f64; 6],
        qflag: i32,
        chkencflag: i32,
    ) -> Result<(), TetgenError> {
        self.reach6[1] += 1;
        let ref_segment = (self.b.cdtrefine & 1) > 0;

        while ref_segment
            && (!self.rf.badsubsegs.is_empty() || !self.rf.stack_enc_segments.is_empty())
        {
            if !self.rf.badsubsegs.is_empty() {
                let list = core::mem::take(&mut self.rf.badsubsegs);
                for bface in list {
                    // A queued segment may have been deleted (split), or processed.
                    if self.sh_alive(&bface) && self.smarktest2ed(&bface) {
                        self.sunmarktest2(&bface);
                        let mut encpt = NULL_POINT;
                        if self.check_enc_segment(&bface, &mut encpt) {
                            let bf = RefineFace {
                                ss: bface,
                                forg: self.sorg(&bface),
                                fdest: self.sdest(&bface),
                                noppo: encpt,
                                ..RefineFace::default()
                            };
                            self.rf.stack_enc_segments.push(bf);
                        }
                    }
                }
            }

            if self.rf.stack_enc_segments.is_empty() {
                break;
            }
            // Stop if we have used the desried number of Steiner points.
            if self.steinerleft == 0 {
                break;
            }
            // Stop if the desried number of tetrahedra is reached.
            if self.rf.elem_limit > 0
                && (self.tetpool.items as i64 - self.hullsize) > self.rf.elem_limit
            {
                break;
            }

            // Pop up an encroached segment.
            let mut bf = self.rf.stack_enc_segments.pop().unwrap();
            if !bf.ss.is_null() && self.sorg(&bf.ss) == bf.forg && self.sdest(&bf.ss) == bf.fdest {
                let mut iloc = LocateResult::Unknown;
                self.split_segment(&mut bf.ss, bf.noppo, param, qflag, chkencflag, &mut iloc)?;
            }
        }

        if !self.rf.badsubsegs.is_empty() {
            // Clean this list (due to ref_segment).
            for bface in core::mem::take(&mut self.rf.badsubsegs) {
                if self.sh_alive(&bface) && self.smarktest2ed(&bface) {
                    self.sunmarktest2(&bface);
                }
            }
        }
        if !self.rf.stack_enc_segments.is_empty() {
            self.rf.stack_enc_segments.clear();
        }
        Ok(())
    }

    // -- subfaces -------------------------------------------------------------

    /// `get_subface_ccent(chkfac, pos)`: the centre of the diametral sphere
    /// of a subface (`InternalError` if degenerate, upstream's
    /// `terminatetetgen(2)`).
    pub fn get_subface_ccent(&self, chkfac: &Face) -> Result<[f64; 3], TetgenError> {
        let v = self.shr(chkfac).v;
        circumsphere3(&self.xr(v[0]), &self.xr(v[1]), &self.xr(v[2]))
            .ok_or(TetgenError::InternalError)
    }

    /// `check_enc_subface(chkfac, pencpt, ccent, radius)`: is the subface
    /// encroached by `*pencpt` (if given), else by an opposite vertex of
    /// one of its two tetrahedra whose projection lies in it (the nearest
    /// is returned). A vertex on the same facet (or an adjacent segment or
    /// ridge vertex) pushes the face on the flip stack instead.
    pub fn check_enc_subface(
        &mut self,
        chkfac: &Face,
        pencpt: &mut Point,
        ccent: &[f64; 3],
        radius: &mut f64,
    ) -> bool {
        let rd = distance(ccent, &self.xr(self.sorg(chkfac)));
        *radius = rd;

        if *pencpt != NULL_POINT {
            // This is only used during the insertion of a Steiner point.
            let mut len = distance(ccent, &self.xr(*pencpt));
            if ((len - rd).abs() / rd) < 1e-3 {
                len = rd; // Rounding.
            }
            return len < rd;
        }

        let mut adjtet = self.stpivot(chkfac);
        if adjtet.tet == NULL_TET {
            return false; // This subface is not attached to any tet.
        }
        let mut encpt = NULL_POINT;
        let mut minprjdist = 0.0;
        for _ in 0..2 {
            let toppo = self.oppo(&adjtet);
            if toppo != DUMMYPOINT {
                let xo = self.xr(toppo);
                let mut len = distance(ccent, &xo);
                if ((len - rd).abs() / rd) < 1e-3 {
                    len = rd; // Rounding.
                }
                if len < rd {
                    let mut adjacent = false;
                    let tt = self.pointtype(toppo);
                    // (The maps are built before any call: unwrap_or is
                    //   never taken.)
                    if tt == VertType::Ridge {
                        adjacent = self
                            .facet_ridge_vertex_adjacent(chkfac, toppo)
                            .unwrap_or(false);
                    } else if tt == VertType::FreeSeg {
                        let parentseg = super::mesh::sdecode(self.point2sh(toppo));
                        adjacent = self.segfacetadjacent(&parentseg, chkfac).unwrap_or(false);
                    } else if tt == VertType::FreeFacet {
                        let parentsh = super::mesh::sdecode(self.point2sh(toppo));
                        if self.getfacetindex(&parentsh) == self.getfacetindex(chkfac) {
                            adjacent = true; // They are on the same facet.
                        }
                    }
                    if adjacent {
                        // They are adjacent and they are on the same facet.
                        self.reach6[32] += 1;
                        self.flippush(&adjtet);
                        return false;
                    }
                    let (xa, xb, xc) = (
                        self.xr(self.org(&adjtet)),
                        self.xr(self.dest(&adjtet)),
                        self.xr(self.apex(&adjtet)),
                    );
                    let prjpt = projpt2face(&xo, &xa, &xb, &xc);
                    if self.pred.orient3d(&xa, &xb, &xo, &prjpt) >= 0.0
                        && self.pred.orient3d(&xb, &xc, &xo, &prjpt) >= 0.0
                        && self.pred.orient3d(&xc, &xa, &xo, &prjpt) >= 0.0
                    {
                        let prjdist = distance(&xo, &prjpt);
                        if encpt == NULL_POINT || prjdist < minprjdist {
                            encpt = toppo;
                            minprjdist = prjdist;
                        }
                    }
                }
            }
            self.fsymself(&mut adjtet);
        }

        if encpt != NULL_POINT {
            *pencpt = encpt;
            return true;
        }
        false
    }

    /// `check_subface(chkfac, ccent, radius, param)`: is the subface's
    /// radius-edge ratio above `-q#`. `chkfac` is turned to its shortest
    /// edge; `param[3..6]` get the insertion radius and the ratio.
    pub fn check_subface(&self, chkfac: &mut Face, radius: f64, param: &mut [f64; 6]) -> bool {
        // Get the shortest edge length.
        let mut emin = 1.0e30;
        let mut shver = 0;
        chkfac.shver = 0;
        while chkfac.shver < 3 {
            let dist = distance(&self.xr(self.sorg(chkfac)), &self.xr(self.sdest(chkfac)));
            if dist < emin {
                emin = dist;
                shver = chkfac.shver;
            }
            chkfac.shver += 1;
        }
        chkfac.shver = shver;

        let ratio = radius / emin;
        if ratio > self.b.minratio {
            // Enlarge the insertion radius (due to small angle).
            let ra = self.points[self.sorg(chkfac)].insradius;
            let rb = self.points[self.sdest(chkfac)].insradius;
            if ra > 0.0 && ra > emin {
                emin = ra;
            }
            if rb > 0.0 && rb > emin {
                emin = rb;
            }
            param[3] = emin;
            param[4] = ratio;
            param[5] = 0.0; // not used.
            return true; // need to split it.
        }
        false
    }

    /// `enqueue_subface(bface, encpt, ccent, param)`.
    pub fn enqueue_subface(
        &mut self,
        bface: &Face,
        encpt: Point,
        ccent: &[f64; 3],
        param: &[f64; 6],
    ) {
        let mut bf = RefineFace {
            ss: *bface,
            forg: self.sorg(bface),
            fdest: self.sdest(bface),
            fapex: self.sapex(bface),
            noppo: encpt,
            ..RefineFace::default()
        };
        bf.cent[..3].copy_from_slice(ccent);
        bf.cent[3..].copy_from_slice(&param[3..]);
        self.rf.subfaces.enqueue(bf);
    }

    /// `parallel_shift(pa, pb, pc, pt, ppt)`: `pt` moved along the normal
    /// of `[pa, pb, pc]` by its average edge length.
    fn parallel_shift(&self, pa: Point, pb: Point, pc: Point, pt: Point) -> [f64; 3] {
        let (mut n, lav) = facenormal_lav(&self.xr(pa), &self.xr(pb), &self.xr(pc));
        let l = (n[0] * n[0] + n[1] * n[1] + n[2] * n[2]).sqrt();
        n[0] /= l;
        n[1] /= l;
        n[2] /= l;
        let x = self.xr(pt);
        [x[0] + lav * n[0], x[1] + lav * n[1], x[2] + lav * n[2]]
    }

    /// `locate_on_surface(searchpt, searchsh)`: walk across the facet from
    /// `searchsh` to the subface holding `searchpt` (`ONFACE`, `ONEDGE`,
    /// `ONVERTEX`), stopping at a segment (`ENCSEGMENT`) or where the
    /// surface bends away (`NONCOPLANAR`).
    pub fn locate_on_surface(
        &mut self,
        searchpt: Point,
        searchsh: &mut Face,
    ) -> Result<LocateResult, TetgenError> {
        let loc;
        let mut searchtet = self.stpivot(searchsh);
        if self.ishulltet(searchtet.tet) {
            *searchsh = sesym(searchsh);
            searchtet = self.stpivot(searchsh);
        }
        let xs = self.xr(searchpt);

        // Select an edge such that pt lies to CCW of it.
        let mut i = 0;
        let (mut pa, mut pb, mut pc) = (NULL_POINT, NULL_POINT, NULL_POINT);
        while i < 3 {
            pa = self.org(&searchtet);
            pb = self.dest(&searchtet);
            pc = self.apex(&searchtet);
            let toppo = self.parallel_shift(pa, pb, pc, pa);
            if self.pred.orient3d(&self.xr(pa), &self.xr(pb), &toppo, &xs) > 0.0 {
                break;
            }
            enextself(&mut searchtet);
            i += 1;
        }
        if i == 3 {
            return Err(TetgenError::InternalError);
        }

        loop {
            // Make sure that the searching vertex and the current subface
            //   (a,b,c) are (nearly) coplanar.
            let (xa, xb, xc) = (self.xr(pa), self.xr(pb), self.xr(pc));
            let n1 = super::surface::facenormal(&xa, &xb, &xc, 1);
            let n2 = super::surface::facenormal(&xb, &xa, &xs, 1);
            let cosang = dot(&n1, &n2) / (dot(&n1, &n1).sqrt() * dot(&n2, &n2).sqrt());
            if cosang > self.rf.cos_facet_separate_ang_tol {
                // The searching vertex is not coplanar with this subface.
                loc = LocateResult::NonCoplanar;
                break;
            }

            let toppo = self.parallel_shift(pa, pb, pc, pc);
            let ori1 = self.pred.orient3d(&xb, &xc, &toppo, &xs);
            let ori2 = self.pred.orient3d(&xc, &xa, &toppo, &xs);

            if ori1 > 0.0 {
                if ori2 > 0.0 {
                    loc = LocateResult::OnFace;
                    break;
                } else if ori2 < 0.0 {
                    eprevself(&mut searchtet);
                } else {
                    eprevself(&mut searchtet);
                    loc = LocateResult::OnEdge;
                    break;
                }
            } else if ori1 < 0.0 {
                if ori2 > 0.0 {
                    enextself(&mut searchtet);
                } else if ori2 < 0.0 {
                    // Randomly choose one.
                    self.reach6[42] += 1;
                    if self.crand.rand() % 2 != 0 {
                        enextself(&mut searchtet);
                    } else {
                        eprevself(&mut searchtet);
                    }
                } else {
                    enextself(&mut searchtet);
                }
            } else if ori2 > 0.0 {
                enextself(&mut searchtet); // p lies on edge [b,c].
                loc = LocateResult::OnEdge;
                break;
            } else if ori2 < 0.0 {
                eprevself(&mut searchtet);
            } else {
                eprevself(&mut searchtet); // p is coincident with apex.
                loc = LocateResult::OnVertex;
                break;
            }

            // Check if we want to cross a segment.
            if self.issubseg(&searchtet) {
                loc = LocateResult::EncSegment;
                break;
            }

            // Goto the adjacent subface at this subedge.
            let mut fcount = 0;
            while fcount < 100000 {
                esymself(&mut searchtet);
                if self.issubface(&searchtet) {
                    break;
                }
                self.fsymself(&mut searchtet);
                fcount += 1;
            }
            if !self.issubface(&searchtet) {
                return Err(TetgenError::InternalError); // report a bug
            }

            // Update the vertices.
            pa = self.org(&searchtet);
            pb = self.dest(&searchtet);
            pc = self.apex(&searchtet);
        }

        *searchsh = self.tspivot(&searchtet);
        Ok(loc)
    }

    /// `split_subface(splitfac, encpt, ccent, param, qflag, chkencflag,
    /// iloc)`: split a subface at its circumcentre (or at the projection of
    /// the encroaching vertex `encpt`). A point that would encroach upon a
    /// segment is not inserted; the segment is split instead. True if the
    /// subface was split.
    #[allow(clippy::too_many_arguments)]
    pub fn split_subface(
        &mut self,
        splitfac: &Face,
        encpt: Point,
        ccent: &[f64; 3],
        param: &mut [f64; 6],
        qflag: i32,
        chkencflag: i32,
        iloc: &mut LocateResult,
    ) -> Result<bool, TetgenError> {
        self.reach6[10] += 1;
        self.rf.insert_point_count += 1;
        let mut splitflag = false;

        // Is this subface adjacent to a sharp segment?
        let mut is_adjacent = false;
        let mut checkface = *splitfac;
        for _ in 0..3 {
            let checkseg = self.sspivot(&checkface);
            if !checkseg.is_null() && self.is_sharp_segment(&checkseg) {
                is_adjacent = true;
                break;
            }
            checkface = senext2(&checkface);
        }

        if is_adjacent && qflag == 0 && (encpt != NULL_POINT || param[4] == 0.0) {
            // Only split it either it is a bad quality triangle, or due to
            //   the qflag, i.e., mesh size requirement.
            self.reach6[12] += 1;
            *iloc = LocateResult::SharpCorner;
            return Ok(false); // reject splitting this subface.
        }

        // Deciding the inserting point.
        let newpt;
        if encpt != NULL_POINT {
            // Insert at the projection of the encpt on the facet.
            self.reach6[13] += 1;
            let ppt = self.shr(splitfac).v;
            let xe = self.xr(encpt);
            let pos = projpt2face(&xe, &self.xr(ppt[0]), &self.xr(ppt[1]), &self.xr(ppt[2]));
            newpt = self.makepoint(VertType::FreeFacet);
            self.points[newpt].xyz = pos;
            // Check whether this new position is too close to an existing vertex.
            let prjdist = distance(&xe, &pos);
            let mut mindist = 1.0e30;
            for p in ppt {
                let dist = distance(&self.xr(p), &pos);
                if dist < mindist {
                    mindist = dist;
                }
            }
            if mindist < prjdist {
                // Use the circumcenter of this triange instead.
                self.reach6[14] += 1;
                self.points[newpt].xyz = *ccent;
            }
        } else {
            // Split the subface at its circumcenter.
            newpt = self.makepoint(VertType::FreeFacet);
            self.points[newpt].xyz = *ccent;
        }

        // This info is needed by create_a_shorter_edge().
        self.setpoint2sh(newpt, sencode(splitfac));

        let mut searchsh = *splitfac;
        let sloc = self.locate_on_surface(newpt, &mut searchsh)?;

        if sloc == LocateResult::EncSegment {
            // Point lies in the outside of the facet.
            self.reach6[15] += 1;
            self.pointdealloc(newpt);
            *iloc = LocateResult::FensedIn;
            return Ok(splitflag);
        } else if sloc == LocateResult::OnVertex || sloc == LocateResult::NonCoplanar {
            self.reach6[16] += 1;
            self.pointdealloc(newpt);
            *iloc = sloc;
            return Ok(splitflag);
        }
        if sloc != LocateResult::OnFace && sloc != LocateResult::OnEdge {
            return Err(TetgenError::InternalError); // report a bug
        }

        // Insert the point.
        let mut searchtet = self.stpivot(&searchsh);
        let mut ivf = InsertPointFlags {
            iloc: sloc,
            bowywat: 3,
            lawson: 2,
            rejflag: 1, // Do check the encroachment of segments.
            chkencflag: chkencflag & !1,
            sloc: LocateResult::InStar,
            sbowywat: 3,
            splitbdflag: true,
            validflag: true,
            respectbdflag: true,
            refineflag: 2,
            refinesh: *splitfac,
            smlenflag: true, // useinsertradius
            check_insert_radius: qflag == 0,
            parentpt: NULL_POINT,
            ..InsertPointFlags::default()
        };

        if self.insertpoint(newpt, &mut searchtet, Some(&mut searchsh), None, &mut ivf)? {
            self.reach6[11] += 1;
            self.st_facref_count += 1;
            if self.steinerleft > 0 {
                self.steinerleft -= 1;
            }
            // useinsertradius.
            let mut rv = 0.0;
            if is_adjacent {
                // A sharp (dihedral) angle is involved.
                let l = ivf.smlen / 3.0;
                rv = if param[3] > l { param[3] } else { l };
            }
            self.points[newpt].insradius = rv;
            self.points[newpt].point2ppt = ivf.parentpt;
            if self.rf.smallest_insradius > ivf.smlen {
                self.rf.smallest_insradius = ivf.smlen;
            }
            self.refine_lawson(chkencflag & !1)?;
            if self.later_unflip_queue.len() as i64 > self.b.unflip_queue_limit {
                self.reach6[39] += 1;
                self.recoverdelaunay()?;
            }
            *iloc = ivf.iloc;
            return Ok(true);
        }

        // Point is not inserted.
        self.pointdealloc(newpt);

        if ivf.iloc == LocateResult::EncSegment {
            self.reach6[17] += 1;
            // Bakup the split subface.
            let bak_pts = self.shr(splitfac).v;
            let ref_segment = (self.b.cdtrefine & 1) > 0;
            if ref_segment || qflag != 0 {
                // Select an encroached segment and split it.
                let mut i = 0;
                while i < self.rf.encseglist.len() {
                    let mut bf = self.rf.encseglist[i];
                    i += 1;
                    if bf.ss.is_null()
                        || self.sorg(&bf.ss) != bf.forg
                        || self.sdest(&bf.ss) != bf.fdest
                    {
                        continue; // Skip this segment.
                    }
                    let mut tmp_iloc = LocateResult::Unknown;
                    let ok = self.split_segment(
                        &mut bf.ss,
                        NULL_POINT,
                        param,
                        qflag,
                        chkencflag | 1,
                        &mut tmp_iloc,
                    )?;
                    self.rf.encseglist[i - 1] = bf;
                    if ok {
                        // Check if this subface is split as well.
                        if self.shr(splitfac).v != bak_pts {
                            splitflag = true;
                            break;
                        }
                    }
                }
            }
            self.rf.encseglist.clear();
            // Some segments may be encroached.
            if !self.rf.badsubsegs.is_empty() {
                self.repairencsegs(param, 0, chkencflag | 1)?; // qflag = 0
            }
            // Check if this subface is split as well.
            if self.shr(splitfac).v != bak_pts {
                splitflag = true;
            }
        } else if ivf.iloc == LocateResult::NearVertex {
            return Err(TetgenError::InternalError); // report a bug
        } else {
            self.reach6[43] += 1;
        }

        *iloc = ivf.iloc;
        Ok(splitflag)
    }

    /// `repairencfacs(param, qflag, chkencflag)`: split the encroached and
    /// the bad subfaces (the queued ones checked first) until none is left.
    pub fn repairencfacs(
        &mut self,
        param: &mut [f64; 6],
        qflag: i32,
        chkencflag: i32,
    ) -> Result<(), TetgenError> {
        self.reach6[9] += 1;
        let ref_subface = (self.b.cdtrefine & 2) > 0;
        // The insertion radius of a rejected circumcenter (split_tetrahedron).
        let tet_emin = param[3];

        while ref_subface && (!self.rf.badsubfacs.is_empty() || self.rf.subfaces.items() > 0) {
            if !self.rf.badsubfacs.is_empty() {
                for mut bface in core::mem::take(&mut self.rf.badsubfacs) {
                    // A queued subface may have been deleted (split), or processed.
                    if self.sh_alive(&bface) && self.smarktest2ed(&bface) {
                        self.sunmarktest2(&bface);
                        for p in param.iter_mut().skip(3) {
                            *p = 0.0; // Clear previous values.
                        }
                        let ccent = self.get_subface_ccent(&bface)?;
                        let mut encpt = NULL_POINT;
                        let mut radius = 0.0;
                        if self.check_enc_subface(&bface, &mut encpt, &ccent, &mut radius) {
                            param[3] = tet_emin; // maybe zero.
                            self.enqueue_subface(&bface, encpt, &ccent, param);
                        } else if self.check_subface(&mut bface, radius, param) {
                            if tet_emin > 0.0 {
                                // Use the larger one.
                                param[3] = if param[3] > tet_emin {
                                    param[3]
                                } else {
                                    tet_emin
                                };
                            }
                            self.enqueue_subface(&bface, NULL_POINT, &ccent, param);
                        }
                    }
                }
                // check_enc_subface() may find some non-Delaunay subfaces.
                self.refine_lawson(chkencflag)?;
            }

            if self.rf.subfaces.items() == 0 {
                break;
            }
            if self.steinerleft == 0 {
                break;
            }
            if self.rf.elem_limit > 0
                && (self.tetpool.items as i64 - self.hullsize) > self.rf.elem_limit
            {
                break;
            }

            let bf = self.rf.subfaces.top().unwrap();
            if !bf.ss.is_null()
                && self.sorg(&bf.ss) == bf.forg
                && self.sdest(&bf.ss) == bf.fdest
                && self.sapex(&bf.ss) == bf.fapex
            {
                // Try to split this subface.
                let encpt = bf.noppo; // The encroaching vertex.
                let ccent = [bf.cent[0], bf.cent[1], bf.cent[2]];
                param[3..].copy_from_slice(&bf.cent[3..]);
                let mut iloc = LocateResult::Unknown;
                if !self
                    .split_subface(&bf.ss, encpt, &ccent, param, qflag, chkencflag, &mut iloc)?
                    && (qflag != 0
                        || (param[4] > 3.0 * self.b.minratio && iloc != LocateResult::SharpCorner))
                {
                    // Queue a unsplit (bad quality) subface.
                    self.reach6[18] += 1;
                    self.rf.unsplit_subfaces.push(bf);
                }
            }
            self.rf.subfaces.dequeue();
        }

        param[3] = tet_emin; // Restore this value.

        if !self.rf.badsubfacs.is_empty() {
            // Clean this list (due to the ref_subface flag).
            for bface in core::mem::take(&mut self.rf.badsubfacs) {
                if self.sh_alive(&bface) && self.smarktest2ed(&bface) {
                    self.sunmarktest2(&bface);
                }
            }
        }
        if self.rf.subfaces.items() > 0 {
            self.rf.subfaces.restart();
            self.rf.unsplit_subfaces.clear();
        }
        Ok(())
    }

    // -- tetrahedra -----------------------------------------------------------

    /// `checktet4split(chktet, param, qflag)`: does the tetrahedron need a
    /// split: above the `-a#` volume (`qflag = 1`), above the radius-edge
    /// bound, or below the dihedral bound. `param[0..3]` get its
    /// circumcentre, `param[3..6]` the shortest edge, the ratio and the
    /// edge ratio; `chktet` is turned to its shortest edge.
    pub fn checktet4split(
        &mut self,
        chktet: &mut TriFace,
        param: &mut [f64; 6],
        qflag: &mut i32,
    ) -> Result<bool, TetgenError> {
        *qflag = 0;
        for p in param.iter_mut() {
            *p = 0.0;
        }
        let v = self.tets[chktet.tet].v;
        if v[3] == DUMMYPOINT {
            return Ok(false); // Do not split a hull tet.
        }
        let (pa, pb, pc, pd) = (self.xr(v[0]), self.xr(v[1]), self.xr(v[2]), self.xr(v[3]));

        let mut a = [[0.0f64; 4]; 4];
        let mut vda = [0.0; 3];
        let mut vdb = [0.0; 3];
        let mut vdc = [0.0; 3];
        for i in 0..3 {
            vda[i] = pa[i] - pd[i];
            a[0][i] = vda[i];
        }
        for i in 0..3 {
            vdb[i] = pb[i] - pd[i];
            a[1][i] = vdb[i];
        }
        for i in 0..3 {
            vdc[i] = pc[i] - pd[i];
            a[2][i] = vdc[i];
        }
        let vab = [pb[0] - pa[0], pb[1] - pa[1], pb[2] - pa[2]];
        let vbc = [pc[0] - pb[0], pc[1] - pb[1], pc[2] - pb[2]];
        let vca = [pa[0] - pc[0], pa[1] - pc[1], pa[2] - pc[2]];

        let mut indx = [0usize; 4];
        let mut d = 0.0;
        if !super::surface::lu_decmp(&mut a, 3, &mut indx, &mut d) {
            // Is it a degenerated tet (vol = 0).
            let d = self.pred.orient3dexact(&pa, &pb, &pc, &pd);
            if d >= 0.0 {
                return Err(TetgenError::InternalError); // A degenerated tetrahedron.
            }
            // We temporarily leave this tet. It should be fixed by mesh improvement.
            return Ok(false);
        }

        // Calculate the circumcenter and radius of this tet.
        let mut rhs = [
            0.5 * dot(&vda, &vda),
            0.5 * dot(&vdb, &vdb),
            0.5 * dot(&vdc, &vdc),
            0.0,
        ];
        super::surface::lu_solve(&a, 3, &indx, &mut rhs);
        for i in 0..3 {
            param[i] = pd[i] + rhs[i];
        }
        let r3 = [rhs[0], rhs[1], rhs[2]];
        let rd = dot(&r3, &r3).sqrt();

        // Check volume if '-a#' is used.
        if self.b.fixedvolume {
            let vol = (a[indx[0]][0] * a[indx[1]][1] * a[indx[2]][2]).abs() / 6.0;
            if vol > self.b.maxvolume {
                *qflag = 1;
            }
            if *qflag == 1 {
                self.reach6[21] += 1;
                return Ok(true);
            }
        }

        // Check the radius-edge ratio. Set by -q#.
        if self.b.minratio > 0.0 {
            let elen = [
                dot(&vdc, &vdc),
                dot(&vda, &vda),
                dot(&vab, &vab),
                dot(&vbc, &vbc),
                dot(&vdb, &vdb),
                dot(&vca, &vca),
            ];
            let mut lmax = elen[0];
            let mut lmin = elen[0];
            let mut eidx = 0;
            for (i, &e) in elen.iter().enumerate().skip(1) {
                lmax = if lmax < e { e } else { lmax };
                if lmin > e {
                    lmin = e;
                    eidx = i;
                }
            }
            // Let chktet be the shortest edge in this tet.
            chktet.ver = EDGE2VER[eidx];
            lmin = lmin.sqrt();
            let dd = rd / lmin;
            if dd > self.b.minratio {
                // A bad radius-edge ratio.
                param[3] = lmin;
                param[4] = dd;
                param[5] = lmax.sqrt() / lmin; // edge ratio.
                self.reach6[22] += 1;
                return Ok(true);
            }
        }

        // Check the minimum dihedral angle. Set by -q/#.
        if self.b.mindihedral > 0.0 {
            let mut n = [[0.0f64; 3]; 4];
            for j in 0..3 {
                let mut b4 = [0.0f64; 4];
                b4[j] = 1.0; // Positive means the inside direction
                super::surface::lu_solve(&a, 3, &indx, &mut b4);
                n[j] = [b4[0], b4[1], b4[2]];
            }
            for i in 0..3 {
                n[3][i] = -n[0][i] - n[1][i] - n[2][i];
            }
            // Normalize the normals.
            for row in n.iter_mut() {
                let l = dot(row, row).sqrt();
                if l == 0.0 {
                    return Err(TetgenError::InternalError);
                }
                for x in row.iter_mut() {
                    *x /= l;
                }
            }
            let cosd = [
                -dot(&n[0], &n[1]),
                -dot(&n[0], &n[2]),
                -dot(&n[0], &n[3]),
                -dot(&n[1], &n[2]),
                -dot(&n[1], &n[3]),
                -dot(&n[2], &n[3]),
            ];
            let mut maxcosd = cosd[0];
            for &c in cosd.iter().skip(1) {
                maxcosd = if c > maxcosd { c } else { maxcosd };
            }
            if maxcosd > self.rf.cosmindihed {
                // A bad dihedral angle.
                self.reach6[23] += 1;
                return Ok(true);
            }
        }

        Ok(false)
    }

    /// `tri_edge_test(a, b, c, p, q, NULL, 0, NULL, NULL)` with `p` given
    /// by coordinates (`locate_point_walk`'s starting point).
    fn tri_edge_test_xyz(&mut self, a: Point, b: Point, c: Point, p: &[f64; 3], q: Point) -> bool {
        let (xa, xb, xc, xq) = (self.xr(a), self.xr(b), self.xr(c), self.xr(q));
        let s_p = self.pred.orient3d(&xa, &xb, &xc, p);
        let s_q = self.pred.orient3d(&xa, &xb, &xc, &xq);
        let mut types = [0i32; 2];
        let mut pos = [0i32; 4];
        self.tri_edge_tail(
            &xa, &xb, &xc, p, &xq, None, s_p, s_q, 0, &mut types, &mut pos,
        ) != 0
    }

    /// `locate_point_walk(searchpt, searchtet, chkencflag)`: walk along the
    /// line from the barycentre of `searchtet` to `searchpt`, stopping at a
    /// subface (`ENCSUBFACE`, with `chkencflag`) or the hull (`OUTSIDE`).
    pub fn locate_point_walk(
        &mut self,
        searchpt: Point,
        searchtet: &mut TriFace,
        chkencflag: bool,
    ) -> Result<LocateResult, TetgenError> {
        // Construct the starting point to be the barycenter of 'searchtet'.
        let v = self.tets[searchtet.tet].v;
        let (x0, x1, x2, x3) = (self.xr(v[0]), self.xr(v[1]), self.xr(v[2]), self.xr(v[3]));
        let mut startpt = [0.0; 3];
        for i in 0..3 {
            startpt[i] = (x0[i] + x1[i] + x2[i] + x3[i]) / 4.0;
        }
        let xs = self.xr(searchpt);

        let mut loc = LocateResult::Outside;
        let (mut torg, mut tdest, mut tapex) = (NULL_POINT, NULL_POINT, NULL_POINT);
        searchtet.ver = 0;
        while searchtet.ver < 4 {
            torg = self.org(searchtet);
            tdest = self.dest(searchtet);
            tapex = self.apex(searchtet);
            let ori = self
                .pred
                .orient3d(&self.xr(torg), &self.xr(tdest), &self.xr(tapex), &xs);
            if ori < 0.0 {
                break;
            }
            searchtet.ver += 1;
        }
        if searchtet.ver == 4 {
            return Err(TetgenError::InternalError);
        }

        // 0: ORGMOVE, 1: DESTMOVE, 2: APEXMOVE.
        let mut max_visited_tets = 10000;
        while max_visited_tets > 0 {
            let toppo = self.oppo(searchtet);
            // Check if the vertex is we seek.
            if toppo == searchpt {
                esymself(searchtet);
                eprevself(searchtet);
                loc = LocateResult::OnVertex;
                break;
            }
            let (xo, xd, xa, xt) = (
                self.xr(torg),
                self.xr(tdest),
                self.xr(tapex),
                self.xr(toppo),
            );
            let oriorg = self.pred.orient3d(&xd, &xa, &xt, &xs);
            let oridest = self.pred.orient3d(&xa, &xo, &xt, &xs);
            let oriapex = self.pred.orient3d(&xo, &xd, &xt, &xs);

            let nextmove;
            if oriorg < 0.0 {
                if oridest < 0.0 {
                    if oriapex < 0.0 {
                        // All three faces are possible.
                        if self.tri_edge_test_xyz(tdest, tapex, toppo, &startpt, searchpt) {
                            nextmove = 0;
                        } else if self.tri_edge_test_xyz(tapex, torg, toppo, &startpt, searchpt) {
                            nextmove = 1;
                        } else if self.tri_edge_test_xyz(torg, tdest, toppo, &startpt, searchpt) {
                            nextmove = 2;
                        } else {
                            self.reach6[41] += 1;
                            nextmove = self.randomnation(3) as i32;
                        }
                    } else if self.tri_edge_test_xyz(tdest, tapex, toppo, &startpt, searchpt) {
                        nextmove = 0;
                    } else if self.tri_edge_test_xyz(tapex, torg, toppo, &startpt, searchpt) {
                        nextmove = 1;
                    } else {
                        self.reach6[41] += 1;
                        nextmove = if self.randomnation(2) != 0 { 0 } else { 1 };
                    }
                } else if oriapex < 0.0 {
                    // Two faces, opposite to origin and apex, are viable.
                    if self.tri_edge_test_xyz(tdest, tapex, toppo, &startpt, searchpt) {
                        nextmove = 0;
                    } else if self.tri_edge_test_xyz(torg, tdest, toppo, &startpt, searchpt) {
                        nextmove = 2;
                    } else {
                        self.reach6[41] += 1;
                        nextmove = if self.randomnation(2) != 0 { 0 } else { 2 };
                    }
                } else {
                    nextmove = 0;
                }
            } else if oridest < 0.0 {
                if oriapex < 0.0 {
                    // Two faces, opposite to destination and apex, are viable.
                    if self.tri_edge_test_xyz(tapex, torg, toppo, &startpt, searchpt) {
                        nextmove = 1;
                    } else if self.tri_edge_test_xyz(torg, tdest, toppo, &startpt, searchpt) {
                        nextmove = 2;
                    } else {
                        self.reach6[41] += 1;
                        nextmove = if self.randomnation(2) != 0 { 1 } else { 2 };
                    }
                } else {
                    nextmove = 1;
                }
            } else if oriapex < 0.0 {
                nextmove = 2;
            } else {
                // The point we seek must be on the boundary of or inside this
                //   tetrahedron. Check for boundary cases.
                if oriorg == 0.0 {
                    enextesymself(searchtet);
                    if oridest == 0.0 {
                        eprevself(searchtet); // edge oppo->apex
                        if oriapex == 0.0 {
                            loc = LocateResult::OnVertex; // oppo is duplicated with p.
                            break;
                        }
                        loc = LocateResult::OnEdge;
                        break;
                    }
                    if oriapex == 0.0 {
                        enextself(searchtet); // edge dest->oppo
                        loc = LocateResult::OnEdge;
                        break;
                    }
                    loc = LocateResult::OnFace;
                    break;
                }
                if oridest == 0.0 {
                    eprevesymself(searchtet);
                    if oriapex == 0.0 {
                        eprevself(searchtet); // edge oppo->org
                        loc = LocateResult::OnEdge;
                        break;
                    }
                    loc = LocateResult::OnFace;
                    break;
                }
                if oriapex == 0.0 {
                    esymself(searchtet);
                    loc = LocateResult::OnFace;
                    break;
                }
                loc = LocateResult::InTetrahedron;
                break;
            }

            // Move to the selected face.
            if nextmove == 0 {
                enextesymself(searchtet);
            } else if nextmove == 1 {
                eprevesymself(searchtet);
            } else {
                esymself(searchtet);
            }
            if chkencflag && self.issubface(searchtet) {
                // We are walking across a subface.
                loc = LocateResult::EncSubface;
                break;
            }
            // Move to the adjacent tetrahedron (maybe a hull tetrahedron).
            //   Upstream decodes the neighbour pointer as it is (its version
            //   is not mapped through `fsymtbl`, unlike `fsymself`).
            *searchtet = super::mesh::decode(self.tets[searchtet.tet].nbr[searchtet.ver & 3]);
            if self.ishulltet(searchtet.tet) {
                loc = LocateResult::Outside;
                break;
            }
            max_visited_tets -= 1;

            // Retreat the three vertices of the base face.
            torg = self.org(searchtet);
            tdest = self.dest(searchtet);
            tapex = self.apex(searchtet);
        }
        Ok(loc)
    }

    /// The `splittet` test after a boundary split in `split_tetrahedron`:
    /// is the tet dead or its vertices changed.
    #[inline]
    fn tet_changed(&self, tet: usize, bak: &[Point; 4]) -> bool {
        self.tets[tet].v[0] == NULL_POINT || self.tets[tet].v != *bak
    }

    /// `split_tetrahedron(splittet, param, qflag, chkencflag, ivf)`: insert
    /// the circumcentre `param[0..3]`; if it would encroach upon a segment
    /// or a subface, split that instead. True if the tetrahedron was split.
    pub fn split_tetrahedron(
        &mut self,
        splittet: &TriFace,
        param: &mut [f64; 6],
        qflag: i32,
        chkencflag: i32,
        ivf: &mut InsertPointFlags,
    ) -> Result<bool, TetgenError> {
        self.reach6[24] += 1;
        self.rf.insert_point_count += 1;
        let mut splitflag = false;

        let newpt = self.makepoint(VertType::FreeVol);
        self.points[newpt].xyz = [param[0], param[1], param[2]];

        // Locate the new point by a walk from the interior of splittet.
        let mut searchtet = *splittet;
        ivf.iloc = self.locate_point_walk(newpt, &mut searchtet, true)?;

        if ivf.iloc == LocateResult::EncSubface || ivf.iloc == LocateResult::Outside {
            // The circumcenter is not visible from the interior of the tet.
            self.reach6[26] += 1;
            self.pointdealloc(newpt); // Do not insert this vertex.
            ivf.iloc = LocateResult::FensedIn;
            return Ok(splitflag);
        }

        ivf.bowywat = 3;
        ivf.lawson = 2;
        ivf.rejflag = 3; // Do check for encroached segments and subfaces.
        ivf.chkencflag = chkencflag & !3;
        ivf.sloc = LocateResult::Unknown;
        ivf.sbowywat = 0;
        ivf.splitbdflag = false;
        ivf.validflag = true;
        ivf.respectbdflag = true;
        ivf.refineflag = 1;
        ivf.refinetet = *splittet;
        ivf.smlenflag = true; // useinsertradius
        ivf.check_insert_radius = qflag == 0;
        ivf.parentpt = NULL_POINT;

        if self.insertpoint(newpt, &mut searchtet, None, None, ivf)? {
            // Vertex is inserted.
            self.reach6[25] += 1;
            self.st_volref_count += 1;
            if self.steinerleft > 0 {
                self.steinerleft -= 1;
            }
            // Save the shortest edge between: emin and ivf.smlen.
            let mut rv = 0.0;
            if param[3] > 0.0 {
                rv = if param[3] < ivf.smlen {
                    param[3]
                } else {
                    ivf.smlen
                };
            }
            self.points[newpt].insradius = rv;
            self.points[newpt].point2ppt = ivf.parentpt;
            if ivf.smlen < self.rf.smallest_insradius {
                self.rf.smallest_insradius = ivf.smlen;
            }
            self.refine_lawson(chkencflag & !3)?;
            if self.later_unflip_queue.len() as i64 > self.b.unflip_queue_limit {
                self.reach6[39] += 1;
                self.recoverdelaunay()?;
            }
            return Ok(true);
        }

        // Point is not inserted.
        self.pointdealloc(newpt);

        if ivf.iloc == LocateResult::EncSegment {
            self.reach6[27] += 1;
            if self.b.nobisect == 0 {
                // Bakup the vertices of this tet.
                let bak_pts = self.tets[splittet.tet].v;
                let ref_segment = (self.b.cdtrefine & 1) > 0;
                if ref_segment || qflag != 0 {
                    let mut i = 0;
                    while i < self.rf.encseglist.len() {
                        let mut bf = self.rf.encseglist[i];
                        i += 1;
                        if bf.ss.is_null()
                            || self.sorg(&bf.ss) != bf.forg
                            || self.sdest(&bf.ss) != bf.fdest
                        {
                            continue; // Skip this segment.
                        }
                        let mut tmp_iloc = LocateResult::Unknown;
                        let ok = self.split_segment(
                            &mut bf.ss,
                            NULL_POINT,
                            param,
                            qflag,
                            chkencflag | 3,
                            &mut tmp_iloc,
                        )?;
                        self.rf.encseglist[i - 1] = bf;
                        if ok {
                            // A Steiner point is inserted on a segment.
                            if self.tet_changed(splittet.tet, &bak_pts) {
                                splitflag = true; // The tet is split as well.
                            }
                            if splitflag {
                                break;
                            }
                        }
                    }
                }
                self.rf.encseglist.clear();
                // Some segments may need to be repaired.
                if !self.rf.badsubsegs.is_empty() {
                    self.repairencsegs(param, 0, chkencflag | 3)?; // qflag = 0
                }
                // Some subfaces may need to be repaired.
                if !self.rf.badsubfacs.is_empty() {
                    self.repairencfacs(param, 0, chkencflag | 2)?; // qflag = 0
                    if !self.rf.unsplit_subfaces.is_empty() {
                        self.rf.unsplit_subfaces.clear();
                    }
                }
                if self.tet_changed(splittet.tet, &bak_pts) {
                    splitflag = true;
                }
            } else {
                self.rf.encseglist.clear();
            }
        } else if ivf.iloc == LocateResult::EncSubface {
            self.reach6[28] += 1;
            if self.b.nobisect == 0 {
                let bak_pts = self.tets[splittet.tet].v;
                let ref_subface = (self.b.cdtrefine & 2) > 0;
                if ref_subface || qflag != 0 {
                    // Split the subface which contains the projection of
                    //   this rejected Steiner point.
                    let mut scount = 0;
                    // Clean the bad radius-edge ratio, so split_subface()
                    //   knows the split is due to a rejected tet ccenter.
                    param[4] = 0.0;
                    let xp = [param[0], param[1], param[2]];
                    let mut i = 0;
                    while i < self.rf.encshlist.len() {
                        let bface = self.rf.encshlist[i];
                        i += 1;
                        if !self.refine_face_matches(&bface) {
                            continue;
                        }
                        let mut adjtet = self.stpivot(&bface.ss);
                        if self.ishulltet(adjtet.tet) {
                            self.fsymself(&mut adjtet);
                        }
                        let toppo = self.oppo(&adjtet);
                        let (xa, xb, xc, xt) = (
                            self.xr(self.org(&adjtet)),
                            self.xr(self.dest(&adjtet)),
                            self.xr(self.apex(&adjtet)),
                            self.xr(toppo),
                        );
                        let prjpt = projpt2face(&xp, &xa, &xb, &xc);
                        if self.pred.orient3d(&xa, &xb, &xt, &prjpt) >= 0.0
                            && self.pred.orient3d(&xb, &xc, &xt, &prjpt) >= 0.0
                            && self.pred.orient3d(&xc, &xa, &xt, &prjpt) >= 0.0
                        {
                            scount += 1;
                            // Found such a subface, try to split it.
                            let cc = [bface.cent[0], bface.cent[1], bface.cent[2]];
                            let mut tmp_iloc = LocateResult::Unknown;
                            self.split_subface(
                                &bface.ss,
                                NULL_POINT,
                                &cc,
                                param,
                                qflag,
                                chkencflag | 2,
                                &mut tmp_iloc,
                            )?;
                            if self.tet_changed(splittet.tet, &bak_pts) {
                                splitflag = true;
                            }
                            if splitflag {
                                break;
                            }
                        }
                    }
                    if scount == 0 {
                        // Select an encroached subface and split it.
                        self.reach6[29] += 1;
                        let mut i = 0;
                        while i < self.rf.encshlist.len() {
                            let bface = self.rf.encshlist[i];
                            i += 1;
                            if !self.refine_face_matches(&bface) {
                                continue;
                            }
                            let cc = [bface.cent[0], bface.cent[1], bface.cent[2]];
                            let mut tmp_iloc = LocateResult::Unknown;
                            self.split_subface(
                                &bface.ss,
                                NULL_POINT,
                                &cc,
                                param,
                                qflag,
                                chkencflag | 2,
                                &mut tmp_iloc,
                            )?;
                            if self.tet_changed(splittet.tet, &bak_pts) {
                                splitflag = true;
                            }
                            if splitflag {
                                break;
                            }
                        }
                    }
                }
                self.rf.encshlist.clear();
                // Some subfaces may need to be repaired.
                if !self.rf.badsubfacs.is_empty() {
                    self.repairencfacs(param, 0, chkencflag | 2)?; // qflag = 0
                    if !self.rf.unsplit_subfaces.is_empty() {
                        self.rf.unsplit_subfaces.clear();
                    }
                }
                if self.tet_changed(splittet.tet, &bak_pts) {
                    splitflag = true;
                }
            } else {
                self.rf.encshlist.clear();
            }
        }

        Ok(splitflag)
    }

    /// `bface->ss.sh != NULL && sorg == forg && sdest == fdest && sapex ==
    /// fapex`: the queued subface is still the one recorded.
    #[inline]
    fn refine_face_matches(&self, bf: &RefineFace) -> bool {
        !bf.ss.is_null()
            && self.sorg(&bf.ss) == bf.forg
            && self.sdest(&bf.ss) == bf.fdest
            && self.sapex(&bf.ss) == bf.fapex
    }

    /// The badface of an unsplit tetrahedron (`unsplit_badtets`).
    fn unsplit_tet(&self, t: &TriFace, param: &[f64; 6], qflag: i32) -> RefineFace {
        RefineFace {
            tt: *t,
            forg: self.org(t),
            fdest: self.dest(t),
            fapex: self.apex(t),
            foppo: self.oppo(t),
            cent: *param,
            key: qflag as f64,
            ..RefineFace::default()
        }
    }

    /// `repairbadtets(queratio, chkencflag)`: split the bad tetrahedra of
    /// `check_tets_list` (and those the splits queue), picked at random.
    pub fn repairbadtets(&mut self, queratio: f64, chkencflag: i32) -> Result<(), TetgenError> {
        self.reach6[19] += 1;
        let mut param = [0.0f64; 6];
        let mut qflag = 0;

        loop {
            let nbad = self.badtetrahedrons.as_ref().map_or(0, |q| q.len());
            if nbad == 0 && self.rf.check_tets_list.is_empty() {
                break;
            }
            if nbad > 0 {
                let q = core::mem::take(self.badtetrahedrons.as_mut().unwrap());
                self.rf.check_tets_list.extend(q);
            }
            if self.steinerleft == 0 {
                break;
            }
            if self.rf.elem_limit > 0
                && (self.tetpool.items as i64 - self.hullsize) > self.rf.elem_limit
            {
                break;
            }

            // Randomly select a tet to split.
            let n = self.rf.check_tets_list.len();
            let i = self.crand.rand() as usize % n;
            let mut checktet = self.rf.check_tets_list[i];
            // Fill the current position by the last tet in the list.
            self.rf.check_tets_list[i] = self.rf.check_tets_list[n - 1];
            self.rf.check_tets_list.pop();

            if !self.isdeadtet(checktet.tet) && self.marktest2ed(checktet.tet) {
                self.unmarktest2(checktet.tet);
                if self.checktet4split(&mut checktet, &mut param, &mut qflag)? {
                    self.reach6[20] += 1;
                    let mut ivf = InsertPointFlags::default();
                    let splitflag =
                        self.split_tetrahedron(&checktet, &mut param, qflag, chkencflag, &mut ivf)?;
                    if !splitflag && (qflag != 0 || param[4] > queratio) {
                        self.reach6[30] += 1;
                        let bt = self.unsplit_tet(&checktet, &param, qflag);
                        self.rf.unsplit_badtets.push(bt);
                    }
                }
            }
        }

        if !self.rf.check_tets_list.is_empty() {
            // Unmark all unchecked tetrahedra.
            for t in core::mem::take(&mut self.rf.check_tets_list) {
                if !self.isdeadtet(t.tet) {
                    self.unmarktest2(t.tet);
                }
            }
        }
        Ok(())
    }

    /// The unsplit-tetrahedra pass of `delaunayrefinement` (the `-q` volume
    /// loop): retry every `unsplit_badtets` entry still alive. Returns the
    /// number split.
    fn split_unsplit_badtets(&mut self, chkencflag: i32) -> Result<i64, TetgenError> {
        let mut splitcount = 0;
        let list = core::mem::take(&mut self.rf.unsplit_badtets);
        for bt in &list {
            if self.isdeadtet(bt.tt.tet)
                || self.org(&bt.tt) != bt.forg
                || self.dest(&bt.tt) != bt.fdest
                || self.apex(&bt.tt) != bt.fapex
                || self.oppo(&bt.tt) != bt.foppo
            {
                continue;
            }
            if self.steinerleft == 0 {
                break;
            }
            if self.rf.elem_limit > 0
                && (self.tetpool.items as i64 - self.hullsize) > self.rf.elem_limit
            {
                break;
            }
            self.reach6[31] += 1;
            let mut ivf = InsertPointFlags::default();
            let qflag = bt.key as i32;
            let v = self.tets[bt.tt.tet].v;
            let (x0, x1, x2, x3) = (self.xr(v[0]), self.xr(v[1]), self.xr(v[2]), self.xr(v[3]));
            let mut param = [0.0f64; 6];
            for j in 0..3 {
                param[j] = (x0[j] + x1[j] + x2[j] + x3[j]) / 4.0;
            }
            param[3..].copy_from_slice(&bt.cent[3..]);
            if self.split_tetrahedron(&bt.tt, &mut param, qflag, chkencflag, &mut ivf)? {
                splitcount += 1;
            }
            // Push new bad quality tetrahedron into queue.
            if let Some(q) = self.badtetrahedrons.as_mut() {
                if !q.is_empty() {
                    let q = core::mem::take(q);
                    self.rf.check_tets_list.extend(q);
                }
            }
        }
        Ok(splitcount)
    }

    /// `delaunayrefinement()`: split encroached segments, then encroached
    /// and bad subfaces, then bad tetrahedra (see the module note).
    pub fn delaunayrefinement(&mut self) -> Result<(), TetgenError> {
        self.reach6[0] += 1;
        let mut param = [0.0f64; 6];

        // Used in locate_on_surface(): -p/#.
        self.rf.cos_facet_separate_ang_tol = (self.b.facet_separate_ang_tol / 180.0 * PI).cos();
        // Used in is_collinear_at(): -p///#.
        self.cos_collinear_ang_tol = (self.b.collinear_ang_tol / 180.0 * PI).cos();
        // The cosine value of the min dihedral angle (-q/#) for tetrahedra.
        self.rf.cosmindihed = (self.b.mindihedral / 180.0 * PI).cos();
        self.rf.smallest_insradius = 1.0e30;

        self.steinerleft = self.b.steinerleft; // Upperbound of # Steiner points (by -S#).
        if self.steinerleft > 0 {
            let steinercount = self.st_segref_count + self.st_facref_count + self.st_volref_count;
            if steinercount < self.steinerleft {
                self.steinerleft -= steinercount;
            } else {
                return Ok(()); // No more Steiner points.
            }
        }

        if self.b.nobisect == 0 {
            if self.segmentendpointslist.is_empty() {
                self.makesegmentendpointsmap(); // create ridge_vertex-to-segment map.
            }
            self.create_segment_info_list();
            self.makefacetverticesmap(); // create ridge_vertex-to-facet map.
            self.create_segment_facet_map(); // create segment-to-facet map.
        }

        // Initialize the pools and priority queues.
        self.rf.badsubsegs.clear();
        self.rf.badsubfacs.clear();
        self.badtetrahedrons = Some(Vec::new());
        self.rf.stack_enc_segments.clear();
        self.rf.subfaces = SubfaceQueue::default();
        self.rf.check_tets_list.clear();
        self.rf.unsplit_subfaces.clear();
        self.rf.unsplit_badtets.clear();
        self.rf.encseglist.clear();
        self.rf.encshlist.clear();

        let mut force_quit_refinement = false;
        if self.steinerleft == 0
            || (self.rf.elem_limit > 0
                && (self.tetpool.items as i64 - self.hullsize) > self.rf.elem_limit)
        {
            force_quit_refinement = true;
        }

        if self.b.nobisect == 0 {
            let ref_segment = (self.b.cdtrefine & 1) > 0;
            if ref_segment && !force_quit_refinement {
                let chkencflag = 1; // Only check encroaching subsegments.
                                    // Add all segments into the pool.
                self.subsegpool.traversalinit();
                while let Some(slot) = self.shellfacetraverse(ShPool::Seg) {
                    let checkseg = Face {
                        pool: ShPool::Seg,
                        sh: slot,
                        shver: 0,
                    };
                    let mut encpt = NULL_POINT;
                    if self.check_enc_segment(&checkseg, &mut encpt) {
                        let bf = RefineFace {
                            ss: checkseg,
                            forg: self.sorg(&checkseg),
                            fdest: self.sdest(&checkseg),
                            noppo: encpt,
                            ..RefineFace::default()
                        };
                        self.rf.stack_enc_segments.push(bf);
                    }
                }
                // Split all encroached segments.
                param = [0.0; 6];
                self.repairencsegs(&mut param, 0, chkencflag)?;
                if !self.later_unflip_queue.is_empty() {
                    self.reach6[39] += 1;
                    self.recoverdelaunay()?;
                }
            }

            let ref_surface = (self.b.cdtrefine & 2) > 0;
            if ref_surface && !force_quit_refinement {
                let chkencflag = 2; // only check encroaching subfaces.
                                    // Add all subfaces into the pool. (`checksh.shver` is carried
                                    //   from one subface to the next, as upstream's: check_subface
                                    //   turns it to the shortest edge.)
                let mut shver = 0;
                self.subfacepool.traversalinit();
                while let Some(slot) = self.shellfacetraverse(ShPool::Sub) {
                    let mut checksh = Face {
                        pool: ShPool::Sub,
                        sh: slot,
                        shver,
                    };
                    let ccent = self.get_subface_ccent(&checksh)?;
                    let mut encpt = NULL_POINT;
                    for p in param.iter_mut().skip(3) {
                        *p = 0.0;
                    }
                    let mut radius = 0.0;
                    if self.check_enc_subface(&checksh, &mut encpt, &ccent, &mut radius) {
                        self.enqueue_subface(&checksh, encpt, &ccent, &param);
                    } else if self.check_subface(&mut checksh, radius, &mut param) {
                        self.enqueue_subface(&checksh, NULL_POINT, &ccent, &param);
                    }
                    shver = checksh.shver;
                }
                // check_enc_subface() may find some non-Delaunay faces.
                self.refine_lawson(chkencflag)?;

                // Split all encroached subfaces.
                param = [0.0; 6];
                let qflag = 0;
                for _iter in 0..3 {
                    self.repairencfacs(&mut param, qflag, chkencflag)?;
                    if !self.rf.unsplit_subfaces.is_empty() {
                        let list = core::mem::take(&mut self.rf.unsplit_subfaces);
                        for bf in &list {
                            if self.refine_face_matches(bf) {
                                // Try to split it in its barycenter.
                                let (xa, xb, xc) =
                                    (self.xr(bf.forg), self.xr(bf.fdest), self.xr(bf.fapex));
                                let mut ccent = [0.0; 3];
                                for j in 0..3 {
                                    ccent[j] = (xa[j] + xb[j] + xc[j]) / 3.0;
                                }
                                param[3..].copy_from_slice(&bf.cent[3..]);
                                let encpt = bf.noppo; // The encroaching vertex.
                                let mut iloc = LocateResult::Unknown;
                                self.split_subface(
                                    &bf.ss, encpt, &ccent, &mut param, qflag, chkencflag, &mut iloc,
                                )?;
                            }
                        }
                        // (unsplit_subfaces->restart(): entries pushed while
                        //   the list was walked are dropped with it.)
                        self.rf.unsplit_subfaces.clear();
                    } else {
                        break; // no unsplit subfaces.
                    }
                }

                if !self.rf.badsubfacs.is_empty() {
                    // Clean this pool.
                    for bface in core::mem::take(&mut self.rf.badsubfacs) {
                        if self.sh_alive(&bface) && self.smarktest2ed(&bface) {
                            self.sunmarktest2(&bface);
                        }
                    }
                }
                if !self.later_unflip_queue.is_empty() {
                    self.reach6[39] += 1;
                    self.recoverdelaunay()?;
                }
            }
        }

        if (self.b.cdtrefine & 4) > 0 && !force_quit_refinement {
            // Add all tetrahedra (no hull tets) into the pool.
            self.tetpool.traversalinit();
            while let Some(t) = self.tetrahedrontraverse() {
                self.marktest2(t);
                self.rf.check_tets_list.push(TriFace { tet: t, ver: 0 });
            }

            let chkencflag = 4; // Check bad tetrahedra.
            let mut queratio = if self.b.minratio > 2.0 {
                self.b.minratio
            } else {
                2.0
            };
            queratio *= 2.0;

            for _iter in 0..3 {
                // Split all bad quality tetrahedra.
                self.repairbadtets(queratio, chkencflag)?;
                if !self.later_unflip_queue.is_empty() {
                    self.reach6[39] += 1;
                    self.recoverdelaunay()?;
                }
                if self.rf.unsplit_badtets.is_empty() {
                    break;
                }
                // Split unsplit tetrahedra.
                let splitcount = self.split_unsplit_badtets(chkencflag)?;
                if splitcount == 0 {
                    break;
                }
            }

            if !self.rf.check_tets_list.is_empty() {
                // Unmark all unchecked tetrahedra.
                for t in core::mem::take(&mut self.rf.check_tets_list) {
                    if !self.isdeadtet(t.tet) {
                        self.unmarktest2(t.tet);
                    }
                }
            }
        }

        // Free the pools.
        self.rf.encseglist.clear();
        self.rf.encshlist.clear();
        self.rf.badsubsegs.clear();
        self.rf.badsubfacs.clear();
        self.rf.subfaces = SubfaceQueue::default();
        self.rf.stack_enc_segments.clear();
        self.rf.unsplit_subfaces.clear();
        self.badtetrahedrons = None;
        self.rf.check_tets_list.clear();
        self.rf.unsplit_badtets.clear();
        Ok(())
    }
}
