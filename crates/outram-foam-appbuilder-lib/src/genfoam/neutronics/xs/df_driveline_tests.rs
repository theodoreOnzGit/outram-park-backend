// SPDX-License-Identifier: GPL-3.0-only
// Copyright (C) 2026 OUTRAM PARK contributors
//! Closed-form unit tests of the rod driveline and the discontinuity-factor integral (gh:#1785, gh:#1474); the
//! comparison with the real `XS` class is `tests/genfoam_df_driveline_vs_upstream.rs`.

use super::disc_factors::domain_integrate;
use super::driveline::{ControlRod, ControlRodDriveline};

fn rod() -> ControlRod {
    ControlRod {
        name: "rod".to_string(),
        start_time: 10.0,
        end_time: 30.0,
        speed: 0.5,
        follower_name: "follower".to_string(),
        initial_distance_from_mesh_cr: 0.25,
    }
}

#[test]
fn rod_position_follows_start_end_and_speed() {
    // follower cells 1 and 2 have centres 1.0 and 3.0: maxFollowerHeight = 3.0
    let z = [0.0, 1.0, 3.0, 5.0];
    let d = ControlRodDriveline::new(vec![rod()], &[vec![1, 2]], &z).unwrap();
    let initial = 3.0 - 0.25;
    assert_eq!(d.positions(0.0)[0], initial);
    assert_eq!(d.positions(10.0)[0], initial);
    assert_eq!(d.positions(20.0)[0], initial - 0.5 * 10.0);
    // at and after the end time the position stays at `speed (end - start)`
    assert_eq!(d.positions(30.0)[0], initial - 0.5 * 20.0);
    assert_eq!(d.positions(1000.0)[0], initial - 0.5 * 20.0);
}

#[test]
fn an_empty_follower_keeps_upstreams_minus_100() {
    let d = ControlRodDriveline::new(vec![rod()], &[vec![]], &[0.0]).unwrap();
    assert_eq!(d.positions(0.0)[0], -100.0 - 0.25);
}

#[test]
fn zones_swap_across_the_rod_position() {
    // zones: 0 = other, 1 = rod, 2 = follower; cells 0..5 at z = 0..5
    let names: Vec<String> = ["other", "rod", "follower"].iter().map(|s| s.to_string()).collect();
    let mesh_zone = vec![0, 2, 2, 2, 1, 1];
    let z = [0.0, 1.0, 2.0, 3.0, 4.0, 5.0];
    let mut r = rod();
    r.initial_distance_from_mesh_cr = 0.0;
    let d = ControlRodDriveline::new(vec![r], &[vec![1, 2, 3]], &z).unwrap();
    // before the start: position 3.0 = top follower cell; nothing above it in the follower, rod cells (z >= 4) stay
    assert_eq!(d.effective_zone_of_cell(&names, &mesh_zone, &z, 0.0), mesh_zone);
    // t = 14: position 3.0 - 0.5 * 4 = 1.0: follower cells above it (z = 2, 3) become rod
    assert_eq!(d.effective_zone_of_cell(&names, &mesh_zone, &z, 14.0), vec![0, 2, 1, 1, 1, 1]);
    // an upward rod (negative speed) lifts above the zone boundary: rod cells below the position become follower
    let mut up = rod();
    up.initial_distance_from_mesh_cr = -2.5;
    up.speed = 0.0;
    let d2 = ControlRodDriveline::new(vec![up], &[vec![1, 2, 3]], &z).unwrap();
    // position 3.0 + 2.5 = 5.5: rod cells at 4 and 5 are below it
    assert_eq!(d2.effective_zone_of_cell(&names, &mesh_zone, &z, 0.0), vec![0, 2, 2, 2, 2, 2]);
}

#[test]
fn a_follower_cell_outside_the_mesh_is_an_error() {
    assert!(ControlRodDriveline::new(vec![rod()], &[vec![9]], &[0.0, 1.0]).is_err());
    assert!(ControlRodDriveline::new(vec![rod()], &[], &[0.0, 1.0]).is_err());
}

#[test]
fn domain_integrate_is_the_sequential_volume_weighted_sum() {
    let ind = [1.0, 0.0, 1.0];
    let flux = [2.0, 5.0, 3.0];
    let vol = [0.5, 1.0, 2.0];
    assert_eq!(domain_integrate(&ind, &flux, &vol), 0.5 * 2.0 + 0.0 + 2.0 * 3.0);
}
