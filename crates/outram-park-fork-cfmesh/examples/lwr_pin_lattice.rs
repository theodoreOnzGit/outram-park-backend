//! LWR fuel-pin coolant mesh: carve the coolant region around a square lattice
//! of vertical fuel pins.
//!
//! The light-water-reactor analogue of the pebble-bed demo — a 3×3 pin bundle
//! in a coolant channel, meshed and quality-checked.
//!
//! ```text
//! cargo run -p outram-park-fork-cfmesh --example lwr_pin_lattice --release
//! ```

use outram_park_fork_cfmesh::carve::carve_around;
use outram_park_fork_cfmesh::checks::check_quality;
use outram_park_fork_cfmesh::reactor::{bounding_domain, pin_lattice};
use outram_park_fork_cfmesh::shapes::surface_volume;
use outram_park_fork_cfmesh::snap::snap_to_surface;

fn main() {
    // ---- A 3×3 fuel-pin bundle: r = 0.41, pitch 1.26 (PWR-like ratio). ------
    let pins = pin_lattice([3, 3], 1.26, 0.41, 4.0, 24);
    let domain = bounding_domain(&pins, 0.63); // half a pitch of coolant around
    println!(
        "LWR bundle: {} pins (r 0.41, pitch 1.26, height 4)",
        pins.len()
    );

    // ---- Carve the coolant region (domain minus every pin). -----------------
    let cell_size = 0.1;
    let carved = carve_around(&domain, &pins, cell_size);
    println!("\nCarve (cell size {cell_size}):");
    println!("  cells             = {}", carved.cell_count());
    println!(
        "  boundary patches  = {} (walls + one per pin)",
        carved.patches.len()
    );
    println!("  coolant volume    = {:.3}", carved.total_volume());
    let pin_vol: f64 = pins.iter().map(|(p, t)| surface_volume(p, t)).sum();
    let (dp, dt) = &domain;
    println!(
        "  (domain {:.3} − pins {:.3} = {:.3})",
        surface_volume(dp, dt),
        pin_vol,
        surface_volume(dp, dt) - pin_vol
    );

    // ---- Snap to the union of all surfaces, then check quality. -------------
    let mut all_pts = domain.0.clone();
    let mut all_tris = domain.1.clone();
    for (pp, pt) in &pins {
        let off = all_pts.len();
        all_pts.extend(pp.iter().copied());
        all_tris.extend(pt.iter().map(|t| [t[0] + off, t[1] + off, t[2] + off]));
    }
    let snapped = snap_to_surface(&carved, &all_pts, &all_tris);
    let q = check_quality(&snapped);
    println!("\nSnapped + checked:");
    println!("  volume            = {:.3}", snapped.total_volume());
    println!(
        "  max non-orth      = {:.1} deg",
        q.max_non_orthogonality_deg
    );
    println!("  negative cells    = {}", q.n_negative_volume_cells);
    println!("  solvable          = {}", q.is_solvable());
    match snapped.validate() {
        Ok(()) => println!("  validate()        = OK (all cells closed)"),
        Err(e) => println!("  validate()        = {e}"),
    }
    println!(
        "\nPipeline ran: {} pins -> carve_around -> snap -> quality.",
        pins.len()
    );
}
