//! The physics side of the GUI, kept OFF the UI thread.
//!
//! Processing a large ENDF tape is one blocking call of tens of seconds. On the
//! UI thread that freezes the page — every frame, every click, the progress
//! bar itself — so the GUI never does physics. Natively a thread owns the
//! nuclear data and the neutron chain; in the browser a Web Worker running
//! this same wasm module does (the module finds no `window` there and enters
//! [`run_worker`]). Each talks to the UI in the same [`Event`]s, so the UI has
//! one code path, and the nuclear data never cross threads: only finished
//! histories do, flattened by [`encode_history`].
//!
//! The engine thread runs the chain strictly in order, so the histories are
//! the same sequence [`crate::sim::headless_csv`] prints for the same seed —
//! the `the_engine_thread_runs_the_headless_sequence` test pins that.

// The wire format serves the browser build (and the tests);
// the native engine thread hands over Rust values directly.
#![cfg_attr(not(target_arch = "wasm32"), allow(dead_code))]

use crate::model::{DataBuilder, JOBS};
use crate::sim::{Chain, History};
use outram_mc_libs::geometry::position::{Direction, Position};
use outram_mc_libs::physics::track_output::{Track, TrackEvent, TrackState};
use std::sync::{Arc, RwLock};

/// Seed of the neutron chain the GUI shows.
pub const CHAIN_SEED: u64 = 1;

pub enum Request {
    /// Transport `n` more neutrons; `animate: false` ones go straight to the
    /// statistics ("Run 100 unanimated").
    Run { n: usize, animate: bool },
}

pub enum Event {
    JobStarted { index: usize },
    JobDone { index: usize, secs: f64 },
    Ready,
    History { h: History, animate: bool },
    Error(String),
}

// ─── Flattening a history for postMessage ────────────────────────────────────

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
    Web { worker: web_sys::Worker, inbox: Arc<RwLock<Vec<Event>>> },
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
            Link::Web { worker, .. } => {
                let _ = worker.post_message(&web::request_to_js(&r));
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
    let push = |e: Event| {
        if let Ok(mut m) = mailbox.write() {
            m.events.push(e);
        }
        repaint();
    };
    let mut b = DataBuilder::default();
    for (index, job) in JOBS.iter().enumerate() {
        push(Event::JobStarted { index });
        let t = std::time::Instant::now();
        if let Err(e) = crate::native_tape(job.tape).and_then(|bytes| b.step(&bytes)) {
            return push(Event::Error(format!("{}: {e}", job.label)));
        }
        push(Event::JobDone { index, secs: t.elapsed().as_secs_f64() });
    }
    let phys = match b.finish() {
        Ok(data) => crate::sim::Physics::new(data),
        Err(e) => return push(Event::Error(e)),
    };
    let mut chain = Chain::new(CHAIN_SEED);
    push(Event::Ready);
    loop {
        let requests = mailbox.write().map(|mut m| std::mem::take(&mut m.requests)).unwrap_or_default();
        if requests.is_empty() {
            std::thread::sleep(std::time::Duration::from_millis(3));
            continue;
        }
        for Request::Run { n, animate } in requests {
            for _ in 0..n {
                push(Event::History { h: chain.run_next(&phys), animate });
            }
        }
    }
}

// ─── Browser: a Web Worker running this same module ──────────────────────────

#[cfg(target_arch = "wasm32")]
pub use web::{run_worker, start_web};

#[cfg(target_arch = "wasm32")]
mod web {
    use super::*;
    use crate::model;
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

    pub(super) fn request_to_js(r: &Request) -> JsValue {
        let o = Object::new();
        let Request::Run { n, animate } = r;
        set(&o, "n", (*n as f64).into());
        set(&o, "animate", (*animate).into());
        o.into()
    }

    fn event_to_js(e: &Event) -> JsValue {
        let o = Object::new();
        match e {
            Event::JobStarted { index } => {
                set(&o, "kind", "started".into());
                set(&o, "index", (*index as f64).into());
            }
            Event::JobDone { index, secs } => {
                set(&o, "kind", "done".into());
                set(&o, "index", (*index as f64).into());
                set(&o, "secs", (*secs).into());
            }
            Event::Ready => set(&o, "kind", "ready".into()),
            Event::History { h, animate } => {
                set(&o, "kind", "history".into());
                set(&o, "animate", (*animate).into());
                set(&o, "data", Float64Array::from(encode_history(h).as_slice()).into());
            }
            Event::Error(m) => {
                set(&o, "kind", "error".into());
                set(&o, "message", m.as_str().into());
            }
        }
        o.into()
    }

    fn event_from_js(v: &JsValue) -> Event {
        let kind = Reflect::get(v, &"kind".into()).ok().and_then(|k| k.as_string()).unwrap_or_default();
        let idx = || get_f64(v, "index").unwrap_or(0.0) as usize;
        match kind.as_str() {
            "started" => Event::JobStarted { index: idx() },
            "done" => Event::JobDone { index: idx(), secs: get_f64(v, "secs").unwrap_or(0.0) },
            "ready" => Event::Ready,
            "history" => {
                let animate = Reflect::get(v, &"animate".into()).ok().and_then(|a| a.as_bool()).unwrap_or(true);
                let data = Reflect::get(v, &"data".into())
                    .ok()
                    .and_then(|d| d.dyn_into::<Float64Array>().ok())
                    .map(|a| a.to_vec())
                    .unwrap_or_default();
                match decode_history(&data) {
                    Ok(h) => Event::History { h, animate },
                    Err(e) => Event::Error(e),
                }
            }
            "error" => Event::Error(
                Reflect::get(v, &"message".into()).ok().and_then(|m| m.as_string()).unwrap_or_default(),
            ),
            other => Event::Error(format!("unknown message '{other}' from the physics worker")),
        }
    }

    /// Page side: start the worker and route its messages into the inbox.
    pub fn start_web(ctx: egui::Context) -> Result<Link, String> {
        let opts = web_sys::WorkerOptions::new();
        opts.set_type(web_sys::WorkerType::Module);
        let worker = web_sys::Worker::new_with_options("./worker.js", &opts).map_err(|e| format!("{e:?}"))?;
        let inbox: Arc<RwLock<Vec<Event>>> = Arc::default();
        let (ib, c) = (inbox.clone(), ctx.clone());
        // `Closure<dyn FnMut>` is wasm-bindgen's only callback type: a framework
        // boundary, not a design choice (the workspace otherwise avoids `dyn`).
        let on_message = Closure::<dyn FnMut(web_sys::MessageEvent)>::new(move |ev: web_sys::MessageEvent| {
            if let Ok(mut i) = ib.write() {
                i.push(event_from_js(&ev.data()));
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
        Ok(Link::Web { worker, inbox })
    }

    fn scope() -> web_sys::DedicatedWorkerGlobalScope {
        js_sys::global().unchecked_into()
    }

    fn post(e: &Event) {
        let _ = scope().post_message(&event_to_js(e));
    }

    /// Worker side: entered from `main` when the module finds no `window`.
    pub fn run_worker() {
        std::panic::set_hook(Box::new(|info| {
            let m = info.to_string();
            web_sys::console::error_1(&m.as_str().into());
            post(&Event::Error(format!("physics worker panicked: {m}")));
        }));
        let state: Arc<RwLock<Option<(crate::sim::Physics, Chain)>>> = Arc::default();
        let st = state.clone();
        let on_message = Closure::<dyn FnMut(web_sys::MessageEvent)>::new(move |ev: web_sys::MessageEvent| {
            let data = ev.data();
            let n = get_f64(&data, "n").unwrap_or(1.0) as usize;
            let animate = Reflect::get(&data, &"animate".into()).ok().and_then(|a| a.as_bool()).unwrap_or(true);
            if let Ok(mut g) = st.write() {
                if let Some((phys, chain)) = g.as_mut() {
                    for _ in 0..n {
                        post(&Event::History { h: chain.run_next(phys), animate });
                    }
                }
            }
        });
        scope().set_onmessage(Some(on_message.as_ref().unchecked_ref()));
        on_message.forget();
        wasm_bindgen_futures::spawn_local(async move {
            if let Err(e) = load(state).await {
                post(&Event::Error(e));
            }
        });
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

    async fn load(state: Arc<RwLock<Option<(crate::sim::Physics, Chain)>>>) -> Result<(), String> {
        // Every request is issued before the first (blocking) job, so the
        // downloads proceed while U-235 is being processed.
        let urls: Vec<String> = JOBS.iter().map(|j| format!("data/{}", model::wire_name(j.tape))).collect();
        let promises: Vec<js_sys::Promise> = urls.iter().map(|u| scope().fetch_with_str(u)).collect();
        let mut b = DataBuilder::default();
        for (index, ((job, p), url)) in JOBS.iter().zip(promises).zip(&urls).enumerate() {
            let z = bytes_of(p, url).await?;
            post(&Event::JobStarted { index });
            let t = js_sys::Date::now();
            b.step(&model::decompress(&z)?).map_err(|e| format!("{}: {e}", job.label))?;
            post(&Event::JobDone { index, secs: (js_sys::Date::now() - t) / 1000.0 });
        }
        let phys = crate::sim::Physics::new(b.finish()?);
        if let Ok(mut s) = state.write() {
            *s = Some((phys, Chain::new(CHAIN_SEED)));
        }
        post(&Event::Ready);
        Ok(())
    }
}
