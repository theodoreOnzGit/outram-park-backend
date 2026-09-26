// SPDX-License-Identifier: GPL-3.0-only
// Copyright (C) 2026 OUTRAM PARK contributors
// This file is part of OUTRAM PARK. See the module header for licence terms.

//! # V&V — HTR-10 model construction against the RMC paper
//!
//! ## Methodology
//!
//! Li, Yu & Wei (2014) state more than they need to. Every quantity that is
//! *both* stated and derivable from other stated quantities is a closure, and
//! reproducing it checks our reconstruction against theirs without any transport
//! solve. A closure only counts if the derivation does not consume the stated
//! value — each one below records what it was derived from.
//!
//! The strongest is the ball count: the hex cell is fitted to the layer height
//! and the filling fraction, and then predicts the core inventory, which was not
//! used in the fit.
//!
//! ## Results (measured 2026-09-16)
//!
//! | quantity | stated | derived | relative |
//! |---|---|---|---|
//! | layer height | 9.798 cm | 9.79796 cm | -4e-7 |
//! | ball filling fraction | 0.61 | 0.610000 | ~0 (fitted) |
//! | **fuel elements in the core** | **27 000** | **27 038** | **+0.14 %** |
//! | heavy metal per fuel ball | 5 g | 4.99991 g | -1.8e-5 |
//! | TRISO packing fraction | 0.050248 | 0.050248 | ~0 |
//!
//! Two of the five are fits by construction (filling fraction, TRISO packing)
//! and are here to catch drift, not as evidence. The other three are evidence.
//!
//! ## What is NOT verified here
//!
//! The paper's `k_eff`-vs-height curve ([`RMC_KEFF_VS_HEIGHT`]). It needs the
//! reflector, which the paper defers to IAEA-TECDOC-1382. See the module docs.

use super::*;

/// The reconstruction must be a **packing**, not an overlapping lattice — a
/// packing fraction computed from overlapping spheres is a fiction.
#[test]
fn the_reconstructed_cell_is_a_real_packing() {
    let cell = bed::HexBedCell::from_paper();
    assert!(
        cell.is_non_overlapping(),
        "balls overlap: in-plane {:.4} cm, interlayer {:.4} cm, diameter {:.4} cm",
        cell.in_plane_spacing(),
        cell.interlayer_spacing(),
        cell.ball_diameter
    );
    // And it must be DILUTED, not close-packed — the paper's bed is 61 %, well
    // below the 74 % an ordered close packing would give.
    assert!(
        cell.packing_fraction() < bed::close_packed_fraction(),
        "a 61 % bed cannot be close-packed"
    );
}

/// **The layer height decodes as two close-packed layers.** This is what pins
/// the axial structure; if it stops holding, the reconstruction has lost its
/// footing.
#[test]
fn the_layer_height_is_two_close_packed_layers() {
    let ratio = table1::LAYER_HEIGHT_CM / bed::close_packed_layer_spacing(table1::BALL_DIAMETER_CM);
    assert!(
        (ratio - 2.0).abs() < 1.0e-4,
        "the paper's 9.798 cm layer is {ratio:.5} close-packed layer spacings, \
         expected exactly 2"
    );
}

/// **The closure that is evidence.** The cell is fitted to the layer height and
/// the filling fraction; the core ball count is not used in the fit, so
/// reproducing it is an independent check.
///
/// 1 % is deliberately loose: the paper's 27 000 is a round design figure rather
/// than a count, and the cell estimate is a continuum one that ignores partial
/// boundary cells.
#[test]
fn the_cell_predicts_the_stated_core_inventory_without_being_told_it() {
    let cell = bed::HexBedCell::from_paper();
    let derived = cell.balls_in_core(table1::CORE_DIAMETER_CM, table1::CORE_HEIGHT_CM);
    let rel = (derived - table1::FUEL_ELEMENTS) / table1::FUEL_ELEMENTS;
    assert!(
        rel.abs() < 0.01,
        "the reconstructed cell tiles to {derived:.0} balls in a {} x {} cm core, \
         against the paper's stated {} ({:+.2} %)",
        table1::CORE_DIAMETER_CM,
        table1::CORE_HEIGHT_CM,
        table1::FUEL_ELEMENTS,
        rel * 100.0
    );
}

/// Every closure the paper supports, in one gate, with the tolerance each
/// deserves.
#[test]
fn every_stated_quantity_closes_against_its_derivation() {
    for c in geometry_closures() {
        // 1 % for the ball count (a round design figure), 1e-4 for the rest.
        let tol = if c.quantity == "fuel elements in the core" {
            0.01
        } else {
            1.0e-4
        };
        assert!(
            c.relative().abs() < tol,
            "{}: paper states {} {}, reconstruction derives {} {} ({:+.4} %) — \
             derived from {}",
            c.quantity,
            c.stated,
            c.units,
            c.derived,
            c.units,
            c.relative() * 100.0,
            c.derived_from
        );
    }
}

/// The 0.57/0.43 split must partition the inventory, not merely be stored.
#[test]
fn the_fuel_moderator_split_partitions_the_core() {
    let cell = bed::HexBedCell::from_paper();
    let (fuel, moderator) =
        cell.fuel_and_moderator_balls(table1::CORE_DIAMETER_CM, table1::CORE_HEIGHT_CM);
    let total = cell.balls_in_core(table1::CORE_DIAMETER_CM, table1::CORE_HEIGHT_CM);
    assert!((fuel + moderator - total).abs() / total < 1.0e-12);
    assert!((fuel / total - table1::FUEL_BALL_FRACTION).abs() < 1.0e-12);
    assert!(
        (table1::FUEL_BALL_FRACTION + table1::MODERATOR_BALL_FRACTION - 1.0).abs() < 1.0e-12,
        "Table 1's 0.57/0.43 must sum to one"
    );
}

/// **The reference curve is one curve.** Guard the finding that the paper's two
/// tables share an identical RMC column: if someone later adds a second curve
/// here, this says why they should not.
#[test]
fn the_reference_curve_is_monotonic_and_brackets_criticality() {
    let c = RMC_KEFF_VS_HEIGHT;
    assert_eq!(c.len(), 12, "the paper tabulates twelve loading heights");
    for w in c.windows(2) {
        assert!(
            w[1].0 > w[0].0 && w[1].1 > w[0].1,
            "k_eff must rise with loading height: {:?} then {:?}",
            w[0],
            w[1]
        );
        let step = w[1].0 - w[0].0;
        assert!(
            (step - table1::LAYER_HEIGHT_CM).abs() < 1.0e-3,
            "loading steps must be one layer ({} cm), got {step:.4}",
            table1::LAYER_HEIGHT_CM
        );
    }
    assert!(
        c.first().unwrap().1 < 1.0 && c.last().unwrap().1 > 1.0,
        "the curve must bracket criticality for an interpolated critical height \
         to mean anything"
    );
}

/// The critical height the paper's own curve implies, by linear interpolation
/// across `k = 1`. Recorded so a future full-core run has something to aim at.
///
/// **Measured: 122.269 cm.** The paper never states this; it is read off its
/// Tables 3/4. HTR-10's experimental first-criticality height is commonly quoted
/// near 123 cm, but that value has NOT been checked against a source here and is
/// deliberately not asserted.
#[test]
fn the_reference_curve_implies_a_critical_height() {
    let c = RMC_KEFF_VS_HEIGHT;
    let i = c
        .windows(2)
        .position(|w| w[0].1 < 1.0 && w[1].1 >= 1.0)
        .expect("curve crosses k = 1");
    let (h0, k0) = c[i];
    let (h1, k1) = c[i + 1];
    let h = h0 + (1.0 - k0) * (h1 - h0) / (k1 - k0);
    assert!(
        (h - 122.269).abs() < 0.005,
        "interpolated critical height {h:.3} cm, expected 122.269 cm from the \
         paper's own curve"
    );
    assert!(h > h0 && h < h1, "the crossing must lie inside its bracket");
}

/// **The axial stack is the reactor's at EVERY loading, not just one.**
///
/// Terry (2005) Fig. 2, z measured down from the model top: top reflector
/// 0 -> 130, core cavity 130 -> 351.818 (bed + void, fixed hardware), conus
/// 351.818 -> 388.764, bottom reflector 388.764 -> 610. Only the split of the
/// cavity between bed and void may change with `n_axial`.
///
/// Two construction defects of exactly this class have shipped: a constant
/// void that grew the cavity with the bed, and a bottom boundary MIRRORED from
/// the top, which rode up with the bed and left the bottom reflector up to
/// 107 cm short at the tallest loading. Both were exact at one loading, so a
/// single-height check cannot catch them; this runs the lowest, benchmark and
/// tallest loadings, and reads the planes the TRANSPORT sees, not just the
/// reported fields.
#[test]
fn the_axial_stack_matches_terry_at_every_loading() {
    use crate::htr10_rmc::core_model::{assemble, assemble_explicit_triso};
    use outram_mc_libs::geometry::surface::SurfaceKind;
    let z_of = |s: &SurfaceKind| match s {
        SurfaceKind::ZPlane(p) => p.z0,
        other => panic!("expected a z-plane, got {other:?}"),
    };
    let close = |a: f64, b: f64, what: &str, n: usize| {
        assert!(
            (a - b).abs() < 1e-9,
            "n_axial {n}: {what} = {a:.6}, Terry says {b:.6}"
        );
    };
    for n in [20usize, 25, 41] {
        let c = assemble_explicit_triso(14, n, 0);
        let bed_bottom = -c.bed_half_height;
        close(c.refl_top - c.cavity_top, 130.0, "top reflector", n);
        close(
            c.cavity_top - bed_bottom,
            221.818,
            "core cavity (bed + void)",
            n,
        );
        close(bed_bottom - c.conus_floor, 36.946, "conus", n);
        close(
            c.conus_floor - c.refl_bottom,
            221.236,
            "bottom reflector",
            n,
        );
        close(c.refl_top - c.refl_bottom, 610.0, "whole model", n);
        // The vacuum planes the transport actually tracks: surfaces 11/12.
        close(
            z_of(&c.geometry.surfaces[11]),
            c.refl_bottom,
            "bottom vacuum plane",
            n,
        );
        close(
            z_of(&c.geometry.surfaces[12]),
            c.refl_top,
            "top vacuum plane",
            n,
        );

        // The homogenised diagnostic model has the same outer extent
        // (surfaces 6/7), with reflector graphite where the conus would be.
        let h = assemble(14, n, 0);
        close(
            h.refl_top - h.refl_bottom,
            610.0,
            "homogenised: whole model",
            n,
        );
        close(
            z_of(&h.geometry.surfaces[6]),
            h.refl_bottom,
            "homogenised: bottom plane",
            n,
        );
        close(
            z_of(&h.geometry.surfaces[7]),
            h.refl_top,
            "homogenised: top plane",
            n,
        );
    }
}

/// **Every material carries natural boron, not just its absorber** (gh:#311).
///
/// Until 2026-09-25 only B-10 was placed, so the boronated brick (zone 17) was
/// ~3.5 % short on scattering atoms. Natural boron is 19.9 / 80.1 at.% B-10 /
/// B-11, so wherever boron appears the atom ratio must be 0.801 / 0.199. The
/// pebble materials are built on a WEIGHT basis and the reflector zones on an
/// ATOM basis, so this also checks the two bases agree.
#[test]
fn every_boron_bearing_material_carries_natural_b11() {
    use crate::htr10_rmc::materials::{htr10_material_set, Htr10MaterialConfig};
    use outram_mc_libs::pebble_beds::htr10::Htr10Nuclides;
    let n = Htr10Nuclides {
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
        b11: 10,
    };
    let want = 0.801 / 0.199;
    let mats = htr10_material_set(n, Htr10MaterialConfig::benchmark_default(300.15));
    let mut with_boron = 0;
    for m in &mats {
        let sum = |i: usize| -> f64 {
            m.components
                .iter()
                .filter(|c| c.nuclide_idx == i)
                .map(|c| c.atom_density)
                .sum()
        };
        let (b10, b11) = (sum(n.b10), sum(n.b11));
        if b10 == 0.0 {
            assert_eq!(b11, 0.0, "{}: B-11 without B-10", m.name);
            continue;
        }
        with_boron += 1;
        let r = b11 / b10;
        assert!(
            (r / want - 1.0).abs() < 1e-3,
            "{}: B-11/B-10 = {r:.5}, natural boron gives {want:.5}",
            m.name
        );
    }
    // Kernel, four graphite layers, three reflector zones, homogenised dummies.
    assert!(with_boron >= 9, "only {with_boron} materials carry boron");
}

/// **The TRISO lattice holds the particles it reports** (gh:#316).
///
/// The particle count was taken on a grid offset `[0.5, 0.5, 0.0]` (8340) while
/// the lattice was built with half-integer centres on all three axes, which
/// holds 8240: every fuel pebble carried 1.2 % less heavy metal than stated.
/// The builder now asserts built == counted; this pins the count itself and
/// the non-cubic grid that realises it.
#[test]
fn the_built_triso_lattice_holds_the_counted_8340_particles() {
    use crate::htr10_rmc::core_model::assemble_explicit_triso;
    use outram_mc_libs::geometry::lattice::Lattice;
    let c = assemble_explicit_triso(14, 20, 0);
    let rect = c
        .geometry
        .lattices
        .iter()
        .find_map(|l| match l {
            Lattice::Rect(r) => Some(r),
            _ => None,
        })
        .expect("the explicit-TRISO core has a rect lattice");
    let built = rect.universes.iter().filter(|&&u| u == 3).count();
    assert_eq!(built, 8340, "TRISO particles per fuel pebble");
    assert_eq!(rect.n, [26, 26, 27], "offset [0.5, 0.5, 0.0] grid");
    // The grid must cover the 2.5 cm fuel zone on every axis.
    for a in 0..3 {
        let lo = [rect.lower_left.x, rect.lower_left.y, rect.lower_left.z][a];
        let hi = lo + rect.n[a] as f64 * rect.pitch[a];
        assert!(
            lo <= -2.5 && hi >= 2.5,
            "axis {a}: [{lo}, {hi}] does not cover the zone"
        );
    }
}
