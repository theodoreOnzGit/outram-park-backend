//! # `TwinShell`: the one UI struct every multifidelity simulator uses (gh:#1756)
//!
//! Maintainer direction, 2026-11-25: "It should be standardized, gruvbox,
//! left panel, right panel, central view and bottom panel shld all be in the
//! struct. If something is not available, then grey it out. This shld be
//! used across all simulators with multifidelity, htr10, msre and ebrii", and
//! Fukushima too. The shell owns the layout and the look; a simulator only
//! fills the slots, through [`ShellHost`]:
//!
//! | Slot | Owned by the shell | Filled by the host |
//! |---|---|---|
//! | theme | **Gruvbox** (`bugis::Theme`, Dark by default), applied on the first frame, Dark / Light in the left panel | nothing |
//! | left panel | `bugis::web_demo::panel::Panel`: folds, folded by default under 700 px, "Controls »" to reopen; the research-only banner | [`ShellHost::left`]: the controls |
//! | page bar | the seven [`TwinPage`]s along the bottom, one row at 390 px, a page without its model **greyed out** | [`ShellHost::page_state`] |
//! | Plant sub-tabs | a row under the top strip on the Plant page, greyed likewise | [`ShellHost::plant_subtabs`] |
//! | right dock | `bugis::web_demo::dock` (right), split first | [`ShellHost::right_dock`], [`ShellHost::right`] |
//! | bottom dock | `bugis::web_demo::dock` (bottom), on what is left | [`ShellHost::bottom_dock`], [`ShellHost::bottom`] |
//! | central view | what is left, with zoom in / out / reset where the view zooms | [`ShellHost::central`], [`ShellHost::zoomable`], [`ShellHost::zoom`] |
//!
//! **Greyed out.** A greyed page or sub-tab can still be tapped (a phone has
//! no hover to explain it): the central view then says it is not built for
//! this simulator, names its issue, and nothing stands in for it. The host's
//! central view is not called for it.
//!
//! The pages and their order are [`TwinPage::ALL`]. The severe-accident
//! model has no page: it acts on all of them (maintainer, 2026-11-25).
//! Atmospheric dispersion is a Plant sub-tab ([`ATMOSPHERIC_DISPERSION`]).
//!
//! **No lag.** The shell only draws; the host keeps every calculation off
//! the UI thread, as each simulator already does.
//!
//! The geometry is a plain function ([`ShellLayout::new`]), tested; the
//! drawing that places it is GUI code, exempt from a reaching test as the
//! workspace rule allows.
//!
//! Research, education and V&V only. NOT VALIDATED. AI draft (gh:#1756).

use bugis::tab_bar;
use bugis::web_demo::dock::{Dock, Edge};
use bugis::web_demo::panel::Panel;
use bugis::web_demo::view::{zoom_buttons, Zoom};
use egui::{Pos2, Rect, RichText, Ui, Vec2};

use super::{TwinPage, ViewState, UNDECLARED_VIEW_ISSUE};

/// The banner every simulator shows (RESPONSIBLE_USE.md).
pub const BANNER: &str =
    "Research, education and V&V only. NOT VALIDATED. Not for operation, licensing or safety decisions.";

/// The Plant page's first sub-tab: the plant itself.
pub const PLANT_SCHEMATIC: &str = "Plant";
/// The Plant page's standard second sub-tab (maintainer, 2026-11-25:
/// "Atmospheric dispersion tab is a sub tab of plant model").
pub const ATMOSPHERIC_DISPERSION: &str = "Dispersion";

/// Height of the strip at the top of the main area kept for "Controls »"
/// and the zoom buttons (44 px + inset, as every web demo keeps).
pub const TOP_STRIP_PX: f32 = 52.0;
/// Height of the Plant sub-tab row.
pub const SUBTAB_ROW_PX: f32 = 40.0;

/// One Plant sub-tab.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct SubTab {
    pub label: &'static str,
    pub state: ViewState,
}

/// Where the reader is: the page, and on the Plant page the sub-tab index.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ShellAt {
    pub page: TwinPage,
    /// Index into [`ShellHost::plant_subtabs`]; 0 off the Plant page.
    pub sub: usize,
}

/// The Plant sub-tabs of a simulator that has only its plant: the plant,
/// and atmospheric dispersion greyed out with `issue`.
pub fn default_plant_subtabs(dispersion_issue: u32) -> Vec<SubTab> {
    vec![
        SubTab { label: PLANT_SCHEMATIC, state: ViewState::Built },
        SubTab { label: ATMOSPHERIC_DISPERSION, state: ViewState::NotBuilt { issue: dispersion_issue } },
    ]
}

/// What a simulator supplies to [`TwinShell`]. Only `title`, `page_state`,
/// `left` and `central` are required; a simulator with no docks, no zoom or
/// no sub-tabs leaves the rest at their defaults.
pub trait ShellHost {
    /// The page heading and the left panel's title.
    fn title(&self) -> String;
    /// Whether `page` has its model; [`ViewState::NotBuilt`] greys it out.
    fn page_state(&self, page: TwinPage) -> ViewState;
    /// The Plant page's sub-tabs, the plant first. Default: the plant and
    /// atmospheric dispersion, greyed (gh:#1756).
    fn plant_subtabs(&self) -> Vec<SubTab> {
        default_plant_subtabs(UNDECLARED_VIEW_ISSUE)
    }
    /// The left panel's contents under the banner and theme: the controls.
    fn left(&mut self, ui: &mut Ui, at: ShellAt);
    /// The central view in `rect`, for a built page or sub-tab.
    fn central(&mut self, ui: &mut Ui, rect: Rect, at: ShellAt);
    /// Whether the central view zooms; the shell then draws + / − / Reset.
    fn zoomable(&self, _at: ShellAt) -> bool {
        false
    }
    /// A zoom button was pressed.
    fn zoom(&mut self, _at: ShellAt, _z: Zoom) {}
    /// The right dock's title, if this view has one.
    fn right_dock(&self, _at: ShellAt) -> Option<&'static str> {
        None
    }
    /// The right dock's body.
    fn right(&mut self, _ui: &mut Ui, _rect: Rect, _at: ShellAt) {}
    /// The bottom dock's title, if this view has one.
    fn bottom_dock(&self, _at: ShellAt) -> Option<&'static str> {
        None
    }
    /// The bottom dock's body.
    fn bottom(&mut self, _ui: &mut Ui, _rect: Rect, _at: ShellAt) {}
}

/// The fixed part of the layout in the central area `full`: the page bar,
/// the top strip, the Plant sub-tab row and what is left for the docks and
/// the central view.
#[derive(Clone, Debug, PartialEq)]
pub struct ShellLayout {
    pub bar: tab_bar::TabBarLayout,
    /// The strip along the top for "Controls »" and the zoom buttons.
    pub top: Rect,
    /// The Plant sub-tab row, `None` off the Plant page or with one sub-tab.
    pub subtabs: Option<Rect>,
    /// What is left: split by the docks, the rest is the central view.
    pub body: Rect,
}

impl ShellLayout {
    /// Lay out `full` for a bar of every page, with a sub-tab row when
    /// `subtab_row`.
    pub fn new(full: Rect, subtab_row: bool) -> Self {
        let bar = tab_bar::layout(full, TwinPage::ALL.len());
        let main = bar.main;
        let top_bottom = (main.top() + TOP_STRIP_PX).min(main.bottom());
        let top = Rect::from_min_max(main.min, Pos2::new(main.right(), top_bottom));
        let (subtabs, body_top) = if subtab_row {
            let b = (top_bottom + SUBTAB_ROW_PX).min(main.bottom());
            (Some(Rect::from_min_max(Pos2::new(main.left(), top_bottom), Pos2::new(main.right(), b))), b)
        } else {
            (None, top_bottom)
        };
        let body = Rect::from_min_max(Pos2::new(main.left(), body_top), main.max);
        ShellLayout { bar, top, subtabs, body }
    }
}

/// The text of a greyed page or sub-tab's central view.
pub fn not_built_text(what: &str, issue: u32) -> String {
    format!("{what} is not built for this simulator yet. Tracked in gh:#{issue}. Nothing here stands in for it.")
}

/// The state of the view at `at`: the page's, and on the Plant page also the
/// sub-tab's (a sub-tab index past the list is the plant).
pub fn view_state(page: ViewState, subtabs: &[SubTab], at: ShellAt) -> ViewState {
    match page {
        ViewState::NotBuilt { .. } => page,
        ViewState::Built if at.page == TwinPage::Plant => subtabs.get(at.sub).map(|s| s.state).unwrap_or(ViewState::Built),
        ViewState::Built => ViewState::Built,
    }
}

/// The shell. See the module docs.
pub struct TwinShell {
    /// The Gruvbox variant (Dark by default).
    pub theme: bugis::Theme,
    applied: Option<bugis::Theme>,
    pub left: Panel,
    /// The page on screen.
    pub page: TwinPage,
    /// The Plant sub-tab on screen.
    pub plant_sub: usize,
    /// One dock per (view, edge), made the first time the view asks for it,
    /// so each view keeps its own folded / open state.
    docks: Vec<(ShellAt, Edge, Dock)>,
}

impl Default for TwinShell {
    fn default() -> Self {
        TwinShell {
            theme: bugis::Theme::default(),
            applied: None,
            left: Panel::default(),
            page: TwinPage::Plant,
            plant_sub: 0,
            docks: Vec::new(),
        }
    }
}

/// The theme to apply this frame: `theme` when it differs from what was
/// last applied (always on the first frame), else nothing.
pub fn theme_to_apply(theme: bugis::Theme, applied: Option<bugis::Theme>) -> Option<bugis::Theme> {
    (applied != Some(theme)).then_some(theme)
}

impl TwinShell {
    /// Where the reader is.
    pub fn at(&self) -> ShellAt {
        ShellAt { page: self.page, sub: if self.page == TwinPage::Plant { self.plant_sub } else { 0 } }
    }

    fn dock(&mut self, at: ShellAt, edge: Edge, title: &'static str) -> &mut Dock {
        let i = match self.docks.iter().position(|(a, e, _)| *a == at && *e == edge) {
            Some(i) => i,
            None => {
                self.docks.push((at, edge, Dock::new(edge, title)));
                self.docks.len() - 1
            }
        };
        &mut self.docks[i].2
    }

    /// Draw one frame over `host` (GUI code, exempt from a reaching test;
    /// its geometry is [`ShellLayout::new`]'s and its states
    /// [`view_state`]'s, both tested). Call from `eframe::App::ui` with the
    /// root `ui`.
    pub fn ui<H: ShellHost>(&mut self, ui: &mut Ui, host: &mut H) {
        if let Some(t) = theme_to_apply(self.theme, self.applied) {
            t.apply(ui.ctx());
            self.applied = Some(t);
        }
        let at = self.at();
        let title = host.title();
        let mut theme = self.theme;
        self.left.show(ui, &title, |ui| {
            ui.label(RichText::new(BANNER).small().strong());
            ui.horizontal(|ui| {
                for t in bugis::Theme::ALL {
                    ui.selectable_value(&mut theme, t, t.label());
                }
            });
            ui.separator();
            host.left(ui, at);
        });
        self.theme = theme;
        egui::CentralPanel::default().show(ui, |ui| self.central_area(ui, host, at));
    }

    fn central_area<H: ShellHost>(&mut self, ui: &mut Ui, host: &mut H, at: ShellAt) {
        let full = ui.max_rect();
        let subtabs = host.plant_subtabs();
        let layout = ShellLayout::new(full, at.page == TwinPage::Plant && subtabs.len() > 1);
        let page_states: Vec<ViewState> = TwinPage::ALL.iter().map(|p| host.page_state(*p)).collect();
        let state = view_state(page_states[at.page.index()], &subtabs, at);

        // Docks first (right on the body, then bottom on what is left), so
        // the central view gets the rest; a greyed view has none.
        let mut central = layout.body;
        let mut docked: Vec<(Edge, bugis::web_demo::dock::Split)> = Vec::new();
        if state.is_built() {
            for edge in [Edge::Right, Edge::Bottom] {
                let title = match edge {
                    Edge::Right => host.right_dock(at),
                    Edge::Bottom => host.bottom_dock(at),
                };
                if let Some(title) = title {
                    let split = self.dock(at, edge, title).split(central);
                    if let Some(body) = split.body {
                        ui.scope_builder(egui::UiBuilder::new().max_rect(body), |ui| match edge {
                            Edge::Right => host.right(ui, body, at),
                            Edge::Bottom => host.bottom(ui, body, at),
                        });
                    }
                    central = split.main;
                    docked.push((edge, split));
                }
            }
        }

        match state {
            ViewState::Built => {
                if central.width() >= 1.0 && central.height() >= 1.0 {
                    ui.scope_builder(egui::UiBuilder::new().max_rect(central), |ui| host.central(ui, central, at));
                }
            }
            ViewState::NotBuilt { issue } => {
                let what = if at.page == TwinPage::Plant {
                    subtabs.get(at.sub).map(|s| s.label).unwrap_or(PLANT_SCHEMATIC)
                } else {
                    at.page.title()
                };
                let inner = central.shrink(12.0);
                ui.scope_builder(egui::UiBuilder::new().max_rect(inner), |ui| {
                    ui.heading(what);
                    ui.label(not_built_text(what, issue));
                });
            }
        }

        // On top of the views: the dock toggles, the top strip, the
        // sub-tabs, the zoom buttons, the page bar and "Controls »".
        for (edge, split) in &docked {
            let title = match edge {
                Edge::Right => host.right_dock(at),
                Edge::Bottom => host.bottom_dock(at),
            };
            if let Some(title) = title {
                self.dock(at, *edge, title).toggle(ui, split);
            }
        }
        if let Some(row) = layout.subtabs {
            ui.painter().rect_filled(row, 0.0, ui.visuals().panel_fill);
            let weak = ui.visuals().weak_text_color();
            ui.scope_builder(egui::UiBuilder::new().max_rect(row.shrink2(Vec2::new(8.0, 2.0))), |ui| {
                egui::ScrollArea::horizontal().id_salt("twin_shell_subtabs").show(ui, |ui| {
                    ui.horizontal_centered(|ui| {
                        for (i, s) in subtabs.iter().enumerate() {
                            let mut text = RichText::new(s.label).size(15.0);
                            if !s.state.is_built() {
                                text = text.color(weak);
                            }
                            if ui.add(egui::Button::selectable(i == self.plant_sub, text).min_size(Vec2::new(0.0, 34.0))).clicked() {
                                self.plant_sub = i;
                            }
                        }
                    });
                });
            });
        }
        if state.is_built() && host.zoomable(at) {
            if let Some(z) = zoom_buttons(ui, layout.top.union(central)) {
                host.zoom(at, z);
            }
        }
        let labels: Vec<&str> = TwinPage::ALL.iter().map(|p| p.tab_label()).collect();
        let greyed: Vec<bool> = page_states.iter().map(|s| !s.is_built()).collect();
        if let Some(i) = tab_bar::tab_bar_greyed(ui, &layout.bar, &labels, at.page.index(), &greyed) {
            self.page = TwinPage::ALL[i];
        }
        self.left.reopen_button(ui, full);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn rect(w: f32, h: f32) -> Rect {
        Rect::from_min_size(Pos2::ZERO, Vec2::new(w, h))
    }

    /// At phone width the seven pages fit one row; the top strip, the
    /// sub-tab row and the body stack down to the bar and tile the main
    /// area, with or without the sub-tab row.
    #[test]
    fn the_layout_tiles_a_phone() {
        let full = rect(390.0, 844.0);
        for row in [false, true] {
            let l = ShellLayout::new(full, row);
            assert_eq!(l.bar.tabs.len(), 7);
            assert!(l.bar.tabs.iter().all(|t| full.contains_rect(*t)));
            assert_eq!(l.top.min, full.min);
            assert_eq!(l.top.height(), TOP_STRIP_PX);
            let below = match l.subtabs {
                Some(s) => {
                    assert_eq!(s.top(), l.top.bottom());
                    assert_eq!(s.height(), SUBTAB_ROW_PX);
                    s.bottom()
                }
                None => l.top.bottom(),
            };
            assert_eq!(l.subtabs.is_some(), row);
            assert_eq!(l.body.top(), below);
            assert_eq!(l.body.bottom(), l.bar.bar.top());
            assert_eq!((l.body.left(), l.body.right()), (0.0, 390.0));
        }
        // A window shorter than the strips never gives a negative body.
        let tiny = ShellLayout::new(rect(390.0, 60.0), true);
        assert!(tiny.body.height() >= 0.0 && tiny.top.height() >= 0.0);
    }

    /// The view's state: a greyed page greys every sub-tab; on a built Plant
    /// page the sub-tab decides; off the Plant page the sub-tab is ignored.
    #[test]
    fn a_view_is_greyed_by_its_page_or_its_sub_tab() {
        let subs = default_plant_subtabs(99);
        let plant = |sub| ShellAt { page: TwinPage::Plant, sub };
        assert_eq!(view_state(ViewState::Built, &subs, plant(0)), ViewState::Built);
        assert_eq!(view_state(ViewState::Built, &subs, plant(1)), ViewState::NotBuilt { issue: 99 });
        assert_eq!(view_state(ViewState::Built, &subs, plant(7)), ViewState::Built);
        let off = ShellAt { page: TwinPage::Graphs, sub: 1 };
        assert_eq!(view_state(ViewState::Built, &subs, off), ViewState::Built);
        let greyed = ViewState::NotBuilt { issue: 5 };
        assert_eq!(view_state(greyed, &subs, plant(0)), greyed);
        assert_eq!(subs.iter().map(|s| s.label).collect::<Vec<_>>(), [PLANT_SCHEMATIC, ATMOSPHERIC_DISPERSION]);
        assert!(not_built_text("CFD, core and primary loop", 1747).starts_with("CFD, core and primary loop is not built for this simulator yet. Tracked in gh:#1747."));
    }

    /// The shell opens on the Plant page with Gruvbox Dark, applies the
    /// theme on the first frame only, and reports the sub-tab only on the
    /// Plant page.
    #[test]
    fn the_shell_starts_on_the_plant_in_gruvbox() {
        let mut s = TwinShell::default();
        assert_eq!(s.page, TwinPage::Plant);
        assert_eq!(s.theme, bugis::Theme::default());
        assert_eq!(theme_to_apply(s.theme, None), Some(s.theme));
        assert_eq!(theme_to_apply(s.theme, Some(s.theme)), None);
        s.plant_sub = 1;
        assert_eq!(s.at(), ShellAt { page: TwinPage::Plant, sub: 1 });
        s.page = TwinPage::MonteCarlo;
        assert_eq!(s.at(), ShellAt { page: TwinPage::MonteCarlo, sub: 0 });
        let d = s.dock(s.at(), Edge::Bottom, "Results").open;
        assert!(d, "a bottom dock starts open");
        assert_eq!(s.docks.len(), 1);
        s.dock(s.at(), Edge::Bottom, "Results");
        assert_eq!(s.docks.len(), 1, "the same view keeps its dock");
    }
}
