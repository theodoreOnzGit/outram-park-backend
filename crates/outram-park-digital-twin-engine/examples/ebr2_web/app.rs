//! The page: an egui app that only draws (the plant runs in the engine,
//! `ebr2::web`).
//!
//! Built on `bugis::web_demo` (the folding side panel, the main view's zoom
//! buttons, the link to the engine) and `bugis::tab_bar` (the bottom bar,
//! gh:#1717). Three tabs: the Plant sketch (`plant_view`), the Trends, and
//! the Reactivity broken into its feedback terms.
//!
//! GUI drawing code: reached by no test (needs a window), per the workspace
//! exception for GUI code. What it draws is tested in `ebr2::session`,
//! `ebr2::web` and `plant_view`'s tests.

use crate::plant_view::{self, Tracers};
use bugis::web_demo::link::Link;
use bugis::web_demo::panel::Panel;
use bugis::web_demo::platform::{query_pairs, set_title};
use bugis::web_demo::view::{apply_zoom, zoom_buttons, View};
use egui::{Align2, Color32, FontId, Pos2, Rect, RichText, Stroke, Vec2};
use egui_plot::{Legend, Line, Plot, PlotPoints};
use outram_park_digital_twin_engine::ebr2::design::{PUMP_RATED_SPEED_RPM, RATED_MASS_FLOWS_KG_PER_S};
use outram_park_digital_twin_engine::ebr2::session::{Command, Initial, SessionConfig, Snapshot, VV_THERMAL_CENTRE_HEIGHT_M};
use outram_park_digital_twin_engine::ebr2::shrt::ShrtTest;
use outram_park_digital_twin_engine::ebr2::web::{Event, Request, DEFAULT_SPEED};

/// Speeds offered: plant seconds per wall second.
const SPEEDS: [(f64, &str); 5] = [(1.0, "1× (real time)"), (2.0, "2×"), (5.0, "5×"), (20.0, "20×"), (60.0, "1 min/s")];
/// History kept for the trends: samples beyond this are thinned by half.
const HISTORY_MAX: usize = 6000;
/// External reactivity the slider offers, [pcm]: a UI range, not a rod
/// worth (beta-eff is 705 pcm; the upper end is about half a dollar).
const EXTERNAL_PCM: std::ops::RangeInclusive<f64> = -500.0..=350.0;

#[derive(Clone, Copy, PartialEq, Eq)]
enum Tab {
    Plant,
    Trends,
    Reactivity,
}

impl Tab {
    const ALL: [Tab; 3] = [Tab::Plant, Tab::Trends, Tab::Reactivity];
    fn label(self) -> &'static str {
        match self {
            Tab::Plant => "Plant",
            Tab::Trends => "Trends",
            Tab::Reactivity => "Reactivity",
        }
    }
}

pub struct App {
    theme: bugis::Theme,
    link: Link<Request, Event>,
    panel: Panel,
    view: View,
    tab: Tab,
    snap: Option<Snapshot>,
    history: Vec<Snapshot>,
    initial: Option<Initial>,
    tracers: Tracers,
    /// The hold the engine posted: `(plant time s, message)`.
    held: Option<(f64, String)>,
    running: bool,
    speed: f64,
    config: SessionConfig,
    pump_hold: bool,
    pump_rpm: f64,
    intermediate_hold: bool,
    intermediate_kg_s: f64,
    external_pcm: f64,
    error: Option<String>,
    title: String,
}

impl App {
    pub fn new(link: Link<Request, Event>) -> Self {
        let q = query_pairs();
        let has = |k: &str| q.iter().any(|(a, _)| a == k);
        let value = |k: &str| q.iter().find(|(a, _)| a == k).map(|(_, v)| v.clone());
        let mut config = SessionConfig::default();
        if value("test").as_deref() == Some("shrt17") {
            config.test = ShrtTest::Shrt17;
        }
        let tab = match value("tab").as_deref() {
            Some("trends") => Tab::Trends,
            Some("reactivity") => Tab::Reactivity,
            _ => Tab::Plant,
        };
        let mut app = Self {
            theme: bugis::Theme::default(),
            link,
            panel: Panel::default(),
            view: View::new(plant_view::HALF_EXTENT),
            tab,
            snap: None,
            history: Vec::new(),
            initial: None,
            tracers: Tracers::default(),
            held: None,
            // The plant runs from page load; `?paused` opens it paused.
            running: !has("paused"),
            speed: DEFAULT_SPEED,
            config,
            pump_hold: false,
            pump_rpm: 780.0,
            intermediate_hold: false,
            intermediate_kg_s: 300.0,
            external_pcm: 0.0,
            error: None,
            title: String::new(),
        };
        app.send(Request::Reset(app.config));
        app.send(Request::Pace(app.speed, !app.running));
        app
    }

    fn send(&mut self, r: Request) {
        self.link.send(r);
    }

    fn reset(&mut self) {
        self.history.clear();
        self.initial = None;
        self.held = None;
        self.error = None;
        self.pump_hold = false;
        self.intermediate_hold = false;
        self.external_pcm = 0.0;
        self.tracers = Tracers::default();
        self.send(Request::Reset(self.config));
    }

    /// Take what the engine posted: the page never asks for a step.
    fn handle(&mut self) {
        for e in self.link.drain() {
            match e {
                Event::Frame(f) => {
                    if let Some(s) = Snapshot::from_slice(&f.to_vec()) {
                        if self.history.last().map_or(true, |h| s.time_s > h.time_s) {
                            self.history.push(s);
                            if self.history.len() > HISTORY_MAX {
                                self.history = self.history.iter().step_by(2).copied().collect();
                            }
                        }
                        self.tracers.advance(&s);
                        self.snap = Some(s);
                    }
                }
                Event::Initial(f) => self.initial = Initial::from_slice(&f.to_vec()),
                Event::Held(t, m) => self.held = Some((t, m)),
                Event::Error(m) => self.error = Some(m),
            }
        }
    }

    fn command(&mut self, c: Command) {
        // A command releases a hold in the engine (`Session::apply`).
        self.held = None;
        self.error = None;
        self.send(Request::Cmd(c));
    }

    fn side_panel(&mut self, ui: &mut egui::Ui) {
        ui.label(RichText::new("Museum · historical reactors").strong());
        ui.label("EBR-II (Argonne, Idaho, 1964-1994): a 62.5 MW pool-type sodium-cooled fast reactor. Core, pumps and intermediate heat exchanger sit in one tank of 340 m³ of sodium.");
        ui.label(RichText::new("RESEARCH, EDUCATION AND V&V ONLY. Status: NOT VALIDATED. An AI-assisted draft, not reviewed by a human; never for operation, licensing or safety decisions.").small().strong().color(ui.visuals().warn_fg_color));
        if let Some(e) = &self.error {
            ui.colored_label(ui.visuals().error_fg_color, e);
        }
        bugis::selector(ui, &mut self.theme);
        ui.separator();

        ui.heading("Test");
        let before = self.config.test;
        ui.radio_value(&mut self.config.test, ShrtTest::Shrt17, "SHRT-17 (1984): protected loss of flow, pumps tripped with scram");
        ui.radio_value(&mut self.config.test, ShrtTest::Shrt45r, "SHRT-45R (1986): unprotected loss of flow, no scram");
        if self.config.test != before {
            self.reset();
        }
        ui.label(RichText::new(match self.config.test {
            ShrtTest::Shrt17 => "SHRT-17's power is its tabulated measurement (a boundary condition), so there is no feedback to show and external reactivity does nothing.",
            ShrtTest::Shrt45r => "SHRT-45R's power is computed: six-group point kinetics with Argonne's feedback coefficients. Watch the reactor shut itself down as the core heats.",
        }).small());

        ui.heading("Run");
        ui.horizontal(|ui| {
            let label = if self.running { "⏸ Pause" } else { "▶ Run" };
            if ui.add(egui::Button::new(RichText::new(label).size(18.0)).min_size(Vec2::new(110.0, 40.0))).clicked() {
                self.running = !self.running;
                self.send(Request::Pace(self.speed, !self.running));
            }
            if ui.add(egui::Button::new(RichText::new("Reset").size(18.0)).min_size(Vec2::new(90.0, 40.0))).clicked() {
                self.reset();
            }
        });
        egui::ComboBox::from_label("speed")
            .selected_text(SPEEDS.iter().find(|s| s.0 == self.speed).map_or("?", |s| s.1))
            .show_ui(ui, |ui| {
                for (v, l) in SPEEDS {
                    if ui.selectable_value(&mut self.speed, v, l).changed() {
                        self.send(Request::Pace(self.speed, !self.running));
                    }
                }
            });
        ui.label(RichText::new("The tests' tables end at 900 s; after that every boundary value holds its last entry.").small());

        ui.separator();
        ui.heading("Operate");
        let s = self.snap.unwrap_or_default();
        if ui.checkbox(&mut self.pump_hold, "Hold the primary pumps (override the SHRT coast-down)").changed() {
            if self.pump_hold {
                self.pump_rpm = 0.5 * (s.pump1_rpm + s.pump2_rpm);
                self.command(Command::PumpSpeedRpm(Some(self.pump_rpm)));
            } else {
                self.command(Command::PumpSpeedRpm(None));
            }
        }
        ui.add_enabled_ui(self.pump_hold, |ui| {
            let r = ui.add(egui::Slider::new(&mut self.pump_rpm, 0.0..=PUMP_RATED_SPEED_RPM).text("both pumps, rpm"));
            if r.drag_stopped() || (r.changed() && !r.dragged()) {
                let rpm = self.pump_rpm;
                self.command(Command::PumpSpeedRpm(Some(rpm)));
            }
        });
        ui.label(RichText::new("The tests trip the pumps at t = 0 (their tables). Holding them is the only way not to trip; releasing resumes the table at the present time. The slider stops at the 870 rpm rated speed (ARC-226 Table 3.1.4.1.2).").small());

        if ui.checkbox(&mut self.intermediate_hold, "Hold the intermediate loop flow").changed() {
            if self.intermediate_hold {
                self.intermediate_kg_s = s.intermediate_flow_kg_s;
                self.command(Command::IntermediateFlowKgS(Some(self.intermediate_kg_s)));
            } else {
                self.command(Command::IntermediateFlowKgS(None));
            }
        }
        ui.add_enabled_ui(self.intermediate_hold, |ui| {
            // Up to 1.25 x the 315 kg/s rated secondary flow (ARC-226 p.3, via
            // `design::RATED_MASS_FLOWS_KG_PER_S`): a UI range.
            let r = ui.add(egui::Slider::new(&mut self.intermediate_kg_s, 0.0..=1.25 * RATED_MASS_FLOWS_KG_PER_S[1]).text("intermediate flow, kg/s"));
            if r.drag_stopped() || (r.changed() && !r.dragged()) {
                let x = self.intermediate_kg_s;
                self.command(Command::IntermediateFlowKgS(Some(x)));
            }
        });

        let kinetics = self.config.test == ShrtTest::Shrt45r;
        ui.add_enabled_ui(kinetics, |ui| {
            let r = ui.add(egui::Slider::new(&mut self.external_pcm, EXTERNAL_PCM).text("external reactivity, pcm"));
            if r.drag_stopped() || (r.changed() && !r.dragged()) {
                let x = self.external_pcm;
                self.command(Command::ExternalPcm(x));
            }
            if ui.add(egui::Button::new("Zero").min_size(Vec2::new(80.0, 36.0))).clicked() {
                self.external_pcm = 0.0;
                self.command(Command::ExternalPcm(0.0));
            }
        });
        ui.label(RichText::new("External reactivity (a rod, a scram) acts on SHRT-45R's point kinetics only. β_eff = 705 pcm (IAEA-TECDOC-1819).").small());

        ui.separator();
        ui.heading("Model (applies on Reset)");
        ui.add(egui::Slider::new(&mut self.config.thermal_centre_height_m, 0.5..=5.0).text("IHX above core, m"));
        ui.label(RichText::new(format!(
            "The height of the IHX's thermal centre above the core's drives natural circulation once the pumps stop. No transcribed source gives it; the model requires it, and the library's V&V record ran at {VV_THERMAL_CENTRE_HEIGHT_M} m. SHRT-17's peak power-to-flow ratio moves from 2.47 to 1.16 between 1 and 4 m."
        )).small());

        ui.separator();
        ui.label(RichText::new("Sources: ANL-ARC-226 and ANL-ARC-228 (Argonne benchmark specifications), IAEA-TECDOC-1819, transcribed in reference-data/ebr2/ (AI transcriptions, unreviewed). Code: outram_park_digital_twin_engine::ebr2. V&V record: docs/ebr2_library.md.").small());
    }

    fn plant(&mut self, ui: &mut egui::Ui, rect: Rect) {
        let resp = ui.interact(rect, ui.id().with("plant"), egui::Sense::click_and_drag());
        if !self.view.fitted || (!self.view.touched && self.view.fit_size != rect.size()) {
            self.view.fit(rect);
        }
        self.view.handle_input(ui, &resp);
        let Some(s) = self.snap else {
            ui.painter().text(rect.center(), Align2::CENTER_CENTER, "Starting the plant…", FontId::proportional(18.0), ui.visuals().text_color());
            return;
        };
        let view = &self.view;
        let to = |x: f64, y: f64| view.to_screen(rect, x, y);
        let (text, muted) = (ui.visuals().text_color(), ui.visuals().weak_text_color());
        plant_view::paint(&ui.painter().with_clip_rect(rect), &to, &s, &self.tracers, text, muted);
        // Readouts and the colour key, on the main view.
        let narrow = rect.width() < 700.0;
        let top = rect.top() + if narrow { 84.0 } else { 52.0 };
        let lines = [
            format!("primary flow {:.0} kg/s ({:.1} % of initial) · power/flow {:.2}", s.primary_flow_kg_s, 100.0 * s.flow_fraction, s.power_to_flow),
            "Indicative layout, not plant geometry: the elevations (ARC-226 Ch. 5) are not transcribed.".to_owned(),
        ];
        for (k, l) in lines.iter().enumerate() {
            ui.painter().text(Pos2::new(rect.center().x, top + 18.0 * k as f32), Align2::CENTER_TOP, l, FontId::proportional(if k == 0 { 13.0 } else { 11.0 }), if k == 0 { text } else { muted });
        }
        let k = Rect::from_min_size(Pos2::new(rect.right() - 170.0, rect.bottom() - 34.0), Vec2::new(150.0, 12.0));
        for i in 0..30 {
            let f = i as f64 / 29.0;
            let c = plant_view::colour(plant_view::T_COLD_C + f * (plant_view::T_HOT_C - plant_view::T_COLD_C));
            let x = k.left() + k.width() * f as f32;
            ui.painter().rect_filled(Rect::from_min_size(Pos2::new(x, k.top()), Vec2::new(k.width() / 29.0 + 1.0, k.height())), 0.0, c);
        }
        ui.painter().text(k.left_bottom() + Vec2::new(0.0, 2.0), Align2::LEFT_TOP, format!("{:.0} °C", plant_view::T_COLD_C), FontId::proportional(12.0), text);
        ui.painter().text(k.right_bottom() + Vec2::new(0.0, 2.0), Align2::RIGHT_TOP, format!("{:.0} °C", plant_view::T_HOT_C), FontId::proportional(12.0), text);
        if let Some(z) = zoom_buttons(ui, rect) {
            apply_zoom(&mut self.view, rect, z);
        }
    }

    fn trends(&self, ui: &mut egui::Ui) {
        let h = &self.history;
        let line = |f: fn(&Snapshot) -> f64, name: &str| Line::new(name, PlotPoints::from_iter(h.iter().map(|s| [s.time_s, f(s)])));
        let height = ((ui.available_height() - 150.0) / 3.0).max(120.0);
        ui.label(RichText::new("No measured SHRT trace is transcribed in reference-data/ebr2/ yet (gh:#1316), so none is overlaid. The table below compares the model's own steady state with the cited initial conditions: a check, not a fit.").small());
        if let Some(i) = self.initial {
            egui::Grid::new("initial").striped(true).show(ui, |ui| {
                ui.strong("t = 0");
                ui.strong("model");
                ui.strong("cited");
                ui.end_row();
                ui.label("primary flow, kg/s");
                ui.label(format!("{:.1}", i.model_flow_kg_s));
                ui.label(format!("{:.1}", i.cited_flow_kg_s));
                ui.end_row();
                ui.label("core inlet, °C");
                ui.label(format!("{:.1}", i.model_core_inlet_c));
                ui.label(format!("{:.1}", i.cited_core_inlet_c));
                ui.end_row();
                ui.label("core outlet, °C");
                ui.label(format!("{:.1}", i.model_core_outlet_c));
                ui.label("–");
                ui.end_row();
            });
        }
        ui.label("Power, MW");
        Plot::new("power").height(height).include_y(0.0).legend(Legend::default()).show(ui, |p| {
            p.line(line(|s| s.total_mw, "total"));
            p.line(line(|s| s.fission_mw, "fission"));
            p.line(line(|s| s.decay_mw, "decay heat"));
        });
        ui.label("Temperatures, °C");
        Plot::new("temps").height(height).include_y(300.0).include_y(600.0).legend(Legend::default()).show(ui, |p| {
            p.line(line(|s| s.fuel_c, "fuel"));
            p.line(line(|s| s.core_outlet_c, "core outlet"));
            p.line(line(|s| s.ihx_outlet_c, "IHX outlet"));
            p.line(line(|s| s.core_inlet_c, "core inlet (pool)"));
        });
        ui.label("Primary flow, % of initial · power/flow ratio × 100");
        Plot::new("flow").height(height).include_y(0.0).include_y(100.0).legend(Legend::default()).show(ui, |p| {
            p.line(line(|s| 100.0 * s.flow_fraction, "flow, %"));
            p.line(line(|s| 100.0 * s.power_to_flow, "P/F × 100"));
        });
    }

    fn reactivity(&self, ui: &mut egui::Ui, rect: Rect) {
        let Some(s) = self.snap else { return };
        if s.kinetics == 0.0 {
            ui.label(RichText::new("SHRT-17 runs on its tabulated power (fission and decay heat, ARC-226 Table 3.1.4.2.1): the power is a boundary condition, so the model computes no reactivity. Switch to SHRT-45R to see the feedback.").strong());
            return;
        }
        let beta = s.beta_eff_pcm;
        ui.label(format!("net {:+.1} pcm ({:+.3} $) · β_eff {:.0} pcm", s.rho_total_pcm, s.rho_total_pcm / beta, beta));
        ui.label(RichText::new("Each term is Argonne's coefficient (VARIANT, IAEA-TECDOC-1819) times the change in its temperature or density since t = 0. Negative terms push the power down.").small());
        // The bars: drawn from the terms, scaled to the largest.
        let terms = s.reactivity_terms();
        let max = terms.iter().map(|t| t.1.abs()).fold(1.0, f64::max);
        let top = ui.cursor().top() + 6.0;
        let row_h = 26.0;
        let label_w = (rect.width() * 0.42).min(190.0);
        let mid = rect.left() + label_w + 0.5 * (rect.width() - label_w);
        let half = 0.5 * (rect.width() - label_w) - 8.0;
        let painter = ui.painter().with_clip_rect(rect);
        let (pos, neg) = (Color32::from_rgb(214, 93, 14), Color32::from_rgb(69, 133, 136));
        for (k, (name, v)) in terms.iter().enumerate() {
            let y = top + row_h * k as f32;
            painter.text(Pos2::new(rect.left(), y + 0.5 * row_h), Align2::LEFT_CENTER, *name, FontId::proportional(13.0), ui.visuals().text_color());
            let w = (v / max) as f32 * half;
            let bar = Rect::from_two_pos(Pos2::new(mid, y + 4.0), Pos2::new(mid + w, y + row_h - 4.0));
            painter.rect_filled(bar, 2.0, if *v >= 0.0 { pos } else { neg });
            let align = if *v >= 0.0 { Align2::LEFT_CENTER } else { Align2::RIGHT_CENTER };
            let at = Pos2::new(mid + w + if *v >= 0.0 { 4.0 } else { -4.0 }, y + 0.5 * row_h);
            painter.text(at, align, format!("{v:+.1}"), FontId::proportional(12.0), ui.visuals().text_color());
        }
        painter.line_segment([Pos2::new(mid, top), Pos2::new(mid, top + row_h * terms.len() as f32)], Stroke::new(1.0, ui.visuals().weak_text_color()));
        ui.add_space(row_h * terms.len() as f32 + 12.0);
        let h = &self.history;
        let line = |f: fn(&Snapshot) -> f64, name: &str| Line::new(name, PlotPoints::from_iter(h.iter().map(|s| [s.time_s, f(s)])));
        Plot::new("rho").height((ui.available_height() - 8.0).max(140.0)).legend(Legend::default()).show(ui, |p| {
            p.line(line(|s| s.rho_total_pcm, "net"));
            p.line(line(|s| s.rho_doppler_pcm, "Doppler"));
            p.line(line(|s| s.rho_axial_pcm, "axial fuel"));
            p.line(line(|s| s.rho_coolant_density_pcm, "coolant density"));
            p.line(line(|s| s.rho_radial_pcm, "radial expansion"));
            p.line(line(|s| s.rho_external_pcm, "external"));
        });
    }

    /// The hold modal: why the plant stopped, and the way on.
    fn held_modal(&mut self, ctx: &egui::Context) {
        let Some((t, m)) = self.held.clone() else { return };
        let mut reset = false;
        egui::Window::new("The plant is held")
            .collapsible(false)
            .resizable(false)
            .anchor(Align2::CENTER_CENTER, Vec2::ZERO)
            .show(ctx, |ui| {
                ui.set_max_width(400.0);
                ui.label(RichText::new(format!("Held at t = {t:.1} s: the next step failed.")).strong());
                ui.label(RichText::new(&m).small());
                ui.label("The usual cause is sodium leaving the model's 400-1000 K property range; sodium boils near 1156 K, so past here the model has nothing honest to say. Change a control (more flow, less reactivity) to go on from this state, or reset.");
                reset = ui.add(egui::Button::new(RichText::new("Reset").size(17.0)).min_size(Vec2::new(100.0, 38.0))).clicked();
            });
        if reset {
            self.reset();
        }
    }
}

impl eframe::App for App {
    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        let ctx = ui.ctx().clone();
        // Each engine event repaints the page (the link's callback).
        self.handle();
        let title = match &self.snap {
            Some(s) => format!("EBR-II · {} · {:.0} s · {:.1} MW", self.config.test.name(), s.time_s, s.total_mw),
            None => "EBR-II · starting".into(),
        };
        if title != self.title {
            set_title(&ctx, &title);
            self.title = title;
        }
        let mut panel = std::mem::take(&mut self.panel);
        panel.show(ui, "EBR-II simulator", |ui| self.side_panel(ui));
        self.panel = panel;
        egui::CentralPanel::default().frame(egui::Frame::NONE.fill(ui.visuals().extreme_bg_color)).show(ui, |ui| {
            let full = ui.max_rect();
            let bar = bugis::tab_bar::layout(full, Tab::ALL.len());
            let main = bar.main;
            let narrow = main.width() < 700.0;
            match self.tab {
                Tab::Plant => self.plant(ui, main),
                Tab::Trends => {
                    let inner = Rect::from_min_max(main.min + Vec2::new(8.0, if narrow { 84.0 } else { 44.0 }), main.max - Vec2::new(8.0, 4.0));
                    ui.scope_builder(egui::UiBuilder::new().max_rect(inner), |ui| egui::ScrollArea::vertical().show(ui, |ui| self.trends(ui)));
                }
                Tab::Reactivity => {
                    let inner = Rect::from_min_max(main.min + Vec2::new(8.0, if narrow { 84.0 } else { 44.0 }), main.max - Vec2::new(8.0, 4.0));
                    ui.scope_builder(egui::UiBuilder::new().max_rect(inner), |ui| self.reactivity(ui, inner));
                }
            }
            // The status strip, on every tab, shown with the panel folded.
            if let Some(s) = &self.snap {
                let at = if narrow { Vec2::new(10.0, 58.0) } else { Vec2::new(if self.panel.open { 8.0 } else { 156.0 }, 14.0) };
                let state = if self.held.is_some() {
                    " · HELD"
                } else if self.running {
                    ""
                } else {
                    " (paused)"
                };
                let colour = if self.held.is_some() { ui.visuals().error_fg_color } else { ui.visuals().text_color() };
                ui.painter().text(
                    full.left_top() + at,
                    Align2::LEFT_TOP,
                    format!("{} · t = {:.0} s · {:.1} MW{state} · NOT VALIDATED", self.config.test.name(), s.time_s, s.total_mw),
                    FontId::proportional(14.0),
                    colour,
                );
            }
            let labels: Vec<&str> = Tab::ALL.iter().map(|t| t.label()).collect();
            let selected = Tab::ALL.iter().position(|t| *t == self.tab).unwrap_or(0);
            if let Some(i) = bugis::tab_bar::tab_bar(ui, &bar, &labels, selected) {
                self.tab = Tab::ALL[i];
            }
            self.panel.reopen_button(ui, full);
        });
        self.held_modal(&ctx);
    }
}
