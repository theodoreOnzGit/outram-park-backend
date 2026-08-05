//! Visual pipe.
//!
//! Renders a pipe run as a coloured line whose colour comes from the fluid
//! temperature and whose optional tracer marks come from the mass flow --
//! the crate's core "rendering derives directly from physics state" idea,
//! applied to the component that carries most of a plant schematic's flow.
//!
//! ## Two ways to supply the state
//!
//! [`PipeVisualState`] is an enum (not a trait object, per the workspace's
//! mandatory design rules) with two variants, because digital-twin
//! applications reach this widget from two directions:
//!
//! - [`PipeVisualState::Physics`] wraps a full
//!   [`tampines::components::Pipe`]. The pipe's per-cell temperature profile
//!   is read straight off its flow backend, so the run is drawn as one
//!   coloured segment **per finite-volume cell** -- cell count drives
//!   displayed cells, cell temperature drives cell colour.
//! - [`PipeVisualState::Scalars`] takes a [`PipeScalars`] triple
//!   (temperature, mass flow, residence time) directly. A
//!   `tampines::components::Pipe` can only be built around a
//!   `SinglePhaseFluidArray` or `CompressibleFluidArray`, which is a heavy
//!   object to stand up for what may be a short connector line between two
//!   pieces of equipment. Simulators whose loop physics is their own lumped
//!   model (rather than a TAMPINES fluid array) supply that model's scalars
//!   here and still get correct colour and tracer motion.
//!
//! The scalar variant is *not* a placeholder for missing physics: the caller
//! is expected to pass real state from its own model. It is a narrower
//! interface, not a fabricated one.

use crate::animation::TracerTrain;
use crate::color_maps::hot_to_cold_colour_mark_1;
use crate::components::hotness_from_temperature;
use egui::{Color32, Pos2, Rect, Response, Sense, Stroke, Ui, Vec2, Widget};
use tampines::components::{Pipe, PipeBackend};
use uom::si::area::square_meter;
use uom::si::f64::{Angle, Area, Length, MassRate, ThermodynamicTemperature, Time};
use uom::si::angle::radian;
use uom::si::length::meter;
use uom::si::thermodynamic_temperature::kelvin;

/// Width of the pipe wall outline, in screen points.
const PIPE_WALL_WIDTH: f32 = 1.5;

/// Axial length of a tracer mark, as a fraction of the run length.
///
/// The mark is drawn as a rectangle spanning the pipe's full cross-section, so
/// it reads as a plug of fluid moving down the run rather than a dot floating
/// in it.
const TRACER_LENGTH_FRACTION: f32 = 0.06;

/// Scalar fluid state for a pipe run whose physics is not a
/// [`tampines::components::Pipe`].
///
/// Every field is the caller's own real model state -- see
/// [`PipeVisualState::Scalars`] for why this narrower interface exists.
#[derive(Debug, Clone, Copy)]
pub struct PipeScalars {
    /// Bulk fluid temperature of the run, used for the colour map.
    pub temperature: ThermodynamicTemperature,
    /// Mass flow through the run. Positive is `screen_position` ->
    /// `screen_position + screen_vector`; negative runs tracers in reverse.
    pub mass_flow: MassRate,
    /// End-to-end residence time, setting how long a tracer mark takes to
    /// cross the run (see [`crate::animation::residence_time_from_flow`]).
    pub residence_time: Time,
}

/// Where a [`PipeVisual`] gets the physics it renders.
///
/// Enum dispatch, not a trait object, per the workspace's mandatory
/// "no trait objects" Rust design rule.
#[derive(Debug, Clone)]
pub enum PipeVisualState {
    /// Backed by a full TAMPINES pipe, drawn one coloured segment per
    /// finite-volume cell.
    Physics(Pipe),
    /// Backed by caller-supplied scalars, drawn as a single coloured run.
    Scalars(PipeScalars),
}


/// How plant-space metres map to screen points.
///
/// The two scales are deliberately **separate**. A pipe run is metres long and
/// its bore is millimetres across; a single scale would render every pipe as an
/// invisible hairline. Keeping them apart lets length stay a true scale drawing
/// while the cross-section is exaggerated enough to see.
///
/// That exaggeration is honest but must be stated: two pipes drawn side by side
/// have *lengths* in true proportion to each other and *cross-sections* in true
/// proportion to each other, but thickness is not on the same scale as length.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct PipeScale {
    /// Screen points per metre of pipe length.
    pub points_per_metre: f32,
    /// Screen points of drawn thickness per square metre of flow
    /// cross-sectional area.
    ///
    /// Thickness is proportional to **area**, not diameter, so a pipe carrying
    /// four times the flow area draws four times as thick.
    pub points_per_square_metre: f32,
    /// Floor on drawn thickness, in points, so a small-bore pipe stays visible
    /// rather than vanishing.
    pub min_thickness_points: f32,
}

impl Default for PipeScale {
    /// Chosen so a 3 m run of 50 mm bore draws about 240 points long and
    /// 20 points thick — legible at gallery size.
    fn default() -> Self {
        Self {
            points_per_metre: 80.0,
            points_per_square_metre: 10_000.0,
            min_thickness_points: 6.0,
        }
    }
}

/// How the working fluid's phase is reflected in the drawing.
///
/// Enum dispatch per the workspace's "no trait objects" rule.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PipePhaseShade {
    /// Liquid: full-strength colour.
    Liquid,
    /// Gas or vapour: lightened. A gas is orders of magnitude less dense than
    /// the liquid at the same temperature, and washing the colour out is the
    /// cheapest way to make that legible at a glance without adding a second
    /// colour axis the reader has to learn.
    Gas,
    /// Two-phase, where the backend carries phase information but this widget
    /// is not yet reading a per-cell quality from it: drawn between the two.
    TwoPhase,
}

impl PipePhaseShade {
    /// Fraction of the way towards white, 0.0 = untouched.
    fn lightening(self) -> f32 {
        match self {
            Self::Liquid => 0.0,
            Self::TwoPhase => 0.25,
            Self::Gas => 0.55,
        }
    }

    /// Apply this shade's lightening to a colour.
    fn apply(self, c: Color32) -> Color32 {
        let f = self.lightening();
        let mix = |v: u8| -> u8 { (v as f32 + (255.0 - v as f32) * f).round() as u8 };
        Color32::from_rgb(mix(c.r()), mix(c.g()), mix(c.b()))
    }
}

/// Visual representation of a pipe run.
///
/// `screen_vector` gives the pipe's on-screen direction and length (from
/// `screen_position` to `screen_position + screen_vector`), which is also the
/// direction a positive mass flow's tracers travel.
pub struct PipeVisual {
    /// The physics state this widget renders.
    pub state: PipeVisualState,
    /// On-screen anchor position (the inlet endpoint of the pipe).
    pub screen_position: Pos2,
    /// On-screen direction and length, from `screen_position` to the outlet.
    pub screen_vector: Vec2,
    /// Temperature mapped to [`crate::color_maps::hot_to_cold_colour_mark_1`]'s
    /// `hotness = 0.0` (coldest displayable colour).
    pub min_temp: ThermodynamicTemperature,
    /// Temperature mapped to `hotness = 1.0` (hottest displayable colour).
    pub max_temp: ThermodynamicTemperature,
    /// Metres-to-points mapping for length and cross-section.
    pub scale: PipeScale,
    /// Optional flow-tracer marks drawn along the run.
    ///
    /// The train is *advanced by the application*, once per frame, and copied
    /// in here at widget-build time -- widgets are rebuilt every repaint, so a
    /// train owned by the widget would reset its phase to zero each frame.
    /// See [`crate::animation`] for the ownership rationale.
    pub tracer: Option<TracerTrain>,
}

impl PipeVisual {
    /// Wrap a [`Pipe`] with the given screen geometry and colour-mapping
    /// temperature range. The run renders one coloured segment per
    /// finite-volume cell of the pipe's flow backend.
    pub fn new(
        physics: Pipe,
        screen_position: Pos2,
        screen_vector: Vec2,
        min_temp: ThermodynamicTemperature,
        max_temp: ThermodynamicTemperature,
    ) -> Self {
        Self {
            state: PipeVisualState::Physics(physics),
            screen_position,
            screen_vector,
            min_temp,
            max_temp,
            scale: PipeScale::default(),
            tracer: None,
        }
    }

    /// Build a pipe run from caller-supplied [`PipeScalars`] rather than a
    /// full [`Pipe`] -- the connector-line path described in the module docs.
    pub fn from_scalars(
        scalars: PipeScalars,
        screen_position: Pos2,
        screen_vector: Vec2,
        min_temp: ThermodynamicTemperature,
        max_temp: ThermodynamicTemperature,
    ) -> Self {
        Self {
            state: PipeVisualState::Scalars(scalars),
            screen_position,
            screen_vector,
            min_temp,
            max_temp,
            scale: PipeScale::default(),
            tracer: None,
        }
    }

    /// Override the metres-to-points mapping. Builder-style.
    pub fn with_scale(mut self, scale: PipeScale) -> Self {
        self.scale = scale;
        self
    }

    /// Attach an application-owned [`TracerTrain`] so this run draws flow
    /// tracer marks. Builder-style, so it chains onto either constructor.
    pub fn with_tracer(mut self, tracer: TracerTrain) -> Self {
        self.tracer = Some(tracer);
        self
    }

    /// Flow cross-sectional area, from the pipe's bore.
    ///
    /// `None` for [`PipeVisualState::Scalars`], which carries no geometry —
    /// those runs fall back to [`PipeScale::min_thickness_points`] rather than
    /// inventing a bore.
    pub fn cross_sectional_area(&self) -> Option<Area> {
        match &self.state {
            PipeVisualState::Scalars(_) => None,
            PipeVisualState::Physics(pipe) => {
                let d = pipe.diameter;
                Some(std::f64::consts::FRAC_PI_4 * d * d)
            }
        }
    }

    /// Physical run length, `None` for scalar-backed runs (no geometry).
    pub fn run_length(&self) -> Option<Length> {
        match &self.state {
            PipeVisualState::Scalars(_) => None,
            PipeVisualState::Physics(pipe) => Some(pipe.length),
        }
    }

    /// Inclination from horizontal, positive uphill. `None` for scalar runs.
    pub fn inclination(&self) -> Option<Angle> {
        match &self.state {
            PipeVisualState::Scalars(_) => None,
            PipeVisualState::Physics(pipe) => Some(pipe.inclination),
        }
    }

    /// How this backend's fluid phase is shaded.
    ///
    /// Derived from the backend, which is the only phase information available
    /// without reading a per-cell quality: a lumped liquid array cannot boil, a
    /// CoolProp compressible array is being used for a gas, and the HEM array
    /// is the one that genuinely carries two-phase state.
    pub fn phase_shade(&self) -> PipePhaseShade {
        match &self.state {
            // Caller-supplied scalars say nothing about phase.
            PipeVisualState::Scalars(_) => PipePhaseShade::Liquid,
            PipeVisualState::Physics(pipe) => match &pipe.backend {
                PipeBackend::Lumped(_) => PipePhaseShade::Liquid,
                PipeBackend::Compressible(_) => PipePhaseShade::Gas,
                PipeBackend::SteamHem(_) => PipePhaseShade::TwoPhase,
            },
        }
    }

    /// Drawn size in screen points: `(length, thickness)`.
    ///
    /// Length is proportional to the real run length and thickness to the real
    /// flow cross-sectional area, both through [`PipeScale`]. A scalar-backed
    /// run has no geometry, so it falls back to the caller's `screen_vector`
    /// length and the minimum thickness.
    pub fn drawn_size(&self) -> (f32, f32) {
        let length = match self.run_length() {
            Some(l) => l.get::<meter>() as f32 * self.scale.points_per_metre,
            None => self.screen_vector.length(),
        };
        let thickness = match self.cross_sectional_area() {
            Some(a) => (a.get::<square_meter>() as f32 * self.scale.points_per_square_metre)
                .max(self.scale.min_thickness_points),
            None => self.scale.min_thickness_points,
        };
        (length.max(1.0), thickness)
    }

    /// Per-cell fluid temperatures along the run, inlet -> outlet.
    ///
    /// For [`PipeVisualState::Physics`] this is the flow backend's
    /// finite-volume temperature profile; for [`PipeVisualState::Scalars`]
    /// it is the single supplied bulk temperature. Returns an empty vector
    /// if a backend cannot report its profile (a fresh array with no cells,
    /// or a backend error) -- callers render nothing rather than a
    /// fabricated colour in that case.
    pub fn cell_temperatures(&self) -> Vec<ThermodynamicTemperature> {
        match &self.state {
            PipeVisualState::Scalars(s) => vec![s.temperature],
            PipeVisualState::Physics(pipe) => match &pipe.backend {
                PipeBackend::Lumped(array) => array.get_temperature_vector().unwrap_or_default(),
                PipeBackend::Compressible(array) => array
                    .t
                    .internal
                    .iter()
                    .map(|t_k| ThermodynamicTemperature::new::<kelvin>(*t_k))
                    .collect(),
                // Same field shape as the compressible array: both are ports of
                // rhoPimpleFoam over different equations of state, so the
                // temperature field is read identically.
                PipeBackend::SteamHem(array) => array
                    .t
                    .internal
                    .iter()
                    .map(|t_k| ThermodynamicTemperature::new::<kelvin>(*t_k))
                    .collect(),
            },
        }
    }
}

impl Widget for PipeVisual {
    /// Draws the run as a rectangle divided into one box per finite-volume
    /// cell (see [`PipeVisual::cell_temperatures`]), then any tracer marks.
    ///
    /// Length and thickness both come from the real pipe geometry via
    /// [`PipeScale`] — length from the run length, thickness from the flow
    /// cross-sectional area — and the run is drawn at the pipe's inclination.
    /// Each cell box is filled by its own temperature, then lightened
    /// according to [`PipeVisual::phase_shade`] so a gas-filled pipe reads
    /// paler than a liquid one at the same temperature.
    ///
    /// Cell colours come from [`hot_to_cold_colour_mark_1`] over the
    /// `[min_temp, max_temp]` range. If the backend reports no cells the run
    /// is drawn in a neutral grey rather than a made-up temperature colour,
    /// so an unpopulated pipe is visibly distinct from a cold one.
    fn ui(self, ui: &mut Ui) -> Response {
        let (length_pts, thickness_pts) = self.drawn_size();
        let half_t = 0.5 * thickness_pts;

        // Run direction. A physics-backed pipe knows its inclination, so the
        // run is drawn at that slope -- this is what lets a natural-circulation
        // loop read correctly, with hot legs rising and cold legs falling.
        // Screen y grows downwards, so a positive (uphill) inclination must
        // take the run UP the screen.
        let direction = match self.inclination() {
            Some(incline) => {
                let a = incline.get::<radian>() as f32;
                Vec2::new(a.cos(), -a.sin())
            }
            None => {
                let v = self.screen_vector;
                if v.length() > f32::EPSILON {
                    v.normalized()
                } else {
                    Vec2::new(1.0, 0.0)
                }
            }
        };
        let normal = Vec2::new(-direction.y, direction.x);

        let start = self.screen_position;
        let end = start + direction * length_pts;
        let rect = Rect::from_two_pos(start, end).expand(half_t.max(1.0));
        let response = ui.allocate_rect(rect, Sense::hover());
        let painter = ui.painter();

        let temperatures = self.cell_temperatures();
        let shade = self.phase_shade();

        // A rectangle spanning the run, divided into one box per cell.
        let quad = |p0: Pos2, p1: Pos2| -> Vec<Pos2> {
            vec![
                p0 + normal * half_t,
                p1 + normal * half_t,
                p1 - normal * half_t,
                p0 - normal * half_t,
            ]
        };

        if temperatures.is_empty() {
            // No cells reported: neutral grey, never a made-up temperature
            // colour, so an unpopulated pipe stays distinct from a cold one.
            painter.add(egui::Shape::convex_polygon(
                quad(start, end),
                Color32::GRAY,
                Stroke::NONE,
            ));
            return response;
        }

        // One box per finite-volume cell, inlet -> outlet, each filled by its
        // own cell temperature and outlined so the cell count is countable.
        let n = temperatures.len();
        let outline = Stroke::new(1.0, Color32::from_black_alpha(90));
        for (i, t) in temperatures.iter().enumerate() {
            let f0 = i as f32 / n as f32;
            let f1 = (i + 1) as f32 / n as f32;
            let p0 = start + direction * (length_pts * f0);
            let p1 = start + direction * (length_pts * f1);
            let hotness = hotness_from_temperature(*t, self.min_temp, self.max_temp);
            let fill = shade.apply(hot_to_cold_colour_mark_1(hotness));
            painter.add(egui::Shape::convex_polygon(quad(p0, p1), fill, outline));
        }

        // Pipe wall, drawn over the cell boxes so the run reads as one pipe.
        painter.add(egui::Shape::convex_polygon(
            quad(start, end),
            Color32::TRANSPARENT,
            Stroke::new(PIPE_WALL_WIDTH, Color32::from_gray(70)),
        ));

        // Tracer marks: white rectangles spanning the bore, each travelling
        // the full run in exactly one residence time (the train is advanced by
        // the application -- see the field docs). Direction of travel is the
        // train's, so a reversed flow runs them backwards.
        if let Some(tracer) = self.tracer {
            let mark_len = (length_pts * TRACER_LENGTH_FRACTION).max(2.0);
            for position in tracer.positions() {
                let centre = length_pts * position as f32;
                // Clipped to the run so a mark straddling the outlet does not
                // spill out of the pipe.
                let a = (centre - 0.5 * mark_len).clamp(0.0, length_pts);
                let b = (centre + 0.5 * mark_len).clamp(0.0, length_pts);
                if b - a < 0.5 {
                    continue;
                }
                painter.add(egui::Shape::convex_polygon(
                    quad(start + direction * a, start + direction * b),
                    Color32::WHITE,
                    Stroke::NONE,
                ));
            }
        }

        response
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use uom::si::mass_rate::kilogram_per_second;
    use uom::si::time::second;

    fn scalars(t_k: f64) -> PipeScalars {
        PipeScalars {
            temperature: ThermodynamicTemperature::new::<kelvin>(t_k),
            mass_flow: MassRate::new::<kilogram_per_second>(1.0),
            residence_time: Time::new::<second>(5.0),
        }
    }

    /// A physics-backed pipe of the given bore (mm), length (m) and
    /// inclination (degrees), on the default scale.
    fn physics_pipe(bore_mm: f64, length_m: f64, incline_deg: f64) -> PipeVisual {
        use tampines::components::PipeBackend;
        use tampines::single_phase::{LiquidMaterial, SinglePhaseFluidArray};
        use tuas_boussinesq_solver::boussinesq_thermophysical_properties::SolidMaterial;
        use uom::si::angle::degree;
        use uom::si::length::millimeter;
        use uom::si::pressure::atmosphere;
        use uom::si::ratio::ratio;
        use uom::si::f64::{Pressure, Ratio};

        let diameter = Length::new::<millimeter>(bore_mm);
        let length = Length::new::<meter>(length_m);
        let incline = Angle::new::<degree>(incline_deg);
        let array = SinglePhaseFluidArray::new_cylinder(
            length,
            diameter,
            // 900 K: FLiBe melts around 732 K, and TUAS rejects an initial
            // temperature below its valid range rather than extrapolating the
            // property correlations. Do not lower this.
            ThermodynamicTemperature::new::<kelvin>(900.0),
            Pressure::new::<atmosphere>(1.0),
            SolidMaterial::SteelSS304L,
            LiquidMaterial::FLiBe,
            Ratio::new::<ratio>(0.0),
            4,
            incline,
        );
        let pipe = tampines::components::Pipe::new(
            PipeBackend::Lumped(array),
            diameter,
            length,
            Length::new::<millimeter>(0.045),
            incline,
        );
        PipeVisual::new(
            pipe,
            Pos2::ZERO,
            Vec2::new(100.0, 0.0),
            ThermodynamicTemperature::new::<kelvin>(300.0),
            ThermodynamicTemperature::new::<kelvin>(900.0),
        )
    }

    fn visual(state: PipeVisualState) -> PipeVisual {
        PipeVisual {
            state,
            screen_position: Pos2::ZERO,
            screen_vector: Vec2::new(100.0, 0.0),
            min_temp: ThermodynamicTemperature::new::<kelvin>(300.0),
            max_temp: ThermodynamicTemperature::new::<kelvin>(400.0),
            scale: PipeScale::default(),
            tracer: None,
        }
    }

    #[test]
    fn scalar_state_reports_its_single_temperature() {
        let v = visual(PipeVisualState::Scalars(scalars(350.0)));
        let temps = v.cell_temperatures();
        assert_eq!(temps.len(), 1);
        assert_eq!(temps[0].get::<kelvin>(), 350.0);
    }

    #[test]
    fn from_scalars_builds_a_scalar_backed_run() {
        let v = PipeVisual::from_scalars(
            scalars(360.0),
            Pos2::ZERO,
            Vec2::new(10.0, 0.0),
            ThermodynamicTemperature::new::<kelvin>(300.0),
            ThermodynamicTemperature::new::<kelvin>(400.0),
        );
        assert!(matches!(v.state, PipeVisualState::Scalars(_)));
        assert!(v.tracer.is_none());
    }

    #[test]
    fn with_tracer_attaches_an_application_owned_train() {
        let v = visual(PipeVisualState::Scalars(scalars(350.0))).with_tracer(TracerTrain::new(3));
        let tracer = v.tracer.expect("tracer should be attached");
        assert_eq!(tracer.count(), 3);
    }

    /// Drawn thickness must be proportional to the flow CROSS-SECTIONAL AREA,
    /// not the diameter — doubling the bore quadruples the area, so the pipe
    /// must draw four times as thick, not twice.
    ///
    /// **Methodology:** build two physics-backed pipes differing only in bore
    /// (50 mm and 100 mm), read `drawn_size()`, and compare the thickness
    /// ratio against the area ratio of 4.
    ///
    /// **Result (2026-08-05):** ratio 4.000, to within 1e-3. Both are above
    /// the minimum-thickness floor, so the floor does not mask the test.
    #[test]
    fn thickness_is_proportional_to_cross_sectional_area() {
        let thick = |bore_mm: f64| -> f32 {
            let v = physics_pipe(bore_mm, 3.0, 0.0);
            v.drawn_size().1
        };
        let (small, large) = (thick(50.0), thick(100.0));
        assert!(
            small > PipeScale::default().min_thickness_points,
            "test would be masked by the thickness floor"
        );
        assert!(
            (large / small - 4.0).abs() < 1e-3,
            "expected 4x thickness for 2x bore, got {}",
            large / small
        );
    }

    /// Drawn length must be proportional to the real run length.
    #[test]
    fn length_is_proportional_to_run_length() {
        let len = |m: f64| physics_pipe(50.0, m, 0.0).drawn_size().0;
        assert!((len(6.0) / len(3.0) - 2.0).abs() < 1e-3);
    }

    /// Phase shading must lighten a gas relative to a liquid at the same
    /// temperature, and must never darken it.
    #[test]
    fn gas_is_lighter_than_liquid() {
        let base = hot_to_cold_colour_mark_1(0.5);
        let liquid = PipePhaseShade::Liquid.apply(base);
        let gas = PipePhaseShade::Gas.apply(base);
        let two_phase = PipePhaseShade::TwoPhase.apply(base);
        assert_eq!(liquid, base, "liquid must be untouched");
        assert!(gas.r() >= two_phase.r() && two_phase.r() >= liquid.r());
        assert!(gas.g() >= two_phase.g() && two_phase.g() >= liquid.g());
        assert!(gas.b() >= two_phase.b() && two_phase.b() >= liquid.b());
    }

    /// The colour map must key off the *cell* temperature, so a mid-range
    /// cell maps to hotness 0.5 over a [300, 400] K display range.
    #[test]
    fn cell_temperature_drives_hotness() {
        let v = visual(PipeVisualState::Scalars(scalars(350.0)));
        let t = v.cell_temperatures()[0];
        assert_eq!(hotness_from_temperature(t, v.min_temp, v.max_temp), 0.5);
    }
}
