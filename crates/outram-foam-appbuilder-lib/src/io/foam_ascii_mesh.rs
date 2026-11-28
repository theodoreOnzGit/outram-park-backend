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
//! faces in canonical upper-triangular order ~~, every point used~~. A mesh failing it is refused by name
//! ([`PolyMeshError`]), as ~~are more than one cell zone (the multi-zone layout of `ZoneMesh::writeData` was never seen in a real file),
//! non-empty face or point zones and~~ are non-empty point zones and patch types other than `patch`, `wall`, `empty` and (since gh:#1236)
//! `mappedWall`. **CORRECTED 2026-11-28 (gh:#1938, gh:#1954, README round 84; checked against the real binary):** a mesh the port BUILDS
//! (`removeBaffles`, `genfoam::multi_region::baffle_merge`) is renumbered as `polyTopoChange::getFaceOrder` does, so it is canonical by
//! construction (the real binary renumbers an out-of-order fluid mesh back the same way, variant `swap`); an UNUSED point is not a refusal:
//! `polyTopoChange::compact` with `orderPoints = false` keeps every point that was not removed (`polyTopoChange.C:983-995`, variant `unused`:
//! the real copy keeps it); the 7 cell zones and 2 face zones of 3D_SmallESFR_old are written as `addZones` copies them (the face labels and
//! flip maps of the BAFFLED mesh, unrenumbered: labels up to 70 979 on a 60 354-face mesh). [`face_set_text`] writes `sets/duplicateFaces`.
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
    /// ~~Number of face zones and of point zones (both must be 0).~~ **CHANGED 2026-11-28 (gh:#1954, README round 84):** the face zones as
    /// `addZones` copies them from the region mesh (`meshHandler.C:507-525`: labels and flip map of the BAFFLED mesh, not renumbered).
    pub face_zones: Vec<FaceZoneOut>,
    /// Number of point zones (must be 0).
    pub n_point_zones: usize,
    /// The `duplicateFaces` `faceSet` of `findBaffles` in its `labelHashSet` iteration order ([`label_hash_set_order`]), when the mesh was BUILT
    /// by `removeBaffles` (gh:#1954, README round 84); written only into the start directory (`<start>/<region>.baffleLess/polyMesh/sets`).
    pub duplicate_face_set: Option<Vec<usize>>,
}

/// One face zone of the copy: `faceZone::writeDict` (`faceZone.C:694-704`: `type`, `faceLabels`, `flipMap`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FaceZoneOut {
    /// Zone name.
    pub name: String,
    /// `faceLabels`, as the region mesh's file holds them.
    pub labels: Vec<usize>,
    /// `flipMap`.
    pub flip_map: Vec<bool>,
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
        m.face_zones.clear();
        m.n_point_zones = 0;
        m
    }

    /// The `sets/duplicateFaces` file of the start directory (`findBaffles`, `mergeOrSplitBaffles.C:215-231`; gh:#1954, README round 84):
    /// `(path under <time>/<region>.baffleLess/polyMesh, text)`, `None` for a mesh that was not built by `removeBaffles`. `location` is the
    /// `polyMesh` directory (`200/fluidRegion.baffleLess/polyMesh`).
    #[must_use]
    pub fn duplicate_faces_file(&self, location: &str) -> Option<(String, String)> {
        let order = self.duplicate_face_set.as_ref()?;
        Some(("sets/duplicateFaces".to_string(), face_set_text(&format!("{location}/sets"), "duplicateFaces", order)))
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
        for face in &self.faces {
            for &pt in face {
                if pt >= self.points.len() {
                    return Err(PolyMeshError::NotCanonical("a face point label beyond the points".to_string()));
                }
            }
        }
        // ~~an unused point (polyTopoChange::compact would drop it)~~ CORRECTED 2026-11-28 (gh:#1938, README round 84): `compact` with
        // `orderPoints = false` keeps it (`polyTopoChange.C:983-995`; the real binary's copy keeps an appended unused point)
        if self.n_point_zones != 0 {
            return Err(PolyMeshError::Refused("a non-empty point zone".to_string()));
        }
        // ~~more than one cell zone / a face zone: refused~~ CHANGED 2026-11-28 (gh:#1954): written as the real binary writes them
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

/// `faceZones` (`ZoneMesh::writeData` with the `meta` names, then `faceZone::writeDict` per zone; gh:#1954, README round 84): the start form
/// `0()` when there is none.
fn face_zones_text(m: &PolyMeshOut, location: &str) -> String {
    if m.face_zones.is_empty() {
        return empty_zones_text(location, "faceZones");
    }
    let names: Vec<String> = m.face_zones.iter().map(|z| z.name.clone()).collect();
    let mut s = header_with_meta("regIOobject", location, "faceZones", &names);
    let _ = write!(s, "{}\n(", m.face_zones.len());
    for z in &m.face_zones {
        let labels = label_entries(&z.labels);
        let flips: Vec<String> = z.flip_map.iter().map(|b| if *b { "1".to_string() } else { "0".to_string() }).collect();
        let _ = write!(s, "{}\n{{\n    type            faceZone;\n    faceLabels      List<label> ", z.name);
        s.push_str(&write_list_text(&labels, true, all_equal(&labels), 10));
        s.push_str(";\n    flipMap         List<bool> ");
        s.push_str(&write_list_text(&flips, true, all_equal(&flips), 10));
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
        ("faceZones".to_string(), face_zones_text(m, location)),
        ("faces".to_string(), faces),
        ("neighbour".to_string(), list_file("labelList", "neighbour", &m.neighbour, true)),
        ("owner".to_string(), list_file("labelList", "owner", &m.owner, true)),
        ("pointZones".to_string(), empty_zones_text(location, "pointZones")),
        ("points".to_string(), write_list_file("vectorField", location, "points", &ListObject::Plain(Data::vector(&pts)), opts)),
    ])
}

/// The iteration order of an ESI `labelHashSet` built by inserting `inserts` in order into a table constructed with capacity request `initial`
/// (gh:#1954, README round 84). Upstream (OpenFOAM-ESI v2506): `HashTableCore::canonicalSize` (`HashTableCore.C:45-80`: 0 below 1, else the next
/// power of two, at least 8), `HashTable::setEntry` (`HashTable.C:380-410`: an empty table takes capacity 128; a new key goes to the HEAD of its
/// bucket; `0.8 capacity < size` doubles the capacity), `setCapacity` (`HashTable.C:659-720`: old buckets in order, each chain head to tail, every
/// entry moved to the head of its new bucket), `hashKeyIndex` (`Hash<int32_t>` is the value itself, `& (capacity - 1)`), and the const iterator
/// (buckets in order, each chain head to tail). A key already present is not inserted again.
#[must_use]
pub fn label_hash_set_order(initial: usize, inserts: &[usize]) -> Vec<usize> {
    let canonical = |req: usize| -> usize {
        if req < 1 {
            return 0;
        }
        let mut p = 8usize;
        while p < req {
            p <<= 1;
        }
        p
    };
    let mut capacity = canonical(initial);
    let mut table: Vec<Vec<usize>> = vec![Vec::new(); capacity];
    let mut size = 0usize;
    // a chain is stored head first: `chain[0]` is the head
    let rehash = |table: &Vec<Vec<usize>>, new_capacity: usize| -> Vec<Vec<usize>> {
        let mut out: Vec<Vec<usize>> = vec![Vec::new(); new_capacity];
        for chain in table {
            for &k in chain {
                out[k & (new_capacity - 1)].insert(0, k);
            }
        }
        out
    };
    for &key in inserts {
        if capacity == 0 {
            capacity = 128;
            table = vec![Vec::new(); capacity];
        }
        let idx = key & (capacity - 1);
        if table[idx].contains(&key) {
            continue;
        }
        table[idx].insert(0, key);
        size += 1;
        if 0.8 * (capacity as f64) < size as f64 {
            let new_capacity = canonical(2 * capacity);
            table = rehash(&table, new_capacity);
            capacity = new_capacity;
        }
    }
    table.into_iter().flatten().collect()
}

/// The `faceSet` file `topoSet::writeObject` writes (`sets/<name>`, class `faceSet`): the keys of the set in its iteration order
/// (`HashTable::writeKeys(os, 10)`, `HashTableIO.C`: `N(a b ...)` on one line for at most 10 keys, else a newline, `N`, `(`, one key per line,
/// `)`). `order` is [`label_hash_set_order`]'s (gh:#1954, README round 84: `<start>/<region>.baffleLess/polyMesh/sets/duplicateFaces`).
#[must_use]
pub fn face_set_text(location: &str, object: &str, order: &[usize]) -> String {
    let mut s = header_text("faceSet", location, object);
    if order.len() <= 10 {
        let _ = write!(s, "{}(", order.len());
        for (k, v) in order.iter().enumerate() {
            if k > 0 {
                s.push(' ');
            }
            let _ = write!(s, "{v}");
        }
        s.push(')');
    } else {
        let _ = write!(s, "\n{}\n(\n", order.len());
        for v in order {
            let _ = writeln!(s, "{v}");
        }
        s.push_str(")\n");
    }
    s.push_str(footer());
    s
}

/// The tokens of a zone file after its `FoamFile` header: words and numbers, and `(` `)` `{` `}` `;` each on its own (`//` and `/* */` comments
/// dropped).
fn zone_tokens(text: &str) -> Vec<String> {
    // drop the header block `FoamFile { ... }`
    let body = match text.find("FoamFile") {
        Some(at) => match text[at..].find('}') {
            Some(close) => &text[at + close + 1..],
            None => "",
        },
        None => text,
    };
    let mut out = Vec::new();
    let mut cur = String::new();
    let chars: Vec<char> = body.chars().collect();
    let mut i = 0;
    let flush = |cur: &mut String, out: &mut Vec<String>| {
        if !cur.is_empty() {
            out.push(core::mem::take(cur));
        }
    };
    while i < chars.len() {
        let c = chars[i];
        if c == '/' && i + 1 < chars.len() && chars[i + 1] == '/' {
            flush(&mut cur, &mut out);
            while i < chars.len() && chars[i] != '\n' {
                i += 1;
            }
            continue;
        }
        if c == '/' && i + 1 < chars.len() && chars[i + 1] == '*' {
            flush(&mut cur, &mut out);
            i += 2;
            while i + 1 < chars.len() && !(chars[i] == '*' && chars[i + 1] == '/') {
                i += 1;
            }
            i += 2;
            continue;
        }
        if c.is_whitespace() {
            flush(&mut cur, &mut out);
        } else if matches!(c, '(' | ')' | '{' | '}' | ';') {
            flush(&mut cur, &mut out);
            out.push(c.to_string());
        } else {
            cur.push(c);
        }
        i += 1;
    }
    flush(&mut cur, &mut out);
    out
}

/// One list value at `tokens[*at]`: an optional `List<T>` word, then `N(` entries `)` or `N{v}`; the entries as text.
fn zone_list(tokens: &[String], at: &mut usize) -> Result<Vec<String>, PolyMeshError> {
    let bad = |m: &str| PolyMeshError::Refused(format!("faceZones file: {m}"));
    if tokens.get(*at).is_some_and(|t| t.starts_with("List<")) {
        *at += 1;
    }
    let n = match tokens.get(*at).map(|t| t.parse::<usize>()) {
        Some(Ok(n)) => n,
        _ => return Err(bad("a list without a size")),
    };
    *at += 1;
    match tokens.get(*at).map(String::as_str) {
        Some("(") => {
            *at += 1;
            let mut v = Vec::with_capacity(n);
            while tokens.get(*at).is_some_and(|t| t != ")") {
                v.push(tokens[*at].clone());
                *at += 1;
            }
            *at += 1;
            if v.len() != n {
                return Err(bad("a list whose size is not its count"));
            }
            Ok(v)
        }
        Some("{") => {
            let Some(val) = tokens.get(*at + 1).cloned() else { return Err(bad("an unclosed uniform list")) };
            if tokens.get(*at + 2).map(String::as_str) != Some("}") {
                return Err(bad("an unclosed uniform list"));
            }
            *at += 3;
            Ok(vec![val; n])
        }
        _ => Err(bad("a list without its opening bracket")),
    }
}

/// The face zones of a region mesh's `faceZones` file (`N ( name { type faceZone; faceLabels <list>; flipMap <list>; } ... )`, the lists as
/// `N(...)`, `N{v}` or multi-line, `List<label>`/`List<bool>` optional): what `addZones` copies into the baffle-less mesh (gh:#1954, README
/// round 84). `flipMap` entries are `0`/`1` (or `false`/`true`).
///
/// # Errors
/// [`PolyMeshError::Refused`] for a file that does not parse, a zone type other than `faceZone`, or a flip map of another length.
pub fn parse_face_zones_out(text: &str) -> Result<Vec<FaceZoneOut>, PolyMeshError> {
    let bad = |m: String| PolyMeshError::Refused(format!("faceZones file: {m}"));
    let tok = zone_tokens(text);
    let mut at = 0;
    let n = match tok.first().map(|t| t.parse::<usize>()) {
        Some(Ok(n)) => n,
        _ => return Err(bad("no zone count".to_string())),
    };
    at += 1;
    if tok.get(at).map(String::as_str) != Some("(") {
        return Err(bad("no zone list".to_string()));
    }
    at += 1;
    let mut out = Vec::with_capacity(n);
    for _ in 0..n {
        let Some(name) = tok.get(at).cloned() else { return Err(bad("a missing zone".to_string())) };
        at += 1;
        if tok.get(at).map(String::as_str) != Some("{") {
            return Err(bad(format!("zone {name} has no block")));
        }
        at += 1;
        let (mut labels, mut flips, mut ztype): (Vec<usize>, Vec<bool>, String) = (Vec::new(), Vec::new(), String::new());
        while tok.get(at).is_some_and(|t| t != "}") {
            let key = tok[at].clone();
            at += 1;
            match key.as_str() {
                "type" => {
                    ztype = tok.get(at).cloned().unwrap_or_default();
                    at += 1;
                }
                "faceLabels" => {
                    let mut v = Vec::new();
                    for e in zone_list(&tok, &mut at)? {
                        match e.parse::<usize>() {
                            Ok(x) => v.push(x),
                            Err(_) => return Err(bad(format!("zone {name}: a face label {e}"))),
                        }
                    }
                    labels = v;
                }
                "flipMap" => {
                    let mut v = Vec::new();
                    for e in zone_list(&tok, &mut at)? {
                        match e.as_str() {
                            "0" | "false" => v.push(false),
                            "1" | "true" => v.push(true),
                            other => return Err(bad(format!("zone {name}: a flip {other}"))),
                        }
                    }
                    flips = v;
                }
                _ => {
                    // an entry the copy does not carry (e.g. `inGroups`): refused, the writer would drop it
                    return Err(bad(format!("zone {name}: the entry {key}")));
                }
            }
            if tok.get(at).map(String::as_str) == Some(";") {
                at += 1;
            }
        }
        at += 1;
        if ztype != "faceZone" {
            return Err(bad(format!("zone {name}: type {ztype}")));
        }
        if flips.len() != labels.len() {
            return Err(bad(format!("zone {name}: {} flips for {} faces", flips.len(), labels.len())));
        }
        out.push(FaceZoneOut { name, labels, flip_map: flips });
    }
    Ok(out)
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
            face_zones: Vec::new(),
            n_point_zones: 0,
            duplicate_face_set: None,
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
        // ~~an unused point is refused~~ CORRECTED 2026-11-28 (gh:#1938, README round 84): `compact` keeps it, so does the copy
        let mut m = tiny();
        m.points.push([0.0, 0.0, 0.0]);
        assert!(m.check_unchanged_by_removal().is_ok());
        // ~~two cell zones or a face zone are refused~~ CHANGED 2026-11-28 (gh:#1954): written; a point zone is still refused
        let mut m = tiny();
        m.cell_zones.push(("zone1".to_string(), vec![]));
        m.face_zones.push(FaceZoneOut { name: "f".to_string(), labels: vec![3], flip_map: vec![true] });
        assert!(m.check_unchanged_by_removal().is_ok());
        let mut m = tiny();
        m.n_point_zones = 1;
        assert!(matches!(m.check_unchanged_by_removal(), Err(PolyMeshError::Refused(_))));
    }

    /// gh:#1954 (README round 84): the multi-zone layout the real `3D_SmallESFR_old` `200/fluidRegion.baffleLess/polyMesh` shows (`(` then the
    /// first name, `}` then the next name, `)` after the last `}`), a face zone's two lists, and a parse of the shipped (OpenFOAM 5.x) layout.
    #[test]
    fn zones_are_written_in_the_real_multi_zone_layout_and_parsed_back() {
        let mut m = tiny();
        m.cell_zones.push(("zone1".to_string(), vec![]));
        m.face_zones = vec![
            FaceZoneOut { name: "walls".to_string(), labels: vec![], flip_map: vec![] },
            FaceZoneOut { name: "b".to_string(), labels: (20..32).collect(), flip_map: (0..12).map(|k| k >= 8).collect() },
        ];
        let files = write_poly_mesh(&m, "200/r.baffleLess/polyMesh", &WriteOptions::precision_17()).expect("zones");
        let get = |n: &str| files.iter().find(|(k, _)| k == n).map(|(_, t)| t.clone()).expect("file");
        assert!(get("cellZones").contains("    cellLabels      List<label> 2(0 1);\n}\nzone1\n{\n    type            cellZone;\n    cellLabels      List<label> 0();\n}\n)\n"));
        let fz = get("faceZones");
        assert!(fz.contains("        names           2 ( walls b );\n"));
        assert!(fz.contains("2\n(walls\n{\n    type            faceZone;\n    faceLabels      List<label> 0();\n    flipMap         List<bool> 0();\n}\nb\n{"));
        assert!(fz.contains("    flipMap         List<bool> \n12\n(\n0\n"));
        assert!(fz.contains("1\n1\n)\n;\n}\n)\n\n// ***"));
        let shipped = "FoamFile\n{\n    class regIOobject;\n}\n// * * //\n\n2\n(\nwalls\n{\n    type faceZone;\nfaceLabels      0();\nflipMap         0();\n}\n\nb\n{\n    type faceZone;\nfaceLabels      List<label> \n3\n(\n7\n8\n9\n)\n;\nflipMap         List<bool> 3{1};\n}\n)\n";
        let z = parse_face_zones_out(shipped).expect("shipped layout");
        assert_eq!(z.len(), 2);
        assert_eq!((z[0].labels.len(), z[1].labels.clone(), z[1].flip_map.clone()), (0, vec![7, 8, 9], vec![true, true, true]));
        assert!(parse_face_zones_out(&shipped.replace("3{1}", "2{1}")).is_err());
    }

    /// gh:#1954 (README round 84): `labelHashSet` iteration order. Keys of one consecutive range in a table larger than the range come out in
    /// bucket order (`key & (capacity - 1)`): the real ESFR `duplicateFaces` starts at 65536 and ends at 65535 (keys 49728..70979, capacity 32768).
    #[test]
    fn a_label_hash_set_iterates_in_bucket_order_and_is_written_as_a_face_set() {
        let keys: Vec<usize> = (49_728..70_980).collect();
        let order = label_hash_set_order(99, &keys);
        assert_eq!(order.len(), 21_252);
        assert_eq!((order[0], order[5443], order[5444], order[21_251]), (65_536, 70_979, 49_728, 65_535));
        // a colliding pair: 3 and 131 share bucket 3 of the first 128 buckets; 131 is inserted at the head, a repeated 3 is not inserted
        let small = label_hash_set_order(0, &[3, 131, 3]);
        assert_eq!(small, vec![131, 3]);
        let txt = face_set_text("0/r.baffleLess/polyMesh/sets", "duplicateFaces", &[]);
        assert!(txt.contains("    class       faceSet;\n    location    \"0/r.baffleLess/polyMesh/sets\";\n    object      duplicateFaces;\n"));
        assert!(txt.ends_with("//\n\n0()\n\n// ************************************************************************* //\n"));
        let long = face_set_text("x", "d", &order);
        assert!(long.contains("//\n\n\n21252\n(\n65536\n"));
        assert!(long.ends_with("65535\n)\n\n\n// ************************************************************************* //\n"));
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
