//! The physics side of the GUI, kept OFF the UI thread, on the library's
//! worker/thread plumbing ([`dhoby_ghaut::web_demo::link`]).
//!
//! Processing a large ENDF tape is one blocking call of tens of seconds. On the
//! UI thread that freezes the page, so the GUI never does physics: natively a
//! thread runs [`McEngine`], in the browser a Web Worker running this same
//! wasm module does. Each talks to the UI in the same [`Event`]s, so the UI has
//! one code path, and the nuclear data never cross threads: only finished
//! histories and generation reports do, flattened to `f64`s.
//!
//! The engine serves one rung at a time. [`Request::Load`] replaces whatever
//! was loaded; every loading event carries the load's `id`, and the UI ignores
//! events from a load it has since replaced. A power iteration runs ONE
//! generation per [`Request::KeffStep`], so the UI can pause it.
//!
//! Requests are served strictly in order, so the TRISO histories are the
//! same sequence `triso::sim::headless_csv` prints for the same seed — the
//! `the_engine_thread_runs_the_headless_sequence` test pins that.

// The wire format serves the browser build (and the tests).
#![cfg_attr(not(target_arch = "wasm32"), allow(dead_code))]

use crate::history::History;
use crate::keff::{Generation, KeffConfig, KinfGeneration};
use crate::table::{Loaded, Rung};
use outram_mc_libs::geometry::position::{Direction, Position};
use outram_mc_libs::material::speed::SpeedTier;
use outram_mc_libs::physics::keff::{GenerationReport, HistoryCounts};
use outram_mc_libs::physics::track_output::{Track, TrackEvent, TrackState};

/// Seed of the neutron chain the GUI shows.
pub const CHAIN_SEED: u64 = 1;

/// Which nuclear-data tier to process tapes at.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Tier {
    /// NJOY's tolerance 0.001, exact ([`SpeedTier::Fast`], the default).
    Exact,
    /// Tolerance 0.01, an approximation ([`SpeedTier::VeryFast`]).
    Loose,
}

impl Tier {
    pub fn speed(self) -> SpeedTier {
        match self {
            Tier::Exact => SpeedTier::Fast,
            Tier::Loose => SpeedTier::VeryFast,
        }
    }
}

pub enum Request {
    /// Process the rung's tapes (replacing whatever was loaded).
    Load { id: u32, rung: Rung, tier: Tier },
    /// Transport `n` more traced neutrons of the loaded rung; `animate: false`
    /// ones go straight to the statistics ("Run 100 unanimated").
    Run { n: usize, animate: bool },
    /// Start a power iteration, replacing any running one.
    KeffStart(KeffConfig),
    /// Run its next generation.
    KeffStep,
    /// Start the rung's small `k_inf` case at `param` (gh:#549), replacing
    /// any running one. Like a power iteration, it runs ONE generation per
    /// [`Request::KinfStep`], so it streams and the UI stops it by not asking.
    KinfStart { param: f64, cfg: KeffConfig },
    KinfStep,
    /// Send the σ(E) panel's curves (once per load).
    XsCurves,
    /// Rasterise a window of the rung's assembled geometry (gh:#528).
    Raster(crate::raster::RasterReq),
    /// A message for the rung's own demo (gh:#785), read by its module.
    Walk(Vec<f64>),
    /// A request to one member of the HTR-10 core's worker pool (gh:#786);
    /// needs no loaded rung.
    Core(crate::htr10::core::CoreReq),
}

pub enum Event {
    JobStarted { id: u32, index: usize },
    JobDone { id: u32, index: usize, secs: f64 },
    Ready { id: u32 },
    History { h: History, animate: bool },
    /// The source before the first generation of a just-started iteration.
    KeffStarted { sites: Vec<[f32; 2]> },
    Generation(Generation),
    /// The iteration has run every generation.
    KeffDone,
    /// A generation of the `k_inf` case, and its end.
    KinfGeneration(KinfGeneration),
    KinfDone,
    /// The σ(E) panel's curves (empty: the rung has no panel).
    XsCurves(Vec<crate::xs::XsCurve>),
    /// A raster: one byte per pixel ([`crate::raster`]), and how long it took.
    Raster { req: crate::raster::RasterReq, map: Vec<u8>, secs: f64 },
    /// The rung's own demo's answer to a [`Request::Walk`] (gh:#785).
    Walk(Vec<f64>),
    /// From a member of the HTR-10 core's worker pool (gh:#786).
    Core(crate::htr10::core::CoreEv),
    Error(String),
}

/// Serve one non-load request on loaded data.
pub fn serve(l: &mut Loaded, r: Request, post: &mut impl FnMut(Event)) {
    match r {
        Request::Load { .. } | Request::Core(_) => {} // the platform's jobs
        Request::Run { n, animate } => {
            for _ in 0..n {
                post(Event::History { h: l.run_next(), animate });
            }
        }
        Request::KeffStart(cfg) => match l.keff_start(cfg) {
            Ok(sites) => post(Event::KeffStarted { sites }),
            Err(e) => post(Event::Error(e)),
        },
        Request::KeffStep => match l.keff_step() {
            Some(g) => {
                post(Event::Generation(g));
                if l.keff_finished() {
                    post(Event::KeffDone);
                }
            }
            None => post(Event::KeffDone),
        },
        Request::KinfStart { param, cfg } => {
            if let Err(e) = l.kinf_start(param, cfg) {
                post(Event::Error(e));
            }
        }
        Request::KinfStep => match l.kinf_step() {
            Some(g) => {
                let last = g.index + 1 >= g.total;
                post(Event::KinfGeneration(g));
                if last {
                    post(Event::KinfDone);
                }
            }
            None => post(Event::KinfDone),
        },
        Request::XsCurves => post(Event::XsCurves(l.xs_curves())),
        Request::Raster(req) => {
            let t = dhoby_ghaut::web_demo::platform::now_s();
            match l.raster(&req) {
                Ok(map) => post(Event::Raster { req, map, secs: dhoby_ghaut::web_demo::platform::now_s() - t }),
                Err(e) => post(Event::Error(e)),
            }
        }
        Request::Walk(msg) => match l.walk(&msg) {
            Ok(v) => post(Event::Walk(v)),
            Err(e) => post(Event::Error(e)),
        },
    }
}

/// A `k_inf` generation as `f64`s (NaN for "no mean yet").
pub fn encode_kinf(g: &KinfGeneration) -> Vec<f64> {
    vec![g.param, g.index as f64, g.total as f64, g.active as u8 as f64, g.k, g.mean, g.sem]
}

pub fn decode_kinf(v: &[f64]) -> Result<KinfGeneration, String> {
    if v.len() != 7 {
        return Err(format!("k_inf message: {} values", v.len()));
    }
    Ok(KinfGeneration { param: v[0], index: v[1] as usize, total: v[2] as usize, active: v[3] != 0.0, k: v[4], mean: v[5], sem: v[6] })
}

/// The engine: what is loaded, and (in the browser) the newest load asked for.
#[derive(Default)]
pub struct McEngine {
    loaded: Option<Loaded>,
    newest_load: u32,
    /// This engine as a member of the HTR-10 core's pool (gh:#786).
    core: std::sync::Arc<std::sync::RwLock<crate::htr10::core::CoreWorker>>,
}

// ─── Native: an engine thread ────────────────────────────────────────────────

#[cfg(not(target_arch = "wasm32"))]
impl dhoby_ghaut::web_demo::link::NativeEngine for McEngine {
    type Req = Request;
    type Ev = Event;
    fn handle(&mut self, req: Request, post: &mut impl FnMut(Event)) {
        match req {
            Request::Load { id, rung, tier } => {
                self.loaded = None;
                match native_load(id, rung, tier, post) {
                    Ok(l) => {
                        self.loaded = Some(l);
                        post(Event::Ready { id });
                    }
                    Err(e) => post(Event::Error(e)),
                }
            }
            Request::Core(r) => {
                let core = self.core.clone();
                let guard = core.write();
                if let Ok(mut w) = guard {
                    crate::htr10::core::serve_native(&mut w, r, post);
                }
            }
            r => match self.loaded.as_mut() {
                Some(l) => serve(l, r, post),
                None => post(Event::Error("no data loaded".into())),
            },
        }
    }
}

/// Process a rung's tapes from `reference-data/endf/`, posting progress.
#[cfg(not(target_arch = "wasm32"))]
pub fn native_load(id: u32, rung: Rung, tier: Tier, post: &mut impl FnMut(Event)) -> Result<Loaded, String> {
    let mut b = rung.builder(tier);
    for (index, (label, tape)) in rung.jobs_for(rung.tier(tier)).iter().enumerate() {
        post(Event::JobStarted { id, index });
        let t = std::time::Instant::now();
        crate::native_tape(tape).and_then(|bytes| b.step(&bytes)).map_err(|e| format!("{label}: {e}"))?;
        post(Event::JobDone { id, index, secs: t.elapsed().as_secs_f64() });
    }
    b.finish()
}

// ─── Flattening for postMessage ──────────────────────────────────────────────

const HEAD: usize = 15;
const PER_STATE: usize = 12;

fn event_code(e: TrackEvent) -> f64 {
    match e {
        TrackEvent::Born => 0.0,
        TrackEvent::SurfaceCrossing => 1.0,
        TrackEvent::Scatter => 2.0,
        TrackEvent::Fission => 3.0,
        TrackEvent::Absorption => 4.0,
        TrackEvent::Rouletted => 5.0,
        TrackEvent::Leak => 6.0,
        TrackEvent::Lost => 7.0,
    }
}

fn event_from(code: f64) -> Result<TrackEvent, String> {
    Ok(match code as i64 {
        0 => TrackEvent::Born,
        1 => TrackEvent::SurfaceCrossing,
        2 => TrackEvent::Scatter,
        3 => TrackEvent::Fission,
        4 => TrackEvent::Absorption,
        5 => TrackEvent::Rouletted,
        6 => TrackEvent::Leak,
        7 => TrackEvent::Lost,
        c => return Err(format!("unknown track event code {c}")),
    })
}

/// Every field of a [`History`] as `f64`s, so it crosses to the page as one
/// `Float64Array`. Integers stay exact (all far below 2^53); `None` is -1.
pub fn encode_history(h: &History) -> Vec<f64> {
    let st = &h.track.states;
    let mut v = Vec::with_capacity(HEAD + PER_STATE * st.len());
    v.extend([
        h.index as f64,
        h.birth.x,
        h.birth.y,
        h.birth.z,
        h.birth_energy_ev,
        h.outcome.map_or(-1.0, event_code),
        h.scatters as f64,
        h.crossings as f64,
        h.path_cm,
        h.time_of_flight_s,
        h.final_energy_ev,
        h.min_energy_ev,
        h.from_fission as u8 as f64,
        h.track.dropped_states as f64,
        st.len() as f64,
    ]);
    for s in st {
        v.extend([
            s.r.x,
            s.r.y,
            s.r.z,
            s.u.u,
            s.u.v,
            s.u.w,
            s.energy,
            s.time,
            s.weight,
            if s.cell == usize::MAX { -1.0 } else { s.cell as f64 },
            s.material.map_or(-1.0, |m| m as f64),
            event_code(s.event),
        ]);
    }
    v
}

pub fn decode_history(v: &[f64]) -> Result<History, String> {
    if v.len() < HEAD {
        return Err("history message too short".into());
    }
    let n = v[14] as usize;
    if v.len() != HEAD + PER_STATE * n {
        return Err(format!("history message: {} values for {n} states", v.len()));
    }
    let states = v[HEAD..]
        .chunks_exact(PER_STATE)
        .map(|c| {
            Ok(TrackState {
                r: Position::new(c[0], c[1], c[2]),
                u: Direction { u: c[3], v: c[4], w: c[5] },
                energy: c[6],
                time: c[7],
                weight: c[8],
                cell: if c[9] < 0.0 { usize::MAX } else { c[9] as usize },
                material: if c[10] < 0.0 { None } else { Some(c[10] as usize) },
                event: event_from(c[11])?,
            })
        })
        .collect::<Result<Vec<_>, String>>()?;
    Ok(History {
        index: v[0] as u64,
        birth: Position::new(v[1], v[2], v[3]),
        birth_energy_ev: v[4],
        outcome: if v[5] < 0.0 { None } else { Some(event_from(v[5])?) },
        scatters: v[6] as usize,
        crossings: v[7] as usize,
        path_cm: v[8],
        time_of_flight_s: v[9],
        final_energy_ev: v[10],
        min_energy_ev: v[11],
        from_fission: v[12] != 0.0,
        track: Track { states, dropped_states: v[13] as usize },
    })
}

const GEN_HEAD: usize = 15;

/// A generation report and its sites as `f64`s. `None` is NaN.
pub fn encode_generation(g: &Generation) -> Vec<f64> {
    let r = &g.report;
    let c = &r.counts;
    let (m, e) = r.k_mean.unwrap_or((f64::NAN, f64::NAN));
    let mut v = Vec::with_capacity(GEN_HEAD + 2 * g.sites.len());
    v.extend([
        r.index as f64,
        r.active as u8 as f64,
        r.k,
        r.entropy.unwrap_or(f64::NAN),
        m,
        e,
        r.bank_size as f64,
        r.production,
        c.tracked as f64,
        c.leaked as f64,
        c.captured as f64,
        c.fissions_by_energy[0] as f64,
        c.fissions_by_energy[1] as f64,
        c.fissions_by_energy[2] as f64,
        g.sites.len() as f64,
    ]);
    for s in &g.sites {
        v.extend([s[0] as f64, s[1] as f64]);
    }
    v
}

pub fn decode_generation(v: &[f64]) -> Result<Generation, String> {
    if v.len() < GEN_HEAD {
        return Err("generation message too short".into());
    }
    let n = v[14] as usize;
    if v.len() != GEN_HEAD + 2 * n {
        return Err(format!("generation message: {} values for {n} sites", v.len()));
    }
    let opt = |x: f64| if x.is_nan() { None } else { Some(x) };
    let report = GenerationReport {
        index: v[0] as usize,
        active: v[1] != 0.0,
        k: v[2],
        entropy: opt(v[3]),
        k_mean: opt(v[4]).map(|m| (m, v[5])),
        bank_size: v[6] as usize,
        production: v[7],
        counts: HistoryCounts {
            tracked: v[8] as u64,
            leaked: v[9] as u64,
            captured: v[10] as u64,
            fissions_by_energy: [v[11] as u64, v[12] as u64, v[13] as u64],
        },
    };
    let sites = v[GEN_HEAD..].chunks_exact(2).map(|c| [c[0] as f32, c[1] as f32]).collect();
    Ok(Generation { report, sites })
}

// ─── Browser: messages as JS objects, and the worker ─────────────────────────

#[cfg(target_arch = "wasm32")]
mod web {
    use super::*;
    use dhoby_ghaut::web_demo::link::{fetch_promise, fetch_start, js, Message, Poster, WorkerEngine};
    use std::sync::{Arc, RwLock};
    use wasm_bindgen::JsValue;

    impl Message for Request {
        fn to_js(&self) -> JsValue {
            let o = js::object();
            match self {
                Request::Load { id, rung, tier } => {
                    js::set(&o, "kind", "load");
                    js::set(&o, "id", *id as f64);
                    js::set(&o, "rung", dhoby_ghaut::web_demo::lesson::Rung::name(*rung));
                    js::set(&o, "exact", *tier == Tier::Exact);
                }
                Request::Run { n, animate } => {
                    js::set(&o, "kind", "run");
                    js::set(&o, "n", *n as f64);
                    js::set(&o, "animate", *animate);
                }
                Request::KeffStart(c) => {
                    js::set(&o, "kind", "keff_start");
                    let v = [
                        c.n_particles as f64,
                        c.n_inactive as f64,
                        c.n_active as f64,
                        c.seed as f64,
                        c.point_source as u8 as f64,
                        c.want_sites as u8 as f64,
                    ];
                    js::set(&o, "cfg", js::f64s(&v));
                }
                Request::KeffStep => js::set(&o, "kind", "keff_step"),
                Request::KinfStart { param, cfg: c } => {
                    js::set(&o, "kind", "kinf_start");
                    js::set(&o, "param", *param);
                    let v = [c.n_particles as f64, c.n_inactive as f64, c.n_active as f64, c.seed as f64];
                    js::set(&o, "cfg", js::f64s(&v));
                }
                Request::KinfStep => js::set(&o, "kind", "kinf_step"),
                Request::XsCurves => js::set(&o, "kind", "xs_curves"),
                Request::Raster(r) => {
                    js::set(&o, "kind", "raster");
                    js::set(&o, "req", js::f64s(&r.encode()));
                }
                Request::Walk(m) => {
                    js::set(&o, "kind", "walk");
                    js::set(&o, "data", js::f64s(m));
                }
                Request::Core(r) => r.to_js(&o),
            }
            o.into()
        }
        fn from_js(v: &JsValue) -> Result<Self, String> {
            let kind = js::get_str(v, "kind");
            if let Some(r) = crate::htr10::core::CoreReq::from_js(&kind, v) {
                return r.map(Request::Core);
            }
            Ok(match kind.as_str() {
                "load" => Request::Load {
                    id: js::get_f64(v, "id").unwrap_or(0.0) as u32,
                    rung: dhoby_ghaut::web_demo::lesson::parse(&js::get_str(v, "rung")).ok_or("unknown rung")?,
                    tier: if js::get_bool(v, "exact").unwrap_or(false) { Tier::Exact } else { Tier::Loose },
                },
                "run" => Request::Run {
                    n: js::get_f64(v, "n").unwrap_or(1.0) as usize,
                    animate: js::get_bool(v, "animate").unwrap_or(true),
                },
                "keff_start" => {
                    let c = js::get_f64s(v, "cfg");
                    if c.len() != 6 {
                        return Err("bad k_eff config".into());
                    }
                    Request::KeffStart(KeffConfig {
                        n_particles: c[0] as usize,
                        n_inactive: c[1] as usize,
                        n_active: c[2] as usize,
                        seed: c[3] as u64,
                        point_source: c[4] != 0.0,
                        want_sites: c[5] != 0.0,
                    })
                }
                "keff_step" => Request::KeffStep,
                "kinf_start" => {
                    let c = js::get_f64s(v, "cfg");
                    if c.len() != 4 {
                        return Err("bad k_inf config".into());
                    }
                    Request::KinfStart {
                        param: js::get_f64(v, "param").ok_or("k_inf: no parameter")?,
                        cfg: KeffConfig {
                            n_particles: c[0] as usize,
                            n_inactive: c[1] as usize,
                            n_active: c[2] as usize,
                            seed: c[3] as u64,
                            point_source: false,
                            want_sites: false,
                        },
                    }
                }
                "kinf_step" => Request::KinfStep,
                "xs_curves" => Request::XsCurves,
                "raster" => Request::Raster(crate::raster::RasterReq::decode(&js::get_f64s(v, "req"))?),
                "walk" => Request::Walk(js::get_f64s(v, "data")),
                other => return Err(format!("unknown request '{other}'")),
            })
        }
    }

    impl Message for Event {
        fn to_js(&self) -> JsValue {
            let o = js::object();
            match self {
                Event::JobStarted { id, index } => {
                    js::set(&o, "kind", "started");
                    js::set(&o, "id", *id as f64);
                    js::set(&o, "index", *index as f64);
                }
                Event::JobDone { id, index, secs } => {
                    js::set(&o, "kind", "done");
                    js::set(&o, "id", *id as f64);
                    js::set(&o, "index", *index as f64);
                    js::set(&o, "secs", *secs);
                }
                Event::Ready { id } => {
                    js::set(&o, "kind", "ready");
                    js::set(&o, "id", *id as f64);
                }
                Event::History { h, animate } => {
                    js::set(&o, "kind", "history");
                    js::set(&o, "animate", *animate);
                    js::set(&o, "data", js::f64s(&encode_history(h)));
                }
                Event::KeffStarted { sites } => {
                    js::set(&o, "kind", "keff_started");
                    let flat: Vec<f64> = sites.iter().flat_map(|s| [s[0] as f64, s[1] as f64]).collect();
                    js::set(&o, "data", js::f64s(&flat));
                }
                Event::Generation(g) => {
                    js::set(&o, "kind", "generation");
                    js::set(&o, "data", js::f64s(&encode_generation(g)));
                }
                Event::KeffDone => js::set(&o, "kind", "keff_done"),
                Event::KinfGeneration(g) => {
                    js::set(&o, "kind", "kinf_generation");
                    js::set(&o, "data", js::f64s(&encode_kinf(g)));
                }
                Event::KinfDone => js::set(&o, "kind", "kinf_done"),
                Event::Raster { req, map, secs } => {
                    js::set(&o, "kind", "raster");
                    js::set(&o, "req", js::f64s(&req.encode()));
                    js::set(&o, "map", js::u8s(map));
                    js::set(&o, "secs", *secs);
                }
                Event::XsCurves(c) => {
                    let (v, labels) = crate::xs::encode(c);
                    js::set(&o, "kind", "xs_curves");
                    js::set(&o, "data", js::f64s(&v));
                    js::set(&o, "labels", labels.as_str());
                }
                Event::Walk(m) => {
                    js::set(&o, "kind", "walk");
                    js::set(&o, "data", js::f64s(m));
                }
                Event::Core(e) => e.to_js(&o),
                Event::Error(m) => {
                    js::set(&o, "kind", "error");
                    js::set(&o, "message", m.as_str());
                }
            }
            o.into()
        }
        fn from_js(v: &JsValue) -> Result<Self, String> {
            if let Some(e) = crate::htr10::core::CoreEv::from_js(&js::get_str(v, "kind"), v) {
                return e.map(Event::Core);
            }
            let id = || js::get_f64(v, "id").unwrap_or(0.0) as u32;
            let idx = || js::get_f64(v, "index").unwrap_or(0.0) as usize;
            Ok(match js::get_str(v, "kind").as_str() {
                "started" => Event::JobStarted { id: id(), index: idx() },
                "done" => Event::JobDone { id: id(), index: idx(), secs: js::get_f64(v, "secs").unwrap_or(0.0) },
                "ready" => Event::Ready { id: id() },
                "history" => Event::History {
                    h: decode_history(&js::get_f64s(v, "data"))?,
                    animate: js::get_bool(v, "animate").unwrap_or(true),
                },
                "keff_started" => Event::KeffStarted {
                    sites: js::get_f64s(v, "data").chunks_exact(2).map(|c| [c[0] as f32, c[1] as f32]).collect(),
                },
                "generation" => Event::Generation(decode_generation(&js::get_f64s(v, "data"))?),
                "keff_done" => Event::KeffDone,
                "kinf_generation" => Event::KinfGeneration(decode_kinf(&js::get_f64s(v, "data"))?),
                "kinf_done" => Event::KinfDone,
                "raster" => Event::Raster {
                    req: crate::raster::RasterReq::decode(&js::get_f64s(v, "req"))?,
                    map: js::get_u8s(v, "map"),
                    secs: js::get_f64(v, "secs").unwrap_or(0.0),
                },
                "xs_curves" => Event::XsCurves(crate::xs::decode(&js::get_f64s(v, "data"), &js::get_str(v, "labels"))?),
                "walk" => Event::Walk(js::get_f64s(v, "data")),
                "error" => Event::Error(js::get_str(v, "message")),
                other => return Err(format!("unknown message '{other}' from the physics worker")),
            })
        }
    }

    impl WorkerEngine for McEngine {
        type Req = Request;
        type Ev = Event;
        fn error(message: String) -> Event {
            Event::Error(message)
        }
        fn handle(state: &Arc<RwLock<Self>>, req: Request, post: Poster<Event>) {
            match req {
                Request::Load { id, rung, tier } => {
                    if let Ok(mut g) = state.write() {
                        g.loaded = None;
                        g.newest_load = id;
                    }
                    let st = state.clone();
                    wasm_bindgen_futures::spawn_local(async move {
                        match load(id, rung, tier, &st, post).await {
                            Ok(Some(l)) => {
                                if let Ok(mut g) = st.write() {
                                    if g.newest_load == id {
                                        g.loaded = Some(l);
                                        post.post(Event::Ready { id });
                                    }
                                }
                            }
                            Ok(None) => {} // superseded by a newer load
                            Err(e) => post.post(Event::Error(e)),
                        }
                    });
                }
                Request::Core(r) => {
                    if let Ok(core) = state.read().map(|g| g.core.clone()) {
                        crate::htr10::core::web::handle(&core, r, post);
                    }
                }
                r => {
                    if let Ok(mut g) = state.write() {
                        match g.loaded.as_mut() {
                            Some(l) => serve(l, r, &mut |e| post.post(e)),
                            None => post.post(Event::Error("no data loaded".into())),
                        }
                    }
                }
            }
        }
    }

    /// Download and process a rung's tapes. `Ok(None)` if a newer load was
    /// asked for meanwhile (checked between tapes).
    async fn load(id: u32, rung: Rung, tier: Tier, st: &Arc<RwLock<McEngine>>, post: Poster<Event>) -> Result<Option<Loaded>, String> {
        let superseded = || st.read().map(|g| g.newest_load != id).unwrap_or(true);
        let jobs = rung.jobs_for(rung.tier(tier));
        // Every request is issued before the first (blocking) job, so the
        // downloads proceed while the first tape is being processed.
        let urls: Vec<String> = jobs.iter().map(|(_, t)| format!("data/{}", crate::tapes::wire_name(t))).collect();
        let promises: Vec<js_sys::Promise> = urls.iter().map(|u| fetch_start(u)).collect();
        let mut b = rung.builder(tier);
        for (index, (((label, _), p), url)) in jobs.iter().zip(promises).zip(&urls).enumerate() {
            let z = fetch_promise(p, url).await?;
            if superseded() {
                return Ok(None);
            }
            post.post(Event::JobStarted { id, index });
            let t = js_sys::Date::now();
            b.step(&crate::tapes::decompress(&z)?).map_err(|e| format!("{label}: {e}"))?;
            post.post(Event::JobDone { id, index, secs: (js_sys::Date::now() - t) / 1000.0 });
        }
        if superseded() {
            return Ok(None);
        }
        b.finish().map(Some)
    }
}
