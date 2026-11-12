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

//! # Rotating drum (closed cylinder mesh, two spheres) against upstream LIGGGHTS
//!
//! gh:#1406 prio-2 row "rotating mesh: ... and a drum"; gh:#1416.
//!
//! ## Methodology
//!
//! Deck `reference-data/liggghts/in.rotating_drum` (LIGGGHTS-PUBLIC `3d5c00f2`,
//! patched `dump custom`): a closed cylinder mesh (`drum_cylinder.stl`, made by
//! `make_drum_stl.py`: `R = 0.05 m`, `y in [-0.05, 0.05]`, 16 segments, each
//! side quad as two facets, both end caps as 16-facet centre fans, 64 facets)
//! rotates about `y` through the origin with `period 0.5 s`. Two `d = 8 mm`,
//! `rho = 2500` spheres start 1 mm above the bottom facets and are carried by
//! friction, bounce, roll over facet edges and corners and hit each other
//! (`E = 1e7`, `nu = 0.3`, `e = 0.9`, `mu = 0.5`, Hertz + history, `dt = 10 us`,
//! 20 000 steps, 251 frames of x, v, omega, 2 atoms x 9 = 4518 values).
//! Upstream rebuilt its neighbour list 83 times (`neighbor 0.003`); the port
//! reproduces that schedule with `with_neighbor_skin(0.003)` because upstream
//! refreshes the rotating mesh's cached normals only on a rebuild. Both read
//! the same STL.
//!
//! Pass criterion: every `f64` of every frame bit-identical.
//!
//! ## Prediction (written before the first run, 2026-11-11)
//!
//! Bit-identical, given the translated mesh contact path (gh:#1416) that made
//! the plate case bit-identical.
//!
//! ## Result
//!
//! Recorded in `docs/cross-code-summary.md` (first run 2026-11-11, Rust 1.99.1).

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

/// Every value of every frame compared on its IEEE-754 bit pattern.
#[test]
fn rotating_drum_matches_liggghts() {
    let (Some(csv), Some(stl)) = (read("rotating_drum.csv"), read("drum_cylinder.stl")) else {
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
    assert_eq!(rows.len(), 251);
    let r = 0.004;
    let rho = 2500.0;
    let sphere = |x: f64, y: f64, z: f64| {
        Particle::new(
            Vec3::new(x, y, z),
            Vec3::zero(),
            Vec3::zero(),
            Mass::new::<kilogram>(4.0 * std::f64::consts::PI / 3.0 * r * r * r * rho),
            Length::new::<meter>(r),
            ThermodynamicTemperature::new::<kelvin>(300.0),
        )
        .expect("valid sphere")
    };
    let mesh = MeshWall::from_ascii_stl(&stl).expect("valid drum stl");
    assert_eq!(mesh.triangles.len(), 64);
    let drum = MovingBoundary::rotating(
        WallGeometry::Mesh(mesh),
        Vec3::zero(),
        Vec3::new(0.0, 1.0, 0.0),
        0.5,
    );
    let material = GranularMaterial::new(1.0e7, 0.3, 0.9, 0.5).expect("valid material");
    let mut sys = GranularSystem::new(
        vec![sphere(0.0, 0.0, -0.0440), sphere(0.009, 0.004, -0.0435)],
        vec![],
        GranularContactModel::hertz_history(material),
        Vec3::new(0.0, 0.0, -9.81),
        1.0e-5,
    )
    .expect("valid system")
    .with_moving_walls(vec![drum])
    .with_neighbor_skin(0.003);

    let (mut bad, mut total) = (0usize, 0usize);
    let mut first = None;
    let mut worst = [0.0_f64; 3];
    let mut last = 0usize;
    for row in &rows {
        let step = row[0] as usize;
        sys.run(step - last);
        last = step;
        for (a, q) in sys.particles().iter().enumerate() {
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
                let want = row[1 + a * 9 + k];
                worst[k / 3] = worst[k / 3].max((g - want).abs());
                total += 1;
                if g.to_bits() != want.to_bits() {
                    bad += 1;
                    first.get_or_insert((step, a, k, *g, want));
                }
            }
        }
    }
    eprintln!("rotating drum vs LIGGGHTS: {bad}/{total} differ, worst {worst:?}, first {first:?}");
    assert_eq!(
        bad, 0,
        "not bit-identical: {bad}/{total}, first {first:?}, worst {worst:?}"
    );
}
