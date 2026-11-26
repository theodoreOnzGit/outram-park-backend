// SPDX-License-Identifier: AGPL-3.0-only
// Copyright (C) 2026 OUTRAM PARK contributors
//
// Provenance: original OUTRAM PARK code. It introduces no meshing algorithm:
// it calls, in upstream's order, this crate's ports of cfMesh v1.1
// (github.com/DAFoam/cfmesh, commit e944f6ba695f, GPL-3.0-or-later,
// Copyright (C) 2014-2017 Creative Fields, Ltd.):
//   meshLibrary/tetMesh/tetMeshGenerator/tetMeshGenerator.C
//     (generateBoundaryLayers, optimiseFinalMesh, refBoundaryLayers)
//   meshLibrary/utilities/smoothers/geometry/meshOptimizer/optimizeMeshFV.C
//     (meshOptimizer::optimizeBoundaryLayer, `optimiseLayer`)

//! **cfMesh's `tetMesh` layer steps on the pipeline's tetrahedra**
//! (gh:#1824): [`LayerQualityStrategy::CfMeshTetMesh`].
//!
//! # What upstream does on a tet mesh
//!
//! `tetMeshGenerator::generateMesh` adds layers in three steps, in this
//! order (`tetMeshGenerator.C`):
//!
//! 1. **`boundaryLayerGeneration`** (`generateBoundaryLayers`):
//!    `boundaryLayers::addLayerForPatch` for every patch under
//!    `patchBoundaryLayers` (or `addLayerForAllPatches` with a global
//!    `nLayers`). One layer of cells, each boundary point moved inward by
//!    `boundaryLayerHeightFactor` (0.5) x its shortest distance to a
//!    neighbouring boundary point (`createNewVertex`), with **no check of
//!    the cells it shrinks**. Ported as
//!    [`crate::layers::boundary_layers::generate_layer`].
//! 2. **`meshOptimisation`** (`optimiseFinalMesh`): `optimizeSurface`,
//!    `optimizeMeshFV`, `optimizeLowQualityFaces`,
//!    **`optimizeBoundaryLayer(false)`**, `untangleMeshFV`.
//!    `optimizeBoundaryLayer` acts only when the `boundaryLayers` dict has
//!    `optimiseLayer 1` (`optimizeMeshFV.C`): it then runs
//!    `boundaryLayerOptimisation::optimiseLayer` (hair-normal smoothing and
//!    local thickness reduction). Ported as
//!    [`crate::boundary_layer_optimisation::optimize_boundary_layer`].
//! 3. **`boundaryLayerRefinement`** (`refBoundaryLayers`):
//!    `refineBoundaryLayers` splits the one layer into `nLayers` with
//!    `thicknessRatio` and `maxFirstLayerThickness`, then
//!    `untangleBoundaryLayer` with the layer points locked. Ported as
//!    `cartesian_mesh::ref_boundary_layers`, the step `tet_mesh` runs.
//!
//! The dict this route stands for is, per layer patch `<p>`:
//!
//! ```text
//! boundaryLayers
//! {
//!     optimiseLayer 1;
//!     patchBoundaryLayers { <p> { nLayers n; thicknessRatio expansion;
//!                                 maxFirstLayerThickness first; } }
//! }
//! ```
//!
//! # Why `optimiseLayer` is on, and the rest of `optimiseFinalMesh` is not
//!
//! Measured on TetGen's tetrahedra of the 1 cm test pipe (gh:#1824,
//! `tests::diagnose_cfmesh_tet_mesh_steps_on_tetgen_tets`): step 1 alone
//! inverts **7** cells. They are tetrahedra that touch the wall at an edge
//! or a face and whose other vertices lie 0.2-0.8 mm from the wall, closer
//! than the 0.5 x boundary-edge move (upstream's rule assumes the octree
//! template's wall cells, which are deeper). `optimiseLayer` thins 16
//! hairs and leaves **0** inverted cells; refinement then keeps 0. The
//! other steps of `optimiseFinalMesh`, as this crate ports them, make
//! TetGen's tetrahedra worse: `optimizeMeshFV` and `untangleMeshFV` took
//! the 25 bad faces to over 1 200 and the 7 inverted cells to 287-401, in
//! 65-200 s. So they run only when the caller asks for the whole
//! `optimiseFinalMesh` ([`super::TetDualOptions::optimise_final_mesh`],
//! off by default, as for every other strategy). That is a limitation of
//! the port on unstructured tetrahedra, not a claim about upstream, whose
//! binary was not run here.
//!
//! # One rule that is the port's, not upstream's
//!
//! Upstream keeps the result of every untangle. Here each of the two
//! untangle steps (the `optimizeMeshFV` / `untangleMeshFV` pair inside
//! `optimizeBoundaryLayer`, and `untangleBoundaryLayer` after the
//! refinement) is kept **unless it leaves more non-positive cells than it
//! was given** (`keep_unless_more_inverted`), and the report says which
//! were not kept. Measured on Mesh Studio's 2 m shapes (2026-11-26,
//! native): on the fuel channel the locked untangles took 6 inverted
//! cells to 0, and `untangleBoundaryLayer` then took the refined layer's 0
//! to 1 463 (not kept); on the pebble both untangles made it worse (6 to
//! 167, 6 to 2 089; neither kept). The rule checks nothing new and
//! relaxes nothing: the pipeline's acceptance gate still refuses any
//! inverted cell, and the pebble's 6 are refused. The untangler defect
//! is gh:#1828; remove the rule once it is fixed.
//!
//! # Thickness
//!
//! Upstream's layer is as thick as step 1 makes it (half the local
//! boundary edge, less where `optimiseLayer` thins it); the requested
//! first thickness only caps the first of the `n` layers
//! (`maxFirstLayerThickness`), and `thicknessRatio` grades them. The total
//! is **not** the requested `first x (1 + r + ... + r^(n-1))`; the
//! [`LayerOutcome`] records each column's actual total.
//!
//! **Untrusted AI-assisted draft pending human V&V.** Education, research
//! and V&V only, per the workspace `RESPONSIBLE_USE.md`.

#[cfg(not(any(feature = "std", test)))]
#[allow(unused_imports)]
use crate::nostd_prelude::*;

use crate::boundary_layer_optimisation::BoundaryLayerOptimisationControls;
use crate::layers::boundary_layers::{generate_layer, BoundaryLayersControls, LayerRequest};
use crate::boundary_layer_optimisation::{
    optimise_layer_hairs, untangle_locked_layer, OptimizeBoundaryLayerSettings,
};
use crate::layers::refine_boundary_layers::{
    detect_boundary_layers, refine_boundary_layers, PatchLayerSettings,
    RefineBoundaryLayersSettings, RefineError,
};
use crate::pipeline::cartesian_mesh::{n_nonpositive_cells, untangle_refined_layer};
use crate::layers::{LayerOutcome, LayerQualityStrategy};
use crate::math::Vec3;
use crate::mesh_optimizer::UntangleControls;
use crate::patches::SurfaceRegions;
use crate::pipeline::tet_mesh::{generator_optimise_final_mesh, Template};
use crate::volume_mesh::VolumeMesh;

/// The `refineBoundaryLayers` settings of the route's dict: every layer
/// patch with `nLayers n`, `thicknessRatio expansion` and
/// `maxFirstLayerThickness first`.
fn refine_settings(
    layer_patches: &[&str],
    n_layers: usize,
    first: f64,
    expansion: f64,
) -> RefineBoundaryLayersSettings {
    let mut s = RefineBoundaryLayersSettings::default();
    for p in layer_patches {
        s.patches.insert(
            (*p).into(),
            PatchLayerSettings {
                n_layers: Some(n_layers),
                thickness_ratio: Some(expansion),
                max_first_layer_thickness: Some(first),
                allow_discontinuity: false,
            },
        );
    }
    s
}

/// Grow `n_layers` layers on `layer_patches` of `mesh` by cfMesh's
/// `tetMesh` steps (module docs). `optimise_layer` is
/// `boundaryLayers/optimiseLayer`'s parameters. With `full_optimise` (the
/// surface and its regions) step 2 is the whole `optimiseFinalMesh`
/// (`tetMeshGenerator`'s, without the octree); without, only its
/// `optimizeBoundaryLayer(false)`.
///
/// Returns the mesh and what the layer did. The mesh is **not** checked
/// here, as upstream does not check it; the pipeline keeps it only if it
/// passes its acceptance gate. On a refusal (a `boundaryLayers` or
/// `refineBoundaryLayers` case the port does not handle, or `n_layers` 0)
/// the mesh comes back unchanged with a note saying why.
pub(crate) fn add_boundary_layers_cfmesh_tet_mesh(
    mesh: &VolumeMesh,
    layer_patches: &[&str],
    n_layers: usize,
    first_thickness: f64,
    expansion: f64,
    optimise_layer: &BoundaryLayerOptimisationControls,
    full_optimise: Option<(&[Vec3], &[[usize; 3]], &SurfaceRegions)>,
) -> (VolumeMesh, LayerOutcome) {
    let strategy = LayerQualityStrategy::CfMeshTetMesh;
    let refused = |why: String| {
        let mut o = LayerOutcome::none(strategy, n_layers);
        o.notes.push(why);
        (mesh.clone(), o)
    };
    if n_layers == 0 {
        return refused("no layer asked for".into());
    }
    let mut notes = Vec::new();

    // 1. boundaryLayerGeneration: addLayerForPatch per layer patch.
    let request = LayerRequest::Patches(layer_patches.iter().map(|p| (*p).to_string()).collect());
    let (mut m, generated) =
        match generate_layer(mesh, &request, &BoundaryLayersControls::default(), false) {
            Ok(x) => x,
            Err(e) => return refused(format!("boundaryLayerGeneration refused: {e}")),
        };
    if generated.n_layer_cells == 0 {
        return refused(format!(
            "boundaryLayerGeneration added no cell ({})",
            generated.summary()
        ));
    }
    let inverted = n_nonpositive_cells;
    notes.push(format!(
        "boundaryLayerGeneration: {}; {} non-positive cells",
        generated.summary(),
        inverted(&m)
    ));
    // The hairs of the one layer, before step 2 (upstream's height).
    let hairs = detect_boundary_layers(&m).hair_edges;
    let height: Vec<f64> = hairs
        .iter()
        .map(|h| m.points[h[1]].sub(m.points[h[0]]).length())
        .collect();

    // 2. meshOptimisation.
    let controls = UntangleControls {
        boundary_moves: true,
        mesh_untangler: true,
        ..UntangleControls::default()
    };
    match full_optimise {
        Some((points, tris, regions)) => match generator_optimise_final_mesh(
            Template::Tet,
            &mut m,
            points,
            tris,
            regions,
            false,
            Some(optimise_layer),
            None,
        ) {
            Ok(line) => notes.push(format!(
                "meshOptimisation: optimiseFinalMesh: {line}; {} non-positive cells",
                inverted(&m)
            )),
            Err(e) => return refused(format!("meshOptimisation refused: {e}")),
        },
        None => {
            // optimizeBoundaryLayer(false): optimiseLayer, then its two
            // untangles with the layer cells locked.
            let ob = OptimizeBoundaryLayerSettings {
                optimise_layer: true,
                add_buffer_layer: false,
                controls: optimise_layer.clone(),
                untangle: controls,
            };
            let (r, layer_cells) = optimise_layer_hairs(&mut m, &ob);
            notes.push(format!(
                "meshOptimisation: optimizeBoundaryLayer(false): optimiseLayer {} hairs, {} passes, {} hairs thinned; {} non-positive cells (the rest of optimiseFinalMesh not run: TetDualOptions::optimise_final_mesh is off)",
                r.n_hairs,
                r.passes,
                r.n_thinned,
                inverted(&m)
            ));
            if r.ran {
                let mut u = m.clone();
                let reps = untangle_locked_layer(&mut u, &layer_cells, &controls);
                let what = format!(
                    "optimizeBoundaryLayer's optimizeMeshFV / untangleMeshFV (bad faces {:?})",
                    reps.iter()
                        .map(|x| (x.flagged_before, x.flagged_after))
                        .collect::<Vec<_>>()
                );
                keep_unless_more_inverted(&mut m, u, &what, &mut notes);
            }
        }
    }
    // Each column: its wall point, the layers it gets, its total thickness
    // (after the thinning) and upstream's height before it.
    let points: Vec<(Vec3, usize, f64, f64)> = hairs
        .iter()
        .zip(&height)
        .map(|(h, &c)| {
            (
                m.points[h[0]],
                n_layers,
                m.points[h[1]].sub(m.points[h[0]]).length(),
                c,
            )
        })
        .collect();

    // 3. boundaryLayerRefinement: refineBoundaryLayers, then
    // untangleBoundaryLayer.
    let s = refine_settings(layer_patches, n_layers, first_thickness, expansion);
    let rr =
        match refine_boundary_layers(&m, &s) {
            Ok((r, rr)) => {
                m = r;
                rr
            }
            Err(RefineError::NotPorted(msg)) => return refused(format!(
                "boundaryLayerRefinement: refineBoundaryLayers NOT PORTED for this mesh ({msg})"
            )),
        };
    notes.push(format!(
        "boundaryLayerRefinement: refineBoundaryLayers split {} cells; {} non-positive cells",
        rr.n_cells_split,
        inverted(&m)
    ));
    let mut u = m.clone();
    match untangle_refined_layer(&mut u, &rr, &controls) {
        Ok((line, _)) => keep_unless_more_inverted(&mut m, u, &line, &mut notes),
        Err(e) => notes.push(format!(
            "untangleBoundaryLayer refused ({e}); the refined layer is kept as it was"
        )),
    }

    // What grew: one column per hair, every layer face with all n.
    let faces = layer_face_centres(mesh, layer_patches)
        .into_iter()
        .map(|c| (c, n_layers))
        .collect();
    let mut outcome = LayerOutcome {
        strategy,
        n_layers_requested: n_layers,
        points,
        faces,
        iterations: 1,
        ..Default::default()
    };
    outcome.notes = notes;
    (m, outcome)
}

/// Keep `candidate`, the result of one of upstream's untangle steps,
/// unless it leaves **more** non-positive cells than `m`, the mesh it was
/// given; either way say so in `notes`.
///
/// Not upstream (it keeps every untangle). The port's untanglers
/// (`optimizeMeshFV`, `optimizeLowQualityFaces`, `untangleMeshFV`) were
/// measured making TetGen's tetrahedra worse (gh:#1824, module docs): on
/// the Mesh Studio fuel channel the refined layer had 0 inverted cells
/// and `untangleBoundaryLayer` left 1 463 (gh:#1828). This rule keeps the port from
/// destroying a mesh it was meant to repair; it relaxes no quality check
/// (the pipeline's acceptance gate still refuses any inverted cell).
fn keep_unless_more_inverted(
    m: &mut VolumeMesh,
    candidate: VolumeMesh,
    what: &str,
    notes: &mut Vec<String>,
) {
    let (before, after) = (n_nonpositive_cells(m), n_nonpositive_cells(&candidate));
    if after > before {
        notes.push(format!("{what}: NOT KEPT, it would leave {after} non-positive cells where it was given {before}"));
    } else {
        notes.push(format!("{what}; {after} non-positive cells"));
        *m = candidate;
    }
}

/// Centres of the input's boundary faces on `layer_patches`, in face order.
fn layer_face_centres(mesh: &VolumeMesh, layer_patches: &[&str]) -> Vec<Vec3> {
    let mut out = Vec::new();
    for p in mesh
        .patches
        .iter()
        .filter(|p| layer_patches.contains(&p.name.as_str()))
    {
        for f in p.start_face..p.start_face + p.n_faces {
            out.push(mesh.face_centroid(f));
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::checks::check_quality;
    use crate::layers::{build_layered_variable, layer_setup, meets_layer_quality};
    use crate::pipeline::staged::{
        surface_to_tet_dual_mesh_multipatch_staged, PipelineStage, StagedMesh,
    };
    use crate::pipeline::tests::pipe_surface;
    use crate::pipeline::{
        acceptable, surface_to_tet_dual_mesh, surface_to_tet_dual_mesh_multipatch, TetDualOptions,
        TetSource,
    };
    use crate::shapes::box_surface;

    fn tet_opts(
        source: TetSource,
        n_layers: usize,
        strategy: LayerQualityStrategy,
    ) -> TetDualOptions {
        TetDualOptions {
            cell_size: 0.006,
            n_layers,
            first_layer_thickness: 3e-4,
            expansion: 1.2,
            layer_patches: alloc::vec!["walls".into()],
            dual: false,
            tet_source: source,
            layer_strategy: strategy,
            ..Default::default()
        }
    }

    /// The properties a tet + layers mesh must have (no regression pin):
    /// valid, no inverted cell, every wall face with a column of exactly
    /// `n` layer cells numbered 1..n from the wall, the layer appended
    /// after the `n_tets` tetrahedra, and the volume of the tetrahedra
    /// kept within `rel_tol`. Returns the relative volume change.
    fn assert_layered(
        s: &StagedMesh,
        n_tets: usize,
        v_tets: f64,
        n: usize,
        rel_tol: f64,
        what: &str,
    ) -> f64 {
        assert!(acceptable(&s.mesh), "{what}: not acceptable");
        assert_eq!(s.report.n_negative_volume_cells, 0, "{what}");
        let walls: usize = s
            .mesh
            .patches
            .iter()
            .filter(|q| q.name == "walls")
            .map(|q| q.n_faces)
            .sum();
        assert_eq!(
            s.first_layer_cell,
            Some(n_tets),
            "{what}: layer cells appended after the tetrahedra"
        );
        assert_eq!(
            s.mesh.n_cells,
            n_tets + n * walls,
            "{what}: n cells per wall face"
        );
        let counts = s.layer_index.counts();
        assert_eq!(s.layer_index.unreached, 0, "{what}");
        assert_eq!(s.layer_index.max as usize, n, "{what}");
        for (k, &c) in counts.iter().enumerate().skip(1) {
            assert_eq!(c, walls, "{what}: layer {k} has one cell per wall face");
        }
        let layers = s
            .stages
            .iter()
            .find(|x| x.stage == PipelineStage::Layers)
            .expect("layer stage recorded");
        assert!(layers.applied, "{what}");
        let o = s.report.layers.as_ref().expect("outcome");
        assert_eq!(o.strategy, LayerQualityStrategy::CfMeshTetMesh, "{what}");
        assert_eq!(o.full_coverage(), 1.0, "{what}");
        let dv = s.report.total_volume / v_tets - 1.0;
        assert!(dv.abs() < rel_tol, "{what}: volume moved by {dv:.3e}");
        dv
    }

    /// cfMesh's tetMesh layer steps grow every layer on every wall face of
    /// TetGen's tetrahedra of the 1 cm pipe, with no inverted cell (gh:#1824).
    ///
    /// **Methodology.** `pipe_surface(0.01, 0.06, 48)` (patches `inlet`,
    /// `outlet`, `walls`), TetGen `-pq1.414/3.5a2e-8`, tetrahedra only
    /// (`dual: false`), 3 layers on `walls`, first 0.3 mm, ratio 1.2,
    /// [`LayerQualityStrategy::CfMeshTetMesh`]. Asserts the properties of
    /// `assert_layered` (no cell count or fingerprint is pinned: the
    /// pieces under it, TetGen's sizing and the layer port, have no
    /// unit-level V&V of their output against upstream on this case yet).
    ///
    /// **Results (2026-11-26, release, native).** 9874 tetrahedra, 3102
    /// wall faces; 19180 cells (9874 + 3 x 3102), 0 inverted, max
    /// non-orthogonality 77.4°, max skewness 1.08; layer index 3102 cells
    /// in each of layers 1-3; mean column 0.653 mm (1.09 mm asked: the
    /// thickness is upstream's). The volume moved by -2.9e-6 relative
    /// (the untangles move boundary points of the curved wall). About 3 s.
    #[test]
    fn tet_mesh_layers_grow_on_every_wall_face_of_tetgen_tets() {
        let (p, t, regions) = pipe_surface(0.01, 0.06, 48);
        let src = TetSource::tetgen(1.414, Some(2e-8));
        let (tets, _) = surface_to_tet_dual_mesh_multipatch(
            &p,
            &t,
            &regions,
            &tet_opts(src, 0, LayerQualityStrategy::BackOffOnly),
        )
        .unwrap();
        let s = surface_to_tet_dual_mesh_multipatch_staged(
            &p,
            &t,
            &regions,
            &tet_opts(src, 3, LayerQualityStrategy::CfMeshTetMesh),
        )
        .unwrap();
        let dv = assert_layered(
            &s,
            tets.n_cells,
            tets.total_volume(),
            3,
            1e-3,
            "TetGen pipe",
        );
        #[cfg(feature = "std")]
        std::println!(
            "TetGen pipe: {} tets -> {} cells, non-ortho {:.1}, skew {:.2}, dV {dv:.2e}; {:?}",
            tets.n_cells,
            s.mesh.n_cells,
            s.report.max_non_orthogonality_deg,
            s.report.max_skewness,
            s.report.stage_notes
        );
        // The thickness is upstream's (half the local edge, thinned where
        // optimiseLayer thins it), not the requested 1.09 mm total: every
        // column is positive and no thicker than upstream's height.
        let o = s.report.layers.as_ref().unwrap();
        assert!(
            o.points
                .iter()
                .all(|q| q.2 > 0.0 && q.2 <= q.3 * (1.0 + 1e-9)),
            "column thickness within upstream's height"
        );
    }

    /// The route on TetGen's tetrahedra of a unit box, single patch: every
    /// boundary face gets both layers, and the volume is kept to round-off
    /// (every face is planar, so no boundary point leaves its plane).
    ///
    /// **Methodology.** `box_surface` 1 m, TetGen `-pq1.414/3.5a0.01`, the
    /// single-patch entry point, 2 layers, first 0.01 m, ratio 1.2.
    /// **Results (2026-11-26, release):** see the asserts; volume within
    /// 1e-9 of 1 m³.
    #[test]
    fn tet_mesh_layers_on_a_tetgen_box_keep_its_volume() {
        let (p, t) = box_surface(Vec3::ZERO, Vec3::new(1.0, 1.0, 1.0));
        let o = TetDualOptions {
            tet_source: TetSource::tetgen(1.414, Some(0.01)),
            dual: false,
            n_layers: 2,
            first_layer_thickness: 0.01,
            expansion: 1.2,
            layer_strategy: LayerQualityStrategy::CfMeshTetMesh,
            ..Default::default()
        };
        let bare = TetDualOptions {
            n_layers: 0,
            ..o.clone()
        };
        let (tets, _) = surface_to_tet_dual_mesh(&p, &t, &bare).unwrap();
        let (m, rep) = surface_to_tet_dual_mesh(&p, &t, &o).unwrap();
        assert!(acceptable(&m), "{:?}", rep.stage_notes);
        assert_eq!(rep.n_negative_volume_cells, 0);
        let walls = tets.patches.iter().map(|q| q.n_faces).sum::<usize>();
        assert_eq!(m.n_cells, tets.n_cells + 2 * walls, "{:?}", rep.stage_notes);
        assert!(
            (m.total_volume() - 1.0).abs() < 1e-9,
            "{}",
            m.total_volume()
        );
        assert_eq!(rep.layers.as_ref().map(|l| l.full_coverage()), Some(1.0));
    }

    /// No layer asked for: the mesh comes back as given, with a note.
    #[test]
    fn zero_layers_return_the_mesh_unchanged() {
        let (p, t) = box_surface(Vec3::ZERO, Vec3::new(1.0, 1.0, 1.0));
        let m = crate::carve::carve_box(&p, &t, 0.5);
        let (out, o) = add_boundary_layers_cfmesh_tet_mesh(
            &m,
            &["walls"],
            0,
            0.01,
            1.2,
            &BoundaryLayerOptimisationControls::default(),
            None,
        );
        assert_eq!(out.n_cells, m.n_cells);
        assert_eq!(o.full_coverage(), 0.0);
        assert!(o.notes[0].contains("no layer"), "{:?}", o.notes);
    }

    /// **Why no layer grew on TetGen's tetrahedra, and what each of
    /// upstream's steps does there** (gh:#1824). Run with `--ignored
    /// --nocapture`; it prints one line per measurement.
    ///
    /// **Methodology.** The pipe of
    /// [`tet_mesh_layers_grow_on_every_wall_face_of_tetgen_tets`], carved
    /// (h 6 mm) and TetGen tetrahedra, `walls` named. (a) The back-off's
    /// guard: [`meets_layer_quality`] on the bare mesh and at each of its
    /// ten scales. (b) The all-boundary tetrahedra: how many, and whether
    /// the cells step 1 inverts are among them. (c) The steps of the
    /// route alone and together: generation; generation + `optimiseLayer`;
    /// generation + the whole `optimiseFinalMesh` with `optimiseLayer`;
    /// each refined into 3 layers.
    ///
    /// **Results (2026-11-26, release, native).**
    ///
    /// - (a) **The guard that blocked back-off** is `meets_layer_quality`'s
    ///   `minDeterminant 0.001`, checked over the whole mesh: TetGen's bare
    ///   tetrahedra already fail it (min cell determinant 1.75e-5; the
    ///   carved tets 6.4e-2 pass). Scales 1 and 0.6 also invert 7 and 1
    ///   cells; from 0.36 down every candidate is valid with no inverted
    ///   cell, and every one fails on the determinant of cells the layer
    ///   did not make. So no thickness can pass.
    /// - (b) 173 tetrahedra have every vertex on the boundary (146 with a
    ///   wall face, gh:#1796's count). **None** of the 7 cells step 1
    ///   inverts is one; removing them as upstream's `surfaceMorpherCells`
    ///   does (`removeCellsWithAllVerticesAtTheBoundary`) loses 0.66 % of
    ///   the volume and step 1 then inverts 10. The first observation was
    ///   not the cause.
    /// - (c) Generation alone: 3102 layer cells, **7 inverted** (tets
    ///   touching the wall by an edge or face whose other vertices lie
    ///   0.2-0.8 mm from it, inside the 0.5 x edge move); refined, 6.
    ///   With `optimiseLayer`: 16 hairs thinned, **0** inverted, refined
    ///   19180 cells, 0 inverted, non-ortho 77.4°, skewness 1.08, 0.1 s.
    ///   The whole `optimiseFinalMesh` without `optimiseLayer`: 401
    ///   inverted (`optimizeMeshFV` 80 -> 897 bad faces), 196 s. Raising
    ///   TetGen's dihedral bound to 10-18° did not help (6-21 inverted
    ///   after generation). Carved tets: generation inverts 232 cells
    ///   (refined: 0), `optimiseFinalMesh` 0.
    #[test]
    #[ignore]
    fn diagnose_cfmesh_tet_mesh_steps_on_tetgen_tets() {
        use crate::layers::boundary_layers::{generate_layer, BoundaryLayersControls, LayerRequest};
        use crate::topology::{surface_morpher_cells, TopoMesh};
        use crate::pipeline::cartesian_mesh::{optimize_boundary_layer_step, ref_boundary_layers};
        let (p, t, regions) = pipe_surface(0.01, 0.06, 48);
        let controls = UntangleControls {
            boundary_moves: true,
            mesh_untangler: true,
            ..UntangleControls::default()
        };
        let show = |tag: &str, m: &VolumeMesh, t0: crate::time::Instant| {
            let q = check_quality(m);
            std::println!(
                "  {tag}: {} cells, {} inverted, non-ortho {:.1}, skew {:.2}, V {:.9e}, acceptable {}, {:.2} s",
                m.n_cells,
                n_nonpositive_cells(m),
                q.max_non_orthogonality_deg,
                q.max_skewness,
                m.total_volume(),
                acceptable(m),
                t0.elapsed().as_secs_f64()
            );
        };
        let walls = alloc::vec!["walls".to_string()];
        for src in [
            TetSource::CarvedCentroid,
            TetSource::tetgen(1.414, Some(2e-8)),
        ] {
            std::println!("{src:?}");
            let (m, _) = surface_to_tet_dual_mesh_multipatch(
                &p,
                &t,
                &regions,
                &tet_opts(src, 0, LayerQualityStrategy::BackOffOnly),
            )
            .unwrap();
            let named = crate::patches::assign_patches_by_region(&m, &p, &t, &regions).unwrap();
            // (a) the back-off guard
            let q0 = check_quality(&named);
            std::println!(
                "  (a) bare: min det {:.3e}, skew {:.3}, meets the layer limits {}",
                q0.min_cell_determinant,
                q0.max_skewness,
                meets_layer_quality(&q0)
            );
            if let Some(su) = layer_setup(&named, &["walls"], 3, 3e-4, 1.2) {
                let counts: alloc::collections::BTreeMap<usize, usize> =
                    su.wall_pts.iter().map(|&w| (w, 3)).collect();
                let mut scale = 1.0;
                for k in 0..10 {
                    let pp: alloc::collections::BTreeMap<usize, f64> =
                        su.cap.iter().map(|(&w, &c)| (w, c * scale)).collect();
                    let c = build_layered_variable(
                        &named,
                        &su.wall_faces,
                        &su.wall_pts,
                        &su.inward,
                        &su.d,
                        &pp,
                        &counts,
                    );
                    let q = check_quality(&c);
                    std::println!(
                        "      try {k} scale {scale:.4}: {} inverted, min det {:.3e}, meets {}",
                        q.n_negative_volume_cells,
                        q.min_cell_determinant,
                        meets_layer_quality(&q)
                    );
                    scale *= 0.6;
                }
            }
            // (b) all-boundary cells and the cells generation inverts
            let mut onb = alloc::vec![false; named.points.len()];
            for f in 0..named.faces.len() {
                if named.neighbour[f].is_none() {
                    for &v in &named.faces[f] {
                        onb[v] = true;
                    }
                }
            }
            let cf = named.cell_face_lists();
            let all_bnd: Vec<bool> = (0..named.n_cells)
                .map(|c| {
                    cf[c]
                        .iter()
                        .all(|&f| named.faces[f].iter().all(|&v| onb[v]))
                })
                .collect();
            let t0 = crate::time::Instant::now();
            let (g, _) = generate_layer(
                &named,
                &LayerRequest::Patches(walls.clone()),
                &BoundaryLayersControls::default(),
                false,
            )
            .unwrap();
            let (vols, _) = crate::checks::cell_geometry(&g);
            let inv: Vec<usize> = (0..g.n_cells).filter(|&c| vols[c] <= 0.0).collect();
            std::println!(
                "  (b) {} all-boundary cells; inverted by generation: {inv:?}, of them all-boundary {}",
                all_bnd.iter().filter(|&&b| b).count(),
                inv.iter().filter(|&&c| c < named.n_cells && all_bnd[c]).count()
            );
            let mut w = TopoMesh::from_volume_mesh(&named);
            let sm = surface_morpher_cells(&mut w).unwrap();
            let (morphed, _) = w.into_volume_mesh();
            let morphed =
                crate::patches::assign_patches_by_region(&morphed, &p, &t, &regions).unwrap();
            let (gm, _) = generate_layer(
                &morphed,
                &LayerRequest::Patches(walls.clone()),
                &BoundaryLayersControls::default(),
                false,
            )
            .unwrap();
            std::println!(
                "      after surfaceMorpherCells ({} removed, dV {:.3e}): generation inverts {}",
                sm.n_cells_removed,
                morphed.total_volume() / named.total_volume() - 1.0,
                n_nonpositive_cells(&gm)
            );
            // (c) the steps
            show("(c) generated", &g, t0);
            let s = refine_settings(&["walls"], 3, 3e-4, 1.2);
            let ol = BoundaryLayerOptimisationControls::default();
            let mut a = g.clone();
            let t0 = crate::time::Instant::now();
            let line = optimize_boundary_layer_step(&mut a, Some(&ol), false, &controls, "");
            show(&alloc::format!("+ optimiseLayer ({line})"), &a, t0);
            let mut b = g.clone();
            let t0 = crate::time::Instant::now();
            let line = generator_optimise_final_mesh(
                Template::Tet,
                &mut b,
                &p,
                &t,
                &regions,
                false,
                Some(&ol),
                None,
            );
            show(
                &alloc::format!("+ whole optimiseFinalMesh with optimiseLayer ({line:?})"),
                &b,
                t0,
            );
            for (tag, mut m) in [
                ("generated", g),
                ("optimiseLayer", a),
                ("whole optimiseFinalMesh", b),
            ] {
                let t0 = crate::time::Instant::now();
                let r = ref_boundary_layers(&mut m, &s, &controls).map(|x| x.0);
                show(&alloc::format!("refined({tag}) {r:?}"), &m, t0);
            }
        }
    }
}
