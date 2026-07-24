//! End-to-end demo of **tetrahedralization** — split a carved hex mesh into an
//! all-tet volume mesh, then show it composes with the polyhedral dual.
//!
//! Runs with only this crate (no GUI, no other crate, no cargo feature):
//!
//! ```text
//! shapes (domain box)
//!   -> carve_box        (hex background volume mesh)
//!   -> tetrahedralize   (centroid subdivision: every cell -> tets)
//!   -> check_quality    (positive-volume tets?)
//!   -> polyhedral_dual  (tet-primal polyhedral dual — cells composed)
//! ```
//!
//! Tetrahedralization is a **space-filling** decomposition (à la cfMesh), not a
//! quality Delaunay mesher: it conserves volume exactly and triangulates the
//! boundary surface. Feeding the tets to `polyhedral_dual` yields the classic
//! tet → polyhedral route.
//!
//! Run it with:
//!
//! ```text
//! cargo run -p outram-park-fork-cfmesh --example tet_mesh --release
//! ```

use outram_park_fork_cfmesh::carve::carve_box;
use outram_park_fork_cfmesh::checks::check_quality;
use outram_park_fork_cfmesh::dual::polyhedral_dual;
use outram_park_fork_cfmesh::math::Vec3;
use outram_park_fork_cfmesh::shapes::box_surface;
use outram_park_fork_cfmesh::tet::tetrahedralize;

fn main() {
    // ---- Geometry: a unit box carved into a hex background mesh. -------------
    let (pts, tris) = box_surface(Vec3::ZERO, Vec3::new(1.0, 1.0, 1.0));
    let cell_size = 0.5;
    let hex = carve_box(&pts, &tris, cell_size);
    println!("Hex background (cell size {cell_size}):");
    println!("  cells              = {}", hex.cell_count());
    println!("  volume             = {:.4}", hex.total_volume());

    // ---- Tetrahedralize: every cell -> tets (centroid subdivision). ---------
    let tets = tetrahedralize(&hex);
    println!("\nTetrahedralization (centroid subdivision):");
    println!("  tets               = {}", tets.cell_count());
    println!("  faces              = {} ({} internal, {} boundary)", tets.face_count(), tets.n_internal_faces(), tets.n_boundary_faces());
    println!("  volume             = {:.4}  (conserved)", tets.total_volume());

    let q = check_quality(&tets);
    println!("  negative-vol tets  = {}", q.n_negative_volume_cells);
    println!("  min tet volume     = {:.3e}", q.min_cell_volume);
    match tets.validate() {
        Ok(()) => println!("  validate(): OK — every tet is closed."),
        Err(e) => println!("  validate(): {e}"),
    }

    // ---- Compose: the tet-primal polyhedral dual. ---------------------------
    let dual = polyhedral_dual(&tets);
    println!("\nTet-primal polyhedral dual (one cell per tet vertex):");
    println!("  cells              = {}", dual.cell_count());
    println!("  volume             = {:.4}  (conserved through both steps)", dual.total_volume());
    match dual.validate() {
        Ok(()) => println!("  validate(): OK — polyhedral cells closed."),
        Err(e) => println!("  validate(): {e}"),
    }

    println!("\nPipeline ran: shapes -> carve_box -> tetrahedralize -> check_quality -> polyhedral_dual.");
}
