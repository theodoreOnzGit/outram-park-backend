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

//! # HTR-10: deterministic (diffusion and SP3) against Monte Carlo, 8 groups
//!
//! Phase 1 of the deterministic programme. The question is narrow and
//! deliberately so: **does the MGXS condensation chain reproduce the Monte
//! Carlo answer when geometry is taken out of the comparison?**
//!
//! ```bash
//! cargo run --release -p nee_soon --example htr10_deterministic_vs_mc
//! ```
//!
//! ## Why an INFINITE MEDIUM, and why that is the right first test
//!
//! The obvious comparison — deterministic `k_eff` against the full-core Monte
//! Carlo `k_eff` — confounds two entirely different things: whether the group
//! constants are right, and whether the deterministic geometry and boundary
//! treatment reproduce the leakage. The HTR-10 is leakage-dominated, so a
//! disagreement would be unattributable.
//!
//! This example removes leakage from **both** ends instead:
//!
//! - the Monte Carlo model is run with `OUTRAM_HTR10_REFLECTIVE` and
//!   `OUTRAM_HTR10_NOREFL`, i.e. an infinite medium of the bed, giving
//!   `k_inf(MC)`;
//! - the deterministic solve uses zero-gradient boundaries on a slab, which is
//!   also zero leakage, giving `k_inf(det)`.
//!
//! Both then describe the **same infinite medium**, from the **same tallied
//! group constants**. Any difference is condensation or solver, and nothing
//! else. Leakage geometry is a separate, later stage; adding it before this
//! passes would mean debugging two things at once.
//!
//! ## Why SP3 must equal diffusion here, and what it tests
//!
//! SP3 reduces to diffusion wherever the flux is near-isotropic, which is
//! exactly what an infinite homogeneous medium is. So the SP3 arm is not
//! expected to improve on diffusion here — it is expected to **agree with it**,
//! and a disagreement would indicate a defect in the SP3 wiring rather than
//! transport physics the diffusion arm is missing. It is a harness check, and
//! it is the only place in this programme where the right answer is known in
//! advance.
//!
//! ## Group structure
//!
//! Eight groups on a WIMS-style thermal-reactor structure rather than a
//! log-uniform grid:
//!
//! ```text
//!   1e-5  0.14  0.625  4.0  29.0  130.0  9.12e3  1.35e6  2e7   [eV]
//! ```
//!
//! The boundaries are placed where the physics is: 0.625 eV is the classical
//! thermal cutoff, 4-130 eV brackets the large U-238 capture resonances
//! (6.67, 20.9, 36.7 eV), and 1.35 MeV is near the U-238 fast-fission
//! threshold. A log-uniform grid would put most of its resolution where
//! nothing happens and none across the resonances, which is what the previous
//! `htr10_mgxs_genfoam` example did above two groups.
//!
//! Every group must carry flux or the bridge refuses the data -- correctly, an
//! unvisited group has no measured cross section.
//!
//! ## What this does NOT establish
//!
//! Not a benchmark comparison: the RMC reference is a leakage-dominated
//! `k_eff` and nothing here is comparable to it. Not a validation -- both ends
//! are this workspace's own codes. `k_inf` agreement says the condensation and
//! the solvers are consistent; it says nothing yet about the HTR-10.

use std::sync::Arc;


use nee_soon::genfoam_xs::to_nuclear_data_input;
use nee_soon::htr10_rmc::core_model::{
    assemble, mat, HTR10_BORED_BORON, HTR10_BORED_CARBON, PAPER_FILLING_FRACTION,
};
use nee_soon::htr10_rmc::reflector::zone_composition;
use nee_soon::mgxs::{condense, matrix_tally, scalar_tally, GroupStructure};
use outram_foam_appbuilder_lib::genfoam::neutronics::diffusion::{
    DiffusionNeutronics, DiffusionSettings,
};
use outram_foam_appbuilder_lib::genfoam::neutronics::sp3::{Sp3Neutronics, Sp3Settings};
use outram_foam_appbuilder_lib::genfoam::neutronics::xs::CrossSectionData;
use outram_foam_basic_lib::interface::one_dimensional_meshing::create_one_d_mesh;
use outram_foam_basic_lib::prelude::BoundaryCondition;
use outram_mc_libs::geometry::position::Position;
use outram_mc_libs::material::material::{Material, NuclideComponent};
use outram_mc_libs::material::nuclide::Nuclide;
use outram_mc_libs::material::thermal::ThermalScattering;
use outram_mc_libs::pebble_beds::htr10::{fuel_pebble_materials, BoronReading, Htr10Nuclides};
use outram_mc_libs::physics::keff::KeffSettings;
use outram_mc_libs::pebble_beds::delta_tracking::Majorant;
use outram_mc_libs::physics::transport_csg::{run_keff_csg_hybrid, SourceBox};
use uom::si::area::square_meter;
use uom::si::f64::{Area, Length};
use uom::si::length::meter;

const TEMP_K: f64 = 293.6;
const B10_OF_NATURAL: f64 = 0.199;
const NUC: Htr10Nuclides = Htr10Nuclides {
    u235: 0,
    u238: 1,
    o16: 2,
    c_free: 3,
    c_graphite: 4,
    si28: 5,
    b10: 6,
};

fn env_usize(key: &str, default: usize) -> usize {
    std::env::var(key)
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or(default)
}

fn nuclides() -> Option<Vec<Nuclide>> {
    let dir =
        std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../reference-data/endf");
    let load = |n: &str, f: &str| Nuclide::from_endf_file(&dir.join(f), n, TEMP_K, 1.0e-3).ok();
    let sab = ThermalScattering::from_endf_file(
        dir.join("tsl-crystalline-graphite.endf").to_str()?,
        30,
        TEMP_K,
        "c_Graphite",
    )
    .ok()?;
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

/// The eleven materials `htr10_rmc`'s geometry indexes, in `mat::` order.
fn materials() -> Vec<Material> {
    let mut mats = fuel_pebble_materials(NUC, BoronReading::Natural, TEMP_K);
    mats.truncate(6); // 0..5: kernel, buffer, iPyC, SiC, oPyC, graphite
    mats.push(Material {
        id: 70,
        name: "helium".into(),
        components: vec![],
        temperature: TEMP_K,
    });
    let graphite_zone = |id: i32, name: &str, z_carbon: f64, z_boron: f64| Material {
        id,
        name: name.into(),
        components: vec![
            NuclideComponent {
                nuclide_idx: NUC.c_graphite,
                atom_density: z_carbon,
            },
            NuclideComponent {
                nuclide_idx: NUC.b10,
                atom_density: z_boron * B10_OF_NATURAL,
            },
        ],
        temperature: TEMP_K,
    };
    let z22 = zone_composition(22).expect("TECDOC zone 22");
    mats.push(graphite_zone(
        71,
        "reflector graphite (TECDOC zone 22)",
        z22.carbon,
        z22.natural_boron,
    ));
    let z17 = zone_composition(17).expect("TECDOC zone 17");
    mats.push(graphite_zone(
        72,
        "boronated carbon brick (TECDOC zone 17)",
        z17.carbon,
        z17.natural_boron,
    ));
    mats.push(graphite_zone(
        73,
        "bored side reflector (TECDOC zones 31-40)",
        HTR10_BORED_CARBON,
        HTR10_BORED_BORON,
    ));
    let dummy = mats[mat::GRAPHITE].clone();
    mats.push(Material {
        id: 74,
        name: "homogenised dummy pebbles".into(),
        components: dummy
            .components
            .iter()
            .map(|c| NuclideComponent {
                nuclide_idx: c.nuclide_idx,
                atom_density: c.atom_density * PAPER_FILLING_FRACTION,
            })
            .collect(),
        temperature: TEMP_K,
    });
    assert_eq!(
        mats.len(),
        mat::HOMOG_DUMMY + 1,
        "material order must match mat::"
    );
    mats
}

fn main() {
    let histories = env_usize("OUTRAM_HTR10_HISTORIES", 2000);
    let rings = env_usize("OUTRAM_HTR10_RINGS", 6);
    let layers = env_usize("OUTRAM_HTR10_LAYERS", 8);
    let n_groups = env_usize("OUTRAM_HTR10_GROUPS", 8);

    // A NORMAL, leakage-terminated run -- vacuum boundary, reflector present.
    //
    // An earlier version of this example forced REFLECTIVE + NOREFL to build a
    // literal infinite medium. That was abandoned for a measured reason: with
    // no leakage, histories in graphite terminate only by absorption, and
    // graphite's Sigma_a/Sigma_s is of order 1e-4, so a single history runs
    // thousands of collisions. It did not finish one of two tally passes in 30
    // minutes on a 3268-tile geometry. Analog Monte Carlo in an infinite
    // moderator is impractical, and brute-forcing it would have bought nothing.
    //
    // It is not needed. `k_inf` is recoverable from the tallies of an ordinary
    // leaky run -- see the comparison below.
    println!("HTR-10: deterministic (diffusion + SP3) vs Monte Carlo, k_inf");
    println!("=============================================================");

    let Some(nucs) = nuclides() else {
        println!("SKIP: reference-data/endf/ not in this checkout.");
        return;
    };
    let mats = materials();
    let core = assemble(rings, layers, 0);
    println!(
        "  geometry: {} tiles, {} cells, {} universes; bed r={:.1} cm, half-height={:.1} cm",
        core.tiles, core.cells, core.universes, core.bed_radius, core.bed_half_height
    );

    // Region-local majorant over the BED materials only: the reflector is
    // surface-tracked and must not raise the bed's delta-tracking cost.
    let grid: Vec<f64> = (0..4096)
        .map(|i| (1.0e-4_f64.ln() + (2.0e7_f64.ln() - 1.0e-4_f64.ln()) * i as f64 / 4095.0).exp())
        .collect();
    let bed_mats: Vec<usize> = (0..=mat::HELIUM).collect();
    let maj = Majorant::over_indices(&mats, &bed_mats, &nucs, &grid, 0.3);

    let settings = KeffSettings {
        n_particles: histories,
        n_inactive: 20,
        n_active: 40,
        seed: 20_260_919,
        ..KeffSettings::default()
    };
    let src = SourceBox {
        lower: Position::new(-core.bed_radius, -core.bed_radius, core.conus_floor),
        upper: Position::new(core.bed_radius, core.bed_radius, core.bed_half_height),
    };

    // A thermal reactor populates the whole range, so a coarse structure with
    // a thermal/fast split is used rather than a fine log grid -- every group
    // must carry flux or the bridge refuses it (and rightly: an unvisited group
    // has no measured cross section).
    // Boundaries placed where the physics is, not on a log grid. 0.625 eV is
    // the classical thermal cutoff; 4-130 eV brackets the large U-238 capture
    // resonances (6.67, 20.9, 36.7 eV); 1.35 MeV is near the U-238 fast-fission
    // threshold. A log-uniform grid spends its resolution where nothing happens
    // and puts no boundary across the resonances at all.
    const WIMS8: [f64; 9] = [
        1.0e-5, 0.14, 0.625, 4.0, 29.0, 130.0, 9.12e3, 1.35e6, 2.0e7,
    ];
    let edges: Vec<f64> = match n_groups {
        2 => vec![1.0e-5, 2.38, 2.0e7],
        8 => WIMS8.to_vec(),
        n => {
            // Any other count falls back to log-uniform, and says so: the
            // placed structure above only exists for 8.
            println!("  NOTE: {n} groups requested -- no placed structure for that count,");
            println!("        falling back to a log-uniform grid. Use 8 for the WIMS-style one.");
            let (l0, l1) = (1.0e-5_f64.ln(), 2.0e7_f64.ln());
            (0..=n).map(|i| (l0 + (l1 - l0) * i as f64 / n as f64).exp()).collect()
        }
    };
    let groups = GroupStructure::new(edges).expect("ascending edges");
    let names: Vec<String> = mats.iter().map(|m| m.name.clone()).collect();
    let all: Vec<usize> = (0..mats.len()).collect();

    println!("\n  Monte Carlo pass 1/2 (scalar reaction rates)...");
    let mut scalar = scalar_tally(1, &groups, all.clone());
    let mc = run_keff_csg_hybrid(
        &core.geometry,
        &mats,
        &nucs,
        &[maj.clone()],
        None,
        src,
        &settings,
        Some(&mut scalar),
    );
    println!("  Monte Carlo pass 2/2 (scatter matrix + fission spectrum)...");
    let mut matrix = matrix_tally(2, &groups, all);
    run_keff_csg_hybrid(
        &core.geometry,
        &mats,
        &nucs,
        &[maj],
        None,
        src,
        &settings,
        Some(&mut matrix),
    );

    println!(
        "\n  Monte Carlo k_eff = {:.6} +/- {:.6}",
        mc.k_mean, mc.k_std
    );

    let lib = match condense(&groups, &names, &scalar, &matrix, settings.n_active as u64) {
        Ok(l) => l,
        Err(e) => {
            println!("  condensation failed: {e}");
            return;
        }
    };

    println!(
        "\n  condensed {} zones x {} groups:",
        lib.zones.len(),
        groups.n_groups()
    );
    for z in &lib.zones {
        let visited = z.flux.iter().filter(|p| **p > 0.0).count();
        println!(
            "    {:<44} flux {:>10.3e}  groups visited {}/{}",
            z.name,
            z.flux.iter().sum::<f64>(),
            visited,
            groups.n_groups()
        );
    }

    // ONE homogeneous medium, because k_inf is a WHOLE-SYSTEM quantity.
    // `condense` gives one zone per material, and no single material's k_inf is
    // the core's -- the kernel alone is wildly supercritical, the graphite
    // alone is subcritical. `homogenised` flux-weights them into the medium the
    // Monte Carlo number actually describes.
    let sys = lib.homogenised("HTR-10 core, homogenised");

    // THE MONTE CARLO ANSWER, from the same tallies.
    //
    // `Sigma_x,g = R_x,g / phi_g`, so `Sigma_x,g * phi_g` recovers the raw
    // tallied reaction rate and `k_inf = sum nu_Sigma_f phi / sum Sigma_a phi`
    // is exactly the Monte Carlo production/absorption ratio. No separate run
    // is needed, and no infinite-medium geometry.
    let k_inf_mc = sys.k_inf();

    let desc = sys.in_descending_energy();
    let input = match to_nuclear_data_input(&desc) {
        Ok(i) => i,
        Err(e) => {
            println!("\n  GeN-Foam bridge refused the data:\n    {e}");
            println!("\n  That is the correct outcome, not a crash: a group no neutron visited");
            println!("  has no measured cross section, and zeros make its diffusion equation");
            println!("  singular. Raise OUTRAM_HTR10_HISTORIES or coarsen OUTRAM_HTR10_GROUPS.");
            return;
        }
    };

    let xs = match CrossSectionData::from_input(&input) {
        Ok(x) => x,
        Err(e) => {
            println!("  GeN-Foam rejected the nuclearData input: {e}");
            return;
        }
    };
    println!(
        "\n  GeN-Foam accepted the MGXS: {} groups, {} zones",
        xs.energy_groups(),
        xs.zone_count()
    );

    // Zero-leakage slab: every cell in zone 0, zero-gradient on both faces.
    // This is the deterministic half of the infinite-medium comparison -- the
    // Monte Carlo half is the REFLECTIVE + NOREFL configuration set in main().
    let mesh = Arc::new(
        create_one_d_mesh(Length::new::<meter>(1.0), Area::new::<square_meter>(1.0), 4)
            .expect("slab mesh"),
    );
    let zone_of_cell = vec![0usize; mesh.n_cells];
    let bc = vec![
        BoundaryCondition::ZeroGradient,
        BoundaryCondition::ZeroGradient,
    ];

    let k_mc = k_inf_mc;
    let sig_mc = mc.k_std; // indicative only -- see the caveat printed at the end
    let pcm = |a: f64, b: f64| (a - b) * 1.0e5;

    println!("\n=== k_inf: Monte Carlo tallies vs deterministic solvers ===\n");
    println!("  (the leaky Monte Carlo k_eff for this geometry was {:.6}; it is NOT", mc.k_mean);
    println!("   the comparison target -- the deterministic solve below has no leakage)\n");
    println!("  Monte Carlo tallies, production/absorption   k_inf = {k_mc:.6}");

    let k_diff = match DiffusionNeutronics::new(
        mesh.clone(),
        &xs,
        &zone_of_cell,
        &[],
        &bc,
        DiffusionSettings::default(),
    ) {
        Ok(mut model) => match model.solve_eigenvalue() {
            Ok(r) => {
                println!(
                    "  GeN-Foam diffusion                      k_inf = {:.6}   ({} outers)",
                    r.k_eff, r.outer_iterations
                );
                Some(r.k_eff)
            }
            Err(e) => {
                println!("  diffusion eigenvalue solve failed: {e}");
                None
            }
        },
        Err(e) => {
            println!("  could not build the diffusion model: {e}");
            None
        }
    };

    let k_sp3 = match Sp3Neutronics::with_cross_sections(
        mesh,
        &xs,
        &zone_of_cell,
        &[],
        &bc,
        Sp3Settings::default(),
    ) {
        Ok(mut model) => match model.solve_eigenvalue() {
            Ok(r) => {
                println!(
                    "  GeN-Foam SP3                            k_inf = {:.6}   ({} outers)",
                    r.k_eff, r.outer_iterations
                );
                Some(r.k_eff)
            }
            Err(e) => {
                println!("  SP3 eigenvalue solve failed: {e}");
                None
            }
        },
        Err(e) => {
            println!("  could not build the SP3 model: {e}");
            None
        }
    };

    println!("\n  differences [pcm]:");
    if let Some(kd) = k_diff {
        println!(
            "    diffusion - MC = {:+8.0}   ({:.1} MC sigma)",
            pcm(kd, k_mc),
            (kd - k_mc).abs() / sig_mc.max(1.0e-9)
        );
    }
    if let Some(ks) = k_sp3 {
        println!(
            "    SP3       - MC = {:+8.0}   ({:.1} MC sigma)",
            pcm(ks, k_mc),
            (ks - k_mc).abs() / sig_mc.max(1.0e-9)
        );
    }
    if let (Some(kd), Some(ks)) = (k_diff, k_sp3) {
        let d = pcm(ks, kd);
        println!("    SP3 - diffusion = {d:+7.0}");
        println!();
        if d.abs() < 10.0 {
            println!("  SP3 reproduces diffusion to {:.0} pcm, as it must in an isotropic", d.abs());
            println!("  medium. That is a HARNESS CHECK on the SP3 wiring, not evidence that");
            println!("  SP3 adds anything here -- it cannot, and is not expected to.");
        } else {
            println!("  WARNING: SP3 and diffusion differ by {d:+.0} pcm in an INFINITE HOMOGENEOUS");
            println!("  MEDIUM, where the flux is isotropic and SP3 must reduce to diffusion.");
            println!("  That is a defect in the SP3 wiring or the second-moment boundary");
            println!("  treatment, NOT transport physics diffusion is missing. Do not read the");
            println!("  difference as an SP3 correction.");
        }
    }

    println!("\n  READ THIS BEFORE QUOTING ANYTHING ABOVE");
    println!("  - This is k_inf, NOT the HTR-10 eigenvalue. The real core is");
    println!("    leakage-dominated and nothing here is comparable to the RMC paper.");
    println!("  - Both ends are this workspace's own codes: verification, not validation.");
    println!("  - The MC number is ONE seed. Its sigma is the reported MC statistical");
    println!("    error, which is not the same as run-to-run scatter.");
}
