//! The twin's page: four views of one design slice, drawn by hand (no
//! egui_plot, as `helical_sg_web`), with the side panel's controls. The UI
//! thread only draws: every 1-D evaluation runs in [`crate::engine`], and
//! every calibrated number is read from the committed artefacts
//! ([`crate::artefacts`]).
//!
//! - **Twin:** UA against Re_hot at the slice; the toggle shows the
//!   uncalibrated 1-D, the calibrated twin (KOH η + δ with its 95 %
//!   interval, and the posterior-mean θ *without* δ, which over-corrects),
//!   or the CFD design points.
//! - **δ(x):** the KOH discrepancy on ln UA over (Re_hot, Re_cold).
//! - **Sweep:** co-kriging (MFK) ±2σ beside KOH and the CFD.
//! - **Filtering:** gh:#1191's committed UKF traces of a drifting fouling
//!   resistance, P-A (closures known) against P-B (textbook closures: the
//!   "confidently wrong" case).

use crate::artefacts::{self, Artefacts, CfdPoint, GridRow, TraceStep, K_LEVELS, L_LEVELS, N_RE};
use crate::engine::{Event, Operating, Point, Request, Slice};
use bugis::web_demo::link::Link;
use bugis::web_demo::panel::Panel;
use bugis::web_demo::platform::{now_s, set_title};
use bugis::web_demo::view::{apply_zoom, zoom_buttons, View};
use egui::{Color32, Pos2, Rect, RichText, Sense, Shape, Stroke, StrokeKind, Vec2};

type TLink = Link<Request, Event>;

/// The V&V records the panel links.
pub const RECORD_URL: &str = "https://github.com/theodoreOnzGit/outram-park-backend/blob/develop/crates/outram-park-digital-twin-engine/verification_and_validation/double_pipe_twin_web_gh1182/README.md";
pub const KOH_URL: &str = "https://github.com/theodoreOnzGit/outram-park-backend/blob/develop/crates/outram-park-digital-twin-engine/verification_and_validation/double_pipe_koh_gh1180.md";
pub const FILTER_URL: &str = "https://github.com/theodoreOnzGit/outram-park-backend/blob/develop/crates/outram-park-digital-twin-engine/verification_and_validation/double_pipe_filtering_gh1191.md";
/// The CFD itself, live, in Foam Studio on the same site.
pub const CFD_URL: &str = "../foam-studio/?shape=exchanger";

/// The plot's colours, from the theme (`bugis::Ink`): series as accents,
/// the CFD points and text in the text colour, axes in the border colour.
#[derive(Clone, Copy)]
struct Hues {
    uncal: Color32,
    post: Color32,
    koh: Color32,
    mfk: Color32,
    cfd: Color32,
    axis: Color32,
    text: Color32,
}

impl Hues {
    fn of(ink: &bugis::Ink) -> Self {
        Self {
            uncal: ink.accent(0),
            post: ink.accent(1),
            koh: ink.accent(5),
            mfk: ink.accent(3),
            cfd: ink.text,
            axis: ink.border,
            text: ink.text,
        }
    }
}

// The δ heat map (`delta_colour`) is a data colour map with a dark middle in
// both themes, so the marks drawn ON it (|δ| > 2 sd dots, values, CFD rings,
// the operating-point cross) keep fixed light colours.
const ON_MAP_CFD: Color32 = Color32::WHITE;
const ON_MAP_HELD_OUT: Color32 = Color32::from_rgb(110, 220, 140);

/// Which picture the main view shows.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Show {
    Twin,
    Delta,
    Sweep,
    Filter,
}

impl Show {
    pub const ALL: [Show; 4] = [Show::Twin, Show::Delta, Show::Sweep, Show::Filter];
    #[must_use]
    pub fn label(self) -> &'static str {
        match self {
            Show::Twin => "Twin",
            Show::Delta => "δ(x)",
            Show::Sweep => "Sweep",
            Show::Filter => "Filtering",
        }
    }
    /// From `?view=`.
    #[must_use]
    pub fn from_query(v: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|s| s.key() == v)
    }
    #[must_use]
    pub fn key(self) -> &'static str {
        match self {
            Show::Twin => "twin",
            Show::Delta => "delta",
            Show::Sweep => "sweep",
            Show::Filter => "filter",
        }
    }
}

/// The twin view's toggle.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Model {
    Uncalibrated,
    Calibrated,
    Cfd,
    All,
}

impl Model {
    pub const ALL: [Model; 4] = [
        Model::Uncalibrated,
        Model::Calibrated,
        Model::Cfd,
        Model::All,
    ];
    #[must_use]
    pub fn label(self) -> &'static str {
        match self {
            Model::Uncalibrated => "1-D uncal.",
            Model::Calibrated => "Calibrated",
            Model::Cfd => "CFD",
            Model::All => "All",
        }
    }
    /// From `?model=`.
    #[must_use]
    pub fn from_query(v: &str) -> Option<Self> {
        match v {
            "uncalibrated" => Some(Model::Uncalibrated),
            "calibrated" => Some(Model::Calibrated),
            "cfd" => Some(Model::Cfd),
            "all" => Some(Model::All),
            _ => None,
        }
    }
    /// The layers this toggle draws: (uncalibrated 1-D, calibrated twin, CFD).
    #[must_use]
    pub fn layers(self) -> (bool, bool, bool) {
        match self {
            Model::Uncalibrated => (true, false, true),
            Model::Calibrated => (false, true, true),
            Model::Cfd => (false, false, true),
            Model::All => (true, true, true),
        }
    }
}

/// The slice and operating point the reader set: Re_hot continuous, the
/// rest on the committed grid's levels.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Slider {
    pub re_hot: f64,
    pub i_rc: usize,
    pub i_k: usize,
    pub i_l: usize,
}

impl Default for Slider {
    /// gh:#1178's held-out H7 (Re_hot 30, Re_cold 15, k_wall 10, L 0.2).
    fn default() -> Self {
        Self {
            re_hot: 30.0,
            i_rc: 4,
            i_k: 1,
            i_l: 2,
        }
    }
}

impl Slider {
    #[must_use]
    pub fn operating(&self) -> Operating {
        Operating::si(
            self.re_hot,
            artefacts::re_cold_levels()[self.i_rc],
            K_LEVELS[self.i_k],
            L_LEVELS[self.i_l],
        )
    }
    /// The grid node nearest the operating Re_hot.
    #[must_use]
    pub fn i_rh(&self) -> usize {
        artefacts::nearest_level(&artefacts::re_hot_levels(), self.re_hot)
    }
}

/// "Play": Re_hot drifts between 10 and 90 and back, geometrically, with a
/// period of `period_s`, so the live 1-D is seen following it.
#[must_use]
pub fn play_re_hot(t_s: f64, period_s: f64) -> f64 {
    let phase = (t_s / period_s).fract();
    let tri = if phase < 0.5 {
        2.0 * phase
    } else {
        2.0 - 2.0 * phase
    };
    (10.0_f64.ln() + tri * (90.0_f64.ln() - 10.0_f64.ln())).exp()
}

/// At most one request in flight; the latest wanted input is sent when the
/// last answer arrives (so sliders never queue stale work).
#[derive(Clone, Debug, Default)]
pub struct Pending {
    next_id: u32,
    in_flight: Option<u32>,
    wanted: Option<Operating>,
    sent: Option<Operating>,
}

impl Pending {
    /// The reader wants `at`.
    pub fn want(&mut self, at: Operating) {
        if self.sent != Some(at) {
            self.wanted = Some(at);
        }
    }
    /// The request to send now, if any.
    pub fn next(&mut self) -> Option<Request> {
        if self.in_flight.is_some() {
            return None;
        }
        let at = self.wanted.take()?;
        self.next_id += 1;
        self.in_flight = Some(self.next_id);
        self.sent = Some(at);
        Some(Request::Evaluate {
            id: self.next_id,
            at,
        })
    }
    /// An answer arrived: is it the one in flight.
    pub fn arrived(&mut self, id: u32) -> bool {
        if self.in_flight == Some(id) || id == 0 {
            self.in_flight = None;
            true
        } else {
            false
        }
    }
    #[must_use]
    pub fn busy(&self) -> bool {
        self.in_flight.is_some()
    }
}

/// Data to world inside a plot box of world size `size` (the smaller side
/// is 100 world units, so Reset fits it), x optionally logarithmic.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Frame {
    pub size: [f64; 2],
    /// Plot-area insets inside the box, world units: left, bottom, right, top.
    pub inset: [f64; 4],
    pub x: [f64; 2],
    pub y: [f64; 2],
    pub log_x: bool,
}

/// Plot-area insets inside the box, world units: left, bottom, right, top.
pub const INSET: [f64; 4] = [14.0, 10.0, 4.0, 4.0];

impl Frame {
    /// A box with the aspect of `rect`.
    #[must_use]
    pub fn for_rect(w: f32, h: f32, x: [f64; 2], y: [f64; 2], log_x: bool) -> Self {
        let m = f64::from(w.min(h)).max(1.0);
        Self {
            size: [100.0 * f64::from(w) / m, 100.0 * f64::from(h) / m],
            inset: INSET,
            x,
            y,
            log_x,
        }
    }
    /// The same frame with other insets (the δ map keeps room for its
    /// colour bar).
    #[must_use]
    pub fn with_inset(mut self, inset: [f64; 4]) -> Self {
        self.inset = inset;
        self
    }
    fn fx(&self, x: f64) -> f64 {
        if self.log_x {
            (x.ln() - self.x[0].ln()) / (self.x[1].ln() - self.x[0].ln())
        } else {
            (x - self.x[0]) / (self.x[1] - self.x[0])
        }
    }
    /// World coordinates of data (x, y).
    #[must_use]
    pub fn w(&self, x: f64, y: f64) -> [f64; 2] {
        let i = self.inset;
        let (l, b) = (i[0], i[1]);
        let (pw, ph) = (self.size[0] - i[0] - i[2], self.size[1] - i[1] - i[3]);
        [
            l + pw * self.fx(x),
            b + ph * (y - self.y[0]) / (self.y[1] - self.y[0]),
        ]
    }
    /// The world point Reset centres on.
    #[must_use]
    pub fn home(&self) -> [f64; 2] {
        [0.5 * self.size[0], 0.5 * self.size[1]]
    }
}

/// Pixels kept above the data for a view's legend (16 px a line).
#[must_use]
pub fn legend_h(show: Show) -> f32 {
    match show {
        Show::Twin | Show::Sweep => 4.0 * 16.0 + 10.0,
        Show::Filter => 3.0 * 16.0 + 10.0,
        Show::Delta => 22.0,
    }
}

/// Tick values for `[lo, hi]`: a 1-2-5 step giving about `n` ticks.
#[must_use]
pub fn ticks(lo: f64, hi: f64, n: usize) -> Vec<f64> {
    if !(hi > lo) || n == 0 {
        return Vec::new();
    }
    let raw = (hi - lo) / n as f64;
    let mag = 10f64.powf(raw.log10().floor());
    let step = [1.0, 2.0, 5.0, 10.0]
        .iter()
        .map(|m| m * mag)
        .find(|s| *s >= raw)
        .unwrap_or(10.0 * mag);
    let mut t = (lo / step).ceil() * step;
    let mut out = Vec::new();
    while t <= hi + 1e-9 * step {
        out.push(if t.abs() < 1e-12 * step { 0.0 } else { t });
        t += step;
    }
    out
}

/// A padded y-range over finite values.
#[must_use]
pub fn y_range(values: impl Iterator<Item = f64>) -> [f64; 2] {
    let (lo, hi) = values
        .filter(|v| v.is_finite())
        .fold((f64::INFINITY, f64::NEG_INFINITY), |(a, b), v| {
            (a.min(v), b.max(v))
        });
    if !(hi >= lo) {
        return [0.0, 1.0];
    }
    let pad = ((hi - lo) * 0.08).max(1e-6 * hi.abs().max(1e-12));
    [lo - pad, hi + pad]
}

/// δ on ln UA as a diverging colour: blue below zero, red above, saturated
/// at ±`span`.
#[must_use]
pub fn delta_colour(d: f64, span: f64) -> Color32 {
    let f = (d / span).clamp(-1.0, 1.0) as f32;
    let mix = |a: f32, b: f32, t: f32| (a + (b - a) * t).round() as u8;
    let (base, t) = if f < 0.0 {
        ([60.0, 110.0, 230.0], -f)
    } else {
        ([230.0, 80.0, 60.0], f)
    };
    let mid = [40.0, 44.0, 52.0];
    Color32::from_rgb(
        mix(mid[0], base[0], t),
        mix(mid[1], base[1], t),
        mix(mid[2], base[2], t),
    )
}

/// The grid rows of a slice, Re_hot ascending.
#[must_use]
pub fn slice_rows(a: &Artefacts, s: &Slider) -> Vec<GridRow> {
    (0..N_RE).map(|i| *a.at(i, s.i_rc, s.i_k, s.i_l)).collect()
}

/// The CFD point at the grid node nearest the operating point, if one lies
/// there.
#[must_use]
pub fn cfd_at_node(a: &Artefacts, s: &Slider) -> Option<CfdPoint> {
    let rh = artefacts::re_hot_levels()[s.i_rh()];
    a.cfd_on_slice(s.i_rc, s.i_k, s.i_l)
        .into_iter()
        .find(|p| (p.x[0] / rh - 1.0).abs() < 1e-9)
        .cloned()
}

/// The readout lines for the twin view at the operating point, from the
/// live slice and the committed node. Each line names where its number
/// comes from.
#[must_use]
pub fn readout(
    a: &Artefacts,
    s: &Slider,
    live: Option<&Slice>,
    model: Model,
    ink: &bugis::Ink,
) -> Vec<(String, Color32)> {
    let h = Hues::of(ink);
    let (uncal, cal, cfd) = model.layers();
    let mut out = Vec::new();
    let node = a.at(s.i_rh(), s.i_rc, s.i_k, s.i_l);
    if let Some(p) = live.map(|l| &l.point) {
        if uncal {
            out.push((
                format!("1-D uncalibrated (live): UA {:.4} W/K", p.nominal),
                h.uncal,
            ));
        }
        if cal {
            out.push((
                format!(
                    "1-D at posterior-mean θ, no δ (live): {:.4} W/K, θ-only 95 % [{:.4}, {:.4}]",
                    p.post_mean, p.theta_lo, p.theta_hi
                ),
                h.post,
            ));
        }
    }
    if cal {
        out.push((
            format!(
                "KOH η+δ at node Re_hot {:.1} (committed): {:.4} W/K, 95 % [{:.4}, {:.4}]",
                node.x[0], node.koh_median, node.koh_lower95, node.koh_upper95
            ),
            h.koh,
        ));
    }
    if cfd {
        match cfd_at_node(a, s) {
            Some(p) => out.push((
                format!(
                    "CFD {} at Re_hot {:.1} ({}): UA {:.4} W/K",
                    p.id,
                    p.x[0],
                    if p.training { "training" } else { "held out" },
                    p.ua
                ),
                h.cfd,
            )),
            None => out.push((
                "CFD: no design point at this node (the CFD exists only at the 44 points)"
                    .to_string(),
                ink.muted,
            )),
        }
    }
    out
}

/// The committed posterior-mean θ the live "no δ" curve uses, as text.
#[must_use]
pub fn theta_line(a: &Artefacts) -> String {
    let m = a.theta.mean_si;
    format!("Posterior-mean θ used live (committed, {} thinned samples beside it): θ_h {:.3}, θ_c {:.3}, r_f {:.2e} m² K/W", a.theta.samples.len(), m[0], m[1], m[2])
}

pub struct TwinApp {
    theme: bugis::Theme,
    link: Option<TLink>,
    link_error: Option<String>,
    panel: Panel,
    view: View,
    artefacts: Result<Artefacts, String>,
    slider: Slider,
    show: Show,
    model: Model,
    pending: Pending,
    live: Option<Slice>,
    error: Option<String>,
    play: bool,
    /// `?spin`: redraw every frame without asking the worker anything, the
    /// lag check's baseline (rendering cost alone, beside `?play`).
    spin: bool,
    play_t0: f64,
    title: String,
}

impl TwinApp {
    /// Start-up: the worker link, the artefacts, the query. GUI glue (exempt:
    /// needs an eframe context; its parts, [`Artefacts::load`], [`Pending`]
    /// and the query parsers, are tested).
    pub fn new(cc: &eframe::CreationContext<'_>) -> Self {
        bugis::Theme::default().apply(&cc.egui_ctx);
        #[cfg(not(target_arch = "wasm32"))]
        let link: Result<TLink, String> = {
            let ctx = cc.egui_ctx.clone();
            Ok(bugis::web_demo::link::start_native(
                crate::engine::Engine::default(),
                move || ctx.request_repaint(),
            ))
        };
        #[cfg(target_arch = "wasm32")]
        let link = bugis::web_demo::link::start_web::<Request, Event>(
            cc.egui_ctx.clone(),
            "./worker.js",
            |m| Event::Error { id: 0, message: m },
        );
        let (link, link_error) = match link {
            Ok(l) => (Some(l), None),
            Err(e) => (None, Some(e)),
        };
        let q = bugis::web_demo::platform::query_pairs();
        let get = |k: &str| bugis::web_demo::platform::query_value(&q, k);
        let mut app = Self {
            theme: bugis::Theme::default(),
            link,
            link_error,
            panel: Panel::default(),
            view: View::new(50.0),
            artefacts: Artefacts::load(),
            slider: Slider::default(),
            show: get("view").and_then(Show::from_query).unwrap_or(Show::Twin),
            model: get("model")
                .and_then(Model::from_query)
                .unwrap_or(Model::All),
            pending: Pending::default(),
            live: None,
            error: None,
            play: get("play").is_some(),
            spin: get("spin").is_some(),
            play_t0: now_s(),
            title: String::new(),
        };
        app.pending.want(app.slider.operating());
        app
    }

    /// Take the worker's answers and send the next request. Plumbing over
    /// tested pieces ([`Pending`]).
    fn pump(&mut self) {
        let events = self.link.as_ref().map(|l| l.drain()).unwrap_or_default();
        for e in events {
            match e {
                Event::Slice(s) => {
                    if self.pending.arrived(s.id) {
                        self.live = Some(s);
                        self.error = None;
                    }
                }
                Event::Error { id, message } => {
                    if self.pending.arrived(id) {
                        self.error = Some(message);
                    }
                }
            }
        }
        if let (Some(r), Some(l)) = (self.pending.next(), &self.link) {
            l.send(r);
        }
    }

    /// One row of level buttons. GUI drawing (exempt from the reaching-test
    /// rule; the levels are the tested grid's).
    fn levels(ui: &mut egui::Ui, label: &str, levels: &[f64], unit: &str, i: &mut usize) -> bool {
        let mut changed = false;
        ui.label(format!("{label} [{unit}]"));
        ui.horizontal_wrapped(|ui| {
            for (k, v) in levels.iter().enumerate() {
                let t = if *v >= 10.0 {
                    format!("{v:.0}")
                } else {
                    format!("{v:.3}")
                        .trim_end_matches('0')
                        .trim_end_matches('.')
                        .to_string()
                };
                if ui
                    .add(
                        egui::Button::selectable(*i == k, RichText::new(t).size(15.0))
                            .min_size(Vec2::new(40.0, 34.0)),
                    )
                    .clicked()
                {
                    changed |= *i != k;
                    *i = k;
                }
            }
        });
        changed
    }

    /// The side panel. GUI drawing (exempt).
    fn side_panel(&mut self, ui: &mut egui::Ui) {
        ui.label("A digital twin of a laminar double-pipe heat exchanger: the TUAS 1-D model, evaluated live in this tab's worker, corrected toward a conjugate-heat-transfer CFD by a Kennedy–O'Hagan calibration (θ and a discrepancy δ(x)).");
        ui.separator();
        ui.label(RichText::new("View").strong());
        ui.horizontal_wrapped(|ui| {
            for s in Show::ALL {
                ui.selectable_value(&mut self.show, s, s.label());
            }
        });
        ui.label(RichText::new("Twin toggle").strong());
        ui.horizontal_wrapped(|ui| {
            for m in Model::ALL {
                ui.selectable_value(&mut self.model, m, m.label());
            }
        });
        ui.separator();
        ui.label(RichText::new("Operating point").strong());
        let mut re = self.slider.re_hot;
        if ui
            .add(
                egui::Slider::new(&mut re, 10.0..=90.0)
                    .logarithmic(true)
                    .text("Re_hot (tube)"),
            )
            .changed()
        {
            self.slider.re_hot = re;
            self.play = false;
        }
        if ui
            .add_sized(
                [200.0, 36.0],
                egui::Button::selectable(self.play, "▶ Play: Re_hot drifts"),
            )
            .clicked()
        {
            self.play = !self.play;
            self.play_t0 = now_s();
        }
        Self::levels(
            ui,
            "Re_cold (annulus)",
            &artefacts::re_cold_levels(),
            "-",
            &mut self.slider.i_rc,
        );
        Self::levels(ui, "k_wall", &K_LEVELS, "W/(m K)", &mut self.slider.i_k);
        Self::levels(ui, "length L", &L_LEVELS, "m", &mut self.slider.i_l);
        ui.label("Re_cold, k_wall and L move on the committed grid's levels, where the calibrated predictive was computed; Re_hot is continuous for the live 1-D, and the calibrated readout uses the nearest of its 9 grid nodes.");
        if ui
            .add_sized([200.0, 36.0], egui::Button::new("Reset to H7 (default)"))
            .clicked()
        {
            self.slider = Slider::default();
            self.play = false;
        }
        ui.separator();
        if let Some(s) = &self.live {
            ui.label(format!(
                "Live 1-D: {} TUAS evaluations in {:.0} ms (one_d::ua, march-free)",
                s.evaluations, s.ms
            ));
        }
        ui.separator();
        ui.label(RichText::new("Where every number comes from").strong());
        for line in NOTES {
            ui.label(format!("• {line}"));
        }
        if let Ok(a) = &self.artefacts {
            ui.label(theta_line(a));
            egui::CollapsingHeader::new("Artefact provenance (as committed)").show(ui, |ui| {
                for l in &a.provenance {
                    ui.label(RichText::new(l).size(12.0));
                }
            });
        }
        ui.add(
            egui::Hyperlink::from_label_and_url(
                "Demo record: method, artefacts, checks",
                RECORD_URL,
            )
            .open_in_new_tab(true),
        );
        ui.add(
            egui::Hyperlink::from_label_and_url("KOH calibration record (gh:#1180)", KOH_URL)
                .open_in_new_tab(true),
        );
        ui.add(
            egui::Hyperlink::from_label_and_url("Filtering record (gh:#1191)", FILTER_URL)
                .open_in_new_tab(true),
        );
        ui.add(
            egui::Hyperlink::from_label_and_url("The CFD itself, live, in Foam Studio", CFD_URL)
                .open_in_new_tab(true),
        );
    }

    /// The main view. GUI drawing (exempt).
    fn canvas(&mut self, ui: &mut egui::Ui) {
        let (resp, painter) = ui.allocate_painter(ui.available_size(), Sense::hover());
        let full = resp.rect;
        let ink = bugis::Ink::of_ui(ui);
        let h = Hues::of(&ink);
        painter.rect_filled(full, 0.0, ink.bg);
        // Tabs under the 52 px button row, then the twin's toggle.
        let tab_w = ((full.width() - 16.0 - 18.0) / 4.0).min(150.0);
        // With the panel open on a phone the main view is a sliver: the
        // tabs and readouts (all in the panel too) give way to the plot.
        let roomy = tab_w >= 70.0;
        let mut y = full.top() + 56.0;
        if roomy {
            for (k, s) in Show::ALL.into_iter().enumerate() {
                let r = Rect::from_min_size(
                    Pos2::new(full.left() + 8.0 + k as f32 * (tab_w + 6.0), y),
                    Vec2::new(tab_w, 38.0),
                );
                if ui
                    .put(
                        r,
                        egui::Button::selectable(
                            self.show == s,
                            RichText::new(s.label()).size(15.0),
                        ),
                    )
                    .clicked()
                {
                    self.show = s;
                }
            }
            y += 44.0;
            if self.show == Show::Twin {
                for (k, m) in Model::ALL.into_iter().enumerate() {
                    let r = Rect::from_min_size(
                        Pos2::new(full.left() + 8.0 + k as f32 * (tab_w + 6.0), y),
                        Vec2::new(tab_w, 36.0),
                    );
                    if ui
                        .put(
                            r,
                            egui::Button::selectable(
                                self.model == m,
                                RichText::new(m.label()).size(14.0),
                            ),
                        )
                        .clicked()
                    {
                        self.model = m;
                    }
                }
                y += 42.0;
            }
        }
        let text = egui::FontId::proportional(12.5);
        let mut lines: Vec<(String, Color32)> = Vec::new();
        if let Some(e) = self.link_error.as_ref().or(self.error.as_ref()) {
            lines.push((e.clone(), ink.error));
        }
        let artefacts = match &self.artefacts {
            Ok(a) => a,
            Err(e) => {
                painter.text(
                    Pos2::new(full.left() + 10.0, y),
                    egui::Align2::LEFT_TOP,
                    format!("artefacts did not load: {e}"),
                    text,
                    ink.error,
                );
                self.panel.reopen_button(ui, full);
                return;
            }
        };
        if self.live.is_none() || self.pending.busy() {
            lines.push((
                if self.live.is_none() {
                    "evaluating the 1-D model in the worker…".to_string()
                } else {
                    "updating…".to_string()
                },
                ink.warn,
            ));
        }
        match self.show {
            Show::Twin => lines.extend(readout(artefacts, &self.slider, self.live.as_ref(), self.model, &ink)),
            Show::Delta => lines.push(("δ(x): ln UA_CFD ≈ η(x, θ) + δ(x), δ's posterior mean (committed). Dot: |δ| > 2 sd. δ partly absorbs θ (not identified apart).".into(), h.text)),
            Show::Sweep => lines.push(("Co-kriging (MFK, 375 TUAS + 36 CFD) mean ± 2σ, beside KOH's 95 % and the CFD (committed).".into(), h.text)),
            Show::Filter => {
                let (ba, bb) = (artefacts::mean_after_burn_in(&artefacts.trace_pa, |s| s.estimate - s.truth), artefacts::mean_after_burn_in(&artefacts.trace_pb, |s| s.estimate - s.truth));
                let (na, nb) = (artefacts::mean_after_burn_in(&artefacts.trace_pa, |s| s.nis), artefacts::mean_after_burn_in(&artefacts.trace_pb, |s| s.nis));
                let (ea, eb) = (artefacts::mean_after_burn_in(&artefacts.trace_pa, |s| s.nees), artefacts::mean_after_burn_in(&artefacts.trace_pb, |s| s.nees));
                lines.push((format!("P-A closures known: r_f bias {:+.1e}, NIS {na:.2}, NEES {ea:.2}", ba), h.koh));
                lines.push((format!("P-B textbook closures: bias {:+.1e}, NIS {nb:.2} (looks healthy), NEES {eb:.0}", bb), ink.error));
                lines.push(("⚠ P-B is confidently wrong: the innovations look fine while r_f is off by its own size.".into(), ink.warn));
            }
        }
        lines.push(("research, education and V&V only".into(), ink.muted));
        if !roomy {
            lines.clear();
        }
        for (l, c) in &lines {
            let g = painter.layout(l.clone(), text.clone(), *c, full.width() - 20.0);
            let h = g.size().y;
            painter.galley(Pos2::new(full.left() + 10.0, y), g, *c);
            y += h + 2.0;
        }
        let plot = Rect::from_min_max(
            Pos2::new(full.left() + 4.0, y + 4.0),
            Pos2::new(full.right() - 4.0, full.bottom() - 48.0),
        );
        if plot.height() > 80.0 && plot.width() > 120.0 {
            let data = Rect::from_min_max(plot.min + Vec2::new(0.0, legend_h(self.show)), plot.max);
            let presp = ui.interact(data, ui.id().with("plot"), Sense::click_and_drag());
            self.view.handle_input(ui, &presp);
            let p = painter.with_clip_rect(plot);
            p.rect_stroke(plot, 4.0, Stroke::new(1.0, h.axis), StrokeKind::Inside);
            match self.show {
                Show::Twin => draw_twin(
                    &p,
                    plot,
                    data,
                    &mut self.view,
                    artefacts,
                    &self.slider,
                    self.live.as_ref(),
                    self.model,
                    false,
                ),
                Show::Sweep => draw_twin(
                    &p,
                    plot,
                    data,
                    &mut self.view,
                    artefacts,
                    &self.slider,
                    self.live.as_ref(),
                    Model::All,
                    true,
                ),
                Show::Delta => draw_delta(&p, plot, data, &mut self.view, artefacts, &self.slider),
                Show::Filter => draw_filter(&p, plot, data, &mut self.view, artefacts),
            }
            if let Some(z) = zoom_buttons(ui, full) {
                apply_zoom(&mut self.view, data, z);
            }
        } else if let Some(z) = zoom_buttons(ui, full) {
            apply_zoom(&mut self.view, full, z);
        }
        self.panel.reopen_button(ui, full);
    }
}

/// Notes for the side panel.
pub const NOTES: [&str; 7] = [
    "1-D model (live): TUAS's SimpleShellAndTubeHeatExchanger with one tube, textbook Gnielinski Nu on both sides (no annulus correlation exists in the workspace, gh:#1195), UA from its nodal conductances without the time march (twin engine one_d::ua, bit-identical to the marched UA). Uncalibrated θ = (1, 1, 0).",
    "Calibrated (committed): Kennedy–O'Hagan with a GP discrepancy δ(x) (SEPIA's formulation ported into raffles, gh:#1180), against the 36 CFD training rows' ln UA. Posterior ln θ_h +0.16 ± 0.12, ln θ_c +0.34 ± 0.17, r_f 5.4e-4 ± 2.5e-4 m² K/W. Held-out UA RMSE 1.28 %, 95 % coverage 8/8 (recorded; regenerated in the demo record).",
    "θ is only partly identified: θ_h and r_f trade off against each other and against δ. The posterior-mean θ WITHOUT δ over-corrects (held-out RMSE 6.30 % vs 8.95 % uncalibrated and 1.28 % with δ): the deliverable is η+δ with its interval, not θ alone.",
    "CFD (committed): 44 runs of the double-pipe conjugate-heat-transfer CFD (gh:#1156/#1178): 36 training (filled), 8 held out (rings). It exists only at those points; between them the twin is what you have.",
    "Sweep (committed): two-level co-kriging (SMT's MFK ported into raffles, gh:#1190), UA only, fitted to 375 TUAS runs and the 36 CFD training rows.",
    "Filtering (committed): gh:#1191's UKF on a synthetic drifting fouling resistance, noisy outlet temperatures. Synthetic truth only, no plant data.",
    "Laminar, water-like constant properties, an invented demonstration exchanger. Education and research only: not for design, operation, licensing or safety decisions.",
];

/// Ticks and labels for a frame. GUI drawing (exempt; [`ticks`] is tested).
fn axes(
    p: &egui::Painter,
    rect: Rect,
    view: &View,
    f: &Frame,
    x_label: &str,
    y_label: &str,
    x_ticks: &[f64],
    y_ticks: &[f64],
) {
    let font = egui::FontId::proportional(11.0);
    let ink = bugis::Ink::of_ctx(p.ctx());
    let h = Hues::of(&ink);
    let s = |x: f64, y: f64| {
        let w = f.w(x, y);
        view.to_screen(rect, w[0], w[1])
    };
    let (x0, x1, y0, y1) = (f.x[0], f.x[1], f.y[0], f.y[1]);
    p.line_segment([s(x0, y0), s(x1, y0)], Stroke::new(1.0, h.axis));
    p.line_segment([s(x0, y0), s(x0, y1)], Stroke::new(1.0, h.axis));
    for &t in x_ticks {
        let a = s(t, y0);
        p.line_segment([a, a + Vec2::new(0.0, 4.0)], Stroke::new(1.0, h.axis));
        p.text(
            a + Vec2::new(0.0, 5.0),
            egui::Align2::CENTER_TOP,
            trim(t),
            font.clone(),
            ink.muted,
        );
    }
    for &t in y_ticks {
        let a = s(x0, t);
        p.line_segment(
            [a, s(x1, t)],
            Stroke::new(0.5, ink.grid),
        );
        p.text(
            a - Vec2::new(4.0, 0.0),
            egui::Align2::RIGHT_CENTER,
            trim(t),
            font.clone(),
            ink.muted,
        );
    }
    p.text(
        s(x1, y0) + Vec2::new(0.0, 20.0),
        egui::Align2::RIGHT_TOP,
        x_label,
        font.clone(),
        ink.muted,
    );
    p.text(
        s(x0, y1) + Vec2::new(4.0, 2.0),
        egui::Align2::LEFT_TOP,
        y_label,
        font,
        ink.muted,
    );
}

/// A tick label without trailing zeros. GUI helper (exempt: only drawing
/// code calls it).
fn trim(t: f64) -> String {
    let s = format!("{t:.4}");
    s.trim_end_matches('0').trim_end_matches('.').to_string()
}

/// A filled band between two curves. GUI drawing (exempt).
fn band(p: &egui::Painter, pts: &[(Pos2, Pos2)], fill: Color32) {
    for w in pts.windows(2) {
        let (a, b) = (w[0], w[1]);
        p.add(Shape::convex_polygon(
            vec![a.0, b.0, b.1, a.1],
            fill,
            Stroke::NONE,
        ));
    }
}

/// Keep the view's Reset on the frame's box. GUI glue (exempt).
fn home(view: &mut View, f: &Frame) {
    view.half_extent = 50.0;
    view.home = f.home();
    if !view.touched {
        view.centre = view.home;
    }
}

/// UA against Re_hot at the slice (the twin view, and with `sweep` the MFK
/// view). GUI drawing (exempt; the numbers come from the tested engine and
/// artefacts).
#[allow(clippy::too_many_arguments)]
fn draw_twin(
    p: &egui::Painter,
    rect: Rect,
    data: Rect,
    view: &mut View,
    a: &Artefacts,
    s: &Slider,
    live: Option<&Slice>,
    model: Model,
    sweep: bool,
) {
    let rows = slice_rows(a, s);
    let ink = bugis::Ink::of_ctx(p.ctx());
    let h = Hues::of(&ink);
    let cfd = a.cfd_on_slice(s.i_rc, s.i_k, s.i_l);
    let (uncal, cal, show_cfd) = model.layers();
    let empty: Vec<Point> = Vec::new();
    let sw = live.map_or(&empty, |l| &l.sweep);
    let ys = rows
        .iter()
        .flat_map(|r| {
            [
                r.koh_lower95,
                r.koh_upper95,
                r.ua_nominal,
                if sweep {
                    r.mfk_mean - 2.0 * r.mfk_sd
                } else {
                    r.ua_post_mean
                },
                if sweep {
                    r.mfk_mean + 2.0 * r.mfk_sd
                } else {
                    r.ua_post_mean
                },
            ]
        })
        .chain(sw.iter().flat_map(|q| [q.theta_lo, q.theta_hi]))
        .chain(cfd.iter().map(|c| c.ua));
    let f = Frame::for_rect(data.width(), data.height(), [10.0, 90.0], y_range(ys), true);
    home(view, &f);
    let to = |x: f64, y: f64| {
        let w = f.w(x, y);
        view.to_screen(data, w[0], w[1])
    };
    axes(
        p,
        data,
        view,
        &f,
        "Re_hot (log)",
        "UA [W/K]",
        &[10.0, 20.0, 30.0, 50.0, 90.0],
        &ticks(f.y[0], f.y[1], 6),
    );
    if sweep {
        band(
            p,
            &rows
                .iter()
                .map(|r| {
                    (
                        to(r.x[0], r.mfk_mean - 2.0 * r.mfk_sd),
                        to(r.x[0], r.mfk_mean + 2.0 * r.mfk_sd),
                    )
                })
                .collect::<Vec<_>>(),
            h.mfk.gamma_multiply(0.2),
        );
        p.add(Shape::line(
            rows.iter().map(|r| to(r.x[0], r.mfk_mean)).collect(),
            Stroke::new(2.0, h.mfk),
        ));
    }
    if cal || sweep {
        band(
            p,
            &rows
                .iter()
                .map(|r| (to(r.x[0], r.koh_lower95), to(r.x[0], r.koh_upper95)))
                .collect::<Vec<_>>(),
            h.koh.gamma_multiply(if sweep { 0.12 } else { 0.22 }),
        );
        p.add(Shape::line(
            rows.iter().map(|r| to(r.x[0], r.koh_median)).collect(),
            Stroke::new(2.0, h.koh),
        ));
        for r in &rows {
            p.circle_filled(to(r.x[0], r.koh_median), 2.5, h.koh);
        }
    }
    if cal && !sweep && !sw.is_empty() {
        band(
            p,
            &sw.iter()
                .map(|q| (to(q.re_hot, q.theta_lo), to(q.re_hot, q.theta_hi)))
                .collect::<Vec<_>>(),
            h.post.gamma_multiply(0.14),
        );
        let pts: Vec<Pos2> = sw.iter().map(|q| to(q.re_hot, q.post_mean)).collect();
        p.extend(Shape::dashed_line(&pts, Stroke::new(2.0, h.post), 7.0, 5.0));
    }
    if (uncal || sweep) && !sw.is_empty() {
        p.add(Shape::line(
            sw.iter().map(|q| to(q.re_hot, q.nominal)).collect(),
            Stroke::new(2.0, h.uncal),
        ));
    }
    if show_cfd {
        for c in &cfd {
            let at = to(c.x[0], c.ua);
            if c.training {
                p.circle_filled(at, 5.0, h.cfd);
            } else {
                p.circle_stroke(at, 5.5, Stroke::new(2.0, h.cfd));
            }
            p.text(
                at + Vec2::new(7.0, -7.0),
                egui::Align2::LEFT_BOTTOM,
                &c.id,
                egui::FontId::proportional(11.0),
                h.cfd,
            );
        }
    }
    // Operating point.
    let (top, bot) = (to(s.re_hot, f.y[1]), to(s.re_hot, f.y[0]));
    p.line_segment(
        [top, bot],
        Stroke::new(1.0, ink.accent(4)),
    );
    if let Some(l) = live {
        if uncal || sweep {
            p.circle_filled(to(l.point.re_hot, l.point.nominal), 4.0, h.uncal);
        }
        if cal && !sweep {
            p.circle_filled(to(l.point.re_hot, l.point.post_mean), 4.0, h.post);
        }
    }
    // Legend.
    let mut items: Vec<(&str, Color32)> = Vec::new();
    if uncal || sweep {
        items.push(("1-D uncalibrated (live)", h.uncal));
    }
    if cal && !sweep {
        items.push(("1-D at mean θ, no δ (live), θ-only 95 %", h.post));
    }
    if cal || sweep {
        items.push(("KOH η+δ median, 95 % (committed)", h.koh));
    }
    if sweep {
        items.push(("MFK mean ± 2σ (committed)", h.mfk));
    }
    if show_cfd {
        items.push((
            if cfd.is_empty() {
                "no CFD point on this slice"
            } else {
                "CFD: filled = training, ring = held out"
            },
            h.cfd,
        ));
    }
    legend(p, rect, &items);
}

/// A legend in the plot's top-left. GUI drawing (exempt).
fn legend(p: &egui::Painter, rect: Rect, items: &[(&str, Color32)]) {
    let font = egui::FontId::proportional(11.5);
    let mut y = rect.top() + 6.0;
    for (t, c) in items {
        p.line_segment(
            [
                Pos2::new(rect.left() + 10.0, y + 7.0),
                Pos2::new(rect.left() + 26.0, y + 7.0),
            ],
            Stroke::new(3.0, *c),
        );
        p.text(
            Pos2::new(rect.left() + 30.0, y),
            egui::Align2::LEFT_TOP,
            *t,
            font.clone(),
            *c,
        );
        y += 16.0;
    }
}

/// δ(x) over (Re_hot, Re_cold) at the slice's (k_wall, L). GUI drawing
/// (exempt; [`delta_colour`] is tested).
fn draw_delta(
    p: &egui::Painter,
    rect: Rect,
    data: Rect,
    view: &mut View,
    a: &Artefacts,
    s: &Slider,
) {
    let (rh, rc) = (artefacts::re_hot_levels(), artefacts::re_cold_levels());
    // Cell edges: geometric midpoints between levels, the ends mirrored.
    let edges = |l: &[f64; N_RE]| -> Vec<f64> {
        let mut e = vec![l[0] * (l[0] / l[1]).sqrt()];
        e.extend(l.windows(2).map(|w| (w[0] * w[1]).sqrt()));
        e.push(l[N_RE - 1] * (l[N_RE - 1] / l[N_RE - 2]).sqrt());
        e
    };
    let (eh, ec) = (edges(&rh), edges(&rc));
    // A square-ish heat map: log both axes, mapped through a linear frame on ln.
    let f = Frame::for_rect(
        data.width(),
        data.height(),
        [eh[0].ln(), eh[N_RE].ln()],
        [ec[0].ln(), ec[N_RE].ln()],
        false,
    )
    .with_inset([14.0, 8.0, 16.0, 2.0]);
    home(view, &f);
    let to = |x: f64, y: f64| {
        let w = f.w(x.ln(), y.ln());
        view.to_screen(data, w[0], w[1])
    };
    let span = 0.1;
    let muted = bugis::Ink::of_ctx(p.ctx()).muted;
    for i in 0..N_RE {
        for j in 0..N_RE {
            let r = a.at(i, j, s.i_k, s.i_l);
            let cell = Rect::from_two_pos(to(eh[i], ec[j]), to(eh[i + 1], ec[j + 1]));
            p.rect_filled(cell.shrink(0.5), 0.0, delta_colour(r.delta_mean, span));
            if r.delta_mean.abs() > 2.0 * r.delta_sd {
                p.circle_filled(cell.center(), 2.5, Color32::WHITE);
            }
            if cell.width() > 46.0 {
                p.text(
                    cell.center() + Vec2::new(0.0, 9.0),
                    egui::Align2::CENTER_CENTER,
                    format!("{:+.3}", r.delta_mean),
                    egui::FontId::proportional(10.0),
                    Color32::from_gray(230),
                );
            }
        }
    }
    for c in a.cfd.iter().filter(|c| {
        (c.x[2] / K_LEVELS[s.i_k] - 1.0).abs() < 1e-9
            && (c.x[3] / L_LEVELS[s.i_l] - 1.0).abs() < 1e-9
    }) {
        p.circle_stroke(
            to(c.x[0], c.x[1]),
            7.0,
            Stroke::new(2.0, if c.training { ON_MAP_CFD } else { ON_MAP_HELD_OUT }),
        );
    }
    let op = to(s.re_hot, rc[s.i_rc]);
    p.line_segment(
        [op - Vec2::new(10.0, 0.0), op + Vec2::new(10.0, 0.0)],
        Stroke::new(2.0, Color32::YELLOW),
    );
    p.line_segment(
        [op - Vec2::new(0.0, 10.0), op + Vec2::new(0.0, 10.0)],
        Stroke::new(2.0, Color32::YELLOW),
    );
    // Axis labels in data (Re) values at ln positions.
    let font = egui::FontId::proportional(11.0);
    for x in [10.0, 30.0, 90.0] {
        p.text(
            to(x, ec[0]) + Vec2::new(0.0, 4.0),
            egui::Align2::CENTER_TOP,
            trim(x),
            font.clone(),
            muted,
        );
    }
    for y in [5.0, 15.0, 45.0] {
        p.text(
            to(eh[0], y) - Vec2::new(4.0, 0.0),
            egui::Align2::RIGHT_CENTER,
            trim(y),
            font.clone(),
            muted,
        );
    }
    p.text(
        to(eh[N_RE], ec[0]) + Vec2::new(0.0, 18.0),
        egui::Align2::RIGHT_TOP,
        "Re_hot",
        font.clone(),
        muted,
    );
    p.text(
        rect.left_top() + Vec2::new(8.0, 4.0),
        egui::Align2::LEFT_TOP,
        format!(
            "Re_cold (rows) · k_wall {} · L {} m",
            K_LEVELS[s.i_k], L_LEVELS[s.i_l]
        ),
        font.clone(),
        muted,
    );
    // Colour bar.
    let bar = Rect::from_min_size(
        Pos2::new(rect.right() - 24.0, data.top() + 30.0),
        Vec2::new(12.0, (rect.height() * 0.4).clamp(60.0, 200.0)),
    );
    let n = 30;
    for k in 0..n {
        let t = k as f64 / (n - 1) as f64;
        let yy = bar.bottom() - bar.height() * t as f32;
        p.rect_filled(
            Rect::from_min_size(
                Pos2::new(bar.left(), yy - bar.height() / n as f32),
                Vec2::new(12.0, bar.height() / n as f32 + 0.5),
            ),
            0.0,
            delta_colour(span * (2.0 * t - 1.0), span),
        );
    }
    p.text(
        bar.left_top() - Vec2::new(2.0, 0.0),
        egui::Align2::RIGHT_TOP,
        format!("+{span}"),
        font.clone(),
        muted,
    );
    p.text(
        bar.left_bottom() - Vec2::new(2.0, 0.0),
        egui::Align2::RIGHT_BOTTOM,
        format!("−{span}"),
        font.clone(),
        muted,
    );
    p.text(
        bar.center_top() - Vec2::new(0.0, 4.0),
        egui::Align2::RIGHT_BOTTOM,
        "δ on ln UA",
        font,
        muted,
    );
}

/// gh:#1191's traces: r_f truth, P-A and P-B estimates ± 2σ. GUI drawing
/// (exempt; the traces are parsed and checked in [`crate::artefacts`]).
fn draw_filter(p: &egui::Painter, rect: Rect, data: Rect, view: &mut View, a: &Artefacts) {
    let scale = 1e4;
    let ink = bugis::Ink::of_ctx(p.ctx());
    let h = Hues::of(&ink);
    let ys = a.trace_pa.iter().chain(&a.trace_pb).flat_map(|s| {
        [
            s.truth * scale,
            (s.estimate - 2.0 * s.sigma) * scale,
            (s.estimate + 2.0 * s.sigma) * scale,
        ]
    });
    let f = Frame::for_rect(
        data.width(),
        data.height(),
        [0.0, 199.0],
        y_range(ys),
        false,
    );
    home(view, &f);
    let to = |x: f64, y: f64| {
        let w = f.w(x, y);
        view.to_screen(data, w[0], w[1])
    };
    axes(
        p,
        data,
        view,
        &f,
        "filter step",
        "r_f [1e-4 m² K/W]",
        &ticks(0.0, 199.0, 5),
        &ticks(f.y[0], f.y[1], 6),
    );
    let draw = |t: &[TraceStep], c: Color32, fill: Color32| {
        band(
            p,
            &t.iter()
                .map(|s| {
                    (
                        to(s.step as f64, (s.estimate - 2.0 * s.sigma) * scale),
                        to(s.step as f64, (s.estimate + 2.0 * s.sigma) * scale),
                    )
                })
                .collect::<Vec<_>>(),
            fill,
        );
        p.add(Shape::line(
            t.iter()
                .map(|s| to(s.step as f64, s.estimate * scale))
                .collect(),
            Stroke::new(1.5, c),
        ));
    };
    draw(
        &a.trace_pa,
        h.koh,
        h.koh.gamma_multiply(0.16),
    );
    draw(
        &a.trace_pb,
        ink.accent(6),
        ink.accent(6).gamma_multiply(0.16),
    );
    p.add(Shape::line(
        a.trace_pa
            .iter()
            .map(|s| to(s.step as f64, s.truth * scale))
            .collect(),
        Stroke::new(2.0, ink.text),
    ));
    legend(
        p,
        rect,
        &[
            ("synthetic truth r_f(k)", ink.text),
            ("UKF, P-A closures known, ± 2σ", h.koh),
            (
                "UKF, P-B textbook closures, ± 2σ",
                ink.accent(6),
            ),
        ],
    );
}

/// The frame loop. GUI drawing (exempt).
impl eframe::App for TwinApp {
    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        let ctx = ui.ctx().clone();
        if self.spin {
            ctx.request_repaint();
        }
        if self.play {
            self.slider.re_hot = play_re_hot(now_s() - self.play_t0, 20.0);
            ctx.request_repaint();
        }
        self.pending.want(self.slider.operating());
        self.pump();
        let title = match (&self.live, self.pending.busy()) {
            (Some(l), false) => format!(
                "Double-pipe twin · {} · UA {:.4} W/K · ready",
                self.show.label(),
                l.point.nominal
            ),
            _ => "Double-pipe twin · evaluating".to_string(),
        };
        if title != self.title {
            set_title(&ctx, &title);
            self.title = title;
        }
        let mut panel = std::mem::take(&mut self.panel);
        panel.show(ui, "Double-pipe twin", |ui| {
            bugis::selector(ui, &mut self.theme);
            self.side_panel(ui);
        });
        self.panel = panel;
        egui::CentralPanel::default()
            .frame(egui::Frame::NONE)
            .show(ui, |ui| self.canvas(ui));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn toggles_queries_and_play() {
        assert_eq!(Show::from_query("delta"), Some(Show::Delta));
        assert!(Show::ALL
            .iter()
            .all(|s| Show::from_query(s.key()) == Some(*s) && !s.label().is_empty()));
        assert_eq!(Model::from_query("calibrated"), Some(Model::Calibrated));
        assert_eq!(Model::from_query("x"), None);
        assert!(Model::ALL
            .iter()
            .all(|m| !m.label().is_empty() && m.layers().2));
        assert_eq!(Model::Cfd.layers(), (false, false, true));
        for m in ["uncalibrated", "cfd", "all"] {
            assert!(Model::from_query(m).is_some());
        }
        assert!((play_re_hot(0.0, 20.0) - 10.0).abs() < 1e-9);
        assert!((play_re_hot(10.0, 20.0) - 90.0).abs() < 1e-9);
        assert!(
            (play_re_hot(5.0, 20.0) - 30.0).abs() < 1e-9,
            "geometric midpoint"
        );
        let s = Slider::default();
        assert_eq!(s.operating().to_si(), [30.0, 15.0, 10.0, 0.2]);
        assert_eq!(s.i_rh(), 4);
    }

    /// One request in flight; the newest wanted input wins; a stale answer
    /// is ignored; an unchanged input is not re-sent.
    #[test]
    fn pending_sends_the_latest_once() {
        let mut p = Pending::default();
        let a = Operating::si(20.0, 15.0, 10.0, 0.2);
        let b = Operating::si(40.0, 15.0, 10.0, 0.2);
        p.want(a);
        let Some(Request::Evaluate { id, at }) = p.next() else {
            panic!("a request")
        };
        assert_eq!(at, a);
        assert!(p.busy());
        p.want(b);
        p.want(a);
        p.want(b);
        assert_eq!(p.next(), None, "one in flight");
        assert!(!p.arrived(id + 7), "stale");
        assert!(p.arrived(id));
        let Some(Request::Evaluate { at, .. }) = p.next() else {
            panic!("the newest")
        };
        assert_eq!(at, b);
        p.arrived(id + 1);
        p.want(b);
        assert_eq!(p.next(), None, "unchanged input not re-sent");
    }

    #[test]
    fn frames_ticks_ranges_and_colours() {
        let f = Frame::for_rect(390.0, 600.0, [10.0, 90.0], [0.0, 1.0], true);
        assert_eq!(f.size[0], 100.0);
        assert!((f.size[1] - 100.0 * 600.0 / 390.0).abs() < 1e-9);
        assert_eq!(f.w(10.0, 0.0), [INSET[0], INSET[1]]);
        let top = f.w(90.0, 1.0);
        assert!(
            (top[0] - (f.size[0] - INSET[2])).abs() < 1e-9
                && (top[1] - (f.size[1] - INSET[3])).abs() < 1e-9
        );
        assert!(
            (f.w(30.0, 0.5)[0] - (INSET[0] + 0.5 * (100.0 - INSET[0] - INSET[2]))).abs() < 1e-9,
            "√(10·90) = 30 sits mid-axis"
        );
        assert_eq!(f.home(), [50.0, 0.5 * f.size[1]]);
        let lin = Frame::for_rect(800.0, 400.0, [0.0, 10.0], [0.0, 1.0], false);
        assert_eq!(lin.size[1], 100.0);
        assert!(
            (lin.w(5.0, 0.0)[0] - (INSET[0] + 0.5 * (200.0 - INSET[0] - INSET[2]))).abs() < 1e-9
        );
        assert_eq!(
            ticks(0.0, 1.0, 5),
            vec![0.0, 0.2, 0.4, 0.6000000000000001, 0.8, 1.0]
        );
        assert_eq!(ticks(0.0, 199.0, 5), vec![0.0, 50.0, 100.0, 150.0]);
        assert!(ticks(1.0, 1.0, 5).is_empty());
        let r = y_range([1.0, 2.0, f64::NAN].into_iter());
        assert!(r[0] < 1.0 && r[1] > 2.0);
        assert_eq!(y_range(std::iter::empty()), [0.0, 1.0]);
        assert_ne!(delta_colour(-0.1, 0.1), delta_colour(0.1, 0.1));
        assert_eq!(delta_colour(0.5, 0.1), delta_colour(0.1, 0.1), "saturates");
        assert_eq!(delta_colour(0.0, 0.1), Color32::from_rgb(40, 44, 52));
    }

    /// The readout names each number's source, uses the committed node and
    /// the live point, and says so where the CFD has no point.
    #[test]
    fn readout_and_slices_come_from_the_artefacts() {
        let a = Artefacts::load().expect("artefacts");
        let s = Slider::default();
        let ink = bugis::Ink::of(&bugis::Theme::default().visuals());
        let rows = slice_rows(&a, &s);
        assert_eq!(rows.len(), N_RE);
        assert!(rows.windows(2).all(|w| w[1].x[0] > w[0].x[0]));
        let h7 = cfd_at_node(&a, &s).expect("H7 sits at the default node");
        assert_eq!(h7.id, "H7");
        let live = crate::engine::Engine::default()
            .slice(1, s.operating())
            .unwrap();
        let all = readout(&a, &s, Some(&live), Model::All, &ink);
        assert_eq!(all.len(), 4);
        // Colours come from the theme (bugis), not hardcoded RGB.
        assert_eq!(all[0].1, ink.accent(0));
        assert_eq!(all[3].1, ink.text);
        assert!(
            all[0].0.contains("live") && all[2].0.contains("committed") && all[3].0.contains("H7")
        );
        assert!(all[2].0.contains(&format!("{:.4}", rows[4].koh_median)));
        let off = Slider { re_hot: 13.0, ..s };
        let cfd = readout(&a, &off, None, Model::Cfd, &ink);
        assert_eq!(cfd.len(), 1);
        assert!(cfd[0].0.contains("no design point"));
        assert_eq!(readout(&a, &s, None, Model::Uncalibrated, &ink).len(), 1);
        assert!(theta_line(&a).contains("θ_h 1.186") && theta_line(&a).contains("200 thinned"));
    }
}
