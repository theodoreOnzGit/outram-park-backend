//! **The Control volumes tab** (gh:#1718): the plant model's control
//! volumes as boxes, with the flow and heat links between them and the
//! primary pressure boundary outlined, drawn from the snapshot's
//! [`CvSnapshot`], which the physics publishes from the ASSEMBLED plant
//! (`physics::control_volumes`). Nothing here lists a CV: an entry the model
//! does not have cannot appear, and one it gains appears in the overflow row
//! until it is given a place.
//!
//! **Purpose: verifying the SB-LOCA fault tree** (gh:#1716). Every basic
//! event of the tree names a boundary component and the CV it sits on; the
//! components are drawn as numbered badges on their CVs, and selecting a
//! basic event on the Fault tree tab highlights its CV here (and selecting a
//! CV here lists and highlights its events there).
//!
//! # What is drawn from state, and what is a drawing choice
//!
//! - **From state:** which entries exist, their names, roles, volumes, live
//!   temperature, mass and through-flow; which links exist; each link's
//!   **direction, from the sign of its mass flow or heat rate** (the
//!   engine's animation rule: a reversed flow draws reversed, a zero flow
//!   draws no arrow); which CVs are inside the pressure boundary; where each
//!   boundary component sits.
//! - **Drawing choices:** where each box goes ([`position`]) and its size.
//!   They are schematic positions in arbitrary world units, chosen so the
//!   primary loop reads as a loop (down the core, across to the steam
//!   generator, up it, back through the cold duct); they are **not**
//!   plant dimensions and no length is drawn, so there is no scale bar.

use bugis::web_demo::view::{apply_zoom, zoom_buttons, View};
use egui::{Align2, Color32, FontId, Pos2, Rect, Shape, Stroke, StrokeKind, Ui, Vec2};
use outram_park_digital_twin_engine::components::LegendUnit;
use uom::si::f64::ThermodynamicTemperature;
use uom::si::thermodynamic_temperature::kelvin;

use crate::physics::control_volumes::{CvRole, CvRow, CvSnapshot, LinkRow, Medium};

/// The research-only banner both new tabs carry (`RESPONSIBLE_USE.md`).
pub const RESEARCH_BANNER: &str =
    "Research and education only: an offline teaching model, not a PSA, not for licensing, operation or safety decisions";

/// Half-size of an ordinary box, world units.
const BOX_HALF: [f64; 2] = [0.8, 0.42];
/// Half-size of a steam-generator node's box, world units.
const SG_BOX_HALF: [f64; 2] = [0.6, 0.36];

/// Schematic position of entry `id`, world units (y up), or `None` for an
/// entry this drawing has not placed yet (it goes in the overflow row).
/// **A drawing choice, not plant geometry**: see the module docs.
pub fn position(id: &str) -> Option<[f64; 2]> {
    let fixed = match id {
        "passive.rccs" => [-6.2, 0.0],
        "passive.rpv" => [-4.4, 0.0],
        "passive.reflector" => [-2.4, 0.0],
        "passive.near_wall" => [-0.7, 0.0],
        "core.fuel" => [1.0, 1.5],
        "core.solid" => [1.0, 0.0],
        "core.helium" => [3.0, 0.0],
        "primary.rpv_annuli" => [3.0, 2.2],
        "primary.hot_duct" => [3.0, -2.2],
        "primary.cold_duct" => [3.0, 4.6],
        "primary.circulator" => [5.8, 4.6],
        "secondary.feedwater_line" => [10.8, 3.6],
        "secondary.feed_pump" => [12.9, 3.6],
        "secondary.condenser" => [12.9, 1.2],
        "secondary.exhaust_duct" => [12.9, -1.4],
        "secondary.turbine" => [12.9, -4.0],
        "secondary.main_steam_line" => [10.8, -4.0],
        _ => return sg_position(id),
    };
    Some(fixed)
}

/// Steam-generator nodes: three columns (helium, metal, water), station 1
/// (the hot inlet) at the bottom, so the helium rises through it.
fn sg_position(id: &str) -> Option<[f64; 2]> {
    let (column, station) = id.strip_prefix("sg.")?.split_once('.')?;
    let i: f64 = station.parse().ok()?;
    let x = match column {
        "he" => 5.8,
        "metal" => 7.2,
        "water" => 8.8,
        _ => return None,
    };
    Some([x, -4.0 + 0.86 * (i - 1.0)])
}

/// Half-size of entry `id`'s box, world units.
pub fn half_size(id: &str) -> [f64; 2] {
    if id.starts_with("sg.") { SG_BOX_HALF } else { BOX_HALF }
}

/// Where every entry of `cvs` is drawn: its [`position`], or the next slot of
/// an overflow row below the drawing for an entry with none. Indexed like
/// `cvs.volumes`.
pub fn place_all(cvs: &CvSnapshot) -> Vec<[f64; 2]> {
    let mut overflow = 0.0;
    cvs.volumes
        .iter()
        .map(|v| {
            position(&v.id).unwrap_or_else(|| {
                let p = [-6.2 + 1.8 * overflow, -6.4];
                overflow += 1.0;
                p
            })
        })
        .collect()
}

/// The point where the segment from box centre `c` (half-size `half`) toward
/// `toward` leaves the box.
pub fn clip_to_box(c: [f64; 2], half: [f64; 2], toward: [f64; 2]) -> [f64; 2] {
    let d = [toward[0] - c[0], toward[1] - c[1]];
    let tx = if d[0] != 0.0 { half[0] / d[0].abs() } else { f64::INFINITY };
    let ty = if d[1] != 0.0 { half[1] / d[1].abs() } else { f64::INFINITY };
    let t = tx.min(ty).min(1.0);
    [c[0] + t * d[0], c[1] + t * d[1]]
}

/// Tail and head of the arrow for a link from `a` to `b` (centres and
/// half-sizes) carrying the signed `value`: from `a` to `b` when positive,
/// **from `b` to `a` when negative**, and `None` (a line with no arrow) when
/// the value is zero or not known. The direction is the sign of the state,
/// never the nominal order (the engine's animation rule).
pub fn arrow_ends(a: [f64; 2], a_half: [f64; 2], b: [f64; 2], b_half: [f64; 2], value: f64) -> Option<([f64; 2], [f64; 2])> {
    if !(value.is_finite() && value != 0.0) {
        return None;
    }
    let from_a = clip_to_box(a, a_half, b);
    let into_b = clip_to_box(b, b_half, a);
    Some(if value > 0.0 { (from_a, into_b) } else { (into_b, from_a) })
}

/// The primary pressure boundary as a world rectangle `[min_x, min_y,
/// max_x, max_y]`: the bounding box of the entries inside it, padded. `None`
/// when no entry is inside.
pub fn boundary_outline(cvs: &CvSnapshot, placed: &[[f64; 2]]) -> Option<[f64; 4]> {
    let pad = 0.18;
    let mut out: Option<[f64; 4]> = None;
    for (v, c) in cvs.volumes.iter().zip(placed) {
        if !v.inside_primary_boundary {
            continue;
        }
        let h = half_size(&v.id);
        let r = [c[0] - h[0] - pad, c[1] - h[1] - pad, c[0] + h[0] + pad, c[1] + h[1] + pad];
        out = Some(match out {
            None => r,
            Some(o) => [o[0].min(r[0]), o[1].min(r[1]), o[2].max(r[2]), o[3].max(r[3])],
        });
    }
    out
}

/// The 1-based badge number of each boundary component, and the CV it is
/// drawn on (its first), in snapshot order.
pub fn badges(cvs: &CvSnapshot) -> Vec<(usize, String, String)> {
    cvs.boundary
        .iter()
        .enumerate()
        .flat_map(|(i, b)| b.cvs.iter().map(move |c| (i + 1, b.id.clone(), c.clone())))
        .collect()
}

/// The bounding half-extent of the drawing around `home`, world units, for
/// Reset.
fn half_extent(placed: &[[f64; 2]]) -> ([f64; 2], f64) {
    let (mut lo, mut hi) = ([f64::INFINITY; 2], [f64::NEG_INFINITY; 2]);
    for p in placed {
        for k in 0..2 {
            lo[k] = lo[k].min(p[k]);
            hi[k] = hi[k].max(p[k]);
        }
    }
    if !lo[0].is_finite() {
        return ([0.0, 0.0], 1.0);
    }
    let home = [0.5 * (lo[0] + hi[0]), 0.5 * (lo[1] + hi[1])];
    (home, 0.5 * (hi[0] - lo[0]).max(hi[1] - lo[1]) + 1.2)
}

/// The tab's own retained state: the view and the selected CV.
pub struct CvTab {
    pub view: View,
    /// The CV the reader selected (or the Fault tree tab handed over).
    pub selected: Option<String>,
}

impl Default for CvTab {
    fn default() -> Self {
        Self { view: View::new(10.0), selected: None }
    }
}

fn medium_colour(m: Medium) -> Color32 {
    use bugis::palette::*;
    match m {
        Medium::Helium => NEUTRAL_BLUE,
        Medium::Graphite => GRAY,
        Medium::Fuel => NEUTRAL_RED,
        Medium::Steel => NEUTRAL_PURPLE,
        Medium::WaterSteam => NEUTRAL_AQUA,
        Medium::None => DARK3,
    }
}

/// Draw the tab into `rect` and return a CV the reader clicked.
///
/// GUI drawing code, exempt from a reaching test (egui paint and input
/// cannot be driven here); its geometry is [`place_all`], [`clip_to_box`],
/// [`arrow_ends`], [`boundary_outline`] and [`badges`], which are tested.
/// `highlighted_components` are boundary-component ids to light (the Fault
/// tree tab's selected event).
pub fn draw(
    ui: &mut Ui,
    rect: Rect,
    cvs: &CvSnapshot,
    tab: &mut CvTab,
    unit: LegendUnit,
    highlighted_components: &[String],
) -> Option<String> {
    let placed = place_all(cvs);
    let (home, half) = half_extent(&placed);
    tab.view.half_extent = half;
    tab.view.home = home;
    let resp = ui.interact(rect, ui.id().with("cv_tab_view"), egui::Sense::click_and_drag());
    tab.view.handle_input(ui, &resp);
    let view = tab.view;
    let painter = ui.painter_at(rect);
    let to_screen = |p: [f64; 2]| view.to_screen(rect, p[0], p[1]);
    let px = view.scale as f32;
    let visuals = ui.visuals().clone();
    let ink = visuals.text_color();
    let font = |k: f32| FontId::proportional((k * px).clamp(0.0, 20.0));

    // The primary pressure boundary.
    if let Some(o) = boundary_outline(cvs, &placed) {
        let r = Rect::from_two_pos(to_screen([o[0], o[1]]), to_screen([o[2], o[3]]));
        painter.rect_stroke(r, 8.0, Stroke::new(3.0, bugis::palette::BRIGHT_RED), StrokeKind::Middle);
        if px * 0.2 >= 7.0 {
            painter.text(
                r.left_top() + Vec2::new(6.0, -4.0),
                Align2::LEFT_BOTTOM,
                "Primary helium pressure boundary",
                font(0.2),
                bugis::palette::BRIGHT_RED,
            );
        }
    }

    // Links under the boxes.
    let index = |id: &str| cvs.volumes.iter().position(|v| v.id == id);
    for l in &cvs.links {
        let (Some(a), Some(b)) = (index(&l.from), index(&l.to)) else { continue };
        draw_link(&painter, l, placed[a], half_size(&l.from), placed[b], half_size(&l.to), &to_screen, px, ink);
    }

    // Boxes.
    let badge_list = badges(cvs);
    let mut clicked = None;
    for (v, c) in cvs.volumes.iter().zip(&placed) {
        let h = half_size(&v.id);
        let r = Rect::from_two_pos(to_screen([c[0] - h[0], c[1] - h[1]]), to_screen([c[0] + h[0], c[1] + h[1]]));
        let selected = tab.selected.as_deref() == Some(v.id.as_str());
        let lit = badge_list.iter().any(|(_, comp, cv)| cv == &v.id && highlighted_components.contains(comp));
        let fill = medium_colour(v.medium).gamma_multiply(if v.role == CvRole::Balance { 0.55 } else { 0.25 });
        painter.rect_filled(r, 4.0, fill);
        let (w, colour) = if selected {
            (3.0, bugis::palette::BRIGHT_AQUA)
        } else if lit {
            (3.0, bugis::palette::BRIGHT_YELLOW)
        } else {
            (1.0, ink)
        };
        if v.role == CvRole::Balance {
            painter.rect_stroke(r, 4.0, Stroke::new(w, colour), StrokeKind::Inside);
        } else {
            dashed_rect(&painter, r, Stroke::new(w, colour));
        }
        box_text(&painter, r, v, unit, px, ink);
        if resp.clicked() && resp.interact_pointer_pos().is_some_and(|p| r.contains(p)) {
            clicked = Some(v.id.clone());
        }
        // Boundary-component badges along the top edge.
        let mine: Vec<&(usize, String, String)> = badge_list.iter().filter(|(_, _, cv)| cv == &v.id).collect();
        for (k, (n, comp, _)) in mine.iter().enumerate() {
            let at = to_screen([c[0] - h[0] + 0.16 + 0.26 * k as f64, c[1] + h[1]]);
            let on = highlighted_components.contains(comp);
            let rad = (0.1 * px).clamp(5.0, 12.0);
            painter.circle_filled(at, rad, if on { bugis::palette::BRIGHT_YELLOW } else { bugis::palette::BRIGHT_ORANGE });
            painter.text(at, Align2::CENTER_CENTER, n.to_string(), FontId::proportional(1.2 * rad), Color32::BLACK);
        }
    }

    // Legend of the badges, under the drawing.
    if px * 0.16 >= 6.0 {
        let mut y = -5.2;
        for (i, b) in cvs.boundary.iter().enumerate() {
            let text = format!("{}  {} -> {}", i + 1, b.name, b.cvs.first().map_or("", String::as_str));
            painter.text(to_screen([-6.9, y]), Align2::LEFT_CENTER, text, font(0.16), ink);
            y -= 0.26;
        }
    }

    // Main-view overlays: the banner, and + / - / Reset.
    banner(&painter, rect, &["Control volumes of the running model", RESEARCH_BANNER], &visuals);
    if let Some(z) = zoom_buttons(ui, rect) {
        apply_zoom(&mut tab.view, rect, z);
    }
    if clicked.is_some() {
        tab.selected = clicked.clone();
    }
    clicked
}

/// The overlay text under the 52 px row the "Controls »" and zoom buttons
/// keep clear (GUI drawing code; exempt, no reaching test).
pub fn banner(painter: &egui::Painter, rect: Rect, lines: &[&str], visuals: &egui::Visuals) {
    let mut at = rect.left_top() + Vec2::new(10.0, 58.0);
    for (k, line) in lines.iter().enumerate() {
        let size = if k == 0 { 15.0 } else { 12.0 };
        let galley = painter.layout(line.to_string(), FontId::proportional(size), visuals.text_color(), rect.width() - 20.0);
        let bg = Rect::from_min_size(at, galley.size()).expand(3.0);
        painter.rect_filled(bg, 3.0, visuals.extreme_bg_color.gamma_multiply(0.85));
        let h = galley.size().y;
        painter.galley(at, galley, visuals.text_color());
        at.y += h + 6.0;
    }
}

/// A box's text: name, then T, M and V, then the through-flow, each line
/// left out when the box is too small to read it (GUI drawing code; exempt).
fn box_text(painter: &egui::Painter, r: Rect, v: &CvRow, unit: LegendUnit, px: f32, ink: Color32) {
    let size = (0.15 * px).min(16.0);
    if size < 6.0 {
        return;
    }
    let mut lines = vec![v.name.clone()];
    match v.role {
        CvRole::NoVolume => lines.push("no CV".into()),
        CvRole::Junction => lines.push("junction, no capacity".into()),
        CvRole::Boundary => lines.push("fixed boundary".into()),
        CvRole::Inventory => lines.push("inventory, no balance".into()),
        CvRole::Balance => {}
    }
    if v.temperature_k.is_finite() {
        lines.push(format!(
            "T {}",
            crate::app::panels::temperature_display(unit, ThermodynamicTemperature::new::<kelvin>(v.temperature_k), 0)
        ));
    }
    let mut mv = Vec::new();
    if v.mass_kg.is_finite() {
        mv.push(format!("M {:.3} kg", v.mass_kg));
    }
    if v.volume_m3.is_finite() {
        mv.push(format!("V {:.3} m3", v.volume_m3));
    }
    if !mv.is_empty() {
        lines.push(mv.join("  "));
    }
    if v.through_flow_kg_per_s.is_finite() {
        lines.push(format!("flow {:.3} kg/s", v.through_flow_kg_per_s));
    }
    let clip = painter.with_clip_rect(r.shrink(2.0));
    let line_h = 1.2 * size;
    let mut y = r.top() + 3.0;
    for (k, line) in lines.iter().enumerate() {
        if y + line_h > r.bottom() + 1.0 {
            break;
        }
        let s = if k == 0 { size } else { 0.85 * size };
        clip.text(Pos2::new(r.center().x, y), Align2::CENTER_TOP, line, FontId::proportional(s), ink);
        y += line_h;
    }
}

/// One link: a solid line for a mass flow, dashed for a heat path, with the
/// arrowhead where [`arrow_ends`] puts it and the value beside it (GUI
/// drawing code; exempt).
#[allow(clippy::too_many_arguments)]
fn draw_link(
    painter: &egui::Painter,
    l: &LinkRow,
    a: [f64; 2],
    a_half: [f64; 2],
    b: [f64; 2],
    b_half: [f64; 2],
    to_screen: &impl Fn([f64; 2]) -> Pos2,
    px: f32,
    ink: Color32,
) {
    let colour = if l.is_flow { ink } else { bugis::palette::NEUTRAL_ORANGE };
    let width = if l.is_flow { 2.0 } else { 1.2 };
    let stroke = Stroke::new(width, colour);
    match arrow_ends(a, a_half, b, b_half, l.value) {
        Some((tail, head)) => {
            let (t, h) = (to_screen(tail), to_screen(head));
            if l.is_flow {
                painter.line_segment([t, h], stroke);
            } else {
                painter.add(Shape::dashed_line(&[t, h], stroke, 6.0, 4.0));
            }
            let dir = (h - t).normalized();
            let side = Vec2::new(-dir.y, dir.x);
            let size = 9.0;
            painter.add(Shape::convex_polygon(
                vec![h, h - dir * size + side * 0.5 * size, h - dir * size - side * 0.5 * size],
                colour,
                Stroke::NONE,
            ));
            if px * 0.13 >= 7.0 {
                let text = if l.is_flow { format!("{:.2} kg/s", l.value.abs()) } else { format!("{:.0} kW", l.value.abs() / 1e3) };
                painter.text(t + 0.5 * (h - t) + side * 8.0, Align2::CENTER_CENTER, text, FontId::proportional((0.13 * px).min(13.0)), colour);
            }
        }
        None => {
            // Zero or unknown: a faint line, no direction claimed.
            let (t, h) = (to_screen(clip_to_box(a, a_half, b)), to_screen(clip_to_box(b, b_half, a)));
            let faint = Stroke::new(1.0, colour.gamma_multiply(0.45));
            painter.add(Shape::dashed_line(&[t, h], faint, 3.0, 4.0));
        }
    }
}

/// A dashed rectangle outline (for entries that are not balance CVs; GUI
/// drawing code, exempt).
fn dashed_rect(painter: &egui::Painter, r: Rect, stroke: Stroke) {
    let pts = [r.left_top(), r.right_top(), r.right_bottom(), r.left_bottom(), r.left_top()];
    painter.add(Shape::dashed_line(&pts, stroke, 5.0, 4.0));
}

/// The side-panel section for this tab: the selected CV's details, its
/// boundary components and, through `events_on`, the fault-tree basic
/// events that sit on it (GUI drawing code; exempt).
pub fn side_panel(ui: &mut Ui, tab: &mut CvTab, cvs: &CvSnapshot, unit: LegendUnit, events_on: &[String]) {
    ui.heading("Control volumes");
    ui.small(RESEARCH_BANNER);
    ui.label(format!(
        "{} entries from the running model: {} balance CVs.",
        cvs.volumes.len(),
        cvs.volumes.iter().filter(|v| v.role == CvRole::Balance).count()
    ));
    ui.small("Solid boxes hold an energy balance; dashed ones do not (see each note). Arrows follow the sign of the flow or heat rate; a line with no arrow carries none, or a rate the model does not expose.");
    ui.separator();
    egui::ComboBox::from_label("Selected CV")
        .selected_text(tab.selected.clone().unwrap_or_else(|| "none".into()))
        .show_ui(ui, |ui| {
            ui.selectable_value(&mut tab.selected, None, "none");
            for v in &cvs.volumes {
                ui.selectable_value(&mut tab.selected, Some(v.id.clone()), &v.id);
            }
        });
    let Some(v) = tab.selected.as_deref().and_then(|id| cvs.get(id)) else { return };
    ui.label(egui::RichText::new(&v.name).strong());
    ui.small(format!("{} · {:?} · {:?} · {:?}", v.id, v.circuit, v.medium, v.role));
    ui.small(v.note.clone());
    if v.temperature_k.is_finite() {
        ui.label(format!(
            "T = {}",
            crate::app::panels::temperature_display(unit, ThermodynamicTemperature::new::<kelvin>(v.temperature_k), 1)
        ));
    }
    if v.volume_m3.is_finite() {
        ui.label(format!("V = {:.4} m3", v.volume_m3));
    }
    if v.mass_kg.is_finite() {
        ui.label(format!("M = {:.4} kg", v.mass_kg));
    }
    if v.through_flow_kg_per_s.is_finite() {
        ui.label(format!("flow = {:.4} kg/s", v.through_flow_kg_per_s));
    }
    ui.label(if v.inside_primary_boundary { "Inside the primary pressure boundary" } else { "Outside the primary pressure boundary" });
    for b in cvs.boundary.iter().filter(|b| b.cvs.iter().any(|c| c == &v.id)) {
        ui.separator();
        ui.label(egui::RichText::new(format!("Boundary component: {}", b.name)).strong());
        ui.small(b.source.clone());
    }
    if !events_on.is_empty() {
        ui.separator();
        ui.label("Fault-tree basic events on this CV:");
        for e in events_on {
            ui.monospace(e);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::physics::control_volumes::{control_volume_map, CvSnapshot};
    use crate::physics::HtgrPlant;

    fn snapshot() -> CvSnapshot {
        CvSnapshot::from_map(&control_volume_map(&HtgrPlant::new()))
    }

    /// Every entry the model has today is placed (none in the overflow row),
    /// no two boxes overlap, and an unknown entry still gets a slot.
    #[test]
    fn every_entry_is_placed_without_overlap() {
        let mut cvs = snapshot();
        let placed = place_all(&cvs);
        for v in &cvs.volumes {
            assert!(position(&v.id).is_some(), "{} has no place", v.id);
        }
        for i in 0..placed.len() {
            for j in i + 1..placed.len() {
                let (a, b) = (half_size(&cvs.volumes[i].id), half_size(&cvs.volumes[j].id));
                let apart = (placed[i][0] - placed[j][0]).abs() >= a[0] + b[0] || (placed[i][1] - placed[j][1]).abs() >= a[1] + b[1];
                assert!(apart, "{} and {} overlap", cvs.volumes[i].id, cvs.volumes[j].id);
            }
        }
        let mut extra = cvs.volumes[0].clone();
        extra.id = "core.node7".into();
        cvs.volumes.push(extra);
        let p = place_all(&cvs);
        assert_eq!(p.last().unwrap()[1], -6.4, "an unplaced entry goes to the overflow row");
        assert!(sg_position("sg.pump.1").is_none() && sg_position("sg.he.x").is_none());
    }

    /// Arrows follow the sign: positive runs a -> b, negative b -> a, zero
    /// and unknown draw no arrow. Ends sit on the box edges.
    #[test]
    fn arrows_follow_the_sign_of_the_flow() {
        let (a, b, h) = ([0.0, 0.0], [4.0, 0.0], [0.8, 0.4]);
        let (t, hd) = arrow_ends(a, h, b, h, 2.5).unwrap();
        assert_eq!((t, hd), ([0.8, 0.0], [3.2, 0.0]));
        let (t, hd) = arrow_ends(a, h, b, h, -2.5).unwrap();
        assert_eq!((t, hd), ([3.2, 0.0], [0.8, 0.0]), "a reversed flow draws reversed");
        assert!(arrow_ends(a, h, b, h, 0.0).is_none());
        assert!(arrow_ends(a, h, b, h, f64::NAN).is_none());
        assert_eq!(clip_to_box([0.0, 0.0], [1.0, 0.5], [0.0, 3.0]), [0.0, 0.5]);
        assert_eq!(clip_to_box([0.0, 0.0], [1.0, 0.5], [0.2, 0.1]), [0.2, 0.1], "a target inside stays");
    }

    /// The pressure-boundary outline encloses every CV inside the boundary
    /// and no CV outside it (the water side, the secondary, the RPV wall that
    /// IS the boundary, the RCCS). Badges sit on the CVs their components
    /// name; the SG tubes badge all eight helium nodes.
    #[test]
    fn the_boundary_outline_and_badges_follow_the_list() {
        let cvs = snapshot();
        let placed = place_all(&cvs);
        let o = boundary_outline(&cvs, &placed).unwrap();
        for (v, c) in cvs.volumes.iter().zip(&placed) {
            let h = half_size(&v.id);
            let inside = c[0] - h[0] >= o[0] && c[0] + h[0] <= o[2] && c[1] - h[1] >= o[1] && c[1] + h[1] <= o[3];
            let overlaps = c[0] + h[0] > o[0] && c[0] - h[0] < o[2] && c[1] + h[1] > o[1] && c[1] - h[1] < o[3];
            if v.inside_primary_boundary {
                assert!(inside, "{} should be inside", v.id);
            } else {
                assert!(!overlaps, "{} should be outside", v.id);
            }
        }
        let b = badges(&cvs);
        assert_eq!(b.iter().filter(|(_, c, _)| c == "sg.tubes").count(), 8);
        assert!(b.iter().any(|(_, c, cv)| c == "rpv.fuel_loading_tube" && cv == "primary.rpv_annuli"));
        let empty = CvSnapshot::default();
        assert!(boundary_outline(&empty, &[]).is_none());
        assert_eq!(half_extent(&[]), ([0.0, 0.0], 1.0));
        let (home, half) = half_extent(&placed);
        assert!(half > 9.0 && home[0] > 0.0);
        assert!(CvTab::default().selected.is_none());
    }
}
