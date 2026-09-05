//! A pipe that owns both its physics and its presentation.
//!
//! # Why this exists
//!
//! [`PipeVisual`] is an [`egui::Widget`], and egui widgets are **consumed by
//! value and rebuilt every repaint**. Anything stateful stored inside one is
//! therefore reset every frame: a physics array would be re-initialised sixty
//! times a second and never advance, and a tracer would sit permanently at
//! phase zero. That is why [`crate::animation::TracerTrain`] has always been
//! application-owned.
//!
//! [`PipeComponent`] is the persistent half. The application holds it across
//! frames, calls [`PipeComponent::step`] once per tick to advance the physics
//! and the tracer together, and calls [`PipeComponent::visual`] to mint the
//! throwaway widget for this frame's draw.
//!
//! ```text
//!   PipeComponent   (persistent: physics + tracer + display settings)
//!         |
//!         | .visual(position)   -- once per frame
//!         v
//!    PipeVisual     (ephemeral egui::Widget, consumed by the draw)
//! ```
//!
//! Stepping physics and tracer in one call is deliberate: the tracer's job is
//! to show how long fluid takes to cross the run, so if the two were advanced
//! separately they could drift and the animation would quietly stop meaning
//! anything.
//!
//! # This adds no physics
//!
//! Per this crate's `CLAUDE.md`, the engine holds no physics of its own.
//! [`PipeComponent::step`] calls [`tampines::components::Pipe::step`], which
//! dispatches to the backend's own solver. What lives here is ownership and
//! presentation, not equations.

use crate::animation::{residence_time_from_velocity, TracerPulse};
use crate::components::{PipeScale, PipeVisual};
use egui::{Pos2, Vec2};
use tampines::components::Pipe;
use tampines::TampinesError;
use uom::si::f64::{MassRate, ThermodynamicTemperature, Time, Velocity};
use uom::si::mass_rate::kilogram_per_second;

/// A pipe run, with its physics and everything needed to draw it.
pub struct PipeComponent {
    /// The physics. Advanced by [`Self::step`]; its geometry also drives the
    /// drawn length, thickness and slope.
    pub pipe: Pipe,
    /// Temperature drawn in the coldest displayable colour.
    pub min_temp: ThermodynamicTemperature,
    /// Temperature drawn in the hottest displayable colour.
    pub max_temp: ThermodynamicTemperature,
    /// Metres-to-points mapping for length and cross-section.
    pub scale: PipeScale,
    /// Metal temperature at or above which the wall is drawn red. `None`
    /// never reddens — see [`PipeVisual::wall_alarm_temp`] for why there is
    /// no default limit.
    pub wall_alarm_temp: Option<ThermodynamicTemperature>,
    /// The tracer mark. Persistent, so it keeps its phase across frames.
    pub tracer: TracerPulse,
    /// Bulk flow velocity. Sets the tracer's speed and, by its sign, the
    /// direction of travel.
    pub velocity: Velocity,
}

impl PipeComponent {
    /// Wrap a [`Pipe`] with a display range and a tracer released no more
    /// often than `tracer_interval`.
    pub fn new(
        pipe: Pipe,
        min_temp: ThermodynamicTemperature,
        max_temp: ThermodynamicTemperature,
        velocity: Velocity,
        tracer_interval: Time,
    ) -> Self {
        Self {
            pipe,
            min_temp,
            max_temp,
            scale: PipeScale::default(),
            wall_alarm_temp: None,
            tracer: TracerPulse::new(tracer_interval),
            velocity,
        }
    }

    /// Set the metal temperature at or above which the wall reddens.
    /// Builder-style.
    pub fn with_wall_alarm(mut self, alarm: ThermodynamicTemperature) -> Self {
        self.wall_alarm_temp = Some(alarm);
        self
    }

    /// Override the metres-to-points mapping. Builder-style.
    pub fn with_scale(mut self, scale: PipeScale) -> Self {
        self.scale = scale;
        self
    }

    /// Residence time of the run at the current velocity, `tau = L/u`.
    pub fn residence_time(&self) -> Time {
        residence_time_from_velocity(self.pipe.length, self.velocity)
    }

    /// Advance the physics **and** the tracer by `dt`.
    ///
    /// One call so the two cannot drift apart: the tracer exists to show how
    /// long fluid takes to cross this run, which is only true if it is
    /// advanced on the same clock as the fluid.
    ///
    /// # Errors
    ///
    /// Propagates whatever [`tampines::components::Pipe::step`] reports. The
    /// tracer is advanced regardless of the physics outcome — a stalled solver
    /// should be visible as a stalled temperature field, not as a frozen
    /// animation that looks like zero flow.
    pub fn step(&mut self, dt: Time) -> Result<(), TampinesError> {
        let tau = self.residence_time();
        // Direction only; TracerPulse takes speed from the residence time and
        // direction from this argument's sign (see its docs).
        let direction = MassRate::new::<kilogram_per_second>(if self.velocity.value >= 0.0 {
            1.0
        } else {
            -1.0
        });
        self.tracer.advance(dt, tau, direction);

        self.pipe.step(dt)
    }

    /// Build this frame's throwaway widget, anchored at `at`.
    ///
    /// Call every repaint. The returned [`PipeVisual`] borrows nothing and is
    /// consumed by the draw; all persistent state stays here.
    pub fn visual(&self, at: Pos2) -> PipeVisual {
        let mut widget = PipeVisual::new(
            self.pipe.clone(),
            at,
            // Direction comes from the pipe's own inclination; this is only
            // the fallback for geometry-less runs.
            Vec2::new(1.0, 0.0),
            self.min_temp,
            self.max_temp,
        )
        .with_scale(self.scale);

        if let Some(alarm) = self.wall_alarm_temp {
            widget = widget.with_wall_alarm(alarm);
        }
        // The pulse shows one mark at a time and reports None between
        // releases, so a gap draws no mark rather than parking one.
        if let Some(x) = self.tracer.position(self.residence_time()) {
            widget = widget.with_mark_at(x);
        }
        widget
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tampines::components::PipeBackend;
    use tampines::single_phase::{LiquidMaterial, SinglePhaseFluidArray};
    use tuas_boussinesq_solver::boussinesq_thermophysical_properties::SolidMaterial;
    use uom::si::angle::degree;
    use uom::si::f64::{Angle, Length, Pressure, Ratio};
    use uom::si::length::{meter, millimeter};
    use uom::si::pressure::atmosphere;
    use uom::si::ratio::ratio;
    use uom::si::thermodynamic_temperature::kelvin;
    use uom::si::time::second;
    use uom::si::velocity::meter_per_second;

    fn component(velocity_m_s: f64) -> PipeComponent {
        let bore = Length::new::<millimeter>(50.0);
        let length = Length::new::<meter>(3.0);
        let incline = Angle::new::<degree>(0.0);
        // 900 K: FLiBe melts near 732 K and TUAS rejects an initial
        // temperature below its valid range rather than extrapolating.
        let array = SinglePhaseFluidArray::new_cylinder(
            length,
            bore,
            ThermodynamicTemperature::new::<kelvin>(900.0),
            Pressure::new::<atmosphere>(1.0),
            SolidMaterial::SteelSS304L,
            LiquidMaterial::FLiBe,
            Ratio::new::<ratio>(0.0),
            4,
            incline,
        );
        let pipe = Pipe::new(
            PipeBackend::Lumped(array),
            bore,
            length,
            Length::new::<millimeter>(0.045),
            incline,
        );
        PipeComponent::new(
            pipe,
            ThermodynamicTemperature::new::<kelvin>(800.0),
            ThermodynamicTemperature::new::<kelvin>(1000.0),
            Velocity::new::<meter_per_second>(velocity_m_s),
            Time::new::<second>(2.5),
        )
    }

    /// Stepping must actually advance the backend rather than returning
    /// `NotYetImplemented`, which is what `Pipe::step` did before this work.
    #[test]
    fn stepping_advances_the_backend() {
        let mut c = component(1.0);
        c.step(Time::new::<second>(0.01))
            .expect("a lumped TUAS pipe should step");
    }

    /// A non-positive timestep must be reported, not clamped — a caller with a
    /// broken clock should find out rather than get a plausible-looking result
    /// for a step that never ran.
    #[test]
    fn non_positive_timestep_is_rejected() {
        let mut c = component(1.0);
        assert!(c.step(Time::new::<second>(0.0)).is_err());
        assert!(c.step(Time::new::<second>(-1.0)).is_err());
    }

    /// Residence time must come from the run length and velocity, so the
    /// tracer's crossing time is the pipe's own.
    #[test]
    fn residence_time_is_length_over_velocity() {
        let c = component(1.5);
        assert!((c.residence_time().get::<second>() - 2.0).abs() < 1e-9);
    }

    /// The tracer must survive across steps. This is the whole reason the
    /// component is persistent: a tracer living inside the egui widget would
    /// be rebuilt at phase zero every frame and never appear to move.
    #[test]
    fn tracer_phase_persists_across_steps() {
        let mut c = component(1.0);
        let tau = c.residence_time();
        let dt = Time::new::<second>(0.1);
        c.step(dt).unwrap();
        let before = c.tracer.position(tau).expect("mark should be in flight");
        c.step(dt).unwrap();
        let after = c
            .tracer
            .position(tau)
            .expect("mark should still be in flight");
        assert!(
            after > before,
            "tracer must advance across steps, got {before} then {after}"
        );
    }

    /// A stagnant run has no tracer speed, so no mark is placed on the widget.
    #[test]
    fn stagnant_run_places_no_mark() {
        let mut c = component(0.0);
        c.step(Time::new::<second>(0.1)).unwrap();
        assert!(c.tracer.position(c.residence_time()).is_none());
    }
}
