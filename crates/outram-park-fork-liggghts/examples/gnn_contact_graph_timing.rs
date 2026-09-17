//! Times the GNN contact-graph bridge on the full HTR-10 bed, and compares it
//! against the DEM's own contact detection.
//!
//! ```bash
//! cargo run --release -p outram-park-fork-liggghts --example gnn_contact_graph_timing
//! ```
//!
//! Needs `reference-data/liggghts/htr10_settled.csv` (27 564 settled pebbles at
//! the published HTR-10 core geometry); it prints a notice and exits if that is
//! not checked out.
//!
//! # Why this exists
//!
//! [`gnn_bridge::contact_graph`] is the entry point a DEM surrogate is trained
//! through, so its cost at reactor scale is worth knowing rather than guessing,
//! and the graph it produces is worth cross-checking against the contact set the
//! force loop actually uses.
//!
//! # Measured 2026-09-17
//!
//! Machine: Intel Xeon @ 2.80 GHz, **4 cores**, 15 GB RAM; `--release`;
//! single-threaded (neither path is parallelised). 27 564 pebbles. Runtimes in
//! this workspace are machine-dependent by rule — re-measure rather than
//! trusting these on different hardware.
//!
//! | call | time | result |
//! |---|---|---|
//! | `contact_graph(skin = 0)` | 1.52 s | 133 686 directed edges |
//! | `contact_graph(skin = 3 mm)` | 1.47 s | 177 750 edges |
//! | `contact_graph(skin = 18 mm)` | 1.47 s | 280 376 edges |
//! | `reach_bound` | 1.53 s | required 1 hop (configured 4), hop 0.06 m |
//! | `GranularSystem::contact_pairs` | **0.0278 s** | 66 843 pairs, `Z = 4.850` |
//!
//! **The two agree exactly.** 133 686 directed edges / 2 = 66 843 undirected,
//! which is the DEM's pair count to the pair — two independent contact
//! detections landing on the same network.
//!
//! **`contact_graph` is O(N²) and that is its whole cost.**
//! `raffles::gnn::Graph::contact_graph` is a plain double loop over all pairs
//! with no spatial hashing: 3.80e8 tests here at ~4 ns each. Hence the time is
//! flat at ~1.5 s while the skin takes the edge count from 133 k to 280 k — the
//! edge collection is noise beside the scan. `reach_bound`'s 1.53 s is almost
//! entirely its internal `contact_graph(0.0)`; the bound itself is arithmetic.
//!
//! Fine as a one-off analysis. Called **per timestep** it would cost
//! `1.5 s x 40 000 = ~17 h` on this bed, and it grows as N², so a 100 k-pebble
//! bed is ~20 s a call. If that ever matters, the fix is to build the edge list
//! with this crate's cell lists and hand it to `Graph::from_undirected_edges`;
//! the exact edge-count agreement above is the check that it would be the same
//! graph.

use outram_park_fork_liggghts::gnn_bridge;
use outram_park_fork_liggghts::particle::{Particle, Vec3};
use uom::si::f64::{Length, Mass, ThermodynamicTemperature};
use uom::si::{length::meter, mass::kilogram, thermodynamic_temperature::kelvin};
const R_P: f64 = 0.03;
const RHO: f64 = 1730.0;
fn main() {
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../reference-data/liggghts/htr10_settled.csv");
    let text = std::fs::read_to_string(path).expect("htr10_settled.csv");
    let mass = RHO * 4.0 / 3.0 * std::f64::consts::PI * R_P.powi(3);
    let t0 = std::time::Instant::now();
    let ps: Vec<Particle> = text
        .lines()
        .skip(1)
        .filter(|l| !l.trim().is_empty())
        .map(|l| {
            let f: Vec<f64> = l.split(',').map(|v| v.parse().unwrap()).collect();
            Particle::new(
                Vec3::new(f[1], f[2], f[3]),
                Vec3::new(f[4], f[5], f[6]),
                Vec3::zero(),
                Mass::new::<kilogram>(mass),
                Length::new::<meter>(R_P),
                ThermodynamicTemperature::new::<kelvin>(300.0),
            )
            .unwrap()
        })
        .collect();
    println!(
        "loaded {} pebbles in {:.3} s",
        ps.len(),
        t0.elapsed().as_secs_f64()
    );

    for skin in [0.0, 0.003, 0.018] {
        let t = std::time::Instant::now();
        let g = gnn_bridge::contact_graph(&ps, skin).expect("graph");
        let dt = t.elapsed().as_secs_f64();
        println!(
            "contact_graph(skin={skin:.3} m): {:>10.4} s   nodes={} edges={}",
            dt,
            g.node_count(),
            g.edge_count()
        );
    }

    let t = std::time::Instant::now();
    let b = gnn_bridge::reach_bound(&ps, 1.0e8, RHO, 1.0e-4, Some(4)).expect("reach");
    println!(
        "reach_bound: {:>10.4} s   required={} configured={:?} hop={:.4} m",
        t.elapsed().as_secs_f64(),
        b.required,
        b.configured,
        b.hop_length
    );

    // The DEM's own contact detection, for comparison: cell lists, O(N).
    use outram_park_fork_liggghts::boundary::Boundary;
    use outram_park_fork_liggghts::granular::{GranularContactModel, GranularMaterial};
    use outram_park_fork_liggghts::granular_system::GranularSystem;
    let m = GranularMaterial::new(1.0e8, 0.2, 0.5, 0.4).unwrap();
    let sys = GranularSystem::new(
        ps.clone(),
        vec![
            Boundary::cylinder(Vec3::zero(), Vec3::new(0.0, 0.0, 1.0), 0.90).unwrap(),
            Boundary::wall(Vec3::zero(), Vec3::new(0.0, 0.0, 1.0)).unwrap(),
        ],
        GranularContactModel::hertz_history(m),
        Vec3::new(0.0, 0.0, -9.81),
        1.0e-4,
    )
    .unwrap();
    let t = std::time::Instant::now();
    let pairs = sys.contact_pairs();
    let dt_dem = t.elapsed().as_secs_f64();
    println!(
        "DEM contact_pairs (cell lists): {:>8.4} s   pairs={}  Z={:.3}",
        dt_dem,
        pairs.len(),
        sys.coordination_number()
    );
    let n = ps.len() as f64;
    println!(
        "pair tests: GNN O(N^2) = {:.3e}   DEM O(N) cell lists",
        n * (n - 1.0) / 2.0
    );
}
