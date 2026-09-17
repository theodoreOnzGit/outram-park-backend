//! What fraction of a uniform source box lands in FISSIONABLE material?
//! Diagnostic for the HTR-10 k = 0 failure (`bn:op-867c`).
use nee_soon::htr10_rmc::core_model::assemble_explicit_triso;
use outram_mc_libs::geometry::cell::SurfaceToken;
use outram_mc_libs::geometry::position::{Direction, Position};
use outram_mc_libs::rng::lcg::prn;

const FISSILE: usize = 0; // the UO2 kernel slot

fn main() {
    let core = assemble_explicit_triso(8, 12, usize::MAX);
    let u = Direction::new(0.0, 0.0, 1.0);
    let mut seed = 1_u64;
    const N: usize = 4_000_000;
    let (mut hits, mut lost) = (0usize, 0usize);
    for _ in 0..N {
        let p = Position::new(
            -50.0 + 100.0 * prn(&mut seed),
            -50.0 + 100.0 * prn(&mut seed),
            -50.0 + 100.0 * prn(&mut seed),
        );
        match core.geometry.locate(p, u, SurfaceToken::NONE).and_then(|q| q.material) {
            Some(FISSILE) => hits += 1,
            Some(_) => {}
            None => lost += 1,
        }
    }
    let p_acc = hits as f64 / N as f64;
    println!("uniform source box [-50, 50]^3, {N} samples");
    println!("  landed in fissionable material : {hits}  ({p_acc:.3e})");
    println!("  lost (outside geometry)        : {lost}");
    println!();
    for n_part in [800usize, 3000, 20000] {
        let budget = n_part * 10_000;
        let expect = budget as f64 * p_acc;
        println!(
            "  {n_part:>6} particles: guard allows {budget:>12} attempts, expect {expect:>9.0} accepts \
             -> {}",
            if expect >= n_part as f64 { "fills" } else { "STARVES" }
        );
    }
}
