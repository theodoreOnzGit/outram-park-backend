// SPDX-License-Identifier: GPL-3.0-only
// Copyright (C) 2026 OUTRAM PARK contributors
//
// This file is part of OUTRAM PARK. See the LICENSE file at the crate root.

//! # `fix insert/stream` against LIGGGHTS-PUBLIC (gh:#1414)
//!
//! ## Methodology
//!
//! Decks `reference-data/liggghts/in.stream_ol` and `in.stream_noch`
//! (`make_stream_decks.sh`): the D/d = 6 cylinder of `in.pebble_bed`, an
//! insertion face `stream_face.stl` (0.04 m square, z = 0.15, two triangles),
//! `r = 3 mm` spheres, `extrude_length 0.05`, `vel` into the face at
//! 1 m/s. `stream_ol`: `overlapcheck yes`, `vel uniform` (released velocity
//! and spin-free rail), 50 particles per insertion, 200 in total.
//! `stream_noch`: `overlapcheck no`, `vel constant 0.05 0 -1`, 100 in total.
//! Insertion steps 1, 10001, 20001, 30001. Frames (`run N pre no`, one
//! continuous run, `%.17g`) at 1, 2, 5000, 10001, 10002, 12000, 20001, 25000,
//! 30000, 34000, 40000; every particle's `x y z v omega` compared on bit
//! patterns, matched by id.
//!
//! ## Prediction (written before the first comparison, 2026-11-11)
//!
//! Bit-identical everywhere: positions come from the translated mesh and fix
//! streams (including the `ntry_mc` Monte-Carlo insertion fraction that
//! advances both before the first position), rails and release are the same
//! arithmetic, and contacts after the first particle reaches the floor
//! (step ~30 000) use the already-verified neighbour-order translation. Any
//! mismatch is a finding.
//!
//! ## Result
//!
//! Printed with `--nocapture`; recorded in
//! `verification_and_validation/insertion/README.md`.

use outram_park_fork_liggghts::boundary::Boundary;
use outram_park_fork_liggghts::granular::{GranularContactModel, GranularMaterial};
use outram_park_fork_liggghts::granular_system::GranularSystem;
use outram_park_fork_liggghts::insert::{InsertVelocity, ParticleDistributionDiscrete, ParticleTemplateSphere, Rate, RateTotal};
use outram_park_fork_liggghts::insert_stream::{InsertStreamFix, StreamExtent};
use outram_park_fork_liggghts::mesh_wall::MeshWall;
use outram_park_fork_liggghts::neighbor_list::LiggghtsNeighbor;
use outram_park_fork_liggghts::particle::Vec3;
use uom::si::f64::{Length, MassDensity};
use uom::si::{length::meter, mass_density::kilogram_per_cubic_meter};

const FRAMES: [u64; 11] = [1, 2, 5000, 10001, 10002, 12000, 20001, 25000, 30000, 34000, 40000];
const BOX_LO: Vec3 = Vec3 { x: -0.035, y: -0.035, z: -0.005 };
const BOX_HI: Vec3 = Vec3 { x: 0.035, y: 0.035, z: 0.35 };

fn read(name: &str) -> Option<String> {
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../reference-data/liggghts")
        .join(name);
    std::fs::read_to_string(path).ok()
}

fn fix_for(deck: &str, face: &MeshWall) -> InsertStreamFix {
    let t = ParticleTemplateSphere::new(
        15485863,
        1,
        Length::new::<meter>(0.003),
        MassDensity::new::<kilogram_per_cubic_meter>(2500.0),
    )
    .unwrap();
    let dist = ParticleDistributionDiscrete::new(15485867, true, vec![(t, 1.0)]).unwrap();
    let (total, rate, vel, ol) = if deck == "stream_ol" {
        (
            RateTotal::Particles(200),
            Rate::Particles(1000.0),
            InsertVelocity::Uniform {
                v: Vec3::new(0.0, 0.0, -1.0),
                fluct: Vec3::new(0.05, 0.05, 0.05),
            },
            true,
        )
    } else {
        (
            RateTotal::Particles(100),
            Rate::Particles(500.0),
            InsertVelocity::Constant(Vec3::new(0.05, 0.0, -1.0)),
            false,
        )
    };
    InsertStreamFix::new(
        32452843,
        dist,
        face,
        StreamExtent::ExtrudeLength(0.05),
        Some(total),
        rate,
        vel,
        5e-6,
        BOX_LO,
        BOX_HI,
    )
    .unwrap()
    .with_overlap_check(ol)
}

fn system(deck: &str) -> GranularSystem {
    let face = MeshWall::from_ascii_stl(&read("stream_face.stl").unwrap()).unwrap();
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
    sys.add_stream_insertion(fix_for(deck, &face));
    sys.with_liggghts_neighbor_order(LiggghtsNeighbor::new(BOX_LO, BOX_HI, 0.003).unwrap())
}

fn frame_rows(csv: &str, step: u64) -> Vec<[f64; 10]> {
    csv.lines()
        .skip(1)
        .filter_map(|l| {
            let v: Vec<f64> = l.split(',').map(|x| x.parse().unwrap()).collect();
            (v[0] as u64 == step).then(|| {
                let mut r = [0.0; 10];
                r.copy_from_slice(&v[1..11]);
                r
            })
        })
        .collect()
}

fn report(deck: &str) {
    let Some(csv) = read(&format!("{deck}.csv")) else {
        eprintln!("skipping: {deck}.csv not present");
        return;
    };
    let mut sys = system(deck);
    let mut at = 0u64;
    let mut total_bad = 0;
    for &step in &FRAMES {
        sys.run((step - at) as usize);
        at = step;
        let rows = frame_rows(&csv, step);
        let ours = sys.particles();
        assert_eq!(ours.len(), rows.len(), "{deck} step {step}: particle count");
        let (mut same_n, mut worst) = (0, 0.0f64);
        for r in &rows {
            let p = &ours[r[0] as usize - 1];
            let mine = [
                p.position.x, p.position.y, p.position.z, p.velocity.x, p.velocity.y,
                p.velocity.z, p.angular_velocity.x, p.angular_velocity.y, p.angular_velocity.z,
            ];
            let mut same = true;
            for k in 0..9 {
                if mine[k].to_bits() != r[1 + k].to_bits() {
                    same = false;
                    worst = worst.max((mine[k] - r[1 + k]).abs());
                }
            }
            if same {
                same_n += 1;
            }
        }
        println!("{deck} step {step:6}: {same_n}/{} bit-identical, worst |d| {worst:.3e}", rows.len());
        total_bad += rows.len() - same_n;
    }
    assert_eq!(total_bad, 0, "{deck}: {total_bad} particle-frames differ");
}

/// `overlapcheck yes`, `vel uniform`.
#[test]
fn insert_stream_overlapcheck_matches_liggghts() {
    report("stream_ol");
}

/// `overlapcheck no`, `vel constant` with a lateral component.
#[test]
fn insert_stream_nocheck_matches_liggghts() {
    report("stream_noch");
}

/// Constructor checks upstream makes.
#[test]
fn insert_stream_rejects_bad_setups() {
    let face = MeshWall::from_ascii_stl(&read("stream_face.stl").unwrap()).unwrap();
    let mk = |ext, vel| {
        let t = ParticleTemplateSphere::new(
            15485863, 1, Length::new::<meter>(0.003),
            MassDensity::new::<kilogram_per_cubic_meter>(2500.0),
        ).unwrap();
        let dist = ParticleDistributionDiscrete::new(15485867, true, vec![(t, 1.0)]).unwrap();
        InsertStreamFix::new(32452843, dist, &face, ext, Some(RateTotal::Particles(5)),
            Rate::Particles(100.0), vel, 5e-6, BOX_LO, BOX_HI)
    };
    let down = InsertVelocity::Constant(Vec3::new(0.0, 0.0, -1.0));
    assert!(mk(StreamExtent::ExtrudeLength(0.05), down).is_ok());
    // velocity parallel to the face
    assert!(mk(StreamExtent::ExtrudeLength(0.05), InsertVelocity::Constant(Vec3::new(1.0, 0.0, 0.0))).is_err());
    // too short for overlapcheck
    assert!(mk(StreamExtent::ExtrudeLength(0.005), down).is_err());
    assert!(mk(StreamExtent::InsertEvery { every: 10, duration: Some(20) }, down).is_err());
}
