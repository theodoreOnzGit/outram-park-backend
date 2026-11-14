//! **The Core k_eff tab** (gh:#1571): the MSRE N-1 full-core Monte Carlo on
//! a pool of workers, started when the reader presses Run and stoppable.
//!
//! The page holds the two state machines of `dhoby_ghaut::mc_web::pool`
//! (gh:#1572): [`DataRelay`] deals the nuclear-data jobs to the workers and
//! relays every product to every other worker, then each builds the model;
//! [`KeffPool`] runs the power iteration on them, one worker sampling the
//! source and every worker transporting chunks of each generation. The
//! workers are `engine::Runner`s that only ever get `Request::Keff`
//! (`keff.rs`), separate from the plant's worker, which keeps running.
//!
//! What the reader sees: the core geometry drawn from the assembled CSG (the
//! drawing rule), k per generation, the running mean with its σ, the
//! measured critical state k_ref = 1.00080 (ORNL-4233, rods at 0.08 % δk/k)
//! and the maintainer's ±1000 pcm band. **The model's default sits outside
//! that band (+1726 ± 23 pcm, the V&V record): the tab shows that, it does
//! not hide it.**
//!
//! GUI and pool-driving code: reached by no test (it needs a window and
//! workers), per the workspace exception for GUI code. What it composes is
//! tested: the two state machines (`dhoby_ghaut::mc_web::pool::tests`, and
//! bit for bit with the HTR-10 core's pool in the `monte_carlo_web`
//! example), the model (`vv::msre_n1::tests::the_study_plans_the_recorded_default`)
//! and the worker (`keff::tests`).

use crate::engine::{Event, Request};
use crate::keff::{coordinator_settings, jobs, Assembly, KEv, KReq, View};
use bugis::web_demo::link::Link;
use dhoby_ghaut::mc_web::keff::KeffConfig;
use dhoby_ghaut::mc_web::pool::{DataOut, DataRelay, GenSummary, KeffPool, Note, RunReq, RELAY_WORDS_PER_FRAME};
use dhoby_ghaut::web_demo::data_cache::Source;
use egui::{Align2, Color32, FontId, Pos2, Rect, RichText, Stroke, Vec2};
use egui_plot::{HLine, Legend, Line, Plot, PlotPoints, Points};
use outram_park_digital_twin_engine::msre::first_criticality::{self as m, Study};
use outram_park_digital_twin_engine::msre::session::{coefficient_from_pair, Coefficients, SPLIT_BASE_F, SPLIT_STEP_F};

/// ORNL-4233's critical state, rods at 0.08 % δk/k (`vv::msre_n1::REFERENCE_K`).
pub const K_REF: f64 = m::REFERENCE_K;
/// The maintainer's band for N-1, ±1000 pcm (gh:#1128, 2026-11-10).
pub const BAND_PCM: f64 = 1000.0;
/// One worker's memory for the pool planner, MB: an ESTIMATE (the HTR-10
/// core's worker holds about 545 MB; MSRE's 52 nuclides at one temperature
/// are of the same order). Not measured.
pub const WORKER_MB: f64 = 700.0;
/// The record's seed for the tab's runs (any is fine; this one is fixed so
/// a run can be repeated bit for bit).
pub const SEED: u64 = 1;

/// Accuracy presets: name, histories, inactive, active, and the σ each is
/// expected to give, pcm, scaled by 1/√(histories × active) from the
/// record's single run of 20 000 × 200 active (about 56 pcm, from the
/// 6-seed ±23 pcm). An ESTIMATE for choosing; each run shows its own σ.
pub const PRESETS: [(&str, usize, usize, usize, f64); 3] = [
    ("Quick", 4_000, 30, 140, 150.0),
    ("Standard", 10_000, 50, 200, 80.0),
    ("Careful", 20_000, 100, 400, 40.0),
];

/// A finished run, kept for the measured coefficients (gh:#1574).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Finished {
    pub study: Study,
    pub cfg: KeffConfig,
    pub k: f64,
    pub sigma: f64,
}

/// **The measured split** (gh:#1574): the N-3 record's three runs on this
/// tab's pool, one after another: A salt and graphite at
/// [`SPLIT_BASE_F`]; C the salt raised by [`SPLIT_STEP_F`]; D the graphite
/// raised. α_fuel from A and C, α_graphite from A and D
/// ([`coefficient_from_pair`]), each with its σ. Never sent to the plant
/// except by the reader's button.
#[derive(Clone, Debug, PartialEq)]
pub struct Split {
    pub cfg: KeffConfig,
    pub shield: bool,
    pub u235_scale: f64,
    /// A, C, D as they finish.
    pub runs: Vec<Finished>,
}

impl Split {
    /// The study of run `i` (0 A, 1 C, 2 D).
    pub fn study(&self, i: usize) -> Study {
        let (b, up) = (SPLIT_BASE_F, SPLIT_BASE_F + SPLIT_STEP_F);
        let (salt_f, graphite_f) = match i {
            0 => (b, b),
            1 => (up, b),
            _ => (b, up),
        };
        Study { salt_f, graphite_f, u235_scale: self.u235_scale, shield: self.shield }
    }

    /// The coefficients, once the three runs are in.
    pub fn coefficients(&self) -> Option<Coefficients> {
        let [a, c, d] = [self.runs.first()?, self.runs.get(1)?, self.runs.get(2)?];
        let (f, sf) = coefficient_from_pair(a.k, a.sigma, c.k, c.sigma, SPLIT_STEP_F);
        let (g, sg) = coefficient_from_pair(a.k, a.sigma, d.k, d.sigma, SPLIT_STEP_F);
        Some(Coefficients::Measured { fuel_per_f: f, fuel_sigma: sf, graphite_per_f: g, graphite_sigma: sg })
    }
}

/// Where a pool is.
#[derive(Clone, Debug, PartialEq)]
enum Phase {
    Processing,
    Assembling,
    Ready,
    Failed(String),
}

/// A pool built for one study.
struct Pool {
    study: Study,
    links: Vec<Link<Request, Event>>,
    data: DataRelay,
    keff: KeffPool<Study>,
    phase: Phase,
    assembled: Vec<Option<Assembly>>,
    started_s: f64,
    ready_s: Option<f64>,
    /// A run waiting for the data.
    pending: Option<KeffConfig>,
}

impl Pool {
    fn start(ctx: &egui::Context, study: Study, workers: usize) -> Result<Self, String> {
        let list = jobs(&study);
        let labels = list.iter().map(|j| j.label()).collect();
        let costs = list.iter().map(|j| j.cost()).collect();
        let mut links = Vec::new();
        for _ in 0..workers {
            #[cfg(target_arch = "wasm32")]
            links.push(bugis::web_demo::link::start_web::<Request, Event>(ctx.clone(), crate::engine::web::WORKER_JS, Event::Error)?);
            #[cfg(not(target_arch = "wasm32"))]
            {
                let c = ctx.clone();
                links.push(bugis::web_demo::link::start_native(crate::engine::Runner::default(), move || c.request_repaint()));
            }
        }
        let n = links.len();
        Ok(Self {
            study,
            links,
            data: DataRelay::new(n, labels, costs),
            keff: KeffPool::new(n, format!("about {WORKER_MB:.0} MB of nuclear data (an estimate)"), coordinator_settings, crate::keff::TRACED),
            phase: Phase::Processing,
            assembled: vec![None; n],
            started_s: bugis::web_demo::platform::now_s(),
            ready_s: None,
            pending: None,
        })
    }

    fn terminate(&self) {
        for l in &self.links {
            l.terminate();
        }
    }

    fn data_phase(&self) -> bool {
        matches!(self.phase, Phase::Processing | Phase::Assembling)
    }

    /// Take every worker's events and send what is due.
    fn pump(&mut self, now: f64) -> Vec<Note> {
        let mut notes = Vec::new();
        for w in 0..self.links.len() {
            for e in self.links[w].drain() {
                match e {
                    Event::Keff(k) => match k {
                        KEv::Started { job } => self.data.on_started(job),
                        KEv::Product { job, secs, data, cached_ms } => {
                            let source = match cached_ms {
                                Some(created_ms) => Source::Cached { created_ms, code: dhoby_ghaut::mc_web::processed_cache::CODE.to_string() },
                                None => Source::Processed,
                            };
                            self.data.on_product(w, job, secs, data, source);
                        }
                        KEv::Assembled(a) => {
                            // The console line an unattended browser check reads.
                            #[cfg(target_arch = "wasm32")]
                            log::info!("msre k_eff pool: worker {} built {} nuclides, {} materials in {:.1} s", w + 1, a.nuclides, a.materials, a.secs);
                            self.assembled[w] = Some(a);
                            let all = (0..self.links.len()).filter(|&v| self.keff.alive[v]).all(|v| self.assembled[v].is_some());
                            if all && self.keff.live_workers() > 0 {
                                self.phase = Phase::Ready;
                                self.ready_s.get_or_insert(now);
                            }
                        }
                        KEv::Source { sites, seed, mesh, epoch } => self.keff.on_source(w, &sites, seed, &mesh, epoch, now),
                        KEv::ChunkDone { data, secs, epoch } => notes.extend(self.keff.on_chunk_done(w, &data, secs, epoch, now)),
                        KEv::Failed { message, epoch } => {
                            if self.data_phase() {
                                self.keff.idle(w);
                                self.phase = Phase::Failed(format!("Worker {} could not build its data: {message}.", w + 1));
                            } else {
                                self.keff.on_failed(w, &message, epoch);
                            }
                        }
                        KEv::Raster { .. } => {}
                    },
                    Event::Error(msg) => {
                        let dp = self.data_phase();
                        if let Some(why) = self.keff.on_error(w, msg, dp) {
                            self.phase = Phase::Failed(why);
                        }
                        if !self.keff.alive[w] {
                            self.links[w].terminate();
                        }
                    }
                    _ => {}
                }
            }
        }
        if !matches!(self.phase, Phase::Failed(_)) {
            let (out, all) = self.data.outbox(RELAY_WORDS_PER_FRAME, self.phase == Phase::Processing);
            for d in out {
                let (w, r) = match d {
                    DataOut::Process { worker, jobs } => (worker, KReq::Process { study: self.study, jobs }),
                    DataOut::Relay { worker, job, data } => (worker, KReq::Product { job, data }),
                    DataOut::Assemble { worker } => (worker, KReq::Assemble { study: self.study }),
                };
                self.links[w].send(Request::Keff(r));
            }
            if all && self.phase == Phase::Processing {
                self.phase = Phase::Assembling;
            }
            if self.phase == Phase::Ready {
                if let Some(cfg) = self.pending.take() {
                    if let Err(e) = self.keff.start_run(cfg, self.study, now) {
                        self.keff.note(e);
                    }
                }
                for (w, r) in self.keff.outbox() {
                    let r = match r {
                        RunReq::Source { cfg, epoch } => KReq::Source { cfg, epoch },
                        RunReq::Chunk { cfg, data, epoch } => KReq::Chunk { cfg, data, epoch },
                    };
                    self.links[w].send(Request::Keff(r));
                }
            }
        }
        notes
    }
}

/// The tab's state.
pub struct KeffTab {
    /// The model the reader is setting up.
    pub study: Study,
    pub preset: usize,
    pool: Option<Pool>,
    /// The geometry's material map, and what it was drawn for.
    raster: Option<(Study, View, usize, usize, Vec<u8>)>,
    asked: Option<(Study, View)>,
    pub view: View,
    texture: Option<(egui::TextureHandle, Study, View)>,
    /// Every run finished this session, newest last.
    pub finished: Vec<Finished>,
    /// The generations of the run on screen.
    gens: Vec<GenSummary>,
    error: Option<String>,
    /// The measured split in progress or done (gh:#1574).
    pub split: Option<Split>,
    /// Coefficients the reader asked to send to the plant (taken by the app).
    send: Option<Coefficients>,
    /// `?kcfg=n,inactive,active,seed`: settings in place of the presets,
    /// for a browser run checked against a native one on the same seed.
    custom: Option<KeffConfig>,
    /// `?krun`: start a run on load (an unattended browser check).
    autorun: bool,
}

impl Default for KeffTab {
    fn default() -> Self {
        Self {
            study: Study::default(),
            preset: 0,
            pool: None,
            raster: None,
            asked: None,
            view: View::Plan,
            texture: None,
            finished: Vec::new(),
            gens: Vec::new(),
            error: None,
            split: None,
            send: None,
            custom: query("kcfg").and_then(|v| {
                let n: Vec<u64> = v.split(',').filter_map(|x| x.trim().parse().ok()).collect();
                (n.len() == 4).then(|| KeffConfig { n_particles: n[0] as usize, n_inactive: n[1] as usize, n_active: n[2] as usize, seed: n[3], point_source: false, want_sites: false })
            }),
            autorun: query("krun").is_some(),
        }
    }
}

/// A URL query value (natively `--key value`).
fn query(key: &str) -> Option<String> {
    bugis::web_demo::platform::query_pairs().into_iter().find(|(k, _)| k == key).map(|(_, v)| v)
}

/// How many pool workers: `?workers=N`, else the device's plan.
fn workers() -> usize {
    let forced = bugis::web_demo::platform::query_pairs().iter().find(|(k, _)| k == "workers").and_then(|(_, v)| v.parse().ok());
    bugis::web_demo::pool::plan(bugis::web_demo::pool::Device::probe(), WORKER_MB, forced).workers
}

impl KeffTab {
    /// The settings of the chosen preset.
    pub fn cfg(&self) -> KeffConfig {
        if let Some(c) = self.custom {
            return c;
        }
        let (_, n, i, a, _) = PRESETS[self.preset.min(PRESETS.len() - 1)];
        KeffConfig { n_particles: n, n_inactive: i, n_active: a, seed: SEED, point_source: false, want_sites: false }
    }

    /// A run is going (or waiting for its data).
    pub fn busy(&self) -> bool {
        self.pool.as_ref().is_some_and(|p| p.pending.is_some() || p.keff.going() || p.data_phase())
    }

    /// Run k_eff on `study` with `cfg`: on the pool that holds its data, or
    /// on a new pool (the old one stopped).
    pub fn run(&mut self, ctx: &egui::Context, study: Study, cfg: KeffConfig) {
        self.error = None;
        self.gens.clear();
        let reuse = self.pool.as_ref().is_some_and(|p| p.study == study && !matches!(p.phase, Phase::Failed(_)));
        if !reuse {
            if let Some(p) = self.pool.take() {
                p.terminate();
            }
            match Pool::start(ctx, study, workers()) {
                Ok(p) => self.pool = Some(p),
                Err(e) => {
                    self.error = Some(e);
                    return;
                }
            }
        }
        if let Some(p) = self.pool.as_mut() {
            p.pending = Some(cfg);
        }
    }

    /// Start the measured split (gh:#1574) at the chosen accuracy, with the
    /// reader's shield and U-235 settings; its first run starts at once.
    pub fn measure(&mut self, ctx: &egui::Context) {
        let sp = Split { cfg: self.cfg(), shield: self.study.shield, u235_scale: self.study.u235_scale, runs: Vec::new() };
        let (s, c) = (sp.study(0), sp.cfg);
        self.split = Some(sp);
        self.run(ctx, s, c);
    }

    /// Coefficients the reader asked to send to the plant (once).
    pub fn take_send(&mut self) -> Option<Coefficients> {
        self.send.take()
    }

    /// The measured coefficients, once the split is complete.
    pub fn measured(&self) -> Option<Coefficients> {
        self.split.as_ref().and_then(Split::coefficients)
    }

    /// Stop the run (the data stay with the workers).
    pub fn stop(&mut self) {
        self.split = self.split.take().filter(|s| s.runs.len() == 3);
        if let Some(p) = self.pool.as_mut() {
            p.pending = None;
            p.keff.stop_run();
        }
    }

    /// Each frame: pump the pool, keep finished runs; ask for the geometry
    /// when it is not drawn for the current settings. Returns a run that
    /// has just finished.
    pub fn pump(&mut self, ctx: &egui::Context, plant: &Link<Request, Event>) -> Option<Finished> {
        let now = bugis::web_demo::platform::now_s();
        if self.autorun {
            self.autorun = false;
            let (s, c) = (self.study, self.cfg());
            self.run(ctx, s, c);
        }
        if self.asked != Some((self.study, self.view)) && self.raster.as_ref().map(|r| (r.0, r.1)) != Some((self.study, self.view)) {
            plant.send(Request::Keff(KReq::Raster { study: self.study, view: self.view, px: 480 }));
            self.asked = Some((self.study, self.view));
        }
        let mut done = None;
        let preset = self.cfg();
        if let Some(p) = self.pool.as_mut() {
            for n in p.pump(now) {
                match n {
                    Note::Generation(g) => self.gens.push(g.without_tracks()),
                    Note::RunDone => {
                        if let Some((k, s)) = p.keff.run.as_ref().and_then(|r| r.it.as_ref()).and_then(|it| it.k_mean()) {
                            let f = Finished { study: p.study, cfg: p.keff.run.as_ref().map_or(preset, |r| r.cfg), k, sigma: s };
                            self.finished.push(f);
                            done = Some(f);
                        }
                    }
                }
            }
            if let Phase::Failed(m) = &p.phase {
                self.error = Some(m.clone());
            } else if let Some(f) = p.keff.run.as_ref().and_then(|r| r.failed.clone()) {
                self.error = Some(f);
            }
        }
        // The measured split goes on to its next run.
        if let (Some(f), Some(sp)) = (done, self.split.as_mut()) {
            if sp.runs.len() < 3 && f.study == sp.study(sp.runs.len()) {
                sp.runs.push(f);
                if sp.runs.len() < 3 {
                    let (s, c) = (sp.study(sp.runs.len()), sp.cfg);
                    self.run(ctx, s, c);
                }
            }
        }
        done
    }

    /// The plant worker's raster reply.
    pub fn on_raster(&mut self, view: View, w: usize, h: usize, ids: Vec<u8>) {
        if let Some((s, v)) = self.asked {
            if v == view {
                self.raster = Some((s, view, w, h, ids));
                self.texture = None;
            }
        }
    }

    /// The side-panel controls. GUI drawing code (needs a window).
    pub fn panel(&mut self, ui: &mut egui::Ui) {
        ui.heading("Core k_eff (full-core Monte Carlo)");
        ui.label(RichText::new("The MSRE N-1 model: ORNL-TM-0728's core, vessel and thermal shield, natural-Mo INOR-8, the fuel salt of first criticality (ORNL-4233), dimensions following temperature, ENDF/B-VIII.0 processed by this workspace's NJOY port in your browser, on a pool of workers.").small());
        let busy = self.busy();
        ui.add_enabled_ui(!busy, |ui| {
            ui.add(egui::Slider::new(&mut self.study.salt_f, 900.0..=1400.0).text("salt °F"));
            ui.add(egui::Slider::new(&mut self.study.graphite_f, 900.0..=1400.0).text("graphite °F"));
            ui.add(egui::Slider::new(&mut self.study.u235_scale, 0.9..=1.1).text("U-235 × (enriching salt)"));
            ui.checkbox(&mut self.study.shield, "Thermal shield (TM-0728, the default; off: ~37 nuclides, lighter)");
            if ui.button("Back to the record's default").clicked() {
                self.study = Study::default();
            }
            egui::ComboBox::from_label("accuracy")
                .selected_text(format!("{} (~±{:.0} pcm)", PRESETS[self.preset].0, PRESETS[self.preset].4))
                .show_ui(ui, |ui| {
                    for (i, p) in PRESETS.iter().enumerate() {
                        ui.selectable_value(&mut self.preset, i, format!("{}: {} × [{} + {}], ~±{:.0} pcm", p.0, p.1, p.2, p.3, p.4));
                    }
                });
        });
        ui.label(RichText::new("The ± are estimates scaled from the V&V record; each run shows its own σ. A changed temperature or shield means the nuclear data are processed again (a cached product is read back).").small());
        ui.horizontal(|ui| {
            if busy {
                if ui.add(egui::Button::new(RichText::new("■ Stop").size(18.0)).min_size(Vec2::new(110.0, 40.0))).clicked() {
                    self.stop();
                }
            } else if ui.add(egui::Button::new(RichText::new("▶ Run k_eff").size(18.0)).min_size(Vec2::new(130.0, 40.0))).clicked() {
                let (s, c) = (self.study, self.cfg());
                self.run(ui.ctx(), s, c);
            }
        });
        if let Some(e) = &self.error {
            ui.colored_label(ui.visuals().error_fg_color, e);
        }
        ui.separator();
        ui.label(RichText::new("Temperature coefficients (the N-3 split)").strong());
        ui.label(RichText::new(format!(
            "Three runs at the chosen accuracy: salt and graphite at {SPLIT_BASE_F:.0} °F, then the salt {SPLIT_STEP_F:.0} °F hotter, then the graphite. α = Δρ/ΔT, ρ = (k − 1)/k. Quick gives about ±2e-5 /°F on each (about half the value); Careful about ±0.6e-5."
        )).small());
        let measuring = self.split.as_ref().is_some_and(|s| s.runs.len() < 3);
        if !busy && !measuring && ui.add(egui::Button::new("Measure the coefficients (3 runs)").min_size(Vec2::new(0.0, 36.0))).clicked() {
            self.measure(ui.ctx());
        }
        if let Some(sp) = &self.split {
            if sp.runs.len() < 3 {
                ui.label(RichText::new(format!("Run {} of 3: salt {:.0} °F, graphite {:.0} °F", sp.runs.len() + 1, sp.study(sp.runs.len()).salt_f, sp.study(sp.runs.len()).graphite_f)).small());
            }
        }
        if let Some(c) = self.measured() {
            ui.label(RichText::new(c.describe()).strong());
            ui.label(RichText::new("The precomputed N-3 split (6 seeds each) is fuel −3.79 ± 0.37, graphite −3.26 ± 0.33 (×1e-5 /°F); measured at the MSRE: fuel −4.7 ± 0.7 (ORNL-4233).").small());
            if ui.add(egui::Button::new("Send to plant (resets it with these)").min_size(Vec2::new(0.0, 36.0))).clicked() {
                self.send = Some(c);
            }
        }
        if let Some(p) = &self.pool {
            let line = match &p.phase {
                Phase::Processing => format!("Nuclear data: {:.0} % on {} workers ({})", 100.0 * p.data.processed(), p.links.len(), p.data.data_source()),
                Phase::Assembling => format!("Building the model on {} workers", p.links.len()),
                Phase::Ready => format!("{} workers hold the model ({}); data ready in {:.0} s", p.keff.live_workers(), p.data.data_source(), p.ready_s.unwrap_or(p.started_s) - p.started_s),
                Phase::Failed(m) => format!("Failed: {m}"),
            };
            ui.label(RichText::new(line).small());
            for l in p.keff.log.iter().rev().take(3) {
                ui.label(RichText::new(l).small().color(ui.visuals().warn_fg_color));
            }
        }
        if !self.finished.is_empty() {
            ui.separator();
            ui.label(RichText::new("Runs this session").strong());
            for f in self.finished.iter().rev().take(6) {
                ui.label(RichText::new(format!(
                    "salt {:.0} °F, graphite {:.0} °F, U-235 ×{:.3}{}: k = {:.5} ± {:.5} ({:+.0} pcm from k_ref)",
                    f.study.salt_f,
                    f.study.graphite_f,
                    f.study.u235_scale,
                    if f.study.shield { "" } else { ", no shield" },
                    f.k,
                    f.sigma,
                    1e5 * (f.k - K_REF) / f.k / K_REF
                ))
                .small());
            }
        }
    }

    /// The main view: geometry above, k below. GUI drawing code.
    pub fn view_ui(&mut self, ui: &mut egui::Ui, rect: Rect) {
        let narrow = rect.width() < 700.0;
        let (geo, plot) = if narrow {
            let h = rect.height() * 0.45;
            (Rect::from_min_size(rect.min, Vec2::new(rect.width(), h)), Rect::from_min_max(Pos2::new(rect.left(), rect.top() + h + 8.0), rect.max))
        } else {
            let w = rect.width() * 0.45;
            (Rect::from_min_size(rect.min, Vec2::new(w, rect.height())), Rect::from_min_max(Pos2::new(rect.left() + w + 8.0, rect.top()), rect.max))
        };
        self.geometry_ui(ui, geo);
        ui.scope_builder(egui::UiBuilder::new().max_rect(plot), |ui| self.plot_ui(ui));
    }

    fn geometry_ui(&mut self, ui: &mut egui::Ui, rect: Rect) {
        ui.scope_builder(egui::UiBuilder::new().max_rect(Rect::from_min_size(rect.min, Vec2::new(rect.width(), 30.0))), |ui| {
            ui.horizontal(|ui| {
                ui.selectable_value(&mut self.view, View::Plan, "Plan (x-y)");
                ui.selectable_value(&mut self.view, View::Elevation, "Elevation (x-z)");
            });
        });
        let area = Rect::from_min_max(rect.min + Vec2::new(0.0, 34.0), rect.max - Vec2::new(0.0, 70.0));
        let Some((s, v, w, h, ids)) = &self.raster else {
            ui.painter().text(area.center(), Align2::CENTER_CENTER, "Drawing the geometry…", FontId::proportional(15.0), ui.visuals().text_color());
            return;
        };
        let plan = s.plan();
        if self.texture.as_ref().map(|t| (t.1, t.2)) != Some((*s, *v)) {
            let px: Vec<Color32> = ids.iter().map(|&i| colour(i)).collect();
            let img = egui::ColorImage { size: [*w, *h], pixels: px, source_size: Vec2::new(*w as f32, *h as f32) };
            self.texture = Some((ui.ctx().load_texture("msre_geometry", img, egui::TextureOptions::NEAREST), *s, *v));
        }
        if let Some((t, ..)) = &self.texture {
            let aspect = *h as f32 / *w as f32;
            let side = area.width().min(area.height() / aspect);
            let r = Rect::from_center_size(area.center(), Vec2::new(side, side * aspect));
            ui.painter().image(t.id(), r, Rect::from_min_max(Pos2::ZERO, Pos2::new(1.0, 1.0)), Color32::WHITE);
            let (_, _, width) = v.frame(s);
            ui.painter().text(r.left_bottom() + Vec2::new(0.0, 4.0), Align2::LEFT_TOP, format!("{:.0} cm across · drawn from the assembled CSG", width[0]), FontId::proportional(12.0), ui.visuals().text_color());
        }
        // Legend: the materials present, wrapped by each label's own width.
        let mut y = rect.bottom() - 64.0;
        let mut x = rect.left();
        for (i, (name, _)) in plan.materials.iter().enumerate() {
            let c = colour(i as u8 + 1);
            let galley = ui.painter().layout_no_wrap(name.to_string(), FontId::proportional(11.0), ui.visuals().text_color());
            let wdt = 14.0 + galley.size().x + 10.0;
            if x + wdt > rect.right() && x > rect.left() {
                x = rect.left();
                y += 15.0;
            }
            ui.painter().rect_filled(Rect::from_min_size(Pos2::new(x, y + 2.0), Vec2::splat(10.0)), 1.0, c);
            ui.painter().galley(Pos2::new(x + 14.0, y), galley, ui.visuals().text_color());
            x += wdt;
        }
    }

    fn plot_ui(&mut self, ui: &mut egui::Ui) {
        let cfg = self.pool.as_ref().and_then(|p| p.keff.run.as_ref()).map_or(self.cfg(), |r| r.cfg);
        let total = cfg.n_inactive + cfg.n_active;
        let mean = self.pool.as_ref().and_then(|p| p.keff.run.as_ref()).and_then(|r| r.it.as_ref()).and_then(|it| it.k_mean());
        let head = match mean {
            Some((k, s)) => format!("k = {k:.5} ± {s:.5}: {:+.0} ± {:.0} pcm from k_ref {K_REF:.5} · generation {}/{total}", 1e5 * (k - K_REF) / (k * K_REF), 1e5 * s / (k * K_REF), self.gens.len()),
            None if self.gens.is_empty() => format!("Press Run: {} histories × [{} inactive + {} active] on {} workers.", cfg.n_particles, cfg.n_inactive, cfg.n_active, workers()),
            None => format!("generation {}/{total} (inactive: the source is converging)", self.gens.len()),
        };
        ui.label(RichText::new(head).strong());
        ui.label(RichText::new("The model's default sits +1726 ± 23 pcm above k_ref, outside the ±1000 pcm band (V&V record, 2026-11-13); what is left is open (gh:#1128). Not validated.").small().color(ui.visuals().warn_fg_color));
        let k: Vec<[f64; 2]> = self.gens.iter().map(|g| [g.gen.index as f64, g.gen.k]).collect();
        let means: Vec<[f64; 2]> = self.gens.iter().filter_map(|g| g.gen.k_mean.map(|(m, _)| [g.gen.index as f64, m])).collect();
        let band = BAND_PCM * 1e-5;
        Plot::new("msre_keff").legend(Legend::default()).allow_scroll(false).x_axis_label("generation").y_axis_label("k").show(ui, |p| {
            p.hline(HLine::new("k_ref (ORNL-4233)", K_REF).stroke(Stroke::new(1.5, Color32::from_rgb(80, 180, 80))));
            p.hline(HLine::new("±1000 pcm band", K_REF * (1.0 + band)).stroke(Stroke::new(1.0, Color32::from_rgb(180, 180, 80))));
            p.hline(HLine::new("±1000 pcm band", K_REF * (1.0 - band)).stroke(Stroke::new(1.0, Color32::from_rgb(180, 180, 80))));
            if cfg.n_inactive > 0 {
                p.vline(egui_plot::VLine::new("active from here", cfg.n_inactive as f64).stroke(Stroke::new(1.0, Color32::GRAY)));
            }
            p.points(Points::new("k per generation", PlotPoints::from(k)).radius(2.0));
            p.line(Line::new("running mean (active)", PlotPoints::from(means)).width(2.0));
        });
    }
}

/// A material's drawing colour (index + 1 from `keff::raster`; 0 void, 254
/// overlap, 255 outside). An indicative palette, no physical meaning.
fn colour(id: u8) -> Color32 {
    const P: [Color32; 15] = [
        Color32::from_rgb(220, 60, 40),
        Color32::from_rgb(70, 70, 70),
        Color32::from_rgb(160, 160, 200),
        Color32::from_rgb(240, 170, 40),
        Color32::from_rgb(150, 90, 60),
        Color32::from_rgb(240, 120, 90),
        Color32::from_rgb(200, 100, 160),
        Color32::from_rgb(90, 140, 90),
        Color32::from_rgb(120, 60, 140),
        Color32::from_rgb(110, 110, 120),
        Color32::from_rgb(60, 130, 200),
        Color32::from_rgb(30, 30, 30),
        Color32::from_rgb(255, 255, 0),
        Color32::from_rgb(0, 150, 150),
        Color32::from_rgb(200, 200, 120),
    ];
    match id {
        0 => Color32::from_rgb(245, 245, 245),
        254 => Color32::from_rgb(255, 0, 255),
        255 => Color32::from_rgb(20, 20, 28),
        i => P[(i as usize - 1) % P.len()],
    }
}

#[cfg(all(test, not(target_arch = "wasm32")))]
mod tests {
    use super::*;
    use crate::keff::{entropy_mesh, native_tape, run_settings, KeffWorker};

    /// **gh:#1571 / #1572: a pooled run on the real MSRE model is
    /// `run_keff_csg_par`'s, bit for bit.** The tab's pool (two native
    /// worker threads, the library's `DataRelay` and `KeffPool`) processes
    /// the default study's 54 jobs, relays them, builds the model on each
    /// worker and runs 500 × [2 + 3] on seed 7. Separately, one worker
    /// processes every job itself and `run_keff_csg_par` runs on its model
    /// (2 threads, the entropy mesh). Every generation's k must be equal to
    /// the bit. Prints the k values, the timings and the hardware.
    #[test]
    #[ignore = "processes the MSRE model's 54 jobs twice natively and runs k (many minutes); run with --ignored --nocapture"]
    fn a_pooled_run_is_run_keff_csg_par_bit_for_bit() {
        let study = Study::default();
        let cfg = KeffConfig { n_particles: 500, n_inactive: 2, n_active: 3, seed: 7, point_source: false, want_sites: false };
        let ctx = egui::Context::default();
        let t0 = std::time::Instant::now();
        let mut pool = Pool::start(&ctx, study, 2).expect("pool");
        pool.pending = Some(cfg);
        let mut ks = Vec::new();
        'run: loop {
            for n in pool.pump(bugis::web_demo::platform::now_s()) {
                match n {
                    Note::Generation(g) => ks.push(g.gen.k),
                    Note::RunDone => break 'run,
                }
            }
            if let Phase::Failed(m) = &pool.phase {
                panic!("{m}");
            }
            if let Some(f) = pool.keff.run.as_ref().and_then(|r| r.failed.clone()) {
                panic!("{f}");
            }
            assert!(t0.elapsed().as_secs() < 7200, "not done in two hours");
            std::thread::sleep(std::time::Duration::from_millis(10));
        }
        let pool_s = t0.elapsed().as_secs_f64();
        pool.terminate();
        let t1 = std::time::Instant::now();
        let mut w = KeffWorker::default();
        let list = jobs(&study);
        for (j, job) in list.iter().enumerate() {
            let b = job.tape().map(|t| native_tape(t).expect("tape")).unwrap_or_default();
            w.process(&study, j, &b).expect("process");
        }
        w.assemble(&study, native_tape, || 0.0).expect("assemble");
        let md = w.model.as_ref().expect("model");
        let r = outram_mc_libs::physics::transport_csg::run_keff_csg_par(
            &md.geometry,
            &md.materials,
            &md.nuclides,
            &[],
            Some(&entropy_mesh()),
            m::source_box(),
            &run_settings(&cfg, md.salt_k),
            None,
            &[],
            None,
            outram_mc_libs::physics::compute::ThreadCount::Fixed(2),
        );
        let ref_s = t1.elapsed().as_secs_f64();
        let bits = |v: &[f64]| v.iter().map(|k| k.to_bits()).collect::<Vec<_>>();
        println!("MSRE_POOL_K {:?}", ks.iter().map(|k| format!("{k:.17e}")).collect::<Vec<_>>());
        println!("MSRE_PAR_K  {:?} mean {:.5} ± {:.5}", r.k_by_generation.iter().map(|k| format!("{k:.17e}")).collect::<Vec<_>>(), r.k_mean, r.k_std);
        println!(
            "MSRE_TIMING pool (process + relay + build + run, 2 workers) {pool_s:.1} s; reference (process + build + run, one worker then 2 threads) {ref_s:.1} s; hardware: {}",
            outram_mc_libs::perf_report::HardwareInfo::detect().headline()
        );
        assert_eq!(bits(&ks), bits(&r.k_by_generation));
    }
}
