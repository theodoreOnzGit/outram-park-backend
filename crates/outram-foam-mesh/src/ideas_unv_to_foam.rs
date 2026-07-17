// SPDX-License-Identifier: GPL-3.0-only
// Copyright (C) 2026 OUTRAM PARK contributors
// Derived from OpenFOAM (www.openfoam.com)
// Copyright (C) 2004-2023 OpenFOAM Foundation
// Copyright (C) 2016-2023 OpenCFD Ltd.
//
// The mesh assembly (face decomposition, owner/neighbour merging, cell
// centre/volume by pyramid decomposition, upper-triangular internal-face
// ordering) follows the algorithm of OpenFOAM's `ideasUnvToFoam` utility and
// `primitiveMesh` geometry (`src/OpenFOAM/meshes/primitiveMesh/`). This is an
// independent Rust re-implementation, not a line-for-line port and not
// affiliated with or endorsed by OpenCFD Ltd. / the OpenFOAM Foundation.
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

//! `ideasUnvToFoam` — import an I-DEAS Universal File (`.unv`) mesh into a
//! `polyMesh`.
//!
//! ## What this does
//!
//! Parses the three UNV dataset records needed to describe a finite-volume
//! mesh and assembles them into an [`outram_foam_basic_lib`] `FvMesh` (the
//! flat polyMesh: points, faces, owner/neighbour, boundary patches, and the
//! derived geometry — cell volumes/centres and face area-vectors/centres):
//!
//! - **Dataset 2411** — nodes: a node label plus its `(x, y, z)` coordinate.
//!   Each node becomes a mesh *point*. Coordinates are read in the file's
//!   length unit and multiplied by an optional `scale` (metres per file unit;
//!   `1.0` if the file is already in metres).
//! - **Dataset 2412** — elements: an FE-descriptor id plus node connectivity.
//!   *Volume* elements become **cells**; *surface* elements are matched to
//!   boundary faces so they can be grouped into named patches.
//! - **Dataset 2467 / 2452 / 2435** — permanent groups: a named set of
//!   element labels. A group of surface elements becomes one **boundary
//!   patch** carrying that group's name.
//!
//! ## Supported element types
//!
//! | FE descriptor | Element | Role | Nodes |
//! |---|---|---|---|
//! | 115 | linear brick | cell (hexahedron) | 8 |
//! | 112 | linear wedge | cell (prism) | 6 |
//! | 111 | linear tetrahedron | cell | 4 |
//! | (5-node fallback) | pyramid | cell | 5 |
//! | 44, 94 | linear quadrilateral / thin-shell quad | boundary face | 4 |
//! | 41, 91 | linear triangle / thin-shell triangle | boundary face | 3 |
//!
//! **Deferred** (silently skipped, counted in [`UnvPolyMesh::n_skipped_elements`]):
//! parabolic / higher-order elements (descriptors 42, 45, 116, 118, …), beam /
//! rod 1-D elements (descriptors 11–32), and any descriptor not in the table
//! above (except the 5-node pyramid fallback). Pyramid has no universally
//! agreed UNV descriptor id, so any 5-node element is treated as a pyramid.
//!
//! ## Assembly
//!
//! Each volume cell is decomposed into its bounding faces (outward-oriented via
//! the cell centroid). Faces shared by exactly two cells are merged into a
//! single *internal* face with `owner = min(cellA, cellB)`, `neighbour = max`,
//! and its area-vector oriented owner → neighbour. Faces belonging to a single
//! cell are *boundary* faces. Internal faces come first (upper-triangular
//! order: sorted by `(owner, neighbour)`), then boundary faces grouped by
//! patch; boundary faces matched to no group land in a trailing `defaultFaces`
//! patch. Face centres/areas and cell centres/volumes are computed with the
//! OpenFOAM `primitiveMesh` decomposition.
//!
//! ## Entry points
//!
//! - [`convert`] — parse a UNV string (scale 1.0, i.e. file already in metres).
//! - [`convert_scaled`] — parse a UNV string with an explicit metre-per-unit scale.
//! - [`convert_file`] — read and parse a `.unv` file from disk.

use std::collections::{BTreeMap, HashMap};
use std::path::Path;

use outram_foam_basic_lib::mesh::{BoundaryPatch, FvMesh, FvMeshBuilder, PatchKind};
use outram_foam_basic_lib::primitives::Vector3;

use crate::MeshError;

/// Small tolerance for degenerate-area / degenerate-volume guards [m² or m³].
const SMALL: f64 = 1.0e-15;

/// The result of importing a `.unv` file: a `polyMesh` in flat form.
///
/// Bundles the point cloud and the global face → point-index connectivity
/// (the parts `FvMesh` does not itself store) together with the assembled
/// [`FvMesh`] (topology + geometry). Together these are a complete polyMesh:
/// `points`, `faces`, owner/neighbour and boundary patches (in `fv_mesh`).
#[derive(Debug, Clone)]
pub struct UnvPolyMesh {
    /// Mesh points [m] — node coordinates, indexed by point id (0-based).
    pub points: Vec<Vector3>,
    /// Global face list: each face is an ordered loop of point indices.
    /// Length equals `fv_mesh.n_faces`; internal faces first, then boundary
    /// faces grouped by patch. Owner-oriented (normal points owner → neighbour
    /// for internal faces, outward for boundary faces).
    pub faces: Vec<Vec<usize>>,
    /// The assembled finite-volume mesh: owner/neighbour, patches, cell
    /// volumes/centres, face area-vectors/centres.
    pub fv_mesh: FvMesh,
    /// Number of 2412 elements that were recognised but deferred (parabolic,
    /// beam, or otherwise unsupported types).
    pub n_skipped_elements: usize,
}

impl UnvPolyMesh {
    /// Total mesh volume [m³] — the sum of all cell volumes. A convenience for
    /// V&V sanity checks.
    pub fn total_volume(&self) -> f64 {
        self.fv_mesh.cell_volumes.iter().sum()
    }
}

/// Recognised UNV element kinds (the subset this converter assembles).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ElemKind {
    // Volume (cell) types.
    Tet,
    Pyramid,
    Prism,
    Hex,
    // Surface (boundary-face) types.
    Tri,
    Quad,
}

impl ElemKind {
    /// Map a UNV FE-descriptor id (and its node count) to an element kind.
    /// Returns `None` for unsupported / deferred descriptors.
    fn from_descriptor(fe_id: i64, n_nodes: usize) -> Option<Self> {
        match fe_id {
            41 | 91 => Some(ElemKind::Tri),
            44 | 94 => Some(ElemKind::Quad),
            111 => Some(ElemKind::Tet),
            112 => Some(ElemKind::Prism),
            115 => Some(ElemKind::Hex),
            // Pyramid has no universally-agreed UNV descriptor id; a 5-node
            // element is treated as a linear pyramid.
            _ if n_nodes == 5 => Some(ElemKind::Pyramid),
            _ => None,
        }
    }

    /// Expected node count for this element kind.
    fn n_nodes(self) -> usize {
        match self {
            ElemKind::Tet => 4,
            ElemKind::Pyramid => 5,
            ElemKind::Prism => 6,
            ElemKind::Hex => 8,
            ElemKind::Tri => 3,
            ElemKind::Quad => 4,
        }
    }

    /// True if this is a volume (cell) element.
    fn is_volume(self) -> bool {
        matches!(
            self,
            ElemKind::Tet | ElemKind::Pyramid | ElemKind::Prism | ElemKind::Hex
        )
    }

    /// Local face topology: each face is a loop of *local* node indices
    /// (indices into the element's own connectivity list), in adjacency order
    /// around the face. Orientation is fixed later against the cell centroid,
    /// so winding here need only be a valid non-self-intersecting cycle.
    /// Only meaningful for volume kinds.
    fn face_topology(self) -> &'static [&'static [usize]] {
        match self {
            ElemKind::Tet => &[&[0, 1, 2], &[0, 1, 3], &[0, 2, 3], &[1, 2, 3]],
            ElemKind::Pyramid => &[
                &[0, 1, 2, 3], // quad base
                &[0, 1, 4],
                &[1, 2, 4],
                &[2, 3, 4],
                &[3, 0, 4],
            ],
            ElemKind::Prism => &[
                &[0, 1, 2], // tri end
                &[3, 4, 5], // tri end
                &[0, 1, 4, 3],
                &[1, 2, 5, 4],
                &[2, 0, 3, 5],
            ],
            ElemKind::Hex => &[
                &[0, 1, 2, 3], // bottom
                &[4, 5, 6, 7], // top
                &[0, 1, 5, 4],
                &[1, 2, 6, 5],
                &[2, 3, 7, 6],
                &[3, 0, 4, 7],
            ],
            // Surface kinds have no sub-faces.
            ElemKind::Tri | ElemKind::Quad => &[],
        }
    }
}

/// A parsed 2412 element: its kind and its point indices (0-based, resolved
/// through the node-label map).
struct ParsedElement {
    kind: ElemKind,
    points: Vec<usize>,
}

/// Intermediate parse of the whole UNV file.
#[derive(Default)]
struct UnvData {
    /// Points in first-seen order [file units, pre-scale].
    points: Vec<Vector3>,
    /// node label → point index.
    node_index: HashMap<i64, usize>,
    /// element label → parsed element.
    elements: HashMap<i64, ParsedElement>,
    /// Number of 2412 records recognised but skipped (deferred types).
    n_skipped: usize,
    /// Group names in file order (deduplicated).
    group_order: Vec<String>,
    /// Group name → referenced element labels.
    groups: HashMap<String, Vec<i64>>,
}

// ── UNV numeric parsing ─────────────────────────────────────────────────────

/// Parse a UNV double, accepting Fortran `D`/`d` exponent markers.
fn parse_f64(tok: &str) -> Option<f64> {
    tok.replace(['D', 'd'], "E").parse::<f64>().ok()
}

// ── Block reader ────────────────────────────────────────────────────────────

/// Split the UNV text into `(dataset_id, content_lines)` blocks. Each block is
/// bracketed by a pair of `-1` delimiter lines (`   -1`, field width 6):
/// `-1`, dataset-id line, content…, `-1`.
fn read_blocks(text: &str) -> Vec<(i64, Vec<&str>)> {
    let lines: Vec<&str> = text.lines().collect();
    let mut blocks = Vec::new();
    let mut i = 0;
    while i < lines.len() {
        if lines[i].trim() != "-1" {
            i += 1;
            continue;
        }
        // lines[i] is an opening delimiter.
        i += 1;
        if i >= lines.len() {
            break;
        }
        let id_line = lines[i].trim();
        if id_line == "-1" {
            // Empty bracket; current line is the next opener — re-loop.
            continue;
        }
        let dataset_id = match id_line.parse::<i64>() {
            Ok(v) => v,
            Err(_) => {
                // Unparseable id: skip to the closing delimiter and move on.
                i += 1;
                while i < lines.len() && lines[i].trim() != "-1" {
                    i += 1;
                }
                i += 1;
                continue;
            }
        };
        i += 1;
        let start = i;
        while i < lines.len() && lines[i].trim() != "-1" {
            i += 1;
        }
        blocks.push((dataset_id, lines[start..i].to_vec()));
        i += 1; // step past the closing delimiter
    }
    blocks
}

// ── Dataset parsers ─────────────────────────────────────────────────────────

/// Parse dataset **2411** (nodes). Two lines per node: a header
/// (`label export_cs disp_cs color`) and a coordinate line (`x y z`).
fn parse_nodes(content: &[&str], data: &mut UnvData) -> Result<(), MeshError> {
    let mut k = 0;
    while k + 1 < content.len() {
        let header: Vec<&str> = content[k].split_whitespace().collect();
        if header.is_empty() {
            k += 1;
            continue;
        }
        let label = header[0]
            .parse::<i64>()
            .map_err(|_| MeshError::DictParse(format!("2411: bad node label {:?}", header[0])))?;
        let coords: Vec<&str> = content[k + 1].split_whitespace().collect();
        if coords.len() < 3 {
            return Err(MeshError::DictParse(format!(
                "2411: node {label} has fewer than 3 coordinates"
            )));
        }
        match (parse_f64(coords[0]), parse_f64(coords[1]), parse_f64(coords[2])) {
            (Some(x), Some(y), Some(z)) => {
                let idx = data.points.len();
                data.points.push(Vector3::new(x, y, z));
                data.node_index.insert(label, idx);
            }
            _ => {
                return Err(MeshError::DictParse(format!(
                    "2411: node {label} has non-numeric coordinates {coords:?}"
                )))
            }
        }
        k += 2;
    }
    Ok(())
}

/// Parse dataset **2412** (elements). Record 1 is
/// `label fe_id phys_prop mat_prop color n_nodes`; beam/rod descriptors
/// (11–32) carry an extra data line before the connectivity, which spans as
/// many lines as needed (≤ 8 node labels per line).
fn parse_elements(content: &[&str], data: &mut UnvData) -> Result<(), MeshError> {
    let mut k = 0;
    while k < content.len() {
        let rec1: Vec<&str> = content[k].split_whitespace().collect();
        if rec1.len() < 6 {
            k += 1;
            continue;
        }
        let label = match rec1[0].parse::<i64>() {
            Ok(v) => v,
            Err(_) => {
                k += 1;
                continue;
            }
        };
        let fe_id = rec1[1].parse::<i64>().unwrap_or(-1);
        let n_nodes = rec1[5].parse::<usize>().unwrap_or(0);
        k += 1;

        // Beam / rod elements (descriptors 11–32) have an extra orientation
        // record before their connectivity.
        if (11..=32).contains(&fe_id) {
            k += 1;
        }

        // Read exactly n_nodes node labels across the following lines.
        let mut node_labels: Vec<i64> = Vec::with_capacity(n_nodes);
        while node_labels.len() < n_nodes && k < content.len() {
            for tok in content[k].split_whitespace() {
                if let Ok(v) = tok.parse::<i64>() {
                    node_labels.push(v);
                }
            }
            k += 1;
        }

        let kind = match ElemKind::from_descriptor(fe_id, n_nodes) {
            Some(kind) if kind.n_nodes() == node_labels.len() => kind,
            _ => {
                data.n_skipped += 1;
                continue;
            }
        };

        // Resolve node labels to point indices.
        let mut points = Vec::with_capacity(node_labels.len());
        for nl in &node_labels {
            let idx = *data.node_index.get(nl).ok_or_else(|| {
                MeshError::Construction(format!(
                    "2412: element {label} references undefined node {nl}"
                ))
            })?;
            points.push(idx);
        }
        data.elements.insert(label, ParsedElement { kind, points });
    }
    Ok(())
}

/// Parse a permanent-group dataset (**2467 / 2452 / 2435**). Record 1 ends with
/// the entity count (field 8); record 2 is the group name; the remaining lines
/// list entities four integers each (`type tag n1 n2`), two entities per line.
/// Entity type 8 = FE element.
fn parse_groups(content: &[&str], data: &mut UnvData) -> Result<(), MeshError> {
    let mut k = 0;
    while k < content.len() {
        let rec1: Vec<&str> = content[k].split_whitespace().collect();
        if rec1.len() < 8 {
            k += 1;
            continue;
        }
        let n_entities = match rec1[7].parse::<usize>() {
            Ok(v) => v,
            Err(_) => {
                k += 1;
                continue;
            }
        };
        k += 1;
        if k >= content.len() {
            break;
        }
        let name = content[k].trim().to_string();
        k += 1;

        // Entities: 4 ints each, 2 per line ⇒ ceil(n/2) lines.
        let n_lines = n_entities.div_ceil(2);
        let mut ints: Vec<i64> = Vec::with_capacity(n_entities * 4);
        for _ in 0..n_lines {
            if k >= content.len() {
                break;
            }
            for tok in content[k].split_whitespace() {
                if let Ok(v) = tok.parse::<i64>() {
                    ints.push(v);
                }
            }
            k += 1;
        }

        let mut labels: Vec<i64> = Vec::new();
        for e in 0..n_entities {
            let base = e * 4;
            if base + 1 >= ints.len() {
                break;
            }
            let etype = ints[base];
            let tag = ints[base + 1];
            if etype == 8 {
                labels.push(tag); // FE element
            }
        }

        if !labels.is_empty() {
            if !data.groups.contains_key(&name) {
                data.group_order.push(name.clone());
            }
            data.groups.entry(name).or_default().extend(labels);
        }
    }
    Ok(())
}

// ── Geometry helpers ────────────────────────────────────────────────────────

/// Face centre [m] and area vector [m²] of a planar polygon given its vertex
/// loop, using the OpenFOAM `primitiveMesh` face decomposition (fan from the
/// vertex average, area-weighted centroid).
fn face_centre_area(pts: &[Vector3]) -> (Vector3, Vector3) {
    let n = pts.len();
    if n == 3 {
        let centre = (pts[0] + pts[1] + pts[2]) / 3.0;
        let sf = (pts[1] - pts[0]).cross(pts[2] - pts[0]) * 0.5;
        return (centre, sf);
    }
    let mut f_centre = Vector3::ZERO;
    for p in pts {
        f_centre += *p;
    }
    f_centre /= n as f64;

    let mut sum_n = Vector3::ZERO;
    let mut sum_a = 0.0;
    let mut sum_ac = Vector3::ZERO;
    for i in 0..n {
        let p1 = pts[i];
        let p2 = pts[(i + 1) % n];
        let c = p1 + p2 + f_centre; // 3 × sub-triangle centroid
        let nrm = (p2 - p1).cross(f_centre - p1); // 2 × sub-triangle area vector
        let a = nrm.mag();
        sum_n += nrm;
        sum_a += a;
        sum_ac += a * c;
    }
    let centre = if sum_a > SMALL {
        sum_ac / (3.0 * sum_a)
    } else {
        f_centre
    };
    (centre, sum_n * 0.5)
}

/// Cell centre [m] and volume [m³] from the cell's outward-oriented faces,
/// using the OpenFOAM pyramid decomposition about an estimated centre.
fn cell_centre_volume(faces: &[(Vector3, Vector3)]) -> (Vector3, f64) {
    let mut c_est = Vector3::ZERO;
    for (cf, _) in faces {
        c_est += *cf;
    }
    c_est /= faces.len() as f64;

    let mut vol3 = 0.0;
    let mut ctr = Vector3::ZERO;
    for (cf, sf) in faces {
        let pyr3 = sf.dot(*cf - c_est); // 3 × pyramid volume (sf outward ⇒ ≥ 0)
        let pc = 0.75 * *cf + 0.25 * c_est; // pyramid centroid
        ctr += pyr3 * pc;
        vol3 += pyr3;
    }
    let centre = if vol3.abs() > SMALL { ctr / vol3 } else { c_est };
    (centre, vol3 / 3.0)
}

// ── Assembly ────────────────────────────────────────────────────────────────

/// One cell's outward-oriented face: its point loop, area-vector, and centre.
struct CellFace {
    loop_pts: Vec<usize>,
    sf: Vector3,
    cf: Vector3,
}

/// A face occurrence keyed by its sorted vertex set.
struct FaceOcc {
    /// Owner-side cell (first cell that produced the face).
    c0: usize,
    /// Outward-oriented loop from `c0`.
    loop0: Vec<usize>,
    /// Second (neighbour-side) cell, if the face is shared.
    c1: Option<usize>,
}

/// Assemble the parsed UNV data into a `polyMesh`.
fn assemble(data: UnvData, scale: f64) -> Result<UnvPolyMesh, MeshError> {
    // Scale points into metres.
    let points: Vec<Vector3> = data.points.iter().map(|p| *p * scale).collect();

    // Split elements into cells (volume) and surface elements; build the
    // surface-face lookup (element label → sorted vertex key).
    let mut cells: Vec<&ParsedElement> = Vec::new();
    let mut surface_key: HashMap<i64, Vec<usize>> = HashMap::new();
    for (label, elem) in &data.elements {
        if elem.kind.is_volume() {
            cells.push(elem);
        } else {
            let mut key = elem.points.clone();
            key.sort_unstable();
            surface_key.insert(*label, key);
        }
    }
    // Deterministic cell order (by point-index list).
    cells.sort_by(|a, b| a.points.cmp(&b.points));

    if cells.is_empty() {
        return Err(MeshError::Construction(
            "no volume (cell) elements found in UNV file".to_string(),
        ));
    }

    // sorted-vertex-key → patch name, from the groups of surface elements.
    let mut key_to_patch: HashMap<Vec<usize>, String> = HashMap::new();
    for name in &data.group_order {
        if let Some(labels) = data.groups.get(name) {
            for l in labels {
                if let Some(key) = surface_key.get(l) {
                    key_to_patch.entry(key.clone()).or_insert_with(|| name.clone());
                }
            }
        }
    }

    // Generate per-cell outward faces and cell geometry.
    let n_cells = cells.len();
    let mut cell_volumes = vec![0.0; n_cells];
    let mut cell_centres = vec![Vector3::ZERO; n_cells];
    let mut face_map: BTreeMap<Vec<usize>, FaceOcc> = BTreeMap::new();

    for (ci, cell) in cells.iter().enumerate() {
        // Approximate cell centroid = mean of its vertices, used to orient faces.
        let mut cell_centroid = Vector3::ZERO;
        for &p in &cell.points {
            cell_centroid += points[p];
        }
        cell_centroid /= cell.points.len() as f64;

        let mut cfaces: Vec<CellFace> = Vec::new();
        for face_local in cell.kind.face_topology() {
            let mut loop_pts: Vec<usize> = face_local.iter().map(|&li| cell.points[li]).collect();
            let coords: Vec<Vector3> = loop_pts.iter().map(|&p| points[p]).collect();
            let (cf, mut sf) = face_centre_area(&coords);
            // Orient outward: normal should point away from the cell centroid.
            if sf.dot(cf - cell_centroid) < 0.0 {
                loop_pts.reverse();
                sf = -sf;
            }
            cfaces.push(CellFace { loop_pts, sf, cf });
        }

        // Cell geometry from its outward faces.
        let face_geom: Vec<(Vector3, Vector3)> = cfaces.iter().map(|f| (f.cf, f.sf)).collect();
        let (centre, vol) = cell_centre_volume(&face_geom);
        if vol <= 0.0 {
            return Err(MeshError::Construction(format!(
                "cell {ci} has non-positive volume {vol:.6e} m^3 (degenerate or inverted element)"
            )));
        }
        cell_volumes[ci] = vol;
        cell_centres[ci] = centre;

        // Register faces for owner/neighbour merging.
        for cf in cfaces {
            let mut key = cf.loop_pts.clone();
            key.sort_unstable();
            match face_map.get_mut(&key) {
                None => {
                    face_map.insert(
                        key,
                        FaceOcc {
                            c0: ci,
                            loop0: cf.loop_pts,
                            c1: None,
                        },
                    );
                }
                Some(occ) => {
                    if occ.c1.is_some() {
                        return Err(MeshError::Construction(format!(
                            "non-manifold face shared by more than two cells: points {:?}",
                            cf.loop_pts
                        )));
                    }
                    occ.c1 = Some(ci);
                }
            }
        }
    }

    // Partition into internal and boundary faces.
    // Internal: (owner, neighbour, loop) with owner < neighbour, loop outward
    // from owner (⇒ oriented owner → neighbour).
    let mut internal: Vec<(usize, usize, Vec<usize>)> = Vec::new();
    // Boundary: (owner, loop, sorted-key).
    let mut boundary: Vec<(usize, Vec<usize>, Vec<usize>)> = Vec::new();

    for (key, occ) in face_map {
        match occ.c1 {
            Some(c1) => {
                let (owner, neighbour, oriented) = if occ.c0 < c1 {
                    (occ.c0, c1, occ.loop0)
                } else {
                    // Owner is the other cell; outward-from-owner is the reverse.
                    let mut rev = occ.loop0.clone();
                    rev.reverse();
                    (c1, occ.c0, rev)
                };
                internal.push((owner, neighbour, oriented));
            }
            None => {
                boundary.push((occ.c0, occ.loop0, key));
            }
        }
    }

    // Upper-triangular ordering of internal faces: sort by (owner, neighbour).
    internal.sort_by_key(|a| (a.0, a.1));
    let n_internal = internal.len();

    // Group boundary faces by patch name (group order, then defaultFaces).
    let mut patch_faces: HashMap<String, Vec<(usize, Vec<usize>)>> = HashMap::new();
    let default_name = "defaultFaces".to_string();
    for (owner, loop_pts, key) in boundary {
        let name = key_to_patch
            .get(&key)
            .cloned()
            .unwrap_or_else(|| default_name.clone());
        patch_faces.entry(name).or_default().push((owner, loop_pts));
    }

    // Ordered patch list: named groups first (file order), then defaultFaces.
    let mut patch_names: Vec<String> = Vec::new();
    for name in &data.group_order {
        if patch_faces.contains_key(name) && !patch_names.contains(name) {
            patch_names.push(name.clone());
        }
    }
    if patch_faces.contains_key(&default_name) {
        patch_names.push(default_name.clone());
    }

    // Build the global face arrays: internal first, then per-patch boundary.
    let n_boundary: usize = patch_faces.values().map(|v| v.len()).sum();
    let n_faces = n_internal + n_boundary;

    let mut faces: Vec<Vec<usize>> = Vec::with_capacity(n_faces);
    let mut owner: Vec<usize> = Vec::with_capacity(n_faces);
    let mut neighbour: Vec<usize> = Vec::with_capacity(n_internal);

    for (own, nei, loop_pts) in &internal {
        faces.push(loop_pts.clone());
        owner.push(*own);
        neighbour.push(*nei);
    }

    let mut patches: Vec<BoundaryPatch> = Vec::new();
    let mut start = n_internal;
    for name in &patch_names {
        let pfaces = patch_faces.get(name).expect("patch present");
        let size = pfaces.len();
        for (own, loop_pts) in pfaces {
            faces.push(loop_pts.clone());
            owner.push(*own);
        }
        patches.push(BoundaryPatch::new(name.clone(), start, size, PatchKind::Patch));
        start += size;
    }

    // Face geometry for all faces (from the final owner-oriented loops).
    let mut face_area_vectors: Vec<Vector3> = Vec::with_capacity(n_faces);
    let mut face_centres: Vec<Vector3> = Vec::with_capacity(n_faces);
    for f in &faces {
        let coords: Vec<Vector3> = f.iter().map(|&p| points[p]).collect();
        let (cf, sf) = face_centre_area(&coords);
        face_centres.push(cf);
        face_area_vectors.push(sf);
    }

    let fv_mesh = FvMeshBuilder::new()
        .n_cells(n_cells)
        .n_internal_faces(n_internal)
        .owner(owner)
        .neighbour(neighbour)
        .patches(patches)
        .cell_volumes(cell_volumes)
        .cell_centres(cell_centres)
        .face_area_vectors(face_area_vectors)
        .face_centres(face_centres)
        .build()
        .map_err(|e| MeshError::Construction(format!("polyMesh assembly failed: {e}")))?;

    Ok(UnvPolyMesh {
        points,
        faces,
        fv_mesh,
        n_skipped_elements: data.n_skipped,
    })
}

// ── Public API ──────────────────────────────────────────────────────────────

/// Parse UNV text into intermediate data, then assemble the polyMesh.
fn parse_and_assemble(text: &str, scale: f64) -> Result<UnvPolyMesh, MeshError> {
    let mut data = UnvData::default();
    for (id, content) in read_blocks(text) {
        match id {
            2411 => parse_nodes(&content, &mut data)?,
            2412 => parse_elements(&content, &mut data)?,
            2467 | 2452 | 2435 | 2477 | 2432 | 2417 => parse_groups(&content, &mut data)?,
            _ => {} // dataset not needed for mesh assembly — ignore
        }
    }
    if data.points.is_empty() {
        return Err(MeshError::DictParse(
            "UNV file contains no nodes (dataset 2411)".to_string(),
        ));
    }
    assemble(data, scale)
}

/// Convert an I-DEAS `.unv` mesh (given as text) into a `polyMesh`, treating
/// the file's coordinates as already being in metres (scale `1.0`).
///
/// See the module docs for the supported dataset records and element types.
pub fn convert(unv_text: &str) -> Result<UnvPolyMesh, MeshError> {
    parse_and_assemble(unv_text, 1.0)
}

/// Convert an I-DEAS `.unv` mesh (given as text) into a `polyMesh`, scaling
/// every coordinate by `scale` [metres per file unit] — e.g. `1.0e-3` for a
/// file authored in millimetres.
pub fn convert_scaled(unv_text: &str, scale: f64) -> Result<UnvPolyMesh, MeshError> {
    parse_and_assemble(unv_text, scale)
}

/// Read a `.unv` file from disk and convert it into a `polyMesh` (scale `1.0`).
pub fn convert_file(path: impl AsRef<Path>) -> Result<UnvPolyMesh, MeshError> {
    let path = path.as_ref();
    let text = std::fs::read_to_string(path)
        .map_err(|e| MeshError::DictParse(format!("cannot read {}: {e}", path.display())))?;
    convert(&text)
}
