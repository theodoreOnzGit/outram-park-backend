// SPDX-License-Identifier: GPL-3.0-only
// Copyright (C) 2026 OUTRAM PARK contributors
// Derived from OpenFOAM (www.openfoam.com / openfoam.org)
// Copyright (C) 2013-2026 OpenFOAM Foundation
//
// This file is part of OUTRAM PARK. See the crate root for the full licence.

//! # Worked example — `ReactingTwoPhaseEulerFoam`
//!
//! A self-contained, top-to-bottom demonstration of the reacting two-phase
//! Euler-Euler solver (the applib port of OpenFOAM's `multiphaseEuler` /
//! historic `reactingTwoPhaseEulerFoam`). Read it straight through — every type
//! it uses comes from this crate's `prelude`, `outram_foam_multiphase`, or
//! `outram_foam_basic_lib`; you should not need to open any internal module.
//!
//! ## The physical case
//!
//! A small 1-D column holds a **hot combustible gas** (continuous phase) laced
//! through an **inert particulate bed** (dispersed phase). The gas carries a
//! two-species composition `fuel → product`. Because the gas starts hot enough
//! to ignite, a single global Arrhenius reaction burns the fuel, releasing heat
//! that (a) raises the gas temperature and (b) is handed to the cold particles
//! through the Ranz-Marshall interfacial heat-transfer coefficient. Over a few
//! milliseconds you watch the fuel burn down, the product build up, and the two
//! phases march toward a common temperature.
//!
//! Run it with:
//!
//! ```text
//! cargo run -p outram-foam-appbuilder-lib --release \
//!     --example reacting_two_phase_euler_combustion
//! ```
//!
//! > **Verification demo, not validation.** The numbers illustrate that the
//! > coupled solve runs and behaves qualitatively correctly; they are **not**
//! > validated against any benchmark or experiment (the human V&V step).

use outram_foam_appbuilder_lib::prelude::{
    InterfacialHeatTransfer, PhaseSelector, PhaseSpecies, PhaseThermo, ReactingTwoPhaseEulerFoam,
    ReactionMechanism,
};
use outram_foam_basic_lib::prelude::{
    BoundaryPatch, FvMesh, FvMeshBuilder, PatchKind, Vector3,
};
use outram_foam_multiphase::two_fluid::{DragModel, Phase, TwoFluidSystem};
use outram_foam_multiphase::two_fluid_pimple::TwoFluidPimple;
use std::sync::Arc;
use uom::si::dynamic_viscosity::pascal_second;
use uom::si::f64::{
    DynamicViscosity, Length, MassDensity, SpecificHeatCapacity, ThermalConductivity,
    ThermodynamicTemperature,
};
use uom::si::length::meter;
use uom::si::mass_density::kilogram_per_cubic_meter;
use uom::si::specific_heat_capacity::joule_per_kilogram_kelvin;
use uom::si::thermal_conductivity::watt_per_meter_kelvin;
use uom::si::thermodynamic_temperature::kelvin;

/// Build a uniform 1-D column of `n` unit cells with inlet/outlet end patches.
fn column_mesh(n: usize) -> Arc<FvMesh> {
    let vx = |x: f64| Vector3::new(x, 0.0, 0.0);
    let owner: Vec<usize> = (0..n - 1).chain([0, n - 1]).collect();
    let neighbour: Vec<usize> = (1..n).collect();
    let cell_volumes = vec![1.0; n];
    let cell_centres: Vec<Vector3> = (0..n).map(|c| vx(c as f64 + 0.5)).collect();
    // internal faces between i and i+1, then the two boundary faces.
    let mut face_area_vectors: Vec<Vector3> = (0..n - 1).map(|_| vx(1.0)).collect();
    face_area_vectors.push(vx(-1.0)); // inlet (outward normal points -x)
    face_area_vectors.push(vx(1.0)); // outlet
    let mut face_centres: Vec<Vector3> = (0..n - 1).map(|i| vx(i as f64 + 1.0)).collect();
    face_centres.push(vx(0.0));
    face_centres.push(vx(n as f64));

    Arc::new(
        FvMeshBuilder::new()
            .n_cells(n)
            .n_internal_faces(n - 1)
            .owner(owner)
            .neighbour(neighbour)
            .patches(vec![
                BoundaryPatch::new("inlet", n - 1, 1, PatchKind::Patch),
                BoundaryPatch::new("outlet", n, 1, PatchKind::Patch),
            ])
            .cell_volumes(cell_volumes)
            .cell_centres(cell_centres)
            .face_area_vectors(face_area_vectors)
            .face_centres(face_centres)
            .build()
            .unwrap(),
    )
}

fn main() {
    // ── 1. Mesh ──────────────────────────────────────────────────────────────
    let mesh = column_mesh(12);

    // ── 2. Two-fluid hydrodynamic core ───────────────────────────────────────
    // Dispersed: cold inert particles (5 % by volume). Continuous: hot gas.
    let particles = Phase::new(
        mesh.clone(),
        "particles",
        MassDensity::new::<kilogram_per_cubic_meter>(1800.0),
        DynamicViscosity::new::<pascal_second>(2.0e-5),
        Length::new::<meter>(5.0e-4),
        0.05,
    )
    .unwrap();
    let gas = Phase::new(
        mesh.clone(),
        "gas",
        MassDensity::new::<kilogram_per_cubic_meter>(0.6),
        DynamicViscosity::new::<pascal_second>(3.0e-5),
        Length::new::<meter>(5.0e-4),
        0.95,
    )
    .unwrap();
    let system = TwoFluidSystem::new(particles, gas).unwrap();
    let hydro = TwoFluidPimple::new(system, DragModel::SchillerNaumann);

    // ── 3. Per-phase thermal state ───────────────────────────────────────────
    // Particles start cold (400 K); gas starts hot enough to ignite (1800 K).
    let particle_thermo = PhaseThermo::new(
        mesh.clone(),
        "he.particles",
        SpecificHeatCapacity::new::<joule_per_kilogram_kelvin>(900.0),
        ThermalConductivity::new::<watt_per_meter_kelvin>(1.5),
        ThermodynamicTemperature::new::<kelvin>(400.0),
    )
    .unwrap();
    let gas_thermo = PhaseThermo::new(
        mesh.clone(),
        "he.gas",
        SpecificHeatCapacity::new::<joule_per_kilogram_kelvin>(1200.0),
        ThermalConductivity::new::<watt_per_meter_kelvin>(0.05),
        ThermodynamicTemperature::new::<kelvin>(1800.0),
    )
    .unwrap();

    // ── 4. Assemble the solver ───────────────────────────────────────────────
    let mut solver = ReactingTwoPhaseEulerFoam::new(
        hydro,
        particle_thermo,
        gas_thermo,
        Default::default(),
        Default::default(),
        Default::default(),
    )
    .unwrap();

    // Ranz-Marshall interfacial heat transfer couples the gas to the particles.
    solver.heat_transfer = InterfacialHeatTransfer::RanzMarshall;
    solver.n_energy_correctors = 2;

    // ── 5. Gas-phase composition + finite-rate combustion ────────────────────
    // Two species carried in the gas: fuel (80 %) and product (20 %).
    solver.species = Some(
        PhaseSpecies::new(
            mesh.clone(),
            PhaseSelector::Continuous,
            vec!["fuel".to_string(), "product".to_string()],
            &[0.8, 0.2],
            1.0e-5,
        )
        .unwrap(),
    );
    // Single global irreversible reaction  fuel -> product,  ΔH = 3 MJ/kg.
    solver.reaction_mechanism = ReactionMechanism::Arrhenius {
        fuel: 0,
        product: 1,
        a_pre: 2.0e4,   // 1/s
        e_act: 6.0e4,   // J/mol
        delta_h: 3.0e6, // J/kg fuel
    };

    // ── 6. March in time and report ──────────────────────────────────────────
    println!(
        "reactingTwoPhaseEulerFoam demo — hot gas burning fuel and heating a cold particle bed\n"
    );
    println!(
        "{:>8}  {:>10}  {:>12}  {:>9}  {:>9}",
        "t [ms]", "T_gas [K]", "T_part [K]", "Y_fuel", "Y_prod"
    );

    let dt = 5.0e-4;
    let report_every = 5;
    for step in 0..=40 {
        if step % report_every == 0 {
            let sp = solver.species.as_ref().unwrap();
            println!(
                "{:>8.2}  {:>10.1}  {:>12.1}  {:>9.4}  {:>9.4}",
                solver.time * 1.0e3,
                solver.continuous_thermo.mean_temperature(),
                solver.dispersed_thermo.mean_temperature(),
                sp.mean_mass_fraction(0),
                sp.mean_mass_fraction(1),
            );
        }
        if step < 40 {
            solver.solve_timestep(dt).expect("solver step failed");
        }
    }

    let sp = solver.species.as_ref().unwrap();
    println!(
        "\nBurned {:.1} % of the fuel; gas and particle temperatures converged from \
         a 1400 K split toward a common value.",
        100.0 * (0.8 - sp.mean_mass_fraction(0)) / 0.8
    );
    println!(
        "(Verification demo only — these numbers are not validated against any benchmark.)"
    );
}
