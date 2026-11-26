// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright (C) 2026 OUTRAM PARK contributors
//
// Port of TetGen 1.6.0 (the mesh data structure: `tetgen.h` class
// `tetgenmesh`, its inline primitives, and `tetgen.cxx` section
// `mempool_cxx`: the lookup tables, `inittables`, `memorypool`,
// `maketetrahedron`, `makepoint`, `initializepools`):
//   TetGen — A Quality Tetrahedral Mesh Generator and 3D Delaunay Triangulator
//   https://github.com/libigl/tetgen, commit e05aca7df74e (TetGen 1.6.0)
//   Copyright (C) 2002-2020 Hang Si, Weierstrass Institute for Applied
//   Analysis and Stochastics (WIAS), Berlin.
//   Licence: GNU Affero General Public License v3.0 or later.
// and of the GNU C Library's `rand()` / `srand()` (the `TYPE_3` additive
// feedback generator of `stdlib/random_r.c`), whose sequence TetGen's output
// depends on; this is a reimplementation of the documented algorithm, not a
// copy of glibc's source.
//
// This file is part of OUTRAM PARK. It is free software: you can redistribute
// it and/or modify it under the terms of the GNU Affero General Public License
// as published by the Free Software Foundation, either version 3 of the
// License, or (at your option) any later version. It is distributed WITHOUT
// ANY WARRANTY; see the GNU Affero General Public License for details.

//! The TetGen mesh object ([`TetgenMesh`]) and its data structure (stage 2,
//! gh:#1791).
//!
//! # Records, with indices for pointers
//!
//! Upstream keeps tetrahedra and points as records in a `memorypool` and
//! links them with raw pointers. Here a record is an element of a `Vec`, and
//! a pointer is an index (no `unsafe`):
//!
//! - A **tetrahedron** is [`TetRecord`], indexed by its pool **slot** (its
//!   allocation position). Upstream's twelve words map one for one: `[0..3]`
//!   the four neighbours ([`TetRecord::nbr`], encoded handles), `[4..7]` the
//!   vertices ([`TetRecord::v`]), `[8]` the subsegment array and `[9]` the
//!   subface array (reserved for ~~stage 3~~ stage 4 on: `meshsurface` does
//!   not touch them, [`TetRecord::seg`] /
//!   [`TetRecord::sub`]), `[10]` the element index, `[11]` the marker and
//!   flag bits ([`TetRecord::marker`], the same bits as upstream's
//!   `infect`, `marktest`, `markface`, `markedge`, `marktest2`, counter).
//! - An **encoded handle** ([`Tetrahedron`], upstream's `tetrahedron` with a
//!   version in its four low bits) is `((slot + 1) << 4) | ver`; `0` is
//!   `NULL`.
//! - A [`TriFace`] is `(tet, ver)` exactly as upstream's `triface`, with
//!   `ver` in `0..12` indexing the same tables ([`ESYMTBL`], [`ENEXTTBL`],
//!   [`ORGPIVOT`], ...). Every table is transcribed, including those the
//!   later stages use ([`BONDTBL`], [`FSYMTBL`], [`TSBONDTBL`], ...).
//! - A **point** is [`PointRecord`], at index `slot + 1` of
//!   [`TetgenMesh::points`]; index [`DUMMYPOINT`] (0) is upstream's
//!   `dummypoint`, the vertex at infinity of the hull tetrahedra.
//!
//! # The pool order is part of the answer
//!
//! TetGen numbers its output by walking the pools ([`MemoryPool`]): in
//! allocation-slot order, skipping dead items; and a freed item goes onto a
//! LIFO dead-item stack that the next allocation takes from first. So the
//! element order of the `.ele` file depends on exactly which slot each new
//! tetrahedron reuses. [`MemoryPool`] reproduces `alloc`, `dealloc`,
//! `traversalinit` and `traverse` (including the per-block layout that
//! `randomsample` reads); a plain `Vec::push` would not.
//!
//! # Randomness
//!
//! Two generators feed the result: TetGen's own `randomnation` (an LCG whose
//! seed lives in the mesh, [`TetgenMesh::randomnation`]) and the C library's
//! `rand()` (the vertex permutation, `locate_dt`'s face choice and the new
//! `recenttet` after each insertion). The reference binary links glibc, so
//! [`GlibcRand`] reproduces glibc's `rand()`; on a C library with another
//! `rand()` (macOS, musl, MSVC) upstream itself produces a different, equally
//! valid tetrahedralisation.

#[cfg(not(any(feature = "std", test)))]
#[allow(unused_imports)]
use crate::nostd_prelude::*;

use super::predicates::Predicates;
use alloc::vec::Vec;

/// An encoded tetrahedron handle (upstream's `tetrahedron`, a pointer with
/// the version in its four low bits): `((slot + 1) << 4) | ver`; `0` is
/// `NULL`.
pub type Tetrahedron = usize;

/// The `NULL` encoded handle.
pub const NULL_HANDLE: Tetrahedron = 0;

/// A tetrahedron slot that is `NULL` (`triface.tet == NULL`).
pub const NULL_TET: usize = usize::MAX;

/// A point: an index into [`TetgenMesh::points`] (pool slot + 1).
pub type Point = usize;

/// Upstream's `dummypoint`, the vertex at infinity.
pub const DUMMYPOINT: Point = 0;

/// A `NULL` point (a dead tetrahedron has `v[0] == NULL_POINT`).
pub const NULL_POINT: Point = usize::MAX;

// ---------------------------------------------------------------------------
// The lookup tables (tetgen.cxx, `mempool_cxx`).
// ---------------------------------------------------------------------------

/// `esymtbl`: takes a directed edge (version) and returns the reversed edge
/// (version) of it.
pub const ESYMTBL: [usize; 12] = [9, 6, 11, 4, 3, 7, 1, 5, 10, 0, 8, 2];

/// `orgpivot`: the 12 permutations of `{0,1,2,3}`, offset by 4 for direct
/// access of the vertex words of a tetrahedron record.
pub const ORGPIVOT: [usize; 12] = [7, 7, 5, 5, 6, 4, 4, 6, 5, 6, 7, 4];
/// `destpivot`.
pub const DESTPIVOT: [usize; 12] = [6, 4, 4, 6, 5, 6, 7, 4, 7, 7, 5, 5];
/// `apexpivot`.
pub const APEXPIVOT: [usize; 12] = [5, 6, 7, 4, 7, 7, 5, 5, 6, 4, 4, 6];
/// `oppopivot`.
pub const OPPOPIVOT: [usize; 12] = [4, 5, 6, 7, 4, 5, 6, 7, 4, 5, 6, 7];

/// `ver2edge`: version to undirected edge.
pub const VER2EDGE: [usize; 12] = [0, 1, 2, 3, 3, 5, 1, 5, 4, 0, 4, 2];
/// `edge2ver`: undirected edge to version.
pub const EDGE2VER: [usize; 6] = [0, 1, 2, 3, 8, 5];

/// `epivot`: edge versions whose apex or opposite may be `dummypoint`.
pub const EPIVOT: [usize; 12] = [4, 5, 2, 11, 4, 5, 2, 11, 4, 5, 2, 11];

/// `snextpivot`: the next edge version in the same edge ring (subfaces).
pub const SNEXTPIVOT: [usize; 6] = [2, 5, 4, 1, 0, 3];

/// `sorgpivot`: the 6 permutations of `{0,1,2}`, offset by 3 (subfaces).
pub const SORGPIVOT: [usize; 6] = [3, 4, 4, 5, 5, 3];
/// `sdestpivot`.
pub const SDESTPIVOT: [usize; 6] = [4, 3, 5, 4, 3, 5];
/// `sapexpivot`.
pub const SAPEXPIVOT: [usize; 6] = [5, 5, 3, 3, 4, 4];

/// The tables `inittables()` computes (upstream fills static arrays at run
/// time; here a `const fn` fills them at compile time, by the same loops).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Tables {
    /// `bondtbl[t1.ver][t2.ver]`.
    pub bondtbl: [[usize; 12]; 12],
    /// `enexttbl`.
    pub enexttbl: [usize; 12],
    /// `eprevtbl`.
    pub eprevtbl: [usize; 12],
    /// `enextesymtbl`.
    pub enextesymtbl: [usize; 12],
    /// `eprevesymtbl`.
    pub eprevesymtbl: [usize; 12],
    /// `eorgoppotbl`.
    pub eorgoppotbl: [usize; 12],
    /// `edestoppotbl`.
    pub edestoppotbl: [usize; 12],
    /// `fsymtbl[t1.ver][t2.ver]`.
    pub fsymtbl: [[usize; 12]; 12],
    /// `facepivot1`.
    pub facepivot1: [usize; 12],
    /// `facepivot2`.
    pub facepivot2: [[usize; 12]; 12],
    /// `tsbondtbl[t.ver][s.shver]`.
    pub tsbondtbl: [[usize; 6]; 12],
    /// `stbondtbl[t.ver][s.shver]`.
    pub stbondtbl: [[usize; 6]; 12],
    /// `tspivottbl[t.ver][s.shver]`.
    pub tspivottbl: [[usize; 6]; 12],
    /// `stpivottbl[t.ver][s.shver]`.
    pub stpivottbl: [[usize; 6]; 12],
}

/// `inittables()`: initialize the look-up tables (line for line).
pub const fn inittables() -> Tables {
    let mut t = Tables {
        bondtbl: [[0; 12]; 12],
        enexttbl: [0; 12],
        eprevtbl: [0; 12],
        enextesymtbl: [0; 12],
        eprevesymtbl: [0; 12],
        eorgoppotbl: [0; 12],
        edestoppotbl: [0; 12],
        fsymtbl: [[0; 12]; 12],
        facepivot1: [0; 12],
        facepivot2: [[0; 12]; 12],
        tsbondtbl: [[0; 6]; 12],
        stbondtbl: [[0; 6]; 12],
        tspivottbl: [[0; 6]; 12],
        stpivottbl: [[0; 6]; 12],
    };
    let mut i;
    let mut j;

    // i = t1.ver; j = t2.ver;
    i = 0;
    while i < 12 {
        j = 0;
        while j < 12 {
            t.bondtbl[i][j] = (j & 3) + (((i & 12) + (j & 12)) % 12);
            j += 1;
        }
        i += 1;
    }

    // i = t1.ver; j = t2.ver
    i = 0;
    while i < 12 {
        j = 0;
        while j < 12 {
            t.fsymtbl[i][j] = (j + 12 - (i & 12)) % 12;
            j += 1;
        }
        i += 1;
    }

    i = 0;
    while i < 12 {
        t.facepivot1[i] = ESYMTBL[i] & 3;
        i += 1;
    }

    i = 0;
    while i < 12 {
        j = 0;
        while j < 12 {
            t.facepivot2[i][j] = t.fsymtbl[ESYMTBL[i]][j];
            j += 1;
        }
        i += 1;
    }

    i = 0;
    while i < 12 {
        t.enexttbl[i] = (i + 4) % 12;
        t.eprevtbl[i] = (i + 8) % 12;
        i += 1;
    }

    i = 0;
    while i < 12 {
        t.enextesymtbl[i] = ESYMTBL[t.enexttbl[i]];
        t.eprevesymtbl[i] = ESYMTBL[t.eprevtbl[i]];
        i += 1;
    }

    i = 0;
    while i < 12 {
        t.eorgoppotbl[i] = t.eprevtbl[ESYMTBL[t.enexttbl[i]]];
        t.edestoppotbl[i] = t.enexttbl[ESYMTBL[t.eprevtbl[i]]];
        i += 1;
    }

    // i = t.ver, j = s.shver
    i = 0;
    while i < 12 {
        j = 0;
        while j < 6 {
            let soffset;
            let toffset;
            if (j & 1) == 0 {
                soffset = (6 - ((i & 12) >> 1)) % 6;
                toffset = (12 - ((j & 6) << 1)) % 12;
            } else {
                soffset = (i & 12) >> 1;
                toffset = (j & 6) << 1;
            }
            t.tsbondtbl[i][j] = (j & 1) + (((j & 6) + soffset) % 6);
            t.stbondtbl[i][j] = (i & 3) + (((i & 12) + toffset) % 12);
            j += 1;
        }
        i += 1;
    }

    // i = t.ver, j = s.shver
    i = 0;
    while i < 12 {
        j = 0;
        while j < 6 {
            let soffset;
            let toffset;
            if (j & 1) == 0 {
                soffset = (i & 12) >> 1;
                toffset = (j & 6) << 1;
            } else {
                soffset = (6 - ((i & 12) >> 1)) % 6;
                toffset = (12 - ((j & 6) << 1)) % 12;
            }
            t.tspivottbl[i][j] = (j & 1) + (((j & 6) + soffset) % 6);
            t.stpivottbl[i][j] = (i & 3) + (((i & 12) + toffset) % 12);
            j += 1;
        }
        i += 1;
    }
    t
}

/// The tables, computed once at compile time.
pub const TABLES: Tables = inittables();
/// `bondtbl`.
pub const BONDTBL: [[usize; 12]; 12] = TABLES.bondtbl;
/// `enexttbl`.
pub const ENEXTTBL: [usize; 12] = TABLES.enexttbl;
/// `eprevtbl`.
pub const EPREVTBL: [usize; 12] = TABLES.eprevtbl;
/// `enextesymtbl`.
pub const ENEXTESYMTBL: [usize; 12] = TABLES.enextesymtbl;
/// `eprevesymtbl`.
pub const EPREVESYMTBL: [usize; 12] = TABLES.eprevesymtbl;
/// `eorgoppotbl`.
pub const EORGOPPOTBL: [usize; 12] = TABLES.eorgoppotbl;
/// `edestoppotbl`.
pub const EDESTOPPOTBL: [usize; 12] = TABLES.edestoppotbl;
/// `fsymtbl`.
pub const FSYMTBL: [[usize; 12]; 12] = TABLES.fsymtbl;
/// `facepivot1`.
pub const FACEPIVOT1: [usize; 12] = TABLES.facepivot1;
/// `facepivot2`.
pub const FACEPIVOT2: [[usize; 12]; 12] = TABLES.facepivot2;
/// `tsbondtbl`.
pub const TSBONDTBL: [[usize; 6]; 12] = TABLES.tsbondtbl;
/// `stbondtbl`.
pub const STBONDTBL: [[usize; 6]; 12] = TABLES.stbondtbl;
/// `tspivottbl`.
pub const TSPIVOTTBL: [[usize; 6]; 12] = TABLES.tspivottbl;
/// `stpivottbl`.
pub const STPIVOTTBL: [[usize; 6]; 12] = TABLES.stpivottbl;

// ---------------------------------------------------------------------------
// Handles.
// ---------------------------------------------------------------------------

/// `triface`: a tetrahedron and one of its twelve versions (an oriented
/// edge of an oriented face).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TriFace {
    /// The tetrahedron's pool slot, or [`NULL_TET`].
    pub tet: usize,
    /// The version, `0..12`.
    pub ver: usize,
}

impl Default for TriFace {
    /// `triface() : tet(0), ver(0)`.
    fn default() -> Self {
        TriFace {
            tet: NULL_TET,
            ver: 0,
        }
    }
}

/// `encode2(ptr, ver)`: compress a handle into one word.
#[inline]
pub fn encode2(tet: usize, ver: usize) -> Tetrahedron {
    ((tet + 1) << 4) | ver
}

/// `encode(t)`.
#[inline]
pub fn encode(t: &TriFace) -> Tetrahedron {
    encode2(t.tet, t.ver)
}

/// `decode(ptr, t)`: the version is the four low bits.
#[inline]
pub fn decode(ptr: Tetrahedron) -> TriFace {
    if ptr == NULL_HANDLE {
        return TriFace {
            tet: NULL_TET,
            ver: 0,
        };
    }
    TriFace {
        tet: (ptr >> 4) - 1,
        ver: ptr & 15,
    }
}

/// `decode_tet_only(ptr)`.
#[inline]
pub fn decode_tet_only(ptr: Tetrahedron) -> usize {
    if ptr == NULL_HANDLE {
        NULL_TET
    } else {
        (ptr >> 4) - 1
    }
}

/// `decode_ver_only(ptr)`.
#[inline]
pub fn decode_ver_only(ptr: Tetrahedron) -> usize {
    ptr & 15
}

/// `enextself(t)`: the next edge (counterclockwise) in the same face.
#[inline]
pub fn enextself(t: &mut TriFace) {
    t.ver = ENEXTTBL[t.ver];
}
/// `eprevself(t)`: the previous edge (clockwise) in the same face.
#[inline]
pub fn eprevself(t: &mut TriFace) {
    t.ver = EPREVTBL[t.ver];
}
/// `esymself(t)`: the reversed edge, in the other face of the same tet.
#[inline]
pub fn esymself(t: &mut TriFace) {
    t.ver = ESYMTBL[t.ver];
}
/// `enextesymself(t)`.
#[inline]
pub fn enextesymself(t: &mut TriFace) {
    t.ver = ENEXTESYMTBL[t.ver];
}
/// `eprevesymself(t)`.
#[inline]
pub fn eprevesymself(t: &mut TriFace) {
    t.ver = EPREVESYMTBL[t.ver];
}
/// `eorgoppoself(t)`.
#[inline]
pub fn eorgoppoself(t: &mut TriFace) {
    t.ver = EORGOPPOTBL[t.ver];
}
/// `edestoppoself(t)`.
#[inline]
pub fn edestoppoself(t: &mut TriFace) {
    t.ver = EDESTOPPOTBL[t.ver];
}
/// `enext(t1, t2)`.
#[inline]
pub fn enext(t: &TriFace) -> TriFace {
    TriFace {
        tet: t.tet,
        ver: ENEXTTBL[t.ver],
    }
}
/// `eprev(t1, t2)`.
#[inline]
pub fn eprev(t: &TriFace) -> TriFace {
    TriFace {
        tet: t.tet,
        ver: EPREVTBL[t.ver],
    }
}
/// `esym(t1, t2)`.
#[inline]
pub fn esym(t: &TriFace) -> TriFace {
    TriFace {
        tet: t.tet,
        ver: ESYMTBL[t.ver],
    }
}
/// `enextesym(t1, t2)`.
#[inline]
pub fn enextesym(t: &TriFace) -> TriFace {
    TriFace {
        tet: t.tet,
        ver: ENEXTESYMTBL[t.ver],
    }
}
/// `eprevesym(t1, t2)`.
#[inline]
pub fn eprevesym(t: &TriFace) -> TriFace {
    TriFace {
        tet: t.tet,
        ver: EPREVESYMTBL[t.ver],
    }
}

// ---------------------------------------------------------------------------
// Records.
// ---------------------------------------------------------------------------

/// A tetrahedron record: upstream's twelve words.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TetRecord {
    /// `[0..3]`: the neighbours at faces 0..3, encoded ([`NULL_HANDLE`] is
    /// "outer space").
    pub nbr: [Tetrahedron; 4],
    /// `[4..7]`: the vertices; `v[0] == NULL_POINT` marks a dead record.
    /// `v[3]` may be [`DUMMYPOINT`] (a hull tetrahedron).
    pub v: [Point; 4],
    /// `[8]`: the subsegment array (~~stage 3~~ stage 4 on); `usize::MAX` is `NULL`.
    pub seg: usize,
    /// `[9]`: the subface array (~~stage 3~~ stage 4 on); `usize::MAX` is `NULL`.
    pub sub: usize,
    /// `[10]`: reserved; the element index (`elemindex`) written by
    /// `outelements`.
    pub elemindex: i32,
    /// `[11]`: the element marker (`elemmarker`): bit 0 `infect`, bit 1
    /// `marktest`, bits 2..5 `markface`, bits 6..11 `markedge`, bit 12
    /// `marktest2`, bits 16.. the counter.
    pub marker: i32,
}

impl TetRecord {
    /// A blank record (what a fresh pool item holds before
    /// `maketetrahedron` writes it).
    pub const BLANK: TetRecord = TetRecord {
        nbr: [NULL_HANDLE; 4],
        v: [NULL_POINT; 4],
        seg: usize::MAX,
        sub: usize::MAX,
        elemindex: 0,
        marker: 0,
    };
}

/// `enum verttype`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum VertType {
    /// `UNUSEDVERTEX`.
    Unused,
    /// `DUPLICATEDVERTEX`.
    Duplicated,
    /// `RIDGEVERTEX`.
    Ridge,
    /// `FACETVERTEX`.
    Facet,
    /// `VOLVERTEX`.
    Vol,
    /// `FREESEGVERTEX`.
    FreeSeg,
    /// `FREEFACETVERTEX`.
    FreeFacet,
    /// `FREEVOLVERTEX`.
    FreeVol,
    /// `NREGULARVERTEX`.
    NRegular,
    /// `DEADVERTEX`.
    Dead,
}

/// A point record: coordinates and the words upstream appends to them.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct PointRecord {
    /// `pt[0..2]`.
    pub xyz: [f64; 3],
    /// `point2tet`: an encoded handle (it may carry a version: the four
    /// points of the first tetrahedron get `encode(firsttet)`).
    pub point2tet: Tetrahedron,
    /// `point2ppt`: the parent point (a duplicate's original).
    pub point2ppt: Point,
    /// `point2sh` (stage 3 on, `-p`/`-r` only); `usize::MAX` is `NULL`.
    pub point2sh: usize,
    /// `pointmark`: the point's index (from `in->firstnumber`).
    pub mark: i32,
    /// `pointtype`.
    pub vtype: VertType,
    /// The low eight flag bits of `[pointmarkindex + 1]` (`pinfect`, ...).
    pub flags: i32,
    /// `pointgeomtag` (`[pointmarkindex + 2]`): the local vertex index of
    /// `insert_vertex_bw`.
    pub geomtag: i32,
    /// `pointinsradius` (`useinsertradius`, `-p`; stage 6, gh:#1795): the
    /// insertion radius Delaunay refinement gives a Steiner point (0 until
    /// set; only `-q` and `-D` set it).
    pub insradius: f64,
}

impl PointRecord {
    /// A blank record.
    pub const BLANK: PointRecord = PointRecord {
        xyz: [0.0; 3],
        point2tet: NULL_HANDLE,
        point2ppt: NULL_POINT,
        point2sh: usize::MAX,
        mark: 0,
        vtype: VertType::Unused,
        flags: 0,
        geomtag: 0,
        insradius: 0.0,
    };
}

// ---------------------------------------------------------------------------
// Subfaces and subsegments (stage 3, gh:#1792).
// ---------------------------------------------------------------------------

/// Which pool a shell face lives in: upstream keeps subfaces and
/// subsegments in two `memorypool`s of the same record layout and tells
/// them apart only by where a pointer points.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ShPool {
    /// `subfaces`.
    Sub,
    /// `subsegs`.
    Seg,
}

/// A `NULL` shell-face slot (`face.sh == NULL`).
pub const NULL_SH: usize = usize::MAX;

/// An encoded shell-face pointer (upstream's `shellface`, a pointer with
/// the edge version `0..6` in its three low bits):
/// `((slot + 1) << 4) | (pool << 3) | shver`, with `pool` 1 for
/// [`ShPool::Seg`]; `0` is `NULL`.
pub type ShellFace = usize;

/// `face`: a subface or subsegment and its edge version `shver` (`0..6`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Face {
    /// The pool the record is in.
    pub pool: ShPool,
    /// The record's pool slot, or [`NULL_SH`].
    pub sh: usize,
    /// The edge version, `0..6`.
    pub shver: usize,
}

impl Default for Face {
    /// `face() : sh(0), shver(0)`.
    fn default() -> Self {
        Face {
            pool: ShPool::Sub,
            sh: NULL_SH,
            shver: 0,
        }
    }
}

impl Face {
    /// `s.sh == NULL`.
    #[inline]
    pub fn is_null(&self) -> bool {
        self.sh == NULL_SH
    }
    /// `s1.sh == s2.sh` (the same record, any version).
    #[inline]
    pub fn same_sh(&self, o: &Face) -> bool {
        self.sh == o.sh && (self.sh == NULL_SH || self.pool == o.pool)
    }
}

/// `sencode(s)` (and `sencode2`).
#[inline]
pub fn sencode(s: &Face) -> ShellFace {
    if s.sh == NULL_SH {
        return 0;
    }
    let pool = match s.pool {
        ShPool::Sub => 0,
        ShPool::Seg => 1,
    };
    ((s.sh + 1) << 4) | (pool << 3) | s.shver
}

/// `sdecode(sptr, s)`.
#[inline]
pub fn sdecode(sptr: ShellFace) -> Face {
    if sptr == 0 {
        return Face::default();
    }
    Face {
        pool: if (sptr >> 3) & 1 == 1 {
            ShPool::Seg
        } else {
            ShPool::Sub
        },
        sh: (sptr >> 4) - 1,
        shver: sptr & 7,
    }
}

/// A subface or subsegment record: upstream's eleven pointers plus the
/// integers after them (`initializepools`, `shsize`).
///
/// - `[0..2]` [`ShellRecord::nbr`]: the adjacent subfaces at edges 0..2
///   (a subsegment's `[0]` is a subface containing it, `ssbond`);
/// - `[3..5]` [`ShellRecord::v`]: the vertices (`v[2]` is `NULL` for a
///   subsegment; `v[0] == NULL_POINT` marks a dead record);
/// - `[6..8]` [`ShellRecord::seg`]: the subsegments at edges 0..2;
/// - `[9..10]` [`ShellRecord::tet`]: the adjacent tetrahedra (stage 4);
/// - [`ShellRecord::marker`] (`shellmark`), [`ShellRecord::flags`]
///   (`sinfect` bit 0, `smarktest` bit 1, `smarktest2` bit 2) and
///   [`ShellRecord::facetindex`] (`useinsertradius`, `-p`).
///
/// The area bound (`checkconstraints`, only with `-q` and a facet or
/// segment constraint list) is not ported.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ShellRecord {
    /// `sh[0..2]`.
    pub nbr: [ShellFace; 3],
    /// `sh[3..5]`.
    pub v: [Point; 3],
    /// `sh[6..8]`.
    pub seg: [ShellFace; 3],
    /// `sh[9..10]`.
    pub tet: [Tetrahedron; 2],
    /// `shellmark`.
    pub marker: i32,
    /// `[shmarkindex + 1]`.
    pub flags: i32,
    /// `[shmarkindex + 2]` (`getfacetindex`).
    pub facetindex: i32,
    /// A subsegment's `sh[6]` written by `mergefacets` as a pointer to the
    /// dihedral-angle cosine it caches there (`segloop.sh[6] = paryang`).
    /// Upstream frees the array but leaves the pointer: after
    /// `meshsurface` a segment with `Some` still has a non-`NULL` `sh[6]`.
    pub dihedang: Option<f64>,
}

impl ShellRecord {
    /// What `makeshellface` writes.
    pub const BLANK: ShellRecord = ShellRecord {
        nbr: [0; 3],
        v: [NULL_POINT; 3],
        seg: [0; 3],
        tet: [NULL_HANDLE; 2],
        marker: 0,
        flags: 0,
        facetindex: 0,
        dihedang: None,
    };
}

/// A `badface` on `flipstack` as `flipshpush` fills it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct FlipShItem {
    /// `ss`.
    pub ss: Face,
    /// `forg`.
    pub forg: Point,
    /// `fdest`.
    pub fdest: Point,
}

// ---------------------------------------------------------------------------
// memorypool.
// ---------------------------------------------------------------------------

/// `memorypool`: the allocation bookkeeping of a pool whose items live in a
/// separate `Vec` indexed by slot. Slot `s` is item `s % itemsperblock` of
/// block `s / itemsperblock`, so the slot order is upstream's traversal
/// order.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MemoryPool {
    /// `itemsperblock`.
    pub itemsperblock: usize,
    /// `items`: the number of live items.
    pub items: usize,
    /// `maxitems`: the number of slots ever handed out (`nextitem`).
    pub maxitems: usize,
    /// `deaditemstack`: freed slots, top of the stack last.
    pub deaditemstack: Vec<usize>,
    /// `pathitem`: the next slot `traverse` returns.
    pub pathitem: usize,
}

impl MemoryPool {
    /// `poolinit(bytecount, itemcount, ...)` followed by `restart()`.
    pub fn new(itemsperblock: usize) -> Self {
        MemoryPool {
            itemsperblock,
            items: 0,
            maxitems: 0,
            deaditemstack: Vec::new(),
            pathitem: 0,
        }
    }

    /// `alloc()`: the most recently freed slot if any, else a fresh one.
    /// Returns `(slot, fresh)`.
    pub fn alloc(&mut self) -> (usize, bool) {
        let r = if let Some(s) = self.deaditemstack.pop() {
            (s, false)
        } else {
            let s = self.maxitems;
            self.maxitems += 1;
            (s, true)
        };
        self.items += 1;
        r
    }

    /// `dealloc(dyingitem)`: push the slot on the dead-item stack.
    pub fn dealloc(&mut self, slot: usize) {
        self.deaditemstack.push(slot);
        self.items -= 1;
    }

    /// `traversalinit()`.
    pub fn traversalinit(&mut self) {
        self.pathitem = 0;
    }

    /// `traverse()`: every slot in allocation order, dead ones included;
    /// `None` at the end (`pathitem == nextitem`).
    pub fn traverse(&mut self) -> Option<usize> {
        if self.pathitem == self.maxitems {
            return None;
        }
        let s = self.pathitem;
        self.pathitem += 1;
        Some(s)
    }
}

// ---------------------------------------------------------------------------
// glibc rand().
// ---------------------------------------------------------------------------

/// glibc's `rand()` / `srand()` (`TYPE_3`: degree 31, separation 3, an
/// additive feedback generator seeded by the Park-Miller LCG with 310
/// discarded outputs).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GlibcRand {
    state: [u32; 31],
    fptr: usize,
    rptr: usize,
}

impl GlibcRand {
    /// The state after `srand(seed)`.
    pub fn new(seed: u32) -> Self {
        let mut r = GlibcRand {
            state: [0; 31],
            fptr: 3,
            rptr: 0,
        };
        r.srand(seed);
        r
    }

    /// `srand(seed)`.
    pub fn srand(&mut self, seed: u32) {
        let seed = if seed == 0 { 1 } else { seed };
        self.state[0] = seed;
        let mut word: i64 = seed as i32 as i64;
        for i in 1..31 {
            // word = 16807 * word % 2147483647, without overflow.
            let hi = word / 127773;
            let lo = word % 127773;
            word = 16807 * lo - 2836 * hi;
            if word < 0 {
                word += 2147483647;
            }
            self.state[i] = word as u32;
        }
        self.fptr = 3;
        self.rptr = 0;
        for _ in 0..310 {
            self.rand();
        }
    }

    /// `rand()`: a value in `0..=RAND_MAX` (`2^31 - 1`).
    pub fn rand(&mut self) -> i32 {
        let v = self.state[self.fptr].wrapping_add(self.state[self.rptr]);
        self.state[self.fptr] = v;
        let result = (v >> 1) as i32;
        self.fptr += 1;
        if self.fptr >= 31 {
            self.fptr = 0;
            self.rptr += 1;
        } else {
            self.rptr += 1;
            if self.rptr >= 31 {
                self.rptr = 0;
            }
        }
        result
    }
}

// ---------------------------------------------------------------------------
// tetgenbehavior (the options stage 2 reads).
// ---------------------------------------------------------------------------

/// The `tetgenbehavior` switches the point-set Delaunay path reads, with
/// upstream's defaults ([`Behavior::default`]).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Behavior {
    /// `-b`: BRIO + Hilbert sorting (default 1). `-b0` turns it off, and
    /// then each point location starts from `randomsample`.
    pub brio_hilbert: bool,
    /// `-b/1`: insert in the input order (default 0).
    pub no_sort: bool,
    /// `-b///`: the maximum Hilbert order (default 52).
    pub hilbert_order: i32,
    /// `-b//`: sort a sub-box with more than this many points (default 8).
    pub hilbert_limit: i32,
    /// `-b`: the BRIO round threshold (default 64).
    pub brio_threshold: i32,
    /// `-b/`: the BRIO round ratio (default 0.125).
    pub brio_ratio: f64,
    /// `-T`: the tolerance (default 1e-8).
    pub epsilon: f64,
    /// `-X`: no exact arithmetic.
    pub noexact: bool,
    /// `-X1`: no static filter.
    pub nostaticfilter: bool,
    /// `-J`: do not jettison duplicated and unused vertices.
    pub nojettison: bool,
    /// `-x`: points per pool block (default 4092).
    pub vertexperblock: usize,
    /// `-x`: tetrahedra per pool block (default 8188).
    pub tetrahedraperblock: usize,
    /// `-p`: the input is a piecewise linear complex (stage 3 on, gh:#1792).
    /// Set by `.poly` / `.smesh` input; adds the subface and subsegment
    /// pools and `useinsertradius`.
    pub plc: bool,
    /// `-p` (first value): two facets meeting at a dihedral angle above
    /// this (degrees) are coplanar and merged (default 179.9).
    pub facet_separate_ang_tol: f64,
    /// `-p//`: the small angle (degrees) between two segments at a vertex
    /// below which `mergefacets` removes one (default 15.0).
    pub facet_small_ang_tol: f64,
    /// `-M`: do not merge coplanar facets (default off).
    pub nomergefacet: bool,
    /// `-Y`: do not split boundary (default 0).
    pub nobisect: i32,
    /// `-d`: diagnose (default off).
    pub diagnose: bool,
    /// `-x`: subfaces and subsegments per pool block (default 4092).
    pub shellfaceperblock: usize,
    /// `fliplinklevel` (default -1: automatic, `autofliplinklevel`).
    /// Boundary recovery changes it while it runs, as upstream does.
    pub fliplinklevel: i32,
    /// `flipstarsize` (default -1: no limit).
    pub flipstarsize: i32,
    /// `fliplinklevelinc` (default 1).
    pub fliplinklevelinc: i32,
    /// `-p/`: `collinear_ang_tol` in degrees (default 179.9).
    pub collinear_ang_tol: f64,
    /// `-Y/`: `supsteiner_level` (default 2).
    pub supsteiner_level: i32,
    /// `-S`: `steinerleft` (default -1: no limit).
    pub steinerleft: i64,
    // -- carveholes, smoothing and mesh improvement (stage 5, gh:#1794) ---
    /// `-A`: `regionattrib`, give every tetrahedron its region's attribute
    /// (default 0). With it each tetrahedron carries one attribute
    /// (`numelemattrib`; a PLC has no input tetrahedron attributes).
    pub regionattrib: bool,
    /// `-O#`: `opt_max_flip_level` (default 3).
    pub opt_max_flip_level: i32,
    /// `-O//#`: `opt_iterations` (default 3).
    pub opt_iterations: i32,
    /// `-o//#`: `opt_max_asp_ratio` (default 1000).
    pub opt_max_asp_ratio: f64,
    /// `opt_max_edge_ratio` (default 100).
    pub opt_max_edge_ratio: f64,
    /// `-o/#`: `optmaxdihedral` in degrees (default 177; without `-q`
    /// `parse_commandline` makes the default 179.9, which
    /// [`TetgenMesh::finish_plc`] applies).
    pub optmaxdihedral: f64,
    /// `-s#`: `smooth_cirterion` (upstream's spelling; default 3: facet and
    /// volume Steiner points).
    pub smooth_cirterion: i32,
    /// `-s#/#`: `smooth_maxiter` (default 7).
    pub smooth_maxiter: i32,
    /// `-s#/#/#`: `smooth_alpha` (default 0.3).
    pub smooth_alpha: f64,
    /// `-U#`: `unflip_queue_limit` (default 1000).
    pub unflip_queue_limit: i64,
    /// `-D#`: `cdtrefine` (default 7; read by mesh improvement's
    /// `noflip_in_surface`).
    pub cdtrefine: i32,
    /// `-z`: number the output from zero (default off: from
    /// `in->firstnumber`).
    pub zeroindex: bool,
    // -- Delaunay refinement (stage 6, gh:#1795) ---------------------------
    /// `-q`: `quality`, refine the mesh (`delaunayrefinement`). With it
    /// `optmaxdihedral` keeps its 177-degree default; `-a#` sets it too
    /// (`parse_commandline`, applied by [`TetgenMesh::finish_plc`]).
    pub quality: bool,
    /// `-q#`: `minratio`, the radius-edge ratio bound (default 2.0).
    pub minratio: f64,
    /// `-q/#`: `mindihedral`, the dihedral-angle bound in degrees (default
    /// 3.5).
    pub mindihedral: f64,
    /// `-a#`: `fixedvolume`, a volume bound on every tetrahedron.
    pub fixedvolume: bool,
    /// `-a#`: `maxvolume` (default -1).
    pub maxvolume: f64,
    /// `-a#`: `maxvolume_length = pow(maxvolume, 1/3) / 3` (default -1).
    pub maxvolume_length: f64,
}

impl Default for Behavior {
    fn default() -> Self {
        Behavior {
            brio_hilbert: true,
            no_sort: false,
            hilbert_order: 52,
            hilbert_limit: 8,
            brio_threshold: 64,
            brio_ratio: 0.125,
            epsilon: 1.0e-8,
            noexact: false,
            nostaticfilter: false,
            nojettison: false,
            vertexperblock: 4092,
            tetrahedraperblock: 8188,
            plc: false,
            facet_separate_ang_tol: 179.9,
            facet_small_ang_tol: 15.0,
            nomergefacet: false,
            nobisect: 0,
            diagnose: false,
            shellfaceperblock: 4092,
            fliplinklevel: -1,
            flipstarsize: -1,
            fliplinklevelinc: 1,
            collinear_ang_tol: 179.9,
            supsteiner_level: 2,
            steinerleft: -1,
            regionattrib: false,
            opt_max_flip_level: 3,
            opt_iterations: 3,
            opt_max_asp_ratio: 1000.0,
            opt_max_edge_ratio: 100.0,
            optmaxdihedral: 177.0,
            smooth_cirterion: 3,
            smooth_maxiter: 7,
            smooth_alpha: 0.3,
            unflip_queue_limit: 1000,
            cdtrefine: 7,
            zeroindex: false,
            quality: false,
            minratio: 2.0,
            mindihedral: 3.5,
            fixedvolume: false,
            maxvolume: -1.0,
            maxvolume_length: -1.0,
        }
    }
}

// ---------------------------------------------------------------------------
// The mesh object.
// ---------------------------------------------------------------------------

/// `class tetgenmesh`: the fields the point-set Delaunay stage uses. Later
/// stages (gh:#1792-#1795) extend this struct (subface and subsegment pools,
/// the flip and cavity lists of `insertpoint`, ...).
#[derive(Debug, Clone)]
pub struct TetgenMesh {
    /// `b`: the switches.
    pub b: Behavior,
    /// `in->firstnumber`: 0 here (the input is a slice).
    pub firstnumber: i32,
    /// `in->numberofpoints`.
    pub numberofpoints: usize,
    /// The point records; index 0 is `dummypoint`, index `s + 1` pool slot
    /// `s`.
    pub points: Vec<PointRecord>,
    /// `points` (the pool bookkeeping).
    pub pointpool: MemoryPool,
    /// The tetrahedron records, by pool slot.
    pub tets: Vec<TetRecord>,
    /// `tetrahedrons` (the pool bookkeeping).
    pub tetpool: MemoryPool,
    /// The predicates' constants (`exactinit`, called by `transfernodes`).
    pub pred: Predicates,
    /// `rand()`'s state (glibc).
    pub crand: GlibcRand,
    /// `randomseed` (for `randomnation`), initially 1.
    pub randomseed: u64,
    /// `samples` (for `randomsample`), initially 0.
    pub samples: i64,
    /// `recenttet`.
    pub recenttet: TriFace,
    /// `hullsize`: the number of hull tetrahedra.
    pub hullsize: i64,
    /// `dupverts`.
    pub dupverts: i64,
    /// `unuverts`.
    pub unuverts: i64,
    /// `nonregularcount`.
    pub nonregularcount: i64,
    /// `nonconvex`.
    pub nonconvex: bool,
    /// `xmin`, `xmax`, ... the bounding box.
    pub xmin: f64,
    /// `xmax`.
    pub xmax: f64,
    /// `ymin`.
    pub ymin: f64,
    /// `ymax`.
    pub ymax: f64,
    /// `zmin`.
    pub zmin: f64,
    /// `zmax`.
    pub zmax: f64,
    /// `longest`: the bounding box diagonal.
    pub longest: f64,
    /// `minedgelength`: `longest * b->epsilon`.
    pub minedgelength: f64,
    /// `transgc[8][3][8]` (Hilbert curve Gray-code permutations).
    pub transgc: [[[i32; 8]; 3]; 8],
    /// `tsb1mod3[8]`.
    pub tsb1mod3: [i32; 8],
    /// `cave_oldtet_list` (an `arraypool` of tetrahedra).
    pub cave_oldtet_list: Vec<usize>,
    /// `cavebdrylist` (an `arraypool` of trifaces).
    pub cavebdrylist: Vec<TriFace>,
    /// `_bw_faces[64][64]` (persistent scratch of `insert_vertex_bw`).
    pub bw_faces: Vec<TriFace>,
    /// The subface records (stage 3, gh:#1792), by pool slot.
    pub subfaces: Vec<ShellRecord>,
    /// `subfaces` (the pool bookkeeping).
    pub subfacepool: MemoryPool,
    /// The subsegment records, by pool slot.
    pub subsegs: Vec<ShellRecord>,
    /// `subsegs` (the pool bookkeeping).
    pub subsegpool: MemoryPool,
    /// `recentsh`: a handle for surface point location.
    pub recentsh: Face,
    /// `flipstack` (the `flippool` linked list), top last.
    pub flipstack: Vec<FlipShItem>,
    /// `caveshlist`.
    pub caveshlist: Vec<Face>,
    /// `caveshbdlist`.
    pub caveshbdlist: Vec<Face>,
    /// `cavesegshlist`.
    pub cavesegshlist: Vec<Face>,
    /// `caveencshlist`.
    pub caveencshlist: Vec<Face>,
    /// `caveencseglist`.
    pub caveencseglist: Vec<Face>,
    /// `minfacetdihed`: the smallest input dihedral angle (radians),
    /// initially `PI`.
    pub minfacetdihed: f64,
    /// `insegments`: the number of input segments (after `meshsurface`).
    pub insegments: i64,
    /// `flip22count`.
    pub flip22count: i64,
    /// The warnings upstream prints during `meshsurface` (skipped facets,
    /// invalid vertices, ...), in order.
    pub warnings: Vec<alloc::string::String>,
    // -- boundary recovery (stage 4, gh:#1793) ------------------------------
    /// The tet-to-segment arrays (`tet2segpool` items: six encoded
    /// subsegment pointers, one per edge), by pool slot; a tetrahedron's
    /// [`TetRecord::seg`] is a slot here.
    pub tet2seg: Vec<[ShellFace; 6]>,
    /// `tet2segpool` (the pool bookkeeping).
    pub tet2segpool: MemoryPool,
    /// The tet-to-subface arrays (`tet2subpool` items: four encoded subface
    /// pointers, one per face), by pool slot ([`TetRecord::sub`]).
    pub tet2sub: Vec<[ShellFace; 4]>,
    /// `tet2subpool` (the pool bookkeeping).
    pub tet2subpool: MemoryPool,
    /// `checksubsegflag`: flips keep the tet-segment links.
    pub checksubsegflag: bool,
    /// `checksubfaceflag`: flips keep the tet-subface links.
    pub checksubfaceflag: bool,
    /// `boundary_recovery_flag`.
    pub boundary_recovery_flag: bool,
    /// `cos_collinear_ang_tol`.
    pub cos_collinear_ang_tol: f64,
    /// `segmentendpointslist` (two points per segment).
    pub segmentendpointslist: Vec<Point>,
    /// `idx_segment_ridge_vertex_list`.
    pub idx_segment_ridge_vertex_list: Vec<i32>,
    /// `segment_ridge_vertex_list`.
    pub segment_ridge_vertex_list: Vec<Point>,
    /// `subsegstack` (an `arraypool` used as a stack).
    pub subsegstack: Vec<Face>,
    /// `subfacstack`.
    pub subfacstack: Vec<Face>,
    /// `subvertstack`: the Steiner points to be removed later.
    pub subvertstack: Vec<Point>,
    /// `autofliplinklevel`.
    pub autofliplinklevel: i32,
    /// `st_segref_count`: Steiner points on segments.
    pub st_segref_count: i64,
    /// `st_facref_count`: Steiner points on facets.
    pub st_facref_count: i64,
    /// `st_volref_count`: Steiner points in the volume.
    pub st_volref_count: i64,
    /// `steinerleft` (the mesh's, initially -1).
    pub steinerleft: i64,
    /// `flip23count`.
    pub flip23count: i64,
    /// `flip32count`.
    pub flip32count: i64,
    /// `flip41count`.
    pub flip41count: i64,
    /// `flip31count`.
    pub flip31count: i64,
    /// `flip14count`.
    pub flip14count: i64,
    /// `flip26count`.
    pub flip26count: i64,
    /// `flipn2ncount`.
    pub flipn2ncount: i64,
    /// `cavetetlist`.
    pub cavetetlist: Vec<TriFace>,
    /// `duplicated_facets_count`.
    pub duplicated_facets_count: i64,
    /// `skipped_segment_list` (segments that self-intersect).
    pub skipped_segment_list: Vec<Face>,
    /// `skipped_facet_list` (subfaces that self-intersect); upstream keeps
    /// it as `NULL` until the first one.
    pub skipped_facet_list: Option<Vec<Face>>,
    /// The tetrahedral flip stack (`flippush`'s `badface` items), top last.
    /// Upstream keeps these and `flipshpush`'s on one linked list
    /// (`flipstack`); the two are never non-empty at once on the paths
    /// ported here.
    pub tflipstack: Vec<TriFace>,
    /// `caveoldtetlist` (`insertpoint`).
    pub caveoldtetlist: Vec<TriFace>,
    /// `cavetetshlist`.
    pub cavetetshlist: Vec<Face>,
    /// `cavetetseglist`.
    pub cavetetseglist: Vec<Face>,
    /// `cavetetvertlist`.
    pub cavetetvertlist: Vec<Point>,
    /// `later_unflip_queue` (`recoverdelaunay`): `(tt, forg, fdest, fapex)`.
    pub later_unflip_queue: Vec<(TriFace, Point, Point, Point)>,
    /// `tetprism_vol_sum`.
    pub tetprism_vol_sum: f64,
    /// `flip44count`.
    pub flip44count: i64,
    /// `recover_delaunay_count`.
    pub recover_delaunay_count: i64,
    /// Per-routine call counts of the recovery code (the `tg_reach`
    /// instrumentation of `verification_and_validation/tetgen_recoverboundary_gh1793/`).
    pub reach: [u64; 64],
    // -- carveholes, smoothing and mesh improvement (stage 5, gh:#1794) ---
    /// `numelemattrib`: 0, or 1 with `-A` (the region attribute).
    pub numelemattrib: usize,
    /// The element attribute (`elemattribute(tet, 0)`), by tetrahedron pool
    /// slot; read only when `numelemattrib == 1`.
    pub elemattrib: Vec<f64>,
    /// `in->pointmarkerlist` (copied from the input by the stage 5 driver;
    /// `jettisonnodes` re-indexes it in place as upstream does).
    pub pointmarkerlist: Option<Vec<i32>>,
    /// `badtetrahedrons`: the tetrahedra `enqueuetetrahedron` queued
    /// (`chkencflag & 4`); `None` is upstream's `NULL` (outside
    /// `improve_mesh`).
    pub badtetrahedrons: Option<Vec<TriFace>>,
    /// `cos_large_dihed` (`get_tetqual`'s 135-degree count).
    pub cos_large_dihed: f64,
    /// `cosmaxdihed`.
    pub cosmaxdihed: f64,
    /// `cossmtdihed`.
    pub cossmtdihed: f64,
    /// `cosslidihed`.
    pub cosslidihed: f64,
    /// `opt_max_sliver_asp_ratio`.
    pub opt_max_sliver_asp_ratio: f64,
    /// `opt_flips_count`.
    pub opt_flips_count: i64,
    /// `opt_collapse_count`.
    pub opt_collapse_count: i64,
    /// `opt_smooth_count`.
    pub opt_smooth_count: i64,
    /// `subdomains` (set by `carveholes` with `-A`).
    pub subdomains: i32,
    /// `subdomain_markers`.
    pub subdomain_markers: Vec<i32>,
    /// Not upstream: the volume of the hull slivers `lawsonflip3d` peeled
    /// after `carveholes` (each `flip32` there removes one tetrahedron from
    /// the domain and flips two boundary triangles), so that a volume check
    /// can account for it (gh:#1794; only `sphere -p` peels any).
    pub hull_sliver_volume: f64,
    /// Per-routine call counts of stage 5 (the `tg5` instrumentation of
    /// `verification_and_validation/tetgen_plc_gh1794/`).
    pub reach5: [u64; 64],
    // -- Delaunay refinement (stage 6, gh:#1795; `super::refine`) -------
    /// The working state of `delaunayrefinement` (its pools, queues, the
    /// facet and segment maps). Upstream keeps these as `tetgenmesh`
    /// members that are `NULL` outside refinement; here they are one
    /// struct, default-empty.
    pub rf: super::refine::RefineState,
    /// Per-routine and per-branch call counts of stage 6 (the `tg6`
    /// instrumentation of `verification_and_validation/tetgen_quality_gh1795/`).
    pub reach6: [u64; 64],
}

impl TetgenMesh {
    /// `tetgenmesh()` + `initializepools()`: an empty mesh with the
    /// `dummypoint` record. The derived record sizes of `initializepools`
    /// (`pointmtrindex`, `elemmarkerindex`, ...) are not needed: the records
    /// are typed structs.
    pub fn new(b: Behavior) -> Self {
        let mut dummy = PointRecord::BLANK;
        // setpointmark(dummypoint, -1): the unique marker for dummypoint.
        dummy.mark = -1;
        dummy.vtype = VertType::Unused;
        let mut points = Vec::new();
        points.push(dummy);
        TetgenMesh {
            b,
            firstnumber: 0,
            numberofpoints: 0,
            points,
            pointpool: MemoryPool::new(b.vertexperblock),
            tets: Vec::new(),
            tetpool: MemoryPool::new(b.tetrahedraperblock),
            pred: Predicates::exactinit(b.noexact, b.nostaticfilter, 0.0, 0.0, 0.0),
            crand: GlibcRand::new(1),
            randomseed: 1,
            samples: 0,
            recenttet: TriFace::default(),
            hullsize: 0,
            dupverts: 0,
            unuverts: 0,
            nonregularcount: 0,
            nonconvex: false,
            xmin: 0.0,
            xmax: 0.0,
            ymin: 0.0,
            ymax: 0.0,
            zmin: 0.0,
            zmax: 0.0,
            longest: 0.0,
            minedgelength: 0.0,
            transgc: [[[0; 8]; 3]; 8],
            tsb1mod3: [0; 8],
            cave_oldtet_list: Vec::new(),
            cavebdrylist: Vec::new(),
            bw_faces: alloc::vec![TriFace::default(); 4096],
            subfaces: Vec::new(),
            subfacepool: MemoryPool::new(b.shellfaceperblock),
            subsegs: Vec::new(),
            subsegpool: MemoryPool::new(b.shellfaceperblock),
            recentsh: Face::default(),
            flipstack: Vec::new(),
            caveshlist: Vec::new(),
            caveshbdlist: Vec::new(),
            cavesegshlist: Vec::new(),
            caveencshlist: Vec::new(),
            caveencseglist: Vec::new(),
            minfacetdihed: core::f64::consts::PI,
            insegments: 0,
            flip22count: 0,
            warnings: Vec::new(),
            tet2seg: Vec::new(),
            tet2segpool: MemoryPool::new(b.shellfaceperblock),
            tet2sub: Vec::new(),
            tet2subpool: MemoryPool::new(b.shellfaceperblock),
            checksubsegflag: false,
            checksubfaceflag: false,
            boundary_recovery_flag: false,
            cos_collinear_ang_tol: (179.9 / 180.0 * core::f64::consts::PI).cos(),
            segmentendpointslist: Vec::new(),
            idx_segment_ridge_vertex_list: Vec::new(),
            segment_ridge_vertex_list: Vec::new(),
            subsegstack: Vec::new(),
            subfacstack: Vec::new(),
            subvertstack: Vec::new(),
            autofliplinklevel: 1,
            st_segref_count: 0,
            st_facref_count: 0,
            st_volref_count: 0,
            steinerleft: -1,
            flip23count: 0,
            flip32count: 0,
            flip41count: 0,
            flip31count: 0,
            flip14count: 0,
            flip26count: 0,
            flipn2ncount: 0,
            cavetetlist: Vec::new(),
            duplicated_facets_count: 0,
            skipped_segment_list: Vec::new(),
            skipped_facet_list: None,
            tflipstack: Vec::new(),
            caveoldtetlist: Vec::new(),
            cavetetshlist: Vec::new(),
            cavetetseglist: Vec::new(),
            cavetetvertlist: Vec::new(),
            later_unflip_queue: Vec::new(),
            tetprism_vol_sum: 0.0,
            flip44count: 0,
            recover_delaunay_count: 0,
            reach: [0; 64],
            numelemattrib: 0,
            elemattrib: Vec::new(),
            pointmarkerlist: None,
            badtetrahedrons: None,
            cos_large_dihed: 0.0,
            cosmaxdihed: 0.0,
            cossmtdihed: 0.0,
            cosslidihed: 0.0,
            opt_max_sliver_asp_ratio: 0.0,
            opt_flips_count: 0,
            opt_collapse_count: 0,
            opt_smooth_count: 0,
            subdomains: 0,
            subdomain_markers: Vec::new(),
            hull_sliver_volume: 0.0,
            reach5: [0; 64],
            rf: super::refine::RefineState::default(),
            reach6: [0; 64],
        }
    }

    // -- point primitives ---------------------------------------------------

    /// The coordinates of `p`.
    #[inline]
    pub fn xyz(&self, p: Point) -> &[f64; 3] {
        &self.points[p].xyz
    }
    /// `pointmark(p)`.
    #[inline]
    pub fn pointmark(&self, p: Point) -> i32 {
        self.points[p].mark
    }
    /// `pointtype(p)`.
    #[inline]
    pub fn pointtype(&self, p: Point) -> VertType {
        self.points[p].vtype
    }
    /// `setpointtype(p, t)`.
    #[inline]
    pub fn setpointtype(&mut self, p: Point, t: VertType) {
        self.points[p].vtype = t;
    }
    /// `point2tet(p)`.
    #[inline]
    pub fn point2tet(&self, p: Point) -> Tetrahedron {
        self.points[p].point2tet
    }
    /// `setpoint2tet(p, t)`.
    #[inline]
    pub fn setpoint2tet(&mut self, p: Point, t: Tetrahedron) {
        self.points[p].point2tet = t;
    }

    /// `makepoint(&p, vtype)`: allocate a point record from the pool.
    pub fn makepoint(&mut self, vtype: VertType) -> Point {
        let (slot, fresh) = self.pointpool.alloc();
        let p = slot + 1;
        if fresh {
            self.points.push(PointRecord::BLANK);
        }
        let mut r = PointRecord::BLANK;
        // Initialize the point marker (starting from in->firstnumber).
        r.mark = self.pointpool.items as i32 - (self.firstnumber == 0) as i32;
        r.flags = 0;
        r.vtype = vtype;
        self.points[p] = r;
        p
    }

    /// `pointdealloc(p)`: mark the point dead and free its slot.
    pub fn pointdealloc(&mut self, p: Point) {
        self.setpointtype(p, VertType::Dead);
        self.pointpool.dealloc(p - 1);
    }

    /// `pointtraverse()`: the next live point in pool order.
    pub fn pointtraverse(&mut self) -> Option<Point> {
        loop {
            let s = self.pointpool.traverse()?;
            if self.points[s + 1].vtype != VertType::Dead {
                return Some(s + 1);
            }
        }
    }

    // -- tetrahedron primitives --------------------------------------------

    /// `maketetrahedron2(&newtet, pa, pb, pc, pd)`; `newtet.ver = 11`.
    pub fn maketetrahedron2(&mut self, pa: Point, pb: Point, pc: Point, pd: Point) -> TriFace {
        let (slot, fresh) = self.tetpool.alloc();
        if fresh {
            self.tets.push(TetRecord::BLANK);
            self.elemattrib.push(0.0);
        }
        let mut r = TetRecord::BLANK;
        r.v = [pa, pb, pc, pd];
        // No volume bound (`-aa`, `varvolume`; ~~`-a`, stage 6~~ stage 6 ports
        //   only `-a#`, which needs none, gh:#1795).
        r.marker = 0;
        // ~~The reserved word [10] is not cleared by upstream; it keeps
        // whatever the slot held (only `outelements` writes it).~~
        // **CORRECTED 2026-11-26 (gh:#1794):** upstream does clear it
        // (`newtet->tet[10] = NULL; // used by mesh improvement`, in both
        // `maketetrahedron` and `maketetrahedron2`, `tetgen.cxx:4682/4719`);
        // only `outelements` reads it, after writing it, so no output
        // changed.
        r.elemindex = 0;
        self.tets[slot] = r;
        // `setelemattribute(newtet, i, 0.0)` for every attribute.
        self.elemattrib[slot] = 0.0;
        TriFace { tet: slot, ver: 11 }
    }

    /// `maketetrahedron(&newtet)`: a record with four `NULL` vertices.
    pub fn maketetrahedron(&mut self) -> TriFace {
        self.maketetrahedron2(NULL_POINT, NULL_POINT, NULL_POINT, NULL_POINT)
    }

    /// `tetrahedrondealloc(t)`: set `v[0]` to `NULL` and free the slot.
    pub fn tetrahedrondealloc(&mut self, tet: usize) {
        self.tets[tet].v[0] = NULL_POINT;
        // Dealloc the space to subfaces/subsegments (stage 4 on; `[8]` and
        // `[9]` are `NULL` before boundary recovery). Upstream leaves the
        // two words as they were in the dead record.
        if self.tets[tet].seg != usize::MAX {
            self.tet2segpool.dealloc(self.tets[tet].seg);
        }
        if self.tets[tet].sub != usize::MAX {
            self.tet2subpool.dealloc(self.tets[tet].sub);
        }
        self.tetpool.dealloc(tet);
    }

    /// `tetrahedrontraverse()`: the next live, non-hull tetrahedron.
    pub fn tetrahedrontraverse(&mut self) -> Option<usize> {
        loop {
            let s = self.tetpool.traverse()?;
            let r = &self.tets[s];
            if r.v[0] != NULL_POINT && r.v[3] != DUMMYPOINT {
                return Some(s);
            }
        }
    }

    /// `alltetrahedrontraverse()`: the next live tetrahedron (hull ones
    /// included).
    pub fn alltetrahedrontraverse(&mut self) -> Option<usize> {
        loop {
            let s = self.tetpool.traverse()?;
            if self.tets[s].v[0] != NULL_POINT {
                return Some(s);
            }
        }
    }

    /// `org(t)`.
    #[inline]
    pub fn org(&self, t: &TriFace) -> Point {
        self.tets[t.tet].v[ORGPIVOT[t.ver] - 4]
    }
    /// `dest(t)`.
    #[inline]
    pub fn dest(&self, t: &TriFace) -> Point {
        self.tets[t.tet].v[DESTPIVOT[t.ver] - 4]
    }
    /// `apex(t)`.
    #[inline]
    pub fn apex(&self, t: &TriFace) -> Point {
        self.tets[t.tet].v[APEXPIVOT[t.ver] - 4]
    }
    /// `oppo(t)`.
    #[inline]
    pub fn oppo(&self, t: &TriFace) -> Point {
        self.tets[t.tet].v[OPPOPIVOT[t.ver] - 4]
    }

    /// `bond(t1, t2)`: glue two tetrahedra at the same face and edge.
    #[inline]
    pub fn bond(&mut self, t1: &TriFace, t2: &TriFace) {
        self.tets[t1.tet].nbr[t1.ver & 3] = encode2(t2.tet, BONDTBL[t1.ver][t2.ver]);
        self.tets[t2.tet].nbr[t2.ver & 3] = encode2(t1.tet, BONDTBL[t2.ver][t1.ver]);
    }

    /// `fsym(t1, t2)`: the adjacent tetrahedron at the same face and edge.
    #[inline]
    pub fn fsym(&self, t: &TriFace) -> TriFace {
        let mut t2 = decode(self.tets[t.tet].nbr[t.ver & 3]);
        t2.ver = FSYMTBL[t.ver][t2.ver];
        t2
    }
    /// `fsymself(t)`.
    #[inline]
    pub fn fsymself(&self, t: &mut TriFace) {
        *t = self.fsym(t);
    }
    /// `fnext(t1, t2)`: the next face rotating about the edge.
    #[inline]
    pub fn fnext(&self, t: &TriFace) -> TriFace {
        let mut t2 = decode(self.tets[t.tet].nbr[FACEPIVOT1[t.ver]]);
        t2.ver = FACEPIVOT2[t.ver][t2.ver];
        t2
    }
    /// `fnextself(t)`.
    #[inline]
    pub fn fnextself(&self, t: &mut TriFace) {
        *t = self.fnext(t);
    }

    /// `ishulltet(t)`.
    #[inline]
    pub fn ishulltet(&self, tet: usize) -> bool {
        self.tets[tet].v[3] == DUMMYPOINT
    }
    /// `isdeadtet(t)`.
    #[inline]
    pub fn isdeadtet(&self, tet: usize) -> bool {
        tet == NULL_TET || self.tets[tet].v[0] == NULL_POINT
    }
    /// `infect(t)`.
    #[inline]
    pub fn infect(&mut self, tet: usize) {
        self.tets[tet].marker |= 1;
    }
    /// `uninfect(t)`.
    #[inline]
    pub fn uninfect(&mut self, tet: usize) {
        self.tets[tet].marker &= !1;
    }
    /// `infected(t)`.
    #[inline]
    pub fn infected(&self, tet: usize) -> bool {
        (self.tets[tet].marker & 1) != 0
    }
    /// `marktest(t)`.
    #[inline]
    pub fn marktest(&mut self, tet: usize) {
        self.tets[tet].marker |= 2;
    }
    /// `unmarktest(t)`.
    #[inline]
    pub fn unmarktest(&mut self, tet: usize) {
        self.tets[tet].marker &= !2;
    }
    /// `marktested(t)`.
    #[inline]
    pub fn marktested(&self, tet: usize) -> bool {
        (self.tets[tet].marker & 2) != 0
    }

    /// `randomnation(choices)`: a random number in `0..choices` from
    /// TetGen's own LCG (seed [`TetgenMesh::randomseed`]).
    pub fn randomnation(&mut self, choices: u32) -> u64 {
        let choices = choices as u64;
        if choices >= 714025 {
            let mut newrandom = (self.randomseed * 1366 + 150889) % 714025;
            self.randomseed = (newrandom * 1366 + 150889) % 714025;
            newrandom = newrandom * (choices / 714025) + self.randomseed;
            if newrandom >= choices {
                newrandom - choices
            } else {
                newrandom
            }
        } else {
            self.randomseed = (self.randomseed * 1366 + 150889) % 714025;
            self.randomseed % choices
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// glibc's first outputs, from `srand(s); rand()` compiled with gcc
    /// 13.3 against glibc 2.39 on 2026-11-26.
    #[test]
    fn glibc_rand_matches_glibc() {
        let cases: [(u32, [i32; 5]); 4] = [
            (
                1,
                [1804289383, 846930886, 1681692777, 1714636915, 1957747793],
            ),
            (
                10,
                [1215069295, 1311962008, 1086128678, 385788725, 1753820418],
            ),
            (
                20000,
                [164365245, 1062436714, 2012788020, 1378038486, 1187980201],
            ),
            (
                0,
                [1804289383, 846930886, 1681692777, 1714636915, 1957747793],
            ),
        ];
        for (seed, want) in cases {
            let mut r = GlibcRand::new(seed);
            for w in want {
                assert_eq!(r.rand(), w, "seed {seed}");
            }
        }
    }

    /// The tables are permutations with the identities upstream relies on.
    #[test]
    fn tables_are_consistent() {
        for v in 0..12 {
            assert_eq!(ESYMTBL[ESYMTBL[v]], v);
            assert_eq!(ENEXTTBL[EPREVTBL[v]], v);
            assert_eq!(ENEXTESYMTBL[v], ESYMTBL[ENEXTTBL[v]]);
            assert_eq!(FACEPIVOT1[v], ESYMTBL[v] & 3);
            assert_eq!(EDGE2VER[VER2EDGE[v]] % 4, EDGE2VER[VER2EDGE[v]] & 3);
        }
        // bondtbl spot values from the formula.
        assert_eq!(BONDTBL[4][8], (8 & 3) + ((4 + 8) % 12));
        assert_eq!(TSBONDTBL[0][0], 0);
        assert!(STPIVOTTBL.iter().flatten().all(|&x| x < 12));
        assert!(SNEXTPIVOT.iter().all(|&x| x < 6));
        assert!(SORGPIVOT
            .iter()
            .zip(SDESTPIVOT)
            .zip(SAPEXPIVOT)
            .all(|((a, b), c)| a + b + c == 12));
        assert_eq!(STBONDTBL[11][5] < 12, TSPIVOTTBL[11][5] < 6);
    }

    /// The pool reuses the last freed slot first and traverses in slot
    /// order.
    #[test]
    fn memorypool_is_lifo_and_slot_ordered() {
        let mut p = MemoryPool::new(4);
        let a: Vec<usize> = (0..5).map(|_| p.alloc().0).collect();
        assert_eq!(a, [0, 1, 2, 3, 4]);
        p.dealloc(1);
        p.dealloc(3);
        assert_eq!(p.alloc(), (3, false));
        assert_eq!(p.alloc(), (1, false));
        assert_eq!(p.alloc(), (5, true));
        p.traversalinit();
        let mut seen = Vec::new();
        while let Some(s) = p.traverse() {
            seen.push(s);
        }
        assert_eq!(seen, [0, 1, 2, 3, 4, 5]);
        assert_eq!(p.items, 6);
    }

    /// `randomnation` against values computed by hand from the LCG.
    #[test]
    fn randomnation_follows_the_lcg() {
        let mut m = TetgenMesh::new(Behavior::default());
        // seed 1: (1 * 1366 + 150889) % 714025 = 152255.
        assert_eq!(m.randomnation(3), 152255 % 3);
        assert_eq!(m.randomseed, 152255);
        let s2 = (152255u64 * 1366 + 150889) % 714025;
        assert_eq!(m.randomnation(1000), s2 % 1000);
        // The large-choices branch.
        let seed = m.randomseed;
        let n1 = (seed * 1366 + 150889) % 714025;
        let s3 = (n1 * 1366 + 150889) % 714025;
        let want = n1 * (2_000_000 / 714025) + s3;
        let want = if want >= 2_000_000 {
            want - 2_000_000
        } else {
            want
        };
        assert_eq!(m.randomnation(2_000_000), want);
    }

    /// Handles encode and decode, and the primitives walk a tet.
    #[test]
    fn handles_and_primitives() {
        let mut m = TetgenMesh::new(Behavior::default());
        let pa = m.makepoint(VertType::Unused);
        let pb = m.makepoint(VertType::Unused);
        assert_eq!((m.pointmark(pa), m.pointmark(pb)), (0, 1));
        let t = m.maketetrahedron2(pa, pb, DUMMYPOINT, DUMMYPOINT);
        assert_eq!(decode(encode(&t)), t);
        assert_eq!(decode_ver_only(encode(&t)), 11);
        assert_eq!(decode_tet_only(NULL_HANDLE), NULL_TET);
        assert!(m.ishulltet(t.tet));
        let mut u = t;
        enextself(&mut u);
        eprevself(&mut u);
        assert_eq!(u, t);
        let e = esym(&t);
        assert_eq!((m.org(&e), m.dest(&e)), (m.dest(&t), m.org(&t)));
        assert_eq!(enext(&t).ver, ENEXTTBL[11]);
        assert_eq!(eprev(&t).ver, EPREVTBL[11]);
        assert_eq!(enextesym(&t).ver, ENEXTESYMTBL[11]);
        assert_eq!(eprevesym(&t).ver, EPREVESYMTBL[11]);
        let mut w = t;
        esymself(&mut w);
        enextesymself(&mut w);
        eprevesymself(&mut w);
        eorgoppoself(&mut w);
        edestoppoself(&mut w);
        assert!(w.ver < 12);
        let _ = m.apex(&t);
        let _ = m.oppo(&t);
        let empty = m.maketetrahedron();
        assert_eq!(m.tets[empty.tet].v, [NULL_POINT; 4]);
        m.infect(t.tet);
        m.marktest(t.tet);
        assert!(m.infected(t.tet) && m.marktested(t.tet));
        m.uninfect(t.tet);
        m.unmarktest(t.tet);
        assert_eq!(m.tets[t.tet].marker, 0);
        m.tetrahedrondealloc(empty.tet);
        assert!(m.isdeadtet(empty.tet));
        m.tetpool.traversalinit();
        assert_eq!(m.alltetrahedrontraverse(), Some(t.tet));
        assert_eq!(m.alltetrahedrontraverse(), None);
        m.pointdealloc(pa);
        m.pointpool.traversalinit();
        assert_eq!(m.pointtraverse(), Some(pb));
    }
}
