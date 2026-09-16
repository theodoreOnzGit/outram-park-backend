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
use outram_mc_libs::material::material::{Material, NuclideComponent};
use outram_mc_libs::material::nuclide::Nuclide;
use outram_mc_libs::material::thermal::ThermalScattering;
use outram_mc_libs::physics::keff::KeffSettings;

/// All materials at 27 C, per the paper: *"All materials are specified to be
/// 27 C."* 300.15 K.
const TEMP_K: f64 = 300.15;

// Nuclide indices into `nuclides()`.
const U235: usize = 0;
const U238: usize = 1;
const O16: usize = 2;
const C_FREE: usize = 3; // free-gas carbon: SiC
const C_GRAPHITE: usize = 4; // graphite-bound carbon: buffer, PyC, matrix, shell
const SI28: usize = 5;
const B10: usize = 6;

/// Natural boron: 19.9 at% B-10 / 80.1 at% B-11, so 18.43 **weight** percent
/// B-10. B-11 is omitted — its absorption cross section is ~0.005 b against
/// B-10's 3840 b, and at 1.3 ppm its scattering contributes nothing.
const B10_WEIGHT_FRACTION_OF_NATURAL_B: f64 = 0.184_3;

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

/// How the two "ppm" rows of Table 2 are interpreted.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Boron {
    /// Table 2 read as written: ppm by weight of **natural** boron, so only
    /// 18.43 wt% of it is the absorbing B-10.
    Natural,
    /// The impurity rows dropped entirely.
    None,
    /// Graphite's 1.3 ppm kept, the kernel's 4 ppm dropped.
    GraphiteOnly,
    /// The mistake: ppm read as **elemental B-10**, over-absorbing by 1/0.1843
    /// = 5.43x.
    AsElementalB10,
}

impl Boron {
    fn label(self) -> &'static str {
        match self {
            Self::Natural => "natural B (as specified)",
            Self::None => "no boron at all",
            Self::GraphiteOnly => "graphite boron only",
            Self::AsElementalB10 => "ppm read as elemental B-10",
        }
    }
    /// B-10 weight fraction applied to the stated ppm.
    fn b10_fraction(self) -> f64 {
        match self {
            Self::Natural | Self::GraphiteOnly => B10_WEIGHT_FRACTION_OF_NATURAL_B,
            Self::None => 0.0,
            Self::AsElementalB10 => 1.0,
        }
    }
    fn kernel_ppm(self) -> f64 {
        match self {
            Self::None | Self::GraphiteOnly => 0.0,
            _ => 4.0,
        }
    }
    fn graphite_ppm(self) -> f64 {
        match self {
            Self::None => 0.0,
            _ => 1.3,
        }
    }
}

const NA: f64 = 6.022_140_76e23;
const M_U235: f64 = 235.043_930;
const M_U238: f64 = 238.050_788;
const M_O16: f64 = 15.994_914_6;
const M_C: f64 = 12.011;
const M_SI: f64 = 28.0855;
const M_B10: f64 = 10.0129;

/// Atom density \[atoms/b-cm\] from a mass density \[g/cm3\] and molar mass.
fn nd(rho: f64, molar: f64) -> f64 {
    rho * NA / molar * 1.0e-24
}

/// The seven-material table, in the order [`DhUniverse::pebble`] requires:
/// five TRISO shells outward, then the fuel-zone matrix, then the outer shell.
fn materials(boron: Boron) -> Vec<Material> {
    // 17 % enrichment read as WEIGHT percent -> atom fraction.
    let w5 = 0.17;
    let x5 = (w5 / M_U235) / ((w5 / M_U235) + ((1.0 - w5) / M_U238));
    let m_u = x5 * M_U235 + (1.0 - x5) * M_U238;

    let rho_kernel = 10.4;
    let m_uo2 = m_u + 2.0 * M_O16;
    let n_uo2 = nd(rho_kernel, m_uo2);
    // Kernel boron is quoted "of uranium", so it rides on the U mass density,
    // not the UO2 density.
    let rho_u = rho_kernel * m_u / m_uo2;
    let n_b10_kernel = nd(
        rho_u * boron.kernel_ppm() * 1.0e-6 * boron.b10_fraction(),
        M_B10,
    );

    let graphite_b10 = |rho: f64| {
        nd(
            rho * boron.graphite_ppm() * 1.0e-6 * boron.b10_fraction(),
            M_B10,
        )
    };

    let mat = |id: i32, name: &str, comps: &[(usize, f64)]| Material {
        id,
        name: name.into(),
        temperature: TEMP_K,
        components: comps
            .iter()
            .filter(|&&(_, n)| n > 0.0)
            .map(|&(nuclide_idx, atom_density)| NuclideComponent {
                nuclide_idx,
                atom_density,
            })
            .collect(),
    };

    // Table 2 gives no density for the fuel ball's own graphite; the moderator
    // ball's 1.73 g/cm3 is used for both matrix and shell. See the module docs.
    let rho_matrix = 1.73;
    let rho_shell = 1.73;
    let m_sic = M_SI + M_C;
    let n_sic = nd(3.18, m_sic);

    vec![
        mat(
            0,
            "UO2 kernel (17 wt%)",
            &[
                (U235, x5 * n_uo2),
                (U238, (1.0 - x5) * n_uo2),
                (O16, 2.0 * n_uo2),
                (B10, n_b10_kernel),
            ],
        ),
        mat(
            1,
            "buffer PyC",
            &[(C_GRAPHITE, nd(1.1, M_C)), (B10, graphite_b10(1.1))],
        ),
        mat(
            2,
            "IPyC",
            &[(C_GRAPHITE, nd(1.9, M_C)), (B10, graphite_b10(1.9))],
        ),
        mat(3, "SiC", &[(SI28, n_sic), (C_FREE, n_sic)]),
        mat(
            4,
            "OPyC",
            &[(C_GRAPHITE, nd(1.9, M_C)), (B10, graphite_b10(1.9))],
        ),
        mat(
            5,
            "matrix graphite",
            &[
                (C_GRAPHITE, nd(rho_matrix, M_C)),
                (B10, graphite_b10(rho_matrix)),
            ],
        ),
        mat(
            6,
            "shell graphite",
            &[
                (C_GRAPHITE, nd(rho_shell, M_C)),
                (B10, graphite_b10(rho_shell)),
            ],
        ),
    ]
}

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
    for arm in [
        Boron::Natural,
        Boron::None,
        Boron::GraphiteOnly,
        Boron::AsElementalB10,
    ] {
        let params = PebbleParams::htr10_li2014().with_materials(materials(arm));
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
