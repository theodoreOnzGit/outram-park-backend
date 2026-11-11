// SPDX-License-Identifier: AGPL-3.0-only
//! # Multicontact surface model + `fix multicontact/halfspace` against LIGGGHTS-PUBLIC
//!
//! gh:#1410 (step 3). Deck `reference-data/liggghts/in.multicontact`
//! (LIGGGHTS-PUBLIC `3d5c00f2`, `pair_style gran ... surface multicontact`,
//! two `wall/gran ... surface multicontact primitive zplane`, `fix mc all
//! multicontact/halfspace`).
//!
//! ## Methodology
//! Six soft spheres (`d = 10 mm`, `rho = 2500`, `E = 1e5`, `nu = 0.3`,
//! `e = 0.5`, `mu = 0.3`, Hertz + history, no gravity) start from
//! `multicontact_init.csv`, overlapping each other and two `z` planes 19.5 mm
//! apart, so particles carry a wall contact and pair contacts at once and the
//! half-space radius expansion is active. 600 steps of 2 us; the upstream
//! trajectory (every 20 steps, `%.17g`) is `multicontact.csv`. The port runs
//! `GranularSystem::with_liggghts_neighbor_order(..).with_multicontact(..)`.
//!
//! ## Prediction (written before the run, 2026-11-11)
//! * Step 0 identical (same input). Frames after that: bit-identical or within
//!   ulp-level growth, because pair order and sort are reproduced
//!   (`LiggghtsNeighbor`) and the arithmetic is translated line by line; the
//!   only expected divergences are (a) history carry-over when a list rebuild
//!   flips a pair's orientation, (b) the multicontact wall `surfacesClose`
//!   history of the cylinder (not used here). Pass criterion: every value
//!   within `1e-8` of its column's largest magnitude; the bit-identical count
//!   and the worst difference are printed. A control run WITHOUT the model must
//!   differ from upstream by far more (so the model is exercised).
//!
//! ## Result
//! See the printed line (`--nocapture`) and `docs/cross-code-summary.md`.

use outram_park_fork_liggghts::boundary::Boundary;
use outram_park_fork_liggghts::granular::{GranularContactModel, GranularMaterial};
use outram_park_fork_liggghts::granular_system::GranularSystem;
use outram_park_fork_liggghts::multicontact::HalfSpaceFix;
use outram_park_fork_liggghts::neighbor_list::LiggghtsNeighbor;
use outram_park_fork_liggghts::particle::{Particle, Vec3};
use uom::si::f64::{Length, Mass, ThermodynamicTemperature};
use uom::si::{length::meter, mass::kilogram, thermodynamic_temperature::kelvin};

const INIT: &str = include_str!("../../../reference-data/liggghts/multicontact_init.csv");
const REF: &str = include_str!("../../../reference-data/liggghts/multicontact.csv");

fn system(multicontact: bool) -> GranularSystem {
    let r = 0.005;
    let mass = 4.0 * std::f64::consts::PI / 3.0 * r * r * r * 2500.0;
    let particles: Vec<Particle> = INIT
        .lines()
        .skip(1)
        .filter(|l| !l.trim().is_empty())
        .map(|l| {
            let v: Vec<f64> = l.split(',').map(|x| x.parse().unwrap()).collect();
            Particle::new(
                Vec3::new(v[1], v[2], v[3]),
                Vec3::new(v[4], v[5], v[6]),
                Vec3::zero(),
                Mass::new::<kilogram>(mass),
                Length::new::<meter>(r),
                ThermodynamicTemperature::new::<kelvin>(300.0),
            )
            .unwrap()
        })
        .collect();
    let material = GranularMaterial::new(1.0e5, 0.3, 0.5, 0.3).unwrap();
    let boundaries = vec![
        Boundary::plane(Vec3::zero(), Vec3::new(0.0, 0.0, 1.0)).unwrap(),
        Boundary::plane(Vec3::new(0.0, 0.0, 0.0195), Vec3::new(0.0, 0.0, 1.0)).unwrap(),
    ];
    let sys = GranularSystem::new(
        particles,
        boundaries,
        GranularContactModel::hertz_history(material),
        Vec3::zero(),
        2e-6,
    )
    .unwrap();
    let cfg = LiggghtsNeighbor::new(Vec3::new(-1.0, -1.0, -0.005), Vec3::new(1.0, 1.0, 0.03), 0.002).unwrap();
    let sys = sys.with_liggghts_neighbor_order(cfg);
    if multicontact {
        sys.with_multicontact(HalfSpaceFix::new(vec![1.0e5], vec![0.3])).unwrap()
    } else {
        sys
    }
}

/// Run and return `(bit-identical values, total values, worst |diff| / column scale)`.
fn compare(multicontact: bool) -> (usize, usize, f64) {
    let mut sys = system(multicontact);
    let mut rows = REF.lines();
    let _ = rows.next();
    let (mut same, mut total, mut worst) = (0usize, 0usize, 0.0_f64);
    let mut done = 0usize;
    let mut frames: Vec<(usize, Vec<f64>)> = Vec::new();
    for l in rows.filter(|l| !l.trim().is_empty()) {
        let v: Vec<f64> = l.split(',').map(|x| x.parse().unwrap()).collect();
        frames.push((v[0] as usize, v[1..].to_vec()));
    }
    let nvar = 9;
    let scale: Vec<f64> = (0..nvar)
        .map(|c| frames.iter().flat_map(|f| f.1.iter().skip(c).step_by(nvar)).fold(0.0_f64, |m, x| m.max(x.abs())))
        .collect();
    for (step, vals) in &frames {
        sys.run(*step - done);
        done = *step;
        for (n, p) in sys.particles().iter().enumerate() {
            if std::env::var("MCDBG").is_ok() && *step <= 60 { println!("step {step} p{n} dz={:e} dx={:e} dvz={:e}", p.position.z - vals[n*9+2], p.position.x - vals[n*9], p.velocity.z - vals[n*9+5]); }
            let ours = [
                p.position.x, p.position.y, p.position.z, p.velocity.x, p.velocity.y, p.velocity.z,
                p.angular_velocity.x, p.angular_velocity.y, p.angular_velocity.z,
            ];
            for c in 0..nvar {
                let theirs = vals[n * nvar + c];
                total += 1;
                if ours[c].to_bits() == theirs.to_bits() {
                    same += 1;
                }
                if scale[c] > 0.0 {
                    worst = worst.max((ours[c] - theirs).abs() / scale[c]);
                }
            }
        }
    }
    (same, total, worst)
}

/// Whole-trajectory comparison (integration test of the pair, wall, pre_force
/// and history hooks together).
#[test]
fn multicontact_trajectory_matches_upstream() {
    let (same, total, worst) = compare(true);
    println!("multicontact: {same}/{total} values bit-identical, worst |diff|/scale = {worst:e}");
    assert!(worst <= 1e-8, "worst {worst:e}");
}

/// The model is exercised: a run without it misses upstream by far more.
#[test]
fn control_without_multicontact_differs() {
    let (_, _, with) = compare(true);
    let (_, _, without) = compare(false);
    println!("control: with {with:e}, without {without:e}");
    assert!(without > 10.0 * with.max(1e-12), "with {with:e} without {without:e}");
}

/// Opt-in: multicontact needs the LIGGGHTS neighbour order.
#[test]
fn multicontact_requires_liggghts_neighbor_order() {
    let sys = GranularSystem::new(
        Vec::new(),
        Vec::new(),
        GranularContactModel::hertz_history(GranularMaterial::new(1e5, 0.3, 0.5, 0.3).unwrap()),
        Vec3::zero(),
        2e-6,
    )
    .unwrap();
    assert!(sys.with_multicontact(HalfSpaceFix::new(vec![1e5], vec![0.3])).is_err());
}
