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

//! # `fix mesh/surface/stress/servo` against upstream (gh:#1416)
//!
//! `MeshModuleStressServo` (`mesh_module_stress_servo.cpp`, LIGGGHTS-PUBLIC
//! `3d5c00f2`, `ctrlPV force`, PID controller) against
//! [`outram_park_fork_liggghts::mesh_stress::MeshServo`].
//!
//! ## Methodology
//!
//! Deck `reference-data/liggghts/in.mesh_servo`: two `d = 10 mm` spheres
//! (`vx = 0.02` on one) rest on a floor wall `z = 0`; a 0.3 m square plate
//! starting at `z = 0.0112` descends under the servo (`axis 0 0 1`,
//! `target_val 0.1`, `vel_max 0.05`, `kp 1`, `ki 0.2`, `kd 1e-4`), 20 000
//! steps of `dt = 1e-5`, `E = 1e7`, `nu = 0.3`, `e = 0.9`, `mu = 0.5`, skin
//! 3 mm. Compared bit for bit: 501 trajectory frames (`mesh_servo.csv`) and,
//! every 250 steps, `f_plate[1..12]` (force, torque, `p_ref`, servo centre
//! `xcm`; `mesh_servo_thermo.txt`, `%.17g`).
//!
//! **Prediction (written before the first run, 2026-11-11).** Bit-identical:
//! the controller is a fixed sequence of IEEE operations on the total force,
//! which is a two-term sum per step (order independent), and `mesh->move`
//! is `node_orig + dX`, reproduced operation for operation.

use outram_park_fork_liggghts::granular::{GranularContactModel, GranularMaterial};
use outram_park_fork_liggghts::granular_system::{FixSlot, GranularSystem};
use outram_park_fork_liggghts::boundary::Boundary;
use outram_park_fork_liggghts::mesh_stress::{MeshServo, ServoModify};
use outram_park_fork_liggghts::mesh_wall::{MeshWall, MovingBoundary, WallGeometry};
use outram_park_fork_liggghts::particle::{Particle, Vec3};
use outram_park_fork_liggghts::variable::VariableTable;
use uom::si::f64::{Length, Mass, ThermodynamicTemperature};
use uom::si::{length::meter, mass::kilogram, thermodynamic_temperature::kelvin};

fn read(name: &str) -> Option<String> {
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../reference-data/liggghts")
        .join(name);
    std::fs::read_to_string(path).ok()
}

fn sphere(row: &[f64], k: usize, r: f64) -> Particle {
    let c = &row[1 + 9 * k..1 + 9 * k + 9];
    Particle::new(
        Vec3::new(c[0], c[1], c[2]),
        Vec3::new(c[3], c[4], c[5]),
        Vec3::zero(),
        Mass::new::<kilogram>(4.0 * std::f64::consts::PI / 3.0 * r * r * r * 2500.0),
        Length::new::<meter>(r),
        ThermodynamicTemperature::new::<kelvin>(300.0),
    )
    .expect("valid sphere")
}

fn table(text: &str, sep_comma: bool, skip: usize) -> Vec<Vec<f64>> {
    text.lines()
        .skip(skip)
        .filter(|l| !l.trim().is_empty())
        .map(|l| {
            if sep_comma {
                l.split(',').map(|v| v.parse().expect("numeric")).collect()
            } else {
                l.split_whitespace()
                    .map(|v| v.parse().expect("numeric"))
                    .collect()
            }
        })
        .collect()
}

/// Servo plate pressing two spheres: trajectories (501 frames) and the twelve
/// `f_plate` values at 81 steps bit-identical to LIGGGHTS.
///
/// Result (2026-11-11, Rust 1.99.1, LIGGGHTS 3d5c00f2): see the assertion
/// message. Measured: **0 of 9 990 compared values differ** (501 frames x 18
/// + 81 x 12). Deck order: floor, then the mesh wall (the crate's default).
#[test]
fn mesh_servo_matches_liggghts() {
    run_servo(&Case::force_pid("mesh_servo", "plate_servo.stl", false));
}

/// The same deck with the **mesh wall declared before the floor**
/// (`in.mesh_servo_rev`, gh:#1482): LIGGGHTS applies `post_force` in
/// declaration order (`Modify::post_force`), and the per-contact
/// `f_pw = f_after - f_before` rounds differently, so its trajectory differs
/// from the forward deck (thermo line 11 first differs). The port follows via
/// `GranularSystem::with_fix_order`.
///
/// Before the fix (fixed order, primitive walls first): 1890 of 9 990 values
/// differed by 1 ulp in force. Result 2026-11-11: see the assertion.
#[test]
fn mesh_servo_reverse_declaration_order_matches_liggghts() {
    run_servo(&Case::force_pid("mesh_servo_rev", "plate_servo.stl", true));
}

/// `ctrlPV torque` (PID): the plate rotates about `z` through `com`, nodes
/// rebuilt as the original nodes rotated by the TOTAL quaternion, cached
/// edge vectors and normals rotated by the incremental one; node velocity
/// `omega x (node - xcm)`. Deck `in.mesh_servo_torque` (two sliding spheres
/// pinched between plate and floor). Result: see the assertion message.
#[test]
fn mesh_servo_torque_matches_liggghts() {
    let mut c = Case::force_pid("mesh_servo_torque", "plate_torque.stl", false);
    c.torque = true;
    c.com = [0.0, 0.0, 0.0099];
    c.target = 0.0001;
    c.expect_moved = 1.0e-5;
    run_servo(&c);
}

/// `mode auto` on the force servo: cruise at `ctrl_op_max` while the mesh
/// neighbour list holds no contact, then the piecewise gain (needs
/// `getTotalNumContacts`, recounted on each neighbour-list rebuild). Deck
/// `in.mesh_servo_auto`, `target_val -0.1`, `ratio 5e-5`.
#[test]
fn mesh_servo_auto_matches_liggghts() {
    let mut c = Case::force_pid("mesh_servo_auto", "plate_auto.stl", false);
    c.auto_ratio = Some(0.00005);
    c.gains = [0.01, 0.0, 0.0];
    c.com = [0.0, 0.0, 0.015];
    c.target = -0.1;
    run_servo(&c);
}

/// `ctrlPV torque` with `mode auto` (`ctrl_op_min = vel_min / rPaMax`).
#[test]
fn mesh_servo_torque_auto_matches_liggghts() {
    let mut c = Case::force_pid("mesh_servo_torque_auto", "plate_torque.stl", false);
    c.torque = true;
    c.auto_ratio = Some(0.00005);
    c.gains = [0.01, 0.0, 0.0];
    c.com = [0.0, 0.0, 0.0099];
    c.target = 0.0001;
    c.expect_moved = 1.0e-5;
    run_servo(&c);
}

/// A VARIABLE set point (gh:#1491): `in.mesh_servo_var` is `in.mesh_servo`
/// with `target_val v_tv`, `tv = 0.1+0.05*sin(2*PI*step/4000)`, evaluated at
/// the start of every `final_integrate` (`sp_style_ == EQUAL`).
///
/// **Prediction (2026-11-12, before the first run):** 9 990 / 9 990 values
/// bit-identical (the evaluator is already bit-identical on the variable
/// movers). **Missed.**
///
/// **Result (2026-11-12, Rust 1.99.1):** 9 963 / 9 990 bit-identical; 27
/// trajectory values differ, the first at step 17 360 (`vz` of sphere 1,
/// 8.308702760360973e-6 against 8.308702760360986e-6); every `f_plate` value
/// at every thermo row is bit-identical. **Cause, demonstrated:** the
/// variable's `sin` is PETIR's `r_sin` (the `libm` crate, a musl port), not
/// glibc's. Over this deck's 20 000 arguments `libm::sin` and glibc's `sin`
/// differ by one ulp at 611, and the set point `0.1+0.05*sin(..)` at 211, the
/// first at step 1 117 (a scratch A/B program). The servo is saturated at
/// `vel_max` while it cruises, so the different set point shows only once it
/// controls. A/B in the port: with the evaluator's `sin` switched to std's
/// (glibc) `f64::sin`, **9 990 / 9 990 bit-identical**; reverted, because the
/// maintainer kept PETIR's maths (gh:#1455, 2026-11-11: such residuals are
/// accepted when demonstrated, counted and pinned). Pinned here at 27.
#[test]
fn mesh_servo_variable_target_matches_liggghts() {
    let mut c = Case::force_pid("mesh_servo_var", "plate_servo.stl", false);
    c.target_var = Some("0.1+0.05*sin(2*PI*step/4000)");
    c.expected_differ = 27;
    run_servo(&c);
}

/// **Two `run` commands, no `fix_modify`** (gh:#1510): `in.mesh_servo_2run`
/// is `in.mesh_servo` as `run 10000` + `run 10000`. At the second `run`
/// LIGGGHTS re-runs every fix's `init()` and `Verlet::setup()`; the port does
/// the same with `GranularSystem::begin_run`. Compared as the single-run
/// deck, plus the thermo line printed at the second run's set-up (its
/// `f_plate[1..6]` are the last step's totals plus the set-up contributions).
///
/// **Prediction (2026-11-13, before the first run):** bit-identical.
/// **Result:** see the assertion message and `run_servo`'s print.
#[test]
fn mesh_servo_two_runs_match_liggghts() {
    let mut c = Case::force_pid("mesh_servo_2run", "plate_servo.stl", false);
    c.runs = vec![(10_000, Vec::new())];
    run_servo(&c);
}

/// Control for [`mesh_servo_two_runs_match_liggghts`]: one continuous run
/// (no `begin_run`) against the two-run reference must differ, or the
/// two-run deck tests nothing.
#[test]
fn continuous_run_differs_from_two_runs() {
    let mut c = Case::force_pid("mesh_servo_2run", "plate_servo.stl", false);
    c.runs = vec![(10_000, Vec::new())];
    c.continuous_control = true;
    run_servo(&c);
}

/// The torque servo over two runs (`in.mesh_servo_torque_2run`, gh:#1510):
/// the second `init()` recomputes `rPaMax` from the rotated nodes and the
/// set-up restarts the total rotation from them (`node_orig` = present
/// nodes, `totalPhi = 0`).
#[test]
fn mesh_servo_torque_two_runs_match_liggghts() {
    let mut c = Case::force_pid("mesh_servo_torque_2run", "plate_torque.stl", false);
    c.torque = true;
    c.com = [0.0, 0.0, 0.0099];
    c.target = 0.0001;
    c.expect_moved = 1.0e-5;
    c.runs = vec![(10_000, Vec::new())];
    run_servo(&c);
}

/// The `fix_modify` of `in.mesh_servo_modify` (gh:#1510): at 8000 a new
/// constant set point, new gains and a lower `vel_max`; at 14000 the servo
/// stops.
fn modify_runs() -> Vec<(usize, Vec<ServoModify>)> {
    vec![
        (
            8_000,
            vec![
                ServoModify::TargetVal(0.15),
                ServoModify::CtrlParam([0.5, 0.4, 0.0002]),
                ServoModify::VelMax(0.03),
            ],
        ),
        (14_000, vec![ServoModify::Integrate(false)]),
    ]
}

/// **Servo `fix_modify` between runs** (gh:#1510): `in.mesh_servo_modify` is
/// `in.mesh_servo` as `run 8000`; `fix_modify plate servo/target_val 0.15
/// servo/ctrlParam 0.5 0.4 0.0002 servo/vel_max 0.03`; `run 6000`;
/// `fix_modify plate servo/integrate stop`; `run 6000`
/// (`MeshModuleStressServo::modify_param`, then `init()` at the next run).
///
/// **Prediction (2026-11-13, before the first run):** bit-identical.
/// **Result:** see the assertion message and `run_servo`'s print.
#[test]
fn mesh_servo_fix_modify_matches_liggghts() {
    let mut c = Case::force_pid("mesh_servo_modify", "plate_servo.stl", false);
    c.runs = modify_runs();
    run_servo(&c);
}

/// Control for [`mesh_servo_fix_modify_matches_liggghts`]: the same three
/// runs WITHOUT the `fix_modify` must miss the reference.
#[test]
fn servo_runs_without_fix_modify_differ() {
    let mut c = Case::force_pid("mesh_servo_modify", "plate_servo.stl", false);
    c.runs = modify_runs().into_iter().map(|(b, _)| (b, Vec::new())).collect();
    c.continuous_control = true;
    let mut c2 = Case::force_pid("mesh_servo_modify", "plate_servo.stl", false);
    c2.runs = c.runs.clone();
    // begin_run at the boundaries but no fix_modify: still must differ
    c2.must_differ = true;
    run_servo(&c);
    run_servo(&c2);
}

/// `fix_modify plate servo/ratio 0.0001` between two runs of the `mode auto`
/// deck (`in.mesh_servo_auto_modify`, gh:#1510): the new `ratio` enters
/// `vel_min_ = ratio r_min / dt` at the second `init()`.
#[test]
fn mesh_servo_auto_ratio_modify_matches_liggghts() {
    let mut c = Case::force_pid("mesh_servo_auto_modify", "plate_auto.stl", false);
    c.auto_ratio = Some(0.00005);
    c.gains = [0.01, 0.0, 0.0];
    c.com = [0.0, 0.0, 0.015];
    c.target = -0.1;
    c.runs = vec![(10_000, vec![ServoModify::Ratio(0.0001)])];
    run_servo(&c);
    // control: the second run without the new ratio misses the reference
    c.runs = vec![(10_000, Vec::new())];
    c.must_differ = true;
    run_servo(&c);
}

/// `ServoModify` and `begin_run` refuse what upstream aborts on, and a
/// `fix_modify` on a wall without a servo is an error (gh:#1510).
#[test]
fn servo_fix_modify_rejects_upstream_errors() {
    let mut s = MeshServo::new([0.0; 3], [0.0, 0.0, 1.0], 0.1, 0.05, [1.0, 0.0, 0.0], 1.0e-5, 0.003);
    assert!(s.modify_param(ServoModify::VelMax(0.0)).is_err());
    assert!(s.modify_param(ServoModify::TargetVal(0.0)).is_err());
    let nodes = [[[1.0, 0.0, 0.0], [0.0, 1.0, 0.0], [0.0, 0.0, 0.0]]];
    // all gains zero: init() aborts
    s.modify_param(ServoModify::CtrlParam([0.0, 0.0, 0.0])).expect("accepted by modify_param");
    assert!(s.init_run(0.005, &nodes).is_err());
    s.modify_param(ServoModify::CtrlParam([1.0, 0.0, 0.0])).expect("valid gains");
    // vel_max >= skin / (2 dt) = 150: init() aborts
    s.modify_param(ServoModify::VelMax(200.0)).expect("accepted by modify_param");
    assert!(s.init_run(0.005, &nodes).is_err());
    s.modify_param(ServoModify::VelMax(0.05)).expect("valid vel_max");
    assert!(s.init_run(0.005, &nodes).is_ok());
    // auto mode needs a ratio
    let mut a = MeshServo::new([0.0; 3], [0.0, 0.0, 1.0], 0.1, 0.05, [1.0, 0.0, 0.0], 1.0e-5, 0.003)
        .with_auto(0.0001, 0.005);
    a.modify_param(ServoModify::Ratio(0.0)).expect("accepted by modify_param");
    assert!(a.init_run(0.005, &nodes).is_err());
    // a stopped servo neither moves nor changes its velocity
    let mut st = MeshServo::new([0.0; 3], [0.0, 0.0, 1.0], 0.1, 0.05, [1.0, 0.0, 0.0], 1.0e-5, 0.003);
    let v = st.final_integrate([0.0; 3]);
    st.modify_param(ServoModify::Integrate(false)).expect("stop");
    assert!(!st.is_integrating());
    assert_eq!(st.final_integrate([0.0, 0.0, 5.0]), v);
    assert_eq!(st.vcm(), v);
    let floor =
        Boundary::wall(Vec3::new(0.0, 0.0, 0.0), Vec3::new(0.0, 0.0, 1.0)).expect("valid wall");
    let material = GranularMaterial::new(1.0e7, 0.3, 0.9, 0.5).expect("valid material");
    let mut sys = GranularSystem::new(
        Vec::new(),
        vec![floor],
        GranularContactModel::hertz_history(material),
        Vec3::zero(),
        1.0e-5,
    )
    .expect("valid system");
    assert!(sys.fix_modify_servo(0, ServoModify::Ratio(1.0)).is_err());
    assert!(sys.begin_run().is_ok());
}

/// One servo deck: its reference files, controller and plate.
struct Case {
    deck: &'static str,
    stl: &'static str,
    mesh_first: bool,
    torque: bool,
    /// `mode auto ratio`; `None` is PID.
    auto_ratio: Option<f64>,
    com: [f64; 3],
    target: f64,
    gains: [f64; 3],
    /// `target_val v_tv`: the equal-style formula of `tv`; `None` is the
    /// constant `target`.
    target_var: Option<&'static str>,
    /// Values pinned as differing, with a demonstrated cause (gh:#1455
    /// policy); 0 everywhere except the `sin` set point below.
    expected_differ: usize,
    /// The servo must have moved the plate centre by more than this
    /// (force servo) or the torque about `z` must be non-zero (torque servo).
    expect_moved: f64,
    /// The deck's later `run` commands: the step each starts at and the
    /// `fix_modify plate servo/...` keywords issued before it (gh:#1510).
    /// Empty: one continuous run.
    runs: Vec<(usize, Vec<ServoModify>)>,
    /// Control: skip `begin_run` at the run boundaries (one continuous run
    /// against a multi-run reference) and require a difference.
    continuous_control: bool,
    /// Control: require a difference from the reference (a run that leaves
    /// out something the deck does).
    must_differ: bool,
}

impl Case {
    fn force_pid(deck: &'static str, stl: &'static str, mesh_first: bool) -> Self {
        Self {
            deck,
            stl,
            mesh_first,
            torque: false,
            auto_ratio: None,
            com: [0.0, 0.0, 0.0112],
            target: 0.1,
            gains: [1.0, 0.2, 0.0001],
            target_var: None,
            expected_differ: 0,
            expect_moved: 1.0e-3,
            runs: Vec::new(),
            continuous_control: false,
            must_differ: false,
        }
    }
}

/// Run `sys` from `*last` to `target`, beginning each pending run whose
/// start step lies strictly before `target` (a boundary AT `target` is left
/// pending: the reference prints its end-of-run line there first).
fn advance(
    sys: &mut GranularSystem,
    last: &mut usize,
    target: usize,
    pending: &mut std::collections::VecDeque<(usize, Vec<ServoModify>)>,
    control: bool,
) {
    while let Some((b, _)) = pending.front() {
        if *b >= target {
            break;
        }
        let b = *b;
        sys.run(b - *last);
        *last = b;
        begin(sys, pending, control);
    }
    sys.run(target - *last);
    *last = target;
}

/// The next `run` command: its `fix_modify` keywords, then `begin_run`
/// (unless this is the continuous control).
fn begin(
    sys: &mut GranularSystem,
    pending: &mut std::collections::VecDeque<(usize, Vec<ServoModify>)>,
    control: bool,
) {
    let (_, mods) = pending.pop_front().expect("a pending run");
    for m in mods {
        sys.fix_modify_servo(0, m).expect("valid fix_modify");
    }
    if !control {
        sys.begin_run().expect("run set-up");
    }
}

fn run_servo(case: &Case) {
    let csv_name = format!("{}.csv", case.deck);
    let thermo_name = format!("{}_thermo.txt", case.deck);
    let mesh_first = case.mesh_first;
    let (Some(csv), Some(thermo), Some(stl)) = (
        read(&csv_name),
        read(&thermo_name),
        read(case.stl),
    ) else {
        eprintln!("skipping: reference-data/liggghts/ not present");
        return;
    };
    let rows = table(&csv, true, 1);
    let thermo = table(&thermo, false, 0);
    assert_eq!(rows.len(), 501);
    // one more thermo line per extra run: the set-up print
    assert_eq!(thermo.len(), 81 + case.runs.len());
    let mut pending: std::collections::VecDeque<_> = case.runs.clone().into();
    let mesh = MeshWall::from_ascii_stl(&stl).expect("valid plate stl");
    let mut servo = MeshServo::new(
        case.com,
        [0.0, 0.0, 1.0],
        case.target,
        0.05,
        case.gains,
        1.0e-5,
        0.003,
    );
    if case.torque {
        servo = servo.with_torque(&mesh.nodes());
    }
    if let Some(formula) = case.target_var {
        let table = VariableTable::new();
        servo = servo.with_target_variable(table.parse(formula).expect("valid formula"));
    }
    if let Some(ratio) = case.auto_ratio {
        servo = servo.with_auto(ratio, 0.005);
    }
    let plate = MovingBoundary::new(
        WallGeometry::Mesh(mesh),
        Vec3::zero(),
        Vec3::zero(),
        Vec3::zero(),
    )
    .with_servo(servo);
    let floor =
        Boundary::wall(Vec3::new(0.0, 0.0, 0.0), Vec3::new(0.0, 0.0, 1.0)).expect("valid wall");
    let material = GranularMaterial::new(1.0e7, 0.3, 0.9, 0.5).expect("valid material");
    let sys = GranularSystem::new(
        vec![sphere(&rows[0], 0, 0.005), sphere(&rows[0], 1, 0.005)],
        vec![floor],
        GranularContactModel::hertz_history(material),
        Vec3::new(0.0, 0.0, -9.81),
        1.0e-5,
    )
    .expect("valid system")
    .with_moving_walls(vec![plate])
    .with_neighbor_skin(0.003);
    if mesh_first {
        assert!(sys.clone().with_fix_order(vec![FixSlot::Gravity]).is_err());
    }
    let mut sys = if mesh_first {
        sys.with_fix_order(vec![
            FixSlot::MovingWall(0),
            FixSlot::PrimitiveWall(0),
            FixSlot::Gravity,
        ])
        .expect("complete fix order")
    } else {
        sys
    };

    let (mut mismatched, mut total, mut first) = (0, 0, None);
    let mut thermo_mismatched = 0_usize;
    let mut last = 0_usize;
    let mut thermo_rows = thermo.iter();
    let mut next_thermo = thermo_rows.next();
    let mut moved = 0.0_f64;
    let (mut max_op, mut zero_contacts, mut some_contacts) = (0.0_f64, false, false);
    for row in &rows {
        let step = row[0] as usize;
        while let Some(th) = next_thermo {
            let ts = th[0] as usize;
            if ts > step {
                break;
            }
            if ts == last && pending.front().is_some_and(|(b, _)| *b == ts) {
                // the second line at a run boundary: printed after set-up
                begin(&mut sys, &mut pending, case.continuous_control);
            } else {
                advance(&mut sys, &mut last, ts, &mut pending, case.continuous_control);
            }
            for k in 0..12 {
                total += 1;
                let got = if k < 9 {
                    sys.mesh_stress_vector(0, k).expect("stress tracked")
                } else {
                    sys.mesh_servo_xcm(0, k - 9).expect("servo")
                };
                if k == 11 && !case.torque {
                    moved = moved.max((case.com[2] - got).abs());
                }
                if k == 5 && case.torque {
                    moved = moved.max(got.abs());
                }
                if got.to_bits() != th[1 + k].to_bits() {
                    mismatched += 1;
                    thermo_mismatched += 1;
                    first.get_or_insert((1000 + ts, k, got, th[1 + k]));
                }
            }
            next_thermo = thermo_rows.next();
            let op = sys.mesh_servo_ctrl_op(0).expect("servo");
            max_op = max_op.max((op[0] * op[0] + op[1] * op[1] + op[2] * op[2]).sqrt());
            if sys.mesh_servo_contacts(0).expect("servo") == 0 {
                zero_contacts = true;
            } else {
                some_contacts = true;
            }
        }
        advance(&mut sys, &mut last, step, &mut pending, case.continuous_control);
        for (i, q) in sys.particles().iter().enumerate() {
            let got = [
                q.position.x,
                q.position.y,
                q.position.z,
                q.velocity.x,
                q.velocity.y,
                q.velocity.z,
                q.angular_velocity.x,
                q.angular_velocity.y,
                q.angular_velocity.z,
            ];
            for (k, g) in got.iter().enumerate() {
                total += 1;
                if g.to_bits() != row[1 + 9 * i + k].to_bits() {
                    mismatched += 1;
                    first.get_or_insert((step, k + 100 * i, *g, row[1 + 9 * i + k]));
                }
            }
        }
    }
    assert!(
        moved > case.expect_moved,
        "the servo never moved the plate: {moved}"
    );
    eprintln!(
        "{}{}: {mismatched}/{total} differ ({thermo_mismatched} of them f_plate values), first {first:?}",
        case.deck,
        if case.continuous_control {
            " (continuous control)"
        } else if case.must_differ {
            " (control)"
        } else {
            ""
        }
    );
    assert!(pending.is_empty(), "runs left unbegun: {pending:?}");
    if case.continuous_control || case.must_differ {
        assert!(mismatched > 0, "a control run reproduced the reference");
        return;
    }
    assert_eq!(
        mismatched, case.expected_differ,
        "{mismatched}/{total} differ, first {first:?}"
    );
    if case.torque {
        assert!(max_op > 1.0e-3, "the torque servo never rotated the plate: {max_op}");
    }
    if case.auto_ratio.is_some() && !case.torque {
        assert!(zero_contacts && some_contacts, "auto mode needs both cruise and contact phases");
    }
}

/// The upstream abort is kept: `vel_max >= skin / (2 dt)` panics.
#[test]
#[should_panic(expected = "vel_max < skin")]
fn servo_rejects_too_fast_a_plate() {
    let _ = MeshServo::new(
        [0.0; 3],
        [0.0, 0.0, 1.0],
        0.1,
        500.0,
        [1.0, 0.0, 0.0],
        1.0e-5,
        0.003,
    );
}
