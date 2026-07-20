//! Author a few mesh primitives and inspect their topology.
//!
//! This is the top-to-bottom entry point for the crate: it needs only
//! `outram-blender` itself (no other crate, no GUI). Run it with:
//!
//! ```text
//! cargo run -p outram-blender --example authoring_primitives --release
//! ```
//!
//! It builds each of the four real generators in [`outram_blender::primitives`]
//! and prints the vertex/edge/face counts and Euler characteristic, showing
//! that the closed solids satisfy `V - E + F = 2` and the flat grid patch (a
//! disc) satisfies `V - E + F = 1`.

use outram_blender::primitives;

fn main() {
    // Each generator returns a `Mesh` built purely through the public
    // add_vertex / add_face API, so edge deduplication is automatic.
    let cube = primitives::cube(1.0);
    let sphere = primitives::uv_sphere(32, 16, 1.0);
    let cylinder = primitives::cylinder(24, 0.5, 2.0);
    let grid = primitives::grid(4, 4, 2.0);

    for (name, m) in [
        ("cube", &cube),
        ("uv_sphere(32,16)", &sphere),
        ("cylinder(24)", &cylinder),
        ("grid(4x4)", &grid),
    ] {
        println!(
            "{name:>18}: V={:>4}  E={:>4}  F={:>4}  chi = V - E + F = {}",
            m.vertex_count(),
            m.edge_count(),
            m.face_count(),
            m.euler_characteristic(),
        );
    }

    // The three closed solids are topological spheres (chi = 2); the grid is a
    // disc (chi = 1). These are exact topological identities.
    assert_eq!(cube.euler_characteristic(), 2);
    assert_eq!(sphere.euler_characteristic(), 2);
    assert_eq!(cylinder.euler_characteristic(), 2);
    assert_eq!(grid.euler_characteristic(), 1);
    println!("\nAll Euler-characteristic checks passed.");
}
