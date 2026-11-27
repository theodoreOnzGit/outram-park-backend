//! **The EBR-II simulator's engine, as a library** (gh:#1741, the layering
//! of gh:#1738): everything the `ebr2_web` demo runs that is not drawing,
//! built like MSRE's `msre::web::engine` (gh:#1578). The example holds only
//! `main`, the app and the drawing.
//!
//! - [`Runner`] holds one `ebr2::session::Session` and answers
//!   [`Request`]s with [`Event`]s, one at a time.
//! - **The plant paces itself**, as MSRE's does (gh:#1573): a clock outside
//!   the page sends [`Request::Tick`] about ten times a second (in the
//!   browser a timer loop inside the worker, [`worker`]; natively a ticker
//!   thread, [`start_native_ticker`]), and the session's `Pacer` turns the
//!   wall time into plant time. The page sends only the speed and pause
//!   ([`Request::Pace`]), commands and resets, so the UI thread only draws.
//! - **A failed step holds the plant** (`Session::advance_guarded`):
//!   [`Event::Held`] is posted once with the held frame, and the plant stays
//!   held until a command or a reset.
//!
//! Built on `bugis::web_demo::link` (links, workers). Off Android, as that
//! is; on wasm32 the engine also carries its Web Worker.
//!
//! Research, education and V&V only. NOT VALIDATED. Never for operation,
//! licensing or safety decisions.

use bugis::web_demo::link::Floats;

/// The MC page's workers: the simplified full core on a pool (gh:#1762).
pub mod keff;
/// The page side of the MC page's pool (gh:#1762).
pub mod keff_pool;

use super::session::{Advance, Command, Pacer, Session, SessionConfig, MAX_ADVANCE_S};

#[derive(Clone)]
pub enum Request {
    /// The wall clock now, [s]: run the plant time the pacer gives for it
    /// and post a frame (nothing while paused or held).
    Tick(f64),
    /// The speed (plant seconds per wall second) and whether paused.
    Pace(f64, bool),
    /// An operator command.
    Cmd(Command),
    /// Start over from a fresh steady state with this configuration.
    Reset(SessionConfig),
    /// A request to the MC page's k_eff worker (gh:#1762).
    Keff(keff::KReq),
}

#[derive(Clone)]
pub enum Event {
    /// A `session::Snapshot` as numbers.
    Frame(Floats),
    /// The model's t = 0 steady state against the cited initial conditions
    /// (`session::Initial` as numbers), posted on every reset.
    Initial(Floats),
    /// A failed step: the plant is held at this time, [s], with the
    /// model's message (`session::Hold`). Posted once per hold.
    Held(f64, String),
    /// A refused command, or a plant that could not be built.
    Error(String),
    /// What the MC page's k_eff worker posts (gh:#1762).
    Keff(keff::KEv),
}

/// The engine's state: the plant and its clock.
pub struct Runner {
    session: Option<Session>,
    /// The MC page's k_eff worker (gh:#1762).
    pub keff: keff::KeffWorker,
    pacer: Pacer,
    /// The browser worker's timer loop has been started ([`worker`]).
    #[cfg_attr(not(target_arch = "wasm32"), allow(dead_code))]
    ticking: bool,
}

/// The speed the plant starts at: 2 plant seconds per wall second, so a
/// whole 900 s test plays in 7.5 minutes; the page offers others.
pub const DEFAULT_SPEED: f64 = 2.0;

impl Default for Runner {
    fn default() -> Self {
        Self { session: Session::new(SessionConfig::default()).ok(), keff: keff::KeffWorker::default(), pacer: Pacer::new(DEFAULT_SPEED, false), ticking: false }
    }
}

impl Runner {
    /// Answer one request: the events it produces, in order. Shared by the
    /// native thread and the worker, so the tests below cover both.
    pub fn answer(&mut self, req: Request) -> Vec<Event> {
        match req {
            Request::Reset(c) => match Session::new(c) {
                Ok(s) => {
                    let out = vec![
                        Event::Initial(Floats::from_vec(s.initial().to_vec())),
                        Event::Frame(Floats::from_vec(s.snapshot().to_vec())),
                    ];
                    self.session = Some(s);
                    out
                }
                Err(e) => vec![Event::Error(e.to_string())],
            },
            Request::Pace(speed, paused) => {
                self.pacer.speed = speed;
                self.pacer.paused = paused;
                Vec::new()
            }
            Request::Tick(wall_s) => {
                let dt = self.pacer.plant_seconds(wall_s);
                if dt > 0.0 {
                    self.run(dt)
                } else {
                    Vec::new()
                }
            }
            Request::Cmd(c) => match self.session.as_mut() {
                Some(s) => match s.apply(c) {
                    Ok(()) => vec![Event::Frame(Floats::from_vec(s.snapshot().to_vec()))],
                    Err(e) => vec![Event::Error(e.to_string())],
                },
                None => vec![Event::Error("no plant: reset it".into())],
            },
            Request::Keff(k) => {
                #[cfg(not(target_arch = "wasm32"))]
                {
                    let mut out = Vec::new();
                    self.keff.answer_native(k, &mut |e| out.push(Event::Keff(e)));
                    out
                }
                #[cfg(target_arch = "wasm32")]
                {
                    self.keff.answer_now(k).into_iter().map(Event::Keff).collect()
                }
            }
        }
    }

    /// Run `dt` seconds of plant time guarded, in `MAX_ADVANCE_S` stretches,
    /// and post one frame at the end; a hold stops the run and posts
    /// [`Event::Held`] once with the held frame. Nothing while held.
    fn run(&mut self, dt: f64) -> Vec<Event> {
        let Some(s) = self.session.as_mut() else { return Vec::new() };
        if s.held().is_some() {
            return Vec::new();
        }
        let mut left = dt;
        let mut out = Vec::new();
        while left > 1e-12 {
            let chunk = left.min(MAX_ADVANCE_S);
            left -= chunk;
            if let Advance::Held(h) = s.advance_guarded(chunk) {
                out.push(Event::Held(h.held_at_s, h.message));
                break;
            }
        }
        out.push(Event::Frame(Floats::from_vec(s.snapshot().to_vec())));
        out
    }
}

#[cfg(not(target_arch = "wasm32"))]
impl bugis::web_demo::link::NativeEngine for Runner {
    type Req = Request;
    type Ev = Event;
    fn handle(&mut self, req: Request, post: &mut impl FnMut(Event)) {
        // A k_eff job posts as it goes (each product when it is done).
        if let Request::Keff(k) = req {
            return self.keff.answer_native(k, &mut |e| post(Event::Keff(e)));
        }
        for e in self.answer(req) {
            post(e);
        }
    }
}

/// A command as wire numbers `[code, value]`; an override's `None` is NaN.
pub fn command_code(c: Command) -> [f64; 2] {
    match c {
        Command::PumpSpeedRpm(x) => [0.0, x.unwrap_or(f64::NAN)],
        Command::IntermediateFlowKgS(x) => [1.0, x.unwrap_or(f64::NAN)],
        Command::ExternalPcm(x) => [2.0, x],
    }
}

/// The inverse of [`command_code`].
pub fn command_from_code(v: &[f64]) -> Option<Command> {
    let x = v.get(1).copied().unwrap_or(f64::NAN);
    let opt = if x.is_nan() { None } else { Some(x) };
    Some(match *v.first()? as i64 {
        0 => Command::PumpSpeedRpm(opt),
        1 => Command::IntermediateFlowKgS(opt),
        2 => Command::ExternalPcm(x),
        _ => return None,
    })
}

/// A configuration as wire numbers `[test code, thermal-centre height m]`.
pub fn config_code(c: SessionConfig) -> [f64; 2] {
    [super::session::test_code(c.test), c.thermal_centre_height_m]
}

/// The inverse of [`config_code`]; missing numbers take the defaults.
pub fn config_from_code(v: &[f64]) -> SessionConfig {
    let d = SessionConfig::default();
    SessionConfig {
        test: v.first().map_or(d.test, |&x| super::session::test_from_code(x)),
        thermal_centre_height_m: v.get(1).copied().unwrap_or(d.thermal_centre_height_m),
    }
}

/// The plant clock's period, [ms]: about ten frames a second.
pub const TICK_MS: i32 = 100;

#[cfg(target_arch = "wasm32")]
pub mod worker {
    use super::*;
    use bugis::web_demo::link::{js, Message, Poster, WorkerEngine};
    use std::sync::{Arc, RwLock};
    use wasm_bindgen::JsValue;

    /// The script the worker starts from (it inits the same module).
    pub const WORKER_JS: &str = "./ebr2_worker.js";

    impl Message for Request {
        fn to_js(&self) -> JsValue {
            if let Request::Keff(k) = self {
                return keff_req_to_js(k);
            }
            let o = js::object();
            let kind = match *self {
                Request::Keff(_) => "",
                Request::Tick(w) => {
                    js::set(&o, "v", js::f64s(&[w]));
                    "tick"
                }
                Request::Pace(speed, paused) => {
                    js::set(&o, "v", js::f64s(&[speed, paused as u8 as f64]));
                    "pace"
                }
                Request::Cmd(c) => {
                    js::set(&o, "v", js::f64s(&command_code(c)));
                    "cmd"
                }
                Request::Reset(c) => {
                    js::set(&o, "v", js::f64s(&config_code(c)));
                    "reset"
                }
            };
            js::set(&o, "kind", kind);
            o.into()
        }
        fn from_js(v: &JsValue) -> Result<Self, String> {
            let kind = js::get_str(v, "kind");
            if kind.starts_with("k_") {
                return keff_req_from_js(&kind, v).map(Request::Keff);
            }
            let n = js::get_f64s(v, "v");
            let g = |i: usize| n.get(i).copied().unwrap_or(0.0);
            Ok(match kind.as_str() {
                "tick" => Request::Tick(g(0)),
                "pace" => Request::Pace(g(0), g(1) != 0.0),
                "cmd" => Request::Cmd(command_from_code(&n).ok_or("bad command")?),
                "reset" => Request::Reset(config_from_code(&n)),
                k => return Err(format!("unknown request '{k}'")),
            })
        }
    }

    impl Message for Event {
        fn to_js(&self) -> JsValue {
            let o = js::object();
            match self {
                Event::Frame(f) => {
                    js::set(&o, "kind", "frame");
                    js::set(&o, "v", f.js());
                }
                Event::Initial(f) => {
                    js::set(&o, "kind", "initial");
                    js::set(&o, "v", f.js());
                }
                Event::Held(t, m) => {
                    js::set(&o, "kind", "held");
                    js::set(&o, "t", *t);
                    js::set(&o, "message", m.as_str());
                }
                Event::Error(m) => {
                    js::set(&o, "kind", "error");
                    js::set(&o, "message", m.as_str());
                }
                Event::Keff(e) => return keff_ev_to_js(e),
            }
            o.into()
        }
        fn from_js(v: &JsValue) -> Result<Self, String> {
            match js::get_str(v, "kind").as_str() {
                k if k.starts_with("k_") => keff_ev_from_js(k, v).map(Event::Keff),
                "frame" => Ok(Event::Frame(Floats::get(v, "v").ok_or("no frame")?)),
                "initial" => Ok(Event::Initial(Floats::get(v, "v").ok_or("no initial")?)),
                "held" => Ok(Event::Held(js::get_f64(v, "t").unwrap_or(f64::NAN), js::get_str(v, "message"))),
                "error" => Ok(Event::Error(js::get_str(v, "message"))),
                k => Err(format!("unknown event '{k}'")),
            }
        }
    }

    impl WorkerEngine for Runner {
        type Req = Request;
        type Ev = Event;
        fn handle(state: &Arc<RwLock<Self>>, req: Request, post: Poster<Event>) {
            // A k_eff pool worker never starts the plant's clock.
            if let Request::Keff(k) = req {
                return keff_handle(state, k, post);
            }
            let (events, start) = match state.write() {
                Ok(mut r) => {
                    let start = !r.ticking;
                    r.ticking = true;
                    (r.answer(req), start)
                }
                Err(_) => (vec![Event::Error("engine state poisoned".into())], false),
            };
            for e in events {
                post.post(e);
            }
            if start {
                start_ticking(state.clone(), post);
            }
        }
        fn error(message: String) -> Event {
            Event::Error(message)
        }
    }

    // ─── The k_eff pool's messages (gh:#1762, after MSRE's, gh:#1571) ──────────────────────────────

    use super::keff::{cfg_from_slice, cfg_to_vec, study_from_slice, study_to_vec, tape_url, KEv, KReq, View};
    use bugis::web_demo::link::Floats;

    fn keff_req_to_js(k: &KReq) -> JsValue {
        let o = js::object();
        let kind = match k {
            KReq::Process { study, jobs } => {
                js::set(&o, "study", js::f64s(&study_to_vec(study)));
                js::set(&o, "jobs", js::f64s(&jobs.iter().map(|&j| j as f64).collect::<Vec<_>>()));
                "k_process"
            }
            KReq::Product { job, data } => {
                js::set(&o, "job", *job as f64);
                js::set(&o, "data", data.js());
                "k_product"
            }
            KReq::Assemble { study } => {
                js::set(&o, "study", js::f64s(&study_to_vec(study)));
                "k_assemble"
            }
            KReq::Source { cfg, epoch } => {
                js::set(&o, "cfg", js::f64s(&cfg_to_vec(cfg)));
                js::set(&o, "epoch", *epoch as f64);
                "k_source"
            }
            KReq::Chunk { cfg, data, epoch } => {
                js::set(&o, "cfg", js::f64s(&cfg_to_vec(cfg)));
                js::set(&o, "data", data.js());
                js::set(&o, "epoch", *epoch as f64);
                "k_chunk"
            }
            KReq::Raster { study, view, px } => {
                js::set(&o, "study", js::f64s(&study_to_vec(study)));
                js::set(&o, "view", view.code());
                js::set(&o, "px", *px as f64);
                "k_raster"
            }
        };
        js::set(&o, "kind", kind);
        o.into()
    }

    fn keff_req_from_js(kind: &str, v: &JsValue) -> Result<KReq, String> {
        let g = |k: &str| js::get_f64(v, k).unwrap_or(0.0);
        Ok(match kind {
            "k_process" => KReq::Process { study: study_from_slice(&js::get_f64s(v, "study")), jobs: js::get_f64s(v, "jobs").iter().map(|&x| x as usize).collect() },
            "k_product" => KReq::Product { job: g("job") as usize, data: Floats::get(v, "data").ok_or("no product")? },
            "k_assemble" => KReq::Assemble { study: study_from_slice(&js::get_f64s(v, "study")) },
            "k_source" => KReq::Source { cfg: cfg_from_slice(&js::get_f64s(v, "cfg")), epoch: g("epoch") as u32 },
            "k_chunk" => KReq::Chunk { cfg: cfg_from_slice(&js::get_f64s(v, "cfg")), data: Floats::get(v, "data").ok_or("no chunk")?, epoch: g("epoch") as u32 },
            "k_raster" => KReq::Raster { study: study_from_slice(&js::get_f64s(v, "study")), view: View::from_code(g("view")), px: g("px") as usize },
            k => return Err(format!("unknown k_eff request '{k}'")),
        })
    }

    fn keff_ev_to_js(e: &KEv) -> JsValue {
        let o = js::object();
        let kind = match e {
            KEv::Started { job } => {
                js::set(&o, "job", *job as f64);
                "k_started"
            }
            KEv::Product { job, secs, data, cached_ms } => {
                js::set(&o, "job", *job as f64);
                js::set(&o, "secs", *secs);
                js::set(&o, "data", data.js());
                js::set(&o, "cached", cached_ms.unwrap_or(f64::NAN));
                "k_product"
            }
            KEv::Assembled(a) => {
                js::set(&o, "nuclides", a.nuclides as f64);
                js::set(&o, "materials", a.materials as f64);
                js::set(&o, "secs", a.secs);
                "k_assembled"
            }
            KEv::Source { sites, seed, mesh, epoch } => {
                js::set(&o, "sites", sites.js());
                // A u64 stream state: exact as a string, not as a JS number.
                js::set(&o, "seed", seed.to_string());
                js::set(&o, "mesh", js::f64s(mesh));
                js::set(&o, "epoch", *epoch as f64);
                "k_source"
            }
            KEv::ChunkDone { data, secs, epoch } => {
                js::set(&o, "data", data.js());
                js::set(&o, "secs", *secs);
                js::set(&o, "epoch", *epoch as f64);
                "k_chunk_done"
            }
            KEv::Failed { message, epoch } => {
                js::set(&o, "message", message.as_str());
                js::set(&o, "epoch", *epoch as f64);
                "k_failed"
            }
            KEv::Raster { view, w, h, ids } => {
                js::set(&o, "view", view.code());
                js::set(&o, "w", *w as f64);
                js::set(&o, "h", *h as f64);
                js::set(&o, "ids", js::u8s(ids));
                "k_raster"
            }
        };
        js::set(&o, "kind", kind);
        o.into()
    }

    fn keff_ev_from_js(kind: &str, v: &JsValue) -> Result<KEv, String> {
        let g = |k: &str| js::get_f64(v, k).unwrap_or(0.0);
        Ok(match kind {
            "k_started" => KEv::Started { job: g("job") as usize },
            "k_product" => KEv::Product {
                job: g("job") as usize,
                secs: g("secs"),
                data: Floats::get(v, "data").ok_or("no product")?,
                cached_ms: js::get_f64(v, "cached").filter(|x| x.is_finite()),
            },
            "k_assembled" => KEv::Assembled(super::keff::Assembly { nuclides: g("nuclides") as usize, materials: g("materials") as usize, secs: g("secs") }),
            "k_source" => {
                let m = js::get_f64s(v, "mesh");
                let mut mesh = [0.0; 9];
                for (d, x) in mesh.iter_mut().zip(m) {
                    *d = x;
                }
                KEv::Source {
                    sites: Floats::get(v, "sites").ok_or("no sites")?,
                    seed: js::get_str(v, "seed").parse().map_err(|e| format!("source seed: {e}"))?,
                    mesh,
                    epoch: g("epoch") as u32,
                }
            }
            "k_chunk_done" => KEv::ChunkDone { data: Floats::get(v, "data").ok_or("no chunk")?, secs: g("secs"), epoch: g("epoch") as u32 },
            "k_failed" => KEv::Failed { message: js::get_str(v, "message"), epoch: g("epoch") as u32 },
            "k_raster" => KEv::Raster { view: View::from_code(g("view")), w: g("w") as usize, h: g("h") as usize, ids: js::get_u8s(v, "ids") },
            k => return Err(format!("unknown k_eff event '{k}'")),
        })
    }

    /// A k_eff request in the browser worker. Processing and assembly
    /// download tapes (`data/<wire name>`), so they run as async tasks that
    /// post as they go; the processed-data cache is the browser's IndexedDB
    /// (`processed_cache::web`), read before each job and written after.
    fn keff_handle(state: &Arc<RwLock<Runner>>, k: KReq, post: Poster<Event>) {
        match k {
            KReq::Process { study, jobs } => {
                let state = state.clone();
                wasm_bindgen_futures::spawn_local(async move {
                    if let Err(message) = keff_process(&state, study, &jobs, post).await {
                        post.post(Event::Keff(KEv::Failed { message, epoch: 0 }));
                    }
                });
            }
            KReq::Assemble { study } => {
                let state = state.clone();
                wasm_bindgen_futures::spawn_local(async move {
                    let e = match keff_assemble(&state, study).await {
                        Ok(a) => KEv::Assembled(a),
                        Err(message) => KEv::Failed { message, epoch: 0 },
                    };
                    post.post(Event::Keff(e));
                });
            }
            other => {
                let events = match state.write() {
                    Ok(mut r) => r.keff.answer_now(other),
                    Err(_) => vec![KEv::Failed { message: "engine state poisoned".into(), epoch: 0 }],
                };
                for e in events {
                    post.post(Event::Keff(e));
                }
            }
        }
    }

    /// A tape's stripped bytes, downloaded beside the page.
    async fn tape_bytes(path: &str) -> Result<Vec<u8>, String> {
        let url = tape_url(path);
        let z = bugis::web_demo::link::fetch_bytes(&url).await?;
        dhoby_ghaut::mc_web::tapes::decompress(&z)
    }

    /// Take the worker's store out of its state for an `await` (no lock is
    /// held across one), and put it back.
    fn take_store(state: &Arc<RwLock<Runner>>) -> dhoby_ghaut::mc_web::processed_cache::DataStore {
        state.write().map(|mut r| std::mem::replace(&mut r.keff.store, dhoby_ghaut::mc_web::processed_cache::DataStore::browser())).unwrap_or_else(|_| dhoby_ghaut::mc_web::processed_cache::DataStore::browser())
    }
    fn put_store(state: &Arc<RwLock<Runner>>, store: dhoby_ghaut::mc_web::processed_cache::DataStore) {
        if let Ok(mut r) = state.write() {
            r.keff.store = store;
        }
    }

    async fn keff_process(state: &Arc<RwLock<Runner>>, study: nee_soon::ebr2_core::Study, jobs: &[usize], post: Poster<Event>) -> Result<(), String> {
        use dhoby_ghaut::mc_web::processed_cache::web as cache;
        let list = super::keff::jobs(&study);
        let mut health = cache::Health::default();
        for &j in jobs {
            post.post(Event::Keff(KEv::Started { job: j }));
            let t = bugis::web_demo::platform::now_s();
            let job = list.get(j).copied().ok_or_else(|| format!("no job {j}"))?;
            let bytes = tape_bytes(job.tape()).await?;
            let mut store = take_store(state);
            cache::before_step(&mut store, &bytes, &mut health).await;
            put_store(state, store);
            let (words, src) = state.write().map_err(|_| "engine state poisoned".to_string())?.keff.process(&study, j, &bytes)?;
            let mut store = take_store(state);
            cache::after_step(&mut store, &mut health).await;
            put_store(state, store);
            post.post(Event::Keff(KEv::Product {
                job: j,
                secs: bugis::web_demo::platform::now_s() - t,
                data: Floats::from_vec(words),
                cached_ms: match src {
                    dhoby_ghaut::web_demo::data_cache::Source::Cached { created_ms, .. } => Some(created_ms),
                    dhoby_ghaut::web_demo::data_cache::Source::Processed => None,
                },
            }));
        }
        Ok(())
    }

    async fn keff_assemble(state: &Arc<RwLock<Runner>>, study: nee_soon::ebr2_core::Study) -> Result<super::keff::Assembly, String> {
        let mut tapes = std::collections::BTreeMap::new();
        for j in super::keff::jobs(&study) {
            let tp = j.tape();
            if !tapes.contains_key(tp) {
                tapes.insert(tp, tape_bytes(tp).await?);
            }
        }
        let mut r = state.write().map_err(|_| "engine state poisoned".to_string())?;
        r.keff.assemble(&study, |p| tapes.get(p).cloned().ok_or_else(|| format!("{p}: not downloaded")), bugis::web_demo::platform::now_s)
    }

    /// The worker's own clock, as MSRE's (`msre::web::engine::worker`):
    /// every [`TICK_MS`] it reads the wall time and answers a
    /// [`Request::Tick`] itself. Started by the page's first request (its
    /// opening reset); a tick is awaited after the last one's work, so a slow
    /// tick delays the next rather than queueing behind it.
    fn start_ticking(state: Arc<RwLock<Runner>>, post: Poster<Event>) {
        wasm_bindgen_futures::spawn_local(async move {
            loop {
                sleep_ms(TICK_MS).await;
                let wall_s = js_sys::Date::now() / 1000.0;
                let events = match state.write() {
                    Ok(mut r) => r.answer(Request::Tick(wall_s)),
                    Err(_) => vec![Event::Error("engine state poisoned".into())],
                };
                for e in events {
                    post.post(e);
                }
            }
        });
    }

    /// Resolve after `ms` milliseconds (the worker scope's `setTimeout`).
    async fn sleep_ms(ms: i32) {
        use wasm_bindgen::JsCast as _;
        let p = js_sys::Promise::new(&mut |resolve, _| {
            let scope: web_sys::DedicatedWorkerGlobalScope = js_sys::global().unchecked_into();
            let _ = scope.set_timeout_with_callback_and_timeout_and_arguments_0(&resolve, ms);
        });
        let _ = wasm_bindgen_futures::JsFuture::from(p).await;
    }
}

/// The native plant clock, as MSRE's (`msre::web::engine::start_native_ticker`):
/// a thread that sends the engine a [`Request::Tick`] with the wall time
/// every [`TICK_MS`], so the plant paces itself and the window only draws.
/// It exits when the link is closed.
#[cfg(not(target_arch = "wasm32"))]
pub fn start_native_ticker(link: &bugis::web_demo::link::Link<Request, Event>) {
    use bugis::web_demo::link::Link;
    let Link::Native(mailbox) = link;
    let ticks = Link::Native(mailbox.clone());
    let mailbox = mailbox.clone();
    std::thread::spawn(move || {
        let t0 = std::time::Instant::now();
        loop {
            std::thread::sleep(std::time::Duration::from_millis(TICK_MS as u64));
            if mailbox.read().map_or(true, |m| m.closed) {
                return;
            }
            ticks.send(Request::Tick(t0.elapsed().as_secs_f64()));
        }
    });
}

#[cfg(all(test, not(target_arch = "wasm32")))]
mod tests {
    use super::*;
    use crate::ebr2::session::{Initial, Snapshot};
    use crate::ebr2::shrt::ShrtTest;

    fn last_frame(e: &[Event]) -> Snapshot {
        e.iter()
            .rev()
            .find_map(|x| match x {
                Event::Frame(f) => Snapshot::from_slice(&f.to_vec()),
                _ => None,
            })
            .expect("a frame")
    }

    /// A reset posts the initial comparison then a frame at t = 0; a
    /// command is echoed as a frame; a refused one is an error event and the
    /// plant goes on; with no plant, commands are errors and runs are silent.
    #[test]
    fn the_engine_runs_the_session() {
        let mut r = Runner::default();
        let e = r.answer(Request::Reset(SessionConfig { test: ShrtTest::Shrt17, ..Default::default() }));
        match &e[0] {
            Event::Initial(f) => assert!(Initial::from_slice(&f.to_vec()).is_some()),
            _ => panic!("the reset posts the initial comparison first"),
        }
        assert_eq!(last_frame(&e).time_s, 0.0);
        assert_eq!(last_frame(&e).kinetics, 0.0);
        let f = last_frame(&r.answer(Request::Cmd(Command::PumpSpeedRpm(Some(500.0)))));
        assert_eq!(f.pump_override, 1.0);
        assert!(matches!(r.answer(Request::Cmd(Command::ExternalPcm(5.0)))[0], Event::Error(_)));
        assert!((last_frame(&r.run(2.5)).time_s - 2.5).abs() < 1e-9);
        assert!(matches!(r.answer(Request::Reset(SessionConfig { thermal_centre_height_m: -1.0, ..Default::default() }))[0], Event::Error(_)));
        let mut empty = Runner { session: None, keff: keff::KeffWorker::default(), pacer: Pacer::new(1.0, false), ticking: false };
        assert!(empty.run(1.0).is_empty());
        assert!(matches!(empty.answer(Request::Cmd(Command::ExternalPcm(0.0)))[0], Event::Error(_)));
    }

    /// The plant runs on wall-clock ticks: the first tick only reads the
    /// clock, then each runs speed x the wall gap; paused, nothing; a 40 s
    /// gap at 60x is capped by the pacer to 60 plant seconds, run in
    /// MAX_ADVANCE_S stretches and posted as one frame.
    #[test]
    fn ticks_drive_the_plant_at_the_chosen_speed() {
        let mut r = Runner::default();
        assert!(r.answer(Request::Tick(1000.0)).is_empty());
        let f = last_frame(&r.answer(Request::Tick(1000.1)));
        assert!((f.time_s - 0.1 * DEFAULT_SPEED).abs() < 1e-9, "{}", f.time_s);
        assert!(r.answer(Request::Pace(DEFAULT_SPEED, true)).is_empty());
        assert!(r.answer(Request::Tick(1000.2)).is_empty(), "paused");
        r.answer(Request::Pace(60.0, false));
        let e = r.answer(Request::Tick(1040.2));
        assert_eq!(e.iter().filter(|x| matches!(x, Event::Frame(_))).count(), 1);
        assert!((last_frame(&e).time_s - (0.1 * DEFAULT_SPEED + 60.0)).abs() < 1e-6, "{}", last_frame(&e).time_s);
    }

    /// Both pumps stopped dead at full power (the session test's case) is
    /// held: Held is posted once with the held frame, the engine is silent
    /// while held, and a command (the pumps back on) releases it.
    #[test]
    fn a_hold_is_posted_once_and_a_command_releases_it() {
        let mut r = Runner::default();
        r.answer(Request::Cmd(Command::PumpSpeedRpm(Some(0.0))));
        let mut held = None;
        for _ in 0..20 {
            let e = r.run(10.0);
            if let Some(t) = e.iter().find_map(|x| match x {
                Event::Held(t, _) => Some(*t),
                _ => None,
            }) {
                held = Some((t, last_frame(&e)));
                break;
            }
        }
        let (t, frame) = held.expect("held within 200 s");
        assert_eq!(frame.time_s, t);
        assert!(r.run(10.0).is_empty(), "silent while held");
        r.answer(Request::Cmd(Command::PumpSpeedRpm(Some(792.0))));
        assert!((last_frame(&r.run(1.0)).time_s - t - 1.0).abs() < 1e-9);
    }

    /// Every command and configuration survives its wire code.
    #[test]
    fn wire_codes_round_trip() {
        for c in [
            Command::PumpSpeedRpm(Some(500.0)),
            Command::PumpSpeedRpm(None),
            Command::IntermediateFlowKgS(Some(120.5)),
            Command::IntermediateFlowKgS(None),
            Command::ExternalPcm(-42.0),
        ] {
            assert_eq!(command_from_code(&command_code(c)), Some(c));
        }
        assert_eq!(command_from_code(&[9.0, 1.0]), None);
        assert_eq!(command_from_code(&[]), None);
        for c in [SessionConfig::default(), SessionConfig { test: ShrtTest::Shrt17, thermal_centre_height_m: 3.5 }] {
            assert_eq!(config_from_code(&config_code(c)), c);
        }
        assert_eq!(config_from_code(&[]), SessionConfig::default());
    }

    /// Natively the ticker thread drives the plant with no request from the
    /// window: frames arrive within seconds; closing the link stops it.
    #[test]
    fn the_native_ticker_drives_the_plant() {
        let link = bugis::web_demo::link::start_native(Runner::default(), || {});
        start_native_ticker(&link);
        let t0 = std::time::Instant::now();
        let mut frames = 0;
        while frames == 0 && t0.elapsed().as_secs_f64() < 10.0 {
            std::thread::sleep(std::time::Duration::from_millis(50));
            frames += link.drain().iter().filter(|e| matches!(e, Event::Frame(_))).count();
        }
        assert!(frames > 0, "no frame from the ticker in 10 s");
        link.terminate();
    }
}
