// SPDX-License-Identifier: GPL-3.0

//! **Paired analog-vs-survival-biased ablation** — is the variance reduction
//! unbiased, and does it pay? GitHub #258.
//!
//! # Methodology
//!
//! Bare HEU sphere at Godiva's critical radius, U-235 + U-238 from
//! `reference-data/endf/` at 293.6 K, 2000 histories × [15 inactive + 40
//! active]. Two arms over the same seeds, differing **only** in
//! `KeffSettings::variance_reduction`:
//!
//! - **ANALOG** — the default; capture kills the particle.
//! - **SURVIVAL** — `survival_biasing: true` with upstream's defaults
//!   (`weight_cutoff = 0.25`, `weight_survive = 1.0`).
//!
//! Reported per arm: the ensemble mean of `k` with the standard error of that
//! mean, the seed-to-seed standard deviation, the wall-clock, and the figure
//! of merit `FOM = 1 / (sd² · t)`.
//!
//! The comparison that matters is the **unbiasedness**: two estimators of one
//! eigenvalue must agree. A resolved difference is a defect, not a saving.
//!
//! # The prediction, made before this was ever run
//!
//! `verification_and_validation/variance_reduction/prediction_before_measuring.md`
//! — difference zero, sd down 1.2–1.5×, cost up 1.5–3×, and **the FOM
//! predicted not to pay on this problem** (×0.5 to ×1.5).
//!
//! # Results
//!
//! `verification_and_validation/variance_reduction/ablation_2026_09_22.md`.
//!
//! ```text
//! cargo run --release -p outram-mc-libs --example variance_reduction_ablation -- 16
//! ```

use std::time::Instant;

use outram_mc_libs::geometry::cell::{Cell, HalfSpaceSense, RegionToken};
use outram_mc_libs::geometry::geometry::Geometry;
use outram_mc_libs::geometry::position::Position;
use outram_mc_libs::geometry::surface::{BoundaryType, Sphere, SurfaceKind};
use outram_mc_libs::geometry::universe::Universe;
use outram_mc_libs::material::material::{Material, NuclideComponent};
use outram_mc_libs::material::nuclide::Nuclide;
use outram_mc_libs::physics::keff::{ComputeType, KeffSettings};
use outram_mc_libs::physics::transport_csg::{run_keff_csg, SourceBox};
use outram_mc_libs::physics::variance_reduction::VarianceReduction;

const R_FUEL: f64 = 8.7407;
const TEMP: f64 = 293.6;

fn heu() -> Option<Vec<Nuclide>> {
    let base =
        std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../reference-data/endf");
    let load = |name: &str, f: &str| -> Option<Nuclide> {
        let p = base.join(f);
        p.exists().then_some(())?;
        Nuclide::from_endf_file(&p, name, TEMP, 1.0e-3).ok()
    };
    Some(vec![
        load("U235", "n-092_U_235-ENDF8.0.endf")?,
        load("U238", "n-092_U_238.endf")?,
    ])
}

fn model() -> (Geometry, Vec<Material>) {
    let geom = Geometry {
        surfaces: vec![SurfaceKind::Sphere(Sphere {
            x0: 0.0,
            y0: 0.0,
            z0: 0.0,
            r: R_FUEL,
            bc: BoundaryType::Vacuum,
        })],
        cells: vec![Cell::material(
            1,
            vec![RegionToken::HalfSpace {
                surface_idx: 0,
                sense: HalfSpaceSense::Inside,
            }],
            0,
            TEMP,
        )],
        universes: vec![Universe {
            id: 0,
            cell_indices: vec![0],
        }],
        lattices: vec![],
        root_universe: 0,
    };
    let mats = vec![Material {
        id: 1,
        name: "HEU".into(),
        components: vec![
            NuclideComponent {
                nuclide_idx: 0,
                atom_density: 4.4994e-2,
            },
            NuclideComponent {
                nuclide_idx: 1,
                atom_density: 2.4984e-3,
            },
        ],
        temperature: TEMP,
    }];
    (geom, mats)
}

fn settings(seed: u64, n_particles: usize, vr: VarianceReduction) -> KeffSettings {
    KeffSettings {
        n_particles,
        n_inactive: 15,
        n_active: 40,
        temperature_k: TEMP,
        seed,
        compute: ComputeType::CpuSingleThread,
        variance_reduction: vr,
        ..KeffSettings::default()
    }
}

fn stats(v: &[f64]) -> (f64, f64, f64) {
    let n = v.len() as f64;
    let mean = v.iter().sum::<f64>() / n;
    if v.len() < 2 {
        return (mean, f64::NAN, f64::NAN);
    }
    let var = v.iter().map(|x| (x - mean).powi(2)).sum::<f64>() / (n - 1.0);
    (mean, var.sqrt(), (var / n).sqrt())
}

fn main() {
    let n_seeds: u64 = std::env::args()
        .nth(1)
        .and_then(|s| s.parse().ok())
        .unwrap_or(16);
    // Second argument: histories per generation. It is a knob because the
    // fission-bank population-control bias of power iteration scales as 1/N,
    // and that is a *hypothesis about the residual* this study has to be able
    // to test rather than assert — see the V&V write-up.
    let n_particles: usize = std::env::args()
        .nth(2)
        .and_then(|s| s.parse().ok())
        .unwrap_or(2000);

    let Some(nucs) = heu() else {
        eprintln!("SKIP: ENDF tapes not in this checkout");
        return;
    };
    let (geom, mats) = model();
    let src = SourceBox {
        lower: Position::new(-R_FUEL, -R_FUEL, -R_FUEL),
        upper: Position::new(R_FUEL, R_FUEL, R_FUEL),
    };

    let survival = VarianceReduction {
        survival_biasing: true,
        ..VarianceReduction::default()
    };
    survival.validate().expect("upstream's own defaults are valid");

    println!("GitHub #258 — analog vs survival biasing, paired over {n_seeds} seeds");
    println!("  survival: cutoff {} survive {}", survival.weight_cutoff, survival.weight_survive);

    let mut k_analog = Vec::new();
    let mut k_survival = Vec::new();
    let (mut t_analog, mut t_survival) = (0.0_f64, 0.0_f64);

    for s in 0..n_seeds {
        let seed = 20_260_922 + s * 7919;

        let t0 = Instant::now();
        let a = run_keff_csg(
            &geom, &mats, &nucs, src,
            &settings(seed, n_particles, VarianceReduction::default()), None,
        );
        t_analog += t0.elapsed().as_secs_f64();

        let t0 = Instant::now();
        let b = run_keff_csg(
            &geom, &mats, &nucs, src,
            &settings(seed, n_particles, survival.clone()), None,
        );
        t_survival += t0.elapsed().as_secs_f64();

        k_analog.push(a.k_mean);
        k_survival.push(b.k_mean);
        println!("  seed {seed}: analog {:.5}  survival {:.5}", a.k_mean, b.k_mean);
    }

    let (ma, sda, sema) = stats(&k_analog);
    let (ms, sds, sems) = stats(&k_survival);
    let fom_a = 1.0 / (sda * sda * t_analog);
    let fom_s = 1.0 / (sds * sds * t_survival);

    println!("\n=== RESULTS ({n_seeds} seeds per arm) ===");
    println!(
        "  ANALOG    k = {ma:.5} +/- {sema:.5}   sd {sda:.5}   {t_analog:.1} s   FOM {fom_a:.3e}"
    );
    println!(
        "  SURVIVAL  k = {ms:.5} +/- {sems:.5}   sd {sds:.5}   {t_survival:.1} s   FOM {fom_s:.3e}"
    );

    let d = ms - ma;
    let sd_d = (sema * sema + sems * sems).sqrt();
    println!(
        "\n  BIAS CHECK: survival - analog = {:+.0} +/- {:.0} pcm ({:.2} sigma)",
        1.0e5 * d,
        1.0e5 * sd_d,
        (d / sd_d).abs()
    );

    // ── The PAIRED statistic, and the assumption the unpaired one rests on ──
    //
    // Both arms run on the SAME seed list, so `sqrt(sema^2 + sems^2)` above is
    // the error of the difference only if the two arms are uncorrelated
    // seed-by-seed. That is plausible — survival biasing consumes a different
    // number of random draws, so the streams diverge after the first few
    // collisions — but plausible is not measured, and the direction matters:
    // a POSITIVE correlation would make the true error SMALLER than quoted,
    // which would make a residual that reads "consistent with zero" not be.
    //
    // So compute the difference's standard error directly from the per-seed
    // differences, which needs no independence assumption at all, and report
    // the correlation alongside it. If the two error bars agree, the unpaired
    // number every earlier row in this study quoted is vindicated; if they do
    // not, the paired one is right and the earlier rows need re-reading.
    let diffs: Vec<f64> = k_survival
        .iter()
        .zip(k_analog.iter())
        .map(|(s, a)| s - a)
        .collect();
    let (md, sdd, semd) = stats(&diffs);
    let rho = {
        let n = k_analog.len() as f64;
        if n < 2.0 || sda == 0.0 || sds == 0.0 {
            0.0
        } else {
            let cov: f64 = k_analog
                .iter()
                .zip(k_survival.iter())
                .map(|(a, b)| (a - ma) * (b - ms))
                .sum::<f64>()
                / (n - 1.0);
            cov / (sda * sds)
        }
    };
    println!(
        "  PAIRED     : survival - analog = {:+.0} +/- {:.0} pcm ({:.2} sigma), \
         seed-to-seed sd {:.0} pcm",
        1.0e5 * md,
        1.0e5 * semd,
        (md / semd).abs(),
        1.0e5 * sdd
    );
    println!(
        "  correlation between arms over seeds = {rho:+.3}  \
         (unpaired error is right only near 0; paired/unpaired = {:.2}x)",
        semd / sd_d
    );
    println!("  sd ratio  (analog / survival) = {:.2}x", sda / sds);
    println!("  cost ratio (survival / analog) = {:.2}x", t_survival / t_analog);
    println!("  FOM ratio (survival / analog)  = {:.2}x", fom_s / fom_a);
}
