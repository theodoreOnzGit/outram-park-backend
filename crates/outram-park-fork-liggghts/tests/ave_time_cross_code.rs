// SPDX-License-Identifier: GPL-3.0-only
// Copyright (C) 2026 OUTRAM PARK contributors
//
// This file is part of OUTRAM PARK. OUTRAM PARK is free software: you can
// redistribute it and/or modify it under the terms of the GNU General Public
// License as published by the Free Software Foundation, either version 3 of
// the License, or (at your option) any later version. It is distributed
// WITHOUT ANY WARRANTY; see <https://www.gnu.org/licenses/>.

//! # `compute ke`, `compute erotate/sphere`, `fix ave/time` against upstream (gh:#1418)
//!
//! ## Methodology
//!
//! Deck `reference-data/liggghts/in.avetime` (LIGGGHTS-PUBLIC `3d5c00f2`, built
//! binary `lmp_serial`): the 354-pebble bed (`pebble_bed_init.csv`) falling
//! under gravity into cylinder + floor walls for 1000 steps, with
//! `compute ke all ke`, `compute er all erotate/sphere`,
//! `fix at all ave/time 10 5 100 c_ke c_er ave one` and
//! `fix at2 all ave/time 5 3 50 c_ke ave running`. Thermo (`%.17g`, every 100
//! steps) prints `c_ke c_er f_at[1] f_at[2] f_at2` (`avetime_thermo.txt`). This
//! test runs the same bed in [`GranularSystem`], calls
//! `diagnostics::compute_ke`/`compute_erotate_sphere` and feeds
//! `diagnostics::AveTime` every step, comparing the five columns as bits.
//!
//! ## Prediction (written before the run, 2026-11-11)
//!
//! Bed state is bit-identical (the deck has no new physics, see
//! `force_fixes_cross_code.rs`), the summation order of the computes is copied
//! from upstream, and the averages are plain adds/divides in upstream's order,
//! so **all columns bit-identical**. Weakness, stated: the bed is still in free
//! fall for 1000 steps (no tangential contacts), so `erotate/sphere` is exactly
//! `0` and does not exercise the rotational arithmetic beyond the zero case.
//!
//! ## Result
//!
//! Printed by `--nocapture`; see the assertion message.

use outram_park_fork_liggghts::boundary::Boundary;
use outram_park_fork_liggghts::diagnostics::{compute_erotate_sphere, compute_ke, AveMode, AveTime};
use outram_park_fork_liggghts::granular::{GranularContactModel, GranularMaterial};
use outram_park_fork_liggghts::granular_system::GranularSystem;
use outram_park_fork_liggghts::neighbor_list::LiggghtsNeighbor;
use outram_park_fork_liggghts::particle::{Particle, Vec3};
use uom::si::f64::{Length, Mass, ThermodynamicTemperature};
use uom::si::{length::meter, mass::kilogram, thermodynamic_temperature::kelvin};

const REF: &str = include_str!("../../../reference-data/liggghts/avetime_thermo.txt");
const REF_CONTACT: &str = include_str!("../../../reference-data/liggghts/avetime_contact_thermo.txt");

fn particles(csv: &str) -> Vec<Particle> {
    let r = 0.005;
    let mass = 2500.0 * 4.0 / 3.0 * std::f64::consts::PI * r * r * r;
    csv.lines()
        .skip(1)
        .filter(|l| !l.trim().is_empty())
        .map(|l| {
            let v: Vec<f64> = l.split(',').map(|x| x.parse().unwrap()).collect();
            Particle::new(
                Vec3::new(v[1], v[2], v[3]),
                Vec3::new(v[4], v[5], v[6]),
                Vec3::zero(),
                Mass::new::<kilogram>(mass),
                Length::new::<meter>(r),
                ThermodynamicTemperature::new::<kelvin>(300.0),
            )
            .unwrap()
        })
        .collect()
}

/// Runs one deck in the port and compares the five thermo columns as bits;
/// returns `(bit-identical, total, count of rows with erotate/sphere != 0)`.
fn run_case(csv: &str, gravity: Vec3, reference: &str) -> (usize, usize, usize) {
    let Ok(init) = std::fs::read_to_string(
        std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../reference-data/liggghts").join(csv),
    ) else {
        eprintln!("skipping: reference data not present");
        return (0, 0, 0);
    };
    let material = GranularMaterial::new(5.0e6, 0.3, 0.5, 0.3).unwrap();
    let mut sys = GranularSystem::new(
        particles(&init),
        vec![
            Boundary::cylinder(Vec3::zero(), Vec3::new(0.0, 0.0, 1.0), 0.030).unwrap(),
            Boundary::wall(Vec3::zero(), Vec3::new(0.0, 0.0, 1.0)).unwrap(),
        ],
        GranularContactModel::hertz_history(material),
        gravity,
        5e-6,
    )
    .unwrap()
    .with_liggghts_neighbor_order(
        LiggghtsNeighbor::new(Vec3::new(-1.0, -1.0, -0.005), Vec3::new(1.0, 1.0, 0.35), 0.003).unwrap(),
    );
    let mut at = AveTime::new(10, 5, 100, 2, AveMode::One, 0).unwrap();
    let mut at2 = AveTime::new(5, 3, 50, 1, AveMode::Running, 0).unwrap();
    let (mut total, mut same) = (0usize, 0usize);
    let mut rotating = 0usize;
    let mut check = |ours: f64, theirs: &str| {
        total += 1;
        let t: f64 = theirs.parse().unwrap();
        if ours.to_bits() == t.to_bits() {
            same += 1;
        } else {
            eprintln!("differs: ours {ours:e} upstream {t:e}");
        }
    };
    let mut done = 0i64;
    for row in reference.lines() {
        let c: Vec<&str> = row.split_whitespace().collect();
        let step: i64 = c[0].parse().unwrap();
        while done < step {
            sys.run(1);
            done += 1;
            let local = sys.particles_in_local_order();
            let (ke, er) = (compute_ke(&local), compute_erotate_sphere(&local));
            at.end_of_step(done, &[ke, er]);
            at2.end_of_step(done, &[ke]);
        }
        let local = sys.particles_in_local_order();
        check(compute_ke(&local), c[1]);
        let er = compute_erotate_sphere(&local);
        if er != 0.0 {
            rotating += 1;
        }
        check(er, c[2]);
        check(at.value(0), c[3]);
        check(at.value(1), c[4]);
        check(at2.scalar(), c[5]);
    }
    (same, total, rotating)
}

/// Free-fall deck `in.avetime`: five columns x 11 thermo rows, **all
/// bit-identical** is the pass criterion.
#[test]
fn ave_time_ke_erotate_match_liggghts() {
    let (same, total, _) = run_case("pebble_bed_init.csv", Vec3::new(0.0, 0.0, -9.81), REF);
    eprintln!("ave/time free-fall deck: {same}/{total} values bit-identical");
    assert_eq!(same, total);
}

/// `fix gravity 9.81 vector 0.6 0 -0.8` exactly as upstream forms it
/// (`fix_gravity.cpp` l.379-397: normalise the direction, then scale).
fn tilted_gravity() -> Vec3 {
    let (xd, zd): (f64, f64) = (0.6, -0.8);
    let length = (xd * xd + 0.0 * 0.0 + zd * zd).sqrt();
    Vec3::new(9.81 * (xd / length), 9.81 * (0.0 / length), 9.81 * (zd / length))
}

/// Contact deck `in.avetime_contact`: the settled bed under tilted gravity
/// `(0.6, 0, -0.8) * 9.81`, 2000 steps, so the pebbles slide and roll on each
/// other and on the walls and `erotate/sphere` is non-zero (exercises the
/// rotational `0.5 * I * w^2` sum and its averaging). Pass: all 21 x 5 values
/// bit-identical, and `erotate/sphere` non-zero on every row after step 0.
#[test]
fn ave_time_nonzero_erotate_with_contacts_matches_liggghts() {
    let (same, total, rotating) = run_case("pebble_bed_settled.csv", tilted_gravity(), REF_CONTACT);
    eprintln!("ave/time contact deck: {same}/{total} values bit-identical, {rotating} rows with erotate != 0");
    assert!(rotating >= 20, "deck must exercise non-zero rotation");
    assert_eq!(same, total);
}
