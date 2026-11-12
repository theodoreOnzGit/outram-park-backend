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
//! flat triangular facets, each with three corner points `[m]` and a (frequently
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

use petir::mathf::RealMath;
use crate::MeshError;
use outram_foam_basic_lib::primitives::Vector3;

/// One triangular facet of a surface `[m]`.
///
/// The three corners `a`, `b`, `c` are stored in the file's winding order. The
/// geometric normal is recomputed from the corners (via [`Triangle::normal`])
/// rather than trusting the file's stored normal, which many CAD exporters get
/// wrong.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Triangle {
    /// First corner `[m]`.
    pub a: Vector3,
    /// Second corner `[m]`.
    pub b: Vector3,
    /// Third corner `[m]`.
    pub c: Vector3,
}

impl Triangle {
    /// Construct a triangle from its three corner points `[m]`.
    pub fn new(a: Vector3, b: Vector3, c: Vector3) -> Self {
        Self { a, b, c }
    }

    /// Geometric (right-hand-rule) unit normal `(b-a) × (c-a)`, normalised.
    /// Returns the zero vector for a degenerate (zero-area) triangle.
    pub fn normal(&self) -> Vector3 {
        (self.b - self.a).cross(self.c - self.a).normalise(1e-300)
    }

    /// Twice the triangle area (magnitude of the un-normalised cross product) `[m²]`.
    pub fn area(&self) -> f64 {
        0.5 * (self.b - self.a).cross(self.c - self.a).mag()
    }

    /// Centroid (arithmetic mean of the three corners) `[m]`.
    pub fn centroid(&self) -> Vector3 {
        (self.a + self.b + self.c) / 3.0
    }

    /// Closest point on the (filled) triangle to `p` `[m]`.
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
    /// The facets making up the surface `[m]`.
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

    /// The surface as OpenFOAM's `triSurface::readSTL` hands it to
    /// `snappyHexMesh`: every solid, corners merged, coordinates single
    /// precision (see [`StlZoned`]). Region membership is not kept here
    /// (`StlZoned::zones` has it).
    pub fn from_zoned(z: &StlZoned) -> Self {
        let triangles = z
            .triangles
            .iter()
            .map(|t| Triangle::new(z.points[t[0]], z.points[t[1]], z.points[t[2]]))
            .collect();
        Self { triangles, name: z.names.first().cloned().unwrap_or_default() }
    }

    /// A closed UV sphere of radius `r` `[m]` centred at `centre` `[m]`, as an
    /// outward-wound triangle soup.
    ///
    /// Built-in geometry so a caller can exercise the mesher without supplying
    /// an STL file. The surface is tessellated with `n_lat` latitude bands ×
    /// `n_lon` longitude sectors (`2 · n_lat · n_lon` triangles), so it
    /// approximates the analytic sphere with a chord error of order
    /// `r · (π / n_lat)² / 8`. Both counts must be `>= 3` for a closed surface;
    /// 24 × 24 is a reasonable default (1152 facets, chord error ≈ 0.2 % of `r`).
    ///
    /// Every facet is wound counter-clockwise seen from outside, so
    /// [`contains_point`](Self::contains_point) treats the interior as "inside".
    ///
    /// # Panics
    /// If `n_lat < 3`, `n_lon < 3`, or `r <= 0`.
    pub fn uv_sphere(centre: Vector3, r: f64, n_lat: usize, n_lon: usize) -> Self {
        assert!(n_lat >= 3 && n_lon >= 3, "need at least 3×3 tessellation");
        assert!(r > 0.0, "sphere radius must be positive");
        let point = |ia: f64, io: f64| -> Vector3 {
            let theta = std::f64::consts::PI * ia; // 0..π  (polar angle)
            let phi = 2.0 * std::f64::consts::PI * io; // 0..2π (azimuth)
            Vector3::new(
                centre.x + r * theta.r_sin() * phi.r_cos(),
                centre.y + r * theta.r_sin() * phi.r_sin(),
                centre.z + r * theta.r_cos(),
            )
        };
        let mut tris = Vec::with_capacity(2 * n_lat * n_lon);
        for i in 0..n_lat {
            for j in 0..n_lon {
                let (a0, a1) = (i as f64 / n_lat as f64, (i + 1) as f64 / n_lat as f64);
                let (o0, o1) = (j as f64 / n_lon as f64, (j + 1) as f64 / n_lon as f64);
                let p00 = point(a0, o0);
                let p01 = point(a0, o1);
                let p10 = point(a1, o0);
                let p11 = point(a1, o1);
                tris.push(Triangle::new(p00, p11, p01));
                tris.push(Triangle::new(p00, p10, p11));
            }
        }
        Self::new("sphere", tris)
    }

    /// A closed axis-aligned box spanning `min`..`max` `[m]`, as an
    /// outward-wound triangle soup (12 facets).
    ///
    /// Built-in geometry for testing feature snapping: a box has 90° creases,
    /// so any [`SnapControls::feature_angle_deg`](crate::snappy_hex_mesh::SnapControls::feature_angle_deg)
    /// below 90 detects all of its edges.
    ///
    /// # Panics
    /// If any component of `max` is not strictly greater than `min`.
    pub fn cuboid(min: Vector3, max: Vector3) -> Self {
        assert!(
            max.x > min.x && max.y > min.y && max.z > min.z,
            "cuboid needs max > min on every axis"
        );
        let c = |x: f64, y: f64, z: f64| Vector3::new(x, y, z);
        let (x0, y0, z0) = (min.x, min.y, min.z);
        let (x1, y1, z1) = (max.x, max.y, max.z);
        // Each quad is split into two outward-wound triangles.
        let quad = |a: Vector3, b: Vector3, d: Vector3, e: Vector3| {
            [Triangle::new(a, b, d), Triangle::new(a, d, e)]
        };
        let mut tris = Vec::with_capacity(12);
        // x-min (outward −x), x-max (+x)
        tris.extend(quad(
            c(x0, y0, z0),
            c(x0, y0, z1),
            c(x0, y1, z1),
            c(x0, y1, z0),
        ));
        tris.extend(quad(
            c(x1, y0, z0),
            c(x1, y1, z0),
            c(x1, y1, z1),
            c(x1, y0, z1),
        ));
        // y-min (−y), y-max (+y)
        tris.extend(quad(
            c(x0, y0, z0),
            c(x1, y0, z0),
            c(x1, y0, z1),
            c(x0, y0, z1),
        ));
        tris.extend(quad(
            c(x0, y1, z0),
            c(x0, y1, z1),
            c(x1, y1, z1),
            c(x1, y1, z0),
        ));
        // z-min (−z), z-max (+z)
        tris.extend(quad(
            c(x0, y0, z0),
            c(x0, y1, z0),
            c(x1, y1, z0),
            c(x1, y0, z0),
        ));
        tris.extend(quad(
            c(x0, y0, z1),
            c(x1, y0, z1),
            c(x1, y1, z1),
            c(x0, y1, z1),
        ));
        Self::new("cuboid", tris)
    }

    /// Number of facets.
    pub fn len(&self) -> usize {
        self.triangles.len()
    }

    /// True if the soup has no facets.
    pub fn is_empty(&self) -> bool {
        self.triangles.is_empty()
    }

    /// Axis-aligned bounding box `(min, max)` of every vertex `[m]`.
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

    /// Closest point on the whole surface to `p` `[m]` (brute force over all
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

    /// Euclidean distance from `p` to the nearest surface point `[m]`.
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

    /// True if the closed segment `a`–`b` crosses any facet (any-hit, the
    /// analogue of `searchableSurface::findLineAny`, which
    /// `refinementSurfaces::findHigherIntersection` calls in OpenFOAM-dev
    /// `852527377c7e`, `refinementSurfaces.C:410`).
    ///
    /// Möller–Trumbore with the direction `b - a`, so the hit parameter is
    /// accepted on `[0, 1]`. Brute force over the facets with a per-facet
    /// bounding-box cull; an accelerated search is tracked in gh:#1470.
    pub fn segment_hits(&self, a: Vector3, b: Vector3) -> bool {
        let dir = b - a;
        let lo = Vector3::new(a.x.min(b.x), a.y.min(b.y), a.z.min(b.z));
        let hi = Vector3::new(a.x.max(b.x), a.y.max(b.y), a.z.max(b.z));
        for t in &self.triangles {
            if t.a.x.max(t.b.x).max(t.c.x) < lo.x
                || t.a.x.min(t.b.x).min(t.c.x) > hi.x
                || t.a.y.max(t.b.y).max(t.c.y) < lo.y
                || t.a.y.min(t.b.y).min(t.c.y) > hi.y
                || t.a.z.max(t.b.z).max(t.c.z) < lo.z
                || t.a.z.min(t.b.z).min(t.c.z) > hi.z
            {
                continue;
            }
            let e1 = t.b - t.a;
            let e2 = t.c - t.a;
            let pvec = dir.cross(e2);
            let det = e1.dot(pvec);
            let scale = e1.mag() * e2.mag() * dir.mag();
            if det.abs() <= 1e-14 * scale {
                continue; // segment parallel to the facet plane
            }
            let inv = 1.0 / det;
            let tvec = a - t.a;
            let u = tvec.dot(pvec) * inv;
            if !(0.0..=1.0).contains(&u) {
                continue;
            }
            let qvec = tvec.cross(e1);
            let v = dir.dot(qvec) * inv;
            if v < 0.0 || u + v > 1.0 {
                continue;
            }
            let s = e2.dot(qvec) * inv;
            if (0.0..=1.0).contains(&s) {
                return true;
            }
        }
        false
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
/// Several such blocks in one file are all read (one soup, region boundaries
/// not kept; see [`read_stl_zoned_bytes`] for the per-region reader).
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
    let mut pending: Option<String> = tokens.next().map(str::to_string);
    if let Some(tok) = pending.clone() {
        if tok != "facet" && tok != "endsolid" {
            name = tok;
            pending = tokens.next().map(str::to_string);
        }
    }

    // Helper to pull the next token, threading the one-token lookahead.
    let mut next = |pending: &mut Option<String>| -> Option<String> {
        if let Some(t) = pending.take() {
            Some(t)
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
            "endsolid" => {
                // A file may hold several `solid ... endsolid` blocks (one per
                // region, as the OpenFOAM `flange.stl` has four): skip the
                // optional name, and carry on with the next `solid`. Upstream
                // `STLReader` reads every solid (`STLAsciiParse`); stopping at
                // the first one silently dropped the other regions.
                let mut resumed = false;
                while let Some(t) = next(&mut pending) {
                    if t == "solid" {
                        resumed = true;
                        break;
                    }
                }
                if !resumed {
                    break;
                }
                // Optional name of the new solid (any words, up to a keyword).
                while let Some(t) = next(&mut pending) {
                    if t == "facet" || t == "endsolid" {
                        pending = Some(t);
                        break;
                    }
                }
            }
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
        f32::from_le_bytes([bytes[off], bytes[off + 1], bytes[off + 2], bytes[off + 3]]) as f64
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

// ── OpenFOAM's STLReader: zones, single precision, merged points ───────────
//
// Ported from OpenFOAM-v2506 (www.openfoam.com), GPL-3.0-or-later:
//   src/fileFormats/stl/STLReader.C (readBINARY, readFile, mergePointsMap),
//   src/fileFormats/stl/STLCore.C (detectBinaryHeader, startsWithSolid,
//     checkBinaryFileSize),
//   src/fileFormats/stl/STLAsciiParseI.H, STLAsciiParseFlex.L (beginSolid,
//     addVertexComponent, endFacet),
//   src/surfMesh/surfaceFormats/stl/triSurfaceSTLformat.C (triSurface::readSTL),
//   src/OpenFOAM/meshes/meshTools/mergePoints.C (Detail::mergePoints).
// Copyright (C) 2011-2016 OpenFOAM Foundation; Copyright (C) 2016-2025
// OpenCFD Ltd. Added for gh:#1476 (cfMesh's `surfaceFile`): cfMesh's
// `triSurf(fileName)` reads an STL through `triSurface`, so this is the reader
// upstream cfMesh runs. The functions above ([`read_stl`] and friends) are the
// snappyHexMesh driver's reader and are unchanged.

/// An STL as OpenFOAM's `triSurface::readSTL` returns it: points merged,
/// one zone (patch) per `solid` name (ASCII) or per attribute value (binary).
///
/// Differences from [`read_stl_bytes`] that matter for parity with upstream:
/// coordinates are rounded to **single precision** (`STLpoint` is `float`),
/// every `solid` of a multi-solid ASCII file is read (not only the first),
/// and coincident corners are merged into shared points.
#[derive(Debug, Clone, PartialEq)]
pub struct StlZoned {
    /// The merged points `[m]`, in order of first appearance.
    pub points: Vec<Vector3>,
    /// Triangles (indices into [`Self::points`]), file order.
    pub triangles: Vec<[usize; 3]>,
    /// Zone of each triangle (index into [`Self::names`] for ASCII).
    pub zones: Vec<usize>,
    /// Zone names, ASCII only (`solid <name>`; a nameless solid is
    /// `"solid"`). Empty for a binary file, as upstream: `triSurface::readSTL`
    /// sets patch names only for ASCII.
    pub names: Vec<String>,
    /// Number of zones (for binary: distinct attribute values).
    pub n_zones: usize,
    /// `true` if the file was read as binary.
    pub binary: bool,
}

/// `STLCore::startsWithSolid`: the 80-byte header, leading white space
/// skipped, begins with `solid` (any case).
fn starts_with_solid(header: &[u8]) -> bool {
    let n = header.len().min(80);
    let mut pos = 0;
    while pos < n && header[pos].is_ascii_whitespace() {
        pos += 1;
    }
    pos < 75 && header[pos..pos + 5].eq_ignore_ascii_case(b"solid")
}

/// `STLCore::detectBinaryHeader` on an uncompressed buffer: binary when the
/// header does not start with `solid` and the triangle count fits the size
/// (`checkBinaryFileSize`, which allows 25 to 50 bytes per triangle).
fn detect_binary(bytes: &[u8]) -> bool {
    if bytes.len() < 84 || starts_with_solid(&bytes[..80]) {
        return false;
    }
    let n = i32::from_le_bytes([bytes[80], bytes[81], bytes[82], bytes[83]]) as i64;
    let content = bytes.len() as i64 - 80;
    n >= 0 && n >= content / 50 && n <= content / 25
}

/// `word::validate`: drop the characters a `word` may not hold.
fn validate_word(s: &str) -> String {
    s.chars().filter(|&c| !c.is_whitespace() && !matches!(c, '"' | '\'' | '/' | ';' | '{' | '}')).collect()
}

/// Read an STL buffer as OpenFOAM's `triSurface::readSTL` does (see
/// [`StlZoned`]): format by `detectBinaryHeader`, then the ASCII or binary
/// reader, then `mergePointsMap` with OpenFOAM's tolerance (`100 *
/// doubleScalarSMALL` for ASCII, `10 *` for binary, absolute).
///
/// # Errors
/// [`MeshError::DictParse`] on a truncated binary file or a facet with a
/// vertex component that is not a number.
pub fn read_stl_zoned_bytes(bytes: &[u8]) -> Result<StlZoned, MeshError> {
    let binary = detect_binary(bytes);
    let raw = if binary { read_binary_zoned(bytes)? } else { read_ascii_zoned(&String::from_utf8_lossy(bytes))? };
    let tol = if binary { 10.0 } else { 100.0 } * 1.0e-15;
    let (point_map, n_unique) = merge_points(&raw.points, tol);
    let mut points = vec![Vector3::ZERO; n_unique];
    for (i, p) in raw.points.iter().enumerate() {
        points[point_map[i]] = *p;
    }
    let triangles = (0..raw.zones.len()).map(|f| [point_map[3 * f], point_map[3 * f + 1], point_map[3 * f + 2]]).collect();
    Ok(StlZoned { points, triangles, zones: raw.zones, names: raw.names, n_zones: raw.n_zones, binary })
}

/// Read an STL file with [`read_stl_zoned_bytes`].
///
/// # Errors
/// As [`read_stl_zoned_bytes`], plus [`MeshError::DictParse`] wrapping an
/// I/O error.
pub fn read_stl_zoned(path: impl AsRef<std::path::Path>) -> Result<StlZoned, MeshError> {
    let bytes = std::fs::read(path.as_ref()).map_err(|e| MeshError::DictParse(format!("cannot read STL file {}: {e}", path.as_ref().display())))?;
    read_stl_zoned_bytes(&bytes)
}

/// The reader's output before `mergePoints`: three corners per facet.
struct RawStl {
    points: Vec<Vector3>,
    zones: Vec<usize>,
    names: Vec<String>,
    n_zones: usize,
}

/// `STLReader::readBINARY`: corners as `f32`, the attribute word as the zone,
/// zones numbered in order of first appearance.
fn read_binary_zoned(bytes: &[u8]) -> Result<RawStl, MeshError> {
    let count = i32::from_le_bytes([bytes[80], bytes[81], bytes[82], bytes[83]]).max(0) as usize;
    if bytes.len() < 84 + 50 * count {
        return Err(MeshError::DictParse(format!("binary STL truncated: {count} facets need {} bytes, have {}", 84 + 50 * count, bytes.len())));
    }
    let rf = |off: usize| f32::from_le_bytes([bytes[off], bytes[off + 1], bytes[off + 2], bytes[off + 3]]) as f64;
    let mut points = Vec::with_capacity(3 * count);
    let mut zones = Vec::with_capacity(count);
    let mut lookup: std::collections::BTreeMap<u16, usize> = std::collections::BTreeMap::new();
    for i in 0..count {
        let base = 84 + 50 * i + 12;
        for v in 0..3 {
            let o = base + 12 * v;
            points.push(Vector3::new(rf(o), rf(o + 4), rf(o + 8)));
        }
        let attrib = u16::from_le_bytes([bytes[base + 36], bytes[base + 37]]);
        let next = lookup.len();
        zones.push(*lookup.entry(attrib).or_insert(next));
    }
    let n_zones = lookup.len();
    Ok(RawStl { points, zones, names: Vec::new(), n_zones })
}

/// The ASCII parser's actions (`beginSolid`, `beginFacet`,
/// `addVertexComponent`, `endFacet`) driven by each line's leading keyword.
/// A component goes through `double` and then `float`, as
/// `addVertexComponent(::atof(..))` does.
fn read_ascii_zoned(text: &str) -> Result<RawStl, MeshError> {
    let mut points: Vec<Vector3> = Vec::new();
    let mut zones: Vec<usize> = Vec::new();
    let mut names: Vec<String> = Vec::new();
    let mut group = 0usize;
    let mut n_facet_points = 0usize;
    for line in text.lines() {
        let mut tok = line.split_whitespace();
        let Some(cmd) = tok.next() else { continue };
        if cmd.eq_ignore_ascii_case("solid") {
            let name = tok.next().map(validate_word).filter(|s| !s.is_empty()).unwrap_or_else(|| "solid".to_string());
            group = match names.iter().position(|n| *n == name) {
                Some(g) => g,
                None => {
                    names.push(name);
                    names.len() - 1
                }
            };
        } else if cmd.eq_ignore_ascii_case("facet") {
            n_facet_points = 0;
        } else if cmd.eq_ignore_ascii_case("vertex") {
            let mut c = [0.0f64; 3];
            for (k, slot) in c.iter_mut().enumerate() {
                let s = tok.next().ok_or_else(|| MeshError::DictParse(format!("vertex with {k} components: {line:?}")))?;
                let v: f64 = s.parse().map_err(|_| MeshError::DictParse(format!("bad float in STL: {s:?}")))?;
                *slot = v as f32 as f64;
            }
            points.push(Vector3::new(c[0], c[1], c[2]));
            n_facet_points += 1;
        } else if cmd.eq_ignore_ascii_case("endfacet") {
            if n_facet_points == 3 {
                if names.is_empty() {
                    names.push("solid".to_string());
                }
                zones.push(group);
            } else {
                let extra = if n_facet_points > 3 { n_facet_points - 3 } else { n_facet_points };
                points.truncate(points.len() - extra);
            }
            n_facet_points = 0;
        }
    }
    let n_zones = names.len();
    Ok(RawStl { points, zones, names, n_zones })
}

/// OpenFOAM's `mergePoints(points, mergeTol, verbose, pointToUnique)`
/// (`Detail::mergePoints`): a point within `merge_tol` (absolute) of an
/// earlier point in distance-from-the-minimum order merges with it. Returns
/// the map to the unique points and their number; unique points are numbered
/// in order of their lowest-index member, as upstream's renumbering does.
pub fn merge_points(points: &[Vector3], merge_tol: f64) -> (Vec<usize>, usize) {
    let n = points.len();
    if n == 0 {
        return (Vec::new(), 0);
    }
    let mut cmp = points[0];
    for p in &points[1..] {
        cmp = Vector3::new(cmp.x.min(p.x), cmp.y.min(p.y), cmp.z.min(p.z));
    }
    let sq = |a: f64| a * a;
    let tol_sqr = sq(merge_tol);
    let sqr_dist: Vec<f64> = points.iter().map(|p| sq(p.x - cmp.x) + sq(p.y - cmp.y) + sq(p.z - cmp.z)).collect();
    let mut order: Vec<usize> = (0..n).collect();
    // `sortedOrder` is a stable sort
    order.sort_by(|&a, &b| sqr_dist[a].total_cmp(&sqr_dist[b]));
    let sorted_tol: Vec<f64> = order
        .iter()
        .map(|&i| {
            let p = points[i];
            2.0 * merge_tol * ((p.x - cmp.x).abs() + (p.y - cmp.y).abs() + (p.z - cmp.z).abs())
        })
        .collect();
    let mut cluster = vec![0usize; n];
    let mut n_new = 0usize;
    for sorti in 0..n {
        let pi = order[sorti];
        let cur = sqr_dist[pi];
        let cp = points[pi];
        let mut matched = None;
        let mut prev = sorti;
        while prev > 0 {
            prev -= 1;
            if (sqr_dist[order[prev]] - cur).abs() > sorted_tol[sorti] {
                break;
            }
            let q = points[order[prev]];
            if sq(cp.x - q.x) + sq(cp.y - q.y) + sq(cp.z - q.z) <= tol_sqr {
                matched = Some(cluster[order[prev]]);
                break;
            }
        }
        cluster[pi] = match matched {
            Some(c) => c,
            None => {
                n_new += 1;
                n_new - 1
            }
        };
    }
    let mut id_of_cluster = vec![usize::MAX; n_new];
    let mut map = vec![0usize; n];
    let mut next = 0usize;
    for i in 0..n {
        let c = cluster[i];
        if id_of_cluster[c] == usize::MAX {
            id_of_cluster[c] = next;
            next += 1;
        }
        map[i] = id_of_cluster[c];
    }
    (map, next)
}

#[cfg(test)]
mod zoned_tests {
    use super::*;

    const TWO_SOLIDS: &str = "solid inlet_S1\n facet normal 0 0 1\n  outer loop\n   vertex 0 0 0\n   vertex 1 0 0\n   vertex 0 1 0\n  endloop\n endfacet\nendsolid inlet_S1\nsolid walls\n facet normal 0 0 1\n  outer loop\n   vertex 1 0 0\n   vertex 1 1 0\n   vertex 0 1 0\n  endloop\n endfacet\nendsolid walls\nsolid inlet_S1\n facet normal 0 0 1\n  outer loop\n   vertex 0.1 0 0\n   vertex 1 0 0\n   vertex 0 1 0\n  endloop\n endfacet\nendsolid\n";

    /// Multi-solid ASCII: every solid is read, a repeated name reuses its
    /// zone (`nameLookup_`), corners are merged in first-appearance order,
    /// and 0.1 is stored as the nearest `f32`.
    #[test]
    fn ascii_multi_solid_zones_and_merged_points() {
        let s = read_stl_zoned_bytes(TWO_SOLIDS.as_bytes()).unwrap();
        assert!(!s.binary);
        assert_eq!(s.names, vec!["inlet_S1".to_string(), "walls".to_string()]);
        assert_eq!(s.zones, vec![0, 1, 0]);
        assert_eq!(s.points.len(), 5);
        assert_eq!(s.triangles, vec![[0, 1, 2], [1, 3, 2], [4, 1, 2]]);
        assert_eq!(s.points[4].x, 0.1f32 as f64);
        assert_ne!(s.points[4].x, 0.1);
    }

    /// gh:#1465: `read_stl_ascii_str` reads every `solid` of a multi-solid file
    /// (it stopped at the first `endsolid`, dropping the other regions of
    /// OpenFOAM's four-region `flange.stl`), and `from_zoned` agrees on the
    /// triangle count and carries the single-precision corners.
    #[test]
    fn plain_reader_reads_all_solids_and_matches_zoned() {
        let plain = read_stl_ascii_str(TWO_SOLIDS).unwrap();
        assert_eq!(plain.len(), 3);
        let zoned = TriangleSoup::from_zoned(&read_stl_zoned_bytes(TWO_SOLIDS.as_bytes()).unwrap());
        assert_eq!(zoned.len(), 3);
        assert_eq!(plain.triangles[2].a.x, 0.1);
        assert_eq!(zoned.triangles[2].a.x, 0.1f32 as f64);
        assert_eq!(zoned.name, "inlet_S1");
    }

    /// Binary: zones from the attribute word, numbered by first appearance;
    /// no names.
    #[test]
    fn binary_attribute_zones() {
        let mut b = vec![0u8; 80];
        b.extend_from_slice(&3i32.to_le_bytes());
        let tri = |b: &mut Vec<u8>, o: f32, attr: u16| {
            b.extend_from_slice(&[0u8; 12]);
            for v in [[o, 0.0, 0.0], [1.0, 0.0, 0.0], [0.0, 1.0, 0.0]] {
                for c in v {
                    b.extend_from_slice(&c.to_le_bytes());
                }
            }
            b.extend_from_slice(&attr.to_le_bytes());
        };
        tri(&mut b, 0.0, 7);
        tri(&mut b, 0.5, 2);
        tri(&mut b, 0.0, 7);
        let s = read_stl_zoned_bytes(&b).unwrap();
        assert!(s.binary);
        assert_eq!(s.zones, vec![0, 1, 0]);
        assert_eq!(s.n_zones, 2);
        assert!(s.names.is_empty());
        assert_eq!(s.points.len(), 4);
        assert_eq!(s.triangles[2], s.triangles[0]);
    }

    /// `mergePoints`: points within the tolerance merge onto the lowest
    /// index; points farther apart do not.
    #[test]
    fn merge_points_tolerance() {
        let p = vec![Vector3::new(1.0, 0.0, 0.0), Vector3::new(0.0, 0.0, 0.0), Vector3::new(1.0 + 1e-16, 0.0, 0.0), Vector3::new(1.0 + 1e-6, 0.0, 0.0)];
        let (m, n) = merge_points(&p, 1e-13);
        assert_eq!(n, 3);
        assert_eq!(m, vec![0, 1, 0, 2]);
    }
}
