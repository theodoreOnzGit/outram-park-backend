//! GitHub #1581: `assign_htr10_sab` lowers the three HTR-10 laws per material.
//!
//! **Methodology.** The HTR-10 slot layout (`Htr10Nuclides::NATURAL_CARBON`,
//! 14 slots) is filled with free stand-in nuclides (H-1 from the embedded CORE
//! data; the function does not look at what a slot holds), the pebble
//! materials are built as the drivers build them, and the three laws are
//! attached with [`assign_htr10_sab`]. The check is structural, because the
//! per-material association is a bookkeeping change:
//!
//! - every original slot stays free and in place;
//! - every component that referenced a graphite carbon slot now references a
//!   bound copy, and likewise the SiC carbon slots (C-in-SiC);
//! - a slot with no law named (Si here, `si_in_sic: None`), the kernel's
//!   U/O slots and the boron slots keep their free entry;
//! - atom densities and component order are unchanged.
//!
//! **Prediction (before running):** 14 free slots + 4 bound copies
//! (C-12 / C-13 graphite, C-12 / C-13 in SiC) = 18 nuclides; nothing else
//! moves.
//!
//! **Result (2026-11-14):** see the assertions below; the test passes.

use njoy_outram_park_fork::leapr::decks::SabMaterial;
use outram_mc_libs::material::material_sab::{assign_htr10_sab, Htr10SabLaws};
use outram_mc_libs::material::nuclide::Nuclide;
use outram_mc_libs::material::thermal::ThermalScattering;
use outram_mc_libs::pebble_beds::htr10::{fuel_pebble_materials, BoronReading, Htr10Nuclides};

const T: f64 = 300.15;
const LAYOUT: Htr10Nuclides = Htr10Nuclides::NATURAL_CARBON;

#[test]
fn htr10_laws_go_only_to_the_materials_that_hold_each_slot() {
    let slots = LAYOUT.slot_count();
    assert_eq!(slots, 14);
    let free: Vec<Nuclide> = (0..slots).map(|_| Nuclide::from_core("H1").unwrap()).collect();
    let laws = Htr10SabLaws {
        graphite: ThermalScattering::from_embedded_leapr_deck(
            SabMaterial::CrystallineGraphite,
            T,
            "c_Graphite",
        )
        .unwrap(),
        c_in_sic: Some(
            ThermalScattering::from_embedded_leapr_deck(SabMaterial::CInSiC, T, "c_SiC").unwrap(),
        ),
        si_in_sic: None,
    };
    let materials = fuel_pebble_materials(LAYOUT, BoronReading::Natural, T);
    let (mats, nucs) = assign_htr10_sab(&materials, &free, &LAYOUT, &laws).unwrap();

    assert_eq!(nucs.len(), slots + 4, "four bound copies are appended");
    assert!(nucs[..slots].iter().all(|n| !n.has_thermal_scattering()), "the slots stay free");
    assert!(nucs[slots..].iter().all(|n| n.has_thermal_scattering()));

    let graphite = LAYOUT.c_graphite.slots();
    let sic = LAYOUT.c_sic.slots();
    let mut bound_graphite = 0;
    let mut bound_sic = 0;
    for (old, new) in materials.iter().zip(&mats) {
        assert_eq!(old.components.len(), new.components.len(), "{}", old.name);
        for (a, b) in old.components.iter().zip(&new.components) {
            assert_eq!(a.atom_density.to_bits(), b.atom_density.to_bits());
            if graphite.contains(&a.nuclide_idx) {
                assert!(b.nuclide_idx >= slots, "{}: graphite carbon is bound", old.name);
                bound_graphite += 1;
            } else if sic.contains(&a.nuclide_idx) {
                assert!(b.nuclide_idx >= slots, "{}: SiC carbon is bound", old.name);
                bound_sic += 1;
            } else {
                assert_eq!(
                    a.nuclide_idx, b.nuclide_idx,
                    "{}: slot {} has no law and stays put",
                    old.name, a.nuclide_idx
                );
            }
        }
    }
    assert!(bound_graphite > 0 && bound_sic > 0, "the pebble holds both kinds of carbon");
}
