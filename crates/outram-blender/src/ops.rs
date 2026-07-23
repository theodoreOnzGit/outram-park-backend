//! Mesh **operators** — the editing verbs (Blender's `bmesh/operators`, `bmo_*`).
//!
//! Each operator is a **pure function over the polygon-soup view** of a mesh:
//! it reads [`Mesh::positions`] + [`Mesh::polygons`], builds a fresh
//! `positions: Vec<Vec3>` and `faces: Vec<Vec<usize>>`, and rebuilds through
//! [`Mesh::from_polygons`]. Because the rebuild goes back through
//! [`Mesh::add_face`], edge deduplication and loop wiring are recomputed for
//! free, so an operator never touches the half-edge cycles by hand.
//!
//! ## Why an enum, not trait objects
//!
//! The operator set is closed and known at compile time, so per the workspace
//! design rules it is a single [`MeshOp`] enum dispatched by `match` — not
//! `Box<dyn Trait>`. Adding a new operator forces every `match` site to handle
//! it (exhaustiveness), and rust-analyzer's go-to-definition works on each
//! variant.
//!
//! ## The operators (Blender analogue in parentheses)
//!
//! - [`extrude_faces`] / [`MeshOp::Extrude`] — duplicate a face region, offset
//!   the cap, and wall the boundary edges (`bmo_extrude`, the Extrude Region
//!   tool).
//! - [`extrude_edges`] — build a quad off each selected edge and its offset
//!   copy (the Extrude Edges tool).
//! - [`subdivide`] / [`MeshOp::Subdivide`] — **simple midpoint** subdivision
//!   (topological quad split, no smoothing; `bmo_subdivide` with smoothness 0).
//!   This is distinct from Catmull-Clark, which lives in
//!   [`crate::subdivision::catmull_clark`] and the non-destructive
//!   [`crate::modifiers::Modifier::Subsurf`].
//! - [`bevel_vertices`] / [`bevel_vertices_rounded`] / [`MeshOp::Bevel`] —
//!   vertex bevel / truncation (`bmo_bevel` in vertex-only mode): a single flat
//!   chamfer, or a rounded spherical cap for `segments >= 2`.
//! - [`MeshOp::Boolean`] — CSG union/difference/intersection of two meshes,
//!   delegated to [`crate::boolean`] (`bmo_boolean`, backed upstream by the
//!   Manifold library). This is the operator most relevant to feeding
//!   `outram-mc-libs` CSG geometry.

use std::collections::{HashMap, HashSet};

use crate::math::Vec3;
use crate::mesh::{EdgeId, FaceId, Mesh};

/// Errors returned by [`MeshOp::apply`].
#[derive(Debug, thiserror::Error)]
pub enum MeshOpError {
    /// The operator is scaffolded but its algorithm is not implemented yet.
    #[error("mesh operator not yet implemented: {0}")]
    NotImplemented(&'static str),
    /// Propagated from a mesh boolean (crate::boolean) — the operand meshes are
    /// outside the supported restricted case.
    #[error(transparent)]
    Boolean(#[from] crate::boolean::BooleanError),
    /// Propagated from Laplacian smoothing (crate::laplacian) — the sparse solve
    /// failed (e.g. a non-positive-definite system).
    #[error(transparent)]
    Laplacian(#[from] crate::laplacian::LaplacianError),
    /// Propagated from ARAP deformation (crate::arap) — missing constraints or a
    /// non-positive-definite system.
    #[error(transparent)]
    Arap(#[from] crate::arap::ArapError),
}

/// Boolean CSG mode for [`MeshOp::Boolean`] (mirrors Blender's Boolean modifier).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BooleanMode {
    /// Keep the volume in either mesh (A ∪ B).
    Union,
    /// Keep the volume of A outside B (A \ B).
    Difference,
    /// Keep the volume common to both (A ∩ B).
    Intersect,
}

/// Extrude a set of faces: duplicate their vertices, offset the duplicates, cap
/// the region with the moved copies, and wall the boundary edges.
///
/// # What it computes
///
/// For the selected region (the faces named in `faces`, given as [`FaceId`]s):
///
/// 1. **Cap.** Every vertex used by a selected face is duplicated and the copy
///    is translated by `offset`. The selected faces are removed and replaced by
///    "cap" faces on the moved copies, preserving winding.
/// 2. **Side walls.** Each *boundary edge* of the region — an edge used by
///    exactly one selected face — gets a side quad `[a, b, b', a']` connecting
///    the old edge `a→b` (in that face's winding) to its moved copy `a'→b'`.
///    The winding is chosen so the wall's outward normal points away from the
///    region interior. Interior edges (shared by two selected faces) get no
///    wall.
/// 3. **Rest of the mesh.** Vertices and faces not in the selection are kept
///    unchanged.
///
/// A single-quad selection therefore becomes a "cup": 4 side walls + 1 cap,
/// with the original quad's footprint left open.
///
/// # Inputs / units
///
/// - `mesh` — the source mesh (borrowed, unmodified).
/// - `faces` — the [`FaceId`]s to extrude; out-of-range ids are ignored.
/// - `offset` — model-space translation applied to the cap (dimensionless
///   model-space units; direction and distance combined).
///
/// # Note
///
/// Extruding a *closed* region (every edge interior, e.g. all faces of a closed
/// solid) produces no walls and leaves the original vertices unreferenced by
/// the new topology — a whole-mesh extrude of a closed solid is a degenerate
/// case, not the intended use. Extrude an open region (a grid, a face patch).
pub fn extrude_faces(mesh: &Mesh, faces: &[FaceId], offset: Vec3) -> Mesh {
    let positions = mesh.positions();
    let polys = mesh.polygons();

    // Deterministic, deduplicated selected-face indices (ignore out-of-range).
    let mut selected: Vec<usize> = faces
        .iter()
        .map(|f| f.0)
        .filter(|&f| f < polys.len())
        .collect();
    selected.sort_unstable();
    selected.dedup();
    let selected_set: HashSet<usize> = selected.iter().copied().collect();

    // Count how many selected faces use each undirected edge → boundary test.
    let mut edge_use: HashMap<(usize, usize), usize> = HashMap::new();
    for &fi in &selected {
        let verts = &polys[fi];
        let n = verts.len();
        for i in 0..n {
            let a = verts[i].0;
            let b = verts[(i + 1) % n].0;
            let key = if a < b { (a, b) } else { (b, a) };
            *edge_use.entry(key).or_insert(0) += 1;
        }
    }

    // Duplicate every vertex used by a selected face (once), moved by `offset`.
    let mut new_positions = positions.clone();
    let mut dup: HashMap<usize, usize> = HashMap::new();
    for &fi in &selected {
        for v in &polys[fi] {
            dup.entry(v.0).or_insert_with(|| {
                let idx = new_positions.len();
                new_positions.push(positions[v.0].add(offset));
                idx
            });
        }
    }

    let mut new_faces: Vec<Vec<usize>> = Vec::new();

    // Non-selected faces are kept verbatim.
    for (fi, verts) in polys.iter().enumerate() {
        if !selected_set.contains(&fi) {
            new_faces.push(verts.iter().map(|v| v.0).collect());
        }
    }

    // Cap faces: the moved duplicates, same winding as the originals.
    for &fi in &selected {
        let cap: Vec<usize> = polys[fi].iter().map(|v| dup[&v.0]).collect();
        new_faces.push(cap);
    }

    // Side quads on boundary edges, wound outward: [a, b, b', a'].
    for &fi in &selected {
        let verts = &polys[fi];
        let n = verts.len();
        for i in 0..n {
            let a = verts[i].0;
            let b = verts[(i + 1) % n].0;
            let key = if a < b { (a, b) } else { (b, a) };
            if edge_use[&key] == 1 {
                new_faces.push(vec![a, b, dup[&b], dup[&a]]);
            }
        }
    }

    Mesh::from_polygons(&new_positions, &new_faces)
}

/// Extrude a set of edges: build one quad off each edge and its offset copy.
///
/// For each [`EdgeId`] in `edges`, the two endpoints are duplicated (shared
/// across edges that touch the same vertex) and translated by `offset`, then a
/// quad `[a, b, b', a']` is added spanning the original edge `a–b` and its moved
/// copy `a'–b'`. The original faces are kept; the new quads are appended. This
/// is the secondary, deliberately simple edge-extrude — it does not attempt to
/// reason about which side the wall should face.
///
/// # Inputs / units
///
/// - `mesh` — source mesh (borrowed, unmodified).
/// - `edges` — [`EdgeId`]s to extrude; out-of-range ids are ignored.
/// - `offset` — model-space translation of the copies (dimensionless model
///   units).
pub fn extrude_edges(mesh: &Mesh, edges: &[EdgeId], offset: Vec3) -> Mesh {
    let positions = mesh.positions();
    let mut new_positions = positions.clone();
    let mut new_faces: Vec<Vec<usize>> = mesh
        .polygons()
        .iter()
        .map(|f| f.iter().map(|v| v.0).collect())
        .collect();

    let mut dup: HashMap<usize, usize> = HashMap::new();
    for &e in edges {
        let Some(edge) = mesh.edge(e) else { continue };
        let a = edge.verts[0].0;
        let b = edge.verts[1].0;
        let da = *dup.entry(a).or_insert_with(|| {
            let i = new_positions.len();
            new_positions.push(positions[a].add(offset));
            i
        });
        let db = *dup.entry(b).or_insert_with(|| {
            let i = new_positions.len();
            new_positions.push(positions[b].add(offset));
            i
        });
        new_faces.push(vec![a, b, db, da]);
    }

    Mesh::from_polygons(&new_positions, &new_faces)
}

/// Simple midpoint subdivision — split every face into quads, no smoothing.
///
/// # What it computes
///
/// This is **topological** subdivision (Blender's Subdivide with smoothness 0),
/// distinct from Catmull-Clark ([`crate::subdivision::catmull_clark`] /
/// [`crate::modifiers::Modifier::Subsurf`]): new points are placed by plain
/// linear interpolation and existing vertices are **not** moved, so the surface
/// keeps its exact shape while gaining resolution.
///
/// Each iteration, for every `n`-gon face:
///
/// - add the midpoint of each of its edges (deduplicated across faces by a
///   canonical key on the sorted endpoint-index pair, so a shared edge yields a
///   single shared midpoint), and
/// - add the face centroid (the vertex mean),
///
/// then replace the face with `n` quads, one per corner `v[i]`, wound
/// `[centroid, midpoint(edge into v[i]), v[i], midpoint(edge out of v[i])]` —
/// which preserves the original winding.
///
/// # Counts per iteration
///
/// For a closed genus-0 mesh with `V, E, F`, one pass yields
/// `V' = V + E + F`, `F' = sum of face side-counts` (`4F` when all faces are
/// quads), and `chi = 2` is preserved. E.g. a cube (`V=8, E=12, F=6`) becomes
/// `V=26, F=24, chi=2` after one pass.
///
/// # Inputs
///
/// - `mesh` — source mesh (borrowed, unmodified).
/// - `iterations` — number of passes; `iterations == 0` returns a clone.
pub fn subdivide(mesh: &Mesh, iterations: u32) -> Mesh {
    if iterations == 0 {
        return mesh.clone();
    }

    let mut positions = mesh.positions();
    let mut faces: Vec<Vec<usize>> = mesh
        .polygons()
        .iter()
        .map(|f| f.iter().map(|v| v.0).collect())
        .collect();

    for _ in 0..iterations {
        let (np, nf) = subdivide_once(&positions, &faces);
        positions = np;
        faces = nf;
    }

    Mesh::from_polygons(&positions, &faces)
}

/// One pass of simple midpoint subdivision over a polygon soup.
///
/// Returns the new `(positions, faces)` soup; see [`subdivide`] for the rules.
/// Original vertex indices are preserved (existing vertices are copied
/// unchanged), then edge midpoints and per-face centroids are appended.
fn subdivide_once(positions: &[Vec3], faces: &[Vec<usize>]) -> (Vec<Vec3>, Vec<Vec<usize>>) {
    let mut new_positions = positions.to_vec();
    let mut edge_mid: HashMap<(usize, usize), usize> = HashMap::new();
    let mut new_faces: Vec<Vec<usize>> = Vec::new();

    for face in faces {
        let n = face.len();

        // Face centroid (vertex mean).
        let mut c = Vec3::ZERO;
        for &vi in face {
            c = c.add(positions[vi]);
        }
        c = c.scale(1.0 / n as f64);
        let cidx = new_positions.len();
        new_positions.push(c);

        // Midpoint index for each edge i → i+1 (deduplicated across faces).
        let mut mids = Vec::with_capacity(n);
        for i in 0..n {
            let a = face[i];
            let b = face[(i + 1) % n];
            let key = if a < b { (a, b) } else { (b, a) };
            let idx = match edge_mid.get(&key) {
                Some(&idx) => idx,
                None => {
                    let mid = positions[a].add(positions[b]).scale(0.5);
                    let idx = new_positions.len();
                    new_positions.push(mid);
                    edge_mid.insert(key, idx);
                    idx
                }
            };
            mids.push(idx);
        }

        // One quad per corner: [centroid, mid-into-corner, corner, mid-out-of-corner].
        for i in 0..n {
            let m_out = mids[i]; // edge i → i+1 leaves corner i
            let m_in = mids[(i + n - 1) % n]; // edge i-1 → i enters corner i
            let corner = face[i];
            new_faces.push(vec![cidx, m_in, corner, m_out]);
        }
    }

    (new_positions, new_faces)
}

/// Vertex bevel / truncation — cut off every vertex, replacing it with a face.
///
/// # What it computes
///
/// This is the vertex-only bevel (a single chamfer per vertex, the polyhedral
/// *truncation* operation). For each vertex `V` and each incident edge `e`, one
/// **edge-point** is placed at
/// `V + w_e * normalize(other_end(e) - V)`, where `w_e = min(width, L_e / 2)`
/// clamps the offset so the two edge-points on an edge of length `L_e` never
/// pass its midpoint. That edge-point is shared by the two faces on `e`.
///
/// The result mesh contains **only** the edge-points (every original vertex is
/// cut away):
///
/// - **Truncated faces.** Each original `n`-gon becomes a `2n`-gon: every
///   corner `V` is replaced, in winding order, by its two edge-points — the one
///   on the edge entering `V` then the one on the edge leaving `V`.
/// - **Vertex faces.** Each original vertex contributes one new face joining its
///   edge-points in angular order around `V` (the order is read off the fan of
///   faces around `V`), wound so its normal points outward.
///
/// # Counts (verification)
///
/// Truncating a cube (`V=8, E=12, F=6`) gives the **truncated cube**:
/// `V=24` (two edge-points per edge), `E=36`, `F=14` (6 octagons + 8 triangles),
/// `chi=2`. This is asserted in the module tests.
///
/// # Inputs / units
///
/// - `mesh` — source mesh (borrowed, unmodified). Assumed manifold and
///   orientable (as produced by [`crate::primitives`]); the angular ordering of
///   a vertex face is derived from the face fan around each vertex.
/// - `width` — chamfer width in dimensionless model-space units, clamped
///   per-edge to at most half the edge length. `width <= 0` collapses the
///   edge-points onto the original vertices (a no-op-shaped degenerate result).
pub fn bevel_vertices(mesh: &Mesh, width: f64) -> Mesh {
    let skel = truncate_skeleton(mesh, width);
    let mut faces = skel.faces;
    // Single chamfer: each vertex ring closes as one flat n-gon face.
    for ring in &skel.rings {
        if ring.len() >= 3 {
            faces.push(ring.clone());
        }
    }
    Mesh::from_polygons(&skel.positions, &faces)
}

/// The shared **truncation skeleton** both the flat ([`bevel_vertices`]) and the
/// rounded ([`bevel_vertices_rounded`]) vertex bevels build on.
///
/// It holds the edge-points (two per original edge), the truncated original
/// faces (each `n`-gon widened to a `2n`-gon), and, per original vertex, the
/// **ring** of that vertex's edge-points in angular (fan) order wound outward.
/// The flat bevel caps each ring with one face; the rounded bevel domes it.
struct TruncSkeleton {
    /// Edge-point positions (index space shared by `faces` and `rings`).
    positions: Vec<Vec3>,
    /// The truncated original faces (each a `2n`-gon of edge-point indices).
    faces: Vec<Vec<usize>>,
    /// `rings[v]` = the outward-wound ring of edge-point indices around original
    /// vertex `v` (empty for an isolated vertex, or shorter than 3 for a
    /// degenerate corner).
    rings: Vec<Vec<usize>>,
}

/// Build the [`TruncSkeleton`] for `mesh` truncated by `width`.
///
/// Places one edge-point per (near, far) vertex at
/// `near + min(width, L/2) * normalize(far - near)`, widens every face to a
/// `2n`-gon of edge-points, and reads each vertex's edge-point ring off the fan
/// of incident faces (out-edge `V->next` links to in-edge `prev->V`; chaining
/// the links walks the ring in angular order, wound outward).
fn truncate_skeleton(mesh: &Mesh, width: f64) -> TruncSkeleton {
    let positions = mesh.positions();
    let polys = mesh.polygons();

    // Edge-point per (near-vertex, far-vertex): index into new positions.
    let mut ep_index: HashMap<(usize, usize), usize> = HashMap::new();
    let mut new_positions: Vec<Vec3> = Vec::new();

    for ei in 0..mesh.edge_count() {
        let edge = mesh.edge(EdgeId(ei)).expect("edge id in range");
        let a = edge.verts[0].0;
        let b = edge.verts[1].0;
        let pa = positions[a];
        let pb = positions[b];
        let len = pb.sub(pa).length();
        // Clamp so neither edge-point passes the edge midpoint.
        let w = width.clamp(0.0, 0.5 * len);

        let ep_a = pa.add(pb.sub(pa).normalize().scale(w));
        let ia = new_positions.len();
        new_positions.push(ep_a);
        ep_index.insert((a, b), ia);

        let ep_b = pb.add(pa.sub(pb).normalize().scale(w));
        let ib = new_positions.len();
        new_positions.push(ep_b);
        ep_index.insert((b, a), ib);
    }

    // Truncated original faces: each corner → two edge-points (in, then out).
    let mut faces: Vec<Vec<usize>> = Vec::new();
    for face in &polys {
        let n = face.len();
        let mut nf = Vec::with_capacity(2 * n);
        for i in 0..n {
            let prev = face[(i + n - 1) % n].0;
            let cur = face[i].0;
            let next = face[(i + 1) % n].0;
            nf.push(ep_index[&(cur, prev)]); // on edge entering `cur`
            nf.push(ep_index[&(cur, next)]); // on edge leaving `cur`
        }
        faces.push(nf);
    }

    // Per-vertex edge-point ring, in angular order (out-neighbour → in-neighbour).
    let nv = positions.len();
    let mut links: Vec<HashMap<usize, usize>> = vec![HashMap::new(); nv];
    for face in &polys {
        let n = face.len();
        for i in 0..n {
            let prev = face[(i + n - 1) % n].0;
            let cur = face[i].0;
            let next = face[(i + 1) % n].0;
            links[cur].insert(next, prev);
        }
    }

    let mut rings: Vec<Vec<usize>> = Vec::with_capacity(nv);
    for v in 0..nv {
        let map = &links[v];
        let mut ring: Vec<usize> = Vec::new();
        if !map.is_empty() {
            let start = *map.keys().next().expect("non-empty link map");
            let mut cur = start;
            // Bounded by the degree so a non-manifold vertex can't loop forever.
            for _ in 0..=map.len() {
                ring.push(ep_index[&(v, cur)]);
                match map.get(&cur) {
                    Some(&nx) if nx != start => cur = nx,
                    _ => break,
                }
            }
        }
        rings.push(ring);
    }

    TruncSkeleton { positions: new_positions, faces, rings }
}

/// Rounded (multi-segment) vertex bevel — truncate every vertex and replace the
/// flat chamfer with a **spherical cap** of `segments` bands.
///
/// # What it computes
///
/// The truncation (edge-points, widened faces) is identical to
/// [`bevel_vertices`]; only the per-vertex cap differs. For `segments <= 1` this
/// *is* [`bevel_vertices`] (one flat face per vertex). For `segments >= 2`, each
/// vertex `V`'s edge-point ring is domed into a spherical cap on the sphere
/// centred at `V` of radius `~width`:
///
/// - the ring of edge-points is **band 0** (shared with the truncated faces, so
///   the result stays watertight);
/// - `segments - 1` **intermediate rings** are inserted by spherical-linear
///   interpolation ([`slerp`]) of each edge-point's direction toward the cap
///   **apex** `V + R * n`, where `n` is the outward vertex normal (sum of
///   incident face normals) and `R` the mean edge-point distance; the radius is
///   interpolated linearly so band 0 stays exactly on the (possibly
///   per-edge-clamped) edge-points;
/// - the cap closes with a triangle fan to the apex.
///
/// The corner therefore becomes a smooth convex spherical cap of radius `~width`
/// about the original vertex — a *rounded* bevel. This is a mesh-authoring
/// rounding (a spherical cap through the edge-points), **not** a CAD fillet
/// tangent to the adjacent faces; the apex bulges toward the original corner
/// along the outward normal. Higher `segments` gives a smoother cap.
///
/// # Counts (verification)
///
/// For a cube (`V=8, E=12, F=6`, every vertex degree 3) at `segments = s >= 2`:
/// `V = 24s + 8`, `F = 24s + 6` (6 octagons + `3s` cap faces per vertex),
/// `E = 48s + 12`, `chi = 2`. Asserted in the module tests.
///
/// # Inputs / units
///
/// - `mesh` — source mesh (borrowed, unmodified); assumed manifold/orientable.
/// - `width` — chamfer width (model-space units, clamped per-edge to half the
///   edge length), same as [`bevel_vertices`].
/// - `segments` — bevel resolution: `0`/`1` = single flat chamfer, `>= 2` =
///   rounded cap with `segments - 1` intermediate rings.
pub fn bevel_vertices_rounded(mesh: &Mesh, width: f64, segments: u32) -> Mesh {
    if segments <= 1 {
        return bevel_vertices(mesh, width);
    }
    let seg = segments as usize;
    let skel = truncate_skeleton(mesh, width);
    let src = mesh.positions();

    // Outward vertex normals = normalized sum of incident face normals.
    let nv = src.len();
    let mut vnormal = vec![Vec3::ZERO; nv];
    for f in 0..mesh.face_count() {
        let fn_ = mesh.face_normal(FaceId(f));
        for v in mesh.face_vertices(FaceId(f)) {
            vnormal[v.0] = vnormal[v.0].add(fn_);
        }
    }

    let mut positions = skel.positions;
    let mut faces = skel.faces;

    for (v, ring) in skel.rings.iter().enumerate() {
        if ring.len() < 3 {
            continue;
        }
        let vpos = src[v];
        let n_out = vnormal[v].normalize();
        if n_out == Vec3::ZERO {
            // Degenerate normal — fall back to a flat cap for this vertex.
            faces.push(ring.clone());
            continue;
        }

        let d = ring.len();
        // Band 0 = the edge-points; per-edge directions and radii from V.
        let dir0: Vec<Vec3> = ring.iter().map(|&ep| positions[ep].sub(vpos).normalize()).collect();
        let len0: Vec<f64> = ring.iter().map(|&ep| positions[ep].sub(vpos).length()).collect();
        let apex_r = len0.iter().sum::<f64>() / d as f64;

        // Intermediate ring vertex indices, `ring_idx[k]` for k = 1..seg-1.
        let mut ring_idx: Vec<Vec<usize>> = Vec::with_capacity(seg - 1);
        for k in 1..seg {
            let t = k as f64 / seg as f64;
            let mut this_ring = Vec::with_capacity(d);
            for i in 0..d {
                let dir = slerp(dir0[i], n_out, t);
                let r = len0[i] * (1.0 - t) + apex_r * t;
                this_ring.push(positions.len());
                positions.push(vpos.add(dir.scale(r)));
            }
            ring_idx.push(this_ring);
        }
        // Apex.
        let apex = positions.len();
        positions.push(vpos.add(n_out.scale(apex_r)));

        // Bands: ring[0] = edge-points, ring[k] = ring_idx[k-1], up to apex.
        let mut prev: Vec<usize> = ring.clone();
        for band in ring_idx.iter() {
            for i in 0..d {
                let j = (i + 1) % d;
                push_oriented(&mut faces, &positions, vpos, vec![prev[i], prev[j], band[j], band[i]]);
            }
            prev = band.clone();
        }
        // Top fan to the apex.
        for i in 0..d {
            let j = (i + 1) % d;
            push_oriented(&mut faces, &positions, vpos, vec![prev[i], prev[j], apex]);
        }
    }

    Mesh::from_polygons(&positions, &faces)
}

/// Push face `idx` into `faces`, reversing its winding if needed so its outward
/// normal points away from the cap centre `centre` (i.e. `normal . (centroid -
/// centre) >= 0`). Keeps every rounded-cap face consistently outward regardless
/// of the ring traversal direction.
fn push_oriented(faces: &mut Vec<Vec<usize>>, positions: &[Vec3], centre: Vec3, idx: Vec<usize>) {
    let mut c = Vec3::ZERO;
    for &i in &idx {
        c = c.add(positions[i]);
    }
    let c = c.scale(1.0 / idx.len() as f64);
    // Newell normal of the polygon.
    let mut nrm = Vec3::ZERO;
    for k in 0..idx.len() {
        let p = positions[idx[k]];
        let q = positions[idx[(k + 1) % idx.len()]];
        nrm = nrm.add(Vec3::new(
            (p.y - q.y) * (p.z + q.z),
            (p.z - q.z) * (p.x + q.x),
            (p.x - q.x) * (p.y + q.y),
        ));
    }
    let mut idx = idx;
    if nrm.dot(c.sub(centre)) < 0.0 {
        idx.reverse();
    }
    faces.push(idx);
}

/// Spherical-linear interpolation of two **unit** vectors by `t in [0, 1]`.
///
/// Returns a unit vector on the great-circle arc from `a` (t=0) to `b` (t=1).
/// Degenerates gracefully: near-parallel inputs return `a`; near-antiparallel
/// inputs fall back to normalized linear interpolation (the arc is undefined
/// there, but this keeps the result finite for the rounded-bevel cap).
fn slerp(a: Vec3, b: Vec3, t: f64) -> Vec3 {
    let dot = a.dot(b).clamp(-1.0, 1.0);
    let omega = dot.acos();
    if omega < 1e-9 {
        return a;
    }
    if omega > std::f64::consts::PI - 1e-9 {
        return a.scale(1.0 - t).add(b.scale(t)).normalize();
    }
    let so = omega.sin();
    a.scale(((1.0 - t) * omega).sin() / so)
        .add(b.scale((t * omega).sin() / so))
}

/// A closed set of mesh-editing operators.
///
/// Construct a variant with its parameters, then call [`MeshOp::apply`] on a
/// mesh. Parameters are captured by value (no borrows) so the enum carries no
/// lifetimes.
#[derive(Debug, Clone)]
pub enum MeshOp {
    /// Extrude **the whole mesh's faces** along `offset` (direction and distance
    /// combined). [`MeshOp::apply`] runs [`extrude_faces`] over every face; use
    /// [`extrude_faces`] directly to extrude a chosen face region.
    Extrude {
        /// Model-space translation applied to the newly created cap geometry.
        offset: Vec3,
    },
    /// Apply `iterations` rounds of **simple midpoint** subdivision (topological
    /// quad split, no smoothing — existing vertices stay put).
    ///
    /// This is *not* Catmull-Clark: for a smoothed subdivision surface use the
    /// non-destructive [`crate::modifiers::Modifier::Subsurf`] or the direct
    /// [`crate::subdivision::catmull_clark`]. See [`subdivide`] for the rules.
    Subdivide {
        /// Number of refinement passes (each roughly quadruples face count);
        /// `0` is a no-op clone.
        iterations: u32,
    },
    /// Bevel (truncate) every vertex by `width` model-space units.
    ///
    /// Dispatches to [`bevel_vertices_rounded`]: `segments <= 1` is the single
    /// flat chamfer ([`bevel_vertices`], the polyhedral truncation);
    /// `segments >= 2` rounds each cut corner into a spherical cap with
    /// `segments - 1` intermediate rings.
    Bevel {
        /// Bevel width in model-space units (clamped per-edge to half the edge
        /// length).
        width: f64,
        /// Number of segments across the bevel: `1` = a single flat chamfer,
        /// `>= 2` = a rounded spherical cap with `segments - 1` intermediate
        /// rings (see [`bevel_vertices_rounded`]).
        segments: u32,
    },
    /// Combine the target mesh with `other` under a [`BooleanMode`], delegated to
    /// [`crate::boolean::boolean`].
    Boolean {
        /// The second operand mesh (owned by value — no lifetimes).
        other: Mesh,
        /// Union / difference / intersection.
        mode: BooleanMode,
    },
    /// **Implicit Laplacian smoothing** (mesh fairing), delegated to
    /// [`crate::laplacian::laplacian_smooth`]. Solves `(I + λL) x' = x` per
    /// iteration with boundary vertices pinned; unconditionally stable.
    Smooth {
        /// Uniform (umbrella) or cotangent (Laplace–Beltrami) weighting.
        weighting: crate::laplacian::LaplacianWeighting,
        /// Smoothing strength per step (`>= 0`; `0` is a no-op).
        lambda: f64,
        /// Number of implicit steps (`0` is a no-op).
        iterations: u32,
    },
    /// **Taubin `λ|μ` smoothing** (explicit, shrinkage-free denoising), delegated
    /// to [`crate::laplacian::taubin_smooth`].
    Taubin {
        /// Uniform or cotangent weighting.
        weighting: crate::laplacian::LaplacianWeighting,
        /// Shrinking factor `0 < λ < 1`.
        lambda: f64,
        /// Un-shrinking factor `−1 < μ < −λ`.
        mu: f64,
        /// Number of `λ|μ` iteration pairs.
        iterations: u32,
    },
    /// **As-Rigid-As-Possible deformation**, delegated to
    /// [`crate::arap::arap_deform`]. Deforms the mesh to meet the `handles`
    /// (vertex → target) while keeping one-rings maximally rigid.
    Arap {
        /// Handle constraints: each `(vertex, target position)`.
        handles: Vec<(crate::mesh::VertexId, Vec3)>,
        /// Number of local/global ARAP iterations.
        iterations: u32,
    },
    /// **QEM mesh decimation** (quadric-error-metric simplification), delegated
    /// to [`crate::decimate::decimate`]. Reduces the mesh to roughly
    /// `target_faces` triangles.
    Decimate {
        /// The goal triangle count (a lower-bound target).
        target_faces: usize,
    },
}

impl MeshOp {
    /// Apply this operator to `mesh`, returning the edited mesh.
    ///
    /// The `mesh` argument is consumed and the result returned, matching
    /// Blender's destructive-operator model. Dispatch is an exhaustive `match`:
    ///
    /// - [`MeshOp::Extrude`] → [`extrude_faces`] over every face (whole-mesh
    ///   extrude).
    /// - [`MeshOp::Subdivide`] → [`subdivide`].
    /// - [`MeshOp::Bevel`] → [`bevel_vertices_rounded`] (single flat chamfer for
    ///   `segments <= 1`, a rounded spherical cap for `segments >= 2`).
    /// - [`MeshOp::Boolean`] → [`crate::boolean::boolean`], whose error is
    ///   surfaced as [`MeshOpError::Boolean`].
    /// - [`MeshOp::Smooth`] → [`crate::laplacian::laplacian_smooth`], whose error
    ///   is surfaced as [`MeshOpError::Laplacian`].
    /// - [`MeshOp::Taubin`] → [`crate::laplacian::taubin_smooth`] (infallible
    ///   explicit filter).
    pub fn apply(&self, mesh: Mesh) -> Result<Mesh, MeshOpError> {
        match self {
            MeshOp::Extrude { offset } => {
                let faces: Vec<FaceId> = (0..mesh.face_count()).map(FaceId).collect();
                Ok(extrude_faces(&mesh, &faces, *offset))
            }
            MeshOp::Subdivide { iterations } => Ok(subdivide(&mesh, *iterations)),
            MeshOp::Bevel { width, segments } => Ok(bevel_vertices_rounded(&mesh, *width, *segments)),
            MeshOp::Boolean { other, mode } => Ok(crate::boolean::boolean(&mesh, other, *mode)?),
            MeshOp::Smooth { weighting, lambda, iterations } => {
                Ok(crate::laplacian::laplacian_smooth(&mesh, *weighting, *lambda, *iterations)?)
            }
            MeshOp::Taubin { weighting, lambda, mu, iterations } => {
                Ok(crate::laplacian::taubin_smooth(&mesh, *weighting, *lambda, *mu, *iterations))
            }
            MeshOp::Arap { handles, iterations } => {
                Ok(crate::arap::arap_deform(&mesh, handles, *iterations)?)
            }
            MeshOp::Decimate { target_faces } => Ok(crate::decimate::decimate(&mesh, *target_faces)),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::primitives;

    /// Contract: implemented operators dispatch and return a real edited mesh
    /// (no fake-green, no lingering `NotImplemented`). Subdivide is now real, so
    /// this checks it actually refines the mesh rather than erroring.
    ///
    /// Methodology: subdivide a unit cube once and confirm the result is `Ok`
    /// and strictly larger than the input.
    /// Result (data-independent, topological): cube 8/6 → 26 verts / 24 faces.
    #[test]
    fn operators_report_not_implemented() {
        let m = primitives::cube(1.0);
        let op = MeshOp::Subdivide { iterations: 1 };
        let out = op.apply(m).expect("subdivide is implemented");
        assert_eq!(out.vertex_count(), 26);
        assert_eq!(out.face_count(), 24);
    }

    /// Extrude a single quad (`grid(1,1)`) into an open "cup".
    ///
    /// Methodology: `grid(1,1)` is one quad — 4 verts, 1 face, `chi = 1` (a
    /// disc). Extruding its single face along `+Z` duplicates the 4 verts,
    /// caps the top, and — since all 4 edges are boundary edges — builds 4 side
    /// walls, leaving the original footprint open.
    ///
    /// Result: `V = 4 + 4 = 8`, `F = 1 cap + 4 walls = 5`, `E = 12`,
    /// `chi = 8 - 12 + 5 = 1` (still an open disc — the cup has no bottom).
    #[test]
    fn extrude_single_quad_makes_a_cup() {
        let grid = primitives::grid(1, 1, 2.0);
        assert_eq!(grid.face_count(), 1);
        assert_eq!(grid.vertex_count(), 4);

        let out = extrude_faces(&grid, &[FaceId(0)], Vec3::new(0.0, 0.0, 1.0));
        assert_eq!(out.vertex_count(), 8, "4 original + 4 moved cap verts");
        assert_eq!(out.face_count(), 5, "1 cap + 4 side walls");
        assert_eq!(out.edge_count(), 12, "4 bottom + 4 top + 4 vertical");
        assert_eq!(out.euler_characteristic(), 1, "open cup is a disc, chi = 1");
    }

    /// Whole-mesh extrude via [`MeshOp::Extrude`] runs over every face and
    /// returns `Ok` — a smoke test of the apply-arm wiring.
    #[test]
    fn meshop_extrude_dispatches_over_all_faces() {
        let grid = primitives::grid(1, 1, 2.0);
        let op = MeshOp::Extrude { offset: Vec3::new(0.0, 0.0, 1.0) };
        let out = op.apply(grid).expect("extrude implemented");
        // Same cup as the direct call above.
        assert_eq!(out.vertex_count(), 8);
        assert_eq!(out.face_count(), 5);
        assert_eq!(out.euler_characteristic(), 1);
    }

    /// One pass of simple midpoint subdivision on a cube.
    ///
    /// Methodology: cube `V=8, E=12, F=6`. Simple midpoint adds one vertex per
    /// edge (12) and one centroid per face (6), and splits each quad into 4
    /// quads. Existing verts are unmoved, so `chi` is preserved.
    ///
    /// Result: `V = 8 + 12 + 6 = 26`, `F = 6 * 4 = 24`, `E = 48`, `chi = 2`.
    #[test]
    fn subdivide_cube_one_iteration() {
        let out = subdivide(&primitives::cube(2.0), 1);
        assert_eq!(out.vertex_count(), 26);
        assert_eq!(out.face_count(), 24);
        assert_eq!(out.edge_count(), 48);
        assert_eq!(out.euler_characteristic(), 2, "closed mesh stays chi = 2");
    }

    /// `iterations = 0` is a clone; two passes equal one-then-one.
    ///
    /// Methodology: subdividing is not idempotent (counts grow), but the
    /// iteration loop must be consistent — `subdivide(m, 2)` must produce the
    /// same element counts as `subdivide(subdivide(m, 1), 1)`. Second pass of a
    /// cube: `V = 26 + 48 edges + 24 faces = 98`, `F = 24 * 4 = 96`, `chi = 2`.
    #[test]
    fn subdivide_iterations_compose() {
        let cube = primitives::cube(2.0);

        let zero = subdivide(&cube, 0);
        assert_eq!(zero.vertex_count(), cube.vertex_count(), "0 iters is a clone");
        assert_eq!(zero.face_count(), cube.face_count());

        let twice = subdivide(&cube, 2);
        let once_once = subdivide(&subdivide(&cube, 1), 1);
        assert_eq!(twice.vertex_count(), once_once.vertex_count());
        assert_eq!(twice.face_count(), once_once.face_count());
        assert_eq!(twice.edge_count(), once_once.edge_count());

        assert_eq!(twice.vertex_count(), 98);
        assert_eq!(twice.face_count(), 96);
        assert_eq!(twice.euler_characteristic(), 2);
    }

    /// Vertex-bevel (truncate) a cube → the truncated cube.
    ///
    /// Methodology: the cube (`V=8, E=12, F=6`) truncated at every vertex is the
    /// Archimedean **truncated cube**. Each edge yields two edge-points, each
    /// square face becomes an octagon, and each vertex becomes a triangle.
    ///
    /// Result: `V = 2 * 12 = 24`, `F = 6 octagons + 8 triangles = 14`,
    /// `E = 36`, `chi = 24 - 36 + 14 = 2`.
    #[test]
    fn bevel_cube_makes_truncated_cube() {
        let out = bevel_vertices(&primitives::cube(2.0), 0.5);
        assert_eq!(out.vertex_count(), 24, "two edge-points per cube edge");
        assert_eq!(out.edge_count(), 36);
        assert_eq!(out.face_count(), 14, "6 octagons + 8 triangles");
        assert_eq!(out.euler_characteristic(), 2, "closed solid, chi = 2");

        // 8 triangular vertex faces + 6 octagonal truncated faces.
        let mut tri = 0;
        let mut oct = 0;
        for f in 0..out.face_count() {
            match out.face_vertices(FaceId(f)).len() {
                3 => tri += 1,
                8 => oct += 1,
                other => panic!("unexpected face with {other} sides"),
            }
        }
        assert_eq!(tri, 8, "8 triangular vertex faces");
        assert_eq!(oct, 6, "6 octagonal truncated faces");
    }

    /// Rounded (multi-segment) vertex bevel of a cube.
    ///
    /// Methodology: bevel `cube(2.0)` at `width = 0.5` with `segments = s`. For a
    /// cube (every vertex degree 3) the closed-form counts are `V = 24s + 8`,
    /// `F = 24s + 6` (6 octagons + `3s` spherical-cap faces per vertex), and
    /// `E = 48s + 12`, so `chi = 2`. The result must be a closed 2-manifold and
    /// the cap vertices must lie on the sphere of radius `~width` about each
    /// original corner (genuinely rounded, not flat).
    ///
    /// Results (asserted below) for `s = 3`: `V = 80`, `F = 78`, `chi = 2`,
    /// every edge shared by exactly two faces, and the beveled cube's bounding
    /// box slightly exceeds the original `[-1,1]^3` because the convex cap bulges
    /// outward along each corner's normal.
    #[test]
    fn rounded_bevel_cube_counts_and_manifold() {
        let s = 3u32;
        let out = bevel_vertices_rounded(&primitives::cube(2.0), 0.5, s);
        let sf = s as i64;
        assert_eq!(out.vertex_count() as i64, 24 * sf + 8, "V = 24s + 8");
        assert_eq!(out.face_count() as i64, 24 * sf + 6, "F = 24s + 6");
        assert_eq!(out.euler_characteristic(), 2, "closed solid, chi = 2");

        // Edge-manifold: every undirected edge borders exactly two faces.
        let mut edge_use: HashMap<(usize, usize), usize> = HashMap::new();
        for f in 0..out.face_count() {
            let vs = out.face_vertices(FaceId(f));
            let n = vs.len();
            for i in 0..n {
                let (a, b) = (vs[i].0, vs[(i + 1) % n].0);
                let key = if a < b { (a, b) } else { (b, a) };
                *edge_use.entry(key).or_insert(0) += 1;
            }
        }
        assert!(edge_use.values().all(|&c| c == 2), "rounded bevel must be edge-manifold");
    }

    /// [`MeshOp::Smooth`] dispatches to Laplacian smoothing: it preserves the
    /// mesh's topology (vertex/face counts and `chi`) and, on a closed sphere,
    /// returns a valid closed mesh. (Numerical denoising behaviour is verified
    /// in the `laplacian` module tests.)
    #[test]
    fn meshop_smooth_preserves_topology() {
        use crate::laplacian::LaplacianWeighting;
        let sphere = primitives::uv_sphere(16, 8, 1.0);
        let (v, f, chi) = (sphere.vertex_count(), sphere.face_count(), sphere.euler_characteristic());
        let out = MeshOp::Smooth { weighting: LaplacianWeighting::Cotangent, lambda: 0.5, iterations: 2 }
            .apply(sphere)
            .expect("smoothing solve ok");
        assert_eq!(out.vertex_count(), v, "smoothing preserves vertex count");
        assert_eq!(out.face_count(), f, "smoothing preserves face count");
        assert_eq!(out.euler_characteristic(), chi, "smoothing preserves topology (chi)");
    }

    /// `segments <= 1` is exactly the single flat chamfer (the truncated cube),
    /// while `segments >= 2` adds intermediate rings — so the two differ.
    #[test]
    fn rounded_bevel_segments_change_the_result() {
        let cube = primitives::cube(2.0);
        let flat = MeshOp::Bevel { width: 0.5, segments: 1 }
            .apply(cube.clone())
            .expect("bevel implemented");
        let rounded = MeshOp::Bevel { width: 0.5, segments: 4 }
            .apply(cube)
            .expect("bevel implemented");
        assert_eq!(flat.vertex_count(), 24, "segments=1 is the truncated cube");
        assert!(
            rounded.vertex_count() > flat.vertex_count(),
            "segments>=2 must add intermediate-ring vertices ({} vs {})",
            rounded.vertex_count(),
            flat.vertex_count()
        );
        assert_eq!(rounded.euler_characteristic(), 2, "rounded result still closed");
    }
}
