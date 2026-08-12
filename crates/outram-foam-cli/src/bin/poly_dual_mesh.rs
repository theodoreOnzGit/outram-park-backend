//! `polyDualMesh` — construct the polyhedral dual of a mesh.
//!
//! Mirrors the upstream OpenFOAM `polyDualMesh` utility: reads the primal
//! `polyMesh` from `<case>/constant/polyMesh` via
//! [`outram_foam_basic_lib::io::poly_mesh::PolyMesh`], builds the cell-centre
//! dual with [`outram_foam_mesh::poly_dual_mesh`], and writes the dual
//! `polyMesh` into `<case>/constant/<DUAL_OUTPUT_SUBDIR>` (a sibling directory,
//! so the primal mesh is left intact).
//!
//! ## Feature angle
//!
//! The dual construction preserves a primal boundary point as an explicit dual
//! vertex when the surface bends across it by more than
//! [`DEFAULT_FEATURE_ANGLE_DEG`]; otherwise the point is merged into a coplanar
//! boundary polygon. This tool uses the default angle.
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
//!   re-orders them internal-first then by patch and rebuilds the patch
//!   descriptors (reusing the primal patch names/kinds), matching the on-disk
//!   OpenFOAM face ordering.

use std::path::Path;

use outram_foam_basic_lib::io::poly_mesh::{MeshFace as IoMeshFace, PolyMesh as IoPolyMesh};
use outram_foam_basic_lib::mesh::{BoundaryPatch, PatchKind};
use outram_foam_cli::{CaseArgs, CliError};
use outram_foam_mesh::poly_dual_mesh::{self, DualMesh, PolyMesh as PrimalPolyMesh};

/// Sub-directory of `<case>/constant` the dual `polyMesh` is written to.
///
/// The dual is written beside the primal (not over it) so a run is repeatable
/// and the primal mesh remains available.
pub const DUAL_OUTPUT_SUBDIR: &str = "dualMesh";

/// Default OpenFOAM-style feature angle `[deg]` for boundary-point retention.
pub const DEFAULT_FEATURE_ANGLE_DEG: f64 = 45.0;

fn main() {
    let args = outram_foam_cli::openfoam_args();
    if let Err(e) = run(&args) {
        eprintln!("polyDualMesh: error: {e}");
        std::process::exit(1);
    }
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
