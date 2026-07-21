//! TRISO doubly-heterogeneous transport by **stochastic-media** methods —
//! Chord Length Sampling (CLS) and Semi-Implicit CLS (SCLS) — measured against the
//! explicit random-packing reference.
//!
//! Run it:
//! ```text
//! cargo run -p outram-mc-libs --release --example triso_stochastic_media
//! ```
//!
//! This is the stochastic-media sibling of [`triso_delta_tracking`](triso_delta_tracking).
//! That example tracks the *explicit* geometry with delta (Woodcock) tracking — it
//! stores every kernel and looks each one up. This example asks the opposite question:
//! **how little of the geometry can a transport calculation get away with remembering?**
//! Read it top-to-bottom; it needs only the `stochastic` module.
//!
//! # The problem
//!
//! A TRISO compact packs O(10⁴) sub-millimetre fuel kernels into a matrix; a pebble
//! packs O(10⁴) of those and a core O(10⁵) pebbles. Storing every kernel is what the
//! explicit model does, and it is exact but heavy. The **stochastic-media** family trades
//! that memory for sampling, along a spectrum:
//!
//! | Model | Geometry stored | Memory |
//! |---|---|---|
//! | Explicit / RSA | every kernel | O(N) — the reference |
//! | **SCLS** | a moving local window of recent kernels | bounded |
//! | **CLS** | none — chord crossings re-sampled on the fly | O(1) |
//!
//! - **CLS** replaces geometry with a distribution: the distance to the next kernel
//!   surface is sampled from a chord-length distribution whose mean is fixed by the
//!   packing statistics (kernel radius + packing fraction). It is cheap and *memoryless*
//!   — a neutron that scatters backward does **not** re-encounter the kernel it just
//!   crossed.
//! - **SCLS** keeps the cheap chord sampling but **remembers** recently-encountered
//!   kernels in a window that follows the neutron (the Dynamic Inclusion Sphere), so a
//!   back-scattered neutron re-enters the same kernel — recovering the geometric
//!   correlation CLS discards, without storing the whole packing.
//!
//! # What this program does
//!
//! 1. Sweeps packing fraction and, at each point, builds one RSA packing and derives all
//!    three media from it, then runs an identical absorbing-kernel random walk on each
//!    ([`AbsorptionBenchmark`]) — kernels absorb, the matrix scatters isotropically. RSA
//!    is the reference; `|P_model − P_RSA|` is the model's error.
//! 2. Shows the SCLS memory directly: a neutron oscillating across a small region and the
//!    bounded set of kernels it keeps remembering.
//!
//! # Honesty banner
//!
//! **No model is claimed superior here.** CLS and SCLS are approximations; whether either
//! reproduces the explicit reference — and which wins — is regime-dependent, and the whole
//! point of running the sweep is to *measure* it rather than assert it. In the default
//! regime below you will typically see both land within a few percent of RSA, and *not*
//! always in the order intuition predicts.
//!
//! # Provenance
//!
//! Geometry model — random-packed TRISO fuel kernels in a matrix — follows the OpenMC
//! `triso.ipynb` notebook (openmc-dev/openmc-notebooks); kernel radius ~0.025 cm is a
//! representative TRISO UO₂ kernel. MIT-licensed OpenMC project material, used here as a
//! modelling reference and adapted to a tractable unit cell (cited per `RESPONSIBLE_USE.md`).
//! The CLS/SCLS methods themselves are **new work**, not an OpenMC port — upstream has no
//! chord-length sampling.

use outram_mc_libs::stochastic::benchmark::AbsorptionBenchmark;
use outram_mc_libs::stochastic::cls::ClsMedium;
use outram_mc_libs::stochastic::medium::MaterialId;
use outram_mc_libs::stochastic::scls::SclsMedium;
use outram_mc_libs::stochastic::spatial_index::SpatialIndex;
use outram_mc_libs::geometry::position::Position;

fn main() {
    println!("TRISO stochastic-media transport — RSA (explicit) vs CLS vs SCLS");
    println!("================================================================\n");

    // A representative TRISO cell: ~0.025 cm (250 µm) fuel kernels in a small cube,
    // an isotropically-scattering matrix, absorbing kernels.
    let base = AbsorptionBenchmark {
        domain_half_width: 0.3, // 0.6 cm cube
        particle_radius: 0.025, // ~250 µm TRISO kernel
        packing_fraction: 0.2,  // overwritten in the sweep
        scatter_mfp: 0.2,       // matrix scatter mean free path [cm]
        max_collisions: 300,
        histories: 5000,
    };
    let seed = 20260721;

    // ---- 1. Packing-fraction sweep: absorption probability per model ------------------
    println!("Absorbing-kernel random walk ({} histories/model, kernel r = {} cm,",
        base.histories, base.particle_radius);
    println!("matrix scatter mfp = {} cm). RSA is the explicit reference.\n", base.scatter_mfp);
    println!(
        "{:>5} | {:>12} | {:>12} {:>9} | {:>12} {:>9}",
        "pf", "RSA (ref)", "CLS", "ΔCLS", "SCLS", "ΔSCLS"
    );
    println!("{:-<5}-+-{:-<12}-+-{:-<12}-{:-<9}-+-{:-<12}-{:-<9}", "", "", "", "", "", "");

    for &pf in &[0.05_f64, 0.10, 0.15, 0.20, 0.25] {
        let bench = AbsorptionBenchmark { packing_fraction: pf, ..base };
        match bench.compare(seed) {
            Ok([rsa, cls, scls]) => {
                let d_cls = cls.absorption_probability - rsa.absorption_probability;
                let d_scls = scls.absorption_probability - rsa.absorption_probability;
                println!(
                    "{:>5.2} | {:>12.4} | {:>12.4} {:>+9.4} | {:>12.4} {:>+9.4}",
                    pf,
                    rsa.absorption_probability,
                    cls.absorption_probability, d_cls,
                    scls.absorption_probability, d_scls,
                );
            }
            Err(e) => println!("{pf:>5.2} | packing failed: {e}"),
        }
    }

    println!("\nΔ columns are the model's absorption error vs the explicit RSA reference.");
    println!("Both approximations track the reference; which is closer is regime-dependent");
    println!("(see the honesty banner in this file's docs — this is a measurement, not a claim).\n");

    // ---- 2. SCLS memory in action -----------------------------------------------------
    println!("SCLS memory: a neutron oscillating across a 0.4 cm region");
    println!("---------------------------------------------------------");
    let cls = ClsMedium::new(base.particle_radius, 0.2, MaterialId(1), MaterialId(0));
    // A window large enough to hold the oscillation region (no culling), so the retained
    // set shows saturation rather than unbounded growth.
    let mut scls = SclsMedium::new(cls, Position::ZERO, 100.0);
    let mut s = 424242u64;
    let sweeps = 300;
    let pts = 40;
    for sw in 0..sweeps {
        for i in 0..pts {
            let frac = i as f64 / pts as f64;
            let x = if sw % 2 == 0 { frac } else { 1.0 - frac } * 0.4;
            let _ = scls.material_at(Position::new(x, 0.0, 0.0), &mut s);
        }
    }
    let crossings = sweeps * pts;
    let remembered = scls.histories().len();
    println!("  {crossings} crossings of the region → {remembered} kernels remembered.");
    println!("  A memoryless model would keep inventing kernels; SCLS re-enters the ones it");
    println!("  already saw, so the retained set saturates (bounded memory).\n");

    // The retained kernels can be handed to the accelerated lookup index (kd-tree),
    // which returns exactly the brute-force answers — the seam SCLS uses at scale.
    let index = SpatialIndex::kd_tree(scls.histories().to_vec());
    let probe = Position::new(0.2, 0.0, 0.0);
    let hit = index.find_containing(probe).expect("kd-tree implemented");
    println!(
        "  kd-tree over the {} retained kernels: point (0.20,0,0) is {}.",
        index.len(),
        match hit {
            Some(h) => format!("inside a kernel centred at ({:.3},{:.3},{:.3})", h.center.x, h.center.y, h.center.z),
            None => "in the matrix".to_string(),
        }
    );

    println!("\nDone. See the module docs for the memory/fidelity spectrum and provenance.");
}
