//! The egui front end of the nuclear data demo: one app, a rung at a time.
//!
//! The UI never does physics: it sends requests to the engine (a thread, or a
//! Web Worker in the browser) through `dhoby_ghaut::web_demo::link`, drains
//! what came back each frame, and draws. The layout is the shared
//! mobile-first one: the plot fills the page with + / − / Reset on it, every
//! control is in the folding side panel.

#[cfg(not(target_arch = "wasm32"))]
use crate::engine::NdEngine;
use crate::engine::{Curve, Event, Request, BROADR_SHOWN, RECONR_TOLS};
use crate::plot::{self, Frame};
use crate::rungs::Rung;
use dhoby_ghaut::web_demo::lesson::{self, Rung as _};
use dhoby_ghaut::web_demo::link::Link;
use dhoby_ghaut::web_demo::loading::Loading;
use dhoby_ghaut::web_demo::panel::Panel;
use dhoby_ghaut::web_demo::platform::{now_s, set_query, set_title};
use dhoby_ghaut::web_demo::view::{apply_zoom, zoom_buttons, View};
use egui::{Color32, Pos2, RichText, Sense, Stroke};

const BG: Color32 = Color32::from_rgb(14, 16, 20);
const TOTAL: Color32 = Color32::from_rgb(120, 170, 255);
const ELASTIC: Color32 = Color32::from_rgb(120, 210, 150);
const CAPTURE: Color32 = Color32::from_rgb(255, 150, 80);
const FAINT: Color32 = Color32::from_rgb(90, 96, 110);
const BANDS: Color32 = Color32::from_rgba_premultiplied(230, 200, 90, 200);
const FREE: Color32 = Color32::from_rgb(200, 200, 200);

enum Phase {
    Loading(Loading),
    Ready,
    Failed(String),
}

pub struct NdApp {
    link: Option<Link<Request, Event>>,
    panel: Panel,
    view: View,
    frame: Frame,
    /// The view is re-framed on the first result of a new rung only, so later
    /// results (a finer RECONR, a new temperature) do not make it jump.
    framed: bool,
    rung: Rung,
    phase: Phase,
    load_id: u32,
    /// Results carry the job they answer; older jobs' results are dropped.
    job: u32,
    in_flight: bool,
    /// The newest request asked for while one was in flight (coalesced).
    pending: Option<Request>,
    curves: Vec<Curve>,
    note: String,
    secs: f64,
    computing_since: Option<f64>,
    /// RECONR rung: the next tolerance to ask for, whether the sequence runs,
    /// and `(tolerance, points, seconds)` per finished step.
    reconr_next: usize,
    reconr_running: bool,
    reconr_log: Vec<(f64, usize, f64)>,
    broadr_t: f64,
    purr_t: f64,
    purr_ladders: usize,
    thermr_mat: u8,
    thermr_t: f64,
    thermr_e: f64,
    groupr_n: usize,
    groupr_s0: f64,
    text_size: f32,
    title: String,
}

impl NdApp {
    pub fn new(cc: &eframe::CreationContext<'_>, rung: Rung) -> Self {
        cc.egui_ctx.set_theme(egui::ThemePreference::Dark);
        #[cfg(not(target_arch = "wasm32"))]
        let link = {
            let ctx = cc.egui_ctx.clone();
            Ok::<_, String>(dhoby_ghaut::web_demo::link::start_native(NdEngine::default(), move || ctx.request_repaint()))
        };
        #[cfg(target_arch = "wasm32")]
        let link = dhoby_ghaut::web_demo::link::start_web(cc.egui_ctx.clone(), "./worker.js", Event::Error);
        let mut app = Self {
            link: None,
            panel: Panel::default(),
            view: View::new(6.0),
            frame: Frame::default(),
            framed: false,
            rung,
            phase: Phase::Failed(String::new()),
            load_id: 0,
            job: 0,
            in_flight: false,
            pending: None,
            curves: Vec::new(),
            note: String::new(),
            secs: 0.0,
            computing_since: None,
            reconr_next: 0,
            reconr_running: false,
            reconr_log: Vec::new(),
            broadr_t: 293.6,
            purr_t: 293.6,
            purr_ladders: 8,
            thermr_mat: 0,
            thermr_t: 296.0,
            thermr_e: 0.0253,
            groupr_n: 30,
            groupr_s0: 50.0,
            text_size: 14.0,
            title: String::new(),
        };
        match link {
            Ok(l) => {
                app.link = Some(l);
                app.switch(rung);
            }
            Err(e) => app.phase = Phase::Failed(format!("could not start the physics worker: {e}")),
        }
        app
    }

    /// Go to a rung: load its tapes (instant when the engine holds them).
    fn switch(&mut self, rung: Rung) {
        self.rung = rung;
        set_query(&[("rung", rung.name())]);
        self.job += 1;
        self.in_flight = false;
        self.pending = None;
        self.curves.clear();
        self.note.clear();
        self.framed = false;
        self.reconr_running = false;
        self.reconr_next = 0;
        self.reconr_log.clear();
        self.computing_since = None;
        let tapes = rung.info().tapes;
        if tapes.is_empty() {
            self.phase = Phase::Ready;
            return;
        }
        self.load_id += 1;
        // Weights only shape the progress bar: U-238 is the slow one.
        let weights = tapes.iter().map(|t| if t.contains("U_238") { 3.0 } else { 1.0 }).collect();
        self.phase = Phase::Loading(Loading::new(tapes.to_vec(), weights));
        self.send(Request::Load { id: self.load_id, rung });
    }

    fn send(&self, r: Request) {
        if let Some(l) = &self.link {
            l.send(r);
        }
    }

    /// Ask for a step now, or after the one in flight (only the newest kept).
    fn ask(&mut self, r: Request) {
        if self.in_flight {
            self.pending = Some(r);
        } else {
            self.in_flight = true;
            self.computing_since = Some(now_s());
            self.send(r);
        }
    }

    /// The rung's first request once its data are in.
    fn start_rung(&mut self) {
        let job = self.job;
        match self.rung {
            Rung::Endf => self.ask(Request::Sections { job }),
            Rung::Reconr => {
                self.reconr_running = true;
                self.reconr_next = 1;
                self.ask(Request::Reconr { job, tol: RECONR_TOLS[0] });
            }
            Rung::Broadr => self.ask(Request::Broadr { job, temp_k: self.broadr_t }),
            Rung::Purr => self.ask(self.purr_request()),
            Rung::Thermr => self.ask(self.thermr_request()),
            Rung::Groupr => self.ask(self.groupr_request()),
            Rung::Acer => {}
        }
    }
    fn purr_request(&self) -> Request {
        Request::Purr { job: self.job, temp_k: self.purr_t, nladr: self.purr_ladders, nsamp: 2000 }
    }
    fn thermr_request(&self) -> Request {
        Request::Thermr { job: self.job, material: self.thermr_mat, temp_k: self.thermr_t, e_emit: self.thermr_e }
    }
    fn groupr_request(&self) -> Request {
        Request::Groupr { job: self.job, ngroups: self.groupr_n, sigma0: self.groupr_s0 }
    }

    fn handle(&mut self, events: Vec<Event>) {
        for e in events {
            match e {
                Event::Error(m) => {
                    self.in_flight = false;
                    self.computing_since = None;
                    self.reconr_running = false;
                    self.phase = Phase::Failed(m);
                }
                Event::JobStarted { id, index } if id == self.load_id => {
                    if let Phase::Loading(l) = &mut self.phase {
                        l.job_started(index);
                    }
                }
                Event::JobDone { id, index, secs } if id == self.load_id => {
                    if let Phase::Loading(l) = &mut self.phase {
                        l.job_done(index, secs);
                    }
                }
                Event::Ready { id } if id == self.load_id => {
                    self.phase = Phase::Ready;
                    self.start_rung();
                }
                Event::Result { job, param, secs, n_points, curves, note } => {
                    if job != self.job {
                        continue;
                    }
                    self.in_flight = false;
                    self.computing_since = None;
                    self.curves = curves;
                    self.note = note;
                    self.secs = secs;
                    if self.rung == Rung::Reconr {
                        self.reconr_log.push((param, n_points, secs));
                    }
                    if let Some(p) = self.pending.take() {
                        self.ask(p);
                    } else if self.rung == Rung::Reconr && self.reconr_running {
                        if self.reconr_next < RECONR_TOLS.len() {
                            let tol = RECONR_TOLS[self.reconr_next];
                            self.reconr_next += 1;
                            self.ask(Request::Reconr { job: self.job, tol });
                        } else {
                            self.reconr_running = false;
                        }
                    }
                }
                _ => {}
            }
        }
    }

    /// Frame the view on the current curves (sensible fixed ranges per rung,
    /// so the picture does not depend on which step came first).
    fn reframe(&mut self, w: f64, h: f64) {
        let fixed = match self.rung {
            Rung::Reconr => Some((1.0e-5, 3.0e7, 1.0e-2, 1.0e5)),
            Rung::Broadr => Some((BROADR_SHOWN.0, BROADR_SHOWN.1, 1.0e-2, 3.0e4)),
            Rung::Groupr => Some((1.0, 1.0e4, 1.0e-3, 3.0e4)),
            Rung::Thermr => Some((1.0e-5, 5.0, 1.0e-1, 1.0e3)),
            _ => None,
        };
        let b = fixed.or_else(|| {
            let sets: Vec<&[[f64; 2]]> = self.curves.iter().map(|c| c.pts.as_slice()).collect();
            plot::bounds(&sets).map(|(a, b, c, d)| (a, b, c / 1.3, d * 1.3))
        });
        if let Some((x0, x1, y0, y1)) = b {
            let (f, half) = Frame::around(x0, x1, y0, y1, w, h);
            self.frame = f;
            self.view = View::new(half);
            self.framed = true;
        }
    }

    fn title_now(&mut self, ctx: &egui::Context) {
        let t = match &self.phase {
            Phase::Loading(l) => format!("{} · loading · {}", self.rung.title(), l.status_line()),
            Phase::Failed(_) => format!("{} · failed", self.rung.title()),
            Phase::Ready if self.in_flight => format!("{} · computing", self.rung.title()),
            Phase::Ready => format!("{} · nuclear data demo", self.rung.title()),
        };
        if t != self.title {
            set_title(ctx, &t);
            self.title = t;
        }
    }
}

impl eframe::App for NdApp {
    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        let ctx = ui.ctx().clone();
        let events = self.link.as_ref().map(|l| l.drain()).unwrap_or_default();
        self.handle(events);
        if matches!(self.phase, Phase::Loading(_)) || self.in_flight {
            ctx.request_repaint_after(std::time::Duration::from_millis(100));
        }
        self.title_now(&ctx);
        let mut panel = std::mem::take(&mut self.panel);
        panel.show(ui, "Nuclear data", |ui| self.side_panel(ui));
        self.panel = panel;
        egui::CentralPanel::default().frame(egui::Frame::NONE).show(ui, |ui| self.main_view(ui));
    }
}

// ─── Side panel ──────────────────────────────────────────────────────────────

impl NdApp {
    fn side_panel(&mut self, ui: &mut egui::Ui) {
        let picked = lesson::picker(ui, self.rung);
        if picked != self.rung {
            self.switch(picked);
        }
        ui.strong(self.rung.title());
        lesson::whats_happening(ui, self.rung);
        ui.separator();
        match &self.phase {
            Phase::Loading(l) => {
                ui.horizontal(|ui| {
                    ui.spinner();
                    ui.label(format!("Processing ENDF files · {}", l.status_line()));
                });
                l.grid(ui);
                ui.weak("Real ENDF/B-VIII.0 tapes, covariances removed before download, read by this workspace's NJOY port in a background worker; the page stays live.");
            }
            Phase::Failed(e) => {
                ui.colored_label(Color32::from_rgb(255, 110, 110), format!("Failed: {e}"));
            }
            Phase::Ready => self.controls(ui),
        }
        if !self.note.is_empty() {
            ui.separator();
            ui.label(RichText::new(&self.note).size(12.5));
            if self.secs > 0.0 {
                ui.weak(format!("computed in {:.1} s, off the page's main thread", self.secs));
            }
        }
        ui.separator();
        self.notes(ui);
        ui.add_space(8.0);
        ui.weak("Research, education and V&V only. Not for reactor operation, licensing, safety or emergency response.");
    }

    fn busy_line(&self, ui: &mut egui::Ui) {
        if let Some(t0) = self.computing_since {
            ui.horizontal(|ui| {
                ui.spinner();
                ui.label(format!("computing… {:.0} s", now_s() - t0));
            });
        }
    }

    fn controls(&mut self, ui: &mut egui::Ui) {
        let job = self.job;
        match self.rung {
            Rung::Endf => {
                ui.label("The main view lists every section on the tape: MF (the kind of data) and MT (the reaction), with its number of data lines.");
            }
            Rung::Reconr => {
                ui.label("RECONR runs once per tolerance, coarse to fine; each finished curve replaces the last. Zoom into the resonances to see the grid points (dots) refine.");
                ui.horizontal(|ui| {
                    if self.reconr_running {
                        if ui.button("⏸ Stop after this step").clicked() {
                            self.reconr_running = false;
                        }
                    } else if ui.button("▶ Restart from 0.3").clicked() {
                        self.reconr_log.clear();
                        self.reconr_running = true;
                        self.reconr_next = 1;
                        self.ask(Request::Reconr { job, tol: RECONR_TOLS[0] });
                    }
                    if !self.reconr_running && self.reconr_next < RECONR_TOLS.len() && self.reconr_next > 0 {
                        if ui.button("Next step").clicked() {
                            let tol = RECONR_TOLS[self.reconr_next];
                            self.reconr_next += 1;
                            self.ask(Request::Reconr { job, tol });
                        }
                    }
                });
                self.busy_line(ui);
                egui::Grid::new("reconr_log").num_columns(3).spacing([14.0, 2.0]).show(ui, |ui| {
                    ui.strong("tolerance");
                    ui.strong("points (MT=1)");
                    ui.strong("time");
                    ui.end_row();
                    for (tol, n, s) in &self.reconr_log {
                        ui.label(format!("{tol}"));
                        ui.label(format!("{n}"));
                        ui.label(format!("{s:.1} s"));
                        ui.end_row();
                    }
                });
                ui.weak("The 0.001 step is NJOY's default tolerance and the one the lessons' records use; on a phone it takes a while (the page stays live).");
            }
            Rung::Broadr => {
                let r = ui.add(egui::Slider::new(&mut self.broadr_t, 0.0..=3000.0).text("temperature (K)"));
                if r.changed() {
                    let t = self.broadr_t;
                    self.ask(Request::Broadr { job, temp_k: t });
                }
                self.busy_line(ui);
                ui.label("Faint: 0 K. Bright: broadened. Watch the 6.67, 20.9 and 36.7 eV capture peaks fall and widen while the area under them (in the note below) stays put.");
            }
            Rung::Purr => {
                let mut changed = ui.add(egui::Slider::new(&mut self.purr_t, 1.0..=2000.0).logarithmic(true).text("temperature (K)")).drag_stopped();
                changed |= ui.add(egui::Slider::new(&mut self.purr_ladders, 2..=32).text("resonance ladders")).drag_stopped();
                if changed {
                    let r = self.purr_request();
                    self.ask(r);
                }
                self.busy_line(ui);
                ui.label("Each dot is one probability band of U-238's total cross section in the unresolved range (20-149 keV), as a factor on the dilute cross section; its size is the band's probability.");
            }
            Rung::Thermr => {
                let mut changed = false;
                ui.horizontal(|ui| {
                    changed |= ui.selectable_value(&mut self.thermr_mat, 0, "graphite").changed();
                    changed |= ui.selectable_value(&mut self.thermr_mat, 1, "H in H2O").changed();
                });
                changed |= ui.add(egui::Slider::new(&mut self.thermr_t, 294.0..=1200.0).text("temperature (K)")).drag_stopped();
                changed |= ui.add(egui::Slider::new(&mut self.thermr_e, 1.0e-4..=1.0).logarithmic(true).text("incident energy for emission (eV)")).drag_stopped();
                if changed {
                    let r = self.thermr_request();
                    self.ask(r);
                }
                self.busy_line(ui);
                ui.label("Blue: bound-atom inelastic σ(E) from S(α,β). Green: graphite's Bragg (coherent elastic) σ(E), with its edges. Grey: the free atom. Ticks along the top: where a neutron of the chosen energy goes, one tick per equiprobable bin.");
                ui.weak("A temperature the evaluation does not tabulate is interpolated by its own rule, or refused outside its range.");
                ui.weak("Each change rebuilds THERMR's whole emission (calcem) table: about 20 s for graphite and 25 s for water natively on two cores (2026-10-04), longer in a browser. The page stays live meanwhile.");
            }
            Rung::Groupr => {
                let mut changed = ui.add(egui::Slider::new(&mut self.groupr_n, 4..=200).logarithmic(true).text("groups (1 eV to 10 keV)")).drag_stopped();
                changed |= ui.add(egui::Slider::new(&mut self.groupr_s0, 1.0..=1.0e5).logarithmic(true).text("background σ0 (b)")).drag_stopped();
                if changed {
                    let r = self.groupr_request();
                    self.ask(r);
                }
                self.busy_line(ui);
                ui.label("Orange: pointwise capture. Light steps: group averages for an infinitely dilute absorber. Bright steps: the same, self-shielded at σ0 (Bondarenko flux). Lower σ0 means more U-238 and lower averages in resonance groups.");
            }
            Rung::Acer => {
                ui.label("Recorded results, not a live calculation: building a U-238 ACE table takes minutes natively. The main view lists the record and where it is.");
            }
        }
    }

    /// Deliberate liberties and provenance, per rung.
    fn notes(&self, ui: &mut egui::Ui) {
        let text = match self.rung {
            Rung::Endf => "Data: ENDF/B-VIII.0 U-238 (MAT 9237), covariance files removed before download (transport never reads them).",
            Rung::Reconr => "Liberty: the coarse steps (0.3 to 0.003) exist only to show the grid refining; every one is a complete RECONR at that tolerance. Recorded check: the port's RECONR reproduces NJOY2016's PENDFs word for word (lesson rung 2).",
            Rung::Broadr => "Liberties: the 0 K curve is RECONR at tolerance 0.01, not 0.001, and only 0.5-600 eV is broadened, with the per-reaction SIGMA1 kernel the lesson's tutorial example uses; within a few Doppler widths of 0.5 eV and 600 eV the result is affected by the cut, so only 1-300 eV is shown.",
            Rung::Purr => "Liberty: 2000 samples per ladder and few ladders, so the bands are noisier than production tables (NJOY's own deck: 20 bands, 64 ladders); the recorded production tables match NJOY2016's in every word (lesson rung 4).",
            Rung::Thermr => "Data: ENDF/B-VIII.0 thermal scattering laws (crystalline graphite; H in H2O). The S(α,β) inelastic and Bragg cross sections are this crate's THERMR port; recorded check: calcem against NJOY2016 to its 7 printed figures (lesson rung 5).",
            Rung::Groupr => "Liberties: the pointwise input is RECONR at tolerance 0.01 and 0 K; the weight is 1/E times the narrow-resonance (Bondarenko) factor with σ_pot set to 0, which cancels in each group average. The recorded GROUPR check (U-238, 29 groups) is in lesson rung 7.",
            Rung::Acer => "Provenance: crates/njoy-outram-park-fork/verification_and_validation/ace_block_parity/ace_block_parity_2026_09_26.md and outram-mc-libs' five_route_keff_2026_09_29.md (2026-09-30 table).",
        };
        ui.label(RichText::new(text).size(12.0).color(Color32::from_rgb(170, 176, 190)));
    }
}

// ─── Main view ───────────────────────────────────────────────────────────────

impl NdApp {
    fn main_view(&mut self, ui: &mut egui::Ui) {
        let rect = ui.available_rect_before_wrap();
        let table_rung = matches!(self.rung, Rung::Endf | Rung::Acer);
        if table_rung && matches!(self.phase, Phase::Ready) {
            ui.painter().rect_filled(rect, 0.0, BG);
            let inner = rect.shrink2(egui::vec2(12.0, 52.0));
            ui.scope_builder(egui::UiBuilder::new().max_rect(inner), |ui| {
                egui::ScrollArea::both().auto_shrink([false, false]).show(ui, |ui| match self.rung {
                    Rung::Endf => self.sections_table(ui),
                    _ => self.acer_table(ui),
                });
            });
            if let Some(z) = zoom_buttons(ui, rect) {
                self.text_size = match z {
                    dhoby_ghaut::web_demo::view::Zoom::In => (self.text_size * 1.2).min(30.0),
                    dhoby_ghaut::web_demo::view::Zoom::Out => (self.text_size / 1.2).max(8.0),
                    dhoby_ghaut::web_demo::view::Zoom::Reset => 14.0,
                };
            }
            self.panel.reopen_button(ui, rect);
            return;
        }
        let (resp, painter) = ui.allocate_painter(ui.available_size(), Sense::click_and_drag());
        let rect = resp.rect;
        painter.rect_filled(rect, 0.0, BG);
        if !self.framed && !self.curves.is_empty() {
            self.reframe(rect.width() as f64, rect.height() as f64);
        }
        self.view.handle_input(ui, &resp);
        match &self.phase {
            Phase::Loading(l) => l.card(&painter, rect, "Processing ENDF data…", None),
            Phase::Failed(e) => {
                painter.text(rect.center(), egui::Align2::CENTER_CENTER, format!("Failed: {e}"), egui::FontId::proportional(14.0), Color32::from_rgb(255, 110, 110));
            }
            Phase::Ready => self.draw_plot(&painter, rect),
        }
        if let Some(t0) = self.computing_since {
            painter.text(
                rect.center_bottom() + egui::vec2(0.0, -46.0),
                egui::Align2::CENTER_BOTTOM,
                format!("computing in the background… {:.0} s (pan and zoom freely)", now_s() - t0),
                egui::FontId::proportional(13.0),
                Color32::from_rgb(250, 200, 80),
            );
        }
        if let Some(z) = zoom_buttons(ui, rect) {
            apply_zoom(&mut self.view, rect, z);
        }
        self.panel.reopen_button(ui, rect);
    }

    fn draw_plot(&self, painter: &egui::Painter, rect: egui::Rect) {
        let (v, f) = (&self.view, &self.frame);
        let (xname, yname) = match self.rung {
            Rung::Purr => ("E (eV)", "total self-shielding factor (band value)"),
            _ => ("E (eV)", "σ (barn)"),
        };
        plot::axes(painter, rect, v, f, xname, yname);
        let thin = |c: Color32| Stroke::new(1.2, c);
        let mut legend: Vec<(Color32, &str)> = Vec::new();
        for c in &self.curves {
            match (self.rung, c.tag) {
                (Rung::Reconr, 1) => {
                    plot::line(painter, rect, v, f, &c.pts, thin(TOTAL));
                    // The grid itself, once zoomed in enough to resolve points.
                    if c.pts.len() < 2_000_000 && self.view.scale > 2.0 * self.view.fit_scale(rect) {
                        plot::dots(painter, rect, v, f, &c.pts, &[], Color32::from_rgba_premultiplied(120, 170, 255, 160));
                    }
                    legend.push((TOTAL, "total (MT=1) and its grid points"));
                }
                (Rung::Reconr, 2) => {
                    plot::line(painter, rect, v, f, &c.pts, thin(ELASTIC));
                    legend.push((ELASTIC, "elastic (MT=2)"));
                }
                (Rung::Reconr, 102) => {
                    plot::line(painter, rect, v, f, &c.pts, thin(CAPTURE));
                    legend.push((CAPTURE, "capture (MT=102)"));
                }
                (Rung::Broadr, 1001) | (Rung::Broadr, 1102) => plot::line(painter, rect, v, f, &c.pts, thin(FAINT)),
                (Rung::Broadr, 1) => {
                    plot::line(painter, rect, v, f, &c.pts, Stroke::new(1.6, TOTAL));
                    legend.push((TOTAL, "total at T"));
                }
                (Rung::Broadr, 102) => {
                    plot::line(painter, rect, v, f, &c.pts, Stroke::new(1.6, CAPTURE));
                    legend.push((CAPTURE, "capture at T"));
                    legend.push((FAINT, "0 K"));
                }
                (Rung::Purr, 1) => {
                    plot::dots(painter, rect, v, f, &c.pts, &c.w, BANDS);
                    if let (Some(a), Some(b)) = (f.to_screen(v, rect, 2.0e4, 1.0), f.to_screen(v, rect, 1.49e5, 1.0)) {
                        painter.line_segment([a, b], Stroke::new(1.0, FREE));
                    }
                    legend.push((BANDS, "bands (dot size = probability)"));
                    legend.push((FREE, "1 = the dilute cross section"));
                }
                (Rung::Thermr, 1) => {
                    plot::line(painter, rect, v, f, &c.pts, Stroke::new(1.8, TOTAL));
                    legend.push((TOTAL, "inelastic, bound atom"));
                }
                (Rung::Thermr, 2) => {
                    plot::line(painter, rect, v, f, &c.pts, Stroke::new(1.4, ELASTIC));
                    legend.push((ELASTIC, "coherent elastic (Bragg)"));
                }
                (Rung::Thermr, 3) => {
                    plot::line(painter, rect, v, f, &c.pts, Stroke::new(1.0, FREE));
                    legend.push((FREE, "free atom"));
                }
                (Rung::Thermr, 10) => {
                    for &[e, _] in &c.pts {
                        if let Some(p) = f.to_screen(v, rect, e, 1.0) {
                            painter.line_segment([Pos2::new(p.x, rect.top() + 48.0), Pos2::new(p.x, rect.top() + 62.0)], Stroke::new(1.5, CAPTURE));
                        }
                    }
                    if let Some(p) = f.to_screen(v, rect, self.thermr_e, 1.0) {
                        painter.circle_filled(Pos2::new(p.x, rect.top() + 55.0), 4.0, Color32::WHITE);
                    }
                    legend.push((CAPTURE, "where it goes (white: from)"));
                }
                (Rung::Groupr, 102) => plot::line(painter, rect, v, f, &c.pts, thin(CAPTURE)),
                (Rung::Groupr, 1) => plot::line(painter, rect, v, f, &c.pts, Stroke::new(1.5, FAINT)),
                (Rung::Groupr, 2) => {
                    plot::line(painter, rect, v, f, &c.pts, Stroke::new(2.0, TOTAL));
                    legend.push((CAPTURE, "capture, pointwise"));
                    legend.push((FAINT, "group averages, infinitely dilute"));
                    legend.push((TOTAL, "group averages at σ0"));
                }
                _ => {}
            }
        }
        if self.rung == Rung::Broadr {
            if let (Some(a), Some(b)) = (f.to_screen(v, rect, 5.0, 1.0), f.to_screen(v, rect, 200.0, 1.0)) {
                painter.text(Pos2::new(a.x, rect.bottom() - 44.0), egui::Align2::LEFT_BOTTOM, "area check: 5-200 eV", egui::FontId::proportional(11.0), FAINT);
                let _ = b;
            }
        }
        plot::legend(painter, rect, &legend);
        if self.curves.is_empty() && self.rung != Rung::Acer {
            painter.text(rect.center(), egui::Align2::CENTER_CENTER, "waiting for the first result…", egui::FontId::proportional(14.0), FAINT);
        }
    }

    fn sections_table(&self, ui: &mut egui::Ui) {
        let s = self.text_size;
        ui.label(RichText::new("Sections on the tape (MF = kind of data, MT = reaction)").size(s + 2.0).strong());
        if let Some(c) = self.curves.first() {
            egui::Grid::new("sections").num_columns(4).spacing([18.0, 2.0]).striped(true).show(ui, |ui| {
                for h in ["#", "MF", "MT", "data lines"] {
                    ui.label(RichText::new(h).size(s).strong());
                }
                ui.end_row();
                for (i, (p, w)) in c.pts.iter().zip(&c.w).enumerate() {
                    let (mf, mt) = ((*w as i64) / 1000, (*w as i64) % 1000);
                    ui.label(RichText::new(format!("{}", i + 1)).size(s));
                    ui.label(RichText::new(format!("{mf}")).size(s).monospace());
                    ui.label(RichText::new(format!("{mt}")).size(s).monospace());
                    ui.label(RichText::new(format!("{}", p[1] as u64)).size(s).monospace());
                    ui.end_row();
                }
            });
        } else {
            ui.spinner();
        }
    }

    fn acer_table(&self, ui: &mut egui::Ui) {
        let s = self.text_size;
        ui.label(RichText::new("Recorded: four NJOY2016 ACE tables reproduced in every word").size(s + 2.0).strong());
        egui::Grid::new("acer").num_columns(3).spacing([18.0, 3.0]).striped(true).show(ui, |ui| {
            for row in [
                ["table (ENDF/B-VIII.0)", "XSS words", "result"],
                ["U-234, 293.6 K, RECONR→BROADR→PURR→ACER", "449 695", "identical"],
                ["U-238, 293.6 K, same deck", "6 247 445", "identical"],
                ["U-235, 293.6 K, same deck", "6 712 632", "identical"],
                ["U-235, 0 K, RECONR→ACER", "15 616 079", "identical"],
            ] {
                for c in row {
                    ui.label(RichText::new(c).size(s));
                }
                ui.end_row();
            }
        });
        ui.add_space(8.0);
        ui.label(RichText::new("Source: verification_and_validation/ace_block_parity/ace_block_parity_2026_09_26.md (2026-09-26).").size(s - 2.0).weak());
        ui.add_space(12.0);
        ui.label(RichText::new("Recorded: OpenMC on this crate's ACE (route 2) vs on NJOY2016's (route 1)").size(s + 2.0).strong());
        egui::Grid::new("routes").num_columns(4).spacing([18.0, 3.0]).striped(true).show(ui, |ui| {
            for row in [
                ["case", "route 1 k", "route 2 k", "route 2 − route 1"],
                ["Godiva", "1.00016 ± 0.00021", "1.00016 ± 0.00021", "+0 ± 29 pcm"],
                ["Jemima", "0.99594 ± 0.00017", "0.99594 ± 0.00017", "+0 ± 25 pcm"],
                ["HST-009", "1.00234 ± 0.00022", "1.00275 ± 0.00031", "+41 ± 38 pcm"],
                ["LCT-008", "1.00244 ± 0.00005", "1.00248 ± 0.00008", "+4 ± 9 pcm"],
            ] {
                for c in row {
                    ui.label(RichText::new(c).size(s));
                }
                ui.end_row();
            }
        });
        ui.add_space(8.0);
        ui.label(
            RichText::new(
                "Source: crates/outram-mc-libs/verification_and_validation/icsbep/five_route_keff_2026_09_29.md, \"Results — after the \
                 OpenMC-parity audit\" (2026-09-30); 32 seeds per cell (route 1 LCT-008: 96). Verification (one data route against \
                 another), not validation.",
            )
            .size(s - 2.0)
            .weak(),
        );
    }
}
