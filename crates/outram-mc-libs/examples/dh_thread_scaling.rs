//! **Does the CLS/SCLS point-query lock actually cost anything?** — the
//! measurement GitHub issue #205 / bead `op-ifl3` rests on and never made.
//!
//! ```text
//! cargo run --release -p outram-mc-libs --example dh_thread_scaling
//! ```
//!
//! # The claim under test
//!
//! [`DhTreatment::ChordLength`] and [`DhTreatment::Scls`] hold their sampler
//! behind a `Mutex` because the point-query seam wants [`Sync`] and the sampler
//! is stateful. The issue argues that this (a) cannot scale on the
//! multi-threaded backend, because one lock serialises the hottest call in the
//! program, and (b) destroys reproducibility there, because threads take the
//! lock in arrival order and consume the RNG stream differently every run.
//!
//! Neither had been measured. The single-threaded timings that prompted the
//! issue cannot show it: **on one thread the lock is uncontended**, so it costs
//! an atomic compare-exchange and nothing else.
//!
//! # Three predictions, signed in advance
//!
//! 1. **Delta tracking scales** roughly with core count.
//! 2. **CLS and SCLS barely scale**, because every query queues on one lock.
//! 3. **CLS is not reproducible** multi-threaded: two runs with identical seeds
//!    differ by of order their own statistical error.
//!
//! If CLS scales fine, issue #205 is wrong about its own premise and should be
//! closed. That is the point of running it before redesigning anything.
//!
//! Deliberately small and data-light: this measures *scaling*, not physics, so
//! it uses the embedded LOW tier and a modest history count. The absolute
//! eigenvalues here are not V&V numbers and are not reported as such.

use std::time::Instant;

use outram_mc_libs::prelude::*;
use outram_mc_libs::material::material::NuclideComponent;
use outram_mc_libs::physics::compute::{ComputeType, ThreadCount};

const TEMP_K: f64 = 293.6;

fn nuclides() -> Vec<Nuclide> {
    ["U235", "U238", "O16", "C0", "C0", "Si28"]
        .iter()
        .map(|n| Nuclide::from_core(n).expect("embedded evaluation"))
        .collect()
}

fn materials() -> Vec<Material> {
    let m = |id: i32, name: &str, c: Vec<(usize, f64)>| Material {
        id,
        name: name.into(),
        temperature: TEMP_K,
        components: c
            .into_iter()
            .map(|(nuclide_idx, atom_density)| NuclideComponent { nuclide_idx, atom_density })
            .collect(),
    };
    vec![
        m(0, "UCO kernel", vec![(0, 4.40e-3), (1, 1.77e-2), (2, 2.27e-2), (3, 9.10e-3)]),
        m(1, "buffer", vec![(4, 5.02e-2)]),
        m(2, "IPyC", vec![(4, 9.53e-2)]),
        m(3, "SiC", vec![(5, 4.79e-2), (3, 4.79e-2)]),
        m(4, "OPyC", vec![(4, 9.53e-2)]),
        m(5, "matrix graphite", vec![(4, 8.53e-2)]),
        m(6, "shell graphite", vec![(4, 8.78e-2)]),
    ]
}

fn run(t: DhTreatment, mats: &[Material], nucs: &[Nuclide], compute: ComputeType, n: usize)
    -> (f64, f64, f64)
{
    let params = PebbleParams::fhr_reference().with_materials(mats.to_vec());
    let u = DhUniverse::pebble(params, t).expect("universe builds");
    let settings = KeffSettings {
        n_particles: n,
        n_inactive: 8,
        n_active: 20,
        temperature_k: TEMP_K,
        compute,
        ..KeffSettings::default()
    };
    let t0 = Instant::now();
    let r = u.keff(nucs, &settings);
    (r.k_mean, r.k_std, t0.elapsed().as_secs_f64())
}

fn main() {
    let n: usize = std::env::var("OUTRAM_DH_SCALING_HISTORIES")
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or(600);

    let nucs = nuclides();
    let mats = materials();
    let cores = std::thread::available_parallelism().map(|v| v.get()).unwrap_or(1);

    println!("CLS/SCLS lock: does it cost anything? (GH #205 / op-ifl3)");
    println!("=========================================================");
    println!("  {n} histories x [8 inactive + 20 active], LOW tier, bare FHR pebble");
    println!("  {cores} logical cores available\n");

    let treatments = [
        DhTreatment::DeltaTracking,
        DhTreatment::ChordLength,
        DhTreatment::Scls,
        DhTreatment::Homogenised,
    ];

    println!("  {:<28} {:>9} {:>9} {:>9}   {}", "treatment", "1 thread", "N threads", "speedup", "k (1T / NT)");
    println!("  {}", "-".repeat(84));
    let mut scaling = Vec::new();
    for t in treatments {
        let (k1, _, s1) = run(t, &mats, &nucs, ComputeType::CpuSingleThread, n);
        let (kn, _, sn) = run(t, &mats, &nucs, ComputeType::CpuMultiThread(ThreadCount::Auto), n);
        let speed = s1 / sn;
        println!(
            "  {:<28} {:>8.2}s {:>8.2}s {:>8.2}x   {:.5} / {:.5}",
            t.name(), s1, sn, speed, k1, kn
        );
        scaling.push((t, speed));
    }

    println!("\n=== Prediction 1: delta tracking scales with core count ===");
    let delta_speed = scaling[0].1;
    println!("  delta tracking {delta_speed:.2}x on {cores} cores — {}",
             if delta_speed > 1.5 { "SCALES" } else { "DOES NOT SCALE" });

    println!("\n=== Prediction 2: CLS and SCLS barely scale (one lock, hottest call) ===");
    for (t, speed) in scaling.iter().skip(1).take(2) {
        println!("  {:<28} {speed:.2}x   vs delta's {delta_speed:.2}x   — {}",
                 t.name(),
                 if *speed < 0.5 * delta_speed { "CONFIRMED, serialised" } else { "NOT confirmed" });
    }

    println!("\n=== Prediction 3: multi-threaded CLS is not reproducible ===");
    let (a, sa, _) = run(DhTreatment::ChordLength, &mats, &nucs,
                         ComputeType::CpuMultiThread(ThreadCount::Auto), n);
    let (b, sb, _) = run(DhTreatment::ChordLength, &mats, &nucs,
                         ComputeType::CpuMultiThread(ThreadCount::Auto), n);
    let dk = (a - b) * 1.0e5;
    let sigma = (sa * sa + sb * sb).sqrt() * 1.0e5;
    println!("  two identical-seed multi-threaded CLS runs: {a:.5} and {b:.5}");
    println!("  difference {dk:+.0} pcm against a statistical {sigma:.0} pcm");
    println!("  {}", if dk.abs() < 1.0e-9 {
        "IDENTICAL — reproducible, so prediction 3 is WRONG"
    } else {
        "DIFFERENT — not reproducible, prediction 3 confirmed"
    });

    // Same check on delta tracking, which has no lock: this is the control. If
    // delta also differs, the non-reproducibility is the backend's, not CLS's.
    let (c, _, _) = run(DhTreatment::DeltaTracking, &mats, &nucs,
                        ComputeType::CpuMultiThread(ThreadCount::Auto), n);
    let (d, _, _) = run(DhTreatment::DeltaTracking, &mats, &nucs,
                        ComputeType::CpuMultiThread(ThreadCount::Auto), n);
    println!("\n  CONTROL — delta tracking, no lock, same two runs: {c:.5} and {d:.5} ({:+.0} pcm)",
             (c - d) * 1.0e5);
    println!("  If the control also differs, the non-reproducibility belongs to the backend,");
    println!("  not to the lock, and prediction 3 is about the wrong thing.");
}
