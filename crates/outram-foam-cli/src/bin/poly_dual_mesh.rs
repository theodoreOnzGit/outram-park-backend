//! `polyDualMesh` — construct the polyhedral dual of a mesh.
//!
//! Mirrors the upstream OpenFOAM `polyDualMesh` utility
//! (`polyDualMesh featureAngle [-overwrite] [-region name] [-splitAllFaces]
//! [-concaveMultiCells] [-doNotPreserveFaceZones]`, gh:#1471):
//! reads the primal `polyMesh` from `<case>/constant[/<region>]/polyMesh` via
//! [`outram_foam_basic_lib::io::poly_mesh::PolyMesh`], builds the dual, and
//! writes it to a **new time directory** (`<case>/<startTime + deltaT>/polyMesh`,
//! as upstream's `runTime++`) or, with `-overwrite`, over
//! `<case>/constant/polyMesh` (the stale `cellZones`, `pointZones` and
//! `faceZones` files are removed, as upstream clears the zones).
//!
//! ## Two constructions
//!
//! - **Default (no `-upstream`)**: the surface-preserving dual of
//!   [`outram_foam_mesh::poly_dual_mesh::poly_dual_mesh`]: a dual point at every
//!   boundary-face centre and boundary-edge midpoint, so a flat patch is
//!   reproduced exactly. `featureAngle` is the angle above which a boundary
//!   *edge* (not a point) is a feature edge; every patch seam is a feature
//!   edge at any angle (gh:#1240).
//! - **`-upstream`**: [`outram_foam_mesh::poly_dual_mesh_upstream`], a
//!   line-for-line port of `polyDualMesh` + `meshDualiser` with
//!   `polyTopoChange`'s numbering (gh:#1472). Only this mode implements
//!   `-splitAllFaces`, `-concaveMultiCells` and face-zone preservation
//!   (`constant/polyMesh/faceZones` is read; `-doNotPreserveFaceZones`
//!   disables it); in the default mode they are an error, not silently
//!   ignored. `-flavour esi` selects OpenFOAM.com v2506 (its face-centre formula and its
//!   hash-set sizing) instead of OpenFOAM-dev (gh:#1473, gh:#1472).
//!
//! The legacy [`run`] entry (no arguments beyond `-case`) keeps its original
//! behaviour: the default construction at [`DEFAULT_FEATURE_ANGLE_DEG`],
//! written to `constant/dualMesh`.
//!
//! ## Bridges
//!
//! Two conversions are needed, because the I/O type
//! ([`outram_foam_basic_lib::io::poly_mesh::PolyMesh`], faces as a `MeshFace`
//! list) and the dualiser input ([`outram_foam_mesh::poly_dual_mesh::PolyMesh`],
//! struct-of-arrays: separate `faces`/`owner`/`neighbour`) store the same
//! connectivity differently:
//!
//! - [`to_primal`] — I/O `PolyMesh` → dualiser input `PolyMesh`.
//! - [`dual_to_io`] — the resulting [`DualMesh`] → I/O `PolyMesh` for writing.
//!   `DualMesh` faces are unordered with `Option` neighbours; `dual_to_io`
//!   re-orders them internal-first then by patch (stably, so an already
//!   ordered upstream-mode dual keeps its order) and rebuilds the patch
//!   descriptors (reusing the primal patch names/kinds).

use std::path::Path;

use clap::Parser;
use outram_foam_basic_lib::io::poly_mesh::{MeshFace as IoMeshFace, PolyMesh as IoPolyMesh};
use outram_foam_basic_lib::mesh::{BoundaryPatch, PatchKind};
use outram_foam_cli::{CaseArgs, CliError};
use outram_foam_mesh::poly_dual_mesh::{self, DualMesh, PolyMesh as PrimalPolyMesh};
use outram_foam_mesh::poly_dual_mesh_upstream::{
    poly_dual_mesh_upstream, FaceZoneInput, UpstreamOptions,
};

/// Sub-directory of `<case>/constant` the legacy [`run`] writes the dual to.
///
/// The dual is written beside the primal (not over it) so a run is repeatable
/// and the primal mesh remains available.
pub const DUAL_OUTPUT_SUBDIR: &str = "dualMesh";

/// Default OpenFOAM-style feature angle `[deg]` for the legacy [`run`] entry.
pub const DEFAULT_FEATURE_ANGLE_DEG: f64 = 45.0;

/// The utility's command line (OpenFOAM single-dash options are promoted to
/// `clap`'s double-dash form by `openfoam_argv`).
#[derive(Debug, Clone, Parser)]
pub struct PolyDualArgs {
    /// `-case` directory.
    #[command(flatten)]
    pub case: CaseArgs,
    /// `featureAngle [0-180]` in degrees (required, as upstream).
    pub feature_angle: f64,
    /// `-overwrite`: replace `constant/polyMesh` instead of writing a new time.
    #[arg(long = "overwrite")]
    pub overwrite: bool,
    /// `-region name`.
    #[arg(long = "region")]
    pub region: Option<String>,
    /// `-splitAllFaces` (needs `-upstream`).
    #[arg(long = "splitAllFaces")]
    pub split_all_faces: bool,
    /// `-concaveMultiCells` (needs `-upstream`).
    #[arg(long = "concaveMultiCells")]
    pub concave_multi_cells: bool,
    /// `-doNotPreserveFaceZones` (needs `-upstream`).
    #[arg(long = "doNotPreserveFaceZones")]
    pub do_not_preserve_face_zones: bool,
    /// Use the upstream-mode construction (gh:#1472).
    #[arg(long = "upstream")]
    pub upstream: bool,
    /// Which OpenFOAM line the upstream mode follows where they differ: `dev` (default) or `esi` (v2506).
    #[arg(long = "flavour", default_value = "dev")]
    pub flavour: String,
}

fn main() {
    let args = PolyDualArgs::parse_from(outram_foam_cli::openfoam_argv(std::env::args_os()));
    if let Err(e) = run_with(&args) {
        eprintln!("polyDualMesh: error: {e}");
        std::process::exit(1);
    }
}

/// Run the utility as upstream's command line does (gh:#1471): build the dual
/// of `<case>/constant[/<region>]/polyMesh` and write it to a new time
/// directory, or over the primal mesh with `-overwrite`.
///
/// # Errors
/// - [`CliError::CaseNotFound`] / [`CliError::Io`] for a missing case, mesh,
///   `controlDict` or an unwritable target.
/// - [`CliError::Tool`] for a rejected option combination
///   (`-splitAllFaces` etc. without `-upstream`), a `featureAngle` outside
///   `[0, 180]`, or a failed dual construction.
pub fn run_with(args: &PolyDualArgs) -> Result<(), CliError> {
    if !(0.0..=180.0).contains(&args.feature_angle) {
        return Err(CliError::Tool(format!(
            "featureAngle {} is outside [0, 180]",
            args.feature_angle
        )));
    }
    if !args.upstream
        && (args.split_all_faces || args.concave_multi_cells || args.do_not_preserve_face_zones)
    {
        return Err(CliError::Tool(
            "-splitAllFaces, -concaveMultiCells and -doNotPreserveFaceZones are \
             implemented by the upstream-mode construction only: add -upstream"
                .to_string(),
        ));
    }
    let case = args.case.case_dir()?;
    let mut base = case.join("constant");
    if let Some(r) = &args.region {
        base = base.join(r);
    }
    let primal_dir = base.join("polyMesh");
    let io_primal = IoPolyMesh::read(&primal_dir).map_err(|e| CliError::Io(e.to_string()))?;
    let primal = to_primal(&io_primal)?;
    let patch_names: Vec<String> = io_primal.patches.iter().map(|p| p.name.clone()).collect();
    let patch_kinds: Vec<PatchKind> = io_primal.patches.iter().map(|p| p.kind).collect();

    let dual = if args.upstream {
        let mut o = UpstreamOptions::new(args.feature_angle);
        match args.flavour.as_str() {
            "dev" => {}
            "esi" => o = o.with_esi2506_flavour(),
            other => return Err(CliError::Tool(format!("-flavour {other}: expected dev or esi"))),
        }
        if args.split_all_faces {
            o = o.with_split_all_faces();
        }
        if args.concave_multi_cells {
            o = o.with_concave_multi_cells();
        }
        if args.do_not_preserve_face_zones {
            o = o.without_face_zone_preservation();
        } else {
            o = o.with_face_zones(read_face_zones(&primal_dir.join("faceZones"))?);
        }
        poly_dual_mesh_upstream(&primal, &o)
    } else {
        poly_dual_mesh::poly_dual_mesh(&primal, args.feature_angle)
    }
    .map_err(|e| CliError::Tool(e.to_string()))?;
    let io_dual = dual_to_io(&dual, &patch_names, &patch_kinds);

    let out_dir = if args.overwrite {
        primal_dir.clone()
    } else {
        let t = next_time_name(&case)?;
        let mut d = case.join(t);
        if let Some(r) = &args.region {
            d = d.join(r);
        }
        d.join("polyMesh")
    };
    io_dual
        .write(&out_dir)
        .map_err(|e| CliError::Io(e.to_string()))?;
    if args.overwrite {
        for z in ["cellZones", "pointZones", "faceZones"] {
            let p = out_dir.join(z);
            if p.is_file() {
                std::fs::remove_file(&p).map_err(|e| CliError::Io(e.to_string()))?;
            }
        }
    }
    print_summary(&io_primal, &io_dual, &out_dir);
    Ok(())
}

/// OpenFOAM `Time::timeName` for `value`: `%g` with `timePrecision` 6.
pub fn time_name(value: f64) -> String {
    if value == 0.0 {
        return "0".to_string();
    }
    // The decimal exponent from Rust's `{:e}` formatting (no libm log10).
    let sci = format!("{:e}", value.abs());
    let exp: i32 = sci.split_once('e').and_then(|(_, e)| e.parse().ok()).unwrap_or(0);
    if !(-4..6).contains(&exp) {
        let s = format!("{value:.5e}");
        let (mant, e) = s.split_once('e').unwrap_or((&s, "0"));
        let mant = if mant.contains('.') {
            mant.trim_end_matches('0').trim_end_matches('.')
        } else {
            mant
        };
        let e: i32 = e.parse().unwrap_or(0);
        return format!("{mant}e{}{:02}", if e < 0 { '-' } else { '+' }, e.abs());
    }
    let decimals = (5 - exp).max(0) as usize;
    let s = format!("{value:.decimals$}");
    if s.contains('.') {
        s.trim_end_matches('0').trim_end_matches('.').to_string()
    } else {
        s
    }
}

/// The time directory name `runTime++` gives: `startTime + deltaT` read from
/// `system/controlDict`, in OpenFOAM's `timeName` format.
///
/// # Errors
/// [`CliError::Io`] when `controlDict` is missing or has no `deltaT`.
pub fn next_time_name(case: &Path) -> Result<String, CliError> {
    let text = std::fs::read_to_string(case.join("system").join("controlDict"))
        .map_err(|e| CliError::Io(format!("system/controlDict: {e}")))?;
    let read = |key: &str| -> Option<f64> {
        text.lines().find_map(|l| {
            let rest = l.trim().strip_prefix(key)?;
            if !rest.starts_with(char::is_whitespace) {
                return None;
            }
            rest.trim().trim_end_matches(';').trim().parse().ok()
        })
    };
    let delta = read("deltaT")
        .ok_or_else(|| CliError::Io("system/controlDict has no numeric deltaT".to_string()))?;
    Ok(time_name(read("startTime").unwrap_or(0.0) + delta))
}

/// Read a `faceZones` file (`N ( name { type faceZone; faceLabels ...; } ... )`).
/// A missing file is no zones.
///
/// # Errors
/// [`CliError::Io`] for an unreadable or malformed file.
pub fn read_face_zones(path: &Path) -> Result<Vec<FaceZoneInput>, CliError> {
    if !path.is_file() {
        return Ok(Vec::new());
    }
    let text = std::fs::read_to_string(path).map_err(|e| CliError::Io(e.to_string()))?;
    let body: String = text
        .lines()
        .filter(|l| !l.trim_start().starts_with("//"))
        .collect::<Vec<_>>()
        .join("\n");
    let body = match body.find("FoamFile") {
        Some(i) => {
            let close = body[i..].find('}').map_or(0, |c| c + i + 1);
            body[close..].to_string()
        }
        None => body,
    };
    let spaced = body
        .replace('(', " ( ")
        .replace(')', " ) ")
        .replace('{', " { ")
        .replace('}', " } ")
        .replace(';', " ; ");
    let tok: Vec<&str> = spaced.split_whitespace().collect();
    let mut zones = Vec::new();
    let mut i = 0;
    while i < tok.len() {
        if tok[i] == "{" && i > 0 {
            let name = tok[i - 1].to_string();
            let mut j = i + 1;
            let mut faces = Vec::new();
            let mut d = 1usize;
            while j < tok.len() && d > 0 {
                match tok[j] {
                    "{" => d += 1,
                    "}" => d -= 1,
                    "faceLabels" => {
                        while j < tok.len() && tok[j] != "(" {
                            j += 1;
                        }
                        j += 1;
                        while j < tok.len() && tok[j] != ")" {
                            faces.push(tok[j].parse().map_err(|_| {
                                CliError::Io(format!("faceZones: bad label {}", tok[j]))
                            })?);
                            j += 1;
                        }
                    }
                    _ => {}
                }
                j += 1;
            }
            zones.push(FaceZoneInput { name, faces });
            i = j;
        } else {
            i += 1;
        }
    }
    Ok(zones)
}

/// Read `<case>/constant/polyMesh`, build the polyhedral dual at the default
/// feature angle, and write it to `<case>/constant/<DUAL_OUTPUT_SUBDIR>`,
/// printing an OpenFOAM-style mesh summary.
///
/// # Errors
/// - [`CliError::CaseNotFound`] if `-case` is not a directory.
/// - [`CliError::Io`] if the primal mesh cannot be read or the dual written.
/// - [`CliError::Tool`] if the dual construction fails (e.g. a non-manifold
///   boundary), or the primal mesh has a malformed internal face.
pub fn run(args: &CaseArgs) -> Result<(), CliError> {
    let case = args.case_dir()?;
    let primal_dir = case.join("constant").join("polyMesh");

    let io_primal = IoPolyMesh::read(&primal_dir).map_err(|e| CliError::Io(e.to_string()))?;
    let primal = to_primal(&io_primal)?;

    // Preserve the primal patch names/kinds to name the dual boundary patches.
    let patch_names: Vec<String> = io_primal.patches.iter().map(|p| p.name.clone()).collect();
    let patch_kinds: Vec<PatchKind> = io_primal.patches.iter().map(|p| p.kind).collect();

    let dual = poly_dual_mesh::poly_dual_mesh(&primal, DEFAULT_FEATURE_ANGLE_DEG)
        .map_err(|e| CliError::Tool(e.to_string()))?;

    let io_dual = dual_to_io(&dual, &patch_names, &patch_kinds);

    let dual_dir = case.join("constant").join(DUAL_OUTPUT_SUBDIR);
    io_dual
        .write(&dual_dir)
        .map_err(|e| CliError::Io(e.to_string()))?;

    print_summary(&io_primal, &io_dual, &dual_dir);
    Ok(())
}

/// Bridge the I/O `PolyMesh` (face list) to the dualiser's struct-of-arrays
/// input `PolyMesh` (`faces` / `owner` / `neighbour`).
///
/// # Errors
/// [`CliError::Tool`] if an internal face (index `< n_internal_faces`) has no
/// neighbour, which would make the primal mesh inconsistent.
fn to_primal(io: &IoPolyMesh) -> Result<PrimalPolyMesh, CliError> {
    let faces: Vec<Vec<usize>> = io.faces.iter().map(|f| f.verts.clone()).collect();
    let owner: Vec<usize> = io.faces.iter().map(|f| f.owner).collect();
    let mut neighbour = Vec::with_capacity(io.n_internal_faces);
    for f in io.faces.iter().take(io.n_internal_faces) {
        neighbour.push(f.neighbour.ok_or_else(|| {
            CliError::Tool("primal internal face has no neighbour cell".to_string())
        })?);
    }
    Ok(PrimalPolyMesh {
        points: io.points.clone(),
        faces,
        owner,
        neighbour,
        n_internal_faces: io.n_internal_faces,
        patches: io.patches.clone(),
        n_cells: io.n_cells,
    })
}

/// Bridge a [`DualMesh`] to the I/O `PolyMesh` for writing.
///
/// Re-orders the (unordered) dual faces into OpenFOAM order — internal faces
/// first, then boundary faces grouped by patch — and rebuilds contiguous
/// [`BoundaryPatch`] descriptors, naming each dual patch after the primal patch
/// it came from.
fn dual_to_io(dual: &DualMesh, patch_names: &[String], patch_kinds: &[PatchKind]) -> IoPolyMesh {
    let nf = dual.faces.len();
    let n_patches = patch_names.len().max(1);

    // Bucket boundary faces by source primal patch index; internal faces are
    // those with a neighbour.
    let mut internal: Vec<usize> = Vec::new();
    let mut patch_faces: Vec<Vec<usize>> = vec![Vec::new(); n_patches];
    for f in 0..nf {
        if dual.neighbour[f].is_some() {
            internal.push(f);
        } else {
            let pi = dual.face_patch[f].unwrap_or(0).min(n_patches - 1);
            patch_faces[pi].push(f);
        }
    }

    let n_internal_faces = internal.len();
    let mut faces: Vec<IoMeshFace> = Vec::with_capacity(nf);
    for &f in &internal {
        faces.push(IoMeshFace {
            verts: dual.faces[f].clone(),
            owner: dual.owner[f],
            neighbour: dual.neighbour[f],
        });
    }

    let mut patches: Vec<BoundaryPatch> = Vec::with_capacity(n_patches);
    let mut start = n_internal_faces;
    for (pi, pf) in patch_faces.iter().enumerate() {
        for &f in pf {
            faces.push(IoMeshFace {
                verts: dual.faces[f].clone(),
                owner: dual.owner[f],
                neighbour: None,
            });
        }
        let name = patch_names
            .get(pi)
            .cloned()
            .unwrap_or_else(|| format!("patch{pi}"));
        let kind = patch_kinds.get(pi).copied().unwrap_or(PatchKind::Patch);
        patches.push(BoundaryPatch::new(name, start, pf.len(), kind));
        start += pf.len();
    }

    IoPolyMesh {
        points: dual.points.clone(),
        faces,
        n_internal_faces,
        n_cells: dual.n_cells,
        patches,
    }
}

/// Print an OpenFOAM-style summary of the primal → dual transformation.
fn print_summary(primal: &IoPolyMesh, dual: &IoPolyMesh, dual_dir: &Path) {
    println!(
        "polyDualMesh: wrote dual polyMesh to {}",
        dual_dir.display()
    );
    println!(
        "  primal:  cells {:>7}   points {:>7}   faces {:>7}",
        primal.n_cells,
        primal.n_points(),
        primal.n_faces()
    );
    println!(
        "  dual:    cells {:>7}   points {:>7}   faces {:>7}",
        dual.n_cells,
        dual.n_points(),
        dual.n_faces()
    );
    println!("  internal faces:  {}", dual.n_internal_faces);
    println!("  boundary faces:  {}", dual.n_boundary_faces());
    println!("  patches:         {}", dual.patches.len());
    for p in &dual.patches {
        println!(
            "    {:<16} {:<10} nFaces: {}",
            p.name,
            format!("{:?}", p.kind),
            p.size
        );
    }
}
