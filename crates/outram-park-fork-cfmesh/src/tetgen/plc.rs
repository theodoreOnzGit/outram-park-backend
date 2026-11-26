// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright (C) 2026 OUTRAM PARK contributors
//
// Port of TetGen 1.6.0 (the PLC input: `tetgen.h` class `tetgenio` (its
// facet, polygon, hole, region and edge lists) and `tetgen.cxx`
// `tetgenio::load_node_call`, `load_poly`, `load_edge`, `readnumberline`,
// `findnextnumber`):
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

//! The PLC input ([`PlcInput`], upstream's `tetgenio` as `-p` reads it):
//! points, facets (each a list of polygons, plus hole points in the facet),
//! facet markers, volume holes, regions and optional input edges.
//!
//! Readers: [`PlcInput::read_poly`] and [`PlcInput::read_smesh`] follow
//! `tetgenio::load_poly` token for token (`readnumberline`,
//! `findnextnumber`, `strtol` with base 0, `strtod`), and
//! [`PlcInput::read_edge`] follows `load_edge`. A `.poly` / `.smesh` that
//! declares zero points (its nodes in a separate `.node` file) is read with
//! [`PlcInput::read_poly_with_node`]. Not read: 2D input (`mesh_dim == 2`)
//! and the `-s` uv parameters, both refused with an error.
//!
//! [`PlcInput::from_triangle_soup`] builds a PLC from a triangulated surface
//! (each triangle a facet with one marker), the way later stages hand
//! cfMesh or Blender surfaces to TetGen; [`PlcInput::to_smesh`] writes it.

#[cfg(not(any(feature = "std", test)))]
#[allow(unused_imports)]
use crate::nostd_prelude::*;

use alloc::string::String;
use alloc::vec::Vec;
use core::fmt::Write;

/// `tetgenio::facet`: polygons (vertex index lists) and the hole points
/// inside the facet.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct PlcFacet {
    /// `polygonlist`: each polygon's vertex indices (from
    /// [`PlcInput::firstnumber`]).
    pub polygons: Vec<Vec<i32>>,
    /// `holelist`: points in the facet's holes.
    pub holes: Vec<[f64; 3]>,
}

/// A piecewise linear complex: upstream's `tetgenio` as `tetgen -p` reads
/// it.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct PlcInput {
    /// `firstnumber`: the index of the first point (0 or 1, from the first
    /// point line).
    pub firstnumber: i32,
    /// `pointlist`.
    pub points: Vec<[f64; 3]>,
    /// `pointmarkerlist` (read, not used by `meshsurface`).
    pub point_markers: Option<Vec<i32>>,
    /// `facetlist`.
    pub facets: Vec<PlcFacet>,
    /// `facetmarkerlist` (`None`: every facet gets the marker -1).
    pub facet_markers: Option<Vec<i32>>,
    /// `holelist`: volume hole points (used by `carveholes`, stage 5).
    pub holes: Vec<[f64; 3]>,
    /// `regionlist`: `x, y, z, attribute, volume bound` per region.
    pub regions: Vec<[f64; 5]>,
    /// `edgelist` (from an `.edge` file): input edges, as index pairs.
    pub edges: Vec<[i32; 2]>,
    /// `edgemarkerlist` (`None`: identified edges get the marker -2).
    pub edge_markers: Option<Vec<i32>>,
}

/// A `.poly` / `.smesh` / `.edge` text upstream refuses (or that the port
/// does not read), with upstream's message where it has one.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PlcReadError(pub String);

/// The `#[inline]` helpers below emulate upstream's C-string scanning on a
/// byte slice: a position past the end is the terminating `'\0'`.
#[inline]
fn at(s: &[u8], i: usize) -> u8 {
    if i < s.len() {
        s[i]
    } else {
        0
    }
}

#[inline]
fn is_numberish(c: u8) -> bool {
    c == b'.' || c == b'+' || c == b'-' || c.is_ascii_digit()
}

/// `findnextnumber(string)`: skip the current field, then anything that
/// does not look like a number; a `#` ends the line.
fn findnextnumber(s: &[u8], mut i: usize) -> usize {
    while at(s, i) != 0
        && at(s, i) != b'#'
        && at(s, i) != b' '
        && at(s, i) != b'\t'
        && at(s, i) != b','
    {
        i += 1;
    }
    while at(s, i) != 0 && at(s, i) != b'#' && !is_numberish(at(s, i)) {
        i += 1;
    }
    if at(s, i) == b'#' {
        // *result = '\0': the rest of the line is gone.
        return s.len();
    }
    i
}

/// `strtol(s + i, &end, 0)`: leading white space, a sign, `0x` hex,
/// leading-`0` octal, else decimal. Returns the value and the end.
fn strtol(s: &[u8], mut i: usize) -> (i64, usize) {
    let start = i;
    while matches!(at(s, i), b' ' | b'\t' | b'\n' | b'\r' | 0x0b | 0x0c) {
        i += 1;
    }
    let mut neg = false;
    if at(s, i) == b'+' || at(s, i) == b'-' {
        neg = at(s, i) == b'-';
        i += 1;
    }
    let mut base = 10;
    if at(s, i) == b'0'
        && (at(s, i + 1) == b'x' || at(s, i + 1) == b'X')
        && at(s, i + 2).is_ascii_hexdigit()
    {
        base = 16;
        i += 2;
    } else if at(s, i) == b'0' {
        base = 8;
    }
    let digits_start = i;
    let mut v: i64 = 0;
    loop {
        let c = at(s, i);
        let d = match c {
            b'0'..=b'9' => (c - b'0') as i64,
            b'a'..=b'f' => (c - b'a' + 10) as i64,
            b'A'..=b'F' => (c - b'A' + 10) as i64,
            _ => 99,
        };
        if d >= base {
            break;
        }
        v = v.saturating_mul(base).saturating_add(d);
        i += 1;
    }
    if i == digits_start {
        return (0, start); // No conversion.
    }
    (if neg { -v } else { v }, i)
}

/// `strtod(s + i, &end)` for decimal numbers (sign, digits, point,
/// exponent), correctly rounded as glibc's is.
fn strtod(s: &[u8], mut i: usize) -> (f64, usize) {
    let start = i;
    while matches!(at(s, i), b' ' | b'\t' | b'\n' | b'\r' | 0x0b | 0x0c) {
        i += 1;
    }
    let b0 = i;
    if at(s, i) == b'+' || at(s, i) == b'-' {
        i += 1;
    }
    let mut nd = 0;
    while at(s, i).is_ascii_digit() {
        i += 1;
        nd += 1;
    }
    if at(s, i) == b'.' {
        i += 1;
        while at(s, i).is_ascii_digit() {
            i += 1;
            nd += 1;
        }
    }
    if nd == 0 {
        return (0.0, start);
    }
    if at(s, i) == b'e' || at(s, i) == b'E' {
        let mut j = i + 1;
        if at(s, j) == b'+' || at(s, j) == b'-' {
            j += 1;
        }
        if at(s, j).is_ascii_digit() {
            while at(s, j).is_ascii_digit() {
                j += 1;
            }
            i = j;
        }
    }
    let t = core::str::from_utf8(&s[b0..i]).unwrap_or("0");
    (t.parse::<f64>().unwrap_or(0.0), i)
}

/// The lines of a text, as `fgets` hands them over (newline kept).
struct Lines<'a> {
    text: &'a [u8],
    pos: usize,
}

impl<'a> Lines<'a> {
    fn new(text: &'a str) -> Self {
        Lines {
            text: text.as_bytes(),
            pos: 0,
        }
    }

    /// `readnumberline`: the next line that has something that looks like
    /// a number (comments skipped); returns the line from that character.
    fn readnumberline(&mut self) -> Option<&'a [u8]> {
        loop {
            if self.pos >= self.text.len() {
                return None;
            }
            let rest = &self.text[self.pos..];
            let len = rest
                .iter()
                .position(|&c| c == b'\n')
                .map_or(rest.len(), |k| k + 1);
            let line = &rest[..len];
            self.pos += len;
            let mut i = 0;
            while at(line, i) != 0 && at(line, i) != b'#' && !is_numberish(at(line, i)) {
                i += 1;
            }
            if at(line, i) == b'#' || at(line, i) == 0 {
                continue;
            }
            return Some(&line[i..]);
        }
    }
}

fn err<T>(m: &str) -> Result<T, PlcReadError> {
    Err(PlcReadError(m.into()))
}

/// `load_node_call(infile, markers, uvflag, ...)`: `n` point lines.
fn load_node_call(
    lines: &mut Lines,
    plc: &mut PlcInput,
    n: usize,
    mesh_dim: i64,
    nattr: i64,
    markers: bool,
) -> Result<(), PlcReadError> {
    let mut pm = Vec::new();
    for i in 0..n {
        let Some(s) = lines.readnumberline() else {
            return err("Error:  Unexpected end of file in the point list.");
        };
        // useindex: the first number is the point's index.
        let mut k;
        if i == 0 {
            let (firstnode, e) = strtol(s, 0);
            if firstnode == 0 || firstnode == 1 {
                plc.firstnumber = firstnode as i32;
            }
            k = e;
        } else {
            k = 0;
        }
        k = findnextnumber(s, k);
        if at(s, k) == 0 {
            return err("Error:  Point has no x coordinate.");
        }
        let (x, e) = strtod(s, k);
        k = findnextnumber(s, e);
        if at(s, k) == 0 {
            return err("Error:  Point has no y coordinate.");
        }
        let (y, e) = strtod(s, k);
        k = e;
        let z;
        if mesh_dim == 3 {
            k = findnextnumber(s, k);
            if at(s, k) == 0 {
                return err("Error:  Point has no z coordinate.");
            }
            let (zz, e) = strtod(s, k);
            z = zz;
            k = e;
        } else {
            z = 0.0;
        }
        plc.points.push([x, y, z]);
        // Read (and drop) the point attributes.
        for _ in 0..nattr {
            k = findnextnumber(s, k);
            if at(s, k) != 0 {
                k = strtod(s, k).1;
            }
        }
        if markers {
            k = findnextnumber(s, k);
            let m = if at(s, k) == 0 {
                0
            } else {
                strtol(s, k).0 as i32
            };
            pm.push(m);
        }
    }
    if markers {
        plc.point_markers = Some(pm);
    }
    Ok(())
}

impl PlcInput {
    /// Read a `.poly` file's text (`tetgenio::load_poly` with a `.poly`).
    pub fn read_poly(text: &str) -> Result<PlcInput, PlcReadError> {
        Self::load_poly(text, false, None)
    }

    /// Read a `.smesh` file's text (`tetgenio::load_poly` with a `.smesh`):
    /// each facet one polygon, no facet holes.
    pub fn read_smesh(text: &str) -> Result<PlcInput, PlcReadError> {
        Self::load_poly(text, true, None)
    }

    /// Read a `.poly` (`smesh == false`) or `.smesh` whose point section is
    /// empty, with its points in the `.node` text `node` (`load_node`).
    pub fn read_poly_with_node(
        text: &str,
        smesh: bool,
        node: &str,
    ) -> Result<PlcInput, PlcReadError> {
        Self::load_poly(text, smesh, Some(node))
    }

    fn load_poly(text: &str, smesh: bool, node: Option<&str>) -> Result<PlcInput, PlcReadError> {
        let mut plc = PlcInput::default();
        let mut lines = Lines::new(text);
        let mut mesh_dim: i64 = 3;
        let mut nattr: i64 = 0;
        let mut markers: i64 = 0;
        let mut uvflag: i64 = 0;

        let Some(s) = lines.readnumberline() else {
            return err("Error:  Empty file.");
        };
        let (n, mut k) = strtol(s, 0);
        k = findnextnumber(s, k);
        if at(s, k) != 0 {
            let r = strtol(s, k);
            mesh_dim = r.0;
            k = r.1;
        }
        k = findnextnumber(s, k);
        if at(s, k) != 0 {
            let r = strtol(s, k);
            nattr = r.0;
            k = r.1;
        }
        k = findnextnumber(s, k);
        if at(s, k) != 0 {
            let r = strtol(s, k);
            markers = r.0;
            k = r.1;
        }
        if at(s, k) != 0 {
            uvflag = strtol(s, k).0;
        }
        if uvflag != 0 {
            return err("The -s uv parameters are not read by the port.");
        }

        if n > 0 {
            load_node_call(
                &mut lines,
                &mut plc,
                n as usize,
                mesh_dim,
                nattr,
                markers != 0,
            )?;
        } else {
            // The points are in a separate .node file (load_node).
            let Some(node) = node else {
                return err("The points are in a separate .node file: use read_poly_with_node.");
            };
            let mut nl = Lines::new(node);
            let Some(s) = nl.readnumberline() else {
                return err("Error:  Empty .node file.");
            };
            let (nn, mut k) = strtol(s, 0);
            let (mut dim, mut na, mut mk) = (3, 0, 0);
            k = findnextnumber(s, k);
            if at(s, k) != 0 {
                let r = strtol(s, k);
                dim = r.0;
                k = r.1;
            }
            k = findnextnumber(s, k);
            if at(s, k) != 0 {
                let r = strtol(s, k);
                na = r.0;
                k = r.1;
            }
            k = findnextnumber(s, k);
            if at(s, k) != 0 {
                mk = strtol(s, k).0;
            }
            mesh_dim = dim;
            load_node_call(&mut nl, &mut plc, nn.max(0) as usize, dim, na, mk != 0)?;
        }

        if mesh_dim != 3 {
            return err("Input error:  2D input is not read by the port (mesh_dim != 3).");
        }
        if plc.points.len() < 4 {
            return err("Input error:  TetGen needs at least 4 points.");
        }

        // Read number of facets and number of boundary markers.
        let Some(s) = lines.readnumberline() else {
            return Ok(plc); // No facet list.
        };
        let (nf, k) = strtol(s, 0);
        if nf <= 0 {
            return Ok(plc);
        }
        let k = findnextnumber(s, k);
        let fmarkers = if at(s, k) == 0 { 0 } else { strtol(s, k).0 };
        let mut fm = Vec::new();

        if !smesh {
            for _ in 0..nf {
                let mut f = PlcFacet::default();
                let mut currentmarker = 0;
                let Some(s) = lines.readnumberline() else {
                    return err("Error:  Unexpected end of file in the facet list.");
                };
                let (npoly, mut k) = strtol(s, 0);
                let mut nholes = 0;
                k = findnextnumber(s, k);
                if at(s, k) != 0 {
                    let r = strtol(s, k);
                    nholes = r.0;
                    k = r.1;
                    if fmarkers == 1 {
                        k = findnextnumber(s, k);
                        if at(s, k) != 0 {
                            currentmarker = strtol(s, k).0 as i32;
                        }
                    }
                }
                if fmarkers == 1 {
                    fm.push(currentmarker);
                }
                if npoly <= 0 {
                    return err("Error:  Wrong number of polygon in a facet.");
                }
                for _ in 0..npoly {
                    let Some(s) = lines.readnumberline() else {
                        return err("Error:  Unexpected end of file in a polygon.");
                    };
                    let mut s = s;
                    let (nv, mut k) = strtol(s, 0);
                    if nv < 1 {
                        return err("Error:  Wrong polygon in a facet.");
                    }
                    let mut p = Vec::with_capacity(nv as usize);
                    for _ in 0..nv {
                        k = findnextnumber(s, k);
                        if at(s, k) == 0 {
                            // Continue on the next non-empty line.
                            let Some(s2) = lines.readnumberline() else {
                                return err("Error: Missing endpoints of a polygon.");
                            };
                            s = s2;
                            k = 0;
                        }
                        let (v, e) = strtol(s, k);
                        p.push(v as i32);
                        k = e;
                    }
                    f.polygons.push(p);
                }
                for _ in 0..nholes {
                    let Some(s) = lines.readnumberline() else {
                        return err("Error:  Unexpected end of file in a facet hole.");
                    };
                    let mut h = [0.0; 3];
                    let mut k = 0;
                    for c in h.iter_mut() {
                        k = findnextnumber(s, k);
                        if at(s, k) == 0 {
                            return err("Error:  A facet hole has no coordinates.");
                        }
                        let r = strtod(s, k);
                        *c = r.0;
                        k = r.1;
                    }
                    f.holes.push(h);
                }
                plc.facets.push(f);
            }
        } else {
            for _ in 0..nf {
                let Some(s) = lines.readnumberline() else {
                    return err("Error:  Unexpected end of file in the facet list.");
                };
                let mut s = s;
                let (nv, mut k) = strtol(s, 0);
                if nv < 1 {
                    return err("Error:  Wrong number of vertex in a facet.");
                }
                let mut p = Vec::with_capacity(nv as usize);
                for _ in 0..nv {
                    k = findnextnumber(s, k);
                    if at(s, k) == 0 {
                        let Some(s2) = lines.readnumberline() else {
                            return err("Error:  Missing endpoints in a facet.");
                        };
                        s = s2;
                        k = 0;
                    }
                    let (v, e) = strtol(s, k);
                    p.push(v as i32);
                    k = e;
                }
                if fmarkers == 1 {
                    k = findnextnumber(s, k);
                    let m = if at(s, k) == 0 {
                        0
                    } else {
                        strtol(s, k).0 as i32
                    };
                    fm.push(m);
                }
                plc.facets.push(PlcFacet {
                    polygons: alloc::vec![p],
                    holes: Vec::new(),
                });
            }
        }
        if fmarkers == 1 {
            plc.facet_markers = Some(fm);
        }

        // Read the hole section.
        let Some(s) = lines.readnumberline() else {
            return Ok(plc);
        };
        let nholes = strtol(s, 0).0;
        for _ in 0..nholes.max(0) {
            let Some(s) = lines.readnumberline() else {
                return err("Error:  Unexpected end of file in the hole list.");
            };
            let mut h = [0.0; 3];
            let mut k = 0;
            for c in h.iter_mut() {
                k = findnextnumber(s, k);
                if at(s, k) == 0 {
                    return err("Error:  A hole has a missing coordinate.");
                }
                let r = strtod(s, k);
                *c = r.0;
                k = r.1;
            }
            plc.holes.push(h);
        }

        // Read the (optional) region section.
        let Some(s) = lines.readnumberline() else {
            return Ok(plc);
        };
        let nregions = strtol(s, 0).0;
        for _ in 0..nregions.max(0) {
            let Some(s) = lines.readnumberline() else {
                return err("Error:  Unexpected end of file in the region list.");
            };
            let mut r5 = [0.0; 5];
            let mut k = 0;
            for c in r5.iter_mut().take(4) {
                k = findnextnumber(s, k);
                if at(s, k) == 0 {
                    return err("Error:  A region has a missing value.");
                }
                let r = strtod(s, k);
                *c = r.0;
                k = r.1;
            }
            k = findnextnumber(s, k);
            r5[4] = if at(s, k) == 0 { r5[3] } else { strtod(s, k).0 };
            plc.regions.push(r5);
        }
        Ok(plc)
    }

    /// Read an `.edge` file's text into [`PlcInput::edges`]
    /// (`tetgenio::load_edge`, which `load_plc` calls after the `.poly`).
    pub fn read_edge(&mut self, text: &str) -> Result<(), PlcReadError> {
        let mut lines = Lines::new(text);
        let Some(s) = lines.readnumberline() else {
            return err("Error:  Empty .edge file.");
        };
        let (n, k) = strtol(s, 0);
        let mut markers = 0;
        if n > 0 {
            let k = findnextnumber(s, k);
            if at(s, k) != 0 {
                markers = strtol(s, k).0;
            }
        }
        let np = self.points.len() as i32;
        let mut em = Vec::new();
        self.edges.clear();
        for _ in 0..n.max(0) {
            let Some(s) = lines.readnumberline() else {
                return err("Error:  Unexpected end of file in the edge list.");
            };
            let mut k = 0;
            let mut e = [0i32; 2];
            for c in e.iter_mut() {
                k = findnextnumber(s, k);
                if at(s, k) == 0 {
                    return err("Error:  An edge is missing a vertex.");
                }
                let r = strtol(s, k);
                k = r.1;
                let corner = r.0 as i32;
                if corner < self.firstnumber || corner >= np + self.firstnumber {
                    return err("Error:  An edge has an invalid vertex index.");
                }
                *c = corner;
            }
            self.edges.push(e);
            if markers != 0 {
                k = findnextnumber(s, k);
                em.push(strtol(s, k).0 as i32);
            }
        }
        self.edge_markers = if markers != 0 { Some(em) } else { None };
        Ok(())
    }

    /// A PLC from a triangle soup: every triangle a facet of one polygon
    /// with its marker (0-based indices, `firstnumber` 0, no holes).
    ///
    /// # Panics
    /// If `markers` and `triangles` differ in length.
    pub fn from_triangle_soup(
        points: &[[f64; 3]],
        triangles: &[[usize; 3]],
        markers: &[i32],
    ) -> PlcInput {
        assert_eq!(triangles.len(), markers.len(), "one marker per triangle");
        PlcInput {
            firstnumber: 0,
            points: points.to_vec(),
            point_markers: None,
            facets: triangles
                .iter()
                .map(|t| PlcFacet {
                    polygons: alloc::vec![alloc::vec![t[0] as i32, t[1] as i32, t[2] as i32]],
                    holes: Vec::new(),
                })
                .collect(),
            facet_markers: Some(markers.to_vec()),
            holes: Vec::new(),
            regions: Vec::new(),
            edges: Vec::new(),
            edge_markers: None,
        }
    }

    /// Write the PLC as `.smesh` text (one polygon per facet: a facet with
    /// several polygons or holes is refused). Coordinates are written in
    /// Rust's shortest round-trip form, which `strtod` reads back exactly.
    pub fn to_smesh(&self) -> Result<String, PlcReadError> {
        let mut s = String::new();
        let _ = writeln!(
            s,
            "# .smesh written by outram-park-fork-cfmesh tetgen::PlcInput"
        );
        let _ = writeln!(s, "{} 3 0 0", self.points.len());
        for (i, p) in self.points.iter().enumerate() {
            let _ = writeln!(
                s,
                "{} {:?} {:?} {:?}",
                i as i32 + self.firstnumber,
                p[0],
                p[1],
                p[2]
            );
        }
        let _ = writeln!(
            s,
            "{} {}",
            self.facets.len(),
            self.facet_markers.is_some() as i32
        );
        for (i, f) in self.facets.iter().enumerate() {
            if f.polygons.len() != 1 || !f.holes.is_empty() {
                return err("A .smesh facet is one polygon without holes.");
            }
            let _ = write!(s, "{}", f.polygons[0].len());
            for v in &f.polygons[0] {
                let _ = write!(s, " {v}");
            }
            if let Some(m) = &self.facet_markers {
                let _ = write!(s, " {}", m[i]);
            }
            let _ = writeln!(s);
        }
        let _ = writeln!(s, "{}", self.holes.len());
        for (i, h) in self.holes.iter().enumerate() {
            let _ = writeln!(s, "{} {:?} {:?} {:?}", i + 1, h[0], h[1], h[2]);
        }
        let _ = writeln!(s, "{}", self.regions.len());
        for (i, r) in self.regions.iter().enumerate() {
            let _ = writeln!(
                s,
                "{} {:?} {:?} {:?} {:?} {:?}",
                i + 1,
                r[0],
                r[1],
                r[2],
                r[3],
                r[4]
            );
        }
        Ok(s)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The C scanning helpers behave as `strtol(.., 0)`, `strtod` and
    /// `findnextnumber` do.
    #[test]
    fn scanning_follows_the_c_library() {
        assert_eq!(strtol(b"  42 x", 0), (42, 4));
        assert_eq!(strtol(b"010", 0), (8, 3)); // octal
        assert_eq!(strtol(b"0x1f", 0), (31, 4));
        assert_eq!(strtol(b"-7", 0), (-7, 2));
        assert_eq!(strtol(b"x", 0), (0, 0));
        assert_eq!(strtod(b"0.25 1", 0), (0.25, 4));
        assert_eq!(strtod(b"-1.5e-3,", 0), (-1.5e-3, 7));
        assert_eq!(strtod(b"2e", 0), (2.0, 1));
        let s = b"1  0  9  # 2\n";
        assert_eq!(findnextnumber(s, 1), 3);
        assert_eq!(findnextnumber(s, 7), s.len()); // the comment ends the line
    }

    /// A `.smesh` with a marker per facet, a hole and a region round-trips
    /// through `to_smesh`, and an `.edge` list is read with its markers.
    #[test]
    fn smesh_round_trips_and_edges_are_read() {
        let pts = [
            [0.0, 0.0, 0.0],
            [1.0, 0.0, 0.0],
            [0.0, 1.0, 0.0],
            [0.0, 0.0, 1.0],
        ];
        let tris = [[0, 2, 1], [0, 1, 3], [1, 2, 3], [0, 3, 2]];
        let mut plc = PlcInput::from_triangle_soup(&pts, &tris, &[1, 2, 2, 3]);
        plc.holes.push([0.1, 0.1, 0.1]);
        plc.regions.push([0.2, 0.2, 0.2, 5.0, 0.5]);
        let text = plc.to_smesh().unwrap();
        let back = PlcInput::read_smesh(&text).unwrap();
        assert_eq!(back, plc);
        let mut back = back;
        back.read_edge("2 1\n0 0 3 7\n1 1 2 9\n").unwrap();
        assert_eq!(back.edges, alloc::vec![[0, 3], [1, 2]]);
        assert_eq!(back.edge_markers, Some(alloc::vec![7, 9]));
        assert!(back.read_edge("1\n0 0 9\n").is_err());
        // Separate .node, a polygon split across lines, a facet hole.
        let poly = "0 3 0 0\n1 1\n1 1 5\n4 1 2\n 3 4\n1 0.5 0.5 0\n0\n";
        let node = "4 3 0 0\n1 0 0 0\n2 1 0 0\n3 1 1 0\n4 0 1 1\n";
        let p = PlcInput::read_poly_with_node(poly, false, node).unwrap();
        assert_eq!(p.firstnumber, 1);
        assert_eq!(p.facets[0].polygons, alloc::vec![alloc::vec![1, 2, 3, 4]]);
        assert_eq!(p.facets[0].holes, alloc::vec![[0.5, 0.5, 0.0]]);
        assert_eq!(p.facet_markers, Some(alloc::vec![5]));
        assert!(PlcInput::read_poly(poly).is_err());
        assert!(PlcInput::read_poly("4 2 0 0\n").is_err());
    }
}
