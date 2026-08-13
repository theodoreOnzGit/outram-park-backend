//! Molten-salt-reactor coolant mesh: carve the salt region in a **cylindrical
//! vessel** threaded by internal channels (e.g. graphite/fuel channels).
//!
//! Completes the reactor trilogy (PBR pebbles, LWR pins, MSR channels): here the
//! *outer* surface is itself a cylinder — carve_around takes any closed surface,
//! not just a box.
//!
//! ```text
//! cargo run -p outram-park-fork-cfmesh --example msr_channels --release
//! ```

use outram_park_fork_cfmesh::carve::carve_around;
use outram_park_fork_cfmesh::checks::check_quality;
use outram_park_fork_cfmesh::math::Vec3;
use outram_park_fork_cfmesh::reactor::pin_lattice;
use outram_park_fork_cfmesh::shapes::{cylinder_surface, surface_volume};
use outram_park_fork_cfmesh::snap::snap_to_surface;

fn main() {
    // ---- Cylindrical vessel with a 2×2 array of internal channels. ----------
    let height = 4.0;
    let vessel = cylinder_surface(Vec3::new(1.5, 1.5, 0.0), 3.0, height, 48); // outer
    let channels = pin_lattice([2, 2], 3.0, 0.6, height, 24); // internal channels
    println!(
        "MSR vessel: cylinder r 3 h {height}, {} internal channels",
        channels.len()
    );

    // carve_around takes the cylindrical vessel as the outer surface.
    let cell_size = 0.2;
    let carved = carve_around(&vessel, &channels, cell_size);
    println!("\nCarve (cell size {cell_size}):");
    println!("  cells             = {}", carved.cell_count());
    println!(
        "  boundary patches  = {} (walls + one per channel)",
        carved.patches.len()
    );
    let vessel_vol = surface_volume(&vessel.0, &vessel.1);
    let chan_vol: f64 = channels.iter().map(|(p, t)| surface_volume(p, t)).sum();
    println!(
        "  salt volume       = {:.3}  (vessel {:.3} − channels {:.3} = {:.3})",
        carved.total_volume(),
        vessel_vol,
        chan_vol,
        vessel_vol - chan_vol
    );

    // ---- Snap + quality. ----------------------------------------------------
    let mut all_pts = vessel.0.clone();
    let mut all_tris = vessel.1.clone();
    for (pp, pt) in &channels {
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
        "\nPipeline ran: cylindrical vessel + {} channels -> carve_around -> snap -> quality.",
        channels.len()
    );
}
