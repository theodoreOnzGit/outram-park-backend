//! Visual FHR (fluoride-salt-cooled high-temperature reactor) vessel.
//!
//! A pebble-bed FHR vessel drawn as cut-away art: the pebble bed, the coolant
//! passing through it, the inlet and outlet plena, two downcomers, and two
//! control rods at their commanded insertion depth. Every region is filled
//! from a temperature the caller supplies, so the vessel reads as a
//! temperature field rather than as a static picture with numbers beside it.
//!
//! Unlike [`crate::components::reactor_vessel::ReactorVesselVisual`] — which
//! wraps a `nee_soon` kinetics model and colours a single rectangle by lumped
//! fuel temperature — this widget takes **fourteen independent temperatures**
//! and owns no physics. It is deliberately scalar-fed, for the same reason
//! [`crate::components::pipe::PipeVisual::from_scalars`] is: an FHR simulator
//! already holds these temperatures in its own plant model, and requiring a
//! particular physics type here would force every caller to adopt it.
//!
//! Scalar-fed does **not** mean placeholder. Callers pass real state from
//! their own model; do not fabricate values to feed it.
//!
//! Migrated into the shared component library from `fhr_sim_v2`'s local widget
//! set (bead `op-wqk.8`, step 2), so other reactor simulators can draw a
//! pebble-bed vessel without re-deriving the art.
//!
//! # Why this bed is drawn UPSIDE DOWN — the pebbles float
//!
//! The pebbles here are placed from the same baked, gravity-settled DEM
//! packing the HTR-10 widget uses ([`crate::components::pebble_packing`]), but
//! **inverted**, and that inversion is physics rather than a drawing accident.
//!
//! An FHR's coolant is molten FLiBe at roughly 1940 kg/m³ at operating
//! temperature, while a graphite pebble is roughly 1740–1800 kg/m³. The
//! pebbles are therefore **less dense than the salt they sit in, and float**.
//! They rise, pack upward against a retaining structure at the **top** of the
//! core, and are injected low and removed high — the mirror image of HTR-10,
//! where helium is a gas, the pebbles settle downward under their own weight,
//! and the bed drains through a cone at the bottom.
//!
//! So the bed's dense, compressed base is drawn at the **top** of the core and
//! its loose free surface faces **down**. If a future edit makes the bottom of
//! this bed the dense end, it has silently turned an FHR into a gas-cooled
//! reactor. The inversion is expressed once, as
//! [`crate::components::htr10_reactor_vessel::VerticalSense::Buoyant`].

use egui::epaint::CubicBezierShape;
use egui::{epaint::PathShape, vec2, Color32, Pos2, Sense, Stroke, Vec2, Widget};
use uom::si::{f64::*, thermodynamic_temperature::degree_celsius};

use crate::color_maps::hot_to_cold_colour_mark_1;
use crate::components::htr10_reactor_vessel::{
    draw_packed_pebbles, PackingTransform, PackingWindow, VerticalSense,
};
use crate::components::pebble_bed_texture::BedTint;
use crate::components::pebble_packing::BARREL_HEIGHT;

/// Width-to-height ratio the artwork was authored against.
///
/// The drawing was laid out by hand at 225 x 1050 points in `fhr_sim_v2`, and
/// every internal coordinate is a fraction of the box it is given. Fractions
/// scale, but they do not preserve *proportion*: hand the widget a square box
/// and the vessel stretches, because the horizontal features expand while the
/// vertical ones do not shrink to match.
///
/// [`fit_native_aspect`] resolves that by fitting this ratio inside whatever
/// box the caller allocated, so the artwork stays correctly proportioned at
/// any size.
pub const NATIVE_ASPECT_RATIO: f32 = 225.0 / 1050.0;

/// The largest sub-rectangle of `available` carrying
/// [`NATIVE_ASPECT_RATIO`], centred within it.
///
/// This is a letterbox: the vessel keeps its proportions and the leftover space
/// is simply not drawn into, rather than the artwork being stretched to fill.
///
/// A caller that already sizes to the native ratio — as `fhr_sim_v2` does, with
/// its hardcoded 225 x 1050 box — gets the identical rectangle back, so this is
/// invisible there and only takes effect for callers using a different shape.
pub fn fit_native_aspect(available: egui::Rect) -> egui::Rect {
    let (w, h) = (available.width(), available.height());
    if w <= 0.0 || h <= 0.0 {
        return available;
    }

    // Height-limited if the box is wider than native, width-limited otherwise.
    let (fitted_w, fitted_h) = if w / h > NATIVE_ASPECT_RATIO {
        (h * NATIVE_ASPECT_RATIO, h)
    } else {
        (w, w / NATIVE_ASPECT_RATIO)
    };

    egui::Rect::from_center_size(available.center(), Vec2::new(fitted_w, fitted_h))
}

/// Screen box of the **"fat" core** — the drawn pebble bed — inside an
/// already-letterboxed vessel `rect`.
///
/// This is the straight-sided region between the inlet and outlet cones of the
/// cut-away: half the vessel width, and 0.45 of a vessel quarter-height either
/// side of centre. Both the coolant polygons and the pebble packing are built
/// from it, so the bed the pebbles are placed in is by construction the bed
/// that is drawn.
pub(crate) fn pebble_bed_rect(rect: egui::Rect) -> egui::Rect {
    let reactor_half_width_x = rect.width() * 0.5;
    let reactor_half_length_y = rect.height() * 0.25;
    egui::Rect::from_center_size(
        rect.center(),
        Vec2::new(reactor_half_width_x, 0.9 * reactor_half_length_y),
    )
}

/// How the baked pebble packing is laid into the FHR bed: **inverted**,
/// height-fitted, and cropped to a central column.
///
/// Returns the transform that places the packing and the window that selects
/// which of its circles are drawn.
///
/// Three decisions are encoded here, and all three are deliberate:
///
/// 1. **Inverted** ([`VerticalSense::Buoyant`]). FHR pebbles float in the
///    denser salt, so the packing's settled base is drawn against the top of
///    the core and its free surface faces down. See the module docs.
/// 2. **Height-fitted, one uniform scale.** The scale is
///    `bed height / BARREL_HEIGHT`, applied to both axes and to every pebble
///    radius. The packing is never stretched — a settled packing is only a
///    packing because its spheres touch.
/// 3. **Cropped, not tiled.** The fat core is proportionally about 1.9 times
///    taller than the packing's barrel, so at a height-fitting scale the
///    barrel is wider than the bed. A central column is taken. Nothing is
///    duplicated to fill the space.
///
/// A degenerate (zero- or negative-height) `bed` yields a zero scale and an
/// empty column, so nothing is drawn rather than a divide-by-zero escaping
/// into the painter.
pub(crate) fn buoyant_bed_packing(bed: egui::Rect) -> (PackingTransform, PackingWindow) {
    let scale = (bed.height() / BARREL_HEIGHT).max(0.0);
    // How much of the barrel's width fits the bed at that scale, in vessel
    // radii. Below 1.0 exactly because the bed is the proportionally taller
    // shape.
    let column_half_width = if scale > 0.0 {
        bed.width() * 0.5 / scale
    } else {
        0.0
    };

    (
        PackingTransform {
            axis_x: bed.center().x,
            // Packing y = 0 — the dense settled base — is drawn at the TOP of
            // the core, where a floating bed packs against its retainer.
            origin_y: bed.top(),
            scale,
            vertical: VerticalSense::Buoyant,
        },
        PackingWindow::barrel_column(column_half_width),
    )
}

/// Visual representation of a pebble-bed FHR reactor vessel.
///
/// Holds one temperature per drawn region plus the two control-rod insertion
/// fractions. All temperatures are absolute thermodynamic temperatures
/// (`uom`-typed, so the unit is carried by the type); the colour mapping works
/// in degrees Celsius internally but callers never need to know that.
///
/// Insertion fractions are dimensionless in `[0, 1]`: `0.0` is fully
/// withdrawn, `1.0` fully inserted. Values outside that range are clamped at
/// render time rather than rejected, so a controller that transiently
/// overshoots does not panic the GUI thread.
pub struct FhrReactorVesselVisual {
    size: Vec2,
    left_control_rod_insertion_frac: f32,
    right_control_rod_insertion_frac: f32,
    min_temp: ThermodynamicTemperature,
    max_temp: ThermodynamicTemperature,
    pebble_core_temp: ThermodynamicTemperature,
    core_mid_coolant_temp: ThermodynamicTemperature,
    core_bottom_temp: ThermodynamicTemperature,
    core_top_temp: ThermodynamicTemperature,
    core_inlet_temp: ThermodynamicTemperature,
    core_outlet_temp: ThermodynamicTemperature,
    left_downcomer_upper_temp: ThermodynamicTemperature,
    left_downcomer_mid_temp: ThermodynamicTemperature,
    left_downcomer_lower_temp: ThermodynamicTemperature,
    right_downcomer_upper_temp: ThermodynamicTemperature,
    right_downcomer_mid_temp: ThermodynamicTemperature,
    right_downcomer_lower_temp: ThermodynamicTemperature,
}

impl FhrReactorVesselVisual {
    /// Build a vessel visual from screen size, a colour-mapping temperature
    /// range, and the fourteen region temperatures.
    ///
    /// `min_temp`/`max_temp` bound the displayed colour scale. Pick them to
    /// span the operating range the simulator expects to reach, so a normal
    /// operating point does not sit pinned at either end of the scale.
    ///
    /// Both control rods start **fully inserted** (`1.0`); use
    /// [`Self::set_left_cr_frac`] and [`Self::set_right_cr_frac`] to drive
    /// them from the simulator's rod controller.
    pub fn new(
        size: Vec2,
        min_temp: ThermodynamicTemperature,
        max_temp: ThermodynamicTemperature,
        pebble_core_temp: ThermodynamicTemperature,
        pebble_bed_coolant_temp: ThermodynamicTemperature,
        core_bottom_temp: ThermodynamicTemperature,
        core_top_temp: ThermodynamicTemperature,
        core_inlet_temp: ThermodynamicTemperature,
        core_outlet_temp: ThermodynamicTemperature,
        left_downcomer_upper_temp: ThermodynamicTemperature,
        left_downcomer_mid_temp: ThermodynamicTemperature,
        left_downcomer_lower_temp: ThermodynamicTemperature,
        right_downcomer_upper_temp: ThermodynamicTemperature,
        right_downcomer_mid_temp: ThermodynamicTemperature,
        right_downcomer_lower_temp: ThermodynamicTemperature,
    ) -> Self {
        Self {
            size,
            left_control_rod_insertion_frac: 1.0,
            right_control_rod_insertion_frac: 1.0,
            min_temp,
            max_temp,
            pebble_core_temp,
            core_mid_coolant_temp: pebble_bed_coolant_temp,
            core_bottom_temp,
            core_top_temp,
            core_inlet_temp,
            core_outlet_temp,
            left_downcomer_upper_temp,
            left_downcomer_mid_temp,
            left_downcomer_lower_temp,
            right_downcomer_upper_temp,
            right_downcomer_mid_temp,
            right_downcomer_lower_temp,
        }
    }

    /// Where `temp` falls in the display range, as a dimensionless fraction.
    ///
    /// Returns `0.0` at `min_temp` and `1.0` at `max_temp`, linearly between.
    /// **Not clamped** — a temperature outside the display range returns a
    /// value outside `[0, 1]`, which the colour map then saturates.
    pub fn hotness(&self, temp: ThermodynamicTemperature) -> f32 {
        let button_temp_degc = temp.get::<degree_celsius>();
        let min_temp_degc = self.min_temp.get::<degree_celsius>();
        let max_temp_degc = self.max_temp.get::<degree_celsius>();

        let hotness: f64 = (button_temp_degc - min_temp_degc) / (max_temp_degc - min_temp_degc);

        return hotness as f32;
    }

    /// Sets the temperature mapped to the coldest displayable colour.
    pub fn set_min_temp(&mut self, min_temp: ThermodynamicTemperature) {
        self.min_temp = min_temp;
    }

    /// Sets the temperature mapped to the hottest displayable colour.
    pub fn set_max_temp(&mut self, max_temp: ThermodynamicTemperature) {
        self.max_temp = max_temp;
    }

    /// On-screen size of the widget, in points.
    pub fn size(&self) -> Vec2 {
        self.size
    }

    /// Sets how far the left control rod is inserted.
    ///
    /// Dimensionless: `0.0` fully withdrawn, `1.0` fully inserted. Values
    /// outside `[0, 1]` are stored as given and clamped at render time.
    pub fn set_left_cr_frac(&mut self, left_control_rod_insertion_frac: f32) {
        self.left_control_rod_insertion_frac = left_control_rod_insertion_frac;
    }

    /// Sets how far the right control rod is inserted.
    ///
    /// Dimensionless: `0.0` fully withdrawn, `1.0` fully inserted. Values
    /// outside `[0, 1]` are stored as given and clamped at render time.
    pub fn set_right_cr_frac(&mut self, right_control_rod_insertion_frac: f32) {
        self.right_control_rod_insertion_frac = right_control_rod_insertion_frac;
    }
}

impl Widget for FhrReactorVesselVisual {
    /// Renders the vessel cut-away: pebble bed, coolant regions, inlet and
    /// outlet plena, both downcomers, and the two control rods at their
    /// commanded insertion depth.
    ///
    /// Each region is filled via
    /// [`crate::color_maps::hot_to_cold_colour_mark_1`] against the
    /// `min_temp`/`max_temp` range. Insertion fractions are clamped to
    /// `[0, 1]` here rather than at the setter, so a transient overshoot from
    /// a controller renders as fully in or fully out instead of panicking.
    fn ui(mut self, ui: &mut egui::Ui) -> egui::Response {
        let size = self.size();
        let (response, painter) = ui.allocate_painter(size, Sense::hover());

        if self.left_control_rod_insertion_frac > 1.0 {
            self.left_control_rod_insertion_frac = 1.0;
        } else if self.left_control_rod_insertion_frac < 0.0 {
            self.left_control_rod_insertion_frac = 0.0;
        };
        if self.right_control_rod_insertion_frac > 1.0 {
            self.right_control_rod_insertion_frac = 1.0;
        } else if self.right_control_rod_insertion_frac < 0.0 {
            self.right_control_rod_insertion_frac = 0.0;
        };

        // Fit the artwork to its native proportions inside whatever box the
        // caller allocated. Every coordinate below is derived from `rect`, so
        // doing this once here makes the whole drawing scale correctly at any
        // size — see `fit_native_aspect`.
        let rect = fit_native_aspect(response.rect);
        let c = rect.center();

        let rect_x = rect.width();
        let rect_y = rect.height();

        let reactor_half_width_x = rect_x * 0.5;
        let reactor_half_length_y = rect_y * 0.25;
        // bottom inlet
        let fhr_coolant_inlet_bottom_left =
            c + vec2(-0.10 * reactor_half_width_x, reactor_half_length_y * 0.9);
        let fhr_coolant_inlet_bottom_right =
            c + vec2(0.10 * reactor_half_width_x, reactor_half_length_y * 0.9);

        // core part
        let fhr_core_inlet_bottom_left =
            c + vec2(-0.10 * reactor_half_width_x, reactor_half_length_y * 0.65);
        let fhr_core_inlet_bottom_right =
            c + vec2(0.10 * reactor_half_width_x, reactor_half_length_y * 0.65);
        // The fat core IS the pebble bed, so its corners are read off the one
        // rect the packing is also laid into — see `pebble_bed_rect`.
        let bed_rect = pebble_bed_rect(rect);
        let fhr_core_fat_bottom_left = Pos2::new(bed_rect.left(), bed_rect.bottom());
        let fhr_core_fat_bottom_right = Pos2::new(bed_rect.right(), bed_rect.bottom());
        let fhr_core_fat_top_left = Pos2::new(bed_rect.left(), bed_rect.top());
        let fhr_core_fat_top_right = Pos2::new(bed_rect.right(), bed_rect.top());
        let fhr_core_outlet_top_left =
            c + vec2(-0.10 * reactor_half_width_x, -reactor_half_length_y * 0.65);
        let fhr_core_outlet_top_right =
            c + vec2(0.10 * reactor_half_width_x, -reactor_half_length_y * 0.65);

        // top outlet
        let fhr_coolant_outlet_top_left =
            c + vec2(-0.10 * reactor_half_width_x, -reactor_half_length_y * 0.9);
        let fhr_coolant_outlet_top_right =
            c + vec2(0.10 * reactor_half_width_x, -reactor_half_length_y * 0.9);

        // colour fill
        let hotness: f32 = 0.1;
        let coolant_fill = hot_to_cold_colour_mark_1(hotness);

        // draw clockwise
        let core_bottom_points = vec![
            fhr_core_inlet_bottom_left,
            fhr_core_fat_bottom_left,
            fhr_core_fat_bottom_right,
            fhr_core_inlet_bottom_right,
        ];
        let core_bottom_inlet_points = vec![
            fhr_coolant_inlet_bottom_left,
            fhr_core_inlet_bottom_left,
            fhr_core_inlet_bottom_right,
            fhr_coolant_inlet_bottom_right,
        ];
        let core_mid_points = vec![
            fhr_core_fat_bottom_left,
            fhr_core_fat_top_left,
            fhr_core_fat_top_right,
            fhr_core_fat_bottom_right,
        ];
        let core_top_points = vec![
            fhr_core_fat_top_left,
            fhr_core_outlet_top_left,
            fhr_core_outlet_top_right,
            fhr_core_fat_top_right,
        ];
        let core_outlet_points = vec![
            //fhr_coolant_inlet_bottom_left,
            //fhr_core_inlet_bottom_left,
            //fhr_core_fat_bottom_left,
            //fhr_core_fat_top_left,
            fhr_core_outlet_top_left,
            fhr_coolant_outlet_top_left,
            fhr_coolant_outlet_top_right,
            fhr_core_outlet_top_right,
            //fhr_core_fat_top_right,
            //fhr_core_fat_bottom_right,
            //fhr_core_inlet_bottom_right,
            //fhr_coolant_inlet_bottom_right,
        ];

        // fhr metal container (grey colour)
        //
        // we will use a cubic Beizier curve

        let reactor_box_top_left = c + vec2(-reactor_half_width_x, -reactor_half_length_y);
        let reactor_box_bottom_left = c + vec2(-reactor_half_width_x, reactor_half_length_y);
        let reactor_box_top_right = c + vec2(reactor_half_width_x, -reactor_half_length_y);
        let reactor_box_bottom_right = c + vec2(reactor_half_width_x, reactor_half_length_y);

        let reactor_curved_edge_fraction = 0.55;

        let reactor_curved_edge_top_left = c + vec2(
            -reactor_half_width_x,
            -reactor_curved_edge_fraction * reactor_half_length_y,
        );
        let reactor_curved_edge_bottom_left = c + vec2(
            -reactor_half_width_x,
            reactor_curved_edge_fraction * reactor_half_length_y,
        );
        let reactor_curved_edge_top_right = c + vec2(
            reactor_half_width_x,
            -reactor_curved_edge_fraction * reactor_half_length_y,
        );
        let reactor_curved_edge_bottom_right = c + vec2(
            reactor_half_width_x,
            reactor_curved_edge_fraction * reactor_half_length_y,
        );

        let metal_fill = Color32::GRAY;

        let fhr_bottom_metal_pts = [
            reactor_curved_edge_bottom_left,
            reactor_box_bottom_left,
            reactor_box_bottom_right,
            reactor_curved_edge_bottom_right,
        ];
        let fhr_top_metal_pts = [
            reactor_curved_edge_top_left,
            reactor_box_top_left,
            reactor_box_top_right,
            reactor_curved_edge_top_right,
        ];

        let fhr_mid_metal_pts = [
            reactor_curved_edge_bottom_left,
            reactor_curved_edge_top_left,
            reactor_curved_edge_top_right,
            reactor_curved_edge_bottom_right,
        ];

        let color = Color32::from_gray(128);
        let stroke = Stroke::new(1.0, color);
        let fhr_bottom_metal_semicircle =
            CubicBezierShape::from_points_stroke(fhr_bottom_metal_pts, true, metal_fill, stroke);

        let fhr_top_metal_semicircle =
            CubicBezierShape::from_points_stroke(fhr_top_metal_pts, true, metal_fill, stroke);
        let fhr_mid_metal_rect =
            PathShape::convex_polygon(fhr_mid_metal_pts.into(), metal_fill, stroke);

        // inner graphite reflector

        let graphite_width_fraction = 0.8;

        let reflector_box_top_left = c + vec2(
            -reactor_half_width_x * graphite_width_fraction,
            -reactor_half_length_y * graphite_width_fraction,
        );
        let reflector_box_bottom_left = c + vec2(
            -reactor_half_width_x * graphite_width_fraction,
            reactor_half_length_y * graphite_width_fraction,
        );
        let reflector_box_top_right = c + vec2(
            reactor_half_width_x * graphite_width_fraction,
            -reactor_half_length_y * graphite_width_fraction,
        );
        let reflector_box_bottom_right = c + vec2(
            reactor_half_width_x * graphite_width_fraction,
            reactor_half_length_y * graphite_width_fraction,
        );

        let reflector_curved_edge_fraction = 0.55;

        let reflector_curved_edge_top_left = c + vec2(
            -reactor_half_width_x * graphite_width_fraction,
            -reflector_curved_edge_fraction * reactor_half_length_y,
        );
        let reflector_curved_edge_bottom_left = c + vec2(
            -reactor_half_width_x * graphite_width_fraction,
            reflector_curved_edge_fraction * reactor_half_length_y,
        );
        let reflector_curved_edge_top_right = c + vec2(
            reactor_half_width_x * graphite_width_fraction,
            -reflector_curved_edge_fraction * reactor_half_length_y,
        );
        let reflector_curved_edge_bottom_right = c + vec2(
            reactor_half_width_x * graphite_width_fraction,
            reflector_curved_edge_fraction * reactor_half_length_y,
        );

        let reflector_bottom_graphite_pts = [
            reflector_curved_edge_bottom_left,
            reflector_box_bottom_left,
            reflector_box_bottom_right,
            reflector_curved_edge_bottom_right,
        ];
        let reflector_top_graphite_pts = [
            reflector_curved_edge_top_left,
            reflector_box_top_left,
            reflector_box_top_right,
            reflector_curved_edge_top_right,
        ];

        let reflector_mid_graphite_pts = [
            reflector_curved_edge_bottom_left,
            reflector_curved_edge_top_left,
            reflector_curved_edge_top_right,
            reflector_curved_edge_bottom_right,
        ];
        let graphite_fill = Color32::BLACK;

        let graphite_stroke = Stroke::new(1.0, graphite_fill);
        let reflector_bottom_graphite_semicircle = CubicBezierShape::from_points_stroke(
            reflector_bottom_graphite_pts,
            true,
            graphite_fill,
            graphite_stroke,
        );

        let reflector_top_graphite_semicircle = CubicBezierShape::from_points_stroke(
            reflector_top_graphite_pts,
            true,
            graphite_fill,
            graphite_stroke,
        );
        let reflector_mid_graphite_rect = PathShape::convex_polygon(
            reflector_mid_graphite_pts.into(),
            graphite_fill,
            graphite_stroke,
        );

        let coolant_stroke = Stroke::new(1.0, coolant_fill);
        // fhr coolant
        let core_bottom_hotness = self.hotness(self.core_bottom_temp);
        let core_bottom_colour = hot_to_cold_colour_mark_1(core_bottom_hotness);
        let fhr_core_bottom_coolant_shape =
            PathShape::convex_polygon(core_bottom_points, core_bottom_colour, coolant_stroke);

        let core_inlet_hotness = self.hotness(self.core_inlet_temp);
        let core_inlet_colour = hot_to_cold_colour_mark_1(core_inlet_hotness);
        let fhr_core_inlet_coolant_shape =
            PathShape::convex_polygon(core_bottom_inlet_points, core_inlet_colour, coolant_stroke);
        let core_mid_hotness = self.hotness(self.core_mid_coolant_temp);
        let core_mid_colour = hot_to_cold_colour_mark_1(core_mid_hotness);
        let fhr_core_mid_coolant_shape =
            PathShape::convex_polygon(core_mid_points, core_mid_colour, stroke);
        let core_top_hotness = self.hotness(self.core_top_temp);
        let core_top_colour = hot_to_cold_colour_mark_1(core_top_hotness);
        let fhr_core_top_coolant_shape =
            PathShape::convex_polygon(core_top_points, core_top_colour, coolant_stroke);
        let core_outlet_hotness = self.hotness(self.core_outlet_temp);
        let core_outlet_colour = hot_to_cold_colour_mark_1(core_outlet_hotness);
        let fhr_core_outlet_coolant_shape =
            PathShape::convex_polygon(core_outlet_points, core_outlet_colour, coolant_stroke);

        // ── Pebble bed ──────────────────────────────────────────────────────
        //
        // Placed from the baked, gravity-settled DEM packing, INVERTED,
        // because FHR pebbles are buoyant in FLiBe — see the module docs.
        //
        // The bed is the "fat" core region, which has no cone, so only the
        // packing's BARREL is used — height-fitted, inverted, and cropped to a
        // central column. All of that lives in `buoyant_bed_packing`.
        let (packing, packing_window) = buoyant_bed_packing(bed_rect);

        let pebble_bed_hotness = self.hotness(self.pebble_core_temp);
        let pebble_bed_colour = hot_to_cold_colour_mark_1(pebble_bed_hotness);

        // next, downcomers

        // left downcomer inlet
        let left_downcomer_inlet_bottom_pt =
            fhr_coolant_inlet_bottom_left + vec2(0.0, -reactor_half_length_y * 0.04);

        let left_downcomer_inlet_top_pt =
            fhr_coolant_inlet_bottom_left + vec2(0.0, -reactor_half_length_y * 0.12);

        let left_downcomer_inlet_mid_bottom_pt = fhr_coolant_inlet_bottom_left
            + vec2(-reactor_half_width_x * 0.65, -reactor_half_length_y * 0.16);

        let left_downcomer_inlet_mid_top_pt = fhr_coolant_inlet_bottom_left
            + vec2(-reactor_half_width_x * 0.6, -reactor_half_length_y * 0.22);

        // left downcomer mid rectangle
        //
        let left_downcomer_mid_rect_bottom_left =
            reactor_curved_edge_bottom_left + vec2(reactor_half_width_x * 0.06, 0.0);

        let left_downcomer_mid_rect_bottom_right =
            reactor_curved_edge_bottom_left + vec2(reactor_half_width_x * 0.16, 0.0);

        let left_downcomer_mid_rect_top_left =
            reactor_curved_edge_top_left + vec2(reactor_half_width_x * 0.06, 0.0);

        let left_downcomer_mid_rect_top_right =
            reactor_curved_edge_top_left + vec2(reactor_half_width_x * 0.16, 0.0);

        // left downcomer outlet

        let left_downcomer_outlet_top_pt =
            fhr_coolant_outlet_top_left + vec2(0.0, reactor_half_length_y * 0.04);

        let left_downcomer_outlet_bottom_pt =
            fhr_coolant_outlet_top_left + vec2(0.0, reactor_half_length_y * 0.12);

        let left_downcomer_outlet_mid_bottom_pt = fhr_coolant_outlet_top_left
            + vec2(-reactor_half_width_x * 0.6, reactor_half_length_y * 0.22);

        let left_downcomer_outlet_mid_top_pt = fhr_coolant_outlet_top_left
            + vec2(-reactor_half_width_x * 0.65, reactor_half_length_y * 0.16);

        let downcomer_inlet_left_1_pts = vec![
            left_downcomer_inlet_bottom_pt,
            left_downcomer_inlet_mid_bottom_pt,
            left_downcomer_inlet_mid_top_pt,
            left_downcomer_inlet_top_pt,
        ];

        let downcomer_inlet_left_2_pts = vec![
            left_downcomer_mid_rect_bottom_left,
            left_downcomer_mid_rect_bottom_right,
            left_downcomer_inlet_mid_top_pt,
            left_downcomer_inlet_mid_bottom_pt,
        ];

        let downcomer_left_mid_pts = vec![
            left_downcomer_mid_rect_bottom_left,
            left_downcomer_mid_rect_top_left,
            left_downcomer_mid_rect_top_right,
            left_downcomer_mid_rect_bottom_right,
        ];

        let downcomer_outlet_left_1_pts = vec![
            left_downcomer_outlet_bottom_pt,
            left_downcomer_outlet_mid_bottom_pt,
            left_downcomer_outlet_mid_top_pt,
            left_downcomer_outlet_top_pt,
        ];

        let downcomer_outlet_left_2_pts = vec![
            left_downcomer_mid_rect_top_left,
            left_downcomer_outlet_mid_top_pt,
            left_downcomer_outlet_mid_bottom_pt,
            left_downcomer_mid_rect_top_right,
        ];

        let downcomer_left_lower_hotness = self.hotness(self.left_downcomer_lower_temp);
        let downcomer_left_lower_colour = hot_to_cold_colour_mark_1(downcomer_left_lower_hotness);
        let left_downcomer_inlet_1_shape = PathShape::convex_polygon(
            downcomer_inlet_left_1_pts,
            downcomer_left_lower_colour,
            coolant_stroke,
        );
        let left_downcomer_inlet_2_shape = PathShape::convex_polygon(
            downcomer_inlet_left_2_pts,
            downcomer_left_lower_colour,
            coolant_stroke,
        );

        let downcomer_left_mid_hotness = self.hotness(self.left_downcomer_mid_temp);
        let downcomer_left_mid_colour = hot_to_cold_colour_mark_1(downcomer_left_mid_hotness);
        let left_downcomer_mid_shape = PathShape::convex_polygon(
            downcomer_left_mid_pts,
            downcomer_left_mid_colour,
            coolant_stroke,
        );

        let downcomer_left_upper_hotness = self.hotness(self.left_downcomer_upper_temp);
        let downcomer_left_upper_colour = hot_to_cold_colour_mark_1(downcomer_left_upper_hotness);
        let left_downcomer_outlet_1_shape = PathShape::convex_polygon(
            downcomer_outlet_left_1_pts,
            downcomer_left_upper_colour,
            coolant_stroke,
        );
        let left_downcomer_outlet_2_shape = PathShape::convex_polygon(
            downcomer_outlet_left_2_pts,
            downcomer_left_upper_colour,
            coolant_stroke,
        );

        // right downcomer

        // right downcomer inlet
        let right_downcomer_inlet_bottom_pt =
            fhr_coolant_inlet_bottom_right + vec2(0.0, -reactor_half_length_y * 0.04);

        let right_downcomer_inlet_top_pt =
            fhr_coolant_inlet_bottom_right + vec2(0.0, -reactor_half_length_y * 0.12);

        let right_downcomer_inlet_mid_bottom_pt = fhr_coolant_inlet_bottom_right
            + vec2(reactor_half_width_x * 0.65, -reactor_half_length_y * 0.16);

        let right_downcomer_inlet_mid_top_pt = fhr_coolant_inlet_bottom_right
            + vec2(reactor_half_width_x * 0.6, -reactor_half_length_y * 0.22);

        // right downcomer mid rectangle
        //
        let right_downcomer_mid_rect_bottom_left =
            reactor_curved_edge_bottom_right + vec2(-reactor_half_width_x * 0.16, 0.0);

        let right_downcomer_mid_rect_bottom_right =
            reactor_curved_edge_bottom_right + vec2(-reactor_half_width_x * 0.06, 0.0);

        let right_downcomer_mid_rect_top_left =
            reactor_curved_edge_top_right + vec2(-reactor_half_width_x * 0.16, 0.0);

        let right_downcomer_mid_rect_top_right =
            reactor_curved_edge_top_right + vec2(-reactor_half_width_x * 0.06, 0.0);

        // right downcomer outlet

        let right_downcomer_outlet_top_pt =
            fhr_coolant_outlet_top_right + vec2(0.0, reactor_half_length_y * 0.04);

        let right_downcomer_outlet_bottom_pt =
            fhr_coolant_outlet_top_right + vec2(0.0, reactor_half_length_y * 0.12);

        let right_downcomer_outlet_mid_bottom_pt = fhr_coolant_outlet_top_right
            + vec2(reactor_half_width_x * 0.6, reactor_half_length_y * 0.22);

        let right_downcomer_outlet_mid_top_pt = fhr_coolant_outlet_top_right
            + vec2(reactor_half_width_x * 0.65, reactor_half_length_y * 0.16);

        let downcomer_inlet_right_1_pts = vec![
            right_downcomer_inlet_bottom_pt,
            right_downcomer_inlet_mid_bottom_pt,
            right_downcomer_inlet_mid_top_pt,
            right_downcomer_inlet_top_pt,
        ];

        let downcomer_inlet_right_2_pts = vec![
            right_downcomer_inlet_mid_top_pt,
            right_downcomer_mid_rect_bottom_left,
            right_downcomer_mid_rect_bottom_right,
            right_downcomer_inlet_mid_bottom_pt,
        ];

        let downcomer_right_mid_pts = vec![
            right_downcomer_mid_rect_bottom_left,
            right_downcomer_mid_rect_top_left,
            right_downcomer_mid_rect_top_right,
            right_downcomer_mid_rect_bottom_right,
        ];

        let downcomer_outlet_right_1_pts = vec![
            right_downcomer_outlet_bottom_pt,
            right_downcomer_outlet_mid_bottom_pt,
            right_downcomer_outlet_mid_top_pt,
            right_downcomer_outlet_top_pt,
        ];

        let downcomer_outlet_right_2_pts = vec![
            right_downcomer_outlet_mid_top_pt,
            right_downcomer_mid_rect_top_right,
            right_downcomer_mid_rect_top_left,
            right_downcomer_outlet_mid_bottom_pt,
        ];

        let downcomer_right_lower_hotness = self.hotness(self.right_downcomer_lower_temp);
        let downcomer_right_lower_colour = hot_to_cold_colour_mark_1(downcomer_right_lower_hotness);
        let right_downcomer_inlet_1_shape = PathShape::convex_polygon(
            downcomer_inlet_right_1_pts,
            downcomer_right_lower_colour,
            coolant_stroke,
        );
        let right_downcomer_inlet_2_shape = PathShape::convex_polygon(
            downcomer_inlet_right_2_pts,
            downcomer_right_lower_colour,
            coolant_stroke,
        );
        let downcomer_right_mid_hotness = self.hotness(self.right_downcomer_mid_temp);
        let downcomer_right_mid_colour = hot_to_cold_colour_mark_1(downcomer_right_mid_hotness);
        let right_downcomer_mid_shape = PathShape::convex_polygon(
            downcomer_right_mid_pts,
            downcomer_right_mid_colour,
            coolant_stroke,
        );
        let downcomer_right_upper_hotness = self.hotness(self.right_downcomer_upper_temp);
        let downcomer_right_upper_colour = hot_to_cold_colour_mark_1(downcomer_right_upper_hotness);
        let right_downcomer_outlet_1_shape = PathShape::convex_polygon(
            downcomer_outlet_right_1_pts,
            downcomer_right_upper_colour,
            coolant_stroke,
        );
        let right_downcomer_outlet_2_shape = PathShape::convex_polygon(
            downcomer_outlet_right_2_pts,
            downcomer_right_upper_colour,
            coolant_stroke,
        );

        // now control rods

        let cr_channel_length_ratio = 1.0;
        let cr_channel_width_ratio = 0.08;

        let cr_left_ref_pt =
            reflector_curved_edge_top_left + vec2(reactor_half_width_x * 0.15, 0.0);

        let left_cr_channel_top_left =
            cr_left_ref_pt + vec2(-reactor_half_width_x * cr_channel_width_ratio, 0.0);

        let left_cr_channel_top_right =
            cr_left_ref_pt + vec2(reactor_half_width_x * cr_channel_width_ratio, 0.0);

        let left_cr_channel_bottom_left = cr_left_ref_pt
            + vec2(
                -reactor_half_width_x * 0.08,
                reactor_half_length_y * cr_channel_length_ratio,
            );

        let left_cr_channel_bottom_right = cr_left_ref_pt
            + vec2(
                reactor_half_width_x * 0.08,
                reactor_half_length_y * cr_channel_length_ratio,
            );

        let cr_channel_fill = Color32::LIGHT_BLUE;

        let left_cr_channel_pts = vec![
            left_cr_channel_top_left,
            left_cr_channel_top_right,
            left_cr_channel_bottom_right,
            left_cr_channel_bottom_left,
        ];

        let cr_left_channel_shape =
            PathShape::convex_polygon(left_cr_channel_pts, cr_channel_fill, coolant_stroke);

        let cr_right_ref_pt =
            reflector_curved_edge_top_right + vec2(-reactor_half_width_x * 0.15, 0.0);

        let right_cr_channel_top_left =
            cr_right_ref_pt + vec2(-reactor_half_width_x * cr_channel_width_ratio, 0.0);

        let right_cr_channel_top_right =
            cr_right_ref_pt + vec2(reactor_half_width_x * cr_channel_width_ratio, 0.0);

        let right_cr_channel_bottom_left = cr_right_ref_pt
            + vec2(
                -reactor_half_width_x * 0.08,
                reactor_half_length_y * cr_channel_length_ratio,
            );

        let right_cr_channel_bottom_right = cr_right_ref_pt
            + vec2(
                reactor_half_width_x * 0.08,
                reactor_half_length_y * cr_channel_length_ratio,
            );
        let right_cr_channel_pts = vec![
            right_cr_channel_top_left,
            right_cr_channel_top_right,
            right_cr_channel_bottom_right,
            right_cr_channel_bottom_left,
        ];

        let cr_right_channel_shape =
            PathShape::convex_polygon(right_cr_channel_pts, cr_channel_fill, coolant_stroke);

        let cr_width_ratio = 0.08;
        let cr_colour = Color32::DARK_GRAY;

        let cr_rod_stroke = Stroke::new(cr_width_ratio * reactor_half_width_x, cr_colour);

        let cr_length = reactor_half_length_y * 0.88;

        let cr_left_centre = cr_left_ref_pt
            + vec2(
                0.0,
                cr_length * self.left_control_rod_insertion_frac - cr_length * 0.9,
            );

        let cr_right_centre = cr_right_ref_pt
            + vec2(
                0.0,
                cr_length * self.right_control_rod_insertion_frac - cr_length * 0.9,
            );

        // now paint everything,
        // NOTE: the order of painting is important!

        // first:
        // control rod line segments (peripheral)
        //

        painter.line_segment(
            [
                cr_left_centre - vec2(-0.20 * reactor_half_width_x, cr_length),
                cr_left_centre + vec2(0.20 * reactor_half_width_x, cr_length),
            ],
            cr_rod_stroke,
        );
        painter.line_segment(
            [
                cr_left_centre - vec2(-0.40 * reactor_half_width_x, cr_length),
                cr_left_centre + vec2(0.40 * reactor_half_width_x, cr_length),
            ],
            cr_rod_stroke,
        );

        painter.line_segment(
            [
                cr_right_centre - vec2(0.20 * reactor_half_width_x, cr_length),
                cr_right_centre + vec2(-0.20 * reactor_half_width_x, cr_length),
            ],
            cr_rod_stroke,
        );
        painter.line_segment(
            [
                cr_right_centre - vec2(0.40 * reactor_half_width_x, cr_length),
                cr_right_centre + vec2(-0.40 * reactor_half_width_x, cr_length),
            ],
            cr_rod_stroke,
        );
        // next metallic and graphite structures
        // fhr metal vessel
        painter.add(fhr_bottom_metal_semicircle);
        painter.add(fhr_top_metal_semicircle);
        painter.add(fhr_mid_metal_rect);
        // fhr reflector graphite
        painter.add(reflector_bottom_graphite_semicircle);
        painter.add(reflector_top_graphite_semicircle);
        painter.add(reflector_mid_graphite_rect);

        // then coolants
        painter.add(fhr_core_bottom_coolant_shape);
        painter.add(fhr_core_inlet_coolant_shape);
        painter.add(fhr_core_top_coolant_shape);
        painter.add(fhr_core_outlet_coolant_shape);
        painter.add(fhr_core_mid_coolant_shape);

        painter.add(left_downcomer_inlet_1_shape);
        painter.add(left_downcomer_inlet_2_shape);
        painter.add(left_downcomer_mid_shape);
        painter.add(left_downcomer_outlet_1_shape);
        painter.add(left_downcomer_outlet_2_shape);

        painter.add(right_downcomer_inlet_1_shape);
        painter.add(right_downcomer_inlet_2_shape);
        painter.add(right_downcomer_mid_shape);
        painter.add(right_downcomer_outlet_1_shape);
        painter.add(right_downcomer_outlet_2_shape);

        // then pebble bed
        //
        // Each pebble is a graphite matrix speckled with TRISO kernels at the
        // fuel colour, not a solid coloured disc — the fission heat is made in
        // the particles, so the artwork reads as a graphite ball with hot dots
        // in it. Positions come from the baked settled packing, so the pebbles
        // rest on one another instead of floating on a lattice; the speckle is
        // seeded from each pebble's index in that table, so it is stable
        // across repaints. The graphite matrix stays black, as this vessel has
        // always drawn it.
        // The bed is baked to luminance textures once and tinted at draw time,
        // so `pebble_bed_colour` still tracks the fuel temperature every frame
        // without forcing a re-bake — see
        // [`crate::components::pebble_bed_texture`].
        draw_packed_pebbles(
            &painter,
            response.id.with("fhr_pebble_bed"),
            &packing,
            &packing_window,
            Color32::BLACK,
            &BedTint::Uniform(pebble_bed_colour),
        );

        // control rod channels and
        // control rod line segments (foreground)
        painter.add(cr_left_channel_shape);
        painter.add(cr_right_channel_shape);
        painter.line_segment(
            [
                cr_right_centre - vec2(0.0, cr_length),
                cr_right_centre + vec2(0.0, cr_length),
            ],
            cr_rod_stroke,
        );
        painter.line_segment(
            [
                cr_left_centre - vec2(0.0, cr_length),
                cr_left_centre + vec2(0.0, cr_length),
            ],
            cr_rod_stroke,
        );

        // finally return response as per what ui requested
        response
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use uom::si::thermodynamic_temperature::degree_celsius;

    /// Builds a vessel with a 500-700 degC display range and every region at
    /// the same temperature, so only the quantity under test varies.
    fn vessel_at(uniform_temp_degc: f64) -> FhrReactorVesselVisual {
        let t = ThermodynamicTemperature::new::<degree_celsius>(uniform_temp_degc);
        FhrReactorVesselVisual::new(
            Vec2::new(400.0, 500.0),
            ThermodynamicTemperature::new::<degree_celsius>(500.0),
            ThermodynamicTemperature::new::<degree_celsius>(700.0),
            t,
            t,
            t,
            t,
            t,
            t,
            t,
            t,
            t,
            t,
            t,
            t,
        )
    }

    /// Verifies the temperature-to-hotness mapping the whole widget colours
    /// from.
    ///
    /// **Methodology.** Display range fixed at 500-700 degC. `hotness` is
    /// evaluated at the two endpoints and the midpoint; the expected values
    /// follow from the linear definition
    /// `(T - T_min) / (T_max - T_min)`. Pass criterion: exact agreement in
    /// `f32` for values representable without rounding.
    ///
    /// **Results (2026-08-06).** 500 degC -> 0.0, 600 degC -> 0.5,
    /// 700 degC -> 1.0, all exact. Interpretation: the mapping is linear and
    /// correctly anchored, so a region drawn at the midpoint of the display
    /// range renders as the neutral colour rather than being biased hot or
    /// cold.
    #[test]
    fn hotness_is_linear_across_the_display_range() {
        let vessel = vessel_at(600.0);

        let at = |degc: f64| vessel.hotness(ThermodynamicTemperature::new::<degree_celsius>(degc));

        assert_eq!(at(500.0), 0.0);
        assert_eq!(at(600.0), 0.5);
        assert_eq!(at(700.0), 1.0);
    }

    /// `hotness` deliberately does not clamp -- the colour map saturates
    /// instead. This pins that contract so a future "fix" that clamps here
    /// does not silently change how out-of-range regions are shaded.
    #[test]
    fn hotness_is_not_clamped_outside_the_display_range() {
        let vessel = vessel_at(600.0);

        assert!(vessel.hotness(ThermodynamicTemperature::new::<degree_celsius>(400.0)) < 0.0);
        assert!(vessel.hotness(ThermodynamicTemperature::new::<degree_celsius>(800.0)) > 1.0);
    }

    /// Both control rods must start fully inserted, so a simulator that
    /// forgets to drive them renders a shut-down core rather than a critical
    /// one.
    #[test]
    fn control_rods_default_to_fully_inserted() {
        let vessel = vessel_at(600.0);

        assert_eq!(vessel.left_control_rod_insertion_frac, 1.0);
        assert_eq!(vessel.right_control_rod_insertion_frac, 1.0);
    }

    /// Setters move the rods independently -- a bug swapping the two would
    /// otherwise only show up visually.
    #[test]
    fn control_rod_setters_are_independent() {
        let mut vessel = vessel_at(600.0);
        vessel.set_left_cr_frac(0.25);
        vessel.set_right_cr_frac(0.75);

        assert_eq!(vessel.left_control_rod_insertion_frac, 0.25);
        assert_eq!(vessel.right_control_rod_insertion_frac, 0.75);
    }
}

#[cfg(test)]
mod packing_tests {
    use super::*;
    use crate::components::pebble_packing::{PackedPebble, BED_TOP, PACKED_PEBBLES, SPHERE_RADIUS};
    use egui::{Pos2, Rect};

    /// A representative drawn vessel: the simulator's native 225 x 1050 pt
    /// box, offset from the origin so a test cannot pass by assuming the
    /// artwork starts at (0, 0).
    fn drawn_rect() -> Rect {
        fit_native_aspect(Rect::from_min_size(
            Pos2::new(31.0, 13.0),
            Vec2::new(225.0, 1050.0),
        ))
    }

    /// **The bed must be inverted — FHR pebbles float.**
    ///
    /// This is the test that exists because the mistake is invisible in the
    /// arithmetic and obvious on screen.
    ///
    /// **Methodology.** Molten FLiBe (about 1940 kg/m³ at operating
    /// temperature) is denser than a graphite pebble (about 1740–1800 kg/m³),
    /// so an FHR bed floats up against a retainer at the **top** of the core
    /// and its free surface faces **down** — the mirror of the gas-cooled
    /// HTR-10 bed the packing was settled as. Two independent checks:
    ///
    /// 1. **Ordering.** The packing's settled base (`y = 0`) must be drawn at
    ///    the bed's top edge and its free surface (`y = BARREL_HEIGHT`) at the
    ///    bottom edge, so a low-`y` pebble gets a *smaller* screen `y` than a
    ///    high-`y` one. Note egui's screen `y` also points down, so this is a
    ///    deliberate double flip: the packing frame and the screen frame end
    ///    up running the same way.
    /// 2. **Density gradient.** Independently of the arithmetic, the drawn
    ///    upper half of the bed must carry more pebble area than the lower
    ///    half, because a gravity-settled packing is compressed at its base
    ///    and loose at its free surface. Areas are summed as `pi r^2` over the
    ///    drawn circles, split by which half of the bed the centre falls in.
    ///
    /// **Result (2026-08-06):** `y = 0` maps to the bed top and
    /// `y = BARREL_HEIGHT` to the bed bottom, both to within 1e-3 pt. The
    /// drawn upper half carries 0.5882 R² of circle area against 0.5353 R² in
    /// the lower half — a ratio of 1.099, i.e. the dense end is 9.9 % heavier
    /// and it is at the **top**. Interpretation: the bed reads as buoyant,
    /// packed against its retainer with a ragged free surface underneath, and
    /// has not been silently turned back into a gravity-settled one.
    #[test]
    fn the_buoyant_bed_is_inverted_so_the_dense_end_is_at_the_top() {
        let bed = pebble_bed_rect(drawn_rect());
        let (packing, _window) = buoyant_bed_packing(bed);

        // 1. Ordering: the settled base is drawn at the TOP.
        let base = PackedPebbleAt::new(0.0, 0.0);
        let surface = PackedPebbleAt::new(0.0, BARREL_HEIGHT);
        assert!((packing.centre(&base.0).y - bed.top()).abs() < 1e-3);
        assert!((packing.centre(&surface.0).y - bed.bottom()).abs() < 1e-3);
        assert!(
            packing.centre(&base.0).y < packing.centre(&surface.0).y,
            "the FHR bed is not inverted — a double flip has cancelled out"
        );

        // 2. The mapping sense itself — asserted DIRECTLY, not inferred from
        //    density.
        //
        // Density is the wrong instrument here and the 3-D bake proved it: a
        // settled monodisperse bed is near-uniform through its bulk (~0.61
        // solid fraction everywhere), so half-versus-half came out at ratio
        // 0.979 and outer-band occupancy at 35 vs 33 — both indistinguishable
        // from noise. The earlier flat-slice bake only appeared to show a
        // gradient because it stored CHORD radii, and the denser region
        // contributed more near-equatorial (large) chords. That was a weighting
        // artefact.
        //
        // The mapping sense, by contrast, is exact. For a BUOYANT bed a pebble
        // higher in the packing (larger packing y, i.e. nearer the settled
        // free surface) must be drawn LOWER on screen (larger screen y),
        // because the bed floats up against a retainer and its free surface
        // faces down. Under GravityUp the same pair maps the other way.
        let low = PackedPebble::new(0.0, 0.2, 0.0);
        let high = PackedPebble::new(0.0, 1.8, 0.0);
        let (low_y, high_y) = (packing.centre(&low).y, packing.centre(&high).y);
        println!(
            "buoyant mapping: packing y 0.2 -> screen {low_y:.2}, y 1.8 -> screen {high_y:.2}"
        );
        assert!(
            high_y > low_y,
            "the buoyant mapping is not inverted: packing y 1.8 drew at screen \
             {high_y:.2}, above y 0.2 at {low_y:.2}. That draws an FHR as if its \
             pebbles sank instead of floating"
        );

        // And the HTR-10 sense must be the opposite, or one of the two is wrong.
        let gravity = PackingTransform {
            axis_x: bed.center().x,
            origin_y: bed.top(),
            scale: 1.0,
            vertical: VerticalSense::GravityUp,
        };
        assert!(
            gravity.centre(&high).y < gravity.centre(&low).y,
            "GravityUp and Buoyant must map vertically in OPPOSITE senses"
        );
    }

    /// The cropped packing must fill the bed without spilling out of it.
    ///
    /// **Methodology.** The fat core is proportionally taller than the
    /// packing's barrel, so the scale is set by height and a central column is
    /// cropped out. Require: the barrel's full height to span the bed exactly;
    /// every kept circle to be drawn wholly inside the bed rectangle; and the
    /// crop to be a strict sub-column of the barrel (`< 1` vessel radius),
    /// which is what makes cropping — rather than stretching — necessary.
    ///
    /// **Result (2026-08-06):** at the native 225 x 1050 pt box the bed is
    /// 112.5 x 236.25 pt, giving a scale of 107.39 pt per vessel radius and a
    /// crop at |x| <= 0.5238 R, so 52.4 % of the barrel's width is used. 98 of
    /// the 261 baked circles are kept and every one lies inside the bed; the
    /// worst edge utilisation — how close a circle comes to the bed wall as a
    /// fraction of the bed half-width — is 0.980. Drawn pebble radius is
    /// 8.05 pt against the previous hand-placed 9.45 pt. Interpretation: the
    /// bed fills top to bottom at a single uniform scale, with no stretching
    /// and nothing tiled.
    #[test]
    fn the_cropped_column_fills_the_bed_without_spilling() {
        let bed = pebble_bed_rect(drawn_rect());
        let (packing, window) = buoyant_bed_packing(bed);

        assert!(
            (packing.scale * BARREL_HEIGHT - bed.height()).abs() < 1e-3,
            "the barrel must span the bed's full height"
        );
        assert!(
            window.max_abs_x < 1.0,
            "a crop narrower than the barrel is the whole point; if this ever \
             reaches 1.0 the bed has become as slender as the packing and the \
             crop can be dropped"
        );

        let mut kept = 0usize;
        let mut worst = 0.0f32;
        for pebble in PACKED_PEBBLES.iter().filter(|p| window.contains(p)) {
            let centre = packing.centre(pebble);
            let radius = packing.radius(pebble);
            kept += 1;
            worst = worst.max(((centre.x - bed.center().x).abs() + radius) / (bed.width() * 0.5));
            assert!(
                centre.x - radius >= bed.left() - 1e-3
                    && centre.x + radius <= bed.right() + 1e-3
                    && centre.y - radius >= bed.top() - 1e-3
                    && centre.y + radius <= bed.bottom() + 1e-3,
                "a pebble is drawn outside the bed"
            );
        }
        println!(
            "bed {:.2} x {:.2} pt; scale {:.2} pt/R; crop |x| <= {:.4} R \
             ({:.1} % of the barrel width); {kept} of {} circles kept; \
             pebble radius {:.2} pt; worst edge utilisation {worst:.3}",
            bed.width(),
            bed.height(),
            packing.scale,
            window.max_abs_x,
            100.0 * window.max_abs_x,
            PACKED_PEBBLES.len(),
            SPHERE_RADIUS * packing.scale,
        );
        assert!(
            kept > 90,
            "only {kept} pebbles survived the crop — the bed would read as sparse"
        );
    }

    /// A degenerate box must not divide by zero or paint a pebble.
    ///
    /// egui hands a widget a zero-sized rectangle transiently during layout,
    /// and a NaN scale would propagate into every circle it draws.
    #[test]
    fn a_degenerate_bed_draws_nothing() {
        let bed = pebble_bed_rect(Rect::from_min_size(Pos2::ZERO, Vec2::ZERO));
        let (packing, window) = buoyant_bed_packing(bed);

        assert_eq!(packing.scale, 0.0);
        assert!(packing.scale.is_finite());
        assert_eq!(
            PACKED_PEBBLES.iter().filter(|p| window.contains(p)).count(),
            0
        );
    }

    /// The packing's own free-surface height must still be the bed's fill
    /// level after inversion — sanity that `BED_TOP` was not left pointing at
    /// the wrong end of a flipped bed.
    #[test]
    fn the_free_surface_is_drawn_at_the_bottom_of_the_bed() {
        let bed = pebble_bed_rect(drawn_rect());
        let (packing, _) = buoyant_bed_packing(bed);

        let surface = PackedPebbleAt::new(0.0, BED_TOP);
        let y = packing.centre(&surface.0).y;
        assert!(
            y > bed.center().y,
            "the settled bed's free surface must be drawn in the LOWER half"
        );
        assert!(y <= bed.bottom() + 1e-3);
    }

    /// A zero-radius probe pebble at a chosen point of the packing frame, for
    /// asking where that point lands on screen.
    struct PackedPebbleAt(crate::components::pebble_packing::PackedPebble);

    impl PackedPebbleAt {
        fn new(x: f32, y: f32) -> Self {
            Self(crate::components::pebble_packing::PackedPebble::new(
                x, y, 0.0,
            ))
        }
    }
}

#[cfg(test)]
mod aspect_tests {
    use super::*;
    use egui::{Pos2, Rect};

    fn ratio(r: Rect) -> f32 {
        r.width() / r.height()
    }

    /// The artwork must keep its proportions in ANY box, which is what makes
    /// it reusable outside the simulator it was drawn for.
    ///
    /// **Methodology.** Fit the native ratio into boxes that are square, far
    /// too wide, and far too tall, and require each result to carry
    /// [`NATIVE_ASPECT_RATIO`] to within 1e-4, to fit inside its box, and to
    /// stay centred on it.
    ///
    /// **Results (2026-08-06):** all three fitted rects carry the native
    /// ratio, none exceeds its box, and all remain centred. Interpretation:
    /// the vessel letterboxes rather than stretching, so the gallery's nearly
    /// square cards no longer distort it.
    #[test]
    fn fit_preserves_the_native_ratio_in_any_box() {
        let boxes = [
            Rect::from_min_size(Pos2::ZERO, Vec2::new(300.0, 300.0)), // square
            Rect::from_min_size(Pos2::ZERO, Vec2::new(900.0, 200.0)), // too wide
            Rect::from_min_size(Pos2::ZERO, Vec2::new(100.0, 900.0)), // too tall
        ];

        for b in boxes {
            let fitted = fit_native_aspect(b);
            assert!(
                (ratio(fitted) - NATIVE_ASPECT_RATIO).abs() < 1e-4,
                "box {b:?} produced ratio {}, expected {NATIVE_ASPECT_RATIO}",
                ratio(fitted)
            );
            assert!(
                fitted.width() <= b.width() + 1e-3 && fitted.height() <= b.height() + 1e-3,
                "fitted {fitted:?} does not fit inside {b:?}"
            );
            assert!(
                (fitted.center() - b.center()).length() < 1e-3,
                "fitted rect is not centred on its box"
            );
        }
    }

    /// A caller already sizing to the native ratio must get its box back
    /// unchanged — `fhr_sim_v2` hardcodes 225 x 1050, and this change must not
    /// alter how the simulator looks.
    #[test]
    fn a_natively_proportioned_box_is_returned_unchanged() {
        let native = Rect::from_min_size(Pos2::new(10.0, 20.0), Vec2::new(225.0, 1050.0));
        let fitted = fit_native_aspect(native);

        assert!((fitted.width() - native.width()).abs() < 1e-3);
        assert!((fitted.height() - native.height()).abs() < 1e-3);
        assert!((fitted.center() - native.center()).length() < 1e-3);
    }

    /// A degenerate box must not produce NaN geometry — a zero-height
    /// allocation happens transiently during egui layout.
    #[test]
    fn degenerate_boxes_are_returned_as_is() {
        let zero = Rect::from_min_size(Pos2::ZERO, Vec2::new(0.0, 0.0));
        assert_eq!(fit_native_aspect(zero), zero);

        let flat = Rect::from_min_size(Pos2::ZERO, Vec2::new(200.0, 0.0));
        assert_eq!(fit_native_aspect(flat), flat);
    }
}
