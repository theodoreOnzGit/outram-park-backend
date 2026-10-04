//! The egui front end: one app, a rung at a time (see [`crate::rungs`]).
//!
//! The UI never does physics: [`crate::engine`] runs the ENDF processing, the
//! neutrons and the power iteration on a thread (natively) or in a Web Worker
//! (in the browser), and this file only draws what it reports. So the page
//! keeps animating — progress bar, elapsed time, the picture — while a
//! nuclide is processed or a generation runs.
//!
//! Layout follows the workspace's mobile-first rule
//! (`docs/claude-md/mobile-first-tutorials-and-demos.md`): the main view fills
//! the page, with + / − / Reset always on it, and every control is in a side
//! panel that opens folded on a narrow screen.

use crate::anim::{self, animated_speed, draw_track, energy_colour, fmt_energy, fmt_speed, fmt_time, Anim};
use crate::engine::{self, Event, Link, Request, Tier};
use crate::godiva::sim::{self as gsim, Generation, KeffConfig};
use crate::history::{outcome_name, History, Stats};
use crate::rungs::{Mode, Rung, RUNGS};
use crate::triso::model as tmodel;
use egui::{Color32, Pos2, Rect, RichText, Sense, Stroke, StrokeKind, Vec2};
use outram_mc_libs::physics::keff::{GenerationReport, HistoryCounts};
use outram_mc_libs::physics::track_output::TrackEvent;
use std::collections::VecDeque;

// ─── Platform shims (no std::time on wasm32 — it compiles, then panics) ──────

fn now_s() -> f64 {
    #[cfg(target_arch = "wasm32")]
    {
        js_sys::Date::now() / 1000.0
    }
    #[cfg(not(target_arch = "wasm32"))]
    {
        static T0: std::sync::OnceLock<std::time::Instant> = std::sync::OnceLock::new();
        T0.get_or_init(std::time::Instant::now).elapsed().as_secs_f64()
    }
}

fn set_title(ctx: &egui::Context, title: &str) {
    #[cfg(target_arch = "wasm32")]
    {
        let _ = ctx;
        if let Some(d) = web_sys::window().and_then(|w| w.document()) {
            d.set_title(title);
        }
    }
    #[cfg(not(target_arch = "wasm32"))]
    ctx.send_viewport_cmd(egui::ViewportCommand::Title(title.to_owned()));
}

/// Keep the page URL in step with the rung and mode, so it can be shared.
fn set_url(rung: Rung, mode: Mode) {
    #[cfg(target_arch = "wasm32")]
    if let Some(h) = web_sys::window().and_then(|w| w.history().ok()) {
        let q = format!("?rung={}&mode={}", rung.info().name, mode.name());
        let _ = h.replace_state_with_url(&wasm_bindgen::JsValue::NULL, "", Some(&q));
    }
    #[cfg(not(target_arch = "wasm32"))]
    let _ = (rung, mode);
}

/// `?autostart` in the page URL (or `MC_AUTOSTART` natively) starts the
/// animation, or the k_eff run, as soon as the data are ready — for
/// unattended browser tests.
fn autostart() -> bool {
    #[cfg(target_arch = "wasm32")]
    {
        web_sys::window().and_then(|w| w.location().search().ok()).is_some_and(|s| s.contains("autostart"))
    }
    #[cfg(not(target_arch = "wasm32"))]
    {
        std::env::var_os("MC_AUTOSTART").is_some()
    }
}

// ─── Colours ─────────────────────────────────────────────────────────────────

const BG: Color32 = Color32::from_rgb(14, 16, 20);
const HELIUM: Color32 = Color32::from_rgb(24, 28, 36);
const URANIUM: Color32 = Color32::from_rgb(70, 64, 58);
const MATERIAL_COLOURS: [Color32; 7] = [
    Color32::from_rgb(214, 120, 46),  // UO2 kernel
    Color32::from_rgb(58, 58, 62),    // buffer
    Color32::from_rgb(150, 150, 154), // inner PyC
    Color32::from_rgb(200, 176, 112), // SiC
    Color32::from_rgb(150, 150, 154), // outer PyC
    Color32::from_rgb(92, 94, 98),    // matrix graphite
    Color32::from_rgb(74, 76, 80),    // shell graphite
];
const K_COLOUR: Color32 = Color32::from_rgb(120, 170, 255);
const MEAN_COLOUR: Color32 = Color32::from_rgb(255, 200, 80);

// ─── View transform ──────────────────────────────────────────────────────────

#[derive(Clone, Copy)]
struct View {
    /// World point (cm) at the centre of the canvas.
    centre: [f64; 2],
    /// Pixels per cm.
    scale: f64,
    /// Half-width of the subject, cm: what Reset fits to the screen.
    half_extent: f64,
    /// Fitted to the canvas yet?
    fitted: bool,
}

impl View {
    fn new(half_extent: f64) -> Self {
        Self { centre: [0.0, 0.0], scale: 1.0, half_extent, fitted: false }
    }
    fn fit_scale(&self, rect: Rect) -> f64 {
        (rect.width().min(rect.height()) as f64) * 0.94 / (2.0 * self.half_extent)
    }
    fn fit(&mut self, rect: Rect) {
        self.centre = [0.0, 0.0];
        self.scale = self.fit_scale(rect);
        self.fitted = true;
    }
    fn to_screen(&self, rect: Rect, x: f64, y: f64) -> Pos2 {
        let c = rect.center();
        Pos2::new(
            c.x + ((x - self.centre[0]) * self.scale) as f32,
            c.y - ((y - self.centre[1]) * self.scale) as f32,
        )
    }
    /// Multiply the zoom by `factor`, keeping the world point under `p` fixed.
    fn zoom_about(&mut self, rect: Rect, p: Pos2, factor: f64) {
        let before = self.to_world(rect, p);
        let fit = self.fit_scale(rect);
        self.scale = (self.scale * factor).clamp(fit * 0.5, fit * 600.0);
        let after = self.to_world(rect, p);
        self.centre[0] += before[0] - after[0];
        self.centre[1] += before[1] - after[1];
    }
    fn to_world(&self, rect: Rect, p: Pos2) -> [f64; 2] {
        let c = rect.center();
        [
            self.centre[0] + (p.x - c.x) as f64 / self.scale,
            self.centre[1] - (p.y - c.y) as f64 / self.scale,
        ]
    }
}

fn half_extent(rung: Rung) -> f64 {
    match rung {
        Rung::Triso => tmodel::half_pitch(),
        Rung::Godiva => crate::godiva::model::RADIUS_CM,
    }
}

/// Draw the TRISO cell from the same particle centres the geometry was built
/// from. (The geometry REVIEW images required by the crate's drawing rule are
/// rendered separately, from the assembled geometry, by `--render-geometry`.)
fn draw_triso(painter: &egui::Painter, rect: Rect, view: &View, centres: &[(f64, f64)]) {
    let p = tmodel::half_pitch();
    let (a, b) = (view.to_screen(rect, -p, p), view.to_screen(rect, p, -p));
    let cell = Rect::from_two_pos(a, b);
    painter.rect_filled(cell, 0.0, HELIUM);
    let s = view.scale as f32;
    let o = view.to_screen(rect, 0.0, 0.0);
    painter.circle_filled(o, tmodel::PEBBLE_R as f32 * s, MATERIAL_COLOURS[tmodel::MAT_SHELL]);
    painter.circle_filled(o, tmodel::FUEL_ZONE_R as f32 * s, MATERIAL_COLOURS[tmodel::MAT_MATRIX]);
    let radii = [
        (tmodel::particle_r(), tmodel::MAT_OPYC),
        (tmodel::sic_r(), tmodel::MAT_SIC),
        (tmodel::ipyc_r(), tmodel::MAT_IPYC),
        (tmodel::buffer_r(), tmodel::MAT_BUFFER),
        (tmodel::KERNEL_R, tmodel::MAT_KERNEL),
    ];
    let clip = painter.clip_rect().expand(8.0);
    for &(x, y) in centres {
        let c = view.to_screen(rect, x, y);
        if !clip.contains(c) {
            continue;
        }
        if tmodel::particle_r() as f32 * s < 1.6 {
            painter.circle_filled(c, 1.2, MATERIAL_COLOURS[tmodel::MAT_KERNEL]);
            continue;
        }
        for (r, m) in radii {
            painter.circle_filled(c, r as f32 * s, MATERIAL_COLOURS[m]);
        }
    }
    painter.rect_stroke(cell, 0.0, Stroke::new(1.5, Color32::from_rgb(120, 170, 255)), StrokeKind::Outside);
    painter.text(
        cell.left_top() + Vec2::new(6.0, 4.0),
        egui::Align2::LEFT_TOP,
        "reflective boundary  ·  helium (void)",
        egui::FontId::proportional(12.0),
        Color32::from_rgb(140, 170, 220),
    );
}

/// Draw Godiva: a uranium disc (the sphere seen from above), vacuum outside.
fn draw_godiva(painter: &egui::Painter, rect: Rect, view: &View) {
    let r = crate::godiva::model::RADIUS_CM;
    let o = view.to_screen(rect, 0.0, 0.0);
    painter.circle_filled(o, (r * view.scale) as f32, URANIUM);
    painter.circle_stroke(o, (r * view.scale) as f32, Stroke::new(1.5, Color32::from_rgb(170, 120, 255)));
    painter.text(
        o + Vec2::new(0.0, -(r * view.scale) as f32 - 6.0),
        egui::Align2::CENTER_BOTTOM,
        format!("HEU metal sphere, r = {r} cm · vacuum outside (leaks)"),
        egui::FontId::proportional(12.0),
        Color32::from_rgb(190, 170, 230),
    );
}

// ─── Loading ─────────────────────────────────────────────────────────────────

/// Rough cost of each job in seconds, used ONLY to weight the progress bar:
/// jobs differ by two orders of magnitude, so counting them would make the
/// bar jump. TRISO: headless Chromium, 2026-10-03. Godiva: native,
/// 2026-10-04, i9-13900K, one thread, shared machine (U-234 / U-235 / U-238:
/// 1.6 / 29.5 / 31.1 s at tolerance 0.001, 1.0 / 11.2 / 11.6 s at 0.01).
fn job_weights(rung: Rung, tier: Tier) -> Vec<f64> {
    match (rung, engine::effective_tier(rung, tier)) {
        (Rung::Triso, _) => vec![35.1, 28.5, 0.2, 0.1, 0.2, 0.1, 0.1, 0.3, 0.3, 0.3, 21.5],
        (Rung::Godiva, Tier::Exact) => vec![1.6, 29.5, 31.1],
        (Rung::Godiva, Tier::Loose) => vec![1.0, 11.2, 11.6],
    }
}

struct Loading {
    rung: Rung,
    tier: Tier,
    jobs: Vec<(&'static str, &'static str)>,
    weights: Vec<f64>,
    started: f64,
    job_started: Vec<Option<f64>>,
    job_secs: Vec<Option<f64>>,
}

impl Loading {
    fn new(rung: Rung, tier: Tier) -> Self {
        let jobs = engine::jobs(rung);
        let n = jobs.len();
        Self {
            rung,
            tier,
            jobs,
            weights: job_weights(rung, tier),
            started: now_s(),
            job_started: vec![None; n],
            job_secs: vec![None; n],
        }
    }
    fn done(&self) -> usize {
        self.job_secs.iter().filter(|s| s.is_some()).count()
    }
    fn current(&self) -> Option<usize> {
        (0..self.jobs.len()).find(|&i| self.job_started[i].is_some() && self.job_secs[i].is_none())
    }
    /// A rough fraction: finished jobs by weight, plus the running job's
    /// elapsed share of its expected time, capped short of done.
    fn fraction(&self, now: f64) -> f32 {
        let total: f64 = self.weights.iter().sum();
        let mut f: f64 = (0..self.jobs.len()).filter(|&i| self.job_secs[i].is_some()).map(|i| self.weights[i]).sum();
        if let Some(i) = self.current() {
            let t = now - self.job_started[i].unwrap_or(now);
            f += self.weights[i] * (t / self.weights[i].max(0.05)).min(0.95);
        }
        (f / total).clamp(0.0, 1.0) as f32
    }
    fn status_line(&self) -> String {
        match self.current() {
            Some(i) => format!("{} ({} of {})", self.jobs[i].0, i + 1, self.jobs.len()),
            None if self.done() == 0 => "Downloading ENDF tapes".into(),
            None => "Assembling the model".into(),
        }
    }
    fn tolerance(&self) -> &'static str {
        match engine::effective_tier(self.rung, self.tier) {
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
    fn new(link: &Link, rung: Rung, autostart: bool) -> Self {
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
            keep: if rung == Rung::Godiva { 6 } else { 0 },
            dots: true,
            trail_cm: if rung == Rung::Godiva { None } else { Some(80.0) },
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
    fn top_up(&mut self, link: &Link) {
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

// ─── Godiva power iteration: Watch (generations) and Run k_eff ───────────────

/// What a button on the Run view asked for.
enum RunAction {
    None,
    Start,
    Pause,
    Resume,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum RunState {
    Idle,
    Running,
    Paused,
    Done,
}

/// One power iteration as the UI follows it, for both the Watch-generations
/// view and the Run k_eff console.
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
    fn start(&mut self, link: &Link) {
        let (cfg, pace) = (self.cfg, self.pace_s);
        *self = Self::new(cfg, pace);
        self.state = RunState::Running;
        self.started_at = now_s();
        self.lines.extend(gsim::CONSOLE_HEADER.iter().map(|s| s.to_string()));
        link.send(Request::KeffStart(cfg));
        link.send(Request::KeffStep);
        self.waiting = true;
    }
    fn pump(&mut self, link: &Link) {
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
        self.lines.push(gsim::console_line(&g.report));
        if g.report.active {
            self.active_counts.add(&g.report.counts);
            self.active_production += g.report.production;
        }
        if self.cfg.want_sites {
            self.sites = g.sites;
        }
        self.gens.push(g.report);
    }
    fn done(&mut self) {
        if self.state == RunState::Done {
            return;
        }
        self.waiting = false;
        self.state = RunState::Done;
        self.elapsed = now_s() - self.started_at;
        let (m, e) = self.result();
        let lines = gsim::summary_lines(&self.active_counts, self.active_production, self.cfg, m, e);
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
    Loading(Loading),
    Ready(Screen),
    Failed(String),
}

/// Watch mode for Godiva has two views; TRISO only has the first.
#[derive(Clone, Copy, PartialEq, Eq)]
enum WatchView {
    Neutrons,
    Generations,
}

pub struct McApp {
    link: Option<Link>,
    rung: Rung,
    mode: Mode,
    watch_view: WatchView,
    load_id: u32,
    /// Rung and tier of the data the engine holds (once ready).
    loaded: Option<(Rung, Tier)>,
    phase: Phase,
    view: View,
    centres: Vec<(f64, f64)>,
    title: String,
    load_timings: Vec<(&'static str, f64)>,
    load_total_s: f64,
    panel_open: bool,
    /// The panel's initial state is decided on the first frame, from the
    /// screen width: folded on a phone, so the picture gets the screen.
    panel_decided: bool,
    /// The "speed at 1 eV" slider, cm/s; reset to the rung's default when the
    /// rung changes.
    speed_at_1ev: f64,
    /// The Run k_eff panel's settings (kept across runs).
    run_cfg: KeffConfig,
    /// Text size of the Run view (its + / − change it).
    run_text: f32,
    /// `?autostart` (read once at start-up: the URL is rewritten later).
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
            Ok::<Link, String>(engine::start_native(move || ctx.request_repaint()))
        };
        #[cfg(target_arch = "wasm32")]
        let link = engine::start_web(cc.egui_ctx.clone());
        let mut app = Self {
            link: None,
            rung,
            mode,
            watch_view: WatchView::Neutrons,
            load_id: 0,
            loaded: None,
            phase: Phase::Failed(String::new()),
            view: View::new(half_extent(rung)),
            centres: tmodel::particle_centres(tmodel::LAYOUT_SEED),
            title: String::new(),
            load_timings: Vec::new(),
            load_total_s: 0.0,
            panel_open: true,
            panel_decided: false,
            speed_at_1ev: rung.info().spectrum.default_speed_at_1ev(),
            run_cfg: KeffConfig::RUN_DEFAULT,
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

    /// The tier a rung and mode need: Run k_eff processes at NJOY's
    /// tolerance so its `k` is comparable with the record; Watch may use the
    /// loosened tier.
    fn tier_for(rung: Rung, mode: Mode) -> Tier {
        match (rung, mode) {
            (Rung::Godiva, Mode::Run) => Tier::Exact,
            _ => Tier::Loose,
        }
    }

    /// Go to a rung and mode: reload the data if what is loaded will not do,
    /// otherwise just change the screen.
    fn switch(&mut self, rung: Rung, mode: Mode) {
        let mode = if rung.info().has_run { mode } else { Mode::Watch };
        if rung != self.rung {
            self.speed_at_1ev = rung.info().spectrum.default_speed_at_1ev();
            self.view = View::new(half_extent(rung));
            self.watch_view = WatchView::Neutrons;
        }
        self.rung = rung;
        self.mode = mode;
        set_url(rung, mode);
        let need = Self::tier_for(rung, mode);
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
            self.phase = Phase::Loading(Loading::new(rung, need));
            if let Some(link) = &self.link {
                link.send(Request::Load { id: self.load_id, rung, tier: need });
            }
        }
    }

    /// Build the screen for the current rung, mode and view, on loaded data.
    fn enter_screen(&mut self) {
        let Some(link) = &self.link else { return };
        self.phase = Phase::Ready(match (self.mode, self.watch_view) {
            (Mode::Run, _) => {
                let mut it = Iteration::new(self.run_cfg, 0.0);
                if self.autostart {
                    it.start(link);
                }
                Screen::Run(it)
            }
            (Mode::Watch, WatchView::Generations) if self.rung == Rung::Godiva => {
                let mut it = Iteration::new(KeffConfig::WATCH_DEFAULT, 0.6);
                it.start(link);
                Screen::Generations(it)
            }
            (Mode::Watch, _) => Screen::Tracks(Tracks::new(link, self.rung, self.autostart)),
        });
    }

    fn handle(&mut self, events: Vec<Event>) {
        for e in events {
            match (&mut self.phase, e) {
                (_, Event::Error(m)) => self.phase = Phase::Failed(m),
                (Phase::Loading(l), Event::JobStarted { id, index }) if id == self.load_id => {
                    l.job_started[index] = Some(now_s())
                }
                (Phase::Loading(l), Event::JobDone { id, index, secs }) if id == self.load_id => {
                    l.job_secs[index] = Some(secs)
                }
                (Phase::Loading(l), Event::Ready { id }) if id == self.load_id => {
                    self.load_timings = l.jobs.iter().zip(&l.job_secs).map(|(j, s)| (j.0, s.unwrap_or(0.0))).collect();
                    self.load_total_s = now_s() - l.started;
                    self.loaded = Some((l.rung, engine::effective_tier(l.rung, l.tier)));
                    self.enter_screen();
                }
                (Phase::Ready(Screen::Tracks(t)), Event::History { h, animate }) => t.receive(h, animate),
                (Phase::Ready(Screen::Generations(it) | Screen::Run(it)), Event::KeffStarted { sites }) => {
                    it.sites = sites
                }
                (Phase::Ready(Screen::Generations(it) | Screen::Run(it)), Event::Generation(g)) => it.receive(g),
                (Phase::Ready(Screen::Generations(it) | Screen::Run(it)), Event::KeffDone) => it.done(),
                _ => {}
            }
        }
    }

    fn title_now(&mut self, ctx: &egui::Context) {
        let name = self.rung.info().title;
        let t = match &self.phase {
            Phase::Loading(l) => format!("{name} · loading {}/{} · {}", l.done(), l.jobs.len(), l.status_line()),
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
        if !self.panel_decided {
            self.panel_open = ui.max_rect().width() >= 700.0;
            self.panel_decided = true;
        }
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

        // Folds two ways: the panel's own drag handle (`open`), and the hide
        // button inside it (`self.panel_open`); whichever changed wins.
        let before = self.panel_open;
        let mut open = before;
        let panel_w = (ui.max_rect().width() * 0.85).min(340.0);
        egui::Panel::left("controls").default_size(panel_w).resizable(true).show_collapsible(ui, &mut open, |ui| {
            egui::ScrollArea::vertical().show(ui, |ui| self.side_panel(ui));
        });
        if self.panel_open == before {
            self.panel_open = open;
        }
        egui::CentralPanel::default().frame(egui::Frame::NONE).show(ui, |ui| self.canvas(ui));
    }
}

// ─── Side panel ──────────────────────────────────────────────────────────────

impl McApp {
    fn side_panel(&mut self, ui: &mut egui::Ui) {
        ui.horizontal(|ui| {
            ui.heading("Monte Carlo");
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                if ui.button("« Hide").on_hover_text("Fold the panel away to see the whole picture").clicked() {
                    self.panel_open = false;
                }
            });
        });

        // Rung and mode.
        let (mut rung, mut mode) = (self.rung, self.mode);
        ui.horizontal_wrapped(|ui| {
            ui.label("Rung:");
            for r in &RUNGS {
                ui.selectable_value(&mut rung, r.rung, r.name);
            }
        });
        if self.rung.info().has_run {
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
        ui.add(egui::Hyperlink::from_label_and_url("What's happening here? (the lesson)", self.rung.lesson_url()).open_in_new_tab(true));
        ui.separator();

        let mut want_view: Option<WatchView> = None;
        let link = self.link.as_ref();
        let rung = self.rung;
        match &mut self.phase {
            Phase::Loading(l) => Self::loading_panel(ui, l),
            Phase::Failed(e) => {
                ui.colored_label(Color32::from_rgb(255, 90, 90), format!("Failed: {e}"));
            }
            Phase::Ready(Screen::Tracks(t)) => {
                if rung == Rung::Godiva {
                    want_view = Self::watch_view_picker(ui, WatchView::Neutrons);
                }
                Self::tracks_panel(ui, t, link, rung, &mut self.speed_at_1ev);
            }
            Phase::Ready(Screen::Generations(it)) => {
                want_view = Self::watch_view_picker(ui, WatchView::Generations);
                Self::generations_panel(ui, it, link);
            }
            Phase::Ready(Screen::Run(it)) => Self::run_panel(ui, it, link, &mut self.run_cfg),
        }
        if let Some(v) = want_view {
            self.watch_view = v;
            self.enter_screen();
        }

        ui.separator();
        match self.rung {
            Rung::Triso => Self::triso_notes(ui),
            Rung::Godiva => Self::godiva_notes(ui),
        }
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

    fn loading_panel(ui: &mut egui::Ui, l: &Loading) {
        let now = now_s();
        ui.heading("Simulation loading…");
        ui.horizontal(|ui| {
            ui.spinner();
            ui.strong(format!("Processing ENDF files · {}", l.status_line()));
        });
        ui.add(egui::ProgressBar::new(l.fraction(now)).show_percentage().text(format!("rough · {:.0} s", now - l.started)));
        ui.label(format!(
            "Every cross section is reconstructed from evaluated ENDF/B-VIII.0 data by OUTRAM PARK's own NJOY port \
             (RECONR + BROADR, tolerance {}), off the page's main thread, so the page stays live. Covariance data \
             are removed before download: transport never reads them.",
            l.tolerance()
        ));
        if l.rung == Rung::Godiva && engine::effective_tier(l.rung, l.tier) == Tier::Exact {
            ui.colored_label(
                Color32::from_rgb(250, 200, 80),
                "Run k_eff uses NJOY's tolerance 0.001, as the recorded result did, so your k is comparable with \
                 it. That takes longer than Watch mode (0.01): on a fast desktop, natively, about 30 s each for \
                 U-235 and U-238 instead of about 11 s; in a browser, and on a phone, longer still.",
            );
        }
        ui.add_space(6.0);
        egui::Grid::new("jobs").num_columns(2).spacing([16.0, 2.0]).show(ui, |ui| {
            for (i, job) in l.jobs.iter().enumerate() {
                ui.label(job.0);
                if let Some(s) = l.job_secs[i] {
                    ui.label(format!("done in {s:.1} s"));
                } else if let Some(t0) = l.job_started[i] {
                    ui.colored_label(Color32::from_rgb(250, 200, 80), format!("processing… {:.0} s", now - t0));
                } else {
                    ui.weak("queued");
                }
                ui.end_row();
            }
        });
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

    fn tracks_panel(ui: &mut egui::Ui, r: &mut Tracks, link: Option<&Link>, rung: Rung, speed: &mut f64) {
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
            let scatters =
                h.track.states[..=k.min(h.track.states.len() - 1)].iter().filter(|s| s.event == TrackEvent::Scatter).count();
            let born = match (h.from_fission, rung) {
                (true, _) => "at the last neutron's fission site",
                (false, Rung::Triso) => "in a random kernel",
                (false, Rung::Godiva) if h.index == 0 => "at the centre",
                (false, Rung::Godiva) => "at a random point (last one did not fission)",
            };
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
                if rung == Rung::Triso {
                    ui.label("in");
                    ui.label(s.material.map_or("helium (void)", |m| tmodel::MATERIAL_NAMES[m]));
                    ui.end_row();
                }
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
                if rung == Rung::Triso {
                    ui.label("reached thermal (< 0.625 eV)");
                    ui.label(format!("{:.0} %", 100.0 * st.thermalised as f64 / n));
                    ui.end_row();
                }
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
        anim::legend_markers(ui, rung == Rung::Godiva);
        if rung == Rung::Triso {
            ui.add_space(4.0);
            ui.strong("Materials");
            for (i, name) in tmodel::MATERIAL_NAMES.iter().enumerate() {
                if i == tmodel::MAT_OPYC {
                    continue; // same colour and composition as inner PyC
                }
                ui.horizontal(|ui| {
                    let (r, _) = ui.allocate_exact_size(Vec2::splat(12.0), Sense::hover());
                    ui.painter().rect_filled(r, 2.0, MATERIAL_COLOURS[i]);
                    ui.label(if i == tmodel::MAT_IPYC { "Pyrolytic carbon (inner & outer)" } else { name });
                });
            }
            ui.horizontal(|ui| {
                let (r, _) = ui.allocate_exact_size(Vec2::splat(12.0), Sense::hover());
                ui.painter().rect_filled(r, 2.0, HELIUM);
                ui.label("Helium coolant (modelled as void)");
            });
        }
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

    fn generations_panel(ui: &mut egui::Ui, it: &mut Iteration, link: Option<&Link>) {
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
            "Shannon entropy measures how spread out the fission source is (5 × 5 × 5 mesh on the sphere's box). \
             It rises as the source spreads from the centre and flattens once the shape has settled: the \
             inactive generations (shaded) are thrown away for that reason.",
        );
    }

    fn run_panel(ui: &mut egui::Ui, it: &mut Iteration, link: Option<&Link>, cfg: &mut KeffConfig) {
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
                *cfg = KeffConfig::RUN_DEFAULT;
            }
        });
        ui.horizontal(|ui| {
            match it.state {
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
                        it.done();
                    }
                }
            }
        });
        ui.label(format!("{} of {} generations · {:.0} s", it.gens.len(), it.total_gens(), it.elapsed));
        ui.weak(
            "Single-threaded, in this tab's background worker: exactly the power iteration the recorded result \
             ran (outram-mc-libs `PowerIteration`, the `run_keff` reference backend), on ENDF/B-VIII.0 processed \
             at NJOY's tolerance.",
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
            let exp = outram_mc_libs::vv::godiva::BENCHMARK_SIGMA;
            ui.label(format!(
                "Experiment: 1.0000 ± {exp:.4}. Recorded: {:.5} ± {:.5} (−52 ± 27 pcm).",
                gsim::RECORDED_K,
                gsim::RECORDED_SEM
            ));
            let hist = (it.cfg.n_particles * it.cfg.n_active) as f64;
            let expect = gsim::RECORDED_SD_ONE_RUN * (5000.0 * 120.0 / hist).sqrt();
            ui.weak(format!(
                "Why your ± is wider: the record pools 32 independent runs of 5000 × 120 active neutrons, so its ± \
                 is the spread of one such run (151 pcm) divided by √32. One run of yours, {:.0} active neutrons, \
                 is expected to scatter by about {:.0} pcm around the true value.",
                hist,
                expect * 1e5
            ));
        }
    }

    fn triso_notes(ui: &mut egui::Ui) {
        egui::CollapsingHeader::new("What this is — and is not").default_open(false).show(ui, |ui| {
            for line in [
                "Transport: outram-mc-libs continuous-energy Monte Carlo, unmodified. Each neutron is a one-particle fixed-source run with fission progeny switched off, so every track is one real history, drawn projected onto the slice.",
                "Data: ENDF/B-VIII.0 (U-235, U-238, O-16, B-10, B-11, C-12, C-13, Si-28/29/30), reconstructed and Doppler-broadened to 296 K in this browser by OUTRAM PARK's NJOY port, in a background worker. Graphite carbon uses the crystalline-graphite S(α,β) thermal-scattering law.",
                "Covariance data (ENDF files 30–40) are removed before download: transport never reads them. A test proves the stripped tapes give bit-identical cross sections and fission spectra.",
                "Low fidelity, deliberately: reconstruction tolerance 0.01, not NJOY's 0.001.",
                "Geometry: HTR-10 pebble dimensions (IAEA-TECDOC-1382) in 2D. The TRISO particles are therefore infinitely long rods, not spheres: 152 of them, so the fuel fraction of the fuelled zone matches the real pebble (5.0 %). Rods self-shield differently from spheres, so this is a picture of how neutrons move, not a model of HTR-10.",
                "Approximations: carbon in the thin SiC layer is free gas; every fission's next neutron takes the U-235 fission spectrum; helium is void.",
                "Animation speed is proportional to the neutron's real speed (classical kinetic energy, v proportional to √E).",
                "Education and research only.",
            ] {
                ui.label(format!("• {line}"));
            }
        });
    }

    fn godiva_notes(ui: &mut egui::Ui) {
        egui::CollapsingHeader::new("What this is — and is not").default_open(false).show(ui, |ui| {
            for line in [
                "Model: ICSBEP HEU-MET-FAST-001 (Godiva), a bare sphere of highly enriched uranium metal, r = 8.7407 cm, U-234/235/238 at the evaluation's atom densities, 293.6 K. The numbers are outram-mc-libs' `vv::godiva`, the model the recorded result was measured on.",
                "Data: ENDF/B-VIII.0, processed in this browser by OUTRAM PARK's NJOY port. Run k_eff: NJOY's tolerance 0.001 (as the record). Watch: the loosened 0.01, whose measured effect on Godiva is +7 ± 41 pcm.",
                "Run k_eff is true Monte Carlo: outram-mc-libs' single-thread reference power iteration, one generation at a time. Its counts (leaked, captured, fissioned) are tallied by that transport.",
                "Watch, one neutron at a time, is an illustration: each track is one real history (a one-particle fixed-source run), chained by hand. Tracks are 3D, drawn projected from above.",
                "Watch, whole generations, is the real power iteration started from a point at the centre, to show the source spreading.",
                "Animation speed is proportional to the neutron's real speed (classical kinetic energy, v proportional to √E).",
                "Education and research only. Not for reactor operation, licensing or safety decisions.",
            ] {
                ui.label(format!("• {line}"));
            }
        });
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
    let pts: Vec<Pos2> =
        it.gens.iter().filter_map(|g| g.entropy.map(|h| ax.p(g.index as f64 + 1.0, h))).collect();
    if pts.len() > 1 {
        painter.add(egui::Shape::line(pts.clone(), Stroke::new(1.5, Color32::from_rgb(120, 220, 160))));
    }
    for p in pts {
        painter.circle_filled(p, 2.0, Color32::from_rgb(120, 220, 160));
    }
}

/// `k` per generation (dots) and the running mean ± σ over the active ones,
/// with the experiment's band.
fn draw_k(painter: &egui::Painter, rect: Rect, it: &Iteration) {
    let n = it.total_gens().max(2) as f64;
    let mut lo = 0.98f64;
    let mut hi = 1.02f64;
    // The first generations come from the guessed source (generation 1
    // leaks far too much, gh:#527) and would squash the rest: they are
    // drawn clamped to the frame instead of setting its range.
    for g in it.gens.iter().filter(|g| g.index >= 2) {
        lo = lo.min(g.k);
        hi = hi.max(g.k);
    }
    let pad = 0.1 * (hi - lo);
    let ax = Axes { rect, x: (0.5, n + 0.5), y: (lo - pad, hi + pad) };
    ax.frame(painter, "k per generation (dots) · running mean ± σ (band)", |v| format!("{v:.3}"));
    ax.shade_inactive(painter, it.cfg.n_inactive);
    // Experiment: 1.0000 ± 0.0010.
    let s = outram_mc_libs::vv::godiva::BENCHMARK_SIGMA;
    let band = Rect::from_two_pos(ax.p(0.5, 1.0 + s), ax.p(n + 0.5, 1.0 - s)).intersect(rect);
    painter.rect_filled(band, 0.0, Color32::from_rgba_unmultiplied(255, 255, 255, 18));
    painter.line_segment([ax.p(0.5, 1.0), ax.p(n + 0.5, 1.0)], Stroke::new(1.0, Color32::from_rgb(200, 200, 200)));
    painter.text(ax.p(0.5, 1.0) + Vec2::new(4.0, -2.0), egui::Align2::LEFT_BOTTOM, "experiment 1.0000 ± 0.0010", egui::FontId::proportional(10.0), Color32::from_rgb(200, 200, 200));
    // Running mean and its band.
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
            if !self.view.fitted || resp.double_clicked() {
                self.view.fit(rect);
            }
            // Zoom about the pointer (wheel or pinch); drag to pan.
            if resp.hovered() {
                let (scroll, zoom) = ui.input(|i| (i.smooth_scroll_delta.y, i.zoom_delta()));
                let factor = zoom as f64 * (scroll as f64 * 0.0025).exp();
                if (factor - 1.0).abs() > 1e-6 {
                    if let Some(p) = resp.hover_pos() {
                        self.view.zoom_about(rect, p, factor);
                    }
                }
            }
            if resp.dragged() {
                let d = resp.drag_delta();
                self.view.centre[0] -= d.x as f64 / self.view.scale;
                self.view.centre[1] += d.y as f64 / self.view.scale;
            }
            match self.rung {
                Rung::Triso => draw_triso(&painter, rect, &self.view, &self.centres),
                Rung::Godiva => draw_godiva(&painter, rect, &self.view),
            }
        }

        let view = self.view;
        let to_screen = |x: f64, y: f64| view.to_screen(rect, x, y);
        let speed = self.speed_at_1ev;
        match &mut self.phase {
            Phase::Loading(l) => Self::loading_card(&painter, rect, l),
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
                if self.rung == Rung::Godiva {
                    painter.text(rect.left_bottom() + Vec2::new(16.0, -40.0), egui::Align2::LEFT_BOTTOM, "illustration: real histories, chained by hand", egui::FontId::proportional(12.0), Color32::from_rgb(170, 176, 190));
                }
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
                match Self::run_view(ui, rect, &painter, it, self.run_text) {
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

        // Scale bar, where there is a length.
        if !is_run {
            let bar_cm = [0.001, 0.002, 0.005, 0.01, 0.02, 0.05, 0.1, 0.2, 0.5, 1.0, 2.0, 5.0]
                .into_iter()
                .find(|&c| c * self.view.scale >= 80.0)
                .unwrap_or(5.0);
            let x0 = rect.left() + 16.0;
            let y0 = rect.bottom() - 12.0;
            let x1 = x0 + (bar_cm * self.view.scale) as f32;
            painter.line_segment([Pos2::new(x0, y0), Pos2::new(x1, y0)], Stroke::new(2.0, Color32::WHITE));
            let label = if bar_cm < 0.1 { format!("{:.0} µm", bar_cm * 1e4) } else { format!("{bar_cm} cm") };
            painter.text(Pos2::new(x0, y0 - 4.0), egui::Align2::LEFT_BOTTOM, label, egui::FontId::proportional(12.0), Color32::WHITE);
        }

        // With the panel folded away, a real button brings it back: the
        // panel's own drag handle is a few pixels wide and hard to hit on a
        // phone. (While it is open, its "« Hide" button folds it.)
        if !self.panel_open {
            let b = Rect::from_min_size(rect.left_top() + Vec2::new(8.0, 8.0), Vec2::new(104.0, 36.0));
            if ui.put(b, egui::Button::new("Controls »")).clicked() {
                self.panel_open = true;
            }
        }

        // Zoom in, zoom out and reset as real buttons (top right). In Run
        // mode there is no geometry: they scale the console and plots' text.
        let buttons = [("+", "Zoom in", 40.0), ("−", "Zoom out", 40.0), ("Reset", "Centre and fit to the screen", 64.0)];
        let gap = 6.0;
        let total: f32 = buttons.iter().map(|b| b.2).sum::<f32>() + gap * (buttons.len() - 1) as f32;
        let mut x = rect.right() - 8.0 - total;
        for (label, hover, w) in buttons {
            let b = Rect::from_min_size(Pos2::new(x, rect.top() + 8.0), Vec2::new(w, 36.0));
            if ui.put(b, egui::Button::new(RichText::new(label).size(18.0))).on_hover_text(hover).clicked() {
                match (label, is_run) {
                    ("+", false) => self.view.zoom_about(rect, rect.center(), 1.5),
                    ("−", false) => self.view.zoom_about(rect, rect.center(), 1.0 / 1.5),
                    (_, false) => self.view.fit(rect),
                    ("+", true) => self.run_text = (self.run_text * 1.2).min(28.0),
                    ("−", true) => self.run_text = (self.run_text / 1.2).max(8.0),
                    (_, true) => self.run_text = 13.0,
                }
            }
            x += w + gap;
        }
    }

    /// Run k_eff's main view: the two plots, then the console, scrolling,
    /// with Run / Pause / Resume on it (the panel is folded on a phone).
    fn run_view(ui: &mut egui::Ui, rect: Rect, painter: &egui::Painter, it: &Iteration, text: f32) -> RunAction {
        let top = rect.top() + 52.0;
        let narrow = rect.width() < 700.0;
        let plot_h = ((rect.height() - 60.0) * 0.36).clamp(110.0, 260.0);
        let plots = Rect::from_min_max(Pos2::new(rect.left() + 10.0, top), Pos2::new(rect.right() - 10.0, top + plot_h));
        if narrow {
            draw_k(painter, plots, it);
        } else {
            let (a, b) = plots.split_left_right_at_fraction(0.62);
            draw_k(painter, a.shrink2(Vec2::new(0.0, 0.0)), it);
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
        let bw = (brow.width()).min(360.0);
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

    /// The loading progress, drawn on the canvas so it shows even with the
    /// panel collapsed (the phone layout).
    fn loading_card(painter: &egui::Painter, rect: Rect, l: &Loading) {
        if rect.width() < 260.0 {
            return; // the side panel is open over most of the screen and shows the same
        }
        let now = now_s();
        let exact = l.rung == Rung::Godiva && engine::effective_tier(l.rung, l.tier) == Tier::Exact;
        let w = (rect.width() - 32.0).min(440.0);
        let card = Rect::from_center_size(rect.center(), Vec2::new(w, if exact { 168.0 } else { 112.0 }));
        painter.rect_filled(card, 8.0, Color32::from_rgba_unmultiplied(14, 16, 22, 235));
        painter.rect_stroke(card, 8.0, Stroke::new(1.0, Color32::from_rgb(70, 80, 100)), StrokeKind::Inside);
        let f = |s: f32| egui::FontId::proportional(s);
        let left = card.left() + 16.0;
        painter.text(Pos2::new(left, card.top() + 14.0), egui::Align2::LEFT_TOP, "Simulation loading…", f(17.0), Color32::WHITE);
        painter.text(Pos2::new(left, card.top() + 40.0), egui::Align2::LEFT_TOP, format!("Processing ENDF files · {}", l.status_line()), f(13.0), Color32::from_rgb(200, 205, 215));
        let bar = Rect::from_min_size(Pos2::new(left, card.top() + 66.0), Vec2::new(w - 32.0, 12.0));
        painter.rect_filled(bar, 6.0, Color32::from_rgb(40, 46, 58));
        let mut fill = bar;
        fill.set_width(bar.width() * l.fraction(now));
        painter.rect_filled(fill, 6.0, Color32::from_rgb(110, 160, 255));
        painter.text(
            Pos2::new(left, card.top() + 86.0),
            egui::Align2::LEFT_TOP,
            format!("rough · {:.0} % · {:.0} s elapsed", 100.0 * l.fraction(now), now - l.started),
            f(12.0),
            Color32::from_rgb(150, 158, 175),
        );
        if exact {
            let galley_text = "Run k_eff processes at NJOY's tolerance 0.001, like the recorded result, so it takes \
                               about three times longer than Watch mode (a fast desktop natively: about a \
                               minute; a browser or a phone: several).";
            let galley = painter.layout(galley_text.to_string(), f(12.0), Color32::from_rgb(250, 200, 80), w - 32.0);
            painter.galley(Pos2::new(left, card.top() + 108.0), galley, Color32::from_rgb(250, 200, 80));
        }
    }
}
