// SPDX-License-Identifier: GPL-3.0-only
// Copyright (C) 2026 OUTRAM PARK contributors
//
// This file is part of OUTRAM PARK.
//
// OUTRAM PARK is free software: you can redistribute it and/or modify it
// under the terms of the GNU General Public License as published by the
// Free Software Foundation, either version 3 of the License, or (at your
// option) any later version.
//
// OUTRAM PARK is distributed in the hope that it will be useful, but
// WITHOUT ANY WARRANTY; without even the implied warranty of
// MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE.  See the GNU
// General Public License for more details.
//
// You should have received a copy of the GNU General Public License along
// with OUTRAM PARK.  If not, see <https://www.gnu.org/licenses/>.

//! # `fix addforce`, `fix viscous`, `fix freeze` against upstream (gh:#1418)
//!
//! ## Methodology
//!
//! Deck `reference-data/liggghts/in.forcefix` (LIGGGHTS-PUBLIC `3d5c00f2`): the
//! 354-pebble `D/d = 6` bed as inserted (`pebble_bed_init.csv`), cylinder and
//! floor walls, gravity, then in declaration order
//! `fix af pw addforce 0.0005 -0.0003 v_fz` (group `id 31:200`,
//! `v_fz = 0.002*sin(step*dt*3000) - 0.001`), `fix vis all viscous 0.002` and
//! `fix frz fr freeze` (group `id <= 30`). 2000 steps, `dump` every 100 steps
//! at `%.17g` (`forcefix.csv`, 21 frames x 354 atoms x 9 fields). This test
//! runs [`force_fixes::ForceFix`] on the same bed with the
//! `with_liggghts_neighbor_order` pair order and compares every value as bits.
//! Pass criterion: all bit-identical.
//!
//! ## Prediction (written before the run, 2026-11-11)
//!
//! Every operation is a plain double add/multiply in the same order as upstream
//! (gravity, addforce, viscous, freeze), so **all values bit-identical**. The
//! one exception that could occur is the `sin` of `v_fz` (PETIR libm vs glibc),
//! at most a last-ulp difference in `fz` (accepted per gh:#1455 if it is the
//! only cause).
//!
//! ## Result
//!
//! See the printed count (`--nocapture`) and the figure recorded in
//! `verification_and_validation/force_fixes.md`.

use outram_park_fork_liggghts::boundary::Boundary;
use outram_park_fork_liggghts::force_fixes::{ForceComponent, ForceFix};
use outram_park_fork_liggghts::granular::{GranularContactModel, GranularMaterial};
use outram_park_fork_liggghts::granular_system::GranularSystem;
use outram_park_fork_liggghts::neighbor_list::LiggghtsNeighbor;
use outram_park_fork_liggghts::particle::{Particle, Vec3};
use outram_park_fork_liggghts::variable::VariableTable;
use uom::si::f64::{Length, Mass, ThermodynamicTemperature};
use uom::si::{length::meter, mass::kilogram, thermodynamic_temperature::kelvin};

const R_P: f64 = 0.005;
const R_CYL: f64 = 0.030;

fn read(name: &str) -> Option<String> {
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../reference-data/liggghts")
        .join(name);
    std::fs::read_to_string(path).ok()
}

fn particles(csv: &str) -> Vec<Particle> {
    let mass = 2500.0 * 4.0 / 3.0 * std::f64::consts::PI * R_P * R_P * R_P;
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
                Length::new::<meter>(R_P),
                ThermodynamicTemperature::new::<kelvin>(300.0),
            )
            .unwrap()
        })
        .collect()
}

/// Frames 0..=700 (8 frames x 354 pebbles x 9 fields): **bit-identical**
/// (measured 2026-11-11, Rust 1.99.1: 25 488 / 25 488, worst relative 0).
/// This is the gate for the three fixes themselves.
#[test]
fn addforce_viscous_freeze_match_liggghts() {
    run(700);
}

/// The whole deck (21 frames): **66 906 / 66 906 bit-identical** (measured
/// 2026-11-11, Rust 1.99.1). ~~Pebble 188 diverged from frame 800~~
/// **CORRECTED 2026-11-11 (gh:#1478)**: the cause was the pair effective mass.
/// Upstream `pair_gran_base.h` sets `meff = mj` when `i` is in the `fix freeze`
/// group and `meff = mi` when `j` is (a frozen particle does not reduce the
/// mass), not the reduced mass; the port used the reduced mass. Fixed by
/// `ContactKinematics::pair_frozen`.
#[test]
fn addforce_viscous_freeze_full_deck() {
    run(usize::MAX);
}

fn run(max_step: usize) {
    let (Some(init), Some(traj)) = (read("pebble_bed_init.csv"), read("forcefix.csv")) else {
        eprintln!("skipping: reference data not present");
        return;
    };
    let ps = particles(&init);
    let n = ps.len();
    let mut table = VariableTable::new();
    table
        .define_equal("fz", "0.002*sin(step*dt*3000.0)-0.001")
        .unwrap();
    let mask = |lo: usize, hi: usize| {
        (0..n)
            .map(|i| (lo..=hi).contains(&(i + 1)))
            .collect::<Vec<bool>>()
    };
    let fixes = vec![
        ForceFix::AddForce {
            group: mask(31, 200),
            components: [
                ForceComponent::parse("0.0005", &table).unwrap(),
                ForceComponent::parse("-0.0003", &table).unwrap(),
                ForceComponent::parse("v_fz", &table).unwrap(),
            ],
            foriginal: [0.0; 3],
        },
        ForceFix::Viscous {
            group: vec![true; n],
            gamma: vec![0.002],
        },
        ForceFix::Freeze {
            group: mask(1, 30),
            foriginal: [0.0; 3],
        },
    ];
    let material = GranularMaterial::new(5.0e6, 0.3, 0.5, 0.3).unwrap();
    let mut sys = GranularSystem::new(
        ps,
        vec![
            Boundary::cylinder(Vec3::zero(), Vec3::new(0.0, 0.0, 1.0), R_CYL).unwrap(),
            Boundary::wall(Vec3::zero(), Vec3::new(0.0, 0.0, 1.0)).unwrap(),
        ],
        GranularContactModel::hertz_history(material),
        Vec3::new(0.0, 0.0, -9.81),
        5e-6,
    )
    .unwrap()
    .with_liggghts_neighbor_order(
        LiggghtsNeighbor::new(
            Vec3::new(-1.0, -1.0, -0.005),
            Vec3::new(1.0, 1.0, 0.35),
            0.003,
        )
        .unwrap(),
    )
    .with_force_fixes(fixes);

    let mut done = 0usize;
    let (mut total, mut same) = (0usize, 0usize);
    let mut worst = 0.0f64;
    let mut first_bad: Option<usize> = None;
    for row in traj.lines().skip(1).filter(|l| !l.trim().is_empty()) {
        let v: Vec<f64> = row.split(',').map(|x| x.parse().unwrap()).collect();
        let step = v[0] as usize;
        if step > max_step {
            break;
        }
        sys.run(step - done);
        done = step;
        for (i, p) in sys.particles().iter().enumerate() {
            let o = 1 + 9 * i;
            let ours = [
                p.position.x,
                p.position.y,
                p.position.z,
                p.velocity.x,
                p.velocity.y,
                p.velocity.z,
                p.angular_velocity.x,
                p.angular_velocity.y,
                p.angular_velocity.z,
            ];
            for k in 0..9 {
                total += 1;
                if ours[k].to_bits() == v[o + k].to_bits() {
                    same += 1;
                } else {
                    first_bad.get_or_insert(step);
                    let scale = v[o + k].abs().max(1e-300);
                    worst = worst.max(((ours[k] - v[o + k]) / scale).abs());
                }
            }
        }
    }
    eprintln!(
        "forcefix: {same}/{total} values bit-identical, first differing frame {first_bad:?}, worst rel {worst:e}"
    );
    assert_eq!(same, total, "first differing frame {first_bad:?}");
}
