//! The page: an egui app that only draws (the engine is elsewhere).
//!
//! Built on `dhoby_ghaut::web_demo` (the side panel that folds, the main
//! view's zoom buttons, the link to the engine) and drawn with the
//! digital-twin engine's widget library (`PipeVisual`, `ValveVisual`,
//! `InstrumentationVisual`, `TemperatureLegend`, its hot-to-cold colour map).
//! The vessel cut-away, the core map and the Mark I containment are painted
//! here: the library has no BWR vessel or containment widget yet.
//!
//! GUI drawing code: reached by no test (needs a window), per the workspace
//! exception for GUI code; the model it draws is tested in `physics`.

use crate::engine::{decode_frame, Control, Event, Request, SAMPLE_LEN};
use crate::physics::core::RINGS;
use crate::physics::params::*;
use crate::physics::{Frame, Mode, EVENTS};
use egui::{Align2, Color32, FontId, Pos2, Rect, RichText, Stroke, StrokeKind, Vec2};
use dhoby_ghaut::web_demo::link::Link;
use dhoby_ghaut::web_demo::panel::Panel;
use dhoby_ghaut::web_demo::platform::{query_pairs, query_value, set_title};
use dhoby_ghaut::web_demo::view::{apply_zoom, zoom_buttons, View};
use outram_park_digital_twin_engine::animation::residence_time_from_flow;
use outram_park_digital_twin_engine::color_maps::hot_to_cold_colour_mark_1;
use outram_park_digital_twin_engine::components::{
    InstrumentationVisual, PipeScalars, PipeVisual, TemperatureLegend, ValveVisual,
};
use outram_park_fork_dwsim_libs::valve::iec_60534::{OpeningCharacteristic, ValveFlowCoefficient};
use tampines::components::Valve;
use uom::si::f64::{Mass, MassRate, Ratio, ThermodynamicTemperature};
use uom::si::mass::kilogram;
use uom::si::mass_rate::kilogram_per_second;
use uom::si::ratio::ratio;
use uom::si::thermodynamic_temperature::kelvin;

/// The colour scale of the core map and the vessel, K.
const T_MIN: f64 = 300.0;
const T_MAX: f64 = 2400.0;
/// Speeds offered (simulated seconds per wall second; 0 = as fast as it goes).
const SPEEDS: [(f64, &str); 6] = [(1.0, "1x (real time)"), (10.0, "10x"), (60.0, "1 min/s"), (600.0, "10 min/s"), (3600.0, "1 h/s"), (0.0, "as fast as it goes")];

#[derive(Clone, Copy, PartialEq, Eq)]
enum Tab {
    Plant,
    Plots,
}

pub struct App {
    link: Link<Request, Event>,
    panel: Panel,
    view: View,
    tab: Tab,
    frame: Option<Frame>,
    history: Vec<[f64; SAMPLE_LEN]>,
    stats: [f64; 4],
    waiting: bool,
    running: bool,
    speed: f64,
    workers: u8,
    mode: Mode,
    error: Option<String>,
    title: String,
}

fn colour(t_k: f64) -> Color32 {
    hot_to_cold_colour_mark_1(((t_k - T_MIN) / (T_MAX - T_MIN)) as f32)
}

fn temp(t_k: f64) -> ThermodynamicTemperature {
    ThermodynamicTemperature::new::<kelvin>(t_k)
}

/// The clock: the scram was at 14:46 on 11 March 2011 (JST).
pub fn clock(t_s: f64) -> String {
    let total_min = (14.0 * 60.0 + 46.0 + t_s / 60.0).floor() as i64;
    let day = 11 + total_min / (24 * 60);
    let m = total_min % (24 * 60);
    let h = (t_s / 3600.0).floor();
    let mm = ((t_s - h * 3600.0) / 60.0).floor();
    format!("{:02}:{:02} {} Mar · t = {} h {:02} min", m / 60, m % 60, day, h, mm)
}

impl App {
    /// The app, with the URL query (natively, `--key value` arguments) as
    /// options: `run` starts at once, `speed=<sim s per wall s, 0 = max>`,
    /// `workers=1|3`, `mode=free`.
    pub fn new(link: Link<Request, Event>) -> Self {
        let mut app = Self::bare(link);
        let q = query_pairs();
        if let Some(v) = query_value(&q, "speed").and_then(|v| v.parse::<f64>().ok()) {
            app.speed = v;
            app.link.send(Request::Speed(v));
        }
        if let Some(n) = query_value(&q, "workers").and_then(|v| v.parse::<u8>().ok()) {
            app.workers = if n >= 3 { 3 } else { 1 };
            app.link.send(Request::Workers(app.workers));
        }
        if query_value(&q, "mode") == Some("free") {
            app.mode = Mode::FreePlay;
            app.link.send(Request::Mode(Mode::FreePlay));
        }
        if q.iter().any(|(k, _)| k == "run") {
            app.running = true;
            app.link.send(Request::Run(true));
        }
        app
    }

    fn bare(link: Link<Request, Event>) -> Self {
        Self {
            link,
            panel: Panel::default(),
            view: View::new(17.0).with_home([0.0, 9.0]),
            tab: Tab::Plant,
            frame: None,
            history: Vec::new(),
            stats: [0.0; 4],
            waiting: false,
            running: false,
            speed: 600.0,
            workers: 3,
            mode: Mode::Historical,
            error: None,
            title: String::new(),
        }
    }

    fn send(&mut self, r: Request) {
        self.link.send(r);
    }

    fn handle(&mut self) {
        for e in self.link.drain() {
            match e {
                Event::Frame { frame, history, stats } => {
                    self.frame = Some(decode_frame(&frame.to_vec()));
                    let h = history.to_vec();
                    for row in h.chunks_exact(SAMPLE_LEN) {
                        let mut a = [0.0; SAMPLE_LEN];
                        a.copy_from_slice(row);
                        self.history.push(a);
                    }
                    self.stats = stats;
                    self.waiting = false;
                }
                Event::Error(m) => {
                    self.error = Some(m);
                    self.waiting = false;
                }
            }
        }
        if !self.waiting {
            self.send(Request::Tick);
            self.waiting = true;
        }
    }

    fn side_panel(&mut self, ui: &mut egui::Ui) {
        ui.label("Fukushima Daiichi Unit 1 (GE BWR/3, Mark I) after the 11 March 2011 earthquake: station blackout, core damage, venting.");
        ui.label(RichText::new("AI draft, not validated. Research and education only; never for operation, licensing, safety decisions or emergency response.").small().color(Color32::from_rgb(255, 190, 120)));
        if let Some(e) = &self.error {
            ui.colored_label(Color32::LIGHT_RED, e);
        }
        ui.separator();
        ui.heading("Run");
        ui.horizontal(|ui| {
            let label = if self.running { "⏸ Pause" } else { "▶ Run" };
            if ui.add(egui::Button::new(RichText::new(label).size(18.0)).min_size(Vec2::new(110.0, 40.0))).clicked() {
                self.running = !self.running;
                self.send(Request::Run(self.running));
            }
            if ui.add(egui::Button::new(RichText::new("Reset").size(18.0)).min_size(Vec2::new(90.0, 40.0))).clicked() {
                self.history.clear();
                self.send(Request::Reset);
            }
        });
        ui.label("Speed");
        egui::ComboBox::from_id_salt("speed").selected_text(SPEEDS.iter().find(|s| s.0 == self.speed).map_or("?", |s| s.1)).show_ui(ui, |ui| {
            for (v, l) in SPEEDS {
                if ui.selectable_label(self.speed == v, l).clicked() {
                    self.speed = v;
                    self.link.send(Request::Speed(v));
                }
            }
        });
        ui.horizontal(|ui| {
            ui.label("Workers");
            for n in [1u8, 3] {
                if ui.selectable_label(self.workers == n, if n == 1 { "1 (all inline)" } else { "3 (plant, core, containment)" }).clicked() {
                    self.workers = n;
                    self.link.send(Request::Workers(n));
                }
            }
        });
        ui.label(RichText::new(format!(
            "{:.0} steps/s · {:.2} ms/step · {} worker(s){}",
            self.stats[0],
            self.stats[1],
            self.stats[3],
            if self.stats[2] > 1.0 { format!(" · behind the clock by {:.0} s", self.stats[2]) } else { String::new() }
        )).small());
        ui.separator();
        ui.heading("Who acts");
        ui.horizontal(|ui| {
            for (m, l) in [(Mode::Historical, "The recorded sequence"), (Mode::FreePlay, "You (free play)")] {
                if ui.selectable_label(self.mode == m, l).clicked() {
                    self.mode = m;
                    self.link.send(Request::Mode(m));
                }
            }
        });
        let free = self.mode == Mode::FreePlay;
        if let Some(f) = self.frame.clone() {
            let c = f.controls;
            ui.add_enabled_ui(free, |ui| {
                let toggle = |ui: &mut egui::Ui, ctl: Control, on: bool, label: &str| {
                    let mut v = on;
                    if ui.add(egui::Checkbox::new(&mut v, RichText::new(label).size(16.0))).changed() {
                        self.link.send(Request::Set(ctl, v));
                    }
                };
                toggle(ui, Control::DcPower, c.dc_power, "DC power (batteries)");
                toggle(ui, Control::IcA, c.ic_open[0], "Isolation condenser A valves open");
                toggle(ui, Control::IcB, c.ic_open[1], "Isolation condenser B valves open");
                toggle(ui, Control::ReliefHeldOpen, c.relief_held_open, "Hold relief valves open (needs DC)");
                toggle(ui, Control::Injection, c.injection, "Fire-engine injection");
                toggle(ui, Control::Vent, c.vent_ordered, "Wetwell vent ordered");
            });
            if !free {
                ui.label(RichText::new("The recorded operator actions drive these. Switch to free play to act yourself from this moment.").small());
            }
            ui.separator();
            ui.heading("Plant now");
            egui::Grid::new("readouts").striped(true).show(ui, |ui| {
                let mut row = |k: &str, v: String| {
                    ui.label(k);
                    ui.label(v);
                    ui.end_row();
                };
                row("Time", clock(f.t_s));
                row("Decay heat", format!("{:.1} MW ({:.2} % of rated)", f.decay_w / 1e6, 100.0 * f.decay_w / THERMAL_POWER_W));
                row("RPV pressure", format!("{:.2} MPa", f.vessel.p_pa / 1e6));
                row("Water level", format!("{:+.2} m above the top of fuel", f.vessel.level_m - ACTIVE_LENGTH_M));
                row("Isolation condensers", format!("{:.1} MW", f.q_ic_w / 1e6));
                row("Relief valves", format!("{} · {:.0} kg/s", if f.relief_open { "open" } else { "closed" }, f.w_relief_kg_s));
                row("Injection", format!("{:.1} kg/s · {:.0} t so far", f.w_injection_kg_s, f.injected_kg / 1e3));
                row("Peak cladding", format!("{:.0} K", f.core.peak_clad_k));
                row("Zr oxidised", format!("{:.0} kg ({:.1} %)", f.core.zr_oxidised_kg, 100.0 * f.core.zr_oxidised_kg / core_zr_kg()));
                row("Hydrogen made", format!("{:.0} kg", f.core.h2_total_kg));
                row("Core relocated", format!("{:.0} %", 100.0 * f.core.relocated));
                row("Drywell", format!("{:.3} MPa · {:.0} °C", f.containment.p_dw_pa / 1e6, f.containment.t_dw_k - 273.15));
                row("Wetwell", format!("{:.3} MPa · pool {:.0} °C", f.containment.p_ww_pa / 1e6, f.containment.t_pool_k - 273.15));
                row("Building H2", format!("{:.2} %", 100.0 * f.containment.rb_h2_fraction));
            });
            ui.separator();
            ui.heading("The recorded sequence");
            for e in EVENTS.iter() {
                let done = f.t_s >= e.t_s;
                ui.label(RichText::new(format!("{} {}", if done { "✔" } else { "·" }, e.label)).small().color(if done { Color32::LIGHT_GREEN } else { Color32::GRAY }));
            }
            ui.label(RichText::new("Times from the INPO 11-005 timeline, not yet checked against the report.").small());
        }
        ui.separator();
        egui::CollapsingHeader::new("What this is, and is not").show(ui, |ui| {
            ui.label("Three lumped models stepped together each second: the pressure vessel's water and steam (IAPWS-IF97 steam tables), the core (3 rings × 12 axial nodes: heat-up, Urbanic-Heidrick zirconium oxidation, hydrogen, relocation) and the Mark I containment with the reactor building.");
            ui.label("Not in the model: radiation inside the core, two-phase level swell, molten-pool physics, fission products, combustion. Many plant numbers are placeholders; each is marked in params.rs with what would replace it.");
            ui.label("Physics runs off the page: a plant worker plus a core worker and a containment worker that step from the same start-of-step snapshot, as the CIET simulator's browser demo does. 1 and 3 workers give the same numbers bit for bit.");
        });
    }

    /// The plant schematic, in metres on the view.
    fn plant(&mut self, ui: &mut egui::Ui, rect: Rect) {
        let Some(f) = self.frame.clone() else {
            ui.painter().text(rect.center(), Align2::CENTER_CENTER, "Starting the plant…", FontId::proportional(20.0), Color32::WHITE);
            return;
        };
        let v = self.view;
        let to = |x: f64, y: f64| v.to_screen(rect, x, y);
        let px = |m: f64| (m * v.scale) as f32;
        let painter = ui.painter_at(rect);
        let stroke = Stroke::new(2.0, Color32::from_gray(200));
        // Reactor building outline (operating floor at the top).
        let rb = Rect::from_two_pos(to(-13.0, -6.0), to(13.0, 27.0));
        let rb_col = if f.containment.rb_exploded {
            Color32::from_rgb(120, 40, 20)
        } else if f.containment.rb_flammable {
            Color32::from_rgb(90, 70, 20)
        } else {
            Color32::from_gray(28)
        };
        painter.rect(rb, 0.0, rb_col, Stroke::new(1.5, Color32::from_gray(90)), StrokeKind::Inside);
        painter.text(to(-12.6, 26.6), Align2::LEFT_TOP, format!("Reactor building · H2 {:.2} %{}", 100.0 * f.containment.rb_h2_fraction, if f.containment.rb_exploded { " · EXPLODED" } else { "" }), FontId::proportional(13.0), Color32::WHITE);
        // Drywell: a light bulb around the vessel.
        let dw_col = colour(f.containment.t_dw_k);
        let bulb_c = to(0.0, 5.0);
        painter.circle(bulb_c, px(6.5), dw_col.gamma_multiply(0.35), stroke);
        let neck = Rect::from_two_pos(to(-4.0, 10.0), to(4.0, 23.0));
        painter.rect(neck, 0.0, dw_col.gamma_multiply(0.35), stroke, StrokeKind::Inside);
        painter.text(to(-6.3, 9.6), Align2::LEFT_BOTTOM, format!("Drywell {:.2} MPa (design {:.2})", f.containment.p_dw_pa / 1e6, CONTAINMENT_DESIGN_PA / 1e6), FontId::proportional(13.0), Color32::WHITE);
        // Torus (cross-section below), pool coloured by its temperature.
        let torus = Rect::from_two_pos(to(-11.0, -5.5), to(11.0, -2.0));
        painter.rect(torus, px(1.7), Color32::from_gray(45), stroke, StrokeKind::Inside);
        let pool = Rect::from_two_pos(to(-11.0, -5.5), to(11.0, -3.9));
        painter.rect_filled(pool, px(1.0), colour(f.containment.t_pool_k));
        painter.text(to(0.0, -2.3), Align2::CENTER_TOP, format!("Wetwell {:.2} MPa · pool {:.0} °C", f.containment.p_ww_pa / 1e6, f.containment.t_pool_k - 273.15), FontId::proportional(13.0), Color32::WHITE);
        // Downcomers.
        for x in [-3.5, 3.5] {
            painter.line_segment([to(x, 0.0), to(x, -4.6)], Stroke::new(3.0, Color32::from_gray(150)));
        }
        // The vessel: steam above the collapsed level, water below.
        let r = RPV_RADIUS_M;
        let y0 = 0.0;
        let vessel = Rect::from_two_pos(to(-r, y0), to(r, y0 + RPV_HEIGHT_M));
        let t_sat = f.vessel.sat.t_k;
        // Steam space grey, water blue: the level reads at a glance.
        painter.rect(vessel, px(1.5), Color32::from_gray(70), Stroke::new(3.0, Color32::WHITE), StrokeKind::Inside);
        let z_level = (BAF_M + f.vessel.level_m).clamp(0.0, RPV_HEIGHT_M);
        if z_level > 0.0 {
            let water = Rect::from_two_pos(to(-r, y0), to(r, y0 + z_level));
            painter.rect_filled(water, px(1.0), Color32::from_rgb(40, 90, 200).gamma_multiply(0.85));
        }
        // The core: 3 rings mirrored, 12 axial nodes, by cladding temperature.
        let w = 0.55;
        let dz = ACTIVE_LENGTH_M / AXIAL_NODES as f64;
        for ring in 0..RINGS {
            for j in 0..AXIAL_NODES {
                let t = f.core.clad_k.get(ring * AXIAL_NODES + j).copied().unwrap_or(f64::NAN);
                let col = if t.is_finite() { colour(t) } else { Color32::from_gray(15) };
                for side in [-1.0, 1.0] {
                    let x0 = side * (ring as f64 * w);
                    let x1 = side * ((ring + 1) as f64 * w);
                    let cell = Rect::from_two_pos(to(x0, BAF_M + j as f64 * dz), to(x1, BAF_M + (j + 1) as f64 * dz));
                    painter.rect(cell, 0.0, col, Stroke::new(0.5, Color32::from_gray(20)), StrokeKind::Inside);
                }
            }
        }
        if f.core.relocated > 0.0 {
            let h = (f.core.relocated * ACTIVE_LENGTH_M).max(0.2);
            let debris = Rect::from_two_pos(to(-1.8, 0.3), to(1.8, 0.3 + h * 0.5));
            painter.rect_filled(debris, px(0.4), colour(f.core.debris_k));
            painter.text(to(-2.0, 0.6), Align2::RIGHT_CENTER, format!("debris {:.0} K", f.core.debris_k), FontId::proportional(12.0), Color32::WHITE);
        }
        if f.core.lower_head_failed {
            painter.text(to(0.0, -0.6), Align2::CENTER_TOP, "lower head failed", FontId::proportional(13.0), Color32::LIGHT_RED);
        }
        // Pipes and valves from the widget library, in plant metres.
        let sat_t = temp(t_sat);
        let pipe = |ui: &mut egui::Ui, a: (f64, f64), b: (f64, f64), t: ThermodynamicTemperature, flow: f64, inventory_kg: f64| {
            let (pa, pb) = (to(a.0, a.1), to(b.0, b.1));
            let scalars = PipeScalars {
                temperature: t,
                mass_flow: MassRate::new::<kilogram_per_second>(flow),
                residence_time: residence_time_from_flow(Mass::new::<kilogram>(inventory_kg), MassRate::new::<kilogram_per_second>(flow)),
            };
            ui.add(PipeVisual::from_scalars(scalars, pa, pb - pa, temp(T_MIN), temp(T_MAX)));
        };
        let valve = |ui: &mut egui::Ui, at: (f64, f64), open: bool| {
            let physics = Valve::new(ValveFlowCoefficient(1.0), OpeningCharacteristic::Linear, Ratio::new::<ratio>(if open { 100.0 } else { 0.0 }));
            ui.add(ValveVisual::new(physics, to(at.0, at.1), Vec2::splat(px(0.9).clamp(10.0, 28.0))));
        };
        // Isolation condensers: two tanks high in the building.
        let ic_steam_flow = f.q_ic_w / (f.vessel.sat.h_g - f.vessel.sat.h_f).max(1.0);
        for (i, y) in [(0usize, 17.5), (1usize, 12.0)] {
            let tank = Rect::from_two_pos(to(7.0, y - 2.0), to(12.0, y + 2.0));
            painter.rect(tank, px(0.3), Color32::from_gray(35), stroke, StrokeKind::Inside);
            let frac = (f.ic_shell_kg[i] / IC_SHELL_WATER_KG).clamp(0.0, 1.0);
            let water = Rect::from_two_pos(to(7.0, y - 2.0), to(12.0, y - 2.0 + 4.0 * frac));
            painter.rect_filled(water, 0.0, colour(f.ic_shell_k[i]).gamma_multiply(0.9));
            painter.text(to(9.5, y + 2.1), Align2::CENTER_BOTTOM, format!("IC {} · {:.0} °C", if i == 0 { "A" } else { "B" }, f.ic_shell_k[i] - 273.15), FontId::proportional(12.0), Color32::WHITE);
            let open = f.controls.ic_open[i];
            let flow = if open { ic_steam_flow / 2.0 } else { 0.0 };
            // Steam in at the top of the tube bundle, condensate back below.
            pipe(ui, (r, y + 1.0), (7.0, y + 1.0), sat_t, flow, 200.0);
            pipe(ui, (7.0, y - 1.5), (r, y - 1.5), sat_t, flow, 2000.0);
            valve(ui, (5.6, y + 1.0), open);
            valve(ui, (5.6, y - 1.5), open);
        }
        // Relief valves to the pool.
        pipe(ui, (-r, 16.0), (-9.0, 16.0), sat_t, f.w_relief_kg_s, 300.0);
        pipe(ui, (-9.0, 16.0), (-9.0, -4.5), sat_t, f.w_relief_kg_s, 300.0);
        valve(ui, (-6.0, 16.0), f.relief_open);
        painter.text(to(-9.0, 16.6), Align2::CENTER_BOTTOM, if f.controls.dc_power { "relief valves (DC)" } else { "safety valves only" }, FontId::proportional(12.0), Color32::WHITE);
        // Main steam line, isolated (MSIVs closed after the earthquake).
        pipe(ui, (-r, 18.5), (-12.5, 18.5), sat_t, 0.0, 300.0);
        valve(ui, (-6.0, 18.5), false);
        painter.text(to(-10.0, 19.1), Align2::CENTER_BOTTOM, "main steam (MSIVs closed)", FontId::proportional(12.0), Color32::GRAY);
        // Fire-engine injection.
        pipe(ui, (-12.5, 9.0), (-r, 9.0), temp(INJECTION_TEMPERATURE_K), f.w_injection_kg_s, 500.0);
        valve(ui, (-7.0, 9.0), f.controls.injection);
        painter.text(to(-10.5, 9.6), Align2::CENTER_BOTTOM, "fire engine", FontId::proportional(12.0), Color32::WHITE);
        // Wetwell vent to the stack.
        pipe(ui, (9.0, -2.0), (9.0, 8.0), temp(f.containment.t_pool_k), f.containment.vented_kg_s, 300.0);
        pipe(ui, (9.0, 8.0), (14.5, 8.0), temp(f.containment.t_pool_k), f.containment.vented_kg_s, 300.0);
        valve(ui, (9.0, 3.0), f.containment.vent_flowing);
        painter.text(to(10.0, 3.0), Align2::LEFT_CENTER, "wetwell vent", FontId::proportional(12.0), Color32::WHITE);
        // Instruments.
        ui.add(InstrumentationVisual::new(to(-r, 21.6), "RPV", format!("{:.2} MPa", f.vessel.p_pa / 1e6)));
        ui.add(InstrumentationVisual::new(to(r + 0.2, BAF_M + ACTIVE_LENGTH_M), "level", format!("{:+.2} m TAF", f.vessel.level_m - ACTIVE_LENGTH_M)));
        ui.add(InstrumentationVisual::new(to(r + 0.2, BAF_M + 1.0), "peak clad", format!("{:.0} K", f.core.peak_clad_k)));
        // Legend, bottom right.
        // Legend, bottom right; smaller on a phone.
        let narrow = rect.width() < 700.0;
        let (size, bar) = if narrow { (Vec2::new(90.0, 140.0), Vec2::new(14.0, 100.0)) } else { (Vec2::new(110.0, 220.0), Vec2::new(26.0, 170.0)) };
        let legend_at = Rect::from_min_size(rect.right_bottom() - size - Vec2::new(10.0, 60.0), size);
        ui.put(legend_at, TemperatureLegend::new(temp(T_MIN), temp(T_MAX)).with_caption("Temperature").with_bar_size(bar));
        if f.vessel.dry && !f.core.lower_head_failed {
            painter.text(to(0.0, -1.6), Align2::CENTER_TOP, "vessel boiled dry: outside the model", FontId::proportional(13.0), Color32::LIGHT_RED);
        }
        painter.text(to(-2.0, BAF_M - 0.3), Align2::RIGHT_TOP, "core: cladding T\n(black = melted, relocated)", FontId::proportional(11.0), Color32::from_gray(200));
    }

    fn plots(&self, ui: &mut egui::Ui) {
        use egui_plot::{Legend, Line, Plot, PlotPoints, VLine};
        let hours = |i: usize| -> Vec<[f64; 2]> { self.history.iter().map(|r| [r[0] / 3600.0, r[i]]).collect() };
        let h = ((ui.available_height() - 40.0) / 4.0).max(140.0);
        egui::ScrollArea::vertical().show(ui, |ui| {
            let marks = |p: &mut egui_plot::PlotUi| {
                for e in EVENTS.iter().take(EVENTS.len() - 1) {
                    p.vline(VLine::new("", e.t_s / 3600.0).color(Color32::from_gray(90)));
                }
            };
            let spec: [(&str, &str, Vec<(usize, &str)>); 4] = [
                ("p", "pressure, MPa", vec![(1, "RPV"), (4, "drywell"), (5, "wetwell")]),
                ("lvl", "level above BAF, m (core top at 3.66)", vec![(2, "collapsed level")]),
                ("t", "temperature, K", vec![(3, "peak cladding"), (8, "pool")]),
                ("h2", "hydrogen made, kg", vec![(6, "H2")]),
            ];
            for (id, label, lines) in spec {
                Plot::new(id).height(h).legend(Legend::default()).x_axis_label("hours after the scram").y_axis_label(label).allow_scroll(false).show(ui, |p| {
                    for (i, name) in lines {
                        p.line(Line::new(name, PlotPoints::from(hours(i))));
                    }
                    marks(p);
                });
            }
        });
    }
}

impl eframe::App for App {
    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        let ctx = ui.ctx().clone();
        self.handle();
        if self.running || self.waiting {
            ctx.request_repaint_after(std::time::Duration::from_millis(30));
        }
        let title = match &self.frame {
            Some(f) => format!("Fukushima Unit 1 · {} · {:.2} MPa", clock(f.t_s), f.vessel.p_pa / 1e6),
            None => "Fukushima Unit 1 · starting".into(),
        };
        if title != self.title {
            set_title(&ctx, &title);
            self.title = title;
        }
        let mut panel = std::mem::take(&mut self.panel);
        panel.show(ui, "Fukushima Daiichi Unit 1", |ui| self.side_panel(ui));
        self.panel = panel;
        egui::CentralPanel::default().frame(egui::Frame::NONE.fill(Color32::from_rgb(14, 16, 20))).show(ui, |ui| {
            let rect = ui.max_rect();
            // Bottom bar: the two views.
            let bar = Rect::from_min_size(Pos2::new(rect.center().x - 110.0, rect.bottom() - 48.0), Vec2::new(220.0, 40.0));
            if self.tab == Tab::Plant {
                let resp = ui.interact(rect, ui.id().with("plant"), egui::Sense::click_and_drag());
                if !self.view.fitted || (!self.view.touched && self.view.fit_size != rect.size()) {
                    self.view.fit(rect);
                }
                self.view.handle_input(ui, &resp);
                self.plant(ui, rect);
                if let Some(z) = zoom_buttons(ui, rect) {
                    apply_zoom(&mut self.view, rect, z);
                }
            } else {
                let inner = Rect::from_min_max(rect.min + Vec2::new(8.0, 56.0), rect.max - Vec2::new(8.0, 56.0));
                ui.scope_builder(egui::UiBuilder::new().max_rect(inner), |ui| self.plots(ui));
            }
            if let Some(f) = &self.frame {
                // Under the 52 px button row on a phone, beside it on a wide screen.
                let at = if rect.width() < 700.0 { Vec2::new(10.0, 58.0) } else { Vec2::new(if self.panel.open { 8.0 } else { 156.0 }, 14.0) };
                ui.painter().text(rect.left_top() + at, Align2::LEFT_TOP, clock(f.t_s), FontId::proportional(15.0), Color32::WHITE);
            }
            ui.scope_builder(egui::UiBuilder::new().max_rect(bar), |ui| {
                ui.horizontal(|ui| {
                    for (t, l) in [(Tab::Plant, "Plant"), (Tab::Plots, "Plots")] {
                        if ui.add(egui::Button::selectable(self.tab == t, RichText::new(l).size(18.0)).min_size(Vec2::new(104.0, 38.0))).clicked() {
                            self.tab = t;
                        }
                    }
                });
            });
            self.panel.reopen_button(ui, rect);
        });
    }
}
