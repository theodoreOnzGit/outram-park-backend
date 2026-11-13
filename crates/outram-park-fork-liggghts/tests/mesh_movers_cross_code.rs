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

//! # Time-dependent `fix move/mesh` movers against upstream (gh:#1416)
//! (plus `linear/variable` and `rotate/variable`, added 2026-11-11: ~~**2259/2259
//! bit-identical each**~~ bit-identical with glibc trig; since 2026-11-12 see
//! "PETIR libm" below for the libm-only counts) against `in.mover_linvar` / `in.mover_rotvar`, Rust 1.99.1)
//!
//! `MeshMoverWiggle`, `MeshMoverVibLin` (`mesh_mover_linear.cpp`) and
//! `MeshMoverRiggle`, `MeshMoverVibRot` (`mesh_mover_rotation.cpp`),
//! LIGGGHTS-PUBLIC `3d5c00f2`.
//!
//! ## Methodology
//!
//! Decks `in.mover_wiggle|viblin|riggle|vibrot` (`reference-data/liggghts/`):
//! a `d = 10 mm` sphere (`E = 1e7`, `nu = 0.3`, `e = 0.9`, `mu = 0.5`) rests
//! on the 0.2 m square plate `plate_square.stl` at `(0.05, 0, 0.0049)` under
//! gravity, `dt = 1e-5`, 10 000 steps, 251 frames of x, v, omega compared bit
//! for bit; the plate moves by the named mover (as `in.rotating_plate` does
//! for `rotate`). The port moves the same mesh with
//! [`MovingBoundary::wiggle`] and friends: the time-dependent velocity or
//! rotation rate is evaluated from `time += dt` exactly as
//! `FixMoveMesh::initial_integrate` does, then applied by the constant-rate
//! machinery already verified for `linear` and `rotate`.
//!
//! **Prediction (written before the first run, 2026-11-11).** Bit-identical in
//! all four: each mover is a few `cos`/`sin` evaluations on top of the
//! verified linear/rotate step.
//!
//! ## PETIR libm (2026-11-12, epic gh:#1336, accepted under gh:#1455)
//!
//! The results above were measured while `mesh_wall.rs`, `tri_mesh.rs` and
//! `variable.rs` called `f64::{sin, cos, ...}` (glibc, as LIGGGHTS links).
//! The workspace no-platform-libm gate (`petir/tests/no_platform_libm.rs`)
//! moved those calls to `petir::mathf::RealMath`, whose `sin`/`cos` differ
//! from glibc's in the last bit on some arguments; through the contact
//! dynamics that grows over 10 000 steps. Measured 2026-11-12 (develop
//! `8e7b07eb9` + the sweep, x86_64 Linux, glibc, Rust 1.99.1):
//!
//! | case | differ / 2259 | worst normwise |
//! |---|---|---|
//! | wiggle, linear/variable | 0 | 0 |
//! | viblin | 323 | 1.33e-16 |
//! | rotate/variable | 785 | 6.13e-15 |
//! | riggle | 596 | 3.50e-14 |
//! | vibrot | 1509 | 7.88e-13 |
//!
//! "Normwise" is the largest `|port - LIGGGHTS|` of a quantity (x, v or
//! omega component) over all frames, over that quantity's largest
//! `|LIGGGHTS|`. **Proof the cause is libm and nothing else:** with only
//! those three files put back to their pre-sweep text (std trig, glibc), all
//! eight tests pass bit-identical again (2259/2259 in every case). That swap
//! is a diagnostic and is not committed (PETIR AGENTS.md). Each affected
//! test pins its count exactly and its worst size; a count that moves is a
//! finding.

use outram_park_fork_liggghts::granular::{GranularContactModel, GranularMaterial};
use outram_park_fork_liggghts::granular_system::GranularSystem;
use outram_park_fork_liggghts::mesh_wall::{MeshWall, MovingBoundary, WallGeometry};
use outram_park_fork_liggghts::particle::{Particle, Vec3};
use uom::si::f64::{Length, Mass, ThermodynamicTemperature};
use uom::si::{length::meter, mass::kilogram, thermodynamic_temperature::kelvin};

fn read(name: &str) -> Option<String> {
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../reference-data/liggghts")
        .join(name);
    std::fs::read_to_string(path).ok()
}

/// One replayed case against its reference.
struct CaseResult {
    /// Compared values that are not bit-identical.
    mismatched: usize,
    /// Compared values (251 frames x 9).
    total: usize,
    /// First mismatch: (step, column, port, LIGGGHTS).
    first: Option<(usize, usize, f64, f64)>,
    /// Largest `|port - LIGGGHTS|` over all values, divided by the largest
    /// `|LIGGGHTS|` in the same column (normwise per quantity, so a value
    /// crossing zero does not read as a huge relative error).
    worst_normwise: f64,
    /// Final sphere position.
    end: [f64; 3],
}

/// Replay `csv` with the plate moved by `plate`; returns the comparison
/// after asserting nothing.
fn run_case(
    csv: &str,
    plate: impl Fn(WallGeometry) -> MovingBoundary,
) -> Option<CaseResult> {
    run_case_runs(csv, None, plate)
}

/// [`run_case`] for a deck whose second `run` starts at `second_run`
/// (`GranularSystem::begin_run` there, gh:#1510).
fn run_case_runs(
    csv: &str,
    second_run: Option<usize>,
    plate: impl Fn(WallGeometry) -> MovingBoundary,
) -> Option<CaseResult> {
    let (Some(csv), Some(stl)) = (read(csv), read("plate_square.stl")) else {
        eprintln!("skipping: reference-data/liggghts/ not present");
        return None;
    };
    let rows: Vec<Vec<f64>> = csv
        .lines()
        .skip(1)
        .filter(|l| !l.trim().is_empty())
        .map(|l| {
            l.split(',')
                .map(|v| v.parse().expect("numeric csv"))
                .collect()
        })
        .collect();
    assert_eq!(rows.len(), 251, "unexpected reference frame count");
    let r = 0.005;
    let rho = 2500.0;
    let p = Particle::new(
        Vec3::new(0.05, 0.0, 0.0049),
        Vec3::zero(),
        Vec3::zero(),
        Mass::new::<kilogram>(4.0 * std::f64::consts::PI / 3.0 * r * r * r * rho),
        Length::new::<meter>(r),
        ThermodynamicTemperature::new::<kelvin>(300.0),
    )
    .expect("valid sphere");
    let mesh = MeshWall::from_ascii_stl(&stl).expect("valid plate stl");
    let material = GranularMaterial::new(1.0e7, 0.3, 0.9, 0.5).expect("valid material");
    let mut sys = GranularSystem::new(
        vec![p],
        vec![],
        GranularContactModel::hertz_history(material),
        Vec3::new(0.0, 0.0, -9.81),
        1.0e-5,
    )
    .expect("valid system")
    .with_moving_walls(vec![plate(WallGeometry::Mesh(mesh))])
    .with_neighbor_skin(0.003);

    let (mut mismatched, mut total, mut first) = (0, 0, None);
    let mut scale = [0.0_f64; 9];
    let mut diff = [0.0_f64; 9];
    let mut last = 0_usize;
    for row in &rows {
        let step = row[0] as usize;
        if let Some(b) = second_run {
            if last <= b && b < step {
                sys.run(b - last);
                last = b;
                sys.begin_run().expect("run set-up");
            }
        }
        sys.run(step - last);
        last = step;
        let q = sys.particles()[0];
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
            let want = row[1 + k];
            total += 1;
            scale[k] = scale[k].max(want.abs());
            diff[k] = diff[k].max((g - want).abs());
            if g.to_bits() != want.to_bits() {
                mismatched += 1;
                first.get_or_insert((step, k, *g, want));
            }
        }
    }
    let worst_normwise = (0..9)
        .filter(|&k| scale[k] > 0.0)
        .map(|k| diff[k] / scale[k])
        .fold(0.0, f64::max);
    let q = sys.particles()[0];
    Some(CaseResult {
        mismatched,
        total,
        first,
        worst_normwise,
        end: [q.position.x, q.position.y, q.position.z],
    })
}

fn assert_moved(name: &str, end: [f64; 3]) {
    // the plate must actually have moved the sphere
    assert!(
        (end[0] - 0.05).abs() > 1e-4 || (end[2] - 0.0049).abs() > 5e-4 || end[1].abs() > 1e-4,
        "{name}: the sphere never moved, the case is vacuous"
    );
}

fn assert_identical(name: &str, res: Option<CaseResult>) {
    let Some(r) = res else {
        return;
    };
    assert_eq!(
        r.mismatched, 0,
        "{name}: not bit-identical vs LIGGGHTS: {}/{} differ, first {:?}",
        r.mismatched, r.total, r.first
    );
    assert_moved(name, r.end);
}

/// Bit-identical except for differences that come ONLY from PETIR's libm
/// against the glibc LIGGGHTS links (accepted, gh:#1455): exactly
/// `n_differ` of the compared values differ, and the worst per-quantity
/// normwise difference is at most `worst`. The A/B proof of the cause is in
/// the calling test's doc comment. A count or size that moves is a finding to
/// explain, not a number to re-pin.
fn assert_libm_only_case(name: &str, n_differ: usize, worst: f64, res: Option<CaseResult>) {
    let Some(r) = res else {
        return;
    };
    println!(
        "{name}: {}/{} differ (libm), worst normwise {:.3e}, first {:?}",
        r.mismatched, r.total, r.worst_normwise, r.first
    );
    assert_eq!(r.mismatched, n_differ, "{name}: libm-only differing count moved");
    assert!(
        r.worst_normwise <= worst,
        "{name}: worst normwise {:.3e} > pinned {worst:.3e}",
        r.worst_normwise
    );
    assert_moved(name, r.end);
}

/// `wiggle amplitude 0.004 0.002 0.002 period 0.08`.
#[test]
fn wiggle_matches_liggghts() {
    assert_identical(
        "wiggle",
        run_case("mover_wiggle.csv", |g| {
            MovingBoundary::wiggle(g, Vec3::new(0.004, 0.002, 0.002), 0.08)
        }),
    );
}

/// **Two runs** (gh:#1510): `in.mover_wiggle_2run` is the wiggle deck as
/// `run 5000` + `run 5000`. Across the set-up the mover's `time_` (and so its
/// phase) persists and `FixMoveMesh::setup` zeroes the node velocities after
/// the wall's set-up `post_force`. LIGGGHTS' own two-run trajectory is
/// byte-identical to its one-run trajectory on this deck (checked with `cmp`
/// on the two dumps, 2026-11-13; the cause was not investigated), so this
/// case cannot tell `begin_run` from a continuous run: it only pins that the
/// set-up leaves the wiggle mesh's motion and phase unchanged. The rotating
/// plate's two-run test is the one that discriminates.
///
/// **Prediction (2026-11-13, before the first run):** bit-identical.
#[test]
fn wiggle_two_runs_match_liggghts() {
    assert_identical(
        "wiggle_2run",
        run_case_runs("mover_wiggle_2run.csv", Some(5_000), |g| {
            MovingBoundary::wiggle(g, Vec3::new(0.004, 0.002, 0.002), 0.08)
        }),
    );
}

/// `viblin axis 0.3 0.2 1.0 order 2 amplitude 0.002 0.001 phase 0 1.1 period 0.06 0.035`.
///
/// **libm-only: 323/2259 differ, worst normwise 1.33e-16** (2026-11-12,
/// gh:#1455; see "PETIR libm" in the module docs for the A/B proof).
#[test]
fn viblin_matches_liggghts() {
    assert_libm_only_case(
        "viblin",
        323,
        1.4e-16,
        run_case("mover_viblin.csv", |g| {
            MovingBoundary::viblin(
                g,
                Vec3::new(0.3, 0.2, 1.0),
                &[0.002, 0.001],
                &[0.0, 1.1],
                &[0.06, 0.035],
            )
            .expect("valid order")
        }),
    );
}

/// `riggle origin 0.01 0 0 axis 0.1 0.2 1 period 0.2 amplitude 4` (degrees).
///
/// **libm-only: 596/2259 differ, worst normwise 3.50e-14** (2026-11-12,
/// gh:#1455; see "PETIR libm" in the module docs for the A/B proof).
#[test]
fn riggle_matches_liggghts() {
    assert_libm_only_case(
        "riggle",
        596,
        3.6e-14,
        run_case("mover_riggle.csv", |g| {
            MovingBoundary::riggle(
                g,
                Vec3::new(0.01, 0.0, 0.0),
                Vec3::new(0.1, 0.2, 1.0),
                0.2,
                4.0,
            )
        }),
    );
}

/// `vibrot origin 0.01 0 0 axis 0.1 0.2 1 order 2 amplitude 0.05 0.02 phase 0
/// 0.7 period 0.15 0.09`.
///
/// **libm-only: 1509/2259 differ, worst normwise 7.88e-13** (2026-11-12,
/// gh:#1455; see "PETIR libm" in the module docs for the A/B proof).
#[test]
fn vibrot_matches_liggghts() {
    assert_libm_only_case(
        "vibrot",
        1509,
        8.0e-13,
        run_case("mover_vibrot.csv", |g| {
            MovingBoundary::vibrot(
                g,
                Vec3::new(0.01, 0.0, 0.0),
                Vec3::new(0.1, 0.2, 1.0),
                &[0.05, 0.02],
                &[0.0, 0.7],
                &[0.15, 0.09],
            )
            .expect("valid order")
        }),
    );
}

/// The vibration mover constructors reject an empty harmonic list and
/// mismatched list lengths.
#[test]
fn vibration_movers_validate_their_order() {
    let Some(stl) = read("plate_square.stl") else {
        eprintln!("skipping: reference-data/liggghts/ not present");
        return;
    };
    let plate = || WallGeometry::Mesh(MeshWall::from_ascii_stl(&stl).expect("valid plate stl"));
    let axis = Vec3::new(0.0, 0.0, 1.0);
    assert!(MovingBoundary::viblin(plate(), axis, &[], &[], &[]).is_none());
    assert!(MovingBoundary::viblin(plate(), axis, &[1.0], &[0.0, 0.0], &[1.0]).is_none());
    assert!(MovingBoundary::vibrot(plate(), Vec3::zero(), axis, &[], &[], &[]).is_none());
    assert!(MovingBoundary::viblin(plate(), axis, &[1.0], &[0.0], &[1.0]).is_some());
}

/// `linear/variable v_vx v_vy v_vz` (gh:#1416, new 2026-11-11): the plate
/// velocity comes from three equal-style variables
/// (`0.3*sin(2*PI*time/0.08)+0.5*step*dt`, `swiggle(0,0.15,0.05)`,
/// `-0.05*cwiggle(0,1,0.1)`) re-evaluated every step through
/// [`outram_park_fork_liggghts::variable`], deck `in.mover_linvar`.
#[test]
fn linear_variable_matches_liggghts() {
    use outram_park_fork_liggghts::variable::VariableTable;
    let mut t = VariableTable::new();
    t.define_equal("vx", "0.3*sin(2*PI*time/0.08)+0.5*step*dt")
        .unwrap();
    t.define_equal("vy", "swiggle(0,0.15,0.05)").unwrap();
    t.define_equal("vz", "-0.05*cwiggle(0,1,0.1)").unwrap();
    assert_identical(
        "linear/variable",
        run_case("mover_linvar.csv", |g| {
            MovingBoundary::linear_variable(g, &t, ["vx", "vy", "vz"]).expect("defined variables")
        }),
    );
}

/// `rotate/variable origin 0.01 0 0 axis 0.1 0.2 1 omega v_ww` with
/// `ww = 3+6*sin(2*PI*time/0.12)` rad/s, deck `in.mover_rotvar`.
///
/// **libm-only: 785/2259 differ, worst normwise 6.13e-15** (2026-11-12,
/// gh:#1455; see "PETIR libm" in the module docs for the A/B proof).
#[test]
fn rotate_variable_matches_liggghts() {
    use outram_park_fork_liggghts::variable::VariableTable;
    let mut t = VariableTable::new();
    t.define_equal("ww", "3.0+6.0*sin(2*PI*time/0.12)").unwrap();
    assert_libm_only_case(
        "rotate/variable",
        785,
        6.2e-15,
        run_case("mover_rotvar.csv", |g| {
            MovingBoundary::rotate_variable(
                g,
                &t,
                Vec3::new(0.01, 0.0, 0.0),
                Vec3::new(0.1, 0.2, 1.0),
                "ww",
            )
            .expect("defined variable")
        }),
    );
}

/// A missing variable is rejected, as upstream's `error->all` does.
#[test]
fn variable_movers_reject_undefined_variables() {
    use outram_park_fork_liggghts::variable::VariableTable;
    let Some(stl) = read("plate_square.stl") else {
        return;
    };
    let plate = || WallGeometry::Mesh(MeshWall::from_ascii_stl(&stl).expect("valid plate stl"));
    let t = VariableTable::new();
    assert!(MovingBoundary::linear_variable(plate(), &t, ["a", "b", "c"]).is_err());
    assert!(MovingBoundary::rotate_variable(
        plate(),
        &t,
        Vec3::zero(),
        Vec3::new(0.0, 0.0, 1.0),
        "w"
    )
    .is_err());
}
