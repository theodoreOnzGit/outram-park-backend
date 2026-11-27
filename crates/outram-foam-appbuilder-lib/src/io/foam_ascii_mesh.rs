// SPDX-License-Identifier: GPL-3.0-only
// Copyright (C) 2026 OUTRAM PARK contributors
// Derived from OpenFOAM (www.openfoam.com)
// Copyright (C) 2004-2023 OpenFOAM Foundation
// Copyright (C) 2016-2023 OpenCFD Ltd.
//
// Port of the ASCII write of the `polyMesh` topology objects of OpenFOAM-ESI v2506:
//   src/OpenFOAM/meshes/polyMesh/polyMeshIO.C                      polyMesh::writeObject (points, faces, owner, neighbour, boundary, zones)
//   src/OpenFOAM/meshes/polyMesh/polyMeshFromShapeMesh.C           (`note "nPoints: ... nInternalFaces: ..."` of owner/neighbour: `polyMesh::updateMesh` `owner_.note()`)
//   src/OpenFOAM/meshes/polyMesh/polyPatches/polyPatch/polyPatch.C `polyPatch::write` (type, inGroups, nFaces, startFace)
//   src/OpenFOAM/meshes/polyMesh/polyPatches/derived/wall/wallPolyPatch.C   (the `wall` group a wall patch carries)
//   src/OpenFOAM/meshes/polyMesh/zones/ZoneMesh/ZoneMesh.C         ZoneMesh::writeData, `meta` names
//   src/OpenFOAM/meshes/polyMesh/zones/cellZone/cellZone.C         cellZone::writeDict
//   GeN-Foam 652b3da src/classes/multiRegion/meshHandler/meshHandler.C:389-506 (`createBaffleLessMeshes`)
//
// This file is part of OUTRAM PARK. See `src/lib.rs` for the full notice.

//! # `foam_ascii_mesh` — the `polyMesh` files of a baffle-less mesh, byte for byte (gh:#1935, README "Round 69")
//!
//! GeN-Foam's `meshHandler::createBaffleLessMeshes` copies the points, faces and cells of a region whose `controlDict` says
//! `removeBaffles { <region> true; }` into a new mesh named `<region>.baffleLess`, copies the patches as plain `polyPatch`es
//! (type `patch`, the original's `inGroups` kept), runs `removeBaffles` and copies the zones. That mesh is registered under
//! `runTime`, so `runTime.write()` puts `<time>/<region>.baffleLess/polyMesh/{points, faces, owner, neighbour, boundary, cellZones,
//! faceZones, pointZones}` in every time directory.
//!
//! [`write_poly_mesh`] writes those eight files from a [`PolyMeshOut`]. [`PolyMeshOut::check_unchanged_by_removal`] is the
//! precondition under which `removeBaffles` (`polyTopoChange::changeMesh`) leaves a mesh WITHOUT duplicate faces exactly as it was:
//! faces in canonical upper-triangular order, every point used. A mesh failing it is refused by name
//! ([`PolyMeshError`]), as are more than one cell zone (the multi-zone layout of `ZoneMesh::writeData` was never seen in a real file),
//! non-empty face or point zones and patch types other than `patch`, `wall`, `empty` and (since gh:#1236) `mappedWall`.
//!
//! `no_std` + `alloc`; no file I/O. V&V: `tests/genfoam_written_rest_vs_upstream.rs` (README "Round 69").

#[cfg(not(any(feature = "std", test)))]
#[allow(unused_imports)]
use crate::nostd_prelude::*;
use core::fmt::Write as _;

use super::foam_ascii::{footer, header_text, statement_text, write_list_file, write_list_text, Data, ListObject, WriteOptions};

/// A patch of the copied mesh: a plain `polyPatch`.
#[derive(Debug, Clone, PartialEq)]
pub struct PolyPatchOut {
    /// Patch name.
    pub name: String,
    /// The group list (`inGroups`) of the copy: the original's, including the constraint-type or `wall` group.
    pub in_groups: Vec<String>,
    /// `physicalType` of the original patch (`patchIdentifier::write` prints it between `type` and `inGroups` when set; gh:#1236, README round 78:
    /// the shipped 3D_SmallESFR_old fluid mesh has `physicalType patch;` on its wall and inlet/outlet patches).
    pub physical_type: Option<String>,
    /// `nFaces`.
    pub n_faces: usize,
    /// `startFace`.
    pub start_face: usize,
}

/// What the baffle-less mesh holds, as `polyMesh::writeObject` writes it.
#[derive(Debug, Clone, PartialEq)]
pub struct PolyMeshOut {
    /// `points`.
    pub points: Vec<[f64; 3]>,
    /// `faces`: the point labels of every face.
    pub faces: Vec<Vec<usize>>,
    /// `owner`, every face.
    pub owner: Vec<usize>,
    /// `neighbour`, the internal faces.
    pub neighbour: Vec<usize>,
    /// `nCells`.
    pub n_cells: usize,
    /// The patches, in order.
    pub patches: Vec<PolyPatchOut>,
    /// The cell zones as `(name, cellLabels)`.
    pub cell_zones: Vec<(String, Vec<usize>)>,
    /// Number of face zones and of point zones (both must be 0).
    pub n_face_zones: usize,
    /// Number of point zones.
    pub n_point_zones: usize,
}

/// Why a mesh is not written.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PolyMeshError {
    /// The mesh is not in the order `polyTopoChange::changeMesh` would leave it in, or has an unused point: `removeBaffles` would renumber it.
    NotCanonical(String),
    /// A construct whose file layout was not seen in a real file.
    Refused(String),
}

impl core::fmt::Display for PolyMeshError {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            Self::NotCanonical(m) => write!(f, "baffle-less mesh: {m}"),
            Self::Refused(m) => write!(f, "baffle-less mesh: {m} is refused by name (not ported)"),
        }
    }
}

impl PolyMeshOut {
    /// What `<start>/<region>.baffleLess/polyMesh` holds (gh:#1236, README round 78): `createBaffleLessMeshes` writes the mesh INSIDE
    /// `removeBaffles` (`mesh.write()`, `mergeOrSplitBaffles.C`), BEFORE it copies the zones (`addZones`, `meshHandler.C:491-525`), so the three zone
    /// files of the start directory are `0()`; the zones appear only in the directories written later. This copy has none.
    #[must_use]
    pub fn at_start(&self) -> Self {
        let mut m = self.clone();
        m.cell_zones.clear();
        m.n_face_zones = 0;
        m.n_point_zones = 0;
        m
    }

    /// The faces of the copy are exactly the faces of the region mesh when `removeBaffles` (`polyTopoChange::changeMesh`, no inflation, no
    /// cell or point ordering, no duplicate faces to merge) renumbers nothing: internal faces ordered by owner and then neighbour
    /// (`polyTopoChange::reorderCompactFaces`), boundary faces in patch order, every point used (`polyTopoChange::compact` drops unused points).
    ///
    /// # Errors
    /// [`PolyMeshError::NotCanonical`] naming the first violation; [`PolyMeshError::Refused`] for zones and patches outside what was seen.
    pub fn check_unchanged_by_removal(&self) -> Result<(), PolyMeshError> {
        let n_int = self.neighbour.len();
        if self.faces.len() != self.owner.len() || n_int > self.owner.len() {
            return Err(PolyMeshError::NotCanonical("faces, owner and neighbour disagree in size".to_string()));
        }
        for f in 1..n_int {
            let ordered = self.owner[f] > self.owner[f - 1] || (self.owner[f] == self.owner[f - 1] && self.neighbour[f] > self.neighbour[f - 1]);
            if !ordered {
                return Err(PolyMeshError::NotCanonical(format!("internal face {f} is out of upper-triangular order (removeBaffles would renumber the faces)")));
            }
        }
        if (0..n_int).any(|f| self.owner[f] >= self.neighbour[f]) {
            return Err(PolyMeshError::NotCanonical("an internal face with owner >= neighbour".to_string()));
        }
        let mut next = n_int;
        for p in &self.patches {
            if p.start_face != next {
                return Err(PolyMeshError::NotCanonical(format!("patch {} does not start where the previous one ends", p.name)));
            }
            next += p.n_faces;
        }
        if next != self.faces.len() {
            return Err(PolyMeshError::NotCanonical("the patches do not cover the boundary faces".to_string()));
        }
        let mut used = vec![false; self.points.len()];
        for face in &self.faces {
            for &pt in face {
                if pt >= used.len() {
                    return Err(PolyMeshError::NotCanonical("a face point label beyond the points".to_string()));
                }
                used[pt] = true;
            }
        }
        if used.iter().any(|u| !*u) {
            return Err(PolyMeshError::NotCanonical("an unused point (polyTopoChange::compact would drop it)".to_string()));
        }
        if self.n_face_zones != 0 || self.n_point_zones != 0 {
            return Err(PolyMeshError::Refused("a non-empty face or point zone".to_string()));
        }
        if self.cell_zones.len() > 1 {
            return Err(PolyMeshError::Refused(format!("{} cell zones (the one-zone layout of `cellZones` is the only one seen)", self.cell_zones.len())));
        }
        Ok(())
    }
}

/// `owner`/`neighbour` header: the `note` line sits between `arch` and `class`.
fn header_with_note(class: &str, location: &str, object: &str, note: &str) -> String {
    let plain = header_text(class, location, object);
    let marker = format!("    class       {class};\n");
    match plain.find(&marker) {
        Some(at) => format!("{}    note        \"{note}\";\n{}", &plain[..at], &plain[at..]),
        None => plain,
    }
}

/// The `meta { names N ( a b ); }` block `ZoneMesh` puts in the header of `cellZones`, before the closing brace of `FoamFile`.
fn header_with_meta(class: &str, location: &str, object: &str, names: &[String]) -> String {
    let plain = header_text(class, location, object);
    let marker = "}\n// * * *";
    match plain.find(marker) {
        Some(at) => {
            let mut s = String::from(&plain[..at]);
            let _ = write!(s, "    meta\n    {{\n        names           {} (", names.len());
            for n in names {
                let _ = write!(s, " {n}");
            }
            s.push_str(" );\n    }\n");
            s.push_str(&plain[at..]);
            s
        }
        None => plain,
    }
}

fn label_entries(v: &[usize]) -> Vec<String> {
    v.iter().map(ToString::to_string).collect()
}

fn all_equal(e: &[String]) -> bool {
    e.len() > 1 && e.iter().all(|x| *x == e[0])
}

/// The `polyBoundaryMesh` file: `N`, `(`, one `{ type patch; [inGroups k(...);] nFaces n; startFace s; }` per patch, `)`.
fn boundary_text(m: &PolyMeshOut, location: &str) -> String {
    let mut s = header_text("polyBoundaryMesh", location, "boundary");
    let _ = write!(s, "{}\n(\n", m.patches.len());
    for p in &m.patches {
        let _ = write!(s, "    {}\n    {{\n", p.name);
        s.push_str(&statement_text(2, "type", "patch"));
        if let Some(pt) = &p.physical_type {
            s.push_str(&statement_text(2, "physicalType", pt));
        }
        if !p.in_groups.is_empty() {
            let mut g = format!("{}(", p.in_groups.len());
            for (k, w) in p.in_groups.iter().enumerate() {
                if k > 0 {
                    g.push(' ');
                }
                g.push_str(w);
            }
            g.push(')');
            s.push_str(&statement_text(2, "inGroups", &g));
        }
        s.push_str(&statement_text(2, "nFaces", &p.n_faces.to_string()));
        s.push_str(&statement_text(2, "startFace", &p.start_face.to_string()));
        s.push_str("    }\n");
    }
    s.push(')');
    s.push_str(&footer());
    s
}

fn cell_zones_text(m: &PolyMeshOut, location: &str) -> String {
    if m.cell_zones.is_empty() {
        // the start-time form (`PolyMeshOut::at_start`): `0()`, no `meta` block
        return empty_zones_text(location, "cellZones");
    }
    let names: Vec<String> = m.cell_zones.iter().map(|(n, _)| n.clone()).collect();
    let mut s = header_with_meta("regIOobject", location, "cellZones", &names);
    let _ = write!(s, "{}\n(", m.cell_zones.len());
    for (name, labels) in &m.cell_zones {
        let e = label_entries(labels);
        let _ = write!(s, "{name}\n{{\n    type            cellZone;\n    cellLabels      List<label> ");
        s.push_str(&write_list_text(&e, true, all_equal(&e), 10));
        s.push_str(";\n}\n");
    }
    s.push(')');
    s.push_str(&footer());
    s
}

fn empty_zones_text(location: &str, object: &str) -> String {
    let mut s = header_text("regIOobject", location, object);
    s.push_str("0()");
    s.push_str(&footer());
    s
}

/// The eight files of `<time>/<region>.baffleLess/polyMesh`, as `(file name, text)` in alphabetical order. `location` is the header's
/// `location` (`100/fluidRegion.baffleLess/polyMesh`).
///
/// # Errors
/// [`PolyMeshError`] when [`PolyMeshOut::check_unchanged_by_removal`] fails.
pub fn write_poly_mesh(m: &PolyMeshOut, location: &str, opts: &WriteOptions) -> Result<Vec<(String, String)>, PolyMeshError> {
    m.check_unchanged_by_removal()?;
    let note = format!("nPoints:{}  nCells:{}  nFaces:{}  nInternalFaces:{}", m.points.len(), m.n_cells, m.faces.len(), m.neighbour.len());
    let list_file = |class: &str, object: &str, v: &[usize], with_note: bool| {
        let e = label_entries(v);
        let mut s = if with_note { header_with_note(class, location, object, &note) } else { header_text(class, location, object) };
        s.push_str(&write_list_text(&e, true, all_equal(&e), 10));
        s.push_str(&footer());
        s
    };
    let face_entries: Vec<String> = m
        .faces
        .iter()
        .map(|f| {
            let mut s = format!("{}(", f.len());
            for (k, p) in f.iter().enumerate() {
                if k > 0 {
                    s.push(' ');
                }
                let _ = write!(s, "{p}");
            }
            s.push(')');
            s
        })
        .collect();
    let mut faces = header_text("faceList", location, "faces");
    faces.push_str(&write_list_text(&face_entries, false, false, 10));
    faces.push_str(&footer());
    let pts: Vec<[f64; 3]> = m.points.clone();
    Ok(vec![
        ("boundary".to_string(), boundary_text(m, location)),
        ("cellZones".to_string(), cell_zones_text(m, location)),
        ("faceZones".to_string(), empty_zones_text(location, "faceZones")),
        ("faces".to_string(), faces),
        ("neighbour".to_string(), list_file("labelList", "neighbour", &m.neighbour, true)),
        ("owner".to_string(), list_file("labelList", "owner", &m.owner, true)),
        ("pointZones".to_string(), empty_zones_text(location, "pointZones")),
        ("points".to_string(), write_list_file("vectorField", location, "points", &ListObject::Plain(Data::vector(&pts)), opts)),
    ])
}

/// The patch types of a case's `boundary` file the copy supports, with the `inGroups` the copy keeps: the groups listed in the file
/// (`inGroups List<word> k(a b)` or `inGroups k(a b)`) plus the group the type itself adds (`wallPolyPatch`: `wall`; a constraint patch:
/// its own type name, `polyPatch::polyPatch` `inGroups().appendUniq(patchType)`).
///
/// `text` is the `constant/<region>/polyMesh/boundary` file. Returns `(name, type, groups, nFaces, startFace)` in file order.
///
/// # Errors
/// [`PolyMeshError::Refused`] for a patch type other than `patch`, `wall`, `empty`, `mappedWall`, or a file that does not parse.
pub fn parse_boundary_patches(text: &str) -> Result<Vec<PolyPatchOut>, PolyMeshError> {
    let bad = |m: &str| PolyMeshError::Refused(format!("boundary file: {m}"));
    let open = text.find("\n(").ok_or_else(|| bad("no patch list"))?;
    let mut rest = &text[open + 2..];
    let mut out = Vec::new();
    loop {
        let Some(brace) = rest.find('{') else { break };
        let name = rest[..brace].trim().to_string();
        let end = rest[brace..].find('}').ok_or_else(|| bad("unclosed patch"))? + brace;
        let body = &rest[brace + 1..end];
        let (mut ptype, mut groups, mut n_faces, mut start) = (String::new(), Vec::<String>::new(), None, None);
        let mut physical_type: Option<String> = None;
        for stmt in body.split(';') {
            let s = stmt.trim();
            if s.is_empty() {
                continue;
            }
            let (k, v) = s.split_once(char::is_whitespace).ok_or_else(|| bad("a statement without a value"))?;
            let v = v.trim();
            match k {
                "type" => ptype = v.to_string(),
                "inGroups" => {
                    let inner = v.split_once('(').and_then(|(_, r)| r.rsplit_once(')')).map_or("", |(a, _)| a);
                    groups = inner.split_whitespace().map(ToString::to_string).collect();
                }
                "physicalType" => physical_type = Some(v.to_string()),
                "nFaces" => n_faces = v.parse::<usize>().ok(),
                "startFace" => start = v.parse::<usize>().ok(),
                _ => {}
            }
        }
        match ptype.as_str() {
            "patch" => {}
            "wall" | "empty" => {
                if !groups.contains(&ptype) {
                    groups.push(ptype.clone());
                }
            }
            // `mappedWall` (the ESFR baffles, gh:#1236): `createBaffleLessMeshes` copies it as a plain `polyPatch` (`type patch`, the sample entries
            // dropped), keeping the groups `wallPolyPatch` and `mappedPatchBase` added; the file lists them, any missing one is appended
            "mappedWall" => {
                for g in ["wall", "mappedPatch"] {
                    if !groups.iter().any(|x| x == g) {
                        groups.push(g.to_string());
                    }
                }
            }
            other => return Err(PolyMeshError::Refused(format!("patch type {other}"))),
        }
        out.push(PolyPatchOut { name, in_groups: groups, physical_type, n_faces: n_faces.ok_or_else(|| bad("no nFaces"))?, start_face: start.ok_or_else(|| bad("no startFace"))? });
        rest = &rest[end + 1..];
    }
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn tiny() -> PolyMeshOut {
        // two cells side by side: points 0..12 are not needed to be geometric here; every point must be used
        let faces = vec![vec![1, 4, 10, 7], vec![0, 3, 9, 6], vec![0, 1, 7, 6], vec![3, 4, 10, 9], vec![0, 1, 4, 3], vec![6, 7, 10, 9], vec![1, 2, 5, 4], vec![7, 8, 11, 10], vec![2, 5, 11, 8], vec![1, 2, 8, 7], vec![4, 5, 11, 10]];
        PolyMeshOut {
            points: (0..12).map(|i| [f64::from(i), 0.0, 0.5]).collect(),
            faces,
            owner: vec![0, 0, 0, 0, 0, 0, 1, 1, 1, 1, 1],
            neighbour: vec![1],
            n_cells: 2,
            patches: vec![
                PolyPatchOut { name: "a".to_string(), in_groups: vec!["wall".to_string()], physical_type: None, n_faces: 6, start_face: 1 },
                PolyPatchOut { name: "b".to_string(), in_groups: vec![], physical_type: None, n_faces: 4, start_face: 7 },
            ],
            cell_zones: vec![("zone0".to_string(), vec![0, 1])],
            n_face_zones: 0,
            n_point_zones: 0,
        }
    }

    #[test]
    fn a_canonical_mesh_is_written_as_eight_files_with_the_real_layout() {
        let m = tiny();
        let files = write_poly_mesh(&m, "100/r.baffleLess/polyMesh", &WriteOptions::precision_17()).expect("canonical");
        assert_eq!(files.iter().map(|(n, _)| n.as_str()).collect::<Vec<_>>(), ["boundary", "cellZones", "faceZones", "faces", "neighbour", "owner", "pointZones", "points"]);
        let get = |n: &str| files.iter().find(|(k, _)| k == n).map(|(_, t)| t.clone()).expect("file");
        assert!(get("owner").contains("    note        \"nPoints:12  nCells:2  nFaces:11  nInternalFaces:1\";\n    class       labelList;"));
        assert!(get("owner").contains("\n11\n(\n0\n0\n"), "{}", get("owner"));
        assert!(get("neighbour").contains("1(1)"));
        assert!(get("boundary").contains("        type            patch;\n        inGroups        1(wall);\n        nFaces          6;\n        startFace       1;"));
        assert!(get("boundary").ends_with("    }\n)\n\n// ************************************************************************* //\n"));
        assert!(get("cellZones").contains("    meta\n    {\n        names           1 ( zone0 );\n    }\n}"));
        assert!(get("cellZones").contains("(zone0\n{\n    type            cellZone;\n    cellLabels      List<label> 2(0 1);\n}\n)\n\n// ***"));
        assert!(get("faceZones").contains("\n0()\n\n// ***"));
        assert!(get("faces").contains("\n11\n(\n4(1 4 10 7)\n"));
    }

    #[test]
    fn a_mesh_remove_baffles_would_renumber_is_refused_by_name() {
        let mut m = tiny();
        m.owner.swap(0, 1); // owner not canonical for the internal face
        m.neighbour = vec![1, 1];
        assert!(matches!(m.check_unchanged_by_removal(), Err(PolyMeshError::NotCanonical(_))));
        let mut m = tiny();
        m.points.push([0.0, 0.0, 0.0]); // an unused point
        assert!(matches!(m.check_unchanged_by_removal(), Err(PolyMeshError::NotCanonical(_))));
        let mut m = tiny();
        m.cell_zones.push(("zone1".to_string(), vec![]));
        assert!(matches!(m.check_unchanged_by_removal(), Err(PolyMeshError::Refused(_))));
        let mut m = tiny();
        m.n_face_zones = 1;
        assert!(matches!(m.check_unchanged_by_removal(), Err(PolyMeshError::Refused(_))));
    }

    #[test]
    fn patch_types_and_groups_follow_the_copy_constructor() {
        let text = "FoamFile\n{\n}\n\n3\n(\n    a\n    {\n        type            patch;\n        nFaces          2;\n        startFace       4;\n    }\n    w\n    {\n        type            wall;\n        inGroups        List<word> 1(wall);\n        nFaces          2;\n        startFace       6;\n    }\n    e\n    {\n        type            empty;\n        nFaces          4;\n        startFace       8;\n    }\n)\n";
        let p = parse_boundary_patches(text).expect("types");
        assert_eq!(p[0].in_groups, Vec::<String>::new());
        assert_eq!(p[1].in_groups, vec!["wall".to_string()]);
        assert_eq!(p[2].in_groups, vec!["empty".to_string()]);
        assert_eq!((p[2].n_faces, p[2].start_face), (4, 8));
        let cyc = text.replace("type            empty;", "type            cyclic;");
        assert!(matches!(parse_boundary_patches(&cyc), Err(PolyMeshError::Refused(_))));
    }
}
