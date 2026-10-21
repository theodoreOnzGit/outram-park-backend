//! The physics side, kept OFF the UI thread on the library's plumbing
//! ([`dhoby_ghaut::web_demo::link`]): a thread natively, a Web Worker running
//! this same wasm module in the browser.
//!
//! Every request is short: one neutron traced both ways, or one generation
//! of one method of "Run many". Processing the tapes is the long part; in the
//! browser it yields between tapes (each download is awaited), natively it
//! runs on the engine thread.

// The wire format serves the browser build (and the tests).
#![cfg_attr(not(target_arch = "wasm32"), allow(dead_code))]

use crate::model;
use crate::physics::{self, Birth, GenResult, Method, Physics, Run, RunConfig, Trace};
use outram_mc_libs::pebble_beds::delta_tracking::Majorant;

pub enum Request {
    /// Process the tapes and build the majorant (replacing anything loaded).
    Load { id: u32 },
    /// Trace neutron `seed` both ways; the delta pane flies on the majorant
    /// times `factor` (1 = the bound).
    Trace { seed: u64, factor: f64 },
    /// Start a "Run many" (replacing any running one).
    RunStart(RunConfig),
    /// Run that method's next generation.
    RunStep(Method),
}

pub enum Event {
    JobStarted {
        id: u32,
        index: usize,
    },
    JobDone {
        id: u32,
        index: usize,
        secs: f64,
    },
    /// Data processed; the majorant took `majorant_secs` and has `points` nodes.
    Ready {
        id: u32,
        majorant_secs: f64,
        points: usize,
    },
    Trace {
        seed: u64,
        factor: f64,
        birth: Birth,
        surface: Trace,
        delta: Trace,
    },
    Gen(GenResult),
    /// Where the loaded data came from (processed now or this browser's
    /// cache) and any cache notes, gh:#818.
    DataInfo(String),
    Error(String),
}

/// What loads: the `triso` rung's tapes, then the majorant (one more job).
pub fn job_labels() -> Vec<&'static str> {
    let mut v: Vec<&'static str> = model::JOBS.iter().map(|j| j.label).collect();
    v.push("majorant");
    v
}

/// Rough cost of each job, for the progress bar only: the `triso` rung's
/// browser timings (2026-10-03) and this demo's majorant.
pub fn job_weights() -> Vec<f64> {
    vec![
        35.1, 28.5, 0.2, 0.1, 0.2, 0.1, 0.1, 0.3, 0.3, 0.3, 21.5, 20.0,
    ]
}

/// The engine: what is loaded, a running "Run many", the scaled majorant
/// last asked for, and (in the browser) the newest load asked for.
#[derive(Default)]
pub struct Engine {
    phys: Option<Physics>,
    run: Option<Run>,
    low: Option<(f64, Majorant)>,
    newest_load: u32,
    /// Natively, what this engine processed (gh:#818); in the browser the
    /// cache is IndexedDB, read per load.
    #[cfg(not(target_arch = "wasm32"))]
    store: crate::processed_cache::DataStore,
}

/// One line on where a load's data came from, and the cache's notes
/// (gh:#818): [`Event::DataInfo`].
pub fn data_info(store: &mut crate::processed_cache::DataStore) -> String {
    let (sources, notes) = store.take_report();
    let mut s = format!(
        "Nuclear data: {}.",
        dhoby_ghaut::web_demo::data_cache::Source::summarize(sources.iter().map(|(_, s)| s))
    );
    for n in notes {
        s.push(' ');
        s.push_str(&n);
    }
    s
}

impl Engine {
    /// Serve one request on loaded data.
    pub fn serve(&mut self, r: Request, post: &mut impl FnMut(Event)) {
        let Some(phys) = self.phys.as_ref() else {
            post(Event::Error("no data loaded".into()));
            return;
        };
        match r {
            Request::Load { .. } => {} // the platform loader's job
            Request::Trace { seed, factor } => {
                let b = physics::birth(phys, seed);
                let surface = physics::trace_surface(phys, b);
                let delta = if factor == 1.0 {
                    physics::trace_delta(phys, b, &phys.majorant)
                } else {
                    if self.low.as_ref().is_none_or(|(f, _)| *f != factor) {
                        self.low = Some((factor, phys.majorant.scaled(factor)));
                    }
                    let (_, m) = self.low.as_ref().expect("just set");
                    physics::trace_delta(phys, b, m)
                };
                post(Event::Trace {
                    seed,
                    factor,
                    birth: b,
                    surface,
                    delta,
                });
            }
            Request::RunStart(cfg) => self.run = Some(Run::new(phys, cfg)),
            Request::RunStep(m) => match self.run.as_mut() {
                Some(run) => match run.step(phys, m) {
                    Some(g) => post(Event::Gen(g)),
                    None => post(Event::Error(format!(
                        "{} has no generation left",
                        m.label()
                    ))),
                },
                None => post(Event::Error("no run started".into())),
            },
        }
    }

    fn set_loaded(&mut self, p: Physics) {
        self.phys = Some(p);
        self.run = None;
        self.low = None;
    }
}

// ─── Native: an engine thread ────────────────────────────────────────────────

#[cfg(not(target_arch = "wasm32"))]
impl dhoby_ghaut::web_demo::link::NativeEngine for Engine {
    type Req = Request;
    type Ev = Event;
    fn handle(&mut self, req: Request, post: &mut impl FnMut(Event)) {
        match req {
            Request::Load { id } => {
                self.phys = None;
                let mut b = model::DataBuilder::default();
                let mut index = 0;
                while let Some(job) = b.next_job() {
                    post(Event::JobStarted { id, index });
                    let t = std::time::Instant::now();
                    let store = &mut self.store;
                    if let Err(e) = crate::native_tape(job.tape).and_then(|bytes| {
                        store.begin_tape(&bytes);
                        b.step(&bytes, store)
                    }) {
                        post(Event::Error(format!("{}: {e}", job.label)));
                        return;
                    }
                    post(Event::JobDone {
                        id,
                        index,
                        secs: t.elapsed().as_secs_f64(),
                    });
                    index += 1;
                }
                post(Event::DataInfo(data_info(&mut self.store)));
                post(Event::JobStarted { id, index });
                match b.finish() {
                    Ok(data) => {
                        let p = Physics::new(data);
                        let (secs, points) = (p.majorant_secs, p.majorant.len());
                        post(Event::JobDone { id, index, secs });
                        self.set_loaded(p);
                        post(Event::Ready {
                            id,
                            majorant_secs: secs,
                            points,
                        });
                    }
                    Err(e) => post(Event::Error(e)),
                }
            }
            r => self.serve(r, post),
        }
    }
}

// ─── Browser: the worker and the wire ────────────────────────────────────────

#[cfg(target_arch = "wasm32")]
mod web {
    use super::*;
    use crate::wire;
    use dhoby_ghaut::web_demo::link::{fetch_promise, fetch_start, js, Message, Poster, WorkerEngine};
    use outram_mc_libs::geometry::position::{Direction, Position};
    use std::sync::{Arc, RwLock};
    use wasm_bindgen::JsValue;

    impl Message for Request {
        fn to_js(&self) -> JsValue {
            let o = js::object();
            match self {
                Request::Load { id } => {
                    js::set(&o, "kind", "load");
                    js::set(&o, "id", *id as f64);
                }
                Request::Trace { seed, factor } => {
                    js::set(&o, "kind", "trace");
                    js::set(&o, "seed", *seed as f64);
                    js::set(&o, "factor", *factor);
                }
                Request::RunStart(c) => {
                    js::set(&o, "kind", "run_start");
                    js::set(&o, "cfg", js::f64s(&wire::encode_run(c)));
                }
                Request::RunStep(m) => {
                    js::set(&o, "kind", "run_step");
                    js::set(&o, "method", m.code());
                }
            }
            o.into()
        }
        fn from_js(v: &JsValue) -> Result<Self, String> {
            Ok(match js::get_str(v, "kind").as_str() {
                "load" => Request::Load {
                    id: js::get_f64(v, "id").unwrap_or(0.0) as u32,
                },
                "trace" => Request::Trace {
                    seed: js::get_f64(v, "seed").unwrap_or(1.0) as u64,
                    factor: js::get_f64(v, "factor").unwrap_or(1.0),
                },
                "run_start" => Request::RunStart(wire::decode_run(&js::get_f64s(v, "cfg"))?),
                "run_step" => {
                    Request::RunStep(Method::from_code(js::get_f64(v, "method").unwrap_or(-1.0))?)
                }
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
                Event::Ready {
                    id,
                    majorant_secs,
                    points,
                } => {
                    js::set(&o, "kind", "ready");
                    js::set(&o, "id", *id as f64);
                    js::set(&o, "secs", *majorant_secs);
                    js::set(&o, "points", *points as f64);
                }
                Event::Trace {
                    seed,
                    factor,
                    birth,
                    surface,
                    delta,
                } => {
                    js::set(&o, "kind", "trace");
                    js::set(&o, "seed", *seed as f64);
                    js::set(&o, "factor", *factor);
                    let b = [
                        birth.r.x,
                        birth.r.y,
                        birth.r.z,
                        birth.u.u,
                        birth.u.v,
                        birth.u.w,
                        birth.e,
                        birth.seed as f64,
                    ];
                    js::set(&o, "birth", js::f64s(&b));
                    js::set(&o, "surface", js::f64s(&wire::encode_trace(surface)));
                    js::set(&o, "delta", js::f64s(&wire::encode_trace(delta)));
                }
                Event::Gen(g) => {
                    js::set(&o, "kind", "gen");
                    js::set(&o, "data", js::f64s(&wire::encode_gen(g)));
                }
                Event::DataInfo(m) => {
                    js::set(&o, "kind", "data_info");
                    js::set(&o, "message", m.as_str());
                }
                Event::Error(m) => {
                    js::set(&o, "kind", "error");
                    js::set(&o, "message", m.as_str());
                }
            }
            o.into()
        }
        fn from_js(v: &JsValue) -> Result<Self, String> {
            let id = || js::get_f64(v, "id").unwrap_or(0.0) as u32;
            let idx = || js::get_f64(v, "index").unwrap_or(0.0) as usize;
            Ok(match js::get_str(v, "kind").as_str() {
                "started" => Event::JobStarted {
                    id: id(),
                    index: idx(),
                },
                "data_info" => Event::DataInfo(js::get_str(v, "message")),
                "done" => Event::JobDone {
                    id: id(),
                    index: idx(),
                    secs: js::get_f64(v, "secs").unwrap_or(0.0),
                },
                "ready" => Event::Ready {
                    id: id(),
                    majorant_secs: js::get_f64(v, "secs").unwrap_or(0.0),
                    points: js::get_f64(v, "points").unwrap_or(0.0) as usize,
                },
                "trace" => {
                    let b = js::get_f64s(v, "birth");
                    if b.len() != 8 {
                        return Err("trace: bad birth".into());
                    }
                    Event::Trace {
                        seed: js::get_f64(v, "seed").unwrap_or(0.0) as u64,
                        factor: js::get_f64(v, "factor").unwrap_or(1.0),
                        birth: Birth {
                            r: Position::new(b[0], b[1], b[2]),
                            u: Direction {
                                u: b[3],
                                v: b[4],
                                w: b[5],
                            },
                            e: b[6],
                            seed: b[7] as u64,
                        },
                        surface: wire::decode_trace(&js::get_f64s(v, "surface"))?,
                        delta: wire::decode_trace(&js::get_f64s(v, "delta"))?,
                    }
                }
                "gen" => Event::Gen(wire::decode_gen(&js::get_f64s(v, "data"))?),
                "error" => Event::Error(js::get_str(v, "message")),
                other => return Err(format!("unknown message '{other}' from the physics worker")),
            })
        }
    }

    impl WorkerEngine for Engine {
        type Req = Request;
        type Ev = Event;
        fn error(message: String) -> Event {
            Event::Error(message)
        }
        fn handle(state: &Arc<RwLock<Self>>, req: Request, post: Poster<Event>) {
            match req {
                Request::Load { id } => {
                    if let Ok(mut g) = state.write() {
                        g.phys = None;
                        g.newest_load = id;
                    }
                    let st = state.clone();
                    wasm_bindgen_futures::spawn_local(async move {
                        match load(id, &st, post).await {
                            Ok(Some(p)) => {
                                if let Ok(mut g) = st.write() {
                                    if g.newest_load == id {
                                        let (secs, points) = (p.majorant_secs, p.majorant.len());
                                        g.set_loaded(p);
                                        post.post(Event::Ready {
                                            id,
                                            majorant_secs: secs,
                                            points,
                                        });
                                    }
                                }
                            }
                            Ok(None) => {}
                            Err(e) => post.post(Event::Error(e)),
                        }
                    });
                }
                r => {
                    if let Ok(mut g) = state.write() {
                        g.serve(r, &mut |e| post.post(e));
                    }
                }
            }
        }
    }

    /// Download the tapes the Monte Carlo demo publishes (`../monte-carlo/data/`),
    /// process them one at a time, then build the majorant. `Ok(None)` if a
    /// newer load was asked for meanwhile.
    async fn load(
        id: u32,
        st: &Arc<RwLock<Engine>>,
        post: Poster<Event>,
    ) -> Result<Option<Physics>, String> {
        let superseded = || st.read().map(|g| g.newest_load != id).unwrap_or(true);
        let urls: Vec<String> = model::JOBS
            .iter()
            .map(|j| format!("../monte-carlo/data/{}", crate::tapes::wire_name(j.tape)))
            .collect();
        // Every download is issued before the first (blocking) job.
        let promises: Vec<js_sys::Promise> = urls.iter().map(|u| fetch_start(u)).collect();
        let mut b = model::DataBuilder::default();
        // The shared cache (gh:#818), as the Monte Carlo demo's loads use it.
        let mut store = crate::processed_cache::DataStore::browser();
        let mut health = crate::processed_cache::web::Health::default();
        for (index, (p, url)) in promises.into_iter().zip(&urls).enumerate() {
            let z = fetch_promise(p, url).await?;
            if superseded() {
                return Ok(None);
            }
            post.post(Event::JobStarted { id, index });
            let t = js_sys::Date::now();
            let bytes = crate::tapes::decompress(&z)?;
            crate::processed_cache::web::before_step(&mut store, &bytes, &mut health).await;
            b.step(&bytes, &mut store)
                .map_err(|e| format!("{}: {e}", model::JOBS[index].label))?;
            let secs = (js_sys::Date::now() - t) / 1000.0;
            crate::processed_cache::web::after_step(&mut store, &mut health).await;
            post.post(Event::JobDone { id, index, secs });
        }
        post.post(Event::DataInfo(data_info(&mut store)));
        if superseded() {
            return Ok(None);
        }
        let index = model::JOBS.len();
        post.post(Event::JobStarted { id, index });
        // Let the "started" message go out before the blocking majorant.
        let _ =
            wasm_bindgen_futures::JsFuture::from(js_sys::Promise::resolve(&JsValue::NULL)).await;
        let p = Physics::new(b.finish()?);
        post.post(Event::JobDone {
            id,
            index,
            secs: p.majorant_secs,
        });
        Ok(Some(p))
    }
}
