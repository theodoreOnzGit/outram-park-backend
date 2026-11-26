// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright (C) 2026 OUTRAM PARK contributors
//
// Port of TetGen 1.6.0 (`tetgen.cxx`, section `reconstruct_cxx`:
// `carveholes`; section `steiner_cxx`: `suppressbdrysteinerpoint`,
// `suppresssteinerpoints`; and the general form of `getvertexstar`):
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

//! Exterior removal and Steiner-point suppression (stage 5, gh:#1794).
//!
//! After boundary recovery the mesh still fills the convex hull.
//! [`TetgenMesh::carveholes`] removes the tetrahedra outside the PLC and
//! inside its volume holes, builds the new hull, spreads the region
//! attributes (`-A`), and peels hull slivers with `lawsonflip3d`.
//! [`TetgenMesh::suppresssteinerpoints`] then removes (or moves into the
//! volume) the Steiner points boundary recovery left.
//!
//! **Branches not ported** (each needs a switch the `-p` route here does
//! not take): `-c` (`b->convex`: exterior tetrahedra kept and marked `-1`),
//! the hole mesh (`-H`, `hole_mesh_filename`), volume bounds of regions
//! (`-a`, `setvolumebound`), the `-m` size interpolation of
//! `suppressbdrysteinerpoint` (`ivf.assignmeshsize`).

#[cfg(not(any(feature = "std", test)))]
#[allow(unused_imports)]
use crate::nostd_prelude::*;

use super::delaunay::{LocateResult, TetgenError};
use super::flip::{senext, senext2, sesym, FlipConstraints};
use super::mesh::{
    decode, encode, encode2, enextself, eprevself, esym, esymself, sdecode, Face, Point,
    PointRecord, ShPool, TetgenMesh, TriFace, VertType, DUMMYPOINT, NULL_HANDLE, NULL_POINT,
    NULL_TET,
};
use super::steiner::{InsertPointFlags, OptParameters};
use super::surface::facenormal;
use alloc::vec::Vec;

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
/// `tetgenmesh::orient3dfast`.
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

/// Which of the mesh's work lists a general `getvertexstar` call fills
/// (upstream passes the `arraypool` itself).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StarList {
    /// `cavetetlist`.
    CaveTet,
    /// `caveoldtetlist`.
    CaveOldTet,
}

impl TetgenMesh {
    #[inline]
    fn xp(&self, p: Point) -> [f64; 3] {
        self.points[p].xyz
    }

    /// `enqueuetetrahedron(chktet)`: queue a tetrahedron once
    /// (`marktest2`) in `badtetrahedrons` (mesh improvement).
    pub fn enqueuetetrahedron(&mut self, chktet: &TriFace) {
        if !self.marktest2ed(chktet.tet) {
            self.marktest2(chktet.tet); // Only queue it once.
            if let Some(q) = self.badtetrahedrons.as_mut() {
                q.push(*chktet);
            }
        }
    }

    /// `randomsample` + `locate` of a coordinate that is not a mesh point
    /// (a hole or region point of `carveholes`). Upstream passes the
    /// coordinates as a `point`; here a temporary record past the end of
    /// the point table stands for it (never in the pool, removed before
    /// return), so no pool slot or point mark moves.
    pub fn locate_coordinates(
        &mut self,
        xyz: &[f64; 3],
    ) -> Result<(LocateResult, TriFace), TetgenError> {
        let tmp = self.points.len();
        let mut r = PointRecord::BLANK;
        r.xyz = *xyz;
        self.points.push(r);
        let mut t = TriFace::default();
        self.randomsample(tmp, &mut t);
        let loc = self.locate(tmp, &mut t, false);
        self.points.pop();
        Ok((loc?, t))
    }

    /// `getvertexstar(fullstar, searchpt, tetlist, vertlist, shlist)` with
    /// the lists chosen: `tetlist` is `list`, `vertlist` is
    /// `cavetetvertlist` when `vertlist`, `shlist` is `caveshlist` when
    /// `shlist`. Returns the size of `tetlist` (upstream walks the whole
    /// list, including entries it held before the call).
    pub fn getvertexstar_with(
        &mut self,
        fullstar: bool,
        searchpt: Point,
        list: StarList,
        vertlist: bool,
        shlist: bool,
    ) -> usize {
        let mut tetlist = match list {
            StarList::CaveTet => core::mem::take(&mut self.cavetetlist),
            StarList::CaveOldTet => core::mem::take(&mut self.caveoldtetlist),
        };
        let mut searchtet = self.point2tetorg(searchpt);
        // Go to the opposite face (the link face) of the vertex.
        super::mesh::enextesymself(&mut searchtet);
        self.infect(searchtet.tet); // Collect this tet (link face).
        tetlist.push(searchtet);
        if vertlist {
            // Collect three (link) vertices.
            let j = searchtet.ver & 3;
            for i in 1..4 {
                let pt = self.tets[searchtet.tet].v[(j + i) % 4];
                self.pinfect(pt);
                self.cavetetvertlist.push(pt);
            }
        }

        let mut collectflag = true;
        let mut neightet = esym(&searchtet);
        if self.issubface(&neightet) {
            if shlist {
                let checksh = self.tspivot(&neightet);
                if !self.sinfected(&checksh) {
                    // Collect this subface (link edge).
                    self.sinfect(&checksh);
                    self.caveshlist.push(checksh);
                }
            }
            if !fullstar {
                collectflag = false;
            }
        }
        if collectflag {
            self.fsymself(&mut neightet); // Goto the adj tet of this face.
            esymself(&mut neightet); // Goto the oppo face of this vertex.
            self.infect(neightet.tet); // Collect this tet (link face).
            tetlist.push(neightet);
            if vertlist {
                // Collect its apex.
                let pt = self.apex(&neightet);
                self.pinfect(pt);
                self.cavetetvertlist.push(pt);
            }
        }

        // Continue to collect all tets in the star.
        let mut i = 0;
        while i < tetlist.len() {
            let mut searchtet = tetlist[i];
            for _ in 0..2 {
                let mut collectflag = true;
                enextself(&mut searchtet);
                let mut neightet = esym(&searchtet);
                if self.issubface(&neightet) {
                    if shlist {
                        let checksh = self.tspivot(&neightet);
                        if !self.sinfected(&checksh) {
                            self.sinfect(&checksh);
                            self.caveshlist.push(checksh);
                        }
                    }
                    if !fullstar {
                        collectflag = false;
                    }
                }
                if collectflag {
                    self.fsymself(&mut neightet);
                    if !self.infected(neightet.tet) {
                        esymself(&mut neightet); // Go to the face opposite to 'searchpt'.
                        self.infect(neightet.tet);
                        tetlist.push(neightet);
                        if vertlist {
                            // Check if a vertex is collected.
                            let pt = self.apex(&neightet);
                            if !self.pinfected(pt) {
                                self.pinfect(pt);
                                self.cavetetvertlist.push(pt);
                            }
                        }
                    }
                }
            }
            i += 1;
        }

        // Uninfect the list of tets and vertices.
        for t in tetlist.iter() {
            self.uninfect(t.tet);
        }
        if vertlist {
            for i in 0..self.cavetetvertlist.len() {
                let p = self.cavetetvertlist[i];
                self.puninfect(p);
            }
        }
        if shlist {
            for i in 0..self.caveshlist.len() {
                let s = self.caveshlist[i];
                self.suninfect(&s);
            }
        }
        let n = tetlist.len();
        match list {
            StarList::CaveTet => self.cavetetlist = tetlist,
            StarList::CaveOldTet => self.caveoldtetlist = tetlist,
        }
        n
    }

    /// `carveholes()` (without `-c` and `-H`): remove the tetrahedra
    /// outside the domain and in its holes, make the new hull, spread the
    /// region attributes (`-A`, from `regions`), and peel hull slivers.
    /// `holes` and `regions` are `in->holelist` and `in->regionlist`.
    pub fn carveholes(
        &mut self,
        holes: &[[f64; 3]],
        regions: &[[f64; 5]],
    ) -> Result<(), TetgenError> {
        self.reach5[0] += 1;
        let mut tetarray: Vec<TriFace> = Vec::new();
        let mut hullarray: Vec<TriFace> = Vec::new();

        // Collect unprotected tets and hull tets.
        self.tetpool.traversalinit();
        while let Some(t) = self.alltetrahedrontraverse() {
            let tetloop = TriFace { tet: t, ver: 11 }; // The face opposite to dummypoint.
            if self.ishulltet(t) && !self.issubface(&tetloop) {
                // Collect an unprotected hull tet and tet.
                self.infect(t);
                hullarray.push(tetloop);
                // tetloop's face number is 11 & 3 = 3.
                let neightet = decode(self.tets[t].nbr[3]);
                if !self.infected(neightet.tet) {
                    self.infect(neightet.tet);
                    tetarray.push(neightet);
                }
            }
        }

        if !holes.is_empty() {
            // Mark as infected any tets inside volume holes.
            for (i, h) in holes.iter().enumerate() {
                let (loc, neightet) = self.locate_coordinates(h)?;
                if loc != LocateResult::Outside {
                    self.reach5[1] += 1;
                    // The tet 'neightet' contain this point.
                    if !self.infected(neightet.tet) {
                        self.infect(neightet.tet);
                        tetarray.push(neightet);
                        let tetloop = decode(self.tets[neightet.tet].nbr[neightet.ver & 3]);
                        if !self.issubface(&neightet) {
                            // Add its adjacent tet if it is not protected.
                            if !self.infected(tetloop.tet) {
                                self.infect(tetloop.tet);
                                if self.ishulltet(tetloop.tet) {
                                    hullarray.push(tetloop);
                                } else {
                                    tetarray.push(tetloop);
                                }
                            }
                        } else {
                            // It is protected. Check if its adjacent tet is a hull tet.
                            if self.ishulltet(tetloop.tet) && !self.infected(tetloop.tet) {
                                // Both sides of the subface are in exterior.
                                self.infect(tetloop.tet);
                                hullarray.push(tetloop);
                            }
                            if self.infected(tetloop.tet) {
                                // Both sides of this subface are in exterior.
                                let checksh = self.tspivot(&neightet);
                                self.sinfect(&checksh); // Only queue it once.
                                self.subfacstack.push(checksh);
                            }
                        }
                    }
                } else {
                    // A hole point locates outside of the convex hull.
                    self.warnings.push(alloc::format!(
                        "Warning:  The {}-th hole point lies outside the convex hull.",
                        i + 1
                    ));
                }
            }
        }

        // (`-H`, a hole mesh, is not ported.)

        let mut regiontets: Vec<TriFace> = Vec::new();
        if self.b.regionattrib && !regions.is_empty() {
            // Record the tetrahedra that contains the region points.
            for (i, r) in regions.iter().enumerate() {
                let (loc, neightet) = self.locate_coordinates(&[r[0], r[1], r[2]])?;
                if loc != LocateResult::Outside {
                    self.reach5[2] += 1;
                    regiontets.push(neightet);
                } else {
                    self.warnings.push(alloc::format!(
                        "Warning:  The {}-th region point lies outside the convex hull.",
                        i + 1
                    ));
                    regiontets.push(TriFace::default());
                }
            }
        }

        // Collect all exterior tets (in concave place and in holes).
        let mut i = 0;
        while i < tetarray.len() {
            let parytet = tetarray[i];
            let j = parytet.ver & 3; // j is the current face number.
                                     // Check the other three adjacent tets.
            for k in 1..4 {
                let neightet = decode(self.tets[parytet.tet].nbr[(j + k) % 4]);
                // neightet may be a hull tet.
                if !self.infected(neightet.tet) {
                    if !self.issubface(&neightet) {
                        // Not protected. Collect it. (It must not be a hull tet).
                        self.infect(neightet.tet);
                        tetarray.push(neightet);
                    } else if self.ishulltet(neightet.tet) {
                        // Protected, a hull tet. Collect it.
                        self.infect(neightet.tet);
                        hullarray.push(neightet);
                        // Both sides of this subface are exterior.
                        let checksh = self.tspivot(&neightet);
                        self.sinfect(&checksh); // Only queue it once.
                        self.subfacstack.push(checksh);
                    }
                } else if self.issubface(&neightet) {
                    // Both sides of this face are in exterior.
                    let checksh = self.tspivot(&neightet);
                    if !self.sinfected(&checksh) {
                        self.sinfect(&checksh);
                        self.subfacstack.push(checksh);
                    }
                }
            }
            i += 1;
        }

        if self.b.regionattrib && !regions.is_empty() {
            // Re-check saved region tets to see if they lie outside.
            for (i, rt) in regiontets.iter_mut().enumerate() {
                if rt.tet != NULL_TET && self.infected(rt.tet) {
                    self.warnings.push(alloc::format!(
                        "Warning:  The {}-th region point lies in the exterior of the domain.",
                        i + 1
                    ));
                    rt.tet = NULL_TET;
                }
            }
        }

        // Collect vertices which point to infected tets. If -Y/1 is used,
        //   collect all Steiner points for removal.
        let cdt = false; // -D is not ported.
        let steinermark = self.numberofpoints as i32 - if self.firstnumber != 0 { 0 } else { 1 };
        self.pointpool.traversalinit();
        while let Some(p) = self.pointtraverse() {
            let vt = self.pointtype(p);
            if vt != VertType::Unused && vt != VertType::Duplicated {
                let ptr = self.point2tet(p);
                // (Upstream decodes the handle unconditionally; every point
                //   reached here carries one.)
                if ptr != NULL_HANDLE && self.infected(decode(ptr).tet) {
                    self.cavetetvertlist.push(p);
                }
                if (!cdt || self.b.nobisect != 0)
                    && self.b.supsteiner_level > 0
                    && self.pointmark(p) > steinermark
                {
                    // Queue it if it is a Steiner point.
                    self.subvertstack.push(p);
                }
            }
        }

        if !tetarray.is_empty() {
            // Remove exterior tets (no -c). Hull tets are updated.
            self.reach5[3] += 1;
            let mut delsegcount = 0i64;

            // Collect segments which point to infected tets.
            self.subsegpool.traversalinit();
            while let Some(s) = self.shellfacetraverse(ShPool::Seg) {
                let segloop = Face {
                    pool: ShPool::Seg,
                    sh: s,
                    shver: 0,
                };
                let neightet = self.sstpivot1(&segloop);
                if neightet.tet != NULL_TET && self.infected(neightet.tet) {
                    self.subsegstack.push(segloop);
                }
            }

            let mut newhullfacearray: Vec<TriFace> = Vec::new();

            // Create and save new hull tets.
            for parytet in tetarray.iter() {
                for j in 0..4 {
                    let mut tetloop = decode(self.tets[parytet.tet].nbr[j]);
                    if !self.infected(tetloop.tet) {
                        // Found a new hull face (must be a subface).
                        let mut checksh = self.tspivot(&tetloop);
                        let mut hulltet = self.maketetrahedron();
                        let pa = self.org(&tetloop);
                        let pb = self.dest(&tetloop);
                        let pc = self.apex(&tetloop);
                        self.setvertices(&hulltet, pb, pa, pc, DUMMYPOINT);
                        self.bond(&tetloop, &hulltet);
                        // Update the subface-to-tet map.
                        checksh = sesym(&checksh);
                        self.tsbond(&hulltet, &checksh);
                        // Update the segment-to-tet map.
                        for _ in 0..3 {
                            if self.issubseg(&tetloop) {
                                let checkseg = self.tsspivot1(&tetloop);
                                self.tssbond1(&hulltet, &checkseg);
                                self.sstbond1(&checkseg, &hulltet);
                            }
                            enextself(&mut tetloop);
                            eprevself(&mut hulltet);
                        }
                        // Update the point-to-tet map.
                        let h = encode2(tetloop.tet, 0);
                        self.setpoint2tet(pa, h);
                        self.setpoint2tet(pb, h);
                        self.setpoint2tet(pc, h);
                        // Save the exterior tet at this hull face.
                        newhullfacearray.push(TriFace {
                            tet: parytet.tet,
                            ver: j,
                        });
                    }
                }
            }

            // Connect new hull tets.
            for i in 0..newhullfacearray.len() {
                let mut parytet = newhullfacearray[i];
                let neightet = self.fsym(&parytet);
                // Get the new hull tet.
                let mut hulltet = self.fsym(&neightet);
                for _ in 0..3 {
                    let casface = esym(&hulltet);
                    if self.tets[casface.tet].nbr[casface.ver & 3] == NULL_HANDLE {
                        // Find the adjacent hull face by traversing the
                        //   tets in the exterior (all infected).
                        let mut neightet = parytet;
                        loop {
                            self.fnextself(&mut neightet);
                            if !self.infected(neightet.tet) {
                                break;
                            }
                        }
                        if !self.ishulltet(neightet.tet) {
                            // An interior tet. Get the new hull tet.
                            self.fsymself(&mut neightet);
                            esymself(&mut neightet);
                        }
                        // Bond them together.
                        self.bond(&casface, &neightet);
                    }
                    enextself(&mut hulltet);
                    enextself(&mut parytet);
                }
                newhullfacearray[i] = parytet;
            }

            if !self.subfacstack.is_empty() {
                // Remove all subfaces which do not attach to any tetrahedron.
                //   Segments which are not attached to any subfaces and tets
                //   are deleted too.
                self.reach5[4] += 1;
                let list = core::mem::take(&mut self.subfacstack);
                for mut parysh in list.iter().copied() {
                    // Dissolve this subface from face links.
                    for _ in 0..3 {
                        let casingout = self.spivot(&parysh);
                        let checkseg = self.sspivot(&parysh);
                        if !casingout.is_null() {
                            let mut casingin = casingout;
                            loop {
                                let checksh = self.spivot(&casingin);
                                if checksh.sh == parysh.sh {
                                    break;
                                }
                                casingin = checksh;
                            }
                            if casingin.sh != casingout.sh {
                                // Update the link: ... -> casingin -> casingout ->...
                                self.sbond1(&casingin, &casingout);
                            } else {
                                // Only one subface at this edge is left.
                                self.sdissolve(&casingout);
                            }
                            if !checkseg.is_null() {
                                // Make sure the segment does not connect to a dead one.
                                self.ssbond(&casingout, &checkseg);
                            }
                        } else if !checkseg.is_null() {
                            self.reach5[5] += 1;
                            self.shellfacedealloc(ShPool::Seg, checkseg.sh);
                            delsegcount += 1;
                        }
                        parysh = senext(&parysh);
                    }
                    // Delete this subface.
                    self.shellfacedealloc(ShPool::Sub, parysh.sh);
                }
            }

            if !self.subsegstack.is_empty() {
                let list = core::mem::take(&mut self.subsegstack);
                for paryseg in list.iter() {
                    if !paryseg.is_null() && self.subsegs[paryseg.sh].v[0] != NULL_POINT {
                        let neightet = self.sstpivot1(paryseg);
                        if self.infected(neightet.tet) {
                            self.reach5[5] += 1;
                            self.shellfacedealloc(ShPool::Seg, paryseg.sh);
                            delsegcount += 1;
                        }
                    }
                }
            }
            let _ = delsegcount;

            if !self.cavetetvertlist.is_empty() {
                // Some vertices may lie in exterior. Mark them as UNUSEDVERTEX.
                let list = core::mem::take(&mut self.cavetetvertlist);
                for &p in list.iter() {
                    let neightet = decode(self.point2tet(p));
                    if self.infected(neightet.tet) {
                        // Found an exterior vertex.
                        if self.pointmark(p) > steinermark {
                            // A Steiner point.
                            match self.pointtype(p) {
                                VertType::FreeSeg => self.st_segref_count -= 1,
                                VertType::FreeFacet => self.st_facref_count -= 1,
                                _ => self.st_volref_count -= 1,
                            }
                            if self.steinerleft > 0 {
                                self.steinerleft += 1;
                            }
                        }
                        self.setpointtype(p, VertType::Unused);
                        self.unuverts += 1;
                        self.reach5[6] += 1;
                    }
                }
                // 'subvertstack' is cleaned in suppresssteinerpoints().
            }

            // Update the hull size.
            self.hullsize += newhullfacearray.len() as i64 - hullarray.len() as i64;

            // Delete all exterior tets and old hull tets.
            for t in tetarray.iter() {
                self.tetrahedrondealloc(t.tet);
            }
            tetarray.clear();
            for t in hullarray.iter() {
                self.tetrahedrondealloc(t.tet);
            }
            hullarray.clear();
        }

        if self.b.regionattrib {
            // Spread region attributes (-A).
            self.reach5[7] += 1;
            let mut maxattr: i32 = 0; // Choose a small number here.
            let mut regioncount = 0;
            let mut tmpary: Vec<i32> = Vec::new();

            for (i, r) in regions.iter().enumerate() {
                if regiontets[i].tet == NULL_TET {
                    continue;
                }
                let attr = r[3] as i32;
                if attr > maxattr {
                    maxattr = attr;
                }
                tetarray.clear(); // Re-use this array.
                self.infect(regiontets[i].tet);
                tetarray.push(regiontets[i]);
                self.spread_attribute(&mut tetarray, attr);
                tmpary.push(attr);
                regioncount += 1;
            }

            // Set attributes for all tetrahedra.
            let mut attr = maxattr + 1;
            self.tetpool.traversalinit();
            while let Some(t) = self.tetrahedrontraverse() {
                if !self.infected(t) {
                    // An unmarked region.
                    tetarray.clear();
                    self.infect(t);
                    tetarray.push(TriFace { tet: t, ver: 11 });
                    let save = self.tetpool.pathitem;
                    self.spread_attribute(&mut tetarray, attr);
                    self.tetpool.pathitem = save;
                    tmpary.push(attr);
                    attr += 1;
                    regioncount += 1;
                }
            }

            // Uninfect processed tets.
            self.tetpool.traversalinit();
            while let Some(t) = self.tetrahedrontraverse() {
                self.uninfect(t);
            }

            self.subdomains = regioncount;
            self.subdomain_markers = tmpary;
        }

        // The mesh is non-convex now (no -c).
        self.nonconvex = true;

        // Push all hull tets into 'flipstack'.
        self.tetpool.traversalinit();
        while let Some(t) = self.alltetrahedrontraverse() {
            if self.tets[t].v[3] == DUMMYPOINT {
                let neightet = self.fsym(&TriFace { tet: t, ver: 11 });
                self.flippush(&neightet);
            }
        }

        let mut fc = FlipConstraints {
            enqflag: 2,
            ..Default::default()
        };
        self.lawsonflip3d(&mut fc)?;
        // (`unflipqueue->restart()`: that queue is not used on this path.)
        Ok(())
    }

    /// The flood fill of `carveholes`' `-A` loops: give every tet reached
    /// from `tetarray` without crossing a subface the attribute `attr`.
    fn spread_attribute(&mut self, tetarray: &mut Vec<TriFace>, attr: i32) {
        let mut j = 0;
        while j < tetarray.len() {
            let tetloop = tetarray[j];
            if self.numelemattrib > 0 {
                self.elemattrib[tetloop.tet] = attr as f64;
            }
            for k in 0..4 {
                let neightet = decode(self.tets[tetloop.tet].nbr[k]);
                // Is the adjacent already checked?
                if !self.infected(neightet.tet) && !self.issubface(&neightet) {
                    self.infect(neightet.tet);
                    tetarray.push(neightet);
                }
            }
            j += 1;
        }
    }

    /// `suppressbdrysteinerpoint(steinerpt)`: move a segment or facet
    /// Steiner point off the boundary by inserting a Steiner point inside
    /// each sector around it and removing it by flips. True if removed.
    pub fn suppressbdrysteinerpoint(&mut self, steinerpt: Point) -> Result<bool, TetgenError> {
        self.reach5[11] += 1;
        let vt = self.pointtype(steinerpt);

        if vt == VertType::FreeSeg {
            let mut leftseg = sdecode(self.point2sh(steinerpt));
            leftseg.shver = 0;
            if self.sdest(&leftseg) == steinerpt {
                // (rightseg is not needed below.)
            } else {
                let rightseg = leftseg;
                leftseg = senext2(&rightseg);
                leftseg = self.spivot(&leftseg);
                leftseg.shver = 0;
            }
            // Get all subfaces at the left segment [lpt, steinerpt].
            let parentsh = self.spivot(&leftseg);
            if !parentsh.is_null() {
                // It is not a dangling segment.
                let mut spinsh = parentsh;
                loop {
                    let mut s = spinsh;
                    // Orient the face consistently.
                    if self.sorg(&s) != self.sorg(&parentsh) {
                        s = sesym(&s);
                    }
                    self.cavesegshlist.push(s);
                    spinsh = self.spivot(&spinsh);
                    if spinsh.is_null() || spinsh.sh == parentsh.sh {
                        break;
                    }
                }
            }
            if self.cavesegshlist.len() < 2 {
                // It is a single segment. Not handle it yet.
                self.cavesegshlist.clear();
                return Ok(false);
            }
        } else if vt == VertType::FreeFacet {
            let mut parentsh = sdecode(self.point2sh(steinerpt));
            // A facet Steiner point. There are exactly two sectors.
            for _ in 0..2 {
                self.cavesegshlist.push(parentsh);
                parentsh = sesym(&parentsh);
            }
        } else {
            return Ok(false); // no need to suppress it.
        }

        let n = self.cavesegshlist.len();
        let mut newsteiners: Vec<Point> = alloc::vec![NULL_POINT; n];
        let sp = self.xp(steinerpt);

        // Search for each sector an interior vertex.
        let mut i = 0;
        while i < self.cavesegshlist.len() {
            let parysh = self.cavesegshlist[i];
            let searchtet = self.stpivot(&parysh);
            // Skip it if it is outside.
            if self.ishulltet(searchtet.tet) {
                i += 1;
                continue;
            }
            // Get the "half-ball".
            self.setpoint2tet(steinerpt, encode(&searchtet));
            self.getvertexstar_with(false, steinerpt, StarList::CaveTet, false, true);
            // Calculate the searching vector.
            let pa = self.xp(self.sorg(&parysh));
            let pb = self.xp(self.sdest(&parysh));
            let pc = self.xp(self.sapex(&parysh));
            let mut v1 = facenormal(&pa, &pb, &pc, 1);
            let len = dot(&v1, &v1).sqrt();
            v1 = [v1[0] / len, v1[1] / len, v1[2] / len];
            if vt == VertType::FreeSeg {
                let nxt = self.cavesegshlist[(i + 1) % n];
                let pd = self.xp(self.sapex(&nxt));
                let mut v2 = facenormal(&pb, &pa, &pd, 1);
                let len = dot(&v2, &v2).sqrt();
                v2 = [v2[0] / len, v2[1] / len, v2[2] / len];
                // Average the two vectors.
                v1 = [
                    0.5 * (v1[0] + v2[0]),
                    0.5 * (v1[1] + v2[1]),
                    0.5 * (v1[2] + v2[2]),
                ];
            }
            // Search the intersection of the ray with the half-ball's shell.
            let len = distance(&pa, &pb);
            let v2 = [
                sp[0] + len * v1[0],
                sp[1] + len * v1[1],
                sp[2] + len * v1[2],
            ];
            let mut startpt = [0.0; 3];
            let mut found = false;
            for j in 0..self.cavetetlist.len() {
                let t = self.cavetetlist[j];
                let qa = self.xp(self.org(&t));
                let qb = self.xp(self.dest(&t));
                let qc = self.xp(self.apex(&t));
                if self.pred.orient3d(&sp, &qa, &qb, &v2) >= 0.0
                    && self.pred.orient3d(&sp, &qb, &qc, &v2) >= 0.0
                    && self.pred.orient3d(&sp, &qc, &qa, &v2) >= 0.0
                {
                    // Found! Calculate the intersection.
                    startpt = self.planelineint(&qa, &qb, &qc, &sp, &v2).0;
                    found = true;
                    break;
                }
            }
            if !found {
                break; // There is no intersection!! Debug is needed.
            }
            // Close the ball by adding the subfaces.
            for j in 0..self.caveshlist.len() {
                let s = self.caveshlist[j];
                let neightet = self.stpivot(&s);
                self.cavetetlist.push(neightet);
            }
            // Search a best point inside the segment [startpt, steinerpt].
            let mut it = 0;
            let mut samplesize: i32 = 100;
            let v1 = [sp[0] - startpt[0], sp[1] - startpt[1], sp[2] - startpt[2]];
            let mut minvol = -1.0;
            let mut candpt = [0.0; 3];
            while it < 3 {
                for j in 1..samplesize - 1 {
                    let f = j as f64 / samplesize as f64;
                    let samplept = [
                        startpt[0] + f * v1[0],
                        startpt[1] + f * v1[1],
                        startpt[2] + f * v1[2],
                    ];
                    // Find the minimum volume for 'samplept'.
                    let mut smallvol = -1.0;
                    let mut k = 0;
                    while k < self.cavetetlist.len() {
                        let t = self.cavetetlist[k];
                        let qa = self.xp(self.org(&t));
                        let qb = self.xp(self.dest(&t));
                        let qc = self.xp(self.apex(&t));
                        let mut ori = self.pred.orient3d(&qb, &qa, &qc, &samplept);
                        {
                            // [2017-10-15] Rounding
                            let lab = distance(&qa, &qb);
                            let lbc = distance(&qb, &qc);
                            let lca = distance(&qc, &qa);
                            let lv = (lab + lbc + lca) / 3.0;
                            let l3 = lv * lv * lv;
                            if ori.abs() / l3 < 1e-8 {
                                ori = 0.0;
                            }
                        }
                        if ori <= 0.0 {
                            break; // An invalid tet.
                        }
                        if smallvol == -1.0 {
                            smallvol = ori;
                        } else if ori < smallvol {
                            smallvol = ori;
                        }
                        k += 1;
                    }
                    if k == self.cavetetlist.len() {
                        // Found a valid point. Remember it.
                        if minvol == -1.0 {
                            candpt = samplept;
                            minvol = smallvol;
                        } else if minvol < smallvol {
                            // It is a better location. Remember it.
                            candpt = samplept;
                            minvol = smallvol;
                        } else {
                            // No improvement of smallest volume.
                            break;
                        }
                    }
                }
                if minvol > 0.0 {
                    break;
                }
                samplesize *= 10;
                it += 1;
            }
            if minvol == -1.0 {
                // Failed to find a valid point.
                self.cavetetlist.clear();
                self.caveshlist.clear();
                break;
            }
            // Create a new Steiner point inside this section.
            let np = self.makepoint(VertType::FreeVol);
            self.points[np].xyz = candpt;
            newsteiners[i] = np;
            self.cavetetlist.clear();
            self.caveshlist.clear();
            i += 1;
        }

        if i < self.cavesegshlist.len() {
            // Failed to suppress the vertex.
            while i > 0 {
                if newsteiners[i - 1] != NULL_POINT {
                    self.pointdealloc(newsteiners[i - 1]);
                }
                i -= 1;
            }
            self.cavesegshlist.clear();
            return Ok(false);
        }

        // First insert Steiner points into the mesh ('cavesegshlist' is
        //   used by insertpoint()).
        let segshlist = core::mem::take(&mut self.cavesegshlist);

        let mut i = 0;
        while i < n {
            let parysh = segshlist[i];
            let mut searchtet = self.stpivot(&parysh);
            // Skip it if it is outside.
            if self.ishulltet(searchtet.tet) {
                i += 1;
                continue;
            }
            // Get the "half-ball".
            self.setpoint2tet(steinerpt, encode(&searchtet));
            self.getvertexstar_with(false, steinerpt, StarList::CaveTet, false, true);
            // Get all tets in this sector.
            let mut neightet = TriFace::default();
            for j in 0..self.cavetetlist.len() {
                neightet = self.cavetetlist[j];
                self.infect(neightet.tet);
                self.caveoldtetlist.push(neightet);
            }
            self.cavetetlist.clear();
            self.caveshlist.clear();

            let mut ivf = InsertPointFlags {
                iloc: LocateResult::InStar, // No need point location.
                ..Default::default()
            };
            searchtet = neightet;
            // (ivf.assignmeshsize = b->metric: -m is not ported.)
            if self.insertpoint(newsteiners[i], &mut searchtet, None, None, &mut ivf)? {
                // The vertex has been inserted.
                self.st_volref_count += 1;
                if self.steinerleft > 0 {
                    self.steinerleft -= 1;
                }
            } else {
                // Not inserted.
                self.pointdealloc(newsteiners[i]);
                newsteiners[i] = NULL_POINT;
                break;
            }
            i += 1;
        }

        if i < n {
            return Ok(false);
        }

        // Now remove the Steiner point from the segment.
        if !self.removevertexbyflips(steinerpt)? {
            return Ok(false);
        }
        self.reach5[12] += 1;

        // We've removed a Steiner points.
        self.setpointtype(steinerpt, VertType::Unused);
        self.unuverts += 1;

        let bak_fliplinklevel = self.b.fliplinklevel;
        self.b.fliplinklevel = 100000; // Unlimited flip level.

        // Try to remove newly added Steiner points.
        for &np in newsteiners.iter() {
            if np != NULL_POINT && !self.removevertexbyflips(np)? && self.b.supsteiner_level > 0 {
                // Save it in subvertstack for removal.
                self.subvertstack.push(np);
            }
        }

        self.b.fliplinklevel = bak_fliplinklevel;
        Ok(true)
    }

    /// `suppresssteinerpoints()`: suppress the boundary Steiner points of
    /// `subvertstack`, remove its volume ones by flips (`-Y/1`), and smooth
    /// the rest (`-Y/2`, the default).
    pub fn suppresssteinerpoints(&mut self) -> Result<(), TetgenError> {
        self.reach5[10] += 1;
        let bak_fliplinklevel = self.b.fliplinklevel;
        self.b.fliplinklevel = 100000; // Unlimited flip level.

        // Try to suppress boundary Steiner points.
        let mut i = 0;
        while i < self.subvertstack.len() {
            let rempt = self.subvertstack[i];
            let vt = self.pointtype(rempt);
            if vt != VertType::Unused && (vt == VertType::FreeSeg || vt == VertType::FreeFacet) {
                self.suppressbdrysteinerpoint(rempt)?;
            }
            i += 1;
        }

        if self.b.supsteiner_level > 0 {
            // -Y/1
            let mut i = 0;
            while i < self.subvertstack.len() {
                let rempt = self.subvertstack[i];
                if self.pointtype(rempt) == VertType::FreeVol && self.removevertexbyflips(rempt)? {
                    self.reach5[13] += 1;
                }
                i += 1;
            }
        }

        self.b.fliplinklevel = bak_fliplinklevel;

        if self.b.supsteiner_level > 1 {
            // -Y/2: smooth interior Steiner points.
            let mut opm = OptParameters {
                max_min_volume: true,
                numofsearchdirs: 20,
                searchstep: 0.001,
                maxiter: 30, // Limit the maximum iterations.
                ..Default::default()
            };
            let mut ivcount;
            loop {
                let mut nt = 0;
                loop {
                    let mut count = 0;
                    ivcount = 0; // Clear the inverted count.
                    for i in 0..self.subvertstack.len() {
                        let rempt = self.subvertstack[i];
                        if self.pointtype(rempt) != VertType::FreeVol {
                            continue;
                        }
                        self.getvertexstar_with(true, rempt, StarList::CaveTet, false, false);
                        // The initial smallest volume (maybe zero or negative).
                        for j in 0..self.cavetetlist.len() {
                            let v = self.tets[self.cavetetlist[j].tet].v;
                            let ori = orient3dfast(
                                &self.xp(v[1]),
                                &self.xp(v[0]),
                                &self.xp(v[2]),
                                &self.xp(v[3]),
                            );
                            if j == 0 || opm.initval > ori {
                                opm.initval = ori;
                            }
                        }
                        let mut smtpt = self.xp(rempt);
                        let mut list = core::mem::take(&mut self.cavetetlist);
                        let it = self.smoothpoint(&mut smtpt, &mut list, true, &mut opm)?;
                        self.cavetetlist = list;
                        if it > 0 {
                            // smoothpoint moves the point itself upstream.
                            self.points[rempt].xyz = smtpt;
                            self.reach5[14] += 1;
                            count += 1;
                        }
                        if opm.imprval <= 0.0 {
                            ivcount += 1; // The mesh contains inverted elements.
                        }
                        self.cavetetlist.clear();
                    }
                    if count == 0 {
                        break; // No point has been smoothed.
                    }
                    nt += 1;
                    if nt > 2 {
                        break; // Already three iterations.
                    }
                }
                if ivcount > 0 && opm.maxiter > 0 {
                    // Set unlimited smoothing steps. Try again.
                    opm.numofsearchdirs = 30;
                    opm.searchstep = 0.0001;
                    opm.maxiter = -1;
                    continue;
                }
                break;
            }
            if ivcount > 0 {
                self.warnings
                    .push("BUG Report!  The mesh contain inverted elements.".into());
            }
        }

        self.subvertstack.clear();
        Ok(())
    }
}
