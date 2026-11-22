// SPDX-License-Identifier: GPL-3.0-only
// Copyright (C) 2026 OUTRAM PARK contributors
// Algorithms ported from OpenFOAM-dev 852527377c7e (OpenFOAM Foundation);
// the v2506 hashes below come from a run of OpenFOAM.com v2506 `blockMesh`.

//! V&V of blockMesh projection (gh:#1462, epic gh:#1458): `geometry`,
//! `project` vertices / edges / faces, `sweep` faces.
//!
//! **Method.** Predictions P1-P9 are in
//! `verification_and_validation/block_mesh_projection.md`, committed before the
//! first run, with the measured results. (a) Closed forms for the searchable
//! surfaces and the projected nodes. (b) A frozen differential against
//! OpenFOAM.com v2506 `blockMesh` (run on 2026-11-11 with
//! `writePrecision 17`; the binary is not needed here): the FNV-1a hash of the
//! bit patterns of all `points` of five dicts, which the v2506 flavour of this
//! port reproduces exactly, so every point is bit-identical. (c) The dev
//! flavour against the v2506 one, to show where the two source trees differ.
//! (d) Analytic checks of `sweep` on the torus and sphere blocks of the dev
//! `mesh/blockMesh/sweep` tutorial.
//! The dicts are in `tests/fixtures/`; the five `proj_*.dict` are the ones that
//! were run in v2506.

use outram_foam_basic_lib::primitives::Vector3;
use outram_foam_mesh::block_mesh::{BlockMeshDict, PolyMesh};
use outram_foam_mesh::block_projection::{
    find_curved_faces, BlockFace, BlockFaceKind, Geometry, ProjectEdge, ProjectionFlavour,
    SearchableSurface,
};
use outram_foam_mesh::MeshError;
use std::sync::Arc;

fn fixture(name: &str) -> String {
    std::fs::read_to_string(format!("{}/tests/fixtures/{name}", env!("CARGO_MANIFEST_DIR")))
        .unwrap()
}

fn build(name: &str, flavour: ProjectionFlavour) -> PolyMesh {
    BlockMeshDict::parse_with_flavour(&fixture(name), flavour)
        .unwrap()
        .build()
        .unwrap()
}

/// FNV-1a over the bit patterns of the points (the `compare_polymesh` hash).
fn points_hash(m: &PolyMesh) -> u64 {
    m.points
        .iter()
        .flat_map(|p| [p.x.to_bits(), p.y.to_bits(), p.z.to_bits()])
        .fold(0xcbf29ce484222325u64, |h, b| (h ^ b).wrapping_mul(0x100000001b3))
}

fn boundary_points(m: &PolyMesh) -> Vec<Vector3> {
    let mut idx: Vec<usize> = Vec::new();
    for p in &m.patches {
        for f in &m.faces[p.start..p.start + p.size] {
            idx.extend(f.verts.iter().copied());
        }
    }
    idx.sort_unstable();
    idx.dedup();
    idx.into_iter().map(|i| m.points[i]).collect()
}

fn mag(v: Vector3) -> f64 {
    v.mag()
}

/// P1: closed forms of the three searchable surfaces.
#[test]
fn p1_surfaces_closed_form() {
    let g = Geometry::from_entries(
        &{
            let toks: Vec<String> = "{ s { type sphere; origin (1 2 3); radius 2; } \
                c { type cylinder; point1 (0 0 0); point2 (0 0 4); radius 1; } \
                p { type plane; planeType pointAndNormal; \
                    pointAndNormalDict { point (0 0 1); normal (0 0 5); } } }"
                .replace('{', " { ")
                .replace('}', " } ")
                .replace('(', " ( ")
                .replace(')', " ) ")
                .replace(';', " ; ")
                .split_whitespace()
                .map(str::to_string)
                .collect();
            let mut pos = 0;
            outram_foam_mesh::block_projection::read_dict_body(&toks, &mut pos).unwrap()
        },
        ProjectionFlavour::Dev,
    )
    .unwrap();
    let (s, c, p) = (&g.surfaces[0].1, &g.surfaces[1].1, &g.surfaces[2].1);
    let big = 1e6;
    // Sphere: radial distance after projection, and the centre special case.
    let q = s.find_nearest(Vector3::new(4.0, 1.0, 7.0), big, ProjectionFlavour::Dev).unwrap();
    let r = mag(q - Vector3::new(1.0, 2.0, 3.0));
    assert!((r - 2.0).abs() < 4e-16 * 2.0, "{r}");
    let q = s.find_nearest(Vector3::new(1.0, 2.0, 3.0), big, ProjectionFlavour::Dev).unwrap();
    assert_eq!(q, Vector3::new(3.0, 2.0, 3.0));
    let q = s.find_nearest(Vector3::new(1.0, 2.0, 3.0), big, ProjectionFlavour::Esi).unwrap();
    assert_eq!(q, Vector3::new(3.0, 2.0, 3.0));
    // Out of range: a miss.
    assert!(s.find_nearest(Vector3::new(40.0, 0.0, 0.0), 1.0, ProjectionFlavour::Dev).is_none());
    // Cylinder wall, end cap, axis point.
    let q = c.find_nearest(Vector3::new(3.0, 4.0, 2.0), big, ProjectionFlavour::Dev).unwrap();
    assert!(((q.x * q.x + q.y * q.y).sqrt() - 1.0).abs() < 4e-16 && (q.z - 2.0).abs() < 1e-15);
    let q = c.find_nearest(Vector3::new(0.3, 0.0, -2.0), big, ProjectionFlavour::Dev).unwrap();
    assert_eq!(q, Vector3::new(0.3, 0.0, 0.0));
    let q = c.find_nearest(Vector3::new(0.0, 0.0, 1.5), big, ProjectionFlavour::Dev).unwrap();
    assert!((mag(Vector3::new(q.x, q.y, 0.0)) - 1.0).abs() < 1e-15 && q.z == 1.5);
    // Plane (normal normalised): distance from the plane after projection.
    let q = p.find_nearest(Vector3::new(2.0, 3.0, 9.0), big, ProjectionFlavour::Dev).unwrap();
    assert_eq!(q, Vector3::new(2.0, 3.0, 1.0));
    // Normals: sphere radial, cylinder radial / cap, plane constant.
    let n = s.normal_at(Vector3::new(3.0, 2.0, 3.0), ProjectionFlavour::Dev);
    assert!((n.x - 1.0).abs() < 1e-12);
    let n = s.normal_at(Vector3::new(3.0, 2.0, 3.0), ProjectionFlavour::Esi);
    assert!((n.x - 1.0).abs() < 1e-15);
    assert_eq!(c.normal_at(Vector3::new(1.0, 0.0, 2.0), ProjectionFlavour::Dev), Vector3::new(1.0, 0.0, 0.0));
    assert_eq!(c.normal_at(Vector3::new(0.5, 0.0, 0.0), ProjectionFlavour::Dev), Vector3::new(0.0, 0.0, -1.0));
    assert_eq!(c.normal_at(Vector3::new(0.5, 0.0, 4.0), ProjectionFlavour::Dev), Vector3::new(0.0, 0.0, 1.0));
    assert_eq!(p.normal_at(Vector3::ZERO, ProjectionFlavour::Dev), Vector3::new(0.0, 0.0, 1.0));
    // Other sidewall / cap branches of the cylinder normal.
    assert_eq!(c.normal_at(Vector3::new(1.01, 0.0, -0.001), ProjectionFlavour::Dev), Vector3::new(1.0, 0.0, 0.0));
    assert_eq!(c.normal_at(Vector3::new(1.0, 0.0, 3.0), ProjectionFlavour::Dev), Vector3::new(1.0, 0.0, 0.0));
    assert_eq!(c.normal_at(Vector3::new(1.2, 0.0, 4.1), ProjectionFlavour::Dev), Vector3::new(1.0, 0.0, 0.0));
    assert_eq!(c.normal_at(Vector3::new(0.2, 0.0, 4.5), ProjectionFlavour::Dev), Vector3::new(0.0, 0.0, 1.0));
    // Bounds: sphere box, plane half-space.
    let (lo, hi) = s.bounds();
    assert_eq!((lo, hi), (Vector3::new(-1.0, 0.0, 1.0), Vector3::new(3.0, 4.0, 5.0)));
    let (lo, hi) = p.bounds();
    assert_eq!(lo.z, -f64::MAX / 10.0);
    assert_eq!(hi.z, 1.0);
    let (lo, hi) = c.bounds();
    assert_eq!((lo.z, hi.z), (0.0, 4.0));
    assert!((hi.x - 1.0).abs() < 1e-15);
}

/// P2-P5: the v2506 flavour reproduces OpenFOAM.com v2506 `blockMesh`'s
/// `points` bit for bit on five dicts (frozen FNV-1a of the binary's output,
/// run 2026-11-11, `writePrecision 17`).
#[test]
fn p2_to_p6_v2506_flavour_matches_the_binary_bit_for_bit() {
    let frozen = [
        ("proj_vertex.dict", 210usize, 0x7892febae9a4f772u64),
        ("proj_edges.dict", 7351, 0x5019a5fe8dc621ca),
        ("proj_faces.dict", 7351, 0x45ba51ed7c5d5b19),
        ("proj_cyl.dict", 567, 0x5153c57b3d7f213f),
        ("proj_two.dict", 567, 0xb35e095b92028826),
    ];
    for (name, n, hash) in frozen {
        let m = build(name, ProjectionFlavour::Esi);
        assert_eq!(m.points.len(), n, "{name}");
        assert_eq!(points_hash(&m), hash, "{name}: points differ from v2506");
    }
}

/// P3: nodes of a projected edge lie on the sphere (the loop leaves a
/// projection as its last act on convergence) and converge in few passes.
#[test]
fn p3_projected_edge_nodes_on_sphere() {
    let g = Arc::new(Geometry {
        surfaces: vec![(
            "s".into(),
            SearchableSurface::Sphere {
                centre: Vector3::ZERO,
                radius: 1.0,
            },
        )],
        flavour: ProjectionFlavour::Dev,
    });
    let a = Vector3::new(1.0, 0.0, 0.0);
    let b = Vector3::new(0.0, 1.0, 0.0);
    let e = ProjectEdge::new(g, vec![0], a, b);
    let lam: Vec<f64> = (0..=10).map(|i| i as f64 / 10.0).collect();
    let (pts, passes) = e.positions_counted(&lam);
    for p in &pts {
        assert!((mag(*p) - 1.0).abs() < 1e-14, "{p:?}");
    }
    // Equal-arc spacing on a quarter circle: the iteration moves the chord
    // fractions onto equal arc fractions.
    let ang = |p: &Vector3| p.y.atan2(p.x);
    for (k, p) in pts.iter().enumerate() {
        let want = std::f64::consts::FRAC_PI_2 * lam[k];
        assert!((ang(p) - want).abs() < 1e-3, "node {k}: {} vs {want}", ang(p));
    }
    assert!(passes <= 4, "passes {passes}");
    // The single-lambda form: projected once, ends exact.
    assert_eq!(e.position(0.0), a);
    assert_eq!(e.position(1.0), b);
    assert!((mag(e.position(0.5)) - 1.0).abs() < 1e-15);
}

/// P4: dev and v2506 face loops differ as the source diff predicts.
#[test]
fn p4_dev_and_v2506_face_loops_differ_as_predicted() {
    let dev = build("proj_faces.dict", ProjectionFlavour::Dev);
    let esi = build("proj_faces.dict", ProjectionFlavour::Esi);
    assert_eq!(dev.points.len(), esi.points.len());
    // (b) radial error of the walls: v2506 leaves them on the sphere.
    let rad = |m: &PolyMesh| {
        boundary_points(m)
            .iter()
            .map(|p| (mag(*p) - 1.0).abs())
            .fold(0.0f64, f64::max)
    };
    let (r_dev, r_esi) = (rad(&dev), rad(&esi));
    println!("P4 wall radial error: dev {r_dev:e}, v2506 {r_esi:e}");
    assert!(r_esi < 1e-15, "{r_esi:e}");
    // MISS recorded: predicted 1e-9 < r_dev < 1e-4, measured 2.1e-4.
    assert!(r_dev > 1e-4 && r_dev < 1e-3, "{r_dev:e}");
    // (c) volume points differ by at most 1e-3 R.
    let worst = dev
        .points
        .iter()
        .zip(&esi.points)
        .map(|(a, b)| mag(*a - *b))
        .fold(0.0f64, f64::max);
    println!("P4 dev vs v2506 worst point difference: {worst:e}");
    assert!(worst > 0.0 && worst < 1e-3, "{worst:e}");
}

/// P5: cylinder faces: dev face-interior nodes within 1e-4 of the radius.
#[test]
fn p5_cylinder_faces() {
    let dev = build("proj_cyl.dict", ProjectionFlavour::Dev);
    let rad = boundary_points(&dev)
        .iter()
        .filter(|p| p.z > 0.2 + 1e-9 && p.z < 1.8 - 1e-9)
        .map(|p| ((p.x * p.x + p.y * p.y).sqrt() - 1.0).abs())
        .fold(0.0f64, f64::max);
    println!("P5 cylinder wall radial error (dev): {rad:e}");
    assert!(rad < 1e-4, "{rad:e}");
}

/// P6: an edge on the cylinder AND the plane satisfies both equations, in both
/// flavours; the flavours agree to round-off.
#[test]
fn p6_two_surface_edge_on_both() {
    for fl in [ProjectionFlavour::Dev, ProjectionFlavour::Esi] {
        let d = BlockMeshDict::parse_with_flavour(&fixture("proj_two.dict"), fl).unwrap();
        for v in &d.vertices[0..4] {
            assert!(v.z.abs() < 1e-8, "{fl:?} {v:?}");
            assert!(((v.x * v.x + v.y * v.y).sqrt() - 1.0).abs() < 1e-8, "{fl:?} {v:?}");
        }
        assert!(d.build().is_ok());
    }
    let a = build("proj_two.dict", ProjectionFlavour::Dev);
    let b = build("proj_two.dict", ProjectionFlavour::Esi);
    let worst = a
        .points
        .iter()
        .zip(&b.points)
        .map(|(p, q)| mag(*p - *q))
        .fold(0.0f64, f64::max);
    println!("P6 dev vs v2506 worst: {worst:e}");
    assert!(worst < 1e-6, "{worst:e}");
}

/// Distance of `p` from the torus centred (6,0,0) with axis y, R = 2, r = 1.
fn torus_residual(p: Vector3) -> f64 {
    let (x, z) = (p.x - 6.0, p.z);
    (((x * x + z * z).sqrt() - 2.0).powi(2) + p.y * p.y).sqrt() - 1.0
}

fn off_torus_count(name: &str, tol: f64) -> (usize, f64) {
    let m = build(name, ProjectionFlavour::Dev);
    let pts = boundary_points(&m);
    let worst_on = pts
        .iter()
        .map(|p| torus_residual(*p).abs())
        .filter(|r| *r < 1e-3)
        .fold(0.0f64, f64::max);
    let off = pts.iter().filter(|p| torus_residual(**p).abs() > tol).count();
    (off, worst_on)
}

/// P7: `sweep` on the torus block. Predicted exact (1e-12) in the orientation
/// the tutorial calls "exact" for rotation factor 1 and in the one for factor 0.
/// MEASURED: factor 0 on the major edges is exact (1e-15) and the 50 nodes off
/// the torus are the interiors of the two flat end disks; factor 1 on the
/// minor edges is NOT exact: worst residual 1.2e-4 (see P7b for why); the wrong
/// orientation is off by more. The sphere block is not exact, as predicted.
#[test]
fn p7_sweep_torus_and_sphere() {
    let (off_minor, worst_minor) = off_torus_count("sweep_torus_minor.dict", 1e-9);
    let (off_major, worst_major) = off_torus_count("sweep_torus_major.dict", 1e-9);
    let (off_wrong, _) = off_torus_count("sweep_torus_minor_wrong.dict", 1e-9);
    println!(
        "P7 torus nodes off the torus (50 are disk interiors): minor/1 {off_minor}, \
         major/0 {off_major}, minor/0 {off_wrong}; worst near-torus residual minor/1 \
         {worst_minor:e}, major/0 {worst_major:e}"
    );
    assert_eq!(off_major, 50);
    assert!(worst_major < 1e-12);
    assert!(worst_minor > 1e-6 && worst_minor < 1e-3, "{worst_minor:e}");
    assert!(off_minor > 50 && off_wrong > 50);

    let m = build("sweep_sphere.dict", ProjectionFlavour::Dev);
    let c = Vector3::new(8.0, 0.0, 1.0);
    let miss = boundary_points(&m)
        .iter()
        .map(|p| (mag(*p - c) - 3.0f64.sqrt()).abs())
        .fold(0.0f64, f64::max);
    println!("P7 sphere block: worst distance from the sphere {miss:e}");
    // MISS recorded: predicted 1e-4 < miss < 1e-1, measured 0.139 (8 % of the
    // radius sqrt(3)); the tutorial only says "falls short".
    assert!(miss > 1e-1 && miss < 0.5, "{miss:e}");
}

/// P7b (the finding behind the P7 miss): the swept rows of a torus face are
/// built from a twist that is *linear* in the row angle (each row adds the same
/// dihedral increment), whereas the twist of a finite rotation about the major
/// axis is not, so the algorithm as written cannot be exact for a 90 degree
/// torus, whatever the density. Measured: the worst distance from the exact
/// equal-angle torus points is about 3.3e-3 for 12 rows and for 96 rows alike
/// (no convergence with refinement), against 1e-15 on the edges.
#[test]
fn p7b_sweep_twist_error_does_not_shrink_with_rows() {
    use outram_foam_mesh::block_projection::sweep_face;
    let mut worst_by_rows = Vec::new();
    for rows in [12usize, 96] {
        let cols = 6usize;
        let exact = |i: usize, j: usize| {
            let a = std::f64::consts::PI + j as f64 / cols as f64 * std::f64::consts::FRAC_PI_2;
            let (x, y) = (4.0 + a.cos(), a.sin());
            let phi = i as f64 / rows as f64 * std::f64::consts::FRAC_PI_2;
            let rx = x - 6.0;
            Vector3::new(6.0 + rx * phi.cos(), y, -rx * phi.sin())
        };
        let mut pts = vec![Vector3::ZERO; (rows + 1) * (cols + 1)];
        for i in 0..=rows {
            for j in 0..=cols {
                pts[i * (cols + 1) + j] = if i == 0 || i == rows || j == 0 || j == cols {
                    exact(i, j)
                } else {
                    Vector3::new(5.0, 0.0, 1.0)
                };
            }
        }
        // block face y-min: rows along z (density index 2), columns along x (0)
        sweep_face([cols, 6, rows], 2, false, 1.0, &mut pts);
        let mut worst = 0.0f64;
        for i in 0..=rows {
            for j in 0..=cols {
                worst = worst.max(mag(pts[i * (cols + 1) + j] - exact(i, j)));
            }
        }
        println!("P7b rows {rows}: worst |p - exact| {worst:e}");
        worst_by_rows.push(worst);
    }
    assert!(worst_by_rows[0] > 1e-3 && worst_by_rows[1] > 1e-3);
    assert!((worst_by_rows[0] - worst_by_rows[1]).abs() < 2e-4);
}

/// P8 (part): a dict without `geometry`/`faces`/`project` builds with no
/// projection code involved and the geometry is empty.
#[test]
fn p8_plain_dicts_do_not_touch_projection() {
    let d = BlockMeshDict::parse(&fixture("T3A_blockMeshDict")).unwrap();
    assert!(d.geometry.surfaces.is_empty() && d.faces.is_empty());
    let d = BlockMeshDict::parse(&fixture("pitzDailySteady_blockMeshDict")).unwrap();
    assert!(d.geometry.surfaces.is_empty() && d.faces.is_empty());
}

fn refuses(text: &str) -> String {
    match BlockMeshDict::parse(text).and_then(|d| d.build()) {
        Err(e) => e.to_string(),
        Ok(_) => panic!("accepted"),
    }
}

const CUBE: &str = "vertices ((0 0 0) (1 0 0) (1 1 0) (0 1 0) (0 0 1) (1 0 1) (1 1 1) (0 1 1));\
    blocks (hex (0 1 2 3 4 5 6 7) (2 2 2) simpleGrading (1 1 1));";

/// P9: unported surface types, unknown surfaces and bad faces are refused.
#[test]
fn p9_nothing_is_silently_skipped() {
    for ty in ["extrudedCircle", "plate", "box", "disk", "searchableBox", "triSurface", "triSurfaceMesh", "bogus"] {
        let t = format!("geometry {{ g {{ type {ty}; }} }} {CUBE}");
        let e = BlockMeshDict::parse(&t).unwrap_err();
        assert!(matches!(e, MeshError::NotImplemented(_)), "{ty}: {e}");
        assert!(e.to_string().contains(ty), "{e}");
    }
    // plane given by equation: refused, not guessed
    let t = format!("geometry {{ p {{ type plane; a 0; b 0; c 1; d 0; }} }} {CUBE}");
    assert!(matches!(BlockMeshDict::parse(&t), Err(MeshError::NotImplemented(_))));
    // undefined surface in a vertex, an edge, a face
    let v = "vertices (project (0 0 0) (nope) (1 0 0) (1 1 0) (0 1 0) (0 0 1) (1 0 1) (1 1 1) (0 1 1));\
        blocks (hex (0 1 2 3 4 5 6 7) (2 2 2) simpleGrading (1 1 1));";
    assert!(refuses(v).contains("Cannot find surface nope"));
    let e = format!("{CUBE} edges (project 0 1 (nope));");
    assert!(refuses(&e).contains("Cannot find surface nope"));
    let e = format!("{CUBE} edges (projectCurve 0 1 (nope));");
    assert!(refuses(&e).contains("Cannot find surface nope"));
    let f = format!("{CUBE} faces (project (0 1 2 3) nope);");
    assert!(refuses(&f).contains("Cannot find surface nope"));
    // malformed faces
    assert!(refuses(&format!("{CUBE} faces (project (0 1 2) nope);")).contains("four vertices"));
    assert!(refuses(&format!("{CUBE} faces (sweep (0 1 2 3) 1.5);")).contains("between 0 and 1"));
    assert!(refuses(&format!("{CUBE} faces (bend (0 1 2 3) 1);")).contains("Unknown blockFace"));
    // an unterminated geometry dictionary
    assert!(BlockMeshDict::parse(&format!("geometry {{ s {{ type sphere; {CUBE}")).is_err());
    // the geometry may follow the vertices that use it
    let late = "vertices (project (0.1 0 0) (s) (1 0 0) (1 1 0) (0 1 0) (0 0 1) (1 0 1) (1 1 1) (0 1 1));\
        blocks (hex (0 1 2 3 4 5 6 7) (2 2 2) simpleGrading (1 1 1));\
        geometry { s { type sphere; centre (0 0 0); radius 2; } }";
    let d = BlockMeshDict::parse(late).unwrap();
    assert!((d.vertices[0].mag() - 2.0).abs() < 1e-15);
    assert!(d.vertices[1..].iter().all(|v| v.mag() <= 3f64.sqrt() + 1e-15));
}

/// Curved-face detection and the `faces` entry parse.
#[test]
fn faces_parse_and_find_curved_faces() {
    let d = BlockMeshDict::parse(&fixture("proj_cyl.dict")).unwrap();
    assert_eq!(d.faces.len(), 4);
    assert_eq!(d.faces[0].kind, BlockFaceKind::Project("cyl".into()));
    let curved = find_curved_faces(&d.blocks[0].vertices, &d.faces);
    // x-min, x-max, y-min, y-max curved; z faces flat
    assert_eq!(curved, [Some(0), Some(1), Some(2), Some(3), None, None]);
    let f = vec![BlockFace {
        vertices: [3, 2, 1, 0],
        kind: BlockFaceKind::Sweep(1.0),
    }];
    // the z-min face listed in the opposite order is still that face
    assert_eq!(find_curved_faces(&[0, 1, 2, 3, 4, 5, 6, 7], &f)[4], Some(0));
    // projectCurve parses to its own kind and builds like project
    let dict = fixture("proj_cyl.dict").replace("project 0 1 (cyl)", "projectCurve 0 1 (cyl)");
    let m = BlockMeshDict::parse(&dict).unwrap().build().unwrap();
    assert_eq!(m.points.len(), 567);
    // a plane written with the dev direct keys
    let g = "geometry { p { type searchablePlane; point (0 0 0); normal (1 1 0); } }";
    let d = BlockMeshDict::parse(&format!("{g} {CUBE}")).unwrap();
    assert!((d.geometry.surfaces[0].1.find_nearest(Vector3::new(2.0, 0.0, 0.0), 10.0, ProjectionFlavour::Dev).unwrap().x - 1.0).abs() < 1e-15);
}

/// A block whose `project` face lies exactly on the surface is unchanged by
/// the correction: a plane face on a plane surface leaves a unit cube's volume.
#[test]
fn projected_flat_face_is_a_no_op() {
    let t = format!(
        "geometry {{ pl {{ type plane; point (0 0 1); normal (0 0 1); }} }} {CUBE} \
         faces (project (4 5 6 7) pl);"
    );
    let m = BlockMeshDict::parse(&t).unwrap().build().unwrap();
    assert!((m.total_volume() - 1.0).abs() < 1e-14, "{}", m.total_volume());
}

/// A face projected onto a dome moves its points and the block interior
/// follows (volume grows), in both flavours.
#[test]
fn projected_dome_grows_the_block() {
    let t = format!(
        "geometry {{ s {{ type sphere; centre (0.5 0.5 -2); radius 3.2; }} }} {CUBE} \
         faces (project (4 5 6 7) s);"
    );
    for fl in [ProjectionFlavour::Dev, ProjectionFlavour::Esi] {
        let m = BlockMeshDict::parse_with_flavour(&t, fl).unwrap().build().unwrap();
        assert!(m.total_volume() > 1.0 + 1e-3, "{fl:?} {}", m.total_volume());
        assert!(m.to_fv_mesh().is_ok());
    }
}
