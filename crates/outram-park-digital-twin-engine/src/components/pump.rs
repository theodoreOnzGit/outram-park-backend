//! Visual pump: the physics wrapper.
//!
//! **Split 2026-11-05** (maintainer: "widget art can live in bugis, the
//! combination of logic and art can live inside the digital twin engine"):
//! the artwork, [`PumpKind`] and the port geometry are
//! [`bugis::components::pump`], re-exported here. This file holds the widget
//! that carries a [`tampines::components::Pump`] beside the drawing state, and
//! draws through [`PumpArt`].
//!
//! **Scalar-fed, deliberately.** `Pump::evaluate` returns
//! `TampinesError::NotYetImplemented`, so the wrapped component contributes
//! nothing to the drawing; the shaft speed and fluid temperature come from the
//! caller. See the bugis module for the full description.

pub use bugis::components::pump::*;
use egui::{Color32, Pos2, Rect, Response, Ui, Vec2, Widget};
use tampines::components::Pump;
use uom::si::f64::{Angle, AngularVelocity, ThermodynamicTemperature, Time};

/// Visual representation of a pump: the drawing state of a [`PumpArt`] plus
/// the wrapped TAMPINES component, when the caller has one.
///
/// Every field but [`Self::physics`] is the art's and means what it means on
/// [`PumpArt`]; [`Self::art`] hands them over.
pub struct PumpVisual {
    /// Which machine to draw.
    pub kind: PumpKind,
    /// The wrapped TAMPINES component, when the caller has one.
    ///
    /// Carried for API compatibility and future composition **only**: it
    /// contributes nothing to the drawing today, because `Pump::evaluate`
    /// returns `NotYetImplemented` and the struct itself holds no fluid state,
    /// no head and no shaft speed. Drawing anything from its efficiency or
    /// specification would be fabricating a reading.
    pub physics: Option<Pump>,
    /// On-screen centre position.
    pub screen_position: Pos2,
    /// On-screen size of the whole machine, in points.
    pub screen_vector: Vec2,
    /// Shaft angular velocity, positive in the drawn direction of rotation.
    pub shaft_speed: AngularVelocity,
    /// Elapsed simulation time, owned and advanced by the **application**.
    pub simulation_time: Time,
    /// Temperature of the pumped fluid, if the caller knows one.
    pub fluid_temperature: Option<ThermodynamicTemperature>,
    /// Temperature mapped to the coldest displayable colour.
    pub min_temp: ThermodynamicTemperature,
    /// Temperature mapped to the hottest displayable colour.
    pub max_temp: ThermodynamicTemperature,
}

impl PumpVisual {
    /// Wrap a [`tampines::components::Pump`] with screen geometry.
    ///
    /// Kept signature-compatible with the previous version of this widget. The
    /// result draws [`PumpArt::at_rest`]: a [`PumpKind::Centrifugal`] machine
    /// at rest, in neutral grey, because the wrapped component reports no shaft
    /// speed and no fluid state.
    pub fn new(physics: Pump, screen_position: Pos2, screen_vector: Vec2) -> Self {
        Self::from_art(Some(physics), PumpArt::at_rest(screen_position, screen_vector))
    }

    /// Build a pump visual directly from the scalars it draws
    /// ([`PumpArt::from_scalars`]).
    #[allow(clippy::too_many_arguments)]
    pub fn from_scalars(
        kind: PumpKind,
        screen_position: Pos2,
        screen_vector: Vec2,
        shaft_speed: AngularVelocity,
        simulation_time: Time,
        fluid_temperature: Option<ThermodynamicTemperature>,
        min_temp: ThermodynamicTemperature,
        max_temp: ThermodynamicTemperature,
    ) -> Self {
        Self::from_art(
            None,
            PumpArt::from_scalars(
                kind,
                screen_position,
                screen_vector,
                shaft_speed,
                simulation_time,
                fluid_temperature,
                min_temp,
                max_temp,
            ),
        )
    }

    /// The visual for `art`, carrying `physics` beside it.
    fn from_art(physics: Option<Pump>, art: PumpArt) -> Self {
        Self {
            kind: art.kind,
            physics,
            screen_position: art.screen_position,
            screen_vector: art.screen_vector,
            shaft_speed: art.shaft_speed,
            simulation_time: art.simulation_time,
            fluid_temperature: art.fluid_temperature,
            min_temp: art.min_temp,
            max_temp: art.max_temp,
        }
    }

    /// Draw a different machine. Builder-style.
    pub fn with_kind(mut self, kind: PumpKind) -> Self {
        self.kind = kind;
        self
    }

    /// Set the shaft angular velocity. Builder-style.
    pub fn with_shaft_speed(mut self, shaft_speed: AngularVelocity) -> Self {
        self.shaft_speed = shaft_speed;
        self
    }

    /// Supply the application's simulation time, which sets the rotor phase.
    /// Builder-style.
    pub fn at_time(mut self, simulation_time: Time) -> Self {
        self.simulation_time = simulation_time;
        self
    }

    /// Set the pumped fluid's temperature. Builder-style.
    pub fn with_fluid_temperature(mut self, fluid_temperature: ThermodynamicTemperature) -> Self {
        self.fluid_temperature = Some(fluid_temperature);
        self
    }

    /// Set the colour-mapping temperature range. Builder-style.
    pub fn with_temperature_range(
        mut self,
        min_temp: ThermodynamicTemperature,
        max_temp: ThermodynamicTemperature,
    ) -> Self {
        self.min_temp = min_temp;
        self.max_temp = max_temp;
        self
    }

    /// The drawing of this pump.
    pub fn art(&self) -> PumpArt {
        PumpArt {
            kind: self.kind,
            screen_position: self.screen_position,
            screen_vector: self.screen_vector,
            shaft_speed: self.shaft_speed,
            simulation_time: self.simulation_time,
            fluid_temperature: self.fluid_temperature,
            min_temp: self.min_temp,
            max_temp: self.max_temp,
        }
    }

    /// Rotor phase, `theta = omega * t` ([`PumpArt::rotor_angle`]).
    pub fn rotor_angle(&self) -> Angle {
        self.art().rotor_angle()
    }

    /// Whether the rotor is turning ([`PumpArt::is_turning`]).
    pub fn is_turning(&self) -> bool {
        self.art().is_turning()
    }

    /// The passage colour ([`PumpArt::fluid_colour`]).
    pub fn fluid_colour(&self) -> Color32 {
        self.art().fluid_colour()
    }

    /// Port positions for a centrifugal pump whose box is `box_rect`
    /// ([`PumpArt::centrifugal_ports`]).
    pub fn centrifugal_ports(box_rect: Rect) -> PumpPorts {
        PumpArt::centrifugal_ports(box_rect)
    }
}

impl Widget for PumpVisual {
    /// Draws [`PumpVisual::art`].
    fn ui(self, ui: &mut Ui) -> Response {
        self.art().ui(ui)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use uom::si::angle::radian;
    use uom::si::angular_velocity::radian_per_second;
    use uom::si::ratio::ratio;
    use uom::si::thermodynamic_temperature::kelvin;
    use uom::si::time::second;
    use uom::ConstZero;

    /// Wrapping a TAMPINES [`Pump`] must not invent state it does not have:
    /// `Pump::evaluate` is unimplemented, so there is no fluid temperature and
    /// no shaft speed to read, and the widget must say so rather than colour
    /// itself from a fabricated number.
    #[test]
    fn wrapping_a_tampines_pump_invents_no_state() {
        use outram_park_fork_dwsim_libs::pump::modes::PumpSpecification;
        use uom::si::f64::{Pressure, Ratio};
        use uom::si::pressure::megapascal;

        let physics = Pump::new(
            PumpSpecification::DeltaP(Pressure::new::<megapascal>(0.3)),
            Ratio::new::<ratio>(0.8),
        );
        let v = PumpVisual::new(physics, Pos2::ZERO, Vec2::new(44.0, 44.0));
        assert_eq!(v.kind, PumpKind::Centrifugal);
        assert!(v.fluid_temperature.is_none());
        assert_eq!(v.fluid_colour(), Color32::GRAY);
        assert!(!v.is_turning());
        assert_eq!(v.rotor_angle().get::<radian>(), 0.0);
    }

    /// The scalar path and the builders reach the art, and both paths draw.
    #[test]
    fn the_scalar_path_and_builders_reach_the_art() {
        let k = |v: f64| ThermodynamicTemperature::new::<kelvin>(v);
        let v = PumpVisual::from_scalars(
            PumpKind::Centrifugal,
            Pos2::new(40.0, 40.0),
            Vec2::new(44.0, 44.0),
            AngularVelocity::ZERO,
            Time::ZERO,
            None,
            k(300.0),
            k(600.0),
        )
        .with_kind(PumpKind::AxialPropeller)
        .with_shaft_speed(AngularVelocity::new::<radian_per_second>(2.0))
        .at_time(Time::new::<second>(3.0))
        .with_fluid_temperature(k(450.0))
        .with_temperature_range(k(300.0), k(900.0));
        assert!(v.physics.is_none());
        assert_eq!(v.art().kind, PumpKind::AxialPropeller);
        assert!(v.is_turning());
        assert!((v.rotor_angle().get::<radian>() - 6.0).abs() < 1e-9);
        assert_eq!(
            v.fluid_colour(),
            bugis::components::temperature_colour(k(450.0), k(300.0), k(900.0))
        );
        let box_rect = Rect::from_center_size(Pos2::new(40.0, 40.0), Vec2::new(44.0, 44.0));
        assert_eq!(PumpVisual::centrifugal_ports(box_rect), PumpArt::centrifugal_ports(box_rect));
        assert!(bugis::components::shapes_drawn_by(v) > 0);
    }
}
