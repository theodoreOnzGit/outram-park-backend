//! The page: an egui app that only draws (the plant runs in the engine).
//!
//! Built on `bugis::web_demo` (the folding side panel, the main view's zoom
//! buttons, the link to the engine) and the digital-twin engine's hot-to-cold
//! colour map. Three views: the plant schematic, the trends, and the T-1
//! frequency response.
//!
//! **The schematic's layout is an indicative drawing, not plant geometry**:
//! boxes placed to read left to right as the salt flows. Colours and numbers
//! are the model's; nothing on it moves, so the animation rule has nothing
//! to derive (flows are shown as numbers).
//!
//! GUI drawing code: reached by no test (needs a window), per the workspace
//! exception for GUI code. What it draws is tested in `msre::session` and
//! `engine::tests`.

use crate::engine::{Event, Request};
use bugis::web_demo::link::Link;
use bugis::web_demo::panel::Panel;
use bugis::web_demo::platform::{query_pairs, set_title};
use bugis::web_demo::view::{apply_zoom, zoom_buttons, View};
use egui::{Align2, Color32, FontId, Pos2, Rect, RichText, Stroke, StrokeKind, Vec2};
use egui_plot::{Legend, Line, Plot, PlotPoints, Points};
use outram_park_digital_twin_engine::color_maps::hot_to_cold_colour_mark_1;
use outram_park_digital_twin_engine::msre::session::{
    Coefficients, Command, SessionConfig, Snapshot, COOLANT_LIQUIDUS_F, FUEL_LIQUIDUS_F, MAX_ADVANCE_S,
    MC_N3_FUEL_PER_F, MC_N3_GRAPHITE_PER_F,
};
use outram_park_digital_twin_engine::msre::design::{ORNL_1965_FUEL_PER_F, ORNL_1965_GRAPHITE_PER_F};

/// The colour scale, °F: the coldest salt shown blue, the hottest red.
const T_COLD_F: f64 = 800.0;
const T_HOT_F: f64 = 1300.0;
/// Speeds offered: plant seconds per wall second.
const SPEEDS: [(f64, &str); 5] = [(1.0, "1× (real time)"), (5.0, "5×"), (20.0, "20×"), (60.0, "1 min/s"), (300.0, "5 min/s")];
/// History kept for the trends: samples beyond this are thinned by half.
const HISTORY_MAX: usize = 6000;

#[derive(Clone, Copy, PartialEq, Eq)]
enum Tab {
    Plant,
    Trends,
    Periods,
}

pub struct App {
    theme: bugis::Theme,
    link: Link<Request, Event>,
    panel: Panel,
    view: View,
    tab: Tab,
    snap: Option<Snapshot>,
    history: Vec<Snapshot>,
    periods: Vec<[f64; 3]>,
    pending: u32,
    running: bool,
    speed: f64,
    config: SessionConfig,
    rods_pcm: f64,
    air: f64,
    servo: bool,
    freq_fuel: f64,
    freq_graphite: f64,
    error: Option<String>,
    title: String,
}

impl App {
    pub fn new(link: Link<Request, Event>) -> Self {
        let q = query_pairs();
        let mut app = Self {
            theme: bugis::Theme::default(),
            link,
            panel: Panel::default(),
            // The schematic spans x = -10..10, y = -5..5 (drawing units).
            view: View::new(10.5).with_home([0.0, 0.0]),
            tab: Tab::Plant,
            snap: None,
            history: Vec::new(),
            periods: Vec::new(),
            pending: 0,
            running: q.iter().any(|(k, _)| k == "run"),
            speed: 5.0,
            config: SessionConfig::default(),
            rods_pcm: 0.0,
            air: 0.8,
            servo: false,
            freq_fuel: ORNL_1965_FUEL_PER_F * 1e5,
            freq_graphite: ORNL_1965_GRAPHITE_PER_F * 1e5,
            error: None,
            title: String::new(),
        };
        app.send(Request::Reset(app.config));
        app.send(Request::Periods(app.freq_fuel * 1e-5, app.freq_graphite * 1e-5));
        app
    }

    fn send(&mut self, r: Request) {
        self.pending += 1;
        self.link.send(r);
    }

    fn reset(&mut self) {
        self.history.clear();
        self.rods_pcm = 0.0;
        self.servo = false;
        self.air = self.config.power_mw / 10.0;
        self.send(Request::Reset(self.config));
    }

    fn handle(&mut self, dt_wall: f64) {
        for e in self.link.drain() {
            self.pending = self.pending.saturating_sub(1);
            match e {
                Event::Frame(f) => {
                    if let Some(s) = Snapshot::from_slice(&f.to_vec()) {
                        if self.history.last().map_or(true, |h| s.time_s > h.time_s) {
                            self.history.push(s);
                            if self.history.len() > HISTORY_MAX {
                                self.history = self.history.iter().step_by(2).copied().collect();
                            }
                        }
                        self.air = s.air_flow;
                        self.servo = s.servo != 0.0;
                        if self.servo {
                            self.rods_pcm = s.rod_pcm;
                        }
                        self.snap = Some(s);
                    }
                }
                Event::Periods(f) => {
                    self.periods = f.to_vec().chunks_exact(3).map(|c| [c[0], c[1], c[2]]).collect();
                }
                Event::Error(m) => self.error = Some(m),
            }
        }
        if self.running && self.pending == 0 {
            let dt = (dt_wall.min(0.1) * self.speed).min(MAX_ADVANCE_S);
            self.send(Request::Advance(dt));
        }
    }

    fn side_panel(&mut self, ui: &mut egui::Ui) {
        ui.label(RichText::new("Museum · historical reactors").strong());
        ui.label("The Molten-Salt Reactor Experiment (ORNL, critical 1965, shut down 1969): an 8 MW graphite-moderated reactor whose fuel was a molten fluoride salt, pumped through the core and a heat exchanger.");
        ui.label(RichText::new("The author's hobby project, built out of curiosity in holidays and free time; not particularly related to their work, which is on LWRs and HTGRs. Good for learning; an AI-assisted draft, NOT validated. Research and education only; never for operation, licensing or safety decisions.").small().color(ui.visuals().warn_fg_color));
        if let Some(e) = &self.error {
            ui.colored_label(ui.visuals().error_fg_color, e);
        }
        bugis::selector(ui, &mut self.theme);
        ui.separator();

        ui.heading("Run");
        ui.horizontal(|ui| {
            let label = if self.running { "⏸ Pause" } else { "▶ Run" };
            if ui.add(egui::Button::new(RichText::new(label).size(18.0)).min_size(Vec2::new(110.0, 40.0))).clicked() {
                self.running = !self.running;
            }
            if ui.add(egui::Button::new(RichText::new("Reset").size(18.0)).min_size(Vec2::new(90.0, 40.0))).clicked() {
                self.reset();
            }
        });
        egui::ComboBox::from_label("speed")
            .selected_text(SPEEDS.iter().find(|s| s.0 == self.speed).map_or("?", |s| s.1))
            .show_ui(ui, |ui| {
                for (v, l) in SPEEDS {
                    ui.selectable_value(&mut self.speed, v, l);
                }
            });

        ui.separator();
        ui.heading("Operate");
        if ui.checkbox(&mut self.servo, "Flux servo holds the power (rods follow)").changed() {
            self.send(Request::Cmd(Command::Servo(self.servo)));
        }
        ui.add_enabled_ui(!self.servo, |ui| {
            let r = ui.add(egui::Slider::new(&mut self.rods_pcm, -300.0..=300.0).text("rods, pcm"));
            if r.drag_stopped() || (r.changed() && !r.dragged()) {
                self.send(Request::Cmd(Command::RodsPcm(self.rods_pcm)));
            }
        });
        let r = ui.add(egui::Slider::new(&mut self.air, 0.01..=1.0).text("radiator air (×10 MW design)"));
        if r.drag_stopped() || (r.changed() && !r.dragged()) {
            self.send(Request::Cmd(Command::AirFraction(self.air)));
        }
        ui.label(RichText::new("Above a few hundred kW the MSRE's power followed the radiator air flow (TM-0728 §2.4.11): more air, colder salt, more reactivity.").small());
        ui.horizontal_wrapped(|ui| {
            for (l, c) in [
                ("Trip fuel pump", Command::TripFuelPump),
                ("Start fuel pump", Command::StartFuelPump),
                ("Trip coolant pump", Command::TripCoolantPump),
                ("Start coolant pump", Command::StartCoolantPump),
            ] {
                if ui.add(egui::Button::new(l).min_size(Vec2::new(130.0, 36.0))).clicked() {
                    self.send(Request::Cmd(c));
                }
            }
        });

        ui.separator();
        ui.heading("Model (applies on Reset)");
        ui.add(egui::Slider::new(&mut self.config.power_mw, 0.1..=8.0).text("start power, MW"));
        egui::ComboBox::from_label("coefficients")
            .selected_text(match self.config.coefficients {
                Coefficients::Ornl1965 => "ORNL 1965 (TM-1070)",
                Coefficients::MonteCarloN3 => "Monte Carlo N-3 (this workspace)",
            })
            .show_ui(ui, |ui| {
                ui.selectable_value(&mut self.config.coefficients, Coefficients::Ornl1965, "ORNL 1965 (TM-1070)");
                ui.selectable_value(&mut self.config.coefficients, Coefficients::MonteCarloN3, "Monte Carlo N-3 (this workspace)");
            });
        ui.checkbox(&mut self.config.line_model, "TM-1626 line model for the circulating fuel");

        ui.separator();
        ui.heading("Fidelity levels");
        ui.label(RichText::new("1. Frequency response: ORNL-TM-1070's linear model, solved in the frequency domain (the Frequency tab).\n2. Plant transient: the same equations in time, with ORNL-TM-1626's circulating precursors, pumps and radiator (the Plant and Trends tabs).\n3. Full-core Monte Carlo (this workspace's MSRE N-1/N-3 model): supplies the 'Monte Carlo N-3' coefficients; an in-browser tab is planned (gh:#1571).").small());
        ui.label(RichText::new("Sources: ORNL-TM-0728, TM-1070, TM-1626, TM-1647, ORNL-4233 (public reports). Code: outram_park_digital_twin_engine::msre.").small());
    }

    fn to_screen(&self, rect: Rect, x: f64, y: f64) -> Pos2 {
        self.view.to_screen(rect, x, y)
    }

    fn hot(t_f: f64) -> Color32 {
        hot_to_cold_colour_mark_1(((t_f - T_COLD_F) / (T_HOT_F - T_COLD_F)).clamp(0.0, 1.0) as f32)
    }

    fn block(&self, ui: &egui::Ui, rect: Rect, (x0, y0, x1, y1): (f64, f64, f64, f64), fill: Color32, title: &str, lines: &[String]) {
        let p = ui.painter();
        let r = Rect::from_two_pos(self.to_screen(rect, x0, y0), self.to_screen(rect, x1, y1));
        p.rect(r, 6.0, fill, Stroke::new(2.0, ui.visuals().text_color()), StrokeKind::Inside);
        // A font that fits: the longest line across the width, every line
        // (plus the title) down the height.
        let longest = lines.iter().map(|l| l.chars().count()).chain([title.chars().count()]).max().unwrap_or(1) as f32;
        let s = (r.width() / (0.56 * longest + 1.0)).min(r.height() / (1.45 * (lines.len() as f32 + 1.6))).clamp(7.0, 20.0);
        p.text(r.center_top() + Vec2::new(0.0, 0.3 * s), Align2::CENTER_TOP, title, FontId::proportional(s * 1.1), Color32::BLACK);
        for (i, l) in lines.iter().enumerate() {
            p.text(r.center_top() + Vec2::new(0.0, 0.5 * s + s * 1.4 * (i as f32 + 1.0)), Align2::CENTER_TOP, l, FontId::proportional(s), Color32::BLACK);
        }
    }

    /// A pipe through `pts`, coloured by `t_f`, labelled beside its longest
    /// run on the side `out` points to (a unit screen direction).
    fn pipe(&self, ui: &egui::Ui, rect: Rect, pts: &[(f64, f64)], t_f: f64, label: &str, out: Vec2) {
        let w = (self.view.fit_scale(rect) as f32 * 0.35).clamp(3.0, 14.0);
        let screen: Vec<Pos2> = pts.iter().map(|&(x, y)| self.to_screen(rect, x, y)).collect();
        ui.painter().add(egui::Shape::line(screen.clone(), Stroke::new(w, Self::hot(t_f))));
        let longest = screen.windows(2).max_by(|a, b| a[0].distance(a[1]).total_cmp(&b[0].distance(b[1])));
        if let Some(seg) = longest {
            let mid = Pos2::new((seg[0].x + seg[1].x) / 2.0, (seg[0].y + seg[1].y) / 2.0) + out * (w + 4.0);
            let align = if out.x > 0.5 {
                Align2::LEFT_CENTER
            } else if out.x < -0.5 {
                Align2::RIGHT_CENTER
            } else if out.y < 0.0 {
                Align2::CENTER_BOTTOM
            } else {
                Align2::CENTER_TOP
            };
            ui.painter().text(mid, align, label, FontId::proportional(13.0), ui.visuals().text_color());
        }
    }

    fn plant(&self, ui: &egui::Ui, rect: Rect) {
        let Some(s) = self.snap else {
            ui.painter().text(rect.center(), Align2::CENTER_CENTER, "Starting the plant…", FontId::proportional(18.0), ui.visuals().text_color());
            return;
        };
        let (up, down, left, right) = (Vec2::new(0.0, -1.0), Vec2::new(0.0, 1.0), Vec2::new(-1.0, 0.0), Vec2::new(1.0, 0.0));
        let fuel_hot = format!("{:.0} °F", s.core_outlet_f);
        let fuel_cold = format!("{:.0} °F", s.hx_fuel_out_f);
        let cool_hot = format!("{:.0} °F", s.hx_coolant_out_f);
        let cool_cold = format!("{:.0} °F", s.radiator_coolant_out_f);
        // Indicative drawing layout (not plant geometry): left to right on a
        // wide screen, top to bottom on a tall one. World y points up.
        let portrait = rect.height() > rect.width() * 1.2;
        let (core, hx, rad) = if portrait {
            self.pipe(ui, rect, &[(4.5, 6.0), (6.0, 6.0), (6.0, 0.0), (3.2, 0.0)], s.hx_fuel_in_f, &fuel_hot, right);
            self.pipe(ui, rect, &[(-3.2, 0.0), (-6.0, 0.0), (-6.0, 6.0), (-4.5, 6.0)], s.core_inlet_f, &fuel_cold, left);
            self.pipe(ui, rect, &[(1.5, -2.0), (1.5, -4.5)], s.hx_coolant_out_f, &cool_hot, right);
            self.pipe(ui, rect, &[(-1.5, -4.5), (-1.5, -2.0)], s.hx_coolant_in_f, &cool_cold, left);
            ((-4.5, 2.8, 4.5, 9.4), (-3.2, -2.0, 3.2, 2.0), (-4.5, -9.4, 4.5, -4.5))
        } else {
            self.pipe(ui, rect, &[(-5.0, 3.5), (-5.0, 4.8), (0.0, 4.8), (0.0, 2.5)], s.hx_fuel_in_f, &fuel_hot, up);
            self.pipe(ui, rect, &[(0.0, -2.5), (0.0, -4.8), (-5.0, -4.8), (-5.0, -3.5)], s.core_inlet_f, &fuel_cold, down);
            self.pipe(ui, rect, &[(1.5, 1.2), (6.5, 1.2), (6.5, 2.5)], s.hx_coolant_out_f, &cool_hot, up);
            self.pipe(ui, rect, &[(6.5, -2.5), (6.5, -1.2), (1.5, -1.2)], s.hx_coolant_in_f, &cool_cold, down);
            ((-8.5, -3.5, -2.5, 3.5), (-1.5, -2.5, 1.5, 2.5), (4.0, -2.5, 9.5, 2.5))
        };
        self.block(ui, rect, core, Self::hot(s.fuel_f), "Reactor core", &[
            format!("{:.3} MW", s.power_mw),
            format!("fuel {:.0} °F", s.fuel_f),
            format!("graphite {:.0} °F", s.graphite_f),
            format!("ρ {:+.1} pcm", s.reactivity_pcm),
            format!("rods {:+.1} pcm{}", s.rod_pcm, if s.servo != 0.0 { " (servo)" } else { "" }),
            format!("feedback {:+.1} pcm", s.feedback_pcm),
            format!("fuel flow {:.0} %", 100.0 * s.fuel_flow),
        ]);
        self.block(ui, rect, hx, Self::hot(0.5 * (s.hx_fuel_in_f + s.hx_fuel_out_f)), "Heat exchanger", &[
            format!("fuel {:.0} to {:.0} °F", s.hx_fuel_in_f, s.hx_fuel_out_f),
            format!("coolant {:.0} to {:.0} °F", s.hx_coolant_in_f, s.hx_coolant_out_f),
            format!("coolant flow {:.0} %", 100.0 * s.coolant_flow),
        ]);
        self.block(ui, rect, rad, Self::hot(s.radiator_coolant_out_f), "Radiator", &[
            format!("air {:.0} % of design", 100.0 * s.air_flow),
            format!("air out {:.0} °F", s.radiator_air_out_f),
            format!("coolant out {:.0} °F", s.radiator_coolant_out_f),
        ]);
        // Status on the main view (shown with the panel folded).
        let (fuel_frozen, coolant_frozen) = s.below_liquidus();
        let mut y = rect.bottom() - 90.0;
        for (on, what, liq) in [(fuel_frozen, "fuel", FUEL_LIQUIDUS_F), (coolant_frozen, "coolant", COOLANT_LIQUIDUS_F)] {
            if on {
                ui.painter().text(Pos2::new(rect.center().x, y), Align2::CENTER_CENTER, format!("⚠ {what} salt below its {liq:.0} °F liquidus: the real plant would freeze here (the model does not)"), FontId::proportional(14.0), ui.visuals().error_fg_color);
                y -= 20.0;
            }
        }
        // Colour key.
        let k = Rect::from_min_size(Pos2::new(rect.right() - 170.0, rect.bottom() - 120.0), Vec2::new(150.0, 12.0));
        for i in 0..30 {
            let f = i as f32 / 29.0;
            let c = hot_to_cold_colour_mark_1(f);
            let x = k.left() + k.width() * f;
            ui.painter().rect_filled(Rect::from_min_size(Pos2::new(x, k.top()), Vec2::new(k.width() / 29.0 + 1.0, k.height())), 0.0, c);
        }
        ui.painter().text(k.left_bottom() + Vec2::new(0.0, 2.0), Align2::LEFT_TOP, format!("{T_COLD_F:.0} °F"), FontId::proportional(12.0), ui.visuals().text_color());
        ui.painter().text(k.right_bottom() + Vec2::new(0.0, 2.0), Align2::RIGHT_TOP, format!("{T_HOT_F:.0} °F"), FontId::proportional(12.0), ui.visuals().text_color());
    }

    fn trends(&self, ui: &mut egui::Ui) {
        let h = &self.history;
        let line = |f: fn(&Snapshot) -> f64, name: &str| Line::new(name, PlotPoints::from_iter(h.iter().map(|s| [s.time_s, f(s)])));
        let height = ((ui.available_height() - 80.0) / 3.0).max(110.0);
        ui.label("Power, MW");
        Plot::new("power").height(height).include_y(0.0).legend(Legend::default()).show(ui, |p| {
            p.line(line(|s| s.power_mw, "fission power"));
            p.line(line(|s| s.heat_mw, "heat in salt"));
        });
        ui.label("Temperatures, °F");
        Plot::new("temps").height(height).include_y(T_COLD_F).include_y(T_HOT_F).legend(Legend::default()).show(ui, |p| {
            p.line(line(|s| s.core_outlet_f, "core outlet"));
            p.line(line(|s| s.core_inlet_f, "core inlet"));
            p.line(line(|s| s.graphite_f, "graphite"));
            p.line(line(|s| s.radiator_coolant_out_f, "radiator coolant out"));
        });
        ui.label("Reactivity, pcm");
        Plot::new("rho").height(height).include_y(-20.0).include_y(20.0).legend(Legend::default()).show(ui, |p| {
            p.line(line(|s| s.rod_pcm, "rods"));
            p.line(line(|s| s.feedback_pcm, "temperature feedback"));
            p.line(line(|s| s.reactivity_pcm, "net"));
        });
    }

    fn period_view(&mut self, ui: &mut egui::Ui) {
        ui.label(RichText::new("MSRE T-1: the natural period of the power oscillation after a small disturbance, at seven powers. Measured by ORNL (TM-1647, 1966-67) against the linear model (TM-1070), with the temperature coefficients below.").small());
        ui.horizontal_wrapped(|ui| {
            if ui.button("ORNL 1965 coefficients").clicked() {
                self.freq_fuel = ORNL_1965_FUEL_PER_F * 1e5;
                self.freq_graphite = ORNL_1965_GRAPHITE_PER_F * 1e5;
            }
            if ui.button("Monte Carlo N-3 (this workspace)").clicked() {
                self.freq_fuel = MC_N3_FUEL_PER_F * 1e5;
                self.freq_graphite = MC_N3_GRAPHITE_PER_F * 1e5;
            }
        });
        let a = ui.add(egui::Slider::new(&mut self.freq_fuel, -8.0..=-1.0).text("fuel, 1e-5 /°F"));
        let b = ui.add(egui::Slider::new(&mut self.freq_graphite, -8.0..=-1.0).text("graphite, 1e-5 /°F"));
        if a.drag_stopped() || b.drag_stopped() || ui.button("Compute").clicked() || ((a.changed() && !a.dragged()) || (b.changed() && !b.dragged())) {
            let (f, g) = (self.freq_fuel * 1e-5, self.freq_graphite * 1e-5);
            self.send(Request::Periods(f, g));
        }
        let rows = &self.periods;
        Plot::new("periods").height((ui.available_height() - 170.0).max(160.0)).legend(Legend::default()).x_axis_label("power, MW").y_axis_label("period, min").show(ui, |p| {
            p.points(Points::new("measured (TM-1647)", PlotPoints::from_iter(rows.iter().map(|r| [r[0], r[1]]))).radius(5.0));
            p.line(Line::new("model (TM-1070)", PlotPoints::from_iter(rows.iter().filter(|r| r[2].is_finite()).map(|r| [r[0], r[2]]))));
        });
        egui::Grid::new("period_table").striped(true).show(ui, |ui| {
            ui.strong("P, MW");
            ui.strong("measured, min");
            ui.strong("model, min");
            ui.strong("model / measured");
            ui.end_row();
            for r in rows {
                ui.label(format!("{:.3}", r[0]));
                ui.label(format!("{:.2}", r[1]));
                if r[2].is_finite() {
                    ui.label(format!("{:.2}", r[2]));
                    ui.label(format!("{:.3}", r[2] / r[1]));
                } else {
                    ui.label("no resonance");
                    ui.label("–");
                }
                ui.end_row();
            }
        });
    }
}

impl eframe::App for App {
    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        let ctx = ui.ctx().clone();
        let dt = ctx.input(|i| i.stable_dt) as f64;
        self.handle(dt);
        if self.running || self.pending > 0 {
            ctx.request_repaint_after(std::time::Duration::from_millis(30));
        }
        let title = match &self.snap {
            Some(s) => format!("MSRE · {:.0} s · {:.2} MW", s.time_s, s.power_mw),
            None => "MSRE · starting".into(),
        };
        if title != self.title {
            set_title(&ctx, &title);
            self.title = title;
        }
        let mut panel = std::mem::take(&mut self.panel);
        panel.show(ui, "MSRE simulator", |ui| self.side_panel(ui));
        self.panel = panel;
        egui::CentralPanel::default().frame(egui::Frame::NONE.fill(ui.visuals().extreme_bg_color)).show(ui, |ui| {
            let rect = ui.max_rect();
            let bar = Rect::from_min_size(Pos2::new(rect.center().x - 165.0, rect.bottom() - 48.0), Vec2::new(330.0, 40.0));
            match self.tab {
                Tab::Plant => {
                    let resp = ui.interact(rect, ui.id().with("plant"), egui::Sense::click_and_drag());
                    let fit_rect = rect.shrink2(Vec2::new(0.0, 56.0));
                    if !self.view.fitted || (!self.view.touched && self.view.fit_size != fit_rect.size()) {
                        self.view.fit(fit_rect);
                    }
                    self.view.handle_input(ui, &resp);
                    self.plant(ui, rect);
                    if let Some(z) = zoom_buttons(ui, rect) {
                        apply_zoom(&mut self.view, fit_rect, z);
                    }
                }
                Tab::Trends => {
                    let inner = Rect::from_min_max(rect.min + Vec2::new(8.0, 56.0), rect.max - Vec2::new(8.0, 56.0));
                    ui.scope_builder(egui::UiBuilder::new().max_rect(inner), |ui| self.trends(ui));
                }
                Tab::Periods => {
                    let inner = Rect::from_min_max(rect.min + Vec2::new(8.0, 56.0), rect.max - Vec2::new(8.0, 56.0));
                    ui.scope_builder(egui::UiBuilder::new().max_rect(inner), |ui| {
                        egui::ScrollArea::vertical().show(ui, |ui| self.period_view(ui));
                    });
                }
            }
            if let Some(s) = &self.snap {
                let at = if rect.width() < 700.0 { Vec2::new(10.0, 58.0) } else { Vec2::new(if self.panel.open { 8.0 } else { 156.0 }, 14.0) };
                ui.painter().text(rect.left_top() + at, Align2::LEFT_TOP, format!("t = {:.0} s{}", s.time_s, if self.running { "" } else { " (paused)" }), FontId::proportional(15.0), ui.visuals().text_color());
            }
            ui.scope_builder(egui::UiBuilder::new().max_rect(bar), |ui| {
                ui.horizontal(|ui| {
                    for (t, l) in [(Tab::Plant, "Plant"), (Tab::Trends, "Trends"), (Tab::Periods, "Frequency")] {
                        if ui.add(egui::Button::selectable(self.tab == t, RichText::new(l).size(17.0)).min_size(Vec2::new(104.0, 38.0))).clicked() {
                            self.tab = t;
                        }
                    }
                });
            });
            self.panel.reopen_button(ui, rect);
        });
    }
}
