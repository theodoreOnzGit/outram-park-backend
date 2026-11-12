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
//! ## Result (2026-11-11, release, rustc 1.99.1) - PREDICTION MET AFTER ONE FIX
//! 1674/1674 values bit-identical over all 600 steps (31 frames x 6 particles
//! x 9 columns), worst |diff|/scale = 0. The control without the model misses
//! by 1.07e-1 (so the model is exercised).
//!
//! First run missed (1.76e-3 from step 2). Cause, found by instrumenting the
//! compiled upstream (prints of the contact-history slots, surfPos and delta
//! per contact; the port's `HalfSpaceFix` inputs and outputs were bit-equal at
//! steps 1 and 2): `ContactKinematics::pair_radii` computed `R* = radi*radj /
//! radsum`, but Hertz (`normal_model_hertz.h` l. 211) computes `R* = radi*radj /
//! (radi+radj)` from `sidata.radi` (UNEXPANDED) and `sidata.radj` (EXPANDED);
//! `radsum` (both expanded) differs under multicontact. Fixed in
//! `granular.rs` (identical for every other model, where radsum = radi+radj).
//! Also copied: upstream sets `sidata.radi` once per particle UNEXPANDED while
//! `sidata.radj` and `radsum` are expanded (`pair_gran_base.h` l. 271-311).
//! Mesh walls and the cylinder close history are covered by the two later tests
//! (~~mesh walls not hooked~~ **CORRECTED 2026-11-11**, gh:#1410).

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
const INIT_CYL: &str = include_str!("../../../reference-data/liggghts/multicontact_cyl_init.csv");
const REF_CYL: &str = include_str!("../../../reference-data/liggghts/multicontact_cyl.csv");
const REF_MESH: &str = include_str!("../../../reference-data/liggghts/multicontact_mesh.csv");
const STL_FLOOR: &str = include_str!("../../../reference-data/liggghts/plate_square.stl");

/// Which deck: two z planes, or two planes plus a `zcylinder 0.0125 0 0` wall.
#[derive(Clone, Copy)]
enum Deck {
    Planes,
    Cylinder,
    /// The `Planes` bed with the lower plane replaced by a 2-triangle STL floor.
    Mesh,
}

fn system(deck: Deck, multicontact: bool) -> GranularSystem {
    let r = 0.005;
    let mass = 4.0 * std::f64::consts::PI / 3.0 * r * r * r * 2500.0;
    let init = match deck {
        Deck::Planes | Deck::Mesh => INIT,
        Deck::Cylinder => INIT_CYL,
    };
    let particles: Vec<Particle> = init
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
    let mut boundaries = vec![
        Boundary::plane(Vec3::zero(), Vec3::new(0.0, 0.0, 1.0)).unwrap(),
        Boundary::plane(Vec3::new(0.0, 0.0, 0.0195), Vec3::new(0.0, 0.0, 1.0)).unwrap(),
    ];
    if matches!(deck, Deck::Mesh) {
        boundaries.remove(0);
    }
    if matches!(deck, Deck::Cylinder) {
        boundaries.push(Boundary::cylinder(Vec3::zero(), Vec3::new(0.0, 0.0, 1.0), 0.0125).unwrap());
    }
    let sys = GranularSystem::new(
        particles,
        boundaries,
        GranularContactModel::hertz_history(material),
        Vec3::zero(),
        2e-6,
    )
    .unwrap();
    let cfg = LiggghtsNeighbor::new(Vec3::new(-1.0, -1.0, -0.005), Vec3::new(1.0, 1.0, 0.03), 0.002).unwrap();
    let mut sys = sys.with_liggghts_neighbor_order(cfg);
    if matches!(deck, Deck::Mesh) {
        let mesh = outram_park_fork_liggghts::mesh_wall::MeshWall::from_ascii_stl(STL_FLOOR).unwrap();
        sys = sys.with_moving_walls(vec![outram_park_fork_liggghts::mesh_wall::MovingBoundary::new(
            outram_park_fork_liggghts::mesh_wall::WallGeometry::Mesh(mesh),
            Vec3::zero(),
            Vec3::zero(),
            Vec3::zero(),
        )]);
    }
    if multicontact {
        sys.with_multicontact(HalfSpaceFix::new(vec![1.0e5], vec![0.3])).unwrap()
    } else {
        sys
    }
}

/// Run and return `(bit-identical values, total values, worst |diff| / column scale)`.
fn compare(deck: Deck, multicontact: bool) -> (usize, usize, f64) {
    let mut sys = system(deck, multicontact);
    let rows = match deck {
        Deck::Planes => REF,
        Deck::Cylinder => REF_CYL,
        Deck::Mesh => REF_MESH,
    };
    let ovr = std::env::var("MC_REF").ok().map(|f| std::fs::read_to_string(f).unwrap());
    let mut rows = ovr.as_deref().unwrap_or(rows).lines();
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
            let ours = [
                p.position.x, p.position.y, p.position.z, p.velocity.x, p.velocity.y, p.velocity.z,
                p.angular_velocity.x, p.angular_velocity.y, p.angular_velocity.z,
            ];
            for c in 0..nvar {
                let theirs = vals[n * nvar + c];
                total += 1;
                if ours[c].to_bits() == theirs.to_bits() {
                    same += 1;
                } else if std::env::var("MC_DEBUG").is_ok() {
                    println!("step {step} particle {} col {c}: {:e} vs {:e}", n + 1, ours[c], theirs);
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
    let (same, total, worst) = compare(Deck::Planes, true);
    println!("multicontact: {same}/{total} values bit-identical, worst |diff|/scale = {worst:e}");
    assert!(worst <= 1e-8, "worst {worst:e}");
}

/// Cylinder wall (gh:#1410): `in.multicontact_cyl`, six spheres between two z
/// planes and a `zcylinder` of radius 12.5 mm; some overlap the cylinder, some
/// sit within one radius of it (so the cylinder's `surfacesClose` history,
/// `delta*radi*rinv` with the cylinder's `delta`, drives the half-space fix).
/// Prediction (written before the run): bit-identical, as the plane deck;
/// without the cylinder `surfacesClose` hook the expansions at gap contacts
/// would be missed.
///
/// Result (2026-11-11, release, rustc 1.99.1): 1674/1674 values bit-identical,
/// worst difference 0; control without the model misses by 3.3e-1; ablating the
/// cylinder `surfacesClose` hook drops it to 972/1674 (worst 1.7e-1), so the
/// hook carries the agreement. Two causes were found by instrumenting a copy of
/// upstream (prints of the contact-property-atom data per step): (1) the port
/// wrote the close history for every particle with `deltan < radius`, upstream
/// only for those in the wall's neighbour list (`Cylinder::resolveNeighlist`,
/// rebuilt with the skin) - now `Boundary::upstream_neighlist`, kept per
/// rebuild; (2) the cylinder `delta` is defined for `deltan > 0` too
/// (`Boundary::cylinder_resolve`). Upstream oddity, NOT copied: a particle
/// that gets a close history but is farther than half a skin from the wall
/// reads another particle's surface position (zero-size page allocation in
/// `FixContactPropertyAtomWall::clear`), so the deck keeps every close
/// particle within 1 mm of the cylinder or beyond one radius.
#[test]
fn multicontact_cylinder_matches_upstream() {
    let (same, total, worst) = compare(Deck::Cylinder, true);
    println!("multicontact cylinder: {same}/{total} values bit-identical, worst = {worst:e}");
    let (_, _, without) = compare(Deck::Cylinder, false);
    println!("cylinder control without the model: {without:e}");
    assert!(without > 10.0 * worst.max(1e-12), "model not exercised: with {worst:e} without {without:e}");
    assert!(worst <= 1e-8, "worst {worst:e}");
}

/// Mesh-wall multicontact (gh:#1410): `in.multicontact_mesh`, the plane deck
/// with its lower plane replaced by `plate_square.stl` (2 facets, diagonal
/// through the origin). Exercises the per-facet history (key tag, wall, tri,
/// slot order = `mesh_partners`), the `deltaData[3]` radius expansion before
/// `resolveTriSphereContactBary`, `is_mesh` in `pre_force`, and the mesh
/// `surfacesClose` history. Prediction (written before the run): bit-identical
/// as the plane deck.
///
/// Result (2026-11-11, release, rustc 1.99.1): 1674/1674 values bit-identical,
/// worst difference 0; control without the model misses by 1.07e-1. Ablation
/// (mesh `deltaData[3]` radius expansion removed): 54/1674 bit-identical, worst
/// 1.5e-3, so the mesh hook carries the agreement.
#[test]
fn multicontact_mesh_matches_upstream() {
    let (same, total, worst) = compare(Deck::Mesh, true);
    println!("multicontact mesh: {same}/{total} values bit-identical, worst = {worst:e}");
    let (_, _, without) = compare(Deck::Mesh, false);
    println!("mesh control without the model: {without:e}");
    assert!(without > 10.0 * worst.max(1e-12), "model not exercised: with {worst:e} without {without:e}");
    assert!(worst <= 1e-8, "worst {worst:e}");
}

/// The model is exercised: a run without it misses upstream by far more.
#[test]
fn control_without_multicontact_differs() {
    let (_, _, with) = compare(Deck::Planes, true);
    let (_, _, without) = compare(Deck::Planes, false);
    println!("control: with {with:e}, without {without:e}");
    // measured 0 vs 1.07e-1
    assert!(without > 10.0 * with, "with {with:e} without {without:e}");
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
