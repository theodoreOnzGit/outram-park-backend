//! `blockMesh` — structured hex meshing from a `blockMeshDict`.
//!
//! Mirrors the upstream OpenFOAM `blockMesh` utility: reads
//! `<case>/system/blockMeshDict`, builds the mesh with
//! [`outram_foam_mesh::block_mesh`], and writes the resulting `polyMesh`
//! connectivity (`points`, `faces`, `owner`, `neighbour`, `boundary`) into
//! `<case>/constant/polyMesh` via
//! [`outram_foam_basic_lib::io::poly_mesh::PolyMesh`].
//!
//! ## Bridge
//!
//! The mesher ([`outram_foam_mesh::block_mesh::PolyMesh`]) and the I/O writer
//! ([`outram_foam_basic_lib::io::poly_mesh::PolyMesh`]) are two distinct types
//! that carry the *same* connectivity — points, faces as vertex loops with
//! owner/neighbour, the internal-face count, and boundary patches. [`bridge`]
//! maps one to the other field-by-field (only the per-crate `MeshFace` needs
//! converting; `Vector3`, `BoundaryPatch`, and `PatchKind` are the shared
//! `outram-foam-basic-lib` types).

use outram_foam_basic_lib::io::poly_mesh::{MeshFace as IoMeshFace, PolyMesh as IoPolyMesh};
use outram_foam_cli::{CaseArgs, CliError};
use outram_foam_mesh::block_mesh;

fn main() {
    let args = outram_foam_cli::openfoam_args();
    if let Err(e) = run(&args) {
        eprintln!("blockMesh: error: {e}");
        std::process::exit(1);
    }
}

/// Read `<case>/system/blockMeshDict`, build the mesh, and write
/// `<case>/constant/polyMesh`, printing an OpenFOAM-style mesh summary.
///
/// # Errors
/// - [`CliError::CaseNotFound`] if `-case` is not a directory.
/// - [`CliError::Io`] if the dict is missing or the `polyMesh` cannot be written.
/// - [`CliError::Tool`] if `blockMesh` fails to parse or build the dict.
pub fn run(args: &CaseArgs) -> Result<(), CliError> {
    let case = args.case_dir()?;
    let dict_path = case.join("system").join("blockMeshDict");
    if !dict_path.is_file() {
        return Err(CliError::Io(format!(
            "blockMeshDict not found at {}",
            dict_path.display()
        )));
    }

    let mesh =
        block_mesh::block_mesh_from_file(&dict_path).map_err(|e| CliError::Tool(e.to_string()))?;

    let io_mesh = bridge(&mesh);

    let poly_dir = case.join("constant").join("polyMesh");
    io_mesh
        .write(&poly_dir)
        .map_err(|e| CliError::Io(e.to_string()))?;

    print_summary(&mesh, &poly_dir);
    Ok(())
}

/// Bridge the `outram-foam-mesh` block-mesh output to the `outram-foam-basic-lib`
/// I/O `PolyMesh`. Both carry identical connectivity; only the per-crate
/// `MeshFace` differs, so it is rebuilt here.
fn bridge(mesh: &block_mesh::PolyMesh) -> IoPolyMesh {
    let faces = mesh
        .faces
        .iter()
        .map(|f| IoMeshFace {
            verts: f.verts.clone(),
            owner: f.owner,
            neighbour: f.neighbour,
        })
        .collect();
    IoPolyMesh {
        points: mesh.points.clone(),
        faces,
        n_internal_faces: mesh.n_internal_faces,
        n_cells: mesh.n_cells,
        patches: mesh.patches.clone(),
    }
}

/// Print an OpenFOAM-style mesh summary (counts + per-patch face totals).
fn print_summary(mesh: &block_mesh::PolyMesh, poly_dir: &std::path::Path) {
    println!("blockMesh: wrote polyMesh to {}", poly_dir.display());
    println!("  points:          {}", mesh.n_points());
    println!("  faces:           {}", mesh.n_faces());
    println!("  internal faces:  {}", mesh.n_internal_faces);
    println!("  boundary faces:  {}", mesh.n_boundary_faces());
    println!("  cells:           {}", mesh.n_cells);
    println!("  patches:         {}", mesh.patches.len());
    for p in &mesh.patches {
        println!(
            "    {:<16} {:<10} nFaces: {}",
            p.name,
            format!("{:?}", p.kind),
            p.size
        );
    }
}
