//! GitHub #462: S(α,β) belongs to the material, as in OpenMC
//! (`src/material.cpp:847-885`), not to the shared nuclide entry.
//!
//! Methodology: H-1 (CORE data) is shared by two materials, "water" and
//! "free H". Only water names H in H2O (LEAPR deck embedded in
//! njoy-outram-park-fork, 293.6 K) through `assign_material_sab`. Pass
//! criteria, all exact: the free material keeps the unbound entry and its
//! macroscopic total at 0.0253 eV equals `N σ_free` bit for bit; water uses
//! a bound copy whose total differs; a thermal collision on the free entry
//! has no bound law to sample; and a `fraction = 0.25` split reproduces
//! `0.25 N σ_bound + 0.75 N σ_free`, which is upstream's
//! `σ_free + f (σ_sab − σ_el)` rearranged.
//!
//! Result (2026-11-14): see the test output recorded on #462.

use njoy_outram_park_fork::leapr::decks::SabMaterial;
use outram_mc_libs::material::material::{Material, NuclideComponent};
use outram_mc_libs::material::material_sab::{assign_material_sab, MaterialSab};
use outram_mc_libs::material::nuclide::Nuclide;
use outram_mc_libs::material::thermal::ThermalScattering;

const T: f64 = 293.6;
const N_H: f64 = 6.7e-2;
const E: f64 = 0.0253;

fn mat(id: i32, name: &str) -> Material {
    Material {
        id,
        name: name.into(),
        temperature: T,
        components: vec![NuclideComponent { nuclide_idx: 0, atom_density: N_H }],
    }
}

#[test]
fn each_material_scatters_with_its_own_law() {
    let h1 = Nuclide::from_core("H1").unwrap();
    let table = ThermalScattering::from_embedded_leapr_deck(SabMaterial::HInH2O, T, "HInH2O").unwrap();
    let materials = vec![mat(1, "water"), mat(2, "free H")];
    let sab = [MaterialSab { material: 0, nuclide: 0, table: 0, fraction: 1.0 }];
    let (mats, nucs) =
        assign_material_sab(&materials, std::slice::from_ref(&h1), std::slice::from_ref(&table), &sab).unwrap();

    assert_eq!(nucs.len(), 2);
    assert!(!nucs[0].has_thermal_scattering());
    assert!(nucs[1].has_thermal_scattering());
    assert_eq!(mats[0].components[0].nuclide_idx, 1, "water uses the bound copy");
    assert_eq!(mats[1].components[0].nuclide_idx, 0, "free H keeps the unbound entry");

    let free = N_H * h1.total_at_energy(E, T);
    let free_mat = mats[1].macro_xs_total(E, &nucs);
    let water = mats[0].macro_xs_total(E, &nucs);
    assert_eq!(free_mat.to_bits(), free.to_bits());
    assert_ne!(water.to_bits(), free.to_bits());
    println!("Sigma_t at {E} eV: water (bound) {water:.6} /cm, free H {free_mat:.6} /cm");

    // A collision in the free material picks entry 0, which has no bound law.
    let mut seed = 11_u64;
    // (`sample_nuclide` returns the component index.)
    let c = mats[1].sample_nuclide(E, &mut seed, &nucs);
    let pick = mats[1].components[c].nuclide_idx;
    assert_eq!(pick, 0);
    assert!(nucs[pick].sample_thermal(E, &mut seed).is_none());
    let c = mats[0].sample_nuclide(E, &mut seed, &nucs);
    let pick = mats[0].components[c].nuclide_idx;
    assert_eq!(pick, 1);
    assert!(nucs[pick].sample_thermal(E, &mut seed).is_some());

    // sab_frac = 0.25: split component.
    let sab = [MaterialSab { material: 0, nuclide: 0, table: 0, fraction: 0.25 }];
    let (mats, nucs) =
        assign_material_sab(&materials, std::slice::from_ref(&h1), std::slice::from_ref(&table), &sab).unwrap();
    assert_eq!(mats[0].components.len(), 2);
    let mixed = mats[0].macro_xs_total(E, &nucs);
    let want = 0.25 * N_H * nucs[1].total_at_energy(E, T) + 0.75 * N_H * h1.total_at_energy(E, T);
    assert!(((mixed - want) / want).abs() < 1e-15, "{mixed} vs {want}");

    // Refusals: a nuclide already bound by the old per-nuclide API, a bad
    // fraction, a material without the nuclide.
    let bound = h1.clone().with_thermal_scattering(table.clone());
    assert!(assign_material_sab(&materials, &[bound], std::slice::from_ref(&table), &sab)
        .unwrap_err()
        .contains("#462"));
    let bad = [MaterialSab { material: 0, nuclide: 0, table: 0, fraction: 0.0 }];
    assert!(assign_material_sab(&materials, std::slice::from_ref(&h1), std::slice::from_ref(&table), &bad).is_err());
    let other = vec![Material { components: vec![], ..mat(3, "empty") }];
    let s = [MaterialSab { material: 0, nuclide: 0, table: 0, fraction: 1.0 }];
    assert!(assign_material_sab(&other, std::slice::from_ref(&h1), std::slice::from_ref(&table), &s).is_err());
}
