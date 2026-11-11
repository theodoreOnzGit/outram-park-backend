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

//! # Contact sub-models against upstream LIGGGHTS-PUBLIC (gh:#1410, #1411)
//!
//! Every test replays a deck in `reference-data/liggghts/` through
//! [`GranularSystem`] and compares **every x, v and omega of every sampled
//! frame on its IEEE-754 bit pattern** with the trajectory of the compiled
//! upstream (LIGGGHTS-PUBLIC `3d5c00f2`, recipe in
//! `upstream_source/README.md`). Methodology, predictions and results per
//! model: `verification_and_validation/` (section 4.11) and
//! `docs/cross-code-summary.md`.
//!
//! If `reference-data/liggghts/` is absent each test skips.

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


fn hooke_history() -> GranularNormalModel {
    GranularNormalModel::hooke(material(), 2.0).expect("valid hooke")
}

/// The standard oblique pair, `v = +-(1, 0.5) m/s`, with initial spins
/// `omega_z = +40` and `-20 rad/s` (a relative spin, without which every
/// rolling model sees `w_r = 0`): 2500 steps, 251 frames.
fn oblique_pair(file: &str, model: GranularContactModel) -> Diff {
    let rows = load(file).unwrap_or_else(|| panic!("{file} missing"));
    assert_eq!(rows.len(), 251, "unexpected reference frame count");
    replay_pair_at(
        &rows,
        model,
        0.0060,
        Vec3::new(1.0, 0.5, 0.0),
        Vec3::new(-1.0, -0.5, 0.0),
        Vec3::new(0.0, 0.0, 40.0),
        Vec3::new(0.0, 0.0, -20.0),
    )
}

/// `rolling_friction epsd` (Ai et al. 2011 elastic-plastic spring-dashpot),
/// `rolling_model_epsd.h`.
///
/// **Methodology.** Deck `in.epsd_pair`: oblique Hooke pair with shear
/// history (as `in.oblique_hooke_rolling`), `coefficientRollingFriction =
/// 0.1`, `coefficientRollingViscousDamping = 0.3`, 2500 steps, every
/// x, v, omega of 251 frames bit for bit.
///
/// **Prediction (written before the first run, 2026-11-11).** Bit-identical:
/// the model is sums, products and one `sqrt`, translated operation by
/// operation (`kr = 2.25 kn mu^2 R*^2`, spring `dt kr w_r`, cap `|Fn| R* mu`,
/// dashpot `visc 2 sqrt(I kr)`), with the rolling torque history a per-contact
/// 3-vector that, as upstream, is written only when `shearupdate` is set.
#[test]
fn epsd_rolling_matches_liggghts() {
    let model = GranularContactModel::new(hooke_history(), TangentialModel::History)
        .with_rolling(RollingModel::epsd(0.1, 0.3).expect("valid"));
    let diff = oblique_pair("epsd_pair.csv", model);
    assert_bit_identical(&diff);
}

/// `rolling_friction epsd2` (spring only, `kr = kt R*^2`).
///
/// **Prediction (2026-11-11).** Bit-identical; as `epsd` without the dashpot
/// and with the history written after the cap.
#[test]
fn epsd2_rolling_matches_liggghts() {
    let model = GranularContactModel::new(hooke_history(), TangentialModel::History)
        .with_rolling(RollingModel::epsd2(0.1).expect("valid"));
    assert_bit_identical(&oblique_pair("epsd2_pair.csv", model));
}

/// `rolling_friction epsd3` (`kr = c_s kn mu^2 R*^2`, `rollingStiffness = 5`).
///
/// **Prediction (2026-11-11).** Bit-identical; as `epsd` with the stiffness
/// factor read from the scalar property instead of 2.25.
#[test]
fn epsd3_rolling_matches_liggghts() {
    let model = GranularContactModel::new(hooke_history(), TangentialModel::History)
        .with_rolling(RollingModel::epsd3(0.1, 0.3, 5.0).expect("valid"));
    assert_bit_identical(&oblique_pair("epsd3_pair.csv", model));
}

/// One spinning sphere hitting a z-plane wall (no gravity): 251 frames of x,
/// v, omega, `fix wall/gran ... primitive type 1 zplane 0`.
fn wall_case(file: &str, model: GranularContactModel) -> Diff {
    let rows = load(file).unwrap_or_else(|| panic!("{file} missing"));
    assert_eq!(rows.len(), 251, "unexpected reference frame count");
    let floor = Boundary::wall(Vec3::zero(), Vec3::new(0.0, 0.0, 1.0)).expect("valid wall");
    let mut p = sphere(Vec3::new(0.0, 0.0, 0.0053), Vec3::new(0.6, 0.2, -0.8));
    p.angular_velocity = Vec3::new(0.0, 30.0, 0.0);
    let mut sys = GranularSystem::new(vec![p], vec![floor], model, Vec3::zero(), 1.0e-6)
        .expect("valid system");
    replay_system(&mut sys, &rows)
}

/// `hooke/hysteresis` against a wall (`in.wall_hyst`), spinning oblique
/// impact: the particle-wall branch of the plastic model (wall `deltaMax`
/// stored per contact, `R* = r`, `m* = m`).
///
/// **Prediction (2026-11-11).** Bit-identical: the wall kinematics are
/// already bit-identical (`wall_bounce`) and the hysteresis arithmetic is the
/// one verified on the pair decks.
#[test]
fn hooke_hysteresis_wall_matches_liggghts() {
    let normal = GranularNormalModel::hooke_hysteresis(material(), 2.0, 3.0, 0.5, 0.05)
        .expect("valid");
    assert_bit_identical(&wall_case(
        "wall_hyst.csv",
        GranularContactModel::new(normal, TangentialModel::History),
    ));
}

/// EPSD rolling against a wall (`in.wall_epsd`): the wall branch (`R* = r`,
/// `I = 1.4 m R*^2`, contact-point rolling velocity `w_r`).
#[test]
fn epsd_rolling_wall_matches_liggghts() {
    let model = GranularContactModel::new(hooke_history(), TangentialModel::History)
        .with_rolling(RollingModel::epsd(0.1, 0.3).expect("valid"));
    assert_bit_identical(&wall_case("wall_epsd.csv", model));
}

/// Ablation control: the EPSD decks are only evidence if the rolling torque
/// matters in them. With rolling switched off (or with the dashpot of `epsd`
/// dropped by replaying it as `epsd2`) the same reference must NOT be matched.
#[test]
fn epsd_decks_are_sensitive_to_the_rolling_model() {
    let off = GranularContactModel::new(hooke_history(), TangentialModel::History);
    let d_off = oblique_pair("epsd_pair.csv", off);
    assert!(
        d_off.mismatched > 100,
        "rolling off should differ widely, got {}/{}",
        d_off.mismatched,
        d_off.total
    );
    let as_two = GranularContactModel::new(hooke_history(), TangentialModel::History)
        .with_rolling(RollingModel::epsd2(0.1).expect("valid"));
    let d_two = oblique_pair("epsd_pair.csv", as_two);
    assert!(d_two.mismatched > 0, "epsd2 must differ from the epsd reference");
    let wall_off = GranularContactModel::new(hooke_history(), TangentialModel::History);
    assert!(
        wall_case("wall_epsd.csv", wall_off).mismatched > 100,
        "the wall deck must be sensitive to rolling too"
    );
}

/// **Finding (2026-11-11).** The existing CDT cross-code deck
/// (`oblique_hooke_rolling`) is symmetric: both spheres spin up identically,
/// so the relative rolling velocity `w_r = omega_i - omega_j` is zero and the
/// CDT torque is never applied; the bit-identity there says nothing about CDT.
/// `in.cdt_pair` adds initial spins `+40` and `-20 rad/s` about z. **Result:**
/// bit-identical (4518/4518), and the model-off control above differs.
#[test]
fn cdt_rolling_with_relative_spin_matches_liggghts() {
    let model = GranularContactModel::new(hooke_history(), TangentialModel::History)
        .with_rolling(RollingModel::cdt(0.1).expect("valid"));
    assert_bit_identical(&oblique_pair("cdt_pair.csv", model));
    let off = GranularContactModel::new(hooke_history(), TangentialModel::History);
    assert!(oblique_pair("cdt_pair.csv", off).mismatched > 100);
}
