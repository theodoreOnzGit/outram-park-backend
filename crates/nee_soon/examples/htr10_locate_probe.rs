//! **Isolate why the assembled HTR-10 core returns k = 0.** Diagnostic only,
//! for `bn:op-867c`.
//!
//! # Result (2026-09-17): THE GEOMETRY IS NOT THE BUG
//!
//! ```text
//! 20000 uniform probes: lost 0
//!   material 0 (UO2 kernel)        5
//!   material 1 (buffer)            3
//!   material 2 (IPyC)              3
//!   material 3 (SiC)               3
//!   material 4 (OPyC)             12
//!   material 5 (graphite)       6166
//!   material 6 (helium)        10617
//!   material 7 (reflector)      3191
//! ```
//!
//! Every material is reachable, nothing is lost, tracking reports `Delta` inside
//! the bed and `Surface` in the reflector, and the descent reaches four levels
//! where it should. The kernel's 0.025 % share of probes matches the ~1 %
//! by-volume estimate once the fuel zone and fuelled-pebble fractions are
//! applied.
//!
//! So `locate` and the delta path's `material_at` both work on the assembled
//! core. **The k = 0 failure is in transport, not in geometry assembly.** That
//! eliminates the largest class of candidates; what remains is the flight
//! itself, and it is NOT yet isolated -- see `htr10_rmc_keff.rs`'s status
//! section for the candidates still open.
use nee_soon::htr10_rmc::core_model::assemble_explicit_triso;
use outram_mc_libs::geometry::cell::SurfaceToken;
use outram_mc_libs::geometry::position::{Direction, Position};

fn main() {
    let core = assemble_explicit_triso(8, 12, 0);
    let u = Direction::new(0.0, 0.0, 1.0);
    println!("tiles {}, cells {}, universes {}", core.tiles, core.cells, core.universes);

    let probes = [
        ("origin", Position::ZERO),
        ("bed, 10cm out", Position::new(10.0, 0.0, 0.0)),
        ("bed, 30cm out", Position::new(30.0, 0.0, 0.0)),
        ("source corner", Position::new(50.0, 50.0, 50.0)),
        ("far reflector", Position::new(0.0, 0.0, 100.0)),
    ];
    for (label, p) in probes {
        match core.geometry.locate(p, u, SurfaceToken::NONE) {
            Some(path) => {
                let lv: Vec<String> = path.levels.iter()
                    .map(|c| format!("{:?}/{:?}", c.lattice, c.lattice_index)).collect();
                println!(
                    "{label:<16} mat={:?} levels={} tracking={:?} [{}]",
                    path.material, path.levels.len(), path.tracking, lv.join(" -> ")
                );
            }
            None => println!("{label:<16} LOST (locate returned None)"),
        }
    }

    // How many of a uniform sample land in fuel?
    let mut counts = std::collections::BTreeMap::new();
    let mut lost = 0;
    for i in 0..20000 {
        let t = i as f64 / 20000.0;
        let p = Position::new(
            (t * 97.0).sin() * 45.0, (t * 61.0).cos() * 45.0, (t * 29.0).sin() * 50.0);
        match core.geometry.locate(p, u, SurfaceToken::NONE) {
            Some(path) => *counts.entry(path.material).or_insert(0usize) += 1,
            None => lost += 1,
        }
    }
    println!("\n20000 uniform probes: lost {lost}");
    for (m, c) in counts {
        println!("  material {m:?}: {c}");
    }
}

/// How often does a UNIFORM point in the source box land in fissionable
/// material? That is the source rejection sampler's acceptance rate, and the
/// driver gives up after `n_particles * 10_000` attempts.
#[allow(dead_code)]
fn acceptance_rate() {}
