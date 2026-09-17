// SPDX-License-Identifier: GPL-3.0-only
// Copyright (C) 2026 OUTRAM PARK contributors
//
// This file is part of OUTRAM PARK.
//
// OUTRAM PARK is free software: you can redistribute it and/or modify it
// under the terms of the GNU General Public License as published by the
// Free Software Foundation, either version 3 of the License, or (at your
// option) any later version.
//
// OUTRAM PARK is distributed in the hope that it will be useful, but
// WITHOUT ANY WARRANTY; without even the implied warranty of
// MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE.  See the GNU
// General Public License for more details.
//
// You should have received a copy of the GNU General Public License along
// with OUTRAM PARK.  If not, see <https://www.gnu.org/licenses/>.

//! # HTR-10 bed structure — radial distribution function `g(r)`
//!
//! ## Why a packing fraction is not the answer on its own
//!
//! The HTR-10 work has so far been judged on one number, the solid fraction
//! `φ`. Two beds can share a `φ` and be structurally different — one ordered,
//! one genuinely random — and for a pebble-bed reactor that difference governs
//! flow resistance, effective conductivity and neutron streaming. `g(r)`
//! resolves it, and it is the standard instrument for doing so.
//!
//! This suite computes `g(r)` for **every** bed the HTR-10 study produces and
//! writes them to a single tidy dataset,
//! `reference-data/liggghts/htr10_rdf.csv`, so the settings can be plotted
//! against each other.
//!
//! ## What the curve must show, and what would be a defect
//!
//! | feature | expected | meaning |
//! |---|---|---|
//! | `g(r) = 0` below `~0.99 d` | yes | see the soft-sphere note below — **not** exactly `d` |
//! | sharp peak at `r ≈ d` | yes | the contact shell |
//! | **split second peak** at `√3 d` and `2 d` | yes | the signature of a **random** packing |
//! | peak at `√2 d ≈ 1.414 d` | **no** | would indicate FCC/HCP **crystallisation** |
//! | `g → 1` at large `r` | yes | by construction of the normalisation; a check on the estimator |
//!
//! The `√3 d` / `2 d` versus `√2 d` distinction is the load-bearing one. A bed
//! that densifies toward the published filling fraction *by crystallising* is a
//! different physical claim from one that densifies while staying random, and
//! `φ` alone cannot tell them apart. This is what makes the dataset worth
//! committing rather than computing ad hoc.
//!
//! ## These are SOFT spheres: `g(r)` is not zero right up to `d`
//!
//! The first run of this suite asserted `g(r) = 0` for all `r < d`, on the
//! hard-sphere reasoning above, and **failed**: the LIGGGHTS reference bed has
//! `g(0.0594 m) = 17.59`, i.e. pairs 0.6 mm closer than touching.
//!
//! That is not a defect, it is the model. These runs use the standard
//! pebble-bed DEM softening, `E = 5e8 Pa` against nuclear graphite's ~9 GPa,
//! and `tests/htr10_pebble_bed.rs` separately measures the resulting maximum
//! contact overlap at **1.71 % of the pebble radius** — 0.51 mm, so a closest
//! approach of `0.9914 d`. The measured `g` agrees with that independently
//! derived bound.
//!
//! So this suite asserts the *physical* bound (no pair closer than `0.95 d`,
//! which a real defect would breach by far) and **reports** the measured
//! closest approach. That turns `g(r)` into a second, independent check on the
//! stiffness-softening assumption: the crate currently guards it with a single
//! maximum overlap, while the near-contact part of `g(r)` is the whole
//! distribution of overlaps in the bed.
//!
//! ## Estimator
//!
//! See [`outram_park_fork_liggghts::rdf`]. In short: shell centres are
//! restricted to pebbles at least `r_max` from every boundary so that every
//! shell lies wholly inside the bed and the normalisation is exact with **no
//! boundary correction**; all pebbles remain available as neighbours; the
//! number density is measured over that same eroded region.
//!
//! The domain is taken as the **cylindrical core only** (`z >= 0`, above the
//! conus inlet) even for the beds that sit in the discharge conus, because the
//! conus is not a cylinder and treating it as one would break the erosion
//! guarantee the estimator depends on. Pebbles inside the conus still count as
//! neighbours; they simply do not serve as shell centres.
//!
//! ## Results
//!
//! Recorded in `crates/outram-park-fork-liggghts/docs/verification-and-validation.md`.

use outram_park_fork_liggghts::compute::{ComputeType, ThreadCount};
use outram_park_fork_liggghts::particle::Vec3;
use outram_park_fork_liggghts::rdf::{radial_distribution, RdfDomain, RdfSettings};

/// Pebble diameter `[m]` — HTR-10 design point.
const D: f64 = 0.06;
/// Core barrel radius `[m]`.
const R_CORE: f64 = 0.90;

fn data_path(name: &str) -> std::path::PathBuf {
    std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../reference-data/liggghts")
        .join(name)
}

fn load_positions(name: &str) -> Option<Vec<Vec3>> {
    let text = std::fs::read_to_string(data_path(name)).ok()?;
    let mut rows: Vec<(usize, Vec3)> = text
        .lines()
        .skip(1)
        .filter(|l| !l.trim().is_empty())
        .map(|l| {
            let f: Vec<f64> = l.split(',').map(|v| v.parse().expect("numeric")).collect();
            (f[0] as usize, Vec3::new(f[1], f[2], f[3]))
        })
        .collect();
    rows.sort_by_key(|r| r.0);
    Some(rows.into_iter().map(|r| r.1).collect())
}

/// Settings for an HTR-10 bed: the cylindrical core only, out to `5 d`.
fn settings_for(centres: &[Vec3]) -> RdfSettings {
    let z_max = centres
        .iter()
        .map(|c| c.z)
        .fold(f64::NEG_INFINITY, f64::max);
    RdfSettings::for_diameter(D, RdfDomain::cylinder(R_CORE, 0.0, z_max))
}

/// Every bed this study produces, as `(file, case label, mu, mu_r, stage)`.
///
/// Missing files are skipped with a note rather than failing: a fresh clone has
/// only the committed LIGGGHTS reference beds, while the sweep outputs appear
/// once `examples/htr10_recirculation_sweep.rs` has been run.
const BEDS: &[(&str, &str, &str, &str, &str)] = &[
    ("htr10_settled.csv", "liggghts_settled", "0.4", "0.1", "settled_flat_floor"),
    ("htr10_settled_ours.csv", "ours_settled", "0.4", "0.1", "settled_flat_floor"),
    ("htr10_conus_settled_liggghts.csv", "liggghts_conus", "0.4", "0.1", "conus_slump"),
    ("htr10_conus_presettled_mu40_mur10.csv", "ours_conus_mu40_mur10", "0.4", "0.1", "conus_slump"),
    ("htr10_conus_presettled_mu40_mur00.csv", "ours_conus_mu40_mur00", "0.4", "0.0", "conus_slump"),
    ("htr10_conus_presettled_mu10_mur10.csv", "ours_conus_mu10_mur10", "0.1", "0.1", "conus_slump"),
    ("htr10_conus_presettled_mu10_mur00.csv", "ours_conus_mu10_mur00", "0.1", "0.0", "conus_slump"),
    ("htr10_recirc_mu40_mur10_bed.csv", "ours_recirc_mu40_mur10", "0.4", "0.1", "recirculated"),
    ("htr10_recirc_mu40_mur00_bed.csv", "ours_recirc_mu40_mur00", "0.4", "0.0", "recirculated"),
    ("htr10_recirc_mu10_mur10_bed.csv", "ours_recirc_mu10_mur10", "0.1", "0.1", "recirculated"),
    ("htr10_recirc_mu10_mur00_bed.csv", "ours_recirc_mu10_mur00", "0.1", "0.0", "recirculated"),
];

/// Compute `g(r)` for every available HTR-10 bed and write the combined tidy
/// dataset the V&V document plots.
///
/// **Methodology.** As the module docs. Every bed uses identical settings
/// (`r_max = 5 d`, 250 bins, cylindrical core domain) so the curves are
/// directly comparable; the only thing that varies between rows is the bed.
///
/// **Pass criteria**, applied to every bed found:
/// 1. no pair closer than `0.95 d` — the soft-sphere overlap bound. These are
///    compressible contacts, not hard spheres (see the module docs); at the
///    measured 1.71 % of radius the closest approach should be `~0.9914 d`, so
///    `0.95 d` is loose enough to be a real defect check and tight enough to
///    catch a bed that has gone through itself;
/// 2. the global maximum of `g` sits in the contact shell, within two bin
///    widths of `d`;
/// 3. `g → 1` far out: the mean of `g` over the last half-diameter is within
///    15 % of 1, which is a check on the *estimator*, not on the bed.
///
/// **This asserts nothing about `φ` and nothing about crystallinity.** Whether
/// recirculation densifies the bed, and whether it does so by ordering it, are
/// the questions the dataset exists to answer; asserting an answer would
/// destroy the measurement.
#[test]
fn radial_distribution_of_every_htr10_bed() {
    let mut csv = String::from("case,mu,mu_r,stage,n_pebbles,n_centres,r_m,r_over_d,g,coordination,counts\n");
    let mut found = 0usize;

    for (file, case, mu, mu_r, stage) in BEDS {
        let Some(centres) = load_positions(file) else {
            eprintln!("skip {file}: not present");
            continue;
        };
        found += 1;
        let settings = settings_for(&centres);
        let rdf = radial_distribution(
            &centres,
            settings,
            ComputeType::CpuMultiThread(ThreadCount::Auto),
        );
        assert!(
            rdf.n_centres > 500,
            "{case}: only {} shell centres survived erosion — too few for a curve",
            rdf.n_centres
        );

        let bin_of = |r: f64| (r / rdf.dr) as usize;
        // (1) soft-sphere overlap bound. The closest approach is measured and
        //     reported; only a gross breach fails.
        let first_occupied = rdf.counts.iter().position(|&c| c > 0);
        let closest = first_occupied.map_or(f64::NAN, |k| k as f64 * rdf.dr);
        for k in 0..bin_of(0.95 * D) {
            assert!(
                rdf.counts[k] == 0,
                "{case}: {} pairs at r = {:.4} m, closer than 0.95 d — beyond any soft-sphere \
                 overlap; the bed has gone through itself",
                rdf.counts[k],
                rdf.r[k]
            );
        }
        // (2) the contact peak is at d
        let peak = rdf.peak_r();
        assert!(
            (peak - D).abs() <= 2.0 * rdf.dr,
            "{case}: contact peak at {peak:.4} m, not at d = {D} m"
        );
        // (3) the estimator returns to 1 far out
        let tail_from = bin_of(settings.r_max - 0.5 * D);
        let tail: f64 = rdf.g[tail_from..].iter().sum::<f64>() / (rdf.g.len() - tail_from) as f64;
        assert!(
            (tail - 1.0).abs() < 0.15,
            "{case}: g -> {tail:.4} at large r, not 1 — the normalisation is wrong"
        );

        let coord = rdf.contact_coordination().unwrap_or(f64::NAN);
        eprintln!(
            "{case:28} N {:6}  centres {:6}  peak g {:6.3} at r/d {:.3}  coordination {coord:5.2}  \
             closest r/d {:.4}  tail {tail:.4}",
            centres.len(),
            rdf.n_centres,
            rdf.g.iter().cloned().fold(0.0, f64::max),
            peak / D,
            closest / D,
        );

        for k in 0..rdf.r.len() {
            csv.push_str(&format!(
                "{case},{mu},{mu_r},{stage},{},{},{:.6},{:.4},{:.6},{:.6},{}\n",
                centres.len(),
                rdf.n_centres,
                rdf.r[k],
                rdf.r[k] / D,
                rdf.g[k],
                rdf.coordination[k],
                rdf.counts[k]
            ));
        }
    }

    assert!(
        found > 0,
        "no HTR-10 bed found at all — reference-data/liggghts is empty?"
    );
    std::fs::write(data_path("htr10_rdf.csv"), &csv).expect("write htr10_rdf.csv");
    eprintln!("wrote reference-data/liggghts/htr10_rdf.csv for {found} bed(s)");
}

/// The two CPU backends must produce the **identical** histogram.
///
/// Binning is integer counting and integer addition is associative, so a
/// per-thread histogram reduced in any order gives exactly the serial counts.
/// Unlike the DEM force loop, this kernel needs no ordered accumulation — and
/// this test is what holds that claim to account.
#[test]
fn cpu_backends_agree_exactly() {
    let Some(centres) = load_positions("htr10_settled.csv") else {
        eprintln!("skipping: htr10_settled.csv not present");
        return;
    };
    let settings = settings_for(&centres);
    let serial = radial_distribution(&centres, settings, ComputeType::CpuSingleThread);
    let parallel = radial_distribution(
        &centres,
        settings,
        ComputeType::CpuMultiThread(ThreadCount::Fixed(7)),
    );
    assert_eq!(
        serial.counts, parallel.counts,
        "CPU single- and multi-thread histograms differ; integer counting must be exact"
    );
    assert_eq!(serial.n_centres, parallel.n_centres);
}

/// The GPU backend is `f32` and the CPU reference is `f64`, so the two are
/// **not** bit-identical — this measures how far apart they actually are.
///
/// **Methodology.** Same bed, same settings, both backends; compare per-bin
/// counts. WGSL has no `f64`, so a pair whose separation falls within `f32`
/// rounding of a bin edge can land in a neighbouring bin. `f32` resolves a 2 m
/// coordinate to ~0.2 µm against a bin width of `d/50 = 1.2 mm`, so the
/// expected rate is of order 1e-4 of binned pairs, each moving one bin.
///
/// **Pass criterion.** Total binned pairs agree to within 1e-5 of each other,
/// and no bin's count differs by more than 0.5 % of itself.
///
/// ~~Total binned pairs agree exactly (no pair is lost or invented — only
/// misbinned)~~ **CORRECTED** — that was wrong, and the first run caught it:
/// CPU 5 331 843 against GPU 5 331 845. `r_max` is itself an `f32` comparison
/// on the GPU, so a pair sitting within `f32` rounding of the *cutoff* is
/// included by one backend and excluded by the other. That is the same
/// rounding effect as an internal bin edge, at the outer boundary, and it is
/// not a kernel defect. Two pairs in 5.3 million is 4e-7.
///
/// Skips silently when no GPU adapter is present, which is the normal outcome
/// on a headless host.
///
/// **Results.** Recorded in the V&V document.
#[test]
#[cfg(all(not(target_os = "android"), not(target_arch = "wasm32")))]
fn gpu_backend_matches_cpu_within_f32_binning() {
    let Some(centres) = load_positions("htr10_settled.csv") else {
        eprintln!("skipping: htr10_settled.csv not present");
        return;
    };
    if outram_park_fork_liggghts::gpu::probe().is_none() {
        eprintln!("skipping: no GPU adapter available");
        return;
    }
    let settings = settings_for(&centres);
    let cpu = radial_distribution(&centres, settings, ComputeType::CpuSingleThread);
    let gpu = radial_distribution(&centres, settings, ComputeType::Gpu);

    let (cpu_total, gpu_total): (u64, u64) = (cpu.counts.iter().sum(), gpu.counts.iter().sum());
    let moved: u64 = cpu
        .counts
        .iter()
        .zip(gpu.counts.iter())
        .map(|(a, b)| a.abs_diff(*b))
        .sum();
    eprintln!(
        "GPU vs CPU: {cpu_total} vs {gpu_total} binned pairs; {moved} bin-assignment differences \
         ({:.3e} of pairs)",
        moved as f64 / cpu_total.max(1) as f64
    );
    let total_drift = cpu_total.abs_diff(gpu_total) as f64 / cpu_total.max(1) as f64;
    assert!(
        total_drift < 1.0e-5,
        "GPU and CPU binned-pair totals differ by {total_drift:.2e} ({cpu_total} vs {gpu_total}) \
         — far beyond f32 rounding at the r_max cutoff; that is a kernel defect"
    );
    for (k, (a, b)) in cpu.counts.iter().zip(gpu.counts.iter()).enumerate() {
        let tol = (*a as f64 * 0.005).max(4.0);
        assert!(
            (*a as f64 - *b as f64).abs() <= tol,
            "bin {k} (r = {:.4} m): CPU {a} vs GPU {b} — beyond f32 bin-edge rounding",
            cpu.r[k]
        );
    }
}
