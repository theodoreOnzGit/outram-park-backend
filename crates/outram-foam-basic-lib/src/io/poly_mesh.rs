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

//! `constant/polyMesh` read/write.
//!
//! The crate's [`FvMesh`] stores only flat finite-volume geometry
//! (cell/face centres, areas, owner/neighbour) with no point/face-vertex
//! connectivity. OpenFOAM's on-disk `polyMesh`, by contrast, is defined by its
//! **connectivity**: `points` (vertices), `faces` (vertex loops),
//! `owner`/`neighbour` (cell adjacency), and `boundary` (patches). This module
//! therefore defines a connectivity-carrying [`PolyMesh`] as the I/O
//! representation and computes full FV geometry from it via
//! [`PolyMesh::to_fv_mesh`] — the same divergence-theorem pyramid
//! decomposition OpenFOAM's `primitiveMesh` uses (mirrored from
//! `outram-foam-mesh`'s `block_mesh` / `poly_dual_mesh`).
//!
//! ## Files
//!
//! | file | class | contents |
//! |---|---|---|
//! | `points`    | `vectorField`      | vertex coordinates `[m]` |
//! | `faces`     | `faceList`         | each face as a vertex-index loop |
//! | `owner`     | `labelList`        | owner cell per face |
//! | `neighbour` | `labelList`        | neighbour cell per internal face |
//! | `boundary`  | `polyBoundaryMesh` | patches (`type`, `nFaces`, `startFace`) |
//!
//! Faces are ordered OpenFOAM-style: internal faces first
//! (`[0, n_internal_faces)`), then boundary faces grouped by patch.

use std::path::Path;

use super::dict::{
    fmt_scalar, tokenize, write_header, Token, BANNER, FOOTER, SEPARATOR,
};
use super::IoError;
use crate::mesh::{BoundaryPatch, FvMesh, FvMeshBuilder, PatchKind};
use crate::primitives::Vector3;

/// A single mesh face: its point-index loop plus owner / neighbour cells.
///
/// `verts` is wound so the face normal points **from `owner` toward
/// `neighbour`** (outward from the owner cell). Boundary faces have
/// `neighbour == None`.
#[derive(Debug, Clone, PartialEq)]
pub struct MeshFace {
    /// Ordered point indices (into [`PolyMesh::points`]) forming the face loop.
    pub verts: Vec<usize>,
    /// Owning cell index.
    pub owner: usize,
    /// Neighbour cell index (internal faces only).
    pub neighbour: Option<usize>,
}

/// A connectivity-carrying poly-mesh — the on-disk `polyMesh` representation.
///
/// Faces are ordered internal-first (`[0, n_internal_faces)`), then boundary
/// faces grouped by patch, matching OpenFOAM. Call [`PolyMesh::to_fv_mesh`]
/// to obtain the geometry-carrying [`FvMesh`].
#[derive(Debug, Clone, PartialEq)]
pub struct PolyMesh {
    /// Mesh points `[m]`.
    pub points: Vec<Vector3>,
    /// All faces, internal first then boundary.
    pub faces: Vec<MeshFace>,
    /// Number of internal faces (leading internal entries in `faces`).
    pub n_internal_faces: usize,
    /// Number of cells.
    pub n_cells: usize,
    /// Boundary patches, covering `[n_internal_faces, faces.len())`.
    pub patches: Vec<BoundaryPatch>,
}

impl PolyMesh {
    /// Number of points.
    pub fn n_points(&self) -> usize {
        self.points.len()
    }
    /// Total number of faces (internal + boundary).
    pub fn n_faces(&self) -> usize {
        self.faces.len()
    }
    /// Number of boundary faces.
    pub fn n_boundary_faces(&self) -> usize {
        self.faces.len() - self.n_internal_faces
    }

    /// Total mesh volume `[m^3]` — the sum of all cell volumes.
    pub fn total_volume(&self) -> f64 {
        self.cell_geometry().0.iter().sum()
    }

    /// Compute per-cell `(volumes, centres)` `[m^3]`, `[m]` via the OpenFOAM
    /// pyramid decomposition (`primitiveMeshCellCentresAndVols`).
    fn cell_geometry(&self) -> (Vec<f64>, Vec<Vector3>) {
        let mut face_ctr = Vec::with_capacity(self.faces.len());
        let mut face_sf = Vec::with_capacity(self.faces.len());
        for f in &self.faces {
            let (c, sf) = face_centre_and_area(&self.points, &f.verts);
            face_ctr.push(c);
            face_sf.push(sf);
        }

        // First pass: estimate each cell centre as the mean of its face centres.
        let mut c_est = vec![Vector3::ZERO; self.n_cells];
        let mut n_cell_faces = vec![0.0f64; self.n_cells];
        for (fi, f) in self.faces.iter().enumerate() {
            c_est[f.owner] += face_ctr[fi];
            n_cell_faces[f.owner] += 1.0;
            if let Some(nb) = f.neighbour {
                c_est[nb] += face_ctr[fi];
                n_cell_faces[nb] += 1.0;
            }
        }
        for c in 0..self.n_cells {
            if n_cell_faces[c] > 0.0 {
                c_est[c] /= n_cell_faces[c];
            }
        }

        // Second pass: sum pyramid volumes / centroids about the estimate.
        let mut vol = vec![0.0f64; self.n_cells];
        let mut ctr = vec![Vector3::ZERO; self.n_cells];
        for (fi, f) in self.faces.iter().enumerate() {
            let cf = face_ctr[fi];
            {
                let cell = f.owner;
                let sf = face_sf[fi];
                let pyr3 = sf.dot(cf - c_est[cell]);
                let pyr_ctr = cf * 0.75 + c_est[cell] * 0.25;
                vol[cell] += pyr3;
                ctr[cell] += pyr_ctr * pyr3;
            }
            if let Some(cell) = f.neighbour {
                let sf = face_sf[fi] * -1.0;
                let pyr3 = sf.dot(cf - c_est[cell]);
                let pyr_ctr = cf * 0.75 + c_est[cell] * 0.25;
                vol[cell] += pyr3;
                ctr[cell] += pyr_ctr * pyr3;
            }
        }
        for c in 0..self.n_cells {
            if vol[c].abs() > f64::EPSILON {
                ctr[c] /= vol[c];
            } else {
                ctr[c] = c_est[c];
            }
            vol[c] /= 3.0;
        }
        (vol, ctr)
    }

    /// Convert to the geometry-carrying [`FvMesh`], computing cell
    /// volumes/centres, face-area vectors, face areas, and face centres from
    /// the point/face topology.
    ///
    /// # Errors
    /// [`IoError::Mesh`] if the assembled mesh fails `FvMesh::validate`.
    pub fn to_fv_mesh(&self) -> Result<FvMesh, IoError> {
        let (cell_volumes, cell_centres) = self.cell_geometry();

        let mut owner = Vec::with_capacity(self.faces.len());
        let mut neighbour = Vec::with_capacity(self.n_internal_faces);
        let mut face_area_vectors = Vec::with_capacity(self.faces.len());
        let mut face_centres = Vec::with_capacity(self.faces.len());
        for f in &self.faces {
            let (c, sf) = face_centre_and_area(&self.points, &f.verts);
            owner.push(f.owner);
            if let Some(nb) = f.neighbour {
                neighbour.push(nb);
            }
            face_area_vectors.push(sf);
            face_centres.push(c);
        }

        FvMeshBuilder::new()
            .n_cells(self.n_cells)
            .n_internal_faces(self.n_internal_faces)
            .owner(owner)
            .neighbour(neighbour)
            .patches(self.patches.clone())
            .cell_volumes(cell_volumes)
            .cell_centres(cell_centres)
            .face_area_vectors(face_area_vectors)
            .face_centres(face_centres)
            .build()
            .map_err(|e| IoError::Mesh(e.to_string()))
    }

    // ── Reading ─────────────────────────────────────────────────────────────

    /// Read a `polyMesh` from a `constant/polyMesh` directory.
    ///
    /// Reads `points`, `faces`, `owner`, `neighbour`, and `boundary`.
    pub fn read(dir: impl AsRef<Path>) -> Result<Self, IoError> {
        let dir = dir.as_ref();
        let points = read_points(&dir.join("points"))?;
        let face_loops = read_faces(&dir.join("faces"))?;
        let owner = read_labels(&dir.join("owner"))?;
        let neighbour = read_labels(&dir.join("neighbour"))?;
        let raw_patches = read_boundary(&dir.join("boundary"))?;

        if owner.len() != face_loops.len() {
            return Err(IoError::parse(
                "polyMesh",
                format!(
                    "owner has {} entries but faces has {}",
                    owner.len(),
                    face_loops.len()
                ),
            ));
        }
        let n_internal_faces = neighbour.len();
        let mut faces = Vec::with_capacity(face_loops.len());
        for (fi, verts) in face_loops.into_iter().enumerate() {
            let nb = if fi < n_internal_faces {
                Some(neighbour[fi])
            } else {
                None
            };
            faces.push(MeshFace {
                verts,
                owner: owner[fi],
                neighbour: nb,
            });
        }

        // Cell count = highest referenced cell index + 1.
        let mut n_cells = 0usize;
        for f in &faces {
            n_cells = n_cells.max(f.owner + 1);
            if let Some(nb) = f.neighbour {
                n_cells = n_cells.max(nb + 1);
            }
        }

        let patches = raw_patches
            .into_iter()
            .map(|p| BoundaryPatch::new(p.name, p.start_face, p.n_faces, kind_from_str(&p.kind)))
            .collect();

        Ok(Self {
            points,
            faces,
            n_internal_faces,
            n_cells,
            patches,
        })
    }

    // ── Writing ─────────────────────────────────────────────────────────────

    /// Write the `polyMesh` files into `dir`, creating it if necessary.
    pub fn write(&self, dir: impl AsRef<Path>) -> Result<(), IoError> {
        let dir = dir.as_ref();
        std::fs::create_dir_all(dir).map_err(|e| IoError::io(dir.display().to_string(), e))?;
        write_file(&dir.join("points"), &self.points_string())?;
        write_file(&dir.join("faces"), &self.faces_string())?;
        write_file(&dir.join("owner"), &self.owner_string())?;
        write_file(&dir.join("neighbour"), &self.neighbour_string())?;
        write_file(&dir.join("boundary"), &self.boundary_string())?;
        Ok(())
    }

    fn points_string(&self) -> String {
        let mut s = header_str("vectorField", "points");
        s.push_str(&format!("{}\n(\n", self.points.len()));
        for p in &self.points {
            s.push_str(&format!(
                "({} {} {})\n",
                fmt_scalar(p.x),
                fmt_scalar(p.y),
                fmt_scalar(p.z)
            ));
        }
        s.push_str(")\n\n");
        s.push_str(FOOTER);
        s
    }

    fn faces_string(&self) -> String {
        let mut s = header_str("faceList", "faces");
        s.push_str(&format!("{}\n(\n", self.faces.len()));
        for f in &self.faces {
            s.push_str(&format!("{}(", f.verts.len()));
            for (i, v) in f.verts.iter().enumerate() {
                if i > 0 {
                    s.push(' ');
                }
                s.push_str(&v.to_string());
            }
            s.push_str(")\n");
        }
        s.push_str(")\n\n");
        s.push_str(FOOTER);
        s
    }

    fn owner_string(&self) -> String {
        let mut s = header_str("labelList", "owner");
        s.push_str(&format!("{}\n(\n", self.faces.len()));
        for f in &self.faces {
            s.push_str(&f.owner.to_string());
            s.push('\n');
        }
        s.push_str(")\n\n");
        s.push_str(FOOTER);
        s
    }

    fn neighbour_string(&self) -> String {
        let mut s = header_str("labelList", "neighbour");
        s.push_str(&format!("{}\n(\n", self.n_internal_faces));
        for f in self.faces.iter().take(self.n_internal_faces) {
            s.push_str(&f.neighbour.unwrap_or(0).to_string());
            s.push('\n');
        }
        s.push_str(")\n\n");
        s.push_str(FOOTER);
        s
    }

    fn boundary_string(&self) -> String {
        let mut s = header_str("polyBoundaryMesh", "boundary");
        s.push_str(&format!("{}\n(\n", self.patches.len()));
        for p in &self.patches {
            s.push_str(&format!("    {}\n    {{\n", p.name));
            s.push_str(&format!("        type            {};\n", kind_to_str(p.kind)));
            s.push_str(&format!("        nFaces          {};\n", p.size));
            s.push_str(&format!("        startFace       {};\n", p.start));
            s.push_str("    }\n");
        }
        s.push_str(")\n\n");
        s.push_str(FOOTER);
        s
    }
}

// ── Geometry (mirrors outram-foam-mesh::block_mesh) ─────────────────────────

/// Face centre `[m]` and area vector `Sf` `[m^2]` from a vertex loop, using the
/// OpenFOAM decomposition of the polygon into triangles about its centroid.
fn face_centre_and_area(points: &[Vector3], verts: &[usize]) -> (Vector3, Vector3) {
    let n = verts.len();
    debug_assert!(n >= 3);
    if n == 3 {
        let a = points[verts[0]];
        let b = points[verts[1]];
        let c = points[verts[2]];
        let centre = (a + b + c) / 3.0;
        let area = (b - a).cross(c - a) * 0.5;
        return (centre, area);
    }
    let mut c_est = Vector3::ZERO;
    for &v in verts {
        c_est += points[v];
    }
    c_est /= n as f64;

    let mut sum_a = 0.0;
    let mut sum_n = Vector3::ZERO;
    let mut sum_ac = Vector3::ZERO;
    for i in 0..n {
        let p1 = points[verts[i]];
        let p2 = points[verts[(i + 1) % n]];
        let mid = (p1 + p2 + c_est) / 3.0;
        let n_tri = (p2 - p1).cross(c_est - p1);
        let a_tri = n_tri.mag();
        sum_a += a_tri;
        sum_n += n_tri;
        sum_ac += mid * a_tri;
    }
    let centre = if sum_a > f64::EPSILON {
        sum_ac / sum_a
    } else {
        c_est
    };
    (centre, sum_n * 0.5)
}

// ── PatchKind <-> OpenFOAM type strings ─────────────────────────────────────

fn kind_from_str(s: &str) -> PatchKind {
    match s {
        "wall" => PatchKind::Wall,
        "symmetry" | "symmetryPlane" => PatchKind::Symmetry,
        "empty" => PatchKind::Empty,
        "wedge" => PatchKind::Wedge,
        "cyclic" => PatchKind::Cyclic,
        "cyclicAMI" => PatchKind::CyclicAmi,
        "processor" => PatchKind::Processor,
        _ => PatchKind::Patch,
    }
}

fn kind_to_str(k: PatchKind) -> &'static str {
    match k {
        PatchKind::Patch => "patch",
        PatchKind::Wall => "wall",
        PatchKind::Symmetry => "symmetry",
        PatchKind::Empty => "empty",
        PatchKind::Wedge => "wedge",
        PatchKind::Cyclic => "cyclic",
        PatchKind::CyclicAmi => "cyclicAMI",
        PatchKind::Processor => "processor",
    }
}

// ── Low-level token cursor over a polyMesh list file ────────────────────────

/// Owns its tokens (no lifetime parameters, per crate design rules).
struct ListCursor {
    tokens: Vec<Token>,
    pos: usize,
    context: String,
}

impl ListCursor {
    fn new(text: &str, context: impl Into<String>) -> Self {
        let mut c = Self {
            tokens: tokenize(text),
            pos: 0,
            context: context.into(),
        };
        c.skip_header();
        c
    }

    fn err(&self, m: impl Into<String>) -> IoError {
        IoError::parse(self.context.clone(), m.into())
    }

    fn peek(&self) -> Option<&str> {
        self.tokens.get(self.pos).map(|t| t.text.as_str())
    }

    fn next(&mut self) -> Option<String> {
        let t = self.tokens.get(self.pos).map(|t| t.text.clone());
        if t.is_some() {
            self.pos += 1;
        }
        t
    }

    fn expect(&mut self, tok: &str) -> Result<(), IoError> {
        match self.next() {
            Some(t) if t == tok => Ok(()),
            other => Err(self.err(format!("expected `{tok}`, found {other:?}"))),
        }
    }

    /// Skip a leading `FoamFile { … }` header block if present.
    fn skip_header(&mut self) {
        if self.peek() == Some("FoamFile") {
            self.next(); // FoamFile
            // consume up to and including the matching '}'
            let mut depth = 0usize;
            while let Some(t) = self.next() {
                match t.as_str() {
                    "{" => depth += 1,
                    "}" => {
                        depth -= 1;
                        if depth == 0 {
                            break;
                        }
                    }
                    _ => {}
                }
            }
        }
    }

    fn parse_usize(&mut self) -> Result<usize, IoError> {
        let t = self
            .next()
            .ok_or_else(|| self.err("expected an integer, found end of input"))?;
        t.parse::<usize>()
            .map_err(|_| self.err(format!("expected an integer, found `{t}`")))
    }

    fn parse_f64(&mut self) -> Result<f64, IoError> {
        let t = self
            .next()
            .ok_or_else(|| self.err("expected a number, found end of input"))?;
        t.parse::<f64>()
            .map_err(|_| self.err(format!("expected a number, found `{t}`")))
    }
}

fn read_points(path: &Path) -> Result<Vec<Vector3>, IoError> {
    let text = read_to_string(path)?;
    let mut c = ListCursor::new(&text, path.display().to_string());
    let count = c.parse_usize()?;
    c.expect("(")?;
    let mut pts = Vec::with_capacity(count);
    for _ in 0..count {
        c.expect("(")?;
        let x = c.parse_f64()?;
        let y = c.parse_f64()?;
        let z = c.parse_f64()?;
        c.expect(")")?;
        pts.push(Vector3::new(x, y, z));
    }
    c.expect(")")?;
    Ok(pts)
}

fn read_faces(path: &Path) -> Result<Vec<Vec<usize>>, IoError> {
    let text = read_to_string(path)?;
    let mut c = ListCursor::new(&text, path.display().to_string());
    let count = c.parse_usize()?;
    c.expect("(")?;
    let mut faces = Vec::with_capacity(count);
    for _ in 0..count {
        let nverts = c.parse_usize()?;
        c.expect("(")?;
        let mut verts = Vec::with_capacity(nverts);
        for _ in 0..nverts {
            verts.push(c.parse_usize()?);
        }
        c.expect(")")?;
        faces.push(verts);
    }
    c.expect(")")?;
    Ok(faces)
}

fn read_labels(path: &Path) -> Result<Vec<usize>, IoError> {
    let text = read_to_string(path)?;
    let mut c = ListCursor::new(&text, path.display().to_string());
    let count = c.parse_usize()?;
    c.expect("(")?;
    let mut out = Vec::with_capacity(count);
    for _ in 0..count {
        out.push(c.parse_usize()?);
    }
    c.expect(")")?;
    Ok(out)
}

struct RawPatch {
    name: String,
    kind: String,
    n_faces: usize,
    start_face: usize,
}

fn read_boundary(path: &Path) -> Result<Vec<RawPatch>, IoError> {
    let text = read_to_string(path)?;
    let mut c = ListCursor::new(&text, path.display().to_string());
    let count = c.parse_usize()?;
    c.expect("(")?;
    let mut patches = Vec::with_capacity(count);
    for _ in 0..count {
        let name = c
            .next()
            .ok_or_else(|| c.err("expected a patch name"))?
            .to_string();
        c.expect("{")?;
        let mut kind = String::from("patch");
        let mut n_faces = 0usize;
        let mut start_face = 0usize;
        // Parse `key value ;` entries; recognise type/nFaces/startFace, skip
        // the rest (inGroups, physicalType, …), tolerating nested `( … )`.
        while c.peek() != Some("}") {
            let key = c
                .next()
                .ok_or_else(|| c.err("unexpected end of input in patch entry"))?
                .to_string();
            match key.as_str() {
                "type" => {
                    kind = c
                        .next()
                        .ok_or_else(|| c.err("expected a patch type"))?
                        .to_string();
                    c.expect(";")?;
                }
                "nFaces" => {
                    n_faces = c.parse_usize()?;
                    c.expect(";")?;
                }
                "startFace" => {
                    start_face = c.parse_usize()?;
                    c.expect(";")?;
                }
                _ => {
                    // Skip an arbitrary value up to ';', balancing parens.
                    let mut depth = 0usize;
                    loop {
                        match c.next().as_deref() {
                            Some("(") => depth += 1,
                            Some(")") => depth = depth.saturating_sub(1),
                            Some(";") if depth == 0 => break,
                            Some(_) => {}
                            None => return Err(c.err("unterminated patch entry")),
                        }
                    }
                }
            }
        }
        c.expect("}")?;
        patches.push(RawPatch {
            name,
            kind,
            n_faces,
            start_face,
        });
    }
    c.expect(")")?;
    Ok(patches)
}

// ── small file helpers ──────────────────────────────────────────────────────

fn read_to_string(path: &Path) -> Result<String, IoError> {
    std::fs::read_to_string(path).map_err(|e| IoError::io(path.display().to_string(), e))
}

fn write_file(path: &Path, contents: &str) -> Result<(), IoError> {
    std::fs::write(path, contents).map_err(|e| IoError::io(path.display().to_string(), e))
}

/// Build a `constant/polyMesh` file header string (banner + FoamFile + separator).
fn header_str(class: &str, object: &str) -> String {
    let mut s = String::new();
    s.push_str(BANNER);
    let header = super::dict::FoamHeader::standard_with_location(class, "constant/polyMesh", object);
    write_header(&mut s, &header);
    s.push_str(SEPARATOR);
    s.push_str("\n\n");
    s
}
