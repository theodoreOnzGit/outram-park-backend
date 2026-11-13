//! Unit tests of the composite-surface plumbing: errors, re-indexing,
//! boundary conditions, enum dispatch. Membership against OpenMC, volumes
//! and drawings are in `tests/composite_surfaces_vs_openmc.rs`.

use super::*;
use crate::geometry::cell::HalfSpaceSense::{Inside, Outside};

fn p(x: f64, y: f64, z: f64) -> Position {
    Position::new(x, y, z)
}

#[test]
fn upstream_value_errors_are_refused() {
    assert!(CylinderSector::new(2.0, 1.0, 0.0, 90.0, [0.0, 0.0], Axis::Z).is_err());
    assert!(CylinderSector::new(1.0, 2.0, 90.0, 90.0, [0.0, 0.0], Axis::Z).is_err());
    assert!(CylinderSector::from_theta_alpha(1.0, 2.0, 360.0, 0.0, [0.0, 0.0], Axis::Z).is_err());
    assert!(IsogonalOctagon::new([0.0, 0.0], 1.0, 2.0, Axis::Z).is_err());
    assert!(IsogonalOctagon::new([0.0, 0.0], 2.0, 1.0, Axis::Z).is_err());
    assert!(ConeOneSided::new(Axis::Z, [0.0; 3], 0.0, true).is_err());
    assert!(CruciformPrism::new(&[1.0, 3.0, 2.0], [0.0, 0.0], Axis::Z).is_err());
    assert!(CruciformPrism::new(&[1.0, -2.0], [0.0, 0.0], Axis::Z).is_err());
    assert!(Vessel::new(1.0, 2.0, 2.0, 1.0, 1.0, [0.0, 0.0], Axis::Z).is_err());
    assert!(ConicalFrustum::new([0.0; 3], [0.0; 3], 1.0, 2.0).is_err());
    assert!(Polygon::new(&[[0.0, 0.0], [1.0, 0.0]], PolygonBasis::Xy).is_err());
    assert!(Polygon::new(
        &[[0.0, 0.0], [1.0, 0.0], [1.0, 0.0], [0.0, 1.0]],
        PolygonBasis::Xy
    )
    .is_err());
    // An rz polygon with an edge on the axis: upstream's ZCylinder(r=0) refuses it.
    let axis_edge = [[0.0, 0.0], [2.0, 0.0], [1.0, 2.0], [0.0, 2.0]];
    assert!(Polygon::new(&axis_edge, PolygonBasis::Rz).is_err());
    // A bow tie crosses itself.
    let bow = [[0.0, 0.0], [2.0, 2.0], [2.0, 0.0], [0.0, 2.0]];
    let e = Polygon::new(&bow, PolygonBasis::Xy).unwrap_err();
    assert!(e.to_string().contains("self-intersecting"), "{e}");
}

#[test]
fn from_theta_alpha_is_theta1_alpha_theta2_alpha_plus_theta() {
    let a = CylinderSector::from_theta_alpha(1.0, 2.0, 60.0, 15.0, [0.0, 0.0], Axis::Z).unwrap();
    let b = CylinderSector::new(1.0, 2.0, 15.0, 75.0, [0.0, 0.0], Axis::Z).unwrap();
    assert_eq!(
        format!("{:?}", a.region().surfaces),
        format!("{:?}", b.region().surfaces)
    );
}

#[test]
fn append_to_reindexes_into_a_global_array() {
    let r = RectangularPrism::new(2.0, 2.0, Axis::Z, [0.0, 0.0], 0.0).unwrap();
    let mut global = vec![SurfaceKind::Sphere(crate::geometry::surface::Sphere {
        x0: 0.0,
        y0: 0.0,
        z0: 0.0,
        r: 9.0,
        bc: T,
    })];
    let (inside, outside) = r.region().append_to(&mut global);
    assert_eq!(global.len(), 5);
    let cell = Cell::material(0, inside, 0, 293.6);
    let u = Direction::new(1.0, 0.0, 0.0);
    assert!(cell.contains(p(0.5, 0.5, 0.0), u, &global, SurfaceToken::NONE));
    assert!(!cell.contains(p(1.5, 0.5, 0.0), u, &global, SurfaceToken::NONE));
    let out = Cell::material(1, outside, 0, 293.6);
    assert!(out.contains(p(1.5, 0.5, 0.0), u, &global, SurfaceToken::NONE));
    // Every token points past the sphere at index 0.
    assert!(out.region.iter().all(|t| match t {
        RegionToken::HalfSpace { surface_idx, .. } => *surface_idx >= 1,
        _ => true,
    }));
}

#[test]
fn boundary_skips_the_disambiguation_planes_and_refuses_periodic() {
    let v = Vessel::new(1.0, 0.0, 2.0, 0.5, 0.5, [0.0, 0.0], Axis::Z).unwrap();
    let r = v
        .region()
        .clone()
        .with_boundary(BoundaryType::Vacuum)
        .unwrap();
    let bcs: Vec<BoundaryType> = r.surfaces.iter().map(SurfaceKind::bc).collect();
    use BoundaryType::{Transmissive as Tr, Vacuum as Va};
    assert_eq!(bcs, vec![Va, Tr, Tr, Va, Va]);
    let c = ConeOneSided::new(Axis::X, [0.0; 3], 1.0, true).unwrap();
    let r = c
        .region()
        .clone()
        .with_boundary(BoundaryType::Reflective)
        .unwrap();
    assert_eq!(r.surfaces[0].bc(), BoundaryType::Reflective);
    assert_eq!(r.surfaces[1].bc(), BoundaryType::Transmissive);
    assert!(v
        .region()
        .clone()
        .with_boundary(BoundaryType::Periodic)
        .is_err());
    // Every surface kind a composite can produce takes a boundary.
    for mut s in r.surfaces.clone() {
        set_bc(&mut s, BoundaryType::White);
        assert_eq!(s.bc(), BoundaryType::White);
    }
}

#[test]
fn octagon_positive_is_upstreams_union_not_a_complement() {
    for axis in [Axis::X, Axis::Y, Axis::Z] {
        let o = IsogonalOctagon::new([0.0, 0.0], 1.0, 1.2, axis).unwrap();
        assert!(!o.region().positive.contains(&RegionToken::Complement));
        // Inside and outside partition space.
        for q in [p(0.1, 0.2, 0.3), p(0.95, 0.95, 0.95), p(2.0, -0.1, 0.4)] {
            assert_ne!(
                o.region().contains(q, Inside),
                o.region().contains(q, Outside)
            );
        }
    }
}

#[test]
fn composite_enum_dispatches_to_every_variant() {
    let all = vec![
        Composite::CylinderSector(
            CylinderSector::new(1.0, 2.0, 0.0, 90.0, [0.0, 0.0], Axis::Z).unwrap(),
        ),
        Composite::IsogonalOctagon(IsogonalOctagon::new([0.0, 0.0], 1.0, 1.2, Axis::Z).unwrap()),
        Composite::ConeOneSided(ConeOneSided::new(Axis::Z, [0.0; 3], 1.0, true).unwrap()),
        Composite::Polygon(
            Polygon::new(&[[0.0, 0.0], [1.0, 0.0], [0.0, 1.0]], PolygonBasis::Xy).unwrap(),
        ),
        Composite::CruciformPrism(CruciformPrism::new(&[1.0], [0.0, 0.0], Axis::Z).unwrap()),
        Composite::RectangularPrism(
            RectangularPrism::new(1.0, 1.0, Axis::Z, [0.0, 0.0], 0.1).unwrap(),
        ),
        Composite::ConicalFrustum(
            ConicalFrustum::new([0.0; 3], [0.0, 0.0, 1.0], 1.0, 0.5).unwrap(),
        ),
        Composite::Vessel(Vessel::new(1.0, 0.0, 1.0, 0.5, 0.5, [0.0, 0.0], Axis::Z).unwrap()),
    ];
    let counts: Vec<usize> = all.iter().map(|c| c.region().surfaces.len()).collect();
    assert_eq!(counts, vec![4, 8, 2, 3, 4, 12, 3, 5]);
    let geom = all[7].region().two_cell_geometry();
    assert_eq!(geom.cells.len(), 2);
}

#[test]
fn concave_polygon_splits_into_more_than_one_convex_piece() {
    // An L shape: no single convex piece covers it.
    let l = [
        [0.0, 0.0],
        [2.0, 0.0],
        [2.0, 1.0],
        [1.0, 1.0],
        [1.0, 2.0],
        [0.0, 2.0],
    ];
    let poly = Polygon::new(&l, PolygonBasis::Xy).unwrap();
    assert!(poly.convex_piece_count() >= 2);
    let r = poly.region();
    assert!(r.contains(p(0.5, 1.5, 0.0), Inside));
    assert!(r.contains(p(1.5, 0.5, 7.0), Inside));
    assert!(!r.contains(p(1.5, 1.5, 0.0), Inside));
    // Clockwise input is reversed to counter-clockwise; closing point dropped.
    let mut cw: Vec<[f64; 2]> = l.iter().rev().copied().collect();
    cw.push(cw[0]);
    let poly_cw = Polygon::new(&cw, PolygonBasis::Xy).unwrap();
    assert_eq!(poly_cw.points().len(), 6);
    assert!(poly_cw.region().contains(p(0.5, 1.5, 0.0), Inside));
}

#[test]
fn collinear_vertices_on_an_edge_do_not_break_the_triangulation() {
    let pts = [[0.0, 0.0], [1.0, 0.0], [2.0, 0.0], [2.0, 2.0], [0.0, 2.0]];
    let poly = Polygon::new(&pts, PolygonBasis::Xy).unwrap();
    assert!(poly.region().contains(p(1.0, 1.0, 0.0), Inside));
    assert!(!poly.region().contains(p(1.0, -0.1, 0.0), Inside));
}
