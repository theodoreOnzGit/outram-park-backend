//! The page: the same neutron in two panes (surface tracking | delta
//! tracking), side by side on a wide screen and stacked on a phone, one step
//! per tap; and "Run many", `k∞` by both methods with the cost per history.
//!
//! The UI thread only draws: every neutron and every generation comes from
//! the engine ([`crate::engine`]) through a [`Link`]. The state that is not
//! drawing (cursors, the run schedule, predictions) is in [`crate::state`]
//! and tested there; this file is GUI drawing and input, exempt from the
//! reached-by-a-test rule as GUI drawing code.

use crate::engine::{job_labels, job_weights, Event, Request};
use crate::model;
use crate::physics::{Method, Physics, RunConfig};
use crate::state::{ManyRun, MarkKind, Pair, PaneTrace, Predict, SegKind};
use crate::wire::{describe, length};
use dhoby_ghaut::web_demo::link::Link;
use dhoby_ghaut::web_demo::loading::Loading;
use dhoby_ghaut::web_demo::panel::Panel;
use dhoby_ghaut::web_demo::platform::{autostart, now_s, query_pairs, query_value, set_title};
use dhoby_ghaut::web_demo::view::{apply_zoom, scale_bar, zoom_buttons, View, Zoom};
use egui::{Color32, Pos2, Rect, Sense, Stroke, StrokeKind, Vec2};
use outram_mc_libs::geometry::cell::RegionToken;
use outram_mc_libs::geometry::geometry::Geometry;
use outram_mc_libs::geometry::surface::SurfaceKind;
use outram_mc_libs::physics::tracking_trace::TraceEvent;

type DtLink = Link<Request, Event>;

/// The lesson's delta-tracking step, from `demos/delta-tracking/`.
const LESSON: &str = "../../tutorials/monte-carlo/triso.html#4-delta-woodcock-tracking";

const BG: Color32 = Color32::from_rgb(14, 16, 20);
const HELIUM: Color32 = Color32::from_rgb(24, 28, 36);
const MATERIAL_COLOURS: [Color32; 7] = [
    Color32::from_rgb(214, 120, 46),  // UO2 kernel
    Color32::from_rgb(58, 58, 62),    // buffer
    Color32::from_rgb(150, 150, 154), // inner PyC
    Color32::from_rgb(200, 176, 112), // SiC
    Color32::from_rgb(150, 150, 154), // outer PyC
    Color32::from_rgb(92, 94, 98),    // matrix graphite
    Color32::from_rgb(74, 76, 80),    // shell graphite
];
const SURFACE_COLOUR: Color32 = Color32::from_rgb(255, 176, 64);
const DELTA_COLOUR: Color32 = Color32::from_rgb(90, 200, 255);
const LOW_COLOUR: Color32 = Color32::from_rgb(235, 110, 220);
const TEXT: Color32 = Color32::from_rgb(214, 218, 226);
const DIM: Color32 = Color32::from_rgb(150, 156, 170);

#[derive(Clone, Copy, PartialEq, Eq)]
enum Screen {
    Step,
    Many,
}

enum Phase {
    Loading(Loading),
    Ready,
    Failed(String),
}

/// The step view's predictions.
struct StepPredictions {
    first_flight: Predict,
    stops: Predict,
}

/// "Run many"'s predictions.
struct ManyPredictions {
    cost: Predict,
    agree: Predict,
    low: Predict,
}

pub struct App {
    link: Option<DtLink>,
    phase: Phase,
    screen: Screen,
    panel: Panel,
    view: View,
    follow: bool,
    /// Zoom of the "Run many" text (its + / − / Reset).
    text_zoom: f32,
    geometry: Geometry,
    centres: Vec<(f64, f64)>,
    /// The surfaces bounding each cell (what a distance-to-boundary query tests).
    cell_surfaces: Vec<Vec<usize>>,
    pair: Option<Pair>,
    seed: u64,
    rate: f64,
    low_on: bool,
    low_factor: f64,
    waiting_trace: bool,
    run_cfg: RunConfig,
    run: Option<ManyRun>,
    step_predict: StepPredictions,
    many_predict: ManyPredictions,
    majorant: Option<(f64, usize)>,
    load_timings: Vec<(&'static str, f64)>,
    load_total_s: f64,
    autostart: bool,
    title: String,
}

impl App {
    pub fn new(cc: &eframe::CreationContext<'_>) -> Self {
        cc.egui_ctx.set_theme(egui::ThemePreference::Dark);
        #[cfg(not(target_arch = "wasm32"))]
        let link: Result<DtLink, String> = {
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
        let q = query_pairs();
        let num = |k: &str, d: f64| {
            query_value(&q, k)
                .and_then(|v| v.parse::<f64>().ok())
                .unwrap_or(d)
        };
        let low = num("low", 0.0);
        let centres = model::particle_centres(model::LAYOUT_SEED);
        let geometry = model::build_geometry(&centres);
        let cell_surfaces = geometry
            .cells
            .iter()
            .map(|c| {
                c.region
                    .iter()
                    .filter_map(|t| match t {
                        RegionToken::HalfSpace { surface_idx, .. } => Some(*surface_idx),
                        _ => None,
                    })
                    .collect()
            })
            .collect();
        let mut view = View::new(model::half_pitch());
        view.scale = 1.0;
        let mut app = Self {
            link: None,
            phase: Phase::Loading(Loading::new(job_labels(), job_weights())),
            screen: if query_value(&q, "mode") == Some("many") { Screen::Many } else { Screen::Step },
            panel: Panel::default(),
            view,
            follow: true,
            text_zoom: 1.0,
            geometry,
            centres,
            cell_surfaces,
            pair: None,
            seed: num("seed", 1.0) as u64,
            rate: 4.0,
            low_on: low > 0.0,
            low_factor: if low > 0.0 { low } else { 0.1 },
            waiting_trace: false,
            run_cfg: RunConfig {
                n_particles: num("n", 500.0) as usize,
                n_inactive: num("inactive", 5.0) as usize,
                n_active: num("active", 40.0) as usize,
                seed: num("runseed", 784.0) as u64,
                low_factor: None,
            },
            run: None,
            step_predict: StepPredictions {
                first_flight: Predict::new(
                    "Both trackers draw the SAME first random number ξ. Whose first flight is longer?",
                    &["surface tracking's", "delta tracking's", "the same"],
                ),
                stops: Predict::new(
                    "Over the whole history, which tracker stops more often (surfaces, or tentative collisions)?",
                    &["surface tracking", "delta tracking", "about the same"],
                ),
            },
            many_predict: ManyPredictions {
                cost: Predict::new("Which method costs less time per neutron here?", &["surface tracking", "delta tracking", "about the same"]),
                agree: Predict::new("Will the two k∞ agree within their uncertainty?", &["yes", "no"]),
                low: Predict::new(
                    "With the majorant too low, k∞ will…",
                    &["go up, with a warning", "go down, with a warning", "move, with no warning", "not move"],
                ),
            },
            majorant: None,
            load_timings: Vec::new(),
            load_total_s: 0.0,
            autostart: autostart(),
            title: String::new(),
        };
        match link {
            Ok(l) => {
                l.send(Request::Load { id: 1 });
                app.link = Some(l);
            }
            Err(e) => app.phase = Phase::Failed(format!("could not start the physics worker: {e}")),
        }
        app
    }

    fn factor(&self) -> f64 {
        if self.low_on {
            self.low_factor
        } else {
            1.0
        }
    }

    fn ask_trace(&mut self) {
        if let Some(l) = &self.link {
            l.send(Request::Trace {
                seed: self.seed,
                factor: self.factor(),
            });
            self.waiting_trace = true;
            self.step_predict.first_flight.chosen = None;
            self.step_predict.stops.chosen = None;
        }
    }

    fn start_run(&mut self) {
        let mut cfg = self.run_cfg;
        cfg.low_factor = self.low_on.then_some(self.low_factor);
        if let Some(l) = &self.link {
            l.send(Request::RunStart(cfg));
            self.run = Some(ManyRun::new(cfg));
        }
    }

    fn handle(&mut self, events: Vec<Event>) {
        for e in events {
            match (&mut self.phase, e) {
                (_, Event::Error(m)) => {
                    if let Some(r) = &mut self.run {
                        r.failed();
                    }
                    self.phase = Phase::Failed(m);
                }
                (Phase::Loading(l), Event::JobStarted { index, .. }) => l.job_started(index),
                (Phase::Loading(l), Event::JobDone { index, secs, .. }) => l.job_done(index, secs),
                (
                    Phase::Loading(l),
                    Event::Ready {
                        majorant_secs,
                        points,
                        ..
                    },
                ) => {
                    self.load_timings = l.timings();
                    self.load_total_s = now_s() - l.started;
                    self.majorant = Some((majorant_secs, points));
                    self.phase = Phase::Ready;
                    self.ask_trace();
                    if self.autostart && self.screen == Screen::Many {
                        self.start_run();
                    }
                }
                (
                    Phase::Ready,
                    Event::Trace {
                        seed,
                        factor,
                        surface,
                        delta,
                        ..
                    },
                ) => {
                    self.waiting_trace = false;
                    let mut p = Pair::new(seed, factor, surface, delta, Physics::domain());
                    p.playing = self.autostart && self.screen == Screen::Step;
                    self.pair = Some(p);
                }
                (Phase::Ready, Event::Gen(g)) => {
                    if let Some(r) = &mut self.run {
                        r.receive(g);
                    }
                }
                _ => {}
            }
        }
    }

    fn pump(&mut self) {
        if let (Some(run), Some(link)) = (&mut self.run, &self.link) {
            if let Some(m) = run.next_request() {
                link.send(Request::RunStep(m));
            }
        }
    }

    fn title_now(&mut self, ctx: &egui::Context) {
        let name = "Delta vs surface tracking";
        let t = match (&self.phase, self.screen) {
            (Phase::Loading(l), _) => format!(
                "{name} · loading {}/{} · {}",
                l.done(),
                l.labels.len(),
                l.status_line()
            ),
            (Phase::Failed(e), _) => format!("{name} · FAILED · {e}"),
            (Phase::Ready, Screen::Step) => match &self.pair {
                Some(p) => format!(
                    "{name} · neutron {} · surface {}/{} · delta {}/{}{}",
                    p.seed,
                    p.surface.cursor,
                    p.surface.len(),
                    p.delta.cursor,
                    p.delta.len(),
                    if p.finished() { " · done" } else { "" }
                ),
                None => format!("{name} · tracing"),
            },
            (Phase::Ready, Screen::Many) => match &self.run {
                Some(r) => {
                    let part = |m: Method| {
                        r.tally(m).map_or(String::new(), |t| {
                            let (k, s) = t.k.unwrap_or((f64::NAN, f64::NAN));
                            format!(
                                " | {} g{} k={k:.5}±{s:.5} {:.0}us/h",
                                m.label(),
                                t.generations,
                                t.us_per_history()
                            )
                        })
                    };
                    format!(
                        "{name} · run {}{}{}{}",
                        if r.all_done() { "done" } else { "running" },
                        part(Method::Surface),
                        part(Method::Delta),
                        part(Method::DeltaLow)
                    )
                }
                None => format!("{name} · run many: ready"),
            },
        };
        if t != self.title {
            set_title(ctx, &t);
            self.title = t;
        }
    }
}

impl eframe::App for App {
    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        let ctx = ui.ctx().clone();
        if let Some(link) = &self.link {
            let events = link.drain();
            self.handle(events);
        }
        self.pump();
        let dt = ui.input(|i| i.stable_dt).min(0.1) as f64;
        // Progress (loading, a run) needs a slow tick only: new results
        // repaint on arrival. Play needs frames.
        let ticking =
            matches!(self.phase, Phase::Loading(_)) || self.run.as_ref().is_some_and(|r| r.running);
        let mut playing = false;
        if let Some(p) = &mut self.pair {
            if self.screen == Screen::Step && p.playing {
                p.play(dt, self.rate);
                playing = true;
            }
        }
        if playing {
            ctx.request_repaint_after(std::time::Duration::from_millis(30));
        } else if ticking {
            ctx.request_repaint_after(std::time::Duration::from_millis(250));
        }
        self.title_now(&ctx);
        let mut panel = std::mem::take(&mut self.panel);
        panel.show(ui, "Delta vs surface", |ui| self.side_panel(ui));
        self.panel = panel;
        egui::CentralPanel::default()
            .frame(egui::Frame::NONE)
            .show(ui, |ui| self.main_view(ui));
    }
}

// ─── Side panel ──────────────────────────────────────────────────────────────

fn predict_ui(ui: &mut egui::Ui, p: &mut Predict, answer: Option<String>) {
    egui::Frame::group(ui.style()).show(ui, |ui| {
        ui.label(egui::RichText::new(format!("Predict: {}", p.question)).strong());
        ui.horizontal_wrapped(|ui| {
            for (i, o) in p.options.iter().enumerate() {
                ui.radio_value(&mut p.chosen, Some(i), *o);
            }
        });
        match (p.chosen, answer) {
            (None, _) => {
                ui.label(
                    egui::RichText::new(
                        "Choose before you look: the answer appears once you have.",
                    )
                    .small()
                    .color(DIM),
                );
            }
            (Some(_), None) => {
                ui.label(
                    egui::RichText::new("Now step on to see.")
                        .small()
                        .color(DIM),
                );
            }
            (Some(_), Some(a)) => {
                if let Some(v) = p.verdict() {
                    ui.label(egui::RichText::new(v).small());
                }
                ui.label(a);
            }
        }
    });
}

impl App {
    fn side_panel(&mut self, ui: &mut egui::Ui) {
        ui.add(
            egui::Hyperlink::from_label_and_url(
                "What's happening here? (the lesson, step 4)",
                LESSON,
            )
            .open_in_new_tab(true),
        );
        ui.label(
            egui::RichText::new(
                "The same neutron in the same TRISO cell, transported by outram-mc-libs two ways: surface tracking \
                 (left or top) and delta tracking (right or bottom).",
            )
            .small(),
        );
        ui.horizontal_wrapped(|ui| {
            ui.selectable_value(&mut self.screen, Screen::Step, "Step through one neutron");
            ui.selectable_value(&mut self.screen, Screen::Many, "Run many");
        });
        ui.separator();
        match &self.phase {
            Phase::Loading(l) => {
                ui.label("Processing ENDF/B-VIII.0 in a background worker (the page stays live):");
                l.grid(ui);
                return;
            }
            Phase::Failed(e) => {
                ui.colored_label(Color32::from_rgb(255, 90, 90), format!("Failed: {e}"));
                return;
            }
            Phase::Ready => {}
        }
        // The majorant switch serves both screens.
        ui.horizontal_wrapped(|ui| {
            let was = (self.low_on, self.low_factor);
            ui.checkbox(&mut self.low_on, "Majorant too low");
            ui.add_enabled(
                self.low_on,
                egui::Slider::new(&mut self.low_factor, 0.05..=1.0)
                    .text("× the bound")
                    .fixed_decimals(2),
            );
            if (self.low_on, self.low_factor) != was
                && self.screen == Screen::Step
                && !self.waiting_trace
            {
                self.ask_trace();
            }
        });
        if self.low_on {
            ui.label(
                egui::RichText::new(
                    "Delta tracking now flies on Σmaj × that factor. Where Σt is above it, the accept test always passes and \
                     collisions are lost. Watch for ⚠ sites; the code itself prints nothing.",
                )
                .small()
                .color(LOW_COLOUR),
            );
        }
        ui.separator();
        match self.screen {
            Screen::Step => self.step_controls(ui),
            Screen::Many => self.many_controls(ui),
        }
        ui.separator();
        self.legend(ui);
        egui::CollapsingHeader::new("What this is, and is not").default_open(false).show(ui, |ui| {
            for line in NOTES {
                ui.label(format!("• {line}"));
            }
            if let Some((secs, points)) = self.majorant {
                ui.label(format!("• The majorant here took {secs:.1} s to build and has {points} energy points."));
            }
        });
        if !self.load_timings.is_empty() {
            egui::CollapsingHeader::new(format!("Data processing: {:.0} s", self.load_total_s))
                .show(ui, |ui| {
                    for (l, s) in &self.load_timings {
                        ui.label(format!("{l:<16} {s:6.1} s"));
                    }
                });
        }
    }

    fn step_controls(&mut self, ui: &mut egui::Ui) {
        let Some(p) = &mut self.pair else {
            ui.label("Tracing the neutron…");
            return;
        };
        ui.label(format!(
            "Neutron {} · surface step {}/{} · delta step {}/{}",
            p.seed,
            p.surface.cursor,
            p.surface.len(),
            p.delta.cursor,
            p.delta.len()
        ));
        ui.horizontal_wrapped(|ui| {
            if ui
                .button("Step")
                .on_hover_text("One step in each pane")
                .clicked()
            {
                p.step();
            }
            if ui.button("Next collision").clicked() {
                p.next_collision();
            }
            if ui
                .button(if p.playing { "Pause" } else { "Play" })
                .clicked()
            {
                p.playing = !p.playing;
            }
            if ui.button("To the end").clicked() {
                p.to_end();
            }
            if ui
                .button("Restart")
                .on_hover_text("Back to the birth of this neutron")
                .clicked()
            {
                p.rewind();
            }
        });
        ui.add(
            egui::Slider::new(&mut self.rate, 1.0..=400.0)
                .logarithmic(true)
                .text("steps per second (Play)"),
        );
        ui.horizontal_wrapped(|ui| {
            ui.checkbox(&mut self.follow, "Follow the neutron");
        });
        let mut new = false;
        ui.horizontal_wrapped(|ui| {
            if ui.button("New neutron").clicked() && !self.waiting_trace {
                self.seed += 1;
                new = true;
            }
            ui.label(format!("(seed {})", self.seed));
        });
        // The answers come from the traces themselves.
        let first = first_flights(p);
        let ans1 = first.map(|(xi, d_col, sig_t, s, maj)| {
            format!(
                "Same ξ = {xi:.3}. Surface: −ln ξ / Σt = {} with Σt = {sig_t:.3}/cm where it is. Delta: −ln ξ / Σmaj = {} with Σmaj = {maj:.3}/cm. \
                 Delta's is shorter by Σt/Σmaj = {:.3}: Σmaj ≥ Σt everywhere, so a delta flight is never the longer.",
                length(d_col),
                length(s),
                sig_t / maj
            )
        });
        let ans2 = p.finished().then(|| {
            let (s, d) = (p.surface.trace.counts, p.delta.trace.counts);
            format!(
                "Surface tracking stopped {} times ({} surface queries, {} crossings); delta tracking {} times ({} virtual, {} real). \
                 Real collisions: {} and {}. A stop costs different amounts in each: see Run many for the time.",
                s.stops(),
                s.boundary_queries,
                s.crossings,
                d.stops(),
                d.virtual_collisions,
                d.tentative - d.virtual_collisions,
                s.collisions,
                d.collisions
            )
        });
        predict_ui(ui, &mut self.step_predict.first_flight, ans1);
        predict_ui(ui, &mut self.step_predict.stops, ans2);
        if new {
            self.ask_trace();
        }
    }

    fn many_controls(&mut self, ui: &mut egui::Ui) {
        let running = self.run.as_ref().is_some_and(|r| r.running);
        ui.add_enabled_ui(!running, |ui| {
            ui.add(
                egui::Slider::new(&mut self.run_cfg.n_particles, 20..=2000)
                    .logarithmic(true)
                    .text("neutrons per generation"),
            );
            ui.add(
                egui::Slider::new(&mut self.run_cfg.n_inactive, 1..=20)
                    .text("inactive generations"),
            );
            ui.add(
                egui::Slider::new(&mut self.run_cfg.n_active, 2..=100).text("active generations"),
            );
        });
        ui.horizontal_wrapped(|ui| {
            if running {
                if ui.button("Stop").clicked() {
                    if let Some(r) = &mut self.run {
                        r.running = false;
                    }
                }
            } else if ui.button("Run").clicked() {
                self.start_run();
            }
            ui.label(
                egui::RichText::new("Each generation of each method is one request to the worker, in turn, so all of them stream.")
                    .small()
                    .color(DIM),
            );
        });
        let run = self.run.as_ref();
        let done = run.is_some_and(|r| r.all_done());
        let ans_cost = run.filter(|_| done).and_then(|r| {
            let (s, d) = (r.tally(Method::Surface)?, r.tally(Method::Delta)?);
            Some(format!(
                "Surface tracking: {:.0} µs per neutron; delta tracking: {:.0} µs, {:.1} times {}. Per neutron, surface tracking \
                 stopped {:.0} times and delta tracking {:.0}; a surface stop is a locate plus a distance to every surface of \
                 the cell (152 particles' in the matrix), a delta stop one locate.",
                s.us_per_history(),
                d.us_per_history(),
                (s.us_per_history() / d.us_per_history()).max(d.us_per_history() / s.us_per_history()),
                if d.us_per_history() < s.us_per_history() { "faster" } else { "slower" },
                s.per_history(s.counts.stops()),
                d.per_history(d.counts.stops()),
            ))
        });
        let ans_agree = run.filter(|_| done).and_then(|r| {
            let (dk, s, z) = r.difference(Method::Delta, Method::Surface)?;
            Some(format!(
                "Delta − surface = {dk:+.0} ± {s:.0} pcm ({z:+.2} σ): {}. That is unbiasedness, measured on this run, not assumed.",
                if z.abs() <= 2.0 { "they agree" } else { "they DISAGREE beyond 2 σ" }
            ))
        });
        let ans_low = run.filter(|_| done).and_then(|r| {
            let (dk, s, z) = r.difference(Method::DeltaLow, Method::Surface)?;
            let t = r.tally(Method::DeltaLow)?;
            Some(format!(
                "Too-low majorant − surface = {dk:+.0} ± {s:.0} pcm ({z:+.2} σ), with {:.1} sites per neutron where Σt > Σmaj. \
                 The run printed no warning: only the count above, which a production code does not keep, shows it.",
                t.per_history(t.counts.majorant_violations)
            ))
        });
        predict_ui(ui, &mut self.many_predict.cost, ans_cost);
        predict_ui(ui, &mut self.many_predict.agree, ans_agree);
        if self.low_on
            || self
                .run
                .as_ref()
                .is_some_and(|r| r.cfg.low_factor.is_some())
        {
            predict_ui(ui, &mut self.many_predict.low, ans_low);
        } else {
            ui.label(egui::RichText::new("Tick “Majorant too low” before Run to add a third run on a majorant scaled down.").small().color(DIM));
        }
    }

    fn legend(&self, ui: &mut egui::Ui) {
        ui.strong("Legend");
        let row = |ui: &mut egui::Ui, c: Color32, filled: bool, text: &str| {
            ui.horizontal_wrapped(|ui| {
                let (r, _) = ui.allocate_exact_size(Vec2::splat(14.0), Sense::hover());
                if filled {
                    ui.painter().circle_filled(r.center(), 5.0, c);
                } else {
                    ui.painter()
                        .circle_stroke(r.center(), 5.0, Stroke::new(1.5, c));
                }
                ui.label(text);
            });
        };
        row(
            ui,
            SURFACE_COLOUR,
            false,
            "surface tracking; faint circles: the surfaces a query tests, bright: the nearest",
        );
        row(
            ui,
            DELTA_COLOUR,
            false,
            "delta tracking: flights on the majorant",
        );
        row(
            ui,
            Color32::from_rgb(160, 165, 175),
            false,
            "virtual collision (rejected, nothing happens)",
        );
        row(ui, Color32::WHITE, true, "real collision");
        row(
            ui,
            LOW_COLOUR,
            true,
            "⚠ Σt > Σmaj: the bound is broken here",
        );
        row(
            ui,
            Color32::from_rgb(255, 90, 90),
            false,
            "end of the history",
        );
        for (i, name) in model::MATERIAL_NAMES.iter().enumerate() {
            if i == model::MAT_OPYC {
                continue;
            }
            ui.horizontal(|ui| {
                let (r, _) = ui.allocate_exact_size(Vec2::splat(12.0), Sense::hover());
                ui.painter().rect_filled(r, 2.0, MATERIAL_COLOURS[i]);
                ui.label(if i == model::MAT_IPYC {
                    "Pyrolytic carbon (inner & outer)"
                } else {
                    name
                });
            });
        }
        ui.horizontal(|ui| {
            let (r, _) = ui.allocate_exact_size(Vec2::splat(12.0), Sense::hover());
            ui.painter().rect_filled(r, 2.0, HELIUM);
            ui.label("Helium (void; for delta tracking a material with Σt = 0)");
        });
    }
}

/// The first flight of each pane, for the first prediction: `(ξ, d_col,
/// Σt, s, Σmaj)`, once both have been shown.
fn first_flights(p: &Pair) -> Option<(f64, f64, f64, f64, f64)> {
    let shown = |t: &PaneTrace| t.trace.events[..t.cursor].to_vec();
    let s = shown(&p.surface);
    let d = shown(&p.delta);
    let sig_t = s.iter().find_map(|e| {
        if let TraceEvent::Located { sigma_t, .. } = e {
            Some(*sigma_t)
        } else {
            None
        }
    })?;
    let (xi, d_col) = s.iter().find_map(|e| {
        if let TraceEvent::Segment {
            xi: Some(x),
            d_collision,
            ..
        } = e
        {
            Some((*x, *d_collision))
        } else {
            None
        }
    })?;
    let (s_d, maj) = d.iter().find_map(|e| {
        if let TraceEvent::Flight {
            distance, majorant, ..
        } = e
        {
            Some((*distance, *majorant))
        } else {
            None
        }
    })?;
    Some((xi, d_col, sig_t, s_d, maj))
}

const NOTES: &[&str] = &[
    "Transport: outram-mc-libs, unmodified. Surface tracking is trace_csg_history (the kernel of run_keff_csg); delta tracking is trace_delta_history (the kernel of the delta-tracked power iteration). Each step is reported by the library's observer hook, which draws no random number, so what you see is the history the library ran.",
    "Same neutron: the same birth point, direction, energy and random seed. Both draw the same first ξ; after that delta tracking spends a ξ on every accept/reject, so the two random walks differ. They are two samples of the same physics.",
    "Geometry: the triso rung's 2D cell (HTR-10 pebble dimensions, IAEA-TECDOC-1382; 152 TRISO rods). Delta tracking asks only Geometry::locate at each tentative site, on the same CSG model. The CSG model is one flat universe (763 cells, no lattice acceleration), so a surface query in the matrix tests 155 surfaces; a faster surface tracker would narrow the cost gap.",
    "Data: ENDF/B-VIII.0, reconstructed and Doppler-broadened at tolerance 0.01 (not NJOY's 0.001) to 296 K in this browser by OUTRAM PARK's NJOY port; graphite S(α,β). The majorant is Majorant::bounding (every nuclide breakpoint, 10 % margin), over every material.",
    "Run many: k∞ of the cell by CsgPowerIteration (surface) and DeltaPowerIteration (delta), stepped one generation per worker request; time is the worker's wall clock for transport only.",
    "Education and research only.",
];

// ─── Main view ───────────────────────────────────────────────────────────────

impl App {
    fn main_view(&mut self, ui: &mut egui::Ui) {
        let (resp, painter) = ui.allocate_painter(ui.available_size(), Sense::hover());
        let full = resp.rect;
        painter.rect_filled(full, 0.0, BG);
        match &self.phase {
            Phase::Loading(l) => {
                l.card(&painter, full, "Processing nuclear data…", Some("The Monte Carlo demo's tapes, processed here in a worker; then the majorant is built."));
            }
            Phase::Failed(e) => {
                painter.text(
                    full.center(),
                    egui::Align2::CENTER_CENTER,
                    format!("Failed: {e}"),
                    egui::FontId::proportional(15.0),
                    Color32::from_rgb(255, 110, 110),
                );
            }
            Phase::Ready => match self.screen {
                Screen::Step => self.step_view(ui, full, &painter),
                Screen::Many => self.many_view(ui, full, &painter),
            },
        }
        if let Some(z) = zoom_buttons(ui, full) {
            match self.screen {
                Screen::Step => {
                    if z == Zoom::Reset {
                        self.follow = false;
                    }
                    let r = pane_geo_rect(pane_rects(full).0);
                    apply_zoom(&mut self.view, r, z);
                }
                Screen::Many => {
                    self.text_zoom = match z {
                        Zoom::In => (self.text_zoom * 1.2).min(2.5),
                        Zoom::Out => (self.text_zoom / 1.2).max(0.6),
                        Zoom::Reset => 1.0,
                    }
                }
            }
        }
        self.panel.reopen_button(ui, full);
    }

    fn step_view(&mut self, ui: &mut egui::Ui, full: Rect, painter: &egui::Painter) {
        let (ra, rb, bar) = pane_rects(full);
        // The view starts zoomed 12 times onto the neutron (`zoom_about`
        // marks it as the reader's, so it is not refitted on a resize).
        if !self.view.fitted {
            let g = pane_geo_rect(ra);
            self.view.fit(g);
            self.view.zoom_about(g, g.center(), 12.0);
        }
        let Some(pair) = &mut self.pair else {
            painter.text(
                full.center(),
                egui::Align2::CENTER_CENTER,
                "Tracing the neutron…",
                egui::FontId::proportional(15.0),
                TEXT,
            );
            return;
        };
        for (rect, which) in [(ra, Which::Surface), (rb, Which::Delta)] {
            let geo = pane_geo_rect(rect);
            let resp = ui.interact(
                geo,
                ui.id().with(("pane", which as u8)),
                Sense::click_and_drag(),
            );
            self.view.handle_input(ui, &resp);
            if resp.dragged() || (ui.input(|i| i.multi_touch().is_some()) && resp.hovered()) {
                self.follow = false;
            }
            let pane = match which {
                Which::Surface => &pair.surface,
                Which::Delta => &pair.delta,
            };
            let mut v = self.view;
            if self.follow {
                v.centre = pane.position();
            }
            draw_pane(
                painter,
                rect,
                &v,
                pane,
                which,
                pair.factor,
                &self.geometry,
                &self.centres,
                &self.cell_surfaces,
            );
        }
        // The playback bar on the main view: Step is the one control a phone
        // reader needs without opening the panel.
        let w = (bar.width() - 16.0).min(420.0);
        let x0 = bar.center().x - w / 2.0;
        let labels = [
            ("Step", 0),
            ("Next collision", 1),
            (if pair.playing { "Pause" } else { "Play" }, 2),
            ("Restart", 3),
        ];
        let bw = (w - 6.0 * 3.0) / 4.0;
        for (i, (label, code)) in labels.into_iter().enumerate() {
            let r = Rect::from_min_size(
                Pos2::new(x0 + i as f32 * (bw + 6.0), bar.top() + 6.0),
                Vec2::new(bw, 38.0),
            );
            if ui.put(r, egui::Button::new(label)).clicked() {
                match code {
                    0 => pair.step(),
                    1 => pair.next_collision(),
                    2 => pair.playing = !pair.playing,
                    _ => pair.rewind(),
                }
            }
        }
    }

    fn many_view(&mut self, ui: &mut egui::Ui, full: Rect, painter: &egui::Painter) {
        let s = self.text_zoom;
        let f = |px: f32| egui::FontId::proportional(px * s);
        let inner = full.shrink2(Vec2::new(14.0, 0.0));
        let mut y = full.top() + 56.0;
        let mut line = |text: String, px: f32, c: Color32, painter: &egui::Painter| {
            let g = painter.layout(text, f(px), c, inner.width());
            painter.galley(Pos2::new(inner.left(), y), g.clone(), c);
            y += g.size().y + 4.0 * s;
        };
        let Some(run) = &self.run else {
            line(
                "Run many: k∞ of the cell by both methods.".into(),
                17.0,
                TEXT,
                painter,
            );
            line(
                "Choose the size in the panel and press Run. Each method's generations stream in as the worker finishes them. \
                 Make your predictions first."
                    .into(),
                14.0,
                DIM,
                painter,
            );
            return;
        };
        let state = if run.all_done() {
            "done"
        } else if run.running {
            "running"
        } else {
            "stopped"
        };
        line(
            format!(
                "k∞ of the TRISO cell, {} neutrons × [{} inactive + {} active] per method, seed {} · {state}",
                run.cfg.n_particles, run.cfg.n_inactive, run.cfg.n_active, run.cfg.seed
            ),
            15.0,
            TEXT,
            painter,
        );
        for (i, &m) in run.methods.iter().enumerate() {
            let t = &run.tallies[i];
            let c = method_colour(m);
            let k = t.k.map_or(
                "k∞ waiting for the active generations".into(),
                |(k, e)| format!("k∞ = {k:.5} ± {e:.5}"),
            );
            line(
                format!(
                    "{}{} · {k} · generation {}/{}",
                    m.label(),
                    if m == Method::DeltaLow {
                        format!(" (× {:.2})", run.cfg.low_factor.unwrap_or(1.0))
                    } else {
                        String::new()
                    },
                    t.generations,
                    run.total_generations()
                ),
                15.0,
                c,
                painter,
            );
            let ph = |n: u64| t.per_history(n);
            let detail = match m {
                Method::Surface => format!(
                    "{:.0} µs per neutron · per neutron: {:.0} stops = {:.0} locate + distance-to-boundary queries, {:.0} crossings, {:.1} real collisions",
                    t.us_per_history(),
                    ph(t.counts.stops()),
                    ph(t.counts.boundary_queries),
                    ph(t.counts.crossings),
                    ph(t.counts.collisions)
                ),
                Method::Delta | Method::DeltaLow => format!(
                    "{:.0} µs per neutron · per neutron: {:.0} stops = {:.0} flights, {:.0} virtual, {:.1} real collisions · {:.2} sites with Σt > Σmaj",
                    t.us_per_history(),
                    ph(t.counts.stops()),
                    ph(t.counts.flights),
                    ph(t.counts.virtual_collisions),
                    ph(t.counts.collisions),
                    ph(t.counts.majorant_violations)
                ),
            };
            line(detail, 13.0, DIM, painter);
        }
        for (a, label) in [
            (Method::Delta, "delta − surface"),
            (Method::DeltaLow, "too low − surface"),
        ] {
            if let Some((dk, sig, z)) = run.difference(a, Method::Surface) {
                line(
                    format!("{label}: {dk:+.0} ± {sig:.0} pcm ({z:+.2} σ)"),
                    15.0,
                    method_colour(a),
                    painter,
                );
            }
        }
        // The running k of each method against generation.
        let plot = Rect::from_min_max(
            Pos2::new(inner.left(), y + 8.0),
            Pos2::new(inner.right(), (full.bottom() - 16.0).max(y + 120.0)),
        );
        if plot.height() > 80.0 {
            draw_k_plot(painter, plot, run, s);
        }
        let _ = ui;
    }
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum Which {
    Surface = 0,
    Delta = 1,
}

fn method_colour(m: Method) -> Color32 {
    match m {
        Method::Surface => SURFACE_COLOUR,
        Method::Delta => DELTA_COLOUR,
        Method::DeltaLow => LOW_COLOUR,
    }
}

/// The two panes (side by side when the view is wider than tall, stacked
/// otherwise) and the playback bar under them. Leaves room at the top for
/// "Controls »" and + / − / Reset.
fn pane_rects(full: Rect) -> (Rect, Rect, Rect) {
    let top = full.top() + 52.0;
    let bar = Rect::from_min_max(Pos2::new(full.left(), full.bottom() - 52.0), full.max);
    let body = Rect::from_min_max(
        Pos2::new(full.left() + 4.0, top),
        Pos2::new(full.right() - 4.0, bar.top()),
    );
    let (a, b) = if body.width() >= body.height() {
        let (a, b) = body.split_left_right_at_fraction(0.5);
        (
            a.shrink2(Vec2::new(3.0, 0.0)),
            b.shrink2(Vec2::new(3.0, 0.0)),
        )
    } else {
        let (a, b) = body.split_top_bottom_at_fraction(0.5);
        (
            a.shrink2(Vec2::new(0.0, 3.0)),
            b.shrink2(Vec2::new(0.0, 3.0)),
        )
    };
    (a, b, bar)
}

/// The geometry area of a pane: under its header, above its footer.
fn pane_geo_rect(r: Rect) -> Rect {
    Rect::from_min_max(
        Pos2::new(r.left(), r.top() + 40.0),
        Pos2::new(r.right(), r.bottom() - 58.0),
    )
}

#[allow(clippy::too_many_arguments)]
fn draw_pane(
    full_painter: &egui::Painter,
    rect: Rect,
    view: &View,
    pane: &PaneTrace,
    which: Which,
    factor: f64,
    geometry: &Geometry,
    centres: &[(f64, f64)],
    cell_surfaces: &[Vec<usize>],
) {
    let accent = match which {
        Which::Surface => SURFACE_COLOUR,
        Which::Delta => DELTA_COLOUR,
    };
    full_painter.rect_filled(rect, 6.0, Color32::from_rgb(20, 23, 29));
    full_painter.rect_stroke(
        rect,
        6.0,
        Stroke::new(1.0, accent.gamma_multiply(0.6)),
        StrokeKind::Inside,
    );
    let geo = pane_geo_rect(rect);
    let painter = full_painter.with_clip_rect(geo);
    draw_cell(&painter, geo, view, centres);
    let to = |p: [f64; 2]| view.to_screen(geo, p[0], p[1]);
    let cur = pane.cursor;
    let current = pane.current().copied();

    // Candidate surfaces of the current surface-tracking query.
    if let (Which::Surface, Some(ev)) = (which, current) {
        let (cell, hit) = match ev {
            TraceEvent::Located { cell, .. } => (Some(cell), None),
            TraceEvent::Segment { surface, .. } => (located_cell(pane), surface),
            _ => (None, None),
        };
        if let Some(c) = cell {
            for &s in &cell_surfaces[c] {
                draw_surface(
                    &painter,
                    geo,
                    view,
                    geometry,
                    s,
                    Stroke::new(1.0, SURFACE_COLOUR.gamma_multiply(0.45)),
                );
            }
        }
        if let Some(s) = hit {
            draw_surface(
                &painter,
                geo,
                view,
                geometry,
                s,
                Stroke::new(2.5, Color32::from_rgb(255, 230, 90)),
            );
        }
    }

    // The track so far: the last 4000 pieces.
    let shown: Vec<_> = pane.segs.iter().filter(|s| s.at < cur).collect();
    let start = shown.len().saturating_sub(4000);
    let n_shown = shown.len();
    for (i, s) in shown.iter().enumerate().skip(start) {
        let last = i + 1 == n_shown;
        let c = match s.kind {
            SegKind::ToSurface | SegKind::ToCollision => SURFACE_COLOUR,
            SegKind::Flight => DELTA_COLOUR,
        };
        let w = if last { 2.5 } else { 1.2 };
        let pts: Vec<Pos2> = s.points.iter().map(|&p| to(p)).collect();
        painter.add(egui::Shape::line(
            pts,
            Stroke::new(w, if last { c } else { c.gamma_multiply(0.55) }),
        ));
    }
    let marks: Vec<_> = pane.marks.iter().filter(|m| m.at < cur).collect();
    let mstart = marks.len().saturating_sub(4000);
    for m in marks.iter().skip(mstart) {
        let p = to(m.p);
        match m.kind {
            MarkKind::Birth => {
                painter.circle_filled(p, 4.0, Color32::from_rgb(110, 230, 120));
            }
            MarkKind::Crossing => {
                painter.circle_filled(p, 1.6, Color32::from_rgb(255, 230, 90));
            }
            MarkKind::Virtual => {
                painter.circle_stroke(p, 2.6, Stroke::new(1.0, Color32::from_rgb(160, 165, 175)));
            }
            MarkKind::Violation => {
                painter.circle_filled(p, 3.5, LOW_COLOUR);
            }
            MarkKind::Real => {
                painter.circle_filled(p, 3.4, Color32::WHITE);
            }
            MarkKind::End => {
                painter.circle_stroke(p, 7.0, Stroke::new(2.0, Color32::from_rgb(255, 90, 90)));
            }
        }
    }

    // The current step, emphasised.
    let here = to(pane.position());
    match current {
        Some(TraceEvent::Segment {
            d_collision,
            d_boundary,
            ..
        }) => {
            let u = pane.direction();
            let p0 = pane.position();
            let along = |d: f64| to([p0[0] + u.u * d, p0[1] + u.v * d]);
            let solid = d_collision.min(d_boundary);
            painter.line_segment(
                [here, along(solid)],
                Stroke::new(3.0, Color32::from_rgb(255, 230, 90)),
            );
            if d_collision > d_boundary && d_collision.is_finite() {
                // Where the sampled collision would have been: abandoned at the surface.
                let far = d_collision.min(d_boundary + 20.0);
                painter.add(egui::Shape::dashed_line(
                    &[along(solid), along(far)],
                    Stroke::new(1.2, SURFACE_COLOUR.gamma_multiply(0.7)),
                    6.0,
                    5.0,
                ));
                painter.circle_stroke(
                    along(far),
                    4.0,
                    Stroke::new(1.2, SURFACE_COLOUR.gamma_multiply(0.7)),
                );
            }
        }
        Some(TraceEvent::Tentative { site, real, .. }) => {
            let c = if site.violates_majorant() {
                LOW_COLOUR
            } else if real {
                Color32::WHITE
            } else {
                Color32::from_rgb(160, 165, 175)
            };
            painter.circle_stroke(
                to([site.position.x, site.position.y]),
                9.0,
                Stroke::new(2.0, c),
            );
        }
        Some(TraceEvent::Collision { r, .. }) => {
            painter.circle_stroke(to([r.x, r.y]), 10.0, Stroke::new(2.5, Color32::WHITE));
        }
        _ => {}
    }
    painter.circle_filled(here, 4.5, accent);
    painter.circle_stroke(here, 6.5, Stroke::new(1.0, Color32::BLACK));
    scale_bar(&painter, geo, view);

    // Header: title and counters.
    let c = pane.counts();
    let title = match which {
        Which::Surface => "Surface tracking".to_string(),
        Which::Delta if factor < 1.0 => {
            format!("Delta tracking · majorant × {factor:.2} (too low)")
        }
        Which::Delta => "Delta (Woodcock) tracking".to_string(),
    };
    let counters = match which {
        Which::Surface => format!(
            "locates {} · surface queries {} · crossings {} · collisions {}",
            c.locates, c.boundary_queries, c.crossings, c.collisions
        ),
        Which::Delta => format!(
            "flights {} · virtual {} · real {}{}",
            c.flights,
            c.virtual_collisions,
            c.tentative - c.virtual_collisions,
            if c.majorant_violations > 0 {
                format!(" · ⚠ Σt>Σmaj {}", c.majorant_violations)
            } else {
                String::new()
            }
        ),
    };
    full_painter.text(
        rect.left_top() + Vec2::new(8.0, 4.0),
        egui::Align2::LEFT_TOP,
        title,
        egui::FontId::proportional(14.0),
        accent,
    );
    full_painter.text(
        rect.left_top() + Vec2::new(8.0, 22.0),
        egui::Align2::LEFT_TOP,
        counters,
        egui::FontId::proportional(12.0),
        TEXT,
    );

    // Footer: what the current step did.
    let foot = Rect::from_min_max(
        Pos2::new(rect.left() + 8.0, geo.bottom() + 4.0),
        Pos2::new(rect.right() - 8.0, rect.bottom() - 2.0),
    );
    let text = match (cur, current) {
        (0, _) => format!("{} steps to show. Press Step.", pane.len()),
        (_, Some(e)) => format!("{}/{}: {}", cur, pane.len(), describe(&e)),
        _ => String::new(),
    };
    let mut w = foot.width();
    if let (Which::Delta, Some(TraceEvent::Tentative { site, xi, real })) = (which, current) {
        // The accept test as a bar: the filled part is Σt/Σmaj, the tick is ξ.
        let bar = Rect::from_min_size(
            Pos2::new(foot.right() - 92.0, foot.top() + 4.0),
            Vec2::new(90.0, 12.0),
        );
        let ratio = (site.sigma_t / site.majorant).clamp(0.0, 1.0) as f32;
        full_painter.rect_filled(bar, 3.0, Color32::from_rgb(45, 50, 62));
        let mut fill = bar;
        fill.set_width(bar.width() * ratio);
        full_painter.rect_filled(
            fill,
            3.0,
            if site.violates_majorant() {
                LOW_COLOUR
            } else {
                Color32::from_rgb(90, 170, 110)
            },
        );
        if let Some(x) = xi {
            let tx = bar.left() + bar.width() * x as f32;
            full_painter.line_segment(
                [
                    Pos2::new(tx, bar.top() - 3.0),
                    Pos2::new(tx, bar.bottom() + 3.0),
                ],
                Stroke::new(2.0, Color32::WHITE),
            );
        }
        full_painter.text(
            bar.center_bottom() + Vec2::new(0.0, 2.0),
            egui::Align2::CENTER_TOP,
            if real {
                "ξ < Σt/Σmaj: real"
            } else {
                "ξ ≥ Σt/Σmaj: virtual"
            },
            egui::FontId::proportional(10.0),
            DIM,
        );
        w -= 100.0;
    }
    let g = full_painter.layout(text, egui::FontId::proportional(12.0), TEXT, w.max(60.0));
    full_painter.galley(foot.left_top(), g, TEXT);
    if pane.trace.dropped > 0 && pane.finished() {
        full_painter.text(
            foot.left_bottom(),
            egui::Align2::LEFT_BOTTOM,
            format!(
                "(the last {} steps are counted, not drawn)",
                pane.trace.dropped
            ),
            egui::FontId::proportional(10.0),
            DIM,
        );
    }
}

/// The cell the last `Located` before the cursor placed the neutron in.
fn located_cell(pane: &PaneTrace) -> Option<usize> {
    pane.trace.events[..pane.cursor].iter().rev().find_map(|e| {
        if let TraceEvent::Located { cell, .. } = e {
            Some(*cell)
        } else {
            None
        }
    })
}

/// One CSG surface, drawn in the x-y plane (cylinders as circles, planes as
/// lines; z-planes are edge-on and not drawn).
fn draw_surface(
    painter: &egui::Painter,
    rect: Rect,
    view: &View,
    g: &Geometry,
    s: usize,
    stroke: Stroke,
) {
    let h = model::half_pitch();
    match g.surfaces.get(s) {
        Some(SurfaceKind::ZCylinder(c)) => {
            painter.circle_stroke(
                view.to_screen(rect, c.x0, c.y0),
                (c.r * view.scale) as f32,
                stroke,
            );
        }
        Some(SurfaceKind::XPlane(p)) => {
            painter.line_segment(
                [
                    view.to_screen(rect, p.x0, -h),
                    view.to_screen(rect, p.x0, h),
                ],
                stroke,
            );
        }
        Some(SurfaceKind::YPlane(p)) => {
            painter.line_segment(
                [
                    view.to_screen(rect, -h, p.y0),
                    view.to_screen(rect, h, p.y0),
                ],
                stroke,
            );
        }
        _ => {}
    }
}

/// The TRISO cell, from the same particle centres the geometry is built from.
fn draw_cell(painter: &egui::Painter, rect: Rect, view: &View, centres: &[(f64, f64)]) {
    let p = model::half_pitch();
    let cell = Rect::from_two_pos(view.to_screen(rect, -p, p), view.to_screen(rect, p, -p));
    painter.rect_filled(cell, 0.0, HELIUM);
    let s = view.scale as f32;
    let o = view.to_screen(rect, 0.0, 0.0);
    painter.circle_filled(
        o,
        model::PEBBLE_R as f32 * s,
        MATERIAL_COLOURS[model::MAT_SHELL],
    );
    painter.circle_filled(
        o,
        model::FUEL_ZONE_R as f32 * s,
        MATERIAL_COLOURS[model::MAT_MATRIX],
    );
    let radii = [
        (model::particle_r(), model::MAT_OPYC),
        (model::sic_r(), model::MAT_SIC),
        (model::ipyc_r(), model::MAT_IPYC),
        (model::buffer_r(), model::MAT_BUFFER),
        (model::KERNEL_R, model::MAT_KERNEL),
    ];
    let clip = painter.clip_rect().expand(8.0);
    for &(x, y) in centres {
        let c = view.to_screen(rect, x, y);
        if !clip.contains(c) {
            continue;
        }
        if model::particle_r() as f32 * s < 1.6 {
            painter.circle_filled(c, 1.2, MATERIAL_COLOURS[model::MAT_KERNEL]);
            continue;
        }
        for (r, m) in radii {
            painter.circle_filled(c, r as f32 * s, MATERIAL_COLOURS[m]);
        }
    }
    painter.rect_stroke(
        cell,
        0.0,
        Stroke::new(1.5, Color32::from_rgb(120, 170, 255)),
        StrokeKind::Outside,
    );
}

/// Each method's running k∞ mean (± σ) against generation.
fn draw_k_plot(painter: &egui::Painter, rect: Rect, run: &ManyRun, s: f32) {
    painter.rect_filled(rect, 4.0, Color32::from_rgb(20, 23, 29));
    let pts: Vec<(usize, f64, f64)> = run
        .k_trace
        .iter()
        .flatten()
        .filter_map(|&(i, _, m)| m.map(|(k, e)| (i, k, e)))
        .collect();
    if pts.is_empty() {
        painter.text(
            rect.center(),
            egui::Align2::CENTER_CENTER,
            "k∞ appears with the first active generation",
            egui::FontId::proportional(13.0 * s),
            DIM,
        );
        return;
    }
    let lo = pts.iter().map(|p| p.1 - p.2).fold(f64::INFINITY, f64::min);
    let hi = pts
        .iter()
        .map(|p| p.1 + p.2)
        .fold(f64::NEG_INFINITY, f64::max);
    let pad = ((hi - lo) * 0.1).max(1e-4);
    let (lo, hi) = (lo - pad, hi + pad);
    let n = run.total_generations().max(2) as f64;
    let inner = rect.shrink2(Vec2::new(48.0, 18.0));
    let at = |g: f64, k: f64| {
        Pos2::new(
            inner.left() + inner.width() * (g / (n - 1.0)) as f32,
            inner.bottom() - inner.height() * ((k - lo) / (hi - lo)) as f32,
        )
    };
    for frac in [0.0, 0.5, 1.0] {
        let k = lo + (hi - lo) * frac;
        let p = at(0.0, k);
        painter.text(
            Pos2::new(rect.left() + 4.0, p.y),
            egui::Align2::LEFT_CENTER,
            format!("{k:.3}"),
            egui::FontId::proportional(10.0 * s),
            DIM,
        );
        painter.line_segment(
            [Pos2::new(inner.left(), p.y), Pos2::new(inner.right(), p.y)],
            Stroke::new(0.5, Color32::from_rgb(50, 55, 66)),
        );
    }
    painter.text(
        rect.center_bottom() - Vec2::new(0.0, 2.0),
        egui::Align2::CENTER_BOTTOM,
        "generation (running mean of the active ones, ± 1 σ)",
        egui::FontId::proportional(10.0 * s),
        DIM,
    );
    for (i, &m) in run.methods.iter().enumerate() {
        let c = method_colour(m);
        let series: Vec<(usize, f64, f64)> = run.k_trace[i]
            .iter()
            .filter_map(|&(g, _, mm)| mm.map(|(k, e)| (g, k, e)))
            .collect();
        let line: Vec<Pos2> = series.iter().map(|&(g, k, _)| at(g as f64, k)).collect();
        for &(g, k, e) in &series {
            painter.line_segment(
                [at(g as f64, k - e), at(g as f64, k + e)],
                Stroke::new(1.0, c.gamma_multiply(0.5)),
            );
        }
        if line.len() > 1 {
            painter.add(egui::Shape::line(line, Stroke::new(2.0, c)));
        } else if let Some(p) = line.first() {
            painter.circle_filled(*p, 2.5, c);
        }
    }
}
