//! **`DeltaDomain::CylinderVacuum`** — `bn:op-867c.6`, gh #214.
//!
//! HTR-10's pebble bed is a cylinder with a reflector around it. Before this,
//! `DeltaDomain` offered a cube or a sphere, so a bed could only be
//! delta-tracked by bounding it with something the wrong shape.
//!
//! # Results (2026-09-17)
//!
//! All assertions below pass. The uniformity check measures its own agreement
//! and prints it.

use outram_mc_libs::geometry::position::{Direction, Position};
use outram_mc_libs::pebble_beds::keff_delta::DeltaDomain;

const R: f64 = 2.0;
const H: f64 = 3.0; // half-height

fn cyl() -> DeltaDomain {
    DeltaDomain::CylinderVacuum {
        radius: R,
        half_height: H,
    }
}

/// Containment must respect BOTH the curved wall and the end caps — a cylinder
/// is not a sphere and not a slab, and getting one of the two constraints
/// wrong is the obvious way to write this.
#[test]
fn containment_respects_the_wall_and_both_end_caps() {
    let d = cyl();
    assert!(d.contains(Position::new(0.0, 0.0, 0.0)), "centre is inside");
    assert!(
        d.contains(Position::new(R - 1e-9, 0.0, 0.0)),
        "just inside the wall"
    );
    assert!(
        d.contains(Position::new(0.0, 0.0, H - 1e-9)),
        "just inside the cap"
    );
    assert!(
        d.contains(Position::new(R, 0.0, H)),
        "the rim is closed, so inside"
    );

    assert!(
        !d.contains(Position::new(R + 1e-9, 0.0, 0.0)),
        "outside the wall"
    );
    assert!(
        !d.contains(Position::new(0.0, 0.0, H + 1e-9)),
        "beyond the cap"
    );
    // The trap: a point beyond the cap but within the radius, and a point
    // within the height but beyond the radius. Each catches one dropped test.
    assert!(
        !d.contains(Position::new(0.0, 0.0, 10.0)),
        "within R, past the cap"
    );
    assert!(
        !d.contains(Position::new(10.0, 0.0, 0.0)),
        "within H, past the wall"
    );
    // Diagonal: inside a bounding SPHERE of radius max(R,H), outside the cylinder.
    assert!(
        !d.contains(Position::new(R * 0.9, R * 0.9, 0.0)),
        "corner of the box is out"
    );
}

/// `sample_point` must fill the cylinder uniformly. A naive `r = R * xi` (no
/// square root) concentrates points near the axis — the classic disc-sampling
/// error, and invisible unless the radial profile is checked.
#[test]
fn sampled_points_fill_the_cylinder_uniformly() {
    let d = cyl();
    let mut seed = 12345_u64;
    const N: usize = 200_000;

    // Equal-AREA radial shells: for a uniform disc each shell should get the
    // same count, which is exactly what a missing sqrt breaks.
    const SHELLS: usize = 5;
    let mut shell = [0usize; SHELLS];
    let mut zsum = 0.0_f64;
    for _ in 0..N {
        let p = d.sample_point(&mut seed);
        assert!(d.contains(p), "every sampled point must be inside: {p:?}");
        let rr = (p.x * p.x + p.y * p.y) / (R * R); // in [0,1], area-proportional
        shell[((rr * SHELLS as f64) as usize).min(SHELLS - 1)] += 1;
        zsum += p.z;
    }

    let expect = N as f64 / SHELLS as f64;
    let mut worst = 0.0_f64;
    for (i, &c) in shell.iter().enumerate() {
        let dev = (c as f64 - expect).abs() / expect.sqrt(); // sigma, Poisson
        println!("  shell {i}: {c} (expected {expect:.0}, {dev:.2} sigma)");
        worst = worst.max(dev);
    }
    println!(
        "worst equal-area shell deviation {worst:.2} sigma; mean z = {:.4}",
        zsum / N as f64
    );

    assert!(
        worst < 5.0,
        "equal-AREA shells must receive equal counts; worst {worst:.2} sigma. A \
         large deviation in the innermost shell is the missing-sqrt signature, \
         which concentrates points on the axis."
    );
    assert!(
        (zsum / N as f64).abs() < 0.05 * H,
        "z must be uniform about the mid-plane, got mean {:.4}",
        zsum / N as f64
    );
}

/// The vacuum arm advances in a straight line and leaves the direction alone —
/// a landing point outside is the escape signal, not something to reflect.
#[test]
fn advance_is_ballistic_and_may_land_outside() {
    let d = cyl();
    let u = Direction::new(1.0, 0.0, 0.0);
    let (p, u2) = d.advance(Position::ZERO, u, 10.0);
    assert!((p.x - 10.0).abs() < 1.0e-12, "straight line, no reflection");
    assert_eq!(u2.u, u.u, "direction unchanged");
    assert!(!d.contains(p), "landing outside is how escape is signalled");

    // And out through an END CAP, not just the wall — the axis direction is the
    // one a sphere-derived implementation would get wrong.
    let w = Direction::new(0.0, 0.0, 1.0);
    let (q, _) = d.advance(Position::ZERO, w, H + 1.0);
    assert!(!d.contains(q), "must escape through the cap too");
}

/// `bounding_half` must cover the whole body on every axis, or a caller using
/// it to size a box would clip the cylinder.
#[test]
fn bounding_half_covers_both_extents() {
    assert_eq!(cyl().bounding_half(), H, "half-height is the larger here");
    let squat = DeltaDomain::CylinderVacuum {
        radius: 5.0,
        half_height: 1.0,
    };
    assert_eq!(squat.bounding_half(), 5.0, "radius is the larger there");
}
