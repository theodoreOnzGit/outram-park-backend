//! The egui front end: loading screen, the pebble, and the animated track.
//!
//! The UI never does physics: [`crate::engine`] runs the ENDF processing and
//! the neutrons on a thread (natively) or in a Web Worker (in the browser),
//! and this file only draws what it reports. So the page keeps animating —
//! progress bar, elapsed time, the pebble — while a nuclide is processed.

use crate::engine::{self, Event, Link, Request};
use crate::model::{self, JOBS};
use crate::sim::{self, History, Stats};
use egui::{Color32, Pos2, Rect, Sense, Stroke, StrokeKind, Vec2};
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

/// `?autostart` in the page URL (or `TRISO_AUTOSTART` natively) starts the
/// run as soon as the data are ready — for unattended browser tests.
fn autostart() -> bool {
    #[cfg(target_arch = "wasm32")]
    {
        web_sys::window().and_then(|w| w.location().search().ok()).is_some_and(|s| s.contains("autostart"))
    }
    #[cfg(not(target_arch = "wasm32"))]
    {
        std::env::var_os("TRISO_AUTOSTART").is_some()
    }
}

// ─── Colours ─────────────────────────────────────────────────────────────────

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

/// Energy colour scale: 1e-3 eV (blue) to 2e7 eV (red), logarithmic.
const E_LO_LOG: f64 = -3.0;
const E_HI_LOG: f64 = 7.3;

fn energy_colour(e_ev: f64, alpha: u8) -> Color32 {
    const STOPS: [(f64, [u8; 3]); 6] = [
        (0.00, [60, 100, 255]),
        (0.25, [40, 200, 235]),
        (0.45, [90, 215, 100]),
        (0.65, [240, 215, 50]),
        (0.82, [250, 140, 40]),
        (1.00, [235, 50, 45]),
    ];
    let t = ((e_ev.max(1e-12).log10() - E_LO_LOG) / (E_HI_LOG - E_LO_LOG)).clamp(0.0, 1.0);
    let k = STOPS.iter().position(|s| s.0 >= t).unwrap_or(STOPS.len() - 1).max(1);
    let (a, b) = (STOPS[k - 1], STOPS[k]);
    let f = ((t - a.0) / (b.0 - a.0)).clamp(0.0, 1.0);
    let mix = |i: usize| (a.1[i] as f64 + f * (b.1[i] as f64 - a.1[i] as f64)).round() as u8;
    Color32::from_rgba_unmultiplied(mix(0), mix(1), mix(2), alpha)
}

fn fmt_time(t_s: f64) -> String {
    match t_s {
        t if t >= 1e-3 => format!("{:.3} ms", t * 1e3),
        t if t >= 1e-6 => format!("{:.2} µs", t * 1e6),
        t => format!("{:.1} ns", t * 1e9),
    }
}

fn fmt_energy(e: f64) -> String {
    match e {
        e if e >= 1e6 => format!("{:.3} MeV", e / 1e6),
        e if e >= 1e3 => format!("{:.3} keV", e / 1e3),
        e => format!("{:.4} eV", e),
    }
}

// ─── View transform ──────────────────────────────────────────────────────────

#[derive(Clone, Copy)]
struct View {
    /// World point (cm) at the centre of the canvas.
    centre: [f64; 2],
    /// Pixels per cm.
    scale: f64,
    /// Fitted to the canvas yet?
    fitted: bool,
}

impl View {
    fn new() -> Self {
        Self { centre: [0.0, 0.0], scale: 1.0, fitted: false }
    }
    fn fit(&mut self, rect: Rect) {
        let p = model::half_pitch();
        self.centre = [0.0, 0.0];
        self.scale = (rect.width().min(rect.height()) as f64) * 0.94 / (2.0 * p);
        self.fitted = true;
    }
    fn to_screen(&self, rect: Rect, x: f64, y: f64) -> Pos2 {
        let c = rect.center();
        Pos2::new(
            c.x + ((x - self.centre[0]) * self.scale) as f32,
            c.y - ((y - self.centre[1]) * self.scale) as f32,
        )
    }
    fn to_world(&self, rect: Rect, p: Pos2) -> [f64; 2] {
        let c = rect.center();
        [
            self.centre[0] + (p.x - c.x) as f64 / self.scale,
            self.centre[1] - (p.y - c.y) as f64 / self.scale,
        ]
    }
}

/// Draw the cell from the same particle centres the geometry was built from.
///
/// This is the interactive view. The geometry REVIEW images required by the
/// crate's drawing rule are rendered separately, from the assembled geometry,
/// by `--render-geometry` (see `render.rs`).
fn draw_cell(painter: &egui::Painter, rect: Rect, view: &View, centres: &[(f64, f64)]) {
    painter.rect_filled(rect, 0.0, Color32::from_rgb(14, 16, 20));
    let p = model::half_pitch();
    let (a, b) = (view.to_screen(rect, -p, p), view.to_screen(rect, p, -p));
    let cell = Rect::from_two_pos(a, b);
    painter.rect_filled(cell, 0.0, HELIUM);
    let s = view.scale as f32;
    let o = view.to_screen(rect, 0.0, 0.0);
    painter.circle_filled(o, model::PEBBLE_R as f32 * s, MATERIAL_COLOURS[model::MAT_SHELL]);
    painter.circle_filled(o, model::FUEL_ZONE_R as f32 * s, MATERIAL_COLOURS[model::MAT_MATRIX]);
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
    painter.rect_stroke(cell, 0.0, Stroke::new(1.5, Color32::from_rgb(120, 170, 255)), StrokeKind::Outside);
    painter.text(
        cell.left_top() + Vec2::new(6.0, 4.0),
        egui::Align2::LEFT_TOP,
        "reflective boundary  ·  helium (void)",
        egui::FontId::proportional(12.0),
        Color32::from_rgb(140, 170, 220),
    );
}

// ─── Animated history ────────────────────────────────────────────────────────

struct Anim {
    hist: History,
    /// Cumulative 3D flight distance at each state, cm.
    cum: Vec<f64>,
    shown_cm: f64,
    /// Already added to the statistics? A history is computed whole before
    /// it is animated; counting it then would show its fate while it is
    /// still in flight.
    counted: bool,
}

impl Anim {
    fn new(hist: History) -> Self {
        let st = &hist.track.states;
        let mut cum = Vec::with_capacity(st.len());
        let mut d = 0.0;
        for (i, s) in st.iter().enumerate() {
            if i > 0 {
                let q = st[i - 1].r;
                d += ((s.r.x - q.x).powi(2) + (s.r.y - q.y).powi(2) + (s.r.z - q.z).powi(2)).sqrt();
            }
            cum.push(d);
        }
        Self { hist, cum, shown_cm: 0.0, counted: false }
    }
    fn total(&self) -> f64 {
        self.cum.last().copied().unwrap_or(0.0)
    }
    fn finished(&self) -> bool {
        self.shown_cm >= self.total()
    }
    /// Index of the segment being flown, and the fraction along it.
    fn head(&self) -> (usize, f64) {
        let k = self.cum.partition_point(|&c| c <= self.shown_cm).saturating_sub(1);
        let k = k.min(self.cum.len().saturating_sub(2));
        let len = self.cum.get(k + 1).map_or(0.0, |c| c - self.cum[k]);
        let f = if len > 0.0 { ((self.shown_cm - self.cum[k]) / len).clamp(0.0, 1.0) } else { 1.0 };
        (k, f)
    }
}

/// Draw one track. `trail` is `(cumulative distances, head distance, trail
/// length)`: segments further than `trail length` behind the head fade towards
/// a floor alpha, so the neutron's recent path stays legible on top of its
/// history. A neutron flies hundreds of cm in a 7 cm cell, so without this a
/// single track covers the whole picture.
fn draw_track(
    painter: &egui::Painter,
    rect: Rect,
    view: &View,
    h: &History,
    upto: Option<(usize, f64)>,
    alpha: u8,
    dots: bool,
    trail: Option<(&[f64], f64, f64)>,
) {
    let st = &h.track.states;
    if st.len() < 2 {
        return;
    }
    let (last_seg, frac) = upto.unwrap_or((st.len() - 2, 1.0));
    let w = if alpha == 255 { 2.0 } else { 1.2 };
    for k in 0..=last_seg.min(st.len() - 2) {
        let (a, b) = (&st[k], &st[k + 1]);
        let f = if k == last_seg { frac } else { 1.0 };
        let end = [a.r.x + f * (b.r.x - a.r.x), a.r.y + f * (b.r.y - a.r.y)];
        let seg_alpha = match trail {
            Some((cum, head, len)) if len.is_finite() => {
                let behind = head - cum.get(k + 1).copied().unwrap_or(head);
                let t = (1.0 - behind / len).clamp(0.0, 1.0);
                (35.0 + (alpha as f64 - 35.0) * t) as u8
            }
            _ => alpha,
        };
        let col = energy_colour(a.energy, seg_alpha);
        painter.line_segment([view.to_screen(rect, a.r.x, a.r.y), view.to_screen(rect, end[0], end[1])], Stroke::new(w, col));
        if dots && k > 0 && a.event == TrackEvent::Scatter {
            painter.circle_filled(view.to_screen(rect, a.r.x, a.r.y), 1.6, Color32::from_white_alpha(seg_alpha / 2));
        }
    }
    let birth = view.to_screen(rect, st[0].r.x, st[0].r.y);
    painter.circle_stroke(birth, 5.0, Stroke::new(1.5, Color32::from_rgba_unmultiplied(90, 230, 120, alpha)));
    if upto.is_none() || (last_seg >= st.len() - 2 && frac >= 1.0) {
        let e = st.last().unwrap();
        let p = view.to_screen(rect, e.r.x, e.r.y);
        match h.outcome {
            Some(TrackEvent::Fission) => {
                painter.circle_filled(p, 5.0, Color32::from_rgba_unmultiplied(255, 230, 80, alpha));
                painter.circle_stroke(p, 9.0, Stroke::new(1.5, Color32::from_rgba_unmultiplied(255, 230, 80, alpha)));
            }
            _ => {
                let s = Stroke::new(2.0, Color32::from_rgba_unmultiplied(255, 70, 70, alpha));
                painter.line_segment([p + Vec2::new(-5.0, -5.0), p + Vec2::new(5.0, 5.0)], s);
                painter.line_segment([p + Vec2::new(-5.0, 5.0), p + Vec2::new(5.0, -5.0)], s);
            }
        }
    } else if let Some((k, f)) = upto {
        let (a, b) = (&st[k], &st[k + 1]);
        let p = view.to_screen(rect, a.r.x + f * (b.r.x - a.r.x), a.r.y + f * (b.r.y - a.r.y));
        painter.circle_filled(p, 4.0, Color32::WHITE);
    }
}

// ─── Loading ─────────────────────────────────────────────────────────────────

/// Rough cost of each job in seconds, measured in headless Chromium on
/// 2026-10-03, used ONLY to weight the progress bar: the jobs differ by two
/// orders of magnitude, so counting them would make the bar jump.
const JOB_WEIGHT_S: [f64; 11] = [35.1, 28.5, 0.2, 0.1, 0.2, 0.1, 0.1, 0.3, 0.3, 0.3, 21.5];

struct Loading {
    started: f64,
    job_started: Vec<Option<f64>>,
    job_secs: Vec<Option<f64>>,
}

impl Loading {
    fn new() -> Self {
        Self { started: now_s(), job_started: vec![None; JOBS.len()], job_secs: vec![None; JOBS.len()] }
    }
    fn done(&self) -> usize {
        self.job_secs.iter().filter(|s| s.is_some()).count()
    }
    /// The job running now, if any.
    fn current(&self) -> Option<usize> {
        (0..JOBS.len()).find(|&i| self.job_started[i].is_some() && self.job_secs[i].is_none())
    }
    /// A rough fraction: finished jobs by weight, plus the running job's
    /// elapsed share of its expected time, capped short of done.
    fn fraction(&self, now: f64) -> f32 {
        let total: f64 = JOB_WEIGHT_S.iter().sum();
        let mut f: f64 = (0..JOBS.len()).filter(|&i| self.job_secs[i].is_some()).map(|i| JOB_WEIGHT_S[i]).sum();
        if let Some(i) = self.current() {
            let t = now - self.job_started[i].unwrap_or(now);
            f += JOB_WEIGHT_S[i] * (t / JOB_WEIGHT_S[i].max(0.05)).min(0.95);
        }
        (f / total).clamp(0.0, 1.0) as f32
    }
    fn status_line(&self) -> String {
        match self.current() {
            Some(i) => format!("{} ({} of {})", JOBS[i].label, i + 1, JOBS.len()),
            None if self.done() == 0 => "Downloading ENDF tapes".into(),
            None => "Assembling the model".into(),
        }
    }
}

// ─── Running ─────────────────────────────────────────────────────────────────

/// Animated neutrons kept in hand while running, so the animation never waits
/// on transport.
const PREFETCH: usize = 3;

struct Running {
    /// Histories received and not yet shown, in chain order.
    queue: VecDeque<History>,
    /// Animated neutrons requested from the engine and not yet received.
    outstanding: usize,
    current: Option<Anim>,
    past: VecDeque<History>,
    stats: Stats,
    running: bool,
    /// Animate the current neutron to the end while stopped ("Next").
    single: bool,
    /// "Next" was pressed before its neutron had arrived.
    want_single: bool,
    speed_cm_s: f64,
    keep: usize,
    dots: bool,
    /// Length of bright trail behind the neutron, cm; `None` draws it all.
    trail_cm: Option<f64>,
}

/// The `outram-mc-libs` deep dive this demo belongs to (gh:#514), on the
/// backend's GitHub Pages site. Absolute so it also works from the native build.
const DEEP_DIVE_URL: &str = "https://theodoreonzgit.github.io/outram-park-backend/deep-dives/monte-carlo/";

impl Running {
    fn new(link: &Link) -> Self {
        link.send(Request::Run { n: PREFETCH, animate: true });
        Self {
            queue: VecDeque::new(),
            outstanding: PREFETCH,
            current: None,
            past: VecDeque::new(),
            stats: Stats::default(),
            running: autostart(),
            single: false,
            want_single: false,
            speed_cm_s: 7.0,
            keep: 0,
            dots: true,
            trail_cm: Some(80.0),
        }
    }
    fn retire(&mut self, h: History) {
        self.past.push_back(h);
        while self.past.len() > self.keep {
            self.past.pop_front();
        }
    }
    /// Show the next queued neutron. `false` if none has arrived yet.
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
    /// Count the current neutron once its animation reaches the end.
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

// ─── The app ─────────────────────────────────────────────────────────────────

#[allow(clippy::large_enum_variant)] // held by value: the workspace forbids `Box<T>`
enum Phase {
    Loading(Loading),
    Ready(Running),
    Failed(String),
}

pub struct TrisoApp {
    link: Option<Link>,
    phase: Phase,
    centres: Vec<(f64, f64)>,
    view: View,
    title: String,
    load_timings: Vec<(&'static str, f64)>,
    load_total_s: f64,
    panel_open: bool,
    /// The panel's initial state is decided on the first frame, from the
    /// screen width: collapsed on a phone, so the pebble gets the screen.
    panel_decided: bool,
}

impl TrisoApp {
    pub fn new(cc: &eframe::CreationContext<'_>) -> Self {
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
        let (link, phase) = match link {
            Ok(l) => (Some(l), Phase::Loading(Loading::new())),
            Err(e) => (None, Phase::Failed(format!("could not start the physics worker: {e}"))),
        };
        Self {
            link,
            phase,
            centres: model::particle_centres(model::LAYOUT_SEED),
            view: View::new(),
            title: String::new(),
            load_timings: Vec::new(),
            load_total_s: 0.0,
            panel_open: true,
            panel_decided: false,
        }
    }

    fn handle(&mut self, events: Vec<Event>) {
        for e in events {
            match (&mut self.phase, e) {
                (_, Event::Error(m)) => self.phase = Phase::Failed(m),
                (Phase::Loading(l), Event::JobStarted { index }) => l.job_started[index] = Some(now_s()),
                (Phase::Loading(l), Event::JobDone { index, secs }) => l.job_secs[index] = Some(secs),
                (Phase::Loading(l), Event::Ready) => {
                    self.load_timings =
                        JOBS.iter().zip(&l.job_secs).map(|(j, s)| (j.label, s.unwrap_or(0.0))).collect();
                    self.load_total_s = now_s() - l.started;
                    if let Some(link) = &self.link {
                        self.phase = Phase::Ready(Running::new(link));
                    }
                }
                (Phase::Ready(r), Event::History { h, animate }) => r.receive(h, animate),
                _ => {}
            }
        }
    }

    fn title_now(&mut self, ctx: &egui::Context) {
        let t = match &self.phase {
            Phase::Loading(l) => format!(
                "TRISO pebble · loading {}/{} · {}",
                l.done(),
                JOBS.len(),
                l.status_line()
            ),
            Phase::Ready(r) => format!(
                "TRISO pebble · ready · {} neutrons · {} fission · {} capture · {} other",
                r.stats.histories, r.stats.fissions, r.stats.captures, r.stats.other
            ),
            Phase::Failed(e) => format!("TRISO pebble · FAILED · {e}"),
        };
        if t != self.title {
            set_title(ctx, &t);
            self.title = t;
        }
    }
}

impl eframe::App for TrisoApp {
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
        if let (Phase::Ready(r), Some(link)) = (&mut self.phase, &self.link) {
            r.top_up(link);
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
        egui::Panel::left("controls").default_size(330.0).resizable(true).show_collapsible(ui, &mut open, |ui| {
            egui::ScrollArea::vertical().show(ui, |ui| self.side_panel(ui));
        });
        if self.panel_open == before {
            self.panel_open = open;
        }
        egui::CentralPanel::default().frame(egui::Frame::NONE).show(ui, |ui| self.canvas(ui));
    }
}

impl TrisoApp {
    fn loading_panel(ui: &mut egui::Ui, l: &Loading) {
        let now = now_s();
        ui.heading("Simulation loading…");
        ui.horizontal(|ui| {
            ui.spinner();
            ui.strong(format!("Processing ENDF files · {}", l.status_line()));
        });
        ui.add(egui::ProgressBar::new(l.fraction(now)).show_percentage().text(format!("rough · {:.0} s", now - l.started)));
        ui.label("About 11 MB of ENDF tapes, covariance data removed.");
        ui.label(
            "Every cross section is reconstructed from evaluated ENDF/B-VIII.0 data by OUTRAM PARK's own \
             NJOY port (RECONR + BROADR, tolerance 0.01), off the page's main thread, so the page stays \
             live. U-235, U-238 and graphite thermal scattering take the longest.",
        );
        ui.add_space(6.0);
        egui::Grid::new("jobs").num_columns(2).spacing([16.0, 2.0]).show(ui, |ui| {
            for (i, job) in JOBS.iter().enumerate() {
                ui.label(job.label);
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

    fn side_panel(&mut self, ui: &mut egui::Ui) {
        ui.horizontal(|ui| {
            ui.heading("TRISO pebble");
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                if ui.button("« Hide").on_hover_text("Fold the panel away to see the whole pebble").clicked() {
                    self.panel_open = false;
                }
            });
        });
        ui.label("One neutron at a time, on real ENDF/B-VIII.0 data.");
        ui.add(
            egui::Hyperlink::from_label_and_url("How the Monte Carlo code works: the neutronics deep dive", DEEP_DIVE_URL)
                .open_in_new_tab(true),
        );
        ui.separator();
        let link = self.link.as_ref();
        match &mut self.phase {
            Phase::Loading(l) => Self::loading_panel(ui, l),
            Phase::Failed(e) => {
                ui.colored_label(Color32::from_rgb(255, 90, 90), format!("Failed: {e}"));
            }
            Phase::Ready(r) => {
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
                ui.horizontal(|ui| {
                    if ui.add_enabled(!r.running, egui::Button::new("Run 100 unanimated")).clicked() {
                        if let Some(link) = link {
                            link.send(Request::Run { n: 100, animate: false });
                        }
                    }
                });
                ui.add(egui::Slider::new(&mut r.speed_cm_s, 1.0..=20_000.0).logarithmic(true).text("cm of flight / s"));
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
                ui.weak("Scroll or pinch to zoom (to see the TRISO layers), drag to pan, double-click to fit.");

                ui.separator();
                ui.strong("This neutron");
                if let Some(a) = &r.current {
                    let h = &a.hist;
                    let (k, _) = a.head();
                    let s = &h.track.states[k.min(h.track.states.len() - 1)];
                    let scatters = h.track.states[..=k.min(h.track.states.len() - 1)]
                        .iter()
                        .filter(|s| s.event == TrackEvent::Scatter)
                        .count();
                    let born = if h.from_fission { "at the last neutron's fission site" } else { "in a random kernel" };
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
                        ui.label("in");
                        ui.label(s.material.map_or("helium (void)", |m| model::MATERIAL_NAMES[m]));
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
                                o => sim::outcome_name(o).to_string(),
                            });
                            ui.end_row();
                        }
                    });
                } else {
                    ui.weak("Press Start.");
                }

                ui.separator();
                let st = &r.stats;
                ui.strong(format!("All {} neutrons", st.histories));
                if st.histories > 0 {
                    let n = st.histories as f64;
                    let pct = |x: u64| format!("{:.0} %", 100.0 * x as f64 / n);
                    egui::Grid::new("tot").num_columns(2).show(ui, |ui| {
                        ui.label("ended in fission");
                        ui.label(pct(st.fissions));
                        ui.end_row();
                        ui.label("captured");
                        ui.label(pct(st.captures));
                        ui.end_row();
                        if st.other > 0 {
                            ui.colored_label(Color32::from_rgb(255, 90, 90), "other (a defect)");
                            ui.label(st.other.to_string());
                            ui.end_row();
                        }
                        ui.label("reached thermal (< 0.625 eV)");
                        ui.label(pct(st.thermalised));
                        ui.end_row();
                        ui.label("mean scatters");
                        ui.label(format!("{:.0}", st.scatters as f64 / n));
                        ui.end_row();
                        ui.label("mean distance flown");
                        ui.label(format!("{:.0} cm", st.path_cm / n));
                        ui.end_row();
                    });
                    ui.weak("Counts from a handful of histories, not converged statistics.");
                }
            }
        }

        ui.separator();
        ui.strong("Neutron energy");
        let (rect, _) = ui.allocate_exact_size(Vec2::new(ui.available_width().min(300.0), 14.0), Sense::hover());
        let n = 80;
        for i in 0..n {
            let t = i as f64 / n as f64;
            let e = 10f64.powf(E_LO_LOG + t * (E_HI_LOG - E_LO_LOG));
            let x0 = rect.left() + rect.width() * i as f32 / n as f32;
            let x1 = rect.left() + rect.width() * (i + 1) as f32 / n as f32;
            ui.painter().rect_filled(Rect::from_x_y_ranges(x0..=x1, rect.y_range()), 0.0, energy_colour(e, 255));
        }
        ui.horizontal(|ui| {
            ui.weak("1 meV · thermal");
            ui.add_space(20.0);
            ui.weak("1 keV");
            ui.add_space(20.0);
            ui.weak("fast · 20 MeV");
        });
        ui.add_space(4.0);
        ui.strong("Materials");
        for (i, name) in model::MATERIAL_NAMES.iter().enumerate() {
            if i == model::MAT_OPYC {
                continue; // same colour and composition as inner PyC
            }
            ui.horizontal(|ui| {
                let (r, _) = ui.allocate_exact_size(Vec2::splat(12.0), Sense::hover());
                ui.painter().rect_filled(r, 2.0, MATERIAL_COLOURS[i]);
                ui.label(if i == model::MAT_IPYC { "Pyrolytic carbon (inner & outer)" } else { name });
            });
        }
        ui.horizontal(|ui| {
            let (r, _) = ui.allocate_exact_size(Vec2::splat(12.0), Sense::hover());
            ui.painter().rect_filled(r, 2.0, HELIUM);
            ui.label("Helium coolant (modelled as void)");
        });
        ui.horizontal(|ui| {
            let (r, _) = ui.allocate_exact_size(Vec2::new(16.0, 16.0), Sense::hover());
            ui.painter().circle_stroke(r.center(), 5.0, Stroke::new(1.5, Color32::from_rgb(90, 230, 120)));
            ui.label("birth");
            let (r, _) = ui.allocate_exact_size(Vec2::new(22.0, 16.0), Sense::hover());
            ui.painter().circle_filled(r.center(), 4.0, Color32::from_rgb(255, 230, 80));
            ui.painter().circle_stroke(r.center(), 7.0, Stroke::new(1.2, Color32::from_rgb(255, 230, 80)));
            ui.label("fission");
            let (r, _) = ui.allocate_exact_size(Vec2::new(16.0, 16.0), Sense::hover());
            let (c, s) = (r.center(), Stroke::new(2.0, Color32::from_rgb(255, 70, 70)));
            ui.painter().line_segment([c + Vec2::new(-4.0, -4.0), c + Vec2::new(4.0, 4.0)], s);
            ui.painter().line_segment([c + Vec2::new(-4.0, 4.0), c + Vec2::new(4.0, -4.0)], s);
            ui.label("capture");
        });

        ui.separator();
        egui::CollapsingHeader::new("What this is — and is not").default_open(false).show(ui, |ui| {
            for line in [
                "Transport: outram-mc-libs continuous-energy Monte Carlo, unmodified. Each neutron is a one-particle fixed-source run with fission progeny switched off, so every track is one real history, drawn projected onto the slice.",
                "Data: ENDF/B-VIII.0 (U-235, U-238, O-16, B-10, B-11, C-12, C-13, Si-28/29/30), reconstructed and Doppler-broadened to 296 K in this browser by OUTRAM PARK's NJOY port, in a background worker. Graphite carbon uses the crystalline-graphite S(α,β) thermal-scattering law.",
                "Covariance data (ENDF files 30–40) are removed before download: transport never reads them. A test proves the stripped tapes give bit-identical cross sections and fission spectra.",
                "Low fidelity, deliberately: reconstruction tolerance 0.01, not NJOY's 0.001.",
                "Geometry: HTR-10 pebble dimensions (IAEA-TECDOC-1382) in 2D. The TRISO particles are therefore infinitely long rods, not spheres: 152 of them, so the fuel fraction of the fuelled zone matches the real pebble (5.0 %). Rods self-shield differently from spheres, so this is a picture of how neutrons move, not a model of HTR-10.",
                "Approximations: carbon in the thin SiC layer is free gas; every fission's next neutron takes the U-235 fission spectrum; helium is void.",
                "Education and research only.",
            ] {
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

    fn canvas(&mut self, ui: &mut egui::Ui) {
        let (resp, painter) = ui.allocate_painter(ui.available_size(), Sense::click_and_drag());
        let rect = resp.rect;
        if !self.view.fitted || resp.double_clicked() {
            self.view.fit(rect);
        }
        // Zoom about the pointer (wheel or pinch); drag to pan.
        if resp.hovered() {
            let (scroll, zoom) = ui.input(|i| (i.smooth_scroll_delta.y, i.zoom_delta()));
            let factor = zoom as f64 * (scroll as f64 * 0.0025).exp();
            if (factor - 1.0).abs() > 1e-6 {
                if let Some(p) = resp.hover_pos() {
                    let before = self.view.to_world(rect, p);
                    let fit = (rect.width().min(rect.height()) as f64) * 0.94 / (2.0 * model::half_pitch());
                    self.view.scale = (self.view.scale * factor).clamp(fit * 0.5, fit * 600.0);
                    let after = self.view.to_world(rect, p);
                    self.view.centre[0] += before[0] - after[0];
                    self.view.centre[1] += before[1] - after[1];
                }
            }
        }
        if resp.dragged() {
            let d = resp.drag_delta();
            self.view.centre[0] -= d.x as f64 / self.view.scale;
            self.view.centre[1] += d.y as f64 / self.view.scale;
        }

        draw_cell(&painter, rect, &self.view, &self.centres);

        match &mut self.phase {
            Phase::Loading(l) => Self::loading_card(&painter, rect, l),
            Phase::Failed(e) => {
                painter.text(rect.center(), egui::Align2::CENTER_CENTER, format!("Failed: {e}"), egui::FontId::proportional(15.0), Color32::from_rgb(255, 110, 110));
            }
            Phase::Ready(r) => {
                let dt = ui.input(|i| i.stable_dt).min(0.1) as f64;
                if r.running || r.single {
                    match &mut r.current {
                        Some(a) if !a.finished() => a.shown_cm += r.speed_cm_s * dt,
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
                    draw_track(&painter, rect, &self.view, h, None, alpha, false, None);
                }
                if let Some(a) = &r.current {
                    let upto = if a.finished() { None } else { Some(a.head()) };
                    let trail = r.trail_cm.map(|len| (a.cum.as_slice(), a.shown_cm.min(a.total()), len));
                    draw_track(&painter, rect, &self.view, &a.hist, upto, 255, r.dots, trail);
                }
            }
        }

        // Scale bar: the smallest round length that is at least 80 px long.
        let bar_cm = [0.001, 0.002, 0.005, 0.01, 0.02, 0.05, 0.1, 0.2, 0.5, 1.0, 2.0]
            .into_iter()
            .find(|&c| c * self.view.scale >= 80.0)
            .unwrap_or(2.0);
        let x0 = rect.left() + 16.0;
        let y0 = rect.bottom() - 18.0;
        let x1 = x0 + (bar_cm * self.view.scale) as f32;
        painter.line_segment([Pos2::new(x0, y0), Pos2::new(x1, y0)], Stroke::new(2.0, Color32::WHITE));
        let label = if bar_cm < 0.1 { format!("{:.0} µm", bar_cm * 1e4) } else { format!("{bar_cm} cm") };
        painter.text(Pos2::new(x0, y0 - 4.0), egui::Align2::LEFT_BOTTOM, label, egui::FontId::proportional(12.0), Color32::WHITE);

        // With the panel folded away, a real button brings it back: the
        // panel's own drag handle is a few pixels wide and hard to hit on a
        // phone. (While it is open, its "« Hide" button folds it.)
        if !self.panel_open {
            let b = Rect::from_min_size(rect.left_top() + Vec2::new(8.0, 8.0), Vec2::new(104.0, 32.0));
            if ui.put(b, egui::Button::new("Controls »")).clicked() {
                self.panel_open = true;
            }
        }
    }

    /// The loading progress, drawn on the canvas so it shows even with the
    /// panel collapsed (the phone layout).
    fn loading_card(painter: &egui::Painter, rect: Rect, l: &Loading) {
        let now = now_s();
        let w = (rect.width() - 32.0).min(420.0);
        let card = Rect::from_center_size(rect.center(), Vec2::new(w, 112.0));
        painter.rect_filled(card, 8.0, Color32::from_rgba_unmultiplied(14, 16, 22, 235));
        painter.rect_stroke(card, 8.0, Stroke::new(1.0, Color32::from_rgb(70, 80, 100)), StrokeKind::Inside);
        let f = |s: f32| egui::FontId::proportional(s);
        let left = card.left() + 16.0;
        painter.text(Pos2::new(left, card.top() + 14.0), egui::Align2::LEFT_TOP, "Simulation loading…", f(17.0), Color32::WHITE);
        painter.text(
            Pos2::new(left, card.top() + 40.0),
            egui::Align2::LEFT_TOP,
            format!("Processing ENDF files · {}", l.status_line()),
            f(13.0),
            Color32::from_rgb(200, 205, 215),
        );
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
    }
}
