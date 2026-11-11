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
    run_servo(&Case::force_pid("mesh_servo", "plate_servo.stl", false));
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
    run_servo(&Case::force_pid("mesh_servo_rev", "plate_servo.stl", true));
}

/// `ctrlPV torque` (PID): the plate rotates about `z` through `com`, nodes
/// rebuilt as the original nodes rotated by the TOTAL quaternion, cached
/// edge vectors and normals rotated by the incremental one; node velocity
/// `omega x (node - xcm)`. Deck `in.mesh_servo_torque` (two sliding spheres
/// pinched between plate and floor). Result: see the assertion message.
#[test]
fn mesh_servo_torque_matches_liggghts() {
    let mut c = Case::force_pid("mesh_servo_torque", "plate_torque.stl", false);
    c.torque = true;
    c.com = [0.0, 0.0, 0.0099];
    c.target = 0.0001;
    c.expect_moved = 1.0e-5;
    run_servo(&c);
}

/// `mode auto` on the force servo: cruise at `ctrl_op_max` while the mesh
/// neighbour list holds no contact, then the piecewise gain (needs
/// `getTotalNumContacts`, recounted on each neighbour-list rebuild). Deck
/// `in.mesh_servo_auto`, `target_val -0.1`, `ratio 5e-5`.
#[test]
fn mesh_servo_auto_matches_liggghts() {
    let mut c = Case::force_pid("mesh_servo_auto", "plate_auto.stl", false);
    c.auto_ratio = Some(0.00005);
    c.gains = [0.01, 0.0, 0.0];
    c.com = [0.0, 0.0, 0.015];
    c.target = -0.1;
    run_servo(&c);
}

/// `ctrlPV torque` with `mode auto` (`ctrl_op_min = vel_min / rPaMax`).
#[test]
fn mesh_servo_torque_auto_matches_liggghts() {
    let mut c = Case::force_pid("mesh_servo_torque_auto", "plate_torque.stl", false);
    c.torque = true;
    c.auto_ratio = Some(0.00005);
    c.gains = [0.01, 0.0, 0.0];
    c.com = [0.0, 0.0, 0.0099];
    c.target = 0.0001;
    c.expect_moved = 1.0e-5;
    run_servo(&c);
}

/// One servo deck: its reference files, controller and plate.
struct Case {
    deck: &'static str,
    stl: &'static str,
    mesh_first: bool,
    torque: bool,
    /// `mode auto ratio`; `None` is PID.
    auto_ratio: Option<f64>,
    com: [f64; 3],
    target: f64,
    gains: [f64; 3],
    /// The servo must have moved the plate centre by more than this
    /// (force servo) or the torque about `z` must be non-zero (torque servo).
    expect_moved: f64,
}

impl Case {
    fn force_pid(deck: &'static str, stl: &'static str, mesh_first: bool) -> Self {
        Self {
            deck,
            stl,
            mesh_first,
            torque: false,
            auto_ratio: None,
            com: [0.0, 0.0, 0.0112],
            target: 0.1,
            gains: [1.0, 0.2, 0.0001],
            expect_moved: 1.0e-3,
        }
    }
}

fn run_servo(case: &Case) {
    let csv_name = format!("{}.csv", case.deck);
    let thermo_name = format!("{}_thermo.txt", case.deck);
    let mesh_first = case.mesh_first;
    let (Some(csv), Some(thermo), Some(stl)) = (
        read(&csv_name),
        read(&thermo_name),
        read(case.stl),
    ) else {
        eprintln!("skipping: reference-data/liggghts/ not present");
        return;
    };
    let rows = table(&csv, true, 1);
    let thermo = table(&thermo, false, 0);
    assert_eq!(rows.len(), 501);
    assert_eq!(thermo.len(), 81);
    let mesh = MeshWall::from_ascii_stl(&stl).expect("valid plate stl");
    let mut servo = MeshServo::new(
        case.com,
        [0.0, 0.0, 1.0],
        case.target,
        0.05,
        case.gains,
        1.0e-5,
        0.003,
    );
    if case.torque {
        servo = servo.with_torque(&mesh.nodes());
    }
    if let Some(ratio) = case.auto_ratio {
        servo = servo.with_auto(ratio, 0.005);
    }
    let plate = MovingBoundary::new(
        WallGeometry::Mesh(mesh),
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
    let (mut max_op, mut zero_contacts, mut some_contacts) = (0.0_f64, false, false);
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
                if k == 11 && !case.torque {
                    moved = moved.max((case.com[2] - got).abs());
                }
                if k == 5 && case.torque {
                    moved = moved.max(got.abs());
                }
                if got.to_bits() != th[1 + k].to_bits() {
                    mismatched += 1;
                    first.get_or_insert((1000 + ts, k, got, th[1 + k]));
                }
            }
            next_thermo = thermo_rows.next();
            let op = sys.mesh_servo_ctrl_op(0).expect("servo");
            max_op = max_op.max((op[0] * op[0] + op[1] * op[1] + op[2] * op[2]).sqrt());
            if sys.mesh_servo_contacts(0).expect("servo") == 0 {
                zero_contacts = true;
            } else {
                some_contacts = true;
            }
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
    assert!(
        moved > case.expect_moved,
        "the servo never moved the plate: {moved}"
    );
    assert_eq!(mismatched, 0, "{mismatched}/{total} differ, first {first:?}");
    if case.torque {
        assert!(max_op > 1.0e-3, "the torque servo never rotated the plate: {max_op}");
    }
    if case.auto_ratio.is_some() && !case.torque {
        assert!(zero_contacts && some_contacts, "auto mode needs both cruise and contact phases");
    }
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
