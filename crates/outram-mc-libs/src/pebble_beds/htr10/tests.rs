// SPDX-License-Identifier: GPL-3.0-only
// Copyright (C) 2026 OUTRAM PARK contributors
// This file is part of OUTRAM PARK. See the module header for licence terms.

//! # Verification — HTR-10 Table 2 composition
//!
//! ## Methodology
//!
//! Table 2 of Li, Yu & Wei (2014) is **over-determined**: it states the
//! heavy-metal loading (5 g/ball) *and* everything needed to compute it (8335
//! kernels, 250 µm kernel radius, 10.4 g/cm3 UO2, 17 % enrichment). The loading
//! is therefore a closure the geometry never sees, and reproducing it is a real
//! check that the table was transcribed faithfully rather than rounded. The
//! packing fraction is over-determined the same way.
//!
//! ## Results (measured 2026-09-16)
//!
//! | check | tolerance | measured |
//! |---|---|---|
//! | heavy metal per ball vs the stated 5 g | 1 mg | **5.00007 g** |
//! | packing fraction vs the stored constant | 1e-6 | agrees |
//! | elemental-B-10 misreading vs natural | — | **5.43x** more B-10 |
//!
//! These are arithmetic, not transport. Eigenvalue consequences are measured in
//! `examples/htr10_pebble_delta_tracking.rs`.

use super::*;
use crate::prelude::TrisoSpec;
use std::f64::consts::PI;

const KERNELS_PER_BALL: f64 = 8335.0;
const FUEL_ZONE_RADIUS: f64 = 2.5;

fn nuclides() -> Htr10Nuclides {
    Htr10Nuclides {
        u235: 0,
        u238: 1,
        o16: 2,
        c_free: 3,
        c_graphite: 4,
        si28: 5,
        b10: 6,
        c_sic: 7,
        si29: 8,
        si30: 9,
    }
}

fn b10_inventory(n: Htr10Nuclides, r: BoronReading) -> f64 {
    fuel_pebble_materials(n, r, 300.15)
        .iter()
        .flat_map(|m| m.components.iter())
        .filter(|c| c.nuclide_idx == n.b10)
        .map(|c| c.atom_density)
        .sum()
}

/// **The 5 g closure.** Table 2 states 5 g of heavy metal per ball and also
/// states everything needed to derive it; they must agree.
#[test]
fn the_tabulated_heavy_metal_loading_closes_to_a_milligram() {
    let spec = TrisoSpec::HTR10_LI2014;
    let x5 = u235_atom_fraction();
    let m_u = x5 * 235.043_930 + (1.0 - x5) * 238.050_788;
    let m_uo2 = m_u + 2.0 * 15.994_914_6;

    let v_kernel = 4.0 / 3.0 * PI * spec.kernel.powi(3);
    let hm = KERNELS_PER_BALL * v_kernel * RHO_UO2 * m_u / m_uo2;

    assert!(
        (hm - 5.0).abs() < 1.0e-3,
        "heavy metal per ball = {hm:.5} g, Table 2 states 5 g — a mismatch means \
         the kernel radius, UO2 density, kernel count or enrichment has drifted \
         from the source"
    );
}

/// **The packing-fraction closure.** The stored constant is *derived* (Table 2
/// quotes none), so this pins the derivation to its inputs.
#[test]
fn the_stored_packing_fraction_is_what_the_geometry_implies() {
    let spec = TrisoSpec::HTR10_LI2014;
    let implied = KERNELS_PER_BALL * (spec.opyc / FUEL_ZONE_RADIUS).powi(3);
    assert!(
        (implied - spec.packing_fraction).abs() < 1.0e-6,
        "stored packing fraction {} but 8335 particles of radius {} in a {} cm \
         fuel zone imply {implied}",
        spec.packing_fraction,
        spec.opyc,
        FUEL_ZONE_RADIUS
    );
}

/// **The ablation knob actually turns.**
///
/// The failure mode this guards is a silent no-op control: every arm reports the
/// same k, the study concludes "boron does not matter", and that conclusion is
/// an artefact of the knob being disconnected. Assert the readings give
/// materially different B-10 inventories *before* trusting any eigenvalue
/// difference between them.
#[test]
fn the_four_boron_readings_are_genuinely_different_compositions() {
    let n = nuclides();
    let natural = b10_inventory(n, BoronReading::Natural);
    let none = b10_inventory(n, BoronReading::None);
    let graphite_only = b10_inventory(n, BoronReading::GraphiteOnly);
    let elemental = b10_inventory(n, BoronReading::AsElementalB10);

    assert_eq!(none, 0.0, "the `None` arm must carry no B-10 at all");
    assert!(natural > 0.0, "the `Natural` arm must carry B-10");
    assert!(
        graphite_only > 0.0 && graphite_only < natural,
        "`GraphiteOnly` ({graphite_only:e}) drops the kernel's boron but keeps \
         the graphite's, so it must sit strictly below `Natural` ({natural:e})"
    );

    let ratio = elemental / natural;
    assert!(
        (ratio - 1.0 / B10_WEIGHT_FRACTION_OF_NATURAL_B).abs() < 0.01,
        "reading ppm as elemental B-10 should raise the B-10 inventory by \
         1/{B10_WEIGHT_FRACTION_OF_NATURAL_B} = 5.43x, got {ratio:.3}x"
    );
}

/// The kernel's boron rides on the **uranium** mass density, not the UO2
/// density, because Table 2 quotes it "of uranium". Getting that wrong
/// over-states it by `m_UO2/m_U` = 1.135x.
#[test]
fn the_kernel_boron_is_quoted_relative_to_uranium_not_uo2() {
    let n = nuclides();
    let mats = fuel_pebble_materials(n, BoronReading::Natural, 300.15);
    let kernel_b10: f64 = mats[0]
        .components
        .iter()
        .filter(|c| c.nuclide_idx == n.b10)
        .map(|c| c.atom_density)
        .sum();

    let x5 = u235_atom_fraction();
    let m_u = x5 * 235.043_930 + (1.0 - x5) * 238.050_788;
    let rho_u = RHO_UO2 * m_u / (m_u + 2.0 * 15.994_914_6);
    let expected = b10_atom_density(rho_u, B_PPM_URANIUM, BoronReading::Natural);

    assert!(
        (kernel_b10 - expected).abs() / expected < 1.0e-12,
        "kernel B-10 {kernel_b10:e} should be {expected:e} — computed against the \
         uranium density {rho_u:.4} g/cm3, not the 10.4 g/cm3 UO2 density"
    );
}

/// The table is the seven `DhUniverse::pebble` requires, in order, at the
/// requested temperature.
#[test]
fn the_material_table_is_seven_entries_in_pebble_order() {
    let mats = fuel_pebble_materials(nuclides(), BoronReading::Natural, 300.15);
    assert_eq!(mats.len(), 7, "DhUniverse::pebble requires seven materials");
    let names: Vec<&str> = mats.iter().map(|m| m.name.as_str()).collect();
    assert_eq!(
        names,
        vec![
            "UO2 kernel",
            "buffer PyC",
            "IPyC",
            "SiC",
            "OPyC",
            "matrix graphite",
            "shell graphite"
        ]
    );
    assert!(mats.iter().all(|m| (m.temperature - 300.15).abs() < 1e-12));
}

/// 17 % read as weight gives 17.18 at%. The two readings are 1.06 % apart in
/// U-235 number density — small, but not nothing, and the module documents the
/// choice for exactly that reason.
#[test]
fn the_enrichment_basis_is_weight_percent_and_the_difference_is_recorded() {
    let x5 = u235_atom_fraction();
    assert!(
        (x5 - 0.171_801).abs() < 1.0e-6,
        "17 wt% should give 17.1801 at%, got {:.6} at%",
        x5 * 100.0
    );
    let relative = (x5 - ENRICHMENT_WT) / ENRICHMENT_WT;
    assert!(
        (relative - 0.010_6).abs() < 1.0e-3,
        "the weight/atom reading gap should be ~1.06 %, got {:.3} %",
        relative * 100.0
    );
}

/// **Independent regression on the atom densities themselves.**
///
/// These figures were computed by hand from Table 2, outside this crate, before
/// [`fuel_pebble_materials`] existed — so they are an external check on the
/// builder rather than a snapshot of it. They also pin the composition across
/// the refactor that moved this arithmetic out of
/// `examples/htr10_pebble_delta_tracking.rs` and into the library, which is what
/// lets an eigenvalue measured before that move still be quoted after it.
///
/// Units are atoms/b-cm. Tolerance is 1e-4 relative — tight enough to catch a
/// transposed digit or a wrong molar mass, loose enough not to fail on the
/// last-digit rounding of the hand values.
#[test]
fn the_atom_densities_match_values_computed_independently_from_table_2() {
    let n = nuclides();
    let mats = fuel_pebble_materials(n, BoronReading::Natural, 300.15);
    let density = |mat: usize, nuclide: usize| -> f64 {
        mats[mat]
            .components
            .iter()
            .find(|c| c.nuclide_idx == nuclide)
            .map(|c| c.atom_density)
            .unwrap_or(0.0)
    };
    let close = |got: f64, want: f64, what: &str| {
        assert!(
            (got - want).abs() / want < 1.0e-4,
            "{what}: builder gives {got:.6e}, hand calculation from Table 2 gives {want:.6e}"
        );
    };

    // kernel — 17 wt% UO2 at 10.4 g/cm3, plus 4 ppm natural B of uranium
    close(density(0, n.u235), 3.992_20e-3, "kernel U-235");
    close(density(0, n.u238), 1.924_52e-2, "kernel U-238");
    close(density(0, n.o16), 4.647_47e-2, "kernel O-16");
    close(density(0, n.b10), 4.064_05e-7, "kernel B-10");

    // coatings
    close(density(1, n.c_graphite), 5.515_24e-2, "buffer C (rho 1.1)");
    close(density(2, n.c_graphite), 9.526_32e-2, "IPyC C (rho 1.9)");
    close(density(4, n.c_graphite), 9.526_32e-2, "OPyC C (rho 1.9)");
    // SiC. Silicon is split over its three natural isotopes as of
    // 2026-09-23, so the TOTAL is what Table 2 pins down -- checking the
    // Si-28 slot alone against the total would now fail for the right
    // reason. Summing is also the stronger check: it verifies the split
    // CONSERVES silicon, which is the property that can actually go wrong.
    let si_total = density(3, n.si28) + density(3, n.si29) + density(3, n.si30);
    close(
        si_total,
        4.776_08e-2,
        "SiC Si, summed over Si-28/29/30 (rho 3.18)",
    );
    // and the split itself is by natural abundance
    close(
        density(3, n.si28) / si_total,
        super::SI28_ATOM_FRACTION,
        "SiC Si-28 atom fraction",
    );
    close(
        density(3, n.si29) / si_total,
        super::SI29_ATOM_FRACTION,
        "SiC Si-29 atom fraction",
    );
    close(
        density(3, n.si30) / si_total,
        super::SI30_ATOM_FRACTION,
        "SiC Si-30 atom fraction",
    );
    // The carbon moved from the free-gas slot to the SiC-bound one; the
    // density is unchanged, only which nuclide slot carries it.
    close(density(3, n.c_sic), 4.776_08e-2, "SiC C (rho 3.18)");
    assert_eq!(
        density(3, n.c_free),
        0.0,
        "SiC carbon must no longer sit in the free-gas slot"
    );

    // graphite matrix and shell, 1.3 ppm natural B
    close(density(5, n.c_graphite), 8.673_97e-2, "matrix C (rho 1.73)");
    close(density(6, n.c_graphite), 8.673_97e-2, "shell C (rho 1.73)");
    close(density(5, n.b10), 2.493_03e-8, "matrix B-10");
    close(density(1, n.b10), 1.585_16e-8, "buffer B-10 (rho 1.1)");

    // SiC carries no boron — Table 2 gives an impurity for graphite, not SiC.
    assert_eq!(density(3, n.b10), 0.0, "SiC should carry no boron");
}
