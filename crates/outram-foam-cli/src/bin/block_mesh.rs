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
use clap::Parser;
use outram_foam_basic_lib::io::esi::{write_cell_zones_esi, EsiWriteOptions};
use outram_foam_cli::{CaseArgs, CliError, WriteFlavour};
use outram_foam_mesh::block_mesh;

const ESI_BLOCK_MESH_MIN_PRECISION: usize = 10;

/// The utility's command line: `-case`, and `-flavour default|esi` (gh:#1363
/// row 7) choosing the layout of the written `polyMesh` files.
#[derive(Debug, Clone, Parser)]
pub struct BlockMeshArgs {
    /// `-case` directory.
    #[command(flatten)]
    pub case: CaseArgs,
    /// File layout: `default` (this project's) or `esi` (OpenFOAM v2506's).
    #[arg(long = "flavour", default_value = "default")]
    pub flavour: String,
}

fn main() {
    let args = BlockMeshArgs::parse_from(outram_foam_cli::openfoam_argv(std::env::args_os()));
    let result = WriteFlavour::parse(&args.flavour).and_then(|f| run_with_flavour(&args.case, f));
    if let Err(e) = result {
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
    run_with_flavour(args, WriteFlavour::Default)
}

/// [`run`] with the written file layout chosen: [`WriteFlavour::Esi`] writes
/// ESI v2506's bytes at the case's `writePrecision` (default 6).
///
/// # Errors
/// As [`run`].
pub fn run_with_flavour(args: &CaseArgs, flavour: WriteFlavour) -> Result<(), CliError> {
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
    // ESI `blockMesh.C` calls `IOstream::minPrecision(10)`: everything it
    // writes has at least 10 significant digits whatever `writePrecision` says.
    let esi_opts = EsiWriteOptions::default()
        .with_precision(outram_foam_cli::write_precision(&case).max(ESI_BLOCK_MESH_MIN_PRECISION));
    match flavour {
        WriteFlavour::Default => io_mesh.write(&poly_dir),
        WriteFlavour::Esi => io_mesh.write_esi(&poly_dir, "constant/polyMesh", &esi_opts),
    }
    .map_err(|e| CliError::Io(e.to_string()))?;

    // Named block zones become `cellZones` (blockMesh.C, "Adding cell zones").
    match flavour {
        WriteFlavour::Default => IoPolyMesh::write_cell_zones(&poly_dir, &mesh.cell_zones),
        WriteFlavour::Esi => {
            write_cell_zones_esi(&poly_dir, "constant/polyMesh", &mesh.cell_zones, &esi_opts)
        }
    }
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
