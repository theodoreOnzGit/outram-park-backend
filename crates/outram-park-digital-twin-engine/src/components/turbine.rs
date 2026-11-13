//! Visual **steam** turbine: the physics wrapper.
//!
//! **Split 2026-11-05** (maintainer: "widget art can live in bugis, the
//! combination of logic and art can live inside the digital twin engine"):
//! the blade artwork, [`TurbineFlowPath`], [`StageAngles`] and the port
//! geometry are [`bugis::components::turbine`], re-exported here. This file
//! holds the state sources ([`TurbineVisualState`]) and the widget, which
//! reduces each to the shaft speed and casing temperature [`TurbineArt`]
//! draws:
//!
//! - [`TurbineVisualState::SteamGenerator`] wraps
//!   [`ThreePhaseElectricGeneratorTurbine`] from `tampines-steam-tables`, a
//!   working torque-balance model: the rotor turns at its real `omega`; no
//!   casing temperature.
//! - [`TurbineVisualState::SteamThermo`] wraps
//!   [`tampines::components::Turbine`]: the casing takes its inlet steam
//!   temperature; `Turbine::expand_to` is not implemented, so there is no
//!   shaft speed and the rotor is drawn stationary.
//! - [`TurbineVisualState::Scalars`]: the caller's own speed and temperature.

pub use bugis::components::turbine::*;
use egui::{Pos2, Response, Ui, Vec2, Widget};
use tampines::components::Turbine;
use tampines_steam_tables::steam_turbine_equations::generator::ThreePhaseElectricGeneratorTurbine;
use uom::si::f64::{Angle, AngularVelocity, ThermodynamicTemperature, Time};
use uom::ConstZero;

/// Where a [`TurbineVisual`] gets the physics it renders.
///
/// Enum dispatch, not a trait object, per the workspace's mandatory "no trait
/// objects" Rust design rule. See the module docs for why each variant exists
/// and what it can and cannot show.
#[derive(Debug, Clone, PartialEq)]
pub enum TurbineVisualState {
    /// Steam turbine coupled to a three-phase synchronous generator. Reports a
    /// real, torque-balance-derived shaft speed, so the rotor spins.
    SteamGenerator(ThreePhaseElectricGeneratorTurbine),
    /// Steam turbine known only by its thermodynamic inlet state. Colours the
    /// casing; cannot report a shaft speed, so the rotor is drawn stationary.
    SteamThermo(Turbine),
    /// Plain scalars from the caller, no physics model: a shaft speed, which
    /// turns the rotor, and optionally an inlet steam temperature, which
    /// colours the casing. The one variant that can do both. For a GUI test
    /// bench, or any caller whose own model already knows these two numbers.
    /// Added 2026-09-22.
    Scalars {
        /// Shaft angular velocity; the rotor phase is `shaft_speed * t`.
        shaft_speed: AngularVelocity,
        /// Inlet steam temperature for the casing colour, if known.
        steam_temperature: Option<ThermodynamicTemperature>,
    },
}

/// Visual representation of a steam turbine: a [`TurbineArt`] driven by a
/// [`TurbineVisualState`].
pub struct TurbineVisual {
    /// The physics state this widget renders.
    pub state: TurbineVisualState,
    /// On-screen centre position.
    pub screen_position: Pos2,
    /// On-screen size of the whole machine, in points.
    pub screen_vector: Vec2,
    /// How steam is admitted and exhausted. Defaults to
    /// [`TurbineFlowPath::DoubleFlow`], standard for large PWR cylinders.
    pub flow_path: TurbineFlowPath,
    /// Elapsed simulation time, owned and advanced by the application.
    pub simulation_time: Time,
    /// Temperature mapped to `hotness = 0.0` (coldest displayable colour).
    pub min_temp: ThermodynamicTemperature,
    /// Temperature mapped to `hotness = 1.0` (hottest displayable colour).
    pub max_temp: ThermodynamicTemperature,
}

impl TurbineVisual {
    /// Wrap a [`ThreePhaseElectricGeneratorTurbine`] — the variant with a real
    /// shaft speed, so the rotor spins at `omega * t`.
    ///
    /// `min_temp`/`max_temp` bound the casing colour map; a generator-backed
    /// turbine has no steam temperature of its own, so the casing is drawn in
    /// neutral grey and the range is carried only so the two variants stay
    /// interchangeable in a gallery.
    pub fn new_generator(
        generator: ThreePhaseElectricGeneratorTurbine,
        screen_position: Pos2,
        screen_vector: Vec2,
        min_temp: ThermodynamicTemperature,
        max_temp: ThermodynamicTemperature,
    ) -> Self {
        Self {
            state: TurbineVisualState::SteamGenerator(generator),
            screen_position,
            screen_vector,
            flow_path: TurbineFlowPath::default(),
            simulation_time: Time::ZERO,
            min_temp,
            max_temp,
        }
    }

    /// Wrap a [`tampines::components::Turbine`] — colour only, no rotation
    /// (see the module docs).
    ///
    /// Argument order matches the other visual components, so this is a
    /// drop-in for the former `TurbineVisual::new`.
    pub fn new_thermo(
        physics: Turbine,
        screen_position: Pos2,
        screen_vector: Vec2,
        min_temp: ThermodynamicTemperature,
        max_temp: ThermodynamicTemperature,
    ) -> Self {
        Self {
            state: TurbineVisualState::SteamThermo(physics),
            screen_position,
            screen_vector,
            flow_path: TurbineFlowPath::default(),
            simulation_time: Time::ZERO,
            min_temp,
            max_temp,
        }
    }

    /// A turbine from plain scalars, no physics model
    /// ([`TurbineArt::from_scalars`]).
    pub fn from_scalars(
        shaft_speed: AngularVelocity,
        steam_temperature: Option<ThermodynamicTemperature>,
        screen_position: Pos2,
        screen_vector: Vec2,
        min_temp: ThermodynamicTemperature,
        max_temp: ThermodynamicTemperature,
    ) -> Self {
        Self {
            state: TurbineVisualState::Scalars {
                shaft_speed,
                steam_temperature,
            },
            screen_position,
            screen_vector,
            flow_path: TurbineFlowPath::default(),
            simulation_time: Time::ZERO,
            min_temp,
            max_temp,
        }
    }

    /// Supply the application's simulation time. Builder-style.
    pub fn at_time(mut self, simulation_time: Time) -> Self {
        self.simulation_time = simulation_time;
        self
    }

    /// Choose single or double flow. Builder-style.
    pub fn with_flow_path(mut self, flow_path: TurbineFlowPath) -> Self {
        self.flow_path = flow_path;
        self
    }

    /// The shaft speed the art turns at: the generator's real `omega`, zero
    /// for a thermodynamic-only turbine (no shaft-speed model), or the
    /// caller's.
    pub fn shaft_speed(&self) -> AngularVelocity {
        match &self.state {
            TurbineVisualState::SteamGenerator(g) => g.get_omega(),
            TurbineVisualState::SteamThermo(_) => AngularVelocity::ZERO,
            TurbineVisualState::Scalars { shaft_speed, .. } => *shaft_speed,
        }
    }

    /// Casing colour source: the inlet steam temperature, when the variant
    /// knows one. The generator variant is an electromechanical model with no
    /// steam path, so it returns `None` and the casing renders neutral grey.
    pub fn casing_temperature(&self) -> Option<ThermodynamicTemperature> {
        match &self.state {
            TurbineVisualState::SteamGenerator(_) => None,
            TurbineVisualState::SteamThermo(t) => Some(t.inlet.get_temperature()),
            TurbineVisualState::Scalars {
                steam_temperature, ..
            } => *steam_temperature,
        }
    }

    /// The drawing of this turbine.
    pub fn art(&self) -> TurbineArt {
        TurbineArt::from_scalars(
            self.shaft_speed(),
            self.casing_temperature(),
            self.screen_position,
            self.screen_vector,
            self.min_temp,
            self.max_temp,
        )
        .with_flow_path(self.flow_path)
        .at_time(self.simulation_time)
    }

    /// Current rotor phase angle, `theta = omega * t` ([`TurbineArt::rotor_angle`]).
    pub fn rotor_angle(&self) -> Angle {
        self.art().rotor_angle()
    }

    /// Placeholder stage quality ([`TurbineArt::stage_quality`]).
    pub fn stage_quality(&self, stage_fraction: f32) -> f32 {
        self.art().stage_quality(stage_fraction)
    }

    /// Placeholder stage angles ([`TurbineArt::stage_angles`]).
    pub fn stage_angles(&self, stage_fraction: f32) -> StageAngles {
        self.art().stage_angles(stage_fraction)
    }

    /// Port positions ([`TurbineArt::ports`]).
    pub fn ports(
        screen_position: Pos2,
        screen_vector: Vec2,
        flow_path: TurbineFlowPath,
    ) -> TurbinePorts {
        TurbineArt::ports(screen_position, screen_vector, flow_path)
    }
}

impl Widget for TurbineVisual {
    /// Draws [`TurbineVisual::art`].
    fn ui(self, ui: &mut Ui) -> Response {
        self.art().ui(ui)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use uom::si::angle::radian;
    use uom::si::angular_velocity::radian_per_second;
    use uom::si::thermodynamic_temperature::kelvin;
    use uom::si::time::second;

    fn range() -> (ThermodynamicTemperature, ThermodynamicTemperature) {
        (
            ThermodynamicTemperature::new::<kelvin>(300.0),
            ThermodynamicTemperature::new::<kelvin>(600.0),
        )
    }

    fn generator_visual(omega_rad_s: f64) -> TurbineVisual {
        let mut g = ThreePhaseElectricGeneratorTurbine::new_250_megawatt_generator();
        g.set_omega(AngularVelocity::new::<radian_per_second>(omega_rad_s));
        let (min, max) = range();
        TurbineVisual::new_generator(g, Pos2::new(120.0, 80.0), Vec2::new(200.0, 100.0), min, max)
    }

    /// The generator's own `omega` turns the rotor: `theta = omega * t`.
    ///
    /// **Methodology:** set the generator's angular velocity to 10.0 rad/s,
    /// advance the application clock to 3.0 s, and compare against 30.0 rad.
    ///
    /// **Result (2026-08-04, re-checked after the 2026-11-05 split):** 30.0
    /// rad to within 1e-9 rad.
    #[test]
    fn rotor_angle_is_omega_times_time() {
        let v = generator_visual(10.0).at_time(Time::new::<second>(3.0));
        assert!((v.rotor_angle().get::<radian>() - 30.0).abs() < 1e-9);
        assert!(bugis::components::shapes_drawn_by(v) > 0);
    }

    /// The generator variant has no steam path, so it must report no casing
    /// temperature rather than inventing one.
    #[test]
    fn generator_variant_reports_no_casing_temperature() {
        assert!(generator_visual(5.0).casing_temperature().is_none());
        assert!(generator_visual(5.0).art().casing_temperature().is_none());
    }

    /// A fresh 250 MW preset starts from rest, so its rotor must not turn until
    /// the model is actually advanced.
    #[test]
    fn fresh_preset_starts_at_rest() {
        let (min, max) = range();
        let g = ThreePhaseElectricGeneratorTurbine::new_250_megawatt_generator();
        let v = TurbineVisual::new_generator(g, Pos2::ZERO, Vec2::new(200.0, 100.0), min, max)
            .at_time(Time::new::<second>(60.0));
        assert_eq!(v.rotor_angle().get::<radian>(), 0.0);
    }

    /// The scalar path and the builders reach the art.
    #[test]
    fn the_scalar_path_and_builders_reach_the_art() {
        let (min, max) = range();
        let v = TurbineVisual::from_scalars(
            AngularVelocity::new::<radian_per_second>(2.0),
            Some(ThermodynamicTemperature::new::<kelvin>(500.0)),
            Pos2::new(120.0, 80.0),
            Vec2::new(200.0, 100.0),
            min,
            max,
        )
        .with_flow_path(TurbineFlowPath::SingleFlow)
        .at_time(Time::new::<second>(4.0));
        let art = v.art();
        assert_eq!(art.flow_path, TurbineFlowPath::SingleFlow);
        assert!((v.rotor_angle().get::<radian>() - 8.0).abs() < 1e-9);
        assert_eq!(art.casing_temperature(), Some(ThermodynamicTemperature::new::<kelvin>(500.0)));
        assert_eq!(v.stage_quality(0.5), art.stage_quality(0.5));
        assert_eq!(v.stage_angles(0.5), art.stage_angles(0.5));
        assert_eq!(
            TurbineVisual::ports(v.screen_position, v.screen_vector, v.flow_path),
            TurbineArt::ports(v.screen_position, v.screen_vector, v.flow_path)
        );
        assert!(bugis::components::shapes_drawn_by(v) > 0);
    }
}
