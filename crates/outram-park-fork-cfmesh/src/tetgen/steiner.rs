// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright (C) 2026 OUTRAM PARK contributors
//
// Port of TetGen 1.6.0 (Steiner points in boundary recovery: `tetgen.cxx`
// `insertpoint`, `insertpoint_abort`, `create_a_shorter_edge`,
// `linelineint`, section `steiner_cxx`: `add_steinerpt_in_schoenhardtpoly`,
// `add_steinerpt_in_segment`, `add_steinerpt_to_recover_edge`, the Steiner
// branches of `recoversubfaces`, `getvertexstar`, `reduceedgesatvertex`,
// `removevertexbyflips`, `smoothpoint`; section `surface_cxx`:
// `sremovevertex`; and `lawsonflip3d` / `recoverdelaunay`, which boundary
// recovery calls after every Steiner point):
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

//! Steiner points in boundary recovery (stage 4, gh:#1793).
//!
//! Where flips cannot recover a segment or a subface, `recoverboundary`
//! adds a vertex with `insertpoint` (Bowyer-Watson, the cavity cut back to
//! a star-shaped one that respects the recovered boundary): in the volume
//! of a Schönhardt-like polyhedron around the missing edge
//! (`add_steinerpt_in_schoenhardtpoly`, placed by `smoothpoint`), on the
//! segment near a crossing segment (`add_steinerpt_in_segment`) or where
//! the first crossing face meets it, or on the missing subface. Each
//! insertion is followed by `recoverdelaunay` (Lawson's 3D flips), and the
//! segment and facet Steiner points are afterwards removed again by flips
//! where possible (`removevertexbyflips`).
//!
//! **Branches not ported**, none reached by boundary recovery with
//! upstream's switches (named so a later stage can find them):
//! `insertpoint`'s weighted (`-w`) tests, `rejflag` encroachment queues
//! (`check_encroachment`, `get_subface_ccent`), `check_insert_radius`,
//! `assignmeshsize` (`-m`), `chkencflag` queues, `cdtflag`, `smlenflag`;
//! `smoothpoint`'s aspect-ratio and dihedral objectives (`get_tetqual`);
//! `create_a_shorter_edge`'s facet maps (`facet_ridge_vertex_adjacent`,
//! `segfacetadjacent`: their lists are built only by `-D` and `-q`, so in
//! boundary recovery upstream dereferences `NULL` there; the port returns
//! [`TetgenError::InternalError`] at the same read). **UPDATED 2026-11-26
//! (gh:#1795):** stage 6 ports `-q`: the maps (`super::refine`), the
//! `rejflag` 1 and 2 rejections, `check_insert_radius`, `smlenflag` and the
//! `chkencflag` 1 and 2 queues; the maps are read when built and the
//! `NULL` read still errors when they are not.

#[cfg(not(any(feature = "std", test)))]
#[allow(unused_imports)]
use crate::nostd_prelude::*;

use super::delaunay::{LocateResult, TetgenError};
use super::flip::{
    edestoppo, eorgoppo, senext, senext2, sesym, FlipConstraints, ACROSSEDGE, ACROSSFACE,
    ACROSSVERT, DISJOINT, INTERSECT, SELF_INTERSECT,
};
use super::mesh::{
    decode, encode2, enext, enextesymself, enextself, eprev, eprevself, esym, esymself, sdecode,
    sencode, Face, Point, ShPool, TetgenMesh, TriFace, VertType, DUMMYPOINT, EPIVOT, NULL_POINT,
    NULL_TET,
};
use alloc::string::String;
use alloc::vec::Vec;

/// `insertvertexflags`, every field `insertpoint` reads.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct InsertPointFlags {
    /// `iloc`: the location, in and out.
    pub iloc: LocateResult,
    /// `bowywat`.
    pub bowywat: i32,
    /// `lawson`.
    pub lawson: i32,
    /// `splitbdflag`.
    pub splitbdflag: bool,
    /// `validflag`.
    pub validflag: bool,
    /// `respectbdflag`.
    pub respectbdflag: bool,
    /// `rejflag`: bit 1 rejects a point that encroaches upon a segment
    /// (`ENCSEGMENT`), bit 2 one that encroaches upon a subface
    /// (`ENCSUBFACE`) (Delaunay refinement, stage 6, gh:#1795), bit 4 one
    /// inside half a cavity vertex's mesh size (`ENCVERTEX`, `-m`,
    /// gh:#1825).
    pub rejflag: i32,
    /// `chkencflag`: bit 4 queues the new tetrahedra
    /// (`enqueuetetrahedron`, mesh improvement, gh:#1794); bits 1 and 2
    /// queue the segments and subfaces at the cavity (`enqueuesubface`,
    /// Delaunay refinement, gh:#1795).
    pub chkencflag: i32,
    /// `cdtflag`.
    pub cdtflag: bool,
    /// `sloc`.
    pub sloc: LocateResult,
    /// `sbowywat`.
    pub sbowywat: i32,
    /// `collect_inial_cavity_flag`.
    pub collect_inial_cavity_flag: bool,
    /// `ignore_near_vertex`.
    pub ignore_near_vertex: bool,
    /// `refineflag` (0, 1, 2, 4 or 8).
    pub refineflag: i32,
    /// `refinetet`.
    pub refinetet: TriFace,
    /// `refinesh`.
    pub refinesh: Face,
    /// `check_insert_radius` (Delaunay refinement, stage 6): reject the
    /// point inside a cavity vertex's insertion radius (`ENCVERTEX`).
    pub check_insert_radius: bool,
    /// `smlenflag` (stage 6): return the shortest new edge in `smlen` and
    /// its other vertex in `parentpt`.
    pub smlenflag: bool,
    /// `smlen`.
    pub smlen: f64,
    /// `parentpt` (`NULL_POINT` for `NULL`).
    pub parentpt: Point,
    /// `assignmeshsize` (`-m`, gh:#1825): give the new point a size
    /// interpolated from its location ([`TetgenMesh::getpointmeshsize`]).
    pub assignmeshsize: bool,
}

impl Default for InsertPointFlags {
    /// `insertvertexflags::init()`.
    fn default() -> Self {
        InsertPointFlags {
            iloc: LocateResult::Unknown,
            bowywat: 0,
            lawson: 0,
            splitbdflag: false,
            validflag: false,
            respectbdflag: false,
            rejflag: 0,
            chkencflag: 0,
            cdtflag: false,
            sloc: LocateResult::Unknown,
            sbowywat: 0,
            collect_inial_cavity_flag: false,
            ignore_near_vertex: false,
            refineflag: 0,
            refinetet: TriFace::default(),
            refinesh: Face::default(),
            check_insert_radius: false,
            smlenflag: false,
            smlen: 0.0,
            parentpt: NULL_POINT,
            assignmeshsize: false,
        }
    }
}

/// `optparameters` (the volume objective only).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct OptParameters {
    /// `max_min_volume`.
    pub max_min_volume: bool,
    /// `initval`.
    pub initval: f64,
    /// `imprval`.
    pub imprval: f64,
    /// `numofsearchdirs`.
    pub numofsearchdirs: i32,
    /// `searchstep`.
    pub searchstep: f64,
    /// `maxiter`.
    pub maxiter: i32,
    /// `smthiter`.
    pub smthiter: i32,
}

impl Default for OptParameters {
    fn default() -> Self {
        OptParameters {
            max_min_volume: false,
            initval: 0.0,
            imprval: 0.0,
            numofsearchdirs: 10,
            searchstep: 0.01,
            maxiter: -1,
            smthiter: 0,
        }
    }
}

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
/// `tetgenmesh::orient3dfast` (the plain determinant).
#[inline]
fn orient3dfast(pa: &[f64; 3], pb: &[f64; 3], pc: &[f64; 3], pd: &[f64; 3]) -> f64 {
    let adx = pa[0] - pd[0];
    let bdx = pb[0] - pd[0];
    let cdx = pc[0] - pd[0];
    let ady = pa[1] - pd[1];
    let bdy = pb[1] - pd[1];
    let cdy = pc[1] - pd[1];
    let adz = pa[2] - pd[2];
    let bdz = pb[2] - pd[2];
    let cdz = pc[2] - pd[2];
    adx * (bdy * cdz - bdz * cdy) + bdx * (cdy * adz - cdz * ady) + cdx * (ady * bdz - adz * bdy)
}

impl TetgenMesh {
    #[inline]
    fn x(&self, p: Point) -> [f64; 3] {
        self.points[p].xyz
    }

    /// The number of `flippool` items (the tetrahedral and surface flip
    /// stacks).
    #[inline]
    pub fn flippool_items(&self) -> usize {
        self.tflipstack.len() + self.flipstack.len()
    }

    /// `create_a_shorter_edge(steinerpt, nearpt)`: may the Steiner point be
    /// inserted closer than `minedgelength` to `nearpt`.
    pub fn create_a_shorter_edge(
        &self,
        steinerpt: Point,
        nearpt: Point,
    ) -> Result<bool, TetgenError> {
        let nearpt_type = self.pointtype(nearpt);
        let steiner_type = self.pointtype(steinerpt);
        let mut createflag = false;
        if nearpt_type == VertType::Ridge {
            if steiner_type == VertType::FreeSeg {
                let parentseg = sdecode(self.point2sh(steinerpt));
                let segidx = self.getfacetindex(&parentseg) as usize;
                let pa = self.segmentendpointslist[segidx * 2];
                let pb = self.segmentendpointslist[segidx * 2 + 1];
                if pa != nearpt && pb != nearpt {
                    createflag = true;
                }
            } else if steiner_type == VertType::FreeFacet {
                // facet_ridge_vertex_adjacent(): its list is built by -q
                //   (stage 6, gh:#1795); NULL in boundary recovery.
                let parentsh = sdecode(self.point2sh(steinerpt));
                if !self.facet_ridge_vertex_adjacent(&parentsh, nearpt)? {
                    createflag = true;
                }
            }
        } else if nearpt_type == VertType::FreeSeg {
            if steiner_type == VertType::FreeSeg {
                let seg1 = sdecode(self.point2sh(steinerpt));
                let seg2 = sdecode(self.point2sh(nearpt));
                if self.getfacetindex(&seg1) != self.getfacetindex(&seg2) {
                    createflag = true;
                }
            } else if steiner_type == VertType::FreeFacet {
                // segfacetadjacent(): its list is built by -q (stage 6).
                let paresntsh = sdecode(self.point2sh(steinerpt));
                let parentseg = sdecode(self.point2sh(nearpt));
                if !self.segfacetadjacent(&parentseg, &paresntsh)? {
                    createflag = true;
                }
            }
        } else if nearpt_type == VertType::FreeFacet {
            if steiner_type == VertType::FreeSeg {
                let paresntsh = sdecode(self.point2sh(nearpt));
                let parentseg = sdecode(self.point2sh(steinerpt));
                if !self.segfacetadjacent(&parentseg, &paresntsh)? {
                    createflag = true;
                }
            } else if steiner_type == VertType::FreeFacet {
                let s1 = sdecode(self.point2sh(nearpt));
                let s2 = sdecode(self.point2sh(steinerpt));
                if self.getfacetindex(&s1) != self.getfacetindex(&s2) {
                    createflag = true;
                }
            }
        }
        Ok(createflag)
    }

    /// `insertpoint_abort(splitseg, ivf)`: restore the cavity.
    pub fn insertpoint_abort(&mut self, splitseg: Option<&Face>, ivf: &InsertPointFlags) {
        for i in 0..self.caveoldtetlist.len() {
            let t = self.caveoldtetlist[i].tet;
            self.uninfect(t);
            self.unmarktest(t);
        }
        for i in 0..self.cavebdrylist.len() {
            let t = self.cavebdrylist[i].tet;
            self.unmarktest(t);
        }
        self.cavetetlist.clear();
        self.cavebdrylist.clear();
        self.caveoldtetlist.clear();
        self.cavetetseglist.clear();
        self.cavetetshlist.clear();
        if ivf.splitbdflag {
            if let Some(sg) = splitseg {
                if !sg.is_null() {
                    self.sunmarktest(sg);
                }
            }
            for i in 0..self.caveshlist.len() {
                let s = self.caveshlist[i];
                self.sunmarktest(&s);
            }
            self.caveshlist.clear();
            self.cavesegshlist.clear();
        }
    }

    /// Push the face (with `EPIVOT` version) of the neighbour at face `i` of
    /// `t` on `cavebdrylist`.
    fn push_bdry_nbr(&mut self, tet: usize, i: usize) {
        let mut neightet = decode(self.tets[tet].nbr[i]);
        neightet.ver = EPIVOT[neightet.ver];
        self.cavebdrylist.push(neightet);
    }

    /// `insertpoint(insertpt, searchtet, splitsh, splitseg, ivf)`: insert a
    /// vertex by the Bowyer-Watson algorithm (the cavity validated against
    /// the boundary), splitting `splitsh` / `splitseg` if given. True if
    /// inserted; `ivf.iloc` returns the location or the reason.
    pub fn insertpoint(
        &mut self,
        insertpt: Point,
        searchtet: &mut TriFace,
        mut splitsh: Option<&mut Face>,
        mut splitseg: Option<&mut Face>,
        ivf: &mut InsertPointFlags,
    ) -> Result<bool, TetgenError> {
        self.reach[22] += 1;
        let mut loc = LocateResult::Outside;

        // Locate the point.
        if searchtet.tet != NULL_TET {
            loc = ivf.iloc;
        }
        if loc == LocateResult::Outside {
            if searchtet.tet == NULL_TET {
                self.randomsample(insertpt, searchtet);
            }
            // Locate the point.
            loc = self.locate(insertpt, searchtet, false)?;
        }
        ivf.iloc = loc; // The return value.

        // Create the initial cavity C(p).
        match loc {
            LocateResult::Outside | LocateResult::InTetrahedron => {
                self.flip14count += 1;
                for i in 0..4 {
                    self.push_bdry_nbr(searchtet.tet, i);
                }
                self.infect(searchtet.tet);
                self.caveoldtetlist.push(*searchtet);
            }
            LocateResult::OnFace => {
                self.flip26count += 1;
                let j = searchtet.ver & 3;
                for i in 1..4 {
                    self.push_bdry_nbr(searchtet.tet, (j + i) % 4);
                }
                let spintet = decode(self.tets[searchtet.tet].nbr[j]);
                let j = spintet.ver & 3;
                for i in 1..4 {
                    self.push_bdry_nbr(spintet.tet, (j + i) % 4);
                }
                self.infect(spintet.tet);
                self.caveoldtetlist.push(spintet);
                self.infect(searchtet.tet);
                self.caveoldtetlist.push(*searchtet);

                if ivf.splitbdflag {
                    if let Some(sh) = splitsh.as_deref() {
                        if !sh.is_null() {
                            self.smarktest(sh);
                            self.caveshlist.push(*sh);
                        }
                    }
                }
            }
            LocateResult::OnEdge => {
                self.flipn2ncount += 1;
                let mut spintet = *searchtet;
                loop {
                    let n1 = eorgoppo(&spintet);
                    let mut neightet = decode(self.tets[n1.tet].nbr[n1.ver & 3]);
                    neightet.ver = EPIVOT[neightet.ver];
                    self.cavebdrylist.push(neightet);
                    let n2 = edestoppo(&spintet);
                    let mut neightet = decode(self.tets[n2.tet].nbr[n2.ver & 3]);
                    neightet.ver = EPIVOT[neightet.ver];
                    self.cavebdrylist.push(neightet);
                    self.infect(spintet.tet);
                    self.caveoldtetlist.push(spintet);
                    self.fnextself(&mut spintet);
                    if spintet.tet == searchtet.tet {
                        break;
                    }
                }

                if ivf.splitbdflag {
                    if let Some(sg) = splitseg.as_deref_mut() {
                        if !sg.is_null() {
                            self.smarktest(sg);
                            sg.shver = 0;
                            let s = self.spivot(sg);
                            if let Some(sh) = splitsh.as_deref_mut() {
                                *sh = s;
                            }
                        }
                    }
                    if let Some(sh) = splitsh.as_deref() {
                        if !sh.is_null() {
                            // Collect all subfaces share at this edge.
                            let pa = self.sorg(sh);
                            let mut neighsh = *sh;
                            loop {
                                if self.sorg(&neighsh) != pa {
                                    neighsh = sesym(&neighsh);
                                }
                                self.smarktest(&neighsh);
                                self.caveshlist.push(neighsh);
                                self.cavesegshlist.push(neighsh);
                                neighsh = self.spivot(&neighsh);
                                if neighsh.same_sh(sh) {
                                    break;
                                }
                                if neighsh.is_null() {
                                    break;
                                }
                            }
                        }
                    }
                }
            }
            LocateResult::InStar => {
                self.reach[43] += 1;
                // All tets in the star are given in 'caveoldtetlist', infected.
                if self.cavebdrylist.is_empty() {
                    for i in 0..self.caveoldtetlist.len() {
                        let cavetet = self.caveoldtetlist[i];
                        for j in 0..4 {
                            let mut neightet = decode(self.tets[cavetet.tet].nbr[j]);
                            if !self.infected(neightet.tet) {
                                neightet.ver = EPIVOT[neightet.ver];
                                self.cavebdrylist.push(neightet);
                            }
                        }
                    }
                }
            }
            LocateResult::OnVertex => {
                // The point already exist. Do nothing and return.
                return Ok(false);
            }
            LocateResult::EncSubface => {
                ivf.iloc = LocateResult::EncSubface;
                return Ok(false);
            }
            _ => return Err(TetgenError::InternalError),
        }

        if ivf.collect_inial_cavity_flag {
            for i in 0..self.caveoldtetlist.len() {
                let t = self.caveoldtetlist[i].tet;
                self.cave_oldtet_list.push(t);
            }
            // Do not insert this point.
            self.insertpoint_abort(splitseg.as_deref(), ivf);
            return Ok(false);
        }

        if loc != LocateResult::InStar {
            // (b->plc) Reject the new point if it lies too close to an
            //   existing point. Collect the vertices of the initial cavity.
            let v = self.tets[searchtet.tet].v;
            if loc == LocateResult::Outside {
                for p in v.iter().take(3) {
                    self.cavetetvertlist.push(*p);
                }
            } else if loc == LocateResult::InTetrahedron {
                for p in v.iter() {
                    self.cavetetvertlist.push(*p);
                }
            } else if loc == LocateResult::OnFace {
                for p in v.iter().take(3) {
                    self.cavetetvertlist.push(*p);
                }
                if v[3] != DUMMYPOINT {
                    self.cavetetvertlist.push(v[3]);
                }
                let spintet = self.fsym(searchtet);
                if self.oppo(&spintet) != DUMMYPOINT {
                    let o = self.oppo(&spintet);
                    self.cavetetvertlist.push(o);
                }
            } else if loc == LocateResult::OnEdge {
                let mut spintet = *searchtet;
                let (o, d) = (self.org(&spintet), self.dest(&spintet));
                self.cavetetvertlist.push(o);
                self.cavetetvertlist.push(d);
                loop {
                    if self.apex(&spintet) != DUMMYPOINT {
                        let a = self.apex(&spintet);
                        self.cavetetvertlist.push(a);
                    }
                    self.fnextself(&mut spintet);
                    if spintet.tet == searchtet.tet {
                        break;
                    }
                }
            }

            let mut pts = NULL_POINT;
            let xi = self.x(insertpt);
            for i in 0..self.cavetetvertlist.len() {
                let p = self.cavetetvertlist[i];
                let rd = distance(&self.x(p), &xi);
                // Is the point very close to an existing point?
                if rd < self.minedgelength
                    && !self.create_a_shorter_edge(insertpt, p)?
                    && !ivf.ignore_near_vertex
                {
                    pts = p;
                    loc = LocateResult::NearVertex;
                    break;
                }
                if ivf.check_insert_radius {
                    // (Delaunay refinement, stage 6, gh:#1795.)
                    let ins_radius = self.points[p].insradius;
                    if ins_radius > 0.0
                        && rd < ins_radius
                        && !self.create_a_shorter_edge(insertpt, p)?
                    {
                        // Reject the insertion of this vertex.
                        self.reach6[35] += 1;
                        pts = p;
                        loc = LocateResult::EncVertex;
                        break;
                    }
                }
                if ivf.rejflag & 4 != 0 {
                    // Is the point encroaches upon an existing point (-m,
                    //   gh:#1825)?
                    if rd < 0.5 * self.points[p].mtr {
                        self.reachm[6] += 1;
                        pts = p;
                        loc = LocateResult::EncVertex;
                        break;
                    }
                }
            }
            self.cavetetvertlist.clear();

            if pts != NULL_POINT {
                // The point is too close to an existing vertex (NEARVERTEX),
                //   or inside its insertion radius (ENCVERTEX, stage 6).
                *searchtet = self.point2tetorg(pts);
                self.insertpoint_abort(splitseg.as_deref(), ivf);
                ivf.iloc = loc;
                return Ok(false);
            }
        }

        if ivf.assignmeshsize {
            // Assign mesh size for the new point (-m, gh:#1825; no
            //   background mesh).
            self.reachm[14] += 1;
            let size = self.getpointmeshsize(insertpt, searchtet, loc);
            self.points[insertpt].mtr = size;
        }

        if ivf.bowywat != 0 {
            // Update the cavity C(p) using the Bowyer-Watson algorithm.
            core::mem::swap(&mut self.cavetetlist, &mut self.cavebdrylist);
            let mut i = 0;
            while i < self.cavetetlist.len() {
                // 'cavetet' is an adjacent tet at outside of the cavity.
                let mut cavetet = self.cavetetlist[i];
                if !self.infected(cavetet.tet) {
                    let mut enqflag = false;
                    if !self.marktested(cavetet.tet) {
                        // Do Delaunay (in-sphere) test.
                        let pts = self.tets[cavetet.tet].v;
                        if pts[3] != DUMMYPOINT {
                            let sign = self.insphere_s(pts[0], pts[1], pts[2], pts[3], insertpt)?;
                            enqflag = sign < 0.0;
                        } else if !self.nonconvex {
                            // Test if this hull face is visible by the new point.
                            let ori = self.ori3(pts[0], pts[1], pts[2], insertpt);
                            if ori < 0.0 {
                                // A visible hull face.
                                enqflag = true;
                            } else if ori == 0.0 {
                                // A coplanar hull face.
                                let neineitet = decode(self.tets[cavetet.tet].nbr[3]);
                                if !self.infected(neineitet.tet) {
                                    if !self.marktested(neineitet.tet) {
                                        let p2 = self.tets[neineitet.tet].v;
                                        let sign =
                                            self.insphere_s(p2[0], p2[1], p2[2], p2[3], insertpt)?;
                                        enqflag = sign < 0.0;
                                    }
                                } else {
                                    enqflag = true;
                                }
                            }
                        } else {
                            // A hull face (must be a subface).
                            let neineitet = decode(self.tets[cavetet.tet].nbr[3]);
                            if !self.infected(neineitet.tet) {
                                if !self.marktested(neineitet.tet) {
                                    let p2 = self.tets[neineitet.tet].v;
                                    let sign =
                                        self.insphere_s(p2[0], p2[1], p2[2], p2[3], insertpt)?;
                                    enqflag = sign < 0.0;
                                }
                            } else {
                                enqflag = true;
                            }
                        }
                        self.marktest(cavetet.tet); // Only test it once.
                    }

                    if enqflag {
                        // Found a tet in the cavity. Put other three faces in check list.
                        let k = cavetet.ver & 3;
                        for j in 1..4 {
                            let neightet = decode(self.tets[cavetet.tet].nbr[(j + k) % 4]);
                            self.cavetetlist.push(neightet);
                        }
                        self.infect(cavetet.tet);
                        self.caveoldtetlist.push(cavetet);
                    } else {
                        // Found a boundary face of the cavity.
                        cavetet.ver = EPIVOT[cavetet.ver];
                        self.cavetetlist[i] = cavetet;
                        self.cavebdrylist.push(cavetet);
                    }
                }
                i += 1;
            }
            self.cavetetlist.clear();
        }

        if ivf.refineflag > 0 && ivf.refineflag == 1 && !self.infected(ivf.refinetet.tet) {
            self.insertpoint_abort(splitseg.as_deref(), ivf);
            ivf.iloc = LocateResult::BadElement;
            return Ok(false);
        }

        if self.checksubsegflag {
            // Collect all segments of C(p).
            for i in 0..self.caveoldtetlist.len() {
                let cavetet = self.caveoldtetlist[i];
                let a = self.tets[cavetet.tet].seg;
                if a != usize::MAX {
                    for j in 0..6 {
                        let e = self.tet2seg[a][j];
                        if e != 0 {
                            let checkseg = sdecode(e);
                            if !self.sinfected(&checkseg) {
                                self.sinfect(&checkseg);
                                self.cavetetseglist.push(checkseg);
                            }
                        }
                    }
                }
            }
            // Uninfect collected segments.
            for i in 0..self.cavetetseglist.len() {
                let s = self.cavetetseglist[i];
                self.suninfect(&s);
            }
            if ivf.rejflag & 1 != 0 && self.reject_encroached_segments(insertpt) {
                // It encroaches upon a segment (stage 6, gh:#1795).
                self.insertpoint_abort(splitseg.as_deref(), ivf);
                ivf.iloc = LocateResult::EncSegment;
                return Ok(false);
            }
        }

        if self.checksubfaceflag {
            // Collect all subfaces of C(p).
            for i in 0..self.caveoldtetlist.len() {
                let cavetet = self.caveoldtetlist[i];
                let a = self.tets[cavetet.tet].sub;
                if a != usize::MAX {
                    for j in 0..4 {
                        let e = self.tet2sub[a][j];
                        if e != 0 {
                            let checksh = sdecode(e);
                            if !self.sinfected(&checksh) {
                                self.sinfect(&checksh);
                                self.cavetetshlist.push(checksh);
                            }
                        }
                    }
                }
            }
            for i in 0..self.cavetetshlist.len() {
                let s = self.cavetetshlist[i];
                self.suninfect(&s);
            }
            if ivf.rejflag & 2 != 0 && self.reject_encroached_subfaces(insertpt)? {
                // It encroaches upon a subface (stage 6, gh:#1795).
                self.insertpoint_abort(splitseg.as_deref(), ivf);
                ivf.iloc = LocateResult::EncSubface;
                return Ok(false);
            }
        }

        if ivf.iloc == LocateResult::Outside && ivf.refineflag != 0 {
            // The vertex lies outside of the domain. Do not insert it.
            self.insertpoint_abort(splitseg.as_deref(), ivf);
            return Ok(false);
        }

        if ivf.splitbdflag {
            // The new point locates in surface mesh. Update the sC(p).
            let mut i = 0;
            while i < self.caveshlist.len() {
                let mut checksh = self.caveshlist[i];
                for _ in 0..3 {
                    if !self.isshsubseg(&checksh) {
                        let neighsh = self.spivot(&checksh);
                        if !self.smarktested(&neighsh) {
                            let mut neightet = self.stpivot(&neighsh);
                            if self.infected(neightet.tet) {
                                self.fsymself(&mut neightet);
                                if self.infected(neightet.tet) {
                                    // This subface is inside C(p).
                                    let sign = self.incircle3d(
                                        self.sorg(&neighsh),
                                        self.sdest(&neighsh),
                                        self.sapex(&neighsh),
                                        insertpt,
                                    );
                                    if sign < 0.0 {
                                        self.smarktest(&neighsh);
                                        self.caveshlist.push(neighsh);
                                    }
                                }
                            }
                        }
                    }
                    checksh = senext(&checksh);
                }
                i += 1;
            }
        }

        if ivf.validflag {
            // Validate C(p) and update it if it is not star-shaped.
            let mut cutcount = 0;

            if ivf.respectbdflag {
                // The initial cavity may include subfaces which are not on the
                //   facets being splitting. Make them boundary of C(p).
                for i in 0..self.cavetetshlist.len() {
                    let parysh = self.cavetetshlist[i];
                    let mut neightet = self.stpivot(&parysh);
                    if self.infected(neightet.tet) {
                        self.fsymself(&mut neightet);
                        if self.infected(neightet.tet) && !self.smarktested(&parysh) {
                            // Found a subface inside C(p). Check if it is a hull face.
                            if self.oppo(&neightet) != DUMMYPOINT {
                                self.fsymself(&mut neightet);
                            }
                            let mut ori;
                            if self.oppo(&neightet) != DUMMYPOINT {
                                ori = self.ori3(
                                    self.org(&neightet),
                                    self.dest(&neightet),
                                    self.apex(&neightet),
                                    insertpt,
                                );
                                if ori < 0.0 {
                                    // A visible face, get its neighbor face.
                                    self.fsymself(&mut neightet);
                                    ori = -ori; // It must be invisible by p.
                                }
                            } else {
                                // A hull tet. It needs to be cut.
                                ori = 1.0;
                            }
                            // Cut this tet if it is either invisible by or coplanar with p.
                            if ori >= 0.0 {
                                self.cut_cavity_tet(&mut neightet, true);
                                cutcount += 1;
                            }
                        }
                    }
                }

                // The initial cavity may include segments in its interior.
                for i in 0..self.cavetetseglist.len() {
                    let paryseg = self.cavetetseglist[i];
                    // Check this segment if it is not a splitting segment.
                    if !self.smarktested(&paryseg) {
                        let neightet = self.sstpivot1(&paryseg);
                        let mut spintet = neightet;
                        loop {
                            if !self.infected(spintet.tet) {
                                break;
                            }
                            self.fnextself(&mut spintet);
                            if spintet.tet == neightet.tet {
                                break;
                            }
                        }
                        if self.infected(spintet.tet) {
                            // Find an adjacent tet at this segment such that both faces
                            //   at this segment are not visible by p.
                            let pa = self.org(&neightet);
                            let pb = self.dest(&neightet);
                            let mut spintet = neightet;
                            let mut found = false;
                            loop {
                                // Check if this face is visible by p.
                                let pc = self.apex(&spintet);
                                if pc != DUMMYPOINT {
                                    let ori = self.ori3(pa, pb, pc, insertpt);
                                    if ori >= 0.0 {
                                        // Not visible. Check another face in this tet.
                                        let neineitet = esym(&spintet);
                                        let pc = self.apex(&neineitet);
                                        if pc != DUMMYPOINT {
                                            let ori = self.ori3(pb, pa, pc, insertpt);
                                            if ori >= 0.0 {
                                                // Not visible. Found this face.
                                                found = true;
                                                break;
                                            }
                                        }
                                    }
                                }
                                self.fnextself(&mut spintet);
                                if spintet.tet == neightet.tet {
                                    break;
                                }
                            }
                            if !found {
                                return Err(TetgenError::InternalError);
                            }
                            let mut neightet = spintet;
                            self.cut_cavity_tet(&mut neightet, true);
                            cutcount += 1;
                        }
                    }
                }
            }

            // Update the cavity by removing invisible faces until it is star-shaped.
            let mut i = 0;
            while i < self.cavebdrylist.len() {
                let cavetet = self.cavebdrylist[i];
                // 'cavetet' is an exterior tet adjacent to the cavity.
                let mut neightet = self.fsym(&cavetet);
                if self.infected(neightet.tet) {
                    let enqflag;
                    if self.apex(&cavetet) != DUMMYPOINT {
                        // It is a cavity boundary face. Check its visibility.
                        if self.oppo(&neightet) != DUMMYPOINT {
                            let ori;
                            if self.issubface(&neightet) {
                                let pa = self.org(&cavetet);
                                let pb = self.dest(&cavetet);
                                let pc = self.apex(&cavetet);
                                let (xa, xb, xc) = (self.x(pa), self.x(pb), self.x(pc));
                                let volume = orient3dfast(&xa, &xb, &xc, &self.x(insertpt));
                                let attrib =
                                    distance(&xa, &xb) * distance(&xb, &xc) * distance(&xc, &xa);
                                if volume.abs() / attrib < self.b.epsilon {
                                    ori = 0.0;
                                } else {
                                    ori = self.ori3(pa, pb, pc, insertpt);
                                }
                            } else {
                                ori = self.ori3(
                                    self.org(&cavetet),
                                    self.dest(&cavetet),
                                    self.apex(&cavetet),
                                    insertpt,
                                );
                            }
                            enqflag = ori > 0.0;
                        } else {
                            // A hull face whose adjacent tet has been cut. Cut it as well.
                            enqflag = false;
                        }
                    } else {
                        enqflag = true; // A hull edge.
                    }
                    if enqflag {
                        // This face is valid, save it.
                        self.cavetetlist.push(cavetet);
                    } else {
                        self.uninfect(neightet.tet);
                        self.unmarktest(neightet.tet);
                        cutcount += 1;
                        // Add three new faces to find new boundaries.
                        for _ in 0..3 {
                            let mut neineitet = esym(&neightet);
                            neineitet.ver = EPIVOT[neineitet.ver];
                            self.cavebdrylist.push(neineitet);
                            enextself(&mut neightet);
                        }
                        // 'cavetet' is not on the cavity boundary anymore.
                        self.unmarktest(cavetet.tet);
                    }
                } else {
                    // 'cavetet' is not on the cavity boundary anymore.
                    self.unmarktest(cavetet.tet);
                }
                i += 1;
            }

            if cutcount > 0 {
                self.reach[44] += 1;
                // The cavity has been updated. Update the cavity boundary faces.
                self.cavebdrylist.clear();
                for i in 0..self.cavetetlist.len() {
                    let cavetet = self.cavetetlist[i];
                    let neightet = self.fsym(&cavetet);
                    if self.infected(neightet.tet) {
                        self.cavebdrylist.push(cavetet);
                    } else {
                        self.unmarktest(cavetet.tet);
                    }
                }

                // Update the list of old tets.
                self.cavetetlist.clear();
                for i in 0..self.caveoldtetlist.len() {
                    let cavetet = self.caveoldtetlist[i];
                    if self.infected(cavetet.tet) {
                        self.cavetetlist.push(cavetet);
                    }
                }
                // Swap 'cavetetlist' and 'caveoldtetlist'.
                core::mem::swap(&mut self.caveoldtetlist, &mut self.cavetetlist);

                // The cavity should contain at least one tet.
                if self.caveoldtetlist.is_empty() {
                    self.insertpoint_abort(splitseg.as_deref(), ivf);
                    ivf.iloc = LocateResult::NullCavity;
                    return Ok(false);
                }

                if ivf.splitbdflag {
                    let mut cutshcount = 0;
                    // Update the sub-cavity sC(p).
                    let mut i: isize = 0;
                    while (i as usize) < self.caveshlist.len() {
                        let parysh = self.caveshlist[i as usize];
                        if self.smarktested(&parysh) {
                            let mut enqflag = false;
                            let mut neightet = self.stpivot(&parysh);
                            if self.infected(neightet.tet) {
                                self.fsymself(&mut neightet);
                                if self.infected(neightet.tet) {
                                    enqflag = true;
                                }
                            }
                            if !enqflag {
                                self.sunmarktest(&parysh);
                                // Use the last entry of this array to fill this entry.
                                let j = self.caveshlist.len() - 1;
                                self.caveshlist[i as usize] = self.caveshlist[j];
                                cutshcount += 1;
                                self.caveshlist.pop();
                                i -= 1;
                            }
                        }
                        i += 1;
                    }

                    if cutshcount > 0 {
                        let mut bad = 0;
                        if loc == LocateResult::OnFace {
                            if let Some(sh) = splitsh.as_deref() {
                                if !sh.is_null() && !self.smarktested(sh) {
                                    bad += 1;
                                }
                            }
                        } else if loc == LocateResult::OnEdge {
                            if let Some(sg) = splitseg.as_deref() {
                                if !sg.is_null() && !self.smarktested(sg) {
                                    bad += 1;
                                }
                            }
                            if let Some(sh) = splitsh.as_deref() {
                                if !sh.is_null() {
                                    let pa = self.sorg(sh);
                                    let mut neighsh = *sh;
                                    loop {
                                        if self.sorg(&neighsh) != pa {
                                            neighsh = sesym(&neighsh);
                                        }
                                        if !self.smarktested(&neighsh) {
                                            bad += 1;
                                        }
                                        neighsh = self.spivot(&neighsh);
                                        if neighsh.same_sh(sh) {
                                            break;
                                        }
                                        if neighsh.is_null() {
                                            break;
                                        }
                                    }
                                }
                            }
                        }
                        if bad > 0 {
                            self.insertpoint_abort(splitseg.as_deref(), ivf);
                            ivf.iloc = LocateResult::NullCavity;
                            return Ok(false);
                        }
                    }
                }
            }
        }

        if ivf.refineflag != 0 {
            if (ivf.refineflag == 1 && !self.infected(ivf.refinetet.tet))
                || (ivf.refineflag == 2 && !self.smarktested(&ivf.refinesh))
            {
                self.insertpoint_abort(splitseg.as_deref(), ivf);
                ivf.iloc = LocateResult::BadElement;
                return Ok(false);
            } else {
                // Boundary recovery: reject this point if the crossing face
                //   (refineflag 4) or edge (8) survives.
                let mut bflag = false;
                if ivf.refineflag == 4 {
                    let adjtet = self.fsym(&ivf.refinetet);
                    if !self.infected(ivf.refinetet.tet) || !self.infected(adjtet.tet) {
                        bflag = true;
                    }
                } else if ivf.refineflag == 8 {
                    let mut spintet = ivf.refinetet;
                    loop {
                        if !self.infected(spintet.tet) {
                            bflag = true;
                            break;
                        }
                        self.fnextself(&mut spintet);
                        if spintet.tet == ivf.refinetet.tet {
                            break;
                        }
                    }
                }
                if bflag {
                    self.insertpoint_abort(splitseg.as_deref(), ivf);
                    ivf.iloc = LocateResult::BadElement;
                    return Ok(false);
                }
            }
        }

        if let Some(sg) = splitseg.as_deref() {
            if !sg.is_null() {
                // A segment will be split. It muts lie inside of the cavity.
                let neightet = self.sstpivot1(sg);
                if neightet.tet != NULL_TET {
                    let mut bflag = false;
                    let mut spintet = neightet;
                    loop {
                        if !self.infected(spintet.tet) {
                            bflag = true;
                            break;
                        }
                        self.fnextself(&mut spintet);
                        if spintet.tet == neightet.tet {
                            break;
                        }
                    }
                    if bflag {
                        self.insertpoint_abort(splitseg.as_deref(), ivf);
                        ivf.iloc = LocateResult::BadElement;
                        return Ok(false);
                    }
                }
            }
        }

        if ivf.cdtflag || ivf.smlenflag || ivf.validflag {
            // There may be other vertices inside C(p). Collect all vertices of C(p).
            for i in 0..self.caveoldtetlist.len() {
                let cavetet = self.caveoldtetlist[i];
                let pts = self.tets[cavetet.tet].v;
                for p in pts {
                    if p != DUMMYPOINT && !self.pinfected(p) {
                        self.pinfect(p);
                        self.cavetetvertlist.push(p);
                    }
                }
            }
            for i in 0..self.cavetetvertlist.len() {
                let p = self.cavetetvertlist[i];
                self.puninfect(p);
            }
            if ivf.smlenflag {
                // Get the length of the shortest edge connecting to 'newpt'
                //   (stage 6, gh:#1795).
                let xi = self.x(insertpt);
                let p0 = self.cavetetvertlist[0];
                ivf.smlen = distance(&self.x(p0), &xi);
                ivf.parentpt = p0;
                for i in 1..self.cavetetvertlist.len() {
                    let p = self.cavetetvertlist[i];
                    let len = distance(&self.x(p), &xi);
                    if len < ivf.smlen {
                        ivf.smlen = len;
                        ivf.parentpt = p;
                    }
                }
            }
        }

        if ivf.cdtflag {
            for i in 0..self.caveoldtetlist.len() {
                let t = self.caveoldtetlist[i].tet;
                self.unmarktest(t);
            }
            for i in 0..self.cavebdrylist.len() {
                let t = self.cavebdrylist[i].tet;
                self.unmarktest(t);
            }
            self.cavetetlist.clear();
            if self.checksubsegflag {
                self.cavetetseglist.clear();
            }
            if self.checksubfaceflag {
                self.cavetetshlist.clear();
            }
            ivf.iloc = LocateResult::InStar;
            return Ok(true);
        }

        // Process the segments and subfaces on the boundary of C(p).
        if self.checksubsegflag {
            let mut i: isize = 0;
            while (i as usize) < self.cavetetseglist.len() {
                let paryseg = self.cavetetseglist[i as usize];
                if !self.smarktested(&paryseg) {
                    let mut j = 0;
                    let mut k = 0;
                    let neightet = self.sstpivot1(&paryseg);
                    let mut neineitet = TriFace::default();
                    let mut spintet = neightet;
                    loop {
                        j += 1;
                        if !self.infected(spintet.tet) {
                            neineitet = spintet; // An outer tet. Remember it.
                        } else {
                            k += 1; // An in tet.
                        }
                        self.fnextself(&mut spintet);
                        if spintet.tet == neightet.tet {
                            break;
                        }
                    }
                    if k == 0 {
                        // The segment is not connect to C(p) anymore.
                        let s = self.cavetetseglist.len() - 1;
                        self.cavetetseglist[i as usize] = self.cavetetseglist[s];
                        self.cavetetseglist.pop();
                        i -= 1;
                    } else if k < j {
                        // The segment is on the boundary of C(p).
                        self.sstbond1(&paryseg, &neineitet);
                    } else if !ivf.splitbdflag {
                        // The segment is inside C(p).
                        self.sinfect(&paryseg); // Flag it as an interior segment.
                        self.caveencseglist.push(paryseg);
                    } else {
                        return Err(TetgenError::InternalError);
                    }
                } else {
                    // Flag it as an interior segment. Do not queue it.
                    self.sinfect(&paryseg);
                }
                i += 1;
            }
        }

        if self.checksubfaceflag {
            let mut i: isize = 0;
            while (i as usize) < self.cavetetshlist.len() {
                let mut parysh = self.cavetetshlist[i as usize];
                if !self.smarktested(&parysh) {
                    // Check if this subface is inside the cavity.
                    let mut k = 0;
                    let mut checksh = Face::default();
                    for _ in 0..2 {
                        let neightet = self.stpivot(&parysh);
                        if !self.infected(neightet.tet) {
                            checksh = parysh; // Remember this side.
                        } else {
                            k += 1;
                        }
                        parysh = sesym(&parysh);
                    }
                    self.cavetetshlist[i as usize] = parysh;
                    if k == 0 {
                        // The subface is not connected to C(p). Remove it.
                        let s = self.cavetetshlist.len() - 1;
                        self.cavetetshlist[i as usize] = self.cavetetshlist[s];
                        self.cavetetshlist.pop();
                        i -= 1;
                    } else if k == 1 {
                        // This side is the outer boundary of C(p).
                        self.cavetetshlist[i as usize] = checksh;
                    } else if !ivf.splitbdflag {
                        self.sinfect(&parysh); // Flag it.
                        self.caveencshlist.push(parysh);
                    } else {
                        return Err(TetgenError::InternalError);
                    }
                } else {
                    // Flag it as an interior subface. Do not queue it.
                    self.sinfect(&parysh);
                }
                i += 1;
            }
        }

        // Create new tetrahedra to fill the cavity.
        let mut newtet = TriFace::default();
        for i in 0..self.cavebdrylist.len() {
            let neightet = self.cavebdrylist[i];
            self.unmarktest(neightet.tet); // Unmark it.
                                           // Get the oldtet (inside the cavity).
            let oldtet = self.fsym(&neightet);
            if self.apex(&neightet) != DUMMYPOINT {
                // Create a new tet in the cavity.
                newtet = self.maketetrahedron();
                let (o, d, a) = (
                    self.org(&neightet),
                    self.dest(&neightet),
                    self.apex(&neightet),
                );
                self.setvertices(&newtet, d, o, a, insertpt);
            } else {
                // Create a new hull tet.
                self.hullsize += 1;
                newtet = self.maketetrahedron();
                let (o, d) = (self.org(&neightet), self.dest(&neightet));
                self.setvertices(&newtet, o, d, insertpt, DUMMYPOINT);
                // Adjust back to the cavity bounday face.
                esymself(&mut newtet);
            }
            // The new tet inherits attribtes from the old tet (`-A`, gh:#1794).
            if self.numelemattrib > 0 {
                self.elemattrib[newtet.tet] = self.elemattrib[oldtet.tet];
            }
            // Connect newtet <==> neightet, this also disconnect the old bond.
            self.bond(&newtet, &neightet);
            // oldtet still connects to neightet.
            self.cavebdrylist[i] = oldtet;
        }

        // Set a handle for speeding point location.
        self.recenttet = newtet;
        self.setpoint2tet(insertpt, encode2(newtet.tet, 0));

        // Re-use this list to save new interior cavity faces.
        self.cavetetlist.clear();

        // Connect adjacent new tetrahedra together.
        for i in 0..self.cavebdrylist.len() {
            // cavtet is an oldtet, get the newtet at this face.
            let mut oldtet = self.cavebdrylist[i];
            let neightet = self.fsym(&oldtet);
            let mut newtet = self.fsym(&neightet);
            // Connect the three other faces of this newtet.
            for _ in 0..3 {
                let neightet = esym(&newtet); // Go to the face.
                if self.tets[neightet.tet].nbr[neightet.ver & 3] == 0 {
                    // Find the adjacent face of this newtet.
                    let mut spintet = oldtet;
                    loop {
                        self.fnextself(&mut spintet);
                        if !self.infected(spintet.tet) {
                            break;
                        }
                    }
                    let mut newneitet = self.fsym(&spintet);
                    esymself(&mut newneitet);
                    self.bond(&neightet, &newneitet);
                    if ivf.lawson > 1 {
                        self.cavetetlist.push(neightet);
                    }
                }
                let o = self.org(&newtet);
                self.setpoint2tet(o, encode2(newtet.tet, 0));
                enextself(&mut newtet);
                enextself(&mut oldtet);
            }
            self.cavebdrylist[i] = newtet; // Save the new tet.
        }

        if self.checksubfaceflag {
            // Connect subfaces on the boundary of the cavity to the new tets.
            for i in 0..self.cavetetshlist.len() {
                let parysh = self.cavetetshlist[i];
                if !self.sinfected(&parysh) {
                    let neightet = self.stpivot(&parysh);
                    let spintet = self.fsym(&neightet);
                    let p2 = sesym(&parysh);
                    self.cavetetshlist[i] = p2;
                    self.tsbond(&spintet, &p2);
                }
            }
        }

        if self.checksubsegflag {
            // Connect segments on the boundary of the cavity to the new tets.
            for i in 0..self.cavetetseglist.len() {
                let paryseg = self.cavetetseglist[i];
                if !self.sinfected(&paryseg) {
                    let neightet = self.sstpivot1(&paryseg);
                    let mut spintet = neightet;
                    loop {
                        self.tssbond1(&spintet, &paryseg);
                        self.fnextself(&mut spintet);
                        if spintet.tet == neightet.tet {
                            break;
                        }
                    }
                }
            }
        }

        let has_sh = splitsh.as_deref().is_some_and(|s| !s.is_null());
        let has_seg = splitseg.as_deref().is_some_and(|s| !s.is_null());
        if has_sh || has_seg {
            // Split a subface or a segment.
            let mut tmp = Face::default();
            let sh: &mut Face = match splitsh.as_deref_mut() {
                Some(s) => s,
                None => &mut tmp,
            };
            self.sinsertvertex_full(
                insertpt,
                sh,
                splitseg.as_deref_mut(),
                ivf.sloc,
                ivf.sbowywat,
                false,
            );
        }

        if self.checksubfaceflag {
            if ivf.splitbdflag {
                // Recover new subfaces in C(p).
                for i in 0..self.caveshbdlist.len() {
                    let parysh = self.caveshbdlist[i];
                    let mut checksh = self.spivot(&parysh); // The new subface [a, b, p].
                    if self.shr(&checksh).v[0] != NULL_POINT {
                        let neightet = self.stpivot(&parysh);
                        let mut spintet = neightet;
                        loop {
                            self.fnextself(&mut spintet);
                            if !self.infected(spintet.tet) {
                                break;
                            }
                            if spintet.tet == neightet.tet {
                                return Err(TetgenError::InternalError);
                            }
                        }
                        let neightet = self.fsym(&spintet);
                        let mut spintet = neightet;
                        loop {
                            self.fnextself(&mut spintet);
                            if self.apex(&spintet) == insertpt {
                                break;
                            }
                        }
                        if self.sorg(&checksh) != self.org(&spintet) {
                            checksh = sesym(&checksh);
                        }
                        self.tsbond(&spintet, &checksh);
                        self.fsymself(&mut spintet);
                        checksh = sesym(&checksh);
                        self.tsbond(&spintet, &checksh);
                    }
                }
            } else {
                // The Boundary recovery phase.
                // Put all new subfaces into stack for recovery.
                for i in 0..self.caveshbdlist.len() {
                    let parysh = self.caveshbdlist[i];
                    let checksh = self.spivot(&parysh); // The new subface [a, b, p].
                    if self.shr(&checksh).v[0] != NULL_POINT {
                        self.subfacstack.push(checksh);
                    }
                }
                // Put all interior subfaces into stack for recovery.
                for i in 0..self.caveencshlist.len() {
                    let parysh = self.caveencshlist[i];
                    if !self.smarktested(&parysh) {
                        self.suninfect(&parysh);
                        self.stdissolve(&parysh); // Detach connections to old tets.
                        self.subfacstack.push(parysh);
                    }
                }
            }
        }

        if self.checksubsegflag {
            if ivf.splitbdflag {
                if splitseg.is_some() {
                    // Recover the two new subsegments in C(p).
                    for i in 0..self.cavesegshlist.len() {
                        let mut checkseg = self.cavesegshlist[i];
                        checkseg.shver = 0;
                        let checksh = self.spivot(&checkseg);
                        let mut neightet;
                        if !checksh.is_null() {
                            neightet = self.stpivot(&checksh);
                        } else {
                            // It's a dangling segment.
                            neightet = self.point2tetorg(self.sorg(&checkseg));
                            let d = self.sdest(&checkseg);
                            self.finddirection(&mut neightet, d)?;
                        }
                        if self.isdeadtet(neightet.tet) {
                            return Err(TetgenError::InternalError);
                        }
                        self.sstbond1(&checkseg, &neightet);
                        let mut spintet = neightet;
                        loop {
                            self.tssbond1(&spintet, &checkseg);
                            self.fnextself(&mut spintet);
                            if spintet.tet == neightet.tet {
                                break;
                            }
                        }
                    }
                }
            } else {
                // The Boundary Recovery Phase.
                if splitseg.is_some() {
                    // Queue two new subsegments in C(p) for recovery.
                    for i in 0..self.cavesegshlist.len() {
                        let checkseg = self.cavesegshlist[i];
                        self.subsegstack.push(checkseg);
                    }
                }
                for i in 0..self.caveencseglist.len() {
                    let checkseg = self.caveencseglist[i];
                    if !self.smarktested(&checkseg) {
                        // It may be split.
                        self.suninfect(&checkseg);
                        self.sstdissolve1(&checkseg); // Detach connections to old tets.
                        let s = self.randomnation(self.subsegstack.len() as u32 + 1) as usize;
                        self.subsegstack.push(Face::default());
                        let last = self.subsegstack.len() - 1;
                        self.subsegstack[last] = self.subsegstack[s];
                        self.subsegstack[s] = checkseg;
                    }
                }
            }
        }

        if ivf.validflag {
            // Some vertices may be completed inside the cavity.
            for i in 0..self.cavetetvertlist.len() {
                let p = self.cavetetvertlist[i];
                let st = decode(self.point2tet(p));
                *searchtet = st;
                if self.infected(st.tet) {
                    // The point has been removed.
                    let vt = self.pointtype(p);
                    if vt != VertType::Unused {
                        self.setpointtype(p, VertType::Unused);
                        self.unuverts += 1;
                    }
                    if vt != VertType::Vol {
                        if vt == VertType::FreeSeg {
                            self.st_segref_count -= 1;
                        } else if vt == VertType::FreeFacet {
                            self.st_facref_count -= 1;
                        } else if vt == VertType::FreeVol {
                            self.st_volref_count -= 1;
                        }
                        if self.steinerleft > 0 {
                            self.steinerleft += 1;
                        }
                    }
                }
            }
        }

        if ivf.chkencflag & 3 != 0 {
            // Queue the segments (bit 1) and subfaces (bit 2) at C(p) for the
            //   encroachment checks (Delaunay refinement, gh:#1795).
            self.queue_cavity_boundary(ivf.chkencflag, splitseg.is_some());
        }

        if ivf.chkencflag & 4 != 0 {
            // Queue all new tetrahedra in C(p) (mesh improvement, gh:#1794).
            for i in 0..self.cavebdrylist.len() {
                let t = self.cavebdrylist[i];
                self.enqueuetetrahedron(&t);
            }
        }

        // C(p) is re-meshed successfully. Delete the old tets in C(p).
        for i in 0..self.caveoldtetlist.len() {
            let t = self.caveoldtetlist[i];
            if self.ishulltet(t.tet) {
                self.hullsize -= 1;
            }
            self.tetrahedrondealloc(t.tet);
        }

        if has_sh || has_seg {
            // Delete the old subfaces in sC(p).
            for i in 0..self.caveshlist.len() {
                let parysh = self.caveshlist[i];
                if self.checksubfaceflag {
                    // Clear connections in the adjacent tets not in C(p).
                    let mut neightet = self.stpivot(&parysh);
                    if neightet.tet != NULL_TET && self.tets[neightet.tet].v[0] != NULL_POINT {
                        self.tsdissolve(&neightet);
                        self.fsymself(&mut neightet);
                        self.tsdissolve(&neightet);
                    }
                }
                self.shellfacedealloc(ShPool::Sub, parysh.sh);
            }
            if has_seg {
                // Delete the old segment in sC(p).
                let sg = *splitseg.as_deref().unwrap();
                self.shellfacedealloc(ShPool::Seg, sg.sh);
            }
        }

        if ivf.lawson != 0 {
            for i in 0..self.cavebdrylist.len() {
                let t = self.cavebdrylist[i];
                self.flippush(&t);
            }
            if ivf.lawson > 1 {
                for i in 0..self.cavetetlist.len() {
                    let t = self.cavetetlist[i];
                    self.flippush(&t);
                }
            }
        }

        // Clean the working lists.
        self.caveoldtetlist.clear();
        self.cavebdrylist.clear();
        self.cavetetlist.clear();
        if self.checksubsegflag {
            self.cavetetseglist.clear();
            self.caveencseglist.clear();
        }
        if self.checksubfaceflag {
            self.cavetetshlist.clear();
            self.caveencshlist.clear();
        }
        if ivf.smlenflag || ivf.validflag {
            self.cavetetvertlist.clear();
        }
        if has_sh || has_seg {
            self.caveshlist.clear();
            self.caveshbdlist.clear();
            self.cavesegshlist.clear();
        }

        Ok(true) // Point is inserted.
    }

    /// Cut a tet from the cavity (`uninfect`, `unmarktest`), queue its face
    /// (`epivot`) and three new faces on `cavebdrylist`.
    fn cut_cavity_tet(&mut self, neightet: &mut TriFace, with_self: bool) {
        self.uninfect(neightet.tet);
        self.unmarktest(neightet.tet);
        if with_self {
            let mut t = *neightet;
            t.ver = EPIVOT[t.ver];
            self.cavebdrylist.push(t);
            neightet.ver = t.ver;
        }
        // Add three new faces to find new boundaries.
        for _ in 0..3 {
            let mut neineitet = esym(neightet);
            neineitet.ver = EPIVOT[neineitet.ver];
            self.cavebdrylist.push(neineitet);
            enextself(neightet);
        }
    }
}

impl TetgenMesh {
    /// `linelineint(A, B, C, D, P, Q, tp, tq)`: the shortest segment `[P, Q]`
    /// between the lines `A B` and `C D`; `None` if they are (nearly)
    /// parallel.
    pub fn linelineint(
        &self,
        a: &[f64; 3],
        b: &[f64; 3],
        c: &[f64; 3],
        d: &[f64; 3],
    ) -> Option<([f64; 3], [f64; 3], f64, f64)> {
        let mut vab = [0.0; 3];
        let mut vcd = [0.0; 3];
        let mut vca = [0.0; 3];
        for i in 0..3 {
            vab[i] = b[i] - a[i];
            vcd[i] = d[i] - c[i];
            vca[i] = a[i] - c[i];
        }
        let vab_vab = dot(&vab, &vab);
        let vcd_vcd = dot(&vcd, &vcd);
        let vab_vcd = dot(&vab, &vcd);
        let det = vab_vab * vcd_vcd - vab_vcd * vab_vcd;
        // Round the result.
        let eps = det / ((vab_vab * vcd_vcd).abs() + (vab_vcd * vab_vcd).abs());
        if eps < self.b.epsilon {
            return None;
        }
        let vca_vab = dot(&vca, &vab);
        let vca_vcd = dot(&vca, &vcd);
        let tp = (vcd_vcd * (-vca_vab) + vab_vcd * vca_vcd) / det;
        let tq = (vab_vcd * (-vca_vab) + vab_vab * vca_vcd) / det;
        let mut p = [0.0; 3];
        let mut q = [0.0; 3];
        for i in 0..3 {
            p[i] = a[i] + tp * vab[i];
        }
        for i in 0..3 {
            q[i] = c[i] + tq * vcd[i];
        }
        Some((p, q, tp, tq))
    }

    /// `smoothpoint(smtpt, linkfacelist, ccw, opm)` with the volume
    /// objective: move `smtpt` towards link-face centres while the smallest
    /// volume grows. Returns the number of iterations (0: not moved).
    pub fn smoothpoint(
        &mut self,
        smtpt: &mut [f64; 3],
        linkfacelist: &mut [TriFace],
        ccw: bool,
        opm: &mut OptParameters,
    ) -> Result<i32, TetgenError> {
        self.reach[35] += 1;
        let objects = linkfacelist.len();
        // Decide the number of moving directions.
        let mut numdirs = objects as i32;
        if numdirs > opm.numofsearchdirs {
            numdirs = opm.numofsearchdirs;
        }

        // Set the initial value.
        opm.imprval = opm.initval;
        let mut iter = 0;
        let mut startpt = *smtpt;
        let mut bestpt = *smtpt;
        let mut minval = 0.0;

        // Iterate until the obj function is not improved.
        loop {
            // Find the best next location.
            let oldval = opm.imprval;

            for i in 0..numdirs as usize {
                // Randomly pick a link face (0 <= k <= objects - i - 1).
                let k = self.randomnation((objects - i) as u32) as usize;
                let t = linkfacelist[k];
                let (pa, pb, pc) = (
                    self.x(self.org(&t)),
                    self.x(self.dest(&t)),
                    self.x(self.apex(&t)),
                );
                let mut fcent = [0.0; 3];
                for j in 0..3 {
                    fcent[j] = (pa[j] + pb[j] + pc[j]) / 3.0;
                }
                let mut nextpt = [0.0; 3];
                for j in 0..3 {
                    nextpt[j] = startpt[j] + opm.searchstep * (fcent[j] - startpt[j]);
                }
                // Calculate the largest minimum function value for the new location.
                let mut j = 0;
                while j < objects {
                    let t = linkfacelist[j];
                    let (pa, pb) = if ccw {
                        (self.x(self.org(&t)), self.x(self.dest(&t)))
                    } else {
                        (self.x(self.dest(&t)), self.x(self.org(&t)))
                    };
                    let pc = self.x(self.apex(&t));
                    let ori = self.pred.orient3d(&pa, &pb, &pc, &nextpt);
                    let val;
                    if ori < 0.0 {
                        if opm.max_min_volume {
                            val = -orient3dfast(&pa, &pb, &pc, &nextpt);
                        } else {
                            // Aspect-ratio / dihedral objectives: not ported.
                            val = 0.0;
                        }
                    } else if opm.max_min_volume {
                        val = -orient3dfast(&pa, &pb, &pc, &nextpt);
                    } else {
                        break; // Discard this point.
                    }
                    // Stop looping when the object value is not improved.
                    if val <= opm.imprval {
                        break;
                    } else if j == 0 {
                        minval = val;
                    } else {
                        minval = if val < minval { val } else { minval };
                    }
                    j += 1;
                }
                if j == objects {
                    // The function value has been improved.
                    opm.imprval = minval;
                    bestpt = nextpt;
                }
                // Swap k-th and (object-i-1)-th entries.
                let j = objects - i - 1;
                linkfacelist.swap(k, j);
            }

            let diff = opm.imprval - oldval;
            if diff > 0.0 && !opm.max_min_volume {
                // Unknown objective function.
                return Err(TetgenError::InternalError);
            }

            if diff > 0.0 {
                // Yes, move p to the new location and continue.
                startpt = bestpt;
                iter += 1;
                if opm.maxiter > 0 && iter >= opm.maxiter {
                    break;
                }
            } else {
                break;
            }
        }

        if iter > 0 {
            // The point has been smoothed.
            opm.smthiter = iter;
            *smtpt = startpt;
        }
        Ok(iter)
    }

    /// `add_steinerpt_in_schoenhardtpoly(abtets, n, splitsliverflag,
    /// chkencflag)`: add a Steiner point inside the polyhedron of the `n`
    /// tets at `[a,b]` that the missing edge `[p0, p_(n-1)]` crosses.
    pub fn add_steinerpt_in_schoenhardtpoly(
        &mut self,
        abtets: &mut [TriFace],
        n: usize,
        splitsliverflag: bool,
        chkencflag: i32,
    ) -> Result<bool, TetgenError> {
        self.reach[25] += 1;
        let mut ivf = InsertPointFlags::default();

        if splitsliverflag {
            self.reach[53] += 1;
            // randomly pick a tet.
            let idx = (self.crand.rand() % n as i32) as usize;
            let t = abtets[idx];
            let (pa, pb, pc, pd) = (
                self.x(self.org(&t)),
                self.x(self.dest(&t)),
                self.x(self.apex(&t)),
                self.x(self.oppo(&t)),
            );
            let steinerpt = self.makepoint(VertType::FreeVol);
            for i in 0..3 {
                self.points[steinerpt].xyz[i] = (pa[i] + pb[i] + pc[i] + pd[i]) / 4.0;
            }
            let mut worktet = t;
            ivf.iloc = LocateResult::Outside;
            ivf.bowywat = 1;
            ivf.lawson = 2;
            ivf.rejflag = 0;
            ivf.chkencflag = chkencflag;
            ivf.sloc = LocateResult::Unknown;
            ivf.sbowywat = 0;
            ivf.splitbdflag = false;
            ivf.validflag = true;
            ivf.respectbdflag = true;
            ivf.assignmeshsize = self.b.metric;
            if self.insertpoint(steinerpt, &mut worktet, None, None, &mut ivf)? {
                if self.flippool_items() > 0 {
                    self.recoverdelaunay()?;
                }
                self.st_volref_count += 1;
                if self.steinerleft > 0 {
                    self.steinerleft -= 1;
                }
                return Ok(true);
            } else {
                self.pointdealloc(steinerpt);
                return Ok(false);
            }
        }

        let pc = self.x(self.apex(&abtets[0])); // pc = p0
        let pd = self.x(self.oppo(&abtets[n - 1])); // pd = p_(n-1)

        // Initialize the list of 2n boundary faces.
        for t in abtets.iter().take(n) {
            self.cavetetlist.push(edestoppo(t)); // [p_i,p_i+1,a]
            self.cavetetlist.push(eorgoppo(t)); // [p_i+1,p_i,b]
        }

        let big_n = 100;
        let stepi = 0.01;
        let mut vcd = [0.0; 3];
        for i in 0..3 {
            vcd[i] = pd[i] - pc[i];
        }

        // Sample N points in edge [c,d].
        let mut maxminvol = 0.0;
        let mut minvol = 0.0;
        let mut maxidx = 0;
        for it in 1..big_n {
            let mut sampt = [0.0; 3];
            for i in 0..3 {
                sampt[i] = pc[i] + (stepi * it as f64) * vcd[i];
            }
            for i in 0..self.cavetetlist.len() {
                let t = self.cavetetlist[i];
                let ori = self.pred.orient3d(
                    &self.x(self.dest(&t)),
                    &self.x(self.org(&t)),
                    &self.x(self.apex(&t)),
                    &sampt,
                );
                if i == 0 || minvol > ori {
                    minvol = ori;
                }
            }
            if it == 1 || maxminvol < minvol {
                maxminvol = minvol;
                maxidx = it;
            }
        }

        if maxminvol <= 0.0 {
            self.cavetetlist.clear();
            return Ok(false);
        }

        let mut smtpt = [0.0; 3];
        for i in 0..3 {
            smtpt[i] = pc[i] + (stepi * maxidx as f64) * vcd[i];
        }

        // Create two faked tets to hold the two non-existing boundary faces:
        //   [d,c,a] and [c,d,b].
        let (p0, pn) = (self.apex(&abtets[0]), self.oppo(&abtets[n - 1]));
        let faketet1 = self.maketetrahedron();
        let o0 = self.org(&abtets[0]);
        self.setvertices(&faketet1, pn, p0, o0, DUMMYPOINT);
        self.cavetetlist.push(faketet1);
        let faketet2 = self.maketetrahedron();
        let d0 = self.dest(&abtets[0]);
        self.setvertices(&faketet2, p0, pn, d0, DUMMYPOINT);
        self.cavetetlist.push(faketet2);

        // Point smooth options.
        let mut opm = OptParameters {
            max_min_volume: true,
            numofsearchdirs: 20,
            searchstep: 0.001,
            maxiter: 100,
            initval: 0.0,
            ..Default::default()
        };

        // Try to relocate the point into the inside of the polyhedron.
        let mut list = core::mem::take(&mut self.cavetetlist);
        let success = self.smoothpoint(&mut smtpt, &mut list, true, &mut opm)?;
        if success != 0 {
            while opm.smthiter == 100 {
                // Try to increase the search stepsize.
                opm.searchstep *= 10.0;
                opm.initval = opm.imprval;
                opm.smthiter = 0;
                self.smoothpoint(&mut smtpt, &mut list, true, &mut opm)?;
            }
        }
        list.clear();
        self.cavetetlist = list;

        // Delete the two faked tets.
        self.tetrahedrondealloc(faketet1.tet);
        self.tetrahedrondealloc(faketet2.tet);

        self.cavetetlist.clear();

        if success != 0 {
            // Insert this Steiner point.
            let steinerpt = self.makepoint(VertType::FreeVol);
            self.points[steinerpt].xyz = smtpt;

            for t in abtets.iter().take(n) {
                self.infect(t.tet);
                self.caveoldtetlist.push(*t);
            }
            let mut worktet = abtets[0]; // No need point location.
            ivf.iloc = LocateResult::InStar;
            ivf.chkencflag = chkencflag;
            ivf.assignmeshsize = self.b.metric;
            if ivf.assignmeshsize {
                // Search the tet containing 'steinerpt' for size
                //   interpolation (-m, gh:#1825). Upstream's located
                //   `abtets[0]` becomes the start; `insertpoint` with
                //   INSTAR then gives the point size 0.
                self.reachm[15] += 1;
                self.locate(steinerpt, &mut abtets[0], false)?;
                worktet = abtets[0];
            }
            if self.insertpoint(steinerpt, &mut worktet, None, None, &mut ivf)? {
                self.st_volref_count += 1;
                if self.steinerleft > 0 {
                    self.steinerleft -= 1;
                }
                return Ok(true);
            } else {
                self.pointdealloc(steinerpt);
                return Ok(false);
            }
        }

        Ok(false)
    }

    /// `add_steinerpt_in_segment(misseg, searchlevel, idir)`: split the
    /// missing segment at its closest point to a segment met while flipping
    /// its first crossing face or edge.
    pub fn add_steinerpt_in_segment(
        &mut self,
        misseg: &mut Face,
        searchlevel: i32,
    ) -> Result<(bool, i32), TetgenError> {
        self.reach[24] += 1;
        let startpt = self.sorg(misseg);
        let endpt = self.sdest(misseg);
        let mut idir = DISJOINT;

        let mut fc = FlipConstraints {
            checkflipeligibility: true,
            collectencsegflag: true,
            ..Default::default()
        };
        fc.seg = [startpt, endpt];

        let mut searchtet = self.point2tetorg(startpt);
        let dir = self.finddirection(&mut searchtet, endpt)?;
        if dir == ACROSSVERT {
            return Ok((false, idir));
        }

        // Try to flip the first intersecting face/edge.
        enextesymself(&mut searchtet); // Go to the opposite face.

        let bak_fliplinklevel = self.b.fliplinklevel;
        self.b.fliplinklevel = searchlevel;

        if dir == ACROSSFACE {
            self.removefacebyflips(&searchtet, &mut fc)?;
        } else if dir == ACROSSEDGE {
            self.removeedgebyflips(&mut searchtet, &mut fc)?;
        }

        let mut split = 0.0;
        let mut split_q = 0.0;
        let mut smlen = 0.0;
        let mut candseg = Face::default();
        let (xs, xe) = (self.x(startpt), self.x(endpt));
        for i in 0..self.caveencseglist.len() {
            let paryseg = self.caveencseglist[i];
            self.suninfect(&paryseg);
            // Calculate the shortest edge between the two lines.
            let pc = self.x(self.sorg(&paryseg));
            let pd = self.x(self.sdest(&paryseg));
            if let Some((p, q, mut tp, mut tq)) = self.linelineint(&xs, &xe, &pc, &pd) {
                // Round tp and tq.
                if tp > 0.0 && tq < 1.0 {
                    if tp < 0.5 {
                        if tp < self.b.epsilon * 1e3 {
                            tp = 0.0;
                        }
                    } else if (1.0 - tp) < self.b.epsilon * 1e3 {
                        tp = 1.0;
                    }
                }
                if tp <= 0.0 || tp >= 1.0 {
                    continue;
                }
                if tq > 0.0 && tq < 1.0 {
                    if tq < 0.5 {
                        if tq < self.b.epsilon * 1e3 {
                            tq = 0.0;
                        }
                    } else if (1.0 - tq) < self.b.epsilon * 1e3 {
                        tq = 1.0;
                    }
                }
                if tq <= 0.0 || tq >= 1.0 {
                    continue;
                }
                // It is a valid shortest edge. Calculate its length.
                let len = distance(&p, &q);
                if split == 0.0 || len < smlen {
                    smlen = len;
                    split = tp;
                    split_q = tq;
                    candseg = paryseg;
                }
            }
        }

        self.caveencseglist.clear();
        self.b.fliplinklevel = bak_fliplinklevel;

        if split == 0.0 {
            // Found no crossing segment.
            return Ok((false, idir));
        }
        let _ = split_q;

        // (b->addsteiner_algo == 1, the default.)
        // Split the segment at the closest point to a near segment.
        let steinerpt = self.makepoint(VertType::FreeSeg);
        for i in 0..3 {
            self.points[steinerpt].xyz[i] = xs[i] + split * (xe[i] - xs[i]);
        }

        // Check if the two segments are nearly crossing each other.
        let pc = self.sorg(&candseg);
        let pd = self.sdest(&candseg);
        if self.is_collinear_at(steinerpt, pc, pd) {
            let segidx = self.getfacetindex(misseg) as usize;
            let (p1, p2) = (
                self.segmentendpointslist[segidx * 2],
                self.segmentendpointslist[segidx * 2 + 1],
            );
            let segidx2 = self.getfacetindex(&candseg) as usize;
            let (p3, p4) = (
                self.segmentendpointslist[segidx2 * 2],
                self.segmentendpointslist[segidx2 * 2 + 1],
            );
            self.warn(alloc::format!(
                "Warning:  Two line segments are almost crossing.\n  1st: [{},{}].\n  2nd: [{},{}].",
                self.pointmark(p1), self.pointmark(p2), self.pointmark(p3), self.pointmark(p4)
            ));
            // Calculate a new angle tolerance.
            let collinear_ang =
                self.interiorangle(&self.x(steinerpt), &self.x(pc), &self.x(pd), None)
                    / core::f64::consts::PI
                    * 180.0;
            let ang_diff = collinear_ang - self.b.collinear_ang_tol;
            let new_ang_tol = collinear_ang + ang_diff / 180.0;
            if new_ang_tol < 180.0 {
                // Reduce the angle tolerance to detect collinear event.
                self.b.collinear_ang_tol = new_ang_tol;
                self.cos_collinear_ang_tol =
                    (self.b.collinear_ang_tol / 180.0 * core::f64::consts::PI).cos();
            } else {
                // Report a self-intersection event due to epsilon.
                idir = SELF_INTERSECT;
                self.pointdealloc(steinerpt);
                return Ok((false, idir));
            }
        }

        // We need to locate the point. Start searching from 'searchtet'.
        let mut searchtet = if split < 0.5 {
            self.point2tetorg(startpt)
        } else {
            self.point2tetorg(endpt)
        };
        let mut splitseg = *misseg;
        let mut splitsh = self.spivot(misseg);
        // for create_a_shorter_edge().
        self.setpoint2sh(steinerpt, sencode(misseg));

        let mut ivf = InsertPointFlags {
            iloc: LocateResult::Outside,
            bowywat: 1,
            lawson: 2,
            rejflag: 0,
            chkencflag: 0,
            sloc: LocateResult::OnEdge,
            sbowywat: 1,
            splitbdflag: false,
            validflag: true,
            respectbdflag: true,
            assignmeshsize: self.b.metric,
            ..Default::default()
        };

        if self.insertpoint(
            steinerpt,
            &mut searchtet,
            Some(&mut splitsh),
            Some(&mut splitseg),
            &mut ivf,
        )? {
            if self.flippool_items() > 0 {
                self.recoverdelaunay()?;
            }
        } else {
            self.pointdealloc(steinerpt);
            return Ok((false, idir));
        }

        // Save this Steiner point (for removal).
        self.subvertstack.push(steinerpt);
        self.st_segref_count += 1;
        if self.steinerleft > 0 {
            self.steinerleft -= 1;
        }
        Ok((true, idir))
    }

    /// Warnings of `add_steinerpt_to_recover_edge` / `recoveredgebyflips`
    /// when the edge meets a vertex `nearpt`: returns whether it is a real
    /// self-intersection (upstream's `intersect_flag`), or an internal
    /// error where upstream stops.
    fn seg_vertex_intersect(
        &mut self,
        misseg: &Face,
        nearpt: Point,
        nearly: bool,
    ) -> Result<bool, TetgenError> {
        let how = if nearly { "nearly" } else { "exactly" };
        let segidx = self.getfacetindex(misseg) as usize;
        let p1 = self.segmentendpointslist[segidx * 2];
        let p2 = self.segmentendpointslist[segidx * 2 + 1];
        if !self.issteinerpoint(nearpt) {
            // It is an input point.
            let tmppt = if self.is_segment(p1, nearpt) {
                p1
            } else if self.is_segment(p2, nearpt) {
                p2
            } else {
                NULL_POINT
            };
            let w = if tmppt != NULL_POINT {
                alloc::format!(
                    "Warning:  Two line segments are {how} overlapping.\n  1st: [{},{}].\n  2nd: [{},{}].",
                    self.pointmark(p1), self.pointmark(p2), self.pointmark(tmppt), self.pointmark(nearpt)
                )
            } else {
                alloc::format!(
                    "Warning:  A vertex lies {how} on a line segment.\n  segment: [{},{}].\n  vertex : [{}].",
                    self.pointmark(p1), self.pointmark(p2), self.pointmark(nearpt)
                )
            };
            self.warn(w);
            Ok(true)
        } else if self.pointtype(nearpt) == VertType::FreeSeg {
            let parsentseg = sdecode(self.point2sh(nearpt));
            let segidx2 = self.getfacetindex(&parsentseg) as usize;
            if segidx2 != segidx {
                let (p3, p4) = (
                    self.segmentendpointslist[segidx2 * 2],
                    self.segmentendpointslist[segidx2 * 2 + 1],
                );
                self.warn(alloc::format!(
                    "Warning:  Two line segments are {how} crossing.\n  1st: [{},{}].\n  2nd: [{},{}].",
                    self.pointmark(p1), self.pointmark(p2), self.pointmark(p3), self.pointmark(p4)
                ));
                Ok(true)
            } else if !nearly {
                // ivf.iloc == ONVERTEX: "This should not be possible."
                Err(TetgenError::InternalError)
            } else {
                Ok(false)
            }
        } else {
            Err(TetgenError::InternalError)
        }
    }

    /// `add_steinerpt_to_recover_edge(startpt, endpt, misseg,
    /// splitsegflag, splitsliverflag, idir)`: add a Steiner point near the
    /// missing edge (`splitsegflag == 0`) or on it. Returns `(added, idir)`.
    pub fn add_steinerpt_to_recover_edge(
        &mut self,
        mut startpt: Point,
        mut endpt: Point,
        mut misseg: Option<&mut Face>,
        splitsegflag: i32,
        splitsliverflag: i32,
    ) -> Result<(bool, i32), TetgenError> {
        self.reach[23] += 1;
        let mut idir = DISJOINT;
        let mut types = [0i32; 2];
        let mut poss = [0i32; 4];

        if let Some(ms) = misseg.as_deref_mut() {
            startpt = self.sorg(ms);
            if self.pointtype(startpt) == VertType::FreeSeg {
                *ms = sesym(ms);
                startpt = self.sorg(ms);
            }
            endpt = self.sdest(ms);
        }

        let mut searchtet = self.point2tetorg(startpt);
        let dir = self.finddirection(&mut searchtet, endpt)?;

        if dir == ACROSSVERT {
            if self.dest(&searchtet) == endpt {
                // This edge exists.
                if let Some(ms) = misseg.as_deref() {
                    self.subsegstack.push(*ms);
                }
                return Ok((true, idir));
            } else {
                // This edge crosses a vertex (not endpt).
                let mut intersect_flag = false;
                if let Some(ms) = misseg.as_deref() {
                    let nearpt = self.dest(&searchtet);
                    let ms = *ms;
                    intersect_flag = self.seg_vertex_intersect(&ms, nearpt, false)?;
                }
                if intersect_flag {
                    idir = SELF_INTERSECT;
                }
                return Ok((false, idir));
            }
        }

        enextself(&mut searchtet);

        if dir == ACROSSFACE {
            // The segment is crossing at least 3 faces. Find the common edge of
            //   the first 3 crossing faces.
            esymself(&mut searchtet);
            let mut spintet = self.fsym(&searchtet);
            let pd = self.oppo(&spintet);

            if pd == endpt {
                if let Some(ms) = misseg.as_deref() {
                    let ms = *ms;
                    // Calclate the smallest angle between (a,b,c) and (startpt, endpt).
                    let mut tmptet = TriFace::default();
                    let mut collinear_ang = 0.0;
                    let (xs, xe) = (self.x(startpt), self.x(endpt));
                    for _ in 0..3 {
                        let ang = self.interiorangle(&self.x(self.org(&searchtet)), &xs, &xe, None);
                        if ang > collinear_ang {
                            collinear_ang = ang;
                            tmptet = searchtet;
                        }
                        enextself(&mut searchtet);
                    }
                    collinear_ang = collinear_ang / core::f64::consts::PI * 180.0;

                    if collinear_ang > self.b.collinear_ang_tol {
                        // Report a self-intersection event due to epsilon.
                        let nearpt = self.org(&tmptet);
                        if !self.issteinerpoint(nearpt) {
                            self.seg_vertex_intersect(&ms, nearpt, true)?;
                        } else if self.pointtype(nearpt) == VertType::FreeSeg {
                            let parsentseg = sdecode(self.point2sh(nearpt));
                            let segidx = self.getfacetindex(&ms);
                            let segidx2 = self.getfacetindex(&parsentseg);
                            if segidx2 != segidx {
                                self.warn(String::from(
                                    "Warning:  Two line segments are nearly crossing.",
                                ));
                            } else {
                                return Err(TetgenError::InternalError);
                            }
                        } else {
                            return Err(TetgenError::InternalError);
                        }
                        // Calculate a new angle tolerance.
                        let ang_diff = collinear_ang - self.b.collinear_ang_tol;
                        let new_ang_tol = collinear_ang + ang_diff / 180.0;
                        if new_ang_tol < 180.0 {
                            self.b.collinear_ang_tol = new_ang_tol;
                            self.cos_collinear_ang_tol =
                                (self.b.collinear_ang_tol / 180.0 * core::f64::consts::PI).cos();
                            // This segment can be recovered by a 2-3 flip.
                            self.subsegstack.push(ms);
                            return Ok((true, idir));
                        } else {
                            idir = SELF_INTERSECT;
                            return Ok((false, idir));
                        }
                    } else {
                        // This segment can be recovered by a 2-3 flip.
                        self.subsegstack.push(ms);
                        return Ok((true, idir));
                    }
                } else {
                    // This edge (not a segment) can be recovered by a 2-3 flip.
                    return Ok((true, idir));
                }
            }

            if self.issubface(&searchtet) {
                if misseg.is_some() {
                    return Err(TetgenError::InternalError);
                }
                return Ok((false, idir));
            }

            for _ in 0..3 {
                let pa = self.org(&spintet);
                let pb = self.dest(&spintet);
                if self.tri_edge_test(pa, pb, pd, startpt, endpt, None, 1, &mut types, &mut poss)
                    != 0
                {
                    break; // Found the edge.
                }
                enextself(&mut spintet);
                eprevself(&mut searchtet);
            }
            esymself(&mut searchtet);
        } else {
            // dir == ACROSSEDGE;
            if self.issubseg(&searchtet) {
                return Err(TetgenError::InternalError);
            }
        }

        if splitsegflag == 0 {
            // Try to recover this segment by adding Steiner points near it.
            let mut spintet = searchtet;
            let mut n = 0;
            let mut endi: isize = -1;
            loop {
                // Check if the endpt appears in the star.
                if self.apex(&spintet) == endpt {
                    endi = n as isize; // Remember the position of endpt.
                }
                n += 1; // Count a tet in the star.
                self.fnextself(&mut spintet);
                if spintet.tet == searchtet.tet {
                    break;
                }
            }

            if endi > 0 {
                let endi = endi as usize;
                // endpt is also in the edge star. Get all tets in the edge star.
                let mut abtets: Vec<TriFace> = Vec::with_capacity(n);
                let mut spintet = searchtet;
                for _ in 0..n {
                    abtets.push(spintet);
                    self.fnextself(&mut spintet);
                }

                let mut success = 0;

                if dir == ACROSSFACE {
                    // Find a Steiner points inside the polyhedron.
                    if self.add_steinerpt_in_schoenhardtpoly(
                        &mut abtets,
                        endi,
                        splitsliverflag != 0,
                        0,
                    )? {
                        success = 1;
                    }
                } else if dir == ACROSSEDGE {
                    // PLC check.
                    if self.issubseg(&searchtet) {
                        return Err(TetgenError::InternalError);
                    }
                    if n > 4 {
                        if endi > 2
                            && self.add_steinerpt_in_schoenhardtpoly(
                                &mut abtets,
                                endi,
                                splitsliverflag != 0,
                                0,
                            )?
                        {
                            success += 1;
                        }
                        if (n - endi) > 2
                            && self.add_steinerpt_in_schoenhardtpoly(
                                &mut abtets[endi..],
                                n - endi,
                                splitsliverflag != 0,
                                0,
                            )?
                        {
                            success += 1;
                        }
                    }
                    // else: a 4-to-4 flip should re-cover the edge (nothing to do).
                } else {
                    return Err(TetgenError::InternalError);
                }

                if success != 0 {
                    if let Some(ms) = misseg.as_deref() {
                        // Add the missing segment back to the recovering list.
                        self.subsegstack.push(*ms);
                    }
                    return Ok((true, idir));
                }
            }

            return Ok((false, idir));
        }

        // (b->addsteiner_algo == 1 > 0.)
        {
            let ms = match misseg.as_deref_mut() {
                Some(m) => m,
                None => return Err(TetgenError::InternalError),
            };
            let (ok, d) = self.add_steinerpt_in_segment(ms, 3)?;
            idir = d;
            if ok {
                return Ok((true, idir));
            }
            if idir == SELF_INTERSECT {
                return Ok((false, idir));
            }
            *ms = sesym(ms);
            let (ok, d) = self.add_steinerpt_in_segment(ms, 3)?;
            idir = d;
            if ok {
                return Ok((true, idir));
            }
            *ms = sesym(ms);
            if idir == SELF_INTERSECT {
                return Ok((false, idir));
            }
        }

        // Let the face [a,b,d] be the first intersecting face of the segment
        //   [startpt, endpt]. We add the interseting point.
        let mut searchtet = self.point2tetorg(startpt);
        let dir = self.finddirection(&mut searchtet, endpt)?;
        if dir == ACROSSVERT {
            if self.dest(&searchtet) == endpt {
                // This edge exists.
                if let Some(ms) = misseg.as_deref() {
                    self.subsegstack.push(*ms);
                }
                return Ok((true, idir));
            } else {
                // This should be a self-intersection.
                if misseg.is_some() {
                    return Err(TetgenError::InternalError);
                }
                return Ok((false, idir));
            }
        }

        enextself(&mut searchtet);
        let pa = self.org(&searchtet);
        let pb = self.dest(&searchtet);
        let pd = self.oppo(&searchtet);

        // Calculate the intersection of the face [a,b,d] and the segment.
        let fpt = self.sort_3pts(pa, pb, pd);
        let ept = self.sort_2pts(startpt, endpt);
        let (ip, u) = self.planelineint(
            &self.x(fpt[0]),
            &self.x(fpt[1]),
            &self.x(fpt[2]),
            &self.x(ept[0]),
            &self.x(ept[1]),
        );

        if u > 0.0 && u < 1.0 {
            // Create a Steiner point.
            let steinerpt = self.makepoint(VertType::FreeSeg);
            self.points[steinerpt].xyz = ip;
            let ms = match misseg.as_deref_mut() {
                Some(m) => m,
                None => return Err(TetgenError::InternalError),
            };
            // for create_a_shorter_edge().
            self.setpoint2sh(steinerpt, sencode(ms));

            esymself(&mut searchtet); // The crossing face/edge.
            let mut splitsh = self.spivot(ms);
            let mut ivf = InsertPointFlags {
                refineflag: if dir == ACROSSFACE { 4 } else { 8 },
                iloc: LocateResult::Outside,
                refinetet: searchtet,
                bowywat: 1,
                lawson: 2,
                rejflag: 0,
                chkencflag: 0,
                sloc: LocateResult::OnEdge,
                sbowywat: 1,
                splitbdflag: false,
                validflag: true,
                respectbdflag: true,
                assignmeshsize: self.b.metric,
                ..Default::default()
            };

            if self.insertpoint(
                steinerpt,
                &mut searchtet,
                Some(&mut splitsh),
                Some(ms),
                &mut ivf,
            )? {
                if self.flippool_items() > 0 {
                    self.recoverdelaunay()?;
                }
                // Save this Steiner point (for removal).
                self.subvertstack.push(steinerpt);
                self.st_segref_count += 1;
                if self.steinerleft > 0 {
                    self.steinerleft -= 1;
                }
                return Ok((true, idir));
            } else {
                // Check if this failure is due to a self-intersection.
                if ivf.iloc == LocateResult::OnVertex || ivf.iloc == LocateResult::NearVertex {
                    let ms = *misseg.as_deref().unwrap();
                    let nearpt = self.org(&searchtet);
                    let nearly = ivf.iloc == LocateResult::NearVertex;
                    let intersect_flag = if !self.issteinerpoint(nearpt) {
                        self.seg_vertex_intersect(&ms, nearpt, nearly)?
                    } else if self.pointtype(nearpt) == VertType::FreeSeg {
                        let parsentseg = sdecode(self.point2sh(nearpt));
                        if self.getfacetindex(&parsentseg) != self.getfacetindex(&ms) {
                            self.warn(String::from("Warning:  Two line segments are crossing."));
                            true
                        } else {
                            false
                        }
                    } else {
                        return Err(TetgenError::InternalError);
                    };
                    if intersect_flag {
                        idir = SELF_INTERSECT;
                    }
                }
                // The vertex is not inserted.
                self.pointdealloc(steinerpt);
            }
        }

        Ok((false, idir)) // Failed to reocver this segment.
    }
}

impl TetgenMesh {
    /// The Steiner branch of `recoversubfaces` when all three edges exist
    /// but the face does not: insert a point where the last crossing edge
    /// meets the subface (or at its barycentre). Returns `success`.
    pub(crate) fn recoversubfaces_steiner_in_face(
        &mut self,
        searchsh: &mut Face,
        searchtet: &mut TriFace,
        pts: [Point; 3],
        cross: [Point; 2],
        steinerflag: i32,
        dir: &mut i32,
    ) -> Result<bool, TetgenError> {
        let [startpt, endpt, apexpt] = pts;
        let fpt = self.sort_3pts(startpt, endpt, apexpt);
        let ept = self.sort_2pts(cross[0], cross[1]);
        let (ip, u) = self.planelineint(
            &self.x(fpt[0]),
            &self.x(fpt[1]),
            &self.x(fpt[2]),
            &self.x(ept[0]),
            &self.x(ept[1]),
        );

        let steinerpt = self.makepoint(VertType::FreeFacet);
        let (xs, xe, xa) = (self.x(startpt), self.x(endpt), self.x(apexpt));
        let bary = |p: &mut [f64; 3]| {
            for j in 0..3 {
                p[j] = (xs[j] + xe[j] + xa[j]) / 3.0;
            }
        };
        if u > 0.0 && u < 1.0 {
            self.points[steinerpt].xyz = ip;
            // Make sure that this Steiner point is inside the subface.
            if self.is_collinear_at(steinerpt, startpt, endpt)
                || self.is_collinear_at(steinerpt, endpt, apexpt)
                || self.is_collinear_at(steinerpt, apexpt, startpt)
            {
                // Add the barycenter of this missing subface.
                bary(&mut self.points[steinerpt].xyz);
                // Avoid creating a very skinny triangle
                if self.is_collinear_at(steinerpt, startpt, endpt)
                    || self.is_collinear_at(steinerpt, endpt, apexpt)
                    || self.is_collinear_at(steinerpt, apexpt, startpt)
                {
                    return Err(TetgenError::InternalError);
                }
            }
        } else {
            // Add the barycenter of this missing subface.
            bary(&mut self.points[steinerpt].xyz);
            if self.is_collinear_at(steinerpt, startpt, endpt)
                || self.is_collinear_at(steinerpt, endpt, apexpt)
                || self.is_collinear_at(steinerpt, apexpt, startpt)
            {
                return Err(TetgenError::InternalError);
            }
        }

        // for create_a_shorter_edge().
        self.setpoint2sh(steinerpt, sencode(searchsh));

        *searchtet = self.point2tetorg(startpt); // Start from 'searchtet'.
        let mut ivf = InsertPointFlags {
            iloc: LocateResult::Outside,
            bowywat: 1,
            lawson: 2,
            rejflag: 0,
            chkencflag: 0,
            sloc: LocateResult::OnFace,
            sbowywat: 1,
            splitbdflag: false,
            validflag: true,
            respectbdflag: true,
            assignmeshsize: self.b.metric,
            ..Default::default()
        };

        if self.insertpoint(steinerpt, searchtet, Some(searchsh), None, &mut ivf)? {
            if self.flippool_items() > 0 {
                self.recoverdelaunay()?;
            }
            // Save this Steiner point (for removal).
            self.subvertstack.push(steinerpt);
            self.st_facref_count += 1;
            if self.steinerleft > 0 {
                self.steinerleft -= 1;
            }
            Ok(true) // This subface has been split.
        } else {
            // Failed to insert this point.
            if ivf.iloc == LocateResult::NearVertex {
                let chkpt = self.org(searchtet);
                let dist = distance(&self.x(steinerpt), &self.x(chkpt));
                if dist < self.minedgelength {
                    if !self.issteinerpoint(chkpt) {
                        self.warn(alloc::format!(
                            "Warning:  A facet and a vertex {} are very close.",
                            self.pointmark(chkpt)
                        ));
                        *dir = SELF_INTERSECT;
                    }
                } else {
                    return Err(TetgenError::InternalError);
                }
            }
            if *dir != SELF_INTERSECT && steinerflag >= 2 {
                // NULLCAVITY ("terminatetetgen") or other: report a bug.
                return Err(TetgenError::InternalError);
            }
            self.pointdealloc(steinerpt);
            Ok(false) // queue this subface.
        }
    }

    /// The Steiner branch of `recoversubfaces` when the edge
    /// `[startpt, endpt]` of the subface is missing: split it (or the
    /// segment on it) where it crosses its first face or edge.
    pub(crate) fn recoversubfaces_steiner_on_edge(
        &mut self,
        searchsh: &mut Face,
        searchtet: &mut TriFace,
        startpt: Point,
        endpt: Point,
        steinerflag: i32,
        dir: &mut i32,
    ) -> Result<bool, TetgenError> {
        let mut success = false;
        // Find the first face/edge crossed by the edge (startpt, endpt).
        *searchtet = self.point2tetorg(startpt);
        *dir = self.finddirection(searchtet, endpt)?;

        if *dir != SELF_INTERSECT {
            enextself(searchtet);
            let pa = self.org(searchtet);
            let pb = self.dest(searchtet);
            let pd = self.oppo(searchtet);

            let fpt = self.sort_3pts(pa, pb, pd);
            let ept = self.sort_2pts(startpt, endpt);
            let (ip, _u) = self.planelineint(
                &self.x(fpt[0]),
                &self.x(fpt[1]),
                &self.x(fpt[2]),
                &self.x(ept[0]),
                &self.x(ept[1]),
            );

            let steinerpt = self.makepoint(VertType::FreeFacet);
            self.points[steinerpt].xyz = ip;

            let mut ivf = InsertPointFlags {
                refinetet: *searchtet, // bakup the crossing face/edge.
                ..Default::default()
            };

            let mut tmptet = *searchtet;
            ivf.iloc = self.locate(steinerpt, &mut tmptet, false)?;

            if ivf.iloc == LocateResult::OnVertex {
                // the origin of tmptet is co-incident with this Steiner point.
                *searchtet = tmptet;
            } else if *dir == ACROSSFACE {
                ivf.iloc = LocateResult::OnFace;
            } else if *dir == ACROSSEDGE {
                ivf.iloc = LocateResult::OnEdge;
            } else {
                return Err(TetgenError::InternalError);
            }

            ivf.bowywat = 1;
            ivf.lawson = 2;
            ivf.rejflag = 0;
            ivf.chkencflag = 0;
            ivf.sloc = LocateResult::OnEdge;
            ivf.sbowywat = 1;
            ivf.splitbdflag = false;
            ivf.validflag = true;
            ivf.respectbdflag = true;
            ivf.assignmeshsize = self.b.metric;

            // searchsh may contain a missing segment.
            let mut misseg = self.sspivot(searchsh);
            let splitseg_flag = !misseg.is_null();
            if splitseg_flag {
                self.setpointtype(steinerpt, VertType::FreeSeg);
                self.setpoint2sh(steinerpt, sencode(&misseg));
            } else {
                self.setpoint2sh(steinerpt, sencode(searchsh));
            }

            let inserted = if splitseg_flag {
                self.insertpoint(
                    steinerpt,
                    searchtet,
                    Some(searchsh),
                    Some(&mut misseg),
                    &mut ivf,
                )?
            } else {
                self.insertpoint(steinerpt, searchtet, Some(searchsh), None, &mut ivf)?
            };
            if inserted {
                if self.flippool_items() > 0 {
                    self.recoverdelaunay()?;
                }
                // Save this Steiner point (for removal).
                self.subvertstack.push(steinerpt);
                if splitseg_flag {
                    self.st_segref_count += 1;
                } else {
                    self.st_facref_count += 1;
                }
                if self.steinerleft > 0 {
                    self.steinerleft -= 1;
                }
                success = true; // This subface has been split.
            } else {
                // Failed to insert this point.
                if ivf.iloc == LocateResult::NearVertex {
                    let chkpt = self.org(searchtet);
                    if !self.issteinerpoint(chkpt) {
                        self.warn(alloc::format!(
                            "Warning:  A {} and a vertex {} are very close.",
                            if splitseg_flag { "segment" } else { "facet" },
                            self.pointmark(chkpt)
                        ));
                        *dir = SELF_INTERSECT;
                    }
                }

                if *dir != SELF_INTERSECT && steinerflag >= 2 {
                    success = false; // queue this subface.
                                     // Failed to split a crossing edge/face.
                    if ivf.iloc == LocateResult::OnVertex || ivf.iloc == LocateResult::NearVertex {
                        if *dir == ACROSSEDGE {
                            let (ok, _idir) =
                                self.add_steinerpt_to_recover_edge(startpt, endpt, None, 0, 1)?;
                            if ok {
                                // Push this subface back to stack, to recover it again.
                                self.subfacstack.push(*searchsh);
                                success = true;
                            }
                        } else {
                            return Err(TetgenError::InternalError);
                        }
                    } else {
                        return Err(TetgenError::InternalError);
                    }
                }
                self.pointdealloc(steinerpt);
            }
        }
        Ok(success)
    }

    /// `getvertexstar(fullstar, searchpt, tetlist, vertlist, shlist)` with
    /// `tetlist = cavetetlist`, `vertlist = cavetetvertlist`, no `shlist`.
    pub fn getvertexstar(&mut self, fullstar: bool, searchpt: Point) -> usize {
        self.reach[32] += 1;
        let mut searchtet = self.point2tetorg(searchpt);
        // Go to the opposite face (the link face) of the vertex.
        enextesymself(&mut searchtet);
        self.infect(searchtet.tet); // Collect this tet (link face).
        self.cavetetlist.push(searchtet);
        // Collect three (link) vertices.
        let j = searchtet.ver & 3;
        for i in 1..4 {
            let pt = self.tets[searchtet.tet].v[(j + i) % 4];
            self.pinfect(pt);
            self.cavetetvertlist.push(pt);
        }

        let mut collectflag = true;
        let mut neightet = esym(&searchtet);
        if self.issubface(&neightet) && !fullstar {
            collectflag = false;
        }
        if collectflag {
            self.fsymself(&mut neightet); // Goto the adj tet of this face.
            esymself(&mut neightet); // Goto the oppo face of this vertex.
            self.infect(neightet.tet); // Collect this tet (link face).
            self.cavetetlist.push(neightet);
            // Collect its apex.
            let pt = self.apex(&neightet);
            self.pinfect(pt);
            self.cavetetvertlist.push(pt);
        }

        // Continue to collect all tets in the star.
        let mut i = 0;
        while i < self.cavetetlist.len() {
            let mut searchtet = self.cavetetlist[i];
            for _ in 0..2 {
                let mut collectflag = true;
                enextself(&mut searchtet);
                let mut neightet = esym(&searchtet);
                if self.issubface(&neightet) && !fullstar {
                    collectflag = false;
                }
                if collectflag {
                    self.fsymself(&mut neightet);
                    if !self.infected(neightet.tet) {
                        esymself(&mut neightet); // Go to the face opposite to 'searchpt'.
                        self.infect(neightet.tet);
                        self.cavetetlist.push(neightet);
                        // Check if a vertex is collected.
                        let pt = self.apex(&neightet);
                        if !self.pinfected(pt) {
                            self.pinfect(pt);
                            self.cavetetvertlist.push(pt);
                        }
                    }
                }
            }
            i += 1;
        }

        // Uninfect the list of tets and vertices.
        for i in 0..self.cavetetlist.len() {
            let t = self.cavetetlist[i].tet;
            self.uninfect(t);
        }
        for i in 0..self.cavetetvertlist.len() {
            let p = self.cavetetvertlist[i];
            self.puninfect(p);
        }
        self.cavetetlist.len()
    }

    /// `reduceedgesatvertex(startpt, endptlist)` on `cavetetvertlist`:
    /// flip away the edges at `startpt`; returns how many remain.
    pub fn reduceedgesatvertex(&mut self, startpt: Point) -> Result<usize, TetgenError> {
        self.reach[33] += 1;
        let mut fc = FlipConstraints {
            checkflipeligibility: true,
            remvert: startpt,
            ..Default::default()
        };
        let mut endptlist = core::mem::take(&mut self.cavetetvertlist);

        loop {
            let mut count = 0;
            let mut i: isize = 0;
            while (i as usize) < endptlist.len() {
                let pendpt = endptlist[i as usize];
                if pendpt == DUMMYPOINT {
                    i += 1;
                    continue; // Do not reduce a virtual edge.
                }
                let mut reduceflag = false;
                // Find the edge.
                let mut searchtet = TriFace::default();
                let dir;
                if self.nonconvex {
                    dir = if self.getedge(startpt, pendpt, &mut searchtet)? {
                        ACROSSVERT
                    } else {
                        // The edge does not exist (was flipped).
                        INTERSECT
                    };
                } else {
                    searchtet = self.point2tetorg(startpt);
                    dir = self.finddirection(&mut searchtet, pendpt)?;
                }
                if dir == ACROSSVERT {
                    if self.dest(&searchtet) == pendpt {
                        // Do not flip a segment.
                        if !self.issubseg(&searchtet) {
                            let n = self.removeedgebyflips(&mut searchtet, &mut fc)?;
                            if n == 2 {
                                reduceflag = true;
                            }
                        }
                    } else {
                        self.cavetetvertlist = endptlist;
                        return Err(TetgenError::InternalError);
                    }
                } else {
                    // The edge has been flipped.
                    reduceflag = true;
                }
                if reduceflag {
                    count += 1;
                    // Move the last vertex into this slot.
                    let j = endptlist.len() - 1;
                    endptlist[i as usize] = endptlist[j];
                    endptlist.pop();
                    i -= 1;
                }
                i += 1;
            }
            if count == 0 {
                // No edge is reduced.
                break;
            }
        }

        let n = endptlist.len();
        self.cavetetvertlist = endptlist;
        Ok(n)
    }
}

impl TetgenMesh {
    /// `sremovevertex(delpt, parentsh, parentseg, lawson)`: remove a vertex
    /// from the surface mesh (from a segment if `parentseg` is given: it is
    /// `[p, b]` and returns the restored segment `[a, b]`). The new
    /// subfaces are returned in `caveshbdlist`.
    pub fn sremovevertex(
        &mut self,
        delpt: Point,
        parentsh: &mut Face,
        parentseg: Option<&mut Face>,
        lawson: bool,
    ) -> Result<(), TetgenError> {
        self.reach[29] += 1;
        if let Some(parentseg) = parentseg {
            self.reach[41] += 1;
            // 'delpt' (p) is a Steiner point in a segment [a,b]; 'parentseg'
            //   is [p,b]. Find the segment [a,p].
            let mut prevseg = self.spivot(&senext2(parentseg));
            prevseg.shver = 0;
            // Restore the original segment [a,b].
            let pa = self.sorg(&prevseg);
            let pb = self.sdest(parentseg);
            let abseg = self.makeshellface(ShPool::Seg);
            self.setshvertices(&abseg, pa, pb, NULL_POINT);
            let mk = self.shellmark(parentseg);
            self.setshellmark(&abseg, mk);
            let fi = self.getfacetindex(parentseg);
            self.setfacetindex(&abseg, fi);
            // Connect [#, a]<->[a, b].
            let mut adjseg1 = self.spivot(&senext2(&prevseg));
            if !adjseg1.is_null() {
                adjseg1.shver = 0;
                adjseg1 = senext(&adjseg1);
                let adjseg2 = senext2(&abseg);
                self.sbond(&adjseg1, &adjseg2);
            }
            // Connect [a, b]<->[b, #].
            let mut adjseg1 = self.spivot(&senext(parentseg));
            if !adjseg1.is_null() {
                adjseg1.shver = 0;
                adjseg1 = senext2(&adjseg1);
                let adjseg2 = senext(&abseg);
                self.sbond(&adjseg1, &adjseg2);
            }
            // Update the point-to-segment map.
            self.setpoint2sh(pa, sencode(&abseg));
            self.setpoint2sh(pb, sencode(&abseg));

            // Get the faces in face ring at segment [p, b]. Re-use 'caveshlist'.
            *parentsh = self.spivot(parentseg);
            if !parentsh.is_null() {
                let mut spinsh = *parentsh;
                loop {
                    self.caveshlist.push(spinsh);
                    spinsh = self.spivot(&spinsh);
                    if spinsh.is_null() {
                        break; // It is possible there is only one facet.
                    }
                    if spinsh.same_sh(parentsh) {
                        break;
                    }
                }
            }

            // Create the face ring of the new segment [a,b].
            for i in 0..self.caveshlist.len() {
                let mut startsh = self.caveshlist[i];
                if self.sorg(&startsh) != delpt {
                    startsh = sesym(&startsh);
                }
                // startsh is [p, b, #1], find the subface [a, p, #2].
                let mut neighsh = startsh;
                loop {
                    neighsh = senext2(&neighsh);
                    let checkseg = self.sspivot(&neighsh);
                    if !checkseg.is_null() {
                        // It must be the segment [a, p].
                        break;
                    }
                    neighsh = self.spivot(&neighsh);
                    if self.sorg(&neighsh) != delpt {
                        neighsh = sesym(&neighsh);
                    }
                }
                // Now neighsh is [a, p, #2].
                let fakesh;
                if !neighsh.same_sh(&startsh) {
                    // Detach the two subsegments [a,p] and [p,b] from subfaces.
                    self.ssdissolve(&startsh);
                    self.ssdissolve(&neighsh);
                    // Create a degenerated subface [a,b,p].
                    let f = self.makeshellface(ShPool::Sub);
                    self.setshvertices(&f, pa, pb, delpt);
                    let mk = self.shellmark(&startsh);
                    self.setshellmark(&f, mk);
                    // Connect fakesh to the segment [a,b].
                    self.ssbond(&f, &abseg);
                    // Connect fakesh to adjacent subfaces: [p,b,#1] and [a,p,#2].
                    let nextsh = senext(&f);
                    self.sbond(&nextsh, &startsh);
                    let nextsh = senext2(&f);
                    self.sbond(&nextsh, &neighsh);
                    self.smarktest(&f); // Mark it as faked.
                    fakesh = f;
                } else {
                    // Special case. There exists already a degenerated face [a,b,p]!
                    neighsh = senext2(&neighsh); // [a,b,p]
                    let mut st = self.spivot(&neighsh); // The original adjacent subface.
                    if self.sorg(&st) != pa {
                        st = sesym(&st);
                    }
                    self.sdissolve(&st);
                    // Connect fakesh to the segment [a,b].
                    self.ssbond(&st, &abseg);
                    fakesh = st; // Do not mark it!
                                 // Delete the degenerated subface.
                    self.shellfacedealloc(ShPool::Sub, neighsh.sh);
                }
                // Save the fakesh in list (for re-creating the face ring).
                self.cavesegshlist.push(fakesh);
            }
            self.caveshlist.clear();

            // Re-create the face ring.
            let n = self.cavesegshlist.len();
            if n > 1 {
                for i in 0..n {
                    let fakesh = self.cavesegshlist[i];
                    let nextsh = self.cavesegshlist[(i + 1) % n];
                    self.sbond1(&fakesh, &nextsh);
                }
            }

            // Delete the two subsegments containing p.
            self.shellfacedealloc(ShPool::Seg, parentseg.sh);
            self.shellfacedealloc(ShPool::Seg, prevseg.sh);
            // Return the new segment.
            *parentseg = abseg;
        } else {
            // p is inside the surface. Let 'delpt' be its apex.
            *parentsh = senext(parentsh);
            // For unifying the code, we add parentsh to list.
            self.cavesegshlist.push(*parentsh);
        }

        // Remove the point (p).
        let mut it = 0;
        while it < self.cavesegshlist.len() {
            let mut psh = self.cavesegshlist[it]; // [a,b,p]
            psh = senext(&psh); // [b,p,a].
            psh = self.spivot(&psh);
            if self.sorg(&psh) != delpt {
                psh = sesym(&psh);
            }
            self.cavesegshlist[it] = psh;
            // now parentsh is [p,b,#].
            if self.sorg(&psh) != delpt {
                // The vertex has already been removed in above special case.
                it += 1;
                continue;
            }

            loop {
                // Initialize the flip edge list. Re-use 'caveshlist'.
                let mut spinsh = psh; // [p, b, #]
                loop {
                    self.caveshlist.push(spinsh);
                    spinsh = senext2(&spinsh);
                    spinsh = self.spivot(&spinsh);
                    if spinsh.same_sh(&psh) {
                        break;
                    }
                    if self.sorg(&spinsh) != delpt {
                        spinsh = sesym(&spinsh);
                    }
                }

                if self.caveshlist.len() == 3 {
                    // Delete the point by a 3-to-1 flip.
                    let mut flipfaces = [
                        self.caveshlist[0],
                        self.caveshlist[1],
                        self.caveshlist[2],
                        Face::default(),
                    ];
                    self.flip31(&mut flipfaces, lawson);
                    for f in flipfaces.iter().take(3) {
                        self.shellfacedealloc(ShPool::Sub, f.sh);
                    }
                    self.caveshlist.clear();
                    // Save the new subface.
                    self.caveshbdlist.push(flipfaces[3]);
                    // The vertex is removed.
                    break;
                }

                // Search an edge to flip.
                let n = self.caveshlist.len();
                let mut i = 0;
                while i < n {
                    let f0 = self.caveshlist[i];
                    let mut f1 = self.spivot(&f0);
                    if self.sorg(&f0) != self.sdest(&f1) {
                        f1 = sesym(&f1);
                    }
                    // Skip this edge if it belongs to a faked subface.
                    if !self.smarktested(&f0) && !self.smarktested(&f1) {
                        let pa = self.sorg(&f0);
                        let pb = self.sdest(&f0);
                        let pc = self.sapex(&f0);
                        let pd = self.sapex(&f1);
                        let xd = self.x(pd);
                        self.calculateabovepoint4(pa, pb, pc, &xd);
                        // Check if a 2-to-2 flip is possible.
                        let ori1 = self.ori3(pc, pd, DUMMYPOINT, pa);
                        let ori2 = self.ori3(pc, pd, DUMMYPOINT, pb);
                        if ori1 * ori2 < 0.0 {
                            // A 2-to-2 flip is found.
                            let mut flipfaces = [f0, f1];
                            self.flip22(&mut flipfaces, lawson);
                            // The 'flipfaces[1]' contains p as its apex.
                            psh = senext2(&flipfaces[1]);
                            self.cavesegshlist[it] = psh;
                            // Save the new subface.
                            self.caveshbdlist.push(flipfaces[0]);
                            break;
                        }
                    }
                    i += 1;
                }

                if i == n {
                    // Do a flip22 and a flip31 to remove p.
                    let f0 = self.caveshlist[0];
                    let mut f1 = self.spivot(&f0);
                    if self.sorg(&f0) != self.sdest(&f1) {
                        f1 = sesym(&f1);
                    }
                    let mut flipfaces = [f0, f1];
                    self.flip22(&mut flipfaces, lawson);
                    psh = senext2(&flipfaces[1]);
                    self.cavesegshlist[it] = psh;
                    // Save the new subface.
                    self.caveshbdlist.push(flipfaces[0]);
                }

                // The edge list at p are changed.
                self.caveshlist.clear();
            }
            it += 1;
        }

        self.cavesegshlist.clear();

        if lawson {
            self.lawsonflip();
        }
        Ok(())
    }

    /// Detach a subsegment from every tet at its edge (`tssdissolve1` around
    /// it, then `sstdissolve1`).
    fn detach_segment(&mut self, checkseg: &Face) {
        let neightet = self.sstpivot1(checkseg);
        let mut spintet = neightet;
        loop {
            self.tssdissolve1(&spintet);
            self.fnextself(&mut spintet);
            if spintet.tet == neightet.tet {
                break;
            }
        }
        self.sstdissolve1(checkseg);
    }

    /// `removevertexbyflips(steinerpt)`: remove a Steiner point by flips
    /// (reduce its edges, then a 4-to-1, 6-to-2 or 2n-to-n flip). True if
    /// removed (or, on a segment before facet recovery, shifted into the
    /// volume).
    pub fn removevertexbyflips(&mut self, steinerpt: Point) -> Result<bool, TetgenError> {
        self.reach[27] += 1;
        let mut fc = FlipConstraints::default();
        let vt = self.pointtype(steinerpt);
        let mut leftseg = Face::default();
        let mut rightseg = Face::default();
        let mut lpt = NULL_POINT;
        let mut rpt = NULL_POINT;

        if vt == VertType::FreeSeg {
            leftseg = sdecode(self.point2sh(steinerpt));
            leftseg.shver = 0;
            if self.sdest(&leftseg) == steinerpt {
                rightseg = self.spivot(&senext(&leftseg));
                rightseg.shver = 0;
            } else {
                rightseg = leftseg;
                leftseg = self.spivot(&senext2(&rightseg));
                leftseg.shver = 0;
            }
            lpt = self.sorg(&leftseg);
            rpt = self.sdest(&rightseg);
            // Check if both leftseg and rightseg are recovered in tet mesh.
            if self.sstpivot1(&leftseg).tet == NULL_TET {
                return Ok(false);
            }
            if self.sstpivot1(&rightseg).tet == NULL_TET {
                return Ok(false);
            }
        } else if vt == VertType::FreeFacet || vt == VertType::FreeVol || vt == VertType::Vol {
            // (only printouts upstream)
        } else {
            // It is not a Steiner point.
            return Ok(false);
        }

        // Try to reduce the number of edges at 'p' by flips.
        self.getvertexstar(true, steinerpt);
        self.cavetetlist.clear(); // This list may be re-used.
        let mut valence = if self.cavetetvertlist.len() > 3 {
            self.reduceedgesatvertex(steinerpt)?
        } else {
            self.cavetetvertlist.len()
        };
        self.cavetetvertlist.clear();

        let mut removeflag = false;
        let mut loc = LocateResult::Unknown;
        let mut searchtet = TriFace::default();
        let mut wrktets = [TriFace::default(); 4];

        if valence == 4 {
            // Only 4 vertices (4 tets) left!
            searchtet = self.point2tetorg(steinerpt);
            loc = LocateResult::InTetrahedron;
            removeflag = true;
        } else if valence == 5 {
            // There are 5 edges.
            if vt == VertType::FreeSeg {
                searchtet = self.sstpivot1(&leftseg);
                if self.org(&searchtet) != steinerpt {
                    esymself(&mut searchtet);
                }
                let mut i = 0; // Count the numbe of tet at the edge [p,lpt].
                let mut neightet = TriFace::default(); // Init the face.
                let mut spintet = searchtet;
                loop {
                    i += 1;
                    if self.apex(&spintet) == rpt {
                        // Remember the face containing the edge [lpt, rpt].
                        neightet = spintet;
                    }
                    self.fnextself(&mut spintet);
                    if spintet.tet == searchtet.tet {
                        break;
                    }
                }
                if i == 4 {
                    // There must be a face [p, lpt, rpt].
                    if self.apex(&neightet) == rpt {
                        // Check if a 6-to-2 flip is possible (to remove 'p').
                        searchtet = esym(&neightet);
                        enextself(&mut searchtet);
                        wrktets[0] = searchtet; // [p,d,a,b]
                        for i in 0..2 {
                            wrktets[i + 1] = self.fnext(&wrktets[i]); // [p,d,b,c], [p,d,c,a]
                        }
                        if self.apex(&wrktets[0]) == self.oppo(&wrktets[2]) {
                            loc = LocateResult::OnFace;
                            removeflag = true;
                        }
                    }
                }
            } else if vt == VertType::FreeFacet {
                // It is possible to do a 6-to-2 flip to remove the vertex.
                searchtet = self.point2tetorg(steinerpt);
                wrktets[0] = searchtet;
                wrktets[1] = searchtet;
                esymself(&mut wrktets[1]);
                enextself(&mut wrktets[1]);
                wrktets[2] = searchtet;
                eprevself(&mut wrktets[2]);
                esymself(&mut wrktets[2]);
                // Get one edge which has valance 3.
                searchtet.tet = NULL_TET;
                for w in wrktets.iter().take(3) {
                    let mut spintet = *w;
                    valence = 0;
                    loop {
                        valence += 1;
                        self.fnextself(&mut spintet);
                        if spintet.tet == w.tet {
                            break;
                        }
                    }
                    if valence == 3 {
                        // Found the edge.
                        searchtet = *w;
                        break;
                    }
                }
                loc = LocateResult::OnFace;
                removeflag = true;
            }
        }

        if !removeflag && vt == VertType::FreeSeg {
            // Check is it possible to recover the edge [lpt,rpt].
            searchtet = self.sstpivot1(&leftseg);
            if self.org(&searchtet) != steinerpt {
                esymself(&mut searchtet);
            }
            let mut spintet = searchtet;
            loop {
                // Go to the bottom face of this tet.
                let mut neightet = eprev(&spintet);
                esymself(&mut neightet); // [steinerpt, p1, p2, lpt]
                                         // Get the adjacent tet.
                self.fsymself(&mut neightet); // [p1, steinerpt, p2, rpt]
                if self.oppo(&neightet) != rpt {
                    // Found a non-matching adjacent tet.
                    break;
                }
                {
                    // [2017-10-15] Check if the tet is inverted?
                    let chkp1 = self.org(&neightet);
                    let chkp2 = self.apex(&neightet);
                    let chkori = self.ori3(rpt, lpt, chkp1, chkp2);
                    if chkori >= 0.0 {
                        // Either inverted or degenerated.
                        break;
                    }
                }
                self.fnextself(&mut spintet);
                if spintet.tet == searchtet.tet {
                    // 'searchtet' is [p,d,p1,p2].
                    loc = LocateResult::OnEdge;
                    removeflag = true;
                    break;
                }
            }
        }

        if !removeflag && vt == VertType::FreeSeg {
            // Check if the edge [lpt, rpt] exists.
            if self.getedge(lpt, rpt, &mut searchtet)? && !self.checksubfaceflag {
                // We have recovered this edge. Shift the vertex into the volume.
                // Detach the subsegments from their surrounding tets.
                self.detach_segment(&leftseg);
                self.detach_segment(&rightseg);
                let mut parentsh = self.spivot(&rightseg); // 'rightseg' has p as its origin.
                self.sremovevertex(steinerpt, &mut parentsh, Some(&mut rightseg), true)?;
                // Clear the list for new subfaces.
                self.caveshbdlist.clear();
                // Insert the new segment.
                self.sstbond1(&rightseg, &searchtet);
                let mut spintet = searchtet;
                loop {
                    self.tssbond1(&spintet, &rightseg);
                    self.fnextself(&mut spintet);
                    if spintet.tet == searchtet.tet {
                        break;
                    }
                }
                // The Steiner point has been shifted into the volume.
                self.reach[49] += 1;
                self.setpointtype(steinerpt, VertType::FreeVol);
                self.st_segref_count -= 1;
                self.st_volref_count += 1;
                return Ok(true);
            }
        }

        if !removeflag {
            return Ok(false);
        }

        if vt == VertType::FreeSeg {
            // Detach the subsegments from their surronding tets.
            self.detach_segment(&leftseg);
            self.detach_segment(&rightseg);
            if self.checksubfaceflag {
                // Detach the subfaces at the subsegments from their attached tets.
                for checkseg in [leftseg, rightseg] {
                    let parentsh = self.spivot(&checkseg);
                    if !parentsh.is_null() {
                        let mut spinsh = parentsh;
                        loop {
                            let neightet = self.stpivot(&spinsh);
                            if neightet.tet != NULL_TET {
                                self.tsdissolve(&neightet);
                            }
                            spinsh = sesym(&spinsh);
                            let neightet = self.stpivot(&spinsh);
                            if neightet.tet != NULL_TET {
                                self.tsdissolve(&neightet);
                            }
                            self.stdissolve(&spinsh);
                            spinsh = self.spivot(&spinsh); // Go to the next subface.
                            if spinsh.same_sh(&parentsh) {
                                break;
                            }
                        }
                    }
                }
            }
        }

        if loc == LocateResult::InTetrahedron {
            self.reach[48] += 1;
            // Collect the four tets containing 'p'.
            let mut fliptets = [TriFace::default(); 4];
            fliptets[0] = searchtet; // [p,d,a,b]
            for i in 0..2 {
                fliptets[i + 1] = self.fnext(&fliptets[i]); // [p,d,b,c], [p,d,c,a]
            }
            fliptets[3] = eprev(&fliptets[0]);
            self.fnextself(&mut fliptets[3]); // it is [a,p,b,c]
            eprevself(&mut fliptets[3]);
            esymself(&mut fliptets[3]); // [a,b,c,p].
            if vt == VertType::FreeFacet && !self.check_last_flip41(&fliptets) {
                return Ok(false);
            }
            self.flip41(&mut fliptets, 1, &mut fc)?;
        } else if loc == LocateResult::OnFace {
            self.reach[47] += 1;
            // Collect the six tets containing 'p'.
            let mut fliptets = [TriFace::default(); 6];
            fliptets[0] = searchtet; // [p,d,a,b]
            for i in 0..2 {
                fliptets[i + 1] = self.fnext(&fliptets[i]); // [p,d,b,c], [p,d,c,a]
            }
            fliptets[3] = eprev(&fliptets[0]);
            self.fnextself(&mut fliptets[3]); // [a,p,b,e]
            esymself(&mut fliptets[3]); // [p,a,e,b]
            eprevself(&mut fliptets[3]); // [e,p,a,b]
            for i in 3..5 {
                fliptets[i + 1] = self.fnext(&fliptets[i]); // [e,p,b,c], [e,p,c,a]
            }
            if vt == VertType::FreeFacet {
                // We need to determine the location of three subfaces at p.
                let mut v = 0;
                for t in fliptets.iter().skip(3) {
                    if self.issubface(t) {
                        v += 1;
                    }
                }
                if v > 0 {
                    // We must do 3-to-2 flip in the upper part.
                    for i in 0..3 {
                        wrktets[i] = esym(&fliptets[i + 3]);
                        fliptets[i + 3] = esym(&fliptets[i]);
                        fliptets[i] = wrktets[i];
                    }
                    // Swap the last two pairs, i.e., [1]<->[[2], and [4]<->[5]
                    fliptets.swap(1, 2);
                    fliptets.swap(4, 5);
                }
                let four = [fliptets[0], fliptets[1], fliptets[2], fliptets[3]];
                if !self.check_last_flip41(&four) {
                    return Ok(false);
                }
            }
            // Remove p by a 6-to-2 flip: a 3-to-2 then a 4-to-1.
            let mut upper = [fliptets[3], fliptets[4], fliptets[5]];
            self.flip32(&mut upper, 1, &mut fc)?;
            fliptets[3] = upper[0];
            fliptets[4] = upper[1];
            fliptets[5] = upper[2];
            let mut four = [fliptets[0], fliptets[1], fliptets[2], fliptets[3]];
            self.flip41(&mut four, 1, &mut fc)?;
        } else if loc == LocateResult::OnEdge {
            self.reach[46] += 1;
            // Count the number of tets at edge [e,p] and [p,d] (this is n).
            let mut n = 0;
            let mut spintet = searchtet;
            loop {
                n += 1;
                self.fnextself(&mut spintet);
                if spintet.tet == searchtet.tet {
                    break;
                }
            }
            // Collect the 2n tets containing 'p'.
            let mut fliptets = alloc::vec![TriFace::default(); 2 * n];
            fliptets[0] = searchtet; // [p,b,p_0,p_1]
            for i in 0..n - 1 {
                fliptets[i + 1] = self.fnext(&fliptets[i]); // [p,d,p_i,p_i+1].
            }
            fliptets[n] = eprev(&fliptets[0]);
            self.fnextself(&mut fliptets[n]); // [p_0,p,p_1,e]
            esymself(&mut fliptets[n]); // [p,p_0,e,p_1]
            eprevself(&mut fliptets[n]); // [e,p,p_0,p_1]
            for i in n..2 * n - 1 {
                fliptets[i + 1] = self.fnext(&fliptets[i]); // [e,p,p_i,p_i+1].
            }
            // Remove p by a 2n-to-n flip.
            let mut w = [TriFace::default(); 3];
            w[0] = fliptets[0]; // [p,d,p_0,p_1]
            eprevself(&mut w[0]); // [p_0,p,d,p_1]
            esymself(&mut w[0]); // [p,p_0,p_1,d]
            enextself(&mut w[0]); // [p_0,p_1,p,d] [0]
            w[1] = fliptets[n]; // [e,p,p_0,p_1]
            enextself(&mut w[1]); // [p,p_0,e,p_1]
            esymself(&mut w[1]); // [p_0,p,p_1,e]
            eprevself(&mut w[1]); // [p_1,p_0,p,e] [1]
            self.flip23(&mut w, 1, &mut fc);
            // Save the new tet [e,d,p,p_0] (degenerated).
            fliptets[n] = w[2];
            // Save the new tet [e,d,p_0,p_1].
            fliptets[0] = w[0];
            // - Repeat from i = 1 to n-2: (n - 2) flips
            for i in 1..n - 1 {
                w[0] = w[1]; // [e,d,p_i,p] (degenerated).
                enextself(&mut w[0]); // [d,p_i,e,p] (...)
                esymself(&mut w[0]); // [p_i,d,p,e] (...)
                eprevself(&mut w[0]); // [p,p_i,d,e] (degenerated) [0].
                w[1] = fliptets[n + i]; // [e,p,p_i,p_i+1]
                enextself(&mut w[1]); // [p,p_i,e,p_i+1] [1]
                w[2] = fliptets[i]; // [p,d,p_i,p_i+1]
                eprevself(&mut w[2]); // [p_i,p,d,p_i+1]
                esymself(&mut w[2]); // [p,p_i,p_i+1,d] [2]
                self.flip32(&mut w, 1, &mut fc)?;
                fliptets[i] = w[0]; // [d,e,p_i+1,p_i]
                esymself(&mut fliptets[i]); // [e,d,p_i,p_i+1]
            }
            // - Do a 4-to-1 flip.
            let mut w4 = [TriFace::default(); 4];
            w4[3] = w[1]; // [e,d,p_n-1,p] (degenerated) [3]
            w4[0] = fliptets[n]; // [e,d,p,p_0] (degenerated)
            eprevself(&mut w4[0]); // [p,e,d,p_0] (...)
            esymself(&mut w4[0]); // [e,p,p_0,d] (...)
            enextself(&mut w4[0]); // [p,p_0,e,d] (degenerated) [0]
            w4[1] = fliptets[n - 1]; // [p,d,p_n-1,p_0]
            esymself(&mut w4[1]); // [d,p,p_0,p_n-1]
            enextself(&mut w4[1]); // [p,p_0,d,p_n-1] [1]
            w4[2] = fliptets[2 * n - 1]; // [e,p,p_n-1,p_0]
            enextself(&mut w4[2]); // [p_p_n-1,e,p_0]
            esymself(&mut w4[2]); // [p_n-1,p,p_0,e]
            enextself(&mut w4[2]); // [p,p_0,p_n-1,e] [2]
            self.flip41(&mut w4, 1, &mut fc)?;
        }

        if vt == VertType::FreeSeg {
            // Remove the vertex from the surface mesh.
            let slawson = !self.checksubfaceflag;
            let mut parentsh = self.spivot(&rightseg); // 'rightseg' has p as its origin.
            self.sremovevertex(steinerpt, &mut parentsh, Some(&mut rightseg), slawson)?;

            // The original segment is returned in 'rightseg'.
            rightseg.shver = 0;
            // Insert the new segment.
            let mut searchtet = self.point2tetorg(lpt);
            self.finddirection(&mut searchtet, rpt)?;
            if self.dest(&searchtet) != rpt {
                return Err(TetgenError::InternalError);
            }
            self.sstbond1(&rightseg, &searchtet);
            let mut spintet = searchtet;
            loop {
                self.tssbond1(&spintet, &rightseg);
                self.fnextself(&mut spintet);
                if spintet.tet == searchtet.tet {
                    break;
                }
            }

            if self.checksubfaceflag {
                // Insert subfaces at segment [lpt,rpt] into the tetrahedralization.
                let parentsh = self.spivot(&rightseg);
                if !parentsh.is_null() {
                    let mut spinsh = parentsh;
                    loop {
                        if self.sorg(&spinsh) != lpt {
                            spinsh = sesym(&spinsh);
                        }
                        let apexpt = self.sapex(&spinsh);
                        // Find the adjacent tet of [lpt,rpt,apexpt];
                        let mut spintet = searchtet;
                        loop {
                            if self.apex(&spintet) == apexpt {
                                self.tsbond(&spintet, &spinsh);
                                spinsh = sesym(&spinsh); // Get to another side of this face.
                                let neightet = self.fsym(&spintet);
                                self.tsbond(&neightet, &spinsh);
                                spinsh = sesym(&spinsh); // Get back to the original side.
                                break;
                            }
                            self.fnextself(&mut spintet);
                        }
                        spinsh = self.spivot(&spinsh);
                        if spinsh.same_sh(&parentsh) {
                            break;
                        }
                    }
                }
            }

            // Clear the set of new subfaces.
            self.caveshbdlist.clear();
        }

        // The point has been removed.
        if self.pointtype(steinerpt) != VertType::Unused {
            self.setpointtype(steinerpt, VertType::Unused);
            self.unuverts += 1;
        }
        if vt != VertType::Vol {
            if vt == VertType::FreeSeg {
                self.st_segref_count -= 1;
            } else if vt == VertType::FreeFacet {
                self.st_facref_count -= 1;
            } else if vt == VertType::FreeVol {
                self.st_volref_count -= 1;
            }
            if self.steinerleft > 0 {
                self.steinerleft += 1;
            }
        }

        Ok(true)
    }

    /// `removevertexbyflips`' [2018-03-08] check that the last 4-to-1 flip
    /// of a facet vertex is valid (upstream's stale `chkface` in the second
    /// test is kept).
    fn check_last_flip41(&self, fliptets: &[TriFace; 4]) -> bool {
        let mut chkface = TriFace::default();
        let mut i = 0;
        while i < 3 {
            let mut checktet = enext(&fliptets[i]);
            esymself(&mut checktet); // [a,d,b,p],[b,d,c,p],[c,d,a,p]
            let mut scount = 0;
            for _ in 0..3 {
                chkface = esym(&checktet);
                if self.issubface(&chkface) {
                    scount += 1;
                }
                enextself(&mut checktet);
            }
            if scount == 3 {
                break; // Found a tet which support a 3-to-1 flip.
            } else if scount == 2 {
                // This is a strange configuration. Do not do this flip.
                return false;
            }
            i += 1;
        }
        if i == 3 {
            // No tet in [p,d,a,b],[p,d,b,c],[p,d,c,a] support it.
            let mut scount = 0;
            for _ in 0..3 {
                // (upstream tests 'chkface', not the 'checktet' it computes)
                if self.issubface(&chkface) {
                    scount += 1;
                }
            }
            if scount != 3 {
                return false;
            }
        }
        true
    }

    /// `lawsonflip3d(fc)`: flip the locally non-Delaunay faces on the flip
    /// stack; unflippable ones are queued (`later_unflip_queue`).
    pub fn lawsonflip3d(&mut self, fc: &mut FlipConstraints) -> Result<i64, TetgenError> {
        let mut totalcount = 0i64;
        let mut sliver_peels = 0i64;
        // The unflip queue (upstream's 'unflip_queue_front' list; its items
        //   are 'flippool' items too).
        let mut unflip_queue: Vec<(TriFace, Point, Point, Point)> = Vec::new();

        while self.tflipstack.len() + unflip_queue.len() != 0 {
            let mut flipcount = 0i64;

            while let Some(popface) = self.tflipstack.pop() {
                let mut fliptets = [TriFace::default(); 5];
                fliptets[0] = popface;

                if self.isdeadtet(fliptets[0].tet) {
                    continue;
                }
                if !self.facemarked(&fliptets[0]) {
                    continue;
                }
                self.unmarkface(&fliptets[0]);

                if self.ishulltet(fliptets[0].tet) {
                    continue;
                }

                fliptets[1] = self.fsym(&fliptets[0]);
                if self.ishulltet(fliptets[1].tet) {
                    if self.nonconvex {
                        self.lawson_hull_face(&fliptets[0], fc, &mut sliver_peels)?;
                    }
                    continue;
                }

                if self.checksubfaceflag && self.issubface(&fliptets[0]) {
                    continue;
                }

                let p = self.tets[fliptets[1].tet].v;
                let o0 = self.oppo(&fliptets[0]);
                let sign = self.insphere_s(p[0], p[1], p[2], p[3], o0)?;

                if sign < 0.0 {
                    let pd = self.oppo(&fliptets[0]);
                    let pe = self.oppo(&fliptets[1]);
                    let mut len3 = distance(&self.x(pd), &self.x(pe));
                    len3 = len3 * len3 * len3;
                    let mut round_flag = false;
                    let mut ori = 0.0;
                    for _ in 0..3 {
                        ori = self.ori3(self.org(&fliptets[0]), self.dest(&fliptets[0]), pd, pe);
                        if ori > 0.0 {
                            fliptets[2] = esym(&fliptets[0]);
                            fliptets[3] = esym(&fliptets[1]);
                            if self.issubface(&fliptets[2]) || self.issubface(&fliptets[3]) {
                                let vol = orient3dfast(
                                    &self.x(self.org(&fliptets[0])),
                                    &self.x(self.dest(&fliptets[0])),
                                    &self.x(pd),
                                    &self.x(pe),
                                );
                                if vol.abs() / len3 < self.b.epsilon {
                                    ori = 0.0; // Do rounding.
                                    round_flag = true;
                                }
                            }
                        }
                        if ori <= 0.0 {
                            break;
                        }
                        enextself(&mut fliptets[0]);
                        eprevself(&mut fliptets[1]);
                    }

                    if ori > 0.0 {
                        let mut three = [fliptets[0], fliptets[1], fliptets[2]];
                        self.flip23(&mut three, 0, fc);
                        flipcount += 1;
                        if fc.remove_ndelaunay_edge {
                            self.tetprism_vol_sum += fc.tetprism_vol_sum;
                            fc.tetprism_vol_sum = 0.0;
                        }
                        continue;
                    } else {
                        // ori <= 0
                        if self.checksubsegflag && self.issubseg(&fliptets[0]) {
                            continue;
                        }
                        let mut scount = 0;
                        esymself(&mut fliptets[0]); // [b,a,d,c]
                        for i in 0..3 {
                            if self.issubface(&fliptets[i]) {
                                scount += 1;
                            }
                            fliptets[i + 1] = self.fnext(&fliptets[i]);
                        }
                        if fliptets[3].tet == fliptets[0].tet {
                            if scount == 1 {
                                continue;
                            } else if scount == 2 {
                                let mut i = 0;
                                while i < 3 {
                                    if !self.issubface(&fliptets[i]) {
                                        break;
                                    }
                                    i += 1;
                                }
                                let neightet = fliptets[(i + 1) % 3]; // (b,a,e,d)
                                let mut face1 = enext(&neightet);
                                esymself(&mut face1); // (e,a,d)
                                let mut face2 = eprev(&neightet);
                                esymself(&mut face2); // (b,e,d)
                                if self.issubface(&face1) || self.issubface(&face2) {
                                    continue;
                                }
                            }
                            let mut three = [fliptets[0], fliptets[1], fliptets[2]];
                            self.flip32(&mut three, 0, fc)?;
                            flipcount += 1;
                            if fc.remove_ndelaunay_edge {
                                self.tetprism_vol_sum += fc.tetprism_vol_sum;
                                fc.tetprism_vol_sum = 0.0;
                            }
                            continue;
                        } else {
                            fliptets[4] = self.fnext(&fliptets[3]);
                            if fliptets[4].tet == fliptets[0].tet {
                                if ori != 0.0
                                    && self.nonconvex
                                    && self.apex(&fliptets[3]) == DUMMYPOINT
                                {
                                    ori = 0.0;
                                    round_flag = true;
                                }
                                if ori == 0.0 {
                                    if self.issubface(&fliptets[0]) {
                                        // (a,b,d)
                                        if !self.issubface(&fliptets[2]) {
                                            continue; // not valid 2-2 flip.
                                        }
                                        if self.issubface(&fliptets[1])
                                            || self.issubface(&fliptets[3])
                                        {
                                            continue; // The surface mesh is degnerated.
                                        }
                                    } else if self.issubface(&fliptets[1])
                                        || self.issubface(&fliptets[2])
                                        || self.issubface(&fliptets[3])
                                    {
                                        continue; // not valid 2-2 flip.
                                    }

                                    if round_flag {
                                        let pb = self.org(&fliptets[0]);
                                        let pa = self.dest(&fliptets[0]);
                                        let pc = self.apex(&fliptets[1]);
                                        let pf = self.apex(&fliptets[3]); // pf may be dummypoint
                                        if self.is_collinear_at(pa, pd, pe)
                                            || self.is_collinear_at(pb, pd, pe)
                                        {
                                            continue; // avoid creating a degenerated (sub)face.
                                        }
                                        let o1 = self.ori3(pe, pd, pc, pa);
                                        let o2 = self.ori3(pe, pd, pb, pc);
                                        if o1 >= 0.0 || o2 >= 0.0 {
                                            continue; // inverted new tets
                                        }
                                        if pf != DUMMYPOINT {
                                            let o3 = self.ori3(pe, pd, pa, pf);
                                            let o4 = self.ori3(pe, pd, pf, pb);
                                            if o3 >= 0.0 || o4 >= 0.0 {
                                                continue; // inverted new tets
                                            }
                                        }
                                        let test_sign = self.insphere_s(pe, pd, pc, pa, pb)?;
                                        if test_sign < 0.0 {
                                            continue;
                                        }
                                        if pf != DUMMYPOINT {
                                            let test_sign = self.insphere_s(pe, pd, pf, pb, pa)?;
                                            if test_sign < 0.0 {
                                                continue;
                                            }
                                        }
                                    }
                                    esymself(&mut fliptets[0]); // [a,b,c,d]
                                    let mut three = [fliptets[0], fliptets[1], fliptets[2]];
                                    self.flip23(&mut three, 0, fc); // No hull tet.
                                    fliptets[0] = three[0];
                                    fliptets[1] = three[1];
                                    fliptets[2] = three[2];
                                    fliptets[1] = self.fnext(&fliptets[3]);
                                    fliptets[2] = self.fnext(&fliptets[1]);
                                    let hf = (self.apex(&fliptets[3]) == DUMMYPOINT) as i32;
                                    let mut upper = [fliptets[1], fliptets[2], fliptets[3]];
                                    self.flip32(&mut upper, hf, fc)?;
                                    flipcount += 1;
                                    self.flip23count -= 1;
                                    self.flip32count -= 1;
                                    self.flip44count += 1;
                                    self.reach[45] += 1;
                                    if fc.remove_ndelaunay_edge {
                                        self.tetprism_vol_sum += fc.tetprism_vol_sum;
                                        fc.tetprism_vol_sum = 0.0;
                                    }
                                    continue;
                                }
                            }
                        }
                        // Queue the original non-Delaunay face.
                        esymself(&mut fliptets[0]); // *** The original non-Delaunay face ****
                        let t = fliptets[0];
                        unflip_queue.push((t, self.org(&t), self.dest(&t), self.apex(&t)));
                    }
                }
            }

            totalcount += flipcount;

            if self.tflipstack.len() + unflip_queue.len() == 0 {
                break;
            }
            if flipcount == 0 {
                break;
            }

            for (tt, forg, fdest, fapex) in core::mem::take(&mut unflip_queue) {
                if !self.isdeadtet(tt.tet)
                    && self.org(&tt) == forg
                    && self.dest(&tt) == fdest
                    && self.apex(&tt) == fapex
                {
                    self.flippush(&tt);
                }
            }
        }

        if self.tflipstack.len() + unflip_queue.len() > 0 {
            for (tt, forg, fdest, fapex) in core::mem::take(&mut unflip_queue) {
                if !self.isdeadtet(tt.tet)
                    && self.org(&tt) == forg
                    && self.dest(&tt) == fdest
                    && self.apex(&tt) == fapex
                {
                    self.later_unflip_queue.push((tt, forg, fdest, fapex));
                }
            }
            // flippool->restart(): clear the pool.
            self.tflipstack.clear();
            self.flipstack.clear();
        }

        Ok(totalcount + sliver_peels)
    }

    /// `lawsonflip3d`'s non-convex hull branch (a face whose neighbour is a
    /// hull tet): peel a hull sliver or remove a boundary Steiner point.
    /// Reached only after `carveholes` (`nonconvex`), i.e. not in boundary
    /// recovery; ported for stage 5.
    fn lawson_hull_face(
        &mut self,
        f0: &TriFace,
        fc: &mut FlipConstraints,
        sliver_peels: &mut i64,
    ) -> Result<(), TetgenError> {
        let mut checksh = self.tspivot(f0);
        for _ in 0..3 {
            if !self.isshsubseg(&checksh) {
                let mut casingout = self.spivot(&checksh);
                if self.sorg(&checksh) != self.sdest(&casingout) {
                    casingout = sesym(&casingout);
                }
                let neightet = self.stpivot(&casingout);
                if neightet.tet == f0.tet {
                    let mut hulltet = edestoppo(&neightet); // [a,b,e,d]
                    self.fsymself(&mut hulltet); // [b,a,e,#]
                    if self.oppo(&hulltet) == DUMMYPOINT {
                        let pe = self.org(&neightet);
                        if matches!(self.pointtype(pe), VertType::FreeFacet | VertType::FreeSeg) {
                            self.reach5[9] += 1;
                            self.removevertexbyflips(pe)?;
                        }
                    } else {
                        let mut hulltet = eorgoppo(&neightet); // [b,a,d,e]
                        self.fsymself(&mut hulltet); // [a,b,d,#]
                        if self.oppo(&hulltet) == DUMMYPOINT {
                            let pd = self.dest(&neightet);
                            if matches!(self.pointtype(pd), VertType::FreeFacet | VertType::FreeSeg)
                            {
                                self.reach5[9] += 1;
                                self.removevertexbyflips(pd)?;
                            }
                        } else {
                            let chk_pe = self.org(&neightet);
                            let chk_pd = self.dest(&neightet);
                            let chk_pa = self.apex(&neightet);
                            let chk_pb = self.oppo(&neightet);
                            let n1 = super::surface::facenormal(
                                &self.x(chk_pa),
                                &self.x(chk_pb),
                                &self.x(chk_pe),
                                1,
                            );
                            let n2 = super::surface::facenormal(
                                &self.x(chk_pb),
                                &self.x(chk_pa),
                                &self.x(chk_pd),
                                1,
                            );
                            let d = n1[0] * n2[0] + n1[1] * n2[1] + n1[2] * n2[2];
                            if d > 0.0 {
                                // (Not upstream: the peeled sliver's volume, for
                                //   the domain-volume check of gh:#1794.)
                                let o = self.pred.orient3d(
                                    &self.x(chk_pe),
                                    &self.x(chk_pd),
                                    &self.x(chk_pa),
                                    &self.x(chk_pb),
                                );
                                self.hull_sliver_volume += o.abs() / 6.0;
                                let f1 = self.fnext(&neightet);
                                let f2 = self.fnext(&f1);
                                let mut three = [neightet, f1, f2];
                                self.flip32(&mut three, 1, fc)?;
                                self.flip32count -= 1;
                                self.flip22count -= 1;
                                *sliver_peels += 1;
                                self.reach5[8] += 1;
                                if fc.remove_ndelaunay_edge {
                                    self.tetprism_vol_sum += fc.tetprism_vol_sum;
                                    fc.tetprism_vol_sum = 0.0;
                                }
                            }
                        }
                    }
                    break;
                }
            }
            checksh = senext(&checksh);
        }
        Ok(())
    }

    /// `recoverdelaunay()`: restore local Delaunayness by Lawson's 3D flips,
    /// then try to flip the edges of the unflippable faces with flip link
    /// levels 1 to 3. (Stage 5's call after `carveholes` uses the same
    /// routine.)
    pub fn recoverdelaunay(&mut self) -> Result<(), TetgenError> {
        self.reach[26] += 1;
        let mut fc = FlipConstraints::default();
        self.tetprism_vol_sum = 0.0; // Initialize it.

        if !self.later_unflip_queue.is_empty() {
            // Flip the saved unflippable faces.
            for (tt, forg, fdest, fapex) in core::mem::take(&mut self.later_unflip_queue) {
                if !self.isdeadtet(tt.tet)
                    && self.org(&tt) == forg
                    && self.dest(&tt) == fdest
                    && self.apex(&tt) == fapex
                {
                    self.flippush(&tt);
                }
            }
            if self.flippool_items() == 0 {
                return Ok(());
            }
        } else if self.flippool_items() == 0 {
            // Flip all locally non-Delaunay faces of the tetrahedralisation.
            self.tetpool.traversalinit();
            while let Some(tet) = self.tetrahedrontraverse() {
                for ver in 0..4 {
                    let tetloop = TriFace { tet, ver };
                    let neightet = decode(self.tets[tet].nbr[ver]);
                    if !self.facemarked(&neightet) {
                        self.flippush(&tetloop);
                    }
                }
                let v = self.tets[tet].v;
                let (a, b, c, d) = (self.x(v[0]), self.x(v[1]), self.x(v[2]), self.x(v[3]));
                self.tetprism_vol_sum += super::flip::tetprismvol(&self.pred, &a, &b, &c, &d);
            }
        }

        self.recover_delaunay_count += 1;

        // Calulate a relatively lower bound for small improvement.
        fc.bak_tetprism_vol = self.tetprism_vol_sum * self.b.epsilon * 1e-3;

        // First only use the basic Lawson's flip.
        fc.remove_ndelaunay_edge = true;
        fc.enqflag = 2;

        self.lawsonflip3d(&mut fc)?;

        if self.later_unflip_queue.is_empty() {
            return Ok(());
        }

        fc.unflip = false;
        fc.collectnewtets = true; // new tets are returned in 'cavetetlist'.
        fc.enqflag = 0;

        let bak_autofliplinklevel = self.autofliplinklevel;
        let bak_fliplinklevel = self.b.fliplinklevel;
        self.autofliplinklevel = 1; // Init level.
        self.b.fliplinklevel = -1; // No fixed level.

        self.reach[54] += 1;
        while !self.later_unflip_queue.is_empty() && self.autofliplinklevel < 4 {
            let bfarray = core::mem::take(&mut self.later_unflip_queue);
            for bf in bfarray.iter() {
                // (upstream works on 'bfarray[i]' in place: 'getedge' and
                //   'removeedgebyflips' update its 'tt', which is what is
                //   saved again.)
                let (mut tt, forg, fdest, fapex) = *bf;
                if self.getedge(forg, fdest, &mut tt)? {
                    if self.removeedgebyflips(&mut tt, &mut fc)? == 2 {
                        self.tetprism_vol_sum += fc.tetprism_vol_sum;
                    } else {
                        // This edge is not removed. Save it in later_flip_queue.
                        self.later_unflip_queue.push((tt, forg, fdest, fapex));
                    }
                    fc.tetprism_vol_sum = 0.0; // Clear it.
                    if !self.cavetetlist.is_empty() {
                        // Queue new faces for flips.
                        let list = core::mem::take(&mut self.cavetetlist);
                        for t in list.iter() {
                            // A queued new tet may be dead.
                            if !self.isdeadtet(t.tet) {
                                for ver in 0..4 {
                                    let parytet = TriFace { tet: t.tet, ver };
                                    let neightet = decode(self.tets[t.tet].nbr[ver]);
                                    if !self.facemarked(&neightet) {
                                        self.flippush(&parytet);
                                    }
                                }
                            }
                        }
                        let mut list = list;
                        list.clear();
                        self.cavetetlist = list;
                    }
                }
            }
            self.autofliplinklevel += 1;
        }

        if self.flippool_items() > 0 {
            // Flip locally non-Delaunay faces.
            fc.remove_ndelaunay_edge = true;
            fc.enqflag = 2;
            self.lawsonflip3d(&mut fc)?;
        }

        if !self.later_unflip_queue.is_empty() {
            self.later_unflip_queue.clear();
        }

        self.autofliplinklevel = bak_autofliplinklevel;
        self.b.fliplinklevel = bak_fliplinklevel;
        Ok(())
    }
}
