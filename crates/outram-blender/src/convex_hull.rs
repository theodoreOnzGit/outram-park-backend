//! **3D convex hull** of a point set — the incremental algorithm on the robust
//! [`crate::boolean_predicates::orient3d`] orientation test.
//!
//! Given a set of points, [`convex_hull`] returns the closed, watertight,
//! outward-wound triangle [`Mesh`] of their convex hull (the smallest convex
//! solid containing them all). It complements the CSG / boolean suite — a hull
//! is the natural bounding volume, and the input to a convex decomposition.
//!
//! ## Method (incremental)
//!
//! Start from a non-degenerate tetrahedron of four input points (chosen for good
//! conditioning: a farthest pair, the point of largest triangle area with that
//! edge, then the point farthest off their plane), with all four faces wound
//! **CCW as seen from outside**. Then add the remaining points one at a time:
//!
//! - a face `(a, b, c)` is **visible** from a new point `p` iff `p` is on its
//!   outward side, i.e. `orient3d(a, b, c, p) == -1` (the crate's outward normal
//!   is the "above" side of `orient3d`, so "above" = `-1` = outward);
//! - if no face is visible, `p` is inside-or-on the hull and is skipped;
//! - otherwise the **horizon** (the loop of edges bordering a visible and a
//!   non-visible face) is found from the visible faces' directed edges — an edge
//!   `(a, b)` is on the horizon iff its reverse `(b, a)` is not also a visible
//!   edge — the visible faces are deleted, and each horizon edge `(a, b)` is
//!   coned to `p` as a new triangle `[a, b, p]` (already outward-wound).
//!
//! All orientation decisions use the exact/robust `orient3d`, so the hull is
//! correct even for near-degenerate configurations that a naive `f64` sign would
//! misjudge. Coplanar points are handled by the strict `== -1` visibility rule
//! (a coplanar face is never deleted, so no sliver/duplicate faces arise; a
//! coplanar cube face still splits into two triangles via its non-coplanar
//! neighbours).
//!
//! ## Degeneracies
//!
//! Duplicate points are removed first. Fewer than four distinct points, an
//! all-collinear set, or an all-coplanar set have no 3D hull and return a
//! [`HullError`] rather than a degenerate/open mesh.
//!
//! > **Untrusted AI-generated draft** until a human reviews it, per the
//! > workspace `RESPONSIBLE_USE.md`. Not for nuclear facility operation,
//! > reactor control, safety-critical, or licensing decisions.

use crate::boolean_predicates::orient3d;
use crate::math::Vec3;
use crate::mesh::Mesh;
use std::collections::{HashMap, HashSet};

/// Errors from [`convex_hull`].
#[derive(Debug, thiserror::Error)]
pub enum HullError {
    /// Fewer than four **distinct** points were supplied — a tetrahedron (the
    /// minimal 3D hull) needs four.
    #[error("convex hull needs at least 4 distinct points, got {0}")]
    NotEnoughPoints(usize),
    /// Every point is collinear — the hull would be a line segment, not a solid.
    #[error("convex hull undefined: all input points are collinear")]
    Collinear,
    /// Every point is coplanar — the hull would be a flat polygon, not a solid.
    #[error("convex hull undefined: all input points are coplanar")]
    Coplanar,
}

/// The convex hull of `points`, as a closed outward-wound triangle [`Mesh`].
///
/// Every input point lies on or inside the returned hull; the result is a
/// watertight genus-0 mesh (`V − E + F = 2`) with all faces wound
/// counter-clockwise as seen from outside. See the module docs for the method.
///
/// # Errors
///
/// [`HullError::NotEnoughPoints`] for fewer than four distinct points,
/// [`HullError::Collinear`] / [`HullError::Coplanar`] for a degenerate (lower
/// dimensional) input.
pub fn convex_hull(points: &[Vec3]) -> Result<Mesh, HullError> {
    let pts = dedup(points);
    if pts.len() < 4 {
        return Err(HullError::NotEnoughPoints(pts.len()));
    }

    let [i0, i1, i2, i3] = initial_tetra(&pts)?;

    // Four outward-wound faces of the seed tetrahedron.
    let verts = [i0, i1, i2, i3];
    let mut faces: Vec<[usize; 3]> = Vec::new();
    for k in 0..4 {
        let o = verts[k];
        let others: Vec<usize> = (0..4).filter(|&m| m != k).map(|m| verts[m]).collect();
        faces.push(orient_outward(&pts, others[0], others[1], others[2], o));
    }

    let seed: HashSet<usize> = verts.iter().copied().collect();
    for pi in 0..pts.len() {
        if seed.contains(&pi) {
            continue;
        }
        add_point(&pts, &mut faces, pi);
    }

    Ok(build_mesh(&pts, &faces))
}

/// Add point `pi` to the running hull `faces` (no-op if inside).
fn add_point(pts: &[Vec3], faces: &mut Vec<[usize; 3]>, pi: usize) {
    let p = pts[pi];
    // Visible faces: p strictly on the outward side (orient3d == -1).
    let mut visible = vec![false; faces.len()];
    let mut any = false;
    for (fi, &[a, b, c]) in faces.iter().enumerate() {
        if orient3d(pts[a], pts[b], pts[c], p) == -1 {
            visible[fi] = true;
            any = true;
        }
    }
    if !any {
        return; // p is inside or on the hull
    }

    // Directed edges of the visible region.
    let mut vis_edges: HashSet<(usize, usize)> = HashSet::new();
    for (fi, &[a, b, c]) in faces.iter().enumerate() {
        if visible[fi] {
            vis_edges.insert((a, b));
            vis_edges.insert((b, c));
            vis_edges.insert((c, a));
        }
    }
    // Horizon edges: directed edges whose reverse is not also a visible edge.
    let mut horizon: Vec<(usize, usize)> = Vec::new();
    for &(a, b) in &vis_edges {
        if !vis_edges.contains(&(b, a)) {
            horizon.push((a, b));
        }
    }

    // Keep only non-visible faces, then cone the horizon to p.
    let mut kept: Vec<[usize; 3]> = faces
        .iter()
        .zip(visible.iter())
        .filter(|(_, &v)| !v)
        .map(|(&f, _)| f)
        .collect();
    for (a, b) in horizon {
        kept.push([a, b, pi]); // already outward-wound (see module docs)
    }
    *faces = kept;
}

// ---------------------------------------------------------------------------
// Seed tetrahedron + helpers.
// ---------------------------------------------------------------------------

/// Choose four well-conditioned, non-coplanar seed points, or error if the
/// input is collinear / coplanar.
fn initial_tetra(pts: &[Vec3]) -> Result<[usize; 4], HullError> {
    let i0 = 0usize;
    // i1: farthest from i0.
    let i1 = (0..pts.len())
        .max_by(|&a, &b| {
            pts[a].sub(pts[i0]).length().total_cmp(&pts[b].sub(pts[i0]).length())
        })
        .unwrap();
    if i1 == i0 {
        return Err(HullError::NotEnoughPoints(pts.len()));
    }

    // i2: largest triangle area with edge (i0, i1).
    let e01 = pts[i1].sub(pts[i0]);
    let (i2, area) = (0..pts.len())
        .map(|k| (k, e01.cross(pts[k].sub(pts[i0])).length()))
        .max_by(|a, b| a.1.total_cmp(&b.1))
        .unwrap();
    let scale = bbox_diagonal(pts).max(1.0);
    if area <= 1e-12 * scale * scale {
        return Err(HullError::Collinear);
    }

    // i3: farthest off the plane (i0, i1, i2), confirmed non-coplanar robustly.
    let n = e01.cross(pts[i2].sub(pts[i0]));
    let i3 = (0..pts.len())
        .max_by(|&a, &b| {
            n.dot(pts[a].sub(pts[i0])).abs().total_cmp(&n.dot(pts[b].sub(pts[i0])).abs())
        })
        .unwrap();
    if orient3d(pts[i0], pts[i1], pts[i2], pts[i3]) == 0 {
        return Err(HullError::Coplanar);
    }
    Ok([i0, i1, i2, i3])
}

/// Wind triangle `(a, b, c)` so its outward side is away from the opposite
/// vertex `o` (i.e. `o` ends up strictly *below* / inside the face).
fn orient_outward(pts: &[Vec3], a: usize, b: usize, c: usize, o: usize) -> [usize; 3] {
    if orient3d(pts[a], pts[b], pts[c], pts[o]) == -1 {
        // o is on the outward side → flip so the face points the other way.
        [a, c, b]
    } else {
        [a, b, c]
    }
}

/// Exact-coordinate dedup (by `f64` bit pattern), preserving first-seen order.
fn dedup(points: &[Vec3]) -> Vec<Vec3> {
    let mut seen: HashSet<(u64, u64, u64)> = HashSet::new();
    let mut out = Vec::with_capacity(points.len());
    for &p in points {
        let key = (p.x.to_bits(), p.y.to_bits(), p.z.to_bits());
        if seen.insert(key) {
            out.push(p);
        }
    }
    out
}

/// Bounding-box diagonal length of a point set (a size scale for tolerances).
fn bbox_diagonal(pts: &[Vec3]) -> f64 {
    let mut lo = pts[0];
    let mut hi = pts[0];
    for &p in pts {
        lo = Vec3::new(lo.x.min(p.x), lo.y.min(p.y), lo.z.min(p.z));
        hi = Vec3::new(hi.x.max(p.x), hi.y.max(p.y), hi.z.max(p.z));
    }
    hi.sub(lo).length()
}

/// Compact the referenced points and rebuild a [`Mesh`] (dropping interior
/// points, so `from_polygons` does not resurrect them as isolated vertices).
fn build_mesh(pts: &[Vec3], faces: &[[usize; 3]]) -> Mesh {
    let mut remap: HashMap<usize, usize> = HashMap::new();
    let mut positions: Vec<Vec3> = Vec::new();
    let mut out_faces: Vec<Vec<usize>> = Vec::with_capacity(faces.len());
    for &[a, b, c] in faces {
        let face: Vec<usize> = [a, b, c]
            .into_iter()
            .map(|v| {
                *remap.entry(v).or_insert_with(|| {
                    positions.push(pts[v]);
                    positions.len() - 1
                })
            })
            .collect();
        out_faces.push(face);
    }
    Mesh::from_polygons(&positions, &out_faces)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Signed volume via the divergence theorem: `(1/6) Σ v0·(v1×v2)` over the
    /// triangles. Positive for an outward-wound closed mesh.
    fn signed_volume(m: &Mesh) -> f64 {
        let ps = m.positions();
        let mut v = 0.0;
        for face in m.polygons() {
            let a = ps[face[0].0];
            for k in 1..face.len() - 1 {
                v += a.dot(ps[face[k].0].cross(ps[face[k + 1].0]));
            }
        }
        v / 6.0
    }

    /// Every input point is inside-or-on the hull: no face sees it strictly
    /// outside (`orient3d == -1`).
    fn assert_contains_all(m: &Mesh, points: &[Vec3]) {
        let ps = m.positions();
        for &q in points {
            for face in m.polygons() {
                let (a, b, c) = (ps[face[0].0], ps[face[1].0], ps[face[2].0]);
                assert!(orient3d(a, b, c, q) != -1, "point {q:?} is outside a hull face");
            }
        }
    }

    /// The hull is convex: every hull vertex is on the inner side (or in-plane)
    /// of every face.
    fn assert_convex(m: &Mesh) {
        let ps = m.positions();
        for face in m.polygons() {
            let (a, b, c) = (ps[face[0].0], ps[face[1].0], ps[face[2].0]);
            for &v in &ps {
                assert!(orient3d(a, b, c, v) >= 0, "non-convex: a vertex is outside a face");
            }
        }
    }

    /// Deterministic xorshift point cloud in `[-1, 1]^3` (radius-clamped by the
    /// caller if a sphere is wanted).
    fn xorshift_points(n: usize) -> Vec<Vec3> {
        let mut s: u64 = 0x9e3779b97f4a7c15;
        let mut next = || {
            s ^= s << 13;
            s ^= s >> 7;
            s ^= s << 17;
            ((s >> 11) as f64 / (1u64 << 53) as f64) * 2.0 - 1.0
        };
        (0..n).map(|_| Vec3::new(next(), next(), next())).collect()
    }

    /// Methodology: **hull of a cube's 8 corners is the cube.** The convex hull
    /// of the eight corners of a side-`s` cube must be that cube, triangulated:
    /// `V = 8`, `F = 12` (each square face split into two triangles), `E = 18`,
    /// `chi = 2`, watertight, outward-wound with signed volume `s³`.
    ///
    /// Results (asserted): `V=8, F=12, E=18, chi=2`, watertight
    /// (`loop_count = 36 = 3F`), volume `= 8` for `s = 2` within `1e-9`, and
    /// every corner contained.
    #[test]
    fn hull_of_cube_corners_is_the_cube() {
        let s = 2.0;
        let h = s / 2.0;
        let corners: Vec<Vec3> = (0..8)
            .map(|i| {
                Vec3::new(
                    if i & 1 == 0 { -h } else { h },
                    if i & 2 == 0 { -h } else { h },
                    if i & 4 == 0 { -h } else { h },
                )
            })
            .collect();
        let hull = convex_hull(&corners).expect("cube corners have a hull");

        assert_eq!(hull.vertex_count(), 8, "cube hull has 8 vertices");
        assert_eq!(hull.face_count(), 12, "cube hull is 12 triangles");
        assert_eq!(hull.edge_count(), 18);
        assert_eq!(hull.euler_characteristic(), 2, "closed (chi=2)");
        assert_eq!(hull.loop_count(), 36, "watertight: every edge borders two faces");
        assert!((signed_volume(&hull) - 8.0).abs() < 1e-9, "volume = s^3 = 8");
        assert_contains_all(&hull, &corners);
        assert_convex(&hull);
    }

    /// Methodology: **hull of the six octahedron vertices is the octahedron.**
    /// `(±1,0,0),(0,±1,0),(0,0,±1)` → `V=6, F=8, E=12, chi=2`, volume `4/3`.
    #[test]
    fn hull_of_octahedron_points() {
        let pts = [
            Vec3::new(1.0, 0.0, 0.0),
            Vec3::new(-1.0, 0.0, 0.0),
            Vec3::new(0.0, 1.0, 0.0),
            Vec3::new(0.0, -1.0, 0.0),
            Vec3::new(0.0, 0.0, 1.0),
            Vec3::new(0.0, 0.0, -1.0),
        ];
        let hull = convex_hull(&pts).expect("octahedron hull");
        assert_eq!(hull.vertex_count(), 6);
        assert_eq!(hull.face_count(), 8);
        assert_eq!(hull.euler_characteristic(), 2);
        assert!((signed_volume(&hull) - 4.0 / 3.0).abs() < 1e-9, "octahedron volume 4/3");
        assert_convex(&hull);
    }

    /// Methodology: **hull of a random point cloud contains every point and is
    /// convex.** 200 deterministic points in `[-1,1]^3`; the hull must be a
    /// closed (`chi=2`), convex solid containing all inputs.
    ///
    /// Results (asserted): `chi = 2`; every input point inside-or-on; every hull
    /// vertex on the inner side of every face (convex); positive volume.
    #[test]
    fn hull_of_random_cloud_is_convex_and_contains_all() {
        let pts = xorshift_points(200);
        let hull = convex_hull(&pts).expect("random cloud hull");
        assert_eq!(hull.euler_characteristic(), 2, "closed hull (chi=2)");
        assert!(signed_volume(&hull) > 0.0, "outward-wound positive volume");
        assert_contains_all(&hull, &pts);
        assert_convex(&hull);
    }

    /// Degenerate inputs error rather than returning a broken mesh.
    #[test]
    fn degenerate_inputs_error() {
        // Fewer than 4 distinct points (duplicates collapse to 1).
        let dup = vec![Vec3::new(1.0, 2.0, 3.0); 5];
        assert!(matches!(convex_hull(&dup), Err(HullError::NotEnoughPoints(1))));

        // Collinear (all on the x-axis).
        let line: Vec<Vec3> = (0..6).map(|i| Vec3::new(i as f64, 0.0, 0.0)).collect();
        assert!(matches!(convex_hull(&line), Err(HullError::Collinear)));

        // Coplanar (all in z = 0).
        let plane = vec![
            Vec3::new(0.0, 0.0, 0.0),
            Vec3::new(1.0, 0.0, 0.0),
            Vec3::new(0.0, 1.0, 0.0),
            Vec3::new(1.0, 1.0, 0.0),
            Vec3::new(2.0, 3.0, 0.0),
        ];
        assert!(matches!(convex_hull(&plane), Err(HullError::Coplanar)));
    }
}
