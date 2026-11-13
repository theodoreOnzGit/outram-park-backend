// SPDX-License-Identifier: GPL-3.0-only
// Copyright (C) 2026 OUTRAM PARK contributors
//
// Ported from cfMesh (Creative Fields, Ltd.; Franjo Juretic), GPL-3.0:
//   meshLibrary/voronoiMesh/voronoiMeshGenerator/voronoiMeshGenerator.{C,H}
//   executables/pMesh/pMesh.C
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

//! cfMesh's **`pMesh`** workflow (`voronoiMeshGenerator`, gh:#1273): the
//! `tetMesh` workflow of [`super::tet_mesh`] with the Voronoi template
//! ([`crate::voronoi_mesh_extractor`]) and upstream's differences:
//!
//! - `surfaceTopology` is `surfaceMorpherCells` alone (no
//!   `topologicalCleaner` loop).
//! - `surfaceProjection` runs `preMapVertices` first (~~not ported, surface
//!   track; noted~~ **wired 2026-11-05, gh:#1274 remainder:** `premap_vertices`, 3 iterations).
//! - `edgeExtraction` uses `meshSurfaceEdgeExtractorFUN` (~~not ported;
//!   `surface_mapper`'s patch mapping stands in~~ **UPDATED 2026-11-05,
//!   gh:#1269:** [`crate::mesh_surface_edge_extractor_fun`], ~~without its
//!   fundamental sheets, whose `extrudeLayer` is not ported~~ **with its
//!   fundamental sheets since gh:#1281** (`TetMeshOptions::fun_sheets`,
//!   `FunSheets::Without` is the ablation)), and
//!   `optimiseMeshSurface` untangles once more after `optimizeSurface`.
//! - `meshOptimisation` runs `meshSurfaceOptimizer::optimizeSurface` before
//!   the volume optimiser, in place of `meshOptimizer::optimizeSurface`.
//! - `workflowCompleted` is never called: the meta data stays on the mesh
//!   (upstream as written).
//!
//! The tables of steps and stand-ins are in [`super::tet_mesh`].

use super::tet_mesh::{run_generator, SavedMesh, Template, TetMeshOptions, TetMeshReport};
use crate::math::Vec3;
use crate::patches::SurfaceRegions;
use crate::volume_mesh::VolumeMesh;

/// cfMesh `pMesh` on the surface `(points, tris)` (module docs).
pub fn p_mesh(points: &[Vec3], tris: &[[usize; 3]], regions: Option<&SurfaceRegions>, opts: &TetMeshOptions) -> Result<(VolumeMesh, TetMeshReport), String> {
    run_generator(Template::Voronoi, points, tris, regions, opts, None)
}

/// [`p_mesh`] restarted from a saved mesh.
pub fn p_mesh_restart(points: &[Vec3], tris: &[[usize; 3]], regions: Option<&SurfaceRegions>, opts: &TetMeshOptions, saved: &SavedMesh) -> Result<(VolumeMesh, TetMeshReport), String> {
    run_generator(Template::Voronoi, points, tris, regions, opts, Some(saved))
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;
    use crate::mesh_octree_creator::MeshDict;
    use crate::pipeline::tet_mesh::create_octree;
    use crate::pipeline::tet_mesh::tests::pipe_surface;
    use crate::shapes::sphere_surface;
    use crate::tet_creator_octree::tet_creator_octree;
    use crate::tet_mesh_extractor_octree::tet_mesh_extractor_octree;
    use crate::voronoi_mesh_extractor::voronoi_mesh_extractor;
    use std::f64::consts::PI;

    fn volumes(m: &VolumeMesh) -> Vec<f64> {
        crate::checks::cell_geometry(m).0
    }

    /// The tet mesh of the creator and the Voronoi mesh of the same tets,
    /// with the tet-mesh point of each Voronoi cell.
    fn same_tets(p: &[Vec3], t: &[[usize; 3]], r: Option<&SurfaceRegions>, dict: &MeshDict) -> (VolumeMesh, crate::voronoi_mesh_extractor::VoronoiMesh, Vec<Option<usize>>) {
        let o = create_octree(p, t, r, dict);
        let tc = tet_creator_octree(&o);
        let tm = tet_mesh_extractor_octree(&tc).unwrap();
        let vm = voronoi_mesh_extractor(&tc).unwrap();
        // tet-mesh point of each Voronoi cell
        let mut point_of_cell = vec![None; vm.mesh.n_cells];
        for (tp, c) in vm.cell_of_tet_point.iter().enumerate() {
            if let Some(c) = c {
                point_of_cell[*c] = tm.point_map[tp];
            }
        }
        (tm.mesh, vm, point_of_cell)
    }

    fn interior_points(m: &VolumeMesh) -> Vec<bool> {
        let mut b = vec![false; m.point_count()];
        for f in m.n_internal_faces()..m.face_count() {
            for &v in &m.faces[f] {
                b[v] = true;
            }
        }
        b.iter().map(|x| !x).collect()
    }

    /// V&V (gh:#1273), predictions 1, 3 and 4 of
    /// `verification_and_validation/p_mesh_gh1273/README.md`: the Voronoi
    /// extraction on the sphere (0.5 m) and the graded pipe (2 mm / 1 mm),
    /// against the median dual (`MedianPerCorner`) of the same tets.
    /// Results (2026-11-05): cells = interior points (1 749 sphere, 26 587
    /// pipe), 0 non-positive, closed; median-dual volumes equal to 3.8e-15 on
    /// the uniform sphere (prediction 3 missed), up to 41 % apart on the pipe.
    #[test]
    fn voronoi_extractor_cells_are_the_interior_tet_vertices() {
        let (sp, st) = sphere_surface(Vec3::ZERO, 3.0, 24, 48);
        let (pp, pt, pr) = pipe_surface(0.01, 0.06, 48);
        let pdict = MeshDict { boundary_cell_size: Some(0.001), ..MeshDict::with_max_cell_size(0.002) };
        for (name, p, t, r, dict) in [("sphere", &sp, &st, None, MeshDict::with_max_cell_size(0.5)), ("pipe", &pp, &pt, Some(&pr), pdict)] {
            let (tm, vm, point_of_cell) = same_tets(p, t, r, &dict);
            let v = vm.mesh.validate();
            let vols = volumes(&vm.mesh);
            let n_nonpos = vols.iter().filter(|&&x| x <= 0.0).count();
            let interior = interior_points(&tm);
            let n_interior = interior.iter().filter(|&&x| x).count();
            let cells_at_interior = point_of_cell.iter().filter(|c| c.is_some_and(|q| interior[q])).count();
            println!("{name}: non-manifold boundary points of the tet template {}", non_manifold_boundary_points(&tm));
            let median = crate::dual::polyhedral_dual(&tm);
            let mvols = volumes(&median);
            let mut max_rel = 0.0f64;
            for (c, q) in point_of_cell.iter().enumerate() {
                let q = q.expect("cell at a used point");
                max_rel = max_rel.max(((vols[c] - mvols[q]) / mvols[q]).abs());
            }
            println!(
                "{name}: tets {} tet points {} interior {n_interior}; voronoi cells {} (at interior points {cells_at_interior}) faces {} edges {} boundary edges {} nonpos {n_nonpos} validate {v:?}; \
                 volume voronoi {:.6e} tet {:.6e}; median dual cells {} max rel. volume diff on the shared cells {max_rel:.3e}",
                tm.n_cells,
                tm.point_count(),
                vm.mesh.n_cells,
                vm.mesh.face_count(),
                vm.n_edges,
                vm.n_boundary_edges,
                vm.mesh.total_volume(),
                tm.total_volume(),
                median.n_cells
            );
            v.unwrap();
            assert_eq!(n_nonpos, 0, "{name}");
            assert_eq!(vm.mesh.n_cells, n_interior, "{name}: one cell per interior tet vertex");
            assert_eq!(cells_at_interior, n_interior, "{name}");
            assert!(vm.mesh.total_volume() < tm.total_volume(), "{name}");
            // MISSED prediction 3 on the sphere: on the uniform BCC interior
            // the median-dual and Voronoi cells have the same volumes
            // (3.8e-15); recorded in the V&V README, not asserted.
        }
    }

    /// Boundary points of `m` whose boundary faces form more than one
    /// edge-connected fan (non-manifold points; `polyDualMesh` refuses
    /// them).
    pub(crate) fn non_manifold_boundary_points(m: &VolumeMesh) -> usize {
        let mut faces_of: std::collections::BTreeMap<usize, Vec<usize>> = std::collections::BTreeMap::new();
        for f in m.n_internal_faces()..m.face_count() {
            for &v in &m.faces[f] {
                faces_of.entry(v).or_default().push(f);
            }
        }
        let mut n = 0;
        for (v, fs) in &faces_of {
            // an edge (v, w) of a face joins it to the other faces with that edge
            let mut comp: Vec<usize> = (0..fs.len()).collect();
            fn root(c: &mut Vec<usize>, i: usize) -> usize {
                let mut i = i;
                while c[i] != i {
                    c[i] = c[c[i]];
                    i = c[i];
                }
                i
            }
            let nbrs = |f: usize| {
                let r = &m.faces[f];
                let k = r.iter().position(|x| x == v).unwrap();
                [r[(k + 1) % r.len()], r[(k + r.len() - 1) % r.len()]]
            };
            for a in 0..fs.len() {
                for b in a + 1..fs.len() {
                    let (na, nb) = (nbrs(fs[a]), nbrs(fs[b]));
                    if na.iter().any(|x| nb.contains(x)) {
                        let (ra, rb) = (root(&mut comp, a), root(&mut comp, b));
                        comp[ra] = rb;
                    }
                }
            }
            let roots: std::collections::BTreeSet<usize> = (0..fs.len()).map(|a| root(&mut comp, a)).collect();
            if roots.len() > 1 {
                n += 1;
            }
        }
        n
    }

    /// V&V (gh:#1273), prediction 2: on the same tets, every Voronoi cell
    /// equals the `CellCentre` (`polyDualMesh`) cell of its tet vertex:
    /// same face count, volume to 1e-12 relative. `polyDualMesh` refuses a
    /// tet mesh with a non-manifold boundary point; such cases are recorded
    /// (count of those points) and skipped. Results (2026-11-05): sphere and
    /// pipe refused (96 / 2 240 non-manifold points); box: 1 241 cells, same
    /// face counts, volumes to 9.8e-15.
    #[cfg(feature = "cell-centre-dual")]
    #[test]
    fn voronoi_cells_equal_the_cell_centre_dual_inside() {
        let (sp, st) = sphere_surface(Vec3::ZERO, 3.0, 24, 48);
        let (pp, pt, pr) = pipe_surface(0.01, 0.06, 48);
        let (bp, bt) = crate::shapes::box_surface(Vec3::new(0.05, 0.05, 0.05), Vec3::new(0.95, 0.95, 0.95));
        let pdict = MeshDict { boundary_cell_size: Some(0.001), ..MeshDict::with_max_cell_size(0.002) };
        let mut compared = 0;
        for (name, p, t, r, dict) in [
            ("sphere", &sp, &st, None, MeshDict::with_max_cell_size(0.5)),
            ("pipe", &pp, &pt, Some(&pr), pdict),
            ("box", &bp, &bt, None, MeshDict::with_max_cell_size(0.1)),
        ] {
            let (tm, vm, point_of_cell) = same_tets(p, t, r, &dict);
            let nm = non_manifold_boundary_points(&tm);
            let cc = match crate::cell_centre_dual::cell_centre_dual(&tm, 45.0) {
                Ok(c) => c,
                Err(e) => {
                    println!("{name}: polyDualMesh refuses the tet mesh ({e}); non-manifold boundary points {nm}");
                    assert!(nm > 0, "{name}: refused although manifold");
                    continue;
                }
            };
            let vols = volumes(&vm.mesh);
            let cvols = volumes(&cc);
            let nf = |m: &VolumeMesh| {
                let mut n = vec![0usize; m.n_cells];
                for f in 0..m.face_count() {
                    n[m.owner[f]] += 1;
                    if let Some(x) = m.neighbour[f] {
                        n[x] += 1;
                    }
                }
                n
            };
            let (vf, cf) = (nf(&vm.mesh), nf(&cc));
            let mut max_rel = 0.0f64;
            let mut face_count_diff = 0;
            for (c, q) in point_of_cell.iter().enumerate() {
                let q = q.unwrap();
                max_rel = max_rel.max(((vols[c] - cvols[q]) / cvols[q]).abs());
                if vf[c] != cf[q] {
                    face_count_diff += 1;
                }
            }
            println!(
                "{name}: non-manifold boundary points {nm}; voronoi cells {} cell-centre cells {}; max rel volume diff {max_rel:.3e}; face-count differences {face_count_diff}",
                vm.mesh.n_cells, cc.n_cells
            );
            assert_eq!(face_count_diff, 0, "{name}");
            assert!(max_rel < 1e-12, "{name}: {max_rel}");
            compared += 1;
        }
        assert!(compared > 0, "no case could be compared");
    }

    /// V&V (gh:#1273), prediction 5: the `pMesh` workflow on the sphere and
    /// the pipe against `tetMesh`. Results (2026-11-05), with gh:#1267 /
    /// #1259 wired in: −1.54 % (sphere, 1 749 cells) and −0.35 % (pipe,
    /// 4 303 cells), 0 non-positive. Before they landed: −0.97 % / −0.05 %.
    /// **Re-measured 2026-11-05 (gh:#1269)** with `edgeExtractor` and
    /// `meshSurfaceEdgeExtractorFUN`'s order wired in: sphere −1.537 %
    /// (unchanged), pipe −0.325 %, max non-orthogonality 55.29° (was
    /// 75.9°). Record: `p_mesh_gh1273/`, `edge_extractor_gh1269/`.
    /// **Re-measured 2026-11-05 (gh:#1281)** with `createFundamentalSheetsJFS`
    /// (default): pipe 9 781 cells, −0.320 %, 58.24°, skew 0.690; the
    /// `FunSheets::Without` row (also run here) reproduces 4 303 / −0.325 %
    /// / 55.29° / 0.577; sphere unchanged (no regions, FUN not run).
    #[test]
    fn diagnose_p_mesh_workflow() {
        let (sp, st) = sphere_surface(Vec3::ZERO, 3.0, 24, 48);
        let (pp, pt, pr) = pipe_surface(0.01, 0.06, 48);
        let pipe_vol = 0.5 * 48.0 * 0.01f64.powi(2) * (2.0 * PI / 48.0).sin() * 0.06;
        use crate::mesh_surface_edge_extractor_fun::FunSheets;
        for (name, p, t, r, h, exact, fun) in [
            ("sphere", &sp, &st, None, 0.5, 4.0 / 3.0 * PI * 27.0, FunSheets::Jfs),
            ("pipe", &pp, &pt, Some(&pr), 0.002, pipe_vol, FunSheets::Jfs),
            ("pipe without sheets", &pp, &pt, Some(&pr), 0.002, pipe_vol, FunSheets::Without),
        ] {
            let opts = TetMeshOptions { dict: MeshDict::with_max_cell_size(h), fun_sheets: fun, ..Default::default() };
            let t0 = std::time::Instant::now();
            let (m, rep) = p_mesh(p, t, r, &opts).unwrap();
            let dt = t0.elapsed().as_secs_f64();
            let q = crate::checks::check_quality(&m);
            let nonpos = volumes(&m).iter().filter(|&&x| x <= 0.0).count();
            let v = m.validate();
            println!(
                "{name}: p_mesh cells {} template {} (vol {:.6e}) vol {:.6e} err {:+.3} % nonpos {nonpos} validate {v:?} non-orth {:.2} skew {:.3} time {dt:.2} s; meta kept {:?}",
                m.n_cells,
                rep.n_template_cells,
                rep.template_volume,
                m.total_volume(),
                (m.total_volume() - exact) / exact * 100.0,
                q.max_non_orthogonality_deg,
                q.max_skewness,
                rep.meta.last_step
            );
            for n in &rep.notes {
                println!("  {n}");
            }
            v.unwrap();
            assert_eq!(nonpos, 0, "{name}");
            assert!(rep.meta.last_step.is_some(), "pMesh never calls workflowCompleted");
        }
    }
}
