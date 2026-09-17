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

//! # HTR-10 timestep cost, and a bitwise reproducibility check
//!
//! Runs 200 DEM steps on the settled HTR-10 bed (27 554 pebbles) and reports
//! **ms per step** plus a **bitwise checksum** of the whole configuration.
//! Two instruments in one binary, because both answer questions about the same
//! 200 steps.
//!
//! ```text
//! cargo run --release -p outram-park-fork-liggghts --example htr10_step_profile -- serial
//! cargo run --release -p outram-park-fork-liggghts --example htr10_step_profile -- 12
//! ```
//!
//! The argument selects the backend: `serial` for
//! [`ComputeType::CpuSingleThread`], `gpu` for [`ComputeType::Gpu`] (which runs
//! the CPU path — the timestep has no GPU kernel, see [`crate::compute`]), or a
//! thread count for [`ComputeType::CpuMultiThread`].
//!
//! ## The checksum is the point, not an extra
//!
//! It XOR-folds every position and velocity component's raw IEEE-754 bits, so
//! **any** last-ulp difference anywhere in the bed changes it. That makes it a
//! direct test of two properties this crate depends on:
//!
//! 1. **Run-to-run reproducibility.** Running twice with the same argument must
//!    give the same checksum. This is how the neighbour-grid defect was found:
//!    three identical runs gave kinetic energies `2.81519841188424304e-2`,
//!    `…18857e-2` and `…32977e-2`, because the cell grid was a randomly-seeded
//!    `HashMap`. Fixed; see V&V § 3.3 and bead `op-t3l.9`.
//! 2. **Backend equivalence.** `serial` and any thread count must give the
//!    *same* checksum. The parallel force loop is required to be bit-identical,
//!    not merely close — see [`crate::compute`] for why that is a requirement
//!    here rather than a nicety.
//!
//! Measured 2026-09-17 on a 12-core host, all four runs `45913cc9e4c58660`:
//! 18.34 ms/step scalar, 12.85 ms/step on 12 threads (55.95 ms/step before the
//! defect fixes of V&V § 3.3).
use outram_park_fork_liggghts::boundary::Boundary;
use outram_park_fork_liggghts::compute::{ComputeType, ThreadCount};
use outram_park_fork_liggghts::granular::{GranularContactModel, GranularMaterial, RollingModel};
use outram_park_fork_liggghts::granular_system::GranularSystem;
use outram_park_fork_liggghts::particle::{Particle, Vec3};
use uom::si::f64::{Length, Mass, ThermodynamicTemperature};
use uom::si::{length::meter, mass::kilogram, thermodynamic_temperature::kelvin};

fn main() {
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../reference-data/liggghts/htr10_settled_ours.csv");
    let text = std::fs::read_to_string(&path).expect("settled bed");
    let mut rows: Vec<(usize, Vec3)> = text
        .lines()
        .skip(1)
        .filter(|l| !l.trim().is_empty())
        .map(|l| {
            let f: Vec<f64> = l.split(',').map(|v| v.parse().expect("num")).collect();
            (f[0] as usize, Vec3::new(f[1], f[2], f[3]))
        })
        .collect();
    rows.sort_by_key(|r| r.0);
    let rho: f64 = 1730.0;
    let r_p: f64 = 0.03;
    let m = rho * 4.0 / 3.0 * std::f64::consts::PI * r_p.powi(3);
    let particles: Vec<Particle> = rows
        .iter()
        .map(|(_, x)| {
            Particle::new(
                *x,
                Vec3::zero(),
                Vec3::zero(),
                Mass::new::<kilogram>(m),
                Length::new::<meter>(r_p),
                ThermodynamicTemperature::new::<kelvin>(300.0),
            )
            .expect("pebble")
        })
        .collect();
    let n = particles.len();
    let material = GranularMaterial::new(5.0e8, 0.2, 0.5, 0.4).expect("mat");
    let model = GranularContactModel::hertz_history(material)
        .with_rolling(RollingModel::cdt(0.1).expect("mu_r"));
    let boundaries = vec![
        Boundary::cylinder(Vec3::zero(), Vec3::new(0.0, 0.0, 1.0), 0.90).expect("barrel"),
        Boundary::wall(Vec3::zero(), Vec3::new(0.0, 0.0, 1.0)).expect("floor"),
    ];
    let backend = match std::env::args().nth(1).as_deref() {
        Some("serial") | None => ComputeType::CpuSingleThread,
        Some("gpu") => ComputeType::Gpu,
        Some(n) => ComputeType::CpuMultiThread(ThreadCount::Fixed(
            n.parse().expect("thread count"),
        )),
    };
    let mut sys = GranularSystem::new(particles, boundaries, model, Vec3::new(0.0, 0.0, -9.81), 3.5e-5)
        .expect("system")
        .with_compute(backend);
    println!("backend {backend:?} -> {} thread(s)", backend.threads());

    let t0 = std::time::Instant::now();
    const STEPS: usize = 200;
    sys.run(STEPS);
    let dt = t0.elapsed().as_secs_f64();
    // Bitwise checksum of the whole configuration: XOR of every coordinate's
    // raw IEEE-754 bits. Any last-ulp difference anywhere changes it.
    let mut h: u64 = 0;
    for p in sys.particles() {
        for v in [p.position.x, p.position.y, p.position.z,
                  p.velocity.x, p.velocity.y, p.velocity.z] {
            h ^= v.to_bits();
            h = h.rotate_left(7);
        }
    }
    let ke = sys.kinetic_energy();
    println!(
        "N = {n};  {STEPS} steps in {dt:.3} s  =>  {:.2} ms/step,  {:.1} steps/s",
        1000.0 * dt / STEPS as f64,
        STEPS as f64 / dt
    );
    println!("CHECKSUM {h:016x}  KE {ke:.17e}");
}
