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
