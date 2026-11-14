//! The engine: the MSRE operator session on a background thread natively,
//! in a Web Worker in the browser (the no-lag rule: the page only draws).
//!
//! It holds one `msre::session::Session` and, beside it, Moltres's
//! precursor drift around the loop (`msre::moltres_drift::LoopDrift`, gh:#1575),
//! stepped with the plant at its fuel flow and fission power. It answers
//! requests one at a time.
//!
//! **The plant paces itself (gh:#1573).** ~~The page asks for the next
//! stretch of plant time only after the last frame arrived.~~ A clock outside
//! the page sends [`Request::Tick`] about ten times a second (in the browser a
//! timer loop inside the worker, [`web`]; natively a ticker thread, `main`),
//! and the engine's `session::Pacer` turns the wall time into plant time at
//! the chosen speed. So the plant runs from page load, whichever tab is open
//! and however the page's frames are throttled; the page sends only the
//! speed and pause ([`Request::Pace`]), commands, resets and thaws.
//!
//! **Salt freeze.** Every stretch runs through
//! `session::Session::advance_guarded`: a stretch that would take a salt below
//! its liquidus is refused, the plant is held at the last liquid state and
//! [`Event::Frozen`] is posted once; [`Request::Thaw`] (a cheat, said so on the
//! page) restarts it hot.

use bugis::web_demo::link::Floats;
use outram_park_digital_twin_engine::msre::moltres_drift::{LoopDrift, LOOP_DRIFT_CELLS};
use outram_park_digital_twin_engine::msre::session::{
    natural_periods, Advance, Coefficients, Command, Freeze, FreezeLocation, Pacer, Session, SessionConfig, Snapshot,
    MAX_ADVANCE_S,
};

/// Arcs the drift's emission profile is averaged into for the page.
pub const DRIFT_BINS: usize = 126;

#[derive(Clone)]
pub enum Request {
    /// The wall clock now, [s]: run the plant time the pacer gives for it
    /// and post a frame (nothing while paused or held frozen).
    Tick(f64),
    /// The speed (plant seconds per wall second) and whether paused.
    Pace(f64, bool),
    /// Thaw a frozen plant: back to the configured hot steady state (a
    /// cheat; `session::Session::thaw`).
    Thaw,
    /// An operator command.
    Cmd(Command),
    /// Start over from a fresh steady state with this configuration.
    Reset(SessionConfig),
    /// The T-1 natural-period table at these coefficients, [1/°F].
    Periods(f64, f64),
    /// The Core k_eff tab's pool (gh:#1571): a k_eff worker's request. A
    /// worker that only ever gets these never starts the plant's clock.
    Keff(crate::keff::KReq),
}

#[derive(Clone)]
pub enum Event {
    /// A `session::Snapshot` as numbers.
    Frame(Floats),
    /// `[power, measured, model]` rows; the model is NaN where it has no
    /// resonance.
    Periods(Floats),
    /// The loop drift after a step: `[time s, loss δk/k (NaN at zero
    /// power), fuel flow fraction, DRIFT_BINS emission values]`; only the
    /// first three if the drift failed and stopped. Sent BESIDE a frame, so
    /// it is not a reply: the page counts frames, period tables and errors.
    Drift(Floats),
    /// A salt would have frozen: `[location code, refused temperature °F,
    /// held-at plant time s]` (`session::Freeze`). Sent once per freeze,
    /// beside the held frame.
    Frozen(Floats),
    /// A k_eff worker's event (gh:#1571).
    Keff(crate::keff::KEv),
    /// A refused command or a failed step.
    Error(String),
}

/// A freeze as the wire carries it.
pub fn freeze_to_vec(f: &Freeze) -> Vec<f64> {
    vec![f.location.code(), f.temperature_f, f.held_at_s]
}

/// The inverse of [`freeze_to_vec`].
pub fn freeze_from_slice(v: &[f64]) -> Option<Freeze> {
    Some(Freeze { location: FreezeLocation::from_code(*v.first()?)?, temperature_f: *v.get(1)?, held_at_s: *v.get(2)? })
}

/// The engine's state: the plant and the loop's precursor drift.
pub struct Runner {
    session: Option<Session>,
    drift: Option<LoopDrift>,
    /// The k_eff worker's state, used only by the tab's pool workers.
    keff: crate::keff::KeffWorker,
    pacer: Pacer,
    /// The browser worker's timer loop has been started (`web`).
    #[cfg_attr(not(target_arch = "wasm32"), allow(dead_code))]
    ticking: bool,
}

/// The speed the plant starts at: 5 plant seconds per wall second.
pub const DEFAULT_SPEED: f64 = 5.0;

impl Default for Runner {
    fn default() -> Self {
        let session = Session::new(SessionConfig::default()).ok();
        let drift = session.as_ref().and_then(|s| drift_for(&s.snapshot()));
        Self { session, drift, keff: crate::keff::KeffWorker::default(), pacer: Pacer::new(DEFAULT_SPEED, false), ticking: false }
    }
}

/// The steady drift at a snapshot's flow and power, or `None` if Moltres
/// refuses them (the page then shows no drift).
fn drift_for(s: &Snapshot) -> Option<LoopDrift> {
    LoopDrift::new(LOOP_DRIFT_CELLS, s.fuel_flow.max(0.0), s.power_mw.max(0.0)).ok()
}

/// The drift as a page event.
fn drift_event(d: &LoopDrift, flow: f64) -> Event {
    let mut v = vec![d.time(), d.loss().unwrap_or(f64::NAN), flow];
    v.extend(d.emission_binned(DRIFT_BINS));
    Event::Drift(Floats::from_vec(v))
}

impl Runner {
    /// Answer one request: the events it produces, in order. Shared by the
    /// native thread and the worker, so the tests below cover both.
    pub fn answer(&mut self, req: Request) -> Vec<Event> {
        match req {
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
            Request::Reset(c) => match Session::new(c) {
                Ok(s) => {
                    let snap = s.snapshot();
                    let mut out = vec![Event::Frame(Floats::from_vec(snap.to_vec()))];
                    self.drift = drift_for(&snap);
                    if let Some(d) = &self.drift {
                        out.push(drift_event(d, snap.fuel_flow));
                    }
                    self.session = Some(s);
                    out
                }
                Err(e) => vec![Event::Error(e.to_string())],
            },
            Request::Periods(f, g) => {
                let mut v = Vec::new();
                for (p, m, model) in natural_periods(f, g) {
                    v.extend([p, m, model.unwrap_or(f64::NAN)]);
                }
                vec![Event::Periods(Floats::from_vec(v))]
            }
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
            Request::Thaw => match self.session.as_mut() {
                Some(s) => match s.thaw() {
                    Ok(_) => {
                        // Thawed or not, the page gets the plant as it is.
                        // The drift restarts steady at the thawed plant.
                        let snap = s.snapshot();
                        let mut out = vec![Event::Frame(Floats::from_vec(snap.to_vec()))];
                        self.drift = drift_for(&snap);
                        if let Some(d) = &self.drift {
                            out.push(drift_event(d, snap.fuel_flow));
                        }
                        out
                    }
                    Err(e) => vec![Event::Error(e.to_string())],
                },
                None => vec![Event::Error("no plant: reset it".into())],
            },
            Request::Cmd(c) => match self.session.as_mut() {
                Some(s) => match s.apply(c) {
                    Ok(()) => vec![Event::Frame(Floats::from_vec(s.snapshot().to_vec()))],
                    Err(e) => vec![Event::Error(e.to_string())],
                },
                None => vec![Event::Error("no plant: reset it".into())],
            },
        }
    }
}

impl Runner {
    /// Run `dt` seconds of plant time guarded, in `MAX_ADVANCE_S` stretches,
    /// stepping the drift beside each, and post one frame (and the drift)
    /// at the end; a freeze stops the run, posts [`Event::Frozen`] once and
    /// the held frame. Nothing while held frozen.
    fn run(&mut self, dt: f64) -> Vec<Event> {
        let Some(s) = self.session.as_mut() else { return Vec::new() };
        if s.frozen().is_some() {
            return Vec::new();
        }
        let mut left = dt;
        let mut out = Vec::new();
        while left > 1e-12 {
            let chunk = left.min(MAX_ADVANCE_S);
            left -= chunk;
            match s.advance_guarded(chunk) {
                Ok(Advance::Ran(snap)) => {
                    if let Some(d) = self.drift.as_mut() {
                        if d.advance(chunk, snap.fuel_flow.max(0.0), snap.power_mw.max(0.0)).is_err() {
                            // Reported as a drift with no profile, not an
                            // Error (the drift stops; the plant goes on).
                            out.push(Event::Drift(Floats::from_vec(vec![d.time(), f64::NAN, snap.fuel_flow])));
                            self.drift = None;
                        }
                    }
                }
                Ok(Advance::Frozen(f)) => {
                    out.push(Event::Frozen(Floats::from_vec(freeze_to_vec(&f))));
                    break;
                }
                Err(e) => {
                    out.push(Event::Error(e.to_string()));
                    return out;
                }
            }
        }
        let snap = s.snapshot();
        out.push(Event::Frame(Floats::from_vec(snap.to_vec())));
        if let Some(d) = &self.drift {
            out.push(drift_event(d, snap.fuel_flow));
        }
        out
    }
}

#[cfg(not(target_arch = "wasm32"))]
impl bugis::web_demo::link::NativeEngine for Runner {
    type Req = Request;
    type Ev = Event;
    fn handle(&mut self, req: Request, post: &mut impl FnMut(Event)) {
        match req {
            // Posted as each job finishes, for the page's progress.
            Request::Keff(k) => self.keff.answer_native(k, &mut |e| post(Event::Keff(e))),
            r => {
                for e in self.answer(r) {
                    post(e);
                }
            }
        }
    }
}

/// Coefficient presets as wire codes.
#[cfg_attr(not(target_arch = "wasm32"), allow(dead_code))] // the wire is the browser's; tested natively
pub fn coefficients_code(c: Coefficients) -> [f64; 5] {
    match c {
        Coefficients::Ornl1965 => [0.0; 5],
        Coefficients::MonteCarloN3 => [1.0, 0.0, 0.0, 0.0, 0.0],
        Coefficients::Measured { fuel_per_f, fuel_sigma, graphite_per_f, graphite_sigma } => [2.0, fuel_per_f, fuel_sigma, graphite_per_f, graphite_sigma],
    }
}

/// The inverse of [`coefficients_code`].
#[cfg_attr(not(target_arch = "wasm32"), allow(dead_code))] // the wire is the browser's; tested natively
pub fn coefficients_from_code(v: &[f64]) -> Coefficients {
    let g = |i: usize| v.get(i).copied().unwrap_or(0.0);
    match g(0) as i64 {
        1 => Coefficients::MonteCarloN3,
        2 => Coefficients::Measured { fuel_per_f: g(1), fuel_sigma: g(2), graphite_per_f: g(3), graphite_sigma: g(4) },
        _ => Coefficients::Ornl1965,
    }
}

/// A command as `(code, value)`, for the wire.
#[cfg_attr(not(target_arch = "wasm32"), allow(dead_code))] // the wire is the browser's; tested natively
pub fn command_code(c: Command) -> (f64, f64) {
    match c {
        Command::RodsPcm(x) => (0.0, x),
        Command::Servo(on) => (1.0, on as u8 as f64),
        Command::AirFraction(x) => (2.0, x),
        Command::TripFuelPump => (3.0, 0.0),
        Command::TripCoolantPump => (4.0, 0.0),
        Command::StartFuelPump => (5.0, 0.0),
        Command::StartCoolantPump => (6.0, 0.0),
    }
}

/// The inverse of [`command_code`].
#[cfg_attr(not(target_arch = "wasm32"), allow(dead_code))] // the wire is the browser's; tested natively
pub fn command_from_code(code: f64, x: f64) -> Option<Command> {
    Some(match code as i64 {
        0 => Command::RodsPcm(x),
        1 => Command::Servo(x != 0.0),
        2 => Command::AirFraction(x),
        3 => Command::TripFuelPump,
        4 => Command::TripCoolantPump,
        5 => Command::StartFuelPump,
        6 => Command::StartCoolantPump,
        _ => return None,
    })
}

#[cfg(target_arch = "wasm32")]
pub mod web {
    use super::*;
    use bugis::web_demo::link::{js, Message, Poster, WorkerEngine};
    use std::sync::{Arc, RwLock};
    use wasm_bindgen::JsValue;

    /// The script the worker starts from (it inits the same module).
    pub const WORKER_JS: &str = "./msre_worker.js";

    impl Message for Request {
        fn to_js(&self) -> JsValue {
            if let Request::Keff(k) = self {
                return keff_req_to_js(k);
            }
            let o = js::object();
            let (kind, a, b, c) = match *self {
                Request::Keff(_) => ("", 0.0, 0.0, 0.0),
                Request::Tick(w) => ("tick", w, 0.0, 0.0),
                Request::Pace(speed, paused) => ("pace", speed, paused as u8 as f64, 0.0),
                Request::Thaw => ("thaw", 0.0, 0.0, 0.0),
                Request::Cmd(cmd) => {
                    let (k, x) = command_code(cmd);
                    ("cmd", k, x, 0.0)
                }
                Request::Reset(cfg) => {
                    js::set(&o, "coef", js::f64s(&coefficients_code(cfg.coefficients)));
                    ("reset", cfg.power_mw, 0.0, cfg.line_model as u8 as f64)
                }
                Request::Periods(f, g) => ("periods", f, g, 0.0),
            };
            js::set(&o, "kind", kind);
            js::set(&o, "a", a);
            js::set(&o, "b", b);
            js::set(&o, "c", c);
            o.into()
        }
        fn from_js(v: &JsValue) -> Result<Self, String> {
            let g = |k: &str| js::get_f64(v, k).unwrap_or(0.0);
            let kind = js::get_str(v, "kind");
            if kind.starts_with("k_") {
                return keff_req_from_js(&kind, v).map(Request::Keff);
            }
            Ok(match kind.as_str() {
                "tick" => Request::Tick(g("a")),
                "pace" => Request::Pace(g("a"), g("b") != 0.0),
                "thaw" => Request::Thaw,
                "cmd" => Request::Cmd(command_from_code(g("a"), g("b")).ok_or("bad command")?),
                "reset" => Request::Reset(SessionConfig {
                    power_mw: g("a"),
                    coefficients: coefficients_from_code(&js::get_f64s(v, "coef")),
                    line_model: g("c") != 0.0,
                }),
                "periods" => Request::Periods(g("a"), g("b")),
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
                Event::Periods(f) => {
                    js::set(&o, "kind", "periods");
                    js::set(&o, "v", f.js());
                }
                Event::Drift(f) => {
                    js::set(&o, "kind", "drift");
                    js::set(&o, "v", f.js());
                }
                Event::Frozen(f) => {
                    js::set(&o, "kind", "frozen");
                    js::set(&o, "v", f.js());
                }
                Event::Keff(e) => return keff_ev_to_js(e),
                Event::Error(m) => {
                    js::set(&o, "kind", "error");
                    js::set(&o, "message", m.as_str());
                }
            }
            o.into()
        }
        fn from_js(v: &JsValue) -> Result<Self, String> {
            match js::get_str(v, "kind").as_str() {
                "frame" => Ok(Event::Frame(Floats::get(v, "v").ok_or("no frame")?)),
                "periods" => Ok(Event::Periods(Floats::get(v, "v").ok_or("no periods")?)),
                "drift" => Ok(Event::Drift(Floats::get(v, "v").ok_or("no drift")?)),
                "frozen" => Ok(Event::Frozen(Floats::get(v, "v").ok_or("no freeze")?)),
                k if k.starts_with("k_") => keff_ev_from_js(k, v).map(Event::Keff),
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

    // ─── The k_eff pool's messages (gh:#1571) ──────────────────────────────

    use crate::keff::{cfg_from_slice, cfg_to_vec, study_from_slice, study_to_vec, tape_url, Job, KEv, KReq, View};

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
            "k_assembled" => KEv::Assembled(crate::keff::Assembly { nuclides: g("nuclides") as usize, materials: g("materials") as usize, secs: g("secs") }),
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

    async fn keff_process(state: &Arc<RwLock<Runner>>, study: outram_park_digital_twin_engine::msre::first_criticality::Study, jobs: &[usize], post: Poster<Event>) -> Result<(), String> {
        use dhoby_ghaut::mc_web::processed_cache::{tape_prefix, web as cache};
        let list = crate::keff::jobs(&study);
        let mut health = cache::Health::default();
        for &j in jobs {
            post.post(Event::Keff(KEv::Started { job: j }));
            let t = bugis::web_demo::platform::now_s();
            let job = list.get(j).copied().ok_or_else(|| format!("no job {j}"))?;
            let bytes = match job.tape() {
                Some(tp) => tape_bytes(tp).await?,
                None => Vec::new(),
            };
            let mut store = take_store(state);
            match job {
                Job::Graphite(_) => cache::before_prefix(&mut store, &tape_prefix(&dhoby_ghaut::mc_web::tapes::generated_source(crate::keff::LEAPR_GRAPHITE_30P)), &mut health).await,
                _ => cache::before_step(&mut store, &bytes, &mut health).await,
            }
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

    async fn keff_assemble(state: &Arc<RwLock<Runner>>, study: outram_park_digital_twin_engine::msre::first_criticality::Study) -> Result<crate::keff::Assembly, String> {
        let mut tapes = std::collections::BTreeMap::new();
        for j in crate::keff::jobs(&study) {
            if let Some(tp) = j.tape() {
                if !tapes.contains_key(tp) {
                    tapes.insert(tp, tape_bytes(tp).await?);
                }
            }
        }
        let mut r = state.write().map_err(|_| "engine state poisoned".to_string())?;
        r.keff.assemble(&study, |p| tapes.get(p).cloned().ok_or_else(|| format!("{p}: not downloaded")), bugis::web_demo::platform::now_s)
    }

    /// The worker's own clock (gh:#1573): every [`TICK_MS`] it reads the wall
    /// time and answers a [`Request::Tick`] itself, posting what it gives.
    /// Started by the first request the page sends (the page's opening
    /// reset), so the plant runs from page load; the page never steps it.
    /// A tick is awaited after the last one's work, so a slow tick delays
    /// the next rather than queueing behind it, and the pacer's wall gap
    /// absorbs the delay.
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

/// The plant clock's period, [ms]: about ten frames a second.
pub const TICK_MS: i32 = 100;

#[cfg(all(test, not(target_arch = "wasm32")))]
mod tests {
    use super::*;
    use outram_park_digital_twin_engine::msre::session::Snapshot;

    fn frame(e: &Event) -> Snapshot {
        match e {
            Event::Frame(f) => Snapshot::from_slice(&f.to_vec()).unwrap(),
            _ => panic!("not a frame"),
        }
    }

    /// The last frame among some events.
    fn last_frame(e: &[Event]) -> Snapshot {
        e.iter().rev().find(|x| matches!(x, Event::Frame(_))).map(frame).expect("a frame")
    }

    /// The engine runs the session: a run moves the clock, a command is
    /// applied and echoed as a frame, a reset starts over at its power.
    #[test]
    fn the_engine_runs_the_session() {
        let mut r = Runner::default();
        assert!((last_frame(&r.run(2.5)).time_s - 2.5).abs() < 1e-9);
        let f = frame(&r.answer(Request::Cmd(Command::RodsPcm(5.0)))[0]);
        assert!((f.rod_pcm - 5.0).abs() < 1e-9);
        let cfg = SessionConfig { power_mw: 1.0, ..Default::default() };
        let f = frame(&r.answer(Request::Reset(cfg))[0]);
        assert_eq!(f.time_s, 0.0);
        assert!((f.power_mw - 1.0).abs() < 1e-9);
    }

    /// A refused command comes back as an error, and the plant goes on.
    #[test]
    fn a_bad_command_is_an_error_event() {
        let mut r = Runner::default();
        assert!(matches!(r.answer(Request::Cmd(Command::AirFraction(0.0)))[0], Event::Error(_)));
        assert!(matches!(r.run(1.0)[0], Event::Frame(_)));
        let mut empty = Runner { session: None, drift: None, keff: crate::keff::KeffWorker::default(), pacer: Pacer::new(1.0, false), ticking: false };
        assert!(empty.run(1.0).is_empty());
        assert!(matches!(empty.answer(Request::Cmd(Command::Servo(true)))[0], Event::Error(_)));
        assert!(matches!(empty.answer(Request::Thaw)[0], Event::Error(_)));
    }

    /// Moltres's drift rides with the plant: a reset sends the steady loop,
    /// each advance a stepped one at the plant's clock, and a fuel-pump trip
    /// (radiator doors then closed, as ORNL did) shrinks the circulation
    /// loss as the precursors stop leaving the core.
    #[test]
    fn the_drift_follows_the_plant() {
        let drift = |e: &[Event]| -> Vec<f64> {
            e.iter()
                .find_map(|x| match x {
                    Event::Drift(f) => Some(f.to_vec()),
                    _ => None,
                })
                .expect("a drift event")
        };
        let mut r = Runner::default();
        let d0 = drift(&r.answer(Request::Reset(SessionConfig::default())));
        assert_eq!(d0.len(), 3 + DRIFT_BINS);
        assert_eq!(d0[0], 0.0);
        assert!((d0[1] - 0.00229218).abs() < 2e-7, "steady loss {}", d0[1]);
        let d1 = drift(&r.run(2.0));
        assert!((d1[0] - 2.0).abs() < 1e-12);
        r.answer(Request::Cmd(Command::TripFuelPump));
        // ORNL closed the radiator doors after a trip so the coolant would
        // not freeze; with them open the guard (gh:#1573) holds the plant.
        r.answer(Request::Cmd(Command::AirFraction(0.01)));
        let before = d1[1];
        let mut last = d1;
        for _ in 0..30 {
            last = drift(&r.run(10.0));
        }
        assert_eq!(last.len(), 3 + DRIFT_BINS);
        assert!(last[2] < 0.5, "fuel flow {}", last[2]);
        assert!(last[1] < 0.5 * before, "loss {before} -> {}", last[1]);
    }

    /// The period table arrives as seven `[power, measured, model]` rows.
    #[test]
    fn the_period_table_arrives_flat() {
        let mut r = Runner::default();
        match &r.answer(Request::Periods(-4.84e-5, -3.7e-5))[0] {
            Event::Periods(f) => {
                let v = f.to_vec();
                assert_eq!(v.len(), 21);
                assert!(v.chunks(3).all(|row| row[2].is_finite() && row[2] > 0.0), "{v:?}");
            }
            _ => panic!("not a period table"),
        }
    }

    /// Every command and coefficient preset survives its wire code.
    #[test]
    fn wire_codes_round_trip() {
        for c in [
            Command::RodsPcm(-12.5),
            Command::Servo(true),
            Command::Servo(false),
            Command::AirFraction(0.3),
            Command::TripFuelPump,
            Command::TripCoolantPump,
            Command::StartFuelPump,
            Command::StartCoolantPump,
        ] {
            let (k, x) = command_code(c);
            assert_eq!(command_from_code(k, x), Some(c));
        }
        assert_eq!(command_from_code(99.0, 0.0), None);
        for c in [
            Coefficients::Ornl1965,
            Coefficients::MonteCarloN3,
            Coefficients::Measured { fuel_per_f: -3.8e-5, fuel_sigma: 0.4e-5, graphite_per_f: -3.3e-5, graphite_sigma: 0.3e-5 },
        ] {
            assert_eq!(coefficients_from_code(&coefficients_code(c)), c);
        }
    }

    /// gh:#1573: the plant runs on wall-clock ticks with no page request
    /// for its steps: the first tick only reads the clock, then each runs
    /// speed × the wall gap (the default 5×); paused, ticks run nothing and
    /// post nothing; a 40 s gap at 300× is capped by the pacer to 300 plant
    /// seconds, run in MAX_ADVANCE_S stretches and posted as one frame.
    #[test]
    fn ticks_drive_the_plant_at_the_chosen_speed() {
        let mut r = Runner::default();
        assert!(r.answer(Request::Tick(1000.0)).is_empty(), "the first tick reads the clock");
        let f = last_frame(&r.answer(Request::Tick(1000.1)));
        assert!((f.time_s - 0.5).abs() < 1e-9, "{}", f.time_s);
        assert!(r.answer(Request::Pace(DEFAULT_SPEED, true)).is_empty());
        assert!(r.answer(Request::Tick(1000.2)).is_empty(), "paused");
        r.answer(Request::Pace(300.0, false));
        let e = r.answer(Request::Tick(1040.2));
        assert_eq!(e.iter().filter(|x| matches!(x, Event::Frame(_))).count(), 1);
        assert!((last_frame(&e).time_s - 300.5).abs() < 1e-6, "{}", last_frame(&e).time_s);
    }

    /// gh:#1573: a fuel-pump trip with the radiator doors open freezes the
    /// coolant. The engine posts Frozen once (location, refused temperature,
    /// held time) with the held frame, then stays silent while held; a thaw
    /// brings the plant back hot at the same clock, and it runs again.
    #[test]
    fn a_freeze_is_posted_once_and_a_thaw_restarts_the_plant() {
        let mut r = Runner::default();
        r.answer(Request::Cmd(Command::TripFuelPump));
        let mut froze = None;
        for _ in 0..200 {
            let e = r.run(10.0);
            if let Some(v) = e.iter().find_map(|x| match x {
                Event::Frozen(f) => Some(f.to_vec()),
                _ => None,
            }) {
                froze = Some((freeze_from_slice(&v).expect("a freeze"), last_frame(&e)));
                break;
            }
        }
        let (f, held) = froze.expect("the coolant froze within 2000 s");
        assert!(!f.location.is_fuel());
        assert_eq!(f.held_at_s, held.time_s);
        assert_eq!(held.below_liquidus(), (false, false), "held liquid: {held:?}");
        assert!(r.run(10.0).is_empty(), "silent while held");
        let thawed = last_frame(&r.answer(Request::Thaw));
        assert_eq!(thawed.time_s, held.time_s);
        assert!((thawed.power_mw - 8.0).abs() < 1e-9 && thawed.fuel_flow == 1.0, "{thawed:?}");
        assert!((last_frame(&r.run(2.0)).time_s - held.time_s - 2.0).abs() < 1e-9);
        assert_eq!(freeze_from_slice(&freeze_to_vec(&f)), Some(f));
    }
}
