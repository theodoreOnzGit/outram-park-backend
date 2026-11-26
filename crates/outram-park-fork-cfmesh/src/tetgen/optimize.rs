// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright (C) 2026 OUTRAM PARK contributors
//
// Port of TetGen 1.6.0 (`tetgen.cxx`, section `optimize_cxx`:
// `get_seg_laplacian_center`, `get_surf_laplacian_center`,
// `get_laplacian_center`, `move_vertex`, `smooth_vertices`, `get_tet`,
// `get_tetqual`, `enqueue_badtet`, `top_badtet`, `dequeue_badtet`,
// `add_steinerpt_to_repair`, `flip_edge_to_improve`, `repair_tet`,
// `repair_badqual_tets`, `improve_mesh`):
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

//! Vertex smoothing and mesh improvement (stage 5, gh:#1794).
//!
//! `tetrahedralize()` runs both under plain `-p` (`b->plc`):
//! [`TetgenMesh::smooth_vertices`] when Steiner points are left in facets
//! or the volume, and [`TetgenMesh::improve_mesh`] always. Mesh
//! improvement queues every tetrahedron with an aspect ratio above
//! `opt_max_asp_ratio` (1000) or a dihedral angle above `optmaxdihedral`
//! (177 degrees) and repairs it by flips (`removeedgebyflips` with
//! `remove_large_angle`), then by a Steiner point at the centroid of a
//! neighbouring tetrahedron, smoothed towards its Laplacian centre.
//!
//! **Not ported:** the `-c` test (`b->convex`) of `improve_mesh`;
//! `insertpoint`'s `smlenflag` (it only fills `ivf.smlen` and
//! `ivf.parentpt`, which `add_steinerpt_to_repair` does not read).

#[cfg(not(any(feature = "std", test)))]
#[allow(unused_imports)]
use crate::nostd_prelude::*;

use super::carve::StarList;
use super::delaunay::{LocateResult, TetgenError};
use super::flip::{edestoppo, senext, senext2, sesym, FlipConstraints};
use super::mesh::{
    edestoppoself, enextself, esym, sdecode, Point, TetgenMesh, TriFace, VertType, EDGE2VER,
    NULL_POINT, NULL_TET,
};
use super::steiner::InsertPointFlags;
use super::surface::{facenormal, lu_decmp, lu_solve};
use alloc::collections::VecDeque;
use alloc::vec::Vec;

const PI: f64 = core::f64::consts::PI;

#[inline]
fn dot(v1: &[f64; 3], v2: &[f64; 3]) -> f64 {
    v1[0] * v2[0] + v1[1] * v2[1] + v1[2] * v2[2]
}
#[inline]
fn distance(p1: &[f64; 3], p2: &[f64; 3]) -> f64 {
    ((p2[0] - p1[0]) * (p2[0] - p1[0])
        + (p2[1] - p1[1]) * (p2[1] - p1[1])
        + (p2[2] - p1[2]) * (p2[2] - p1[2]))
        .sqrt()
}

/// `badface` as mesh improvement uses it.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct BadFace {
    /// `tt`: the tetrahedron, at the edge with the largest dihedral angle.
    pub tt: TriFace,
    /// `key`: the aspect ratio.
    pub key: f64,
    /// `cent`: cosines of the largest / smallest dihedral angle, the edge
    /// ratio, the shortest edge, the volume, (unused).
    pub cent: [f64; 6],
    /// `forg`.
    pub forg: Point,
    /// `fdest`.
    pub fdest: Point,
    /// `fapex`.
    pub fapex: Point,
    /// `foppo`.
    pub foppo: Point,
    /// `ss.shver`, re-used: the number of dihedral angles above 135 degrees.
    pub lcount: i32,
}

impl Default for BadFace {
    /// `badface::init()`.
    fn default() -> Self {
        BadFace {
            tt: TriFace::default(),
            key: 0.0,
            cent: [0.0; 6],
            forg: NULL_POINT,
            fdest: NULL_POINT,
            fapex: NULL_POINT,
            foppo: NULL_POINT,
            lcount: 0,
        }
    }
}

/// The priority queue of bad tetrahedra (`bt_queuefront[64]`, ...): 64
/// FIFO buckets by `1 / log(key)`; `top` is the front of the highest
/// non-empty bucket, as upstream's linked buckets give.
#[derive(Debug, Clone)]
pub struct BadTetQueue {
    buckets: Vec<VecDeque<BadFace>>,
    recentq: i32,
}

impl BadTetQueue {
    fn new() -> Self {
        BadTetQueue {
            buckets: (0..64).map(|_| VecDeque::new()).collect(),
            recentq: -1,
        }
    }
    /// `badqual_tets_pool->items`.
    pub fn items(&self) -> usize {
        self.buckets.iter().map(|b| b.len()).sum()
    }
    /// `enqueue_badtet(bf)`.
    fn enqueue(&mut self, bf: &BadFace) {
        let qual = 1.0 / bf.key.ln();
        let mut queuenumber: i32 = 0;
        if qual < 1.0 {
            queuenumber = (64.0 * (1.0 - qual)) as i32;
            if queuenumber > 63 {
                queuenumber = 63;
            }
        }
        self.buckets[queuenumber as usize].push_back(*bf);
    }
    /// `top_badtet()`.
    fn top(&mut self) -> Option<BadFace> {
        self.recentq = -1;
        for q in (0..64).rev() {
            if let Some(f) = self.buckets[q].front() {
                self.recentq = q as i32;
                return Some(*f);
            }
        }
        None
    }
    /// `dequeue_badtet()`.
    fn dequeue(&mut self) {
        if self.recentq >= 0 {
            self.buckets[self.recentq as usize].pop_front();
        }
    }
}

impl TetgenMesh {
    #[inline]
    fn xq(&self, p: Point) -> [f64; 3] {
        self.points[p].xyz
    }

    /// `get_tetqual(chktet, NULL, bf)` (with `chktet`, after `bf->init()`)
    /// or `get_tetqual(NULL, NULL, bf)` (the vertices already in `bf`):
    /// the aspect ratio, dihedral angles and edge ratio. False for a
    /// degenerate or inverted tetrahedron; `bf` then holds what upstream
    /// left in it.
    pub fn get_tetqual(&self, chktet: Option<&TriFace>, bf: &mut BadFace) -> bool {
        if let Some(t) = chktet {
            *bf = BadFace::default();
            let v = self.tets[t.tet].v;
            bf.forg = v[0];
            bf.fdest = v[1];
            bf.fapex = v[2];
            bf.foppo = v[3];
        }
        let (pa, pb, pc, pd) = (
            self.xq(bf.forg),
            self.xq(bf.fdest),
            self.xq(bf.fapex),
            self.xq(bf.foppo),
        );
        let mut a = [[0.0f64; 4]; 4];
        for i in 0..3 {
            a[0][i] = pa[i] - pd[i]; // d->a vec
            a[1][i] = pb[i] - pd[i]; // d->b vec
            a[2][i] = pc[i] - pd[i]; // d->c vec
        }
        let mut vab = [0.0; 3];
        let mut vbc = [0.0; 3];
        let mut vca = [0.0; 3];
        for i in 0..3 {
            vab[i] = pb[i] - pa[i];
            vbc[i] = pc[i] - pb[i];
            vca[i] = pa[i] - pc[i];
        }
        let a0 = [a[0][0], a[0][1], a[0][2]];
        let a1 = [a[1][0], a[1][1], a[1][2]];
        let a2 = [a[2][0], a[2][1], a[2][2]];
        let l = [
            dot(&a2, &a2),
            dot(&a0, &a0),
            dot(&vab, &vab),
            dot(&vbc, &vbc),
            dot(&a1, &a1),
            dot(&vca, &vca),
        ];
        let mut lmax = l[0];
        let mut lmin = l[0];
        for &li in l.iter().skip(1) {
            lmax = if lmax < li { li } else { lmax };
            lmin = if lmin > li { li } else { lmin };
        }
        let lmax = lmax.sqrt();
        let lmin = lmin.sqrt();
        bf.cent[2] = lmax / lmin;
        bf.cent[3] = lmin;

        let mut n = [[0.0f64; 3]; 4];
        let mut h = [0.0f64; 4];
        let mut flat_flag = false;
        let mut indx = [0usize; 4];
        let mut d = 0.0;
        if lu_decmp(&mut a, 3, &mut indx, &mut d) {
            bf.cent[4] = (a[indx[0]][0] * a[indx[1]][1] * a[indx[2]][2]).abs();
            if bf.cent[4] > 0.0 {
                for j in 0..3 {
                    let mut rhs = [0.0f64; 4];
                    rhs[j] = 1.0; // Positive means the inside direction
                    lu_solve(&a, 3, &indx, &mut rhs);
                    n[j] = [rhs[0], rhs[1], rhs[2]];
                }
                for i in 0..3 {
                    n[3][i] = -n[0][i] - n[1][i] - n[2][i];
                }
            } else {
                flat_flag = true;
            }
        } else {
            flat_flag = true;
        }

        if flat_flag {
            // This tet is nearly degenerate.
            bf.cent[4] = self.pred.orient3d(&pb, &pa, &pc, &pd);
            if bf.cent[4] <= 0.0 {
                return false; // degenerated or inverted.
            }
            n[0] = facenormal(&pc, &pb, &pd, 1); // face [c,b,d]
            n[1] = facenormal(&pa, &pc, &pd, 1); // face [a,c,d]
            n[2] = facenormal(&pb, &pa, &pd, 1); // face [b,a,d]
            n[3] = facenormal(&pa, &pb, &pc, 1); // face [a,b,c]
        }

        // Normalized the normals.
        for i in 0..4 {
            h[i] = dot(&n[i], &n[i]).sqrt();
            if h[i] > 0.0 {
                for j in 0..3 {
                    n[i][j] /= h[i];
                }
            } else {
                return false;
            }
        }

        if !flat_flag {
            let mut minheightinv = h[0];
            for &hi in h.iter().skip(1) {
                if hi > minheightinv {
                    minheightinv = hi;
                }
            }
            bf.key = lmax * minheightinv;
        } else {
            bf.key = 1.0e30; // infinity.
        }

        // The cosine of the dihedral angles of the edges.
        let mut cosmaxd = 1.0;
        let mut cosmind = -1.0;
        let mut idx = 0;
        bf.lcount = 0;
        const FACES: [(usize, usize); 6] = [(0, 1), (1, 2), (2, 3), (0, 3), (2, 0), (1, 3)];
        for (i, &(f1, f2)) in FACES.iter().enumerate() {
            let mut cosd = -dot(&n[f1], &n[f2]);
            if cosd < -1.0 {
                cosd = -1.0; // Rounding.
            }
            if cosd > 1.0 {
                cosd = 1.0;
            }
            if cosd < cosmaxd {
                cosmaxd = cosd;
                idx = i;
            }
            cosmind = if cosd > cosmind { cosd } else { cosmind };
            if cosd < self.cos_large_dihed {
                bf.lcount += 1;
            }
        }
        bf.cent[0] = cosmaxd;
        bf.cent[1] = cosmind;
        // Remember the edge with largest dihedral angle.
        if let Some(t) = chktet {
            bf.tt.tet = t.tet;
        }
        bf.tt.ver = EDGE2VER[idx];
        bf.cent[5] = 0.0;
        true
    }

    /// `get_tetqual(pa, pb, pc, pd, bf)`: `None` where upstream returns
    /// false.
    pub fn get_tetqual_pts(&self, pa: Point, pb: Point, pc: Point, pd: Point) -> Option<BadFace> {
        let mut bf = BadFace {
            forg: pa,
            fdest: pb,
            fapex: pc,
            foppo: pd,
            ..Default::default()
        };
        if self.get_tetqual(None, &mut bf) {
            Some(bf)
        } else {
            None
        }
    }

    /// `get_tet(pa, pb, pc, pd, searchtet)`: the tetrahedron `[a,b,c,d]`.
    pub fn get_tet(
        &mut self,
        pa: Point,
        pb: Point,
        pc: Point,
        pd: Point,
        searchtet: &mut TriFace,
    ) -> Result<bool, TetgenError> {
        if self.getedge(pa, pb, searchtet)? {
            let mut spintet = *searchtet;
            loop {
                if self.apex(&spintet) == pc {
                    *searchtet = spintet;
                    break;
                }
                self.fnextself(&mut spintet);
                if spintet.tet == searchtet.tet {
                    break;
                }
            }
            if self.apex(searchtet) == pc {
                if self.oppo(searchtet) == pd {
                    return Ok(true);
                }
                self.fsymself(searchtet);
                if self.oppo(searchtet) == pd {
                    return Ok(true);
                }
            }
        }
        Ok(false)
    }

    // -- smoothing ----------------------------------------------------------

    /// `get_seg_laplacian_center(mesh_vert, target)`: the midpoint of the
    /// segment's endpoints around a segment Steiner point.
    pub fn get_seg_laplacian_center(
        &self,
        mesh_vert: Point,
    ) -> Result<Option<[f64; 3]>, TetgenError> {
        if self.pointtype(mesh_vert) == VertType::Unused {
            return Ok(None);
        }
        let mut leftseg = sdecode(self.point2sh(mesh_vert));
        leftseg.shver = 0;
        let rightseg;
        if self.sdest(&leftseg) == mesh_vert {
            let mut r = self.spivot(&senext(&leftseg));
            r.shver = 0;
            if self.sorg(&r) != mesh_vert {
                r = sesym(&r);
            }
            if self.sorg(&r) != mesh_vert {
                return Err(TetgenError::InternalError);
            }
            rightseg = r;
        } else {
            rightseg = leftseg;
            let mut l = self.spivot(&senext2(&rightseg));
            l.shver = 0;
            if self.sdest(&l) != mesh_vert {
                l = sesym(&l);
            }
            if self.sdest(&l) != mesh_vert {
                return Err(TetgenError::InternalError);
            }
            leftseg = l;
        }
        let lpt = self.xq(self.sorg(&leftseg));
        let rpt = self.xq(self.sdest(&rightseg));
        Ok(Some([
            0.5 * (lpt[0] + rpt[0]),
            0.5 * (lpt[1] + rpt[1]),
            0.5 * (lpt[2] + rpt[2]),
        ]))
    }

    /// `get_surf_laplacian_center(mesh_vert, target)`: the mean of the
    /// link vertices of a facet Steiner point (each counted twice).
    pub fn get_surf_laplacian_center(&mut self, mesh_vert: Point) -> Option<[f64; 3]> {
        if self.pointtype(mesh_vert) == VertType::Unused {
            return None;
        }
        self.reach5[17] += 1;
        self.getvertexstar_with(true, mesh_vert, StarList::CaveOldTet, false, true);
        let npt = self.caveshlist.len();
        let mut target = [0.0f64; 3];
        for i in 0..npt {
            let s = self.caveshlist[i];
            let e1 = self.xq(self.sorg(&s));
            let e2 = self.xq(self.sdest(&s));
            for j in 0..3 {
                target[j] += e1[j];
            }
            for j in 0..3 {
                target[j] += e2[j];
            }
        }
        let npt2 = (npt * 2) as f64;
        for t in target.iter_mut() {
            *t /= npt2;
        }
        self.caveoldtetlist.clear();
        self.caveshlist.clear();
        Some(target)
    }

    /// `get_laplacian_center(mesh_vert, target)`: the mean of the link
    /// vertices of a volume Steiner point. Leaves its star in
    /// `caveoldtetlist`, as upstream does.
    pub fn get_laplacian_center(&mut self, mesh_vert: Point) -> Option<[f64; 3]> {
        if self.pointtype(mesh_vert) == VertType::Unused {
            return None;
        }
        self.reach5[18] += 1;
        self.getvertexstar_with(true, mesh_vert, StarList::CaveOldTet, true, false);
        let npt = self.cavetetvertlist.len();
        let mut target = [0.0f64; 3];
        for i in 0..npt {
            let p = self.xq(self.cavetetvertlist[i]);
            for j in 0..3 {
                target[j] += p[j];
            }
        }
        for t in target.iter_mut() {
            *t /= npt as f64;
        }
        self.cavetetvertlist.clear();
        Some(target)
    }

    /// `move_vertex(mesh_vert, target)`: move a vertex `smooth_alpha` of the
    /// way to `target` (halving up to three times while a tetrahedron of its
    /// star would invert), then restore local Delaunayness. True if moved.
    pub fn move_vertex(
        &mut self,
        mesh_vert: Point,
        target: &[f64; 3],
    ) -> Result<bool, TetgenError> {
        if self.pointtype(mesh_vert) == VertType::Unused {
            self.caveoldtetlist.clear();
            return Ok(false);
        }
        let mv = self.xq(mesh_vert);
        // Do not move if the target is already very close the vertex.
        if distance(&mv, target) < self.minedgelength {
            self.caveoldtetlist.clear();
            return Ok(false);
        }
        let mut alpha = self.b.smooth_alpha;
        let mut dir = [0.0; 3];
        let mut newpos = [0.0; 3];
        for j in 0..3 {
            dir[j] = target[j] - mv[j];
            newpos[j] = mv[j] + alpha * dir[j];
        }
        if self.caveoldtetlist.is_empty() {
            self.getvertexstar_with(true, mesh_vert, StarList::CaveOldTet, false, false);
        }

        let mut moveflag = true;
        let mut iter = 0;
        while iter < 3 {
            for i in 0..self.caveoldtetlist.len() {
                let t = self.caveoldtetlist[i];
                if self.ishulltet(t.tet) {
                    continue; // Skip a hull face.
                }
                let pa = self.xq(self.org(&t));
                let pb = self.xq(self.dest(&t));
                let pc = self.xq(self.apex(&t));
                if self.pred.orient3d(&pa, &pb, &pc, &newpos) >= 0.0 {
                    moveflag = false;
                    break; // This tet becomes invalid.
                }
            }
            if moveflag {
                break;
            }
            alpha /= 2.0;
            for j in 0..3 {
                newpos[j] = mv[j] + alpha * dir[j];
            }
            iter += 1;
        }

        if moveflag {
            self.points[mesh_vert].xyz = newpos;
            self.reach5[16] += 1;
            // Push all faces of this vertex star and link into queue.
            for i in 0..self.caveoldtetlist.len() {
                let mut cavetet = self.caveoldtetlist[i];
                if self.ishulltet(cavetet.tet) {
                    continue;
                }
                self.flippush(&cavetet);
                for _ in 0..3 {
                    let checkface = esym(&cavetet);
                    let neightet = self.fsym(&checkface);
                    if !self.facemarked(&neightet) {
                        self.flippush(&checkface);
                    }
                    enextself(&mut cavetet);
                }
                self.caveoldtetlist[i] = cavetet;
            }
            let queued = self.badtetrahedrons.is_some();
            if queued {
                // Queue all cavity tets for quality check.
                for i in 0..self.caveoldtetlist.len() {
                    let t = self.caveoldtetlist[i];
                    if self.ishulltet(t.tet) {
                        continue;
                    }
                    self.enqueuetetrahedron(&t);
                }
            }
            let mut fc = FlipConstraints {
                enqflag: 2, // queue all exterior faces of a flip.
                chkencflag: if queued { 4 } else { 0 },
                ..Default::default()
            };
            self.lawsonflip3d(&mut fc)?;
        }
        self.caveoldtetlist.clear();
        Ok(moveflag)
    }

    /// `smooth_vertices()`: move the Steiner points towards their
    /// Laplacian centres (`-s`: criterion, iterations, relaxation).
    pub fn smooth_vertices(&mut self) -> Result<(), TetgenError> {
        self.reach5[15] += 1;
        let mut smpt_list: Vec<Point> = Vec::new();
        let mut surf_smpt_list: Vec<Point> = Vec::new();
        let mut seg_smpt_list: Vec<Point> = Vec::new();
        self.pointpool.traversalinit();
        while let Some(p) = self.pointtraverse() {
            match self.pointtype(p) {
                VertType::FreeVol => smpt_list.push(p),
                VertType::FreeFacet => surf_smpt_list.push(p),
                VertType::FreeSeg => seg_smpt_list.push(p),
                _ => {}
            }
        }
        if smpt_list.len() as i64 != self.st_volref_count
            || surf_smpt_list.len() as i64 != self.st_facref_count
            || seg_smpt_list.len() as i64 != self.st_segref_count
        {
            return Err(TetgenError::InternalError);
        }

        // (Upstream's target arrays are uninitialised `new REAL[]`; an
        //   unused vertex's target is never read: move_vertex returns first.)
        let mut seg_target = alloc::vec![[0.0f64; 3]; seg_smpt_list.len()];
        let mut surf_target = alloc::vec![[0.0f64; 3]; surf_smpt_list.len()];
        let mut target = alloc::vec![[0.0f64; 3]; smpt_list.len()];
        let crit = self.b.smooth_cirterion;
        let limit = self.b.unflip_queue_limit as usize;

        for _iter in 0..self.b.smooth_maxiter {
            let mut movedcount = 0;
            if crit & 4 > 0 {
                for (i, &p) in seg_smpt_list.iter().enumerate() {
                    if let Some(t) = self.get_seg_laplacian_center(p)? {
                        seg_target[i] = t;
                    }
                }
                for (i, &p) in seg_smpt_list.iter().enumerate() {
                    if self.move_vertex(p, &seg_target[i])? {
                        if self.later_unflip_queue.len() > limit {
                            self.recoverdelaunay()?;
                        }
                        movedcount += 1;
                    }
                }
            }
            if crit & 2 > 0 {
                for (i, &p) in surf_smpt_list.iter().enumerate() {
                    if let Some(t) = self.get_surf_laplacian_center(p) {
                        surf_target[i] = t;
                    }
                }
                for (i, &p) in surf_smpt_list.iter().enumerate() {
                    if self.move_vertex(p, &surf_target[i])? {
                        if self.later_unflip_queue.len() > limit {
                            self.recoverdelaunay()?;
                        }
                        movedcount += 1;
                    }
                }
            }
            if crit & 1 > 0 {
                for (i, &p) in smpt_list.iter().enumerate() {
                    if let Some(t) = self.get_laplacian_center(p) {
                        target[i] = t;
                    }
                    self.caveoldtetlist.clear();
                }
                for (i, &p) in smpt_list.iter().enumerate() {
                    if self.move_vertex(p, &target[i])? {
                        if self.later_unflip_queue.len() > limit {
                            self.recoverdelaunay()?;
                        }
                        movedcount += 1;
                    }
                }
            }
            if movedcount == 0 {
                break;
            }
            if !self.later_unflip_queue.is_empty() {
                self.recoverdelaunay()?;
            }
        }
        Ok(())
    }

    // -- mesh improvement ---------------------------------------------------

    /// Queue `bf` (its vertices taken from `bf.tt`) if it is bad: aspect
    /// ratio above `opt_max_asp_ratio` or largest dihedral angle above
    /// `optmaxdihedral`.
    fn enqueue_if_bad(&self, q: &mut BadTetQueue, bf: &mut BadFace) {
        if bf.key > self.b.opt_max_asp_ratio || bf.cent[0] < self.cosmaxdihed {
            bf.forg = self.org(&bf.tt);
            bf.fdest = self.dest(&bf.tt);
            bf.fapex = self.apex(&bf.tt);
            bf.foppo = self.oppo(&bf.tt);
            q.enqueue(bf);
        }
    }

    /// `flip_edge_to_improve(sliver_edge, improved_cosmaxd)`: remove the
    /// edge by flips that do not create a worse dihedral angle.
    fn flip_edge_to_improve(
        &mut self,
        q: &mut BadTetQueue,
        sliver_edge: &mut TriFace,
        improved_cosmaxd: f64,
    ) -> Result<bool, TetgenError> {
        if self.issubseg(sliver_edge) {
            return Ok(false);
        }
        let mut fc = FlipConstraints {
            noflip_in_surface: self.b.nobisect > 0 || (self.b.cdtrefine & 2) == 0,
            remove_large_angle: true,
            unflip: true,
            collectnewtets: true,
            checkflipeligibility: true,
            cosdihed_in: improved_cosmaxd,
            cosdihed_out: 0.0, // 90 degree.
            max_asp_out: 0.0,
            ..Default::default()
        };
        if self.removeedgebyflips(sliver_edge, &mut fc)? == 2 {
            self.reach5[25] += 1;
            // This sliver is removed by flips.
            if fc.cosdihed_out < self.cosmaxdihed || fc.max_asp_out > self.b.opt_max_asp_ratio {
                // Queue new bad tets for further improvements.
                for j in 0..self.cavetetlist.len() {
                    let t = self.cavetetlist[j];
                    if !self.isdeadtet(t.tet) && !self.ishulltet(t.tet) {
                        let mut bf = BadFace::default();
                        if self.get_tetqual(Some(&t), &mut bf) {
                            self.enqueue_if_bad(q, &mut bf);
                        } else {
                            return Err(TetgenError::InternalError);
                        }
                    }
                }
            }
            self.cavetetlist.clear();
            return Ok(true);
        }
        Ok(false)
    }

    /// The short-edge branch shared by `repair_tet` and
    /// `add_steinerpt_to_repair` (a skinny tetrahedron).
    fn check_short_edge(&mut self, bf: &BadFace) -> Result<(), TetgenError> {
        let lmin = bf.cent[3];
        // Get the shortest edge of this tet.
        let mut short_edge = bf.tt;
        let mut i = 0;
        while i < 6 {
            short_edge.ver = EDGE2VER[i];
            let dd = distance(
                &self.xq(self.org(&short_edge)),
                &self.xq(self.dest(&short_edge)),
            );
            if (lmin - dd).abs() / lmin < 1e-4 {
                break;
            }
            i += 1;
        }
        if i == 6 {
            return Err(TetgenError::InternalError);
        }
        if lmin <= self.minedgelength {
            self.reach5[29] += 1;
            // A very short edge. Check if it was correctly created.
            let e1 = self.org(&short_edge);
            let e2 = self.dest(&short_edge);
            if self.issteinerpoint(e1) {
                if !self.create_a_shorter_edge(e1, e2)? {
                    return Err(TetgenError::InternalError);
                }
            } else if self.issteinerpoint(e2) && !self.create_a_shorter_edge(e2, e1)? {
                return Err(TetgenError::InternalError);
            }
        }
        Ok(())
    }

    /// `repair_tet(bf, bFlips, bSmooth, bSteiners)`.
    fn repair_tet(
        &mut self,
        q: &mut BadTetQueue,
        bf: &mut BadFace,
        b_flips: bool,
        b_smooth: bool,
        b_steiners: bool,
    ) -> Result<bool, TetgenError> {
        self.reach5[21] += 1;
        let cosmaxd = bf.cent[0];
        let eta = bf.cent[2];
        let lcount = bf.lcount;

        if cosmaxd < self.cossmtdihed {
            // It is a sliver (flat).
            let mut shape = '0';
            if lcount == 2 {
                shape = 'S';
                self.reach5[22] += 1;
            } else if lcount == 3 {
                shape = 'T';
                self.reach5[23] += 1;
            }
            if b_flips {
                if shape == 'S' {
                    let mut sliver_edge = bf.tt;
                    if self.flip_edge_to_improve(q, &mut sliver_edge, cosmaxd)? {
                        self.opt_flips_count += 1;
                        return Ok(true);
                    }
                    // Due to 'unflip', the flip function may modify the sliver.
                    let (a, b, c, d) = (bf.forg, bf.fdest, bf.fapex, bf.foppo);
                    if self.get_tet(a, b, c, d, &mut bf.tt)? {
                        // Try to flip the opposite edge of this sliver.
                        let mut sliver_edge = edestoppo(&bf.tt); // face [c,d,a]
                        if self.flip_edge_to_improve(q, &mut sliver_edge, cosmaxd)? {
                            self.opt_flips_count += 1;
                            return Ok(true);
                        }
                    }
                } else if shape == 'T' {
                    let mut sliver_edge = edestoppo(&bf.tt); // face [c,d,a]
                    if self.flip_edge_to_improve(q, &mut sliver_edge, cosmaxd)? {
                        self.opt_flips_count += 1;
                        return Ok(true);
                    }
                }
            }
        } else if eta > self.b.opt_max_edge_ratio {
            // It is a skinny tet.
            self.reach5[24] += 1;
            self.check_short_edge(bf)?;
        }

        if b_steiners && (bf.key > self.opt_max_sliver_asp_ratio || cosmaxd < self.cosslidihed) {
            // This sliver is not removed. Due to 'unflip', the flip function
            //   may modify the sliver.
            let (a, b, c, d) = (bf.forg, bf.fdest, bf.fapex, bf.foppo);
            if self.get_tet(a, b, c, d, &mut bf.tt)?
                && self.add_steinerpt_to_repair(q, bf, b_smooth)?
            {
                return Ok(true);
            }
        }
        Ok(false) // not repaired
    }

    /// `add_steinerpt_to_repair(bf, bSmooth)`: split the largest
    /// neighbouring tetrahedron at a sliver's edge by its centroid.
    fn add_steinerpt_to_repair(
        &mut self,
        q: &mut BadTetQueue,
        bf: &mut BadFace,
        b_smooth: bool,
    ) -> Result<bool, TetgenError> {
        self.reach5[26] += 1;
        let cosmaxd = bf.cent[0];
        let eta = bf.cent[2];
        let lcount = bf.lcount;
        let mut splittet = TriFace::default();

        if cosmaxd < self.cosslidihed {
            // It is a sliver (flat).
            if lcount == 2 {
                // A square. Try to remove the edge [a,b].
                let mut max_vol = 0.0;
                let mut check_sliver = bf.tt;
                for _ in 0..2 {
                    let mut is_bdry = false;
                    if self.issubseg(&check_sliver) {
                        is_bdry = true;
                    } else {
                        let mut spintet = check_sliver;
                        loop {
                            if self.issubface(&spintet) {
                                is_bdry = true;
                                break;
                            }
                            self.fnextself(&mut spintet);
                            if spintet.tet == check_sliver.tet {
                                break;
                            }
                        }
                    }
                    if !is_bdry {
                        let mut spintet = check_sliver;
                        loop {
                            let v = self.tets[spintet.tet].v;
                            let vol = self.pred.orient3d(
                                &self.xq(v[1]),
                                &self.xq(v[0]),
                                &self.xq(v[2]),
                                &self.xq(v[3]),
                            );
                            if vol > max_vol {
                                max_vol = vol;
                                splittet = spintet;
                            }
                            self.fnextself(&mut spintet);
                            if spintet.tet == check_sliver.tet {
                                break;
                            }
                        }
                    }
                    // Check the opposite edge.
                    edestoppoself(&mut check_sliver);
                }
            }
            // (lcount == 3, a triangle: nothing, as upstream.)
        } else if eta > self.b.opt_max_edge_ratio {
            self.check_short_edge(bf)?;
        }

        if splittet.tet == NULL_TET {
            return Ok(false); // not added.
        }

        // Do not add if the splittet is also a bad qual tet.
        let mut tmpbf = BadFace::default();
        if self.get_tetqual(Some(&splittet), &mut tmpbf) {
            if tmpbf.cent[0] < self.cosslidihed {
                return Ok(false);
            }
        } else {
            return Ok(false);
        }

        let steinerpt = self.makepoint(VertType::FreeVol);
        let v = self.tets[splittet.tet].v;
        let (p0, p1, p2, p3) = (self.xq(v[0]), self.xq(v[1]), self.xq(v[2]), self.xq(v[3]));
        let mut c = [0.0; 3];
        for j in 0..3 {
            c[j] = (p0[j] + p1[j] + p2[j] + p3[j]) / 4.0;
        }
        self.points[steinerpt].xyz = c;

        let mut ivf = InsertPointFlags {
            iloc: LocateResult::Outside,
            bowywat: 3,
            lawson: 2,
            rejflag: 0,
            chkencflag: if self.badtetrahedrons.is_some() { 4 } else { 0 },
            sloc: LocateResult::Unknown,
            sbowywat: 0,
            splitbdflag: false,
            validflag: true,
            respectbdflag: true,
            ..Default::default()
        };
        // (ivf.smlenflag = 1: not ported, see the module note.)
        if self.insertpoint(steinerpt, &mut splittet, None, None, &mut ivf)? {
            self.reach5[27] += 1;
            self.st_volref_count += 1;
            if !self.tflipstack.is_empty() {
                let mut fc = FlipConstraints {
                    enqflag: 2,
                    chkencflag: if self.badtetrahedrons.is_some() { 4 } else { 0 },
                    ..Default::default()
                };
                self.lawsonflip3d(&mut fc)?;
            }
            if self.later_unflip_queue.len() > self.b.unflip_queue_limit as usize {
                self.later_unflip_queue.clear(); // clean it.
            }
        } else {
            // Point is not inserted.
            self.reach5[28] += 1;
            self.pointdealloc(steinerpt);
            return Ok(false);
        }

        if b_smooth {
            // (Upstream passes the uninitialised `ccent` on when the point
            //   is unused; move_vertex then returns before reading it.)
            let ccent = self.get_laplacian_center(steinerpt).unwrap_or([0.0; 3]);
            if self.move_vertex(steinerpt, &ccent)? {
                self.opt_smooth_count += 1;
            }
        }

        let queued = self.badtetrahedrons.as_ref().map_or(0, |v| v.len());
        if queued > 0 {
            // Push new bad quality tetrahedron into queue.
            let list = self.badtetrahedrons.replace(Vec::new()).unwrap_or_default();
            for bface in list.iter() {
                if !self.isdeadtet(bface.tet) && self.marktest2ed(bface.tet) {
                    // A queued tet may have been processed.
                    self.unmarktest2(bface.tet);
                    if !self.ishulltet(bface.tet) {
                        let mut nbf = BadFace::default();
                        // (Upstream ignores get_tetqual's result here.)
                        self.get_tetqual(Some(bface), &mut nbf);
                        self.enqueue_if_bad(q, &mut nbf);
                    }
                }
            }
        }

        // Check if the bad quality tet is removed or not.
        let (a, b, c, d) = (bf.forg, bf.fdest, bf.fapex, bf.foppo);
        if self.get_tet(a, b, c, d, &mut bf.tt)? {
            // Try to remove it.
            if self.repair_tet(q, bf, true, false, false)? {
                return Ok(true);
            }
        } else {
            // This tet is removed.
            return Ok(true);
        }
        Ok(false) // not added.
    }

    /// `repair_badqual_tets(bFlips, bSmooth, bSteiners)`: repair every
    /// queued tetrahedron; the failed ones are queued again.
    fn repair_badqual_tets(
        &mut self,
        q: &mut BadTetQueue,
        b_flips: bool,
        b_smooth: bool,
        b_steiners: bool,
    ) -> Result<i64, TetgenError> {
        self.reach5[20] += 1;
        let mut repaired_count = 0i64;
        let mut unsplit: Vec<BadFace> = Vec::new();
        while q.items() > 0 {
            // Get a badtet of highest priority.
            let Some(mut bt) = q.top() else {
                return Err(TetgenError::InternalError);
            };
            let (a, b, c, d) = (bt.forg, bt.fdest, bt.fapex, bt.foppo);
            if self.get_tet(a, b, c, d, &mut bt.tt)? {
                if self.repair_tet(q, &mut bt, b_flips, b_smooth, b_steiners)? {
                    repaired_count += 1;
                } else {
                    // Failed to repair this tet. Save it.
                    unsplit.push(bt);
                }
            }
            // Return the badtet to the pool.
            q.dequeue();
        }
        if !unsplit.is_empty() {
            // Re-initialise the priority queue.
            *q = BadTetQueue::new();
            for bt in unsplit.iter() {
                q.enqueue(bt);
            }
        }
        Ok(repaired_count)
    }

    /// `improve_mesh()`: repair the tetrahedra whose aspect ratio or
    /// largest dihedral angle is too large, by flips (link levels 1 to
    /// `opt_max_flip_level`), then by flips, smoothing and Steiner points
    /// (`opt_iterations` rounds), then by flips again.
    pub fn improve_mesh(&mut self) -> Result<(), TetgenError> {
        self.reach5[19] += 1;
        let mut q = BadTetQueue::new();
        self.badtetrahedrons = Some(Vec::new());

        self.cos_large_dihed = (135.0 / 180.0 * PI).cos(); // used in get_tetqual
        self.cosmaxdihed = (self.b.optmaxdihedral / 180.0 * PI).cos(); // set by -o/#
                                                                       // The smallest dihedral angle to identify slivers.
        let mut sliver_ang_tol = self.b.optmaxdihedral - 5.0;
        if sliver_ang_tol < 172.0 {
            sliver_ang_tol = 172.0;
        }
        self.cossmtdihed = (sliver_ang_tol / 180.0 * PI).cos();
        // The smallest dihedral angle to split slivers.
        let mut split_sliver_ang_tol = self.b.optmaxdihedral + 10.0;
        if split_sliver_ang_tol < 179.0 {
            split_sliver_ang_tol = 179.0;
        } else if split_sliver_ang_tol > 180.0 {
            split_sliver_ang_tol = 179.9;
        }
        self.cosslidihed = (split_sliver_ang_tol / 180.0 * PI).cos();
        self.opt_max_sliver_asp_ratio = self.b.opt_max_asp_ratio * 10.0; // set by -o//#

        // Put all bad tetrahedra into array. (-c is not ported.)
        self.tetpool.traversalinit();
        while let Some(t) = self.tetrahedrontraverse() {
            let checktet = TriFace { tet: t, ver: 0 };
            let mut bf = BadFace::default();
            if self.get_tetqual(Some(&checktet), &mut bf) {
                self.enqueue_if_bad(&mut q, &mut bf);
            } else {
                return Err(TetgenError::InternalError); // a degenerated tet.
            }
        }

        // Backup flip edge options.
        let bakautofliplinklevel = self.autofliplinklevel;
        let bakfliplinklevel = self.b.fliplinklevel;
        let bakmaxflipstarsize = self.b.flipstarsize;

        self.b.fliplinklevel = 1; // initial (<= b->opt_max_flip_level, -O#)
        self.b.flipstarsize = 10; // b->optmaxflipstarsize;

        // Only using flips.
        while q.items() > 0 {
            self.repair_badqual_tets(&mut q, true, false, false)?;
            if self.b.fliplinklevel < self.b.opt_max_flip_level {
                self.b.fliplinklevel += 1;
            } else {
                break; // maximal flip level is reached.
            }
        }

        let mut iter = 0;
        let mut bak_st_count = self.st_volref_count;
        while q.items() > 0 && iter < self.b.opt_iterations {
            let repaired_count = self.repair_badqual_tets(&mut q, true, true, true)?;
            // Break if no repair and no new Steiner point.
            if repaired_count == 0 && bak_st_count == self.st_volref_count {
                break;
            }
            bak_st_count = self.st_volref_count;
            iter += 1;
        }

        // Do last flips.
        if q.items() > 0 {
            self.repair_badqual_tets(&mut q, true, false, false)?;
        }

        if self.later_unflip_queue.len() > self.b.unflip_queue_limit as usize {
            self.later_unflip_queue.clear(); // clean it.
        }

        // Restore original flip edge options.
        self.autofliplinklevel = bakautofliplinklevel;
        self.b.fliplinklevel = bakfliplinklevel;
        self.b.flipstarsize = bakmaxflipstarsize;

        self.badtetrahedrons = None;
        Ok(())
    }
}
