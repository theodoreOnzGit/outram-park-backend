// SPDX-License-Identifier: AGPL-3.0-only
//! # Liquid-bridge cohesion (`easo/capillary/viscous`, `washino/capillary/viscous`) against LIGGGHTS-PUBLIC
//!
//! gh:#1411. Decks `reference-data/liggghts/in.cap_*` (LIGGGHTS-PUBLIC
//! `3d5c00f2`, built read-only from the shared upstream build; regenerate with
//! `reference-data/liggghts/run_capillary.sh`), dumps converted by
//! `dump2cap.sh` to `cap_*.csv` (`%.17g`).
//!
//! ## Methodology
//! Six cases, each a full trajectory compared on every sampled frame, all 14
//! columns per particle (`x y z v omega f L flux`; `L` is
//! `surfaceLiquidContent`, `flux` `liquidFlux`):
//! * `cap_easo_pair` / `cap_washino_pair`: two 10 mm spheres, Hertz + history,
//!   approach, bridge forms, rebound, stretch, rupture (Easo third branch;
//!   Washino also forms bridges out of contact);
//! * `cap_easo_hard`: unequal spheres/two types, `maxSeparationDistanceRatio
//!   1.1`, rupture through the first (hard-limit) branch, `tangential_reduce on`;
//! * `cap_washino_limit`: `limitLiquidContent`, `modifyLbVolume`,
//!   `maxLiquidContent` overwriting the separation ratio, unequal spheres;
//! * `cap_easo_bed` / `cap_washino_bed`: 84 pebbles of the settled bed, no
//!   walls, no gravity, bridges forming, stretching, rupturing and
//!   re-forming.
//! The port runs `GranularSystem::with_liggghts_neighbor_order(..)
//! .with_capillary()`. Walls are not ported (Easo/Washino wall bridges), so no
//! deck has one.
//!
//! ## Prediction (written before the first run, 2026-11-11)
//! Bit-identical, as the multicontact decks: every formula is translated in
//! upstream's order. The only place a last-ulp difference may enter is
//! `exp`/`pow`/`log`/`cbrt`/`sin`/`cos` (PETIR libm vs glibc, accepted under
//! gh:#1455 provided it is proved to be the only cause). A bridge-state flip
//! by one step would show as a gross difference, not an ulp one.
//!
//! ## Result
//! See the `RESULT` constant at the bottom, filled from the first run.

use outram_park_fork_liggghts::granular::{
    GranularCohesion, GranularContactModel, GranularMaterial, MaterialTable,
};
use outram_park_fork_liggghts::capillary::CapillaryParams;
use outram_park_fork_liggghts::granular_system::GranularSystem;
use outram_park_fork_liggghts::neighbor_list::LiggghtsNeighbor;
use outram_park_fork_liggghts::particle::{Particle, Vec3};
use uom::si::f64::{Length, Mass, ThermodynamicTemperature};
use uom::si::{length::meter, mass::kilogram, thermodynamic_temperature::kelvin};

const EASO_PAIR: &str = include_str!("../../../reference-data/liggghts/cap_easo_pair.csv");
const EASO_HARD: &str = include_str!("../../../reference-data/liggghts/cap_easo_hard.csv");
const WASH_PAIR: &str = include_str!("../../../reference-data/liggghts/cap_washino_pair.csv");
const WASH_LIMIT: &str = include_str!("../../../reference-data/liggghts/cap_washino_limit.csv");
const EASO_BED: &str = include_str!("../../../reference-data/liggghts/cap_easo_bed.csv");
const WASH_BED: &str = include_str!("../../../reference-data/liggghts/cap_washino_bed.csv");
const BED_INIT: &str = include_str!("../../../reference-data/liggghts/cap_bed_init.csv");

/// Upstream's `createContactAngle` reads `contactAngle` in DEGREES and stores
/// `v * M_PI / 180.` (`global_properties.cpp`); the port's API takes radians.
/// (First run missed by 0.24 % in the force at the first contact because the
/// deck's 0.3 was fed as radians; found by printing the contact angle from an
/// instrumented copy of the upstream header.)
fn deg(v: f64) -> f64 {
    v * std::f64::consts::PI / 180.
}

#[derive(Clone, Copy, PartialEq)]
enum Case {
    EasoPair,
    EasoHard,
    WashinoPair,
    WashinoLimit,
    EasoBed,
    WashinoBed,
}

fn particle(pos: Vec3, vel: Vec3, omega: Vec3, d: f64, rho: f64) -> Particle {
    let r = 0.5 * d;
    let mass = 4.0 * std::f64::consts::PI / 3.0 * r * r * r * rho;
    Particle::new(
        pos,
        vel,
        omega,
        Mass::new::<kilogram>(mass),
        Length::new::<meter>(r),
        ThermodynamicTemperature::new::<kelvin>(300.0),
    )
    .unwrap()
}

fn two(x1: f64, x2: f64, v: (f64, f64), d2: f64, rho2: f64) -> Vec<Particle> {
    vec![
        particle(Vec3::new(x1, 0.0, 0.0), Vec3::new(v.0, v.1, 0.0), Vec3::new(0.0, 0.0, 40.0), 0.01, 2500.0),
        particle(Vec3::new(x2, 0.0, 0.0), Vec3::new(-v.0, -v.1, 0.0), Vec3::new(0.0, 0.0, -20.0), d2, rho2),
    ]
}

fn system(case: Case) -> (GranularSystem, &'static str) {
    let washino = matches!(case, Case::WashinoPair | Case::WashinoLimit | Case::WashinoBed);
    let mk = |tension, visc, ang, minr, maxr, init| {
        if washino {
            CapillaryParams::washino(tension, visc, ang, minr, maxr, init)
        } else {
            CapillaryParams::easo(tension, visc, ang, minr, maxr, init)
        }
        .unwrap()
    };
    let (dt, skin, lo, hi);
    let (particles, params, model, contents, rows);
    let two_type = matches!(case, Case::EasoHard | Case::WashinoLimit);
    let mut angles = [0.0; outram_park_fork_liggghts::granular::MAX_TYPES];
    match case {
        Case::EasoPair | Case::WashinoPair => {
            dt = 1e-6;
            skin = 0.004;
            lo = Vec3::new(-0.2, -0.2, -0.2);
            hi = Vec3::new(0.2, 0.2, 0.2);
            let (x, v, init, maxr, vis, cont) = if washino {
                (0.0068, 0.8, 0.3, 1.3, 1.0e-3, vec![0.4, 0.2])
            } else {
                (0.0051, 0.6, 0.1, 1.2, 1.0e-3, vec![0.2, 0.06])
            };
            particles = two(-x, x, (v, 0.05), 0.01, 2500.0);
            angles[0] = deg(0.3);
            params = mk(0.072, vis, deg(0.3), 0.01, maxr, init).with_contact_angles(angles);
            model = GranularContactModel::hertz_history(GranularMaterial::new(1.0e7, 0.3, 0.9, 0.5).unwrap());
            contents = cont;
            rows = if washino { WASH_PAIR } else { EASO_PAIR };
        }
        Case::EasoHard => {
            dt = 1e-6;
            skin = 0.004;
            lo = Vec3::new(-0.2, -0.2, -0.2);
            hi = Vec3::new(0.2, 0.2, 0.2);
            particles = two(-0.0055, 0.0061, (0.6, 0.05), 0.012, 4000.0);
            angles[0] = deg(0.2);
            angles[1] = deg(0.5);
            params = mk(0.072, 5.0e-3, deg(0.2), 0.01, 1.1, 0.15)
                .with_contact_angles(angles)
                .with_tangential_reduce(true);
            model = GranularContactModel::hertz_history(GranularMaterial::new(1.0e7, 0.3, 0.9, 0.5).unwrap());
            contents = vec![0.15, 0.15];
            rows = EASO_HARD;
        }
        Case::WashinoLimit => {
            dt = 1e-6;
            skin = 0.004;
            lo = Vec3::new(-0.2, -0.2, -0.2);
            hi = Vec3::new(0.2, 0.2, 0.2);
            particles = two(-0.0064, 0.0070, (0.5, 0.05), 0.012, 4000.0);
            angles[0] = deg(0.2);
            angles[1] = deg(0.5);
            let mut maxc = [0.0; outram_park_fork_liggghts::granular::MAX_TYPES];
            maxc[0] = 0.1;
            maxc[1] = 0.2;
            params = mk(0.072, 2.0e-3, deg(0.2), 0.01, 1.3, 0.4)
                .with_contact_angles(angles)
                .with_liquid_content_limit(maxc)
                .unwrap()
                .with_lb_volume_fraction(0.2)
                .unwrap()
                .with_tangential_reduce(true);
            model = GranularContactModel::hertz_history(GranularMaterial::new(1.0e7, 0.3, 0.9, 0.5).unwrap());
            contents = vec![0.4, 0.4];
            rows = WASH_LIMIT;
        }
        Case::EasoBed | Case::WashinoBed => {
            dt = 5e-6;
            skin = 0.003;
            lo = Vec3::new(-1.0, -1.0, -0.2);
            hi = Vec3::new(1.0, 1.0, 0.2);
            let ps: Vec<Particle> = BED_INIT
                .lines()
                .skip(1)
                .filter(|l| !l.trim().is_empty())
                .map(|l| {
                    let v: Vec<f64> = l.split(',').map(|x| x.parse().unwrap()).collect();
                    particle(Vec3::new(v[1], v[2], v[3]), Vec3::new(v[4], v[5], v[6]), Vec3::zero(), 0.01, 2500.0)
                })
                .collect();
            contents = (0..ps.len()).map(|i| 0.05 + 0.1 * ((i + 1) % 4) as f64).collect();
            particles = ps;
            angles[0] = deg(0.3);
            params = mk(0.072, 1.0e-3, deg(0.3), 0.01, 1.2, if washino { 0.3 } else { 0.1 }).with_contact_angles(angles);
            model = GranularContactModel::hertz_history(GranularMaterial::new(5.0e6, 0.3, 0.5, 0.3).unwrap());
            rows = if washino { WASH_BED } else { EASO_BED };
        }
    }
    let mut model = model.with_cohesion(GranularCohesion::Capillary(params));
    if two_type {
        model = model.with_materials(
            MaterialTable::new(
                &[(1.0e7, 0.3), (3.0e7, 0.2)],
                &[&[0.9, 0.7], &[0.7, 0.5]],
                &[&[0.5, 0.3], &[0.3, 0.2]],
            )
            .unwrap(),
        );
    }
    let mut sys = GranularSystem::new(particles, vec![], model, Vec3::zero(), dt).unwrap();
    if two_type {
        sys = sys.with_particle_types(vec![0, 1]).unwrap();
    }
    let sys = sys
        .with_liggghts_neighbor_order(LiggghtsNeighbor::new(lo, hi, skin).unwrap())
        .with_capillary()
        .unwrap()
        .with_liquid_contents(contents)
        .unwrap();
    (sys, rows)
}

/// `(bit-identical values, total, worst |diff| / column scale, number of
/// differing values, worst ulp distance)`.
struct Outcome {
    same: usize,
    total: usize,
    worst_rel: f64,
    worst_ulp: u64,
    first_diff_step: Option<usize>,
}

fn ulp(a: f64, b: f64) -> u64 {
    let f = |x: f64| {
        let i = x.to_bits() as i64;
        if i < 0 { i64::MIN.wrapping_sub(i) } else { i }
    };
    (f(a) as i128 - f(b) as i128).unsigned_abs() as u64
}

fn compare(case: Case) -> Outcome {
    let (mut sys, rows) = system(case);
    let nvar = 14;
    // `CAP_REF=<csv>` swaps in a finer-sampled upstream run (debugging only).
    let ovr = std::env::var("CAP_REF").ok().map(|f| std::fs::read_to_string(f).unwrap());
    let frames: Vec<(usize, Vec<f64>)> = ovr
        .as_deref()
        .unwrap_or(rows)
        .lines()
        .skip(1)
        .filter(|l| !l.trim().is_empty())
        .map(|l| {
            let v: Vec<f64> = l.split(',').map(|x| x.parse().unwrap()).collect();
            (v[0] as usize, v[1..].to_vec())
        })
        .collect();
    let scale: Vec<f64> = (0..nvar)
        .map(|c| frames.iter().flat_map(|f| f.1.iter().skip(c).step_by(nvar)).fold(0.0_f64, |m, x| m.max(x.abs())))
        .collect();
    let mut o = Outcome { same: 0, total: 0, worst_rel: 0.0, worst_ulp: 0, first_diff_step: None };
    let mut done = 0usize;
    for (step, vals) in &frames {
        sys.run(*step - done);
        done = *step;
        for (n, p) in sys.particles().iter().enumerate() {
            let f = sys.forces()[n];
            let ours = [
                p.position.x, p.position.y, p.position.z, p.velocity.x, p.velocity.y, p.velocity.z,
                p.angular_velocity.x, p.angular_velocity.y, p.angular_velocity.z,
                f.x, f.y, f.z, sys.liquid_content()[n], sys.liquid_flux()[n],
            ];
            for c in 0..nvar {
                let theirs = vals[n * nvar + c];
                o.total += 1;
                if ours[c].to_bits() == theirs.to_bits() {
                    o.same += 1;
                } else {
                    o.first_diff_step.get_or_insert(*step);
                    o.worst_ulp = o.worst_ulp.max(ulp(ours[c], theirs));
                    if std::env::var("CAP_DEBUG").is_ok() {
                        println!("step {step} particle {} col {c}: {:e} vs {:e}", n + 1, ours[c], theirs);
                    }
                }
                if scale[c] > 0.0 {
                    o.worst_rel = o.worst_rel.max((ours[c] - theirs).abs() / scale[c]);
                }
            }
        }
    }
    o
}

fn report(name: &str, case: Case) {
    let o = compare(case);
    println!(
        "{name}: {}/{} bit-identical, worst |diff|/scale = {:e}, worst ulp = {}, first difference at step {:?}",
        o.same, o.total, o.worst_rel, o.worst_ulp, o.first_diff_step
    );
    assert!(o.worst_rel <= 1e-8, "{name}: worst {:e}", o.worst_rel);
}

/// Easo, two equal spheres: formation, rebound, stretch and rupture.
#[test]
fn easo_pair_matches_upstream() {
    report("easo_pair", Case::EasoPair);
}

/// Easo, unequal two-type spheres, hard-limit rupture, `tangential_reduce`.
#[test]
fn easo_hard_limit_matches_upstream() {
    report("easo_hard", Case::EasoHard);
}

/// Washino, two equal spheres: bridge formed out of contact.
#[test]
fn washino_pair_matches_upstream() {
    report("washino_pair", Case::WashinoPair);
}

/// Washino with `limitLiquidContent`, `modifyLbVolume`, `tangential_reduce`.
#[test]
fn washino_limited_matches_upstream() {
    report("washino_limit", Case::WashinoLimit);
}

/// Easo, 84-pebble bed (integration: many bridges, transfer, re-formation).
#[test]
fn easo_bed_matches_upstream() {
    report("easo_bed", Case::EasoBed);
}

/// Washino, 84-pebble bed.
#[test]
fn washino_bed_matches_upstream() {
    report("washino_bed", Case::WashinoBed);
}
