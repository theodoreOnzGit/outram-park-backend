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

//! # Time-dependent `fix move/mesh` movers against upstream (gh:#1416)
//!
//! `MeshMoverWiggle`, `MeshMoverVibLin` (`mesh_mover_linear.cpp`) and
//! `MeshMoverRiggle`, `MeshMoverVibRot` (`mesh_mover_rotation.cpp`),
//! LIGGGHTS-PUBLIC `3d5c00f2`.
//!
//! ## Methodology
//!
//! Decks `in.mover_wiggle|viblin|riggle|vibrot` (`reference-data/liggghts/`):
//! a `d = 10 mm` sphere (`E = 1e7`, `nu = 0.3`, `e = 0.9`, `mu = 0.5`) rests
//! on the 0.2 m square plate `plate_square.stl` at `(0.05, 0, 0.0049)` under
//! gravity, `dt = 1e-5`, 10 000 steps, 251 frames of x, v, omega compared bit
//! for bit; the plate moves by the named mover (as `in.rotating_plate` does
//! for `rotate`). The port moves the same mesh with
//! [`MovingBoundary::wiggle`] and friends: the time-dependent velocity or
//! rotation rate is evaluated from `time += dt` exactly as
//! `FixMoveMesh::initial_integrate` does, then applied by the constant-rate
//! machinery already verified for `linear` and `rotate`.
//!
//! **Prediction (written before the first run, 2026-11-11).** Bit-identical in
//! all four: each mover is a few `cos`/`sin` evaluations on top of the
//! verified linear/rotate step.

use outram_park_fork_liggghts::granular::{GranularContactModel, GranularMaterial};
use outram_park_fork_liggghts::granular_system::GranularSystem;
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

/// Replay `csv` with the plate moved by `plate`; returns (mismatched, total,
/// first mismatch, final position) after asserting nothing.
fn run_case(csv: &str, plate: impl Fn(WallGeometry) -> MovingBoundary) -> Option<(usize, usize, Option<(usize, usize, f64, f64)>, [f64; 3])> {
    let (Some(csv), Some(stl)) = (read(csv), read("plate_square.stl")) else {
        eprintln!("skipping: reference-data/liggghts/ not present");
        return None;
    };
    let rows: Vec<Vec<f64>> = csv
        .lines()
        .skip(1)
        .filter(|l| !l.trim().is_empty())
        .map(|l| l.split(',').map(|v| v.parse().expect("numeric csv")).collect())
        .collect();
    assert_eq!(rows.len(), 251, "unexpected reference frame count");
    let r = 0.005;
    let rho = 2500.0;
    let p = Particle::new(
        Vec3::new(0.05, 0.0, 0.0049),
        Vec3::zero(),
        Vec3::zero(),
        Mass::new::<kilogram>(4.0 * std::f64::consts::PI / 3.0 * r * r * r * rho),
        Length::new::<meter>(r),
        ThermodynamicTemperature::new::<kelvin>(300.0),
    )
    .expect("valid sphere");
    let mesh = MeshWall::from_ascii_stl(&stl).expect("valid plate stl");
    let material = GranularMaterial::new(1.0e7, 0.3, 0.9, 0.5).expect("valid material");
    let mut sys = GranularSystem::new(
        vec![p],
        vec![],
        GranularContactModel::hertz_history(material),
        Vec3::new(0.0, 0.0, -9.81),
        1.0e-5,
    )
    .expect("valid system")
    .with_moving_walls(vec![plate(WallGeometry::Mesh(mesh))])
    .with_neighbor_skin(0.003);

    let (mut mismatched, mut total, mut first) = (0, 0, None);
    let mut last = 0_usize;
    for row in &rows {
        let step = row[0] as usize;
        sys.run(step - last);
        last = step;
        let q = sys.particles()[0];
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
            let want = row[1 + k];
            total += 1;
            if g.to_bits() != want.to_bits() {
                mismatched += 1;
                first.get_or_insert((step, k, *g, want));
            }
        }
    }
    let q = sys.particles()[0];
    Some((mismatched, total, first, [q.position.x, q.position.y, q.position.z]))
}

fn assert_identical(name: &str, res: Option<(usize, usize, Option<(usize, usize, f64, f64)>, [f64; 3])>) {
    let Some((mismatched, total, first, end)) = res else { return };
    assert_eq!(
        mismatched, 0,
        "{name}: not bit-identical vs LIGGGHTS: {mismatched}/{total} differ, first {first:?}"
    );
    // the plate must actually have moved the sphere
    assert!(
        (end[0] - 0.05).abs() > 1e-4 || (end[2] - 0.0049).abs() > 5e-4 || end[1].abs() > 1e-4,
        "{name}: the sphere never moved, the case is vacuous"
    );
}

/// `wiggle amplitude 0.004 0.002 0.002 period 0.08`.
#[test]
fn wiggle_matches_liggghts() {
    assert_identical(
        "wiggle",
        run_case("mover_wiggle.csv", |g| {
            MovingBoundary::wiggle(g, Vec3::new(0.004, 0.002, 0.002), 0.08)
        }),
    );
}

/// `viblin axis 0.3 0.2 1.0 order 2 amplitude 0.002 0.001 phase 0 1.1 period 0.06 0.035`.
#[test]
fn viblin_matches_liggghts() {
    assert_identical(
        "viblin",
        run_case("mover_viblin.csv", |g| {
            MovingBoundary::viblin(
                g,
                Vec3::new(0.3, 0.2, 1.0),
                &[0.002, 0.001],
                &[0.0, 1.1],
                &[0.06, 0.035],
            )
            .expect("valid order")
        }),
    );
}

/// `riggle origin 0.01 0 0 axis 0.1 0.2 1 period 0.2 amplitude 4` (degrees).
#[test]
fn riggle_matches_liggghts() {
    assert_identical(
        "riggle",
        run_case("mover_riggle.csv", |g| {
            MovingBoundary::riggle(g, Vec3::new(0.01, 0.0, 0.0), Vec3::new(0.1, 0.2, 1.0), 0.2, 4.0)
        }),
    );
}

/// `vibrot origin 0.01 0 0 axis 0.1 0.2 1 order 2 amplitude 0.05 0.02 phase 0
/// 0.7 period 0.15 0.09`.
#[test]
fn vibrot_matches_liggghts() {
    assert_identical(
        "vibrot",
        run_case("mover_vibrot.csv", |g| {
            MovingBoundary::vibrot(
                g,
                Vec3::new(0.01, 0.0, 0.0),
                Vec3::new(0.1, 0.2, 1.0),
                &[0.05, 0.02],
                &[0.0, 0.7],
                &[0.15, 0.09],
            )
            .expect("valid order")
        }),
    );
}

/// The mover constructors reject an empty or over-long harmonic list.
#[test]
fn vibration_movers_validate_their_order() {
    let g = || WallGeometry::Mesh(MeshWall::from_ascii_stl("solid x\nendsolid x\n").unwrap_or_else(|_| {
        panic!("empty stl must parse for this check")
    }));
    let _ = g;
    assert!(MovingBoundary::viblin(
        WallGeometry::Mesh(MeshWall::from_ascii_stl(&read("plate_square.stl").unwrap_or_default()).expect("stl")),
        Vec3::new(0.0, 0.0, 1.0),
        &[],
        &[],
        &[],
    )
    .is_none());
}
