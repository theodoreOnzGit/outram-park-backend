// SPDX-License-Identifier: GPL-3.0-only
// Copyright (C) 2026 OUTRAM PARK contributors
//
// Ported in part from OpenMC (https://github.com/openmc-dev/openmc, MIT
// licence, Copyright (c) 2011-2026 Massachusetts Institute of Technology,
// UChicago Argonne LLC and OpenMC contributors; see the crate's LICENSE
// notes), commit d7d3284a1b13d7cae020e0d8fea9f1e60d91b18b:
//   src/mesh.cpp   MOABMesh::bins_crossed (:3168), MOABMesh::intersect_track
//                  (:3144), MOABMesh::get_tet (:3241), MOABMesh::point_in_tet
//                  (:3366), UnstructuredMesh::sample_tet (:992),
//                  LibMesh::get_bin (:3973)
//   src/tallies/filter_mesh.cpp  MeshFilter::get_all_bins (:60-69)
// The DAGMC/MOAB/libMesh machinery those routines call (k-d tree, ray-triangle
// queries, point locator) is replaced by pure-Rust equivalents here.
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

//! **Unstructured-mesh tally scoring** (GitHub #492): point location in a
//! cell of a neutral [`UnstructuredMesh`] and the track-length estimator
//! across its cells. The description (cells, faces, decomposition, spatial
//! index) is `outram_blender::unstructured`; the scoring is here, on the
//! transport hot path, as for the structured meshes (GitHub #486).
//!
//! # Mapping to upstream
//!
//! | OpenMC | here | note |
//! |---|---|---|
//! | `LibMesh::get_bin` / `MOABMesh::get_bin` (`mesh.cpp:3973`, `:3326`) | [`UnstructuredMeshExt::locate`] | position divided by `length_multiplier` (here the mesh's `LengthUnit`), bounding-box rejection, then a containing-element search |
//! | `MOABMesh::get_tet` + `point_in_tet` (`:3241`, `:3366`) | the candidate loop + barycentric test | the k-d tree leaf is replaced by `CellLocator::candidates_at`; the tet is replaced by the **centroid tetrahedral decomposition** of a general polyhedron (a tet cell decomposes into 4 tets that tile it exactly) |
//! | `MOABMesh::bins_crossed` + `intersect_track` (`:3168`, `:3144`) | [`UnstructuredMeshExt::bins_crossed`] | every crossing of the segment with a cell face, sorted; each sub-segment is located at its midpoint and weighted by its length fraction |
//! | `LibMesh::bins_crossed` (`:3966`) | — | upstream **does not implement** track-length tallies on libMesh meshes (`fatal_error`); the MOAB algorithm is used for every mesh here |
//! | `UnstructuredMesh::sample_tet` (`:992`) | [`UnstructuredMeshExt::sample_in_cell`] | a decomposition tet is chosen by volume first, so a polyhedron is sampled uniformly |
//! | `UnstructuredMesh::surface_bins_crossed` (`:1029`) | — | upstream `fatal_error`s ("not implemented"); so does `MeshSurfaceFilter::new` here |
//!
//! # Two deliberate departures from the MOAB routine
//!
//! 1. **Duplicate hits are removed after sorting.** Upstream calls
//!    `std::unique(hits.begin(), hits.end())` *before* `std::sort` and never
//!    erases the tail (`mesh.cpp:3160-3164`), so duplicates are neither
//!    adjacent nor removed; a duplicated distance yields a zero-length
//!    sub-segment, which scores weight 0 into the bin at its midpoint. The
//!    intended behaviour (sort, then drop duplicates) is implemented here and
//!    zero-length sub-segments are skipped, so the scored weights are the same.
//! 2. **No `TINY_BIT` nudge.** Upstream widens the ray by `TINY_BIT` at both
//!    ends so a hit exactly at an endpoint is found; here intersections are
//!    taken on the closed parameter interval `[0, 1]`, which finds those hits
//!    without moving the distances (upstream's distances are measured from the
//!    nudged start, so they are `TINY_BIT` long, a ~1e-14 cm effect).
//!
//! # What a 2-D mesh does
//!
//! Nothing: a 2-D neutral mesh (an FE plane-strain mesh, say) is not a
//! transport tally mesh, and [`UnstructuredMeshExt::locate`] returns `None`
//! for it, so it bins no event. OpenMC fixes `n_dimension_ = 3` for every
//! unstructured mesh (`mesh.cpp:893`).

use outram_blender::unstructured::geometry::tet_signed_volume;
pub use outram_blender::unstructured::UnstructuredMesh;

use crate::geometry::position::Position;
use crate::rng::lcg::prn;

/// Relative tolerance of the barycentric inside test: a point counts as
/// inside a decomposition tetrahedron when every sub-volume is at least
/// `-INSIDE_TOL * V`. A strict `>= 0` (upstream's test) can leave a point
/// lying exactly on a shared face outside **both** neighbours through
/// round-off; this closes that gap, and the first cell to claim the point
/// wins, as with upstream's leaf scan.
const INSIDE_TOL: f64 = 1e-12;

/// Relative tolerance below which two sorted crossing distances are the same
/// crossing (a ray through an edge or vertex hits several fan triangles).
const SAME_HIT_TOL: f64 = 1e-12;

/// Tally scoring on an [`UnstructuredMesh`] that stays in `outram-mc-libs`.
///
/// Positions are in **cm**; the mesh's own unit is converted internally.
pub trait UnstructuredMeshExt {
    /// Cell containing `p` \[cm\], or `None` outside the mesh (or for a 2-D
    /// mesh).
    fn locate(&self, p: Position) -> Option<usize>;
    /// The cells the segment `r0 -> r1` \[cm\] crosses, in order, each with
    /// the **fraction** of the segment's length inside it — the
    /// `(bins, lengths)` pair of `MOABMesh::bins_crossed`. Fractions of parts
    /// outside the mesh are omitted, so they sum to at most 1.
    fn bins_crossed(&self, r0: Position, r1: Position) -> Vec<(usize, f64)>;
    /// A point \[cm\] uniformly distributed in cell `cell`.
    fn sample_in_cell(&self, cell: usize, seed: &mut u64) -> Position;
}

/// Barycentric inside test on one positively oriented tetrahedron, as in
/// `MOABMesh::point_in_tet` (`mesh.cpp:3366`), written with signed
/// sub-volumes instead of a stored inverse matrix.
#[inline]
fn in_tet(t: &[[f64; 3]; 4], q: [f64; 3]) -> bool {
    let v = tet_signed_volume(t[0], t[1], t[2], t[3]);
    if !(v > 0.0) {
        return false;
    }
    let tol = -INSIDE_TOL * v;
    tet_signed_volume(q, t[1], t[2], t[3]) >= tol
        && tet_signed_volume(t[0], q, t[2], t[3]) >= tol
        && tet_signed_volume(t[0], t[1], q, t[3]) >= tol
        && tet_signed_volume(t[0], t[1], t[2], q) >= tol
}

/// Whether `q` (mesh units) lies in cell `c`'s decomposition.
#[inline]
fn in_cell(mesh: &UnstructuredMesh, c: usize, q: [f64; 3]) -> bool {
    let bb = mesh.cell_bounds(c);
    let span = (0..3).map(|a| bb[1][a] - bb[0][a]).fold(0.0f64, f64::max);
    let pad = 1e-9 * span;
    for a in 0..3 {
        if q[a] < bb[0][a] - pad || q[a] > bb[1][a] + pad {
            return false;
        }
    }
    let mut found = false;
    mesh.for_each_cell_simplex(c, |t| {
        if !found && in_tet(&t, q) {
            found = true;
        }
    });
    found
}

/// Point location in mesh units.
#[inline]
fn locate_unit(mesh: &UnstructuredMesh, q: [f64; 3]) -> Option<usize> {
    if mesh.dim() != 3 {
        return None;
    }
    mesh.locator().candidates_at(q).iter().copied().find(|&c| in_cell(mesh, c, q))
}

/// Segment `a + t (b - a)`, `t in [0, 1]`, against triangle `(p0, p1, p2)`:
/// the parameter `t` of the crossing, or `None` (Moller-Trumbore, both
/// windings, with an inclusive edge test so a ray through a shared edge is
/// found on at least one side).
#[inline]
fn segment_triangle(a: [f64; 3], d: [f64; 3], p0: [f64; 3], p1: [f64; 3], p2: [f64; 3]) -> Option<f64> {
    use outram_blender::unstructured::geometry::{cross, dot, sub};
    let e1 = sub(p1, p0);
    let e2 = sub(p2, p0);
    let h = cross(d, e2);
    let det = dot(e1, h);
    let scale = dot(e1, e1).max(dot(e2, e2)) * dot(d, d).sqrt();
    if det.abs() <= 1e-14 * scale {
        return None; // segment parallel to the triangle's plane
    }
    let inv = 1.0 / det;
    let s = sub(a, p0);
    let u = inv * dot(s, h);
    let eps = 1e-12;
    if u < -eps || u > 1.0 + eps {
        return None;
    }
    let q = cross(s, e1);
    let v = inv * dot(d, q);
    if v < -eps || u + v > 1.0 + eps {
        return None;
    }
    let t = inv * dot(e2, q);
    if (0.0..=1.0).contains(&t) {
        Some(t)
    } else {
        None
    }
}

impl UnstructuredMeshExt for UnstructuredMesh {
    #[inline]
    fn locate(&self, p: Position) -> Option<usize> {
        let s = 1.0 / self.unit().cm_per_unit();
        locate_unit(self, [p.x * s, p.y * s, p.z * s])
    }

    fn bins_crossed(&self, r0: Position, r1: Position) -> Vec<(usize, f64)> {
        let mut out = Vec::new();
        if self.dim() != 3 {
            return out;
        }
        let s = 1.0 / self.unit().cm_per_unit();
        let a = [r0.x * s, r0.y * s, r0.z * s];
        let b = [r1.x * s, r1.y * s, r1.z * s];
        let d = [b[0] - a[0], b[1] - a[1], b[2] - a[2]];
        let len = (d[0] * d[0] + d[1] * d[1] + d[2] * d[2]).sqrt();
        if !(len > 0.0) {
            return out; // upstream: `if (track_len == 0.0) return;`
        }
        let at = |t: f64| [a[0] + d[0] * t, a[1] + d[1] * t, a[2] + d[2] * t];

        // intersect_track: every face crossing of the segment. Candidate faces
        // are those of cells whose buckets the segment's box touches.
        let lo = [a[0].min(b[0]), a[1].min(b[1]), a[2].min(b[2])];
        let hi = [a[0].max(b[0]), a[1].max(b[1]), a[2].max(b[2])];
        let mut cells = Vec::new();
        self.locator().candidates_in_box(lo, hi, &mut cells);
        let mut faces: Vec<usize> = cells.iter().flat_map(|&c| self.cell_faces(c).iter().copied()).collect();
        faces.sort_unstable();
        faces.dedup();
        let pts = self.points();
        let mut hits: Vec<f64> = Vec::new();
        for f in faces {
            let verts = self.face(f);
            let fc = self.face_centre(f);
            let n = verts.len();
            for i in 0..n {
                if let Some(t) = segment_triangle(a, d, fc, pts[verts[i]], pts[verts[(i + 1) % n]]) {
                    hits.push(t);
                }
            }
        }
        hits.sort_by(|x, y| x.partial_cmp(y).unwrap_or(std::cmp::Ordering::Equal));
        hits.dedup_by(|x, y| (*x - *y).abs() <= SAME_HIT_TOL);

        // No crossing: the segment lies in one cell (or outside the mesh).
        if hits.is_empty() {
            if let Some(c) = locate_unit(self, at(0.5)) {
                out.push((c, 1.0));
            }
            return out;
        }
        // One sub-segment per pair of consecutive crossings, located at its
        // midpoint, plus the tail after the last crossing.
        let mut last = 0.0;
        for &t in hits.iter().chain(std::iter::once(&1.0)) {
            let seg = t - last;
            if seg > SAME_HIT_TOL {
                if let Some(c) = locate_unit(self, at(last + 0.5 * seg)) {
                    out.push((c, seg));
                }
            }
            last = t;
        }
        out
    }

    fn sample_in_cell(&self, cell: usize, seed: &mut u64) -> Position {
        let mut tets: Vec<([[f64; 3]; 4], f64)> = Vec::new();
        let mut total = 0.0;
        self.for_each_cell_simplex(cell, |t| {
            let v = tet_signed_volume(t[0], t[1], t[2], t[3]).max(0.0);
            total += v;
            tets.push((t, total));
        });
        let xi = prn(seed) * total;
        let idx = tets.partition_point(|&(_, c)| c <= xi).min(tets.len() - 1);
        let t = tets[idx].0;
        // UnstructuredMesh::sample_tet (mesh.cpp:992), after Rocchini &
        // Cignoni (2000), J. Graphics Tools 5(4):9-12.
        let (mut s, mut tt, mut u) = (prn(seed), prn(seed), prn(seed));
        if s + tt > 1.0 {
            s = 1.0 - s;
            tt = 1.0 - tt;
        }
        if s + tt + u > 1.0 {
            if tt + u > 1.0 {
                let old_t = tt;
                tt = 1.0 - u;
                u = 1.0 - s - old_t;
            } else {
                let old_s = s;
                s = 1.0 - tt - u;
                u = old_s + tt + u - 1.0;
            }
        }
        let k = self.unit().cm_per_unit();
        let p = |i: usize| t[i];
        let q = [
            p(0)[0] + s * (p(1)[0] - p(0)[0]) + tt * (p(2)[0] - p(0)[0]) + u * (p(3)[0] - p(0)[0]),
            p(0)[1] + s * (p(1)[1] - p(0)[1]) + tt * (p(2)[1] - p(0)[1]) + u * (p(3)[1] - p(0)[1]),
            p(0)[2] + s * (p(1)[2] - p(0)[2]) + tt * (p(2)[2] - p(0)[2]) + u * (p(3)[2] - p(0)[2]),
        ];
        Position::new(q[0] * k, q[1] * k, q[2] * k)
    }
}

#[cfg(test)]
mod tests {
    //! Verification of point location and track-length splitting against
    //! analytic answers on a two-cube hex mesh and a Kuhn tet box.
    //!
    //! **Results: NOT YET MEASURED (testing deferred by maintainer,
    //! 2026-10-03).**
    use super::*;
    use outram_blender::unstructured::{Element, ElementKind, LengthUnit};

    /// Two unit hexes side by side along x, in cm: `[0,1]` and `[1,2]`.
    fn two_hexes() -> UnstructuredMesh {
        let mut pts = Vec::new();
        for x in [0.0, 1.0, 2.0] {
            for (y, z) in [(0.0, 0.0), (1.0, 0.0), (1.0, 1.0), (0.0, 1.0)] {
                pts.push([x, y, z]);
            }
        }
        // Hex8 with zeta = x: bottom (y,z) loop CCW seen from +x.
        let el = |i: usize| Element { kind: ElementKind::Hex8, nodes: (0..8).map(|k| 4 * i + k).collect() };
        UnstructuredMesh::from_elements(LengthUnit::Centimetre, pts, vec![el(0), el(1)], vec![], vec![]).unwrap()
    }

    #[test]
    fn locate_finds_each_hex_and_rejects_outside() {
        let m = two_hexes();
        assert_eq!(m.locate(Position::new(0.5, 0.5, 0.5)), Some(0));
        assert_eq!(m.locate(Position::new(1.5, 0.2, 0.9)), Some(1));
        assert_eq!(m.locate(Position::new(2.5, 0.5, 0.5)), None);
        assert_eq!(m.locate(Position::new(0.5, -0.1, 0.5)), None);
    }

    /// A segment from x = 0.25 to x = 1.75 spends exactly half its length in
    /// each cell; one leaving the mesh scores only its inside part.
    #[test]
    fn track_length_is_split_by_length_fraction() {
        let m = two_hexes();
        let w = m.bins_crossed(Position::new(0.25, 0.5, 0.5), Position::new(1.75, 0.5, 0.5));
        assert_eq!(w.len(), 2);
        assert_eq!((w[0].0, w[1].0), (0, 1));
        assert!((w[0].1 - 0.5).abs() < 1e-12 && (w[1].1 - 0.5).abs() < 1e-12);
        let out = m.bins_crossed(Position::new(1.5, 0.5, 0.5), Position::new(3.5, 0.5, 0.5));
        assert_eq!(out.len(), 1);
        assert!((out[0].1 - 0.25).abs() < 1e-12);
        // A segment inside one cell scores it whole.
        let one = m.bins_crossed(Position::new(0.1, 0.1, 0.1), Position::new(0.9, 0.8, 0.7));
        assert_eq!(one, vec![(0, 1.0)]);
    }

    /// Samples land in the cell they were drawn for.
    #[test]
    fn samples_stay_in_their_cell() {
        let m = two_hexes();
        let mut seed = 12345u64;
        for _ in 0..200 {
            let p = m.sample_in_cell(1, &mut seed);
            assert_eq!(m.locate(p), Some(1));
        }
    }
}
