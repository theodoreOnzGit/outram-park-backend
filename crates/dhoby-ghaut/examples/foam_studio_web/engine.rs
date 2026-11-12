//! Mesh and solve, off the UI thread: a background thread natively, the Web
//! Worker in the browser (`web/foam_studio/worker.js`). Every request is
//! short (a mesh, or one chunk of time steps), so a new design or a stop is
//! served within one chunk.
//!
//! **The physics is not here.** Meshing is `outram_foam_mesh::block_primitives`
//! (the pipe O-grid and the box block, through the blockMesh port), or for
//! the cylinder's unstructured choice the cfMesh port through
//! `outram_blender::foam_mesh` ([`mesh_unstructured`], gh:#1202); the
//! solver is `outram_foam_appbuilder_lib`'s `PimpleFoam` with the boundary
//! conditions and diagnostics of `cases::laminar_pipe`, unchanged. V&V record:
//! `crates/outram-foam-appbuilder-lib/verification_and_validation/laminar_pipe_gh1155.md`.
//! This file chooses resolutions, steps in chunks and cuts the sections the
//! page draws.
use std::sync::Arc;

use outram_foam_appbuilder_lib::cases::double_pipe::{self, DoublePipeExchanger, ExchangerReport};
use outram_foam_appbuilder_lib::cases::laminar_pipe::{max_continuity_error, patch_flux, PipeFlow};
use outram_foam_appbuilder_lib::solvers::cht_multi_region_foam::ChtMultiRegionFoam;
use outram_foam_appbuilder_lib::solvers::pimple_foam::PimpleFoam;
use outram_foam_basic_lib::fv_operators::fvm::max_non_orthogonality_deg;
use outram_foam_basic_lib::mesh::FvMesh;
use outram_foam_mesh::block_mesh::PolyMesh;
use outram_foam_mesh::block_primitives::{BoxBlock, DoublePipe, HelicalPipe, PipeOGrid};
use crate::coil::{CoilRun, CoilSpec};
use outram_foam_mesh::section::{boundary_edges, plane_section, Axis, CellSection};
use uom::si::f64::{KinematicViscosity, Length, Velocity};
use uom::si::kinematic_viscosity::square_meter_per_second;
use uom::si::length::meter;
use uom::si::velocity::meter_per_second;

/// Default cell count: interactive (maintainer decision, 2026-11-03).
pub const DEFAULT_CELLS: usize = 20_000;
/// Largest cell count offered; above [`DEFAULT_CELLS`] it is a batch run.
pub const MAX_CELLS: usize = 50_000;
/// Inlet (bulk) velocity \[m/s\]; Re is set through ν.
pub const U_IN: f64 = 5.0e-3;
/// Largest viscous number ν Δt / h² a run uses (h the mean cell size
/// (V/N)^(1/3)): a guard for low Re, where `PimpleFoam` diverges (stage 0,
/// gh:#1162). 0.5 is where both measured-stable 20k pipe runs sit (Re = 100
/// at Co ≈ 4 and Re = 25 at Co ≈ 1: 0.48 each, gh:#1161 record); with 2,
/// Re = 10 diverged on the 20k pipe (measured 2026-11-03).
/// `FOAM_MAX_VISCOUS` (native only) overrides it for experiments.
pub const MAX_VISCOUS_DEFAULT: f64 = 0.5;

/// The steady criterion: max cell |ΔU| / U per flow-through time (stage 0).
pub const STEADY: f64 = 1.0e-3;

/// What a primitive face becomes.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Tag {
    Inlet,
    Outlet,
    Wall,
}

impl Tag {
    pub fn name(self) -> &'static str {
        match self {
            Tag::Inlet => "inlet",
            Tag::Outlet => "outlet",
            Tag::Wall => "walls",
        }
    }
    fn code(self) -> f64 {
        match self {
            Tag::Inlet => 0.0,
            Tag::Outlet => 1.0,
            Tag::Wall => 2.0,
        }
    }
    fn from_code(c: f64) -> Self {
        match c as u8 {
            0 => Tag::Inlet,
            1 => Tag::Outlet,
            _ => Tag::Wall,
        }
    }
}

/// The primitive, in metres, positioned by its centre.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Shape {
    /// A cylinder along z. Faces: z-min end, z-max end, side.
    Cylinder { radius: f64, length: f64, centre: [f64; 3] },
    /// An axis-aligned box. Faces: x-, x+, y-, y+, z-, z+.
    Box { size: [f64; 3], centre: [f64; 3] },
    /// A counterflow double-pipe heat exchanger along z (gh:#1156): hot
    /// fluid in the tube (radius `r_tube`, in at z−), a conducting tube wall
    /// of thickness `wall`, cold fluid in the annulus out to `r_shell` (in at
    /// z+), adiabatic shell. Its patches are fixed, not tagged.
    DoublePipe { r_tube: f64, wall: f64, r_shell: f64, length: f64, centre: [f64; 3] },
    /// A helical coil section (gh:#1157): a pipe of radius `tube_radius`
    /// coiled about a vertical (z) axis through `centre`, coil diameter
    /// `coil_diameter` at the tube centreline, rise `pitch` per turn,
    /// `turns` turns, centred on `centre`; inlet at the bottom end. Its
    /// patches are fixed (inlet, outlet, walls).
    Coil { tube_radius: f64, coil_diameter: f64, pitch: f64, turns: f64, centre: [f64; 3] },
}

impl Shape {
    /// Number of taggable faces.
    pub fn n_faces(&self) -> usize {
        match self {
            Shape::Cylinder { .. } => 3,
            Shape::Box { .. } => 6,
            Shape::DoublePipe { .. } | Shape::Coil { .. } => 0,
        }
    }
    /// Human names of the faces.
    pub fn face_names(&self) -> &'static [&'static str] {
        match self {
            Shape::Cylinder { .. } => &["end z−", "end z+", "side"],
            Shape::Box { .. } => &["x−", "x+", "y−", "y+", "z−", "z+"],
            Shape::DoublePipe { .. } | Shape::Coil { .. } => &[],
        }
    }
    /// Bounding box `[min, max]` \[m\].
    pub fn bounds(&self) -> [[f64; 3]; 2] {
        match *self {
            Shape::Cylinder { radius, length, centre: c } | Shape::DoublePipe { r_shell: radius, length, centre: c, .. } => {
                [[c[0] - radius, c[1] - radius, c[2] - 0.5 * length], [c[0] + radius, c[1] + radius, c[2] + 0.5 * length]]
            }
            Shape::Box { size: s, centre: c } => {
                [[c[0] - 0.5 * s[0], c[1] - 0.5 * s[1], c[2] - 0.5 * s[2]], [c[0] + 0.5 * s[0], c[1] + 0.5 * s[1], c[2] + 0.5 * s[2]]]
            }
            Shape::Coil { tube_radius: r, coil_diameter, pitch, turns, centre: c } => {
                let (a, h) = (0.5 * coil_diameter + r, 0.5 * pitch * turns + r);
                [[c[0] - a, c[1] - a, c[2] - h], [c[0] + a, c[1] + a, c[2] + h]]
            }
        }
    }
}

/// A design: the shape, a tag per face (only the first `n_faces` count), the
/// cell budget, and (for the cylinder) which mesher (gh:#1202).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Design {
    pub shape: Shape,
    pub tags: [Tag; 6],
    pub target_cells: usize,
    pub mesh: PipeMesh,
}

/// The cylinder's mesh (gh:#1202). Other shapes ignore it.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum PipeMesh {
    /// The O-grid of blockMesh (`PipeOGrid`), sized by the cell budget: the
    /// default, and the mesh every gh:#1155 number was recorded on.
    Structured,
    /// cfMesh's tet → polyhedral dual → wall prism layers, from
    /// `outram_blender`'s cylinder primitive (`foam_mesh::mesh_to_tet_dual_multipatch`).
    Unstructured(TetDualSpec),
}

/// The unstructured mesher's controls, lengths in metres. Clamped by
/// [`TetDualSpec::clamped`] before use, so a phone cannot ask for a mesh
/// that would take minutes.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct TetDualSpec {
    /// Background (Cartesian carve) cell size.
    pub cell_size: f64,
    /// Prism layers on the wall (0 for none).
    pub n_layers: usize,
    /// First (wall) layer thickness.
    pub first_layer: f64,
    /// Layer-to-layer growth.
    pub expansion: f64,
    /// How the polyhedral dual builds its wall faces (gh:#1240): one per
    /// boundary-triangle corner (the default), or upstream `polyDualMesh`'s
    /// one per point per patch, as a median or a cell-centre dual.
    pub dual: outram_park_fork_cfmesh::dual::DualKind,
    /// Which boundary points the two `polyDualMesh` duals keep (gh:#1240):
    /// the utility's rule (every point on a feature edge, the default) or
    /// the class's (corners only). Ignored by the per-corner dual.
    pub feature_points: outram_park_fork_cfmesh::dual::FeaturePointRule,
    /// What the layer stage does about cells that fail the quality checks
    /// (gh:#1215): back off the thickness (~~the default~~ the default
    /// before gh:#1298), cfMesh's layer optimiser (the default since
    /// gh:#1298), or snappyHexMesh's local collapse.
    pub layer_quality: outram_blender::foam_mesh::LayerQualityStrategy,
    /// The cfMesh steps around the dual (gh:#1298): upstream's
    /// `tetMeshGenerator` sequence without the Laplace pass (the default),
    /// or the route before gh:#1298 (the named ablation).
    pub route: CfMeshRoute,
}

/// Which cfMesh steps run around the dual (gh:#1298). The record is the
/// gh:#1283 and gh:#1298 sections of
/// `crates/outram-foam-appbuilder-lib/verification_and_validation/laminar_pipe_gh1155.md`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CfMeshRoute {
    /// **The default since gh:#1298** (gh:#1283's route `upEnl`): cfMesh's
    /// `meshSurfaceOptimizer` in the feature-edge capture
    /// (`TetDualOptions::untangle_surface`), then `optimiseFinalMesh`
    /// (`TetDualOptions::optimise_final_mesh`: `optimizeSurface`,
    /// `untangleMeshFV` with the boundary moves and the LP untangler,
    /// `optimizeLowQualityFaces`, `untangleMeshFV`), with the points the
    /// boundary passes move mapped back onto the surface after each pass
    /// (`BoundaryProjection::EachPass`, **not upstream**: upstream lets
    /// them leave the surface). `optimizeMeshFV`'s Laplace pass is off: it
    /// thickens the layer stack by about 50 % (gh:#1256). With layers and
    /// `CfMeshOptimise` the layer stage runs the same optimiser.
    UpstreamNoLaplace,
    /// The route before gh:#1298, kept as the explicit ablation: the
    /// `revert_tangled` feature-edge capture, no `optimiseFinalMesh`, and
    /// the cfMesh optimiser's own defaults (no boundary moves, no LP
    /// untangler, no projection).
    PreGh1298,
}

impl TetDualSpec {
    /// The mesher choices every spec starts from. Literal specs end with
    /// `..TetDualSpec::BASE`; only the four numbers above are meaningful
    /// in it. ~~Its numbers are h 4 mm, no layer~~ **CHANGED 2026-11-05
    /// (gh:#1298, maintainer decision):** its numbers are the demo default's
    /// on the demo pipe ([`Self::default_for`]`(0.01)`): h 5 mm, 3 layers,
    /// first layer 0.3 mm, × 1.2.
    ///
    /// ~~The per-corner dual, the utility's feature points and the back-off
    /// layer stage, i.e. the mesher every recorded number before gh:#1240's
    /// browser controls was taken on.~~ **CHANGED 2026-11-05 (gh:#1298,
    /// maintainer-approved):** the cell-centre dual (`polyDualMesh`) with
    /// the class's feature points (corners only), cfMesh's layer optimiser,
    /// and [`CfMeshRoute::UpstreamNoLaplace`]. The old choices are
    /// [`Self::PRE_GH1298`], the named ablation.
    pub const BASE: Self = Self {
        cell_size: 0.005,
        n_layers: 3,
        first_layer: 3e-4,
        expansion: 1.2,
        dual: outram_park_fork_cfmesh::dual::DualKind::CellCentre,
        feature_points: outram_park_fork_cfmesh::dual::FeaturePointRule::Corners,
        layer_quality: outram_blender::foam_mesh::LayerQualityStrategy::CfMeshOptimise,
        route: CfMeshRoute::UpstreamNoLaplace,
    };

    /// **The ablation of gh:#1298**: the mesher choices of [`Self::BASE`]
    /// before 2026-11-05, i.e. the per-corner median dual, the utility's
    /// feature points, the back-off layer stage and
    /// [`CfMeshRoute::PreGh1298`]. Every Foam Studio number recorded before
    /// gh:#1298 was taken on it; specs that reproduce those records end
    /// with `..TetDualSpec::PRE_GH1298`. Its numbers stay the old
    /// default's (h 4 mm, no layer) now that [`Self::BASE`]'s are h 5 mm
    /// with 3 layers.
    pub const PRE_GH1298: Self = Self {
        cell_size: 0.004,
        n_layers: 0,
        dual: outram_park_fork_cfmesh::dual::DualKind::MedianPerCorner,
        feature_points: outram_park_fork_cfmesh::dual::FeaturePointRule::OnFeatureEdge,
        layer_quality: outram_blender::foam_mesh::LayerQualityStrategy::BackOffOnly,
        route: CfMeshRoute::PreGh1298,
        ..Self::BASE
    };

    /// The four mesher choices as the small integers the worker messages
    /// carry: `[dual, feature points, layer quality, route]` (the route
    /// since gh:#1298: 0 upstream without Laplace, 1 the pre-gh:#1298
    /// route).
    pub fn choice_codes(&self) -> [f64; 4] {
        use outram_blender::foam_mesh::LayerQualityStrategy as L;
        use outram_park_fork_cfmesh::dual::{DualKind as D, FeaturePointRule as F};
        let d = match self.dual {
            D::MedianPerCorner => 0.0,
            D::MedianPerPoint => 1.0,
            D::CellCentre => 2.0,
        };
        let f = match self.feature_points {
            F::OnFeatureEdge => 0.0,
            F::Corners => 1.0,
        };
        let l = match self.layer_quality {
            L::BackOffOnly => 0.0,
            L::CfMeshOptimise => 1.0,
            L::SnappyCollapse => 2.0,
        };
        let r = match self.route {
            CfMeshRoute::UpstreamNoLaplace => 0.0,
            CfMeshRoute::PreGh1298 => 1.0,
        };
        [d, f, l, r]
    }

    /// `self` with the choices [`Self::choice_codes`] wrote; an unknown code
    /// is an error, not a silent default.
    pub fn with_choice_codes(self, c: [f64; 4]) -> Result<Self, String> {
        use outram_blender::foam_mesh::LayerQualityStrategy as L;
        use outram_park_fork_cfmesh::dual::{DualKind as D, FeaturePointRule as F};
        let dual = match c[0] as u8 {
            0 => D::MedianPerCorner,
            1 => D::MedianPerPoint,
            2 => D::CellCentre,
            k => return Err(format!("unknown dual kind code {k}")),
        };
        let feature_points = match c[1] as u8 {
            0 => F::OnFeatureEdge,
            1 => F::Corners,
            k => return Err(format!("unknown feature-point rule code {k}")),
        };
        let layer_quality = match c[2] as u8 {
            0 => L::BackOffOnly,
            1 => L::CfMeshOptimise,
            2 => L::SnappyCollapse,
            k => return Err(format!("unknown layer strategy code {k}")),
        };
        let route = match c[3] as u8 {
            0 => CfMeshRoute::UpstreamNoLaplace,
            1 => CfMeshRoute::PreGh1298,
            k => return Err(format!("unknown cfMesh route code {k}")),
        };
        Ok(Self { dual, feature_points, layer_quality, route, ..self })
    }

    /// `self` with the cell size and layer count a page URL names
    /// (gh:#1202): `h=` in mm, `layers=` (at most [`MAX_LAYERS`]); a value
    /// that does not parse leaves the control as it is. `h=4&layers=0` is
    /// the bare h 4 mm case, the default until the maintainer's gh:#1298
    /// decision.
    pub fn with_size_query(self, h_mm: Option<&str>, layers: Option<&str>) -> Self {
        let mut s = self;
        if let Some(h) = h_mm.and_then(|v| v.parse::<f64>().ok()) {
            s.cell_size = h * 1e-3;
        }
        if let Some(n) = layers.and_then(|v| v.parse::<usize>().ok()) {
            s.n_layers = n.min(MAX_LAYERS);
        }
        s
    }

    /// `self` with the route a page URL names (gh:#1298): `route=legacy`
    /// is [`CfMeshRoute::PreGh1298`], `route=upstream` the default;
    /// anything else leaves it as it is.
    pub fn with_route_query(self, route: Option<&str>) -> Self {
        match route {
            Some("legacy") => Self { route: CfMeshRoute::PreGh1298, ..self },
            Some("upstream") => Self { route: CfMeshRoute::UpstreamNoLaplace, ..self },
            _ => self,
        }
    }

    /// `self` with the choices a page URL names (gh:#1240 browser controls):
    /// `dual=point|centre`, `fp=corners`, `layerq=cfmesh|snappy`; anything
    /// else leaves a choice as it is.
    pub fn with_query(self, dual: Option<&str>, fp: Option<&str>, layerq: Option<&str>) -> Self {
        use outram_blender::foam_mesh::LayerQualityStrategy as L;
        use outram_park_fork_cfmesh::dual::{DualKind as D, FeaturePointRule as F};
        let mut s = self;
        match dual {
            Some("corner") => s.dual = D::MedianPerCorner,
            Some("point") => s.dual = D::MedianPerPoint,
            Some("centre") => s.dual = D::CellCentre,
            _ => {}
        }
        match fp {
            Some("edges") => s.feature_points = F::OnFeatureEdge,
            Some("corners") => s.feature_points = F::Corners,
            _ => {}
        }
        match layerq {
            Some("backoff") => s.layer_quality = L::BackOffOnly,
            Some("cfmesh") => s.layer_quality = L::CfMeshOptimise,
            Some("snappy") => s.layer_quality = L::SnappyCollapse,
            _ => {}
        }
        s
    }
}

/// Facets of the cylinder surface handed to cfMesh (the blender primitive's
/// segments): 48, as the stage 0 harness used.
pub const CYL_SEGMENTS: usize = 48;
/// The unstructured mesher's limits, relative to the pipe radius R: cell
/// size in [R / CELL_PER_R_MAX, R / CELL_PER_R_MIN], at most
/// [`MAX_LAYERS`] layers, first layer in [1e-3 R, 0.1 R], expansion in
/// [1, 1.5]; and the estimated cell count ([`TetDualSpec::estimated_cells`])
/// at most [`MAX_CELLS`].
///
/// Measured basis (`--mesh-sweep`, native release, 2026-11-04, the demo pipe
/// R 10 mm, L 200 mm, first layer 0.3 mm, expansion 1.2): h = 6 / 5 / 4 /
/// 3.3 mm gave 1939 / 3113 / 6353 / 9055 cells bare and 4774 / 7632 / 11968
/// / 16620 wall faces, meshed in 0.2-2 s (0.54 s for the 20k default). Below
/// h = R/3 the pipeline starts skipping stages (2.5 mm: fewer wall faces than
/// at 3.3; 2 mm: the dual skipped, an all-tet mesh), so the cell count stops
/// following h and the mesh is not what the reader asked for: the finest
/// cell offered is R/3. Above R/1.5 the pipe is two cells across.
pub const CELL_PER_R_MAX: f64 = 3.0;
pub const CELL_PER_R_MIN: f64 = 1.5;
pub const MAX_LAYERS: usize = 3;

impl TetDualSpec {
    /// The demo's default for a pipe of radius `radius`. ~~Cell 0.4 R, **no
    /// layer** (first layer 0.03 R and expansion 1.2 for when layers are asked
    /// for).~~ **CHANGED 2026-11-05 (gh:#1298, maintainer decision): cell
    /// 0.5 R, 3 wall layers, first layer 0.03 R, expansion 1.2**, on
    /// [`Self::BASE`]'s mesher. On the demo pipe (h 5 mm): see the gh:#1298
    /// layer-case numbers at the end of this comment; the pin is
    /// `headless_unstructured_default_is_pinned`. The bare cell 0.4 R case
    /// below stays reachable (page URL `h=4&layers=0`). History of the bare
    /// 0.4 R default: On the demo pipe: 6 353 cells, meshed in about 0.5 s; ~~61 steps
    /// and about 9 s natively with `Gauss linear` convection, u_c/2U 0.942,
    /// dp/dz 0.941 × HP~~ **UPDATED 2026-11-04 (gh:#1202)**, with the
    /// feature-edge capture (the inlet now spans 0.9936 πR², was 0.9396) and
    /// the tutorials' `linearUpwind` + `cellLimited` convection: 29 steps,
    /// about 5 s, u_c/2U 0.8925, dp/dz 1.0326 × HP (the V&V record). History,
    /// all 2026-11-04: ~~2 layers,
    /// cell R/2 (18 377 cells)~~ diverged in the solve, as every mesh with 2+
    /// layers did; ~~1 layer, cell R/2 (10 745)~~ and ~~1 layer, cell 0.6 R
    /// (6 713)~~ ran but, with one layer, sit further from Hagen–Poiseuille
    /// than the bare mesh of the same cell size (0.85 against 0.86 at 6 mm,
    /// 0.87 against 0.89 at 5 mm).
    ///
    /// **gh:#1298 (2026-11-05):** the same sizes on the new mesher
    /// ([`Self::BASE`]: cell-centre dual, corners, upstream's sequence):
    /// 6 353 cells, meshed in 2.4 s natively, 27 steps, u_c/2U 0.8832,
    /// dp/dz 1.0649 × HP, inlet flux 0.99391 πR² (the V&V record's
    /// gh:#1298 section; ~~the pin is `headless_unstructured_default_is_pinned`~~
    /// no longer pinned: it missed its dp/dz prediction, 1.00–1.04).
    ///
    /// **The default since the maintainer's gh:#1298 decision (2026-11-05):**
    /// cell 0.5 R, 3 layers, first 0.03 R, × 1.2: 7 049 cells, 26 steps,
    /// u_c/2U 0.9394, dp/dz 1.0062 × HP (gh:#1298's layer-case run, and the
    /// V&V record's re-run of the default; pinned by
    /// `headless_unstructured_default_is_pinned`).
    pub fn default_for(radius: f64) -> Self {
        Self { cell_size: 0.5 * radius, n_layers: 3, first_layer: 0.03 * radius, expansion: 1.2, ..Self::BASE }
    }

    /// Rough cell count of the tet-dual mesh of a pipe (radius `r`, length
    /// `l`) with these settings: `.0` dual cells per h³ of pipe volume, plus
    /// `n_layers` prisms per wall face, the wall carrying `.1` faces per h²
    /// of wall area, from [`MESH_ESTIMATE`] for the per-corner dual and
    /// [`MESH_ESTIMATE_POLY_DUAL`] for the two `polyDualMesh` duals (one
    /// wall face per boundary point, gh:#1298: the per-corner coefficient
    /// overestimated them about fourfold, ~26k shown for 7 025 cells).
    /// Measured on this pipe; used only to refuse settings that would
    /// freeze a phone.
    pub fn estimated_cells(&self, r: f64, l: f64) -> f64 {
        let h = self.cell_size.max(1e-12);
        let vol = std::f64::consts::PI * r * r * l;
        let wall = 2.0 * std::f64::consts::PI * r * l;
        let k = match self.dual {
            outram_park_fork_cfmesh::dual::DualKind::MedianPerCorner => MESH_ESTIMATE,
            _ => MESH_ESTIMATE_POLY_DUAL,
        };
        k.0 * vol / (h * h * h) + self.n_layers as f64 * k.1 * wall / (h * h)
    }

    /// The spec within the limits above, for a pipe of radius `r` and length
    /// `l`: each control clamped to its range, then the cell size raised until
    /// the estimate is at most [`MAX_CELLS`].
    pub fn clamped(&self, r: f64, l: f64) -> Self {
        let mut s = Self {
            cell_size: self.cell_size.clamp(r / CELL_PER_R_MAX, r / CELL_PER_R_MIN),
            n_layers: self.n_layers.min(MAX_LAYERS),
            first_layer: self.first_layer.clamp(1e-3 * r, 0.1 * r),
            expansion: self.expansion.clamp(1.0, 1.5),
            ..*self
        };
        while s.estimated_cells(r, l) > MAX_CELLS as f64 && s.cell_size < r / CELL_PER_R_MIN {
            s.cell_size = (s.cell_size * 1.05).min(r / CELL_PER_R_MIN);
        }
        s
    }
}

/// `(dual cells per h³ of pipe volume, wall faces per h² of wall area)` of
/// the tet-dual pipe mesh: 6.2-6.7 and 13.7-15.2 over h = 4-6 mm in the
/// `--mesh-sweep` of 2026-11-04 (see [`CELL_PER_R_MAX`]).
pub const MESH_ESTIMATE: (f64, f64) = (6.5, 15.0);

/// The same two coefficients for the `polyDualMesh` duals (`CellCentre`,
/// `MedianPerPoint`), the default's since gh:#1298. Measured by
/// `--mesh-sweep` on the gh:#1298 default (2026-11-05, the demo pipe):
/// bare cells per h³ 6.67 / 6.19 / 6.47 / 6.39 at h = 6 / 5 / 4 / 3.3 mm
/// (the per-corner value 6.5 kept: the cell count is the same), wall faces
/// per h² 2.36 / 2.61 / 2.61 / 2.52 (824 / 1 312 / 2 052 / 2 904 wall
/// faces; each layer adds one cell per wall face). 2.6 is the h 4–5 mm
/// value, where the default sits; it overestimates h 6 mm by ~4 %.
pub const MESH_ESTIMATE_POLY_DUAL: (f64, f64) = (6.5, 2.6);

impl Default for Design {
    /// The stage 0 pipe: D = 20 mm, L = 200 mm, inlet at z−, 20k cells.
    fn default() -> Self {
        Self {
            shape: Shape::Cylinder { radius: 0.01, length: 0.2, centre: [0.0, 0.0, 0.1] },
            tags: [Tag::Inlet, Tag::Outlet, Tag::Wall, Tag::Wall, Tag::Wall, Tag::Wall],
            target_cells: DEFAULT_CELLS,
            mesh: PipeMesh::Structured,
        }
    }
}

impl Design {
    /// The demo's coil section (gh:#1157): the V&V coil's tube and coil
    /// (d = 10 mm, D_w = 100 mm, h = 20 mm) with [`COIL_DEFAULT_TURNS`]
    /// turns, 20k cells.
    pub fn coil() -> Self {
        let s = CoilSpec::vv();
        Self {
            shape: Shape::Coil { tube_radius: s.tube_radius, coil_diameter: s.coil_diameter, pitch: s.pitch, turns: COIL_DEFAULT_TURNS, centre: [0.0; 3] },
            tags: [Tag::Wall; 6],
            target_cells: DEFAULT_CELLS,
            mesh: PipeMesh::Structured,
        }
    }

    /// The coil spec and its axis origin (the helix axis at the inlet's
    /// height), for a coil design.
    pub fn coil_spec(&self) -> Option<(CoilSpec, [f64; 3])> {
        match self.shape {
            Shape::Coil { tube_radius, coil_diameter, pitch, turns, centre } => {
                Some((CoilSpec { tube_radius, coil_diameter, pitch, turns }, [centre[0], centre[1], centre[2] - 0.5 * pitch * turns]))
            }
            _ => None,
        }
    }

    /// The demo's double-pipe exchanger: tube r = 5 mm with a 1 mm wall, shell
    /// r = 10 mm, L = 200 mm (the geometry of the V&V record
    /// `cht_double_pipe_gh1156.md`), 20k cells.
    pub fn exchanger() -> Self {
        Self {
            shape: Shape::DoublePipe { r_tube: 0.005, wall: 0.001, r_shell: 0.01, length: 0.2, centre: [0.0, 0.0, 0.1] },
            tags: [Tag::Wall; 6],
            target_cells: DEFAULT_CELLS,
            mesh: PipeMesh::Structured,
        }
    }
}

/// Why a design cannot be meshed or run, in words for the page.
pub fn check(d: &Design) -> Result<(), String> {
    if d.target_cells == 0 || d.target_cells > MAX_CELLS {
        return Err(format!("cells must be 1..{MAX_CELLS}"));
    }
    if let Some(g) = coil_grid(d) {
        return g.check().map_err(|e| e.to_string());
    }
    if let Shape::DoublePipe { r_tube, wall, r_shell, length, .. } = d.shape {
        if !(r_tube > 1e-4 && wall > 1e-5 && r_shell > r_tube + wall + 1e-4 && length > 1e-3) {
            return Err("the tube, its wall and the shell must nest with positive gaps".into());
        }
        return Ok(());
    }
    let tags = &d.tags[..d.shape.n_faces()];
    let n_in = tags.iter().filter(|t| **t == Tag::Inlet).count();
    if n_in != 1 {
        return Err(format!("tag exactly one face as the inlet (now {n_in})"));
    }
    if !tags.contains(&Tag::Outlet) {
        return Err("tag at least one face as an outlet".into());
    }
    if d.shape == (Shape::Cylinder { radius: 0.0, length: 0.0, centre: [0.0; 3] }) {
        return Err("empty shape".into());
    }
    let [lo, hi] = d.shape.bounds();
    if (0..3).any(|i| !(hi[i] - lo[i] > 1e-6)) {
        return Err("every dimension must be positive".into());
    }
    if d.target_cells == 0 || d.target_cells > MAX_CELLS {
        return Err(format!("cells must be 1..{MAX_CELLS}"));
    }
    Ok(())
}

fn m(v: f64) -> Length {
    Length::new::<meter>(v)
}

/// The demo's O-grid of side `n`: `(n, n − 2, n·L/D)`, the stage 0 family
/// (8/6/80 is 20 480 cells at L/D = 10). Its radial cells are a third longer
/// than `PipeOGrid::level`'s (nr = n), which keeps the viscous number at
/// Co ≈ 4 under 2: on the level family the same budget needs Co ≈ 2 and
/// four times the steps (measured 2026-11-03, 20k: 136 steps).
pub fn demo_ogrid(radius: f64, length: f64, n: usize) -> PipeOGrid {
    let ld = length / (2.0 * radius);
    let nz = ((n as f64 * ld).round() as usize).max(1);
    PipeOGrid::new(m(radius), m(length), n, n.saturating_sub(2).max(1), nz)
}

/// The pipe O-grid for a cylinder: the largest [`demo_ogrid`] within 5 % of
/// the budget (n ≥ 2).
pub fn pipe_grid(radius: f64, length: f64, centre: [f64; 3], target: usize) -> PipeOGrid {
    let mut lvl = 2;
    for k in 2..128 {
        if demo_ogrid(radius, length, k).n_cells() as f64 <= 1.05 * target as f64 {
            lvl = k;
        } else {
            break;
        }
    }
    let mut g = demo_ogrid(radius, length, lvl);
    g.origin = [m(centre[0]), m(centre[1]), m(centre[2] - 0.5 * length)];
    g
}

/// The box block: near-cubic cells, at most the budget.
pub fn box_block(size: [f64; 3], centre: [f64; 3], target: usize) -> BoxBlock {
    let vol = size[0] * size[1] * size[2];
    let mut h = (vol / target.max(1) as f64).cbrt();
    let cells = loop {
        let c = size.map(|s| ((s / h).round() as usize).max(1));
        if c.iter().product::<usize>() <= target.max(1) || h > 1e9 {
            break c;
        }
        h *= 1.02;
    };
    let mut b = BoxBlock::new(size.map(m), cells);
    b.origin = [0, 1, 2].map(|i| m(centre[i] - 0.5 * size[i]));
    b
}

/// The double-pipe grid for a cell budget: `4 n` cells around every region;
/// radial cells `n − 1` (tube fluid ring), 2 (wall), `n` (annulus); axial
/// `1.5 n L / (2 r_shell)`; the largest n ≥ 3 within 5 % of the budget.
pub fn exchanger_grid(r_tube: f64, wall: f64, r_shell: f64, length: f64, centre: [f64; 3], target: usize) -> DoublePipe {
    let make = |n: usize| DoublePipe {
        r_tube_inner: m(r_tube),
        r_tube_outer: m(r_tube + wall),
        r_shell: m(r_shell),
        length: m(length),
        origin: [m(centre[0]), m(centre[1]), m(centre[2] - 0.5 * length)],
        n,
        nr: [n - 1, 2, n],
        nz: ((1.5 * n as f64 * length / (2.0 * r_shell)).round() as usize).max(2),
    };
    let mut best = make(3);
    for n in 3..64 {
        let g = make(n);
        if g.n_cells().iter().sum::<usize>() as f64 <= 1.05 * target as f64 {
            best = g;
        } else {
            break;
        }
    }
    best
}

/// The coil grid of a coil design for its cell budget
/// ([`CoilSpec::for_budget`]), positioned; `None` for other shapes.
pub fn coil_grid(d: &Design) -> Option<HelicalPipe> {
    let (spec, origin) = d.coil_spec()?;
    let mut g = spec.for_budget(d.target_cells);
    g.axis_origin = origin.map(m);
    Some(g)
}

/// The mesh of a design (checked), and the flow axis (0 x, 1 y, 2 z) with the
/// signed inlet-to-outlet direction, and the cell length along it \[m\].
#[cfg_attr(target_arch = "wasm32", allow(dead_code))]
pub fn mesh_design(d: &Design) -> Result<(PolyMesh, usize, f64), String> {
    mesh_design_noted(d).map(|(p, a, h, _)| (p, a, h))
}

/// The unstructured pipe (gh:#1202): `outram_blender`'s cylinder primitive
/// (`CYL_SEGMENTS` sides) with its z− end, z+ end and side named by the
/// design's tags, through cfMesh's tet → dual → layers pipeline
/// (`outram_blender::foam_mesh::mesh_to_tet_dual_multipatch`), the prism
/// layers grown on the `walls`-tagged faces only
/// (`TetDualOptions::layer_patches`, the port of cfMesh's per-patch
/// `boundaryLayers`), moved to the design's centre, and converted through
/// the neutral mesh (`convert::cfmesh`, `convert::foam`) to the `PolyMesh`
/// the solver and the section cutter take. Returns the stage notes of the
/// pipeline too (what it skipped).
pub fn mesh_unstructured(radius: f64, length: f64, centre: [f64; 3], tags: [Tag; 3], spec: &TetDualSpec) -> Result<(PolyMesh, Vec<String>), String> {
    // The spec's layer strategy (the page's control, gh:#1240), unless the
    // native knob FOAM_LAYER_STRATEGY names one (the V&V tooling).
    #[cfg(not(target_arch = "wasm32"))]
    let strategy = layer_strategy_from_env().unwrap_or(spec.layer_quality);
    #[cfg(target_arch = "wasm32")]
    let strategy = spec.layer_quality;
    mesh_unstructured_with(radius, length, centre, tags, spec, strategy).map(|(p, n, _)| (p, n))
}

/// [`mesh_unstructured`] with the layer strategy named (gh:#1215), also
/// returning what the layer stage did. The native knob
/// `FOAM_LAYER_STRATEGY` of [`mesh_unstructured`] goes through here.
pub fn mesh_unstructured_with(
    radius: f64,
    length: f64,
    centre: [f64; 3],
    tags: [Tag; 3],
    spec: &TetDualSpec,
    strategy: outram_blender::foam_mesh::LayerQualityStrategy,
) -> Result<(PolyMesh, Vec<String>, Option<outram_blender::foam_mesh::LayerOutcome>), String> {
    // The spec's dual kind unless the native knob FOAM_DUAL_KIND names one.
    #[cfg(not(target_arch = "wasm32"))]
    let dual = dual_kind_from_env().unwrap_or(spec.dual);
    #[cfg(target_arch = "wasm32")]
    let dual = spec.dual;
    mesh_unstructured_dual(radius, length, centre, tags, spec, strategy, dual)
}

/// [`mesh_unstructured_with`] with the dual kind named too (gh:#1240):
/// the native knob `FOAM_DUAL_KIND` of [`mesh_unstructured_with`] goes
/// through here, and so does `--render-dual-kinds`.
pub fn mesh_unstructured_dual(
    radius: f64,
    length: f64,
    centre: [f64; 3],
    tags: [Tag; 3],
    spec: &TetDualSpec,
    strategy: outram_blender::foam_mesh::LayerQualityStrategy,
    dual: outram_park_fork_cfmesh::dual::DualKind,
) -> Result<(PolyMesh, Vec<String>, Option<outram_blender::foam_mesh::LayerOutcome>), String> {
    use outram_blender::foam_mesh::{mesh_to_tet_dual_multipatch, TetDualOptions, Vec3};
    let s = spec.clamped(radius, length);
    let cyl = outram_blender::primitives::cylinder(CYL_SEGMENTS, radius, length);
    let label = |t: [Vec3; 3]| -> String {
        let flat = (t[0].z - t[1].z).abs() < 1e-12 && (t[1].z - t[2].z).abs() < 1e-12;
        let face = match (flat, t[0].z < 0.0) {
            (true, true) => 0,
            (true, false) => 1,
            _ => 2,
        };
        tags[face].name().to_string()
    };
    let opts = TetDualOptions {
        cell_size: s.cell_size,
        n_layers: s.n_layers,
        first_layer_thickness: s.first_layer,
        expansion: s.expansion,
        layer_patches: vec![Tag::Wall.name().to_string()],
        ..TetDualOptions::default()
    };
    // Native-only isolation knob for the V&V record (gh:#1202):
    // FOAM_CFMESH_STAGES=carve (carve + snap: hexahedra) or tet (no dual).
    #[cfg(not(target_arch = "wasm32"))]
    let opts = match std::env::var("FOAM_CFMESH_STAGES").as_deref() {
        Ok("carve") => TetDualOptions { dual: false, delaunay: false, n_layers: 0, ..opts },
        Ok("tet") => TetDualOptions { dual: false, n_layers: 0, ..opts },
        _ => opts,
    };
    let opts = TetDualOptions { layer_strategy: strategy, dual_kind: dual, dual_feature_points: s.feature_points, ..opts };
    // The cfMesh route (gh:#1298): the spec's, unless the native knob
    // FOAM_CFMESH_ROUTE=legacy|upstream names one (the V&V tooling).
    #[cfg(not(target_arch = "wasm32"))]
    let route = match std::env::var("FOAM_CFMESH_ROUTE").as_deref() {
        Ok("legacy") => CfMeshRoute::PreGh1298,
        Ok("upstream") => CfMeshRoute::UpstreamNoLaplace,
        _ => s.route,
    };
    #[cfg(target_arch = "wasm32")]
    let route = s.route;
    let opts = route_options(opts, route);
    // Native-only knob (gh:#1240): FOAM_DUAL_FP=corners|edges, the
    // polyDualMesh class's / utility's feature points, for the two new dual
    // kinds; it overrides the spec's choice for the V&V tooling.
    #[cfg(not(target_arch = "wasm32"))]
    let opts = match std::env::var("FOAM_DUAL_FP").as_deref() {
        Ok("corners") => TetDualOptions { dual_feature_points: outram_park_fork_cfmesh::dual::FeaturePointRule::Corners, ..opts },
        Ok("edges") => TetDualOptions { dual_feature_points: outram_park_fork_cfmesh::dual::FeaturePointRule::OnFeatureEdge, ..opts },
        _ => opts,
    };
    // Native-only knob (gh:#1241): FOAM_CORRECT_EDGES=1, cfMesh's
    // correctEdgesBetweenPatches before the feature-edge capture.
    #[cfg(not(target_arch = "wasm32"))]
    let opts = TetDualOptions { correct_edges: std::env::var("FOAM_CORRECT_EDGES").as_deref() == Ok("1"), ..opts };
    // Native-only knobs, each `1` (on) or `0` (off); **unset keeps the
    // route's value** (gh:#1298; before it, unset meant off, the route
    // then being the pre-gh:#1298 one):
    // - gh:#1255 FOAM_CFMESH_BMOVES, untangleMeshFV's boundary moves;
    // - gh:#1256 FOAM_CFMESH_LAPLACE, optimizeMeshFV's laplacianPC pass;
    // - gh:#1260 FOAM_CFMESH_LPU, the LP meshUntangler;
    // - gh:#1259 FOAM_CFMESH_OPTSURF, optimizeSurface(2) inside optimiseLayer;
    // - gh:#1283 FOAM_CFMESH_PROJECT=each|after|off, the boundary moves
    //   projected back onto the surface (not upstream);
    // - gh:#1259 FOAM_UNTANGLE_SURFACE, cfMesh's meshSurfaceOptimizer in the
    //   feature-edge capture; gh:#1283 FOAM_OPTIMISE_FINAL, optimiseFinalMesh.
    #[cfg(not(target_arch = "wasm32"))]
    let opts = {
        let flag = |k: &str, cur: bool| match std::env::var(k).as_deref() {
            Ok("1") => true,
            Ok("0") => false,
            _ => cur,
        };
        let c = opts.cfmesh_layer_optimisation.clone();
        TetDualOptions {
            cfmesh_layer_optimisation: outram_park_fork_cfmesh::boundary_layer_optimisation::BoundaryLayerOptimisationControls {
                untangle_boundary_moves: flag("FOAM_CFMESH_BMOVES", c.untangle_boundary_moves),
                optimize_mesh_fv_laplace: flag("FOAM_CFMESH_LAPLACE", c.optimize_mesh_fv_laplace),
                untangle_mesh_untangler: flag("FOAM_CFMESH_LPU", c.untangle_mesh_untangler),
                optimise_surface: flag("FOAM_CFMESH_OPTSURF", c.optimise_surface),
                boundary_projection: match std::env::var("FOAM_CFMESH_PROJECT").as_deref() {
                    Ok("each") => outram_park_fork_cfmesh::final_mesh::BoundaryProjection::EachPass,
                    Ok("after") => outram_park_fork_cfmesh::final_mesh::BoundaryProjection::AfterUntangle,
                    Ok("off") => outram_park_fork_cfmesh::final_mesh::BoundaryProjection::Off,
                    _ => c.boundary_projection,
                },
                ..c.clone()
            },
            untangle_surface: flag("FOAM_UNTANGLE_SURFACE", opts.untangle_surface),
            optimise_final_mesh: flag("FOAM_OPTIMISE_FINAL", opts.optimise_final_mesh),
            ..opts
        }
    };
    #[allow(unused_mut)]
    let (mut vm, mut report) = mesh_to_tet_dual_multipatch(&cyl, label, &opts)?;
    // Native-only V&V note (gh:#1283): the inflow area the uniform inlet
    // velocity sees, |Σ S_f·ẑ| over the inlet faces / πR² (a bowed inlet
    // has more face area than flux; the solve's Q is U_IN times this).
    #[cfg(not(target_arch = "wasm32"))]
    if let Some(q) = vm.patches.iter().find(|q| q.name == Tag::Inlet.name()) {
        let a: f64 = (q.start_face..q.start_face + q.n_faces).map(|f| vm.face_area_vector(f).z).sum();
        report.stage_notes.push(format!("inlet flux area |sum S.z|/piR2 = {:.5}", a.abs() / (std::f64::consts::PI * radius * radius)));
    }
    for p in &mut vm.points {
        *p = Vec3::new(p.x + centre[0], p.y + centre[1], p.z + centre[2]);
    }
    let neutral = outram_blender::unstructured::convert::cfmesh::from_volume_mesh(&vm).map_err(|e| format!("{e:?}"))?;
    let foam = outram_blender::unstructured::convert::foam::to_poly_mesh(&neutral).map_err(|e| format!("{e:?}"))?;
    Ok((PolyMesh::from_foam_poly_mesh(&foam), report.stage_notes, report.layers))
}

/// `opts` with the cfMesh steps of `route` (gh:#1298; see [`CfMeshRoute`]).
/// [`CfMeshRoute::PreGh1298`] leaves `opts` as they are (every new step
/// off, `TetDualOptions::default()`'s optimiser controls).
pub fn route_options(opts: outram_blender::foam_mesh::TetDualOptions, route: CfMeshRoute) -> outram_blender::foam_mesh::TetDualOptions {
    use outram_blender::foam_mesh::TetDualOptions;
    use outram_park_fork_cfmesh::boundary_layer_optimisation::BoundaryLayerOptimisationControls;
    match route {
        CfMeshRoute::PreGh1298 => opts,
        CfMeshRoute::UpstreamNoLaplace => TetDualOptions {
            untangle_surface: true,
            optimise_final_mesh: true,
            cfmesh_layer_optimisation: BoundaryLayerOptimisationControls {
                untangle_boundary_moves: true,
                untangle_mesh_untangler: true,
                optimize_mesh_fv_laplace: false,
                boundary_projection: outram_park_fork_cfmesh::final_mesh::BoundaryProjection::EachPass,
                ..opts.cfmesh_layer_optimisation.clone()
            },
            ..opts
        },
    }
}

/// The layer strategy `FOAM_LAYER_STRATEGY` names (native only, gh:#1215; the
/// knob of the differential record, `laminar_pipe_gh1155.md`): `cfmesh`,
/// `snappy` or `backoff`; ~~the default back-off for anything else~~
/// **CHANGED 2026-11-05 (gh:#1240 browser controls):** `None` when unset or
/// unrecognised, so the spec's own choice (the page's control) applies.
#[cfg(not(target_arch = "wasm32"))]
fn layer_strategy_from_env() -> Option<outram_blender::foam_mesh::LayerQualityStrategy> {
    use outram_blender::foam_mesh::LayerQualityStrategy as S;
    match std::env::var("FOAM_LAYER_STRATEGY").as_deref() {
        Ok("cfmesh") => Some(S::CfMeshOptimise),
        Ok("snappy") => Some(S::SnappyCollapse),
        Ok("backoff") => Some(S::BackOffOnly),
        _ => None,
    }
}

/// The dual kind `FOAM_DUAL_KIND` names (native only, gh:#1240; the knob of
/// the differential record, `laminar_pipe_gh1155.md`): `point`
/// (`MedianPerPoint`), `centre` (`CellCentre`) or `corner`; ~~the default
/// per-corner median dual for anything else~~ **CHANGED 2026-11-05 (browser
/// controls):** `None` when unset or unrecognised, so the spec's choice
/// applies.
#[cfg(not(target_arch = "wasm32"))]
fn dual_kind_from_env() -> Option<outram_park_fork_cfmesh::dual::DualKind> {
    use outram_park_fork_cfmesh::dual::DualKind as D;
    match std::env::var("FOAM_DUAL_KIND").as_deref() {
        Ok("point") => Some(D::MedianPerPoint),
        Ok("centre") => Some(D::CellCentre),
        Ok("corner") => Some(D::MedianPerCorner),
        _ => None,
    }
}

/// [`mesh_design`] with the mesher's notes (empty for the block meshes).
pub fn mesh_design_noted(d: &Design) -> Result<(PolyMesh, usize, f64, Vec<String>), String> {
    check(d)?;
    let name = |i: usize| d.tags[i].name().to_string();
    let none = |(p, a, h): (PolyMesh, usize, f64)| (p, a, h, Vec::new());
    match d.shape {
        Shape::Cylinder { radius, length, centre } if matches!(d.mesh, PipeMesh::Unstructured(_)) => {
            let PipeMesh::Unstructured(spec) = d.mesh else { unreachable!() };
            let (poly, notes) = mesh_unstructured(radius, length, centre, [d.tags[0], d.tags[1], d.tags[2]], &spec)?;
            Ok((poly, 2, spec.clamped(radius, length).cell_size, notes))
        }
        Shape::Cylinder { radius, length, centre } => {
            let mut g = pipe_grid(radius, length, centre, d.target_cells);
            g.patch_names = [name(0), name(1), name(2)];
            let dz = length / g.nz as f64;
            g.mesh().map(|p| none((p, 2, dz))).map_err(|e| e.to_string())
        }
        Shape::Box { size, centre } => {
            let mut b = box_block(size, centre, d.target_cells);
            b.patch_names = [name(0), name(1), name(2), name(3), name(4), name(5)];
            let inlet = d.tags.iter().take(6).position(|t| *t == Tag::Inlet).unwrap_or(4);
            let axis = inlet / 2;
            let h = size[axis] / b.cells[axis] as f64;
            b.mesh().map(|p| none((p, axis, h))).map_err(|e| e.to_string())
        }
        Shape::DoublePipe { r_tube, wall, r_shell, length, centre } => {
            let g = exchanger_grid(r_tube, wall, r_shell, length, centre, d.target_cells);
            let [a, b, c] = g.meshes().map_err(|e| e.to_string())?;
            let all = PolyMesh::concatenate(&[("tube_fluid", &a), ("tube_wall", &b), ("annulus_fluid", &c)]);
            Ok(none((all, 2, length / g.nz as f64)))
        }
        Shape::Coil { .. } => {
            let g = coil_grid(d).ok_or("not a coil")?;
            let ds = g.arc_length().get::<meter>() / g.n_layers as f64;
            g.mesh().map(|p| none((p, 2, ds))).map_err(|e| e.to_string())
        }
    }
}

/// The three region meshes of a double-pipe design (tube fluid, tube wall,
/// annulus fluid), in the order [`mesh_design`] concatenates them.
pub fn exchanger_meshes(d: &Design) -> Result<[Arc<FvMesh>; 3], String> {
    let Shape::DoublePipe { r_tube, wall, r_shell, length, centre } = d.shape else {
        return Err("not a double pipe".into());
    };
    let g = exchanger_grid(r_tube, wall, r_shell, length, centre, d.target_cells);
    let [a, b, c] = g.meshes().map_err(|e| e.to_string())?;
    let fv = |p: PolyMesh| p.to_fv_mesh().map(Arc::new).map_err(|e| e.to_string());
    Ok([fv(a)?, fv(b)?, fv(c)?])
}

/// How the solve runs.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Mode {
    /// Pseudo-transient PISO at Co ≈ 4 to the steady state (the default),
    /// with Δt capped by the viscous number (see [`MAX_VISCOUS_DEFAULT`]).
    Steady,
    /// Co ≈ 1, to watch the profile develop.
    Transient,
}

impl Mode {
    pub fn courant(self) -> f64 {
        match self {
            Mode::Steady => 4.0,
            Mode::Transient => 1.0,
        }
    }
}

/// What the UI asks for.
#[derive(Clone, Debug, PartialEq)]
pub enum Request {
    /// Mesh `design`, replacing any case; the probe sits at fraction `probe`
    /// of the way from inlet to outlet on the axis.
    Mesh { id: u32, design: Design, probe: f64 },
    /// Set up the solve on the current mesh from rest.
    Start { id: u32, reynolds: f64, mode: Mode },
    /// Advance `steps` time steps and report.
    Step { id: u32, steps: usize },
    /// Move the probe and the cross-section to fraction `probe`.
    Probe { id: u32, probe: f64 },
}

/// A plane section, ready to draw: the plane (axis 0/1/2, offset \[m\]) and
/// one polygon per cut cell in in-plane coordinates \[m\]. With `frame`
/// (`[origin, e1, e2]`, 9 numbers) the plane is not axis-aligned: an
/// in-plane point `(a, b)` is `origin + a e1 + b e2` (the coil's
/// cross-section, `e1 = N` towards the coil axis, `e2 = B`), and `axis`
/// and `offset` are unused.
#[derive(Clone, Debug, PartialEq)]
pub struct Section {
    pub axis: u8,
    pub offset: f64,
    pub polygons: Vec<Vec<[f32; 2]>>,
    pub frame: Option<[f64; 9]>,
}

/// What the mesh looks like to the page.
#[derive(Clone, Debug, PartialEq)]
pub struct Meshed {
    pub id: u32,
    pub n_cells: usize,
    pub max_non_ortho_deg: f64,
    /// Flow axis 0/1/2 and the cell length along it \[m\].
    pub flow_axis: u8,
    pub cell_len: f64,
    /// Boundary edges: x0 y0 z0 x1 y1 z1 tag (0 inlet, 1 outlet, 2 wall).
    pub edges: Vec<f32>,
    pub axial: Section,
    pub cross: Section,
    /// checkMesh-style max skewness (`outram_foam_mesh::mesh_quality`).
    pub max_skewness: f64,
    /// Wall time of the meshing \[s\] (in the worker, in the browser).
    pub mesh_s: f64,
    /// What the mesher skipped or changed, for the page (empty for the block
    /// meshes).
    pub notes: String,
}

/// Non-orthogonal correctors on the unstructured pipe (see `Engine::start`):
/// 2, as OpenFOAM's unstructured-mesh tutorials `elbow` and `flange`.
pub const NON_ORTH_UNSTRUCTURED: usize = 2;
/// Above this max non-orthogonality \[deg\] the unstructured pipe runs
/// `Gauss linear limited corrected` [`LIMITED_COEFF`] (see `Engine::start`).
/// 70°: between snappyHexMesh's `maxNonOrtho 65` of the tutorials that run
/// `corrected` and the 75° their layers are allowed in the tutorials that
/// run `limited corrected 0.33` (propeller). ~~Before the maintainer's
/// direction of 2026-11-04 this was 80° with `limited 0` and no correctors.~~
pub const LIMITED_ABOVE_DEG: f64 = 70.0;
/// ψ of `limited corrected ψ` (propeller, turbineSiting).
pub const LIMITED_COEFF: f64 = 0.33;

/// Radial bins of the profile plot.
pub const PROFILE_BINS: usize = 12;

/// Wall edges drawn at most (see `Engine::mesh`).
pub const MAX_DRAWN_EDGES: usize = 20_000;

/// One report of the solve.
#[derive(Clone, Debug, PartialEq)]
pub struct Progress {
    pub id: u32,
    pub steps: usize,
    pub time_s: f64,
    pub dt_s: f64,
    /// Max cell |ΔU|/U per flow-through time (the steady metric).
    pub change: f64,
    /// Worst cell |Σ φ| over the inlet flow rate.
    pub continuity: f64,
    pub outlet_over_inlet: f64,
    /// |U| \[m/s\] and kinematic p \[m²/s²\] at the probe cell.
    pub probe_u: f64,
    pub probe_p: f64,
    /// Mean p of the inlet cells minus that of the outlet cells \[m²/s²\].
    pub dp: f64,
    pub ms_per_step: f64,
    pub steady: bool,
    /// Per section cell: |U|, kinematic p, and the axial component of U.
    pub axial_u: Vec<f32>,
    pub axial_p: Vec<f32>,
    pub cross_u: Vec<f32>,
    pub cross_p: Vec<f32>,
    pub cross_ua: Vec<f32>,
    /// 0 while the flow is marched, 1 once it is frozen and the exchanger's
    /// temperatures are iterated (the pipe stays at 0).
    pub phase: u8,
    /// Per section cell: T \[K\] (empty for the pipe).
    pub axial_t: Vec<f32>,
    pub cross_t: Vec<f32>,
    /// The exchanger's numbers (NaN for the pipe): see [`Exch`].
    pub exch: [f64; EXCH_LEN],
    /// The coil's secondary velocity on the cross-section, `u_n, u_b` per
    /// cell \[m/s\] (empty unless a coil); there `cross_ua` is `u_t`.
    pub cross_sec: Vec<f32>,
    /// The coil's numbers (NaN unless a coil): see [`CoilNo`].
    pub coil: [f64; COIL_LEN],
    /// The pipe's radial profile by binning cell centres in r (gh:#1202,
    /// `laminar_pipe::radial_profile_binned`): pairs `r/R, u_z/U`; empty
    /// unless a cylinder is solved.
    pub profile: Vec<f32>,
}

/// Indices into [`Progress::coil`].
pub struct CoilNo;
impl CoilNo {
    /// Dean number.
    pub const DEAN: usize = 0;
    /// f·Re/64 over the last turn (at most L/2) and the window before it.
    pub const F_RE_64: usize = 1;
    pub const F_RE_64_BEFORE: usize = 2;
    /// Mishra & Gupta (1979) f_c/f_s (tampines).
    pub const MISHRA_GUPTA: usize = 3;
    /// Axial maximum's n/R (negative: outer side), largest secondary speed
    /// over U, core u_n/U (negative: outward), the vortex-pair sign test.
    pub const PEAK_N: usize = 4;
    pub const MAX_SECONDARY: usize = 5;
    pub const CORE_UN: usize = 6;
    pub const PAIR: usize = 7;
}
/// Length of [`Progress::coil`].
pub const COIL_LEN: usize = 8;
/// Turns of the demo coil (see the V&V record for why).
pub const COIL_DEFAULT_TURNS: f64 = 2.0;
/// Above this Reynolds number the coil's steady march runs at Co 1, not 4:
/// on the V&V grid (m = 3) the Co 4 march did not converge at Re = 400 and
/// 800 (the steady metric grew after 150 steps), while Re ≤ 200 converged
/// at Co 4 (measured 2026-11-03, `helical_coil_dean_gh1157.md`). At Co 1
/// Re 400 does not converge on that grid either (gh:#1172).
pub const COIL_CO4_MAX_RE: f64 = 250.0;

/// The coil's Courant number for `mode` at `reynolds` (see
/// [`COIL_CO4_MAX_RE`]).
pub fn coil_courant(mode: Mode, reynolds: f64) -> f64 {
    if reynolds > COIL_CO4_MAX_RE {
        1.0
    } else {
        mode.courant()
    }
}

/// Indices into [`Progress::exch`].
pub struct Exch;
impl Exch {
    /// Hot (tube) and cold (annulus) mixing-cup outlet temperatures \[K\].
    pub const T_HOT_OUT: usize = 0;
    pub const T_COLD_OUT: usize = 1;
    /// Heat lost by the hot stream and gained by the cold one \[W\].
    pub const Q_HOT: usize = 2;
    pub const Q_COLD: usize = 3;
    /// Solved effectiveness, and ε-NTU (dwsim-libs, counter-current) at the
    /// local UA.
    pub const EPS: usize = 4;
    pub const EPS_NTU: usize = 5;
    /// Local UA \[W/K\] and NTU = UA/C_min.
    pub const UA: usize = 6;
    pub const NTU: usize = 7;
    /// Largest |ΔT| of the last energy pass over the inlet difference.
    pub const ENERGY_CHANGE: usize = 8;
}
/// Length of [`Progress::exch`].
pub const EXCH_LEN: usize = 9;
/// Hot and cold inlet temperatures of the exchanger \[K\] (the
/// `DoublePipeExchanger::demo` values).
pub const T_HOT_IN: f64 = 350.0;
pub const T_COLD_IN: f64 = 300.0;
/// The exchanger's energy is steady when an energy pass changes no cell by
/// more than this fraction of the inlet temperature difference.
pub const ENERGY_STEADY: f64 = 1.0e-7;
/// Energy passes per exchanger step (one request).
pub const ENERGY_PASSES_PER_STEP: usize = 10;

#[derive(Clone, Debug, PartialEq)]
pub enum Event {
    Meshed(Meshed),
    /// A moved cross-section (after [`Request::Probe`]).
    Cross { id: u32, cross: Section },
    Progress(Progress),
    Error(String),
}

/// A running double-pipe exchanger: the three coupled regions, the case,
/// whether the flow is frozen yet, and where each region's cells start in
/// the concatenated drawing mesh.
pub struct ExchangerRun {
    pub cht: ChtMultiRegionFoam,
    pub spec: DoublePipeExchanger,
    pub energy_phase: bool,
    pub offsets: [usize; 3],
    pub last_energy_change: f64,
}

/// What is being solved.
pub enum Case {
    /// A pipe or duct by `PimpleFoam` (stage 1).
    Pipe(PimpleFoam, PipeFlow),
    /// The double-pipe exchanger by `ChtMultiRegionFoam` (stage 2).
    Exchanger(ExchangerRun),
    /// The helical coil section by `PimpleFoam` (gh:#1157).
    Coil(CoilRun),
}

/// The engine: at most one mesh and one case.
#[derive(Default)]
pub struct Engine {
    id: u32,
    poly: Option<PolyMesh>,
    fv: Option<Arc<FvMesh>>,
    design: Option<Design>,
    flow_axis: usize,
    cell_len: f64,
    axial: Vec<CellSection>,
    cross: Vec<CellSection>,
    probe_cell: usize,
    case: Option<Case>,
    steps: usize,
    time: f64,
    /// For a coil: the straight O-grid's mesh (same cell order) whose z
    /// sections are the coil's true cross-sections, and the coil grid.
    straight: Option<PolyMesh>,
    coil: Option<HelicalPipe>,
    /// Where the cross-section plane is on the flow axis \[m\].
    probe_x: f64,
    /// OpenFOAM's `adjustTimeStep` with this `maxCo` on the pipe
    /// (gh:#1241; [`courant_numbers`], [`adjusted_delta_t`]); `None` (the
    /// default) keeps the fixed pseudo-time step every recorded number used.
    /// Natively `FOAM_MAX_CO` sets it at the start of a run.
    pub max_co: Option<f64>,
    /// `maxDeltaT` of the adjustable step: the pipe's fixed step.
    max_dt: f64,
}

/// OpenFOAM's Courant number of an incompressible case (OpenFOAM-dev
/// `852527377c7e`, `src/finiteVolume/cfdTools/incompressible/CourantNo.H`):
/// `CoNum = ½ Δt max_c(Σ_f |φ_f| / V_c)` and `meanCoNum = ½ Δt Σ_c Σ_f
/// |φ_f| / Σ_c V_c`, the sum over every face of the cell, boundary faces
/// included (`fvc::surfaceSum(mag(phi))`). Returns `(CoNum, meanCoNum)`.
pub fn courant_numbers(s: &outram_foam_appbuilder_lib::solvers::pimple_foam::PimpleFoam) -> (f64, f64) {
    let m = &s.mesh;
    let mut sum = vec![0.0_f64; m.n_cells];
    for f in 0..m.n_internal_faces {
        let a = s.phi.internal[f].abs();
        sum[m.owner[f]] += a;
        sum[m.neighbour[f]] += a;
    }
    for (pi, p) in m.patches.iter().enumerate() {
        for i in 0..p.size {
            sum[m.owner[p.start + i]] += s.phi.boundary[pi].values[i].abs();
        }
    }
    let dt = s.control.delta_t;
    let co = (0..m.n_cells).map(|c| sum[c] / m.cell_volumes[c]).fold(0.0_f64, f64::max);
    let mean = sum.iter().sum::<f64>() / m.cell_volumes.iter().sum::<f64>();
    (0.5 * co * dt, 0.5 * mean * dt)
}

/// OpenFOAM's time-step update (OpenFOAM-dev `852527377c7e`,
/// `src/finiteVolume/cfdTools/general/include/setDeltaT.H`):
/// `maxDeltaTFact = maxCo / (CoNum + small)`, `deltaTFact = min(min(
/// maxDeltaTFact, 1 + 0.1 maxDeltaTFact), 1.2)`, `Δt = min(deltaTFact Δt,
/// maxDeltaT)`; and with `initial` the first step's
/// (`setInitialDeltaT.H`): `Δt = min(maxCo Δt / CoNum, min(Δt, maxDeltaT))`
/// when `CoNum > small`.
pub fn adjusted_delta_t(dt: f64, co_num: f64, max_co: f64, max_dt: f64, initial: bool) -> f64 {
    const SMALL: f64 = 1e-15;
    if initial {
        return if co_num > SMALL { (max_co * dt / co_num).min(dt.min(max_dt)) } else { dt };
    }
    let fact = max_co / (co_num + SMALL);
    let dt_fact = fact.min(1.0 + 0.1 * fact).min(1.2);
    (dt_fact * dt).min(max_dt)
}

fn axis_of(i: usize) -> Axis {
    match i {
        0 => Axis::X,
        1 => Axis::Y,
        _ => Axis::Z,
    }
}

/// One value per section cell.
fn pick(cs: &[CellSection], f: impl Fn(usize) -> f64) -> Vec<f32> {
    cs.iter().map(|c| f(c.cell) as f32).collect()
}

fn section_of(cs: &[CellSection], axis: usize, offset: f64) -> Section {
    Section {
        axis: axis as u8,
        offset,
        polygons: cs.iter().map(|c| c.polygon.iter().map(|p| [p[0] as f32, p[1] as f32]).collect()).collect(),
        frame: None,
    }
}

/// The viscous-number cap of the run (native: `FOAM_MAX_VISCOUS` overrides).
fn max_viscous() -> f64 {
    #[cfg(not(target_arch = "wasm32"))]
    let v = std::env::var("FOAM_MAX_VISCOUS").ok().and_then(|v| v.parse().ok()).unwrap_or(MAX_VISCOUS_DEFAULT);
    #[cfg(target_arch = "wasm32")]
    let v = MAX_VISCOUS_DEFAULT;
    v
}

/// The exchanger's numbers from the case report, with ε-NTU at the local UA
/// from `outram-park-fork-dwsim-libs` (counter-current).
pub fn exchanger_numbers(r: &ExchangerReport, energy_change: f64) -> [f64; EXCH_LEN] {
    use outram_park_fork_dwsim_libs::heat_exchanger::{lmtd::FlowArrangement, ntu_effectiveness::evaluate};
    use uom::si::f64::{TemperatureInterval, ThermalConductance};
    use uom::si::thermal_conductance::watt_per_kelvin;
    let w = ThermalConductance::new::<watt_per_kelvin>;
    let e = evaluate(
        FlowArrangement::CounterCurrent,
        w(r.ua_local.max(0.0)),
        w(r.c_hot),
        w(r.c_cold),
        TemperatureInterval::new::<uom::si::temperature_interval::kelvin>(T_HOT_IN - T_COLD_IN),
    );
    let mut x = [f64::NAN; EXCH_LEN];
    x[Exch::T_HOT_OUT] = r.t_hot_out;
    x[Exch::T_COLD_OUT] = r.t_cold_out;
    x[Exch::Q_HOT] = r.q_hot;
    x[Exch::Q_COLD] = r.q_cold;
    x[Exch::EPS] = r.effectiveness;
    x[Exch::EPS_NTU] = e.effectiveness.get::<uom::si::ratio::ratio>();
    x[Exch::UA] = r.ua_local;
    x[Exch::NTU] = e.ntu.get::<uom::si::ratio::ratio>();
    x[Exch::ENERGY_CHANGE] = energy_change;
    x
}

impl Engine {
    /// The in-plane axis for the axial section: y unless the flow is along y.
    fn axial_axis(&self) -> usize {
        if self.flow_axis == 1 {
            0
        } else {
            1
        }
    }

    fn centre(&self) -> [f64; 3] {
        let [lo, hi] = self.design.map_or([[0.0; 3]; 2], |d| d.shape.bounds());
        [0, 1, 2].map(|i| 0.5 * (lo[i] + hi[i]))
    }

    /// Where the probe and the cross-section sit for fraction `f`; the plane
    /// is nudged off the grid planes by 1e-4 of a cell.
    fn place_probe(&mut self, f: f64) -> Section {
        let (Some(d), Some(poly), Some(fv)) = (self.design, &self.poly, &self.fv) else {
            return section_of(&[], 2, 0.0);
        };
        if let (Some(g), Some(st)) = (&self.coil, &self.straight) {
            return Self::coil_cross(g, st, fv, f, &mut self.cross, &mut self.probe_cell);
        }
        let [lo, hi] = d.shape.bounds();
        let a = self.flow_axis;
        let inlet_low = match d.shape {
            Shape::Cylinder { .. } => d.tags[0] == Tag::Inlet,
            Shape::Box { .. } => d.tags[2 * a] == Tag::Inlet,
            // The hot (tube) inlet is at z−.
            Shape::DoublePipe { .. } | Shape::Coil { .. } => true,
        };
        let f = f.clamp(0.0, 1.0);
        let x = if inlet_low { lo[a] + f * (hi[a] - lo[a]) } else { hi[a] - f * (hi[a] - lo[a]) };
        let x = x.clamp(lo[a] + 0.51 * self.cell_len, hi[a] - 0.51 * self.cell_len) + 1e-4 * self.cell_len;
        self.cross = plane_section(poly, axis_of(a), m(x));
        self.probe_x = x;
        let mut p = self.centre();
        p[a] = x;
        self.probe_cell = (0..fv.n_cells)
            .min_by(|&i, &j| {
                let di = fv.cell_centres[i] - outram_foam_basic_lib::primitives::Vector3::new(p[0], p[1], p[2]);
                let dj = fv.cell_centres[j] - outram_foam_basic_lib::primitives::Vector3::new(p[0], p[1], p[2]);
                di.mag().total_cmp(&dj.mag())
            })
            .unwrap_or(0);
        section_of(&self.cross, a, x)
    }

    /// The coil's cross-section at fraction `f` of the arc: the layer of
    /// cells whose centre is nearest, cut from the straight grid (same cell
    /// order) at its mid-plane and drawn in the Frenet plane there
    /// (`e1 = N`, `e2 = B`); the probe is the cell nearest the centreline.
    pub fn coil_cross(g: &HelicalPipe, straight: &PolyMesh, fv: &FvMesh, f: f64, cross: &mut Vec<CellSection>, probe: &mut usize) -> Section {
        let l = g.arc_length().get::<meter>();
        let ds = l / g.n_layers as f64;
        let k = ((f.clamp(0.0, 1.0) * l / ds).floor()).clamp(0.0, g.n_layers as f64 - 1.0);
        let s = (k + 0.5) * ds;
        *cross = plane_section(straight, Axis::Z, m(s));
        // Straight (x, y) are the swept frame's (U, V); rotate to (N, B).
        let th = -g.torsion() * s;
        let (st, ct) = (th.sin(), th.cos());
        for c in cross.iter_mut() {
            for p in &mut c.polygon {
                let (x, y) = (p[0], p[1]);
                *p = [x * ct - y * st, x * st + y * ct];
            }
        }
        let o = g.centreline(m(s));
        let [_, n, b] = g.frenet(m(s));
        let target = outram_foam_basic_lib::primitives::Vector3::new(o[0], o[1], o[2]);
        *probe = cross.iter().map(|c| c.cell).min_by(|&i, &j| (fv.cell_centres[i] - target).mag().total_cmp(&(fv.cell_centres[j] - target).mag())).unwrap_or(0);
        let mut sec = section_of(cross, 2, s);
        sec.frame = Some([o[0], o[1], o[2], n[0], n[1], n[2], b[0], b[1], b[2]]);
        sec
    }

    fn mesh(&mut self, id: u32, design: Design, probe: f64) -> Result<Meshed, String> {
        let t0 = bugis::web_demo::platform::now_s();
        let (poly, axis, cell_len, mut notes) = mesh_design_noted(&design)?;
        let mesh_s = bugis::web_demo::platform::now_s() - t0;
        let quality = outram_foam_mesh::mesh_quality::assess_quality(&poly.to_foam_poly_mesh());
        // Some cfMesh settings leave a near-degenerate cell (the sweep of
        // 2026-11-04 before the feature-edge capture and the layer quality
        // limits: h 4 mm with 2 layers, skewness 251); say so on the page.
        if quality.max_non_ortho_deg >= 89.0 || quality.max_skewness > 20.0 {
            notes.push(format!(
                "warning: near-degenerate cells (max non-orthogonality {:.1}°, skewness {:.0}); the solve may be poor or diverge, try another cell size",
                quality.max_non_ortho_deg, quality.max_skewness
            ));
        }
        self.coil = coil_grid(&design);
        self.straight = match &self.coil {
            Some(g) => Some(g.straight_grid().mesh().map_err(|e| e.to_string())?),
            None => None,
        };
        let fv = Arc::new(poly.to_fv_mesh().map_err(|e| e.to_string())?);
        self.id = id;
        self.design = Some(design);
        self.flow_axis = axis;
        self.cell_len = cell_len;
        self.case = None;
        // Edge colours: inlet 0, outlet 1, anything else a wall (2). The
        // exchanger's patches are "<region>/<patch>".
        let tag_of: Vec<f32> = poly
            .patches
            .iter()
            .map(|p| match p.name.rsplit('/').next().unwrap_or("") {
                "inlet" => 0.0,
                "outlet" => 1.0,
                _ => 2.0,
            })
            .collect();
        let mut edges = Vec::new();
        let all = boundary_edges(&poly);
        // An unstructured wall has many small faces: draw at most about
        // MAX_DRAWN_EDGES of them (every k-th wall edge; all inlet and outlet
        // edges), so the message and each frame stay small.
        let stride = (all.len() / MAX_DRAWN_EDGES).max(1);
        for (k, (a, b, pi)) in all.into_iter().enumerate() {
            if tag_of[pi] == 2.0 && k % stride != 0 {
                continue;
            }
            edges.extend(a.iter().chain(b.iter()).map(|v| *v as f32));
            edges.push(tag_of[pi]);
        }
        let aa = self.axial_axis();
        let off = self.centre()[aa] + 1e-7;
        self.axial = plane_section(&poly, axis_of(aa), m(off));
        let n_cells = fv.n_cells;
        let max_non_ortho_deg = max_non_orthogonality_deg(&fv);
        self.poly = Some(poly);
        self.fv = Some(fv);
        let cross = self.place_probe(probe);
        Ok(Meshed {
            id,
            n_cells,
            max_non_ortho_deg,
            flow_axis: axis as u8,
            cell_len,
            edges,
            axial: section_of(&self.axial, aa, off),
            cross,
            max_skewness: quality.max_skewness,
            mesh_s,
            notes: notes.join("; "),
        })
    }

    /// The flow set-up of the current design at `reynolds`, on the inlet's
    /// hydraulic diameter (the tube's, for the exchanger).
    fn flow(&self, reynolds: f64) -> PipeFlow {
        let (dh, len) = match self.design.map(|d| d.shape) {
            Some(Shape::Cylinder { radius, length, .. }) => (2.0 * radius, length),
            Some(Shape::DoublePipe { r_tube, length, .. }) => (2.0 * r_tube, length),
            // Not used: the coil's flow is `CoilRun`'s.
            Some(Shape::Coil { tube_radius, .. }) => (2.0 * tube_radius, 0.0),
            Some(Shape::Box { size, .. }) => {
                let a = self.flow_axis;
                let (b, c) = (size[(a + 1) % 3], size[(a + 2) % 3]);
                (2.0 * b * c / (b + c), size[a])
            }
            None => (0.02, 0.2),
        };
        PipeFlow {
            radius: m(0.5 * dh),
            length: m(len),
            bulk_velocity: Velocity::new::<meter_per_second>(U_IN),
            nu: KinematicViscosity::new::<square_meter_per_second>(U_IN * dh / reynolds.max(1e-3)),
        }
    }

    /// The exchanger at tube Reynolds number `reynolds`: `DoublePipeExchanger::demo`
    /// with the design's length and the flow rate that gives that Re in the
    /// tube (ν fixed at 1e-6 m²/s); each fluid at Co 4 capped by the viscous
    /// number.
    fn start_exchanger(&mut self, reynolds: f64) -> Result<(), String> {
        let d = self.design.ok_or("mesh first")?;
        let Shape::DoublePipe { r_tube, length, .. } = d.shape else { return Err("not a double pipe".into()) };
        let [a, b, c] = exchanger_meshes(&d)?;
        let mut spec = DoublePipeExchanger::demo();
        spec.length = m(length);
        let nu = spec.nu.get::<square_meter_per_second>();
        spec.flow_rate = uom::si::f64::VolumeRate::new::<uom::si::volume_rate::cubic_meter_per_second>(reynolds * nu / (2.0 * r_tube) * std::f64::consts::PI * r_tube * r_tube);
        let q = spec.flow_rate.get::<uom::si::volume_rate::cubic_meter_per_second>();
        let inlet_area = |mesh: &FvMesh| -> f64 {
            mesh.patches.iter().find(|p| p.name == "inlet").map_or(1.0, |p| (0..p.size).map(|i| mesh.face_areas[p.start + i]).sum())
        };
        let dz = m(self.cell_len);
        let dt = |mesh: &FvMesh| double_pipe::pseudo_time_step(mesh, Velocity::new::<meter_per_second>(q / inlet_area(mesh)), spec.nu, dz, Mode::Steady.courant(), max_viscous());
        let (dta, dtc) = (dt(&a), dt(&c));
        let offsets = [0, a.n_cells, a.n_cells + b.n_cells];
        let cht = spec.build(a, b, c, dta, dtc).map_err(|e| e.to_string())?;
        self.case = Some(Case::Exchanger(ExchangerRun { cht, spec, energy_phase: false, offsets, last_energy_change: f64::NAN }));
        self.steps = 0;
        self.time = 0.0;
        Ok(())
    }

    fn start(&mut self, reynolds: f64, mode: Mode) -> Result<(), String> {
        if matches!(self.design.map(|d| d.shape), Some(Shape::DoublePipe { .. })) {
            return self.start_exchanger(reynolds);
        }
        let fv = self.fv.clone().ok_or("mesh first")?;
        if let Some(g) = self.coil.clone() {
            // The V&V record's settings: Gauss linear convection (the Dean
            // vortices are resolved convection), Co 4 (1 above
            // `COIL_CO4_MAX_RE`, or to watch it develop) under the viscous
            // cap, one non-orthogonal corrector.
            let run = CoilRun::on_mesh(g, fv, reynolds, outram_foam_appbuilder_lib::io::fv_schemes::DivScheme::GaussLinearDeferred, coil_courant(mode, reynolds), max_viscous());
            self.case = Some(Case::Coil(run));
            self.steps = 0;
            self.time = 0.0;
            return Ok(());
        }
        let flow = self.flow(reynolds);
        // Co ≈ 4 (or 1), but the viscous number ν Δt / h² held at
        // `MAX_VISCOUS_DEFAULT` or below, h the mean cell size (V/N)^(1/3);
        // `cases::double_pipe::pseudo_time_step` (gh:#1162: PimpleFoam
        // diverged at ν Δt / Δy² = 8 on a uniform channel). The smallest
        // spacing is the wrong length here: the O-grid's thin ring cells at
        // the square's corners (0.37 mm at 20k) would cap Co at ≈ 1.6, while
        // the uncapped Co ≈ 4 run is stable (gh:#1161 record: 34 steps) —
        // measured 2026-11-03, 79 steps with that cap.
        let dt = double_pipe::pseudo_time_step(&fv, flow.bulk_velocity, flow.nu, m(self.cell_len), mode.courant(), max_viscous());
        let mut s = flow.build_solver(fv, dt);
        // gh:#1161: the lagged correction alone (0 correctors) went unstable
        // at Co ≈ 4 on the 50k O-grid; one corrector is stable there.
        s.solution.pimple.n_non_orthogonal_correctors = 1;
        // gh:#1202, maintainer direction: the unstructured mesh gets
        // non-orthogonal correctors and the laplacian/snGrad scheme upstream's
        // tutorials use on unstructured and snappy meshes (OpenFOAM-dev
        // `852527377c7e`, read 2026-11-04; the V&V record lists each):
        // - `nNonOrthogonalCorrectors 2`: `legacy/incompressible/icoFoam/elbow`
        //   (laminar PISO on an unstructured Fluent mesh) and
        //   `legacy/basic/laplacianFoam/flange` (unstructured ANSYS mesh);
        // - `Gauss linear corrected` up to LIMITED_ABOVE_DEG (elbow, flange,
        //   and the snappy cases meshed to maxNonOrtho 65: motorBikeSteady,
        //   drivaerFastback, windAroundBuildings);
        // - `Gauss linear limited corrected 0.33` above it (propeller,
        //   turbineSiting: snappy meshes whose layers are allowed to 75°;
        //   `limitedSnGrad` ported for this, `non_orthogonal::Limited`).
        // Convection stays upwind on both meshes (the demo's scheme, so the
        // two meshes are compared under one scheme); the tutorials use
        // `limitedLinearV 1` (elbow) or `linearUpwind grad(U)`.
        if matches!(self.design.map(|d| d.mesh), Some(PipeMesh::Unstructured(_))) {
            s.solution.pimple.n_non_orthogonal_correctors = NON_ORTH_UNSTRUCTURED;
            // ~~Convection: `Gauss linear` on the unstructured mesh~~ (gh:#1202,
            // 2026-11-04, superseded below). Upwind on faces oblique to the flow adds cross-stream
            // false diffusion of order U·h (Patankar 1980, §5.6), 10-15× ν here:
            // it flattened the developed profile to u_c/2U 0.62. The tutorials on
            // unstructured meshes use second-order upwind-biased schemes
            // (`limitedLinearV 1`, elbow; `linearUpwind grad(U)`, propeller), which
            // are not ported; `Gauss linear` is the second-order scheme the port
            // has (the coil demo uses it too). The O-grid, whose faces are aligned
            // with the flow, keeps upwind and its recorded numbers.
            // UPDATED 2026-11-04 (gh:#1202): `linearUpwind` and `cellLimited`
            // are ported; the demo now runs the tutorials' pair,
            // `div(phi,U) Gauss linearUpwind grad(U)` with
            // `grad(U) cellLimited Gauss linear 1` (`incompressibleFluid/propeller`,
            // `multiRegion/CHT/misalignedDuct`), chosen before measuring it.
            s.schemes.default_div = outram_foam_appbuilder_lib::io::fv_schemes::DivScheme::GaussLinearUpwind("grad(U)".into());
            s.schemes.grad_u = Some(outram_foam_appbuilder_lib::io::fv_schemes::GradScheme::CellLimitedGaussLinear(1.0));
            if max_non_orthogonality_deg(&s.mesh) > LIMITED_ABOVE_DEG {
                s.schemes.default_laplacian = outram_foam_appbuilder_lib::io::fv_schemes::LaplacianScheme::GaussLinearLimited(LIMITED_COEFF);
            }
            // Native-only experiment knobs for the V&V record (gh:#1202):
            // FOAM_NON_ORTH (correctors), FOAM_LAPLACIAN=uncorrected|corrected|<ψ>.
            #[cfg(not(target_arch = "wasm32"))]
            {
                use outram_foam_appbuilder_lib::io::fv_schemes::LaplacianScheme;
                if let Some(n) = std::env::var("FOAM_NON_ORTH").ok().and_then(|v| v.parse().ok()) {
                    s.solution.pimple.n_non_orthogonal_correctors = n;
                }
                match std::env::var("FOAM_LAPLACIAN").as_deref() {
                    Ok("uncorrected") => s.schemes.default_laplacian = LaplacianScheme::GaussLinearUncorrected,
                    Ok("corrected") => s.schemes.default_laplacian = LaplacianScheme::GaussLinearCorrected,
                    Ok(v) => {
                        if let Ok(psi) = v.parse::<f64>() {
                            s.schemes.default_laplacian = LaplacianScheme::GaussLinearLimited(psi);
                        }
                    }
                    Err(_) => {}
                }
            }
        }
        // gh:#1241: adjustTimeStep (off unless asked for); its maxDeltaT is
        // this fixed step.
        self.max_dt = s.control.delta_t;
        #[cfg(not(target_arch = "wasm32"))]
        if self.max_co.is_none() {
            self.max_co = std::env::var("FOAM_MAX_CO").ok().and_then(|v| v.parse().ok());
        }
        self.case = Some(Case::Pipe(s, flow));
        self.steps = 0;
        self.time = 0.0;
        Ok(())
    }

    /// The radial profile at the cross-section plane, for a solved cylinder:
    /// cell centres binned in r ([`PROFILE_BINS`] bins) over a slab about the
    /// plane, half a cell thick on the O-grid (exactly the layer the plane
    /// cuts) and one cell size each side on the unstructured mesh, whose
    /// centres do not sit on planes. Flattened `r/R, u_z/U` pairs.
    fn binned_profile(&self) -> Vec<f32> {
        let (Some(Case::Pipe(s, flow)), Some(d)) = (self.case.as_ref(), self.design) else { return Vec::new() };
        let Shape::Cylinder { centre, .. } = d.shape else { return Vec::new() };
        let hz = match d.mesh {
            PipeMesh::Structured => 0.5 * self.cell_len,
            PipeMesh::Unstructured(_) => self.cell_len,
        };
        outram_foam_appbuilder_lib::cases::laminar_pipe::radial_profile_binned(s, flow, (centre[0], centre[1]), self.probe_x, hz, PROFILE_BINS)
            .into_iter()
            .flat_map(|(r, u, _)| [r as f32, u as f32])
            .collect()
    }

    /// Per-cell fields on the drawing mesh: |U|, p, axial U and T (T empty
    /// for the pipe), plus the run's diagnostics.
    fn progress(&self, change: f64, ms: f64) -> Option<Progress> {
        let a = self.flow_axis;
        let comp = |v: outram_foam_basic_lib::primitives::Vector3| match a {
            0 => v.x,
            1 => v.y,
            _ => v.z,
        };
        let n = self.fv.as_ref().map_or(0, |f| f.n_cells);
        let mut coil = [f64::NAN; COIL_LEN];
        let mut cross_sec = Vec::new();
        let mut cross_ut: Option<Vec<f32>> = None;
        if let Some(Case::Coil(run)) = self.case.as_ref() {
            let r = run.report();
            coil = [r.dean, r.f_re_64, r.f_re_64_before, r.mishra_gupta, r.section.axial_peak_n, r.section.max_secondary, r.section.core_u_n, r.section.vortex_pair];
            let mut ut = Vec::new();
            for c in &self.cross {
                let [a, b, cc] = outram_foam_appbuilder_lib::cases::helical_coil::frenet_components(run.solver.u.internal[c.cell], &run.cells.frenet[c.cell]);
                ut.push(a as f32);
                cross_sec.push(b as f32);
                cross_sec.push(cc as f32);
            }
            cross_ut = Some(ut);
        }
        let (u, p, t, continuity, out_in, dp, dt, exch, phase): (Vec<_>, Vec<f64>, Vec<f64>, f64, f64, f64, f64, [f64; EXCH_LEN], u8) = match self.case.as_ref()? {
            Case::Pipe(s, _) | Case::Coil(CoilRun { solver: s, .. }) => {
                let mesh = &s.mesh;
                let pp = s.p.internal.as_slice();
                let mean_p = |name: &str| {
                    mesh.patches.iter().find(|q| q.name == name).map_or(0.0, |q| {
                        let (mut s1, mut w) = (0.0, 0.0);
                        for fi in 0..q.size {
                            let a = mesh.face_areas[q.start + fi];
                            s1 += pp[mesh.owner[q.start + fi]] * a;
                            w += a;
                        }
                        s1 / w.max(1e-300)
                    })
                };
                let inflow = -patch_flux(s, "inlet");
                (
                    s.u.internal.as_slice().to_vec(),
                    pp.to_vec(),
                    Vec::new(),
                    max_continuity_error(s),
                    patch_flux(s, "outlet") / inflow.max(1e-300),
                    mean_p("inlet") - mean_p("outlet"),
                    s.control.delta_t,
                    [f64::NAN; EXCH_LEN],
                    0,
                )
            }
            Case::Exchanger(x) => {
                let mut u = vec![outram_foam_basic_lib::primitives::Vector3::ZERO; n];
                let mut p = vec![0.0; n];
                let mut t = vec![f64::NAN; n];
                for (k, f) in x.cht.fluids.iter().enumerate() {
                    let o = x.offsets[2 * k];
                    for c in 0..f.mesh.n_cells {
                        u[o + c] = f.u.internal[c];
                        p[o + c] = f.p.internal[c];
                        t[o + c] = f.energy.as_ref().map_or(f64::NAN, |e| e.t.internal[c]);
                    }
                }
                let w = &x.cht.solids[0].energy.t;
                for c in 0..w.mesh.n_cells {
                    t[x.offsets[1] + c] = w.internal[c];
                }
                let f0 = &x.cht.fluids[0];
                let cont = max_continuity_error(f0).max(max_continuity_error(&x.cht.fluids[1]));
                let oi = |f: &PimpleFoam| patch_flux(f, "outlet") / (-patch_flux(f, "inlet")).max(1e-300);
                let exch = if x.energy_phase {
                    exchanger_numbers(&double_pipe::report(&x.cht, &x.spec), x.last_energy_change)
                } else {
                    [f64::NAN; EXCH_LEN]
                };
                (u, p, t, cont, oi(f0).min(oi(&x.cht.fluids[1])), f64::NAN, f0.control.delta_t, exch, u8::from(x.energy_phase))
            }
        };
        let tt = |c: usize| t.get(c).copied().unwrap_or(f64::NAN);
        Some(Progress {
            id: self.id,
            steps: self.steps,
            time_s: self.time,
            dt_s: dt,
            change,
            continuity,
            outlet_over_inlet: out_in,
            probe_u: u.get(self.probe_cell).map_or(0.0, |v| v.mag()),
            probe_p: p.get(self.probe_cell).copied().unwrap_or(0.0),
            dp,
            ms_per_step: ms,
            steady: match self.case.as_ref()? {
                Case::Pipe(..) | Case::Coil(..) => change < STEADY,
                Case::Exchanger(x) => x.energy_phase && x.last_energy_change < ENERGY_STEADY,
            },
            axial_u: pick(&self.axial, |c| u[c].mag()),
            axial_p: pick(&self.axial, |c| p[c]),
            cross_u: pick(&self.cross, |c| u[c].mag()),
            cross_p: pick(&self.cross, |c| p[c]),
            cross_ua: cross_ut.unwrap_or_else(|| pick(&self.cross, |c| comp(u[c]))),
            phase,
            axial_t: if t.is_empty() { Vec::new() } else { pick(&self.axial, tt) },
            cross_t: if t.is_empty() { Vec::new() } else { pick(&self.cross, tt) },
            exch,
            cross_sec,
            coil,
            profile: self.binned_profile(),
        })
    }

    /// Advance the case by up to `steps` and return (steps done, change).
    /// The exchanger marches its flows to the steady criterion, then freezes
    /// them and iterates the energy, [`ENERGY_PASSES_PER_STEP`] passes per
    /// step, to [`ENERGY_STEADY`].
    fn advance(&mut self, steps: usize) -> Result<(usize, f64), String> {
        let diverged = || "the solve diverged; try a smaller Reynolds number or fewer cells".to_string();
        let mut done = 0;
        let mut change = f64::INFINITY;
        match self.case.as_mut().ok_or("no case")? {
            Case::Pipe(s, flow) => {
                // gh:#1241: OpenFOAM's adjustTimeStep, when asked for; the
                // fixed step otherwise. maxDeltaT: the step the case set.
                let max_dt = self.max_dt;
                for _ in 0..steps.max(1) {
                    if let (Some(max_co), true) = (self.max_co, self.steps + done == 0) {
                        let (co, _) = courant_numbers(s);
                        s.control.delta_t = adjusted_delta_t(s.control.delta_t, co, max_co, max_dt, true);
                    }
                    change = flow.step(s).map_err(|_| diverged())?;
                    if let Some(max_co) = self.max_co {
                        let (co, _) = courant_numbers(s);
                        s.control.delta_t = adjusted_delta_t(s.control.delta_t, co, max_co, max_dt, false);
                    }
                    if !change.is_finite() {
                        return Err(diverged());
                    }
                    done += 1;
                    self.time += s.control.delta_t;
                    if change < STEADY {
                        break;
                    }
                }
            }
            Case::Coil(run) => {
                for _ in 0..steps.max(1) {
                    change = run.step().map_err(|_| diverged())?;
                    done += 1;
                    self.time += run.solver.control.delta_t;
                    if change < STEADY {
                        break;
                    }
                }
            }
            Case::Exchanger(x) => {
                for _ in 0..steps.max(1) {
                    if x.energy_phase {
                        let r = x.cht.step().map_err(|e| e.to_string())?;
                        x.last_energy_change = r.last_pass_change / (T_HOT_IN - T_COLD_IN);
                        change = x.last_energy_change;
                        done += 1;
                        if change < ENERGY_STEADY {
                            break;
                        }
                    } else {
                        change = double_pipe::step_flows(&mut x.cht, x.spec.length).map_err(|_| diverged())?;
                        if !change.is_finite() {
                            return Err(diverged());
                        }
                        done += 1;
                        self.time += x.cht.fluids[0].control.delta_t;
                        if change < STEADY {
                            double_pipe::freeze_flows_for_steady_energy(&mut x.cht);
                            x.cht.energy_outer_correctors = ENERGY_PASSES_PER_STEP;
                            x.energy_phase = true;
                            break;
                        }
                    }
                }
            }
        }
        Ok((done, change))
    }

    /// Serve one request.
    pub fn serve(&mut self, req: Request, post: &mut impl FnMut(Event)) {
        match req {
            Request::Mesh { id, design, probe } => match self.mesh(id, design, probe) {
                Ok(me) => post(Event::Meshed(me)),
                Err(e) => post(Event::Error(format!("meshing failed: {e}"))),
            },
            Request::Start { id, reynolds, mode } if id == self.id => match self.start(reynolds, mode) {
                Ok(()) => {
                    if let Some(p) = self.progress(f64::INFINITY, 0.0) {
                        post(Event::Progress(p));
                    }
                }
                Err(e) => post(Event::Error(e)),
            },
            Request::Step { id, steps } if id == self.id => {
                if self.case.is_none() {
                    return;
                }
                let t0 = bugis::web_demo::platform::now_s();
                match self.advance(steps) {
                    Ok((done, change)) => {
                        self.steps += done;
                        let ms = (bugis::web_demo::platform::now_s() - t0) * 1e3 / done.max(1) as f64;
                        if let Some(p) = self.progress(change, ms) {
                            post(Event::Progress(p));
                        }
                    }
                    Err(e) => {
                        post(Event::Error(e));
                        self.case = None;
                    }
                }
            }
            Request::Probe { id, probe } if id == self.id => {
                let cross = self.place_probe(probe);
                post(Event::Cross { id, cross });
                if self.case.is_some() {
                    if let Some(p) = self.progress(f64::NAN, 0.0) {
                        post(Event::Progress(p));
                    }
                }
            }
            // A request from a design that has since been replaced.
            _ => {}
        }
    }
}

#[cfg(not(target_arch = "wasm32"))]
impl bugis::web_demo::link::NativeEngine for Engine {
    type Req = Request;
    type Ev = Event;
    fn handle(&mut self, req: Request, post: &mut impl FnMut(Event)) {
        self.serve(req, post);
    }
}

#[cfg(target_arch = "wasm32")]
impl bugis::web_demo::link::WorkerEngine for Engine {
    type Req = Request;
    type Ev = Event;
    fn handle(state: &std::sync::Arc<std::sync::RwLock<Self>>, req: Request, post: bugis::web_demo::link::Poster<Event>) {
        if let Ok(mut e) = state.write() {
            e.serve(req, &mut |ev| post.post(ev));
        }
    }
    fn error(message: String) -> Event {
        Event::Error(message)
    }
}

// ─── Wire format: numbers in one Float64Array, text in one string ────────────

fn put_section(d: &mut Vec<f64>, s: &Section) {
    d.push(f64::from(s.axis));
    d.push(s.offset);
    match &s.frame {
        Some(f) => {
            d.push(1.0);
            d.extend_from_slice(f);
        }
        None => d.push(0.0),
    }
    d.push(s.polygons.len() as f64);
    for p in &s.polygons {
        d.push(p.len() as f64);
        for q in p {
            d.push(f64::from(q[0]));
            d.push(f64::from(q[1]));
        }
    }
}

struct Reader<'a> {
    d: &'a [f64],
    i: usize,
}

impl Reader<'_> {
    fn f(&mut self) -> Result<f64, String> {
        let v = self.d.get(self.i).copied().ok_or_else(|| format!("message too short ({} numbers)", self.d.len()))?;
        self.i += 1;
        Ok(v)
    }
    fn n(&mut self) -> Result<usize, String> {
        let v = self.f()?;
        if v < 0.0 || v > 1e8 {
            return Err(format!("bad count {v}"));
        }
        Ok(v as usize)
    }
    fn f32s(&mut self) -> Result<Vec<f32>, String> {
        let n = self.n()?;
        (0..n).map(|_| self.f().map(|v| v as f32)).collect()
    }
    fn section(&mut self) -> Result<Section, String> {
        let axis = self.f()? as u8;
        let offset = self.f()?;
        let frame = if self.f()? != 0.0 {
            let mut f = [0.0; 9];
            for v in &mut f {
                *v = self.f()?;
            }
            Some(f)
        } else {
            None
        };
        let n = self.n()?;
        let mut polygons = Vec::with_capacity(n);
        for _ in 0..n {
            let k = self.n()?;
            let mut p = Vec::with_capacity(k);
            for _ in 0..k {
                p.push([self.f()? as f32, self.f()? as f32]);
            }
            polygons.push(p);
        }
        Ok(Section { axis, offset, polygons, frame })
    }
}

fn put_f32s(d: &mut Vec<f64>, v: &[f32]) {
    d.push(v.len() as f64);
    d.extend(v.iter().map(|x| f64::from(*x)));
}

#[cfg_attr(not(target_arch = "wasm32"), allow(dead_code))]
impl Request {
    pub fn to_wire(&self) -> Vec<f64> {
        match self {
            Request::Mesh { id, design, probe } => {
                let (k, a, b) = match design.shape {
                    Shape::Cylinder { radius, length, centre } => (0.0, [radius, length, 0.0, 0.0], centre),
                    Shape::Box { size, centre } => (1.0, [size[0], size[1], size[2], 0.0], centre),
                    Shape::DoublePipe { r_tube, wall, r_shell, length, centre } => (2.0, [r_tube, wall, r_shell, length], centre),
                    Shape::Coil { tube_radius, coil_diameter, pitch, turns, centre } => (3.0, [tube_radius, coil_diameter, pitch, turns], centre),
                };
                let mut d = vec![0.0, f64::from(*id), k, a[0], a[1], a[2], a[3], b[0], b[1], b[2], design.target_cells as f64, *probe];
                d.extend(design.tags.iter().map(|t| t.code()));
                match design.mesh {
                    PipeMesh::Structured => d.extend([0.0; 9]),
                    PipeMesh::Unstructured(s) => {
                        d.extend([1.0, s.cell_size, s.n_layers as f64, s.first_layer, s.expansion]);
                        d.extend(s.choice_codes());
                    }
                }
                d
            }
            Request::Start { id, reynolds, mode } => {
                vec![1.0, f64::from(*id), *reynolds, if *mode == Mode::Steady { 0.0 } else { 1.0 }]
            }
            Request::Step { id, steps } => vec![2.0, f64::from(*id), *steps as f64],
            Request::Probe { id, probe } => vec![3.0, f64::from(*id), *probe],
        }
    }
    pub fn from_wire(d: &[f64]) -> Result<Self, String> {
        let mut r = Reader { d, i: 0 };
        let kind = r.f()? as u32;
        let id = r.f()? as u32;
        match kind {
            0 => {
                let k = r.f()?;
                let a = [r.f()?, r.f()?, r.f()?, r.f()?];
                let centre = [r.f()?, r.f()?, r.f()?];
                let target_cells = r.n()?;
                let probe = r.f()?;
                let mut tags = [Tag::Wall; 6];
                for t in &mut tags {
                    *t = Tag::from_code(r.f()?);
                }
                let shape = match k as u8 {
                    0 => Shape::Cylinder { radius: a[0], length: a[1], centre },
                    1 => Shape::Box { size: [a[0], a[1], a[2]], centre },
                    2 => Shape::DoublePipe { r_tube: a[0], wall: a[1], r_shell: a[2], length: a[3], centre },
                    _ => Shape::Coil { tube_radius: a[0], coil_diameter: a[1], pitch: a[2], turns: a[3], centre },
                };
                let mesh = if r.f()? == 0.0 {
                    let _ = [r.f()?, r.f()?, r.f()?, r.f()?, r.f()?, r.f()?, r.f()?, r.f()?];
                    PipeMesh::Structured
                } else {
                    PipeMesh::Unstructured(
                        TetDualSpec { cell_size: r.f()?, n_layers: r.n()?, first_layer: r.f()?, expansion: r.f()?, ..TetDualSpec::BASE }
                            .with_choice_codes([r.f()?, r.f()?, r.f()?, r.f()?])?,
                    )
                };
                Ok(Request::Mesh { id, design: Design { shape, tags, target_cells, mesh }, probe })
            }
            1 => Ok(Request::Start { id, reynolds: r.f()?, mode: if r.f()? == 0.0 { Mode::Steady } else { Mode::Transient } }),
            2 => Ok(Request::Step { id, steps: r.n()? }),
            3 => Ok(Request::Probe { id, probe: r.f()? }),
            k => Err(format!("unknown request {k}")),
        }
    }
}

#[cfg_attr(not(target_arch = "wasm32"), allow(dead_code))]
impl Event {
    pub fn to_wire(&self) -> (Vec<f64>, String) {
        match self {
            Event::Meshed(me) => {
                let mut d = vec![0.0, f64::from(me.id), me.n_cells as f64, me.max_non_ortho_deg, f64::from(me.flow_axis), me.cell_len];
                put_f32s(&mut d, &me.edges);
                put_section(&mut d, &me.axial);
                put_section(&mut d, &me.cross);
                d.extend([me.max_skewness, me.mesh_s]);
                (d, me.notes.clone())
            }
            Event::Cross { id, cross } => {
                let mut d = vec![1.0, f64::from(*id)];
                put_section(&mut d, cross);
                (d, String::new())
            }
            Event::Progress(p) => {
                let mut d = vec![
                    2.0,
                    f64::from(p.id),
                    p.steps as f64,
                    p.time_s,
                    p.dt_s,
                    p.change,
                    p.continuity,
                    p.outlet_over_inlet,
                    p.probe_u,
                    p.probe_p,
                    p.dp,
                    p.ms_per_step,
                    f64::from(u8::from(p.steady)),
                ];
                for v in [&p.axial_u, &p.axial_p, &p.cross_u, &p.cross_p, &p.cross_ua] {
                    put_f32s(&mut d, v);
                }
                d.push(f64::from(p.phase));
                put_f32s(&mut d, &p.axial_t);
                put_f32s(&mut d, &p.cross_t);
                d.extend_from_slice(&p.exch);
                put_f32s(&mut d, &p.cross_sec);
                d.extend_from_slice(&p.coil);
                put_f32s(&mut d, &p.profile);
                (d, String::new())
            }
            Event::Error(m) => (vec![3.0, 0.0], m.clone()),
        }
    }
    pub fn from_wire(d: &[f64], text: &str) -> Result<Self, String> {
        let mut r = Reader { d, i: 0 };
        let kind = r.f()? as u32;
        let id = r.f()? as u32;
        match kind {
            0 => Ok(Event::Meshed(Meshed {
                id,
                n_cells: r.n()?,
                max_non_ortho_deg: r.f()?,
                flow_axis: r.f()? as u8,
                cell_len: r.f()?,
                edges: r.f32s()?,
                axial: r.section()?,
                cross: r.section()?,
                max_skewness: r.f()?,
                mesh_s: r.f()?,
                notes: text.to_string(),
            })),
            1 => Ok(Event::Cross { id, cross: r.section()? }),
            2 => Ok(Event::Progress(Progress {
                id,
                steps: r.n()?,
                time_s: r.f()?,
                dt_s: r.f()?,
                change: r.f()?,
                continuity: r.f()?,
                outlet_over_inlet: r.f()?,
                probe_u: r.f()?,
                probe_p: r.f()?,
                dp: r.f()?,
                ms_per_step: r.f()?,
                steady: r.f()? != 0.0,
                axial_u: r.f32s()?,
                axial_p: r.f32s()?,
                cross_u: r.f32s()?,
                cross_p: r.f32s()?,
                cross_ua: r.f32s()?,
                phase: r.f()? as u8,
                axial_t: r.f32s()?,
                cross_t: r.f32s()?,
                exch: {
                    let mut x = [0.0; EXCH_LEN];
                    for v in &mut x {
                        *v = r.f()?;
                    }
                    x
                },
                cross_sec: r.f32s()?,
                coil: {
                    let mut x = [0.0; COIL_LEN];
                    for v in &mut x {
                        *v = r.f()?;
                    }
                    x
                },
                profile: r.f32s()?,
            })),
            3 => Ok(Event::Error(text.to_string())),
            k => Err(format!("bad event {k}")),
        }
    }
}

#[cfg(target_arch = "wasm32")]
mod web {
    use super::{Event, Request};
    use bugis::web_demo::link::{js, Message};
    use wasm_bindgen::JsValue;

    fn wrap(d: &[f64], text: &str) -> JsValue {
        let o = js::object();
        js::set(&o, "kind", "msg");
        js::set(&o, "data", js::f64s(d));
        js::set(&o, "text", text);
        o.into()
    }

    impl Message for Request {
        fn to_js(&self) -> JsValue {
            wrap(&self.to_wire(), "")
        }
        fn from_js(v: &JsValue) -> Result<Self, String> {
            Request::from_wire(&js::get_f64s(v, "data"))
        }
    }

    impl Message for Event {
        fn to_js(&self) -> JsValue {
            let (d, t) = self.to_wire();
            wrap(&d, &t)
        }
        fn from_js(v: &JsValue) -> Result<Self, String> {
            Event::from_wire(&js::get_f64s(v, "data"), &js::get_str(v, "text"))
        }
    }
}

/// The headless run (engine CLAUDE.md: headless mode is mandatory): mesh the
/// design, solve in `mode` to steady (or `max_steps`), and report one line per
/// chunk plus the pipe diagnostics. Deterministic apart from the timings.
#[cfg_attr(target_arch = "wasm32", allow(dead_code))]
pub fn headless(design: Design, reynolds: f64, mode: Mode, max_steps: usize) -> Result<Vec<String>, String> {
    let mut e = Engine::default();
    let mut out = Vec::new();
    let mut last: Option<Event> = None;
    let t0 = bugis::web_demo::platform::now_s();
    e.serve(Request::Mesh { id: 1, design, probe: 0.75 }, &mut |ev| last = Some(ev));
    match last.take() {
        Some(Event::Meshed(me)) => {
            out.push(format!(
                "# mesh: {} cells, max non-orthogonality {:.1} deg, cell length {:.3e} m, {:.3} s",
                me.n_cells,
                me.max_non_ortho_deg,
                me.cell_len,
                bugis::web_demo::platform::now_s() - t0
            ));
            out.push(format!("# quality: skew={:.3} mesh_s={:.3} notes=[{}]", me.max_skewness, me.mesh_s, me.notes));
        }
        Some(Event::Error(m)) => return Err(m),
        _ => return Err("no mesh".into()),
    }
    e.serve(Request::Start { id: 1, reynolds, mode }, &mut |ev| last = Some(ev));
    out.push("# steps  t[s]  change  continuity  out/in  dp[m2/s2]  ms/step  phase".into());
    let t1 = bugis::web_demo::platform::now_s();
    let mut last_progress = None;
    while e.steps < max_steps {
        e.serve(Request::Step { id: 1, steps: 5 }, &mut |ev| last = Some(ev));
        match &last {
            Some(Event::Progress(p)) => {
                out.push(format!(
                    "{} {:.3} {:.3e} {:.1e} {:.9} {:.6e} {:.1} {}",
                    p.steps, p.time_s, p.change, p.continuity, p.outlet_over_inlet, p.dp, p.ms_per_step, p.phase
                ));
                last_progress = Some(p.clone());
                if p.steady {
                    break;
                }
            }
            Some(Event::Error(m)) => return Err(m.clone()),
            _ => return Err("no progress".into()),
        }
    }
    let wall = bugis::web_demo::platform::now_s() - t1;
    if let (Some(p), Shape::DoublePipe { .. }) = (&last_progress, design.shape) {
        let x = &p.exch;
        out.push(format!(
            "# exchanger: Thot_out={:.4} Tcold_out={:.4} Qhot={:.6} Qcold={:.6} eps={:.5} eps_ntu={:.5} UA={:.5} NTU={:.4}",
            x[Exch::T_HOT_OUT],
            x[Exch::T_COLD_OUT],
            x[Exch::Q_HOT],
            x[Exch::Q_COLD],
            x[Exch::EPS],
            x[Exch::EPS_NTU],
            x[Exch::UA],
            x[Exch::NTU]
        ));
    }
    if let Shape::Coil { .. } = design.shape {
        out.insert(1, format!("# grid (n, nr, layers, cells): {:?}", coil_grid(&design).map(|g| (g.n, g.nr, g.n_layers, g.n_cells()))));
    }
    if let (Some(Case::Pipe(s, flow)), Shape::Cylinder { centre, length, .. }) = (&e.case, design.shape) {
        use outram_foam_appbuilder_lib::cases::laminar_pipe::{centreline_velocity, pressure_gradient};
        use uom::si::acceleration::meter_per_second_squared;
        let z0 = centre[2] - 0.5 * length;
        let uc = centreline_velocity(s, flow, (centre[0], centre[1]), z0 + 0.75 * length, 0.05 * length).get::<meter_per_second>();
        let g = pressure_gradient(s, z0 + 0.65 * length, z0 + 0.85 * length);
        out.push(format!(
            "# pipe (u_c/2U at 0.75 L, -dp/dz/HP over 0.65-0.85 L, f Re): uc={:.4} dpdz={:.4} fRe={:.2}",
            uc / (2.0 * U_IN),
            g.get::<meter_per_second_squared>() / flow.hagen_poiseuille_gradient().get::<meter_per_second_squared>(),
            flow.friction_factor(g) * flow.reynolds()
        ));
        // gh:#1202: the profile and wall shear over the same slab, for both meshes.
        use outram_foam_appbuilder_lib::cases::laminar_pipe::{profile_rms, wall_shear_stress};
        let rms = profile_rms(s, flow, (centre[0], centre[1]), z0 + 0.65 * length, z0 + 0.85 * length);
        let tau = wall_shear_stress(s, z0 + 0.65 * length, z0 + 0.85 * length) / flow.hagen_poiseuille_wall_shear();
        out.push(format!("# pipe profile (RMS of (u_z - u_HP)/2U, tau_w/HP over 0.65-0.85 L): rms={rms:.4} tau={tau:.4}"));
        if let Some(p) = &last_progress {
            let bins: Vec<String> = p.profile.chunks(2).map(|q| format!("{:.3}:{:.3}", q[0], q[1])).collect();
            out.push(format!("# binned profile at 0.75 L (r/R:u/U): {}", bins.join(" ")));
        }
    }
    if let (Some(p), Shape::Coil { .. }) = (&last_progress, design.shape) {
        let c = &p.coil;
        out.push(format!(
            "# coil: De={:.3} fRe64={:.5} fRe64_before={:.5} MG={:.5} peak_n={:.4} max_sec={:.4} core_un={:.4} pair={:.4}",
            c[CoilNo::DEAN],
            c[CoilNo::F_RE_64],
            c[CoilNo::F_RE_64_BEFORE],
            c[CoilNo::MISHRA_GUPTA],
            c[CoilNo::PEAK_N],
            c[CoilNo::MAX_SECONDARY],
            c[CoilNo::CORE_UN],
            c[CoilNo::PAIR]
        ));
    }
    out.push(format!("# {} steps, {:.2} s wall in the solve", e.steps, wall));
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn small_pipe() -> Design {
        Design { target_cells: 2000, ..Design::default() }
    }

    /// Diagnostic for the V&V record (gh:#1202), `--ignored`: the discrete
    /// geometry and the two-point Laplacian of φ = x² + y² (∇²φ = 4 exactly)
    /// on the O-grid and on the cfMesh dual, per interior cell: cell closure
    /// |Σ S_f| / Σ|S_f|, Gauss volume Σ x_f·S_f / 3 against the cell volume,
    /// and the uncorrected (`|S|²/(S·d)`) and orthogonal (`|S|/|d|`) Laplacians
    /// against 4. Prints a table; asserts nothing.
    #[test]
    #[ignore]
    fn diagnose_dual_mesh_geometry_and_laplacian() {
        use outram_foam_basic_lib::primitives::Vector3;
        fn worst(cells: &[usize], f: impl Fn(usize) -> f64) -> f64 {
            cells.iter().map(|&c| f(c).abs()).fold(0.0_f64, f64::max)
        }
        fn mean(cells: &[usize], f: impl Fn(usize) -> f64) -> f64 {
            cells.iter().map(|&c| f(c)).sum::<f64>() / cells.len() as f64
        }
        let cases: Vec<(&str, PolyMesh)> = vec![
            ("O-grid 7920", mesh_design(&Design { target_cells: 8000, ..Design::default() }).unwrap().0),
            ("dual h=4 mm bare", mesh_unstructured(0.01, 0.2, [0.0, 0.0, 0.1], [Tag::Inlet, Tag::Outlet, Tag::Wall], &TetDualSpec { cell_size: 0.004, n_layers: 0, first_layer: 3e-4, expansion: 1.2, ..TetDualSpec::PRE_GH1298 }).unwrap().0),
        ];
        for (name, poly) in cases {
            let m = poly.to_fv_mesh().unwrap();
            let n = m.n_cells;
            let (mut ssum, mut sabs, mut xs) = (vec![Vector3::ZERO; n], vec![0.0; n], vec![0.0; n]);
            let (mut lu, mut lo) = (vec![0.0; n], vec![0.0; n]);
            let mut boundary = vec![false; n];
            let phi = |p: Vector3| p.x * p.x + p.y * p.y;
            for f in 0..m.face_area_vectors.len() {
                let s = m.face_area_vectors[f];
                let o = m.owner[f];
                let xf = m.face_centres[f];
                ssum[o] = ssum[o] + s;
                sabs[o] += s.mag();
                xs[o] += xf.dot(s) / 3.0;
                if f < m.n_internal_faces {
                    let nb = m.neighbour[f];
                    ssum[nb] = ssum[nb] - s;
                    sabs[nb] += s.mag();
                    xs[nb] -= xf.dot(s) / 3.0;
                    let d = m.cell_centres[nb] - m.cell_centres[o];
                    let dphi = phi(m.cell_centres[nb]) - phi(m.cell_centres[o]);
                    let fu = dphi * s.mag() * s.mag() / s.dot(d);
                    let fo = dphi * s.mag() / d.mag();
                    lu[o] += fu;
                    lu[nb] -= fu;
                    lo[o] += fo;
                    lo[nb] -= fo;
                } else {
                    boundary[o] = true;
                }
            }
            let interior: Vec<usize> = (0..n).filter(|&c| !boundary[c]).collect();
            let ic = &interior;
            println!(
                "{name}: {} cells ({} interior); closure max {:.2e}; Gauss volume max rel {:.2e}; uncorrected lap/4: mean {:.4}, max |err| {:.3}; orthogonal lap/4: mean {:.4}, max |err| {:.3}",
                n,
                interior.len(),
                worst(ic, |c| ssum[c].mag() / sabs[c]),
                worst(ic, |c| xs[c] / m.cell_volumes[c] - 1.0),
                mean(ic, |c| lu[c] / m.cell_volumes[c] / 4.0),
                worst(ic, |c| lu[c] / m.cell_volumes[c] / 4.0 - 1.0),
                mean(ic, |c| lo[c] / m.cell_volumes[c] / 4.0),
                worst(ic, |c| lo[c] / m.cell_volumes[c] / 4.0 - 1.0),
            );
        }
    }

    /// Diagnostic for the V&V record (gh:#1202), `--ignored`: the checks the
    /// maintainer listed, on the O-grid and on cfMesh duals (with and without a
    /// wall layer). Predictions written before the first run (2026-11-04) are
    /// in the record. Per mesh it prints:
    /// - Σ V against the polygon prism πR²L·(n/2π)sin(2π/n) is not known for
    ///   the dual, so against πR²L, and the wall area against 2πRL;
    /// - face non-planarity: max over faces of the largest vertex distance
    ///   from the plane through the face centre with the face normal, over
    ///   √|S|;
    /// - orientation: internal faces with S·(C_N − C_P) ≤ 0, boundary faces
    ///   with S·(C_f − C_P) ≤ 0;
    /// - a manufactured Poisson problem −∇²u = 4, u = 0 on the walls,
    ///   zero-gradient at the ends (exact u = R_w² − r² for a circular wall),
    ///   solved with the port's `laplacian_with_delta(NonOrthogonal)` plus the
    ///   explicit correction iterated 20 times (and once without it): the fit
    ///   u = a − b r² over cells with r < 0.7 R in the middle half, where b
    ///   must be 1 whatever the wall's exact radius, and the RMS of
    ///   u − (a − r²) over all cells relative to a.
    #[test]
    #[ignore]
    fn diagnose_geometry_orientation_and_poisson() {
        use outram_foam_basic_lib::prelude::*;
        let (r, l) = (0.01, 0.2);
        let spec = |h: f64, n: usize| TetDualSpec { cell_size: h, n_layers: n, first_layer: 3e-4, expansion: 1.2, ..TetDualSpec::PRE_GH1298 };
        let tags = [Tag::Inlet, Tag::Outlet, Tag::Wall];
        let cases: Vec<(&str, PolyMesh)> = vec![
            ("O-grid 7920", mesh_design(&Design { target_cells: 8000, ..Design::default() }).unwrap().0),
            ("dual h=4 mm, no layer", mesh_unstructured(r, l, [0.0, 0.0, 0.1], tags, &spec(0.004, 0)).unwrap().0),
            ("dual h=6 mm, 1 layer", mesh_unstructured(r, l, [0.0, 0.0, 0.1], tags, &spec(0.006, 1)).unwrap().0),
        ];
        for (name, poly) in cases {
            let m = Arc::new(poly.to_fv_mesh().unwrap());
            // 1. Volume, wall area, non-planarity.
            let vol: f64 = m.cell_volumes.iter().sum();
            let wall: f64 = m.patches.iter().filter(|p| p.name == "walls").map(|p| (0..p.size).map(|i| m.face_areas[p.start + i]).sum::<f64>()).sum();
            let mut nonplanar = 0.0_f64;
            for (fi, f) in poly.faces.iter().enumerate() {
                let n = m.face_area_vectors[fi] * (1.0 / m.face_areas[fi].max(1e-300));
                let dev = f.verts.iter().map(|&v| (poly.points[v] - m.face_centres[fi]).dot(n).abs()).fold(0.0_f64, f64::max);
                nonplanar = nonplanar.max(dev / m.face_areas[fi].sqrt().max(1e-300));
            }
            // 2. Orientation.
            let bad_int = (0..m.n_internal_faces).filter(|&f| m.face_area_vectors[f].dot(m.cell_centres[m.neighbour[f]] - m.cell_centres[m.owner[f]]) <= 0.0).count();
            let bad_bnd = (m.n_internal_faces..m.face_area_vectors.len()).filter(|&f| m.face_area_vectors[f].dot(m.face_centres[f] - m.cell_centres[m.owner[f]]) <= 0.0).count();
            println!(
                "{name}: {} cells; V/(pi R^2 L) {:.5}; wall/(2 pi R L) {:.5}; max non-planarity {:.3e}; misoriented internal {bad_int}, boundary {bad_bnd}",
                m.n_cells,
                vol / (std::f64::consts::PI * r * r * l),
                wall / (2.0 * std::f64::consts::PI * r * l),
                nonplanar
            );
            // 3. Poisson −∇²u = 4.
            for corrected in [false, true] {
                let (b, a_r2, rms, _) = manufactured_poisson(&m, r, if corrected { 20 } else { 1 }, corrected, true);
                println!(
                    "  Poisson ({}): fit u = a - b r^2: b {:.4} (exact 1), a/R^2 {:.4}; RMS of u - fit over all cells / a {:.4}",
                    if corrected { "corrected, 20 passes" } else { "uncorrected" },
                    b,
                    a_r2,
                    rms
                );
            }
        }
    }


    /// The gh:#1202 manufactured Poisson problem on a frozen mesh: −∇²u = 4
    /// with u = 0 on `walls` and zero gradient on the ends (exact u = R² − r²
    /// on a circular section), solved once uncorrected or with `passes`
    /// deferred non-orthogonal correction passes. Returns the fit u = a − b r²
    /// over the core (r < 0.7 R, 0.05 m about z = 0.1 m) as (b, a/R², RMS of
    /// u − fit over all cells / a) and the max |Δu| of each pass.
    fn manufactured_poisson(m: &Arc<outram_foam_basic_lib::prelude::FvMesh>, r: f64, passes: usize, corrected: bool, verbose: bool) -> (f64, f64, f64, Vec<f64>) {
        use outram_foam_appbuilder_lib::cases::laminar_pipe::fit_line;
        use outram_foam_appbuilder_lib::solvers::non_orthogonal;
        use outram_foam_basic_lib::prelude::*;
        let vol: f64 = m.cell_volumes.iter().sum();
        let mut changes = Vec::new();
        let bcs = |m: &FvMesh| -> Vec<PatchField<f64>> {
            m.patches.iter().map(|p| if p.name == "walls" { PatchField::fixed_value(p.size, 0.0) } else { PatchField::zero_gradient(p.size) }).collect()
        };
        let mut u = VolScalarField::new("u", m.clone(), Field::new(vec![0.0; m.n_cells]), bcs(&m));
        // Interpolated, so the boundary faces carry Γ = 1 too (`SurfaceScalarField::uniform` sets
        // the internal faces only; the first run had no Dirichlet term at all).
        let gamma = fvc::interpolate(&VolScalarField::uniform("g", m.clone(), 1.0));
        for _ in 0..passes {
            let mut eqn = fvm::laplacian_with_delta(&gamma, &u, fvm::DeltaCoeff::NonOrthogonal);
            for c in 0..m.n_cells {
                eqn.source[c] += 4.0 * m.cell_volumes[c];
            }
            if corrected {
                let corr = non_orthogonal::scalar_face_correction(&gamma, &fvc::grad(&u), &m);
                non_orthogonal::add_to_source_scalar(eqn.source.as_mut_slice(), &corr, &m);
            }
            let (sol, perf) = eqn.solve_cg_with_guess("u", &u, SolverSettings { tolerance: 1e-12, max_iter: 20_000 });
            let umax = sol.internal.as_slice().iter().fold(0.0_f64, |a, v| a.max(v.abs()));
            if !corrected && verbose {
                let (gs, gperf) = eqn.solve("u", SolverSettings { tolerance: 1e-10, max_iter: 200_000 });
                let gmax = gs.internal.as_slice().iter().fold(0.0_f64, |a, v| a.max(v.abs()));
                let nfix: usize = u.boundary.iter().filter(|b| matches!(b.bc, BoundaryCondition::FixedValue(_))).map(|b| b.values.len()).sum();
                let offsum = eqn.ldu.upper.iter().sum::<f64>();
                println!(
                    "  (CG: {} it, residual {:.2e}, max |u| {umax:.3e}; GS: {} it, residual {:.2e}, max |u| {gmax:.3e}; diag sum {:.3e}, 2*upper sum {:.3e}, fixed faces {nfix})",
                    perf.n_iterations,
                    perf.final_residual,
                    gperf.n_iterations,
                    gperf.final_residual,
                    eqn.ldu.diag.iter().sum::<f64>(),
                    2.0 * offsum
                );
            }
            let mut next = VolScalarField::new("u", m.clone(), sol.internal.clone(), bcs(&m));
            for (pi, p) in m.patches.iter().enumerate() {
                if p.name != "walls" {
                    for fi in 0..p.size {
                        next.boundary[pi].values[fi] = next.internal[m.owner[p.start + fi]];
                    }
                }
            }
            let change = next.internal.as_slice().iter().zip(u.internal.as_slice()).fold(0.0_f64, |a, (x, y)| a.max((x - y).abs()));
            changes.push(change);
            u = next;
        }
        let r2 = |c: usize| m.cell_centres[c].x.powi(2) + m.cell_centres[c].y.powi(2);
        let pts: Vec<(f64, f64, f64)> = (0..m.n_cells)
            .filter(|&c| r2(c) < 0.49 * r * r && (m.cell_centres[c].z - 0.1).abs() < 0.05)
            .map(|c| (r2(c), u.internal[c], m.cell_volumes[c]))
            .collect();
        let (a, b) = fit_line(&pts);
        let rms = ((0..m.n_cells).map(|c| (u.internal[c] - (a + b * r2(c))).powi(2) * m.cell_volumes[c]).sum::<f64>() / vol).sqrt() / a;
        let r2 = |c: usize| m.cell_centres[c].x.powi(2) + m.cell_centres[c].y.powi(2);
        let pts: Vec<(f64, f64, f64)> = (0..m.n_cells)
            .filter(|&c| r2(c) < 0.49 * r * r && (m.cell_centres[c].z - 0.1).abs() < 0.05)
            .map(|c| (r2(c), u.internal[c], m.cell_volumes[c]))
            .collect();
        let (a, b) = fit_line(&pts);
        let rms = ((0..m.n_cells).map(|c| (u.internal[c] - (a + b * r2(c))).powi(2) * m.cell_volumes[c]).sum::<f64>() / vol).sqrt() / a;
        (-b, a / (r * r), rms, changes)
    }

    /// Diagnostic (gh:#1241 item 2), `--ignored`: why 3 back-off layers
    /// diverge on the per-point duals although the mesh passes the layer
    /// checks. Per mesh (`FOAM_DIAG_CASES` filters by label): the frozen
    /// mesh's thin cells (Δ = V / max |S_f|) and their local viscous number
    /// ν Δt / Δ² and Courant number 2U Δt / Δ at the run's Δt; the
    /// manufactured Poisson problem uncorrected and with 20 correction
    /// passes (the change of each pass); then the flow, one step at a time,
    /// until max |U| exceeds 20 U or 300 steps: the step it happens and the
    /// five cells with the largest |U| then, with their quality.
    #[test]
    #[ignore]
    fn diagnose_three_layer_divergence() {
        use outram_park_fork_cfmesh::dual::{DualKind, FeaturePointRule};
        use outram_blender::foam_mesh::LayerQualityStrategy as S;
        let spec = |h: f64, n: usize, dual: DualKind| Design {
            mesh: PipeMesh::Unstructured(TetDualSpec { cell_size: h, n_layers: n, first_layer: 3e-4, expansion: 1.2, dual, feature_points: FeaturePointRule::Corners, layer_quality: S::BackOffOnly, route: CfMeshRoute::PreGh1298 }),
            ..Design::default()
        };
        let cases = [
            ("centre h5 N3", spec(0.005, 3, DualKind::CellCentre)),
            ("point h5 N3", spec(0.005, 3, DualKind::MedianPerPoint)),
            ("centre h5 N2 (runs)", spec(0.005, 2, DualKind::CellCentre)),
            ("centre h4 N3", spec(0.004, 3, DualKind::CellCentre)),
            ("centre h6 N3", spec(0.006, 3, DualKind::CellCentre)),
        ];
        let filter = std::env::var("FOAM_DIAG_CASES").ok();
        // Ablations (FOAM_DIAG_ABL, comma list; default `base`): `upwind`
        // (div(phi,U) Gauss upwind), `nolimit` (grad(U) Gauss linear, no
        // cellLimited), `linear` (div Gauss linear), `dt4` (Δt / 4),
        // `uncorrected` (laplacian), `nonorth0` (no corrector).
        let abls: Vec<String> = std::env::var("FOAM_DIAG_ABL").map_or(vec!["base".into()], |v| v.split(',').map(String::from).collect());
        for (name, d) in cases {
            if filter.as_deref().is_some_and(|f| !f.split(',').any(|x| name.starts_with(x))) {
                continue;
            }
            for abl in &abls {
            let mut e = Engine::default();
            e.serve(Request::Mesh { id: 1, design: d, probe: 0.75 }, &mut |_| {});
            e.serve(Request::Start { id: 1, reynolds: 100.0, mode: Mode::Steady }, &mut |_| {});
            let Some(Case::Pipe(s, flow)) = e.case.as_mut() else {
                println!("{name}: no pipe case");
                continue;
            };
            {
                use outram_foam_appbuilder_lib::io::fv_schemes::{DivScheme, LaplacianScheme};
                match abl.as_str() {
                    "upwind" => s.schemes.default_div = DivScheme::GaussUpwind,
                    "linear" => s.schemes.default_div = DivScheme::GaussLinear,
                    "nolimit" => s.schemes.grad_u = None,
                    "dt4" => s.control.delta_t *= 0.25,
                    "dt100" => s.control.delta_t *= 0.01,
                    "dt10" => s.control.delta_t *= 0.1,
                    "dt30" => s.control.delta_t /= 30.0,
                    "uncorrected" => s.schemes.default_laplacian = LaplacianScheme::GaussLinearUncorrected,
                    "nonorth0" => s.solution.pimple.n_non_orthogonal_correctors = 0,
                    _ => {}
                }
            }
            let name = format!("{name} [{abl}]");
            let m = s.mesh.clone();
            let dt = s.control.delta_t;
            let nu = flow.nu.get::<uom::si::kinematic_viscosity::square_meter_per_second>();
            // Thin dimension, wall cells, per-cell quality.
            let mut smax = vec![0.0_f64; m.n_cells];
            let mut no_max = vec![0.0_f64; m.n_cells];
            let mut w_min = vec![0.5_f64; m.n_cells];
            let mut vr_min = vec![1.0_f64; m.n_cells];
            let mut wall_cell = vec![false; m.n_cells];
            for f in 0..m.face_areas.len() {
                let o = m.owner[f];
                smax[o] = smax[o].max(m.face_areas[f]);
                if f < m.n_internal_faces {
                    let n = m.neighbour[f];
                    smax[n] = smax[n].max(m.face_areas[f]);
                    let dv = m.cell_centres[n] - m.cell_centres[o];
                    let sf = m.face_area_vectors[f];
                    let no = (dv.dot(sf) / (dv.mag() * sf.mag())).clamp(-1.0, 1.0).acos().to_degrees();
                    let d_o = sf.dot(m.face_centres[f] - m.cell_centres[o]).abs();
                    let d_n = sf.dot(m.cell_centres[n] - m.face_centres[f]).abs();
                    let w = d_o.min(d_n) / (d_o + d_n);
                    let vr = m.cell_volumes[o].min(m.cell_volumes[n]) / m.cell_volumes[o].max(m.cell_volumes[n]);
                    for c in [o, n] {
                        no_max[c] = no_max[c].max(no);
                        w_min[c] = w_min[c].min(w);
                        vr_min[c] = vr_min[c].min(vr);
                    }
                }
            }
            for p in m.patches.iter().filter(|p| p.name == "walls") {
                for i in 0..p.size {
                    wall_cell[m.owner[p.start + i]] = true;
                }
            }
            let thin: Vec<f64> = (0..m.n_cells).map(|c| m.cell_volumes[c] / smax[c]).collect();
            let visc: Vec<f64> = thin.iter().map(|t| nu * dt / (t * t)).collect();
            let mut order: Vec<usize> = (0..m.n_cells).collect();
            order.sort_by(|&a, &b| visc[b].partial_cmp(&visc[a]).unwrap());
            let rank_of = |c: usize| order.iter().position(|&x| x == c).unwrap();
            println!(
                "{name}: {} cells, dt {dt:.3e} s; thin min {:.3e} m; local viscous number max {:.1} (p99 {:.1}); local Co max {:.2}; max NO {:.1}, min weight {:.3}, min vol ratio {:.4}",
                m.n_cells,
                thin.iter().cloned().fold(f64::MAX, f64::min),
                visc[order[0]],
                visc[order[m.n_cells / 100]],
                2.0 * U_IN * dt / thin.iter().cloned().fold(f64::MAX, f64::min),
                no_max.iter().cloned().fold(0.0, f64::max),
                w_min.iter().cloned().fold(1.0, f64::min),
                vr_min.iter().cloned().fold(1.0, f64::min),
            );
            for (passes, corrected) in [(1, false), (20, true)] {
                let (b, a, rms, ch) = manufactured_poisson(&m, 0.01, passes, corrected, false);
                let chs: Vec<String> = ch.iter().map(|x| format!("{x:.1e}")).collect();
                println!("  Poisson {}: b {b:.4} a/R2 {a:.4} rms {rms:.4}; pass changes {}", if corrected { "corrected" } else { "uncorrected" }, chs.join(" "));
            }
            // FOAM_DIAG_CELL_AT=r_mm,z_mm: every face of the cell nearest that point.
            if let Some(at) = std::env::var("FOAM_DIAG_CELL_AT").ok() {
                let v: Vec<f64> = at.split(',').filter_map(|x| x.parse().ok()).collect();
                let c = (0..m.n_cells)
                    .min_by(|&a, &b| {
                        let da = (m.cell_centres[a].x.hypot(m.cell_centres[a].y) * 1e3 - v[0]).abs() + (m.cell_centres[a].z * 1e3 - v[1]).abs();
                        let db = (m.cell_centres[b].x.hypot(m.cell_centres[b].y) * 1e3 - v[0]).abs() + (m.cell_centres[b].z * 1e3 - v[1]).abs();
                        da.partial_cmp(&db).unwrap()
                    })
                    .unwrap();
                let cc = m.cell_centres[c];
                println!("  cell {c} at r {:.3} z {:.3} mm, V {:.3e}, thin {:.3e}:", cc.x.hypot(cc.y) * 1e3, cc.z * 1e3, m.cell_volumes[c], thin[c]);
                for f in 0..m.face_areas.len() {
                    let (o, n) = (m.owner[f], if f < m.n_internal_faces { Some(m.neighbour[f]) } else { None });
                    if o != c && n != Some(c) {
                        continue;
                    }
                    let patch = m.patches.iter().find(|p| f >= p.start && f < p.start + p.size).map_or("internal", |p| p.name.as_str());
                    let sgn = if o == c { 1.0 } else { -1.0 };
                    let pyr = sgn * m.face_area_vectors[f].dot(m.face_centres[f] - cc) / 3.0;
                    let other = match n { Some(n) if o == c => Some(n), Some(_) => Some(o), None => None };
                    println!(
                        "    face {f} ({patch}) |S| {:.2e}, pyramid/V {:.3}, centre r {:.3} z {:.3} mm{}",
                        m.face_areas[f],
                        pyr / m.cell_volumes[c],
                        m.face_centres[f].x.hypot(m.face_centres[f].y) * 1e3,
                        m.face_centres[f].z * 1e3,
                        other.map_or(String::new(), |x| format!(", to cell {x} (thin {:.2e}, V {:.2e}, wall {})", thin[x], m.cell_volumes[x], wall_cell[x]))
                    );
                }
            }
            let describe = |c: usize, umag: f64| {
                let x = m.cell_centres[c];
                format!(
                    "cell {c}: |U|/U {:.2e}, r {:.3} z {:.1} mm, wall {}, thin {:.3e} m, visc {:.1} (rank {}), NO {:.1}, weight {:.3}, volRatio {:.4}",
                    umag / U_IN,
                    x.x.hypot(x.y) * 1e3,
                    x.z * 1e3,
                    wall_cell[c],
                    thin[c],
                    visc[c],
                    rank_of(c),
                    no_max[c],
                    w_min[c],
                    vr_min[c]
                )
            };
            let mut history: Vec<String> = Vec::new();
            let mut first_over = false;
            // `co1`, `co4`: OpenFOAM's adjustTimeStep at that maxCo (gh:#1241).
            let max_co: Option<f64> = abl.strip_prefix("co").and_then(|v| v.parse().ok());
            let max_dt = s.control.delta_t;
            let n_steps = std::env::var("FOAM_DIAG_STEPS").ok().and_then(|v| v.parse().ok()).unwrap_or(300usize);
            for step in 1..=n_steps {
                if let Some(mc) = max_co {
                    let (co, _) = courant_numbers(s);
                    s.control.delta_t = adjusted_delta_t(s.control.delta_t, co, mc, max_dt, step == 1);
                }
                let r = flow.step(s);
                let um: Vec<f64> = s.u.internal.as_slice().iter().map(|v| v.mag()).collect();
                let (cmax, umax) = um.iter().enumerate().fold((0, 0.0_f64), |a, (c, &v)| if v.is_nan() || v > a.1 { (c, if v.is_nan() { f64::INFINITY } else { v }) } else { a });
                if step <= 5 || step % 10 == 0 {
                    history.push(format!("{step}:{:.2e}@{cmax}", umax / U_IN));
                }
                // OpenFOAM's Courant number (CourantNo.H): half dt sum|phi_f| / V.
                if step == 1 || step == 5 || (umax > 3.0 * U_IN && !first_over) {
                    let mut sum = vec![0.0_f64; m.n_cells];
                    for f in 0..m.n_internal_faces {
                        let a = s.phi.internal[f].abs();
                        sum[m.owner[f]] += a;
                        sum[m.neighbour[f]] += a;
                    }
                    for (pi, p) in m.patches.iter().enumerate() {
                        for i in 0..p.size {
                            sum[m.owner[p.start + i]] += s.phi.boundary[pi].values[i].abs();
                        }
                    }
                    let dt_now = s.control.delta_t;
                    let co: Vec<f64> = (0..m.n_cells).map(|c| 0.5 * dt_now * sum[c] / m.cell_volumes[c]).collect();
                    let (cm, cv) = co.iter().enumerate().fold((0, 0.0_f64), |a, (c, &v)| if v > a.1 { (c, v) } else { a });
                    let mut sorted = co.clone();
                    sorted.sort_by(|a, b| a.partial_cmp(b).unwrap());
                    println!(
                        "  step {step}: Courant max {cv:.2} at {}; p99 {:.2}, mean {:.3}; Co of cell {cmax} {:.2}",
                        describe(cm, um[cm]),
                        sorted[sorted.len() * 99 / 100],
                        sorted.iter().sum::<f64>() / sorted.len() as f64,
                        co[cmax]
                    );
                }
                if umax > 3.0 * U_IN && !first_over {
                    first_over = true;
                    println!("  first above 3 U at step {step}: {}", describe(cmax, umax));
                    for f in 0..m.n_internal_faces {
                        let (o, n) = (m.owner[f], m.neighbour[f]);
                        if o == cmax || n == cmax {
                            let other = if o == cmax { n } else { o };
                            let sf = m.face_area_vectors[f];
                            let d_o = sf.dot(m.face_centres[f] - m.cell_centres[o]).abs();
                            let d_n = sf.dot(m.cell_centres[n] - m.face_centres[f]).abs();
                            println!("    face {f} to {}: weight {:.3}, |S| {:.2e}", describe(other, um[other]), d_o.min(d_n) / (d_o + d_n), m.face_areas[f]);
                        }
                    }
                }
                if r.is_err() || !umax.is_finite() || umax > 20.0 * U_IN {
                    println!("  blow-up at step {step} ({}): max |U| at {}", if r.is_err() { "solver error" } else { "|U| > 20 U" }, describe(cmax, umax));
                    let mut idx: Vec<usize> = (0..m.n_cells).filter(|&c| um[c].is_finite()).collect();
                    idx.sort_by(|&a, &b| um[b].partial_cmp(&um[a]).unwrap());
                    for &c in idx.iter().take(5) {
                        println!("    {}", describe(c, um[c]));
                    }
                    break;
                }
                if step == n_steps {
                    println!("  no blow-up in {n_steps} steps (dt now {:.3e} s); max |U| {}", s.control.delta_t, describe(cmax, umax));
                }
                if step == n_steps || r.is_err() || !umax.is_finite() || umax > 20.0 * U_IN {
                    // The worst cell and its neighbours: U (radial, axial), p, net flux.
                    let mut net = vec![0.0_f64; m.n_cells];
                    let mut absf = vec![0.0_f64; m.n_cells];
                    for f in 0..m.n_internal_faces {
                        net[m.owner[f]] += s.phi.internal[f];
                        net[m.neighbour[f]] -= s.phi.internal[f];
                        absf[m.owner[f]] += s.phi.internal[f].abs();
                        absf[m.neighbour[f]] += s.phi.internal[f].abs();
                    }
                    for (pi, p) in m.patches.iter().enumerate() {
                        for i in 0..p.size {
                            net[m.owner[p.start + i]] += s.phi.boundary[pi].values[i];
                            absf[m.owner[p.start + i]] += s.phi.boundary[pi].values[i].abs();
                        }
                    }
                    let show = |c: usize| {
                        let x = m.cell_centres[c];
                        let rr = x.x.hypot(x.y).max(1e-300);
                        let u = s.u.internal[c];
                        let ur = (u.x * x.x + u.y * x.y) / rr;
                        format!(
                            "cell {c} (r {:.3} z {:.2} mm, V {:.2e}, wall {}): u_r/U {:.2}, u_z/U {:.2}, |U|/U {:.2}, p {:.3e}, net flux / sum|flux| {:.2e}",
                            rr * 1e3, x.z * 1e3, m.cell_volumes[c], wall_cell[c], ur / U_IN, u.z / U_IN, u.mag() / U_IN, s.p.internal[c], net[c] / absf[c].max(1e-300)
                        )
                    };
                    let c0 = cmax.min(m.n_cells - 1);
                    println!("  worst: {}", show(c0));
                    for f in 0..m.n_internal_faces {
                        let (o, n) = (m.owner[f], m.neighbour[f]);
                        if o == c0 || n == c0 {
                            let other = if o == c0 { n } else { o };
                            println!("    via face {f} (phi {:.2e}): {}", s.phi.internal[f], show(other));
                        }
                    }
                }
            }
            println!("  history (step:max|U|/U@cell): {}", history.join(" "));
            }
        }
    }

    /// Diagnostic (gh:#1202), `--ignored`: does the unstructured miss depend
    /// on Re (convection) or the momentum interpolation? The 6 713-cell dual
    /// at Re = 20 and 100, and the bare h 4 mm dual at Re = 20; prints the
    /// headless pipe lines.
    #[test]
    #[ignore]
    fn diagnose_unstructured_reynolds_dependence() {
        for (h, n, re) in [(0.006, 1, 20.0), (0.004, 0, 20.0), (0.006, 1, 100.0)] {
            let d = Design { mesh: PipeMesh::Unstructured(TetDualSpec { cell_size: h, n_layers: n, first_layer: 3e-4, expansion: 1.2, ..TetDualSpec::PRE_GH1298 }), ..Design::default() };
            let lines = headless(d, re, Mode::Steady, 2000).unwrap_or_else(|e| vec![e]);
            println!("h {h} layers {n} Re {re}:");
            for l in lines.iter().filter(|l| l.starts_with("# pipe") || l.starts_with("# binned") || l.contains("steps,")) {
                println!("  {l}");
            }
        }
        // Development along the pipe (check 5): u_c/2U against z on the dual
        // and on the O-grid, Re = 100, to steady; and the dual with central
        // (`Gauss linear`) convection instead of upwind, to test upwind's
        // cross-stream false diffusion on faces oblique to the flow.
        use outram_foam_appbuilder_lib::cases::laminar_pipe::{centreline_velocity, pressure_gradient};
        use outram_foam_appbuilder_lib::io::fv_schemes::DivScheme;
        use uom::si::acceleration::meter_per_second_squared;
        let dual6 = Design { mesh: PipeMesh::Unstructured(TetDualSpec { cell_size: 0.006, n_layers: 1, first_layer: 3e-4, expansion: 1.2, ..TetDualSpec::PRE_GH1298 }), ..Design::default() };
        let dual4 = Design { mesh: PipeMesh::Unstructured(TetDualSpec { cell_size: 0.004, n_layers: 0, first_layer: 3e-4, expansion: 1.2, ..TetDualSpec::PRE_GH1298 }), ..Design::default() };
        for (name, d, linear) in [
            ("dual h 6 mm, 1 layer, upwind", dual6, false),
            ("dual h 6 mm, 1 layer, Gauss linear", dual6, true),
            ("dual h 4 mm, no layer, Gauss linear", dual4, true),
            ("O-grid 7920, upwind", Design { target_cells: 8000, ..Design::default() }, false),
            ("O-grid 7920, Gauss linear", Design { target_cells: 8000, ..Design::default() }, true),
        ] {
            let mut e = Engine::default();
            e.serve(Request::Mesh { id: 1, design: d, probe: 0.75 }, &mut |_| {});
            e.serve(Request::Start { id: 1, reynolds: 100.0, mode: Mode::Steady }, &mut |_| {});
            if let (true, Some(Case::Pipe(s, _))) = (linear, e.case.as_mut()) {
                s.schemes.default_div = DivScheme::GaussLinear;
            }
            let mut steady = false;
            for _ in 0..400 {
                e.serve(Request::Step { id: 1, steps: 5 }, &mut |ev| {
                    if let Event::Progress(p) = ev {
                        steady = p.steady;
                    }
                });
                if steady {
                    break;
                }
            }
            if let Some(Case::Pipe(s, flow)) = &e.case {
                let row: Vec<String> = [0.05, 0.1, 0.2, 0.3, 0.45, 0.6, 0.75, 0.9]
                    .iter()
                    .map(|f| format!("{f}:{:.3}", centreline_velocity(s, flow, (0.0, 0.0), f * 0.2, 0.01).get::<meter_per_second>() / (2.0 * U_IN)))
                    .collect();
                let g = pressure_gradient(s, 0.13, 0.17).get::<meter_per_second_squared>() / flow.hagen_poiseuille_gradient().get::<meter_per_second_squared>();
                println!("{name}: steady {steady} after {} steps; dp/dz/HP {g:.4}; u_c/2U at z/L: {}", e.steps, row.join(" "));
            }
        }
    }

    /// Diagnostic (gh:#1202), `--ignored`: the unstructured pipe at Re = 100
    /// over a refinement series, with and without a wall layer, under the
    /// convection scheme `FOAM_DIV_U` names (`linear`, the default here;
    /// `linearUpwind`; `linearUpwindLimited`: `linearUpwind grad(U)` with
    /// `grad(U) cellLimited Gauss linear 1`) and the meshes `FOAM_SWEEP`
    /// names (`bare`, `layered` or both). Prints the headless pipe lines.
    #[test]
    #[ignore]
    fn diagnose_unstructured_linear_refinement() {
        use outram_foam_appbuilder_lib::io::fv_schemes::{DivScheme, GradScheme};
        let which = std::env::var("FOAM_DIV_U").unwrap_or_else(|_| "linear".into());
        let (div, grad) = match which.as_str() {
            "linearUpwind" => (DivScheme::GaussLinearUpwind("grad(U)".into()), None),
            "linearUpwindLimited" => (DivScheme::GaussLinearUpwind("grad(U)".into()), Some(GradScheme::CellLimitedGaussLinear(1.0))),
            "upwind" => (DivScheme::GaussUpwind, None),
            _ => (DivScheme::GaussLinear, None),
        };
        let sweep = std::env::var("FOAM_SWEEP").unwrap_or_default();
        let bare = [(0.006, 0), (0.005, 0), (0.004, 0), (0.00334, 0)];
        let layered = [(0.006, 1), (0.005, 1), (0.004, 1)];
        let list: Vec<(f64, usize)> = match sweep.as_str() {
            "bare" => bare.to_vec(),
            "layered" => layered.to_vec(),
            "matched" => vec![(0.004, 0), (0.006, 1), (0.00334, 0), (0.005, 1), (0.006, 2), (0.004, 1), (0.005, 2)],
            "multi" => vec![(0.006, 2), (0.005, 2), (0.004, 2), (0.005, 3), (0.004, 3)],
            _ => bare.iter().chain(layered.iter()).copied().collect(),
        };
        println!("convection {which}: {div:?}, grad(U) {grad:?}");
        for (h, n) in list {
            let d = Design { mesh: PipeMesh::Unstructured(TetDualSpec { cell_size: h, n_layers: n, first_layer: 3e-4, expansion: 1.2, ..TetDualSpec::PRE_GH1298 }), ..Design::default() };
            let mut e = Engine::default();
            let mut cells = 0;
            e.serve(Request::Mesh { id: 1, design: d, probe: 0.75 }, &mut |ev| {
                if let Event::Meshed(me) = ev {
                    cells = me.n_cells;
                }
            });
            e.serve(Request::Start { id: 1, reynolds: 100.0, mode: Mode::Steady }, &mut |_| {});
            if let Some(Case::Pipe(s, _)) = e.case.as_mut() {
                s.schemes.default_div = div.clone();
                s.schemes.grad_u = grad.clone();
            }
            let t0 = bugis::web_demo::platform::now_s();
            let mut out = String::new();
            for _ in 0..600 {
                let mut done = false;
                e.serve(Request::Step { id: 1, steps: 5 }, &mut |ev| match ev {
                    Event::Progress(p) => done = p.steady,
                    Event::Error(m) => {
                        out = m;
                        done = true;
                    }
                    _ => {}
                });
                if done {
                    break;
                }
            }
            let wall = bugis::web_demo::platform::now_s() - t0;
            if let Some(Case::Pipe(s, flow)) = &e.case {
                use outram_foam_appbuilder_lib::cases::laminar_pipe::{centreline_velocity, pressure_gradient, profile_rms, wall_shear_stress};
                use uom::si::acceleration::meter_per_second_squared;
                let uc = centreline_velocity(s, flow, (0.0, 0.0), 0.15, 0.01).get::<meter_per_second>() / (2.0 * U_IN);
                let g = pressure_gradient(s, 0.13, 0.17).get::<meter_per_second_squared>() / flow.hagen_poiseuille_gradient().get::<meter_per_second_squared>();
                let rms = profile_rms(s, flow, (0.0, 0.0), 0.13, 0.17);
                let tau = wall_shear_stress(s, 0.13, 0.17) / flow.hagen_poiseuille_wall_shear();
                println!("h {h} layers {n}: {cells} cells, {} steps, {wall:.1} s: uc {uc:.4} dpdz {g:.4} rms {rms:.4} tau {tau:.4} {out}", e.steps);
            } else {
                println!("h {h} layers {n}: {cells} cells: {out}");
            }
        }
    }

    /// Diagnostic (gh:#1202, coordinator's instrument check), `--ignored`:
    /// the u_c and profile instruments side by side on the unstructured
    /// default and on the O-grid 7 920 control, Re = 100, to steady. Per
    /// mesh and scheme it prints: the binned first bin; the fit
    /// u = a − b r² over r < R/2 (the headline) and over r < 0.9 R; the RMS
    /// of (u − u_HP(r))/2U at each cell centre over 0.65–0.85 L with the
    /// nominal U and R, and with the measured U_b = Q/A and R_eq = √(A/π);
    /// the slab flow against the inlet flux; the inlet flux against U πR²;
    /// the slab section area against πR²; and u_c, dp/dz normalised by the
    /// measured flow (2 U_b; 8 ν U_b / R_eq²).
    #[test]
    #[ignore]
    fn diagnose_pipe_instruments() {
        use outram_foam_appbuilder_lib::cases::laminar_pipe::{centreline_velocity, fit_line, patch_flux, pressure_gradient, radial_profile_binned};
        use outram_foam_appbuilder_lib::io::fv_schemes::{DivScheme, GradScheme};
        use uom::si::acceleration::meter_per_second_squared;
        let dual = Design { mesh: PipeMesh::Unstructured(TetDualSpec { cell_size: 0.004, n_layers: 0, first_layer: 3e-4, expansion: 1.2, ..TetDualSpec::PRE_GH1298 }), ..Design::default() };
        let ogrid = Design { target_cells: 8000, ..Design::default() };
        let lu = (DivScheme::GaussLinearUpwind("grad(U)".into()), Some(GradScheme::CellLimitedGaussLinear(1.0)));
        let lin = (DivScheme::GaussLinear, None);
        for (name, d, scheme) in [("dual h 4 mm, linearUpwind+cellLimited", dual, Some(lu)), ("dual h 4 mm, Gauss linear", dual, Some(lin)), ("dual h 4 mm, linearUpwind unlimited", dual, Some((DivScheme::GaussLinearUpwind("grad(U)".into()), None))), ("O-grid 7920, upwind (its setting)", ogrid, None)] {
            let mut e = Engine::default();
            e.serve(Request::Mesh { id: 1, design: d, probe: 0.75 }, &mut |_| {});
            e.serve(Request::Start { id: 1, reynolds: 100.0, mode: Mode::Steady }, &mut |_| {});
            if let (Some((div, grad)), Some(Case::Pipe(s, _))) = (scheme, e.case.as_mut()) {
                s.schemes.default_div = div;
                s.schemes.grad_u = grad;
            }
            for _ in 0..400 {
                let mut done = false;
                e.serve(Request::Step { id: 1, steps: 5 }, &mut |ev| {
                    if let Event::Progress(p) = ev {
                        done = p.steady;
                    }
                });
                if done {
                    break;
                }
            }
            let Some(Case::Pipe(s, flow)) = &e.case else { panic!("no pipe") };
            let m = &s.mesh;
            let (r, l, u) = (0.01, 0.2, U_IN);
            let (z0, z1) = (0.65 * l, 0.85 * l);
            let r2 = |c: usize| m.cell_centres[c].x.powi(2) + m.cell_centres[c].y.powi(2);
            let fit = |rmax: f64| {
                let pts: Vec<(f64, f64, f64)> = (0..m.n_cells)
                    .filter(|&c| (m.cell_centres[c].z - 0.75 * l).abs() < 0.05 * l && r2(c) < rmax * rmax * r * r)
                    .map(|c| (r2(c), s.u.internal[c].z, m.cell_volumes[c]))
                    .collect();
                fit_line(&pts).0 / (2.0 * u)
            };
            let head = centreline_velocity(s, flow, (0.0, 0.0), 0.75 * l, 0.05 * l).get::<meter_per_second>() / (2.0 * u);
            let bins = radial_profile_binned(s, flow, (0.0, 0.0), 0.75 * l, self_hz(d, &e), PROFILE_BINS);
            let bin0 = bins.first().map_or(f64::NAN, |b| b.1 / 2.0);
            let slab: Vec<usize> = (0..m.n_cells).filter(|&c| (z0..=z1).contains(&m.cell_centres[c].z)).collect();
            let vol: f64 = slab.iter().map(|&c| m.cell_volumes[c]).sum();
            let area = vol / (z1 - z0);
            let q_slab = slab.iter().map(|&c| s.u.internal[c].z * m.cell_volumes[c]).sum::<f64>() / (z1 - z0);
            let q_in = -patch_flux(s, "inlet");
            // The section the probe plane (0.75 L) cuts, exactly: Σ of the
            // cut polygons' areas; and Σ u_z A over them (midpoint quadrature).
            let a_cut: f64 = e.cross.iter().map(|c| outram_foam_mesh::section::polygon_area(&c.polygon).abs()).sum();
            let q_cut: f64 = e.cross.iter().map(|c| s.u.internal[c.cell].z * outram_foam_mesh::section::polygon_area(&c.polygon).abs()).sum();
            println!("  cut at 0.75 L: area / pi R^2 {:.4}; sum u A / inlet Q {:.4} (slab-volume area / pi R^2 {:.4})", a_cut / (std::f64::consts::PI * r * r), q_cut / q_in, area / (std::f64::consts::PI * r * r));
            let area = a_cut;
            let ub = q_in / area;
            let r_eq2 = area / std::f64::consts::PI;
            let rms = |uu: f64, rr2: f64| {
                (slab.iter().map(|&c| ((s.u.internal[c].z - 2.0 * uu * (1.0 - r2(c) / rr2)) / (2.0 * uu)).powi(2) * m.cell_volumes[c]).sum::<f64>() / vol).sqrt()
            };
            let g = pressure_gradient(s, z0, z1).get::<meter_per_second_squared>();
            let nu = flow.nu.get::<uom::si::kinematic_viscosity::square_meter_per_second>();
            let hp_nom = flow.hagen_poiseuille_gradient().get::<meter_per_second_squared>();
            let hp_meas = 8.0 * nu * ub / r_eq2;
            println!(
                "{name}: {} cells, {} steps\n  binned bin0 u/2U {bin0:.4} (bin centre r/R {:.3}); fit r<R/2 {head:.4} (recomputed {:.4}); fit r<0.9R {:.4}\n  RMS nominal {:.4}; RMS measured-flow {:.4}\n  slab Q / inlet Q {:.6}; inlet Q / (U pi R^2) {:.4}; slab area / pi R^2 {:.4}\n  u_c / 2U_b {:.4}; dp/dz / (8 nu U_b / R_eq^2) {:.4}; (nominal: u_c/2U {head:.4}, dp/dz/HP {:.4})",
                m.n_cells,
                e.steps,
                bins.first().map_or(f64::NAN, |b| b.0),
                fit(0.5),
                fit(0.9),
                rms(u, r * r),
                rms(ub, r_eq2),
                q_slab / q_in,
                q_in / (u * std::f64::consts::PI * r * r),
                area / (std::f64::consts::PI * r * r),
                head * u / ub,
                g / hp_meas,
                g / hp_nom,
            );
        }
        fn self_hz(d: Design, e: &Engine) -> f64 {
            match d.mesh {
                PipeMesh::Structured => 0.5 * e.cell_len,
                PipeMesh::Unstructured(_) => e.cell_len,
            }
        }
    }

    /// Diagnostic (gh:#1202), `--ignored`: the port's linear interpolation
    /// weight `|C_N − C_f| / |C_N − C_O|` against upstream's
    /// `surfaceInterpolation::makeWeights`, `|S·(C_N − C_f)| / (|S·(C_f − C_O)|
    /// + |S·(C_N − C_f)|)`, on the demo meshes: faces that differ, the worst
    /// difference, and port weights outside [0, 1].
    #[test]
    #[ignore]
    fn diagnose_interpolation_weights() {
        let spec = |h: f64, n: usize| TetDualSpec { cell_size: h, n_layers: n, first_layer: 3e-4, expansion: 1.2, ..TetDualSpec::PRE_GH1298 };
        let tags = [Tag::Inlet, Tag::Outlet, Tag::Wall];
        let cases: Vec<(&str, PolyMesh)> = vec![
            ("O-grid 20480 (structured default)", mesh_design(&Design::default()).unwrap().0),
            ("O-grid 7920", mesh_design(&Design { target_cells: 8000, ..Design::default() }).unwrap().0),
            ("dual h 4 mm", mesh_unstructured(0.01, 0.2, [0.0, 0.0, 0.1], tags, &spec(0.004, 0)).unwrap().0),
        ];
        for (name, poly) in cases {
            let m = poly.to_fv_mesh().unwrap();
            let (mut n_diff, mut worst, mut outside, mut sum) = (0usize, 0.0_f64, 0usize, 0.0);
            for f in 0..m.n_internal_faces {
                let (co, cn, cf, s) = (m.cell_centres[m.owner[f]], m.cell_centres[m.neighbour[f]], m.face_centres[f], m.face_area_vectors[f]);
                let w_port = (cn - cf).mag() / (cn - co).mag();
                let (so, sn) = (s.dot(cf - co).abs(), s.dot(cn - cf).abs());
                let w_of = sn / (so + sn);
                let d = (w_port - w_of).abs();
                if d > 1e-12 {
                    n_diff += 1;
                }
                worst = worst.max(d);
                sum += d;
                if !(0.0..=1.0).contains(&w_port) {
                    outside += 1;
                }
            }
            println!(
                "{name}: {} internal faces; differ (>1e-12) {n_diff}; mean |dw| {:.3e}; max |dw| {worst:.3e}; port w outside [0,1] {outside}",
                m.n_internal_faces,
                sum / m.n_internal_faces as f64
            );
        }
    }

    /// The mesher choices: ~~the default is the mesher every recorded number
    /// was taken on~~ **gh:#1298:** the default is the cell-centre dual,
    /// corners only, cfMesh's optimiser and upstream's sequence; the old
    /// default is `PRE_GH1298`, reachable from the URL; both cross the
    /// clamp and the wire codes unchanged. **gh:#1298, maintainer decision
    /// (2026-11-05):** the default case is h 5 mm with 3 layers (~~h 4 mm,
    /// no layer~~), which the clamp and the 50k cap leave alone; the URL's
    /// `h=4&layers=0` reaches the bare h 4 mm case, and `PRE_GH1298` keeps
    /// the old default's numbers.
    #[test]
    fn mesher_choices_default_to_upstreams_sequence_and_follow_the_url() {
        use outram_blender::foam_mesh::LayerQualityStrategy as L;
        use outram_park_fork_cfmesh::dual::{DualKind as D, FeaturePointRule as F};
        let d = TetDualSpec::default_for(0.01);
        assert_eq!((d.dual, d.feature_points, d.layer_quality, d.route), (D::CellCentre, F::Corners, L::CfMeshOptimise, CfMeshRoute::UpstreamNoLaplace));
        assert_eq!(d.choice_codes(), [2.0, 1.0, 1.0, 0.0]);
        assert_eq!((d.cell_size, d.n_layers, d.first_layer, d.expansion), (0.005, 3, 0.03 * 0.01, 1.2));
        assert_eq!((TetDualSpec::BASE.cell_size, TetDualSpec::BASE.n_layers), (d.cell_size, d.n_layers));
        assert_eq!(d.clamped(0.01, 0.2), d, "the clamp and the 50k cap leave the default alone");
        // The bare h 4 mm case, the default before the gh:#1298 decision,
        // from the URL; and a value that does not parse changes nothing.
        let bare = d.with_size_query(Some("4"), Some("0"));
        assert_eq!(bare, TetDualSpec { cell_size: 0.004, n_layers: 0, ..d });
        assert_eq!(bare.clamped(0.01, 0.2), bare);
        assert_eq!(d.with_size_query(Some("x"), None), d);
        assert_eq!(d.with_size_query(None, Some("9")).n_layers, MAX_LAYERS);
        // The ablation is the pre-gh:#1298 default, choice for choice.
        let old = TetDualSpec::PRE_GH1298;
        assert_eq!((old.dual, old.feature_points, old.layer_quality, old.route), (D::MedianPerCorner, F::OnFeatureEdge, L::BackOffOnly, CfMeshRoute::PreGh1298));
        assert_eq!((old.cell_size, old.n_layers, old.first_layer, old.expansion), (0.004, 0, 3e-4, 1.2));
        // The URL reaches it, and back.
        let q = d.with_query(Some("corner"), Some("edges"), Some("backoff")).with_route_query(Some("legacy"));
        assert_eq!(q.choice_codes(), [0.0, 0.0, 0.0, 1.0]);
        assert_eq!(q.clamped(0.01, 0.2).choice_codes(), [0.0, 0.0, 0.0, 1.0]);
        assert_eq!(q.with_route_query(Some("upstream")).route, CfMeshRoute::UpstreamNoLaplace);
        // Unknown URL values leave the choices alone.
        assert_eq!(q.with_query(Some("bogus"), None, Some("?")).with_route_query(Some("?")), q);
        let back = q.with_query(Some("centre"), Some("corners"), Some("cfmesh")).with_route_query(Some("upstream"));
        assert_eq!(back, d);
        // The route's options: the ablation leaves the default options
        // alone; the default switches on exactly gh:#1283's `upEnl` steps.
        let o = outram_blender::foam_mesh::TetDualOptions::default();
        assert_eq!(route_options(o.clone(), CfMeshRoute::PreGh1298), o);
        let u = route_options(o.clone(), CfMeshRoute::UpstreamNoLaplace);
        let c = &u.cfmesh_layer_optimisation;
        assert!(u.untangle_surface && u.optimise_final_mesh && c.untangle_boundary_moves && c.untangle_mesh_untangler && !c.optimize_mesh_fv_laplace);
        assert_eq!(c.boundary_projection, outram_park_fork_cfmesh::final_mesh::BoundaryProjection::EachPass);
    }

    #[test]
    fn requests_and_events_survive_the_wire() {
        let reqs = [
            Request::Mesh { id: 3, design: Design::default(), probe: 0.5 },
            Request::Mesh {
                id: 4,
                design: Design {
                    shape: Shape::Box { size: [0.1, 0.02, 0.03], centre: [0.0, 1.0, 2.0] },
                    tags: [Tag::Inlet, Tag::Outlet, Tag::Wall, Tag::Wall, Tag::Wall, Tag::Wall],
                    target_cells: 900,
                    mesh: PipeMesh::Structured,
                },
                probe: 0.25,
            },
            Request::Start { id: 3, reynolds: 50.0, mode: Mode::Transient },
            Request::Mesh { id: 5, design: Design::exchanger(), probe: 0.3 },
            Request::Mesh { id: 6, design: Design::coil(), probe: 0.6 },
            Request::Mesh { id: 7, design: Design { mesh: PipeMesh::Unstructured(TetDualSpec::default_for(0.01)), ..Design::default() }, probe: 0.4 },
            // gh:#1240 browser controls: every non-default mesher choice crosses the wire.
            Request::Mesh {
                id: 8,
                design: Design {
                    mesh: PipeMesh::Unstructured(TetDualSpec::default_for(0.01).with_query(Some("centre"), Some("corners"), Some("cfmesh"))),
                    ..Design::default()
                },
                probe: 0.4,
            },
            Request::Mesh {
                id: 9,
                design: Design { mesh: PipeMesh::Unstructured(TetDualSpec::default_for(0.01).with_query(Some("point"), None, Some("snappy"))), ..Design::default() },
                probe: 0.4,
            },
            // gh:#1298: the ablation's route crosses the wire.
            Request::Mesh { id: 10, design: Design { mesh: PipeMesh::Unstructured(TetDualSpec::PRE_GH1298), ..Design::default() }, probe: 0.4 },
            Request::Step { id: 3, steps: 7 },
            Request::Probe { id: 3, probe: 0.9 },
        ];
        for r in reqs {
            assert_eq!(Request::from_wire(&r.to_wire()).unwrap(), r);
        }
        assert!(Request::from_wire(&[9.0, 1.0]).is_err());
        // An unknown mesher-choice code is refused, not silently defaulted.
        let mut w = Request::Mesh { id: 8, design: Design { mesh: PipeMesh::Unstructured(TetDualSpec::default_for(0.01)), ..Design::default() }, probe: 0.4 }.to_wire();
        let n = w.len();
        w[n - 3] = 7.0;
        assert!(Request::from_wire(&w).is_err());
        // gh:#1298: an unknown route code too.
        let mut w = Request::Mesh { id: 8, design: Design { mesh: PipeMesh::Unstructured(TetDualSpec::default_for(0.01)), ..Design::default() }, probe: 0.4 }.to_wire();
        let n = w.len();
        w[n - 1] = 5.0;
        assert!(Request::from_wire(&w).is_err());
        assert!(Request::from_wire(&[0.0]).is_err());
        let sec = Section { axis: 1, offset: 0.5, polygons: vec![vec![[0.0, 0.0], [1.0, 0.0], [1.0, 1.0]]], frame: None };
        let framed = Section { frame: Some([1.0, 2.0, 3.0, 0.0, 1.0, 0.0, 0.0, 0.0, 1.0]), ..sec.clone() };
        let evs = [
            Event::Meshed(Meshed {
                id: 2,
                n_cells: 10,
                max_non_ortho_deg: 12.5,
                flow_axis: 2,
                cell_len: 0.01,
                edges: vec![0.0, 1.0, 2.0, 3.0, 4.0, 5.0, 2.0],
                axial: sec.clone(),
                cross: sec.clone(),
                max_skewness: 0.5,
                mesh_s: 0.25,
                notes: "a note".into(),
            }),
            Event::Cross { id: 2, cross: sec },
            Event::Cross { id: 2, cross: framed },
            Event::Progress(Progress {
                id: 2,
                steps: 4,
                time_s: 1.5,
                dt_s: 0.5,
                change: 0.1,
                continuity: 1e-12,
                outlet_over_inlet: 1.0,
                probe_u: 0.01,
                probe_p: 2e-5,
                dp: 8e-5,
                ms_per_step: 12.0,
                steady: false,
                axial_u: vec![1.0, 2.0],
                axial_p: vec![3.0],
                cross_u: vec![],
                cross_p: vec![4.0],
                cross_ua: vec![5.0],
                phase: 1,
                axial_t: vec![300.0],
                cross_t: vec![310.0, 320.0],
                exch: [1.0, 2.0, 3.0, 4.0, 5.0, 6.0, 7.0, 8.0, 9.0],
                cross_sec: vec![0.5, -0.5],
                coil: [1.0, 2.0, 3.0, 4.0, 5.0, 6.0, 7.0, 8.0],
                profile: vec![0.1, 1.9, 0.5, 1.4],
            }),
        ];
        for e in evs {
            let (d, t) = e.to_wire();
            assert_eq!(Event::from_wire(&d, &t).unwrap(), e);
        }
        let (d, t) = Event::Error("x".into()).to_wire();
        assert_eq!(Event::from_wire(&d, &t).unwrap(), Event::Error("x".into()));
    }

    #[test]
    fn designs_are_checked_and_sized() {
        assert!(check(&Design::default()).is_ok());
        let mut d = Design::default();
        d.tags[0] = Tag::Wall;
        assert!(check(&d).unwrap_err().contains("inlet"));
        d.tags[0] = Tag::Inlet;
        d.tags[1] = Tag::Wall;
        assert!(check(&d).unwrap_err().contains("outlet"));
        d = Design { target_cells: MAX_CELLS + 1, ..Design::default() };
        assert!(check(&d).is_err());
        d = Design { shape: Shape::Box { size: [0.1, 0.0, 0.1], centre: [0.0; 3] }, ..Design::default() };
        assert!(check(&d).is_err());
        // The default pipe is stage 0's 8/6/80 grid.
        let g = pipe_grid(0.01, 0.2, [0.0; 3], DEFAULT_CELLS);
        assert_eq!((g.n, g.nr, g.nz, g.n_cells()), (8, 6, 80, 20480));
        assert_eq!(pipe_grid(0.01, 0.2, [0.0; 3], MAX_CELLS).n_cells(), 42000);
        assert_eq!(pipe_grid(0.01, 0.2, [0.0; 3], 2_000).n_cells(), 1920);
        let b = box_block([0.1, 0.02, 0.02], [0.0; 3], 1000);
        assert!(b.n_cells() <= 1000 && b.n_cells() > 500, "{:?}", b.cells);
        assert!(Shape::Box { size: [1.0; 3], centre: [0.0; 3] }.face_names().len() == 6);
    }

    /// The headless regression (engine CLAUDE.md): the 1.9k-cell pipe at
    /// Re = 100 reaches the steady criterion, conserves mass, and its
    /// developed numbers sit where the V&V record puts this resolution.
    #[test]
    fn headless_pipe_reaches_steady_hagen_poiseuille() {
        let lines = headless(small_pipe(), 100.0, Mode::Steady, 400).unwrap();
        let joined = lines.join("\n");
        assert!(joined.contains("1920 cells"), "{joined}");
        let pipe = lines.iter().find(|l| l.starts_with("# pipe")).expect("pipe line");
        println!("{pipe}"); // gh:#1488: print the measured numbers so a record can quote them
        let val = |k: &str| -> f64 {
            pipe.split_whitespace().find_map(|t| t.strip_prefix(k)).and_then(|v| v.parse().ok()).expect(k)
        };
        let uc = val("uc=");
        let ratio = val("dpdz=");
        assert!((0.97..1.02).contains(&uc), "u_c/2U {uc}");
        assert!((0.99..1.06).contains(&ratio), "dp/dz / HP {ratio}");
        let last = lines.iter().rev().find(|l| !l.starts_with('#')).unwrap();
        let out_in: f64 = last.split_whitespace().nth(4).unwrap().parse().unwrap();
        assert!((out_in - 1.0).abs() < 1e-8, "{last}");
    }

    /// **Since gh:#1298 this pins the ablation, `TetDualSpec::PRE_GH1298`**
    /// (the old default mesher); the new default is pinned by
    /// `headless_unstructured_default_is_pinned`.
    ///
    /// The headless regression of the **unstructured** pipe (gh:#1202): the
    /// cfMesh dual, the demo default (h = 4 mm, no layer, 6353 cells), at
    /// Re = 100, reaches the steady criterion and conserves mass; its patches
    /// arrive named `inlet`, `outlet`, `walls` (typed wall); its developed
    /// numbers are pinned where the V&V record measured them on 2026-11-04
    /// (`laminar_pipe_gh1155.md`, gh:#1202 section): with the feature-edge
    /// capture and `linearUpwind grad(U)` + `cellLimited Gauss linear 1`,
    /// 2 correctors: u_c/2U **0.8925**, dp/dz **1.0326** × HP. Earlier
    /// pins, struck: ~~h 6 mm, 1 layer, upwind: 0.6105 / 1.2452 (`limited 0`),
    /// then 0.6179 / 1.3355 (`limited corrected 0.33`, 2 correctors)~~;
    /// ~~h 4 mm, `Gauss linear`, chamfered rim: 0.9416 / 0.9414~~.
    /// Pinned so that a change is seen. Deterministic: run twice, the same
    /// lines.
    #[test]
    fn headless_unstructured_pipe_runs_and_is_pinned() {
        let d = Design { mesh: PipeMesh::Unstructured(TetDualSpec { cell_size: 0.004, n_layers: 0, first_layer: 3e-4, expansion: 1.2, ..TetDualSpec::PRE_GH1298 }), ..Design::default() };
        let (poly, _, _, _) = mesh_design_noted(&d).unwrap();
        let fv = poly.to_fv_mesh().unwrap();
        let mut names: Vec<(&str, outram_foam_basic_lib::mesh::PatchKind)> = fv.patches.iter().map(|p| (p.name.as_str(), p.kind)).collect();
        names.sort_by_key(|n| n.0);
        assert_eq!(names.iter().map(|n| n.0).collect::<Vec<_>>(), ["inlet", "outlet", "walls"]);
        assert_eq!(names[2].1, outram_foam_basic_lib::mesh::PatchKind::Wall);
        let lines = headless(d, 100.0, Mode::Steady, 200).unwrap();
        let joined = lines.join("\n");
        assert!(joined.contains("6353 cells"), "{joined}");
        let pipe = lines.iter().find(|l| l.starts_with("# pipe (")).expect("pipe line");
        println!("{pipe}"); // gh:#1488: print the measured numbers so a record can quote them
        let val = |k: &str| -> f64 { pipe.split_whitespace().find_map(|t| t.strip_prefix(k)).and_then(|v| v.parse().ok()).expect(k) };
        assert!((val("uc=") - 0.8925).abs() < 2e-3, "{pipe}");
        assert!((val("dpdz=") - 1.0326).abs() < 2e-3, "{pipe}");
        let last = lines.iter().rev().find(|l| !l.starts_with('#')).unwrap();
        let out_in: f64 = last.split_whitespace().nth(4).unwrap().parse().unwrap();
        assert!((out_in - 1.0).abs() < 1e-8, "{last}");
        let again = headless(d, 100.0, Mode::Steady, 200).unwrap();
        let strip = |v: &[String]| v.iter().filter(|l| l.starts_with("# pipe")).cloned().collect::<Vec<_>>();
        assert_eq!(strip(&again), strip(&lines), "deterministic");
    }

    /// gh:#1298 — the **new unstructured default** (`TetDualSpec::default_for`:
    /// ~~h 4 mm, no layer~~ **h 5 mm, 3 wall layers (first 0.3 mm, × 1.2)
    /// since the maintainer's decision of 2026-11-05**, the cell-centre dual
    /// with corners, cfMesh's layer optimiser, upstream's sequence without
    /// the Laplace pass), Re 100, steady. Pinned where the V&V record
    /// measured it on 2026-11-05 (`laminar_pipe_gh1155.md`, gh:#1298
    /// section, "The default becomes h 5 mm with 3 layers"): **7 049 cells,
    /// u_c/2U 0.9394, dp/dz 1.0062 × HP** (predicted from gh:#1298's layer
    /// case: 7 049 ± 30, 0.9394 ± 0.002, 1.0062 ± 0.002). Old pin, struck:
    /// ~~h 4 mm bare: 6 353 cells, u_c/2U 0.8832, dp/dz 1.0649 × HP~~ (its
    /// prediction was 0.86–0.91 and 1.00–1.04: u_c held, dp/dz missed high;
    /// the miss stays in the record). Mass conserved; deterministic.
    #[test]
    fn headless_unstructured_default_is_pinned() {
        let d = Design { mesh: PipeMesh::Unstructured(TetDualSpec::default_for(0.01)), ..Design::default() };
        let lines = headless(d, 100.0, Mode::Steady, 200).unwrap();
        let joined = lines.join("\n");
        assert!(joined.contains("7049 cells"), "{joined}");
        let pipe = lines.iter().find(|l| l.starts_with("# pipe (")).expect("pipe line");
        println!("{pipe}"); // gh:#1488: print the measured numbers so a record can quote them
        let val = |k: &str| -> f64 { pipe.split_whitespace().find_map(|t| t.strip_prefix(k)).and_then(|v| v.parse().ok()).expect(k) };
        assert!((val("uc=") - 0.9394).abs() < 2e-3, "{pipe}");
        assert!((val("dpdz=") - 1.0062).abs() < 2e-3, "{pipe}");
        let last = lines.iter().rev().find(|l| !l.starts_with('#')).unwrap();
        let out_in: f64 = last.split_whitespace().nth(4).unwrap().parse().unwrap();
        assert!((out_in - 1.0).abs() < 1e-8, "{last}");
        let again = headless(d, 100.0, Mode::Steady, 200).unwrap();
        let strip = |v: &[String]| v.iter().filter(|l| l.starts_with("# pipe")).cloned().collect::<Vec<_>>();
        assert_eq!(strip(&again), strip(&lines), "deterministic");
    }

    /// gh:#1241 — OpenFOAM's time-step update by hand
    /// (`setInitialDeltaT.H`, `setDeltaT.H`): at Co 4 with `maxCo 1` the first
    /// step is cut to a quarter; later steps fall by `maxCo / Co` when Co is
    /// too large, grow by at most `1 + 0.1 maxCo/Co` and 1.2, and never pass
    /// `maxDeltaT`; Co 0 leaves the first step alone. Then the pipe with
    /// `max_co = 1` (the small O-grid, Re 100, 20 steps): after the first
    /// step Co stays within the growth bound of 1 (≤ 1.2), and the run with
    /// the option off keeps the fixed step.
    #[test]
    fn adjust_time_step_follows_openfoam() {
        assert!((adjusted_delta_t(2.0, 4.0, 1.0, 2.0, true) - 0.5).abs() < 1e-15);
        assert_eq!(adjusted_delta_t(2.0, 0.0, 1.0, 2.0, true), 2.0);
        assert!((adjusted_delta_t(1.0, 4.0, 1.0, 10.0, false) - 0.25).abs() < 1e-12);
        assert!((adjusted_delta_t(1.0, 0.5, 1.0, 10.0, false) - 1.2).abs() < 1e-12);
        assert!((adjusted_delta_t(1.0, 0.9, 1.0, 10.0, false) - (1.0 + 0.1 / 0.9)).abs() < 1e-12);
        assert_eq!(adjusted_delta_t(1.0, 0.1, 1.0, 1.1, false), 1.1);
        for max_co in [None, Some(1.0)] {
            let mut e = Engine { max_co, ..Engine::default() };
            e.serve(Request::Mesh { id: 1, design: small_pipe(), probe: 0.75 }, &mut |_| {});
            e.serve(Request::Start { id: 1, reynolds: 100.0, mode: Mode::Steady }, &mut |_| {});
            let dt0 = match &e.case {
                Some(Case::Pipe(s, _)) => s.control.delta_t,
                _ => panic!("no pipe"),
            };
            for _ in 0..4 {
                e.serve(Request::Step { id: 1, steps: 5 }, &mut |_| {});
            }
            let Some(Case::Pipe(s, _)) = &e.case else { panic!("no pipe") };
            let (co, mean) = courant_numbers(s);
            match max_co {
                None => assert_eq!(s.control.delta_t, dt0),
                Some(m) => assert!(co <= 1.2 * m && mean < co && s.control.delta_t < dt0, "Co {co}, dt {} of {dt0}", s.control.delta_t),
            }
        }
    }

    /// The unstructured controls are clamped (a phone cannot ask for a mesh
    /// that freezes it) and the estimate follows the measured sweep.
    #[test]
    fn unstructured_specs_are_clamped() {
        let s = TetDualSpec { cell_size: 1e-5, n_layers: 9, first_layer: 1.0, expansion: 3.0, ..TetDualSpec::PRE_GH1298 }.clamped(0.01, 0.2);
        assert!(s.cell_size >= 0.01 / CELL_PER_R_MAX - 1e-15 && s.n_layers == MAX_LAYERS && s.first_layer <= 1e-3 + 1e-15 && s.expansion == 1.5);
        assert!(s.estimated_cells(0.01, 0.2) <= MAX_CELLS as f64);
        let d = TetDualSpec::default_for(0.01);
        let e = d.estimated_cells(0.01, 0.2);
        // ~~The default (h 4 bare): within 15 % of 6 353~~; since the
        // gh:#1298 decision the default is h 5 mm with 3 layers: 7 188
        // estimated against 7 049 measured, far under the 50k cap, so the
        // clamp leaves h at 5 mm. The bare h 4 mm case keeps its check.
        assert!((e / 7_049.0 - 1.0).abs() < 0.10, "estimate {e} vs measured 7049");
        assert!(e < MAX_CELLS as f64);
        assert_eq!(d.clamped(0.01, 0.2), d);
        let bare = TetDualSpec { cell_size: 0.004, n_layers: 0, ..d }.estimated_cells(0.01, 0.2);
        assert!((bare / 6_353.0 - 1.0).abs() < 0.15, "estimate {bare} vs measured 6353");
        // gh:#1298: the polyDualMesh estimate against the --mesh-sweep
        // counts of the new default with 3 layers (h 6 / 5 / 4 mm: 4 417 /
        // 7 049 / 12 509 cells), and the per-corner one against its own
        // (h 5 mm, 3 layers: 26 009 cells, gh:#1285's table). Before
        // gh:#1298 the polyDualMesh duals got the per-corner estimate,
        // ~26k for 7 049.
        for (h, measured) in [(0.006, 4_417.0), (0.005, 7_049.0), (0.004, 12_509.0)] {
            let e = TetDualSpec { cell_size: h, n_layers: 3, ..d }.estimated_cells(0.01, 0.2);
            assert!((e / measured - 1.0).abs() < 0.10, "h {h}: estimate {e} vs measured {measured}");
        }
        let corner = TetDualSpec { cell_size: 0.005, n_layers: 3, ..TetDualSpec::PRE_GH1298 }.estimated_cells(0.01, 0.2);
        assert!((corner / 26_009.0 - 1.0).abs() < 0.15, "per-corner estimate {corner} vs 26009");
    }

    /// The headless exchanger regression (gh:#1156): a ~2.4k-cell double
    /// pipe at the demo's Re = 30 marches its flows, freezes them, iterates
    /// the energy to steady, closes energy between the streams, and its
    /// effectiveness sits within a few per cent of ε-NTU at the local UA
    /// (the coarse-grid gap of the V&V record, which shrinks as 1/nz).
    #[test]
    fn headless_exchanger_closes_energy_and_matches_ntu() {
        let mut d = Design::exchanger();
        d.target_cells = 2_500;
        assert!(check(&d).is_ok());
        let lines = headless(d, 30.0, Mode::Steady, 2_000).unwrap();
        let joined = lines.join("\n");
        let ex = lines.iter().find(|l| l.starts_with("# exchanger")).unwrap_or_else(|| panic!("{joined}"));
        let val = |k: &str| -> f64 { ex.split_whitespace().find_map(|t| t.strip_prefix(k)).and_then(|v| v.parse().ok()).expect(k) };
        let (qh, qc) = (val("Qhot="), val("Qcold="));
        assert!((qh / qc - 1.0).abs() < 1e-6, "{ex}");
        let (eps, ntu) = (val("eps="), val("eps_ntu="));
        assert!(eps > 0.3 && eps < 0.8, "{ex}");
        assert!((eps / ntu - 1.0).abs() < 0.05, "{ex}");
        assert!(val("Thot_out=") < T_HOT_IN && val("Tcold_out=") > T_COLD_IN, "{ex}");
        // Both phases ran, and the last report is steady.
        let last = lines.iter().rev().find(|l| !l.starts_with('#')).unwrap();
        assert!(last.ends_with(" 1"), "{last}");
        assert!(lines.iter().any(|l| !l.starts_with('#') && l.ends_with(" 0")), "{joined}");
    }

    /// The headless coil regression (gh:#1157): a ~3k-cell, two-turn demo
    /// coil at Re = 200 (De = 63) reaches the steady criterion, conserves
    /// mass, shows the Dean flow (core outward, axial maximum outward, the
    /// return flows meeting at the inner wall) and a friction factor above
    /// the straight pipe's; numbers pinned loosely, to this grid.
    #[test]
    fn headless_coil_shows_dean_flow() {
        let d = Design { target_cells: 3_000, ..Design::coil() };
        let lines = headless(d, 200.0, Mode::Steady, 3_000).unwrap();
        let joined = lines.join("\n");
        let co = lines.iter().find(|l| l.starts_with("# coil")).unwrap_or_else(|| panic!("{joined}"));
        let val = |k: &str| -> f64 { co.split_whitespace().find_map(|t| t.strip_prefix(k)).and_then(|v| v.parse().ok()).expect(k) };
        assert!((val("De=") - 63.12).abs() < 0.05, "{co}");
        assert!(val("core_un=") < -0.02 && val("peak_n=") < -0.1 && val("pair=") < 0.0, "{co}");
        let (f, mg) = (val("fRe64="), val("MG="));
        assert!(f > 1.15 && f < 1.6 && (mg - 1.3465).abs() < 1e-3, "{co}");
        let last = lines.iter().rev().find(|l| !l.starts_with('#')).unwrap();
        let out_in: f64 = last.split_whitespace().nth(4).unwrap().parse().unwrap();
        assert!((out_in - 1.0).abs() < 1e-8, "{last}");
        assert!(joined.contains("cells"), "{joined}");
    }

    #[test]
    fn coil_designs_are_checked_sized_and_served() {
        let d = Design::coil();
        assert!(check(&d).is_ok());
        let g = coil_grid(&d).unwrap();
        assert!(g.n_cells() <= DEFAULT_CELLS && g.n_cells() > 15_000, "{}", g.n_cells());
        let big = coil_grid(&Design { target_cells: MAX_CELLS, ..d }).unwrap();
        assert!(big.n_cells() <= MAX_CELLS && big.n_cells() > 2 * g.n_cells(), "{}", big.n_cells());
        let mut bad = d;
        bad.shape = Shape::Coil { tube_radius: 0.005, coil_diameter: 0.1, pitch: 0.005, turns: 2.0, centre: [0.0; 3] };
        assert!(check(&bad).is_err(), "turns overlap");
        assert_eq!((coil_courant(Mode::Steady, 200.0), coil_courant(Mode::Steady, 400.0), coil_courant(Mode::Transient, 100.0)), (4.0, 1.0, 1.0));
        let [lo, hi] = d.shape.bounds();
        assert!((hi[0] - lo[0] - 0.11).abs() < 1e-12 && (hi[2] - lo[2] - (0.02 * COIL_DEFAULT_TURNS + 0.01)).abs() < 1e-12);
        // Served: the cross-section is one layer in the Frenet plane, its
        // polygons tile the tube's polygon, and the inlet is at the bottom.
        let mut e = Engine::default();
        let mut evs = Vec::new();
        let small = Design { target_cells: 1_500, ..d };
        e.serve(Request::Mesh { id: 2, design: small, probe: 0.5 }, &mut |ev| evs.push(ev));
        let Some(Event::Meshed(me)) = evs.pop() else { panic!("no mesh") };
        let g = coil_grid(&small).unwrap();
        assert_eq!(me.cross.polygons.len(), g.cells_per_section());
        let f = me.cross.frame.expect("framed");
        let o = g.centreline(m(f64::from(me.cross.offset as f32)));
        assert!((0..3).all(|i| (f[i] - o[i]).abs() < 1e-6), "{f:?} {o:?}");
        let area: f64 = me.cross.polygons.iter().map(|p| outram_foam_mesh::section::polygon_area(&p.iter().map(|q| [f64::from(q[0]), f64::from(q[1])]).collect::<Vec<_>>())).sum();
        assert!((area / g.straight_grid().polygon_area() - 1.0).abs() < 1e-5, "{area}");
        assert!(!me.axial.polygons.is_empty());
        e.serve(Request::Start { id: 2, reynolds: 100.0, mode: Mode::Transient }, &mut |ev| evs.push(ev));
        e.serve(Request::Step { id: 2, steps: 2 }, &mut |ev| evs.push(ev));
        let Some(Event::Progress(p)) = evs.pop() else { panic!("no progress") };
        assert_eq!(p.cross_sec.len(), 2 * p.cross_ua.len());
        assert!(p.coil[CoilNo::DEAN] > 0.0 && (p.outlet_over_inlet - 1.0).abs() < 1e-8);
        e.serve(Request::Probe { id: 2, probe: 0.1 }, &mut |ev| evs.push(ev));
        assert!(evs.iter().any(|ev| matches!(ev, Event::Cross { cross, .. } if cross.frame.is_some() && cross.offset < 0.1 * g.arc_length().get::<meter>() + g.arc_length().get::<meter>() / g.n_layers as f64)));
    }

    #[test]
    fn exchanger_designs_are_checked_and_sized() {
        let d = Design::exchanger();
        let Shape::DoublePipe { r_tube, wall, r_shell, length, centre } = d.shape else { panic!() };
        let g = exchanger_grid(r_tube, wall, r_shell, length, centre, DEFAULT_CELLS);
        let n: usize = g.n_cells().iter().sum();
        assert!(n <= 21_000 && n > 12_000, "{n} {:?}", g.n_cells());
        assert_eq!(g.n_cells(), [(25 + 80) * 75, 40 * 75, 100 * 75]);
        let big: usize = exchanger_grid(r_tube, wall, r_shell, length, centre, MAX_CELLS).n_cells().iter().sum();
        assert!(big <= 52_500 && big > 30_000, "{big}");
        let mut bad = d;
        bad.shape = Shape::DoublePipe { r_tube: 0.005, wall: 0.006, r_shell: 0.01, length: 0.2, centre };
        assert!(check(&bad).is_err());
        assert!(exchanger_meshes(&Design::default()).is_err());
        assert_eq!(d.shape.n_faces(), 0);
        assert!(d.shape.face_names().is_empty());
        assert_eq!(d.shape.bounds()[1][0], 0.01);
        // The drawing mesh is the three regions side by side.
        let (poly, axis, dz) = mesh_design(&d).unwrap();
        assert_eq!(poly.n_cells, n);
        assert_eq!(axis, 2);
        assert!((dz - 0.2 / 75.0).abs() < 1e-15);
    }

    #[test]
    fn engine_serves_a_box_and_moves_the_probe() {
        let mut e = Engine::default();
        let mut evs = Vec::new();
        let d = Design {
            shape: Shape::Box { size: [0.02, 0.02, 0.1], centre: [0.0, 0.0, 0.05] },
            tags: [Tag::Wall, Tag::Wall, Tag::Wall, Tag::Wall, Tag::Inlet, Tag::Outlet],
            target_cells: 800,
            mesh: PipeMesh::Structured,
        };
        e.serve(Request::Mesh { id: 5, design: d, probe: 0.5 }, &mut |ev| evs.push(ev));
        let Some(Event::Meshed(me)) = evs.pop() else { panic!("no mesh") };
        assert!(me.n_cells <= 800 && me.flow_axis == 2 && me.max_non_ortho_deg < 1e-3, "{} {} {}", me.n_cells, me.flow_axis, me.max_non_ortho_deg);
        assert!(!me.axial.polygons.is_empty() && !me.cross.polygons.is_empty());
        e.serve(Request::Start { id: 5, reynolds: 20.0, mode: Mode::Transient }, &mut |ev| evs.push(ev));
        e.serve(Request::Step { id: 5, steps: 3 }, &mut |ev| evs.push(ev));
        let Some(Event::Progress(p)) = evs.pop() else { panic!("no progress") };
        assert_eq!(p.steps, 3);
        assert_eq!(p.cross_u.len(), me.cross.polygons.len());
        assert!(p.dp > 0.0 && (p.outlet_over_inlet - 1.0).abs() < 1e-8);
        e.serve(Request::Probe { id: 5, probe: 0.9 }, &mut |ev| evs.push(ev));
        assert!(evs.iter().any(|ev| matches!(ev, Event::Cross { .. })));
        // A stale id is ignored.
        let n = evs.len();
        e.serve(Request::Step { id: 4, steps: 1 }, &mut |ev| evs.push(ev));
        assert_eq!(evs.len(), n);
        // A bad design reports an error.
        e.serve(Request::Mesh { id: 6, design: Design { target_cells: 0, ..Design::default() }, probe: 0.5 }, &mut |ev| evs.push(ev));
        assert!(matches!(evs.last(), Some(Event::Error(_))));
    }
}
