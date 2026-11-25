//! The page: an egui app that only draws (the plant runs in the engine).
//!
//! Built on `bugis::web_demo` (the folding side panel, the main view's zoom
//! buttons, the link to the engine) and the digital-twin engine's hot-to-cold
//! colour map. Four views: the plant, the trends, the T-1 frequency
//! response, and the fuel loop's precursors from Moltres (gh:#1575).
//!
//! ~~**The schematic's layout is an indicative drawing, not plant geometry**:
//! boxes placed to read left to right as the salt flows.~~ **CHANGED
//! 2026-11-14:** the plant view is `bugis::components::MsrePlantSchematic`,
//! an elevation view drawn from TM-0728's stated sizes and elevations (its
//! module docs list what is dimensioned and what is indicative). Colours and
//! numbers are the model's; nothing on it moves, so the animation rule has
//! nothing to derive (flows are shown as numbers and arrows). The Moltres
//! ring's colours are the drift's emission, so they move only as the
//! physics moves them.
//!
//! GUI drawing code: reached by no test (needs a window), per the workspace
//! exception for GUI code. What it draws is tested in `msre::session` and
//! `engine::tests`.

use crate::engine::{freeze_from_slice, Event, Request, DEFAULT_SPEED};
use bugis::web_demo::link::Link;
use bugis::web_demo::platform::{query_pairs, set_title};
use bugis::web_demo::view::{apply_zoom, View};
use outram_park_digital_twin_engine::twin::shell::{ShellAt, ShellHost, TwinShell};
use outram_park_digital_twin_engine::twin::{TwinPage, ViewState};
use bugis::components::{MsreLayout, MsrePlantScalars, MsrePlantSchematic};
use egui::{Align2, Color32, FontId, Pos2, Rect, RichText, Stroke, Vec2};
use outram_park_digital_twin_engine::msre::moltres_drift::{LoopDrift, LOOP_DRIFT_CELLS};
use petir::mathf::RealMath;
use uom::si::f64::{Power, Ratio, ThermodynamicTemperature};
use uom::si::power::megawatt;
use uom::si::ratio::ratio;
use uom::si::thermodynamic_temperature::degree_fahrenheit;
use egui_plot::{Legend, Line, Plot, PlotPoints, Points};
use outram_park_digital_twin_engine::color_maps::hot_to_cold_colour_mark_1;
use outram_park_digital_twin_engine::msre::session::{
    Coefficients, Command, Freeze, SessionConfig, Snapshot, COOLANT_LIQUIDUS_F, FUEL_LIQUIDUS_F,
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

/// The MSRE simulator's pages on the standard twin shell (gh:#1757): the
/// shell owns the Gruvbox theme, the left panel, the page bar and the
/// greying; this app fills the slots ([`ShellHost`]). Plant is the plant,
/// Graphs the trends, Kinetics the Moltres precursor drift, Freq. the
/// frequency response, MC the full-core k_eff. Nodes and CFD are greyed:
/// the browser page has no control-volume view yet (the native twin has,
/// `msre::twin`; gh:#1757) and no CFD model exists (recorded on gh:#1743).
pub struct App {
    shell: TwinShell,
    link: Link<Request, Event>,
    view: View,
    /// The plant view's fit rect, for the shell's zoom buttons.
    plant_fit: Rect,
    snap: Option<Snapshot>,
    history: Vec<Snapshot>,
    /// The latest Moltres drift event (`engine::Event::Drift`'s layout).
    drift: Option<Vec<f64>>,
    /// `[time s, loss %]` from the drift, for its trend.
    drift_history: Vec<[f64; 2]>,
    periods: Vec<[f64; 3]>,
    /// The salt freeze the engine is holding the plant at (gh:#1573).
    freeze: Option<Freeze>,
    /// The Core k_eff tab (gh:#1571).
    keff: crate::keff_tab::KeffTab,
    /// The coefficients the running plant was reset with (the panel's
    /// choice applies on Reset; gh:#1574 shows which the plant uses).
    plant_coefficients: Coefficients,
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
            shell: TwinShell::default(),
            link,
            plant_fit: Rect::NOTHING,
            // The plant drawing (bugis MsrePlantSchematic) spans 63 x 40 ft,
            // centred on the origin; the half-extent is refitted to the
            // screen's aspect before each fit.
            view: View::new(0.5 * bugis::components::msre_plant::EXTENT_FT.0 as f64).with_home([0.0, 0.0]),
            // `?tab=keff` opens on the k_eff tab (an unattended browser
            // check, as `?krun`; gh:#1661). Any other value: Plant.
            snap: None,
            history: Vec::new(),
            drift: None,
            drift_history: Vec::new(),
            periods: Vec::new(),
            freeze: None,
            keff: crate::keff_tab::KeffTab::default(),
            plant_coefficients: SessionConfig::default().coefficients,
            // The plant runs from page load (gh:#1573); `?paused` opens it
            // paused.
            running: !q.iter().any(|(k, _)| k == "paused"),
            speed: DEFAULT_SPEED,
            config: SessionConfig::default(),
            rods_pcm: 0.0,
            air: 0.8,
            servo: false,
            freq_fuel: ORNL_1965_FUEL_PER_F * 1e5,
            freq_graphite: ORNL_1965_GRAPHITE_PER_F * 1e5,
            error: None,
            title: String::new(),
        };
        // `?tab=keff` opens on the Monte Carlo page (an unattended browser
        // check, as `?krun`; gh:#1661). Any other value: Plant.
        if q.iter().any(|(k, v)| k == "tab" && v == "keff") {
            app.shell.page = TwinPage::MonteCarlo;
        }
        app.send(Request::Reset(app.config));
        app.send(Request::Periods(app.freq_fuel * 1e-5, app.freq_graphite * 1e-5));
        app.send_pace();
        app
    }

    /// Tell the engine the speed and whether paused: it keeps its own clock.
    fn send_pace(&mut self) {
        self.send(Request::Pace(self.speed, !self.running));
    }

    fn send(&mut self, r: Request) {
        self.link.send(r);
    }

    fn reset(&mut self) {
        self.history.clear();
        self.drift_history.clear();
        self.rods_pcm = 0.0;
        self.servo = false;
        self.freeze = None;
        self.plant_coefficients = self.config.coefficients;
        self.air = self.config.power_mw / 10.0;
        self.send(Request::Reset(self.config));
    }

    /// Take what the engine posted. The engine paces the plant itself
    /// (gh:#1573): the page never asks for a step, it only draws frames.
    fn handle(&mut self) {
        for e in self.link.drain() {
            match e {
                Event::Drift(f) => {
                    let v = f.to_vec();
                    if v.len() >= 2 && v[1].is_finite() && self.drift_history.last().map_or(true, |h| v[0] > h[0]) {
                        self.drift_history.push([v[0], 100.0 * v[1]]);
                        if self.drift_history.len() > HISTORY_MAX {
                            self.drift_history = self.drift_history.iter().step_by(2).copied().collect();
                        }
                    }
                    self.drift = Some(v);
                }
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
                Event::Frozen(f) => self.freeze = freeze_from_slice(&f.to_vec()),
                Event::Keff(crate::keff::KEv::Raster { view, w, h, ids }) => self.keff.on_raster(view, w, h, ids),
                Event::Keff(_) => {}
                Event::Error(m) => self.error = Some(m),
            }
        }
    }

    /// The freeze modal (gh:#1573, after `fhr_sim_v2`'s): which salt, where,
    /// how cold, and a thaw stated plainly as a cheat. GUI drawing code
    /// (needs a window); the freeze it shows is tested in `msre::session`.
    fn freeze_modal(&mut self, ctx: &egui::Context) {
        let Some(f) = self.freeze else { return };
        let mut thaw = false;
        let mut reset = false;
        egui::Window::new("Salt freeze: the plant is held")
            .collapsible(false)
            .resizable(false)
            .anchor(Align2::CENTER_CENTER, Vec2::ZERO)
            .show(ctx, |ui| {
                ui.set_max_width(420.0);
                ui.label(RichText::new(f.headline()).strong());
                ui.label(format!("The simulation stopped at t = {:.0} s, the last moment every salt was still liquid, and is held there. The model has no freezing, so it cannot go on past this point honestly.", f.held_at_s));
                ui.label(RichText::new("A frozen salt loop is a real molten-salt-reactor hazard: the MSRE's operators closed the radiator doors after a pump trip so the coolant would not freeze.").small());
                ui.separator();
                ui.label(RichText::new("The thaw button is a CHEAT. It puts the plant back in its hot starting state at once. A real loop is thawed by trace heating over hours to days, with a real risk of plugs and of never recovering it; none of that is modelled.").color(ui.visuals().warn_fg_color));
                ui.horizontal(|ui| {
                    thaw = ui.add(egui::Button::new(RichText::new("Thaw (a cheat)").size(17.0)).min_size(Vec2::new(150.0, 38.0))).clicked();
                    reset = ui.add(egui::Button::new(RichText::new("Reset").size(17.0)).min_size(Vec2::new(100.0, 38.0))).clicked();
                });
            });
        if thaw {
            self.freeze = None;
            self.servo = false;
            self.rods_pcm = 0.0;
            self.send(Request::Thaw);
        } else if reset {
            self.reset();
        }
    }

    fn side_panel(&mut self, ui: &mut egui::Ui, page: TwinPage) {
        if page == TwinPage::MonteCarlo {
            self.keff.panel(ui);
            ui.separator();
        }
        ui.label(RichText::new("Museum · historical reactors").strong());
        ui.label("The Molten-Salt Reactor Experiment (ORNL, critical 1965, shut down 1969): an 8 MW graphite-moderated reactor whose fuel was a molten fluoride salt, pumped through the core and a heat exchanger.");
        ui.label(RichText::new("The author's hobby project, built out of curiosity in holidays and free time; not particularly related to their work, which is on LWRs and HTGRs. Good for learning; an AI-assisted draft, NOT validated. Research and education only; never for operation, licensing or safety decisions.").small().color(ui.visuals().warn_fg_color));
        if let Some(e) = &self.error {
            ui.colored_label(ui.visuals().error_fg_color, e);
        }
        ui.separator();

        ui.heading("Run");
        ui.horizontal(|ui| {
            let label = if self.running { "⏸ Pause" } else { "▶ Run" };
            if ui.add(egui::Button::new(RichText::new(label).size(18.0)).min_size(Vec2::new(110.0, 40.0))).clicked() {
                self.running = !self.running;
                self.send_pace();
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
                        self.send_pace();
                    }
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
                Coefficients::Measured { .. } => "measured in this browser (k_eff tab)",
            })
            .show_ui(ui, |ui| {
                ui.selectable_value(&mut self.config.coefficients, Coefficients::Ornl1965, "ORNL 1965 (TM-1070)");
                ui.selectable_value(&mut self.config.coefficients, Coefficients::MonteCarloN3, "Monte Carlo N-3 (this workspace)");
                if let Some(m) = self.keff.measured() {
                    ui.selectable_value(&mut self.config.coefficients, m, "measured in this browser (k_eff tab)");
                }
            });
        ui.checkbox(&mut self.config.line_model, "TM-1626 line model for the circulating fuel");

        ui.separator();
        ui.heading("Fidelity levels");
        ui.label(RichText::new("1. Frequency response: ORNL-TM-1070's linear model, solved in the frequency domain (the Frequency tab).\n2. Plant transient: the same equations in time, with ORNL-TM-1626's circulating precursors, pumps and radiator (the Plant and Trends tabs).\n3. Full-core Monte Carlo (this workspace's MSRE N-1/N-3 model): supplies the 'Monte Carlo N-3' coefficients, and runs in your browser on the k_eff tab.").small());
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
        // The engine never shows a salt below its liquidus (it holds the
        // plant first, gh:#1573): the drawing marks the salt that froze.
        let (fuel_frozen, coolant_frozen) = match self.freeze {
            Some(f) => (f.location.is_fuel(), !f.location.is_fuel()),
            None => s.below_liquidus(),
        };
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
            format!("coefficients: {}", self.plant_coefficients.describe()),
            format!("{:.2} MW · ρ {:+.1} pcm: rods {:+.1}{}, feedback {:+.1}", s.power_mw, s.reactivity_pcm, s.rod_pcm, if s.servo != 0.0 { " (servo)" } else { "" }, s.feedback_pcm),
            format!("fuel {:.0} °F, graphite {:.0} °F; flows: fuel {:.0} %, coolant {:.0} %", s.fuel_f, s.graphite_f, 100.0 * s.fuel_flow, 100.0 * s.coolant_flow),
        ];
        let top = rect.top() + if rect.width() < 700.0 { 80.0 } else { 40.0 };
        for (k, l) in lines.iter().enumerate() {
            ui.painter().text(Pos2::new(rect.center().x, top + 18.0 * k as f32), Align2::CENTER_TOP, l, FontId::proportional(13.0), ui.visuals().text_color());
        }
        // Status on the main view (shown with the panel folded).
        let mut y = rect.bottom() - 90.0;
        for (on, what, liq) in [(fuel_frozen, "fuel", FUEL_LIQUIDUS_F), (coolant_frozen, "coolant", COOLANT_LIQUIDUS_F)] {
            if on {
                ui.painter().text(Pos2::new(rect.center().x, y), Align2::CENTER_CENTER, format!("⚠ {what} salt at its {liq:.0} °F liquidus: the plant is held (it would freeze)"), FontId::proportional(14.0), ui.visuals().error_fg_color);
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

    /// The Moltres tab: delayed-neutron emission around the fuel loop as a
    /// coloured ring (gh:#1575), the loss now against the steady and the
    /// measured values, and its trend. GUI drawing code (needs a window).
    fn loop_view(&self, ui: &mut egui::Ui, rect: Rect) {
        let text = ui.visuals().text_color();
        let muted = ui.visuals().weak_text_color();
        ui.label(RichText::new("Delayed neutrons around the fuel loop: Moltres's precursor drift (finite volumes, 504 cells) stepped with the plant at its fuel flow and power. Trip the fuel pump to watch the precursors stay in the core.").small());
        let Some(d) = self.drift.as_ref() else {
            ui.label("Waiting for the drift…");
            return;
        };
        let bins = &d[3.min(d.len())..];
        // On a phone the readouts go under the ring (they are wider than its
        // hole) and the region names are shortened to stay on screen.
        let narrow = rect.width() < 600.0;
        let top = ui.cursor().top() + 8.0;
        let plot_h = (rect.height() * 0.28).clamp(110.0, 220.0);
        let text_h = if narrow { 84.0 } else { 0.0 };
        let ring_area = Rect::from_min_max(Pos2::new(rect.left(), top), Pos2::new(rect.right(), rect.bottom() - plot_h - 24.0 - text_h));
        let r = if narrow { (0.27 * rect.width()).min(0.36 * ring_area.height()) } else { 0.36 * ring_area.width().min(ring_area.height()) };
        let c = ring_area.center();
        let painter = ui.painter().with_clip_rect(rect);
        let at = |f: f64, rad: f32| {
            let a = (-0.5 * std::f64::consts::PI + 2.0 * std::f64::consts::PI * f) as f32;
            c + Vec2::new(a.r_cos(), a.r_sin()) * rad
        };
        if bins.is_empty() {
            painter.text(c, Align2::CENTER_CENTER, "Moltres drift stopped (check the console)", FontId::proportional(15.0), ui.visuals().error_fg_color);
            return;
        }
        // The ring: each arc coloured by its emission on the shared map,
        // scaled to the hottest arc.
        let max = bins.iter().copied().fold(0.0_f64, f64::max).max(1e-300);
        let n = bins.len();
        for (i, e) in bins.iter().enumerate() {
            let col = hot_to_cold_colour_mark_1((e / max).clamp(0.0, 1.0) as f32);
            let seg: Vec<Pos2> = (0..=4).map(|k| at((i as f64 + k as f64 / 4.0) / n as f64, r)).collect();
            painter.add(egui::Shape::line(seg, Stroke::new(0.22 * r, col)));
        }
        // Region boundaries and names (TM-1626 Table 1's residence times).
        let starts = LoopDrift::region_starts();
        let names = if narrow {
            ["lower plenum", "core", "upper plenum", "ext. loop"]
        } else {
            ["lower plenum", "core (graphite)", "upper plenum", "external loop: pump, heat exchanger"]
        };
        for (k, s0) in starts.iter().enumerate() {
            painter.line_segment([at(*s0, 0.8 * r), at(*s0, 1.2 * r)], Stroke::new(2.0, text));
            let s1 = if k + 1 < starts.len() { starts[k + 1] } else { 1.0 };
            let mid = at(0.5 * (s0 + s1), if narrow { 1.25 } else { 1.32 } * r);
            let align = if mid.x < c.x - 0.2 * r { Align2::RIGHT_CENTER } else if mid.x > c.x + 0.2 * r { Align2::LEFT_CENTER } else { Align2::CENTER_CENTER };
            painter.text(mid, align, names[k], FontId::proportional(13.0), text);
        }
        // Flow direction: an arrowhead on the ring, by the fuel flow's sign.
        if d[2] > 1e-6 {
            let (p0, p1) = (at(0.0, 0.62 * r), at(0.03, 0.62 * r));
            painter.arrow(p0, p1 - p0, Stroke::new(2.0, muted));
        }
        let loss = if d[1].is_finite() { format!("{:.4} % δk/k", 100.0 * d[1]) } else { "no power".to_owned() };
        for (k, (s, size)) in [
            (format!("lost to circulation: {loss}"), 15.0),
            (format!("fuel flow {:.0} %", 100.0 * d[2]), 13.0),
            (format!("steady, full flow: {:.4} % (Moltres, {LOOP_DRIFT_CELLS} cells)", 0.229218), 12.0),
            ("measured (TM-1626): 0.212 ± 0.004 %".to_owned(), 12.0),
        ]
        .iter()
        .enumerate()
        {
            let anchor = if narrow { Pos2::new(c.x, c.y + 1.45 * r + 10.0) } else { c + Vec2::new(0.0, -1.5 * 19.0) };
            painter.text(anchor + Vec2::new(0.0, k as f32 * 19.0), Align2::CENTER_CENTER, s, FontId::proportional(*size), if k == 0 { text } else { muted });
        }
        let plot_rect = Rect::from_min_max(Pos2::new(rect.left(), rect.bottom() - plot_h), rect.max);
        ui.scope_builder(egui::UiBuilder::new().max_rect(plot_rect), |ui| {
            Plot::new("drift_loss").height(plot_h - 4.0).include_y(0.0).show(ui, |p| {
                p.line(Line::new("loss, % δk/k", PlotPoints::from_iter(self.drift_history.iter().copied())).color(Color32::from_rgb(214, 93, 14)));
            });
        });
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
        // Each engine event repaints the page (the link's callback), so the
        // page needs no timer of its own.
        self.handle();
        let _finished = self.keff.pump(&ctx, &self.link);
        // "Send to plant" (gh:#1574): only the reader's button does this. The
        // running plant switches to them with its state kept (the htgr_sim_v1
        // way, `MsreModel::set_temperature_coefficients`); no reset.
        if let Some(c) = self.keff.take_send() {
            self.config.coefficients = c;
            self.plant_coefficients = c;
            self.send(Request::Cmd(Command::Coefficients(c)));
        }
        let title = match &self.snap {
            Some(s) => format!("MSRE · {:.0} s · {:.2} MW", s.time_s, s.power_mw),
            None => "MSRE · starting".into(),
        };
        if title != self.title {
            set_title(&ctx, &title);
            self.title = title;
        }
        let mut shell = std::mem::take(&mut self.shell);
        shell.ui(ui, self);
        self.shell = shell;
        self.freeze_modal(&ctx);
    }
}

impl ShellHost for App {
    fn title(&self) -> String {
        "MSRE simulator".into()
    }

    fn page_state(&self, page: TwinPage) -> ViewState {
        match page {
            TwinPage::Plant | TwinPage::Graphs | TwinPage::Kinetics | TwinPage::Surrogate | TwinPage::MonteCarlo => ViewState::Built,
            TwinPage::ControlVolumes => ViewState::NotBuilt { issue: 1757 },
            TwinPage::Cfd => ViewState::NotBuilt { issue: 1743 },
        }
    }

    fn left(&mut self, ui: &mut egui::Ui, at: ShellAt) {
        self.side_panel(ui, at.page);
    }

    fn zoomable(&self, at: ShellAt) -> bool {
        at.page == TwinPage::Plant
    }

    fn zoom(&mut self, _at: ShellAt, z: bugis::web_demo::view::Zoom) {
        apply_zoom(&mut self.view, self.plant_fit, z);
    }

    fn central(&mut self, ui: &mut egui::Ui, rect: Rect, at: ShellAt) {
        ui.painter().rect_filled(rect, 0.0, ui.visuals().extreme_bg_color);
        // The status strip, on every page: time, power, run state.
        let strip = 24.0;
        if let Some(s) = &self.snap {
            let state = if self.freeze.is_some() {
                " · HELD: salt freeze"
            } else if self.running {
                ""
            } else {
                " (paused)"
            };
            let colour = if self.freeze.is_some() { ui.visuals().error_fg_color } else { ui.visuals().text_color() };
            ui.painter().text(rect.left_top() + Vec2::new(10.0, 4.0), Align2::LEFT_TOP, format!("t = {:.0} s · {:.2} MW{state}", s.time_s, s.power_mw), FontId::proportional(15.0), colour);
        }
        let below = Rect::from_min_max(rect.min + Vec2::new(0.0, strip), rect.max);
        let inner = below.shrink2(Vec2::new(8.0, 4.0));
        match at.page {
            TwinPage::Plant => {
                let resp = ui.interact(below, ui.id().with("plant"), egui::Sense::click_and_drag());
                let fit_rect = below;
                self.plant_fit = fit_rect;
                if !self.view.fitted || (!self.view.touched && self.view.fit_size != fit_rect.size()) {
                    self.view.half_extent = MsreLayout::half_extent_to_fit(fit_rect.size());
                    self.view.fit(fit_rect);
                }
                self.view.handle_input(ui, &resp);
                self.plant(ui, below);
            }
            TwinPage::Graphs => {
                ui.scope_builder(egui::UiBuilder::new().max_rect(inner), |ui| self.trends(ui));
            }
            TwinPage::Kinetics => {
                ui.scope_builder(egui::UiBuilder::new().max_rect(inner), |ui| self.loop_view(ui, inner));
            }
            TwinPage::MonteCarlo => {
                ui.scope_builder(egui::UiBuilder::new().max_rect(inner), |ui| self.keff.view_ui(ui, inner));
            }
            TwinPage::Surrogate => {
                ui.scope_builder(egui::UiBuilder::new().max_rect(inner), |ui| {
                    egui::ScrollArea::vertical().show(ui, |ui| self.period_view(ui));
                });
            }
            // Greyed (page_state): the shell draws their note, not this.
            TwinPage::ControlVolumes | TwinPage::Cfd => {}
        }
    }
}
