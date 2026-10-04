//! The physics side of the nuclear data demo, kept OFF the UI thread.
//!
//! Every calculation here (RECONR, BROADR, PURR, THERMR, the group collapse)
//! is a call of up to tens of seconds into `njoy-outram-park-fork`, unchanged.
//! The UI never makes one: the engine runs on a thread natively and in a Web
//! Worker in the browser, through `dhoby_ghaut::web_demo::link` (the shared
//! no-lag plumbing, gh:#521), and only finished curves cross back.
//!
//! **Streaming, stop and restart.** Long work is split into steps the UI asks
//! for one at a time: RECONR runs once per tolerance in a coarse-to-fine
//! sequence, and each finished curve is drawn as it arrives, so σ(E) refines
//! on screen. Stop means the UI asks for no further step; a slider moved while
//! a step runs is coalesced into one request sent when that step's result
//! arrives. A single step cannot be interrupted, which is why each is one
//! call.
//!
//! No physics lives here: each handler calls the crate's public API and
//! repackages the numbers for drawing.

#![cfg_attr(not(target_arch = "wasm32"), allow(dead_code))]

use crate::rungs::{Rung, GRAPHITE, H_IN_H2O, U238};
use njoy_outram_park_fork::endf::tape::Tape;
use njoy_outram_park_fork::reconr::{reconr, ReconrConfig, ReconrResult, ReconrSection};
use std::sync::Arc;

/// RECONR tolerance the BROADR and GROUPR rungs build on. NJOY's default is
/// 0.001; the demo uses 0.01 so a phone gets there in reasonable time. A
/// deliberate liberty, stated in the panel.
pub const BASE_TOL: f64 = 0.01;

/// The RECONR rung's tolerance sequence, coarse to fine; the last is NJOY's
/// default and the one the lessons' records use.
pub const RECONR_TOLS: [f64; 6] = [0.3, 0.1, 0.03, 0.01, 0.003, 0.001];

/// BROADR window: what is broadened (with margin) and what is shown, eV.
pub const BROADR_IN: (f64, f64) = (0.5, 600.0);
pub const BROADR_SHOWN: (f64, f64) = (1.0, 300.0);

/// GROUPR rung energy range, eV.
pub const GROUPR_RANGE: (f64, f64) = (1.0, 1.0e4);

pub enum Request {
    /// Download/parse the rung's tapes (tapes already held are kept).
    Load { id: u32, rung: Rung },
    /// The tape's section list and resonance-range flags (rung `endf`).
    Sections { job: u32 },
    /// RECONR at one tolerance (rung `reconr`).
    Reconr { job: u32, tol: f64 },
    /// BROADR of the base reconstruction to one temperature (rung `broadr`).
    Broadr { job: u32, temp_k: f64 },
    /// PURR probability tables at one temperature (rung `purr`).
    Purr { job: u32, temp_k: f64, nladr: usize, nsamp: usize },
    /// THERMR cross sections and an emission spectrum (rung `thermr`).
    Thermr { job: u32, material: u8, temp_k: f64, e_emit: f64 },
    /// GROUPR-style group averages, infinitely dilute and at `sigma0`.
    Groupr { job: u32, ngroups: usize, sigma0: f64 },
}

/// One drawn series: `tag` says what it is (see each handler), `pts` are
/// `(x, y)`, `w` an optional per-point weight (a probability, or a packed
/// MF*1000+MT label for the section list).
#[derive(Clone, Debug, Default)]
pub struct Curve {
    pub tag: u32,
    pub pts: Vec<[f64; 2]>,
    pub w: Vec<f64>,
}

pub enum Event {
    JobStarted { id: u32, index: usize },
    JobDone { id: u32, index: usize, secs: f64 },
    Ready { id: u32 },
    /// A finished step: its curves, how long it took, and a one-line result.
    Result { job: u32, param: f64, secs: f64, n_points: usize, curves: Vec<Curve>, note: String },
    Error(String),
}

/// What the engine holds: parsed tapes, and the cached base reconstruction.
#[derive(Default)]
pub struct NdEngine {
    tapes: Vec<(String, Tape, i32)>,
    base: Option<ReconrResult>,
}

fn clip(p: &[(f64, f64)], lo: f64, hi: f64) -> Vec<(f64, f64)> {
    p.iter().copied().filter(|&(e, _)| e >= lo && e <= hi).collect()
}

fn curve(tag: u32, p: &[(f64, f64)]) -> Curve {
    Curve { tag, pts: p.iter().map(|&(x, y)| [x, y]).collect(), w: Vec::new() }
}

impl NdEngine {
    pub fn has(&self, name: &str) -> bool {
        self.tapes.iter().any(|(n, _, _)| n == name)
    }
    fn tape(&self, name: &str) -> Result<(&Tape, i32), String> {
        self.tapes.iter().find(|(n, _, _)| n == name).map(|(_, t, m)| (t, *m)).ok_or_else(|| format!("{name} is not loaded"))
    }
    pub fn add_tape(&mut self, name: &str, bytes: &[u8]) -> Result<(), String> {
        let tape = Tape::read(std::io::Cursor::new(bytes)).map_err(|e| format!("{name}: {e}"))?;
        let mat = *tape.materials().first().ok_or_else(|| format!("{name}: no material on tape"))?;
        self.tapes.push((name.to_string(), tape, mat));
        Ok(())
    }
    fn base(&mut self) -> Result<ReconrResult, String> {
        if self.base.is_none() {
            let (tape, mat) = self.tape(U238)?;
            let r = reconr(tape, &ReconrConfig { mat, tolerance: BASE_TOL, temperature: 0.0 }).map_err(|e| e.to_string())?;
            self.base = Some(r);
        }
        Ok(self.base.clone().expect("just built"))
    }

    /// Serve one step; `now` is a clock in seconds (it differs natively and
    /// in wasm). Every outcome is posted, errors included.
    pub fn serve(&mut self, r: Request, now: impl Fn() -> f64, post: &mut impl FnMut(Event)) {
        let t0 = now();
        let out = match r {
            Request::Load { .. } => return,
            Request::Sections { job } => self.sections(job),
            Request::Reconr { job, tol } => self.reconr(job, tol),
            Request::Broadr { job, temp_k } => self.broadr(job, temp_k),
            Request::Purr { job, temp_k, nladr, nsamp } => self.purr(job, temp_k, nladr, nsamp),
            Request::Thermr { job, material, temp_k, e_emit } => self.thermr(job, material, temp_k, e_emit),
            Request::Groupr { job, ngroups, sigma0 } => self.groupr(job, ngroups, sigma0),
        };
        match out {
            Ok(Event::Result { job, param, n_points, curves, note, .. }) => {
                post(Event::Result { job, param, secs: now() - t0, n_points, curves, note })
            }
            Ok(e) => post(e),
            Err(e) => post(Event::Error(e)),
        }
    }

    // ─── rung: endf ──────────────────────────────────────────────────────────

    /// Tag 0: one point per section, `(index, rows)`, with `MF*1000+MT` in
    /// `w`. The note lists the resonance ranges and their flags.
    fn sections(&self, job: u32) -> Result<Event, String> {
        let (tape, mat) = self.tape(U238)?;
        let mut c = Curve::default();
        for (i, s) in tape.sections().iter().enumerate() {
            c.pts.push([i as f64, s.rows.len() as f64]);
            c.w.push((s.key.mf * 1000 + s.key.mt) as f64);
        }
        let mut note = format!("ENDF/B-VIII.0 U-238, MAT {mat}: {} sections.", tape.sections().len());
        if let Some(sec) = tape.section(mat, 2, 151) {
            if let Ok(info) = njoy_outram_park_fork::reconr::mf2::parse_resonance_info(sec) {
                for r in &info.ranges {
                    let f = match r.formalism {
                        Some(f) => format!("resolved, {f:?}"),
                        None if r.lru == 2 => "unresolved (average parameters)".into(),
                        None => "no resonance parameters".into(),
                    };
                    note.push_str(&format!(" MF=2 range {:.4e}-{:.4e} eV: LRU={}, {f}.", r.el, r.eh, r.lru));
                }
            }
            if sec.rows.len() > 1 {
                if let Ok(u) = njoy_outram_park_fork::unresr::mf2::parse_lru2_ranges(&sec.rows[1..]) {
                    for r in &u {
                        note.push_str(&format!(" Unresolved range: LSSF={}.", r.lssf));
                    }
                }
            }
        }
        Ok(Event::Result { job, param: 0.0, secs: 0.0, n_points: c.pts.len(), curves: vec![c], note })
    }

    // ─── rung: reconr ────────────────────────────────────────────────────────

    /// Tags 1, 2, 102: MT=1, 2, 102 of a full RECONR at `tol`, 0 K.
    fn reconr(&self, job: u32, tol: f64) -> Result<Event, String> {
        let (tape, mat) = self.tape(U238)?;
        let r = reconr(tape, &ReconrConfig { mat, tolerance: tol, temperature: 0.0 }).map_err(|e| e.to_string())?;
        let mut curves = Vec::new();
        let mut n_points = 0;
        for mt in [1, 2, 102] {
            if let Some(s) = r.sections.iter().find(|s| i32::from(s.mt) == mt) {
                if mt == 1 {
                    n_points = s.pairs.len();
                }
                curves.push(curve(mt as u32, &s.pairs));
            }
        }
        let note = format!("RECONR at tolerance {tol}: {n_points} points on the union grid (MT=1), 0 K.");
        Ok(Event::Result { job, param: tol, secs: 0.0, n_points, curves, note })
    }

    // ─── rung: broadr ────────────────────────────────────────────────────────

    /// Tags 1 / 102: broadened MT=1 / MT=102; 1001 / 1102: the 0 K ones; all
    /// in [`BROADR_SHOWN`]. The note carries the area check (the sum rule).
    fn broadr(&mut self, job: u32, temp_k: f64) -> Result<Event, String> {
        let base = self.base()?;
        let awr = base.material.awr;
        let mut window: Vec<ReconrSection> = Vec::new();
        for mt in [1, 102] {
            if let Some(s) = base.sections.iter().find(|s| i32::from(s.mt) == mt) {
                let mut w = s.clone();
                w.pairs = clip(&s.pairs, BROADR_IN.0, BROADR_IN.1);
                window.push(w);
            }
        }
        let broadened = if temp_k > 0.0 {
            njoy_outram_park_fork::broadr::doppler_broaden_below(&window, awr, temp_k, BROADR_IN.1)
        } else {
            window.clone()
        };
        let mut curves = Vec::new();
        let mut n_points = 0;
        for (set, off) in [(&window, 1000u32), (&broadened, 0u32)] {
            for s in set.iter() {
                let mt = i32::from(s.mt) as u32;
                let p = clip(&s.pairs, BROADR_SHOWN.0, BROADR_SHOWN.1);
                if off == 0 && mt == 1 {
                    n_points = p.len();
                }
                curves.push(curve(off + mt, &p));
            }
        }
        // The sum rule, live: the area under capture, 5-200 eV (lin-lin, so
        // the trapezoid is exact on each table).
        let area = |set: &[ReconrSection]| -> f64 {
            set.iter().find(|s| i32::from(s.mt) == 102).map_or(0.0, |s| {
                let p = clip(&s.pairs, 5.0, 200.0);
                p.windows(2).map(|w| 0.5 * (w[0].1 + w[1].1) * (w[1].0 - w[0].0)).sum()
            })
        };
        let (a0, a1) = (area(&window), area(&broadened));
        let mut note = format!("BROADR (per-reaction SIGMA1 kernel) to {temp_k:.0} K.");
        if a0 > 0.0 {
            note.push_str(&format!(
                " Area under capture, 5-200 eV: {a0:.5e} b·eV at 0 K, {a1:.5e} at {temp_k:.0} K ({:+.3} %).",
                100.0 * (a1 / a0 - 1.0)
            ));
        }
        Ok(Event::Result { job, param: temp_k, secs: 0.0, n_points, curves, note })
    }

    // ─── rung: purr ──────────────────────────────────────────────────────────

    /// Tag 1: one point per band, `(E, total factor or barns)`, weight = the
    /// band's probability. The note gives the probability-weighted mean.
    fn purr(&self, job: u32, temp_k: f64, nladr: usize, nsamp: usize) -> Result<Event, String> {
        use njoy_outram_park_fork::purr::{UrrProbabilityTables, UrrSample};
        let (tape, mat) = self.tape(U238)?;
        let t = UrrProbabilityTables::from_endf(tape, mat, temp_k, 20, nladr, nsamp)
            .map_err(|e| e.to_string())?
            .ok_or("this evaluation has no unresolved range")?;
        let mut c = Curve { tag: 1, ..Default::default() };
        let mut worst_mean: f64 = 0.0;
        let mut factors = true;
        for &e in t.energies() {
            // `covers` is strict at both ends; nudge the end points inward.
            let e_in = e.clamp(t.e_low * (1.0 + 1e-9), t.e_high * (1.0 - 1e-9));
            // The probability of each band value: the share of a fine,
            // uniform grid of xi on which `sample` returns it (`sample` is
            // piecewise constant in xi, so this converges to the band widths;
            // 4000 points resolve a 1/20 band to 0.05 %).
            const N: usize = 4000;
            let mut bands: Vec<(f64, usize)> = Vec::new();
            for k in 0..N {
                let xi = (k as f64 + 0.5) / N as f64;
                let v = match t.sample(e_in, xi) {
                    Some(UrrSample::SelfShieldingFactors(f)) => f[0],
                    Some(UrrSample::CrossSections(x)) => {
                        factors = false;
                        x[0]
                    }
                    None => continue,
                };
                match bands.iter_mut().find(|(b, _)| *b == v) {
                    Some((_, n)) => *n += 1,
                    None => bands.push((v, 1)),
                }
            }
            let mut mean = 0.0;
            for (v, n) in bands {
                let p = n as f64 / N as f64;
                c.pts.push([e, v]);
                c.w.push(p);
                mean += p * v;
            }
            if factors {
                worst_mean = worst_mean.max((mean - 1.0).abs());
            }
        }
        let note = if factors {
            format!(
                "LSSF={}: total self-shielding factors, {} energies x {} bands at {temp_k:.0} K, {nladr} ladders x \
                 {nsamp} samples. Probability-weighted mean of the factor: within {worst_mean:.2e} of 1 at every energy.",
                t.lssf,
                t.len(),
                t.n_bands()
            )
        } else {
            format!("LSSF={}: total cross sections in barns, {} energies x {} bands.", t.lssf, t.len(), t.n_bands())
        };
        Ok(Event::Result { job, param: temp_k, secs: 0.0, n_points: t.len(), curves: vec![c], note })
    }

    // ─── rung: thermr ────────────────────────────────────────────────────────

    /// Tags: 1 incoherent inelastic σ(E); 2 coherent elastic (graphite); 3
    /// the free-atom σ (a constant); 10 the emission spectrum at `e_emit`,
    /// one point `(E', 1/n)` per equiprobable bin.
    fn thermr(&self, job: u32, material: u8, temp_k: f64, e_emit: f64) -> Result<Event, String> {
        use njoy_outram_park_fork::thermr::scattering::{CoherentElasticScattering, IncoherentInelasticScattering};
        use njoy_outram_park_fork::units::{NeutronEnergy, Temperature};
        use uom::si::area::barn;
        use uom::si::energy::electronvolt;
        use uom::si::thermodynamic_temperature::kelvin;
        let name = if material == 0 { GRAPHITE } else { H_IN_H2O };
        let (tape, mat) = self.tape(name)?;
        let t = Temperature::new::<kelvin>(temp_k);
        let inel = IncoherentInelasticScattering::from_tape(tape, mat, t).map_err(|e| e.to_string())?;
        let emax = inel.energy_max_ev();
        let n = 160;
        let grid: Vec<f64> = (0..n).map(|i| 1.0e-5 * (emax / 1.0e-5).powf(i as f64 / (n - 1) as f64)).collect();
        let ev = |e: f64| NeutronEnergy::new::<electronvolt>(e);
        let mut curves = vec![Curve { tag: 1, pts: grid.iter().map(|&e| [e, inel.inelastic_xs(ev(e)).get::<barn>()]).collect(), w: Vec::new() }];
        let free = inel.free_cross_section().get::<barn>();
        curves.push(Curve { tag: 3, pts: vec![[1.0e-5, free], [emax, free]], w: Vec::new() });
        if material == 0 {
            if let Ok(coh) = CoherentElasticScattering::from_tape(tape, mat, t) {
                let mut es = grid.clone();
                for (edge, _) in coh.bragg_edge_table() {
                    let e = edge.get::<electronvolt>();
                    if e < emax {
                        es.push(e * (1.0 - 1e-7));
                        es.push(e * (1.0 + 1e-7));
                    }
                }
                es.sort_by(|a, b| a.total_cmp(b));
                curves.push(Curve { tag: 2, pts: es.iter().map(|&e| [e, coh.cross_section(ev(e)).get::<barn>()]).collect(), w: Vec::new() });
            }
        }
        let bins = inel.emission(ev(e_emit), 32, 8);
        let nb = bins.len().max(1) as f64;
        curves.push(Curve { tag: 10, pts: bins.iter().map(|b| [b.outgoing_energy.get::<electronvolt>(), 1.0 / nb]).collect(), w: Vec::new() });
        let note = format!(
            "{}: T asked {temp_k:.1} K, used {:.1} K; S(alpha,beta) up to {emax:.3} eV; free-atom sigma {free:.3} b per \
             principal atom; emission from {e_emit:.4} eV in {} equiprobable bins.",
            if material == 0 { "Crystalline graphite" } else { "H in H2O" },
            inel.selected_temperature().get::<kelvin>(),
            bins.len()
        );
        Ok(Event::Result { job, param: temp_k, secs: 0.0, n_points: n, curves, note })
    }

    // ─── rung: groupr ────────────────────────────────────────────────────────

    /// Tags: 102 the pointwise capture in range; 1 the infinitely dilute
    /// group steps; 2 the steps at `sigma0` (two points per step).
    fn groupr(&mut self, job: u32, ngroups: usize, sigma0: f64) -> Result<Event, String> {
        use njoy_outram_park_fork::groupr::panel::{group_average_vector, GroupFlux, PointwiseXs};
        use njoy_outram_park_fork::groupr::unresolved::bondarenko_flux_value;
        let base = self.base()?;
        let get = |mt: i32| base.sections.iter().find(|s| i32::from(s.mt) == mt).map(|s| s.pairs.clone()).unwrap_or_default();
        let (lo, hi) = GROUPR_RANGE;
        let total = clip(&get(1), lo, hi);
        let capture = clip(&get(102), lo, hi);
        let ng = ngroups.max(1);
        let bounds: Vec<f64> = (0..=ng).map(|g| lo * (hi / lo).powf(g as f64 / ng as f64)).collect();
        let sigma = PointwiseXs::LinLin(Arc::new(capture.clone()));
        // 1/E times the Bondarenko narrow-resonance factor, tabulated on the
        // total cross section's grid (GROUPR's `genflx` shape). sigma_pot only
        // normalises each dilution and cancels in the average, so 0 is used.
        let flux_at = |s0: f64| -> GroupFlux {
            GroupFlux::Tabulated(Arc::new(total.iter().map(|&(e, st)| (e, bondarenko_flux_value(st, 1.0 / e, 0.0, s0))).collect()))
        };
        let dilute = group_average_vector(&sigma, &flux_at(1.0e10), &bounds);
        let shielded = group_average_vector(&sigma, &flux_at(sigma0), &bounds);
        let steps = |v: &[f64]| -> Vec<(f64, f64)> { v.iter().enumerate().flat_map(|(g, &s)| [(bounds[g], s), (bounds[g + 1], s)]).collect() };
        let ri = |v: &[f64]| -> f64 { v.iter().enumerate().map(|(g, &s)| s * (bounds[g + 1] / bounds[g]).ln()).sum() };
        let note = format!(
            "{ng} groups, 1 eV-10 keV, weight (1/E)(sigma0)/(sigma_t + sigma0), base RECONR tolerance {BASE_TOL}, 0 K. Capture \
             resonance integral over the range: {:.2} b infinitely dilute, {:.2} b at sigma0 = {sigma0:.0} b.",
            ri(&dilute),
            ri(&shielded)
        );
        let curves = vec![curve(102, &capture), curve(1, &steps(&dilute)), curve(2, &steps(&shielded))];
        Ok(Event::Result { job, param: sigma0, secs: 0.0, n_points: ng, curves, note })
    }
}

// ─── Native: a thread ────────────────────────────────────────────────────────

/// A reference tape's bytes, covariances stripped: the same bytes the
/// browser gets after inflating its download.
#[cfg(not(target_arch = "wasm32"))]
pub fn native_tape(tape: &str) -> Result<Vec<u8>, String> {
    let path = njoy_outram_park_fork::reference_data::reference_endf(tape).ok_or_else(|| format!("reference tape {tape} is not present"))?;
    let raw = std::fs::read(&path).map_err(|e| format!("{}: {e}", path.display()))?;
    Ok(crate::tapes::strip_covariances(&raw))
}

#[cfg(not(target_arch = "wasm32"))]
impl dhoby_ghaut::web_demo::link::NativeEngine for NdEngine {
    type Req = Request;
    type Ev = Event;
    fn handle(&mut self, req: Request, post: &mut impl FnMut(Event)) {
        match req {
            Request::Load { id, rung } => {
                for (index, tape) in rung.info().tapes.iter().enumerate() {
                    post(Event::JobStarted { id, index });
                    let t = std::time::Instant::now();
                    if !self.has(tape) {
                        if let Err(e) = native_tape(tape).and_then(|b| self.add_tape(tape, &b)) {
                            post(Event::Error(e));
                            return;
                        }
                    }
                    post(Event::JobDone { id, index, secs: t.elapsed().as_secs_f64() });
                }
                post(Event::Ready { id });
            }
            r => self.serve(r, dhoby_ghaut::web_demo::platform::now_s, post),
        }
    }
}

// ─── Browser: a Web Worker ───────────────────────────────────────────────────

#[cfg(target_arch = "wasm32")]
mod web {
    use super::*;
    use dhoby_ghaut::web_demo::link::{fetch_bytes, js, Message, Poster, WorkerEngine};
    use std::sync::RwLock;
    use wasm_bindgen::JsValue;

    fn rung_code(r: Rung) -> f64 {
        crate::rungs::RUNGS.iter().position(|x| x.rung == r).unwrap_or(0) as f64
    }
    fn rung_from(c: f64) -> Rung {
        crate::rungs::RUNGS.get(c as usize).map_or(Rung::Reconr, |x| x.rung)
    }

    /// Requests travel as `{kind, a: [numbers]}`.
    impl Message for Request {
        fn to_js(&self) -> JsValue {
            let o = js::object();
            let (kind, a): (&str, Vec<f64>) = match self {
                Request::Load { id, rung } => ("load", vec![*id as f64, rung_code(*rung)]),
                Request::Sections { job } => ("sections", vec![*job as f64]),
                Request::Reconr { job, tol } => ("reconr", vec![*job as f64, *tol]),
                Request::Broadr { job, temp_k } => ("broadr", vec![*job as f64, *temp_k]),
                Request::Purr { job, temp_k, nladr, nsamp } => ("purr", vec![*job as f64, *temp_k, *nladr as f64, *nsamp as f64]),
                Request::Thermr { job, material, temp_k, e_emit } => ("thermr", vec![*job as f64, *material as f64, *temp_k, *e_emit]),
                Request::Groupr { job, ngroups, sigma0 } => ("groupr", vec![*job as f64, *ngroups as f64, *sigma0]),
            };
            js::set(&o, "kind", kind);
            js::set(&o, "a", js::f64s(&a));
            o.into()
        }
        fn from_js(v: &JsValue) -> Result<Self, String> {
            let a = js::get_f64s(v, "a");
            let n = |i: usize| a.get(i).copied().unwrap_or(0.0);
            Ok(match js::get_str(v, "kind").as_str() {
                "load" => Request::Load { id: n(0) as u32, rung: rung_from(n(1)) },
                "sections" => Request::Sections { job: n(0) as u32 },
                "reconr" => Request::Reconr { job: n(0) as u32, tol: n(1) },
                "broadr" => Request::Broadr { job: n(0) as u32, temp_k: n(1) },
                "purr" => Request::Purr { job: n(0) as u32, temp_k: n(1), nladr: n(2) as usize, nsamp: n(3) as usize },
                "thermr" => Request::Thermr { job: n(0) as u32, material: n(1) as u8, temp_k: n(2), e_emit: n(3) },
                "groupr" => Request::Groupr { job: n(0) as u32, ngroups: n(1) as usize, sigma0: n(2) },
                other => return Err(format!("unknown request '{other}'")),
            })
        }
    }

    /// `Result`'s curves travel flattened:
    /// `[job, param, secs, n_points, n_curves, (tag, n, x0, y0, …, nw, w…)*]`.
    fn encode(job: u32, param: f64, secs: f64, n_points: usize, curves: &[Curve]) -> Vec<f64> {
        let mut v = vec![job as f64, param, secs, n_points as f64, curves.len() as f64];
        for c in curves {
            v.push(c.tag as f64);
            v.push(c.pts.len() as f64);
            for p in &c.pts {
                v.extend(p);
            }
            v.push(c.w.len() as f64);
            v.extend(&c.w);
        }
        v
    }

    fn decode(v: &[f64], note: String) -> Result<Event, String> {
        let bad = || "malformed result message".to_string();
        if v.len() < 5 {
            return Err(bad());
        }
        let mut i = 5;
        let mut curves = Vec::new();
        for _ in 0..v[4] as usize {
            let tag = *v.get(i).ok_or_else(bad)? as u32;
            let n = *v.get(i + 1).ok_or_else(bad)? as usize;
            i += 2;
            let pts = v.get(i..i + 2 * n).ok_or_else(bad)?.chunks_exact(2).map(|c| [c[0], c[1]]).collect();
            i += 2 * n;
            let nw = *v.get(i).ok_or_else(bad)? as usize;
            i += 1;
            let w = v.get(i..i + nw).ok_or_else(bad)?.to_vec();
            i += nw;
            curves.push(Curve { tag, pts, w });
        }
        Ok(Event::Result { job: v[0] as u32, param: v[1], secs: v[2], n_points: v[3] as usize, curves, note })
    }

    impl Message for Event {
        fn to_js(&self) -> JsValue {
            let o = js::object();
            let (kind, a, note): (&str, Vec<f64>, &str) = match self {
                Event::JobStarted { id, index } => ("started", vec![*id as f64, *index as f64], ""),
                Event::JobDone { id, index, secs } => ("done", vec![*id as f64, *index as f64, *secs], ""),
                Event::Ready { id } => ("ready", vec![*id as f64], ""),
                Event::Result { job, param, secs, n_points, curves, note } => ("result", encode(*job, *param, *secs, *n_points, curves), note.as_str()),
                Event::Error(m) => ("error", Vec::new(), m.as_str()),
            };
            js::set(&o, "kind", kind);
            js::set(&o, "a", js::f64s(&a));
            js::set(&o, "note", note);
            o.into()
        }
        fn from_js(v: &JsValue) -> Result<Self, String> {
            let a = js::get_f64s(v, "a");
            let n = |i: usize| a.get(i).copied().unwrap_or(0.0);
            Ok(match js::get_str(v, "kind").as_str() {
                "started" => Event::JobStarted { id: n(0) as u32, index: n(1) as usize },
                "done" => Event::JobDone { id: n(0) as u32, index: n(1) as usize, secs: n(2) },
                "ready" => Event::Ready { id: n(0) as u32 },
                "result" => decode(&a, js::get_str(v, "note"))?,
                "error" => Event::Error(js::get_str(v, "note")),
                other => return Err(format!("unknown event '{other}'")),
            })
        }
    }

    impl WorkerEngine for NdEngine {
        type Req = Request;
        type Ev = Event;
        fn error(message: String) -> Event {
            Event::Error(message)
        }
        fn handle(state: &Arc<RwLock<Self>>, req: Request, post: Poster<Event>) {
            match req {
                Request::Load { id, rung } => {
                    let st = state.clone();
                    wasm_bindgen_futures::spawn_local(async move {
                        for (index, tape) in rung.info().tapes.iter().enumerate() {
                            post.post(Event::JobStarted { id, index });
                            let t = js_sys::Date::now();
                            let have = st.read().map(|g| g.has(tape)).unwrap_or(false);
                            if !have {
                                let r = async {
                                    let z = fetch_bytes(&format!("data/{}", crate::tapes::wire_name(tape))).await?;
                                    let bytes = crate::tapes::decompress(&z)?;
                                    st.write().map_err(|e| e.to_string())?.add_tape(tape, &bytes)
                                }
                                .await;
                                if let Err(e) = r {
                                    post.post(Event::Error(e));
                                    return;
                                }
                            }
                            post.post(Event::JobDone { id, index, secs: (js_sys::Date::now() - t) / 1000.0 });
                        }
                        post.post(Event::Ready { id });
                    });
                }
                r => {
                    if let Ok(mut g) = state.write() {
                        g.serve(r, dhoby_ghaut::web_demo::platform::now_s, &mut |e| post.post(e));
                    }
                }
            }
        }
    }
}
