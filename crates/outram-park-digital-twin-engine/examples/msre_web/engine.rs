//! The engine: the MSRE operator session on a background thread natively,
//! in a Web Worker in the browser (the no-lag rule: the page only draws).
//!
//! It holds one `msre::session::Session` and answers requests one at a time.
//! The page asks for the next stretch of plant time only after the last
//! frame arrived, so at most one request is in flight and each is short
//! (`session::MAX_ADVANCE_S`).

use bugis::web_demo::link::Floats;
use outram_park_digital_twin_engine::msre::session::{
    natural_periods, Coefficients, Command, Session, SessionConfig,
};

#[derive(Clone, Debug, PartialEq)]
pub enum Request {
    /// Run this many seconds of plant time and post a frame.
    Advance(f64),
    /// An operator command.
    Cmd(Command),
    /// Start over from a fresh steady state with this configuration.
    Reset(SessionConfig),
    /// The T-1 natural-period table at these coefficients, [1/°F].
    Periods(f64, f64),
}

#[derive(Clone)]
pub enum Event {
    /// A `session::Snapshot` as numbers.
    Frame(Floats),
    /// `[power, measured, model]` rows; the model is NaN where it has no
    /// resonance.
    Periods(Floats),
    /// A refused command or a failed step.
    Error(String),
}

/// The engine's state: the plant.
pub struct Runner {
    session: Option<Session>,
}

impl Default for Runner {
    fn default() -> Self {
        Self { session: Session::new(SessionConfig::default()).ok() }
    }
}

impl Runner {
    /// Answer one request: the events it produces, in order. Shared by the
    /// native thread and the worker, so the tests below cover both.
    pub fn answer(&mut self, req: Request) -> Vec<Event> {
        match req {
            Request::Reset(c) => match Session::new(c) {
                Ok(s) => {
                    let f = Event::Frame(Floats::from_vec(s.snapshot().to_vec()));
                    self.session = Some(s);
                    vec![f]
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
            Request::Advance(dt) => match self.session.as_mut() {
                Some(s) => match s.advance(dt) {
                    Ok(snap) => vec![Event::Frame(Floats::from_vec(snap.to_vec()))],
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

#[cfg(not(target_arch = "wasm32"))]
impl bugis::web_demo::link::NativeEngine for Runner {
    type Req = Request;
    type Ev = Event;
    fn handle(&mut self, req: Request, post: &mut impl FnMut(Event)) {
        for e in self.answer(req) {
            post(e);
        }
    }
}

/// Coefficient presets as wire codes.
#[cfg_attr(not(target_arch = "wasm32"), allow(dead_code))] // the wire is the browser's; tested natively
pub fn coefficients_code(c: Coefficients) -> f64 {
    match c {
        Coefficients::Ornl1965 => 0.0,
        Coefficients::MonteCarloN3 => 1.0,
    }
}

/// The inverse of [`coefficients_code`].
#[cfg_attr(not(target_arch = "wasm32"), allow(dead_code))] // the wire is the browser's; tested natively
pub fn coefficients_from_code(x: f64) -> Coefficients {
    if x == 1.0 {
        Coefficients::MonteCarloN3
    } else {
        Coefficients::Ornl1965
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
            let o = js::object();
            let (kind, a, b, c) = match *self {
                Request::Advance(dt) => ("advance", dt, 0.0, 0.0),
                Request::Cmd(cmd) => {
                    let (k, x) = command_code(cmd);
                    ("cmd", k, x, 0.0)
                }
                Request::Reset(cfg) => (
                    "reset",
                    cfg.power_mw,
                    coefficients_code(cfg.coefficients),
                    cfg.line_model as u8 as f64,
                ),
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
            Ok(match js::get_str(v, "kind").as_str() {
                "advance" => Request::Advance(g("a")),
                "cmd" => Request::Cmd(command_from_code(g("a"), g("b")).ok_or("bad command")?),
                "reset" => Request::Reset(SessionConfig {
                    power_mw: g("a"),
                    coefficients: coefficients_from_code(g("b")),
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
                "error" => Ok(Event::Error(js::get_str(v, "message"))),
                k => Err(format!("unknown event '{k}'")),
            }
        }
    }

    impl WorkerEngine for Runner {
        type Req = Request;
        type Ev = Event;
        fn handle(state: &Arc<RwLock<Self>>, req: Request, post: Poster<Event>) {
            let events = match state.write() {
                Ok(mut r) => r.answer(req),
                Err(_) => vec![Event::Error("engine state poisoned".into())],
            };
            for e in events {
                post.post(e);
            }
        }
        fn error(message: String) -> Event {
            Event::Error(message)
        }
    }
}

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

    /// The engine runs the session: an advance moves the clock, a command
    /// is applied and echoed as a frame, a reset starts over at its power.
    #[test]
    fn the_engine_runs_the_session() {
        let mut r = Runner::default();
        assert!((frame(&r.answer(Request::Advance(2.5))[0]).time_s - 2.5).abs() < 1e-9);
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
        assert!(matches!(r.answer(Request::Advance(1.0))[0], Event::Frame(_)));
        let mut empty = Runner { session: None };
        assert!(matches!(empty.answer(Request::Advance(1.0))[0], Event::Error(_)));
        assert!(matches!(empty.answer(Request::Cmd(Command::Servo(true)))[0], Event::Error(_)));
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
        for c in [Coefficients::Ornl1965, Coefficients::MonteCarloN3] {
            assert_eq!(coefficients_from_code(coefficients_code(c)), c);
        }
    }
}
