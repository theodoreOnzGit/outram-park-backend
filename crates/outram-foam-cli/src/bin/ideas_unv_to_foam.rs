//! `ideasUnvToFoam` — import an I-DEAS `.unv` mesh into a `polyMesh`.
//!
//! Mirrors the upstream OpenFOAM `ideasUnvToFoam` utility: converts an I-DEAS
//! Universal File with [`outram_foam_mesh::ideas_unv_to_foam::convert_file`] and
//! writes the resulting `polyMesh` connectivity into `<case>/constant/polyMesh`
//! via [`outram_foam_basic_lib::io::poly_mesh::PolyMesh`].
//!
//! ## Arguments
//!
//! - `-case <dir>` — the OpenFOAM case directory (default `.`).
//! - `<unv>` — path to the `.unv` file, positional. It may be absolute or given
//!   relative to the case directory. If omitted, the single `.unv` file in the
//!   case directory is used (an error is returned when there is none, or more
//!   than one).
//!
//! ## Bridge
//!
//! [`outram_foam_mesh::ideas_unv_to_foam::UnvPolyMesh`] stores the point cloud
//! and the global face → vertex-loop list alongside an assembled `FvMesh`
//! (which carries owner/neighbour and the patches, but no vertex connectivity).
//! [`bridge`] recombines the loops with the `FvMesh` owner/neighbour to build
//! the I/O [`PolyMesh`](outram_foam_basic_lib::io::poly_mesh::PolyMesh): faces
//! are already ordered internal-first then by patch, so face `f` is internal
//! iff `f < fv_mesh.n_internal_faces`.

use std::path::{Path, PathBuf};

use clap::Parser;
use outram_foam_basic_lib::io::poly_mesh::{MeshFace as IoMeshFace, PolyMesh as IoPolyMesh};
use outram_foam_cli::{CaseArgs, CliError};
use outram_foam_mesh::ideas_unv_to_foam::{self, UnvPolyMesh};

/// `ideasUnvToFoam` command-line arguments.
#[derive(Parser)]
#[command(name = "ideasUnvToFoam")]
struct Args {
    /// Case directory to operate on (the OpenFOAM `-case` option).
    #[arg(long = "case", short = 'c', default_value = ".")]
    case: PathBuf,
    /// The `.unv` file (absolute, or relative to the case directory). When
    /// omitted, the single `.unv` file in the case directory is used.
    unv: Option<PathBuf>,
}

fn main() {
    let args = Args::parse_from(outram_foam_cli::openfoam_argv(std::env::args_os()));
    let case = CaseArgs { case: args.case };
    if let Err(e) = run_with_unv(&case, args.unv) {
        eprintln!("ideasUnvToFoam: error: {e}");
        std::process::exit(1);
    }
}

/// Convert the single `.unv` file found in the case directory and write
/// `<case>/constant/polyMesh`. Convenience wrapper over [`run_with_unv`] with no
/// explicit path.
///
/// # Errors
/// As [`run_with_unv`]; additionally [`CliError::Io`] when the case directory
/// does not contain exactly one `.unv` file.
pub fn run(args: &CaseArgs) -> Result<(), CliError> {
    run_with_unv(args, None)
}

/// Convert `unv` (or the sole `.unv` file in the case directory when `None`) and
/// write `<case>/constant/polyMesh`, printing an OpenFOAM-style mesh summary.
///
/// # Errors
/// - [`CliError::CaseNotFound`] if `-case` is not a directory.
/// - [`CliError::Io`] if the `.unv` file cannot be located or the `polyMesh`
///   cannot be written.
/// - [`CliError::Tool`] if the UNV file cannot be parsed / assembled.
pub fn run_with_unv(args: &CaseArgs, unv: Option<PathBuf>) -> Result<(), CliError> {
    let case = args.case_dir()?;
    let unv_path = resolve_unv(&case, unv)?;

    let mesh = ideas_unv_to_foam::convert_file(&unv_path).map_err(|e| CliError::Tool(e.to_string()))?;

    let io_mesh = bridge(&mesh);

    let poly_dir = case.join("constant").join("polyMesh");
    io_mesh
        .write(&poly_dir)
        .map_err(|e| CliError::Io(e.to_string()))?;

    print_summary(&mesh, &unv_path, &poly_dir);
    Ok(())
}

/// Resolve the `.unv` file to import. An explicit path is used as-is when
/// absolute or already existing, otherwise resolved against the case directory.
/// With no explicit path, the case directory must contain exactly one `.unv`.
fn resolve_unv(case: &Path, unv: Option<PathBuf>) -> Result<PathBuf, CliError> {
    if let Some(p) = unv {
        let cand = if p.is_absolute() || p.exists() {
            p
        } else {
            case.join(p)
        };
        if cand.is_file() {
            Ok(cand)
        } else {
            Err(CliError::Io(format!(
                "UNV file not found: {}",
                cand.display()
            )))
        }
    } else {
        let mut found = Vec::new();
        for entry in std::fs::read_dir(case).map_err(|e| CliError::Io(e.to_string()))? {
            let path = entry.map_err(|e| CliError::Io(e.to_string()))?.path();
            if path.extension().and_then(|s| s.to_str()) == Some("unv") {
                found.push(path);
            }
        }
        match found.len() {
            0 => Err(CliError::Io(format!(
                "no `.unv` file found in {}",
                case.display()
            ))),
            1 => Ok(found.pop().unwrap()),
            n => Err(CliError::Io(format!(
                "{n} `.unv` files in {}; pass one explicitly",
                case.display()
            ))),
        }
    }
}

/// Bridge the `UnvPolyMesh` (point cloud + face loops + assembled `FvMesh`) to
/// the I/O `PolyMesh`, reattaching owner/neighbour from the `FvMesh`.
fn bridge(mesh: &UnvPolyMesh) -> IoPolyMesh {
    let fv = &mesh.fv_mesh;
    let n_internal = fv.n_internal_faces;
    let faces = mesh
        .faces
        .iter()
        .enumerate()
        .map(|(fi, verts)| {
            let neighbour = if fi < n_internal {
                Some(fv.neighbour[fi])
            } else {
                None
            };
            IoMeshFace {
                verts: verts.clone(),
                owner: fv.owner[fi],
                neighbour,
            }
        })
        .collect();
    IoPolyMesh {
        points: mesh.points.clone(),
        faces,
        n_internal_faces: n_internal,
        n_cells: fv.n_cells,
        patches: fv.patches.clone(),
    }
}

/// Print an OpenFOAM-style mesh summary.
fn print_summary(mesh: &UnvPolyMesh, unv_path: &Path, poly_dir: &Path) {
    let fv = &mesh.fv_mesh;
    println!(
        "ideasUnvToFoam: converted {} -> {}",
        unv_path.display(),
        poly_dir.display()
    );
    println!("  points:          {}", mesh.points.len());
    println!("  faces:           {}", fv.n_faces);
    println!("  internal faces:  {}", fv.n_internal_faces);
    println!(
        "  boundary faces:  {}",
        fv.n_faces - fv.n_internal_faces
    );
    println!("  cells:           {}", fv.n_cells);
    println!("  patches:         {}", fv.patches.len());
    if mesh.n_skipped_elements > 0 {
        println!(
            "  skipped (deferred) elements: {}",
            mesh.n_skipped_elements
        );
    }
    for p in &fv.patches {
        println!(
            "    {:<16} {:<10} nFaces: {}",
            p.name,
            format!("{:?}", p.kind),
            p.size
        );
    }
}
