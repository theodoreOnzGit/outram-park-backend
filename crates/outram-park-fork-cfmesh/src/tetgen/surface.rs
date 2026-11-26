// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright (C) 2026 OUTRAM PARK contributors
//
// Port of TetGen 1.6.0 (PLC surface meshing: `tetgen.h` subface and
// subsegment primitives, `tetgen.cxx` `makeshellface`, `shellfacedealloc`,
// `shellfacetraverse`, `makeindex2pointmap`, `makepoint2submap`,
// `lu_decmp`, `lu_solve`, `incircle3d`, `facenormal`, `facedihedral`,
// `triarea`, `circumsphere`, `calculateabovepoint`, `calculateabovepoint4`,
// section `surface_cxx`: `flipshpush`, `flip22`, `lawsonflip`,
// `sinsertvertex`, `slocate`, `sscoutsegment`, `scarveholes`,
// `triangulate`, `unifysegments`, `identifyinputedges`, `mergefacets`,
// `meshsurface`, and the part of `tetrahedralize` that runs for `-p` up to
// `meshsurface`):
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

//! PLC surface meshing (stage 3, gh:#1792): what `tetrahedralize()` runs
//! under `-p` up to and including `meshsurface()`.
//!
//! The call sequence is upstream's: `initializepools` ([`TetgenMesh::new`]
//! with [`Behavior::plc`]), `transfernodes`, `incrementaldelaunay` (the
//! stage 2 code, unchanged: the `-p` path inserts the PLC's vertices with
//! `insert_vertex_bw` exactly as for a point set, and calls neither
//! `insertpoint` nor a 3D flip before `meshsurface`), then
//! [`TetgenMesh::meshsurface`]: per facet, the vertex and segment lists,
//! `triangulate` (an initial triangle, `sinsertvertex` with the
//! Bowyer-Watson cavity per vertex, `sscoutsegment` with `flip22` per
//! segment, `lawsonflip`, `scarveholes`), then `unifysegments` (face rings
//! at segments), `identifyinputedges` (with an `.edge` list),
//! `mergefacets` (coplanar facets and small angles) and the ridge vertex
//! types. `meshsurface` does not touch the tetrahedra: the
//! tetrahedralisation after it is the Delaunay one of the vertices.
//!
//! Upstream's `markacutevertices` is not called by `meshsurface` in 1.6.0
//! (it does not exist there), so it is not ported.
//!
//! **Branches not ported**, none reached with default switches and the
//! `meshsurface` call path:
//!
//! - ~~`sinsertvertex`'s `splitseg` and `bowywat == 3` (`INSTAR`) branches:
//!   `triangulate` passes `NULL` and `bowywat = 1`~~ **CORRECTED 2026-11-26
//!   (gh:#1793):** ported for boundary recovery as
//!   [`TetgenMesh::sinsertvertex_full`]; `sinsertvertex` calls it with no
//!   segment;
//! - ~~`flip22`'s `chkencflag` queues (`badsubsegs`, `badsubfacs`): always 0~~
//!   (ported in stage 6, gh:#1795: `flip22_chk`); before stage 6: always 0
//!   here; `flip31` and `sremovevertex` (not called by `meshsurface`;
//!   **ported in stage 4**, gh:#1793, `flip.rs` and `steiner.rs`);
//! - `sscoutsegment`'s `reporterrorflag` printouts (the results are kept);
//! - the area and length bounds (`checkconstraints`, `-q` with
//!   `facetconstraintlist` / `segmentconstraintlist`);
//! - `meshsurface`'s `.stl` branch (`jettisonnodes`): `.poly` / `.smesh`
//!   input is `b->object == POLY`.
//!
//! Where upstream dereferences a `NULL` pointer (it would crash), the port
//! returns [`TetgenError::InternalError`].

#[cfg(not(any(feature = "std", test)))]
#[allow(unused_imports)]
use crate::nostd_prelude::*;

use super::delaunay::{LocateResult, TetgenError};
use super::mesh::{
    decode, encode, sdecode, sencode, Behavior, Face, FlipShItem, Point, ShPool, ShellRecord,
    TetgenMesh, VertType, DUMMYPOINT, NULL_HANDLE, NULL_POINT, NULL_SH, SAPEXPIVOT, SDESTPIVOT,
    SNEXTPIVOT, SORGPIVOT,
};
use super::plc::PlcInput;
use alloc::string::String;
use alloc::vec::Vec;
use core::fmt::Write;

/// `enum interresult` (the values `sscoutsegment` returns).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum InterResult {
    /// `SHAREEDGE`: the segment is an edge of the triangulation.
    ShareEdge,
    /// `ACROSSVERT`: a vertex lies on the segment.
    AcrossVert,
    /// `ACROSSEDGE`: the segment crosses an edge (or a segment).
    AcrossEdge,
}

/// The surface mesh after [`TetgenMesh::meshsurface`], in upstream's pool
/// order: what [`plc_meshsurface`] returns.
#[derive(Debug, Clone)]
pub struct PlcSurface {
    /// The mesh object (stage 4 continues from it).
    pub mesh: TetgenMesh,
    /// The live subfaces in pool order: the three vertices as input indices
    /// (`pointmark - firstnumber`, i.e. 0-based into
    /// [`PlcInput::points`]) and the facet marker.
    pub subfaces: Vec<([usize; 3], i32)>,
    /// The live subsegments in pool order: two input indices and the
    /// segment marker (`-1` for a facet boundary edge, the `.edge` marker or
    /// `-2` for an input edge).
    pub subsegs: Vec<([usize; 2], i32)>,
}

/// Mesh the surface of a PLC as `tetgen -p` does before boundary recovery:
/// the Delaunay tetrahedralisation of its vertices, then `meshsurface`
/// (every facet triangulated into a 2D constrained Delaunay triangulation,
/// coplanar facets with equal markers merged). Default switches.
pub fn plc_meshsurface(input: &PlcInput) -> Result<PlcSurface, TetgenError> {
    plc_meshsurface_with(input, &Behavior::default())
}

/// [`plc_meshsurface`] with explicit switches (`b.plc` is set here).
pub fn plc_meshsurface_with(input: &PlcInput, b: &Behavior) -> Result<PlcSurface, TetgenError> {
    let mesh = TetgenMesh::from_plc(input, b)?;
    let first = mesh.firstnumber;
    let mut subfaces = Vec::with_capacity(mesh.subfacepool.items);
    for r in mesh.subfaces.iter().filter(|r| r.v[0] != NULL_POINT) {
        let idx = |p: Point| (mesh.pointmark(p) - first) as usize;
        subfaces.push(([idx(r.v[0]), idx(r.v[1]), idx(r.v[2])], r.marker));
    }
    let mut subsegs = Vec::with_capacity(mesh.subsegpool.items);
    for r in mesh.subsegs.iter().filter(|r| r.v[0] != NULL_POINT) {
        let idx = |p: Point| (mesh.pointmark(p) - first) as usize;
        subsegs.push(([idx(r.v[0]), idx(r.v[1])], r.marker));
    }
    Ok(PlcSurface {
        mesh,
        subfaces,
        subsegs,
    })
}

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

/// `facenormal(pa, pb, pc, n, pivot, NULL)`.
pub fn facenormal(pa: &[f64; 3], pb: &[f64; 3], pc: &[f64; 3], pivot: i32) -> [f64; 3] {
    let v1 = [pb[0] - pa[0], pb[1] - pa[1], pb[2] - pa[2]]; // a->b
    let v2 = [pa[0] - pc[0], pa[1] - pc[1], pa[2] - pc[2]]; // c->a
    let (pv1, pv2);
    if pivot > 0 {
        // Choose edge vectors by Burdakov's algorithm.
        let v3 = [pc[0] - pb[0], pc[1] - pb[1], pc[2] - pb[2]]; // b->c
        let l1 = dot(&v1, &v1);
        let l2 = dot(&v2, &v2);
        let l3 = dot(&v3, &v3);
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
    } else {
        pv1 = v1;
        pv2 = v2;
    }
    let n = cross(&pv1, &pv2);
    [-n[0], -n[1], -n[2]]
}

/// `lu_decmp(lu, 3, ps, &d, 0)`.
pub(super) fn lu_decmp(lu: &mut [[f64; 4]; 4], n: usize, ps: &mut [usize; 4], d: &mut f64) -> bool {
    let mut scales = [0.0f64; 4];
    let mut pivotindex = 0usize;
    *d = 1.0;
    for i in 0..n {
        let mut biggest = 0.0f64;
        for j in 0..n {
            let tempf = lu[i][j].abs();
            if biggest < tempf {
                biggest = tempf;
            }
        }
        if biggest != 0.0 {
            scales[i] = 1.0 / biggest;
        } else {
            // scales[i] = 0.0 (never read again).
            return false; // Zero row: singular matrix.
        }
        ps[i] = i;
    }
    for k in 0..n - 1 {
        let mut biggest = 0.0f64;
        for i in k..n {
            let tempf = lu[ps[i]][k].abs() * scales[ps[i]];
            if biggest < tempf {
                biggest = tempf;
                pivotindex = i;
            }
        }
        if biggest == 0.0 {
            return false; // Zero column: singular matrix.
        }
        if pivotindex != k {
            ps.swap(k, pivotindex);
            *d = -(*d);
        }
        let pivot = lu[ps[k]][k];
        for i in k + 1..n {
            let mult = lu[ps[i]][k] / pivot;
            lu[ps[i]][k] = mult;
            if mult != 0.0 {
                for j in k + 1..n {
                    lu[ps[i]][j] -= mult * lu[ps[k]][j];
                }
            }
        }
    }
    lu[ps[n - 1]][n - 1] != 0.0
}

/// `lu_solve(lu, 3, ps, b, 0)`.
pub(super) fn lu_solve(lu: &[[f64; 4]; 4], n: usize, ps: &[usize; 4], b: &mut [f64; 4]) {
    let mut x = [0.0f64; 4];
    for i in 0..n {
        let mut dot = 0.0;
        for j in 0..i {
            dot += lu[ps[i]][j] * x[j];
        }
        x[i] = b[ps[i]] - dot;
    }
    for i in (0..n).rev() {
        let mut dot = 0.0;
        for j in i + 1..n {
            dot += lu[ps[i]][j] * x[j];
        }
        x[i] = (x[i] - dot) / lu[ps[i]][i];
    }
    b[..n].copy_from_slice(&x[..n]);
}

/// `circumsphere(pa, pb, pc, NULL, cent, &radius)`: the circumcircle of a
/// triangle (in its plane). Returns `None` when `lu_decmp` finds the system
/// singular (upstream sets the radius to 0 and leaves the centre unset).
fn circumcircle(pa: &[f64; 3], pb: &[f64; 3], pc: &[f64; 3]) -> Option<([f64; 3], f64)> {
    let mut a = [[0.0f64; 4]; 4];
    a[0][0] = pb[0] - pa[0];
    a[0][1] = pb[1] - pa[1];
    a[0][2] = pb[2] - pa[2];
    a[1][0] = pc[0] - pa[0];
    a[1][1] = pc[1] - pa[1];
    a[1][2] = pc[2] - pa[2];
    let r0 = [a[0][0], a[0][1], a[0][2]];
    let r1 = [a[1][0], a[1][1], a[1][2]];
    let c = cross(&r0, &r1);
    a[2][0] = c[0];
    a[2][1] = c[1];
    a[2][2] = c[2];
    let mut rhs = [0.5 * dot(&r0, &r0), 0.5 * dot(&r1, &r1), 0.0, 0.0];
    let mut indx = [0usize; 4];
    let mut d = 0.0;
    if !lu_decmp(&mut a, 3, &mut indx, &mut d) {
        return None;
    }
    lu_solve(&a, 3, &indx, &mut rhs);
    let cent = [pa[0] + rhs[0], pa[1] + rhs[1], pa[2] + rhs[2]];
    let radius = (rhs[0] * rhs[0] + rhs[1] * rhs[1] + rhs[2] * rhs[2]).sqrt();
    Some((cent, radius))
}

/// `triarea(pa, pb, pc)`.
fn triarea(pa: &[f64; 3], pb: &[f64; 3], pc: &[f64; 3]) -> f64 {
    let v1 = [pb[0] - pa[0], pb[1] - pa[1], pb[2] - pa[2]];
    let v2 = [pc[0] - pa[0], pc[1] - pa[1], pc[2] - pa[2]];
    let c = cross(&v1, &v2);
    0.5 * dot(&c, &c).sqrt()
}

impl TetgenMesh {
    // -- the -p driver -----------------------------------------------------

    /// `tetrahedralize()` under `-p`, up to and including `meshsurface()`:
    /// `initializepools`, `transfernodes`, `incrementaldelaunay`,
    /// `meshsurface`.
    pub fn from_plc(input: &PlcInput, b: &Behavior) -> Result<TetgenMesh, TetgenError> {
        if input.points.len() < 4 {
            // load_poly: "TetGen needs at least 4 points."
            return Err(TetgenError::TooFewPoints);
        }
        if input.points.iter().flatten().any(|x| !x.is_finite()) {
            return Err(TetgenError::NonFinite);
        }
        let mut bb = *b;
        bb.plc = true;
        let mut m = TetgenMesh::new(bb);
        m.firstnumber = input.firstnumber;
        m.transfernodes(&input.points)?;
        // transfernodes' "Read the point metric tensor" (-m, gh:#1825).
        m.transfer_point_mtrs(&input.point_mtrs);
        m.incrementaldelaunay()?;
        let mut work = input.clone();
        m.meshsurface(&mut work)?;
        Ok(m)
    }

    // -- shell-face primitives (tetgen.h) ----------------------------------

    /// The record of `s`.
    #[inline]
    pub fn shr(&self, s: &Face) -> &ShellRecord {
        match s.pool {
            ShPool::Sub => &self.subfaces[s.sh],
            ShPool::Seg => &self.subsegs[s.sh],
        }
    }
    /// The record of `s`, mutable.
    #[inline]
    pub fn shr_mut(&mut self, s: &Face) -> &mut ShellRecord {
        match s.pool {
            ShPool::Sub => &mut self.subfaces[s.sh],
            ShPool::Seg => &mut self.subsegs[s.sh],
        }
    }
    /// `sbond(s1, s2)`.
    #[inline]
    pub fn sbond(&mut self, s1: &Face, s2: &Face) {
        self.shr_mut(s1).nbr[s1.shver >> 1] = sencode(s2);
        self.shr_mut(s2).nbr[s2.shver >> 1] = sencode(s1);
    }
    /// `sbond1(s1, s2)`: `s1` points to `s2`.
    #[inline]
    pub fn sbond1(&mut self, s1: &Face, s2: &Face) {
        self.shr_mut(s1).nbr[s1.shver >> 1] = sencode(s2);
    }
    /// `sdissolve(s)`.
    #[inline]
    pub fn sdissolve(&mut self, s: &Face) {
        self.shr_mut(s).nbr[s.shver >> 1] = 0;
    }
    /// `spivot(s1, s2)`.
    #[inline]
    pub fn spivot(&self, s: &Face) -> Face {
        sdecode(self.shr(s).nbr[s.shver >> 1])
    }
    /// `sorg(s)`.
    #[inline]
    pub fn sorg(&self, s: &Face) -> Point {
        self.shr(s).v[SORGPIVOT[s.shver] - 3]
    }
    /// `sdest(s)`.
    #[inline]
    pub fn sdest(&self, s: &Face) -> Point {
        self.shr(s).v[SDESTPIVOT[s.shver] - 3]
    }
    /// `sapex(s)`.
    #[inline]
    pub fn sapex(&self, s: &Face) -> Point {
        self.shr(s).v[SAPEXPIVOT[s.shver] - 3]
    }
    /// `setshvertices(s, pa, pb, pc)` (`setsorg`, `setsdest`, `setsapex`).
    #[inline]
    pub fn setshvertices(&mut self, s: &Face, pa: Point, pb: Point, pc: Point) {
        let r = self.shr_mut(s);
        r.v[SORGPIVOT[s.shver] - 3] = pa;
        r.v[SDESTPIVOT[s.shver] - 3] = pb;
        r.v[SAPEXPIVOT[s.shver] - 3] = pc;
    }
    /// `shellmark(s)`.
    #[inline]
    pub fn shellmark(&self, s: &Face) -> i32 {
        self.shr(s).marker
    }
    /// `setshellmark(s, value)`.
    #[inline]
    pub fn setshellmark(&mut self, s: &Face, value: i32) {
        self.shr_mut(s).marker = value;
    }
    /// `sinfect(s)`.
    #[inline]
    pub fn sinfect(&mut self, s: &Face) {
        self.shr_mut(s).flags |= 1;
    }
    /// `sinfected(s)`.
    #[inline]
    pub fn sinfected(&self, s: &Face) -> bool {
        (self.shr(s).flags & 1) != 0
    }
    /// `smarktest(s)`.
    #[inline]
    pub fn smarktest(&mut self, s: &Face) {
        self.shr_mut(s).flags |= 2;
    }
    /// `sunmarktest(s)`.
    #[inline]
    pub fn sunmarktest(&mut self, s: &Face) {
        self.shr_mut(s).flags &= !2;
    }
    /// `smarktested(s)`.
    #[inline]
    pub fn smarktested(&self, s: &Face) -> bool {
        (self.shr(s).flags & 2) != 0
    }
    /// `ssbond(s, edge)`.
    #[inline]
    pub fn ssbond(&mut self, s: &Face, edge: &Face) {
        self.shr_mut(s).seg[s.shver >> 1] = sencode(edge);
        self.shr_mut(edge).nbr[0] = sencode(s);
    }
    /// `ssdissolve(s)`.
    #[inline]
    pub fn ssdissolve(&mut self, s: &Face) {
        self.shr_mut(s).seg[s.shver >> 1] = 0;
    }
    /// `sspivot(s, edge)`.
    #[inline]
    pub fn sspivot(&self, s: &Face) -> Face {
        sdecode(self.shr(s).seg[s.shver >> 1])
    }
    /// `isshsubseg(s)`.
    #[inline]
    pub fn isshsubseg(&self, s: &Face) -> bool {
        self.shr(s).seg[s.shver >> 1] != 0
    }
    /// `isshtet(s)`.
    #[inline]
    pub fn isshtet(&self, s: &Face) -> bool {
        self.shr(s).tet[s.shver & 1] != NULL_HANDLE
    }
    /// `pinfect(p)`.
    #[inline]
    pub fn pinfect(&mut self, p: Point) {
        self.points[p].flags |= 1;
    }
    /// `puninfect(p)`.
    #[inline]
    pub fn puninfect(&mut self, p: Point) {
        self.points[p].flags &= !1;
    }
    /// `pinfected(p)`.
    #[inline]
    pub fn pinfected(&self, p: Point) -> bool {
        (self.points[p].flags & 1) != 0
    }

    /// `makeshellface(pool, &newface)`: allocate a record (the last freed
    /// slot first) and clear it.
    pub fn makeshellface(&mut self, pool: ShPool) -> Face {
        let (slot, fresh) = match pool {
            ShPool::Sub => self.subfacepool.alloc(),
            ShPool::Seg => self.subsegpool.alloc(),
        };
        let recs = match pool {
            ShPool::Sub => &mut self.subfaces,
            ShPool::Seg => &mut self.subsegs,
        };
        if fresh {
            recs.push(ShellRecord::BLANK);
        }
        recs[slot] = ShellRecord::BLANK;
        Face {
            pool,
            sh: slot,
            shver: 0,
        }
    }

    /// `shellfacedealloc(pool, sh)`: mark dead (`sh[3] = NULL`) and free.
    /// `memorypool::dealloc` then writes the dead-item link into the
    /// record's first word (`sh[0]`); that write is emulated too.
    pub fn shellfacedealloc(&mut self, pool: ShPool, slot: usize) {
        let (mp, recs) = match pool {
            ShPool::Sub => (&mut self.subfacepool, &mut self.subfaces),
            ShPool::Seg => (&mut self.subsegpool, &mut self.subsegs),
        };
        recs[slot].v[0] = NULL_POINT;
        let link = match mp.deaditemstack.last() {
            Some(&top) => sencode(&Face {
                pool,
                sh: top,
                shver: 0,
            }),
            None => 0,
        };
        recs[slot].nbr[0] = link;
        mp.dealloc(slot);
    }

    /// `shellfacetraverse(pool)`: the next live record in pool order.
    pub fn shellfacetraverse(&mut self, pool: ShPool) -> Option<usize> {
        loop {
            let s = match pool {
                ShPool::Sub => self.subfacepool.traverse()?,
                ShPool::Seg => self.subsegpool.traverse()?,
            };
            let dead = match pool {
                ShPool::Sub => self.subfaces[s].v[0] == NULL_POINT,
                ShPool::Seg => self.subsegs[s].v[0] == NULL_POINT,
            };
            if !dead {
                return Some(s);
            }
        }
    }

    /// `traversalinit()` of a shell-face pool.
    fn shtraversalinit(&mut self, pool: ShPool) {
        match pool {
            ShPool::Sub => self.subfacepool.traversalinit(),
            ShPool::Seg => self.subsegpool.traversalinit(),
        }
    }

    #[inline]
    fn orient3d_p(&self, a: Point, b: Point, c: Point, d: Point) -> f64 {
        self.pred
            .orient3d(self.xyz(a), self.xyz(b), self.xyz(c), self.xyz(d))
    }

    #[inline]
    fn facenormal_p(&self, a: Point, b: Point, c: Point) -> [f64; 3] {
        facenormal(self.xyz(a), self.xyz(b), self.xyz(c), 1)
    }

    #[inline]
    fn distance_p(&self, a: Point, b: Point) -> f64 {
        distance(self.xyz(a), self.xyz(b))
    }

    // -- geometry (geom_cxx) ------------------------------------------------

    /// `incircle3d(pa, pb, pc, pd)`: negative if `pd` is inside the
    /// circumcircle of `[a, b, c]` (`[a, b]` the common edge of `[a, b, c]`
    /// and `[b, a, d]`), rounded to 0 within `b->epsilon`.
    pub fn incircle3d(&self, pa: Point, pb: Point, pc: Point, pd: Point) -> f64 {
        let n1 = self.facenormal_p(pa, pb, pc);
        let area2_0 = dot(&n1, &n1);
        let n2 = self.facenormal_p(pb, pa, pd);
        let area2_1 = dot(&n2, &n2);
        let (c, r, d);
        if area2_0 > area2_1 {
            // Choose [a, b, c] as the base triangle.
            let (cc, rr) =
                circumcircle(self.xyz(pa), self.xyz(pb), self.xyz(pc)).unwrap_or(([0.0; 3], 0.0));
            c = cc;
            r = rr;
            d = distance(&c, self.xyz(pd));
        } else if area2_1 > 0.0 {
            let (cc, rr) =
                circumcircle(self.xyz(pb), self.xyz(pa), self.xyz(pd)).unwrap_or(([0.0; 3], 0.0));
            c = cc;
            r = rr;
            d = distance(&c, self.xyz(pc));
        } else {
            // The four points are collinear.
            return 0.0;
        }
        let mut sign = d - r;
        if sign.abs() / r < self.b.epsilon {
            sign = 0.0;
        }
        sign
    }

    /// `facedihedral(pa, pb, pc1, pc2)`: the angle (`0..2 pi`) between the
    /// normals of `[a, b, c1]` and `[a, b, c2]`.
    pub fn facedihedral(&self, pa: Point, pb: Point, pc1: Point, pc2: Point) -> f64 {
        let n1 = self.facenormal_p(pa, pb, pc1);
        let n2 = self.facenormal_p(pa, pb, pc2);
        let n1len = dot(&n1, &n1).sqrt();
        let n2len = dot(&n2, &n2).sqrt();
        let mut costheta = dot(&n1, &n2) / (n1len * n2len);
        if costheta > 1.0 {
            costheta = 1.0;
        } else if costheta < -1.0 {
            costheta = -1.0;
        }
        let mut theta = costheta.acos();
        let ori = self.orient3d_p(pa, pb, pc1, pc2);
        if ori > 0.0 {
            theta = 2.0 * core::f64::consts::PI - theta;
        }
        theta
    }

    /// `calculateabovepoint(facpoints, &pa, &pb, &pc)`: three far-apart
    /// points of the facet, and `dummypoint` set above it.
    pub fn calculateabovepoint(&mut self, facpoints: &[Point]) -> Option<(Point, Point, Point)> {
        let pa = facpoints[0];
        let mut pb = NULL_POINT;
        let mut pc = NULL_POINT;
        let a = *self.xyz(pa);
        // Get a point b s.t. the length of [a, b] is maximal.
        let mut lab = 0.0f64;
        for &q in &facpoints[1..] {
            let p = self.xyz(q);
            let x = p[0] - a[0];
            let y = p[1] - a[1];
            let z = p[2] - a[2];
            let len = x * x + y * y + z * z;
            if len > lab {
                lab = len;
                pb = q;
            }
        }
        lab = lab.sqrt();
        if lab == 0.0 {
            let w = alloc::format!(
                "Warning:  All points of a facet are coincident with {}.",
                self.pointmark(pa)
            );
            self.warnings.push(w);
            return None;
        }
        // Get a point c s.t. the area of [a, b, c] is maximal.
        let bb = *self.xyz(pb);
        let v1 = [bb[0] - a[0], bb[1] - a[1], bb[2] - a[2]];
        let mut aa = 0.0f64;
        for &q in &facpoints[1..] {
            let p = self.xyz(q);
            let v2 = [p[0] - a[0], p[1] - a[1], p[2] - a[2]];
            let n = cross(&v1, &v2);
            let area = dot(&n, &n);
            if area > aa {
                aa = area;
                pc = q;
            }
        }
        if aa == 0.0 {
            let w = alloc::format!(
                "Warning:  All points of a facet are collinaer with [{}, {}].",
                self.pointmark(pa),
                self.pointmark(pb)
            );
            self.warnings.push(w);
            return None;
        }
        // Calculate an above point of this facet.
        let mut n = self.facenormal_p(pa, pb, pc);
        let len = dot(&n, &n).sqrt();
        n[0] /= len;
        n[1] /= len;
        n[2] /= len;
        lab /= 2.0; // Half the maximal length.
        self.points[DUMMYPOINT].xyz = [a[0] + lab * n[0], a[1] + lab * n[1], a[2] + lab * n[2]];
        Some((pa, pb, pc))
    }

    /// `calculateabovepoint4(pa, pb, pc, pd)`, with `pd` given by its
    /// coordinates.
    pub fn calculateabovepoint4(&mut self, pa: Point, pb: Point, pc: Point, pd: &[f64; 3]) {
        let n1 = self.facenormal_p(pa, pb, pc);
        let len1 = dot(&n1, &n1).sqrt();
        let n2 = facenormal(self.xyz(pa), self.xyz(pb), pd, 1);
        let len2 = dot(&n2, &n2).sqrt();
        let (mut norm, len) = if len1 > len2 { (n1, len1) } else { (n2, len2) };
        norm[0] /= len;
        norm[1] /= len;
        norm[2] /= len;
        let len = self.distance_p(pa, pb);
        let a = *self.xyz(pa);
        self.points[DUMMYPOINT].xyz = [
            a[0] + len * norm[0],
            a[1] + len * norm[1],
            a[2] + len * norm[2],
        ];
    }

    // -- maps ---------------------------------------------------------------

    /// `makeindex2pointmap(idx2verlist)`: point by index (from
    /// `in->firstnumber`), in pool order.
    pub fn makeindex2pointmap(&mut self) -> Vec<Point> {
        let mut idx2verlist = alloc::vec![NULL_POINT; self.pointpool.items + 1];
        self.pointpool.traversalinit();
        let mut idx = self.firstnumber as usize;
        while let Some(p) = self.pointtraverse() {
            idx2verlist[idx] = p;
            idx += 1;
        }
        idx2verlist
    }

    /// `makepoint2submap(pool, idx2faclist, facperverlist)`: the records at
    /// each vertex (vertex `i` counted from 0), each with that vertex as
    /// origin.
    pub fn makepoint2submap(&mut self, pool: ShPool) -> (Vec<usize>, Vec<Face>) {
        let n = self.pointpool.items;
        let first = self.firstnumber;
        let mut idx2faclist = alloc::vec![0usize; n + 1];
        self.shtraversalinit(pool);
        while let Some(s) = self.shellfacetraverse(pool) {
            let f = Face {
                pool,
                sh: s,
                shver: 0,
            };
            let r = *self.shr(&f);
            idx2faclist[(self.pointmark(r.v[0]) - first) as usize] += 1;
            idx2faclist[(self.pointmark(r.v[1]) - first) as usize] += 1;
            if r.v[2] != NULL_POINT {
                idx2faclist[(self.pointmark(r.v[2]) - first) as usize] += 1;
            }
        }
        let mut j = idx2faclist[0];
        idx2faclist[0] = 0;
        for i in 0..n {
            let k = idx2faclist[i + 1];
            idx2faclist[i + 1] = idx2faclist[i] + j;
            j = k;
        }
        let mut facperverlist = alloc::vec![Face::default(); idx2faclist[n]];
        self.shtraversalinit(pool);
        while let Some(s) = self.shellfacetraverse(pool) {
            let mut shloop = Face {
                pool,
                sh: s,
                shver: 0,
            };
            let r = *self.shr(&shloop);
            let j = (self.pointmark(r.v[0]) - first) as usize;
            shloop.shver = 0;
            facperverlist[idx2faclist[j]] = shloop;
            idx2faclist[j] += 1;
            if r.v[2] != NULL_POINT {
                let j = (self.pointmark(r.v[1]) - first) as usize;
                shloop.shver = 2;
                facperverlist[idx2faclist[j]] = shloop;
                idx2faclist[j] += 1;
                let j = (self.pointmark(r.v[2]) - first) as usize;
                shloop.shver = 4;
                facperverlist[idx2faclist[j]] = shloop;
                idx2faclist[j] += 1;
            } else {
                let j = (self.pointmark(r.v[1]) - first) as usize;
                shloop.shver = 1;
                facperverlist[idx2faclist[j]] = shloop;
                idx2faclist[j] += 1;
            }
        }
        for i in (0..n).rev() {
            idx2faclist[i + 1] = idx2faclist[i];
        }
        idx2faclist[0] = 0;
        (idx2faclist, facperverlist)
    }

    // -- surface flips ------------------------------------------------------

    /// `flipshpush(flipedge)`.
    pub fn flipshpush(&mut self, flipedge: &Face) {
        let item = FlipShItem {
            ss: *flipedge,
            forg: self.sorg(flipedge),
            fdest: self.sdest(flipedge),
        };
        self.flipstack.push(item);
    }

    /// `flip22(flipfaces, flipflag, 0)`: `[a,b,c]`, `[b,a,d]` become
    /// `[c,d,b]`, `[d,c,a]`. `flipfaces[1]` may be reversed on return, as
    /// upstream.
    pub fn flip22(&mut self, flipfaces: &mut [Face; 2], flipflag: bool) {
        self.flip22_chk(flipfaces, flipflag, 0);
    }

    /// `flip22(flipfaces, flipflag, chkencflag)`: [`TetgenMesh::flip22`]
    /// with the encroachment queues of Delaunay refinement (stage 6,
    /// gh:#1795): `chkencflag & 1` queues the four boundary segments met,
    /// `& 2` the two flipped subfaces.
    pub fn flip22_chk(&mut self, flipfaces: &mut [Face; 2], flipflag: bool, chkencflag: i32) {
        self.reach[31] += 1;
        let pa = self.sorg(&flipfaces[0]);
        let pb = self.sdest(&flipfaces[0]);
        let pc = self.sapex(&flipfaces[0]);
        let pd = self.sapex(&flipfaces[1]);

        if self.sorg(&flipfaces[1]) != pb {
            flipfaces[1].shver ^= 1; // sesymself
        }

        self.flip22count += 1;

        // Collect the four boundary edges.
        let senext = |f: &Face| Face {
            shver: SNEXTPIVOT[f.shver],
            ..*f
        };
        let senext2 = |f: &Face| Face {
            shver: SNEXTPIVOT[SNEXTPIVOT[f.shver]],
            ..*f
        };
        let mut bdedges = [
            senext(&flipfaces[0]),
            senext2(&flipfaces[0]),
            senext(&flipfaces[1]),
            senext2(&flipfaces[1]),
        ];
        let mut outfaces = [Face::default(); 4];
        let mut infaces = [Face::default(); 4];
        let mut bdsegs = [Face::default(); 4];

        // Collect outer boundary faces.
        for i in 0..4 {
            outfaces[i] = self.spivot(&bdedges[i]);
            infaces[i] = outfaces[i];
            bdsegs[i] = self.sspivot(&bdedges[i]);
            if !outfaces[i].is_null() && self.isshsubseg(&bdedges[i]) {
                let mut checkface = self.spivot(&infaces[i]);
                while !checkface.same_sh(&bdedges[i]) {
                    infaces[i] = checkface;
                    checkface = self.spivot(&infaces[i]);
                }
            }
        }

        // Transform [a,b,c] -> [c,d,b].
        self.setshvertices(&flipfaces[0], pc, pd, pb);
        // Transform [b,a,d] -> [d,c,a].
        self.setshvertices(&flipfaces[1], pd, pc, pa);

        // Update the point-to-subface map.
        if self.pointtype(pa) == VertType::FreeFacet {
            self.points[pa].point2sh = sencode(&flipfaces[1]);
        }
        if self.pointtype(pb) == VertType::FreeFacet {
            self.points[pb].point2sh = sencode(&flipfaces[0]);
        }
        if self.pointtype(pc) == VertType::FreeFacet {
            self.points[pc].point2sh = sencode(&flipfaces[0]);
        }
        if self.pointtype(pd) == VertType::FreeFacet {
            self.points[pd].point2sh = sencode(&flipfaces[0]);
        }

        // Reconnect boundary edges to outer boundary faces.
        for i in 0..4 {
            let k = (3 + i) % 4;
            if !outfaces[k].is_null() {
                // Make sure that the subface has the ori as the segment.
                if !bdsegs[k].is_null() {
                    bdsegs[k].shver = 0;
                    if self.sorg(&bdedges[i]) != self.sorg(&bdsegs[k]) {
                        bdedges[i].shver ^= 1;
                    }
                }
                self.sbond1(&bdedges[i], &outfaces[k]);
                self.sbond1(&infaces[k], &bdedges[i]);
            } else {
                self.sdissolve(&bdedges[i]);
            }
            if !bdsegs[k].is_null() {
                self.ssbond(&bdedges[i], &bdsegs[k]);
                if chkencflag & 1 != 0 {
                    // Queue this segment for encroaching check.
                    let sg = bdsegs[k];
                    self.enqueuesubface(&sg);
                }
            } else {
                self.ssdissolve(&bdedges[i]);
            }
        }

        if chkencflag & 2 != 0 {
            // Queue the flipped subfaces for quality/encroaching checks.
            for f in flipfaces.iter() {
                let f = *f;
                self.enqueuesubface(&f);
            }
        }

        self.recentsh = flipfaces[0];

        if flipflag {
            // Put the boundary edges into flip stack.
            for e in &bdedges {
                self.flipshpush(e);
            }
        }
    }

    /// `lawsonflip()`: flip non-locally Delaunay edges until the stack is
    /// empty; returns the number of flips.
    pub fn lawsonflip(&mut self) -> i64 {
        let mut flipcount = 0;
        while let Some(popface) = self.flipstack.pop() {
            let mut flipfaces = [popface.ss, Face::default()];
            let pa = popface.forg;
            let pb = popface.fdest;
            // Skip it if it is dead.
            if self.shr(&flipfaces[0]).v[0] == NULL_POINT {
                continue;
            }
            // Skip it if it is not the same edge as we saved.
            if self.sorg(&flipfaces[0]) != pa || self.sdest(&flipfaces[0]) != pb {
                continue;
            }
            // Skip it if it is a subsegment.
            if self.isshsubseg(&flipfaces[0]) {
                continue;
            }
            // Get the adjacent face.
            flipfaces[1] = self.spivot(&flipfaces[0]);
            if flipfaces[1].is_null() {
                continue; // Skip a hull edge.
            }
            let pc = self.sapex(&flipfaces[0]);
            let pd = self.sapex(&flipfaces[1]);
            let sign = self.incircle3d(pa, pb, pc, pd);
            if sign < 0.0 {
                // It is non-locally Delaunay. Flip it.
                self.flip22(&mut flipfaces, true);
                flipcount += 1;
            }
        }
        flipcount
    }

    // -- surface point location and insertion -------------------------------

    /// `slocate(searchpt, searchsh, aflag, cflag, rflag)`. The point is
    /// given by its coordinates `sp` and, when it is a mesh point, its index
    /// `spid` (upstream compares the pointer: a hole point given as raw
    /// coordinates, as `scarveholes` passes, is never a vertex; pass
    /// [`NULL_POINT`]).
    pub fn slocate(
        &mut self,
        sp: &[f64; 3],
        spid: Point,
        searchsh: &mut Face,
        aflag: bool,
        cflag: bool,
        rflag: bool,
    ) -> LocateResult {
        #[derive(PartialEq)]
        enum Move {
            Bc,
            Ca,
        }
        let mut pa = self.sorg(searchsh);
        let mut pb = self.sdest(searchsh);
        let mut pc = self.sapex(searchsh);

        if !aflag {
            // No above point is given. Calculate an above point for this facet.
            self.calculateabovepoint4(pa, pb, pc, sp);
        }

        // 'dummypoint' is given. Make sure it is above [a,b,c]
        let ori = self.orient3d_p(pa, pb, pc, DUMMYPOINT);
        if ori > 0.0 {
            searchsh.shver ^= 1; // Reverse the face orientation.
        } else if ori == 0.0 {
            return LocateResult::Unknown;
        }

        let dp = *self.xyz(DUMMYPOINT);
        // Find an edge of the face s.t. p lies on its right-hand side (CCW).
        let mut i = 0;
        while i < 3 {
            pa = self.sorg(searchsh);
            pb = self.sdest(searchsh);
            let ori = self.pred.orient3d(self.xyz(pa), self.xyz(pb), &dp, sp);
            if ori > 0.0 {
                break;
            }
            searchsh.shver = SNEXTPIVOT[searchsh.shver];
            i += 1;
        }
        if i == 3 {
            return LocateResult::Unknown;
        }

        pc = self.sapex(searchsh);

        if pc == spid {
            searchsh.shver = SNEXTPIVOT[SNEXTPIVOT[searchsh.shver]];
            return LocateResult::OnVertex;
        }

        let mut loc;
        loop {
            let ori_bc = self.pred.orient3d(self.xyz(pb), self.xyz(pc), &dp, sp);
            let ori_ca = self.pred.orient3d(self.xyz(pc), self.xyz(pa), &dp, sp);

            let nextmove;
            if ori_bc < 0.0 {
                if ori_ca < 0.0 {
                    // (--) Any of the edges is a viable move.
                    if self.randomnation(2) != 0 {
                        nextmove = Move::Ca;
                    } else {
                        nextmove = Move::Bc;
                    }
                } else {
                    nextmove = Move::Bc; // (-#)
                }
            } else if ori_ca < 0.0 {
                nextmove = Move::Ca; // (#-)
            } else if ori_bc > 0.0 {
                if ori_ca > 0.0 {
                    loc = LocateResult::OnFace; // (++)
                    break;
                } else {
                    // (+0) On edge [c, a].
                    searchsh.shver = SNEXTPIVOT[SNEXTPIVOT[searchsh.shver]];
                    loc = LocateResult::OnEdge;
                    break;
                }
            } else if ori_ca > 0.0 {
                // (0+) On edge [b, c].
                searchsh.shver = SNEXTPIVOT[searchsh.shver];
                loc = LocateResult::OnEdge;
                break;
            } else {
                // (00) p is coincident with vertex c.
                searchsh.shver = SNEXTPIVOT[SNEXTPIVOT[searchsh.shver]];
                return LocateResult::OnVertex;
            }

            // Move to the next face.
            if nextmove == Move::Bc {
                searchsh.shver = SNEXTPIVOT[searchsh.shver];
            } else {
                searchsh.shver = SNEXTPIVOT[SNEXTPIVOT[searchsh.shver]];
            }
            if !cflag && self.isshsubseg(searchsh) {
                // NON-convex case. Check if we will cross a boundary.
                return LocateResult::EncSegment;
            }
            let mut neighsh = self.spivot(searchsh);
            if neighsh.is_null() {
                return LocateResult::Outside; // A hull edge.
            }
            // Adjust the edge orientation.
            if self.sorg(&neighsh) != self.sdest(searchsh) {
                neighsh.shver ^= 1;
            }

            // Update the newly discovered face and its endpoints.
            *searchsh = neighsh;
            pa = self.sorg(searchsh);
            pb = self.sdest(searchsh);
            pc = self.sapex(searchsh);

            if pc == spid {
                searchsh.shver = SNEXTPIVOT[SNEXTPIVOT[searchsh.shver]];
                return LocateResult::OnVertex;
            }
        }

        if rflag {
            // Round the locate result before return.
            let pa = self.sorg(searchsh);
            let pb = self.sdest(searchsh);
            let pc = self.sapex(searchsh);

            let n = self.facenormal_p(pa, pb, pc);
            let area_abc = dot(&n, &n).sqrt();

            let n = facenormal(self.xyz(pb), self.xyz(pc), sp, 1);
            let mut area_bcp = dot(&n, &n).sqrt();
            if (area_bcp / area_abc) < self.b.epsilon {
                area_bcp = 0.0;
            }

            let n = facenormal(self.xyz(pc), self.xyz(pa), sp, 1);
            let mut area_cap = dot(&n, &n).sqrt();
            if (area_cap / area_abc) < self.b.epsilon {
                area_cap = 0.0;
            }

            let mut area_abp;
            if loc == LocateResult::OnFace || loc == LocateResult::Outside {
                let n = facenormal(self.xyz(pa), self.xyz(pb), sp, 1);
                area_abp = dot(&n, &n).sqrt();
                if (area_abp / area_abc) < self.b.epsilon {
                    area_abp = 0.0;
                }
            } else {
                area_abp = 0.0;
            }

            if area_abp == 0.0 {
                if area_bcp == 0.0 {
                    searchsh.shver = SNEXTPIVOT[searchsh.shver];
                    loc = LocateResult::OnVertex; // p is close to b.
                } else if area_cap == 0.0 {
                    loc = LocateResult::OnVertex; // p is close to a.
                } else {
                    loc = LocateResult::OnEdge; // p is on edge [a,b].
                }
            } else if area_bcp == 0.0 {
                if area_cap == 0.0 {
                    searchsh.shver = SNEXTPIVOT[SNEXTPIVOT[searchsh.shver]];
                    loc = LocateResult::OnVertex; // p is close to c.
                } else {
                    searchsh.shver = SNEXTPIVOT[searchsh.shver];
                    loc = LocateResult::OnEdge; // p is on edge [b,c].
                }
            } else if area_cap == 0.0 {
                searchsh.shver = SNEXTPIVOT[SNEXTPIVOT[searchsh.shver]];
                loc = LocateResult::OnEdge; // p is on edge [c,a].
            } else {
                loc = LocateResult::OnFace; // p is on face [a,b,c].
            }
        }

        loc
    }

    /// `sinsertvertex(insertpt, searchsh, NULL, iloc, bowywat, rflag)`:
    /// insert a vertex into a facet's triangulation (no segment to split;
    /// see the module notes). On return `caveshlist` holds the old
    /// subfaces in the cavity and `caveshbdlist` its boundary edges, each
    /// bonded to its new subface.
    pub fn sinsertvertex(
        &mut self,
        insertpt: Point,
        searchsh: &mut Face,
        iloc: LocateResult,
        bowywat: i32,
        rflag: bool,
    ) -> LocateResult {
        self.sinsertvertex_full(insertpt, searchsh, None, iloc, bowywat, rflag)
    }

    /// `sinsertvertex(insertpt, searchsh, splitseg, iloc, bowywat, rflag)`
    /// with every branch (stage 4, gh:#1793): a segment to split
    /// (`splitseg`, the two new subsegments are returned in
    /// `cavesegshlist`) and `bowywat == 3` (`INSTAR`: the sub-cavity was
    /// formed by the caller). [`TetgenMesh::sinsertvertex`] is this with no
    /// segment.
    pub fn sinsertvertex_full(
        &mut self,
        insertpt: Point,
        searchsh: &mut Face,
        mut splitseg: Option<&mut Face>,
        iloc: LocateResult,
        bowywat: i32,
        rflag: bool,
    ) -> LocateResult {
        self.reach[28] += 1;
        let mut loc = LocateResult::Outside;
        if bowywat == 3 {
            loc = LocateResult::InStar;
        }
        let has_seg = splitseg.as_ref().is_some_and(|s| !s.is_null());
        if has_seg {
            // A segment is going to be split, no point location.
            *searchsh = self.spivot(splitseg.as_ref().unwrap());
            if loc != LocateResult::InStar {
                loc = LocateResult::OnEdge;
            }
        } else {
            if loc != LocateResult::InStar {
                loc = iloc;
            }
            if loc == LocateResult::Outside {
                // Do point location in surface mesh.
                if searchsh.is_null() {
                    *searchsh = self.recentsh;
                }
                // Search the vertex. An above point must be provided ('aflag' = 1).
                let sp = *self.xyz(insertpt);
                loc = self.slocate(&sp, insertpt, searchsh, true, true, rflag);
            }
        }

        let senext = |f: &Face| Face {
            shver: SNEXTPIVOT[f.shver],
            ..*f
        };
        let senext2 = |f: &Face| Face {
            shver: SNEXTPIVOT[SNEXTPIVOT[f.shver]],
            ..*f
        };

        // Form the initial sC(p).
        if loc == LocateResult::OnFace {
            self.smarktest(searchsh);
            self.caveshlist.push(*searchsh);
        } else if loc == LocateResult::OnEdge {
            let pa = if has_seg {
                let sg = splitseg.as_mut().unwrap();
                sg.shver = 0;
                self.sorg(sg)
            } else {
                self.sorg(searchsh)
            };
            if !searchsh.is_null() {
                // Collect all subfaces share at this edge.
                let mut neighsh = *searchsh;
                loop {
                    // Adjust the origin of its edge to be 'pa'.
                    if self.sorg(&neighsh) != pa {
                        neighsh.shver ^= 1;
                    }
                    self.smarktest(&neighsh);
                    self.caveshlist.push(neighsh);
                    self.cavesegshlist.push(neighsh);
                    neighsh = self.spivot(&neighsh);
                    if neighsh.same_sh(searchsh) {
                        break;
                    }
                    if neighsh.is_null() {
                        break;
                    }
                }
            }
        } else if loc == LocateResult::OnVertex {
            return loc;
        } else if loc == LocateResult::Outside {
            // Enlarge the convex hull of the triangulation by including p.
            let mut neighsh = *searchsh;
            loop {
                neighsh = senext2(&neighsh);
                let mut casout = self.spivot(&neighsh);
                if casout.is_null() {
                    // A convex hull edge. Is it visible by p.
                    let ori = self.orient3d_p(
                        self.sorg(&neighsh),
                        self.sdest(&neighsh),
                        DUMMYPOINT,
                        insertpt,
                    );
                    if ori < 0.0 {
                        *searchsh = neighsh; // Visible, update 'searchsh'.
                    } else {
                        break; // 'searchsh' is the right-most visible edge.
                    }
                } else {
                    if self.sorg(&casout) != self.sdest(&neighsh) {
                        casout.shver ^= 1;
                    }
                    neighsh = casout;
                }
            }
            // Create new triangles for all visible edges of p (from right to left).
            let mut casin = Face::default(); // No adjacent face at right.
            let mut pa = self.sorg(searchsh);
            let mut pb = self.sdest(searchsh);
            loop {
                // Create a new subface on top of the (visible) edge.
                let newsh = self.makeshellface(ShPool::Sub);
                self.setshvertices(&newsh, pb, pa, insertpt);
                let mk = self.shellmark(searchsh);
                self.setshellmark(&newsh, mk);
                // useinsertradius (-p): setfacetindex(newsh, getfacetindex(*searchsh)).
                let fi = self.shr(searchsh).facetindex;
                self.shr_mut(&newsh).facetindex = fi;
                // Connect the new subface to the bottom subfaces.
                self.sbond1(&newsh, searchsh);
                self.sbond1(searchsh, &newsh);
                // Connect the new subface to its right-adjacent subface.
                if !casin.is_null() {
                    let casout = senext(&newsh);
                    self.sbond1(&casout, &casin);
                    self.sbond1(&casin, &casout);
                }
                // The left-adjacent subface has not been created yet.
                casin = senext2(&newsh);
                // Add the new face into list (inside the B-W cavity).
                self.smarktest(&newsh);
                self.caveshlist.push(newsh);
                // Move to the convex hull edge at the left of 'searchsh'.
                let mut neighsh = *searchsh;
                loop {
                    neighsh = senext(&neighsh);
                    let mut casout = self.spivot(&neighsh);
                    if casout.is_null() {
                        *searchsh = neighsh;
                        break;
                    }
                    if self.sorg(&casout) != self.sdest(&neighsh) {
                        casout.shver ^= 1;
                    }
                    neighsh = casout;
                }
                // A convex hull edge. Is it visible by p.
                pa = self.sorg(searchsh);
                pb = self.sdest(searchsh);
                let ori = self.orient3d_p(pa, pb, DUMMYPOINT, insertpt);
                // Finish the process if p is not visible by the hull edge.
                if ori >= 0.0 {
                    break;
                }
            }
        }

        // Form the Bowyer-Watson cavity sC(p).
        let mut i = 0;
        while i < self.caveshlist.len() {
            let mut cavesh = self.caveshlist[i];
            for _ in 0..3 {
                let sign;
                if !self.isshsubseg(&cavesh) {
                    let neighsh = self.spivot(&cavesh);
                    if !neighsh.is_null() {
                        // The adjacent face exists.
                        if !self.smarktested(&neighsh) {
                            if bowywat != 0 {
                                let s = if loc == LocateResult::InStar {
                                    // It must be a boundary edge.
                                    1.0
                                } else if !self.isshtet(&neighsh) {
                                    // Check if the subface is non-Delaunay wrt. the new pt.
                                    self.incircle3d(
                                        self.sorg(&neighsh),
                                        self.sdest(&neighsh),
                                        self.sapex(&neighsh),
                                        insertpt,
                                    )
                                } else {
                                    1.0 // A boundary edge.
                                };
                                if s < 0.0 {
                                    // Add the adjacent face in list (in B-W cavity).
                                    self.smarktest(&neighsh);
                                    self.caveshlist.push(neighsh);
                                }
                                sign = s;
                            } else {
                                sign = 1.0; // A boundary edge.
                            }
                        } else {
                            sign = -1.0; // Not a boundary edge.
                        }
                    } else {
                        // No adjacent face. It is a hull edge.
                        if loc == LocateResult::Outside {
                            // It is a boundary edge if it does not contain p.
                            if self.sorg(&cavesh) == insertpt || self.sdest(&cavesh) == insertpt {
                                sign = -1.0;
                            } else {
                                sign = 1.0;
                            }
                        } else {
                            sign = 1.0;
                        }
                    }
                } else {
                    // Do not across a segment. It is a boundary edge.
                    sign = 1.0;
                }
                if sign >= 0.0 {
                    self.caveshbdlist.push(cavesh);
                }
                cavesh = senext(&cavesh);
            }
            i += 1;
        }

        // Creating new subfaces.
        let mut newsh = Face::default();
        for i in 0..self.caveshbdlist.len() {
            let mut parysh = self.caveshbdlist[i];
            let checkseg = self.sspivot(&parysh);
            if (parysh.shver & 1) != 0 {
                parysh.shver ^= 1;
            }
            let pa = self.sorg(&parysh);
            let pb = self.sdest(&parysh);
            // Create a new subface.
            newsh = self.makeshellface(ShPool::Sub);
            self.setshvertices(&newsh, pa, pb, insertpt);
            let mk = self.shellmark(&parysh);
            self.setshellmark(&newsh, mk);
            let fi = self.shr(&parysh).facetindex;
            self.shr_mut(&newsh).facetindex = fi;
            // Update the point-to-subface map.
            if self.pointtype(pa) == VertType::FreeFacet {
                self.points[pa].point2sh = sencode(&newsh);
            }
            if self.pointtype(pb) == VertType::FreeFacet {
                self.points[pb].point2sh = sencode(&newsh);
            }
            // Connect newsh to outer subfaces.
            let casout = self.spivot(&parysh);
            if !casout.is_null() {
                let mut casin = casout;
                if !checkseg.is_null() {
                    // Make sure that newsh has the right ori at this segment.
                    let mut cs = checkseg;
                    cs.shver = 0;
                    if self.sorg(&newsh) != self.sorg(&cs) {
                        newsh.shver ^= 1;
                        parysh.shver ^= 1; // This side should also be inverse.
                    }
                    let mut neighsh = self.spivot(&casin);
                    while !neighsh.same_sh(&parysh) {
                        casin = neighsh;
                        neighsh = self.spivot(&casin);
                    }
                }
                self.sbond1(&newsh, &casout);
                self.sbond1(&casin, &newsh);
            }
            if !checkseg.is_null() {
                self.ssbond(&newsh, &checkseg);
            }
            // Connect oldsh <== newsh (for connecting adjacent new subfaces).
            self.sbond1(&parysh, &newsh);
            self.caveshbdlist[i] = parysh;
        }

        if !newsh.is_null() {
            // Set a handle for searching.
            self.recentsh = newsh;
        }

        // Update the point-to-subface map.
        if self.pointtype(insertpt) == VertType::FreeFacet {
            self.points[insertpt].point2sh = sencode(&newsh);
        }

        // Connect adjacent new subfaces together.
        for i in 0..self.caveshbdlist.len() {
            // Get an old subface at edge [a, b].
            let parysh = self.caveshbdlist[i];
            let mut newsh = self.spivot(&parysh); // The new subface [a, b, p].
            newsh = senext(&newsh); // At edge [b, p].
            let neighsh0 = self.spivot(&newsh);
            if neighsh0.is_null() {
                // Find the adjacent new subface at edge [b, p].
                let pb = self.sdest(&parysh);
                let mut neighsh = parysh;
                loop {
                    neighsh = senext(&neighsh);
                    neighsh = self.spivot(&neighsh);
                    if neighsh.is_null() {
                        break;
                    }
                    if !self.smarktested(&neighsh) {
                        break;
                    }
                    if self.sdest(&neighsh) != pb {
                        neighsh.shver ^= 1;
                    }
                }
                if !neighsh.is_null() {
                    // Now 'neighsh' is a new subface at edge [b, #].
                    if self.sorg(&neighsh) != pb {
                        neighsh.shver ^= 1;
                    }
                    neighsh = senext2(&neighsh); // Go to the open edge [p, b].
                    self.sbond(&newsh, &neighsh);
                }
            }
            let mut newsh = self.spivot(&parysh); // The new subface [a, b, p].
            newsh = senext2(&newsh); // At edge [p, a].
            let neighsh0 = self.spivot(&newsh);
            if neighsh0.is_null() {
                // Find the adjacent new subface at edge [p, a].
                let pa = self.sorg(&parysh);
                let mut neighsh = parysh;
                loop {
                    neighsh = senext2(&neighsh);
                    neighsh = self.spivot(&neighsh);
                    if neighsh.is_null() {
                        break;
                    }
                    if !self.smarktested(&neighsh) {
                        break;
                    }
                    if self.sorg(&neighsh) != pa {
                        neighsh.shver ^= 1;
                    }
                }
                if !neighsh.is_null() {
                    // Now 'neighsh' is a new subface at edge [#, a].
                    if self.sdest(&neighsh) != pa {
                        neighsh.shver ^= 1;
                    }
                    neighsh = senext(&neighsh); // Go to the open edge [a, p].
                    self.sbond(&newsh, &neighsh);
                }
            }
        }

        if loc == LocateResult::OnEdge || has_seg || !self.cavesegshlist.is_empty() {
            // An edge is being split. In case (2) the edge is on the boundary
            //   of the cavity, and degenerated new faces must be removed.
            let nseg = self.cavesegshlist.len();
            for i in 0..nseg {
                let parysh = self.cavesegshlist[i];
                // Get a possible new degenerated subface.
                let mut cavesh = self.spivot(&parysh);
                if self.sapex(&cavesh) == insertpt {
                    // Found a degenerated new subface, i.e., case (2).
                    let mut newsh = Face::default();
                    if nseg > 1 {
                        // There are more than one subface share at this edge.
                        let j = (i + 1) % nseg;
                        let parysh2 = self.cavesegshlist[j];
                        let mut neighsh = self.spivot(&parysh2);
                        // Adjust cavesh and neighsh both at edge a->b, and has p as apex.
                        if self.sorg(&neighsh) != self.sorg(&cavesh) {
                            neighsh.shver ^= 1;
                        }
                        // Connect adjacent faces at two other edges of cavesh and neighsh.
                        for _ in 0..2 {
                            cavesh = senext(&cavesh);
                            neighsh = senext(&neighsh);
                            newsh = self.spivot(&cavesh);
                            let casout = self.spivot(&neighsh);
                            self.sbond1(&newsh, &casout); // newsh <- casout.
                        }
                    } else {
                        // Only one subface contains this edge: squeeze the
                        //   degenerated new face by disconnecting it.
                        for _ in 0..2 {
                            cavesh = senext(&cavesh);
                            newsh = self.spivot(&cavesh);
                            self.sdissolve(&newsh);
                        }
                    }
                    // Update the point-to-subface map.
                    if self.pointtype(insertpt) == VertType::FreeFacet {
                        self.points[insertpt].point2sh = sencode(&newsh);
                    }
                }
            }

            let mut newsegs = [Face::default(); 2];
            if has_seg {
                let sg = splitseg.as_mut().unwrap();
                if loc != LocateResult::InStar {
                    self.smarktest(sg); // Mark it as being processed.
                }
                self.reach[51] += 1;
                let pa = self.sorg(sg);
                let pb = self.sdest(sg);

                // Insert the new point p.
                let aseg = self.makeshellface(ShPool::Seg);
                let bseg = self.makeshellface(ShPool::Seg);
                self.setshvertices(&aseg, pa, insertpt, NULL_POINT);
                self.setshvertices(&bseg, insertpt, pb, NULL_POINT);
                let mk = self.shellmark(sg);
                self.setshellmark(&aseg, mk);
                self.setshellmark(&bseg, mk);
                // useinsertradius (-p): the segment index.
                let fi = self.shr(sg).facetindex;
                self.shr_mut(&aseg).facetindex = fi;
                self.shr_mut(&bseg).facetindex = fi;

                // Connect [#, a]<->[a, p].
                let boutseg = self.spivot(&senext2(sg)); // Temporarily use boutseg.
                if !boutseg.is_null() {
                    let aoutseg = senext2(&aseg);
                    self.sbond(&boutseg, &aoutseg);
                }
                // Connect [p, b]<->[b, #].
                let aoutseg = self.spivot(&senext(sg));
                if !aoutseg.is_null() {
                    let boutseg = senext(&bseg);
                    self.sbond(&boutseg, &aoutseg);
                }
                // Connect [a, p] <-> [p, b].
                let aoutseg = senext(&aseg);
                let boutseg = senext2(&bseg);
                self.sbond(&aoutseg, &boutseg);

                // Connect subsegs [a, p] and [p, b] to adjacent new subfaces.
                for i in 0..self.cavesegshlist.len() {
                    let parysh = self.cavesegshlist[i];
                    let mut neighsh = self.spivot(&parysh);
                    // neighsh is a degenerated new face.
                    if self.sorg(&neighsh) != pa {
                        neighsh.shver ^= 1;
                    }
                    let newsh = self.spivot(&senext2(&neighsh)); // The edge [p, a] in newsh
                    self.ssbond(&newsh, &aseg);
                    let newsh = self.spivot(&senext(&neighsh)); // The edge [b, p] in newsh
                    self.ssbond(&newsh, &bseg);
                }

                // Let the point remember the segment it lies on.
                if self.pointtype(insertpt) == VertType::FreeSeg {
                    self.points[insertpt].point2sh = sencode(&aseg);
                }
                // Update the point-to-seg map.
                if self.pointtype(pa) == VertType::FreeSeg {
                    self.points[pa].point2sh = sencode(&aseg);
                }
                if self.pointtype(pb) == VertType::FreeSeg {
                    self.points[pb].point2sh = sencode(&bseg);
                }
                newsegs = [aseg, bseg];
            }

            // Delete all degenerated new faces.
            for i in 0..self.cavesegshlist.len() {
                let p = self.spivot(&self.cavesegshlist[i]);
                self.cavesegshlist[i] = p;
                if self.sapex(&p) == insertpt {
                    self.shellfacedealloc(ShPool::Sub, p.sh);
                }
            }
            self.cavesegshlist.clear();

            if has_seg {
                // Return the two new subsegments (for further process).
                self.cavesegshlist.push(newsegs[0]);
                self.cavesegshlist.push(newsegs[1]);
            }
        }

        loc
    }

    /// `sscoutsegment(searchsh, endpt, insertsegflag, reporterrorflag,
    /// chkencflag)`: recover the segment from `sorg(searchsh)` to `endpt`
    /// in the facet's triangulation by `flip22` (upstream's tail recursion
    /// is a loop here).
    pub fn sscoutsegment(
        &mut self,
        searchsh: &mut Face,
        endpt: Point,
        insertsegflag: bool,
        chkencflag: bool,
    ) -> Result<InterResult, TetgenError> {
        #[derive(PartialEq)]
        enum Move {
            Ab,
            Ca,
        }
        loop {
            // The origin of 'searchsh' is fixed.
            let startpt = self.sorg(searchsh);
            let len = self.distance_p(startpt, endpt);
            let dp = *self.xyz(DUMMYPOINT);
            let mut pb;
            let mut pc;
            let dir;

            // Search an edge in 'searchsh' on the path of this segment.
            loop {
                pb = self.sdest(searchsh);
                if pb == endpt {
                    dir = InterResult::ShareEdge; // Found!
                    break;
                }
                pc = self.sapex(searchsh);
                if pc == endpt {
                    searchsh.shver = SNEXTPIVOT[SNEXTPIVOT[searchsh.shver]];
                    searchsh.shver ^= 1;
                    dir = InterResult::ShareEdge; // Found!
                    break;
                }

                // Round the results.
                let ori_ab = if (triarea(self.xyz(startpt), self.xyz(pb), self.xyz(endpt)).sqrt()
                    / len)
                    < self.b.epsilon
                {
                    0.0
                } else {
                    self.pred
                        .orient3d(self.xyz(startpt), self.xyz(pb), &dp, self.xyz(endpt))
                };
                let ori_ca = if (triarea(self.xyz(pc), self.xyz(startpt), self.xyz(endpt)).sqrt()
                    / len)
                    < self.b.epsilon
                {
                    0.0
                } else {
                    self.pred
                        .orient3d(self.xyz(pc), self.xyz(startpt), &dp, self.xyz(endpt))
                };

                let nextmove;
                if ori_ab < 0.0 {
                    if ori_ca < 0.0 {
                        // (--) Both sides are viable moves.
                        if self.randomnation(2) != 0 {
                            nextmove = Move::Ca;
                        } else {
                            nextmove = Move::Ab;
                        }
                    } else {
                        nextmove = Move::Ab; // (-#)
                    }
                } else if ori_ca < 0.0 {
                    nextmove = Move::Ca; // (#-)
                } else if ori_ab > 0.0 {
                    if ori_ca > 0.0 {
                        // (++) The segment intersects with edge [b, c].
                        dir = InterResult::AcrossEdge;
                        break;
                    } else {
                        // (+0) The segment collinear with edge [c, a].
                        searchsh.shver = SNEXTPIVOT[SNEXTPIVOT[searchsh.shver]];
                        searchsh.shver ^= 1;
                        dir = InterResult::AcrossVert;
                        break;
                    }
                } else if ori_ca > 0.0 {
                    // (0+) The segment is collinear with edge [a, b].
                    dir = InterResult::AcrossVert;
                    break;
                } else {
                    // (00) startpt == endpt. Not possible.
                    return Err(TetgenError::InternalError);
                }

                // Move 'searchsh' to the next face, keep the origin unchanged.
                if nextmove == Move::Ab {
                    if chkencflag && self.isshsubseg(searchsh) {
                        return Ok(InterResult::AcrossEdge); // ACROSS_SEG
                    }
                    let mut neighsh = self.spivot(searchsh);
                    if !neighsh.is_null() {
                        if self.sorg(&neighsh) != pb {
                            neighsh.shver ^= 1;
                        }
                        *searchsh = Face {
                            shver: SNEXTPIVOT[neighsh.shver],
                            ..neighsh
                        };
                    } else {
                        // This side (startpt->pb) is outside (rounding error).
                        // Try the next side, i.e., (pc->startpt).
                        let mut neighsh = Face {
                            shver: SNEXTPIVOT[SNEXTPIVOT[searchsh.shver]],
                            ..*searchsh
                        };
                        if chkencflag && self.isshsubseg(&neighsh) {
                            *searchsh = neighsh;
                            return Ok(InterResult::AcrossEdge);
                        }
                        neighsh = self.spivot(&neighsh);
                        if neighsh.is_null() {
                            return Err(TetgenError::InternalError); // NULL sdest.
                        }
                        if self.sdest(&neighsh) != pc {
                            neighsh.shver ^= 1;
                        }
                        *searchsh = neighsh;
                    }
                } else {
                    let mut neighsh = Face {
                        shver: SNEXTPIVOT[SNEXTPIVOT[searchsh.shver]],
                        ..*searchsh
                    };
                    if chkencflag && self.isshsubseg(&neighsh) {
                        *searchsh = neighsh;
                        return Ok(InterResult::AcrossEdge);
                    }
                    neighsh = self.spivot(&neighsh);
                    if !neighsh.is_null() {
                        if self.sdest(&neighsh) != pc {
                            neighsh.shver ^= 1;
                        }
                        *searchsh = neighsh;
                    } else {
                        // The same reason as above. Try (startpt->pb).
                        if chkencflag && self.isshsubseg(searchsh) {
                            return Ok(InterResult::AcrossEdge);
                        }
                        let mut neighsh = self.spivot(searchsh);
                        if neighsh.is_null() {
                            return Err(TetgenError::InternalError); // NULL sorg.
                        }
                        if self.sorg(&neighsh) != pb {
                            neighsh.shver ^= 1;
                        }
                        *searchsh = Face {
                            shver: SNEXTPIVOT[neighsh.shver],
                            ..neighsh
                        };
                    }
                }
            }

            if dir == InterResult::ShareEdge {
                if insertsegflag {
                    // Insert the segment into the triangulation.
                    let newseg = self.makeshellface(ShPool::Seg);
                    self.setshvertices(&newseg, startpt, endpt, NULL_POINT);
                    // Set the default segment marker.
                    self.setshellmark(&newseg, -1);
                    self.ssbond(searchsh, &newseg);
                    let neighsh = self.spivot(searchsh);
                    if !neighsh.is_null() {
                        self.ssbond(&neighsh, &newseg);
                    }
                }
                return Ok(dir);
            }

            if dir == InterResult::AcrossVert {
                // A point is found collinear with this segment.
                return Ok(dir);
            }

            // dir == AcrossEdge: edge [b, c] intersects with the segment.
            let f0 = Face {
                shver: SNEXTPIVOT[searchsh.shver],
                ..*searchsh
            };
            if self.isshsubseg(&f0) {
                return Ok(dir); // ACROSS_SEG
            }
            // Flip edge [b, c], queue unflipped edges (for Delaunay checks).
            let mut f1 = self.spivot(&f0);
            if self.sorg(&f1) != self.sdest(&f0) {
                f1.shver ^= 1;
            }
            let mut flipshs = [f0, f1];
            self.flip22(&mut flipshs, true);
            // The flip may create an inverted triangle, check it.
            let pa = self.sapex(&flipshs[1]);
            let pb = self.sapex(&flipshs[0]);
            let pc = self.sorg(&flipshs[0]);
            let pd = self.sdest(&flipshs[0]);
            // Check if pa and pb are on the different sides of [pc, pd].
            let ori_ab = self.orient3d_p(pc, pd, DUMMYPOINT, pb);
            let ori_ca = self.orient3d_p(pd, pc, DUMMYPOINT, pa);
            if ori_ab <= 0.0 {
                self.flipshpush(&flipshs[0]);
            } else if ori_ca <= 0.0 {
                self.flipshpush(&flipshs[1]);
            }
            // Set 'searchsh' s.t. its origin is 'startpt'.
            *searchsh = flipshs[0];
        }
    }

    /// `scarveholes(holes, holelist)`: remove the triangles outside the
    /// facet and in its holes.
    pub fn scarveholes(&mut self, holelist: &[[f64; 3]]) {
        let senext = |f: &Face| Face {
            shver: SNEXTPIVOT[f.shver],
            ..*f
        };
        // Get all triangles. Infect unprotected convex hull triangles.
        let r = self.recentsh;
        self.smarktest(&r);
        self.caveshlist.push(r);
        let mut i = 0;
        while i < self.caveshlist.len() {
            let mut searchsh = self.caveshlist[i];
            searchsh.shver = 0;
            for _ in 0..3 {
                let neighsh = self.spivot(&searchsh);
                // Is this side on the convex hull?
                if !neighsh.is_null() {
                    if !self.smarktested(&neighsh) {
                        self.smarktest(&neighsh);
                        self.caveshlist.push(neighsh);
                    }
                } else if !self.isshsubseg(&searchsh) && !self.sinfected(&searchsh) {
                    // A hull side, not protected by a segment.
                    self.sinfect(&searchsh);
                    self.caveshbdlist.push(searchsh);
                }
                searchsh = senext(&searchsh);
            }
            i += 1;
        }

        // Infect the triangles in the holes.
        for h in holelist {
            let mut searchsh = self.recentsh;
            let loc = self.slocate(h, NULL_POINT, &mut searchsh, true, true, false);
            if loc != LocateResult::Outside {
                self.sinfect(&searchsh);
                self.caveshbdlist.push(searchsh);
            }
        }

        // Find and infect all exterior triangles.
        let mut i = 0;
        while i < self.caveshbdlist.len() {
            let mut searchsh = self.caveshbdlist[i];
            searchsh.shver = 0;
            for _ in 0..3 {
                let neighsh = self.spivot(&searchsh);
                if !neighsh.is_null() {
                    if !self.isshsubseg(&searchsh) {
                        if !self.sinfected(&neighsh) {
                            self.sinfect(&neighsh);
                            self.caveshbdlist.push(neighsh);
                        }
                    } else {
                        self.sdissolve(&neighsh); // Disconnect a protected face.
                    }
                }
                searchsh = senext(&searchsh);
            }
            i += 1;
        }

        // Delete exterior triangles, unmark interior triangles.
        for i in 0..self.caveshlist.len() {
            let p = self.caveshlist[i];
            if self.sinfected(&p) {
                self.shellfacedealloc(ShPool::Sub, p.sh);
            } else {
                self.sunmarktest(&p);
            }
        }

        self.caveshlist.clear();
        self.caveshbdlist.clear();
    }

    /// `triangulate(shmark, ptlist, conlist, holes, holelist)`: a
    /// constrained Delaunay triangulation of one facet. Returns `false`
    /// where upstream returns 0 (the facet is skipped, with a warning).
    pub fn triangulate(
        &mut self,
        shmark: i32,
        ptlist: &[Point],
        conlist: &[[Point; 2]],
        holelist: &[[f64; 3]],
    ) -> Result<bool, TetgenError> {
        let (pa, pb, pc);
        if ptlist.len() < 2 {
            // Not a segment or a facet.
            return Ok(true);
        } else if ptlist.len() == 2 {
            let pa = ptlist[0];
            let pb = ptlist[1];
            if self.distance_p(pa, pb) > 0.0 {
                // It is a single segment.
                let newseg = self.makeshellface(ShPool::Seg);
                self.setshvertices(&newseg, pa, pb, NULL_POINT);
                self.setshellmark(&newseg, -1);
            }
            if self.pointtype(pa) == VertType::Vol {
                self.setpointtype(pa, VertType::Facet);
            }
            if self.pointtype(pb) == VertType::Vol {
                self.setpointtype(pb, VertType::Facet);
            }
            return Ok(true);
        } else if ptlist.len() == 3 {
            pa = ptlist[0];
            pb = ptlist[1];
            pc = ptlist[2];
        } else {
            // Calculate an above point of this facet.
            match self.calculateabovepoint(ptlist) {
                Some((a, b, c)) => {
                    pa = a;
                    pb = b;
                    pc = c;
                }
                None => {
                    let w =
                        alloc::format!("Warning:  Unable to triangulate facet #{shmark}. Skipped!");
                    self.warnings.push(w);
                    return Ok(false); // The point set is degenerate.
                }
            }
        }

        // Create an initial triangulation.
        let mut newsh = self.makeshellface(ShPool::Sub);
        self.setshvertices(&newsh, pa, pb, pc);
        self.setshellmark(&newsh, shmark);
        self.recentsh = newsh;

        for p in [pa, pb, pc] {
            if self.pointtype(p) == VertType::Vol {
                self.setpointtype(p, VertType::Facet);
            }
        }

        if ptlist.len() == 3 {
            // The triangulation only has one element.
            for _ in 0..3 {
                let newseg = self.makeshellface(ShPool::Seg);
                let (o, d) = (self.sorg(&newsh), self.sdest(&newsh));
                self.setshvertices(&newseg, o, d, NULL_POINT);
                self.setshellmark(&newseg, -1);
                self.ssbond(&newsh, &newseg);
                newsh.shver = SNEXTPIVOT[newsh.shver];
            }
            return Ok(true);
        }

        // Triangulate the facet; 'caveencshlist' / 'caveencseglist' keep the
        //   new subfaces and segments for cleaning up on failure.
        self.caveencshlist.push(newsh);

        // Incrementally build the triangulation.
        self.pinfect(pa);
        self.pinfect(pb);
        self.pinfect(pc);
        let mut i = 0;
        while i < ptlist.len() {
            let ppt = ptlist[i];
            if !self.pinfected(ppt) {
                let mut searchsh = self.recentsh; // Start from 'recentsh'.
                                                  // Insert the vertex. Use Bowyer-Watson algo. Round the location.
                let iloc = self.sinsertvertex(ppt, &mut searchsh, LocateResult::Outside, 1, true);
                if iloc != LocateResult::OnVertex {
                    // Point inserted successfully.
                    if self.pointtype(ppt) == VertType::Vol {
                        self.setpointtype(ppt, VertType::Facet);
                    }
                    // Save the set of new subfaces.
                    for j in 0..self.caveshbdlist.len() {
                        let s = self.spivot(&self.caveshbdlist[j]); // [a, b, p].
                                                                    // Do not save a deleted new face (degenerated).
                        if self.shr(&s).v[0] != NULL_POINT {
                            self.caveencshlist.push(s);
                        }
                    }
                    // Delete all removed subfaces.
                    for j in 0..self.caveshlist.len() {
                        let s = self.caveshlist[j].sh;
                        self.shellfacedealloc(ShPool::Sub, s);
                    }
                    // Clear the global lists.
                    self.caveshbdlist.clear();
                    self.caveshlist.clear();
                    self.cavesegshlist.clear();
                } else {
                    // The facet triangulation is failed.
                    break;
                }
            }
            i += 1;
        }
        self.puninfect(pa);
        self.puninfect(pb);
        self.puninfect(pc);

        if i < ptlist.len() {
            // The facet triangulation is failed. Clean the new subfaces.
            let w = alloc::format!("Warning:  Fail to triangulate facet #{shmark}. Skipped!");
            self.warnings.push(w);
            for k in 0..self.caveencshlist.len() {
                let s = self.caveencshlist[k];
                if self.shr(&s).v[0] != NULL_POINT {
                    self.shellfacedealloc(ShPool::Sub, s.sh);
                }
            }
            self.caveencshlist.clear();
            return Ok(false);
        }

        // Insert the segments.
        let mut i = 0;
        while i < conlist.len() {
            let cons = conlist[i];
            let mut searchsh = self.recentsh;
            let c0 = *self.xyz(cons[0]);
            let iloc = self.slocate(&c0, cons[0], &mut searchsh, true, true, false);
            if iloc != LocateResult::OnVertex {
                // Not found due to roundoff errors. Do a brute-force search.
                let mut bflag = false;
                self.subfacepool.traversalinit();
                let mut cur = self.shellfacetraverse(ShPool::Sub);
                searchsh.pool = ShPool::Sub;
                loop {
                    let Some(s) = cur else {
                        searchsh.sh = NULL_SH;
                        break;
                    };
                    searchsh.sh = s;
                    // Only search the subface in the same facet.
                    if self.shellmark(&searchsh) == shmark {
                        let v = self.subfaces[s].v;
                        if v[0] == cons[0] {
                            searchsh.shver = 0;
                            bflag = true;
                        } else if v[1] == cons[0] {
                            searchsh.shver = 2;
                            bflag = true;
                        } else if v[2] == cons[0] {
                            searchsh.shver = 4;
                            bflag = true;
                        }
                    }
                    if bflag {
                        // [2019-12-03] The subface is not guaranteed to be coplanar.
                        let pa = self.sorg(&searchsh);
                        let pb = self.sdest(&searchsh);
                        let pc = self.sapex(&searchsh);
                        let chkori = self.orient3d_p(pa, pb, pc, cons[1]);
                        if chkori != 0.0 {
                            let mut len = self.distance_p(pa, pb);
                            len += self.distance_p(pb, pc);
                            len += self.distance_p(pc, pa);
                            len /= 3.0;
                            let len3 = len * len * len;
                            let eps = chkori.abs() / len3;
                            if eps < 1e-5 {
                                break; // They are almost coplanar.
                            }
                        } else {
                            break;
                        }
                        bflag = false; // not this subface.
                    }
                    cur = self.shellfacetraverse(ShPool::Sub);
                }
            }
            if !searchsh.is_null() {
                // Recover the segment. Some edges may be flipped.
                if self.sscoutsegment(&mut searchsh, cons[1], true, false)?
                    != InterResult::ShareEdge
                {
                    break; // Fail to recover a segment.
                }
                // Save this newseg.
                let newseg = self.sspivot(&searchsh);
                self.caveencseglist.push(newseg);
                if !self.flipstack.is_empty() {
                    // Recover locally Delaunay edges.
                    self.lawsonflip();
                }
            } else {
                break; // Failed to find a segment.
            }
            i += 1;
        }

        if i < conlist.len() {
            let w =
                alloc::format!("Warning:  Fail to recover a segment in facet #{shmark}. Skipped!");
            self.warnings.push(w);
            for k in 0..self.caveencshlist.len() {
                let s = self.caveencshlist[k];
                if self.shr(&s).v[0] != NULL_POINT {
                    self.shellfacedealloc(ShPool::Sub, s.sh);
                }
            }
            for k in 0..self.caveencseglist.len() {
                let s = self.caveencseglist[k];
                if self.shr(&s).v[0] != NULL_POINT {
                    self.shellfacedealloc(ShPool::Seg, s.sh);
                }
            }
            self.caveencshlist.clear();
            self.caveencseglist.clear();
            return Ok(false);
        }

        // Remove exterior and hole triangles.
        self.scarveholes(holelist);

        self.caveencshlist.clear();
        self.caveencseglist.clear();
        Ok(true)
    }

    /// `unifysegments()`: remove redundant segments and build the face
    /// rings at segments, ordered by the dihedral angle about each segment.
    pub fn unifysegments(&mut self) -> Result<(), TetgenError> {
        if !self.flipstack.is_empty() {
            // Upstream counts the stale flip-stack items as `flippool->items`
            // and walks its `facelink` list past its end (a NULL read).
            return Err(TetgenError::InternalError);
        }
        // Create a mapping from vertices to subfaces.
        let (idx2faclist, facperverlist) = self.makepoint2submap(ShPool::Sub);

        let mut subsegloop = Face {
            pool: ShPool::Seg,
            sh: NULL_SH,
            shver: 0,
        };
        let mut facelink: Vec<Face> = Vec::new();
        self.subsegpool.traversalinit();
        while let Some(s) = self.shellfacetraverse(ShPool::Seg) {
            subsegloop.sh = s;
            let torg = self.sorg(&subsegloop);
            let tdest = self.sdest(&subsegloop);

            let idx = (self.pointmark(torg) - self.firstnumber) as usize;
            // The faces containing the edge (torg, tdest), ordered by the
            //   right-hand rule with the thumb from torg to tdest.
            for k in idx2faclist[idx]..idx2faclist[idx + 1] {
                let mut sface = facperverlist[k];
                // The face may be deleted if it is a duplicated face.
                if self.shr(&sface).v[0] == NULL_POINT {
                    continue;
                }
                // Search the edge torg->tdest.
                if self.sdest(&sface) != tdest {
                    sface.shver = SNEXTPIVOT[SNEXTPIVOT[sface.shver]];
                    sface.shver ^= 1;
                }
                if self.sdest(&sface) != tdest {
                    continue;
                }

                // Save the face f in facelink.
                let items = facelink.len();
                if items >= 2 {
                    let mut f1 = 0usize;
                    for _m in 0..items - 1 {
                        let f2 = f1 + 1;
                        let ori1 = self.facedihedral(
                            torg,
                            tdest,
                            self.sapex(&facelink[f1]),
                            self.sapex(&facelink[f2]),
                        );
                        let ori2 = self.facedihedral(
                            torg,
                            tdest,
                            self.sapex(&facelink[f1]),
                            self.sapex(&sface),
                        );
                        if ori1 >= ori2 {
                            break; // insert this face between f1 and f2.
                        }
                        f1 = f2;
                    }
                    facelink.insert(f1 + 1, sface);
                } else {
                    facelink.push(sface);
                }
            }

            // Set the connection between this segment and faces containing
            //   it; remove redundant segments.
            for k in 0..facelink.len() {
                let f = facelink[k];
                let testseg = self.sspivot(&f);
                if testseg.is_null() {
                    // Upstream reads testseg.sh[3] through NULL.
                    return Err(TetgenError::InternalError);
                }
                // If 'testseg' is not 'subsegloop' and is not dead, it is redundant.
                if !testseg.same_sh(&subsegloop) && self.shr(&testseg).v[0] != NULL_POINT {
                    self.shellfacedealloc(ShPool::Seg, testseg.sh);
                }
                // Bonds the subface and the segment together.
                self.ssbond(&f, &subsegloop);
            }

            // Create the face ring at the segment.
            let items = facelink.len();
            if items > 1 {
                for k in 1..=items {
                    let f1 = facelink[k - 1];
                    let f2 = if k < items { facelink[k] } else { facelink[0] };
                    // Calculate the dihedral angle between the two facet.
                    let n1 = self.facenormal_p(torg, tdest, self.sapex(&f1));
                    let n2 = self.facenormal_p(torg, tdest, self.sapex(&f2));
                    let mut cosang = dot(&n1, &n2) / (dot(&n1, &n1).sqrt() * dot(&n2, &n2).sqrt());
                    // Rounding.
                    if cosang > 1.0 {
                        cosang = 1.0;
                    } else if cosang < -1.0 {
                        cosang = -1.0;
                    }
                    let ang = cosang.acos();
                    // Record the smallest input dihedral angle.
                    if ang < self.minfacetdihed {
                        self.minfacetdihed = ang;
                    }
                    self.sbond1(&f1, &f2);
                }
            }

            facelink.clear(); // flippool->restart()
        }
        Ok(())
    }

    /// `identifyinputedges(idx2verlist)`: make the input edges (`.edge`)
    /// segments, either at an edge of the surface mesh or dangling.
    pub fn identifyinputedges(&mut self, input: &mut PlcInput, idx2verlist: &[Point]) {
        // Construct a map from points to subfaces.
        let (idx2shlist, shperverlist) = self.makepoint2submap(ShPool::Sub);
        let first = self.firstnumber;

        for i in 0..input.edges.len() {
            let mut endpts = input.edges[i];
            if endpts[0] == endpts[1] {
                let w = alloc::format!("Warning:  Edge #{i} is degenerated. Skipped.");
                self.warnings.push(w);
                continue; // Skip a degenerated edge.
            } else if self.dupverts > 0 {
                // Replace duplicated vertices.
                for j in 0..2 {
                    let checkpt = idx2verlist[endpts[j] as usize];
                    if self.pointtype(checkpt) == VertType::Duplicated {
                        let meshpt = self.points[checkpt].point2ppt;
                        endpts[j] = self.pointmark(meshpt);
                    }
                }
                input.edges[i] = endpts;
            }
            // Existing segments have the marker -1; identified ones -2.
            let edgemarker = match &input.edge_markers {
                Some(m) => m[i],
                None => -2,
            };

            // Find a face contains the edge.
            let mut newseg = Face {
                pool: ShPool::Seg,
                sh: NULL_SH,
                shver: 0,
            };
            let mut searchsh = Face::default();
            let idx = (endpts[0] - first) as usize;
            for j in idx2shlist[idx]..idx2shlist[idx + 1] {
                let checkpt = self.sdest(&shperverlist[j]);
                if self.pointmark(checkpt) == endpts[1] {
                    searchsh = shperverlist[j];
                    break; // Found.
                } else {
                    let checkpt = self.sapex(&shperverlist[j]);
                    if self.pointmark(checkpt) == endpts[1] {
                        let f = shperverlist[j];
                        searchsh = Face {
                            shver: SNEXTPIVOT[SNEXTPIVOT[f.shver]] ^ 1,
                            ..f
                        };
                        break;
                    }
                }
            }

            if !searchsh.is_null() {
                // Check if this edge is already a segment of the mesh.
                let checkseg = self.sspivot(&searchsh);
                if !checkseg.is_null() {
                    newseg = checkseg; // This segment already exist.
                } else {
                    // Create a new segment at this edge.
                    let pa = self.sorg(&searchsh);
                    let pb = self.sdest(&searchsh);
                    newseg = self.makeshellface(ShPool::Seg);
                    self.setshvertices(&newseg, pa, pb, NULL_POINT);
                    self.ssbond(&searchsh, &newseg);
                    let neighsh = self.spivot(&searchsh);
                    if !neighsh.is_null() {
                        self.ssbond(&neighsh, &newseg);
                    }
                }
            } else {
                // It is a dangling segment (not belong to any facets).
                let pa = idx2verlist[endpts[0] as usize];
                let pb = idx2verlist[endpts[1] as usize];
                if pa == pb {
                    let w = alloc::format!("Warning:  Edge #{i} is degenerated. Skipped.");
                    self.warnings.push(w);
                    continue;
                }
                // Check if segment [a,b] already exists (brute force).
                self.subsegpool.traversalinit();
                while let Some(s) = self.shellfacetraverse(ShPool::Seg) {
                    let v = self.subsegs[s].v;
                    if (v[0] == pa && v[1] == pb) || (v[0] == pb && v[1] == pa) {
                        newseg.sh = s; // Found!
                        break;
                    }
                }
                if newseg.is_null() {
                    newseg = self.makeshellface(ShPool::Seg);
                    self.setshvertices(&newseg, pa, pb, NULL_POINT);
                }
            }

            self.setshellmark(&newseg, edgemarker);
        }
    }

    /// `mergefacets()`: remove the segments between coplanar facets with the
    /// same marker, and the one of two segments meeting at a small angle,
    /// then restore local Delaunayness by `lawsonflip`.
    pub fn mergefacets(&mut self) {
        let pi = core::f64::consts::PI;
        // First, remove coplanar segments.
        let cosang_tol = (self.b.facet_separate_ang_tol / 180.0 * pi).cos();

        let mut segloop = Face {
            pool: ShPool::Seg,
            sh: NULL_SH,
            shver: 0,
        };
        self.subsegpool.traversalinit();
        while let Some(s) = self.shellfacetraverse(ShPool::Seg) {
            segloop.sh = s;
            // Only remove a segment if it has a marker '-1'.
            if self.shellmark(&segloop) != -1 {
                continue;
            }
            let parentsh = self.spivot(&segloop);
            if parentsh.is_null() {
                continue;
            }
            let neighsh = self.spivot(&parentsh);
            if neighsh.is_null() {
                continue;
            }
            let neineish = self.spivot(&neighsh);
            if !neineish.same_sh(&parentsh) {
                continue;
            }
            // Exactly two subfaces at this segment; merge them only if they
            //   have the same boundary marker.
            if self.shellmark(&parentsh) == self.shellmark(&neighsh) {
                let pa = self.sorg(&segloop);
                let pb = self.sdest(&segloop);
                let pc = self.sapex(&parentsh);
                let pd = self.sapex(&neighsh);
                // Calculate the dihedral angle at the segment [a,b].
                let n1 = self.facenormal_p(pa, pb, pc);
                let n2 = self.facenormal_p(pa, pb, pd);
                let cosang = dot(&n1, &n2) / (dot(&n1, &n1).sqrt() * dot(&n2, &n2).sqrt());
                if cosang < cosang_tol {
                    self.ssdissolve(&parentsh);
                    self.ssdissolve(&neighsh);
                    self.shellfacedealloc(ShPool::Seg, segloop.sh);
                    // Add the edge to flip stack.
                    self.flipshpush(&parentsh);
                } else {
                    // Save 'cosang' in the segment's sh[6].
                    self.subsegs[s].dihedang = Some(cosang);
                }
            }
        }

        // Second, remove ridge segments at small angles.
        let cosang_tol = (self.b.facet_small_ang_tol / 180.0 * pi).cos();
        let cosang_sep_tol = ((self.b.facet_separate_ang_tol - 5.0) / 180.0 * pi).cos();

        // shloop.shver is not reset between subfaces: after a removal the
        //   next subface starts from the version the loop broke at.
        let mut shloop = Face::default();
        self.subfacepool.traversalinit();
        while let Some(s) = self.shellfacetraverse(ShPool::Sub) {
            shloop.sh = s;
            for _ in 0..3 {
                if self.isshsubseg(&shloop) {
                    let neighsh = Face {
                        shver: SNEXTPIVOT[shloop.shver],
                        ..shloop
                    };
                    if self.isshsubseg(&neighsh) {
                        // Found two segments sharing at one vertex.
                        let pa = *self.xyz(self.sorg(&shloop));
                        let pb = *self.xyz(self.sdest(&shloop));
                        let pc = *self.xyz(self.sapex(&shloop));
                        let n1 = [pa[0] - pb[0], pa[1] - pb[1], pa[2] - pb[2]];
                        let n2 = [pc[0] - pb[0], pc[1] - pb[1], pc[2] - pb[2]];
                        let cosang = dot(&n1, &n2) / (dot(&n1, &n1).sqrt() * dot(&n2, &n2).sqrt());
                        if cosang > cosang_tol {
                            // Found a small angle.
                            let mut segloop = Face {
                                pool: ShPool::Seg,
                                sh: NULL_SH,
                                shver: 0,
                            };
                            let seg1 = self.sspivot(&shloop);
                            let seg2 = self.sspivot(&neighsh);
                            let cosang1 = self.shr(&seg1).dihedang.unwrap_or(1.0);
                            let cosang2 = self.shr(&seg2).dihedang.unwrap_or(1.0);
                            if cosang1 < cosang_sep_tol {
                                if cosang2 < cosang_sep_tol {
                                    if cosang1 < cosang2 {
                                        segloop = seg1;
                                    } else {
                                        segloop = seg2;
                                    }
                                } else {
                                    segloop = seg1;
                                }
                            } else if cosang2 < cosang_sep_tol {
                                segloop = seg2;
                            }
                            if !segloop.is_null() {
                                // Remove this segment.
                                segloop.shver = 0;
                                let parentsh = self.spivot(&segloop);
                                let neighsh = self.spivot(&parentsh);
                                self.ssdissolve(&parentsh);
                                self.ssdissolve(&neighsh);
                                self.shellfacedealloc(ShPool::Seg, segloop.sh);
                                // Add the edge to flip stack.
                                self.flipshpush(&parentsh);
                                break;
                            }
                        }
                    }
                }
                shloop.shver = SNEXTPIVOT[shloop.shver];
            }
        }

        if !self.flipstack.is_empty() {
            self.lawsonflip(); // Recover Delaunayness.
        }
    }

    /// `meshsurface()`: triangulate every facet, unify the segments, insert
    /// the input edges, merge coplanar facets and mark the ridge vertices.
    /// `input` is upstream's `in`, which `meshsurface` edits (duplicate
    /// vertex indices are replaced in the polygons and edges).
    pub fn meshsurface(&mut self, input: &mut PlcInput) -> Result<(), TetgenError> {
        // Create a map from indices to points.
        let idx2verlist = self.makeindex2pointmap();
        let first = input.firstnumber;
        let npts = input.points.len() as i32;

        let mut ptlist: Vec<Point> = Vec::new();
        let mut conlist: Vec<[Point; 2]> = Vec::new();

        // Loop the facet list, triangulate each facet.
        for shmark in 1..=input.facets.len() {
            // Process the duplicated points first: if p and q are duplicated
            //   and p's index > q's, p is substituted by q.
            if self.dupverts > 0 {
                let f = &mut input.facets[shmark - 1];
                for p in f.polygons.iter_mut() {
                    for v in p.iter_mut() {
                        // Upstream indexes idx2verlist unchecked; an index out
                        //   of range is skipped later with a warning.
                        let Some(&tstart) = idx2verlist.get(*v as usize) else {
                            continue;
                        };
                        if tstart != NULL_POINT && self.pointtype(tstart) == VertType::Duplicated {
                            let tend = self.points[tstart].point2ppt;
                            *v = self.pointmark(tend);
                        }
                    }
                }
            }

            // Loop polygons of F, get the set of vertices and segments.
            let npoly = input.facets[shmark - 1].polygons.len();
            for i in 0..npoly {
                let p = input.facets[shmark - 1].polygons[i].clone();
                let nv = p.len();
                if nv == 0 {
                    // load_poly refuses an empty polygon; skip one built in code.
                    continue;
                }
                // Get the first vertex.
                let mut end1 = p[0];
                if end1 < first || end1 >= first + npts {
                    let w = alloc::format!(
                        "Warning:  Invalid the 1st vertex {end1} of polygon {} in facet {shmark}.",
                        i + 1
                    );
                    self.warnings.push(w);
                    continue; // Skip this polygon.
                }
                let mut tstart = idx2verlist[end1 as usize];
                // Add tstart to V if it haven't been added yet.
                if !self.pinfected(tstart) {
                    self.pinfect(tstart);
                    ptlist.push(tstart);
                }
                // Loop other vertices of this polygon.
                for j in 1..=nv {
                    let end2 = if j < nv { p[j] } else { p[0] };
                    if end2 < first || end2 >= first + npts {
                        let w = alloc::format!(
                            "Warning:  Invalid vertex {end2} in polygon {} in facet {shmark}.",
                            i + 1
                        );
                        self.warnings.push(w);
                    } else if end1 != end2 {
                        // 'end1' and 'end2' form a segment.
                        let tend = idx2verlist[end2 as usize];
                        if !self.pinfected(tend) {
                            self.pinfect(tend);
                            ptlist.push(tend);
                        }
                        // Save the segment in S (conlist).
                        conlist.push([tstart, tend]);
                        // Set the start for next continuous segment.
                        end1 = end2;
                        tstart = tend;
                    } else if nv > 2 {
                        // Two identical vertices mean an isolated vertex of F.
                        let w = alloc::format!(
                            "Warning:  Polygon {} has two identical verts in facet {shmark}.",
                            i + 1
                        );
                        self.warnings.push(w);
                    }
                    // Is the polygon degenerate (a segment or a vertex)?
                    if nv == 2 {
                        break;
                    }
                }
            }
            // Unmark vertices.
            for &p in &ptlist {
                self.puninfect(p);
            }

            // Triangulate F into a CDT (default marker -1).
            let mark = match &input.facet_markers {
                Some(m) => m[shmark - 1],
                None => -1,
            };
            let holes = input.facets[shmark - 1].holes.clone();
            self.triangulate(mark, &ptlist, &conlist, &holes)?;

            // Clear working lists.
            ptlist.clear();
            conlist.clear();
        }

        // Remove redundant segments and build the face links.
        self.unifysegments()?;
        if !input.edges.is_empty() {
            // There are input segments. Insert them.
            self.identifyinputedges(input, &idx2verlist);
        }
        if !self.b.diagnose && !self.b.nomergefacet && self.b.nobisect == 0 {
            // Merge coplanar facets.
            self.mergefacets();
        }

        // Mark all segment vertices to be RIDGEVERTEX.
        self.subsegpool.traversalinit();
        while let Some(s) = self.shellfacetraverse(ShPool::Seg) {
            let v = self.subsegs[s].v;
            self.setpointtype(v[0], VertType::Ridge);
            self.setpointtype(v[1], VertType::Ridge);
        }

        // The total number of input segments.
        self.insegments = self.subsegpool.items as i64;
        Ok(())
    }

    // -- the differential dump ------------------------------------------------

    /// The state after `meshsurface` in the text format of the gh:#1792
    /// instrumented upstream build (`dump_meshsurface`, see
    /// `verification_and_validation/tetgen_meshsurface_gh1792/`): every
    /// point, subface, subsegment and tetrahedron slot in pool order with
    /// its links, the dead-item stacks, `recentsh`, `recenttet`, the
    /// `randomnation` seed, `insegments`, `minfacetdihed` and the next
    /// glibc `rand()` value (drawn from a copy).
    pub fn dump_meshsurface(&self) -> String {
        let mut out = String::new();
        let pref = |p: Point| -> String {
            if p == NULL_POINT {
                "-".into()
            } else if p == DUMMYPOINT {
                "D".into()
            } else {
                alloc::format!("{}", p - 1)
            }
        };
        let shref = |e: usize| -> String {
            if e == 0 {
                return "-".into();
            }
            let f = sdecode(e);
            let c = match f.pool {
                ShPool::Sub => 'f',
                ShPool::Seg => 's',
            };
            alloc::format!("{c}{}.{}", f.sh, f.shver)
        };
        let tref = |h: usize| -> String {
            if h == NULL_HANDLE {
                return "-".into();
            }
            let t = decode(h);
            alloc::format!("t{}.{}", t.tet, t.ver)
        };
        let _ = writeln!(out, "meshsurface dump v1");
        let _ = writeln!(
            out,
            "firstnumber {} numberofpoints {} dupverts {}",
            self.firstnumber, self.numberofpoints, self.dupverts
        );
        let _ = writeln!(
            out,
            "points {} {}",
            self.pointpool.maxitems, self.pointpool.items
        );
        for k in 0..self.pointpool.maxitems {
            let r = &self.points[k + 1];
            let ppt = if r.vtype == VertType::Duplicated {
                pref(r.point2ppt)
            } else {
                "-".into()
            };
            let _ = writeln!(
                out,
                "p {} {} {} {} {:016x} {:016x} {:016x} {}",
                k,
                r.mark,
                r.vtype as i32,
                r.flags & 255,
                r.xyz[0].to_bits(),
                r.xyz[1].to_bits(),
                r.xyz[2].to_bits(),
                ppt
            );
        }
        let d = self.points[DUMMYPOINT].xyz;
        let _ = writeln!(
            out,
            "dummy {:016x} {:016x} {:016x}",
            d[0].to_bits(),
            d[1].to_bits(),
            d[2].to_bits()
        );
        for (name, c, pool, recs) in [
            ("subfaces", 'f', &self.subfacepool, &self.subfaces),
            ("subsegs", 's', &self.subsegpool, &self.subsegs),
        ] {
            let _ = writeln!(out, "{name} {} {}", pool.maxitems, pool.items);
            for (k, r) in recs.iter().enumerate().take(pool.maxitems) {
                if r.v[0] == NULL_POINT {
                    let _ = writeln!(out, "{c} {k} dead");
                    continue;
                }
                if c == 'f' {
                    let _ = writeln!(
                        out,
                        "f {k} {} {} {} {} {} {} {} {} {} {} {} {} {}",
                        pref(r.v[0]),
                        pref(r.v[1]),
                        pref(r.v[2]),
                        r.marker,
                        r.flags,
                        shref(r.nbr[0]),
                        shref(r.nbr[1]),
                        shref(r.nbr[2]),
                        shref(r.seg[0]),
                        shref(r.seg[1]),
                        shref(r.seg[2]),
                        tref(r.tet[0]),
                        tref(r.tet[1])
                    );
                } else {
                    let _ = writeln!(
                        out,
                        "s {k} {} {} {} {} {} {} {} {} {} {} {}",
                        pref(r.v[0]),
                        pref(r.v[1]),
                        pref(r.v[2]),
                        r.marker,
                        r.flags,
                        shref(r.nbr[0]),
                        shref(r.nbr[1]),
                        shref(r.nbr[2]),
                        if r.seg[0] != 0 || r.dihedang.is_some() {
                            "x"
                        } else {
                            "-"
                        },
                        tref(r.tet[0]),
                        tref(r.tet[1])
                    );
                }
            }
            let _ = write!(out, "{c}dead");
            for s in pool.deaditemstack.iter().rev() {
                let _ = write!(out, " {s}");
            }
            let _ = writeln!(out);
        }
        let _ = writeln!(
            out,
            "tets {} {} {}",
            self.tetpool.maxitems, self.tetpool.items, self.hullsize
        );
        for (k, t) in self.tets.iter().enumerate().take(self.tetpool.maxitems) {
            if t.v[0] == NULL_POINT {
                let _ = writeln!(out, "t {k} dead");
                continue;
            }
            let _ = writeln!(
                out,
                "t {k} {} {} {} {} {} {} {} {} {} {} {}",
                pref(t.v[0]),
                pref(t.v[1]),
                pref(t.v[2]),
                pref(t.v[3]),
                tref(t.nbr[0]),
                tref(t.nbr[1]),
                tref(t.nbr[2]),
                tref(t.nbr[3]),
                t.marker,
                if t.seg != usize::MAX { "x" } else { "-" },
                if t.sub != usize::MAX { "x" } else { "-" }
            );
        }
        let _ = write!(out, "tdead");
        for s in self.tetpool.deaditemstack.iter().rev() {
            let _ = write!(out, " {s}");
        }
        let _ = writeln!(out);
        let rs = if self.recentsh.is_null() {
            "-".into()
        } else {
            shref(sencode(&self.recentsh))
        };
        let _ = writeln!(out, "recentsh {rs}");
        let rt = if self.recenttet.tet == super::mesh::NULL_TET {
            "-".into()
        } else {
            tref(encode(&self.recenttet))
        };
        let _ = writeln!(out, "recenttet {rt}");
        let _ = writeln!(out, "randomseed {}", self.randomseed);
        let _ = writeln!(out, "insegments {}", self.insegments);
        let _ = writeln!(out, "minfacetdihed {:016x}", self.minfacetdihed.to_bits());
        let _ = writeln!(out, "flippool {}", self.flipstack.len());
        let mut cr = self.crand.clone();
        let _ = writeln!(out, "rand {}", cr.rand());
        out
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// `lu_decmp` / `lu_solve` solve a small system, and `circumcircle`
    /// gives the circumcentre of a right triangle (the hypotenuse midpoint).
    #[test]
    fn circumcircle_of_a_right_triangle() {
        let (c, r) = circumcircle(&[0.0, 0.0, 0.0], &[2.0, 0.0, 0.0], &[0.0, 2.0, 0.0]).unwrap();
        assert_eq!(c, [1.0, 1.0, 0.0]);
        assert_eq!(r, 2.0f64.sqrt());
        assert!(circumcircle(&[0.0; 3], &[1.0, 0.0, 0.0], &[2.0, 0.0, 0.0]).is_none());
        assert_eq!(triarea(&[0.0; 3], &[2.0, 0.0, 0.0], &[0.0, 2.0, 0.0]), 2.0);
        assert_eq!(
            facenormal(&[0.0; 3], &[1.0, 0.0, 0.0], &[0.0, 1.0, 0.0], 0),
            [0.0, 0.0, 1.0]
        );
    }

    /// `slocate` without an above point (`aflag` 0: `calculateabovepoint4`)
    /// finds a subface's centroid on that face, and, without the convexity
    /// flag (`cflag` 0), stops at a segment for a point beyond the facet.
    #[test]
    fn slocate_computes_its_own_above_point() {
        let pts = [
            [0.0, 0.0, 0.0],
            [1.0, 0.0, 0.0],
            [1.0, 1.0, 0.0],
            [0.0, 1.0, 0.0],
            [0.0, 0.0, 1.0],
            [1.0, 0.0, 1.0],
            [1.0, 1.0, 1.0],
            [0.0, 1.0, 1.0],
        ];
        let tris = [
            [0, 3, 2],
            [0, 2, 1],
            [4, 5, 6],
            [4, 6, 7],
            [0, 1, 5],
            [0, 5, 4],
            [1, 2, 6],
            [1, 6, 5],
            [2, 3, 7],
            [2, 7, 6],
            [3, 0, 4],
            [3, 4, 7],
        ];
        let marks = [1, 1, 2, 2, 3, 3, 4, 4, 5, 5, 6, 6];
        let plc = PlcInput::from_triangle_soup(&pts, &tris, &marks);
        let mut m = plc_meshsurface(&plc).unwrap().mesh;
        assert!(m.warnings.is_empty());
        let f = m.recentsh;
        let (a, b, c) = (m.sorg(&f), m.sdest(&f), m.sapex(&f));
        let (pa, pb, pc) = (*m.xyz(a), *m.xyz(b), *m.xyz(c));
        let cen = [
            (pa[0] + pb[0] + pc[0]) / 3.0,
            (pa[1] + pb[1] + pc[1]) / 3.0,
            (pa[2] + pb[2] + pc[2]) / 3.0,
        ];
        let mut s = f;
        let loc = m.slocate(&cen, NULL_POINT, &mut s, false, false, true);
        assert_eq!(loc, LocateResult::OnFace);
        assert!(s.same_sh(&f));
        // In the facet's plane, far beyond it: a segment stops the walk.
        let far = [
            pa[0] + 5.0 * (pb[0] - pa[0]) + 5.0 * (pc[0] - pa[0]),
            pa[1] + 5.0 * (pb[1] - pa[1]) + 5.0 * (pc[1] - pa[1]),
            pa[2] + 5.0 * (pb[2] - pa[2]) + 5.0 * (pc[2] - pa[2]),
        ];
        let mut s = f;
        let loc = m.slocate(&far, NULL_POINT, &mut s, false, false, false);
        assert_eq!(loc, LocateResult::EncSegment);
        // Every subface is a box face half; every segment a box edge.
        assert_eq!(m.subfacepool.items, 12);
        assert_eq!(m.subsegpool.items, 12);
        assert_eq!(m.insegments, 12);
    }

    /// The shell-face encoding round-trips and the pool reuses slots LIFO,
    /// writing the dead-item link into `sh[0]`.
    #[test]
    fn shell_faces_encode_and_reuse_slots() {
        let mut m = TetgenMesh::new(Behavior::default());
        let a = m.makeshellface(ShPool::Sub);
        let b = m.makeshellface(ShPool::Sub);
        let s = m.makeshellface(ShPool::Seg);
        for f in [a, b, s] {
            for v in 0..6 {
                let g = Face { shver: v, ..f };
                assert_eq!(sdecode(sencode(&g)), g);
            }
        }
        assert_eq!(sencode(&Face::default()), 0);
        m.shellfacedealloc(ShPool::Sub, a.sh);
        m.shellfacedealloc(ShPool::Sub, b.sh);
        assert_eq!(sdecode(m.subfaces[b.sh].nbr[0]).sh, a.sh);
        let c = m.makeshellface(ShPool::Sub);
        assert_eq!(c.sh, b.sh);
        // A record with NULL vertices reads as dead (upstream's `sh[3]`).
        m.subfacepool.traversalinit();
        assert_eq!(m.shellfacetraverse(ShPool::Sub), None);
        m.setshvertices(&c, 1, 2, 3);
        m.subfacepool.traversalinit();
        assert_eq!(m.shellfacetraverse(ShPool::Sub), Some(b.sh));
        assert_eq!(m.shellfacetraverse(ShPool::Sub), None);
    }
}
