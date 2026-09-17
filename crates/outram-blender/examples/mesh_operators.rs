//! Exercise the now-real mesh operators, modifier stack, procedural graph, and
//! export bridges top-to-bottom.
//!
//! This is the second entry point for the crate (after `authoring_primitives`).
//! It needs only `outram-blender` itself — no other crate, no GUI. Run it with:
//!
//! ```text
//! cargo run -p outram-blender --example mesh_operators --release
//! ```
//!
//! It starts from a [`primitives::cube`] and walks through each capability that
//! was turned from a scaffold stub into working code: the destructive
//! [`ops::MeshOp`] operators (subdivide / extrude / bevel / boolean), the
//! non-destructive [`modifiers::ModifierStack`] (array + Catmull-Clark
//! subsurf), a [`procedural::GeometryGraph`] node evaluation, and the two
//! [`export`] bridges (OpenFOAM polyMesh text + CSG primitive fitting). Every
//! step prints the vertex/edge/face counts and the Euler characteristic so the
//! topology can be eyeballed.

use outram_blender::boolean::boolean;
use outram_blender::export::{to_csg_primitive, to_polymesh_text};
use outram_blender::laplacian::LaplacianWeighting;
use outram_blender::math::Vec3;
use outram_blender::modifiers::{Modifier, ModifierStack};
use outram_blender::ops::{BooleanMode, MeshOp};
use outram_blender::parameterize::{parameterize, BoundaryShape};
use outram_blender::procedural::{GeometryGraph, GeometryNode, PrimitiveKind};
use outram_blender::subdivision::catmull_clark;
use outram_blender::{
    mesh::{Mesh, VertexId},
    primitives,
};

/// Print a one-line topology summary (`V`, `E`, `F`, and `chi = V - E + F`).
fn report(label: &str, m: &Mesh) {
    println!(
        "{label:>34}: V={:>4}  E={:>4}  F={:>4}  chi={}",
        m.vertex_count(),
        m.edge_count(),
        m.face_count(),
        m.euler_characteristic(),
    );
}

fn main() {
    // ---- Destructive operators (crate::ops). -----------------------------
    let cube = primitives::cube(2.0);
    report("cube(2.0)", &cube);

    // Simple midpoint subdivide: each quad -> 4 quads (topology only, no smoothing).
    let subdiv = MeshOp::Subdivide { iterations: 1 }
        .apply(cube.clone())
        .expect("subdivide is infallible");
    report("MeshOp::Subdivide x1 (midpoint)", &subdiv);

    // Vertex bevel (truncation): cube -> truncated cube (6 octagons + 8 triangles).
    let beveled = MeshOp::Bevel {
        width: 0.4,
        segments: 1,
    }
    .apply(cube.clone())
    .expect("bevel is infallible");
    report("MeshOp::Bevel (truncated cube)", &beveled);

    // Boolean intersection of two overlapping cubes (convex restricted case).
    let shifted = shift_x(&primitives::cube(2.0), 1.0);
    let inter = MeshOp::Boolean {
        other: shifted,
        mode: BooleanMode::Intersect,
    }
    .apply(cube.clone())
    .expect("convex cube intersection is supported");
    report("MeshOp::Boolean Intersect (overlap box)", &inter);

    // ---- Catmull-Clark subdivision surface (crate::subdivision). ---------
    // Same topology as the midpoint subdivide, but smoothed toward the limit surface.
    let cc = catmull_clark(&cube, 2);
    report("catmull_clark(cube, levels=2)", &cc);

    // ---- Non-destructive modifier stack (crate::modifiers). --------------
    // Three cubes in a row, then one level of Catmull-Clark subsurf over the lot.
    let stack = ModifierStack::new()
        .push(Modifier::Array {
            count: 3,
            offset: [1.2, 0.0, 0.0],
        })
        .push(Modifier::Subsurf { levels: 1 });
    let derived = stack.evaluate(&cube).expect("array + subsurf evaluate");
    report("ModifierStack: Array x3 -> Subsurf", &derived);

    // ---- Procedural geometry-node graph (crate::procedural). -------------
    // Two spheres joined into one mesh by a small directed graph.
    let mut g = GeometryGraph::new();
    let a = g.add(GeometryNode::Primitive(PrimitiveKind::UvSphere {
        segments: 12,
        rings: 6,
        radius: 1.0,
    }));
    let b = g.add(GeometryNode::Primitive(PrimitiveKind::UvSphere {
        segments: 12,
        rings: 6,
        radius: 1.0,
    }));
    let b_moved = g.add(GeometryNode::Transform {
        input: b,
        translate: [3.0, 0.0, 0.0],
    });
    let joined = g.add(GeometryNode::Join { a, b: b_moved });
    g.add(GeometryNode::OutputMesh { input: joined });
    let graph_mesh = g.evaluate().expect("graph evaluates");
    report("GeometryGraph: two joined spheres", &graph_mesh);

    // ---- Export bridges (crate::export). ---------------------------------
    let poly = to_polymesh_text(&cube);
    println!(
        "\npolyMesh export of cube(2.0): points file {} bytes, faces file {} bytes",
        poly.points.len(),
        poly.faces.len(),
    );

    match to_csg_primitive(&cube) {
        Ok(csg) => println!(
            "CSG primitive fit of cube(2.0): {} analytic surfaces, {} region tokens",
            csg.surfaces.len(),
            csg.region.len(),
        ),
        Err(e) => println!("CSG primitive fit failed: {e}"),
    }

    // A direct boolean call outside the MeshOp wrapper, for completeness.
    let self_inter = boolean(&cube, &cube, BooleanMode::Intersect).expect("self-intersection");
    report("boolean(cube, cube, Intersect)", &self_inter);

    // ---- Sparse-solve geometry-processing operators (crate::{laplacian, ----
    //      arap, decimate, loop_subdivision, convex_hull, parameterize}).
    // These are the `faer`-backed advanced operators. A UV sphere is a good
    // closed test surface; a grid is an open disk for parameterization + ARAP.
    let sphere = primitives::uv_sphere(16, 8, 1.0);
    report("uv_sphere(16,8)", &sphere);

    // Implicit Laplacian fairing (cotangent weights): unconditionally stable,
    // topology-preserving. V/E/F are unchanged; only vertex positions move.
    let smoothed = MeshOp::Smooth {
        weighting: LaplacianWeighting::Cotangent,
        lambda: 0.5,
        iterations: 3,
    }
    .apply(sphere.clone())
    .expect("Laplacian smooth");
    report("Laplacian smooth x3", &smoothed);

    // Taubin λ|μ smoothing: denoises without the shrinkage a plain Laplacian
    // pass causes. Same topology, near-preserved scale.
    let taubin = MeshOp::Taubin {
        weighting: LaplacianWeighting::Uniform,
        lambda: 0.5,
        mu: -0.53,
        iterations: 5,
    }
    .apply(sphere.clone())
    .expect("Taubin smooth");
    report("Taubin smooth x5", &taubin);

    // QEM (Garland–Heckbert) decimation to ~half the triangle budget.
    let target = sphere.face_count() / 2;
    let decimated = MeshOp::Decimate {
        target_faces: target,
    }
    .apply(sphere.clone())
    .expect("decimate is infallible");
    report("QEM decimate ~50%", &decimated);

    // Loop subdivision: one refinement step quadruples the triangle count of a
    // triangle mesh (the convex hull below is triangulated, so feed it that).
    let hull = MeshOp::ConvexHull
        .apply(sphere.clone())
        .expect("convex hull of sphere vertices");
    report("convex hull of sphere verts", &hull);
    let loop_s = MeshOp::LoopSubdivide { iterations: 1 }
        .apply(hull.clone())
        .expect("loop subdivide is infallible");
    report("Loop subdivide x1 (of hull)", &loop_s);

    // Harmonic (Tutte) parameterization of an open disk: a flat grid unwrapped
    // onto the unit square. Returns per-vertex (u, v) coordinates.
    let plane = primitives::grid(8, 8, 2.0);
    report("grid(8,8)", &plane);
    let uv = parameterize(&plane, LaplacianWeighting::Uniform, BoundaryShape::Square)
        .expect("grid is a genus-0 disk");
    let (umin, umax) = uv.iter().fold((f64::MAX, f64::MIN), |(lo, hi), &(u, _)| {
        (lo.min(u), hi.max(u))
    });
    println!(
        "{:>34}: {} UV coords, u in [{:.3}, {:.3}]",
        "Tutte parameterize(grid)",
        uv.len(),
        umin,
        umax,
    );

    // ARAP: bend the grid by dragging its four corner vertices upward in Z while
    // pinning the centre, keeping every one-ring as rigid as possible.
    let n = plane.vertex_count();
    let centre = VertexId(n / 2);
    let handles = vec![
        (VertexId(0), Vec3::new(-1.0, -1.0, 0.6)),
        (VertexId(8), Vec3::new(-1.0, 1.0, 0.6)),
        (VertexId(72), Vec3::new(1.0, -1.0, 0.6)),
        (VertexId(80), Vec3::new(1.0, 1.0, 0.6)),
        (centre, Vec3::new(0.0, 0.0, 0.0)),
    ];
    let bent = MeshOp::Arap {
        handles,
        iterations: 8,
    }
    .apply(plane.clone())
    .expect("ARAP deform");
    report("ARAP bend grid (4 corners up)", &bent);

    // ---- Mesh repair / modeling / cutting operators (crate::{weld, ----
    //      fill_holes, recalc_normals, solidify, triangulate, inset, bisect}).
    // Weld a face-varying "exploded" cube (each face owns its 4 corners) back
    // into a shared-vertex solid: 24 loose corners collapse to 8.
    let exploded = explode(&cube);
    report("exploded cube (24 corners)", &exploded);
    let welded = MeshOp::Weld { distance: 1e-6 }
        .apply(exploded)
        .expect("weld");
    report("MeshOp::Weld (stitched)", &welded);

    // Recalculate normals outside makes winding globally consistent + outward.
    let recalced = MeshOp::RecalculateNormals
        .apply(welded)
        .expect("recalc normals");
    report("MeshOp::RecalculateNormals", &recalced);

    // Open a hole (drop one face) then cap it back to watertight.
    let holed = drop_face0(&cube);
    report("cube minus one face (open)", &holed);
    let capped = MeshOp::FillHoles.apply(holed).expect("fill holes");
    report("MeshOp::FillHoles (capped)", &capped);

    // Solidify a flat grid into a closed slab.
    let plate = primitives::grid(4, 4, 2.0);
    let slab = MeshOp::Solidify { thickness: 0.1 }
        .apply(plate)
        .expect("solidify");
    report("MeshOp::Solidify (grid->slab)", &slab);

    // Triangulate and inset the cube.
    let triangulated = MeshOp::Triangulate
        .apply(cube.clone())
        .expect("triangulate");
    report("MeshOp::Triangulate", &triangulated);
    let inset = MeshOp::Inset { amount: 0.3 }
        .apply(cube.clone())
        .expect("inset");
    report("MeshOp::Inset (0.3)", &inset);

    // Bisect the cube at z = 0 (keep the lower half), then cap the cut: a
    // closed box of half the volume.
    let lower = MeshOp::Bisect {
        point: Vec3::ZERO,
        normal: Vec3::new(0.0, 0.0, 1.0),
    }
    .apply(cube.clone())
    .expect("bisect");
    let half = MeshOp::FillHoles.apply(lower).expect("cap the cut");
    report("MeshOp::Bisect + FillHoles", &half);

    println!("\nAll operator / modifier / procedural / export steps ran.");
}

/// Return a face-varying "explosion" of `mesh`: every face gets its own private
/// copy of each corner, so shared corners become several coincident vertices —
/// the classic soup a `Weld` stitches back together.
fn explode(mesh: &Mesh) -> Mesh {
    let ps = mesh.positions();
    let mut positions: Vec<outram_blender::math::Vec3> = Vec::new();
    let mut faces: Vec<Vec<usize>> = Vec::new();
    for poly in mesh.polygons() {
        let mut face = Vec::with_capacity(poly.len());
        for v in poly {
            positions.push(ps[v.0]);
            face.push(positions.len() - 1);
        }
        faces.push(face);
    }
    Mesh::from_polygons(&positions, &faces)
}

/// Return a copy of `mesh` with its first face removed, opening a hole.
fn drop_face0(mesh: &Mesh) -> Mesh {
    let ps = mesh.positions();
    let faces: Vec<Vec<usize>> = mesh
        .polygons()
        .into_iter()
        .enumerate()
        .filter(|(i, _)| *i != 0)
        .map(|(_, poly)| poly.into_iter().map(|v| v.0).collect())
        .collect();
    Mesh::from_polygons(&ps, &faces)
}

/// Return a copy of `mesh` translated by `dx` along X (helper for the boolean demo).
///
/// Uses the polygon-soup view so it stays independent of the half-edge internals.
fn shift_x(mesh: &Mesh, dx: f64) -> Mesh {
    let positions: Vec<_> = mesh
        .positions()
        .into_iter()
        .map(|p| outram_blender::math::Vec3::new(p.x + dx, p.y, p.z))
        .collect();
    let faces: Vec<Vec<usize>> = mesh
        .polygons()
        .into_iter()
        .map(|face| face.into_iter().map(|v| v.0).collect())
        .collect();
    Mesh::from_polygons(&positions, &faces)
}
