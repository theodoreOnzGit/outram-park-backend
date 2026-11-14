//! Translational periodic boundary conditions (GitHub #259): single-particle
//! track tests against analytic geometry, and the refusals by name.

use super::*;
use crate::geometry::cell::{Cell, HalfSpaceSense, RegionToken, SurfaceToken};
use crate::geometry::geometry::{Crossing, Geometry};
use crate::geometry::position::{Direction, Position};
use crate::geometry::surface::{BoundaryType, Sphere, SurfaceKind, XPlane, YPlane, ZPlane};
use crate::geometry::universe::Universe;

/// A [-1,1]^3 void box whose x faces are a translational periodic pair. With
/// `yz_periodic` the y and z faces are periodic pairs too; otherwise they are
/// reflective.
fn periodic_box(yz_periodic: bool) -> Geometry {
    let yz = |partner: usize| {
        if yz_periodic {
            BoundaryType::PeriodicWith(partner)
        } else {
            BoundaryType::Reflective
        }
    };
    let surfaces = vec![
        SurfaceKind::XPlane(XPlane { x0: -1.0, bc: BoundaryType::PeriodicWith(1) }),
        SurfaceKind::XPlane(XPlane { x0: 1.0, bc: BoundaryType::PeriodicWith(0) }),
        SurfaceKind::YPlane(YPlane { y0: -1.0, bc: yz(3) }),
        SurfaceKind::YPlane(YPlane { y0: 1.0, bc: yz(2) }),
        SurfaceKind::ZPlane(ZPlane { z0: -1.0, bc: yz(5) }),
        SurfaceKind::ZPlane(ZPlane { z0: 1.0, bc: yz(4) }),
    ];
    let mut region = Vec::new();
    for k in 0..3 {
        region.push(RegionToken::HalfSpace {
            surface_idx: 2 * k,
            sense: HalfSpaceSense::Outside,
        });
        region.push(RegionToken::HalfSpace {
            surface_idx: 2 * k + 1,
            sense: HalfSpaceSense::Inside,
        });
    }
    Geometry {
        surfaces,
        cells: vec![Cell::material(1, region, 0, 293.6)],
        universes: vec![Universe { id: 0, cell_indices: vec![0] }],
        lattices: vec![],
        root_universe: 0,
    }
}

/// The pair's translation is upstream's `u * (d2 - d1)`: exactly -2 x from
/// x = +1 to x = -1, and +2 x the other way.
#[test]
fn translation_is_the_plane_spacing() {
    let g = periodic_box(true);
    let t10 = translational_periodic_translation(&g, 1, 0).unwrap();
    let t01 = translational_periodic_translation(&g, 0, 1).unwrap();
    assert_eq!((t10.x, t10.y, t10.z), (-2.0, 0.0, 0.0));
    assert_eq!((t01.x, t01.y, t01.z), (2.0, 0.0, 0.0));
    assert!(g.validate_boundary_conditions().is_ok());
}

/// Single-particle track: hitting x = +1 puts the particle at x = -1 (plus
/// the nudge) with the direction unchanged bit for bit, on the inner side of
/// the partner, and `locate` finds it back in the box. Reflective would have
/// flipped `u.u`; that difference is what #259 is about.
#[test]
fn crossing_translates_and_keeps_direction() {
    let g = periodic_box(false);
    let u = Direction::new(0.6, 0.8, 0.0);
    let r = Position::new(1.0, 0.3, 0.2);
    let c = g.cross_surface(1, r, u, &mut 7_u64);
    assert!(c.alive);
    assert_eq!((c.u.u, c.u.v, c.u.w), (u.u, u.v, u.w));
    assert!((c.r.x + 1.0).abs() < 1e-8, "x = {}", c.r.x);
    assert!((c.r.y - 0.3).abs() < 1e-8 && (c.r.z - 0.2).abs() < 1e-8);
    assert_eq!(c.on_surface, SurfaceToken::on(0, HalfSpaceSense::Outside));
    let path = g.locate(c.r, c.u, c.on_surface).expect("relocated");
    assert_eq!(path.levels[0].cell, 0);
    // And back the other way.
    let back = Direction::new(-0.6, 0.8, 0.0);
    let c2 = g.cross_surface(0, Position::new(-1.0, 0.0, 0.0), back, &mut 7_u64);
    assert!((c2.r.x - 1.0).abs() < 1e-8);
    assert_eq!(c2.on_surface, SurfaceToken::on(1, HalfSpaceSense::Inside));
}

/// Analytic: free streaming in a fully periodic void box is a straight line
/// in the unfolded lattice. After a path length `s` through 40 crossings the
/// wrapped position equals `r0 + u s` folded into [-1, 1). Each crossing adds
/// two 1e-9 cm nudges, so the bound is 1e-6 cm.
#[test]
fn free_streaming_is_a_straight_line_in_the_unfolded_lattice() {
    let g = periodic_box(true);
    let r0 = Position::new(0.1, -0.2, 0.3);
    let (a, b, c) = (0.48_f64, 0.6_f64, 0.64_f64);
    let n = (a * a + b * b + c * c).sqrt();
    let u = Direction::new(a / n, b / n, c / n);
    let mut path = g.locate(r0, u, SurfaceToken::NONE).unwrap();
    let mut r = r0;
    let mut s = 0.0;
    let mut seed = 1_u64;
    for _ in 0..40 {
        let hit = g.distance_to_boundary(&path);
        let Crossing::Surface(i) = hit.crossing else {
            panic!("no surface ahead")
        };
        let d = hit.distance;
        r = Position::new(r.x + u.u * d, r.y + u.v * d, r.z + u.w * d);
        s += d;
        let crossed = g.cross_surface(i, r, u, &mut seed);
        assert!(crossed.alive);
        r = crossed.r;
        path = g
            .locate(r, u, crossed.on_surface)
            .expect("lost after a periodic crossing");
    }
    let fold = |x: f64| (x + 1.0).rem_euclid(2.0) - 1.0;
    let want = [fold(r0.x + u.u * s), fold(r0.y + u.v * s), fold(r0.z + u.w * s)];
    for (got, want) in [r.x, r.y, r.z].into_iter().zip(want) {
        let d = (got - want).abs();
        let d = d.min(2.0 - d);
        assert!(d < 1e-6, "wrapped {got} vs analytic {want} after s = {s}");
    }
}

/// Refused by name: a partner that does not name this surface back, a
/// non-parallel pair (upstream's ROTATIONAL case, not ported), a non-plane,
/// and a periodic surface with no partner.
#[test]
fn non_translational_pairs_are_refused() {
    let mut g = periodic_box(false);
    g.surfaces[1] = SurfaceKind::XPlane(XPlane { x0: 1.0, bc: BoundaryType::Reflective });
    assert!(g.validate_boundary_conditions().unwrap_err().contains("not PeriodicWith"));

    let mut g = periodic_box(false);
    g.surfaces[1] = SurfaceKind::YPlane(YPlane { y0: 1.0, bc: BoundaryType::PeriodicWith(0) });
    assert!(g.validate_boundary_conditions().unwrap_err().contains("ROTATIONAL"));

    let mut g = periodic_box(false);
    g.surfaces[1] = SurfaceKind::Sphere(Sphere {
        x0: 0.0,
        y0: 0.0,
        z0: 0.0,
        r: 1.0,
        bc: BoundaryType::PeriodicWith(0),
    });
    assert!(g.validate_boundary_conditions().unwrap_err().contains("Only planes"));

    let mut g = periodic_box(false);
    g.surfaces[0] = SurfaceKind::XPlane(XPlane { x0: -1.0, bc: BoundaryType::Periodic });
    g.surfaces[1] = SurfaceKind::XPlane(XPlane { x0: 1.0, bc: BoundaryType::Reflective });
    assert!(g.validate_boundary_conditions().unwrap_err().contains("no partner"));
}
