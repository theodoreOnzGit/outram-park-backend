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
//! ## Result (2026-11-10, Rust 1.99.1, upstream g++ 15.2.1): prediction NOT met, two findings
//!
//! 1. **A real, 2 %-size difference, found and handled in the test setup.**
//!    With the rotation attached before the system is built, the first
//!    half-kick differs from upstream by ~2x (`v_y` after one step `1.309e-3`
//!    against upstream's `5.786e-4`, `ω_x` `0.648` against `0.286`). Cause:
//!    upstream's mesh node velocities are zero until the mover's first
//!    `initial_integrate` (`mesh_mover_rotation.cpp`), so the **setup-time force
//!    evaluation sees a wall at rest**; this port's `with_moving_walls`
//!    evaluates forces with the full wall velocity at once. Building the
//!    system with the wall at rest and switching the rotation on afterwards
//!    reproduces upstream's step 1 to `2e-15` relative. This is an upstream
//!    setup convention, not physics; the port was **not** changed to copy it
//!    (gap issue #1416 decides), the test copies it.
//! 2. **After that, not bit-identical: 1915 of 2259 values differ, worst
//!    `|Δx| = 5.6e-15 m`, `|Δv| = 8.1e-12 m/s`, `|Δω| = 1.1e-9 rad/s`** over the
//!    251 frames (three bounces, each of which amplifies earlier round-off).
//!    First differing value: `y` at step 40, `1.3817222445900777e-5` against
//!    `1.381722244590358e-5` (`2e-13` relative). Cause, from reading upstream:
//!    the mesh contact path differs in operation order from
//!    `TriMesh::resolveTriSphereContactBary` (`tri_mesh_I.h`): upstream forms
//!    `csPlane = p − n·(n·(p−n0))`, `delta = p − csPlane`, `r = radi − deltan`,
//!    `en = delta·(1/r)`, and re-derives the overlap as `radi − r`
//!    (`surface_model_default.h`), while this port takes the facet normal and
//!    Ericson closest point as given (`MeshWall::particle_overlap`). A
//!    faithful translation of `resolveTriSphereContactBary` is gap issue
//!    #1416's job; until then this test pins the measured residual.
//!
//! `rotating_plate_residual_is_roundoff_level` is that pin (a regression pin
//! at about 10x the measurement, NOT a parity claim);
//! `rotating_plate_matches_liggghts` is the bit-identity criterion and is
//! `#[ignore]`d until the contact path is translated.

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
    // LIGGGHTS: omega_ = 2.*M_PI/period.
    let omega = 2.0 * std::f64::consts::PI / 0.5;
    let plate = MovingBoundary::new(
        WallGeometry::Mesh(mesh),
        Vec3::zero(),
        Vec3::zero(),
        Vec3::zero(),
    );
    let material = GranularMaterial::new(1.0e7, 0.3, 0.9, 0.5).expect("valid material");
    let mut sys = GranularSystem::new(
        vec![p],
        vec![],
        GranularContactModel::hertz_history(material),
        Vec3::new(0.0, 0.0, -9.81),
        1.0e-5,
    )
    .expect("valid system")
    .with_moving_walls(vec![plate]);
    // Upstream's mesh node velocities are zero until the mover's first
    // `initial_integrate`, so the setup-time force evaluation sees a wall at
    // rest (see the module docs, finding 1). Switch the rotation on after setup.
    sys.moving_walls_mut()[0].angular_velocity = Vec3::new(0.0, 0.0, omega);

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

/// Bit-identity criterion for the rotating plate. **Ignored**: the prediction
/// was not met (module docs, finding 2: 1915/2259 values differ, worst
/// `|Δω| = 1.1e-9 rad/s`); un-ignore when `resolveTriSphereContactBary` is
/// translated (gh:#1416).
#[test]
#[ignore = "bit-identity not met: mesh contact path differs in operation order from upstream (gh:#1416)"]
fn rotating_plate_matches_liggghts() {
    let Some(st) = run_case() else { return };
    assert_eq!(
        st.mismatched, 0,
        "not bit-identical vs LIGGGHTS: {}/{} values differ, first {:?}",
        st.mismatched, st.total, st.first
    );
}

/// Regression pin on the measured residual (not a parity claim): over the 251
/// frames, `max|Δx| = 5.6e-15 m`, `max|Δv| = 8.1e-12 m/s`,
/// `max|Δω| = 1.1e-9 rad/s` (2026-11-10). Bounds are 10x those, so a change
/// that moves the rotating-wall path by more than round-off growth fails it.
/// Together with the first-step agreement it also pins the setup convention
/// (module docs, finding 1): without it `|Δω|` is ~0.7 rad/s.
#[test]
fn rotating_plate_residual_is_roundoff_level() {
    let Some(st) = run_case() else { return };
    eprintln!(
        "rotating plate vs LIGGGHTS: {}/{} values differ, max|dx| {:e} m, max|dv| {:e} m/s, \
         max|dw| {:e} rad/s, first {:?}",
        st.mismatched, st.total, st.worst[0], st.worst[1], st.worst[2], st.first
    );
    assert!(st.worst[0] <= 5.6e-14, "position drift {:e} m", st.worst[0]);
    assert!(st.worst[1] <= 8.1e-11, "velocity drift {:e} m/s", st.worst[1]);
    assert!(st.worst[2] <= 1.1e-8, "spin drift {:e} rad/s", st.worst[2]);
}
