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

//! # Cross-code verification of the stateless path (`contact` + `simulation`)
//!
//! The tests in `liggghts_cross_code.rs` verify [`crate::granular`], which
//! carries shear history. This file verifies the **other** engine — the
//! stateless [`ContactModel`] driven by [`DemSimulation`] — against the
//! upstream model it actually corresponds to: `pair_style gran model hertz
//! **tangential no_history**`.
//!
//! ## Why this file exists
//!
//! Before 2026-09-16 the stateless path was recorded as "divergent from
//! upstream" and left that way. Two of the three recorded divergences did not
//! survive checking:
//!
//! - **The contact-radius lever arm was a real defect** and is fixed:
//!   `contact.rs` now uses `c_r = r − δ_n/2` for both the surface-velocity
//!   moment and the torque arm, as `surface_model_default.h` does.
//! - **The integrator was a real defect** and is fixed: [`DemSimulation`] now
//!   runs [`crate::integrator::VelocityVerlet`] rather than the
//!   non-symplectic single-shot update.
//! - **The "damping branch" divergence was a mis-reading on my part.** It was
//!   recorded by comparing `contact.rs` against upstream's *history* model.
//!   Against `tangential_model_no_history.h` — the model it actually
//!   implements — upstream also caps the damping coefficient
//!   (`γ = min(γ_t, µ|F_n|/v_rel)`), which is exactly what `contact.rs` does.
//!   There was never a defect there.
//!
//! With the two real defects fixed, this path is no longer "known-divergent";
//! it is verified, and this file is what holds it to that.
//!
//! ## Methodology
//!
//! Same generation route as the other cross-code file: upstream LIGGGHTS
//! compiled and run, full `%.17g` dump precision, every sampled frame
//! compared. Case: oblique collision with friction, `µ = 0.5`, `e = 0.9`,
//! `E = 10 MPa`, `ν = 0.3`, `d = 10 mm`, `dt = 1 µs`, 2500 steps.
//! Input: `reference-data/liggghts/in.oblique_nohist`.
//!
//! ## Results (2026-09-16)
//!
//! | Quantity | max deviation over 251 frames |
//! |---|---|
//! | position | `0 m` |
//! | velocity | `1.11e-16 m/s` (1 ulp at `|v| ≈ 1`) |
//! | angular velocity | `1.42e-14 rad/s` (≈3 ulp at `ω_z ≈ 41.6`) |
//!
//! Agreement to floating-point round-off — the residual is the expected
//! consequence of summing identical terms in a different association order,
//! the same signature the history-model oblique case shows. Final state in
//! both codes:
//! `v = (−0.72176686213860586, 0.68189877446166391, 0) m/s`,
//! `ω_z = −41.624058372713392 rad/s`.
//!
//! Note the no-history spin (`−41.62 rad/s`) is about half the history model's
//! (`−83.18 rad/s`) on the same impact — the shear spring does real work, which
//! is why a packed bed needs it.

use outram_park_fork_liggghts::contact::{ContactModel, HertzContact};
use outram_park_fork_liggghts::particle::{Particle, Vec3};
use outram_park_fork_liggghts::simulation::DemSimulation;
use uom::si::f64::{Length, Mass, Pressure, ThermodynamicTemperature};
use uom::si::{length::meter, mass::kilogram, pressure::pascal, thermodynamic_temperature::kelvin};

fn sphere(position: Vec3, velocity: Vec3) -> Particle {
    let mass = 2500.0 * std::f64::consts::PI / 6.0 * 1.0e-6;
    Particle::new(
        position,
        velocity,
        Vec3::zero(),
        Mass::new::<kilogram>(mass),
        Length::new::<meter>(0.005),
        ThermodynamicTemperature::new::<kelvin>(300.0),
    )
    .expect("valid pebble")
}

fn load(name: &str) -> Option<Vec<Vec<f64>>> {
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../reference-data/liggghts")
        .join(name);
    let text = std::fs::read_to_string(path).ok()?;
    Some(
        text.lines()
            .skip(1)
            .filter(|l| !l.trim().is_empty())
            .map(|l| {
                l.split(',')
                    .map(|v| v.parse().expect("numeric csv"))
                    .collect()
            })
            .collect(),
    )
}

/// The stateless `contact` + `simulation` path against upstream
/// `tangential no_history`.
///
/// See the module docs for methodology and the 2026-09-16 measured result
/// (round-off agreement across all 251 frames: `max|Δv| = 1.11e-16 m/s`,
/// `max|Δω| = 1.42e-14 rad/s`, positions exact).
#[test]
fn stateless_path_matches_liggghts_no_history() {
    let Some(rows) = load("oblique_nohist.csv") else {
        eprintln!("skipping: reference-data/liggghts/ not present");
        return;
    };
    assert_eq!(rows.len(), 251, "unexpected reference frame count");

    let hertz = HertzContact::new(Pressure::new::<pascal>(1.0e7), 0.3, 0.9, 0.5)
        .expect("valid hertz model");
    let particles = vec![
        sphere(Vec3::new(-0.0060, 0.0, 0.0), Vec3::new(1.0, 0.5, 0.0)),
        sphere(Vec3::new(0.0060, 0.0, 0.0), Vec3::new(-1.0, -0.5, 0.0)),
    ];
    let mut sim = DemSimulation::new(
        particles,
        vec![],
        ContactModel::Hertz(hertz),
        Vec3::zero(),
        1.0e-6,
    )
    .expect("valid simulation");

    let (mut dx, mut dv, mut dw) = (0.0_f64, 0.0_f64, 0.0_f64);
    let mut last = 0_usize;
    for row in &rows {
        let step = row[0] as usize;
        for _ in last..step {
            sim.step();
        }
        last = step;
        for (index, base) in [(0_usize, 1_usize), (1, 10)] {
            let p = sim.particles()[index];
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
    eprintln!("stateless path vs LIGGGHTS no_history: dx={dx:e} dv={dv:e} dw={dw:e}");
    assert!(dx <= 1e-15, "position drift {dx:e} m vs LIGGGHTS");
    assert!(dv <= 1e-13, "velocity drift {dv:e} m/s vs LIGGGHTS");
    assert!(dw <= 1e-11, "spin drift {dw:e} rad/s vs LIGGGHTS");

    // The case must actually spin the particles up, or it proves nothing about
    // the contact radii or the torque arm.
    let final_wz = rows.last().expect("frames")[9];
    assert!(
        final_wz.abs() > 10.0,
        "reference case should spin the particle up, got wz = {final_wz}"
    );
}
