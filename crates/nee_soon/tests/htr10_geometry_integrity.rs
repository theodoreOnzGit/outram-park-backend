//! **HTR-10 geometry integrity** — `bn:op-867c.15`, gh #214.
//!
//! Cheap, falsifiable checks that run in the DEFAULT suite, so the model stays
//! honest between the expensive transport runs. None of these needs a converged
//! eigenvalue; all of them would catch a silently wrong geometry.
//!
//! # Why a consolidating test and not just the component tests
//!
//! Each piece is already gated where it lives — the bed arithmetic in
//! `htr10_rmc::bed`, the reflector transcription in `htr10_rmc::reflector`, the
//! TRISO array in `outram-mc-libs`. What none of them checks is whether the
//! pieces **agree with each other and with the published plant**. A bed that is
//! internally consistent and a TRISO array that is internally consistent can
//! still describe two different reactors.
//!
//! # Results (2026-09-17)
//!
//! Printed by each test.

use nee_soon::htr10_rmc::bed::{bed_tile_levels, count_tiles, HexBedCell};
use nee_soon::htr10_rmc::reflector::{listed_zones, zone_composition, MAX_ZONE};
use nee_soon::htr10_rmc::table1;
use outram_mc_libs::pebble_beds::sphere_packing::{cubic_array_in_ball, cubic_pitch_for_count};

/// The bed cell must reproduce the plant's published ball inventory.
///
/// This is the strongest single check available without transport: 27,000 fuel
/// elements is a **stated plant figure**, and the cell geometry is derived from
/// different stated figures (6 cm balls, 0.61 filling fraction, a 180 x 197 cm
/// core). If the derivation were wrong the two would not meet.
#[test]
fn the_bed_reproduces_the_published_ball_inventory() {
    let cell = HexBedCell::from_paper();
    let total = cell.balls_in_core(table1::CORE_DIAMETER_CM, table1::CORE_HEIGHT_CM);
    let (fuel, moderator) = cell.fuel_and_moderator_balls(
        table1::CORE_DIAMETER_CM,
        table1::CORE_HEIGHT_CM,
    );
    // The published 27,000 is the TOTAL ball inventory, not the fuel subset.
    // (An earlier version of this test compared it against the 57 % fuel share
    // and read -42.9 %, which is just 0.57 - 1. The constant's name invites
    // that: `FUEL_ELEMENTS` is what TECDOC calls them, but the 57:43 split is
    // between FUELLED and DUMMY balls within that inventory.)
    let err = 100.0 * (total / table1::FUEL_ELEMENTS - 1.0);
    println!(
        "bed: {total:.0} balls total -> {fuel:.0} fuelled + {moderator:.0} dummy; \
         stated {:.0} ({err:+.3} %)",
        table1::FUEL_ELEMENTS
    );
    assert!(
        err.abs() < 1.0,
        "derived TOTAL ball count is {err:+.3} % from the published {:.0}; the \
         cell geometry and the plant figures disagree",
        table1::FUEL_ELEMENTS
    );
    assert!(
        (fuel / total - table1::FUEL_BALL_FRACTION).abs() < 1.0e-6,
        "and the fuelled share of that inventory must be the stated {}",
        table1::FUEL_BALL_FRACTION
    );
    assert!(
        cell.is_non_overlapping(),
        "the derived pitch puts balls into each other"
    );
    let pf = cell.packing_fraction();
    assert!(
        (pf - 0.61).abs() < 5.0e-3,
        "cell packing fraction {pf:.5} must be the stated 0.61"
    );
}

/// The lattice tile assignment must agree with the CONTINUUM ball split, i.e.
/// the discrete geometry and the arithmetic must describe the same core.
#[test]
fn the_tile_assignment_agrees_with_the_continuum_split() {
    let cell = HexBedCell::from_paper();
    let (fuel_c, mod_c) =
        cell.fuel_and_moderator_balls(table1::CORE_DIAMETER_CM, table1::CORE_HEIGHT_CM);
    let continuum = fuel_c / (fuel_c + mod_c);

    let levels = bed_tile_levels(12, 21, 1, 2);
    let (fuel_d, mod_d) = count_tiles(&levels, 1);
    let discrete = fuel_d as f64 / (fuel_d + mod_d) as f64;

    println!("continuum fuel fraction {continuum:.6}, discrete {discrete:.6}");
    assert!(
        (continuum - discrete).abs() < 1.0e-3,
        "the tile assignment ({discrete:.6}) and the continuum split \
         ({continuum:.6}) must describe the same core"
    );
}

/// Every reflector zone the R-Z model can reference must have a composition,
/// and the one that does not must be the cavity.
#[test]
fn the_reflector_covers_every_zone_the_model_can_reference() {
    let zones = listed_zones();
    let missing: Vec<usize> = (0..=MAX_ZONE).filter(|z| zone_composition(*z).is_none()).collect();
    println!("{} of {} zones have a composition; absent: {missing:?}", zones.len(), MAX_ZONE + 1);
    assert_eq!(missing, vec![5], "only the top core cavity may be absent");
}

/// The TRISO array must sit in the fuel zone with the adjudicated radii, and
/// its count must be within the recorded distance of the paper's 8335.
#[test]
fn the_triso_array_matches_the_adjudicated_geometry() {
    const R_ZONE: f64 = 2.5;
    const R_PART: f64 = 0.0455; // TECDOC-1382, adjudicated in op-867c.12
    let (pitch, count) = cubic_pitch_for_count(R_PART, R_ZONE, 8335, [0.5, 0.5, 0.0]);
    let arr = cubic_array_in_ball(R_PART, R_ZONE, pitch, [0.5, 0.5, 0.0]);
    assert_eq!(arr.len(), count);
    let err = 100.0 * (count as f64 / 8335.0 - 1.0);
    println!("TRISO: {count} particles at pitch {pitch:.6} cm ({err:+.3} % vs 8335)");
    assert!(
        err.abs() < 0.1,
        "the realised TRISO count is {err:+.3} % from the stated 8335; the \
         recorded discrepancy is 0.060 %, so anything larger means the radii or \
         the rejection rule changed"
    );
    for s in &arr {
        assert!(s.center.norm() + R_PART <= R_ZONE + 1.0e-12);
    }
}
