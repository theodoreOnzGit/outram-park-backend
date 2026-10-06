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

/// A topic colour darkened for a card's fill.
fn fill_of(c: Color32) -> Color32 {
    Color32::from_rgb((c.r() as u16 * 35 / 100) as u8, (c.g() as u16 * 35 / 100) as u8, (c.b() as u16 * 35 / 100) as u8)
}

fn topic_of(map: &CodeMap, krate: &str) -> Topic {
    map.get(krate).map(|c| c.topic).unwrap_or(Topic::Utility)
}

/// Text lines in a card, scaled with the zoom and clipped to the card;
/// skipped when too small to read.
fn card_text(painter: &egui::Painter, r: Rect, lines: &[(&str, f32, Color32)], scale: f32) {
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
    for ((text, _, colour), s) in lines.iter().zip(sizes) {
        if s >= 5.0 {
            p.text(Pos2::new(r.left() + (8.0 * scale).clamp(2.0, 12.0), y + 0.5 * s * 1.25), Align2::LEFT_CENTER, *text, font(s), *colour);
        }
        y += s * 1.25;
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
            if let Some(h) = hit_at(pointer) {
                self.click(h, snap);
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
        self.cams.entry(level).or_default().buttons(ui, rect);
        if !self.panel_open {
            let b = Rect::from_min_size(rect.left_top() + Vec2::new(8.0, 8.0), Vec2::new(104.0, 36.0));
            if ui.put(b, egui::Button::new("Controls »")).clicked() {
                self.panel_open = true;
            }
        }
        if subject.is_none() || !snap.errors.is_empty() {
            let msg = if snap.map.is_none() && snap.errors.is_empty() { "Loading the code map…".to_string() } else if let Some(e) = snap.errors.first() { e.clone() } else { self.loading_message(snap) };
            painter.text(rect.center(), Align2::CENTER_CENTER, msg, font(16.0), Color32::from_rgb(200, 205, 215));
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
                if ui.button("Open crate").clicked() {
                    self.go(Place::at(Level::Crate(c.clone())));
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
        let b = layout.bounds;
        // Frames.
        for f in &layout.frames {
            let r = cam.rect(rect, f.rect.x, f.rect.y, f.rect.w, f.rect.h);
            let c = hex(topic_colour(f.topic));
            painter.rect(r, 8.0 * z, c.gamma_multiply(0.08), Stroke::new(1.5, c.gamma_multiply(0.7)), StrokeKind::Inside);
            if 13.0 * z >= 5.0 && f.kind != FrameKind::App {
                painter.text(r.left_top() + Vec2::new(10.0 * z, 8.0 * z), Align2::LEFT_TOP, &f.title, font(13.0 * z), c);
            }
        }
        for l in &layout.labels {
            if 12.0 * z >= 5.0 {
                let at = cam.to_screen(rect, l.x, l.y);
                painter.text(at, Align2::CENTER_CENTER, &l.text, font(12.0 * z), Color32::from_gray(150));
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
        painter.rect(rr, 8.0 * z, Color32::from_rgb(40, 46, 58), Stroke::new(1.5, Color32::from_gray(160)), StrokeKind::Inside);
        card_text(painter, rr, &[(map.root.as_str(), 20.0, Color32::WHITE)], z);
        let in_index = |c: &str| snap.index.as_ref().is_some_and(|i| i.crates.iter().any(|e| e.name == c));
        for card in &layout.cards {
            let Some(c) = map.get(&card.name) else { continue };
            let r = cam.rect(rect, card.rect.x, card.rect.y, card.rect.w, card.rect.h);
            let col = hex(topic_colour(c.topic));
            painter.rect(r, 6.0 * z, fill_of(col), Stroke::new(1.5, col), StrokeKind::Inside);
            let sub = card_subtitle(c);
            let fns = snap.index.as_ref().and_then(|i| i.crates.iter().find(|e| e.name == c.name)).map(|e| format!("{} modules · {} fn", e.modules, e.functions));
            let mut lines = vec![(c.name.as_str(), 14.0, Color32::WHITE), (sub.as_str(), 11.0, Color32::from_gray(200))];
            if let Some(f) = &fns {
                lines.push((f.as_str(), 10.0, Color32::from_gray(170)));
            }
            card_text(painter, r, &lines, z);
            hits.push((r, Hit::MapCrate(c.name.clone())));
            if in_index(&c.name) {
                let t = Rect::from_center_size(Pos2::new(r.right() - 14.0 * z, r.bottom() - 12.0 * z), Vec2::splat((20.0 * z).max(14.0)));
                let open = self.map_expanded.contains(&c.name);
                triangle(painter, t.center(), 10.0 * z.max(0.6), true, open, open, Color32::WHITE);
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
            painter.rect(list, 4.0, Color32::from_rgba_unmultiplied(20, 24, 32, 240), Stroke::new(1.0, Color32::from_gray(110)), StrokeKind::Inside);
            for (i, (label, file, n)) in rows.iter().enumerate() {
                let rr = Rect::from_min_size(list.left_top() + Vec2::new(0.0, 2.0 + row_h * i as f32), Vec2::new(list.width(), row_h));
                let text = if *n > 0 { format!("{label}  ({n} fn)") } else { label.clone() };
                painter.with_clip_rect(rr).text(rr.left_center() + Vec2::new(8.0, 0.0), Align2::LEFT_CENTER, text, mono(11.0), Color32::from_rgb(190, 210, 255));
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
        let tl = model::tree_layout(&tree, &self.collapsed);
        let cam = self.cams.get(&self.place.level).copied().unwrap_or_default();
        let z = cam.scale as f32;
        let col = hex(topic_colour(topic_of(map, krate)));
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
                Stroke::new(2.0, if sel.as_deref() == Some(a.from.as_str()) { Color32::from_rgb(120, 190, 255) } else { Color32::from_rgb(255, 170, 90) })
            } else {
                Stroke::new(1.0, Color32::from_rgba_unmultiplied(150, 160, 190, 28))
            };
            painter.line_segment([cam.to_screen(rect, p.x, p.y), cam.to_screen(rect, q.x, q.y)], stroke);
        }
        // Tree connectors.
        for (item, p, parent) in &tl.cards {
            if let Some(pp) = parent.and_then(|x| tl.position(x)) {
                if let Some(c) = connector_sized(pp, size_of(parent.unwrap()), *p, size_of(*item)) {
                    curve(painter, &cam, rect, c, Stroke::new(1.2, Color32::from_gray(110)));
                }
            }
        }
        for (item, p, _) in &tl.cards {
            let r = screen(&cam, rect, centred(*p, size_of(*item)));
            match item {
                TreeItem::Crate => {
                    let file = tree.lib_root.map(|i| tree.nodes[i].file.clone());
                    let selected = file.is_some() && sel == file;
                    painter.rect(r, 8.0 * z, fill_of(col), Stroke::new(if selected { 3.0 } else { 2.0 }, if selected { Color32::WHITE } else { col }), StrokeKind::Inside);
                    let n = tree.lib_root.map(|i| tree.nodes[i].functions).unwrap_or(0);
                    let sub = format!("M{} · lib.rs · {n} fn", slice.krate.maturity.unwrap_or(0));
                    card_text(painter, r, &[(krate, 17.0, Color32::WHITE), (sub.as_str(), 11.0, Color32::from_gray(200))], z);
                    if let Some(f) = file {
                        hits.push((r, Hit::TreeModule(f)));
                    }
                }
                TreeItem::Examples => {
                    painter.rect(r, 8.0 * z, Color32::from_rgb(34, 36, 44), Stroke::new(1.5, Color32::from_gray(140)), StrokeKind::Inside);
                    let n = format!("{} examples", tree.example_roots.len());
                    card_text(painter, r, &[("examples", 14.0, Color32::from_gray(230)), (n.as_str(), 11.0, Color32::from_gray(180))], z);
                }
                TreeItem::Module(i) => {
                    let node = &tree.nodes[*i];
                    let selected = sel.as_deref() == Some(node.file.as_str());
                    painter.rect(r, 6.0 * z, Color32::from_rgb(28, 32, 42), Stroke::new(if selected { 3.0 } else { 1.5 }, if selected { Color32::WHITE } else { col }), StrokeKind::Inside);
                    let rel = node.file.strip_prefix(&format!("{}/", slice.krate.dir)).unwrap_or(&node.file).to_string();
                    let sub = format!("mod · M{} · {} fn", node.maturity.or(slice.krate.maturity).unwrap_or(0), node.functions);
                    card_text(painter, r, &[(node.name.as_str(), 14.0, Color32::WHITE), (rel.as_str(), 10.0, Color32::from_gray(170)), (sub.as_str(), 10.0, Color32::from_rgb(170, 200, 240))], z);
                    hits.push((r, Hit::TreeModule(node.file.clone())));
                    if !node.children.is_empty() {
                        let right = p.x >= 0.0;
                        let tc = if right { Pos2::new(r.right() + 10.0 * z, r.center().y) } else { Pos2::new(r.left() - 10.0 * z, r.center().y) };
                        let folded = self.collapsed.contains(&node.file);
                        triangle(painter, tc, 12.0 * z.max(0.6), right, !folded, true, Color32::from_gray(200));
                        hits.push((Rect::from_center_size(tc, Vec2::splat((20.0 * z).max(16.0))), Hit::TreeToggle(node.file.clone())));
                    }
                }
            }
            if selected_hint(item, &tree, &sel) && 11.0 * z >= 5.0 {
                painter.text(r.center_bottom() + Vec2::new(0.0, 4.0 * z), Align2::CENTER_TOP, "tap again to open", font(11.0 * z), Color32::from_gray(220));
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
        let ring_ids: Vec<String> = module.functions.iter().filter(|f| self.show_tests || !f.test).map(|f| f.id.clone()).collect();
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
        // Centre card to ring.
        for (_, p) in &hl.ring {
            if let Some(c) = connector_sized(Point::new(0.0, 0.0), CENTRE_CARD_SIZE, *p, CARD_SIZE) {
                curve(painter, &cam, rect, c, Stroke::new(1.0, Color32::from_gray(80)));
            }
        }
        // Call edges.
        let sel = self.place.selected.clone();
        for (a, b) in &hl.edges {
            let (Some(p), Some(q)) = (hl.position(a), hl.position(b)) else { continue };
            let lit = sel.as_deref() == Some(a.as_str()) || sel.as_deref() == Some(b.as_str());
            let stroke = Stroke::new(if lit { 2.2 } else { 1.3 }, if lit { Color32::from_rgb(130, 200, 255) } else { Color32::from_rgba_unmultiplied(130, 200, 255, 110) });
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
        painter.rect(mr, 8.0 * z, fill_of(col), Stroke::new(2.0, col), StrokeKind::Inside);
        let name = file.rsplit('/').next().unwrap_or(file).to_string();
        let path = if module.path.is_empty() { format!("{krate} (target root)") } else { module.path.clone() };
        let sub = format!("{} functions", ring_ids.len());
        card_text(painter, mr, &[(name.as_str(), 16.0, Color32::WHITE), (path.as_str(), 10.0, Color32::from_gray(190)), (sub.as_str(), 11.0, Color32::from_gray(200))], z);
        hits.push((mr, Hit::ModuleCard));
        let mut draw_fn = |id: &str, p: Point, toggles: (bool, bool)| {
            let r = screen(&cam, rect, centred(p, CARD_SIZE));
            let k = crate_of(map, id).unwrap_or("");
            let topic = topic_of(map, k);
            let tc = hex(topic_colour(topic));
            let review = facts.review(id);
            let selected = sel.as_deref() == Some(id);
            let fill = if k == krate { Color32::from_rgb(30, 34, 44) } else { fill_of(tc) };
            painter.rect(r, 6.0 * z, fill, Stroke::new(if selected { 3.5 } else { 2.0 }, if selected { Color32::WHITE } else { review_colour(&review) }), StrokeKind::Inside);
            let found = snap.slice_of(id).and_then(|s| model::find_function(&s.krate, id).map(|(m, f, _)| (model::function_maturity(m, map, k), f.owner.clone(), f.name.clone())));
            let title = match &found {
                Some((_, Some(o), n)) => format!("{o}::{n}"),
                Some((_, None, n)) => n.clone(),
                None => model::short_name(id).to_string(),
            };
            let mat = found.as_ref().and_then(|f| f.0).map(|m| format!("M{m} · ")).unwrap_or_default();
            let line2 = format!("{mat}{}", review.label());
            let mut lines = vec![(title.as_str(), 12.5, Color32::WHITE), (line2.as_str(), 10.0, review_colour(&review))];
            if k != krate {
                lines.push((k, 9.5, tc));
            }
            card_text(painter, r, &lines, z);
            hits.push((r, Hit::Function(id.to_string())));
            let t = (16.0 * z).max(14.0);
            for (on, dir, right) in [(toggles.1, Dir::Callees, true), (toggles.0, Dir::Callers, false)] {
                if !on {
                    continue;
                }
                let c = if right { Pos2::new(r.right() + 0.6 * t, r.center().y) } else { Pos2::new(r.left() - 0.6 * t, r.center().y) };
                let open = self.place.expanded.contains(&(id.to_string(), dir));
                triangle(painter, c, 11.0 * z.max(0.6), right, false, open, Color32::from_rgb(200, 220, 255));
                hits.push((Rect::from_center_size(c, Vec2::splat(t * 1.3)), Hit::FnToggle(id.to_string(), dir)));
            }
        };
        for (id, p) in &hl.ring {
            draw_fn(id, *p, (true, true));
        }
        for (id, p, col) in &hl.unfolded {
            draw_fn(id, *p, (*col < 0, *col > 0));
        }
        for id in &hl.waiting {
            if let Some(p) = hl.position(id) {
                painter.text(cam.to_screen(rect, p.x, p.y + 0.5 * CARD_SIZE.1 + 8.0), Align2::CENTER_TOP, "loading…", font(11.0), Color32::from_gray(200));
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
