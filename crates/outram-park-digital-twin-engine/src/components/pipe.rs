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
use uom::si::f64::{MassRate, ThermodynamicTemperature, Time};
use uom::si::thermodynamic_temperature::kelvin;

/// Width of the drawn pipe run, in screen points.
const PIPE_STROKE_WIDTH: f32 = 4.0;

/// Radius of a tracer mark, in screen points.
const TRACER_RADIUS: f32 = 3.0;

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
            tracer: None,
        }
    }

    /// Attach an application-owned [`TracerTrain`] so this run draws flow
    /// tracer marks. Builder-style, so it chains onto either constructor.
    pub fn with_tracer(mut self, tracer: TracerTrain) -> Self {
        self.tracer = Some(tracer);
        self
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
            },
        }
    }
}

impl Widget for PipeVisual {
    /// Draws the run as one coloured segment per cell (see
    /// [`PipeVisual::cell_temperatures`]), then any tracer marks on top.
    ///
    /// Cell colours come from [`hot_to_cold_colour_mark_1`] over the
    /// `[min_temp, max_temp]` range. If the backend reports no cells the run
    /// is drawn in a neutral grey rather than a made-up temperature colour,
    /// so an unpopulated pipe is visibly distinct from a cold one.
    fn ui(self, ui: &mut Ui) -> Response {
        let start = self.screen_position;
        let end = start + self.screen_vector;
        let rect = Rect::from_two_pos(start, end).expand(PIPE_STROKE_WIDTH);
        let response = ui.allocate_rect(rect, Sense::hover());
        let painter = ui.painter();

        let temperatures = self.cell_temperatures();

        if temperatures.is_empty() {
            painter.line_segment([start, end], Stroke::new(PIPE_STROKE_WIDTH, Color32::GRAY));
            return response;
        }

        // One coloured sub-segment per cell, inlet -> outlet.
        let n = temperatures.len();
        for (i, t) in temperatures.iter().enumerate() {
            let f0 = i as f32 / n as f32;
            let f1 = (i + 1) as f32 / n as f32;
            let p0 = start + self.screen_vector * f0;
            let p1 = start + self.screen_vector * f1;
            let hotness = hotness_from_temperature(*t, self.min_temp, self.max_temp);
            painter.line_segment(
                [p0, p1],
                Stroke::new(PIPE_STROKE_WIDTH, hot_to_cold_colour_mark_1(hotness)),
            );
        }

        if let Some(tracer) = self.tracer {
            for position in tracer.positions() {
                let at = start + self.screen_vector * position as f32;
                painter.circle_filled(at, TRACER_RADIUS, Color32::WHITE);
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

    fn visual(state: PipeVisualState) -> PipeVisual {
        PipeVisual {
            state,
            screen_position: Pos2::ZERO,
            screen_vector: Vec2::new(100.0, 0.0),
            min_temp: ThermodynamicTemperature::new::<kelvin>(300.0),
            max_temp: ThermodynamicTemperature::new::<kelvin>(400.0),
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

    /// The colour map must key off the *cell* temperature, so a mid-range
    /// cell maps to hotness 0.5 over a [300, 400] K display range.
    #[test]
    fn cell_temperature_drives_hotness() {
        let v = visual(PipeVisualState::Scalars(scalars(350.0)));
        let t = v.cell_temperatures()[0];
        assert_eq!(hotness_from_temperature(t, v.min_temp, v.max_temp), 0.5);
    }
}
