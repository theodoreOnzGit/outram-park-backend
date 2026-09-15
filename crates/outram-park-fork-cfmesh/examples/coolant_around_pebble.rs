//! End-to-end reactor-geometry demo: mesh the **coolant region around a
//! pebble** and report its quality.
//!
//! Runs the whole volume-meshing pipeline with only this crate (no GUI, no other
//! crate, no cargo feature):
//!
//! ```text
//! shapes (domain box + pebble sphere)
//!   -> carve_region  (keep cells inside the box AND outside the pebble)
//!   -> snap_to_surface  (body-fit the staircase onto the surfaces)
//!   -> check_quality  (is the mesh solvable?)
//! ```
//!
//! Run it with:
//!
//! ```text
//! cargo run -p outram-park-fork-cfmesh --example coolant_around_pebble --release
//! ```

use outram_park_fork_cfmesh::carve::carve_region;
use outram_park_fork_cfmesh::checks::check_quality;
use outram_park_fork_cfmesh::math::Vec3;
use outram_park_fork_cfmesh::shapes::{box_surface, sphere_surface, surface_volume};
use outram_park_fork_cfmesh::snap::snap_to_surface;
use std::f64::consts::PI;

fn main() {
    // ---- Geometry: a cubic coolant domain with a single central pebble. -----
    let (domain_pts, domain_tris) =
        box_surface(Vec3::new(-2.0, -2.0, -2.0), Vec3::new(2.0, 2.0, 2.0));
    let pebble_radius = 1.0;
    let (pebble_pts, pebble_tris) = sphere_surface(Vec3::ZERO, pebble_radius, 20, 40);

    let domain_vol = surface_volume(&domain_pts, &domain_tris);
    let pebble_vol = surface_volume(&pebble_pts, &pebble_tris);
    println!("Geometry:");
    println!("  domain box volume  = {domain_vol:.4}");
    println!(
        "  pebble volume      = {pebble_vol:.4}  (analytic (4/3)pi r^3 = {:.4})",
        4.0 / 3.0 * PI
    );
    println!("  expected coolant   = {:.4}", domain_vol - pebble_vol);

    // ---- Carve the coolant region (inside the box, outside the pebble). -----
    let cell_size = 0.1;
    let carved = carve_region(
        &domain_pts,
        &domain_tris,
        &[(&pebble_pts[..], &pebble_tris[..])],
        cell_size,
    );
    println!("\nCarve (cell size {cell_size}):");
    println!("  cells              = {}", carved.cell_count());
    println!(
        "  faces              = {} ({} internal, {} boundary)",
        carved.face_count(),
        carved.n_internal_faces(),
        carved.n_boundary_faces()
    );
    println!("  staircase volume   = {:.4}", carved.total_volume());
    // Boundary is split into BC-ready patches: the domain wall and the pebble.
    println!("  boundary patches:");
    for p in &carved.patches {
        let role = if p.name == "walls" {
            "domain wall"
        } else {
            "pebble surface"
        };
        println!("    {:<8} {:>6} faces  ({role})", p.name, p.n_faces);
    }

    // ---- Snap the staircase onto both surfaces (body-fit). ------------------
    // Snap to the union of the two surfaces so every boundary point lands on
    // whichever surface (domain wall or pebble) is nearest.
    let mut all_pts = domain_pts.clone();
    let mut all_tris = domain_tris.clone();
    let offset = all_pts.len();
    all_pts.extend(pebble_pts.iter().copied());
    all_tris.extend(
        pebble_tris
            .iter()
            .map(|t| [t[0] + offset, t[1] + offset, t[2] + offset]),
    );
    let snapped = snap_to_surface(&carved, &all_pts, &all_tris);
    println!("\nSnap (body-fitted):");
    println!("  volume             = {:.4}", snapped.total_volume());

    // ---- Quality: is it solvable? -------------------------------------------
    let q = check_quality(&snapped);
    println!("\nQuality (checkMesh-style):");
    println!(
        "  max non-orthogonality = {:.2} deg",
        q.max_non_orthogonality_deg
    );
    println!("  max skewness          = {:.3}", q.max_skewness);
    println!("  max aspect ratio      = {:.3}", q.max_aspect_ratio);
    println!("  min cell volume       = {:.3e}", q.min_cell_volume);
    println!("  negative-volume cells = {}", q.n_negative_volume_cells);
    println!("  solvable (checkMesh thresholds): {}", q.is_solvable());

    match snapped.validate() {
        Ok(()) => println!("\nvalidate(): OK — every cell is closed."),
        Err(e) => println!("\nvalidate(): {e}"),
    }
    println!("\nPipeline ran: shapes -> carve_region -> snap -> check_quality.");
}
