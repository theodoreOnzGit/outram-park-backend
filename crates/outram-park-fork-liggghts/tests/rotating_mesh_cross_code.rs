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

//! # Rotating mesh wall against upstream LIGGGHTS `fix move/mesh rotate`
//!
//! gh:#1406 prio-2 row "rotating mesh: one sphere on a rotating plate".
//!
//! ## Methodology
//!
//! Upstream deck `reference-data/liggghts/in.rotating_plate` (LIGGGHTS-PUBLIC
//! `3d5c00f2`, patched `dump custom`, recipe in
//! `upstream_source/README.md`): a `d = 10 mm`, `ρ = 2500 kg/m³` sphere starts
//! at `(0.05, 0, 0.0049)` m, 0.1 mm into a `0.3 m` square plate (two facets,
//! `plate_square.stl`, normal `+z`) that rotates about `z` through the origin
//! with `period 0.5 s` (`fix move/mesh ... rotate origin 0 0 0 axis 0 0 1
//! period 0.5`, `ω = 2π/0.5 = 4π rad/s`). `E = 1e7`, `ν = 0.3`, `e = 0.9`,
//! `µ = 0.5`, Hertz + tangential history, gravity `-z`, `dt = 10 µs`, 10 000
//! steps, 251 frames of x, v, ω. The sphere bounces three times on the
//! moving surface, so the case exercises mesh-wall contact make and break, the
//! wall surface velocity `ω × r` in the friction law, and the shear history
//! against a moving wall. Same geometry read by both codes
//! (`MeshWall::from_ascii_stl`).
//!
//! Pass criterion of this test: every `f64` of every frame bit-identical.
//!
//! ## Prediction (written before the first run, 2026-11-10)
//!
//! Bit-identical in the first contact frames, possibly drifting at the ulp
//! level later: upstream rotates the mesh nodes incrementally and forms the
//! wall velocity from per-node velocities `ω × (node − origin)` interpolated to
//! the contact point, where this port's `MovingBoundary::advance` uses a
//! Rodrigues rotation and `surface_velocity` evaluates `ω × (contact point −
//! pivot)` directly. Same quantity, different operation order.
//!
//! ## Result, first run (2026-11-10, previous agent): prediction NOT met
//!
//! Two findings: (1) upstream's mesh node velocities are zero until the mover's
//! first `initial_integrate`, so the set-up force evaluation sees a wall at
//! rest, which the port did not copy (first half-kick ~2x off); (2) 1915 of
//! 2259 values differed in the last bits (worst `|dx| 5.6e-15 m`, `|dv|
//! 8.1e-12 m/s`, `|dw| 1.1e-9 rad/s`) because the mesh contact path was not
//! translated from `TriMesh::resolveTriSphereContactBary` and the surface-model
//! wall branch.
//!
//! ## Result, after the translation (2026-11-11, Rust 1.99.1): **2259/2259 bit-identical**
//!
//! gh:#1416 translation in `src/tri_mesh.rs` (contact resolution in the face,
//! edge and corner regions with upstream's barycentric region code, edge/corner
//! activity from the mesh topology, per-node velocities interpolated with the
//! barycentric coordinates), `ContactKinematics::mesh_wall` (`r = radi -
//! deltan`, `en = delta*rinv`, `cr = radi - 0.5*deltan`), and the mover
//! (`MovingBoundary::rotating`: node velocities zero until the first advance,
//! incremental quaternion rotation of nodes **and of the cached edge vectors
//! and normals**, recomputed from the nodes only on a neighbour-list rebuild,
//! which `GranularSystem::with_neighbor_skin(0.003)` reproduces from upstream's
//! `neighbor 0.003`: particle moved > skin/2, or a mesh node did, one step
//! earlier). The prediction (bit-identical) is met; the order dependence
//! the first prediction worried about was real and is exactly the rebuild
//! schedule above: without `with_neighbor_skin` the geometry is recomputed
//! every step and the last bits differ.

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

struct Stats {
    mismatched: usize,
    total: usize,
    first: Option<(usize, usize, f64, f64)>,
    worst: [f64; 3],
}

/// Run the case and compare every value of every frame.
fn run_case() -> Option<Stats> {
    let (Some(csv), Some(stl)) = (read("rotating_plate.csv"), read("plate_square.stl")) else {
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
    // `fix move/mesh rotate origin 0 0 0 axis 0 0 1 period 0.5`
    let plate = MovingBoundary::rotating(
        WallGeometry::Mesh(mesh),
        Vec3::zero(),
        Vec3::new(0.0, 0.0, 1.0),
        0.5,
    );
    let material = GranularMaterial::new(1.0e7, 0.3, 0.9, 0.5).expect("valid material");
    // `neighbor 0.003 bin`: upstream's rebuild schedule decides when the
    // rotating mesh's cached normals are recomputed (gh:#1416).
    let mut sys = GranularSystem::new(
        vec![p],
        vec![],
        GranularContactModel::hertz_history(material),
        Vec3::new(0.0, 0.0, -9.81),
        1.0e-5,
    )
    .expect("valid system")
    .with_moving_walls(vec![plate])
    .with_neighbor_skin(0.003);

    let mut st = Stats {
        mismatched: 0,
        total: 0,
        first: None,
        worst: [0.0; 3],
    };
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
            st.worst[k / 3] = st.worst[k / 3].max((g - want).abs());
            st.total += 1;
            if g.to_bits() != want.to_bits() {
                st.mismatched += 1;
                st.first.get_or_insert((step, k, *g, want));
            }
        }
    }
    Some(st)
}

/// Bit-identity criterion for the rotating plate: every `f64` of every frame.
#[test]
fn rotating_plate_matches_liggghts() {
    let Some(st) = run_case() else { return };
    assert_eq!(
        st.mismatched, 0,
        "not bit-identical vs LIGGGHTS: {}/{} values differ, first {:?}",
        st.mismatched, st.total, st.first
    );
}
