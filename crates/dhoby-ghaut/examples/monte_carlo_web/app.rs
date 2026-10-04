//! The egui front end: one app, a rung at a time (the table is `rung_table!`
//! in `main.rs`; the contract each rung meets is in [`crate::rungs`]).
//!
//! Built on the library's web-demo framework ([`dhoby_ghaut::web_demo`]): its
//! [`Panel`], [`View`] and zoom buttons give the mobile-first layout, and its
//! [`Link`] keeps the physics in [`crate::engine`] (a thread natively, a Web
//! Worker in the browser), so this file only draws what the engine reports
//! and the page never freezes.

use crate::anim::{self, animated_speed, draw_track, energy_colour, fmt_energy, fmt_speed, fmt_time, Anim};
use crate::engine::{Event, Request, Tier};
use crate::history::{outcome_name, History, Stats};
use crate::keff::{console_line, summary_lines, Generation, KeffConfig, CONSOLE_HEADER};
use crate::rungs::Mode;
use crate::table::Rung;
use dhoby_ghaut::web_demo::lesson::{self, Rung as _};
use dhoby_ghaut::web_demo::link::Link;
use dhoby_ghaut::web_demo::loading::Loading;
use dhoby_ghaut::web_demo::panel::Panel;
use dhoby_ghaut::web_demo::platform::{autostart, now_s, set_query, set_title};
use dhoby_ghaut::web_demo::view::{apply_zoom, scale_bar, zoom_buttons, View, Zoom};
use egui::{Color32, Pos2, Rect, RichText, Sense, Stroke, StrokeKind, Vec2};
use outram_mc_libs::physics::keff::{GenerationReport, HistoryCounts};
use outram_mc_libs::physics::track_output::TrackEvent;
use std::collections::VecDeque;

type McLink = Link<Request, Event>;

const BG: Color32 = Color32::from_rgb(14, 16, 20);
const K_COLOUR: Color32 = Color32::from_rgb(120, 170, 255);
const MEAN_COLOUR: Color32 = Color32::from_rgb(255, 200, 80);

// ─── Loading ─────────────────────────────────────────────────────────────────

/// A load in progress: the library's job tracker plus what is being loaded.
struct LoadState {
    rung: Rung,
    /// The tier actually processed (after the rung's own choice).
    tier: Tier,
    loading: Loading,
}

impl LoadState {
    fn new(rung: Rung, requested: Tier) -> Self {
        let tier = rung.tier(requested);
        let labels = rung.jobs().iter().map(|j| j.0).collect();
        Self { rung, tier, loading: Loading::new(labels, rung.job_weights(tier)) }
    }
    fn tolerance(&self) -> &'static str {
        match self.tier {
            Tier::Exact => "0.001 (NJOY's own)",
            Tier::Loose => "0.01 (loosened)",
        }
    }
}

// ─── Watch: one neutron at a time ────────────────────────────────────────────

/// Animated neutrons kept in hand while running, so the animation never waits
/// on transport.
const PREFETCH: usize = 3;

struct Tracks {
    queue: VecDeque<History>,
    outstanding: usize,
    current: Option<Anim>,
    past: VecDeque<History>,
    stats: Stats,
    running: bool,
    /// Animate the current neutron to the end while stopped ("Next").
    single: bool,
    /// "Next" was pressed before its neutron had arrived.
    want_single: bool,
    keep: usize,
    dots: bool,
    /// Length of bright trail behind the neutron, cm; `None` draws it all.
    trail_cm: Option<f64>,
}

impl Tracks {
    fn new(link: &McLink, fast: bool, autostart: bool) -> Self {
        link.send(Request::Run { n: PREFETCH, animate: true });
        Self {
            queue: VecDeque::new(),
            outstanding: PREFETCH,
            current: None,
            past: VecDeque::new(),
            stats: Stats::default(),
            running: autostart,
            single: false,
            want_single: false,
            // A fast system's tracks are short: keep a few, whole.
            keep: if fast { 6 } else { 0 },
            dots: true,
            trail_cm: if fast { None } else { Some(80.0) },
        }
    }
    fn retire(&mut self, h: History) {
        self.past.push_back(h);
        while self.past.len() > self.keep {
            self.past.pop_front();
        }
    }
    fn start_next(&mut self) -> bool {
        let Some(h) = self.queue.pop_front() else { return false };
        if let Some(a) = self.current.take() {
            if !a.counted {
                self.stats.add(&a.hist); // skipped past before it finished
            }
            self.retire(a.hist);
        }
        self.current = Some(Anim::new(h));
        true
    }
    fn count_if_finished(&mut self) {
        if let Some(a) = &mut self.current {
            if a.finished() && !a.counted {
                a.counted = true;
                self.stats.add(&a.hist);
            }
        }
    }
    fn top_up(&mut self, link: &McLink) {
        if !(self.running || self.want_single) {
            return;
        }
        let have = self.queue.len() + self.outstanding;
        if have < PREFETCH {
            let n = PREFETCH - have;
            link.send(Request::Run { n, animate: true });
            self.outstanding += n;
        }
    }
    fn receive(&mut self, h: History, animate: bool) {
        if !animate {
            self.stats.add(&h);
            self.retire(h);
            return;
        }
        self.outstanding = self.outstanding.saturating_sub(1);
        self.queue.push_back(h);
        if self.want_single && self.start_next() {
            self.want_single = false;
            self.single = true;
        }
    }
}

// ─── Power iteration: Watch (generations) and Run k_eff ──────────────────────

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum RunState {
    Idle,
    Running,
    Paused,
    Done,
}

/// What a button on the Run view asked for.
enum RunAction {
    None,
    Start,
    Pause,
    Resume,
}

/// One power iteration as the UI follows it, for both the Watch-generations
/// view and the Run k_eff console. The engine runs one generation per
/// request, so pausing is just not asking for the next.
struct Iteration {
    cfg: KeffConfig,
    state: RunState,
    gens: Vec<GenerationReport>,
    sites: Vec<[f32; 2]>,
    lines: Vec<String>,
    /// Counts and fission production summed over the ACTIVE generations.
    active_counts: HistoryCounts,
    active_production: f64,
    /// A step was asked for and its generation has not arrived yet.
    waiting: bool,
    started_at: f64,
    elapsed: f64,
    last_step_at: f64,
    /// Watch only: seconds between generations, so the spreading is visible.
    pace_s: f64,
}

impl Iteration {
    fn new(cfg: KeffConfig, pace_s: f64) -> Self {
        Self {
            cfg,
            state: RunState::Idle,
            gens: Vec::new(),
            sites: Vec::new(),
            lines: Vec::new(),
            active_counts: HistoryCounts::default(),
            active_production: 0.0,
            waiting: false,
            started_at: 0.0,
            elapsed: 0.0,
            last_step_at: 0.0,
            pace_s,
        }
    }
    fn start(&mut self, link: &McLink) {
        let (cfg, pace) = (self.cfg, self.pace_s);
        *self = Self::new(cfg, pace);
        self.state = RunState::Running;
        self.started_at = now_s();
        self.lines.extend(CONSOLE_HEADER.iter().map(|s| s.to_string()));
        link.send(Request::KeffStart(cfg));
        link.send(Request::KeffStep);
        self.waiting = true;
    }
    fn pump(&mut self, link: &McLink) {
        if self.state == RunState::Running {
            self.elapsed = now_s() - self.started_at;
            if !self.waiting && now_s() - self.last_step_at >= self.pace_s {
                link.send(Request::KeffStep);
                self.waiting = true;
            }
        }
    }
    fn receive(&mut self, g: Generation) {
        self.waiting = false;
        self.last_step_at = now_s();
        self.lines.push(console_line(&g.report));
        if g.report.active {
            self.active_counts.add(&g.report.counts);
            self.active_production += g.report.production;
        }
        if self.cfg.want_sites {
            self.sites = g.sites;
        }
        self.gens.push(g.report);
    }
    fn done(&mut self, rung: Rung) {
        if self.state == RunState::Done {
            return;
        }
        self.waiting = false;
        self.state = RunState::Done;
        self.elapsed = now_s() - self.started_at;
        let (m, e) = self.result();
        let lines = summary_lines(&self.active_counts, self.active_production, self.cfg, m, e, rung.reference());
        self.lines.extend(lines);
        self.lines.push(format!(" Wall time in this tab: {:.0} s", self.elapsed));
    }
    fn result(&self) -> (f64, f64) {
        self.gens.last().and_then(|g| g.k_mean).unwrap_or((f64::NAN, f64::NAN))
    }
    fn total_gens(&self) -> usize {
        self.cfg.n_inactive + self.cfg.n_active
    }
}

// ─── The app ─────────────────────────────────────────────────────────────────

/// What the main view shows.
#[allow(clippy::large_enum_variant)] // held by value: the workspace forbids `Box<T>`
enum Screen {
    Tracks(Tracks),
    Generations(Iteration),
    Run(Iteration),
}

#[allow(clippy::large_enum_variant)]
enum Phase {
    Loading(LoadState),
    Ready(Screen),
    Failed(String),
}

/// Watch mode has two views where the rung has generations to show.
#[derive(Clone, Copy, PartialEq, Eq)]
enum WatchView {
    Neutrons,
    Generations,
}

pub struct McApp {
    link: Option<McLink>,
    rung: Rung,
    mode: Mode,
    watch_view: WatchView,
    load_id: u32,
    /// Rung and processed tier of the data the engine holds (once ready).
    loaded: Option<(Rung, Tier)>,
    phase: Phase,
    view: View,
    panel: Panel,
    title: String,
    load_timings: Vec<(&'static str, f64)>,
    load_total_s: f64,
    /// The "speed at 1 eV" slider, cm/s; reset to the rung's default when the
    /// rung changes.
    speed_at_1ev: f64,
    /// The Run k_eff panel's settings (reset to the rung's default with it).
    run_cfg: KeffConfig,
    /// Text size of the Run view (its + / − change it).
    run_text: f32,
    /// `?autostart`, read once at start-up (the URL is rewritten later).
    autostart: bool,
}

impl McApp {
    pub fn new(cc: &eframe::CreationContext<'_>, rung: Rung, mode: Mode) -> Self {
        // The energy colours are chosen against a dark ground; keep the panel
        // dark too rather than follow a light system theme.
        cc.egui_ctx.set_theme(egui::ThemePreference::Dark);
        #[cfg(not(target_arch = "wasm32"))]
        let link = {
            let ctx = cc.egui_ctx.clone();
            Ok::<McLink, String>(dhoby_ghaut::web_demo::link::start_native(crate::engine::McEngine::default(), move || ctx.request_repaint()))
        };
        #[cfg(target_arch = "wasm32")]
        let link = dhoby_ghaut::web_demo::link::start_web::<Request, Event>(cc.egui_ctx.clone(), "./worker.js", Event::Error);
        let mut app = Self {
            link: None,
            rung,
            mode,
            watch_view: WatchView::Neutrons,
            load_id: 0,
            loaded: None,
            phase: Phase::Failed(String::new()),
            view: View::new(rung.half_extent()),
            panel: Panel::default(),
            title: String::new(),
            load_timings: Vec::new(),
            load_total_s: 0.0,
            speed_at_1ev: rung.info().spectrum.default_speed_at_1ev(),
            run_cfg: rung.run_default().unwrap_or(KeffConfig {
                n_particles: 1000,
                n_inactive: 10,
                n_active: 20,
                seed: 1,
                point_source: false,
                want_sites: false,
            }),
            run_text: 13.0,
            autostart: autostart(),
        };
        match link {
            Ok(l) => {
                app.link = Some(l);
                app.switch(rung, mode);
            }
            Err(e) => app.phase = Phase::Failed(format!("could not start the physics worker: {e}")),
        }
        app
    }

    /// The tier a mode asks for: Run k_eff processes at NJOY's tolerance so
    /// its `k` is comparable with the record; Watch may use the loosened
    /// tier. The rung has the last word ([`crate::rungs::McRung::tier`]).
    fn tier_for(mode: Mode) -> Tier {
        match mode {
            Mode::Run => Tier::Exact,
            Mode::Watch => Tier::Loose,
        }
    }

    /// Go to a rung and mode: reload the data if what is loaded will not do,
    /// otherwise just change the screen.
    fn switch(&mut self, rung: Rung, mode: Mode) {
        let mode = if rung.has_run() { mode } else { Mode::Watch };
        if rung != self.rung {
            self.speed_at_1ev = rung.info().spectrum.default_speed_at_1ev();
            self.view = View::new(rung.half_extent());
            self.watch_view = WatchView::Neutrons;
            if let Some(c) = rung.run_default() {
                self.run_cfg = c;
            }
        }
        self.rung = rung;
        self.mode = mode;
        set_query(&[("rung", rung.name()), ("mode", mode.name())]);
        let need = rung.tier(Self::tier_for(mode));
        let ok = match self.loaded {
            // Exact data serve Watch too; loose data do not serve Run.
            Some((r, t)) => r == rung && (t == need || t == Tier::Exact),
            None => false,
        };
        if ok && !matches!(self.phase, Phase::Loading(_)) {
            self.enter_screen();
        } else {
            self.load_id += 1;
            self.loaded = None;
            self.phase = Phase::Loading(LoadState::new(rung, need));
            if let Some(link) = &self.link {
                link.send(Request::Load { id: self.load_id, rung, tier: need });
            }
        }
    }

    /// Build the screen for the current rung, mode and view, on loaded data.
    fn enter_screen(&mut self) {
        let Some(link) = &self.link else { return };
        let fast = self.rung.info().spectrum == anim::Spectrum::Fast;
        self.phase = Phase::Ready(match (self.mode, self.watch_view, self.rung.watch_generations()) {
            (Mode::Run, _, _) => {
                let mut it = Iteration::new(self.run_cfg, 0.0);
                if self.autostart {
                    it.start(link);
                }
                Screen::Run(it)
            }
            (Mode::Watch, WatchView::Generations, Some(cfg)) => {
                let mut it = Iteration::new(cfg, 0.6);
                it.start(link);
                Screen::Generations(it)
            }
            (Mode::Watch, _, _) => Screen::Tracks(Tracks::new(link, fast, self.autostart)),
        });
    }

    fn handle(&mut self, events: Vec<Event>) {
        let rung = self.rung;
        for e in events {
            match (&mut self.phase, e) {
                (_, Event::Error(m)) => self.phase = Phase::Failed(m),
                (Phase::Loading(l), Event::JobStarted { id, index }) if id == self.load_id => l.loading.job_started(index),
                (Phase::Loading(l), Event::JobDone { id, index, secs }) if id == self.load_id => l.loading.job_done(index, secs),
                (Phase::Loading(l), Event::Ready { id }) if id == self.load_id => {
                    self.load_timings = l.loading.timings();
                    self.load_total_s = now_s() - l.loading.started;
                    self.loaded = Some((l.rung, l.tier));
                    self.enter_screen();
                }
                (Phase::Ready(Screen::Tracks(t)), Event::History { h, animate }) => t.receive(h, animate),
                (Phase::Ready(Screen::Generations(it) | Screen::Run(it)), Event::KeffStarted { sites }) => it.sites = sites,
                (Phase::Ready(Screen::Generations(it) | Screen::Run(it)), Event::Generation(g)) => it.receive(g),
                (Phase::Ready(Screen::Generations(it) | Screen::Run(it)), Event::KeffDone) => it.done(rung),
                _ => {}
            }
        }
    }

    fn title_now(&mut self, ctx: &egui::Context) {
        let name = self.rung.info().title;
        let t = match &self.phase {
            Phase::Loading(l) => format!("{name} · loading {}/{} · {}", l.loading.done(), l.loading.labels.len(), l.loading.status_line()),
            Phase::Ready(Screen::Tracks(r)) => format!(
                "{name} · {} neutrons · {} fission · {} capture · {} leak",
                r.stats.histories, r.stats.fissions, r.stats.captures, r.stats.leaks
            ),
            Phase::Ready(Screen::Generations(it) | Screen::Run(it)) => {
                format!("{name} · generation {}/{} · {:?}", it.gens.len(), it.total_gens(), it.state)
            }
            Phase::Failed(e) => format!("{name} · FAILED · {e}"),
        };
        if t != self.title {
            set_title(ctx, &t);
            self.title = t;
        }
    }
}

impl eframe::App for McApp {
    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        let ctx = ui.ctx().clone();
        if let Some(link) = &self.link {
            let events = link.drain();
            self.handle(events);
        }
        if let (Phase::Ready(s), Some(link)) = (&mut self.phase, &self.link) {
            match s {
                Screen::Tracks(t) => t.top_up(link),
                Screen::Generations(it) | Screen::Run(it) => {
                    it.pump(link);
                    if it.state == RunState::Running {
                        ctx.request_repaint_after(std::time::Duration::from_millis(100));
                    }
                }
            }
        }
        if matches!(self.phase, Phase::Loading(_)) {
            // Elapsed time and the progress bar keep moving between events.
            ctx.request_repaint_after(std::time::Duration::from_millis(100));
        }
        self.title_now(&ctx);
        let mut panel = std::mem::take(&mut self.panel);
        panel.show(ui, "Monte Carlo", |ui| self.side_panel(ui));
        self.panel = panel;
        egui::CentralPanel::default().frame(egui::Frame::NONE).show(ui, |ui| self.canvas(ui));
    }
}

// ─── Side panel ──────────────────────────────────────────────────────────────

impl McApp {
    fn side_panel(&mut self, ui: &mut egui::Ui) {
        // Rung and mode.
        let rung = lesson::picker(ui, self.rung);
        let mut mode = self.mode;
        if self.rung.has_run() {
            ui.horizontal_wrapped(|ui| {
                ui.label("Mode:");
                ui.selectable_value(&mut mode, Mode::Watch, "Watch (illustration)");
                ui.selectable_value(&mut mode, Mode::Run, "Run k_eff (true MC)");
            });
        }
        if (rung, mode) != (self.rung, self.mode) {
            self.switch(rung, mode);
        }
        ui.strong(self.rung.info().title);
        lesson::whats_happening(ui, self.rung);
        ui.separator();

        let mut want_view: Option<WatchView> = None;
        let link = self.link.as_ref();
        let rung = self.rung;
        let has_generations = rung.watch_generations().is_some();
        match &mut self.phase {
            Phase::Loading(l) => Self::loading_panel(ui, l),
            Phase::Failed(e) => {
                ui.colored_label(Color32::from_rgb(255, 90, 90), format!("Failed: {e}"));
            }
            Phase::Ready(Screen::Tracks(t)) => {
                if has_generations {
                    want_view = Self::watch_view_picker(ui, WatchView::Neutrons);
                }
                Self::tracks_panel(ui, t, link, rung, &mut self.speed_at_1ev);
            }
            Phase::Ready(Screen::Generations(it)) => {
                want_view = Self::watch_view_picker(ui, WatchView::Generations);
                Self::generations_panel(ui, it, link);
            }
            Phase::Ready(Screen::Run(it)) => Self::run_panel(ui, it, link, &mut self.run_cfg, rung),
        }
        if let Some(v) = want_view {
            self.watch_view = v;
            self.enter_screen();
        }

        ui.separator();
        egui::CollapsingHeader::new("What this is — and is not").default_open(false).show(ui, |ui| {
            for line in self.rung.notes() {
                ui.label(format!("• {line}"));
            }
        });
        if !self.load_timings.is_empty() {
            egui::CollapsingHeader::new(format!("Data processing: {:.0} s", self.load_total_s)).show(ui, |ui| {
                for (l, s) in &self.load_timings {
                    ui.label(format!("{l:<16} {s:6.1} s"));
                }
            });
        }
    }

    fn watch_view_picker(ui: &mut egui::Ui, now: WatchView) -> Option<WatchView> {
        let mut v = now;
        ui.horizontal_wrapped(|ui| {
            ui.label("Watch:");
            ui.selectable_value(&mut v, WatchView::Neutrons, "one neutron at a time");
            ui.selectable_value(&mut v, WatchView::Generations, "whole generations");
        });
        (v != now).then_some(v)
    }

    fn loading_panel(ui: &mut egui::Ui, l: &LoadState) {
        let now = now_s();
        ui.heading("Simulation loading…");
        ui.horizontal(|ui| {
            ui.spinner();
            ui.strong(format!("Processing ENDF files · {}", l.loading.status_line()));
        });
        ui.add(egui::ProgressBar::new(l.loading.fraction(now)).show_percentage().text(format!("rough · {:.0} s", now - l.loading.started)));
        ui.label(format!(
            "Every cross section is reconstructed from evaluated ENDF/B-VIII.0 data by OUTRAM PARK's own NJOY port \
             (RECONR + BROADR, tolerance {}), off the page's main thread, so the page stays live. Covariance data \
             are removed before download: transport never reads them.",
            l.tolerance()
        ));
        if let Some(note) = l.rung.loading_note(l.tier) {
            ui.colored_label(Color32::from_rgb(250, 200, 80), note);
        }
        ui.add_space(6.0);
        l.loading.grid(ui);
    }

    /// The speed slider: one unit everywhere ("speed at 1 eV"), with the
    /// equivalent at the rung's characteristic energy beside it.
    fn speed_slider(ui: &mut egui::Ui, rung: Rung, speed: &mut f64) {
        ui.add(
            egui::Slider::new(speed, 0.0001..=20_000.0)
                .logarithmic(true)
                .text("speed at 1 eV (cm/s)")
                .custom_formatter(|v, _| fmt_speed(v)),
        );
        let (e, name) = rung.info().spectrum.characteristic();
        ui.weak(format!(
            "= {} at {name}. Speed follows the neutron's real speed, v proportional to √E: neutrons slow down as they lose energy.",
            fmt_speed(animated_speed(e, *speed))
        ));
        if ui.button("Default speed").clicked() {
            *speed = rung.info().spectrum.default_speed_at_1ev();
        }
    }

    fn tracks_panel(ui: &mut egui::Ui, r: &mut Tracks, link: Option<&McLink>, rung: Rung, speed: &mut f64) {
        ui.horizontal(|ui| {
            let label = if r.running { "⏸ Stop" } else { "▶ Start" };
            if ui.button(label).clicked() {
                r.running = !r.running;
            }
            if ui.add_enabled(!r.running, egui::Button::new("Next neutron")).clicked() {
                if r.start_next() {
                    r.single = true;
                } else {
                    r.want_single = true;
                }
            }
            if ui.button("Clear").clicked() {
                r.past.clear();
            }
        });
        if ui.add_enabled(!r.running, egui::Button::new("Run 100 unanimated")).clicked() {
            if let Some(link) = link {
                link.send(Request::Run { n: 100, animate: false });
            }
        }
        Self::speed_slider(ui, rung, speed);
        ui.add(egui::Slider::new(&mut r.keep, 0..=40).text("past tracks kept"));
        let mut fade = r.trail_cm.is_some();
        ui.horizontal(|ui| {
            ui.checkbox(&mut fade, "fade the track behind the neutron");
            if fade {
                let mut v = r.trail_cm.unwrap_or(80.0);
                ui.add(egui::DragValue::new(&mut v).range(5.0..=2000.0).suffix(" cm"));
                r.trail_cm = Some(v);
            } else {
                r.trail_cm = None;
            }
        });
        ui.checkbox(&mut r.dots, "mark collisions");
        ui.weak("Zoom with + and − (or scroll / pinch), drag to pan, Reset to recentre.");

        ui.separator();
        ui.strong("This neutron");
        if let Some(a) = &r.current {
            let h = &a.hist;
            let (k, _) = a.head();
            let s = &h.track.states[k.min(h.track.states.len() - 1)];
            let scatters = h.track.states[..=k.min(h.track.states.len() - 1)].iter().filter(|s| s.event == TrackEvent::Scatter).count();
            let born = if h.from_fission { "at the last neutron's fission site" } else { "afresh (the last one did not fission)" };
            egui::Grid::new("now").num_columns(2).show(ui, |ui| {
                ui.label("number");
                ui.label(format!("#{}", h.index + 1));
                ui.end_row();
                ui.label("born");
                ui.label(born);
                ui.end_row();
                ui.label("birth energy");
                ui.label(fmt_energy(h.birth_energy_ev));
                ui.end_row();
                ui.label("energy now");
                ui.colored_label(energy_colour(s.energy, 255), fmt_energy(s.energy));
                ui.end_row();
                ui.label("drawn at");
                ui.label(fmt_speed(animated_speed(a.energy_now(), *speed)));
                ui.end_row();
                ui.label("scatters so far");
                ui.label(scatters.to_string());
                ui.end_row();
                ui.label("distance flown");
                ui.label(format!("{:.1} cm", a.shown_cm.min(a.total())));
                ui.end_row();
                ui.label("real flight time");
                ui.label(fmt_time(s.time));
                ui.end_row();
                if a.finished() {
                    ui.label("fate");
                    ui.strong(match h.outcome {
                        Some(TrackEvent::Fission) => "fission".to_string(),
                        Some(TrackEvent::Absorption) => "captured".to_string(),
                        Some(TrackEvent::Leak) => "leaked out".to_string(),
                        o => outcome_name(o).to_string(),
                    });
                    ui.end_row();
                }
            });
        } else {
            ui.weak("Press Start.");
        }

        ui.separator();
        let st = &r.stats;
        ui.strong(format!("All {} neutrons: how they ended", st.histories));
        if st.histories > 0 {
            Self::counts_grid(ui, &st.counts, st.other);
            egui::Grid::new("tot").num_columns(2).show(ui, |ui| {
                let n = st.histories as f64;
                ui.label("reached thermal (< 0.625 eV)");
                ui.label(format!("{:.0} %", 100.0 * st.thermalised as f64 / n));
                ui.end_row();
                ui.label("mean scatters");
                ui.label(format!("{:.1}", st.scatters as f64 / n));
                ui.end_row();
                ui.label("mean distance flown");
                ui.label(format!("{:.1} cm", st.path_cm / n));
                ui.end_row();
            });
            ui.weak("Counts from a handful of histories, not converged statistics.");
        }
        ui.separator();
        anim::energy_bar(ui);
        anim::legend_markers(ui, true);
        ui.add_space(4.0);
        rung.legend(ui);
    }

    /// Leaked / captured / fissioned, and fissions by incident energy.
    fn counts_grid(ui: &mut egui::Ui, c: &HistoryCounts, other: u64) {
        let t = c.tracked.max(1) as f64;
        let pct = |x: u64| format!("{x}  ({:.1} %)", 100.0 * x as f64 / t);
        egui::Grid::new(ui.next_auto_id()).num_columns(2).show(ui, |ui| {
            ui.label("leaked out");
            ui.label(pct(c.leaked));
            ui.end_row();
            ui.label("captured");
            ui.label(pct(c.captured));
            ui.end_row();
            ui.label("fissioned");
            ui.label(pct(c.fissions()));
            ui.end_row();
            if other > 0 {
                ui.colored_label(Color32::from_rgb(255, 90, 90), "other (a defect)");
                ui.label(other.to_string());
                ui.end_row();
            }
            let [b0, b1, b2] = c.fissions_by_energy;
            ui.label("fissions < 0.625 eV");
            ui.label(b0.to_string());
            ui.end_row();
            ui.label("fissions 0.625 eV – 100 keV");
            ui.label(b1.to_string());
            ui.end_row();
            ui.label("fissions > 100 keV");
            ui.label(b2.to_string());
            ui.end_row();
            if c.tracked > 0 {
                ui.label("non-leakage P_NL = 1 − L/N");
                ui.label(format!("{:.3}", 1.0 - c.leaked as f64 / t));
                ui.end_row();
            }
        });
    }

    fn generations_panel(ui: &mut egui::Ui, it: &mut Iteration, link: Option<&McLink>) {
        ui.label(format!(
            "A real power iteration: {} neutrons per generation, ALL starting at the centre (a deliberately bad \
             guess). Each dot is where a neutron of the next generation starts: a fission site of this one.",
            it.cfg.n_particles
        ));
        ui.horizontal(|ui| {
            match it.state {
                RunState::Running => {
                    if ui.button("⏸ Pause").clicked() {
                        it.state = RunState::Paused;
                    }
                }
                RunState::Paused => {
                    if ui.button("▶ Resume").clicked() {
                        it.state = RunState::Running;
                    }
                }
                _ => {}
            }
            if ui.button("Restart").clicked() {
                if let Some(link) = link {
                    it.start(link);
                }
            }
        });
        ui.add(egui::Slider::new(&mut it.pace_s, 0.0..=3.0).text("s between generations"));
        if let Some(g) = it.gens.last() {
            ui.label(format!(
                "Generation {} of {} ({}): k = {:.4}, entropy {}",
                g.index + 1,
                it.total_gens(),
                if g.active { "active" } else { "inactive" },
                g.k,
                g.entropy.map_or("–".into(), |h| format!("{h:.3} bits"))
            ));
            Self::counts_grid(ui, &g.counts, 0);
        }
        ui.weak(
            "Shannon entropy measures how spread out the fission source is (5 × 5 × 5 mesh on the bounding box). \
             It rises as the source spreads from the centre and flattens once the shape has settled: the \
             inactive generations (shaded) are thrown away for that reason.",
        );
    }

    fn run_panel(ui: &mut egui::Ui, it: &mut Iteration, link: Option<&McLink>, cfg: &mut KeffConfig, rung: Rung) {
        let editable = matches!(it.state, RunState::Idle | RunState::Done);
        ui.add_enabled_ui(editable, |ui| {
            egui::Grid::new("cfg").num_columns(2).show(ui, |ui| {
                ui.label("neutrons / generation");
                ui.add(egui::DragValue::new(&mut cfg.n_particles).range(100..=20_000).speed(50));
                ui.end_row();
                ui.label("inactive generations");
                ui.add(egui::DragValue::new(&mut cfg.n_inactive).range(1..=200));
                ui.end_row();
                ui.label("active generations");
                ui.add(egui::DragValue::new(&mut cfg.n_active).range(2..=1000));
                ui.end_row();
                ui.label("seed");
                ui.add(egui::DragValue::new(&mut cfg.seed).range(1..=1_000_000));
                ui.end_row();
            });
            if ui.button("Defaults").clicked() {
                if let Some(d) = rung.run_default() {
                    *cfg = d;
                }
            }
        });
        ui.horizontal(|ui| match it.state {
            RunState::Idle | RunState::Done => {
                if ui.button("▶ Run").clicked() {
                    if let Some(link) = link {
                        it.cfg = *cfg;
                        it.start(link);
                    }
                }
            }
            RunState::Running => {
                if ui.button("⏸ Pause").clicked() {
                    it.state = RunState::Paused;
                }
            }
            RunState::Paused => {
                if ui.button("▶ Resume").clicked() {
                    it.state = RunState::Running;
                    it.started_at = now_s() - it.elapsed;
                }
                if ui.button("Stop").clicked() {
                    it.done(rung);
                }
            }
        });
        ui.label(format!("{} of {} generations · {:.0} s", it.gens.len(), it.total_gens(), it.elapsed));
        ui.weak(
            "Single-threaded, in this tab's background worker: outram-mc-libs' reference power iteration \
             (`PowerIteration`, what `run_keff` runs), one generation at a time, on ENDF/B-VIII.0 processed at \
             NJOY's tolerance.",
        );
        if it.active_counts.tracked > 0 {
            ui.separator();
            ui.strong("Active generations: how neutrons ended");
            Self::counts_grid(ui, &it.active_counts, 0);
        }
        if it.state == RunState::Done {
            let (m, e) = it.result();
            ui.separator();
            ui.strong("Your result");
            ui.label(RichText::new(format!("k = {m:.5} ± {e:.5}  ({:+.0} ± {:.0} pcm)", (m - 1.0) * 1e5, e * 1e5)).monospace());
            if let Some(r) = rung.reference() {
                if let Some((name, k, s)) = r.experiment {
                    ui.label(format!("Experiment ({name}): {k:.4} ± {s:.4}."));
                }
                ui.label(format!("Recorded: {:.5} ± {:.5} ({:+.0} ± {:.0} pcm), {}.", r.k, r.sem, (r.k - 1.0) * 1e5, r.sem * 1e5, r.label));
                let hist = (it.cfg.n_particles * it.cfg.n_active) as f64;
                let expect = r.sd_one_run * (r.histories_one_run / hist).sqrt();
                ui.weak(format!(
                    "Why your ± is wider: the record pools independent runs, so its ± is the spread of one run \
                     ({:.0} pcm at {:.0} active neutrons) divided by the square root of their number. One run of \
                     yours, {:.0} active neutrons, is expected to scatter by about {:.0} pcm around the true value.",
                    r.sd_one_run * 1e5,
                    r.histories_one_run,
                    hist,
                    expect * 1e5
                ));
            }
        }
    }
}

// ─── Main view ───────────────────────────────────────────────────────────────

/// A plot frame with linear axes.
struct Axes {
    rect: Rect,
    x: (f64, f64),
    y: (f64, f64),
}

impl Axes {
    fn p(&self, x: f64, y: f64) -> Pos2 {
        let fx = ((x - self.x.0) / (self.x.1 - self.x.0)) as f32;
        let fy = ((y - self.y.0) / (self.y.1 - self.y.0)) as f32;
        Pos2::new(self.rect.left() + fx * self.rect.width(), self.rect.bottom() - fy * self.rect.height())
    }
    fn frame(&self, painter: &egui::Painter, title: &str, fmt: impl Fn(f64) -> String) {
        painter.rect_filled(self.rect, 4.0, Color32::from_rgb(20, 23, 30));
        painter.rect_stroke(self.rect, 4.0, Stroke::new(1.0, Color32::from_rgb(60, 66, 80)), StrokeKind::Inside);
        let f = egui::FontId::proportional(11.0);
        let c = Color32::from_rgb(170, 176, 190);
        painter.text(self.rect.left_top() + Vec2::new(6.0, 3.0), egui::Align2::LEFT_TOP, title, egui::FontId::proportional(12.0), Color32::WHITE);
        painter.text(self.rect.right_top() + Vec2::new(-4.0, 3.0), egui::Align2::RIGHT_TOP, fmt(self.y.1), f.clone(), c);
        painter.text(self.rect.right_bottom() + Vec2::new(-4.0, -3.0), egui::Align2::RIGHT_BOTTOM, fmt(self.y.0), f, c);
    }
    /// Shade the inactive generations `[0.5, n_inactive + 0.5]`.
    fn shade_inactive(&self, painter: &egui::Painter, n_inactive: usize) {
        let a = self.p(0.5, self.y.1);
        let b = self.p(n_inactive as f64 + 0.5, self.y.0);
        let r = Rect::from_two_pos(a, b).intersect(self.rect);
        painter.rect_filled(r, 0.0, Color32::from_rgba_unmultiplied(120, 120, 140, 40));
        painter.text(r.center_bottom() + Vec2::new(0.0, -3.0), egui::Align2::CENTER_BOTTOM, "inactive", egui::FontId::proportional(10.0), Color32::from_rgb(150, 150, 170));
    }
}

/// Entropy per generation, with the inactive generations shaded.
fn draw_entropy(painter: &egui::Painter, rect: Rect, it: &Iteration) {
    let n = it.total_gens().max(2) as f64;
    // From just below the lowest value to the 5 x 5 x 5 mesh's ceiling,
    // log2(125) bits, so the rise is visible.
    let ceiling = (125f64).log2();
    let lowest = it.gens.iter().filter_map(|g| g.entropy).fold(ceiling, f64::min);
    let ax = Axes { rect, x: (0.5, n + 0.5), y: ((lowest - 0.5).floor().max(0.0), ceiling) };
    ax.frame(painter, "Shannon entropy of the fission source (bits)", |v| format!("{v:.1}"));
    ax.shade_inactive(painter, it.cfg.n_inactive);
    let pts: Vec<Pos2> = it.gens.iter().filter_map(|g| g.entropy.map(|h| ax.p(g.index as f64 + 1.0, h))).collect();
    if pts.len() > 1 {
        painter.add(egui::Shape::line(pts.clone(), Stroke::new(1.5, Color32::from_rgb(120, 220, 160))));
    }
    for p in pts {
        painter.circle_filled(p, 2.0, Color32::from_rgb(120, 220, 160));
    }
}

/// `k` per generation (dots) and the running mean ± σ over the active ones,
/// with the experiment's band when there is one.
fn draw_k(painter: &egui::Painter, rect: Rect, it: &Iteration, experiment: Option<(f64, f64)>) {
    let n = it.total_gens().max(2) as f64;
    let mut lo = 0.98f64;
    let mut hi = 1.02f64;
    // The first generations come from the guessed source (generation 1
    // leaks far too much on a bare sphere, gh:#527) and would squash the
    // rest: they are drawn clamped to the frame instead of setting its range.
    for g in it.gens.iter().filter(|g| g.index >= 2) {
        lo = lo.min(g.k);
        hi = hi.max(g.k);
    }
    let pad = 0.1 * (hi - lo);
    let ax = Axes { rect, x: (0.5, n + 0.5), y: (lo - pad, hi + pad) };
    ax.frame(painter, "k per generation (dots) · running mean ± σ (band)", |v| format!("{v:.3}"));
    ax.shade_inactive(painter, it.cfg.n_inactive);
    if let Some((k, s)) = experiment {
        let band = Rect::from_two_pos(ax.p(0.5, k + s), ax.p(n + 0.5, k - s)).intersect(rect);
        painter.rect_filled(band, 0.0, Color32::from_rgba_unmultiplied(255, 255, 255, 18));
        painter.line_segment([ax.p(0.5, k), ax.p(n + 0.5, k)], Stroke::new(1.0, Color32::from_rgb(200, 200, 200)));
        painter.text(ax.p(0.5, k) + Vec2::new(4.0, -2.0), egui::Align2::LEFT_BOTTOM, format!("experiment {k:.4} ± {s:.4}"), egui::FontId::proportional(10.0), Color32::from_rgb(200, 200, 200));
    }
    let means: Vec<(f64, f64, f64)> =
        it.gens.iter().filter_map(|g| g.k_mean.filter(|m| m.1 > 0.0).map(|(m, e)| (g.index as f64 + 1.0, m, e))).collect();
    if means.len() > 1 {
        let upper: Vec<Pos2> = means.iter().map(|&(x, m, e)| ax.p(x, m + e)).collect();
        let lower: Vec<Pos2> = means.iter().map(|&(x, m, e)| ax.p(x, m - e)).collect();
        for w in 0..means.len() - 1 {
            let quad = vec![upper[w], upper[w + 1], lower[w + 1], lower[w]];
            painter.add(egui::Shape::convex_polygon(quad, Color32::from_rgba_unmultiplied(255, 200, 80, 50), Stroke::NONE));
        }
        let line: Vec<Pos2> = means.iter().map(|&(x, m, _)| ax.p(x, m)).collect();
        painter.add(egui::Shape::line(line, Stroke::new(2.0, MEAN_COLOUR)));
    }
    for g in &it.gens {
        let c = if g.active { K_COLOUR } else { Color32::from_rgb(110, 115, 130) };
        painter.circle_filled(ax.p(g.index as f64 + 1.0, g.k.clamp(ax.y.0, ax.y.1)), 2.2, c);
    }
}

impl McApp {
    fn canvas(&mut self, ui: &mut egui::Ui) {
        let (resp, painter) = ui.allocate_painter(ui.available_size(), Sense::click_and_drag());
        let rect = resp.rect;
        painter.rect_filled(rect, 0.0, BG);
        let is_run = matches!(self.phase, Phase::Ready(Screen::Run(_)));
        if !is_run {
            self.view.handle_input(ui, &resp);
            self.rung.draw(&painter, rect, &self.view);
        }

        let view = self.view;
        let to_screen = |x: f64, y: f64| view.to_screen(rect, x, y);
        let speed = self.speed_at_1ev;
        let rung = self.rung;
        match &mut self.phase {
            Phase::Loading(l) => {
                let title = "Simulation loading…";
                l.loading.card(&painter, rect, title, rung.loading_note(l.tier));
            }
            Phase::Failed(e) => {
                painter.text(rect.center(), egui::Align2::CENTER_CENTER, format!("Failed: {e}"), egui::FontId::proportional(15.0), Color32::from_rgb(255, 110, 110));
            }
            Phase::Ready(Screen::Tracks(r)) => {
                let dt = ui.input(|i| i.stable_dt).min(0.1) as f64;
                if r.running || r.single {
                    match &mut r.current {
                        Some(a) if !a.finished() => a.advance(dt, speed),
                        Some(_) if r.single => r.single = false,
                        _ if r.running => {
                            r.start_next();
                        }
                        _ => {}
                    }
                    ui.ctx().request_repaint();
                }
                r.count_if_finished();
                let n = r.past.len();
                for (i, h) in r.past.iter().enumerate() {
                    let alpha = (25.0 + 45.0 * (i + 1) as f32 / n.max(1) as f32) as u8;
                    draw_track(&painter, to_screen, h, None, alpha, false, None);
                }
                if let Some(a) = &r.current {
                    let upto = if a.finished() { None } else { Some(a.head()) };
                    let trail = r.trail_cm.map(|len| (a.cum.as_slice(), a.shown_cm.min(a.total()), len));
                    draw_track(&painter, to_screen, &a.hist, upto, 255, r.dots, trail);
                }
                painter.text(rect.left_bottom() + Vec2::new(16.0, -40.0), egui::Align2::LEFT_BOTTOM, "illustration: real histories, chained by hand", egui::FontId::proportional(12.0), Color32::from_rgb(170, 176, 190));
            }
            Phase::Ready(Screen::Generations(it)) => {
                for s in &it.sites {
                    painter.circle_filled(to_screen(s[0] as f64, s[1] as f64), 1.6, Color32::from_rgba_unmultiplied(255, 220, 90, 170));
                }
                let h = (rect.height() * 0.28).clamp(90.0, 180.0);
                let plot = Rect::from_min_max(Pos2::new(rect.left() + 12.0, rect.bottom() - h - 30.0), Pos2::new(rect.right() - 12.0, rect.bottom() - 30.0));
                draw_entropy(&painter, plot, it);
                let label = match it.gens.last() {
                    Some(g) => format!("generation {} of {} · {} source neutrons shown (of {})", g.index + 1, it.total_gens(), it.sites.len(), it.cfg.n_particles),
                    None => format!("generation 0 · all {} neutrons start at the centre", it.cfg.n_particles),
                };
                painter.text(Pos2::new(rect.left() + 12.0, plot.top() - 6.0), egui::Align2::LEFT_BOTTOM, label, egui::FontId::proportional(12.0), Color32::WHITE);
            }
            Phase::Ready(Screen::Run(it)) => {
                let experiment = rung.reference().and_then(|r| r.experiment).map(|(_, k, s)| (k, s));
                match Self::run_view(ui, rect, &painter, it, self.run_text, experiment) {
                    RunAction::Start => {
                        if let Some(link) = &self.link {
                            it.cfg = self.run_cfg;
                            it.start(link);
                        }
                    }
                    RunAction::Pause => it.state = RunState::Paused,
                    RunAction::Resume => {
                        it.state = RunState::Running;
                        it.started_at = now_s() - it.elapsed;
                    }
                    RunAction::None => {}
                }
            }
        }

        if !is_run {
            scale_bar(&painter, rect, &self.view);
        }
        self.panel.reopen_button(ui, rect);
        // + / − / Reset. In Run mode there is no geometry: they scale the
        // console and plots' text.
        if let Some(z) = zoom_buttons(ui, rect) {
            match (is_run, z) {
                (false, z) => apply_zoom(&mut self.view, rect, z),
                (true, Zoom::In) => self.run_text = (self.run_text * 1.2).min(28.0),
                (true, Zoom::Out) => self.run_text = (self.run_text / 1.2).max(8.0),
                (true, Zoom::Reset) => self.run_text = 13.0,
            }
        }
    }

    /// Run k_eff's main view: the two plots, then the console, scrolling,
    /// with Run / Pause / Resume on it (the panel is folded on a phone).
    fn run_view(ui: &mut egui::Ui, rect: Rect, painter: &egui::Painter, it: &Iteration, text: f32, experiment: Option<(f64, f64)>) -> RunAction {
        let top = rect.top() + 52.0;
        let narrow = rect.width() < 700.0;
        let plot_h = ((rect.height() - 60.0) * 0.36).clamp(110.0, 260.0);
        let plots = Rect::from_min_max(Pos2::new(rect.left() + 10.0, top), Pos2::new(rect.right() - 10.0, top + plot_h));
        if narrow {
            draw_k(painter, plots, it, experiment);
        } else {
            let (a, b) = plots.split_left_right_at_fraction(0.62);
            draw_k(painter, a, it, experiment);
            draw_entropy(painter, b.translate(Vec2::new(8.0, 0.0)).with_max_x(plots.right()), it);
        }
        // One action button, between the plots and the console.
        let mut action = RunAction::None;
        let brow = Rect::from_min_size(Pos2::new(rect.left() + 10.0, plots.bottom() + 8.0), Vec2::new(rect.width() - 20.0, 36.0));
        let (label, act) = match it.state {
            RunState::Idle => (format!("▶ Run k_eff  ({} × [{} + {}])", it.cfg.n_particles, it.cfg.n_inactive, it.cfg.n_active), RunAction::Start),
            RunState::Done => ("▶ Run again (settings in Controls »)".to_string(), RunAction::Start),
            RunState::Running => (format!("⏸ Pause · generation {} of {} · {:.0} s", it.gens.len(), it.total_gens(), it.elapsed), RunAction::Pause),
            RunState::Paused => ("▶ Resume".to_string(), RunAction::Resume),
        };
        let bw = brow.width().min(360.0);
        if ui.put(Rect::from_min_size(brow.min, Vec2::new(bw, 36.0)), egui::Button::new(RichText::new(label).size(15.0))).clicked() {
            action = act;
        }
        let console = Rect::from_min_max(Pos2::new(rect.left() + 10.0, brow.bottom() + 8.0), Pos2::new(rect.right() - 10.0, rect.bottom() - 10.0));
        painter.rect_filled(console, 4.0, Color32::from_rgb(8, 9, 12));
        if it.lines.is_empty() {
            painter.text(console.center(), egui::Align2::CENTER_CENTER, "openmc.run()-style output appears here", egui::FontId::proportional(text), Color32::from_rgb(140, 146, 160));
            return action;
        }
        ui.scope_builder(egui::UiBuilder::new().max_rect(console.shrink(6.0)), |ui| {
            egui::ScrollArea::both().stick_to_bottom(true).auto_shrink([false, false]).show(ui, |ui| {
                for l in &it.lines {
                    ui.add(egui::Label::new(RichText::new(l).monospace().size(text).color(Color32::from_rgb(210, 216, 226))).extend());
                }
                if it.state == RunState::Running {
                    ui.label(RichText::new(" …").monospace().size(text));
                }
            });
        });
        action
    }
}
