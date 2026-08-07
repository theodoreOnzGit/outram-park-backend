// SPDX-License-Identifier: GPL-3.0-only
// Copyright (C) 2026 OUTRAM PARK contributors
//
// Algorithm references (re-implemented in Rust, not transcribed):
//
//   [1] Robust geometric predicates `orient3d` and `insphere`.
//       Jonathan Richard Shewchuk, "Adaptive Precision Floating-Point
//       Arithmetic and Fast Robust Geometric Predicates", Discrete &
//       Computational Geometry 18(3):305-363, 1997, and the accompanying
//       `predicates.c` (Carnegie Mellon University).
//       https://www.cs.cmu.edu/~quake/robust.html
//       Shewchuk places `predicates.c` in the public domain for any use.
//       ATTRIBUTION NOTE: only the *determinant formulations* of orient3d and
//       insphere are taken from that work. Shewchuk's adaptive exact-arithmetic
//       expansions are NOT implemented here — these are plain f64 evaluations
//       of the same determinants, so they are fast but not exact, and are
//       therefore NOT a substitute for `predicates.c` on degenerate input.
//
//   [2] Bistellar (Lawson) 2->3 / 3->2 flips.
//       C. L. Lawson, "Software for C1 surface interpolation", in Mathematical
//       Software III, Academic Press, 1977; and B. Joe, "Construction of
//       three-dimensional Delaunay triangulations using local transformations",
//       CAGD 8(2):123-142, 1991. Standard published technique, no code reused.
//
// This file is part of OUTRAM PARK.
//
// OUTRAM PARK is free software: you can redistribute it and/or modify it
// under the terms of the GNU General Public License as published by the
// Free Software Foundation, either version 3 of the License, or (at your
// option) any later version.
//
// OUTRAM PARK is distributed in the hope that it will be useful, but
// WITHOUT ANY WARRANTY; without even the implied warranty of
// MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE.  See the GNU
// General Public License for more details.
//
// You should have received a copy of the GNU General Public License along
// with OUTRAM PARK.  If not, see <https://www.gnu.org/licenses/>.

//! **Flip-based Delaunay** improvement of a tetrahedral mesh.
//!
//! The centroid-subdivision [`crate::tet::tetrahedralize`] produces a valid,
//! space-filling tet mesh, but not a *Delaunay* one — some interior faces fail
//! the empty-circumsphere (locally-Delaunay) test, which is what leaves slivers.
//! This module improves the mesh toward Delaunay by **bistellar flips** (Lawson
//! flips): local retriangulations that keep the same vertices and the same
//! filled region but swap the connectivity of a small cluster of tets.
//!
//! # The two flips
//!
//! - **2→3 flip.** Two tets sharing a triangular face (a convex bipyramid over
//!   5 vertices) become three tets sharing the opposite edge. Used when the
//!   shared face is not locally Delaunay and the bipyramid is convex.
//! - **3→2 flip.** The reverse: three tets sharing an edge become two sharing a
//!   face. Used to remove a non-Delaunay interior edge.
//!
//! A flip is applied only when the shared face is **not locally Delaunay** (the
//! opposite apex lies inside the circumsphere) **and** every resulting tet has
//! strictly positive volume — the validity guard that guarantees the mesh stays
//! a valid tiling (no inverted or overlapping tets). The driver sweeps until no
//! improving flip remains or an iteration cap is hit.
//!
//! # Predicates — attribution
//!
//! [`orient3d`] (signed volume) and [`insphere`] (in-circumsphere) are the two
//! geometric predicates of **Jonathan Richard Shewchuk**, *Adaptive Precision
//! Floating-Point Arithmetic and Fast Robust Geometric Predicates*, Discrete &
//! Computational Geometry 18(3):305-363, 1997, and the accompanying
//! `predicates.c` (Carnegie Mellon University,
//! <https://www.cs.cmu.edu/~quake/robust.html>, released by the author into the
//! public domain).
//!
//! **What is and is not taken from that work.** Only the *determinant
//! formulations* are — the `(b-a)·((c-a)×(d-a))` orientation determinant and the
//! lifted 4x4 in-sphere determinant. Shewchuk's actual contribution, the
//! adaptive multi-stage exact-arithmetic expansions that make those determinants
//! *robust*, is **not implemented here**: these are plain `f64` evaluations.
//! They are therefore fast and adequate for the well-conditioned meshes this
//! crate generates, but they are **not** a substitute for `predicates.c` and
//! must not be described as robust predicates. Exact/adaptive arithmetic is
//! future work; until then, near-degenerate configurations are simply left
//! un-flipped rather than flipped wrongly, so the result is always a valid mesh.
//!
//! # Guarantees & scope
//!
//! Volume is conserved (flips retriangulate the same region), the boundary is
//! untouched (only interior faces/edges flip), and every intermediate mesh is
//! valid. Pure flipping reaches a **local** Delaunay optimum, not necessarily
//! the global Delaunay triangulation (that needs point insertion too, per
//! `op-38z`). Pure Rust, Android-safe.

use crate::math::Vec3;
use crate::volume_mesh::{cells_faces, from_cell_faces, VolumeMesh};
use std::collections::HashMap;

/// Signed volume × 6 of the tetrahedron `(a, b, c, d)`: `(b−a)·((c−a)×(d−a))`.
/// Strictly positive iff the tet is **positively oriented** (`d` on the
/// positive side of the plane `a, b, c`).
///
/// Units: cubic metres × 6, for inputs in metres. The sign is the meaningful
/// output; the magnitude is only used here as a positive-volume test.
///
/// **Attribution and accuracy.** This is Shewchuk's `orient3d` determinant
/// (J. R. Shewchuk, *Adaptive Precision Floating-Point Arithmetic and Fast
/// Robust Geometric Predicates*, Discrete & Computational Geometry
/// 18(3):305-363, 1997), evaluated in **plain `f64`** — *without* his adaptive
/// exact-arithmetic expansions. Near-degenerate (nearly coplanar) input can
/// therefore return the wrong sign. Callers in this crate treat a near-zero
/// result as "do not flip", which keeps the mesh valid; do not rely on this
/// function as a robust predicate. See the module docs.
pub fn orient3d(a: Vec3, b: Vec3, c: Vec3, d: Vec3) -> f64 {
    b.sub(a).dot(c.sub(a).cross(d.sub(a)))
}

/// In-sphere predicate. When `(a, b, c, d)` is **positively oriented**
/// (`orient3d(a,b,c,d) > 0`), returns a value `> 0` iff `e` lies strictly
/// **inside** the circumsphere of `a, b, c, d`, `< 0` if strictly outside, and
/// `0` if cospherical. Positions in metres; only the sign is meaningful.
///
/// **Attribution and accuracy.** This is Shewchuk's `insphere` — the standard
/// lifted 4x4 determinant with rows `(p - e, |p - e|^2)` (same reference as
/// [`orient3d`]) — evaluated in **plain `f64`**, *without* his adaptive
/// exact-arithmetic expansions, so it can return the wrong sign on
/// near-cospherical input. Callers treat a near-zero result as "not worth
/// flipping". See the module docs.
pub fn insphere(a: Vec3, b: Vec3, c: Vec3, d: Vec3, e: Vec3) -> f64 {
    // Rows: (p − e, |p − e|²) for p in {a, b, c, d}; determinant of the 4×4.
    let row = |p: Vec3| {
        let r = p.sub(e);
        [r.x, r.y, r.z, r.dot(r)]
    };
    // The lifted determinant with rows (p − e, |p − e|²) is negative for an
    // interior `e` when (a,b,c,d) is positively oriented; negate so the sign
    // matches the documented convention (> 0 ⇔ inside).
    -det4(row(a), row(b), row(c), row(d))
}

/// Determinant of a 4×4 matrix given as four rows (cofactor expansion).
fn det4(m0: [f64; 4], m1: [f64; 4], m2: [f64; 4], m3: [f64; 4]) -> f64 {
    // 3×3 minor determinant.
    let det3 = |a: [f64; 3], b: [f64; 3], c: [f64; 3]| -> f64 {
        a[0] * (b[1] * c[2] - b[2] * c[1]) - a[1] * (b[0] * c[2] - b[2] * c[0])
            + a[2] * (b[0] * c[1] - b[1] * c[0])
    };
    let minor = |col: usize| -> f64 {
        let pick = |r: [f64; 4]| -> [f64; 3] {
            let mut out = [0.0; 3];
            let mut j = 0;
            for (k, &v) in r.iter().enumerate() {
                if k != col {
                    out[j] = v;
                    j += 1;
                }
            }
            out
        };
        det3(pick(m1), pick(m2), pick(m3))
    };
    m0[0] * minor(0) - m0[1] * minor(1) + m0[2] * minor(2) - m0[3] * minor(3)
}

/// Order `[a, b, c, d]` so the tet is positively oriented (swap the last two if
/// not). Assumes the four points are not coplanar.
fn oriented(mut t: [usize; 4], pts: &[Vec3]) -> [usize; 4] {
    if orient3d(pts[t[0]], pts[t[1]], pts[t[2]], pts[t[3]]) < 0.0 {
        t.swap(2, 3);
    }
    t
}

/// The four triangular faces of a tet, each a sorted vertex triple, paired with
/// the omitted (apex) vertex.
fn tet_faces(t: [usize; 4]) -> [([usize; 3], usize); 4] {
    let key = |mut f: [usize; 3]| {
        f.sort_unstable();
        f
    };
    [
        (key([t[1], t[2], t[3]]), t[0]),
        (key([t[0], t[2], t[3]]), t[1]),
        (key([t[0], t[1], t[3]]), t[2]),
        (key([t[0], t[1], t[2]]), t[3]),
    ]
}

/// A tetrahedral mesh as vertex-index quadruples, with the shared point set.
/// Built from a [`VolumeMesh`] whose cells are all tetrahedra.
struct TetMesh {
    pts: Vec<Vec3>,
    tets: Vec<[usize; 4]>,
}

impl TetMesh {
    /// Extract from a [`VolumeMesh`]; returns `None` if any cell is not a tet.
    fn from_volume(mesh: &VolumeMesh) -> Option<TetMesh> {
        let mut tets = Vec::with_capacity(mesh.n_cells);
        for cell in cells_faces(mesh) {
            let mut vs: Vec<usize> = cell.iter().flatten().copied().collect();
            vs.sort_unstable();
            vs.dedup();
            if vs.len() != 4 {
                return None;
            }
            tets.push(oriented([vs[0], vs[1], vs[2], vs[3]], &mesh.points));
        }
        Some(TetMesh { pts: mesh.points.clone(), tets })
    }

    /// Rebuild a [`VolumeMesh`] (4 triangular faces per tet, matched by
    /// [`from_cell_faces`]).
    fn to_volume(&self) -> VolumeMesh {
        let cells: Vec<Vec<Vec<usize>>> = self
            .tets
            .iter()
            .map(|t| vec![vec![t[1], t[2], t[3]], vec![t[0], t[2], t[3]], vec![t[0], t[1], t[3]], vec![t[0], t[1], t[2]]])
            .collect();
        from_cell_faces(self.pts.clone(), &cells)
    }

}

/// Improve `mesh` toward Delaunay by bistellar (2→3 / 3→2) flips, returning the
/// flipped mesh. `mesh`'s cells must all be tetrahedra (e.g. from
/// [`crate::tet::tetrahedralize`]); if not, `mesh` is returned unchanged.
///
/// Volume and the boundary are preserved; every intermediate mesh is valid.
/// Flipping reaches a *local* Delaunay optimum (see the module docs).
///
/// # Examples
///
/// ```
/// use outram_park_fork_cfmesh::{math::Vec3, shapes::box_surface, carve::carve_box, tet::tetrahedralize, delaunay::flip_to_delaunay};
///
/// let (p, t) = box_surface(Vec3::ZERO, Vec3::new(1.0, 1.0, 1.0));
/// let tets = tetrahedralize(&carve_box(&p, &t, 0.5));
/// let flipped = flip_to_delaunay(&tets, 10_000);
///
/// // Flips retriangulate the same region: volume conserved, still valid.
/// assert!((flipped.total_volume() - tets.total_volume()).abs() < 1e-9);
/// assert!(flipped.validate().is_ok());
/// ```
pub fn flip_to_delaunay(mesh: &VolumeMesh, max_flips: usize) -> VolumeMesh {
    // Robustness guard 1: only a **valid** tet mesh can be safely Delaunay-flipped.
    // An already-tangled input (e.g. tets from a non-star-shaped snapped cell,
    // which carry negative-volume cells) is returned unchanged — flipping cannot
    // fix a tangled mesh and would only amplify the damage. Checked on the input
    // mesh's own winding (per-tet re-orientation would hide the tangle).
    if crate::checks::check_quality(mesh).n_negative_volume_cells > 0 {
        return mesh.clone();
    }
    let Some(tm) = TetMesh::from_volume(mesh) else {
        return from_cell_faces(mesh.points.clone(), &cells_faces(mesh));
    };
    let pts = tm.pts;
    let orig = tm.tets;
    let nd_before = count_non_delaunay(&pts, &orig);
    let grow_cap = orig.len() * 2; // safety valve against runaway 2→3 growth

    // Tombstoned tet list: flips mark old tets `None` and push replacements, so a
    // whole sweep runs against one adjacency snapshot (no per-flip rebuild).
    let mut live: Vec<Option<[usize; 4]>> = orig.iter().copied().map(Some).collect();
    let mut n_live = orig.len();

    let mut flips = 0usize;
    // Bounded sweeps guarantee termination; each sweep flips many independent
    // faces, so a handful suffice in practice.
    for _ in 0..64 {
        if flips >= max_flips || n_live > grow_cap {
            break;
        }
        // Adjacency for this sweep: interior faces -> their two tets, and edges
        // -> incident tets (for 3→2), over the currently-live tets.
        let mut fmap: HashMap<[usize; 3], Vec<(usize, usize)>> = HashMap::new();
        let mut emap: HashMap<(usize, usize), Vec<usize>> = HashMap::new();
        for (i, ot) in live.iter().enumerate() {
            let Some(t) = *ot else { continue };
            for (f, apex) in tet_faces(t) {
                fmap.entry(f).or_default().push((i, apex));
            }
            for e in tet_edges(t) {
                emap.entry(e).or_default().push(i);
            }
        }
        let mut touched = vec![false; live.len()];
        let mut did = 0usize;

        for (abc, ts) in &fmap {
            if flips >= max_flips || n_live > grow_cap {
                break;
            }
            if ts.len() != 2 {
                continue;
            }
            let (i1, d) = ts[0];
            let (i2, e) = ts[1];
            if touched[i1] || touched[i2] || is_locally_delaunay(&pts, *abc, d, e) {
                continue;
            }
            // Try 2→3 (convex bipyramid): 2 tets removed, 3 added.
            if let Some(newt) = flip_2_3(&pts, *abc, d, e) {
                touched[i1] = true;
                touched[i2] = true;
                live[i1] = None;
                live[i2] = None;
                live.extend(newt.into_iter().map(Some));
                n_live += 1;
                did += 1;
                flips += 1;
                continue;
            }
            // Else a 3→2 on the non-convex (reflex) edge: 3 removed, 2 added.
            if let Some((ring, newt)) = flip_3_2(&pts, *abc, &emap, &live) {
                if ring.iter().all(|&i| !touched[i]) {
                    for &i in &ring {
                        touched[i] = true;
                        live[i] = None;
                    }
                    live.extend(newt.into_iter().map(Some));
                    n_live -= 1;
                    did += 1;
                    flips += 1;
                }
            }
        }
        if did == 0 {
            break; // local optimum (or only un-flippable configs remain)
        }
    }

    // Robustness guard 2: improve-or-noop. Accept the flipped mesh only if it is
    // still valid and no worse on the Delaunay metric than the input; otherwise
    // return the input unchanged. So `flip_to_delaunay` can never make a mesh
    // worse (f64 predicates can misjudge near-degenerate configs; this is the
    // safety net that keeps the result trustworthy).
    let after: Vec<[usize; 4]> = live.into_iter().flatten().collect();
    let accept = after.iter().all(|&t| tet_positive(&pts, t)) && count_non_delaunay(&pts, &after) <= nd_before;
    let tets = if accept { after } else { orig };
    TetMesh { pts, tets }.to_volume()
}

/// Count interior faces of a tet list that are not locally Delaunay.
fn count_non_delaunay(pts: &[Vec3], tets: &[[usize; 4]]) -> usize {
    let mut fmap: HashMap<[usize; 3], Vec<usize>> = HashMap::new();
    for t in tets {
        for (f, apex) in tet_faces(*t) {
            fmap.entry(f).or_default().push(apex);
        }
    }
    fmap.iter().filter(|(abc, apexes)| apexes.len() == 2 && !is_locally_delaunay(pts, **abc, apexes[0], apexes[1])).count()
}

/// Undirected edge key.
fn edge_key(a: usize, b: usize) -> (usize, usize) {
    if a < b {
        (a, b)
    } else {
        (b, a)
    }
}

/// The six edges of a tet, as sorted vertex pairs.
fn tet_edges(t: [usize; 4]) -> [(usize, usize); 6] {
    [
        edge_key(t[0], t[1]),
        edge_key(t[0], t[2]),
        edge_key(t[0], t[3]),
        edge_key(t[1], t[2]),
        edge_key(t[1], t[3]),
        edge_key(t[2], t[3]),
    ]
}

/// Is face `abc` (shared by tets with apexes `d`, `e`) locally Delaunay?
fn is_locally_delaunay(pts: &[Vec3], abc: [usize; 3], d: usize, e: usize) -> bool {
    let t = oriented([abc[0], abc[1], abc[2], d], pts);
    insphere(pts[t[0]], pts[t[1]], pts[t[2]], pts[t[3]], pts[e]) <= 1e-12
}

/// Strictly-positive-volume tet?
fn tet_positive(pts: &[Vec3], t: [usize; 4]) -> bool {
    orient3d(pts[t[0]], pts[t[1]], pts[t[2]], pts[t[3]]) > 1e-14
}

/// The 2→3 flip of the bipyramid over face `abc` with apexes `d`, `e`: three
/// tets sharing edge `de`. Returns them (positively oriented) only if the
/// bipyramid is convex (all three are strictly positive) — else `None`.
fn flip_2_3(pts: &[Vec3], abc: [usize; 3], d: usize, e: usize) -> Option<Vec<[usize; 4]>> {
    let [a, b, c] = abc;
    let out: Vec<[usize; 4]> = [[a, b, d, e], [b, c, d, e], [c, a, d, e]].iter().map(|&t| oriented(t, pts)).collect();
    out.iter().all(|&t| tet_positive(pts, t)).then_some(out)
}

/// The 3→2 flip removing a reflex edge of the non-convex bipyramid `abc`. If one
/// of `ab, bc, ca` is shared by exactly three tets whose three opposite vertices
/// form a valid pair of replacement tets, returns `(ring tets, two new tets)`.
fn flip_3_2(
    pts: &[Vec3],
    abc: [usize; 3],
    emap: &HashMap<(usize, usize), Vec<usize>>,
    live: &[Option<[usize; 4]>],
) -> Option<(Vec<usize>, Vec<[usize; 4]>)> {
    let [a, b, c] = abc;
    for &(x, y) in &[(a, b), (b, c), (c, a)] {
        let ring = emap.get(&edge_key(x, y))?;
        if ring.len() != 3 {
            continue;
        }
        // The three vertices opposite the shared edge across the ring.
        let mut opp: Vec<usize> = Vec::new();
        for &ti in ring {
            if let Some(t) = live[ti] {
                opp.extend(t.iter().copied().filter(|&v| v != x && v != y));
            }
        }
        opp.sort_unstable();
        opp.dedup();
        if opp.len() != 3 {
            continue;
        }
        let n1 = oriented([opp[0], opp[1], opp[2], x], pts);
        let n2 = oriented([opp[0], opp[1], opp[2], y], pts);
        if tet_positive(pts, n1) && tet_positive(pts, n2) {
            return Some((ring.clone(), vec![n1, n2]));
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::carve::carve_box;
    use crate::checks::check_quality;
    use crate::shapes::box_surface;
    use crate::tet::tetrahedralize;
    use crate::volume_mesh::{from_cell_faces, VolumeMesh};

    /// Sanity — the predicates have the documented signs. A unit tet is
    /// positively oriented; the centre of a regular tet's circumsphere test:
    /// a point just inside returns `> 0`, just outside `< 0`.
    #[test]
    fn predicates_have_correct_signs() {
        let a = Vec3::new(0.0, 0.0, 0.0);
        let b = Vec3::new(1.0, 0.0, 0.0);
        let c = Vec3::new(0.0, 1.0, 0.0);
        let d = Vec3::new(0.0, 0.0, 1.0);
        assert!(orient3d(a, b, c, d) > 0.0, "unit tet positively oriented");
        // Circumsphere of this tet passes through the 4 corners; its centre is
        // (0.5,0.5,0.5). A point near the centroid is inside; a far point out.
        assert!(insphere(a, b, c, d, Vec3::new(0.25, 0.25, 0.25)) > 0.0, "interior point inside sphere");
        assert!(insphere(a, b, c, d, Vec3::new(5.0, 5.0, 5.0)) < 0.0, "far point outside sphere");
    }

    /// Build a VolumeMesh of two tets sharing triangle `abc`, apexes `d`, `e`.
    fn two_tets(a: Vec3, b: Vec3, c: Vec3, d: Vec3, e: Vec3) -> VolumeMesh {
        let pts = vec![a, b, c, d, e];
        let (ia, ib, ic, id, ie) = (0, 1, 2, 3, 4);
        let t1 = vec![vec![ib, ic, id], vec![ia, ic, id], vec![ia, ib, id], vec![ia, ib, ic]];
        let t2 = vec![vec![ib, ic, ie], vec![ia, ic, ie], vec![ia, ib, ie], vec![ia, ib, ic]];
        from_cell_faces(pts, &[t1, t2])
    }

    /// V&V — a 2→3 flip fixes a non-Delaunay bipyramid. Methodology: place a flat
    /// shared triangle with two apexes close above/below so the shared face is
    /// NOT locally Delaunay (each apex sits inside the other tet's circumsphere);
    /// flip. Pass criteria: 2 tets → 3 tets, the result is Delaunay (0
    /// non-Delaunay faces), volume conserved, all tets positive, validate Ok.
    #[test]
    fn flip_fixes_non_delaunay_bipyramid() {
        // A wide, flat shared triangle with two nearby apexes -> the shared face
        // is non-Delaunay (thin tets; each apex inside the other's circumsphere).
        let a = Vec3::new(-1.0, -1.0, 0.0);
        let b = Vec3::new(1.0, -1.0, 0.0);
        let c = Vec3::new(0.0, 1.0, 0.0);
        let d = Vec3::new(0.0, 0.0, 0.25);
        let e = Vec3::new(0.0, 0.0, -0.25);
        let mesh = two_tets(a, b, c, d, e);
        let vol0 = mesh.total_volume();

        let tm = TetMesh::from_volume(&mesh).expect("two tets");
        assert!(count_non_delaunay(&tm.pts, &tm.tets) >= 1, "input has a non-Delaunay face");

        let flipped = flip_to_delaunay(&mesh, 100);
        assert_eq!(flipped.cell_count(), 3, "2→3 flip yields three tets");
        assert!((flipped.total_volume() - vol0).abs() < 1e-12, "volume conserved");
        assert_eq!(check_quality(&flipped).n_negative_volume_cells, 0, "all tets positive");
        flipped.validate().expect("closed");
        let ftm = TetMesh::from_volume(&flipped).unwrap();
        assert_eq!(count_non_delaunay(&ftm.pts, &ftm.tets), 0, "flipped bipyramid is Delaunay");
    }

    /// V&V — on a full tet mesh, flipping never breaks validity and never
    /// worsens Delaunay-ness. Methodology: tetrahedralize a 2×2×2 carve, flip.
    /// Pass criteria: volume conserved exactly, mesh valid, and the count of
    /// non-locally-Delaunay faces does not increase (flips only improve it).
    #[test]
    fn flipping_a_tet_mesh_conserves_volume_and_improves_delaunay() {
        let (p, t) = box_surface(Vec3::ZERO, Vec3::new(1.0, 1.0, 1.0));
        let tets = tetrahedralize(&carve_box(&p, &t, 0.5));
        let btm = TetMesh::from_volume(&tets).unwrap();
        let before = count_non_delaunay(&btm.pts, &btm.tets);

        let flipped = flip_to_delaunay(&tets, 5_000);
        assert!((flipped.total_volume() - 1.0).abs() < 1e-9, "volume conserved: {}", flipped.total_volume());
        flipped.validate().expect("valid mesh");
        assert_eq!(check_quality(&flipped).n_negative_volume_cells, 0, "no inverted tets");

        let atm = TetMesh::from_volume(&flipped).unwrap();
        let after = count_non_delaunay(&atm.pts, &atm.tets);
        assert!(after <= before, "flips do not worsen Delaunay-ness: {before} -> {after}");
        // 2→3 growth stays within the safety cap (no runaway).
        assert!(flipped.cell_count() <= 2 * tets.cell_count(), "growth bounded: {} <= 2×{}", flipped.cell_count(), tets.cell_count());
    }

    /// V&V — robustness guard: an **already-invalid** tet mesh (inverted cells)
    /// is returned unchanged, never amplified. Methodology: tetrahedralize a box,
    /// shove one interior vertex far outside to invert incident tets (negative
    /// volumes), then flip. Pass criterion: the flipper detects the invalid input
    /// and returns it with the same cell count (a no-op), rather than thrashing.
    #[test]
    fn invalid_input_is_returned_unchanged() {
        let (p, t) = box_surface(Vec3::ZERO, Vec3::new(1.0, 1.0, 1.0));
        let mut tets = tetrahedralize(&carve_box(&p, &t, 0.5));
        let n0 = tets.cell_count();

        // Find an interior vertex and displace it far enough to invert tets.
        let mut pinned = vec![false; tets.points.len()];
        for f in 0..tets.face_count() {
            if tets.neighbour[f].is_none() {
                for &v in &tets.faces[f] {
                    pinned[v] = true;
                }
            }
        }
        let v = pinned.iter().position(|&p| !p).expect("interior vertex");
        tets.points[v] = tets.points[v].add(Vec3::new(10.0, 10.0, 10.0));
        assert!(check_quality(&tets).n_negative_volume_cells > 0, "input is now invalid");

        let out = flip_to_delaunay(&tets, 10_000);
        assert_eq!(out.cell_count(), n0, "invalid input returned unchanged (no thrashing)");
    }
}
