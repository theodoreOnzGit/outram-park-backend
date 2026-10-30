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
//! exception for GUI code; the model it draws is tested in `sembawang::bwr_mark1`.

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
            // The reactor building's outline, y = -8.2 to 27 m, plus a margin.
            view: View::new(17.9).with_home([0.0, 9.4]),
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
                row("Zr oxidised", format!("{:.0} kg ({:.1} %)", f.core.zr_oxidised_kg, 100.0 * f.core.zr_oxidised_kg / Params::default().core_zr_kg()));
                row("Hydrogen made", format!("{:.0} kg", f.core.h2_total_kg));
                row("Core relocated", format!("{:.0} %", 100.0 * f.core.relocated));
                row("Drywell", format!("{:.3} MPa · {:.0} °C", f.containment.p_dw_pa / 1e6, f.containment.t_dw_k - 273.15));
                row("Wetwell", format!("{:.3} MPa · pool {:.0} °C", f.containment.p_ww_pa / 1e6, f.containment.t_pool_k - 273.15));
                row("Pool surface layer", format!("{:.0} °C (above the downcomer exits)", f.containment.t_pool_top_k - 273.15));
                row("Leak paths", format!("steam line {} · drywell head flange {}", if f.msl.failed { "ruptured" } else { "intact" }, if f.containment.flange_leaking { "leaking" } else { "closed" }));
                row("Building H2", format!("{:.2} %", 100.0 * f.containment.rb_h2_fraction));
                row("Concrete ablated", format!("{:.1} t", f.containment.cav_ablated_kg / 1e3));
                row("Cesium released", format!("{:.2} kg vented · {:.2} kg to the building", f.containment.cs_vented_kg, f.containment.cs_building_kg));
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
            ui.label("Three lumped models stepped together each second: the pressure vessel's water and steam (IAPWS-IF97 steam tables, with the two-phase level swell), the core (3 rings × 12 axial nodes: heat-up, Urbanic-Heidrick zirconium oxidation, hydrogen, candling, rod collapse, the control blades' steel, a core plate per ring holding the relocating debris, the lower head) and the Mark I containment (drywell and torus with MELCOR's heat-structure and pool-surface models, the torus pool in two layers, fission products, core-concrete interaction) with the reactor building.");
            ui.label("Not in the model: molten-pool physics; combustion as a calculation (the building explodes at a hydrogen threshold); CO and CO2 as their own gases; the control blades' B4C. In-core radiation is simplified (rings, levels, the shroud and the boundaries above and below). Many plant numbers are placeholders; each is marked in sembawang's bwr_mark1 params.rs with its tag and what would replace it. The head-failure time is a knife edge: inputs changed by 1 % move it by about an hour (gh:#1044).");
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
        let rb = Rect::from_two_pos(to(-13.0, -8.2), to(13.0, 27.0));
        let rb_col = if f.containment.rb_exploded {
            Color32::from_rgb(120, 40, 20)
        } else if f.containment.rb_flammable {
            Color32::from_rgb(90, 70, 20)
        } else {
            Color32::from_gray(28)
        };
        painter.rect(rb, 0.0, rb_col, Stroke::new(1.5, Color32::from_gray(90)), StrokeKind::Inside);
        painter.text(to(-12.6, 26.6), Align2::LEFT_TOP, format!("Reactor building · H2 {:.2} %{}", 100.0 * f.containment.rb_h2_fraction, if f.containment.rb_exploded { " · EXPLODED" } else { "" }), FontId::proportional(13.0), Color32::WHITE);
        // Torus: the Mark I suppression chamber, drawn as a donut seen from
        // above at an angle and cut open along its mid-plane, so the pool
        // shows (`torus` below has the geometry). The drywell is drawn after
        // it, so it stands in the donut's hole as it does in the plant.
        let tor = torus::Torus::DRAWN;
        let ring = |rho: f64, y: f64, fill: Color32, line: Stroke| {
            let pts: Vec<Pos2> = tor.ellipse(rho, y).into_iter().map(|(x, y)| to(x, y)).collect();
            painter.add(egui::Shape::convex_polygon(pts, fill, line));
        };
        let shell = Color32::from_gray(58);
        let inside = Color32::from_gray(34);
        let (cy, rt) = (tor.centre_y, tor.tube_radius);
        // The body: the tube's round cross-section swept around the ring,
        // whose outline is the centre-line ellipse pushed out by the tube
        // radius. This is what makes it a donut and not a pan.
        let body: Vec<Pos2> = tor.outline().into_iter().map(|(x, y)| to(x, y)).collect();
        painter.add(egui::Shape::convex_polygon(body, shell, stroke));
        let bottom = cy - tor.major_radius * torus::ELEVATION_DEG.to_radians().sin() - rt;
        // The cut face: the tube's inner wall, then the pool surface.
        ring(tor.major_radius + rt, cy, inside, stroke);
        let (y_w, half) = tor.water_surface(POOL_WATER_M3 / (POOL_WATER_M3 + WETWELL_GAS_M3));
        ring(tor.major_radius + half, y_w, colour(f.containment.t_pool_k), Stroke::NONE);
        ring(tor.major_radius - half, y_w, inside, Stroke::NONE);
        // The hole.
        ring(tor.major_radius - rt, cy, rb_col, stroke);
        painter.text(
            // On the torus's front skin, below the pool's near edge.
            to(0.0, 0.5 * (bottom + y_w - (tor.major_radius + half) * torus::ELEVATION_DEG.to_radians().sin())),
            Align2::CENTER_CENTER,
            format!("Wetwell (torus) {:.2} MPa · pool {:.0} °C", f.containment.p_ww_pa / 1e6, f.containment.t_pool_k - 273.15),
            FontId::proportional(13.0),
            Color32::WHITE,
        );
        // Drywell: a light bulb around the vessel.
        let dw_col = colour(f.containment.t_dw_k);
        let bulb_c = to(0.0, 5.0);
        painter.circle(bulb_c, px(6.5), dw_col.gamma_multiply(0.35), stroke);
        let neck = Rect::from_two_pos(to(-4.0, 10.0), to(4.0, 23.0));
        painter.rect(neck, 0.0, dw_col.gamma_multiply(0.35), stroke, StrokeKind::Inside);
        painter.text(to(-6.3, 9.6), Align2::LEFT_BOTTOM, format!("Drywell {:.2} MPa (design {:.2})", f.containment.p_dw_pa / 1e6, CONTAINMENT_DESIGN_PA / 1e6), FontId::proportional(13.0), Color32::WHITE);
        // Vent lines: from low on the drywell, out and down at
        // `VENT_FROM_VERTICAL_DEG` from the vertical into the pool
        // (maintainer, 2026-10-27: "diagonal ... 60 from vertical, so it shld
        // be more flat, similar to GE designs online"). Their ends sit on the
        // pool surface's centre line, on the near side.
        for side in [-1.0, 1.0] {
            let (a, b) = tor.vent_line(side, (0.0, 5.0), 6.5, y_w);
            painter.line_segment([to(a.0, a.1), to(b.0, b.1)], Stroke::new(px(0.9).clamp(3.0, 9.0), Color32::from_gray(150)));
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
                // Fit between the button row and the tab bar, which take the
                // same height top and bottom, so the centre is unchanged.
                let fit_rect = rect.shrink2(Vec2::new(0.0, 56.0));
                if !self.view.fitted || (!self.view.touched && self.view.fit_size != fit_rect.size()) {
                    self.view.fit(fit_rect);
                }
                self.view.handle_input(ui, &resp);
                self.plant(ui, rect);
                if let Some(z) = zoom_buttons(ui, rect) {
                    apply_zoom(&mut self.view, fit_rect, z);
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

/// The torus as drawn: a donut viewed from above at `ELEVATION_DEG`, cut
/// along its horizontal mid-plane. Plain geometry in plant metres, no egui,
/// so it is tested on its own.
///
/// **Indicative drawing dimensions, not plant data.** A Mark I torus is far
/// bigger than this (major diameter around 30 m); it is drawn to fit the
/// reactor building's 26 m outline in this schematic. What IS from the model
/// is the pool's share of the tube: `water_surface` is given the fraction
/// `POOL_WATER_M3 / (POOL_WATER_M3 + WETWELL_GAS_M3)` (1750 and 2620 m^3,
/// `sembawang::bwr_mark1::params`), which the containment model holds fixed, so the drawn
/// level is fixed too.
mod torus {
    /// The vent lines' angle from the vertical (maintainer, 2026-10-27), so
    /// 30 degrees below the horizontal.
    pub const VENT_FROM_VERTICAL_DEG: f64 = 60.0;
    /// Where they leave the drywell: this far below its horizontal
    /// mid-line, measured round the bulb from its centre. Low on the bulb, as
    /// on GE's Mark I drawings, so a line that flat still reaches the pool.
    /// A drawing choice.
    pub const VENT_EXIT_DEG: f64 = 60.0;
    /// The viewing angle above the horizontal. A drawing choice.
    pub const ELEVATION_DEG: f64 = 20.0;

    #[derive(Clone, Copy, Debug)]
    pub struct Torus {
        pub centre_y: f64,
        pub major_radius: f64,
        pub tube_radius: f64,
    }

    impl Torus {
        pub const DRAWN: Torus = Torus { centre_y: -2.4, major_radius: 8.2, tube_radius: 2.4 };

        pub fn cos_elevation(&self) -> f64 {
            ELEVATION_DEG.to_radians().cos()
        }

        /// A horizontal circle of radius `rho` at drawn height `y`, as the
        /// ellipse the viewer sees (the far side higher), 64 points.
        pub fn ellipse(&self, rho: f64, y: f64) -> Vec<(f64, f64)> {
            let k = ELEVATION_DEG.to_radians().sin();
            (0..64)
                .map(|i| {
                    let th = std::f64::consts::TAU * i as f64 / 64.0;
                    (rho * th.cos(), y + k * rho * th.sin())
                })
                .collect()
        }

        /// The body's silhouette: every point of the tube's centre-line
        /// ellipse (radius `major_radius`, drawn at `centre_y`) moved out
        /// along its normal by `tube_radius`, i.e. the envelope of the tube's
        /// round cross-sections. Convex, as the offset of a convex curve.
        pub fn outline(&self) -> Vec<(f64, f64)> {
            let k = ELEVATION_DEG.to_radians().sin();
            let (a, b) = (self.major_radius, k * self.major_radius);
            (0..96)
                .map(|i| {
                    let th = std::f64::consts::TAU * i as f64 / 96.0;
                    let (nx, ny) = (b * th.cos(), a * th.sin());
                    let n = nx.hypot(ny);
                    (a * th.cos() + self.tube_radius * nx / n, self.centre_y + b * th.sin() + self.tube_radius * ny / n)
                })
                .collect()
        }

        /// The pool surface for a tube `fill` full by cross-section area:
        /// its drawn height and its half-width across the tube (the surface
        /// is the annulus `major_radius ± half`).
        pub fn water_surface(&self, fill: f64) -> (f64, f64) {
            let depth = segment_depth(fill.clamp(0.0, 1.0)) * self.tube_radius;
            let h = depth - self.tube_radius;
            let half = (self.tube_radius * self.tube_radius - h * h).max(0.0).sqrt();
            (self.centre_y + h * self.cos_elevation(), half)
        }

        /// A vent line leaving a drywell of radius `r` centred at `c` at
        /// `VENT_EXIT_DEG` round the bulb, on the `side` (-1 left, +1 right),
        /// running out and down at `VENT_FROM_VERTICAL_DEG` from the vertical
        /// until it meets the near half of the pool surface's centre line
        /// (the ellipse of radius `major_radius` at height `y_w`).
        pub fn vent_line(&self, side: f64, c: (f64, f64), r: f64, y_w: f64) -> ((f64, f64), (f64, f64)) {
            let e = VENT_EXIT_DEG.to_radians();
            let a = (c.0 + side * r * e.cos(), c.1 - r * e.sin());
            let v = VENT_FROM_VERTICAL_DEG.to_radians();
            let d = (side * v.sin(), -v.cos());
            let k = ELEVATION_DEG.to_radians().sin();
            // Inside the ellipse: (x/R)^2 + ((y - y_w)/(k R))^2 < 1.
            let outside = |t: f64| {
                let (x, y) = (a.0 + t * d.0, a.1 + t * d.1);
                (x / self.major_radius).powi(2) + ((y - y_w) / (k * self.major_radius)).powi(2) - 1.0
            };
            // March to the first crossing below the far side, then bisect.
            let (mut lo, mut hi) = (0.0, 0.0);
            let mut t = 0.0;
            let mut was_inside = outside(0.0) < 0.0;
            while t < 40.0 {
                t += 0.05;
                let ins = outside(t) < 0.0;
                if was_inside && !ins {
                    (lo, hi) = (t - 0.05, t);
                    break;
                }
                was_inside = ins;
            }
            for _ in 0..50 {
                let m = 0.5 * (lo + hi);
                if outside(m) < 0.0 {
                    lo = m;
                } else {
                    hi = m;
                }
            }
            let t = 0.5 * (lo + hi);
            (a, (a.0 + t * d.0, a.1 + t * d.1))
        }
    }

    /// The depth of a circular segment, as a fraction of the radius (0 to 2),
    /// that holds `fill` of the circle's area: bisection on
    /// `A(d) = acos(1 - d) - (1 - d) sqrt(2d - d^2)` over `pi`.
    pub fn segment_depth(fill: f64) -> f64 {
        let area = |d: f64| ((1.0 - d).acos() - (1.0 - d) * (2.0 * d - d * d).max(0.0).sqrt()) / std::f64::consts::PI;
        let (mut lo, mut hi) = (0.0, 2.0);
        for _ in 0..60 {
            let m = 0.5 * (lo + hi);
            if area(m) < fill {
                lo = m;
            } else {
                hi = m;
            }
        }
        0.5 * (lo + hi)
    }

    #[cfg(test)]
    mod tests {
        use super::*;

        /// Half full is the mid-plane; empty and full are the bottom and top;
        /// the segment area is monotone.
        #[test]
        fn the_segment_depth_inverts_the_segment_area() {
            assert!((segment_depth(0.5) - 1.0).abs() < 1e-12);
            // 1e-9, not 1e-12: at the top of the tube dA/dd -> 0, so round-off
            // in acos near -1 leaves the full-tube depth 3e-11 short of 2
            // (measured 2026-10-27). Far below a pixel.
            assert!(segment_depth(0.0) < 1e-9 && (segment_depth(1.0) - 2.0).abs() < 1e-9);
            // The model's pool, 1750 of 4370 m^3: just under half, so below the mid-plane.
            let d = segment_depth(1750.0 / 4370.0);
            assert!(d > 0.8 && d < 1.0, "{d}");
        }

        /// The silhouette reaches exactly one tube radius past the ring at
        /// its sides and below its nearest point.
        #[test]
        fn the_outline_is_the_ring_widened_by_the_tube() {
            let t = Torus::DRAWN;
            let o = t.outline();
            let k = ELEVATION_DEG.to_radians().sin();
            let right = o.iter().map(|p| p.0).fold(f64::MIN, f64::max);
            let lowest = o.iter().map(|p| p.1).fold(f64::MAX, f64::min);
            assert!((right - (t.major_radius + t.tube_radius)).abs() < 1e-12, "{right}");
            assert!((lowest - (t.centre_y - k * t.major_radius - t.tube_radius)).abs() < 1e-12, "{lowest}");
        }

        /// The pool surface sits below the tube's centre for a 40 % fill and
        /// is narrower than the tube.
        #[test]
        fn the_drawn_pool_is_below_the_mid_plane() {
            let t = Torus::DRAWN;
            let (y, half) = t.water_surface(0.4);
            assert!(y < t.centre_y && half < t.tube_radius && half > 0.9 * t.tube_radius, "{y} {half}");
        }

        /// The vent line runs at the asked 60 degrees from the vertical,
        /// outward and down, and ends on the near half of the pool's
        /// centre-line ellipse, mirrored left and right.
        #[test]
        fn the_vent_lines_run_60_degrees_from_vertical_into_the_pool() {
            let t = Torus::DRAWN;
            let (y_w, _) = t.water_surface(0.4);
            let (a, b) = t.vent_line(1.0, (0.0, 5.0), 6.5, y_w);
            let from_vertical = ((b.0 - a.0) / (a.1 - b.1)).atan().to_degrees();
            assert!((from_vertical - 60.0).abs() < 1e-9, "{from_vertical}");
            assert!(b.1 < y_w, "ends on the near side: {b:?}");
            let k = ELEVATION_DEG.to_radians().sin();
            let on = (b.0 / t.major_radius).powi(2) + ((b.1 - y_w) / (k * t.major_radius)).powi(2);
            assert!((on - 1.0).abs() < 1e-6, "{on}");
            let (a2, b2) = t.vent_line(-1.0, (0.0, 5.0), 6.5, y_w);
            assert!((a2.0 + a.0).abs() < 1e-12 && (b2.0 + b.0).abs() < 1e-9 && (b2.1 - b.1).abs() < 1e-9);
        }
    }
}
