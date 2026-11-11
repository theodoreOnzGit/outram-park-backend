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

//! # `fix heat/gran/conduction` against `heat_gran` (gh:#1412, gh:#1406 prio 3)
//!
//! ## Methodology
//!
//! LIGGGHTS-PUBLIC `3d5c00f2` (built per `upstream_source/README.md`, dump
//! precision patched to `%.17g`) runs two decks; `dump2heat.sh` reduces the
//! dumps to `step,id,T,heatFlux`, columns copied as printed:
//!
//! 1. `in.heat_pair`: two `d = 10 mm` spheres (`E = 1e7`, `nu = 0.3`,
//!    `e = 0.9`, `mu = 0.5`, `rho = 2500`) at `x = +-4.8 mm`, 0.4 mm overlap, at
//!    rest, `T = 400 K` / `300 K`, `k = 50 W/m/K`, `cp = 500 J/kg/K`,
//!    `dt = 1 us`, 2500 steps, sampled every 10 (251 frames x 2 atoms x 2
//!    quantities). `contact_area overlap` (default).
//! 2. `in.heat_bed`: the committed settled `D/d = 6` bed (354 pebbles, cylinder
//!    + floor + gravity, as `in.pgl_pebble`), `k = 50`, `cp = 10`, initial
//!    `T = 300 + 200 z/0.35` per pebble (read back from the step-0 dump, so the
//!    variable's arithmetic is not re-derived), 100 steps, every step dumped;
//!    steps 0, 1, 10, 100 committed (`heat_bed.csv`).
//!
//! The port builds the same systems with `GranularSystem::with_heat_conduction`
//! (and, for the bed, `with_liggghts_neighbor_order`, so the flux sum order is
//! upstream's) and the T and heat flux of every atom are compared on their bit
//! patterns.
//!
//! ## Prediction (written 2026-11-11 before the run)
//!
//! * Pair: **bit-identical** in all 251 frames (one contact, no sum order; the
//!   area, `hc`, flux and update formulas are translated operation by
//!   operation).
//! * Bed: **bit-identical** at steps 0, 1, 10, 100 with the LIGGGHTS neighbour
//!   order (the heat flux of a pebble is a sum over its contacts in list
//!   order; the mechanical trajectory is bit-identical per the
//!   `pair_gran_local` test). With the default cell-grid order the sums differ
//!   at ulp level: reported as an ablation.
//! * The clean-room [`crate::thermal`] model: its conductance is
//!   `2 k_s a_c` with the Hertz radius `a_c = sqrt(R* delta)`; upstream's is
//!   `2 sqrt(pi) k_s a` with the geometric chord radius `a` (`a^2 ~ 2 R*
//!   delta`), so the ratio is predicted to be `sqrt(2 pi) = 2.507` to within
//!   "the second-order overlap terms (< 1 %)". **That tolerance was wrong**: the
//!   measured ratio is 2.4813, 1.01 % low, because the exact chord radius is
//!   `a^2 = r delta - delta^2/4`, giving `ratio = sqrt(2 pi (1 - delta/(4 r)))`
//!   (`delta/(4r) = 2 %` at 0.4 mm overlap on `r = 5 mm`). The test asserts the
//!   closed form to 2e-4, derived independently of the code under test; the
//!   original sentence is kept here, struck through in intent, so the miss is on
//!   record.
//!
//! ## Result
//!
//! Recorded by the test (`--nocapture`) and in
//! `docs/cross-code-summary.md`.

use outram_park_fork_liggghts::boundary::Boundary;
use outram_park_fork_liggghts::granular::{GranularContactModel, GranularMaterial};
use outram_park_fork_liggghts::granular_system::GranularSystem;
use outram_park_fork_liggghts::heat_gran::HeatGranConduction;
use outram_park_fork_liggghts::neighbor_list::LiggghtsNeighbor;
use outram_park_fork_liggghts::particle::{Particle, Vec3};
use outram_park_fork_liggghts::thermal::{hertzian_contact_radius, ThermalModel};
use std::collections::HashMap;
use uom::si::f64::{Length, Mass, ThermodynamicTemperature};
use uom::si::{length::meter, mass::kilogram, thermodynamic_temperature::kelvin};

fn read(name: &str) -> Option<String> {
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../reference-data/liggghts")
        .join(name);
    std::fs::read_to_string(path).ok()
}

/// `step -> id -> (T, flux)` from a `dump2heat.sh` CSV.
fn parse(csv: &str) -> HashMap<usize, HashMap<usize, (f64, f64)>> {
    let mut out: HashMap<usize, HashMap<usize, (f64, f64)>> = HashMap::new();
    for l in csv.lines().skip(1).filter(|l| !l.trim().is_empty()) {
        let v: Vec<&str> = l.split(',').collect();
        out.entry(v[0].parse().unwrap())
            .or_default()
            .insert(v[1].parse().unwrap(), (v[2].parse().unwrap(), v[3].parse().unwrap()));
    }
    out
}

fn sphere(pos: Vec3, vel: Vec3, r: f64, rho: f64, t: f64) -> Particle {
    let mass = 4.0 * std::f64::consts::PI / 3.0 * r * r * r * rho;
    Particle::new(
        pos,
        vel,
        Vec3::zero(),
        Mass::new::<kilogram>(mass),
        Length::new::<meter>(r),
        ThermodynamicTemperature::new::<kelvin>(t),
    )
    .unwrap()
}

fn pair_system() -> GranularSystem {
    let material = GranularMaterial::new(1.0e7, 0.3, 0.9, 0.5).unwrap();
    GranularSystem::new(
        vec![
            sphere(Vec3::new(-0.0048, 0.0, 0.0), Vec3::zero(), 0.005, 2500.0, 400.0),
            sphere(Vec3::new(0.0048, 0.0, 0.0), Vec3::zero(), 0.005, 2500.0, 300.0),
        ],
        vec![],
        GranularContactModel::hertz_history(material),
        Vec3::zero(),
        1e-6,
    )
    .unwrap()
    .with_heat_conduction(HeatGranConduction::single_type(50.0, 500.0).unwrap())
    .unwrap()
}

/// Two touching spheres at `DeltaT`: every frame's T and heat flux of both
/// atoms, bit patterns.
#[test]
fn two_spheres_match_upstream() {
    let Some(csv) = read("heat_pair.csv") else {
        eprintln!("skipping: reference-data/liggghts/heat_pair.csv not present");
        return;
    };
    let want = parse(&csv);
    let mut sys = pair_system();
    let (mut total, mut same) = (0usize, 0usize);
    let mut flux10 = 0.0;
    let mut frames: Vec<usize> = want.keys().copied().collect();
    frames.sort_unstable();
    let mut done = 0usize;
    for step in frames {
        sys.run(step - done);
        done = step;
        for id in 1..=2usize {
            let (t, q) = want[&step][&id];
            let (ot, oq) = (sys.particles()[id - 1].temperature, sys.heat_flux()[id - 1]);
            total += 2;
            same += usize::from(ot.to_bits() == t.to_bits());
            same += usize::from(oq.to_bits() == q.to_bits());
        }
        if step == 10 {
            flux10 = sys.heat_flux()[0];
        }
    }
    eprintln!("heat pair: {same}/{total} values bit-identical over {} frames", total / 4);
    assert_eq!(same, total);

    // The clean-room model on the same contact at step 10: its conductance
    // against upstream's (see the module docs).
    // upstream's heat flux was evaluated at the state of step 10's force
    // evaluation, i.e. before the temperature update of step 10, so use the
    // temperatures of step 9.
    let mut s9 = pair_system();
    s9.run(9);
    let (a9, b9) = (s9.particles()[0], s9.particles()[1]);
    let delta9 = a9.radius + b9.radius - a9.position.sub(b9.position).norm();
    let ac = hertzian_contact_radius(a9.radius, b9.radius, delta9).unwrap();
    // Particle 0 is the hot one: heat rate INTO particle 0 from particle 1.
    let model = ThermalModel::ContactConduction { k_i: 50.0, k_j: 50.0 };
    let q_clean =
        outram_park_fork_liggghts::thermal::particle_pair_heat_rate(&model, ac, &a9, &b9).unwrap();
    let ratio = flux10 / q_clean;
    // Closed form of the ratio: upstream hc = 2 sqrt(pi) k a with the exact
    // chord radius a^2 = r delta - delta^2/4 (r = sphere radius), clean-room
    // hc = 2 k a_c with a_c^2 = R* delta = r delta/2, so
    // ratio = sqrt(2 pi (1 - delta/(4 r))).
    let r = a9.radius;
    let closed = (2.0 * std::f64::consts::PI * (1.0 - delta9 / (4.0 * r))).sqrt();
    eprintln!(
        "heat pair, step 10: upstream flux {flux10:.6e} W, clean-room {q_clean:.6e} W, ratio {ratio:.5}; \
         closed form sqrt(2 pi (1 - delta/4r)) = {closed:.5} (leading term sqrt(2 pi) = {:.5}, \
         delta = {delta9:.3e} m)",
        (2.0 * std::f64::consts::PI).sqrt()
    );
    assert!((ratio / closed - 1.0).abs() < 2e-4, "ratio {ratio} vs closed form {closed}");
}

/// The settled bed with a temperature gradient: T and heat flux of all 354
/// pebbles at steps 0, 1, 10, 100, with LIGGGHTS' neighbour order (and, as an
/// ablation, the default cell-grid order).
#[test]
fn bed_matches_upstream() {
    let (Some(bed), Some(csv)) = (read("pebble_bed_settled.csv"), read("heat_bed.csv")) else {
        eprintln!("skipping: reference-data/liggghts/ heat_bed.csv or pebble_bed_settled.csv not present");
        return;
    };
    let want = parse(&csv);
    let r = 0.005;
    let t0 = &want[&0];
    let particles: Vec<Particle> = bed
        .lines()
        .skip(1)
        .filter(|l| !l.trim().is_empty())
        .enumerate()
        .map(|(n, l)| {
            let v: Vec<f64> = l.split(',').map(|x| x.parse().unwrap()).collect();
            assert_eq!(v[0] as usize, n + 1);
            sphere(Vec3::new(v[1], v[2], v[3]), Vec3::new(v[4], v[5], v[6]), r, 2500.0, t0[&(n + 1)].0)
        })
        .collect();
    let build = |lig: bool| {
        let material = GranularMaterial::new(5.0e6, 0.3, 0.5, 0.3).unwrap();
        let sys = GranularSystem::new(
            particles.clone(),
            vec![
                Boundary::cylinder(Vec3::zero(), Vec3::new(0.0, 0.0, 1.0), 0.030).unwrap(),
                Boundary::wall(Vec3::zero(), Vec3::new(0.0, 0.0, 1.0)).unwrap(),
            ],
            GranularContactModel::hertz_history(material),
            Vec3::new(0.0, 0.0, -9.81),
            5e-6,
        )
        .unwrap();
        let sys = if lig {
            sys.with_liggghts_neighbor_order(
                LiggghtsNeighbor::new(Vec3::new(-1.0, -1.0, -0.005), Vec3::new(1.0, 1.0, 0.35), 0.003)
                    .unwrap(),
            )
        } else {
            sys
        };
        sys.with_heat_conduction(HeatGranConduction::single_type(50.0, 10.0).unwrap()).unwrap()
    };
    for (label, lig) in [("liggghts order", true), ("cell-grid order", false)] {
        let mut sys = build(lig);
        let mut done = 0usize;
        let mut bad = Vec::new();
        let mut worst_rel = 0.0_f64;
        for step in [0usize, 1, 10, 100] {
            sys.run(step - done);
            done = step;
            let mut n_bad = 0usize;
            for id in 1..=354usize {
                let (t, q) = want[&step][&id];
                let (ot, oq) = (sys.particles()[id - 1].temperature, sys.heat_flux()[id - 1]);
                n_bad += usize::from(ot.to_bits() != t.to_bits());
                n_bad += usize::from(oq.to_bits() != q.to_bits());
                worst_rel = worst_rel.max(((ot - t) / t).abs());
                if q != 0.0 {
                    worst_rel = worst_rel.max(((oq - q) / q).abs());
                }
            }
            bad.push(n_bad);
        }
        eprintln!(
            "heat bed ({label}): differing values of 708 at steps 0,1,10,100 = {bad:?}, worst relative {worst_rel:e}"
        );
        if lig {
            assert_eq!(bad, vec![0, 0, 0, 0]);
        } else {
            assert!(worst_rel < 1e-8, "cell-grid order: worst relative {worst_rel:e}");
        }
    }
}
