//! The live half of the twin, off the UI thread: a background thread
//! natively, the Web Worker in the browser (`web/double_pipe_twin/`).
//!
//! **The physics is not here.** Every UA is the twin engine's
//! `multifidelity::double_pipe::one_d::ua` (gh:#1178/#1180): TUAS's
//! `SimpleShellAndTubeHeatExchanger` with one tube, its two nodal
//! conductances read without the time march (bit-identical to the marched
//! UA, pinned there). That is what makes the 1-D model real-time: one call
//! is well under a millisecond. This file picks the x and θ and flattens
//! the answers for the page.
//!
//! One request = one slice: the 1-D UA along Re_hot at the slice's
//! (Re_cold, k_wall, L), at nominal θ and at the posterior-mean θ, and the
//! spread of the 1-D UA over the committed θ posterior samples (θ's
//! uncertainty alone, without δ); plus the same at the operating point.

use crate::artefacts::{self, ThetaPosterior};
use outram_park_digital_twin_engine::multifidelity::double_pipe::{one_d, DoublePipeInput, Theta};
use uom::si::f64::{Length, Ratio, ThermalConductivity};
use uom::si::length::meter;
use uom::si::ratio::ratio;
use uom::si::thermal_conductance::watt_per_kelvin;
use uom::si::thermal_conductivity::watt_per_meter_kelvin;

/// Points along Re_hot in a sweep (log-spaced over 10 … 90).
pub const N_SWEEP: usize = 33;
/// Every this many committed θ samples enters a sweep point's band (all of
/// them enter the operating point's).
pub const BAND_STRIDE: usize = 4;

/// Where the twin is evaluated: a typed design point (uom at the API); the
/// slice is the same x with Re_hot swept.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Operating {
    pub re_hot: Ratio,
    pub re_cold: Ratio,
    pub k_wall: ThermalConductivity,
    pub length: Length,
}

impl Operating {
    /// From SI numbers (Re, Re, W/(m K), m).
    #[must_use]
    pub fn si(re_hot: f64, re_cold: f64, k_wall: f64, length: f64) -> Self {
        Self {
            re_hot: Ratio::new::<ratio>(re_hot),
            re_cold: Ratio::new::<ratio>(re_cold),
            k_wall: ThermalConductivity::new::<watt_per_meter_kelvin>(k_wall),
            length: Length::new::<meter>(length),
        }
    }
    /// `[Re_hot, Re_cold, k_wall, L]` in SI.
    #[must_use]
    pub fn to_si(&self) -> [f64; 4] {
        [
            self.re_hot.get::<ratio>(),
            self.re_cold.get::<ratio>(),
            self.k_wall.get::<watt_per_meter_kelvin>(),
            self.length.get::<meter>(),
        ]
    }
    /// The twin engine's input at Re_hot `re_hot`.
    #[must_use]
    pub fn input_at(&self, re_hot: f64) -> DoublePipeInput {
        let [_, c, k, l] = self.to_si();
        DoublePipeInput::si(re_hot, c, k, l)
    }
}

/// The default slice and point: gh:#1178's held-out H7 (Re_hot 30,
/// Re_cold 15, k_wall 10 W/(m K), L 0.2 m).
#[must_use]
#[cfg_attr(target_arch = "wasm32", allow(dead_code))]
pub fn default_operating() -> Operating {
    Operating::si(30.0, 15.0, 10.0, 0.2)
}

/// The sweep's Re_hot values.
#[must_use]
pub fn sweep_re() -> Vec<f64> {
    let (a, b) = (10.0_f64.ln(), 90.0_f64.ln());
    (0..N_SWEEP)
        .map(|i| (a + (b - a) * i as f64 / (N_SWEEP - 1) as f64).exp())
        .collect()
}

/// What the UI asks for.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Request {
    Evaluate { id: u32, at: Operating },
}

/// The 1-D answers at one x \[W/K\].
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Point {
    pub re_hot: f64,
    pub nominal: f64,
    pub post_mean: f64,
    /// 2.5 % and 97.5 % of the 1-D UA over the θ samples (no δ).
    pub theta_lo: f64,
    pub theta_hi: f64,
}

/// One slice's live answers.
#[derive(Clone, Debug, PartialEq)]
pub struct Slice {
    pub id: u32,
    pub at: [f64; 4],
    pub point: Point,
    pub sweep: Vec<Point>,
    /// Wall-clock milliseconds the worker spent.
    pub ms: f64,
    /// 1-D evaluations it made.
    pub evaluations: usize,
}

#[derive(Clone, Debug, PartialEq)]
pub enum Event {
    Slice(Slice),
    Error { id: u32, message: String },
}

/// The engine: the committed θ posterior, parsed once.
pub struct Engine {
    theta: Result<ThetaPosterior, String>,
}

impl Default for Engine {
    fn default() -> Self {
        Self {
            theta: artefacts::parse_theta(artefacts::THETA_CSV),
        }
    }
}

/// The q-quantile of `v` (sorted in place), nearest rank as gh:#1180's
/// reporting.
fn quantile(v: &mut [f64], q: f64) -> f64 {
    v.sort_by(f64::total_cmp);
    v[((q * (v.len() - 1) as f64).round()) as usize]
}

impl Engine {
    /// The 1-D at `x` at nominal θ, the mean θ and the θ samples (every
    /// `stride`-th), and the number of calls it took.
    fn point(&self, x: &DoublePipeInput, stride: usize) -> Result<(Point, usize), String> {
        let post = self.theta.as_ref().map_err(Clone::clone)?;
        let ua = |th: &Theta| {
            one_d::ua(x, th)
                .map(|u| u.get::<watt_per_kelvin>())
                .map_err(|e| e.to_string())
        };
        let mut band: Vec<f64> = (0..post.samples.len())
            .step_by(stride)
            .map(|i| ua(&post.sample_theta(i)))
            .collect::<Result<_, _>>()?;
        let n = band.len() + 2;
        Ok((
            Point {
                re_hot: x.re_hot.get::<ratio>(),
                nominal: ua(&Theta::nominal())?,
                post_mean: ua(&post.mean_theta())?,
                theta_lo: quantile(&mut band, 0.025),
                theta_hi: quantile(&mut band, 0.975),
            },
            n,
        ))
    }

    /// One slice (the whole request: a few thousand sub-millisecond calls).
    ///
    /// # Errors
    /// A wall conductivity TUAS cannot represent, or a TUAS error.
    pub fn slice(&self, id: u32, at: Operating) -> Result<Slice, String> {
        let t0 = bugis::web_demo::platform::now_s();
        let (point, mut evaluations) = self.point(&at.input_at(at.re_hot.get::<ratio>()), 1)?;
        let mut sweep = Vec::with_capacity(N_SWEEP);
        for re in sweep_re() {
            let (p, n) = self.point(&at.input_at(re), BAND_STRIDE)?;
            evaluations += n;
            sweep.push(p);
        }
        Ok(Slice {
            id,
            at: at.to_si(),
            point,
            sweep,
            ms: (bugis::web_demo::platform::now_s() - t0) * 1e3,
            evaluations,
        })
    }

    /// Serve one request.
    pub fn serve(&mut self, req: Request, post: &mut impl FnMut(Event)) {
        let Request::Evaluate { id, at } = req;
        post(match self.slice(id, at) {
            Ok(s) => Event::Slice(s),
            Err(message) => Event::Error { id, message },
        });
    }
}

/// The headless trace (the engine CLAUDE.md rule): at the default slice
/// (Re_cold 15, k_wall 10, L 0.2, the held-out H7's), the live 1-D sweep
/// beside the committed KOH and MFK values at the nine grid Re_hot, as CSV
/// with a fixed header and precision. Deterministic: no clock in the output.
///
/// # Errors
/// The engine's or the artefacts'.
#[cfg_attr(target_arch = "wasm32", allow(dead_code))]
pub fn headless_csv(at: Operating) -> Result<String, String> {
    let e = Engine::default();
    let s = e.slice(1, at)?;
    let a = artefacts::Artefacts::load()?;
    let [_, c, k, l] = at.to_si();
    let (i_rc, i_k, i_l) = (
        artefacts::nearest_level(&artefacts::re_cold_levels(), c),
        artefacts::nearest_level(&artefacts::K_LEVELS, k),
        artefacts::nearest_level(&artefacts::L_LEVELS, l),
    );
    let mut out = format!("# double-pipe twin (gh:#1182) headless: Re_cold {c:.4}, k_wall {k} W/(m K), L {l} m; live 1-D (one_d::ua), {} evaluations\n", s.evaluations);
    out.push_str("source,re_hot,ua_1d_nominal,ua_1d_post_mean,ua_1d_theta_lo95,ua_1d_theta_hi95,koh_median,koh_lo95,koh_hi95,delta_mean_lnUA,mfk_mean,mfk_sd\n");
    for p in &s.sweep {
        out.push_str(&format!(
            "live,{:.6},{:.9e},{:.9e},{:.9e},{:.9e},,,,,,\n",
            p.re_hot, p.nominal, p.post_mean, p.theta_lo, p.theta_hi
        ));
    }
    for i in 0..artefacts::N_RE {
        let g = a.at(i, i_rc, i_k, i_l);
        out.push_str(&format!(
            "grid,{:.6},{:.9e},{:.9e},,,{:.9e},{:.9e},{:.9e},{:+.6e},{:.9e},{:.9e}\n",
            g.x[0],
            g.ua_nominal,
            g.ua_post_mean,
            g.koh_median,
            g.koh_lower95,
            g.koh_upper95,
            g.delta_mean,
            g.mfk_mean,
            g.mfk_sd
        ));
    }
    Ok(out)
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
        Event::Error { id: 0, message }
    }
}

// ─── Wire format: numbers in one Float64Array, text in one string ────────────

const POINT: usize = 5;

impl Point {
    fn push(&self, d: &mut Vec<f64>) {
        d.extend_from_slice(&[
            self.re_hot,
            self.nominal,
            self.post_mean,
            self.theta_lo,
            self.theta_hi,
        ]);
    }
    fn read(d: &[f64]) -> Self {
        Self {
            re_hot: d[0],
            nominal: d[1],
            post_mean: d[2],
            theta_lo: d[3],
            theta_hi: d[4],
        }
    }
}

#[cfg_attr(not(target_arch = "wasm32"), allow(dead_code))]
impl Request {
    pub fn to_wire(&self) -> Vec<f64> {
        let Request::Evaluate { id, at } = self;
        let mut d = vec![0.0, f64::from(*id)];
        d.extend(at.to_si());
        d
    }
    pub fn from_wire(d: &[f64]) -> Result<Self, String> {
        match d {
            [k, id, a, b, c, l] if *k == 0.0 => Ok(Request::Evaluate {
                id: *id as u32,
                at: Operating::si(*a, *b, *c, *l),
            }),
            _ => Err(format!("bad request ({} numbers)", d.len())),
        }
    }
}

#[cfg_attr(not(target_arch = "wasm32"), allow(dead_code))]
impl Event {
    pub fn to_wire(&self) -> (Vec<f64>, String) {
        match self {
            Event::Slice(s) => {
                let mut d = vec![
                    0.0,
                    f64::from(s.id),
                    s.ms,
                    s.evaluations as f64,
                    s.sweep.len() as f64,
                ];
                d.extend(s.at);
                s.point.push(&mut d);
                for p in &s.sweep {
                    p.push(&mut d);
                }
                (d, String::new())
            }
            Event::Error { id, message } => (vec![1.0, f64::from(*id)], message.clone()),
        }
    }
    pub fn from_wire(d: &[f64], text: &str) -> Result<Self, String> {
        const HEAD: usize = 9;
        match d.first().copied() {
            Some(k) if k == 0.0 && d.len() >= HEAD + POINT => {
                let n = d[4] as usize;
                if d.len() != HEAD + POINT * (n + 1) {
                    return Err(format!(
                        "slice length mismatch ({} numbers for {n} points)",
                        d.len()
                    ));
                }
                Ok(Event::Slice(Slice {
                    id: d[1] as u32,
                    ms: d[2],
                    evaluations: d[3] as usize,
                    at: [d[5], d[6], d[7], d[8]],
                    point: Point::read(&d[HEAD..]),
                    sweep: (0..n)
                        .map(|i| Point::read(&d[HEAD + POINT * (i + 1)..]))
                        .collect(),
                }))
            }
            Some(k) if k == 1.0 && d.len() >= 2 => Ok(Event::Error {
                id: d[1] as u32,
                message: text.to_string(),
            }),
            _ => Err(format!("bad event ({} numbers)", d.len())),
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
    use crate::artefacts::{index, re_cold_levels, re_hot_levels, Artefacts, K_LEVELS, L_LEVELS};

    fn default_at() -> Operating {
        default_operating()
    }

    #[test]
    fn requests_and_events_survive_the_wire() {
        let r = Request::Evaluate {
            id: 4,
            at: default_at(),
        };
        assert_eq!(Request::from_wire(&r.to_wire()), Ok(r));
        assert!(Request::from_wire(&[3.0, 1.0]).is_err());
        let mut e = Engine::default();
        let mut got = Vec::new();
        e.serve(r, &mut |ev| got.push(ev));
        let ev = got.pop().expect("an event");
        let (d, t) = ev.to_wire();
        assert_eq!(Event::from_wire(&d, &t), Ok(ev));
        assert!(Event::from_wire(&d[..d.len() - 1], &t).is_err());
        let err = Event::Error {
            id: 2,
            message: "no".into(),
        };
        let (d, t) = err.to_wire();
        assert_eq!(Event::from_wire(&d, &t), Ok(err));
        // A wall TUAS cannot represent is an error event, not a panic.
        e.serve(
            Request::Evaluate {
                id: 9,
                at: Operating::si(30.0, 15.0, 3.0, 0.2),
            },
            &mut |ev| got.push(ev),
        );
        assert!(matches!(got.last(), Some(Event::Error { id: 9, .. })));
        let mut v = vec![3.0, 1.0, 2.0];
        assert_eq!(quantile(&mut v, 0.5), 2.0);
    }

    /// The live 1-D the page draws is the committed one: at every grid node
    /// of three slices the worker's nominal and posterior-mean UA equal the
    /// artefact's bit for bit (same call, same θ), and the θ band holds
    /// the posterior-mean value.
    #[test]
    fn the_live_one_d_is_the_committed_one() {
        let a = Artefacts::load().expect("artefacts");
        let e = Engine::default();
        for (i_rc, i_k, i_l) in [(4, 1, 2), (0, 0, 0), (8, 2, 4)] {
            for i_rh in [0, 3, 8] {
                let g = &a.grid[index(i_rh, i_rc, i_k, i_l)];
                let x = DoublePipeInput::si(
                    re_hot_levels()[i_rh],
                    re_cold_levels()[i_rc],
                    K_LEVELS[i_k],
                    L_LEVELS[i_l],
                );
                let (p, n) = e.point(&x, 1).expect("evaluates");
                assert_eq!(p.nominal, g.ua_nominal);
                assert_eq!(p.post_mean, g.ua_post_mean);
                assert!(
                    p.theta_lo < p.post_mean && p.post_mean < p.theta_hi,
                    "{p:?}"
                );
                assert_eq!(n, a.theta.samples.len() + 2);
            }
        }
    }

    /// **The headless regression test** (the engine CLAUDE.md rule): the
    /// default slice through the headless path, twice, byte-identical; loose
    /// physical bounds (a harness check, not V&V): every UA positive and
    /// rising with Re_hot at nominal θ, the θ band ordered.
    #[test]
    fn headless_is_deterministic_and_bounded() {
        let a = headless_csv(default_at()).expect("runs");
        let b = headless_csv(default_at()).expect("runs");
        assert_eq!(a, b, "deterministic");
        assert_eq!(a.lines().count(), 2 + N_SWEEP + crate::artefacts::N_RE);
        let s = Engine::default().slice(1, default_at()).unwrap();
        assert_eq!(s.sweep.len(), N_SWEEP);
        for w in s.sweep.windows(2) {
            assert!(w[1].nominal > w[0].nominal, "UA rises with Re_hot");
        }
        for p in &s.sweep {
            assert!(p.nominal > 0.0 && p.nominal < 10.0 && p.theta_lo <= p.theta_hi);
        }
        assert!(
            (s.sweep[0].re_hot - 10.0).abs() < 1e-12
                && (s.sweep[N_SWEEP - 1].re_hot - 90.0).abs() < 1e-9
        );
        assert!(s.evaluations > 1000);
    }
}
