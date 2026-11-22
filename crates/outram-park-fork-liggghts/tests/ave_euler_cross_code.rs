// SPDX-License-Identifier: GPL-3.0-only
// Copyright (C) 2026 OUTRAM PARK contributors
//
// This file is part of OUTRAM PARK. OUTRAM PARK is free software: you can
// redistribute it and/or modify it under the terms of the GNU General Public
// License as published by the Free Software Foundation, either version 3 of
// the License, or (at your option) any later version. It is distributed
// WITHOUT ANY WARRANTY; see <https://www.gnu.org/licenses/>.

//! # `fix ave/euler` against upstream (gh:#1418)
//!
//! ## Methodology
//!
//! Deck `reference-data/liggghts/in.aveeuler` (generator
//! `make_aveeuler_deck.sh`; LIGGGHTS-PUBLIC `3d5c00f2`): the settled 354-pebble
//! bed (`pebble_bed_settled.csv`, r = 5 mm, rho = 2500) in a +-0.035 m x
//! -0.005..0.35 m box, tilted gravity `(0.6, 0, -0.8) * 9.81`, 2000 steps,
//! `fix ae all ave/euler nevery 100 cell_size_relative 3 parallel no`. The grid
//! is 2 x 2 x 11 = 44 cells. `thermo 500` prints, with `%.17g`, columns 1-7 of
//! every cell (`f_ae[i][1..7]`: centre xyz, volume fraction, mass-averaged
//! velocity xyz) at steps 0, 500, 1000, 1500, 2000 = 5 x 44 x 7 = 1540 values.
//! This test runs the same bed in `GranularSystem` and feeds
//! `diagnostics::AveEuler` the particles in LIGGGHTS' local atom order every
//! step.
//!
//! ## Prediction (written before the run, 2026-11-11)
//!
//! Bed state is bit-identical (`ave_time_cross_code.rs` already depends on
//! this), the binning is the same expression, and per-cell sums run in
//! ascending local atom index like upstream's linked lists, so **all 1540
//! values bit-identical**. Not covered, and stated: the stress columns (need
//! `compute stress/atom pair`), the radius column (not readable through
//! `thermo`, whose bound is `size_array_cols = 7`), `basevolume_region`.
//!
//! ## Reference binary
//!
//! Stock `lmp_serial` crashes with a floating-point exception when `thermo`
//! reads `f_ae[i][j]`: `FixAveEuler` leaves `Fix::global_freq` at 0 and
//! `Thermo::init` computes `thermo_every % global_freq`. The reference was run
//! on the same objects relinked with `global_freq = nevery;` added to the
//! constructor (the averaging code is unchanged). Upstream defect, not copied.
//!
//! ## Result
//!
//! Printed by `--nocapture`.

use outram_park_fork_liggghts::boundary::Boundary;
use outram_park_fork_liggghts::diagnostics::AveEuler;
use outram_park_fork_liggghts::granular::{GranularContactModel, GranularMaterial};
use outram_park_fork_liggghts::granular_system::GranularSystem;
use outram_park_fork_liggghts::neighbor_list::LiggghtsNeighbor;
use outram_park_fork_liggghts::particle::{Particle, Vec3};
use uom::si::f64::{Length, Mass, ThermodynamicTemperature};
use uom::si::{length::meter, mass::kilogram, thermodynamic_temperature::kelvin};

const REF: &str = include_str!("../../../reference-data/liggghts/aveeuler_thermo.txt");

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

/// `fix gravity 9.81 vector 0.6 0 -0.8` as upstream forms it.
fn tilted_gravity() -> Vec3 {
    let (xd, zd): (f64, f64) = (0.6, -0.8);
    let length = (xd * xd + 0.0 * 0.0 + zd * zd).sqrt();
    Vec3::new(9.81 * (xd / length), 9.81 * (0.0 / length), 9.81 * (zd / length))
}

const LO: [f64; 3] = [-0.035, -0.035, -0.005];
const HI: [f64; 3] = [0.035, 0.035, 0.35];

/// 1540 of 1540 values bit-identical is the pass criterion (see module docs).
#[test]
fn ave_euler_matches_liggghts() {
    let Ok(init) = std::fs::read_to_string(
        std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../reference-data/liggghts/pebble_bed_settled.csv"),
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
        tilted_gravity(),
        5e-6,
    )
    .unwrap()
    .with_liggghts_neighbor_order(
        LiggghtsNeighbor::new(Vec3::new(LO[0], LO[1], LO[2]), Vec3::new(HI[0], HI[1], HI[2]), 0.003).unwrap(),
    );
    // cutneighmax - skin = (2 r + skin) - skin for the granular pair style.
    let mut ae = AveEuler::new(100, 3.0, (0.01 + 0.003) - 0.003, LO, HI).unwrap();
    assert_eq!(ae.ncells(), 44);
    let (mut done, mut total, mut same, mut nonzero_vol) = (0i64, 0usize, 0usize, 0usize);
    // setup() calls end_of_step() once at step 0.
    ae.end_of_step(0, &sys.particles_in_local_order());
    for row in REF.lines() {
        let c: Vec<&str> = row.split_whitespace().collect();
        let step: i64 = c[0].parse().unwrap();
        while done < step {
            sys.run(1);
            done += 1;
            ae.end_of_step(done, &sys.particles_in_local_order());
        }
        assert_eq!(c.len(), 1 + 44 * 7);
        for i in 0..44 {
            for j in 0..7 {
                let theirs: f64 = c[1 + i * 7 + j].parse().unwrap();
                let ours = ae.array(i, j);
                total += 1;
                if ours.to_bits() == theirs.to_bits() {
                    same += 1;
                } else {
                    eprintln!("step {step} cell {i} col {j}: ours {ours:e} upstream {theirs:e}");
                }
                if j == 3 && theirs != 0.0 {
                    nonzero_vol += 1;
                }
            }
        }
    }
    eprintln!("ave/euler: {same}/{total} values bit-identical, {nonzero_vol} non-zero volume fractions");
    assert!(nonzero_vol >= 20, "deck must populate several cells");
    assert_eq!(same, total);
}

/// Argument checks and `compute_array` bounds of the translation.
#[test]
fn ave_euler_checks_arguments() {
    assert!(AveEuler::new(0, 3.0, 0.01, LO, HI).is_err());
    assert!(AveEuler::new(100, 0.5, 0.01, LO, HI).is_err());
    let ae = AveEuler::new(100, 3.0, 0.01, LO, HI).unwrap();
    assert_eq!(ae.array(ae.ncells(), 3), 0.0);
    assert_eq!(ae.array(0, 9), 0.0);
    assert_eq!(ae.radius(0), 0.0);
}
