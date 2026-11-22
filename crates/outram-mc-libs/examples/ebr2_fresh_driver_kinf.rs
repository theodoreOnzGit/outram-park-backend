//! # Fresh EBR-II MARK-II driver subassembly: k-infinity and the nuclear-data gap ablation (gh #1329)
//!
//! The first fresh-core EBR-II step (epic #1312, brief #1317 item 7): one
//! fresh MARK-II driver subassembly in an infinite lattice, isothermal 20 °C,
//! continuous-energy ENDF/B-VIII.0 processed here at 293.15 K with the
//! correct-physics defaults (URR probability tables and DBRC on; no physics
//! ablated). Model, inputs and citations:
//! `examples/common/ebr2_fresh_driver_model.rs`. Record:
//! `verification_and_validation/ebr2_fresh_driver/README.md`.
//!
//! > Research, education and V&V only. Not for reactor operation, licensing
//! > or any safety decision. AI draft, unreviewed; every input is an AI
//! > transcription.
//!
//! ```text
//! RAYON_NUM_THREADS=3 cargo run --release -p outram-mc-libs --example ebr2_fresh_driver_kinf
//! # particles per cycle, inactive, active (defaults 4000 30 120); arms (default all)
//! EBR2_ARMS=A,D cargo run --release -p outram-mc-libs --example ebr2_fresh_driver_kinf -- 4000 30 120
//! ```
//!
//! **What it does.** Draws the assembled geometry of arm A (three PNGs into
//! `verification_and_validation/ebr2_fresh_driver/`), processes every
//! nuclide any arm needs **once** (the arms share one temperature, so a
//! nuclide's data are the same in every arm), then runs k for each
//! [`GapTreatment`] arm with the same seed and settings and prints the table
//! the README records: k_inf ± σ (standard error over active cycles, and the
//! batch-means σ), and each arm minus arm A.
//!
//! **No reference.** There is no measured k_inf of a fresh MARK-II driver,
//! and ARC-226, ARC-228 and IAEA-TECDOC-1819 print no computed one
//! (searched 2026-11-10). The result is reported uncompared.
//!
//! # Results (2026-11-10, release, `-- 2000 20 60`, seed 1329, 3 threads)
//!
//! k_inf ± σ (standard error; batch-means σ in brackets), and minus arm A:
//!
//! | arm | treatment | k_inf | minus A [pcm] |
//! |---|---|---|---|
//! | A | reference: Ni, Mo, Zr natural; Ru, Rh, Pd, Nb omitted | **2.01400 ± 0.00278** (0.00377) | - |
//! | B | brief (a): Ni, Mo, Zr, Ru, Rh, Pd, Nb omitted | 2.04107 ± 0.00274 (0.00231) | +2707 ± 390 |
//! | C | brief (b): Mo-95 for Mo; Ni, Zr, Ru, Rh, Pd, Nb omitted | 2.01960 ± 0.00336 (0.00420) | +560 ± 436 |
//! | D | Ni omitted | 2.03168 ± 0.00323 (0.00354) | +1768 ± 426 |
//! | E | Mo-95 for all Mo | 2.00942 ± 0.00259 (0.00293) | -458 ± 380 |
//! | F | Mo omitted | 2.02939 ± 0.00263 (0.00276) | +1539 ± 382 |
//! | G | Ru/Rh/Pd/Nb as natural Mo | 2.00962 ± 0.00306 (0.00271) | -438 ± 413 |
//!
//! Predicted before running (gh #1329): A 1.6-1.8 (**missed**, high); B
//! +1500..+3500, C +500..+2500, D +400..+1500 (came out 0.6 σ above),
//! E -500..-1500, F +600..+2000, G -500..-1500. Every sign right; C, E and
//! G are not resolved from zero at this statistics. Quote A, 2.014 ± 0.004,
//! uncompared (no reference exists), with G as a lower bound on the
//! omitted Ru/Rh/Pd/Nb. Hardware: CPU only, 4 cores, Intel Xeon @ 2.10 GHz,
//! 15.7 GiB, Linux, shared and loaded; 265 s processing, 429-780 s per arm.
//! Full record: `verification_and_validation/ebr2_fresh_driver/README.md`.
//!
//! `main` and its printing are CLI glue, untested by design (workspace rule
//! exception); every function of the model it runs is reached by
//! `tests/model_ebr2_fresh_driver.rs`.

#[path = "common/ebr2_fresh_driver_model.rs"]
#[allow(dead_code)]
mod ebr2;

use std::path::Path;
use std::time::Instant;

use ebr2::*;
use njoy_outram_park_fork::reference_data::{ace_submodule_dir, reference_endf_dir};
use outram_mc_libs::geometry::plot::PlotBasis;
use outram_mc_libs::geometry::position::Position;
use outram_mc_libs::model::prelude::*;
use outram_mc_libs::model::NuclideKey;
use rayon::prelude::*;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<usize> = std::env::args().skip(1).filter_map(|a| a.parse().ok()).collect();
    let particles = args.first().copied().unwrap_or(4000);
    let inactive = args.get(1).copied().unwrap_or(30);
    let active = args.get(2).copied().unwrap_or(120);
    let seed = 1_329_u64;
    let wanted: Option<Vec<String>> = std::env::var("EBR2_ARMS")
        .ok()
        .map(|s| s.split(',').map(|a| a.trim().to_string()).collect());
    let arms: Vec<GapTreatment> = GapTreatment::arms()
        .into_iter()
        .filter(|t| wanted.as_ref().is_none_or(|w| w.iter().any(|a| a == t.arm)))
        .collect();

    let endf = reference_endf_dir();
    let ace = ace_submodule_dir().join("endf/endf-b-viii.0");
    let data = library(&endf, &ace)?;
    println!("library: {}", data.label());

    // Pictures of what the solver sees (arm A; the arms differ only in
    // composition).
    let a_model = build_model(data.clone(), &GapTreatment::on_hand())?;
    let assembled_a = a_model.assemble()?;
    draw(&assembled_a)?;

    // Assemble every arm; process the union of their nuclides once.
    let mut assembled = Vec::new();
    for t in &arms {
        assembled.push((*t, build_model(data.clone(), t)?.assemble()?));
    }
    let mut keys: Vec<NuclideKey> = Vec::new();
    for (_, a) in &assembled {
        for k in &a.nuclides {
            if !keys.contains(k) {
                keys.push(*k);
            }
        }
    }
    let clock = Instant::now();
    println!("processing {} nuclides at {TEMPERATURE_K} K (correct physics on)", keys.len());
    let processed: Vec<(NuclideKey, outram_mc_libs::material::nuclide::Nuclide)> = keys
        .par_iter()
        .map(|k| {
            let t = Instant::now();
            let n = data.load_nuclide(k.isotope, TEMPERATURE_K, k.law);
            println!("  {} in {:.1} s", k.isotope.name(), t.elapsed().as_secs_f64());
            n.map(|n| (*k, n))
        })
        .collect::<Result<_, _>>()?;
    let processing_s = clock.elapsed().as_secs_f64();
    println!("processed in {processing_s:.1} s");

    let settings = KeffSettings::new(
        MCSettings::Custom(CustomRun {
            particles,
            inactive,
            active,
            seed,
        }),
        None,
    )
    .silent();
    let mut rows: Vec<(GapTreatment, Keff, Option<f64>, f64, usize)> = Vec::new();
    for (t, a) in &assembled {
        let nuclides: Vec<_> = a
            .nuclides
            .iter()
            .map(|k| {
                processed
                    .iter()
                    .find(|(p, _)| p == k)
                    .map(|(_, n)| n.clone())
                    .ok_or("nuclide not processed")
            })
            .collect::<Result<_, _>>()?;
        let clock = Instant::now();
        let run = a.keff_with(&nuclides, &settings)?;
        let secs = clock.elapsed().as_secs_f64();
        let k = run.keff();
        let p = &run.provenance;
        let urr = p.physics.iter().filter(|n| n.urr_probability_tables).count();
        let dbrc = p.physics.iter().filter(|n| n.dbrc.is_ok()).count();
        println!(
            "arm {}: k_inf = {k} (batch-means sigma {}), {secs:.1} s on {} threads; \
             {} nuclides, URR tables on {urr}, DBRC on {dbrc}; ablations {:?}",
            t.arm,
            p.sigma_batch_means.map_or("n/a".into(), |s| format!("{s:.5}")),
            p.threads,
            p.physics.len(),
            p.ablations
        );
        rows.push((*t, k, p.sigma_batch_means, secs, p.threads));
    }

    println!();
    println!(
        "{particles} particles x [{inactive} inactive + {active} active], seed {seed}, \
         {TEMPERATURE_K} K; processing {processing_s:.0} s"
    );
    println!(
        "hardware: {}",
        outram_mc_libs::perf_report::HardwareInfo::detect().headline()
    );
    println!("| arm | treatment | k_inf | sigma (s/sqrt n) | sigma (batch means) | minus A [pcm] |");
    println!("|---|---|---|---|---|---|");
    let base = rows.iter().find(|r| r.0.arm == "A").map(|r| r.1);
    for (t, k, bm, _, _) in &rows {
        let diff = base.filter(|_| t.arm != "A").map_or(String::from("-"), |b| {
            let d = (k.mean - b.mean) * 1e5;
            let s = (k.std * k.std + b.std * b.std).sqrt() * 1e5;
            format!("{d:+.0} ± {s:.0}")
        });
        println!(
            "| {} | {} | {:.5} | {:.5} | {} | {diff} |",
            t.arm,
            t.description,
            k.mean,
            k.std,
            bm.map_or("n/a".into(), |s| format!("{s:.5}"))
        );
    }
    Ok(())
}

/// Draw arm A's assembled geometry: the whole lattice cell (x-y at
/// mid-height), an x-z slice through the centre row of pins, and a zoom on
/// a corner pin, the duct and the sodium between ducts. Glue around the
/// ported plotter (untested by design, as `main`).
fn draw(a: &AssembledModel) -> Result<(), Box<dyn std::error::Error>> {
    let out = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("verification_and_validation/ebr2_fresh_driver");
    std::fs::create_dir_all(&out)?;
    let sa = driver_subassembly();
    let corner = sa.pin_centres_cm()[0];
    let pitch_cm = SA_PITCH_MM / 10.0;
    for (img, file) in [
        (
            a.draw_window(
                PlotBasis::Xy,
                Position::new(0.0, 0.0, 0.0),
                [7.0, 7.0],
                1200,
                &format!(
                    "EBR-II fresh MARK-II driver, z = 0: {} pins, pin pitch {:.4} cm, SA pitch {pitch_cm} cm",
                    sa.pin_count(),
                    pin_pitch_mm() / 10.0
                ),
            ),
            "ebr2_fresh_driver_xy.png",
        ),
        (
            a.draw_window(
                PlotBasis::Xz,
                Position::new(0.0, 0.0, 0.0),
                [7.0, 36.0],
                1200,
                "x-z at y = 0: centre row of 11 pins, 34.29 cm drawn (reflective ends), axes not to scale",
            ),
            "ebr2_fresh_driver_xz.png",
        ),
        (
            a.draw_window(
                PlotBasis::Xy,
                Position::new(corner[0], corner[1], 0.0),
                [1.6, 1.6],
                900,
                &format!(
                    "corner pin at ({:.3}, {:.3}) cm: slug r {:.4}, clad {:.4} to {:.4} cm, duct, Na between ducts",
                    corner[0],
                    corner[1],
                    SLUG_DIAMETER_MM / 20.0,
                    (CLAD_OD_MM / 2.0 - CLAD_WALL_MM) / 10.0,
                    CLAD_OD_MM / 20.0
                ),
            ),
            "ebr2_fresh_driver_pin_xy.png",
        ),
    ] {
        let p = out.join(file);
        img.write_png(&p)?;
        println!("drew {}", p.display());
    }
    Ok(())
}
