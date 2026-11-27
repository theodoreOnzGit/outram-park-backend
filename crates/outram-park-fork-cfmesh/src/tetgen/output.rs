// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright (C) 2026 OUTRAM PARK contributors
//
// Port of TetGen 1.6.0 (`tetgen.cxx`, section `output_cxx`: `outnodes`,
// `outelements`, `outsubfaces`, `outsubsegments`, written to text as the
// command-line program writes `.node`, `.ele`, `.face`, `.edge`; and the
// point-marker re-indexing of `jettisonnodes`):
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

//! The `-p` output (stage 5, gh:#1794): [`TetgenOutput`] holds what
//! `outnodes`, `outelements`, `outsubfaces` and `outsubsegments` write, and
//! renders it as upstream's files byte for byte ([`TetgenOutput::node_text`]
//! and friends; `%.17g` is C's, [`fmt_g17`]). [`to_volume_mesh`] turns it
//! into this crate's [`VolumeMesh`].
//!
//! Not ported (switches the `-p` route here does not take): `-o2` extra
//! nodes, `-nn` neighbours, `-B` (`nobound`), `-R` (`reversetetori`), point
//! attributes (a `.poly` with attributes is read without them, stage 3),
//! weighted output (`-w`), `-f` / `-e` (all faces / all edges).
//!
//! **`-m` (gh:#1825)** adds `outmetrics`: the `.mtr` file (every point's
//! size, [`TetgenOutput::mtr_text`]) and the `.p2t` file (a tetrahedron of
//! every point, [`TetgenOutput::p2t_text`]), and `outelements` then points
//! every tetrahedron's vertices at it (`setpoint2tet`).

#[cfg(not(any(feature = "std", test)))]
#[allow(unused_imports)]
use crate::nostd_prelude::*;

use super::mesh::{
    decode, encode, sdecode, Face, Point, ShPool, TetgenMesh, TriFace, VertType, NULL_TET,
};
use crate::math::Vec3;
use crate::volume_mesh::{BoundaryPatch, VolumeMesh};
use alloc::collections::BTreeMap;
use alloc::string::String;
use alloc::vec::Vec;
use core::fmt::Write;

/// What `tetgen -p` writes (`tetgenio` out lists and the file header
/// counts), numbered from `firstindex`.
#[derive(Debug, Clone, PartialEq)]
pub struct TetgenOutput {
    /// The first index: 0 with `-z`, else `in->firstnumber`.
    pub firstindex: i32,
    /// `.node`: the coordinates.
    pub points: Vec<[f64; 3]>,
    /// `.node`: the boundary markers, when the input had point markers.
    pub point_markers: Option<Vec<i32>>,
    /// `.ele`: the tetrahedra (indices from `firstindex`), in upstream's
    /// vertex order (`orient3d < 0`, TetGen's positive orientation).
    pub tets: Vec<[i32; 4]>,
    /// `.ele`: one attribute per tetrahedron (the region, `-A`).
    pub tet_attributes: Option<Vec<f64>>,
    /// `.face`: the boundary (and internal constraint) triangles, oriented
    /// to the volume on their right-hand side; `None` when there are no
    /// subfaces (upstream writes no file).
    pub faces: Option<Vec<[i32; 3]>>,
    /// `.face`: the facet markers.
    pub face_markers: Vec<i32>,
    /// `.edge`: the subsegments.
    pub edges: Vec<[i32; 2]>,
    /// `.edge`: their markers (`0` becomes `1`, upstream's default).
    pub edge_markers: Vec<i32>,
    /// `.mtr` (`-m`, `outmetrics`, gh:#1825): every point's mesh size, in
    /// `.node` order; `None` without `-m`.
    pub point_mtrs: Option<Vec<f64>>,
    /// `.p2t` (`-m`): for every point, the `.ele` index of a tetrahedron
    /// containing it (the last one `outelements` visits); `None` without
    /// `-m`.
    pub point2tet: Option<Vec<i32>>,
}

/// C's `printf("%-16.8e", x)` (glibc): nine significant digits in
/// scientific form, a sign and at least two exponent digits, padded on the
/// right to 16 characters. Rust's exact `{:.8e}` gives the digits.
pub fn fmt_e8_left16(x: f64) -> String {
    let r = alloc::format!("{x:.8e}");
    let (mant, exp) = r.split_once('e').unwrap_or((r.as_str(), "0"));
    let e: i32 = exp.parse().unwrap_or(0);
    let sign = if e < 0 { '-' } else { '+' };
    let mut s = alloc::format!("{mant}e{sign}{:02}", e.abs());
    while s.len() < 16 {
        s.push(' ');
    }
    s
}

/// C's `printf("%.17g", x)` (glibc), the format TetGen writes coordinates
/// and attributes with. Rust's exact `{:.*e}` gives the 17 significant
/// digits; the `%g` rules (fixed for exponents `-4..17`, trailing zeros
/// removed, two-digit exponent) are applied here. The same rules, at six
/// digits, are `outram_park_fork_liggghts::mesh_dump::format_g`; this crate
/// cannot depend on that one.
pub fn fmt_g17(x: f64) -> String {
    const P: i32 = 17;
    if x.is_nan() {
        return if x.is_sign_negative() {
            "-nan".into()
        } else {
            "nan".into()
        };
    }
    if x.is_infinite() {
        return if x < 0.0 { "-inf".into() } else { "inf".into() };
    }
    if x == 0.0 {
        return if x.is_sign_negative() {
            "-0".into()
        } else {
            "0".into()
        };
    }
    let e = alloc::format!("{:.*e}", (P - 1) as usize, x);
    let (mant, exp) = e.split_once('e').unwrap_or((e.as_str(), "0"));
    let xexp: i32 = exp.parse().unwrap_or(0);
    let strip = |s: &str| -> String {
        if s.contains('.') {
            let t = s.trim_end_matches('0');
            t.trim_end_matches('.').into()
        } else {
            s.into()
        }
    };
    if xexp < -4 || xexp >= P {
        let m = strip(mant);
        let sign = if xexp < 0 { '-' } else { '+' };
        alloc::format!("{m}e{sign}{:02}", xexp.abs())
    } else {
        let f = alloc::format!("{:.*}", (P - 1 - xexp) as usize, x);
        strip(&f)
    }
}

impl TetgenOutput {
    /// The `.node` file (`outnodes`; `commandline` is upstream's
    /// `b->commandline`, every argument followed by a space).
    pub fn node_text(&self, commandline: &str) -> String {
        let mut s = String::new();
        let bmark = self.point_markers.is_some() as i32;
        let _ = writeln!(s, "{}  3  0  {}", self.points.len(), bmark);
        for (i, p) in self.points.iter().enumerate() {
            let _ = write!(
                s,
                "{:4}    {}  {}  {}",
                i as i32 + self.firstindex,
                fmt_g17(p[0]),
                fmt_g17(p[1]),
                fmt_g17(p[2])
            );
            if let Some(m) = &self.point_markers {
                let _ = write!(s, "    {}", m[i]);
            }
            s.push('\n');
        }
        let _ = writeln!(s, "# Generated by {commandline}");
        s
    }

    /// The `.ele` file (`outelements`).
    pub fn ele_text(&self, commandline: &str) -> String {
        let mut s = String::new();
        let eextras = self.tet_attributes.is_some() as i32;
        let _ = writeln!(s, "{}  4  {}", self.tets.len(), eextras);
        for (i, t) in self.tets.iter().enumerate() {
            let _ = write!(
                s,
                "{:5}   {:5} {:5} {:5} {:5}",
                i as i32 + self.firstindex,
                t[0],
                t[1],
                t[2],
                t[3]
            );
            if let Some(a) = &self.tet_attributes {
                let _ = write!(s, "    {}", fmt_g17(a[i]));
            }
            s.push('\n');
        }
        let _ = writeln!(s, "# Generated by {commandline}");
        s
    }

    /// The `.face` file (`outsubfaces`), `None` when upstream writes none.
    pub fn face_text(&self, commandline: &str) -> Option<String> {
        let faces = self.faces.as_ref()?;
        let mut s = String::new();
        let _ = writeln!(s, "{}  1", faces.len());
        for (i, f) in faces.iter().enumerate() {
            let _ = writeln!(
                s,
                "{:5}   {:4}  {:4}  {:4}    {}",
                i as i32 + self.firstindex,
                f[0],
                f[1],
                f[2],
                self.face_markers[i]
            );
        }
        let _ = writeln!(s, "# Generated by {commandline}");
        Some(s)
    }

    /// The `.edge` file (`outsubsegments`).
    pub fn edge_text(&self, commandline: &str) -> String {
        let mut s = String::new();
        let _ = writeln!(s, "{}  1", self.edges.len());
        for (i, e) in self.edges.iter().enumerate() {
            let _ = writeln!(
                s,
                "{:5}   {:4}  {:4}  {}",
                i as i32 + self.firstindex,
                e[0],
                e[1],
                self.edge_markers[i]
            );
        }
        let _ = writeln!(s, "# Generated by {commandline}");
        s
    }

    /// The `.mtr` file (`outmetrics`, `-m`, gh:#1825): `None` without
    /// `-m`. Upstream writes no command line into it.
    pub fn mtr_text(&self) -> Option<String> {
        let m = self.point_mtrs.as_ref()?;
        let mut s = String::new();
        let _ = writeln!(s, "{}  1", m.len());
        for x in m {
            let _ = writeln!(s, " {}", fmt_e8_left16(*x));
        }
        Some(s)
    }

    /// The `.p2t` file (`outmetrics`, `-m`): `None` without `-m`.
    pub fn p2t_text(&self, commandline: &str) -> Option<String> {
        let t = self.point2tet.as_ref()?;
        let mut s = String::new();
        for (i, e) in t.iter().enumerate() {
            let _ = writeln!(s, "{}  {}", i as i32 + self.firstindex, e);
        }
        let _ = writeln!(s, "# Generated by {commandline}");
        Some(s)
    }
}

impl TetgenMesh {
    /// The `in->pointmarkerlist` re-indexing of `jettisonnodes` (upstream
    /// does it inside its point loop; the port's [`TetgenMesh::jettisonnodes`]
    /// from stage 2 does not carry the list, so this runs the same
    /// traversal first, then calls it).
    pub fn jettisonnodes_plc(&mut self) {
        self.reach5[32] += 1;
        if let Some(mut list) = self.pointmarkerlist.take() {
            let mut oldidx = 0usize;
            let mut newidx = 0usize;
            self.pointpool.traversalinit();
            while let Some(p) = self.pointtraverse() {
                let jet = matches!(self.pointtype(p), VertType::Duplicated | VertType::Unused);
                if !jet {
                    if oldidx < self.numberofpoints {
                        list[newidx] = list[oldidx];
                    }
                    newidx += 1;
                }
                oldidx += 1;
            }
            self.pointmarkerlist = Some(list);
        }
        self.jettisonnodes();
    }

    /// `outnodes`, `outelements` (writes `elemindex`), `outsubfaces` and
    /// `outsubsegments` into a [`TetgenOutput`], as `tetrahedralize()`
    /// calls them for `-p` (no `-B`, first order).
    pub fn output_plc(&mut self) -> TetgenOutput {
        let firstindex = if self.b.zeroindex {
            0
        } else {
            self.firstnumber
        };
        let shift = if self.firstnumber == 1 && firstindex == 0 {
            1
        } else {
            0
        };

        // outnodes().
        let bmark = self.pointmarkerlist.is_some();
        let mut points = Vec::with_capacity(self.pointpool.items);
        let mut markers = Vec::new();
        self.pointpool.traversalinit();
        let mut index = 0usize;
        while let Some(p) = self.pointtraverse() {
            if bmark {
                let mut marker = 0;
                if index < self.numberofpoints {
                    marker = self.pointmarkerlist.as_ref().map_or(0, |l| l[index]);
                } else if matches!(self.pointtype(p), VertType::FreeSeg | VertType::FreeFacet) {
                    let parentsh = sdecode(self.point2sh(p));
                    if !parentsh.is_null() {
                        marker = self.shellmark(&parentsh);
                    }
                }
                markers.push(marker);
            }
            points.push(self.points[p].xyz);
            index += 1;
        }

        // outelements().
        let mut tets = Vec::new();
        let mut attrs = Vec::new();
        self.tetpool.traversalinit();
        let mut elementnumber = firstindex;
        while let Some(t) = self.tetrahedrontraverse() {
            let v = self.tets[t].v;
            tets.push([
                self.pointmark(v[0]) - shift,
                self.pointmark(v[1]) - shift,
                self.pointmark(v[2]) - shift,
                self.pointmark(v[3]) - shift,
            ]);
            if self.numelemattrib > 0 {
                attrs.push(self.elemattrib[t]);
            }
            self.tets[t].elemindex = elementnumber;
            if self.b.metric {
                // -m: update the point-to-tet map, so that every point is
                //   pointing to a real tet, not a fictious one (.p2t).
                for p in v {
                    self.setpoint2tet(p, encode(&TriFace { tet: t, ver: 0 }));
                }
            }
            elementnumber += 1;
        }

        // outsubfaces() (only if there are subfaces).
        let mut faces = None;
        let mut face_markers = Vec::new();
        if self.subfacepool.items > 0 {
            let mut fl = Vec::new();
            self.subfacepool.traversalinit();
            while let Some(s) = self.shellfacetraverse(ShPool::Sub) {
                let faceloop = Face {
                    pool: ShPool::Sub,
                    sh: s,
                    shver: 0,
                };
                let mut abuttingtet = self.stpivot(&faceloop);
                let (torg, tdest, tapex): (Point, Point, Point);
                if abuttingtet.tet != NULL_TET {
                    if self.ishulltet(abuttingtet.tet) {
                        self.fsymself(&mut abuttingtet);
                    }
                    torg = self.org(&abuttingtet);
                    tdest = self.dest(&abuttingtet);
                    tapex = self.apex(&abuttingtet);
                } else {
                    torg = self.sorg(&faceloop);
                    tdest = self.sdest(&faceloop);
                    tapex = self.sapex(&faceloop);
                }
                fl.push([
                    self.pointmark(torg) - shift,
                    self.pointmark(tdest) - shift,
                    self.pointmark(tapex) - shift,
                ]);
                face_markers.push(self.shellmark(&faceloop));
            }
            faces = Some(fl);
        }

        // outsubsegments().
        let mut edges = Vec::new();
        let mut edge_markers = Vec::new();
        self.subsegpool.traversalinit();
        while let Some(s) = self.shellfacetraverse(ShPool::Seg) {
            let edgeloop = Face {
                pool: ShPool::Seg,
                sh: s,
                shver: 0,
            };
            edges.push([
                self.pointmark(self.sorg(&edgeloop)) - shift,
                self.pointmark(self.sdest(&edgeloop)) - shift,
            ]);
            let mut marker = self.shellmark(&edgeloop);
            if marker == 0 {
                marker = 1; // Default marker of a boundary edge is 1.
            }
            edge_markers.push(marker);
        }

        // outmetrics() (-m; with one size per point, msize is 1).
        let (mut point_mtrs, mut point2tet) = (None, None);
        if self.b.metric {
            self.reachm[17] += 1;
            let mut m = Vec::with_capacity(self.pointpool.items);
            let mut p2t = Vec::with_capacity(self.pointpool.items);
            self.pointpool.traversalinit();
            while let Some(p) = self.pointtraverse() {
                m.push(self.points[p].mtr);
                let parenttet = decode(self.point2tet(p));
                // A point in no tetrahedron (only with -J) has a NULL or
                //   stale link; upstream reads through it (a crash for
                //   NULL). Here NULL gives -1.
                p2t.push(if parenttet.tet == NULL_TET {
                    -1
                } else {
                    self.tets[parenttet.tet].elemindex
                });
            }
            point_mtrs = Some(m);
            point2tet = Some(p2t);
        }

        TetgenOutput {
            firstindex,
            points,
            point_markers: if bmark { Some(markers) } else { None },
            tets,
            tet_attributes: if self.numelemattrib > 0 {
                Some(attrs)
            } else {
                None
            },
            faces,
            face_markers,
            edges,
            edge_markers,
            point_mtrs,
            point2tet,
        }
    }
}

/// The TetGen tetrahedra as this crate's [`VolumeMesh`]: one cell per
/// tetrahedron (in `.ele` order), the faces shared by two tetrahedra
/// internal (owner the first tetrahedron that lists the face), the others
/// boundary faces in one patch per facet marker of the `.face` triangle on
/// it (markers ascending). The patch of marker `m` is named `names[m]` when
/// `names` has that entry (a PLC built with
/// [`super::PlcInput::from_triangle_soup`] and a `SurfaceRegions`'
/// `region_of_tri` as markers), else `marker_<m>`. Internal subfaces (the
/// interface between two regions) are internal faces; the region attribute
/// is not carried. Ends with [`crate::renumber_mesh::upper_triangular`]
/// (AGENTS.md: every public route that returns a `VolumeMesh`). The mesh
/// is assembled with `cells: None` (no cell face lists are carried);
/// `upper_triangular` then gives the cells the lists of its order.
///
/// A boundary face with no `.face` triangle on it (impossible for a
/// complete `-p` run: `carveholes` leaves only subfaces on the hull) goes
/// to the patch `unmarked`.
pub fn to_volume_mesh(out: &TetgenOutput, names: Option<&[String]>) -> VolumeMesh {
    let first = out.firstindex;
    let points: Vec<Vec3> = out
        .points
        .iter()
        .map(|p| Vec3::new(p[0], p[1], p[2]))
        .collect();
    let idx = |i: i32| (i - first) as usize;
    let key = |mut k: [usize; 3]| {
        k.sort_unstable();
        k
    };
    // The facet marker of every .face triangle.
    let mut face_marker: BTreeMap<[usize; 3], i32> = BTreeMap::new();
    if let Some(faces) = &out.faces {
        for (f, m) in faces.iter().zip(out.face_markers.iter()) {
            face_marker.insert(key([idx(f[0]), idx(f[1]), idx(f[2])]), *m);
        }
    }
    // TetGen's [a,b,c,d] has orient3d(a,b,c,d) < 0: d lies on the side of
    // [a,b,c] its right-hand normal points to, so these rings point out.
    let mut first_seen: BTreeMap<[usize; 3], (usize, usize, [usize; 3])> = BTreeMap::new();
    let mut int_faces: Vec<Vec<usize>> = Vec::new();
    let mut int_owner = Vec::new();
    let mut int_nb = Vec::new();
    let mut seq = 0usize;
    for (c, t) in out.tets.iter().enumerate() {
        let [a, b, cc, d] = [idx(t[0]), idx(t[1]), idx(t[2]), idx(t[3])];
        for ring in [[a, cc, b], [a, b, d], [b, cc, d], [cc, a, d]] {
            let k = key(ring);
            if let Some((_, owner, oring)) = first_seen.remove(&k) {
                int_faces.push(oring.to_vec());
                int_owner.push(owner);
                int_nb.push(Some(c));
            } else {
                first_seen.insert(k, (seq, c, ring));
                seq += 1;
            }
        }
    }
    let mut bnd: Vec<(usize, usize, [usize; 3], Option<i32>)> = first_seen
        .into_iter()
        .map(|(k, (s, c, ring))| (s, c, ring, face_marker.get(&k).copied()))
        .collect();
    bnd.sort_unstable_by_key(|x| x.0);
    let mut by_marker: BTreeMap<Option<i32>, Vec<(usize, [usize; 3])>> = BTreeMap::new();
    for (_, c, ring, m) in bnd {
        by_marker.entry(m).or_default().push((c, ring));
    }
    let mut faces = int_faces;
    let mut owner = int_owner;
    let mut neighbour = int_nb;
    let mut patches = Vec::new();
    // Marked patches by ascending marker, then the unmarked ones.
    let mut order: Vec<Option<i32>> = by_marker.keys().filter(|m| m.is_some()).copied().collect();
    if by_marker.contains_key(&None) {
        order.push(None);
    }
    for m in order {
        let list = &by_marker[&m];
        let name = match m {
            Some(m) => match names {
                Some(n) if m >= 0 && (m as usize) < n.len() => n[m as usize].clone(),
                _ => alloc::format!("marker_{m}"),
            },
            None => "unmarked".into(),
        };
        patches.push(BoundaryPatch {
            name,
            start_face: faces.len(),
            n_faces: list.len(),
        });
        for (c, ring) in list {
            faces.push(ring.to_vec());
            owner.push(*c);
            neighbour.push(None);
        }
    }
    let mesh = VolumeMesh {
        points,
        faces,
        owner,
        neighbour,
        n_cells: out.tets.len(),
        patches,
        cells: None,
    };
    crate::renumber_mesh::upper_triangular(&mesh)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tetgen::{tetrahedralize_plc, PlcInput, Switches};

    /// C's `%.17g` on values whose glibc output is known.
    #[test]
    fn fmt_g17_is_c_percent_17g() {
        assert_eq!(fmt_g17(0.0), "0");
        assert_eq!(fmt_g17(1.0), "1");
        assert_eq!(fmt_g17(-2.5), "-2.5");
        assert_eq!(fmt_g17(0.1), "0.10000000000000001");
        assert_eq!(fmt_g17(1e-5), "1.0000000000000001e-05");
        assert_eq!(fmt_g17(0.0001), "0.0001");
        assert_eq!(fmt_g17(1e17), "1e+17");
        assert_eq!(fmt_g17(12345678901234567.0), "12345678901234568");
        assert_eq!(fmt_g17(1.0 / 3.0), "0.33333333333333331");
        assert_eq!(fmt_g17(f64::MAX), "1.7976931348623157e+308");
    }

    /// A unit box with two facet markers: the `VolumeMesh` keeps every
    /// tetrahedron, is closed and accepted by the pipeline's gate, its
    /// faces are upper-triangular, the patches follow the markers (named
    /// from `names`) and the volume is 1.
    #[test]
    fn to_volume_mesh_is_a_valid_upper_triangular_mesh() {
        let poly =
            "8 3 0 0\n1 0 0 0\n2 1 0 0\n3 1 1 0\n4 0 1 0\n5 0 0 1\n6 1 0 1\n7 1 1 1\n8 0 1 1\n\
                    6 1\n1 0 0\n4 1 4 3 2\n1 0 1\n4 5 6 7 8\n1 0 1\n4 1 2 6 5\n1 0 1\n4 2 3 7 6\n\
                    1 0 1\n4 3 4 8 7\n1 0 1\n4 4 1 5 8\n0\n0\n";
        let plc = PlcInput::read_poly(poly).unwrap();
        let r = tetrahedralize_plc(&plc, &Switches::parse("pz").unwrap().behavior()).unwrap();
        let out = r.output.unwrap();
        let names: Vec<String> = alloc::vec!["bottom".into(), "sides".into()];
        let vm = to_volume_mesh(&out, Some(&names));
        assert_eq!(vm.n_cells, out.tets.len());
        assert!(crate::pipeline::acceptable(&vm));
        assert!(!crate::poly_mesh_gen_checks::check_upper_triangular(&vm).0);
        let p: Vec<(&str, usize)> = vm
            .patches
            .iter()
            .map(|p| (p.name.as_str(), p.n_faces))
            .collect();
        assert_eq!(p, [("bottom", 2), ("sides", 10)]);
        assert!((vm.total_volume() - 1.0).abs() < 1e-12);
        // Without names the patches are named by marker.
        let vm2 = to_volume_mesh(&out, None);
        assert_eq!(vm2.patches[0].name, "marker_0");
    }
}
