// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright (C) 2026 OUTRAM PARK contributors
//
// Port of TetGen 1.6.0 (`tetgen.cxx`: the `-m` mesh sizes read by
// `transfernodes`, `getpointmeshsize`, `collectremovepoints`,
// `meshcoarsening`; the size branches of `insertpoint`, `checktet4split`,
// `split_segment`, `split_subface`, `split_tetrahedron` and the Steiner
// point routines live beside the code they extend):
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

//! `-m`: mesh sizes on the points (gh:#1825, epic gh:#1789).
//!
//! **What upstream does.** A `.mtr` file gives every input point a size
//! (`tetgenio::load_mtr`, [`super::PlcInput::read_mtr`]); `transfernodes`
//! copies it, times `-m#`, into the point (`pointmtrindex`). Under `-m`:
//!
//! - `checktet4split` splits a tetrahedron whose circumradius exceeds the
//!   size of any of its vertices that has one (`qflag = 1`, so the split
//!   does not check the insertion radius), before the radius-edge test;
//! - `insertpoint` rejects a point closer than **half** a cavity vertex's
//!   size (`rejflag & 4`, `ENCVERTEX`; set by the three refinement splits)
//!   and gives every inserted point a size (`assignmeshsize`): the P1
//!   interpolation of its tetrahedron's (face's, edge's) vertex sizes when
//!   they all have one, else 0 ([`TetgenMesh::getpointmeshsize`]);
//! - `meshcoarsening` runs after boundary recovery: an interior vertex
//!   whose shortest edge is below its size is removed by flips
//!   ([`TetgenMesh::meshcoarsening`]);
//! - the output gains a `.mtr` (every point's size) and a `.p2t` (a
//!   tetrahedron of every point) file.
//!
//! Surface triangles and segments carry no size test: the sizes reach the
//! boundary only through tetrahedron circumcentres that encroach upon it.
//! The sizes of new points come from the **current** mesh, so a size field
//! given on the boundary points alone is the same everywhere a tetrahedron
//! has only boundary vertices: grading into the volume needs sized points
//! inside it (the wall grading of `pipeline::tet_source` adds them).
//!
//! **Not ported:** the background mesh (`-m` with `.b.node` / `.b.ele`,
//! `bgm`, `interpolatemeshsize`, which needs `-r`'s `reconstructmesh`);
//! anisotropic sizes (3 or 6 `.mtr` columns, refused by `read_mtr`);
//! `-R` coarsening (`coarsen_param`); the `-D` (`delaunizesegments`,
//! `refineregion`) and `-i` (`insertconstrainedpoints`) callers.

#[cfg(not(any(feature = "std", test)))]
#[allow(unused_imports)]
use crate::nostd_prelude::*;

use super::delaunay::{LocateResult, TetgenError};
use super::mesh::{Point, TetgenMesh, TriFace, VertType};
use alloc::vec::Vec;

#[inline]
fn dot(v1: &[f64; 3], v2: &[f64; 3]) -> f64 {
    v1[0] * v2[0] + v1[1] * v2[1] + v1[2] * v2[2]
}
#[inline]
fn cross(v1: &[f64; 3], v2: &[f64; 3]) -> [f64; 3] {
    [
        v1[1] * v2[2] - v1[2] * v2[1],
        v1[2] * v2[0] - v1[0] * v2[2],
        v1[0] * v2[1] - v1[1] * v2[0],
    ]
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
/// `tetgenmesh::triarea(pa, pb, pc)`.
#[inline]
fn triarea(pa: &[f64; 3], pb: &[f64; 3], pc: &[f64; 3]) -> f64 {
    let v1 = [pb[0] - pa[0], pb[1] - pa[1], pb[2] - pa[2]];
    let v2 = [pc[0] - pa[0], pc[1] - pa[1], pc[2] - pa[2]];
    let c = cross(&v1, &v2);
    0.5 * dot(&c, &c).sqrt()
}

impl TetgenMesh {
    /// `transfernodes()`' "Read the point metric tensor" lines, after the
    /// points are made: `pointloop[pointmtrindex] = pointmtrlist[i] *
    /// metric_scale` for every input point (pool slots 1..=n in input
    /// order). Upstream copies the list **whether or not `-m` is given**;
    /// without `-m` and without `-q` the size word is the insertion-radius
    /// word (`sizeoftensor` is then 1, `pointinsradiusindex ==
    /// pointmtrindex`), so the size lands in [`super::mesh::PointRecord::insradius`],
    /// and with `-q` (no `-m`) in a word nothing reads.
    pub fn transfer_point_mtrs(&mut self, mtrs: &[f64]) {
        if mtrs.is_empty() {
            return;
        }
        for (i, &m) in mtrs.iter().enumerate().take(self.numberofpoints) {
            self.reachm[18] += 1;
            let mtr = m * self.b.metric_scale;
            let p = i + 1;
            if self.b.metric {
                self.points[p].mtr = mtr;
            } else if !self.b.quality {
                self.points[p].insradius = mtr;
            }
        }
    }

    /// `getpointmeshsize(searchpt, searchtet, iloc)`: the size of a point
    /// at location `iloc` of `searchtet`, linearly interpolated from the
    /// sizes of the tetrahedron's (`INTETRAHEDRON`: its four record
    /// vertices), the face's (`ONFACE`: org, dest, apex) or the edge's
    /// (`ONEDGE`) vertices when every one has a size, the vertex's own
    /// (`ONVERTEX`); 0 otherwise (`INSTAR` among them).
    pub fn getpointmeshsize(
        &mut self,
        searchpt: Point,
        searchtet: &TriFace,
        iloc: LocateResult,
    ) -> f64 {
        self.reachm[0] += 1;
        let mut size = 0.0;
        let sp = self.points[searchpt].xyz;
        if iloc == LocateResult::InTetrahedron {
            let pts = self.tets[searchtet.tet].v;
            let m = |k: usize| self.points[pts[k]].mtr;
            // Only do interpolation if all vertices have non-zero sizes.
            if m(0) > 0.0 && m(1) > 0.0 && m(2) > 0.0 && m(3) > 0.0 {
                // P1 interpolation.
                self.reachm[1] += 1;
                let x = |k: usize| self.points[pts[k]].xyz;
                let volume = orient3dfast(&x(0), &x(1), &x(2), &x(3));
                let vol = [
                    orient3dfast(&sp, &x(1), &x(2), &x(3)),
                    orient3dfast(&x(0), &sp, &x(2), &x(3)),
                    orient3dfast(&x(0), &x(1), &sp, &x(3)),
                    orient3dfast(&x(0), &x(1), &x(2), &sp),
                ];
                for (i, v) in vol.iter().enumerate() {
                    let wei = (v / volume).abs();
                    size += wei * m(i);
                }
            }
        } else if iloc == LocateResult::OnFace {
            let (pa, pb, pc) = (
                self.org(searchtet),
                self.dest(searchtet),
                self.apex(searchtet),
            );
            let (ma, mb, mc) = (
                self.points[pa].mtr,
                self.points[pb].mtr,
                self.points[pc].mtr,
            );
            if ma > 0.0 && mb > 0.0 && mc > 0.0 {
                self.reachm[2] += 1;
                let (xa, xb, xc) = (
                    self.points[pa].xyz,
                    self.points[pb].xyz,
                    self.points[pc].xyz,
                );
                let volume = triarea(&xa, &xb, &xc);
                let vol = [
                    triarea(&sp, &xb, &xc),
                    triarea(&xa, &sp, &xc),
                    triarea(&xa, &xb, &sp),
                ];
                size = (vol[0] / volume) * ma + (vol[1] / volume) * mb + (vol[2] / volume) * mc;
            }
        } else if iloc == LocateResult::OnEdge {
            let (pa, pb) = (self.org(searchtet), self.dest(searchtet));
            let (ma, mb) = (self.points[pa].mtr, self.points[pb].mtr);
            if ma > 0.0 && mb > 0.0 {
                self.reachm[3] += 1;
                let (xa, xb) = (self.points[pa].xyz, self.points[pb].xyz);
                let volume = distance(&xa, &xb);
                let vol = [distance(&sp, &xb), distance(&xa, &sp)];
                size = (vol[0] / volume) * ma + (vol[1] / volume) * mb;
            }
        } else if iloc == LocateResult::OnVertex {
            let pa = self.org(searchtet);
            if self.points[pa].mtr > 0.0 {
                self.reachm[4] += 1;
                size = self.points[pa].mtr;
            }
        }
        if size == 0.0 {
            self.reachm[5] += 1;
        }
        size
    }

    /// `collectremovepoints(remptlist)` under `-m` (and the point-marker
    /// `-1` list, which `meshcoarsening` reads whenever it runs): every
    /// vertex not on the boundary (not a ridge, facet, free-facet,
    /// free-segment or unused vertex) that has a size larger than its
    /// shortest edge (`getvertexstar(1, ...)`), then every input point
    /// marked `-1`, each once. `-R`'s `coarsen_param` part is not ported.
    pub fn collectremovepoints(&mut self) -> Vec<Point> {
        let mut remptlist: Vec<Point> = Vec::new();
        // If a mesh sizing function is given. Collect vertices whose mesh
        //   size is greater than its smallest edge length.
        if self.b.metric {
            self.pointpool.traversalinit();
            while let Some(ptloop) = self.pointtraverse() {
                // Do not remove a boundary vertex.
                let vt = self.pointtype(ptloop);
                if matches!(
                    vt,
                    VertType::Ridge
                        | VertType::Facet
                        | VertType::FreeFacet
                        | VertType::FreeSeg
                        | VertType::Unused
                ) {
                    continue;
                }
                if self.points[ptloop].mtr > 0.0 {
                    self.reachm[8] += 1;
                    // Get the smallest edge length at this vertex.
                    self.getvertexstar(true, ptloop);
                    let xp = self.points[ptloop].xyz;
                    let mut smlen = distance(&xp, &self.points[self.cavetetvertlist[0]].xyz);
                    for i in 1..self.cavetetvertlist.len() {
                        let len = distance(&xp, &self.points[self.cavetetvertlist[i]].xyz);
                        if len < smlen {
                            smlen = len;
                        }
                    }
                    self.cavetetvertlist.clear();
                    self.cavetetlist.clear();
                    if smlen < self.points[ptloop].mtr {
                        self.reachm[9] += 1;
                        self.pinfect(ptloop);
                        remptlist.push(ptloop);
                    }
                }
            }
        }

        // If 'in->pointmarkerlist' exists, Collect vertices with markers '-1'.
        if let Some(markers) = self.pointmarkerlist.clone() {
            self.pointpool.traversalinit();
            let mut index = 0;
            while let Some(ptloop) = self.pointtraverse() {
                if index < self.numberofpoints {
                    if markers[index] == -1 {
                        self.reachm[13] += 1;
                        self.pinfect(ptloop);
                        remptlist.push(ptloop);
                    }
                } else {
                    // Remaining are not input points. Stop here.
                    break;
                }
                index += 1;
            }
        }

        // Unmark all collected vertices.
        for &p in &remptlist {
            self.puninfect(p);
        }
        remptlist
    }

    /// `meshcoarsening()` (`-m`; `-R` is not ported): remove the vertices
    /// [`TetgenMesh::collectremovepoints`] lists by flips
    /// (`removevertexbyflips`), raising the automatic flip link level each
    /// round and, after three rounds without progress, trying once with an
    /// unbounded level. `fliplinklevel` is restored at the end.
    pub fn meshcoarsening(&mut self) -> Result<(), TetgenError> {
        // Collect the set of points to be removed.
        let mut remptlist = self.collectremovepoints();
        if remptlist.is_empty() {
            return Ok(());
        }
        self.reachm[10] += 1;

        let mut ms = remptlist.len();
        let mut nit = 0;
        let bak_fliplinklevel = self.b.fliplinklevel;
        self.b.fliplinklevel = -1;
        self.autofliplinklevel = 1; // Init value.

        loop {
            // Remove the list of points.
            let mut i = 0usize;
            while i < remptlist.len() {
                let p = remptlist[i];
                if self.removevertexbyflips(p)? {
                    self.reachm[11] += 1;
                    // Move the last entry to the current place.
                    let last = remptlist.len() - 1;
                    remptlist[i] = remptlist[last];
                    remptlist.truncate(last);
                } else {
                    i += 1;
                }
            }

            if !remptlist.is_empty() {
                if self.b.fliplinklevel >= 0 {
                    break; // We have tried all levels.
                }
                if remptlist.len() == ms {
                    nit += 1;
                    if nit >= 3 {
                        // Do the last round with unbounded flip link level.
                        self.b.fliplinklevel = 100000;
                    }
                } else {
                    ms = remptlist.len();
                    if nit > 0 {
                        nit -= 1;
                    }
                }
                self.autofliplinklevel += self.b.fliplinklevelinc;
            } else {
                // All points are removed.
                break;
            }
        }

        if !remptlist.is_empty() {
            self.reachm[12] += remptlist.len() as u64;
        }
        self.b.fliplinklevel = bak_fliplinklevel;
        Ok(())
    }
}
