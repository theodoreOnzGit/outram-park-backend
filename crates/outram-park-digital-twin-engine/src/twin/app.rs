//! # `TwinApp`: the template's GUI (gh:#1745)
//!
//! One egui app for every multifidelity twin. The adopter supplies a
//! [`TwinHost`]: where the twin runs (a native thread, [`super::local`]; a
//! browser worker), its plant controls, and the data each page draws. The
//! app supplies everything else, the same way for every reactor:
//!
//! - a bottom tab bar of the five pages (`bugis::tab_bar`, which fits a
//!   390 px phone), with the short labels of [`TwinPage::tab_label`];
//! - **Gruvbox** (maintainer, 2026-11-25: the twin frontends "should use
//!   gruvbox"): `bugis::Theme`, Gruvbox Dark by default, applied by the app
//!   itself on its first frame, with a Dark / Light selector in the panel,
//!   so no adopter has to remember it;
//! - the collapsible control panel (`bugis::web_demo::panel`, folded by
//!   default on a narrow screen) holding the adopter's plant controls and
//!   the research-only banner;
//! - **Plant**: a trend of any plant column against time, with on-screen
//!   zoom in, zoom out and reset (`bugis::web_demo::view::zoom_buttons`) on
//!   the time window ([`TrendWindow`]);
//! - **Control volumes**: the live map as two tables, volumes then links;
//! - **Monte Carlo**: the runs with `k ± σ`, the live coefficients, the set
//!   in use with its source, and "Run" / "Send to plant" buttons;
//! - **Coarse / fine multiphysics**: the result lines, or "not built" with
//!   the issue.
//!
//! **No lag (workspace hard rule).** The app only draws and forwards
//! [`TwinAction`]s: the host runs the plant and every Monte Carlo or
//! multiphysics run off the UI thread and the app reads what the host has.
//!
//! The text each page shows is built by plain functions ([`cv_row`],
//! [`link_row`], [`mc_lines`], [`not_built`], [`TrendWindow`],
//! [`trend_points`]), which the tests check; the drawing that places it is
//! GUI code, exempt from a reaching test as the workspace rule allows.
//!
//! Research, education and V&V only. NOT VALIDATED. AI draft (gh:#1745).

use bugis::tab_bar;
use bugis::web_demo::panel::Panel;
use bugis::web_demo::view::{zoom_buttons, Zoom};
use egui::{Rect, RichText, Ui, Vec2};
use uom::si::mass::kilogram;
use uom::si::mass_rate::kilogram_per_second;
use uom::si::power::kilowatt;
use uom::si::thermodynamic_temperature::degree_celsius;

use super::{ControlVolume, ControlVolumes, CvLink, CvLinkKind, KeffResult, MeshResolution, TwinError, TwinPage};

/// The banner every twin shows (RESPONSIBLE_USE.md).
pub const BANNER: &str =
    "Research, education and V&V only. NOT VALIDATED. Not for operation, licensing or safety decisions.";

/// What the reader asked for, for the host to carry out off the UI thread.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TwinAction {
    RunMonteCarlo,
    SendToPlant,
    RunMultiphysics(MeshResolution),
}

/// One Monte Carlo run as the page shows it.
#[derive(Clone, Debug, PartialEq)]
pub struct McRunView {
    pub label: String,
    /// `None` until it finishes.
    pub result: Option<KeffResult>,
}

/// The Monte Carlo page's data.
#[derive(Clone, Debug, PartialEq, Default)]
pub struct McPageView {
    pub runs: Vec<McRunView>,
    pub running: bool,
    /// The latest live set, one line per coefficient, empty if none.
    pub live: Vec<String>,
    /// The set the plant is using, with its source.
    pub in_use: String,
    /// The last failure, if any.
    pub error: Option<String>,
}

/// What an adopter supplies to [`TwinApp`].
pub trait TwinHost {
    /// The window or page title.
    fn title(&self) -> String;
    /// Whether a page has its model ([`super::MultiFidelityTwin::page_status`]).
    fn page_status(&self, page: TwinPage) -> Result<(), TwinError>;
    /// The plant columns ([`super::TwinSnapshot::columns`]); column 0 is time.
    fn columns(&self) -> Vec<&'static str>;
    /// The plant history, one row per snapshot, oldest first.
    fn history(&self) -> Vec<Vec<f64>>;
    /// The live control volumes.
    fn control_volumes(&self) -> ControlVolumes;
    /// The Monte Carlo page's data.
    fn monte_carlo(&self) -> McPageView;
    /// The result lines of the last run at `resolution`, empty if none.
    fn multiphysics(&self, resolution: MeshResolution) -> Vec<String>;
    /// The adopter's plant controls, drawn in the panel (GUI code).
    fn controls(&mut self, ui: &mut Ui);
    /// Carry out an action off the UI thread.
    fn act(&mut self, action: TwinAction);
}

/// "Not built" text for a page without its model.
pub fn not_built(page: TwinPage, issue: u32) -> String {
    format!("{} is not built for this twin yet. Tracked in gh:#{issue}. Nothing on this page stands in for it.", page.title())
}

fn blank_or(v: Option<String>) -> String {
    v.unwrap_or_else(|| "—".into())
}

/// One volume as a table row: id, name, role, temperature [°C], mass [kg],
/// inside the pressure boundary. A value the model does not have is "—".
pub fn cv_row(v: &ControlVolume) -> [String; 6] {
    [
        v.id.clone(),
        v.name.clone(),
        v.role.word().into(),
        blank_or(v.temperature.map(|t| format!("{:.1}", t.get::<degree_celsius>()))),
        blank_or(v.mass.map(|m| format!("{:.1}", m.get::<kilogram>()))),
        if v.on_pressure_boundary { "yes".into() } else { "no".into() },
    ]
}

/// One link as a table row: from, to, what it carries, the rate.
pub fn link_row(l: &CvLink) -> [String; 4] {
    let (kind, rate) = match l.kind {
        CvLinkKind::Flow(m) => ("flow", format!("{:.2} kg/s", m.get::<kilogram_per_second>())),
        CvLinkKind::Heat(p) => ("heat", blank_or(p.map(|p| format!("{:.1} kW", p.get::<kilowatt>())))),
    };
    [l.from.clone(), l.to.clone(), kind.into(), rate]
}

/// The Monte Carlo page as lines of text.
pub fn mc_lines(v: &McPageView) -> Vec<String> {
    let mut out = vec![format!("In use: {}", v.in_use)];
    for r in &v.runs {
        out.push(match r.result {
            Some(k) => format!("{}: k_eff = {:.5} ± {:.5}", r.label, k.k, k.sigma),
            None if v.running => format!("{}: running…", r.label),
            None => format!("{}: not run", r.label),
        });
    }
    if v.live.is_empty() {
        out.push("No live set yet: run the cases, then send the result to the plant.".into());
    } else {
        out.push("Live set (not in use until sent):".into());
        out.extend(v.live.iter().cloned());
    }
    if let Some(e) = &v.error {
        out.push(format!("Failed: {e}"));
    }
    out
}

/// The plant trend's time window, s: zoom in halves it, zoom out doubles
/// it, reset returns to the default; between [`TrendWindow::MIN_S`] and
/// [`TrendWindow::MAX_S`].
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct TrendWindow {
    pub seconds: f64,
}

impl TrendWindow {
    pub const DEFAULT_S: f64 = 600.0;
    pub const MIN_S: f64 = 10.0;
    pub const MAX_S: f64 = 86_400.0;

    pub fn apply(&mut self, z: Zoom) {
        self.seconds = match z {
            Zoom::In => (self.seconds * 0.5).max(Self::MIN_S),
            Zoom::Out => (self.seconds * 2.0).min(Self::MAX_S),
            Zoom::Reset => Self::DEFAULT_S,
        };
    }
}

impl Default for TrendWindow {
    fn default() -> Self {
        TrendWindow { seconds: Self::DEFAULT_S }
    }
}

/// `(time, value)` of column `col` over the last `window_s` seconds of
/// `history` (column 0 is time). Rows too short are skipped.
pub fn trend_points(history: &[Vec<f64>], col: usize, window_s: f64) -> Vec<[f64; 2]> {
    let Some(last) = history.iter().rev().find_map(|r| r.first().copied()) else {
        return Vec::new();
    };
    history
        .iter()
        .filter(|r| r.len() > col && r[0] >= last - window_s)
        .map(|r| [r[0], r[col]])
        .collect()
}

/// The vertical range the trend shows: the data's, widened about its
/// middle to at least 0.1 % of its largest magnitude (and at least 1e-9),
/// so a steady value does not fill the plot with its round-off. `None`
/// for no points.
pub fn y_range(points: &[[f64; 2]]) -> Option<(f64, f64)> {
    let (lo, hi) = points.iter().fold((f64::INFINITY, f64::NEG_INFINITY), |(a, b), p| (a.min(p[1]), b.max(p[1])));
    if !(lo.is_finite() && hi.is_finite()) {
        return None;
    }
    let span = (1e-3 * lo.abs().max(hi.abs())).max(1e-9);
    if hi - lo >= span {
        return Some((lo, hi));
    }
    let mid = 0.5 * (lo + hi);
    Some((mid - 0.5 * span, mid + 0.5 * span))
}

/// The template's app over a host. See the module docs.
pub struct TwinApp<H: TwinHost> {
    pub host: H,
    pub page: TwinPage,
    pub window: TrendWindow,
    /// The plant column the trend shows.
    pub column: usize,
    /// The Gruvbox variant (Dark by default).
    pub theme: bugis::Theme,
    /// The variant last applied to the context; `None` before the first
    /// frame.
    applied: Option<bugis::Theme>,
    panel: Panel,
}

/// The theme to apply this frame: `theme` when it differs from what was
/// last applied (always on the first frame), else nothing.
pub fn theme_to_apply(theme: bugis::Theme, applied: Option<bugis::Theme>) -> Option<bugis::Theme> {
    (applied != Some(theme)).then_some(theme)
}

impl<H: TwinHost> TwinApp<H> {
    /// The app on the Plant page, trending column 1 (the first after time).
    pub fn new(host: H) -> Self {
        TwinApp {
            host,
            page: TwinPage::Plant,
            window: TrendWindow::default(),
            column: 1,
            theme: bugis::Theme::default(),
            applied: None,
            panel: Panel::default(),
        }
    }

    /// Draw one frame (GUI code). Call from `eframe::App::ui`.
    pub fn ui(&mut self, ui: &mut Ui) {
        if let Some(t) = theme_to_apply(self.theme, self.applied) {
            t.apply(ui.ctx());
            self.applied = Some(t);
        }
        let title = self.host.title();
        let mut panel = core::mem::take(&mut self.panel);
        panel.show(ui, &title, |ui| {
            ui.label(RichText::new(BANNER).small().strong());
            ui.horizontal(|ui| {
                for t in bugis::Theme::ALL {
                    ui.selectable_value(&mut self.theme, t, t.label());
                }
            });
            ui.separator();
            self.host.controls(ui);
            ui.separator();
            let columns = self.host.columns();
            egui::ComboBox::from_label("Plant trend")
                .selected_text(columns.get(self.column).copied().unwrap_or("—"))
                .show_ui(ui, |ui| {
                    for (i, c) in columns.iter().enumerate().skip(1) {
                        ui.selectable_value(&mut self.column, i, *c);
                    }
                });
        });
        self.panel = panel;
        egui::CentralPanel::default().show(ui, |ui| {
            let full = ui.max_rect();
            let bar = tab_bar::layout(full, TwinPage::ALL.len());
            // Below the "Controls »" row every demo keeps clear.
            let main = Rect::from_min_max(bar.main.min + Vec2::new(8.0, 56.0), bar.main.max - Vec2::new(8.0, 4.0));
            ui.scope_builder(egui::UiBuilder::new().max_rect(main), |ui| self.page_ui(ui, main));
            if self.page == TwinPage::Plant {
                if let Some(z) = zoom_buttons(ui, bar.main) {
                    self.window.apply(z);
                }
            }
            let labels: Vec<&str> = TwinPage::ALL.iter().map(|p| p.tab_label()).collect();
            let selected = TwinPage::ALL.iter().position(|p| *p == self.page).unwrap_or(0);
            if let Some(i) = tab_bar::tab_bar(ui, &bar, &labels, selected) {
                self.page = TwinPage::ALL[i];
            }
            self.panel.reopen_button(ui, full);
        });
    }

    /// One page's content (GUI code).
    fn page_ui(&mut self, ui: &mut Ui, rect: Rect) {
        ui.heading(self.page.title());
        if let Err(TwinError::NotBuilt { page, issue }) = self.host.page_status(self.page) {
            ui.label(not_built(page, issue));
            return;
        }
        match self.page {
            TwinPage::Plant => {
                let history = self.host.history();
                let columns = self.host.columns();
                let name = columns.get(self.column).copied().unwrap_or("—");
                let points = trend_points(&history, self.column, self.window.seconds);
                ui.label(format!("{name}, last {:.0} s", self.window.seconds));
                let (lo, hi) = y_range(&points).unwrap_or((0.0, 1.0));
                egui_plot::Plot::new("twin_trend")
                    .height((rect.height() - 40.0).max(120.0))
                    .include_y(lo)
                    .include_y(hi)
                    .allow_zoom(false)
                    .allow_drag(false)
                    .show(ui, |p| p.line(egui_plot::Line::new(name, egui_plot::PlotPoints::from_iter(points))));
            }
            TwinPage::ControlVolumes => {
                let cvs = self.host.control_volumes();
                egui::ScrollArea::both().show(ui, |ui| {
                    egui::Grid::new("twin_cvs").striped(true).show(ui, |ui| {
                        for h in ["id", "name", "role", "T [°C]", "mass [kg]", "boundary"] {
                            ui.strong(h);
                        }
                        ui.end_row();
                        for v in &cvs.volumes {
                            for c in cv_row(v) {
                                ui.label(c);
                            }
                            ui.end_row();
                        }
                    });
                    ui.add_space(12.0);
                    egui::Grid::new("twin_links").striped(true).show(ui, |ui| {
                        for h in ["from", "to", "carries", "rate"] {
                            ui.strong(h);
                        }
                        ui.end_row();
                        for l in &cvs.links {
                            for c in link_row(l) {
                                ui.label(c);
                            }
                            ui.end_row();
                        }
                    });
                });
            }
            TwinPage::MonteCarlo => {
                let v = self.host.monte_carlo();
                ui.horizontal_wrapped(|ui| {
                    if ui.add_enabled(!v.running, egui::Button::new("Run the cases")).clicked() {
                        self.host.act(TwinAction::RunMonteCarlo);
                    }
                    if ui.add_enabled(!v.live.is_empty(), egui::Button::new("Send to plant")).clicked() {
                        self.host.act(TwinAction::SendToPlant);
                    }
                });
                egui::ScrollArea::vertical().show(ui, |ui| {
                    for l in mc_lines(&v) {
                        ui.label(l);
                    }
                });
            }
            TwinPage::CoarseMultiphysics | TwinPage::FineMultiphysics => {
                let r = if self.page == TwinPage::CoarseMultiphysics { MeshResolution::Coarse } else { MeshResolution::Fine };
                if ui.button("Run").clicked() {
                    self.host.act(TwinAction::RunMultiphysics(r));
                }
                for l in self.host.multiphysics(r) {
                    ui.label(l);
                }
            }
        }
    }
}

#[cfg(test)]
mod tests;
