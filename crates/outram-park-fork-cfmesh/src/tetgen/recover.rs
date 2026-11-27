// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright (C) 2026 OUTRAM PARK contributors
//
// Port of TetGen 1.6.0 (boundary recovery: `tetgen.cxx` section
// `constrained_cxx`: `finddirection`, `scoutsegment`, `scoutsubface`;
// section `steiner_cxx`: `recoveredgebyflips`, `recoversegments`,
// `recoverfacebyflips`, `recoversubfaces`, `getedge`, `recoverboundary`;
// `makesegmentendpointsmap`; and the part of `tetrahedralize` that runs for
// `-p` up to and including `recoverboundary`):
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

//! Boundary recovery (stage 4, gh:#1793): `recoverboundary()`, what
//! `tetrahedralize()` runs under `-p` (without `-D`) after `meshsurface()`.
//!
//! Every input segment, then every subface, is searched in the Delaunay
//! tetrahedralisation of the PLC's vertices and, where missing, recovered:
//! first by flips only (`recoveredgebyflips`, `recoverfacebyflips`: the
//! crossing faces and edges are removed by `flip23` / `flipnm`, with the
//! flip link level raised each round while the number of missing items
//! falls), then with Steiner points in the volume, on segments and on
//! facets ([`super::steiner`]), which are afterwards removed again where
//! flips allow (`removevertexbyflips`). The segment and subface order is
//! upstream's random permutation (`randomnation`).
//!
//! The CDT path (`-D`: `constraineddelaunay`, `delaunizesegments`,
//! `constrainedfacets` and its cavity routines `formregion`,
//! `scoutcrossedge`, `formcavity`, `delaunizecavity`, `fillcavity`,
//! `carvecavity`, `restorecavity`, `flipcertify`, `flipinsertfacet`,
//! `insertpoint_cdt`, `refineregion`, `getsteinerptonsegment`) is **not**
//! reached by `recoverboundary` and is not ported (upstream calls them only
//! from `constraineddelaunay`).

#[cfg(not(any(feature = "std", test)))]
#[allow(unused_imports)]
use crate::nostd_prelude::*;

use super::delaunay::TetgenError;
use super::flip::{
    senext, senext2, sesym, FlipConstraints, ACROSSEDGE, ACROSSFACE, ACROSSVERT, DISJOINT,
    SELF_INTERSECT, SHAREEDGE, SHAREFACE, SHAREVERT, TOUCHFACE,
};
use super::mesh::{
    decode, encode, enextesymself, enextself, eprevesymself, eprevself, esymself, sdecode, sencode,
    Behavior, Face, Point, ShPool, TetgenMesh, TriFace, VertType, DUMMYPOINT, EORGOPPOTBL,
    NULL_HANDLE, NULL_POINT, NULL_SH, NULL_TET,
};
use super::plc::PlcInput;
use alloc::string::String;
use alloc::vec::Vec;
use core::fmt::Write;
use petir::mathf::RealMath;

/// The result of [`tetrahedralize_plc`]: ~~the mesh after boundary
/// recovery~~ **UPDATED 2026-11-26 (gh:#1794):** the mesh at the end of the
/// whole `-p` route, and its output.
#[derive(Debug, Clone)]
pub struct PlcTetrahedralization {
    /// The mesh object. Every subface is a face of it (upstream stops with
    /// an internal error otherwise); its Steiner points are counted in
    /// `st_segref_count`, `st_facref_count` and `st_volref_count`, its
    /// warnings are in `warnings`.
    pub mesh: TetgenMesh,
    /// The input segments left unrecovered (upstream's "Warning:  N
    /// segments are not recovered."); 0 on every committed case.
    pub missing_segments: usize,
    /// What `tetgen -p` writes (`.node`, `.ele`, `.face`, `.edge`); `None`
    /// with `-d` (stage 5, gh:#1794).
    pub output: Option<super::output::TetgenOutput>,
}

/// Tetrahedralise a PLC as `tetgen -p` does: ~~**up to and including
/// boundary recovery** (`recoverboundary`)~~ **UPDATED 2026-11-26
/// (gh:#1794): the whole `-p` route** (~~without `-q`~~ **UPDATED
/// 2026-11-26 (gh:#1795):** with `b.quality`, i.e. `-q` or `-a#`,
/// `delaunayrefinement` runs after `recoverdelaunay`): the Delaunay
/// tetrahedralisation of the vertices, `meshsurface`, then every segment
/// and subface recovered by flips and, where flips cannot, by Steiner
/// points (`recoverboundary`); then `carveholes` (the exterior and the
/// volume holes removed, the regions' attributes spread with `-A`),
/// Steiner-point suppression, `recoverdelaunay`, `smooth_vertices`,
/// `improve_mesh`, `jettisonnodes` and the output
/// ([`TetgenMesh::finish_plc`]). `b.plc` is set here; the other switches
/// are upstream's (`-Y` is `b.nobisect`, `-A` `b.regionattrib`; see
/// [`super::Switches`]).
///
/// The state after recovery equals an instrumented upstream's, slot for
/// slot, on the 19 cases x {`-p`, `-pY`} of
/// `verification_and_validation/tetgen_recoverboundary_gh1793/`; the output
/// files equal the unmodified upstream binary's byte for byte on the cases
/// of `verification_and_validation/tetgen_plc_gh1794/`. Upstream's default
/// (non-`-Q`) control flow is followed; its printed warnings are collected
/// in `TetgenMesh::warnings`.
///
/// ```
/// use outram_park_fork_cfmesh::tetgen::{tetrahedralize_plc, Behavior, PlcInput};
/// let poly = "8 3 0 0\n1 0 0 0\n2 1 0 0\n3 1 1 0\n4 0 1 0\n5 0 0 1\n6 1 0 1\n7 1 1 1\n8 0 1 1\n\
///             6 0\n1\n4 1 4 3 2\n1\n4 5 6 7 8\n1\n4 1 2 6 5\n1\n4 2 3 7 6\n1\n4 3 4 8 7\n\
///             1\n4 4 1 5 8\n0\n0\n";
/// let plc = PlcInput::read_poly(poly).unwrap();
/// let r = tetrahedralize_plc(&plc, &Behavior::default()).unwrap();
/// assert_eq!(r.missing_segments, 0);
/// assert_eq!(r.mesh.subfacepool.items, 12); // every subface recovered
/// ```
pub fn tetrahedralize_plc(
    input: &PlcInput,
    b: &Behavior,
) -> Result<PlcTetrahedralization, TetgenError> {
    let mut mesh = TetgenMesh::from_plc(input, b)?;
    let missing_segments = mesh.recoverboundary()?;
    let output = mesh.finish_plc(input)?;
    Ok(PlcTetrahedralization {
        mesh,
        missing_segments,
        output,
    })
}

impl TetgenMesh {
    /// `makesegmentendpointsmap()`: index every segment (its subsegments
    /// get the segment index as `facetindex`) and build the ridge-vertex to
    /// segment map.
    pub fn makesegmentendpointsmap(&mut self) {
        self.reach[37] += 1;
        let mut segptlist: Vec<[Point; 2]> = Vec::new();
        let n = self.pointpool.items + 2;
        self.idx_segment_ridge_vertex_list = alloc::vec![0; n];

        self.subsegpool.traversalinit();
        let mut segindex = 0;
        while let Some(slot) = self.shellfacetraverse(ShPool::Seg) {
            let segloop = Face {
                pool: ShPool::Seg,
                sh: slot,
                shver: 0,
            };
            let prevseg = self.spivot(&senext2(&segloop));
            if prevseg.is_null() {
                let eorg = self.sorg(&segloop);
                let mut edest = self.sdest(&segloop);
                self.setfacetindex(&segloop, segindex);
                let mut nextseg = self.spivot(&senext(&segloop));
                while !nextseg.is_null() {
                    self.setfacetindex(&nextseg, segindex);
                    nextseg.shver = 0;
                    if self.sorg(&nextseg) != edest {
                        nextseg.shver ^= 1;
                    }
                    edest = self.sdest(&nextseg);
                    // Go the next connected subsegment at edest.
                    nextseg = self.spivot(&senext(&nextseg));
                }
                segptlist.push([eorg, edest]);
                segindex += 1;
                let (mo, md) = (
                    self.pointmark(eorg) as usize,
                    self.pointmark(edest) as usize,
                );
                self.idx_segment_ridge_vertex_list[mo] += 1;
                self.idx_segment_ridge_vertex_list[md] += 1;
            }
        }

        self.segmentendpointslist = Vec::with_capacity(segptlist.len() * 2);
        for p in &segptlist {
            self.segmentendpointslist.push(p[0]);
            self.segmentendpointslist.push(p[1]);
        }

        // Create the adj_ridge_vertex_list.
        let items = self.pointpool.items;
        let mut j = self.idx_segment_ridge_vertex_list[0];
        self.idx_segment_ridge_vertex_list[0] = 0;
        let mut i = 0;
        while i < items + 1 {
            let k = self.idx_segment_ridge_vertex_list[i + 1];
            self.idx_segment_ridge_vertex_list[i + 1] = self.idx_segment_ridge_vertex_list[i] + j;
            j = k;
            i += 1;
        }

        let total_count = self.idx_segment_ridge_vertex_list[i] as usize + 1;
        self.segment_ridge_vertex_list = alloc::vec![NULL_POINT; total_count];
        for p in &segptlist {
            let (eorg, edest) = (p[0], p[1]);
            let j = self.pointmark(eorg) as usize;
            let k = self.pointmark(edest) as usize;
            let ij = self.idx_segment_ridge_vertex_list[j] as usize;
            self.segment_ridge_vertex_list[ij] = edest;
            let ik = self.idx_segment_ridge_vertex_list[k] as usize;
            self.segment_ridge_vertex_list[ik] = eorg;
            self.idx_segment_ridge_vertex_list[j] += 1;
            self.idx_segment_ridge_vertex_list[k] += 1;
        }

        // Counters in idx_adj_ridge_vertex_list[] are shifted by 1.
        let mut i = items as isize;
        while i >= 0 {
            self.idx_segment_ridge_vertex_list[i as usize + 1] =
                self.idx_segment_ridge_vertex_list[i as usize];
            i -= 1;
        }
        self.idx_segment_ridge_vertex_list[0] = 0;
    }

    /// `finddirection(searchtet, endpt)`: walk around the origin of
    /// `searchtet` to the tet whose face or edge the ray to `endpt` crosses
    /// (`ACROSSVERT`, `ACROSSEDGE` or `ACROSSFACE`).
    pub fn finddirection(
        &mut self,
        searchtet: &mut TriFace,
        endpt: Point,
    ) -> Result<i32, TetgenError> {
        self.reach[3] += 1;
        #[derive(PartialEq)]
        enum Move {
            H,
            R,
            L,
        }
        // The origin is fixed.
        let pa = self.org(searchtet);
        if self.tets[searchtet.tet].v[3] == DUMMYPOINT {
            // A hull tet. Choose the neighbor of its base face.
            *searchtet = decode(self.tets[searchtet.tet].nbr[3]);
            // Reset the origin to be pa.
            let v = self.tets[searchtet.tet].v;
            searchtet.ver = if v[0] == pa {
                11
            } else if v[1] == pa {
                3
            } else if v[2] == pa {
                7
            } else {
                0
            };
        }

        let mut pb = self.dest(searchtet);
        // Check whether the destination or apex is 'endpt'.
        if pb == endpt {
            return Ok(ACROSSVERT);
        }

        let mut pc = self.apex(searchtet);
        if pc == endpt {
            eprevesymself(searchtet);
            return Ok(ACROSSVERT);
        }

        // Walk through tets around pa until the right one is found.
        loop {
            let pd = self.oppo(searchtet);
            // Check whether the opposite vertex is 'endpt'.
            if pd == endpt {
                esymself(searchtet);
                enextself(searchtet);
                return Ok(ACROSSVERT);
            }
            // Check if we have entered outside of the domain.
            if pd == DUMMYPOINT {
                if self.nonconvex {
                    return Ok(ACROSSFACE);
                } else {
                    return Err(TetgenError::InternalError);
                }
            }

            let hori = self.ori3(pa, pb, pc, endpt);
            let rori = self.ori3(pb, pa, pd, endpt);
            let lori = self.ori3(pa, pc, pd, endpt);

            let nextmove;
            if hori > 0.0 {
                if rori > 0.0 {
                    if lori > 0.0 {
                        // Any of the three neighbors is a viable move.
                        let s = self.randomnation(3);
                        nextmove = if s == 0 {
                            Move::H
                        } else if s == 1 {
                            Move::R
                        } else {
                            Move::L
                        };
                    } else if self.randomnation(2) != 0 {
                        nextmove = Move::H;
                    } else {
                        nextmove = Move::R;
                    }
                } else if lori > 0.0 {
                    if self.randomnation(2) != 0 {
                        nextmove = Move::H;
                    } else {
                        nextmove = Move::L;
                    }
                } else {
                    nextmove = Move::H;
                }
            } else if rori > 0.0 {
                if lori > 0.0 {
                    if self.randomnation(2) != 0 {
                        nextmove = Move::R;
                    } else {
                        nextmove = Move::L;
                    }
                } else {
                    nextmove = Move::R;
                }
            } else if lori > 0.0 {
                nextmove = Move::L;
            } else {
                // 'endpt' lies either on the plane(s) or across face bcd.
                if hori == 0.0 {
                    if rori == 0.0 {
                        // pa->'endpt' is COLLINEAR with pa->pb.
                        return Ok(ACROSSVERT);
                    }
                    if lori == 0.0 {
                        // pa->'endpt' is COLLINEAR with pa->pc.
                        eprevesymself(searchtet); // [a,c,d]
                        return Ok(ACROSSVERT);
                    }
                    // pa->'endpt' crosses the edge pb->pc.
                    return Ok(ACROSSEDGE);
                }
                if rori == 0.0 {
                    if lori == 0.0 {
                        // pa->'endpt' is COLLINEAR with pa->pd.
                        esymself(searchtet); // face bad.
                        enextself(searchtet); // face [a,d,b]
                        return Ok(ACROSSVERT);
                    }
                    // pa->'endpt' crosses the edge pb->pd.
                    esymself(searchtet); // face bad.
                    enextself(searchtet); // face adb
                    return Ok(ACROSSEDGE);
                }
                if lori == 0.0 {
                    // pa->'endpt' crosses the edge pc->pd.
                    eprevesymself(searchtet); // [a,c,d]
                    return Ok(ACROSSEDGE);
                }
                // pa->'endpt' crosses the face bcd.
                return Ok(ACROSSFACE);
            }

            // Move to the next tet, fix pa as its origin.
            if nextmove == Move::R {
                self.fnextself(searchtet);
            } else if nextmove == Move::L {
                eprevself(searchtet);
                self.fnextself(searchtet);
                enextself(searchtet);
            } else {
                // HMOVE
                self.fsymself(searchtet);
                enextself(searchtet);
            }
            if self.org(searchtet) != pa {
                return Err(TetgenError::InternalError);
            }
            pb = self.dest(searchtet);
            pc = self.apex(searchtet);
        }
    }

    /// `scoutsegment(startpt, endpt, sedge, searchtet, refpt, NULL)`:
    /// `SHAREEDGE` if the edge exists, else the first crossing (and, with
    /// `want_refpt`, the vertex that most encroaches on the edge, or
    /// `NULL_POINT` if none does, in `refpt`).
    pub fn scoutsegment(
        &mut self,
        startpt: Point,
        endpt: Point,
        searchtet: &mut TriFace,
        refpt: Option<&mut Point>,
    ) -> Result<i32, TetgenError> {
        self.reach[4] += 1;
        *searchtet = self.point2tetorg(startpt);
        let mut dir = self.finddirection(searchtet, endpt)?;

        if dir == ACROSSVERT {
            let pd = self.dest(searchtet);
            if pd == endpt {
                if self.issubseg(searchtet) {
                    return Err(TetgenError::SelfIntersection);
                }
                return Ok(SHAREEDGE);
            } else {
                // A point is on the path.
                return Err(TetgenError::SelfIntersection);
            }
        }

        // dir is either ACROSSEDGE or ACROSSFACE.
        enextesymself(searchtet); // Go to the opposite face.
        self.fsymself(searchtet); // Enter the adjacent tet.

        if dir == ACROSSEDGE {
            if self.issubseg(searchtet) {
                return Err(TetgenError::SelfIntersection);
            }
        } else if dir == ACROSSFACE {
            if self.checksubfaceflag && self.issubface(searchtet) {
                return Err(TetgenError::SelfIntersection);
            }
        } else {
            return Err(TetgenError::InternalError);
        }

        let refpt = match refpt {
            None => return Ok(dir),
            Some(r) => r,
        };

        let mut types = [0i32; 2];
        let mut poss = [0i32; 4];
        let mut pos = 0;
        let (xs, xe) = (*self.xyz(startpt), *self.xyz(endpt));

        let pa = self.org(searchtet);
        let mut angmax = self.interiorangle(&self.points[pa].xyz.clone(), &xs, &xe, None);
        *refpt = pa;
        let pb = self.dest(searchtet);
        let ang = self.interiorangle(&self.points[pb].xyz.clone(), &xs, &xe, None);
        if ang > angmax {
            angmax = ang;
            *refpt = pb;
        }
        let pc = self.apex(searchtet);
        let ang = self.interiorangle(&self.points[pc].xyz.clone(), &xs, &xe, None);
        if ang > angmax {
            angmax = ang;
            *refpt = pc;
        }
        let mut reftet = *searchtet; // Save the tet containing the refpt.

        // Search intersecting faces along the segment.
        let mut neightet = TriFace::default();
        loop {
            let pd = self.oppo(searchtet);
            // Stop if we meet 'endpt'.
            if pd == endpt {
                break;
            }
            let ang = self.interiorangle(&self.points[pd].xyz.clone(), &xs, &xe, None);
            if ang > angmax {
                angmax = ang;
                *refpt = pd;
                reftet = *searchtet;
            }

            // Find a face intersecting the segment.
            if dir == ACROSSFACE {
                // One of the three oppo faces in 'searchtet' intersects the segment.
                neightet = *searchtet;
                let j = neightet.ver & 3; // j is the current face number.
                for i in j + 1..j + 4 {
                    neightet.ver = i % 4;
                    let (a, b, c, d) = (
                        self.org(&neightet),
                        self.dest(&neightet),
                        self.apex(&neightet),
                        self.oppo(&neightet),
                    );
                    if self.tri_edge_test(
                        a,
                        b,
                        c,
                        startpt,
                        endpt,
                        Some(d),
                        1,
                        &mut types,
                        &mut poss,
                    ) != 0
                    {
                        dir = types[0];
                        pos = poss[0];
                        break;
                    } else {
                        dir = DISJOINT;
                        pos = 0;
                    }
                }
            } else if dir == ACROSSEDGE {
                // Check the two opposite faces (of the edge) in 'searchtet'.
                for i in 0..2 {
                    neightet = if i == 0 {
                        super::mesh::enextesym(searchtet)
                    } else {
                        super::mesh::eprevesym(searchtet)
                    };
                    let (a, b, c, d) = (
                        self.org(&neightet),
                        self.dest(&neightet),
                        self.apex(&neightet),
                        self.oppo(&neightet),
                    );
                    if self.tri_edge_test(
                        a,
                        b,
                        c,
                        startpt,
                        endpt,
                        Some(d),
                        1,
                        &mut types,
                        &mut poss,
                    ) != 0
                    {
                        dir = types[0];
                        pos = poss[0];
                        break;
                    } else {
                        dir = DISJOINT;
                        pos = 0;
                    }
                }
                if dir == DISJOINT {
                    // No intersection. Rotate to the next tet at the edge.
                    dir = ACROSSEDGE;
                    self.fnextself(searchtet);
                    continue;
                }
            }

            if dir == ACROSSVERT {
                // This segment passing a vertex.
                return Err(TetgenError::SelfIntersection);
            } else if dir == ACROSSEDGE {
                // Get the edge intersects with the segment.
                for _ in 0..pos {
                    enextself(&mut neightet);
                }
            }
            // Go to the next tet.
            *searchtet = self.fsym(&neightet);

            if dir == ACROSSEDGE {
                if self.issubseg(searchtet) {
                    return Err(TetgenError::SelfIntersection);
                }
            } else if dir == ACROSSFACE {
                if self.checksubfaceflag && self.issubface(searchtet) {
                    return Err(TetgenError::SelfIntersection);
                }
            } else {
                return Err(TetgenError::InternalError);
            }
        }

        // A valid reference point should inside the diametrial circumsphere of
        //   the missing segment, i.e., it encroaches upon it.
        if 2.0 * angmax < core::f64::consts::PI {
            *refpt = NULL_POINT;
        }

        *searchtet = reftet;
        Ok(dir)
    }

    /// `getedge(e1, e2, tedge)`: find the edge `[e1, e2]` (true) in the
    /// mesh, `tedge` from `e1` to `e2`.
    pub fn getedge(
        &mut self,
        e1: Point,
        e2: Point,
        tedge: &mut TriFace,
    ) -> Result<bool, TetgenError> {
        self.reach[34] += 1;
        if e1 == NULL_POINT || e2 == NULL_POINT {
            return Ok(false);
        }
        if self.pointtype(e1) == VertType::Unused || self.pointtype(e2) == VertType::Unused {
            return Ok(false);
        }

        // Quickly check if 'tedge' is just this edge.
        if !self.isdeadtet(tedge.tet) {
            if self.org(tedge) == e1 {
                if self.dest(tedge) == e2 {
                    return Ok(true);
                }
            } else if self.org(tedge) == e2 && self.dest(tedge) == e1 {
                esymself(tedge);
                return Ok(true);
            }
        }

        // Search for the edge [e1, e2].
        *tedge = self.point2tetorg(e1);
        self.finddirection(tedge, e2)?;
        if self.dest(tedge) == e2 {
            return Ok(true);
        } else {
            // Search for the edge [e2, e1].
            *tedge = self.point2tetorg(e2);
            self.finddirection(tedge, e1)?;
            if self.dest(tedge) == e1 {
                esymself(tedge);
                return Ok(true);
            }
        }

        // Go to the link face of e1.
        let mut searchtet = self.point2tetorg(e1);
        enextesymself(&mut searchtet);

        // Search e2.
        for _ in 0..3 {
            let pt = self.apex(&searchtet);
            if pt == e2 {
                // Found. 'searchtet' is [#,#,e2,e1].
                *tedge = TriFace {
                    tet: searchtet.tet,
                    ver: EORGOPPOTBL[searchtet.ver],
                };
                return Ok(true);
            }
            enextself(&mut searchtet);
        }

        // Get the adjacent link face at 'searchtet'.
        let mut neightet = self.fnext(&searchtet);
        esymself(&mut neightet);
        let pt = self.apex(&neightet);
        if pt == e2 {
            *tedge = TriFace {
                tet: neightet.tet,
                ver: EORGOPPOTBL[neightet.ver],
            };
            return Ok(true);
        }

        // Continue searching in the link face of e1.
        // (upstream re-uses 'cavebdrylist' as 'tetlist')
        let mut tetlist = core::mem::take(&mut self.cavebdrylist);
        self.infect(searchtet.tet);
        tetlist.push(searchtet);
        self.infect(neightet.tet);
        tetlist.push(neightet);

        let mut done = false;
        let mut i = 0;
        while i < tetlist.len() && !done {
            let mut searchtet = tetlist[i];
            let mut j = 0;
            while j < 2 && !done {
                enextself(&mut searchtet);
                let mut neightet = self.fnext(&searchtet);
                if !self.infected(neightet.tet) {
                    esymself(&mut neightet);
                    let pt = self.apex(&neightet);
                    if pt == e2 {
                        // Found. 'neightet' is [#,#,e2,e1].
                        *tedge = TriFace {
                            tet: neightet.tet,
                            ver: EORGOPPOTBL[neightet.ver],
                        };
                        done = true;
                    } else {
                        self.infect(neightet.tet);
                        tetlist.push(neightet);
                    }
                }
                j += 1;
            }
            i += 1;
        }

        // Uninfect the list of visited tets.
        for t in &tetlist {
            self.uninfect(t.tet);
        }
        tetlist.clear();
        self.cavebdrylist = tetlist;

        Ok(done)
    }

    /// `recoveredgebyflips(startpt, endpt, sedge, searchtet, fullsearch,
    /// idir)`: recover the edge by removing the faces and edges it crosses.
    /// `sedge` is the segment or subface the edge belongs to (self-
    /// intersection reports). Returns `(recovered, idir)`.
    pub fn recoveredgebyflips(
        &mut self,
        startpt: Point,
        endpt: Point,
        sedge: Option<&Face>,
        searchtet: &mut TriFace,
        fullsearch: bool,
    ) -> Result<(bool, i32), TetgenError> {
        self.reach[2] += 1;
        let mut idir = DISJOINT; // init.
        let mut fc = FlipConstraints {
            checkflipeligibility: true,
            ..Default::default()
        };
        fc.seg = [startpt, endpt];

        // The mainloop of the edge reocvery.
        loop {
            // Search the edge from 'startpt'.
            *searchtet = self.point2tetorg(startpt);
            let mut dir = self.finddirection(searchtet, endpt)?;

            if dir == ACROSSVERT {
                if self.dest(searchtet) == endpt {
                    return Ok((true, idir)); // Edge is recovered.
                } else {
                    if let Some(sedge) = sedge {
                        // Check and report if there exists a self-intersection.
                        let intersect_flag;
                        let nearpt = self.dest(searchtet);
                        if self.shr(sedge).v[2] == NULL_POINT {
                            // It is a segment.
                            if !self.issteinerpoint(nearpt) {
                                // It is an input point.
                                let segidx = self.getfacetindex(sedge) as usize;
                                let p1 = self.segmentendpointslist[segidx * 2];
                                let p2 = self.segmentendpointslist[segidx * 2 + 1];
                                let tmppt = if self.is_segment(p1, nearpt) {
                                    p1
                                } else if self.is_segment(p2, nearpt) {
                                    p2
                                } else {
                                    NULL_POINT
                                };
                                if tmppt != NULL_POINT {
                                    self.warn(alloc::format!(
                                        "Warning:  Two line segments are exactly overlapping.\n  1st: [{},{}].\n  2nd: [{},{}].",
                                        self.pointmark(p1), self.pointmark(p2), self.pointmark(tmppt), self.pointmark(nearpt)
                                    ));
                                } else {
                                    self.warn(alloc::format!(
                                        "Warning:  A vertex lies exactly on a line segment.\n  1st: [{},{}].\n  2nd: [{}].",
                                        self.pointmark(p1), self.pointmark(p2), self.pointmark(nearpt)
                                    ));
                                }
                                intersect_flag = true;
                            } else if self.pointtype(nearpt) == VertType::FreeSeg {
                                // Check if two segments are (nearly) intersecting.
                                let segidx = self.getfacetindex(sedge);
                                let parsentseg = sdecode(self.point2sh(nearpt));
                                let segidx2 = self.getfacetindex(&parsentseg);
                                if segidx2 != segidx {
                                    self.warn(String::from(
                                        "Warning:  Two line segments are exactly crossing.",
                                    ));
                                    intersect_flag = true;
                                } else {
                                    return Err(TetgenError::InternalError);
                                }
                            } else {
                                // FREEFACETVERTEX: "very unlikely" (terminatetetgen);
                                // other cases: "to be checked".
                                return Err(TetgenError::InternalError);
                            }
                        } else {
                            // It is an edge of a facet.
                            if !self.issteinerpoint(nearpt) {
                                let (p1, p2, p3) =
                                    (self.sorg(sedge), self.sdest(sedge), self.sapex(sedge));
                                self.warn(alloc::format!(
                                    "Warning:  A vertex lies on a facet.\n  vertex: [{}]\n  facet triangle: [{},{},{}], tag({}).",
                                    self.pointmark(nearpt), self.pointmark(p1), self.pointmark(p2), self.pointmark(p3), self.shellmark(sedge)
                                ));
                                intersect_flag = true;
                            } else if self.pointtype(nearpt) == VertType::FreeSeg {
                                self.warn(String::from(
                                    "Warning:  A facet and a segment intersect.",
                                ));
                                intersect_flag = true;
                            } else if self.pointtype(nearpt) == VertType::FreeFacet {
                                self.warn(String::from("Warning:  Two facets intersect."));
                                intersect_flag = true;
                            } else {
                                // A FREEVOLVERTEX: not a real self-intersection.
                                return Err(TetgenError::InternalError);
                            }
                        }
                        if intersect_flag {
                            idir = SELF_INTERSECT;
                        }
                    }
                    return Ok((false, idir));
                }
            }

            // The edge is missing.

            // Try to remove the first intersecting face/edge.
            enextesymself(searchtet); // Go to the opposite face.

            if dir == ACROSSFACE {
                if self.checksubfaceflag && self.issubface(searchtet) {
                    if let Some(sedge) = sedge {
                        // A self-intersection is detected.
                        let is_seg = self.shr(sedge).v[2] == NULL_POINT;
                        if is_seg {
                            self.warn(String::from(
                                "Warning:  A segment and a facet exactly intersect.",
                            ));
                        } else {
                            self.warn(String::from("Warning:  Two facets exactly intersect."));
                        }
                        idir = SELF_INTERSECT;
                    }
                    return Ok((false, idir));
                }
                // Try to flip a crossing face.
                if self.removefacebyflips(searchtet, &mut fc)? {
                    continue;
                }
            } else if dir == ACROSSEDGE {
                if self.checksubsegflag && self.issubseg(searchtet) {
                    if let Some(sedge) = sedge {
                        // A self-intersection is detected.
                        let is_seg = self.shr(sedge).v[2] == NULL_POINT;
                        if is_seg {
                            let seg = self.tsspivot1(searchtet);
                            let segidx = self.getfacetindex(sedge);
                            let segidx2 = self.getfacetindex(&seg);
                            if segidx != segidx2 {
                                self.warn(String::from(
                                    "Warning:  Two segments exactly intersect.",
                                ));
                            } else {
                                return Err(TetgenError::InternalError);
                            }
                        } else {
                            self.warn(String::from(
                                "Warning:  A facet and a segment exactly intersect.",
                            ));
                        }
                        idir = SELF_INTERSECT;
                    }
                    return Ok((false, idir));
                }
                // Try to flip an intersecting edge.
                if self.removeedgebyflips(searchtet, &mut fc)? == 2 {
                    continue;
                }
            } else {
                return Err(TetgenError::InternalError);
            }

            // The edge is missing.

            if fullsearch {
                // Try to flip one of the faces/edges which intersects the edge.
                self.reach[38] += 1;
                let mut types = [0i32; 2];
                let mut poss = [0i32; 4];
                let mut pos = 0;
                let mut success = false;
                let mut neightet = TriFace::default();

                // Loop through the sequence of intersecting faces/edges from
                //   'startpt' to 'endpt'.
                *searchtet = self.point2tetorg(startpt);
                dir = self.finddirection(searchtet, endpt)?;

                // Go to the face/edge intersecting the searching edge.
                enextesymself(searchtet); // Go to the opposite face.
                                          // This face/edge has been tried in previous step.

                loop {
                    // Loop I-I
                    // Find the next intersecting face/edge.
                    self.fsymself(searchtet);
                    if dir == ACROSSFACE {
                        neightet = *searchtet;
                        let j = neightet.ver & 3; // j is the current face number.
                        for i in j + 1..j + 4 {
                            neightet.ver = i % 4;
                            let (a, b, c, d) = (
                                self.org(&neightet),
                                self.dest(&neightet),
                                self.apex(&neightet),
                                self.oppo(&neightet),
                            );
                            if self.tri_edge_test(
                                a,
                                b,
                                c,
                                startpt,
                                endpt,
                                Some(d),
                                1,
                                &mut types,
                                &mut poss,
                            ) != 0
                            {
                                dir = types[0];
                                pos = poss[0];
                                break;
                            } else {
                                dir = DISJOINT;
                                pos = 0;
                            }
                        }
                        // There must be an intersection face/edge.
                        if dir == DISJOINT {
                            return Err(TetgenError::InternalError);
                        }
                    } else if dir == ACROSSEDGE {
                        loop {
                            // Loop I-I-I
                            for i in 0..2 {
                                neightet = if i == 0 {
                                    super::mesh::enextesym(searchtet)
                                } else {
                                    super::mesh::eprevesym(searchtet)
                                };
                                let (a, b, c, d) = (
                                    self.org(&neightet),
                                    self.dest(&neightet),
                                    self.apex(&neightet),
                                    self.oppo(&neightet),
                                );
                                if self.tri_edge_test(
                                    a,
                                    b,
                                    c,
                                    startpt,
                                    endpt,
                                    Some(d),
                                    1,
                                    &mut types,
                                    &mut poss,
                                ) != 0
                                {
                                    dir = types[0];
                                    pos = poss[0];
                                    break;
                                } else {
                                    dir = DISJOINT;
                                    pos = 0;
                                }
                            }
                            if dir != DISJOINT {
                                // Find an intersection face/edge.
                                break;
                            }
                            // No intersection. Rotate to the next tet at the edge.
                            self.fnextself(searchtet);
                        }
                    } else {
                        return Err(TetgenError::InternalError);
                    }

                    // Adjust to the intersecting edge/vertex.
                    for _ in 0..pos {
                        enextself(&mut neightet);
                    }

                    if dir == SHAREVERT {
                        // Check if we have reached the 'endpt'.
                        let pd = self.org(&neightet);
                        if pd == endpt {
                            // Failed to recover the edge.
                            break;
                        } else {
                            return Ok((false, idir));
                        }
                    }

                    // The next to be flipped face/edge.
                    *searchtet = neightet;

                    // Bakup this face (tetrahedron).
                    let bak = [
                        self.org(searchtet),
                        self.dest(searchtet),
                        self.apex(searchtet),
                        self.oppo(searchtet),
                    ];

                    // Try to flip this intersecting face/edge.
                    if dir == ACROSSFACE {
                        if self.checksubfaceflag && self.issubface(searchtet) {
                            return Ok((false, idir));
                        }
                        if self.removefacebyflips(searchtet, &mut fc)? {
                            success = true;
                            break;
                        }
                    } else if dir == ACROSSEDGE {
                        if self.checksubsegflag && self.issubseg(searchtet) {
                            return Ok((false, idir));
                        }
                        if self.removeedgebyflips(searchtet, &mut fc)? == 2 {
                            success = true;
                            break;
                        }
                    } else if dir == ACROSSVERT {
                        return Ok((false, idir));
                    } else {
                        return Err(TetgenError::InternalError);
                    }

                    // The face/edge is not flipped.
                    if searchtet.tet == NULL_TET
                        || self.org(searchtet) != bak[0]
                        || self.dest(searchtet) != bak[1]
                        || self.apex(searchtet) != bak[2]
                        || self.oppo(searchtet) != bak[3]
                    {
                        // 'searchtet' was flipped. We must restore it.
                        *searchtet = self.point2tetorg(bak[0]);
                        let dir1 = self.finddirection(searchtet, bak[1])?;
                        if dir1 == ACROSSVERT {
                            if self.dest(searchtet) == bak[1] {
                                let mut spintet = *searchtet;
                                loop {
                                    if self.apex(&spintet) == bak[2] {
                                        // Found the face.
                                        *searchtet = spintet;
                                        break;
                                    }
                                    self.fnextself(&mut spintet);
                                    if spintet.tet == searchtet.tet {
                                        searchtet.tet = NULL_TET;
                                        break; // Not find.
                                    }
                                }
                                if searchtet.tet != NULL_TET && self.oppo(searchtet) != bak[3] {
                                    self.fsymself(searchtet);
                                    if self.oppo(searchtet) != bak[3] {
                                        // The original (intersecting) tet has been flipped.
                                        searchtet.tet = NULL_TET;
                                        break; // Not find.
                                    }
                                }
                            } else {
                                searchtet.tet = NULL_TET; // Not find.
                            }
                        } else {
                            searchtet.tet = NULL_TET; // Not find.
                        }
                        if searchtet.tet == NULL_TET {
                            success = false; // This face/edge has been destroyed.
                            break;
                        }
                    }
                }

                if success {
                    // One of intersecting faces/edges is flipped.
                    continue;
                }
            }

            // The edge is missing.
            break;
        }

        Ok((false, idir))
    }

    /// Record a warning upstream prints (`!b->quiet && !b->nowarning`).
    pub(crate) fn warn(&mut self, w: String) {
        self.warnings.push(w);
    }

    /// Bond a recovered segment to every tet at its edge
    /// (`sstbond1` + the `tssbond1` loop of `recoversegments`).
    fn insert_recovered_segment(&mut self, sseg: &Face, searchtet: &TriFace) {
        self.sstbond1(sseg, searchtet);
        let mut spintet = *searchtet;
        loop {
            self.tssbond1(&spintet, sseg);
            self.fnextself(&mut spintet);
            if spintet.tet == searchtet.tet {
                break;
            }
        }
    }

    /// `recoversegments(misseglist, fullsearch, steinerflag)`: recover the
    /// segments on `subsegstack`; the missing ones go to `misseglist`.
    pub fn recoversegments(
        &mut self,
        misseglist: &mut Vec<Face>,
        fullsearch: bool,
        steinerflag: i32,
    ) -> Result<(), TetgenError> {
        self.reach[1] += 1;
        // Loop until 'subsegstack' is empty.
        while let Some(mut sseg) = self.subsegstack.pop() {
            // Check if this segment has been recovered.
            let mut searchtet = self.sstpivot1(&sseg);
            if searchtet.tet != NULL_TET {
                continue; // Not a missing segment.
            }

            let startpt = self.sorg(&sseg);
            let endpt = self.sdest(&sseg);

            let mut success = false;
            let (ok, mut idir) =
                self.recoveredgebyflips(startpt, endpt, Some(&sseg), &mut searchtet, false)?;
            if ok {
                success = true;
            } else if idir != SELF_INTERSECT {
                // Try to recover it from the other direction.
                let (ok, d) =
                    self.recoveredgebyflips(endpt, startpt, Some(&sseg), &mut searchtet, false)?;
                idir = d;
                if ok {
                    success = true;
                }
            }

            if !success && fullsearch && idir != SELF_INTERSECT {
                let (ok, d) =
                    self.recoveredgebyflips(startpt, endpt, Some(&sseg), &mut searchtet, true)?;
                idir = d;
                if ok {
                    success = true;
                }
            }

            if success {
                // Segment is recovered. Insert it.
                self.insert_recovered_segment(&sseg, &searchtet);
            } else {
                if idir != SELF_INTERSECT && steinerflag > 0 {
                    // Try to recover the segment but do not split it.
                    let (ok, d) =
                        self.add_steinerpt_to_recover_edge(startpt, endpt, Some(&mut sseg), 0, 0)?;
                    idir = d;
                    if ok {
                        success = true;
                    }
                    if !success && idir != SELF_INTERSECT && steinerflag > 1 {
                        // Split the segment.
                        let (ok, d) = self.add_steinerpt_to_recover_edge(
                            startpt,
                            endpt,
                            Some(&mut sseg),
                            1,
                            0,
                        )?;
                        idir = d;
                        if ok {
                            success = true;
                        }
                    }
                }

                if !success {
                    if idir != SELF_INTERSECT {
                        // Save this segment (recover it later).
                        misseglist.push(sseg);
                    } else {
                        // Save this segment (do not recover it again).
                        self.skipped_segment_list.push(sseg);
                        self.smarktest3(&sseg);
                        // Save all subfaces at this segment, do not recover them later.
                        let mut ss = sseg;
                        ss.shver = 0;
                        let neighsh = self.spivot(&ss);
                        let mut spinsh = neighsh;
                        while !spinsh.is_null() {
                            self.skipped_facet_list
                                .get_or_insert_with(Vec::new)
                                .push(spinsh);
                            self.smarktest3(&spinsh); // do not recover it.
                            spinsh = self.spivot(&spinsh);
                            if spinsh.same_sh(&neighsh) {
                                break;
                            }
                        }
                    }
                }
            }
        }
        Ok(())
    }

    /// `scoutsubface(searchsh, searchtet, shflag)`: if the subface is a face
    /// of the mesh, bond it (both sides) and return true.
    pub fn scoutsubface(
        &mut self,
        searchsh: &mut Face,
        searchtet: &mut TriFace,
        shflag: bool,
    ) -> Result<bool, TetgenError> {
        self.reach[21] += 1;
        let pa = self.sorg(searchsh);
        let pb = self.sdest(searchsh);

        // Get a tet whose origin is a.
        *searchtet = self.point2tetorg(pa);
        // Search the edge [a,b].
        let dir = self.finddirection(searchtet, pb)?;
        if dir == ACROSSVERT {
            // Check validity of a PLC.
            if self.dest(searchtet) != pb {
                if shflag {
                    return Err(TetgenError::SelfIntersection);
                } else {
                    return Err(TetgenError::InternalError);
                }
            }
            // The edge exists. Check if the face exists.
            let pc = self.sapex(searchsh);
            let mut spintet = *searchtet;
            loop {
                if self.apex(&spintet) == pc {
                    // Found a face matching to 'searchsh'!
                    if !self.issubface(&spintet) {
                        // Insert 'searchsh'.
                        self.tsbond(&spintet, searchsh);
                        self.fsymself(&mut spintet);
                        *searchsh = sesym(searchsh);
                        self.tsbond(&spintet, searchsh);
                        *searchtet = spintet;
                        return Ok(true);
                    } else {
                        return Err(TetgenError::InternalError);
                    }
                }
                self.fnextself(&mut spintet);
                if spintet.tet == searchtet.tet {
                    break;
                }
            }
        }
        Ok(false)
    }

    /// `recoverfacebyflips(pa, pb, pc, searchsh, searchtet, dir, p1, p2)`:
    /// recover the face `[a,b,c]` (its edges exist) by removing the edges
    /// that cross it. Returns `(success, dir)`; `cross` gets the last
    /// crossing edge tried.
    #[allow(clippy::too_many_arguments)]
    pub fn recoverfacebyflips(
        &mut self,
        pa: Point,
        pb: Point,
        pc: Point,
        searchsh: &mut Face,
        searchtet: &mut TriFace,
        cross: &mut [Point; 2],
    ) -> Result<(bool, i32), TetgenError> {
        self.reach[20] += 1;
        let mut types = [0i32; 2];
        let mut poss = [0i32; 4];
        let mut fc = FlipConstraints {
            checkflipeligibility: true,
            ..Default::default()
        };
        fc.fac = [pa, pb, pc];

        let mut dir = DISJOINT;
        let mut success = false;

        let mut i = 0;
        while i < 3 && !success {
            loop {
                // Get a tet containing the edge [a,b].
                *searchtet = self.point2tetorg(fc.fac[i]);
                self.finddirection(searchtet, fc.fac[(i + 1) % 3])?;
                // Search the face [a,b,c]
                let mut spintet = *searchtet;
                loop {
                    if self.apex(&spintet) == fc.fac[(i + 2) % 3] {
                        // Found the face.
                        *searchtet = spintet;
                        // Return the face [a,b,c].
                        for _ in 0..i {
                            eprevself(searchtet);
                        }
                        dir = SHAREFACE;
                        success = true;
                        break;
                    }
                    self.fnextself(&mut spintet);
                    if spintet.tet == searchtet.tet {
                        break;
                    }
                }

                if success {
                    break;
                }

                // The face is missing. Try to recover it.
                let mut flipedge;
                // Find a crossing edge of this face.
                let mut spintet = *searchtet;
                loop {
                    let pd = self.apex(&spintet);
                    let pe = self.oppo(&spintet);
                    if pd != DUMMYPOINT && pe != DUMMYPOINT {
                        // Check if [d,e] intersects [a,b,c]
                        let intflag =
                            self.tri_edge_test(pa, pb, pc, pd, pe, None, 1, &mut types, &mut poss);
                        if intflag > 0 {
                            // By the assumption that all edges of the face exist, they can
                            //   only intersect at a single point.
                            if intflag == 2 {
                                // Go to the edge [d,e].
                                flipedge = super::flip::edestoppo(&spintet); // [d,e,a,b]
                                if !searchsh.is_null() {
                                    // Check the intersection type.
                                    dir = types[0]; // return this value.
                                    if types[0] == ACROSSFACE || types[0] == ACROSSEDGE {
                                        // Check if [e,d] is a segment.
                                        if self.issubseg(&flipedge) {
                                            self.warn(String::from(
                                                "Warning:  A segment and a facet intersect.",
                                            ));
                                            dir = SELF_INTERSECT;
                                            return Ok((false, dir)); // Found a self-intersection.
                                        } else {
                                            // Check if [e,d] is an edge of a subface.
                                            let mut chkface = flipedge;
                                            loop {
                                                if self.issubface(&chkface) {
                                                    break;
                                                }
                                                self.fsymself(&mut chkface);
                                                if chkface.tet == flipedge.tet {
                                                    break;
                                                }
                                            }
                                            if self.issubface(&chkface) {
                                                // Two subfaces are intersecting.
                                                self.warn(String::from(
                                                    "Warning:  Found two facets intersect.",
                                                ));
                                                dir = SELF_INTERSECT;
                                                return Ok((false, dir)); // Found a self-intersection.
                                            }
                                        }
                                    } else if types[0] == TOUCHFACE {
                                        // This is possible when a Steiner point was added on it.
                                        let touchpt = if poss[1] == 0 { pd } else { pe };
                                        if !self.issteinerpoint(touchpt) {
                                            self.warn(alloc::format!(
                                                "Warning:  A vertex lies on a facet.\n  vertex : [{}]",
                                                self.pointmark(touchpt)
                                            ));
                                            dir = SELF_INTERSECT;
                                            return Ok((false, dir));
                                        } else if self.pointtype(touchpt) == VertType::FreeSeg {
                                            self.warn(String::from(
                                                "Warning:  A segment and a facet intersect.",
                                            ));
                                            dir = SELF_INTERSECT;
                                            return Ok((false, dir));
                                        } else if self.pointtype(touchpt) == VertType::FreeFacet {
                                            self.warn(String::from(
                                                "Warning:  Found two facets intersect.",
                                            ));
                                            dir = SELF_INTERSECT;
                                            return Ok((false, dir));
                                        } else if self.pointtype(touchpt) == VertType::FreeVol {
                                            // A volume Steiner point was added in this subface.
                                            // Split this subface by this point.
                                            self.setpointtype(touchpt, VertType::FreeFacet);
                                            self.sinsertvertex_full(
                                                touchpt,
                                                searchsh,
                                                None,
                                                super::delaunay::LocateResult::OnFace,
                                                0,
                                                false,
                                            );
                                            self.st_volref_count -= 1;
                                            self.st_facref_count += 1;
                                            // Queue this vertex for removal.
                                            self.subvertstack.push(touchpt);
                                            // Queue new subfaces for recovery.
                                            for k in 0..self.caveshbdlist.len() {
                                                let parysh = self.caveshbdlist[k];
                                                let checksh = self.spivot(&parysh); // The new subface [a, b, p].
                                                                                    // Do not recover a deleted new face (degenerated).
                                                if self.shr(&checksh).v[0] != NULL_POINT {
                                                    self.subfacstack.push(checksh);
                                                }
                                            }
                                            // Delete the old subfaces in sC(p).
                                            for k in 0..self.caveshlist.len() {
                                                let s = self.caveshlist[k];
                                                self.shellfacedealloc(ShPool::Sub, s.sh);
                                            }
                                            // Clear working lists.
                                            self.caveshlist.clear();
                                            self.caveshbdlist.clear();
                                            self.cavesegshlist.clear();
                                            // We can return this function.
                                            searchsh.sh = NULL_SH; // It has been split.
                                            return Ok((true, dir));
                                        } else {
                                            // Other cases may be due to a bug or a PLC error.
                                            return Err(TetgenError::InternalError);
                                        }
                                    } else {
                                        // ACROSSVERT, TOUCHEDGE, SHAREVERTEX: not possible
                                        //   or due to a PLC error.
                                        return Err(TetgenError::InternalError);
                                    }
                                }
                            } else {
                                // intflag == 4. Coplanar case.
                                return Err(TetgenError::InternalError);
                            }
                            break;
                        }
                    }
                    self.fnextself(&mut spintet);
                    if spintet.tet == searchtet.tet {
                        return Err(TetgenError::InternalError);
                    }
                }
                // Try to flip the edge [d,e].
                // Remember a crossing edge.
                cross[0] = self.org(&flipedge);
                cross[1] = self.dest(&flipedge);

                if self.removeedgebyflips(&mut flipedge, &mut fc)? == 2 {
                    // A crossing edge is removed.
                    continue;
                }

                // Unable to remove a crossing edge of this face.
                break;
            }
            i += 1;
        }

        Ok((success, dir))
    }

    /// Delete the temporary segments `recoversubfaces` put on the edges
    /// `bdsegs[js]` of a subface (`smarktest2ed`).
    fn delete_temporary_segments(&mut self, bdsegs: &[Face; 3], js: impl Iterator<Item = usize>) {
        for j in js {
            if !bdsegs[j].is_null() && self.smarktest2ed(&bdsegs[j]) {
                let neineish = self.spivot(&bdsegs[j]);
                self.ssdissolve(&neineish);
                let neighsh = self.spivot(&neineish);
                if !neighsh.is_null() {
                    self.ssdissolve(&neighsh);
                }
                let searchtet = self.sstpivot1(&bdsegs[j]);
                let mut spintet = searchtet;
                loop {
                    self.tssdissolve1(&spintet);
                    self.fnextself(&mut spintet);
                    if spintet.tet == searchtet.tet {
                        break;
                    }
                }
                self.shellfacedealloc(ShPool::Seg, bdsegs[j].sh);
            }
        }
    }

    /// `recoversubfaces(misshlist, steinerflag)`: recover the subfaces on
    /// `subfacstack`; the missing ones go to `misshlist`.
    pub fn recoversubfaces(
        &mut self,
        misshlist: &mut Vec<Face>,
        steinerflag: i32,
    ) -> Result<(), TetgenError> {
        self.reach[19] += 1;
        let mut cross = [NULL_POINT; 2]; // endpoints of a crossing edge.

        // Loop until 'subfacstack' is empty.
        while let Some(mut searchsh) = self.subfacstack.pop() {
            if self.shr(&searchsh).v[0] == NULL_POINT {
                continue; // Skip a dead subface.
            }
            if self.smarktest3ed(&searchsh) {
                continue; // Skip a self-intersected subface.
            }
            let neightet = self.stpivot(&searchsh);
            if neightet.tet != NULL_TET {
                continue; // Skip a recovered subface.
            }

            let mut dir = DISJOINT; // No self intersection is detected.
            let mut success = false;
            let mut searchtet = TriFace::default();
            let mut bdsegs = [Face::default(); 3];
            let (mut startpt, mut endpt) = (NULL_POINT, NULL_POINT);

            // The three edges of the face need to be existed first.
            let mut i = 0;
            while i < 3 {
                bdsegs[i] = self.sspivot(&searchsh);
                if !bdsegs[i].is_null() {
                    // Check if this segment exist.
                    searchtet = self.sstpivot1(&bdsegs[i]);
                    if searchtet.tet == NULL_TET {
                        // This segment is not recovered yet. Try to recover it.
                        success = false;
                        startpt = self.sorg(&searchsh);
                        endpt = self.sdest(&searchsh);
                        let seg = bdsegs[i];
                        let (ok, d) = self.recoveredgebyflips(
                            startpt,
                            endpt,
                            Some(&seg),
                            &mut searchtet,
                            false,
                        )?;
                        dir = d;
                        if ok {
                            success = true;
                        } else if dir != SELF_INTERSECT {
                            let (ok, d) = self.recoveredgebyflips(
                                endpt,
                                startpt,
                                Some(&seg),
                                &mut searchtet,
                                false,
                            )?;
                            dir = d;
                            if ok {
                                success = true;
                            }
                        }
                        if success {
                            // Segment is recovered. Insert it.
                            self.insert_recovered_segment(&seg, &searchtet);
                        } else {
                            // An edge of this subface is missing. Can't recover this subface.
                            // Delete any temporary segment that has been created.
                            self.delete_temporary_segments(&bdsegs, (0..i).rev());
                            break; // i
                        }
                    }
                } else {
                    // This edge is not a segment. Check whether it exists or not.
                    success = false;
                    startpt = self.sorg(&searchsh);
                    endpt = self.sdest(&searchsh);
                    searchtet = self.point2tetorg(startpt);
                    self.finddirection(&mut searchtet, endpt)?;
                    if self.dest(&searchtet) == endpt {
                        success = true; // Found this edge.
                    } else {
                        // The edge is missing. Try to recover it.
                        let sh = searchsh;
                        let (ok, d) = self.recoveredgebyflips(
                            startpt,
                            endpt,
                            Some(&sh),
                            &mut searchtet,
                            false,
                        )?;
                        dir = d;
                        if ok {
                            success = true;
                        } else if dir != SELF_INTERSECT {
                            let (ok, d) = self.recoveredgebyflips(
                                endpt,
                                startpt,
                                Some(&sh),
                                &mut searchtet,
                                false,
                            )?;
                            dir = d;
                            if ok {
                                success = true;
                            }
                        }
                    }

                    if success && dir != SELF_INTERSECT {
                        // This edge exists.
                        // Insert a temporary segment to protect this edge.
                        bdsegs[i] = self.makeshellface(ShPool::Seg);
                        self.setshvertices(&bdsegs[i], startpt, endpt, NULL_POINT);
                        self.smarktest2(&bdsegs[i]); // It's a temporary segment.
                                                     // Insert this segment into surface mesh.
                        self.ssbond(&searchsh, &bdsegs[i]);
                        let neighsh = self.spivot(&searchsh);
                        if !neighsh.is_null() {
                            self.ssbond(&neighsh, &bdsegs[i]);
                        }
                        // Insert this segment into tetrahedralization.
                        let seg = bdsegs[i];
                        self.insert_recovered_segment(&seg, &searchtet);
                    } else {
                        // An edge of this subface is missing. Can't recover this subface.
                        // Delete any temporary segment that has been created.
                        self.delete_temporary_segments(&bdsegs, (0..i).rev());
                        break;
                    }
                }
                searchsh = senext(&searchsh);
                i += 1;
            }

            if i == 3 {
                // All edges of this subface exist (or have been recovered).
                // Recover the subface.
                startpt = self.sorg(&searchsh);
                endpt = self.sdest(&searchsh);
                let apexpt = self.sapex(&searchsh);

                let (ok, d) = self.recoverfacebyflips(
                    startpt,
                    endpt,
                    apexpt,
                    &mut searchsh,
                    &mut searchtet,
                    &mut cross,
                )?;
                success = ok;
                dir = d;

                // Delete any temporary segment that has been created.
                self.delete_temporary_segments(&bdsegs, 0..3);

                if success {
                    if !searchsh.is_null() {
                        // Face is recovered. Insert it.
                        let chkface = self.tspivot(&searchtet);
                        if chkface.is_null() {
                            self.tsbond(&searchtet, &searchsh);
                            self.fsymself(&mut searchtet);
                            searchsh = sesym(&searchsh);
                            self.tsbond(&searchtet, &searchsh);
                        } else if self.shellmark(&chkface) == self.shellmark(&searchsh) {
                            // A duplicated facet is found.
                            let v = self.shr(&searchsh).v;
                            self.warn(alloc::format!(
                                "Warning:  A duplicated triangle ({},{},{}) tag({}) is ignored.",
                                self.pointmark(v[0]),
                                self.pointmark(v[1]),
                                self.pointmark(v[2]),
                                self.shellmark(&searchsh)
                            ));
                            self.duplicated_facets_count += 1;
                            self.reach[50] += 1;
                            self.smarktest3(&searchsh); // do not recover it.
                            self.sinfect(&searchsh); // it is an igonred duplicated facet.
                        } else {
                            self.warn(String::from("Warning:  Two facets are overlapping."));
                            dir = SELF_INTERSECT;
                            success = false;
                        }
                    }
                } else if dir != SELF_INTERSECT && steinerflag != 0 {
                    self.reach[39] += 1;
                    success = self.recoversubfaces_steiner_in_face(
                        &mut searchsh,
                        &mut searchtet,
                        [startpt, endpt, apexpt],
                        cross,
                        steinerflag,
                        &mut dir,
                    )?;
                }
            } else {
                // when i < 3: an edge (startpt, endpt) of this subface is missing.
                if dir != SELF_INTERSECT && steinerflag > 0 {
                    self.reach[40] += 1;
                    success = self.recoversubfaces_steiner_on_edge(
                        &mut searchsh,
                        &mut searchtet,
                        startpt,
                        endpt,
                        steinerflag,
                        &mut dir,
                    )?;
                }
            }

            if success {
                continue; // recover the next subface.
            }

            if dir == SELF_INTERSECT {
                // Found a self-intersection. This subface cannot be recovered.
                self.skipped_facet_list
                    .get_or_insert_with(Vec::new)
                    .push(searchsh);
                self.smarktest3(&searchsh); // do not recover it later.
                continue; // recover the next subface.
            }

            // This subface is missing.
            if steinerflag >= 2 {
                return Err(TetgenError::InternalError);
            }

            // Save this subface to recover it later.
            misshlist.push(searchsh);
        }
        Ok(())
    }

    /// Put the subsegments (`pool == Seg`, on `subsegstack`) or subfaces
    /// (on `subfacstack`) on their stack in upstream's random order: the
    /// `i`-th item of the pool swaps with entry `randomnation(i + 1)`
    /// (absolute indices into the stack, as upstream's `fastlookup`).
    fn random_stack(&mut self, pool: ShPool) {
        let items = match pool {
            ShPool::Seg => self.subsegpool.items,
            ShPool::Sub => self.subfacepool.items,
        };
        match pool {
            ShPool::Seg => self.subsegpool.traversalinit(),
            ShPool::Sub => self.subfacepool.traversalinit(),
        }
        for i in 0..items {
            let s = self.randomnation(i as u32 + 1) as usize;
            let sh = {
                // Move the s-th item to the i-th.
                let stack = match pool {
                    ShPool::Seg => &mut self.subsegstack,
                    ShPool::Sub => &mut self.subfacstack,
                };
                stack.push(Face::default());
                let last = stack.len() - 1;
                stack[last] = stack[s];
                0
            };
            let _ = sh;
            // Put i-th item to be the s-th.
            let sh = self.shellfacetraverse(pool).unwrap_or(NULL_SH);
            let stack = match pool {
                ShPool::Seg => &mut self.subsegstack,
                ShPool::Sub => &mut self.subfacstack,
            };
            stack[s] = Face { pool, sh, shver: 0 };
        }
    }

    /// `recoverboundary(tv)`: recover every segment, then every subface.
    /// Returns the number of segments left unrecovered (upstream warns
    /// "N segments are not recovered").
    pub fn recoverboundary(&mut self) -> Result<usize, TetgenError> {
        self.reach = [0; 64];
        self.reach[0] += 1;
        let mut bdrysteinerptlist: Vec<Point> = Vec::new();

        self.boundary_recovery_flag = true;
        self.cos_collinear_ang_tol =
            (self.b.collinear_ang_tol / 180.0 * core::f64::consts::PI).r_cos();

        if self.segmentendpointslist.is_empty() {
            // We need segment adjacent information during flips.
            self.makesegmentendpointsmap();
        }

        // Segments will be introduced.
        self.checksubsegflag = true;

        let mut misseglist: Vec<Face> = Vec::new();

        // In random order.
        self.random_stack(ShPool::Seg);

        // The init number of missing segments.
        let mut ms = self.subsegpool.items;
        let mut nit = 0;
        if self.b.fliplinklevel < 0 {
            self.autofliplinklevel = 1; // Init value.
        }

        // First, trying to recover segments by only doing flips.
        loop {
            self.recoversegments(&mut misseglist, false, 0)?;

            if !misseglist.is_empty() {
                if self.b.fliplinklevel >= 0 {
                    break;
                } else {
                    if misseglist.len() >= ms {
                        nit += 1;
                        if nit >= 3 {
                            // Do the last round with unbounded flip link level.
                            self.b.fliplinklevel = 100000;
                        }
                    } else {
                        ms = misseglist.len();
                        if nit > 0 {
                            nit -= 1;
                        }
                    }
                    self.subsegstack.append(&mut misseglist);
                    self.autofliplinklevel += self.b.fliplinklevelinc;
                }
            } else {
                // All segments are recovered.
                break;
            }
        }

        if !misseglist.is_empty() {
            // Second, trying to recover segments by doing more flips (fullsearch).
            while !misseglist.is_empty() {
                ms = misseglist.len();
                self.subsegstack.append(&mut misseglist);
                self.recoversegments(&mut misseglist, true, 0)?;
                if misseglist.len() < ms {
                    continue;
                } else {
                    break;
                }
            }
        }

        if !misseglist.is_empty() {
            // Third, trying to recover segments by doing more flips (fullsearch)
            //   and adding Steiner points in the volume.
            self.recoverdelaunay()?;
            while !misseglist.is_empty() {
                ms = misseglist.len();
                self.subsegstack.append(&mut misseglist);
                self.recoversegments(&mut misseglist, false, 1)?; // no full search
                if misseglist.len() < ms {
                    continue;
                } else {
                    break;
                }
            }
        }

        if !misseglist.is_empty() {
            // Last, trying to recover segments by doing more flips (fullsearch),
            //   and adding Steiner points in the volume, and splitting segments.
            self.recoverdelaunay()?;
            while !misseglist.is_empty() {
                ms = misseglist.len();
                self.subsegstack.append(&mut misseglist);
                self.recoversegments(&mut misseglist, false, 2)?; // no full search
                if misseglist.len() < ms {
                    continue;
                } else {
                    break;
                }
            }
        }

        if !self.skipped_segment_list.is_empty() {
            let n = self.skipped_segment_list.len();
            self.warn(alloc::format!(
                "  Skipped {n} segments due to intersections."
            ));
        }

        if self.st_segref_count > 0 {
            // Try to remove the Steiner points added in segments.
            let bak_fliplinklevel = self.b.fliplinklevel;
            self.b.fliplinklevel = 20; // limit this value
            let mut i = 0;
            while i < self.subvertstack.len() {
                // Get the Steiner point.
                let rempt = self.subvertstack[i];
                if !self.removevertexbyflips(rempt)? {
                    // Save it in list.
                    bdrysteinerptlist.push(rempt);
                }
                i += 1;
            }
            self.b.fliplinklevel = bak_fliplinklevel; // restore it.
            self.subvertstack.clear();
        }

        // Subfaces will be introduced.
        self.checksubfaceflag = true;

        let mut misshlist: Vec<Face> = Vec::new();

        // Randomly order the subfaces.
        self.random_stack(ShPool::Sub);

        ms = self.subfacepool.items;
        nit = 0;
        self.b.fliplinklevel = -1; // Init.
        if self.b.fliplinklevel < 0 {
            self.autofliplinklevel = 1; // Init value.
        }

        loop {
            self.recoversubfaces(&mut misshlist, 0)?;

            if !misshlist.is_empty() {
                if self.b.fliplinklevel >= 0 {
                    break;
                } else {
                    if misshlist.len() >= ms {
                        nit += 1;
                        if nit >= 3 {
                            // Do the last round with unbounded flip link level.
                            if self.autofliplinklevel < 30 {
                                self.b.fliplinklevel = 30;
                            } else {
                                self.b.fliplinklevel = self.autofliplinklevel + 30;
                            }
                        }
                    } else {
                        ms = misshlist.len();
                        if nit > 0 {
                            nit -= 1;
                        }
                    }
                    self.subfacstack.append(&mut misshlist);
                    self.autofliplinklevel += self.b.fliplinklevelinc;
                }
            } else {
                // All subfaces are recovered.
                break;
            }
        }

        if !misshlist.is_empty() {
            // There are missing subfaces. Add Steiner points.
            self.recoverdelaunay()?;
            while !misshlist.is_empty() {
                ms = misshlist.len();
                self.subfacstack.append(&mut misshlist);
                self.recoversubfaces(&mut misshlist, 1)?;
                if misshlist.len() < ms {
                    continue;
                } else {
                    break;
                }
            }
        }

        if !misshlist.is_empty() {
            self.recoverdelaunay()?;
            while !misshlist.is_empty() {
                ms = misshlist.len();
                self.subfacstack.append(&mut misshlist);
                self.recoversubfaces(&mut misshlist, 2)?; // steinerflag = 2;
                if misshlist.len() < ms {
                    continue;
                } else {
                    break;
                }
            }

            if !self.subsegstack.is_empty() {
                // Save unrecovered subsegments.
                let stack = core::mem::take(&mut self.subsegstack);
                for checkseg in stack {
                    if checkseg.is_null() || self.shr(&checkseg).v[0] == NULL_POINT {
                        continue;
                    }
                    // Check if this subsegment is missing.
                    let neightet = self.sstpivot1(&checkseg);
                    if neightet.tet != NULL_TET {
                        continue;
                    }
                    // Save a missing subsegment.
                    misseglist.push(checkseg);
                }
            }
        }

        // There may be un-recovered subsegments.
        if !misshlist.is_empty() {
            return Err(TetgenError::InternalError);
        }

        if self.duplicated_facets_count > 0 {
            self.subfacepool.traversalinit();
            while let Some(slot) = self.shellfacetraverse(ShPool::Sub) {
                let mut faceloop = Face {
                    pool: ShPool::Sub,
                    sh: slot,
                    shver: 0,
                };
                if self.sinfected(&faceloop) {
                    // Delete an ignored duplicated subface.
                    self.shellfacedealloc(ShPool::Sub, slot);
                }
                if !self.smarktest3ed(&faceloop) {
                    faceloop.shver = 0;
                    let mut neightet = self.stpivot(&faceloop);
                    if neightet.tet == NULL_TET {
                        return Err(TetgenError::InternalError);
                    }
                    // Update the subface connections at its three edges.
                    for _ in 0..3 {
                        let sseg = self.sspivot(&faceloop);
                        if !sseg.is_null() {
                            self.ssbond(&faceloop, &sseg); // Update segment connection.
                        }
                        // Get all subfaces at this edge.
                        let mut sfaces: Vec<Face> = Vec::new();
                        let mut spintet = neightet;
                        loop {
                            if self.issubface(&spintet) {
                                sfaces.push(self.tspivot(&spintet));
                                if sfaces.len() > 256 {
                                    return Err(TetgenError::InternalError);
                                }
                            }
                            self.fnextself(&mut spintet);
                            if spintet.tet == neightet.tet {
                                break;
                            }
                        }
                        // Re-create the face ring.
                        let snum = sfaces.len();
                        for j in 0..snum - 1 {
                            self.sbond1(&sfaces[j], &sfaces[j + 1]);
                        }
                        self.sbond1(&sfaces[snum - 1], &sfaces[0]);
                        enextself(&mut neightet);
                        faceloop = senext(&faceloop);
                    }
                }
            }
        }

        if self.st_facref_count > 0 {
            // Try to remove the Steiner points added in facets.
            let bak_fliplinklevel = self.b.fliplinklevel;
            self.b.fliplinklevel = 30; // limit this value
            let mut i = 0;
            while i < self.subvertstack.len() {
                let rempt = self.subvertstack[i];
                if !self.removevertexbyflips(rempt)? {
                    // Save it in list.
                    bdrysteinerptlist.push(rempt);
                }
                i += 1;
            }
            self.b.fliplinklevel = bak_fliplinklevel;
            self.subvertstack.clear();
        }

        // There may be missing segments and subfaces.
        let mut notrecovered = 0;
        if !misseglist.is_empty() {
            for checkseg in misseglist.iter() {
                // A saved missing segment might be split or recovered.
                if checkseg.is_null() || self.shr(checkseg).v[0] == NULL_POINT {
                    continue; // it is split.
                }
                let adjtet = self.sstpivot1(checkseg);
                if adjtet.tet != NULL_TET {
                    continue; // it is recovered.
                }
                // This is a missing segmemt.
                self.subsegstack.push(*checkseg);
            }
            if !self.subsegstack.is_empty() {
                notrecovered = self.subsegstack.len();
                self.warn(alloc::format!(
                    "Warning:  {notrecovered} segments are not recovered."
                ));
                self.subsegstack.clear();
            }
        }

        self.boundary_recovery_flag = false;
        let _ = bdrysteinerptlist;
        Ok(notrecovered)
    }

    /// The state right after `recoverboundary()` in the format of the
    /// instrumented upstream build (`dump_recoverboundary`, see
    /// `verification_and_validation/tetgen_recoverboundary_gh1793/`): every
    /// point (with `point2tet` and `point2sh`), subface, subsegment (with
    /// the facet index) and tetrahedron slot (with its six segment and four
    /// subface links) in pool order, the dead-item stacks, `recentsh`,
    /// `recenttet`, the random state, the Steiner and flip counters and the
    /// next glibc `rand()` value (drawn from a copy).
    pub fn dump_recoverboundary(&self) -> String {
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
            if e == 0 || e == usize::MAX {
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
        let _ = writeln!(out, "recoverboundary dump v1");
        let _ = writeln!(
            out,
            "firstnumber {} numberofpoints {} dupverts {} unuverts {}",
            self.firstnumber, self.numberofpoints, self.dupverts, self.unuverts
        );
        let _ = writeln!(
            out,
            "points {} {}",
            self.pointpool.maxitems, self.pointpool.items
        );
        for k in 0..self.pointpool.maxitems {
            let r = &self.points[k + 1];
            if r.vtype == VertType::Dead {
                let _ = writeln!(out, "p {k} dead");
                continue;
            }
            let ppt = if r.vtype == VertType::Duplicated {
                pref(r.point2ppt)
            } else {
                "-".into()
            };
            let _ = writeln!(
                out,
                "p {} {} {} {} {:016x} {:016x} {:016x} {} {} {}",
                k,
                r.mark,
                r.vtype as i32,
                r.flags & 255,
                r.xyz[0].to_bits(),
                r.xyz[1].to_bits(),
                r.xyz[2].to_bits(),
                ppt,
                tref(r.point2tet),
                shref(r.point2sh)
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
                        "f {k} {} {} {} {} {} {} {} {} {} {} {} {} {} {}",
                        pref(r.v[0]),
                        pref(r.v[1]),
                        pref(r.v[2]),
                        r.marker,
                        r.flags,
                        r.facetindex,
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
                        "s {k} {} {} {} {} {} {} {} {} {} {} {} {}",
                        pref(r.v[0]),
                        pref(r.v[1]),
                        pref(r.v[2]),
                        r.marker,
                        r.flags,
                        r.facetindex,
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
            let _ = write!(
                out,
                "t {k} {} {} {} {} {} {} {} {} {}",
                pref(t.v[0]),
                pref(t.v[1]),
                pref(t.v[2]),
                pref(t.v[3]),
                tref(t.nbr[0]),
                tref(t.nbr[1]),
                tref(t.nbr[2]),
                tref(t.nbr[3]),
                t.marker
            );
            if t.seg == usize::MAX {
                let _ = write!(out, " -");
            } else {
                let a = &self.tet2seg[t.seg];
                let _ = write!(out, " ");
                for (e, x) in a.iter().enumerate() {
                    let _ = write!(out, "{}{}", if e > 0 { "," } else { "" }, shref(*x));
                }
            }
            if t.sub == usize::MAX {
                let _ = write!(out, " -");
            } else {
                let a = &self.tet2sub[t.sub];
                let _ = write!(out, " ");
                for (e, x) in a.iter().enumerate() {
                    let _ = write!(out, "{}{}", if e > 0 { "," } else { "" }, shref(*x));
                }
            }
            let _ = writeln!(out);
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
        let rt = if self.recenttet.tet == NULL_TET {
            "-".into()
        } else {
            tref(encode(&self.recenttet))
        };
        let _ = writeln!(out, "recenttet {rt}");
        let _ = writeln!(out, "randomseed {}", self.randomseed);
        let _ = writeln!(out, "insegments {}", self.insegments);
        let _ = writeln!(
            out,
            "st_segref_count {} st_facref_count {} st_volref_count {} steinerleft {}",
            self.st_segref_count, self.st_facref_count, self.st_volref_count, self.steinerleft
        );
        let _ = writeln!(
            out,
            "flips flip23 {} flip32 {} flip41 {} flip22 {} flip31 {}",
            self.flip23count,
            self.flip32count,
            self.flip41count,
            self.flip22count,
            self.flip31count
        );
        let _ = writeln!(
            out,
            "fliplinklevel {} autofliplinklevel {} duplicated_facets {} skipped_facets {}",
            self.b.fliplinklevel,
            self.autofliplinklevel,
            self.duplicated_facets_count,
            self.skipped_facet_list.as_ref().map_or(0, |l| l.len())
        );
        let _ = writeln!(
            out,
            "subvertstack {} subsegstack {} subfacstack {}",
            self.subvertstack.len(),
            self.subsegstack.len(),
            self.subfacstack.len()
        );
        let _ = writeln!(
            out,
            "flippool {}",
            self.flipstack.len() + self.tflipstack.len()
        );
        let mut cr = self.crand.clone();
        let _ = writeln!(out, "rand {}", cr.rand());
        out
    }
}
