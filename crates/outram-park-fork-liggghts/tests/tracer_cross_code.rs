// SPDX-License-Identifier: GPL-3.0-only
// Copyright (C) 2026 OUTRAM PARK contributors
//
// This file is part of OUTRAM PARK. See the LICENSE file at the crate root.

//! # `fix property/atom/tracer` + `compute nparticles/tracer/region` against LIGGGHTS-PUBLIC (gh:#1418)
//!
//! ## Methodology
//!
//! Deck `reference-data/liggghts/in.tracer` (LIGGGHTS-PUBLIC `3d5c00f2`): the
//! `stream_ol` insert/stream deck (200 spheres, 50 at steps 1, 10001, 20001,
//! 30001) with two tracers marking particles found in the block `rm`
//! (`z` 0.06..0.12): `tr1` heaviside, `mark_step 3`, `check_mark_every 7`;
//! `tr2` dirac, `mark_step 9001`, `check_mark_every 10` (marks once, at step
//! 9010). Two `compute nparticles/tracer/region` over the block `rc`: `ntr1`
//! with `reset_marker yes` (clears markers in the mass pass, so it feeds
//! back into `tr1`), `ntr2` with `reset_marker no`. Thermo every 1000 steps
//! (`%.17g`) prints `step atoms c_ntr1[1..4] c_ntr2[1..4] f_tr1 f_tr2`
//! (`tracer_thermo.txt`); a dump every 5000 steps writes both per-atom markers
//! (`tracer_markers_dump.txt`; LIGGGHTS writes dumps before thermo at a step).
//! The port runs the same system and replays the same fix/compute order after
//! every step; all columns and all markers are compared on bit patterns.
//!
//! ## Prediction (written before the first comparison, 2026-11-11)
//!
//! Bit-identical in every column: positions are bit-identical to LIGGGHTS
//! (`insert_stream_cross_code`), the region test is the same comparison, the
//! counts are exact integers, and the mass sums are accumulated in the
//! upstream local (post-sort) order. Any mismatch is a finding.
//!
//! ## Result
//!
//! Printed by `--nocapture`; recorded in
//! `verification_and_validation/tracer/README.md`.

use outram_park_fork_liggghts::boundary::Boundary;
use outram_park_fork_liggghts::granular::{GranularContactModel, GranularMaterial};
use outram_park_fork_liggghts::granular_system::GranularSystem;
use outram_park_fork_liggghts::insert::{InsertRegion, InsertVelocity, ParticleDistributionDiscrete, ParticleTemplateSphere, Rate, RateTotal};
use outram_park_fork_liggghts::insert_stream::{InsertStreamFix, StreamExtent};
use outram_park_fork_liggghts::mesh_wall::MeshWall;
use outram_park_fork_liggghts::neighbor_list::LiggghtsNeighbor;
use outram_park_fork_liggghts::particle::Vec3;
use outram_park_fork_liggghts::tracer::{MarkerStyle, NparticlesTracerRegion, PropertyAtomTracer};
use uom::si::f64::{Length, MassDensity};
use uom::si::{length::meter, mass_density::kilogram_per_cubic_meter};

const THERMO: &str = include_str!("../../../reference-data/liggghts/tracer_thermo.txt");
const DUMP: &str = include_str!("../../../reference-data/liggghts/tracer_markers_dump.txt");
const STL: &str = include_str!("../../../reference-data/liggghts/stream_face.stl");
const BOX_LO: Vec3 = Vec3 { x: -0.035, y: -0.035, z: -0.005 };
const BOX_HI: Vec3 = Vec3 { x: 0.035, y: 0.035, z: 0.35 };

fn system() -> GranularSystem {
    let face = MeshWall::from_ascii_stl(STL).unwrap();
    let material = GranularMaterial::new(5.0e6, 0.3, 0.5, 0.3).unwrap();
    let boundaries = vec![
        Boundary::cylinder(Vec3::zero(), Vec3::new(0.0, 0.0, 1.0), 0.030).unwrap(),
        Boundary::wall(Vec3::zero(), Vec3::new(0.0, 0.0, 1.0)).unwrap(),
    ];
    let mut sys = GranularSystem::new(
        Vec::new(),
        boundaries,
        GranularContactModel::hertz_history(material),
        Vec3::new(0.0, 0.0, -9.81),
        5e-6,
    )
    .unwrap();
    let t = ParticleTemplateSphere::new(
        15485863,
        1,
        Length::new::<meter>(0.003),
        MassDensity::new::<kilogram_per_cubic_meter>(2500.0),
    )
    .unwrap();
    let dist = ParticleDistributionDiscrete::new(15485867, true, vec![(t, 1.0)]).unwrap();
    let fix = InsertStreamFix::new(
        32452843,
        dist,
        &face,
        StreamExtent::ExtrudeLength(0.05),
        Some(RateTotal::Particles(200)),
        Rate::Particles(1000.0),
        InsertVelocity::Uniform { v: Vec3::new(0.0, 0.0, -1.0), fluct: Vec3::new(0.05, 0.05, 0.05) },
        5e-6,
        BOX_LO,
        BOX_HI,
    )
    .unwrap()
    .with_overlap_check(true);
    sys.add_stream_insertion(fix);
    sys.with_liggghts_neighbor_order(LiggghtsNeighbor::new(BOX_LO, BOX_HI, 0.003).unwrap())
}

/// Markers of every atom in the dump frame at `step`, by id.
fn dump_frame(step: i64) -> Vec<(f64, f64)> {
    let mut lines = DUMP.lines();
    while let Some(l) = lines.next() {
        if l.starts_with("ITEM: TIMESTEP") {
            let s: i64 = lines.next().unwrap().trim().parse().unwrap();
            if s == step {
                let mut rows = Vec::new();
                let mut in_atoms = false;
                for l in lines.by_ref() {
                    if l.starts_with("ITEM: TIMESTEP") {
                        break;
                    }
                    if l.starts_with("ITEM: ATOMS") {
                        in_atoms = true;
                    } else if in_atoms {
                        let v: Vec<f64> = l.split_whitespace().map(|x| x.parse().unwrap()).collect();
                        rows.push((v[1], v[2]));
                    }
                }
                return rows;
            }
        }
    }
    panic!("no frame {step}");
}

/// Both tracers and both computes against upstream for 40 000 steps (see the
/// module docs for the setup, the prediction and the result).
#[test]
fn tracer_and_nparticles_region_match_liggghts() {
    let rm = InsertRegion::block(Vec3::new(-0.03, -0.03, 0.06), Vec3::new(0.03, 0.03, 0.12)).unwrap();
    let rc = InsertRegion::block(Vec3::new(-0.03, -0.03, -0.005), Vec3::new(0.03, 0.03, 0.2)).unwrap();
    let mut tr1 = PropertyAtomTracer::new(rm, 3, MarkerStyle::Heaviside, 7, 0).unwrap();
    let mut tr2 = PropertyAtomTracer::new(rm, 9001, MarkerStyle::Dirac, 10, 0).unwrap();
    let c1 = NparticlesTracerRegion::new(rc, true);
    let c2 = NparticlesTracerRegion::new(rc, false);
    let mut sys = system();
    let (mut total, mut same) = (0usize, 0usize);
    let mut cmp = |ours: f64, theirs: f64, what: &str, step: i64| {
        total += 1;
        if ours.to_bits() == theirs.to_bits() {
            same += 1;
        } else {
            eprintln!("step {step} {what}: ours {ours:e} upstream {theirs:e}");
        }
    };
    let mut rows = THERMO.lines();
    rows.next(); // step 0: nothing inserted yet
    for step in 1..=40000i64 {
        sys.run(1);
        tr1.end_of_step(step, sys.particles());
        tr2.end_of_step(step, sys.particles());
        if step % 5000 == 0 {
            let frame = dump_frame(step);
            assert_eq!(frame.len(), sys.particles().len());
            for (i, (a, b)) in frame.iter().enumerate() {
                cmp(tr1.marker(i), *a, "marker tr1", step);
                cmp(tr2.marker(i), *b, "marker tr2", step);
            }
        }
        if step % 1000 == 0 {
            let r: Vec<f64> = rows.next().unwrap().split_whitespace().map(|x| x.parse().unwrap()).collect();
            assert_eq!(r[0] as i64, step);
            let order = sys.local_order_indices();
            let v1 = c1.compute_vector(&mut tr1, sys.particles(), &order);
            let v2 = c2.compute_vector(&mut tr2, sys.particles(), &order);
            cmp(sys.particles().len() as f64, r[1], "atoms", step);
            for k in 0..4 {
                cmp(v1[k], r[2 + k], "ntr1", step);
                cmp(v2[k], r[6 + k], "ntr2", step);
            }
            cmp(tr1.compute_scalar(), r[10], "f_tr1", step);
            cmp(tr2.compute_scalar(), r[11], "f_tr2", step);
        }
    }
    println!("tracer: {same}/{total} bit-identical");
    assert_eq!(same, total);
}
