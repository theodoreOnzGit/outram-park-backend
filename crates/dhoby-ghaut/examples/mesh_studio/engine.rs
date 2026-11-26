//! Mesh Studio's engine: meshing and cutting, off the UI thread
//! (`bugis::web_demo::link`: a thread natively, a Web Worker in the browser).
//!
//! Two requests, both answered with one event:
//!
//! - [`Request::Build`] authors the surface and runs cfMesh's staged
//!   tet -> dual -> layers pipeline ([`crate::model::build`]): seconds, the
//!   one long request. The page stays live because it runs here.
//! - [`Request::Section`] cuts one stage's mesh with a plane and returns the
//!   polygons with a value per polygon: milliseconds, from caches of each
//!   stage's `PolyMesh` and per-cell values.
//!
//! Every message crosses as numbers in one `Vec<f64>` plus one string (the
//! wire format below), the same on every target, so the round trip is
//! tested natively (`tests::the_wire_format_round_trips`).

use outram_blender::foam_mesh::TetDualOptions;
use outram_foam_mesh::block_mesh::PolyMesh;
use outram_park_fork_cfmesh::dual::DualKind;
use outram_park_fork_cfmesh::layers::LayerQualityStrategy;
use outram_park_fork_cfmesh::pipeline::staged::PipelineStage;
use outram_park_fork_cfmesh::pipeline::TetSource;

use crate::model::{build, Built, Cleanup, GeomKind, Params};
use crate::section::{bounds, cut, to_poly, values, Colouring};

/// Every stage in pipeline order, with its wire code (its index here).
#[cfg_attr(not(target_arch = "wasm32"), allow(dead_code))] // natively only the round-trip test uses the wire format
pub const STAGES: [PipelineStage; 11] = [
    PipelineStage::Carve,
    PipelineStage::Snap,
    PipelineStage::Tetrahedralise,
    PipelineStage::Delaunay,
    PipelineStage::Dual,
    PipelineStage::Smooth,
    PipelineStage::FeatureCapture,
    PipelineStage::OptimiseFinalMesh,
    PipelineStage::Layers,
    PipelineStage::NamePatches,
    PipelineStage::Final,
];

#[cfg_attr(not(target_arch = "wasm32"), allow(dead_code))] // natively only the round-trip test uses the wire format
fn stage_code(s: PipelineStage) -> f64 {
    STAGES.iter().position(|&t| t == s).unwrap_or(0) as f64
}

#[cfg_attr(not(target_arch = "wasm32"), allow(dead_code))] // natively only the round-trip test uses the wire format
fn stage_of(c: f64) -> PipelineStage {
    STAGES
        .get(c as usize)
        .copied()
        .unwrap_or(PipelineStage::Final)
}

/// The layer strategies the page offers, in wire order.
pub const STRATEGIES: [LayerQualityStrategy; 3] = [
    LayerQualityStrategy::BackOffOnly,
    LayerQualityStrategy::CfMeshOptimise,
    LayerQualityStrategy::SnappyCollapse,
];

/// The dual kinds the page offers, in wire order.
pub const DUALS: [DualKind; 3] = [
    DualKind::MedianPerCorner,
    DualKind::MedianPerPoint,
    DualKind::CellCentre,
];

/// What the page asks the engine to mesh: the shape and the controls the
/// panel exposes. Everything else is [`Params::default`].
#[derive(Clone, Debug, PartialEq)]
pub struct Design {
    pub kind: GeomKind,
    pub cleanup: Cleanup,
    /// Cube edge or sphere radius \[m\] (cylinder: radius = size, height = 2.5 size).
    pub size: f64,
    /// Background (carve) cell size \[m\].
    pub cell_size: f64,
    pub n_layers: usize,
    /// First (wall) layer thickness \[m\].
    pub first_layer: f64,
    /// Layer-to-layer growth ratio.
    pub expansion: f64,
    pub strategy: LayerQualityStrategy,
    pub dual: DualKind,
    /// The mesh type: `true` for the polyhedral dual of the tetrahedra,
    /// `false` for the tetrahedra themselves (layers, when asked for, grow
    /// on either). The pipeline's own switch, `TetDualOptions::dual`.
    pub polyhedral: bool,
    /// Where the tetrahedra come from (gh:#1796): `true` for TetGen's
    /// unstructured quality tetrahedralisation of the surface (the page's
    /// default), `false` for the carved, snapped, centroid-split grid (the
    /// cfMesh port's default, and the headless sweep's).
    pub tetgen: bool,
    /// TetGen's radius-edge ratio bound (`-q`).
    pub radius_edge: f64,
    /// TetGen's largest tetrahedron \[m³\] (`-a`); `None` derives it from
    /// the cell size ([`Design::tet_volume`]).
    pub max_volume: Option<f64>,
}

impl Default for Design {
    fn default() -> Self {
        let p = Params::default();
        Design {
            kind: p.kind,
            cleanup: p.cleanup,
            size: p.size,
            cell_size: p.opts.cell_size,
            n_layers: p.opts.n_layers,
            first_layer: p.opts.first_layer_thickness,
            expansion: p.opts.expansion,
            strategy: p.opts.layer_strategy,
            dual: p.opts.dual_kind,
            polyhedral: p.opts.dual,
            tetgen: true,
            radius_edge: Self::DEFAULT_RADIUS_EDGE,
            max_volume: None,
        }
    }
}

impl Design {
    /// The smallest cell size the page may ask for, as a fraction of the
    /// shape's size: 1/12 keeps the carve near 12^3 cells before the
    /// tetrahedra, a few seconds in a phone's browser.
    pub const MIN_CELLS_ACROSS: f64 = 12.0;

    /// The most boundary layers the page offers. Stacks of 20 or more are
    /// common in wall-resolved CFD (the maintainer's reference pictures,
    /// gh:#1796); every layer adds one cell per wall face.
    pub const MAX_LAYERS: usize = 25;

    /// TetGen's radius-edge bound when the page opens: `-q1.414`, the
    /// bound TetGen's manual recommends for quality meshes (stage 6,
    /// gh:#1795, checked the port against upstream at it on these shapes).
    pub const DEFAULT_RADIUS_EDGE: f64 = 1.414;

    /// The volume of a regular tetrahedron of edge `h` \[m³\]: the
    /// `-a` bound the page derives from a cell size `h`, so TetGen's
    /// interior tetrahedra have edges of about the cell size.
    pub fn regular_tet_volume(h: f64) -> f64 {
        h * h * h / (6.0 * core::f64::consts::SQRT_2)
    }

    /// The `-a` bound this design meshes with \[m³\]: the one asked for,
    /// or the regular tetrahedron of the cell size.
    pub fn tet_volume(&self) -> f64 {
        self.max_volume
            .unwrap_or_else(|| Self::regular_tet_volume(self.cell_size))
    }

    /// The tet source this design asks the pipeline for.
    pub fn tet_source(&self) -> TetSource {
        if self.tetgen {
            TetSource::tetgen(self.radius_edge, Some(self.tet_volume()))
        } else {
            TetSource::CarvedCentroid
        }
    }

    /// A design from the page's URL query (or `--key value` natively):
    /// `shape`, `h`, `n`, `first`, `ratio`, `strategy`, `mesh`, `dual`,
    /// `tets`, `q`, `vol`
    /// (see the doc comment at the top of `app.rs`). Unknown values are
    /// ignored; the result is [`Self::clamped`].
    pub fn from_query(q: &[(String, String)]) -> Self {
        let get = |k: &str| q.iter().find(|(a, _)| a == k).map(|(_, b)| b.clone());
        let mut design = Design::default();
        if let Some(k) = get("shape").and_then(|s| GeomKind::from_slug(&s)) {
            design.kind = k;
            design.cell_size = k.default_cell_size(design.size);
            design.dual = k.default_dual();
        }
        if let Some(h) = get("h").and_then(|s| s.parse().ok()) {
            design.cell_size = h;
        }
        if let Some(n) = get("n").and_then(|s| s.parse().ok()) {
            design.n_layers = n;
        }
        if let Some(t) = get("first").and_then(|s| s.parse().ok()) {
            design.first_layer = t;
        }
        if let Some(r) = get("ratio").and_then(|s| s.parse().ok()) {
            design.expansion = r;
        }
        match get("strategy").as_deref() {
            Some("cfmesh") => design.strategy = STRATEGIES[1],
            Some("snappy") => design.strategy = STRATEGIES[2],
            Some("backoff") => design.strategy = STRATEGIES[0],
            _ => {}
        }
        match get("mesh").as_deref() {
            Some("tet") => design.polyhedral = false,
            Some("poly") => design.polyhedral = true,
            _ => {}
        }
        match get("dual").as_deref() {
            Some("corner") => design.dual = DUALS[0],
            Some("point") => design.dual = DUALS[1],
            Some("centre") => design.dual = DUALS[2],
            _ => {}
        }
        match get("tets").as_deref() {
            Some("tetgen") => design.tetgen = true,
            Some("carved") => design.tetgen = false,
            _ => {}
        }
        if let Some(r) = get("q").and_then(|s| s.parse().ok()) {
            design.radius_edge = r;
        }
        if let Some(v) = get("vol").and_then(|s| s.parse::<f64>().ok()) {
            design.max_volume = Some(v);
        }
        design.clamped()
    }

    /// Clamp to what a phone can mesh in seconds and to valid layer values.
    pub fn clamped(mut self) -> Self {
        self.size = self.size.clamp(0.1, 10.0);
        let span = self.span();
        self.cell_size = self
            .cell_size
            .clamp(span / Self::MIN_CELLS_ACROSS, span / 2.0);
        self.n_layers = self.n_layers.min(Self::MAX_LAYERS);
        self.first_layer = self.first_layer.clamp(1e-4 * span, 0.1 * span);
        self.expansion = self.expansion.clamp(1.0, 2.0);
        // TetGen: a ratio bound it can meet (it needs > 1; below about 1.1
        // refinement may not end), and no tetrahedron smaller than the
        // regular one of the smallest cell size, the same phone limit.
        self.radius_edge = self.radius_edge.clamp(1.1, 4.0);
        let v_lo = Self::regular_tet_volume(span / Self::MIN_CELLS_ACROSS);
        let v_hi = Self::regular_tet_volume(span);
        self.max_volume = self.max_volume.map(|v| {
            if v.is_finite() {
                v.clamp(v_lo, v_hi)
            } else {
                v_hi
            }
        });
        self
    }

    /// The length the cell-size limits are measured against \[m\]: the
    /// largest extent of a primitive, the channel diameter (`size`) of a
    /// reactor shape (whose length would allow cells wider than its gaps).
    pub fn span(&self) -> f64 {
        match self.kind {
            GeomKind::Cylinder => 2.5 * self.size,
            GeomKind::Sphere => 2.0 * self.size,
            _ => self.size,
        }
    }

    /// The model's parameters for this design.
    pub fn params(&self) -> Params {
        let d = self.clone().clamped();
        let base = Params::default();
        Params {
            kind: d.kind,
            size: d.size,
            cyl_radius: d.size,
            cyl_height: 2.5 * d.size,
            cleanup: d.cleanup,
            opts: TetDualOptions {
                cell_size: d.cell_size,
                n_layers: d.n_layers,
                first_layer_thickness: d.first_layer,
                expansion: d.expansion,
                layer_strategy: d.strategy,
                dual_kind: d.dual,
                dual: d.polyhedral,
                tet_source: d.tet_source(),
                ..base.opts
            },
            ..base
        }
    }
}

/// One stage as the page lists it.
#[derive(Clone, Debug, PartialEq)]
pub struct StageInfo {
    pub stage: PipelineStage,
    pub applied: bool,
    pub n_cells: usize,
    /// Whether this stage's cells carry the boundary-layer index.
    pub has_layers: bool,
}

/// What a finished build reports.
#[derive(Clone, Debug, PartialEq)]
pub struct Summary {
    pub id: u64,
    pub cells: usize,
    pub volume: f64,
    pub max_non_orth_deg: f64,
    pub max_skewness: f64,
    pub negative_cells: usize,
    /// Cells per layer index: entry 0 is the core, k the k-th layer.
    pub layer_counts: Vec<usize>,
    pub stages: Vec<StageInfo>,
    pub lo: [f64; 3],
    pub hi: [f64; 3],
    pub build_s: f64,
    /// Wall-layer thickness \[m\]: the total the design asks for, then the
    /// least, median and greatest total thickness of the layer columns that
    /// grew (all 0 with no layer). The back-off strategy shrinks every column
    /// until the quality limits hold, so the grown layers can be much thinner
    /// than asked; the report says so instead of leaving the reader to infer
    /// it from the picture.
    pub layer_thickness: [f64; 4],
    /// The pipeline's stage notes, one per line.
    pub notes: String,
}

/// The thickness `n` layers would total from a first layer `first` growing
/// by `ratio` per layer \[m\].
pub fn requested_thickness(n: usize, first: f64, ratio: f64) -> f64 {
    (0..n).map(|k| first * ratio.powi(k as i32)).sum()
}

/// `[requested, min, median, max]` total column thickness \[m\] over the wall
/// points that grew at least one layer (see [`Summary::layer_thickness`]).
pub fn layer_thickness(
    d: &Design,
    layers: Option<&outram_park_fork_cfmesh::layers::LayerOutcome>,
) -> [f64; 4] {
    let asked = requested_thickness(d.n_layers, d.first_layer, d.expansion);
    let mut t: Vec<f64> = layers
        .map(|o| o.points.iter().filter(|p| p.1 > 0).map(|p| p.2).collect())
        .unwrap_or_default();
    if t.is_empty() {
        return [asked, 0.0, 0.0, 0.0];
    }
    t.sort_by(f64::total_cmp);
    [asked, t[0], t[t.len() / 2], t[t.len() - 1]]
}

/// One cut of one stage.
#[derive(Clone, Debug, PartialEq)]
pub struct Cut {
    pub id: u64,
    pub stage: PipelineStage,
    pub axis: u8,
    pub offset: f64,
    pub colouring: Colouring,
    /// One polygon per cut cell, in-plane coordinates \[m\].
    pub polygons: Vec<Vec<[f64; 2]>>,
    /// The colouring's value for each polygon.
    pub values: Vec<f64>,
    /// The colouring's range over **every** cell of the stage, so the
    /// colour scale does not jump as the plane moves.
    pub min: f64,
    pub max: f64,
}

#[derive(Clone, Debug, PartialEq)]
pub enum Request {
    Build {
        id: u64,
        design: Design,
    },
    Section {
        id: u64,
        stage: PipelineStage,
        axis: u8,
        offset: f64,
        colouring: Colouring,
    },
}

#[derive(Clone, Debug, PartialEq)]
pub enum Event {
    Built(Summary),
    Section(Cut),
    Error(String),
}

/// The engine: the last build and its caches.
#[derive(Default)]
pub struct Engine {
    built: Option<Built>,
    id: u64,
    /// Per stage index: the stage's `PolyMesh`, built on first cut.
    polys: Vec<Option<PolyMesh>>,
    /// Per stage index and colouring code: per-cell values.
    vals: Vec<[Option<Vec<f64>>; 3]>,
}

impl Engine {
    fn stage_index(&self, stage: PipelineStage) -> Option<usize> {
        self.built
            .as_ref()?
            .stages
            .iter()
            .position(|s| s.stage == stage)
    }

    /// Does stage `i` carry the finished mesh's cell labels (so the layer
    /// index applies)?
    fn stage_has_layers(b: &Built, i: usize) -> bool {
        let s = &b.stages[i];
        let after_layers = matches!(
            s.stage,
            PipelineStage::Layers | PipelineStage::NamePatches | PipelineStage::Final
        );
        after_layers && b.layer_index.max > 0 && s.mesh.n_cells == b.mesh.n_cells
    }

    /// Answer `req`, posting one event.
    pub fn serve(&mut self, req: Request, post: &mut impl FnMut(Event)) {
        match req {
            Request::Build { id, design } => {
                let t0 = bugis::web_demo::platform::now_s();
                match build(&design.params()) {
                    Ok(b) => {
                        let (lo, hi) = bounds(&b.mesh);
                        let stages = (0..b.stages.len())
                            .map(|i| StageInfo {
                                stage: b.stages[i].stage,
                                applied: b.stages[i].applied,
                                n_cells: b.stages[i].mesh.n_cells,
                                has_layers: Self::stage_has_layers(&b, i),
                            })
                            .collect();
                        let summary = Summary {
                            id,
                            cells: b.report.cell_count,
                            volume: b.report.total_volume,
                            max_non_orth_deg: b.report.max_non_orthogonality_deg,
                            max_skewness: b.report.max_skewness,
                            negative_cells: b.report.n_negative_volume_cells,
                            layer_counts: b.layer_index.counts(),
                            stages,
                            lo,
                            hi,
                            build_s: bugis::web_demo::platform::now_s() - t0,
                            layer_thickness: layer_thickness(&design, b.report.layers.as_ref()),
                            notes: b.report.stage_notes.join("\n"),
                        };
                        self.polys = vec![None; b.stages.len()];
                        self.vals = vec![[None, None, None]; b.stages.len()];
                        self.built = Some(b);
                        self.id = id;
                        post(Event::Built(summary));
                    }
                    Err(e) => {
                        self.built = None;
                        post(Event::Error(format!("meshing failed: {e}")));
                    }
                }
            }
            Request::Section {
                id,
                stage,
                axis,
                offset,
                colouring,
            } => {
                if id != self.id {
                    return; // a cut of a superseded build: drop it
                }
                match self.section(stage, axis, offset, colouring) {
                    Ok(mut c) => {
                        c.id = id;
                        post(Event::Section(c));
                    }
                    Err(e) => post(Event::Error(e)),
                }
            }
        }
    }

    fn section(
        &mut self,
        stage: PipelineStage,
        axis: u8,
        offset: f64,
        colouring: Colouring,
    ) -> Result<Cut, String> {
        let i = self
            .stage_index(stage)
            .ok_or("no such stage in this build")?;
        let b = self.built.as_ref().ok_or("nothing built yet")?;
        if b.stages[i].mesh.n_cells == 0 {
            // A stage this route did not run and gave nothing (carve and
            // snap on TetGen's tetrahedra, gh:#1796): an empty cut.
            return Ok(Cut {
                id: 0,
                stage,
                axis,
                offset,
                colouring,
                polygons: Vec::new(),
                values: Vec::new(),
                min: 0.0,
                max: 1.0,
            });
        }
        if self.polys[i].is_none() {
            self.polys[i] = Some(to_poly(&b.stages[i].mesh)?);
        }
        let k = colouring.code() as usize;
        if self.vals[i][k].is_none() {
            let layer = Self::stage_has_layers(b, i).then_some(b.layer_index.per_cell.as_slice());
            self.vals[i][k] = Some(values(&b.stages[i].mesh, colouring, layer));
        }
        let (poly, v) = (
            self.polys[i].as_ref().unwrap(),
            self.vals[i][k].as_ref().unwrap(),
        );
        let cut_cells = cut(poly, axis, offset);
        let (min, max) = v
            .iter()
            .fold((f64::INFINITY, f64::NEG_INFINITY), |(a, z), &x| {
                (a.min(x), z.max(x))
            });
        Ok(Cut {
            id: 0,
            stage,
            axis,
            offset,
            colouring,
            values: cut_cells.iter().map(|c| v[c.cell]).collect(),
            polygons: cut_cells.into_iter().map(|c| c.polygon).collect(),
            min: if min.is_finite() { min } else { 0.0 },
            max: if max.is_finite() { max } else { 1.0 },
        })
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
    fn handle(
        state: &std::sync::Arc<std::sync::RwLock<Self>>,
        req: Request,
        post: bugis::web_demo::link::Poster<Event>,
    ) {
        if let Ok(mut e) = state.write() {
            e.serve(req, &mut |ev| post.post(ev));
        }
    }
    fn error(message: String) -> Event {
        Event::Error(message)
    }
}

// ─── Wire format: numbers in one Vec<f64>, text in one string ────────────────

#[cfg_attr(not(target_arch = "wasm32"), allow(dead_code))] // natively only the round-trip test uses the wire format
fn kind_code(k: GeomKind) -> f64 {
    GeomKind::ALL.iter().position(|&x| x == k).unwrap_or(0) as f64
}

#[cfg_attr(not(target_arch = "wasm32"), allow(dead_code))] // natively only the round-trip test uses the wire format
fn cleanup_code(c: Cleanup) -> f64 {
    Cleanup::ALL.iter().position(|&x| x == c).unwrap_or(0) as f64
}

/// A reader over the numbers, failing on a short message rather than
/// panicking.
#[cfg_attr(not(target_arch = "wasm32"), allow(dead_code))] // natively only the round-trip test uses the wire format
struct Words<'a> {
    d: &'a [f64],
    i: usize,
}

#[cfg_attr(not(target_arch = "wasm32"), allow(dead_code))] // natively only the round-trip test uses the wire format
impl Words<'_> {
    fn next(&mut self) -> Result<f64, String> {
        let v = *self.d.get(self.i).ok_or("message too short")?;
        self.i += 1;
        Ok(v)
    }
    fn count(&mut self) -> Result<usize, String> {
        let v = self.next()?;
        if v < 0.0 || v.fract() != 0.0 || v > 1e9 {
            return Err(format!("bad count {v}"));
        }
        Ok(v as usize)
    }
}

#[cfg_attr(not(target_arch = "wasm32"), allow(dead_code))] // natively only the round-trip test uses the wire format
impl Request {
    pub fn to_wire(&self) -> Vec<f64> {
        match self {
            Request::Build { id, design: d } => vec![
                0.0,
                *id as f64,
                kind_code(d.kind),
                cleanup_code(d.cleanup),
                d.size,
                d.cell_size,
                d.n_layers as f64,
                d.first_layer,
                d.expansion,
                STRATEGIES
                    .iter()
                    .position(|&s| s == d.strategy)
                    .unwrap_or(0) as f64,
                DUALS.iter().position(|&s| s == d.dual).unwrap_or(0) as f64,
                f64::from(u8::from(d.polyhedral)),
                f64::from(u8::from(d.tetgen)),
                d.radius_edge,
                d.max_volume.unwrap_or(0.0),
            ],
            Request::Section {
                id,
                stage,
                axis,
                offset,
                colouring,
            } => {
                vec![
                    1.0,
                    *id as f64,
                    stage_code(*stage),
                    f64::from(*axis),
                    *offset,
                    colouring.code(),
                ]
            }
        }
    }

    pub fn from_wire(d: &[f64]) -> Result<Self, String> {
        let mut w = Words { d, i: 0 };
        match w.next()? as i64 {
            0 => {
                let id = w.next()? as u64;
                let kind = *GeomKind::ALL.get(w.count()?).ok_or("bad shape")?;
                let cleanup = *Cleanup::ALL.get(w.count()?).ok_or("bad cleanup")?;
                let (size, cell_size) = (w.next()?, w.next()?);
                let n_layers = w.count()?;
                let (first_layer, expansion) = (w.next()?, w.next()?);
                let strategy = *STRATEGIES.get(w.count()?).ok_or("bad strategy")?;
                let dual = *DUALS.get(w.count()?).ok_or("bad dual")?;
                let polyhedral = w.next()? != 0.0;
                let tetgen = w.next()? != 0.0;
                let radius_edge = w.next()?;
                // 0 on the wire: derived from the cell size.
                let max_volume = Some(w.next()?).filter(|&v| v > 0.0);
                Ok(Request::Build {
                    id,
                    design: Design {
                        kind,
                        cleanup,
                        size,
                        cell_size,
                        n_layers,
                        first_layer,
                        expansion,
                        strategy,
                        dual,
                        polyhedral,
                        tetgen,
                        radius_edge,
                        max_volume,
                    },
                })
            }
            1 => {
                let id = w.next()? as u64;
                let stage = stage_of(w.next()?);
                let axis = w.count()?.min(2) as u8;
                let offset = w.next()?;
                let colouring = Colouring::from_code(w.next()?).ok_or("bad colouring")?;
                Ok(Request::Section {
                    id,
                    stage,
                    axis,
                    offset,
                    colouring,
                })
            }
            k => Err(format!("unknown request {k}")),
        }
    }
}

#[cfg_attr(not(target_arch = "wasm32"), allow(dead_code))] // natively only the round-trip test uses the wire format
impl Event {
    pub fn to_wire(&self) -> (Vec<f64>, String) {
        match self {
            Event::Built(s) => {
                let mut d = vec![
                    0.0,
                    s.id as f64,
                    s.cells as f64,
                    s.volume,
                    s.max_non_orth_deg,
                    s.max_skewness,
                    s.negative_cells as f64,
                    s.build_s,
                ];
                d.extend_from_slice(&s.layer_thickness);
                d.extend_from_slice(&s.lo);
                d.extend_from_slice(&s.hi);
                d.push(s.layer_counts.len() as f64);
                d.extend(s.layer_counts.iter().map(|&c| c as f64));
                d.push(s.stages.len() as f64);
                for st in &s.stages {
                    d.extend([
                        stage_code(st.stage),
                        f64::from(u8::from(st.applied)),
                        st.n_cells as f64,
                        f64::from(u8::from(st.has_layers)),
                    ]);
                }
                (d, s.notes.clone())
            }
            Event::Section(c) => {
                let mut d = vec![
                    1.0,
                    c.id as f64,
                    stage_code(c.stage),
                    f64::from(c.axis),
                    c.offset,
                    c.colouring.code(),
                    c.min,
                    c.max,
                ];
                d.push(c.polygons.len() as f64);
                for (p, v) in c.polygons.iter().zip(&c.values) {
                    d.push(*v);
                    d.push(p.len() as f64);
                    for q in p {
                        d.extend_from_slice(q);
                    }
                }
                (d, String::new())
            }
            Event::Error(m) => (vec![2.0], m.clone()),
        }
    }

    pub fn from_wire(d: &[f64], text: &str) -> Result<Self, String> {
        let mut w = Words { d, i: 0 };
        match w.next()? as i64 {
            0 => {
                let id = w.next()? as u64;
                let cells = w.count()?;
                let (volume, max_non_orth_deg, max_skewness) = (w.next()?, w.next()?, w.next()?);
                let negative_cells = w.count()?;
                let build_s = w.next()?;
                let layer_thickness = [w.next()?, w.next()?, w.next()?, w.next()?];
                let lo = [w.next()?, w.next()?, w.next()?];
                let hi = [w.next()?, w.next()?, w.next()?];
                let n = w.count()?;
                let layer_counts = (0..n).map(|_| w.count()).collect::<Result<_, _>>()?;
                let n = w.count()?;
                let mut stages = Vec::with_capacity(n);
                for _ in 0..n {
                    let stage = stage_of(w.next()?);
                    let applied = w.next()? != 0.0;
                    let n_cells = w.count()?;
                    let has_layers = w.next()? != 0.0;
                    stages.push(StageInfo {
                        stage,
                        applied,
                        n_cells,
                        has_layers,
                    });
                }
                Ok(Event::Built(Summary {
                    id,
                    cells,
                    volume,
                    max_non_orth_deg,
                    max_skewness,
                    negative_cells,
                    layer_counts,
                    stages,
                    lo,
                    hi,
                    build_s,
                    layer_thickness,
                    notes: text.to_string(),
                }))
            }
            1 => {
                let id = w.next()? as u64;
                let stage = stage_of(w.next()?);
                let axis = w.count()?.min(2) as u8;
                let offset = w.next()?;
                let colouring = Colouring::from_code(w.next()?).ok_or("bad colouring")?;
                let (min, max) = (w.next()?, w.next()?);
                let n = w.count()?;
                let mut polygons = Vec::with_capacity(n);
                let mut vals = Vec::with_capacity(n);
                for _ in 0..n {
                    vals.push(w.next()?);
                    let m = w.count()?;
                    polygons.push(
                        (0..m)
                            .map(|_| Ok([w.next()?, w.next()?]))
                            .collect::<Result<Vec<_>, String>>()?,
                    );
                }
                Ok(Event::Section(Cut {
                    id,
                    stage,
                    axis,
                    offset,
                    colouring,
                    polygons,
                    values: vals,
                    min,
                    max,
                }))
            }
            2 => Ok(Event::Error(text.to_string())),
            k => Err(format!("unknown event {k}")),
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

#[cfg(test)]
mod tests {
    use super::*;

    fn serve_one(e: &mut Engine, req: Request) -> Event {
        let mut got = Vec::new();
        e.serve(req, &mut |ev| got.push(ev));
        assert_eq!(got.len(), 1, "one event per request");
        got.remove(0)
    }

    /// The four mesh types the page offers all build valid meshes: the
    /// tetrahedra or their polyhedral dual, each with or without boundary
    /// layers.
    ///
    /// **Methodology.** The default 2 m box (cell size 0.5 m, 3 layers
    /// when asked for), built through the engine on the **carved** tet
    /// source (`tetgen: false`; the TetGen source is pinned in
    /// `every_mesh_type_on_tetgen_tets_of_the_reactor_shapes`).
    /// **Results (2026-11-26, structured carve-and-split tets):**
    ///
    /// | mesh type | core cells | per layer |
    /// |---|---|---|
    /// | tets | 2304 | none |
    /// | tets + 3 layers | 2304 | 384 |
    /// | dual | 429 | none |
    /// | dual + 3 layers | 429 | 1152 |
    ///
    /// 384 = 6 faces × 64 boundary triangles; 1152 = the dual's wall faces.
    /// Pinned so a change to either route is seen.
    #[test]
    fn every_mesh_type_builds_a_valid_mesh() {
        for (polyhedral, n_layers, want) in [
            (false, 0, vec![2304]),
            (false, 3, vec![2304, 384, 384, 384]),
            (true, 0, vec![429]),
            (true, 3, vec![429, 1152, 1152, 1152]),
        ] {
            let d = Design {
                polyhedral,
                n_layers,
                tetgen: false,
                ..Design::default()
            };
            let mut e = Engine::default();
            let Event::Built(s) = serve_one(&mut e, Request::Build { id: 1, design: d }) else {
                panic!("no build for polyhedral={polyhedral}, n={n_layers}")
            };
            assert_eq!(s.negative_cells, 0, "polyhedral={polyhedral}, n={n_layers}");
            assert_eq!(
                s.layer_counts, want,
                "polyhedral={polyhedral}, n={n_layers}"
            );
            let dual = s.stages.iter().find(|x| x.stage == PipelineStage::Dual);
            assert_eq!(dual.is_some_and(|x| x.applied), polyhedral, "dual stage");
        }
    }

    /// The four mesh types on TetGen's tetrahedra (gh:#1796), on the three
    /// reactor shapes, with the page's defaults.
    ///
    /// **Methodology.** `Design::from_query` with `shape=<s>&tets=tetgen`
    /// and `mesh=tet|poly`, `n=0|3`: size 2 m, TetGen `-pq1.414` with
    /// `-a` the regular tetrahedron of the cell size (pipe h 0.25 m, so
    /// 1.84e-3 m³; fuel channel and pebble h 1/6 m, 5.46e-4 m³),
    /// cell-centre dual with corner feature points, back-off layers (first
    /// 20 mm, x 1.3) on the walls. Pinned: the cells per layer index
    /// (entry 0 the core), and that every mesh is free of negative cells.
    ///
    /// **Results (2026-11-26, release, native).**
    ///
    /// | shape | tets | tets + 3 | dual | dual + 3 |
    /// |---|---|---|---|---|
    /// | pipe | 18020 | 18020, **no layer** | 3562 | 3562 + 3 × 1088 |
    /// | fuel channel | 30804 | 30804, **no layer** | 6399 | 6399 + 3 × 2638 |
    /// | pebble | 35347 | 35347, **no layer** | 7186 | 7186, **no layer** |
    ///
    /// Back-off grows no layer on any TetGen tetrahedral mesh (nor does
    /// `CfMeshOptimise`, measured: the stage is refused), nor on the
    /// pebble's dual; the report's stage notes say so, and so does the
    /// pinned `notes` check. `CfMeshOptimise` grows all three on the
    /// pebble's dual (`cfmesh_layers_grow_on_the_tetgen_dual_of_the_pebble`).
    #[test]
    fn every_mesh_type_on_tetgen_tets_of_the_reactor_shapes() {
        for (shape, tets, dual, dual_layers) in [
            ("pipe", 18020, 3562, Some(1088)),
            ("fuel-channel", 30804, 6399, Some(2638)),
            ("pebble", 35347, 7186, None),
        ] {
            for (mesh, n) in [("tet", 0usize), ("tet", 3), ("poly", 0), ("poly", 3)] {
                let q: Vec<(String, String)> =
                    [("shape", shape), ("tets", "tetgen"), ("mesh", mesh)]
                        .iter()
                        .map(|(k, v)| (k.to_string(), v.to_string()))
                        .chain([("n".to_string(), n.to_string())])
                        .collect();
                let d = Design::from_query(&q);
                assert!(d.tetgen && d.n_layers == n, "{q:?}");
                let what = format!("{shape} {mesh} n={n}");
                let mut e = Engine::default();
                let Event::Built(s) = serve_one(&mut e, Request::Build { id: 1, design: d }) else {
                    panic!("no build: {what}")
                };
                assert_eq!(s.negative_cells, 0, "{what}");
                let core = if mesh == "tet" { tets } else { dual };
                let want = match (mesh, n, dual_layers) {
                    ("poly", 3, Some(k)) => vec![core, k, k, k],
                    _ => vec![core],
                };
                assert_eq!(s.layer_counts, want, "{what}");
                if n > 0 && want.len() == 1 {
                    assert!(
                        s.notes.contains("boundary layers added none"),
                        "{what}: {}",
                        s.notes
                    );
                }
                assert!(
                    s.notes.starts_with("tet source TetGen -pq1.414/3.5a"),
                    "{what}: {}",
                    s.notes
                );
                let carve = s
                    .stages
                    .iter()
                    .find(|x| x.stage == PipelineStage::Carve)
                    .unwrap();
                assert!(!carve.applied && carve.n_cells == 0, "{what}");
                // A cut of the stage TetGen replaced is empty, not an error.
                let Event::Section(c) = serve_one(
                    &mut e,
                    Request::Section {
                        id: 1,
                        stage: PipelineStage::Carve,
                        axis: 2,
                        offset: 0.0,
                        colouring: Colouring::Layer,
                    },
                ) else {
                    panic!("{what}: no cut")
                };
                assert!(c.polygons.is_empty(), "{what}");
            }
        }
    }

    /// `CfMeshOptimise` grows every layer on the pebble's TetGen dual,
    /// where back-off grows none (gh:#1796's "Pebble" question).
    ///
    /// **Methodology.** `shape=pebble&tets=tetgen&mesh=poly&n=3&strategy=cfmesh`,
    /// otherwise as `every_mesh_type_on_tetgen_tets_of_the_reactor_shapes`.
    /// **Results (2026-11-26, release, native):** 15052 cells, 7186 core
    /// and 2622 per layer, median column 31 % of the 79.8 mm asked, max
    /// non-orthogonality 68.7°, skewness 3.20, 93 s with two other builds
    /// running (the carved pebble's dual: 2354 per layer, 37 %, 78.2°,
    /// 216 s, four runs at a time).
    #[test]
    fn cfmesh_layers_grow_on_the_tetgen_dual_of_the_pebble() {
        let q: Vec<(String, String)> = [
            ("shape", "pebble"),
            ("tets", "tetgen"),
            ("mesh", "poly"),
            ("n", "3"),
            ("strategy", "cfmesh"),
        ]
        .iter()
        .map(|(k, v)| (k.to_string(), v.to_string()))
        .collect();
        let mut e = Engine::default();
        let Event::Built(s) = serve_one(
            &mut e,
            Request::Build {
                id: 1,
                design: Design::from_query(&q),
            },
        ) else {
            panic!("no build")
        };
        assert_eq!(s.negative_cells, 0);
        assert_eq!(s.layer_counts, vec![7186, 2622, 2622, 2622]);
    }

    /// A 20-layer stack on the pipe's TetGen dual (the maintainer's
    /// reference shows about 20 thin layers, gh:#1796).
    ///
    /// **Methodology.** `shape=pipe&tets=tetgen&mesh=poly&n=20&first=0.002&ratio=1.1`
    /// (asked total 114.5 mm) under each strategy. **Results (2026-11-26,
    /// release, native):** `CfMeshOptimise` grows all 20 layers on every
    /// wall face (3562 core + 20 × 1088 = 25322 cells, median column 52 %
    /// of the asked thickness, max non-orthogonality 54.7°, skewness 2.20,
    /// about 11 s); back-off grows **none** (it grows all 3 of a 3-layer
    /// stack on the same mesh).
    #[test]
    fn twenty_layers_grow_on_the_pipe_with_the_cfmesh_optimiser() {
        let mut got = Vec::new();
        for strategy in ["cfmesh", "backoff"] {
            let q: Vec<(String, String)> = [
                ("shape", "pipe"),
                ("tets", "tetgen"),
                ("mesh", "poly"),
                ("n", "20"),
                ("first", "0.002"),
                ("ratio", "1.1"),
                ("strategy", strategy),
            ]
            .iter()
            .map(|(k, v)| (k.to_string(), v.to_string()))
            .collect();
            let mut e = Engine::default();
            let Event::Built(s) = serve_one(
                &mut e,
                Request::Build {
                    id: 1,
                    design: Design::from_query(&q),
                },
            ) else {
                panic!("no build")
            };
            assert_eq!(s.negative_cells, 0, "{strategy}");
            got.push(s.layer_counts);
        }
        let mut want = vec![3562];
        want.extend([1088; 20]);
        assert_eq!(got, vec![want, vec![3562]]);
    }

    /// The query keys of gh:#1796 (`tets`, `q`, `vol`) and the clamps.
    #[test]
    fn the_tet_source_query_keys_and_clamps() {
        let q = |pairs: &[(&str, &str)]| {
            let v: Vec<(String, String)> = pairs
                .iter()
                .map(|(k, v)| (k.to_string(), v.to_string()))
                .collect();
            Design::from_query(&v)
        };
        let d = q(&[]);
        assert!(d.tetgen, "the page's default is TetGen");
        assert_eq!(d.radius_edge, Design::DEFAULT_RADIUS_EDGE);
        assert_eq!(d.max_volume, None);
        assert_eq!(d.tet_volume(), Design::regular_tet_volume(d.cell_size));
        assert!(!q(&[("tets", "carved")]).tetgen);
        assert_eq!(
            q(&[("tets", "carved")]).tet_source(),
            TetSource::CarvedCentroid
        );
        let d = q(&[("q", "2"), ("vol", "0.01")]);
        assert_eq!((d.radius_edge, d.max_volume), (2.0, Some(0.01)));
        assert_eq!(d.tet_source(), TetSource::tetgen(2.0, Some(0.01)));
        // Clamped: a ratio TetGen can meet, a volume a phone can mesh.
        let d = q(&[("q", "1.0"), ("vol", "1e-12")]);
        assert_eq!(d.radius_edge, 1.1);
        assert_eq!(
            d.max_volume,
            Some(Design::regular_tet_volume(
                d.span() / Design::MIN_CELLS_ACROSS
            ))
        );
        // ~~The crate's default is untouched: the headless sweep stays
        // carved.~~ CHANGED 2026-11-26 (maintainer: "set the default crate
        // wide to tetgen"): the headless sweep takes the crate default,
        // TetGen sized by its cell size.
        assert_eq!(
            crate::model::Params::default().opts.tet_source,
            TetSource::default()
        );
        assert!(matches!(TetSource::default(), TetSource::TetGenAtCellSize { .. }));
    }

    /// Requests and events survive the wire exactly (every `f64` is carried
    /// as itself), including a real build's summary and a real cut.
    #[test]
    fn the_wire_format_round_trips() {
        let req = Request::Build {
            id: 7,
            design: Design {
                n_layers: 2,
                strategy: LayerQualityStrategy::SnappyCollapse,
                polyhedral: false,
                ..Design::default()
            },
        };
        assert_eq!(Request::from_wire(&req.to_wire()).unwrap(), req);
        // The tet source and its bounds (gh:#1796), derived and given.
        for (tetgen, max_volume) in [(false, None), (true, None), (true, Some(0.003))] {
            let req = Request::Build {
                id: 8,
                design: Design {
                    tetgen,
                    radius_edge: 1.7,
                    max_volume,
                    ..Design::default()
                },
            };
            assert_eq!(Request::from_wire(&req.to_wire()).unwrap(), req);
        }
        let req = Request::Section {
            id: 7,
            stage: PipelineStage::Dual,
            axis: 1,
            offset: 0.25,
            colouring: Colouring::Skewness,
        };
        assert_eq!(Request::from_wire(&req.to_wire()).unwrap(), req);

        let mut e = Engine::default();
        let built = serve_one(
            &mut e,
            Request::Build {
                id: 1,
                design: Design::default(),
            },
        );
        let (d, t) = built.to_wire();
        assert_eq!(Event::from_wire(&d, &t).unwrap(), built);
        let cut = serve_one(
            &mut e,
            Request::Section {
                id: 1,
                stage: PipelineStage::Final,
                axis: 2,
                offset: 0.1,
                colouring: Colouring::Layer,
            },
        );
        let (d, t) = cut.to_wire();
        assert_eq!(Event::from_wire(&d, &t).unwrap(), cut);
        let err = Event::Error("x".into());
        let (d, t) = err.to_wire();
        assert_eq!(Event::from_wire(&d, &t).unwrap(), err);
        assert!(
            Request::from_wire(&[0.0, 1.0]).is_err(),
            "a short message is an error, not a panic"
        );
    }

    /// The engine's summary matches the model, and cuts follow the stages:
    /// the carve has no layers, the finished mesh has every layer.
    #[test]
    fn the_requested_thickness_is_the_geometric_sum() {
        assert_eq!(requested_thickness(0, 0.02, 1.3), 0.0);
        assert!((requested_thickness(3, 0.02, 1.3) - 0.0798).abs() < 1e-15);
        assert!((requested_thickness(4, 0.01, 1.0) - 0.04).abs() < 1e-15);
    }

    /// On the carved source, whose carve stage has cells to cut (the
    /// TetGen source's empty carve is checked in
    /// `every_mesh_type_on_tetgen_tets_of_the_reactor_shapes`).
    #[test]
    fn the_engine_reports_the_build_and_cuts_each_stage() {
        let carved = Design {
            tetgen: false,
            ..Design::default()
        };
        let mut e = Engine::default();
        let Event::Built(s) = serve_one(
            &mut e,
            Request::Build {
                id: 3,
                design: carved.clone(),
            },
        ) else {
            panic!("no build")
        };
        let b = build(&carved.params()).unwrap();
        assert_eq!(s.cells, b.report.cell_count);
        assert_eq!(s.layer_counts, b.layer_index.counts());
        assert_eq!(
            s.stages.first().map(|x| x.stage),
            Some(PipelineStage::Carve)
        );
        assert_eq!(s.stages.last().map(|x| x.stage), Some(PipelineStage::Final));
        assert!(s.stages.last().unwrap().has_layers && !s.stages[0].has_layers);
        let d = Design::default();
        let [asked, lo, med, hi] = s.layer_thickness;
        assert_eq!(
            asked,
            requested_thickness(d.n_layers, d.first_layer, d.expansion)
        );
        assert!(
            0.0 < lo && lo <= med && med <= hi && hi <= asked * (1.0 + 1e-12),
            "{:?}",
            s.layer_thickness
        );

        let cut_of = |e: &mut Engine, stage| match serve_one(
            e,
            Request::Section {
                id: 3,
                stage,
                axis: 2,
                offset: 0.1,
                colouring: Colouring::Layer,
            },
        ) {
            Event::Section(c) => c,
            other => panic!("{other:?}"),
        };
        let carve = cut_of(&mut e, PipelineStage::Carve);
        assert!(!carve.polygons.is_empty() && carve.values.iter().all(|&v| v == 0.0));
        let fin = cut_of(&mut e, PipelineStage::Final);
        assert_eq!(fin.max as usize, b.layer_index.max as usize);
        assert_eq!(fin.polygons.len(), fin.values.len());

        // A cut of a superseded build is dropped, not answered.
        let mut got = 0;
        e.serve(
            Request::Section {
                id: 2,
                stage: PipelineStage::Final,
                axis: 2,
                offset: 0.0,
                colouring: Colouring::Layer,
            },
            &mut |_| got += 1,
        );
        assert_eq!(got, 0);
    }
}
