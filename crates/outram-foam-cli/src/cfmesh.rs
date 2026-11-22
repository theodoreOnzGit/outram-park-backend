// SPDX-License-Identifier: GPL-3.0-only
// Copyright (C) 2026 OUTRAM PARK contributors
//
// Mirrors cfMesh (Creative Fields, Ltd.; Franjo Juretic), GPL-3.0:
//   executables/{cartesianMesh,tetMesh,pMesh,cartesian2DMesh}/*.C and the
//   generators' constructors (meshLibrary/{cartesianMesh,tetMesh,voronoiMesh,
//   cartesian2DMesh}/*Generator.C: checkMeshDict, triSurf(surfaceFile),
//   meshOctreeCreator, generateMesh, writeMesh)
// Source: github.com/DAFoam/cfmesh, commit e944f6ba695fcefce73ad306074f6b3c0ea432ae.
// Copyright (C) Creative Fields, Ltd. Licence: GPL-3.0-or-later, conveyed
// here under GPL-3.0-only.
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

//! The cfMesh executables (`cartesianMesh`, `tetMesh`, `pMesh`,
//! `cartesian2DMesh`, gh:#1476): read `<case>/system/meshDict` and its
//! `surfaceFile`, run the matching `outram_park_fork_cfmesh::pipeline`
//! workflow, and write `<case>/constant/polyMesh`, as the upstream
//! executables do.
//!
//! The steps follow the generators' constructors: `checkMeshDict`, the
//! surface (`triSurf(surfaceFile)`; a surface with **stored feature edges**
//! is refused, because `triSurfacePatchManipulator` is not ported and the
//! mesh would silently differ), the workflow (`restartFromLatestStep` loads
//! the mesh a `stopAfter` run wrote), `renameBoundary` (inside the
//! workflow), and `polyMeshGen::write`.
//!
//! Every `meshDict` entry is printed with what the port does with it
//! (`outram_park_fork_cfmesh::mesh_dict::KeywordStatus`); an entry no
//! upstream code reads is an error, as a typo should be.

use crate::CliError;
use outram_foam_basic_lib::io::dict::FoamFile;
use outram_park_fork_cfmesh::mesh_dict::{check_mesh_dict, read_mesh_dict, Generator, GeneratorOptions, KeywordStatus};
use outram_park_fork_cfmesh::pipeline::{cartesian_2d_mesh, cartesian_mesh, p_mesh, tet_mesh};
use outram_park_fork_cfmesh::poly_mesh_gen_io::{read_poly_mesh_gen, write_poly_mesh_gen, NamedSets, PolyMeshGenMeta};
use outram_park_fork_cfmesh::tri_surf_io::read_surface_bytes;
use outram_park_fork_cfmesh::volume_mesh::VolumeMesh;
use outram_park_fork_cfmesh::workflow_controls::{MeshMetaData, WorkflowStep};
use std::path::Path;
use std::time::Instant;

/// What one run produced.
#[derive(Debug, Clone)]
pub struct CfMeshRun {
    /// The mesh written.
    pub mesh: VolumeMesh,
    /// Its patch types.
    pub patch_types: Vec<String>,
    /// The step the workflow stopped after (`stopAfter`), if any.
    pub stopped_after: Option<WorkflowStep>,
    /// The generator's report lines and the reader's notes.
    pub notes: Vec<String>,
    /// The `meshDict` entries the port read but does not act on.
    pub reported: Vec<String>,
    /// Wall time of the meshing (reading and writing excluded) [s].
    pub seconds: f64,
}

fn read_surface(case: &Path, file: &str) -> Result<outram_park_fork_cfmesh::tri_surf_io::SurfaceRead, String> {
    let p = case.join(file);
    let bytes = std::fs::read(&p).map_err(|e| format!("surfaceFile {}: {e}", p.display()))?;
    read_surface_bytes(file, &bytes)
}

/// Run `generator` on the case in `case` and write its `constant/polyMesh`.
///
/// # Errors
/// - [`CliError::Io`]: `meshDict`, a surface or the mesh cannot be read or
///   written.
/// - [`CliError::Tool`]: `checkMeshDict` stops, an entry no cfMesh code
///   reads, a surface with feature edges, or the workflow fails.
pub fn run_cfmesh(case: &Path, generator: Generator) -> Result<CfMeshRun, CliError> {
    let dict_path = case.join("system").join("meshDict");
    let dict = FoamFile::read(&dict_path).map_err(|e| CliError::Io(e.to_string()))?.dict;
    let warnings = check_mesh_dict(&dict, |f| case.join(f).is_file()).map_err(|e| CliError::Tool(format!("checkMeshDict: {e}")))?;
    for w in &warnings {
        eprintln!("--> checkMeshDict warning: {w}");
    }
    let reading = read_mesh_dict(&dict, generator).map_err(CliError::Tool)?;
    let mut reported = Vec::new();
    for k in &reading.keywords {
        match &k.status {
            KeywordStatus::Honoured => println!("meshDict  {:<56} honoured", k.path),
            KeywordStatus::Reported(why) => {
                println!("meshDict  {:<56} REPORTED: {why}", k.path);
                reported.push(format!("{}: {why}", k.path));
            }
            KeywordStatus::NotApplicable(why) => println!("meshDict  {:<56} not applicable: {why}", k.path),
            KeywordStatus::Unknown => {}
        }
    }
    let unknown: Vec<String> = reading.unknown().iter().map(|k| k.path.clone()).collect();
    if !unknown.is_empty() {
        return Err(CliError::Tool(format!("meshDict entries no cfMesh code reads: {unknown:?}")));
    }
    for w in &reading.warnings {
        eprintln!("--> warning: {w}");
    }
    let surf = read_surface(case, &reading.surface_file).map_err(CliError::Io)?;
    for w in &surf.warnings {
        eprintln!("--> triSurf warning: {w}");
    }
    let s = surf.surface;
    if !s.feature_edges.is_empty() {
        return Err(CliError::Tool(format!(
            "surface {} stores {} feature edges: upstream re-patches it with triSurfacePatchManipulator, which is not ported; refused rather than meshing a different surface",
            reading.surface_file,
            s.feature_edges.len()
        )));
    }
    println!("surface {}: {} points, {} triangles, {} patches", reading.surface_file, s.points.len(), s.triangles.len(), s.patches.len());
    let (opts, mut notes) = reading.resolve(&s, |f| read_surface(case, f).map(|r| r.surface)).map_err(CliError::Tool)?;
    let regions = s.surface_regions();
    let poly_dir = case.join("constant").join("polyMesh");
    let saved = if reading.workflow.restart_from_latest_step && poly_dir.join("meshMetaDict").is_file() {
        let r = read_poly_mesh_gen(&poly_dir).map_err(CliError::Io)?;
        notes.push(format!("restartFromLatestStep: loaded constant/polyMesh (lastStep {:?})", r.meta.workflow.last_step.map(|x| x.name())));
        Some(outram_park_fork_cfmesh::pipeline::tet_mesh::SavedMesh { mesh: r.mesh, patch_types: r.patch_types, meta: r.meta.workflow })
    } else {
        None
    };
    let t0 = Instant::now();
    let (pts, tris, reg) = (&s.points[..], &s.triangles[..], Some(&regions));
    let (mesh, patch_types, stopped_after, meta, gen_notes): (VolumeMesh, Vec<String>, Option<WorkflowStep>, MeshMetaData, Vec<String>) = match (&opts, &saved) {
        (GeneratorOptions::CartesianMesh(o), None) => {
            let (m, r) = cartesian_mesh::cartesian_mesh(pts, tris, reg, o).map_err(CliError::Tool)?;
            (m, r.patch_types, r.stopped_after, r.meta, r.notes)
        }
        (GeneratorOptions::CartesianMesh(o), Some(sv)) => {
            let (m, r) = cartesian_mesh::cartesian_mesh_restart(pts, tris, reg, o, sv).map_err(CliError::Tool)?;
            (m, r.patch_types, r.stopped_after, r.meta, r.notes)
        }
        (GeneratorOptions::TetMesh(o), None) => {
            let (m, r) = tet_mesh::tet_mesh(pts, tris, reg, o).map_err(CliError::Tool)?;
            (m, r.patch_types, r.stopped_after, r.meta, r.notes)
        }
        (GeneratorOptions::TetMesh(o), Some(sv)) => {
            let (m, r) = tet_mesh::tet_mesh_restart(pts, tris, reg, o, sv).map_err(CliError::Tool)?;
            (m, r.patch_types, r.stopped_after, r.meta, r.notes)
        }
        (GeneratorOptions::PMesh(o), None) => {
            let (m, r) = p_mesh::p_mesh(pts, tris, reg, o).map_err(CliError::Tool)?;
            (m, r.patch_types, r.stopped_after, r.meta, r.notes)
        }
        (GeneratorOptions::PMesh(o), Some(sv)) => {
            let (m, r) = p_mesh::p_mesh_restart(pts, tris, reg, o, sv).map_err(CliError::Tool)?;
            (m, r.patch_types, r.stopped_after, r.meta, r.notes)
        }
        (GeneratorOptions::Cartesian2DMesh(o), None) => {
            let (m, r) = cartesian_2d_mesh::cartesian_2d_mesh(pts, tris, reg, o).map_err(CliError::Tool)?;
            (m, r.patch_types, r.stopped_after, r.meta, r.notes)
        }
        (GeneratorOptions::Cartesian2DMesh(o), Some(sv)) => {
            let (m, r) = cartesian_2d_mesh::cartesian_2d_mesh_restart(pts, tris, reg, o, sv).map_err(CliError::Tool)?;
            (m, r.patch_types, r.stopped_after, r.meta, r.notes)
        }
    };
    let seconds = t0.elapsed().as_secs_f64();
    notes.extend(gen_notes);
    let meta = PolyMeshGenMeta { surface_file: Some(reading.surface_file.clone()), workflow: meta };
    write_poly_mesh_gen(&mesh, &patch_types, &NamedSets::default(), &meta, &poly_dir).map_err(CliError::Io)?;
    Ok(CfMeshRun { mesh, patch_types, stopped_after, notes, reported, seconds })
}

/// The executables' `main`: parse `-case`, run, print a summary, exit 1 on
/// error. CLI glue (no test reaches it; [`run_cfmesh`] is tested).
pub fn cfmesh_main(generator: Generator) {
    let args = crate::openfoam_args();
    let name = generator.name();
    let case = match args.case_dir() {
        Ok(c) => c,
        Err(e) => {
            eprintln!("{name}: error: {e}");
            std::process::exit(1);
        }
    };
    match run_cfmesh(&case, generator) {
        Ok(r) => {
            for n in &r.notes {
                println!("  {n}");
            }
            if let Some(s) = r.stopped_after {
                println!("stopAfter: the workflow stopped after {}", s.name());
            }
            let m = &r.mesh;
            println!("Mesh: {} points, {} faces ({} internal), {} cells", m.points.len(), m.faces.len(), m.n_internal_faces(), m.n_cells);
            for (p, t) in m.patches.iter().zip(r.patch_types.iter()) {
                println!("  patch {:<24} {:<14} {} faces", p.name, t, p.n_faces);
            }
            if !r.reported.is_empty() {
                println!("meshDict entries read but not acted on: {}", r.reported.len());
            }
            println!("ExecutionTime = {:.2} s (meshing)", r.seconds);
            println!("End");
        }
        Err(e) => {
            eprintln!("{name}: error: {e}");
            std::process::exit(1);
        }
    }
}
