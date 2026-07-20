//! Mesh boolean / CSG operator — **entry point + exact convex fast path**.
//!
//! Blender analogue: `bmesh/tools/bmesh_boolean` / the `bmo_boolean` operator,
//! which upstream is backed by the **Manifold** library. [`boolean`] is the
//! single public entry point for all three CSG modes; it dispatches between two
//! implementations:
//!
//! - **Exact convex fast path (this module).** When both operands are convex
//!   and the mode is [`crate::ops::BooleanMode::Intersect`], the intersection is
//!   computed here by **half-space clipping** — an exact, non-triangulated
//!   result (a clean n-gon mesh).
//! - **General arrangement path** ([`crate::boolean_general`]). Union,
//!   Difference, and any **non-convex** operand are handled there, by cutting
//!   the two surfaces against each other and classifying the resulting patches
//!   with the generalized winding number. See that module for its contract and
//!   limitations (coplanar-overlap rejection, generic-position assumption,
//!   triangulated output).
//!
//! ## The convex fast path, in detail
//!
//! Every face of operand `B` defines an outward half-space
//!
//! - plane through [`Mesh::face_centroid`] `c` with outward unit normal
//!   [`Mesh::face_normal`] `n`;
//! - the solid interior of `B` is the set of points `p` with
//!   `dot(n, p - c) <= 0` for **every** face of `B`.
//!
//! Operand `A`'s convex polytope is clipped against each of `B`'s face
//! half-spaces in turn (successive 3-D Sutherland–Hodgman polygon clipping):
//! for each clip plane every face polygon of the running result is clipped to
//! the inside half-space, the segments where faces cross the plane are collected,
//! and a fresh **cap** face is built from that loop of cut points (ordered by
//! angle about the plane normal, wound so its outward normal matches the clip
//! plane). The result is the convex intersection `A ∩ B` — a valid closed convex
//! mesh — for which Euler's identity `V - E + F = 2` holds.
//!
//! A **non-overlapping / empty** convex intersection returns
//! [`BooleanError::Unsupported`] rather than an empty mesh, so a caller cannot
//! mistake "no overlap" for a valid degenerate solid. (When the operands are
//! *not* both convex, `Intersect` never reaches this path — it goes to the
//! general pipeline, which handles the empty/disjoint case there.)
//!
//! > **Untrusted AI-generated draft** until a human reviews it, per the workspace
//! > `RESPONSIBLE_USE.md`. Not for nuclear facility operation, reactor control,
//! > safety-critical, or licensing decisions.

use crate::math::Vec3;
use crate::mesh::{FaceId, Mesh};
use crate::ops::BooleanMode;

/// Errors from a mesh boolean operation.
///
/// The only variant is [`BooleanError::Unsupported`], carrying a short static
/// reason. It is returned for the inputs neither the convex fast path nor the
/// general arrangement pipeline can resolve (see the module docs and
/// [`crate::boolean_general`]) — an honest signal, never a silently wrong mesh.
#[derive(Debug, thiserror::Error)]
pub enum BooleanError {
    /// The boolean could not be resolved: an empty/non-overlapping intersection,
    /// a **coplanar overlapping face** between the operands, an
    /// exactly-degenerate arrangement, or a result that welds to fewer than four
    /// faces. The `&'static str` names which.
    #[error("boolean unsupported: {0}")]
    Unsupported(&'static str),
}

/// Compute the boolean of two closed meshes under `mode`.
///
/// Single entry point for all three CSG modes, dispatching between the exact
/// convex fast path (this module) and the general arrangement pipeline
/// ([`crate::boolean_general`]) — see the module-level docs. In summary:
///
/// - [`BooleanMode::Intersect`] on two **convex** closed meshes returns the
///   convex intersection `A ∩ B` (a valid closed mesh, `V - E + F = 2`), computed
///   exactly by half-space clipping of `a` against every face half-space of `b`.
/// - [`BooleanMode::Union`], [`BooleanMode::Difference`], and any **non-convex**
///   operand are computed by [`crate::boolean_general::boolean_general`]
///   (surface arrangement + winding classification; triangulated output).
/// - An **empty / non-overlapping** intersection, or a coplanar-overlap /
///   degenerate arrangement input, returns [`BooleanError::Unsupported`] rather
///   than a silently wrong mesh.
///
/// Both meshes are dimensionless model-space geometry (see [`crate::math`]);
/// `mode` selects the CSG operation. Operands are taken by shared reference and
/// are not modified.
pub fn boolean(a: &Mesh, b: &Mesh, mode: BooleanMode) -> Result<Mesh, BooleanError> {
    // Fast path: two convex operands under Intersect are handled exactly by the
    // half-space clipper below, which yields a clean n-gon (non-triangulated)
    // result. Everything else — Union, Difference, or any non-convex operand —
    // goes through the general arrangement + winding-classification pipeline in
    // [`crate::boolean_general`].
    const CONVEX_REL_EPS: f64 = 1e-7;
    if mode == BooleanMode::Intersect && is_convex(a, CONVEX_REL_EPS) && is_convex(b, CONVEX_REL_EPS)
    {
        return intersect_convex(a, b);
    }
    crate::boolean_general::boolean_general(a, b, mode)
}

// ---------------------------------------------------------------------------
// Internal: convex intersection by half-space clipping.
//
// A "polygon soup" (`Vec<Poly>`, each `Poly` a ring of `Vec3` in winding order)
// is the working representation — the same soup view `Mesh::polygons` /
// `Mesh::from_polygons` bridge to/from, so no half-edge surgery is needed.
// ---------------------------------------------------------------------------

/// One face polygon as its ring of positions, in winding order.
type Poly = Vec<Vec3>;

/// Intersect two convex meshes by clipping `a` against each face half-space of
/// `b`. Returns [`BooleanError::Unsupported`] if either operand is non-convex
/// (best-effort) or the intersection is empty/degenerate.
fn intersect_convex(a: &Mesh, b: &Mesh) -> Result<Mesh, BooleanError> {
    // Relative convexity tolerance: a vertex is "outside" a face plane only if
    // its signed distance exceeds this fraction of the mesh's bounding-box
    // diagonal. Exact-coincident vertices (signed distance 0) count as inside.
    const CONVEX_REL_EPS: f64 = 1e-7;

    if !is_convex(a, CONVEX_REL_EPS) {
        return Err(BooleanError::Unsupported("operand is not convex"));
    }
    if !is_convex(b, CONVEX_REL_EPS) {
        return Err(BooleanError::Unsupported("operand is not convex"));
    }

    // Absolute tolerances scaled to the geometry so they behave the same at any
    // model scale. `clip_eps` decides which side of a clip plane a vertex is on;
    // `weld` merges coincident cut points / corners into one shared vertex so
    // the output is a closed, edge-deduplicated manifold.
    let scale = mesh_scale(a).max(mesh_scale(b)).max(1.0);
    let clip_eps = 1e-9 * scale;
    let weld = 1e-7 * scale;

    // Running result starts as A's face polygons.
    let mut polys = poly_soup(a);
    if polys.is_empty() {
        return Err(BooleanError::Unsupported("empty or non-overlapping intersection"));
    }

    // Clip against every face half-space of B.
    for f in 0..b.face_count() {
        let n = b.face_normal(FaceId(f));
        if n == Vec3::ZERO {
            continue; // degenerate face — no meaningful half-space
        }
        let c = b.face_centroid(FaceId(f));
        polys = clip_soup_against_plane(&polys, n, c, clip_eps, weld);
        if polys.is_empty() {
            return Err(BooleanError::Unsupported("empty or non-overlapping intersection"));
        }
    }

    // A closed solid needs at least four faces; fewer means the clip collapsed
    // to a coplanar/degenerate sliver (surfaces merely touching), not a volume.
    if polys.len() < 4 {
        return Err(BooleanError::Unsupported("empty or non-overlapping intersection"));
    }

    build_mesh(&polys, weld)
}

/// Best-effort convexity test: the mesh is convex iff every vertex lies on the
/// inner side (signed distance `<= eps`) of **every** face's supporting plane.
///
/// `eps_rel` is scaled by the bounding-box diagonal. Faces with a degenerate
/// (zero) normal are skipped. Correct outward face normals are assumed (as the
/// [`crate::primitives`] generators produce); this is a heuristic guard against
/// obviously non-convex input, not a proof of convexity.
fn is_convex(m: &Mesh, eps_rel: f64) -> bool {
    let ps = m.positions();
    if ps.len() < 4 || m.face_count() < 4 {
        return false; // not a closed volume we can clip against
    }
    let eps = eps_rel * mesh_scale(m).max(1.0);
    for f in 0..m.face_count() {
        let n = m.face_normal(FaceId(f));
        if n == Vec3::ZERO {
            continue;
        }
        let c = m.face_centroid(FaceId(f));
        for &p in &ps {
            if n.dot(p.sub(c)) > eps {
                return false;
            }
        }
    }
    true
}

/// Bounding-box diagonal length of a mesh's vertices (a size scale for
/// tolerances). Returns `0.0` for an empty mesh.
fn mesh_scale(m: &Mesh) -> f64 {
    let ps = m.positions();
    if ps.is_empty() {
        return 0.0;
    }
    let mut lo = ps[0];
    let mut hi = ps[0];
    for &p in &ps {
        lo = Vec3::new(lo.x.min(p.x), lo.y.min(p.y), lo.z.min(p.z));
        hi = Vec3::new(hi.x.max(p.x), hi.y.max(p.y), hi.z.max(p.z));
    }
    hi.sub(lo).length()
}

/// Convert a mesh into its polygon soup (each face as a ring of positions).
fn poly_soup(m: &Mesh) -> Vec<Poly> {
    let ps = m.positions();
    m.polygons()
        .iter()
        .map(|face| face.iter().map(|v| ps[v.0]).collect())
        .collect()
}

/// Clip a whole polygon soup against one half-space `dot(n, p - c) <= 0` and add
/// the newly exposed cap face.
///
/// Each face is clipped to the inside half-space; faces fully outside vanish.
/// The cut points where faces cross the plane are collected into one loop and
/// re-capped so the running result stays a closed convex mesh.
fn clip_soup_against_plane(polys: &[Poly], n: Vec3, c: Vec3, eps: f64, weld: f64) -> Vec<Poly> {
    let mut out: Vec<Poly> = Vec::new();
    let mut cut_points: Vec<Vec3> = Vec::new();

    for poly in polys {
        let (clipped, crossings) = clip_polygon(poly, n, c, eps);
        if clipped.len() >= 3 {
            out.push(clipped);
        }
        cut_points.extend(crossings);
    }

    if let Some(cap) = build_cap(&cut_points, n, weld) {
        out.push(cap);
    }
    out
}

/// Clip one convex polygon to the half-space `dot(n, p - c) <= 0`
/// (Sutherland–Hodgman against a single plane).
///
/// Returns `(clipped_polygon, crossing_points)`: the inside portion in winding
/// order, and the (0 or 2, for a convex polygon) points where the boundary
/// crosses the plane — the endpoints of that face's contribution to the cap
/// edge.
fn clip_polygon(poly: &[Vec3], n: Vec3, c: Vec3, eps: f64) -> (Vec<Vec3>, Vec<Vec3>) {
    let len = poly.len();
    if len < 3 {
        return (Vec::new(), Vec::new());
    }
    // Signed distance of each vertex to the plane (negative = inside).
    let sd: Vec<f64> = poly.iter().map(|&p| n.dot(p.sub(c))).collect();

    let mut out: Vec<Vec3> = Vec::new();
    let mut crossings: Vec<Vec3> = Vec::new();
    for i in 0..len {
        let j = (i + 1) % len;
        let dc = sd[i];
        let dn = sd[j];
        let cur_in = dc <= eps;
        let nxt_in = dn <= eps;

        if cur_in {
            out.push(poly[i]);
        }
        // Boundary crosses the plane between i and j: emit the intersection
        // point (after the current vertex if it was inside, giving correct
        // winding) and record it as a cap-edge endpoint.
        if cur_in != nxt_in {
            let denom = dc - dn;
            if denom.abs() > f64::EPSILON {
                let t = dc / denom;
                let ip = poly[i].add(poly[j].sub(poly[i]).scale(t));
                out.push(ip);
                crossings.push(ip);
            }
        }
    }
    (out, crossings)
}

/// Build the cap face that closes a cut, from the loose cut points of one clip
/// plane.
///
/// Points are welded to unique corners, ordered by angle about the plane normal
/// `n` (so the ring is a simple convex polygon), then wound so the cap's outward
/// normal matches `n`. Returns `None` if fewer than three distinct corners
/// remain (no real cap — the plane only grazed the solid).
fn build_cap(points: &[Vec3], n: Vec3, weld: f64) -> Option<Poly> {
    // Weld coincident cut points (each cap corner is generated by two faces).
    let mut uniq: Vec<Vec3> = Vec::new();
    for &p in points {
        if !uniq.iter().any(|&q| q.sub(p).length() <= weld) {
            uniq.push(p);
        }
    }
    if uniq.len() < 3 {
        return None;
    }

    // Centroid + an orthonormal in-plane basis (u, w) with u x w = n.
    let g = mean(&uniq);
    let (u, w) = plane_basis(n);
    uniq.sort_by(|a, b| {
        let aa = a.sub(g).dot(w).atan2(a.sub(g).dot(u));
        let ab = b.sub(g).dot(w).atan2(b.sub(g).dot(u));
        aa.partial_cmp(&ab).unwrap_or(std::cmp::Ordering::Equal)
    });

    // Ensure the winding gives an outward normal aligned with n.
    if newell_normal(&uniq).dot(n) < 0.0 {
        uniq.reverse();
    }
    Some(uniq)
}

/// Arithmetic mean of a set of points.
fn mean(points: &[Vec3]) -> Vec3 {
    let mut acc = Vec3::ZERO;
    for &p in points {
        acc = acc.add(p);
    }
    acc.scale(1.0 / points.len() as f64)
}

/// An orthonormal basis `(u, w)` spanning the plane with unit normal `n`, such
/// that `u x w = n` (right-handed with `n`).
fn plane_basis(n: Vec3) -> (Vec3, Vec3) {
    // Pick any axis not nearly parallel to n, then Gram–Schmidt it into the plane.
    let a = if n.x.abs() < 0.9 {
        Vec3::new(1.0, 0.0, 0.0)
    } else {
        Vec3::new(0.0, 1.0, 0.0)
    };
    let u = a.sub(n.scale(a.dot(n))).normalize();
    let w = n.cross(u);
    (u, w)
}

/// Newell-method normal of a polygon ring (not normalized; direction follows the
/// winding). Used to check/flip a cap's winding.
fn newell_normal(poly: &[Vec3]) -> Vec3 {
    let len = poly.len();
    let mut nx = 0.0;
    let mut ny = 0.0;
    let mut nz = 0.0;
    for i in 0..len {
        let cur = poly[i];
        let next = poly[(i + 1) % len];
        nx += (cur.y - next.y) * (cur.z + next.z);
        ny += (cur.z - next.z) * (cur.x + next.x);
        nz += (cur.x - next.x) * (cur.y + next.y);
    }
    Vec3::new(nx, ny, nz)
}

/// Weld a polygon soup into a `Mesh`: coincident corners (within `weld`) are
/// merged to one shared vertex so edges deduplicate and the result is a closed
/// manifold. Returns [`BooleanError::Unsupported`] if the welded result is not a
/// valid volume (fewer than four faces or vertices).
fn build_mesh(polys: &[Poly], weld: f64) -> Result<Mesh, BooleanError> {
    let mut positions: Vec<Vec3> = Vec::new();
    let mut faces: Vec<Vec<usize>> = Vec::new();

    for poly in polys {
        let mut idxs: Vec<usize> = Vec::new();
        for &p in poly {
            let idx = find_or_add(&mut positions, p, weld);
            // Drop consecutive duplicates (a vertex that landed exactly on a
            // clip plane can appear twice) to keep edges non-degenerate.
            if idxs.last() != Some(&idx) {
                idxs.push(idx);
            }
        }
        // Drop a wrap-around duplicate (first == last).
        if idxs.len() >= 2 && idxs.first() == idxs.last() {
            idxs.pop();
        }
        if idxs.len() >= 3 {
            faces.push(idxs);
        }
    }

    if faces.len() < 4 || positions.len() < 4 {
        return Err(BooleanError::Unsupported("empty or non-overlapping intersection"));
    }
    Ok(Mesh::from_polygons(&positions, &faces))
}

/// Find an existing welded vertex within `weld` of `p`, or append `p` and return
/// its new index.
fn find_or_add(positions: &mut Vec<Vec3>, p: Vec3, weld: f64) -> usize {
    for (i, &q) in positions.iter().enumerate() {
        if q.sub(p).length() <= weld {
            return i;
        }
    }
    positions.push(p);
    positions.len() - 1
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::primitives;

    /// Axis-aligned bounding box `(min, max)` of a mesh's vertices.
    fn bounds(m: &Mesh) -> (Vec3, Vec3) {
        let ps = m.positions();
        let mut lo = ps[0];
        let mut hi = ps[0];
        for &p in &ps {
            lo = Vec3::new(lo.x.min(p.x), lo.y.min(p.y), lo.z.min(p.z));
            hi = Vec3::new(hi.x.max(p.x), hi.y.max(p.y), hi.z.max(p.z));
        }
        (lo, hi)
    }

    fn approx(a: f64, b: f64, tol: f64) -> bool {
        (a - b).abs() <= tol
    }

    /// Rebuild a mesh with every vertex position mapped through `f` (used to
    /// translate/rotate an operand while preserving its faces/winding).
    fn map_positions(m: &Mesh, f: impl Fn(Vec3) -> Vec3) -> Mesh {
        let ps: Vec<Vec3> = m.positions().iter().map(|&p| f(p)).collect();
        let faces: Vec<Vec<usize>> = m
            .polygons()
            .iter()
            .map(|face| face.iter().map(|v| v.0).collect())
            .collect();
        Mesh::from_polygons(&ps, &faces)
    }

    /// A non-convex closed genus-0 mesh: an **L-shaped prism** (L cross-section
    /// in the z=0..1 slab). `V=12, E=18, F=8 -> chi = 2`. Its foot's top face
    /// (plane y=1, outward +y) has the tower vertices at y=2 on its outer side,
    /// so a convexity test that checks "every vertex inside every face plane"
    /// flags it.
    fn l_prism() -> Mesh {
        // L cross-section, CCW in xy (concave at D=(1,1)).
        let xy = [
            (0.0, 0.0), // A
            (2.0, 0.0), // B
            (2.0, 1.0), // C
            (1.0, 1.0), // D  (reflex corner)
            (1.0, 2.0), // E
            (0.0, 2.0), // F
        ];
        let mut positions: Vec<Vec3> = Vec::new();
        for &(x, y) in &xy {
            positions.push(Vec3::new(x, y, 0.0)); // bottom 0..5
        }
        for &(x, y) in &xy {
            positions.push(Vec3::new(x, y, 1.0)); // top 6..11
        }
        let n = xy.len();
        let mut faces: Vec<Vec<usize>> = Vec::new();
        // Bottom cap (outward -z): reverse of the CCW xy order.
        faces.push((0..n).rev().collect());
        // Top cap (outward +z): CCW xy order, shifted by n.
        faces.push((0..n).map(|i| i + n).collect());
        // Side quads: [b_i, b_{i+1}, t_{i+1}, t_i] -> outward normal.
        for i in 0..n {
            let j = (i + 1) % n;
            faces.push(vec![i, j, j + n, i + n]);
        }
        Mesh::from_polygons(&positions, &faces)
    }

    /// Methodology: intersect the axis-aligned cube `A = [-1,1]^3`
    /// (`primitives::cube(2.0)`) with a copy `B` translated by (1,1,1) so
    /// `B = [0,2]^3`. The exact convex intersection is the box `[0,1]^3`.
    /// Pass criterion: result is a closed box (`chi = 2`) whose bounding box is
    /// `[0,1]^3` to a tight tolerance.
    ///
    /// Results (analytically exact; asserted below): `chi = 2`; bounds
    /// `min = (0,0,0)`, `max = (1,1,1)`. Cut points fall exactly on integer
    /// coordinates (crossing parameter t = 1/2 on unit-symmetric edges), so the
    /// bounds are exact up to floating round-off; a 1e-9 tolerance is used.
    #[test]
    fn intersect_translated_cubes_is_unit_box() {
        let a = primitives::cube(2.0); // [-1,1]^3
        let b = map_positions(&primitives::cube(2.0), |p| {
            Vec3::new(p.x + 1.0, p.y + 1.0, p.z + 1.0)
        }); // [0,2]^3

        let out = boolean(&a, &b, BooleanMode::Intersect).expect("convex intersect should succeed");

        assert_eq!(out.euler_characteristic(), 2, "intersection must be a closed box (chi=2)");
        let (lo, hi) = bounds(&out);
        let tol = 1e-9;
        assert!(approx(lo.x, 0.0, tol) && approx(lo.y, 0.0, tol) && approx(lo.z, 0.0, tol), "min = {lo:?}");
        assert!(approx(hi.x, 1.0, tol) && approx(hi.y, 1.0, tol) && approx(hi.z, 1.0, tol), "max = {hi:?}");
    }

    /// Methodology: intersect `primitives::cube(2.0)` with an identical copy.
    /// The intersection of a convex body with itself is the body: `[-1,1]^3`.
    /// Pass criterion: `chi = 2` and bounds `[-1,1]^3` (idempotent bounds).
    ///
    /// Results (analytically exact; asserted below): `chi = 2`; bounds
    /// `min = (-1,-1,-1)`, `max = (1,1,1)`.
    #[test]
    fn intersect_cube_with_itself_is_idempotent() {
        let a = primitives::cube(2.0);
        let b = primitives::cube(2.0);

        let out = boolean(&a, &b, BooleanMode::Intersect).expect("self-intersect should succeed");

        assert_eq!(out.euler_characteristic(), 2);
        let (lo, hi) = bounds(&out);
        let tol = 1e-9;
        assert!(approx(lo.x, -1.0, tol) && approx(lo.y, -1.0, tol) && approx(lo.z, -1.0, tol), "min = {lo:?}");
        assert!(approx(hi.x, 1.0, tol) && approx(hi.y, 1.0, tol) && approx(hi.z, 1.0, tol), "max = {hi:?}");
    }

    /// Union and Difference now dispatch to the general pipeline
    /// ([`crate::boolean_general`]) instead of reporting `Unsupported`.
    /// Methodology: `a = cube(2.0)` (`[-1,1]^3`) with a fully-interior
    /// `b = cube(1.0)` (`[-0.5,0.5]^3`, no surface intersection). Union is the
    /// outer cube (volume 8, `chi = 2`); Difference is the outer cube with an
    /// interior cubic cavity (a two-shell solid, `chi = 4`: `2 + 2`). Pass
    /// criterion: both succeed with those topologies.
    #[test]
    fn union_and_difference_dispatch_to_general_path() {
        let a = primitives::cube(2.0);
        let b = primitives::cube(1.0);

        let uni = boolean(&a, &b, BooleanMode::Union).expect("union ok");
        assert_eq!(uni.euler_characteristic(), 2, "union of nested cubes is the outer cube (chi=2)");

        let diff = boolean(&a, &b, BooleanMode::Difference).expect("difference ok");
        assert_eq!(
            diff.euler_characteristic(),
            4,
            "cube-minus-interior-cube is a two-shell solid (chi = 2 + 2 = 4)"
        );
    }

    /// A non-convex operand is now **supported** (routed to the general
    /// pipeline), not rejected. Methodology: intersect the concave L-prism
    /// (cross-section area 3, height 1 -> volume 3, occupying
    /// `[0,2]x[0,2]x[0,1]`) with a **strictly** enclosing `cube(6.0)`
    /// (`[-3,3]^3` — the L-prism does not touch its boundary, so there is no
    /// coplanar shared face). The intersection is the L-prism itself: closed
    /// (`chi = 2`), same bounds. Pass criterion: succeeds; `chi = 2`; xy bounds
    /// `[0,2]`, z bounds `[0,1]`.
    #[test]
    fn nonconvex_operand_intersect_is_supported() {
        let a = l_prism();
        let b = primitives::cube(6.0);
        let out = boolean(&a, &b, BooleanMode::Intersect).expect("non-convex intersect should work");
        assert_eq!(out.euler_characteristic(), 2, "L-prism ∩ enclosing cube is the L-prism (chi=2)");
        let (lo, hi) = bounds(&out);
        let tol = 1e-9;
        assert!(approx(lo.x, 0.0, tol) && approx(hi.x, 2.0, tol), "x=({},{})", lo.x, hi.x);
        assert!(approx(lo.y, 0.0, tol) && approx(hi.y, 2.0, tol), "y=({},{})", lo.y, hi.y);
        assert!(approx(lo.z, 0.0, tol) && approx(hi.z, 1.0, tol), "z=({},{})", lo.z, hi.z);
    }

    /// Two disjoint convex meshes: the intersection is empty and must report
    /// `Unsupported`, not an empty/degenerate mesh. `A = [-1,1]^3`, `B` shifted
    /// by (10,0,0) so they do not overlap.
    #[test]
    fn disjoint_cubes_report_empty_intersection() {
        let a = primitives::cube(2.0);
        let b = map_positions(&primitives::cube(2.0), |p| Vec3::new(p.x + 10.0, p.y, p.z));
        let r = boolean(&a, &b, BooleanMode::Intersect);
        assert!(matches!(r, Err(BooleanError::Unsupported(_))), "disjoint intersect must be Unsupported, got {r:?}");
    }

    /// Best-effort robustness check on a rotated operand (limitation note in the
    /// module docs). Methodology: rotate `primitives::cube(2.0)` by 45 degrees
    /// about the z-axis and intersect with the axis-aligned `cube(2.0)`. The
    /// exact result is an **octagonal prism** (square-∩-rotated-square cross
    /// section, extruded over z in [-1,1]): `V=16, E=24, F=10 -> chi = 2`.
    ///
    /// Pass criterion / results (asserted below): non-empty; `chi = 2`; z-bounds
    /// exactly [-1,1]; the x/y extent stays within the axis cube ([-1,1]). This
    /// exercises non-axis-aligned clip planes and the angular cap ordering. If a
    /// future change breaks arbitrary-rotation robustness, restrict testing to
    /// the axis-aligned/translated cases above and note it here.
    #[test]
    fn intersect_rotated_and_axis_cube_is_octagonal_prism() {
        let angle = std::f64::consts::FRAC_PI_4; // 45 degrees
        let (s, c) = (angle.sin(), angle.cos());
        let a = primitives::cube(2.0);
        let b = map_positions(&primitives::cube(2.0), |p| {
            Vec3::new(p.x * c - p.y * s, p.x * s + p.y * c, p.z)
        });

        let out = boolean(&a, &b, BooleanMode::Intersect).expect("rotated convex intersect should succeed");

        assert_eq!(out.euler_characteristic(), 2, "octagonal prism must be closed (chi=2)");
        let (lo, hi) = bounds(&out);
        let tol = 1e-9;
        assert!(approx(lo.z, -1.0, tol) && approx(hi.z, 1.0, tol), "z-bounds = ({}, {})", lo.z, hi.z);
        // Intersection cannot extend beyond the axis cube.
        assert!(lo.x >= -1.0 - tol && hi.x <= 1.0 + tol, "x extent = ({}, {})", lo.x, hi.x);
        assert!(lo.y >= -1.0 - tol && hi.y <= 1.0 + tol, "y extent = ({}, {})", lo.y, hi.y);
    }
}
