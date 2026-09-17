//! End-to-end demo of the **polyhedral dual** — turn a hex background mesh into
//! a polyhedral cell mesh (one cell per vertex), à la OpenFOAM `polyDualMesh`.
//!
//! Runs the whole pipeline with only this crate (no GUI, no other crate, no
//! cargo feature):
//!
//! ```text
//! shapes (domain box)
//!   -> carve_box        (hex background volume mesh)
//!   -> polyhedral_dual  (median dual: one polyhedron per primal vertex)
//!   -> check_quality    (is the polyhedral mesh solvable?)
//! ```
//!
//! The dual is a **repartition** of the same region: it tiles the identical
//! volume with polyhedral cells that carry more neighbours per cell (better
//! gradient reconstruction) than the hex background it came from.
//!
//! Run it with:
//!
//! ```text
//! cargo run -p outram-park-fork-cfmesh --example polyhedral_dual --release
//! ```

use outram_park_fork_cfmesh::carve::carve_box;
use outram_park_fork_cfmesh::checks::check_quality;
use outram_park_fork_cfmesh::dual::polyhedral_dual;
use outram_park_fork_cfmesh::math::Vec3;
use outram_park_fork_cfmesh::shapes::box_surface;
use outram_park_fork_cfmesh::volume_mesh::cells_faces;

fn main() {
    // ---- Geometry: a unit box, carved into a hex background mesh. ------------
    let (pts, tris) = box_surface(Vec3::ZERO, Vec3::new(1.0, 1.0, 1.0));
    let cell_size = 0.25;
    let hex = carve_box(&pts, &tris, cell_size);
    println!("Hex background (cell size {cell_size}):");
    println!("  cells              = {}", hex.cell_count());
    println!("  vertices           = {}", hex.point_count());
    println!("  volume             = {:.4}", hex.total_volume());

    // ---- Take the polyhedral (median) dual: one cell per vertex. ------------
    let dual = polyhedral_dual(&hex);
    let faces_per_cell: Vec<usize> = cells_faces(&dual).iter().map(|c| c.len()).collect();
    let max_faces = faces_per_cell.iter().copied().max().unwrap();
    let avg_faces = faces_per_cell.iter().sum::<usize>() as f64 / faces_per_cell.len() as f64;
    println!("\nPolyhedral dual (one cell per primal vertex):");
    println!("  cells              = {}", dual.cell_count());
    println!(
        "  faces              = {} ({} internal, {} boundary)",
        dual.face_count(),
        dual.n_internal_faces(),
        dual.n_boundary_faces()
    );
    println!("  faces/cell         = {avg_faces:.1} avg, {max_faces} max (polyhedral)");
    println!(
        "  volume             = {:.4}  (repartition — preserved)",
        dual.total_volume()
    );

    // ---- Quality: is the polyhedral mesh solvable? --------------------------
    let q = check_quality(&dual);
    println!("\nQuality (checkMesh-style):");
    println!(
        "  max non-orthogonality = {:.2} deg",
        q.max_non_orthogonality_deg
    );
    println!("  max skewness          = {:.3}", q.max_skewness);
    println!("  max aspect ratio      = {:.3}", q.max_aspect_ratio);
    println!("  min cell volume       = {:.3e}", q.min_cell_volume);
    println!("  negative-volume cells = {}", q.n_negative_volume_cells);

    match dual.validate() {
        Ok(()) => println!("\nvalidate(): OK — every polyhedral cell is closed."),
        Err(e) => println!("\nvalidate(): {e}"),
    }
    println!("\nPipeline ran: shapes -> carve_box -> polyhedral_dual -> check_quality.");
}
