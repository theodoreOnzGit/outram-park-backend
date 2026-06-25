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

//! Reader for OpenFOAM `constant/polyMesh/` directories.
//!
//! ## Why a custom parser, not the OpenFOAM C++ reader?
//!
//! OpenFOAM's own file reader (`ISstream`, `IOobject`, `IOdictionary`) is a
//! deeply-templated C++ library with a runtime type registry.  There is no
//! stable C API to wrap with bindgen, and the template depth makes even
//! generating Rust FFI shims impractical.  No mature Rust crate exists for
//! OpenFOAM ASCII format either.  The ASCII format itself is straightforward
//! enough (FoamFile header + N-element list) that a purpose-built 400-line
//! Rust parser is far simpler than attempting C++ interop.

use std::path::Path;
use std::sync::Arc;
use openfoam_basic_lib::prelude::{
    BoundaryPatch, FvMesh, FvMeshBuilder, PatchKind, Vector3,
};
use crate::error::AppBuilderError;

// ── public entry point ────────────────────────────────────────────────────────

/// Read an OpenFOAM `constant/polyMesh/` directory and return an `FvMesh`.
///
/// Reads `points`, `faces`, `owner`, `neighbour`, and `boundary`, then
/// derives all geometric quantities (face centres, face area vectors, cell
/// centres, cell volumes) via pyramid decomposition — the same algorithm used
/// by `primitiveMesh::makeFaceCentresAndAreas` and
/// `primitiveMesh::makeCellCentresAndVols` in OpenFOAM's C++ source.
pub fn read_poly_mesh(poly_mesh_dir: &Path) -> Result<Arc<FvMesh>, AppBuilderError> {
    let read = |name: &str| -> Result<String, AppBuilderError> {
        let p = poly_mesh_dir.join(name);
        std::fs::read_to_string(&p).map_err(|e| AppBuilderError::Io { path: p, source: e })
    };

    let points_raw    = read("points")?;
    let faces_raw     = read("faces")?;
    let owner_raw     = read("owner")?;
    let neighbour_raw = read("neighbour")?;
    let boundary_raw  = read("boundary")?;

    let points    = parse_points(&points_raw,    "points")?;
    let faces     = parse_faces(&faces_raw,      "faces")?;
    let owner     = parse_index_list(&owner_raw,     "owner")?;
    let neighbour = parse_index_list(&neighbour_raw, "neighbour")?;
    let patches   = parse_boundary(&boundary_raw,    "boundary")?;

    let n_cells = owner.iter()
        .chain(neighbour.iter())
        .copied()
        .max()
        .map(|m| m + 1)
        .unwrap_or(0);
    let n_internal_faces = neighbour.len();

    let (face_centres, face_area_vectors, face_areas) =
        compute_face_geometry(&points, &faces, "faces")?;

    let (cell_volumes, cell_centres) = compute_cell_geometry(
        &face_centres, &face_area_vectors, &owner, &neighbour, n_cells,
    );

    let mesh = FvMeshBuilder::new()
        .n_cells(n_cells)
        .n_internal_faces(n_internal_faces)
        .owner(owner)
        .neighbour(neighbour)
        .patches(patches)
        .cell_volumes(cell_volumes)
        .cell_centres(cell_centres)
        .face_area_vectors(face_area_vectors)
        .face_areas(face_areas)
        .face_centres(face_centres)
        .build()
        .map_err(|msg| AppBuilderError::Parse {
            file: poly_mesh_dir.display().to_string(),
            line: 0,
            msg,
        })?;

    Ok(Arc::new(mesh))
}

// ── comment stripping ─────────────────────────────────────────────────────────

/// Strip `/* ... */` block comments and `// ...` line comments, preserving
/// newlines so that error line numbers stay meaningful.
pub(crate) fn strip_foam_comments(text: &str) -> String {
    let bytes = text.as_bytes();
    let n = bytes.len();
    let mut out = String::with_capacity(n);
    let mut i = 0;
    while i < n {
        if i + 1 < n && bytes[i] == b'/' && bytes[i + 1] == b'*' {
            // Block comment — skip until `*/`, keep newlines for line numbers.
            i += 2;
            while i + 1 < n && !(bytes[i] == b'*' && bytes[i + 1] == b'/') {
                if bytes[i] == b'\n' { out.push('\n'); }
                i += 1;
            }
            i += 2;
        } else if i + 1 < n && bytes[i] == b'/' && bytes[i + 1] == b'/' {
            // Line comment — skip to end of line.
            while i < n && bytes[i] != b'\n' { i += 1; }
        } else {
            out.push(bytes[i] as char);
            i += 1;
        }
    }
    out
}

/// Return the portion of `stripped` that comes after the first top-level
/// `FoamFile { ... }` dictionary block.  Every OpenFOAM ASCII file has exactly
/// one such header before the payload data.
fn data_section(stripped: &str) -> &str {
    let start = match stripped.find('{') {
        Some(i) => i,
        None    => return stripped,
    };
    let mut depth = 0usize;
    for (off, c) in stripped[start..].char_indices() {
        match c {
            '{' => depth += 1,
            '}' => {
                depth -= 1;
                if depth == 0 { return &stripped[start + off + 1..]; }
            }
            _ => {}
        }
    }
    stripped
}

// ── list extraction ───────────────────────────────────────────────────────────

/// In the data section, locate the leading integer count `N` and the body text
/// inside the surrounding `( ... )` list.
fn extract_list_body<'a>(section: &'a str, file: &str)
    -> Result<(usize, &'a str), AppBuilderError>
{
    let s = section.trim();

    // The count is the first whitespace-delimited token.
    let count_end = s.find(|c: char| c.is_whitespace() || c == '(')
        .unwrap_or(s.len());
    let count: usize = s[..count_end].trim().parse().map_err(|_| AppBuilderError::Parse {
        file: file.to_string(),
        line: 0,
        msg: format!("expected list count, got {:?}", &s[..count_end.min(30)]),
    })?;

    // Find the opening `(`.
    let open = s.find('(').ok_or_else(|| AppBuilderError::Parse {
        file: file.to_string(),
        line: 0,
        msg: "list opening '(' not found".into(),
    })?;

    // Walk to the matching `)` at depth 1.
    let body_start = open + 1;
    let mut depth = 1usize;
    let mut close = body_start;
    for (off, c) in s[body_start..].char_indices() {
        match c {
            '(' => depth += 1,
            ')' => {
                depth -= 1;
                if depth == 0 { close = body_start + off; break; }
            }
            _ => {}
        }
    }

    Ok((count, &s[body_start..close]))
}

// ── per-file parsers ──────────────────────────────────────────────────────────

/// Parse the `points` file.  Each entry is `(x y z)`.
pub fn parse_points(text: &str, file: &str) -> Result<Vec<[f64; 3]>, AppBuilderError> {
    let stripped = strip_foam_comments(text);
    let section  = data_section(&stripped);
    let (count, body) = extract_list_body(section, file)?;
    let mut pts = Vec::with_capacity(count);

    let mut s = body;
    while let Some(open) = s.find('(') {
        let close = s[open..].find(')').ok_or_else(|| AppBuilderError::Parse {
            file: file.to_string(), line: 0,
            msg: "unclosed '(' in points entry".into(),
        })? + open;

        let triple = s[open + 1..close].trim();
        let nums: Vec<f64> = triple.split_whitespace()
            .map(|t| t.parse::<f64>().map_err(|_| AppBuilderError::Parse {
                file: file.to_string(), line: 0,
                msg: format!("bad float in points: {t:?}"),
            }))
            .collect::<Result<_, _>>()?;
        if nums.len() != 3 {
            return Err(AppBuilderError::Parse {
                file: file.to_string(), line: 0,
                msg: format!("point has {} components, expected 3", nums.len()),
            });
        }
        pts.push([nums[0], nums[1], nums[2]]);
        s = &s[close + 1..];
    }

    if pts.len() != count {
        return Err(AppBuilderError::Parse {
            file: file.to_string(), line: 0,
            msg: format!("expected {count} points, parsed {}", pts.len()),
        });
    }
    Ok(pts)
}

/// Parse the `faces` file.  Each entry is `N(v0 v1 … vN-1)`.
///
/// The leading digit(s) before `(` give the vertex count; they are redundant
/// with the actual number of tokens inside `( )` but we verify consistency.
pub fn parse_faces(text: &str, file: &str) -> Result<Vec<Vec<usize>>, AppBuilderError> {
    let stripped = strip_foam_comments(text);
    let section  = data_section(&stripped);
    let (count, body) = extract_list_body(section, file)?;
    let mut faces = Vec::with_capacity(count);

    let mut s = body.trim();
    while !s.is_empty() {
        s = s.trim_start();
        if s.is_empty() { break; }

        // Skip leading digit(s) before the '('.
        let open = s.find('(').ok_or_else(|| AppBuilderError::Parse {
            file: file.to_string(), line: 0,
            msg: "face entry missing '('".into(),
        })?;
        let close = s[open..].find(')').ok_or_else(|| AppBuilderError::Parse {
            file: file.to_string(), line: 0,
            msg: "face entry missing ')'".into(),
        })? + open;

        let verts: Vec<usize> = s[open + 1..close].split_whitespace()
            .map(|t| t.parse::<usize>().map_err(|_| AppBuilderError::Parse {
                file: file.to_string(), line: 0,
                msg: format!("bad vertex index in faces: {t:?}"),
            }))
            .collect::<Result<_, _>>()?;
        faces.push(verts);
        s = &s[close + 1..];
    }

    if faces.len() != count {
        return Err(AppBuilderError::Parse {
            file: file.to_string(), line: 0,
            msg: format!("expected {count} faces, parsed {}", faces.len()),
        });
    }
    Ok(faces)
}

/// Parse the `owner` or `neighbour` file: one non-negative integer per entry.
pub fn parse_index_list(text: &str, file: &str) -> Result<Vec<usize>, AppBuilderError> {
    let stripped = strip_foam_comments(text);
    let section  = data_section(&stripped);
    let (count, body) = extract_list_body(section, file)?;
    let indices: Vec<usize> = body.split_whitespace()
        .map(|t| t.parse::<usize>().map_err(|_| AppBuilderError::Parse {
            file: file.to_string(), line: 0,
            msg: format!("bad index in {file}: {t:?}"),
        }))
        .collect::<Result<_, _>>()?;
    if indices.len() != count {
        return Err(AppBuilderError::Parse {
            file: file.to_string(), line: 0,
            msg: format!("expected {count} entries, parsed {}", indices.len()),
        });
    }
    Ok(indices)
}

/// Parse the `boundary` file into a list of `BoundaryPatch`.
pub fn parse_boundary(text: &str, file: &str) -> Result<Vec<BoundaryPatch>, AppBuilderError> {
    let stripped = strip_foam_comments(text);
    let section  = data_section(&stripped);
    let (count, body) = extract_list_body(section, file)?;
    let mut patches = Vec::with_capacity(count);

    let mut s = body.trim();
    while !s.is_empty() {
        s = s.trim_start();
        if s.is_empty() { break; }

        // Patch name: word before the opening `{`.
        let name_end = s.find(|c: char| c.is_whitespace() || c == '{')
            .unwrap_or(s.len());
        let name = s[..name_end].trim().to_string();
        if name.is_empty() { break; }
        s = s[name_end..].trim_start();

        if !s.starts_with('{') {
            return Err(AppBuilderError::Parse {
                file: file.to_string(), line: 0,
                msg: format!("expected '{{' after patch name {name:?}"),
            });
        }
        let close = find_block_end(s).ok_or_else(|| AppBuilderError::Parse {
            file: file.to_string(), line: 0,
            msg: format!("unclosed '{{' for patch {name:?}"),
        })?;
        let block = &s[1..close];

        let kind       = parse_patch_kind(block);
        let n_faces    = parse_dict_usize(block, "nFaces").unwrap_or(0);
        let start_face = parse_dict_usize(block, "startFace").unwrap_or(0);

        patches.push(BoundaryPatch::new(name, start_face, n_faces, kind));
        s = &s[close + 1..];
    }

    if patches.len() != count {
        return Err(AppBuilderError::Parse {
            file: file.to_string(), line: 0,
            msg: format!("expected {count} patches, parsed {}", patches.len()),
        });
    }
    Ok(patches)
}

// ── boundary helpers ──────────────────────────────────────────────────────────

/// Return the character offset of the `}` that closes the first `{` in `s`.
fn find_block_end(s: &str) -> Option<usize> {
    let mut depth = 0usize;
    for (i, c) in s.char_indices() {
        match c {
            '{' => depth += 1,
            '}' => {
                depth -= 1;
                if depth == 0 { return Some(i); }
            }
            _ => {}
        }
    }
    None
}

/// Extract the `usize` value for a dictionary keyword like `nFaces  20;`.
fn parse_dict_usize(block: &str, key: &str) -> Option<usize> {
    let pos   = block.find(key)?;
    let after = block[pos + key.len()..].trim_start();
    let end   = after.find(|c: char| c.is_whitespace() || c == ';').unwrap_or(after.len());
    after[..end].parse().ok()
}

/// Map the `type` field in a patch dictionary block to a `PatchKind`.
fn parse_patch_kind(block: &str) -> PatchKind {
    if let Some(pos) = block.find("type") {
        let after = block[pos + 4..].trim_start();
        let end   = after.find(|c: char| c.is_whitespace() || c == ';').unwrap_or(after.len());
        return match after[..end].trim() {
            "wall"                        => PatchKind::Wall,
            "symmetry" | "symmetryPlane" => PatchKind::Symmetry,
            "empty"                       => PatchKind::Empty,
            "wedge"                       => PatchKind::Wedge,
            "cyclic"                      => PatchKind::Cyclic,
            "processor"                   => PatchKind::Processor,
            _                             => PatchKind::Patch,
        };
    }
    PatchKind::Patch
}

// ── geometry ──────────────────────────────────────────────────────────────────

/// Compute face centres, face area vectors, and face magnitudes from vertex
/// positions and face connectivity using triangle-fan decomposition.
///
/// Algorithm (mirrors `primitiveMesh::makeFaceCentresAndAreas`):
///   1. Estimate face centre as the average of its vertex positions.
///   2. Fan-triangulate from that estimate: for each consecutive edge (vᵢ, vᵢ₊₁),
///      compute the triangle area vector `0.5 · (vᵢ−fc) × (vᵢ₊₁−fc)` and
///      the triangle centroid `(fc + vᵢ + vᵢ₊₁) / 3`.
///   3. Sum → face area vector Sf; weighted sum → corrected face centre.
pub(crate) fn compute_face_geometry(
    points: &[[f64; 3]],
    faces:  &[Vec<usize>],
    file:   &str,
) -> Result<(Vec<Vector3>, Vec<Vector3>, Vec<f64>), AppBuilderError> {
    let n_faces = faces.len();
    let mut face_centres      = Vec::with_capacity(n_faces);
    let mut face_area_vectors = Vec::with_capacity(n_faces);
    let mut face_areas        = Vec::with_capacity(n_faces);

    for (fi, verts) in faces.iter().enumerate() {
        if verts.len() < 3 {
            return Err(AppBuilderError::Parse {
                file: file.to_string(), line: fi,
                msg: format!("face {fi} has {} vertices, need ≥ 3", verts.len()),
            });
        }

        // Step 1: average of vertices as initial face-centre estimate.
        let mut fc_est = [0.0f64; 3];
        for &vi in verts {
            let p = points[vi];
            fc_est[0] += p[0]; fc_est[1] += p[1]; fc_est[2] += p[2];
        }
        let inv_n = 1.0 / verts.len() as f64;
        let fc_est = Vector3::new(fc_est[0] * inv_n, fc_est[1] * inv_n, fc_est[2] * inv_n);

        // Step 2: triangle fan.
        let mut sf         = Vector3::ZERO;
        let mut centre_num = Vector3::ZERO;
        let mut area_sum   = 0.0f64;
        let nv = verts.len();

        for i in 0..nv {
            let a = pt(points, verts[i]);
            let b = pt(points, verts[(i + 1) % nv]);
            let tri_sf     = (a - fc_est).cross(b - fc_est) * 0.5;
            let tri_area   = tri_sf.mag();
            let tri_centre = (fc_est + a + b) * (1.0 / 3.0);
            sf         = sf + tri_sf;
            centre_num = centre_num + tri_centre * tri_area;
            area_sum  += tri_area;
        }

        // Step 3: corrected face centre.
        let face_centre = if area_sum > f64::MIN_POSITIVE { centre_num * (1.0 / area_sum) }
                          else { fc_est };

        face_centres.push(face_centre);
        face_area_vectors.push(sf);
        face_areas.push(sf.mag());
    }

    Ok((face_centres, face_area_vectors, face_areas))
}

/// Compute cell volumes and cell centres via pyramid decomposition.
///
/// Algorithm (mirrors `primitiveMesh::makeCellCentresAndVols`):
///   For each cell C with bounding faces fᵢ:
///   1. Estimate cell centre as the average of the bounding face centres.
///   2. For each face, form a pyramid with apex at the estimate:
///      - signed volume = (1/3) · sign · Sf · (fc − cc_est)
///        (sign = +1 if cell is owner, −1 if neighbour — Sf points owner→neighbour)
///      - pyramid centroid = (3/4) · fc + (1/4) · cc_est
///   3. Sum signed volumes → cell volume; weighted centroid sum → cell centre.
pub(crate) fn compute_cell_geometry(
    face_centres:      &[Vector3],
    face_area_vectors: &[Vector3],
    owner:             &[usize],
    neighbour:         &[usize],
    n_cells:           usize,
) -> (Vec<f64>, Vec<Vector3>) {
    // Build cell→face adjacency.
    let mut cell_faces: Vec<Vec<(usize, bool)>> = vec![Vec::new(); n_cells];
    for (f, &o) in owner.iter().enumerate() {
        cell_faces[o].push((f, true));
    }
    for (f, &nb) in neighbour.iter().enumerate() {
        cell_faces[nb].push((f, false));
    }

    let mut cell_volumes = vec![0.0f64; n_cells];
    let mut cell_centres = vec![Vector3::ZERO; n_cells];

    for c in 0..n_cells {
        let faces = &cell_faces[c];
        if faces.is_empty() { continue; }

        // Step 1: estimated centre.
        let mut cc_est = Vector3::ZERO;
        for &(f, _) in faces { cc_est = cc_est + face_centres[f]; }
        cc_est = cc_est * (1.0 / faces.len() as f64);

        // Step 2: pyramid summation.
        let mut vol        = 0.0f64;
        let mut centre_num = Vector3::ZERO;

        for &(f, is_owner) in faces {
            let sf  = if is_owner { face_area_vectors[f] } else { -face_area_vectors[f] };
            let fc  = face_centres[f];
            // Signed pyramid volume: (1/3) · Sf · (fc − cc_est)
            let pv  = sf.dot(fc - cc_est) / 3.0;
            // Pyramid centroid: 3/4 of face centre + 1/4 of apex.
            let pc  = fc * 0.75 + cc_est * 0.25;
            vol        += pv;
            centre_num = centre_num + pc * pv;
        }

        cell_volumes[c] = vol.abs();
        cell_centres[c] = if vol.abs() > f64::MIN_POSITIVE {
            centre_num * (1.0 / vol)
        } else {
            cc_est
        };
    }

    (cell_volumes, cell_centres)
}

#[inline]
fn pt(points: &[[f64; 3]], i: usize) -> Vector3 {
    Vector3::new(points[i][0], points[i][1], points[i][2])
}
