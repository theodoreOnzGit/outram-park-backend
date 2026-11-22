//! **The DEM pour, off the UI thread, for any pebble-bed k_eff simulator**
//! (gh:#1655): moved unchanged from `dhoby-ghaut`'s `examples/dem_web/
//! engine.rs` (the HTR-10 DEM page and the HTR-10 rung's pour both run it),
//! except that the vessel now comes from the CSG API: HTR-10's DEM vessel
//! as a CSG cell (`outram_mc_libs::pebble_beds::region_bed::
//! htr10_vessel_geometry`), read into the pour's vessel by
//! `region_bed::axisym_vessel` and poured by `Htr10Fill::in_vessel`. That
//! is `Htr10Fill::new`'s pour bit for bit (`tests` below, and
//! `outram-mc-libs`' `tests/region_bed_htr10_1656.rs`).
//!
//! A background thread natively, the Web Worker in the browser. Every
//! request is one short chunk of steps, so a new pour or a stop is served
//! within one chunk.
//!
//! **The physics is not here.** It is `outram_park_fork_liggghts::htr10_fill`
//! (the LIGGGHTS port's `GranularSystem`, the HTR-10 vessel with the
//! published conus as a mesh wall, the gh:#216 contact settings: µ = 0.1,
//! µ_r = 0, E = 5e8 Pa, ν = 0.2, e = 0.5, dt = 35 µs), unchanged. This file
//! only steps it in chunks and streams the centres.

use outram_park_fork_liggghts::compute::ThreadCount;
use outram_park_fork_liggghts::htr10_fill::{
    wall_radius_at, FillProgress, Htr10Fill, Htr10FillSettings, PEBBLE_RADIUS_M, VALVE_Z_M,
};
use uom::si::length::meter;
use uom::si::time::second;

/// What the UI asks for.
#[derive(Clone, Debug, PartialEq)]
pub enum Request {
    /// Seed a new pour of `n` pebbles (replacing any running one) and report
    /// it unstepped.
    Start { id: u32, n: usize, seed: u64 },
    /// Advance the pour `id` by `steps` and report.
    Step { id: u32, steps: usize },
}

/// One report of the pour.
#[derive(Clone, Debug, PartialEq)]
pub struct Snapshot {
    pub id: u32,
    pub n: usize,
    pub steps: usize,
    pub time_s: f64,
    /// Mean KE per core pebble over a one-radius drop (`htr10_fill`'s settle
    /// measure).
    pub ke_ratio: f64,
    /// Whole-core filling fraction (gh:#216's instrument).
    pub phi: f64,
    /// Bulk solid fraction of the filled vessel ([`vessel_bulk_fraction`]).
    pub phi_bulk: f64,
    pub surface_m: f64,
    pub n_core: usize,
    pub settled: bool,
    pub gave_up: bool,
    /// Wall-clock milliseconds per step over this chunk (0 for `Start`).
    pub ms_per_step: f64,
    /// Centres \[m\], `x, y, z` flattened.
    pub centres: Vec<f32>,
}

#[derive(Clone, Debug, PartialEq)]
pub enum Event {
    Progress(Snapshot),
    Error(String),
}

/// The engine: at most one pour.
#[derive(Default)]
pub struct Engine {
    pour: Option<(u32, Htr10Fill)>,
}

/// The settings of a browser pour: `htr10_fill`'s defaults (V&V § 4.9) with
/// `n` pebbles, one thread (the browser has one), and the seed.
pub fn settings(n: usize, seed: u64) -> Htr10FillSettings {
    Htr10FillSettings {
        n_pebbles: n,
        threads: ThreadCount::Fixed(1),
        seed,
        ..Htr10FillSettings::default()
    }
}

/// **Bulk solid fraction of the filled vessel** \[-\]: the solid volume
/// between `4 r` above the valve and `4 r` below the bed surface (99th
/// percentile of the centre heights, plus `r`), over the vessel volume
/// between them, `∫ π R(z)² dz` through the tube, the conus and the barrel.
/// The solid is the exact sphere-cap integral, so a pebble straddling a slab
/// face counts only its part inside. `NaN` until the bed is `8 r` deep.
///
/// Why a second measure. The whole-core φ (`htr10_fill::whole_core_fraction`,
/// gh:#216's like-for-like measure against the published 0.61) divides by the
/// core cylinder up to the surface, so on a shallow bed its rough top layer
/// and the conus inlet weigh heavily: a 4 000-pebble pour stands ~15 cm (2.5
/// diameters) above the floor. Excluding `4 r` at each end is the bulk-slab
/// instrument of `outram-park-fork-liggghts`'
/// `examples/htr10_recirculation_sweep.rs` (`bulk_solid_fraction`, ported
/// here, its logic unchanged), widened from the core cylinder to the whole
/// vessel so a reduced pour has a slab at all. It includes the wall-effect
/// layers of the tube and conus, which a full-size core slab does not.
pub fn vessel_bulk_fraction(centres: &[[f64; 3]]) -> f64 {
    // The calculator is outram-mc-libs' (2026-11-18, maintainer: "the
    // packing fraction calculator should also go to OUTRAM MC libs"),
    // operation for operation the copy that stood here; this vessel's
    // bottom (the valve) and wall, on 1 mm slices.
    outram_mc_libs::pebble_beds::packing_fraction::bulk_fraction(centres, PEBBLE_RADIUS_M, VALVE_Z_M, wall_radius_at, 1e-3)
}

/// HTR-10's DEM vessel from the CSG API (module docs).
///
/// # Errors
/// The region reader's refusal, as text (it does not happen for this
/// geometry: pinned by the tests).
pub fn htr10_vessel() -> Result<outram_park_fork_liggghts::axisym_vessel::AxisymVessel, String> {
    use outram_mc_libs::pebble_beds::region_bed::{axisym_vessel, htr10_vessel_geometry, LengthUnit};
    axisym_vessel(&htr10_vessel_geometry(LengthUnit::Metre), 0, LengthUnit::Metre).map_err(|e| format!("{e:?}"))
}

fn snapshot(id: u32, fill: &Htr10Fill, p: FillProgress, ms_per_step: f64) -> Snapshot {
    let raw = fill.centres();
    let c64: Vec<[f64; 3]> = raw.iter().map(|c| [c.x, c.y, c.z]).collect();
    let centres = raw
        .iter()
        .flat_map(|c| [c.x as f32, c.y as f32, c.z as f32])
        .collect();
    Snapshot {
        id,
        n: fill.settings().n_pebbles,
        steps: p.steps,
        time_s: p.time.get::<second>(),
        ke_ratio: p.ke_ratio_core,
        phi: p.phi_whole_core,
        phi_bulk: vessel_bulk_fraction(&c64),
        surface_m: p.surface_height.get::<meter>(),
        n_core: p.n_in_core,
        settled: p.settled,
        gave_up: p.gave_up,
        ms_per_step,
        centres,
    }
}

impl Engine {
    /// Serve one request.
    pub fn serve(&mut self, req: Request, post: &mut impl FnMut(Event)) {
        match req {
            Request::Start { id, n, seed } => match htr10_vessel().and_then(|v| Htr10Fill::in_vessel(settings(n, seed), &v).map_err(|e| format!("{e:?}"))) {
                Ok(fill) => {
                    let p = fill.progress();
                    post(Event::Progress(snapshot(id, &fill, p, 0.0)));
                    self.pour = Some((id, fill));
                }
                Err(e) => post(Event::Error(format!("the pour was refused: {e}"))),
            },
            Request::Step { id, steps } => match &mut self.pour {
                Some((pid, fill)) if *pid == id => {
                    let t0 = bugis::web_demo::platform::now_s();
                    let p = fill.advance(steps.max(1));
                    let ms = (bugis::web_demo::platform::now_s() - t0) * 1000.0
                        / steps.max(1) as f64;
                    post(Event::Progress(snapshot(id, fill, p, ms)));
                }
                // A chunk asked for by a pour that has since been replaced.
                _ => {}
            },
        }
    }
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
        Event::Error(message)
    }
}

// ─── Wire format: numbers in one Float64Array, text in one string ────────────
//
// Natively the enums cross the thread boundary as they are; in the browser as
// `{kind: "msg", data: Float64Array, text}`. Plain Rust, so the native tests
// check the round trip the worker relies on.

#[cfg_attr(not(target_arch = "wasm32"), allow(dead_code))]
impl Request {
    pub fn to_wire(&self) -> Vec<f64> {
        match *self {
            Request::Start { id, n, seed } => vec![0.0, f64::from(id), n as f64, seed as f64],
            Request::Step { id, steps } => vec![1.0, f64::from(id), steps as f64],
        }
    }
    pub fn from_wire(d: &[f64]) -> Result<Self, String> {
        let g = |i: usize| {
            d.get(i)
                .copied()
                .ok_or_else(|| format!("request too short ({} numbers)", d.len()))
        };
        match g(0)? as u32 {
            0 => Ok(Request::Start {
                id: g(1)? as u32,
                n: g(2)? as usize,
                seed: g(3)? as u64,
            }),
            1 => Ok(Request::Step {
                id: g(1)? as u32,
                steps: g(2)? as usize,
            }),
            k => Err(format!("unknown request {k}")),
        }
    }
}

/// Numbers in a snapshot before its centres.
const HEAD: usize = 13;

#[cfg_attr(not(target_arch = "wasm32"), allow(dead_code))]
impl Event {
    pub fn to_wire(&self) -> (Vec<f64>, String) {
        match self {
            Event::Progress(s) => {
                let mut d = vec![
                    0.0,
                    f64::from(s.id),
                    s.n as f64,
                    s.steps as f64,
                    s.time_s,
                    s.ke_ratio,
                    s.phi,
                    s.phi_bulk,
                    s.surface_m,
                    s.n_core as f64,
                    f64::from(u8::from(s.settled)),
                    f64::from(u8::from(s.gave_up)),
                    s.ms_per_step,
                ];
                d.extend(s.centres.iter().map(|&c| f64::from(c)));
                (d, String::new())
            }
            Event::Error(m) => (vec![1.0], m.clone()),
        }
    }
    pub fn from_wire(d: &[f64], text: &str) -> Result<Self, String> {
        match d.first().copied() {
            Some(k) if k == 0.0 && d.len() >= HEAD => Ok(Event::Progress(Snapshot {
                id: d[1] as u32,
                n: d[2] as usize,
                steps: d[3] as usize,
                time_s: d[4],
                ke_ratio: d[5],
                phi: d[6],
                phi_bulk: d[7],
                surface_m: d[8],
                n_core: d[9] as usize,
                settled: d[10] != 0.0,
                gave_up: d[11] != 0.0,
                ms_per_step: d[12],
                centres: d[HEAD..].iter().map(|&c| c as f32).collect(),
            })),
            Some(k) if k == 1.0 => Ok(Event::Error(text.to_string())),
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

    #[test]
    fn requests_and_events_survive_the_wire() {
        for r in [
            Request::Start {
                id: 3,
                n: 4000,
                seed: 0x5EED_0010,
            },
            Request::Step { id: 3, steps: 40 },
        ] {
            assert_eq!(Request::from_wire(&r.to_wire()), Ok(r));
        }
        let s = Snapshot {
            id: 2,
            n: 2,
            steps: 80,
            time_s: 0.0028,
            ke_ratio: 1.5e-3,
            phi: 0.55,
            phi_bulk: 0.58,
            surface_m: 0.4,
            n_core: 1,
            settled: false,
            gave_up: false,
            ms_per_step: 1.25,
            centres: vec![0.1, -0.2, 0.3, 0.0, 0.0, -0.5],
        };
        let (d, t) = Event::Progress(s.clone()).to_wire();
        assert_eq!(Event::from_wire(&d, &t), Ok(Event::Progress(s)));
        let (d, t) = Event::Error("no".into()).to_wire();
        assert_eq!(Event::from_wire(&d, &t), Ok(Event::Error("no".into())));
        assert!(Request::from_wire(&[7.0]).is_err());
    }

    /// The engine pours, steps the pour it was asked for, and ignores a chunk
    /// asked for by a replaced pour. A 120-pebble pour, 200 steps: a plumbing
    /// test, not a packing result.
    #[test]
    fn the_engine_steps_the_current_pour_only() {
        let mut e = Engine::default();
        let mut got = Vec::new();
        e.serve(
            Request::Start {
                id: 1,
                n: 120,
                seed: 7,
            },
            &mut |ev| got.push(ev),
        );
        e.serve(Request::Step { id: 1, steps: 200 }, &mut |ev| got.push(ev));
        e.serve(Request::Step { id: 9, steps: 200 }, &mut |ev| got.push(ev));
        assert_eq!(got.len(), 2, "the stale request is ignored");
        match &got[1] {
            Event::Progress(s) => {
                assert_eq!((s.id, s.n, s.steps), (1, 120, 200));
                assert_eq!(s.centres.len(), 360);
                assert!(s.ms_per_step > 0.0);
            }
            Event::Error(m) => panic!("{m}"),
        }
    }

    /// The vessel from the CSG API is `AxisymVessel::htr10()`, and the
    /// engine's pour, with its g(r) diagnostics, is `Htr10Fill::new`'s to
    /// the bit (gh:#1655 done-when: "the g(r) of a pour matches today's to
    /// the bit for the same seed"). 600 pebbles, 1500 steps.
    #[test]
    fn the_engine_pours_htr10s_bed_and_g_of_r_to_the_bit() {
        assert_eq!(htr10_vessel(), Ok(outram_park_fork_liggghts::axisym_vessel::AxisymVessel::htr10()));
        let (n, seed, steps) = (600, 0x5EED_0010, 1500);
        let mut e = Engine::default();
        let mut got = Vec::new();
        e.serve(Request::Start { id: 1, n, seed }, &mut |ev| got.push(ev));
        e.serve(Request::Step { id: 1, steps }, &mut |ev| got.push(ev));
        let Some(Event::Progress(s)) = got.pop() else { panic!("no snapshot") };
        let mut fill = Htr10Fill::new(settings(n, seed)).expect("fill");
        fill.advance(steps);
        let reference: Vec<f32> = fill.centres().iter().flat_map(|c| [c.x as f32, c.y as f32, c.z as f32]).collect();
        assert_eq!(s.centres.iter().map(|c| c.to_bits()).collect::<Vec<_>>(), reference.iter().map(|c| c.to_bits()).collect::<Vec<_>>());
        let a = crate::dem_pour::diagnostics(&s.centres, s.phi_bulk, 1.0).to_words();
        let b = crate::dem_pour::diagnostics(&reference, s.phi_bulk, 1.0).to_words();
        assert_eq!(a.iter().map(|w| w.to_bits()).collect::<Vec<_>>(), b.iter().map(|w| w.to_bits()).collect::<Vec<_>>());
    }

    /// A lattice of touching spheres in the tube, simple cubic, fills π/6 of
    /// the space its cells span; the slab measure must find that, and give
    /// `NaN` for a bed too shallow to have a slab.
    #[test]
    fn the_bulk_fraction_of_a_known_packing() {
        let (r, d) = (PEBBLE_RADIUS_M, 2.0 * PEBBLE_RADIUS_M);
        // Columns inside the tube, six layers from the valve up (the slab is
        // then layers 2-3, all below the conus): the cells tile the square
        // |x|, |y| < 0.15, so compare against the tube's area.
        let mut c = Vec::new();
        for k in 0..6 {
            for i in -2..3 {
                for j in -2..3 {
                    c.push([
                        f64::from(i) * d,
                        f64::from(j) * d,
                        VALVE_Z_M + r + f64::from(k) * d,
                    ]);
                }
            }
        }
        let phi = vessel_bulk_fraction(&c);
        let square = (5.0 * d) * (5.0 * d);
        let tube = std::f64::consts::PI * 0.25 * 0.25;
        let expect = std::f64::consts::PI / 6.0 * square / tube;
        assert!(
            (phi - expect).abs() < 0.01 * expect,
            "{phi} against {expect}"
        );
        assert!(
            vessel_bulk_fraction(&c[..25]).is_nan(),
            "one layer has no slab"
        );
    }
}
