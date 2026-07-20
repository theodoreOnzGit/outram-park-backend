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
use outram_blender::modifiers::{Modifier, ModifierStack};
use outram_blender::ops::{BooleanMode, MeshOp};
use outram_blender::procedural::{GeometryGraph, GeometryNode, PrimitiveKind};
use outram_blender::subdivision::catmull_clark;
use outram_blender::{mesh::Mesh, primitives};

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
    let beveled = MeshOp::Bevel { width: 0.4, segments: 1 }
        .apply(cube.clone())
        .expect("bevel is infallible");
    report("MeshOp::Bevel (truncated cube)", &beveled);

    // Boolean intersection of two overlapping cubes (convex restricted case).
    let shifted = shift_x(&primitives::cube(2.0), 1.0);
    let inter = MeshOp::Boolean { other: shifted, mode: BooleanMode::Intersect }
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
        .push(Modifier::Array { count: 3, offset: [1.2, 0.0, 0.0] })
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
    let b_moved = g.add(GeometryNode::Transform { input: b, translate: [3.0, 0.0, 0.0] });
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

    println!("\nAll operator / modifier / procedural / export steps ran.");
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
