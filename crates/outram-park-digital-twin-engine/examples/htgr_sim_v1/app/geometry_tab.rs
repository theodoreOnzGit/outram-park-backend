//! HTR-10 R-Z benchmark geometry viewer -- draws
//! [`htr10_rz_geometry::htr10_rz_zones`] as a coloured cross-section, the
//! same picture `generate_htr10_geometry.py` (GitHub issue #23) renders,
//! plus the derived helium gas volumes beside it.
//!
//! This is a **static reference view**, not a live schematic: nothing here
//! reads [`crate::app::state::HtgrSnapshot`] or changes across a frame. It
//! exists so the benchmark geometry that drives
//! [`crate::physics::reactor_model::one_node::PebbleBedCore::pebble_bed_helium_volume`]
//! is something a reader can actually see, not just trust a doc comment
//! about. See [`htr10_rz_geometry`]'s module doc comment for the
//! NOT-VALIDATED caveat that carries over to everything drawn here.

use egui::{Align2, Color32, FontId, Pos2, Stroke, Ui, Vec2};
use uom::si::length::centimeter;

use crate::physics::reactor_model::htr10_rz_geometry::{
    axial_ticks_cm, dummy_pebble_helium_volume_conus,
    dummy_pebble_helium_volume_lower_discharge_tube,
    dummy_pebble_helium_volume_middle_discharge_tube,
    dummy_pebble_helium_volume_upper_discharge_tube, htr10_rz_zones, pebble_bed_helium_volume,
    radial_ticks_cm, top_cavity_helium_volume, Htr10RzZone, ZoneMaterial,
};
use crate::physics::reactor_model::one_node::pebble_diameter;

/// Face and edge colour for a zone's material, matching
/// `generate_htr10_geometry.py`'s `STYLES` legend as closely as a flat
/// `Color32` pair can. Kept here, not on [`ZoneMaterial`] itself, so the
/// geometry module stays free of a GUI dependency -- see its module doc
/// comment.
fn zone_colors(material: ZoneMaterial) -> (Color32, Color32) {
    match material {
        ZoneMaterial::Boronated => (
            Color32::from_rgb(244, 230, 162),
            Color32::from_rgb(74, 66, 42),
        ),
        ZoneMaterial::Carbon => (
            Color32::from_rgb(200, 155, 90),
            Color32::from_rgb(94, 68, 38),
        ),
        ZoneMaterial::Graphite => (
            Color32::from_rgb(201, 167, 125),
            Color32::from_rgb(92, 70, 49),
        ),
        // Between Bottom's (52,52,52) and Mixed's (217,217,217) in lightness,
        // per the maintainer's request -- see ZoneMaterial::TopReflector's
        // doc comment.
        ZoneMaterial::TopReflector => (
            Color32::from_rgb(130, 113, 95),
            Color32::from_rgb(58, 48, 38),
        ),
        ZoneMaterial::Bottom => (Color32::from_rgb(52, 52, 52), Color32::from_rgb(17, 17, 17)),
        ZoneMaterial::Control => (
            Color32::from_rgb(229, 210, 184),
            Color32::from_rgb(106, 84, 61),
        ),
        ZoneMaterial::ColdChannel => (
            Color32::from_rgb(228, 216, 242),
            Color32::from_rgb(105, 87, 130),
        ),
        ZoneMaterial::Hot => (
            Color32::from_rgb(143, 114, 181),
            Color32::from_rgb(73, 55, 95),
        ),
        ZoneMaterial::ColdChamber => (
            Color32::from_rgb(199, 180, 222),
            Color32::from_rgb(91, 73, 110),
        ),
        ZoneMaterial::Cavity => (Color32::WHITE, Color32::from_rgb(51, 51, 51)),
        ZoneMaterial::Mixed => (
            Color32::from_rgb(217, 217, 217),
            Color32::from_rgb(85, 85, 85),
        ),
        ZoneMaterial::Dummy => (
            Color32::from_rgb(245, 205, 214),
            Color32::from_rgb(143, 100, 112),
        ),
        ZoneMaterial::Unknown => (Color32::WHITE, Color32::from_rgb(119, 119, 119)),
    }
}

/// One line of the material legend: label text paired with the same colours
/// [`zone_colors`] paints that material with.
fn legend_entries() -> [(&'static str, ZoneMaterial); 13] {
    [
        ("Dummy pebbles", ZoneMaterial::Dummy),
        ("Mixed fuel/dummy pebbles", ZoneMaterial::Mixed),
        ("Top core cavity", ZoneMaterial::Cavity),
        ("Bottom reflector", ZoneMaterial::Bottom),
        ("Boronated carbon bricks", ZoneMaterial::Boronated),
        ("Carbon bricks", ZoneMaterial::Carbon),
        ("Graphite reflector", ZoneMaterial::Graphite),
        ("Top reflector graphite", ZoneMaterial::TopReflector),
        ("Control-rod reflector", ZoneMaterial::Control),
        ("Hot-coolant reflector", ZoneMaterial::Hot),
        ("Cold-coolant chamber", ZoneMaterial::ColdChamber),
        ("Cold-coolant channel", ZoneMaterial::ColdChannel),
        ("Material pending", ZoneMaterial::Unknown),
    ]
}

/// The convex polygon(s) to fill for a zone's `(r, z)` footprint.
///
/// `egui`'s [`egui::Shape::Path`] fill is only defined for convex polygons.
/// Every zone here is convex except benchmark volume 48 (the L-shaped
/// graphite reflector -- see the "Volume 48" comment in
/// [`htr10_rz_geometry::htr10_rz_zones`]), which this splits into the same
/// two rectangles its own doc comment describes it as the union of. This is
/// a rendering-only decomposition; [`Htr10RzZone::vertices_cm`] (used for the
/// physics-facing [`Htr10RzZone::volume_of_revolution`]) is untouched.
fn zone_render_polygons(zone: &Htr10RzZone) -> Vec<Vec<(f64, f64)>> {
    if zone.volume == 48 && zone.vertices_cm.len() == 6 {
        vec![
            vec![
                (108.6, 40.0),
                (167.793, 40.0),
                (167.793, 95.0),
                (108.6, 95.0),
            ],
            vec![
                (148.6, 95.0),
                (167.793, 95.0),
                (167.793, 388.764),
                (148.6, 388.764),
            ],
        ]
    } else {
        vec![zone.vertices_cm.clone()]
    }
}

/// Whether `point` lies inside the convex polygon `vertices` (or on its
/// boundary), tested by checking every edge's cross product has a consistent
/// sign -- valid for any convex polygon, which every entry
/// [`zone_render_polygons`] returns is (see that function's doc comment).
fn point_in_convex_polygon(point: (f64, f64), vertices: &[(f64, f64)]) -> bool {
    let n = vertices.len();
    let mut sign = 0.0_f64;
    for i in 0..n {
        let (x1, y1) = vertices[i];
        let (x2, y2) = vertices[(i + 1) % n];
        let cross = (x2 - x1) * (point.1 - y1) - (y2 - y1) * (point.0 - x1);
        if cross != 0.0 {
            if sign == 0.0 {
                sign = cross.signum();
            } else if cross.signum() != sign {
                return false;
            }
        }
    }
    true
}

/// Draw a schematic pebble packing inside a **volume group**'s combined
/// footprint -- a **body-centred-cubic-projected lattice** (rows spaced by
/// `pebble_diameter * sqrt(3)/2`, each row offset by half a pitch from its
/// neighbours, the standard 2-D depiction of a BCC/close-packed
/// cross-section), not a real DEM packing. Only [`ZoneMaterial::Mixed`] (the
/// settled bed, zone 99) and [`ZoneMaterial::Dummy`] (dummy-pebble-only
/// regions) actually hold discrete pebbles; every other zone is a
/// homogenised reflector or structure, so this is a no-op for them.
///
/// `group` is every [`Htr10RzZone`] entry sharing one benchmark volume
/// number (see [`Htr10RzZone`]'s doc comment on why more than one entry can
/// share a number) -- the row phase is anchored to the **group's** combined
/// z-extent, not each entry's own, so a zone drawn across several z-sub-bands
/// (7, 12, 78) gets one continuous lattice with no seam at the internal
/// band boundaries. Candidate centres are generated over the group's combined
/// bounding box and kept only if [`point_in_convex_polygon`] accepts them
/// against *any* of the group's render sub-polygons, so the lattice stays
/// inside the true footprint rather than the bounding box (matters for zone
/// 91, the one non-rectangular pebble-bearing shape). Circles are drawn as
/// thin outlines, not filled, so the material colour underneath still reads.
fn draw_pebble_lattice(
    painter: &egui::Painter,
    group: &[&Htr10RzZone],
    to_screen: &impl Fn(f64, f64) -> Pos2,
    scale: f32,
) {
    let material = group[0].material;
    if !matches!(material, ZoneMaterial::Mixed | ZoneMaterial::Dummy) {
        return;
    }

    let all_polygons: Vec<Vec<(f64, f64)>> = group
        .iter()
        .flat_map(|zone| zone_render_polygons(zone))
        .collect();

    let pitch_cm = pebble_diameter().get::<centimeter>();
    let row_pitch_cm = pitch_cm * 3.0_f64.sqrt() / 2.0;
    let stroke = Stroke::new(0.6, Color32::from_black_alpha(130));
    let circle_radius_px = (pitch_cm * 0.42) as f32 * scale;

    let r_min = all_polygons
        .iter()
        .flatten()
        .map(|(r, _)| *r)
        .fold(f64::MAX, f64::min);
    let r_max = all_polygons
        .iter()
        .flatten()
        .map(|(r, _)| *r)
        .fold(f64::MIN, f64::max);
    let z_min = all_polygons
        .iter()
        .flatten()
        .map(|(_, z)| *z)
        .fold(f64::MAX, f64::min);
    let z_max = all_polygons
        .iter()
        .flatten()
        .map(|(_, z)| *z)
        .fold(f64::MIN, f64::max);

    let mut row = 0u32;
    let mut z = z_min + pitch_cm / 2.0;
    while z < z_max {
        let offset_cm = if row % 2 == 0 { 0.0 } else { pitch_cm / 2.0 };
        let mut r = r_min + pitch_cm / 2.0 + offset_cm;
        while r < r_max {
            if all_polygons
                .iter()
                .any(|polygon| point_in_convex_polygon((r, z), polygon))
            {
                painter.circle_stroke(to_screen(r, z), circle_radius_px, stroke);
            }
            r += pitch_cm;
        }
        z += row_pitch_cm;
        row += 1;
    }
}

/// Starting pixels-per-centimetre the cross-section is drawn at, before a
/// reader has touched the zoom buttons -- see [`ZoomLevel`].
///
/// **Sized to the model's true extent, not fit-to-viewport, by design.** An
/// earlier version scaled the canvas to whatever space happened to be
/// visible (capped at 900 px tall), which for this model's 190 x 610 cm
/// extent forced a scale far too small to make out its narrower columns --
/// volumes 21 (5.6 cm wide) and 57 (8 cm wide) were reported unreadable at
/// that scale. Sizing to the true extent instead, with the surrounding
/// `ScrollArea` in [`draw_geometry`] free to pan around a canvas larger than
/// the screen, is what a `ScrollArea` is for. 5.0 px/cm puts even the
/// narrowest column (21, 57) at 28-40 px wide, well clear of the width the
/// label-skip threshold below needs.
pub const DEFAULT_PIXELS_PER_CM: f32 = 5.0;

/// Zoom bounds -- see [`ZoomLevel::zoom_in`]/[`ZoomLevel::zoom_out`]. The
/// floor keeps the narrowest columns (21, 57) from shrinking back below the
/// unreadable scale [`DEFAULT_PIXELS_PER_CM`]'s doc comment describes; the
/// ceiling is just large enough that one pebble-lattice circle (roughly
/// [`crate::physics::reactor_model::one_node::pebble_diameter`] wide) still
/// reads as a circle rather than the canvas becoming unusably huge.
pub const MIN_PIXELS_PER_CM: f32 = 1.5;
pub const MAX_PIXELS_PER_CM: f32 = 20.0;

/// The Geometry tab's zoom state -- how many screen pixels one centimetre of
/// the R-Z model draws at. Owned by the caller (see
/// [`crate::app::HtgrSimApp::geometry_zoom`]) and persisted across frames, so
/// a zoom-button click and the resulting scroll position both survive to the
/// next repaint -- unlike [`draw_geometry`]'s data, this is genuinely GUI
/// state, not something rebuilt from the physics snapshot each frame.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ZoomLevel(f32);

impl Default for ZoomLevel {
    fn default() -> Self {
        Self(DEFAULT_PIXELS_PER_CM)
    }
}

impl ZoomLevel {
    /// Current pixels-per-centimetre.
    pub fn pixels_per_cm(&self) -> f32 {
        self.0
    }

    /// Step in by 25%, clamped to [`MAX_PIXELS_PER_CM`].
    pub fn zoom_in(&mut self) {
        self.0 = (self.0 * 1.25).min(MAX_PIXELS_PER_CM);
    }

    /// Step out by 20% (the inverse of [`Self::zoom_in`]'s 25% step, so the
    /// two are exact round trips), clamped to [`MIN_PIXELS_PER_CM`].
    pub fn zoom_out(&mut self) {
        self.0 = (self.0 / 1.25).max(MIN_PIXELS_PER_CM);
    }

    /// Back to [`DEFAULT_PIXELS_PER_CM`].
    pub fn reset(&mut self) {
        self.0 = DEFAULT_PIXELS_PER_CM;
    }
}

/// Draw the R-Z cross-section, data coordinates `r` (0-190 cm) horizontal and
/// `z` (0-610 cm) vertical -- z increasing downward needs no flip, since
/// screen `y` already increases downward, matching the benchmark's own
/// convention. Sized per `zoom`, not fit to the visible area -- see
/// [`ZoomLevel`]'s doc comment -- so the caller must wrap this in a scrolling
/// container; [`draw_geometry`] does.
fn draw_cross_section(ui: &mut Ui, zoom: ZoomLevel) {
    let zones = htr10_rz_zones();
    let r_max = radial_ticks_cm().iter().cloned().fold(0.0_f64, f64::max);
    let z_max = axial_ticks_cm().iter().cloned().fold(0.0_f64, f64::max);

    let margin = 24.0_f32;
    let scale = zoom.pixels_per_cm();
    let canvas_size = Vec2::new(
        r_max as f32 * scale + 2.0 * margin,
        z_max as f32 * scale + 2.0 * margin,
    );
    let (response, painter) = ui.allocate_painter(canvas_size, egui::Sense::hover());
    let rect = response.rect;

    let to_screen = |r_cm: f64, z_cm: f64| -> Pos2 {
        Pos2::new(
            rect.left() + margin + r_cm as f32 * scale,
            rect.top() + margin + z_cm as f32 * scale,
        )
    };

    // Background so the "material pending" white regions read against
    // something.
    painter.rect_filled(rect, 0.0, Color32::from_gray(30));

    // Group entries by benchmark volume number, preserving first-seen order
    // (a `Vec` alongside the map, not relying on hash-map iteration order,
    // so paint order is deterministic frame to frame). Volumes 7, 12 and 78
    // are each drawn across more than one z-sub-band in the source script
    // (see `Htr10RzZone`'s doc comment) -- grouping them here is what makes
    // the fill, outline, pebble lattice and label all read as ONE continuous
    // zone instead of visibly separate pieces with a seam at each internal
    // band boundary (2026-08-18 fix).
    let mut group_order: Vec<u32> = Vec::new();
    let mut groups: std::collections::HashMap<u32, Vec<&Htr10RzZone>> =
        std::collections::HashMap::new();
    for zone in &zones {
        groups
            .entry(zone.volume)
            .or_insert_with(Vec::new)
            .push(zone);
        if groups[&zone.volume].len() == 1 {
            group_order.push(zone.volume);
        }
    }

    for volume in group_order {
        let group = &groups[&volume];
        let material = group[0].material;
        let (fill, edge) = zone_colors(material);

        // FILL: every entry's render sub-polygons, no stroke -- an outline
        // drawn per sub-polygon is exactly what would put a visible seam at
        // an internal band boundary that isn't really there physically.
        for zone in group.iter() {
            for polygon in zone_render_polygons(zone) {
                let points: Vec<Pos2> = polygon.iter().map(|&(r, z)| to_screen(r, z)).collect();
                painter.add(egui::Shape::convex_polygon(points, fill, Stroke::NONE));
            }
        }

        // OUTLINE: one continuous perimeter for the whole group, not one per
        // drawn sub-band/sub-polygon.
        let (r_min, r_max, z_min, z_max) = {
            let mut r_min = f64::MAX;
            let mut r_max = f64::MIN;
            let mut z_min = f64::MAX;
            let mut z_max = f64::MIN;
            for zone in group.iter() {
                for &(r, z) in &zone.vertices_cm {
                    r_min = r_min.min(r);
                    r_max = r_max.max(r);
                    z_min = z_min.min(z);
                    z_max = z_max.max(z);
                }
            }
            (r_min, r_max, z_min, z_max)
        };
        if group.len() == 1 {
            // Single entry -- its own vertices_cm is already the true outer
            // boundary, whether a plain rectangle or (volume 48) the
            // L-shape; stroking it directly (not its fill-split render
            // sub-polygons) avoids a seam at the L-shape's own internal
            // fill-split line.
            let points: Vec<Pos2> = group[0]
                .vertices_cm
                .iter()
                .map(|&(r, z)| to_screen(r, z))
                .collect();
            painter.add(egui::Shape::closed_line(points, Stroke::new(1.0, edge)));
        } else {
            // Multiple entries -- confirmed 2026-08-18 to always be
            // same-r-range rectangles stacked in z (volumes 7, 12, 78), so
            // their combined bounding box is exactly the true outer
            // boundary, not an approximation of it.
            let corners = [
                to_screen(r_min, z_min),
                to_screen(r_max, z_min),
                to_screen(r_max, z_max),
                to_screen(r_min, z_max),
            ];
            painter.add(egui::Shape::closed_line(
                corners.to_vec(),
                Stroke::new(1.0, edge),
            ));
        }

        draw_pebble_lattice(&painter, group, &to_screen, scale);

        // Label position: the group's combined bounding-box centre, which
        // for a plain rectangle (or a stacked-rectangle group) is exactly
        // the vertex average, so this changes nothing for those zones --
        // except volume 48, whose L-shape makes a bounding-box centre (and
        // a naive vertex average alike) land outside the polygon, in the
        // notch. `generate_htr10_geometry.py` already found and fixed this
        // with an explicit override (`MANUAL_LABEL_POSITIONS = {48:
        // (158.1965, 246.882)}`), ported here unchanged.
        let (centroid_r, centroid_z) = if volume == 48 {
            (158.1965, 246.882)
        } else {
            ((r_min + r_max) / 2.0, (z_min + z_max) / 2.0)
        };
        let bbox_w_px = (r_max - r_min) as f32 * scale;
        let bbox_h_px = (z_max - z_min) as f32 * scale;
        // Skip labels too small to read rather than drawing illegible text.
        if bbox_w_px > 12.0 && bbox_h_px > 10.0 {
            let font_size = if bbox_w_px < 22.0 || bbox_h_px < 18.0 {
                8.0
            } else {
                11.0
            };
            let text_color = match material {
                ZoneMaterial::Bottom | ZoneMaterial::Hot | ZoneMaterial::TopReflector => {
                    Color32::WHITE
                }
                _ => Color32::BLACK,
            };
            painter.text(
                to_screen(centroid_r, centroid_z),
                Align2::CENTER_CENTER,
                volume.to_string(),
                FontId::proportional(font_size),
                text_color,
            );
        }
    }

    // Axis ticks: radial along the top, axial along the left, using the
    // same validated tick lists the geometry module's own self-consistency
    // test checks the zone data against.
    for &r in radial_ticks_cm() {
        let p = to_screen(r, 0.0);
        painter.text(
            Pos2::new(p.x, rect.top() + 4.0),
            Align2::CENTER_TOP,
            format_tick(r),
            FontId::proportional(8.0),
            Color32::from_gray(200),
        );
    }
    for &z in axial_ticks_cm() {
        let p = to_screen(0.0, z);
        painter.text(
            Pos2::new(rect.left() + 2.0, p.y),
            Align2::LEFT_CENTER,
            format_tick(z),
            FontId::proportional(8.0),
            Color32::from_gray(200),
        );
    }
}

/// A tick value with no more decimal places than it needs -- Rust's
/// `format!` has no Python-style `{:g}`, so this trims trailing zeros (and a
/// trailing `.`) off a fixed 3-decimal rendering by hand instead. Three
/// places is exactly enough for every value in [`radial_ticks_cm`] and
/// [`axial_ticks_cm`] (e.g. `167.793`).
fn format_tick(value: f64) -> String {
    let s = format!("{value:.3}");
    let s = s.trim_end_matches('0');
    s.trim_end_matches('.').to_string()
}

/// Draw the material legend as a wrapped row of coloured swatches.
fn draw_legend(ui: &mut Ui) {
    ui.horizontal_wrapped(|ui| {
        for (label, material) in legend_entries() {
            let (fill, edge) = zone_colors(material);
            let (rect, _) = ui.allocate_exact_size(Vec2::new(14.0, 14.0), egui::Sense::hover());
            ui.painter().rect_filled(rect, 2.0, fill);
            ui.painter()
                .rect_stroke(rect, 2.0, Stroke::new(1.0, edge), egui::StrokeKind::Outside);
            ui.label(label);
            ui.add_space(10.0);
        }
    });
}

/// Derived helium-gas-volume readouts, in litres, from
/// [`htr10_rz_geometry`]'s volume functions -- see that module's doc comment
/// for the porosity assumption and the NOT-VALIDATED caveat these inherit.
fn draw_helium_volumes(ui: &mut Ui) {
    use uom::si::volume::liter;

    ui.heading("Helium gas volumes (from this geometry)");
    egui::Grid::new("htr10_geometry_helium_volumes")
        .num_columns(2)
        .striped(true)
        .show(ui, |ui| {
            let row = |ui: &mut Ui, label: &str, litres: f64| {
                ui.label(label);
                ui.label(format!("{litres:.1} L"));
                ui.end_row();
            };
            row(
                ui,
                "Top cavity (zone 5)",
                top_cavity_helium_volume().get::<liter>(),
            );
            row(
                ui,
                "Pebble bed void (zone 99)",
                pebble_bed_helium_volume().get::<liter>(),
            );
            row(
                ui,
                "Dummy pebbles, conus (zone 91)",
                dummy_pebble_helium_volume_conus().get::<liter>(),
            );
            row(
                ui,
                "Dummy pebbles, upper discharge tube (zone 6)",
                dummy_pebble_helium_volume_upper_discharge_tube().get::<liter>(),
            );
            row(
                ui,
                "Dummy pebbles, middle discharge tube (zone 7)",
                dummy_pebble_helium_volume_middle_discharge_tube().get::<liter>(),
            );
            row(
                ui,
                "Dummy pebbles, lower discharge tube (zone 81)",
                dummy_pebble_helium_volume_lower_discharge_tube().get::<liter>(),
            );
        });
    ui.small(
        "Pebble-bed and dummy-pebble volumes assume the published 0.39 bed \
         porosity applies equally to dummy-pebble packing -- not independently \
         confirmed. NOT VALIDATED: hand-transcribed benchmark geometry, no \
         digitiser calibration record.",
    );
}

/// Top-level body for the "HTR-10 Geometry" panel. `zoom` is the caller's
/// persisted [`ZoomLevel`] (see [`crate::app::HtgrSimApp::geometry_zoom`]);
/// the zoom in/out/reset buttons here mutate it in place.
pub fn draw_geometry(ui: &mut Ui, zoom: &mut ZoomLevel) {
    ui.heading("HTR-10 simplified benchmark model -- R-Z zone geometry");
    ui.label(
        "Reconstructed from Terry et al. (2005) Fig. 2, via GitHub issue #23's \
         generate_htr10_geometry.py. R (cm) across, Z (cm) down. NOT VALIDATED \
         -- see crates/outram-park-digital-twin-engine's htr10_rz_geometry \
         module doc comment.",
    );
    ui.horizontal(|ui| {
        if ui
            .button("\u{1F50D}\u{2212} Zoom Out")
            .on_hover_text("Also: Ctrl - to zoom the whole window")
            .clicked()
        {
            zoom.zoom_out();
        }
        if ui.button("Reset").clicked() {
            zoom.reset();
        }
        if ui
            .button("\u{1F50D}+ Zoom In")
            .on_hover_text("Also: Ctrl + to zoom the whole window")
            .clicked()
        {
            zoom.zoom_in();
        }
        ui.label(format!("{:.1} px/cm", zoom.pixels_per_cm()));
    });
    ui.separator();
    draw_legend(ui);
    ui.separator();

    egui::ScrollArea::both()
        .id_salt("htr10_geometry_canvas_scroll")
        .show(ui, |ui| {
            draw_cross_section(ui, *zoom);
        });

    ui.separator();
    draw_helium_volumes(ui);
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Methodology: a fresh [`ZoomLevel`] must report exactly
    /// [`DEFAULT_PIXELS_PER_CM`], and [`ZoomLevel::reset`] after zooming away
    /// from it must return to exactly the same value -- not an
    /// accumulated-rounding-error approximation of it.
    ///
    /// Result (2026-08-18): both checks pass exactly (`==`, not a tolerance).
    #[test]
    fn default_and_reset_land_on_default_pixels_per_cm_exactly() {
        let zoom = ZoomLevel::default();
        assert_eq!(zoom.pixels_per_cm(), DEFAULT_PIXELS_PER_CM);

        let mut zoom = ZoomLevel::default();
        zoom.zoom_in();
        zoom.zoom_in();
        zoom.reset();
        assert_eq!(zoom.pixels_per_cm(), DEFAULT_PIXELS_PER_CM);
    }

    /// Methodology: repeated [`ZoomLevel::zoom_in`] must never exceed
    /// [`MAX_PIXELS_PER_CM`], and repeated [`ZoomLevel::zoom_out`] must never
    /// fall below [`MIN_PIXELS_PER_CM`] -- checked by driving each 100 steps
    /// past where the bound would already have been hit, so this is a
    /// clamping check, not merely "the bound is reachable".
    ///
    /// Result (2026-08-18): both bounds hold after 100 steps each.
    #[test]
    fn zoom_in_and_out_clamp_at_their_bounds() {
        let mut zoom = ZoomLevel::default();
        for _ in 0..100 {
            zoom.zoom_in();
        }
        assert_eq!(zoom.pixels_per_cm(), MAX_PIXELS_PER_CM);

        let mut zoom = ZoomLevel::default();
        for _ in 0..100 {
            zoom.zoom_out();
        }
        assert_eq!(zoom.pixels_per_cm(), MIN_PIXELS_PER_CM);
    }

    /// Methodology: one [`ZoomLevel::zoom_in`] immediately undone by one
    /// [`ZoomLevel::zoom_out`] (both comfortably inside the bounds, so
    /// neither step clamps) must return to the starting value, since
    /// `zoom_in` multiplies by 1.25 and `zoom_out` divides by the same
    /// 1.25 -- exact inverses algebraically (`x * 1.25 / 1.25 == x`).
    ///
    /// Result (2026-08-18): round-trips to within `1e-6` of the start (exact
    /// equality is not claimed, since `f32` multiply-then-divide is not
    /// bit-exact in general).
    #[test]
    fn zoom_in_then_zoom_out_round_trips_within_a_small_tolerance() {
        let mut zoom = ZoomLevel::default();
        let start = zoom.pixels_per_cm();
        zoom.zoom_in();
        zoom.zoom_out();
        assert!(
            (zoom.pixels_per_cm() - start).abs() < 1e-6,
            "expected {start}, got {}",
            zoom.pixels_per_cm()
        );
    }

    /// Methodology: run [`draw_geometry`] inside a real (headless) `egui`
    /// pass -- the same harness [`super::super::panels`]'s and
    /// `app_scaffold::crash`'s own tests use -- and confirm it does not
    /// panic, including the new zoom button row.
    ///
    /// Result (2026-08-18): completes without panicking.
    #[test]
    fn draw_geometry_does_not_panic_across_a_headless_egui_pass() {
        let mut zoom = ZoomLevel::default();
        let ctx = egui::Context::default();
        let _ = ctx.run_ui(egui::RawInput::default(), |ui| {
            draw_geometry(ui, &mut zoom);
        });
    }
}
