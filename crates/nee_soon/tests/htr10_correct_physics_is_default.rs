// SPDX-License-Identifier: GPL-3.0-only
//! **The HTR-10 model's correct physics is the DEFAULT, and this test keeps it
//! that way** (workspace hard rule, 2026-09-20; the pattern of
//! `crates/outram-mc-libs/tests/correct_physics_is_default.rs`).
//!
//! Three maintainer decisions of 2026-10-01 changed what the default HTR-10
//! nuclide set and materials carry. Each was a physics term the data supplied
//! and the model left out by default:
//!
//! - **natural carbon** (gh:#425): on ENDF/B-VIII.0 every carbon was C-12 at the
//!   natural-carbon density although the C-13 tape is in the checkout;
//! - **helium coolant** (gh:#426): `mat::HELIUM` was an empty material, i.e.
//!   exact vacuum;
//! - **real nickel and iron** in the rod steel (gh:#329): only the Ni -> Fe /
//!   Fe-57 -> Fe-56 substitutions could run.
//!
//! This constructs through the ordinary path (`Htr10DataConfig::default()` ->
//! `Htr10NuclideLayout::plan` -> `htr10_material_set`) and asserts each term is
//! present. **Counting a symbol's presence is not checking a default**, so it
//! checks the built compositions, not the enum names. Pure: no nuclear data
//! is read.
//!
//! Result (2026-10-01): passes.

use nee_soon::htr10_rmc::core_model::mat;
use nee_soon::htr10_rmc::data::{
    CarbonTreatment, Coolant, CoolantNuclides, DataDir, Htr10DataConfig, Htr10NuclideLayout,
    NuclearDataLibrary, RodMetalTreatment, ThermalLaw, ThermalScatteringTreatment,
};
use nee_soon::htr10_rmc::materials::{htr10_material_set, GraphiteLaw, Htr10MaterialConfig};
use outram_mc_libs::pebble_beds::htr10::CarbonSlot;

#[test]
fn the_default_htr10_data_configuration_is_the_correct_physics() {
    let cfg = Htr10DataConfig::default();
    assert_eq!(cfg.library, NuclearDataLibrary::EndfB8);
    assert_eq!(cfg.carbon, CarbonTreatment::Natural);
    assert_eq!(cfg.coolant, Coolant::Helium);
    assert_eq!(cfg.rod_metal, RodMetalTreatment::Full);
    assert_eq!(cfg.thermal, ThermalScatteringTreatment::Bound);
    assert_eq!(cfg.graphite_law, GraphiteLaw::Reactor30P);
}

#[test]
fn the_default_materials_carry_c13_helium_and_nickel() {
    let layout = Htr10NuclideLayout::plan(&Htr10DataConfig::default()).expect("valid");
    let mats = htr10_material_set(&layout, Htr10MaterialConfig::benchmark_default(300.15));

    // C-13 is present, with the graphite law bound to it, in the matrix
    // graphite and the reflector.
    let CarbonSlot::Natural { c13, .. } = layout.pebble.c_graphite else {
        panic!("default carbon must be natural C-12 / C-13");
    };
    assert_eq!(layout.slots[c13].tape.file, "n-006_C_013-ENDF8.0.endf");
    assert!(matches!(
        layout.slots[c13].thermal,
        Some(ThermalLaw::Graphite { .. })
    ));
    for k in [mat::GRAPHITE, mat::REFLECTOR] {
        assert!(
            mats[k]
                .components
                .iter()
                .any(|c| c.nuclide_idx == c13 && c.atom_density > 0.0),
            "{} carries no C-13",
            mats[k].name
        );
    }

    // The coolant is helium, not an empty material.
    assert!(matches!(layout.coolant, CoolantNuclides::Helium { .. }));
    assert!(
        !mats[mat::HELIUM].components.is_empty(),
        "coolant is vacuum"
    );

    // The rod steel carries real nickel, read from the ACE submodule.
    let ni = layout.metal.ni58;
    assert_eq!(layout.slots[ni].name, "Ni58");
    assert_eq!(layout.slots[ni].tape.dir, DataDir::AceSubmoduleEndfB8);
    assert!(mats[mat::ROD_STEEL]
        .components
        .iter()
        .any(|c| c.nuclide_idx == ni));
    assert_ne!(
        layout.metal.fe57, layout.metal.fe56,
        "Fe-57 must be its own nuclide"
    );
}
