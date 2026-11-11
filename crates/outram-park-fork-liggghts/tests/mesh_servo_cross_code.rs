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

//! # `fix mesh/surface/stress/servo` against upstream (gh:#1416)
//!
//! `MeshModuleStressServo` (`mesh_module_stress_servo.cpp`, LIGGGHTS-PUBLIC
//! `3d5c00f2`, `ctrlPV force`, PID controller) against
//! [`outram_park_fork_liggghts::mesh_stress::MeshServo`].
//!
//! ## Methodology
//!
//! Deck `reference-data/liggghts/in.mesh_servo`: two `d = 10 mm` spheres
//! (`vx = 0.02` on one) rest on a floor wall `z = 0`; a 0.3 m square plate
//! starting at `z = 0.0112` descends under the servo (`axis 0 0 1`,
//! `target_val 0.1`, `vel_max 0.05`, `kp 1`, `ki 0.2`, `kd 1e-4`), 20 000
//! steps of `dt = 1e-5`, `E = 1e7`, `nu = 0.3`, `e = 0.9`, `mu = 0.5`, skin
//! 3 mm. Compared bit for bit: 501 trajectory frames (`mesh_servo.csv`) and,
//! every 250 steps, `f_plate[1..12]` (force, torque, `p_ref`, servo centre
//! `xcm`; `mesh_servo_thermo.txt`, `%.17g`).
//!
//! **Prediction (written before the first run, 2026-11-11).** Bit-identical:
//! the controller is a fixed sequence of IEEE operations on the total force,
//! which is a two-term sum per step (order independent), and `mesh->move`
//! is `node_orig + dX`, reproduced operation for operation.

use outram_park_fork_liggghts::granular::{GranularContactModel, GranularMaterial};
use outram_park_fork_liggghts::granular_system::{FixSlot, GranularSystem};
use outram_park_fork_liggghts::boundary::Boundary;
use outram_park_fork_liggghts::mesh_stress::MeshServo;
use outram_park_fork_liggghts::mesh_wall::{MeshWall, MovingBoundary, WallGeometry};
use outram_park_fork_liggghts::particle::{Particle, Vec3};
use uom::si::f64::{Length, Mass, ThermodynamicTemperature};
use uom::si::{length::meter, mass::kilogram, thermodynamic_temperature::kelvin};

fn read(name: &str) -> Option<String> {
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../reference-data/liggghts")
        .join(name);
    std::fs::read_to_string(path).ok()
}

fn sphere(row: &[f64], k: usize, r: f64) -> Particle {
    let c = &row[1 + 9 * k..1 + 9 * k + 9];
    Particle::new(
        Vec3::new(c[0], c[1], c[2]),
        Vec3::new(c[3], c[4], c[5]),
        Vec3::zero(),
        Mass::new::<kilogram>(4.0 * std::f64::consts::PI / 3.0 * r * r * r * 2500.0),
        Length::new::<meter>(r),
        ThermodynamicTemperature::new::<kelvin>(300.0),
    )
    .expect("valid sphere")
}

fn table(text: &str, sep_comma: bool, skip: usize) -> Vec<Vec<f64>> {
    text.lines()
        .skip(skip)
        .filter(|l| !l.trim().is_empty())
        .map(|l| {
            if sep_comma {
                l.split(',').map(|v| v.parse().expect("numeric")).collect()
            } else {
                l.split_whitespace()
                    .map(|v| v.parse().expect("numeric"))
                    .collect()
            }
        })
        .collect()
}

/// Servo plate pressing two spheres: trajectories (501 frames) and the twelve
/// `f_plate` values at 81 steps bit-identical to LIGGGHTS.
///
/// Result (2026-11-11, Rust 1.99.1, LIGGGHTS 3d5c00f2): see the assertion
/// message. Measured: **0 of 9 990 compared values differ** (501 frames x 18
/// + 81 x 12). Deck order: floor, then the mesh wall (the crate's default).
#[test]
fn mesh_servo_matches_liggghts() {
    run_servo("mesh_servo.csv", "mesh_servo_thermo.txt", false);
}

/// The same deck with the **mesh wall declared before the floor**
/// (`in.mesh_servo_rev`, gh:#1482): LIGGGHTS applies `post_force` in
/// declaration order (`Modify::post_force`), and the per-contact
/// `f_pw = f_after - f_before` rounds differently, so its trajectory differs
/// from the forward deck (thermo line 11 first differs). The port follows via
/// `GranularSystem::with_fix_order`.
///
/// Before the fix (fixed order, primitive walls first): 1890 of 9 990 values
/// differed by 1 ulp in force. Result 2026-11-11: see the assertion.
#[test]
fn mesh_servo_reverse_declaration_order_matches_liggghts() {
    run_servo("mesh_servo_rev.csv", "mesh_servo_rev_thermo.txt", true);
}

fn run_servo(csv_name: &str, thermo_name: &str, mesh_first: bool) {
    let (Some(csv), Some(thermo), Some(stl)) = (
        read(csv_name),
        read(thermo_name),
        read("plate_servo.stl"),
    ) else {
        eprintln!("skipping: reference-data/liggghts/ not present");
        return;
    };
    let rows = table(&csv, true, 1);
    let thermo = table(&thermo, false, 0);
    assert_eq!(rows.len(), 501);
    assert_eq!(thermo.len(), 81);
    let servo = MeshServo::new(
        [0.0, 0.0, 0.0112],
        [0.0, 0.0, 1.0],
        0.1,
        0.05,
        [1.0, 0.2, 0.0001],
        1.0e-5,
        0.003,
    );
    let plate = MovingBoundary::new(
        WallGeometry::Mesh(MeshWall::from_ascii_stl(&stl).expect("valid plate stl")),
        Vec3::zero(),
        Vec3::zero(),
        Vec3::zero(),
    )
    .with_servo(servo);
    let floor =
        Boundary::wall(Vec3::new(0.0, 0.0, 0.0), Vec3::new(0.0, 0.0, 1.0)).expect("valid wall");
    let material = GranularMaterial::new(1.0e7, 0.3, 0.9, 0.5).expect("valid material");
    let sys = GranularSystem::new(
        vec![sphere(&rows[0], 0, 0.005), sphere(&rows[0], 1, 0.005)],
        vec![floor],
        GranularContactModel::hertz_history(material),
        Vec3::new(0.0, 0.0, -9.81),
        1.0e-5,
    )
    .expect("valid system")
    .with_moving_walls(vec![plate])
    .with_neighbor_skin(0.003);
    if mesh_first {
        assert!(sys.clone().with_fix_order(vec![FixSlot::Gravity]).is_err());
    }
    let mut sys = if mesh_first {
        sys.with_fix_order(vec![
            FixSlot::MovingWall(0),
            FixSlot::PrimitiveWall(0),
            FixSlot::Gravity,
        ])
        .expect("complete fix order")
    } else {
        sys
    };

    let (mut mismatched, mut total, mut first) = (0, 0, None);
    let mut last = 0_usize;
    let mut thermo_rows = thermo.iter();
    let mut next_thermo = thermo_rows.next();
    let mut moved = 0.0_f64;
    for row in &rows {
        let step = row[0] as usize;
        while let Some(th) = next_thermo {
            let ts = th[0] as usize;
            if ts > step {
                break;
            }
            sys.run(ts - last);
            last = ts;
            for k in 0..12 {
                total += 1;
                let got = if k < 9 {
                    sys.mesh_stress_vector(0, k).expect("stress tracked")
                } else {
                    sys.mesh_servo_xcm(0, k - 9).expect("servo")
                };
                if k == 11 {
                    moved = moved.max(0.0112 - got);
                }
                if got.to_bits() != th[1 + k].to_bits() {
                    mismatched += 1;
                    first.get_or_insert((1000 + ts, k, got, th[1 + k]));
                }
            }
            next_thermo = thermo_rows.next();
        }
        sys.run(step - last);
        last = step;
        for (i, q) in sys.particles().iter().enumerate() {
            let got = [
                q.position.x,
                q.position.y,
                q.position.z,
                q.velocity.x,
                q.velocity.y,
                q.velocity.z,
                q.angular_velocity.x,
                q.angular_velocity.y,
                q.angular_velocity.z,
            ];
            for (k, g) in got.iter().enumerate() {
                total += 1;
                if g.to_bits() != row[1 + 9 * i + k].to_bits() {
                    mismatched += 1;
                    first.get_or_insert((step, k + 100 * i, *g, row[1 + 9 * i + k]));
                }
            }
        }
    }
    assert!(moved > 1.0e-3, "the servo never moved the plate: {moved}");
    assert_eq!(mismatched, 0, "{mismatched}/{total} differ, first {first:?}");
}

/// The upstream abort is kept: `vel_max >= skin / (2 dt)` panics.
#[test]
#[should_panic(expected = "vel_max < skin")]
fn servo_rejects_too_fast_a_plate() {
    let _ = MeshServo::new(
        [0.0; 3],
        [0.0, 0.0, 1.0],
        0.1,
        500.0,
        [1.0, 0.0, 0.0],
        1.0e-5,
        0.003,
    );
}
