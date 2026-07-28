//! Pebble-bed reactor coolant mesh: carve the coolant region around a
//! structured packing of fuel pebbles.
//!
//! Demonstrates the multi-hole path scaling to reactor geometry (bbox-culled
//! inside test), producing a body-fitted coolant mesh with per-pebble boundary
//! patches — the substrate for coupled TH/neutronics of a pebble bed.
//!
//! ```text
//! cargo run -p outram-park-fork-cfmesh --example pebble_bed --release
//! ```

use outram_park_fork_cfmesh::carve::carve_around;
use outram_park_fork_cfmesh::checks::check_quality;
use outram_park_fork_cfmesh::reactor::{bounding_domain, sphere_packing};
use outram_park_fork_cfmesh::snap::snap_to_surface;

fn main() {
    // ---- A 3×3×3 structured packing of pebbles, with coolant gaps. ----------
    let pebbles = sphere_packing([3, 3, 3], 2.0, 0.85, 12, 24);
    let domain = bounding_domain(&pebbles, 1.0);
    println!("Pebble bed: {} pebbles (radius 0.85, spacing 2)", pebbles.len());

    // ---- Carve the coolant region (domain minus every pebble). --------------
    let cell_size = 0.2;
    let carved = carve_around(&domain, &pebbles, cell_size);
    println!("\nCarve (cell size {cell_size}):");
    println!("  cells             = {}", carved.cell_count());
    println!("  boundary patches  = {} (walls + one per pebble)", carved.patches.len());
    println!("  coolant volume    = {:.3}", carved.total_volume());

    // ---- Snap to the union of all surfaces, then check quality. -------------
    let mut all_pts = domain.0.clone();
    let mut all_tris = domain.1.clone();
    for (pp, pt) in &pebbles {
        let off = all_pts.len();
        all_pts.extend(pp.iter().copied());
        all_tris.extend(pt.iter().map(|t| [t[0] + off, t[1] + off, t[2] + off]));
    }
    let snapped = snap_to_surface(&carved, &all_pts, &all_tris);
    let q = check_quality(&snapped);
    println!("\nSnapped + checked:");
    println!("  volume            = {:.3}", snapped.total_volume());
    println!("  max non-orth      = {:.1} deg", q.max_non_orthogonality_deg);
    println!("  negative cells    = {}", q.n_negative_volume_cells);
    println!("  solvable          = {}", q.is_solvable());
    match snapped.validate() {
        Ok(()) => println!("  validate()        = OK (all cells closed)"),
        Err(e) => println!("  validate()        = {e}"),
    }
    println!("\nPipeline ran: {} pebbles -> carve_around -> snap -> quality.", pebbles.len());
}
