//! **The Core k_eff tab's page** (gh:#1571): the reader's controls, the
//! geometry drawn from the assembled CSG (the drawing rule), k per
//! generation with its running mean, the measured critical state k_ref =
//! 1.00080 (ORNL-4233) and the maintainer's ±1000 pcm band. **The model's
//! default sits outside that band (+1726 ± 23 pcm, the V&V record): the tab
//! shows that, it does not hide it.**
//!
//! **Since 2026-11-18 (gh:#1661) the tab uses the standard k_eff layout**
//! every k_eff simulator shares (#1652; `dhoby_ghaut::mc_web`): the left
//! panel holds the model's sliders (temperatures, U-235, shield, the three
//! control rods by `rod_bank`, the salt level, the enrichment) and the
//! run settings (`run_settings`); the bottom dock k and the Shannon
//! entropy (`keffplot`); the right dock the rods (`rod_bank::draw_bank`);
//! the main view the geometry, with zoom in, out and reset. With every new
//! control at its default (rods not modelled, full vessel, the salt's own
//! enrichment) the study is the record's, so a run is the same run as
//! before.
//!
//! The pool, the measured split and the presets are the library's
//! (`msre::web::keff_pool`, gh:#1578); this file is the egui half.
//!
//! GUI drawing and tab-driving code: reached by no test (it needs a window
//! and workers), per the workspace exception for GUI code. What it drives is
//! tested in `msre::web` (the pool bit for bit with `run_keff_csg_par`).

use crate::engine::{Event, Request};
use bugis::web_demo::link::Link;
use bugis::web_demo::loading::Loading;
use bugis::web_demo::dock::{Dock, Edge};
use dhoby_ghaut::mc_web::keff::KeffConfig;
use dhoby_ghaut::mc_web::keffplot::{self, GenPoint, RefLine, RunShape, StatusLine};
use dhoby_ghaut::mc_web::rod_bank::{self, BankStyle, RodBank};
use dhoby_ghaut::mc_web::run_settings::{run_settings_ui, RunPresets};
use dhoby_ghaut::mc_web::pool::{GenSummary, Note};
use egui::{Align2, Color32, FontId, Pos2, Rect, RichText, Vec2};
use outram_park_digital_twin_engine::msre::first_criticality::{self as m, Study};
use outram_park_digital_twin_engine::msre::session::{Coefficients, SPLIT_BASE_F, SPLIT_STEP_F};
use outram_park_digital_twin_engine::msre::web::keff::{jobs, Job, KReq, View};
use outram_park_digital_twin_engine::msre::web::keff_pool::{plan_workers, preset_cfg, Finished, Phase, Pool, Split, BAND_PCM, K_REF, PRESETS};

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
    /// The data processing as the HTR-10 demo shows it: a card on the main
    /// view and a per-job table in the panel (gh:#1578), for the pool's study.
    loading: Option<(Study, Loading)>,
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
    /// `?kmeasure`: start the measured split on load (the same check).
    automeasure: bool,
    /// The run settings the reader has set (the standard left-panel
    /// fields, gh:#1661); a preset or `?kcfg=` fills them.
    run_cfg: KeffConfig,
    /// The three rods (gh:#1658), in `m::OPENINGS` order (rods 2, 1, 3),
    /// and whether they are modelled at all (off: the record's empty
    /// thimbles).
    rods: RodBank,
    rods_on: bool,
    /// The salt level (gh:#1659): on, and z in cm.
    level_on: bool,
    level_cm: f64,
    /// The enrichment (gh:#1660): on, and the U-235 weight fraction.
    enrich_on: bool,
    enrich_w: f64,
    /// The main view's zoom and pan over the geometry drawing.
    geo_view: bugis::web_demo::view::View,
    /// The standard docks: k and entropy below, the rods on the right.
    keff_dock: Dock,
    views_dock: Dock,
}

/// The V&V record's settings (gh:#1128): 20 000 × [100 + 200], seed 1.
const RECORD_RUN: KeffConfig = KeffConfig { n_particles: 20_000, n_inactive: 100, n_active: 200, seed: 1, point_source: false, want_sites: false };

/// The rods' names in `m::OPENINGS` order (ORNL-4233's numbering,
/// `m::rods_inserted`).
const ROD_LABELS: [&str; 3] = ["rod 2", "rod 1 (regulating, ORNL-4233)", "rod 3"];

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
            loading: None,
            finished: Vec::new(),
            gens: Vec::new(),
            error: None,
            split: None,
            send: None,
            custom: KeffTab::custom_from_query(),
            run_cfg: KeffTab::custom_from_query().unwrap_or(preset_cfg(0)),
            autorun: query("krun").is_some(),
            automeasure: query("kmeasure").is_some(),
            rods: RodBank::from_query(3, 1, &bugis::web_demo::platform::query_pairs()),
            rods_on: query("rods").is_some(),
            level_on: false,
            level_cm: m::H_CORE,
            enrich_on: false,
            enrich_w: 0.30993,
            geo_view: bugis::web_demo::view::View::new(1.0),
            keff_dock: Dock::new(Edge::Bottom, "k_eff · entropy"),
            views_dock: Dock::new(Edge::Right, "Rods"),
        }
    }
}

/// A URL query value (natively `--key value`).
fn query(key: &str) -> Option<String> {
    bugis::web_demo::platform::query_pairs().into_iter().find(|(k, _)| k == key).map(|(_, v)| v)
}

/// How many pool workers: `?workers=N`, else the device's plan.
fn workers() -> usize {
    plan_workers(query("workers").and_then(|v| v.parse().ok()))
}

impl KeffTab {
    /// `?kcfg=n,inactive,active,seed`: settings in place of the presets.
    fn custom_from_query() -> Option<KeffConfig> {
        query("kcfg").and_then(|v| {
            let n: Vec<u64> = v.split(',').filter_map(|x| x.trim().parse().ok()).collect();
            (n.len() == 4).then(|| KeffConfig { n_particles: n[0] as usize, n_inactive: n[1] as usize, n_active: n[2] as usize, seed: n[3], point_source: false, want_sites: false })
        })
    }

    /// The settings of the chosen preset.
    pub fn cfg(&self) -> KeffConfig {
        self.custom.unwrap_or(self.run_cfg)
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
        if self.automeasure {
            self.automeasure = false;
            self.measure(ctx);
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
                            // The console line an unattended browser check reads.
                            #[cfg(target_arch = "wasm32")]
                            log::info!("msre k_eff run done: salt {} F, graphite {} F: k = {k:e} +- {s:e}", f.study.salt_f, f.study.graphite_f);
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
        self.sync_loading();
        // The measured split goes on to its next run.
        if let (Some(f), Some(sp)) = (done, self.split.as_mut()) {
            if sp.runs.len() < 3 && f.study == sp.study(sp.runs.len()) {
                sp.runs.push(f);
                #[cfg(target_arch = "wasm32")]
                if let Some(c) = sp.coefficients() {
                    log::info!("msre measured split: {}", c.describe());
                }
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
            ui.separator();
            ui.label(RichText::new("Control rods").strong());
            ui.checkbox(&mut self.rods_on, "Model the rods (off: the record's empty thimbles)");
            if self.rods_on {
                let labels: Vec<String> = ROD_LABELS.iter().map(|l| (*l).to_string()).collect();
                rod_bank::controls_ui(
                    ui,
                    "msre-rods",
                    &mut self.rods,
                    &labels,
                    "Three Gd2O3-Al2O3 rods in thimbles near the centre (TM-0728 Table 5.4). 0 % is the upper limit, 51 in withdrawn, which still leaves 2.75 in of poison in the top of the graphite; 100 % is the driven lower limit (ORNL-4233 Fig. 2).",
                    Some("Needs ENDF/B-VIII.0 Gd-152, 154, 155, 158 and 160, not yet in the published data (gh:#1658): a run with rods fails at the data step until they are."),
                );
            }
            ui.separator();
            ui.label(RichText::new("Fuel salt").strong());
            ui.checkbox(&mut self.level_on, "Partly filled (off: a full vessel, the record)");
            if self.level_on {
                ui.add(egui::Slider::new(&mut self.level_cm, -20.0..=m::H_CORE + 20.0).text("salt surface z, cm (graphite 0 to 163)"));
            }
            ui.checkbox(&mut self.enrich_on, "Set the enrichment (off: the salt's own, 30.99 wt %)");
            if self.enrich_on {
                ui.add(egui::Slider::new(&mut self.enrich_w, 0.20..=0.93).text("U-235 wt fraction of the uranium"));
            }
            self.study.rods = self.rods_on.then(|| [self.rods.rods[0], self.rods.rods[1], self.rods.rods[2]]);
            self.study.salt_level_cm = self.level_on.then_some(self.level_cm);
            self.study.u235_enrichment = self.enrich_on.then_some(self.enrich_w);
            if ui.button("Back to the record's default").clicked() {
                self.study = Study::default();
                (self.rods_on, self.level_on, self.enrich_on) = (false, false, false);
            }
            ui.separator();
            ui.label(RichText::new("k_eff settings").strong());
            let before = self.preset;
            egui::ComboBox::from_label("preset")
                .selected_text(format!("{} (~±{:.0} pcm)", PRESETS[self.preset].0, PRESETS[self.preset].4))
                .show_ui(ui, |ui| {
                    for (i, p) in PRESETS.iter().enumerate() {
                        ui.selectable_value(&mut self.preset, i, format!("{}: {} × [{} + {}], ~±{:.0} pcm", p.0, p.1, p.2, p.3, p.4));
                    }
                });
            if self.preset != before {
                self.run_cfg = preset_cfg(self.preset);
                self.custom = None;
            }
            if run_settings_ui(ui, "msre_cfg", &mut self.run_cfg, 0, &RunPresets { defaults: preset_cfg(0), recorded: Some(RECORD_RUN) }, busy) {
                self.custom = None;
            }
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
            if ui.add(egui::Button::new("Send to plant (switches the running plant to these)").min_size(Vec2::new(0.0, 36.0))).clicked() {
                self.send = Some(c);
            }
            ui.label(RichText::new("The plant keeps its state; feedback is α(T − T_ref), so a plant away from its starting temperatures steps in reactivity at the switch (a change of model, not something an operator could do).").small());
        }
        if let Some(p) = &self.pool {
            let line = match &p.phase {
                Phase::Processing => processing_line(p),
                Phase::Assembling => format!("Building the model on {} workers", p.links.len()),
                Phase::Ready => format!("{} workers hold the model ({}); data ready in {:.0} s", p.keff.live_workers(), p.data.data_source(), p.ready_s.unwrap_or(p.started_s) - p.started_s),
                Phase::Failed(m) => format!("Failed: {m}"),
            };
            ui.label(RichText::new(line).small());
            if let Some((_, l)) = self.loading.as_ref() {
                egui::CollapsingHeader::new(format!("Nuclear-data jobs ({} of {} done)", l.done(), l.labels.len())).default_open(false).show(ui, |ui| l.grid(ui));
            }
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

    /// Follow the pool's jobs into the loading card (a job started, a job
    /// done), starting a fresh card for a new study. Tab-driving code.
    fn sync_loading(&mut self) {
        let Some(p) = self.pool.as_ref() else { return };
        if self.loading.as_ref().map(|l| l.0) != Some(p.study) {
            let list = jobs(&p.study);
            let labels = list.iter().map(job_label).collect();
            // The card reads each weight as the job's expected seconds. The
            // costs are desktop estimates (U-235 58 s); on a phone U-235
            // took about 500 s (the maintainer, 2026-11-15), so a phone's
            // are scaled by that ratio (an ESTIMATE from one measurement),
            // and the bar does not race to a third and stall.
            let slow = if bugis::web_demo::pool::Device::probe().phone { PHONE_SLOWDOWN } else { 1.0 };
            self.loading = Some((p.study, Loading::new(labels, list.iter().map(|j| j.cost() * slow).collect())));
        }
        if let Some((_, l)) = self.loading.as_mut() {
            for j in 0..p.data.started.len().min(l.labels.len()) {
                if p.data.started[j] && l.job_started[j].is_none() {
                    l.job_started(j);
                }
                if let (Some(secs), None) = (p.data.secs[j], l.job_secs[j]) {
                    l.job_done(j, secs);
                }
            }
        }
    }

    /// The main view in the standard k_eff layout (gh:#1661): the
    /// geometry in the middle, the k and entropy dock below, the rods dock
    /// on the right. GUI drawing code.
    pub fn view_ui(&mut self, ui: &mut egui::Ui, rect: Rect) {
        let views = self.views_dock.split(rect);
        if let Some(body) = views.body {
            self.rods_ui(ui, body);
        }
        if !views.covers_main() {
            let keff = self.keff_dock.split(views.main);
            // A folded right dock leaves its tab over the main view's top
            // right, where the zoom buttons go: put them below it.
            let clear = if views.body.is_none() { (views.header.bottom() + 6.0 - keff.main.top()).max(0.0) } else { 0.0 };
            self.geometry_ui(ui, keff.main, clear);
            if let Some(body) = keff.body {
                let painter = ui.painter_at(body);
                self.keff_body(ui, &painter, body);
            }
            // While the nuclear data are processed, the card over the main
            // view (as the HTR-10 demo's), so it shows with the panel folded.
            if let (Some(p), Some((_, l))) = (self.pool.as_ref(), self.loading.as_ref()) {
                if p.data_phase() {
                    let note = if p.data.secs.iter().any(Option::is_some) {
                        format!("First visit: several minutes in this browser; U-235 and U-238 are the longest (U-235 alone took about 500 s on a phone, measured by the maintainer, 2026-11-15). A later visit reads the results from this browser's cache. So far: {}.", p.data.data_source())
                    } else {
                        "First visit: several minutes in this browser; U-235 and U-238 are the longest (U-235 alone took about 500 s on a phone, measured by the maintainer, 2026-11-15). A later visit reads the results from this browser's cache.".to_string()
                    };
                    let title = format!("Processing ENDF/B-VIII.0 on {} workers", p.links.len());
                    l.card(ui.painter(), keff.main, &title, Some(&note));
                }
            }
            let row = self.keff_dock.toggle(ui, &keff);
            self.run_buttons(ui, row);
        }
        self.views_dock.toggle(ui, &views);
    }

    /// Run / Stop in the bottom dock's header, as HTR-10's. GUI code.
    fn run_buttons(&mut self, ui: &mut egui::Ui, row: Rect) {
        if row.width() < 60.0 {
            return;
        }
        let b = Rect::from_min_size(row.min, Vec2::new(110.0_f32.min(row.width()), row.height()));
        if self.busy() {
            if ui.put(b, egui::Button::new(RichText::new("■ Stop").size(16.0))).clicked() {
                self.stop();
            }
        } else if ui.put(b, egui::Button::new(RichText::new("▶ Run").size(16.0))).clicked() {
            let (s, c) = (self.study, self.cfg());
            self.run(ui.ctx(), s, c);
        }
    }

    /// The bottom dock: the run's k with its running mean against k_ref
    /// and the ±1000 pcm band, and the source's Shannon entropy, with the
    /// shared plots (`dhoby_ghaut::mc_web::keffplot`). GUI drawing code.
    fn keff_body(&self, ui: &mut egui::Ui, painter: &egui::Painter, rect: Rect) {
        let rect = rect.shrink2(Vec2::new(8.0, 4.0));
        let run = self.pool.as_ref().and_then(|p| p.keff.run.as_ref());
        let cfg = run.map_or(self.cfg(), |r| r.cfg);
        let pts: Vec<GenPoint> = self.gens.iter().map(|g| GenPoint::from(&g.gen)).collect();
        let shape = RunShape { n_inactive: cfg.n_inactive, total: cfg.n_inactive + cfg.n_active };
        let refs = [RefLine { label: format!("k_ref {K_REF:.5} (ORNL-4233) ± 1000 pcm band"), k: K_REF, sigma: Some(K_REF * BAND_PCM * 1e-5), colour: keffplot::REFERENCE_COLOUR }];
        let mean = pts.last().and_then(|g| g.k_mean);
        let line = match (run, mean) {
            (None, _) => format!("No run yet. Next: {} × [{} + {}], seed {}", cfg.n_particles, cfg.n_inactive, cfg.n_active, cfg.seed),
            (Some(_), None) => format!("generation {}/{}: inactive, the source is settling", self.gens.len(), shape.total),
            (Some(_), Some((k, s))) => format!("k = {k:.5} ± {s:.5}: {:+.0} ± {:.0} pcm from k_ref · generation {}/{}", 1e5 * (k - K_REF) / (k * K_REF), 1e5 * s / (k * K_REF), self.gens.len(), shape.total),
        };
        let text = 13.0;
        let (plots, pane, strip) = keffplot::body_layout(rect);
        keffplot::draw_strip(painter, strip, line, bugis::Ink::of_ui(ui).text, text);
        let ceiling = run.and_then(|r| r.entropy_bins()).map(|b| (b as f64).log2());
        keffplot::draw_pair(painter, plots, &pts, shape, &refs, ceiling, text * 0.92);
        let warn = ui.visuals().warn_fg_color;
        let status: Vec<StatusLine> = vec![(
            "The model's default sits +1726 ± 23 pcm above k_ref, outside the ±1000 pcm band (V&V record, 2026-11-13); what is left is open (gh:#1128). Not validated.".to_string(),
            warn,
        )];
        let console: Vec<String> = self.pool.as_ref().map(|p| p.keff.log.clone()).unwrap_or_default();
        keffplot::draw_text_pane(ui, painter, pane, &status, &console, "", text * 0.9, "msre-console");
    }

    /// The right dock: the three rods as columns against the graphite, by
    /// the library's drawing (`rod_bank::draw_bank`), on the model's own
    /// axis (z of the stringer bottom at 0, up). GUI drawing code; the
    /// positions are `m::rod_lower_end_z`'s, which is tested.
    fn rods_ui(&mut self, ui: &mut egui::Ui, rect: Rect) {
        let painter = ui.painter_at(rect);
        let d = m::Dims::at(self.study.salt_f, self.study.graphite_f);
        let length = m::ROD_POISON_LENGTH_IN * m::INCH * d.graphite;
        let spans: Vec<(f64, f64)> = (0..3)
            .map(|i| {
                let lo = m::rod_lower_end_z(m::rod_position_in(self.rods.rods[i]), d);
                // The drawing's depth runs down: depth = top − z.
                (-(lo + length), -lo)
            })
            .collect();
        let top = m::H_CORE * d.graphite;
        let style = BankStyle {
            title: if self.rods_on { "Rods (side): poison, graphite in grey".into() } else { "Rods not modelled (the record): shown where they would be".into() },
            depth_top: -(top + 70.0),
            depth_bottom: 20.0,
            core: (-top, 0.0),
            core_label: "graphite".into(),
            absorber: Color32::from_rgb(110, 110, 120),
            core_colour: Color32::from_rgb(70, 70, 70),
        };
        rod_bank::draw_bank(&painter, rect.shrink(6.0), &style, &spans, self.rods.selected, 13.0);
    }

    fn geometry_ui(&mut self, ui: &mut egui::Ui, rect: Rect, zoom_clear: f32) {
        ui.scope_builder(egui::UiBuilder::new().max_rect(Rect::from_min_size(rect.min, Vec2::new(rect.width(), 30.0))), |ui| {
            ui.horizontal(|ui| {
                ui.selectable_value(&mut self.view, View::Plan, "Plan (x-y)");
                ui.selectable_value(&mut self.view, View::Elevation, "Elevation (x-z)");
            });
        });
        let Some((s, v, w, h, ids)) = &self.raster else {
            let area = Rect::from_min_max(rect.min + Vec2::new(0.0, 34.0), rect.max);
            ui.painter().text(area.center(), Align2::CENTER_CENTER, "Drawing the geometry…", FontId::proportional(15.0), ui.visuals().text_color());
            return;
        };
        let plan = s.plan();
        // Legend first: the materials present, wrapped by each label's own
        // width, so the drawing gets what is left (on a phone it wraps to
        // five rows, which a fixed allowance let the caption overlap).
        let mut items = Vec::new();
        let (mut x, mut row) = (0.0_f32, 0_usize);
        for (i, (name, _)) in plan.materials.iter().enumerate() {
            let galley = ui.painter().layout_no_wrap(name.to_string(), FontId::proportional(11.0), ui.visuals().text_color());
            let wdt = 14.0 + galley.size().x + 10.0;
            if x + wdt > rect.width() && x > 0.0 {
                x = 0.0;
                row += 1;
            }
            items.push((x, row, colour(i as u8 + 1), galley));
            x += wdt;
        }
        let legend_h = 15.0 * (row + 1) as f32;
        let legend_top = rect.bottom() - legend_h;
        // 34 px of view buttons above, 20 px of caption below the drawing.
        let area = Rect::from_min_max(rect.min + Vec2::new(0.0, 34.0), Pos2::new(rect.right(), (legend_top - 20.0).max(rect.top() + 60.0)));
        if self.texture.as_ref().map(|t| (t.1, t.2)) != Some((*s, *v)) {
            let px: Vec<Color32> = ids.iter().map(|&i| colour(i)).collect();
            let img = egui::ColorImage { size: [*w, *h], pixels: px, source_size: Vec2::new(*w as f32, *h as f32) };
            self.texture = Some((ui.ctx().load_texture("msre_geometry", img, egui::TextureOptions::NEAREST), *s, *v));
        }
        if let Some((t, ..)) = &self.texture {
            // Zoom and pan with the shared view (gh:#1661): the drawing's
            // centre is the world origin, in cm.
            let (_, _, width) = v.frame(s);
            self.geo_view.half_extent = 0.5 * width[0].max(width[1]);
            let resp = ui.interact(area, ui.id().with("msre-geometry"), egui::Sense::click_and_drag());
            self.geo_view.handle_input(ui, &resp);
            let (hw, hh) = (0.5 * width[0], 0.5 * width[1]);
            let r = Rect::from_two_pos(self.geo_view.to_screen(area, -hw, hh), self.geo_view.to_screen(area, hw, -hh));
            let painter = ui.painter_at(area);
            painter.image(t.id(), r, Rect::from_min_max(Pos2::ZERO, Pos2::new(1.0, 1.0)), Color32::WHITE);
            ui.painter().text(area.left_bottom() + Vec2::new(0.0, 3.0), Align2::LEFT_TOP, format!("{:.0} cm across · drawn from the assembled CSG", width[0]), FontId::proportional(12.0), ui.visuals().text_color());
            let zoom_rect = Rect::from_min_max(Pos2::new(area.left(), area.top() + zoom_clear), area.max);
            if let Some(z) = bugis::web_demo::view::zoom_buttons(ui, zoom_rect) {
                bugis::web_demo::view::apply_zoom(&mut self.geo_view, area, z);
            }
        }
        for (x, row, c, galley) in items {
            let (x, y) = (rect.left() + x, legend_top + 15.0 * row as f32);
            ui.painter().rect_filled(Rect::from_min_size(Pos2::new(x, y + 2.0), Vec2::splat(10.0)), 1.0, c);
            ui.painter().galley(Pos2::new(x + 14.0, y), galley, ui.visuals().text_color());
        }
    }
}

/// The pool's line while the nuclear data are processed (gh:#1578: the
/// first version read "Nuclear data: −0 % (no nuclear data)" for the
/// minutes before the first job finished, which looked like missing data).
/// GUI text.
fn processing_line(p: &Pool) -> String {
    let n = p.data.labels.len();
    let done = p.data.secs.iter().filter(|s| s.is_some()).count();
    let now: Vec<&str> = (0..n).filter(|&j| p.data.started[j] && p.data.secs[j].is_none()).map(|j| p.data.labels[j].as_str()).collect();
    let now_text = if now.is_empty() { "starting".to_string() } else { now.iter().take(4).copied().collect::<Vec<_>>().join(", ") + if now.len() > 4 { ", ..." } else { "" } };
    if done == 0 {
        format!(
            "Processing the nuclear data on {} workers: 0 of {n} jobs done, now {now_text}. A first visit processes ENDF/B-VIII.0 in this browser and takes several minutes (U-235 and U-238 are the longest); a later visit reads the results back from this browser's cache.",
            p.links.len()
        )
    } else {
        format!("Nuclear data: {:.0} % by cost, {done} of {n} jobs, on {} workers ({}); now {now_text}", 100.0 * p.data.processed(), p.links.len(), p.data.data_source())
    }
}

/// How much longer a phone takes than the desktop cost estimates: U-235
/// about 500 s on the maintainer's phone (2026-11-15) against the 58 s
/// estimate. An ESTIMATE from one measurement; only the progress bar uses it.
const PHONE_SLOWDOWN: f64 = 500.0 / 58.0;

/// A job's name for the loading card (its nuclide, or its law).
fn job_label(j: &Job) -> &'static str {
    match j {
        Job::Nuclide(n) => n.name,
        Job::Graphite(_) => "graphite S(a,b)",
        Job::Water => "H in H2O S(a,b)",
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
