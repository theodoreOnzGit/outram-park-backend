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

//! # HTR-10: Monte Carlo MGXS handed to GeN-Foam, on ONE shared geometry
//!
//! The coupling layer's whole purpose is that the stochastic and deterministic
//! ends describe the *same* reactor. This example does that for the HTR-10:
//! `nee_soon::htr10_rmc` builds the core once, `outram-mc` transports it and
//! tallies multigroup constants, and GeN-Foam's diffusion solver then runs on
//! those constants alone. Neither end gets its own geometry or its own
//! composition, so a disagreement cannot be a modelling difference hiding as a
//! transport difference.
//!
//! ```bash
//! cargo run --release -p nee_soon --example htr10_mgxs_genfoam
//! ```
//!
//! Environment knobs, all optional:
//! `OUTRAM_HTR10_HISTORIES` (default 2000), `OUTRAM_HTR10_RINGS` (6),
//! `OUTRAM_HTR10_LAYERS` (8), `OUTRAM_HTR10_GROUPS` (2).
//!
//! ## What this does and does not show
//!
//! It shows the **coupling path** carrying a real reactor: a 40 000-pebble
//! core with an explicit reflector, condensed to a handful of group constants
//! that a diffusion solver accepts and solves.
//!
//! It is **not** a validation of the HTR-10 eigenvalue, and the deterministic
//! `k` here is deliberately not compared against the RMC paper. Two reasons,
//! both structural: the diffusion solve below is a zero-leakage infinite-medium
//! collapse of a core whose real eigenvalue is leakage-dominated, and
//! `htr10_rmc`'s own documentation records that the paper's `k_eff` curve
//! cannot be reproduced yet because the reflector modelling it refers to lives
//! in IAEA-TECDOC-1382. Quoting a benchmark comparison from this would be
//! exactly the kind of fitted-looking number the workspace rules forbid.

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
    // Appended 2026-09-23: slots 0..=6 keep their indices so no
    // existing material silently repoints at a different nuclide.
    c_sic: 7,
    si29: 8,
    si30: 9,
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
    // SiC's carbon and silicon are bound in a crystal, not a free gas.
    // ENDF/B-VIII.0 ships tsl-CinSiC (MAT 44) and tsl-SiinSiC (MAT 43) so
    // the coating need not be approximated; a missing law falls back to
    // free gas rather than dropping the nuclide.
    let sic_law = |mat_no: i32, file: &str, name: &'static str| -> Option<ThermalScattering> {
        ThermalScattering::from_endf_file(dir.join(file).to_str()?, mat_no, TEMP_K, name).ok()
    };
    let c_in_sic = sic_law(44, "tsl-CinSiC.endf", "c_SiC");
    let si_in_sic = sic_law(43, "tsl-SiinSiC.endf", "Si_SiC");
    let bind_sic = |n: Nuclide, s: &Option<ThermalScattering>| match s {
        Some(t) => n.with_thermal_scattering(t.clone()),
        None => n,
    };
    Some(vec![
        load("U235", "n-092_U_235-ENDF8.0.endf")?,
        load("U238", "n-092_U_238.endf")?,
        load("O16", "n-008_O_016-ENDF8.0.endf")?,
        load("C12", "n-006_C_012-ENDF8.0.endf")?,
        load("C12", "n-006_C_012-ENDF8.0.endf")?.with_thermal_scattering(sab),
        bind_sic(load("Si28", "n-014_Si_028-ENDF8.0.endf")?, &si_in_sic),
        load("B10", "n-005_B_010-ENDF8.0.endf")?,
        // 7, 8, 9: carbon bound in SiC, and silicon's other two natural
        // isotopes. The atom density was always built from silicon's natural
        // molar mass, so this splits a correct total rather than changing it.
        bind_sic(load("C12", "n-006_C_012-ENDF8.0.endf")?, &c_in_sic),
        bind_sic(load("Si29", "n-014_Si_029-ENDF8.0.endf")?, &si_in_sic),
        bind_sic(load("Si30", "n-014_Si_030-ENDF8.0.endf")?, &si_in_sic),
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
    let n_groups = env_usize("OUTRAM_HTR10_GROUPS", 2);

    println!("HTR-10: Monte Carlo MGXS -> GeN-Foam, one shared geometry");
    println!("=========================================================");

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
    let edges: Vec<f64> = if n_groups == 2 {
        vec![1.0e-5, 2.38, 2.0e7]
    } else {
        let (l0, l1) = (1.0e-5_f64.ln(), 2.0e7_f64.ln());
        (0..=n_groups)
            .map(|i| (l0 + (l1 - l0) * i as f64 / n_groups as f64).exp())
            .collect()
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

    let desc = lib.in_descending_energy();
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

    // Zero-leakage collapse: every cell in zone 0. This exercises the solver on
    // the tallied constants; it is NOT the HTR-10 eigenvalue, because the real
    // core is leakage-dominated and this geometry has no leakage at all.
    let mesh = Arc::new(
        create_one_d_mesh(Length::new::<meter>(1.0), Area::new::<square_meter>(1.0), 4)
            .expect("slab mesh"),
    );
    let zone_of_cell = vec![0usize; mesh.n_cells];
    let bc = vec![
        BoundaryCondition::ZeroGradient,
        BoundaryCondition::ZeroGradient,
    ];
    match DiffusionNeutronics::new(
        mesh,
        &xs,
        &zone_of_cell,
        &[],
        &bc,
        DiffusionSettings::default(),
    ) {
        Ok(mut model) => match model.solve_eigenvalue() {
            Ok(r) => {
                println!(
                    "  GeN-Foam k_inf (zone 0, zero leakage) = {:.6} in {} outer iterations",
                    r.k_eff, r.outer_iterations
                );
                println!(
                    "\n  NOTE: this is NOT the HTR-10 eigenvalue and must not be quoted as one."
                );
                println!("  It is a zero-leakage collapse of one zone, run to show the coupling");
                println!("  path carries a real core. The HTR-10 k_eff is leakage-dominated, and");
                println!("  htr10_rmc's own docs record that the RMC paper's curve cannot be");
                println!("  reproduced until the IAEA-TECDOC-1382 reflector detail is modelled.");
            }
            Err(e) => println!("  eigenvalue solve failed: {e}"),
        },
        Err(e) => println!("  could not build the diffusion model: {e}"),
    }
}
