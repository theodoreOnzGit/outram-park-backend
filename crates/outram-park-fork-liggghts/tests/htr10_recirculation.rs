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

//! # HTR-10 slow pebble recirculation — does defuelling densify the bed?
//!
//! ## The question
//!
//! A single poured-and-settled HTR-10 bed reaches a solid fraction of
//! **0.5732** in the bulk and **0.5570** on the whole-core closure, against the
//! published filling fraction of **0.61** (`htr10_pebble_bed.rs`). Both codes
//! agree with each other to four decimals, so the gap is not a code
//! disagreement — it is that a single pour with friction lands at *random loose
//! packing*, while a real core is continuously defuelled and refuelled.
//!
//! This case tests the mechanism directly: extract pebbles through the bottom
//! conus and discharge tube, re-insert them at the top, and watch `φ`.
//!
//! ## Why the extraction must be SLOW, and what that buys
//!
//! Real HTR-10 runs **5-pass** recirculation over a fuel lifetime of years —
//! on the order of one pebble every several minutes. Inertia is irrelevant at
//! that rate, so the flow is **quasi-static**, and quasi-static granular flow
//! is **rate-independent**: the packing is set by the *sequence* of
//! rearrangements, not by how fast they occur.
//!
//! That is what makes this simulable at all. This case cannot reproduce the
//! real rate and does not try to. It only has to stay in the same *regime*,
//! and that is a checkable property rather than an assumption: the bed must
//! stay in enduring contact, with kinetic energy negligible against the
//! gravitational work done. The test measures the ratio and asserts it.
//!
//! **Fast discharge would measure the wrong thing.** A freely draining bed
//! shears, and shearing granular material *dilates* (Reynolds). A free-flow
//! run would plausibly show `φ` falling and invite the conclusion that
//! recirculation does not densify, when all it would have shown is that
//! avalanching does not.
//!
//! ## Geometry — the published discharge path
//!
//! From `docs/reactor-scoping/htr10-neutronics.md`:
//!
//! | Quantity | Value |
//! |---|---|
//! | core radius | 90 cm |
//! | height of conus | 36.946 cm |
//! | fuel discharge tube radius | 25 cm |
//!
//! The conus and tube are a generated triangulated wall,
//! `reference-data/liggghts/htr10_discharge.stl` (480 facets, inward normals),
//! produced by `make_htr10_discharge_stl.sh` beside it. `boundary.rs` has no
//! cone primitive — it says so explicitly — so this takes the mesh-wall path
//! the angle-of-repose case already uses. The implied cone is **29.6° from
//! horizontal**, which is a shallow hopper and is what the published numbers
//! give.
//!
//! The flat floor of `htr10_pebble_bed.rs` is *removed*, so the bed drains into
//! the conus. A plane at the bottom of the tube is the **defuelling valve**:
//! pebbles rest on it, and extraction removes the lowest ones from there. That
//! is what makes the extraction quasi-static — without it the 50 cm orifice
//! free-flows at roughly a thousand pebbles a second.
//!
//! ## Caveat: 0.61 and 19.5° are both quoted, not measured
//!
//! Before reading any disagreement as a model defect: the benchmark's packing
//! fraction of 0.61 is recorded in `docs/reactor-scoping/htr10-plant-data.md`
//! as **"Quoted"** from a specification table, not as a measurement. And the
//! 19.5° upper-surface cone angle was, in Terry's own words, *calculated by an
//! INL discrete-element code*, with "the cone angle was not measured in the
//! experiment". Both may be modelling assumptions.
//!
//! ## Results
//!
//! Recorded below the test, and written per batch to
//! `reference-data/liggghts/htr10_recirculation.csv`.

use outram_park_fork_liggghts::boundary::Boundary;
use outram_park_fork_liggghts::granular::{GranularContactModel, GranularMaterial, RollingModel};
use outram_park_fork_liggghts::granular_system::GranularSystem;
use outram_park_fork_liggghts::mesh_wall::{MeshWall, MovingBoundary, WallGeometry};
use outram_park_fork_liggghts::particle::{Particle, Vec3};
use uom::si::f64::{Length, Mass, ThermodynamicTemperature};
use uom::si::{length::meter, mass::kilogram, thermodynamic_temperature::kelvin};

const R_P: f64 = 0.03;
const R_CORE: f64 = 0.90;
const RHO: f64 = 1730.0;
const H_CONE: f64 = 0.36946;
const R_TUBE: f64 = 0.25;
const TUBE_LEN: f64 = 0.25;
/// The defuelling valve: pebbles rest on this plane until extracted.
const Z_VALVE: f64 = -(H_CONE + TUBE_LEN);

const YOUNGS_MODULUS: f64 = 5.0e8;
const DT: f64 = 3.5e-5;

/// Steps allowed for the bed to slump into the conus before recirculation.
const PRESETTLE_STEPS: usize = 15_000;
/// Pebbles extracted and re-inserted per batch.
const BATCH: usize = 50;
/// Settling steps between batches.
const SETTLE_STEPS: usize = 2_000;
/// Number of batches.
const BATCHES: usize = 28;

fn data_path(name: &str) -> std::path::PathBuf {
    std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../reference-data/liggghts")
        .join(name)
}

fn load_positions(name: &str) -> Option<Vec<Vec3>> {
    let text = std::fs::read_to_string(data_path(name)).ok()?;
    let mut rows: Vec<(usize, Vec3)> = text
        .lines()
        .skip(1)
        .filter(|l| !l.trim().is_empty())
        .map(|l| {
            let f: Vec<f64> = l.split(',').map(|v| v.parse().expect("numeric")).collect();
            (f[0] as usize, Vec3::new(f[1], f[2], f[3]))
        })
        .collect();
    rows.sort_by_key(|r| r.0);
    Some(rows.into_iter().map(|r| r.1).collect())
}

fn pebble(x: Vec3) -> Particle {
    let m = RHO * 4.0 / 3.0 * std::f64::consts::PI * R_P.powi(3);
    Particle::new(
        x,
        Vec3::zero(),
        Vec3::zero(),
        Mass::new::<kilogram>(m),
        Length::new::<meter>(R_P),
        ThermodynamicTemperature::new::<kelvin>(300.0),
    )
    .expect("valid pebble")
}

/// Bulk solid fraction over a slab excluding `4 r` at the floor and the free
/// surface, by exact sphere-cap integration — the same measure
/// `htr10_pebble_bed.rs` uses, so the numbers are directly comparable.
///
/// Only pebbles **above the conus inlet** (`z > 0`) enter, so the measure is of
/// the cylindrical core and not of the funnel.
fn bulk_solid_fraction(centres: &[Vec3]) -> (f64, f64) {
    let mut zs: Vec<f64> = centres.iter().map(|c| c.z).filter(|z| *z > 0.0).collect();
    if zs.len() < 100 {
        return (f64::NAN, f64::NAN);
    }
    zs.sort_by(|a, b| a.partial_cmp(b).expect("finite"));
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
    let solid: f64 = centres.iter().map(|c| cap(c.z)).sum();
    let slab = std::f64::consts::PI * R_CORE * R_CORE * (z_hi - z_lo);
    (solid / slab, zs[zs.len() - 1])
}

/// Golden-angle spiral over the core cross-section — a deterministic,
/// low-discrepancy re-insertion pattern.
///
/// Deliberately **not** a random draw: this crate has no RNG dependency and
/// does not need one here, and a fixed spiral keeps the case bit-reproducible.
/// Consecutive points are far apart, so a batch inserted at one height cannot
/// self-overlap: 50 points over `r <= 0.85 m` sit ~0.21 m apart against a
/// 0.06 m pebble.
fn insertion_site(k: usize, n: usize, z: f64) -> Vec3 {
    let golden = std::f64::consts::PI * (3.0 - 5.0_f64.sqrt());
    let frac = (k as f64 + 0.5) / n as f64;
    let r = 0.85 * frac.sqrt();
    let theta = golden * k as f64;
    Vec3::new(r * theta.cos(), r * theta.sin(), z)
}

/// Slow (quasi-static) recirculation of the HTR-10 bed through its published
/// conus and discharge tube.
///
/// See the module docs for the question, the geometry and the caveats.
///
/// **Methodology.** Start from the settled bed of `htr10_pebble_bed.rs`, remove
/// its flat floor and attach the conus + discharge tube, and let it slump
/// (`PRESETTLE_STEPS`). Then, per batch: remove the `BATCH` lowest pebbles
/// resting on the defuelling valve, re-insert the same number at the top on a
/// golden-angle spiral, and settle for `SETTLE_STEPS`. `φ` is measured over the
/// cylindrical core only, by the same slab measure as the settling case.
///
/// **Pass criteria.** (1) The run stays quasi-static — mean kinetic energy per
/// pebble stays far below the gravitational energy of a single pebble-radius
/// drop, so the bed is in enduring contact rather than avalanching. (2) The bed
/// neither collapses nor explodes. (3) Pebble count is conserved exactly.
///
/// **This asserts no direction for `φ`.** Whether slow recirculation densifies
/// the bed is the open question the case exists to measure; writing the
/// expected answer into an assertion would destroy the measurement.
///
/// **Results.** Filled in from the measured run.
#[test]
#[cfg_attr(
    not(feature = "long-tests"),
    ignore = "long test: 27 554 pebbles, pre-settle plus 28 recirculation batches. \
              Runs by default; skipped under --no-default-features"
)]
fn slow_recirculation_through_the_discharge_system() {
    let Some(init) = load_positions("htr10_settled_ours.csv") else {
        eprintln!("skipping: reference-data/liggghts/htr10_settled_ours.csv not present");
        return;
    };
    let stl = match std::fs::read_to_string(data_path("htr10_discharge.stl")) {
        Ok(t) => t,
        Err(e) => {
            eprintln!("skipping: htr10_discharge.stl unreadable: {e}");
            return;
        }
    };
    let mesh = MeshWall::from_ascii_stl(&stl).expect("valid discharge mesh");
    eprintln!(
        "N = {}, discharge mesh {} facets",
        init.len(),
        mesh.triangles.len()
    );

    let material =
        GranularMaterial::new(YOUNGS_MODULUS, 0.2, 0.5, 0.4).expect("valid graphite material");
    let model = GranularContactModel::hertz_history(material)
        .with_rolling(RollingModel::cdt(0.1).expect("valid mu_r"));
    let boundaries = vec![
        Boundary::cylinder(Vec3::zero(), Vec3::new(0.0, 0.0, 1.0), R_CORE).expect("core barrel"),
        // the defuelling valve at the bottom of the tube — NOT a floor under
        // the core; the conus is what the bed rests on.
        Boundary::wall(Vec3::new(0.0, 0.0, Z_VALVE), Vec3::new(0.0, 0.0, 1.0)).expect("valve"),
    ];

    let particles: Vec<Particle> = init.iter().map(|x| pebble(*x)).collect();
    let n0 = particles.len();
    let mut sys = GranularSystem::new(
        particles,
        boundaries,
        model,
        Vec3::new(0.0, 0.0, -9.81),
        DT,
    )
    .expect("valid system")
    .with_moving_walls(vec![MovingBoundary::new(
        // Static in practice: the discharge geometry does not move. It is a
        // MovingBoundary only because that is the path a mesh wall takes.
        WallGeometry::Mesh(mesh),
        Vec3::zero(),
        Vec3::zero(),
        Vec3::zero(),
    )]);

    let started = std::time::Instant::now();
    sys.run(PRESETTLE_STEPS);

    let m_pebble = RHO * 4.0 / 3.0 * std::f64::consts::PI * R_P.powi(3);
    // Gravitational energy of a one-radius drop, per pebble — the yardstick for
    // "is this quasi-static?".
    let e_drop = m_pebble * 9.81 * R_P;

    let mut log = String::from("batch,extracted,n,phi,bed_top,ke_per_pebble_over_edrop\n");
    let mut worst_ratio = 0.0_f64;
    let record = |sys: &GranularSystem, batch: usize, extracted: usize, log: &mut String| -> f64 {
        let centres: Vec<Vec3> = sys.particles().iter().map(|p| p.position).collect();
        let (phi, top) = bulk_solid_fraction(&centres);
        let ratio = sys.kinetic_energy() / sys.particles().len() as f64 / e_drop;
        log.push_str(&format!(
            "{batch},{extracted},{},{phi:.6},{top:.6},{ratio:.6e}\n",
            sys.particles().len()
        ));
        eprintln!(
            "batch {batch:3}  extracted {extracted:5}  N {:5}  phi {phi:.4}  top {top:.4} m  \
             KE/pebble / E_drop {ratio:.2e}",
            sys.particles().len()
        );
        ratio
    };
    let phi_0 = {
        let r = record(&sys, 0, 0, &mut log);
        worst_ratio = worst_ratio.max(r);
        let centres: Vec<Vec3> = sys.particles().iter().map(|p| p.position).collect();
        bulk_solid_fraction(&centres).0
    };

    let mut extracted = 0usize;
    for batch in 1..=BATCHES {
        // --- extract: the lowest pebbles resting on the valve ---
        let mut by_z: Vec<(usize, f64)> = sys
            .particles()
            .iter()
            .enumerate()
            .map(|(i, p)| (i, p.position.z))
            .filter(|(_, z)| *z < -H_CONE)
            .collect();
        by_z.sort_by(|a, b| a.1.partial_cmp(&b.1).expect("finite z"));
        let doomed: Vec<usize> = by_z.iter().take(BATCH).map(|(i, _)| *i).collect();
        let removed = sys.remove_particles(&doomed).len();
        extracted += removed;

        // --- re-insert the same number at the top ---
        let top = sys
            .particles()
            .iter()
            .map(|p| p.position.z)
            .fold(f64::NEG_INFINITY, f64::max);
        for k in 0..removed {
            sys.insert_particle(pebble(insertion_site(k, removed.max(1), top + 0.02)));
        }
        assert_eq!(
            sys.particles().len(),
            n0,
            "recirculation must conserve pebble count"
        );

        sys.run(SETTLE_STEPS);
        worst_ratio = worst_ratio.max(record(&sys, batch, extracted, &mut log));
    }
    let elapsed = started.elapsed().as_secs_f64();

    let centres: Vec<Vec3> = sys.particles().iter().map(|p| p.position).collect();
    let (phi_end, top_end) = bulk_solid_fraction(&centres);
    eprintln!(
        "RECIRCULATION DONE: phi {phi_0:.4} -> {phi_end:.4} ({:+.4}) over {extracted} pebbles \
         ({:.1} % of the bed);  bed top {top_end:.4} m;  worst KE/E_drop {worst_ratio:.2e};  \
         {elapsed:.0} s",
        phi_end - phi_0,
        100.0 * extracted as f64 / n0 as f64
    );
    let _ = std::fs::write(data_path("htr10_recirculation.csv"), &log);

    // (1) the run must have stayed quasi-static, or it measured the wrong regime
    assert!(
        worst_ratio < 1.0e-2,
        "not quasi-static: kinetic energy per pebble reached {worst_ratio:.2e} of a \
         one-radius drop; the bed was avalanching, not creeping"
    );
    // (2) still a bed
    assert!(
        (0.50..0.68).contains(&phi_end),
        "solid fraction {phi_end:.4} left the physical range for a random sphere packing"
    );
    // (3) conservation
    assert_eq!(sys.particles().len(), n0);
    assert!(extracted > 0, "no pebbles were extracted — check the valve");
}
