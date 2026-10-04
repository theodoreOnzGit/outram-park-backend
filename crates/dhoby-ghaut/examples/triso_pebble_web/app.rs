//! The egui front end: loading screen, the pebble, and the animated track.
//!
//! Loading processes one ENDF tape per frame. Each job is a single blocking
//! call (U-235 is the longest), so the page cannot repaint DURING a job; it
//! repaints between them. A frame that is about to block first draws "now
//! processing X" and only starts the job on the NEXT frame, so the label the
//! user sees is always the job that is running.

use crate::model::{self, DataBuilder, JOBS};
use crate::sim::{self, Chain, History, Physics, Stats};
use egui::{Color32, Pos2, Rect, Sense, Stroke, StrokeKind, Vec2};
use outram_mc_libs::physics::track_output::TrackEvent;
use std::collections::VecDeque;

// ─── Where tape bytes come from ──────────────────────────────────────────────

#[cfg(target_arch = "wasm32")]
#[derive(Default)]
pub struct WebTapes {
    ready: std::collections::HashMap<&'static str, Result<Vec<u8>, String>>,
    fetched: usize,
    downloaded_bytes: usize,
}

pub enum Source {
    #[cfg(not(target_arch = "wasm32"))]
    Native,
    #[cfg(target_arch = "wasm32")]
    Web(std::sync::Arc<std::sync::RwLock<WebTapes>>),
}

impl Source {
    #[cfg(not(target_arch = "wasm32"))]
    pub fn native() -> Self {
        Source::Native
    }

    /// Start downloading every tape at once, in the background.
    ///
    /// All requests are issued before the first (blocking) nuclear-data job
    /// can start, so the transfers proceed while U-235 is being processed.
    /// Issued one after another instead, each download would wait for the
    /// previous nuclide's processing to finish.
    #[cfg(target_arch = "wasm32")]
    pub fn web(ctx: &egui::Context) -> Self {
        let store = std::sync::Arc::new(std::sync::RwLock::new(WebTapes::default()));
        for job in JOBS {
            let (s, ctx) = (store.clone(), ctx.clone());
            wasm_bindgen_futures::spawn_local(async move {
                let url = format!("data/{}", model::wire_name(job.tape));
                let got = fetch_bytes(&url).await.and_then(|z| model::decompress(&z).map(|b| (z.len(), b)));
                if let Ok(mut st) = s.write() {
                    match got {
                        Ok((n, b)) => {
                            st.downloaded_bytes += n;
                            st.ready.insert(job.tape, Ok(b));
                        }
                        Err(e) => {
                            st.ready.insert(job.tape, Err(e));
                        }
                    }
                    st.fetched += 1;
                }
                ctx.request_repaint();
            });
        }
        Source::Web(store)
    }

    fn has(&self, tape: &str) -> bool {
        match self {
            #[cfg(not(target_arch = "wasm32"))]
            Source::Native => {
                let _ = tape; // natively every tape is on disk
                true
            }
            #[cfg(target_arch = "wasm32")]
            Source::Web(s) => s.read().is_ok_and(|s| s.ready.contains_key(tape)),
        }
    }

    fn take(&self, tape: &'static str) -> Option<Result<Vec<u8>, String>> {
        match self {
            #[cfg(not(target_arch = "wasm32"))]
            Source::Native => Some(crate::native_tape(tape)),
            #[cfg(target_arch = "wasm32")]
            Source::Web(s) => s.write().ok().and_then(|mut s| s.ready.remove(tape)),
        }
    }

    /// `(tapes downloaded, MB downloaded)`, or `None` when there is no download.
    fn download_status(&self) -> Option<(usize, f64)> {
        match self {
            #[cfg(not(target_arch = "wasm32"))]
            Source::Native => None,
            #[cfg(target_arch = "wasm32")]
            Source::Web(s) => {
                let s = s.read().ok()?;
                Some((s.fetched, s.downloaded_bytes as f64 / 1.0e6))
            }
        }
    }
}

#[cfg(target_arch = "wasm32")]
async fn fetch_bytes(url: &str) -> Result<Vec<u8>, String> {
    use wasm_bindgen::JsCast as _;
    use wasm_bindgen_futures::JsFuture;
    let err = |e: wasm_bindgen::JsValue| format!("{url}: {e:?}");
    let window = web_sys::window().ok_or("no window")?;
    let resp: web_sys::Response = JsFuture::from(window.fetch_with_str(url)).await.map_err(err)?.dyn_into().map_err(err)?;
    if !resp.ok() {
        return Err(format!("{url}: HTTP {}", resp.status()));
    }
    let buf = JsFuture::from(resp.array_buffer().map_err(err)?).await.map_err(err)?;
    Ok(js_sys::Uint8Array::new(&buf).to_vec())
}

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

// ─── The app ─────────────────────────────────────────────────────────────────

struct Running {
    phys: Physics,
    chain: Chain,
    current: Option<Anim>,
    past: VecDeque<History>,
    stats: Stats,
    running: bool,
    /// Animate the current neutron to the end even while stopped ("Next").
    single: bool,
    speed_cm_s: f64,
    keep: usize,
    dots: bool,
    /// Length of bright trail behind the neutron, cm; `None` draws it all.
    trail_cm: Option<f64>,
}

impl Running {
    fn start_next(&mut self) {
        if let Some(a) = self.current.take() {
            if !a.counted {
                self.stats.add(&a.hist); // skipped past before it finished
            }
            self.retire(a.hist);
        }
        self.current = Some(Anim::new(self.chain.run_next(&self.phys)));
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
    fn retire(&mut self, h: History) {
        self.past.push_back(h);
        while self.past.len() > self.keep {
            self.past.pop_front();
        }
    }
}

#[allow(clippy::large_enum_variant)] // see `Ready`
enum Phase {
    Loading { builder: DataBuilder, armed: bool, timings: Vec<(&'static str, f64)>, started: f64 },
    // Held by value: the workspace forbids `Box<T>` outside recursive types.
    Ready(Running),
    Failed(String),
}

pub struct TrisoApp {
    source: Source,
    phase: Phase,
    centres: Vec<(f64, f64)>,
    view: View,
    title: String,
    load_timings: Vec<(&'static str, f64)>,
    load_total_s: f64,
}

impl TrisoApp {
    pub fn new(cc: &eframe::CreationContext<'_>, source: Source) -> Self {
        // The energy colours are chosen against a dark ground; keep the panel
        // dark too rather than follow a light system theme.
        cc.egui_ctx.set_theme(egui::ThemePreference::Dark);
        Self {
            source,
            phase: Phase::Loading { builder: DataBuilder::default(), armed: false, timings: Vec::new(), started: now_s() },
            centres: model::particle_centres(model::LAYOUT_SEED),
            view: View::new(),
            title: String::new(),
            load_timings: Vec::new(),
            load_total_s: 0.0,
        }
    }

    fn title_now(&mut self, ctx: &egui::Context, t: String) {
        if t != self.title {
            set_title(ctx, &t);
            self.title = t;
        }
    }

    /// Advance loading by at most one job. Returns the next phase if it changed.
    fn step_loading(&mut self, ctx: &egui::Context) -> Option<Phase> {
        let Phase::Loading { builder, armed, timings, started } = &mut self.phase else { return None };
        let Some(job) = builder.next_job() else {
            let b = std::mem::take(builder);
            self.load_timings = std::mem::take(timings);
            self.load_total_s = now_s() - *started;
            return Some(match b.finish() {
                Ok(data) => Phase::Ready(Running {
                    phys: Physics::new(data),
                    chain: Chain::new(1),
                    current: None,
                    past: VecDeque::new(),
                    stats: Stats::default(),
                    running: autostart(),
                    single: false,
                    speed_cm_s: 60.0,
                    keep: 1,
                    dots: true,
                    trail_cm: Some(80.0),
                }),
                Err(e) => Phase::Failed(e),
            });
        };
        if !self.source.has(job.tape) {
            ctx.request_repaint_after(std::time::Duration::from_millis(100));
            return None;
        }
        if !*armed {
            *armed = true; // draw "processing <job>" this frame, run it next frame
            ctx.request_repaint();
            return None;
        }
        let t = now_s();
        let result = match self.source.take(job.tape) {
            Some(Ok(bytes)) => builder.step(&bytes),
            Some(Err(e)) => Err(e),
            None => return None,
        };
        *armed = false;
        if let Err(e) = result {
            return Some(Phase::Failed(format!("{}: {e}", job.label)));
        }
        timings.push((job.label, now_s() - t));
        ctx.request_repaint();
        None
    }

    fn loading_panel(&self, ui: &mut egui::Ui) {
        let Phase::Loading { builder, armed, timings, started } = &self.phase else { return };
        let (done, total) = builder.progress();
        ui.heading("Processing ENDF/B-VIII.0 in your browser");
        ui.label(
            "Every cross section is reconstructed here, from the evaluated nuclear data, by \
             OUTRAM PARK's own NJOY port (RECONR + BROADR, tolerance 0.01). Each nuclide is one \
             long computation: the page cannot redraw while it runs, so it may look frozen — \
             U-235 and U-238 take the longest.",
        );
        ui.add_space(6.0);
        ui.add(egui::ProgressBar::new(done as f32 / total as f32).text(format!("{done} / {total} nuclear-data jobs")));
        if let Some((n, mb)) = self.source.download_status() {
            ui.label(format!("Downloaded {n} / {total} tapes ({mb:.1} MB, covariance data removed)"));
        }
        ui.label(format!("Elapsed {:.0} s", now_s() - started));
        ui.add_space(6.0);
        egui::Grid::new("jobs").num_columns(2).spacing([16.0, 2.0]).show(ui, |ui| {
            for (i, job) in JOBS.iter().enumerate() {
                ui.label(job.label);
                if let Some((_, s)) = timings.get(i) {
                    ui.label(format!("done in {s:.1} s"));
                } else if i == done && *armed {
                    ui.colored_label(Color32::from_rgb(250, 200, 80), "processing… (page busy)");
                } else if i == done && self.source.has(job.tape) {
                    ui.label("next");
                } else if !self.source.has(job.tape) && i >= done {
                    ui.weak("downloading");
                } else {
                    ui.weak("queued");
                }
                ui.end_row();
            }
        });
    }
}

impl eframe::App for TrisoApp {
    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        let ctx = ui.ctx().clone();
        if let Some(next) = self.step_loading(&ctx) {
            self.phase = next;
        }

        // Title doubles as a status line an unattended browser test can read.
        let title = match &self.phase {
            Phase::Loading { builder, armed, .. } => {
                let (d, t) = builder.progress();
                let label = builder.next_job().map_or("", |j| j.label);
                if *armed { format!("TRISO pebble · processing {label} ({}/{t})", d + 1) } else { format!("TRISO pebble · loading {d}/{t}") }
            }
            Phase::Ready(r) => format!(
                "TRISO pebble · ready · {} neutrons · {} fission · {} capture · {} other",
                r.stats.histories, r.stats.fissions, r.stats.captures, r.stats.other
            ),
            Phase::Failed(e) => format!("TRISO pebble · FAILED · {e}"),
        };
        self.title_now(&ctx, title);

        egui::Panel::left("controls").default_size(330.0).resizable(true).show(ui, |ui| {
            egui::ScrollArea::vertical().show(ui, |ui| self.side_panel(ui));
        });
        egui::CentralPanel::default().frame(egui::Frame::NONE).show(ui, |ui| self.canvas(ui));
    }
}

impl TrisoApp {
    fn side_panel(&mut self, ui: &mut egui::Ui) {
        ui.heading("TRISO pebble");
        ui.label("One neutron at a time, on real ENDF/B-VIII.0 data.");
        ui.separator();
        match &mut self.phase {
            Phase::Loading { .. } => self.loading_panel(ui),
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
                        r.start_next();
                        r.single = true;
                    }
                    if ui.button("Clear").clicked() {
                        r.past.clear();
                    }
                });
                ui.horizontal(|ui| {
                    if ui.add_enabled(!r.running, egui::Button::new("Run 100 unanimated")).clicked() {
                        for _ in 0..100 {
                            let h = r.chain.run_next(&r.phys);
                            r.stats.add(&h);
                            r.retire(h);
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
                ui.weak("Scroll to zoom (to see the TRISO layers), drag to pan, double-click to fit.");

                ui.separator();
                ui.strong("This neutron");
                if let Some(a) = &r.current {
                    let h = &a.hist;
                    let (k, _) = a.head();
                    let s = &h.track.states[k.min(h.track.states.len() - 1)];
                    let scatters = h.track.states[..=k.min(h.track.states.len() - 1)].iter().filter(|s| s.event == TrackEvent::Scatter).count();
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
                "Transport: outram-mc-libs continuous-energy Monte Carlo, unmodified. Each neutron is a one-particle fixed-source run with fission progeny switched off, so every track is one real history.",
                "Data: ENDF/B-VIII.0 (U-235, U-238, O-16, B-10, B-11, C-12, C-13, Si-28/29/30), reconstructed and Doppler-broadened to 296 K in this browser by OUTRAM PARK's NJOY port. Graphite carbon uses the crystalline-graphite S(α,β) thermal-scattering law.",
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
        // Zoom about the pointer; drag to pan.
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

        let Phase::Ready(r) = &mut self.phase else { return };
        let dt = ui.input(|i| i.stable_dt).min(0.1) as f64;
        if r.running || r.single {
            match &mut r.current {
                Some(a) if !a.finished() => a.shown_cm += r.speed_cm_s * dt,
                Some(_) if r.single => r.single = false,
                _ if r.running => r.start_next(),
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
    }
}
