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

use egui::epaint::CubicBezierShape;
use egui::{epaint::PathShape, vec2, Color32, Pos2, Sense, Stroke, Vec2, Widget};
use uom::si::{f64::*, thermodynamic_temperature::degree_celsius};

use crate::color_maps::hot_to_cold_colour_mark_1;

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
        let fhr_core_fat_bottom_left =
            c + vec2(-0.50 * reactor_half_width_x, reactor_half_length_y * 0.45);
        let fhr_core_fat_bottom_right =
            c + vec2(0.50 * reactor_half_width_x, reactor_half_length_y * 0.45);

        let fhr_core_fat_top_left =
            c + vec2(-0.50 * reactor_half_width_x, -reactor_half_length_y * 0.45);
        let fhr_core_fat_top_right =
            c + vec2(0.50 * reactor_half_width_x, -reactor_half_length_y * 0.45);
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

        // now for pebble bed
        //
        let fhr_width = reactor_half_width_x * 2.0;
        let fhr_height = reactor_half_length_y * 2.0;
        let pebble_radius = fhr_width * 0.042;
        let core_radius = pebble_radius * 0.8;
        let pebble_ctr = c;

        painter.circle_filled(pebble_ctr, pebble_radius, Color32::BLACK);
        painter.circle_filled(pebble_ctr, core_radius, Color32::DARK_RED);

        let pebble_centers = vec![
            c + vec2(2.0 * pebble_radius, 0.1 * pebble_radius),
            c + vec2(1.0 * pebble_radius, -0.5 * pebble_radius),
            c + vec2(4.0 * pebble_radius, -0.3 * pebble_radius),
            c + vec2(-2.0 * pebble_radius, 0.1 * pebble_radius),
            c + vec2(-1.0 * pebble_radius, -0.5 * pebble_radius),
            c + vec2(-4.0 * pebble_radius, -0.3 * pebble_radius),
            c + vec2(2.0 * pebble_radius, 1.1 * pebble_radius),
            c + vec2(1.0 * pebble_radius, -1.5 * pebble_radius),
            c + vec2(4.0 * pebble_radius, -2.3 * pebble_radius),
            c + vec2(-2.0 * pebble_radius, 1.1 * pebble_radius),
            c + vec2(-1.0 * pebble_radius, -2.5 * pebble_radius),
            c + vec2(-4.0 * pebble_radius, -2.3 * pebble_radius),
            c + vec2(2.0 * pebble_radius, 1.1 * pebble_radius),
            c + vec2(3.0 * pebble_radius, -1.5 * pebble_radius),
            c + vec2(5.0 * pebble_radius, -2.3 * pebble_radius),
            c + vec2(-2.0 * pebble_radius, 1.1 * pebble_radius),
            c + vec2(-3.0 * pebble_radius, -2.5 * pebble_radius),
            c + vec2(-5.0 * pebble_radius, -2.3 * pebble_radius),
            c + vec2(-5.0 * pebble_radius, 2.3 * pebble_radius),
            c + vec2(5.0 * pebble_radius, 2.3 * pebble_radius),
            c + vec2(-4.2 * pebble_radius, 1.3 * pebble_radius),
            c + vec2(4.0 * pebble_radius, 1.4 * pebble_radius),
            c + vec2(-0.2 * pebble_radius, 1.3 * pebble_radius),
            c + vec2(0.0 * pebble_radius, 1.8 * pebble_radius),
        ];

        // add another list but transpose upwards
        let mut pebble_centres_bottom: Vec<Pos2> = pebble_centers.clone();
        let mut pebble_centres_top: Vec<Pos2> = pebble_centers.clone();

        for (i, pebble_center) in pebble_centers.iter().enumerate() {
            pebble_centres_bottom[i] = *pebble_center + vec2(0.0, fhr_height * 0.1);
        }
        for (i, pebble_center) in pebble_centers.iter().enumerate() {
            pebble_centres_top[i] = *pebble_center + vec2(0.0, -fhr_height * 0.1);
        }

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
        for pebble_center in pebble_centers.iter() {
            painter.circle_filled(*pebble_center, pebble_radius, Color32::BLACK);
            painter.circle_filled(*pebble_center, core_radius, pebble_bed_colour);
        }

        for pebble_center in pebble_centres_bottom.iter() {
            painter.circle_filled(*pebble_center, pebble_radius, Color32::BLACK);
            painter.circle_filled(*pebble_center, core_radius, pebble_bed_colour);
        }
        for pebble_center in pebble_centres_top.iter() {
            painter.circle_filled(*pebble_center, pebble_radius, Color32::BLACK);
            painter.circle_filled(*pebble_center, core_radius, pebble_bed_colour);
        }

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
