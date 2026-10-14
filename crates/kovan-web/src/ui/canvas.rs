//! The three canvases: the code map, a crate's module tree, a module's
//! function ring with its unfolded callers and callees. Plus the card
//! drawing they share and the right-click / long-press menu.

use std::sync::Arc;

use egui::{Align2, Color32, FontId, Pos2, Rect, Sense, Shape, Stroke, StrokeKind, Vec2};
use kovan_common::call_graph::split::CrateSlice;
use kovan_common::code_map::layout::FrameKind;
use kovan_common::code_map::svg::{card_subtitle, topic_colour};
use kovan_common::code_map::{CodeMap, Topic};
use kovan_common::geometry::Point;
use kovan_common::mindmap_view::{connector_sized, CARD_SIZE, CENTRE_CARD_SIZE};

use super::bar::review_colour;
use super::camera::{Camera, WorldRect};
use super::{CodeReview, Hit, Level, Place, Snap};
use crate::model::{self, crate_of, Dir, ModTree, TreeItem, MOD_CARD};

/// `#rrggbb` to a colour.
pub(crate) fn hex(s: &str) -> Color32 {
    let v = u32::from_str_radix(s.trim_start_matches('#'), 16).unwrap_or(0x888888);
    Color32::from_rgb((v >> 16) as u8, (v >> 8) as u8, v as u8)
}

/// A font size for zoomed canvas text, rounded to whole points and capped
/// at 40: every distinct size puts its glyphs in egui's font atlas, and an
/// unrounded size that follows the zoom filled the atlas within a few
/// frames (panic in `epaint::text::font`, seen 2026-10-06 in Chromium).
fn quantise(size: f32) -> f32 {
    size.round().clamp(5.0, 40.0)
}

fn font(size: f32) -> FontId {
    FontId::proportional(quantise(size))
}

fn mono(size: f32) -> FontId {
    FontId::monospace(quantise(size))
}

/// The canvas colours for the theme in force. The cards were drawn for a
/// dark background (the site forces dark visuals); desktop kovan sets its
/// own theme for the whole window (#820), and under a light one (Gruvbox
/// light) the cards stayed dark. Dark keeps the colours the cards always
/// had, so the published site does not change; light takes its fills and
/// text from the host's visuals, so the cards follow whichever light theme
/// is set.
#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct Palette {
    dark: bool,
    /// Behind the card fills, for mixing a topic tint (light only).
    bg: Color32,
    /// The code map's root card.
    root: Color32,
    /// A module card, and a function of the crate being viewed.
    card: Color32,
    /// The examples card.
    examples: Color32,
    /// The in-place module list under an expanded crate on the map.
    list: Color32,
    /// Card titles, and the outline of a selected card.
    strong: Color32,
    /// Card subtitles.
    text: Color32,
    /// Third lines, labels and the plain connectors.
    weak: Color32,
    /// Blue-ish informational text: a module's "mod · M2" line, the module
    /// list, the unfold toggles.
    info: Color32,
    /// An outline with no topic colour (the root and examples cards).
    outline: Color32,
    /// A lit call: from the selected module or function, and into it.
    lit_out: Color32,
    lit_in: Color32,
}

impl Palette {
    /// The palette for `v`: the dark one when `v.dark_mode`, else derived
    /// from `v`'s own fills and text colours.
    pub(crate) fn of(v: &egui::Visuals) -> Self {
        if v.dark_mode {
            return Palette {
                dark: true,
                bg: v.panel_fill,
                root: Color32::from_rgb(40, 46, 58),
                card: Color32::from_rgb(28, 32, 42),
                examples: Color32::from_rgb(34, 36, 44),
                list: Color32::from_rgba_unmultiplied(20, 24, 32, 240),
                strong: Color32::WHITE,
                text: Color32::from_gray(200),
                weak: Color32::from_gray(170),
                info: Color32::from_rgb(190, 210, 255),
                outline: Color32::from_gray(160),
                lit_out: Color32::from_rgb(120, 190, 255),
                lit_in: Color32::from_rgb(255, 170, 90),
            };
        }
        // Fills are the panel colour shaded towards the text colour, so they
        // keep the theme's hue (cream under Gruvbox light). Not
        // `faint_bg_color`: egui's light theme makes it a transparent tint.
        let (bg, ink) = (v.panel_fill, v.strong_text_color());
        let shade = |k: u16| {
            let mix = |a: u8, b: u8| ((a as u16 * (100 - k) + b as u16 * k) / 100) as u8;
            Color32::from_rgb(mix(bg.r(), ink.r()), mix(bg.g(), ink.g()), mix(bg.b(), ink.b()))
        };
        Palette {
            dark: false,
            bg,
            root: shade(16),
            card: shade(6),
            examples: shade(10),
            list: v.window_fill,
            strong: v.strong_text_color(),
            text: v.text_color(),
            weak: v.weak_text_color(),
            info: v.hyperlink_color,
            outline: v.weak_text_color(),
            lit_out: v.hyperlink_color,
            lit_in: Color32::from_rgb(196, 98, 16),
        }
    }

    /// A topic colour as a card's fill: darkened to 35 % on dark, a 22 %
    /// tint over the background on light.
    fn fill_of(&self, c: Color32) -> Color32 {
        if self.dark {
            return Color32::from_rgb((c.r() as u16 * 35 / 100) as u8, (c.g() as u16 * 35 / 100) as u8, (c.b() as u16 * 35 / 100) as u8);
        }
        let mix = |a: u8, b: u8| ((a as u16 * 22 + b as u16 * 78) / 100) as u8;
        Color32::from_rgb(mix(c.r(), self.bg.r()), mix(c.g(), self.bg.g()), mix(c.b(), self.bg.b()))
    }

    /// A bright accent (a review state, a topic) used as text: unchanged on
    /// dark, darkened to 60 % on light so it reads on a light card.
    fn on_card(&self, c: Color32) -> Color32 {
        if self.dark {
            return c;
        }
        Color32::from_rgb((c.r() as u16 * 60 / 100) as u8, (c.g() as u16 * 60 / 100) as u8, (c.b() as u16 * 60 / 100) as u8)
    }
}

fn topic_of(map: &CodeMap, krate: &str) -> Topic {
    map.get(krate).map(|c| c.topic).unwrap_or(Topic::Utility)
}

/// Text lines in a card, scaled with the zoom and clipped to the card;
/// skipped when too small to read. Returns each line's drawn rectangle
/// (`None` when skipped), so a caller can continue a line in another colour.
fn card_text(painter: &egui::Painter, r: Rect, lines: &[(&str, f32, Color32)], scale: f32) -> Vec<Option<Rect>> {
    let p = painter.with_clip_rect(r.shrink(1.0));
    // Zoomed far out, the first line (the name) is kept at 8 pt, clipped to
    // the card, and the others dropped, so the whole map stays legible.
    let small = lines.first().is_some_and(|l| l.1 * scale < 8.0);
    let sizes: Vec<f32> = lines
        .iter()
        .enumerate()
        .map(|(i, l)| if small { if i == 0 && r.height() >= 8.0 { 8.0 } else { 0.0 } } else { l.1 * scale })
        .collect();
    let total: f32 = sizes.iter().map(|s| s * 1.25).sum();
    let mut y = r.center().y - 0.5 * total;
    let mut drawn = Vec::with_capacity(lines.len());
    for ((text, _, colour), s) in lines.iter().zip(sizes) {
        drawn.push((s >= 5.0).then(|| p.text(Pos2::new(r.left() + (8.0 * scale).clamp(2.0, 12.0), y + 0.5 * s * 1.25), Align2::LEFT_CENTER, *text, font(s), *colour)));
        y += s * 1.25;
    }
    drawn
}

/// Green for reviewed, red for to review: the module cards' counts and the
/// unreviewed function cards (maintainer, 2026-10-12).
const REVIEWED: Color32 = Color32::from_rgb(90, 190, 110);
const TO_REVIEW: Color32 = Color32::from_rgb(220, 80, 80);

/// Continue the line drawn at `after` with "· N reviewed" in green and
/// "· M to review" in red (each only when non-zero), clipped to the card.
/// GUI drawing code (exempt from the test rule; the counts are
/// [`model::Facts::review_split`], tested).
fn review_counts(painter: &egui::Painter, pal: &Palette, card: Rect, after: Option<Rect>, size: f32, (done, todo): (usize, usize)) {
    let Some(after) = after else { return };
    let p = painter.with_clip_rect(card.shrink(1.0));
    let mut x = after.right();
    for (n, what, colour) in [(done, "reviewed", REVIEWED), (todo, "to review", TO_REVIEW)] {
        if n > 0 {
            x = p.text(Pos2::new(x, after.center().y), Align2::LEFT_CENTER, format!(" \u{b7} {n} {what}"), font(size), pal.on_card(colour)).right();
        }
    }
}

fn curve(painter: &egui::Painter, cam: &Camera, rect: Rect, c: [Point; 4], stroke: Stroke) {
    let pts = c.map(|p| cam.to_screen(rect, p.x, p.y));
    painter.add(Shape::CubicBezier(egui::epaint::CubicBezierShape::from_points_stroke(pts, false, Color32::TRANSPARENT, stroke)));
}

/// A small filled (or hollow) triangle pointing `right` or left, the
/// unfold toggles. Drawn, not a glyph: fonts may lack ▸.
fn triangle(painter: &egui::Painter, c: Pos2, size: f32, right: bool, down: bool, filled: bool, colour: Color32) {
    let h = 0.5 * size;
    let pts = if down {
        vec![Pos2::new(c.x - h, c.y - 0.5 * h), Pos2::new(c.x + h, c.y - 0.5 * h), Pos2::new(c.x, c.y + 0.6 * h)]
    } else if right {
        vec![Pos2::new(c.x - 0.5 * h, c.y - h), Pos2::new(c.x + 0.6 * h, c.y), Pos2::new(c.x - 0.5 * h, c.y + h)]
    } else {
        vec![Pos2::new(c.x + 0.5 * h, c.y - h), Pos2::new(c.x - 0.6 * h, c.y), Pos2::new(c.x + 0.5 * h, c.y + h)]
    };
    let fill = if filled { colour } else { Color32::TRANSPARENT };
    painter.add(Shape::convex_polygon(pts, fill, Stroke::new(1.5, colour)));
}

fn centred(p: Point, size: (f64, f64)) -> WorldRect {
    [p.x - 0.5 * size.0, p.y - 0.5 * size.1, p.x + 0.5 * size.0, p.y + 0.5 * size.1]
}

fn bounds(rects: impl Iterator<Item = WorldRect>) -> WorldRect {
    rects.fold([f64::MAX, f64::MAX, f64::MIN, f64::MIN], |b, r| [b[0].min(r[0]), b[1].min(r[1]), b[2].max(r[2]), b[3].max(r[3])])
}

fn screen(cam: &Camera, rect: Rect, w: WorldRect) -> Rect {
    Rect::from_min_max(cam.to_screen(rect, w[0], w[1]), cam.to_screen(rect, w[2], w[3]))
}

impl CodeReview {
    pub(crate) fn canvas(&mut self, ui: &mut egui::Ui, snap: &Snap) {
        let (resp, painter) = ui.allocate_painter(ui.available_size(), Sense::click_and_drag());
        let rect = resp.rect;
        let mut hits: Vec<(Rect, Hit)> = Vec::new();
        let level = self.place.level.clone();
        let subject = match &level {
            Level::Map => self.draw_map(&painter, rect, snap, &mut hits),
            Level::Crate(k) => self.draw_crate(&painter, rect, snap, k, &mut hits),
            Level::Module { krate, file } => self.draw_module(&painter, rect, snap, krate, file, &mut hits),
        };
        let cam = self.cams.entry(level.clone()).or_default();
        // The code map (3.8:1) opens fitted to its height (2026-10-06).
        cam.open_to_height = level == Level::Map;
        if let Some(s) = subject {
            if cam.subject != s {
                cam.subject = s;
                cam.home = [0.5 * (s[0] + s[2]), 0.5 * (s[1] + s[3])];
                if !cam.touched {
                    cam.fitted = false;
                }
            }
        }
        cam.handle_input(ui, &resp);
        let pointer = resp.interact_pointer_pos().or(resp.hover_pos());
        let hit_at = |p: Option<Pos2>| p.and_then(|p| hits.iter().rev().find(|(r, _)| r.contains(p)).map(|(_, h)| h.clone()));
        if resp.clicked() {
            match hit_at(pointer) {
                // A crate on the code map asks where to go (2026-10-06).
                Some(Hit::MapCrate(c)) => {
                    let at = pointer.unwrap_or(rect.center());
                    self.crate_choice = Some((c, at, ui.ctx().cumulative_pass_nr()));
                }
                Some(h) => self.click(h, snap),
                None => {}
            }
        }
        if resp.secondary_clicked() {
            self.menu = hit_at(pointer);
        }
        if let Some(p) = pointer {
            if let Some(h) = hit_at(Some(p)) {
                if resp.hovered() {
                    if let Some(tip) = self.tooltip(&h, snap) {
                        resp.clone().on_hover_text_at_pointer(tip);
                    }
                }
            }
        }
        resp.context_menu(|ui| self.menu_ui(ui, snap));
        self.crate_choice_ui(ui, snap);
        self.cams.entry(level).or_default().buttons(ui, rect);
        if !self.panel_open {
            let b = Rect::from_min_size(rect.left_top() + Vec2::new(8.0, 8.0), Vec2::new(104.0, 36.0));
            if ui.put(b, egui::Button::new("Controls »")).clicked() {
                self.panel_open = true;
            }
        }
        if subject.is_none() || !snap.errors.is_empty() {
            let msg = if snap.map.is_none() && snap.errors.is_empty() { "Loading the code map…".to_string() } else if let Some(e) = snap.errors.first() { e.clone() } else { self.loading_message(snap) };
            let text = if ui.visuals().dark_mode { Color32::from_rgb(200, 205, 215) } else { ui.visuals().text_color() };
            painter.text(rect.center(), Align2::CENTER_CENTER, msg, font(16.0), text);
        }
    }

    fn loading_message(&self, snap: &Snap) -> String {
        match &self.place.level {
            Level::Crate(k) | Level::Module { krate: k, .. } => self.load_state(snap, k),
            Level::Map => String::new(),
        }
    }

    fn tooltip(&self, h: &Hit, snap: &Snap) -> Option<String> {
        let map = snap.map.as_ref()?;
        match h {
            Hit::MapCrate(c) => map.get(c).map(|c| kovan_common::code_map::svg::tooltip(map, c)),
            Hit::MapToggle(_) => Some("Show or hide this crate's top-level modules".into()),
            Hit::TreeToggle(_) => Some("Fold or unfold this module's submodules".into()),
            Hit::FnToggle(_, Dir::Callees) => Some("Unfold the functions this one calls".into()),
            Hit::FnToggle(_, Dir::Callers) => Some("Unfold the functions that call this one".into()),
            Hit::Function(id) => {
                let slice = snap.slice_of(id)?;
                let (_, f, _) = model::find_function(&slice.krate, id)?;
                Some(format!("{}\n{}\n{}", f.signature, f.doc, id))
            }
            Hit::TreeModule(f) | Hit::MapModule { file: f, .. } => Some(f.clone()),
            Hit::ModuleCard => None,
            Hit::Label(t) => Some(t.clone()),
        }
    }

    fn click(&mut self, h: Hit, snap: &Snap) {
        let Some(map) = snap.map.clone() else { return };
        match h {
            Hit::MapCrate(c) => self.go(Place::at(Level::Crate(c))),
            Hit::MapToggle(c) => {
                if !self.map_expanded.remove(&c) {
                    self.map_expanded.insert(c);
                }
            }
            Hit::MapModule { krate, file } => self.go(Place::at(Level::Module { krate, file })),
            Hit::TreeModule(file) => {
                if self.selected_module.as_deref() == Some(file.as_str()) {
                    if let Some(k) = crate_of(&map, &file) {
                        let k = k.to_string();
                        self.go(Place::at(Level::Module { krate: k, file }));
                    }
                } else {
                    self.selected_module = Some(file);
                }
            }
            Hit::TreeToggle(file) => {
                if !self.collapsed.remove(&file) {
                    self.collapsed.insert(file);
                }
            }
            // Left click opens the source panel, which carries the
            // function's actions too: the maintainer could not reach the
            // right-click menu in the browser (2026-10-06). It stays as well.
            Hit::Function(id) => {
                let mut p = self.place.clone();
                p.selected = Some(id.clone());
                self.replace(p);
                self.source = Some(id);
                self.sync_hash();
            }
            Hit::FnToggle(id, dir) => {
                let mut p = self.place.clone();
                let key = (id, dir);
                if !p.expanded.remove(&key) {
                    p.expanded.insert(key);
                }
                self.go(p);
            }
            Hit::ModuleCard | Hit::Label(_) => {}
        }
    }

    fn menu_ui(&mut self, ui: &mut egui::Ui, snap: &Snap) {
        let Some(h) = self.menu.clone() else {
            ui.weak("Click a card for its panel");
            return;
        };
        let Some(map) = snap.map.clone() else { return };
        match h {
            Hit::Function(id) | Hit::FnToggle(id, _) => {
                ui.label(egui::RichText::new(model::short_name(&id)).monospace().strong());
                if ui.button("View source").clicked() {
                    let mut p = self.place.clone();
                    p.selected = Some(id.clone());
                    self.replace(p);
                    self.source = Some(id.clone());
                    self.sync_hash();
                    ui.close();
                }
                if self.function_actions(ui, &id, snap, &map) {
                    ui.close();
                }
                // No Review here: web-kovan is read-only (stamping is desktop kovan's, #740).
            }
            Hit::TreeModule(file) | Hit::MapModule { file, .. } => {
                if ui.button("Open module").clicked() {
                    if let Some(k) = crate_of(&map, &file) {
                        let k = k.to_string();
                        self.go(Place::at(Level::Module { krate: k, file }));
                    }
                    ui.close();
                }
            }
            Hit::MapCrate(c) | Hit::MapToggle(c) => {
                if self.crate_actions(ui, &c, snap) {
                    ui.close();
                }
                let on = self.map_expanded.contains(&c);
                if ui.button(if on { "Hide modules" } else { "Show modules" }).clicked() {
                    self.click(Hit::MapToggle(c), snap);
                    ui.close();
                }
            }
            Hit::TreeToggle(_) | Hit::ModuleCard | Hit::Label(_) => {
                ui.close();
            }
        }
    }

    /// Where a crate on the code map can go: its module root (the crate
    /// view) or its deep dive, when the site has one. Shared by the
    /// left-click popup and the right-click menu. True when one was taken.
    fn crate_actions(&mut self, ui: &mut egui::Ui, krate: &str, snap: &Snap) -> bool {
        let mut taken = false;
        if ui.button("Module root").on_hover_text("This crate's modules, laid out as its file tree").clicked() {
            self.go(Place::at(Level::Crate(krate.to_string())));
            taken = true;
        }
        let dir = snap.map.as_ref().and_then(|m| m.get(krate)).and_then(|c| c.dir.clone());
        let books = match (&snap.site_links, &dir) {
            (Some(l), Some(d)) => l.deep_dives_of(d).into_iter().cloned().collect(),
            _ => Vec::new(),
        };
        if books.is_empty() {
            ui.add_enabled(false, egui::Button::new("Deep dive")).on_disabled_hover_text("This crate has no deep dive yet");
        }
        let root = self.store.site_root();
        for b in &books {
            let label = if books.len() == 1 { "Deep dive".to_string() } else { format!("Deep dive: {}", b.url.trim_end_matches('/').rsplit('/').next().unwrap_or(&b.url)) };
            if ui.button(label).on_hover_text(format!("{root}{}", b.url)).clicked() {
                ui.ctx().open_url(egui::OpenUrl::same_tab(format!("{root}{}", b.url)));
                taken = true;
            }
        }
        taken
    }

    /// The popup a left click on a crate opens, at the click.
    fn crate_choice_ui(&mut self, ui: &mut egui::Ui, snap: &Snap) {
        let Some((krate, at, opened)) = self.crate_choice.clone() else { return };
        let mut taken = false;
        let shown = egui::Area::new(egui::Id::new("kw_crate_choice"))
            .order(egui::Order::Foreground)
            .fixed_pos(at + Vec2::new(4.0, 4.0))
            .constrain(true)
            .show(ui.ctx(), |ui| {
                egui::Frame::popup(ui.style()).show(ui, |ui| {
                    ui.label(egui::RichText::new(&krate).strong());
                    ui.spacing_mut().button_padding = Vec2::new(10.0, 6.0);
                    taken = self.crate_actions(ui, &krate, snap);
                });
            });
        let escape = ui.input(|i| i.key_pressed(egui::Key::Escape));
        let elsewhere = shown.response.clicked_elsewhere() && ui.ctx().cumulative_pass_nr() > opened;
        if taken || escape || elsewhere {
            self.crate_choice = None;
        }
    }

    /// A function's actions, as buttons: shared by the right-click menu and
    /// the source panel's header (reached by a left click). True when one
    /// was taken.
    pub(crate) fn function_actions(&mut self, ui: &mut egui::Ui, id: &str, snap: &Snap, map: &CodeMap) -> bool {
        let mut taken = false;
        if let Level::Module { file, .. } = &self.place.level {
            if model::file_of(id) != file && ui.button("Open its module").clicked() {
                self.open_function(id, false, map);
                taken = true;
            }
        }
        for (dir, label) in [(Dir::Callees, "callees"), (Dir::Callers, "callers")] {
            let on = self.place.expanded.contains(&(id.to_string(), dir));
            let tip = if on { "Fold them back on the canvas" } else { "Unfold them on the canvas" };
            if ui.button(format!("{} {label}", if on { "Fold" } else { "Unfold" })).on_hover_text(tip).clicked() {
                self.click(Hit::FnToggle(id.to_string(), dir), snap);
                taken = true;
            }
        }
        if ui.button("Show walks through here").clicked() {
            self.status = Some("Walkthroughs are not published yet (#741); none pass through this function.".into());
            taken = true;
        }
        // Desktop only (#857): a stamp that went stale only because a callee,
        // or a type or constant the function uses, changed is re-confirmed
        // without the wizard.
        if self.mode.can_stamp() {
            let inherited = snap.facts().review(id).kind() == kovan_common::review::state::StateKind::InheritedStale;
            if ui
                .add_enabled(inherited, egui::Button::new("Quick stamp (something it uses changed)\u{2026}"))
                .on_hover_text("This function's own code did not change; a function it calls, or a type or constant it uses, did. Shows what changed there and renews your stamp with the same answers, without the questions.")
                .on_disabled_hover_text("Only when the stamp is stale because something this function calls or uses changed. If its own code changed, review it again with Stamp.")
                .clicked()
            {
                self.quick_stamp(snap, id);
                taken = true;
            }
        }
        // Desktop only (#839): connect a reviewed function to a concept.
        if self.mode.can_stamp() {
            let reviewed = snap.facts().review(id).holds();
            if ui
                .add_enabled(reviewed, egui::Button::new("Add connection\u{2026}"))
                .on_hover_text("Connect this function to a concept of the standard corpus")
                .on_disabled_hover_text("Available once the function is reviewed (its stamp is valid, or waits only for a workspace test)")
                .clicked()
            {
                match self.function_ref(snap, id) {
                    Some(f) => self.host_request = Some(super::HostRequest::AddConnection(f)),
                    None => self.status = Some("The function is not in a loaded workspace crate.".into()),
                }
                taken = true;
            }
        }
        if ui.button("Copy link").clicked() {
            let link = crate::platform::url_with_hash(&model::DeepLink::Function { id: id.to_string(), source: false }.to_hash());
            ui.ctx().copy_text(link);
            taken = true;
        }
        taken
    }

    // ---- the code map -----------------------------------------------------

    fn draw_map(&self, painter: &egui::Painter, rect: Rect, snap: &Snap, hits: &mut Vec<(Rect, Hit)>) -> Option<WorldRect> {
        let (map, layout) = (snap.map.as_ref()?, snap.layout.as_ref()?);
        let cam = self.cams.get(&Level::Map).copied().unwrap_or_default();
        let z = cam.scale as f32;
        let pal = Palette::of(&painter.ctx().global_style().visuals);
        let b = layout.bounds;
        // Frames.
        for f in &layout.frames {
            let r = cam.rect(rect, f.rect.x, f.rect.y, f.rect.w, f.rect.h);
            let c = hex(topic_colour(f.topic));
            painter.rect(r, 8.0 * z, c.gamma_multiply(0.08), Stroke::new(1.5, c.gamma_multiply(0.7)), StrokeKind::Inside);
            if 13.0 * z >= 5.0 && f.kind != FrameKind::App {
                painter.text(r.left_top() + Vec2::new(10.0 * z, 8.0 * z), Align2::LEFT_TOP, &f.title, font(13.0 * z), pal.on_card(c));
            }
        }
        for l in &layout.labels {
            if 12.0 * z >= 5.0 {
                let at = cam.to_screen(rect, l.x, l.y);
                painter.text(at, Align2::CENTER_CENTER, &l.text, font(12.0 * z), if pal.dark { Color32::from_gray(150) } else { pal.weak });
                if !l.tip.is_empty() {
                    hits.push((Rect::from_center_size(at, Vec2::new(36.0 * z.max(0.5), 18.0 * z.max(0.5))), Hit::Label(l.tip.clone())));
                }
            }
        }
        // Dependency edges, faint.
        let size = (kovan_common::code_map::layout::CARD_W, kovan_common::code_map::layout::CARD_H);
        for e in &map.edges {
            if let (Some(a), Some(bc)) = (layout.card(&e.from), layout.card(&e.to)) {
                let (ax, ay) = a.rect.centre();
                let (bx, by) = bc.rect.centre();
                if let Some(c) = connector_sized(Point::new(ax, ay), size, Point::new(bx, by), size) {
                    curve(painter, &cam, rect, c, Stroke::new(1.0, Color32::from_rgba_unmultiplied(160, 170, 190, 50)));
                }
            }
        }
        // Root card.
        let rr = cam.rect(rect, layout.root.x, layout.root.y, layout.root.w, layout.root.h);
        painter.rect(rr, 8.0 * z, pal.root, Stroke::new(1.5, pal.outline), StrokeKind::Inside);
        card_text(painter, rr, &[(map.root.as_str(), 20.0, pal.strong)], z);
        let in_index = |c: &str| snap.index.as_ref().is_some_and(|i| i.crates.iter().any(|e| e.name == c));
        for card in &layout.cards {
            let Some(c) = map.get(&card.name) else { continue };
            let r = cam.rect(rect, card.rect.x, card.rect.y, card.rect.w, card.rect.h);
            let col = hex(topic_colour(c.topic));
            painter.rect(r, 6.0 * z, pal.fill_of(col), Stroke::new(1.5, col), StrokeKind::Inside);
            let sub = card_subtitle(c);
            let fns = snap.index.as_ref().and_then(|i| i.crates.iter().find(|e| e.name == c.name)).map(|e| format!("{} modules · {} fn", e.modules, e.functions));
            let mut lines = vec![(c.name.as_str(), 14.0, pal.strong), (sub.as_str(), 11.0, pal.text)];
            if let Some(f) = &fns {
                lines.push((f.as_str(), 10.0, pal.weak));
            }
            card_text(painter, r, &lines, z);
            hits.push((r, Hit::MapCrate(c.name.clone())));
            if in_index(&c.name) {
                let t = Rect::from_center_size(Pos2::new(r.right() - 14.0 * z, r.bottom() - 12.0 * z), Vec2::splat((20.0 * z).max(14.0)));
                let open = self.map_expanded.contains(&c.name);
                triangle(painter, t.center(), 10.0 * z.max(0.6), true, open, open, pal.strong);
                hits.push((t, Hit::MapToggle(c.name.clone())));
            }
        }
        // Expanded crates: their top-level modules in place, under the card.
        for name in &self.map_expanded {
            let Some(card) = layout.card(name) else { continue };
            let r = cam.rect(rect, card.rect.x, card.rect.y, card.rect.w, card.rect.h);
            let rows: Vec<(String, String, usize)> = match snap.slice(name) {
                Some(s) => {
                    let t = ModTree::build(&s.krate, false);
                    let mut v: Vec<(String, String, usize)> = t.top_level().iter().map(|&i| (t.nodes[i].name.clone(), t.nodes[i].file.clone(), t.nodes[i].functions)).collect();
                    if !t.example_roots.is_empty() {
                        v.push((format!("{} examples", t.example_roots.len()), String::new(), 0));
                    }
                    v
                }
                None => vec![("loading…".into(), String::new(), 0)],
            };
            // The list is an overlay at screen size (rows 20 pt, at least
            // 200 pt wide), so it stays readable and tappable at any zoom.
            let row_h = 20.0;
            let list = Rect::from_min_size(r.left_bottom() + Vec2::new(0.0, 2.0), Vec2::new(r.width().max(200.0), row_h * rows.len() as f32 + 4.0));
            painter.rect(list, 4.0, pal.list, Stroke::new(1.0, if pal.dark { Color32::from_gray(110) } else { pal.outline }), StrokeKind::Inside);
            for (i, (label, file, n)) in rows.iter().enumerate() {
                let rr = Rect::from_min_size(list.left_top() + Vec2::new(0.0, 2.0 + row_h * i as f32), Vec2::new(list.width(), row_h));
                let text = if *n > 0 { format!("{label}  ({n} fn)") } else { label.clone() };
                painter.with_clip_rect(rr).text(rr.left_center() + Vec2::new(8.0, 0.0), Align2::LEFT_CENTER, text, mono(11.0), pal.info);
                if !file.is_empty() {
                    hits.push((rr, Hit::MapModule { krate: name.clone(), file: file.clone() }));
                }
            }
        }
        Some([b.x, b.y, b.x + b.w, b.y + b.h])
    }

    // ---- the crate view ---------------------------------------------------

    fn draw_crate(&self, painter: &egui::Painter, rect: Rect, snap: &Snap, krate: &str, hits: &mut Vec<(Rect, Hit)>) -> Option<WorldRect> {
        let map = snap.map.as_ref()?;
        let slice = snap.slice(krate)?;
        let tree = ModTree::build(&slice.krate, self.show_tests);
        let facts = snap.facts();
        let tl = model::tree_layout(&tree, &self.collapsed);
        let cam = self.cams.get(&self.place.level).copied().unwrap_or_default();
        let z = cam.scale as f32;
        let col = hex(topic_colour(topic_of(map, krate)));
        let pal = Palette::of(&painter.ctx().global_style().visuals);
        let size_of = |it: TreeItem| if it == TreeItem::Crate { (MOD_CARD.0 * 1.15, MOD_CARD.1 * 1.25) } else { MOD_CARD };
        // Module-to-module calls, faint; lit for the selected module.
        let pos_of_file = |f: &str| tree.find(f).and_then(|i| tl.position(TreeItem::Module(i)).or_else(|| (tree.lib_root == Some(i)).then(|| Point::new(0.0, 0.0))));
        let sel = self.selected_module.clone();
        for a in &slice.module_calls {
            if a.from == a.to {
                continue;
            }
            let (Some(p), Some(q)) = (pos_of_file(&a.from), pos_of_file(&a.to)) else { continue };
            let lit = sel.as_deref() == Some(a.from.as_str()) || sel.as_deref() == Some(a.to.as_str());
            if !lit && slice.module_calls.len() > 400 {
                continue;
            }
            let stroke = if lit {
                Stroke::new(2.0, if sel.as_deref() == Some(a.from.as_str()) { pal.lit_out } else { pal.lit_in })
            } else {
                Stroke::new(1.0, Color32::from_rgba_unmultiplied(150, 160, 190, 28))
            };
            painter.line_segment([cam.to_screen(rect, p.x, p.y), cam.to_screen(rect, q.x, q.y)], stroke);
        }
        // Tree connectors.
        for (item, p, parent) in &tl.cards {
            if let Some(pp) = parent.and_then(|x| tl.position(x)) {
                if let Some(c) = connector_sized(pp, size_of(parent.unwrap()), *p, size_of(*item)) {
                    curve(painter, &cam, rect, c, Stroke::new(1.2, if pal.dark { Color32::from_gray(110) } else { pal.weak }));
                }
            }
        }
        for (item, p, _) in &tl.cards {
            let r = screen(&cam, rect, centred(*p, size_of(*item)));
            match item {
                TreeItem::Crate => {
                    let file = tree.lib_root.map(|i| tree.nodes[i].file.clone());
                    let selected = file.is_some() && sel == file;
                    painter.rect(r, 8.0 * z, pal.fill_of(col), Stroke::new(if selected { 3.0 } else { 2.0 }, if selected { pal.strong } else { col }), StrokeKind::Inside);
                    let ids = tree.lib_root.map(|i| tree.nodes[i].fn_ids.as_slice()).unwrap_or(&[]);
                    let sub = if ids.is_empty() { format!("M{} · lib.rs · 0 fn", slice.krate.maturity.unwrap_or(0)) } else { format!("M{} · lib.rs", slice.krate.maturity.unwrap_or(0)) };
                    let drawn = card_text(painter, r, &[(krate, 17.0, pal.strong), (sub.as_str(), 11.0, pal.text)], z);
                    review_counts(painter, &pal, r, drawn.get(1).copied().flatten(), 11.0 * z, facts.review_split(ids));
                    if let Some(f) = file {
                        hits.push((r, Hit::TreeModule(f)));
                    }
                }
                TreeItem::Examples => {
                    painter.rect(r, 8.0 * z, pal.examples, Stroke::new(1.5, if pal.dark { Color32::from_gray(140) } else { pal.outline }), StrokeKind::Inside);
                    let n = format!("{} examples", tree.example_roots.len());
                    let (title, sub) = if pal.dark { (Color32::from_gray(230), Color32::from_gray(180)) } else { (pal.strong, pal.text) };
                    card_text(painter, r, &[("examples", 14.0, title), (n.as_str(), 11.0, sub)], z);
                }
                TreeItem::Module(i) => {
                    let node = &tree.nodes[*i];
                    let selected = sel.as_deref() == Some(node.file.as_str());
                    painter.rect(r, 6.0 * z, pal.card, Stroke::new(if selected { 3.0 } else { 1.5 }, if selected { pal.strong } else { col }), StrokeKind::Inside);
                    let rel = node.file.strip_prefix(&format!("{}/", slice.krate.dir)).unwrap_or(&node.file).to_string();
                    let m = node.maturity.or(slice.krate.maturity).unwrap_or(0);
                    let sub = if node.fn_ids.is_empty() { format!("mod · M{m} · 0 fn") } else { format!("mod · M{m}") };
                    let info = if pal.dark { Color32::from_rgb(170, 200, 240) } else { pal.info };
                    let drawn = card_text(painter, r, &[(node.name.as_str(), 14.0, pal.strong), (rel.as_str(), 10.0, pal.weak), (sub.as_str(), 10.0, info)], z);
                    review_counts(painter, &pal, r, drawn.get(2).copied().flatten(), 10.0 * z, facts.review_split(&node.fn_ids));
                    hits.push((r, Hit::TreeModule(node.file.clone())));
                    if !node.children.is_empty() {
                        let right = p.x >= 0.0;
                        let tc = if right { Pos2::new(r.right() + 10.0 * z, r.center().y) } else { Pos2::new(r.left() - 10.0 * z, r.center().y) };
                        let folded = self.collapsed.contains(&node.file);
                        triangle(painter, tc, 12.0 * z.max(0.6), right, !folded, true, pal.text);
                        hits.push((Rect::from_center_size(tc, Vec2::splat((20.0 * z).max(16.0))), Hit::TreeToggle(node.file.clone())));
                    }
                }
            }
            if selected_hint(item, &tree, &sel) && 11.0 * z >= 5.0 {
                painter.text(r.center_bottom() + Vec2::new(0.0, 4.0 * z), Align2::CENTER_TOP, "tap again to open", font(11.0 * z), if pal.dark { Color32::from_gray(220) } else { pal.text });
            }
        }
        Some(bounds(tl.cards.iter().map(|(it, p, _)| centred(*p, size_of(*it)))).map_pad(40.0))
    }

    // ---- the module view --------------------------------------------------

    fn draw_module(&self, painter: &egui::Painter, rect: Rect, snap: &Snap, krate: &str, file: &str, hits: &mut Vec<(Rect, Hit)>) -> Option<WorldRect> {
        let map = snap.map.as_ref()?;
        let slice = snap.slice(krate)?;
        let (module, _, _) = model::find_module(&slice.krate, file)?;
        let facts = snap.facts();
        // A deep-linked test joins the ring even while test code is hidden (#867).
        let ring_ids = model::ring_ids(module, self.show_tests, self.place.selected.as_deref());
        let n_reviewables = model::reviewables(module, self.show_tests).count();
        let slice_for = |id: &str| -> Option<Arc<CrateSlice>> { snap.slice_of(id).cloned() };
        let hl = model::hierarchy_layout(
            &ring_ids,
            &self.place.expanded,
            |id: &str| slice_for(id).map(|s| facts.callees(map, &s, id)),
            |id: &str| slice_for(id).map(|s| facts.callers(&s, id)),
        );
        let cam = self.cams.get(&self.place.level).copied().unwrap_or_default();
        let z = cam.scale as f32;
        let home = topic_of(map, krate);
        let pal = Palette::of(&painter.ctx().global_style().visuals);
        // Centre card to ring.
        for (_, p) in &hl.ring {
            if let Some(c) = connector_sized(Point::new(0.0, 0.0), CENTRE_CARD_SIZE, *p, CARD_SIZE) {
                curve(painter, &cam, rect, c, Stroke::new(1.0, if pal.dark { Color32::from_gray(80) } else { pal.weak }));
            }
        }
        // Call edges.
        let sel = self.place.selected.clone();
        for (a, b) in &hl.edges {
            let (Some(p), Some(q)) = (hl.position(a), hl.position(b)) else { continue };
            let lit = sel.as_deref() == Some(a.as_str()) || sel.as_deref() == Some(b.as_str());
            let call = if pal.dark { Color32::from_rgb(130, 200, 255) } else { pal.lit_out };
            let faint = if pal.dark { Color32::from_rgba_unmultiplied(130, 200, 255, 110) } else { call.gamma_multiply(110.0 / 255.0) };
            let stroke = Stroke::new(if lit { 2.2 } else { 1.3 }, if lit { call } else { faint });
            // A dot where the call arrives, on the callee's edge.
            let tip = match kovan_common::mindmap_view::connector(p, q) {
                Some(c) => {
                    curve(painter, &cam, rect, c, stroke);
                    c[3]
                }
                None => {
                    painter.line_segment([cam.to_screen(rect, p.x, p.y), cam.to_screen(rect, q.x, q.y)], stroke);
                    q
                }
            };
            painter.circle_filled(cam.to_screen(rect, tip.x, tip.y), 3.5 * z.max(0.7), stroke.color);
        }
        // The module card.
        let mr = screen(&cam, rect, centred(Point::new(0.0, 0.0), CENTRE_CARD_SIZE));
        let col = hex(topic_colour(home));
        painter.rect(mr, 8.0 * z, pal.fill_of(col), Stroke::new(2.0, col), StrokeKind::Inside);
        let name = file.rsplit('/').next().unwrap_or(file).to_string();
        let path = if module.path.is_empty() { format!("{krate} (target root)") } else { module.path.clone() };
        let n_items = module.items.iter().filter(|f| self.show_tests || !f.test).count();
        let sub = if n_items == 0 {
            format!("{n_reviewables} functions")
        } else {
            format!("{} functions · {n_items} types and constants", n_reviewables - n_items)
        };
        let path_colour = if pal.dark { Color32::from_gray(190) } else { pal.weak };
        card_text(painter, mr, &[(name.as_str(), 16.0, pal.strong), (path.as_str(), 10.0, path_colour), (sub.as_str(), 11.0, pal.text)], z);
        hits.push((mr, Hit::ModuleCard));
        let mut draw_fn = |id: &str, p: Point| {
            let r = screen(&cam, rect, centred(p, CARD_SIZE));
            let k = crate_of(map, id).unwrap_or("");
            let topic = topic_of(map, k);
            let tc = hex(topic_colour(topic));
            let review = facts.review(id);
            let selected = sel.as_deref() == Some(id);
            let home_fill = if pal.dark { Color32::from_rgb(30, 34, 44) } else { pal.card };
            let fill = if k == krate { home_fill } else { pal.fill_of(tc) };
            painter.rect(r, 6.0 * z, fill, Stroke::new(if selected { 3.5 } else { 2.0 }, if selected { pal.strong } else { review_colour(&review) }), StrokeKind::Inside);
            let found = snap.slice_of(id).and_then(|s| model::find_function(&s.krate, id).map(|(m, f, _)| (model::function_maturity(m, map, k), model::display_name(f), model::card_kind(f), f.test_fn)));
            let title = match &found {
                Some((_, n, _, _)) => n.clone(),
                None => model::short_name(id).to_string(),
            };
            let mat = found.as_ref().and_then(|f| f.0).map(|m| format!("M{m} · ")).unwrap_or_default();
            // A type or constant says what it is in plain words (#843), a
            // test that it is a test (#867).
            let what = found.as_ref().map(|f| f.2.clone()).unwrap_or_default();
            let line2 = format!("{what}{mat}{}", review.label());
            let mut lines = vec![(title.as_str(), 12.5, pal.strong), (line2.as_str(), 10.0, pal.on_card(review_colour(&review)))];
            if k != krate {
                lines.push((k, 9.5, pal.on_card(tc)));
            }
            card_text(painter, r, &lines, z);
            hits.push((r, Hit::Function(id.to_string())));
            let t = (16.0 * z).max(14.0);
            // A test is an entry point: nothing calls it, so no callers toggle.
            let entry = found.as_ref().is_some_and(|f| f.3);
            for (on, dir, right) in [(true, Dir::Callees, true), (!entry, Dir::Callers, false)] {
                if !on {
                    continue;
                }
                let c = if right { Pos2::new(r.right() + 0.6 * t, r.center().y) } else { Pos2::new(r.left() - 0.6 * t, r.center().y) };
                let open = self.place.expanded.contains(&(id.to_string(), dir));
                triangle(painter, c, 11.0 * z.max(0.6), right, false, open, if pal.dark { Color32::from_rgb(200, 220, 255) } else { pal.info });
                hits.push((Rect::from_center_size(c, Vec2::splat(t * 1.3)), Hit::FnToggle(id.to_string(), dir)));
            }
        };
        // Every card unfolds both ways (#867): a test among the callers
        // unfolds what it exercises.
        for (id, p) in hl.ring.iter().map(|r| (&r.0, r.1)).chain(hl.unfolded.iter().map(|u| (&u.0, u.1))) {
            draw_fn(id, p);
        }
        for id in &hl.waiting {
            if let Some(p) = hl.position(id) {
                painter.text(cam.to_screen(rect, p.x, p.y + 0.5 * CARD_SIZE.1 + 8.0), Align2::CENTER_TOP, "loading…", font(11.0), pal.text);
            }
        }
        Some(bounds(std::iter::once(centred(Point::new(0.0, 0.0), CENTRE_CARD_SIZE)).chain(hl.points().map(|p| centred(p, CARD_SIZE)))).map_pad(50.0))
    }
}

fn selected_hint(item: &TreeItem, tree: &ModTree, sel: &Option<String>) -> bool {
    let file = match item {
        TreeItem::Crate => tree.lib_root.map(|i| tree.nodes[i].file.as_str()),
        TreeItem::Module(i) => Some(tree.nodes[*i].file.as_str()),
        TreeItem::Examples => None,
    };
    file.is_some() && file == sel.as_deref()
}

trait Pad {
    fn map_pad(self, p: f64) -> WorldRect;
}

impl Pad for WorldRect {
    fn map_pad(self, p: f64) -> WorldRect {
        [self[0] - p, self[1] - p, self[2] + p, self[3] + p]
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// WCAG relative luminance of an opaque colour.
    fn luminance(c: Color32) -> f32 {
        let lin = |u: u8| {
            let x = u as f32 / 255.0;
            if x <= 0.03928 { x / 12.92 } else { ((x + 0.055) / 1.055).powf(2.4) }
        };
        0.2126 * lin(c.r()) + 0.7152 * lin(c.g()) + 0.0722 * lin(c.b())
    }

    fn contrast(a: Color32, b: Color32) -> f32 {
        let (x, y) = (luminance(a), luminance(b));
        (x.max(y) + 0.05) / (x.min(y) + 0.05)
    }

    /// Dark is what the site shows: the colours the cards were always drawn
    /// with, unchanged.
    #[test]
    fn the_dark_palette_keeps_the_original_card_colours() {
        let p = Palette::of(&egui::Visuals::dark());
        assert_eq!(p.root, Color32::from_rgb(40, 46, 58));
        assert_eq!(p.card, Color32::from_rgb(28, 32, 42));
        assert_eq!(p.strong, Color32::WHITE);
        let topic = Color32::from_rgb(200, 100, 40);
        assert_eq!(p.fill_of(topic), Color32::from_rgb(70, 35, 14));
        assert_eq!(p.on_card(topic), topic);
    }

    /// Under a light theme (desktop kovan's Gruvbox light, or egui's light)
    /// the cards are light and their text is readable on them.
    #[test]
    fn under_a_light_theme_the_cards_are_light_and_readable() {
        let mut gruvbox_light = egui::Visuals::light();
        gruvbox_light.panel_fill = Color32::from_rgb(0xf9, 0xf5, 0xd7);
        gruvbox_light.faint_bg_color = Color32::from_rgb(0xeb, 0xdb, 0xb2);
        gruvbox_light.widgets.inactive.bg_fill = Color32::from_rgb(0xeb, 0xdb, 0xb2);
        for v in [egui::Visuals::light(), gruvbox_light] {
            let p = Palette::of(&v);
            let topic = Color32::from_rgb(69, 133, 136);
            for fill in [p.root, p.card, p.examples, p.fill_of(topic)] {
                assert!(luminance(fill) > 0.5, "{fill:?} is a dark fill under a light theme");
                assert!(contrast(p.strong, fill) >= 4.5, "title on {fill:?}: {}", contrast(p.strong, fill));
                assert!(contrast(p.text, fill) >= 4.5, "subtitle on {fill:?}: {}", contrast(p.text, fill));
            }
            // A bright state colour as card text is darkened to read on the card.
            let green = Color32::from_rgb(90, 190, 110);
            assert!(contrast(p.on_card(green), p.card) > contrast(green, p.card));
        }
    }
}
