// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright (C) 2026 OUTRAM PARK contributors
//
// Port of TetGen 1.6.0 (point-set Delaunay tetrahedralisation: `tetgen.cxx`
// `transfernodes`, `hilbert_init`, `hilbert_split`, `hilbert_sort3`,
// `brio_multiscale_sort`, `randomsample`, `locate_dt`, `locate`,
// `insert_vertex_bw`, `initialdelaunay`, `incrementaldelaunay`,
// `insphere_s`, `orient3dfast`, `jettisonnodes`, `outnodes`, `outelements`,
// and the part of `tetrahedralize` that runs for a plain point set):
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

//! Point-set Delaunay tetrahedralisation (stage 2, gh:#1791): what
//! `tetrahedralize()` runs for a `.node` file with no `-p`, `-r`, `-q`,
//! `-w` or `-m`.
//!
//! The call sequence is upstream's: `initializepools` ([`TetgenMesh::new`]),
//! `transfernodes`, `incrementaldelaunay` (vertex permutation with glibc's
//! `rand()`, BRIO + Hilbert sort, `initialdelaunay`, then
//! `insert_vertex_bw` per vertex, which locates with `locate_dt` and fills
//! the Bowyer-Watson cavity), `jettisonnodes` when a duplicate was found,
//! and `outnodes` / `outelements` with `-z` numbering.
//!
//! **Upstream does not call `insertpoint` here.** `incrementaldelaunay`
//! inserts every vertex with `insert_vertex_bw` (the Bowyer-Watson cavity,
//! `ivf.bowywat = 1`), not with the flip-based `insertpoint`; `insertpoint`
//! (with `locate` and the flips) is first called by ~~`meshsurface`'s and~~
//! the recovery's Steiner points (`delaunizesegments`, stage 4), so it is
//! ported with ~~stage 3~~ stage 4 on. **CORRECTED 2026-11-26 (gh:#1792):**
//! `meshsurface` calls neither `insertpoint` nor a 3D flip (checked in
//! `tetgen.cxx`: no call between `flipshpush` and `meshsurface`).
//! [`TetgenMesh::locate`] (the walk with `randomnation`'s face choice) is
//! ported here for them.
//!
//! **Branches not ported** (none is reached by default flags): `-w`
//! (weighted, `orient4d_s` and the `NONREGULAR` result), the metric tensor
//! `-m`, point attributes, `-r`, the `-s` (PSC) and `-B`/marker columns of
//! `outnodes`, `-o2` and element attributes in `outelements`, and the
//! verbose printouts. Where upstream would `terminatetetgen`, the port
//! returns a [`TetgenError`].

#[cfg(not(any(feature = "std", test)))]
#[allow(unused_imports)]
use crate::nostd_prelude::*;

use super::mesh::{
    decode, decode_tet_only, encode, encode2, enextesymself, enextself, eprevesymself, eprevself,
    esym, esymself, Behavior, Point, TetgenMesh, TriFace, VertType, DUMMYPOINT, EPIVOT,
    NULL_HANDLE, NULL_POINT, NULL_TET,
};
use super::predicates::Predicates;
use alloc::vec::Vec;

/// `enum locateresult` (the values stage 2 produces or reads).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LocateResult {
    /// `UNKNOWN`.
    Unknown,
    /// `OUTSIDE`.
    Outside,
    /// `INTETRAHEDRON`.
    InTetrahedron,
    /// `ONFACE`.
    OnFace,
    /// `ONEDGE`.
    OnEdge,
    /// `ONVERTEX`.
    OnVertex,
    /// `ENCSUBFACE` (only from [`TetgenMesh::locate`] with `chkencflag`).
    EncSubface,
    /// `NEARVERTEX`.
    NearVertex,
    /// `NONREGULAR`.
    NonRegular,
    /// `ENCSEGMENT` (only from `slocate` with `cflag` off, stage 3).
    EncSegment,
    /// `ENCVERTEX` (stage 4 on, `insertpoint`).
    EncVertex,
    /// `INSTAR` (stage 4 on: the cavity is given).
    InStar,
    /// `BADELEMENT` (stage 4 on, `insertpoint`).
    BadElement,
    /// `NULLCAVITY` (stage 4 on, `insertpoint`).
    NullCavity,
}

/// `insertvertexflags` (the fields `insert_vertex_bw` reads).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct InsertVertexFlags {
    /// `iloc`: the location, in and out.
    pub iloc: LocateResult,
    /// `bowywat`.
    pub bowywat: i32,
    /// `lawson`.
    pub lawson: i32,
}

/// Where upstream calls `terminatetetgen`, or the input is unusable.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TetgenError {
    /// Fewer than four points (upstream reads past the array).
    TooFewPoints,
    /// "The point set is trivial." (`terminatetetgen(this, 10)`).
    TrivialPointSet,
    /// "All vertices are (nearly) identical."
    AllIdentical,
    /// "All vertices are (nearly) collinear."
    AllCollinear,
    /// "All vertices are coplanar."
    AllCoplanar,
    /// A coordinate is not finite.
    NonFinite,
    /// An internal error (`terminatetetgen(this, 2)`, "report a bug"): a
    /// location walk found no face below the point, or the symbolic
    /// perturbation of `insphere_s` was degenerate.
    InternalError,
    /// The input self-intersects where upstream stops with
    /// `terminatetetgen(this, 3)` (stage 4 on: `scoutsegment`,
    /// `scoutsubface`).
    SelfIntersection,
}

/// The output of [`delaunay_tetrahedralize`]: what `-z` writes to `.node`
/// and `.ele`.
#[derive(Debug, Clone, PartialEq)]
pub struct Tetrahedralization {
    /// The output points, in `.node` order (`outnodes`): the input points
    /// minus the jettisoned duplicates, in input order.
    pub points: Vec<[f64; 3]>,
    /// For every input point, its index in [`Tetrahedralization::points`];
    /// a duplicate maps to the point it coincides with.
    pub input_to_output: Vec<usize>,
    /// The tetrahedra in `.ele` order (`outelements`), each in upstream's
    /// vertex order (see [`delaunay_tetrahedralize`] for the orientation).
    pub tets: Vec<[usize; 4]>,
    /// The number of input points found coincident with an earlier one.
    pub duplicates: usize,
}

/// Delaunay tetrahedralisation of a point set, as `tetgen -z` writes it.
///
/// Runs TetGen 1.6.0's point-set path with default switches
/// (`tetgen -zQ file.node` on the same points, written in the same order
/// with `%.17g` coordinates, gives the same `.node` and `.ele` files,
/// verified bit for bit in
/// `verification_and_validation/tetgen_delaunay_gh1791/`). Coincident
/// points are reported by upstream ("Point #i is coincident with #j.
/// Ignored!") and jettisoned from the output; [`Tetrahedralization::input_to_output`]
/// maps them.
///
/// **Orientation.** TetGen's `orient3d(a, b, c, d)` is positive when `d`
/// lies *below* the plane of `a, b, c` (Shewchuk's convention); every
/// output tetrahedron `[a, b, c, d]` has `orient3d(a, b, c, d) < 0`, which
/// is TetGen's positive orientation.
pub fn delaunay_tetrahedralize(points: &[[f64; 3]]) -> Result<Tetrahedralization, TetgenError> {
    delaunay_tetrahedralize_with(points, &Behavior::default())
}

/// [`delaunay_tetrahedralize`] with explicit switches ([`Behavior`]: `-b`,
/// `-T`, `-X`, `-J`, `-x`).
pub fn delaunay_tetrahedralize_with(
    points: &[[f64; 3]],
    b: &Behavior,
) -> Result<Tetrahedralization, TetgenError> {
    if points.len() < 4 {
        return Err(TetgenError::TooFewPoints);
    }
    if points.iter().flatten().any(|x| !x.is_finite()) {
        return Err(TetgenError::NonFinite);
    }
    let mut m = TetgenMesh::new(*b);
    m.transfernodes(points)?;
    m.incrementaldelaunay()?;
    let duplicates = m.dupverts as usize;
    if !m.b.nojettison && (m.dupverts > 0 || m.unuverts > 0) {
        m.jettisonnodes();
    }
    let (out_points, marks) = m.outnodes();
    let tets = m.outelements();
    // input_to_output: input point i is pool slot i (points are allocated
    // in input order and never freed before jettison).
    let mut input_to_output = Vec::with_capacity(points.len());
    for i in 0..points.len() {
        let mut p = i + 1;
        if m.points[p].vtype == VertType::Dead {
            // A jettisoned duplicate: follow point2ppt to the original.
            while m.points[p].vtype == VertType::Dead {
                p = m.points[p].point2ppt;
            }
        }
        input_to_output.push(m.points[p].mark as usize);
    }
    let _ = marks;
    Ok(Tetrahedralization {
        points: out_points,
        input_to_output,
        tets,
        duplicates,
    })
}

impl TetgenMesh {
    /// The predicates (`orient3d`, `insphere`, ...), as set by `exactinit`.
    #[inline]
    fn orient3d(&self, a: Point, b: Point, c: Point, d: Point) -> f64 {
        let p: &Predicates = &self.pred;
        p.orient3d(self.xyz(a), self.xyz(b), self.xyz(c), self.xyz(d))
    }

    /// `transfernodes()`: read the vertices (here from a slice), compute
    /// the bounding box, call `exactinit` with its extents, `srand(n)`,
    /// `longest` and `minedgelength`.
    pub fn transfernodes(&mut self, input: &[[f64; 3]]) -> Result<(), TetgenError> {
        self.numberofpoints = input.len();
        let (mut x, mut y, mut z);
        for (i, c) in input.iter().enumerate() {
            let p = self.makepoint(VertType::Unused);
            self.points[p].xyz = *c;
            x = c[0];
            y = c[1];
            z = c[2];
            // Determine the smallest and largest x, y and z coordinates.
            if i == 0 {
                self.xmin = x;
                self.xmax = x;
                self.ymin = y;
                self.ymax = y;
                self.zmin = z;
                self.zmax = z;
            } else {
                self.xmin = if x < self.xmin { x } else { self.xmin };
                self.xmax = if x > self.xmax { x } else { self.xmax };
                self.ymin = if y < self.ymin { y } else { self.ymin };
                self.ymax = if y > self.ymax { y } else { self.ymax };
                self.zmin = if z < self.zmin { z } else { self.zmin };
                self.zmax = if z > self.zmax { z } else { self.zmax };
            }
        }

        x = self.xmax - self.xmin;
        y = self.ymax - self.ymin;
        z = self.zmax - self.zmin;

        self.pred = Predicates::exactinit(self.b.noexact, self.b.nostaticfilter, x, y, z);

        // Use the number of points as the random seed.
        self.crand.srand(input.len() as u32);

        // 'longest' is the largest possible edge length formed by input vertices.
        self.longest = (x * x + y * y + z * z).sqrt();
        if self.longest == 0.0 {
            return Err(TetgenError::TrivialPointSet);
        }
        // Two identical points are distinguished by 'minedgelength'.
        self.minedgelength = self.longest * self.b.epsilon;
        Ok(())
    }

    /// `hilbert_init(n)`: the Gray-code permutation table `transgc` and
    /// `tsb1mod3`.
    pub fn hilbert_init(&mut self, n: i32) {
        let mut gc = [0i32; 8];
        let big_n: i32 = if n == 2 { 4 } else { 8 };
        let mask: i32 = if n == 2 { 3 } else { 7 };

        // Generate the Gray code sequence.
        for i in 0..big_n {
            gc[i as usize] = i ^ (i >> 1);
        }

        for e in 0..big_n {
            for d in 0..n {
                // Calculate the end point (f).
                let f = e ^ (1 << d); // Toggle the d-th bit of 'e'.
                                      // travel_bit = 2**p, the bit we want to travel.
                let travel_bit = e ^ f;
                for i in 0..big_n {
                    // Rotate gc[i] left by (p + 1) % n bits.
                    let k = gc[i as usize] * (travel_bit * 2);
                    let g = (k | (k / big_n)) & mask;
                    // Calculate the permuted Gray code by xor with the start point (e).
                    self.transgc[e as usize][d as usize][i as usize] = g ^ e;
                }
            }
        }

        // Count the consecutive '1' bits (trailing) on the right.
        self.tsb1mod3[0] = 0;
        for i in 1..big_n {
            let mut v: i32 = !i; // Count the 0s.
            v = (v ^ (v.wrapping_sub(1))) >> 1; // Set v's trailing 0s to 1s and zero rest
            let mut c = 0;
            while v != 0 {
                v >>= 1;
                c += 1;
            }
            self.tsb1mod3[i as usize] = c % n;
        }
    }

    /// `hilbert_split()`: partition `v` about the box's mid-plane on the
    /// axis between Gray codes `gc0` and `gc1`; returns the split index.
    #[allow(clippy::too_many_arguments)]
    fn hilbert_split(
        &self,
        v: &mut [Point],
        gc0: i32,
        gc1: i32,
        bxmin: f64,
        bxmax: f64,
        bymin: f64,
        bymax: f64,
        bzmin: f64,
        bzmax: f64,
    ) -> usize {
        let arraysize = v.len() as isize;
        // Find the current splitting axis.
        let axis = ((gc0 ^ gc1) >> 1) as usize;

        // Calulate the split position along the axis.
        let split = if axis == 0 {
            0.5 * (bxmin + bxmax)
        } else if axis == 1 {
            0.5 * (bymin + bymax)
        } else {
            0.5 * (bzmin + bzmax)
        };

        // Find the direction (+1 or -1) of the axis.
        let d: i32 = if (gc0 & (1 << axis)) == 0 { 1 } else { -1 };

        let mut i: isize = 0;
        let mut j: isize = arraysize - 1;

        if d > 0 {
            loop {
                while i < arraysize {
                    if self.points[v[i as usize]].xyz[axis] >= split {
                        break;
                    }
                    i += 1;
                }
                while j >= 0 {
                    if self.points[v[j as usize]].xyz[axis] < split {
                        break;
                    }
                    j -= 1;
                }
                // Is the partition finished?
                if i == j + 1 {
                    break;
                }
                v.swap(i as usize, j as usize);
            }
        } else {
            loop {
                while i < arraysize {
                    if self.points[v[i as usize]].xyz[axis] <= split {
                        break;
                    }
                    i += 1;
                }
                while j >= 0 {
                    if self.points[v[j as usize]].xyz[axis] > split {
                        break;
                    }
                    j -= 1;
                }
                if i == j + 1 {
                    break;
                }
                v.swap(i as usize, j as usize);
            }
        }
        i as usize
    }

    /// `hilbert_sort3()`: sort points along the 3D Hilbert curve.
    #[allow(clippy::too_many_arguments)]
    pub fn hilbert_sort3(
        &self,
        v: &mut [Point],
        e: i32,
        d: i32,
        bxmin: f64,
        bxmax: f64,
        bymin: f64,
        bymax: f64,
        bzmin: f64,
        bzmax: f64,
        depth: i32,
    ) {
        let n = 3;
        let mask = 7;
        let mut p = [0usize; 9];
        let tg = &self.transgc[e as usize][d as usize];
        let arraysize = v.len();

        p[0] = 0;
        p[8] = arraysize;

        let bb = (bxmin, bxmax, bymin, bymax, bzmin, bzmax);
        let split = |s: &Self, sl: &mut [Point], a: i32, c: i32| {
            s.hilbert_split(sl, a, c, bb.0, bb.1, bb.2, bb.3, bb.4, bb.5)
        };
        p[4] = split(self, &mut v[..p[8]], tg[3], tg[4]);
        p[2] = split(self, &mut v[..p[4]], tg[1], tg[2]);
        p[1] = split(self, &mut v[..p[2]], tg[0], tg[1]);
        p[3] = split(self, &mut v[p[2]..p[4]], tg[2], tg[3]) + p[2];
        p[6] = split(self, &mut v[p[4]..p[8]], tg[5], tg[6]) + p[4];
        p[5] = split(self, &mut v[p[4]..p[6]], tg[4], tg[5]) + p[4];
        p[7] = split(self, &mut v[p[6]..p[8]], tg[6], tg[7]) + p[6];

        if self.b.hilbert_order > 0 {
            // A maximum order is prescribed.
            if (depth + 1) == self.b.hilbert_order {
                return;
            }
        }

        // Recursively sort the points in sub-boxes.
        for w in 0..8usize {
            if (p[w + 1] - p[w]) as i64 > self.b.hilbert_limit as i64 {
                let wi = w as i32;
                // Calculcate the start point (ei) of the curve in this sub-box.
                let mut e_w = if w == 0 {
                    0
                } else {
                    let k = 2 * ((wi - 1) / 2);
                    k ^ (k >> 1)
                };
                let k = e_w;
                e_w = ((k << (d + 1)) & mask) | ((k >> (n - d - 1)) & mask);
                let ei = e ^ e_w;
                // Calulcate the direction (di) of the curve in this sub-box.
                let d_w = if w == 0 {
                    0
                } else if (wi % 2) == 0 {
                    self.tsb1mod3[w - 1]
                } else {
                    self.tsb1mod3[w]
                };
                let di = (d + d_w + 1) % n;
                // Calculate the bounding box of the sub-box.
                let (x1, x2) = if tg[w] & 1 != 0 {
                    (0.5 * (bxmin + bxmax), bxmax)
                } else {
                    (bxmin, 0.5 * (bxmin + bxmax))
                };
                let (y1, y2) = if tg[w] & 2 != 0 {
                    (0.5 * (bymin + bymax), bymax)
                } else {
                    (bymin, 0.5 * (bymin + bymax))
                };
                let (z1, z2) = if tg[w] & 4 != 0 {
                    (0.5 * (bzmin + bzmax), bzmax)
                } else {
                    (bzmin, 0.5 * (bzmin + bzmax))
                };
                self.hilbert_sort3(
                    &mut v[p[w]..p[w + 1]],
                    ei,
                    di,
                    x1,
                    x2,
                    y1,
                    y2,
                    z1,
                    z2,
                    depth + 1,
                );
            }
        }
    }

    /// `brio_multiscale_sort()`: biased randomised insertion order rounds,
    /// each sorted along the Hilbert curve.
    pub fn brio_multiscale_sort(
        &self,
        v: &mut [Point],
        threshold: i32,
        ratio: f64,
        depth: &mut i32,
    ) {
        let arraysize = v.len();
        let mut middle = 0usize;
        if arraysize as i64 >= threshold as i64 {
            *depth += 1;
            // middle = arraysize * ratio (int * double, truncated).
            middle = ((arraysize as i32) as f64 * ratio) as i32 as usize;
            self.brio_multiscale_sort(&mut v[..middle], threshold, ratio, depth);
        }
        // Sort the right-array (rnd-th round) using the Hilbert curve.
        self.hilbert_sort3(
            &mut v[middle..],
            0,
            0,
            self.xmin,
            self.xmax,
            self.ymin,
            self.ymax,
            self.zmin,
            self.zmax,
            0,
        );
    }

    /// `randomsample()`: choose a starting tetrahedron for point location
    /// from `recenttet` and a random sample per pool block (only with
    /// `-b0`; the default starts from `recenttet`).
    pub fn randomsample(&mut self, searchpt: Point, searchtet: &mut TriFace) {
        let sp = *self.xyz(searchpt);
        let dist2 = |q: &[f64; 3]| {
            (sp[0] - q[0]) * (sp[0] - q[0])
                + (sp[1] - q[1]) * (sp[1] - q[1])
                + (sp[2] - q[2]) * (sp[2] - q[2])
        };
        let mut searchdist;
        if !self.nonconvex {
            if searchtet.tet == NULL_TET {
                // A null tet. Choose the recenttet as the starting tet.
                *searchtet = self.recenttet;
            }
            // Choose the base face whose vertices must not be 'dummypoint'.
            searchtet.ver = 3;
            let torg = self.org(searchtet);
            searchdist = dist2(self.xyz(torg));

            if self.recenttet.tet != searchtet.tet {
                self.recenttet.ver = 3;
                let torg = self.org(&self.recenttet);
                let dist = dist2(self.xyz(torg));
                if dist < searchdist {
                    *searchtet = self.recenttet;
                    searchdist = dist;
                }
            }
        } else {
            searchdist = self.longest;
        }

        // Select "good" candidate using k random samples, taking the closest one.
        while self.samples * self.samples * self.samples * self.samples < self.tetpool.items as i64
        {
            self.samples += 1;
        }
        let perblock = self.b.tetrahedraperblock as i64;
        // Find how much blocks in current tet pool.
        let tetblocks = (self.tetpool.maxitems as i64 + perblock - 1) / perblock;
        // Find the average samples per block. Each block at least have 1 sample.
        let samplesperblock = 1 + (self.samples / tetblocks);
        let mut sampleblocks = self.samples / samplesperblock;
        if sampleblocks == 0 {
            sampleblocks = 1;
        }
        for i in 0..sampleblocks {
            let firsttet = (i * perblock) as usize;
            let mut j: i64 = 0;
            while j < samplesperblock {
                let samplenum = if i == tetblocks - 1 {
                    // This is the last block.
                    self.randomnation((self.tetpool.maxitems as i64 - i * perblock) as i32 as u32)
                } else {
                    self.randomnation(perblock as u32)
                };
                let tetptr = firsttet + samplenum as usize;
                let torg = self.tets[tetptr].v[0];
                if torg != NULL_POINT {
                    let dist = dist2(self.xyz(torg));
                    if dist < searchdist {
                        searchtet.tet = tetptr;
                        searchtet.ver = 11; // torg = org(t);
                        searchdist = dist;
                    }
                } else {
                    // A dead tet. Re-sample it.
                    if i != tetblocks - 1 {
                        j -= 1;
                    }
                }
                j += 1;
            }
        }
    }

    /// `locate_dt()`: walk from `searchtet` to the tetrahedron containing
    /// `searchpt`, choosing the exit face with glibc's `rand() % 3`.
    pub fn locate_dt(
        &mut self,
        searchpt: Point,
        searchtet: &mut TriFace,
    ) -> Result<LocateResult, TetgenError> {
        let loc;

        if searchtet.tet == NULL_TET {
            searchtet.tet = self.recenttet.tet;
        }

        if self.ishulltet(searchtet.tet) {
            // Get its adjacent tet (inside the hull).
            searchtet.tet = decode_tet_only(self.tets[searchtet.tet].nbr[3]);
        }

        // Let searchtet be the face such that 'searchpt' lies above to it.
        searchtet.ver = 0;
        while searchtet.ver < 4 {
            let ori = self.orient3d(
                self.org(searchtet),
                self.dest(searchtet),
                self.apex(searchtet),
                searchpt,
            );
            if ori < 0.0 {
                break;
            }
            searchtet.ver += 1;
        }

        if searchtet.ver == 4 {
            return Err(TetgenError::InternalError);
        }

        // Walk through tetrahedra to locate the point.
        loop {
            let toppo = self.oppo(searchtet);

            // Check if the vertex is we seek.
            if toppo == searchpt {
                // Adjust the origin of searchtet to be searchpt.
                esymself(searchtet);
                eprevself(searchtet);
                loc = LocateResult::OnVertex;
                break;
            }

            // Randomly choose one of three faces (containig toppo) of this tet.
            let s = self.crand.rand() % 3;
            for _ in 0..s {
                enextself(searchtet);
            }

            let oriorg = self.orient3d(self.dest(searchtet), self.apex(searchtet), toppo, searchpt);
            if oriorg < 0.0 {
                enextesymself(searchtet);
            } else {
                let oridest =
                    self.orient3d(self.apex(searchtet), self.org(searchtet), toppo, searchpt);
                if oridest < 0.0 {
                    eprevesymself(searchtet);
                } else {
                    let oriapex =
                        self.orient3d(self.org(searchtet), self.dest(searchtet), toppo, searchpt);
                    if oriapex < 0.0 {
                        esymself(searchtet);
                    } else {
                        // oriorg >= 0, oridest >= 0, oriapex >= 0 ==> found the point.
                        if oriorg == 0.0 {
                            // Go to the face opposite to origin.
                            enextesymself(searchtet);
                            if oridest == 0.0 {
                                eprevself(searchtet); // edge oppo->apex
                                if oriapex == 0.0 {
                                    // oppo is duplicated with p.
                                    loc = LocateResult::OnVertex;
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
                            // Go to the face opposite to destination.
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
                            // Go to the face opposite to apex
                            esymself(searchtet);
                            loc = LocateResult::OnFace;
                            break;
                        }
                        loc = LocateResult::InTetrahedron;
                        break;
                    }
                }
            }

            // Move to the next tet adjacent to the selected face.
            *searchtet = decode(self.tets[searchtet.tet].nbr[searchtet.ver & 3]); // fsymself

            if self.ishulltet(searchtet.tet) {
                loc = LocateResult::Outside;
                break;
            }
        }

        Ok(loc)
    }

    /// `locate()`: the general location walk (used by `insertpoint` from
    /// ~~stage 3~~ stage 4 on), choosing among viable exit faces with
    /// [`TetgenMesh::randomnation`]. `chkencflag` stops at a subface bonded
    /// to a tetrahedron (~~stage 3 on~~ `tsbond`, stage 4 on; stage 3 builds
    /// subfaces but bonds none to a tetrahedron, so it never triggers yet).
    pub fn locate(
        &mut self,
        searchpt: Point,
        searchtet: &mut TriFace,
        chkencflag: bool,
    ) -> Result<LocateResult, TetgenError> {
        #[derive(PartialEq)]
        enum Move {
            Org,
            Dest,
            Apex,
        }
        let loc;
        let (mut torg, mut tdest, mut tapex);
        self.reach[36] += 1;

        if searchtet.tet == NULL_TET {
            searchtet.tet = self.recenttet.tet;
        }
        if self.ishulltet(searchtet.tet) {
            searchtet.tet = decode_tet_only(self.tets[searchtet.tet].nbr[3]);
        }

        // Let searchtet be the face such that 'searchpt' lies above to it.
        searchtet.ver = 0;
        torg = NULL_POINT;
        tdest = NULL_POINT;
        tapex = NULL_POINT;
        while searchtet.ver < 4 {
            torg = self.org(searchtet);
            tdest = self.dest(searchtet);
            tapex = self.apex(searchtet);
            let ori = self.orient3d(torg, tdest, tapex, searchpt);
            if ori < 0.0 {
                break;
            }
            searchtet.ver += 1;
        }
        if searchtet.ver == 4 {
            return Err(TetgenError::InternalError);
        }

        loop {
            let toppo = self.oppo(searchtet);
            if toppo == searchpt {
                esymself(searchtet);
                eprevself(searchtet);
                loc = LocateResult::OnVertex;
                break;
            }

            let oriorg = self.orient3d(tdest, tapex, toppo, searchpt);
            let oridest = self.orient3d(tapex, torg, toppo, searchpt);
            let oriapex = self.orient3d(torg, tdest, toppo, searchpt);

            let nextmove;
            if oriorg < 0.0 {
                if oridest < 0.0 {
                    if oriapex < 0.0 {
                        // All three faces are possible.
                        let s = self.randomnation(3);
                        nextmove = if s == 0 {
                            Move::Org
                        } else if s == 1 {
                            Move::Dest
                        } else {
                            Move::Apex
                        };
                    } else if self.randomnation(2) != 0 {
                        nextmove = Move::Org;
                    } else {
                        nextmove = Move::Dest;
                    }
                } else if oriapex < 0.0 {
                    if self.randomnation(2) != 0 {
                        nextmove = Move::Org;
                    } else {
                        nextmove = Move::Apex;
                    }
                } else {
                    nextmove = Move::Org;
                }
            } else if oridest < 0.0 {
                if oriapex < 0.0 {
                    if self.randomnation(2) != 0 {
                        nextmove = Move::Dest;
                    } else {
                        nextmove = Move::Apex;
                    }
                } else {
                    nextmove = Move::Dest;
                }
            } else if oriapex < 0.0 {
                nextmove = Move::Apex;
            } else {
                if oriorg == 0.0 {
                    enextesymself(searchtet);
                    if oridest == 0.0 {
                        eprevself(searchtet);
                        if oriapex == 0.0 {
                            loc = LocateResult::OnVertex;
                            break;
                        }
                        loc = LocateResult::OnEdge;
                        break;
                    }
                    if oriapex == 0.0 {
                        enextself(searchtet);
                        loc = LocateResult::OnEdge;
                        break;
                    }
                    loc = LocateResult::OnFace;
                    break;
                }
                if oridest == 0.0 {
                    eprevesymself(searchtet);
                    if oriapex == 0.0 {
                        eprevself(searchtet);
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
            if nextmove == Move::Org {
                enextesymself(searchtet);
            } else if nextmove == Move::Dest {
                eprevesymself(searchtet);
            } else {
                esymself(searchtet);
            }
            if chkencflag {
                // issubface(*searchtet). ~~no tetrahedron holds a subface
                //   before stage 4 (`tsbond`)~~ CORRECTED 2026-11-26
                //   (gh:#1793): the test was "the tet has a subface array";
                //   upstream tests the entry at this face, which stage 4's
                //   `tsbond` fills ([`TetgenMesh::issubface`]).
                if self.issubface(searchtet) {
                    loc = LocateResult::EncSubface;
                    break;
                }
            }
            // Move to the adjacent tetrahedron (maybe a hull tetrahedron).
            *searchtet = decode(self.tets[searchtet.tet].nbr[searchtet.ver & 3]);
            if self.ishulltet(searchtet.tet) {
                loc = LocateResult::Outside;
                break;
            }

            torg = self.org(searchtet);
            tdest = self.dest(searchtet);
            tapex = self.apex(searchtet);
        }

        Ok(loc)
    }

    /// `insphere_s()`: insphere with symbolic perturbation (never zero).
    pub fn insphere_s(
        &self,
        pa: Point,
        pb: Point,
        pc: Point,
        pd: Point,
        pe: Point,
    ) -> Result<f64, TetgenError> {
        let sign = self.pred.insphere(
            self.xyz(pa),
            self.xyz(pb),
            self.xyz(pc),
            self.xyz(pd),
            self.xyz(pe),
        );
        if sign != 0.0 {
            return Ok(sign);
        }

        // Symbolic perturbation.
        let mut pt = [pa, pb, pc, pd, pe];
        // Sort the five points such that their indices are in the increasing order.
        let mut swaps = 0;
        let mut n = 5;
        loop {
            let mut count = 0;
            n -= 1;
            for i in 0..n {
                if self.pointmark(pt[i]) > self.pointmark(pt[i + 1]) {
                    pt.swap(i, i + 1);
                    count += 1;
                }
            }
            swaps += count;
            if count == 0 {
                break;
            }
        }

        let mut ori_a = self.orient3d(pt[1], pt[2], pt[3], pt[4]);
        if ori_a != 0.0 {
            // Flip the sign if there are odd number of swaps.
            if (swaps % 2) != 0 {
                ori_a = -ori_a;
            }
            return Ok(ori_a);
        }

        let mut ori_b = -self.orient3d(pt[0], pt[2], pt[3], pt[4]);
        if ori_b == 0.0 {
            return Err(TetgenError::InternalError);
        }
        if (swaps % 2) != 0 {
            ori_b = -ori_b;
        }
        Ok(ori_b)
    }

    /// Whether `point2tet(p)` names a tetrahedron whose third vertex is
    /// `insertpt` (upstream: `((point *) point2tet(p))[6] == insertpt`).
    ///
    /// Upstream dereferences the stored word as a raw pointer. The four
    /// points of the first tetrahedron (and `dummypoint`) hold
    /// `encode(firsttet)` / `encode(tetopa)`, a pointer carrying version 11
    /// in its low bits; the read then lands 11 bytes off the record and
    /// returns the high bytes of word 7 and the low bytes of word 8 (`NULL`),
    /// a small integer that is never a point's address. Such a handle
    /// therefore never compares equal, and neither does it here.
    #[inline]
    fn point2tet_apex_is(&self, p: Point, insertpt: Point) -> bool {
        let h = self.point2tet(p);
        if h == NULL_HANDLE || (h & 15) != 0 {
            return false;
        }
        self.tets[decode_tet_only(h)].v[2] == insertpt
    }

    /// `insert_vertex_bw()`: insert a vertex with the Bowyer-Watson
    /// algorithm (the initial Delaunay construction). Returns `Ok(false)`
    /// when the vertex is not inserted (`ivf.iloc` says why).
    pub fn insert_vertex_bw(
        &mut self,
        insertpt: Point,
        searchtet: &mut TriFace,
        ivf: &mut InsertVertexFlags,
    ) -> Result<bool, TetgenError> {
        const ROW_V08_TBL: [usize; 12] = [8, 9, 10, 11, 0, 1, 2, 3, 4, 5, 6, 7];
        const ROW_V11_TBL: [usize; 12] = [8, 9, 10, 11, 0, 1, 2, 3, 4, 5, 6, 7];
        const COL_V01_TBL: [usize; 12] = [1, 1, 1, 1, 5, 5, 5, 5, 9, 9, 9, 9];
        const COL_V02_TBL: [usize; 12] = [2, 2, 2, 2, 6, 6, 6, 6, 10, 10, 10, 10];
        const COL_V08_TBL: [usize; 12] = [8, 8, 8, 8, 0, 0, 0, 0, 4, 4, 4, 4];
        const COL_V11_TBL: [usize; 12] = [11, 11, 11, 11, 3, 3, 3, 3, 7, 7, 7, 7];

        let mut loc = LocateResult::Outside;

        // Locate the point.
        if searchtet.tet != NULL_TET {
            loc = ivf.iloc;
        }

        if loc == LocateResult::Outside {
            if searchtet.tet == NULL_TET {
                // `-w` (weighted) is not ported: always randomsample.
                self.randomsample(insertpt, searchtet);
            }
            loc = self.locate_dt(insertpt, searchtet)?;
        }

        ivf.iloc = loc; // The return value.

        let mut cave_oldtet_list = core::mem::take(&mut self.cave_oldtet_list);
        let mut cavebdrylist = core::mem::take(&mut self.cavebdrylist);

        // Create the initial cavity C(p) which contains all tetrahedra that
        //   intersect p. It may include 1, 2, or n tetrahedra.
        match loc {
            LocateResult::Outside | LocateResult::InTetrahedron => {
                self.infect(searchtet.tet);
                cave_oldtet_list.push(searchtet.tet);
            }
            LocateResult::OnFace => {
                self.infect(searchtet.tet);
                cave_oldtet_list.push(searchtet.tet);
                let n = decode_tet_only(self.tets[searchtet.tet].nbr[searchtet.ver & 3]);
                self.infect(n);
                cave_oldtet_list.push(n);
            }
            LocateResult::OnEdge => {
                // Add all adjacent boundary tets into list.
                let mut spintet = *searchtet;
                loop {
                    self.infect(spintet.tet);
                    cave_oldtet_list.push(spintet.tet);
                    self.fnextself(&mut spintet);
                    if spintet.tet == searchtet.tet {
                        break;
                    }
                }
            }
            LocateResult::OnVertex => {
                // The point already exist. Do nothing and return.
                self.cave_oldtet_list = cave_oldtet_list;
                self.cavebdrylist = cavebdrylist;
                return Ok(false);
            }
            _ => {}
        }

        // Create the cavity C(p).
        let mut i = 0;
        while i < cave_oldtet_list.len() {
            let mut cavetet = TriFace {
                tet: cave_oldtet_list[i],
                ver: 0,
            };
            cavetet.ver = 0;
            while cavetet.ver < 4 {
                let neigh = decode_tet_only(self.tets[cavetet.tet].nbr[cavetet.ver]);
                if !self.infected(neigh) {
                    // neightet.tet is current outside the cavity.
                    let mut enqflag = false;
                    if !self.marktested(neigh) {
                        let pts = self.tets[neigh].v;
                        if !self.ishulltet(neigh) {
                            let sign = self.insphere_s(pts[0], pts[1], pts[2], pts[3], insertpt)?;
                            enqflag = sign < 0.0;
                        } else {
                            let ori = self.orient3d(pts[0], pts[1], pts[2], insertpt);
                            if ori < 0.0 {
                                // A visible hull face.
                                enqflag = true;
                            } else if ori == 0.0 {
                                // A coplanar hull face. Test the adjacent tet
                                //   (not faked) of this hull face.
                                let neinei = decode_tet_only(self.tets[neigh].nbr[3]);
                                let pts = self.tets[neinei].v;
                                let sign =
                                    self.insphere_s(pts[0], pts[1], pts[2], pts[3], insertpt)?;
                                enqflag = sign < 0.0;
                            }
                        }
                        self.marktest(neigh);
                    }
                    if enqflag {
                        self.infect(neigh);
                        cave_oldtet_list.push(neigh);
                    } else {
                        // A boundary face.
                        cavebdrylist.push(cavetet);
                    }
                }
                cavetet.ver += 1;
            }
            i += 1;
        }

        // Create new tetrahedra to fill the cavity.
        let f_out = cavebdrylist.len();
        let v_out = (f_out + 4) / 2;

        let mut local_vcount: i32 = 0;
        let mut sidx = [0usize; 3];
        let mut shiftbits = 0u32;
        let mut tmp_bw_faces: Vec<TriFace> = Vec::new();
        let small = v_out < 64;

        if small {
            shiftbits = 6;
            tmp_bw_faces = core::mem::take(&mut self.bw_faces);
        } else if v_out < 1024 {
            // Dynamically allocate an array to store the adjacencies.
            let mut arysize: usize = 1;
            let mut tmp = v_out;
            shiftbits = 1;
            loop {
                tmp >>= 1;
                if tmp == 0 {
                    break;
                }
                shiftbits += 1;
            }
            arysize <<= shiftbits;
            tmp_bw_faces = alloc::vec![TriFace::default(); arysize * arysize];
        }

        if v_out < 1024 {
            for i in 0..f_out {
                let oldtet = cavebdrylist[i];

                // Get the tet outside the cavity.
                let mut neightet = decode(self.tets[oldtet.tet].nbr[oldtet.ver]);
                self.unmarktest(neightet.tet);

                if self.ishulltet(oldtet.tet) {
                    // neightet.tet may be also a hull tet (=> oldtet is a hull edge).
                    neightet.ver = EPIVOT[neightet.ver];
                    if self.apex(&neightet) == DUMMYPOINT {
                        self.hullsize += 1; // Create a new hull tet.
                    }
                }

                // Create a new tet in the cavity.
                let vv = [
                    self.dest(&neightet),
                    self.org(&neightet),
                    self.apex(&neightet),
                ];
                let newtet = self.maketetrahedron2(vv[1], vv[0], insertpt, vv[2]);
                self.tets[newtet.tet].nbr[2] = encode2(neightet.tet, neightet.ver);
                self.tets[neightet.tet].nbr[neightet.ver & 3] =
                    encode2(newtet.tet, COL_V02_TBL[neightet.ver]);

                // Fill the adjacency matrix, and count v_out.
                for j in 0..3 {
                    if !self.point2tet_apex_is(vv[j], insertpt) {
                        // Found a unique vertex of the cavity.
                        self.points[vv[j]].geomtag = local_vcount;
                        local_vcount += 1;
                        self.setpoint2tet(vv[j], encode2(newtet.tet, 0));
                    }
                    sidx[j] = self.points[vv[j]].geomtag as usize;
                }

                let mut nt = TriFace {
                    tet: newtet.tet,
                    ver: 11,
                };
                tmp_bw_faces[(sidx[1] << shiftbits) | sidx[0]] = nt;
                nt.ver = 1;
                tmp_bw_faces[(sidx[2] << shiftbits) | sidx[1]] = nt;
                nt.ver = 8;
                tmp_bw_faces[(sidx[0] << shiftbits) | sidx[2]] = nt;

                cavebdrylist[i] = newtet;
            }

            // Set a handle for speeding point location.
            // Randomly pick a new tet.
            let r = (self.crand.rand() % f_out as i32) as usize;
            self.recenttet = cavebdrylist[r];
            self.setpoint2tet(insertpt, encode2(self.recenttet.tet, 0));

            for i in 0..f_out {
                let mut neightet = cavebdrylist[i];
                if self.tets[neightet.tet].nbr[3] == NULL_HANDLE {
                    neightet.ver = 11;
                    let j = self.points[self.org(&neightet)].geomtag as usize;
                    let k = self.points[self.dest(&neightet)].geomtag as usize;
                    let neineitet = tmp_bw_faces[(k << shiftbits) | j];
                    self.tets[neightet.tet].nbr[3] =
                        encode2(neineitet.tet, ROW_V11_TBL[neineitet.ver]);
                    self.tets[neineitet.tet].nbr[neineitet.ver & 3] =
                        encode2(neightet.tet, COL_V11_TBL[neineitet.ver]);
                }
                if self.tets[neightet.tet].nbr[1] == NULL_HANDLE {
                    neightet.ver = 1;
                    let j = self.points[self.org(&neightet)].geomtag as usize;
                    let k = self.points[self.dest(&neightet)].geomtag as usize;
                    let neineitet = tmp_bw_faces[(k << shiftbits) | j];
                    self.tets[neightet.tet].nbr[1] = encode2(neineitet.tet, neineitet.ver);
                    self.tets[neineitet.tet].nbr[neineitet.ver & 3] =
                        encode2(neightet.tet, COL_V01_TBL[neineitet.ver]);
                }
                if self.tets[neightet.tet].nbr[0] == NULL_HANDLE {
                    neightet.ver = 8;
                    let j = self.points[self.org(&neightet)].geomtag as usize;
                    let k = self.points[self.dest(&neightet)].geomtag as usize;
                    let neineitet = tmp_bw_faces[(k << shiftbits) | j];
                    self.tets[neightet.tet].nbr[0] =
                        encode2(neineitet.tet, ROW_V08_TBL[neineitet.ver]);
                    self.tets[neineitet.tet].nbr[neineitet.ver & 3] =
                        encode2(neightet.tet, COL_V08_TBL[neineitet.ver]);
                }
            }

            if small {
                self.bw_faces = tmp_bw_faces;
            }
        } else {
            // Fill a very large cavity with original neighboring searching method.
            for i in 0..f_out {
                let oldtet = cavebdrylist[i];
                let mut neightet = decode(self.tets[oldtet.tet].nbr[oldtet.ver]);
                self.unmarktest(neightet.tet);

                if self.ishulltet(oldtet.tet) {
                    neightet.ver = EPIVOT[neightet.ver];
                    if self.apex(&neightet) == DUMMYPOINT {
                        self.hullsize += 1;
                    }
                }

                let vv = [
                    self.dest(&neightet),
                    self.org(&neightet),
                    self.apex(&neightet),
                ];
                let newtet = self.maketetrahedron2(vv[1], vv[0], insertpt, vv[2]);
                self.tets[newtet.tet].nbr[2] = encode2(neightet.tet, neightet.ver);
                self.tets[neightet.tet].nbr[neightet.ver & 3] =
                    encode2(newtet.tet, COL_V02_TBL[neightet.ver]);

                for &vj in &vv {
                    if !self.point2tet_apex_is(vj, insertpt) {
                        local_vcount += 1;
                        self.setpoint2tet(vj, encode2(newtet.tet, 0));
                    }
                }
            }

            // Set a handle for speeding point location.
            let r = (self.crand.rand() % f_out as i32) as usize;
            self.recenttet = cavebdrylist[r];
            // This is still an oldtet.
            let mut rt = self.recenttet;
            self.fsymself(&mut rt);
            self.fsymself(&mut rt);
            self.recenttet = rt;
            self.setpoint2tet(insertpt, encode2(self.recenttet.tet, 0));

            for i in 0..f_out {
                let mut oldtet = cavebdrylist[i];
                let neightet = self.fsym(&oldtet);
                let mut newtet = self.fsym(&neightet);
                // Connect the three other faces of this newtet.
                for _ in 0..3 {
                    let nt = esym(&newtet); // Go to the face.
                    if self.tets[nt.tet].nbr[nt.ver & 3] == NULL_HANDLE {
                        // Find the adjacent face of this newtet.
                        let mut spintet = oldtet;
                        loop {
                            self.fnextself(&mut spintet);
                            if !self.infected(spintet.tet) {
                                break;
                            }
                        }
                        let mut neineitet = self.fsym(&spintet);
                        esymself(&mut neineitet);
                        self.bond(&nt, &neineitet);
                    }
                    enextself(&mut newtet);
                    enextself(&mut oldtet);
                }
            }
        }
        let _ = local_vcount;

        // C(p) is re-meshed successfully.

        // Delete the old tets in C(p).
        for &t in &cave_oldtet_list {
            if self.ishulltet(t) {
                self.hullsize -= 1;
            }
            self.tetrahedrondealloc(t);
        }

        cave_oldtet_list.clear();
        cavebdrylist.clear();
        self.cave_oldtet_list = cave_oldtet_list;
        self.cavebdrylist = cavebdrylist;

        Ok(true)
    }

    /// `initialdelaunay()`: one tetrahedron `abcd` and four hull
    /// tetrahedra.
    pub fn initialdelaunay(&mut self, pa: Point, pb: Point, pc: Point, pd: Point) {
        // Create the first tetrahedron.
        let firsttet = self.maketetrahedron2(pa, pb, pc, pd);
        // Create four hull tetrahedra.
        let tetopa = self.maketetrahedron2(pb, pc, pd, DUMMYPOINT);
        let tetopb = self.maketetrahedron2(pc, pa, pd, DUMMYPOINT);
        let tetopc = self.maketetrahedron2(pa, pb, pd, DUMMYPOINT);
        let tetopd = self.maketetrahedron2(pb, pa, pc, DUMMYPOINT);

        self.hullsize += 4;

        // Connect hull tetrahedra to firsttet (at four faces of firsttet).
        self.bond(&firsttet, &tetopd);
        let worktet = esym(&firsttet);
        self.bond(&worktet, &tetopc); // ab
        let worktet = super::mesh::enextesym(&firsttet);
        self.bond(&worktet, &tetopa); // bc
        let worktet = super::mesh::eprevesym(&firsttet);
        self.bond(&worktet, &tetopb); // ca

        // Connect hull tetrahedra together (at six edges of firsttet).
        let w = esym(&tetopc);
        let w1 = esym(&tetopd);
        self.bond(&w, &w1); // ab
        let w = esym(&tetopa);
        let w1 = super::mesh::eprevesym(&tetopd);
        self.bond(&w, &w1); // bc
        let w = esym(&tetopb);
        let w1 = super::mesh::enextesym(&tetopd);
        self.bond(&w, &w1); // ca
        let w = super::mesh::eprevesym(&tetopc);
        let w1 = super::mesh::enextesym(&tetopb);
        self.bond(&w, &w1); // da
        let w = super::mesh::eprevesym(&tetopa);
        let w1 = super::mesh::enextesym(&tetopc);
        self.bond(&w, &w1); // db
        let w = super::mesh::eprevesym(&tetopb);
        let w1 = super::mesh::enextesym(&tetopa);
        self.bond(&w, &w1); // dc

        // Set the vertex type.
        for p in [pa, pb, pc, pd] {
            if self.pointtype(p) == VertType::Unused {
                self.setpointtype(p, VertType::Vol);
            }
        }

        for p in [pa, pb, pc, pd] {
            self.setpoint2tet(p, encode(&firsttet));
        }
        self.setpoint2tet(DUMMYPOINT, encode(&tetopa));

        // Remember the first tetrahedron.
        self.recenttet = firsttet;
    }

    /// The member `orient3dfast()` (the same expression as Shewchuk's).
    fn orient3dfast_pts(&self, a: Point, b: Point, c: Point, d: Point) -> f64 {
        self.pred
            .orient3dfast(self.xyz(a), self.xyz(b), self.xyz(c), self.xyz(d))
    }

    /// `incrementaldelaunay()`: permute, sort, find four independent
    /// vertices, then insert every vertex with `insert_vertex_bw`.
    pub fn incrementaldelaunay(&mut self) -> Result<(), TetgenError> {
        let n = self.numberofpoints;
        // Form a random permuation (uniformly at random) of the set of vertices.
        let mut permutarray: Vec<Point> = alloc::vec![NULL_POINT; n];
        self.pointpool.traversalinit();

        if self.b.no_sort {
            for slot in permutarray.iter_mut() {
                *slot = self
                    .pointpool
                    .traverse()
                    .map(|s| s + 1)
                    .unwrap_or(NULL_POINT);
            }
        } else {
            self.crand.srand(n as u32);
            for i in 0..n {
                let randindex = (self.crand.rand() % (i as i32 + 1)) as usize;
                permutarray[i] = permutarray[randindex];
                permutarray[randindex] = self
                    .pointpool
                    .traverse()
                    .map(|s| s + 1)
                    .unwrap_or(NULL_POINT);
            }
            if self.b.brio_hilbert {
                self.hilbert_init(3); // in->mesh_dim
                let mut ngroup = 0;
                self.brio_multiscale_sort(
                    &mut permutarray,
                    self.b.brio_threshold,
                    self.b.brio_ratio,
                    &mut ngroup,
                );
            }
        }

        // Calculate the diagonal size of its bounding box.
        let dx = self.xmax - self.xmin;
        let dy = self.ymax - self.ymin;
        let dz = self.zmax - self.zmin;
        let bboxsize = (dx * dx + dy * dy + dz * dz).sqrt();
        let bboxsize2 = bboxsize * bboxsize;
        let bboxsize3 = bboxsize2 * bboxsize;
        let eps = self.b.epsilon;

        let distance = |m: &Self, a: Point, b: Point| {
            let p1 = m.xyz(a);
            let p2 = m.xyz(b);
            ((p2[0] - p1[0]) * (p2[0] - p1[0])
                + (p2[1] - p1[1]) * (p2[1] - p1[1])
                + (p2[2] - p1[2]) * (p2[2] - p1[2]))
                .sqrt()
        };

        // Make sure the second vertex is not identical with the first one.
        let mut i = 1;
        while (distance(self, permutarray[0], permutarray[i]) / bboxsize) < eps {
            i += 1;
            if i == n - 1 {
                return Err(TetgenError::AllIdentical);
            }
        }
        if i > 1 {
            permutarray.swap(i, 1);
        }

        // Make sure the third vertex is not collinear with the first two.
        i = 2;
        let mut v1 = [0.0f64; 3];
        let mut v2 = [0.0f64; 3];
        for j in 0..3 {
            v1[j] = self.xyz(permutarray[1])[j] - self.xyz(permutarray[0])[j];
            v2[j] = self.xyz(permutarray[i])[j] - self.xyz(permutarray[0])[j];
        }
        let cross = |v1: &[f64; 3], v2: &[f64; 3]| -> [f64; 3] {
            [
                v1[1] * v2[2] - v2[1] * v1[2],
                -(v1[0] * v2[2] - v2[0] * v1[2]),
                v1[0] * v2[1] - v2[0] * v1[1],
            ]
        };
        let mut nn = cross(&v1, &v2);
        while ((nn[0] * nn[0] + nn[1] * nn[1] + nn[2] * nn[2]).sqrt() / bboxsize2) < eps {
            i += 1;
            if i == n - 1 {
                return Err(TetgenError::AllCollinear);
            }
            for j in 0..3 {
                v2[j] = self.xyz(permutarray[i])[j] - self.xyz(permutarray[0])[j];
            }
            nn = cross(&v1, &v2);
        }
        if i > 2 {
            permutarray.swap(i, 2);
        }

        // Make sure the fourth vertex is not coplanar with the first three.
        i = 3;
        let mut ori = self.orient3dfast_pts(
            permutarray[0],
            permutarray[1],
            permutarray[2],
            permutarray[i],
        );
        while (ori.abs() / bboxsize3) < eps {
            i += 1;
            if i == n {
                return Err(TetgenError::AllCoplanar);
            }
            ori = self.orient3dfast_pts(
                permutarray[0],
                permutarray[1],
                permutarray[2],
                permutarray[i],
            );
        }
        if i > 3 {
            permutarray.swap(i, 3);
        }

        // Orient the first four vertices so that they follow the right-hand rule.
        if ori > 0.0 {
            permutarray.swap(0, 1);
        }

        // Create the initial Delaunay tetrahedralization.
        self.initialdelaunay(
            permutarray[0],
            permutarray[1],
            permutarray[2],
            permutarray[3],
        );

        let mut ivf = InsertVertexFlags {
            iloc: LocateResult::Unknown,
            bowywat: 1, // Use Bowyer-Watson algorithm
            lawson: 0,
        };

        for &p in permutarray.iter().skip(4) {
            if self.pointtype(p) == VertType::Unused {
                self.setpointtype(p, VertType::Vol);
            }
            let mut searchtet = TriFace::default();
            if self.b.brio_hilbert || self.b.no_sort {
                // Start the last updated tet.
                searchtet.tet = self.recenttet.tet;
            } else {
                // Randomly choose the starting tet for point location.
                searchtet.tet = NULL_TET;
            }
            ivf.iloc = LocateResult::Outside;
            // Insert the vertex.
            if !self.insert_vertex_bw(p, &mut searchtet, &mut ivf)? {
                if ivf.iloc == LocateResult::OnVertex {
                    // The point already exists. Mark it and do nothing on it.
                    let swapvertex = self.org(&searchtet);
                    self.points[p].point2ppt = swapvertex;
                    self.setpointtype(p, VertType::Duplicated);
                    self.dupverts += 1;
                } else if ivf.iloc == LocateResult::NearVertex {
                    return Err(TetgenError::InternalError);
                } else if ivf.iloc == LocateResult::NonRegular {
                    self.setpointtype(p, VertType::NRegular);
                    self.nonregularcount += 1;
                }
            }
        }
        Ok(())
    }

    /// `jettisonnodes()`: free duplicated and unused points and renumber
    /// the rest in pool order.
    pub fn jettisonnodes(&mut self) {
        self.pointpool.traversalinit();
        let mut newidx: i32 = 0;
        while let Some(p) = self.pointtraverse() {
            let jetflag = matches!(self.pointtype(p), VertType::Duplicated | VertType::Unused);
            if jetflag {
                // It is a duplicated or unused point, delete it.
                self.pointdealloc(p);
            } else {
                // Re-index it.
                self.points[p].mark = newidx + self.firstnumber;
                newidx += 1;
            }
        }
        self.dupverts = 0;
        self.unuverts = 0;
        // Dead points cannot be reallocated for new nodes.
        self.pointpool.deaditemstack.clear();
    }

    /// `outnodes()` (coordinates only, `-z`): the live points in pool
    /// order, and their marks.
    pub fn outnodes(&mut self) -> (Vec<[f64; 3]>, Vec<i32>) {
        let mut pts = Vec::with_capacity(self.pointpool.items);
        let mut marks = Vec::with_capacity(self.pointpool.items);
        self.pointpool.traversalinit();
        while let Some(p) = self.pointtraverse() {
            pts.push(self.points[p].xyz);
            marks.push(self.points[p].mark);
        }
        (pts, marks)
    }

    /// `outelements()` (`-z`, first-order, no attributes): the live
    /// non-hull tetrahedra in pool order, `[p0, p1, p2, p3]` by point mark;
    /// writes each record's `elemindex`.
    pub fn outelements(&mut self) -> Vec<[usize; 4]> {
        let ntets = self.tetpool.items as i64 - self.hullsize;
        let mut out = Vec::with_capacity(ntets.max(0) as usize);
        // firstindex = 0 (-z); shift = 0 since in->firstnumber == 0.
        let shift = 0;
        self.tetpool.traversalinit();
        let mut elementnumber: i32 = 0;
        while let Some(t) = self.tetrahedrontraverse() {
            let v = self.tets[t].v;
            out.push([
                (self.pointmark(v[0]) - shift) as usize,
                (self.pointmark(v[1]) - shift) as usize,
                (self.pointmark(v[2]) - shift) as usize,
                (self.pointmark(v[3]) - shift) as usize,
            ]);
            self.tets[t].elemindex = elementnumber;
            elementnumber += 1;
        }
        out
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn lcg_points(n: usize, seed: u64) -> Vec<[f64; 3]> {
        let mut s = seed;
        let mut next = || {
            s = s
                .wrapping_mul(6364136223846793005)
                .wrapping_add(1442695040888963407);
            (s >> 11) as f64 / (1u64 << 53) as f64
        };
        (0..n).map(|_| [next(), next(), next()]).collect()
    }

    /// The hull is closed, every live tet's neighbours point back, and
    /// `locate` finds each input point as a vertex.
    #[test]
    fn mesh_is_consistent_and_locate_finds_vertices() {
        let pts = lcg_points(300, 7);
        let mut m = TetgenMesh::new(Behavior::default());
        m.transfernodes(&pts).unwrap();
        m.incrementaldelaunay().unwrap();
        for t in 0..m.tets.len() {
            if m.isdeadtet(t) {
                continue;
            }
            for f in 0..4 {
                let tf = TriFace { tet: t, ver: f };
                let n = m.fsym(&tf);
                assert!(!m.isdeadtet(n.tet));
                let back = m.fsym(&n);
                assert_eq!(back.tet, t);
            }
        }
        for p in 1..=pts.len() {
            let mut st = m.recenttet;
            let loc = m.locate(p, &mut st, false).unwrap();
            assert_eq!(loc, LocateResult::OnVertex);
            assert_eq!(m.org(&st), p);
        }
        // An outside point.
        let far = m.makepoint(VertType::Unused);
        m.points[far].xyz = [5.0, 5.0, 5.0];
        let mut st = m.recenttet;
        assert_eq!(m.locate(far, &mut st, true).unwrap(), LocateResult::Outside);
        // -b0: randomsample.
        let mut b = Behavior::default();
        b.brio_hilbert = false;
        let r = delaunay_tetrahedralize_with(&pts, &b).unwrap();
        assert!(!r.tets.is_empty());
    }

    /// Degenerate inputs give upstream's errors.
    #[test]
    fn degenerate_inputs_are_errors() {
        assert_eq!(
            delaunay_tetrahedralize(&[[0.0; 3]; 3]).unwrap_err(),
            TetgenError::TooFewPoints
        );
        assert_eq!(
            delaunay_tetrahedralize(&[[1.0; 3]; 5]).unwrap_err(),
            TetgenError::TrivialPointSet
        );
        let flat: Vec<[f64; 3]> = (0..6).map(|i| [i as f64, (i * i) as f64, 0.0]).collect();
        assert_eq!(
            delaunay_tetrahedralize(&flat).unwrap_err(),
            TetgenError::AllCoplanar
        );
        let line: Vec<[f64; 3]> = (0..6).map(|i| [i as f64, 0.0, 0.0]).collect();
        assert_eq!(
            delaunay_tetrahedralize(&line).unwrap_err(),
            TetgenError::AllCollinear
        );
        let mut same = alloc::vec![[0.0; 3]; 5];
        same.push([1.0, 0.0, 0.0]);
        let e = delaunay_tetrahedralize(&same).unwrap_err();
        assert!(matches!(
            e,
            TetgenError::AllIdentical | TetgenError::AllCollinear
        ));
        assert_eq!(
            delaunay_tetrahedralize(&[[f64::NAN; 3]; 4]).unwrap_err(),
            TetgenError::NonFinite
        );
    }
}
