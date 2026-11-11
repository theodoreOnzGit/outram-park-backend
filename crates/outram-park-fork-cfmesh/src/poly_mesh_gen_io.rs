// SPDX-License-Identifier: GPL-3.0-only
// Copyright (C) 2026 OUTRAM PARK contributors
//
// Ported from cfMesh (Creative Fields, Ltd.; Franjo Juretic), GPL-3.0:
//   meshLibrary/utilities/meshes/polyMeshGen/polyMeshGen.C (write, read),
//   polyMeshGenPoints.C, polyMeshGenFaces.C, polyMeshGenCells.C (write and
//   read with the point, face and cell subsets as pointSet / faceSet /
//   cellSet files)
//   meshLibrary/utilities/workflowControls/workflowControls.C (the
//   meshMetaDict entries lastStep / completedSteps)
// Source: the cfMesh v1.1 line as carried by github.com/DAFoam/cfmesh, commit
// e944f6ba695fcefce73ad306074f6b3c0ea432ae.
// Copyright (C) Creative Fields, Ltd. Licence: GPL-3.0-or-later (cfMesh files
// say "either version 3 of the License, or (at your option) any later
// version"), conveyed here under GPL-3.0-only.
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

//! `polyMeshGen::write` and `read` (gh:#1476, feature `foam-export`): a
//! generator's mesh written to `constant/polyMesh` as the cfMesh
//! executables write it, and read back for `restartFromLatestStep`.
//!
//! `write` removes the old mesh files (`points` … `meshMetaDict`, and the
//! `sets` folder), writes `points`, `faces`, `owner`, `neighbour` through
//! `outram_foam_basic_lib::io::PolyMesh`, then `boundary` with each patch's
//! **type as the generator reports it** (`PolyMesh`'s `PatchKind` would
//! turn `symmetryPlane` into `symmetry`), one `pointSet` / `faceSet` /
//! `cellSet` file per subset under `sets/`, and `meshMetaDict`
//! (`surfaceFile`, `lastStep`, `completedSteps`).
//!
//! Differences from upstream, all in what is written, not in the mesh:
//! a set's labels are written ascending (upstream writes a `labelHashSet` in
//! hash order); `surfaceMeta` (`triSurfaceMetaData`) is not written; the
//! numbers are written by `outram-foam-basic-lib`'s formatter (full
//! precision; upstream uses `controlDict`'s `writePrecision`).

use crate::foam::to_poly_mesh;
use crate::mesh_subsets::MeshSubsets;
use crate::volume_mesh::{BoundaryPatch, VolumeMesh};
use crate::workflow_controls::{MeshMetaData, WorkflowStep};
use outram_foam_basic_lib::io::dict::{tokenize, FoamDict, FoamEntry, FoamFile, FoamHeader, FoamValue};
use outram_foam_basic_lib::io::poly_mesh::PolyMesh;
use std::path::Path;

/// One named subset to write: its labels (point, face or cell indices).
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct NamedSets {
    /// `pointSubsets_`: (name, labels).
    pub points: Vec<(String, Vec<usize>)>,
    /// `faceSubsets_`.
    pub faces: Vec<(String, Vec<usize>)>,
    /// `cellSubsets_`.
    pub cells: Vec<(String, Vec<usize>)>,
}

impl NamedSets {
    /// The subsets of a [`MeshSubsets`], by kind.
    pub fn from_mesh_subsets(s: &MeshSubsets) -> Self {
        let take = |g: &crate::mesh_subsets::SubsetGroup| g.indices().into_iter().map(|id| (g.name(id).unwrap_or("").to_string(), g.elements(id))).collect();
        NamedSets { points: take(&s.points), faces: take(&s.faces), cells: take(&s.cells) }
    }
}

/// What a cfMesh run leaves beside the mesh.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct PolyMeshGenMeta {
    /// `surfaceFile`, as the `meshDict` gives it.
    pub surface_file: Option<String>,
    /// The workflow's `lastStep` / `completedSteps`.
    pub workflow: MeshMetaData,
}

const OLD_FILES: [&str; 12] = ["points", "faces", "owner", "neighbour", "cells", "boundary", "pointZones", "faceZones", "cellZones", "meshModifiers", "parallelData", "meshMetaDict"];

fn io_err(p: &Path, e: std::io::Error) -> String {
    format!("{}: {e}", p.display())
}

/// `polyMeshGen::write` into `dir` (normally `<case>/constant/polyMesh`).
///
/// # Errors
/// A file that cannot be removed or written.
pub fn write_poly_mesh_gen(mesh: &VolumeMesh, patch_types: &[String], sets: &NamedSets, meta: &PolyMeshGenMeta, dir: &Path) -> Result<(), String> {
    for f in OLD_FILES {
        let p = dir.join(f);
        if p.is_file() {
            std::fs::remove_file(&p).map_err(|e| io_err(&p, e))?;
        }
    }
    let sets_dir = dir.join("sets");
    if sets_dir.is_dir() {
        std::fs::remove_dir_all(&sets_dir).map_err(|e| io_err(&sets_dir, e))?;
    }
    to_poly_mesh(mesh).write(dir).map_err(|e| e.to_string())?;
    let b = dir.join("boundary");
    std::fs::write(&b, boundary_string(&mesh.patches, patch_types)).map_err(|e| io_err(&b, e))?;
    for (class, list) in [("pointSet", &sets.points), ("faceSet", &sets.faces), ("cellSet", &sets.cells)] {
        for (name, labels) in list {
            std::fs::create_dir_all(&sets_dir).map_err(|e| io_err(&sets_dir, e))?;
            let p = sets_dir.join(name);
            std::fs::write(&p, set_string(class, name, labels)).map_err(|e| io_err(&p, e))?;
        }
    }
    let m = dir.join("meshMetaDict");
    std::fs::write(&m, meta_string(meta)).map_err(|e| io_err(&m, e))
}

fn header(class: &str, location: &str, object: &str) -> String {
    format!(
        "/*--------------------------------*- C++ -*----------------------------------*\\\n  OUTRAM PARK cfMesh port (outram-park-fork-cfmesh)\n\\*---------------------------------------------------------------------------*/\nFoamFile\n{{\n    version     2.0;\n    format      ascii;\n    class       {class};\n    location    \"{location}\";\n    object      {object};\n}}\n// * * * * * * * * * * * * * * * * * * * * * * * * * * * * * * * * * * * * * //\n\n"
    )
}

/// The `boundary` file: each patch's `type`, `nFaces`, `startFace`; an
/// empty type is written as `patch`.
fn boundary_string(patches: &[BoundaryPatch], types: &[String]) -> String {
    let mut s = header("polyBoundaryMesh", "constant/polyMesh", "boundary");
    s.push_str(&format!("{}\n(\n", patches.len()));
    for (i, p) in patches.iter().enumerate() {
        let t = types.get(i).map(String::as_str).filter(|t| !t.is_empty()).unwrap_or("patch");
        s.push_str(&format!("    {}\n    {{\n        type            {t};\n        nFaces          {};\n        startFace       {};\n    }}\n", p.name, p.n_faces, p.start_face));
    }
    s.push_str(")\n\n// ************************************************************************* //\n");
    s
}

fn set_string(class: &str, name: &str, labels: &[usize]) -> String {
    let mut l = labels.to_vec();
    l.sort_unstable();
    l.dedup();
    let mut s = header(class, "constant/polyMesh/sets", name);
    s.push_str(&format!("{}\n(\n", l.len()));
    for x in l {
        s.push_str(&format!("{x}\n"));
    }
    s.push_str(")\n\n// ************************************************************************* //\n");
    s
}

fn meta_dict(meta: &PolyMeshGenMeta) -> FoamDict {
    let mut d = FoamDict::new();
    if let Some(f) = &meta.surface_file {
        d.insert("surfaceFile", FoamEntry::Str(f.clone()));
    }
    if let Some(l) = meta.workflow.last_step {
        d.insert("lastStep", FoamEntry::Word(l.name().into()));
    }
    if !meta.workflow.completed_steps.is_empty() {
        d.insert("completedSteps", FoamEntry::List(meta.workflow.completed_steps.iter().map(|s| FoamValue::Word(s.name().into())).collect()));
    }
    d
}

fn meta_string(meta: &PolyMeshGenMeta) -> String {
    let mut h = FoamHeader::standard("dictionary", "meshMetaDict");
    h.set("location", "\"constant/polyMesh\"");
    FoamFile { header: Some(h), dict: meta_dict(meta) }.to_foam_string()
}

/// A mesh read back by [`read_poly_mesh_gen`].
#[derive(Debug, Clone)]
pub struct ReadPolyMeshGen {
    /// The mesh.
    pub mesh: VolumeMesh,
    /// Each patch's `type`, as the `boundary` file gives it.
    pub patch_types: Vec<String>,
    /// The subsets under `sets/` (`pointSet`, `faceSet`, `cellSet`).
    pub sets: NamedSets,
    /// `meshMetaDict` (empty if the file is absent).
    pub meta: PolyMeshGenMeta,
}

/// `polyMeshGen::read` from `dir`, with `meshMetaDict`: what
/// `restartFromLatestStep` loads.
///
/// # Errors
/// A missing or malformed `polyMesh` file.
pub fn read_poly_mesh_gen(dir: &Path) -> Result<ReadPolyMeshGen, String> {
    let pm = PolyMesh::read(dir).map_err(|e| e.to_string())?;
    let mesh = VolumeMesh {
        points: pm.points.iter().map(|p| crate::math::Vec3::new(p.x, p.y, p.z)).collect(),
        faces: pm.faces.iter().map(|f| f.verts.clone()).collect(),
        owner: pm.faces.iter().map(|f| f.owner).collect(),
        neighbour: pm.faces.iter().map(|f| f.neighbour).collect(),
        n_cells: pm.n_cells,
        patches: pm.patches.iter().map(|p| BoundaryPatch { name: p.name.clone(), start_face: p.start, n_faces: p.size }).collect(),
        cells: None,
    };
    let b = dir.join("boundary");
    let text = std::fs::read_to_string(&b).map_err(|e| io_err(&b, e))?;
    let patch_types = boundary_types(&text);
    let mut sets = NamedSets::default();
    let sets_dir = dir.join("sets");
    if sets_dir.is_dir() {
        let mut entries: Vec<_> = std::fs::read_dir(&sets_dir).map_err(|e| io_err(&sets_dir, e))?.filter_map(|e| e.ok().map(|e| e.path())).collect();
        entries.sort();
        for p in entries {
            let text = std::fs::read_to_string(&p).map_err(|e| io_err(&p, e))?;
            let (class, labels) = read_set(&text).map_err(|e| format!("{}: {e}", p.display()))?;
            let name = p.file_name().map(|n| n.to_string_lossy().to_string()).unwrap_or_default();
            match class.as_str() {
                "pointSet" => sets.points.push((name, labels)),
                "faceSet" => sets.faces.push((name, labels)),
                "cellSet" => sets.cells.push((name, labels)),
                _ => {}
            }
        }
    }
    let mut meta = PolyMeshGenMeta::default();
    let m = dir.join("meshMetaDict");
    if m.is_file() {
        let d = FoamFile::read(&m).map_err(|e| e.to_string())?.dict;
        meta.surface_file = match d.get("surfaceFile") {
            Some(FoamEntry::Str(s)) | Some(FoamEntry::Word(s)) => Some(s.clone()),
            _ => None,
        };
        meta.workflow.last_step = d.get_word("lastStep").and_then(WorkflowStep::from_name);
        if let Some(FoamEntry::List(l)) = d.get("completedSteps") {
            meta.workflow.completed_steps = l.iter().filter_map(|v| v.as_word().and_then(WorkflowStep::from_name)).collect();
        }
    }
    Ok(ReadPolyMeshGen { mesh, patch_types, sets, meta })
}

/// The index after a leading `FoamFile { … }` header, and its `class`.
fn skip_header(t: &[outram_foam_basic_lib::io::dict::Token]) -> (usize, String) {
    if t.first().map(|x| x.text.as_str()) != Some("FoamFile") {
        return (0, String::new());
    }
    let mut class = String::new();
    let mut i = 1;
    while i < t.len() && t[i].text != "}" {
        if t[i].text == "class" && i + 1 < t.len() {
            class = t[i + 1].text.clone();
        }
        i += 1;
    }
    (i + 1, class)
}

/// The `type` of each patch in a `boundary` file, in order.
fn boundary_types(text: &str) -> Vec<String> {
    let t = tokenize(text);
    let (start, _) = skip_header(&t);
    let mut out = Vec::new();
    let mut depth = 0usize;
    for i in start..t.len() {
        match t[i].text.as_str() {
            "{" => {
                if depth == 0 {
                    out.push("patch".to_string());
                }
                depth += 1;
            }
            "}" => depth = depth.saturating_sub(1),
            "type" if depth == 1 && i + 1 < t.len() => {
                if let Some(last) = out.last_mut() {
                    *last = t[i + 1].text.clone();
                }
            }
            _ => {}
        }
    }
    out
}

/// A `topoSet` file: its class and labels (`N ( labels )`).
fn read_set(text: &str) -> Result<(String, Vec<usize>), String> {
    let t = tokenize(text);
    let (start, class) = skip_header(&t);
    let open = t[start..].iter().position(|x| x.text == "(").ok_or("no label list")? + start;
    let close = t[open..].iter().position(|x| x.text == ")").ok_or("unterminated label list")? + open;
    let labels = t[open + 1..close].iter().map(|x| x.text.parse::<usize>().map_err(|_| format!("bad label {}", x.text))).collect::<Result<Vec<_>, _>>()?;
    Ok((class, labels))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::cartesian::cartesian_box;
    use crate::math::Vec3;

    /// Write a mesh with types, subsets and meta data, read it back:
    /// geometry, patch types (`symmetryPlane` kept), sets and the workflow
    /// steps survive; a second write removes the first's sets.
    #[test]
    fn write_read_round_trip_with_subsets_and_meta() {
        let dir = std::env::temp_dir().join(format!("cfmesh_pmg_rt_{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        let m = cartesian_box(Vec3::ZERO, Vec3::new(1.0, 1.0, 1.0), [2, 2, 2]);
        let types: Vec<String> = (0..m.patches.len()).map(|i| if i == 0 { "symmetryPlane".to_string() } else { "wall".to_string() }).collect();
        let sets = NamedSets { points: vec![("p".into(), vec![3, 1])], faces: vec![], cells: vec![("c".into(), vec![0, 7])] };
        let meta = PolyMeshGenMeta {
            surface_file: Some("geom.stl".into()),
            workflow: MeshMetaData { last_step: Some(WorkflowStep::EdgeExtraction), completed_steps: vec![WorkflowStep::TemplateGeneration, WorkflowStep::EdgeExtraction] },
        };
        write_poly_mesh_gen(&m, &types, &sets, &meta, &dir).unwrap();
        let r = read_poly_mesh_gen(&dir).unwrap();
        assert_eq!((&r.mesh.points, &r.mesh.faces, &r.mesh.owner, &r.mesh.neighbour, r.mesh.n_cells, &r.mesh.patches), (&m.points, &m.faces, &m.owner, &m.neighbour, m.n_cells, &m.patches));
        assert_eq!(r.patch_types, types);
        assert_eq!(r.sets.points, vec![("p".to_string(), vec![1, 3])]);
        assert_eq!(r.sets.cells, vec![("c".to_string(), vec![0, 7])]);
        assert_eq!(r.meta, meta);
        write_poly_mesh_gen(&m, &types, &NamedSets::default(), &PolyMeshGenMeta::default(), &dir).unwrap();
        let r2 = read_poly_mesh_gen(&dir).unwrap();
        assert!(r2.sets.points.is_empty() && r2.sets.cells.is_empty());
        assert_eq!(r2.meta, PolyMeshGenMeta::default());
        let _ = std::fs::remove_dir_all(&dir);
        let s = crate::mesh_subsets::MeshSubsets::default();
        assert_eq!(NamedSets::from_mesh_subsets(&s), NamedSets::default());
    }
}
