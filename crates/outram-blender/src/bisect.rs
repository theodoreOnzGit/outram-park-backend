//! Bisect — cut a mesh by a plane and keep one half.
//!
//! This is the pure-Rust analogue of Blender's **Bisect**: a plane (a point and
//! a normal) slices the mesh, and the half on the normal-negative side —
//! `n · (x − point) <= 0` — is kept. Every face is clipped against that
//! half-space with the Sutherland–Hodgman algorithm, so faces straddling the
//! plane are cut cleanly and faces fully on the discarded side vanish.
//!
//! The cut is left **open**: bisect does not cap the exposed section. That is
//! deliberate — it composes with [`crate::fill_holes`], which caps the planar
//! boundary loop into a watertight solid. The pair `bisect` then `fill_holes`
//! is the half-space-cut primitive the CSG / Monte-Carlo workflow builds on.
//!
//! # Shared crossing vertices
//!
//! Where the plane crosses an edge, one new vertex is created and **shared** by
//! both faces on that edge — the crossing point is keyed by the undirected
//! original edge, so the result stays manifold along the cut rather than
//! splitting into a crack. No `faer`, no external dependency; Android-safe.

use crate::math::Vec3;
use crate::mesh::Mesh;
use std::collections::HashMap;

/// One clipped-polygon vertex: either an original mesh vertex that survived, or
/// a new point where the plane crossed an original edge.
enum OutVertex {
    /// An original vertex index (on the kept side or exactly on the plane).
    Orig(usize),
    /// The plane crossing on the undirected original edge `(lo, hi)`.
    Cross(usize, usize),
}

/// Cut `mesh` by the plane through `point` with the given `normal`, keeping the
/// half where `normal · (x − point) <= 0`.
///
/// `normal` need not be unit length (only its sign and direction matter).
/// Faces straddling the plane are clipped; the exposed cut is left open (cap it
/// with [`crate::fill_holes::fill_holes`] for a closed solid). If the whole
/// mesh is on the kept side the mesh is returned unchanged; if none of it is,
/// an empty mesh is returned. This is infallible.
///
/// # Examples
///
/// ```
/// use outram_blender::{primitives, bisect::bisect, fill_holes::fill_holes, math::Vec3};
///
/// // Slice a cube [-1,1]³ at z = 0, keeping the lower half, then cap it:
/// // the result is a closed box of half the volume.
/// let cube = primitives::cube(2.0);
/// let lower = bisect(&cube, Vec3::ZERO, Vec3::new(0.0, 0.0, 1.0));
/// let solid = fill_holes(&lower);
/// assert_eq!(solid.euler_characteristic(), 2);
/// ```
pub fn bisect(mesh: &Mesh, point: Vec3, normal: Vec3) -> Mesh {
    let positions = mesh.positions();
    let polygons = mesh.polygons();

    // Signed distance to the plane (sign is what matters, not the scale).
    let sdist = |p: Vec3| normal.dot(p.sub(point));

    let mut out_faces: Vec<Vec<OutVertex>> = Vec::new();
    for poly in &polygons {
        let k = poly.len();
        if k < 3 {
            continue;
        }
        let mut clipped: Vec<OutVertex> = Vec::new();
        for i in 0..k {
            let cur = poly[i].0;
            let nxt = poly[(i + 1) % k].0;
            let sc = sdist(positions[cur]);
            let sn = sdist(positions[nxt]);
            let cur_in = sc <= 0.0;
            let nxt_in = sn <= 0.0;
            if cur_in {
                clipped.push(OutVertex::Orig(cur));
            }
            // Edge strictly crosses the plane (one endpoint each side).
            if cur_in != nxt_in {
                let (lo, hi) = if cur < nxt { (cur, nxt) } else { (nxt, cur) };
                clipped.push(OutVertex::Cross(lo, hi));
            }
        }
        if clipped.len() >= 3 {
            out_faces.push(clipped);
        }
    }

    // Compact: assign output indices to the original vertices and crossings that
    // are actually referenced. Crossings are computed once per undirected edge.
    let mut new_positions: Vec<Vec3> = Vec::new();
    let mut orig_remap: HashMap<usize, usize> = HashMap::new();
    let mut cross_remap: HashMap<(usize, usize), usize> = HashMap::new();

    let mut resolve = |ov: &OutVertex,
                       new_positions: &mut Vec<Vec3>|
     -> usize {
        match *ov {
            OutVertex::Orig(v) => *orig_remap.entry(v).or_insert_with(|| {
                new_positions.push(positions[v]);
                new_positions.len() - 1
            }),
            OutVertex::Cross(a, b) => *cross_remap.entry((a, b)).or_insert_with(|| {
                // Interpolate the crossing: t = s(a) / (s(a) − s(b)).
                let sa = sdist(positions[a]);
                let sb = sdist(positions[b]);
                let t = sa / (sa - sb);
                let p = positions[a].add(positions[b].sub(positions[a]).scale(t));
                new_positions.push(p);
                new_positions.len() - 1
            }),
        }
    };

    let faces: Vec<Vec<usize>> = out_faces
        .iter()
        .map(|f| f.iter().map(|ov| resolve(ov, &mut new_positions)).collect())
        .collect();

    Mesh::from_polygons(&new_positions, &faces)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::fill_holes::fill_holes;
    use crate::primitives;
    use std::collections::HashSet;

    /// Six times the signed volume of the mesh as-wound (positive = outward).
    fn signed_volume6(mesh: &Mesh) -> f64 {
        let ps = mesh.positions();
        let mut v = 0.0;
        for poly in mesh.polygons() {
            let p0 = ps[poly[0].0];
            for i in 1..poly.len() - 1 {
                v += p0.dot(ps[poly[i].0].cross(ps[poly[i + 1].0]));
            }
        }
        v
    }

    /// True when every directed half-edge is unique and its reverse is present.
    fn is_watertight_consistent(mesh: &Mesh) -> bool {
        let mut directed: HashSet<(usize, usize)> = HashSet::new();
        for poly in mesh.polygons() {
            let k = poly.len();
            for i in 0..k {
                let a = poly[i].0;
                let b = poly[(i + 1) % k].0;
                if !directed.insert((a, b)) {
                    return false;
                }
            }
        }
        directed.iter().all(|&(a, b)| directed.contains(&(b, a)))
    }

    /// V&V — headline. Methodology: bisect `cube(2.0)` (spanning [−1,1]³) at the
    /// plane z = 0 with normal +z, keeping the z ≤ 0 half, then cap the open cut
    /// with `fill_holes`. Pass criterion: a closed solid of exactly half the
    /// cube's volume. Result: the capped mesh is watertight, χ = 2, and its
    /// signed volume is 4 (half of 2³ = 8) — measured 6·V = 24.
    #[test]
    fn lower_half_of_cube_has_half_the_volume() {
        let cube = primitives::cube(2.0);
        let lower = bisect(&cube, Vec3::ZERO, Vec3::new(0.0, 0.0, 1.0));
        // Every kept vertex is on or below the plane.
        assert!(lower.positions().iter().all(|p| p.z <= 1e-12), "kept half is z <= 0");
        // The cut is open until we cap it.
        assert!(!is_watertight_consistent(&lower), "bisect leaves the cut open");

        let solid = fill_holes(&lower);
        assert!(is_watertight_consistent(&solid), "capped cut is watertight");
        assert_eq!(solid.euler_characteristic(), 2, "closed genus-0 solid");
        let v6 = signed_volume6(&solid);
        assert!((v6 - 24.0).abs() < 1e-9, "6·V = 6·4 = 24 (half of the cube)");
    }

    /// V&V — a plane fully outside the mesh keeps everything. With the whole
    /// cube on the kept side, bisect returns it unchanged.
    #[test]
    fn plane_outside_keeps_everything() {
        let cube = primitives::cube(2.0);
        // Plane at z = 5, normal +z: every vertex has z ≤ 5, so all are kept.
        let kept = bisect(&cube, Vec3::new(0.0, 0.0, 5.0), Vec3::new(0.0, 0.0, 1.0));
        assert_eq!(kept.vertex_count(), 8);
        assert_eq!(kept.face_count(), 6);
        assert_eq!(kept.euler_characteristic(), 2);
        assert!(is_watertight_consistent(&kept));
    }

    /// V&V — a plane with the mesh entirely on the discarded side yields an
    /// empty mesh.
    #[test]
    fn plane_discarding_everything_is_empty() {
        let cube = primitives::cube(2.0);
        // Keep z ≤ −5: nothing qualifies.
        let empty = bisect(&cube, Vec3::new(0.0, 0.0, -5.0), Vec3::new(0.0, 0.0, 1.0));
        assert_eq!(empty.vertex_count(), 0);
        assert_eq!(empty.face_count(), 0);
    }

    /// V&V — the `MeshOp::Bisect` dispatch matches the free function.
    #[test]
    fn meshop_bisect_matches_free_function() {
        use crate::ops::MeshOp;
        let cube = primitives::cube(2.0);
        let n = Vec3::new(1.0, 0.0, 0.0);
        let direct = bisect(&cube, Vec3::ZERO, n);
        let via_op =
            MeshOp::Bisect { point: Vec3::ZERO, normal: n }.apply(cube).expect("bisect infallible");
        assert_eq!(via_op.vertex_count(), direct.vertex_count());
        assert_eq!(via_op.face_count(), direct.face_count());
    }
}
