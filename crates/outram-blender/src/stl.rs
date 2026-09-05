// SPDX-License-Identifier: GPL-3.0-only
// Copyright (C) 2026 OUTRAM PARK contributors
//
// ASCII and binary STL read/write. The STL file format is a public de-facto
// standard originated by 3D Systems, Inc.; this implementation is written from the
// public format description and contains no upstream source.
//
// This file is part of OUTRAM PARK.
//
// OUTRAM PARK is free software: you can redistribute it and/or modify it
// under the terms of the GNU General Public License as published by the
// Free Software Foundation, either version 3 of the License, or (at your
// option) any later version.
//
// OUTRAM PARK is distributed in the hope that it will be useful, but
// WITHOUT ANY WARRANTY; without even the implied warranty of
// MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE.  See the GNU
// General Public License for more details.
//
// You should have received a copy of the GNU General Public License along
// with OUTRAM PARK.  If not, see <https://www.gnu.org/licenses/>.

//! STL import / export — the lingua franca for surface-mesh interchange.
//!
//! STL (STereoLithography) is an unstructured **triangle soup**: a flat list of
//! independent triangles, each with a facet normal and three vertices, with *no*
//! shared-vertex topology. It is the format most CAD/mesh tools and the DAGMC /
//! Monte-Carlo pipelines read and write, so it is the crate's interoperability
//! bridge alongside the OpenFOAM polyMesh and CSG exporters in
//! [`crate::export`].
//!
//! # Writing
//!
//! [`to_stl_ascii`] / [`to_stl_binary`] fan-triangulate every face and emit one
//! facet per triangle with an outward normal computed from the triangle
//! geometry (`(b − a) × (c − a)`, normalized). [`write_stl_ascii`] /
//! [`write_stl_binary`] are the disk counterparts. Binary is compact and exact
//! to `f32`; ASCII is human-readable and keeps full `f64` precision.
//!
//! # Reading
//!
//! [`from_stl_ascii`] / [`from_stl_binary`] parse a triangle soup into a
//! [`Mesh`]. Because STL has no shared vertices, the parsed mesh has one vertex
//! per triangle corner (a cube reads back as 36 vertices, 12 faces) — run
//! [`crate::weld::weld`] to merge coincident corners into a proper topological
//! mesh. [`from_stl_bytes`] auto-detects ASCII vs binary by the binary
//! size invariant, and [`read_stl`] does the same from a file.
//!
//! No `faer`, no external dependency; Android-safe.

use crate::math::Vec3;
use crate::mesh::Mesh;
use std::path::Path;

/// Errors from parsing an STL stream.
#[derive(Debug, thiserror::Error)]
pub enum StlError {
    /// The byte stream is too short to be a valid binary STL (needs at least
    /// the 80-byte header plus the 4-byte triangle count).
    #[error("STL stream too short: {0} bytes (need at least 84 for binary)")]
    Truncated(usize),
    /// A binary STL whose length does not match `84 + 50 * triangle_count`.
    #[error("binary STL length {got} does not match header count (expected {expected})")]
    BadBinaryLength {
        /// The actual byte length.
        got: usize,
        /// The length implied by the header's triangle count.
        expected: usize,
    },
    /// An ASCII STL with a malformed `vertex` line or a truncated triangle.
    #[error("malformed ASCII STL: {0}")]
    Parse(String),
    /// A filesystem error while reading an STL file.
    #[error("I/O error reading STL: {0}")]
    Io(#[from] std::io::Error),
}

// ---------------------------------------------------------------------------
// Writing
// ---------------------------------------------------------------------------

/// Fan-triangulate `mesh` and return it as an **ASCII** STL string.
///
/// Each triangle is written with an outward facet normal computed from its
/// geometry. Coordinates keep full `f64` precision (round-trippable). The solid
/// is named `outram_blender`.
///
/// # Examples
///
/// ```
/// use outram_blender::{primitives, stl::to_stl_ascii};
///
/// let stl = to_stl_ascii(&primitives::cube(2.0));
/// // A cube's 6 quads fan-triangulate into 12 facets.
/// assert_eq!(stl.matches("facet normal").count(), 12);
/// ```
pub fn to_stl_ascii(mesh: &Mesh) -> String {
    let positions = mesh.positions();
    let mut s = String::from("solid outram_blender\n");
    for tri in triangles(mesh) {
        let n = facet_normal(&positions, tri);
        s.push_str(&format!("  facet normal {} {} {}\n", n.x, n.y, n.z));
        s.push_str("    outer loop\n");
        for &v in &tri {
            let p = positions[v];
            s.push_str(&format!("      vertex {} {} {}\n", p.x, p.y, p.z));
        }
        s.push_str("    endloop\n");
        s.push_str("  endfacet\n");
    }
    s.push_str("endsolid outram_blender\n");
    s
}

/// Fan-triangulate `mesh` and return it as a **binary** STL byte buffer.
///
/// Layout: an 80-byte header, a little-endian `u32` triangle count, then 50
/// bytes per triangle (facet normal + three vertices as `f32`, plus a `u16`
/// attribute count of `0`). Coordinates are stored to `f32` precision.
pub fn to_stl_binary(mesh: &Mesh) -> Vec<u8> {
    let positions = mesh.positions();
    let tris = triangles(mesh);
    let mut out = Vec::with_capacity(84 + 50 * tris.len());
    // 80-byte header (a label, zero-padded).
    let mut header = [0u8; 80];
    let tag = b"OUTRAM PARK outram-blender binary STL";
    header[..tag.len()].copy_from_slice(tag);
    out.extend_from_slice(&header);
    out.extend_from_slice(&(tris.len() as u32).to_le_bytes());
    for tri in &tris {
        let n = facet_normal(&positions, *tri);
        for c in [n.x, n.y, n.z] {
            out.extend_from_slice(&(c as f32).to_le_bytes());
        }
        for &v in tri {
            let p = positions[v];
            for c in [p.x, p.y, p.z] {
                out.extend_from_slice(&(c as f32).to_le_bytes());
            }
        }
        out.extend_from_slice(&0u16.to_le_bytes()); // attribute byte count
    }
    out
}

/// Write `mesh` to `path` as ASCII STL.
pub fn write_stl_ascii(mesh: &Mesh, path: &Path) -> std::io::Result<()> {
    std::fs::write(path, to_stl_ascii(mesh))
}

/// Write `mesh` to `path` as binary STL.
pub fn write_stl_binary(mesh: &Mesh, path: &Path) -> std::io::Result<()> {
    std::fs::write(path, to_stl_binary(mesh))
}

// ---------------------------------------------------------------------------
// Reading
// ---------------------------------------------------------------------------

/// Parse an **ASCII** STL string into a [`Mesh`] (an unwelded triangle soup).
///
/// Facet normals in the file are ignored (recomputed on any re-export). The
/// result has one vertex per triangle corner; [`crate::weld::weld`] it to
/// recover shared-vertex topology.
///
/// # Errors
///
/// [`StlError::Parse`] if a `vertex` line lacks three parseable coordinates or
/// the vertex count is not a multiple of three.
pub fn from_stl_ascii(text: &str) -> Result<Mesh, StlError> {
    let mut coords: Vec<Vec3> = Vec::new();
    let mut it = text.split_whitespace();
    while let Some(tok) = it.next() {
        if tok == "vertex" {
            let x = next_f64(&mut it)?;
            let y = next_f64(&mut it)?;
            let z = next_f64(&mut it)?;
            coords.push(Vec3::new(x, y, z));
        }
    }
    if coords.len() % 3 != 0 {
        return Err(StlError::Parse(format!(
            "vertex count {} is not a multiple of 3",
            coords.len()
        )));
    }
    Ok(soup_to_mesh(&coords))
}

/// Parse a **binary** STL byte buffer into a [`Mesh`] (an unwelded triangle
/// soup).
///
/// # Errors
///
/// [`StlError::Truncated`] if shorter than the 84-byte minimum, or
/// [`StlError::BadBinaryLength`] if the length does not match the header's
/// triangle count.
pub fn from_stl_binary(bytes: &[u8]) -> Result<Mesh, StlError> {
    if bytes.len() < 84 {
        return Err(StlError::Truncated(bytes.len()));
    }
    let count = u32::from_le_bytes([bytes[80], bytes[81], bytes[82], bytes[83]]) as usize;
    let expected = 84 + 50 * count;
    if bytes.len() != expected {
        return Err(StlError::BadBinaryLength {
            got: bytes.len(),
            expected,
        });
    }
    let mut coords: Vec<Vec3> = Vec::with_capacity(count * 3);
    for t in 0..count {
        let base = 84 + t * 50;
        // Skip the 12-byte facet normal; read the three vertices (9 f32).
        for v in 0..3 {
            let off = base + 12 + v * 12;
            let x = read_f32(bytes, off);
            let y = read_f32(bytes, off + 4);
            let z = read_f32(bytes, off + 8);
            coords.push(Vec3::new(x as f64, y as f64, z as f64));
        }
    }
    Ok(soup_to_mesh(&coords))
}

/// Parse an STL byte buffer, **auto-detecting** ASCII vs binary.
///
/// A stream is treated as binary when its length exactly matches the binary
/// size invariant `84 + 50 * count` (read from the header); otherwise it is
/// parsed as ASCII. This is the robust test — an ASCII file that merely starts
/// with the word `solid` will not accidentally satisfy the binary length.
pub fn from_stl_bytes(bytes: &[u8]) -> Result<Mesh, StlError> {
    if bytes.len() >= 84 {
        let count = u32::from_le_bytes([bytes[80], bytes[81], bytes[82], bytes[83]]) as usize;
        if bytes.len() == 84 + 50 * count {
            return from_stl_binary(bytes);
        }
    }
    let text = std::str::from_utf8(bytes)
        .map_err(|e| StlError::Parse(format!("not binary and not valid UTF-8: {e}")))?;
    from_stl_ascii(text)
}

/// Read an STL file from `path`, auto-detecting ASCII vs binary.
pub fn read_stl(path: &Path) -> Result<Mesh, StlError> {
    let bytes = std::fs::read(path)?;
    from_stl_bytes(&bytes)
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

/// Fan-triangulate every face into `[v0, vi, vi+1]` index triples.
fn triangles(mesh: &Mesh) -> Vec<[usize; 3]> {
    let mut tris = Vec::new();
    for poly in mesh.polygons() {
        let k = poly.len();
        if k < 3 {
            continue;
        }
        for i in 1..k - 1 {
            tris.push([poly[0].0, poly[i].0, poly[i + 1].0]);
        }
    }
    tris
}

/// Outward unit normal of a triangle, or [`Vec3::ZERO`] if degenerate.
fn facet_normal(positions: &[Vec3], tri: [usize; 3]) -> Vec3 {
    let a = positions[tri[0]];
    let b = positions[tri[1]];
    let c = positions[tri[2]];
    b.sub(a).cross(c.sub(a)).normalize()
}

/// Build a triangle-soup [`Mesh`] from a flat list of corner positions (three
/// consecutive entries per triangle).
fn soup_to_mesh(coords: &[Vec3]) -> Mesh {
    let faces: Vec<Vec<usize>> = (0..coords.len() / 3)
        .map(|t| vec![3 * t, 3 * t + 1, 3 * t + 2])
        .collect();
    Mesh::from_polygons(coords, &faces)
}

/// Read the next whitespace token as an `f64`.
fn next_f64<'a>(it: &mut impl Iterator<Item = &'a str>) -> Result<f64, StlError> {
    let tok = it
        .next()
        .ok_or_else(|| StlError::Parse("truncated vertex line".into()))?;
    tok.parse::<f64>()
        .map_err(|e| StlError::Parse(format!("bad coordinate {tok:?}: {e}")))
}

/// Read a little-endian `f32` at byte offset `off`.
fn read_f32(bytes: &[u8], off: usize) -> f32 {
    f32::from_le_bytes([bytes[off], bytes[off + 1], bytes[off + 2], bytes[off + 3]])
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::primitives;
    use crate::weld::weld;

    /// V&V — ASCII round-trip. Methodology: `cube(2.0)` → `to_stl_ascii` →
    /// `from_stl_ascii` → `weld`. Pass criterion: the welded import is the
    /// original cube topology. Result: 12 facets written; the soup reads back as
    /// 36 vertices / 12 triangles and welds to V = 8, E = 18, F = 12, χ = 2.
    #[test]
    fn ascii_round_trip_cube() {
        let cube = primitives::cube(2.0);
        let text = to_stl_ascii(&cube);
        assert_eq!(
            text.matches("facet normal").count(),
            12,
            "6 quads → 12 facets"
        );

        let soup = from_stl_ascii(&text).expect("parse ascii STL");
        assert_eq!(
            soup.vertex_count(),
            36,
            "triangle soup: 12 tris × 3 corners"
        );
        assert_eq!(soup.face_count(), 12);

        let welded = weld(&soup, 1e-9);
        assert_eq!(welded.vertex_count(), 8, "welds back to 8 corners");
        assert_eq!(welded.edge_count(), 18, "12 cube edges + 6 diagonals");
        assert_eq!(welded.face_count(), 12);
        assert_eq!(welded.euler_characteristic(), 2);
    }

    /// V&V — binary round-trip. Methodology: `cube(2.0)` → `to_stl_binary` →
    /// `from_stl_binary` → `weld`. Pass criterion: correct binary size and the
    /// welded import matches the cube. Result: byte length = 84 + 50·12 = 684;
    /// welds to V = 8, F = 12, χ = 2 (positions exact to f32, so a 1e-4 weld
    /// tolerance is ample).
    #[test]
    fn binary_round_trip_cube() {
        let cube = primitives::cube(2.0);
        let bytes = to_stl_binary(&cube);
        assert_eq!(
            bytes.len(),
            84 + 50 * 12,
            "80 header + 4 count + 50/tri × 12"
        );

        let soup = from_stl_binary(&bytes).expect("parse binary STL");
        assert_eq!(soup.face_count(), 12);
        let welded = weld(&soup, 1e-4);
        assert_eq!(welded.vertex_count(), 8);
        assert_eq!(welded.face_count(), 12);
        assert_eq!(welded.euler_characteristic(), 2);
    }

    /// V&V — auto-detect routes each encoding to the right parser. Binary bytes
    /// (matching the size invariant) and ASCII text both import to 12 faces.
    #[test]
    fn auto_detect_ascii_and_binary() {
        let cube = primitives::cube(2.0);
        let from_bin = from_stl_bytes(&to_stl_binary(&cube)).expect("auto binary");
        let from_txt = from_stl_bytes(to_stl_ascii(&cube).as_bytes()).expect("auto ascii");
        assert_eq!(from_bin.face_count(), 12);
        assert_eq!(from_txt.face_count(), 12);
    }

    /// V&V — positions survive an ASCII round-trip exactly (full f64 precision).
    #[test]
    fn ascii_preserves_positions() {
        let sphere = primitives::uv_sphere(8, 4, 1.0);
        let welded = weld(&from_stl_ascii(&to_stl_ascii(&sphere)).unwrap(), 1e-9);
        // Every original vertex has an exact match in the welded import.
        let orig = sphere.positions();
        let got = welded.positions();
        for p in &orig {
            assert!(
                got.iter().any(|q| q.sub(*p).length() < 1e-12),
                "vertex {p:?} must survive the round-trip",
            );
        }
    }

    /// V&V — a malformed ASCII vertex line is reported, not silently accepted.
    #[test]
    fn malformed_ascii_errors() {
        let bad = "solid x\n facet normal 0 0 1\n outer loop\n vertex 0 0\n";
        assert!(matches!(from_stl_ascii(bad), Err(StlError::Parse(_))));
    }
}
