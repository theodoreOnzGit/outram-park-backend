//! The helical-coil steam generator solve, off the UI thread: a background
//! thread natively, the Web Worker in the browser (`web/helical_sg/`). Every
//! request is ONE march of the shooting iteration, so a new input or a stale
//! solve is dropped within one march.
//!
//! **The physics is not here.** It is `tampines::components::helical_coil_otsg`
//! (gh:#1157), unchanged: the IF97 water-side march with the coil, boiling,
//! dryout and two-phase pressure-drop closures cited there. This file only
//! builds its inputs from the sliders, steps its [`Shooting`] and flattens the
//! profile for the page.

use tampines::components::helical_coil_otsg::{
    HelicalCoilGeometry, HelicalCoilOtsg, OperatingPoint, OtsgOptions, OtsgSolution, Shooting,
};
use uom::si::f64::{
    HeatTransfer, Length, MassRate, Pressure, SpecificHeatCapacity, ThermalConductivity,
    ThermodynamicTemperature,
};
use uom::si::heat_flux_density::watt_per_square_meter;
use uom::si::heat_transfer::watt_per_square_meter_kelvin;
use uom::si::length::meter;
use uom::si::mass_rate::kilogram_per_second;
use uom::si::power::watt;
use uom::si::pressure::pascal;
use uom::si::ratio::ratio;
use uom::si::specific_heat_capacity::joule_per_kilogram_kelvin;
use uom::si::thermal_conductivity::watt_per_meter_kelvin;
use uom::si::thermodynamic_temperature::kelvin;

/// Every input the side panel sets, in SI, flat so it crosses the worker
/// boundary as numbers.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Params {
    pub coil_diameter_m: f64,
    pub pitch_m: f64,
    pub turns: f64,
    pub tube_inner_diameter_m: f64,
    pub wall_thickness_m: f64,
    pub wall_conductivity: f64,
    pub tubes: f64,
    pub water_flow: f64,
    pub feed_temperature_k: f64,
    pub inlet_pressure_pa: f64,
    pub hot_flow: f64,
    pub hot_inlet_k: f64,
    pub hot_cp: f64,
    pub shell_htc: f64,
    pub nodes: f64,
    /// Physics switches, bit `i` = term `i` of [`TERMS`] is ON.
    pub terms: u32,
}

/// The physics terms the panel can ablate, in bit order.
pub const TERMS: [&str; 7] = [
    "coil curvature",
    "nucleate boiling",
    "subcooled boiling",
    "dryout (Biasi CHF)",
    "gravity",
    "acceleration",
    "wall resistance",
];

/// Numbers in [`Params`] on the wire.
const N_PARAMS: usize = 16;

impl Default for Params {
    /// The library's illustrative HTR-10-scale case
    /// (`HelicalCoilGeometry::illustrative`, `OperatingPoint::illustrative`),
    /// 200 cells, every physics term on.
    fn default() -> Self {
        Self::from_library(
            &HelicalCoilGeometry::illustrative(),
            &OperatingPoint::illustrative(),
            OtsgOptions::default(),
        )
    }
}

impl Params {
    /// Read a library case into slider values.
    pub fn from_library(g: &HelicalCoilGeometry, o: &OperatingPoint, opt: OtsgOptions) -> Self {
        let flags = [
            opt.curvature,
            opt.nucleate_boiling,
            opt.subcooled_boiling,
            opt.dryout,
            opt.gravity,
            opt.acceleration,
            opt.wall_resistance,
        ];
        Self {
            coil_diameter_m: g.coil_diameter.get::<meter>(),
            pitch_m: g.pitch.get::<meter>(),
            turns: g.turns,
            tube_inner_diameter_m: g.tube_inner_diameter.get::<meter>(),
            wall_thickness_m: 0.5
                * (g.tube_outer_diameter.get::<meter>() - g.tube_inner_diameter.get::<meter>()),
            wall_conductivity: g.wall_conductivity.get::<watt_per_meter_kelvin>(),
            tubes: f64::from(g.parallel_tubes),
            water_flow: o.water_mass_flow.get::<kilogram_per_second>(),
            feed_temperature_k: o.water_inlet_temperature.get::<kelvin>(),
            inlet_pressure_pa: o.water_inlet_pressure.get::<pascal>(),
            hot_flow: o.hot_mass_flow.get::<kilogram_per_second>(),
            hot_inlet_k: o.hot_inlet_temperature.get::<kelvin>(),
            hot_cp: o.hot_specific_heat.get::<joule_per_kilogram_kelvin>(),
            shell_htc: o.shell_side_htc.get::<watt_per_square_meter_kelvin>(),
            nodes: opt.nodes as f64,
            terms: flags
                .iter()
                .enumerate()
                .fold(0u32, |acc, (i, &on)| acc | (u32::from(on) << i)),
        }
    }

    /// Is physics term `i` on.
    pub fn term(&self, i: usize) -> bool {
        self.terms & (1 << i) != 0
    }

    /// The library geometry these values describe (not validated here).
    pub fn geometry(&self) -> HelicalCoilGeometry {
        HelicalCoilGeometry {
            coil_diameter: Length::new::<meter>(self.coil_diameter_m),
            pitch: Length::new::<meter>(self.pitch_m),
            turns: self.turns,
            tube_inner_diameter: Length::new::<meter>(self.tube_inner_diameter_m),
            tube_outer_diameter: Length::new::<meter>(
                self.tube_inner_diameter_m + 2.0 * self.wall_thickness_m,
            ),
            wall_conductivity: ThermalConductivity::new::<watt_per_meter_kelvin>(
                self.wall_conductivity,
            ),
            parallel_tubes: self.tubes.round().max(1.0) as u32,
        }
    }

    /// The validated model, or the library's refusal as text.
    pub fn model(&self) -> Result<HelicalCoilOtsg, String> {
        let op = OperatingPoint {
            water_mass_flow: MassRate::new::<kilogram_per_second>(self.water_flow),
            water_inlet_temperature: ThermodynamicTemperature::new::<kelvin>(
                self.feed_temperature_k,
            ),
            water_inlet_pressure: Pressure::new::<pascal>(self.inlet_pressure_pa),
            hot_mass_flow: MassRate::new::<kilogram_per_second>(self.hot_flow),
            hot_inlet_temperature: ThermodynamicTemperature::new::<kelvin>(self.hot_inlet_k),
            hot_specific_heat: SpecificHeatCapacity::new::<joule_per_kilogram_kelvin>(self.hot_cp),
            shell_side_htc: HeatTransfer::new::<watt_per_square_meter_kelvin>(self.shell_htc),
        };
        let mut opt = OtsgOptions::default().with_nodes(self.nodes.round().max(2.0) as usize);
        let ablations: [fn(OtsgOptions) -> OtsgOptions; 7] = [
            OtsgOptions::without_curvature,
            OtsgOptions::without_nucleate_boiling,
            OtsgOptions::without_subcooled_boiling,
            OtsgOptions::without_dryout,
            OtsgOptions::without_gravity,
            OtsgOptions::without_acceleration,
            OtsgOptions::without_wall_resistance,
        ];
        for (i, off) in ablations.iter().enumerate() {
            if !self.term(i) {
                opt = off(opt);
            }
        }
        HelicalCoilOtsg::new(self.geometry(), op, opt).map_err(|e| e.to_string())
    }

    fn to_wire(self) -> [f64; N_PARAMS] {
        [
            self.coil_diameter_m,
            self.pitch_m,
            self.turns,
            self.tube_inner_diameter_m,
            self.wall_thickness_m,
            self.wall_conductivity,
            self.tubes,
            self.water_flow,
            self.feed_temperature_k,
            self.inlet_pressure_pa,
            self.hot_flow,
            self.hot_inlet_k,
            self.hot_cp,
            self.shell_htc,
            self.nodes,
            f64::from(self.terms),
        ]
    }

    fn from_wire(d: &[f64]) -> Option<Self> {
        if d.len() < N_PARAMS {
            return None;
        }
        Some(Self {
            coil_diameter_m: d[0],
            pitch_m: d[1],
            turns: d[2],
            tube_inner_diameter_m: d[3],
            wall_thickness_m: d[4],
            wall_conductivity: d[5],
            tubes: d[6],
            water_flow: d[7],
            feed_temperature_k: d[8],
            inlet_pressure_pa: d[9],
            hot_flow: d[10],
            hot_inlet_k: d[11],
            hot_cp: d[12],
            shell_htc: d[13],
            nodes: d[14],
            terms: d[15] as u32,
        })
    }
}

/// What the UI asks for.
#[derive(Clone, Debug, PartialEq)]
pub enum Request {
    /// A new solve (replacing any running one); runs its first march.
    Solve { id: u32, params: Params },
    /// The next march of solve `id`.
    Step { id: u32 },
}

/// The per-node profile, flattened, one `Vec` per quantity (SI).
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Profile {
    pub s: Vec<f64>,
    pub t_water: Vec<f64>,
    pub t_hot: Vec<f64>,
    pub t_wall: Vec<f64>,
    pub quality: Vec<f64>,
    pub pressure: Vec<f64>,
    pub heat_flux: Vec<f64>,
    pub regime: Vec<f64>,
    /// `q''/q''_CHF`, NaN where not evaluated.
    pub chf_ratio: Vec<f64>,
}

const PROFILE_FIELDS: usize = 9;

impl Profile {
    fn from_solution(s: &OtsgSolution) -> Self {
        let mut p = Profile::default();
        for n in &s.points {
            p.s.push(n.length.get::<meter>());
            p.t_water.push(n.water_temperature.get::<kelvin>());
            p.t_hot.push(n.hot_temperature.get::<kelvin>());
            p.t_wall.push(n.inner_wall_temperature.get::<kelvin>());
            p.quality.push(n.equilibrium_quality.get::<ratio>());
            p.pressure.push(n.pressure.get::<pascal>());
            p.heat_flux.push(n.inner_heat_flux.get::<watt_per_square_meter>());
            p.regime.push(f64::from(n.regime.code()));
            p.chf_ratio.push(n.chf_ratio.unwrap_or(f64::NAN));
        }
        p
    }
    fn fields(&self) -> [&Vec<f64>; PROFILE_FIELDS] {
        [
            &self.s,
            &self.t_water,
            &self.t_hot,
            &self.t_wall,
            &self.quality,
            &self.pressure,
            &self.heat_flux,
            &self.regime,
            &self.chf_ratio,
        ]
    }
    pub fn len(&self) -> usize {
        self.s.len()
    }
}

/// One report of a solve.
#[derive(Clone, Debug, PartialEq)]
pub struct Snapshot {
    pub id: u32,
    pub iteration: usize,
    pub converged: bool,
    pub duty_water_w: f64,
    pub duty_hot_w: f64,
    pub closure: f64,
    pub hot_outlet_k: f64,
    pub hot_inlet_returned_k: f64,
    pub dp_friction: f64,
    pub dp_gravity: f64,
    pub dp_acceleration: f64,
    /// Tube length at dryout \[m\], NaN if the tube does not dry out.
    pub dryout_s: f64,
    /// Wall-clock milliseconds of this march.
    pub ms: f64,
    pub profile: Profile,
}

impl Snapshot {
    fn from_solution(id: u32, s: &OtsgSolution, ms: f64) -> Self {
        Self {
            id,
            iteration: s.shooting_iterations,
            converged: s.converged,
            duty_water_w: s.duty_water.get::<watt>(),
            duty_hot_w: s.duty_hot.get::<watt>(),
            closure: s.energy_closure.get::<ratio>(),
            hot_outlet_k: s.hot_outlet_temperature.get::<kelvin>(),
            hot_inlet_returned_k: s.hot_inlet_returned.get::<kelvin>(),
            dp_friction: s.pressure_drop_friction.get::<pascal>(),
            dp_gravity: s.pressure_drop_gravity.get::<pascal>(),
            dp_acceleration: s.pressure_drop_acceleration.get::<pascal>(),
            dryout_s: s.dryout_length.map_or(f64::NAN, |l| l.get::<meter>()),
            ms,
            profile: Profile::from_solution(s),
        }
    }
    /// Total water-side pressure drop \[Pa\].
    pub fn dp_total(&self) -> f64 {
        self.dp_friction + self.dp_gravity + self.dp_acceleration
    }
}

#[derive(Clone, Debug, PartialEq)]
pub enum Event {
    Progress(Snapshot),
    Error { id: u32, message: String },
}

/// The engine: at most one solve.
#[derive(Default)]
pub struct Engine {
    solve: Option<(u32, Shooting)>,
}

impl Engine {
    /// Run one march of the current solve and report it.
    fn march(&mut self, post: &mut impl FnMut(Event)) {
        let Some((id, sh)) = &mut self.solve else {
            return;
        };
        let id = *id;
        let t0 = dhoby_ghaut::web_demo::platform::now_s();
        match sh.step() {
            Ok(_) => {
                let ms = (dhoby_ghaut::web_demo::platform::now_s() - t0) * 1000.0;
                if let Some(s) = sh.solution() {
                    post(Event::Progress(Snapshot::from_solution(id, &s, ms)));
                }
            }
            Err(e) => {
                self.solve = None;
                post(Event::Error {
                    id,
                    message: e.to_string(),
                });
            }
        }
    }

    /// Serve one request.
    pub fn serve(&mut self, req: Request, post: &mut impl FnMut(Event)) {
        match req {
            Request::Solve { id, params } => match params.model() {
                Ok(m) => {
                    self.solve = Some((id, m.shooting()));
                    self.march(post);
                }
                Err(message) => {
                    self.solve = None;
                    post(Event::Error { id, message });
                }
            },
            Request::Step { id } => {
                if matches!(&self.solve, Some((sid, _)) if *sid == id) {
                    self.march(post);
                }
            }
        }
    }
}

/// Run `params` to convergence on a fresh engine, natively and
/// deterministically (the headless mode and the tests).
#[cfg_attr(target_arch = "wasm32", allow(dead_code))]
pub fn solve_to_convergence(params: Params) -> Result<Snapshot, String> {
    let mut e = Engine::default();
    let mut last: Option<Event> = None;
    e.serve(Request::Solve { id: 1, params }, &mut |ev| last = Some(ev));
    loop {
        match last.take() {
            Some(Event::Progress(s)) if s.converged => return Ok(s),
            Some(Event::Progress(_)) => e.serve(Request::Step { id: 1 }, &mut |ev| last = Some(ev)),
            Some(Event::Error { message, .. }) => return Err(message),
            None => return Err("the engine posted nothing".into()),
        }
    }
}

/// The headless trace: a CSV of the converged profile with a fixed header
/// and precision (deterministic: no clock, no randomness).
#[cfg_attr(target_arch = "wasm32", allow(dead_code))]
pub fn headless_csv(params: Params) -> Result<String, String> {
    let s = solve_to_convergence(params)?;
    let mut out = String::new();
    out.push_str(&format!(
        "# helical-coil OTSG (gh:#1157), {} cells: duty {:.3} kW, closure {:.2e}, hot out {:.4} K, dp {:.2} Pa, dryout s {:.5} m, marches {}\n",
        s.profile.len() - 1,
        s.duty_water_w / 1e3,
        s.closure,
        s.hot_outlet_k,
        s.dp_total(),
        s.dryout_s,
        s.iteration
    ));
    out.push_str("s_m,t_water_k,t_hot_k,t_wall_inner_k,quality_eq,pressure_pa,heat_flux_w_m2,regime\n");
    let p = &s.profile;
    for i in 0..p.len() {
        out.push_str(&format!(
            "{:.6},{:.6},{:.6},{:.6},{:.8},{:.3},{:.3},{}\n",
            p.s[i], p.t_water[i], p.t_hot[i], p.t_wall[i], p.quality[i], p.pressure[i], p.heat_flux[i], p.regime[i] as u8
        ));
    }
    Ok(out)
}

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
        Event::Error { id: 0, message }
    }
}

// ─── Wire format: numbers in one Float64Array, text in one string ────────────

#[cfg_attr(not(target_arch = "wasm32"), allow(dead_code))]
impl Request {
    pub fn to_wire(&self) -> Vec<f64> {
        match self {
            Request::Solve { id, params } => {
                let mut d = vec![0.0, f64::from(*id)];
                d.extend(params.to_wire());
                d
            }
            Request::Step { id } => vec![1.0, f64::from(*id)],
        }
    }
    pub fn from_wire(d: &[f64]) -> Result<Self, String> {
        match (d.first().copied(), d.get(1).copied()) {
            (Some(k), Some(id)) if k == 0.0 => Ok(Request::Solve {
                id: id as u32,
                params: Params::from_wire(&d[2..]).ok_or("solve request too short")?,
            }),
            (Some(k), Some(id)) if k == 1.0 => Ok(Request::Step { id: id as u32 }),
            _ => Err(format!("bad request ({} numbers)", d.len())),
        }
    }
}

/// Numbers in a snapshot before its profile.
const HEAD: usize = 15;

#[cfg_attr(not(target_arch = "wasm32"), allow(dead_code))]
impl Event {
    pub fn to_wire(&self) -> (Vec<f64>, String) {
        match self {
            Event::Progress(s) => {
                let n = s.profile.len();
                let mut d = vec![
                    0.0,
                    f64::from(s.id),
                    s.iteration as f64,
                    f64::from(u8::from(s.converged)),
                    s.duty_water_w,
                    s.duty_hot_w,
                    s.closure,
                    s.hot_outlet_k,
                    s.hot_inlet_returned_k,
                    s.dp_friction,
                    s.dp_gravity,
                    s.dp_acceleration,
                    s.dryout_s,
                    s.ms,
                    n as f64,
                ];
                for f in s.profile.fields() {
                    d.extend_from_slice(f);
                }
                (d, String::new())
            }
            Event::Error { id, message } => (vec![1.0, f64::from(*id)], message.clone()),
        }
    }
    pub fn from_wire(d: &[f64], text: &str) -> Result<Self, String> {
        match d.first().copied() {
            Some(k) if k == 0.0 && d.len() >= HEAD => {
                let n = d[14] as usize;
                if d.len() != HEAD + PROFILE_FIELDS * n {
                    return Err(format!("profile length mismatch ({} numbers for {n} nodes)", d.len()));
                }
                let col = |j: usize| d[HEAD + j * n..HEAD + (j + 1) * n].to_vec();
                Ok(Event::Progress(Snapshot {
                    id: d[1] as u32,
                    iteration: d[2] as usize,
                    converged: d[3] != 0.0,
                    duty_water_w: d[4],
                    duty_hot_w: d[5],
                    closure: d[6],
                    hot_outlet_k: d[7],
                    hot_inlet_returned_k: d[8],
                    dp_friction: d[9],
                    dp_gravity: d[10],
                    dp_acceleration: d[11],
                    dryout_s: d[12],
                    ms: d[13],
                    profile: Profile {
                        s: col(0),
                        t_water: col(1),
                        t_hot: col(2),
                        t_wall: col(3),
                        quality: col(4),
                        pressure: col(5),
                        heat_flux: col(6),
                        regime: col(7),
                        chf_ratio: col(8),
                    },
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

    fn small() -> Params {
        Params {
            nodes: 40.0,
            ..Params::default()
        }
    }

    #[test]
    fn params_round_trip_the_library_and_the_wire() {
        let p = Params::default();
        assert_eq!(p.terms, 0b111_1111, "every term on by default");
        assert_eq!(p.geometry(), HelicalCoilGeometry::illustrative());
        assert_eq!(Params::from_wire(&p.to_wire()), Some(p));
        let mut q = p;
        q.terms &= !(1 << 3);
        let m = q.model().expect("valid");
        assert!(!m.options().dryout && m.options().curvature);
        let bad = Params {
            pitch_m: 0.005,
            ..p
        };
        assert!(bad.model().is_err(), "pitch below the tube diameter");
    }

    #[test]
    fn requests_and_events_survive_the_wire() {
        for r in [
            Request::Solve {
                id: 7,
                params: small(),
            },
            Request::Step { id: 7 },
        ] {
            assert_eq!(Request::from_wire(&r.to_wire()), Ok(r));
        }
        assert!(Request::from_wire(&[5.0, 1.0]).is_err());
        let mut got = Vec::new();
        let mut e = Engine::default();
        e.serve(
            Request::Solve {
                id: 3,
                params: small(),
            },
            &mut |ev| got.push(ev),
        );
        let ev = got.pop().expect("an event");
        let (d, t) = ev.to_wire();
        // NaN (no CHF ratio at subcooled nodes) is not equal to itself, so
        // compare through the wire twice.
        let back = Event::from_wire(&d, &t).expect("decodes");
        assert_eq!(back.to_wire().0.len(), d.len());
        let (d2, _) = back.to_wire();
        assert!(d.iter().zip(&d2).all(|(a, b)| a == b || (a.is_nan() && b.is_nan())));
        let err = Event::Error {
            id: 2,
            message: "no".into(),
        };
        let (d, t) = err.to_wire();
        assert_eq!(Event::from_wire(&d, &t), Ok(err));
    }

    /// One march per request; a stale step is ignored; a bad input is an
    /// error event, not a panic.
    #[test]
    fn the_engine_runs_one_march_per_request() {
        let mut e = Engine::default();
        let mut got = Vec::new();
        e.serve(Request::Solve { id: 1, params: small() }, &mut |ev| got.push(ev));
        e.serve(Request::Step { id: 1 }, &mut |ev| got.push(ev));
        e.serve(Request::Step { id: 9 }, &mut |ev| got.push(ev));
        assert_eq!(got.len(), 2, "the stale step is ignored");
        match &got[1] {
            Event::Progress(s) => {
                assert_eq!((s.id, s.iteration), (1, 2));
                // March 2 is the too-hot end of the bracket (hot outlet =
                // hot inlet): it stops as soon as the hot side overshoots,
                // so its profile is cut short. The page draws only
                // converged results.
                assert!(!s.converged && s.profile.len() >= 1 && s.profile.len() <= 41);
            }
            Event::Error { message, .. } => panic!("{message}"),
        }
        let bad = Params {
            feed_temperature_k: 600.0,
            ..small()
        };
        e.serve(Request::Solve { id: 2, params: bad }, &mut |ev| got.push(ev));
        assert!(matches!(got.last(), Some(Event::Error { id: 2, .. })));
    }

    /// **The headless regression test** (the engine CLAUDE.md rule): the
    /// default case through the headless path, twice, byte-identical, with
    /// loose physical bounds (a harness check, not V&V) and the converged
    /// values the V&V record quotes for 200 cells pinned to 0.01 K / 1 Pa.
    #[test]
    fn headless_is_deterministic_and_bounded() {
        let a = headless_csv(Params::default()).expect("solves");
        let b = headless_csv(Params::default()).expect("solves");
        assert_eq!(a, b, "deterministic");
        let s = solve_to_convergence(Params::default()).expect("solves");
        let p = &s.profile;
        assert_eq!(p.len(), 201);
        assert!(s.closure.abs() < 1e-8);
        for i in 0..p.len() {
            assert!(p.t_water[i] > 300.0 && p.t_water[i] < 973.15);
            assert!(p.t_water[i] <= p.t_hot[i] + 1e-9);
            assert!(p.pressure[i] > 4.0e6 && p.pressure[i] <= 4.5e6);
        }
        // Pinned (V&V record §2.2, P4).
        let out = p.t_water[p.len() - 1];
        assert!((out - 827.5366).abs() < 0.01, "steam out {out}");
        assert!((s.dp_total() - 79010.07).abs() < 1.0, "dp {}", s.dp_total());
        assert!((s.dryout_s - 27.3795).abs() < 1e-3, "dryout {}", s.dryout_s);
        assert!(a.lines().count() == 203);
    }
}
