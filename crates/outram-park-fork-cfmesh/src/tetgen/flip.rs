// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright (C) 2026 OUTRAM PARK contributors
//
// Port of TetGen 1.6.0 (`tetgen.h`: the tetrahedron-subface and
// tetrahedron-segment primitives `tsbond`, `tspivot`, `stpivot`,
// `tsdissolve`, `stdissolve`, `ssbond1`, `tssbond1`, `sstbond1`,
// `tssdissolve1`, `sstdissolve1`, `tsspivot1`, `sstpivot1`, `issubface`,
// `issubseg`, `point2tetorg`, the element counter and face/edge marks;
// `tetgen.cxx`: `tri_edge_2d`, `tri_edge_tail`, `tri_edge_test`,
// `planelineint`, `interiorangle`, section `flip_cxx`: `flippush`, `flip23`,
// `flip32`, `flip41`, `flipnm`, `flipnm_post`, and from `steiner_cxx`:
// `sort_2pts`, `sort_3pts`, `is_collinear_at`, `is_segment`,
// `valid_constrained_f23`, `valid_constrained_f32`,
// `checkflipeligibility`, `removeedgebyflips`, `removefacebyflips`):
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

//! The 3D flips and the tetrahedron-subface/segment links (stage 4,
//! gh:#1793).
//!
//! `flip23`, `flip32` and `flip41` keep the subsegment and subface links of
//! the tetrahedra they rebuild when `checksubsegflag` / `checksubfaceflag`
//! are set (boundary recovery sets them); `flipnm` removes an edge through a
//! sequence of them, recursing into reflex link edges up to the flip link
//! level. Everything follows upstream line for line, including the pool
//! allocation order of the tetrahedron, `tet2segpool` and `tet2subpool`
//! records.
//!
//! **`abtets`.** Upstream's `flipnm` stores, in the entry after the
//! shrinking star, either the flipped vertex `c` (cast to a `tetrahedron *`)
//! or a pointer to the star array of a flipped link edge, with flags packed
//! in `ver`. Here an entry is an [`AbTet`]: a handle, a 2-to-3 record or an
//! n-to-m record owning its star. The bits are the same information.
//!
//! **Branches not ported** (not reached by boundary recovery, which never
//! sets them): ~~`flipconstraints::remove_large_angle` (`get_tetqual`, mesh
//! improvement, stage 6),~~ `chkencflag` queues (`enqueuesubface`,
//! ~~`enqueuetetrahedron`,~~ Delaunay refinement), ~~element attributes
//! and~~ volume bounds (~~`-A`,~~ `-a`). `remove_ndelaunay_edge` (the
//! `tetprismvol` sum) and `unflip` are ported: `recoverdelaunay` (stage 5)
//! uses the first. **UPDATED 2026-11-26 (gh:#1794):** stage 5's mesh
//! improvement adds `remove_large_angle` (in `checkflipeligibility`, with
//! `get_tetqual`), the `chkencflag & 4` queue (`enqueuetetrahedron`, in
//! `flip23`, `flip32`, `flip41`) and the element attribute copies of `-A`
//! (`flip23`, `flip32`).

#[cfg(not(any(feature = "std", test)))]
#[allow(unused_imports)]
use crate::nostd_prelude::*;

use super::delaunay::TetgenError;
use super::mesh::{
    decode, encode, encode2, enext, enextesym, enextself, eprev, eprevesym, eprevesymself,
    eprevself, esym, esymself, sdecode, sencode, Face, Point, ShPool, ShellFace, TetgenMesh,
    TriFace, VertType, DUMMYPOINT, EDESTOPPOTBL, EORGOPPOTBL, NULL_HANDLE, NULL_POINT, NULL_TET,
    SNEXTPIVOT, STBONDTBL, STPIVOTTBL, TSBONDTBL, TSPIVOTTBL, VER2EDGE,
};
use alloc::vec::Vec;

// ---------------------------------------------------------------------------
// enum interresult (as the integers `types[]` carries).
// ---------------------------------------------------------------------------

/// `DISJOINT`.
pub const DISJOINT: i32 = 0;
/// `INTERSECT`.
pub const INTERSECT: i32 = 1;
/// `SHAREVERT`.
pub const SHAREVERT: i32 = 2;
/// `SHAREEDGE`.
pub const SHAREEDGE: i32 = 3;
/// `SHAREFACE`.
pub const SHAREFACE: i32 = 4;
/// `TOUCHEDGE`.
pub const TOUCHEDGE: i32 = 5;
/// `TOUCHFACE`.
pub const TOUCHFACE: i32 = 6;
/// `ACROSSVERT`.
pub const ACROSSVERT: i32 = 7;
/// `ACROSSEDGE`.
pub const ACROSSEDGE: i32 = 8;
/// `ACROSSFACE`.
pub const ACROSSFACE: i32 = 9;
/// `SELF_INTERSECT`.
pub const SELF_INTERSECT: i32 = 10;

// ---------------------------------------------------------------------------
// Handle helpers.
// ---------------------------------------------------------------------------

/// `eorgoppo(t1, t2)`.
#[inline]
pub fn eorgoppo(t: &TriFace) -> TriFace {
    TriFace {
        tet: t.tet,
        ver: EORGOPPOTBL[t.ver],
    }
}
/// `edestoppo(t1, t2)`.
#[inline]
pub fn edestoppo(t: &TriFace) -> TriFace {
    TriFace {
        tet: t.tet,
        ver: EDESTOPPOTBL[t.ver],
    }
}
/// `sesym(s1, s2)`.
#[inline]
pub fn sesym(s: &Face) -> Face {
    Face {
        shver: s.shver ^ 1,
        ..*s
    }
}
/// `senext(s1, s2)`.
#[inline]
pub fn senext(s: &Face) -> Face {
    Face {
        shver: SNEXTPIVOT[s.shver],
        ..*s
    }
}
/// `senext2(s1, s2)`.
#[inline]
pub fn senext2(s: &Face) -> Face {
    Face {
        shver: SNEXTPIVOT[SNEXTPIVOT[s.shver]],
        ..*s
    }
}
/// `sencode2(sh, shver)`.
#[inline]
fn sencode2(s: &Face, shver: usize) -> ShellFace {
    sencode(&Face { shver, ..*s })
}

#[inline]
fn sub3(a: &[f64; 3], b: &[f64; 3]) -> [f64; 3] {
    [a[0] - b[0], a[1] - b[1], a[2] - b[2]]
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

/// `tetprismvol(p0, p1, p2, p3)`: the volume of the tet-prism (the lifted
/// tetrahedron and its projection), the quantity `recoverdelaunay`
/// minimises.
pub fn tetprismvol(
    pred: &super::predicates::Predicates,
    p0: &[f64; 3],
    p1: &[f64; 3],
    p2: &[f64; 3],
    p3: &[f64; 3],
) -> f64 {
    let (p4, p5, p6, p7) = (p0, p1, p2, p3);
    // TO DO: these weights can be pre-calculated!
    let w4 = dot(p0, p0);
    let w5 = dot(p1, p1);
    let w6 = dot(p2, p2);
    let w7 = dot(p3, p3);
    // Calculate the volume of the tet-prism.
    let vol0 = pred.orient4d(p5, p6, p4, p3, p7, w5, w6, w4, 0.0, w7);
    let vol1 = pred.orient4d(p3, p6, p2, p0, p1, 0.0, w6, 0.0, 0.0, 0.0);
    let vol2 = pred.orient4d(p4, p6, p3, p0, p1, w4, w6, 0.0, 0.0, 0.0);
    let vol3 = pred.orient4d(p6, p5, p4, p3, p1, w6, w5, w4, 0.0, 0.0);
    vol0.abs() + vol1.abs() + vol2.abs() + vol3.abs()
}

/// `flipconstraints`.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct FlipConstraints {
    /// `enqflag` (= flipflag).
    pub enqflag: i32,
    /// `chkencflag` (always 0 in boundary recovery; the queues are not
    /// ported).
    pub chkencflag: i32,
    /// `unflip`: undo the performed flips.
    pub unflip: bool,
    /// `collectnewtets`: collect the new tets in `cavetetlist`.
    pub collectnewtets: bool,
    /// `collectencsegflag`: queue the segments met in `caveencseglist`.
    pub collectencsegflag: bool,
    /// `noflip_in_surface`.
    pub noflip_in_surface: bool,
    /// `remove_ndelaunay_edge`.
    pub remove_ndelaunay_edge: bool,
    /// `bak_tetprism_vol`.
    pub bak_tetprism_vol: f64,
    /// `tetprism_vol_sum`.
    pub tetprism_vol_sum: f64,
    /// `checkflipeligibility`.
    pub checkflipeligibility: bool,
    /// `seg[2]`: a constraining edge (`seg[0] == NULL_POINT`: none).
    pub seg: [Point; 2],
    /// `fac[3]`: a constraining face (`fac[0] == NULL_POINT`: none).
    pub fac: [Point; 3],
    /// `remvert`: a vertex to be removed (`NULL_POINT`: none).
    pub remvert: Point,
    /// `remove_large_angle` (mesh improvement, stage 5).
    pub remove_large_angle: bool,
    /// `cosdihed_in`: the cosine of the dihedral angle to improve.
    pub cosdihed_in: f64,
    /// `cosdihed_out`: the worst new cosine.
    pub cosdihed_out: f64,
    /// `max_asp_out`: the worst new aspect ratio.
    pub max_asp_out: f64,
}

impl Default for FlipConstraints {
    fn default() -> Self {
        FlipConstraints {
            enqflag: 0,
            chkencflag: 0,
            unflip: false,
            collectnewtets: false,
            collectencsegflag: false,
            noflip_in_surface: false,
            remove_ndelaunay_edge: false,
            bak_tetprism_vol: 0.0,
            tetprism_vol_sum: 0.0,
            checkflipeligibility: false,
            seg: [NULL_POINT; 2],
            fac: [NULL_POINT; 3],
            remvert: NULL_POINT,
            remove_large_angle: false,
            cosdihed_in: 0.0,
            cosdihed_out: 0.0,
            max_asp_out: 0.0,
        }
    }
}

/// An entry of `flipnm`'s `abtets` array. Upstream overloads a `triface`:
/// a handle, or (in the entry just past the current star) the record of
/// the flip that shrank the star, packed into `tet` and `ver`.
#[derive(Debug, Clone, PartialEq)]
pub enum AbTet {
    /// A tetrahedron handle.
    Tet(TriFace),
    /// A 2-to-3 flip of the face `[a,b,c]` at position `pos`
    /// (`abtets[n-1].tet = c`, `ver = (1 << 4) | (pos << 6)`).
    Flip23 {
        /// The flipped link vertex `c`.
        pc: Point,
        /// The position `i` of the flip.
        pos: usize,
    },
    /// A flipped link edge (`abtets[n-1].tet = tmpabtets`, `ver =
    /// edgepivot | (1 << 5) | (pos << 6) | (n1 << 19)`).
    FlipNm {
        /// The star of the flipped link edge (`tmpabtets`).
        star: Vec<AbTet>,
        /// `edgepivot` (1 or 2).
        edgepivot: usize,
        /// The position `i` of the flip.
        pos: usize,
        /// The size of the star.
        n1: usize,
    },
    /// The flipped link vertex `c` kept in `tmpabtets[0]` (`ver = 1 << 5`).
    Vertex(Point),
}

impl AbTet {
    /// The handle (upstream reads the entry as a `triface`).
    #[inline]
    pub fn t(&self) -> TriFace {
        match self {
            AbTet::Tet(t) => *t,
            _ => TriFace::default(),
        }
    }
}

impl TetgenMesh {
    // -- predicates on points ----------------------------------------------

    #[inline]
    pub(crate) fn ori3(&self, a: Point, b: Point, c: Point, d: Point) -> f64 {
        self.pred
            .orient3d(self.xyz(a), self.xyz(b), self.xyz(c), self.xyz(d))
    }

    // -- tet-subface / tet-segment primitives (tetgen.h) -------------------

    /// `tsbond(t, s)`: bond a tetrahedron and a subface (same face, same
    /// edge, same orientation). Allocates the tet's subface array from
    /// `tet2subpool` on first use.
    pub fn tsbond(&mut self, t: &TriFace, s: &Face) {
        if self.tets[t.tet].sub == usize::MAX {
            let (slot, fresh) = self.tet2subpool.alloc();
            if fresh {
                self.tet2sub.push([0; 4]);
            }
            self.tet2sub[slot] = [0; 4];
            self.tets[t.tet].sub = slot;
        }
        let a = self.tets[t.tet].sub;
        self.tet2sub[a][t.ver & 3] = sencode2(s, TSBONDTBL[t.ver][s.shver]);
        self.shr_mut(s).tet[s.shver & 1] = encode2(t.tet, STBONDTBL[t.ver][s.shver]);
    }

    /// `tspivot(t, s)`: the subface at `t` (`sh == NULL` if none).
    pub fn tspivot(&self, t: &TriFace) -> Face {
        let a = self.tets[t.tet].sub;
        if a == usize::MAX {
            return Face::default();
        }
        let mut s = sdecode(self.tet2sub[a][t.ver & 3]);
        s.shver = TSPIVOTTBL[t.ver][s.shver];
        s
    }

    /// `issubface(t)`.
    #[inline]
    pub fn issubface(&self, t: &TriFace) -> bool {
        let a = self.tets[t.tet].sub;
        a != usize::MAX && self.tet2sub[a][t.ver & 3] != 0
    }

    /// `stpivot(s, t)`: the tetrahedron at `s` (same edge and orientation).
    pub fn stpivot(&self, s: &Face) -> TriFace {
        let mut t = decode(self.shr(s).tet[s.shver & 1]);
        if t.tet == NULL_TET {
            return t;
        }
        t.ver = STPIVOTTBL[t.ver][s.shver];
        t
    }

    /// `tsdissolve(t)`.
    pub fn tsdissolve(&mut self, t: &TriFace) {
        let a = self.tets[t.tet].sub;
        if a != usize::MAX {
            self.tet2sub[a][t.ver & 3] = 0;
        }
    }

    /// `stdissolve(s)`.
    pub fn stdissolve(&mut self, s: &Face) {
        self.shr_mut(s).tet = [NULL_HANDLE; 2];
    }

    /// `ssbond1(s, edge)`: `s` points to the segment, not back.
    pub fn ssbond1(&mut self, s: &Face, edge: &Face) {
        self.shr_mut(s).seg[s.shver >> 1] = sencode(edge);
    }

    /// `tssbond1(t, s)`: the tet's edge points to the segment. Allocates
    /// the tet's segment array from `tet2segpool` on first use.
    pub fn tssbond1(&mut self, t: &TriFace, s: &Face) {
        if self.tets[t.tet].seg == usize::MAX {
            let (slot, fresh) = self.tet2segpool.alloc();
            if fresh {
                self.tet2seg.push([0; 6]);
            }
            self.tet2seg[slot] = [0; 6];
            self.tets[t.tet].seg = slot;
        }
        let a = self.tets[t.tet].seg;
        self.tet2seg[a][VER2EDGE[t.ver]] = sencode(s);
    }

    /// `sstbond1(s, t)`.
    pub fn sstbond1(&mut self, s: &Face, t: &TriFace) {
        self.shr_mut(s).tet[0] = encode(t);
    }

    /// `tssdissolve1(t)`.
    pub fn tssdissolve1(&mut self, t: &TriFace) {
        let a = self.tets[t.tet].seg;
        if a != usize::MAX {
            self.tet2seg[a][VER2EDGE[t.ver]] = 0;
        }
    }

    /// `sstdissolve1(s)`.
    pub fn sstdissolve1(&mut self, s: &Face) {
        self.shr_mut(s).tet[0] = NULL_HANDLE;
    }

    /// `tsspivot1(t, s)`: the segment at the edge of `t`.
    pub fn tsspivot1(&self, t: &TriFace) -> Face {
        let a = self.tets[t.tet].seg;
        if a != usize::MAX {
            sdecode(self.tet2seg[a][VER2EDGE[t.ver]])
        } else {
            Face::default()
        }
    }

    /// `issubseg(t)`.
    #[inline]
    pub fn issubseg(&self, t: &TriFace) -> bool {
        let a = self.tets[t.tet].seg;
        a != usize::MAX && self.tet2seg[a][VER2EDGE[t.ver]] != 0
    }

    /// `sstpivot1(s, t)`.
    #[inline]
    pub fn sstpivot1(&self, s: &Face) -> TriFace {
        decode(self.shr(s).tet[0])
    }

    /// `point2tetorg(pa, searchtet)`: a tetrahedron whose origin is `pa`.
    pub fn point2tetorg(&self, pa: Point) -> TriFace {
        let mut t = decode(self.point2tet(pa));
        let v = self.tets[t.tet].v;
        t.ver = if v[0] == pa {
            11
        } else if v[1] == pa {
            3
        } else if v[2] == pa {
            7
        } else {
            0
        };
        t
    }

    /// `setpoint2sh(p, s)`.
    #[inline]
    pub fn setpoint2sh(&mut self, p: Point, s: ShellFace) {
        self.points[p].point2sh = s;
    }

    /// `point2sh(p)` (`0` for `NULL`).
    #[inline]
    pub fn point2sh(&self, p: Point) -> ShellFace {
        let s = self.points[p].point2sh;
        if s == usize::MAX {
            0
        } else {
            s
        }
    }

    /// `issteinerpoint(p)`.
    #[inline]
    pub fn issteinerpoint(&self, p: Point) -> bool {
        matches!(
            self.pointtype(p),
            VertType::FreeSeg | VertType::FreeFacet | VertType::FreeVol
        )
    }

    /// `getfacetindex(s)`.
    #[inline]
    pub fn getfacetindex(&self, s: &Face) -> i32 {
        self.shr(s).facetindex
    }

    /// `setfacetindex(s, value)`.
    #[inline]
    pub fn setfacetindex(&mut self, s: &Face, value: i32) {
        self.shr_mut(s).facetindex = value;
    }

    /// `smarktest2(s)`.
    #[inline]
    pub fn smarktest2(&mut self, s: &Face) {
        self.shr_mut(s).flags |= 4;
    }
    /// `smarktest2ed(s)`.
    #[inline]
    pub fn smarktest2ed(&self, s: &Face) -> bool {
        (self.shr(s).flags & 4) != 0
    }
    /// `smarktest3(s)`.
    #[inline]
    pub fn smarktest3(&mut self, s: &Face) {
        self.shr_mut(s).flags |= 8;
    }
    /// `smarktest3ed(s)`.
    #[inline]
    pub fn smarktest3ed(&self, s: &Face) -> bool {
        (self.shr(s).flags & 8) != 0
    }
    /// `suninfect(s)`.
    #[inline]
    pub fn suninfect(&mut self, s: &Face) {
        self.shr_mut(s).flags &= !1;
    }
    /// `pmarktest(p)`.
    #[inline]
    pub fn pmarktest(&mut self, p: Point) {
        self.points[p].flags |= 2;
    }
    /// `punmarktest(p)`.
    #[inline]
    pub fn punmarktest(&mut self, p: Point) {
        self.points[p].flags &= !2;
    }
    /// `pmarktested(p)`.
    #[inline]
    pub fn pmarktested(&self, p: Point) -> bool {
        (self.points[p].flags & 2) != 0
    }
    /// `pmarktest2(p)`.
    #[inline]
    pub fn pmarktest2(&mut self, p: Point) {
        self.points[p].flags |= 4;
    }
    /// `punmarktest2(p)`.
    #[inline]
    pub fn punmarktest2(&mut self, p: Point) {
        self.points[p].flags &= !4;
    }
    /// `pmarktest2ed(p)`.
    #[inline]
    pub fn pmarktest2ed(&self, p: Point) -> bool {
        (self.points[p].flags & 4) != 0
    }
    /// `pmarktest3(p)`.
    #[inline]
    pub fn pmarktest3(&mut self, p: Point) {
        self.points[p].flags |= 8;
    }
    /// `punmarktest3(p)`.
    #[inline]
    pub fn punmarktest3(&mut self, p: Point) {
        self.points[p].flags &= !8;
    }
    /// `pmarktest3ed(p)`.
    #[inline]
    pub fn pmarktest3ed(&self, p: Point) -> bool {
        (self.points[p].flags & 8) != 0
    }

    // -- tetrahedron flags ---------------------------------------------------

    /// `setelemmarker(t, 0)`: clear all flags and the counter.
    #[inline]
    pub fn clearelemmarker(&mut self, tet: usize) {
        self.tets[tet].marker = 0;
    }
    /// `elemcounter(t)`.
    #[inline]
    pub fn elemcounter(&self, tet: usize) -> i32 {
        self.tets[tet].marker >> 16
    }
    /// `setelemcounter(t, value)`.
    #[inline]
    pub fn setelemcounter(&mut self, tet: usize, value: i32) {
        let mut c = self.tets[tet].marker;
        c &= 65535;
        c |= value << 16;
        self.tets[tet].marker = c;
    }
    /// `increaseelemcounter(t)`.
    #[inline]
    pub fn increaseelemcounter(&mut self, tet: usize) {
        let c = self.elemcounter(tet);
        self.setelemcounter(tet, c + 1);
    }
    /// `decreaseelemcounter(t)`.
    #[inline]
    pub fn decreaseelemcounter(&mut self, tet: usize) {
        let c = self.elemcounter(tet);
        self.setelemcounter(tet, c - 1);
    }
    /// `markface(t)`.
    #[inline]
    pub fn markface(&mut self, t: &TriFace) {
        self.tets[t.tet].marker |= 4 << (t.ver & 3);
    }
    /// `unmarkface(t)`.
    #[inline]
    pub fn unmarkface(&mut self, t: &TriFace) {
        self.tets[t.tet].marker &= !(4 << (t.ver & 3));
    }
    /// `facemarked(t)`.
    #[inline]
    pub fn facemarked(&self, t: &TriFace) -> bool {
        (self.tets[t.tet].marker & (4 << (t.ver & 3))) != 0
    }
    /// `markedge(t)`.
    #[inline]
    pub fn markedge(&mut self, t: &TriFace) {
        self.tets[t.tet].marker |= 64 << VER2EDGE[t.ver];
    }
    /// `unmarkedge(t)`.
    #[inline]
    pub fn unmarkedge(&mut self, t: &TriFace) {
        self.tets[t.tet].marker &= !(64 << VER2EDGE[t.ver]);
    }
    /// `edgemarked(t)`.
    #[inline]
    pub fn edgemarked(&self, t: &TriFace) -> bool {
        (self.tets[t.tet].marker & (64 << VER2EDGE[t.ver])) != 0
    }
    /// `marktest2(t)`.
    #[inline]
    pub fn marktest2(&mut self, tet: usize) {
        self.tets[tet].marker |= 4096;
    }
    /// `unmarktest2(t)`.
    #[inline]
    pub fn unmarktest2(&mut self, tet: usize) {
        self.tets[tet].marker &= !4096;
    }
    /// `marktest2ed(t)`.
    #[inline]
    pub fn marktest2ed(&self, tet: usize) -> bool {
        (self.tets[tet].marker & 4096) != 0
    }

    /// `setvertices(t, a, b, c, d)`.
    #[inline]
    pub fn setvertices(&mut self, t: &TriFace, a: Point, b: Point, c: Point, d: Point) {
        use super::mesh::{APEXPIVOT, DESTPIVOT, OPPOPIVOT, ORGPIVOT};
        let r = &mut self.tets[t.tet];
        r.v[ORGPIVOT[t.ver] - 4] = a;
        r.v[DESTPIVOT[t.ver] - 4] = b;
        r.v[APEXPIVOT[t.ver] - 4] = c;
        r.v[OPPOPIVOT[t.ver] - 4] = d;
    }

    /// `dealloc` of a tet's segment array (`[8] = NULL`).
    fn free_tet2seg(&mut self, tet: usize) {
        if self.tets[tet].seg != usize::MAX {
            self.tet2segpool.dealloc(self.tets[tet].seg);
            self.tets[tet].seg = usize::MAX;
        }
    }

    /// `dealloc` of a tet's subface array (`[9] = NULL`).
    fn free_tet2sub(&mut self, tet: usize) {
        if self.tets[tet].sub != usize::MAX {
            self.tet2subpool.dealloc(self.tets[tet].sub);
            self.tets[tet].sub = usize::MAX;
        }
    }

    // -- geometry -----------------------------------------------------------

    /// `interiorangle(o, p1, p2, n)`: the angle at `o` (`0..PI` without a
    /// normal; `0..2 PI` with one).
    pub fn interiorangle(
        &self,
        o: &[f64; 3],
        p1: &[f64; 3],
        p2: &[f64; 3],
        n: Option<&[f64; 3]>,
    ) -> f64 {
        let v1 = sub3(p1, o);
        let v2 = sub3(p2, o);
        let len1 = dot(&v1, &v1).sqrt();
        let len2 = dot(&v2, &v2).sqrt();
        let lenlen = len1 * len2;
        let mut costheta = dot(&v1, &v2) / lenlen;
        if costheta > 1.0 {
            costheta = 1.0; // Roundoff.
        } else if costheta < -1.0 {
            costheta = -1.0; // Roundoff.
        }
        let mut theta = costheta.acos();
        if let Some(n) = n {
            // Get a point above the face (o, p1, p2);
            let np = [o[0] + n[0], o[1] + n[1], o[2] + n[2]];
            // Adjust theta (0 - 2 * PI).
            let ori = self.pred.orient3d(p1, o, &np, p2);
            if ori > 0.0 {
                theta = 2.0 * core::f64::consts::PI - theta;
            }
        }
        theta
    }

    /// `planelineint(pa, pb, pc, e1, e2, ip, u)`: the intersection of the
    /// line `e1 e2` with the plane `pa pb pc`, `ip = e1 + u (e2 - e1)`.
    pub fn planelineint(
        &self,
        pa: &[f64; 3],
        pb: &[f64; 3],
        pc: &[f64; 3],
        e1: &[f64; 3],
        e2: &[f64; 3],
    ) -> ([f64; 3], f64) {
        let u_ = e1;
        let v_ = e2;
        let vuv = [v_[0] - u_[0], v_[1] - u_[1], v_[2] - u_[2]];
        let a = [pa[0], pb[0], pc[0]];
        let a3 = -vuv[0];
        let b = [pa[1], pb[1], pc[1]];
        let b3 = -vuv[1];
        let c = [pa[2], pb[2], pc[2]];
        let c3 = -vuv[2];
        let d = [1.0, 1.0, 1.0];
        let d3 = 0.0;
        let o = [0.0, 0.0, 0.0];
        let o3 = 0.0;
        let det = self
            .pred
            .orient4dexact(&a, &b, &c, &d, &o, a3, b3, c3, d3, o3);
        if det != 0.0 {
            let det1 = self.pred.orient3dexact(pa, pb, pc, u_);
            let u = det1 / det;
            (
                [u_[0] + u * vuv[0], u_[1] + u * vuv[1], u_[2] + u * vuv[2]],
                u,
            )
        } else {
            ([0.0; 3], 0.0)
        }
    }

    /// `tri_edge_2d(A, B, C, P, Q, R, level, types, pos)`: a coplanar
    /// triangle and edge. `r == None` is upstream's `R == NULL` (a lift
    /// point is computed). `types` and `pos` are the caller's arrays:
    /// entries upstream leaves unwritten keep their values.
    #[allow(clippy::too_many_arguments)]
    pub fn tri_edge_2d(
        &mut self,
        a: &[f64; 3],
        b: &[f64; 3],
        c: &[f64; 3],
        p: &[f64; 3],
        q: &[f64; 3],
        r: Option<[f64; 3]>,
        level: i32,
        types: &mut [i32; 2],
        pos: &mut [i32; 4],
    ) -> i32 {
        self.reach[14] += 1;
        let r: [f64; 3] = match r {
            Some(r) => r,
            None => {
                // Calculate a lift point.
                let mut n = super::surface::facenormal(a, b, c, 1);
                let mut len = dot(&n, &n).sqrt();
                if len != 0.0 {
                    n[0] /= len;
                    n[1] /= len;
                    n[2] /= len;
                    len = distance(a, b);
                    len += distance(b, c);
                    len += distance(c, a);
                    len /= 3.0;
                    [a[0] + len * n[0], a[1] + len * n[1], a[2] + len * n[2]]
                } else {
                    // The triangle [A,B,C] is (nearly) degenerate.
                    return 0; // DISJOINT
                }
            }
        };
        let o =
            |x: &[f64; 3], y: &[f64; 3], z: &[f64; 3], w: &[f64; 3]| self.pred.orient3d(x, y, z, w);
        // Test A's, B's, and C's orientations wrt plane PQR.
        let s_a = o(p, q, &r, a);
        let s_b = o(p, q, &r, b);
        let s_c = o(p, q, &r, c);
        // U, V as indices: 0 = A, 1 = B, 2 = C; 0 = P, 1 = Q, 2 = R.
        let (uu, vv, z1): ([usize; 3], [usize; 3], i32);
        const I3: [usize; 3] = [0, 1, 2];
        const PT: [usize; 3] = [2, 0, 1];
        const PT2: [usize; 3] = [1, 2, 0];
        const I2: [usize; 3] = [0, 1, 2];
        const PL: [usize; 3] = [1, 0, 2];
        if s_a < 0.0 {
            if s_b < 0.0 {
                if s_c < 0.0 {
                    return 0; // (---)
                } else if s_c > 0.0 {
                    (uu, vv, z1) = (I3, I2, 0); // (--+)
                } else {
                    (uu, vv, z1) = (I3, I2, 1); // (--0)
                }
            } else if s_b > 0.0 {
                if s_c < 0.0 {
                    (uu, vv, z1) = (PT, I2, 0); // (-+-)
                } else if s_c > 0.0 {
                    (uu, vv, z1) = (PT2, PL, 0); // (-++)
                } else {
                    (uu, vv, z1) = (PT, I2, 2); // (-+0)
                }
            } else if s_c < 0.0 {
                (uu, vv, z1) = (PT, I2, 1); // (-0-)
            } else if s_c > 0.0 {
                (uu, vv, z1) = (PT2, PL, 2); // (-0+)
            } else {
                (uu, vv, z1) = (PT2, PL, 3); // (-00)
            }
        } else if s_a > 0.0 {
            if s_b < 0.0 {
                if s_c < 0.0 {
                    (uu, vv, z1) = (PT2, I2, 0); // (+--)
                } else if s_c > 0.0 {
                    (uu, vv, z1) = (PT, PL, 0); // (+-+)
                } else {
                    (uu, vv, z1) = (PT, PL, 2); // (+-0)
                }
            } else if s_b > 0.0 {
                if s_c < 0.0 {
                    (uu, vv, z1) = (I3, PL, 0); // (++-)
                } else if s_c > 0.0 {
                    return 0; // (+++)
                } else {
                    (uu, vv, z1) = (I3, PL, 1); // (++0)
                }
            } else if s_c < 0.0 {
                (uu, vv, z1) = (PT2, I2, 2); // (+0-)
            } else if s_c > 0.0 {
                (uu, vv, z1) = (PT, PL, 1); // (+0+)
            } else {
                (uu, vv, z1) = (PT2, I2, 3); // (+00)
            }
        } else if s_b < 0.0 {
            if s_c < 0.0 {
                (uu, vv, z1) = (PT2, I2, 1); // (0--)
            } else if s_c > 0.0 {
                (uu, vv, z1) = (I3, I2, 2); // (0-+)
            } else {
                (uu, vv, z1) = (PT, PL, 3); // (0-0)
            }
        } else if s_b > 0.0 {
            if s_c < 0.0 {
                (uu, vv, z1) = (I3, PL, 2); // (0+-)
            } else if s_c > 0.0 {
                (uu, vv, z1) = (PT2, PL, 1); // (0++)
            } else {
                (uu, vv, z1) = (PT, I2, 3); // (0+0)
            }
        } else if s_c < 0.0 {
            (uu, vv, z1) = (I3, PL, 3); // (00-)
        } else if s_c > 0.0 {
            (uu, vv, z1) = (I3, I2, 3); // (00+)
        } else {
            // (000): not possible unless ABC is degenerate.
            (uu, vv, z1) = (I3, I2, 4);
        }
        let abc = [a, b, c];
        let pqr = [p, q, &r];
        let u = [abc[uu[0]], abc[uu[1]], abc[uu[2]]];
        let v = [pqr[vv[0]], pqr[vv[1]], pqr[vv[2]]];
        let pu = [uu[0] as i32, uu[1] as i32, uu[2] as i32];
        let pv = [vv[0] as i32, vv[1] as i32, vv[2] as i32];

        let s1 = o(u[0], u[2], &r, v[1]); // A, C, R, Q
        let s2 = o(u[1], u[2], &r, v[0]); // B, C, R, P

        if s1 > 0.0 {
            return 0;
        }
        if s2 < 0.0 {
            return 0;
        }
        if level == 0 {
            return 1; // They are intersected.
        }

        if z1 == 1 {
            if s1 == 0.0 {
                // C = Q.
                types[0] = SHAREVERT;
                pos[0] = pu[2];
                pos[1] = pv[1];
                types[1] = DISJOINT;
            } else if s2 == 0.0 {
                // C = P.
                types[0] = SHAREVERT;
                pos[0] = pu[2];
                pos[1] = pv[0];
                types[1] = DISJOINT;
            } else {
                // C in [P, Q].
                types[0] = ACROSSVERT;
                pos[0] = pu[2];
                pos[1] = pv[0];
                types[1] = DISJOINT;
            }
            return 4;
        }

        let s3 = o(u[0], u[2], &r, v[0]); // A, C, R, P
        let s4 = o(u[1], u[2], &r, v[1]); // B, C, R, Q

        if z1 == 0 {
            // (tritri-03)
            if s1 < 0.0 {
                if s3 > 0.0 {
                    if s4 > 0.0 {
                        types[0] = ACROSSEDGE;
                        pos[0] = pu[2];
                        pos[1] = pv[0];
                        types[1] = TOUCHFACE;
                        pos[2] = 3;
                        pos[3] = pv[1];
                    } else if s4 == 0.0 {
                        types[0] = ACROSSEDGE;
                        pos[0] = pu[2];
                        pos[1] = pv[0];
                        types[1] = TOUCHEDGE;
                        pos[2] = pu[1];
                        pos[3] = pv[1];
                    } else {
                        types[0] = ACROSSEDGE;
                        pos[0] = pu[2];
                        pos[1] = pv[0];
                        types[1] = ACROSSEDGE;
                        pos[2] = pu[1];
                        pos[3] = pv[0];
                    }
                } else if s3 == 0.0 {
                    if s4 > 0.0 {
                        types[0] = TOUCHEDGE;
                        pos[0] = pu[2];
                        pos[1] = pv[0];
                        types[1] = TOUCHFACE;
                        pos[2] = 3;
                        pos[3] = pv[1];
                    } else if s4 == 0.0 {
                        types[0] = TOUCHEDGE;
                        pos[0] = pu[2];
                        pos[1] = pv[0];
                        types[1] = TOUCHEDGE;
                        pos[2] = pu[1];
                        pos[3] = pv[1];
                    } else {
                        types[0] = TOUCHEDGE;
                        pos[0] = pu[2];
                        pos[1] = pv[0];
                        types[1] = ACROSSEDGE;
                        pos[2] = pu[1];
                        pos[3] = pv[0];
                    }
                } else if s2 > 0.0 {
                    if s4 > 0.0 {
                        types[0] = TOUCHFACE;
                        pos[0] = 3;
                        pos[1] = pv[0];
                        types[1] = TOUCHFACE;
                        pos[2] = 3;
                        pos[3] = pv[1];
                    } else if s4 == 0.0 {
                        types[0] = TOUCHFACE;
                        pos[0] = 3;
                        pos[1] = pv[0];
                        types[1] = TOUCHEDGE;
                        pos[2] = pu[1];
                        pos[3] = pv[1];
                    } else {
                        types[0] = TOUCHFACE;
                        pos[0] = 3;
                        pos[1] = pv[0];
                        types[1] = ACROSSEDGE;
                        pos[2] = pu[1];
                        pos[3] = pv[0];
                    }
                } else {
                    // P = l (#0##).
                    types[0] = TOUCHEDGE;
                    pos[0] = pu[1];
                    pos[1] = pv[0];
                    types[1] = DISJOINT;
                }
            } else {
                // Q = k (0####)
                types[0] = TOUCHEDGE;
                pos[0] = pu[2];
                pos[1] = pv[1];
                types[1] = DISJOINT;
            }
        } else if z1 == 2 {
            // (tritri-23)
            if s1 < 0.0 {
                if s3 > 0.0 {
                    if s4 > 0.0 {
                        types[0] = ACROSSVERT;
                        pos[0] = pu[0];
                        pos[1] = pv[0];
                        types[1] = TOUCHFACE;
                        pos[2] = 3;
                        pos[3] = pv[1];
                    } else if s4 == 0.0 {
                        types[0] = ACROSSVERT;
                        pos[0] = pu[0];
                        pos[1] = pv[0];
                        types[1] = TOUCHEDGE;
                        pos[2] = pu[1];
                        pos[3] = pv[1];
                    } else {
                        types[0] = ACROSSVERT;
                        pos[0] = pu[0];
                        pos[1] = pv[0];
                        types[1] = ACROSSEDGE;
                        pos[2] = pu[1];
                        pos[3] = pv[0];
                    }
                } else if s3 == 0.0 {
                    if s4 > 0.0 {
                        types[0] = SHAREVERT;
                        pos[0] = pu[0];
                        pos[1] = pv[0];
                        types[1] = TOUCHFACE;
                        pos[2] = 3;
                        pos[3] = pv[1];
                    } else if s4 == 0.0 {
                        types[0] = SHAREVERT;
                        pos[0] = pu[0];
                        pos[1] = pv[0];
                        types[1] = TOUCHEDGE;
                        pos[2] = pu[1];
                        pos[3] = pv[1];
                    } else {
                        types[0] = SHAREVERT;
                        pos[0] = pu[0];
                        pos[1] = pv[0];
                        types[1] = ACROSSEDGE;
                        pos[2] = pu[1];
                        pos[3] = pv[0];
                    }
                } else if s2 > 0.0 {
                    // Upstream writes types[0]/pos[0..1] twice here (and
                    // leaves types[1] as it was); ported as written.
                    if s4 > 0.0 {
                        types[0] = TOUCHFACE;
                        pos[0] = 3;
                        pos[1] = pv[0];
                        types[0] = TOUCHFACE;
                        pos[0] = 3;
                        pos[1] = pv[1];
                    } else if s4 == 0.0 {
                        types[0] = TOUCHFACE;
                        pos[0] = 3;
                        pos[1] = pv[0];
                        types[0] = TOUCHEDGE;
                        pos[0] = pu[1];
                        pos[1] = pv[1];
                    } else {
                        types[0] = TOUCHFACE;
                        pos[0] = 3;
                        pos[1] = pv[0];
                        types[0] = ACROSSEDGE;
                        pos[0] = pu[1];
                        pos[1] = pv[0];
                    }
                } else {
                    // P = l (#0##).
                    types[0] = TOUCHEDGE;
                    pos[0] = pu[1];
                    pos[1] = pv[0];
                    types[1] = DISJOINT;
                }
            } else {
                // Q = A (0###).
                types[0] = SHAREVERT;
                pos[0] = pu[0];
                pos[1] = pv[1];
                types[1] = DISJOINT;
            }
        } else if z1 == 3 {
            // (tritri-33)
            if s1 < 0.0 {
                if s3 > 0.0 {
                    if s4 > 0.0 {
                        types[0] = ACROSSVERT;
                        pos[0] = pu[0];
                        pos[1] = pv[0];
                        types[1] = TOUCHEDGE;
                        pos[2] = pu[0];
                        pos[3] = pv[1];
                    } else if s4 == 0.0 {
                        types[0] = ACROSSVERT;
                        pos[0] = pu[0];
                        pos[1] = pv[0];
                        types[1] = SHAREVERT;
                        pos[2] = pu[1];
                        pos[3] = pv[1];
                    } else {
                        types[0] = ACROSSVERT;
                        pos[0] = pu[0];
                        pos[1] = pv[0];
                        types[1] = ACROSSVERT;
                        pos[2] = pu[1];
                        pos[3] = pv[0];
                    }
                } else if s3 == 0.0 {
                    if s4 > 0.0 {
                        types[0] = SHAREVERT;
                        pos[0] = pu[0];
                        pos[1] = pv[0];
                        types[1] = TOUCHEDGE;
                        pos[2] = pu[0];
                        pos[3] = pv[1];
                    } else if s4 == 0.0 {
                        types[0] = SHAREEDGE;
                        pos[0] = pu[0];
                        pos[1] = pv[0];
                        types[1] = DISJOINT;
                    } else {
                        types[0] = SHAREVERT;
                        pos[0] = pu[0];
                        pos[1] = pv[0];
                        types[1] = ACROSSVERT;
                        pos[2] = pu[1];
                        pos[3] = pv[0];
                    }
                } else if s2 > 0.0 {
                    if s4 > 0.0 {
                        types[0] = TOUCHEDGE;
                        pos[0] = pu[0];
                        pos[1] = pv[0];
                        types[1] = TOUCHEDGE;
                        pos[2] = pu[0];
                        pos[3] = pv[1];
                    } else if s4 == 0.0 {
                        types[0] = TOUCHEDGE;
                        pos[0] = pu[0];
                        pos[1] = pv[0];
                        types[1] = SHAREVERT;
                        pos[2] = pu[1];
                        pos[3] = pv[1];
                    } else {
                        types[0] = TOUCHEDGE;
                        pos[0] = pu[0];
                        pos[1] = pv[0];
                        types[1] = ACROSSVERT;
                        pos[2] = pu[1];
                        pos[3] = pv[0];
                    }
                } else {
                    // P = B (#0##).
                    types[0] = SHAREVERT;
                    pos[0] = pu[1];
                    pos[1] = pv[0];
                    types[1] = DISJOINT;
                }
            } else {
                // Q = A (0###).
                types[0] = SHAREVERT;
                pos[0] = pu[0];
                pos[1] = pv[1];
                types[1] = DISJOINT;
            }
        }
        4
    }

    /// `tri_edge_tail(A, B, C, P, Q, R, sP, sQ, level, types, pos)`.
    #[allow(clippy::too_many_arguments)]
    pub fn tri_edge_tail(
        &mut self,
        a: &[f64; 3],
        b: &[f64; 3],
        c: &[f64; 3],
        p: &[f64; 3],
        q: &[f64; 3],
        r: Option<[f64; 3]>,
        s_p: f64,
        s_q: f64,
        level: i32,
        types: &mut [i32; 2],
        pos: &mut [i32; 4],
    ) -> i32 {
        self.reach[13] += 1;
        let (u, v, pu, pv, z1): ([&[f64; 3]; 3], [&[f64; 3]; 2], [i32; 3], [i32; 2], i32);
        if s_p < 0.0 {
            if s_q < 0.0 {
                return 0; // (--) disjoint
            } else if s_q > 0.0 {
                (u, v, pu, pv, z1) = ([a, b, c], [p, q], [0, 1, 2], [0, 1], 0); // (-+)
            } else {
                (u, v, pu, pv, z1) = ([a, b, c], [p, q], [0, 1, 2], [0, 1], 1); // (-0)
            }
        } else if s_p > 0.0 {
            if s_q < 0.0 {
                (u, v, pu, pv, z1) = ([a, b, c], [q, p], [0, 1, 2], [1, 0], 0); // (+-)
            } else if s_q > 0.0 {
                return 0; // (++) disjoint
            } else {
                (u, v, pu, pv, z1) = ([b, a, c], [p, q], [1, 0, 2], [0, 1], 1); // (+0)
            }
        } else if s_q < 0.0 {
            (u, v, pu, pv, z1) = ([a, b, c], [q, p], [0, 1, 2], [1, 0], 1); // (0-)
        } else if s_q > 0.0 {
            (u, v, pu, pv, z1) = ([b, a, c], [q, p], [1, 0, 2], [1, 0], 1); // (0+)
        } else {
            // (00): A, B, C, P, and Q are coplanar.
            return self.tri_edge_2d(a, b, c, p, q, r, level, types, pos);
        }

        let s1 = self.pred.orient3d(u[0], u[1], v[0], v[1]);
        if s1 < 0.0 {
            return 0;
        }
        let s2 = self.pred.orient3d(u[1], u[2], v[0], v[1]);
        if s2 < 0.0 {
            return 0;
        }
        let s3 = self.pred.orient3d(u[2], u[0], v[0], v[1]);
        if s3 < 0.0 {
            return 0;
        }
        if level == 0 {
            return 1; // The are intersected.
        }

        types[1] = DISJOINT; // No second intersection point.

        if z1 == 0 {
            if s1 > 0.0 {
                if s2 > 0.0 {
                    if s3 > 0.0 {
                        // [P, Q] passes interior of [A, B, C].
                        types[0] = ACROSSFACE;
                        pos[0] = 3;
                        pos[1] = 0;
                    } else {
                        // [P, Q] intersects [C, A].
                        types[0] = ACROSSEDGE;
                        pos[0] = pu[2];
                        pos[1] = 0;
                    }
                } else if s3 > 0.0 {
                    // [P, Q] intersects [B, C].
                    types[0] = ACROSSEDGE;
                    pos[0] = pu[1];
                    pos[1] = 0;
                } else {
                    // [P, Q] passes C.
                    types[0] = ACROSSVERT;
                    pos[0] = pu[2];
                    pos[1] = 0;
                }
            } else if s2 > 0.0 {
                if s3 > 0.0 {
                    // [P, Q] intersects [A, B].
                    types[0] = ACROSSEDGE;
                    pos[0] = pu[0];
                    pos[1] = 0;
                } else {
                    // [P, Q] passes A.
                    types[0] = ACROSSVERT;
                    pos[0] = pu[0];
                    pos[1] = 0;
                }
            } else if s3 > 0.0 {
                // [P, Q] passes B.
                types[0] = ACROSSVERT;
                pos[0] = pu[1];
                pos[1] = 0;
            }
        } else if s1 > 0.0 {
            // z1 == 1
            if s2 > 0.0 {
                if s3 > 0.0 {
                    // Q lies in [A, B, C].
                    types[0] = TOUCHFACE;
                    pos[0] = 0;
                    pos[1] = pv[1];
                } else {
                    // Q lies on [C, A].
                    types[0] = TOUCHEDGE;
                    pos[0] = pu[2];
                    pos[1] = pv[1];
                }
            } else if s3 > 0.0 {
                // Q lies on [B, C].
                types[0] = TOUCHEDGE;
                pos[0] = pu[1];
                pos[1] = pv[1];
            } else {
                // Q = C.
                types[0] = SHAREVERT;
                pos[0] = pu[2];
                pos[1] = pv[1];
            }
        } else if s2 > 0.0 {
            if s3 > 0.0 {
                // Q lies on [A, B].
                types[0] = TOUCHEDGE;
                pos[0] = pu[0];
                pos[1] = pv[1];
            } else {
                // Q = A.
                types[0] = SHAREVERT;
                pos[0] = pu[0];
                pos[1] = pv[1];
            }
        } else if s3 > 0.0 {
            // Q = B.
            types[0] = SHAREVERT;
            pos[0] = pu[1];
            pos[1] = pv[1];
        }
        // T and E intersect in a single point.
        2
    }

    /// `tri_edge_test(A, B, C, P, Q, R, level, types, pos)`.
    #[allow(clippy::too_many_arguments)]
    pub fn tri_edge_test(
        &mut self,
        a: Point,
        b: Point,
        c: Point,
        p: Point,
        q: Point,
        r: Option<Point>,
        level: i32,
        types: &mut [i32; 2],
        pos: &mut [i32; 4],
    ) -> i32 {
        let (a, b, c, p, q) = (
            *self.xyz(a),
            *self.xyz(b),
            *self.xyz(c),
            *self.xyz(p),
            *self.xyz(q),
        );
        let r = r.map(|r| *self.xyz(r));
        // Test the locations of P and Q with respect to ABC.
        let s_p = self.pred.orient3d(&a, &b, &c, &p);
        let s_q = self.pred.orient3d(&a, &b, &c, &q);
        self.tri_edge_tail(&a, &b, &c, &p, &q, r, s_p, s_q, level, types, pos)
    }

    // -- steiner_cxx helpers -------------------------------------------------

    /// `sort_2pts(p1, p2, ppt)`: by point mark.
    pub fn sort_2pts(&self, p1: Point, p2: Point) -> [Point; 2] {
        if self.pointmark(p1) < self.pointmark(p2) {
            [p1, p2]
        } else {
            [p2, p1]
        }
    }

    /// `sort_3pts(p1, p2, p3, ppt)`: by point mark.
    pub fn sort_3pts(&self, p1: Point, p2: Point, p3: Point) -> [Point; 3] {
        let i1 = self.pointmark(p1);
        let i2 = self.pointmark(p2);
        let i3 = self.pointmark(p3);
        if i1 < i2 {
            if i1 < i3 {
                if i2 < i3 {
                    [p1, p2, p3]
                } else {
                    [p1, p3, p2]
                }
            } else {
                [p3, p1, p2]
            }
        } else if i2 < i3 {
            if i1 < i3 {
                [p2, p1, p3]
            } else {
                [p2, p3, p1]
            }
        } else {
            [p3, p2, p1]
        }
    }

    /// `is_collinear_at(mid, left, right)`: the angle at `mid` is above
    /// `collinear_ang_tol`.
    pub fn is_collinear_at(&mut self, mid: Point, left: Point, right: Point) -> bool {
        self.reach[18] += 1;
        let (m, l, r) = (*self.xyz(mid), *self.xyz(left), *self.xyz(right));
        let v1 = [l[0] - m[0], l[1] - m[1], l[2] - m[2]];
        let v2 = [r[0] - m[0], r[1] - m[1], r[2] - m[2]];
        let l1 = (v1[0] * v1[0] + v1[1] * v1[1] + v1[2] * v1[2]).sqrt();
        let l2 = (v2[0] * v2[0] + v2[1] * v2[1] + v2[2] * v2[2]).sqrt();
        let d = v1[0] * v2[0] + v1[1] * v2[1] + v1[2] * v2[2];
        let cos_ang = d / (l1 * l2);
        cos_ang < self.cos_collinear_ang_tol
    }

    /// `is_segment(p1, p2)`: are the two vertices endpoints of (a part of)
    /// one input segment.
    pub fn is_segment(&mut self, p1: Point, p2: Point) -> bool {
        self.reach[17] += 1;
        let t1 = self.pointtype(p1);
        let t2 = self.pointtype(p2);
        if t1 == VertType::Ridge {
            if t2 == VertType::Ridge {
                let idx = self.pointmark(p1) as usize;
                let lo = self.idx_segment_ridge_vertex_list[idx] as usize;
                let hi = self.idx_segment_ridge_vertex_list[idx + 1] as usize;
                for i in lo..hi {
                    if self.segment_ridge_vertex_list[i] == p2 {
                        return true;
                    }
                }
            } else if t2 == VertType::FreeSeg {
                let parsentseg = sdecode(self.point2sh(p2));
                let segidx = self.getfacetindex(&parsentseg) as usize;
                if self.segmentendpointslist[segidx * 2] == p1
                    || self.segmentendpointslist[segidx * 2 + 1] == p1
                {
                    return true;
                }
            }
        } else if t1 == VertType::FreeSeg {
            if t2 == VertType::Ridge {
                let parsentseg = sdecode(self.point2sh(p1));
                let segidx = self.getfacetindex(&parsentseg) as usize;
                if self.segmentendpointslist[segidx * 2] == p2
                    || self.segmentendpointslist[segidx * 2 + 1] == p2
                {
                    return true;
                }
            } else if t2 == VertType::FreeSeg {
                let s1 = sdecode(self.point2sh(p1));
                let s2 = sdecode(self.point2sh(p2));
                if self.getfacetindex(&s1) == self.getfacetindex(&s2) {
                    return true;
                }
            }
        }
        false
    }

    /// `valid_constrained_f23(checktet, pd, pe)`: reject a 2-to-3 flip that
    /// would make a face with three (nearly) collinear vertices on segments.
    pub fn valid_constrained_f23(&mut self, checktet: &TriFace, pd: Point, pe: Point) -> bool {
        self.reach[15] += 1;
        let mut checktet = *checktet;
        let mut validflag = true;
        for _k in 0..3 {
            let checkpt = self.org(&checktet);
            let mut spintet = esym(&checktet);
            enextself(&mut spintet); // [x, d], x = a,b,c
            let checkseg1 = self.tsspivot1(&spintet);
            let mut isseg = !checkseg1.is_null();
            if !isseg && self.boundary_recovery_flag {
                isseg = self.is_segment(checkpt, pd);
            }
            if isseg {
                let mut spintet = self.fsym(&checktet);
                esymself(&mut spintet);
                eprevself(&mut spintet);
                let checkseg2 = self.tsspivot1(&spintet);
                isseg = !checkseg2.is_null();
                if !isseg && self.boundary_recovery_flag {
                    isseg = self.is_segment(checkpt, pe);
                }
                if isseg {
                    if self.pointtype(checkpt) == VertType::FreeSeg {
                        validflag = false;
                        break;
                    } else if checkpt != DUMMYPOINT
                        && pe != DUMMYPOINT
                        && pd != DUMMYPOINT
                        && self.is_collinear_at(checkpt, pe, pd)
                    {
                        validflag = false;
                        break;
                    }
                }
            }
            enextself(&mut checktet);
        }
        validflag
    }

    /// `valid_constrained_f32(abtets, pa, pb)`: reject a 3-to-2 flip whose
    /// new face `[c,d,e]` would be (nearly) degenerate on segments.
    pub fn valid_constrained_f32(&mut self, abtets: &[TriFace; 3], _pa: Point, _pb: Point) -> bool {
        self.reach[16] += 1;
        let mut validflag = true;
        let mut checksegs = [Face::default(); 3];
        for k in 0..3 {
            let mut spintet = enext(&abtets[k]);
            esymself(&mut spintet);
            eprevself(&mut spintet); // [c,d], [d,e], and [e,c]
            checksegs[k] = self.tsspivot1(&spintet);
            // Ignore a temporaray segment (used in recoversubfaces()).
            if !checksegs[k].is_null() && self.smarktest2ed(&checksegs[k]) {
                checksegs[k].sh = super::mesh::NULL_SH;
            }
        }
        for k in 0..3 {
            let chkpt = self.apex(&abtets[k]);
            let leftpt = self.apex(&abtets[(k + 2) % 3]);
            let rightpt = self.apex(&abtets[(k + 1) % 3]);
            let mut isseg = !checksegs[k].is_null();
            if !isseg && self.boundary_recovery_flag {
                isseg = self.is_segment(chkpt, rightpt);
            }
            if isseg {
                isseg = !checksegs[(k + 2) % 3].is_null();
                if !isseg && self.boundary_recovery_flag {
                    isseg = self.is_segment(chkpt, leftpt);
                }
                if isseg {
                    if self.pointtype(chkpt) == VertType::FreeSeg {
                        validflag = false;
                        break;
                    } else if chkpt != DUMMYPOINT
                        && leftpt != DUMMYPOINT
                        && rightpt != DUMMYPOINT
                        && self.is_collinear_at(chkpt, leftpt, rightpt)
                    {
                        validflag = false;
                        break;
                    }
                }
            }
        }
        validflag
    }

    /// `checkflipeligibility(fliptype, pa, pb, pc, pd, pe, level,
    /// edgepivot, fc)`: does the flip cross the constraining edge or face,
    /// or create an edge at the vertex being removed. Returns `rejflag`.
    /// ~~The `remove_large_angle` test (mesh improvement) is not ported.~~
    /// **UPDATED 2026-11-26 (gh:#1794):** the `remove_large_angle` test of
    /// mesh improvement is ported ([`TetgenMesh::flip_angle_rejected`]).
    #[allow(clippy::too_many_arguments)]
    pub fn checkflipeligibility(
        &mut self,
        fliptype: i32,
        pa: Point,
        pb: Point,
        pc: Point,
        pd: Point,
        pe: Point,
        level: i32,
        edgepivot: i32,
        fc: &mut FlipConstraints,
    ) -> Result<bool, TetgenError> {
        self.reach[12] += 1;
        let mut types = [0i32; 2];
        let mut poss = [0i32; 4];
        let mut rejflag = false;

        if fc.seg[0] != NULL_POINT {
            // A constraining edge is given (e.g., for edge recovery).
            if fliptype == 1 {
                // A 2-to-3 flip: [a,b,c] => [e,d,a], [e,d,b], [e,d,c].
                let tmppts = [pa, pb, pc];
                let mut i = 0;
                while i < 3 && !rejflag {
                    if tmppts[i] != DUMMYPOINT {
                        // Test if the face [e,d,#] intersects the edge.
                        let intflag = self.tri_edge_test(
                            pe, pd, tmppts[i], fc.seg[0], fc.seg[1], None, 1, &mut types, &mut poss,
                        );
                        if intflag == 2 {
                            // They intersect at a single point.
                            let dir = types[0];
                            if dir == ACROSSFACE {
                                rejflag = true;
                            } else if dir == ACROSSEDGE {
                                if poss[0] == 0 {
                                    rejflag = true;
                                }
                            } else if dir == ACROSSVERT || dir == TOUCHEDGE || dir == TOUCHFACE {
                                rejflag = true;
                            }
                        } else if intflag == 4 {
                            // They may intersect at either a point or a line segment.
                            let dir = types[0];
                            if dir == ACROSSEDGE {
                                if poss[0] == 0 {
                                    rejflag = true;
                                }
                            } else if dir == ACROSSFACE {
                                return Err(TetgenError::InternalError);
                            } else if dir == ACROSSVERT || dir == TOUCHEDGE || dir == TOUCHFACE {
                                rejflag = true;
                            }
                        }
                    }
                    i += 1;
                }
            } else if fliptype == 2 {
                // A 3-to-2 flip: [e,d,a], [e,d,b], [e,d,c] => [a,b,c]
                if pc != DUMMYPOINT {
                    // Check if the new face [a,b,c] intersect the edge in its interior.
                    let intflag = self.tri_edge_test(
                        pa, pb, pc, fc.seg[0], fc.seg[1], None, 1, &mut types, &mut poss,
                    );
                    if intflag == 2 {
                        if types[0] == ACROSSFACE {
                            rejflag = true;
                        }
                    } else if intflag == 4 && types[0] == ACROSSEDGE {
                        rejflag = true;
                    }
                }
            }
        }

        if fc.fac[0] != NULL_POINT && !rejflag && fliptype == 1 {
            // A 2-to-3 flip: test if the new edge [e,d] intersects the face.
            let intflag = self.tri_edge_test(
                fc.fac[0], fc.fac[1], fc.fac[2], pe, pd, None, 1, &mut types, &mut poss,
            );
            if intflag == 2 {
                let dir = types[0];
                if dir == ACROSSFACE || dir == ACROSSEDGE {
                    rejflag = true;
                }
            } else if intflag == 4 {
                // The edge [e,d] is coplanar with the face.
                let mut i = 0;
                while i < 2 && !rejflag {
                    let dir = types[i];
                    if dir == ACROSSFACE || dir == ACROSSEDGE {
                        rejflag = true;
                    }
                    i += 1;
                }
            }
        }

        if fc.remvert != NULL_POINT && !rejflag && fliptype == 1 {
            // Do not create a new edge which contains the vertex to remove.
            if pd == fc.remvert || pe == fc.remvert {
                rejflag = true;
            }
        }

        if fc.remove_large_angle && !rejflag {
            rejflag =
                self.flip_angle_rejected(fliptype, [pa, pb, pc, pd, pe], level, edgepivot, fc);
        }

        Ok(rejflag)
    }

    /// `checkflipeligibility`'s `remove_large_angle` block (gh:#1794):
    /// reject a flip whose new tetrahedra do not improve the largest
    /// dihedral angle (`diff <= 0`, with upstream's relative rounding to
    /// `b->epsilon`); otherwise record the worst new angle and aspect ratio
    /// in `fc.cosdihed_out` / `fc.max_asp_out`.
    pub fn flip_angle_rejected(
        &self,
        fliptype: i32,
        p: [Point; 5],
        level: i32,
        edgepivot: i32,
        fc: &mut FlipConstraints,
    ) -> bool {
        let [pa, pb, pc, pd, pe] = p;
        // One tet's (cosmaxd, key), or None where upstream sets diff = 0.
        let check = |m: &Self, q: [Point; 4], fc: &FlipConstraints| -> Option<(f64, f64)> {
            match m.get_tetqual_pts(q[0], q[1], q[2], q[3]) {
                Some(bf) => {
                    let cosmaxd = bf.cent[0];
                    let mut diff = cosmaxd - fc.cosdihed_in;
                    if (diff / fc.cosdihed_in).abs() < m.b.epsilon {
                        diff = 0.0; // Rounding.
                    }
                    if diff <= 0.0 {
                        None
                    } else {
                        Some((cosmaxd, bf.key))
                    }
                }
                None => None, // no improve.
            }
        };
        let two = |m: &Self, q1: [Point; 4], q2: [Point; 4], fc: &mut FlipConstraints| -> bool {
            let mut min_cosmaxd: f64 = 1.0;
            let mut max_asp: f64 = 0.0;
            let Some((c1, k1)) = check(m, q1, fc) else {
                return true;
            };
            min_cosmaxd = if min_cosmaxd < c1 { min_cosmaxd } else { c1 };
            max_asp = if max_asp > k1 { max_asp } else { k1 };
            let Some((c2, k2)) = check(m, q2, fc) else {
                return true;
            };
            min_cosmaxd = if min_cosmaxd < c2 { min_cosmaxd } else { c2 };
            max_asp = if max_asp > k2 { max_asp } else { k2 };
            fc.cosdihed_out = if fc.cosdihed_out < min_cosmaxd {
                fc.cosdihed_out
            } else {
                min_cosmaxd
            };
            fc.max_asp_out = if fc.max_asp_out > max_asp {
                fc.max_asp_out
            } else {
                max_asp
            };
            false
        };
        let one = |m: &Self, q: [Point; 4], fc: &mut FlipConstraints| -> bool {
            let Some((c, k)) = check(m, q, fc) else {
                return true;
            };
            fc.cosdihed_out = if fc.cosdihed_out < c {
                fc.cosdihed_out
            } else {
                c
            };
            fc.max_asp_out = if fc.max_asp_out > k {
                fc.max_asp_out
            } else {
                k
            };
            false
        };
        if fliptype == 1 {
            // A 2-to-3 flip: [a,b,c] => [e,d,a], [e,d,b], [e,d,c]. Only
            //   [e,d,b,c] and [e,d,c,a] are checked.
            if pc != DUMMYPOINT && pe != DUMMYPOINT && pd != DUMMYPOINT {
                return two(self, [pe, pd, pb, pc], [pe, pd, pc, pa], fc);
            }
        } else if fliptype == 2 {
            // A 3-to-2 flip: [e,d,a], [e,d,b], [e,d,c] => [a,b,c].
            if pa != DUMMYPOINT && pb != DUMMYPOINT && pc != DUMMYPOINT {
                if level == 0 {
                    return two(self, [pa, pb, pc, pd], [pb, pa, pc, pe], fc);
                } else if edgepivot == 1 {
                    // The new tet [a,b,c,d] will be flipped. Only check [b,a,c,e].
                    return one(self, [pb, pa, pc, pe], fc);
                } else {
                    // The new tet [b,a,c,e] will be flipped. Only check [a,b,c,d].
                    return one(self, [pa, pb, pc, pd], fc);
                }
            }
        }
        false
    }

    // -- flip_cxx ------------------------------------------------------------

    /// `flippush(flipstack, flipface)`: queue a face (marked) unless marked.
    pub fn flippush(&mut self, flipface: &TriFace) {
        if !self.facemarked(flipface) {
            self.tflipstack.push(*flipface);
            self.markface(flipface);
        }
    }

    /// `flip23(fliptets, hullflag, fc)`: `[a,b,c,d]`, `[b,a,c,e]` become
    /// `[e,d,a,b]`, `[e,d,b,c]`, `[e,d,c,a]` (returned in `fliptets`).
    pub fn flip23(&mut self, fliptets: &mut [TriFace; 3], hullflag: i32, fc: &mut FlipConstraints) {
        self.reach[9] += 1;
        let mut dummyflag = 0; // range = {-1, 0, 1, 2}.
        if hullflag > 0 {
            // Check if e is dummypoint.
            if self.oppo(&fliptets[1]) == DUMMYPOINT {
                fliptets.swap(0, 1);
                dummyflag = -1; // d is dummypoint.
            } else if self.org(&fliptets[0]) == DUMMYPOINT {
                dummyflag = 1; // a is dummypoint.
                enextself(&mut fliptets[0]);
                eprevself(&mut fliptets[1]);
            } else if self.dest(&fliptets[0]) == DUMMYPOINT {
                dummyflag = 2; // b is dummypoint.
                eprevself(&mut fliptets[0]);
                enextself(&mut fliptets[1]);
            } else {
                dummyflag = 0; // either c or d may be dummypoint.
            }
        }

        let pa = self.org(&fliptets[0]);
        let pb = self.dest(&fliptets[0]);
        let pc = self.apex(&fliptets[0]);
        let pd = self.oppo(&fliptets[0]);
        let pe = self.oppo(&fliptets[1]);

        self.flip23count += 1;

        // Get the outer boundary faces.
        let mut topcastets = [TriFace::default(); 3];
        let mut botcastets = [TriFace::default(); 3];
        for t in topcastets.iter_mut() {
            *t = self.fnext(&fliptets[0]);
            enextself(&mut fliptets[0]);
        }
        for t in botcastets.iter_mut() {
            *t = self.fnext(&fliptets[1]);
            eprevself(&mut fliptets[1]);
        }

        // Re-use fliptets[0] and fliptets[1].
        fliptets[0].ver = 11;
        fliptets[1].ver = 11;
        self.clearelemmarker(fliptets[0].tet);
        self.clearelemmarker(fliptets[1].tet);
        if self.checksubsegflag {
            self.free_tet2seg(fliptets[0].tet);
            self.free_tet2seg(fliptets[1].tet);
        }
        if self.checksubfaceflag {
            self.free_tet2sub(fliptets[0].tet);
            self.free_tet2sub(fliptets[1].tet);
        }
        // Create a new tet.
        fliptets[2] = self.maketetrahedron();
        // The new tet have the same attributes from the old tet (`-A`).
        if self.numelemattrib > 0 {
            self.elemattrib[fliptets[2].tet] = self.elemattrib[fliptets[0].tet];
        }

        if hullflag > 0 {
            if pd != DUMMYPOINT {
                self.setvertices(&fliptets[0], pe, pd, pa, pb);
                self.setvertices(&fliptets[1], pe, pd, pb, pc);
                if pc != DUMMYPOINT {
                    self.setvertices(&fliptets[2], pe, pd, pc, pa);
                } else {
                    self.setvertices(&fliptets[2], pd, pe, pa, pc);
                    esymself(&mut fliptets[2]);
                }
            } else {
                // d is dummypoint.
                self.setvertices(&fliptets[0], pa, pb, pe, pd);
                self.setvertices(&fliptets[1], pb, pc, pe, pd);
                self.setvertices(&fliptets[2], pc, pa, pe, pd);
                for t in fliptets.iter_mut() {
                    eprevesymself(t);
                    enextself(t);
                }
                // We deleted one hull tet, and created three hull tets.
                self.hullsize += 2;
            }
        } else {
            self.setvertices(&fliptets[0], pe, pd, pa, pb);
            self.setvertices(&fliptets[1], pe, pd, pb, pc);
            self.setvertices(&fliptets[2], pe, pd, pc, pa);
        }

        if fc.remove_ndelaunay_edge {
            // calc_tetprism_vol
            let (xa, xb, xc, xd, xe) = (
                *self.xyz(pa),
                *self.xyz(pb),
                *self.xyz(pc),
                *self.xyz(pd),
                *self.xyz(pe),
            );
            let (volneg, volpos): ([f64; 2], [f64; 3]);
            if pd != DUMMYPOINT {
                if pc != DUMMYPOINT {
                    volpos = [
                        tetprismvol(&self.pred, &xe, &xd, &xa, &xb),
                        tetprismvol(&self.pred, &xe, &xd, &xb, &xc),
                        tetprismvol(&self.pred, &xe, &xd, &xc, &xa),
                    ];
                    volneg = [
                        tetprismvol(&self.pred, &xa, &xb, &xc, &xd),
                        tetprismvol(&self.pred, &xb, &xa, &xc, &xe),
                    ];
                } else {
                    volpos = [tetprismvol(&self.pred, &xe, &xd, &xa, &xb), 0.0, 0.0];
                    volneg = [0.0, 0.0];
                }
            } else {
                volpos = [0.0, 0.0, 0.0];
                volneg = [0.0, tetprismvol(&self.pred, &xb, &xa, &xc, &xe)];
            }
            let vol_diff = volpos[0] + volpos[1] + volpos[2] - volneg[0] - volneg[1];
            fc.tetprism_vol_sum += vol_diff;
        }

        // Bond three new tets together.
        for i in 0..3 {
            let newface = esym(&fliptets[i]);
            self.bond(&newface, &fliptets[(i + 1) % 3]);
        }
        // Bond to top outer boundary faces (at [a,b,c,d]).
        for i in 0..3 {
            let newface = eorgoppo(&fliptets[i]);
            self.bond(&newface, &topcastets[i]);
        }
        // Bond bottom outer boundary faces (at [b,a,c,e]).
        for i in 0..3 {
            let newface = edestoppo(&fliptets[i]);
            self.bond(&newface, &botcastets[i]);
        }

        if self.checksubsegflag {
            // The middle three: [a,b], [b,c], [c,a].
            for i in 0..3 {
                if self.issubseg(&topcastets[i]) {
                    let checkseg = self.tsspivot1(&topcastets[i]);
                    let newface = eorgoppo(&fliptets[i]);
                    self.tssbond1(&newface, &checkseg);
                    self.sstbond1(&checkseg, &newface);
                }
            }
            // The top three: [d,a], [d,b], [d,c]. Two tets per edge.
            for i in 0..3 {
                let casface = eprev(&topcastets[i]);
                if self.issubseg(&casface) {
                    let checkseg = self.tsspivot1(&casface);
                    let newface = enext(&fliptets[i]);
                    self.tssbond1(&newface, &checkseg);
                    self.sstbond1(&checkseg, &newface);
                    let mut newface = esym(&fliptets[(i + 2) % 3]);
                    eprevself(&mut newface);
                    self.tssbond1(&newface, &checkseg);
                    self.sstbond1(&checkseg, &newface);
                }
            }
            // The bot three: [a,e], [b,e], [c,e]. Two tets per edge.
            for i in 0..3 {
                let casface = enext(&botcastets[i]);
                if self.issubseg(&casface) {
                    let checkseg = self.tsspivot1(&casface);
                    let newface = eprev(&fliptets[i]);
                    self.tssbond1(&newface, &checkseg);
                    self.sstbond1(&checkseg, &newface);
                    let mut newface = esym(&fliptets[(i + 2) % 3]);
                    enextself(&mut newface);
                    self.tssbond1(&newface, &checkseg);
                    self.sstbond1(&checkseg, &newface);
                }
            }
        }

        if self.checksubfaceflag {
            // Bond 6 subfaces if there are.
            for i in 0..3 {
                if self.issubface(&topcastets[i]) {
                    let checksh = sesym(&self.tspivot(&topcastets[i]));
                    let newface = eorgoppo(&fliptets[i]);
                    self.tsbond(&newface, &checksh);
                }
            }
            for i in 0..3 {
                if self.issubface(&botcastets[i]) {
                    let checksh = sesym(&self.tspivot(&botcastets[i]));
                    let newface = edestoppo(&fliptets[i]);
                    self.tsbond(&newface, &checksh);
                }
            }
        }

        if fc.chkencflag & 4 != 0 {
            // Put three new tets into check list.
            for t in fliptets.iter() {
                self.enqueuetetrahedron(t);
            }
        }

        // Update the point-to-tet map.
        self.setpoint2tet(pa, encode2(fliptets[0].tet, 0));
        self.setpoint2tet(pb, encode2(fliptets[0].tet, 0));
        self.setpoint2tet(pc, encode2(fliptets[1].tet, 0));
        self.setpoint2tet(pd, encode2(fliptets[0].tet, 0));
        self.setpoint2tet(pe, encode2(fliptets[0].tet, 0));

        if hullflag > 0 && dummyflag != 0 {
            // Restore the original position of the points (for flipnm()).
            if dummyflag == -1 {
                // Reverse the edge.
                for t in fliptets.iter_mut() {
                    esymself(t);
                }
                // Swap the last two new tets.
                fliptets.swap(1, 2);
            } else if dummyflag == 1 {
                // a is dummypoint.
                let newface = fliptets[0];
                fliptets[0] = fliptets[2];
                fliptets[2] = fliptets[1];
                fliptets[1] = newface;
            } else {
                // b is dummypoint.
                let newface = fliptets[0];
                fliptets[0] = fliptets[1];
                fliptets[1] = fliptets[2];
                fliptets[2] = newface;
            }
        }

        if fc.enqflag > 0 {
            // Queue faces which may be locally non-Delaunay.
            for i in 0..3 {
                let newface = eprevesym(&fliptets[i]);
                self.flippush(&newface);
            }
            if fc.enqflag > 1 {
                for i in 0..3 {
                    let newface = enextesym(&fliptets[i]);
                    self.flippush(&newface);
                }
            }
        }

        self.recenttet = fliptets[0];
    }

    /// `flip32(fliptets, hullflag, fc)`: `[e,d,a,b]`, `[e,d,b,c]`,
    /// `[e,d,c,a]` become `[a,b,c,d]`, `[b,a,c,e]` (in `fliptets[0..2]`).
    /// Two subfaces at `[e,d]` are flipped 2-to-2 with it.
    pub fn flip32(
        &mut self,
        fliptets: &mut [TriFace; 3],
        hullflag: i32,
        fc: &mut FlipConstraints,
    ) -> Result<(), TetgenError> {
        self.reach[10] += 1;
        let mut dummyflag = 0;
        let mut spivot: i32 = -1;
        let mut scount = 0;

        if hullflag > 0 {
            // Check if e is 'dummypoint'.
            if self.org(&fliptets[0]) == DUMMYPOINT {
                // Reverse the edge.
                for t in fliptets.iter_mut() {
                    esymself(t);
                }
                // Swap the last two tets.
                fliptets.swap(1, 2);
                dummyflag = -1; // e is dummypoint.
            } else if self.apex(&fliptets[0]) == DUMMYPOINT {
                dummyflag = 1; // a is dummypoint.
                let newface = fliptets[0];
                fliptets[0] = fliptets[1];
                fliptets[1] = fliptets[2];
                fliptets[2] = newface;
            } else if self.apex(&fliptets[1]) == DUMMYPOINT {
                dummyflag = 2; // b is dummypoint.
                let newface = fliptets[0];
                fliptets[0] = fliptets[2];
                fliptets[2] = fliptets[1];
                fliptets[1] = newface;
            } else {
                dummyflag = 0; // either c or d may be dummypoint.
            }
        }

        let pa = self.apex(&fliptets[0]);
        let pb = self.apex(&fliptets[1]);
        let pc = self.apex(&fliptets[2]);
        let pd = self.dest(&fliptets[0]);
        let pe = self.org(&fliptets[0]);

        self.flip32count += 1;

        // Get the outer boundary faces.
        let mut topcastets = [TriFace::default(); 3];
        let mut botcastets = [TriFace::default(); 3];
        for i in 0..3 {
            let casface = eorgoppo(&fliptets[i]);
            topcastets[i] = self.fsym(&casface);
        }
        for i in 0..3 {
            let casface = edestoppo(&fliptets[i]);
            botcastets[i] = self.fsym(&casface);
        }

        let mut flipshs = [Face::default(); 3];
        if self.checksubfaceflag {
            // Check if there are interior subfaces at the edge [e,d].
            for i in 0..3 {
                flipshs[i] = self.tspivot(&fliptets[i]);
                if !flipshs[i].is_null() {
                    // Found an interior subface.
                    self.stdissolve(&flipshs[i]);
                    scount += 1;
                } else {
                    spivot = i as i32;
                }
            }
        }

        // Re-use fliptets[0] and fliptets[1].
        fliptets[0].ver = 11;
        fliptets[1].ver = 11;
        self.clearelemmarker(fliptets[0].tet);
        self.clearelemmarker(fliptets[1].tet);
        if self.checksubsegflag {
            self.free_tet2seg(fliptets[0].tet);
            self.free_tet2seg(fliptets[1].tet);
        }
        if self.checksubfaceflag {
            self.free_tet2sub(fliptets[0].tet);
            self.free_tet2sub(fliptets[1].tet);
        }
        if self.checksubfaceflag && scount > 0 && spivot >= 0 && self.numelemattrib > 0 {
            // The element attributes must be set correctly: the tet
            //   fliptets[spivot] is in the first region (`-A`; volume
            //   bounds, `-a`, are not ported).
            let a = self.elemattrib[fliptets[spivot as usize].tet];
            self.elemattrib[fliptets[0].tet] = a;
            self.elemattrib[fliptets[1].tet] = a;
        }
        // Delete an old tet.
        self.tetrahedrondealloc(fliptets[2].tet);

        if hullflag > 0 {
            if pc != DUMMYPOINT {
                if pd == DUMMYPOINT {
                    // We deleted three hull tets, and created one hull tet.
                    self.hullsize -= 2;
                }
                self.setvertices(&fliptets[0], pa, pb, pc, pd);
                self.setvertices(&fliptets[1], pb, pa, pc, pe);
            } else {
                // c is dummypoint. The two new tets are hull tets.
                self.setvertices(&fliptets[0], pb, pa, pd, pc);
                self.setvertices(&fliptets[1], pa, pb, pe, pc);
                esymself(&mut fliptets[0]);
                esymself(&mut fliptets[1]);
            }
        } else {
            self.setvertices(&fliptets[0], pa, pb, pc, pd);
            self.setvertices(&fliptets[1], pb, pa, pc, pe);
        }

        if fc.remove_ndelaunay_edge {
            let (xa, xb, xc, xd, xe) = (
                *self.xyz(pa),
                *self.xyz(pb),
                *self.xyz(pc),
                *self.xyz(pd),
                *self.xyz(pe),
            );
            let (volneg, volpos): ([f64; 3], [f64; 2]);
            if pc != DUMMYPOINT {
                if pd != DUMMYPOINT {
                    volneg = [
                        tetprismvol(&self.pred, &xe, &xd, &xa, &xb),
                        tetprismvol(&self.pred, &xe, &xd, &xb, &xc),
                        tetprismvol(&self.pred, &xe, &xd, &xc, &xa),
                    ];
                    volpos = [
                        tetprismvol(&self.pred, &xa, &xb, &xc, &xd),
                        tetprismvol(&self.pred, &xb, &xa, &xc, &xe),
                    ];
                } else {
                    volneg = [0.0, 0.0, 0.0];
                    volpos = [0.0, tetprismvol(&self.pred, &xb, &xa, &xc, &xe)];
                }
            } else {
                volneg = [tetprismvol(&self.pred, &xe, &xd, &xa, &xb), 0.0, 0.0];
                volpos = [0.0, 0.0];
            }
            let vol_diff = volpos[0] + volpos[1] - volneg[0] - volneg[1] - volneg[2];
            fc.tetprism_vol_sum += vol_diff;
        }

        // Bond abcd <==> bace.
        let (f0, f1) = (fliptets[0], fliptets[1]);
        self.bond(&f0, &f1);
        // Bond new faces to top outer boundary faces (at abcd).
        for t in topcastets.iter() {
            let newface = esym(&fliptets[0]);
            self.bond(&newface, t);
            enextself(&mut fliptets[0]);
        }
        // Bond new faces to bottom outer boundary faces (at bace).
        for t in botcastets.iter() {
            let newface = esym(&fliptets[1]);
            self.bond(&newface, t);
            eprevself(&mut fliptets[1]);
        }

        if self.checksubsegflag {
            // Bond 9 segments to new (flipped) tets.
            for t in topcastets.iter() {
                // edges a->b, b->c, c->a.
                if self.issubseg(t) {
                    let checkseg = self.tsspivot1(t);
                    let (f0, f1) = (fliptets[0], fliptets[1]);
                    self.tssbond1(&f0, &checkseg);
                    self.sstbond1(&checkseg, &f0);
                    self.tssbond1(&f1, &checkseg);
                    self.sstbond1(&checkseg, &f1);
                }
                enextself(&mut fliptets[0]);
                eprevself(&mut fliptets[1]);
            }
            // The three top edges.
            for t in topcastets.iter() {
                // edges b->d, c->d, a->d.
                let mut newface = esym(&fliptets[0]);
                eprevself(&mut newface);
                let casface = enext(t);
                if self.issubseg(&casface) {
                    let checkseg = self.tsspivot1(&casface);
                    self.tssbond1(&newface, &checkseg);
                    self.sstbond1(&checkseg, &newface);
                }
                enextself(&mut fliptets[0]);
            }
            // The three bot edges.
            for t in botcastets.iter() {
                // edges b<-e, c<-e, a<-e.
                let mut newface = esym(&fliptets[1]);
                enextself(&mut newface);
                let casface = eprev(t);
                if self.issubseg(&casface) {
                    let checkseg = self.tsspivot1(&casface);
                    self.tssbond1(&newface, &checkseg);
                    self.sstbond1(&checkseg, &newface);
                }
                eprevself(&mut fliptets[1]);
            }
        }

        if self.checksubfaceflag {
            // Bond the top three casing subfaces.
            for t in topcastets.iter() {
                // At edges [b,a], [c,b], [a,c]
                if self.issubface(t) {
                    let checksh = sesym(&self.tspivot(t));
                    let newface = esym(&fliptets[0]);
                    self.tsbond(&newface, &checksh);
                }
                enextself(&mut fliptets[0]);
            }
            // Bond the bottom three casing subfaces.
            for t in botcastets.iter() {
                // At edges [a,b], [b,c], [c,a]
                if self.issubface(t) {
                    let checksh = sesym(&self.tspivot(t));
                    let newface = esym(&fliptets[1]);
                    self.tsbond(&newface, &checksh);
                }
                eprevself(&mut fliptets[1]);
            }

            if scount > 0 {
                // Perform a 2-to-2 flip in subfaces.
                let sp = spivot as usize;
                let mut flipfaces = [flipshs[(sp + 1) % 3], sesym(&flipshs[(sp + 2) % 3])];
                self.flip22(&mut flipfaces, false);
                // Connect the flipped subfaces to flipped tets.
                topcastets[0] = fliptets[0];
                botcastets[0] = fliptets[1];
                for _ in 0..((sp + 1) % 3) {
                    enextself(&mut topcastets[0]);
                    eprevself(&mut botcastets[0]);
                }
                // Connect the top subface to the top tets.
                esymself(&mut topcastets[0]);
                flipfaces[0] = sesym(&flipfaces[0]);
                // Check if there already exists a subface.
                let checksh = self.tspivot(&topcastets[0]);
                if checksh.is_null() {
                    let t = topcastets[0];
                    self.tsbond(&t, &flipfaces[0]);
                    let t = self.fsym(&t); // fsymself(topcastets[0])
                    flipfaces[0] = sesym(&flipfaces[0]);
                    self.tsbond(&t, &flipfaces[0]);
                } else {
                    // An invalid 2-to-2 flip. Report a bug.
                    return Err(TetgenError::InternalError);
                }
                // Connect the bot subface to the bottom tets.
                esymself(&mut botcastets[0]);
                flipfaces[1] = sesym(&flipfaces[1]);
                let checksh = self.tspivot(&botcastets[0]);
                if checksh.is_null() {
                    let t = botcastets[0];
                    self.tsbond(&t, &flipfaces[1]);
                    let t = self.fsym(&t); // fsymself(botcastets[0])
                    flipfaces[1] = sesym(&flipfaces[1]);
                    self.tsbond(&t, &flipfaces[1]);
                } else {
                    return Err(TetgenError::InternalError);
                }
            }
        }

        if fc.chkencflag & 4 != 0 {
            // Put two new tets into check list.
            for t in fliptets.iter().take(2) {
                self.enqueuetetrahedron(t);
            }
        }

        self.setpoint2tet(pa, encode2(fliptets[0].tet, 0));
        self.setpoint2tet(pb, encode2(fliptets[0].tet, 0));
        self.setpoint2tet(pc, encode2(fliptets[0].tet, 0));
        self.setpoint2tet(pd, encode2(fliptets[0].tet, 0));
        self.setpoint2tet(pe, encode2(fliptets[1].tet, 0));

        if hullflag > 0 && dummyflag != 0 {
            // Restore the original position of the points (for flipnm()).
            if dummyflag == -1 {
                // e were dummypoint. Swap the two new tets.
                fliptets.swap(0, 1);
            } else if dummyflag == 1 {
                eprevself(&mut fliptets[0]);
                enextself(&mut fliptets[1]);
            } else {
                enextself(&mut fliptets[0]);
                eprevself(&mut fliptets[1]);
            }
        }

        if fc.enqflag > 0 {
            // Queue faces which may be locally non-Delaunay.
            let newface = enextesym(&fliptets[0]);
            self.flippush(&newface);
            let newface = eprevesym(&fliptets[1]);
            self.flippush(&newface);
            if fc.enqflag > 1 {
                let newface = eprevesym(&fliptets[0]);
                self.flippush(&newface);
                let newface = enextesym(&fliptets[1]);
                self.flippush(&newface);
                let newface = esym(&fliptets[0]);
                self.flippush(&newface);
                let newface = esym(&fliptets[1]);
                self.flippush(&newface);
            }
        }

        self.recenttet = fliptets[0];
        Ok(())
    }
}

impl TetgenMesh {
    /// `flip31(flipfaces, flipflag)`: `[p,a,b]`, `[p,b,c]`, `[p,c,a]` are
    /// replaced by `[a,b,c]`, returned in `flipfaces[3]`. The three old
    /// subfaces are not deleted (the caller does).
    pub fn flip31(&mut self, flipfaces: &mut [Face; 4], flipflag: bool) {
        self.reach[30] += 1;
        let pa = self.sdest(&flipfaces[0]);
        let pb = self.sdest(&flipfaces[1]);
        let pc = self.sdest(&flipfaces[2]);

        self.flip31count += 1;

        let mut bdedges = [Face::default(); 3];
        let mut outfaces = [Face::default(); 3];
        let mut infaces = [Face::default(); 3];
        let mut bdsegs = [Face::default(); 3];
        // Collect all infos at the three boundary edges.
        for i in 0..3 {
            bdedges[i] = senext(&flipfaces[i]);
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

        // Create a new subface.
        flipfaces[3] = self.makeshellface(ShPool::Sub);
        self.setshvertices(&flipfaces[3], pa, pb, pc);
        let mk = self.shellmark(&flipfaces[0]);
        self.setshellmark(&flipfaces[3], mk);
        // useinsertradius (-p): the facet index.
        let fi = self.getfacetindex(&flipfaces[0]);
        self.setfacetindex(&flipfaces[3], fi);

        // Update the point-to-subface map.
        for p in [pa, pb, pc] {
            if self.pointtype(p) == VertType::FreeFacet {
                self.setpoint2sh(p, sencode(&flipfaces[3]));
            }
        }

        // Update the three new boundary edges.
        bdedges[0] = flipfaces[3];
        bdedges[1] = senext(&flipfaces[3]);
        bdedges[2] = senext2(&flipfaces[3]);

        // Reconnect boundary edges to outer boundary faces.
        for i in 0..3 {
            if !outfaces[i].is_null() {
                // Make sure that the subface has the ori as the segment.
                if !bdsegs[i].is_null() {
                    bdsegs[i].shver = 0;
                    if self.sorg(&bdedges[i]) != self.sorg(&bdsegs[i]) {
                        bdedges[i].shver ^= 1;
                    }
                }
                self.sbond1(&bdedges[i], &outfaces[i]);
                self.sbond1(&infaces[i], &bdedges[i]);
            }
            if !bdsegs[i].is_null() {
                self.ssbond(&bdedges[i], &bdsegs[i]);
            }
        }

        self.recentsh = flipfaces[3];

        if flipflag {
            for e in &bdedges {
                self.flipshpush(e);
            }
        }
    }

    /// `flip41(fliptets, hullflag, fc)`: remove the vertex `p` of the four
    /// tets `[p,d,a,b]`, `[p,d,b,c]`, `[p,d,c,a]`, `[a,b,c,p]`;
    /// `fliptets[0]` returns `[a,b,c,d]`. Three subfaces at `p` are flipped
    /// 3-to-1 with it.
    pub fn flip41(
        &mut self,
        fliptets: &mut [TriFace; 4],
        hullflag: i32,
        fc: &mut FlipConstraints,
    ) -> Result<(), TetgenError> {
        self.reach[11] += 1;
        let mut dummyflag = 0;
        let mut spivot: i32 = -1;
        let mut scount = 0;

        let pa = self.org(&fliptets[3]);
        let pb = self.dest(&fliptets[3]);
        let pc = self.apex(&fliptets[3]);
        let pd = self.dest(&fliptets[0]);
        let pp = self.org(&fliptets[0]); // The removing vertex.

        self.flip41count += 1;

        // Get the outer boundary faces.
        let mut topcastets = [TriFace::default(); 3];
        for i in 0..3 {
            let mut t = enext(&fliptets[i]);
            self.fnextself(&mut t); // [d,a,b,#], [d,b,c,#], [d,c,a,#]
            enextself(&mut t); // [a,b,d,#], [b,c,d,#], [c,a,d,#]
            topcastets[i] = t;
        }
        let botcastet = self.fsym(&fliptets[3]); // [b,a,c,#]

        let mut flipshs = [Face::default(); 4];
        if self.checksubfaceflag {
            // Check if there are three subfaces at 'p'.
            for i in 0..3 {
                let newface = self.fnext(&fliptets[3]); // [a,b,p,d],[b,c,p,d],[c,a,p,d].
                flipshs[i] = self.tspivot(&newface);
                if !flipshs[i].is_null() {
                    spivot = i as i32; // Remember this subface.
                    scount += 1;
                }
                enextself(&mut fliptets[3]);
            }
            if scount > 0 {
                // There are three subfaces connecting at p.
                if scount < 3 {
                    // The new subface is one of {[a,b,d], [b,c,d], [c,a,d]}.
                    let mut neightet = self.fsym(&topcastets[spivot as usize]);
                    for f in flipshs.iter_mut().take(3) {
                        let newface = esym(&neightet);
                        *f = self.tspivot(&newface);
                        eprevself(&mut neightet);
                    }
                } else {
                    spivot = 3; // The new subface is [a,b,c].
                }
            }
        }

        // Re-use fliptets[0] for [a,b,c,d].
        fliptets[0].ver = 11;
        self.clearelemmarker(fliptets[0].tet);
        if self.checksubsegflag {
            self.free_tet2seg(fliptets[0].tet);
        }
        if self.checksubfaceflag {
            self.free_tet2sub(fliptets[0].tet);
        }
        // Delete the other three tets.
        for t in fliptets.iter().skip(1) {
            self.tetrahedrondealloc(t.tet);
        }

        if pp != DUMMYPOINT {
            // Mark the point pp as unused.
            self.setpointtype(pp, VertType::Unused);
            self.unuverts += 1;
        }

        // Create the new tet [a,b,c,d].
        if hullflag > 0 {
            if pa == DUMMYPOINT {
                self.setvertices(&fliptets[0], pc, pb, pd, pa);
                esymself(&mut fliptets[0]); // [b,c,a,d]
                eprevself(&mut fliptets[0]); // [a,b,c,d]
                dummyflag = 1;
            } else if pb == DUMMYPOINT {
                self.setvertices(&fliptets[0], pa, pc, pd, pb);
                esymself(&mut fliptets[0]); // [c,a,b,d]
                enextself(&mut fliptets[0]); // [a,b,c,d]
                dummyflag = 2;
            } else if pc == DUMMYPOINT {
                self.setvertices(&fliptets[0], pb, pa, pd, pc);
                esymself(&mut fliptets[0]); // [a,b,c,d]
                dummyflag = 3;
            } else if pd == DUMMYPOINT {
                self.setvertices(&fliptets[0], pa, pb, pc, pd);
                dummyflag = 4;
            } else {
                self.setvertices(&fliptets[0], pa, pb, pc, pd);
                dummyflag = if pp == DUMMYPOINT { -1 } else { 0 };
            }
            if dummyflag > 0 {
                // We deleted 3 hull tets, and create 1 hull tet.
                self.hullsize -= 2;
            } else if dummyflag < 0 {
                // We deleted 4 hull tets.
                self.hullsize -= 4;
            }
        } else {
            self.setvertices(&fliptets[0], pa, pb, pc, pd);
        }

        if fc.remove_ndelaunay_edge {
            let x = |p: Point| *self.xyz(p);
            let (xa, xb, xc, xd, xp) = (x(pa), x(pb), x(pc), x(pd), x(pp));
            let pr = &self.pred;
            let (volneg, volpos): ([f64; 4], f64);
            if dummyflag > 0 {
                if pa == DUMMYPOINT {
                    volneg = [0.0, tetprismvol(pr, &xp, &xd, &xb, &xc), 0.0, 0.0];
                } else if pb == DUMMYPOINT {
                    volneg = [0.0, 0.0, tetprismvol(pr, &xp, &xd, &xc, &xa), 0.0];
                } else if pc == DUMMYPOINT {
                    volneg = [tetprismvol(pr, &xp, &xd, &xa, &xb), 0.0, 0.0, 0.0];
                } else {
                    volneg = [0.0, 0.0, 0.0, tetprismvol(pr, &xa, &xb, &xc, &xp)];
                }
                volpos = 0.0;
            } else if dummyflag < 0 {
                volneg = [0.0; 4];
                volpos = tetprismvol(pr, &xa, &xb, &xc, &xd);
            } else {
                volneg = [
                    tetprismvol(pr, &xp, &xd, &xa, &xb),
                    tetprismvol(pr, &xp, &xd, &xb, &xc),
                    tetprismvol(pr, &xp, &xd, &xc, &xa),
                    tetprismvol(pr, &xa, &xb, &xc, &xp),
                ];
                volpos = tetprismvol(pr, &xa, &xb, &xc, &xd);
            }
            let vol_diff = volpos - volneg[0] - volneg[1] - volneg[2] - volneg[3];
            fc.tetprism_vol_sum += vol_diff;
        }

        // Bond the new tet to adjacent tets.
        for t in topcastets.iter() {
            let newface = esym(&fliptets[0]); // At faces [b,a,d], [c,b,d], [a,c,d].
            self.bond(&newface, t);
            enextself(&mut fliptets[0]);
        }
        let f0 = fliptets[0];
        self.bond(&f0, &botcastet);

        if self.checksubsegflag {
            // Bond 6 segments (at edges of [a,b,c,d]) if there there are.
            for t in topcastets.iter() {
                let newface = eprev(t); // At edges [d,a],[d,b],[d,c].
                if self.issubseg(&newface) {
                    let checkseg = self.tsspivot1(&newface);
                    let mut newface = esym(&fliptets[0]);
                    enextself(&mut newface); // At edges [a,d], [b,d], [c,d].
                    self.tssbond1(&newface, &checkseg);
                    self.sstbond1(&checkseg, &newface);
                }
                enextself(&mut fliptets[0]);
            }
            for t in topcastets.iter() {
                if self.issubseg(t) {
                    let checkseg = self.tsspivot1(t); // At edges [a,b],[b,c],[c,a].
                    let f0 = fliptets[0];
                    self.tssbond1(&f0, &checkseg);
                    self.sstbond1(&checkseg, &f0);
                }
                enextself(&mut fliptets[0]);
            }
        }

        if self.checksubfaceflag {
            // Bond 4 subfaces (at faces of [a,b,c,d]) if there are.
            for t in topcastets.iter() {
                if self.issubface(t) {
                    let checksh = sesym(&self.tspivot(t)); // At faces [a,b,d],[b,c,d],[c,a,d]
                    let newface = esym(&fliptets[0]); // At faces [b,a,d],[c,b,d],[a,c,d]
                    self.tsbond(&newface, &checksh);
                }
                enextself(&mut fliptets[0]);
            }
            if self.issubface(&botcastet) {
                let checksh = sesym(&self.tspivot(&botcastet)); // At face [b,a,c]
                let f0 = fliptets[0];
                self.tsbond(&f0, &checksh);
            }

            if spivot >= 0 {
                // Perform a 3-to-1 flip in surface triangulation.
                for f in flipshs.iter_mut().take(3) {
                    *f = senext2(f);
                }
                self.flip31(&mut flipshs, false);
                // Delete the three old subfaces.
                for f in flipshs.iter().take(3) {
                    self.shellfacedealloc(ShPool::Sub, f.sh);
                }
                if spivot < 3 {
                    // Bond the new subface to the new tet [a,b,c,d].
                    let t = topcastets[spivot as usize];
                    self.tsbond(&t, &flipshs[3]);
                    let newface = self.fsym(&t);
                    let checksh = sesym(&flipshs[3]);
                    self.tsbond(&newface, &checksh);
                } else {
                    // Bound the new subface [a,b,c] to the new tet [a,b,c,d].
                    let f0 = fliptets[0];
                    self.tsbond(&f0, &flipshs[3]);
                    let newface = self.fsym(&f0);
                    let checksh = sesym(&flipshs[3]);
                    self.tsbond(&newface, &checksh);
                }
            }
        }

        if fc.chkencflag & 4 != 0 {
            let f0 = fliptets[0];
            self.enqueuetetrahedron(&f0);
        }

        // Update the point-to-tet map.
        for p in [pa, pb, pc, pd] {
            self.setpoint2tet(p, encode2(fliptets[0].tet, 0));
        }

        if fc.enqflag > 0 {
            // Queue faces which may be locally non-Delaunay.
            let f0 = fliptets[0];
            self.flippush(&f0); // [a,b,c] (opposite to new point).
            if fc.enqflag > 1 {
                for _ in 0..3 {
                    let newface = esym(&fliptets[0]);
                    self.flippush(&newface);
                    enextself(&mut fliptets[0]);
                }
            }
        }

        self.recenttet = fliptets[0];
        let _ = dummyflag;
        Ok(())
    }

    /// `flipnm(abtets, n, level, abedgepivot, fc)`: flip the edge `[a,b]`
    /// whose star is `abtets[0..n]` through a sequence of elementary flips.
    /// Returns 2 if the edge is flipped, else the size of the current star.
    pub fn flipnm(
        &mut self,
        abtets: &mut [AbTet],
        n: usize,
        level: i32,
        abedgepivot: i32,
        fc: &mut FlipConstraints,
    ) -> Result<usize, TetgenError> {
        self.reach[7] += 1;
        let at = |abtets: &[AbTet], i: usize| abtets[i].t();
        let pa = self.org(&at(abtets, 0));
        let pb = self.dest(&at(abtets, 0));

        if n > 3 {
            // Try to reduce the size of the Star(ab) by flipping a face in it.
            let mut reflexlinkedgecount = 0;

            let mut i = 0;
            while i < n {
                // Let the face of 'abtets[i]' be [a,b,c].
                if self.checksubfaceflag && self.issubface(&at(abtets, i)) {
                    i += 1;
                    continue; // Skip a subface.
                }
                // Do not flip this face if it is involved in two Stars.
                if self.elemcounter(at(abtets, i).tet) > 1
                    || self.elemcounter(at(abtets, (i + n - 1) % n).tet) > 1
                {
                    i += 1;
                    continue;
                }

                let pc = self.apex(&at(abtets, i));
                let pd = self.apex(&at(abtets, (i + 1) % n));
                let pe = self.apex(&at(abtets, (i + n - 1) % n));
                if pd == DUMMYPOINT || pe == DUMMYPOINT {
                    i += 1;
                    continue; // [a,b,c] is a hull face.
                }

                // Decide whether [a,b,c] is flippable or not.
                let mut reducflag = false;
                let mut ori = 0.0;

                let hullflag = (pc == DUMMYPOINT) as i32; // pc may be dummypoint.
                let mut hulledgeflag = false;
                if hullflag == 0 {
                    ori = self.ori3(pb, pc, pd, pe); // Is [b,c] locally convex?
                    if ori > 0.0 {
                        ori = self.ori3(pc, pa, pd, pe); // Is [c,a] locally convex?
                        if ori > 0.0 {
                            // Test if [a,b] is locally convex OR flat.
                            ori = self.ori3(pa, pb, pd, pe);
                            if ori > 0.0 {
                                // Found a 2-to-3 flip: [a,b,c] => [e,d]
                                reducflag = true;
                            } else if ori == 0.0 && n == 4 {
                                // The "flat" tet can be removed immediately by a 3-to-2 flip.
                                reducflag = true;
                                // Check if [e,d] is a hull edge.
                                let pf = self.apex(&at(abtets, (i + 2) % n));
                                hulledgeflag = pf == DUMMYPOINT;
                            }
                        }
                    }
                    if !reducflag {
                        reflexlinkedgecount += 1;
                    }
                } else if n == 4 {
                    // 'c' is dummypoint.
                    let pf = self.apex(&at(abtets, (i + 2) % n));
                    ori = self.ori3(pd, pe, pf, pa);
                    if ori < 0.0 {
                        ori = self.ori3(pe, pd, pf, pb);
                        if ori < 0.0 {
                            // Found a 4-to-4 flip: [a,b] => [e,d]
                            reducflag = true;
                            ori = 0.0; // Signal as a 4-to-4 flip (like a co-planar case).
                            hulledgeflag = true; // [e,d] is a hull edge.
                        }
                    }
                }

                if reducflag && self.nonconvex && hulledgeflag {
                    // We will create a hull edge [e,d]. Make sure it does not exist.
                    let mut spintet = TriFace::default();
                    if self.getedge(pe, pd, &mut spintet)? {
                        // The 2-to-3 flip is not a topological valid flip.
                        reducflag = false;
                    }
                }

                if reducflag {
                    let checktet = at(abtets, i);
                    if !self.valid_constrained_f23(&checktet, pd, pe) {
                        reducflag = false;
                    }
                }

                if reducflag {
                    // [a,b,c] could be removed by a 2-to-3 flip.
                    let mut rejflag = false;
                    if fc.checkflipeligibility {
                        rejflag = self.checkflipeligibility(
                            1,
                            pa,
                            pb,
                            pc,
                            pd,
                            pe,
                            level,
                            abedgepivot,
                            fc,
                        )?;
                    }
                    if !rejflag {
                        // Do flip: [a,b,c] => [e,d].
                        let f0 = at(abtets, i);
                        let f1 = self.fsym(&f0); // abtets[i-1].
                        let mut fliptets = [f0, f1, TriFace::default()];
                        self.flip23(&mut fliptets, hullflag, fc);

                        // Shrink the array 'abtets', maintain the original order.
                        edestoppoself(&mut fliptets[0]); // [a,b,e,d]
                                                         // Increase the counter of this new tet (it is in Star(ab)).
                        self.increaseelemcounter(fliptets[0].tet);
                        abtets[(i + n - 1) % n] = AbTet::Tet(fliptets[0]);
                        for j in i..n - 1 {
                            abtets[j] = abtets[j + 1].clone(); // Upshift
                        }
                        // The last entry remembers 'c' and the position [i] of this flip.
                        abtets[n - 1] = AbTet::Flip23 { pc, pos: i };

                        if fc.collectnewtets {
                            // Push the two new tets [e,d,b,c] and [e,d,c,a] into a stack.
                            self.cavetetlist.push(fliptets[1]);
                            self.cavetetlist.push(fliptets[2]);
                        }

                        // Star(ab) is reduced. Try to flip the edge [a,b].
                        let mut nn = self.flipnm(abtets, n - 1, level, abedgepivot, fc)?;

                        if nn == 2 {
                            // The edge has been flipped.
                            return Ok(nn);
                        } else if fc.unflip || ori == 0.0 {
                            // Undo the previous 2-to-3 flip, i.e., do a 3-to-2 flip to
                            //   transform [e,d] => [a,b,c].
                            let mut f0 = at(abtets, (i + (n - 1) - 1) % (n - 1)); // [a,b,e,d]
                            edestoppoself(&mut f0); // [e,d,a,b]
                            let f1 = self.fnext(&f0); // [1] is [e,d,b,c]
                            let f2 = self.fnext(&f1); // [2] is [e,d,c,a]
                            let mut fliptets = [f0, f1, f2];
                            // Restore the two original tets in Star(ab).
                            self.flip32(&mut fliptets, hullflag, fc)?;
                            // Marktest the two restored tets in Star(ab).
                            for f in fliptets.iter().take(2) {
                                self.increaseelemcounter(f.tet);
                            }
                            // Expand the array 'abtets', maintain the original order.
                            let mut j = n as isize - 2;
                            while j >= i as isize {
                                abtets[j as usize + 1] = abtets[j as usize].clone(); // Downshift
                                j -= 1;
                            }
                            abtets[(i + n - 1) % n] = AbTet::Tet(esym(&fliptets[1])); // [a,b,e,c]
                            abtets[i] = AbTet::Tet(fliptets[0]); // [a,b,c,d]
                            nn += 1;
                            if fc.collectnewtets {
                                // Pop two (flipped) tets from the stack.
                                let l = self.cavetetlist.len();
                                self.cavetetlist.truncate(l - 2);
                            }
                        }

                        if !fc.unflip {
                            // The flips are not reversed. The current Star(ab) can not be
                            //   further reduced. Return its current size (# of tets).
                            return Ok(nn);
                        }
                        // unflip is set. Continue the search for flips.
                    }
                }
                i += 1;
            }

            // The Star(ab) is not reduced.
            if reflexlinkedgecount > 0 {
                // There are reflex edges in the Link(ab).
                if (self.b.fliplinklevel < 0 && level < self.autofliplinklevel)
                    || (self.b.fliplinklevel >= 0 && level < self.b.fliplinklevel)
                {
                    // Try to reduce the Star(ab) by flipping a reflex edge in Link(ab).
                    let mut i = 0;
                    while i < n {
                        // Do not flip this face [a,b,c] if there are two Stars involved.
                        if self.elemcounter(at(abtets, i).tet) > 1
                            || self.elemcounter(at(abtets, (i + n - 1) % n).tet) > 1
                        {
                            i += 1;
                            continue;
                        }
                        let pc = self.apex(&at(abtets, i));
                        if pc == DUMMYPOINT {
                            i += 1;
                            continue; // [a,b] is a hull edge.
                        }
                        let pd = self.apex(&at(abtets, (i + 1) % n));
                        let pe = self.apex(&at(abtets, (i + n - 1) % n));
                        if pd == DUMMYPOINT || pe == DUMMYPOINT {
                            i += 1;
                            continue; // [a,b,c] is a hull face.
                        }

                        let mut edgepivot = 0; // No edge is selected yet.
                        let mut flipedge = TriFace::default();

                        // Test if [b,c] is locally convex or flat.
                        let ori = self.ori3(pb, pc, pd, pe);
                        if ori <= 0.0 {
                            // Select the edge [c,b].
                            flipedge = enext(&at(abtets, i)); // [b,c,a,d]
                            edgepivot = 1;
                        }
                        if edgepivot == 0 {
                            // Test if [c,a] is locally convex or flat.
                            let ori = self.ori3(pc, pa, pd, pe);
                            if ori <= 0.0 {
                                // Select the edge [a,c].
                                flipedge = eprev(&at(abtets, i)); // [c,a,b,d].
                                edgepivot = 2;
                            }
                        }

                        if edgepivot == 0 {
                            i += 1;
                            continue;
                        }

                        // An edge is selected.
                        if self.checksubsegflag && self.issubseg(&flipedge) {
                            // Do not flip it if it is a segment.
                            if fc.collectencsegflag {
                                let checkseg = self.tsspivot1(&flipedge);
                                if !self.sinfected(&checkseg) {
                                    // Queue this segment in list.
                                    self.sinfect(&checkseg);
                                    self.caveencseglist.push(checkseg);
                                }
                            }
                            i += 1;
                            continue;
                        }

                        // Try to flip the selected edge ([c,b] or [a,c]).
                        esymself(&mut flipedge);
                        // Count the number of tets at the edge.
                        let mut subface_count = 0;
                        let mut n1 = 0;
                        let mut jsum = 0; // Sum of the star counters.
                        let mut spintet = flipedge;
                        loop {
                            if self.issubface(&spintet) {
                                subface_count += 1;
                            }
                            n1 += 1;
                            jsum += self.elemcounter(spintet.tet);
                            self.fnextself(&mut spintet);
                            if spintet.tet == flipedge.tet {
                                break;
                            }
                        }
                        if n1 < 3 {
                            // This is only possible when the mesh contains inverted
                            //   elements.  Reprot a bug.
                            return Err(TetgenError::InternalError);
                        }
                        if jsum > 2 {
                            // The Star(flipedge) overlaps other Stars.
                            i += 1;
                            continue; // Do not flip this edge.
                        }

                        if fc.noflip_in_surface && subface_count > 0 {
                            i += 1;
                            continue;
                        }

                        if self.b.flipstarsize > 0 && n1 > self.b.flipstarsize as usize {
                            // The star size exceeds the given limit.
                            i += 1;
                            continue; // Do not flip it.
                        }

                        // Allocate spaces for Star(flipedge).
                        let mut tmpabtets: Vec<AbTet> = Vec::with_capacity(n1);
                        // Form the Star(flipedge).
                        let mut spintet = flipedge;
                        for _ in 0..n1 {
                            tmpabtets.push(AbTet::Tet(spintet));
                            // Increase the star counter of this tet.
                            self.increaseelemcounter(spintet.tet);
                            self.fnextself(&mut spintet);
                        }

                        // Try to flip the selected edge away.
                        self.reach[42] += 1;
                        let nn = self.flipnm(&mut tmpabtets, n1, level + 1, edgepivot, fc)?;

                        if nn == 2 {
                            // The edge is flipped. Star(ab) is reduced.
                            // Shrink the array 'abtets', maintain the original order.
                            let mut spintet;
                            if edgepivot == 1 {
                                // 'tmpabtets[0]' is [d,a,e,b] => contains [a,b].
                                spintet = tmpabtets[0].t(); // [d,a,e,b]
                                enextself(&mut spintet);
                                esymself(&mut spintet);
                                enextself(&mut spintet); // [a,b,e,d]
                            } else {
                                // 'tmpabtets[1]' is [b,d,e,a] => contains [a,b].
                                spintet = tmpabtets[1].t(); // [b,d,e,a]
                                eprevself(&mut spintet);
                                esymself(&mut spintet);
                                eprevself(&mut spintet); // [a,b,e,d]
                            }
                            self.increaseelemcounter(spintet.tet); // It is in Star(ab).
                                                                   // Put the new tet at [i-1]-th entry.
                            abtets[(i + n - 1) % n] = AbTet::Tet(spintet);
                            for j in i..n - 1 {
                                abtets[j] = abtets[j + 1].clone(); // Upshift
                            }
                            // Remember the flipped link vertex 'c'.
                            tmpabtets[0] = AbTet::Vertex(pc);
                            // Remember the flips in the last entry of the array 'abtets'.
                            abtets[n - 1] = AbTet::FlipNm {
                                star: tmpabtets,
                                edgepivot: edgepivot as usize,
                                pos: i,
                                n1,
                            };

                            // Continue to flip the edge [a,b].
                            let mut nn = self.flipnm(abtets, n - 1, level, abedgepivot, fc)?;

                            if nn == 2 {
                                // The edge has been flipped.
                                return Ok(nn);
                            } else if fc.unflip {
                                // Recover the flipped edge ([c,b] or [a,c]).
                                let mut tmpabtets = match core::mem::replace(
                                    &mut abtets[n - 1],
                                    AbTet::Tet(TriFace::default()),
                                ) {
                                    AbTet::FlipNm { star, .. } => star,
                                    _ => return Err(TetgenError::InternalError),
                                };
                                if edgepivot == 1 {
                                    // The flip edge is [c,b].
                                    let mut t0 = at(abtets, (i + (n - 1) - 1) % (n - 1)); // [a,b,e,d]
                                    eprevself(&mut t0);
                                    esymself(&mut t0);
                                    eprevself(&mut t0); // [d,a,e,b]
                                    let t1 = self.fsym(&t0); // [a,d,e,c]
                                    tmpabtets[0] = AbTet::Tet(t0);
                                    tmpabtets[1] = AbTet::Tet(t1);
                                } else {
                                    // The flip edge is [a,c].
                                    let mut t1 = at(abtets, (i + (n - 1) - 1) % (n - 1)); // [a,b,e,d]
                                    enextself(&mut t1);
                                    esymself(&mut t1);
                                    enextself(&mut t1); // [b,d,e,a]
                                    let t0 = self.fsym(&t1); // [d,b,e,c]
                                    tmpabtets[0] = AbTet::Tet(t0);
                                    tmpabtets[1] = AbTet::Tet(t1);
                                }

                                // Recover the flipped edge ([c,b] or [a,c]).
                                self.flipnm_post(&mut tmpabtets, n1, 2, edgepivot, fc)?;

                                // Insert the two recovered tets into Star(ab).
                                let mut j = n as isize - 2;
                                while j >= i as isize {
                                    abtets[j as usize + 1] = abtets[j as usize].clone(); // Downshift
                                    j -= 1;
                                }
                                let (mut f0, mut f1);
                                if edgepivot == 1 {
                                    f0 = tmpabtets[1].t();
                                    enextself(&mut f0);
                                    esymself(&mut f0); // [a,b,e,c]
                                    f1 = tmpabtets[0].t();
                                    esymself(&mut f1);
                                    eprevself(&mut f1); // [a,b,c,d]
                                } else {
                                    f0 = tmpabtets[1].t();
                                    eprevself(&mut f0);
                                    esymself(&mut f0); // [a,b,e,c]
                                    f1 = tmpabtets[0].t();
                                    esymself(&mut f1);
                                    enextself(&mut f1); // [a,b,c,d]
                                }
                                self.increaseelemcounter(f0.tet);
                                self.increaseelemcounter(f1.tet);
                                // Insert the two recovered tets into Star(ab).
                                abtets[(i + n - 1) % n] = AbTet::Tet(f0);
                                abtets[i] = AbTet::Tet(f1);
                                nn += 1;
                            }

                            if !fc.unflip {
                                // The flips are not reversed. The current Star(ab) can not be
                                //   further reduced. Return its size (# of tets).
                                return Ok(nn);
                            }
                            // unflip is set. Continue the search for flips.
                        } else {
                            // The selected edge is not flipped.
                            if !fc.unflip {
                                // Release the memory used in this attempted flip.
                                self.flipnm_post(&mut tmpabtets, n1, nn, edgepivot, fc)?;
                            }
                            // Decrease the star counters of tets in Star(flipedge).
                            for t in tmpabtets.iter().take(nn) {
                                self.decreaseelemcounter(t.t().tet);
                            }
                        }
                        i += 1;
                    }
                }
            }
        } else {
            // Check if a 3-to-2 flip is possible.
            let t0 = at(abtets, 0);
            let t1 = at(abtets, 1);
            let t2 = at(abtets, 2);
            let (pc, pd, pe, hullflag);
            if self.apex(&t0) == DUMMYPOINT {
                pc = self.apex(&t1);
                pd = self.apex(&t2);
                pe = self.apex(&t0);
                hullflag = 1;
            } else if self.apex(&t1) == DUMMYPOINT {
                pc = self.apex(&t2);
                pd = self.apex(&t0);
                pe = self.apex(&t1);
                hullflag = 2;
            } else {
                pc = self.apex(&t0);
                pd = self.apex(&t1);
                pe = self.apex(&t2);
                hullflag = if pe == DUMMYPOINT { 3 } else { 0 };
            }

            let mut reducflag = false;
            let mut rejflag = false;

            if hullflag == 0 {
                // Make sure that no inverted tet will be created.
                let ori = self.ori3(pd, pc, pe, pa);
                if ori < 0.0 {
                    let ori = self.ori3(pc, pd, pe, pb);
                    if ori < 0.0 {
                        reducflag = true;
                    }
                }
            } else {
                // [a,b] is a hull edge.
                if !self.nonconvex {
                    // The mesh is convex, only do flip if it is a coplanar hull edge.
                    let ori = self.ori3(pa, pb, pc, pd);
                    if ori == 0.0 {
                        reducflag = true;
                    }
                } else {
                    reducflag = true;
                }
                if reducflag {
                    // Make sure that no inverted tet will be created.
                    self.reach[52] += 1;
                    let mut searchpt = NULL_POINT;
                    let mut bigvol = 0.0;
                    let mut f0 = at(abtets, (hullflag % 3) as usize); // [a,b,c,d].
                    eorgoppoself_(&mut f0); // [d,c,b,a]
                    let mut spintet = f0;
                    loop {
                        self.fnextself(&mut spintet);
                        let chkpt = self.oppo(&spintet);
                        if chkpt == pb {
                            break;
                        }
                        if chkpt != DUMMYPOINT && self.apex(&spintet) != DUMMYPOINT {
                            let ori = -self.ori3(pd, pc, self.apex(&spintet), chkpt);
                            if ori > bigvol {
                                bigvol = ori;
                                searchpt = chkpt;
                            }
                        }
                    }
                    if searchpt != NULL_POINT {
                        // Now valid the configuration.
                        let ori1 = self.ori3(pd, pc, searchpt, pa);
                        let ori2 = self.ori3(pd, pc, searchpt, pb);
                        if ori1 * ori2 >= 0.0 {
                            reducflag = false; // Not valid.
                        } else {
                            let ori1 = self.ori3(pa, pb, searchpt, pc);
                            let ori2 = self.ori3(pa, pb, searchpt, pd);
                            if ori1 * ori2 >= 0.0 {
                                reducflag = false; // Not valid.
                            }
                        }
                    } else {
                        // No valid searchpt is found.
                        reducflag = false; // Do not flip it.
                    }
                }
            }

            if reducflag {
                // A 3-to-2 flip is possible.
                if self.checksubfaceflag {
                    let mut nn = 0;
                    let mut edgepivot: i32 = -1; // Re-use it.
                    for j in 0..3 {
                        if self.issubface(&at(abtets, j)) {
                            nn += 1; // Found a subface.
                        } else {
                            edgepivot = j as i32;
                        }
                    }
                    if nn == 1 {
                        // Found only 1 subface containing this edge.
                        rejflag = true;
                    } else if nn == 2 {
                        // Found two subfaces. A 2-to-2 flip is possible. Validate it.
                        let mut spintet = eorgoppo(&at(abtets, ((edgepivot + 1) % 3) as usize)); // [q,p,b,a]
                        if self.issubface(&spintet) {
                            rejflag = true; // Conflict to a 2-to-2 flip.
                        } else {
                            esymself(&mut spintet);
                            if self.issubface(&spintet) {
                                rejflag = true; // Conflict to a 2-to-2 flip.
                            }
                        }
                    } else if nn == 3 {
                        // Report a bug.
                        return Err(TetgenError::InternalError);
                    }
                }

                let mut three = [at(abtets, 0), at(abtets, 1), at(abtets, 2)];
                if !rejflag && !self.valid_constrained_f32(&three, pa, pb) {
                    rejflag = true;
                }

                if !rejflag && fc.checkflipeligibility {
                    // Here we must exchange 'a' and 'b'.
                    rejflag =
                        self.checkflipeligibility(2, pc, pd, pe, pb, pa, level, abedgepivot, fc)?;
                }
                if !rejflag {
                    // Do flip: [a,b] => [c,d,e]
                    self.flip32(&mut three, hullflag, fc)?;
                    for (j, t) in three.iter().enumerate() {
                        abtets[j] = AbTet::Tet(*t);
                    }
                    if fc.remove_ndelaunay_edge && level == 0 {
                        // It is the desired removing edge. Check if we have improved
                        //   the objective function.
                        if fc.tetprism_vol_sum >= 0.0
                            || fc.tetprism_vol_sum.abs() < fc.bak_tetprism_vol
                        {
                            // No improvement! flip back: [c,d,e] => [a,b].
                            self.flip23(&mut three, hullflag, fc);
                            for (j, t) in three.iter().enumerate() {
                                abtets[j] = AbTet::Tet(*t);
                            }
                            // Increase the element counter -- They are in cavity.
                            for t in three.iter() {
                                self.increaseelemcounter(t.tet);
                            }
                            return Ok(3);
                        }
                    }
                    if fc.collectnewtets {
                        // Collect new tets.
                        if level == 0 {
                            // Push the two new tets into stack.
                            self.cavetetlist.push(three[0]);
                            self.cavetetlist.push(three[1]);
                        } else if abedgepivot == 1 {
                            // [c,b]
                            self.cavetetlist.push(three[1]);
                        } else {
                            self.cavetetlist.push(three[0]);
                        }
                    }
                    return Ok(2);
                }
            }
        }

        // The current (reduced) Star size.
        Ok(n)
    }

    /// `flipnm_post(abtets, n, nn, abedgepivot, fc)`: walk back through the
    /// flips `flipnm` did (undoing them if `fc.unflip`); upstream also
    /// frees the nested star arrays here.
    pub fn flipnm_post(
        &mut self,
        abtets: &mut [AbTet],
        n: usize,
        nn: usize,
        abedgepivot: i32,
        fc: &mut FlipConstraints,
    ) -> Result<(), TetgenError> {
        self.reach[8] += 1;
        let mut nn = nn;
        if nn == 2 {
            // The edge [a,b] has been flipped.
            if fc.unflip {
                // Do a 2-to-3 flip to recover the edge [a,b].
                let mut three = [abtets[0].t(), abtets[1].t(), abtets[2].t()];
                self.flip23(&mut three, 1, fc);
                for (j, t) in three.iter().enumerate() {
                    abtets[j] = AbTet::Tet(*t);
                }
                if fc.collectnewtets {
                    let l = self.cavetetlist.len();
                    if abedgepivot == 0 {
                        self.cavetetlist.truncate(l - 2);
                    } else {
                        self.cavetetlist.truncate(l - 1);
                    }
                }
            }
            // The initial size of Star(ab) is 3.
            nn += 1;
        }

        // Walk through the performed flips.
        for i in nn..n {
            match abtets[i].clone() {
                AbTet::Flip23 { pos: t, .. } => {
                    // It was a 2-to-3 flip: [a,b,c]->[e,d].
                    if fc.unflip {
                        let mut f0 = abtets[(t + i - 1) % i].t(); // [a,b,e,d]
                        eprevself(&mut f0);
                        esymself(&mut f0);
                        enextself(&mut f0); // [e,d,a,b]
                        let f1 = self.fnext(&f0); // [e,d,b,c]
                        let f2 = self.fnext(&f1); // [e,d,c,a]
                        let mut fliptets = [f0, f1, f2];
                        // Do a 3-to-2 flip: [e,d] => [a,b,c].
                        self.flip32(&mut fliptets, 1, fc)?;
                        // Expand the array 'abtets', maintain the original order.
                        let mut j = i as isize - 1;
                        while j >= t as isize {
                            abtets[j as usize + 1] = abtets[j as usize].clone(); // Downshift
                            j -= 1;
                        }
                        abtets[(t + i) % (i + 1)] = AbTet::Tet(esym(&fliptets[1])); // [a,b,e,c]
                        abtets[t] = AbTet::Tet(fliptets[0]); // [a,b,c,d]
                        if fc.collectnewtets {
                            let l = self.cavetetlist.len();
                            self.cavetetlist.truncate(l - 2);
                        }
                    }
                }
                AbTet::FlipNm {
                    star: mut tmpabtets,
                    edgepivot,
                    pos: t,
                    n1,
                } => {
                    if fc.unflip {
                        if edgepivot == 1 {
                            // The flip edge is [c,b].
                            let mut t0 = abtets[(t + i - 1) % i].t(); // [a,b,e,d]
                            eprevself(&mut t0);
                            esymself(&mut t0);
                            eprevself(&mut t0); // [d,a,e,b]
                            let t1 = self.fsym(&t0); // [a,d,e,c]
                            tmpabtets[0] = AbTet::Tet(t0);
                            tmpabtets[1] = AbTet::Tet(t1);
                        } else {
                            // The flip edge is [a,c].
                            let mut t1 = abtets[(t + i - 1) % i].t(); // [a,b,e,d]
                            enextself(&mut t1);
                            esymself(&mut t1);
                            enextself(&mut t1); // [b,d,e,a]
                            let t0 = self.fsym(&t1); // [d,b,e,c]
                            tmpabtets[0] = AbTet::Tet(t0);
                            tmpabtets[1] = AbTet::Tet(t1);
                        }
                        // Do a n1-to-m1 flip to recover the flipped edge ([c,b] or [a,c]).
                        self.flipnm_post(&mut tmpabtets, n1, 2, edgepivot as i32, fc)?;
                        // Insert the two recovered tets into the original Star(ab).
                        let mut j = i as isize - 1;
                        while j >= t as isize {
                            abtets[j as usize + 1] = abtets[j as usize].clone(); // Downshift
                            j -= 1;
                        }
                        let (mut f0, mut f1);
                        if edgepivot == 1 {
                            f0 = tmpabtets[1].t();
                            enextself(&mut f0);
                            esymself(&mut f0); // [a,b,e,c]
                            f1 = tmpabtets[0].t();
                            esymself(&mut f1);
                            eprevself(&mut f1); // [a,b,c,d]
                        } else {
                            f0 = tmpabtets[1].t();
                            eprevself(&mut f0);
                            esymself(&mut f0); // [a,b,e,c]
                            f1 = tmpabtets[0].t();
                            esymself(&mut f1);
                            enextself(&mut f1); // [a,b,c,d]
                        }
                        abtets[(t + i) % (i + 1)] = AbTet::Tet(f0);
                        abtets[t] = AbTet::Tet(f1);
                    } else {
                        // Only free the spaces.
                        self.flipnm_post(&mut tmpabtets, n1, 2, edgepivot as i32, fc)?;
                    }
                }
                _ => {}
            }
        }
        Ok(())
    }

    /// `removeedgebyflips(flipedge, fc)`: 2 if the edge is removed, else the
    /// size of its star (0 if it is a segment or may not be flipped).
    pub fn removeedgebyflips(
        &mut self,
        flipedge: &mut TriFace,
        fc: &mut FlipConstraints,
    ) -> Result<usize, TetgenError> {
        self.reach[5] += 1;
        if self.checksubsegflag && self.issubseg(flipedge) {
            // Do not flip a segment.
            if fc.collectencsegflag {
                let checkseg = self.tsspivot1(flipedge);
                if !self.sinfected(&checkseg) {
                    // Queue this segment in list.
                    self.sinfect(&checkseg);
                    self.caveencseglist.push(checkseg);
                }
            }
            return Ok(0);
        }

        // Count the number of tets at edge [a,b].
        let mut subface_count = 0;
        let mut n = 0;
        let mut spintet = *flipedge;
        loop {
            if self.issubface(&spintet) {
                subface_count += 1;
            }
            n += 1;
            self.fnextself(&mut spintet);
            if spintet.tet == flipedge.tet {
                break;
            }
        }
        if n < 3 {
            // It is only possible when the mesh contains inverted tetrahedra.
            return Err(TetgenError::InternalError);
        }

        if fc.noflip_in_surface && subface_count > 0 {
            return Ok(0);
        }

        if self.b.flipstarsize > 0 && n > self.b.flipstarsize as usize {
            // The star size exceeds the limit.
            return Ok(0);
        }

        // Collect the tets at edge [a,b].
        let mut abtets: Vec<AbTet> = Vec::with_capacity(n);
        let mut spintet = *flipedge;
        for _ in 0..n {
            abtets.push(AbTet::Tet(spintet));
            self.setelemcounter(spintet.tet, 1);
            self.fnextself(&mut spintet);
        }

        // Try to flip the edge (level = 0, edgepivot = 0).
        let nn = self.flipnm(&mut abtets, n, 0, 0, fc)?;

        if nn > 2 {
            // Edge is not flipped. Unmarktest the remaining tets in Star(ab).
            for t in abtets.iter().take(nn) {
                self.setelemcounter(t.t().tet, 0);
            }
            // Restore the input edge (needed by Lawson's flip).
            *flipedge = abtets[0].t();
        }

        // Release the temporary allocated spaces. NOTE: fc->unflip must be 0.
        let bakunflip = fc.unflip;
        fc.unflip = false;
        self.flipnm_post(&mut abtets, n, nn, 0, fc)?;
        fc.unflip = bakunflip;

        Ok(nn)
    }

    /// `removefacebyflips(flipface, fc)`: true if the face is removed (by a
    /// 2-to-3 flip, or by removing one of its edges).
    pub fn removefacebyflips(
        &mut self,
        flipface: &TriFace,
        fc: &mut FlipConstraints,
    ) -> Result<bool, TetgenError> {
        self.reach[6] += 1;
        let f0 = *flipface;
        let f1 = self.fsym(flipface);
        let pa = self.org(&f0);
        let pb = self.dest(&f0);
        let pc = self.apex(&f0);
        let pd = self.oppo(&f0);
        let pe = self.oppo(&f1);

        let mut reducflag = false;
        let mut flipedge = TriFace::default();
        let ori = self.ori3(pa, pb, pd, pe);
        if ori > 0.0 {
            let ori = self.ori3(pb, pc, pd, pe);
            if ori > 0.0 {
                let ori = self.ori3(pc, pa, pd, pe);
                if ori > 0.0 {
                    // Found a 2-to-3 flip.
                    reducflag = true;
                } else {
                    flipedge = eprev(flipface); // [c,a]
                }
            } else {
                flipedge = enext(flipface); // [b,c]
            }
        } else {
            flipedge = *flipface; // [a,b]
        }

        if reducflag {
            let checkface = f0;
            if !self.valid_constrained_f23(&checkface, pd, pe) {
                return Ok(false);
            }
        }

        if reducflag {
            // A 2-to-3 flip is found.
            let mut fliptets = [f0, f1, TriFace::default()];
            self.flip23(&mut fliptets, 0, fc);
            return Ok(true);
        } else if self.removeedgebyflips(&mut flipedge, fc)? == 2 {
            // Try to flip the selected edge of this face.
            return Ok(true);
        }

        // Face is not removed.
        Ok(false)
    }
}

/// `eorgoppoself(t)`.
#[inline]
fn eorgoppoself_(t: &mut TriFace) {
    t.ver = EORGOPPOTBL[t.ver];
}

/// `edestoppoself(t)`.
#[inline]
fn edestoppoself(t: &mut TriFace) {
    t.ver = EDESTOPPOTBL[t.ver];
}
