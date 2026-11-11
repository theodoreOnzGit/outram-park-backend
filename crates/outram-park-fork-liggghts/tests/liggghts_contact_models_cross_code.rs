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
    GranularCohesion, GranularContactModel, GranularMaterial, GranularNormalModel, MaterialTable,
    PairModel, PairModelTable, RollingModel,
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
    let normal =
        GranularNormalModel::hooke_hysteresis(material(), 2.0, 3.0, 0.5, 0.05).expect("valid");
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
    assert!(
        d_two.mismatched > 0,
        "epsd2 must differ from the epsd reference"
    );
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

/// The reference collided: atom 1's x-velocity left its initial `+1 m/s`.
fn assert_collided(file: &str) {
    let rows = load(file).expect("reference present");
    let vx_end = rows.last().expect("frames")[4];
    assert!(
        vx_end < 0.9,
        "{file}: no collision in the reference (vx = {vx_end})"
    );
}

// ---------------------------------------------------------------------------
// Luding family (gh:#1410): normal_model_luding.h, tangential_model_luding_tn.h,
// rolling_model_luding.h.
// ---------------------------------------------------------------------------

/// `model luding tangential tan_luding rolling_friction luding`, deltaMax
/// below `deltaMaxLim` (interpolated `k2`).
///
/// **Methodology.** Deck `in.luding_full`: `K = 1e4 N/m`, `k2Max/k1 = 3`,
/// `kc/k1 = 0.4`, `phi_f = 0.05`, `f_adh = 1e-3 N`, `e = 0.9`, `mu_s = 0.5`,
/// `mu_d/mu_s = 0.8`, `FrictionViscosity = 0.6`, `kT2kcMax = 0.5`;
/// rolling `mu_r = 0.1`, visc 0.3, `kR2kcMax = 0.5`; oblique pair with
/// relative spins; 2500 steps; every x, v, omega of 251 frames bit for bit.
///
/// **Prediction (written before the first run, 2026-11-11).** Bit-identical:
/// a line-for-line translation of the three headers, with `kc` and `f_adh`
/// passed from the normal to the tangential and rolling model as upstream
/// does (history values), and the Luding rolling spring written
/// unconditionally (also at set-up).
#[test]
fn luding_full_matches_liggghts() {
    let normal = GranularNormalModel::luding(material(), 1.0e4, 3.0, 0.4, 0.05, 0.001)
        .expect("valid luding");
    let model = GranularContactModel::new(
        normal,
        TangentialModel::luding_tn(0.8, 0.6, 0.5).expect("valid tan_luding"),
    )
    .with_rolling(RollingModel::luding(0.1, 0.3, 0.5).expect("valid rolling"));
    assert_collided("luding_full.csv");
    assert_bit_identical(&oblique_pair("luding_full.csv", model));
}

/// As above with `phi_f = 0.02`, `kc/k1 = 0.2`, `f_adh = 0`: `deltaMax` passes
/// `deltaMaxLim` (`k2 = k2Max`) and unloading reaches the cohesion branch,
/// which rewrites `deltaMax = (k2 + kc)/(k2 - k1) delta_n` (a different
/// formula from `hooke/hysteresis`).
#[test]
fn luding_plastic_cohesion_branch_matches_liggghts() {
    let normal =
        GranularNormalModel::luding(material(), 1.0e4, 3.0, 0.2, 0.02, 0.0).expect("valid luding");
    let model = GranularContactModel::new(
        normal,
        TangentialModel::luding_tn(0.8, 0.6, 0.5).expect("valid tan_luding"),
    )
    .with_rolling(RollingModel::luding(0.1, 0.3, 0.5).expect("valid rolling"));
    assert_collided("luding_plastic.csv");
    assert_bit_identical(&oblique_pair("luding_plastic.csv", model));
}

/// `kc = 0`, `f_adh = 0`, with the ordinary history tangential model: the
/// `limitForce` clamp (on by default for Luding) removes the negative normal
/// force.
#[test]
fn luding_limit_force_matches_liggghts() {
    let normal =
        GranularNormalModel::luding(material(), 1.0e4, 3.0, 0.0, 0.05, 0.0).expect("valid luding");
    let model = GranularContactModel::new(normal, TangentialModel::History);
    assert_collided("luding_nocoh.csv");
    assert_bit_identical(&oblique_pair("luding_nocoh.csv", model));
}

fn luding_full_model() -> GranularContactModel {
    let normal = GranularNormalModel::luding(material(), 1.0e4, 3.0, 0.4, 0.05, 0.001)
        .expect("valid luding");
    GranularContactModel::new(
        normal,
        TangentialModel::luding_tn(0.8, 0.6, 0.5).expect("valid tan_luding"),
    )
    .with_rolling(RollingModel::luding(0.1, 0.3, 0.5).expect("valid rolling"))
}

/// Luding against a wall (`in.luding_wall`): the wall branches of all three
/// models (`R* = r`, `m* = m`, `w_r` from the contact point).
#[test]
fn luding_wall_matches_liggghts() {
    let rows = load("luding_wall.csv").expect("reference present");
    assert!(
        rows.last().expect("frames")[6] > 0.0,
        "the sphere must bounce"
    );
    assert_bit_identical(&wall_case("luding_wall.csv", luding_full_model()));
}

/// Ablation controls for the Luding decks: replacing the Luding tangential
/// or rolling model by the ordinary ones, or switching rolling off, must NOT
/// reproduce the reference (the decks exercise those models).
#[test]
fn luding_decks_are_sensitive_to_each_sub_model() {
    let full = luding_full_model();
    let plain_tangential = GranularContactModel {
        tangential: TangentialModel::History,
        ..full
    };
    assert!(oblique_pair("luding_full.csv", plain_tangential).mismatched > 100);
    let no_rolling = GranularContactModel {
        rolling: RollingModel::Off,
        ..full
    };
    assert!(oblique_pair("luding_full.csv", no_rolling).mismatched > 100);
    let no_adhesion = GranularContactModel {
        normal: GranularNormalModel::luding(material(), 1.0e4, 3.0, 0.4, 0.05, 0.0)
            .expect("valid luding"),
        ..full
    };
    assert!(oblique_pair("luding_full.csv", no_adhesion).mismatched > 100);
}

// ---------------------------------------------------------------------------
// Thornton-Ning (gh:#1410): normal_model_thornton_ning.h.
// ---------------------------------------------------------------------------

fn tn_case(file: &str, gamma_surf: f64, yield_ratio: f64) -> Diff {
    assert_collided(file);
    let normal =
        GranularNormalModel::thornton_ning(material(), gamma_surf, yield_ratio).expect("valid");
    oblique_pair(
        file,
        GranularContactModel::new(normal, TangentialModel::History),
    )
}

/// `thornton_ning`, adhesive and yielding (`a_Y = 0.05 r_i`): virgin loading,
/// plastic branch, elastic unloading with the modified radius, detachment.
///
/// **Methodology.** Deck `in.tn_plastic`: `E = 1e7`, `nu = 0.3`, `e = 0.9`,
/// `gamma_s = 0.05 J/m^2`, yield ratio 0.05, oblique pair with relative
/// spins, history tangential model, 2500 steps, 251 frames bit for bit.
///
/// **Prediction (written before the first run, 2026-11-11).** Bit-identical
/// in all four decks (sums, products, `sqrt` and `pow(x, 1/3)` through the
/// same libm as upstream; the `goto` translated as a flag). The pair is
/// visited in one fixed order so the sign-flipping history never flips.
#[test]
fn thornton_ning_plastic_matches_liggghts() {
    assert_bit_identical(&tn_case("tn_plastic.csv", 0.05, 0.05));
}

/// `thornton_ning`, adhesive, never yields (`a_Y = 0.2 r_i`): the elastic JKR branches and the
/// `-(5/9) fc` detachment.
#[test]
fn thornton_ning_elastic_matches_liggghts() {
    assert_bit_identical(&tn_case("tn_elastic.csv", 0.05, 0.2));
}

/// `thornton_ning` with `gamma_s = 0`: no cohesion, `f` clipped at zero and
/// the detaching branch.
#[test]
fn thornton_ning_without_cohesion_matches_liggghts() {
    assert_bit_identical(&tn_case("tn_nocoh.csv", 0.0, 0.05));
}

/// `thornton_ning` against a wall (`in.tn_wall`).
#[test]
fn thornton_ning_wall_matches_liggghts() {
    let rows = load("tn_wall.csv").expect("reference present");
    assert!(
        rows.last().expect("frames")[6] > 0.0,
        "the sphere must bounce"
    );
    let normal = GranularNormalModel::thornton_ning(material(), 0.05, 0.05).expect("valid");
    assert_bit_identical(&wall_case(
        "tn_wall.csv",
        GranularContactModel::new(normal, TangentialModel::History),
    ));
}

/// The three TN decks are told apart: a model with a different yield ratio or
/// surface energy does not reproduce them (so they exercise the yield and
/// adhesion branches), and the plastic and elastic decks differ from each other.
#[test]
fn thornton_ning_decks_are_sensitive() {
    assert!(
        tn_case("tn_plastic.csv", 0.05, 0.2).mismatched > 100,
        "yield branch not exercised"
    );
    assert!(
        tn_case("tn_plastic.csv", 0.02, 0.05).mismatched > 100,
        "adhesion not exercised"
    );
    assert!(
        tn_case("tn_nocoh.csv", 0.05, 0.05).mismatched > 100,
        "gamma_s = 0 not distinct"
    );
}

// ---------------------------------------------------------------------------
// Edinburgh elasto-plastic adhesive (gh:#1410): normal_model_edinburgh.h and
// normal_model_edinburgh_stiffness.h.
// ---------------------------------------------------------------------------

#[allow(clippy::too_many_arguments)]
fn edinburgh_case(
    file: &str,
    stiffness: Option<f64>,
    kn2kc: f64,
    exponent: f64,
    f_adh: f64,
    fix_kc: bool,
) -> Diff {
    assert_collided(file);
    let normal = GranularNormalModel::edinburgh(
        material(),
        stiffness,
        3.0,
        kn2kc,
        exponent,
        exponent,
        f_adh,
        0.05,
    )
    .expect("valid edinburgh")
    .with_fix_kc(fix_kc);
    oblique_pair(
        file,
        GranularContactModel::new(normal, TangentialModel::History),
    )
}

/// `edinburgh/stiffness`, linear (`dex = cex = 1`), `fixKc off`: the
/// surface-energy-driven adhesive stiffness (`calculate_k_adh`, the
/// `goto temp_calc` re-evaluation) with a pull-off force.
///
/// **Methodology.** Deck `in.edi_stiff_lin`: `K = 1e4 N/m`, `k2/k1 = 3`,
/// `gamma_s = 0.05 J/m^2`, `f_adh = 5e-4 N`, oblique pair with relative
/// spins, history tangential, 2500 steps, 251 frames bit for bit.
///
/// **Prediction (written before the first run, 2026-11-11).** Bit-identical
/// in all three decks: sums, products, `sqrt` and `pow` through the same libm
/// as upstream, the `goto` as a loop. The sticky nonlinear deck (the pair
/// ends bound together, `v_x = +7e-4 m/s`) is the one most likely to expose
/// a branch error.
#[test]
fn edinburgh_stiffness_linear_matches_liggghts() {
    assert_bit_identical(&edinburgh_case(
        "edi_stiff_lin.csv",
        Some(1.0e4),
        0.3,
        1.0,
        0.0005,
        false,
    ));
}

/// `edinburgh` (Hertzian `k_n = 4/3 Y_eff sqrt(R*)`, `dex = cex = 1.5`),
/// `fixKc off`.
#[test]
fn edinburgh_nonlinear_matches_liggghts() {
    assert_bit_identical(&edinburgh_case(
        "edi_nonlin.csv",
        None,
        0.3,
        1.5,
        0.0,
        false,
    ));
}

/// `edinburgh/stiffness fixKc on`: `k_c = kn2kc k_n`, with upstream's
/// shadowed `k_adh` (the `limitForce` test sees zero).
#[test]
fn edinburgh_fix_kc_matches_liggghts() {
    assert_bit_identical(&edinburgh_case(
        "edi_fixkc.csv",
        Some(1.0e4),
        0.3,
        1.0,
        0.0,
        true,
    ));
}

/// Controls: the Edinburgh decks are sensitive to the surface energy
/// (`fixKc off`) and to the exponent.
#[test]
fn edinburgh_decks_are_sensitive() {
    let alt =
        GranularNormalModel::edinburgh(material(), Some(1.0e4), 3.0, 0.3, 1.0, 1.0, 0.0005, 0.02)
            .expect("valid");
    let d = oblique_pair(
        "edi_stiff_lin.csv",
        GranularContactModel::new(alt, TangentialModel::History),
    );
    assert!(d.mismatched > 100, "surface energy not exercised");
    let d = edinburgh_case("edi_fixkc.csv", Some(1.0e4), 0.3, 1.0, 0.0, false);
    assert!(d.mismatched > 100, "fixKc on/off not distinct");
}

// ---------------------------------------------------------------------------
// Cohesion (gh:#1411): cohesion_model_sjkr.h, cohesion_model_sjkr2.h.
// ---------------------------------------------------------------------------

fn hertz_with(cohesion: GranularCohesion) -> GranularContactModel {
    GranularContactModel::hertz_history(material()).with_cohesion(cohesion)
}

/// `cohesion sjkr` (simplified JKR): `F_coh = -k_A A_contact` with the exact
/// two-sphere contact area `-(pi/4) (r-ri-rj)(r+ri-rj)(r-ri+rj)(r+ri+rj)/r^2`.
///
/// **Methodology.** Deck `in.sjkr_pair`: Hertz + history, `E = 1e7`,
/// `k_A = 2e5 J/m^3`, oblique pair with relative spins, 2500 steps, 251
/// frames of x, v, omega bit for bit. The cohesion force acts along `e_n`
/// between the normal and tangential model (`i_forces.delta_F +=`), so the
/// force sum is `(Fn en + Fcoh en) + Ft`.
///
/// **Prediction (written before the first run, 2026-11-11).** Bit-identical
/// in all five decks: the area is a product/quotient chain translated in
/// upstream's operator order, and the three force terms are summed in
/// upstream's order.
#[test]
fn sjkr_pair_matches_liggghts() {
    let model = hertz_with(GranularCohesion::sjkr(2.0e5).expect("valid"));
    assert_bit_identical(&oblique_pair("sjkr_pair.csv", model));
}

/// `cohesion sjkr2` (spherical-cap area `2 pi R* delta`).
#[test]
fn sjkr2_pair_matches_liggghts() {
    let model = hertz_with(GranularCohesion::sjkr2(2.0e5).expect("valid"));
    assert_bit_identical(&oblique_pair("sjkr2_pair.csv", model));
}

/// `cohesion sjkr tangential_reduce on`: the cohesion force is added to
/// `sidata.Fn`, so the tangential Coulomb limit sees it.
#[test]
fn sjkr_tangential_reduce_matches_liggghts() {
    let model = hertz_with(
        GranularCohesion::sjkr(2.0e5)
            .expect("valid")
            .with_tangential_reduce(true),
    );
    assert_bit_identical(&oblique_pair("sjkr_reduce.csv", model));
}

/// SJKR and SJKR2 against a wall: the sphere-wall contact areas
/// `(ri^2 - r^2) pi` and `2 pi ri (ri - r)` with `r = ri - delta`.
#[test]
fn sjkr_and_sjkr2_wall_match_liggghts() {
    for (file, cohesion) in [
        (
            "sjkr_wall.csv",
            GranularCohesion::sjkr(2.0e5).expect("valid"),
        ),
        (
            "sjkr2_wall.csv",
            GranularCohesion::sjkr2(2.0e5).expect("valid"),
        ),
    ] {
        let rows = load(file).expect("reference present");
        assert!(
            rows.last().expect("frames")[6] > 0.0,
            "{file}: the sphere must bounce"
        );
        assert_bit_identical(&wall_case(file, hertz_with(cohesion)));
    }
}

/// Controls: cohesion off, SJKR vs SJKR2 and `tangential_reduce` are told
/// apart by the decks.
#[test]
fn cohesion_decks_are_sensitive() {
    let off = GranularContactModel::hertz_history(material());
    assert!(oblique_pair("sjkr_pair.csv", off).mismatched > 100);
    let swapped = hertz_with(GranularCohesion::sjkr2(2.0e5).expect("valid"));
    assert!(oblique_pair("sjkr_pair.csv", swapped).mismatched > 100);
    let plain = hertz_with(GranularCohesion::sjkr(2.0e5).expect("valid"));
    assert!(oblique_pair("sjkr_reduce.csv", plain).mismatched > 0);
}

/// Long Edinburgh runs (20 000 steps, 251 frames at spacing 80): a slow
/// approach (`v = +-(0.3, 0.1) m/s`) that leaves the pair bound and
/// oscillating, so the contact is reloaded many times — the
/// `deltan > old_delta` re-evaluation (`goto temp_calc`, `count_flag = 1`)
/// of `fixKc off`. **Ablation (2026-11-11):** with that re-evaluation
/// disabled in the port the four short decks above still pass, so they never
/// reach it; these two decks do not (see the V&V section).
#[test]
fn edinburgh_long_oscillation_matches_liggghts() {
    for (file, stiffness, exponent, f_adh) in [
        ("edi_long_stiff_lin.csv", Some(1.0e4), 1.0, 0.0005),
        ("edi_long_nonlin.csv", None, 1.5, 0.0),
    ] {
        assert_collided_below(file, 0.3);
        let rows = load(file).expect("reference present");
        assert_eq!(rows.len(), 251);
        let normal = GranularNormalModel::edinburgh(
            material(),
            stiffness,
            3.0,
            0.3,
            exponent,
            exponent,
            f_adh,
            0.05,
        )
        .expect("valid edinburgh");
        let model = GranularContactModel::new(normal, TangentialModel::History);
        let diff = replay_pair_at(
            &rows,
            model,
            0.0060,
            Vec3::new(0.3, 0.1, 0.0),
            Vec3::new(-0.3, -0.1, 0.0),
            Vec3::new(0.0, 0.0, 40.0),
            Vec3::new(0.0, 0.0, -20.0),
        );
        assert_bit_identical(&diff);
    }
}

/// The long reference left its initial `+0.3 m/s`.
fn assert_collided_below(file: &str, v0: f64) {
    let rows = load(file).expect("reference present");
    let vx_end = rows.last().expect("frames")[4];
    assert!(
        vx_end < 0.9 * v0,
        "{file}: no collision in the reference (vx = {vx_end})"
    );
}

// ---------------------------------------------------------------------------
// gh:#1410 (2026-11-11): TWO-TYPE property tables for the newer contact
// models. Decks `in.mt_*` (generated by `make_multitype_decks.sh`).
// ---------------------------------------------------------------------------

/// `E = (1e7, 3e7)`, `nu = (0.3, 0.2)`, `e` and `mu` matrices as in
/// `in.twotype_oblique`.
fn mt_materials() -> MaterialTable {
    MaterialTable::new(
        &[(1.0e7, 0.3), (3.0e7, 0.2)],
        &[&[0.9, 0.7], &[0.7, 0.5]],
        &[&[0.5, 0.3], &[0.3, 0.2]],
    )
    .expect("valid table")
}

/// **Result (2026-11-11, Rust 1.99.1, upstream g++ 15.2.1).** Predictions
/// confirmed at the first run: all eight decks (`mt_epsd`, `mt_epsd2`,
/// `mt_epsd3`, `mt_luding`, `mt_tn`, `mt_edi`, `mt_sjkr`, `mt_sjkr2`)
/// 4518 of 4518 `f64` bit-identical each (251 frames x 2 atoms x 9 values).
/// The sensitivity test shows the per-pair coefficients are carried.
///
/// Run the two-type oblique pair (type 1: `d = 10 mm`, `rho = 2500`; type 2:
/// `d = 12 mm`, `rho = 4000`; spins `+40`, `-20`) with `model`, which must
/// carry the materials and pair-model tables.
fn mt_run(file: &str, model: GranularContactModel) -> Diff {
    assert_collided(file);
    let rows = load(file).unwrap_or_else(|| panic!("{file} missing"));
    assert_eq!(rows.len(), 251, "unexpected reference frame count");
    let mut pa = sphere_rho(
        0.005,
        2500.0,
        Vec3::new(-0.0060, 0.0, 0.0),
        Vec3::new(1.0, 0.5, 0.0),
    );
    let mut pb = sphere_rho(
        0.006,
        4000.0,
        Vec3::new(0.0060, 0.0, 0.0),
        Vec3::new(-1.0, -0.5, 0.0),
    );
    pa.angular_velocity = Vec3::new(0.0, 0.0, 40.0);
    pb.angular_velocity = Vec3::new(0.0, 0.0, -20.0);
    let mut sys = GranularSystem::new(vec![pa, pb], vec![], model, Vec3::zero(), 1.0e-6)
        .expect("valid system")
        .with_particle_types(vec![0, 1])
        .expect("valid types");
    replay_system(&mut sys, &rows)
}

/// Symmetric 2x2 matrix entry from `[m00, m01 = m10, m11]`.
fn sym(m: [f64; 3], i: usize, j: usize) -> f64 {
    m[i + j]
}

/// Build the 2x2 [`PairModelTable`] from a per-`(i, j)` constructor.
fn mt_table(f: impl Fn(usize, usize) -> PairModel) -> PairModelTable {
    let mut t = PairModelTable::new(2).expect("2 types");
    for i in 0..2 {
        for j in 0..2 {
            t = t.set(i, j, f(i, j)).expect("in range");
        }
    }
    t
}

/// Hooke (`vchar = 2`) + history normal and tangential; rolling per pair.
fn mt_rolling_case(file: &str, rolling: impl Fn(usize, usize) -> RollingModel) -> Diff {
    let table = mt_table(|i, j| PairModel {
        normal: hooke_history(),
        tangential: TangentialModel::History,
        rolling: rolling(i, j),
        cohesion: GranularCohesion::Off,
    });
    let model = GranularContactModel::new(hooke_history(), TangentialModel::History)
        .with_rolling(rolling(0, 0))
        .with_materials(mt_materials())
        .with_pair_models(table)
        .expect("kinds match");
    mt_run(file, model)
}

/// Two-type `rolling_friction epsd`: per-pair `mu_r` (0.1, 0.15, 0.2) and
/// rolling viscosity (0.3, 0.4, 0.5) on top of the per-pair `Y_eff`,
/// `e`, `mu` of the Hooke/history contact.
///
/// **Prediction (written before the first run, 2026-11-11).** Bit-identical:
/// the pair table only selects which scalar enters the already-verified
/// single-type formulas, the way upstream's `coeffRollFrict[itype][jtype]`
/// does.
#[test]
fn two_type_epsd_matches_liggghts() {
    let d = mt_rolling_case("mt_epsd.csv", |i, j| {
        RollingModel::epsd(sym([0.1, 0.15, 0.2], i, j), sym([0.3, 0.4, 0.5], i, j))
            .expect("valid")
    });
    assert_bit_identical(&d);
}

/// Two-type `epsd2` (per-pair `mu_r`). Prediction as [`two_type_epsd_matches_liggghts`].
#[test]
fn two_type_epsd2_matches_liggghts() {
    let d = mt_rolling_case("mt_epsd2.csv", |i, j| {
        RollingModel::epsd2(sym([0.1, 0.15, 0.2], i, j)).expect("valid")
    });
    assert_bit_identical(&d);
}

/// Two-type `epsd3` (per-pair `mu_r`, viscosity; scalar stiffness 5).
/// Prediction as [`two_type_epsd_matches_liggghts`].
#[test]
fn two_type_epsd3_matches_liggghts() {
    let d = mt_rolling_case("mt_epsd3.csv", |i, j| {
        RollingModel::epsd3(sym([0.1, 0.15, 0.2], i, j), sym([0.3, 0.4, 0.5], i, j), 5.0)
            .expect("valid")
    });
    assert_bit_identical(&d);
}

/// Two-type `luding` + `tan_luding` + `rolling luding`, every coefficient
/// a per-pair matrix (`K`, `k2/k1`, `kc/k1`, `phi_f`, `f_0`, `mu_d/mu_s`,
/// friction viscosity, `kT2kcMax`, rolling `mu_r`, viscosity, `kR2kcMax`).
///
/// **Prediction (written before the first run, 2026-11-11).** Bit-identical.
#[test]
fn two_type_luding_matches_liggghts() {
    let pm = |i: usize, j: usize| {
        let normal = GranularNormalModel::luding(
            material(),
            sym([1.0e4, 1.5e4, 2.0e4], i, j),
            sym([3.0, 2.5, 2.0], i, j),
            sym([0.4, 0.3, 0.2], i, j),
            sym([0.05, 0.04, 0.03], i, j),
            sym([0.001, 0.002, 0.003], i, j),
        )
        .expect("valid luding");
        PairModel {
            normal,
            tangential: TangentialModel::luding_tn(
                sym([0.8, 0.7, 0.6], i, j),
                sym([0.6, 0.5, 0.4], i, j),
                sym([0.5, 0.6, 0.7], i, j),
            )
            .expect("valid"),
            rolling: RollingModel::luding(
                sym([0.1, 0.15, 0.2], i, j),
                sym([0.3, 0.4, 0.5], i, j),
                sym([0.5, 0.6, 0.7], i, j),
            )
            .expect("valid"),
            cohesion: GranularCohesion::Off,
        }
    };
    let e = pm(0, 0);
    let model = GranularContactModel::new(e.normal, e.tangential)
        .with_rolling(e.rolling)
        .with_materials(mt_materials())
        .with_pair_models(mt_table(pm))
        .expect("kinds match");
    assert_bit_identical(&mt_run("mt_luding.csv", model));
}

/// Two-type `thornton_ning`: per-pair surface energy (0.05, 0.04, 0.03) and
/// the per-TYPE yield ratio (0.05, 0.08) that upstream indexes by `itype`.
///
/// **Prediction (written before the first run, 2026-11-11).** Bit-identical
/// provided the port visits the pair in LIGGGHTS' `(itype, jtype)` order; a
/// mismatch would show in the yield radius `a_Y = r_i yield_ratio[itype]`.
#[test]
fn two_type_thornton_ning_matches_liggghts() {
    let yr = [0.05, 0.08];
    let pm = |i: usize, j: usize| PairModel {
        normal: GranularNormalModel::thornton_ning(
            material(),
            sym([0.05, 0.04, 0.03], i, j),
            yr[i],
        )
        .expect("valid"),
        tangential: TangentialModel::History,
        rolling: RollingModel::Off,
        cohesion: GranularCohesion::Off,
    };
    let e = pm(0, 0);
    let model = GranularContactModel::new(e.normal, e.tangential)
        .with_materials(mt_materials())
        .with_pair_models(mt_table(pm))
        .expect("kinds match");
    assert_bit_identical(&mt_run("mt_tn.csv", model));
}

/// Two-type `edinburgh` (nonlinear, `fixKc` off): per-pair `k2/k1`, `kc/kn`,
/// `f_0` and surface energy, scalar exponents 1.5.
///
/// **Prediction (written before the first run, 2026-11-11).** Bit-identical.
#[test]
fn two_type_edinburgh_matches_liggghts() {
    let pm = |i: usize, j: usize| PairModel {
        normal: GranularNormalModel::edinburgh(
            material(),
            None,
            sym([3.0, 2.5, 2.0], i, j),
            sym([0.3, 0.25, 0.2], i, j),
            1.5,
            1.5,
            sym([0.0, 0.001, 0.002], i, j),
            sym([0.05, 0.04, 0.03], i, j),
        )
        .expect("valid"),
        tangential: TangentialModel::History,
        rolling: RollingModel::Off,
        cohesion: GranularCohesion::Off,
    };
    let e = pm(0, 0);
    let model = GranularContactModel::new(e.normal, e.tangential)
        .with_materials(mt_materials())
        .with_pair_models(mt_table(pm))
        .expect("kinds match");
    assert_bit_identical(&mt_run("mt_edi.csv", model));
}

/// Two-type `cohesion sjkr` and `sjkr2`: per-pair energy density
/// (2e5, 3e5, 4e5 J/m^3).
///
/// **Prediction (written before the first run, 2026-11-11).** Bit-identical.
#[test]
fn two_type_sjkr_and_sjkr2_match_liggghts() {
    for (file, two) in [("mt_sjkr.csv", false), ("mt_sjkr2.csv", true)] {
        let pm = |i: usize, j: usize| {
            let kd = sym([2.0e5, 3.0e5, 4.0e5], i, j);
            PairModel {
                normal: GranularNormalModel::hertz(material()),
                tangential: TangentialModel::History,
                rolling: RollingModel::Off,
                cohesion: if two {
                    GranularCohesion::sjkr2(kd).expect("valid")
                } else {
                    GranularCohesion::sjkr(kd).expect("valid")
                },
            }
        };
        let e = pm(0, 0);
        let model = GranularContactModel::new(e.normal, e.tangential)
            .with_cohesion(e.cohesion)
            .with_materials(mt_materials())
            .with_pair_models(mt_table(pm))
            .expect("kinds match");
        assert_bit_identical(&mt_run(file, model));
    }
}

/// The pair table is genuinely used: forcing every pair to the (0,0)
/// coefficients changes the Thornton-Ning trajectory, so the passing tests
/// above are not vacuous. A table entry that changes a model kind is refused.
#[test]
fn two_type_pair_table_is_sensitive() {
    let pm = |_i: usize, _j: usize| PairModel {
        normal: GranularNormalModel::thornton_ning(material(), 0.05, 0.05).expect("valid"),
        tangential: TangentialModel::History,
        rolling: RollingModel::Off,
        cohesion: GranularCohesion::Off,
    };
    let e = pm(0, 0);
    let model = GranularContactModel::new(e.normal, e.tangential)
        .with_materials(mt_materials())
        .with_pair_models(mt_table(pm))
        .expect("kinds match");
    assert!(!mt_run("mt_tn.csv", model).bit_identical());
    let bad = mt_table(|_, _| PairModel {
        normal: hooke_history(),
        tangential: TangentialModel::History,
        rolling: RollingModel::Off,
        cohesion: GranularCohesion::Off,
    });
    assert!(GranularContactModel::new(
        GranularNormalModel::hertz(material()),
        TangentialModel::History
    )
    .with_pair_models(bad)
    .is_err());
}
