//! Schematic cooling tower: the physics wrapper.
//!
//! **Split 2026-11-05** (maintainer: "widget art can live in bugis, the
//! combination of logic and art can live inside the digital twin engine"):
//! the artwork, its kinds and the psychrometric helpers
//! ([`approach_to_wet_bulb`], [`cooling_range`], [`plume_opacity`], ...) are
//! [`bugis::components::cooling_tower`], re-exported here. This file holds the
//! widget that wraps a [`tampines::components::CoolingTower`] or the caller's
//! CoolProp-resolved [`CoolingTowerScalars`], and draws through
//! [`CoolingTowerArt`].
//!
//! [`tampines::components::CoolingTower`] holds a **real** ambient air inlet
//! state, a real circulating-water inlet temperature and flow rate, and a
//! **target** approach. Its `evaluate` is not implemented, so there is no
//! water outlet temperature and no exit air state: [`CoolingTowerVisual::new`]
//! draws [`CoolingTowerArt::inlet_only`] — the basin neutral, no approach
//! (only the target, labelled as a target) and **no plume**. See the bugis
//! module for the full description.

pub use bugis::components::cooling_tower::*;
use egui::{Pos2, Response, Ui, Vec2, Widget};
use tampines::components::CoolingTower;
use tampines::humid_air::HumidAirState;
use uom::si::f64::{Angle, AngularVelocity, TemperatureInterval, ThermodynamicTemperature, Time, VolumeRate};

/// The psychrometric readout the art draws, from a CoolProp-resolved
/// humid-air state.
pub fn air_readout(air: &HumidAirState) -> AirReadout {
    AirReadout {
        t_dry_bulb: air.t_dry_bulb,
        relative_humidity: air.relative_humidity,
        humidity_ratio: Some(air.humidity_ratio),
    }
}

/// Scalar state of a cooling tower, as the caller's own model holds it.
///
/// Every field is **real state the caller already has** — see
/// [`crate::components::PipeVisual::from_scalars`] for why this narrower
/// interface exists. The two air states are full
/// [`tampines::humid_air::HumidAirState`] values, resolved by the caller
/// through the CoolProp-backed psychrometrics
/// (`tampines::humid_air::state_from_t_p_r` and friends); the widget only reads
/// properties off them ([`air_readout`]).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct CoolingTowerScalars {
    /// Ambient air drawn in at the base.
    pub air_inlet: HumidAirState,
    /// Air leaving the tower, past the drift eliminators. Its **relative
    /// humidity drives the plume** through [`plume_opacity`].
    pub air_outlet: HumidAirState,
    /// Wet-bulb temperature of the entering air, supplied by the caller
    /// because [`HumidAirState`] carries no wet-bulb field.
    pub inlet_wet_bulb: ThermodynamicTemperature,
    /// Warm water returning from the plant to the distribution deck.
    pub water_inlet_temp: ThermodynamicTemperature,
    /// Cooled water leaving the basin.
    pub water_outlet_temp: ThermodynamicTemperature,
    /// Circulating-water volumetric flow rate. A **zero** flow draws no spray
    /// and no rain.
    pub water_flow_rate: VolumeRate,
}

impl CoolingTowerScalars {
    /// The numbers the art draws from this state.
    pub fn to_art(&self) -> CoolingTowerArtScalars {
        CoolingTowerArtScalars {
            air_inlet: air_readout(&self.air_inlet),
            air_outlet: air_readout(&self.air_outlet),
            inlet_wet_bulb: self.inlet_wet_bulb,
            water_inlet_temp: self.water_inlet_temp,
            water_outlet_temp: self.water_outlet_temp,
            water_flow_rate: self.water_flow_rate,
        }
    }
}

/// Where a [`CoolingTowerVisual`] gets the state it renders.
///
/// Enum dispatch, not a trait object, per the workspace's mandatory "no trait
/// objects" Rust design rule.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum CoolingTowerVisualState {
    /// Backed by a [`tampines::components::CoolingTower`]: its air inlet
    /// state, water inlet temperature and water flow rate are real and are
    /// drawn; there is no outlet side.
    Physics(CoolingTower),
    /// Backed by caller-supplied scalars from the caller's own plant model.
    Scalars(CoolingTowerScalars),
}

/// Visual representation of a cooling tower, in one of two draught
/// architectures: a [`CoolingTowerArt`] built from a
/// [`tampines::components::CoolingTower`] ([`Self::new`], whose signature is
/// preserved) or from the caller's own psychrometric plant state
/// ([`Self::from_scalars`]).
pub struct CoolingTowerVisual {
    state: CoolingTowerVisualState,
    art: CoolingTowerArt,
}

impl CoolingTowerVisual {
    /// Wrap a [`CoolingTower`] with the given screen geometry and
    /// colour-mapping temperature range.
    ///
    /// **Signature preserved** from the original placeholder widget. Draws
    /// [`CoolingTowerArt::inlet_only`] from the component's real inlet side;
    /// nothing is fabricated for the outlet side it does not have.
    ///
    /// Defaults to [`CoolingTowerKind::NaturalDraught`]; change it with
    /// [`Self::with_kind`].
    pub fn new(
        physics: CoolingTower,
        screen_position: Pos2,
        screen_vector: Vec2,
        min_temp: ThermodynamicTemperature,
        max_temp: ThermodynamicTemperature,
    ) -> Self {
        let inlet = CoolingTowerInlet {
            air_inlet: air_readout(&physics.air_inlet),
            water_inlet_temperature: physics.water_inlet_temperature,
            water_flow_rate: physics.water_flow_rate,
            target_approach: physics.target_approach,
        };
        Self {
            state: CoolingTowerVisualState::Physics(physics),
            art: CoolingTowerArt::inlet_only(inlet, screen_position, screen_vector, min_temp, max_temp),
        }
    }

    /// Build a cooling-tower visual from the caller's own psychrometric plant
    /// state ([`CoolingTowerArt::from_scalars`]).
    pub fn from_scalars(
        kind: CoolingTowerKind,
        screen_position: Pos2,
        screen_vector: Vec2,
        min_temp: ThermodynamicTemperature,
        max_temp: ThermodynamicTemperature,
        scalars: CoolingTowerScalars,
    ) -> Self {
        Self {
            state: CoolingTowerVisualState::Scalars(scalars),
            art: CoolingTowerArt::from_scalars(
                kind,
                screen_position,
                screen_vector,
                min_temp,
                max_temp,
                scalars.to_art(),
            ),
        }
    }

    /// Draw a different draught architecture with the same state.
    pub fn with_kind(mut self, kind: CoolingTowerKind) -> Self {
        self.art = self.art.with_kind(kind);
        self
    }

    /// Set the fan shaft speed ([`CoolingTowerArt::with_fan_speed`]).
    pub fn with_fan_speed(mut self, fan_speed: AngularVelocity) -> Self {
        self.art = self.art.with_fan_speed(fan_speed);
        self
    }

    /// Supply the **application's** simulation time, which sets the fan phase
    /// `theta = omega * t` ([`CoolingTowerArt::at_time`]).
    pub fn at_time(mut self, simulation_time: Time) -> Self {
        self.art = self.art.at_time(simulation_time);
        self
    }

    /// Turn the internal component labels and readouts off — for thumbnails.
    pub fn without_labels(mut self) -> Self {
        self.art = self.art.without_labels();
        self
    }

    /// Which draught architecture this visual draws.
    pub fn kind(&self) -> CoolingTowerKind {
        self.art.kind()
    }

    /// On-screen size of the box the artwork letterboxes into, in points.
    pub fn size(&self) -> Vec2 {
        self.art.size()
    }

    /// Where this visual gets its state.
    pub fn state(&self) -> &CoolingTowerVisualState {
        &self.state
    }

    /// The scalar state the artwork is drawn from, or `None` on the
    /// physics-backed path.
    pub fn scalars(&self) -> Option<CoolingTowerScalars> {
        match self.state {
            CoolingTowerVisualState::Physics(_) => None,
            CoolingTowerVisualState::Scalars(s) => Some(s),
        }
    }

    /// The wrapped component, or `None` on the scalar path.
    pub fn physics(&self) -> Option<CoolingTower> {
        match self.state {
            CoolingTowerVisualState::Physics(c) => Some(c),
            CoolingTowerVisualState::Scalars(_) => None,
        }
    }

    /// Current fan phase angle, `theta = omega * t`, or `None` for a kind with
    /// no fan ([`CoolingTowerArt::fan_angle`]).
    pub fn fan_angle(&self) -> Option<Angle> {
        self.art.fan_angle()
    }

    /// The **achieved** approach to the entering wet-bulb temperature, or
    /// `None` on the physics-backed path ([`CoolingTowerArt::approach`]).
    pub fn approach(&self) -> Option<TemperatureInterval> {
        self.art.approach()
    }

    /// The wrapped component's **target** approach, or `None` on the scalar
    /// path. Never used to colour anything.
    pub fn target_approach(&self) -> Option<TemperatureInterval> {
        self.art.target_approach()
    }

    /// The cooling range across the tower, or `None` on the physics-backed
    /// path ([`CoolingTowerArt::cooling_range`]).
    pub fn cooling_range(&self) -> Option<TemperatureInterval> {
        self.art.cooling_range()
    }

    /// The drawing this widget paints.
    pub fn art(&self) -> &CoolingTowerArt {
        &self.art
    }
}

impl Widget for CoolingTowerVisual {
    /// Draws the [`CoolingTowerArt`].
    fn ui(self, ui: &mut Ui) -> Response {
        self.art.ui(ui)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tampines::humid_air::state_from_t_p_r;
    use uom::si::angular_velocity::radian_per_second;
    use uom::si::f64::{Pressure, Ratio};
    use uom::si::pressure::kilopascal;
    use uom::si::ratio::ratio;
    use uom::si::temperature_interval::kelvin as kelvin_interval;
    use uom::si::thermodynamic_temperature::degree_celsius;
    use uom::si::time::second;
    use uom::si::volume_rate::cubic_meter_per_second;

    fn degc(v: f64) -> ThermodynamicTemperature {
        ThermodynamicTemperature::new::<degree_celsius>(v)
    }

    /// A CoolProp-resolved humid-air state at 101.325 kPa.
    fn air(t_degc: f64, rh: f64) -> HumidAirState {
        state_from_t_p_r(
            degc(t_degc),
            Pressure::new::<kilopascal>(101.325),
            Ratio::new::<ratio>(rh),
        )
        .expect("humid-air state must resolve on the liquid-water branch")
    }

    /// Illustrative tower state: 32 degC / 60 % ambient air in, saturated air
    /// out at 38 degC, water cooled 40 -> 30 degC against a 25.4 degC wet
    /// bulb. Demonstration values only, from no design.
    fn scalars() -> CoolingTowerScalars {
        CoolingTowerScalars {
            air_inlet: air(32.0, 0.60),
            air_outlet: air(38.0, 1.00),
            inlet_wet_bulb: degc(25.4),
            water_inlet_temp: degc(40.0),
            water_outlet_temp: degc(30.0),
            water_flow_rate: VolumeRate::new::<cubic_meter_per_second>(12.0),
        }
    }

    fn physics_tower() -> CoolingTower {
        CoolingTower::new(
            air(32.0, 0.60),
            degc(40.0),
            VolumeRate::new::<cubic_meter_per_second>(12.0),
            TemperatureInterval::new::<kelvin_interval>(4.0),
        )
    }

    /// The physics-backed path must draw what the component really holds and
    /// **nothing** downstream of it.
    ///
    /// **Methodology.** `tampines::components::CoolingTower` holds a real
    /// CoolProp-resolved air inlet state, a real water inlet temperature and
    /// flow rate, and a *target* approach; its `evaluate` returns
    /// `NotYetImplemented`. Wrap one with [`CoolingTowerVisual::new`] and
    /// require: the inlet side to reach the art as [`CoolingTowerInlet`]; the
    /// achieved approach and the range to be `None` while the target approach
    /// is reported separately. (The drawing-state half — no plume, neutral
    /// basin — is bugis's `the_inlet_only_path_draws_no_plume_and_no_approach`,
    /// since the resolved drawing state is private to the art.)
    ///
    /// **Result (2026-08-12, re-checked after the 2026-11-05 split):** the
    /// wrapped 32 degC / 60 % RH inlet air and 40 degC / 12 m^3/s water
    /// survived unchanged; `approach()` and `cooling_range()` were `None`
    /// while `target_approach()` returned 4.0 K.
    #[test]
    fn the_physics_path_draws_no_plume_and_no_approach() {
        let tower = physics_tower();
        let visual = CoolingTowerVisual::new(
            tower,
            Pos2::new(200.0, 250.0),
            Vec2::new(300.0, 400.0),
            degc(0.0),
            degc(100.0),
        );
        match visual.art().state() {
            CoolingTowerArtState::Inlet(inlet) => {
                assert_eq!(inlet.air_inlet, air_readout(&tower.air_inlet));
                assert_eq!(inlet.water_inlet_temperature, tower.water_inlet_temperature);
                assert_eq!(inlet.water_flow_rate, tower.water_flow_rate);
            }
            other => panic!("the physics path must draw the inlet side only, got {other:?}"),
        }
        assert_eq!(visual.approach(), None);
        assert_eq!(visual.cooling_range(), None);
        assert_eq!(
            visual.target_approach(),
            Some(TemperatureInterval::new::<kelvin_interval>(4.0))
        );
        assert!(visual.scalars().is_none());
        assert_eq!(visual.physics(), Some(tower));
        assert!(matches!(visual.state(), CoolingTowerVisualState::Physics(_)));
        assert_eq!(visual.kind(), CoolingTowerKind::NaturalDraught);
        assert_eq!(visual.size(), Vec2::new(300.0, 400.0));
        assert!(bugis::components::shapes_drawn_by(visual) > 0);
    }

    /// The scalar path passes the CoolProp states to the art as readouts, and
    /// the builders reach the art.
    #[test]
    fn the_scalar_path_and_builders_reach_the_art() {
        let v = CoolingTowerVisual::from_scalars(
            CoolingTowerKind::NaturalDraught,
            Pos2::new(200.0, 250.0),
            Vec2::new(300.0, 400.0),
            degc(0.0),
            degc(100.0),
            scalars(),
        )
        .with_kind(CoolingTowerKind::InducedDraught)
        .with_fan_speed(AngularVelocity::new::<radian_per_second>(3.0))
        .at_time(Time::new::<second>(10.0))
        .without_labels();
        assert_eq!(v.scalars(), Some(scalars()));
        assert_eq!(v.art().scalars(), Some(scalars().to_art()));
        assert!(v.physics().is_none());
        assert!(v.target_approach().is_none());
        assert_eq!(v.kind(), CoolingTowerKind::InducedDraught);
        assert!((v.fan_angle().unwrap().get::<uom::si::angle::radian>() - 30.0).abs() < 1e-9);
        assert!((v.approach().unwrap().get::<kelvin_interval>() - 4.6).abs() < 1e-9);
        assert!((v.cooling_range().unwrap().get::<kelvin_interval>() - 10.0).abs() < 1e-9);
        assert!(bugis::components::shapes_drawn_by(v) > 0);
    }

    /// The supplied air states must really be CoolProp-resolved psychrometry,
    /// not free-floating numbers.
    ///
    /// **Methodology.** `tampines::humid_air::state_from_t_p_r` wraps
    /// `outram_park_fork_coolprop::humid_air::ha_props` (ASHRAE RP-1485).
    /// Resolve 32 degC / 60 % RH at 101.325 kPa and require the state to come
    /// back with the requested dry-bulb and relative humidity, a physically
    /// sensible humidity ratio, and a saturated state at the same temperature
    /// to carry strictly more water and more enthalpy.
    ///
    /// **Result (2026-08-12):** 32.000 degC at RH 0.600 gave W = 0.018121
    /// kg/kg and h = 78.56 kJ/kg dry air; the saturated state at the same
    /// temperature gave W = 0.030800 kg/kg and h = 110.98 kJ/kg dry air.
    /// Interpretation: the widget is reading a real psychrometric state, so
    /// the plume and the air colours are anchored to the CoolProp port rather
    /// than to display constants.
    #[test]
    fn the_air_states_come_from_the_coolprop_psychrometrics() {
        let humid = air(32.0, 0.60);
        assert!((humid.t_dry_bulb.get::<degree_celsius>() - 32.0).abs() < 1e-6);
        assert!((humid.relative_humidity.get::<ratio>() - 0.60).abs() < 1e-6);
        assert!(humid.humidity_ratio.get::<ratio>() > 0.0);

        let saturated = air(32.0, 1.00);
        println!(
            "32 degC: W = {:.6} at RH 0.60, W = {:.6} at RH 1.00",
            humid.humidity_ratio.get::<ratio>(),
            saturated.humidity_ratio.get::<ratio>()
        );
        println!(
            "enthalpy {:.2} -> {:.2} kJ/kg dry air",
            humid
                .enthalpy
                .get::<uom::si::available_energy::joule_per_kilogram>()
                / 1000.0,
            saturated
                .enthalpy
                .get::<uom::si::available_energy::joule_per_kilogram>()
                / 1000.0
        );
        assert!(
            saturated.humidity_ratio > humid.humidity_ratio,
            "saturated air must carry more water"
        );
        assert!(
            saturated.enthalpy > humid.enthalpy,
            "saturated air must carry more enthalpy"
        );
        assert_eq!(plume_opacity(saturated.relative_humidity), 1.0);
        assert_eq!(plume_opacity(humid.relative_humidity), 0.0);
    }
}
