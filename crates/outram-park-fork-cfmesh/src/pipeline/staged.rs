// SPDX-License-Identifier: AGPL-3.0-only
//! The tet-dual pipeline **with its stages kept**, and the boundary-layer
//! index of every cell (gh:#1765).
//!
//! [`surface_to_tet_dual_mesh`](super::surface_to_tet_dual_mesh) returns the
//! finished mesh and a report. A reader learning how the mesh was made needs
//! more: the mesh after each stage (carve, snap, tetrahedralise, Delaunay,
//! dual, smooth, feature capture, optimise, layers), and, in the finished
//! mesh, **which cells are boundary-layer cells and which layer each is**.
//! [`surface_to_tet_dual_mesh_staged`] and
//! [`surface_to_tet_dual_mesh_multipatch_staged`] return both.
//!
//! # Observation only
//!
//! The staged routes run the **same** pipeline body as the plain ones
//! (`run_pipeline_observed`); the log receives clones of the meshes the
//! stages already produced and is never read back by the pipeline. The
//! finished mesh is therefore bit-identical to the plain route's, pinned by
//! `tests::staged_route_is_bit_identical_to_the_plain_route` for every layer
//! strategy.
//!
//! # The layer index
//!
//! Every layer strategy builds the original cells first and **appends** the
//! layer cells after them, and the final upper-triangular renumbering keeps
//! the cell labels (it reorders faces only). So the layer cells of the
//! finished mesh are exactly the labels from the cell count before the layer
//! stage onward. Their layer number is their depth from the wall:
//! [`boundary_layer_index`] seeds layer 1 with the layer cells that own a
//! face on a layered patch and walks inward through faces shared with other
//! layer cells (a breadth-first search, so a cell's index is its smallest
//! number of face crossings from the wall plus one). Core cells are 0.
//!
//! **Where the index is exact.** ~~In a uniform column stack each cell's
//! depth is its layer; where a strategy gives neighbouring columns
//! different heights (`CfMeshOptimise`, `SnappyCollapse`) a cell can touch a
//! shallower column sideways and take the smaller depth.~~ **Since
//! gh:#1829** the layers are grown first and every wall face carries every
//! layer, so the prisms' index is exact (one cell per wall face per layer,
//! `tests::uniform_layers_have_one_cell_per_wall_face_per_layer`). On the
//! **dual** of the layered mesh the index is not searched: the pipeline
//! records each dual cell's primal point's place in its column
//! ([`record_layer_index`]), so layer `k` holds one cell per wall point.
//!
//! # Cost
//!
//! Each recorded stage holds a full copy of its mesh, so a staged run keeps
//! roughly one mesh per stage in memory. The plain routes record nothing.
//!
//! **Untrusted AI-assisted draft pending human V&V.** Education, research and
//! V&V only, per the workspace `RESPONSIBLE_USE.md`.

#[cfg(not(any(feature = "std", test)))]
#[allow(unused_imports)]
use crate::nostd_prelude::*;
use alloc::collections::VecDeque;

use crate::math::Vec3;
use crate::patches::SurfaceRegions;
use crate::volume_mesh::VolumeMesh;

use super::{run_pipeline_observed, TetDualOptions, TetDualReport};

/// One step of the tet-dual pipeline, in the order the pipeline runs them.
///
/// A stage appears in [`StagedMesh::stages`] only when the options ask for it;
/// [`StageSnapshot::applied`] says whether its result was kept (the pipeline
/// skips a stage that would leave an invalid mesh, and says so in the report's
/// stage notes).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum PipelineStage {
    /// The Cartesian (or octree-graded) carve of the surface's bounding box:
    /// the cells inside the surface, as hexahedra. Always runs.
    Carve,
    /// Boundary points moved onto the surface.
    Snap,
    /// Every cell split into tetrahedra.
    Tetrahedralise,
    /// Edge flips toward a Delaunay tetrahedralisation.
    Delaunay,
    /// Prism boundary layers grown on the wall patches. ~~After the dual
    /// and the optimiser~~ **MOVED 2026-11-26 (gh:#1829):** grown first,
    /// from TetGen's wall triangulation, with TetGen's tetrahedra of the
    /// inner region; the snapshot is the prisms and tetrahedra together.
    Layers,
    /// The polyhedral dual of the tetrahedra: one polyhedral cell per
    /// tetrahedron vertex.
    Dual,
    /// Smart-Laplacian smoothing.
    Smooth,
    /// The boundary mapped onto the surface patches and their feature edges.
    FeatureCapture,
    /// cfMesh's `optimiseFinalMesh`.
    OptimiseFinalMesh,
    /// Boundary faces named after the surface's regions.
    NamePatches,
    /// The finished mesh, in OpenFOAM's upper-triangular face order. Always
    /// the last entry, and equal to [`StagedMesh::mesh`].
    Final,
}

impl PipelineStage {
    /// A short name for the stage, for a stepper or a table.
    #[must_use]
    pub fn label(self) -> &'static str {
        match self {
            Self::Carve => "carve",
            Self::Snap => "snap",
            Self::Tetrahedralise => "tetrahedralise",
            Self::Delaunay => "Delaunay flips",
            Self::Dual => "polyhedral dual",
            Self::Smooth => "smooth",
            Self::FeatureCapture => "feature capture",
            Self::OptimiseFinalMesh => "optimise",
            Self::Layers => "boundary layers",
            Self::NamePatches => "name patches",
            Self::Final => "final",
        }
    }
}

/// The mesh as one stage left it.
#[derive(Debug, Clone)]
pub struct StageSnapshot {
    /// Which stage.
    pub stage: PipelineStage,
    /// `true` when the stage's result was kept; `false` when the pipeline
    /// skipped it to keep the mesh valid, in which case [`Self::mesh`] is the
    /// mesh the stage was given.
    pub applied: bool,
    /// The mesh after the stage.
    pub mesh: VolumeMesh,
}

/// What a staged run collects while the pipeline runs. Filled by the
/// pipeline through [`record`] and [`record_layers`]; read through
/// [`StagedMesh`].
#[derive(Debug, Clone, Default)]
pub struct StageLog {
    stages: Vec<StageSnapshot>,
    /// Cell count before the layer stage, when the stage added cells.
    first_layer_cell: Option<usize>,
    /// The patches the layers grew on.
    layer_patches: Vec<String>,
    /// Every cell's layer, when the pipeline knows it outright (gh:#1829:
    /// the dual of a layered mesh, whose cells are its points).
    per_cell: Option<Vec<u32>>,
}

/// Record a stage, if a log is being kept.
pub(crate) fn record(
    log: &mut Option<&mut StageLog>,
    stage: PipelineStage,
    applied: bool,
    mesh: &VolumeMesh,
) {
    if let Some(l) = log.as_deref_mut() {
        l.stages.push(StageSnapshot {
            stage,
            applied,
            mesh: mesh.clone(),
        });
    }
}

/// Record the layer stage: as [`record`], plus where the layer cells start
/// (`before` = the cell count the stage was given) and which patches they
/// grew on. A stage that was skipped, or kept but added no cell, leaves no
/// layer cells.
pub(crate) fn record_layers(
    log: &mut Option<&mut StageLog>,
    kept: bool,
    before: usize,
    patches: &[&str],
    mesh: &VolumeMesh,
) {
    if let Some(l) = log.as_deref_mut() {
        let grew = kept && mesh.cell_count() > before;
        l.first_layer_cell = if grew { Some(before) } else { None };
        l.layer_patches = patches.iter().map(|p| (*p).to_string()).collect();
        l.stages.push(StageSnapshot {
            stage: PipelineStage::Layers,
            applied: grew,
            mesh: mesh.clone(),
        });
    }
}

/// Record every cell's layer outright (gh:#1829): the dual of a mesh whose
/// layers were grown first has one cell per primal point, and a cell's
/// layer is its point's place in its column. Overrides the search from
/// [`record_layers`].
pub(crate) fn record_layer_index(log: &mut Option<&mut StageLog>, per_cell: Vec<u32>) {
    if let Some(l) = log.as_deref_mut() {
        l.per_cell = Some(per_cell);
    }
}

/// The boundary-layer index of every cell of a mesh: `0` for a core cell,
/// `k` for a cell in the k-th layer from the wall. See the module docs for
/// how it is found and where it is exact.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LayerIndex {
    /// One entry per cell.
    pub per_cell: Vec<u32>,
    /// The deepest layer found (0 when there are no layer cells).
    pub max: u32,
    /// Layer cells the search did not reach from a layered wall face. Their
    /// entry in [`Self::per_cell`] is 0. Expected to be 0; a nonzero count
    /// means the layer cells are not connected to the wall as assumed.
    pub unreached: usize,
}

impl LayerIndex {
    /// No layer cells: every cell is core.
    #[must_use]
    pub fn none(n_cells: usize) -> Self {
        LayerIndex {
            per_cell: vec![0; n_cells],
            max: 0,
            unreached: 0,
        }
    }

    /// How many cells are in each layer: entry `k` counts the cells with
    /// index `k` (entry 0 is the core).
    #[must_use]
    pub fn counts(&self) -> Vec<usize> {
        let mut c = vec![0usize; self.max as usize + 1];
        for &i in &self.per_cell {
            c[i as usize] += 1;
        }
        c
    }
}

/// The layer index of `mesh`, whose layer cells are the labels from
/// `first_layer_cell` on and whose layers grew on `layer_patches`.
///
/// Layer 1 is every layer cell owning a face on one of `layer_patches`; each
/// further layer is the layer cells sharing a face with the previous one and
/// not yet numbered. See the module docs.
#[must_use]
pub fn boundary_layer_index(
    mesh: &VolumeMesh,
    first_layer_cell: usize,
    layer_patches: &[&str],
) -> LayerIndex {
    let n = mesh.n_cells;
    if first_layer_cell >= n {
        return LayerIndex::none(n);
    }
    let is_layer = |c: usize| c >= first_layer_cell;

    // Face adjacency among layer cells only.
    let mut adj: Vec<Vec<usize>> = vec![Vec::new(); n - first_layer_cell];
    for (f, nb) in mesh.neighbour.iter().enumerate() {
        if let Some(b) = *nb {
            let a = mesh.owner[f];
            if is_layer(a) && is_layer(b) {
                adj[a - first_layer_cell].push(b);
                adj[b - first_layer_cell].push(a);
            }
        }
    }

    let mut idx = vec![0u32; n];
    let mut queue: VecDeque<usize> = VecDeque::new();
    for p in mesh
        .patches
        .iter()
        .filter(|p| layer_patches.contains(&p.name.as_str()))
    {
        for f in p.start_face..p.start_face + p.n_faces {
            let c = mesh.owner[f];
            if is_layer(c) && idx[c] == 0 {
                idx[c] = 1;
                queue.push_back(c);
            }
        }
    }
    let mut max = 0u32;
    while let Some(c) = queue.pop_front() {
        max = max.max(idx[c]);
        for &d in &adj[c - first_layer_cell] {
            if idx[d] == 0 {
                idx[d] = idx[c] + 1;
                queue.push_back(d);
            }
        }
    }
    let unreached = (first_layer_cell..n).filter(|&c| idx[c] == 0).count();
    LayerIndex {
        per_cell: idx,
        max,
        unreached,
    }
}

/// A finished tet-dual mesh with its stages and its layer index.
#[derive(Debug, Clone)]
pub struct StagedMesh {
    /// The finished mesh: bit-identical to the plain route's.
    pub mesh: VolumeMesh,
    /// The plain route's report, unchanged.
    pub report: TetDualReport,
    /// The mesh after each stage that ran, in pipeline order;
    /// [`PipelineStage::Final`] is last.
    pub stages: Vec<StageSnapshot>,
    /// The boundary-layer index of every cell of [`Self::mesh`].
    pub layer_index: LayerIndex,
    /// The label of the first layer cell of [`Self::mesh`], or `None` when
    /// the layer stage added no cell, or (gh:#1829) when the mesh is the
    /// dual of a layered mesh, whose layer cells are not a contiguous range
    /// ([`Self::layer_index`] has them).
    pub first_layer_cell: Option<usize>,
}

fn finish(mesh: VolumeMesh, report: TetDualReport, log: StageLog) -> StagedMesh {
    let contiguous = log.per_cell.is_none();
    let layer_index = match (log.per_cell, log.first_layer_cell) {
        (Some(per_cell), _) if per_cell.len() == mesh.n_cells => {
            let max = per_cell.iter().copied().max().unwrap_or(0);
            LayerIndex { per_cell, max, unreached: 0 }
        }
        (_, Some(first)) => {
            let names: Vec<&str> = log.layer_patches.iter().map(String::as_str).collect();
            boundary_layer_index(&mesh, first, &names)
        }
        _ => LayerIndex::none(mesh.n_cells),
    };
    StagedMesh {
        mesh,
        report,
        stages: log.stages,
        layer_index,
        first_layer_cell: log.first_layer_cell.filter(|_| contiguous),
    }
}

/// [`surface_to_tet_dual_mesh`](super::surface_to_tet_dual_mesh), keeping
/// every stage's mesh and numbering the layer cells.
///
/// # Errors
/// As the plain route.
pub fn surface_to_tet_dual_mesh_staged(
    points: &[Vec3],
    tris: &[[usize; 3]],
    opts: &TetDualOptions,
) -> Result<StagedMesh, String> {
    let mut log = StageLog::default();
    let (mesh, report) = run_pipeline_observed(points, tris, opts, None, true, Some(&mut log))?;
    Ok(finish(mesh, report, log))
}

/// [`surface_to_tet_dual_mesh_multipatch`](super::surface_to_tet_dual_mesh_multipatch),
/// keeping every stage's mesh and numbering the layer cells.
///
/// # Errors
/// As the plain route.
pub fn surface_to_tet_dual_mesh_multipatch_staged(
    points: &[Vec3],
    tris: &[[usize; 3]],
    regions: &SurfaceRegions,
    opts: &TetDualOptions,
) -> Result<StagedMesh, String> {
    regions.validate(tris.len())?;
    let mut log = StageLog::default();
    let (mesh, report) =
        run_pipeline_observed(points, tris, opts, Some(regions), true, Some(&mut log))?;
    Ok(finish(mesh, report, log))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::pipeline::tests::{mesh_fingerprint, pipe_surface};
    use crate::pipeline::{surface_to_tet_dual_mesh, surface_to_tet_dual_mesh_multipatch};
    use crate::shapes::box_surface;

    /// The report with the wall-clock durations in its notes (the stage notes
    /// and the layer outcome's) blanked
    /// (`"3.6 s smoothing"` becomes `"# s smoothing"`). The optimiser notes
    /// time themselves, so two runs of the *plain* route differ there too;
    /// every other field, and every other word, is compared.
    fn without_timings(r: &TetDualReport) -> TetDualReport {
        let mut r = r.clone();
        for note in &mut r.stage_notes {
            *note = blank_seconds(note);
        }
        if let Some(layers) = r.layers.as_mut() {
            for note in &mut layers.notes {
                *note = blank_seconds(note);
            }
        }
        r
    }

    /// `"took 3.6 s here"` -> `"took # s here"`: every number followed by the
    /// word `s` (seconds) is blanked.
    fn blank_seconds(note: &str) -> String {
        let c: Vec<char> = note.chars().collect();
        let mut out = String::with_capacity(note.len());
        let numeric = |ch: char| ch.is_ascii_digit() || ch == '.';
        let mut i = 0;
        while i < c.len() {
            if numeric(c[i]) && (i == 0 || !c[i - 1].is_alphanumeric()) {
                let start = i;
                while i < c.len() && numeric(c[i]) {
                    i += 1;
                }
                let seconds = c.get(i) == Some(&' ')
                    && c.get(i + 1) == Some(&'s')
                    && c.get(i + 2).map_or(true, |ch| !ch.is_alphanumeric());
                if seconds {
                    out.push('#');
                } else {
                    out.extend(&c[start..i]);
                }
            } else {
                out.push(c[i]);
                i += 1;
            }
        }
        out
    }

    /// The 1 cm pipe's options (h 6 mm, TetGen bounded by the regular
    /// tetrahedron of h), `n_layers` layers from 0.3 mm at 1.2 on `walls`,
    /// the cell-centre dual when `dual`.
    fn pipe_opts(dual: bool, n_layers: usize) -> TetDualOptions {
        TetDualOptions {
            cell_size: 0.006,
            n_layers,
            first_layer_thickness: 3e-4,
            expansion: 1.2,
            layer_patches: vec!["walls".into()],
            dual,
            dual_kind: crate::dual::DualKind::CellCentre,
            dual_feature_points: crate::dual::FeaturePointRule::Corners,
            ..Default::default()
        }
    }

    /// The staged routes only observe: same mesh, same report.
    ///
    /// **Methodology.** Mesh the same input through the plain route and the
    /// staged one and compare `mesh_fingerprint` (every point's bits, every
    /// face, owner, neighbour and patch — the hash the crate's own pins use)
    /// and the report. Cases: the default options on a box (single-patch
    /// route, layers first, median dual) and a 1 cm pipe with 3 layers grown
    /// first, as prisms and tetrahedra and as their cell-centre dual
    /// (multipatch route). ~~Under each of the three layer strategies~~
    /// (gh:#1829: there is one layer route).
    #[test]
    fn staged_route_is_bit_identical_to_the_plain_route() {
        let (bp, bt) = box_surface(Vec3::ZERO, Vec3::new(1.0, 1.0, 1.0));
        let o = TetDualOptions {
            cell_size: 0.25,
            ..Default::default()
        };
        let (plain, rep) = surface_to_tet_dual_mesh(&bp, &bt, &o).unwrap();
        let staged = surface_to_tet_dual_mesh_staged(&bp, &bt, &o).unwrap();
        assert_eq!(
            mesh_fingerprint(&plain),
            mesh_fingerprint(&staged.mesh),
            "box"
        );
        assert_eq!(
            without_timings(&rep),
            without_timings(&staged.report),
            "box report"
        );

        let (p, t, regions) = pipe_surface(0.01, 0.06, 48);
        for dual in [false, true] {
            let o = pipe_opts(dual, 3);
            let (plain, rep) = surface_to_tet_dual_mesh_multipatch(&p, &t, &regions, &o).unwrap();
            let staged = surface_to_tet_dual_mesh_multipatch_staged(&p, &t, &regions, &o).unwrap();
            assert_eq!(
                mesh_fingerprint(&plain),
                mesh_fingerprint(&staged.mesh),
                "pipe dual={dual}"
            );
            assert_eq!(
                without_timings(&rep),
                without_timings(&staged.report),
                "pipe dual={dual} report"
            );
        }
    }

    /// The stages come back in pipeline order, end with `Final`, and `Final`
    /// is the returned mesh.
    #[test]
    fn stages_are_in_order_and_end_with_the_returned_mesh() {
        let (p, t, regions) = pipe_surface(0.01, 0.06, 48);
        let staged = surface_to_tet_dual_mesh_multipatch_staged(
            &p,
            &t,
            &regions,
            &pipe_opts(true, 3),
        )
        .unwrap();
        let order: Vec<PipelineStage> = staged.stages.iter().map(|s| s.stage).collect();
        assert_eq!(order.first(), Some(&PipelineStage::Carve));
        assert_eq!(order.last(), Some(&PipelineStage::Final));
        assert!(
            order.contains(&PipelineStage::Dual) && order.contains(&PipelineStage::Layers),
            "{order:?}"
        );
        let rank = |s: PipelineStage| s as usize;
        assert!(
            order.windows(2).all(|w| rank(w[0]) < rank(w[1])),
            "out of pipeline order: {order:?}"
        );
        let last = &staged.stages.last().unwrap().mesh;
        assert_eq!(mesh_fingerprint(last), mesh_fingerprint(&staged.mesh));
    }

    /// Layers grown first: one cell per wall face per layer, and on their
    /// dual one cell per wall point per layer.
    ///
    /// **Methodology.** Layers-first (gh:#1829) gives every wall face all
    /// `n` prisms. On the prisms and tetrahedra the index must put exactly
    /// one cell per wall face in each of layers 1..=n, reach every layer
    /// cell, and count the tetrahedra as core. On the cell-centre dual a
    /// cell is its primal point's, so layer `k` holds one cell per wall
    /// point (the `k-1`-th point of each column); the wall points are the
    /// layer stage's own record ([`crate::layers::LayerOutcome::points`]).
    #[test]
    fn uniform_layers_have_one_cell_per_wall_face_per_layer() {
        let (p, t, regions) = pipe_surface(0.01, 0.06, 48);
        for n in [1usize, 2, 3] {
            let staged = surface_to_tet_dual_mesh_multipatch_staged(
                &p,
                &t,
                &regions,
                &pipe_opts(false, n),
            )
            .unwrap();
            let m = &staged.mesh;
            let first = staged.first_layer_cell.expect("layers grew");
            let wall_faces: usize = m
                .patches
                .iter()
                .filter(|p| p.name == "walls")
                .map(|p| p.n_faces)
                .sum();
            assert_eq!(m.n_cells - first, n * wall_faces, "n {n}: layer cells");
            let li = &staged.layer_index;
            assert_eq!(li.unreached, 0, "n {n}");
            assert_eq!(li.max as usize, n, "n {n}");
            let counts = li.counts();
            assert_eq!(counts[0], first, "n {n}: core");
            for k in 1..=n {
                assert_eq!(counts[k], wall_faces, "n {n}: layer {k}");
            }
            let dual = surface_to_tet_dual_mesh_multipatch_staged(&p, &t, &regions, &pipe_opts(true, n)).unwrap();
            assert!(dual.first_layer_cell.is_none(), "n {n}: the dual's layer cells are not a range");
            let wall_points = dual.report.layers.as_ref().unwrap().points.len();
            let dc = dual.layer_index.counts();
            assert_eq!(dc.len(), n + 1, "n {n}");
            for k in 1..=n {
                assert_eq!(dc[k], wall_points, "n {n}: dual layer {k}");
            }
        }
    }

    /// With no layers asked for there is no layer stage and no layer cell.
    #[test]
    fn no_layers_means_an_all_core_index() {
        let (p, t, regions) = pipe_surface(0.01, 0.06, 48);
        let staged = surface_to_tet_dual_mesh_multipatch_staged(
            &p,
            &t,
            &regions,
            &pipe_opts(true, 0),
        )
        .unwrap();
        assert!(staged.first_layer_cell.is_none());
        assert!(staged
            .stages
            .iter()
            .all(|s| s.stage != PipelineStage::Layers));
        assert_eq!(staged.layer_index, LayerIndex::none(staged.mesh.n_cells));
    }
}
