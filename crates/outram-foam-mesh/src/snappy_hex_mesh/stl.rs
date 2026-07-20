// SPDX-License-Identifier: GPL-3.0-only
// Copyright (C) 2026 OUTRAM PARK contributors
// Derived from OpenFOAM (www.openfoam.com)
// Copyright (C) 2004-2023 OpenFOAM Foundation
// Copyright (C) 2016-2023 OpenCFD Ltd.
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

//! Triangulated-surface (STL) input for `snappyHexMesh`.
//!
//! A stereolithography (STL) file is a "triangle soup": an unordered list of
//! flat triangular facets, each with three corner points [m] and a (frequently
//! unreliable) stored normal. `snappyHexMesh` uses this surface for three
//! purposes, all provided here:
//!
//! - **Proximity** — how close a background cell sits to the surface, driving
//!   octree refinement ([`TriangleSoup::nearest_point`],
//!   [`TriangleSoup::distance_to`]).
//! - **Inside/outside** — which side of a *closed* surface a point lies on,
//!   driving region removal ([`TriangleSoup::contains_point`], by odd/even ray
//!   crossing).
//! - **Projection** — the nearest surface point a boundary vertex snaps to
//!   (Phase 2; [`Triangle::closest_point`]).
//!
//! Both ASCII and little-endian binary STL are read. Coordinates are treated as
//! metres to match the rest of the mesh layer; STL itself is unit-less, so the
//! caller is responsible for supplying a metre-scaled file.

use crate::MeshError;
use outram_foam_basic_lib::primitives::Vector3;

/// One triangular facet of a surface [m].
///
/// The three corners `a`, `b`, `c` are stored in the file's winding order. The
/// geometric normal is recomputed from the corners (via [`Triangle::normal`])
/// rather than trusting the file's stored normal, which many CAD exporters get
/// wrong.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Triangle {
    /// First corner [m].
    pub a: Vector3,
    /// Second corner [m].
    pub b: Vector3,
    /// Third corner [m].
    pub c: Vector3,
}

impl Triangle {
    /// Construct a triangle from its three corner points [m].
    pub fn new(a: Vector3, b: Vector3, c: Vector3) -> Self {
        Self { a, b, c }
    }

    /// Geometric (right-hand-rule) unit normal `(b-a) × (c-a)`, normalised.
    /// Returns the zero vector for a degenerate (zero-area) triangle.
    pub fn normal(&self) -> Vector3 {
        (self.b - self.a).cross(self.c - self.a).normalise(1e-300)
    }

    /// Twice the triangle area (magnitude of the un-normalised cross product) [m²].
    pub fn area(&self) -> f64 {
        0.5 * (self.b - self.a).cross(self.c - self.a).mag()
    }

    /// Centroid (arithmetic mean of the three corners) [m].
    pub fn centroid(&self) -> Vector3 {
        (self.a + self.b + self.c) / 3.0
    }

    /// Closest point on the (filled) triangle to `p` [m].
    ///
    /// Uses the Voronoi-region method of Ericson, *Real-Time Collision
    /// Detection* (2005), §5.1.5 — it classifies `p` against the triangle's
    /// vertex, edge, and face Voronoi regions and returns the exact projection.
    /// This is the primitive Phase-2 snapping projects boundary points with.
    pub fn closest_point(&self, p: Vector3) -> Vector3 {
        let (a, b, c) = (self.a, self.b, self.c);
        let ab = b - a;
        let ac = c - a;
        let ap = p - a;

        let d1 = ab.dot(ap);
        let d2 = ac.dot(ap);
        // Vertex region outside A.
        if d1 <= 0.0 && d2 <= 0.0 {
            return a;
        }

        let bp = p - b;
        let d3 = ab.dot(bp);
        let d4 = ac.dot(bp);
        // Vertex region outside B.
        if d3 >= 0.0 && d4 <= d3 {
            return b;
        }

        // Edge region AB.
        let vc = d1 * d4 - d3 * d2;
        if vc <= 0.0 && d1 >= 0.0 && d3 <= 0.0 {
            let v = d1 / (d1 - d3);
            return a + v * ab;
        }

        let cp = p - c;
        let d5 = ab.dot(cp);
        let d6 = ac.dot(cp);
        // Vertex region outside C.
        if d6 >= 0.0 && d5 <= d6 {
            return c;
        }

        // Edge region AC.
        let vb = d5 * d2 - d1 * d6;
        if vb <= 0.0 && d2 >= 0.0 && d6 <= 0.0 {
            let w = d2 / (d2 - d6);
            return a + w * ac;
        }

        // Edge region BC.
        let va = d3 * d6 - d5 * d4;
        if va <= 0.0 && (d4 - d3) >= 0.0 && (d5 - d6) >= 0.0 {
            let w = (d4 - d3) / ((d4 - d3) + (d5 - d6));
            return b + w * (c - b);
        }

        // Interior (face) region — barycentric projection.
        let denom = 1.0 / (va + vb + vc);
        let v = vb * denom;
        let w = vc * denom;
        a + v * ab + w * ac
    }

    /// Signed ray/triangle intersection distance for a ray `origin + t*dir`,
    /// via the Möller–Trumbore algorithm. Returns `Some(t)` with `t > eps`
    /// when the ray crosses the triangle interior, else `None`.
    ///
    /// Used by [`TriangleSoup::contains_point`] to count surface crossings.
    pub fn ray_intersection(&self, origin: Vector3, dir: Vector3) -> Option<f64> {
        const EPS: f64 = 1e-12;
        let e1 = self.b - self.a;
        let e2 = self.c - self.a;
        let pvec = dir.cross(e2);
        let det = e1.dot(pvec);
        if det.abs() < EPS {
            return None; // ray parallel to triangle plane
        }
        let inv_det = 1.0 / det;
        let tvec = origin - self.a;
        let u = tvec.dot(pvec) * inv_det;
        if !(0.0..=1.0).contains(&u) {
            return None;
        }
        let qvec = tvec.cross(e1);
        let v = dir.dot(qvec) * inv_det;
        if v < 0.0 || u + v > 1.0 {
            return None;
        }
        let t = e2.dot(qvec) * inv_det;
        if t > EPS {
            Some(t)
        } else {
            None
        }
    }
}

/// An unordered collection of triangular facets — a surface "triangle soup".
///
/// For inside/outside queries to be meaningful the surface must be closed and
/// (ideally) manifold; `snappyHexMesh` requires this of its input STL too.
#[derive(Debug, Clone, Default)]
pub struct TriangleSoup {
    /// The facets making up the surface [m].
    pub triangles: Vec<Triangle>,
    /// Optional solid name from the STL `solid <name>` line.
    pub name: String,
}

impl TriangleSoup {
    /// Build a soup from an explicit list of triangles (used by tests that
    /// synthesise a surface without a file).
    pub fn new(name: impl Into<String>, triangles: Vec<Triangle>) -> Self {
        Self {
            triangles,
            name: name.into(),
        }
    }

    /// Number of facets.
    pub fn len(&self) -> usize {
        self.triangles.len()
    }

    /// True if the soup has no facets.
    pub fn is_empty(&self) -> bool {
        self.triangles.is_empty()
    }

    /// Axis-aligned bounding box `(min, max)` of every vertex [m].
    /// Returns `None` for an empty soup.
    pub fn bounding_box(&self) -> Option<(Vector3, Vector3)> {
        let mut it = self
            .triangles
            .iter()
            .flat_map(|t| [t.a, t.b, t.c].into_iter());
        let first = it.next()?;
        let mut lo = first;
        let mut hi = first;
        for v in it {
            lo = Vector3::new(lo.x.min(v.x), lo.y.min(v.y), lo.z.min(v.z));
            hi = Vector3::new(hi.x.max(v.x), hi.y.max(v.y), hi.z.max(v.z));
        }
        Some((lo, hi))
    }

    /// Closest point on the whole surface to `p` [m] (brute force over all
    /// facets). Returns `None` for an empty soup.
    pub fn nearest_point(&self, p: Vector3) -> Option<Vector3> {
        let mut best: Option<(f64, Vector3)> = None;
        for t in &self.triangles {
            let q = t.closest_point(p);
            let d2 = p.dist_sqr(q);
            if best.map(|(bd, _)| d2 < bd).unwrap_or(true) {
                best = Some((d2, q));
            }
        }
        best.map(|(_, q)| q)
    }

    /// Euclidean distance from `p` to the nearest surface point [m].
    /// Returns `f64::INFINITY` for an empty soup.
    pub fn distance_to(&self, p: Vector3) -> f64 {
        match self.nearest_point(p) {
            Some(q) => p.dist(q),
            None => f64::INFINITY,
        }
    }

    /// True if `p` is inside the *closed* surface, by ray-crossing parity
    /// (odd number of forward crossings ⇒ inside).
    ///
    /// The test ray uses a fixed, deliberately non-axis-aligned direction so it
    /// is very unlikely to graze a shared triangle edge — the classic weakness
    /// of an axis-aligned ray. It is not bullet-proof against a ray passing
    /// exactly through a vertex; for the smooth closed surfaces used in the
    /// tests this is not hit. A degenerate/open surface gives meaningless
    /// results (documented limitation).
    pub fn contains_point(&self, p: Vector3) -> bool {
        // Irrational-ish direction avoids axis-aligned edge grazing.
        let dir = Vector3::new(0.577_350_3, 0.577_9, 0.576_1).normalise(1e-30);
        let mut crossings = 0usize;
        for t in &self.triangles {
            if t.ray_intersection(p, dir).is_some() {
                crossings += 1;
            }
        }
        crossings % 2 == 1
    }
}

// ── Readers ─────────────────────────────────────────────────────────────────

/// Read an STL file from `path`, auto-detecting ASCII vs binary.
///
/// Detection: a file whose leading non-whitespace token is `solid` *and* whose
/// body contains the `facet` keyword is treated as ASCII; otherwise it is read
/// as little-endian binary. (A binary STL may legally begin with the bytes
/// `solid`, so the `facet` check disambiguates.)
pub fn read_stl(path: impl AsRef<std::path::Path>) -> Result<TriangleSoup, MeshError> {
    let bytes = std::fs::read(path.as_ref())
        .map_err(|e| MeshError::DictParse(format!("cannot read STL file: {e}")))?;
    read_stl_bytes(&bytes)
}

/// Parse STL from an in-memory byte buffer, auto-detecting ASCII vs binary.
pub fn read_stl_bytes(bytes: &[u8]) -> Result<TriangleSoup, MeshError> {
    let looks_ascii = {
        let head = &bytes[..bytes.len().min(512)];
        let text = String::from_utf8_lossy(head);
        let trimmed = text.trim_start();
        trimmed.starts_with("solid") && text.contains("facet")
    };
    if looks_ascii {
        let text = String::from_utf8_lossy(bytes);
        read_stl_ascii_str(&text)
    } else {
        read_stl_binary(bytes)
    }
}

/// Parse an ASCII STL from a string.
///
/// Grammar (whitespace-insensitive):
/// ```text
/// solid <name>
///   facet normal nx ny nz
///     outer loop
///       vertex x y z
///       vertex x y z
///       vertex x y z
///     endloop
///   endfacet
///   ...
/// endsolid <name>
/// ```
/// The stored `normal` is parsed but ignored (the geometric normal is used).
pub fn read_stl_ascii_str(text: &str) -> Result<TriangleSoup, MeshError> {
    let mut tokens = text.split_whitespace();
    let mut name = String::new();
    let mut triangles = Vec::new();

    // Expect leading `solid [name]`.
    match tokens.next() {
        Some("solid") => {}
        other => {
            return Err(MeshError::DictParse(format!(
                "ASCII STL must start with `solid`, found {other:?}"
            )))
        }
    }

    // The solid name runs to end-of-line; split_whitespace lost the newline, so
    // we accept a single-token name and stop collecting once we hit `facet` or
    // `endsolid`. Simpler: peek tokens and treat the first non-keyword run as
    // the name is fragile, so we just record the next token if it is not a
    // keyword.
    let mut pending = tokens.next();
    if let Some(tok) = pending {
        if tok != "facet" && tok != "endsolid" {
            name = tok.to_string();
            pending = tokens.next();
        }
    }

    // Helper to pull the next token, threading the one-token lookahead.
    let mut next = |pending: &mut Option<&str>| -> Option<String> {
        if let Some(t) = pending.take() {
            Some(t.to_string())
        } else {
            tokens.next().map(|s| s.to_string())
        }
    };

    let parse_f = |s: &str| -> Result<f64, MeshError> {
        s.parse::<f64>()
            .map_err(|_| MeshError::DictParse(format!("bad float in STL: {s:?}")))
    };

    while let Some(tok) = next(&mut pending) {
        match tok.as_str() {
            "facet" => {
                // `normal nx ny nz` — consume and discard.
                let kw = next(&mut pending);
                if kw.as_deref() != Some("normal") {
                    return Err(MeshError::DictParse(format!(
                        "expected `normal` after `facet`, found {kw:?}"
                    )));
                }
                for _ in 0..3 {
                    next(&mut pending);
                }
                // `outer loop`
                let outer = next(&mut pending);
                let loopkw = next(&mut pending);
                if outer.as_deref() != Some("outer") || loopkw.as_deref() != Some("loop") {
                    return Err(MeshError::DictParse(
                        "expected `outer loop` in facet".to_string(),
                    ));
                }
                let mut verts = [Vector3::ZERO; 3];
                for v in verts.iter_mut() {
                    let vk = next(&mut pending);
                    if vk.as_deref() != Some("vertex") {
                        return Err(MeshError::DictParse(format!(
                            "expected `vertex`, found {vk:?}"
                        )));
                    }
                    let x = parse_f(&next(&mut pending).ok_or_else(eof)?)?;
                    let y = parse_f(&next(&mut pending).ok_or_else(eof)?)?;
                    let z = parse_f(&next(&mut pending).ok_or_else(eof)?)?;
                    *v = Vector3::new(x, y, z);
                }
                // `endloop endfacet`
                let el = next(&mut pending);
                let ef = next(&mut pending);
                if el.as_deref() != Some("endloop") || ef.as_deref() != Some("endfacet") {
                    return Err(MeshError::DictParse(
                        "expected `endloop endfacet`".to_string(),
                    ));
                }
                triangles.push(Triangle::new(verts[0], verts[1], verts[2]));
            }
            "endsolid" => break,
            other => {
                return Err(MeshError::DictParse(format!(
                    "unexpected token in ASCII STL: {other:?}"
                )))
            }
        }
    }

    Ok(TriangleSoup { triangles, name })
}

fn eof() -> MeshError {
    MeshError::DictParse("unexpected end of STL file".to_string())
}

/// Parse a little-endian binary STL from a byte buffer.
///
/// Layout: 80-byte header, `u32` facet count, then per facet 12 × `f32`
/// (normal + 3 vertices) plus a `u16` attribute-byte-count, all little-endian.
pub fn read_stl_binary(bytes: &[u8]) -> Result<TriangleSoup, MeshError> {
    if bytes.len() < 84 {
        return Err(MeshError::DictParse(
            "binary STL shorter than 84-byte header+count".to_string(),
        ));
    }
    let count = u32::from_le_bytes([bytes[80], bytes[81], bytes[82], bytes[83]]) as usize;
    let expected = 84 + count * 50;
    if bytes.len() < expected {
        return Err(MeshError::DictParse(format!(
            "binary STL truncated: need {expected} bytes for {count} facets, have {}",
            bytes.len()
        )));
    }
    let rf = |off: usize| -> f64 {
        f32::from_le_bytes([
            bytes[off],
            bytes[off + 1],
            bytes[off + 2],
            bytes[off + 3],
        ]) as f64
    };
    let mut triangles = Vec::with_capacity(count);
    for i in 0..count {
        let base = 84 + i * 50 + 12; // skip the 3-float normal
        let a = Vector3::new(rf(base), rf(base + 4), rf(base + 8));
        let b = Vector3::new(rf(base + 12), rf(base + 16), rf(base + 20));
        let c = Vector3::new(rf(base + 24), rf(base + 28), rf(base + 32));
        triangles.push(Triangle::new(a, b, c));
    }
    Ok(TriangleSoup {
        triangles,
        name: String::new(),
    })
}
