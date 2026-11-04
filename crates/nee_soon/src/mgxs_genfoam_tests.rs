// SPDX-License-Identifier: AGPL-3.0-only
// Copyright (C) 2026 OUTRAM PARK contributors

//! Tests of `mgxs_genfoam` (gh:#1187): the state points, the defaults and
//! their ablations (pinned), the stacking, the volume mixing. They read the
//! on-disk `htr10_mgxs` records (~1.2 MB JSON each). The mesh and the
//! diffusion comparison are `examples/htr10_mgxs_genfoam_rz.rs`.

use super::*;

fn regions() -> Htr10MgxsRegions {
    Htr10MgxsRegions::from_regions(vec![
        ("bed".into(), RegionRecipe::Bed { solid_fraction: 0.61 }),
        ("dummyBed".into(), RegionRecipe::DummyBed { solid_fraction: 0.61 }),
        ("cavity".into(), RegionRecipe::Cavity),
        ("refl".into(), RegionRecipe::Slot(mat::REFLECTOR)),
    ])
}

/// The four records load, ascending in temperature, on one bed, with the
/// `k` the index lists.
#[test]
fn the_four_records_are_the_state_points() {
    let p = MgxsStatePoints::htr10_records().unwrap();
    let t: Vec<f64> = p.points.iter().map(|x| x.temperature.get::<kelvin>()).collect();
    assert_eq!(t, vec![300.15, 600.0, 900.0, 1200.0]);
    assert_eq!(p.layers(), 12);
    assert!((p.points[0].k_eff.get::<ratio>() - 0.996548).abs() < 1e-6);
    assert!((p.points[3].k_eff.get::<ratio>() - 0.885270).abs() < 1e-6);
    assert!(p.slot_has_flux(mat::KERNEL) && p.slot_has_flux(mat::REFLECTOR));
    assert_eq!(layers_of("full HTR-10 core, Şeker 13-ball bed N = 12 (123.576 cm)"), Some(12));
}

/// **Every default ON, pinned**: rebalanced, descending, SI. Each ablation
/// changes exactly what it names, against the default input.
#[test]
fn the_defaults_are_on_and_each_ablation_changes_its_own_thing() {
    let p = MgxsStatePoints::htr10_records().unwrap();
    let lib = &p.points[0].library;
    let r = regions();
    assert_eq!(r.defaults(), (true, true, true));
    let nd = r.single_state(lib).unwrap();
    let bed = &nd.states[0].zones[0];
    // Descending: the fission spectrum peaks in group 0 (the fastest).
    assert!(bed.chi_prompt[0] > bed.chi_prompt[3], "{:?}", bed.chi_prompt);
    // SI: a graphite-bed D is of order a centimetre, i.e. ~0.01 m.
    assert!(bed.d.iter().all(|d| *d > 1e-3 && *d < 0.1), "{:?}", bed.d);

    let asc = r.clone().without_descending_order();
    assert_eq!(asc.defaults(), (true, false, true));
    let a = asc.single_state(lib).unwrap();
    let ab = &a.states[0].zones[0];
    assert!(ab.chi_prompt[3] > ab.chi_prompt[0]);
    for g in 0..4 {
        assert_eq!(ab.d[g].to_bits(), bed.d[3 - g].to_bits());
    }

    let cm_units = r.clone().without_unit_conversion();
    let c = cm_units.single_state(lib).unwrap();
    for g in 0..4 {
        assert!((c.states[0].zones[0].d[g] / (bed.d[g] * 100.0) - 1.0).abs() < 1e-12);
        assert!((c.states[0].zones[0].sigma_removal[g] * 100.0 / bed.sigma_removal[g] - 1.0).abs() < 1e-12);
    }

    let raw = r.clone().without_rebalance();
    let w = raw.single_state(lib).unwrap();
    let moved = (0..4).any(|g| w.states[0].zones[0].sigma_removal[g] != bed.sigma_removal[g]);
    assert!(moved, "the rebalance must change the removal");
    // Scattering and production are untouched by the rebalance.
    assert_eq!(w.states[0].zones[0].nu_sigma_eff, bed.nu_sigma_eff);
}

/// The stack has one variable, `TFuel` with the log law, the coldest point
/// as `reference`, every state carrying its temperature; it builds
/// GeN-Foam's `CrossSectionData`.
#[test]
fn the_state_points_stack_into_one_parametrised_input() {
    let p = MgxsStatePoints::htr10_records().unwrap();
    let nd = regions().nuclear_data(&p).unwrap();
    assert_eq!(nd.xs_variables.len(), 1);
    assert_eq!(nd.xs_variables[0].name, "TFuel");
    assert_eq!(nd.xs_variables[0].law, VariableLaw::Log);
    assert_eq!(nd.states.len(), 4);
    assert_eq!(nd.states[0].name, "reference");
    assert_eq!(nd.states[0].parameters["TFuel"], 300.15);
    assert_eq!(nd.states[3].name, "T1200K");
    let xs = CrossSectionData::from_input(&nd).unwrap();
    assert_eq!(xs.variable_names(), vec!["TFuel"]);
    assert_eq!(xs.energy_groups(), 4);
}

/// Volume mixing a zone with itself is the zone; mixing with a zero-cross-
/// section zone dilutes every cross section by the fraction.
#[test]
fn volume_mixing() {
    let p = MgxsStatePoints::htr10_records().unwrap();
    let z = &p.points[0].library.zones[mat::REFLECTOR];
    let same = mix_by_volume(&[(z, 0.3), (z, 0.7)]);
    for g in 0..z.total.len() {
        assert!((same.total[g] / z.total[g] - 1.0).abs() < 1e-12);
        assert!((same.scatter[g][g] / z.scatter[g][g] - 1.0).abs() < 1e-12 || z.scatter[g][g] == 0.0);
    }
    let mut empty = z.clone();
    for v in [&mut empty.total, &mut empty.absorption, &mut empty.nu_fission] {
        v.iter_mut().for_each(|x| *x = 0.0);
    }
    empty.scatter.iter_mut().for_each(|r| r.iter_mut().for_each(|x| *x = 0.0));
    let half = mix_by_volume(&[(z, 0.5), (&empty, 0.5)]);
    assert!((half.total[10] / z.total[10] - 0.5).abs() < 1e-12);
}
