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
use crate::keff::{console_line, summary_lines, Generation, KeffConfig, KinfCase, KinfGeneration, LineStyle, CONSOLE_HEADER};
use crate::raster::Slicer;
use crate::sweep::RecordedSweep;
use crate::walkdemo::{Outgoing, WalkDemo};
use crate::xs::{self, XsCurve};
use crate::rungs::Mode;
use crate::table::Rung;
use dhoby_ghaut::web_demo::data_cache::Source;
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
    /// Per job: processed now, or read from the browser's cache (gh:#818).
    sources: Vec<Option<Source>>,
}

impl LoadState {
    fn new(rung: Rung, requested: Tier) -> Self {
        let tier = rung.tier(requested);
        let labels: Vec<&'static str> = rung.jobs_for(tier).iter().map(|j| j.0).collect();
        let sources = vec![None; labels.len()];
        Self { rung, tier, loading: Loading::new(labels, rung.job_weights(tier)), sources }
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

// ─── A small k_inf case (gh:#549) ────────────────────────────────────────────

/// A finished (or stopped) `k_inf` run, kept for the session so the reader
/// builds up their own curve.
#[derive(Clone, Copy, Debug)]
struct KinfPoint {
    param: f64,
    mean: f64,
    sem: f64,
    /// Every generation ran (false: stopped early, drawn hollow).
    complete: bool,
    secs: f64,
}

/// The `k_inf` case as the UI follows it: one generation per request, so
/// stopping is not asking for the next.
struct KinfRun {
    case: KinfCase,
    /// The slider: where the NEXT run goes.
    param: f64,
    cfg: KeffConfig,
    state: RunState,
    /// The parameter the current (or last) run is at.
    running_param: f64,
    gens: Vec<KinfGeneration>,
    waiting: bool,
    started_at: f64,
    elapsed: f64,
}

impl KinfRun {
    fn new(case: KinfCase) -> Self {
        Self {
            param: case.default,
            cfg: case.cfg,
            running_param: case.default,
            case,
            state: RunState::Idle,
            gens: Vec::new(),
            waiting: false,
            started_at: 0.0,
            elapsed: 0.0,
        }
    }
    fn start(&mut self, link: &McLink) {
        self.state = RunState::Running;
        self.running_param = self.param;
        self.gens.clear();
        self.started_at = now_s();
        self.elapsed = 0.0;
        link.send(Request::KinfStart { param: self.param, cfg: self.cfg });
        link.send(Request::KinfStep);
        self.waiting = true;
    }
    fn pump(&mut self, link: &McLink) {
        if self.state == RunState::Running {
            self.elapsed = now_s() - self.started_at;
            if !self.waiting {
                link.send(Request::KinfStep);
                self.waiting = true;
            }
        }
    }
    fn receive(&mut self, g: KinfGeneration) {
        // A late generation of a run already replaced is ignored.
        if self.state == RunState::Running && g.param == self.running_param {
            self.waiting = false;
            self.gens.push(g);
        }
    }
    /// End the run (finished, or stopped by the reader); the point it reached.
    fn finish(&mut self, complete: bool) -> Option<KinfPoint> {
        if !matches!(self.state, RunState::Running | RunState::Paused) {
            return None;
        }
        self.state = RunState::Done;
        self.waiting = false;
        self.elapsed = now_s() - self.started_at;
        let g = self.gens.last()?;
        (g.sem.is_finite() && g.sem > 0.0).then_some(KinfPoint { param: g.param, mean: g.mean, sem: g.sem, complete, secs: self.elapsed })
    }
}

// ─── The app ─────────────────────────────────────────────────────────────────

/// What the main view shows.
#[allow(clippy::large_enum_variant)] // held by value: the workspace forbids `Box<T>`
enum Screen {
    Tracks(Tracks),
    Generations(Iteration),
    Run(Iteration),
    Kinf(KinfRun),
    Geometry(Slicer),
    Layers(Layers),
    /// The lattice bed beside the DEM bed (gh:#787).
    Beds(crate::beds::BedsView),
    /// The rung's own demo (gh:#785, [`crate::walkdemo`]).
    Walk(WalkDemo),
    /// The whole HTR-10 core on a worker pool (gh:#786). The view itself is
    /// [`McApp::core`], held by the app so its pool and processed data
    /// outlive a switch to another view or rung (gh:#818).
    Core,
}

/// Send a rung demo's messages to the worker.
fn send_walk(link: Option<&McLink>, out: Vec<Outgoing>) {
    if let Some(link) = link {
        for o in out {
            link.send(match o {
                Outgoing::Raster(r) => Request::Raster(r),
                Outgoing::Walk(m) => Request::Walk(m),
            });
        }
    }
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
    /// The rung's small `k_inf` case (gh:#549).
    Kinf,
    /// A live slice of the assembled geometry, the zoom ladder (gh:#528).
    Geometry,
    /// A recorded sweep read with a slider, beside the geometry (gh:#528).
    Layers,
    /// The lattice bed beside a DEM-poured random bed (gh:#787).
    Beds,
    /// The rung's own demo (gh:#785).
    Demo,
    /// Neutrons in the whole core, live on a worker pool (gh:#786).
    Core,
}

impl WatchView {
    /// `?view=` in the URL (`neutrons`, `generations`, `pitch`).
    fn name(self) -> &'static str {
        match self {
            WatchView::Neutrons => "neutrons",
            WatchView::Generations => "generations",
            WatchView::Kinf => "pitch",
            WatchView::Geometry => "geometry",
            WatchView::Layers => "layers",
            WatchView::Beds => "beds",
            WatchView::Demo => "demo",
            WatchView::Core => "core",
        }
    }
    fn parse(s: &str) -> Option<Self> {
        let all = [WatchView::Neutrons, WatchView::Generations, WatchView::Kinf, WatchView::Geometry, WatchView::Layers, WatchView::Beds, WatchView::Demo, WatchView::Core];
        // `fuel` is the htr10 rung's name for its k_inf view.
        if s == "fuel" {
            return Some(WatchView::Kinf);
        }
        all.into_iter().find(|v| v.name() == s)
    }
    /// The view a rung can show: a rung without tracks opens on its geometry,
    /// and a view the rung does not have falls back to the first it has.
    fn for_rung(self, rung: Rung) -> Self {
        let has = |v: WatchView| match v {
            WatchView::Neutrons => rung.has_tracks(),
            WatchView::Generations => rung.watch_generations().is_some(),
            WatchView::Kinf => rung.kinf_case().is_some(),
            WatchView::Geometry => rung.raster_info().is_some(),
            WatchView::Layers => rung.sweep().is_some(),
            WatchView::Beds => rung.beds(),
            WatchView::Demo => rung.walk_demo().is_some(),
            WatchView::Core => rung.has_core_pool(),
        };
        if has(self) {
            return self;
        }
        [WatchView::Neutrons, WatchView::Core, WatchView::Geometry, WatchView::Layers, WatchView::Kinf, WatchView::Demo].into_iter().find(|&v| has(v)).unwrap_or(WatchView::Neutrons)
    }
}

/// The `layers` view: the recorded sweep and the bed at the slider's N.
struct Layers {
    sweep: RecordedSweep,
    n: f64,
    slicer: Slicer,
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
    /// The σ(E) panel (gh:#549): curves from the loaded data (empty until
    /// the worker sends them, and for a rung without the panel), which are
    /// shown, and whether the panel is on.
    xs: Vec<XsCurve>,
    xs_shown: Vec<bool>,
    xs_on: bool,
    /// The reader's finished `k_inf` runs this session, for the loaded rung.
    kinf_points: Vec<KinfPoint>,
    /// The whole-core view, made the first time it is shown and then kept
    /// for the session: its worker pool, their processed data and any run
    /// survive a switch to another view or rung (gh:#818; until 2026-10-09
    /// the view was rebuilt on every switch and the old pool's workers,
    /// about 545 MB each, were left running unreachable).
    core: Option<crate::htr10::core::screen::CoreScreen>,
    /// Where the loaded rung's data came from, per job (gh:#818).
    data_sources: Vec<Source>,
    /// Cache notes this session (misses with a reason, storage notes).
    data_notes: Vec<String>,
    /// What the cache holds, as the engine last reported it.
    cache_info: Option<String>,
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
        // `?workers=` (gh:#786), read before `switch` rewrites the URL.
        let _ = crate::htr10::core::screen::remember_query();
        let mut app = Self {
            link: None,
            rung,
            mode,
            watch_view: dhoby_ghaut::web_demo::platform::query_value(&dhoby_ghaut::web_demo::platform::query_pairs(), "view")
                .and_then(WatchView::parse)
                .unwrap_or(WatchView::Neutrons),
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
            xs: Vec::new(),
            xs_shown: Vec::new(),
            xs_on: true,
            kinf_points: Vec::new(),
            core: None,
            data_sources: Vec::new(),
            data_notes: Vec::new(),
            cache_info: None,
        };
        match link {
            Ok(l) => {
                l.send(Request::CacheInfo);
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
    /// The k_inf view computes too, so it asks for the full tier (a rung such
    /// as `lct008` processes one tier whatever is asked; `htr10` loads its
    /// tapes only for this view).
    fn tier_for(mode: Mode, view: WatchView) -> Tier {
        match (mode, view) {
            (Mode::Run, _) | (Mode::Watch, WatchView::Kinf) => Tier::Exact,
            (Mode::Watch, _) => Tier::Loose,
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
            self.kinf_points.clear();
            if let Some(c) = rung.run_default() {
                self.run_cfg = c;
            }
        }
        self.rung = rung;
        self.mode = mode;
        self.watch_view = self.watch_view.for_rung(rung);
        self.set_url();
        let need = rung.tier(Self::tier_for(mode, self.watch_view));
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
            self.xs.clear();
            self.xs_shown.clear();
            self.phase = Phase::Loading(LoadState::new(rung, need));
            if let Some(link) = &self.link {
                link.send(Request::Load { id: self.load_id, rung, tier: need });
            }
        }
    }

    /// The URL says what is on screen, so a shared link opens it.
    fn set_url(&self) {
        let view = (self.mode == Mode::Watch && self.watch_view != WatchView::Neutrons).then(|| self.watch_view.name());
        match view {
            Some(v) => set_query(&[("rung", self.rung.name()), ("mode", self.mode.name()), ("view", v)]),
            None => set_query(&[("rung", self.rung.name()), ("mode", self.mode.name())]),
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
            (Mode::Watch, WatchView::Geometry, _) if self.rung.raster_info().is_some() => {
                let info = self.rung.raster_info().expect("checked");
                let (start, param) = (info.start, info.param);
                Screen::Geometry(Slicer::new(info, start, param))
            }
            (Mode::Watch, WatchView::Layers, _) if self.rung.sweep().is_some() => {
                let sweep = self.rung.sweep().expect("checked");
                let n = sweep.default;
                let info = self.rung.raster_info().expect("a sweep rung draws its geometry");
                Screen::Layers(Layers { slicer: Slicer::new(info, 0, n), sweep, n })
            }
            (Mode::Watch, WatchView::Beds, _) if self.rung.beds() => Screen::Beds(crate::beds::BedsView::new()),
            (Mode::Watch, WatchView::Core, _) if self.rung.has_core_pool() && self.rung.raster_info().is_some() => {
                // Made once and kept (gh:#818): coming back to this view
                // finds the pool, its data and any run as they were.
                if self.core.is_none() {
                    let info = self.rung.raster_info().expect("checked");
                    self.core = Some(crate::htr10::core::screen::CoreScreen::new(info, self.speed_at_1ev, self.autostart));
                }
                Screen::Core
            }
            (Mode::Watch, WatchView::Kinf, _) if self.rung.kinf_case().is_some() => {
                let mut k = KinfRun::new(self.rung.kinf_case().expect("checked"));
                if self.autostart {
                    k.start(link);
                }
                Screen::Kinf(k)
            }
            (Mode::Watch, WatchView::Demo, _) if self.rung.walk_demo().is_some() => {
                Screen::Walk(WalkDemo::new(self.rung.walk_demo().expect("checked")))
            }
            (Mode::Watch, _, _) => Screen::Tracks(Tracks::new(link, fast, self.autostart)),
        });
    }

    fn handle(&mut self, ctx: &egui::Context, events: Vec<Event>) {
        let rung = self.rung;
        for e in events {
            match (&mut self.phase, e) {
                (_, Event::Error(m)) => self.phase = Phase::Failed(m),
                (Phase::Loading(l), Event::JobStarted { id, index }) if id == self.load_id => l.loading.job_started(index),
                (Phase::Loading(l), Event::JobDone { id, index, secs, source }) if id == self.load_id => {
                    l.loading.job_done(index, secs);
                    if let Some(s) = l.sources.get_mut(index) {
                        *s = Some(source);
                    }
                }
                (_, Event::DataNote(m)) => {
                    log::warn!("nuclear-data cache: {m}");
                    self.data_notes.push(m);
                }
                (_, Event::CacheInfo(m)) => self.cache_info = Some(m),
                (Phase::Loading(l), Event::Ready { id }) if id == self.load_id => {
                    self.data_sources = l.sources.iter().flatten().cloned().collect();
                    log::info!("{} data: {}", l.rung.info().title, Source::summarize(self.data_sources.iter()));
                    if let Some(link) = &self.link {
                        link.send(Request::CacheInfo);
                    }
                    self.load_timings = l.loading.timings();
                    self.load_total_s = now_s() - l.loading.started;
                    self.loaded = Some((l.rung, l.tier));
                    self.enter_screen();
                    // The σ(E) panel's curves, computed once in the worker.
                    if let Some(link) = &self.link {
                        link.send(Request::XsCurves);
                    }
                }
                (Phase::Ready(Screen::Geometry(sl)), Event::Raster { req, map, secs }) => sl.receive(ctx, req, map, secs),
                (Phase::Ready(Screen::Layers(ly)), Event::Raster { req, map, secs }) => ly.slicer.receive(ctx, req, map, secs),
                (Phase::Ready(Screen::Walk(w)), Event::Raster { req, map, secs }) => w.receive_raster(ctx, req, map, secs),
                (Phase::Ready(Screen::Walk(w)), Event::Walk(m)) => {
                    if let Err(e) = w.receive(&m) {
                        self.phase = Phase::Failed(e);
                    }
                }
                (Phase::Ready(Screen::Core), Event::Raster { req, map, secs }) => {
                    if let Some(c) = self.core.as_mut() {
                        c.slicer.receive(ctx, req, map, secs);
                    }
                }
                (_, Event::XsCurves(c)) => {
                    self.xs_shown = vec![true; c.len()];
                    self.xs = c;
                }
                (Phase::Ready(Screen::Kinf(k)), Event::KinfGeneration(g)) => k.receive(g),
                (Phase::Ready(Screen::Kinf(k)), Event::KinfDone) => {
                    if let Some(p) = k.finish(true) {
                        self.kinf_points.push(p);
                    }
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
            Phase::Ready(Screen::Kinf(k)) => {
                let at = match k.case.choices.iter().find(|c| c.0 == k.running_param) {
                    Some((_, label)) => format!("{} {label}", k.case.param.0),
                    None => format!("{} {:.4} {}", k.case.param.0, k.running_param, k.case.param.1),
                };
                let mean = k.gens.last().filter(|g| g.mean.is_finite()).map_or(String::new(), |g| format!(" · k∞ = {:.5} ± {:.5} · {:.0} s", g.mean, g.sem, k.elapsed));
                format!("{name} · k∞, {at} · generation {}/{} · {:?}{mean}", k.gens.len(), k.cfg.n_inactive + k.cfg.n_active, k.state)
            }
            Phase::Ready(Screen::Geometry(sl)) => {
                let state = if sl.busy() { "slicing" } else if sl.settled() { "slice ready" } else { "waiting" };
                format!("{name} · geometry: {} · {state}", sl.info.ladder[sl.preset].label)
            }
            Phase::Ready(Screen::Layers(ly)) => {
                let state = if ly.slicer.busy() { "slicing" } else if ly.slicer.settled() { "slice ready" } else { "waiting" };
                format!("{name} · layers N = {} · {state}", ly.n)
            }
            Phase::Ready(Screen::Beds(_)) => format!("{name} · liberties: lattice vs random bed"),
            Phase::Ready(Screen::Walk(w)) => format!("{name} · {}", w.status()),
            Phase::Ready(Screen::Core) => format!("{name} · {}", self.core.as_ref().map(|c| c.title()).unwrap_or_default()),
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
            self.handle(&ctx, events);
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
                Screen::Kinf(k) => {
                    k.pump(link);
                    if k.state == RunState::Running {
                        ctx.request_repaint_after(std::time::Duration::from_millis(100));
                    }
                }
                // Raster requests go out from the main view, which knows its size.
                Screen::Geometry(_) | Screen::Layers(_) | Screen::Beds(_) => {}
                Screen::Walk(w) => send_walk(Some(link), w.pump()),
                Screen::Core => {}
            }
        }
        // The core's pool keeps working whatever is on screen (gh:#818): its
        // relays, generations and runs go on behind another view or rung.
        if let Some(c) = self.core.as_mut() {
            c.pump(&ctx);
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
        let views = [WatchView::Neutrons, WatchView::Core, WatchView::Geometry, WatchView::Layers, WatchView::Beds, WatchView::Generations, WatchView::Kinf, WatchView::Demo]
            .into_iter()
            .filter(|&v| v.for_rung(rung) == v)
            .collect::<Vec<_>>();
        let many = views.len() > 1;
        match &mut self.phase {
            Phase::Loading(l) => Self::loading_panel(ui, l),
            Phase::Failed(e) => {
                ui.colored_label(Color32::from_rgb(255, 90, 90), format!("Failed: {e}"));
            }
            Phase::Ready(Screen::Tracks(t)) => {
                if many {
                    want_view = Self::watch_view_picker(ui, WatchView::Neutrons, &views);
                }
                Self::tracks_panel(ui, t, link, rung, &mut self.speed_at_1ev);
                Self::xs_controls(ui, &self.xs, &mut self.xs_shown, &mut self.xs_on);
            }
            Phase::Ready(Screen::Generations(it)) => {
                want_view = Self::watch_view_picker(ui, WatchView::Generations, &views);
                Self::generations_panel(ui, it, link);
            }
            Phase::Ready(Screen::Kinf(k)) => {
                want_view = Self::watch_view_picker(ui, WatchView::Kinf, &views);
                if let Some(p) = Self::kinf_panel(ui, k, link, &self.kinf_points) {
                    self.kinf_points.push(p);
                }
            }
            Phase::Ready(Screen::Run(it)) => Self::run_panel(ui, it, link, &mut self.run_cfg, rung),
            Phase::Ready(Screen::Geometry(sl)) => {
                want_view = Self::watch_view_picker(ui, WatchView::Geometry, &views);
                Self::geometry_panel(ui, sl);
            }
            Phase::Ready(Screen::Layers(ly)) => {
                want_view = Self::watch_view_picker(ui, WatchView::Layers, &views);
                Self::layers_panel(ui, ly);
            }
            Phase::Ready(Screen::Beds(b)) => {
                want_view = Self::watch_view_picker(ui, WatchView::Beds, &views);
                b.panel(ui);
            }
            Phase::Ready(Screen::Walk(w)) => {
                if many {
                    want_view = Self::watch_view_picker(ui, WatchView::Demo, &views);
                }
                send_walk(link, w.panel(ui));
            }
            Phase::Ready(Screen::Core) => {
                want_view = Self::watch_view_picker(ui, WatchView::Core, &views);
                if let Some(c) = self.core.as_mut() {
                    c.panel(ui);
                }
            }
        }
        if let Some(v) = want_view {
            // A view may need other data (htr10's k_inf loads its tapes):
            // `switch` reloads only if what is loaded will not do.
            self.watch_view = v;
            self.switch(self.rung, self.mode);
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
        self.cache_panel(ui);
    }

    /// The shared nuclear-data cache (gh:#818): where this rung's data came
    /// from, what the cache holds, its notes, and the "Clear" control. GUI
    /// drawing (the test rule's exception); the decisions are
    /// `Source::summarize` and the engine's `ClearCache` / `CacheInfo`.
    fn cache_panel(&mut self, ui: &mut egui::Ui) {
        ui.separator();
        ui.strong("Nuclear data");
        if !self.data_sources.is_empty() {
            ui.label(format!("This rung's data: {}.", Source::summarize(self.data_sources.iter())));
        }
        if let Some(c) = self.core.as_ref().and_then(|c| c.pool.as_ref()) {
            ui.label(format!("Whole-core pool: {}.", c.state.data_source()));
        }
        ui.label(format!("Cache: {}.", self.cache_info.as_deref().unwrap_or("asking…")));
        ui.weak("Processed nuclides are kept in this browser (IndexedDB, this site only), keyed by each tape's contents, the processing settings and the code version, so a reload, another demo page or a discarded tab reads them back instead of processing again.");
        ui.horizontal_wrapped(|ui| {
            if ui.add(egui::Button::new("🗑 Clear cached nuclear data").min_size(egui::vec2(0.0, 36.0))).clicked() {
                if let Some(link) = &self.link {
                    link.send(Request::ClearCache);
                }
                self.cache_info = None;
            }
            if ui.add(egui::Button::new("↻ Refresh").min_size(egui::vec2(0.0, 36.0))).clicked() {
                if let Some(link) = &self.link {
                    link.send(Request::CacheInfo);
                }
            }
        });
        if !self.data_notes.is_empty() {
            egui::CollapsingHeader::new(format!("Cache notes ({})", self.data_notes.len())).default_open(true).show(ui, |ui| {
                for n in self.data_notes.iter().rev().take(12) {
                    ui.colored_label(Color32::from_rgb(250, 200, 80), n);
                }
            });
        }
    }

    /// The rung's views (`views`, from [`WatchView::for_rung`]).
    fn watch_view_picker(ui: &mut egui::Ui, now: WatchView, views: &[WatchView]) -> Option<WatchView> {
        let mut v = now;
        ui.horizontal_wrapped(|ui| {
            ui.label("Watch:");
            for &w in views {
                let label = match w {
                    WatchView::Neutrons => "one neutron at a time",
                    WatchView::Generations => "whole generations",
                    WatchView::Kinf => "k∞ (true MC)",
                    WatchView::Geometry => "geometry (zoom ladder)",
                    WatchView::Layers => "layers (recorded k)",
                    WatchView::Beds => "liberties: lattice vs random bed",
                    WatchView::Demo => "the demo",
                    WatchView::Core => "whole core (neutrons, live k)",
                };
                ui.selectable_value(&mut v, w, label);
            }
        });
        (v != now).then_some(v)
    }

    fn geometry_panel(ui: &mut egui::Ui, sl: &mut Slicer) {
        ui.label("Zoom ladder: each step is a slice of the core the recorded runs used, drawn in this tab's worker.");
        let mut go = None;
        ui.horizontal_wrapped(|ui| {
            for (i, p) in sl.info.ladder.iter().enumerate() {
                if ui.selectable_label(i == sl.preset, p.label).clicked() {
                    go = Some(i);
                }
            }
        });
        if let Some(i) = go {
            sl.go(i);
        }
        Self::slice_legend(ui, sl);
    }

    fn slice_legend(ui: &mut egui::Ui, sl: &Slicer) {
        ui.weak(format!("Source: {}. Last slice: {:.1} s in the worker.", sl.info.source, sl.last_secs));
        ui.strong("In this slice");
        for (c, name) in sl.present() {
            ui.horizontal(|ui| {
                let (r, _) = ui.allocate_exact_size(Vec2::splat(12.0), Sense::hover());
                ui.painter().rect_filled(r, 2.0, c);
                ui.label(name.to_lowercase());
            });
        }
    }

    fn layers_panel(ui: &mut egui::Ui, ly: &mut Layers) {
        ui.add(egui::Slider::new(&mut ly.n, ly.sweep.range.0..=ly.sweep.range.1).step_by(1.0).fixed_decimals(0).text(ly.sweep.param));
        ly.slicer.param = ly.n;
        for d in ly.sweep.details(ly.n) {
            ui.label(d);
        }
        ui.weak("Recorded results only: nothing is computed here. The bed beside the plot is built in the worker at this N.");
        egui::CollapsingHeader::new("Deliberate liberties of the record").default_open(true).show(ui, |ui| {
            for n in &ly.sweep.notes {
                ui.label(format!("• {n}"));
            }
        });
        Self::slice_legend(ui, &ly.slicer);
    }

    /// The σ(E) panel's switch and one toggle per curve.
    fn xs_controls(ui: &mut egui::Ui, curves: &[XsCurve], shown: &mut [bool], on: &mut bool) {
        if curves.is_empty() {
            return;
        }
        ui.separator();
        ui.checkbox(on, "σ(E) panel beside the picture");
        if *on {
            for (c, s) in curves.iter().zip(shown.iter_mut()) {
                ui.horizontal(|ui| {
                    let (r, _) = ui.allocate_exact_size(Vec2::new(14.0, 12.0), Sense::hover());
                    ui.painter().line_segment([r.left_center(), r.right_center()], Stroke::new(2.0, xs::colour(c.channel)));
                    ui.checkbox(s, c.label.as_str());
                });
            }
            ui.weak(
                "The cross sections this tab processed for transport (RECONR + BROADR, and the moderator's S(α,β) law below its \
                 cutoff), read through the same call the collisions use, decimated for drawing (each of 900 log bins keeps its \
                 highest and lowest point). The dot rides the neutron's energy; a cross marks where it was captured or fissioned.",
            );
        }
    }

    /// The `k_inf` case's controls. Returns a point if the reader stopped a run.
    fn kinf_panel(ui: &mut egui::Ui, k: &mut KinfRun, link: Option<&McLink>, done: &[KinfPoint]) -> Option<KinfPoint> {
        let mut stopped = None;
        ui.strong(k.case.title);
        Self::kinf_slider(ui, k, 0.0);
        ui.add_enabled_ui(k.state != RunState::Running, |ui| {
            egui::Grid::new("kinf_cfg").num_columns(2).show(ui, |ui| {
                ui.label("neutrons / generation");
                ui.add(egui::DragValue::new(&mut k.cfg.n_particles).range(100..=20_000).speed(50));
                ui.end_row();
                ui.label("inactive generations");
                ui.add(egui::DragValue::new(&mut k.cfg.n_inactive).range(1..=200));
                ui.end_row();
                ui.label("active generations");
                ui.add(egui::DragValue::new(&mut k.cfg.n_active).range(2..=1000));
                ui.end_row();
                ui.label("seed");
                ui.add(egui::DragValue::new(&mut k.cfg.seed).range(1..=1_000_000));
                ui.end_row();
            });
            if ui.button("Defaults").clicked() {
                k.cfg = k.case.cfg;
            }
        });
        ui.horizontal(|ui| {
            if k.state == RunState::Running {
                if ui.button("■ Stop").clicked() {
                    stopped = k.finish(false);
                }
            } else if ui.button("▶ Run").clicked() {
                if let Some(link) = link {
                    k.start(link);
                }
            }
        });
        let at = |v: f64| match k.case.choices.iter().find(|c| c.0 == v) {
            Some((_, label)) => format!("{} {label}", k.case.param.0),
            None => format!("{} {v:.4} {}", k.case.param.0, k.case.param.1),
        };
        if let Some(g) = k.gens.last() {
            ui.label(format!(
                "{}: generation {} of {} ({}), k = {:.4}{} · {:.0} s",
                at(g.param),
                g.index + 1,
                g.total,
                if g.active { "active" } else { "inactive" },
                g.k,
                if g.sem.is_finite() { format!(", mean {:.4} ± {:.4}", g.mean, g.sem) } else { String::new() },
                k.elapsed
            ));
        }
        if !done.is_empty() {
            ui.separator();
            ui.strong("Your runs");
            egui::Grid::new("kinf_done").num_columns(3).show(ui, |ui| {
                for p in done {
                    ui.label(at(p.param));
                    ui.label(format!("{:.4} ± {:.4}", p.mean, p.sem));
                    ui.label(if p.complete { format!("{:.0} s", p.secs) } else { "stopped".into() });
                    ui.end_row();
                }
            });
        }
        egui::CollapsingHeader::new("What this run is — and is not").default_open(false).show(ui, |ui| {
            for n in &k.case.notes {
                ui.label(format!("• {n}"));
            }
        });
        stopped
    }

    /// The parameter slider, `width` px wide (0: the default width).
    fn kinf_slider(ui: &mut egui::Ui, k: &mut KinfRun, width: f32) {
        let (name, unit) = k.case.param;
        if !k.case.choices.is_empty() {
            // Discrete cases: one finger-sized button each.
            ui.horizontal(|ui| {
                ui.label(format!("{name}:"));
                for &(v, label) in &k.case.choices {
                    if ui.add(egui::Button::selectable(k.param == v, RichText::new(label).size(15.0)).min_size(Vec2::new(0.0, 30.0))).clicked() {
                        k.param = v;
                    }
                }
            });
            return;
        }
        ui.scope(|ui| {
            if width > 0.0 {
                ui.spacing_mut().slider_width = width;
            }
            ui.add(egui::Slider::new(&mut k.param, k.case.range.0..=k.case.range.1).step_by(0.005).text(format!("{name} ({unit})")));
        });
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
    // The first generations come from the guessed source (on a bare sphere
    // generation 1 leaks more than the settled source: k_1 = 0.897 for
    // Godiva, seed 1, and 0.468 before gh:#527 was fixed) and would squash
    // the rest: they are drawn clamped to the frame instead of setting its
    // range.
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
        let (resp, full_painter) = ui.allocate_painter(ui.available_size(), Sense::hover());
        let full = resp.rect;
        full_painter.rect_filled(full, 0.0, BG);
        let is_run = matches!(self.phase, Phase::Ready(Screen::Run(_)));
        let is_kinf = matches!(self.phase, Phase::Ready(Screen::Kinf(_)));
        // The σ(E) panel (gh:#549) beside the geometry on a wide screen,
        // under it on a narrow (portrait) one.
        let xs_on = self.xs_on && !self.xs.is_empty() && matches!(self.phase, Phase::Ready(Screen::Tracks(_)));
        // The layers view splits the same way: the bed beside its plot.
        let is_layers = matches!(self.phase, Phase::Ready(Screen::Layers(_)));
        let is_beds = matches!(self.phase, Phase::Ready(Screen::Beds(_)));
        let is_slice = is_layers || matches!(self.phase, Phase::Ready(Screen::Geometry(_) | Screen::Core));
        let (rect, xs_rect) = if !(xs_on || is_layers) {
            (full, None)
        } else if full.width() >= 700.0 && full.width() > full.height() {
            let (a, b) = full.split_left_right_at_fraction(0.58);
            (a, Some(b.shrink2(Vec2::new(6.0, 8.0)).translate(Vec2::new(-2.0, 0.0))))
        } else {
            let (a, b) = full.split_top_bottom_at_fraction(0.55);
            (a, Some(b.shrink2(Vec2::new(8.0, 4.0))))
        };
        let painter = full_painter.with_clip_rect(rect);
        let geo_resp = ui.interact(rect, ui.id().with("geometry"), Sense::click_and_drag());
        // A rung's own demo splits the view itself (`walkdemo::split`); the
        // zoom buttons and scale bar belong to its picture.
        let is_walk = matches!(self.phase, Phase::Ready(Screen::Walk(_)));
        let zoom_rect = if is_walk { crate::walkdemo::split(full).0 } else { rect };
        if !is_run && !is_kinf && !is_slice && !is_beds && !is_walk {
            self.view.handle_input(ui, &geo_resp);
            self.rung.draw(&painter, rect, &self.view);
        }

        let view = self.view;
        let to_screen = |x: f64, y: f64| view.to_screen(rect, x, y);
        let speed = self.speed_at_1ev;
        let rung = self.rung;
        match &mut self.phase {
            Phase::Loading(l) => {
                let title = "Simulation loading…";
                // Where the data are coming from (gh:#818), under the rung's
                // own note.
                let note = match (rung.loading_note(l.tier), l.sources.iter().any(Option::is_some)) {
                    (n, false) => n.map(str::to_string),
                    (None, true) => Some(format!("So far: {}.", Source::summarize(l.sources.iter().flatten()))),
                    (Some(n), true) => Some(format!("{n}\nSo far: {}.", Source::summarize(l.sources.iter().flatten()))),
                };
                l.loading.card(&painter, rect, title, note.as_deref());
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
                if let Some(xr) = xs_rect {
                    // The marker rides the animated neutron; once it is
                    // absorbed it leaves both views and a cross marks where.
                    let marker = r.current.as_ref().and_then(|a| {
                        if !a.finished() {
                            return Some(xs::Marker { energy_ev: a.energy_now(), ended: None });
                        }
                        let ended = match a.hist.outcome {
                            Some(TrackEvent::Absorption) => "captured",
                            Some(TrackEvent::Fission) => "fission",
                            _ => return None,
                        };
                        Some(xs::Marker { energy_ev: a.hist.track.states.last()?.energy, ended: Some(ended) })
                    });
                    xs::draw(&full_painter, xr, &self.xs, &self.xs_shown, marker, 12.0);
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
            Phase::Ready(Screen::Kinf(k)) => {
                if let Some(p) = Self::kinf_view(ui, rect, &painter, k, &self.kinf_points, self.link.as_ref(), self.run_text) {
                    self.kinf_points.push(p);
                }
            }
            Phase::Ready(Screen::Geometry(sl)) => {
                Self::slice_view(ui, rect, &painter, &geo_resp, sl, self.link.as_ref());
                // The ladder, finger-sized, along the bottom of the main view.
                let n = sl.info.ladder.len();
                let bw = ((rect.width() - 20.0 - 6.0 * (n as f32 - 1.0)) / n as f32).min(110.0);
                let mut go = None;
                // A dark strip, so unselected (transparent) buttons read over the slice.
                let strip = Rect::from_min_max(Pos2::new(rect.left() + 4.0, rect.bottom() - 80.0), Pos2::new(rect.left() + 16.0 + n as f32 * (bw + 6.0), rect.bottom() - 36.0));
                painter.rect_filled(strip, 6.0, Color32::from_rgba_unmultiplied(14, 16, 20, 215));
                for (i, p) in sl.info.ladder.iter().enumerate() {
                    // Above the scale bar.
                    let b = Rect::from_min_size(Pos2::new(rect.left() + 10.0 + i as f32 * (bw + 6.0), rect.bottom() - 76.0), Vec2::new(bw, 36.0));
                    if ui.put(b, egui::Button::selectable(i == sl.preset, RichText::new(p.label).size(13.0))).clicked() {
                        go = Some(i);
                    }
                }
                if let Some(i) = go {
                    sl.go(i);
                }
            }
            Phase::Ready(Screen::Beds(b)) => b.draw(ui, rect, &painter, &geo_resp),
            Phase::Ready(Screen::Core) => {
                if let Some(c) = self.core.as_mut() {
                    c.canvas(ui, rect, &painter, &geo_resp, self.link.as_ref());
                }
            }
            Phase::Ready(Screen::Layers(ly)) => {
                Self::slice_view(ui, rect, &painter, &geo_resp, &mut ly.slicer, self.link.as_ref());
                if let Some(pr) = xs_rect {
                    // The N slider on the main view (the panel is folded on a phone).
                    let row = Rect::from_min_size(pr.left_top() + Vec2::new(4.0, 2.0), Vec2::new(pr.width() - 8.0, 32.0));
                    ui.scope_builder(egui::UiBuilder::new().max_rect(row), |ui| {
                        ui.spacing_mut().slider_width = (row.width() - 120.0).max(80.0);
                        ui.spacing_mut().interact_size.y = 30.0;
                        ui.add(egui::Slider::new(&mut ly.n, ly.sweep.range.0..=ly.sweep.range.1).step_by(1.0).fixed_decimals(0).text(ly.sweep.param));
                    });
                    ly.slicer.param = ly.n;
                    // The selected row, short, so a phone shows it with the panel folded.
                    let mut y = row.bottom() + 4.0;
                    // Wrapped to the view's width (a phone is narrower than one
                    // row), newest curve first (a sweep lists superseded curves
                    // before the ones that replace them, #589), and only as many
                    // rows as leave the plot room to draw; the rest are in the
                    // panel.
                    let text = (self.run_text * 0.92).max(9.0);
                    let plot_needs = ly.sweep.curves.len() as f32 * text * 1.3 + text * 3.5 + 8.0 + 90.0;
                    let rows: Vec<String> = ly.sweep.details(ly.n).iter().rev().map(|d| d.split(';').next().unwrap_or("").to_string()).collect();
                    let mut shown = 0;
                    for d in &rows {
                        let g = full_painter.layout(d.clone(), egui::FontId::proportional(12.0), Color32::from_rgb(210, 216, 226), pr.width() - 12.0);
                        let h = g.size().y;
                        if pr.bottom() - (y + h + 18.0) < plot_needs {
                            break;
                        }
                        full_painter.galley(Pos2::new(pr.left() + 6.0, y), g, Color32::from_rgb(210, 216, 226));
                        y += h + 2.0;
                        shown += 1;
                    }
                    if shown < rows.len() {
                        full_painter.text(Pos2::new(pr.left() + 6.0, y), egui::Align2::LEFT_TOP, format!("+{} more at this N under Controls", rows.len() - shown), egui::FontId::proportional(11.0), Color32::from_rgb(150, 156, 170));
                        y += 16.0;
                    }
                    let plot = Rect::from_min_max(Pos2::new(pr.left(), y + 2.0), pr.right_bottom());
                    crate::sweep::draw(&full_painter, plot, &ly.sweep, ly.n, text);
                }
            }
            Phase::Ready(Screen::Walk(w)) => {
                let out = w.canvas(ui, rect, &painter, &geo_resp);
                send_walk(self.link.as_ref(), out);
            }
        }

        let (sview, sview_rect) = match &mut self.phase {
            Phase::Ready(Screen::Geometry(sl)) => (Some(&mut sl.view), rect),
            Phase::Ready(Screen::Layers(ly)) => (Some(&mut ly.slicer.view), rect),
            Phase::Ready(Screen::Beds(b)) => (Some(&mut b.view), rect),
            Phase::Ready(Screen::Walk(w)) => (Some(w.view_mut()), zoom_rect),
            Phase::Ready(Screen::Core) => (self.core.as_mut().map(|c| &mut c.slicer.view), rect),
            _ => (None, rect),
        };
        if let Some(v) = &sview {
            scale_bar(&painter, sview_rect, v);
        } else if !is_run && !is_kinf {
            scale_bar(&painter, rect, &self.view);
        }
        self.panel.reopen_button(ui, full);
        // + / − / Reset. In Run mode and the k∞ plot there is no geometry:
        // they scale the text.
        if let Some(z) = zoom_buttons(ui, zoom_rect) {
            match (is_run || is_kinf, z) {
                (false, z) => match sview {
                    Some(v) => apply_zoom(v, zoom_rect, z),
                    None => apply_zoom(&mut self.view, rect, z),
                },
                (true, Zoom::In) => self.run_text = (self.run_text * 1.2).min(28.0),
                (true, Zoom::Out) => self.run_text = (self.run_text / 1.2).max(8.0),
                (true, Zoom::Reset) => self.run_text = 13.0,
            }
        }
    }

    /// A live slice: gestures, the newest image under the current view, a new
    /// raster asked for once the view settles, and its status.
    fn slice_view(ui: &mut egui::Ui, rect: Rect, painter: &egui::Painter, resp: &egui::Response, sl: &mut Slicer, link: Option<&McLink>) {
        sl.view.handle_input(ui, resp);
        if let (Some(req), Some(link)) = (sl.pump(rect), link) {
            link.send(Request::Raster(req));
        }
        sl.draw(painter, rect);
        if sl.busy() || !sl.settled() {
            ui.ctx().request_repaint_after(std::time::Duration::from_millis(80));
        }
        if sl.busy() {
            painter.text(rect.left_bottom() + Vec2::new(16.0, -88.0), egui::Align2::LEFT_BOTTOM, "slicing the assembled core in the worker…", egui::FontId::proportional(12.0), Color32::from_rgb(250, 200, 80));
        }
    }

    /// The `k_inf` case's main view: the plot against the recorded curves,
    /// with the slider and Run / Stop on it (the panel is folded on a phone).
    /// Returns a point if the reader stopped a run here.
    fn kinf_view(ui: &mut egui::Ui, rect: Rect, painter: &egui::Painter, k: &mut KinfRun, done: &[KinfPoint], link: Option<&McLink>, text: f32) -> Option<KinfPoint> {
        let mut stopped = None;
        let top = rect.top() + 52.0;
        // Controls row: slider, then Run / Stop.
        let row = Rect::from_min_size(Pos2::new(rect.left() + 10.0, top), Vec2::new(rect.width() - 20.0, 36.0));
        let narrow = rect.width() < 560.0;
        let (srow, brow) = if narrow {
            (row, Rect::from_min_size(row.left_bottom() + Vec2::new(0.0, 6.0), Vec2::new(row.width().min(360.0), 36.0)))
        } else {
            let b = Rect::from_min_size(Pos2::new(row.right() - 230.0, row.top()), Vec2::new(230.0, 36.0));
            (row.with_max_x(b.left() - 10.0), b)
        };
        ui.scope_builder(egui::UiBuilder::new().max_rect(srow), |ui| {
            ui.spacing_mut().interact_size.y = 30.0;
            Self::kinf_slider(ui, k, (srow.width() - 150.0).max(80.0));
        });
        let label = match k.state {
            RunState::Running => format!("■ Stop · gen {}/{} · {:.0} s", k.gens.len(), k.cfg.n_inactive + k.cfg.n_active, k.elapsed),
            _ => match k.case.choices.iter().find(|c| c.0 == k.param) {
                Some((_, label)) => format!("▶ Run k∞, {label}"),
                None => format!("▶ Run k∞ at {:.3} {}", k.param, k.case.param.1),
            },
        };
        if ui.put(brow, egui::Button::new(RichText::new(label).size(15.0))).clicked() {
            if k.state == RunState::Running {
                stopped = k.finish(false);
            } else if let Some(link) = link {
                k.start(link);
            }
        }
        let plot = Rect::from_min_max(Pos2::new(rect.left() + 10.0, brow.bottom() + 10.0), Pos2::new(rect.right() - 10.0, rect.bottom() - 10.0));
        draw_kinf(painter, plot, k, done, text);
        stopped
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

/// `k_inf` against the parameter: the recorded curves in the workspace's
/// plotting convention (ours dotted, a reference code dashed, recorded points
/// as markers with ±1σ bars), the reader's finished runs as filled markers
/// (hollow if stopped early), the running mean ± σ of the run in progress,
/// the marked values and the slider's position.
fn draw_kinf(painter: &egui::Painter, rect: Rect, k: &KinfRun, done: &[KinfPoint], text: f32) {
    let f = egui::FontId::proportional(text);
    let small = egui::FontId::proportional((text * 0.85).max(8.0));
    let grey = Color32::from_rgb(170, 176, 190);
    painter.rect_filled(rect, 4.0, Color32::from_rgb(18, 21, 28));
    painter.text(rect.left_top() + Vec2::new(8.0, 5.0), egui::Align2::LEFT_TOP, k.case.title, f.clone(), Color32::WHITE);
    let ours = Color32::from_rgb(120, 170, 255);
    let reference = Color32::from_rgb(200, 200, 200);
    let mine = Color32::from_rgb(120, 230, 150);
    let live = Color32::from_rgb(255, 200, 80);
    // Ranges: the case's parameter range; k from the curves and the runs.
    let categorical = !k.case.choices.is_empty();
    let (x0, x1) = if categorical { (k.case.range.0 - 0.6, k.case.range.1 + 0.6) } else { (k.case.range.0 - 0.05, k.case.range.1 + 0.05) };
    let (mut y0, mut y1) = (f64::INFINITY, f64::NEG_INFINITY);
    let mut see = |v: f64| {
        if v.is_finite() {
            y0 = y0.min(v);
            y1 = y1.max(v);
        }
    };
    k.case.curves.iter().flat_map(|c| &c.points).for_each(|p| see(p.1));
    done.iter().for_each(|p| see(p.mean));
    if let Some(g) = k.gens.last() {
        see(g.mean);
    }
    if !y0.is_finite() {
        (y0, y1) = (0.0, 1.5);
    }
    let pad = 0.06 * (y1 - y0).max(0.05);
    let (y0, y1) = (y0 - pad, y1 + pad);
    let legend_lines = k.case.curves.len() + 2;
    let legend_h = legend_lines as f32 * text * 1.3 + 8.0;
    let plot = Rect::from_min_max(rect.left_top() + Vec2::new(text * 3.4, text * 1.8 + 8.0), rect.right_bottom() - Vec2::new(10.0, text * 1.7 + legend_h));
    if plot.height() < 40.0 {
        return;
    }
    let p = |x: f64, y: f64| {
        Pos2::new(
            plot.left() + ((x - x0) / (x1 - x0)) as f32 * plot.width(),
            plot.bottom() - ((y - y0) / (y1 - y0)) as f32 * plot.height(),
        )
    };
    let gridc = Color32::from_rgb(40, 45, 56);
    // Ticks at least ~48 px apart; a categorical case labels its cases.
    let px_per = plot.width() as f64 / (x1 - x0);
    let xstep = [0.25, 0.5, 1.0].into_iter().find(|s| s * px_per >= 48.0).unwrap_or(1.0);
    for &(v, label) in &k.case.choices {
        painter.text(Pos2::new(p(v, y0).x, plot.bottom() + 2.0), egui::Align2::CENTER_TOP, label, small.clone(), grey);
    }
    let mut x = if categorical { f64::INFINITY } else { (x0 / xstep).ceil() * xstep };
    while x <= x1 {
        let a = p(x, y0);
        painter.line_segment([a, Pos2::new(a.x, plot.top())], Stroke::new(1.0, gridc));
        painter.text(Pos2::new(a.x, plot.bottom() + 2.0), egui::Align2::CENTER_TOP, format!("{x:.2}"), small.clone(), grey);
        x += xstep;
    }
    let ystep = if y1 - y0 > 0.5 { 0.1 } else { 0.05 };
    let mut y = (y0 / ystep).ceil() * ystep;
    while y <= y1 {
        let a = p(x0, y);
        painter.line_segment([a, Pos2::new(plot.right(), a.y)], Stroke::new(1.0, gridc));
        painter.text(Pos2::new(plot.left() - 3.0, a.y), egui::Align2::RIGHT_CENTER, format!("{y:.2}"), small.clone(), grey);
        y += ystep;
    }
    if !categorical {
        painter.text(Pos2::new(plot.right(), plot.bottom() + text * 1.15 + 2.0), egui::Align2::RIGHT_TOP, format!("{} ({})", k.case.param.0, k.case.param.1), small.clone(), grey);
    }
    painter.text(Pos2::new(plot.left() + 4.0, plot.top() + 2.0), egui::Align2::LEFT_TOP, "k∞", small.clone(), grey);
    let clip = painter.with_clip_rect(plot.expand(4.0));
    for (v, name) in &k.case.marks {
        let a = p(*v, y1);
        clip.extend(egui::Shape::dashed_line(&[a, p(*v, y0)], Stroke::new(1.0, Color32::from_rgb(190, 170, 230)), 4.0, 4.0));
        clip.text(a + Vec2::new(3.0, 2.0), egui::Align2::LEFT_TOP, *name, small.clone(), Color32::from_rgb(190, 170, 230));
    }
    // The slider's position.
    let s = p(k.param, y1);
    clip.line_segment([s, p(k.param, y0)], Stroke::new(1.0, live.gamma_multiply(0.5)));
    for c in &k.case.curves {
        let col = if c.style == LineStyle::Reference { reference } else { ours };
        let pts: Vec<Pos2> = c.points.iter().map(|q| p(q.0, q.1)).collect();
        match c.style {
            // Separate cases are not joined by a line.
            _ if categorical => {}
            LineStyle::Published => {
                clip.add(egui::Shape::line(pts.clone(), Stroke::new(1.5, col)));
            }
            LineStyle::Ours => clip.extend(egui::Shape::dotted_line(&pts, col, 5.0, 1.3)),
            LineStyle::Reference => clip.extend(egui::Shape::dashed_line(&pts, Stroke::new(1.3, col), 7.0, 5.0)),
        }
        for q in &c.points {
            let a = p(q.0, q.1);
            clip.line_segment([p(q.0, q.1 - q.2), p(q.0, q.1 + q.2)], Stroke::new(1.0, col));
            if c.style == LineStyle::Reference {
                clip.rect_stroke(Rect::from_center_size(a, Vec2::splat(6.0)), 0.0, Stroke::new(1.2, col), StrokeKind::Middle);
            } else {
                clip.circle_stroke(a, 3.0, Stroke::new(1.2, col));
            }
        }
    }
    let bar = |y: f64, e: f64, x: f64, col: Color32| {
        clip.line_segment([p(x, y - e), p(x, y + e)], Stroke::new(2.0, col));
        for yy in [y - e, y + e] {
            let a = p(x, yy);
            clip.line_segment([a - Vec2::new(4.0, 0.0), a + Vec2::new(4.0, 0.0)], Stroke::new(2.0, col));
        }
    };
    for d in done {
        bar(d.mean, d.sem, d.param, mine);
        if d.complete {
            clip.circle_filled(p(d.param, d.mean), 4.5, mine);
        } else {
            clip.circle_stroke(p(d.param, d.mean), 4.5, Stroke::new(1.5, mine));
        }
    }
    if k.state == RunState::Running {
        if let Some(g) = k.gens.last() {
            if g.sem.is_finite() {
                bar(g.mean, g.sem, g.param, live);
                clip.circle_filled(p(g.param, g.mean), 5.0, live);
            } else {
                // Inactive generations: this generation's k, faint.
                clip.circle_stroke(p(g.param, g.k.clamp(y0, y1)), 5.0, Stroke::new(1.5, live.gamma_multiply(0.6)));
            }
        }
    }
    // With two cases, the reader's own difference between them.
    if categorical && k.case.choices.len() == 2 {
        let last = |v: f64| done.iter().rev().find(|d| d.param == v && d.complete);
        if let (Some(a), Some(b)) = (last(k.case.choices[0].0), last(k.case.choices[1].0)) {
            let (d, e) = ((a.mean - b.mean) * 1e5, (a.sem * a.sem + b.sem * b.sem).sqrt() * 1e5);
            painter.text(
                Pos2::new(plot.left() + 6.0, plot.top() + text * 1.3),
                egui::Align2::LEFT_TOP,
                format!("your runs: {} − {} = {d:+.0} ± {e:.0} pcm", k.case.choices[0].1, k.case.choices[1].1),
                egui::FontId::proportional(text),
                Color32::from_rgb(120, 230, 150),
            );
        }
    }
    // Legend under the axis.
    let mut ly = plot.bottom() + text * 1.7 + 8.0;
    let mut line = |draw: &dyn Fn(Pos2), label: &str| {
        let a = Pos2::new(rect.left() + 12.0, ly + text * 0.55);
        draw(a);
        painter.text(a + Vec2::new(30.0, 0.0), egui::Align2::LEFT_CENTER, label, small.clone(), Color32::from_rgb(210, 216, 226));
        ly += text * 1.3;
    };
    for c in &k.case.curves {
        let col = if c.style == LineStyle::Reference { reference } else { ours };
        let style = c.style;
        line(
            &|a: Pos2| {
                let seg = [a, a + Vec2::new(24.0, 0.0)];
                match style {
                    LineStyle::Published => {
                        painter.line_segment(seg, Stroke::new(1.5, col));
                    }
                    LineStyle::Ours => painter.extend(egui::Shape::dotted_line(&seg, col, 5.0, 1.3)),
                    LineStyle::Reference => painter.extend(egui::Shape::dashed_line(&seg, Stroke::new(1.3, col), 7.0, 5.0)),
                }
            },
            &c.label,
        );
    }
    line(&|a: Pos2| { painter.circle_filled(a + Vec2::new(12.0, 0.0), 4.5, mine); }, "your runs (hollow: stopped early), ± 1σ");
    line(&|a: Pos2| { painter.circle_filled(a + Vec2::new(12.0, 0.0), 5.0, live); }, "the run in progress, mean ± σ");
}
