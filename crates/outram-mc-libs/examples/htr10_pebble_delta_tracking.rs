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

//! # HTR-10 fuel pebble, k-infinity by delta (Woodcock) tracking
//!
//! Builds the HTR-10 fuel pebble from Li, Yu & Wei (2014), *"Research on
//! Benchmark Calculation and Analysis of HTR-10 with RMC Code"*, HTR 2014
//! (Weihai, 27-31 October 2014), **Table 2**, and solves its k-infinity with
//! exact delta tracking over the explicit TRISO packing.
//!
//! ```bash
//! cargo run --release --example htr10_pebble_delta_tracking
//! OUTRAM_HTR10_HISTORIES=4000 cargo run --release --example htr10_pebble_delta_tracking
//! ```
//!
//! ## Why delta tracking
//!
//! A fuel pebble is a doubly-heterogeneous medium: 8335 TRISO particles, each
//! five concentric shells, scattered through graphite. Surface tracking would
//! have to find the next of ~42 000 surfaces on every flight.
//! [`DhTreatment::DeltaTracking`] instead samples flights at a majorant cross
//! section and rejects at the sampled point, which is **unbiased** — it
//! reproduces the explicit-geometry answer to within its own statistics rather
//! than approximating it. It is the reference arm the crate's other treatments
//! (chord-length, SCLS, homogenised, ring-RPT) are judged against.
//!
//! ## The geometry, and where it comes from
//!
//! | quantity | value | source |
//! |---|---|---|
//! | pebble diameter | 6 cm | Table 2 |
//! | fuel-free shell | 0.5 cm | Table 2 |
//! | TRISO per pebble | 8335 | Table 2 |
//! | kernel radius | 250 µm | Table 2 |
//! | buffer / IPyC / SiC / OPyC | 90 / 40 / 35 / 40 µm | Table 2 |
//! | packing fraction | 0.050247 | **derived** — see [`TrisoSpec::HTR10_LI2014`] |
//!
//! ## Three things Table 2 does not say, and what is assumed instead
//!
//! Stated here rather than buried, because each is a real assumption and a
//! reader has to be able to overrule it:
//!
//! 1. **"Fuel enrichment 17 %" gives no basis.** Taken as **weight** percent,
//!    the industry convention, giving 17.1801 at% U-235. The mass closure
//!    cannot arbitrate — heavy-metal loading comes out 4.99991 g under a weight
//!    reading and 4.99992 g under an atom reading, a 0.01 mg difference — but
//!    the two differ by **1.06 % in U-235 number density**, which the eigenvalue
//!    does see.
//! 2. **No density is given for the fuel ball's own graphite.** Table 2 gives
//!    1.73 g/cm3 only for the *moderator* ball. The matrix and shell are taken
//!    at that same 1.73 g/cm3.
//! 3. **"ppm" has no stated basis.** ~~Taken as **by weight**, and as *natural*
//!    boron — so only the 18.43 wt% that is B-10 absorbs.~~ **CHANGED
//!    2026-10-01 (gh:#424):** taken as **atom** ppm of natural boron (19.9
//!    at.% B-10), as TECDOC's MIT and BATAN tables do; the weight reading is
//!    the `NaturalWeightPpm` arm. Results recorded below predate the change. See the sensitivity
//!    study below, which exists because this one is easy to get wrong.
//!
//! ## The boron sensitivity, and why it is here
//!
//! Table 2 gives 4 ppm natural boron in the uranium and 1.3 ppm in the
//! graphite. Those two rows are **not** equally important, and a hand estimate
//! at 2200 m/s says the smaller-looking one dominates: boron is ~0.06 % of the
//! kernel's absorption but ~24 % of the graphite's, because graphite is such a
//! weak absorber (sigma_a ~ 0.0035 b) that 1.3 ppm of a 3840 b absorber
//! competes with it.
//!
//! So the example runs four arms and *measures* it rather than asserting it:
//!
//! | arm | what it models |
//! |---|---|
//! | `natural B (as specified)` | Table 2 read correctly |
//! | `no boron at all` | the impurity rows dropped |
//! | `graphite boron only` | tests whether the kernel's 4 ppm matters |
//! | `ppm read as elemental B-10` | the 5.43x over-absorption mistake |
//!
//! ## Results (measured 2026-09-16)
//!
//! ENDF/B-VIII.0 + crystalline-graphite S(alpha,beta) at 300.15 K, 6000
//! histories x [20 inactive + 60 active], delta tracking over the explicit
//! packing:
//!
//! | arm | `k_inf` | sigma | vs natural |
//! |---|---|---|---|
//! | natural B (as specified) | **1.68515** | 0.00178 | (reference) |
//! | no boron at all | 1.70002 | 0.00175 | **+1487 pcm (6.0 sigma)** |
//! | graphite boron only | 1.68297 | 0.00175 | -218 pcm (0.9 sigma) |
//! | ppm read as elemental B-10 | 1.62804 | 0.00191 | **-5711 pcm (21.9 sigma)** |
//!
//! ### What the arms say
//!
//! **The prediction was recorded before the run and it held.** A 2200 m/s hand
//! estimate said the kernel's 4 ppm was ~0.06 % of local absorption and the
//! graphite's 1.3 ppm ~24 %, so dropping the kernel's boron should be
//! *invisible*. It is: the `graphite boron only` arm sits **0.9 sigma** from
//! natural, and its sign is even negative — removing an absorber cannot lower
//! `k`, so that is noise, which is what an unresolved null looks like.
//!
//! Since the kernel contributes nothing measurable, essentially the whole
//! **+1487 pcm** of the `no boron` arm is the graphite's 1.3 ppm. A row that
//! reads like a trace impurity is worth about 1500 pcm, because graphite is weak
//! enough as an absorber (sigma_a ~ 0.0035 b) that 1.3 ppm of a 3840 b absorber
//! competes with it.
//!
//! And **misreading that same row as elemental B-10 costs 5711 pcm** — nearly
//! four times the impurity's entire worth, because it multiplies the absorber by
//! 5.43x. Nothing in Table 2's wording prevents it.
//!
//! ### Agreement between runs
//!
//! An earlier 2000-history run gave `k_inf = 1.68532 +/- 0.00308` for the
//! natural arm — **17 pcm** from the 6000-history value — with +1093 pcm
//! (2.7 sigma), -536 pcm (1.2 sigma) and -5974 pcm (14.3 sigma) on the other
//! three. Every arm agrees within statistics, and the two unresolved nulls stay
//! unresolved as the statistics improve rather than resolving into something.
//!
//! ## After the per-material S(a,b) migration (gh:#1581, #1583)
//!
//! The table above was measured 2026-09-16 at 6000 histories, **before** the
//! migration, and has **not** been re-measured: it stays as recorded. The
//! migration lengthens the nuclide array, which re-draws the URR stream.
//! `OUTRAM_HTR10_SEEDS=N` runs N master seeds per arm in one process (seed
//! loop outer). A 24-seed old-vs-new ensemble at 300 x [20 + 60] gave every
//! arm and every boron delta consistent with zero (|z| <= 1.84): natural-B
//! atom ppm `1.68887` (old, sd 829 pcm) vs `1.68681` (new, sd 589 pcm),
//! -206 +- 208 pcm. That is a null at about +-200 pcm, not a proof of zero.
//! Method, the five-arm table, hardware and raw outputs:
//! `verification_and_validation/sab_per_material_1581/README.md`.
//!
//! ## This is verification input, not a validated result
//!
//! The eigenvalue printed is a **bare pebble k-infinity**, reflective at the
//! pebble surface. It is *not* comparable to the paper's Table 3/4 numbers,
//! which are whole-core k-eff against loading height. No published reference
//! k-infinity for this pebble is asserted here, because none was read; this
//! example establishes the pebble model and quantifies its input ambiguities.

use std::path::PathBuf;
use std::time::Instant;

use outram_mc_libs::dh_universe::{DhTreatment, DhUniverse, PebbleParams};
use outram_mc_libs::material::material_sab::{assign_htr10_sab, Htr10SabLaws};
use outram_mc_libs::material::nuclide::Nuclide;
use outram_mc_libs::material::thermal::ThermalScattering;
use outram_mc_libs::pebble_beds::htr10::{fuel_pebble_materials, BoronReading, Htr10Nuclides};
use outram_mc_libs::physics::keff::KeffSettings;

/// All materials at 27 C, per the paper: *"All materials are specified to be
/// 27 C."* 300.15 K.
const TEMP_K: f64 = 300.15;

fn reference_endf(file: &str) -> Option<PathBuf> {
    let p = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../reference-data/endf")
        .join(file);
    p.exists().then_some(p)
}

fn nuclides() -> Option<(Vec<Nuclide>, Htr10SabLaws)> {
    let load = |name: &str, file: &str| -> Option<Nuclide> {
        let path = reference_endf(file)?;
        eprint!("  reconstructing {name:<6} from {file} … ");
        let t0 = Instant::now();
        let n = Nuclide::from_endf_file(&path, name, uom::si::f64::ThermodynamicTemperature::new::<uom::si::thermodynamic_temperature::kelvin>(TEMP_K), 1.0e-3).ok()?;
        eprintln!("{:.1?}", t0.elapsed());
        Some(n)
    };

    // Graphite S(alpha,beta) — mandatory for a graphite-moderated pebble. Free-gas
    // carbon would badly misrepresent the thermal spectrum this design lives in.
    let sab = ThermalScattering::from_endf_file(
        reference_endf("tsl-crystalline-graphite.endf")?.to_str()?,
        30, // MAT 30 — C in crystalline graphite (ENDF/B-VIII.0)
        TEMP_K,
        "c_Graphite",
    )
    .ok()?;
    eprintln!("  graphite S(a,b): ENDF/B-VIII.0 crystalline graphite @ {TEMP_K} K … ok");

    // SiC's carbon and silicon are bound in a crystal, not a free gas.
    // ENDF/B-VIII.0 ships tsl-CinSiC (MAT 44) and tsl-SiinSiC (MAT 43); a
    // missing law falls back to free gas rather than dropping the nuclide,
    // so this diagnostic describes the same material set the transport path
    // is run with rather than an approximation of it.
    let sic_law = |mat_no: i32, file: &str, name: &'static str| -> Option<ThermalScattering> {
        ThermalScattering::from_endf_file(reference_endf(file)?.to_str()?, mat_no, TEMP_K, name)
            .ok()
    };
    let c_in_sic = sic_law(44, "tsl-CinSiC.endf", "c_SiC");
    let si_in_sic = sic_law(43, "tsl-SiinSiC.endf", "Si_SiC");
    // Every slot is loaded free; the three laws are attached per material in
    // `main` (`assign_htr10_sab`, gh:#462 / #1581).
    let nuclides = vec![
        load("U235", "n-092_U_235-ENDF8.0.endf")?,
        load("U238", "n-092_U_238.endf")?,
        load("O16", "n-008_O_016-ENDF8.0.endf")?,
        load("C12", "n-006_C_012-ENDF8.0.endf")?,
        load("C12", "n-006_C_012-ENDF8.0.endf")?,
        load("Si28", "n-014_Si_028-ENDF8.0.endf")?,
        load("B10", "n-005_B_010-ENDF8.0.endf")?,
        // 7, 8, 9: the slots `Htr10Nuclides` names. Filling the struct
        // fields in is not enough -- these indices are used to index this
        // very slice, so the entries have to exist or the example reads
        // past its own end at run time.
        load("C12", "n-006_C_012-ENDF8.0.endf")?,
        load("Si29", "n-014_Si_029-ENDF8.0.endf")?,
        load("Si30", "n-014_Si_030-ENDF8.0.endf")?,
        // 10: B-11, the rest of natural boron (gh:#311).
        load("B11", "n-005_B_011-ENDF8.0.endf")?,
        // 11, 12, 13: C-13, the 1.07 at.% of natural carbon (gh:#425,
        // 2026-10-01): free, graphite-bound and SiC-bound, as their C-12
        // partners at 3, 4 and 7 (`Htr10Nuclides::NATURAL_CARBON`).
        load("C13", "n-006_C_013-ENDF8.0.endf")?,
        load("C13", "n-006_C_013-ENDF8.0.endf")?,
        load("C13", "n-006_C_013-ENDF8.0.endf")?,
    ];
    Some((nuclides, Htr10SabLaws { graphite: sab, c_in_sic, si_in_sic }))
}

/// Where each nuclide sits in the slice handed to [`DhUniverse::keff`].
const NUCLIDES: Htr10Nuclides = Htr10Nuclides::NATURAL_CARBON;

fn main() {
    let n_particles: usize = std::env::var("OUTRAM_HTR10_HISTORIES")
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or(2000);

    let n_seeds: usize = std::env::var("OUTRAM_HTR10_SEEDS")
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or(1);
    let seed0: u64 = std::env::var("OUTRAM_HTR10_SEED0")
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or(1);

    println!("HTR-10 fuel pebble — k-infinity by delta (Woodcock) tracking");
    println!("============================================================");
    println!("  geometry : Li, Yu & Wei (2014), HTR 2014 Weihai, Table 2");
    println!("             6 cm ball, 0.5 cm fuel-free shell, 8335 TRISO,");
    println!("             250 um kernel + 90/40/35/40 um coatings, pf 5.0247 %");
    println!("  data     : ENDF/B-VIII.0 + crystalline-graphite S(a,b), {TEMP_K} K");
    println!("  boundary : reflective at the pebble surface (bare pebble k-inf)\n");

    eprintln!("Reconstructing cross sections (setup, not transport):");
    let t_data = Instant::now();
    let Some((free_nucs, laws)) = nuclides() else {
        println!("SKIP: reference-data/endf/ tapes not available in this checkout.");
        return;
    };
    eprintln!("  data ready in {:.1} s\n", t_data.elapsed().as_secs_f64());

    let settings = KeffSettings {
        n_particles,
        n_inactive: 20,
        n_active: 60,
        temperature_k: TEMP_K,
        ..KeffSettings::default()
    };
    println!(
        "  histories: {} x [{} inactive + {} active]\n",
        settings.n_particles, settings.n_inactive, settings.n_active
    );

    println!(
        "{:<30} {:>10} {:>10} {:>12} {:>9}",
        "boron interpretation", "k_inf", "sigma", "vs natural", "secs"
    );
    println!("{}", "-".repeat(75));

    let mut baseline: Option<(f64, f64)> = None;
    // Build every arm's universe once (gh:#1583: with several seeds the seed
    // loop is OUTER so a run cut short still holds equal N in every arm).
    let mut built = Vec::new();
    for arm in BoronReading::all() {
        let arm_materials = fuel_pebble_materials(NUCLIDES, arm, TEMP_K);
        // Per-material S(a,b) (gh:#1581): each arm's own materials decide
        // which slots are bound.
        let (arm_materials, nucs) =
            assign_htr10_sab(&arm_materials, &free_nucs, &NUCLIDES, &laws)
                .expect("S(a,b) per material");
        let params = PebbleParams::htr10_li2014().with_materials(arm_materials);
        let universe = match DhUniverse::pebble(params, DhTreatment::DeltaTracking) {
            Ok(u) => u,
            Err(e) => {
                println!("{:<30} FAILED: {e}", arm.label());
                continue;
            }
        };
        // `OUTRAM_MAJORANT_AUDIT=1`: audit the majorant `keff` would build,
        // old and new construction, and skip the transport (GitHub #585).
        if std::env::var_os("OUTRAM_MAJORANT_AUDIT").is_some() {
            let tag = format!("htr10_pebble_delta_tracking [{}]", arm.label());
            println!("{}", universe.majorant_audit_line(&nucs, &tag));
            continue;
        }
        built.push((arm, universe, nucs));
    }
    if std::env::var_os("OUTRAM_MAJORANT_AUDIT").is_some() {
        return;
    }
    // `OUTRAM_HTR10_SEEDS=N` (gh:#1583): N master seeds per arm inside this
    // one process, so the slow ENDF load is paid once. N = 1 (the default) is
    // the single-seed table, unchanged.
    if n_seeds > 1 {
        let mut ks: Vec<Vec<f64>> = vec![Vec::new(); built.len()];
        for i in 0..n_seeds {
            let seed = seed0 + i as u64;
            let s = KeffSettings { seed, ..settings.clone() };
            for (ai, (arm, universe, nucs)) in built.iter().enumerate() {
                let t0 = Instant::now();
                let r = universe.keff(nucs, &s);
                println!(
                    "SEED {:<30} {:>4} {:>10.5} {:>10.5} {:>8.1}s",
                    arm.label(),
                    seed,
                    r.k_mean,
                    r.k_std,
                    t0.elapsed().as_secs_f64()
                );
                ks[ai].push(r.k_mean);
            }
        }
        for (ai, (arm, _, _)) in built.iter().enumerate() {
            let n = ks[ai].len() as f64;
            let mean = ks[ai].iter().sum::<f64>() / n;
            let var = ks[ai].iter().map(|k| (k - mean).powi(2)).sum::<f64>() / (n - 1.0);
            println!(
                "ENSEMBLE {:<30} n={} mean {:.5} sd {:.5} sem {:.5}",
                arm.label(),
                ks[ai].len(),
                mean,
                var.sqrt(),
                (var / n).sqrt()
            );
        }
        return;
    }
    for (arm, universe, nucs) in &built {
        let nucs = nucs.as_slice();
        let t0 = Instant::now();
        let r = universe.keff(&nucs, &settings);
        let secs = t0.elapsed().as_secs_f64();

        let delta = match baseline {
            None => {
                baseline = Some((r.k_mean, r.k_std));
                "  (reference)".to_string()
            }
            Some((k0, s0)) => {
                let dk = (r.k_mean - k0) * 1.0e5;
                let sig = ((r.k_std * r.k_std + s0 * s0).sqrt() * 1.0e5).max(1.0);
                format!("{dk:+8.0} pcm ({:.1}s)", dk.abs() / sig)
            }
        };
        println!(
            "{:<30} {:>10.5} {:>10.5} {:>12} {:>9.1}",
            arm.label(),
            r.k_mean,
            r.k_std,
            delta,
            secs
        );
    }

    println!(
        "\n  'vs natural' is (k - k_natural) in pcm, with the sigma-distance of the\n  \
         difference in brackets. A difference under ~2 sigma is not resolved by this\n  \
         run — raise OUTRAM_HTR10_HISTORIES before reading anything into it."
    );
}
