//! **Price free-gas target motion on Godiva** — a paired-seed ablation of
//! [`Nuclide::with_target_at_rest`], the in-process hook that landed with
//! `b774d32` (coverage gap 3 of `docs/neutronics-physics-coverage.md`).
//!
//! Run 1 of job 4 in `docs/handoff-heavy-neutronics-runs.md`. **This is a
//! harness check, not a physics result**, and it is the cheaper half of a pair:
//! the measurement that matters is
//! `examples/fhr_pebble_target_at_rest_ablation.rs`, and this run exists so
//! that a null result there could not be blamed on a dead switch.
//!
//! # What is being ablated
//!
//! Two arms over the **same seeds**, identical in every other respect. The
//! ablation is one call — `.map(Nuclide::with_target_at_rest)` — which makes
//! [`Nuclide::free_gas_kt`] return `0`, so `free_gas_elastic_scatter` falls
//! through to its target-at-rest branch. The ablation is expressed **through
//! the production code path**; no branch is added to the transport kernel, and
//! no cross section moves (they were Doppler-broadened at construction and are
//! temperature-independent at lookup).
//!
//! Before `b774d32` this was reachable only by zeroing one example's transport
//! temperature through `OUTRAM_RINGRPT_TARGET_AT_REST`, a process-wide switch
//! that cannot put two arms in one paired-seed study.
//!
//! # Prediction on record, made before any measurement
//!
//! **~zero.** Godiva's flux is almost entirely above
//! `FREE_GAS_THRESHOLD · kT` = 10.12 eV at 293.6 K, where the production path
//! already holds a heavy target at rest.
//! `tests/ablation_hook_controls.rs` proves the two arms are **bit-identical**
//! above that threshold — 2048/2048 draws, RNG streams in lockstep.
//!
//! **A non-zero reading here is a bug report, not a measurement**: it means the
//! flag is reaching a path it should not.
//!
//! # Statistics
//!
//! Unlike the angular ablations, this hook **does not preserve the RNG
//! stream** — the free-gas kernel draws a target velocity the target-at-rest
//! kernel never draws, so the arms diverge at the first thermal collision below
//! the threshold. The difference is attributable **over an ensemble of seeds
//! only**, never history by history. Both the paired and unpaired figures are
//! printed and the run says which is tighter.
//!
//! # Results (2026-09-17, 128 seeds per arm, ENDF/B-VIII.0)
//!
//! 4 cores; 141.2 s nuclear data + 1372.7 + 1355.6 s transport.
//! Control fired: kinematics `kT 2.530049e-2 -> 0.0` eV on all three nuclides.
//!
//! | arm | n | mean vs ICSBEP | sd | sem |
//! |---|---|---|---|---|
//! | **MOVING** (the physics) | 128 | **+1 pcm** | 200 | ±18 |
//! | **AT-REST** (ablated) | 128 | **+1 pcm** | 200 | ±18 |
//! | difference, unpaired | | +0 pcm | | ±25 (0.0σ) |
//! | **difference, paired** | | **+0 pcm** | **paired sd 5** | **±0 (1.0σ)** |
//!
//! **The prediction HELD**, and bounded below **1 pcm at 3σ**.
//!
//! The load-bearing number is the **paired sd of 5 pcm against either arm's
//! 200**. This is the *only* ablation in this study where pairing works — the
//! continuum angular law, ν̄ and χ all have paired sd exceeding their arms'
//! (261, 247, 246). Five pcm is the transport-level confirmation of what
//! `tests/ablation_hook_controls.rs` shows at the kernel level: above
//! `FREE_GAS_THRESHOLD·kT` = 10.12 eV the two kernels are bit-identical
//! (2048/2048 draws, RNG in lockstep), so only a negligible sub-threshold tail
//! can differ.
//!
//! So the switch is **alive but reaches nothing here**: the kT control fires on
//! every nuclide while the eigenvalue does not move. That is the precondition
//! the FHR run needs — a null there cannot be blamed on a dead switch.
//!
//! ```text
//! OUTRAM_GODIVA_SEEDS=128 cargo run --release -p outram-mc-libs \
//!     --features endf-pebble-cases --example godiva_target_at_rest_ablation
//! ```

#[cfg(target_os = "android")]
fn main() {
    println!("godiva_target_at_rest_ablation is desktop-only (reads reference-data/endf/).");
}

#[cfg(not(target_os = "android"))]
fn main() {
    desktop::run();
}

#[cfg(not(target_os = "android"))]
mod desktop {
    use njoy_outram_park_fork::reference_data::reference_endf;
    use outram_mc_libs::material::material::{Material, NuclideComponent};
    use outram_mc_libs::material::nuclide::Nuclide;
    use outram_mc_libs::physics::keff::{run_keff, KeffSettings};
    use std::sync::Arc;
    use std::time::Instant;

    /// Godiva's fuel temperature \[K\] — the tapes are broadened to it.
    const TEMP_K: f64 = 293.6;
    /// ICSBEP HEU-MET-FAST-001 sphere radius \[cm\].
    const RADIUS_CM: f64 = 8.7407;

    /// `(tape file, nuclide name, atom density \[atoms/barn·cm\])`.
    const NUCLIDES: &[(&str, &str, f64)] = &[
        ("n-092_U_234-ENDF8.0.endf", "U234", 4.9184e-4),
        ("n-092_U_235-ENDF8.0.endf", "U235", 4.4994e-2),
        ("n-092_U_238.endf", "U238", 2.4984e-3),
    ];

    fn workers() -> usize {
        std::env::var("OUTRAM_GODIVA_WORKERS")
            .ok()
            .and_then(|v| v.parse::<usize>().ok())
            .filter(|&w| w > 0)
            .unwrap_or_else(|| {
                std::thread::available_parallelism()
                    .map(|n| n.get())
                    .unwrap_or(4)
            })
    }

    fn stats(x: &[f64]) -> (f64, f64, f64) {
        let n = x.len() as f64;
        let mean = x.iter().sum::<f64>() / n;
        let var = x.iter().map(|v| (v - mean).powi(2)).sum::<f64>() / (n - 1.0).max(1.0);
        let sd = var.sqrt();
        (mean, sd, sd / n.sqrt())
    }

    fn run_arm(nuclides: &Arc<Vec<Nuclide>>, material: &Material, seeds: &[u64]) -> Vec<f64> {
        let mut out = vec![0.0; seeds.len()];
        let chunk = seeds.len().div_ceil(workers());
        std::thread::scope(|s| {
            for (sd_chunk, out_chunk) in seeds.chunks(chunk).zip(out.chunks_mut(chunk)) {
                let nuclides = Arc::clone(nuclides);
                s.spawn(move || {
                    for (k, &seed) in sd_chunk.iter().enumerate() {
                        let settings = KeffSettings {
                            n_particles: 5000,
                            n_inactive: 40,
                            n_active: 120,
                            temperature_k: TEMP_K,
                            seed,
                            ..KeffSettings::default()
                        };
                        let r = run_keff(RADIUS_CM, material, &nuclides, &settings);
                        out_chunk[k] = (r.k_mean - 1.0) * 1.0e5;
                    }
                });
            }
        });
        out
    }

    pub fn run() {
        let n_seeds: usize = std::env::var("OUTRAM_GODIVA_SEEDS")
            .ok()
            .and_then(|v| v.parse().ok())
            .unwrap_or(16);

        println!("Reconstructing HEU isotopes from ENDF/B-VIII.0 (RECONR + BROADR @ {TEMP_K} K)…");
        let t0 = Instant::now();
        let mut moving = Vec::new();
        for &(file, name, _) in NUCLIDES {
            let Some(p) = reference_endf(file) else {
                println!("  missing {file} — set OUTRAM_PARK_ENDF_DIR; skipping.");
                return;
            };
            moving.push(
                Nuclide::from_endf_file(&p, name, TEMP_K, 1.0e-3)
                    .unwrap_or_else(|e| panic!("from_endf_file({}): {e}", p.display())),
            );
        }

        let at_rest: Vec<Nuclide> = moving
            .iter()
            .cloned()
            .map(Nuclide::with_target_at_rest)
            .collect();

        // ── Controls, before any transport ──
        //
        // "There was something to remove" is the assertion usually skipped and
        // the one that matters most: a hook that removes nothing reports "no
        // difference", and that reads as "this physics does not matter". Here
        // the mechanism is the kinematics temperature, so the control is that
        // it was non-zero and is now zero, on every nuclide.
        assert!(
            moving.iter().all(|n| !n.is_target_at_rest()),
            "the unablated arm already holds its targets at rest"
        );
        assert!(
            at_rest.iter().all(|n| n.is_target_at_rest()),
            "with_target_at_rest did not take on every nuclide -- a partial no-op"
        );
        for (m, r) in moving.iter().zip(&at_rest) {
            assert!(
                m.free_gas_kt(TEMP_K) > 0.0,
                "{}: kinematics kT is already zero before the ablation",
                m.name
            );
            assert_eq!(
                r.free_gas_kt(TEMP_K),
                0.0,
                "{}: kinematics kT survived the ablation",
                r.name
            );
            println!(
                "  {:<5} kinematics kT {:.6e} -> {:.1} eV",
                m.name,
                m.free_gas_kt(TEMP_K),
                r.free_gas_kt(TEMP_K)
            );
        }
        println!(
            "Nuclear data ready in {:.1} s.\n",
            t0.elapsed().as_secs_f64()
        );

        let material = Material {
            id: 1,
            name: "Godiva HEU".into(),
            temperature: TEMP_K,
            components: NUCLIDES
                .iter()
                .enumerate()
                .map(|(i, &(_, _, rho))| NuclideComponent {
                    nuclide_idx: i,
                    atom_density: rho,
                })
                .collect(),
        };

        let seeds: Vec<u64> = (1..=n_seeds as u64).collect();
        let moving = Arc::new(moving);
        let at_rest = Arc::new(at_rest);

        println!(
            "{n_seeds} seeds per arm on {} workers, 5000 histories × [40 inactive + 120 active]…",
            workers()
        );
        let t = Instant::now();
        let a = run_arm(&moving, &material, &seeds);
        println!("  MOVING  arm done in {:.1} s", t.elapsed().as_secs_f64());
        let t = Instant::now();
        let b = run_arm(&at_rest, &material, &seeds);
        println!("  AT-REST arm done in {:.1} s\n", t.elapsed().as_secs_f64());

        let (ma, sa, ea) = stats(&a);
        let (mb, sb, eb) = stats(&b);
        let diff = mb - ma;
        let ediff = (ea * ea + eb * eb).sqrt();
        let d: Vec<f64> = b.iter().zip(&a).map(|(x, y)| x - y).collect();
        let (md, sdp, edp) = stats(&d);

        println!("Δk from ICSBEP HEU-MET-FAST-001 = 1.0000, pcm");
        println!("  arm        n     mean      sd     sem");
        println!(
            "  MOVING   {:>3}   {:+7.0}  {:>6.0}  {:>6.0}",
            a.len(),
            ma,
            sa,
            ea
        );
        println!(
            "  AT-REST  {:>3}   {:+7.0}  {:>6.0}  {:>6.0}",
            b.len(),
            mb,
            sb,
            eb
        );
        println!();
        println!(
            "  difference (AT-REST − MOVING), unpaired = {:+.0} ± {:.0} pcm  ({:.1} sigma)",
            diff,
            ediff,
            (diff / ediff).abs()
        );
        println!(
            "  difference (AT-REST − MOVING), paired   = {:+.0} ± {:.0} pcm  ({:.1} sigma), \
             paired sd {:.0}",
            md,
            edp,
            (md / edp).abs(),
            sdp
        );
        let (quote, qerr) = if sdp > sa.max(sb) {
            println!(
                "  -> paired sd ({sdp:.0}) EXCEEDS either arm's ({:.0}); the seeds do not pair, \
                 so quote the UNPAIRED figure.",
                sa.max(sb)
            );
            (diff, ediff)
        } else {
            println!(
                "  -> paired sd ({sdp:.0}) is below either arm's ({:.0}); quote the PAIRED \
                 figure.",
                sa.max(sb)
            );
            (md, edp)
        };

        println!();
        println!("  Prediction on record, made before any measurement: ~ZERO. Godiva's flux is");
        println!("  almost entirely above FREE_GAS_THRESHOLD*kT = 10.12 eV at 293.6 K, where the");
        println!("  production path already holds a heavy target at rest, and the control test");
        println!("  shows the two arms bit-identical above that threshold (2048/2048 draws).");
        println!("  A NON-ZERO READING HERE IS A BUG REPORT, NOT A MEASUREMENT.");
        println!();
        println!("  VERDICT:");
        if quote.abs() < 3.0 * qerr {
            println!(
                "    HELD. {quote:+.0} ± {qerr:.0} pcm is consistent with zero; bounded below \
                 {:.0} pcm at 3 sigma.",
                3.0 * qerr
            );
            println!(
                "    The switch is alive (the kT control above fired) and reaches nothing on \
                 this case,"
            );
            println!(
                "    which is what makes a null on the FHR pebble readable as physics rather \
                 than a dead switch."
            );
        } else {
            println!(
                "    FAILED: {quote:+.0} ± {qerr:.0} pcm at {:.1} sigma is resolved away from \
                 zero.",
                (quote / qerr).abs()
            );
            println!(
                "    Per the hand-off this is a BUG REPORT -- the flag is reaching a path it \
                 should not."
            );
            println!(
                "    Do not widen the interpretation to make it a measurement; find the path."
            );
        }
    }
}
