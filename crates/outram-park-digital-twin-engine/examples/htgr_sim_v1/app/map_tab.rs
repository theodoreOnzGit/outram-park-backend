//! **Map** -- Gaussian puff atmospheric dispersion around the plant.
//!
//! Since 2026-09-22 (maintainer direction) this tab holds only the dispersion
//! widgets: the live core map, the pebble and TRISO particle drill-down and
//! the release table moved to [`super::thermal_tab`] ("Live thermal state and
//! FP release"), which replaced the static geometry viewer.
//!
//! # NOT VALIDATED
//!
//! The puff model is `changi::puff` (ported from R `puff` 0.1.1), not FLEXPART,
//! and it is driven by the release the thermal tab shows, which is itself
//! **per curie of core inventory**. See
//! [`crate::physics::atmospheric_dispersion`].

use egui::{Align2, Color32, FontId, Pos2, Sense, Stroke, Ui, Vec2};

use outram_park_digital_twin_engine::color_maps::hot_to_cold_colour_mark_1;

use super::state::HtgrSnapshot;

/// Draw the dispersion rose: receptors as a polar plot around the release
/// point, sized and shaded by `chi/Q`.
///
/// # Why a rose and not a contour plot
///
/// A contour plot of a Gaussian plume looks authoritative and would be, here,
/// an interpolation between 24 evaluated points -- a picture of a plume rather
/// than a readout of one, which this crate's rule forbids. The rose draws
/// exactly the points that were computed and nothing between them. It also
/// makes the sector structure visible, so nobody mistakes the resolution for
/// finer than it is.
fn draw_dispersion_rose(ui: &mut Ui, s: &HtgrSnapshot) {
    let size = ui.available_width().min(320.0).max(200.0);
    let (response, painter) = ui.allocate_painter(Vec2::new(size, size), Sense::hover());
    let rect = response.rect;
    let centre = rect.center();
    let max_radius = size * 0.40;

    painter.rect_filled(rect, 0.0, Color32::from_gray(24));

    let outermost = s
        .receptors
        .iter()
        .map(|r| r.distance_m)
        .fold(0.0_f64, f64::max);
    if outermost <= 0.0 {
        painter.text(
            centre,
            Align2::CENTER_CENTER,
            "no dispersion run yet",
            FontId::proportional(11.0),
            Color32::from_gray(150),
        );
        return;
    }

    // Distance rings, drawn at the real radii so the plot is to scale.
    let mut drawn: Vec<f64> = Vec::new();
    for receptor in &s.receptors {
        if receptor.distance_m > 0.0 && !drawn.iter().any(|d| (d - receptor.distance_m).abs() < 1e-9)
        {
            drawn.push(receptor.distance_m);
        }
    }
    for distance in &drawn {
        let r = max_radius * (*distance / outermost) as f32;
        painter.circle_stroke(centre, r, Stroke::new(0.5, Color32::from_gray(70)));
    }

    // The peak sets the shading scale: chi/Q spans orders of magnitude between
    // upwind and downwind, so a LINEAR scale would render everything but the
    // plume centreline as black. The scale is therefore logarithmic over four
    // decades below the peak, and that is stated on screen rather than left
    // for a reader to infer from a picture that would otherwise mislead.
    let peak = s
        .receptors
        .iter()
        .map(|r| r.chi_over_q)
        .fold(0.0_f64, f64::max);

    for receptor in &s.receptors {
        if receptor.distance_m <= 0.0 {
            continue;
        }
        let (x, y) = bearing_to_plot(receptor.bearing_deg, receptor.distance_m / outermost);
        let at = Pos2::new(
            centre.x + max_radius * x as f32,
            // Screen y grows downward while north is up, so the plot y is
            // negated. Not a sign error -- the inverse of one.
            centre.y - max_radius * y as f32,
        );
        let shade = log_shade(receptor.chi_over_q, peak);
        painter.circle_filled(at, 6.0, shade);
        painter.circle_stroke(at, 6.0, Stroke::new(0.5, Color32::from_gray(90)));
    }

    // The wind arrow, drawn pointing the way the plume TRAVELS -- the opposite
    // of the meteorological "from" bearing the operator dials in. Derived from
    // the snapshot, never from the layout.
    let travel_deg = s.wind_from_deg + 180.0;
    let (wx, wy) = bearing_to_plot(travel_deg, 1.0);
    let tip = Pos2::new(
        centre.x + max_radius * 1.08 * wx as f32,
        centre.y - max_radius * 1.08 * wy as f32,
    );
    painter.line_segment([centre, tip], Stroke::new(2.0, Color32::from_rgb(120, 190, 255)));
    painter.text(
        tip,
        Align2::CENTER_CENTER,
        "plume",
        FontId::proportional(9.0),
        Color32::from_rgb(160, 210, 255),
    );

    painter.text(
        Pos2::new(centre.x, rect.top() + 8.0),
        Align2::CENTER_CENTER,
        "N",
        FontId::proportional(10.0),
        Color32::from_gray(170),
    );
    painter.text(
        Pos2::new(rect.left() + 4.0, rect.bottom() - 4.0),
        Align2::LEFT_BOTTOM,
        format!("outer ring {outermost:.0} m"),
        FontId::proportional(9.0),
        Color32::from_gray(150),
    );
}

/// Unit-circle position for a compass bearing, `(x east, y north)`.
///
/// Same convention as
/// [`crate::physics::atmospheric_dispersion::bearing_to_site_frame`]: sine on
/// east, cosine on north, so bearing 0 is north.
fn bearing_to_plot(bearing_deg: f64, radius: f64) -> (f64, f64) {
    let radians = bearing_deg.to_radians();
    (radius * radians.sin(), radius * radians.cos())
}

/// Shade a receptor by `chi/Q` on a **logarithmic** scale spanning four
/// decades below the peak.
///
/// Linear shading would be actively misleading here: `chi/Q` runs from ~1e-5
/// on the plume centreline to ~1e-28 upwind, so on a linear scale every
/// receptor but one or two renders identically black and the map would suggest
/// the plume is far narrower than the model says. Four decades is the span
/// over which the difference is worth seeing; below that the values are
/// negligible and are drawn at the floor colour.
fn log_shade(value: f64, peak: f64) -> Color32 {
    const DECADES: f64 = 4.0;
    if !(value > 0.0) || !(peak > 0.0) {
        return Color32::from_gray(45);
    }
    let decades_below = (peak / value).log10();
    let fraction = (1.0 - decades_below / DECADES).clamp(0.0, 1.0);
    hot_to_cold_colour_mark_1(fraction as f32)
}

/// The dispersion table and its caveats.
fn draw_dispersion_table(ui: &mut Ui, s: &HtgrSnapshot) {
    ui.label(format!(
        "Gaussian puff (changi::puff, ported from R `puff` 0.1.1) -- NOT FLEXPART.  \
         Wind {:.1} m/s FROM {:.0} deg, Pasquill class {}.  Last run at t = {}",
        s.wind_speed_m_per_s,
        s.wind_from_deg,
        if s.stability_class.is_empty() { "--" } else { s.stability_class },
        if s.dispersion_evaluated_at_s.is_finite() {
            format!("{:.0} s", s.dispersion_evaluated_at_s)
        } else {
            "--".to_string()
        }
    ));
    ui.label(
        "chi/Q is the quotable column: it is a dilution factor and does NOT depend on the \
         source, so no inventory or leak-rate assumption enters it. The two activity columns \
         DO -- they are per curie of core inventory and per unit of a placeholder leak \
         fraction, i.e. a transfer function, not a consequence, and not figures for any \
         reactor. Research, education and V&V only; no dose quantity is computed.",
    );
    ui.add_space(4.0);

    egui::Grid::new("htgr_map_dispersion_grid")
        .num_columns(5)
        .striped(true)
        .show(ui, |ui| {
            for heading in [
                "Bearing",
                "Distance",
                "chi/Q [s/m^3]",
                "Air [Bq.s/m^3 per Ci]",
                "Ground [Bq/m^2 per Ci]",
            ] {
                ui.label(heading);
            }
            ui.end_row();

            // Only the downwind half is tabulated: 24 rows is a wall, and the
            // upwind receptors are 20+ orders below the centreline and carry
            // no information a reader acts on. The ROSE shows all 24, so
            // nothing is hidden -- this is a table-length choice, not a
            // filter on what was computed.
            let travel_deg = (s.wind_from_deg + 180.0).rem_euclid(360.0);
            let mut rows: Vec<&super::state::ReceptorSnapshot> = s
                .receptors
                .iter()
                .filter(|r| {
                    r.distance_m > 0.0 && angular_distance(r.bearing_deg, travel_deg) <= 90.0
                })
                .collect();
            rows.sort_by(|a, b| {
                a.distance_m
                    .total_cmp(&b.distance_m)
                    .then(a.bearing_deg.total_cmp(&b.bearing_deg))
            });

            for r in rows {
                ui.label(format!("{:.0} deg", r.bearing_deg));
                ui.label(format!("{:.0} m", r.distance_m));
                ui.label(format!("{:.4e}", r.chi_over_q));
                ui.label(format!("{:.3e}", r.air_bq_s_per_m3));
                ui.label(format!("{:.3e}", r.ground_bq_per_m2));
                ui.end_row();
            }
        });
    ui.add_space(4.0);
    ui.label(
        "Downwind half shown; the rose above plots all 24 receptors. Ground deposition is DRY \
         only -- wet scavenging is not ported, so it is not an upper bound.",
    );
}

/// Smallest angle between two compass bearings, degrees.
fn angular_distance(a_deg: f64, b_deg: f64) -> f64 {
    let diff = (a_deg - b_deg).rem_euclid(360.0);
    diff.min(360.0 - diff)
}

/// Draw the whole Map tab: the Gaussian puff dispersion widgets.
pub fn draw_map(ui: &mut Ui, s: &HtgrSnapshot) {
    ui.heading("Atmospheric dispersion -- Gaussian puff");
    ui.columns(2, |columns| {
        draw_dispersion_rose(&mut columns[0], s);
        draw_dispersion_table(&mut columns[1], s);
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The rose's plot convention must match the physics module's site frame,
    /// and the plume arrow must point where the plume TRAVELS.
    ///
    /// Two separate sign traps in one picture. First, bearing-to-plot must put
    /// north at `+y` (sine on east, cosine on north) — swapping them mirrors
    /// the rose about the NE diagonal, which still looks like a plume. Second,
    /// the arrow is drawn at `wind_from + 180` because meteorological wind
    /// direction names where the wind comes *from*; drawing it at
    /// `wind_from` puts the plume arrow pointing back up the plume, which is
    /// the single most common error in a dispersion display.
    #[test]
    fn the_rose_convention_matches_the_physics_and_the_arrow_points_downwind() {
        use crate::physics::atmospheric_dispersion::bearing_to_site_frame;

        for bearing in [0.0, 45.0, 90.0, 180.0, 270.0, 315.0] {
            let (px, py) = bearing_to_plot(bearing, 1.0);
            let (sx, sy) = bearing_to_site_frame(bearing, 1.0);
            assert!(
                (px - sx).abs() < 1e-12 && (py - sy).abs() < 1e-12,
                "bearing {bearing}: rose ({px:.6}, {py:.6}) must match the site frame \
                 ({sx:.6}, {sy:.6})"
            );
        }

        // North at bearing 0 is +y.
        let (x, y) = bearing_to_plot(0.0, 1.0);
        assert!(x.abs() < 1e-12 && (y - 1.0).abs() < 1e-12);
        // East at bearing 90 is +x.
        let (x, y) = bearing_to_plot(90.0, 1.0);
        assert!((x - 1.0).abs() < 1e-12 && y.abs() < 1e-12);

        // A wind FROM the north (0 deg) must draw the plume arrow SOUTH.
        let travel = 0.0 + 180.0;
        let (ax, ay) = bearing_to_plot(travel, 1.0);
        assert!(
            ay < -0.9 && ax.abs() < 1e-9,
            "a north wind must point the plume arrow south; got ({ax:.3}, {ay:.3})"
        );
    }

    /// The log shading must span four decades, clamp outside them, and never
    /// render a zero or a negative as anything but the floor.
    ///
    /// A linear scale here would make the plume look far narrower than the
    /// model says — chi/Q runs from ~1e-5 on the centreline to ~1e-28 upwind —
    /// so the log scale is load-bearing for the picture being honest, and its
    /// endpoints are pinned.
    #[test]
    fn the_log_shading_spans_four_decades_and_clamps() {
        let peak = 1.0e-5;
        let at_peak = log_shade(peak, peak);
        let one_decade = log_shade(peak / 10.0, peak);
        let four_decades = log_shade(peak / 1.0e4, peak);
        let far_below = log_shade(peak / 1.0e20, peak);

        assert_ne!(at_peak, one_decade, "one decade must be visibly different");
        assert_eq!(
            four_decades, far_below,
            "beyond four decades the scale must clamp, not keep darkening"
        );
        // Degenerate inputs must not panic or produce a bright receptor.
        let floor = Color32::from_gray(45);
        assert_eq!(log_shade(0.0, peak), floor);
        assert_eq!(log_shade(-1.0, peak), floor);
        assert_eq!(log_shade(peak, 0.0), floor);
    }

    /// The downwind filter must select the half-plane the plume is actually
    /// in, and must wrap correctly around north.
    ///
    /// The wrap is the interesting case: with a wind from 90 deg the plume
    /// travels to 270 deg, and the downwind sectors are 180-360 — but with a
    /// wind from 270 the plume travels to 90 and the sectors straddle 0, where
    /// a naive difference would exclude exactly the sectors it should keep.
    #[test]
    fn the_downwind_filter_wraps_around_north() {
        // Plume travelling due north (0 deg): 315 and 45 are both within 90,
        // and they straddle the wrap.
        assert!(angular_distance(315.0, 0.0) <= 90.0);
        assert!(angular_distance(45.0, 0.0) <= 90.0);
        assert!(angular_distance(180.0, 0.0) > 90.0);

        // Symmetric, and never greater than 180.
        for (a, b) in [(10.0, 350.0), (0.0, 180.0), (90.0, 270.0), (359.0, 1.0)] {
            let ab = angular_distance(a, b);
            assert!((ab - angular_distance(b, a)).abs() < 1e-12, "must be symmetric");
            assert!((0.0..=180.0).contains(&ab), "got {ab} for ({a}, {b})");
        }
        assert!((angular_distance(359.0, 1.0) - 2.0).abs() < 1e-12);
    }
}
