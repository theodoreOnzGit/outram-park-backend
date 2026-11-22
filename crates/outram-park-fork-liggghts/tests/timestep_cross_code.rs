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

//! # `timestep.rs` against upstream `fix check/timestep/gran`
//!
//! gh:#1406 prio-2 row "fix check/timestep/gran on the HTR-10 and D/d = 6 decks".
//!
//! ## Methodology
//!
//! Decks `reference-data/liggghts/in.timestep_pebble` (354 pebbles, `d = 10 mm`,
//! `rho = 2500`, `E = 5e6`, `nu = 0.3`, `dt = 5 us`, skin `3 mm`) and
//! `in.timestep_htr10` (27 554 pebbles, `d = 60 mm`, `rho = 1730`, `E = 5e8`,
//! `nu = 0.2`, `dt = 35 us`, skin `18 mm`) load the committed settled beds
//! (`csv2data.sh`), attach `fix check/timestep/gran 1 0.1 0.1` and run one
//! step with no walls and no gravity; `f_ts[1..3]` (Rayleigh, Hertz and skin
//! fractions) are printed at 17 digits at step 1
//! (`reference-data/liggghts/timestep_check.txt`). This test builds the same
//! beds in a `GranularSystem` (same material, no walls, zero gravity), runs
//! one step and evaluates [`timestep::estimate`] and the fractions.
//!
//! ## Prediction (written before the run, 2026-11-11)
//!
//! Rayleigh fraction: **bit-identical** (it depends only on `r`, `rho`, `E`,
//! `nu`). Hertz and skin fractions depend on the fastest particle's speed after
//! one step, i.e. on pair-force bit-identity for that particle: bit-identical
//! if the one-step velocities are, otherwise within a few ulp.
//!
//! ## Result
//!
//! **All six values bit-identical** (2026-11-11, Rust 1.99.1): pebble bed
//! Rayleigh `8.17088224384693654e-3`, Hertz `9.95311260696870150e-4`, skin
//! `9.21176436274946973e-6`; HTR-10 `1.17171564017016352e-1`,
//! `1.67451010566499262e-2`, `3.57167396650032946e-4`. The first run, before the
//! set-up shear fix (gh:#1418: the port advanced the shear history during the
//! set-up force evaluation, upstream does not), gave Rayleigh bit-identical but
//! Hertz `5.3e-10` and skin `2.65e-9` off on the pebble bed, because one extra
//! `v_tr*dt` of shear changed the fastest particle's one-step velocity. That
//! prediction (\"bit-identical if the one-step velocities are\") held once the
//! defect was fixed.

use outram_park_fork_liggghts::granular::{GranularContactModel, GranularMaterial};
use outram_park_fork_liggghts::granular_system::GranularSystem;
use outram_park_fork_liggghts::particle::{Particle, Vec3};
use outram_park_fork_liggghts::timestep;
use uom::si::f64::{Length, Mass, ThermodynamicTemperature};
use uom::si::{length::meter, mass::kilogram, thermodynamic_temperature::kelvin};

fn read(name: &str) -> Option<String> {
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../reference-data/liggghts")
        .join(name);
    std::fs::read_to_string(path).ok()
}

struct Case {
    name: &'static str,
    csv: &'static str,
    d: f64,
    rho: f64,
    e: f64,
    nu: f64,
    dt: f64,
    skin: f64,
}

fn run_case(c: &Case, reference: &str) {
    let Some(csv) = read(c.csv) else {
        eprintln!("skipping {}: reference data not present", c.name);
        return;
    };
    let r = c.d / 2.0;
    // `set atom * diameter d density rho`: m = 4/3 pi r^3 rho
    let mass = 4.0 * std::f64::consts::PI / 3.0 * r * r * r * c.rho;
    let particles: Vec<Particle> = csv
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
    let material = GranularMaterial::new(c.e, c.nu, 0.5, 0.3).unwrap();
    let mut sys = GranularSystem::new(
        particles,
        vec![],
        GranularContactModel::hertz_history(material),
        Vec3::zero(),
        c.dt,
    )
    .unwrap();
    sys.run(1);
    let est = timestep::estimate(sys.particles(), c.rho, &material).unwrap();
    let ours = [
        est.rayleigh_fraction(c.dt),
        est.hertz_fraction(c.dt),
        est.skin_fraction(c.dt, c.skin),
    ];
    let line = reference
        .lines()
        .find(|l| l.starts_with(c.name))
        .expect("reference line");
    let theirs: Vec<f64> = line
        .split_whitespace()
        .skip(1)
        .map(|x| x.parse().unwrap())
        .collect();
    for (k, (a, b)) in ours.iter().zip(&theirs).enumerate() {
        let rel = ((a - b) / b).abs();
        eprintln!(
            "{} fraction[{k}]: ours {a:.17e} upstream {b:.17e} bits equal {} rel {rel:e}",
            c.name,
            a.to_bits() == b.to_bits()
        );
    }
    for k in 0..3 {
        assert_eq!(
            ours[k].to_bits(),
            theirs[k].to_bits(),
            "{} fraction[{k}]",
            c.name
        );
    }
}

/// D/d = 6 bed and HTR-10 bed: Rayleigh, Hertz and skin fractions against
/// upstream's printed `f_ts[1..3]`.
#[test]
fn timestep_fractions_match_upstream() {
    let Some(reference) = read("timestep_check.txt") else {
        return;
    };
    run_case(
        &Case {
            name: "pebble",
            csv: "pebble_bed_settled.csv",
            d: 0.01,
            rho: 2500.0,
            e: 5.0e6,
            nu: 0.3,
            dt: 5e-6,
            skin: 0.003,
        },
        &reference,
    );
    run_case(
        &Case {
            name: "htr10",
            csv: "htr10_settled.csv",
            d: 0.06,
            rho: 1730.0,
            e: 5.0e8,
            nu: 0.2,
            dt: 3.5e-5,
            skin: 0.018,
        },
        &reference,
    );
}
