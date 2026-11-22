// SPDX-License-Identifier: AGPL-3.0-only
//! **HTR-10's vessel built as CSG pours HTR-10's bed, bit for bit**
//! (gh:#1656 done-when).
//!
//! Methodology. HTR-10's DEM vessel (`outram_park_fork_liggghts::htr10_fill`:
//! discharge tube from the valve, conus, barrel) is built as a CSG cell
//! in metres from that module's own constants (`region_bed::htr10_vessel_geometry`): three slabs, the tube and
//! barrel inside z-cylinders, the conus inside a z-cone whose apex and
//! slope are computed from the same constants, joined by a union, with a
//! top plane 3 m up. `pack_in_region_in(.., Pour { n, seed, gravity: None,
//! steps }, LengthUnit::Metre)` pours into it, and `Htr10Fill` pours with
//! the same `n`, seed and steps. Pass criterion: every centre equal to the
//! bit, in order. The cone's own radius at the knots is NOT used (the
//! cylinders' radii are, `region_bed`'s continuity rule), which is what makes
//! bit-identity reachable at all; that the cone's computed radius agrees to
//! 1e-9 relative is checked on the way.
//!
//! Results (2026-11-19, 600 pebbles, seed 0x5EED0010, 1500 steps): equal,
//! 600 of 600 centres.

use outram_mc_libs::pebble_beds::region_bed::{htr10_vessel_geometry, pack_in_region_in, LengthUnit, RegionPacking};
use outram_park_fork_liggghts::htr10_fill::{Htr10Fill, Htr10FillSettings, CONE_HEIGHT_M, CORE_RADIUS_M, PEBBLE_RADIUS_M, TUBE_RADIUS_M};

#[test]
fn htr10_vessel_as_csg_pours_htr10s_bed_bit_for_bit() {
    let (n, steps) = (600, 1500);
    let seed = Htr10FillSettings::default().seed;
    // The cone's computed radius at the knots agrees with the cylinders'.
    let k = (CORE_RADIUS_M - TUBE_RADIUS_M) / CONE_HEIGHT_M;
    let apex = -CONE_HEIGHT_M - TUBE_RADIUS_M / k;
    assert!((k * (0.0 - apex) / CORE_RADIUS_M - 1.0).abs() < 1e-9);
    assert!((k * (-CONE_HEIGHT_M - apex) / TUBE_RADIUS_M - 1.0).abs() < 1e-9);

    let bed = pack_in_region_in(&htr10_vessel_geometry(LengthUnit::Metre), 0, PEBBLE_RADIUS_M, RegionPacking::Pour { n, seed, gravity: None, steps }, LengthUnit::Metre).expect("region pour");
    let mut fill = Htr10Fill::new(Htr10FillSettings { n_pebbles: n, seed, ..Htr10FillSettings::default() }).expect("fill");
    fill.advance(steps);
    let reference = fill.centres();
    assert_eq!(bed.centres.len(), reference.len());
    let same = bed.centres.iter().zip(&reference).filter(|(a, b)| a.x.to_bits() == b.x.to_bits() && a.y.to_bits() == b.y.to_bits() && a.z.to_bits() == b.z.to_bits()).count();
    assert_eq!(same, n, "{same} of {n} centres bit-identical");
}
