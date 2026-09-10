// SPDX-License-Identifier: GPL-3.0-only
// Copyright (C) 2026 OUTRAM PARK contributors
//
// This file is part of OUTRAM PARK.

//! **Crude oil → cut slate, end to end, from one import.**
//!
//! The acceptance workflow of GitHub issue #70 ("declare
//! `outram-park-fork-dwsim-libs` mature and dogfood it"), written the way a
//! first-time user would write it: a single
//! `use outram_park_fork_dwsim_libs::prelude::*;` and nothing else from the
//! crate. If a step below needs a name that is not in the prelude, that is a
//! defect in the prelude, not in this example.
//!
//! ## What it does
//!
//! 1. Describes a **heavy crude** the black-oil way — 22 °API, gas gravity
//!    0.80 ([`BlackOilCrude::heavy`]) — and prints the bulk properties the
//!    black-oil correlations imply (specific gravity, mean molar mass, mean
//!    normal boiling point).
//! 2. Cuts it into **12 pseudo-components** ([`BlackOilCrude::pseudo_components`])
//!    and prints the slate: normal boiling point, `Tc`, `Pc`, `ω`, mole
//!    fraction, and which conventional refinery cut each lands in.
//! 3. Takes the default **12-stage atmospheric column** with three side draws
//!    ([`CrudeColumnConfig::atmospheric_default`]), which runs on
//!    **Peng-Robinson 1978** — the α-function that stays valid past `ω = 0.49`,
//!    which heavy pseudo-components exceed.
//! 4. Solves the **rigorous MESH column** ([`solve_crude_column`]) and prints
//!    the cut slate: distillate, kerosene, diesel, AGO draws and atmospheric
//!    residue, with the converged draw temperatures.
//! 5. Checks the material balance closes on the whole crude.
//!
//! ## Units
//!
//! The crate's documented SI base units throughout: pressure Pa, temperature K,
//! molar flow mol/s, molar mass kg/mol (printed as g/mol where a refiner would
//! expect it).
//!
//! ## Honest scope
//!
//! A **scoping calculation, not a validated yield prediction.** The
//! pseudo-component slate is a distribution assumption from two bulk numbers,
//! not a measured assay; the column is reboiled rather than steam-stripped (see
//! [`CrudeColumnConfig`]); the cut labels are assigned from converged draw
//! *temperatures* after the fact and nothing constrains a draw to land in a
//! band. The heavy end above the residue cut point bypasses the column to the
//! residue, which is both what makes the solve possible and what physically
//! happens in a CDU. Independent OUTRAM PARK fork, not the official DWSIM; not
//! for nuclear, safety-critical or operational use.
//!
//! ## Measured output (2026-09-10, release build)
//!
//! 22 °API → SG 0.9218, mean molar mass 499.1 g/mol, mean NBP 767.6 K; the
//! column converged in **38** iterations to a final error of `8.5103e-7`;
//! naphtha 0.15005 mol/s (442.4 K), kerosene 0.04001 (524.2 K), diesel
//! 0.03335 (537.9 K), diesel 0.02668 (550.7 K), residue 0.74991 (601.5 K);
//! products total 1.000000 mol/s. These reproduce the figures recorded in #70
//! from the Python bindings to every quoted decimal.
//!
//! Run with:
//! `cargo run -p outram-park-fork-dwsim-libs --release --example crude_column_from_prelude`

use outram_park_fork_dwsim_libs::prelude::*;

fn main() {
    println!("======================================================================");
    println!(" OUTRAM PARK / DWSIM-fork  —  atmospheric crude column from a black-oil spec");
    println!("======================================================================");

    // ── 1. The crude, described the way a production engineer describes one ──
    let crude = BlackOilCrude::heavy();
    println!(
        " Crude            : {:.1} °API, gas SG {:.2}, BS&W {:.1} %vol",
        crude.api_gravity, crude.gas_specific_gravity, crude.bsw_percent
    );
    println!(
        " Oil SG (60/60 F) : {:.4}   [= 141.5 / (°API + 131.5)]",
        crude.oil_specific_gravity()
    );
    println!(
        " Mean molar mass  : {:.1} g/mol",
        crude.liquid_molar_mass_g_per_mol()
    );
    println!(
        " Mean NBP         : {:.1} K  ({:.0} °C)",
        crude.mean_normal_boiling_point_k(),
        crude.mean_normal_boiling_point_k() - 273.15
    );
    println!();

    // ── 2. Pseudo-component characterisation ─────────────────────────────────
    let cut_count = 12;
    let slate: Vec<PseudoComponent> = match crude.pseudo_components(cut_count) {
        Ok(s) => s,
        Err(e) => {
            eprintln!("characterisation failed: {e:?}");
            std::process::exit(1);
        }
    };
    println!("----------------------------------------------------------------------");
    println!(" {cut_count} pseudo-components (ascending normal boiling point)");
    println!("----------------------------------------------------------------------");
    println!(
        "   {:<16} {:>8} {:>8} {:>9} {:>7} {:>8}  {}",
        "name", "Tb [K]", "Tc [K]", "Pc [bar]", "omega", "z [-]", "cut"
    );
    for pc in &slate {
        let c = &pc.component;
        // `mole_fraction` is a uom Ratio; read it back as a plain number.
        let z = pc.mole_fraction.get::<units::ratio>();
        println!(
            "   {:<16} {:>8.1} {:>8.1} {:>9.2} {:>7.3} {:>8.4}  {}",
            c.name,
            c.normal_boiling_point,
            c.critical_temperature,
            c.critical_pressure / 1.0e5,
            c.acentric_factor,
            z,
            CrudeCut::from_normal_boiling_point_k(c.normal_boiling_point).label()
        );
    }
    let z_sum: f64 = slate
        .iter()
        .map(|pc| pc.mole_fraction.get::<units::ratio>())
        .sum();
    println!("   mole fractions sum to {z_sum:.9}");
    println!();

    // ── 3. The column ────────────────────────────────────────────────────────
    let config = CrudeColumnConfig::atmospheric_default();
    println!("----------------------------------------------------------------------");
    println!(
        " Column: {} stages, feed on stage {}, {:.2} bar, reflux ratio {:.1}",
        config.n_stages,
        config.feed_stage,
        config.pressure_pa / 1.0e5,
        config.reflux_ratio
    );
    println!(
        " Feed {:.3} mol/s; bottoms {:.2} of column feed; side draws {:?}",
        config.feed_flow_mol_s, config.bottoms_fraction, config.side_draws
    );
    println!(
        " Residue cut point {:.0} K (heavier pseudo-components bypass to residue)",
        config.residue_cut_point_k
    );
    println!(" Thermodynamics: {:?}", config.package);
    assert_eq!(config.package, PropertyPackageModel::PengRobinson1978);
    println!("----------------------------------------------------------------------");

    // ── 4. Solve ─────────────────────────────────────────────────────────────
    let result: CrudeColumnResult = match solve_crude_column(&crude, &config, cut_count) {
        Ok(r) => r,
        Err(e) => {
            eprintln!("column solve failed: {e}");
            std::process::exit(1);
        }
    };
    println!(
        " Converged in {} inner iterations, final error {:.4e}",
        result.iterations, result.final_error
    );
    println!();
    println!(
        "   {:<6} {:>12} {:>10} {:>8}  {}",
        "stage", "flow [mol/s]", "T [K]", "T [C]", "cut"
    );
    for cut in &result.cuts {
        println!(
            "   {:<6} {:>12.5} {:>10.2} {:>8.1}  {}",
            cut.stage,
            cut.flow_mol_s,
            cut.temperature_k,
            cut.temperature_k - 273.15,
            cut.cut.label()
        );
    }
    println!();
    println!(" Stage temperatures, condenser -> reboiler [K]:");
    println!(
        "   {:?}",
        result
            .stage_temperatures_k
            .iter()
            .map(|t| (t * 100.0).round() / 100.0)
            .collect::<Vec<_>>()
    );

    // ── 5. Material balance on the whole crude ───────────────────────────────
    let total = result.total_product_mol_s();
    println!();
    println!(
        " Products total {total:.6} mol/s against a {:.6} mol/s feed (residual {:.2e})",
        config.feed_flow_mol_s,
        total - config.feed_flow_mol_s
    );
    println!("======================================================================");
    println!(" Scoping result: pseudo-components from two bulk numbers, reboiled column,");
    println!(" cut labels from draw temperatures. Not a validated yield prediction.");
    println!("======================================================================");
}
