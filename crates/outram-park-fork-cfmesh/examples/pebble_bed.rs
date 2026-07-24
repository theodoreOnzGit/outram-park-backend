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

use outram_park_fork_cfmesh::carve::carve_region;
use outram_park_fork_cfmesh::checks::check_quality;
use outram_park_fork_cfmesh::math::Vec3;
use outram_park_fork_cfmesh::shapes::{box_surface, sphere_surface};
use outram_park_fork_cfmesh::snap::snap_to_surface;

fn main() {
    // ---- A 3×3×3 structured packing of pebbles in a cubic domain. -----------
    let n = 3; // pebbles per axis
    let spacing = 2.0; // centre-to-centre
    let radius = 0.85; // pebble radius (< spacing/2, so coolant gaps remain)
    let margin = 1.0; // domain padding around the packing

    let lo = -margin;
    let hi = (n - 1) as f64 * spacing + margin;
    let (domain_pts, domain_tris) = box_surface(Vec3::new(lo, lo, lo), Vec3::new(hi, hi, hi));

    // Build the pebble surfaces (owned), then borrow them for carve_region.
    let mut pebbles: Vec<(Vec<Vec3>, Vec<[usize; 3]>)> = Vec::new();
    for i in 0..n {
        for j in 0..n {
            for k in 0..n {
                let c = Vec3::new(i as f64 * spacing, j as f64 * spacing, k as f64 * spacing);
                pebbles.push(sphere_surface(c, radius, 12, 24));
            }
        }
    }
    let hole_refs: Vec<(&[Vec3], &[[usize; 3]])> =
        pebbles.iter().map(|(p, t)| (&p[..], &t[..])).collect();
    println!("Pebble bed: {}³ = {} pebbles, radius {radius}, spacing {spacing}", n, n * n * n);

    // ---- Carve the coolant region (domain minus every pebble). --------------
    let cell_size = 0.2;
    let carved = carve_region(&domain_pts, &domain_tris, &hole_refs, cell_size);
    println!("\nCarve (cell size {cell_size}):");
    println!("  cells             = {}", carved.cell_count());
    println!("  boundary patches  = {} (walls + one per pebble)", carved.patches.len());
    println!("  coolant volume    = {:.3}", carved.total_volume());

    // ---- Snap to the union of all surfaces, then check quality. -------------
    let mut all_pts = domain_pts.clone();
    let mut all_tris = domain_tris.clone();
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
    println!("\nPipeline ran: {} pebbles -> carve_region -> snap -> quality.", n * n * n);
}
