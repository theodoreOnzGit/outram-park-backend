// SPDX-License-Identifier: GPL-3.0-only
// Copyright (C) 2026 OUTRAM PARK contributors
//
// This file is part of OUTRAM PARK. See the LICENSE file at the crate root.

//! # `compute wall/gran/local` against `GranularSystem::wall_gran_local` (gh:#1418)
//!
//! ## Methodology
//!
//! Deck `reference-data/liggghts/in.wgl_pebble` (LIGGGHTS-PUBLIC `3d5c00f2`):
//! the committed settled `D/d = 6` bed (354 pebbles, `E = 5e6`, Hertz +
//! history, `mu = 0.3`, `e = 0.5`, `dt = 5 us`, gravity) on a **mesh floor**
//! (`plate_square.stl`, two triangles, `z = 0`, shared diagonal `x = y` under
//! the bed) plus a primitive cylinder (not reported by the compute). `compute
//! wall/gran/local pos vel id force torque history contactArea delta` is
//! dumped every step with `%.17g`; steps 1, 10 and 100 are committed
//! (`wgl_pebble_step{1,10,100}.txt`, 21/30/30 contacts). This test builds the
//! same system, runs the same number of steps and compares all 26 columns of
//! every upstream row, matched on `(particle id, triangle id)`, on bit
//! patterns; the row count must be identical.
//!
//! ## Prediction (written before the run, 2026-11-11)
//!
//! Every column of every contact bit-identical at all three steps: the mesh
//! contact kinematics and the frozen force evaluation are the ones already
//! bit-identical in the mesh-wall tests and in `pair_gran_local_cross_code`;
//! the new arithmetic is the contact point `x - delta`, `(r_i^2 - r^2) pi` and
//! `r_i - sqrt(r^2)`, all single operations in upstream's order. The case
//! does not exercise a moving mesh (wall velocity 0) or an edge/corner
//! contact.
//!
//! ## Result
//!
//! Printed with `--nocapture`; recorded in `docs/cross-code-summary.md`.

use outram_park_fork_liggghts::boundary::Boundary;
use outram_park_fork_liggghts::granular::{GranularContactModel, GranularMaterial};
use outram_park_fork_liggghts::granular_system::GranularSystem;
use outram_park_fork_liggghts::mesh_wall::{MeshWall, MovingBoundary, WallGeometry};
use outram_park_fork_liggghts::neighbor_list::LiggghtsNeighbor;
use outram_park_fork_liggghts::particle::{Particle, Vec3};
use uom::si::f64::{Length, Mass, ThermodynamicTemperature};
use uom::si::{length::meter, mass::kilogram, thermodynamic_temperature::kelvin};

const BED: &str = include_str!("../../../reference-data/liggghts/pebble_bed_settled.csv");
const STL: &str = include_str!("../../../reference-data/liggghts/plate_square.stl");
const STEPS: [(usize, &str); 3] = [
    (1, include_str!("../../../reference-data/liggghts/wgl_pebble_step1.txt")),
    (10, include_str!("../../../reference-data/liggghts/wgl_pebble_step10.txt")),
    (100, include_str!("../../../reference-data/liggghts/wgl_pebble_step100.txt")),
];

fn system() -> GranularSystem {
    let r = 0.005;
    let mass = 4.0 * std::f64::consts::PI / 3.0 * r * r * r * 2500.0;
    let particles: Vec<Particle> = BED
        .lines()
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
        .collect();
    let material = GranularMaterial::new(5.0e6, 0.3, 0.5, 0.3).unwrap();
    let cylinder = Boundary::cylinder(Vec3::zero(), Vec3::new(0.0, 0.0, 1.0), 0.030).unwrap();
    let floor = MovingBoundary::new(
        WallGeometry::Mesh(MeshWall::from_ascii_stl(STL).unwrap()),
        Vec3::zero(),
        Vec3::zero(),
        Vec3::zero(),
    );
    GranularSystem::new(
        particles,
        vec![cylinder],
        GranularContactModel::hertz_history(material),
        Vec3::new(0.0, 0.0, -9.81),
        5e-6,
    )
    .unwrap()
    .with_moving_walls(vec![floor])
    .with_neighbor_skin(0.003)
    .with_liggghts_neighbor_order(
        LiggghtsNeighbor::new(Vec3::new(-1.0, -1.0, -0.005), Vec3::new(1.0, 1.0, 0.35), 0.003).unwrap(),
    )
}

/// All 26 columns of every contact at steps 1, 10, 100 (see module docs for
/// the prediction and result).
#[test]
fn wall_gran_local_matches_upstream() {
    let mut sys = system();
    let mut done = 0usize;
    let (mut total_values, mut bad_values) = (0usize, 0usize);
    for (step, dump) in STEPS {
        sys.run(step - done);
        done = step;
        let rows: Vec<Vec<f64>> = dump
            .lines()
            .skip(9)
            .filter(|l| !l.trim().is_empty())
            .map(|l| l.split_whitespace().map(|x| x.parse().unwrap()).collect())
            .collect();
        let ours = sys.wall_gran_local();
        assert_eq!(ours.len(), rows.len(), "step {step}: contact count");
        let mut tris = [0usize; 2];
        let mut step_bad = 0usize;
        for row in &rows {
            let (tag, tri) = (row[14] as u64 - 1, row[13] as usize);
            tris[tri] += 1;
            let rec = ours
                .iter()
                .find(|r| r.tag == tag && r.tri == tri)
                .unwrap_or_else(|| panic!("step {step}: upstream contact (tag {tag}, tri {tri}) missing"));
            assert_eq!(rec.mesh as f64, row[12]);
            let mine = [
                rec.contact_point.x, rec.contact_point.y, rec.contact_point.z,
                rec.pos_i.x, rec.pos_i.y, rec.pos_i.z,
                rec.wall_vel.x, rec.wall_vel.y, rec.wall_vel.z,
                rec.vel_i.x, rec.vel_i.y, rec.vel_i.z,
                rec.force.x, rec.force.y, rec.force.z,
                rec.torque.x, rec.torque.y, rec.torque.z,
                rec.history.x, rec.history.y, rec.history.z,
                rec.contact_area, rec.delta,
            ];
            let theirs: Vec<f64> = row[0..12].iter().chain(&row[15..26]).copied().collect();
            for k in 0..23 {
                total_values += 1;
                if mine[k].to_bits() != theirs[k].to_bits() {
                    bad_values += 1;
                    step_bad += 1;
                    eprintln!("step {step} tag {tag} tri {tri} field {k}: ours {:e} upstream {:e}", mine[k], theirs[k]);
                }
            }
        }
        println!(
            "wall/gran/local step {step}: {} contacts (tri0 {}, tri1 {}), {step_bad} of {} values differ",
            rows.len(), tris[0], tris[1], rows.len() * 23
        );
    }
    println!("wall/gran/local: {} of {total_values} values bit-identical", total_values - bad_values);
    assert_eq!(bad_values, 0);
}
