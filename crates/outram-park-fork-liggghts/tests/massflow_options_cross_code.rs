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

//! # `fix massflow/mesh` options against upstream (gh:#1416)
//!
//! The options of `FixMassflowMesh` / `FixMassflowMeshSieve` that
//! `massflow_mesh_cross_code.rs` does not cover, against LIGGGHTS-PUBLIC
//! `3d5c00f2`: a **moving counting mesh** (`fix move/mesh linear`), `delete_atoms yes`
//! with `sum_property`, and `fix massflow/mesh/sieve`.
//!
//! ## Methodology
//!
//! All three decks derive from `in.massflow_plate` (twelve `d = 10 mm`
//! spheres, `E = 1e7`, `nu = 0.3`, `e = 0.9`, `mu = 0.5`, floor wall at
//! `z = -0.12`, 60 000 steps of `dt = 1e-5`, `neighbor 0.003`, the 0.3 m square
//! `plate_square.stl` at `z = 0`). Every 500 steps all particle positions,
//! velocities and angular velocities and every printed `f_<fix>[i]` are
//! compared on their IEEE-754 bit patterns.
//!
//! * `in.massflow_move`: the plate rises at 0.25 m/s (`fix move/mesh linear`),
//!   so `setRefPoint` runs every step and the mesh neighbour list uses the
//!   full skin. The plate has no `fix wall/gran`: it exerts no force
//!   ([`MovingBoundary::without_contact`]).
//! * `in.massflow_delete`: `count once delete_atoms yes sum_property myprop`
//!   on the downward counter. A counted particle is removed at the next
//!   neighbour rebuild (`pre_exchange`) by copying the last atom into its
//!   slot, so the atom count falls 12 -> 5. Compared: the raw dump (matched
//!   by atom id, since the order changes), `f_mfd[1..7]` (mass, count, rates,
//!   deleted mass, deleted count, summed property) and `f_mfu[1..4]`.
//! * `in.massflow_sieve`: `massflow/mesh/sieve`, aperture 20 mm, spacing
//!   15 mm, stiffness 2e4 N/m, damping 2: a first contact draws
//!   `RanPark(15485863).uniform()` against the pass probability; a blocked
//!   particle feels the sieve force.
//!
//! ## Prediction (written before the first run, 2026-11-11)
//!
//! Bit-identical in all three: no particle-particle contact, so the only
//! order-dependent sums are the per-step counter sums (at most a few terms),
//! and the sieve's random draws are consumed one at a time unless two
//! particles first touch the sieve in the same step.
//!
//! ## Result
//!
//! See the assertion messages and the V&V note on gh:#1416.

use outram_park_fork_liggghts::boundary::Boundary;
use outram_park_fork_liggghts::granular::{GranularContactModel, GranularMaterial};
use outram_park_fork_liggghts::granular_system::GranularSystem;
use outram_park_fork_liggghts::massflow::{MassFlowConfig, MassFlowMesh, SieveConfig};
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

fn config(side: f64, once: bool) -> MassFlowConfig {
    MassFlowConfig {
        vec_side: Vec3::new(0.0, 0.0, side),
        once,
        point_at_outlet: None,
        inside_out: false,
        skin: 0.003,
    }
}

const R: f64 = 0.005;

fn sphere(c: &[f64]) -> Particle {
    let mass = 4.0 * std::f64::consts::PI / 3.0 * R * R * R * 2500.0;
    Particle::new(
        Vec3::new(c[0], c[1], c[2]),
        Vec3::new(c[3], c[4], c[5]),
        Vec3::zero(),
        Mass::new::<kilogram>(mass),
        Length::new::<meter>(R),
        ThermodynamicTemperature::new::<kelvin>(300.0),
    )
    .expect("valid sphere")
}

fn system(particles: Vec<Particle>) -> GranularSystem {
    let floor =
        Boundary::wall(Vec3::new(0.0, 0.0, -0.12), Vec3::new(0.0, 0.0, 1.0)).expect("valid wall");
    let material = GranularMaterial::new(1.0e7, 0.3, 0.9, 0.5).expect("valid material");
    GranularSystem::new(
        particles,
        vec![floor],
        GranularContactModel::hertz_history(material),
        Vec3::new(0.0, 0.0, -9.81),
        1.0e-5,
    )
    .expect("valid system")
}

fn table(text: &str) -> Vec<Vec<f64>> {
    text.lines()
        .filter(|l| !l.trim().is_empty())
        .map(|l| {
            l.split_whitespace()
                .map(|v| v.parse().expect("numeric"))
                .collect()
        })
        .collect()
}

fn trajectory_rows(csv: &str) -> Vec<Vec<f64>> {
    csv.lines()
        .skip(1)
        .filter(|l| !l.trim().is_empty())
        .map(|l| l.split(',').map(|v| v.parse().expect("numeric")).collect())
        .collect()
}

/// Bit comparison bookkeeping.
struct Tally {
    mismatched: usize,
    total: usize,
    first: Option<(usize, usize, f64, f64)>,
}

impl Tally {
    fn new() -> Self {
        Self {
            mismatched: 0,
            total: 0,
            first: None,
        }
    }
    fn check(&mut self, step: usize, k: usize, got: f64, want: f64) {
        self.total += 1;
        if got.to_bits() != want.to_bits() {
            self.mismatched += 1;
            self.first.get_or_insert((step, k, got, want));
        }
    }
    fn assert_clean(&self, what: &str) {
        assert_eq!(
            self.mismatched, 0,
            "{what}: {}/{} differ, first {:?}",
            self.mismatched, self.total, self.first
        );
    }
}

fn compare_constant_atoms(
    sys: &GranularSystem,
    row: &[f64],
    step: usize,
    tally: &mut Tally,
) {
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
            tally.check(step, 1000 + 9 * i + k, *g, row[1 + 9 * i + k]);
        }
    }
}

/// Moving counting mesh: the plate rises at 0.25 m/s while two counters
/// (`count multiple` up, `count once` down) follow it. 121 samples x
/// (108 + 8) values bit-identical.
#[test]
fn massflow_on_moving_mesh_matches_liggghts() {
    let (Some(csv), Some(thermo), Some(stl)) = (
        read("massflow_move.csv"),
        read("massflow_move_thermo.txt"),
        read("plate_square.stl"),
    ) else {
        eprintln!("skipping: reference-data/liggghts/ not present");
        return;
    };
    let rows = trajectory_rows(&csv);
    let thermo = table(&thermo);
    assert_eq!((rows.len(), thermo.len()), (121, 121));
    let particles: Vec<Particle> = (0..12).map(|i| sphere(&rows[0][1 + 9 * i..])).collect();
    let plate = MovingBoundary::new(
        WallGeometry::Mesh(MeshWall::from_ascii_stl(&stl).expect("valid plate stl")),
        Vec3::new(0.0, 0.0, 0.25),
        Vec3::zero(),
        Vec3::zero(),
    )
    .without_contact();
    let mut sys = system(particles)
        .with_moving_walls(vec![plate])
        .with_neighbor_skin(0.003);
    let mesh = || MeshWall::from_ascii_stl(&stl).expect("valid plate stl");
    let up = sys.add_massflow_mesh(
        MassFlowMesh::new(mesh(), config(1.0, false))
            .expect("valid fix")
            .with_moving_wall(0),
    );
    let down = sys.add_massflow_mesh(
        MassFlowMesh::new(mesh(), config(-1.0, true))
            .expect("valid fix")
            .with_moving_wall(0),
    );
    let mut tally = Tally::new();
    let mut last = 0;
    for (row, th) in rows.iter().zip(&thermo) {
        let step = row[0] as usize;
        sys.run(step - last);
        last = step;
        compare_constant_atoms(&sys, row, step, &mut tally);
        for k in 0..4 {
            tally.check(step, k, sys.massflow_vector(up, k), th[1 + k]);
        }
        for k in 0..4 {
            tally.check(step, 4 + k, sys.massflow_vector(down, k), th[5 + k]);
        }
    }
    tally.assert_clean("moving mesh");
    assert_eq!(sys.massflow(down).crossings().len(), 5);
}

/// One frame of a raw LAMMPS dump: step and `(id, x y z vx vy vz wx wy wz)`.
fn parse_dump(text: &str) -> Vec<(usize, Vec<(u64, [f64; 9])>)> {
    let mut frames = Vec::new();
    let mut lines = text.lines();
    while let Some(l) = lines.next() {
        if !l.starts_with("ITEM: TIMESTEP") {
            continue;
        }
        let step: usize = lines.next().expect("step").trim().parse().expect("int");
        lines.next(); // NUMBER OF ATOMS
        let n: usize = lines.next().expect("n").trim().parse().expect("int");
        for _ in 0..4 {
            lines.next(); // BOX BOUNDS + 3 lines
        }
        lines.next(); // ATOMS header
        let mut atoms = Vec::new();
        for _ in 0..n {
            let f: Vec<f64> = lines
                .next()
                .expect("atom row")
                .split_whitespace()
                .map(|v| v.parse().expect("numeric"))
                .collect();
            let mut a = [0.0; 9];
            a.copy_from_slice(&f[1..10]);
            atoms.push((f[0] as u64, a));
        }
        frames.push((step, atoms));
    }
    frames
}

/// `delete_atoms yes` + `sum_property`: 121 frames, the atom count falling
/// 12 -> 5, every surviving atom (matched by id) and `f_mfd[1..7]`,
/// `f_mfu[1..4]` bit-identical.
#[test]
fn massflow_delete_atoms_and_sum_property_match_liggghts() {
    let (Some(dump), Some(thermo), Some(stl)) = (
        read("massflow_delete_dump.txt"),
        read("massflow_delete_thermo.txt"),
        read("plate_square.stl"),
    ) else {
        eprintln!("skipping: reference-data/liggghts/ not present");
        return;
    };
    let frames = parse_dump(&dump);
    let thermo = table(&thermo);
    assert_eq!((frames.len(), thermo.len()), (121, 121));
    let mut first_frame = frames[0].1.clone();
    first_frame.sort_by_key(|(id, _)| *id);
    let particles: Vec<Particle> = first_frame.iter().map(|(_, a)| sphere(a)).collect();
    assert_eq!(particles.len(), 12);
    let mut sys = system(particles).with_neighbor_skin(0.003);
    let mesh = || MeshWall::from_ascii_stl(&stl).expect("valid plate stl");
    let up = sys.add_massflow_mesh(MassFlowMesh::new(mesh(), config(1.0, false)).expect("valid fix"));
    let down = sys.add_massflow_mesh(
        MassFlowMesh::new(mesh(), config(-1.0, true))
            .expect("valid fix")
            .with_delete_atoms()
            .expect("count once")
            .with_sum_property(),
    );
    for id in 1..=12_u64 {
        let v: f64 = format!("0.{id}{id}").parse().expect("literal");
        sys.set_massflow_property(down, id - 1, v); // port tags are 0-based
    }
    let mut tally = Tally::new();
    let mut last = 0;
    let mut min_atoms = 12;
    for ((step, atoms), th) in frames.iter().zip(&thermo) {
        assert_eq!(th[0] as usize, *step);
        sys.run(step - last);
        last = *step;
        assert_eq!(sys.particles().len(), atoms.len(), "atom count at step {step}");
        min_atoms = min_atoms.min(atoms.len());
        for (i, q) in sys.particles().iter().enumerate() {
            let tag = sys.tags()[i] + 1; // upstream ids are 1-based
            let want = atoms
                .iter()
                .find(|(id, _)| *id == tag)
                .unwrap_or_else(|| panic!("atom {tag} missing in the dump at step {step}: port ids {:?}, dump ids {:?}", sys.tags().iter().map(|t| t + 1).collect::<Vec<_>>(), atoms.iter().map(|a| a.0).collect::<Vec<_>>()));
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
                tally.check(*step, 1000 + 10 * tag as usize + k, *g, want.1[k]);
            }
        }
        for k in 0..7 {
            tally.check(*step, k, sys.massflow_vector(down, k), th[1 + k]);
        }
        for k in 0..4 {
            tally.check(*step, 7 + k, sys.massflow_vector(up, k), th[8 + k]);
        }
    }
    tally.assert_clean("delete_atoms");
    assert_eq!(min_atoms, 5, "seven particles were counted and deleted");
}

/// `massflow/mesh/sieve`: 121 frames x (108 + 4) values bit-identical.
#[test]
fn massflow_sieve_matches_liggghts() {
    let (Some(csv), Some(thermo), Some(stl)) = (
        read("massflow_sieve.csv"),
        read("massflow_sieve_thermo.txt"),
        read("plate_square.stl"),
    ) else {
        eprintln!("skipping: reference-data/liggghts/ not present");
        return;
    };
    let rows = trajectory_rows(&csv);
    let thermo = table(&thermo);
    assert_eq!((rows.len(), thermo.len()), (121, 121));
    let particles: Vec<Particle> = (0..12).map(|i| sphere(&rows[0][1 + 9 * i..])).collect();
    let mut sys = system(particles).with_neighbor_skin(0.003);
    let sv = sys.add_massflow_mesh(
        MassFlowMesh::new(
            MeshWall::from_ascii_stl(&stl).expect("valid plate stl"),
            config(-1.0, true),
        )
        .expect("valid fix")
        .with_sieve(SieveConfig {
            size: 0.02,
            spacing: 0.015,
            stiffness: 2.0e4,
            damping: 2.0,
        })
        .expect("valid sieve"),
    );
    let mut tally = Tally::new();
    let mut last = 0;
    for (row, th) in rows.iter().zip(&thermo) {
        let step = row[0] as usize;
        sys.run(step - last);
        last = step;
        compare_constant_atoms(&sys, row, step, &mut tally);
        for k in 0..4 {
            tally.check(step, k, sys.massflow_vector(sv, k), th[1 + k]);
        }
    }
    tally.assert_clean("sieve");
}

/// `point_at_outlet` and `inside_out` on a non-planar counting mesh (gh:#1491).
///
/// Deck `in.massflow_outlet`: the closed 64-facet `drum_cylinder.stl`
/// (radius 0.05 m, axis y) as a counting surface only (no `wall/gran`); six
/// spheres start inside it moving out, six outside moving in, and a floor at
/// `z = -0.12` sends them back up. `mfo` is `count once point_at_outlet 0 0 0`,
/// `mfi` the same with `inside_out`. Upstream counts a particle in a facet's
/// mesh neighbour list only when `deltan < radius`, with
/// `dot = delta . (x - p_outlet)` (negated by `inside_out`).
///
/// **Prediction (2026-11-12, before the first run):** bit-identical; no sum
/// has more than a few equal-mass terms per step.
///
/// **Result (2026-11-12, Rust 1.99.1, LIGGGHTS 3d5c00f2):** 121 frames x
/// (108 trajectory + 8 counter) values bit-identical; both counters end at 6
/// particles (`mfo` the six leaving the drum, `mfi` the six entering it).
#[test]
fn massflow_point_at_outlet_and_inside_out_match_liggghts() {
    let (Some(csv), Some(thermo), Some(stl)) = (
        read("massflow_outlet.csv"),
        read("massflow_outlet_thermo.txt"),
        read("drum_cylinder.stl"),
    ) else {
        eprintln!("skipping: reference-data/liggghts/ not present");
        return;
    };
    let rows = trajectory_rows(&csv);
    let thermo = table(&thermo);
    assert_eq!((rows.len(), thermo.len()), (121, 121));
    let particles: Vec<Particle> = (0..12).map(|i| sphere(&rows[0][1 + 9 * i..])).collect();
    let mut sys = system(particles).with_neighbor_skin(0.003);
    let mesh = || MeshWall::from_ascii_stl(&stl).expect("valid drum stl");
    let outlet = |inside_out: bool| MassFlowConfig {
        vec_side: Vec3::zero(),
        once: true,
        point_at_outlet: Some(Vec3::zero()),
        inside_out,
        skin: 0.003,
    };
    let out = sys.add_massflow_mesh(MassFlowMesh::new(mesh(), outlet(false)).expect("valid fix"));
    let inn = sys.add_massflow_mesh(MassFlowMesh::new(mesh(), outlet(true)).expect("valid fix"));
    let mut tally = Tally::new();
    let mut last = 0;
    for (row, th) in rows.iter().zip(&thermo) {
        let step = row[0] as usize;
        sys.run(step - last);
        last = step;
        compare_constant_atoms(&sys, row, step, &mut tally);
        for k in 0..4 {
            tally.check(step, k, sys.massflow_vector(out, k), th[1 + k]);
        }
        for k in 0..4 {
            tally.check(step, 4 + k, sys.massflow_vector(inn, k), th[5 + k]);
        }
    }
    eprintln!(
        "point_at_outlet: {}/{} values differ, first {:?}",
        tally.mismatched, tally.total, tally.first
    );
    tally.assert_clean("point_at_outlet / inside_out");
    assert_eq!(sys.massflow(out).crossings().len(), 6);
    assert_eq!(sys.massflow(inn).crossings().len(), 6);
}

/// The sieve and delete constructors keep upstream's guards.
#[test]
fn sieve_and_delete_guards() {
    let Some(stl) = read("plate_square.stl") else {
        eprintln!("skipping: reference-data/liggghts/ not present");
        return;
    };
    let mesh = || MeshWall::from_ascii_stl(&stl).expect("valid plate stl");
    // `delete_atoms yes` requires `count once`
    assert!(MassFlowMesh::new(mesh(), config(1.0, false))
        .expect("valid fix")
        .with_delete_atoms()
        .is_err());
    let ok = |size: f64, spacing: f64, stiffness: f64| {
        MassFlowMesh::new(mesh(), config(1.0, true))
            .expect("valid fix")
            .with_sieve(SieveConfig {
                size,
                spacing,
                stiffness,
                damping: 0.0,
            })
            .is_ok()
    };
    assert!(ok(0.02, 0.015, 1.0e4));
    assert!(!ok(-1.0, 0.015, 1.0e4));
    assert!(!ok(0.02, 2.0e5, 1.0e4));
    assert!(!ok(0.02, 0.015, 2.0e10));
}
