//! **The engine**: every calculation the dispersion demo shows, run off the
//! UI thread (the no-lagging HARD RULE) through `dhoby_ghaut::web_demo::link`.
//! Each request is one short calculation (a field of at most ~10^4 cells, or
//! one puff step), so a worker answers it in milliseconds and the UI never
//! waits.
//!
//! **No physics is written here.** Every number is a call into the crates the
//! lessons teach:
//! - the steady plume and its sigmas: `buangkok::pydoseia::dispersion`
//!   (pyDOSEIA's single-plume master equation, BARC/AERB sigmas; rungs 1-3);
//! - `changi`'s Pasquill-Gifford (Martin/ISC) sigmas and the puff field
//!   kernel `changi::puff::wgsl::field_serial` (rung 4), the same CPU kernel
//!   `htgr_sim_v1`'s map uses;
//! - plume rise and wake: `buangkok::pydoseia::plume_rise` (rung 3);
//! - deposition velocities: `buangkok::pydoseia::dose::deposition_velocity_m_per_s`
//!   (pyDOSEIA's SRS-19 screening values), decay in flight:
//!   `changi::activity::decay_transfer::DecayTransfer` (rung 5);
//! - dose: the capstone's recorded pathway doses ([`crate::recorded`]) scaled
//!   by buangkok's `chi/Q`, which is exactly how the capstone computes its
//!   distance table (every pathway is linear in `chi/Q`), and Liu & Cao's
//!   published Table 9 from `buangkok::published` (rungs 6-7).
//!
//! The one piece of logic that is the demo's own is the puff population's
//! bookkeeping (emit, march, drop): the two advection rules of
//! `changi::puff::simulate::AdvectionPolicy` (`LagrangianTrajectory`, the
//! default, and `UpstreamFrozenWind`) restated for an interactive wind,
//! because `changi`'s marching loop is crate-private. It is illustration:
//! the verified numbers are the lessons'.

use buangkok::pydoseia::dispersion::{
    dilution_single_plume_no_met, height_correction_factor, sigma_y, sigma_z, MeanSpeedScaling,
    PlumeGeometry, Receptor, StabilityClass as PlumeClass,
};
use changi::puff::stability::StabilityClass as PuffClass;
use uom::si::f64::{Length, Time, Velocity};
use uom::si::length::{kilometer, meter};
use uom::si::time::second;
use uom::si::velocity::meter_per_second;

/// Seconds a puff lives (upstream `puff`'s default).
pub const PUFF_LIFE_S: f64 = 1200.0;
/// Seconds per puff step; a puff is emitted every step.
pub const PUFF_DT_S: f64 = 10.0;
/// Wind measured at this height, m (pyDOSEIA's reference height).
const MEASUREMENT_HEIGHT_M: f64 = 10.0;

/// Everything the UI can ask for.
#[derive(Clone, Debug, PartialEq)]
pub enum Request {
    /// Ground-level `chi/Q` of the steady plume over a square map.
    Plume { id: u32, p: PlumeParams },
    /// Deposition per unit release over the map: `v_d * chi/Q * decay gain`.
    Deposition {
        id: u32,
        p: PlumeParams,
        element: u8,
        half_life_s: f64,
    },
    /// Both sigma sets against distance for one class.
    Sigmas { id: u32, class: u8 },
    /// Ground-level centreline `chi/Q` without rise, with rise, and in a
    /// building wake.
    Rise {
        id: u32,
        class: u8,
        wind: f64,
        h: f64,
        w0: f64,
        area: f64,
    },
    /// Dose against distance: live scaling of the capstone, and the records.
    Dose { id: u32, class: u8, wind: f64 },
    /// Advance the puff population `steps` steps and return its field.
    Puffs { id: u32, q: PuffParams },
    /// One step animation's numbers (gh:#548, [`crate::steps`]).
    Step {
        id: u32,
        step: u8,
        p: crate::steps::StepParams,
    },
}

/// The steady-plume map's inputs.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct PlumeParams {
    /// Pasquill class 0..=5 (A..F).
    pub class: u8,
    /// Wind speed at 10 m, m/s.
    pub wind: f64,
    /// Release height, m.
    pub h: f64,
    /// Bearing the wind blows TOWARDS, degrees clockwise from north.
    pub dir_deg: f64,
    /// Cells per side of the square map.
    pub cells: u32,
    /// Half-width of the map, m.
    pub half_width: f64,
}

/// One puff step's inputs.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct PuffParams {
    pub class: u8,
    pub wind: f64,
    pub dir_deg: f64,
    pub h: f64,
    pub emit: bool,
    /// Upstream R `puff`'s frozen wind instead of the Lagrangian default.
    pub frozen: bool,
    pub reset: bool,
    pub steps: u32,
    pub cells: u32,
    pub half_width: f64,
}

/// A line or a set of markers on a plot.
#[derive(Clone, Debug, PartialEq)]
pub struct Series {
    pub label: String,
    /// 0 solid (published), 1 dotted (ours), 2 dashed (reference), 3 markers only.
    pub style: u8,
    /// Index into the app's palette.
    pub colour: u8,
    pub xs: Vec<f64>,
    pub ys: Vec<f64>,
}

/// Everything the engine sends back.
#[derive(Clone, Debug, PartialEq)]
pub enum Event {
    /// A map: `cells x cells` values, row 0 north, and the puffs' centres and
    /// `sigma_y` (triples) when the map is the puff train.
    Field {
        id: u32,
        cells: u32,
        half_width: f64,
        values: Vec<f64>,
        puffs: Vec<f64>,
        time_s: f64,
        ms: f64,
    },
    /// Curves for a plot.
    Curves {
        id: u32,
        series: Vec<Series>,
        ms: f64,
    },
    /// A step animation's numbers.
    Step {
        id: u32,
        frame: crate::steps::StepFrame,
        ms: f64,
    },
    /// A worker failure (constructed by the browser worker's error path).
    #[cfg_attr(not(target_arch = "wasm32"), allow(dead_code))]
    Error(String),
}

pub fn plume_class(c: u8) -> PlumeClass {
    PlumeClass::ALL[(c as usize).min(5)]
}

pub fn puff_class(c: u8) -> PuffClass {
    [
        PuffClass::A,
        PuffClass::B,
        PuffClass::C,
        PuffClass::D,
        PuffClass::E,
        PuffClass::F,
    ][(c as usize).min(5)]
}

/// Ground-level `chi/Q` (s/m^3) of buangkok's steady plume at `(x, y)` in the
/// plume's own frame (x downwind), release height `h`, wind `u10` at 10 m.
pub fn plume_chi_over_q(class: u8, u10: f64, h: f64, x: f64, y: f64) -> f64 {
    if x <= 0.0 {
        return 0.0;
    }
    let geometry = PlumeGeometry {
        release_height: Length::new::<meter>(h),
        measurement_height: Length::new::<meter>(MEASUREMENT_HEIGHT_M),
        receptor: Receptor::Offset {
            y: Length::new::<meter>(y),
            z: Length::new::<meter>(0.0),
        },
    };
    let speeds = MeanSpeedScaling::PerClass([Velocity::new::<meter_per_second>(u10); 6]);
    dilution_single_plume_no_met(Length::new::<meter>(x), geometry, speeds)[(class as usize).min(5)]
        .seconds_per_cubic_meter()
}

/// Plume rise, m, at `x` m downwind, for wind `u_stack` at stack height and
/// exit velocity `w0`, with upstream pyDOSEIA's default stack diameters
/// (inner 5 m, outer 8 m): the neutral/unstable formula for A-D, the stable
/// windy formula with the class's own `S` for E and F (the labelled
/// divergence from upstream's always-F, defect D6). Unchecked against the
/// formulas' sources (#542).
pub fn plume_rise_m(class: u8, w0: f64, u_stack: f64, x: f64) -> f64 {
    use buangkok::pydoseia::plume_rise::{
        plume_rise_neutral_unstable, plume_rise_stable_both_formulas, StableClass,
    };
    let (d_i, d_e) = (5.0, 8.0);
    match class {
        4 => plume_rise_stable_both_formulas(w0, u_stack, d_i, StableClass::E).windy_formula,
        5 => plume_rise_stable_both_formulas(w0, u_stack, d_i, StableClass::F).windy_formula,
        _ => plume_rise_neutral_unstable(w0, x, u_stack, d_i, d_e),
    }
}

/// The plume frame of a map point: `(along, across)` for wind blowing towards
/// bearing `dir_deg`.
pub fn to_plume_frame(east: f64, north: f64, dir_deg: f64) -> (f64, f64) {
    let t = dir_deg.to_radians();
    let (dx, dy) = (t.sin(), t.cos());
    (east * dx + north * dy, -east * dy + north * dx)
}

/// Cell centre `(east, north)`, row 0 north (the convention of
/// `changi::puff::wgsl::FieldGrid`).
pub fn cell_centre(cells: u32, half: f64, col: u32, row: u32) -> (f64, f64) {
    let step = 2.0 * half / cells as f64;
    (
        -half + (col as f64 + 0.5) * step,
        half - (row as f64 + 0.5) * step,
    )
}

pub fn plume_field(p: &PlumeParams, per_cell: impl Fn(f64, f64) -> f64) -> Vec<f64> {
    let n = p.cells.clamp(8, 160);
    let mut out = Vec::with_capacity((n * n) as usize);
    for row in 0..n {
        for col in 0..n {
            let (e, nn) = cell_centre(n, p.half_width, col, row);
            let (x, y) = to_plume_frame(e, nn, p.dir_deg);
            out.push(per_cell(x, y));
        }
    }
    out
}

/// Element symbols the deposition rung offers, with pyDOSEIA's grouping.
pub const ELEMENTS: [(&str, &str); 4] = [
    ("Kr", "krypton (noble gas)"),
    ("I", "iodine (pyDOSEIA: aerosol value)"),
    ("Cs", "caesium (particulate)"),
    ("Br", "bromine (reactive halogen)"),
];

/// The live puff population (demo bookkeeping; see the module doc).
#[derive(Clone, Copy, Debug)]
struct DemoPuff {
    x: f64,
    y: f64,
    path: f64,
    age: f64,
    u_emit: f64,
    v_emit: f64,
    class: u8,
}

/// The engine's state: only the puff population persists between requests.
#[derive(Default)]
pub struct Engine {
    puffs: Vec<DemoPuff>,
    time_s: f64,
}

fn log_space(a: f64, b: f64, n: usize) -> Vec<f64> {
    let (la, lb) = (a.ln(), b.ln());
    (0..n)
        .map(|i| (la + (lb - la) * i as f64 / (n - 1) as f64).exp())
        .collect()
}

impl Engine {
    /// Serve one request, posting its answer.
    pub fn serve(&mut self, req: Request, post: &mut impl FnMut(Event)) {
        let t0 = dhoby_ghaut::web_demo::platform::now_s();
        let ms = |t0: f64| (dhoby_ghaut::web_demo::platform::now_s() - t0) * 1e3;
        match req {
            Request::Plume { id, p } => {
                let values = plume_field(&p, |x, y| plume_chi_over_q(p.class, p.wind, p.h, x, y));
                post(Event::Field {
                    id,
                    cells: p.cells.clamp(8, 160),
                    half_width: p.half_width,
                    values,
                    puffs: Vec::new(),
                    time_s: 0.0,
                    ms: ms(t0),
                });
            }
            Request::Deposition {
                id,
                p,
                element,
                half_life_s,
            } => {
                let symbol = ELEMENTS[(element as usize).min(ELEMENTS.len() - 1)].0;
                let vd = buangkok::pydoseia::dose::deposition_velocity_m_per_s(symbol);
                let decay =
                    changi::activity::decay_transfer::DecayTransfer::from_half_life(Time::new::<
                        second,
                    >(
                        half_life_s
                    ));
                let class = plume_class(p.class);
                let u_release = p.wind
                    * height_correction_factor(
                        class,
                        Length::new::<meter>(p.h),
                        Length::new::<meter>(MEASUREMENT_HEIGHT_M),
                    );
                let values = plume_field(&p, |x, y| {
                    if x <= 0.0 {
                        return 0.0;
                    }
                    let gain = decay.gain(Time::new::<second>(x / u_release)).value;
                    vd * plume_chi_over_q(p.class, p.wind, p.h, x, y) * gain
                });
                post(Event::Field {
                    id,
                    cells: p.cells.clamp(8, 160),
                    half_width: p.half_width,
                    values,
                    puffs: Vec::new(),
                    time_s: 0.0,
                    ms: ms(t0),
                });
            }
            Request::Sigmas { id, class } => {
                let xs = log_space(50.0, 20_000.0, 120);
                let c = puff_class(class);
                let pc = plume_class(class);
                let mut sy_c = Vec::new();
                let mut sz_c = Vec::new();
                for &x in &xs {
                    let s = changi::puff::dispersion::pasquill_gifford_sigmas(
                        c,
                        Length::new::<meter>(x),
                    )
                    .expect("x > 0 has sigmas");
                    sy_c.push(s.sigma_y.get::<meter>());
                    sz_c.push(s.sigma_z.get::<meter>());
                }
                let sy_b: Vec<f64> = xs
                    .iter()
                    .map(|&x| sigma_y(pc, Length::new::<meter>(x)).get::<meter>())
                    .collect();
                let sz_b: Vec<f64> = xs
                    .iter()
                    .map(|&x| sigma_z(pc, Length::new::<meter>(x)).get::<meter>())
                    .collect();
                let series = vec![
                    Series {
                        label: "sigma_y, changi (Martin / US EPA ISC)".into(),
                        style: 1,
                        colour: 0,
                        xs: xs.clone(),
                        ys: sy_c,
                    },
                    Series {
                        label: "sigma_z, changi (Martin / US EPA ISC)".into(),
                        style: 2,
                        colour: 0,
                        xs: xs.clone(),
                        ys: sz_c,
                    },
                    Series {
                        label: "sigma_y, buangkok (pyDOSEIA, BARC/AERB)".into(),
                        style: 1,
                        colour: 1,
                        xs: xs.clone(),
                        ys: sy_b,
                    },
                    Series {
                        label: "sigma_z, buangkok (pyDOSEIA, BARC/AERB)".into(),
                        style: 2,
                        colour: 1,
                        xs,
                        ys: sz_b,
                    },
                ];
                post(Event::Curves {
                    id,
                    series,
                    ms: ms(t0),
                });
            }
            Request::Rise {
                id,
                class,
                wind,
                h,
                w0,
                area,
            } => {
                use buangkok::pydoseia::plume_rise::building_wake_gifford;
                let pc = plume_class(class);
                let u_stack = wind
                    * height_correction_factor(
                        pc,
                        Length::new::<meter>(h),
                        Length::new::<meter>(MEASUREMENT_HEIGHT_M),
                    );
                let xs = log_space(50.0, 20_000.0, 160);
                let rise = |x: f64| plume_rise_m(class, w0, u_stack, x);
                let no_rise: Vec<f64> = xs
                    .iter()
                    .map(|&x| plume_chi_over_q(class, wind, h, x, 0.0))
                    .collect();
                let with_rise: Vec<f64> = xs
                    .iter()
                    .map(|&x| plume_chi_over_q(class, wind, h + rise(x).max(0.0), x, 0.0))
                    .collect();
                let u_ground = wind
                    * height_correction_factor(
                        pc,
                        Length::new::<meter>(0.0),
                        Length::new::<meter>(MEASUREMENT_HEIGHT_M),
                    );
                let wake: Vec<f64> = xs
                    .iter()
                    .map(|&x| {
                        let unwaked = plume_chi_over_q(class, wind, 0.0, x, 0.0);
                        let l = Length::new::<meter>(x);
                        building_wake_gifford(
                            unwaked,
                            area,
                            u_ground,
                            sigma_y(pc, l).get::<meter>(),
                            sigma_z(pc, l).get::<meter>(),
                        )
                    })
                    .collect();
                let ground: Vec<f64> = xs
                    .iter()
                    .map(|&x| plume_chi_over_q(class, wind, 0.0, x, 0.0))
                    .collect();
                let series = vec![
                    Series {
                        label: format!("stack {h:.0} m, no rise"),
                        style: 1,
                        colour: 0,
                        xs: xs.clone(),
                        ys: no_rise,
                    },
                    Series {
                        label: "stack + plume rise (pyDOSEIA formula, unchecked: #542)".into(),
                        style: 1,
                        colour: 1,
                        xs: xs.clone(),
                        ys: with_rise,
                    },
                    Series {
                        label: "ground release, no wake".into(),
                        style: 2,
                        colour: 2,
                        xs: xs.clone(),
                        ys: ground,
                    },
                    Series {
                        label: format!(
                            "ground release in a {area:.0} m2 building wake (Gifford, unchecked)"
                        ),
                        style: 1,
                        colour: 3,
                        xs,
                        ys: wake,
                    },
                ];
                post(Event::Curves {
                    id,
                    series,
                    ms: ms(t0),
                });
            }
            Request::Dose { id, class, wind } => {
                post(Event::Curves {
                    id,
                    series: dose_series(Some((class, wind))),
                    ms: ms(t0),
                });
            }
            Request::Step { id, step, p } => {
                match crate::steps::Step::from_code(step) {
                    Some(st) => post(Event::Step {
                        id,
                        frame: crate::steps::compute(st, p),
                        ms: ms(t0),
                    }),
                    None => post(Event::Error(format!("unknown step {step}"))),
                }
            }
            Request::Puffs { id, q } => {
                let values = self.step_puffs(&q);
                let puffs = self
                    .puffs
                    .iter()
                    .flat_map(|p| [p.x, p.y, self.sigma_y_of(p)])
                    .collect();
                post(Event::Field {
                    id,
                    cells: q.cells.clamp(8, 128),
                    half_width: q.half_width,
                    values,
                    puffs,
                    time_s: self.time_s,
                    ms: ms(t0),
                });
            }
        }
    }

    fn sigma_y_of(&self, p: &DemoPuff) -> f64 {
        changi::puff::dispersion::pasquill_gifford_sigmas(
            puff_class(p.class),
            Length::new::<meter>(p.path),
        )
        .map_or(0.0, |s| s.sigma_y.get::<meter>())
    }

    /// March the population and return its ground-level field (relative:
    /// unit mass per puff).
    fn step_puffs(&mut self, q: &PuffParams) -> Vec<f64> {
        if q.reset {
            self.puffs.clear();
            self.time_s = 0.0;
        }
        let t = q.dir_deg.to_radians();
        let (u, v) = (q.wind * t.sin(), q.wind * t.cos());
        for _ in 0..q.steps.min(60) {
            // Advect what is aloft, on the wind that blows now (Lagrangian),
            // or recompute from age on the wind of birth (upstream's rule).
            for p in &mut self.puffs {
                p.age += PUFF_DT_S;
                if q.frozen {
                    p.x = p.u_emit * p.age;
                    p.y = p.v_emit * p.age;
                    p.path = p.x.hypot(p.y);
                } else {
                    p.x += u * PUFF_DT_S;
                    p.y += v * PUFF_DT_S;
                    p.path += q.wind * PUFF_DT_S;
                }
            }
            self.puffs.retain(|p| p.age <= PUFF_LIFE_S);
            if q.emit {
                // Each puff keeps the class it was born in (the #352 lesson).
                self.puffs.push(DemoPuff {
                    x: 0.0,
                    y: 0.0,
                    path: 0.0,
                    age: 0.0,
                    u_emit: u,
                    v_emit: v,
                    class: q.class,
                });
            }
            self.time_s += PUFF_DT_S;
        }
        let states: Vec<changi::puff::wgsl::PuffState> = self
            .puffs
            .iter()
            .filter_map(|p| {
                let s = changi::puff::dispersion::pasquill_gifford_sigmas(
                    puff_class(p.class),
                    Length::new::<meter>(p.path),
                )?;
                Some(changi::puff::wgsl::PuffState {
                    x: p.x as f32,
                    y: p.y as f32,
                    sigma_y: s.sigma_y.get::<meter>() as f32,
                    sigma_z: s.sigma_z.get::<meter>() as f32,
                    weight: 1.0,
                })
            })
            .collect();
        let grid = changi::puff::wgsl::FieldGrid {
            cells: q.cells.clamp(8, 128) as usize,
            half_width_m: q.half_width as f32,
            source_height_m: q.h as f32,
        };
        changi::puff::wgsl::field_serial(&states, &grid)
            .into_iter()
            .map(f64::from)
            .collect()
    }
}

/// Dose against distance: the capstone's recorded points, Liu & Cao's
/// published Table 9 (context: a different accident), and, when asked, the
/// capstone's pathway doses scaled by buangkok's `chi/Q` for another class
/// and wind (the capstone's own arithmetic; ground release).
pub fn dose_series(live: Option<(u8, f64)>) -> Vec<Series> {
    use crate::recorded::{CHI_OVER_Q_400M_F_1MS, DOSE_400M_SV, DOSE_VS_DISTANCE};
    let mut out = Vec::new();
    let published =
        buangkok::published::accident_dose_by_distance::htr10_accident_dose_by_distance();
    let km =
        |r: &buangkok::published::accident_dose_by_distance::PublishedAccidentDoseAtDistance| {
            r.distance.get::<kilometer>() * 1e3
        };
    out.push(Series {
        label: "Liu & Cao 2002 Table 9, depressurisation, whole body (published; a DIFFERENT accident and model, context only)".into(),
        style: 0,
        colour: 4,
        xs: published.iter().map(km).collect(),
        ys: published.iter().map(|r| r.depressurization.whole_body_msv).collect(),
    });
    out.push(Series {
        label: "Liu & Cao 2002 Table 9, water ingress, whole body (published; context only)".into(),
        style: 0,
        colour: 5,
        xs: published.iter().map(km).collect(),
        ys: published
            .iter()
            .map(|r| r.water_ingress.whole_body_msv)
            .collect(),
    });
    out.push(Series {
        label: "capstone, recorded (ours: class F, 1 m/s, ground release)".into(),
        style: 3,
        colour: 0,
        xs: DOSE_VS_DISTANCE.iter().map(|p| p.0).collect(),
        ys: DOSE_VS_DISTANCE.iter().map(|p| p.1).collect(),
    });
    if let Some((class, wind)) = live {
        let d400: f64 = DOSE_400M_SV.iter().map(|p| p.1).sum::<f64>() * 1e3;
        let xs = log_space(250.0, 75_000.0, 120);
        let ys = xs
            .iter()
            .map(|&x| d400 * plume_chi_over_q(class, wind, 0.0, x, 0.0) / CHI_OVER_Q_400M_F_1MS)
            .collect();
        out.push(Series {
            label: format!(
                "capstone release, class {} at {wind:.1} m/s (ours, scaled live by chi/Q)",
                ["A", "B", "C", "D", "E", "F"][(class as usize).min(5)]
            ),
            style: 1,
            colour: 1,
            xs,
            ys,
        });
    }
    out
}

// ─── The two engine shapes the framework runs ────────────────────────────────

#[cfg(not(target_arch = "wasm32"))]
impl dhoby_ghaut::web_demo::link::NativeEngine for Engine {
    type Req = Request;
    type Ev = Event;
    fn handle(&mut self, req: Request, post: &mut impl FnMut(Event)) {
        self.serve(req, post);
    }
}

#[cfg(target_arch = "wasm32")]
impl dhoby_ghaut::web_demo::link::WorkerEngine for Engine {
    type Req = Request;
    type Ev = Event;
    fn handle(
        state: &std::sync::Arc<std::sync::RwLock<Self>>,
        req: Request,
        post: dhoby_ghaut::web_demo::link::Poster<Event>,
    ) {
        if let Ok(mut e) = state.write() {
            e.serve(req, &mut |ev| post.post(ev));
        }
    }
    fn error(message: String) -> Event {
        Event::Error(message)
    }
}

// ─── Wire format: numbers in one Float64Array, text in one string ────────────
//
// Natively the enums cross the thread boundary as they are; in the browser
// they cross as a JS object `{kind: "msg", data: Float64Array, text}`. The
// flattening is plain Rust so that the native tests check the round trip the
// worker relies on.

#[cfg_attr(not(target_arch = "wasm32"), allow(dead_code))]
impl Request {
    pub fn to_wire(&self) -> Vec<f64> {
        let pp = |p: &PlumeParams| {
            vec![
                p.class as f64,
                p.wind,
                p.h,
                p.dir_deg,
                p.cells as f64,
                p.half_width,
            ]
        };
        match self {
            Request::Plume { id, p } => [vec![0.0, *id as f64], pp(p)].concat(),
            Request::Deposition {
                id,
                p,
                element,
                half_life_s,
            } => [
                vec![1.0, *id as f64],
                pp(p),
                vec![*element as f64, *half_life_s],
            ]
            .concat(),
            Request::Sigmas { id, class } => vec![2.0, *id as f64, *class as f64],
            Request::Rise {
                id,
                class,
                wind,
                h,
                w0,
                area,
            } => vec![3.0, *id as f64, *class as f64, *wind, *h, *w0, *area],
            Request::Dose { id, class, wind } => vec![4.0, *id as f64, *class as f64, *wind],
            Request::Puffs { id, q } => vec![
                5.0,
                *id as f64,
                q.class as f64,
                q.wind,
                q.dir_deg,
                q.h,
                q.emit as u8 as f64,
                q.frozen as u8 as f64,
                q.reset as u8 as f64,
                q.steps as f64,
                q.cells as f64,
                q.half_width,
            ],
            Request::Step { id, step, p } => vec![
                6.0,
                *id as f64,
                *step as f64,
                p.class as f64,
                p.wind,
                p.h,
                p.x,
                p.w0,
            ],
        }
    }

    pub fn from_wire(d: &[f64]) -> Result<Request, String> {
        let g = |i: usize| {
            d.get(i)
                .copied()
                .ok_or_else(|| format!("request too short ({} values)", d.len()))
        };
        let pp = |o: usize| -> Result<PlumeParams, String> {
            Ok(PlumeParams {
                class: g(o)? as u8,
                wind: g(o + 1)?,
                h: g(o + 2)?,
                dir_deg: g(o + 3)?,
                cells: g(o + 4)? as u32,
                half_width: g(o + 5)?,
            })
        };
        let id = g(1)? as u32;
        Ok(match g(0)? as u8 {
            0 => Request::Plume { id, p: pp(2)? },
            1 => Request::Deposition {
                id,
                p: pp(2)?,
                element: g(8)? as u8,
                half_life_s: g(9)?,
            },
            2 => Request::Sigmas {
                id,
                class: g(2)? as u8,
            },
            3 => Request::Rise {
                id,
                class: g(2)? as u8,
                wind: g(3)?,
                h: g(4)?,
                w0: g(5)?,
                area: g(6)?,
            },
            4 => Request::Dose {
                id,
                class: g(2)? as u8,
                wind: g(3)?,
            },
            5 => Request::Puffs {
                id,
                q: PuffParams {
                    class: g(2)? as u8,
                    wind: g(3)?,
                    dir_deg: g(4)?,
                    h: g(5)?,
                    emit: g(6)? != 0.0,
                    frozen: g(7)? != 0.0,
                    reset: g(8)? != 0.0,
                    steps: g(9)? as u32,
                    cells: g(10)? as u32,
                    half_width: g(11)?,
                },
            },
            6 => Request::Step {
                id,
                step: g(2)? as u8,
                p: crate::steps::StepParams {
                    class: g(3)? as u8,
                    wind: g(4)?,
                    h: g(5)?,
                    x: g(6)?,
                    w0: g(7)?,
                },
            },
            k => return Err(format!("unknown request kind {k}")),
        })
    }
}

#[cfg_attr(not(target_arch = "wasm32"), allow(dead_code))]
impl Event {
    /// Numbers, and the text (series labels joined by `\u{1f}`, or an error).
    pub fn to_wire(&self) -> (Vec<f64>, String) {
        match self {
            Event::Field {
                id,
                cells,
                half_width,
                values,
                puffs,
                time_s,
                ms,
            } => {
                let mut d = vec![
                    0.0,
                    *id as f64,
                    *cells as f64,
                    *half_width,
                    *time_s,
                    *ms,
                    values.len() as f64,
                    puffs.len() as f64,
                ];
                d.extend_from_slice(values);
                d.extend_from_slice(puffs);
                (d, String::new())
            }
            Event::Curves { id, series, ms } => {
                let mut d = vec![1.0, *id as f64, *ms, series.len() as f64];
                for s in series {
                    d.extend_from_slice(&[s.style as f64, s.colour as f64, s.xs.len() as f64]);
                    d.extend_from_slice(&s.xs);
                    d.extend_from_slice(&s.ys);
                }
                let text = series
                    .iter()
                    .map(|s| s.label.as_str())
                    .collect::<Vec<_>>()
                    .join("\u{1f}");
                (d, text)
            }
            Event::Error(m) => (vec![2.0], m.clone()),
            Event::Step { id, frame, ms } => {
                let (f, text) = frame.to_wire();
                let mut d = vec![3.0, *id as f64, *ms];
                d.extend(f);
                (d, text)
            }
        }
    }

    pub fn from_wire(d: &[f64], text: &str) -> Result<Event, String> {
        let g = |i: usize| {
            d.get(i)
                .copied()
                .ok_or_else(|| "event too short".to_string())
        };
        Ok(match g(0)? as u8 {
            0 => {
                let (nv, np) = (g(6)? as usize, g(7)? as usize);
                if d.len() < 8 + nv + np {
                    return Err("field event truncated".into());
                }
                Event::Field {
                    id: g(1)? as u32,
                    cells: g(2)? as u32,
                    half_width: g(3)?,
                    time_s: g(4)?,
                    ms: g(5)?,
                    values: d[8..8 + nv].to_vec(),
                    puffs: d[8 + nv..8 + nv + np].to_vec(),
                }
            }
            1 => {
                let n = g(3)? as usize;
                let mut labels = text.split('\u{1f}');
                let mut series = Vec::with_capacity(n);
                let mut i = 4;
                for _ in 0..n {
                    let (style, colour, len) = (g(i)? as u8, g(i + 1)? as u8, g(i + 2)? as usize);
                    i += 3;
                    if d.len() < i + 2 * len {
                        return Err("curves event truncated".into());
                    }
                    let xs = d[i..i + len].to_vec();
                    let ys = d[i + len..i + 2 * len].to_vec();
                    i += 2 * len;
                    series.push(Series {
                        label: labels.next().unwrap_or("").to_string(),
                        style,
                        colour,
                        xs,
                        ys,
                    });
                }
                Event::Curves {
                    id: g(1)? as u32,
                    series,
                    ms: g(2)?,
                }
            }
            2 => Event::Error(text.to_string()),
            3 => Event::Step {
                id: g(1)? as u32,
                ms: g(2)?,
                frame: crate::steps::StepFrame::from_wire(&d[3..], text)?,
            },
            k => return Err(format!("unknown event kind {k}")),
        })
    }
}

#[cfg(target_arch = "wasm32")]
mod web {
    use super::{Event, Request};
    use dhoby_ghaut::web_demo::link::{js, Message};
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

    fn p() -> PlumeParams {
        PlumeParams {
            class: 3,
            wind: 2.0,
            h: 30.0,
            dir_deg: 90.0,
            cells: 40,
            half_width: 2000.0,
        }
    }

    /// Every request and event survives the worker's wire format.
    #[test]
    fn messages_round_trip_through_the_wire_format() {
        let q = PuffParams {
            class: 5,
            wind: 3.5,
            dir_deg: 45.0,
            h: 20.0,
            emit: true,
            frozen: false,
            reset: true,
            steps: 4,
            cells: 32,
            half_width: 3000.0,
        };
        for r in [
            Request::Plume { id: 7, p: p() },
            Request::Deposition {
                id: 8,
                p: p(),
                element: 2,
                half_life_s: 189.0,
            },
            Request::Sigmas { id: 9, class: 0 },
            Request::Rise {
                id: 10,
                class: 4,
                wind: 2.0,
                h: 30.0,
                w0: 10.0,
                area: 1000.0,
            },
            Request::Dose {
                id: 11,
                class: 5,
                wind: 1.0,
            },
            Request::Puffs { id: 12, q },
        ] {
            assert_eq!(Request::from_wire(&r.to_wire()).unwrap(), r);
        }
        let mut e = Engine::default();
        let mut got = Vec::new();
        e.serve(
            Request::Dose {
                id: 1,
                class: 5,
                wind: 1.0,
            },
            &mut |ev| got.push(ev),
        );
        e.serve(Request::Puffs { id: 2, q }, &mut |ev| got.push(ev));
        for ev in got {
            let (d, t) = ev.to_wire();
            assert_eq!(Event::from_wire(&d, &t).unwrap(), ev);
        }
    }

    /// The map is buangkok's plume: on the plume axis, east of the source for
    /// a wind blowing towards 90 degrees, the cell value is the ground-level
    /// centreline chi/Q the rung-1 example prints; upwind is zero.
    #[test]
    fn the_plume_map_is_buangkoks_plume() {
        // Rung 1's record: class D, 2 m/s at 10 m, H = 30 m, 1000 m: 3.643e-5.
        let v = plume_chi_over_q(3, 2.0, 30.0, 1000.0, 0.0);
        assert!((v / 3.643e-5 - 1.0).abs() < 2e-4, "{v}");
        let (x, y) = to_plume_frame(1000.0, 0.0, 90.0);
        assert!((x - 1000.0).abs() < 1e-9 && y.abs() < 1e-9);
        assert_eq!(plume_chi_over_q(3, 2.0, 30.0, -10.0, 0.0), 0.0);
    }

    /// The live dose curve, at the capstone's own class and wind, reproduces
    /// the capstone's recorded table (the capstone is linear in chi/Q).
    #[test]
    fn the_live_dose_curve_reproduces_the_recorded_capstone() {
        let d400: f64 = crate::recorded::DOSE_400M_SV
            .iter()
            .map(|p| p.1)
            .sum::<f64>()
            * 1e3;
        for (x, dose) in crate::recorded::DOSE_VS_DISTANCE {
            let live = d400 * plume_chi_over_q(5, 1.0, 0.0, x, 0.0)
                / crate::recorded::CHI_OVER_Q_400M_F_1MS;
            assert!(
                (live / dose - 1.0).abs() < 2e-3,
                "{x} m: live {live} vs recorded {dose}"
            );
        }
    }

    /// Puffs remember: after the wind turns from east to north, the old puffs
    /// stay east of the source under the Lagrangian rule, and upstream's
    /// frozen wind keeps flying them east (the #344 lesson).
    #[test]
    fn the_puff_train_bends_only_under_the_lagrangian_rule() {
        let base = PuffParams {
            class: 3,
            wind: 5.0,
            dir_deg: 90.0,
            h: 10.0,
            emit: true,
            frozen: false,
            reset: true,
            steps: 20,
            cells: 16,
            half_width: 3000.0,
        };
        for frozen in [false, true] {
            let mut e = Engine::default();
            e.step_puffs(&PuffParams { frozen, ..base });
            e.step_puffs(&PuffParams {
                frozen,
                reset: false,
                dir_deg: 0.0,
                emit: false,
                ..base
            });
            let oldest = e
                .puffs
                .iter()
                .max_by(|a, b| a.age.total_cmp(&b.age))
                .unwrap();
            if frozen {
                assert!(oldest.y.abs() < 1e-9, "frozen: no northward motion");
            } else {
                assert!(
                    oldest.y > 900.0 && oldest.x > 900.0,
                    "lagrangian: east then north ({}, {})",
                    oldest.x,
                    oldest.y
                );
            }
        }
    }
}
