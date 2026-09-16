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
//! 3. **"ppm" has no stated basis.** Taken as **by weight**, and as *natural*
//!    boron — so only the 18.43 wt% that is B-10 absorbs. See the sensitivity
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

fn nuclides() -> Option<Vec<Nuclide>> {
    let load = |name: &str, file: &str| -> Option<Nuclide> {
        let path = reference_endf(file)?;
        eprint!("  reconstructing {name:<6} from {file} … ");
        let t0 = Instant::now();
        let n = Nuclide::from_endf_file(&path, name, TEMP_K, 1.0e-3).ok()?;
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

    Some(vec![
        load("U235", "n-092_U_235-ENDF8.0.endf")?,
        load("U238", "n-092_U_238.endf")?,
        load("O16", "n-008_O_016-ENDF8.0.endf")?,
        load("C12", "n-006_C_012-ENDF8.0.endf")?,
        load("C12", "n-006_C_012-ENDF8.0.endf")?.with_thermal_scattering(sab),
        load("Si28", "n-014_Si_028-ENDF8.0.endf")?,
        load("B10", "n-005_B_010-ENDF8.0.endf")?,
    ])
}

/// Where each nuclide sits in the slice handed to [`DhUniverse::keff`].
const NUCLIDES: Htr10Nuclides = Htr10Nuclides {
    u235: 0,
    u238: 1,
    o16: 2,
    c_free: 3,     // SiC only
    c_graphite: 4, // buffer / PyC / matrix / shell, with S(alpha,beta)
    si28: 5,
    b10: 6,
};

fn main() {
    let n_particles: usize = std::env::var("OUTRAM_HTR10_HISTORIES")
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or(2000);

    println!("HTR-10 fuel pebble — k-infinity by delta (Woodcock) tracking");
    println!("============================================================");
    println!("  geometry : Li, Yu & Wei (2014), HTR 2014 Weihai, Table 2");
    println!("             6 cm ball, 0.5 cm fuel-free shell, 8335 TRISO,");
    println!("             250 um kernel + 90/40/35/40 um coatings, pf 5.0247 %");
    println!("  data     : ENDF/B-VIII.0 + crystalline-graphite S(a,b), {TEMP_K} K");
    println!("  boundary : reflective at the pebble surface (bare pebble k-inf)\n");

    eprintln!("Reconstructing cross sections (setup, not transport):");
    let t_data = Instant::now();
    let Some(nucs) = nuclides() else {
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
    for arm in BoronReading::all() {
        let params = PebbleParams::htr10_li2014()
            .with_materials(fuel_pebble_materials(NUCLIDES, arm, TEMP_K));
        let universe = match DhUniverse::pebble(params, DhTreatment::DeltaTracking) {
            Ok(u) => u,
            Err(e) => {
                println!("{:<30} FAILED: {e}", arm.label());
                continue;
            }
        };
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
