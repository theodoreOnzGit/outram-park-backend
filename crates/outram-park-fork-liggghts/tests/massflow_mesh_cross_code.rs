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

//! # `fix massflow/mesh` against upstream (gh:#1416)
//!
//! `FixMassflowMesh` (`fix_massflow_mesh.cpp`), LIGGGHTS-PUBLIC `3d5c00f2`,
//! against [`outram_park_fork_liggghts::massflow::MassFlowMesh`].
//!
//! ## Methodology
//!
//! Deck `reference-data/liggghts/in.massflow_plate`: twelve `d = 10 mm`
//! spheres (`E = 1e7`, `nu = 0.3`, `e = 0.9`, `mu = 0.5`, no mutual contact)
//! on a 4 x 3 grid fall or are thrown through the 0.3 m square
//! `plate_square.stl` (`z = 0`; two columns lie outside it) and bounce on a
//! floor wall at `z = -0.12`, 60 000 steps of `dt = 1e-5`. Two counters sit on
//! the same mesh: `mfu` (`vec_side 0 0 1`, `count multiple`) and `mfd`
//! (`vec_side 0 0 -1`, `count once`). The port starts from the deck's step-0
//! state (`massflow_plate.csv`), runs the same system and compares, every 500
//! steps, all `12 x 9` trajectory values and the eight thermo values
//! `f_mfu[1..4]`, `f_mfd[1..4]` (mass, count, mass rate, count rate; printed
//! `%.17g`, `massflow_plate_thermo.txt`) bit for bit. The counters are
//! evaluated at step 0 as upstream's thermo does (this arms the rate
//! bookkeeping).
//!
//! **Prediction (written before the first run, 2026-11-11).** Bit-identical:
//! counts are integers, masses sums of the same four-term products in the same
//! order, rates one division by the same `delta_t_`. The only way to differ is
//! a particle counted a step early or late, which the 500-step sampling
//! would show as a count mismatch.
//!
//! **Result (2026-11-11, Rust 1.99.1, LIGGGHTS 3d5c00f2): 14036/14036 values
//! bit-identical** (121 samples x (108 trajectory + 8 counter values)); final
//! `mfu = (0.014398966328953218 kg, 11)`, `mfd = (0.0065449846949787354 kg, 5)`.
//! A first version that visited every particle near a facet's bounding
//! sphere instead of upstream's mesh neighbour list differed from step
//! 36 500 (one extra upward crossing: a stale-counter effect, see
//! `massflow.rs`); translating `resolveTriSphereNeighbuild` and the rebuild
//! schedule removed it. Not cross-coded: `point_at_outlet`, `inside_out`.

use outram_park_fork_liggghts::boundary::Boundary;
use outram_park_fork_liggghts::granular::{GranularContactModel, GranularMaterial};
use outram_park_fork_liggghts::granular_system::GranularSystem;
use outram_park_fork_liggghts::massflow::{MassFlowConfig, MassFlowMesh};
use outram_park_fork_liggghts::mesh_wall::MeshWall;
use outram_park_fork_liggghts::particle::{Particle, Vec3};
use uom::si::f64::{Length, Mass, ThermodynamicTemperature};
use uom::si::{length::meter, mass::kilogram, thermodynamic_temperature::kelvin};

fn read(name: &str) -> Option<String> {
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../reference-data/liggghts")
        .join(name);
    std::fs::read_to_string(path).ok()
}

fn config(side: f64, once: bool) -> MassFlowConfig {
    MassFlowConfig {
        vec_side: Vec3::new(0.0, 0.0, side),
        once,
        point_at_outlet: None,
        inside_out: false,
        skin: 0.003,
    }
}

/// 12 spheres, `massflow` counters up/down, 60 000 steps: trajectories and
/// counters bit-identical to LIGGGHTS at all 121 sampled steps.
///
/// Result (2026-11-11, Rust 1.99.1): see the assertion message; the final
/// state is `mfu = (0.014398966328953218 kg, 11)`, `mfd = (0.0065449846949787354 kg, 5)`.
#[test]
fn massflow_mesh_matches_liggghts() {
    let (Some(csv), Some(thermo), Some(stl)) =
        (read("massflow_plate.csv"), read("massflow_plate_thermo.txt"), read("plate_square.stl"))
    else {
        eprintln!("skipping: reference-data/liggghts/ not present");
        return;
    };
    let rows: Vec<Vec<f64>> = csv
        .lines()
        .skip(1)
        .filter(|l| !l.trim().is_empty())
        .map(|l| l.split(',').map(|v| v.parse().expect("numeric csv")).collect())
        .collect();
    let thermo: Vec<Vec<f64>> = thermo
        .lines()
        .map(|l| l.split_whitespace().map(|v| v.parse().expect("numeric thermo")).collect())
        .collect();
    assert_eq!(rows.len(), 121);
    assert_eq!(thermo.len(), 121);
    let n = 12;
    let r = 0.005;
    let mass = 4.0 * std::f64::consts::PI / 3.0 * r * r * r * 2500.0;
    let particles: Vec<Particle> = (0..n)
        .map(|i| {
            let c = &rows[0][1 + 9 * i..1 + 9 * i + 9];
            Particle::new(
                Vec3::new(c[0], c[1], c[2]),
                Vec3::new(c[3], c[4], c[5]),
                Vec3::zero(),
                Mass::new::<kilogram>(mass),
                Length::new::<meter>(r),
                ThermodynamicTemperature::new::<kelvin>(300.0),
            )
            .expect("valid sphere")
        })
        .collect();
    let floor = Boundary::wall(Vec3::new(0.0, 0.0, -0.12), Vec3::new(0.0, 0.0, 1.0)).expect("valid wall");
    let material = GranularMaterial::new(1.0e7, 0.3, 0.9, 0.5).expect("valid material");
    let mut sys = GranularSystem::new(
        particles,
        vec![floor],
        GranularContactModel::hertz_history(material),
        Vec3::new(0.0, 0.0, -9.81),
        1.0e-5,
    )
    .expect("valid system")
    .with_neighbor_skin(0.003);
    let mesh = || MeshWall::from_ascii_stl(&stl).expect("valid plate stl");
    let up = sys.add_massflow_mesh(MassFlowMesh::new(mesh(), config(1.0, false)).expect("valid fix"));
    let down = sys.add_massflow_mesh(MassFlowMesh::new(mesh(), config(-1.0, true)).expect("valid fix"));

    let (mut mismatched, mut total, mut first) = (0, 0, None);
    let mut last = 0_usize;
    for (row, th) in rows.iter().zip(&thermo) {
        let step = row[0] as usize;
        assert_eq!(th[0] as usize, step);
        sys.run(step - last);
        last = step;
        let mut check = |k: usize, got: f64, want: f64| {
            total += 1;
            if got.to_bits() != want.to_bits() {
                mismatched += 1;
                first.get_or_insert((step, k, got, want));
            }
        };
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
                check(1000 + 9 * i + k, *g, row[1 + 9 * i + k]);
            }
        }
        for k in 0..4 {
            check(k, sys.massflow_vector(up, k), th[1 + k]);
        }
        for k in 0..4 {
            check(4 + k, sys.massflow_vector(down, k), th[5 + k]);
        }
    }
    assert_eq!(mismatched, 0, "{mismatched}/{total} differ, first {first:?}");
    assert_eq!(sys.massflow(up).crossings().len(), 11);
    assert_eq!(sys.massflow(down).crossings().len(), 5);
}

/// The constructor's guards (`fix massflow/mesh` `error->fix_error`): zero or
/// in-plane `vec_side`, `point_at_outlet` with `count multiple`, `inside_out`
/// without an outlet point, and a non-planar mesh.
#[test]
fn massflow_constructor_rejects_invalid_setups() {
    let Some(stl) = read("plate_square.stl") else {
        return;
    };
    let mesh = || MeshWall::from_ascii_stl(&stl).expect("valid plate stl");
    let ok = config(1.0, true);
    assert!(MassFlowMesh::new(mesh(), ok).is_ok());
    assert!(MassFlowMesh::new(mesh(), MassFlowConfig { vec_side: Vec3::zero(), ..ok }).is_err());
    assert!(MassFlowMesh::new(mesh(), MassFlowConfig { vec_side: Vec3::new(1.0, 0.0, 0.0), ..ok }).is_err());
    let outlet = MassFlowConfig { point_at_outlet: Some(Vec3::new(0.0, 0.0, -1.0)), ..ok };
    assert!(MassFlowMesh::new(mesh(), outlet).is_ok());
    assert!(MassFlowMesh::new(mesh(), MassFlowConfig { once: false, ..outlet }).is_err());
    assert!(MassFlowMesh::new(mesh(), MassFlowConfig { inside_out: true, ..ok }).is_err());
    // a closed drum is not planar
    let Some(drum) = read("drum_cylinder.stl") else {
        return;
    };
    let drum = MeshWall::from_ascii_stl(&drum).expect("valid drum stl");
    assert!(MassFlowMesh::new(drum, config(1.0, true)).is_err());
}

/// `point_at_outlet` / `inside_out` (unit-level only, not cross-coded): a sphere falling through
/// the plate toward an outlet point below is counted once.
#[test]
fn massflow_point_at_outlet_counts_a_fall_through() {
    let Some(stl) = read("plate_square.stl") else {
        return;
    };
    let mesh = MeshWall::from_ascii_stl(&stl).expect("valid plate stl");
    let r = 0.005;
    let p = Particle::new(
        Vec3::new(0.0, 0.0, 0.02),
        Vec3::new(0.0, 0.0, -1.0),
        Vec3::zero(),
        Mass::new::<kilogram>(1.0e-3),
        Length::new::<meter>(r),
        ThermodynamicTemperature::new::<kelvin>(300.0),
    )
    .expect("valid sphere");
    let material = GranularMaterial::new(1.0e7, 0.3, 0.9, 0.5).expect("valid material");
    for inside_out in [false, true] {
        let mut sys = GranularSystem::new(
            vec![p],
            vec![],
            GranularContactModel::hertz_history(material),
            Vec3::zero(),
            1.0e-5,
        )
        .expect("valid system");
        let cfg = MassFlowConfig {
            vec_side: Vec3::zero(),
            once: true,
            point_at_outlet: Some(Vec3::new(0.0, 0.0, -0.5)),
            inside_out,
            skin: 0.003,
        };
        let id = sys.add_massflow_mesh(MassFlowMesh::new(mesh.clone(), cfg).expect("valid fix"));
        sys.run(4000);
        let counted = sys.massflow_vector(id, 1);
        // measured 2026-11-11 (NOT cross-coded against LIGGGHTS, no deck was run for
        // the outlet branch): the dot product with the outlet direction is positive
        // after the plate for the default orientation and negated by `inside_out`
        assert_eq!(counted, if inside_out { 0.0 } else { 1.0 }, "inside_out={inside_out}");
    }
}
