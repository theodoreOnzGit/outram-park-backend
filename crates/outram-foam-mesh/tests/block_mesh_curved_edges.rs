// SPDX-License-Identifier: GPL-3.0-only
// Copyright (C) 2026 OUTRAM PARK contributors
// Algorithms ported from OpenFOAM-dev 852527377c7e (OpenFOAM Foundation);
// fixtures `T3A_blockMeshDict` and `pitzDailySteady_blockMeshDict` are copies
// of the GPL-3.0-or-later tutorials in that commit.

//! V&V of blockMesh curved edges, `units`, `defaultPatch`, `$var` and the
//! refusal of unsupported keywords (gh:#1461, gh:#1463, epic gh:#1458).
//!
//! **Method.** No upstream `blockMesh` binary is available on this machine, so
//! each port is judged against upstream's own analytic definitions: the circle
//! through three points (`arcEdge`), the helix of an angle/axis arc, the knots
//! of a `polyLine`, the straight-line-distance parameterisation and the knot
//! interpolation of a Catmull-Rom `splineEdge`, the closed-form area of a
//! polygonal annular sector for the blended block, and `units` multipliers from
//! `etc/configDict`. The predictions were written and committed before the
//! first run: `verification_and_validation/block_mesh_curved_edges.md` (P1-P8),
//! which also holds the measured results. Pass criteria are the P-numbers'
//! tolerances, quoted at each test.

use outram_foam_basic_lib::mesh::PatchKind;
use outram_foam_basic_lib::primitives::Vector3;
use outram_foam_mesh::block_edges::{BlockEdge, BlockEdgeKind, EdgeCurve};
use outram_foam_mesh::block_mesh::{block_mesh, BlockMeshDict};
use outram_foam_mesh::MeshError;

const T3A: &str = include_str!("fixtures/T3A_blockMeshDict");
const PITZ: &str = include_str!("fixtures/pitzDailySteady_blockMeshDict");

fn c45() -> f64 {
    std::f64::consts::FRAC_1_SQRT_2
}

/// Quarter annulus r in [1, 2], one block (4 radial, n angular, 1) with the two arcs.
fn annulus_dict(n: usize, with_edges: bool) -> String {
    let edges = if with_edges {
        format!(
            "edges ( arc 1 2 ({a} {a} 0) arc 0 3 ({b} {b} 0) arc 5 6 ({a} {a} 1) arc 4 7 ({b} {b} 1) );",
            a = 2.0 * c45(),
            b = c45()
        )
    } else {
        String::new()
    };
    format!(
        "convertToMeters 1;
vertices ((1 0 0) (2 0 0) (0 2 0) (0 1 0) (1 0 1) (2 0 1) (0 2 1) (0 1 1));
blocks ( hex (0 1 2 3 4 5 6 7) (4 {n} 1) simpleGrading (1 1 1) );
{edges}
boundary ( walls {{ type wall; faces ((0 1 5 4) (1 2 6 5) (2 3 7 6) (3 0 4 7)); }} );"
    )
}

/// P1: nodes of the arc-edged block lie on circles at uniform angle, and the
/// volume is the closed form of the polygonal sector.
#[test]
fn quarter_annulus_nodes_on_circles_and_closed_form_volume() {
    let n = 8usize;
    let m = block_mesh(&annulus_dict(n, true)).unwrap();
    // Every z = 0 node: radius is 1 + s/4 for s = 0..=4 and angle is a
    // multiple of (pi/2)/n.
    let mut worst_r = 0.0f64;
    let mut worst_a = 0.0f64;
    for p in m.points.iter().filter(|p| p.z == 0.0) {
        let r = (p.x * p.x + p.y * p.y).sqrt();
        let s = ((r - 1.0) * 4.0).round();
        worst_r = worst_r.max((r - (1.0 + s / 4.0)).abs());
        let a = p.y.atan2(p.x) / (std::f64::consts::FRAC_PI_2 / n as f64);
        worst_a = worst_a.max((a - a.round()).abs() * std::f64::consts::FRAC_PI_2 / n as f64);
    }
    println!("P1 n={n}: worst radius error {worst_r:e}, worst angle error {worst_a:e}");
    assert!(worst_r < 1e-14, "radius error {worst_r:e}");
    assert!(worst_a < 1e-14, "angle error {worst_a:e}");
    let want = (n as f64 / 2.0) * (std::f64::consts::FRAC_PI_2 / n as f64).sin() * (4.0 - 1.0);
    let got = m.total_volume();
    println!("P1 volume {got} vs closed form {want}, rel {:e}", ((got - want) / want).abs());
    assert!(((got - want) / want).abs() < 1e-12, "volume {got} vs {want}");
}

/// P1 contrast: without the `edges` list the same dict is the straight-sided
/// trapezoid of area 1.5, which is what every curved dict used to build.
#[test]
fn without_edges_the_block_is_straight_sided() {
    let m = block_mesh(&annulus_dict(8, false)).unwrap();
    println!("P1 straight-edge volume {}", m.total_volume());
    assert!((m.total_volume() - 1.5).abs() < 1e-12);
}

/// Reversing an edge's vertex order gives the same mesh (the
/// `blockDescriptorEdges` reversed branch: inverted grading, reversed points).
#[test]
fn reversed_edge_orientation_gives_the_same_points() {
    let fwd = block_mesh(&annulus_dict(6, true)).unwrap();
    let rev_text = annulus_dict(6, true).replace("arc 1 2", "arc 2 1").replace("arc 0 3", "arc 3 0");
    let rev = block_mesh(&rev_text).unwrap();
    assert_eq!(fwd.points.len(), rev.points.len());
    for p in &fwd.points {
        assert!(
            rev.points.iter().any(|q| (*q - *p).mag() < 1e-13),
            "no matching point for {p:?}"
        );
    }
}

fn resolve(kind: BlockEdgeKind, v: &[Vector3]) -> EdgeCurve {
    BlockEdge { start: 0, end: 1, kind }.resolve(v).unwrap()
}

/// P2: angle/axis arc is a unit circle about the origin; with an axial chord
/// component it is a helix whose z is linear in lambda.
#[test]
fn arc_angle_axis_circle_and_helix() {
    let axis = Vector3::new(0.0, 0.0, 1.0);
    let flat = [Vector3::new(1.0, 0.0, 0.0), Vector3::new(0.0, 1.0, 0.0)];
    let arc = resolve(BlockEdgeKind::ArcAngleAxis { angle_deg: 90.0, axis }, &flat);
    for k in 0..=10 {
        let p = arc.position(k as f64 / 10.0);
        assert!((p.mag() - 1.0).abs() < 1e-15, "radius {}", p.mag());
        assert_eq!(p.z, 0.0);
    }
    let p1 = arc.position(1.0);
    assert!((p1 - flat[1]).mag() < 1e-15);
    let helix_v = [Vector3::new(1.0, 0.0, 0.0), Vector3::new(0.0, 1.0, 1.0)];
    let helix = resolve(BlockEdgeKind::ArcAngleAxis { angle_deg: 90.0, axis }, &helix_v);
    for k in 0..=10 {
        let l = k as f64 / 10.0;
        let p = helix.position(l);
        assert!((p.z - l).abs() < 1e-15);
        assert!((p.x.hypot(p.y) - 1.0).abs() < 1e-15);
    }
    assert!((helix.position(1.0) - helix_v[1]).mag() < 1e-15);
}

/// Upstream rejects an arc point on the chord (`det(dps) < vSmall`).
#[test]
fn arc_point_on_the_chord_is_an_error() {
    let v = [Vector3::new(0.0, 0.0, 0.0), Vector3::new(2.0, 0.0, 0.0)];
    let e = BlockEdge {
        start: 0,
        end: 1,
        kind: BlockEdgeKind::ArcThroughPoint(Vector3::new(1.0, 0.0, 0.0)),
    };
    assert!(matches!(e.resolve(&v), Err(MeshError::Construction(_))));
}

fn knots() -> (Vec<Vector3>, Vec<Vector3>) {
    let inner = vec![
        Vector3::new(1.0, 0.8, 0.0),
        Vector3::new(2.0, 0.5, 0.0),
        Vector3::new(3.0, 1.2, 0.0),
    ];
    (vec![Vector3::new(0.0, 0.0, 0.0), Vector3::new(4.0, 0.0, 0.0)], inner)
}

/// P3: a polyLine passes through each knot at its normalised cumulative length.
#[test]
fn polyline_passes_through_knots() {
    let (ends, inner) = knots();
    let curve = resolve(BlockEdgeKind::PolyLine(inner.clone()), &ends);
    let mut all = vec![ends[0]];
    all.extend(inner);
    all.push(ends[1]);
    let total: f64 = all.windows(2).map(|w| (w[1] - w[0]).mag()).sum();
    let mut cum = 0.0;
    for k in 1..all.len() {
        cum += (all[k] - all[k - 1]).mag();
        let p = curve.position(if k + 1 == all.len() { 1.0 } else { cum / total });
        assert!((p - all[k]).mag() < 1e-15, "knot {k}: {}", (p - all[k]).mag());
    }
}

/// P4a: collinear, equally spaced knots give a straight line in which lambda
/// is the distance fraction, to the iteration tolerance rootSmall (1.5e-8).
#[test]
fn spline_on_a_line_is_distance_parameterised() {
    let ends = [Vector3::new(0.0, 0.0, 0.0), Vector3::new(4.0, 0.0, 0.0)];
    let inner = vec![
        Vector3::new(1.0, 0.0, 0.0),
        Vector3::new(2.0, 0.0, 0.0),
        Vector3::new(3.0, 0.0, 0.0),
    ];
    for kind in [
        BlockEdgeKind::Spline(inner.clone()),
        BlockEdgeKind::BSpline(inner.clone()),
    ] {
        let c = resolve(kind, &ends);
        for k in 0..=20 {
            let l = k as f64 / 20.0;
            let p = c.position(l);
            assert!((p.x - 4.0 * l).abs() <= 1.5e-8 * 4.0 && p.y == 0.0, "l={l} p={p:?}");
        }
    }
}

/// P4b: Catmull-Rom interpolates its knots at their distance-fraction
/// parameters (to 1e-6 of the length); both splines hit the end points.
#[test]
fn catmull_rom_interpolates_knots() {
    let (ends, inner) = knots();
    let c = resolve(BlockEdgeKind::Spline(inner.clone()), &ends);
    let len = 4.0;
    for k in &inner {
        let d0 = (*k - ends[0]).mag();
        let d1 = (*k - ends[1]).mag();
        let p = c.position(d0 / (d0 + d1));
        assert!((p - *k).mag() < 1e-6 * len, "knot {k:?} -> {p:?}");
    }
    let b = resolve(BlockEdgeKind::BSpline(inner), &ends);
    assert_eq!(b.position(0.0), ends[0]);
    assert_eq!(b.position(1.0), ends[1]);
}

/// P5: `units`, `convertToMeters`, `scale` and their conflict.
#[test]
fn units_keyword_and_conflicts() {
    let cube = |head: &str| {
        format!(
            "{head}
vertices ((0 0 0) (1 0 0) (1 1 0) (0 1 0) (0 0 1) (1 0 1) (1 1 1) (0 1 1));
blocks ( hex (0 1 2 3 4 5 6 7) (1 1 1) simpleGrading (1 1 1) );
boundary ( a {{ type patch; faces ((0 3 2 1)); }} );"
        )
    };
    let mm = block_mesh(&cube("units [mm];")).unwrap();
    assert!((mm.total_volume() / 1e-9 - 1.0).abs() < 1e-12);
    let inch = BlockMeshDict::parse(&cube("units [in];")).unwrap();
    assert_eq!(inch.convert_to_meters, 25.4 * 1e-3);
    assert_eq!(BlockMeshDict::parse(&cube("scale 0.5;")).unwrap().convert_to_meters, 0.5);
    assert!(BlockMeshDict::parse(&cube("units [mm]; convertToMeters 1;")).is_err());
    assert!(matches!(
        BlockMeshDict::parse(&cube("units [kg];")),
        Err(MeshError::NotImplemented(_))
    ));
}

/// P6: `defaultPatch` name and type; the default is `empty`.
#[test]
fn default_patch_name_and_type() {
    let dict = |extra: &str| {
        format!(
            "{extra}
vertices ((0 0 0) (1 0 0) (1 1 0) (0 1 0) (0 0 1) (1 0 1) (1 1 1) (0 1 1));
blocks ( hex (0 1 2 3 4 5 6 7) (2 2 1) simpleGrading (1 1 1) );
boundary ( left {{ type patch; faces ((0 4 7 3)); }} );"
        )
    };
    let last = |t: &str| {
        let m = block_mesh(t).unwrap();
        let p = m.patches.last().unwrap().clone();
        (p.name, p.kind)
    };
    assert_eq!(last(&dict("")), ("defaultFaces".to_string(), PatchKind::Empty));
    assert_eq!(
        last(&dict("defaultPatch { name rest; type patch; }")),
        ("rest".to_string(), PatchKind::Patch)
    );
    assert!(BlockMeshDict::parse(&dict("defaultPatch { name rest; }")).is_err());
}

/// P7: unsupported keywords are errors, never skipped.
#[test]
fn unsupported_keywords_are_errors() {
    let base = |extra: &str, edge: &str| {
        format!(
            "{extra}
vertices ((0 0 0) (1 0 0) (1 1 0) (0 1 0) (0 0 1) (1 0 1) (1 1 1) (0 1 1));
blocks ( hex (0 1 2 3 4 5 6 7) (1 1 1) simpleGrading (1 1 1) );
edges ( {edge} );
boundary ( a {{ type patch; faces ((0 3 2 1)); }} );"
        )
    };
    for extra in [
        "geometry { s { type triSurfaceMesh; } }",
        "faces ( project (0 1 2 3) s );",
        "mergePatchPairs ( (a b) );",
        "x #calc \"1+1\";",
        "y ${missing};",
    ] {
        assert!(
            matches!(BlockMeshDict::parse(&base(extra, "")), Err(MeshError::NotImplemented(_))),
            "{extra}"
        );
    }
    assert!(matches!(
        BlockMeshDict::parse(&base("", "project 0 1 (s)")),
        Err(MeshError::NotImplemented(_))
    ));
    assert!(BlockMeshDict::parse(&base("mergePatchPairs ( );", "")).is_ok());
    // Supported since gh:#1463: an explicit name table.
    assert!(BlockMeshDict::parse(&base("namedVertices { a 0; }", "")).is_ok());
}

/// P8: T3A (8 arcs, `defaultPatch { type empty; }`) builds a valid mesh whose
/// arc-edge nodes lie on the circle of their three dictionary points.
#[test]
fn t3a_tutorial_builds_with_its_arcs() {
    let dict = BlockMeshDict::parse(T3A).unwrap();
    assert_eq!(dict.edges.len(), 8);
    assert_eq!(dict.default_patch_kind, PatchKind::Empty);
    let m = dict.build().unwrap();
    m.to_fv_mesh().unwrap();
    // Arc 1 3 through (0.02158.. 0.00794.. -0.05): block 0 edge from vertex 1
    // to 3 is the 7-cell x edge... check every mesh point at z = -0.05 that is
    // within the arc's angular span lies on the circle where it is a node of
    // that edge: the arc's own three points are mesh points, and the circle
    // through them must contain every node placed on the arc.
    let v = |i: usize| dict.vertices[i];
    let pa = Vector3::new(0.0215794997003908, 0.00794068122157562, -0.05);
    let (p0, p1) = (v(1), v(3));
    let arc = BlockEdge {
        start: 1,
        end: 3,
        kind: BlockEdgeKind::ArcThroughPoint(pa),
    }
    .resolve(&dict.vertices)
    .unwrap();
    let EdgeCurve::Arc(a) = &arc else { panic!("not an arc") };
    let r = (p0 - a.centre()).mag();
    for pt in [p1, pa] {
        assert!(((pt - a.centre()).mag() / r - 1.0).abs() < 1e-12);
    }
    let mut on_arc = 0;
    for p in m.points.iter().filter(|p| (p.z + 0.05).abs() < 1e-12) {
        let d = (*p - a.centre()).mag();
        if (d / r - 1.0).abs() < 1e-9 && p.x > 0.02 && p.x < 0.0261 && p.y > 0.0 && p.y < 0.0147 + 1e-9 {
            on_arc += 1;
        }
    }
    // 20 cells across block 1..3 / blocks 0 and 1 share it: at least the 8
    // interior nodes of the 7-cell edge of block 0 lie on that circle.
    println!("P8 T3A: {} points, {} cells, {on_arc} nodes on arc 1-3", m.points.len(), m.n_cells);
    assert!(on_arc >= 8, "only {on_arc} nodes on the arc");
}

/// Block zone names are kept as `cell_zones` (gh:#1463), not discarded.
#[test]
fn block_zone_names_become_cell_zones() {
    let dict = "vertices ((0 0 0) (1 0 0) (1 1 0) (0 1 0) (0 0 1) (1 0 1) (1 1 1) (0 1 1)
        (2 0 0) (2 1 0) (2 0 1) (2 1 1));
blocks ( hex (0 1 2 3 4 5 6 7) left (2 1 1) simpleGrading (1 1 1)
         hex (1 8 9 2 5 10 11 6) right (3 1 1) simpleGrading (1 1 1) );
boundary ( a { type patch; faces ((0 4 7 3)); } );";
    let m = block_mesh(dict).unwrap();
    assert_eq!(m.cell_zones.len(), 2);
    assert_eq!(m.cell_zones[0], ("left".to_string(), vec![0, 1]));
    assert_eq!(m.cell_zones[1], ("right".to_string(), vec![2, 3, 4]));
}

/// `pitzDailySteady`: `units [mm]` and `$posY`/`$negY` variables.
#[test]
fn pitz_daily_steady_units_and_variables() {
    let dict = BlockMeshDict::parse(PITZ).unwrap();
    assert_eq!(dict.convert_to_meters, 1e-3);
    let m = dict.build().unwrap();
    m.to_fv_mesh().unwrap();
    let max_x = m.points.iter().map(|p| p.x).fold(f64::MIN, f64::max);
    assert!((max_x - 0.290).abs() < 1e-12, "max x {max_x}");
    println!("pitzDailySteady: {} points, {} cells, max x {max_x}", m.points.len(), m.n_cells);
    assert_eq!(m.patches.iter().map(|p| p.name.as_str()).collect::<Vec<_>>(),
        ["inlet", "outlet", "upperWall", "lowerWall", "frontAndBack"]);
}
