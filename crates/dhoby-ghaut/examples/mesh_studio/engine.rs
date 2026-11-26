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
        }
    }
}

impl Design {
    /// The smallest cell size the page may ask for, as a fraction of the
    /// shape's size: 1/12 keeps the carve near 12^3 cells before the
    /// tetrahedra, a few seconds in a phone's browser.
    pub const MIN_CELLS_ACROSS: f64 = 12.0;

    /// Clamp to what a phone can mesh in seconds and to valid layer values.
    pub fn clamped(mut self) -> Self {
        self.size = self.size.clamp(0.1, 10.0);
        let span = self.span();
        self.cell_size = self
            .cell_size
            .clamp(span / Self::MIN_CELLS_ACROSS, span / 2.0);
        self.n_layers = self.n_layers.min(5);
        self.first_layer = self.first_layer.clamp(1e-4 * span, 0.1 * span);
        self.expansion = self.expansion.clamp(1.0, 2.0);
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

    /// Requests and events survive the wire exactly (every `f64` is carried
    /// as itself), including a real build's summary and a real cut.
    #[test]
    fn the_wire_format_round_trips() {
        let req = Request::Build {
            id: 7,
            design: Design {
                n_layers: 2,
                strategy: LayerQualityStrategy::SnappyCollapse,
                ..Design::default()
            },
        };
        assert_eq!(Request::from_wire(&req.to_wire()).unwrap(), req);
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

    #[test]
    fn the_engine_reports_the_build_and_cuts_each_stage() {
        let mut e = Engine::default();
        let Event::Built(s) = serve_one(
            &mut e,
            Request::Build {
                id: 3,
                design: Design::default(),
            },
        ) else {
            panic!("no build")
        };
        let b = build(&Design::default().params()).unwrap();
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
