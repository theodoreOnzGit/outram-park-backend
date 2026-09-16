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

//! # Bulk pebble-bed settling against upstream LIGGGHTS
//!
//! The cases in `liggghts_cross_code.rs` verify a *contact*. A pebble bed is a
//! many-contact, frictional, **chaotic** assembly, so it is checked here
//! separately and **statistically** — a step-by-step trajectory comparison is
//! not a meaningful test for it, and claiming one would be dishonest.
//!
//! ## Methodology
//!
//! Cylindrical container `R = 30 mm` (`D/d = 6`), 354 pebbles `d = 10 mm`,
//! `ρ = 2500 kg/m³`, `E = 5 MPa`, `ν = 0.3`, `e = 0.5`, `µ = 0.3`. Settled
//! under gravity for 400 000 steps at `dt = 5 µs` (2.0 s simulated). The run
//! starts from **LIGGGHTS' own post-insertion configuration**
//! (`reference-data/liggghts/pebble_bed_init.csv`), so both codes integrate
//! the identical initial state; the settled LIGGGHTS state is
//! `pebble_bed_settled.csv`.
//!
//! Solid fraction is measured over a bulk slab excluding 4 particle radii at
//! the bottom wall and at the free surface, by exact sphere-cap integration —
//! so a particle straddling a slab face contributes only the part inside it.
//!
//! ## Results (2026-09-15)
//!
//! | Quantity | Rust | LIGGGHTS | difference |
//! |---|---|---|---|
//! | solid fraction `φ` | 0.5571 | 0.5582 | **0.20 % relative** |
//! | voidage `ε` | 0.4429 | 0.4418 | 0.25 % relative |
//! | bed top `z_top` [m] | 0.12015 | 0.12244 | 1.9 % (≈ ¼ diameter) |
//! | coordination number `Z` | 4.475 | — | — |
//! | residual `KE` [J] | `4.5e-8` | `2.3e-11` | see below |
//!
//! The residual-energy gap was diagnosed, not waved through: **100 % of the
//! remaining kinetic energy sits in 3 of the 354 particles**, with the contact
//! count steady at 964–965 from `t = 0.75 s`. The packed bed is static; what
//! remains is a couple of unconstrained pebbles rattling on the free surface at
//! `≤ 1.6e-2 m/s`, which is also where `z_top` differs. Recorded as an open
//! observation in `docs/verification-and-validation.md`, not as fully
//! understood.
//!
//! ## Runtime
//!
//! ~210 s in release mode. That is under the workspace's 5-minute threshold,
//! so it is not gated and runs in an ordinary suite -- which is what keeps the
//! packing-fraction figure quoted in the maturity roster honest.
//!
//! ```bash
//! cargo test --release -p outram-park-fork-liggghts --test pebble_bed_bulk
//! ```

use outram_park_fork_liggghts::boundary::Boundary;
use outram_park_fork_liggghts::granular::{ContactKinematics, GranularContactModel, GranularMaterial};
use outram_park_fork_liggghts::granular_system::GranularSystem;
use outram_park_fork_liggghts::particle::{Particle, Vec3};
use uom::si::f64::{Length, Mass, ThermodynamicTemperature};
use uom::si::{length::meter, mass::kilogram, thermodynamic_temperature::kelvin};

/// Pebble radius `[m]`.
const R_P: f64 = 0.005;
/// Container radius `[m]` (`D/d = 6`).
const R_CYL: f64 = 0.030;
/// Pebble density `[kg/m³]`.
const RHO: f64 = 2500.0;

/// Load a committed `id,x,y,z,vx,vy,vz` state file, sorted by id.
fn load_state(name: &str) -> Option<Vec<(Vec3, Vec3)>> {
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../reference-data/liggghts")
        .join(name);
    let text = std::fs::read_to_string(path).ok()?;
    let mut rows: Vec<(usize, Vec3, Vec3)> = text
        .lines()
        .skip(1)
        .filter(|l| !l.trim().is_empty())
        .map(|l| {
            let f: Vec<f64> = l
                .split(',')
                .map(|v| v.parse().expect("numeric csv"))
                .collect();
            (
                f[0] as usize,
                Vec3::new(f[1], f[2], f[3]),
                Vec3::new(f[4], f[5], f[6]),
            )
        })
        .collect();
    rows.sort_by_key(|r| r.0);
    Some(rows.into_iter().map(|r| (r.1, r.2)).collect())
}

/// Bulk solid fraction `[-]` and bed top `[m]` for a set of centres.
///
/// The slab excludes `4·r` at the bottom and at the free surface; solid volume
/// inside it is the exact sphere-cap integral, so partially-included particles
/// are counted correctly.
fn packing(centres: &[Vec3]) -> (f64, f64) {
    let mut zs: Vec<f64> = centres.iter().map(|p| p.z).collect();
    zs.sort_by(|a, b| a.partial_cmp(b).expect("finite z"));
    let (z_lo, z_hi) = (zs[0] + 4.0 * R_P, zs[zs.len() - 1] - 4.0 * R_P);
    let cap = |zc: f64| {
        let lo = z_lo.max(zc - R_P);
        let hi = z_hi.min(zc + R_P);
        if hi <= lo {
            return 0.0;
        }
        let f = |z: f64| std::f64::consts::PI * (R_P * R_P * (z - zc) - (z - zc).powi(3) / 3.0);
        f(hi) - f(lo)
    };
    let solid: f64 = centres.iter().map(|p| cap(p.z)).sum();
    let slab = std::f64::consts::PI * R_CYL * R_CYL * (z_hi - z_lo);
    (solid / slab, zs[zs.len() - 1])
}

/// Mean coordination number `[-]`: contacts per particle, counting both ends.
fn coordination(ps: &[Particle]) -> f64 {
    let mut n = 0_usize;
    for i in 0..ps.len() {
        for j in (i + 1)..ps.len() {
            if ContactKinematics::pair(&ps[i], &ps[j]).is_some() {
                n += 2;
            }
        }
    }
    n as f64 / ps.len() as f64
}

/// Settle the bed from LIGGGHTS' initial configuration and compare the bulk
/// statistics against LIGGGHTS' settled state.
///
/// See the module docs for methodology and the 2026-09-15 measured results.
#[test]
fn bulk_packing_matches_liggghts() {
    let (Some(init), Some(settled)) = (
        load_state("pebble_bed_init.csv"),
        load_state("pebble_bed_settled.csv"),
    ) else {
        eprintln!("skipping: reference-data/liggghts/ not present");
        return;
    };
    assert_eq!(init.len(), 354, "unexpected reference particle count");

    let mass = RHO * 4.0 / 3.0 * std::f64::consts::PI * R_P.powi(3);
    let particles: Vec<Particle> = init
        .iter()
        .map(|(x, v)| {
            Particle::new(
                *x,
                *v,
                Vec3::zero(),
                Mass::new::<kilogram>(mass),
                Length::new::<meter>(R_P),
                ThermodynamicTemperature::new::<kelvin>(300.0),
            )
            .expect("valid pebble")
        })
        .collect();

    let material = GranularMaterial::new(5.0e6, 0.3, 0.5, 0.3).expect("valid material");
    let boundaries = vec![
        Boundary::cylinder(Vec3::zero(), Vec3::new(0.0, 0.0, 1.0), R_CYL).expect("valid cylinder"),
        Boundary::wall(Vec3::zero(), Vec3::new(0.0, 0.0, 1.0)).expect("valid floor"),
    ];
    let mut sys = GranularSystem::new(
        particles,
        boundaries,
        GranularContactModel::hertz_history(material),
        Vec3::new(0.0, 0.0, -9.81),
        5.0e-6,
    )
    .expect("valid system");
    sys.run(400_000);

    let ours: Vec<Vec3> = sys.particles().iter().map(|p| p.position).collect();
    let theirs: Vec<Vec3> = settled.iter().map(|(x, _)| *x).collect();
    let (phi_ours, z_ours) = packing(&ours);
    let (phi_theirs, z_theirs) = packing(&theirs);

    eprintln!(
        "phi: ours {phi_ours:.4} vs LIGGGHTS {phi_theirs:.4}  \
         (eps {:.4} vs {:.4});  z_top {z_ours:.5} vs {z_theirs:.5} m;  Z = {:.3}",
        1.0 - phi_ours,
        1.0 - phi_theirs,
        coordination(sys.particles())
    );

    // Bulk packing fraction: measured 0.20 % relative difference. The 2 %
    // bound is a regression catch with ~10x margin, not a physics tolerance.
    let rel = (phi_ours - phi_theirs).abs() / phi_theirs;
    assert!(
        rel < 0.02,
        "bulk packing fraction differs from LIGGGHTS by {:.2} % (ours {phi_ours:.4}, \
         theirs {phi_theirs:.4})",
        rel * 100.0
    );

    // The bed must actually be a bed: a random packing of equal spheres sits
    // near phi ~ 0.55-0.64, and a collapsed or exploded run would not.
    assert!(
        (0.50..0.65).contains(&phi_ours),
        "settled solid fraction {phi_ours:.4} is outside the physical range for \
         a random sphere packing"
    );

    // Bed height within one particle diameter.
    assert!(
        (z_ours - z_theirs).abs() < 2.0 * R_P,
        "bed top {z_ours:.5} m differs from LIGGGHTS {z_theirs:.5} m by more \
         than one particle diameter"
    );

    // The bulk must be static. Measured residual KE is 4.5e-8 J, all of it in
    // 3 of 354 surface particles; 1e-5 J is a divergence catch.
    let ke = sys.kinetic_energy();
    assert!(ke < 1.0e-5, "bed has not settled: KE = {ke:e} J");
}
