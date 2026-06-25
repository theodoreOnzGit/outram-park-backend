use std::path::Path;
use std::sync::Arc;
use openfoam_basic_lib::prelude::FvMesh;
use crate::error::AppBuilderError;

/// Read an OpenFOAM `constant/polyMesh/` directory and return an `FvMesh`.
///
/// Expected files:
///   `points`     — one `(x y z)` vertex per line
///   `faces`      — one face per line: `N(v0 v1 … vN-1)`
///   `owner`      — owner cell index per face
///   `neighbour`  — neighbour cell index per internal face
///   `boundary`   — patch definitions (name, type, startFace, nFaces)
///
/// C++ source: `src/OpenFOAM/meshes/polyMesh/`
pub fn read_poly_mesh(poly_mesh_dir: &Path) -> Result<Arc<FvMesh>, AppBuilderError> {
    let _ = poly_mesh_dir;
    todo!("read_poly_mesh: parse points, faces, owner, neighbour, boundary files")
}

/// Parse the `points` file: lines of `(x y z)` into `Vec<[f64; 3]>`.
fn parse_points(_text: &str) -> Result<Vec<[f64; 3]>, AppBuilderError> {
    todo!("parse_points")
}

/// Parse the `faces` file: lines of `N(v0 v1 … vN-1)` into `Vec<Vec<usize>>`.
fn parse_faces(_text: &str) -> Result<Vec<Vec<usize>>, AppBuilderError> {
    todo!("parse_faces")
}

/// Parse the `owner` / `neighbour` files: one integer per line.
fn parse_index_list(_text: &str) -> Result<Vec<usize>, AppBuilderError> {
    todo!("parse_index_list")
}
