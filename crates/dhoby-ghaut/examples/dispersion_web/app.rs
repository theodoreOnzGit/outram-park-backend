//! The dispersion demo's app: a side panel of controls (`web_demo::panel`),
//! a main view that is either a map (`web_demo::view`, metres) or a plot,
//! and a link to the engine (`web_demo::link`) that does every calculation
//! off this thread. Each frame only drains the link, draws and sends.

use crate::engine::{Event, PlumeParams, PuffParams, Request, Series, ELEMENTS, PUFF_DT_S};
use crate::rungs::Rung;
use crate::step_view::{self, Tracers};
use crate::steps::{Step, StepFrame, StepParams};
use dhoby_ghaut::web_demo::lesson::{self, Rung as _};
use dhoby_ghaut::web_demo::link::Link;
use dhoby_ghaut::web_demo::panel::Panel;
use dhoby_ghaut::web_demo::platform::{now_s, set_query, set_title};
use dhoby_ghaut::web_demo::view::{apply_zoom, zoom_buttons, View, Zoom};
use egui::{Color32, Pos2, Rect, Stroke, Vec2};

type DLink = Link<Request, Event>;

const CLASSES: [&str; 6] = ["A", "B", "C", "D", "E", "F"];
/// Series colours: blue, orange, green, purple, grey, brown.
const PALETTE: [Color32; 6] = [
    Color32::from_rgb(110, 170, 255),
    Color32::from_rgb(255, 170, 80),
    Color32::from_rgb(120, 220, 140),
    Color32::from_rgb(200, 140, 255),
    Color32::from_rgb(200, 200, 200),
    Color32::from_rgb(220, 150, 120),
];

/// A finished map from the engine.
struct Field {
    cells: u32,
    half_width: f64,
    values: Vec<f64>,
    puffs: Vec<f64>,
    max: f64,
    time_s: f64,
}

/// Every control, for every rung (each rung reads the ones it needs).
#[derive(Clone, Copy, PartialEq)]
struct Controls {
    class: u8,
    wind: f64,
    h: f64,
    dir_deg: f64,
    element: u8,
    half_life_s: f64,
    w0: f64,
    area: f64,
    frozen: bool,
    emit: bool,
    running: bool,
    /// Simulated seconds per real second in the puff rung.
    speed: f64,
}

impl Default for Controls {
    fn default() -> Self {
        Self {
            class: 3,
            wind: 2.0,
            h: 30.0,
            // Towards north (up): on a phone held upright the plume runs along
            // the long side of the screen.
            dir_deg: 0.0,
            element: 1,
            half_life_s: 0.0,
            w0: 10.0,
            area: 1000.0,
            frozen: false,
            emit: true,
            running: true,
            speed: 60.0,
        }
    }
}

pub struct DispApp {
    link: Option<DLink>,
    error: Option<String>,
    rung: Rung,
    panel: Panel,
    view: View,
    c: Controls,
    /// What the shown picture was computed from (`None`: recompute).
    shown: Option<(Rung, Controls)>,
    next_id: u32,
    in_flight: Option<(u32, f64)>,
    field: Option<Field>,
    curves: Vec<Series>,
    last_ms: f64,
    plot_text: f32,
    title: String,
    reset_puffs: bool,
    last_step_at: f64,
    /// A step animation of the current rung, when one is open (gh:#548).
    step: Option<Step>,
    step_frame: Option<StepFrame>,
    /// What the shown step frame was computed from.
    step_shown: Option<(Step, StepParams)>,
    tracers: Tracers,
    /// The slice step's downwind distance, m, and the step's play state.
    slice_x: f64,
    step_playing: bool,
    last_anim_at: f64,
}

/// Half-width of the map, m, per rung.
fn map_half_width(r: Rung) -> f64 {
    match r {
        Rung::Puffs => 4000.0,
        _ => 3000.0,
    }
}

impl DispApp {
    pub fn new(cc: &eframe::CreationContext<'_>, rung: Rung) -> Self {
        cc.egui_ctx.set_theme(egui::ThemePreference::Dark);
        #[cfg(not(target_arch = "wasm32"))]
        let link: Result<DLink, String> = {
            let ctx = cc.egui_ctx.clone();
            Ok(dhoby_ghaut::web_demo::link::start_native(
                crate::engine::Engine::default(),
                move || ctx.request_repaint(),
            ))
        };
        #[cfg(target_arch = "wasm32")]
        let link = dhoby_ghaut::web_demo::link::start_web::<Request, Event>(
            cc.egui_ctx.clone(),
            "./worker.js",
            Event::Error,
        );
        let (link, error) = match link {
            Ok(l) => (Some(l), None),
            Err(e) => (None, Some(e)),
        };
        let step = dhoby_ghaut::web_demo::platform::query_pairs()
            .iter()
            .find(|(k, _)| k == "step")
            .and_then(|(_, v)| Step::parse(v));
        // A step names its rung; the step wins if the two disagree.
        let rung = step.map_or(rung, Step::rung);
        let mut c = Controls::default();
        if matches!(rung, Rung::Dose | Rung::Capstone) {
            // The capstone's own dispersion, so the live curve starts on the record.
            c.class = 5;
            c.wind = 1.0;
        }
        Self {
            link,
            error,
            rung,
            panel: Panel::default(),
            view: View::new(map_half_width(rung)),
            c,
            shown: None,
            next_id: 1,
            in_flight: None,
            field: None,
            curves: Vec::new(),
            last_ms: 0.0,
            plot_text: 13.0,
            title: String::new(),
            reset_puffs: true,
            last_step_at: 0.0,
            step,
            step_frame: None,
            step_shown: None,
            tracers: Tracers::default(),
            slice_x: 100.0,
            step_playing: true,
            last_anim_at: now_s(),
        }
    }

    /// Open a step animation of the current rung, or go back to the rung view.
    fn set_step(&mut self, s: Option<Step>) {
        if s != self.step {
            self.step = s;
            self.step_frame = None;
            self.step_shown = None;
            self.tracers.clear();
            self.reset_puffs = true;
            self.slice_x = 100.0;
        }
        match self.step {
            Some(st) => set_query(&[("rung", self.rung.name()), ("step", st.name())]),
            None => set_query(&[("rung", self.rung.name())]),
        }
    }

    fn step_params(&self) -> StepParams {
        StepParams {
            class: self.c.class,
            wind: self.c.wind,
            h: self.c.h,
            x: self.slice_x,
            w0: self.c.w0,
        }
    }

    /// The step animations that run on the UI thread: the tracers, and the
    /// slice's moving distance (one cheap worker request per frame).
    fn animate(&mut self) {
        let now = now_s();
        let dt = (now - self.last_anim_at).min(0.1);
        self.last_anim_at = now;
        if !self.step_playing {
            return;
        }
        match self.step {
            Some(Step::Tracers) | Some(Step::Rise) => {
                if let Some(f) = &self.step_frame {
                    self.tracers.advance(f, dt * self.c.speed);
                }
            }
            Some(Step::Slice) => {
                self.slice_x += dt * 300.0;
                if self.slice_x > crate::steps::REACH_M {
                    self.slice_x = 100.0;
                }
            }
            _ => {}
        }
    }

    fn set_rung(&mut self, r: Rung) {
        if r != self.rung {
            self.rung = r;
            self.view = View::new(map_half_width(r));
            self.field = None;
            self.curves.clear();
            self.shown = None;
            self.reset_puffs = true;
            self.step = None;
            self.step_frame = None;
            self.step_shown = None;
            if r == Rung::Capstone || r == Rung::Dose {
                // The capstone's own dispersion, so the live curve starts on the record.
                self.c.class = 5;
                self.c.wind = 1.0;
            }
        }
        set_query(&[("rung", self.rung.name())]);
    }

    /// The request that recomputes the current picture.
    fn request(&self, id: u32) -> Request {
        let c = self.c;
        let half = map_half_width(self.rung);
        let p = PlumeParams {
            class: c.class,
            wind: c.wind,
            h: c.h,
            dir_deg: c.dir_deg,
            cells: 96,
            half_width: half,
        };
        match self.rung {
            Rung::Plume => Request::Plume { id, p },
            Rung::Deposition => Request::Deposition {
                id,
                p,
                element: c.element,
                half_life_s: c.half_life_s,
            },
            Rung::Sigmas => Request::Sigmas { id, class: c.class },
            Rung::RiseWake => Request::Rise {
                id,
                class: c.class,
                wind: c.wind,
                h: c.h,
                w0: c.w0,
                area: c.area,
            },
            Rung::Dose => Request::Dose {
                id,
                class: c.class,
                wind: c.wind,
            },
            Rung::Capstone => Request::Dose {
                id,
                class: 5,
                wind: 1.0,
            },
            Rung::Puffs => Request::Puffs {
                id,
                q: PuffParams {
                    class: c.class,
                    wind: c.wind,
                    dir_deg: c.dir_deg,
                    h: c.h,
                    // The single-puff step emits once, on the restart.
                    emit: c.emit && (self.step != Some(Step::SinglePuff) || self.reset_puffs),
                    frozen: c.frozen,
                    reset: self.reset_puffs,
                    steps: 1,
                    cells: 72,
                    half_width: half,
                },
            },
        }
    }

    /// Send what the picture needs, one request in flight at a time, so the
    /// worker never builds a queue the reader would wait behind.
    fn pump(&mut self) {
        let Some(link) = &self.link else { return };
        if self.in_flight.is_some() {
            return;
        }
        let now = now_s();
        if let Some(st) = self.step.filter(|s| *s != Step::SinglePuff) {
            let p = self.step_params();
            if self.step_shown != Some((st, p)) {
                let id = self.next_id;
                self.next_id += 1;
                link.send(Request::Step { id, step: st as u8, p });
                self.in_flight = Some((id, now));
                self.step_shown = Some((st, p));
            }
            return;
        }
        let wanted = if self.rung == Rung::Puffs {
            let interval = PUFF_DT_S / self.c.speed.max(1.0);
            (self.c.running && now - self.last_step_at >= interval) || self.reset_puffs
        } else {
            self.shown != Some((self.rung, self.c))
        };
        if !wanted {
            return;
        }
        let id = self.next_id;
        self.next_id += 1;
        link.send(self.request(id));
        self.in_flight = Some((id, now));
        self.shown = Some((self.rung, self.c));
        if self.rung == Rung::Puffs {
            self.last_step_at = now;
            self.reset_puffs = false;
        }
    }

    fn handle(&mut self, events: Vec<Event>) {
        for e in events {
            match e {
                Event::Field {
                    id,
                    cells,
                    half_width,
                    values,
                    puffs,
                    time_s,
                    ms,
                } => {
                    if self.in_flight.map(|f| f.0) == Some(id) {
                        self.in_flight = None;
                    }
                    let max = values.iter().copied().fold(0.0_f64, f64::max);
                    self.field = Some(Field {
                        cells,
                        half_width,
                        values,
                        puffs,
                        max,
                        time_s,
                    });
                    self.last_ms = ms;
                }
                Event::Curves { id, series, ms } => {
                    if self.in_flight.map(|f| f.0) == Some(id) {
                        self.in_flight = None;
                    }
                    self.curves = series;
                    self.last_ms = ms;
                }
                Event::Step { id, frame, ms } => {
                    if self.in_flight.map(|f| f.0) == Some(id) {
                        self.in_flight = None;
                    }
                    if Step::from_code(frame.step) == self.step {
                        self.step_frame = Some(frame);
                    }
                    self.last_ms = ms;
                }
                Event::Error(m) => {
                    self.in_flight = None;
                    self.error = Some(m);
                }
            }
        }
    }

    fn side_panel(&mut self, ui: &mut egui::Ui) {
        ui.label(egui::RichText::new("Research, education and V&V only. Not for emergency planning or response, and never a dose to a real person.").small());
        ui.separator();
        let picked = lesson::picker(ui, self.rung);
        if picked != self.rung {
            self.set_rung(picked);
        }
        ui.strong(self.rung.title());
        lesson::whats_happening(ui, self.rung);
        let steps: Vec<Step> = Step::of(self.rung).collect();
        if !steps.is_empty() {
            ui.label("Step animations (one per lesson step):");
            let mut pick = self.step;
            ui.horizontal_wrapped(|ui| {
                ui.selectable_value(&mut pick, None, "the rung view");
                for st in &steps {
                    ui.selectable_value(&mut pick, Some(*st), st.title());
                }
            });
            if pick != self.step {
                self.set_step(pick);
            }
        }
        ui.separator();
        if let Some(st) = self.step.filter(|s| *s != Step::SinglePuff) {
            self.step_panel(ui, st);
            return;
        }
        let c = &mut self.c;
        let class_row = |ui: &mut egui::Ui, class: &mut u8, set: &str| {
            ui.horizontal_wrapped(|ui| {
                ui.label(format!("Stability ({set}):"));
                for (i, n) in CLASSES.iter().enumerate() {
                    ui.selectable_value(class, i as u8, *n);
                }
            });
        };
        match self.rung {
            Rung::Plume | Rung::Deposition => {
                class_row(ui, &mut c.class, "pyDOSEIA");
                ui.add(egui::Slider::new(&mut c.wind, 0.5..=12.0).text("wind at 10 m, m/s"));
                ui.add(egui::Slider::new(&mut c.h, 0.0..=120.0).text("release height, m"));
                ui.add(
                    egui::Slider::new(&mut c.dir_deg, 0.0..=359.0).text("wind blows towards, deg"),
                );
                if self.rung == Rung::Deposition {
                    egui::ComboBox::from_label("element")
                        .selected_text(ELEMENTS[c.element as usize].1)
                        .show_ui(ui, |ui| {
                            for (i, e) in ELEMENTS.iter().enumerate() {
                                ui.selectable_value(&mut c.element, i as u8, e.1);
                            }
                        });
                    ui.add(
                        egui::Slider::new(&mut c.half_life_s, 0.0..=7200.0)
                            .text("half-life, s (0 = stable)"),
                    );
                    ui.label("Kr-89 is 189 s. Deposition velocities are pyDOSEIA's SRS-19 screening values; dry only, the plume is not depleted (#543).");
                }
            }
            Rung::Sigmas => {
                class_row(ui, &mut c.class, "both sets");
                ui.label("Dotted: sigma_y. Dashed: sigma_z. Blue: changi (Martin / US EPA ISC). Orange: buangkok (pyDOSEIA BARC/AERB). Fitted over roughly 0.1-10 km.");
            }
            Rung::RiseWake => {
                class_row(ui, &mut c.class, "pyDOSEIA");
                ui.add(egui::Slider::new(&mut c.wind, 0.5..=12.0).text("wind at 10 m, m/s"));
                ui.add(egui::Slider::new(&mut c.h, 10.0..=120.0).text("stack height, m"));
                ui.add(egui::Slider::new(&mut c.w0, 0.0..=30.0).text("exit velocity W0, m/s"));
                ui.add(
                    egui::Slider::new(&mut c.area, 100.0..=5000.0)
                        .text("building cross-section, m2"),
                );
                ui.label("pyDOSEIA's rise and wake formulas, unchecked against their sources (#542); A-D neutral formula, E-F stable windy formula. None is used in any dose pathway.");
            }
            Rung::Puffs => {
                class_row(ui, &mut c.class, "changi, new puffs");
                ui.add(egui::Slider::new(&mut c.wind, 0.5..=12.0).text("wind, m/s"));
                ui.add(
                    egui::Slider::new(&mut c.dir_deg, 0.0..=359.0).text("wind blows towards, deg"),
                );
                ui.add(egui::Slider::new(&mut c.h, 0.0..=100.0).text("release height, m"));
                ui.add(
                    egui::Slider::new(&mut c.speed, 5.0..=300.0)
                        .logarithmic(true)
                        .text("simulated s per real s"),
                );
                ui.checkbox(&mut c.emit, "emitting");
                ui.checkbox(
                    &mut c.frozen,
                    "upstream R puff's frozen wind (the defect, #344)",
                );
                ui.horizontal(|ui| {
                    if ui.button(if c.running { "Pause" } else { "Run" }).clicked() {
                        c.running = !c.running;
                    }
                    if ui.button("Restart").clicked() {
                        self.reset_puffs = true;
                    }
                });
                ui.label("Turn the wind: each puff remembers where it is and the class it was born in. Colours are relative ground-level concentration per unit mass (illustration).");
            }
            Rung::Dose => {
                class_row(ui, &mut c.class, "pyDOSEIA");
                ui.add(egui::Slider::new(&mut c.wind, 0.5..=12.0).text("wind at 10 m, m/s"));
                ui.label("Solid: Liu & Cao (2002) Table 9, published, for DIFFERENT accidents with other codes: context, not a check. Markers: the capstone, recorded. Dotted: the capstone's release under this class and wind, scaled by chi/Q (the capstone's own arithmetic).");
            }
            Rung::Capstone => {
                ui.label(format!(
                    "The release and the 400 m doses are recorded, not computed here: {}.",
                    crate::recorded::SOURCE
                ));
                ui.label("The dotted line is the 400 m dose scaled by buangkok's chi/Q, the capstone's own arithmetic; it passes through every recorded point.");
                ui.label(format!("Released over 96 h: {:.3e} Bq (bounding: 1400 C whole-core hold, KORA failure).", crate::recorded::RELEASE_TOTAL_BQ));
                egui::Grid::new("release").striped(true).show(ui, |ui| {
                    ui.strong("nuclide");
                    ui.strong("released, Bq");
                    ui.strong("of core");
                    ui.end_row();
                    for (n, bq, core) in crate::recorded::RELEASE {
                        ui.label(n);
                        ui.label(format!("{bq:.2e}"));
                        ui.label(format!("{:.1e}", bq / core));
                        ui.end_row();
                    }
                });
                for (p, sv) in crate::recorded::DOSE_400M_SV {
                    ui.label(format!("400 m, {p}: {:.1} mSv", sv * 1e3));
                }
            }
        }
        ui.separator();
        ui.label(format!(
            "Last calculation: {:.1} ms in the background {}.",
            self.last_ms,
            if cfg!(target_arch = "wasm32") {
                "worker"
            } else {
                "thread"
            }
        ));
        if let Some((_, t)) = self.in_flight {
            ui.label(format!("Computing ({:.1} s)…", now_s() - t));
        }
    }

    /// Controls and notes of a step animation.
    fn step_panel(&mut self, ui: &mut egui::Ui, st: Step) {
        let c = &mut self.c;
        ui.horizontal_wrapped(|ui| {
            ui.label("Stability (pyDOSEIA):");
            for (i, n) in CLASSES.iter().enumerate() {
                ui.selectable_value(&mut c.class, i as u8, *n);
            }
        });
        ui.add(egui::Slider::new(&mut c.wind, 0.5..=12.0).text("wind at 10 m, m/s"));
        ui.add(egui::Slider::new(&mut c.h, if st == Step::Rise { 10.0..=120.0 } else { 0.0..=120.0 }).text("release height, m"));
        if st == Step::Rise {
            ui.add(egui::Slider::new(&mut c.w0, 0.0..=30.0).text("exit velocity W0, m/s"));
        }
        if matches!(st, Step::Tracers | Step::Rise) {
            ui.add(egui::Slider::new(&mut c.speed, 5.0..=300.0).logarithmic(true).text("simulated s per real s"));
        }
        if st == Step::Slice {
            ui.add(egui::Slider::new(&mut self.slice_x, 100.0..=crate::steps::REACH_M).text("downwind distance x, m"));
        }
        if matches!(st, Step::Tracers | Step::Rise | Step::Slice) {
            ui.horizontal(|ui| {
                if ui.button(if self.step_playing { "Pause" } else { "Play" }).clicked() {
                    self.step_playing = !self.step_playing;
                }
                if ui.button("Restart").clicked() {
                    self.tracers.clear();
                    self.slice_x = 100.0;
                }
            });
        }
        let note = match st {
            Step::Tracers => "Each tracer is carried at the wind speed and random-walks across the wind with steps sized so that, at distance x, the crowd has buangkok's sigma_y(x) and sigma_z(x). Predict first: if the wind doubles, does the concentration 1 km downwind go up, down, or stay the same?",
            Step::Slice => "The slice widens and fades, but the number under it stays 1: every crosswind plane carries the whole release (buangkok's flux check, the lesson's conservation argument). Predict first: when sigma_z doubles at the same sigma_y, what happens to the peak?",
            Step::Mirror => "The ground is a wall the plume cannot cross. Adding a mirror source at -H folds the would-be underground part back up: that is the second exponential in chi/Q.",
            Step::DayNight => "changi's Pasquill table picks the class from the wind and the hour; the same wind gives a broad, low daytime plume and a thin, long night one. Ground-level chi/Q from buangkok's plume.",
            Step::Rise => "A hot or fast plume keeps rising after it leaves the stack, until the wind bends it over: the effective height H_e is what the plume formula should use. pyDOSEIA's rise formulas, unchecked against their sources (#542).",
            Step::SinglePuff => "",
        };
        ui.label(note);
        ui.label("Tracers are an illustration (they carry no concentration). Research, education and V&V only.");
        ui.separator();
        ui.label(format!(
            "Last calculation: {:.1} ms in the background {}.",
            self.last_ms,
            if cfg!(target_arch = "wasm32") { "worker" } else { "thread" }
        ));
    }

    fn canvas(&mut self, ui: &mut egui::Ui) {
        if let Some(st) = self.step.filter(|s| *s != Step::SinglePuff) {
            let (resp, painter) = ui.allocate_painter(ui.available_size(), egui::Sense::click_and_drag());
            let rect = resp.rect;
            painter.rect_filled(rect, 0.0, Color32::from_rgb(14, 16, 20));
            let below = Rect::from_min_max(rect.min + Vec2::new(0.0, 52.0), rect.max);
            if let Some(f) = &self.step_frame {
                step_view::draw(&painter, below, st, f, &self.tracers, self.plot_text);
            } else {
                painter.text(rect.center(), egui::Align2::CENTER_CENTER, "Computing in the background…", egui::FontId::proportional(15.0), Color32::LIGHT_GRAY);
            }
            self.panel.reopen_button(ui, rect);
            if let Some(z) = zoom_buttons(ui, rect) {
                self.plot_text = match z {
                    Zoom::In => (self.plot_text * 1.2).min(28.0),
                    Zoom::Out => (self.plot_text / 1.2).max(8.0),
                    Zoom::Reset => 13.0,
                };
            }
            return;
        }
        let (resp, painter) =
            ui.allocate_painter(ui.available_size(), egui::Sense::click_and_drag());
        let rect = resp.rect;
        painter.rect_filled(rect, 0.0, Color32::from_rgb(14, 16, 20));
        if self.rung.is_map() {
            self.view.handle_input(ui, &resp);
            self.draw_map(&painter, rect);
        } else {
            draw_plot(&painter, rect, &self.curves, self.rung, self.plot_text);
        }
        if let Some(e) = &self.error {
            painter.text(
                rect.center(),
                egui::Align2::CENTER_CENTER,
                e,
                egui::FontId::proportional(14.0),
                Color32::from_rgb(255, 130, 130),
            );
        } else if self.field.is_none() && self.curves.is_empty() {
            painter.text(
                rect.center() - Vec2::new(0.0, 60.0),
                egui::Align2::CENTER_CENTER,
                "Computing in the background…",
                egui::FontId::proportional(15.0),
                Color32::LIGHT_GRAY,
            );
        }
        self.panel.reopen_button(ui, rect);
        if let Some(z) = zoom_buttons(ui, rect) {
            if self.rung.is_map() {
                apply_zoom(&mut self.view, rect, z);
            } else {
                self.plot_text = match z {
                    Zoom::In => (self.plot_text * 1.2).min(28.0),
                    Zoom::Out => (self.plot_text / 1.2).max(8.0),
                    Zoom::Reset => 13.0,
                };
            }
        }
    }

    fn draw_map(&self, painter: &egui::Painter, rect: Rect) {
        let v = &self.view;
        if let Some(f) = &self.field {
            let n = f.cells.max(1);
            let step = 2.0 * f.half_width / n as f64;
            if f.max > 0.0 {
                for row in 0..n {
                    for col in 0..n {
                        let val = f.values[(row * n + col) as usize];
                        let t = ((val / f.max).log10() + 4.0) / 4.0;
                        if !(t > 0.0) {
                            continue;
                        }
                        let x0 = -f.half_width + col as f64 * step;
                        let y1 = f.half_width - row as f64 * step;
                        let a = v.to_screen(rect, x0, y1);
                        let b = v.to_screen(rect, x0 + step, y1 - step);
                        painter.rect_filled(Rect::from_two_pos(a, b), 0.0, ramp(t.min(1.0)));
                    }
                }
            }
            for p in f.puffs.chunks(3) {
                let c = v.to_screen(rect, p[0], p[1]);
                painter.circle_stroke(
                    c,
                    (p[2] * v.scale) as f32,
                    Stroke::new(1.0, Color32::from_rgba_unmultiplied(255, 255, 255, 60)),
                );
            }
            let what = match self.rung {
                Rung::Deposition => "deposition per unit release, relative (log, 4 decades)",
                Rung::Puffs => {
                    "instantaneous ground-level concentration, relative (log, 4 decades)"
                }
                _ => "ground-level chi/Q, relative to its maximum (log, 4 decades)",
            };
            let mut legend = format!("{what}; max {:.2e}", f.max);
            if self.rung == Rung::Puffs {
                legend.push_str(&format!(
                    "; t = {:.0} s; {} puffs",
                    f.time_s,
                    f.puffs.len() / 3
                ));
            } else if self.rung == Rung::Plume {
                legend.push_str(" s/m3");
            } else {
                legend.push_str(" m^-2");
            }
            let galley = painter.layout(
                legend,
                egui::FontId::proportional(12.0),
                Color32::LIGHT_GRAY,
                rect.width() - 24.0,
            );
            let at = rect.left_bottom() + Vec2::new(12.0, -34.0 - galley.size().y);
            painter.galley(at, galley, Color32::LIGHT_GRAY);
        }
        // The source and the wind.
        let s = v.to_screen(rect, 0.0, 0.0);
        painter.circle_filled(s, 4.0, Color32::from_rgb(255, 90, 90));
        let t = self.c.dir_deg.to_radians();
        let tip = s + Vec2::new(t.sin() as f32, -(t.cos() as f32)) * 40.0;
        painter.arrow(
            s,
            tip - s,
            Stroke::new(2.0, Color32::from_rgb(255, 210, 120)),
        );
        metre_scale_bar(painter, rect, v);
    }
}

/// Dark to yellow-white, for `t` in (0, 1].
fn ramp(t: f64) -> Color32 {
    let t = t.clamp(0.0, 1.0) as f32;
    let r = (40.0 + 215.0 * t.powf(0.8)) as u8;
    let g = (20.0 + 220.0 * t.powf(1.6)) as u8;
    let b = (90.0 + 80.0 * (1.0 - t) - 40.0 * t) as u8;
    Color32::from_rgba_unmultiplied(r, g, b, (90.0 + 165.0 * t) as u8)
}

/// A scale bar in metres (the library's `scale_bar` labels its world unit as
/// cm, the Monte Carlo demo's).
fn metre_scale_bar(painter: &egui::Painter, rect: Rect, view: &View) {
    let bar = [
        10.0, 20.0, 50.0, 100.0, 200.0, 500.0, 1000.0, 2000.0, 5000.0, 10_000.0,
    ]
    .into_iter()
    .find(|&m| m * view.scale >= 80.0)
    .unwrap_or(10_000.0);
    let x0 = rect.left() + 16.0;
    let y0 = rect.bottom() - 12.0;
    let x1 = x0 + (bar * view.scale) as f32;
    painter.line_segment(
        [Pos2::new(x0, y0), Pos2::new(x1, y0)],
        Stroke::new(2.0, Color32::WHITE),
    );
    let label = if bar >= 1000.0 {
        format!("{} km", bar / 1000.0)
    } else {
        format!("{bar} m")
    };
    painter.text(
        Pos2::new(x1 + 6.0, y0),
        egui::Align2::LEFT_CENTER,
        label,
        egui::FontId::proportional(12.0),
        Color32::WHITE,
    );
}

/// Log-log plot of the series, with a legend. Published curves solid, ours
/// dotted, references dashed, recorded points as markers (the workspace's
/// plotting convention).
fn draw_plot(painter: &egui::Painter, rect: Rect, series: &[Series], rung: Rung, text: f32) {
    let font = egui::FontId::proportional(text);
    let small = egui::FontId::proportional((text * 0.85).max(8.0));
    let legend_h = (series.len() as f32) * (text * 1.35) * 2.0 + 8.0;
    let plot = Rect::from_min_max(
        rect.left_top() + Vec2::new(text * 4.5, 56.0),
        rect.right_bottom() - Vec2::new(16.0, legend_h + text * 2.5),
    );
    if plot.width() < 40.0 || plot.height() < 40.0 || series.is_empty() {
        return;
    }
    let pts = series
        .iter()
        .flat_map(|s| s.xs.iter().zip(&s.ys))
        .filter(|(x, y)| **x > 0.0 && **y > 0.0);
    let (mut x0, mut x1, mut y0, mut y1) = (f64::MAX, f64::MIN, f64::MAX, f64::MIN);
    for (x, y) in pts {
        x0 = x0.min(x.log10());
        x1 = x1.max(x.log10());
        y0 = y0.min(y.log10());
        y1 = y1.max(y.log10());
    }
    if x0 >= x1 || y0 > y1 {
        return;
    }
    y1 = y1.ceil();
    y0 = y0.floor().max(y1 - 8.0);
    if y0 >= y1 {
        y0 = y1 - 1.0;
    }
    let to = |x: f64, y: f64| {
        Pos2::new(
            plot.left() + ((x.log10() - x0) / (x1 - x0)) as f32 * plot.width(),
            plot.bottom() - ((y.log10() - y0) / (y1 - y0)) as f32 * plot.height(),
        )
    };
    painter.rect_stroke(
        plot,
        0.0,
        Stroke::new(1.0, Color32::from_gray(90)),
        egui::StrokeKind::Inside,
    );
    for d in (y0 as i32)..=(y1 as i32) {
        let p = to(10f64.powf(x0), 10f64.powi(d));
        painter.line_segment(
            [Pos2::new(plot.left(), p.y), Pos2::new(plot.right(), p.y)],
            Stroke::new(0.5, Color32::from_gray(50)),
        );
        painter.text(
            Pos2::new(plot.left() - 4.0, p.y),
            egui::Align2::RIGHT_CENTER,
            format!("1e{d}"),
            small.clone(),
            Color32::GRAY,
        );
    }
    for d in (x0.ceil() as i32)..=(x1.floor() as i32) {
        let p = to(10f64.powi(d), 10f64.powf(y0));
        painter.line_segment(
            [Pos2::new(p.x, plot.top()), Pos2::new(p.x, plot.bottom())],
            Stroke::new(0.5, Color32::from_gray(50)),
        );
        let label = if d >= 3 {
            format!("{} km", 10f64.powi(d - 3))
        } else {
            format!("{} m", 10f64.powi(d))
        };
        painter.text(
            Pos2::new(p.x, plot.bottom() + 4.0),
            egui::Align2::CENTER_TOP,
            label,
            small.clone(),
            Color32::GRAY,
        );
    }
    let (title, ylabel) = match rung {
        Rung::Sigmas => ("Dispersion coefficients against distance", "sigma, m"),
        Rung::RiseWake => (
            "Ground-level centreline chi/Q against distance",
            "chi/Q, s/m3",
        ),
        _ => (
            "Dose against distance (bounding model; never a dose to a person)",
            "mSv",
        ),
    };
    painter.text(
        rect.left_top() + Vec2::new(12.0, 14.0),
        egui::Align2::LEFT_TOP,
        title,
        font.clone(),
        Color32::WHITE,
    );
    painter.text(
        Pos2::new(plot.left(), plot.top() - 4.0),
        egui::Align2::LEFT_BOTTOM,
        ylabel,
        small.clone(),
        Color32::GRAY,
    );
    for s in series {
        let line: Vec<Pos2> =
            s.xs.iter()
                .zip(&s.ys)
                .filter(|(x, y)| **x > 0.0 && **y >= 10f64.powf(y0))
                .map(|(x, y)| to(*x, *y))
                .collect();
        let colour = PALETTE[(s.colour as usize) % PALETTE.len()];
        match s.style {
            0 => {
                painter.add(egui::Shape::line(line, Stroke::new(3.0, colour)));
            }
            1 => painter.extend(egui::Shape::dotted_line(&line, colour, 5.0, 1.3)),
            2 => painter.extend(egui::Shape::dashed_line(
                &line,
                Stroke::new(1.5, colour),
                8.0,
                5.0,
            )),
            _ => {
                for p in line {
                    painter.circle_filled(p, 3.5, colour);
                }
            }
        }
    }
    let mut y = plot.bottom() + text * 2.2;
    for s in series {
        let colour = PALETTE[(s.colour as usize) % PALETTE.len()];
        let style = ["solid", "dotted", "dashed", "markers"][(s.style as usize).min(3)];
        let galley = painter.layout(
            format!("{style}: {}", s.label),
            small.clone(),
            colour,
            rect.width() - 24.0,
        );
        let h = galley.size().y;
        painter.galley(Pos2::new(rect.left() + 12.0, y), galley, colour);
        y += h + 4.0;
    }
}

impl eframe::App for DispApp {
    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        let ctx = ui.ctx().clone();
        if let Some(link) = &self.link {
            let events = link.drain();
            self.handle(events);
        }
        self.animate();
        self.pump();
        let stepping = self.step.is_some() && self.step_playing;
        if self.rung == Rung::Puffs && self.c.running || self.in_flight.is_some() || stepping {
            ctx.request_repaint_after(std::time::Duration::from_millis(30));
        }
        let title = format!("Dispersion demo · {}", self.rung.title());
        if title != self.title {
            set_title(&ctx, &title);
            self.title = title;
        }
        let mut panel = std::mem::take(&mut self.panel);
        panel.show(ui, "Dispersion", |ui| self.side_panel(ui));
        self.panel = panel;
        egui::CentralPanel::default()
            .frame(egui::Frame::NONE)
            .show(ui, |ui| self.canvas(ui));
    }
}
