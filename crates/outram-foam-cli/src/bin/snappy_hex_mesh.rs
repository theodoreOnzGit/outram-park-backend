// SPDX-License-Identifier: GPL-3.0-only
// Copyright (C) 2026 OUTRAM PARK contributors
// Derived from OpenFOAM (www.openfoam.org), Copyright (C) 2011-2026 OpenFOAM Foundation

//! `snappyHexMesh` — mesh around a triangulated surface (gh:#1470).
//!
//! Follows OpenFOAM-dev `852527377c7e` `snappyHexMesh.C` in what it reads:
//!
//! - `<case>/system/snappyHexMeshDict` (`#include "file"` resolved; override
//!   with `-dict`);
//! - the background mesh `<case>/constant/polyMesh` (a `blockMesh` box; its
//!   bounds and per-axis counts are recovered from the points);
//! - the surface named by `geometry.<name>.file`, from `constant/geometry/`
//!   (dev) or `constant/triSurface/` (older cases).
//!
//! and in where it writes: `-overwrite` replaces `constant/polyMesh`;
//! otherwise the mesh goes to `<case>/1/polyMesh` (upstream's first mesh time).
//!
//! **What runs.** Cells are selected by OpenFOAM's surface-intersection rule
//! (gh:#1465 step 1), then this crate's inside-test cell removal, then snapping
//! if `snap true`. Every dictionary keyword not acted on is printed under
//! `Not ported:` so a keyword that does nothing is never invisible. `addLayers
//! true` is reported and layers are NOT added from the dictionary
//! (gh:#1469). The result is a research-grade approximation of what
//! upstream produces, not a reproduction (see gh:#1459).
//!
//! Research, education and V&V only (`RESPONSIBLE_USE.md`).

use std::path::PathBuf;

use clap::Parser;
use outram_foam_basic_lib::io::poly_mesh::PolyMesh as IoPolyMesh;
use outram_foam_cli::{CaseArgs, CliError};
use outram_foam_mesh::driver::mesh_from_surface;
use outram_foam_mesh::snappy_hex_mesh::dict::{background_from_points, SnappyDict};
use outram_foam_mesh::snappy_hex_mesh::read_stl;

/// Command line: `-case`, `-dict`, `-overwrite`.
#[derive(Debug, Clone, Parser)]
pub struct SnappyArgs {
    /// Case selection (`-case`).
    #[command(flatten)]
    pub case: CaseArgs,
    /// Dictionary to read instead of `system/snappyHexMeshDict` (`-dict`).
    #[arg(long = "dict")]
    pub dict: Option<PathBuf>,
    /// Overwrite `constant/polyMesh` instead of writing `1/polyMesh`.
    #[arg(long = "overwrite", default_value_t = false)]
    pub overwrite: bool,
}

fn main() {
    let args = SnappyArgs::parse_from(outram_foam_cli::openfoam_argv(std::env::args_os()));
    match run(&args) {
        Ok(report) => print!("{report}"),
        Err(e) => {
            eprintln!("snappyHexMesh: error: {e}");
            std::process::exit(1);
        }
    }
}

/// Run the mesher on a case and return the printed report.
///
/// # Errors
/// - [`CliError::CaseNotFound`] / [`CliError::Io`] for a missing case, dict,
///   background mesh or surface file, or a write failure;
/// - [`CliError::Tool`] for a dictionary or meshing error.
pub fn run(args: &SnappyArgs) -> Result<String, CliError> {
    let case = args.case.case_dir()?;
    let dict_path = args
        .dict
        .clone()
        .unwrap_or_else(|| case.join("system").join("snappyHexMeshDict"));
    let dict = SnappyDict::read(&dict_path).map_err(|e| CliError::Tool(e.to_string()))?;

    let bg = IoPolyMesh::read(case.join("constant").join("polyMesh"))
        .map_err(|e| CliError::Io(format!("background mesh: {e}")))?;
    let (domain, divisions) =
        background_from_points(&bg.points).map_err(|e| CliError::Tool(e.to_string()))?;

    let surface_name = dict
        .surface_levels
        .first()
        .map(|(n, _, _)| n.clone())
        .ok_or_else(|| CliError::Tool("no refinementSurfaces entry".to_string()))?;
    let geom = dict
        .geometry
        .iter()
        .find(|g| g.name == surface_name)
        .ok_or_else(|| CliError::Tool(format!("refinement surface `{surface_name}` is not in geometry")))?;
    let file = geom
        .file
        .clone()
        .ok_or_else(|| CliError::Tool(format!("geometry `{surface_name}` has no file (type {})", geom.kind)))?;
    let stl_path = [case.join("constant").join("geometry").join(&file), case.join("constant").join("triSurface").join(&file)]
        .into_iter()
        .find(|p| p.is_file())
        .ok_or_else(|| CliError::Io(format!("surface `{file}` not found in constant/geometry or constant/triSurface")))?;
    let surface = read_stl(&stl_path).map_err(|e| CliError::Io(e.to_string()))?;

    let controls = dict
        .to_meshing_controls(domain, divisions)
        .map_err(|e| CliError::Tool(e.to_string()))?;
    let generated = mesh_from_surface(&surface, &controls).map_err(|e| CliError::Tool(e.to_string()))?;

    let out_dir = if args.overwrite {
        case.join("constant").join("polyMesh")
    } else {
        case.join("1").join("polyMesh")
    };
    generated
        .mesh
        .write(&out_dir)
        .map_err(|e| CliError::Io(format!("writing {}: {e}", out_dir.display())))?;

    let mut r = String::new();
    r.push_str(&format!(
        "Background: {}x{}x{} cells; surface {} ({} triangles) from {}\n",
        divisions[0],
        divisions[1],
        divisions[2],
        surface_name,
        surface.len(),
        stl_path.display()
    ));
    r.push_str(&format!(
        "Castellated mesh: {} cells; final mesh: {} cells, {} points, {} faces\n",
        generated.n_cells_castellated,
        generated.mesh.n_cells,
        generated.mesh.points.len(),
        generated.mesh.faces.len()
    ));
    r.push_str(&format!("Written to {}\n", out_dir.display()));
    if dict.add_layers {
        r.push_str("addLayers true: layers are NOT added from the dictionary (gh:#1469)\n");
    }
    if !dict.castellated_mesh {
        r.push_str("castellatedMesh false: ignored, castellation always runs (gh:#1470)\n");
    }
    r.push_str("Not ported:\n");
    for (path, why) in &dict.not_ported {
        r.push_str(&format!("    {path}: {why}\n"));
    }
    Ok(r)
}
