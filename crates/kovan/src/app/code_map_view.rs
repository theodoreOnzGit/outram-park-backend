//! The **Code Map** view (GitHub #734): the workspace's crates drawn from
//! their `[package.metadata.kovan]` tags, with their required dependencies.
//!
//! What it shows is [`crate::code_map`]'s model and layout, the same ones
//! `kovan-cli code-map` writes as JSON and SVG for the static site: the
//! outram-park root, the row-4 app boxes, the topic boxes (rows 3 and 2,
//! fidelity high to low left to right), the shared-utilities base and the
//! knowledge-management box. A card shows the crate's name, maturity (badge
//! and label, "2 · parts at 3" when modules rate higher) and fidelity;
//! maturity-0 crates are greyed. Selecting a crate lights its outgoing
//! (orange) and incoming (blue) edges and dims the rest; its details
//! (row, topic, fidelity, maturity, higher-rated modules with their why,
//! dependencies, dependents) fill the side panel.
//!
//! Where the data comes from: `cargo metadata --no-deps` on a workspace
//! folder, run on a **background thread** (the no-lag rule, root
//! `CLAUDE.md`), or a `code_map.json` written by `kovan-cli code-map`.
//! rust-analyzer is never used. On first show it tries the Cargo workspace
//! Kovan was started in (the current directory or a parent); otherwise, or
//! when that has no tags, it says so and offers "Choose workspace…" (a
//! folder picker, the file-picker rule in `crates/kovan/CLAUDE.md`).
//!
//! Look and controls are the mind map's ([`crate::mindmap`]): the same
//! card painting (tinted fill, colour strip, border), the same scroll-area
//! canvas with its pan limit ([`crate::mindmap_view::CanvasLayout`],
//! [`crate::mindmap_view::fit_zoom`]), −, +, Fit and 100 % buttons, and
//! Ctrl + scroll. Edges are the mind map's edge-to-edge curves
//! ([`crate::mindmap_view::connector_sized`]). Mobile-first: the buttons
//! are always on screen, the strip wraps, and the details panel folds away
//! ("« Hide" / "Details »"), folded by default under 700 px.
//!
//! **Any Rust workspace or crate (GitHub #780).** "Choose workspace…" takes
//! any folder whose `Cargo.toml` declares `[workspace]` or `[package]`;
//! crates without tags are drawn as greyed placeholders
//! ([`CodeMap::from_cargo_metadata_allowing_untagged`]) and named in the
//! status line. "Recent" lists the folders chosen before (their own list,
//! [`crate::index_fresh::recent`], apart from the Home screen's libraries).
//! "Index fresh…" indexes the folder shown into that repository itself
//! ([`super::index_fresh_view`]); the map stays the last good one until the
//! run has finished and the reloaded map is ready.

use std::path::{Path, PathBuf};
use std::sync::{Arc, RwLock};

use crate::code_map::layout::{layout, Layout, Rect};
use crate::code_map::svg::{card_subtitle, maturity_colour, tooltip, topic_colour};
use crate::code_map::{fit_label, maturity_label, CodeMap, Fidelity};
use crate::mindmap_layout::{Bounds, Point};
use crate::mindmap_view::{fit_zoom, CanvasLayout};

use super::index_fresh_view::{FreshEvent, FreshPanel};
use crate::index_fresh::recent::{default_file as recent_file, RecentWorkspaces};

const ZOOM_LIMITS: (f64, f64) = (0.05, 4.0);
const ZOOM_STEP: f64 = 1.25;

/// What the background loader has produced.
enum Load {
    Idle,
    Loading(String),
    /// The map, where it came from, and the crates with no tag.
    Ready(CodeMap, String, Vec<String>),
    Failed(String),
}

/// A request the view makes of the app (the shared file dialog lives there).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum CodeMapRequest {
    ChooseWorkspace,
    ChooseJson,
}

#[derive(Debug, Clone, Copy)]
struct Viewport {
    zoom: Option<f64>,
    recentre: bool,
    last_zoom: f64,
    last_offset: (f64, f64),
    last_viewport: (f64, f64),
}

pub(crate) struct CodeMapView {
    load: Arc<RwLock<Load>>,
    shown: Option<(CodeMap, Layout, String)>,
    /// The shown map's crates that have no tag (placeholders).
    untagged: Vec<String>,
    /// "Index fresh…" (#780).
    fresh: FreshPanel,
    /// Whether rust-analyzer is installed, and the prompt to install it
    /// (#820): "Index fresh" needs it.
    rust_analyzer: super::rust_analyzer_view::RustAnalyzerPanel,
    /// Recently chosen workspaces (#780), most recent first.
    recent: RecentWorkspaces,
    /// The workspace an "Index fresh" just finished in, until the app takes
    /// it (the Code Review view then rebuilds its data, #820).
    index_finished: Option<PathBuf>,
    tried_default: bool,
    workspace: Option<PathBuf>,
    selected: Option<String>,
    panel_open: Option<bool>,
    /// Whether the last frame was wide (>= 700 px): crossing the line folds
    /// or opens the panel, as the mobile-first rule asks.
    was_wide: Option<bool>,
    /// Centre the view on this crate next frame (a link in the panel).
    centre_on: Option<String>,
    vp: Viewport,
}

impl Default for CodeMapView {
    fn default() -> Self {
        Self {
            load: Arc::new(RwLock::new(Load::Idle)),
            shown: None,
            untagged: Vec::new(),
            fresh: FreshPanel::default(),
            rust_analyzer: Default::default(),
            recent: recent_file().map(|f| RecentWorkspaces::load_from(&f)).unwrap_or_default(),
            index_finished: None,
            tried_default: false,
            workspace: None,
            selected: None,
            panel_open: None,
            was_wide: None,
            centre_on: None,
            vp: Viewport { zoom: None, recentre: true, last_zoom: 0.0, last_offset: (0.0, 0.0), last_viewport: (0.0, 0.0) },
        }
    }
}

/// The Cargo workspace containing the current directory, if any: the nearest
/// ancestor whose `Cargo.toml` declares `[workspace]`.
fn launch_workspace() -> Option<PathBuf> {
    let mut dir = std::env::current_dir().ok()?;
    loop {
        if is_cargo_workspace(&dir) {
            return Some(dir);
        }
        if !dir.pop() {
            return None;
        }
    }
}

fn colour(hex: &str) -> egui::Color32 {
    let v = u32::from_str_radix(hex.trim_start_matches('#'), 16).unwrap_or(0x808080);
    egui::Color32::from_rgb((v >> 16) as u8, (v >> 8) as u8, v as u8)
}

fn bounds_of(r: Rect) -> Bounds {
    Bounds { min_x: r.x, min_y: r.y, max_x: r.right(), max_y: r.bottom() }
}

impl CodeMapView {
    /// Load the map of the workspace or crate at `dir` on a background
    /// thread, and remember it in the recent list (saved on that thread).
    /// GUI glue: the loader is `code_map::load_any_workspace`, tested in
    /// `index_fresh::tests`.
    pub(crate) fn load_workspace(&mut self, dir: PathBuf) {
        self.workspace = Some(dir.clone());
        self.recent.push(&dir);
        let recent = self.recent.clone();
        let slot = self.load.clone();
        if let Ok(mut l) = slot.write() {
            *l = Load::Loading(format!("running cargo metadata in {}", dir.display()));
        }
        std::thread::spawn(move || {
            if let Some(f) = recent_file() {
                let _ = recent.save_to(&f);
            }
            let result = crate::code_map::load_any_workspace(&dir);
            if let Ok(mut l) = slot.write() {
                *l = match result {
                    Ok((map, untagged)) => Load::Ready(map, dir.display().to_string(), untagged),
                    Err(e) => Load::Failed(format!("{}: {e}", dir.display())),
                };
            }
        });
    }

    /// Load a `code_map.json` written by `kovan-cli code-map`, on a
    /// background thread.
    pub(crate) fn load_json(&mut self, path: PathBuf) {
        let slot = self.load.clone();
        if let Ok(mut l) = slot.write() {
            *l = Load::Loading(format!("reading {}", path.display()));
        }
        std::thread::spawn(move || {
            let result = std::fs::read_to_string(&path)
                .map_err(|e| e.to_string())
                .and_then(|s| serde_json::from_str::<CodeMap>(&s).map_err(|e| e.to_string()));
            if let Ok(mut l) = slot.write() {
                *l = match result {
                    Ok(map) => Load::Ready(map, path.display().to_string(), Vec::new()),
                    Err(e) => Load::Failed(format!("{}: {e}", path.display())),
                };
            }
        });
    }

    /// Take a finished load, if any, without blocking.
    fn poll(&mut self) -> Option<String> {
        let Ok(mut l) = self.load.try_write() else {
            return Some("loading\u{2026}".into());
        };
        match std::mem::replace(&mut *l, Load::Idle) {
            Load::Ready(map, source, untagged) => {
                let lay = layout(&map);
                self.shown = Some((map, lay, source));
                self.untagged = untagged;
                self.selected = None;
                self.vp.zoom = None;
                self.vp.recentre = true;
                None
            }
            Load::Loading(msg) => {
                let s = msg.clone();
                *l = Load::Loading(msg);
                Some(format!("{s}\u{2026}"))
            }
            Load::Failed(e) => {
                *l = Load::Failed(e.clone());
                Some(format!("could not load the code map: {e}"))
            }
            Load::Idle => None,
        }
    }

    /// The workspace or crate folder shown, if any.
    pub(crate) fn workspace(&self) -> Option<&Path> {
        self.workspace.as_deref()
    }

    /// The workspace an "Index fresh" finished in since this was last asked.
    pub(crate) fn take_index_finished(&mut self) -> Option<PathBuf> {
        self.index_finished.take()
    }

    /// Once, load the Cargo workspace kovan was started in (if any): what
    /// the first show of this view does, callable by the Code Review view
    /// when it is opened first. GUI glue.
    pub(crate) fn ensure_default_workspace(&mut self) {
        if !self.tried_default {
            self.tried_default = true;
            if let Some(dir) = launch_workspace() {
                self.load_workspace(dir);
            }
        }
    }

    /// Draw the view. Returns a request for the app's file dialog.
    pub(crate) fn ui(&mut self, ui: &mut egui::Ui) -> Option<CodeMapRequest> {
        self.ensure_default_workspace();
        self.rust_analyzer.ui(ui, self.workspace.as_deref());
        let status = self.poll();
        if matches!(status.as_deref(), Some(s) if s.ends_with('\u{2026}')) {
            ui.ctx().request_repaint_after(std::time::Duration::from_millis(100));
        }
        let wide = ui.available_width() >= 700.0;
        if self.was_wide != Some(wide) {
            self.was_wide = Some(wide);
            self.panel_open = Some(wide);
        }
        let panel_open = *self.panel_open.get_or_insert(wide);
        let mut request = None;

        // ── Control strip: always on screen, wraps on a phone ───────────
        let shown_zoom = self.vp.zoom.unwrap_or(self.vp.last_zoom.max(ZOOM_LIMITS.0));
        ui.horizontal_wrapped(|ui| {
            crate::app::navigation_style(ui);
            if panel_open {
                if ui.button("\u{ab} Hide").on_hover_text("Fold the details panel away").clicked() {
                    self.panel_open = Some(false);
                }
            } else if ui.button("Details \u{bb}").on_hover_text("Show the details panel").clicked() {
                self.panel_open = Some(true);
            }
            if ui.button(" \u{2212} ").on_hover_text("Zoom out").clicked() {
                self.vp.zoom = Some(shown_zoom / ZOOM_STEP);
            }
            if ui.button(" + ").on_hover_text("Zoom in").clicked() {
                self.vp.zoom = Some(shown_zoom * ZOOM_STEP);
            }
            if ui.button("Fit").on_hover_text("Whole map in view").clicked() {
                self.vp.zoom = None;
                self.vp.recentre = true;
            }
            if ui.button("100 %").clicked() {
                self.vp.zoom = Some(1.0);
            }
            ui.label(format!("{:.0} %", 100.0 * shown_zoom));
            ui.separator();
            if ui
                .button("Choose workspace\u{2026}")
                .on_hover_text("Any Rust workspace or crate folder (a Cargo.toml with [workspace] or [package]); crates without [package.metadata.kovan] tags are drawn as placeholders")
                .clicked()
            {
                request = Some(CodeMapRequest::ChooseWorkspace);
            }
            if ui
                .button("Open code_map.json\u{2026}")
                .on_hover_text("A map written by `kovan-cli code-map`")
                .clicked()
            {
                request = Some(CodeMapRequest::ChooseJson);
            }
            if !self.recent.paths.is_empty() {
                let mut pick = None;
                ui.menu_button("Recent \u{25be}", |ui| {
                    for p in &self.recent.paths {
                        if ui.button(p.display().to_string()).clicked() {
                            pick = Some(p.clone());
                            ui.close();
                        }
                    }
                });
                if let Some(p) = pick {
                    self.load_workspace(p);
                }
            }
            if let Some(dir) = self.workspace.clone() {
                if ui.button("Reload").on_hover_text("Run cargo metadata again").clicked() {
                    self.load_workspace(dir.clone());
                }
                if ui
                    .add_enabled(!self.fresh.busy(), egui::Button::new("Index fresh\u{2026}"))
                    .on_hover_text(
                        "Index this workspace or crate into its own repository: kovan_root.toml, every \
                         folder's kovan.toml, the link index and missing review.md skeletons. Asks first; \
                         commits nothing.",
                    )
                    .clicked()
                {
                    self.fresh.start(dir);
                }
            }
        });
        if let Some(FreshEvent::Finished(dir)) = self.fresh.ui(ui) {
            // Swap in the new map only once it has loaded; until then the
            // last good one stays on screen.
            self.index_finished = Some(dir.clone());
            self.load_workspace(dir);
        }
        ui.horizontal_wrapped(|ui| {
            if let Some((map, _, source)) = &self.shown {
                ui.weak(format!(
                    "{} crates, {} required dependencies \u{2014} {source}",
                    map.crates.len(),
                    map.edges.len()
                ));
            }
            if !self.untagged.is_empty() {
                ui.weak(format!(
                    "{} crate(s) have no [package.metadata.kovan] tag: drawn greyed in the base row",
                    self.untagged.len()
                ));
            }
            if let Some(s) = &status {
                ui.colored_label(ui.visuals().warn_fg_color, s);
            }
        });

        let Some((map, lay, _)) = self.shown.as_ref() else {
            ui.add_space(24.0);
            ui.vertical_centered(|ui| {
                ui.label(if status.is_some() {
                    "Waiting for the map\u{2026}"
                } else {
                    "No code map loaded. Kovan was not started inside a Cargo workspace."
                });
                ui.weak("Choose a workspace folder (cargo metadata runs in the background) or open a code_map.json.");
            });
            return request;
        };

        // ── Details panel ───────────────────────────────────────────────
        let mut go_to: Option<String> = None;
        if panel_open {
            let width = (ui.available_width() * 0.85).min(340.0);
            egui::Panel::left("code-map-details")
                .resizable(true)
                .default_size(width)
                .max_size((ui.available_width() * 0.85).max(200.0))
                .show(ui, |ui| {
                    egui::ScrollArea::vertical().show(ui, |ui| {
                        if self.selected.as_ref().is_some_and(|s| self.untagged.contains(s)) {
                            ui.colored_label(
                                ui.visuals().warn_fg_color,
                                "Untagged: this crate has no [package.metadata.kovan] tag, so its row, topic and maturity are placeholders.",
                            );
                        }
                        details(ui, map, self.selected.as_deref(), &mut go_to);
                    });
                });
        }

        // ── Canvas ──────────────────────────────────────────────────────
        let bounds = bounds_of(lay.bounds);
        let avail = ui.available_size();
        let viewport = (avail.x as f64, avail.y as f64);
        let vp = &mut self.vp;
        if vp.zoom.is_none() && viewport != vp.last_viewport {
            vp.recentre = true;
        }
        // Fit leaves a 3 % margin so the outermost boxes' borders show.
        let zoom = vp
            .zoom
            .unwrap_or_else(|| 0.97 * fit_zoom(bounds, viewport))
            .clamp(ZOOM_LIMITS.0, ZOOM_LIMITS.1);
        if let Some(z) = vp.zoom.as_mut() {
            *z = z.clamp(ZOOM_LIMITS.0, ZOOM_LIMITS.1);
        }
        let canvas = CanvasLayout::new(bounds, zoom, viewport);
        let centre_card = self.centre_on.take().and_then(|n| lay.card(&n).map(|c| c.rect.centre()));
        let offset = if let Some((x, y)) = centre_card {
            Some(canvas.offset_centring(Point::new(x, y), viewport))
        } else if vp.recentre {
            vp.recentre = false;
            Some(canvas.offset_centring(bounds.centre(), viewport))
        } else if vp.last_zoom > 0.0 && zoom != vp.last_zoom {
            let before = CanvasLayout::new(bounds, vp.last_zoom, vp.last_viewport);
            let middle = before.to_world((
                vp.last_offset.0 + 0.5 * vp.last_viewport.0,
                vp.last_offset.1 + 0.5 * vp.last_viewport.1,
            ));
            Some(canvas.offset_centring(middle, viewport))
        } else {
            None
        };
        let mut area = egui::ScrollArea::both()
            .id_salt("code-map-viewport")
            .auto_shrink([false, false])
            .scroll_source(egui::scroll_area::ScrollSource::ALL);
        if let Some((x, y)) = offset {
            area = area.scroll_offset(egui::vec2(x as f32, y as f32));
        }
        let selected = self.selected.clone();
        let mut clicked: Option<Option<String>> = None;
        let output = area.show(ui, |ui| {
            let (rect, background) = ui.allocate_exact_size(
                egui::vec2(canvas.size.0 as f32, canvas.size.1 as f32),
                egui::Sense::click(),
            );
            if background.clicked() {
                clicked = Some(None);
            }
            let painter = ui.painter_at(rect);
            let z = zoom as f32;
            let at = |x: f64, y: f64| {
                let (cx, cy) = canvas.to_canvas(Point::new(x, y));
                rect.min + egui::vec2(cx as f32, cy as f32)
            };
            let to_rect = |r: &Rect| egui::Rect::from_min_max(at(r.x, r.y), at(r.right(), r.bottom()));
            let font = |px: f64| egui::FontId::proportional((px * zoom) as f32);
            let strong = ui.visuals().strong_text_color();
            let weak = ui.visuals().weak_text_color();

            // Boxes and their titles.
            for f in &lay.frames {
                let r = to_rect(&f.rect);
                let c = colour(topic_colour(f.topic));
                painter.rect_filled(r, 10.0 * z, c.gamma_multiply(0.06));
                painter.rect_stroke(r, 10.0 * z, egui::Stroke::new((1.6 * z).max(0.5), c), egui::StrokeKind::Middle);
                painter.text(
                    at(f.rect.x + 0.5 * f.rect.w, f.rect.y + 20.0),
                    egui::Align2::CENTER_CENTER,
                    &f.title,
                    font(15.0),
                    strong,
                );
            }
            for l in &lay.labels {
                painter.text(at(l.x, l.y), egui::Align2::CENTER_CENTER, &l.text, font(12.5), weak);
            }

            // Edges: faint; the selection's own lit, the rest dimmed.
            let related = |name: &str| -> bool {
                selected.as_deref().is_some_and(|s| {
                    s == name || map.edges.iter().any(|e| (e.from == s && e.to == name) || (e.to == s && e.from == name))
                })
            };
            for e in &map.edges {
                let (Some(a), Some(b)) = (lay.card(&e.from), lay.card(&e.to)) else { continue };
                let (ax, ay) = a.rect.centre();
                let (bx, by) = b.rect.centre();
                let Some(curve) = crate::mindmap_view::connector_sized(
                    Point::new(ax, ay),
                    (a.rect.w, a.rect.h),
                    Point::new(bx, by),
                    (b.rect.w, b.rect.h),
                ) else {
                    continue;
                };
                let stroke = match selected.as_deref() {
                    Some(s) if e.from == s => egui::Stroke::new((2.6 * z).max(1.0), egui::Color32::from_rgb(217, 119, 6)),
                    Some(s) if e.to == s => egui::Stroke::new((2.6 * z).max(1.0), egui::Color32::from_rgb(37, 99, 235)),
                    Some(_) => egui::Stroke::new((1.0 * z).max(0.5), egui::Color32::from_gray(128).gamma_multiply(0.08)),
                    None => egui::Stroke::new((1.2 * z).max(0.5), egui::Color32::from_gray(128).gamma_multiply(0.3)),
                };
                painter.add(egui::epaint::CubicBezierShape::from_points_stroke(
                    curve.map(|p| at(p.x, p.y)),
                    false,
                    egui::Color32::TRANSPARENT,
                    stroke,
                ));
            }

            // The root card.
            let rr = to_rect(&lay.root);
            painter.rect_filled(rr, 10.0 * z, egui::Color32::from_rgb(30, 41, 59));
            painter.text(rr.center(), egui::Align2::CENTER_CENTER, &map.root, font(22.0), egui::Color32::WHITE);

            // Crate cards, painted as the mind map's.
            for card in &lay.cards {
                let Some(c) = map.get(&card.name) else { continue };
                let r = to_rect(&card.rect);
                let resp = ui
                    .interact(r, ui.id().with(("code-map-card", &c.name)), egui::Sense::click())
                    .on_hover_text(tooltip(map, c));
                if resp.clicked() {
                    clicked = Some(Some(c.name.clone()));
                }
                let mut tint = colour(topic_colour(c.topic));
                let mut alpha = if c.maturity == 0 { 0.5 } else { 1.0 };
                if selected.is_some() && !related(&c.name) {
                    alpha *= 0.3;
                }
                tint = tint.gamma_multiply(alpha);
                let rounding = 6.0 * z;
                painter.rect_filled(r, rounding, ui.visuals().extreme_bg_color.gamma_multiply(alpha));
                painter.rect_filled(r, rounding, tint.gamma_multiply(0.15));
                painter.rect_filled(
                    egui::Rect::from_min_max(r.min, egui::pos2(r.min.x + 6.0 * z, r.max.y)),
                    egui::CornerRadius { nw: rounding as u8, sw: rounding as u8, ne: 0, se: 0 },
                    tint,
                );
                let edge = if selected.as_deref() == Some(c.name.as_str()) {
                    egui::Stroke::new((3.0 * z).max(1.0), strong)
                } else {
                    egui::Stroke::new((1.2 * z).max(0.5), tint)
                };
                painter.rect_stroke(r, rounding, edge, egui::StrokeKind::Middle);
                let text = painter.with_clip_rect(r.shrink(2.0 * z));
                let (name, size) = fit_label(&c.name, card.rect.w - 22.0, 14.0, 11.0);
                text.text(
                    at(card.rect.x + 14.0, card.rect.y + 18.0),
                    egui::Align2::LEFT_CENTER,
                    name,
                    font(size),
                    strong.gamma_multiply(alpha),
                );
                let (sub, size) = fit_label(&card_subtitle(c), card.rect.w - 50.0, 11.5, 10.0);
                text.text(
                    at(card.rect.x + 14.0, card.rect.y + 40.0),
                    egui::Align2::LEFT_CENTER,
                    sub,
                    font(size),
                    weak.gamma_multiply(alpha),
                );
                let b = card.rect.right() - 17.0;
                let by = card.rect.bottom() - 16.0;
                painter.circle_filled(at(b, by), 10.0 * z, colour(maturity_colour(c.maturity)).gamma_multiply(alpha));
                painter.text(
                    at(b, by),
                    egui::Align2::CENTER_CENTER,
                    c.maturity.to_string(),
                    font(12.0),
                    egui::Color32::WHITE.gamma_multiply(alpha),
                );
            }
        });
        let vp = &mut self.vp;
        vp.last_zoom = zoom;
        vp.last_offset = (output.state.offset.x as f64, output.state.offset.y as f64);
        vp.last_viewport = viewport;
        // A pinch has no hover position on a touch screen: take the point
        // between the fingers instead (2026-10-06).
        let pinch_here = ui.input(|i| i.multi_touch()).is_some_and(|t| output.inner_rect.contains(t.center_pos));
        if pinch_here || ui.rect_contains_pointer(output.inner_rect) {
            let factor = ui.input(|i| i.zoom_delta()) as f64;
            if factor != 1.0 {
                vp.zoom = Some((zoom * factor).clamp(ZOOM_LIMITS.0, ZOOM_LIMITS.1));
            }
        }
        if let Some(sel) = clicked {
            self.selected = sel;
        }
        if let Some(name) = go_to {
            self.selected = Some(name.clone());
            self.centre_on = Some(name);
        }
        request
    }
}

/// The side panel: the selected crate's tags and neighbours, or the key.
fn details(ui: &mut egui::Ui, map: &CodeMap, selected: Option<&str>, go_to: &mut Option<String>) {
    let Some(c) = selected.and_then(|s| map.get(s)) else {
        ui.heading("Code map");
        ui.label("Click a crate to see its tags, what it depends on and what depends on it.");
        ui.add_space(8.0);
        ui.strong("How to read it");
        ui.label("Rows: degree of integration (0\u{2013}1 utilities, 2 domain solvers, 3 coupled multiphysics, 4 apps).");
        ui.label("Inside a topic box, fidelity runs high (F4, brute force) on the left to low (F0, lumped) on the right; a range spans its columns.");
        ui.label("Badge: maturity, the crate's lowest part (0 concept, greyed; 1 AI draft; 2 AI V&V; 3 human reviewed; 4 human V&V).");
        ui.label("Lines: required dependencies. Selected: orange what it needs, blue what needs it.");
        ui.add_space(8.0);
        ui.weak("Built from cargo metadata; rust-analyzer is not used.");
        return;
    };
    ui.heading(&c.name);
    if let Some(b) = &c.backronym {
        ui.label(egui::RichText::new(b).italics());
    }
    if let Some(d) = &c.description {
        ui.label(d);
    }
    ui.add_space(6.0);
    let fidelity = match c.fidelity {
        Some(Fidelity::Level(l)) => format!("{l} \u{2014} {}", Fidelity::level_meaning(l)),
        Some(Fidelity::Range(lo, hi)) => format!(
            "{lo}\u{2013}{hi} \u{2014} {} to {}",
            Fidelity::level_meaning(lo),
            Fidelity::level_meaning(hi)
        ),
        None => "none (utility or knowledge management)".into(),
    };
    let field = |ui: &mut egui::Ui, k: &str, v: String| {
        ui.horizontal_wrapped(|ui| {
            ui.strong(k);
            ui.label(v);
        });
    };
    field(ui, "row", c.row.to_string());
    field(ui, "topic", c.topic.as_str().to_string());
    field(ui, "fidelity", fidelity);
    field(ui, "maturity", format!("{} \u{2014} {}", c.maturity, maturity_label(c.maturity)));
    if let Some(d) = &c.dir {
        field(ui, "folder", d.clone());
    }
    if !c.maturity_modules.is_empty() {
        ui.add_space(6.0);
        ui.strong("Parts rated higher");
        for m in &c.maturity_modules {
            ui.label(format!("{} at {} ({}): {}", m.module, m.level, maturity_label(m.level), m.why));
        }
    }
    let list = |ui: &mut egui::Ui, title: &str, names: Vec<&str>, go_to: &mut Option<String>| {
        ui.add_space(6.0);
        ui.strong(format!("{title} ({})", names.len()));
        if names.is_empty() {
            ui.weak("none");
        }
        for n in names {
            if ui.link(n).clicked() {
                *go_to = Some(n.to_string());
            }
        }
    };
    list(ui, "Depends on", map.dependencies(&c.name), go_to);
    list(ui, "Needed by", map.dependents(&c.name), go_to);
}

/// Whether `dir` holds a `Cargo.toml` declaring `[workspace]`.
fn is_cargo_workspace(dir: &Path) -> bool {
    std::fs::read_to_string(dir.join("Cargo.toml"))
        .map(|t| t.lines().any(|l| l.trim() == "[workspace]"))
        .unwrap_or(false)
}

#[cfg(test)]
mod lag_tests {
    use super::*;
    use std::time::{Duration, Instant};

    /// **No-lag check of "Index fresh" (GitHub #780, the no-lag HARD RULE).**
    ///
    /// **Methodology.** Headless (no window, no GPU): the Code Map view of
    /// the crate at `KOVAN_FRESH_LAG_CRATE` (a scratch copy: the run writes
    /// into it) is drawn in a 1200 × 800 window, frame after frame, while a
    /// real "Index fresh" (rust-analyzer scip under nice, then parsing,
    /// links, call graph, writing, then the map reload) runs on its worker.
    /// Every frame feeds input: the pointer over the canvas, a scroll (pan)
    /// and a zoom step, alternating. Each frame's UI-thread time is
    /// `run_ui` plus `tessellate`, i.e. everything but the GPU upload.
    /// Pass: the 99th-percentile frame under 16 ms, and the run finished.
    /// It does not measure the GPU or the compositor.
    ///
    /// Ignored by default (it runs rust-analyzer); run with
    /// `KOVAN_FRESH_LAG_CRATE=<dir> cargo test --release -p … --lib
    /// lag_tests -- --ignored --nocapture`.
    ///
    /// **Result (2026-10-07, rust-analyzer 1.98.0, 16-core desktop):** a
    /// scratch copy of `syn` 2.0.119 (one crate, 65 391 `.rs` lines, a
    /// one-card map): 863 frames over 14.1 s, UI-thread frame time median
    /// 0.18 ms, p99 0.94 ms, max 1.41 ms, no frame over 16 ms; the run
    /// wrote its 8 `kovan.toml`, the link file, the root and 8 skeletons.
    /// Not covered: a 48-crate map while indexing outram-park itself
    /// (not run: about 15 GB), and the GPU/compositor side.
    #[test]
    #[ignore = "manual no-lag check (#780): needs KOVAN_FRESH_LAG_CRATE and rust-analyzer"]
    fn frames_stay_under_a_frame_budget_while_indexing() {
        let Some(dir) = std::env::var_os("KOVAN_FRESH_LAG_CRATE").map(PathBuf::from) else {
            eprintln!("KOVAN_FRESH_LAG_CRATE not set: skipped");
            return;
        };
        let ctx = egui::Context::default();
        let window = egui::vec2(1200.0, 800.0);
        let mut view = CodeMapView { tried_default: true, ..CodeMapView::default() };
        view.load_workspace(dir.clone());
        let mut times: Vec<Duration> = Vec::new();
        let frame = |view: &mut CodeMapView, i: usize, times: &mut Vec<Duration>| {
            let centre = egui::pos2(700.0, 450.0);
            let mut events = vec![egui::Event::PointerMoved(centre)];
            if i % 2 == 0 {
                events.push(egui::Event::MouseWheel {
                    unit: egui::MouseWheelUnit::Point,
                    delta: egui::vec2(if i % 4 == 0 { 30.0 } else { -30.0 }, 20.0),
                    modifiers: egui::Modifiers::NONE,
                    phase: egui::TouchPhase::Move,
                });
            } else {
                events.push(egui::Event::Zoom(if i % 6 < 3 { 1.05 } else { 1.0 / 1.05 }));
            }
            let input = egui::RawInput {
                screen_rect: Some(egui::Rect::from_min_size(egui::Pos2::ZERO, window)),
                events,
                ..Default::default()
            };
            let t = Instant::now();
            let out = ctx.run_ui(input, |ui| {
                let _ = view.ui(ui);
            });
            let _ = ctx.tessellate(out.shapes, out.pixels_per_point);
            times.push(t.elapsed());
            std::thread::sleep(Duration::from_millis(16));
        };
        let mut i = 0;
        while view.shown.is_none() {
            frame(&mut view, i, &mut times);
            i += 1;
            assert!(i < 3000, "the map did not load");
        }
        times.clear();
        let started = Instant::now();
        view.fresh.run_now(dir.clone(), crate::index_fresh::FreshChoices::default());
        while view.fresh.busy() {
            frame(&mut view, i, &mut times);
            i += 1;
        }
        // The map reload after the run.
        for _ in 0..120 {
            frame(&mut view, i, &mut times);
            i += 1;
        }
        let wall = started.elapsed();
        let mut sorted = times.clone();
        sorted.sort();
        let pct = |p: f64| sorted[((sorted.len() - 1) as f64 * p) as usize];
        let over = sorted.iter().filter(|t| **t > Duration::from_millis(16)).count();
        eprintln!(
            "no-lag check: {} frames over {:.1} s; UI-thread frame time median {:?}, p99 {:?}, max {:?}; {} frame(s) over 16 ms",
            sorted.len(),
            wall.as_secs_f64(),
            pct(0.5),
            pct(0.99),
            sorted[sorted.len() - 1],
            over
        );
        assert!(dir.join("kovan_root.toml").is_file(), "the run did not finish");
        assert!(pct(0.99) < Duration::from_millis(16), "p99 frame {:?}", pct(0.99));
    }
}
