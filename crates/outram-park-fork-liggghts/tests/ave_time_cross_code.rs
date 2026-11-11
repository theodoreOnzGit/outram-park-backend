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

/// Five columns x 11 thermo rows: **all bit-identical** is the pass criterion;
/// the measured count is in the assertion message.
#[test]
fn ave_time_ke_erotate_match_liggghts() {
    let Ok(init) = std::fs::read_to_string(
        std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../reference-data/liggghts/pebble_bed_init.csv"),
    ) else {
        eprintln!("skipping: reference data not present");
        return;
    };
    let material = GranularMaterial::new(5.0e6, 0.3, 0.5, 0.3).unwrap();
    let mut sys = GranularSystem::new(
        particles(&init),
        vec![
            Boundary::cylinder(Vec3::zero(), Vec3::new(0.0, 0.0, 1.0), 0.030).unwrap(),
            Boundary::wall(Vec3::zero(), Vec3::new(0.0, 0.0, 1.0)).unwrap(),
        ],
        GranularContactModel::hertz_history(material),
        Vec3::new(0.0, 0.0, -9.81),
        5e-6,
    )
    .unwrap()
    .with_liggghts_neighbor_order(
        LiggghtsNeighbor::new(Vec3::new(-1.0, -1.0, -0.005), Vec3::new(1.0, 1.0, 0.35), 0.003).unwrap(),
    );
    let mut at = AveTime::new(10, 5, 100, 2, AveMode::One, 0).unwrap();
    let mut at2 = AveTime::new(5, 3, 50, 1, AveMode::Running, 0).unwrap();
    let (mut total, mut same) = (0usize, 0usize);
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
    for row in REF.lines() {
        let c: Vec<&str> = row.split_whitespace().collect();
        let step: i64 = c[0].parse().unwrap();
        while done < step {
            sys.run(1);
            done += 1;
            let (ke, er) = (compute_ke(sys.particles()), compute_erotate_sphere(sys.particles()));
            at.end_of_step(done, &[ke, er]);
            at2.end_of_step(done, &[ke]);
        }
        check(compute_ke(sys.particles()), c[1]);
        check(compute_erotate_sphere(sys.particles()), c[2]);
        check(at.value(0), c[3]);
        check(at.value(1), c[4]);
        check(at2.scalar(), c[5]);
    }
    eprintln!("ave/time deck: {same}/{total} values bit-identical");
    assert_eq!(same, total);
}
