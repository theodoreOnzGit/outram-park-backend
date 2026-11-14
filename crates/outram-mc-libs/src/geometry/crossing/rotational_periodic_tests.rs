//! Rotational periodic boundary conditions (GitHub #259): single-particle
//! track tests against analytic geometry. Port of OpenMC's
//! `RotationalPeriodicBC` (`src/boundary_condition.cpp:152-262`, `8f79d9d`).
//!
//! Methodology: a void wedge about the z axis, bounded by two planes through
//! the origin that are a periodic pair, a z cylinder of radius 100 cm
//! (vacuum) and two reflective z planes. The reference is plane geometry
//! only: the rotation angle is the wedge angle (exact up to `atan2`
//! round-off), a rotated position keeps its radius and z, a rotated direction
//! keeps its unit length and z component, and a straight line of the unfolded
//! plane, folded into the wedge, keeps `|r|`, the folded polar angle and
//! `u . r`. Tolerances: 1e-15 for one rotation, 1e-6 cm after several
//! crossings (each adds two 1e-9 cm nudges).
//! Result (2026-11-14, release): all pass; see the issue comment for numbers.

use super::*;
use petir::mathf::RealMath;
use crate::geometry::cell::{Cell, HalfSpaceSense, RegionToken, SurfaceToken};
use crate::geometry::geometry::{Crossing, Geometry};
use crate::geometry::position::{Direction, Position};
use crate::geometry::surface::{
    BoundaryType, Plane, SurfaceKind, XPlane, YPlane, ZCylinder, ZPlane,
};
use crate::geometry::universe::Universe;

/// Wedge of opening angle `theta_deg` (60, 90, 30, ...) between the polar
/// angles 0 and `theta`. Surface 0 is the face at polar angle 0 (`y = 0`),
/// surface 1 the face at `theta`; interior is `evaluate > 0` on both.
fn wedge(theta_deg: f64) -> Geometry {
    let th = theta_deg.to_radians();
    let face0 = SurfaceKind::YPlane(YPlane { y0: 0.0, bc: BoundaryType::PeriodicWith(1) });
    let face1 = if theta_deg == 90.0 {
        SurfaceKind::XPlane(XPlane { x0: 0.0, bc: BoundaryType::PeriodicWith(0) })
    } else {
        // Line through the origin at polar angle `th`; normal (sin, -cos, 0)
        // is positive on the side of the angle-0 face.
        SurfaceKind::Plane(Plane {
            a: th.r_sin(),
            b: -th.r_cos(),
            c: 0.0,
            d: 0.0,
            bc: BoundaryType::PeriodicWith(0),
        })
    };
    let surfaces = vec![
        face0,
        face1,
        SurfaceKind::ZCylinder(ZCylinder { x0: 0.0, y0: 0.0, r: 100.0, bc: BoundaryType::Vacuum }),
        SurfaceKind::ZPlane(ZPlane { z0: -1.0, bc: BoundaryType::Reflective }),
        SurfaceKind::ZPlane(ZPlane { z0: 1.0, bc: BoundaryType::Reflective }),
    ];
    let hs = |surface_idx, sense| RegionToken::HalfSpace { surface_idx, sense };
    // RPN: an `Intersection` after every token past the first, otherwise
    // `Cell::contains` tests only the last token.
    let mut region = vec![hs(0, HalfSpaceSense::Outside)];
    for t in [
        hs(1, HalfSpaceSense::Outside),
        hs(2, HalfSpaceSense::Inside),
        hs(3, HalfSpaceSense::Outside),
        hs(4, HalfSpaceSense::Inside),
    ] {
        region.push(t);
        region.push(RegionToken::Intersection);
    }
    Geometry {
        surfaces,
        cells: vec![Cell::material(1, region, 0, 293.6)],
        universes: vec![Universe { id: 0, cell_indices: vec![0] }],
        lattices: vec![],
        root_universe: 0,
    }
}

fn unit(a: f64, b: f64, c: f64) -> Direction {
    let n = (a * a + b * b + c * c).sqrt();
    Direction::new(a / n, b / n, c / n)
}

fn angle_of(g: &Geometry, i: usize, j: usize) -> f64 {
    match periodic_transform(g, i, j).unwrap() {
        PeriodicTransform::Rotation { axis, angle } => {
            assert_eq!(axis, 2, "wedge about z");
            angle
        }
        other => panic!("expected a rotation, got {other:?}"),
    }
}

/// The rotation angle is the wedge angle, +theta from face 0 to face 1 and
/// -theta back, about z, to within `atan2` round-off.
#[test]
fn rotation_angle_is_the_wedge_angle() {
    for theta in [90.0_f64, 60.0, 30.0, 45.0] {
        let g = wedge(theta);
        assert!(g.validate_boundary_conditions().is_ok(), "{theta}");
        let want = theta.to_radians();
        let fwd = angle_of(&g, 0, 1);
        let back = angle_of(&g, 1, 0);
        assert!((fwd - want).abs() < 4e-16, "{theta}: {fwd} vs {want}");
        assert!((back + want).abs() < 4e-16, "{theta}: {back} vs {}", -want);
    }
}

/// One crossing of the y = 0 face of the 90 degree wedge. By hand: the
/// position (x, 0, z) goes to (0, x, z) and the direction (a, b, c) to
/// (-b, a, c); the particle ends on the interior side of x = 0, in the cell.
#[test]
fn quarter_wedge_crossing_matches_the_hand_rotation() {
    let g = wedge(90.0);
    let u = unit(0.3, -0.9, 0.1);
    let r = Position::new(1.5, 0.0, 0.2);
    let c = g.cross_surface(0, r, u, &mut 7_u64);
    assert!(c.alive);
    assert!(c.r.x.abs() < 1e-8 && c.r.x > 0.0, "x = {}", c.r.x);
    assert!((c.r.y - 1.5).abs() < 1e-8 && (c.r.z - 0.2).abs() < 1e-8);
    assert!((c.u.u - (-u.v)).abs() < 1e-15 && (c.u.v - u.u).abs() < 1e-15);
    assert_eq!(c.u.w, u.w, "the axial component is untouched, bit for bit");
    assert!((c.u.u * c.u.u + c.u.v * c.u.v + c.u.w * c.u.w - 1.0).abs() < 1e-15);
    assert_eq!(c.on_surface, SurfaceToken::on(1, HalfSpaceSense::Outside));
    let path = g.locate(c.r, c.u, c.on_surface).expect("relocated into the wedge");
    assert_eq!(path.levels[0].cell, 0);
}

/// The 60 degree wedge uses a general plane for face 1. Rotation by exactly
/// the wedge angle: radius and z kept, polar angle moved from 0 to 60 degrees,
/// the direction rotated by the same angle, and the particle is inside.
#[test]
fn sixty_degree_crossing_rotates_position_and_direction_together() {
    let g = wedge(60.0);
    let u = unit(0.2, -0.7, -0.3);
    let r = Position::new(2.5, 0.0, -0.4);
    let c = g.cross_surface(0, r, u, &mut 7_u64);
    let th = 60.0_f64.to_radians();
    let rho = (c.r.x * c.r.x + c.r.y * c.r.y).sqrt();
    assert!((rho - 2.5).abs() < 1e-8, "radius {rho}");
    assert!((c.r.y.r_atan2(c.r.x) - th).abs() < 1e-8, "polar angle");
    let (s, co) = (th.r_sin(), th.r_cos());
    assert!((c.u.u - (co * u.u - s * u.v)).abs() < 1e-15);
    assert!((c.u.v - (s * u.u + co * u.v)).abs() < 1e-15);
    assert_eq!(c.u.w, u.w);
    assert_eq!(c.on_surface, SurfaceToken::on(1, HalfSpaceSense::Outside));
    let path = g.locate(c.r, c.u, c.on_surface).expect("relocated into the wedge");
    assert_eq!(path.levels[0].cell, 0);
    // The entering particle moves into the wedge: inward normal is (sin, -cos).
    assert!(c.u.u * s - c.u.v * co > 0.0);
}

/// Out through face 0 and back through face 1 is the identity on the
/// particle, to the nudges.
#[test]
fn there_and_back_is_the_identity() {
    for theta in [90.0, 60.0, 30.0] {
        let g = wedge(theta);
        let u = unit(0.25, -0.8, 0.2);
        let r = Position::new(3.0, 0.0, 0.1);
        let a = g.cross_surface(0, r, u, &mut 1_u64);
        // Leave through face 1 along the direction we arrived with: that is
        // the reverse of entering, so reflect the direction and cross back.
        let rev = Direction::new(-a.u.u, -a.u.v, -a.u.w);
        let on1 = Position::new(
            3.0 * theta.to_radians().r_cos(),
            3.0 * theta.to_radians().r_sin(),
            0.1,
        );
        let b = g.cross_surface(1, on1, rev, &mut 1_u64);
        let back = Direction::new(-b.u.u, -b.u.v, -b.u.w);
        assert!((back.u - u.u).abs() < 1e-15, "{theta}");
        assert!((back.v - u.v).abs() < 1e-15, "{theta}");
        assert!((back.w - u.w).abs() < 1e-15, "{theta}: axial component, renormalised twice");
        // `b.r` is the reverse crossing of the arrival point; its radius and z
        // are those of the start.
        let rho = (b.r.x * b.r.x + b.r.y * b.r.y).sqrt();
        assert!((rho - 3.0).abs() < 1e-8 && (b.r.z - 0.1).abs() < 1e-8, "{theta}");
        assert!(b.r.y.abs() < 1e-8, "back on the y = 0 face");
    }
}

/// Repeated crossings stay consistent. A straight line of the unfolded plane,
/// folded into the wedge by the successive rotations, must keep `|r|`, keep
/// the folded polar angle (the unfolded one modulo the wedge angle) and keep
/// `u . r`, which is the radial rate and is invariant under a common rotation
/// of position and direction. A direction rotated differently from the
/// position would break the last one while leaving the first two intact.
#[test]
fn folded_straight_lines_stay_consistent_over_many_crossings() {
    for theta_deg in [90.0_f64, 60.0, 30.0] {
        let g = wedge(theta_deg);
        let th = theta_deg.to_radians();
        let mut seed = 12345_u64;
        let mut crossings = 0_usize;
        for _ in 0..200 {
            let rho0 = 0.5 + 4.5 * crate::rng::lcg::prn(&mut seed);
            let phi0 = th * (0.05 + 0.9 * crate::rng::lcg::prn(&mut seed));
            let r0 = Position::new(rho0 * phi0.r_cos(), rho0 * phi0.r_sin(), 0.0);
            let az = std::f64::consts::TAU * crate::rng::lcg::prn(&mut seed);
            let mu = 0.4 * (2.0 * crate::rng::lcg::prn(&mut seed) - 1.0);
            let sin_t = (1.0 - mu * mu).sqrt();
            let u0 = Direction::new(sin_t * az.r_cos(), sin_t * az.r_sin(), mu);
            let mut r = r0;
            let mut u = u0;
            let mut path = g.locate(r, u, SurfaceToken::NONE).unwrap();
            let mut s = 0.0;
            for _ in 0..40 {
                let hit = g.distance_to_boundary(&path);
                let Crossing::Surface(i) = hit.crossing else { panic!("no surface ahead") };
                // Stay inside the 100 cm vacuum cylinder.
                if i == 2 || hit.distance > 50.0 {
                    break;
                }
                r = Position::new(r.x + u.u * hit.distance, r.y + u.v * hit.distance, r.z + u.w * hit.distance);
                s += hit.distance;
                let crossed = g.cross_surface(i, r, u, &mut seed);
                assert!(crossed.alive);
                if i <= 1 {
                    crossings += 1;
                }
                r = crossed.r;
                u = crossed.u;
                path = g.locate(r, u, crossed.on_surface).expect("lost after a crossing");
                // Unfolded reference.
                let big = Position::new(r0.x + u0.u * s, r0.y + u0.v * s, r0.z + u0.w * s);
                if i > 1 {
                    // A z-plane reflection flips the axial component in the
                    // folded run only; the invariants below are radial and
                    // azimuthal, so rebuild the reference from the folded z.
                    break;
                }
                let rho_big = (big.x * big.x + big.y * big.y).sqrt();
                let rho = (r.x * r.x + r.y * r.y).sqrt();
                assert!((rho - rho_big).abs() < 1e-6, "{theta_deg}: |r| {rho} vs {rho_big}");
                let fold = big.y.r_atan2(big.x).rem_euclid(th);
                let got = r.y.r_atan2(r.x).rem_euclid(th);
                let dphi = (fold - got).abs().min(th - (fold - got).abs());
                assert!(dphi * rho < 1e-6, "{theta_deg}: polar angle {got} vs folded {fold}");
                let radial = u.u * r.x + u.v * r.y;
                let radial_big = u0.u * big.x + u0.v * big.y;
                assert!((radial - radial_big).abs() < 1e-6, "{theta_deg}: u.r {radial} vs {radial_big}");
                assert!((u.u * u.u + u.v * u.v + u.w * u.w - 1.0).abs() < 1e-14);
                assert!((u.w - u0.w).abs() < 1e-14, "axial component preserved");
            }
        }
        assert!(crossings > 100, "{theta_deg}: only {crossings} periodic crossings exercised");
    }
}

/// Everything not ported or not well-formed is refused by name.
#[test]
fn malformed_rotational_pairs_are_refused() {
    // Angle that does not tile the circle: 70 degrees.
    let g = wedge(70.0);
    let e = g.validate_boundary_conditions().unwrap_err();
    assert!(e.contains("evenly divide 360"), "{e}");

    // A plane that misses the origin: face 1 shifted off it.
    let mut g = wedge(60.0);
    if let SurfaceKind::Plane(p) = &mut g.surfaces[1] {
        p.d = 0.5;
    }
    let e = g.validate_boundary_conditions().unwrap_err();
    assert!(e.contains("does not intersect the origin"), "{e}");

    // Normals with no common zero component.
    let mut g = wedge(60.0);
    g.surfaces[1] = SurfaceKind::Plane(Plane {
        a: 0.5,
        b: -0.5,
        c: 0.7,
        d: 0.0,
        bc: BoundaryType::PeriodicWith(0),
    });
    let e = g.validate_boundary_conditions().unwrap_err();
    assert!(e.contains("zero"), "{e}");

    // A pair bounding no cell has no orientation.
    let mut g = wedge(60.0);
    g.cells[0].region = vec![RegionToken::HalfSpace {
        surface_idx: 2,
        sense: HalfSpaceSense::Inside,
    }];
    let e = g.validate_boundary_conditions().unwrap_err();
    assert!(e.contains("must both bound a cell"), "{e}");

    // The translational-only helper names the rotation.
    let g = wedge(90.0);
    assert!(translational_periodic_translation(&g, 0, 1).unwrap_err().contains("ROTATIONAL"));
}
