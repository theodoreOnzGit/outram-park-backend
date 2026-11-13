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

use crate::error::AppBuilderError;
use outram_foam_basic_lib::prelude::{BoundaryPatch, FvMesh, FvMeshBuilder, PatchKind, Vector3};
use std::path::Path;
use std::sync::Arc;

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

    let points_raw = read("points")?;
    let faces_raw = read("faces")?;
    let owner_raw = read("owner")?;
    let neighbour_raw = read("neighbour")?;
    let boundary_raw = read("boundary")?;

    let points = parse_points(&points_raw, "points")?;
    let faces = parse_faces(&faces_raw, "faces")?;
    let owner = parse_index_list(&owner_raw, "owner")?;
    let neighbour = parse_index_list(&neighbour_raw, "neighbour")?;
    let patches = parse_boundary(&boundary_raw, "boundary")?;

    let n_cells = owner
        .iter()
        .chain(neighbour.iter())
        .copied()
        .max()
        .map(|m| m + 1)
        .unwrap_or(0);
    let n_internal_faces = neighbour.len();

    let (face_centres, face_area_vectors, face_areas) =
        compute_face_geometry(&points, &faces, "faces")?;

    let (cell_volumes, cell_centres) = compute_cell_geometry(
        &face_centres,
        &face_area_vectors,
        &owner,
        &neighbour,
        n_cells,
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
        .map_err(|e| AppBuilderError::Parse {
            file: poly_mesh_dir.display().to_string(),
            line: 0,
            msg: e.to_string(),
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
                if bytes[i] == b'\n' {
                    out.push('\n');
                }
                i += 1;
            }
            i += 2;
        } else if i + 1 < n && bytes[i] == b'/' && bytes[i + 1] == b'/' {
            // Line comment — skip to end of line.
            while i < n && bytes[i] != b'\n' {
                i += 1;
            }
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
        None => return stripped,
    };
    let mut depth = 0usize;
    for (off, c) in stripped[start..].char_indices() {
        match c {
            '{' => depth += 1,
            '}' => {
                depth -= 1;
                if depth == 0 {
                    return &stripped[start + off + 1..];
                }
            }
            _ => {}
        }
    }
    stripped
}

// ── list extraction ───────────────────────────────────────────────────────────

/// In the data section, locate the leading integer count `N` and the body text
/// inside the surrounding `( ... )` list.
fn extract_list_body<'a>(
    section: &'a str,
    file: &str,
) -> Result<(usize, &'a str), AppBuilderError> {
    let s = section.trim();

    // The count is the first whitespace-delimited token.
    let count_end = s
        .find(|c: char| c.is_whitespace() || c == '(')
        .unwrap_or(s.len());
    let count: usize = s[..count_end]
        .trim()
        .parse()
        .map_err(|_| AppBuilderError::Parse {
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
                if depth == 0 {
                    close = body_start + off;
                    break;
                }
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
    let section = data_section(&stripped);
    let (count, body) = extract_list_body(section, file)?;
    let mut pts = Vec::with_capacity(count);

    let mut s = body;
    while let Some(open) = s.find('(') {
        let close = s[open..].find(')').ok_or_else(|| AppBuilderError::Parse {
            file: file.to_string(),
            line: 0,
            msg: "unclosed '(' in points entry".into(),
        })? + open;

        let triple = s[open + 1..close].trim();
        let nums: Vec<f64> = triple
            .split_whitespace()
            .map(|t| {
                t.parse::<f64>().map_err(|_| AppBuilderError::Parse {
                    file: file.to_string(),
                    line: 0,
                    msg: format!("bad float in points: {t:?}"),
                })
            })
            .collect::<Result<_, _>>()?;
        if nums.len() != 3 {
            return Err(AppBuilderError::Parse {
                file: file.to_string(),
                line: 0,
                msg: format!("point has {} components, expected 3", nums.len()),
            });
        }
        pts.push([nums[0], nums[1], nums[2]]);
        s = &s[close + 1..];
    }

    if pts.len() != count {
        return Err(AppBuilderError::Parse {
            file: file.to_string(),
            line: 0,
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
    let section = data_section(&stripped);
    let (count, body) = extract_list_body(section, file)?;
    let mut faces = Vec::with_capacity(count);

    let mut s = body.trim();
    while !s.is_empty() {
        s = s.trim_start();
        if s.is_empty() {
            break;
        }

        // Skip leading digit(s) before the '('.
        let open = s.find('(').ok_or_else(|| AppBuilderError::Parse {
            file: file.to_string(),
            line: 0,
            msg: "face entry missing '('".into(),
        })?;
        let close = s[open..].find(')').ok_or_else(|| AppBuilderError::Parse {
            file: file.to_string(),
            line: 0,
            msg: "face entry missing ')'".into(),
        })? + open;

        let verts: Vec<usize> = s[open + 1..close]
            .split_whitespace()
            .map(|t| {
                t.parse::<usize>().map_err(|_| AppBuilderError::Parse {
                    file: file.to_string(),
                    line: 0,
                    msg: format!("bad vertex index in faces: {t:?}"),
                })
            })
            .collect::<Result<_, _>>()?;
        faces.push(verts);
        s = &s[close + 1..];
    }

    if faces.len() != count {
        return Err(AppBuilderError::Parse {
            file: file.to_string(),
            line: 0,
            msg: format!("expected {count} faces, parsed {}", faces.len()),
        });
    }
    Ok(faces)
}

/// Parse the `owner` or `neighbour` file: one non-negative integer per entry.
pub fn parse_index_list(text: &str, file: &str) -> Result<Vec<usize>, AppBuilderError> {
    let stripped = strip_foam_comments(text);
    let section = data_section(&stripped);
    let (count, body) = extract_list_body(section, file)?;
    let indices: Vec<usize> = body
        .split_whitespace()
        .map(|t| {
            t.parse::<usize>().map_err(|_| AppBuilderError::Parse {
                file: file.to_string(),
                line: 0,
                msg: format!("bad index in {file}: {t:?}"),
            })
        })
        .collect::<Result<_, _>>()?;
    if indices.len() != count {
        return Err(AppBuilderError::Parse {
            file: file.to_string(),
            line: 0,
            msg: format!("expected {count} entries, parsed {}", indices.len()),
        });
    }
    Ok(indices)
}

/// Parse the `boundary` file into a list of `BoundaryPatch`.
pub fn parse_boundary(text: &str, file: &str) -> Result<Vec<BoundaryPatch>, AppBuilderError> {
    let stripped = strip_foam_comments(text);
    let section = data_section(&stripped);
    let (count, body) = extract_list_body(section, file)?;
    let mut patches = Vec::with_capacity(count);

    let mut s = body.trim();
    while !s.is_empty() {
        s = s.trim_start();
        if s.is_empty() {
            break;
        }

        // Patch name: word before the opening `{`.
        let name_end = s
            .find(|c: char| c.is_whitespace() || c == '{')
            .unwrap_or(s.len());
        let name = s[..name_end].trim().to_string();
        if name.is_empty() {
            break;
        }
        s = s[name_end..].trim_start();

        if !s.starts_with('{') {
            return Err(AppBuilderError::Parse {
                file: file.to_string(),
                line: 0,
                msg: format!("expected '{{' after patch name {name:?}"),
            });
        }
        let close = find_block_end(s).ok_or_else(|| AppBuilderError::Parse {
            file: file.to_string(),
            line: 0,
            msg: format!("unclosed '{{' for patch {name:?}"),
        })?;
        let block = &s[1..close];

        let kind = parse_patch_kind(block);
        let n_faces = parse_dict_usize(block, "nFaces").unwrap_or(0);
        let start_face = parse_dict_usize(block, "startFace").unwrap_or(0);

        patches.push(BoundaryPatch::new(name, start_face, n_faces, kind));
        s = &s[close + 1..];
    }

    if patches.len() != count {
        return Err(AppBuilderError::Parse {
            file: file.to_string(),
            line: 0,
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
                if depth == 0 {
                    return Some(i);
                }
            }
            _ => {}
        }
    }
    None
}

/// Extract the `usize` value for a dictionary keyword like `nFaces  20;`.
fn parse_dict_usize(block: &str, key: &str) -> Option<usize> {
    let pos = block.find(key)?;
    let after = block[pos + key.len()..].trim_start();
    let end = after
        .find(|c: char| c.is_whitespace() || c == ';')
        .unwrap_or(after.len());
    after[..end].parse().ok()
}

/// Map the `type` field in a patch dictionary block to a `PatchKind`.
fn parse_patch_kind(block: &str) -> PatchKind {
    if let Some(pos) = block.find("type") {
        let after = block[pos + 4..].trim_start();
        let end = after
            .find(|c: char| c.is_whitespace() || c == ';')
            .unwrap_or(after.len());
        return match after[..end].trim() {
            "wall" => PatchKind::Wall,
            "symmetry" | "symmetryPlane" => PatchKind::Symmetry,
            "empty" => PatchKind::Empty,
            "wedge" => PatchKind::Wedge,
            "cyclic" => PatchKind::Cyclic,
            "processor" => PatchKind::Processor,
            _ => PatchKind::Patch,
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
    faces: &[Vec<usize>],
    file: &str,
) -> Result<(Vec<Vector3>, Vec<Vector3>, Vec<f64>), AppBuilderError> {
    let n_faces = faces.len();
    let mut face_centres = Vec::with_capacity(n_faces);
    let mut face_area_vectors = Vec::with_capacity(n_faces);
    let mut face_areas = Vec::with_capacity(n_faces);
    let pts: Vec<Vector3> = points.iter().map(|p| Vector3::new(p[0], p[1], p[2])).collect();

    for (fi, verts) in faces.iter().enumerate() {
        if verts.len() < 3 {
            return Err(AppBuilderError::Parse {
                file: file.to_string(),
                line: fi,
                msg: format!("face {fi} has {} vertices, need ≥ 3", verts.len()),
            });
        }
        // gh:#1363: basic-lib's `face_centre_and_area`, upstream's
        // `makeFaceCentresAndAreas` operation for operation (this function
        // had an older private copy of the fan decomposition).
        let (centre, sf) = outram_foam_basic_lib::io::face_centre_and_area(&pts, verts);
        face_centres.push(centre);
        face_area_vectors.push(sf);
        face_areas.push(sf.mag());
    }

    Ok((face_centres, face_area_vectors, face_areas))
}

/// Compute cell volumes and cell centres via pyramid decomposition,
/// upstream's `primitiveMeshTools::makeCellCentresAndVols`.
///
/// gh:#1363: delegates to
/// [`outram_foam_basic_lib::io::cell_geometry_from_faces`], which follows
/// upstream's operation order (owner faces then neighbour faces, `* (1/3)`
/// last). The private implementation this replaced summed face by face with
/// `/ 3.0`, which left cell volumes up to 14 ulp from upstream's on the
/// shockTube blockMesh mesh.
pub(crate) fn compute_cell_geometry(
    face_centres: &[Vector3],
    face_area_vectors: &[Vector3],
    owner: &[usize],
    neighbour: &[usize],
    n_cells: usize,
) -> (Vec<f64>, Vec<Vector3>) {
    outram_foam_basic_lib::io::cell_geometry_from_faces(face_centres, face_area_vectors, owner, neighbour, n_cells)
}

// ── cellZones ────────────────────────────────────────────────────────────────

/// One named cell zone: the material region a neutronics or thermal-hydraulic
/// solver looks up its per-zone properties by.
///
/// Mirrors one entry of `constant/<region>/polyMesh/cellZones`. Cell indices are
/// the same 0-based indices an [`FvMesh`] uses, so `cells` indexes straight into
/// a field's internal array.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CellZone {
    /// Zone name, matching a zone declared in the case's `nuclearData`.
    pub name: String,
    /// The cells belonging to this zone, in file order.
    pub cells: Vec<usize>,
}

/// Parse an OpenFOAM `cellZones` file.
///
/// # Format
///
/// ```text
/// N
/// (
///     zoneName
///     {
///         type cellZone;
///         cellLabels List<label> M ( 0 1 2 … );
///     }
///     …
/// )
/// ```
///
/// # Errors
///
/// [`AppBuilderError::Parse`] if the list header is malformed, a zone body is
/// unclosed, `cellLabels` is missing, or the declared zone count does not match
/// the number parsed.
///
/// # Note on overlap
///
/// OpenFOAM does not forbid a cell appearing in two zones, and this function
/// does not either — it reports what the file says. Use
/// [`zone_of_cell`] when a single zone per cell is required; that is where the
/// ambiguity is resolved and reported.
pub fn parse_cell_zones(text: &str, file: &str) -> Result<Vec<CellZone>, AppBuilderError> {
    let stripped = strip_foam_comments(text);
    let section = data_section(&stripped);
    let (count, body) = extract_list_body(section, file)?;
    let mut zones = Vec::with_capacity(count);

    let mut s = body.trim();
    while !s.is_empty() {
        s = s.trim_start();
        if s.is_empty() {
            break;
        }
        let name_end = s
            .find(|c: char| c.is_whitespace() || c == '{')
            .unwrap_or(s.len());
        let name = s[..name_end].trim().to_string();
        if name.is_empty() {
            break;
        }
        s = s[name_end..].trim_start();
        if !s.starts_with('{') {
            return Err(AppBuilderError::Parse {
                file: file.to_string(),
                line: 0,
                msg: format!("expected '{{' after cell-zone name {name:?}"),
            });
        }
        let close = find_block_end(s).ok_or_else(|| AppBuilderError::Parse {
            file: file.to_string(),
            line: 0,
            msg: format!("unclosed '{{' for cell zone {name:?}"),
        })?;
        let block = &s[1..close];
        let cells = parse_cell_labels(block, &name, file)?;
        zones.push(CellZone { name, cells });
        s = &s[close + 1..];
    }

    if zones.len() != count {
        return Err(AppBuilderError::Parse {
            file: file.to_string(),
            line: 0,
            msg: format!("expected {count} cell zones, parsed {}", zones.len()),
        });
    }
    Ok(zones)
}

/// The `cellLabels  List<label>  M ( … )` entry inside one zone block.
fn parse_cell_labels(block: &str, zone: &str, file: &str) -> Result<Vec<usize>, AppBuilderError> {
    let pos = block
        .find("cellLabels")
        .ok_or_else(|| AppBuilderError::Parse {
            file: file.to_string(),
            line: 0,
            msg: format!("cell zone {zone:?} has no `cellLabels` entry"),
        })?;
    // OpenFOAM writes `cellLabels  List<label>  M ( … )`; the type tag is
    // optional in the format and absent from some writers, so it is skipped
    // rather than required.
    let rest = block[pos + "cellLabels".len()..].trim_start();
    let rest = rest.strip_prefix("List<label>").unwrap_or(rest);
    let (declared, body) = extract_list_body(rest, file)?;
    let mut cells = Vec::with_capacity(declared);
    for tok in body.split_whitespace() {
        cells.push(tok.parse::<usize>().map_err(|_| AppBuilderError::Parse {
            file: file.to_string(),
            line: 0,
            msg: format!("cell zone {zone:?}: `{tok}` is not a cell index"),
        })?);
    }
    if cells.len() != declared {
        return Err(AppBuilderError::Parse {
            file: file.to_string(),
            line: 0,
            msg: format!(
                "cell zone {zone:?}: declared {declared} cell labels, found {}",
                cells.len()
            ),
        });
    }
    Ok(cells)
}

/// Read and parse `<polyMesh>/cellZones`.
///
/// # Errors
///
/// [`AppBuilderError::Io`] if the file cannot be read, or
/// [`AppBuilderError::Parse`] per [`parse_cell_zones`].
///
/// # Note
///
/// Only the uncompressed file is read. OpenFOAM commonly writes
/// `cellZones.gz`; decompress it first (`gzip -d`), as one would to inspect the
/// case by hand.
pub fn read_cell_zones(poly_mesh_dir: &Path) -> Result<Vec<CellZone>, AppBuilderError> {
    let p = poly_mesh_dir.join("cellZones");
    let text = std::fs::read_to_string(&p).map_err(|e| AppBuilderError::Io {
        path: p.clone(),
        source: e,
    })?;
    parse_cell_zones(&text, "cellZones")
}

/// Build the per-cell zone index a neutronics solver needs, from the zone list
/// and the zone ordering the cross-section data uses.
///
/// Returns `zone_of_cell[c]` = the index into `zone_order` of the zone owning
/// cell `c`, which is exactly the `zone_of_cell` argument of
/// [`DiffusionNeutronics::new`].
///
/// # Parameters
///
/// - `zones` — as read by [`read_cell_zones`].
/// - `zone_order` — the zone names in the order the cross-section data holds
///   them (`CrossSectionData::zone_index`). A zone present in the mesh but
///   absent here is an error, not a silent skip.
/// - `n_cells` — the mesh cell count.
///
/// # Errors
///
/// [`AppBuilderError::Parse`] if a mesh zone is not in `zone_order`, if a cell
/// index is out of range, if a cell is claimed by two zones, or if any cell is
/// left unassigned. All four are conditions under which a solve would otherwise
/// run and produce a quietly wrong answer: an unassigned cell would take
/// whatever zone 0 happens to be.
///
/// [`DiffusionNeutronics::new`]: crate::genfoam::neutronics::diffusion::DiffusionNeutronics::new
pub fn zone_of_cell(
    zones: &[CellZone],
    zone_order: &[&str],
    n_cells: usize,
) -> Result<Vec<usize>, AppBuilderError> {
    let err = |msg: String| AppBuilderError::Parse {
        file: "cellZones".to_string(),
        line: 0,
        msg,
    };
    let mut out = vec![usize::MAX; n_cells];
    for z in zones {
        let idx = zone_order
            .iter()
            .position(|n| *n == z.name)
            .ok_or_else(|| {
                err(format!(
                    "mesh cell zone `{}` has no cross-section data (known zones: {})",
                    z.name,
                    zone_order.join(", ")
                ))
            })?;
        for &c in &z.cells {
            if c >= n_cells {
                return Err(err(format!(
                    "cell zone `{}` names cell {c}, but the mesh has {n_cells} cells",
                    z.name
                )));
            }
            if out[c] != usize::MAX && out[c] != idx {
                return Err(err(format!(
                    "cell {c} is claimed by both `{}` and `{}`",
                    zone_order[out[c]], z.name
                )));
            }
            out[c] = idx;
        }
    }
    if let Some(c) = out.iter().position(|&z| z == usize::MAX) {
        let unassigned = out.iter().filter(|&&z| z == usize::MAX).count();
        return Err(err(format!(
            "{unassigned} of {n_cells} cells are in no cell zone (first is cell {c}); \
             every cell needs cross-section data"
        )));
    }
    Ok(out)
}
