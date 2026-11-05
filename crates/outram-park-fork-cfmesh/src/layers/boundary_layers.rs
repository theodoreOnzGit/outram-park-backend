// SPDX-License-Identifier: GPL-3.0-only
// Copyright (C) 2026 OUTRAM PARK contributors
//
// Ported from cfMesh v1.1 (github.com/DAFoam/cfmesh, commit e944f6ba695f):
//   meshLibrary/utilities/boundaryLayers/boundaryLayers/
//     boundaryLayers.{C,H}              (addLayerForPatch, the constructor's
//                                        blHeightFactor_ 0.5, blMinHeight_ 0)
//     boundaryLayersCreateVertices.C    (findPatchVertices, createNewVertex,
//                                        createNewVertices(boolList))
//     boundaryLayersFacesAndCells.C     (createNewFacesAndCells)
// Copyright (C) 2014-2017 Creative Fields, Ltd. Licence: GPL-3.0-only.
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

//! **cfMesh's `boundaryLayers` creation route** (gh:#1270): one layer of
//! cells on the named patches, as `boundaryLayers::addLayerForPatch` builds
//! it. Opt-in, beside the crate's three layer strategies
//! ([`super::LayerQualityStrategy`]); nothing calls it by default.
//!
//! # What upstream does, and what this port reuses
//!
//! Upstream creates one new point per boundary point of the treated
//! patches (`createNewVertex`), then **swaps** the two: the original
//! point moves to the new position (so the cells it belongs to shrink) and
//! the new label takes the old position on the boundary. Each treated face
//! then gets one cell: its face (now internal), the face of new labels (the
//! boundary), and one quad per edge (`createNewFacesAndCells`); a quad
//! whose edge borders an untreated face becomes a boundary face of that
//! face's patch, so the untreated patch is re-partitioned, its points
//! sliding in its plane.
//!
//! That is a column of one layer per wall point, from the old position to
//! the new one, which is what [`super::build_layered_variable`] assembles:
//! this route uses it (and [`super::layer_setup`]'s classification of the
//! wall points into `PATCHNODE`, `EDGENODE` with one or two other patches,
//! and its refusal of three) and adds only upstream's **point placement**:
//!
//! - **`PATCHNODE`** (only treated patches at the point): along the unit
//!   point normal of the surface engine (`pointNormals`), inward, by
//!   `blHeightFactor` (0.5) x the shortest distance to a neighbouring
//!   boundary point (`pointPoints`).
//! - **`EDGENODE`, one other patch:** along the treated faces' area-vector
//!   sum with its component along the other patch's unit normal sum
//!   removed (so the point stays in that patch's plane), by 0.5 x the
//!   smallest `|(q − p)·n|` over neighbours on no treated patch; then, for
//!   each face of the other patch at the point, if the distance `dst` to
//!   the segment between the point's two neighbours in that face is below
//!   the current distance, the distance becomes `0.9 dst` (upstream's
//!   test is `dst < dist`, its value `0.9 dst`; ported as written).
//! - **`EDGENODE`, two other patches:** towards the first neighbouring
//!   boundary point that lies on both (`pointPoints` order), half way
//!   (the normal is `(p − q) / (2 · 0.5 |p − q|)`, so the move is half the
//!   edge for any height factor; ported as written), then the same 0.9 cap.
//! - Every distance is at least `max(blMinHeight, VSMALL)`; a NaN or
//!   infinite position leaves the point where it is.
//!
//! Where this differs from the crate's [`super::add_boundary_layers_adaptive_on_patches`]:
//! that route takes the thickness from the request (capped at 0.5 x the
//! shortest wall edge at the point), smooths the interior normals and backs
//! off globally until the mesh passes upstream's quality limits; this one
//! places one layer by the local edge length, along the unsmoothed point
//! normal, with no quality check. Split it into several layers with
//! [`super::refine_boundary_layers`].
//!
//! **Numbering, not geometry, differs from upstream:** the columns are
//! numbered as [`super::layer_columns`] numbers them (non-wall points
//! first), where upstream appends the boundary copies at the end; and a
//! point's column runs from the old position `x` to `x + |d| (d/|d|)`
//! rather than `x + d`, equal to round-off. Patch-wise layers at feature
//! edges between treated patches of different groups
//! (`findPatchesToBeTreatedTogether`, `createNewVertices(labelList)`,
//! `createLayerCells`, `otherVrts_`), `addWrapperLayer`,
//! `checkTopologyOfBoundaryFaces`, the 2D mode and the MPI branches are
//! not ported: this route treats all named patches as one group, which is
//! upstream's `addLayerForPatch` when they are (one patch, or patches
//! meeting at convex edges under 135°).

use super::{build_layered_variable_with_columns, distance_to_segment, layer_setup, LayerVertex};
use crate::math::Vec3;
use crate::surface_engine::{SurfaceEngine, SurfacePartitioner, VSMALL};
use crate::topology::TopoMesh;
use crate::volume_mesh::VolumeMesh;
use std::collections::BTreeMap;

/// `boundaryLayers` settings (the `boundaryLayers` sub-dictionary of
/// `meshDict` this upstream line reads).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct BoundaryLayersControls {
    /// `boundaryLayerHeightFactor` (default 0.5: half the distance to the
    /// nearest neighbour).
    pub height_factor: f64,
    /// `boundaryLayerMinHeight` [m] (default 0: no minimum).
    pub min_height: f64,
}

impl Default for BoundaryLayersControls {
    fn default() -> Self {
        BoundaryLayersControls { height_factor: 0.5, min_height: 0.0 }
    }
}

/// One wall point of a [`boundary_layers_add_layer`] layer.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct LayerPoint {
    /// Mesh point (in the input mesh).
    pub point: usize,
    /// Position before (on the wall).
    pub wall: Vec3,
    /// Position upstream computes (`p − dist · normal`).
    pub moved: Vec3,
    /// Unit direction of the column (`moved − wall`, normalised; zero for
    /// a point that does not move).
    pub direction: Vec3,
    /// Column length [m]: the column's top is `wall + direction · length`.
    pub length: f64,
    /// Number of untreated patches at the point (0: `PATCHNODE`; 1 or 2:
    /// `EDGENODE`).
    pub n_other_patches: usize,
}

impl LayerPoint {
    /// Column length [m].
    pub fn thickness(&self) -> f64 {
        self.length
    }
}

/// What [`boundary_layers_add_layer`] built.
#[derive(Debug, Clone, PartialEq)]
pub struct BoundaryLayersOutcome {
    /// Every wall point, ascending.
    pub points: Vec<LayerPoint>,
    /// Treated boundary faces (one layer cell each), input mesh labels.
    pub wall_faces: Vec<usize>,
    /// Cell of the output mesh owning each layer: the layer cells are
    /// the last `wall_faces.len()` cells, in wall-face order.
    pub first_layer_cell: usize,
}

/// cfMesh `boundaryLayers::addLayerForPatch` on the named patches, treated
/// as one group (see the module documentation). Returns `None` where
/// upstream would stop: no treated face, or a point that touches three or
/// more untreated patches, or two with no shared neighbour. The output
/// keeps the input's patch names, each face in the patch upstream gives it.
pub fn boundary_layers_add_layer(mesh: &VolumeMesh, layer_patches: &[&str], c: &BoundaryLayersControls) -> Option<(VolumeMesh, BoundaryLayersOutcome)> {
    let s = layer_setup(mesh, layer_patches, 1, 1.0, 1.0)?;
    let engine = SurfaceEngine::new(mesh);
    let part = SurfacePartitioner::new(&engine);
    let treated: Vec<bool> = (0..=mesh.patches.len()).map(|pi| pi < mesh.patches.len() && layer_patches.contains(&mesh.patches[pi].name.as_str())).collect();
    // findPatchVertices: a point is a patch vertex if any of its patches is
    // treated.
    let patch_vertex: Vec<bool> = part.point_patches.iter().map(|pp| pp.iter().any(|&q| treated[q])).collect();

    let mut inward: BTreeMap<usize, Vec3> = BTreeMap::new();
    let mut totals: BTreeMap<usize, f64> = BTreeMap::new();
    let mut out_points = Vec::new();
    for &wp in &s.wall_pts {
        let b = engine.bp[wp];
        let p = mesh.points[wp];
        let (normal, mut dist, n_other) = match s.kind[&wp] {
            LayerVertex::Patch => {
                let mut dist = f64::MAX;
                for &q in &engine.point_points[b] {
                    dist = dist.min(c.height_factor * mesh.points[engine.boundary_points[q]].sub(p).length());
                }
                (engine.point_normals[b], dist, 0)
            }
            LayerVertex::Edge => {
                let (mut n, mut v) = (Vec3::ZERO, Vec3::ZERO);
                for &bf in &engine.point_faces[b] {
                    if treated[engine.face_patches[bf]] {
                        n = n.add(engine.face_normals[bf]);
                    } else {
                        v = v.add(engine.face_normals[bf]);
                    }
                }
                let v = v.scale(1.0 / (v.length() + VSMALL));
                let n = n.sub(v.scale(n.dot(v)));
                let n = n.scale(1.0 / (n.length() + VSMALL));
                let mut dist = f64::MAX;
                for &q in &engine.point_points[b] {
                    if patch_vertex[q] {
                        continue;
                    }
                    dist = dist.min(c.height_factor * mesh.points[engine.boundary_points[q]].sub(p).dot(n).abs());
                }
                (n, dist, 1)
            }
            LayerVertex::Corner => {
                let others: Vec<usize> = {
                    let mut o: Vec<usize> = Vec::new();
                    for &q in &part.point_patches[b] {
                        if !treated[q] && !o.contains(&q) {
                            o.push(q);
                        }
                    }
                    o
                };
                let q = *engine.point_points[b].iter().find(|&&q| others.iter().all(|o| part.point_patches[q].contains(o)))?;
                let d = p.sub(mesh.points[engine.boundary_points[q]]);
                let dist = c.height_factor * d.length() + VSMALL;
                (d.scale(1.0 / (2.0 * dist)), dist, 2)
            }
        };
        if n_other > 0 {
            for &bf in &engine.point_faces[b] {
                if treated[engine.face_patches[bf]] {
                    continue;
                }
                let ring = &engine.face_rings[bf];
                let k = ring.len();
                let pos = ring.iter().position(|&x| x == wp)?;
                let (e1, e2) = (mesh.points[ring[(pos + k - 1) % k]], mesh.points[ring[(pos + 1) % k]]);
                let dst = distance_to_segment(p, e1, e2);
                if dst < dist {
                    dist = 0.9 * dst;
                }
            }
        }
        dist = dist.max(c.min_height.max(VSMALL));
        let new_p = p.sub(normal.scale(dist));
        let new_p = if new_p.x.is_finite() && new_p.y.is_finite() && new_p.z.is_finite() { new_p } else { p };
        let disp = new_p.sub(p);
        let len = disp.length();
        inward.insert(wp, if len > 0.0 { disp.scale(1.0 / len) } else { Vec3::ZERO });
        totals.insert(wp, len);
        out_points.push(LayerPoint { point: wp, wall: p, moved: new_p, direction: inward[&wp], length: len, n_other_patches: n_other });
    }

    let counts: BTreeMap<usize, usize> = s.wall_pts.iter().map(|&wp| (wp, 1)).collect();
    let (layered, column) = build_layered_variable_with_columns(mesh, &s.wall_faces, &s.wall_pts, &inward, &[0.0, 1.0], &totals, &counts);

    // createNewFacesAndCells' patches: a treated face's copy keeps its
    // patch; a side quad on an edge to an untreated face takes that face's
    // patch; every other boundary face keeps its own (its wall points now
    // the tops of their columns).
    let key = |ring: &[usize]| -> Vec<usize> {
        let mut k = ring.to_vec();
        k.sort_unstable();
        k
    };
    let mut new_of = vec![usize::MAX; mesh.points.len()];
    let mut next = 0usize;
    for (op, slot) in new_of.iter_mut().enumerate() {
        if !s.wall_pts.contains(&op) {
            *slot = next;
            next += 1;
        }
    }
    let top = |p: usize| if s.wall_pts.contains(&p) { *column[&p].last().unwrap() } else { new_of[p] };
    let mut patch_of_key: BTreeMap<Vec<usize>, usize> = BTreeMap::new();
    for bf in 0..engine.boundary_faces.len() {
        let pi = engine.face_patches[bf];
        let ring = &engine.face_rings[bf];
        if treated[pi] {
            patch_of_key.insert(key(&ring.iter().map(|&p| column[&p][0]).collect::<Vec<_>>()), pi);
            let k = ring.len();
            for i in 0..k {
                let e = engine.face_edges[bf][i];
                let ef = &engine.edge_faces[e];
                if ef.len() != 2 {
                    continue;
                }
                let nei = if ef[0] == bf { ef[1] } else { ef[0] };
                if !treated[engine.face_patches[nei]] {
                    let (a, b2) = (ring[i], ring[(i + 1) % k]);
                    patch_of_key.insert(key(&[column[&a][0], column[&b2][0], column[&b2][1], column[&a][1]]), engine.face_patches[nei]);
                }
            }
        } else {
            patch_of_key.insert(key(&ring.iter().map(|&p| top(p)).collect::<Vec<_>>()), pi);
        }
    }
    let mut w = TopoMesh::from_volume_mesh(&layered);
    w.patch_names = mesh.patches.iter().map(|p| p.name.clone()).collect();
    for f in 0..w.faces.len() {
        w.region[f] = if w.neighbour[f].is_none() { patch_of_key.get(&key(&w.faces[f])).copied() } else { None };
    }
    let (out, _) = w.into_volume_mesh();
    let outcome = BoundaryLayersOutcome { points: out_points, wall_faces: s.wall_faces.clone(), first_layer_cell: mesh.n_cells };
    Some((out, outcome))
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;
    use crate::carve::carve_box;
    use crate::checks::check_quality;
    use crate::patches::{assign_patches_by_region, SurfaceRegions};
    use crate::shapes::box_surface;

    /// The unit box carved at `h`, `walls` on its four y/z sides, `inlet`
    /// at x = 0 and `outlet` at x = 1 (the `layers` doc example).
    pub(crate) fn named_box(h: f64) -> VolumeMesh {
        let (p, t) = box_surface(Vec3::ZERO, Vec3::new(1.0, 1.0, 1.0));
        let regions = SurfaceRegions::from_labels(&["walls", "walls", "walls", "walls", "walls", "walls", "walls", "walls", "outlet", "outlet", "inlet", "inlet"]);
        assign_patches_by_region(&carve_box(&p, &t, h), &p, &t, &regions).unwrap()
    }

    fn patch_faces(m: &VolumeMesh, name: &str) -> Vec<usize> {
        m.patches.iter().find(|q| q.name == name).map_or(Vec::new(), |q| (q.start_face..q.start_face + q.n_faces).collect())
    }

    /// V&V (gh:#1270) — cfMesh's layer on the carved box, by hand.
    /// Methodology: unit box carved at h 0.25 m (64 hexes), `walls` on the
    /// four y/z sides (64 faces, 80 points), `inlet`/`outlet` at x = 0 / 1;
    /// `boundary_layers_add_layer(walls)` with upstream's defaults.
    /// Predicted before the first run (from `boundaryLayersCreateVertices.C`,
    /// re-read for this test):
    /// - 128 cells (one per wall face), volume 1 to 1e-12, closed, no
    ///   inverted cell;
    /// - `walls` 64 faces; `inlet` and `outlet` 16 + 16 each (the
    ///   rim's side quads join them), every one of their points still at
    ///   x = 0 / x = 1 **exactly** (re-partitioned, not moved);
    /// - column lengths: 0.5 h = 0.125 at the 36 flat points, the 12
    ///   points on the wall–wall edges (along the diagonal) and the 24
    ///   rim points (`EDGENODE`, one other patch); at the 8 corners every
    ///   neighbour is a patch vertex, so the distance comes only from the
    ///   cap 0.9 x h/sqrt(2) = 0.15910 (the segment between the corner's
    ///   two inlet neighbours).
    #[test]
    fn cfmesh_layer_on_a_carved_box_by_hand() {
        let h = 0.25;
        let m = named_box(h);
        assert_eq!((m.cell_count(), patch_faces(&m, "walls").len()), (64, 64));
        let (l, o) = boundary_layers_add_layer(&m, &["walls"], &BoundaryLayersControls::default()).unwrap();
        assert_eq!(l.cell_count(), 128);
        assert_eq!(o.wall_faces.len(), 64);
        assert_eq!(o.points.len(), 80);
        l.validate().unwrap();
        assert!((l.total_volume() - 1.0).abs() < 1e-12);
        assert_eq!(check_quality(&l).n_negative_volume_cells, 0);
        assert_eq!(patch_faces(&l, "walls").len(), 64);
        for (name, x) in [("inlet", 0.0), ("outlet", 1.0)] {
            let fs = patch_faces(&l, name);
            assert_eq!(fs.len(), 32, "{name}");
            assert!(fs.iter().all(|&f| l.faces[f].iter().all(|&v| l.points[v].x == x)), "{name} moved");
        }
        let corner = 0.9 * h / 2f64.sqrt();
        let mut n = [0usize; 3];
        for p in &o.points {
            let at_end = p.wall.x == 0.0 || p.wall.x == 1.0;
            let on_edge = (p.wall.y == 0.0 || p.wall.y == 1.0) && (p.wall.z == 0.0 || p.wall.z == 1.0);
            let want = if at_end && on_edge { corner } else { 0.5 * h };
            assert!((p.thickness() - want).abs() < 1e-12, "{p:?}");
            n[p.n_other_patches] += 1;
        }
        assert_eq!(n, [48, 32, 0]);
    }

    /// V&V (gh:#1270) — differential: cfMesh's layer against the crate's
    /// `BackOffOnly` layer (`add_boundary_layers_adaptive_on_patches`'s
    /// body) on the same box, one layer, the crate asked for 0.125 m (= 0.5
    /// h, so its cap 0.5 x the shortest wall edge does not bind).
    /// Predicted before the first run:
    /// - both conserve the volume (1e-12) and give 128 cells; the crate
    ///   passes on its first try (no back-off);
    /// - column lengths agree (0.125, 1e-12) at the 72 points off the
    ///   corners; at the 8 corners cfMesh's is 0.159 and the crate's 0.125
    ///   (the crate takes the smaller of its cap and upstream's border cap);
    /// - the directions agree at the rim and corners (both use upstream's
    ///   projected normal, held fixed by the crate's smoothing) and at the
    ///   wall–wall edge points (diagonal by symmetry), and differ at the
    ///   flat points next to an edge (the crate smooths, cfMesh does not):
    ///   24 of the 36 flat points (y or z = 0.25, 0.75); the 12 on a wall's
    ///   mid-line (0.5) keep the flat normal by symmetry in both;
    /// - neither has an inverted cell.
    #[test]
    fn cfmesh_layer_against_back_off_on_the_box() {
        let m = named_box(0.25);
        let (l, o) = boundary_layers_add_layer(&m, &["walls"], &BoundaryLayersControls::default()).unwrap();
        let (b, ob) = super::super::add_boundary_layers_back_off(&m, &["walls"], 1, 0.125, 1.0);
        let setup = super::super::layer_setup(&m, &["walls"], 1, 0.125, 1.0).unwrap();
        let (qc, qb) = (check_quality(&l), check_quality(&b));
        println!("cfMesh: {} cells, V {:.15}, non-ortho {:.2}, skew {:.3}; back-off: {} cells, V {:.15}, non-ortho {:.2}, skew {:.3}, tries {}", l.cell_count(), l.total_volume(), qc.max_non_orthogonality_deg, qc.max_skewness, b.cell_count(), b.total_volume(), qb.max_non_orthogonality_deg, qb.max_skewness, ob.iterations);
        assert_eq!((l.cell_count(), b.cell_count()), (128, 128));
        assert!((b.total_volume() - 1.0).abs() < 1e-12 && (l.total_volume() - 1.0).abs() < 1e-12);
        assert_eq!(ob.iterations, 1);
        assert_eq!((qc.n_negative_volume_cells, qb.n_negative_volume_cells), (0, 0));
        let (mut same_len, mut corner_diff, mut dir_diff) = (0, 0, 0);
        for (p, q) in o.points.iter().zip(&ob.points) {
            assert_eq!(p.wall, q.0);
            if (p.thickness() - q.2).abs() < 1e-12 {
                same_len += 1;
            } else {
                assert!((p.thickness() - 0.9 * 0.25 / 2f64.sqrt()).abs() < 1e-12 && (q.2 - 0.125).abs() < 1e-12, "{p:?} {q:?}");
                corner_diff += 1;
            }
            if p.direction.sub(setup.inward[&p.point]).length() > 1e-12 {
                dir_diff += 1;
                let mid = |c: f64| (c - 0.5).abs() < 1e-12;
                assert!(p.n_other_patches == 0 && !mid(p.wall.y) && !mid(p.wall.z), "{p:?}");
            }
        }
        println!("column lengths equal at {same_len}, corners differing {corner_diff}, directions differing {dir_diff}");
        assert_eq!((same_len, corner_diff, dir_diff), (72, 8, 24));
    }

    /// The bare pipe (R 10 mm, 48 facets, default dual) of length `l` at
    /// cell size `h`, with its `walls`, `inlet`, `outlet`.
    pub(crate) fn bare_pipe(l: f64, h: f64) -> VolumeMesh {
        use crate::pipeline::tests::pipe_surface;
        use crate::pipeline::{surface_to_tet_dual_mesh_multipatch, TetDualOptions};
        let (p, t, regions) = pipe_surface(0.01, l, 48);
        let o = TetDualOptions { cell_size: h, n_layers: 0, ..Default::default() };
        surface_to_tet_dual_mesh_multipatch(&p, &t, &regions, &o).unwrap().0
    }

    /// One line of the pipe differential: both routes on `bare`.
    fn pipe_differential(bare: &VolumeMesh, label: &str) -> (usize, usize, usize, usize, f64, f64, f64) {
        let n_wall = patch_faces(bare, "walls").len();
        let (l, o) = boundary_layers_add_layer(bare, &["walls"], &BoundaryLayersControls::default()).unwrap();
        let (b, ob) = super::super::add_boundary_layers_back_off(bare, &["walls"], 1, 1.0, 1.0);
        let v0 = bare.total_volume();
        let (qc, qb) = (check_quality(&l), check_quality(&b));
        let stats = |t: &[f64]| {
            let (mn, mx) = t.iter().fold((f64::MAX, 0.0f64), |(a, b), &x| (a.min(x), b.max(x)));
            (mn, t.iter().sum::<f64>() / t.len().max(1) as f64, mx)
        };
        let tc: Vec<f64> = o.points.iter().map(|p| p.thickness()).collect();
        let tb: Vec<f64> = ob.points.iter().filter(|p| p.1 > 0).map(|p| p.2).collect();
        // PATCHNODE ratio crate / cfMesh.
        let ratios: Vec<f64> = o.points.iter().zip(&ob.points).filter(|(p, q)| p.n_other_patches == 0 && q.1 > 0).map(|(p, q)| q.2 / p.thickness()).collect();
        let (rmin, _, rmax) = stats(&ratios);
        let rim: Vec<(f64, f64)> = o.points.iter().zip(&ob.points).filter(|(p, _)| p.n_other_patches > 0).map(|(p, q)| (p.thickness(), q.2)).collect();
        let (cs, bs) = (stats(&tc), stats(&tb));
        println!("{label}: bare {} cells, {n_wall} wall faces, {} wall points ({} on the rim)", bare.cell_count(), o.points.len(), rim.len());
        println!(
            "  cfMesh: {} cells, dV/V {:.2e}, first layer min/mean/max {:.3}/{:.3}/{:.3} mm, non-ortho {:.1}, skew {:.2}, inverted {}, neg pyramids {}, min det {:.3}",
            l.cell_count(),
            (l.total_volume() - v0) / v0,
            cs.0 * 1e3,
            cs.1 * 1e3,
            cs.2 * 1e3,
            qc.max_non_orthogonality_deg,
            qc.max_skewness,
            qc.n_negative_volume_cells,
            qc.n_negative_pyramid_faces,
            qc.min_cell_determinant
        );
        println!(
            "  back-off: {} cells, dV/V {:.2e}, first layer min/mean/max {:.3}/{:.3}/{:.3} mm, non-ortho {:.1}, skew {:.2}, inverted {}, neg pyramids {}, min det {:.3}, tries {}",
            b.cell_count(),
            (b.total_volume() - v0) / v0,
            bs.0 * 1e3,
            bs.1 * 1e3,
            bs.2 * 1e3,
            qb.max_non_orthogonality_deg,
            qb.max_skewness,
            qb.n_negative_volume_cells,
            qb.n_negative_pyramid_faces,
            qb.min_cell_determinant,
            ob.iterations
        );
        let rim_c = stats(&rim.iter().map(|r| r.0).collect::<Vec<_>>());
        let rim_b = stats(&rim.iter().map(|r| r.1).collect::<Vec<_>>());
        println!(
            "  PATCHNODE length ratio back-off/cfMesh min {rmin:.15} max {rmax:.15}; rim cfMesh {:.3}/{:.3}/{:.3} mm, back-off {:.3}/{:.3}/{:.3} mm",
            rim_c.0 * 1e3,
            rim_c.1 * 1e3,
            rim_c.2 * 1e3,
            rim_b.0 * 1e3,
            rim_b.1 * 1e3,
            rim_b.2 * 1e3
        );
        assert!(((l.total_volume() - v0) / v0).abs() < 1e-12);
        (l.cell_count(), b.cell_count(), bare.cell_count() + n_wall, ob.iterations, rmin, rmax, 0.6f64.powi(ob.iterations.max(1) as i32 - 1))
    }

    /// V&V (gh:#1270) — differential on the pipe: cfMesh's layer against
    /// the crate's `BackOffOnly`, one layer, the crate asked for 1 m so its
    /// cap (0.5 x the shortest wall edge at the point) decides. Methodology:
    /// the demo pipe shortened to L 60 mm, h 6 mm, default dual, bare.
    /// Predicted before the first run:
    /// - cfMesh conserves the volume (1e-12 relative) and gives bare +
    ///   wall-face cells; so does the crate, unless it backs off to nothing;
    /// - at every `PATCHNODE` the crate's column is cfMesh's times the
    ///   crate's uniform back-off factor 0.6^(tries − 1), to 1e-12: both
    ///   take 0.5 x the shortest edge to a neighbouring boundary point (all
    ///   wall points there), and that is all either does to the length;
    /// - at the rim the two differ (cfMesh: 0.5 x the shortest
    ///   wall-normal distance to an inlet-interior neighbour, then its 0.9
    ///   cap; the crate: 0.5 x its shortest wall edge, then the cap);
    /// - cfMesh has no quality check: its skewness may exceed the crate's
    ///   limit (4), and it may have inverted cells; both are recorded.
    #[test]
    fn cfmesh_layer_against_back_off_on_the_pipe() {
        let bare = bare_pipe(0.06, 0.006);
        let (cc, bc, want, tries, rmin, rmax, scale) = pipe_differential(&bare, "pipe L 60 mm h 6 mm");
        assert_eq!(cc, want);
        if bc == want {
            assert!((rmin - scale).abs() < 1e-12 && (rmax - scale).abs() < 1e-12, "{rmin} {rmax} {scale} ({tries} tries)");
        }
    }

    /// Diagnostic (gh:#1270), `--ignored`: the same differential on the
    /// Foam Studio pipe (R 10 mm, L 200 mm, 48 facets, h 5 mm, bare).
    #[test]
    #[ignore]
    fn diagnose_cfmesh_layer_on_the_foam_studio_pipe() {
        let bare = bare_pipe(0.2, 0.005);
        pipe_differential(&bare, "Foam Studio pipe L 200 mm h 5 mm");
    }
}
