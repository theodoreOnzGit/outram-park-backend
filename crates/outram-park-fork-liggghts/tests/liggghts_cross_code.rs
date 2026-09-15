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

//! # Cross-code verification against upstream LIGGGHTS-PUBLIC
//!
//! ## Methodology
//!
//! Upstream LIGGGHTS-PUBLIC (commit `3d5c00f2`, LAMMPS base "23 Nov 2013") was
//! **compiled from source and executed**; its trajectories are committed under
//! `reference-data/liggghts/` together with the exact input scripts and a note
//! of the two-line output-precision patch applied to `dump_custom.cpp` (which
//! changes printed digits only — see that directory's `README.md`). These tests
//! re-run the same cases through [`GranularSystem`] and compare **every sampled
//! frame**, not just the endpoint.
//!
//! This is the evidence leg the crate's `CLAUDE.md` recorded as missing at its
//! 2026-09-06 maturity declaration ("no cross-code comparison against upstream
//! LIGGGHTS").
//!
//! Every case: monodisperse spheres `d = 10 mm`, `ρ = 2500 kg/m³`,
//! `E = 10 MPa`, `ν = 0.3`, `e = 0.9`, `µ = 0.5`, `dt = 1 µs`, `fix nve/sphere`.
//!
//! ## Results (2026-09-15)
//!
//! | Case | Frames | `max|Δx|` [m] | `max|Δv|` [m/s] | `max|Δω|` [rad/s] |
//! |---|---|---|---|---|
//! | head-on, Hertz | 251 | `0` | `0` | `0` |
//! | head-on, Hooke | 251 | `0` | `0` | `0` |
//! | oblique + friction, Hertz | 251 | `0` | `1.11e-16` | `5.68e-14` |
//! | wall bounce + gravity | 2001 | `0` | `0` | — |
//!
//! **Three of the four cases are bit-identical to upstream over their whole
//! trajectory**, including the 400 000-step wall-bounce run. The oblique case —
//! the one that exercises the tangential history spring, Coulomb slip and
//! contact torque — agrees to floating-point round-off: `1.11e-16 m/s` is one
//! ulp at `|v| ≈ 1 m/s`, and `5.68e-14 rad/s` is ~3 ulp at the final
//! `ω_z = −83.1815 rad/s`. The residual is the expected consequence of
//! summing the same terms in a different association order, not a physics
//! difference.
//!
//! Realised restitution, head-on Hertz: **−0.900007 m/s** from `+1.0 m/s`, in
//! both codes, against the requested `e = 0.9`.
//!
//! ## What this does and does not establish
//!
//! It establishes that this crate's **contact force laws, contact kinematics,
//! shear-history bookkeeping and integrator reproduce LIGGGHTS' own** on these
//! cases. It does **not** establish that LIGGGHTS' granular physics is right
//! for a pebble bed — that is validation against experiment, and is discussed
//! separately in `docs/verification-and-validation.md`.
//!
//! ## Skipping
//!
//! If `reference-data/liggghts/` is absent (e.g. a sparse checkout) each test
//! prints a skip notice and passes, matching the convention in
//! `crates/petir/tests/gsl_*.rs`.

use outram_park_fork_liggghts::boundary::Boundary;
use outram_park_fork_liggghts::granular::{
    GranularContactModel, GranularMaterial, GranularNormalModel, TangentialModel,
};
use outram_park_fork_liggghts::granular_system::GranularSystem;
use outram_park_fork_liggghts::particle::{Particle, Vec3};
use uom::si::f64::{Length, Mass, ThermodynamicTemperature};
use uom::si::{length::meter, mass::kilogram, thermodynamic_temperature::kelvin};

/// Particle mass `[kg]` for `d = 10 mm`, `ρ = 2500 kg/m³`.
fn pebble_mass() -> f64 {
    2500.0 * std::f64::consts::PI / 6.0 * 1.0e-6
}

fn sphere(position: Vec3, velocity: Vec3) -> Particle {
    Particle::new(
        position,
        velocity,
        Vec3::zero(),
        Mass::new::<kilogram>(pebble_mass()),
        Length::new::<meter>(0.005),
        ThermodynamicTemperature::new::<kelvin>(300.0),
    )
    .expect("valid pebble")
}

fn material() -> GranularMaterial {
    GranularMaterial::new(1.0e7, 0.3, 0.9, 0.5).expect("valid material")
}

/// Load a committed reference CSV as `step` plus a flat row of `f64`.
///
/// Returns `None` when the reference data is not checked out.
fn load(name: &str) -> Option<Vec<Vec<f64>>> {
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../reference-data/liggghts")
        .join(name);
    let text = std::fs::read_to_string(path).ok()?;
    Some(
        text.lines()
            .skip(1)
            .filter(|l| !l.trim().is_empty())
            .map(|l| l.split(',').map(|v| v.parse().expect("numeric csv")).collect())
            .collect(),
    )
}

/// Replay a two-particle case and return the worst per-frame deviations
/// `(|Δx|, |Δv|, |Δω|)` against the reference.
fn replay_pair(
    rows: &[Vec<f64>],
    model: GranularContactModel,
    v_a: Vec3,
    v_b: Vec3,
) -> (f64, f64, f64) {
    let particles = vec![
        sphere(Vec3::new(-0.0060, 0.0, 0.0), v_a),
        sphere(Vec3::new(0.0060, 0.0, 0.0), v_b),
    ];
    let mut sys = GranularSystem::new(particles, vec![], model, Vec3::zero(), 1.0e-6)
        .expect("valid system");

    let (mut dx, mut dv, mut dw) = (0.0_f64, 0.0_f64, 0.0_f64);
    let mut last = 0_usize;
    for row in rows {
        let step = row[0] as usize;
        sys.run(step - last);
        last = step;
        // Columns: step, then 9 per atom (x y z vx vy vz wx wy wz).
        for (index, base) in [(0_usize, 1_usize), (1, 10)] {
            let p = sys.particles()[index];
            for (got, want) in [
                (p.position.x, row[base]),
                (p.position.y, row[base + 1]),
                (p.position.z, row[base + 2]),
            ] {
                dx = dx.max((got - want).abs());
            }
            for (got, want) in [
                (p.velocity.x, row[base + 3]),
                (p.velocity.y, row[base + 4]),
                (p.velocity.z, row[base + 5]),
            ] {
                dv = dv.max((got - want).abs());
            }
            for (got, want) in [
                (p.angular_velocity.x, row[base + 6]),
                (p.angular_velocity.y, row[base + 7]),
                (p.angular_velocity.z, row[base + 8]),
            ] {
                dw = dw.max((got - want).abs());
            }
        }
    }
    (dx, dv, dw)
}

/// Head-on binary collision, Hertz normal model.
///
/// **Result (2026-09-15).** Bit-identical to LIGGGHTS across all 251 frames:
/// `max|Δx| = 0 m`, `max|Δv| = 0 m/s`, `max|Δω| = 0 rad/s`. Realised
/// restitution `0.900007` in both codes.
#[test]
fn headon_hertz_matches_liggghts() {
    let Some(rows) = load("headon_hertz.csv") else {
        eprintln!("skipping: reference-data/liggghts/ not present");
        return;
    };
    assert_eq!(rows.len(), 251, "unexpected reference frame count");
    let (dx, dv, dw) = replay_pair(
        &rows,
        GranularContactModel::hertz_history(material()),
        Vec3::new(1.0, 0.0, 0.0),
        Vec3::new(-1.0, 0.0, 0.0),
    );
    assert!(dx <= 1e-15, "position drift {dx:e} m vs LIGGGHTS");
    assert!(dv <= 1e-13, "velocity drift {dv:e} m/s vs LIGGGHTS");
    assert!(dw <= 1e-11, "spin drift {dw:e} rad/s vs LIGGGHTS");
}

/// Head-on binary collision, Hooke (linearised) normal model,
/// `characteristicVelocity = 2 m/s`.
///
/// **Result (2026-09-15).** Bit-identical across all 251 frames. Realised
/// restitution `0.899972` in both codes — note this differs from the Hertz
/// case's `0.900007`, and both codes reproduce their own model's value, which
/// is the point: the agreement is not an artefact of both landing on the
/// requested `0.9`.
#[test]
fn headon_hooke_matches_liggghts() {
    let Some(rows) = load("headon_hooke.csv") else {
        eprintln!("skipping: reference-data/liggghts/ not present");
        return;
    };
    let model = GranularContactModel::new(
        GranularNormalModel::hooke(material(), 2.0).expect("valid hooke"),
        TangentialModel::History,
    );
    let (dx, dv, dw) = replay_pair(
        &rows,
        model,
        Vec3::new(1.0, 0.0, 0.0),
        Vec3::new(-1.0, 0.0, 0.0),
    );
    assert!(dx <= 1e-15, "position drift {dx:e} m vs LIGGGHTS");
    assert!(dv <= 1e-13, "velocity drift {dv:e} m/s vs LIGGGHTS");
    assert!(dw <= 1e-11, "spin drift {dw:e} rad/s vs LIGGGHTS");
}

/// Oblique collision with Coulomb friction — the case that exercises the
/// **tangential shear-history spring**, the slip rescaling, and the contact
/// torque that spins the particles up.
///
/// **Result (2026-09-15).** `max|Δx| = 0 m`, `max|Δv| = 1.11e-16 m/s` (one ulp
/// at `|v| ≈ 1`), `max|Δω| = 5.68e-14 rad/s` (~3 ulp at the final
/// `ω_z = −83.1815 rad/s`). Final state `v = (−0.751987, 0.577628, 0)` m/s and
/// `ω_z = −83.1815 rad/s` in both codes.
///
/// This is the test that would fail if the shear history, the `c_r = r − δ/2`
/// contact radii, or the "damping only while sticking" branch were wrong — all
/// three are places where the stateless `crate::contact` module diverges from
/// upstream.
#[test]
fn oblique_friction_matches_liggghts() {
    let Some(rows) = load("oblique_hertz.csv") else {
        eprintln!("skipping: reference-data/liggghts/ not present");
        return;
    };
    let (dx, dv, dw) = replay_pair(
        &rows,
        GranularContactModel::hertz_history(material()),
        Vec3::new(1.0, 0.5, 0.0),
        Vec3::new(-1.0, -0.5, 0.0),
    );
    assert!(dx <= 1e-15, "position drift {dx:e} m vs LIGGGHTS");
    assert!(dv <= 1e-13, "velocity drift {dv:e} m/s vs LIGGGHTS");
    assert!(dw <= 1e-11, "spin drift {dw:e} rad/s vs LIGGGHTS");
    // The case must actually have spun the particles up, or it proves nothing.
    let final_wz = rows.last().expect("frames")[9];
    assert!(
        final_wz.abs() > 10.0,
        "reference case should spin the particle up, got wz = {final_wz}"
    );
}

/// Sphere dropped onto a primitive `zplane` wall under gravity, 400 000 steps —
/// repeated bounces, so it exercises contact *make and break* and therefore the
/// shear-history lifecycle, as well as the wall branch's `R* = r`, `m* = m`.
///
/// **Result (2026-09-15).** Bit-identical to LIGGGHTS across all 2001 sampled
/// frames (`max|Δz| = 0 m`, `max|Δv_z| = 0 m/s`). Final height
/// `z = 0.013708858 m` in both codes.
#[test]
fn wall_bounce_matches_liggghts() {
    let Some(rows) = load("wall_bounce.csv") else {
        eprintln!("skipping: reference-data/liggghts/ not present");
        return;
    };
    assert_eq!(rows.len(), 2001, "unexpected reference frame count");
    let floor = Boundary::wall(Vec3::zero(), Vec3::new(0.0, 0.0, 1.0)).expect("valid wall");
    let mut sys = GranularSystem::new(
        vec![sphere(Vec3::new(0.0, 0.0, 0.1), Vec3::zero())],
        vec![floor],
        GranularContactModel::hertz_history(material()),
        Vec3::new(0.0, 0.0, -9.81),
        1.0e-6,
    )
    .expect("valid system");

    let (mut dz, mut dv) = (0.0_f64, 0.0_f64);
    let mut last = 0_usize;
    let mut bounced = false;
    for row in &rows {
        let step = row[0] as usize;
        sys.run(step - last);
        last = step;
        let p = sys.particles()[0];
        dz = dz.max((p.position.z - row[3]).abs());
        dv = dv.max((p.velocity.z - row[6]).abs());
        if p.velocity.z > 0.1 {
            bounced = true;
        }
    }
    assert!(bounced, "the reference case must actually bounce");
    assert!(dz <= 1e-15, "height drift {dz:e} m vs LIGGGHTS");
    assert!(dv <= 1e-13, "velocity drift {dv:e} m/s vs LIGGGHTS");
}
