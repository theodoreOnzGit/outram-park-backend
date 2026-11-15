//! **The Core k_eff tab's page** (gh:#1571): the reader's controls, the
//! geometry drawn from the assembled CSG (the drawing rule), k per
//! generation with its running mean, the measured critical state k_ref =
//! 1.00080 (ORNL-4233) and the maintainer's ±1000 pcm band. **The model's
//! default sits outside that band (+1726 ± 23 pcm, the V&V record): the tab
//! shows that, it does not hide it.**
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
use dhoby_ghaut::mc_web::keff::KeffConfig;
use dhoby_ghaut::mc_web::pool::{GenSummary, Note};
use egui::{Align2, Color32, FontId, Pos2, Rect, RichText, Stroke, Vec2};
use egui_plot::{HLine, Legend, Line, Plot, PlotPoints, Points};
use outram_park_digital_twin_engine::msre::first_criticality::Study;
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
            loading: None,
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
            automeasure: query("kmeasure").is_some(),
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
    /// The settings of the chosen preset.
    pub fn cfg(&self) -> KeffConfig {
        if let Some(c) = self.custom {
            return c;
        }
        preset_cfg(self.preset)
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
            self.loading = Some((p.study, Loading::new(labels, list.iter().map(Job::cost).collect())));
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
        // While the nuclear data are processed, the card over the plot (as
        // the HTR-10 demo's), so it shows with the panel folded on a phone.
        if let (Some(p), Some((_, l))) = (self.pool.as_ref(), self.loading.as_ref()) {
            if p.data_phase() {
                let note = if p.data.secs.iter().any(Option::is_some) {
                    format!("First visit: several minutes in this browser (U-235 and U-238 are the longest); a later visit reads the results from this browser's cache. So far: {}.", p.data.data_source())
                } else {
                    "First visit: several minutes in this browser (U-235 and U-238 are the longest); a later visit reads the results from this browser's cache.".to_string()
                };
                let title = format!("Processing ENDF/B-VIII.0 on {} workers", p.links.len());
                l.card(ui.painter(), plot, &title, Some(&note));
            }
        }
    }

    fn geometry_ui(&mut self, ui: &mut egui::Ui, rect: Rect) {
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
            let aspect = *h as f32 / *w as f32;
            let side = area.width().min(area.height() / aspect);
            let r = Rect::from_center_size(area.center(), Vec2::new(side, side * aspect));
            ui.painter().image(t.id(), r, Rect::from_min_max(Pos2::ZERO, Pos2::new(1.0, 1.0)), Color32::WHITE);
            let (_, _, width) = v.frame(s);
            ui.painter().text(r.left_bottom() + Vec2::new(0.0, 3.0), Align2::LEFT_TOP, format!("{:.0} cm across · drawn from the assembled CSG", width[0]), FontId::proportional(12.0), ui.visuals().text_color());
        }
        for (x, row, c, galley) in items {
            let (x, y) = (rect.left() + x, legend_top + 15.0 * row as f32);
            ui.painter().rect_filled(Rect::from_min_size(Pos2::new(x, y + 2.0), Vec2::splat(10.0)), 1.0, c);
            ui.painter().galley(Pos2::new(x + 14.0, y), galley, ui.visuals().text_color());
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
