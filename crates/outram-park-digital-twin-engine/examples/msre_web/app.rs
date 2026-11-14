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
use bugis::components::{MsreLayout, MsrePlantScalars, MsrePlantSchematic};
use egui::{Align2, FontId, Pos2, Rect, RichText, Vec2};
use uom::si::f64::{Power, Ratio, ThermodynamicTemperature};
use uom::si::power::megawatt;
use uom::si::ratio::ratio;
use uom::si::thermodynamic_temperature::degree_fahrenheit;
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
            // The plant drawing (bugis MsrePlantSchematic) spans 63 x 40 ft,
            // centred on the origin; the half-extent is refitted to the
            // screen's aspect before each fit.
            view: View::new(0.5 * bugis::components::msre_plant::EXTENT_FT.0 as f64).with_home([0.0, 0.0]),
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

    fn plant(&self, ui: &egui::Ui, rect: Rect) {
        let Some(s) = self.snap else {
            ui.painter().text(rect.center(), Align2::CENTER_CENTER, "Starting the plant…", FontId::proportional(18.0), ui.visuals().text_color());
            return;
        };
        // The plant elevation view from the widget library (bugis), drawn
        // from TM-0728's stated dimensions; the same landscape layout on
        // every screen, so the drain tanks stay below the reactor.
        let ext = MsreLayout::extent();
        let (hw, hh) = (0.5 * ext.x as f64, 0.5 * ext.y as f64);
        let at = Rect::from_two_pos(self.view.to_screen(rect, -hw, hh), self.view.to_screen(rect, hw, -hh));
        let f = ThermodynamicTemperature::new::<degree_fahrenheit>;
        let (fuel_frozen, coolant_frozen) = s.below_liquidus();
        let scalars = MsrePlantScalars {
            power: Power::new::<megawatt>(s.power_mw),
            core_fuel: f(s.fuel_f),
            core_outlet: f(s.core_outlet_f),
            core_inlet: f(s.core_inlet_f),
            graphite: f(s.graphite_f),
            coolant_hot: f(s.hx_coolant_out_f),
            coolant_cold: f(s.radiator_coolant_out_f),
            radiator_air_out: f(s.radiator_air_out_f),
            fuel_flow: Ratio::new::<ratio>(s.fuel_flow),
            coolant_flow: Ratio::new::<ratio>(s.coolant_flow),
            air_flow: Ratio::new::<ratio>(s.air_flow),
            fuel_frozen,
            coolant_frozen,
            freeze_valve_frozen: true,
            cold: f(T_COLD_F),
            hot: f(T_HOT_F),
        };
        MsrePlantSchematic::new(scalars).paint(&ui.painter().with_clip_rect(rect), at);
        // Readouts the drawing does not carry.
        let lines = [
            format!("{:.2} MW · ρ {:+.1} pcm: rods {:+.1}{}, feedback {:+.1}", s.power_mw, s.reactivity_pcm, s.rod_pcm, if s.servo != 0.0 { " (servo)" } else { "" }, s.feedback_pcm),
            format!("fuel {:.0} °F, graphite {:.0} °F; flows: fuel {:.0} %, coolant {:.0} %", s.fuel_f, s.graphite_f, 100.0 * s.fuel_flow, 100.0 * s.coolant_flow),
        ];
        let top = rect.top() + if rect.width() < 700.0 { 80.0 } else { 40.0 };
        for (k, l) in lines.iter().enumerate() {
            ui.painter().text(Pos2::new(rect.center().x, top + 18.0 * k as f32), Align2::CENTER_TOP, l, FontId::proportional(13.0), ui.visuals().text_color());
        }
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
                    // The whole elevation view fits fit_rect, wide screen or
                    // phone. `View::handle_input` refits against the full
                    // `rect`, so scale the half-extent to give the same
                    // scale there.
                    let min_side = |r: Rect| r.width().min(r.height()).max(1.0) as f64;
                    self.view.half_extent = MsreLayout::half_extent_to_fit(fit_rect.size()) * min_side(rect) / min_side(fit_rect);
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
