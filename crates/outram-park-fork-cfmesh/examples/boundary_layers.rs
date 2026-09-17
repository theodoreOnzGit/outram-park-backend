//! End-to-end demo of **prism boundary layers** — the near-wall inflation a
//! CFD/TH solver needs to resolve wall gradients (low y⁺).
//!
//! Runs the whole volume-meshing pipeline with only this crate (no GUI, no other
//! crate, no cargo feature):
//!
//! ```text
//! shapes (channel box)
//!   -> carve_box       (staircase volume mesh, one `walls` patch)
//!   -> add_boundary_layers  (graded prism stack on `walls`)
//!   -> check_quality   (is the layered mesh solvable?)
//! ```
//!
//! Boundary-layer insertion is a **repartition** of the same region: the
//! interior wall points move inward by the total layer thickness and the vacated
//! shell is filled with `n_layers` graded prism cells per wall face. So the
//! total volume is preserved exactly while the near-wall resolution jumps.
//!
//! Run it with:
//!
//! ```text
//! cargo run -p outram-park-fork-cfmesh --example boundary_layers --release
//! ```

use outram_park_fork_cfmesh::carve::carve_box;
use outram_park_fork_cfmesh::checks::check_quality;
use outram_park_fork_cfmesh::layers::add_boundary_layers;
use outram_park_fork_cfmesh::math::Vec3;
use outram_park_fork_cfmesh::shapes::box_surface;

fn main() {
    // ---- Geometry: a simple channel box. ------------------------------------
    let (pts, tris) = box_surface(Vec3::ZERO, Vec3::new(2.0, 1.0, 1.0));

    // ---- Carve a coarse background mesh (single `walls` patch). -------------
    let cell_size = 0.25;
    let base = carve_box(&pts, &tris, cell_size);
    println!("Background carve (cell size {cell_size}):");
    println!("  cells              = {}", base.cell_count());
    println!("  wall faces         = {}", base.n_boundary_faces());
    println!("  volume             = {:.4}", base.total_volume());

    // ---- Add graded prism boundary layers on the wall patch. ----------------
    let n_layers = 4;
    let first_thickness = 0.01;
    let expansion = 1.3;
    let layered = add_boundary_layers(&base, "walls", n_layers, first_thickness, expansion);

    // Report the geometric progression that was laid down.
    let mut t = first_thickness;
    let mut total = 0.0;
    let mut thicknesses = Vec::new();
    for _ in 0..n_layers {
        thicknesses.push(t);
        total += t;
        t *= expansion;
    }
    println!(
        "\nBoundary layers (n = {n_layers}, first = {first_thickness}, expansion = {expansion}):"
    );
    println!("  layer thicknesses  = {thicknesses:?}");
    println!("  total thickness    = {total:.4}");
    println!(
        "  cells              = {}  (+{} prisms)",
        layered.cell_count(),
        layered.cell_count() - base.cell_count()
    );
    println!(
        "  volume             = {:.4}  (repartition — preserved)",
        layered.total_volume()
    );

    // ---- Quality: is the layered mesh solvable? -----------------------------
    let q = check_quality(&layered);
    println!("\nQuality (checkMesh-style):");
    println!(
        "  max non-orthogonality = {:.2} deg",
        q.max_non_orthogonality_deg
    );
    println!("  max skewness          = {:.3}", q.max_skewness);
    println!(
        "  max aspect ratio      = {:.3}  (tall thin prisms are expected)",
        q.max_aspect_ratio
    );
    println!("  min cell volume       = {:.3e}", q.min_cell_volume);
    println!("  negative-volume cells = {}", q.n_negative_volume_cells);

    match layered.validate() {
        Ok(()) => println!("\nvalidate(): OK — every cell (interior + prism) is closed."),
        Err(e) => println!("\nvalidate(): {e}"),
    }
    println!("\nPipeline ran: shapes -> carve_box -> add_boundary_layers -> check_quality.");
}
