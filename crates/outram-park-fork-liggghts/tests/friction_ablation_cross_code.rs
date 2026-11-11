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

//! # Friction ablation (V&V 4.9 cells) run in BOTH codes on the `D/d = 6` bed
//! (gh:#1406 prio 2)
//!
//! ## What the committed `htr10_*_mu*_mur*.csv` files are
//!
//! Checked cell by cell (2026-11-11): `reference-data/liggghts/README.md` lists
//! `htr10_conus_presettled_<label>.csv`, `htr10_phi_vs_step_<label>.csv` and
//! `htr10_recirc_<label>*.csv` as **this port's own output**. Of the HTR-10
//! friction cells (`mu10_mur00`, `mu10_mur10`, `mu40_mur00`, `mu40_mur10`, and
//! the extra `mu125`..`mu200` sweep), only **`mu40_mur10`** has a LIGGGHTS
//! counterpart: `in.htr10_conus` runs `mu = 0.4`, `mu_r = 0.1` (CDT), giving
//! `htr10_conus_settled_liggghts.csv`. The other three cells were **never run
//! in LIGGGHTS**. The HTR-10 conus slump is ~10 min of port time per cell and
//! more for LIGGGHTS with one core, so per the task the ablation is run on the
//! `D/d = 6` bed instead (this file), and the HTR-10 cells stay port-only; the
//! V&V document says so.
//!
//! ## Methodology
//!
//! The 354-pebble bed as inserted (`pebble_bed_init.csv`) is settled for
//! 400 000 steps (`dt = 5 us`, 2 s) in both codes at the four cells
//! `mu in {0.1, 0.4}` x `mu_r in {0, 0.1}` (`mu_r = 0` without a rolling
//! model, `mu_r = 0.1` CDT), Hertz + history, `E = 5e6`, `nu = 0.3`, `e = 0.5`,
//! cylinder `R = 30 mm` + floor + gravity. LIGGGHTS: `in.ablate` /
//! `in.ablate_cdt` through `run_ablate.sh`, frames every 50 000 steps in
//! `ablate_<cell>.csv` (wide trajectory CSV: x, y, z, v, omega of every
//! pebble). The port runs the same with
//! `GranularSystem::with_liggghts_neighbor_order` (the deck's box and skin) and
//! every frame is compared on bit patterns. The packing fraction `phi` of both
//! final states (the quantity the HTR-10 neutronics consumes; same slab
//! estimator as `pebble_bed_bulk.rs`) is reported per cell.
//!
//! ## Prediction (written 2026-11-11 before the run)
//!
//! The mechanical path and the sum order are bit-identical per the
//! per-contact and rotating-mesh tests, so every frame of every cell is
//! predicted **bit-identical** (chaos cannot amplify a difference that is
//! zero), and `phi` identical to all printed digits. If a cell is not, the
//! first differing frame names where the translation stops being exact.
//!
//! ## Result
//!
//! Printed by the test (`--nocapture`); recorded in `docs/cross-code-summary.md`
//! and the V&V document.

use outram_park_fork_liggghts::boundary::Boundary;
use outram_park_fork_liggghts::granular::{GranularContactModel, GranularMaterial, RollingModel};
use outram_park_fork_liggghts::granular_system::GranularSystem;
use outram_park_fork_liggghts::neighbor_list::LiggghtsNeighbor;
use outram_park_fork_liggghts::particle::{Particle, Vec3};
use petir::mathf::RealMath;
use uom::si::f64::{Length, Mass, ThermodynamicTemperature};
use uom::si::{length::meter, mass::kilogram, thermodynamic_temperature::kelvin};

const R_P: f64 = 0.005;
const R_CYL: f64 = 0.030;

fn read(name: &str) -> Option<String> {
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../reference-data/liggghts")
        .join(name);
    std::fs::read_to_string(path).ok()
}

/// Bulk solid fraction and bed top, as in `pebble_bed_bulk.rs`.
fn packing(centres: &[Vec3]) -> f64 {
    let mut zs: Vec<f64> = centres.iter().map(|p| p.z).collect();
    zs.sort_by(|a, b| a.partial_cmp(b).expect("finite z"));
    let (z_lo, z_hi) = (zs[0] + 4.0 * R_P, zs[zs.len() - 1] - 4.0 * R_P);
    let cap = |zc: f64| {
        let lo = z_lo.max(zc - R_P);
        let hi = z_hi.min(zc + R_P);
        if hi <= lo {
            return 0.0;
        }
        let f = |z: f64| std::f64::consts::PI * (R_P * R_P * (z - zc) - (z - zc).r_powi(3) / 3.0);
        f(hi) - f(lo)
    };
    let solid: f64 = centres.iter().map(|p| cap(p.z)).sum();
    solid / (std::f64::consts::PI * R_CYL * R_CYL * (z_hi - z_lo))
}

fn init_particles(csv: &str) -> Vec<Particle> {
    let mass = 2500.0 * 4.0 / 3.0 * std::f64::consts::PI * R_P * R_P * R_P;
    csv.lines()
        .skip(1)
        .filter(|l| !l.trim().is_empty())
        .map(|l| {
            let v: Vec<f64> = l.split(',').map(|x| x.parse().unwrap()).collect();
            Particle::new(
                Vec3::new(v[1], v[2], v[3]),
                Vec3::new(v[4], v[5], v[6]),
                Vec3::zero(),
                Mass::new::<kilogram>(mass),
                Length::new::<meter>(R_P),
                ThermodynamicTemperature::new::<kelvin>(300.0),
            )
            .unwrap()
        })
        .collect()
}

fn run_cell(label: &str, mu: f64, mur: f64) {
    let (Some(init), Some(traj)) = (read("pebble_bed_init.csv"), read(&format!("ablate_{label}.csv")))
    else {
        eprintln!("skipping {label}: reference data not present");
        return;
    };
    let material = GranularMaterial::new(5.0e6, 0.3, 0.5, mu).unwrap();
    let mut model = GranularContactModel::hertz_history(material);
    if mur > 0.0 {
        model = model.with_rolling(RollingModel::cdt(mur).unwrap());
    }
    let mut sys = GranularSystem::new(
        init_particles(&init),
        vec![
            Boundary::cylinder(Vec3::zero(), Vec3::new(0.0, 0.0, 1.0), R_CYL).unwrap(),
            Boundary::wall(Vec3::zero(), Vec3::new(0.0, 0.0, 1.0)).unwrap(),
        ],
        model,
        Vec3::new(0.0, 0.0, -9.81),
        5e-6,
    )
    .unwrap()
    .with_liggghts_neighbor_order(
        LiggghtsNeighbor::new(Vec3::new(-1.0, -1.0, -0.005), Vec3::new(1.0, 1.0, 0.35), 0.003)
            .unwrap(),
    );

    let mut done = 0usize;
    let mut first_bad: Option<usize> = None;
    let (mut total, mut same) = (0usize, 0usize);
    let mut last_theirs: Vec<Vec3> = Vec::new();
    for row in traj.lines().skip(1).filter(|l| !l.trim().is_empty()) {
        let v: Vec<f64> = row.split(',').map(|x| x.parse().unwrap()).collect();
        let step = v[0] as usize;
        sys.run(step - done);
        done = step;
        let mut bad = 0usize;
        last_theirs.clear();
        for (i, p) in sys.particles().iter().enumerate() {
            let o = 1 + 9 * i;
            let ours = [
                p.position.x,
                p.position.y,
                p.position.z,
                p.velocity.x,
                p.velocity.y,
                p.velocity.z,
                p.angular_velocity.x,
                p.angular_velocity.y,
                p.angular_velocity.z,
            ];
            for k in 0..9 {
                total += 1;
                if ours[k].to_bits() == v[o + k].to_bits() {
                    same += 1;
                } else {
                    bad += 1;
                }
            }
            last_theirs.push(Vec3::new(v[o], v[o + 1], v[o + 2]));
        }
        if bad > 0 && first_bad.is_none() {
            first_bad = Some(step);
        }
    }
    let ours: Vec<Vec3> = sys.particles().iter().map(|p| p.position).collect();
    let (phi_ours, phi_theirs) = (packing(&ours), packing(&last_theirs));
    eprintln!(
        "ablation {label}: {same}/{total} values bit-identical, first differing frame {first_bad:?}, \
         final phi ours {phi_ours:.6} vs LIGGGHTS {phi_theirs:.6}"
    );
    assert_eq!(same, total, "{label}: first differing frame {first_bad:?}");
}

/// Cell `mu = 0.1`, `mu_r = 0` (the HTR-10 literature cell, V&V 4.9).
#[test]
#[cfg_attr(not(feature = "long-tests"), ignore = "long test (~2 min per cell)")]
fn cell_mu10_mur00() {
    run_cell("mu10_mur00", 0.1, 0.0);
}

/// Cell `mu = 0.4`, `mu_r = 0`.
#[test]
#[cfg_attr(not(feature = "long-tests"), ignore = "long test (~2 min per cell)")]
fn cell_mu40_mur00() {
    run_cell("mu40_mur00", 0.4, 0.0);
}

/// Cell `mu = 0.1`, `mu_r = 0.1` (CDT).
#[test]
#[cfg_attr(not(feature = "long-tests"), ignore = "long test (~2 min per cell)")]
fn cell_mu10_mur10() {
    run_cell("mu10_mur10", 0.1, 0.1);
}

/// Cell `mu = 0.4`, `mu_r = 0.1` (CDT), the one HTR-10 cell LIGGGHTS also ran.
#[test]
#[cfg_attr(not(feature = "long-tests"), ignore = "long test (~2 min per cell)")]
fn cell_mu40_mur10() {
    run_cell("mu40_mur10", 0.4, 0.1);
}
