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

/// Draw a schematic pebble packing inside `zone`'s footprint -- a **body-
/// centred-cubic-projected lattice** (rows spaced by `pebble_diameter *
/// sqrt(3)/2`, each row offset by half a pitch from its neighbours, the
/// standard 2-D depiction of a BCC/close-packed cross-section), not a real
/// DEM packing. Only [`ZoneMaterial::Mixed`] (the settled bed, zone 99) and
/// [`ZoneMaterial::Dummy`] (dummy-pebble-only regions) actually hold discrete
/// pebbles; every other zone is a homogenised reflector or structure, so this
/// is a no-op for them.
///
/// Candidate centres are generated over each render polygon's own bounding
/// box and kept only if [`point_in_convex_polygon`] accepts them, so the
/// lattice stays inside the zone's true footprint rather than its bounding
/// box (matters for zone 91, the one non-rectangular pebble-bearing shape).
/// Circles are drawn as thin outlines, not filled, so the material colour
/// underneath still reads.
fn draw_pebble_lattice(
    painter: &egui::Painter,
    zone: &Htr10RzZone,
    to_screen: &impl Fn(f64, f64) -> Pos2,
    scale: f32,
) {
    if !matches!(zone.material, ZoneMaterial::Mixed | ZoneMaterial::Dummy) {
        return;
    }

    let pitch_cm = pebble_diameter().get::<centimeter>();
    let row_pitch_cm = pitch_cm * 3.0_f64.sqrt() / 2.0;
    let stroke = Stroke::new(0.6, Color32::from_black_alpha(130));
    let circle_radius_px = (pitch_cm * 0.42) as f32 * scale;

    for polygon in zone_render_polygons(zone) {
        let r_min = polygon.iter().map(|(r, _)| *r).fold(f64::MAX, f64::min);
        let r_max = polygon.iter().map(|(r, _)| *r).fold(f64::MIN, f64::max);
        let z_min = polygon.iter().map(|(_, z)| *z).fold(f64::MAX, f64::min);
        let z_max = polygon.iter().map(|(_, z)| *z).fold(f64::MIN, f64::max);

        let mut row = 0u32;
        let mut z = z_min + pitch_cm / 2.0;
        while z < z_max {
            let offset_cm = if row % 2 == 0 { 0.0 } else { pitch_cm / 2.0 };
            let mut r = r_min + pitch_cm / 2.0 + offset_cm;
            while r < r_max {
                if point_in_convex_polygon((r, z), &polygon) {
                    painter.circle_stroke(to_screen(r, z), circle_radius_px, stroke);
                }
                r += pitch_cm;
            }
            z += row_pitch_cm;
            row += 1;
        }
    }
}

/// Pixels per centimetre the cross-section is drawn at.
///
/// **Fixed, not fit-to-viewport.** An earlier version scaled the canvas to
/// whatever space happened to be visible (capped at 900 px tall), which for
/// this model's 190 x 610 cm extent forced a scale far too small to make out
/// its narrower columns -- volumes 21 (5.6 cm wide) and 57 (8 cm wide) were
/// reported unreadable at that scale. This constant instead sizes the canvas
/// to its true extent regardless of viewport size, and the surrounding
/// `ScrollArea` in [`draw_geometry`] is what lets a reader pan around a
/// canvas larger than the screen -- exactly what a `ScrollArea` is for. 5.0
/// px/cm puts even the narrowest column (21, 57) at 28-40 px wide, well
/// clear of the width the label-skip threshold below needs.
const PIXELS_PER_CM: f32 = 5.0;

/// Draw the R-Z cross-section, data coordinates `r` (0-190 cm) horizontal and
/// `z` (0-610 cm) vertical -- z increasing downward needs no flip, since
/// screen `y` already increases downward, matching the benchmark's own
/// convention. Sized per [`PIXELS_PER_CM`], not fit to the visible area --
/// see that constant's doc comment -- so the caller must wrap this in a
/// scrolling container; [`draw_geometry`] does.
fn draw_cross_section(ui: &mut Ui) {
    let zones = htr10_rz_zones();
    let r_max = radial_ticks_cm().iter().cloned().fold(0.0_f64, f64::max);
    let z_max = axial_ticks_cm().iter().cloned().fold(0.0_f64, f64::max);

    let margin = 24.0_f32;
    let scale = PIXELS_PER_CM;
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

    for zone in &zones {
        let (fill, edge) = zone_colors(zone.material);
        for polygon in zone_render_polygons(zone) {
            let points: Vec<Pos2> = polygon.iter().map(|&(r, z)| to_screen(r, z)).collect();
            painter.add(egui::Shape::convex_polygon(
                points.clone(),
                fill,
                Stroke::new(1.0, edge),
            ));
        }
        draw_pebble_lattice(&painter, zone, &to_screen, scale);

        // Label position. A plain vertex average is exactly right for every
        // zone here except 48: its L-shape makes that average land outside
        // the polygon (in the notch), a problem `generate_htr10_geometry.py`
        // already found and fixed with an explicit override
        // (`MANUAL_LABEL_POSITIONS = {48: (158.1965, 246.882)}`) -- ported
        // here unchanged rather than re-deriving it.
        let (centroid_r, centroid_z) = if zone.volume == 48 {
            (158.1965, 246.882)
        } else {
            (
                zone.vertices_cm.iter().map(|(r, _)| r).sum::<f64>()
                    / zone.vertices_cm.len() as f64,
                zone.vertices_cm.iter().map(|(_, z)| z).sum::<f64>()
                    / zone.vertices_cm.len() as f64,
            )
        };
        let bbox_w_px = (zone
            .vertices_cm
            .iter()
            .map(|(r, _)| *r)
            .fold(f64::MIN, f64::max)
            - zone
                .vertices_cm
                .iter()
                .map(|(r, _)| *r)
                .fold(f64::MAX, f64::min)) as f32
            * scale;
        let bbox_h_px = (zone
            .vertices_cm
            .iter()
            .map(|(_, z)| *z)
            .fold(f64::MIN, f64::max)
            - zone
                .vertices_cm
                .iter()
                .map(|(_, z)| *z)
                .fold(f64::MAX, f64::min)) as f32
            * scale;
        // Skip labels too small to read rather than drawing illegible text.
        if bbox_w_px > 12.0 && bbox_h_px > 10.0 {
            let font_size = if bbox_w_px < 22.0 || bbox_h_px < 18.0 {
                8.0
            } else {
                11.0
            };
            let text_color = match zone.material {
                ZoneMaterial::Bottom | ZoneMaterial::Hot | ZoneMaterial::TopReflector => {
                    Color32::WHITE
                }
                _ => Color32::BLACK,
            };
            painter.text(
                to_screen(centroid_r, centroid_z),
                Align2::CENTER_CENTER,
                zone.volume.to_string(),
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

/// Top-level body for the "HTR-10 Geometry" panel.
pub fn draw_geometry(ui: &mut Ui) {
    ui.heading("HTR-10 simplified benchmark model -- R-Z zone geometry");
    ui.label(
        "Reconstructed from Terry et al. (2005) Fig. 2, via GitHub issue #23's \
         generate_htr10_geometry.py. R (cm) across, Z (cm) down. NOT VALIDATED \
         -- see crates/outram-park-digital-twin-engine's htr10_rz_geometry \
         module doc comment.",
    );
    ui.separator();
    draw_legend(ui);
    ui.separator();

    egui::ScrollArea::both()
        .id_salt("htr10_geometry_canvas_scroll")
        .show(ui, |ui| {
            draw_cross_section(ui);
        });

    ui.separator();
    draw_helium_volumes(ui);
}
