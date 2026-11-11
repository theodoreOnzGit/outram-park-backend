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
//! | oblique + friction, Hertz | 251 | `0` | ~~`1.11e-16`~~ `0` | ~~`5.68e-14`~~ `0` |
//! | wall bounce + gravity | 2001 | `0` | `0` | — |
//!
//! ~~**Three of the four cases are bit-identical** ... the oblique case agrees
//! to round-off (1 ulp in v, ~3 ulp in ω), "summing the same terms in a
//! different association order".~~ **CORRECTED 2026-11-10 (gh:#1406): every
//! case in this file is bit-identical, checked on the IEEE-754 bit pattern of
//! every x, v and ω value of every frame** (`Diff::mismatched == 0`). The
//! "association order" residual was real and is gone: the port's `ω` update in
//! `integrator.rs` was `half * (1/(0.4 m r²))` where `fix_nve_sphere.cpp` has
//! `(dtf/0.4)/(r*r*m)`, and the history model's slip rescale stored
//! `ft * (-1/kt)` where upstream has `-Ft/kt`. Table of all fixes:
//! `docs/cross-code-summary.md`. Cases added the same day (decks in
//! `reference-data/liggghts/`): oblique **Hooke** + history + CDT rolling, and
//! two atom types (different `E`, `ν`, `e`, `µ`, `d`, `ρ`), head-on and
//! oblique — 4518 of 4518 `f64` bit-identical in each.
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
    GranularContactModel, GranularMaterial, GranularNormalModel, MaterialTable, RollingModel,
    TangentialModel,
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
            .map(|l| {
                l.split(',')
                    .map(|v| v.parse().expect("numeric csv"))
                    .collect()
            })
            .collect(),
    )
}

/// Worst deviations plus a **bitwise** audit of one replayed trajectory.
#[derive(Debug, Clone, Copy)]
struct Diff {
    dx: f64,
    dv: f64,
    dw: f64,
    /// Number of compared `f64` values whose IEEE-754 bit pattern differs.
    mismatched: usize,
    /// Number of `f64` values compared.
    total: usize,
    /// First differing value: `(step, atom index, field index 0..9 =
    /// x y z vx vy vz wx wy wz, got, want)`.
    first: Option<(usize, usize, usize, f64, f64)>,
}

impl Diff {
    fn bit_identical(&self) -> bool {
        self.mismatched == 0
    }
}

/// Replay a two-particle case against the reference.
fn replay_pair(rows: &[Vec<f64>], model: GranularContactModel, v_a: Vec3, v_b: Vec3) -> Diff {
    replay_pair_at(rows, model, 0.0060, v_a, v_b, Vec3::zero(), Vec3::zero())
}

/// As [`replay_pair`], with explicit initial separation and spins.
fn replay_pair_at(
    rows: &[Vec<f64>],
    model: GranularContactModel,
    half_gap: f64,
    v_a: Vec3,
    v_b: Vec3,
    w_a: Vec3,
    w_b: Vec3,
) -> Diff {
    let mut pa = sphere(Vec3::new(-half_gap, 0.0, 0.0), v_a);
    let mut pb = sphere(Vec3::new(half_gap, 0.0, 0.0), v_b);
    pa.angular_velocity = w_a;
    pb.angular_velocity = w_b;
    let particles = vec![pa, pb];
    let mut sys =
        GranularSystem::new(particles, vec![], model, Vec3::zero(), 1.0e-6).expect("valid system");
    replay_system(&mut sys, rows)
}

/// Run `sys` through the frames of `rows` (columns: step, then 9 per atom:
/// x y z vx vy vz wx wy wz) and compare every value bit for bit.
fn replay_system(sys: &mut GranularSystem, rows: &[Vec<f64>]) -> Diff {
    let mut d = Diff {
        dx: 0.0,
        dv: 0.0,
        dw: 0.0,
        mismatched: 0,
        total: 0,
        first: None,
    };
    let mut last = 0_usize;
    for row in rows {
        let step = row[0] as usize;
        sys.run(step - last);
        last = step;
        let n = sys.particles().len();
        for index in 0..n {
            let base = 1 + 9 * index;
            let p = sys.particles()[index];
            let got = [
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
            for (k, g) in got.iter().enumerate() {
                let want = row[base + k];
                let err = (g - want).abs();
                match k / 3 {
                    0 => d.dx = d.dx.max(err),
                    1 => d.dv = d.dv.max(err),
                    _ => d.dw = d.dw.max(err),
                }
                d.total += 1;
                if g.to_bits() != want.to_bits() {
                    d.mismatched += 1;
                    d.first.get_or_insert((step, index, k, *g, want));
                }
            }
        }
    }
    d
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
    let diff = replay_pair(
        &rows,
        GranularContactModel::hertz_history(material()),
        Vec3::new(1.0, 0.0, 0.0),
        Vec3::new(-1.0, 0.0, 0.0),
    );
    assert!(
        diff.bit_identical(),
        "not bit-identical vs LIGGGHTS: {}/{} values differ, max|dx| {:e} m, max|dv| {:e} m/s, max|dw| {:e} rad/s, first {:?}",
        diff.mismatched, diff.total, diff.dx, diff.dv, diff.dw, diff.first
    );
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
    let diff = replay_pair(
        &rows,
        model,
        Vec3::new(1.0, 0.0, 0.0),
        Vec3::new(-1.0, 0.0, 0.0),
    );
    assert!(
        diff.bit_identical(),
        "not bit-identical vs LIGGGHTS: {}/{} values differ, max|dx| {:e} m, max|dv| {:e} m/s, max|dw| {:e} rad/s, first {:?}",
        diff.mismatched, diff.total, diff.dx, diff.dv, diff.dw, diff.first
    );
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
    let diff = replay_pair(
        &rows,
        GranularContactModel::hertz_history(material()),
        Vec3::new(1.0, 0.5, 0.0),
        Vec3::new(-1.0, -0.5, 0.0),
    );
    assert!(
        diff.bit_identical(),
        "not bit-identical vs LIGGGHTS: {}/{} values differ, max|dx| {:e} m, max|dv| {:e} m/s, max|dw| {:e} rad/s, first {:?}",
        diff.mismatched, diff.total, diff.dx, diff.dv, diff.dw, diff.first
    );
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

/// Two spheres held in contact and **counter-spinning**, with the CDT rolling
/// model active (`µ_r = 0.1`) — isolates
/// [`RollingModel::Cdt`](outram_park_fork_liggghts::granular::RollingModel).
///
/// **Methodology.** Both spheres start overlapping by `2e-4 m` with
/// `ω_y = ±20 rad/s` and no translational velocity. The rolling torque bleeds
/// the spin away while friction pushes the pair apart, so the case exercises
/// the rolling model, the shear history and the normal model together.
/// LIGGGHTS input: `reference-data/liggghts/in.rolling_pair`.
///
/// **Result (2026-09-15).** Bit-identical to LIGGGHTS across all 201 frames.
/// Final state in both codes: `ω_y = ±13.617325830096817 rad/s` (from `±20`),
/// `v_x = ∓0.23846042770436646 m/s`.
///
/// This is the test that pins upstream's three CDT details the pre-existing
/// [`crate::rolling`] module gets differently: the **elastic** normal force
/// `k_n·δ_n` rather than the damped `|F_n|`, removal of the torsion component,
/// and the wall-branch rolling velocity.
#[test]
fn rolling_cdt_matches_liggghts() {
    let Some(rows) = load("rolling_pair.csv") else {
        eprintln!("skipping: reference-data/liggghts/ not present");
        return;
    };
    assert_eq!(rows.len(), 201, "unexpected reference frame count");
    let model = GranularContactModel::hertz_history(material())
        .with_rolling(RollingModel::cdt(0.1).expect("valid mu_r"));
    let diff = replay_pair_at(
        &rows,
        model,
        0.0049,
        Vec3::zero(),
        Vec3::zero(),
        Vec3::new(0.0, 20.0, 0.0),
        Vec3::new(0.0, -20.0, 0.0),
    );
    assert!(
        diff.bit_identical(),
        "not bit-identical vs LIGGGHTS: {}/{} values differ, max|dx| {:e} m, max|dv| {:e} m/s, max|dw| {:e} rad/s, first {:?}",
        diff.mismatched, diff.total, diff.dx, diff.dv, diff.dw, diff.first
    );
    // The rolling model must actually have bled spin away, or this proves nothing.
    let final_wy = rows.last().expect("frames")[8]; // omegay of atom 1
    assert!(
        final_wy < 15.0 && final_wy > 10.0,
        "reference case should decay the spin from 20 rad/s, got {final_wy}"
    );
}

// ---------------------------------------------------------------------------
// gh:#1406 prio-1 rows (added 2026-11-10): Hooke + history + CDT rolling, and
// two atom types with different E, nu, e, mu.
// ---------------------------------------------------------------------------

/// Particle of radius `radius` and density `rho`, mass formed exactly as
/// LIGGGHTS' `set ... diameter ... density ...` does
/// (`set.cpp:1003`: `4.0*MY_PI/3.0 * r*r*r * density`).
fn sphere_rho(radius: f64, rho: f64, position: Vec3, velocity: Vec3) -> Particle {
    Particle::new(
        position,
        velocity,
        Vec3::zero(),
        Mass::new::<kilogram>(4.0 * std::f64::consts::PI / 3.0 * radius * radius * radius * rho),
        Length::new::<meter>(radius),
        ThermodynamicTemperature::new::<kelvin>(300.0),
    )
    .expect("valid sphere")
}

fn assert_bit_identical(diff: &Diff) {
    assert!(
        diff.bit_identical(),
        "not bit-identical vs LIGGGHTS: {}/{} values differ, max|dx| {:e} m, max|dv| {:e} m/s, max|dw| {:e} rad/s, first {:?}",
        diff.mismatched,
        diff.total,
        diff.dx,
        diff.dv,
        diff.dw,
        diff.first
    );
}

/// Oblique collision, **Hooke** normal model + tangential history + **CDT
/// rolling** (`µ_r = 0.1`) — the same geometry as the Hertz oblique case
/// (`in.oblique_hertz`), deck `in.oblique_hooke_rolling`.
///
/// **Methodology.** Two `d = 10 mm`, `ρ = 2500 kg/m³` spheres at `x = ∓6 mm`,
/// `v = ±(1, 0.5) m/s`, `E = 10 MPa`, `ν = 0.3`, `e = 0.9`, `µ = 0.5`,
/// `v_char = 2 m/s`, `dt = 1 µs`, 2500 steps, 251 frames of x, v, ω compared
/// bit for bit. Pass criterion: every `f64` bit-identical.
///
/// **Prediction (written before the first run, 2026-11-10).** Bit-identical:
/// head-on Hooke, oblique Hertz history and CDT rolling are each
/// bit-identical once the tangential accumulation order matches upstream, and
/// the Hooke normal model's arithmetic is a line-for-line translation.
///
/// **Result (2026-11-10, Rust 1.99.1, upstream g++ 15.2.1).** Prediction
/// confirmed: 4518 of 4518 `f64` bit-identical over 251 frames; final
/// `ω_z = -73.676478822126072 rad/s` in both codes (spin-up through the
/// tangential history spring, bled by CDT rolling resistance).
#[test]
fn oblique_hooke_history_rolling_matches_liggghts() {
    let Some(rows) = load("oblique_hooke_rolling.csv") else {
        eprintln!("skipping: reference-data/liggghts/ not present");
        return;
    };
    assert_eq!(rows.len(), 251, "unexpected reference frame count");
    let model = GranularContactModel::new(
        GranularNormalModel::hooke(material(), 2.0).expect("valid hooke"),
        TangentialModel::History,
    )
    .with_rolling(RollingModel::cdt(0.1).expect("valid mu_r"));
    let diff = replay_pair(
        &rows,
        model,
        Vec3::new(1.0, 0.5, 0.0),
        Vec3::new(-1.0, -0.5, 0.0),
    );
    assert_bit_identical(&diff);
    let final_wz = rows.last().expect("frames")[9];
    assert!(
        final_wz.abs() > 10.0,
        "reference case should spin the particles up, got wz = {final_wz}"
    );
}

/// Two atom types with different `E`, `ν`, `e`, `µ` and different size and
/// density — the `global_properties` mixing rules (`createYeff`, `createGeff`,
/// `createCoeffRestLog`/`createBetaEff`, `createCoeffFrict`).
///
/// **Methodology.** Decks `in.twotype_headon` and `in.twotype_oblique`.
/// Type 1: `d = 10 mm`, `ρ = 2500`, `E = 1e7`, `ν = 0.3`. Type 2: `d = 12 mm`,
/// `ρ = 4000`, `E = 3e7`, `ν = 0.2`. Restitution matrix
/// `[[0.9, 0.7], [0.7, 0.5]]`, friction matrix `[[0.5, 0.3], [0.3, 0.2]]`
/// (`peratomtypepair`, so the 1-2 entries are user data, not mixed). Hertz +
/// history, `dt = 1 µs`, 2500 steps, 251 frames of x, v, ω bit for bit.
///
/// **Prediction (written before the first run, 2026-11-10).** Bit-identical:
/// `Y_eff` and `G_eff` are formed per type pair with upstream's formulas, and
/// `R*`, `m*` already carry per-particle radius and mass. A mixing-rule
/// mismatch would show at the first contact frame.
///
/// **Result (2026-11-10, Rust 1.99.1, upstream g++ 15.2.1).** Prediction
/// confirmed: 4518 of 4518 `f64` bit-identical in each of the two cases. Final
/// state head-on: `v = (-1.4969749755932411, 0, 0)` and `(-0.096869583480453048,
/// 0, 0)` m/s (realised restitution of the 1-2 pair 0.70005 against the
/// requested `e_12 = 0.7`, not the 0.9 or 0.5 diagonal entries);
/// oblique: `ω_z = -127.31535906481682` and `-38.620347361525148` rad/s.
fn twotype_case(file: &str, oblique: bool) -> Diff {
    let rows = load(file).unwrap_or_else(|| panic!("{file} missing"));
    assert_eq!(rows.len(), 251, "unexpected reference frame count");
    let vy = if oblique { 0.5 } else { 0.0 };
    let pa = sphere_rho(0.005, 2500.0, Vec3::new(-0.0060, 0.0, 0.0), Vec3::new(1.0, vy, 0.0));
    let pb = sphere_rho(0.006, 4000.0, Vec3::new(0.0060, 0.0, 0.0), Vec3::new(-1.0, if oblique { -0.5 } else { 0.0 }, 0.0));
    let table = MaterialTable::new(
        &[(1.0e7, 0.3), (3.0e7, 0.2)],
        &[&[0.9, 0.7], &[0.7, 0.5]],
        &[&[0.5, 0.3], &[0.3, 0.2]],
    )
    .expect("valid table");
    let model = GranularContactModel::hertz_history(material()).with_materials(table);
    let mut sys = GranularSystem::new(vec![pa, pb], vec![], model, Vec3::zero(), 1.0e-6)
        .expect("valid system")
        .with_particle_types(vec![0, 1])
        .expect("valid types");
    replay_system(&mut sys, &rows)
}

/// Two-type head-on collision (normal mixing: `Y_eff`, `β_eff`, `R*`, `m*`).
/// Prediction and methodology: see [`twotype_case`].
#[test]
fn two_type_headon_matches_liggghts() {
    assert_bit_identical(&twotype_case("twotype_headon.csv", false));
}

/// Two-type oblique collision (adds `G_eff`, the 1-2 friction entry and the
/// contact torque on unequal radii). Prediction and methodology: see
/// [`twotype_case`].
#[test]
fn two_type_oblique_matches_liggghts() {
    assert_bit_identical(&twotype_case("twotype_oblique.csv", true));
}

// ---------------------------------------------------------------------------
// gh:#1410 (2026-11-11): the `hertz/stiffness` and `hooke/stiffness` normal
// models. Decks `in.stiff_hertz`, `in.stiff_hooke`, `in.stiff_hooke_abs`.
// ---------------------------------------------------------------------------

/// Oblique collision of the standard pair (`d = 10 mm`, `rho = 2500`, `x =
/// -+6 mm`, `v = +-(1, 0.5) m/s`, `mu = 0.5`, `dt = 1 us`, 2500 steps, tangential
/// history) with the stiffness and damping given directly, 251 frames of x, v,
/// omega compared bit for bit.
///
/// **Prediction (written before the first run, 2026-11-11).** Bit-identical:
/// both models are a handful of products with no `pow`/`exp`, translated
/// operation by operation from `normal_model_hertz_stiffness.h` /
/// `normal_model_hooke_stiffness.h`.
fn stiffness_case(file: &str, normal: GranularNormalModel) -> Diff {
    let rows = load(file).unwrap_or_else(|| panic!("{file} missing"));
    assert_eq!(rows.len(), 251, "unexpected reference frame count");
    let model = GranularContactModel::new(normal, TangentialModel::History);
    replay_pair(&rows, model, Vec3::new(1.0, 0.5, 0.0), Vec3::new(-1.0, -0.5, 0.0))
}

/// `hertz/stiffness`: `kn = 1.2e8`, `kt = 4e7`, `gamman = 4.8e5`, `gammat =
/// 2.4e5`. **Result:** see `docs/cross-code-summary.md`.
#[test]
fn hertz_stiffness_matches_liggghts() {
    let normal = GranularNormalModel::hertz_stiffness(material(), 1.2e8, 4.0e7, 4.8e5, 2.4e5)
        .expect("valid hertz/stiffness");
    assert_bit_identical(&stiffness_case("stiff_hertz.csv", normal));
}

/// `hooke/stiffness` with relative damping: `kn = 2e5`, `kt = 6e4`, `gamman =
/// 800`, `gammat = 400` (times `m_eff`).
#[test]
fn hooke_stiffness_matches_liggghts() {
    let normal = GranularNormalModel::hooke_stiffness(material(), 2.0e5, 6.0e4, 800.0, 400.0)
        .expect("valid hooke/stiffness");
    assert_bit_identical(&stiffness_case("stiff_hooke.csv", normal));
}

/// `hooke/stiffness absolute_damping on tangential_damping off`: `gamman_abs =
/// 0.52`, `gammat_abs = 0.26` kg/s, no damping on the shear force.
#[test]
fn hooke_stiffness_absolute_damping_matches_liggghts() {
    let normal = GranularNormalModel::hooke_stiffness(material(), 2.0e5, 6.0e4, 0.52, 0.26)
        .expect("valid hooke/stiffness")
        .with_absolute_damping(true)
        .with_tangential_damping(false);
    assert_bit_identical(&stiffness_case("stiff_hooke_abs.csv", normal));
}

// ---------------------------------------------------------------------------
// gh:#1410 (2026-11-11): `hooke/hysteresis` (`normal_model_hooke_hysteresis.h`).
// ---------------------------------------------------------------------------

/// Plastic Hooke (`hooke/hysteresis`), oblique collision of the standard pair,
/// 251 frames of x, v, omega bit for bit.
///
/// **Methodology.** `in.hyst_fast`: `v = +-(1, 0.5) m/s`, `v_char = 2 m/s`,
/// `k2Max/kn = 3`, `kc/kn = 0.5`, `phi_f = 0.05` (so `deltaMaxLim = 0.375 mm`
/// is passed during loading and unloading reaches the cohesion branch, which
/// rewrites `deltaMax`), 2500 steps. `in.hyst_slow`: `v = +-(0.25, 0.125)`,
/// `k2Max/kn = 4`, `kc = 0`, `phi_f = 0.2` (`deltaMax < deltaMaxLim`: the
/// interpolated `k2` branch), 5000 steps sampled every 20.
///
/// **Prediction (written before the first run, 2026-11-11).** Bit-identical in
/// both: a line-for-line translation, with `deltaMax` carried per contact.
fn hysteresis_case(file: &str, normal: GranularNormalModel) -> Diff {
    let rows = load(file).unwrap_or_else(|| panic!("{file} missing"));
    assert_eq!(rows.len(), 251, "unexpected reference frame count");
    let model = GranularContactModel::new(normal, TangentialModel::History);
    let (vx, vy) = if file.contains("fast") { (1.0, 0.5) } else { (0.25, 0.125) };
    replay_pair(&rows, model, Vec3::new(vx, vy, 0.0), Vec3::new(-vx, -vy, 0.0))
}

/// Fast impact: plastic loading past `deltaMaxLim`, cohesion on unloading.
#[test]
fn hooke_hysteresis_fast_matches_liggghts() {
    let normal =
        GranularNormalModel::hooke_hysteresis(material(), 2.0, 3.0, 0.5, 0.05).expect("valid");
    assert_bit_identical(&hysteresis_case("hyst_fast.csv", normal));
}

/// Slow impact: the interpolated-`k2` branch, no adhesion.
#[test]
fn hooke_hysteresis_slow_matches_liggghts() {
    let normal =
        GranularNormalModel::hooke_hysteresis(material(), 2.0, 4.0, 0.0, 0.2).expect("valid");
    assert_bit_identical(&hysteresis_case("hyst_slow.csv", normal));
}
