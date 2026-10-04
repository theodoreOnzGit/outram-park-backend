//! The physics side of the GUI, kept OFF the UI thread.
//!
//! Processing a large ENDF tape is one blocking call of tens of seconds. On the
//! UI thread that freezes the page — every frame, every click, the progress
//! bar itself — so the GUI never does physics. Natively a thread owns the
//! nuclear data and the neutrons; in the browser a Web Worker running this
//! same wasm module does (the module finds no `window` there and enters
//! [`run_worker`]). Each talks to the UI in the same [`Event`]s, so the UI has
//! one code path, and the nuclear data never cross threads: only finished
//! histories and generation reports do, flattened to `f64`s.
//!
//! The engine serves one rung at a time. [`Request::Load`] replaces whatever
//! was loaded; every loading event carries the load's `id`, and the UI ignores
//! events from a load it has since replaced.
//!
//! The engine runs requests strictly in order, so the TRISO histories are the
//! same sequence [`crate::triso::sim::headless_csv`] prints for the same seed —
//! the `the_engine_thread_runs_the_headless_sequence` test pins that.

// The wire format serves the browser build (and the tests);
// the native engine thread hands over Rust values directly.
#![cfg_attr(not(target_arch = "wasm32"), allow(dead_code))]

use crate::godiva;
use crate::history::History;
use crate::rungs::Rung;
use crate::triso;
use outram_mc_libs::geometry::position::{Direction, Position};
use outram_mc_libs::material::speed::SpeedTier;
use outram_mc_libs::physics::keff::{GenerationReport, HistoryCounts};
use outram_mc_libs::physics::track_output::{Track, TrackEvent, TrackState};
use std::sync::{Arc, RwLock};

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
    fn code(self) -> f64 {
        match self {
            Tier::Exact => 0.0,
            Tier::Loose => 1.0,
        }
    }
    fn from_code(c: f64) -> Tier {
        if c == 0.0 { Tier::Exact } else { Tier::Loose }
    }
}

pub enum Request {
    /// Process the rung's tapes (replacing whatever was loaded).
    Load { id: u32, rung: Rung, tier: Tier },
    /// Transport `n` more traced neutrons of the loaded rung; `animate: false`
    /// ones go straight to the statistics ("Run 100 unanimated").
    Run { n: usize, animate: bool },
    /// Start a power iteration (Godiva), replacing any running one.
    KeffStart(godiva::sim::KeffConfig),
    /// Run its next generation.
    KeffStep,
}

pub enum Event {
    JobStarted { id: u32, index: usize },
    JobDone { id: u32, index: usize, secs: f64 },
    Ready { id: u32 },
    History { h: History, animate: bool },
    /// The source before the first generation of a just-started iteration.
    KeffStarted { sites: Vec<[f32; 2]> },
    Generation(godiva::sim::Generation),
    /// The iteration has run every generation.
    KeffDone,
    Error(String),
}

/// The tapes of a rung, `(label, file)`, in processing order.
pub fn jobs(rung: Rung) -> Vec<(&'static str, &'static str)> {
    match rung {
        Rung::Triso => triso::model::JOBS.iter().map(|j| (j.label, j.tape)).collect(),
        Rung::Godiva => godiva::model::JOBS.iter().map(|j| (j.label, j.tape)).collect(),
    }
}

/// The tier a rung is processed at. The TRISO rung always uses its own
/// [`triso::model::SPEED`]; Godiva uses what was asked for.
pub fn effective_tier(rung: Rung, tier: Tier) -> Tier {
    match rung {
        Rung::Triso => Tier::Loose,
        Rung::Godiva => tier,
    }
}

/// Incremental processing of a rung's tapes.
#[allow(clippy::large_enum_variant)]
pub enum Builder {
    Triso(triso::model::DataBuilder),
    Godiva(godiva::model::DataBuilder),
}

impl Builder {
    pub fn new(rung: Rung, tier: Tier) -> Self {
        match rung {
            Rung::Triso => Builder::Triso(triso::model::DataBuilder::default()),
            Rung::Godiva => Builder::Godiva(godiva::model::DataBuilder::new(tier.speed())),
        }
    }
    pub fn step(&mut self, bytes: &[u8]) -> Result<(), String> {
        match self {
            Builder::Triso(b) => b.step(bytes),
            Builder::Godiva(b) => b.step(bytes),
        }
    }
    pub fn finish(self) -> Result<Loaded, String> {
        Ok(match self {
            Builder::Triso(b) => Loaded::Triso { phys: triso::sim::Physics::new(b.finish()?), chain: triso::sim::Chain::new(CHAIN_SEED) },
            Builder::Godiva(b) => Loaded::Godiva {
                phys: godiva::sim::Physics::new(b.finish()?),
                chain: godiva::sim::Chain::new(CHAIN_SEED),
                keff: None,
            },
        })
    }
}

/// The engine's state once a rung's data are processed.
#[allow(clippy::large_enum_variant)]
pub enum Loaded {
    Triso { phys: triso::sim::Physics, chain: triso::sim::Chain },
    Godiva { phys: godiva::sim::Physics, chain: godiva::sim::Chain, keff: Option<godiva::sim::Keff> },
}

impl Loaded {
    /// Serve one (non-load) request, posting what it produces.
    pub fn serve(&mut self, r: Request, post: &mut impl FnMut(Event)) {
        match (self, r) {
            (_, Request::Load { .. }) => {} // handled by the platform loader
            (Loaded::Triso { phys, chain }, Request::Run { n, animate }) => {
                for _ in 0..n {
                    post(Event::History { h: chain.run_next(phys), animate });
                }
            }
            (Loaded::Godiva { phys, chain, .. }, Request::Run { n, animate }) => {
                for _ in 0..n {
                    post(Event::History { h: chain.run_next(phys), animate });
                }
            }
            (Loaded::Godiva { keff, .. }, Request::KeffStart(cfg)) => {
                let k = godiva::sim::Keff::new(cfg);
                post(Event::KeffStarted { sites: if cfg.want_sites { k.initial_sites() } else { Vec::new() } });
                *keff = Some(k);
            }
            (Loaded::Godiva { phys, keff, .. }, Request::KeffStep) => match keff.as_mut().and_then(|k| k.step(phys)) {
                Some(g) => {
                    post(Event::Generation(g));
                    if keff.as_ref().is_some_and(|k| k.finished()) {
                        post(Event::KeffDone);
                    }
                }
                None => post(Event::KeffDone),
            },
            (Loaded::Triso { .. }, Request::KeffStart(_) | Request::KeffStep) => {
                post(Event::Error("the TRISO rung has no k_eff mode".into()))
            }
        }
    }
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
pub fn encode_generation(g: &godiva::sim::Generation) -> Vec<f64> {
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

pub fn decode_generation(v: &[f64]) -> Result<godiva::sim::Generation, String> {
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
    Ok(godiva::sim::Generation { report, sites })
}

// ─── The UI's handle on the engine ───────────────────────────────────────────

/// What the native engine thread and the UI share. `Arc<RwLock<_>>`, per the
/// workspace Rust rules; the thread polls `requests`, the UI drains `events`.
#[cfg(not(target_arch = "wasm32"))]
#[derive(Default)]
pub struct Mailbox {
    pub events: Vec<Event>,
    pub requests: Vec<Request>,
}

pub enum Link {
    #[cfg(not(target_arch = "wasm32"))]
    Native(Arc<RwLock<Mailbox>>),
    #[cfg(target_arch = "wasm32")]
    /// `outbox` holds requests until the worker says it is listening: a
    /// module worker sets its `onmessage` only after the wasm has loaded, and
    /// a message dispatched before that is dropped.
    Web { worker: web_sys::Worker, inbox: Arc<RwLock<Vec<Event>>>, outbox: Arc<RwLock<Option<Vec<wasm_bindgen::JsValue>>>> },
}

impl Link {
    pub fn send(&self, r: Request) {
        match self {
            #[cfg(not(target_arch = "wasm32"))]
            Link::Native(m) => {
                if let Ok(mut m) = m.write() {
                    m.requests.push(r);
                }
            }
            #[cfg(target_arch = "wasm32")]
            Link::Web { worker, outbox, .. } => {
                let msg = web::request_to_js(&r);
                if let Ok(mut o) = outbox.write() {
                    if let Some(queue) = o.as_mut() {
                        queue.push(msg);
                        return;
                    }
                }
                let _ = worker.post_message(&msg);
            }
        }
    }

    pub fn drain(&self) -> Vec<Event> {
        match self {
            #[cfg(not(target_arch = "wasm32"))]
            Link::Native(m) => m.write().map(|mut m| std::mem::take(&mut m.events)).unwrap_or_default(),
            #[cfg(target_arch = "wasm32")]
            Link::Web { inbox, .. } => inbox.write().map(|mut i| std::mem::take(&mut *i)).unwrap_or_default(),
        }
    }
}

// ─── Native: an engine thread ────────────────────────────────────────────────

#[cfg(not(target_arch = "wasm32"))]
pub fn start_native(repaint: impl Fn() + Send + 'static) -> Link {
    let mailbox = Arc::new(RwLock::new(Mailbox::default()));
    let m = mailbox.clone();
    std::thread::spawn(move || native_engine(&m, repaint));
    Link::Native(mailbox)
}

#[cfg(not(target_arch = "wasm32"))]
fn native_engine(mailbox: &Arc<RwLock<Mailbox>>, repaint: impl Fn()) {
    let mut post = |e: Event| {
        if let Ok(mut m) = mailbox.write() {
            m.events.push(e);
        }
        repaint();
    };
    let mut loaded: Option<Loaded> = None;
    loop {
        let requests = mailbox.write().map(|mut m| std::mem::take(&mut m.requests)).unwrap_or_default();
        if requests.is_empty() {
            std::thread::sleep(std::time::Duration::from_millis(3));
            continue;
        }
        for r in requests {
            match r {
                Request::Load { id, rung, tier } => {
                    loaded = None;
                    match native_load(id, rung, tier, &mut post) {
                        Ok(l) => {
                            loaded = Some(l);
                            post(Event::Ready { id });
                        }
                        Err(e) => post(Event::Error(e)),
                    }
                }
                r => match loaded.as_mut() {
                    Some(l) => l.serve(r, &mut post),
                    None => post(Event::Error("no data loaded".into())),
                },
            }
        }
    }
}

#[cfg(not(target_arch = "wasm32"))]
pub fn native_load(id: u32, rung: Rung, tier: Tier, post: &mut impl FnMut(Event)) -> Result<Loaded, String> {
    let mut b = Builder::new(rung, effective_tier(rung, tier));
    for (index, (label, tape)) in jobs(rung).into_iter().enumerate() {
        post(Event::JobStarted { id, index });
        let t = std::time::Instant::now();
        crate::native_tape(tape).and_then(|bytes| b.step(&bytes)).map_err(|e| format!("{label}: {e}"))?;
        post(Event::JobDone { id, index, secs: t.elapsed().as_secs_f64() });
    }
    b.finish()
}

// ─── Browser: a Web Worker running this same module ──────────────────────────

#[cfg(target_arch = "wasm32")]
pub use web::{run_worker, start_web};

#[cfg(target_arch = "wasm32")]
mod web {
    use super::*;
    use js_sys::{Float64Array, Object, Reflect};
    use wasm_bindgen::closure::Closure;
    use wasm_bindgen::{JsCast as _, JsValue};
    use wasm_bindgen_futures::JsFuture;

    fn set(o: &Object, k: &str, v: JsValue) {
        let _ = Reflect::set(o, &k.into(), &v);
    }
    fn get_f64(o: &JsValue, k: &str) -> Option<f64> {
        Reflect::get(o, &k.into()).ok().and_then(|v| v.as_f64())
    }
    fn get_str(o: &JsValue, k: &str) -> String {
        Reflect::get(o, &k.into()).ok().and_then(|v| v.as_string()).unwrap_or_default()
    }
    fn get_array(o: &JsValue, k: &str) -> Vec<f64> {
        Reflect::get(o, &k.into())
            .ok()
            .and_then(|d| d.dyn_into::<Float64Array>().ok())
            .map(|a| a.to_vec())
            .unwrap_or_default()
    }

    pub(super) fn request_to_js(r: &Request) -> JsValue {
        let o = Object::new();
        match r {
            Request::Load { id, rung, tier } => {
                set(&o, "kind", "load".into());
                set(&o, "id", (*id as f64).into());
                set(&o, "rung", rung.info().name.into());
                set(&o, "tier", tier.code().into());
            }
            Request::Run { n, animate } => {
                set(&o, "kind", "run".into());
                set(&o, "n", (*n as f64).into());
                set(&o, "animate", (*animate).into());
            }
            Request::KeffStart(c) => {
                set(&o, "kind", "keff_start".into());
                let v = [
                    c.n_particles as f64,
                    c.n_inactive as f64,
                    c.n_active as f64,
                    c.seed as f64,
                    c.point_source as u8 as f64,
                    c.want_sites as u8 as f64,
                ];
                set(&o, "cfg", Float64Array::from(v.as_slice()).into());
            }
            Request::KeffStep => set(&o, "kind", "keff_step".into()),
        }
        o.into()
    }

    fn request_from_js(v: &JsValue) -> Result<Request, String> {
        Ok(match get_str(v, "kind").as_str() {
            "load" => Request::Load {
                id: get_f64(v, "id").unwrap_or(0.0) as u32,
                rung: Rung::parse(&get_str(v, "rung")).ok_or("unknown rung")?,
                tier: Tier::from_code(get_f64(v, "tier").unwrap_or(1.0)),
            },
            "run" => Request::Run {
                n: get_f64(v, "n").unwrap_or(1.0) as usize,
                animate: Reflect::get(v, &"animate".into()).ok().and_then(|a| a.as_bool()).unwrap_or(true),
            },
            "keff_start" => {
                let c = get_array(v, "cfg");
                if c.len() != 6 {
                    return Err("bad k_eff config".into());
                }
                Request::KeffStart(godiva::sim::KeffConfig {
                    n_particles: c[0] as usize,
                    n_inactive: c[1] as usize,
                    n_active: c[2] as usize,
                    seed: c[3] as u64,
                    point_source: c[4] != 0.0,
                    want_sites: c[5] != 0.0,
                })
            }
            "keff_step" => Request::KeffStep,
            other => return Err(format!("unknown request '{other}'")),
        })
    }

    fn event_to_js(e: &Event) -> JsValue {
        let o = Object::new();
        match e {
            Event::JobStarted { id, index } => {
                set(&o, "kind", "started".into());
                set(&o, "id", (*id as f64).into());
                set(&o, "index", (*index as f64).into());
            }
            Event::JobDone { id, index, secs } => {
                set(&o, "kind", "done".into());
                set(&o, "id", (*id as f64).into());
                set(&o, "index", (*index as f64).into());
                set(&o, "secs", (*secs).into());
            }
            Event::Ready { id } => {
                set(&o, "kind", "ready".into());
                set(&o, "id", (*id as f64).into());
            }
            Event::History { h, animate } => {
                set(&o, "kind", "history".into());
                set(&o, "animate", (*animate).into());
                set(&o, "data", Float64Array::from(encode_history(h).as_slice()).into());
            }
            Event::KeffStarted { sites } => {
                set(&o, "kind", "keff_started".into());
                let flat: Vec<f64> = sites.iter().flat_map(|s| [s[0] as f64, s[1] as f64]).collect();
                set(&o, "data", Float64Array::from(flat.as_slice()).into());
            }
            Event::Generation(g) => {
                set(&o, "kind", "generation".into());
                set(&o, "data", Float64Array::from(encode_generation(g).as_slice()).into());
            }
            Event::KeffDone => set(&o, "kind", "keff_done".into()),
            Event::Error(m) => {
                set(&o, "kind", "error".into());
                set(&o, "message", m.as_str().into());
            }
        }
        o.into()
    }

    fn event_from_js(v: &JsValue) -> Event {
        let id = || get_f64(v, "id").unwrap_or(0.0) as u32;
        let idx = || get_f64(v, "index").unwrap_or(0.0) as usize;
        match get_str(v, "kind").as_str() {
            "started" => Event::JobStarted { id: id(), index: idx() },
            "done" => Event::JobDone { id: id(), index: idx(), secs: get_f64(v, "secs").unwrap_or(0.0) },
            "ready" => Event::Ready { id: id() },
            "history" => {
                let animate = Reflect::get(v, &"animate".into()).ok().and_then(|a| a.as_bool()).unwrap_or(true);
                match decode_history(&get_array(v, "data")) {
                    Ok(h) => Event::History { h, animate },
                    Err(e) => Event::Error(e),
                }
            }
            "keff_started" => Event::KeffStarted {
                sites: get_array(v, "data").chunks_exact(2).map(|c| [c[0] as f32, c[1] as f32]).collect(),
            },
            "generation" => match decode_generation(&get_array(v, "data")) {
                Ok(g) => Event::Generation(g),
                Err(e) => Event::Error(e),
            },
            "keff_done" => Event::KeffDone,
            "error" => Event::Error(get_str(v, "message")),
            other => Event::Error(format!("unknown message '{other}' from the physics worker")),
        }
    }

    /// Page side: start the worker and route its messages into the inbox.
    pub fn start_web(ctx: egui::Context) -> Result<Link, String> {
        let opts = web_sys::WorkerOptions::new();
        opts.set_type(web_sys::WorkerType::Module);
        let worker = web_sys::Worker::new_with_options("./worker.js", &opts).map_err(|e| format!("{e:?}"))?;
        let inbox: Arc<RwLock<Vec<Event>>> = Arc::default();
        let outbox: Arc<RwLock<Option<Vec<JsValue>>>> = Arc::new(RwLock::new(Some(Vec::new())));
        let (ib, ob, w, c) = (inbox.clone(), outbox.clone(), worker.clone(), ctx.clone());
        // `Closure<dyn FnMut>` is wasm-bindgen's only callback type: a framework
        // boundary, not a design choice (the workspace otherwise avoids `dyn`).
        let on_message = Closure::<dyn FnMut(web_sys::MessageEvent)>::new(move |ev: web_sys::MessageEvent| {
            let data = ev.data();
            if get_str(&data, "kind") == "hello" {
                // The worker is listening: send what was queued, then go direct.
                let queued = ob.write().ok().and_then(|mut o| o.take()).unwrap_or_default();
                for m in queued {
                    let _ = w.post_message(&m);
                }
                return;
            }
            if let Ok(mut i) = ib.write() {
                i.push(event_from_js(&data));
            }
            c.request_repaint();
        });
        worker.set_onmessage(Some(on_message.as_ref().unchecked_ref()));
        on_message.forget();
        let (ib, c) = (inbox.clone(), ctx);
        let on_error = Closure::<dyn FnMut(web_sys::ErrorEvent)>::new(move |ev: web_sys::ErrorEvent| {
            if let Ok(mut i) = ib.write() {
                i.push(Event::Error(format!("physics worker: {}", ev.message())));
            }
            c.request_repaint();
        });
        worker.set_onerror(Some(on_error.as_ref().unchecked_ref()));
        on_error.forget();
        Ok(Link::Web { worker, inbox, outbox })
    }

    fn scope() -> web_sys::DedicatedWorkerGlobalScope {
        js_sys::global().unchecked_into()
    }

    fn post(e: Event) {
        let _ = scope().post_message(&event_to_js(&e));
    }

    /// What the worker holds: the loaded rung, and the id of the newest load
    /// asked for (an older load still downloading checks it and stops).
    #[derive(Default)]
    struct WorkerState {
        loaded: Option<Loaded>,
        newest_load: u32,
    }

    /// Worker side: entered from `main` when the module finds no `window`.
    pub fn run_worker() {
        std::panic::set_hook(Box::new(|info| {
            let m = info.to_string();
            web_sys::console::error_1(&m.as_str().into());
            post(Event::Error(format!("physics worker panicked: {m}")));
        }));
        let state: Arc<RwLock<WorkerState>> = Arc::default();
        let st = state.clone();
        let on_message = Closure::<dyn FnMut(web_sys::MessageEvent)>::new(move |ev: web_sys::MessageEvent| {
            match request_from_js(&ev.data()) {
                Ok(Request::Load { id, rung, tier }) => {
                    if let Ok(mut g) = st.write() {
                        g.loaded = None;
                        g.newest_load = id;
                    }
                    let st = st.clone();
                    wasm_bindgen_futures::spawn_local(async move {
                        match load(id, rung, tier, st.clone()).await {
                            Ok(Some(l)) => {
                                if let Ok(mut g) = st.write() {
                                    if g.newest_load == id {
                                        g.loaded = Some(l);
                                        post(Event::Ready { id });
                                    }
                                }
                            }
                            Ok(None) => {} // superseded
                            Err(e) => post(Event::Error(e)),
                        }
                    });
                }
                Ok(r) => {
                    if let Ok(mut g) = st.write() {
                        match g.loaded.as_mut() {
                            Some(l) => l.serve(r, &mut post),
                            None => post(Event::Error("no data loaded".into())),
                        }
                    }
                }
                Err(e) => post(Event::Error(e)),
            }
        });
        scope().set_onmessage(Some(on_message.as_ref().unchecked_ref()));
        on_message.forget();
        // Tell the page it can send: anything it sent earlier was queued there.
        let hello = Object::new();
        set(&hello, "kind", "hello".into());
        let _ = scope().post_message(&hello);
    }

    async fn bytes_of(p: js_sys::Promise, url: &str) -> Result<Vec<u8>, String> {
        let err = |e: JsValue| format!("{url}: {e:?}");
        let resp: web_sys::Response = JsFuture::from(p).await.map_err(err)?.dyn_into().map_err(err)?;
        if !resp.ok() {
            return Err(format!("{url}: HTTP {}", resp.status()));
        }
        let buf = JsFuture::from(resp.array_buffer().map_err(err)?).await.map_err(err)?;
        Ok(js_sys::Uint8Array::new(&buf).to_vec())
    }

    /// Download and process a rung's tapes. `Ok(None)` if a newer load was
    /// asked for meanwhile.
    async fn load(id: u32, rung: Rung, tier: Tier, st: Arc<RwLock<WorkerState>>) -> Result<Option<Loaded>, String> {
        let superseded = || st.read().map(|g| g.newest_load != id).unwrap_or(true);
        let jobs = jobs(rung);
        // Every request is issued before the first (blocking) job, so the
        // downloads proceed while the first tape is being processed.
        let urls: Vec<String> = jobs.iter().map(|(_, t)| format!("data/{}", crate::tapes::wire_name(t))).collect();
        let promises: Vec<js_sys::Promise> = urls.iter().map(|u| scope().fetch_with_str(u)).collect();
        let mut b = Builder::new(rung, effective_tier(rung, tier));
        for (index, (((label, _), p), url)) in jobs.iter().zip(promises).zip(&urls).enumerate() {
            let z = bytes_of(p, url).await?;
            if superseded() {
                return Ok(None);
            }
            post(Event::JobStarted { id, index });
            let t = js_sys::Date::now();
            b.step(&crate::tapes::decompress(&z)?).map_err(|e| format!("{label}: {e}"))?;
            post(Event::JobDone { id, index, secs: (js_sys::Date::now() - t) / 1000.0 });
        }
        if superseded() {
            return Ok(None);
        }
        b.finish().map(Some)
    }
}
