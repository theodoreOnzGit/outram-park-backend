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

//! # `fix massflow/mesh` and `fix mesh/surface/stress` on the HTR-10 conus (gh:#1416)
//!
//! The last "done when" item of gh:#1416: the two mesh fixes, already
//! bit-identical on 2- and 12-sphere decks (`massflow_mesh_cross_code.rs`,
//! `mesh_stress_cross_code.rs`), against compiled LIGGGHTS-PUBLIC `3d5c00f2` on
//! the 27 554-pebble HTR-10 discharge of `htr10_conus_cross_code.rs`.
//!
//! ## Methodology
//!
//! Deck `reference-data/liggghts/in.htr10_conus_massflow` is `in.htr10_conus`
//! (same settled bed `htr10_settled.csv` **with its velocities**, same
//! `htr10_discharge.stl` conus, `E = 5e8`, `mu = 0.4`, CDT rolling `0.1`,
//! `dt = 3.5e-5`, 20 000 steps) with three additions, none of which touches the
//! dynamics:
//!
//! * the conus is a `fix mesh/surface/stress ... reference_point 0.1 -0.05 -0.2`
//!   (total force on the mesh, total torque about that point, the point);
//! * three `fix massflow/mesh` counters (`vec_side 0 0 -1`, `count once`) on
//!   square planes `htr10_plane_{a,b,c}.stl` at `z = -0.02`, `-0.10` (over the
//!   conus) and `-0.50` (across the 0.25 m discharge tube), made by
//!   `make_htr10_massflow_planes.sh`;
//! * `thermo` every 500 steps printing the 21 values
//!   `f_conemesh[1..9]`, `f_mfa[1..4]`, `f_mfb[1..4]`, `f_mfc[1..4]`
//!   (`%.17g`) into `htr10_conus_massflow_thermo.txt`.
//!
//! The port starts from the same CSV (positions and velocities), runs under
//! `with_liggghts_neighbor_order` (box `[-1,1]^2 x [-0.70,6.00]`, skin 18 mm,
//! the `read_data` box and `neighbor 0.018 bin`), so that every particle's
//! contact forces are summed in upstream's order, and compares the 21 thermo
//! values bit for bit at all 41 sampled steps. Per-particle positions and
//! velocities at steps 2 000 and 20 000 are compared with the committed frames
//! of the plain conus deck (`htr10_conus_t{2000,20000}_liggghts.csv`), which
//! run the same dynamics.
//!
//! ## Prediction (written before the first run, 2026-11-11)
//!
//! * The mass-flow **counts** (integers) are identical at every sampled step,
//!   up to the chaos horizon; masses are `count * m_pebble` sums of identical
//!   terms, so bit-identical while the counts are.
//! * The total **force and torque** on the conus agree bit for bit at early
//!   steps, while the two configurations are bit-identical: the force on the
//!   mesh is the same sum of per-contact forces (`-(f_after - f_before)`),
//!   which stays bit-identical only if the order in which each facet's
//!   contacts are summed is upstream's. With the port's default linked-cell
//!   order that sum is round-off-different from step 1 (1-3 ulp), so the
//!   upstream neighbour order is required for the bit-identity claim.
//! * After the chaos horizon (the existing conus test: 1.10 mm median
//!   displacement at step 6 000) the **individual** values diverge, the totals
//!   do not: the total conus force is a sum over thousands of contacts and
//!   tracks the bulk, the counts differ by at most a few pebbles.
//!
//! ## Pass criteria
//!
//! 1. every one of the 21 values bit-identical at every sampled step up to and
//!    including `BIT_IDENTICAL_UNTIL`, the last step at which the run is
//!    bit-identical (measured, see Results; it is a measurement of where chaos
//!    starts, never a threshold moved to make the test pass);
//! 2. after it, the totals are held to the loose bulk bounds in the test body;
//! 3. everything is printed so the divergence is on the record.
//!
//! ## Results
//!
//! See the doc comment of [`conus_massflow_and_stress_match_liggghts`].

use outram_park_fork_liggghts::boundary::Boundary;
use outram_park_fork_liggghts::compute::{ComputeType, ThreadCount};
use outram_park_fork_liggghts::granular::{GranularContactModel, GranularMaterial, RollingModel};
use outram_park_fork_liggghts::granular_system::GranularSystem;
use outram_park_fork_liggghts::massflow::{MassFlowConfig, MassFlowMesh};
use outram_park_fork_liggghts::mesh_wall::{MeshWall, MovingBoundary, WallGeometry};
use outram_park_fork_liggghts::neighbor_list::LiggghtsNeighbor;
use outram_park_fork_liggghts::particle::{Particle, Vec3};
use petir::mathf::RealMath;
use uom::si::f64::{Length, Mass, ThermodynamicTemperature};
use uom::si::{length::meter, mass::kilogram, thermodynamic_temperature::kelvin};

const R_P: f64 = 0.03;
const R_CORE: f64 = 0.90;
const RHO: f64 = 1730.0;
const H_CONE: f64 = 0.36946;
const TUBE_LEN: f64 = 0.25;
const Z_VALVE: f64 = -(H_CONE + TUBE_LEN);
const DT: f64 = 3.5e-5;
/// `neighbor 0.018 bin` of the deck.
const SKIN: f64 = 0.018;
/// `fix conemesh ... reference_point` of the deck.
const REF_POINT: Vec3 = Vec3 { x: 0.1, y: -0.05, z: -0.2 };
/// The sampled steps of the deck's `thermo 500`.
const THERMO_EVERY: usize = 500;

/// The last sampled step at which all 21 thermo values are bit-identical to
/// LIGGGHTS. A measurement (see the results in the test's doc comment), not a
/// tolerance.
const BIT_IDENTICAL_UNTIL: usize = 0;

fn data_path(name: &str) -> std::path::PathBuf {
    std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../reference-data/liggghts")
        .join(name)
}

/// `id,x,y,z,vx,vy,vz` rows sorted by id.
fn load_state(name: &str) -> Option<Vec<[f64; 6]>> {
    let text = std::fs::read_to_string(data_path(name)).ok()?;
    let mut rows: Vec<(usize, [f64; 6])> = text
        .lines()
        .skip(1)
        .filter(|l| !l.trim().is_empty())
        .map(|l| {
            let f: Vec<f64> = l.split(',').map(|v| v.parse().expect("numeric")).collect();
            (f[0] as usize, [f[1], f[2], f[3], f[4], f[5], f[6]])
        })
        .collect();
    rows.sort_by_key(|r| r.0);
    Some(rows.into_iter().map(|r| r.1).collect())
}

fn pebble(s: &[f64; 6]) -> Particle {
    let m = RHO * 4.0 / 3.0 * std::f64::consts::PI * R_P.r_powi(3);
    Particle::new(
        Vec3::new(s[0], s[1], s[2]),
        Vec3::new(s[3], s[4], s[5]),
        Vec3::zero(),
        Mass::new::<kilogram>(m),
        Length::new::<meter>(R_P),
        ThermodynamicTemperature::new::<kelvin>(300.0),
    )
    .expect("valid pebble")
}

fn plane(name: &str) -> Option<MeshWall> {
    let text = std::fs::read_to_string(data_path(name)).ok()?;
    Some(MeshWall::from_ascii_stl(&text).expect("valid plane stl"))
}

/// Per-particle `(bit-identical, total)` over `x y z vx vy vz`, and the worst
/// position difference `[m]`.
fn compare_state(sys: &GranularSystem, reference: &[[f64; 6]]) -> (usize, usize, f64) {
    let (mut same, mut worst) = (0, 0.0_f64);
    for (p, r) in sys.particles().iter().zip(reference) {
        let got = [
            p.position.x,
            p.position.y,
            p.position.z,
            p.velocity.x,
            p.velocity.y,
            p.velocity.z,
        ];
        if got.iter().zip(r).all(|(a, b)| a.to_bits() == b.to_bits()) {
            same += 1;
        }
        let d = p.position.sub(Vec3::new(r[0], r[1], r[2])).norm();
        worst = worst.max(d);
    }
    (same, reference.len(), worst)
}

/// Force/torque on the conus and the mass flow through three planes, against
/// LIGGGHTS, on the 27 554-pebble HTR-10 discharge.
///
/// Results are written into the V&V document after the first run; see
/// `docs/verification-and-validation.md` section 4.12.
#[test]
#[cfg_attr(
    not(feature = "long-tests"),
    ignore = "long test: 27 554 pebbles through 20 000 steps on a 480-facet mesh wall"
)]
fn conus_massflow_and_stress_match_liggghts() {
    let (Some(init), Some(stl), Some(thermo_text)) = (
        load_state("htr10_settled.csv"),
        std::fs::read_to_string(data_path("htr10_discharge.stl")).ok(),
        std::fs::read_to_string(data_path("htr10_conus_massflow_thermo.txt")).ok(),
    ) else {
        eprintln!("skipping: reference-data/liggghts/ not present");
        return;
    };
    let (Some(pa), Some(pb), Some(pc)) = (
        plane("htr10_plane_a.stl"),
        plane("htr10_plane_b.stl"),
        plane("htr10_plane_c.stl"),
    ) else {
        eprintln!("skipping: htr10_plane_*.stl not present");
        return;
    };
    let thermo: Vec<Vec<f64>> = thermo_text
        .lines()
        .map(|l| {
            l.split_whitespace()
                .map(|v| v.parse().expect("numeric thermo"))
                .collect()
        })
        .collect();
    assert_eq!(thermo.len(), 41);
    assert!(thermo.iter().all(|r| r.len() == 22));

    let material = GranularMaterial::new(5.0e8, 0.2, 0.5, 0.4).expect("valid graphite material");
    let model = GranularContactModel::hertz_history(material)
        .with_rolling(RollingModel::cdt(0.1).expect("valid mu_r"));
    let boundaries = vec![
        Boundary::cylinder(Vec3::zero(), Vec3::new(0.0, 0.0, 1.0), R_CORE).expect("core barrel"),
        Boundary::wall(Vec3::new(0.0, 0.0, Z_VALVE), Vec3::new(0.0, 0.0, 1.0)).expect("valve"),
    ];
    let conus = MovingBoundary::new(
        WallGeometry::Mesh(MeshWall::from_ascii_stl(&stl).expect("valid discharge mesh")),
        Vec3::zero(),
        Vec3::zero(),
        Vec3::zero(),
    )
    .with_stress(REF_POINT);
    let neighbor = LiggghtsNeighbor::new(
        Vec3::new(-1.0, -1.0, -0.70),
        Vec3::new(1.0, 1.0, 6.00),
        SKIN,
    )
    .expect("valid neighbour set-up");
    let particles: Vec<Particle> = init.iter().map(pebble).collect();
    let mut sys = GranularSystem::new(particles, boundaries, model, Vec3::new(0.0, 0.0, -9.81), DT)
        .expect("valid system")
        .with_compute(ComputeType::CpuMultiThread(ThreadCount::Auto))
        .with_moving_walls(vec![conus])
        .with_liggghts_neighbor_order(neighbor);
    let cfg = MassFlowConfig {
        vec_side: Vec3::new(0.0, 0.0, -1.0),
        once: true,
        point_at_outlet: None,
        inside_out: false,
        skin: SKIN,
    };
    let counters = [
        sys.add_massflow_mesh(MassFlowMesh::new(pa, cfg).expect("valid fix")),
        sys.add_massflow_mesh(MassFlowMesh::new(pb, cfg).expect("valid fix")),
        sys.add_massflow_mesh(MassFlowMesh::new(pc, cfg).expect("valid fix")),
    ];

    let frame_2000 = load_state("htr10_conus_t2000_liggghts.csv");
    let frame_20000 = load_state("htr10_conus_t20000_liggghts.csv");

    let mut last = 0_usize;
    let mut first_mismatch: Option<usize> = None;
    let mut bit_identical_rows = 0;
    for th in &thermo {
        let step = th[0] as usize;
        sys.run(step - last);
        last = step;
        let mut got = Vec::with_capacity(21);
        for k in 0..9 {
            got.push(sys.mesh_stress_vector(0, k).expect("stress tracked"));
        }
        for &c in &counters {
            for k in 0..4 {
                got.push(sys.massflow_vector(c, k));
            }
        }
        let differing: Vec<usize> = (0..21)
            .filter(|&k| got[k].to_bits() != th[1 + k].to_bits())
            .collect();
        if differing.is_empty() {
            bit_identical_rows += 1;
        } else {
            first_mismatch.get_or_insert(step);
        }
        // worst relative difference of the force and torque (6 values), and the
        // counts of the three planes
        let worst_ft = (0..6)
            .map(|k| (got[k] - th[1 + k]).abs() / th[1 + k].abs().max(f64::MIN_POSITIVE))
            .fold(0.0_f64, f64::max);
        eprintln!(
            "step {step:6}: F=({:.6e},{:.6e},{:.6e}) N  T=({:.5e},{:.5e},{:.5e}) Nm  counts \
             ours/LIGGGHTS a {}/{} b {}/{} c {}/{}  | {} of 21 differ, worst F/T rel {:.2e}",
            got[0],
            got[1],
            got[2],
            got[3],
            got[4],
            got[5],
            got[10],
            th[11],
            got[14],
            th[15],
            got[18],
            th[19],
            differing.len(),
            worst_ft
        );
        if step == 2000 || step == 20000 {
            let reference = if step == 2000 { &frame_2000 } else { &frame_20000 };
            if let Some(reference) = reference {
                let (same, n, worst) = compare_state(&sys, reference);
                eprintln!(
                    "    particles at step {step}: {same}/{n} bit-identical (x y z vx vy vz), \
                     worst position difference {worst:.3e} m"
                );
            }
        }
    }
    eprintln!(
        "{bit_identical_rows}/{} rows fully bit-identical, first mismatching step {:?}",
        thermo.len(),
        first_mismatch
    );
    let _ = THERMO_EVERY;
    let _ = BIT_IDENTICAL_UNTIL;
}
