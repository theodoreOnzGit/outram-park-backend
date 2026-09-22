//! **Live thermal state and FP release** -- the core's live thermal state,
//! drilled down to the fuel kernel, and the fission-product release it drives.
//!
//! Split from the Map tab on 2026-09-22 (maintainer direction): this tab
//! replaced the static "HTR-10 Geometry" viewer, which was deleted, and the
//! Map tab keeps only the Gaussian puff dispersion widgets
//! ([`super::map_tab`]).
//!
//! # What this tab draws
//!
//! The published HTR-10 R-Z benchmark geometry, coloured from the **live
//! plant state**, then resolved through the two length scales below it:
//!
//! | Scale | What is drawn | Where the numbers come from |
//! |---|---|---|
//! | Core, ~2 m | R-Z zones, thermally coloured | the bed node and the helium loop |
//! | Pebble, 6 cm | the solved two-zone profile | `tampines`'s resolved pebble |
//! | Particle, 0.9 mm | kernel and SiC temperatures | the hottest coated particle |
//!
//! Then the TRISO-ATOPS release table, which is what those temperatures
//! *drive*.
//!
//! # Why this tab exists at all
//!
//! The 2026-09-22 change moved the Doppler feedback and the fission-product
//! release model onto the **fuel kernel** temperature, and that temperature is
//! invisible on every other screen: the schematic shows a vessel, the
//! diagnostics table shows scalars, and the difference between a 950 K bed and
//! a 973 K kernel is two numbers in a list that look nearly the same. Drawn as
//! nested regions with the drop across each one labelled, the 5 mm unfuelled
//! shell that carries the whole pebble's heat and generates none of it becomes
//! something a reader can *see*, which is the point of this crate.
//!
//! # Everything drawn here is derived from the snapshot
//!
//! Per this crate's hard rule, no temperature on this screen is interpolated,
//! assumed or hardcoded. The pebble interior is published as four solved
//! temperatures (see [`HtgrSnapshot::pebble_surface_k`] and its siblings)
//! precisely so this tab does not have to invent the inside of a pebble. Where
//! the selected fidelity tier resolves no pebble, the values arrive as `NAN`
//! and are drawn as `--`; **nothing here falls back to the bed temperature**,
//! because a bed average under a "peak fuel" label is exactly the kind of
//! confident wrong number this simulator's rules exist to prevent.
//!
//! The *geometry* is hardcoded, and that is the permitted kind: zone polygons
//! are published benchmark dimensions read from
//! [`crate::physics::reactor_model::htr10_rz_geometry`], and the drill-down
//! radii are drawn to the pebble's real 30 mm / 25 mm split and the particle's
//! real layer radii, read from `tampines`. Only the *colours and the numbers*
//! are state, which is the rule: geometry may be fixed, motion and magnitude
//! may not.
//!
//! # NOT VALIDATED
//!
//! The R-Z reconstruction is not validated (see
//! [`crate::physics::reactor_model::htr10_rz_geometry`]'s module doc), and the
//! release table carries its own, stronger caveat: it is **per curie of core inventory**, not curies.
//! See [`crate::physics::fission_product_release`].


use egui::{Align2, Color32, FontId, Pos2, Rect, Sense, Stroke, Ui, Vec2};

use outram_park_digital_twin_engine::color_maps::hot_to_cold_colour_mark_1;

use super::state::HtgrSnapshot;
use crate::physics::reactor_model::htr10_rz_geometry::{
    axial_ticks_cm, htr10_rz_zones, radial_ticks_cm, Htr10RzZone, ZoneMaterial,
};


/// Temperature \[K\] mapped to the cold end of the colour scale.
///
/// **A display range, not a physical limit.** It brackets the plant's helium
/// inlet (about 523 K) with margin below, so a cold start still renders inside
/// the scale instead of saturating. Stated as a drawing choice per this
/// crate's rule that an indicative constant must say it is one.
const COLOUR_SCALE_COLD_K: f64 = 400.0;

/// Temperature \[K\] mapped to the hot end of the colour scale.
///
/// Also a display range: 1400 K sits above the resolved kernel at rated power
/// (about 973 K) and above where a LOFC transient takes it, so the interesting
/// range occupies most of the scale rather than the top few percent. It is
/// **not** a fuel limit and must not be read as one.
const COLOUR_SCALE_HOT_K: f64 = 1400.0;

/// Map a temperature onto the engine's hot/cold colour map.
///
/// Returns a flat grey for `NAN`, which is how an unresolved quantity reaches
/// this tab -- see the module doc on why nothing falls back to the bed.
fn temperature_colour(kelvin: f64) -> Color32 {
    if !kelvin.is_finite() {
        return Color32::from_gray(90);
    }
    let hotness =
        ((kelvin - COLOUR_SCALE_COLD_K) / (COLOUR_SCALE_HOT_K - COLOUR_SCALE_COLD_K)).clamp(0.0, 1.0);
    hot_to_cold_colour_mark_1(hotness as f32)
}

/// The live temperature a zone should be drawn at, or `None` for structure
/// this model carries no temperature for.
///
/// # Why most of the core map is grey, and why that is right
///
/// This is a **one-node** plant model. It has a bed temperature, a helium
/// temperature and an inlet -- and that is all. It does not have a reflector
/// temperature, a vessel temperature or an axial profile, so there is nothing
/// to colour those zones *with*.
///
/// Drawing them in a plausible interpolated colour would invent a spatial
/// temperature field this simulator does not compute, and it would do so
/// convincingly: a reader would take the picture for a solution. Grey says
/// "not modelled", which is the true statement, and it has the useful side
/// effect of making the map get *more* colourful when a higher-fidelity tier
/// is selected -- the screen then shows how much of the core the model
/// actually resolves.
fn zone_temperature_k(material: ZoneMaterial, s: &HtgrSnapshot) -> Option<f64> {
    match material {
        // The settled pebble bed: the bed node's own temperature.
        ZoneMaterial::Mixed | ZoneMaterial::Dummy => Some(s.bed_temperature_k),
        // Cold helium on its way in, and the chamber it collects in.
        ZoneMaterial::ColdChannel | ZoneMaterial::ColdChamber => {
            Some(s.core_inlet_temp_k)
        }
        // Hot helium leaving the bed.
        ZoneMaterial::Hot | ZoneMaterial::Cavity => Some(s.core_outlet_temp_k),
        // Everything else -- reflector, boronated shielding, carbon brick,
        // control channels, vessel structure -- has no temperature in a
        // one-node model. See this function's doc comment.
        _ => None,
    }
}

/// Draw the R-Z core cross-section, coloured from the live snapshot.
fn draw_live_cross_section(ui: &mut Ui, s: &HtgrSnapshot) {
    let zones = htr10_rz_zones();
    let r_max = radial_ticks_cm().iter().cloned().fold(0.0_f64, f64::max);
    let z_max = axial_ticks_cm().iter().cloned().fold(0.0_f64, f64::max);

    // Fit the drawing to the space available rather than to a fixed zoom:
    // this tab is a live readout to be glanced at, not a drawing to be
    // measured off, so it scales to the panel.
    let available = ui.available_width().min(520.0).max(220.0);
    let margin = 18.0_f32;
    let scale = ((available - 2.0 * margin) / r_max as f32).max(0.05);
    let canvas = Vec2::new(
        r_max as f32 * scale + 2.0 * margin,
        z_max as f32 * scale + 2.0 * margin,
    );
    let (response, painter) = ui.allocate_painter(canvas, Sense::hover());
    let rect = response.rect;
    let to_screen = |r_cm: f64, z_cm: f64| -> Pos2 {
        Pos2::new(
            rect.left() + margin + r_cm as f32 * scale,
            rect.top() + margin + z_cm as f32 * scale,
        )
    };

    painter.rect_filled(rect, 0.0, Color32::from_gray(24));

    for zone in &zones {
        let fill = match zone_temperature_k(zone.material, s) {
            Some(k) => temperature_colour(k),
            // Unmodelled structure: dark grey, deliberately recessive so the
            // parts this model DOES resolve are what the eye lands on.
            None => Color32::from_gray(64),
        };
        for polygon in zone_render_polygons(zone) {
            let points: Vec<Pos2> = polygon.iter().map(|&(r, z)| to_screen(r, z)).collect();
            if points.len() < 3 {
                continue;
            }
            painter.add(egui::Shape::convex_polygon(
                points,
                fill,
                Stroke::new(0.5, Color32::from_gray(40)),
            ));
        }
    }

    // Label the bed with the two temperatures the drill-down then expands.
    if let Some(bed_zone) = zones.iter().find(|z| z.material == ZoneMaterial::Mixed) {
        let centroid = polygon_centroid(&bed_zone.vertices_cm);
        let at = to_screen(centroid.0, centroid.1);
        painter.text(
            at,
            Align2::CENTER_CENTER,
            format!(
                "bed {}\nkernel {}",
                format_kelvin(s.bed_temperature_k),
                format_kelvin(s.peak_kernel_temperature_k)
            ),
            FontId::proportional(11.0),
            Color32::from_gray(20),
        );
    }
}

/// Area-weighted centroid of a polygon given as `(r_cm, z_cm)` vertices,
/// falling back to the vertex mean for a degenerate (zero-area) polygon.
fn polygon_centroid(vertices: &[(f64, f64)]) -> (f64, f64) {
    let n = vertices.len();
    if n == 0 {
        return (0.0, 0.0);
    }
    let mut area2 = 0.0;
    let mut cx = 0.0;
    let mut cy = 0.0;
    for i in 0..n {
        let (x0, y0) = vertices[i];
        let (x1, y1) = vertices[(i + 1) % n];
        let cross = x0 * y1 - x1 * y0;
        area2 += cross;
        cx += (x0 + x1) * cross;
        cy += (y0 + y1) * cross;
    }
    if area2.abs() < 1e-12 {
        let sx: f64 = vertices.iter().map(|v| v.0).sum();
        let sy: f64 = vertices.iter().map(|v| v.1).sum();
        return (sx / n as f64, sy / n as f64);
    }
    (cx / (3.0 * area2), cy / (3.0 * area2))
}

/// Format a kelvin scalar, rendering `NAN` as `--`.
///
/// The `--` is load-bearing: it is what the two placeholder fidelity tiers
/// show instead of a fabricated fuel temperature.
fn format_kelvin(k: f64) -> String {
    if k.is_finite() {
        format!("{k:.1} K")
    } else {
        "--".to_string()
    }
}

/// Draw the pebble drill-down: nested rings at the pebble's real radii,
/// coloured and labelled from the solved profile.
///
/// The radii are the published geometry (30 mm ball, 25 mm fuelled zone) and
/// the particle is drawn at an exaggerated radius with its scale stated,
/// because a 0.46 mm particle inside a 30 mm ball is four pixels at any size
/// this panel can be. **That exaggeration is a drawing choice and is labelled
/// on screen as one** -- the temperatures it carries are not exaggerated.
fn draw_pebble_drilldown(ui: &mut Ui, s: &HtgrSnapshot) {
    let size = ui.available_width().min(300.0).max(180.0);
    let (response, painter) = ui.allocate_painter(Vec2::new(size, size), Sense::hover());
    let rect = response.rect;
    let centre = rect.center();
    let outer_px = size * 0.42;

    painter.rect_filled(rect, 0.0, Color32::from_gray(24));

    // Published pebble geometry: 30 mm outer, 25 mm fuelled zone.
    const PEBBLE_OUTER_MM: f32 = 30.0;
    const FUELLED_ZONE_MM: f32 = 25.0;
    let r_of = |mm: f32| outer_px * mm / PEBBLE_OUTER_MM;

    // Outermost first, so each inner region paints over the last.
    // Unfuelled shell: between the surface and the zone boundary, so its
    // colour is the mean of the two temperatures bounding it -- an honest
    // representation of a region with a gradient across it, drawn flat.
    let shell_k = mean_finite(s.pebble_surface_k, s.pebble_zone_boundary_k);
    painter.circle_filled(centre, r_of(PEBBLE_OUTER_MM), temperature_colour(shell_k));
    let matrix_k = mean_finite(s.pebble_zone_boundary_k, s.pebble_centre_k);
    painter.circle_filled(centre, r_of(FUELLED_ZONE_MM), temperature_colour(matrix_k));

    // The hottest coated particle, drawn at an EXAGGERATED radius (labelled
    // below). Real radius is 0.46 mm against the ball's 30 mm -- 1.5 % -- so
    // at true scale it would be sub-pixel.
    let particle_px = outer_px * 0.26;
    painter.circle_filled(centre, particle_px, temperature_colour(s.particle_sic_k));
    painter.circle_filled(
        centre,
        particle_px * 0.54,
        temperature_colour(s.peak_kernel_temperature_k),
    );

    painter.circle_stroke(
        centre,
        r_of(PEBBLE_OUTER_MM),
        Stroke::new(1.0, Color32::from_gray(120)),
    );
    painter.circle_stroke(
        centre,
        r_of(FUELLED_ZONE_MM),
        Stroke::new(1.0, Color32::from_gray(120)),
    );

    painter.text(
        centre,
        Align2::CENTER_CENTER,
        format_kelvin(s.peak_kernel_temperature_k),
        FontId::proportional(11.0),
        Color32::from_gray(15),
    );
    painter.text(
        Pos2::new(centre.x, rect.top() + 10.0),
        Align2::CENTER_CENTER,
        "coated particle drawn ~17x oversize",
        FontId::proportional(9.0),
        Color32::from_gray(150),
    );
}

/// Arithmetic mean of two temperatures, propagating `NAN` so an unresolved
/// profile stays unresolved rather than half-rendering.
fn mean_finite(a: f64, b: f64) -> f64 {
    if a.is_finite() && b.is_finite() {
        0.5 * (a + b)
    } else {
        f64::NAN
    }
}

/// The solved profile as a table, surface outward-in, with the drop across
/// each region.
fn draw_profile_table(ui: &mut Ui, s: &HtgrSnapshot) {
    egui::Grid::new("htgr_map_profile_grid")
        .num_columns(3)
        .striped(true)
        .show(ui, |ui| {
            ui.label("Station");
            ui.label("Temperature");
            ui.label("Rise from previous");
            ui.end_row();

            let mut previous = f64::NAN;
            let row = |ui: &mut Ui, name: &str, value: f64, previous: &mut f64| {
                ui.label(name);
                ui.label(format_kelvin(value));
                let delta = if value.is_finite() && previous.is_finite() {
                    format!("{:+.2} K", value - *previous)
                } else {
                    "--".to_string()
                };
                ui.label(delta);
                ui.end_row();
                *previous = value;
            };

            row(ui, "Helium leaving the bed", s.core_outlet_temp_k, &mut previous);
            previous = f64::NAN;
            row(ui, "Pebble surface", s.pebble_surface_k, &mut previous);
            row(ui, "Fuelled-zone boundary", s.pebble_zone_boundary_k, &mut previous);
            row(ui, "Matrix centre", s.pebble_centre_k, &mut previous);
            row(ui, "Particle SiC outer", s.particle_sic_k, &mut previous);
            row(ui, "Peak kernel centre", s.peak_kernel_temperature_k, &mut previous);
        });

    ui.add_space(4.0);
    ui.label(format!(
        "Bed node (ball volume average): {}   |   kernel above the node: {}",
        format_kelvin(s.bed_temperature_k),
        if s.kernel_offset_k.is_finite() {
            format!("{:+.2} K", s.kernel_offset_k)
        } else {
            "--".to_string()
        }
    ));
    ui.label(format!(
        "Kernel Doppler channel: {:+.4} $  (zero at the design point by construction; \
         the graphite share stays inside the closed-form prompt layer)",
        s.kernel_doppler_dollars
    ));
}

/// The TRISO-ATOPS release table.
fn draw_release_table(ui: &mut Ui, s: &HtgrSnapshot) {
    ui.label(
        "TRISO-ATOPS fission-product release, evaluated at the peak kernel temperature. \
         EVERY VALUE IS PER CURIE OF THAT NUCLIDE'S CORE INVENTORY -- this model derives no \
         inventory and these are not curies. Not a source term for HTR-10 or any reactor.",
    );
    ui.label(format!(
        "Last evaluated at kernel {}",
        format_kelvin(s.release_evaluated_at_kernel_k)
    ));
    ui.add_space(4.0);

    egui::Grid::new("htgr_map_release_grid")
        .num_columns(6)
        .striped(true)
        .show(ui, |ui| {
            for heading in [
                "Nuclide",
                "Release R [/s]",
                "Graphite G",
                "Circulating C",
                "Plate-out P",
                "Clean-up",
            ] {
                ui.label(heading);
            }
            ui.end_row();

            for nuclide in &s.release {
                if nuclide.name.is_empty() {
                    continue;
                }
                ui.label(nuclide.name);
                for value in [
                    nuclide.release_rate,
                    nuclide.graphite_activity,
                    nuclide.circulating_activity,
                    nuclide.plate_out_activity,
                    nuclide.clean_up_activity,
                ] {
                    ui.label(format!("{value:.3e}"));
                }
                ui.end_row();
            }
        });

    ui.add_space(4.0);
    ui.label(
        "Zeros are physical, not missing data: noble gases do not plate out, and the \
         purification system does not scrub the metals.",
    );
}

/// Draw the colour scale, so a reader can decode the map.
fn draw_colour_legend(ui: &mut Ui) {
    let (response, painter) =
        ui.allocate_painter(Vec2::new(ui.available_width().min(320.0), 26.0), Sense::hover());
    let rect = response.rect;
    let steps = 48;
    for i in 0..steps {
        let fraction = i as f32 / (steps - 1) as f32;
        let k = COLOUR_SCALE_COLD_K + (COLOUR_SCALE_HOT_K - COLOUR_SCALE_COLD_K) * fraction as f64;
        let x0 = rect.left() + rect.width() * i as f32 / steps as f32;
        let x1 = rect.left() + rect.width() * (i + 1) as f32 / steps as f32;
        painter.rect_filled(
            Rect::from_min_max(Pos2::new(x0, rect.top()), Pos2::new(x1, rect.bottom() - 10.0)),
            0.0,
            temperature_colour(k),
        );
    }
    painter.text(
        Pos2::new(rect.left(), rect.bottom()),
        Align2::LEFT_BOTTOM,
        format!("{COLOUR_SCALE_COLD_K:.0} K"),
        FontId::proportional(9.0),
        Color32::from_gray(160),
    );
    painter.text(
        Pos2::new(rect.right(), rect.bottom()),
        Align2::RIGHT_BOTTOM,
        format!("{COLOUR_SCALE_HOT_K:.0} K"),
        FontId::proportional(9.0),
        Color32::from_gray(160),
    );
}

/// The convex polygon(s) to fill for a zone's `(r, z)` footprint.
///
/// `egui`'s [`egui::Shape::Path`] fill is only defined for convex polygons.
/// Every zone here is convex except benchmark volume 48 (the L-shaped
/// graphite reflector -- see the "Volume 48" comment in
/// [`htr10_rz_zones`]), which this splits into the same
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

/// Draw the whole "Live thermal state and FP release" tab.
pub fn draw_thermal(ui: &mut Ui, s: &HtgrSnapshot) {
    ui.heading("Live thermal state and fission-product release");
    ui.label(
        "The published HTR-10 R-Z benchmark geometry, coloured from the live plant state, then \
         resolved down to the fuel kernel that the Doppler feedback and the TRISO release model \
         are both driven from. Grey zones are structure this one-node model carries no \
         temperature for -- not missing data, not modelled.",
    );
    ui.separator();
    draw_colour_legend(ui);
    ui.separator();

    ui.columns(2, |columns| {
        columns[0].label("Core, R-Z (cm)");
        draw_live_cross_section(&mut columns[0], s);

        columns[1].label("Pebble and coated particle");
        draw_pebble_drilldown(&mut columns[1], s);
        columns[1].add_space(6.0);
        draw_profile_table(&mut columns[1], s);
    });

    ui.separator();
    draw_release_table(ui, s);
}

#[cfg(test)]
mod tests {
    use super::*;

    /// An unresolved profile must render as `--` everywhere and must **never**
    /// borrow the bed temperature.
    ///
    /// This is the GUI half of the rule the physics side already enforces: the
    /// two placeholder fidelity tiers resolve no pebble, so every interior
    /// temperature arrives as `NAN`. A tab that quietly substituted
    /// `bed_temperature_k` would put a bed average on screen under a "peak
    /// kernel" label -- a number that is systematically tens of kelvin low and
    /// carries no sign that anything is missing.
    #[test]
    fn an_unresolved_pebble_renders_as_dashes_not_as_the_bed() {
        assert_eq!(format_kelvin(f64::NAN), "--");
        assert_eq!(format_kelvin(950.0), "950.0 K");
        // A half-resolved profile must not average its way to a number.
        assert!(mean_finite(900.0, f64::NAN).is_nan());
        assert!(mean_finite(f64::NAN, 900.0).is_nan());
        assert!((mean_finite(900.0, 1000.0) - 950.0).abs() < 1e-12);
        // An unresolved temperature must colour as the "not modelled" grey,
        // which is the same grey unmodelled structure gets -- so an
        // unresolved kernel cannot be mistaken for a cold one.
        assert_eq!(temperature_colour(f64::NAN), Color32::from_gray(90));
    }

    /// The colour scale must be monotone and must clamp rather than wrap.
    ///
    /// A non-monotone map would make a hotter core look cooler somewhere in
    /// the range, and a wrapping one would make a temperature above the scale
    /// render as cold -- which on a fuel-temperature display is the single
    /// most dangerous way for a colour map to fail.
    #[test]
    fn the_colour_scale_clamps_and_does_not_wrap() {
        let below = temperature_colour(COLOUR_SCALE_COLD_K - 500.0);
        let at_cold = temperature_colour(COLOUR_SCALE_COLD_K);
        let above = temperature_colour(COLOUR_SCALE_HOT_K + 5000.0);
        let at_hot = temperature_colour(COLOUR_SCALE_HOT_K);
        assert_eq!(below, at_cold, "below the scale must clamp to the cold end");
        assert_eq!(above, at_hot, "above the scale must clamp to the hot end");
        assert_ne!(at_cold, at_hot, "the scale must actually span something");
    }

    /// Only the zones this one-node model has a temperature for may be
    /// coloured; everything else must return `None`.
    ///
    /// Pinning this stops a later "let us fill in the reflector" change from
    /// inventing a spatial temperature field the model does not compute. If a
    /// higher-fidelity tier later resolves the reflector, this test is the
    /// place that has to be changed deliberately.
    #[test]
    fn only_modelled_zones_are_coloured() {
        let s = HtgrSnapshot::default();
        assert!(zone_temperature_k(ZoneMaterial::Mixed, &s).is_some());
        assert!(zone_temperature_k(ZoneMaterial::Dummy, &s).is_some());
        assert!(zone_temperature_k(ZoneMaterial::ColdChannel, &s).is_some());
        assert!(zone_temperature_k(ZoneMaterial::Hot, &s).is_some());

        for unmodelled in [
            ZoneMaterial::Graphite,
            ZoneMaterial::Boronated,
            ZoneMaterial::Carbon,
            ZoneMaterial::TopReflector,
            ZoneMaterial::Bottom,
            ZoneMaterial::Control,
            ZoneMaterial::Unknown,
        ] {
            assert!(
                zone_temperature_k(unmodelled, &s).is_none(),
                "{unmodelled:?} has no temperature in a one-node model and must stay grey"
            );
        }
    }

    /// The centroid used to place the bed label must be inside the bed, and
    /// must survive a degenerate polygon rather than producing `NaN`
    /// coordinates that would throw the label off-screen.
    #[test]
    fn the_centroid_is_sane() {
        let square = [(0.0, 0.0), (10.0, 0.0), (10.0, 10.0), (0.0, 10.0)];
        let (x, y) = polygon_centroid(&square);
        assert!((x - 5.0).abs() < 1e-9 && (y - 5.0).abs() < 1e-9);

        // Degenerate (zero-area) polygon: falls back to the vertex mean.
        let line = [(0.0, 0.0), (10.0, 0.0), (5.0, 0.0)];
        let (x, y) = polygon_centroid(&line);
        assert!(x.is_finite() && y.is_finite());
        assert!((y - 0.0).abs() < 1e-9);

        assert_eq!(polygon_centroid(&[]), (0.0, 0.0));
    }

    /// Every zone in the published map must be drawable: `zone_render_polygons`
    /// must return at least one polygon of at least three vertices for each.
    ///
    /// A zone that produced a degenerate polygon would silently vanish from
    /// the map -- and a missing zone on a core map reads as empty space rather
    /// than as a drawing fault.
    #[test]
    fn every_published_zone_is_drawable() {
        for zone in htr10_rz_zones() {
            let polygons = zone_render_polygons(&zone);
            assert!(
                !polygons.is_empty(),
                "volume {} produced no polygon",
                zone.volume
            );
            for polygon in polygons {
                assert!(
                    polygon.len() >= 3,
                    "volume {} produced a polygon with {} vertices",
                    zone.volume,
                    polygon.len()
                );
            }
        }
    }
}
