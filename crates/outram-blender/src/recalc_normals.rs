//! Recalculate normals — make a mesh's face winding globally consistent and
//! outward-facing.
//!
//! This is the pure-Rust analogue of Blender's **Recalculate Normals Outside**
//! (`mesh.normals_make_consistent`). It repairs a polygon soup whose faces are
//! wound inconsistently — the common state of an imported or hand-assembled
//! mesh — so that adjacent faces agree on their shared edges and every
//! connected component's normals point outward.
//!
//! # Why this is not just [`crate::boolean_general`]'s orient-outward
//!
//! `boolean_general` flips a whole mesh by signed-volume sign, but only when it
//! is **already** consistently wound. This operator does the harder job first:
//! a breadth-first propagation across the face-adjacency graph that flips any
//! neighbour disagreeing on a shared edge, turning an inconsistent soup into a
//! consistently-wound one. Only then is each connected component flipped
//! outward.
//!
//! # Algorithm
//!
//! 1. **Adjacency.** Index faces by their undirected edges (sorted vertex
//!    pair). Two faces sharing an edge are adjacent.
//! 2. **Orientation propagation (BFS).** Seed each connected component with one
//!    face (unflipped). For a neighbour across a shared edge, the two faces are
//!    consistent iff they traverse that edge in **opposite** directions; if not,
//!    the neighbour is marked to flip. Directions compose by XOR, so the
//!    neighbour's flip is a closed-form function of the seed-relative
//!    orientation.
//! 3. **Outward pass.** For each connected component, compute the signed volume
//!    (divergence theorem) of its now-consistent faces; if negative, flip the
//!    whole component so its normals point outward.
//!
//! A non-orientable surface (e.g. a Möbius band) cannot be made globally
//! consistent — the BFS will visit it, but the result is best-effort along the
//! spanning tree, matching Blender's own behaviour. No `faer`, no external
//! dependency; Android-safe.

use crate::math::Vec3;
use crate::mesh::Mesh;
use std::collections::HashMap;

/// Return `mesh` with every face rewound so the surface is consistently wound
/// and each connected component faces outward (positive enclosed volume).
///
/// This is infallible. An already-correct mesh is returned with the same
/// winding; an inconsistently-wound soup is repaired; a fully-inward mesh is
/// flipped outward. Winding is the only thing changed — vertex positions and
/// the face-vertex sets are untouched.
///
/// # Examples
///
/// ```
/// use outram_blender::{primitives, recalc_normals::recalculate_normals};
///
/// // A pristine cube is already outward-wound, so this is a no-op on topology.
/// let cube = primitives::cube(2.0);
/// let fixed = recalculate_normals(&cube);
/// assert_eq!(fixed.vertex_count(), 8);
/// assert_eq!(fixed.face_count(), 6);
/// assert_eq!(fixed.euler_characteristic(), 2);
/// ```
pub fn recalculate_normals(mesh: &Mesh) -> Mesh {
    let positions = mesh.positions();
    let polygons: Vec<Vec<usize>> =
        mesh.polygons().iter().map(|p| p.iter().map(|v| v.0).collect()).collect();
    let nf = polygons.len();
    if nf == 0 {
        return Mesh::new();
    }

    // undirected edge (lo, hi) -> list of (face, orig_ab) where orig_ab is true
    // when the face traverses the edge lo -> hi in its original winding.
    let mut edge_faces: HashMap<(usize, usize), Vec<(usize, bool)>> = HashMap::new();
    for (f, poly) in polygons.iter().enumerate() {
        let k = poly.len();
        for i in 0..k {
            let a = poly[i];
            let b = poly[(i + 1) % k];
            let (lo, hi, ab) = if a < b { (a, b, true) } else { (b, a, false) };
            edge_faces.entry((lo, hi)).or_default().push((f, ab));
        }
    }

    // BFS orientation propagation. flip[f] = reverse this face's winding.
    let mut flip = vec![false; nf];
    let mut visited = vec![false; nf];
    // Per-face component id, for the outward pass.
    let mut component = vec![usize::MAX; nf];
    let mut components: Vec<Vec<usize>> = Vec::new();

    // orig_ab for a (face, edge): recover from edge_faces lookups on demand.
    let orig_ab_of = |f: usize, lo: usize, hi: usize| -> bool {
        edge_faces[&(lo, hi)].iter().find(|(g, _)| *g == f).map(|(_, ab)| *ab).unwrap()
    };

    for seed in 0..nf {
        if visited[seed] {
            continue;
        }
        let comp_id = components.len();
        let mut comp = Vec::new();
        visited[seed] = true;
        component[seed] = comp_id;
        let mut queue = std::collections::VecDeque::new();
        queue.push_back(seed);
        while let Some(f) = queue.pop_front() {
            comp.push(f);
            let poly = &polygons[f];
            let k = poly.len();
            for i in 0..k {
                let a = poly[i];
                let b = poly[(i + 1) % k];
                let (lo, hi) = if a < b { (a, b) } else { (b, a) };
                // final_ab(f) = orig_ab(f) XOR flip[f].
                let final_ab_f = orig_ab_of(f, lo, hi) ^ flip[f];
                for &(g, _) in &edge_faces[&(lo, hi)] {
                    if g == f || visited[g] {
                        continue;
                    }
                    // Want final_ab(g) = !final_ab(f); flip[g] = orig_ab(g) XOR that.
                    let orig_ab_g = orig_ab_of(g, lo, hi);
                    flip[g] = orig_ab_g ^ (!final_ab_f);
                    visited[g] = true;
                    component[g] = comp_id;
                    queue.push_back(g);
                }
            }
        }
        components.push(comp);
    }

    // Outward pass: per component, flip all if signed volume is negative.
    for comp in &components {
        let mut vol6 = 0.0;
        for &f in comp {
            vol6 += face_signed_volume6(&positions, &polygons[f], flip[f]);
        }
        if vol6 < 0.0 {
            for &f in comp {
                flip[f] = !flip[f];
            }
        }
    }

    // Rebuild with flips applied.
    let new_faces: Vec<Vec<usize>> = polygons
        .iter()
        .enumerate()
        .map(|(f, poly)| {
            if flip[f] {
                poly.iter().rev().copied().collect()
            } else {
                poly.clone()
            }
        })
        .collect();

    Mesh::from_polygons(&positions, &new_faces)
}

/// Six times the signed volume contribution of one face (fan-triangulated),
/// with the face optionally reversed. Summed over a closed surface this is
/// `6·V`; its sign tells inward (`< 0`) from outward (`> 0`).
fn face_signed_volume6(positions: &[Vec3], poly: &[usize], flip: bool) -> f64 {
    let k = poly.len();
    if k < 3 {
        return 0.0;
    }
    let idx = |i: usize| if flip { poly[k - 1 - i] } else { poly[i] };
    let p0 = positions[idx(0)];
    let mut acc = 0.0;
    for i in 1..k - 1 {
        let v1 = positions[idx(i)];
        let v2 = positions[idx(i + 1)];
        acc += p0.dot(v1.cross(v2));
    }
    acc
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::primitives;
    use std::collections::HashSet;

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

    /// Signed volume × 6 of a mesh as-wound (positive = outward).
    fn signed_volume6(mesh: &Mesh) -> f64 {
        let ps = mesh.positions();
        let mut v = 0.0;
        for poly in mesh.polygons() {
            let idx: Vec<usize> = poly.iter().map(|x| x.0).collect();
            v += super::face_signed_volume6(&ps, &idx, false);
        }
        v
    }

    /// Build a cube with the faces in `reversed` wound inward (inconsistent).
    fn cube_with_reversed(reversed: &[usize]) -> Mesh {
        let cube = primitives::cube(2.0);
        let ps = cube.positions();
        let faces: Vec<Vec<usize>> = cube
            .polygons()
            .into_iter()
            .enumerate()
            .map(|(i, poly)| {
                let v: Vec<usize> = poly.into_iter().map(|x| x.0).collect();
                if reversed.contains(&i) {
                    v.into_iter().rev().collect()
                } else {
                    v
                }
            })
            .collect();
        Mesh::from_polygons(&ps, &faces)
    }

    /// V&V — headline. Methodology: take `cube(2.0)` and reverse faces {0,2,4}
    /// so the surface is inconsistently wound (some directed half-edges have no
    /// reverse partner). Recalculate. Pass criterion: fully consistent winding
    /// and outward-facing. Result: `is_watertight_consistent` holds and the
    /// signed volume is positive (+8 for a side-2 cube), from an input that
    /// failed both.
    #[test]
    fn inconsistent_cube_is_repaired_outward() {
        let bad = cube_with_reversed(&[0, 2, 4]);
        assert!(!is_watertight_consistent(&bad), "input is inconsistently wound");

        let fixed = recalculate_normals(&bad);
        assert_eq!(fixed.face_count(), 6);
        assert_eq!(fixed.euler_characteristic(), 2);
        assert!(is_watertight_consistent(&fixed), "repaired to consistent winding");
        let v6 = signed_volume6(&fixed);
        assert!(v6 > 0.0, "normals point outward (signed volume > 0)");
        assert!((v6 - 48.0).abs() < 1e-9, "6·V = 6·(2³) = 48 for a side-2 cube");
    }

    /// V&V — an all-inward cube is flipped outward. Reversing every face gives a
    /// consistently-wound but inward mesh (negative volume); recalc flips it.
    #[test]
    fn all_inward_cube_flips_outward() {
        let inward = cube_with_reversed(&[0, 1, 2, 3, 4, 5]);
        assert!(is_watertight_consistent(&inward), "uniform reverse is still consistent");
        assert!(signed_volume6(&inward) < 0.0, "but inward (negative volume)");

        let fixed = recalculate_normals(&inward);
        assert!(is_watertight_consistent(&fixed));
        assert!(signed_volume6(&fixed) > 0.0, "flipped outward");
    }

    /// V&V — an already-correct cube keeps its (outward, consistent) winding.
    #[test]
    fn correct_cube_is_unchanged() {
        let cube = primitives::cube(2.0);
        let fixed = recalculate_normals(&cube);
        assert!(is_watertight_consistent(&fixed));
        assert!((signed_volume6(&fixed) - 48.0).abs() < 1e-9);
        assert_eq!(fixed.face_count(), 6);
        assert_eq!(fixed.euler_characteristic(), 2);
    }

    /// V&V — the `MeshOp::RecalculateNormals` dispatch matches the free function.
    #[test]
    fn meshop_recalc_matches_free_function() {
        use crate::ops::MeshOp;
        let bad = cube_with_reversed(&[1, 3]);
        let direct = recalculate_normals(&bad);
        let via_op = MeshOp::RecalculateNormals.apply(bad).expect("recalc infallible");
        assert_eq!(via_op.face_count(), direct.face_count());
        assert_eq!(via_op.euler_characteristic(), direct.euler_characteristic());
        assert!(is_watertight_consistent(&via_op));
    }
}
