//! **The EBR-II simulator's MC page** (gh:#1762): the simplified full core
//! (`nee_soon::ebr2_core`: Run 138B's 637-position layout, fresh fuel, every
//! subassembly homogenised) on a pool of browser workers, laid out in the
//! standard twin shell: the study and run settings in the left panel, the
//! geometry drawn from the assembled CSG in the central view (zoom in, out,
//! reset from the shell), k and the Shannon entropy in the bottom dock
//! (`dhoby_ghaut::mc_web::keffplot`). After MSRE's k_eff tab
//! (`examples/msre_web/keff_tab.rs`, gh:#1571/#1661); the pool and workers
//! are the library's (`ebr2::web::{keff, keff_pool}`).
//!
//! **A SIMPLIFIED FRESH CORE, not the Run 138B burnup state** (maintainer's
//! option A, gh:#1762): its `k` is not a prediction of EBR-II's critical
//! state, and the page says so wherever it shows one.
//!
//! GUI drawing and tab-driving code: reached by no test (it needs a window
//! and workers), per the workspace exception for GUI code. What it drives is
//! tested in `ebr2::web::keff` (the jobs, the wire, the raster of the
//! assembled core) and `nee_soon::ebr2_core` (the table and the lattice).

use bugis::web_demo::link::Link;
use bugis::web_demo::loading::Loading;
use dhoby_ghaut::mc_web::keff::KeffConfig;
use dhoby_ghaut::mc_web::keffplot::{self, GenPoint, RunShape, StatusLine};
use dhoby_ghaut::mc_web::pool::{GenSummary, Note};
use dhoby_ghaut::mc_web::run_settings::{run_settings_ui, RunPresets};
use egui::{Align2, Color32, FontId, Pos2, Rect, RichText, Vec2};
use nee_soon::ebr2_core::{self as c, Study};
use outram_park_digital_twin_engine::ebr2::web::keff::{jobs, KReq, View};
use outram_park_digital_twin_engine::ebr2::web::keff_pool::{plan_workers, preset_cfg, Finished, Phase, Pool, PRESETS};
use outram_park_digital_twin_engine::ebr2::web::{Event, Request};

/// The label every `k` of this page carries.
pub const FRESH: &str = "simplified fresh core, not the Run 138B burnup state";

/// The page's state.
pub struct KeffTab {
    pub study: Study,
    preset: usize,
    run_cfg: KeffConfig,
    pool: Option<Pool>,
    raster: Option<(Study, View, usize, usize, Vec<u8>)>,
    asked: Option<(Study, View)>,
    pub view: View,
    texture: Option<(egui::TextureHandle, Study, View)>,
    loading: Option<(Study, Loading)>,
    pub finished: Vec<Finished>,
    gens: Vec<GenSummary>,
    error: Option<String>,
    /// The geometry's zoom and pan.
    pub geo_view: bugis::web_demo::view::View,
    /// The geometry's drawing rect last frame (for the shell's zoom).
    pub geo_rect: Rect,
    /// `?krun`: start a run on load (an unattended browser check, as
    /// MSRE's page has).
    autorun: bool,
}

impl Default for KeffTab {
    fn default() -> Self {
        Self {
            study: Study::default(),
            preset: 0,
            // `?kcfg=n,inactive,active,seed` in place of the first preset
            // (a browser run checked against a native one).
            run_cfg: query("kcfg")
                .and_then(|v| {
                    let n: Vec<u64> = v.split(',').filter_map(|x| x.trim().parse().ok()).collect();
                    (n.len() == 4).then(|| KeffConfig { n_particles: n[0] as usize, n_inactive: n[1] as usize, n_active: n[2] as usize, seed: n[3], point_source: false, want_sites: false })
                })
                .unwrap_or(preset_cfg(0)),
            pool: None,
            raster: None,
            asked: None,
            view: View::Core,
            texture: None,
            loading: None,
            finished: Vec::new(),
            gens: Vec::new(),
            error: None,
            geo_view: bugis::web_demo::view::View::new(1.0),
            geo_rect: Rect::NOTHING,
            autorun: query("krun").is_some(),
        }
    }
}

fn query(key: &str) -> Option<String> {
    bugis::web_demo::platform::query_pairs().into_iter().find(|(k, _)| k == key).map(|(_, v)| v)
}

impl KeffTab {
    /// A run is going, or waiting for its data.
    pub fn busy(&self) -> bool {
        self.pool.as_ref().is_some_and(|p| p.pending.is_some() || p.keff.going() || p.data_phase())
    }

    /// Run k_eff on the page's study: on the pool that holds its data, or a
    /// new pool (the old one stopped).
    pub fn run(&mut self, ctx: &egui::Context) {
        self.error = None;
        self.gens.clear();
        let study = self.study;
        let reuse = self.pool.as_ref().is_some_and(|p| p.study == study && !matches!(p.phase, Phase::Failed(_)));
        if !reuse {
            if let Some(p) = self.pool.take() {
                p.terminate();
            }
            match Pool::start(ctx, study, plan_workers(query("workers").and_then(|v| v.parse().ok()))) {
                Ok(p) => self.pool = Some(p),
                Err(e) => {
                    self.error = Some(e);
                    return;
                }
            }
        }
        if let Some(p) = self.pool.as_mut() {
            p.pending = Some(self.run_cfg);
        }
    }

    /// Stop: a run stops (the data stay); during the data phase the pool
    /// closes (finished products are in this browser's cache).
    pub fn stop(&mut self) {
        if self.pool.as_ref().is_some_and(|p| p.data_phase()) {
            if let Some(p) = self.pool.take() {
                p.terminate();
            }
            return;
        }
        if let Some(p) = self.pool.as_mut() {
            p.pending = None;
            p.keff.stop_run();
        }
    }

    /// Each frame: pump the pool, keep finished runs, ask the plant worker
    /// for the geometry when it is not drawn for the current settings.
    pub fn pump(&mut self, ctx: &egui::Context, plant: &Link<Request, Event>) {
        let now = bugis::web_demo::platform::now_s();
        if self.autorun {
            self.autorun = false;
            self.run(ctx);
        }
        if self.asked != Some((self.study, self.view)) && self.raster.as_ref().map(|r| (r.0, r.1)) != Some((self.study, self.view)) {
            plant.send(Request::Keff(KReq::Raster { study: self.study, view: self.view, px: 480 }));
            self.asked = Some((self.study, self.view));
        }
        let cfg = self.run_cfg;
        if let Some(p) = self.pool.as_mut() {
            for n in p.pump(now) {
                match n {
                    Note::Generation(g) => self.gens.push(g.without_tracks()),
                    Note::RunDone => {
                        if let Some((k, s)) = p.keff.run.as_ref().and_then(|r| r.it.as_ref()).and_then(|it| it.k_mean()) {
                            #[cfg(target_arch = "wasm32")]
                            log::info!("ebr2 k_eff run done: {} K, control {:.4}: k = {k:e} +- {s:e}", p.study.temperature_k, p.study.control_insertion);
                            self.finished.push(Finished { study: p.study, cfg: p.keff.run.as_ref().map_or(cfg, |r| r.cfg), k, sigma: s });
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

    fn sync_loading(&mut self) {
        let Some(p) = self.pool.as_ref() else { return };
        if self.loading.as_ref().map(|l| l.0) != Some(p.study) {
            let list = jobs(&p.study);
            self.loading = Some((p.study, Loading::new(list.iter().map(|j| nee_soon::ebr2_core::NUCLIDES[j.index].0).collect(), list.iter().map(|j| j.cost()).collect())));
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

    /// The left panel's controls. GUI drawing code.
    pub fn panel(&mut self, ui: &mut egui::Ui) {
        ui.heading("Full-core Monte Carlo");
        ui.label(RichText::new(format!("EBR-II, Run 138B's 637 positions, {FRESH}: fresh U-5Fs drivers, every subassembly homogenised at ARC-228's volume fractions (several fractions ASSUMED; see the model's docs). ENDF/B-VIII.0 processed in your browser on a pool of workers.")).small());
        let busy = self.busy();
        ui.add_enabled_ui(!busy, |ui| {
            ui.add(egui::Slider::new(&mut self.study.temperature_k, 293.15..=900.0).text("temperature, K (Doppler only)"));
            let mut drop_cm = self.study.control_insertion * c::STROKE_CM;
            if ui.add(egui::Slider::new(&mut drop_cm, 0.0..=c::STROKE_CM).text("control rods' fuel drop, cm")).changed() {
                self.study.control_insertion = drop_cm / c::STROKE_CM;
            }
            ui.label(RichText::new("Lowering a high-worth rod moves its fuel out of the core and its B4C in (ARC-228 p.19). SHRT-45R started at 21.06 cm.").small());
            if ui.button("Back to the default (293 K, SHRT-45R rods)").clicked() {
                self.study = Study::default();
            }
            ui.separator();
            let before = self.preset;
            egui::ComboBox::from_label("preset").selected_text(PRESETS[self.preset].0).show_ui(ui, |ui| {
                for (i, p) in PRESETS.iter().enumerate() {
                    ui.selectable_value(&mut self.preset, i, format!("{}: {} × [{} + {}]", p.0, p.1, p.2, p.3));
                }
            });
            if self.preset != before {
                self.run_cfg = preset_cfg(self.preset);
            }
            run_settings_ui(ui, "ebr2_cfg", &mut self.run_cfg, 0, &RunPresets { defaults: preset_cfg(0), recorded: None }, busy);
        });
        ui.horizontal(|ui| {
            if busy {
                if ui.add(egui::Button::new(RichText::new("■ Stop").size(18.0)).min_size(Vec2::new(110.0, 40.0))).clicked() {
                    self.stop();
                }
            } else if ui.add(egui::Button::new(RichText::new("▶ Run k_eff").size(18.0)).min_size(Vec2::new(130.0, 40.0))).clicked() {
                self.run(ui.ctx());
            }
        });
        if let Some(e) = &self.error {
            ui.colored_label(ui.visuals().error_fg_color, e);
        }
        if let Some(p) = &self.pool {
            let line = match &p.phase {
                Phase::Processing => format!("Processing the nuclear data on {} workers: {:.0} % by cost. A first visit takes several minutes; a later one reads this browser's cache.", p.links.len(), 100.0 * p.data.processed()),
                Phase::Assembling => format!("Building the model on {} workers", p.links.len()),
                Phase::Ready => format!("{} workers hold the model ({})", p.keff.live_workers(), p.data.data_source()),
                Phase::Failed(m) => format!("Failed: {m}"),
            };
            ui.label(RichText::new(line).small());
            if let Some((_, l)) = self.loading.as_ref() {
                egui::CollapsingHeader::new(format!("Nuclear-data jobs ({} of {} done)", l.done(), l.labels.len())).default_open(false).show(ui, |ui| l.grid(ui));
            }
        }
        if !self.finished.is_empty() {
            ui.separator();
            ui.label(RichText::new("Runs this session").strong());
            for f in self.finished.iter().rev().take(6) {
                ui.label(RichText::new(format!("{:.0} K, rods {:.1} cm: k = {:.5} ± {:.5} ({FRESH})", f.study.temperature_k, f.study.control_insertion * c::STROKE_CM, f.k, f.sigma)).small());
            }
        }
    }

    /// The bottom dock: k and its running mean, the source entropy, the
    /// status. No reference line: this core has no measured k to stand
    /// against. GUI drawing code.
    pub fn keff_body(&self, ui: &mut egui::Ui, rect: Rect) {
        let painter = ui.painter_at(rect);
        let rect = rect.shrink2(Vec2::new(8.0, 4.0));
        let run = self.pool.as_ref().and_then(|p| p.keff.run.as_ref());
        let cfg = run.map_or(self.run_cfg, |r| r.cfg);
        let pts: Vec<GenPoint> = self.gens.iter().map(|g| GenPoint::from(&g.gen)).collect();
        let shape = RunShape { n_inactive: cfg.n_inactive, total: cfg.n_inactive + cfg.n_active };
        let line = match (run, pts.last().and_then(|g| g.k_mean)) {
            (None, _) => format!("No run yet. Next: {} × [{} + {}], seed {}", cfg.n_particles, cfg.n_inactive, cfg.n_active, cfg.seed),
            (Some(_), None) => format!("generation {}/{}: inactive, the source is settling", self.gens.len(), shape.total),
            (Some(_), Some((k, s))) => format!("k = {k:.5} ± {s:.5} ({FRESH}) · generation {}/{}", self.gens.len(), shape.total),
        };
        let text = 13.0;
        let (plots, pane, strip) = keffplot::body_layout(rect);
        keffplot::draw_strip(&painter, strip, line, bugis::Ink::of_ui(ui).text, text);
        let ceiling = run.and_then(|r| r.entropy_bins()).map(|b| (b as f64).log2());
        keffplot::draw_pair(&painter, plots, &pts, shape, &[], ceiling, text * 0.92);
        let status: Vec<StatusLine> = vec![(format!("A {FRESH}: fresh fuel puts k above the real core's. Not validated."), ui.visuals().warn_fg_color)];
        let console: Vec<String> = self.pool.as_ref().map(|p| p.keff.log.clone()).unwrap_or_default();
        keffplot::draw_text_pane(ui, &painter, pane, &status, &console, "", text * 0.9, "ebr2-console");
    }

    /// The central view: the view picker, the geometry from the assembled
    /// CSG with its legend, and the loading card while the data are
    /// processed. GUI drawing code.
    pub fn geometry_ui(&mut self, ui: &mut egui::Ui, rect: Rect) {
        ui.scope_builder(egui::UiBuilder::new().max_rect(Rect::from_min_size(rect.min, Vec2::new(rect.width(), 30.0))), |ui| {
            ui.horizontal(|ui| {
                for v in View::ALL {
                    ui.selectable_value(&mut self.view, v, v.label());
                }
            });
        });
        let mut items = Vec::new();
        let (mut x, mut row) = (0.0_f32, 0_usize);
        for (i, name) in c::MATERIALS.iter().enumerate() {
            let galley = ui.painter().layout_no_wrap(name.to_string(), FontId::proportional(11.0), ui.visuals().text_color());
            let wdt = 14.0 + galley.size().x + 10.0;
            if x + wdt > rect.width() && x > 0.0 {
                x = 0.0;
                row += 1;
            }
            items.push((x, row, colour(i as u8 + 1), galley));
            x += wdt;
        }
        let legend_top = rect.bottom() - 15.0 * (row + 1) as f32;
        let area = Rect::from_min_max(rect.min + Vec2::new(0.0, 34.0), Pos2::new(rect.right(), (legend_top - 20.0).max(rect.top() + 60.0)));
        self.geo_rect = area;
        match &self.raster {
            None => {
                ui.painter().text(area.center(), Align2::CENTER_CENTER, "Drawing the core…", FontId::proportional(15.0), ui.visuals().text_color());
            }
            Some((s, v, w, h, ids)) => {
                if self.texture.as_ref().map(|t| (t.1, t.2)) != Some((*s, *v)) {
                    let px: Vec<Color32> = ids.iter().map(|&i| colour(i)).collect();
                    let img = egui::ColorImage { size: [*w, *h], pixels: px, source_size: Vec2::new(*w as f32, *h as f32) };
                    self.texture = Some((ui.ctx().load_texture("ebr2_core", img, egui::TextureOptions::NEAREST), *s, *v));
                }
                if let Some((t, ..)) = &self.texture {
                    let (_, _, width) = v.frame();
                    self.geo_view.half_extent = 0.5 * width[0].max(width[1]);
                    let resp = ui.interact(area, ui.id().with("ebr2-geometry"), egui::Sense::click_and_drag());
                    self.geo_view.handle_input(ui, &resp);
                    let (hw, hh) = (0.5 * width[0], 0.5 * width[1]);
                    let r = Rect::from_two_pos(self.geo_view.to_screen(area, -hw, hh), self.geo_view.to_screen(area, hw, -hh));
                    ui.painter_at(area).image(t.id(), r, Rect::from_min_max(Pos2::ZERO, Pos2::new(1.0, 1.0)), Color32::WHITE);
                    ui.painter().text(area.left_bottom() + Vec2::new(0.0, 3.0), Align2::LEFT_TOP, format!("{:.0} cm across · drawn from the assembled CSG · {FRESH}", width[0]), FontId::proportional(12.0), ui.visuals().text_color());
                }
            }
        }
        for (x, row, col, galley) in items {
            let (x, y) = (rect.left() + x, legend_top + 15.0 * row as f32);
            ui.painter().rect_filled(Rect::from_min_size(Pos2::new(x, y + 2.0), Vec2::splat(10.0)), 1.0, col);
            ui.painter().galley(Pos2::new(x + 14.0, y), galley, ui.visuals().text_color());
        }
        if let (Some(p), Some((_, l))) = (self.pool.as_ref(), self.loading.as_ref()) {
            if p.data_phase() {
                let title = format!("Processing ENDF/B-VIII.0 on {} workers", p.links.len());
                l.card(ui.painter(), area, &title, Some("First visit: several minutes in this browser (U-235 and U-238 are the longest); a later visit reads this browser's cache."));
            }
        }
    }
}

/// A material's colour (index + 1; 0 void, 254 overlap, 255 outside), as
/// `examples/ebr2_core_images.rs` draws them. Indicative, no physical meaning.
fn colour(id: u8) -> Color32 {
    const P: [Color32; 7] = [
        Color32::from_rgb(220, 60, 40),
        Color32::from_rgb(240, 150, 60),
        Color32::from_rgb(200, 100, 160),
        Color32::from_rgb(30, 30, 30),
        Color32::from_rgb(90, 140, 90),
        Color32::from_rgb(160, 160, 200),
        Color32::from_rgb(60, 130, 200),
    ];
    match id {
        0 => Color32::from_rgb(245, 245, 245),
        254 => Color32::from_rgb(255, 0, 255),
        255 => Color32::from_rgb(20, 20, 28),
        i => P[(i as usize - 1) % P.len()],
    }
}
