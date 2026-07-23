//! Export bridges from an authored [`Mesh`] to the OUTRAM PARK solvers.
//!
//! The **default** exporters are self-contained and dependency-free: the
//! OpenFOAM bridge emits **text** (the polyMesh ASCII files as `String`s), the
//! CSG bridge emits **local mirror types** ([`CsgSurface`], [`RegionToken`],
//! [`CsgDescription`]) that shadow the consumer crate's geometry vocabulary, and
//! [`FacetedSolid`] carries a triangulated boundary. So the frontend stays light
//! and Android-buildable with no solver-crate dependency.
//!
//! **Real-type bridges** to the actual solver crates are available behind opt-in
//! cargo features, so neither solver crate is a hard dependency:
//!
//! - `foam-export` → `to_poly_mesh` returns a real
//!   `outram_foam_basic_lib::io::poly_mesh::PolyMesh` (the type its OpenFOAM
//!   reader/writer round-trips), and `from_poly_mesh` reads one back into a
//!   [`Mesh`] — combined with `PolyMesh::read(dir)` this **imports** an OpenFOAM
//!   `constant/polyMesh` directory, the inverse of `write_polymesh`;
//! - `mc-export` → `to_mc_geometry` returns a real
//!   `outram_mc_libs::prelude::Geometry` (surfaces + a cell region).
//!
//! These are the wired counterparts of the text / mirror exporters (epic
//! `op-hzs`, beads `op-hzs.6`/`op-hzs.7`).
//!
//! An authored mesh here is a **boundary surface** — a shell of vertices,
//! edges, and polygon faces, with *no cells*. That single fact shapes both
//! bridges, so it is stated up front and repeated at each export point rather
//! than hidden.
//!
//! ## 1. OpenFOAM polyMesh (CFD) — [`to_polymesh_text`] / [`write_polymesh`]
//!
//! OpenFOAM's `polyMesh` (mirrored by `outram_foam_basic_lib`'s `io::poly_mesh`)
//! is normally a finite-volume **VOLUME** mesh: `points`, `faces`,
//! `owner[f]`/`neighbour[f]` (the two cells straddling each *internal* face),
//! and named boundary **patches**.
//!
//! Our surface mesh has faces but **no cells**, so what we emit is a polyMesh
//! **boundary description**, not a solve-ready volume mesh:
//!
//! - `points` — the vertex coordinates;
//! - `faces` — each face as `n(v0 v1 …)`, wound as the mesh winds it;
//! - `owner` — every face owned by a single **dummy cell `0`**, because a
//!   surface has no real cells;
//! - `neighbour` — **empty** (there are no internal faces);
//! - `boundary` — one patch `authoredSurface` of `type patch`, covering all
//!   faces.
//!
//! This is the input a volume mesher (blockMesh / snappyHexMesh) would *fill*
//! to produce cells; it is a boundary patch, **not** a ready-to-solve mesh.
//! Coordinates are dimensionless model space — the caller assigns a length
//! unit (conventionally metres) when handing the mesh to a solver.
//!
//! ## 2. `outram-mc-libs` CSG (Monte Carlo transport) — [`to_csg_primitive`]
//!
//! `outram-mc-libs`'s geometry is **constructive solid geometry**: analytic
//! surfaces (`XPlane`/`YPlane`/`ZPlane`, `Sphere`, `ZCylinder`, …) combined by
//! an RPN region of signed half-spaces into cells. A triangulated boundary
//! mesh does not map onto analytic surfaces directly, so the bridge takes the
//! **primitive-fitting** route: recognise that a mesh *came from* a
//! [`crate::primitives`] generator and emit the exact analytic CSG for it.
//!
//! Implemented analytic fits: an axis-aligned **cube/box** (six planes), a
//! **uv-sphere** (one `Sphere`), a **Z-axis cylinder** (`ZCylinder` ∩ two
//! `ZPlane` caps), and **any convex polyhedron** (the faceted convex route: one
//! general [`CsgSurface::Plane`] per face, intersected — exact because a convex
//! solid is the intersection of its face half-spaces). A **non-convex** mesh is
//! not a half-space intersection, so [`to_csg_primitive`] returns
//! [`ExportError::NotImplemented`] for it.
//!
//! ## 3. Faceted / DAGMC boundary (non-convex Monte Carlo) — [`to_faceted_solid`]
//!
//! For an arbitrary (non-convex) solid — e.g. a general boolean result — the
//! [`FacetedSolid`] route keeps the triangulated boundary as-is (outward
//! oriented) and answers inside/outside by the **generalized winding number**,
//! the DAGMC point-in-volume idea. This is the honest representation when no
//! analytic primitive fits.
//!
//! ## Shared foundation — [`triangulate`]
//!
//! [`triangulate`] provides a dependency-free indexed triangle soup
//! ([`IndexedTriangles`]) — the common denominator an OBJ/STL writer, a
//! polyMesh patch, or a faceted-CSG surface each build on. It is fully
//! implemented and tested.

use crate::math::Vec3;
use crate::mesh::{FaceId, Mesh};

/// Errors from an export bridge.
#[derive(Debug, thiserror::Error)]
pub enum ExportError {
    /// A requested export path is documented but not implemented for this mesh.
    ///
    /// Returned, for example, when [`to_csg_primitive`] is handed a mesh it
    /// cannot recognise as one of the fitted [`crate::primitives`] shapes (the
    /// general faceted/DAGMC route is not written yet). The payload is a
    /// human-readable explanation of what was expected.
    #[error("export bridge not yet implemented: {0}")]
    NotImplemented(&'static str),
}

/// A dependency-free, flattened triangle mesh: the common export denominator.
///
/// Every polygon face of a [`Mesh`] is fan-triangulated into this indexed form
/// (`positions` + triangle `indices` triplets). This is what an OBJ/STL writer,
/// a polyMesh boundary patch, or a faceted-CSG surface would each build from —
/// so it is implemented and tested even while the solver bridges are stubs.
#[derive(Debug, Clone, Default)]
pub struct IndexedTriangles {
    /// Vertex positions, indexed by the entries of [`IndexedTriangles::indices`].
    pub positions: Vec<Vec3>,
    /// Flat list of triangle corner indices, three consecutive entries per
    /// triangle, each indexing into [`IndexedTriangles::positions`].
    pub indices: Vec<u32>,
}

impl IndexedTriangles {
    /// Number of triangles (== `indices.len() / 3`).
    pub fn triangle_count(&self) -> usize {
        self.indices.len() / 3
    }
}

/// Fan-triangulate every face of `mesh` into an [`IndexedTriangles`] soup.
///
/// Each `n`-gon face contributes `n - 2` triangles by a simple fan from its
/// first vertex — valid for the convex faces the [`crate::primitives`]
/// generators produce. Positions are copied 1:1 from the mesh (indices are
/// preserved), so `positions.len()` equals `mesh.vertex_count()`.
///
/// This is real, tested code — the dependency-free foundation the solver
/// bridges below build on.
pub fn triangulate(mesh: &Mesh) -> IndexedTriangles {
    let mut positions = Vec::with_capacity(mesh.vertex_count());
    for i in 0..mesh.vertex_count() {
        // vertex(i) is Some for every i < vertex_count.
        positions.push(mesh.vertex(crate::mesh::VertexId(i)).unwrap().position);
    }

    let mut indices = Vec::new();
    for f in 0..mesh.face_count() {
        let vs = mesh.face_vertices(crate::mesh::FaceId(f));
        if vs.len() < 3 {
            continue;
        }
        // Fan from the first corner: (v0, v1, v2), (v0, v2, v3), …
        for k in 1..(vs.len() - 1) {
            indices.push(vs[0].0 as u32);
            indices.push(vs[k].0 as u32);
            indices.push(vs[k + 1].0 as u32);
        }
    }
    IndexedTriangles { positions, indices }
}

// ===========================================================================
// 1. OpenFOAM polyMesh boundary export
// ===========================================================================

/// The five OpenFOAM `polyMesh` ASCII files, each held as a `String`.
///
/// Produced by [`to_polymesh_text`] with no filesystem access, so it is fully
/// unit-testable; [`write_polymesh`] is the only function that touches disk.
///
/// **These describe a boundary SURFACE, not a solve-ready volume mesh.** Every
/// face is owned by a single dummy cell `0` and there are no internal faces
/// (`neighbour` is empty). A volume mesher must fill the interior before this
/// is a mesh a CFD solver can march on. See the module docs.
///
/// Coordinates are dimensionless model space; the caller assigns a length unit
/// (conventionally metres) at export.
#[derive(Debug, Clone)]
pub struct PolyMeshText {
    /// `constant/polyMesh/points` — `class vectorField; object points;`. One
    /// `(x y z)` per mesh vertex, in [`crate::mesh::VertexId`] order.
    pub points: String,
    /// `constant/polyMesh/faces` — `class faceList; object faces;`. One
    /// `n(v0 v1 …)` per mesh face, wound as the mesh winds it.
    pub faces: String,
    /// `constant/polyMesh/owner` — `class labelList; object owner;`. One label
    /// per face, all `0` (the single dummy cell). A `note` records that this is
    /// a boundary patch, not a volume mesh.
    pub owner: String,
    /// `constant/polyMesh/neighbour` — `class labelList; object neighbour;`.
    /// Empty (zero entries): a boundary surface has no internal faces.
    pub neighbour: String,
    /// `constant/polyMesh/boundary` — `class polyBoundaryMesh; object boundary;`.
    /// A single patch `authoredSurface` of `type patch`, `nFaces` = face count,
    /// `startFace 0`.
    pub boundary: String,
}

/// Build an OpenFOAM ASCII `FoamFile` header block for one polyMesh file.
///
/// `class`/`object` are the OpenFOAM type and file name (e.g. `vectorField` /
/// `points`); `note` is an optional annotation line placed inside the block.
/// Returns the banner comment plus the `FoamFile { … }` dictionary, ending in a
/// trailing newline, ready to have the list body appended.
fn foam_header(class: &str, object: &str, note: Option<&str>) -> String {
    let mut s = String::new();
    s.push_str(
        "/*--------------------------------*- C++ -*----------------------------------*\\\n\
         | =========                 |                                                 |\n\
         | \\\\      /  F ield         | OUTRAM PARK: authored-surface polyMesh export    |\n\
         |  \\\\    /   O peration     | Boundary description (no cells) - see note       |\n\
         |   \\\\  /    A nd           |                                                 |\n\
         |    \\\\/     M anipulation  |                                                 |\n\
         \\*---------------------------------------------------------------------------*/\n",
    );
    s.push_str("FoamFile\n{\n");
    s.push_str("    version     2.0;\n");
    s.push_str("    format      ascii;\n");
    s.push_str(&format!("    class       {class};\n"));
    if let Some(n) = note {
        s.push_str(&format!("    note        \"{n}\";\n"));
    }
    s.push_str(&format!("    object      {object};\n"));
    s.push_str("}\n");
    s.push_str(
        "// * * * * * * * * * * * * * * * * * * * * * * * * * * * * * * * * * * * * * //\n\n",
    );
    s
}

/// Serialise `mesh` to the five OpenFOAM `polyMesh` files as strings.
///
/// Pure and filesystem-free (testable): builds `points`, `faces`, `owner`,
/// `neighbour`, and `boundary` from the mesh's public topology. Faces are
/// written in the mesh's own winding order. Every face is assigned owner cell
/// `0` (a surface has no real cells) and `neighbour` is empty, so the result is
/// a **boundary patch for a volume mesher to fill, not a solve-ready volume
/// mesh** (see [`PolyMeshText`]).
///
/// Coordinates are copied verbatim as dimensionless model-space `f64`s; the
/// caller assigns a length unit (conventionally metres) at export.
pub fn to_polymesh_text(mesh: &Mesh) -> PolyMeshText {
    let n_points = mesh.vertex_count();
    let n_faces = mesh.face_count();

    // ---- points -----------------------------------------------------------
    let mut points = foam_header("vectorField", "points", None);
    points.push_str(&format!("{n_points}\n(\n"));
    for p in mesh.positions() {
        points.push_str(&format!("({} {} {})\n", p.x, p.y, p.z));
    }
    points.push_str(")\n");

    // ---- faces ------------------------------------------------------------
    let mut faces = foam_header("faceList", "faces", None);
    faces.push_str(&format!("{n_faces}\n(\n"));
    for f in 0..n_faces {
        let vs = mesh.face_vertices(FaceId(f));
        let idx: Vec<String> = vs.iter().map(|v| v.0.to_string()).collect();
        faces.push_str(&format!("{}({})\n", vs.len(), idx.join(" ")));
    }
    faces.push_str(")\n");

    // ---- owner ------------------------------------------------------------
    let note = format!(
        "nCells:1 nFaces:{n_faces} - boundary surface, all faces owned by dummy \
         cell 0; NOT a solve-ready volume mesh"
    );
    let mut owner = foam_header("labelList", "owner", Some(note.as_str()));
    owner.push_str(&format!("{n_faces}\n(\n"));
    for _ in 0..n_faces {
        owner.push_str("0\n");
    }
    owner.push_str(")\n");

    // ---- neighbour (empty: no internal faces) -----------------------------
    let mut neighbour = foam_header("labelList", "neighbour", None);
    neighbour.push_str("0\n(\n)\n");

    // ---- boundary ---------------------------------------------------------
    let mut boundary = foam_header("polyBoundaryMesh", "boundary", None);
    boundary.push_str("1\n(\n");
    boundary.push_str("    authoredSurface\n    {\n");
    boundary.push_str("        type            patch;\n");
    boundary.push_str(&format!("        nFaces          {n_faces};\n"));
    boundary.push_str("        startFace       0;\n");
    boundary.push_str("    }\n)\n");

    PolyMeshText { points, faces, owner, neighbour, boundary }
}

/// Write the five [`PolyMeshText`] files into `dir/`, creating `dir` if needed.
///
/// The only filesystem-touching function in this module: it calls
/// [`to_polymesh_text`] and writes `points`, `faces`, `owner`, `neighbour`, and
/// `boundary` (no `.txt` extension — OpenFOAM's exact file names) into `dir`.
/// Typically `dir` is a case's `constant/polyMesh` directory. The output is a
/// boundary surface, not a solve-ready volume mesh (see [`to_polymesh_text`]).
pub fn write_polymesh(mesh: &Mesh, dir: &std::path::Path) -> std::io::Result<()> {
    let t = to_polymesh_text(mesh);
    std::fs::create_dir_all(dir)?;
    std::fs::write(dir.join("points"), t.points)?;
    std::fs::write(dir.join("faces"), t.faces)?;
    std::fs::write(dir.join("owner"), t.owner)?;
    std::fs::write(dir.join("neighbour"), t.neighbour)?;
    std::fs::write(dir.join("boundary"), t.boundary)?;
    Ok(())
}

// ===========================================================================
// 2. CSG primitive-fitting export
// ===========================================================================

/// An analytic CSG surface — a **local mirror** of `outram-mc-libs`'s
/// `SurfaceKind`.
///
/// Each variant is the implicit surface `f(x, y, z) = 0`; its
/// [`CsgSurface::signed_value`] gives `f`, whose sign selects a half-space
/// (see [`Sense`]). Offsets and radii are dimensionless model-space lengths
/// (the caller assigns a length unit, conventionally metres, at export). The
/// variant set here is exactly the subset the primitive fitter emits;
/// `outram-mc-libs` defines the same shapes.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum CsgSurface {
    /// Plane `x = x0`, normal along +X. `f = x - x0`.
    XPlane {
        /// The X coordinate of the plane.
        x0: f64,
    },
    /// Plane `y = y0`, normal along +Y. `f = y - y0`.
    YPlane {
        /// The Y coordinate of the plane.
        y0: f64,
    },
    /// Plane `z = z0`, normal along +Z. `f = z - z0`.
    ZPlane {
        /// The Z coordinate of the plane.
        z0: f64,
    },
    /// Sphere of radius `r` centred at `(x0, y0, z0)`.
    /// `f = (x-x0)^2 + (y-y0)^2 + (z-z0)^2 - r^2` (negative inside).
    Sphere {
        /// Centre X.
        x0: f64,
        /// Centre Y.
        y0: f64,
        /// Centre Z.
        z0: f64,
        /// Radius (`> 0`).
        r: f64,
    },
    /// Infinite cylinder of radius `r` about the line `x = x0, y = y0`, axis
    /// parallel to Z. `f = (x-x0)^2 + (y-y0)^2 - r^2` (negative inside).
    ZCylinder {
        /// Axis X.
        x0: f64,
        /// Axis Y.
        y0: f64,
        /// Radius (`> 0`).
        r: f64,
    },
    /// General (arbitrarily oriented) plane with unit normal `(a, b, c)` at
    /// signed distance `d` from the origin along that normal:
    /// `f = a*x + b*y + c*z - d`. The `+normal` side is `f > 0`. Used by the
    /// faceted convex route ([`to_csg_primitive`]), where each face of a convex
    /// polyhedron becomes one such plane. `(a, b, c)` is expected to be unit
    /// length so `f` is a true signed distance.
    Plane {
        /// Normal X component (unit normal).
        a: f64,
        /// Normal Y component (unit normal).
        b: f64,
        /// Normal Z component (unit normal).
        c: f64,
        /// Signed distance of the plane from the origin along `(a, b, c)`.
        d: f64,
    },
}

impl CsgSurface {
    /// Evaluate the implicit function `f(p)` for this surface.
    ///
    /// `f = 0` on the surface; `f > 0` is the [`Sense::Positive`] half-space and
    /// `f < 0` the [`Sense::Negative`] one. For a `Sphere`/`ZCylinder` the
    /// interior is `f < 0`; for the planes the +axis side is `f > 0`.
    pub fn signed_value(&self, p: Vec3) -> f64 {
        match *self {
            CsgSurface::XPlane { x0 } => p.x - x0,
            CsgSurface::YPlane { y0 } => p.y - y0,
            CsgSurface::ZPlane { z0 } => p.z - z0,
            CsgSurface::Sphere { x0, y0, z0, r } => {
                let dx = p.x - x0;
                let dy = p.y - y0;
                let dz = p.z - z0;
                dx * dx + dy * dy + dz * dz - r * r
            }
            CsgSurface::ZCylinder { x0, y0, r } => {
                let dx = p.x - x0;
                let dy = p.y - y0;
                dx * dx + dy * dy - r * r
            }
            CsgSurface::Plane { a, b, c, d } => a * p.x + b * p.y + c * p.z - d,
        }
    }
}

/// Which side of a [`CsgSurface`] a half-space selects — a local mirror of
/// `outram-mc-libs`'s surface sense.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Sense {
    /// The `f > 0` side (outside a sphere/cylinder; +axis side of a plane).
    Positive,
    /// The `f < 0` side (inside a sphere/cylinder; -axis side of a plane).
    Negative,
}

/// One token of an RPN CSG region — a **local mirror** of `outram-mc-libs`'s
/// `RegionToken` (`Cell.region`).
///
/// The region is evaluated as a stack machine over booleans (see
/// [`CsgDescription::contains`]): a [`RegionToken::Halfspace`] pushes "point is
/// on the chosen side of surface `surface`"; [`RegionToken::Intersection`] /
/// [`RegionToken::Union`] pop two and push their AND / OR;
/// [`RegionToken::Complement`] pops one and pushes its negation.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum RegionToken {
    /// The signed half-space of surface index `surface` on side `sense`.
    Halfspace {
        /// Index into [`CsgDescription::surfaces`].
        surface: usize,
        /// Which side of that surface this half-space is.
        sense: Sense,
    },
    /// Boolean AND of the top two operands (set intersection).
    Intersection,
    /// Boolean OR of the top two operands (set union).
    Union,
    /// Boolean NOT of the top operand (set complement).
    Complement,
}

/// A complete CSG solid: analytic `surfaces` plus an RPN `region` over them.
///
/// A **local mirror** of the geometry `outram-mc-libs` consumes — surfaces
/// (`SurfaceKind`) and a region (`Cell.region`) — so this crate need not depend
/// on it. Produced by [`to_csg_primitive`]. Lengths are dimensionless model
/// space; a length unit (conventionally metres) is assigned at export.
#[derive(Debug, Clone, PartialEq)]
pub struct CsgDescription {
    /// The analytic surfaces, referenced by index from `region`.
    pub surfaces: Vec<CsgSurface>,
    /// The region as an RPN token stream over `surfaces` (see [`RegionToken`]).
    pub region: Vec<RegionToken>,
}

impl CsgDescription {
    /// Test whether point `p` lies inside this CSG region.
    ///
    /// Evaluates the RPN [`CsgDescription::region`] as a boolean stack machine:
    /// each half-space contributes whether `p` is on its chosen side, and the
    /// intersection/union/complement operators combine them. Returns `false`
    /// for a malformed (stack-underflowing or non-singleton) region rather than
    /// panicking. On a surface (`f == 0`) a point counts as *not* in the
    /// positive half-space and *not* in the negative one (strict inequalities).
    pub fn contains(&self, p: Vec3) -> bool {
        let mut stack: Vec<bool> = Vec::new();
        for tok in &self.region {
            match *tok {
                RegionToken::Halfspace { surface, sense } => {
                    let Some(surf) = self.surfaces.get(surface) else {
                        return false;
                    };
                    let f = surf.signed_value(p);
                    let inside = match sense {
                        Sense::Positive => f > 0.0,
                        Sense::Negative => f < 0.0,
                    };
                    stack.push(inside);
                }
                RegionToken::Intersection => {
                    let (Some(b), Some(a)) = (stack.pop(), stack.pop()) else {
                        return false;
                    };
                    stack.push(a && b);
                }
                RegionToken::Union => {
                    let (Some(b), Some(a)) = (stack.pop(), stack.pop()) else {
                        return false;
                    };
                    stack.push(a || b);
                }
                RegionToken::Complement => {
                    let Some(a) = stack.pop() else {
                        return false;
                    };
                    stack.push(!a);
                }
            }
        }
        stack.len() == 1 && stack[0]
    }
}

/// Fit `mesh` to an analytic CSG solid consumable by `outram-mc-libs`.
///
/// Recognises the [`crate::primitives`] shapes it can express exactly and emits
/// the matching [`CsgDescription`] (local mirror types — no dependency on
/// `outram-mc-libs`):
///
/// - **axis-aligned cube/box** (8 vertices, 6 quad faces, all normals ±X/±Y/±Z,
///   every vertex on the bounding box) → six planes at the box bounds, region =
///   the intersection of the six inward half-spaces (the box interior);
/// - **uv-sphere** (vertices equidistant from their centroid, and vertex/face
///   counts matching a `2 + (rings-1)*segments` / `rings*segments` uv-sphere) →
///   one `Sphere` at the fitted centre/radius, region = its interior
///   (`Negative` half-space);
/// - **Z-axis cylinder** (a [`crate::primitives::cylinder`]: `2*segments`
///   vertices, `segments` side quads + two `segments`-gon caps, side vertices at
///   a constant radius about a Z-parallel axis) → one `ZCylinder` intersected
///   with two `ZPlane` caps;
/// - **any other convex closed polyhedron** → the **faceted convex** route: one
///   general [`CsgSurface::Plane`] per face (outward normal), region = the
///   intersection of every inward (`Negative`) half-space. This is exact for a
///   convex solid — e.g. a rotated box or a convex boolean result — because a
///   convex polyhedron *is* the intersection of its face half-spaces.
///
/// A **non-convex** mesh cannot be written as a single half-space intersection,
/// so it returns [`ExportError::NotImplemented`] here; use [`to_faceted_solid`]
/// for the DAGMC-style boundary representation of an arbitrary (non-convex)
/// solid.
///
/// Lengths are dimensionless model space; a length unit (conventionally metres)
/// is assigned when the description reaches the transport solver.
pub fn to_csg_primitive(mesh: &Mesh) -> Result<CsgDescription, ExportError> {
    if let Some(desc) = try_fit_box(mesh) {
        return Ok(desc);
    }
    if let Some(desc) = try_fit_sphere(mesh) {
        return Ok(desc);
    }
    if let Some(desc) = try_fit_cylinder(mesh) {
        return Ok(desc);
    }
    if let Some(desc) = try_fit_convex_faceted(mesh) {
        return Ok(desc);
    }
    Err(ExportError::NotImplemented(
        "CSG primitive fitting recognises an axis-aligned box, a uv-sphere, a \
         Z-axis cylinder, or any convex polyhedron (faceted); a non-convex mesh \
         is not a half-space intersection — use to_faceted_solid for its \
         DAGMC-style boundary representation instead",
    ))
}

/// Tolerance for recognising axis-alignment and coincident bounds/radii.
///
/// Absolute in model-space units; the fitters scale it by the shape's extent so
/// a large primitive is not held to a tighter-than-`f64` relative bound.
const FIT_TOL: f64 = 1e-9;

/// Is `n` (assumed near unit length) aligned with a coordinate axis?
fn is_axis_aligned(n: Vec3) -> bool {
    let (ax, ay, az) = (n.x.abs(), n.y.abs(), n.z.abs());
    // Exactly one component ~1, the other two ~0.
    let near_one = |v: f64| (v - 1.0).abs() < 1e-6;
    let near_zero = |v: f64| v.abs() < 1e-6;
    (near_one(ax) && near_zero(ay) && near_zero(az))
        || (near_zero(ax) && near_one(ay) && near_zero(az))
        || (near_zero(ax) && near_zero(ay) && near_one(az))
}

/// Try to recognise `mesh` as an axis-aligned box; emit its 6-plane CSG.
fn try_fit_box(mesh: &Mesh) -> Option<CsgDescription> {
    if mesh.vertex_count() != 8 || mesh.face_count() != 6 {
        return None;
    }
    // Every face must be a quad with an axis-aligned outward normal.
    for f in 0..6 {
        if mesh.face_vertices(FaceId(f)).len() != 4 {
            return None;
        }
        if !is_axis_aligned(mesh.face_normal(FaceId(f))) {
            return None;
        }
    }

    let ps = mesh.positions();
    let (mut xmin, mut xmax) = (f64::INFINITY, f64::NEG_INFINITY);
    let (mut ymin, mut ymax) = (f64::INFINITY, f64::NEG_INFINITY);
    let (mut zmin, mut zmax) = (f64::INFINITY, f64::NEG_INFINITY);
    for p in &ps {
        xmin = xmin.min(p.x);
        xmax = xmax.max(p.x);
        ymin = ymin.min(p.y);
        ymax = ymax.max(p.y);
        zmin = zmin.min(p.z);
        zmax = zmax.max(p.z);
    }
    let extent = (xmax - xmin).max(ymax - ymin).max(zmax - zmin);
    if extent <= 0.0 {
        return None;
    }
    let tol = FIT_TOL * (1.0 + extent);

    // Every vertex must sit on a min or max bound of each axis — i.e. the eight
    // vertices really are the corners of the axis-aligned bounding box.
    let on = |v: f64, a: f64, b: f64| (v - a).abs() < tol || (v - b).abs() < tol;
    for p in &ps {
        if !on(p.x, xmin, xmax) || !on(p.y, ymin, ymax) || !on(p.z, zmin, zmax) {
            return None;
        }
    }

    // Six planes at the bounds; region = intersection of the six inward
    // half-spaces (interior of the box):
    //   x > xmin (Positive) and x < xmax (Negative), likewise y, z.
    let surfaces = vec![
        CsgSurface::XPlane { x0: xmin }, // 0
        CsgSurface::XPlane { x0: xmax }, // 1
        CsgSurface::YPlane { y0: ymin }, // 2
        CsgSurface::YPlane { y0: ymax }, // 3
        CsgSurface::ZPlane { z0: zmin }, // 4
        CsgSurface::ZPlane { z0: zmax }, // 5
    ];
    let region = vec![
        RegionToken::Halfspace { surface: 0, sense: Sense::Positive },
        RegionToken::Halfspace { surface: 1, sense: Sense::Negative },
        RegionToken::Intersection,
        RegionToken::Halfspace { surface: 2, sense: Sense::Positive },
        RegionToken::Intersection,
        RegionToken::Halfspace { surface: 3, sense: Sense::Negative },
        RegionToken::Intersection,
        RegionToken::Halfspace { surface: 4, sense: Sense::Positive },
        RegionToken::Intersection,
        RegionToken::Halfspace { surface: 5, sense: Sense::Negative },
        RegionToken::Intersection,
    ];
    Some(CsgDescription { surfaces, region })
}

/// Try to recognise `mesh` as a uv-sphere; emit its single-`Sphere` CSG.
fn try_fit_sphere(mesh: &Mesh) -> Option<CsgDescription> {
    let v = mesh.vertex_count();
    let f = mesh.face_count();

    // uv-sphere topology: F = rings*segments, V = 2 + (rings-1)*segments.
    // => segments = F - V + 2, rings = F / segments. Guard usize arithmetic.
    if v > f + 2 {
        return None;
    }
    let segments = f + 2 - v;
    if segments < 3 || !f.is_multiple_of(segments) {
        return None;
    }
    let rings = f / segments;
    if rings < 2 || 2 + (rings - 1) * segments != v {
        return None;
    }

    // Fit centre = centroid of vertices, radius = mean distance to it.
    let ps = mesh.positions();
    if ps.is_empty() {
        return None;
    }
    let mut c = Vec3::ZERO;
    for p in &ps {
        c = c.add(*p);
    }
    let centre = c.scale(1.0 / ps.len() as f64);

    let mut r_sum = 0.0;
    for p in &ps {
        r_sum += p.sub(centre).length();
    }
    let r = r_sum / ps.len() as f64;
    if r <= 0.0 {
        return None;
    }

    // Every vertex must lie within tolerance of the fitted radius.
    let tol = FIT_TOL * (1.0 + r);
    for p in &ps {
        if (p.sub(centre).length() - r).abs() > tol {
            return None;
        }
    }

    let surfaces = vec![CsgSurface::Sphere {
        x0: centre.x,
        y0: centre.y,
        z0: centre.z,
        r,
    }];
    // Region = interior of the sphere (the Negative half-space).
    let region = vec![RegionToken::Halfspace { surface: 0, sense: Sense::Negative }];
    Some(CsgDescription { surfaces, region })
}

/// Try to recognise `mesh` as a Z-axis [`crate::primitives::cylinder`]; emit its
/// `ZCylinder` ∩ two-`ZPlane`-cap CSG.
///
/// Recognition (geometric, not construction-order-dependent): the mesh must
/// have `2*segments` vertices and `segments + 2` faces (so `segments = F - 2`),
/// with every vertex lying on one of exactly two Z-planes (`zmin`/`zmax`, evenly
/// split) at a constant radius `r` about a common Z-parallel axis
/// `(x0, y0)` — the axis being the mean of the vertex `(x, y)`. A 4-segment
/// cylinder is an axis-aligned box and is caught earlier by [`try_fit_box`], so
/// this handles `segments >= 5` in practice.
fn try_fit_cylinder(mesh: &Mesh) -> Option<CsgDescription> {
    let f = mesh.face_count();
    if f < 5 {
        return None; // segments = f - 2 >= 3
    }
    let segments = f - 2;
    if segments < 3 || mesh.vertex_count() != 2 * segments {
        return None;
    }

    let ps = mesh.positions();
    if ps.is_empty() {
        return None;
    }
    let (mut zmin, mut zmax) = (f64::INFINITY, f64::NEG_INFINITY);
    let (mut sx, mut sy) = (0.0, 0.0);
    for p in &ps {
        zmin = zmin.min(p.z);
        zmax = zmax.max(p.z);
        sx += p.x;
        sy += p.y;
    }
    let height = zmax - zmin;
    if height <= 0.0 {
        return None;
    }
    let (x0, y0) = (sx / ps.len() as f64, sy / ps.len() as f64);

    // Radius = mean in-plane distance from the axis.
    let radial = |p: &Vec3| ((p.x - x0).powi(2) + (p.y - y0).powi(2)).sqrt();
    let r = ps.iter().map(radial).sum::<f64>() / ps.len() as f64;
    if r <= 0.0 {
        return None;
    }

    let tol = FIT_TOL * (1.0 + r.max(height));
    let (mut n_bottom, mut n_top) = (0usize, 0usize);
    for p in &ps {
        if (radial(p) - r).abs() > tol {
            return None; // not a constant-radius surface of revolution
        }
        if (p.z - zmin).abs() <= tol {
            n_bottom += 1;
        } else if (p.z - zmax).abs() <= tol {
            n_top += 1;
        } else {
            return None; // a vertex off both caps -> not this cylinder
        }
    }
    if n_bottom != segments || n_top != segments {
        return None;
    }

    // Region = inside the cylinder AND between the two caps:
    //   f_cyl < 0 (Negative) and z > zmin (Positive) and z < zmax (Negative).
    let surfaces = vec![
        CsgSurface::ZCylinder { x0, y0, r }, // 0
        CsgSurface::ZPlane { z0: zmin },     // 1
        CsgSurface::ZPlane { z0: zmax },     // 2
    ];
    let region = vec![
        RegionToken::Halfspace { surface: 0, sense: Sense::Negative },
        RegionToken::Halfspace { surface: 1, sense: Sense::Positive },
        RegionToken::Intersection,
        RegionToken::Halfspace { surface: 2, sense: Sense::Negative },
        RegionToken::Intersection,
    ];
    Some(CsgDescription { surfaces, region })
}

/// Try to express a **convex** closed `mesh` as the intersection of its face
/// half-spaces (the faceted convex route).
///
/// A convex polyhedron equals the intersection of the inward half-spaces of all
/// its face planes, so this is *exact* for any convex solid — a rotated box, an
/// octahedron, or a convex boolean result (e.g. two overlapping boxes'
/// intersection). Returns `None` if the mesh is not convex (some vertex lies on
/// the outward side of a face plane by more than a scaled tolerance) or has a
/// degenerate face. Coincident face planes (e.g. from a triangulated flat face)
/// are de-duplicated so the emitted description stays compact.
///
/// Correct outward face normals are assumed (as [`crate::primitives`] and the
/// boolean output produce). A non-convex mesh cannot be a single half-space
/// intersection — see [`to_faceted_solid`] for its DAGMC-style representation.
fn try_fit_convex_faceted(mesh: &Mesh) -> Option<CsgDescription> {
    if mesh.face_count() < 4 || mesh.vertex_count() < 4 {
        return None;
    }
    let ps = mesh.positions();

    // Size scale for the convexity tolerance (bounding-box diagonal).
    let mut lo = ps[0];
    let mut hi = ps[0];
    for &p in &ps {
        lo = Vec3::new(lo.x.min(p.x), lo.y.min(p.y), lo.z.min(p.z));
        hi = Vec3::new(hi.x.max(p.x), hi.y.max(p.y), hi.z.max(p.z));
    }
    let extent = hi.sub(lo).length();
    let tol = FIT_TOL * (1.0 + extent);

    let mut surfaces: Vec<CsgSurface> = Vec::new();
    for fi in 0..mesh.face_count() {
        let n = mesh.face_normal(FaceId(fi));
        if n == Vec3::ZERO {
            return None; // degenerate face — cannot form a plane
        }
        let d = n.dot(mesh.face_centroid(FaceId(fi)));
        // Convexity: every vertex must be on the inner (n.p - d <= tol) side.
        for &p in &ps {
            if n.dot(p) - d > tol {
                return None;
            }
        }
        // De-duplicate coincident planes (same normal and offset within tol).
        let plane = CsgSurface::Plane { a: n.x, b: n.y, c: n.z, d };
        let dup = surfaces.iter().any(|s| match (*s, plane) {
            (
                CsgSurface::Plane { a: a1, b: b1, c: c1, d: d1 },
                CsgSurface::Plane { a: a2, b: b2, c: c2, d: d2 },
            ) => {
                (a1 - a2).abs() < 1e-9
                    && (b1 - b2).abs() < 1e-9
                    && (c1 - c2).abs() < 1e-9
                    && (d1 - d2).abs() < tol
            }
            _ => false,
        });
        if !dup {
            surfaces.push(plane);
        }
    }

    if surfaces.len() < 4 {
        return None; // fewer than four distinct planes cannot bound a volume
    }

    // Region = intersection of every inward (Negative) half-space.
    let mut region = vec![RegionToken::Halfspace { surface: 0, sense: Sense::Negative }];
    for k in 1..surfaces.len() {
        region.push(RegionToken::Halfspace { surface: k, sense: Sense::Negative });
        region.push(RegionToken::Intersection);
    }
    Some(CsgDescription { surfaces, region })
}

// ===========================================================================
// 3. Faceted (DAGMC-style) boundary export — the non-convex route
// ===========================================================================

/// A solid represented **directly by its triangulated boundary** — the
/// DAGMC-style representation for meshes that are *not* a recognised primitive
/// and are *not* convex (so they cannot be a CSG half-space intersection).
///
/// This is the honest export route for an arbitrary boolean result: rather than
/// forcing it into analytic surfaces, the solid *is* its outward-oriented
/// triangle surface, and inside/outside is decided by the **generalized winding
/// number** (the same test [`crate::boolean_classify`] uses and DAGMC's
/// point-in-volume query embodies). A local mirror — `outram-mc-libs` would
/// consume the triangle soup for ray-traced surface tracking; wiring that real
/// dependency is deferred with the rest of this module.
///
/// Coordinates are dimensionless model space; a length unit (conventionally
/// metres) is assigned when the solid reaches the transport solver.
#[derive(Debug, Clone, Default)]
pub struct FacetedSolid {
    /// Vertex positions, indexed by [`FacetedSolid::triangles`].
    pub positions: Vec<Vec3>,
    /// Outward-wound triangles (each a corner-index triple into
    /// [`FacetedSolid::positions`]). Outward orientation is enforced at
    /// construction so face normals point out of the solid.
    pub triangles: Vec<[u32; 3]>,
}

impl FacetedSolid {
    /// Number of boundary triangles.
    pub fn triangle_count(&self) -> usize {
        self.triangles.len()
    }

    /// Test whether point `p` is inside the solid, by the **generalized winding
    /// number** of the boundary triangles at `p` (`|w| > 0.5` ⇒ inside).
    ///
    /// This mirrors [`crate::boolean_classify::winding_number`] (Van
    /// Oosterom–Strackee signed solid angle per triangle, summed and divided by
    /// `4*pi`) evaluated directly on the stored triangles. It is robust to the
    /// solid's *global* winding direction (the magnitude test ignores sign), and
    /// well-defined for the closed, manifold, outward-oriented boundary this
    /// type stores. A point essentially *on* the surface is a hard case (see the
    /// classifier's limitations) and may fall either way.
    pub fn contains(&self, p: Vec3) -> bool {
        const COINCIDENT_EPS: f64 = 1e-12;
        let mut solid_angle = 0.0;
        for tri in &self.triangles {
            let ra = self.positions[tri[0] as usize].sub(p);
            let rb = self.positions[tri[1] as usize].sub(p);
            let rc = self.positions[tri[2] as usize].sub(p);
            let (la, lb, lc) = (ra.length(), rb.length(), rc.length());
            if la < COINCIDENT_EPS || lb < COINCIDENT_EPS || lc < COINCIDENT_EPS {
                continue;
            }
            let numerator = ra.dot(rb.cross(rc));
            let denominator =
                la * lb * lc + ra.dot(rb) * lc + rb.dot(rc) * la + rc.dot(ra) * lb;
            solid_angle += 2.0 * numerator.atan2(denominator);
        }
        (solid_angle / (4.0 * std::f64::consts::PI)).abs() > 0.5
    }
}

/// Build the DAGMC-style [`FacetedSolid`] boundary of `mesh`: fan-triangulate
/// every face and orient the triangles **outward** (positive enclosed volume).
///
/// Works for any closed mesh, convex or not — this is the fallback the analytic
/// [`to_csg_primitive`] fitters do not cover. Outward orientation is enforced so
/// that a downstream consumer using face normals (not just the sign-agnostic
/// [`FacetedSolid::contains`]) sees them pointing out of the solid.
pub fn to_faceted_solid(mesh: &Mesh) -> FacetedSolid {
    let tris = triangulate(mesh); // IndexedTriangles (fan-triangulated)
    let positions = tris.positions;
    let mut triangles: Vec<[u32; 3]> = tris
        .indices
        .chunks_exact(3)
        .map(|c| [c[0], c[1], c[2]])
        .collect();

    // Orient outward: flip all if the net signed volume is negative.
    let mut vol6 = 0.0;
    for t in &triangles {
        let v0 = positions[t[0] as usize];
        let v1 = positions[t[1] as usize];
        let v2 = positions[t[2] as usize];
        vol6 += v0.dot(v1.cross(v2));
    }
    if vol6 < 0.0 {
        for t in &mut triangles {
            t.swap(1, 2);
        }
    }

    FacetedSolid { positions, triangles }
}

// ===========================================================================
// 4. Real-type bridges to the OUTRAM PARK solver crates (feature-gated)
//
// The exporters above emit self-contained text / local-mirror types so the
// default build stays light and Android-friendly. The two functions below hand
// geometry to the ACTUAL solver crates' types, behind opt-in cargo features
// (`foam-export`, `mc-export`) so neither crate is a hard dependency of the
// authoring frontend.
// ===========================================================================

/// Convert `mesh` to a real `outram-foam-basic-lib` **polyMesh boundary
/// description** (`io::poly_mesh::PolyMesh`) — the CFD export bridge.
///
/// (Feature `foam-export`.) This is the real-type counterpart of
/// [`to_polymesh_text`]: it builds the actual `PolyMesh` connectivity type that
/// `outram-foam-basic-lib`'s OpenFOAM reader/writer round-trips, so a caller can
/// `poly.write(dir)` it or hand it to a volume mesher.
///
/// Because an authored mesh is a **boundary surface with no cells**, every face
/// is a boundary face owned by a single dummy cell `0` (`neighbour = None`),
/// `n_internal_faces = 0`, and all faces sit in one `authoredSurface`
/// `PatchKind::Patch` patch — exactly the dummy-cell convention
/// [`to_polymesh_text`] documents. It is a boundary patch for a volume mesher
/// (blockMesh / snappyHexMesh) to fill, **not** a solve-ready `FvMesh`; do not
/// call `to_fv_mesh` on it expecting real cell volumes.
///
/// Coordinates are copied verbatim as dimensionless model-space `f64`s into
/// `Vector3`; the caller assigns a length unit (conventionally metres).
#[cfg(feature = "foam-export")]
pub fn to_poly_mesh(mesh: &Mesh) -> outram_foam_basic_lib::io::poly_mesh::PolyMesh {
    use outram_foam_basic_lib::io::poly_mesh::{MeshFace, PolyMesh};
    use outram_foam_basic_lib::mesh::{BoundaryPatch, PatchKind};
    use outram_foam_basic_lib::primitives::Vector3;

    let points: Vec<Vector3> = mesh
        .positions()
        .iter()
        .map(|p| Vector3::new(p.x, p.y, p.z))
        .collect();

    let mut faces: Vec<MeshFace> = Vec::with_capacity(mesh.face_count());
    for f in 0..mesh.face_count() {
        let verts: Vec<usize> = mesh.face_vertices(FaceId(f)).iter().map(|v| v.0).collect();
        // Boundary face: owned by the dummy cell 0, no neighbour cell.
        faces.push(MeshFace { verts, owner: 0, neighbour: None });
    }

    let n_faces = faces.len();
    let patches = vec![BoundaryPatch::new("authoredSurface", 0, n_faces, PatchKind::Patch)];

    PolyMesh {
        points,
        faces,
        n_internal_faces: 0,
        n_cells: 1, // single dummy cell (a surface has no real cells)
        patches,
    }
}

/// Build an outram-blender [`Mesh`] from a real `outram-foam-basic-lib`
/// [`PolyMesh`](outram_foam_basic_lib::io::poly_mesh::PolyMesh) — the CFD
/// **import** bridge, the inverse of [`to_poly_mesh`].
///
/// (Feature `foam-export` — the same feature that gates the foam real-type
/// bridge in both directions.) Combined with
/// `outram_foam_basic_lib::io::poly_mesh::PolyMesh::read(dir)`, this reads an
/// OpenFOAM `constant/polyMesh` directory straight back into an authorable
/// surface mesh:
///
/// ```no_run
/// use outram_blender::export::from_poly_mesh;
/// use outram_foam_basic_lib::io::poly_mesh::PolyMesh;
///
/// let poly = PolyMesh::read("case/constant/polyMesh").expect("read polyMesh");
/// let mesh = from_poly_mesh(&poly);
/// ```
///
/// # What is and isn't reconstructed
///
/// A polyMesh is a **volume** description (points + faces + owner/neighbour +
/// cells); an outram-blender [`Mesh`] is a **boundary surface**. This
/// conversion keeps every polyMesh face as a mesh face — it rebuilds the full
/// face soup (internal *and* boundary faces), not the cell structure. For a
/// mesh authored/exported by [`to_poly_mesh`]/[`write_polymesh`] (all-boundary,
/// single dummy cell) the write→read round-trip is exact. Face winding follows
/// the polyMesh face vertex order; run
/// [`crate::recalc_normals::recalculate_normals`] if a consistent outward
/// orientation is required. Faces with fewer than three vertices (malformed
/// input) are skipped rather than panicking.
#[cfg(feature = "foam-export")]
pub fn from_poly_mesh(poly: &outram_foam_basic_lib::io::poly_mesh::PolyMesh) -> Mesh {
    let positions: Vec<crate::math::Vec3> = poly
        .points
        .iter()
        .map(|p| crate::math::Vec3::new(p.x, p.y, p.z))
        .collect();
    let faces: Vec<Vec<usize>> = poly
        .faces
        .iter()
        .filter(|f| f.verts.len() >= 3)
        .map(|f| f.verts.clone())
        .collect();
    Mesh::from_polygons(&positions, &faces)
}

/// Convert `mesh` to a real `outram-mc-libs` CSG `Geometry` — the Monte Carlo
/// export bridge.
///
/// (Feature `mc-export`.) Fits `mesh` to analytic CSG via [`to_csg_primitive`],
/// then maps the local-mirror surfaces/region onto `outram-mc-libs`'s real
/// `SurfaceKind` / `RegionToken` and wraps them in a single-cell `Geometry`:
///
/// - each [`CsgSurface`] → the matching `SurfaceKind` variant, tagged
///   `BoundaryType::Transmissive` (an interior surface);
/// - [`Sense::Negative`] → `HalfSpaceSense::Inside` (evaluate `< 0`),
///   [`Sense::Positive`] → `HalfSpaceSense::Outside` (evaluate `> 0`);
/// - the region RPN maps 1:1 onto `outram-mc-libs`'s `RegionToken`;
/// - the region becomes one `Cell` (id `1`) filled `CellFill::Void`, in a single
///   root `Universe`. **The `Void` fill is a placeholder** — the caller assigns
///   the real material/fill and temperature; this bridge exports *geometry*
///   only.
///
/// # Errors
///
/// Returns [`ExportError::NotImplemented`] if `mesh` is not a fittable primitive
/// (propagated from [`to_csg_primitive`]) **or** if the fitted CSG contains a
/// general [`CsgSurface::Plane`] — `outram-mc-libs` has no general-plane surface
/// (only `XPlane`/`YPlane`/`ZPlane`/`Sphere`/`ZCylinder`), so a convex-faceted
/// CSG (from an arbitrary convex polyhedron) cannot be represented. Box, sphere,
/// and Z-cylinder primitives export exactly.
#[cfg(feature = "mc-export")]
pub fn to_mc_geometry(mesh: &Mesh) -> Result<outram_mc_libs::prelude::Geometry, ExportError> {
    use outram_mc_libs::geometry::position::Position;
    use outram_mc_libs::geometry::surface::{Sphere, XPlane, YPlane, ZCylinder, ZPlane};
    use outram_mc_libs::prelude::{
        BoundaryType, Cell, CellFill, Geometry, HalfSpaceSense, SurfaceKind, Universe,
    };
    use outram_mc_libs::prelude::RegionToken as McToken;

    let desc = to_csg_primitive(mesh)?;

    let bc = BoundaryType::Transmissive;
    let mut surfaces: Vec<SurfaceKind> = Vec::with_capacity(desc.surfaces.len());
    for s in &desc.surfaces {
        let kind = match *s {
            CsgSurface::XPlane { x0 } => SurfaceKind::XPlane(XPlane { x0, bc }),
            CsgSurface::YPlane { y0 } => SurfaceKind::YPlane(YPlane { y0, bc }),
            CsgSurface::ZPlane { z0 } => SurfaceKind::ZPlane(ZPlane { z0, bc }),
            CsgSurface::Sphere { x0, y0, z0, r } => {
                SurfaceKind::Sphere(Sphere { x0, y0, z0, r, bc })
            }
            CsgSurface::ZCylinder { x0, y0, r } => {
                SurfaceKind::ZCylinder(ZCylinder { x0, y0, r, bc })
            }
            CsgSurface::Plane { .. } => {
                return Err(ExportError::NotImplemented(
                    "outram-mc-libs has no general Plane surface; a convex-faceted CSG (from \
                     an arbitrary convex polyhedron) cannot be exported to Monte Carlo -- only \
                     box / sphere / Z-cylinder primitives map exactly",
                ));
            }
        };
        surfaces.push(kind);
    }

    let region: Vec<McToken> = desc
        .region
        .iter()
        .map(|t| match *t {
            RegionToken::Halfspace { surface, sense } => McToken::HalfSpace {
                surface_idx: surface,
                sense: match sense {
                    Sense::Negative => HalfSpaceSense::Inside,
                    Sense::Positive => HalfSpaceSense::Outside,
                },
            },
            RegionToken::Intersection => McToken::Intersection,
            RegionToken::Union => McToken::Union,
            RegionToken::Complement => McToken::Complement,
        })
        .collect();

    // One geometry-only cell; the caller reassigns the fill/material/temperature.
    let cell = Cell {
        id: 1,
        region,
        fill: CellFill::Void,
        temperature: 293.6,
        translation: Position::ZERO,
    };
    Ok(Geometry {
        surfaces,
        cells: vec![cell],
        universes: vec![Universe { id: 0, cell_indices: vec![0] }],
        lattices: vec![],
        root_universe: 0,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::primitives;

    /// Parse the first stand-alone unsigned-integer line of an OpenFOAM list
    /// body — the element count that precedes the `(` in a `points`/`faces`
    /// list. Used to round-trip the emitted counts out of the text.
    fn first_int(s: &str) -> usize {
        s.lines()
            .find_map(|l| l.trim().parse::<usize>().ok())
            .expect("no stand-alone integer count line found")
    }

    #[test]
    fn triangulate_cube_gives_12_triangles() {
        // A cube has 6 quads; fan-triangulating each quad yields 2 triangles.
        let tris = triangulate(&primitives::cube(1.0));
        assert_eq!(tris.positions.len(), 8);
        assert_eq!(tris.triangle_count(), 12);
        // Every index is in range.
        assert!(tris.indices.iter().all(|&i| (i as usize) < tris.positions.len()));
    }

    /// polyMesh export round-trip.
    ///
    /// Methodology: export `cube(1.0)` (8 points, 6 quad faces) to the five
    /// polyMesh strings and read the element counts back out of the `points`
    /// and `faces` list bodies; check each file carries its own `object` name.
    ///
    /// Results (deterministic, by construction): the `points` body count parses
    /// to **8** and the `faces` body count to **6**; the strings contain
    /// `object      points` / `object      faces` respectively.
    #[test]
    fn polymesh_text_round_trips_counts() {
        let t = to_polymesh_text(&primitives::cube(1.0));

        assert_eq!(first_int(&t.points), 8, "points body must list 8 points");
        assert_eq!(first_int(&t.faces), 6, "faces body must list 6 faces");

        assert!(t.points.contains("object      points"));
        assert!(t.faces.contains("object      faces"));
        assert!(t.owner.contains("object      owner"));
        assert!(t.neighbour.contains("object      neighbour"));
        assert!(t.boundary.contains("object      boundary"));

        // Owner: one label per face, all dummy cell 0; boundary patch present.
        assert_eq!(first_int(&t.owner), 6, "owner lists one label per face");
        assert!(t.owner.contains("nCells:1"));
        assert!(t.boundary.contains("authoredSurface"));
        assert!(t.boundary.contains("nFaces          6"));
        // Neighbour empty: no internal faces on a boundary surface.
        assert_eq!(first_int(&t.neighbour), 0);
    }

    /// CSG fit of an axis-aligned cube.
    ///
    /// Methodology: fit `cube(2.0)` (spans -1..1 on each axis) and inspect the
    /// emitted surfaces and RPN region.
    ///
    /// Results (by construction): **6** surfaces — three axis planes at each of
    /// the measured offsets **{xmin,ymin,zmin} = -1.0** and
    /// **{xmax,ymax,zmax} = +1.0**; the region is **6 half-spaces + 5
    /// intersections** and classifies the origin as inside and (2,2,2) as
    /// outside.
    #[test]
    fn csg_fit_cube() {
        let desc = to_csg_primitive(&primitives::cube(2.0)).expect("cube must fit");

        assert_eq!(desc.surfaces.len(), 6, "a box is six planes");

        // Measured plane offsets: min bounds -1.0, max bounds +1.0.
        let offsets: Vec<f64> = desc
            .surfaces
            .iter()
            .map(|s| match *s {
                CsgSurface::XPlane { x0 } => x0,
                CsgSurface::YPlane { y0 } => y0,
                CsgSurface::ZPlane { z0 } => z0,
                _ => panic!("box surfaces must all be axis planes"),
            })
            .collect();
        assert_eq!(offsets, vec![-1.0, 1.0, -1.0, 1.0, -1.0, 1.0]);

        // Region shape: six half-spaces combined by five intersections.
        let halfspaces = desc
            .region
            .iter()
            .filter(|t| matches!(t, RegionToken::Halfspace { .. }))
            .count();
        let intersections = desc
            .region
            .iter()
            .filter(|t| matches!(t, RegionToken::Intersection))
            .count();
        assert_eq!(halfspaces, 6);
        assert_eq!(intersections, 5);

        // "Inside" is really the interior.
        assert!(desc.contains(Vec3::ZERO), "origin is inside the box");
        assert!(
            !desc.contains(Vec3::new(2.0, 2.0, 2.0)),
            "a point beyond the +bounds is outside"
        );
    }

    /// CSG fit of a uv-sphere.
    ///
    /// Methodology: fit `uv_sphere(16, 8, 3.0)`; the fitter recovers the centre
    /// (centroid of vertices) and radius (mean vertex distance), then checks all
    /// vertices lie within `~1e-9*(1+r)` of that radius.
    ///
    /// Results (by construction): **1** `Sphere` surface; measured fitted radius
    /// **= 3.0** to within `1e-6` (every uv-sphere vertex is placed at exactly
    /// `radius` from the origin, so the fit recovers the input); centre at the
    /// origin to within floating-point tolerance; region is the single interior
    /// (`Negative`) half-space, so the centre is inside and a point at radius 4
    /// is outside.
    #[test]
    fn csg_fit_sphere() {
        let desc =
            to_csg_primitive(&primitives::uv_sphere(16, 8, 3.0)).expect("sphere must fit");

        assert_eq!(desc.surfaces.len(), 1, "a sphere is one surface");
        match desc.surfaces[0] {
            CsgSurface::Sphere { x0, y0, z0, r } => {
                assert!((r - 3.0).abs() < 1e-6, "fitted radius {r} != 3.0");
                assert!(x0.abs() < 1e-6 && y0.abs() < 1e-6 && z0.abs() < 1e-6);
            }
            other => panic!("expected a Sphere, got {other:?}"),
        }
        assert_eq!(
            desc.region,
            vec![RegionToken::Halfspace { surface: 0, sense: Sense::Negative }]
        );

        assert!(desc.contains(Vec3::ZERO), "centre is inside the sphere");
        assert!(
            !desc.contains(Vec3::new(4.0, 0.0, 0.0)),
            "a point at radius 4 is outside a radius-3 sphere"
        );
    }

    /// A flat grid is not a closed solid — the fitter must refuse it.
    ///
    /// Methodology: `grid(4, 3, 1.0)` is a disc (20 vertices, 12 faces). Its
    /// counts happen to satisfy the cylinder pre-check (`v = 2*(f-2) = 20`), so
    /// this also guards that the cylinder fitter's zero-height rejection and the
    /// convex fitter's single-plane rejection both fire.
    ///
    /// Result: [`ExportError::NotImplemented`].
    #[test]
    fn csg_grid_is_not_implemented() {
        let g = primitives::grid(4, 3, 1.0);
        assert!(matches!(
            to_csg_primitive(&g),
            Err(ExportError::NotImplemented(_))
        ));
    }

    /// CSG fit of a Z-axis cylinder.
    ///
    /// Methodology: fit `cylinder(16, 1.0, 2.0)` (radius 1, spanning z in
    /// [-1, 1] about the Z axis). The fitter must recover the analytic
    /// `ZCylinder` (the faceted prism approximates a smooth cylinder, exactly as
    /// the sphere fit recovers a `Sphere` from a uv-sphere) plus the two cap
    /// planes.
    ///
    /// Results (asserted below): **3** surfaces — `ZCylinder{0,0,r=1}`,
    /// `ZPlane{-1}`, `ZPlane{+1}`; region classifies the origin inside, a point
    /// above the top cap `(0,0,1.5)` outside, and a point beyond the radius
    /// `(1.5,0,0)` outside.
    #[test]
    fn csg_fit_cylinder() {
        let desc = to_csg_primitive(&primitives::cylinder(16, 1.0, 2.0)).expect("cylinder must fit");
        assert_eq!(desc.surfaces.len(), 3, "cylinder = 1 ZCylinder + 2 cap planes");

        match desc.surfaces[0] {
            CsgSurface::ZCylinder { x0, y0, r } => {
                assert!(x0.abs() < 1e-9 && y0.abs() < 1e-9, "axis off origin: ({x0},{y0})");
                assert!((r - 1.0).abs() < 1e-9, "fitted radius {r} != 1.0");
            }
            other => panic!("surface 0 must be a ZCylinder, got {other:?}"),
        }
        assert!(matches!(desc.surfaces[1], CsgSurface::ZPlane { z0 } if (z0 + 1.0).abs() < 1e-9));
        assert!(matches!(desc.surfaces[2], CsgSurface::ZPlane { z0 } if (z0 - 1.0).abs() < 1e-9));

        assert!(desc.contains(Vec3::ZERO), "axis centre is inside the cylinder");
        assert!(!desc.contains(Vec3::new(0.0, 0.0, 1.5)), "point above the top cap is outside");
        assert!(!desc.contains(Vec3::new(1.5, 0.0, 0.0)), "point beyond the radius is outside");
    }

    /// A **stretched** octahedron (six vertices at `+-1` on x/y but `+-2` on z,
    /// eight triangular faces) as the **faceted convex** route. The stretch
    /// makes the vertices *non*-equidistant from the centroid, so the sphere
    /// fitter (which a regular octahedron — geometrically `uv_sphere(4,2)` —
    /// would match) rejects it; it is not a box or cylinder either, so it falls
    /// through to one general `Plane` per face. Its solid is
    /// `|x| + |y| + |z|/2 <= 1`.
    ///
    /// Results (asserted below): **8** `Plane` surfaces (one per face); the
    /// region classifies the origin inside, `(2,2,2)` outside, and `(0.2,0.2,
    /// 0.2)` inside — i.e. the intersection of the eight face half-spaces is
    /// exactly the stretched-octahedron interior.
    #[test]
    fn csg_fit_octahedron_faceted_convex() {
        // Vertices: +X -X +Y -Y +Z -Z = indices 0..5 (z stretched to +-2).
        let positions = [
            Vec3::new(1.0, 0.0, 0.0),
            Vec3::new(-1.0, 0.0, 0.0),
            Vec3::new(0.0, 1.0, 0.0),
            Vec3::new(0.0, -1.0, 0.0),
            Vec3::new(0.0, 0.0, 2.0),
            Vec3::new(0.0, 0.0, -2.0),
        ];
        // Eight outward-wound triangular faces (top apex +Z, bottom apex -Z).
        let faces = vec![
            vec![0, 2, 4], vec![2, 1, 4], vec![1, 3, 4], vec![3, 0, 4],
            vec![2, 0, 5], vec![1, 2, 5], vec![3, 1, 5], vec![0, 3, 5],
        ];
        let octa = Mesh::from_polygons(&positions, &faces);

        let desc = to_csg_primitive(&octa).expect("convex octahedron must fit");
        assert_eq!(desc.surfaces.len(), 8, "octahedron = eight face planes");
        assert!(
            desc.surfaces.iter().all(|s| matches!(s, CsgSurface::Plane { .. })),
            "faceted convex fit must emit general planes"
        );
        assert!(desc.contains(Vec3::ZERO), "origin is inside the octahedron");
        assert!(!desc.contains(Vec3::new(2.0, 2.0, 2.0)), "|x|+|y|+|z|/2 = 5 is outside");
        assert!(desc.contains(Vec3::new(0.2, 0.2, 0.2)), "0.5 < 1 is inside");
    }

    /// A **non-convex** solid cannot be a half-space intersection, so
    /// [`to_csg_primitive`] refuses it and [`to_faceted_solid`] is the route.
    ///
    /// Methodology: build a concave **L-prism** (L cross-section in `z in
    /// [0,1]`, reentrant notch over `x,y in (1,2)`), which is not any fitted
    /// primitive and not convex. Assert `to_csg_primitive` returns
    /// `NotImplemented`, then that `to_faceted_solid`'s winding `contains`
    /// classifies a solid interior point inside and the reentrant-notch point
    /// (inside the AABB but outside the solid) outside.
    ///
    /// Results (asserted below): `to_csg_primitive` -> `NotImplemented`;
    /// `contains((0.5,0.5,0.5)) = true`; `contains((1.5,1.5,0.5)) = false`;
    /// the boundary fan-triangulates to 20 triangles (two hexagon caps -> 4
    /// each, six quad sides -> 2 each).
    #[test]
    fn faceted_solid_handles_nonconvex_l_prism() {
        // L cross-section (concave at the reentrant corner (1,1)).
        let xy = [(0.0, 0.0), (2.0, 0.0), (2.0, 1.0), (1.0, 1.0), (1.0, 2.0), (0.0, 2.0)];
        let mut positions: Vec<Vec3> = Vec::new();
        for &(x, y) in &xy {
            positions.push(Vec3::new(x, y, 0.0));
        }
        for &(x, y) in &xy {
            positions.push(Vec3::new(x, y, 1.0));
        }
        let n = xy.len();
        let mut faces: Vec<Vec<usize>> = Vec::new();
        faces.push((0..n).rev().collect()); // bottom cap (outward -z)
        faces.push((0..n).map(|i| i + n).collect()); // top cap (outward +z)
        for i in 0..n {
            let j = (i + 1) % n;
            faces.push(vec![i, j, j + n, i + n]); // side quads
        }
        let l_prism = Mesh::from_polygons(&positions, &faces);

        // Not any fitted primitive, and not convex.
        assert!(matches!(to_csg_primitive(&l_prism), Err(ExportError::NotImplemented(_))));

        let solid = to_faceted_solid(&l_prism);
        // Two hexagon caps fan to 4 triangles each + six quad sides to 2 each = 20.
        assert_eq!(solid.triangle_count(), 2 * (n - 2) + 2 * n);
        assert!(solid.contains(Vec3::new(0.5, 0.5, 0.5)), "solid interior point must be inside");
        assert!(
            !solid.contains(Vec3::new(1.5, 1.5, 0.5)),
            "reentrant-notch point (inside AABB, outside solid) must be outside"
        );
    }

    /// Real-type CFD bridge (feature `foam-export`): `to_poly_mesh(cube)` builds
    /// an `outram-foam-basic-lib` `PolyMesh` boundary description — 8 points, 6
    /// boundary faces (no neighbours), one dummy cell, one all-covering patch.
    #[cfg(feature = "foam-export")]
    #[test]
    fn foam_poly_mesh_export_cube() {
        let poly = to_poly_mesh(&primitives::cube(2.0));
        assert_eq!(poly.points.len(), 8, "cube has 8 points");
        assert_eq!(poly.faces.len(), 6, "cube has 6 faces");
        assert_eq!(poly.n_internal_faces, 0, "a surface has no internal faces");
        assert_eq!(poly.n_cells, 1, "single dummy cell");
        assert_eq!(poly.patches.len(), 1, "one boundary patch");
        assert_eq!(poly.patches[0].start, 0);
        assert_eq!(poly.patches[0].size, 6, "patch covers every face");
        assert!(poly.faces.iter().all(|f| f.neighbour.is_none()), "all boundary faces");
        // Real accessors agree.
        assert_eq!(poly.n_points(), 8);
        assert_eq!(poly.n_boundary_faces(), 6);
    }

    /// V&V — in-memory polyMesh round-trip (feature `foam-export`).
    ///
    /// Methodology: export `cube(2.0)` to a real `PolyMesh` with
    /// [`to_poly_mesh`], then read it straight back with [`from_poly_mesh`].
    /// Pass criterion: the reconstructed mesh is the original cube topology.
    /// Result: V = 8, E = 12, F = 6, χ = 2 — exact, because `to_poly_mesh`
    /// preserves every point and face and `from_poly_mesh` is its inverse.
    #[cfg(feature = "foam-export")]
    #[test]
    fn foam_poly_mesh_in_memory_round_trip() {
        let cube = primitives::cube(2.0);
        let poly = to_poly_mesh(&cube);
        let back = from_poly_mesh(&poly);
        assert_eq!(back.vertex_count(), 8, "8 points survive the round-trip");
        assert_eq!(back.edge_count(), 12);
        assert_eq!(back.face_count(), 6);
        assert_eq!(back.euler_characteristic(), 2, "closed genus-0 cube");
    }

    /// V&V — on-disk polyMesh round-trip (feature `foam-export`).
    ///
    /// Methodology: [`write_polymesh`] a `cube(2.0)` to a temp
    /// `constant/polyMesh` directory, read it back with the foam crate's
    /// `PolyMesh::read`, then convert with [`from_poly_mesh`]. This exercises
    /// the *actual* file-based read path a user takes to import an OpenFOAM
    /// mesh. Pass criterion: the imported mesh matches the original cube. Result:
    /// V = 8, E = 12, F = 6, χ = 2.
    #[cfg(feature = "foam-export")]
    #[test]
    fn foam_poly_mesh_disk_round_trip() {
        use outram_foam_basic_lib::io::poly_mesh::PolyMesh;

        let cube = primitives::cube(2.0);
        let dir = std::env::temp_dir()
            .join(format!("outram_blender_polymesh_rt_{}", std::process::id()))
            .join("constant")
            .join("polyMesh");
        write_polymesh(&cube, &dir).expect("write polyMesh to temp dir");

        let poly = PolyMesh::read(&dir).expect("read polyMesh back from disk");
        let back = from_poly_mesh(&poly);
        assert_eq!(back.vertex_count(), 8);
        assert_eq!(back.edge_count(), 12);
        assert_eq!(back.face_count(), 6);
        assert_eq!(back.euler_characteristic(), 2);

        // Clean up the temp tree (best effort).
        if let Some(root) = dir.parent().and_then(|p| p.parent()) {
            let _ = std::fs::remove_dir_all(root);
        }
    }

    /// Real-type Monte Carlo bridge (feature `mc-export`): `to_mc_geometry` maps
    /// a fitted box to six `SurfaceKind` planes intersected in one cell, and a
    /// uv-sphere to a single `Sphere` surface.
    #[cfg(feature = "mc-export")]
    #[test]
    fn mc_geometry_export_box_and_sphere() {
        use outram_mc_libs::prelude::{CellFill, RegionToken as McToken, SurfaceKind};

        let geom = to_mc_geometry(&primitives::cube(2.0)).expect("cube exports to MC CSG");
        assert_eq!(geom.surfaces.len(), 6, "box = six planes");
        assert!(
            geom.surfaces.iter().all(|s| matches!(
                s,
                SurfaceKind::XPlane(_) | SurfaceKind::YPlane(_) | SurfaceKind::ZPlane(_)
            )),
            "box surfaces must all be axis planes"
        );
        assert_eq!(geom.cells.len(), 1);
        assert!(matches!(geom.cells[0].fill, CellFill::Void), "geometry-only cell is Void");
        let halfspaces = geom.cells[0]
            .region
            .iter()
            .filter(|t| matches!(t, McToken::HalfSpace { .. }))
            .count();
        let intersections = geom.cells[0]
            .region
            .iter()
            .filter(|t| matches!(t, McToken::Intersection))
            .count();
        assert_eq!(halfspaces, 6, "six half-spaces");
        assert_eq!(intersections, 5, "combined by five intersections");

        let sph = to_mc_geometry(&primitives::uv_sphere(16, 8, 3.0)).expect("sphere exports");
        assert_eq!(sph.surfaces.len(), 1);
        assert!(matches!(sph.surfaces[0], SurfaceKind::Sphere(_)));
    }

    /// The MC bridge honestly refuses a convex-faceted CSG: `outram-mc-libs` has
    /// no general-plane surface, so a stretched octahedron (which fits to general
    /// `CsgSurface::Plane`s) returns `NotImplemented` rather than a wrong mapping.
    #[cfg(feature = "mc-export")]
    #[test]
    fn mc_geometry_rejects_convex_faceted_plane() {
        let positions = [
            Vec3::new(1.0, 0.0, 0.0),
            Vec3::new(-1.0, 0.0, 0.0),
            Vec3::new(0.0, 1.0, 0.0),
            Vec3::new(0.0, -1.0, 0.0),
            Vec3::new(0.0, 0.0, 2.0),
            Vec3::new(0.0, 0.0, -2.0),
        ];
        let faces = vec![
            vec![0, 2, 4], vec![2, 1, 4], vec![1, 3, 4], vec![3, 0, 4],
            vec![2, 0, 5], vec![1, 2, 5], vec![3, 1, 5], vec![0, 3, 5],
        ];
        let octa = Mesh::from_polygons(&positions, &faces);
        assert!(matches!(to_mc_geometry(&octa), Err(ExportError::NotImplemented(_))));
    }

    /// [`to_faceted_solid`] orients outward regardless of the input winding, so
    /// its `contains` works on the inward-wound-or-not input. Methodology: a
    /// `cube(2.0)` (already outward) round-trips to 12 outward triangles whose
    /// signed volume is `+8`; `contains` agrees with the box interior.
    #[test]
    fn faceted_solid_cube_is_outward_and_contains() {
        let solid = to_faceted_solid(&primitives::cube(2.0));
        assert_eq!(solid.triangle_count(), 12);
        // Signed volume of the faceted triangles = +8 (outward-oriented).
        let mut vol6 = 0.0;
        for t in &solid.triangles {
            let (a, b, c) = (
                solid.positions[t[0] as usize],
                solid.positions[t[1] as usize],
                solid.positions[t[2] as usize],
            );
            vol6 += a.dot(b.cross(c));
        }
        assert!((vol6 / 6.0 - 8.0).abs() < 1e-9, "faceted cube volume {} != 8", vol6 / 6.0);
        assert!(solid.contains(Vec3::ZERO));
        assert!(!solid.contains(Vec3::new(5.0, 0.0, 0.0)));
    }
}
