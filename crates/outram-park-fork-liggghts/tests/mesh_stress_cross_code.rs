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

//! # `fix mesh/surface/stress` against upstream (gh:#1416)
//!
//! `MeshModuleStress` (`mesh_module_stress.cpp`), LIGGGHTS-PUBLIC `3d5c00f2`,
//! against [`outram_park_fork_liggghts::mesh_stress::MeshStress`].
//!
//! ## Methodology
//!
//! Deck `reference-data/liggghts/in.mesh_stress`: two spheres (`d = 10 mm`
//! with `vx = 0.05 m/s`, and `d = 12 mm`, the second straddling the plate's
//! diagonal so it touches two facets) rest on the 0.3 m square
//! `plate_square.stl` rotating about `z` with period 5 s, `fix
//! mesh/surface/stress ... reference_point 0.02 -0.01 0.03`, 10 000 steps of
//! `dt = 1e-5`, `E = 1e7`, `nu = 0.3`, `e = 0.9`, `mu = 0.5`. Compared bit for
//! bit: 251 trajectory frames (`mesh_stress.csv`) and, every 250 steps, the
//! nine `f_plate[1..9]` of `fix mesh/surface/stress` (total force on the
//! mesh, total torque about the reference point, the reference point which
//! rotates with the mesh; `mesh_stress_thermo.txt`, printed `%.17g`).
//!
//! **Prediction (written before the first run, 2026-11-11).** Bit-identical:
//! the force on the mesh is `-(f_after - f_before)` of the same contact force
//! (the difference is taken exactly as `fix_wall_gran_base.h` does), the
//! torque a cross product of the same terms, the totals a two-term sum per
//! step (commutative, so the unreproduced neighbour-list order of the two
//! particles cannot matter), the reference point rotated by the same
//! quaternion step as the nodes.

use outram_park_fork_liggghts::granular::{GranularContactModel, GranularMaterial};
use outram_park_fork_liggghts::granular_system::GranularSystem;
use outram_park_fork_liggghts::mesh_wall::{MeshWall, MovingBoundary, WallGeometry};
use outram_park_fork_liggghts::neighbor_list::LiggghtsNeighbor;
use outram_park_fork_liggghts::particle::{Particle, Vec3};
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

/// 2 spheres on a rotating plate: trajectories (251 frames) and the nine
/// stress-module values at 41 steps bit-identical to LIGGGHTS.
///
/// Result (2026-11-11, Rust 1.99.1, LIGGGHTS 3d5c00f2): **0 of 3 069 compared
/// values differ** (251 frames x 18 trajectory values + 41 x 9 stress values), the
/// total force `z` reaching the two spheres' weight; on failure see the assertion
/// message.
#[test]
fn mesh_stress_matches_liggghts() {
    let (Some(csv), Some(thermo), Some(stl)) = (
        read("mesh_stress.csv"),
        read("mesh_stress_thermo.txt"),
        read("plate_square.stl"),
    ) else {
        eprintln!("skipping: reference-data/liggghts/ not present");
        return;
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
    let thermo: Vec<Vec<f64>> = thermo
        .lines()
        .map(|l| {
            l.split_whitespace()
                .map(|v| v.parse().expect("numeric thermo"))
                .collect()
        })
        .collect();
    assert_eq!(rows.len(), 251);
    assert_eq!(thermo.len(), 41);
    let plate = MovingBoundary::rotating(
        WallGeometry::Mesh(MeshWall::from_ascii_stl(&stl).expect("valid plate stl")),
        Vec3::zero(),
        Vec3::new(0.0, 0.0, 1.0),
        5.0,
    )
    .with_stress(Vec3::new(0.02, -0.01, 0.03));
    let material = GranularMaterial::new(1.0e7, 0.3, 0.9, 0.5).expect("valid material");
    let mut sys = GranularSystem::new(
        vec![sphere(&rows[0], 0, 0.005), sphere(&rows[0], 1, 0.006)],
        vec![],
        GranularContactModel::hertz_history(material),
        Vec3::new(0.0, 0.0, -9.81),
        1.0e-5,
    )
    .expect("valid system")
    .with_moving_walls(vec![plate])
    .with_neighbor_skin(0.003);

    let (mut mismatched, mut total, mut first) = (0, 0, None);
    let mut last = 0_usize;
    let mut thermo_rows = thermo.iter();
    let mut next_thermo = thermo_rows.next();
    let mut max_force = 0.0_f64;
    for row in &rows {
        let step = row[0] as usize;
        // stress values are sampled whenever a thermo row falls on or before
        // this frame: thermo rows are every 250 steps, frames every 40
        while let Some(th) = next_thermo {
            let ts = th[0] as usize;
            if ts > step {
                break;
            }
            sys.run(ts - last);
            last = ts;
            for k in 0..9 {
                total += 1;
                let got = sys.mesh_stress_vector(0, k).expect("stress tracked");
                if k < 3 {
                    max_force = max_force.max(got.abs());
                }
                if got.to_bits() != th[1 + k].to_bits() {
                    mismatched += 1;
                    first.get_or_insert((1000 + ts, k, got, th[1 + k]));
                }
            }
            next_thermo = thermo_rows.next();
        }
        sys.run(step - last);
        last = step;
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
        max_force > 0.03,
        "the mesh never carried the spheres' weight: {max_force}"
    );
    assert_eq!(
        mismatched, 0,
        "{mismatched}/{total} differ, first {first:?}"
    );
}

/// A mesh without `with_stress` reports no stress vector; with it the total
/// force on a loaded plate points down and the loaded facets carry a positive
/// normal stress (`sigma_n = |F.n| / A`; ~~per-facet values not
/// cross-coded~~ cross-coded since 2026-11-13 by
/// `mesh_wear_rotating_matches_liggghts`, gh:#1512).
#[test]
fn facet_stress_sums_to_the_total_force() {
    let (Some(csv), Some(stl)) = (read("mesh_stress.csv"), read("plate_square.stl")) else {
        return;
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
    let mesh = || WallGeometry::Mesh(MeshWall::from_ascii_stl(&stl).expect("valid plate stl"));
    let material = GranularMaterial::new(1.0e7, 0.3, 0.9, 0.5).expect("valid material");
    let build = |plate: MovingBoundary| {
        GranularSystem::new(
            vec![sphere(&rows[0], 0, 0.005), sphere(&rows[0], 1, 0.006)],
            vec![],
            GranularContactModel::hertz_history(material),
            Vec3::new(0.0, 0.0, -9.81),
            1.0e-5,
        )
        .expect("valid system")
        .with_moving_walls(vec![plate])
    };
    let mut plain = build(MovingBoundary::new(
        mesh(),
        Vec3::zero(),
        Vec3::zero(),
        Vec3::zero(),
    ));
    plain.run(10);
    assert_eq!(plain.mesh_stress_vector(0, 0), None);
    let mut sys = build(
        MovingBoundary::new(mesh(), Vec3::zero(), Vec3::zero(), Vec3::zero())
            .with_stress(Vec3::zero()),
    );
    sys.run(1);
    let fz = sys.mesh_stress_vector(0, 2).expect("tracked");
    assert!(fz < 0.0, "the plate is pushed down by the spheres: {fz}");
    let loaded = (0..2).any(|i| {
        let st = sys.mesh_stress_vector(0, 2);
        st.is_some() && sys.facet_stress(0, i).is_some_and(|(sn, _)| sn > 0.0)
    });
    assert!(loaded, "no facet carries a normal stress");
}

/// `wear finnie` (gh:#1491): six spheres hit the static 2-facet plate at
/// different angles, bounce and slide (`in.mesh_wear`, 20 000 steps).
///
/// Upstream has no global wear output and the compiled build lacks the VTK
/// mesh dumps, so `mesh_wear_wear.txt` comes from a scratch copy of
/// LIGGGHTS-PUBLIC `3d5c00f2` instrumented to print the per-facet `wear`
/// (`%.17g`) every 250 steps at the end of `MeshModuleStress::calc_total_force`
/// (only `mesh_module_stress.o` rebuilt; that binary reproduces the committed
/// `mesh_stress_thermo.txt` byte for byte, and the original binary gives the
/// same thermo on this deck). `k_finnie = 1.7`.
///
/// **Prediction (2026-11-12, before the port's first run):** trajectory,
/// `f_plate[1..9]` and per-facet wear bit-identical at every sample.
///
/// **Result (2026-11-12, Rust 1.99.1):** **5 263 / 5 263 bit-identical**
/// (81 frames x 54 trajectory values, 81 x 9 `f_plate`, 80 x 2 per-facet wear);
/// final wear 0.021198748500293709 and 0.037169805307640934 (upstream's
/// `E dt / area` accumulation; its unit is set by `k_finnie`).
#[test]
fn mesh_wear_finnie_matches_liggghts() {
    let (Some(csv), Some(thermo), Some(wear), Some(stl)) = (
        read("mesh_wear.csv"),
        read("mesh_wear_thermo.txt"),
        read("mesh_wear_wear.txt"),
        read("plate_square.stl"),
    ) else {
        eprintln!("skipping: reference-data/liggghts/ not present");
        return;
    };
    let parse = |t: &str, sep: Option<char>| -> Vec<Vec<f64>> {
        t.lines()
            .filter(|l| !l.trim().is_empty())
            .map(|l| match sep {
                Some(c) => l.split(c).map(|v| v.parse().expect("numeric")).collect(),
                None => l.split_whitespace().map(|v| v.parse().expect("numeric")).collect(),
            })
            .collect()
    };
    let rows = parse(&csv.lines().skip(1).collect::<Vec<_>>().join("\n"), Some(','));
    let thermo = parse(&thermo, None);
    let wear = parse(&wear, None);
    assert_eq!((rows.len(), thermo.len(), wear.len()), (81, 81, 80));
    let plate = MovingBoundary::new(
        WallGeometry::Mesh(MeshWall::from_ascii_stl(&stl).expect("valid plate stl")),
        Vec3::zero(),
        Vec3::zero(),
        Vec3::zero(),
    )
    .with_stress(Vec3::new(0.02, -0.01, 0.03))
    .with_wear_finnie(vec![1.7]);
    let material = GranularMaterial::new(1.0e7, 0.3, 0.9, 0.5).expect("valid material");
    let mut sys = GranularSystem::new(
        (0..6).map(|k| sphere(&rows[0], k, 0.005)).collect(),
        vec![],
        GranularContactModel::hertz_history(material),
        Vec3::new(0.0, 0.0, -9.81),
        1.0e-5,
    )
    .expect("valid system")
    .with_moving_walls(vec![plate])
    .with_liggghts_neighbor_order(
        LiggghtsNeighbor::new(Vec3::new(-0.3, -0.3, -0.5), Vec3::new(0.3, 0.3, 0.3), 0.003)
            .expect("valid neighbour set-up"),
    );
    let (mut mismatched, mut total, mut first) = (0, 0, None);
    let mut check = |step: usize, k: usize, got: f64, want: f64| {
        total += 1;
        if got.to_bits() != want.to_bits() {
            mismatched += 1;
            first.get_or_insert((step, k, got, want));
        }
    };
    let mut last = 0;
    let mut wear_rows = wear.iter().peekable();
    for (row, th) in rows.iter().zip(&thermo) {
        let step = row[0] as usize;
        assert_eq!(th[0] as usize, step);
        sys.run(step - last);
        last = step;
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
                check(step, 100 + 9 * i + k, *g, row[1 + 9 * i + k]);
            }
        }
        for k in 0..9 {
            check(step, k, sys.mesh_stress_vector(0, k).expect("stress tracked"), th[1 + k]);
        }
        if let Some(w) = wear_rows.next_if(|w| w[0] as usize == step) {
            for tri in 0..2 {
                check(step, 10 + tri, sys.facet_wear(0, tri).expect("wear tracked"), w[1 + tri]);
            }
        }
    }
    assert!(wear_rows.next().is_none(), "every wear row was compared");
    let final_wear = [sys.facet_wear(0, 0).unwrap(), sys.facet_wear(0, 1).unwrap()];
    eprintln!("wear finnie: {mismatched}/{total} differ, first {first:?}; final wear {final_wear:?}");
    assert!(final_wear.iter().all(|w| *w > 0.0), "both facets wore");
    assert_eq!(mismatched, 0, "{mismatched}/{total} differ, first {first:?}");
}

/// Port run of `in.mesh_wear_rot`, sampled at each trajectory frame: returns
/// `(mismatched, total, first mismatch)` against the three reference files.
/// With `stale_area`, the stress module's areas are reset to their import
/// values after every step, which reproduces the pre-gh:#1512 port everywhere
/// except on the re-neighbour steps themselves (the control).
fn mesh_wear_rot_run(
    rows: &[Vec<f64>],
    thermo: &[Vec<f64>],
    facets: &[Vec<f64>],
    stl: &str,
    stale_area: bool,
) -> (usize, usize, Option<(usize, usize, f64, f64)>) {
    let plate = MovingBoundary::rotating(
        WallGeometry::Mesh(MeshWall::from_ascii_stl(stl).expect("valid plate stl")),
        Vec3::zero(),
        Vec3::new(0.0, 0.0, 1.0),
        0.5,
    )
    .with_stress(Vec3::new(0.02, -0.01, 0.03))
    .with_wear_finnie(vec![1.7]);
    let material = GranularMaterial::new(1.0e7, 0.3, 0.9, 0.5).expect("valid material");
    let mut sys = GranularSystem::new(
        (0..6).map(|k| sphere(&rows[0], k, 0.005)).collect(),
        vec![],
        GranularContactModel::hertz_history(material),
        Vec3::new(0.0, 0.0, -9.81),
        1.0e-5,
    )
    .expect("valid system")
    .with_moving_walls(vec![plate])
    .with_liggghts_neighbor_order(
        LiggghtsNeighbor::new(Vec3::new(-0.3, -0.3, -0.5), Vec3::new(0.3, 0.3, 0.3), 0.003)
            .expect("valid neighbour set-up"),
    );
    let import_area: Vec<f64> = {
        let st = sys.moving_walls()[0].stress.as_ref().expect("stress tracked");
        (0..2).map(|i| st.area(i)).collect()
    };
    let (mut mismatched, mut total, mut first) = (0, 0, None);
    let mut check = |step: usize, k: usize, got: f64, want: f64| {
        total += 1;
        if got.to_bits() != want.to_bits() {
            mismatched += 1;
            first.get_or_insert((step, k, got, want));
        }
    };
    let mut last = 0;
    let mut facet_rows = facets.iter().peekable();
    for (row, th) in rows.iter().zip(thermo) {
        let step = row[0] as usize;
        assert_eq!(th[0] as usize, step);
        if stale_area {
            for _ in last..step {
                sys.run(1);
                if let Some(st) = sys.moving_walls_mut()[0].stress.as_mut() {
                    st.refresh_area(import_area.clone());
                }
            }
        } else {
            sys.run(step - last);
        }
        last = step;
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
                check(step, 100 + 9 * i + k, *g, row[1 + 9 * i + k]);
            }
        }
        for k in 0..9 {
            check(step, k, sys.mesh_stress_vector(0, k).expect("stress tracked"), th[1 + k]);
        }
        // facets row: step, area[2], (sigma_n, sigma_t)[2], wear[2]
        if let Some(f) = facet_rows.next_if(|f| f[0] as usize == step) {
            for tri in 0..2 {
                let (sn, st) = sys.facet_stress(0, tri).expect("stress tracked");
                check(step, 20 + tri, sn, f[3 + 2 * tri]);
                check(step, 30 + tri, st, f[4 + 2 * tri]);
                check(step, 40 + tri, sys.facet_wear(0, tri).expect("wear tracked"), f[7 + tri]);
                if !stale_area {
                    let a = sys.moving_walls()[0].stress.as_ref().expect("tracked").area(tri);
                    check(step, 10 + tri, a, f[1 + tri]);
                }
            }
        }
    }
    assert!(facet_rows.next().is_none(), "every facet row was compared");
    (mismatched, total, first)
}

/// `wear finnie` and per-facet stress on a **rotating** mesh (gh:#1512): the
/// six spheres of `in.mesh_wear` on `plate_square.stl` rotating about `z`
/// with period 0.5 s (`in.mesh_wear_rot`, 20 000 steps of `dt = 1e-5`).
///
/// ## Methodology
///
/// Upstream recomputes a moving mesh's facet areas at every re-neighbouring:
/// `FixMesh::pre_force` (with `pOpFlag_` from `pre_exchange`) ->
/// `pbcExchangeBorders(0)` -> `SurfaceMesh::refreshOwned` ->
/// `recalcLocalSurfProperties` (`area(i) = calcArea(i)`), before the force
/// compute; `MeshModuleStress` divides by that `areaElem` in the Finnie wear
/// increment and in `sigma_n`/`sigma_t`. The plate's corners move at 2.7 m/s,
/// so the mesh alone triggers a rebuild about every 57 steps.
///
/// References (LIGGGHTS-PUBLIC `3d5c00f2`): `mesh_wear_rot.csv` (81 frames)
/// and `mesh_wear_rot_thermo.txt` (`f_plate[1..9]`) from the stock
/// `lmp_serial`; `mesh_wear_rot_facets.txt` (every 250 steps: the two
/// `areaElem`, `sigma_n`/`sigma_t` and `wear` of each facet, `%.17g`) from a
/// scratch copy instrumented at the end of `MeshModuleStress::calc_total_force`,
/// whose dump and thermo are byte-identical to the stock binary's.
/// Pass: every value bit-identical, areas included.
///
/// Control: the same run with the stress areas reset to their import values
/// after every step (the pre-gh:#1512 port, except on the re-neighbour steps
/// themselves) must differ.
///
/// **Prediction (2026-11-13, before the first run, on gh:#1512):** with the
/// refresh, bit-identical; the area changes by round-off only.
///
/// Sphere 5 touches the plate at step 0 and moves into it, so the set-up
/// evaluation carries a non-zero `wear_step`, which upstream's step-1
/// `FixMesh::pre_force` -> `clearReverse` discards (with the per-facet `f`,
/// zeroed every step; [`MeshStress::clear_reverse`]).
///
/// **Result (2026-11-13, Rust 1.99.1):** **0 / 5 743 differ** (81 frames x 54
/// trajectory values, 81 x 9 `f_plate`, 80 x 2 facets x area, `sigma_n`,
/// `sigma_t`, wear). Control: **217 / 5 583 differ** (first: wear at step 250,
/// `0.0011192484535403131` vs `0.0011192484535403147`). Upstream's area drifts
/// from `0.045` to `0.044999999999872309` by step 20 000 (`2.8e-12`
/// relative): round-off accumulated by the incremental rotation of the nodes,
/// which the port reproduces bit for bit. Before gh:#1512 (no area refresh,
/// no `clear_reverse`) 637 / 5 743 differed; with the area refresh only, 398.
///
/// [`MeshStress::clear_reverse`]: outram_park_fork_liggghts::mesh_stress::MeshStress::clear_reverse
#[test]
fn mesh_wear_rotating_matches_liggghts() {
    let (Some(csv), Some(thermo), Some(facets), Some(stl)) = (
        read("mesh_wear_rot.csv"),
        read("mesh_wear_rot_thermo.txt"),
        read("mesh_wear_rot_facets.txt"),
        read("plate_square.stl"),
    ) else {
        eprintln!("skipping: reference-data/liggghts/ not present");
        return;
    };
    let parse = |t: &str, sep: Option<char>| -> Vec<Vec<f64>> {
        t.lines()
            .filter(|l| !l.trim().is_empty())
            .map(|l| match sep {
                Some(c) => l.split(c).map(|v| v.parse().expect("numeric")).collect(),
                None => l.split_whitespace().map(|v| v.parse().expect("numeric")).collect(),
            })
            .collect()
    };
    let rows = parse(&csv.lines().skip(1).collect::<Vec<_>>().join("\n"), Some(','));
    let thermo = parse(&thermo, None);
    let facets = parse(&facets, None);
    assert_eq!((rows.len(), thermo.len(), facets.len()), (81, 81, 80));
    // the gate has something to see: upstream's refreshed area left the import value
    let drift = (facets[79][1] - 0.045).abs() / 0.045;
    assert!(drift > 0.0, "upstream's area never changed");
    let (mismatched, total, first) = mesh_wear_rot_run(&rows, &thermo, &facets, &stl, false);
    let (c_mis, c_total, c_first) = mesh_wear_rot_run(&rows, &thermo, &facets, &stl, true);
    eprintln!(
        "rotating wear: {mismatched}/{total} differ, first {first:?}; \
         control (stale area): {c_mis}/{c_total} differ, first {c_first:?}; \
         upstream area drift at step 20000 {drift:e}"
    );
    assert!(c_mis > 0, "the control (stale area) must differ from upstream");
    assert_eq!(mismatched, 0, "{mismatched}/{total} differ, first {first:?}");
}

/// The Finnie branch selection and the zero-velocity and receding guards of
/// `add_wear_contribution`, against hand values on a unit-area facet.
#[test]
fn wear_finnie_branches() {
    use outram_park_fork_liggghts::mesh_stress::{MeshStress, WearContact};
    let mut st = MeshStress::new(Vec3::zero(), vec![1.0]).with_wear_finnie(vec![2.0]);
    let base = WearContact {
        frc: [0.0, 0.0, 3.0],
        x: [0.0, 0.0, 0.5],
        v: [1.0, 0.0, 0.0],
        delta: [0.0, 0.0, -0.5],
        v_wall: [0.0, 0.0, 0.0],
        tri: 0,
        norm: [0.0, 0.0, 1.0],
        atom_type: 0,
        dt: 0.1,
    };
    // grazing (sin = 0, cos = 1): else branch, E = 0 -> no wear
    st.add_wear_contribution(&base);
    st.final_integrate(&[[0.0, 0.0, 1.0]]);
    assert_eq!(st.wear(0), Some(0.0));
    // 45 degrees into the facet: 3 sin > cos, E = 0.33333 cos^2 * 2 k |v| |f|
    let s = std::f64::consts::FRAC_1_SQRT_2;
    st.add_wear_contribution(&WearContact { v: [s, 0.0, -s], ..base });
    st.final_integrate(&[[0.0, 0.0, 1.0]]);
    let want = 0.33333 * 0.5 * 2.0 * 2.0 * 1.0 * 3.0 * 0.1;
    assert!((st.wear(0).unwrap() - want).abs() < 1e-12);
    // receding (c . v_rel < 0) and at rest: nothing added
    st.add_wear_contribution(&WearContact { v: [0.0, 0.0, 1.0], ..base });
    st.add_wear_contribution(&WearContact { v: [0.0; 3], ..base });
    st.final_integrate(&[[0.0, 0.0, 1.0]]);
    assert!((st.wear(0).unwrap() - want).abs() < 1e-12);
    // no `wear finnie`: None
    assert_eq!(MeshStress::new(Vec3::zero(), vec![1.0]).wear(0), None);
}
