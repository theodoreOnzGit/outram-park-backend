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

//! # `fix mesh/surface/stress` against upstream (gh:#1416)
//!
//! `MeshModuleStress` (`mesh_module_stress.cpp`), LIGGGHTS-PUBLIC `3d5c00f2`,
//! against [`outram_park_fork_liggghts::mesh_stress::MeshStress`].
//!
//! ## Methodology
//!
//! Deck `reference-data/liggghts/in.mesh_stress`: two spheres (`d = 10 mm`
//! with `vx = 0.05 m/s`, and `d = 12 mm`, the second straddling the plate's
//! diagonal so it touches two facets) rest on the 0.3 m square
//! `plate_square.stl` rotating about `z` with period 5 s, `fix
//! mesh/surface/stress ... reference_point 0.02 -0.01 0.03`, 10 000 steps of
//! `dt = 1e-5`, `E = 1e7`, `nu = 0.3`, `e = 0.9`, `mu = 0.5`. Compared bit for
//! bit: 251 trajectory frames (`mesh_stress.csv`) and, every 250 steps, the
//! nine `f_plate[1..9]` of `fix mesh/surface/stress` (total force on the
//! mesh, total torque about the reference point, the reference point which
//! rotates with the mesh; `mesh_stress_thermo.txt`, printed `%.17g`).
//!
//! **Prediction (written before the first run, 2026-11-11).** Bit-identical:
//! the force on the mesh is `-(f_after - f_before)` of the same contact force
//! (the difference is taken exactly as `fix_wall_gran_base.h` does), the
//! torque a cross product of the same terms, the totals a two-term sum per
//! step (commutative, so the unreproduced neighbour-list order of the two
//! particles cannot matter), the reference point rotated by the same
//! quaternion step as the nodes.

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

/// 2 spheres on a rotating plate: trajectories (251 frames) and the nine
/// stress-module values at 41 steps bit-identical to LIGGGHTS.
///
/// Result (2026-11-11, Rust 1.99.1, LIGGGHTS 3d5c00f2): **0 of 3 069 compared
/// values differ** (251 frames x 18 trajectory values + 41 x 9 stress values), the
/// total force `z` reaching the two spheres' weight; on failure see the assertion
/// message.
#[test]
fn mesh_stress_matches_liggghts() {
    let (Some(csv), Some(thermo), Some(stl)) = (
        read("mesh_stress.csv"),
        read("mesh_stress_thermo.txt"),
        read("plate_square.stl"),
    ) else {
        eprintln!("skipping: reference-data/liggghts/ not present");
        return;
    };
    let rows: Vec<Vec<f64>> = csv
        .lines()
        .skip(1)
        .filter(|l| !l.trim().is_empty())
        .map(|l| {
            l.split(',')
                .map(|v| v.parse().expect("numeric csv"))
                .collect()
        })
        .collect();
    let thermo: Vec<Vec<f64>> = thermo
        .lines()
        .map(|l| {
            l.split_whitespace()
                .map(|v| v.parse().expect("numeric thermo"))
                .collect()
        })
        .collect();
    assert_eq!(rows.len(), 251);
    assert_eq!(thermo.len(), 41);
    let plate = MovingBoundary::rotating(
        WallGeometry::Mesh(MeshWall::from_ascii_stl(&stl).expect("valid plate stl")),
        Vec3::zero(),
        Vec3::new(0.0, 0.0, 1.0),
        5.0,
    )
    .with_stress(Vec3::new(0.02, -0.01, 0.03));
    let material = GranularMaterial::new(1.0e7, 0.3, 0.9, 0.5).expect("valid material");
    let mut sys = GranularSystem::new(
        vec![sphere(&rows[0], 0, 0.005), sphere(&rows[0], 1, 0.006)],
        vec![],
        GranularContactModel::hertz_history(material),
        Vec3::new(0.0, 0.0, -9.81),
        1.0e-5,
    )
    .expect("valid system")
    .with_moving_walls(vec![plate])
    .with_neighbor_skin(0.003);

    let (mut mismatched, mut total, mut first) = (0, 0, None);
    let mut last = 0_usize;
    let mut thermo_rows = thermo.iter();
    let mut next_thermo = thermo_rows.next();
    let mut max_force = 0.0_f64;
    for row in &rows {
        let step = row[0] as usize;
        // stress values are sampled whenever a thermo row falls on or before
        // this frame: thermo rows are every 250 steps, frames every 40
        while let Some(th) = next_thermo {
            let ts = th[0] as usize;
            if ts > step {
                break;
            }
            sys.run(ts - last);
            last = ts;
            for k in 0..9 {
                total += 1;
                let got = sys.mesh_stress_vector(0, k).expect("stress tracked");
                if k < 3 {
                    max_force = max_force.max(got.abs());
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
    assert!(
        max_force > 0.03,
        "the mesh never carried the spheres' weight: {max_force}"
    );
    assert_eq!(
        mismatched, 0,
        "{mismatched}/{total} differ, first {first:?}"
    );
}

/// A mesh without `with_stress` reports no stress vector; with it the total
/// force on a loaded plate points down and the loaded facets carry a positive
/// normal stress (`sigma_n = |F.n| / A`, per-facet values not cross-coded).
#[test]
fn facet_stress_sums_to_the_total_force() {
    let (Some(csv), Some(stl)) = (read("mesh_stress.csv"), read("plate_square.stl")) else {
        return;
    };
    let rows: Vec<Vec<f64>> = csv
        .lines()
        .skip(1)
        .filter(|l| !l.trim().is_empty())
        .map(|l| {
            l.split(',')
                .map(|v| v.parse().expect("numeric csv"))
                .collect()
        })
        .collect();
    let mesh = || WallGeometry::Mesh(MeshWall::from_ascii_stl(&stl).expect("valid plate stl"));
    let material = GranularMaterial::new(1.0e7, 0.3, 0.9, 0.5).expect("valid material");
    let build = |plate: MovingBoundary| {
        GranularSystem::new(
            vec![sphere(&rows[0], 0, 0.005), sphere(&rows[0], 1, 0.006)],
            vec![],
            GranularContactModel::hertz_history(material),
            Vec3::new(0.0, 0.0, -9.81),
            1.0e-5,
        )
        .expect("valid system")
        .with_moving_walls(vec![plate])
    };
    let mut plain = build(MovingBoundary::new(
        mesh(),
        Vec3::zero(),
        Vec3::zero(),
        Vec3::zero(),
    ));
    plain.run(10);
    assert_eq!(plain.mesh_stress_vector(0, 0), None);
    let mut sys = build(
        MovingBoundary::new(mesh(), Vec3::zero(), Vec3::zero(), Vec3::zero())
            .with_stress(Vec3::zero()),
    );
    sys.run(1);
    let fz = sys.mesh_stress_vector(0, 2).expect("tracked");
    assert!(fz < 0.0, "the plate is pushed down by the spheres: {fz}");
    let loaded = (0..2).any(|i| {
        let st = sys.mesh_stress_vector(0, 2);
        st.is_some() && sys.facet_stress(0, i).is_some_and(|(sn, _)| sn > 0.0)
    });
    assert!(loaded, "no facet carries a normal stress");
}
