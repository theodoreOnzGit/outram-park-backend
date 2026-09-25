//! **Map** -- Gaussian puff atmospheric dispersion around the plant.
//!
//! Since 2026-09-22 (maintainer direction) this tab holds only the dispersion
//! widgets: the live core map, the pebble and TRISO particle drill-down and
//! the release table moved to [`super::thermal_tab`] ("Live thermal state and
//! FP release"), which replaced the static geometry viewer.
//!
//! # The picture is a live plume, evaluated once per screen pixel
//!
//! Two maintainer directions, both 2026-09-25, shape what this draws:
//!
//! - *"fill the map with single pixels, not the big boxes"* -- the field is
//!   requested at the map square's own width in **physical screen pixels**,
//!   so every pixel painted is a separate evaluation of the puff model at that
//!   pixel's coordinates. It is uploaded as one texture rather than as tens of
//!   thousands of filled rectangles, which is a rendering choice and changes
//!   no value.
//! - *"timestep according to real-time, I want to see a real-time plume"* --
//!   the field is the **instantaneous** concentration at the plume clock, so
//!   it grows out of the stack, travels downwind, and settles after one puff
//!   lifetime. See [`crate::physics::atmospheric_dispersion::DispersionGrid`],
//!   which spells out why that is a *different quantity* from the
//!   time-integrated `chi/Q` the table below quotes, despite sharing units.
//!
//! The fast-forward buttons move the **plume** clock only. The plant clock is
//! shown beside it and is never advanced by them -- see
//! [`crate::physics::atmospheric_dispersion::MapFieldRequest::plume_clock_offset`]
//! for why the plume may legitimately run ahead and what that must never be
//! read as.
//!
//! # NOT VALIDATED
//!
//! The puff model is `changi::puff` (ported from R `puff` 0.1.1), not FLEXPART,
//! and it is driven by the release the thermal tab shows, which is itself
//! **per curie of core inventory**. See
//! [`crate::physics::atmospheric_dispersion`].

use egui::{
    Align2, Color32, ColorImage, FontId, Pos2, Rect, Sense, Stroke, TextureHandle, TextureOptions,
    Ui, Vec2,
};

use outram_park_digital_twin_engine::app_scaffold::SharedState;
use outram_park_digital_twin_engine::color_maps::hot_to_cold_colour_mark_1;

use super::state::HtgrSnapshot;

/// Fraction of the tab's height the map square takes.
///
/// **Maintainer direction, 2026-09-25: "make the map fill like 60% of the
/// height"**, with the dispersion table below it rather than beside it. A
/// drawing-layout constant with no physical counterpart, recorded here as
/// their call so the next reader does not "fix" it back to a square that
/// fits whatever is left.
const MAP_HEIGHT_FRACTION: f32 = 0.60;

/// Floor on the map square's side \[points\].
///
/// A window short enough that 60 % of it is smaller than this gets a map that
/// overflows into the tab's two-way scroll area instead of collapsing to an
/// unreadable thumbnail. Purely a drawing choice.
const MAP_MIN_SIDE: f32 = 320.0;

/// Steps the plume-clock fast-forward offers \[s of plume clock\].
///
/// **Maintainer direction, 2026-09-25: "timesteps of 1-2 hrs at a time".**
/// One and two hours, plus a one-hour rewind so a jump can be walked back
/// without restarting.
const PLUME_JUMPS_S: [(f64, &str); 3] = [
    (3600.0, "+1 h"),
    (7200.0, "+2 h"),
    (-3600.0, "-1 h"),
];

/// The Map tab's retained state.
///
/// Only the field texture and the key it was built for. Everything else the
/// tab draws is rebuilt per frame from the snapshot, which is the crate's
/// usual arrangement; a texture is the exception because it is a GPU upload,
/// and re-uploading a 512 x 512 image every frame would cost more than
/// computing the field does.
#[derive(Default)]
pub struct MapTabState {
    /// The uploaded `chi/Q` field, one texel per grid cell.
    texture: Option<TextureHandle>,
    /// `(cells, plume clock)` the texture was built from. The field changes
    /// only when one of these does -- see `DispersionGrid` -- so this is the
    /// complete upload key and not an approximation of one.
    built_for: Option<(usize, f64)>,
}

impl MapTabState {
    /// The texture for this snapshot's field, re-uploading only when the field
    /// has actually changed.
    ///
    /// Returns `None` when there is no field yet, which is the state before
    /// the first dispersion evaluation. Nothing is substituted in that case:
    /// an invented plume would look exactly like a computed one.
    fn field_texture(&mut self, ui: &Ui, s: &HtgrSnapshot) -> Option<&TextureHandle> {
        let cells = s.dispersion_grid_cells;
        if cells == 0 || s.dispersion_grid.len() < cells * cells {
            return None;
        }
        let key = (cells, s.dispersion_grid_time_s);
        if self.built_for != Some(key) || self.texture.is_none() {
            let peak = s
                .dispersion_grid
                .iter()
                .copied()
                .fold(0.0_f32, f32::max) as f64;
            let pixels: Vec<Color32> = s.dispersion_grid[..cells * cells]
                .iter()
                .map(|value| log_shade(*value as f64, peak))
                .collect();
            let image = ColorImage::new([cells, cells], pixels);
            match &mut self.texture {
                // NEAREST, not LINEAR: at one texel per screen pixel there is
                // nothing to interpolate, and filtering would blur evaluated
                // values into each other -- which is exactly the "picture of a
                // plume rather than a readout of one" the rose's docs reject.
                Some(handle) => handle.set(image, TextureOptions::NEAREST),
                None => {
                    self.texture = Some(ui.ctx().load_texture(
                        "htgr_dispersion_field",
                        image,
                        TextureOptions::NEAREST,
                    ))
                }
            }
            self.built_for = Some(key);
        }
        self.texture.as_ref()
    }
}

/// Draw the dispersion rose: the evaluated `chi/Q` field, with the receptor
/// ring, distance rings and sector spokes over it.
///
/// # Why a rose and not a contour plot
///
/// A contour plot of a Gaussian plume looks authoritative and would be, here,
/// an interpolation between 24 evaluated points -- a picture of a plume rather
/// than a readout of one, which this crate's rule forbids. The rose draws
/// exactly the points that were computed and nothing between them. It also
/// makes the sector structure visible, so nobody mistakes the resolution for
/// finer than it is.
///
/// The field underneath it is subject to the same rule and satisfies it the
/// other way: at one cell per screen pixel there is no gap left to
/// interpolate across, so nothing on it is drawn that was not evaluated.
///
/// Returns the side of the map square actually painted \[points\], which is
/// what the caller turns into the next frame's resolution request.
fn draw_dispersion_rose(ui: &mut Ui, s: &HtgrSnapshot, state: &mut MapTabState, side: f32) -> f32 {
    let (response, painter) = ui.allocate_painter(Vec2::new(side, side), Sense::hover());
    let rect = response.rect;
    let centre = rect.center();
    let size = rect.width().min(rect.height());
    let max_radius = size * 0.40;

    // White ground, not the dark canvas this used to have (maintainer,
    // 2026-09-24). A map is read against paper, and a dark field makes the
    // low-concentration sectors -- most of the plot -- the hardest part to
    // see, which is backwards: the quiet sectors are the reassuring result.
    painter.rect_filled(rect, 2.0, Color32::WHITE);
    painter.rect_stroke(
        rect,
        2.0,
        Stroke::new(1.0, Color32::from_gray(180)),
        egui::StrokeKind::Inside,
    );

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
            Color32::from_gray(120),
        );
        return rect.width();
    }

    // Distance rings, drawn at the real radii so the plot is to scale.
    let mut drawn: Vec<f64> = Vec::new();
    for receptor in &s.receptors {
        if receptor.distance_m > 0.0 && !drawn.iter().any(|d| (d - receptor.distance_m).abs() < 1e-9)
        {
            drawn.push(receptor.distance_m);
        }
    }

    // --- the evaluated field, one texel per grid cell ---
    //
    // Painted FIRST so the rings, spokes and receptor markers sit on top of
    // it. Every cell is a real evaluation of the puff model at that cell's
    // coordinates, so this is a readout at every pixel of the square rather
    // than an interpolation between 24 -- which is what the rose's own docs
    // rule out.
    let field_peak = s
        .dispersion_grid
        .iter()
        .copied()
        .fold(0.0_f32, f32::max) as f64;
    let grid_px = max_radius * (s.dispersion_grid_half_width_m / outermost) as f32;
    if let Some(texture) = state.field_texture(ui, s) {
        let field_rect = Rect::from_center_size(centre, Vec2::splat(2.0 * grid_px));
        painter.image(
            texture.id(),
            field_rect,
            Rect::from_min_max(Pos2::ZERO, Pos2::new(1.0, 1.0)),
            Color32::WHITE,
        );
    }

    drawn.sort_by(|a, b| a.partial_cmp(b).expect("finite distances"));
    for distance in &drawn {
        let r = max_radius * (*distance / outermost) as f32;
        painter.circle_stroke(centre, r, Stroke::new(1.0, Color32::from_black_alpha(90)));
        // Label each ring on its own circle. The rings ARE the distance
        // scale, so naming them is what turns the plot from a decoration
        // into something a reader can take a number off.
        painter.text(
            Pos2::new(centre.x + r, centre.y - 5.0),
            Align2::LEFT_BOTTOM,
            format!("{distance:.0} m"),
            FontId::proportional(9.0),
            Color32::from_gray(110),
        );
    }
    // Sector spokes, so the 45-degree resolution is visible rather than
    // implied by the marker spacing.
    for sector in 0..8 {
        let (sx, sy) = bearing_to_plot(45.0 * sector as f64, 1.0);
        painter.line_segment(
            [
                centre,
                Pos2::new(
                    centre.x + max_radius * sx as f32,
                    centre.y - max_radius * sy as f32,
                ),
            ],
            Stroke::new(0.5, Color32::from_black_alpha(40)),
        );
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
        painter.circle_stroke(at, 6.0, Stroke::new(0.8, Color32::from_gray(60)));
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
    painter.line_segment([centre, tip], Stroke::new(2.5, Color32::from_rgb(20, 90, 190)));
    painter.text(
        tip,
        Align2::CENTER_CENTER,
        "plume",
        FontId::proportional(9.0),
        Color32::from_rgb(20, 90, 190),
    );

    painter.text(
        Pos2::new(centre.x, rect.top() + 8.0),
        Align2::CENTER_CENTER,
        "N",
        FontId::proportional(11.0),
        Color32::from_gray(60),
    );
    painter.text(
        Pos2::new(rect.left() + 4.0, rect.bottom() - 4.0),
        Align2::LEFT_BOTTOM,
        format!("outer ring {outermost:.0} m"),
        FontId::proportional(9.0),
        Color32::from_gray(110),
    );
    // The colour scale, in the one place it cannot be mistaken for anything
    // else: what the brightest pixel on screen is worth, and how far down the
    // ramp runs. Without it the picture is a shape with no magnitude, and the
    // magnitude is the only thing anyone should quote off it.
    if field_peak > 0.0 {
        painter.text(
            Pos2::new(rect.right() - 4.0, rect.bottom() - 4.0),
            Align2::RIGHT_BOTTOM,
            format!(
                "field peak {field_peak:.3e} s/m^3, 4 decades below it to the floor \
                 ({} x {} cells)",
                s.dispersion_grid_cells, s.dispersion_grid_cells
            ),
            FontId::proportional(9.0),
            Color32::from_gray(110),
        );
    }

    rect.width()
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

/// Shade a receptor or a field cell by `chi/Q` on a **logarithmic** scale
/// spanning four decades below the peak.
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
        // Light, not dark: on a white ground the "no value" marker must
        // recede. The old near-black was correct for the dark canvas and
        // wrong the moment the background changed.
        return Color32::from_gray(225);
    }
    let decades_below = (peak / value).log10();
    let fraction = (1.0 - decades_below / DECADES).clamp(0.0, 1.0);
    hot_to_cold_colour_mark_1(fraction as f32)
}

/// Smallest angle between two compass bearings, degrees.
fn angular_distance(a_deg: f64, b_deg: f64) -> f64 {
    let diff = (a_deg - b_deg).rem_euclid(360.0);
    diff.min(360.0 - diff)
}

/// Format a duration in seconds as `h:mm:ss`, for the two clocks.
fn clock_text(seconds: f64) -> String {
    if !seconds.is_finite() {
        return "--".to_string();
    }
    let total = seconds.max(0.0).round() as u64;
    format!(
        "{}:{:02}:{:02}",
        total / 3600,
        (total % 3600) / 60,
        total % 60
    )
}

/// The plume clock readout and its fast-forward buttons.
///
/// # What these buttons move, and what they deliberately do not
///
/// They move the **plume** clock. The plant clock beside them is untouched,
/// and both are on screen at once so the difference cannot be missed.
///
/// That split is not a convenience: the plant model cannot skip time (its
/// timestep is pinned by an advective Courant limit -- see
/// [`crate::physics::PLANT_TIMESTEP_S`]) and it computes at roughly real time,
/// so a genuine one-hour plant jump costs an hour. The dispersion field has no
/// such constraint, because `chi/Q` is a dilution factor that does not depend
/// on the source at all: jumping its clock is the same closed form evaluated
/// at a later argument, exact rather than extrapolated
/// (`atmospheric_dispersion::tests::a_plume_clock_jump_equals_having_run_the_clock_there`
/// pins that). Maintainer direction, 2026-09-25, chose this split knowing it.
fn draw_plume_clock(ui: &mut Ui, physics: &SharedState<HtgrSnapshot>, s: &HtgrSnapshot) {
    ui.horizontal_wrapped(|ui| {
        ui.label(format!("Plant clock {}", clock_text(s.sim_time_s)));
        ui.separator();
        ui.label(format!(
            "Plume clock {}",
            clock_text(s.dispersion_grid_time_s)
        ));
        ui.separator();
        ui.label("Fast forward the plume:");
        for (jump_s, label) in PLUME_JUMPS_S {
            if ui
                .button(label)
                .on_hover_text(
                    "Moves the PLUME clock only. The reactor, the release channel and the \
                     table below stay on the plant clock -- those depend on the source and \
                     the plant cannot skip time. chi/Q does not depend on the source, so the \
                     field at a later clock is exact, not extrapolated.",
                )
                .clicked()
            {
                let offset = (s.plume_clock_offset_s + jump_s).max(-s.sim_time_s);
                physics.update(|state| state.plume_clock_offset_s = offset);
            }
        }
        if ui.button("Now").clicked() {
            physics.update(|state| state.plume_clock_offset_s = 0.0);
        }
    });
    if s.plume_clock_offset_s.abs() > f64::EPSILON {
        ui.colored_label(
            Color32::from_rgb(200, 120, 20),
            format!(
                "Plume clock is running {} AHEAD of the plant. The map is the plume this \
                 wind would have produced by then; the plant state, the release and the \
                 table below are still at the plant clock. The plume settles after one puff \
                 lifetime (20 min), so past that a further jump changes nothing unless the \
                 wind does.",
                clock_text(s.plume_clock_offset_s.abs())
            ),
        );
    }
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
    ui.label(
        "These rows are the TIME-INTEGRATED chi/Q at each receptor, over the whole puff run. \
         The map above is the INSTANTANEOUS field at the plume clock. Both are s/m^3 and \
         they are different quantities -- do not read a pixel against a row.",
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

/// Cells per side to ask the dispersion field for, given the map square's side
/// in points and the display's points-to-pixels ratio.
///
/// **One evaluation per physical screen pixel** -- the whole point of the
/// 2026-09-25 direction. On a HiDPI display a point is more than a pixel, so
/// asking in points would still paint every cell across two or more pixels,
/// which is the "big box" being replaced, only smaller.
///
/// The physics clamps this to what the host can afford
/// ([`crate::physics::atmospheric_dispersion::max_grid_cells`]), so an
/// oversized request costs a coarser map, never a missed physics tick.
fn requested_cells(side_points: f32, pixels_per_point: f32) -> usize {
    (side_points * pixels_per_point).round().max(1.0) as usize
}

/// Whether a new resolution request is worth sending.
///
/// A window being dragged changes the map's width by a pixel at a time, and
/// every changed request invalidates the field cache and forces a fresh
/// evaluation. A 2 % dead band means a resize settles instead of recomputing
/// the field on every intermediate width, while any real size change still
/// gets through. Purely a rate-limiting choice; it cannot change a value.
fn resolution_request_changed(current: usize, wanted: usize) -> bool {
    let tolerance = (current as f64 * 0.02).max(2.0);
    (current as f64 - wanted as f64).abs() > tolerance
}

/// Draw the whole Map tab: the Gaussian puff dispersion widgets.
///
/// `view` is the tab's viewport, measured by the caller **outside** the scroll
/// area -- inside one, the available height is the content's own budget rather
/// than the window's, so a panel that sized itself from in there would grow
/// every frame it filled.
///
/// Layout is the map square above the table, not beside it (maintainer,
/// 2026-09-25), with the map at [`MAP_HEIGHT_FRACTION`] of the viewport
/// height. The combination overflows the viewport by construction, which is
/// what the caller's two-way scroll area is for.
pub fn draw_map(
    ui: &mut Ui,
    physics: &SharedState<HtgrSnapshot>,
    s: &HtgrSnapshot,
    state: &mut MapTabState,
    view: Vec2,
) {
    ui.heading("Atmospheric dispersion -- Gaussian puff");
    draw_plume_clock(ui, physics, s);
    ui.add_space(4.0);

    let side = (view.y * MAP_HEIGHT_FRACTION).max(MAP_MIN_SIDE);
    let painted = draw_dispersion_rose(ui, s, state, side);

    // Tell the physics what resolution this map can show. A control input,
    // written the same way the wind is -- see `HtgrSnapshot::
    // map_field_cells_requested`.
    let wanted = requested_cells(painted, ui.ctx().pixels_per_point());
    if resolution_request_changed(s.map_field_cells_requested, wanted) {
        physics.update(|state| state.map_field_cells_requested = wanted);
    }

    ui.add_space(8.0);
    ui.separator();
    draw_dispersion_table(ui, s);
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

    /// The resolution request must be in PHYSICAL PIXELS, and must not
    /// re-fire on every pixel of a window drag.
    ///
    /// Both halves have teeth. Asking in points on a 2x display would paint
    /// every evaluated cell across four pixels -- the "big box" the
    /// single-pixel direction replaces, only smaller and harder to notice.
    /// And a request that changed on every intermediate width would
    /// invalidate the field cache on every frame of a resize, turning a drag
    /// into a sustained recomputation.
    #[test]
    fn the_resolution_request_is_in_pixels_and_has_a_dead_band() {
        assert_eq!(requested_cells(320.0, 1.0), 320);
        assert_eq!(requested_cells(320.0, 2.0), 640, "HiDPI must ask for real pixels");
        assert_eq!(requested_cells(0.0, 1.0), 1, "a degenerate size must not be zero");

        // A one-pixel wobble on a 500-cell map is inside the dead band; a
        // real resize is not.
        assert!(!resolution_request_changed(500, 501));
        assert!(!resolution_request_changed(500, 495));
        assert!(resolution_request_changed(500, 560));
        // The dead band has a floor, so a small map still responds.
        assert!(resolution_request_changed(64, 80));
    }

    /// The clock readout must be `h:mm:ss` and must not panic on the `NAN`
    /// the snapshot carries before the first field.
    #[test]
    fn the_clock_reads_out_in_hours_minutes_seconds() {
        assert_eq!(clock_text(0.0), "0:00:00");
        assert_eq!(clock_text(59.4), "0:00:59");
        assert_eq!(clock_text(3600.0), "1:00:00");
        assert_eq!(clock_text(7265.0), "2:01:05");
        assert_eq!(clock_text(f64::NAN), "--");
        assert_eq!(clock_text(-5.0), "0:00:00");
    }
}
