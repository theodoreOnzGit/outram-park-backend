// SPDX-License-Identifier: GPL-3.0-only
// Copyright (C) 2026 OUTRAM PARK contributors
//
// Ported from cfMesh (Creative Fields, Ltd.; Franjo Juretic), GPL-3.0:
//   meshLibrary/utilities/surfaceTools/meshSurfaceMapper/meshSurfaceMapperPremapVertices.C
//   meshLibrary/utilities/surfaceTools/meshSurfaceMapper/meshSurfaceMapperCornersAndEdges.C
//     (faceMetricInPatch only)
// Source: the cfMesh v1.1 line as carried by github.com/DAFoam/cfmesh, commit
// e944f6ba695fcefce73ad306074f6b3c0ea432ae.
// Copyright (C) Creative Fields, Ltd. Licence: GPL-3.0-or-later (cfMesh files
// say "either version 3 of the License, or (at your option) any later
// version"), conveyed here under GPL-3.0-only.
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

//! cfMesh **`meshSurfaceMapper::preMapVertices`** (gh:#1274): smooth the
//! mesh surface half-way towards the geometry before
//! `mapVerticesOntoSurface`. `cartesianMeshGenerator::mapMeshToSurface`
//! (and `voronoiMeshGenerator`'s) call it; nothing else in this crate does.
//!
//! Per iteration (upstream default 3):
//!
//! 1. **Patches near each boundary face.** The octree leaves overlapping
//!    the box `centre ± max |centre − point|` give the surface regions of
//!    their triangles; the face takes the region of least
//!    [`face_metric_in_patch`], and every point of the face collects it
//!    (`appendIfNotIn`).
//! 2. **Shrinking Laplace.** Each boundary point's target is the average of
//!    its faces' centres, weighted by `max(|p − c|² / d₀², SMALL)`, where
//!    `d₀²` is the point's squared distance to that face centre **before
//!    the first iteration** (upstream computes it once).
//! 3. **Half a step to the surface.** The target is projected onto the one
//!    region (`findNearestSurfacePointInRegion`) or onto the several
//!    (`findNearestPointToPatches`), and the point moves **half way** from
//!    where it is to that projection.
//! 4. `meshSurfaceOptimizer::untangleSurface()`.
//!
//! **Where the port is explicit about upstream's edge cases:** a face with
//! no triangle in its box gets upstream's `bestPatch = -1`; a point whose
//! only collected patch is that `-1` is not moved here (upstream's region
//! search with region `-1` finds no triangle and leaves `pMap = p`, so
//! `newP = p`); a `-1` among several patches is dropped. The searches use
//! the octree with upstream's stopping rule and projection
//! ([`NearestSearch::Upstream`], [`TriangleProjection::Upstream`]). MPI is
//! not ported.

use crate::correct_edges::foam_face_centre;
use crate::math::Vec3;
use crate::mesh_octree::{BoundBox, MeshOctree};
use crate::mesh_octree_queries::{NearestSearch, TriangleProjection};
use crate::patches::SurfaceRegions;
use crate::surface_engine::{ring_area_vector, SurfaceEngine, SurfacePartitioner};
use crate::surface_mapper::SurfaceQueries;
use crate::surface_optimizer::MeshSurfaceOptimizer;
use crate::volume_mesh::VolumeMesh;

/// OpenFOAM `SMALL` for `double`.
const SMALL: f64 = 1.0e-15;

/// What [`pre_map_vertices`] did.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct PreMapReport {
    /// Iterations run.
    pub n_iterations: usize,
    /// Boundary points moved in the first iteration.
    pub n_moved: usize,
    /// Boundary points left where they were (no patch found near them).
    pub n_without_patch: usize,
    /// Mean distance of the boundary points to the surface before the
    /// first iteration and after the last.
    pub mean_distance: (f64, f64),
    /// Inverted points `untangleSurface` left after the last iteration.
    pub n_inverted_last: usize,
}

/// `findNearestSurfacePointInRegion(region, p)`: the nearest point of the
/// region with upstream's search, `None` where upstream finds nothing.
fn nearest_in_region(o: &MeshOctree, p: Vec3, region: usize) -> Option<Vec3> {
    o.find_nearest_surface_point_with(p, NearestSearch::Upstream, TriangleProjection::Upstream, Some(region)).map(|h| h.point)
}

/// cfMesh `meshSurfaceMapper::faceMetricInPatch(bfI, patchI)`: how well
/// boundary face `ring` would sit on region `patch`, as the squared distance
/// of its centre to the region plus the change of its area when every
/// point and the centre are projected onto the region. A region search
/// that finds nothing gives `f64::MAX` (the face never picks that region).
pub fn face_metric_in_patch(o: &MeshOctree, points: &[Vec3], ring: &[usize], patch: usize) -> f64 {
    let centre = foam_face_centre(points, ring);
    let area = ring_area_vector(points, ring);
    let Some(proj_centre) = nearest_in_region(o, centre, patch) else {
        return f64::MAX;
    };
    let mut proj = Vec::with_capacity(ring.len());
    for &v in ring {
        match nearest_in_region(o, points[v], patch) {
            Some(q) => proj.push(q),
            None => return f64::MAX,
        }
    }
    let mut proj_area = Vec3::ZERO;
    for i in 0..proj.len() {
        // triPointRef(a, b, c).normal() = 0.5 (b − a) x (c − a)
        let a = proj[i];
        let b = proj[(i + 1) % proj.len()];
        proj_area = proj_area.add(b.sub(a).cross(proj_centre.sub(a)).scale(0.5));
    }
    let d = centre.sub(proj_centre);
    d.dot(d) + (proj_area.length() - area.length()).abs()
}

/// cfMesh `meshSurfaceMapper(mse, octree).preMapVertices(nIterations)` on
/// the boundary of `mesh` (module docs). The surface is the octree's;
/// `points`, `tris` and `regions` are the same surface for the
/// untangler's re-mapping ([`SurfaceQueries`], as
/// [`crate::surface_optimizer::map_mesh_to_surface`] uses). The whole
/// boundary is one patch here, as upstream's at that stage.
pub fn pre_map_vertices(mesh: &mut VolumeMesh, o: &MeshOctree, points: &[Vec3], tris: &[[usize; 3]], regions: &SurfaceRegions, n_iterations: usize) -> PreMapReport {
    let one = vec![0usize; mesh.face_count()];
    let mut engine = SurfaceEngine::from_parts(&mesh.points, &mesh.faces, &mesh.owner, &mesh.neighbour, &one);
    let nb = engine.n_boundary_points();
    let octree_arc = std::sync::Arc::new(o.clone());
    let q = SurfaceQueries::new(points, tris, regions);
    let mean_dist = |m: &VolumeMesh, e: &SurfaceEngine| {
        let s: f64 = e.boundary_points.iter().map(|&v| q.nearest(m.points[v]).sub(m.points[v]).length()).sum();
        s / e.boundary_points.len().max(1) as f64
    };
    let mut rep = PreMapReport { mean_distance: (mean_dist(mesh, &engine), 0.0), ..Default::default() };
    // faceCentreDistances: computed once, before the iterations
    let fcd: Vec<Vec<f64>> = engine
        .face_rings
        .iter()
        .enumerate()
        .map(|(bf, ring)| {
            let c = engine.face_centres[bf];
            ring.iter().map(|&v| {
                let d = mesh.points[v].sub(c);
                d.dot(d)
            }).collect()
        })
        .collect();
    for iter in 0..n_iterations {
        // patches in the vicinity of each boundary face
        let mut bpp: Vec<Vec<Option<usize>>> = vec![Vec::new(); nb];
        for bf in 0..engine.face_rings.len() {
            let ring = &engine.face_rings[bf];
            let fc = engine.face_centres[bf];
            let bs = ring.iter().fold(0.0f64, |m, &v| m.max(fc.sub(mesh.points[v]).length()));
            let bb = BoundBox { min: fc.sub(Vec3::new(bs, bs, bs)), max: fc.add(Vec3::new(bs, bs, bs)) };
            let mut patches: Vec<usize> = Vec::new();
            for l in o.find_leaves_contained_in_box(&bb) {
                for t in o.contained_triangles(l) {
                    let r = o.surface.regions[t];
                    if !patches.contains(&r) {
                        patches.push(r);
                    }
                }
            }
            let mut metric = f64::MAX;
            let mut best = None;
            for &pt in &patches {
                let m = face_metric_in_patch(o, &mesh.points, ring, pt);
                if m < metric {
                    metric = m;
                    best = Some(pt);
                }
            }
            for &v in ring {
                let row = &mut bpp[engine.bp[v]];
                if !row.contains(&best) {
                    row.push(best);
                }
            }
        }
        // the shrinking Laplace, then half a step to the surface
        let mut targets = Vec::with_capacity(nb);
        for b in 0..nb {
            let p = mesh.points[engine.boundary_points[b]];
            let (mut c, mut s) = (Vec3::ZERO, 0.0);
            for (k, &bf) in engine.point_faces[b].iter().enumerate() {
                let fc = engine.face_centres[bf];
                let pos = engine.point_in_faces[b][k];
                let d = p.sub(fc);
                let w = (d.dot(d) / fcd[bf][pos]).max(SMALL);
                c = c.add(fc.scale(w));
                s += w;
            }
            // `lps.coordinates() /= lps.scalarValue()`: each component divided
            targets.push(c.div(s));
        }
        let mut n_moved = 0;
        let mut n_without = 0;
        for b in 0..nb {
            let p = mesh.points[engine.boundary_points[b]];
            let known: Vec<usize> = bpp[b].iter().flatten().copied().collect();
            let p_map = if bpp[b].len() == 1 {
                match bpp[b][0] {
                    Some(r) => nearest_in_region(o, targets[b], r),
                    None => None,
                }
            } else {
                o.find_nearest_point_to_patches(targets[b], &known, 1e-4).map(|x| x.0)
            };
            match p_map {
                Some(pm) => {
                    engine.move_boundary_vertex_no_update(&mut mesh.points, b, p.add(pm.sub(p).scale(0.5)));
                    n_moved += 1;
                }
                None => n_without += 1,
            }
        }
        if iter == 0 {
            rep.n_moved = n_moved;
            rep.n_without_patch = n_without;
        }
        engine.update_geometry_all(&mesh.points);
        // meshSurfaceOptimizer(surfaceEngine_, meshOctree_).untangleSurface()
        let part = SurfacePartitioner::new(&engine);
        let mut opt = MeshSurfaceOptimizer::new(engine.clone(), part, Some(SurfaceQueries::new(points, tris, regions).with_octree(octree_arc.clone())));
        let u = opt.untangle_surface(&mut mesh.points, 2);
        rep.n_inverted_last = u.n_inverted_last;
        engine.update_geometry_all(&mesh.points);
        rep.n_iterations += 1;
    }
    rep.mean_distance.1 = mean_dist(mesh, &engine);
    rep
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::cartesian_mesh::{cartesian_mesh_extractor, CartesianMeshOptions};
    use crate::mesh_octree_creator::MeshDict;
    use crate::pipeline::tet_mesh::create_octree;
    use crate::shapes::sphere_surface;

    /// V&V (gh:#1274), prediction C3 of
    /// `verification_and_validation/cartesian_mesh_gh1274/README.md`: on the
    /// sphere's cartesian template (R 3, `maxCellSize` 0.5) every boundary
    /// point finds a patch, and three iterations of half steps bring the
    /// mean distance to the surface down, to between 1/8 and 1/2 of where
    /// it started (three halvings, less what the smoothing pulls back).
    /// Result: in the README.
    #[test]
    fn pre_map_vertices_halves_the_distance_on_the_sphere() {
        let (p, t) = sphere_surface(Vec3::ZERO, 3.0, 24, 48);
        let o = create_octree(&p, &t, None, &MeshDict::with_max_cell_size(0.5));
        let mut m = cartesian_mesh_extractor(&o, &CartesianMeshOptions::default()).mesh;
        let reg = SurfaceRegions::single("defaultFaces", t.len());
        let rep = pre_map_vertices(&mut m, &o, &p, &t, &reg, 3);
        println!("preMapVertices: {rep:?}; ratio {:.4}", rep.mean_distance.1 / rep.mean_distance.0);
        assert_eq!(rep.n_iterations, 3);
        assert_eq!(rep.n_without_patch, 0);
        let ratio = rep.mean_distance.1 / rep.mean_distance.0;
        assert!(ratio > 0.0 && ratio < 0.5, "ratio {ratio}");
        m.validate().unwrap();
    }
}
