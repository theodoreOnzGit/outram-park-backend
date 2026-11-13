//! Schematic steam generator: the physics wrapper.
//!
//! **Split 2026-11-05** (maintainer: "widget art can live in bugis, the
//! combination of logic and art can live inside the digital twin engine"):
//! the artwork, [`SteamGeneratorKind`] and [`SteamGeneratorScalars`] are
//! [`bugis::components::steam_generator`], re-exported here. This file holds
//! the widget that can wrap a live [`tampines::components::SteamGenerator`]
//! (its secondary-side temperature is read every repaint) and draws through
//! [`SteamGeneratorArt`].

pub use bugis::components::steam_generator::*;
use egui::{Pos2, Response, Ui, Vec2, Widget};
use tampines::components::SteamGenerator;
use uom::si::f64::ThermodynamicTemperature;

/// A [`tampines::components::SteamGenerator`] plus the scalars it does not
/// itself expose.
///
/// The physics component owns a secondary-side
/// [`tampines::hem::HemSteamCv`], whose `get_temperature()` is a real working
/// getter — that is where [`SteamGeneratorScalars::steam_temp`] comes from on
/// this path, so the steam space and the steam nozzle track the real state.
///
/// It exposes **no primary-side temperature** (its `step` is not yet
/// implemented), so the primary inlet/outlet, the feedwater temperature and
/// the level are supplied alongside it by the caller. They are not fabricated:
/// [`SteamGeneratorVisual::new`] starts them all at the secondary temperature,
/// which draws an isothermal generator until a caller supplies real values
/// with [`SteamGeneratorVisual::with_primary_temperatures`],
/// [`SteamGeneratorVisual::with_feedwater_temperature`] and
/// [`SteamGeneratorVisual::with_water_level`].
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct SteamGeneratorPhysicsState {
    /// The underlying TAMPINES component.
    pub physics: SteamGenerator,
    /// Primary coolant entering the generator (hot leg).
    pub primary_inlet_temp: ThermodynamicTemperature,
    /// Primary coolant leaving the generator (cold leg).
    pub primary_outlet_temp: ThermodynamicTemperature,
    /// Secondary feedwater entering the generator.
    pub feedwater_temp: ThermodynamicTemperature,
    /// Secondary water level, dimensionless in `[0, 1]`.
    pub water_level_frac: f32,
}

/// Where a [`SteamGeneratorVisual`] gets the state it renders.
///
/// Enum dispatch, not a trait object, per the workspace's mandatory "no trait
/// objects" Rust design rule.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum SteamGeneratorVisualState {
    /// Backed by caller-supplied scalars — a simulator that already holds
    /// these temperatures in its own plant model.
    Scalars(SteamGeneratorScalars),
    /// Backed by a live TAMPINES steam generator; the steam temperature is
    /// read from its secondary-side control volume every frame.
    Physics(SteamGeneratorPhysicsState),
}

impl SteamGeneratorVisualState {
    /// The scalars the artwork is actually drawn from.
    ///
    /// For [`Self::Physics`] this reads
    /// [`tampines::hem::HemSteamCv::get_temperature`] on the secondary side,
    /// so the physics path is live data rather than a snapshot taken at
    /// construction.
    pub fn resolve(&self) -> SteamGeneratorScalars {
        match self {
            Self::Scalars(s) => *s,
            Self::Physics(p) => SteamGeneratorScalars {
                primary_inlet_temp: p.primary_inlet_temp,
                primary_outlet_temp: p.primary_outlet_temp,
                feedwater_temp: p.feedwater_temp,
                steam_temp: p.physics.secondary_side.get_temperature(),
                water_level_frac: p.water_level_frac,
            },
        }
    }
}

/// Visual representation of a steam generator, in one of three
/// architectures: a [`SteamGeneratorArt`] drawn from the caller's scalars or
/// from a live TAMPINES steam generator ([`SteamGeneratorVisualState`]).
pub struct SteamGeneratorVisual {
    kind: SteamGeneratorKind,
    state: SteamGeneratorVisualState,
    screen_position: Pos2,
    screen_vector: Vec2,
    min_temp: ThermodynamicTemperature,
    max_temp: ThermodynamicTemperature,
    show_labels: bool,
}

impl SteamGeneratorVisual {
    /// Wrap a [`SteamGenerator`] with the given screen geometry and
    /// colour-mapping temperature range.
    ///
    /// This is the **physics-backed** path, and it is unchanged from the
    /// original widget: the secondary side is coloured by
    /// [`tampines::hem::HemSteamCv::get_temperature`], read live every repaint.
    ///
    /// The component exposes nothing about the primary side, so the primary
    /// inlet/outlet and feedwater temperatures start equal to the secondary
    /// temperature — an isothermal generator, which is visibly uninformative
    /// rather than quietly wrong — and the level starts at 0.5. Supply real
    /// values with [`Self::with_primary_temperatures`],
    /// [`Self::with_feedwater_temperature`] and [`Self::with_water_level`].
    ///
    /// Defaults to [`SteamGeneratorKind::VerticalUTube`]; change it with
    /// [`Self::with_kind`].
    ///
    /// `screen_position` is the **centre** of the box the artwork is
    /// letterboxed into, and `screen_vector` its size in points.
    pub fn new(
        physics: SteamGenerator,
        screen_position: Pos2,
        screen_vector: Vec2,
        min_temp: ThermodynamicTemperature,
        max_temp: ThermodynamicTemperature,
    ) -> Self {
        let secondary = physics.secondary_side.get_temperature();
        Self {
            kind: SteamGeneratorKind::VerticalUTube,
            state: SteamGeneratorVisualState::Physics(SteamGeneratorPhysicsState {
                physics,
                primary_inlet_temp: secondary,
                primary_outlet_temp: secondary,
                feedwater_temp: secondary,
                water_level_frac: 0.5,
            }),
            screen_position,
            screen_vector,
            min_temp,
            max_temp,
            show_labels: true,
        }
    }

    /// Build a generator visual from the caller's own scalar plant state
    /// ([`SteamGeneratorArt::from_scalars`]).
    pub fn from_scalars(
        kind: SteamGeneratorKind,
        screen_position: Pos2,
        screen_vector: Vec2,
        min_temp: ThermodynamicTemperature,
        max_temp: ThermodynamicTemperature,
        scalars: SteamGeneratorScalars,
    ) -> Self {
        Self {
            kind,
            state: SteamGeneratorVisualState::Scalars(scalars),
            screen_position,
            screen_vector,
            min_temp,
            max_temp,
            show_labels: true,
        }
    }

    /// Draw a different architecture with the same state.
    pub fn with_kind(mut self, kind: SteamGeneratorKind) -> Self {
        self.kind = kind;
        self
    }

    /// Supply the primary inlet and outlet temperatures. On the physics path
    /// these replace the isothermal start; on the scalar path they overwrite
    /// the supplied values.
    pub fn with_primary_temperatures(
        mut self,
        inlet: ThermodynamicTemperature,
        outlet: ThermodynamicTemperature,
    ) -> Self {
        match &mut self.state {
            SteamGeneratorVisualState::Scalars(s) => {
                s.primary_inlet_temp = inlet;
                s.primary_outlet_temp = outlet;
            }
            SteamGeneratorVisualState::Physics(p) => {
                p.primary_inlet_temp = inlet;
                p.primary_outlet_temp = outlet;
            }
        }
        self
    }

    /// Supply the feedwater temperature.
    pub fn with_feedwater_temperature(mut self, feedwater: ThermodynamicTemperature) -> Self {
        match &mut self.state {
            SteamGeneratorVisualState::Scalars(s) => s.feedwater_temp = feedwater,
            SteamGeneratorVisualState::Physics(p) => p.feedwater_temp = feedwater,
        }
        self
    }

    /// Supply the secondary water level, dimensionless in `[0, 1]`.
    pub fn with_water_level(mut self, frac: f32) -> Self {
        match &mut self.state {
            SteamGeneratorVisualState::Scalars(s) => s.water_level_frac = frac,
            SteamGeneratorVisualState::Physics(p) => p.water_level_frac = frac,
        }
        self
    }

    /// Turn the internal component labels off — for thumbnails.
    pub fn without_labels(mut self) -> Self {
        self.show_labels = false;
        self
    }

    /// Which architecture this visual draws.
    pub fn kind(&self) -> SteamGeneratorKind {
        self.kind
    }

    /// On-screen size of the box the artwork letterboxes into, in points.
    pub fn size(&self) -> Vec2 {
        self.screen_vector
    }

    /// The scalars the artwork is drawn from (live on the physics path).
    pub fn scalars(&self) -> SteamGeneratorScalars {
        self.state.resolve()
    }

    /// The drawing of this generator, from [`Self::scalars`].
    pub fn art(&self) -> SteamGeneratorArt {
        let art = SteamGeneratorArt::from_scalars(
            self.kind,
            self.screen_position,
            self.screen_vector,
            self.min_temp,
            self.max_temp,
            self.scalars(),
        );
        if self.show_labels {
            art
        } else {
            art.without_labels()
        }
    }
}

impl Widget for SteamGeneratorVisual {
    /// Draws [`SteamGeneratorVisual::art`].
    fn ui(self, ui: &mut Ui) -> Response {
        self.art().ui(ui)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use outram_park_fork_dwsim_libs::heat_exchanger::lmtd::FlowArrangement;
    use tampines::components::HeatExchanger;
    use tampines::hem::HemSteamCv;
    use uom::si::area::square_meter;
    use uom::si::available_energy::joule_per_kilogram;
    use uom::si::f64::{Area, AvailableEnergy, HeatTransfer, Pressure, Volume};
    use uom::si::heat_transfer::watt_per_square_meter_kelvin;
    use uom::si::pressure::megapascal;
    use uom::si::thermodynamic_temperature::degree_celsius;
    use uom::si::volume::cubic_meter;

    fn degc(v: f64) -> ThermodynamicTemperature {
        ThermodynamicTemperature::new::<degree_celsius>(v)
    }

    /// The physics-backed path must read the **live** secondary-side
    /// temperature, not a value captured at construction.
    ///
    /// **Methodology.** Build a `tampines::components::SteamGenerator` around
    /// a secondary-side `HemSteamCv` flashed from 6.0 MPa and
    /// 2 780 kJ/kg, wrap it with `SteamGeneratorVisual::new` (the original
    /// constructor, whose signature is preserved), and require
    /// `scalars().steam_temp` to equal
    /// `secondary_side.get_temperature()` exactly. Then replace the control
    /// volume with a 3.0 MPa / 2 900 kJ/kg state and require the visual to
    /// follow it.
    ///
    /// **Result (2026-08-06):** both states matched their getter bit for bit —
    /// 275.586 degC at 6.0 MPa / 2 780 kJ/kg, which is just inside the
    /// two-phase dome (`h_g` is about 2 784 kJ/kg there) and so returns the
    /// saturation temperature, and 264.723 degC at 3.0 MPa / 2 900 kJ/kg,
    /// which is superheated (`h_g` about 2 804 kJ/kg). Interpretation: the
    /// physics path is live
    /// data, so a widget that silently ignored its control volume would fail
    /// here.
    #[test]
    fn the_physics_path_reads_the_live_secondary_temperature() {
        let volume = Volume::new::<cubic_meter>(1.0);
        let steam = HemSteamCv::new_from_ph(
            Pressure::new::<megapascal>(6.0),
            AvailableEnergy::new::<joule_per_kilogram>(2_780_000.0),
            volume,
        );
        let exchanger = HeatExchanger::new(
            FlowArrangement::CounterCurrent,
            Area::new::<square_meter>(800.0),
            HeatTransfer::new::<watt_per_square_meter_kelvin>(1500.0),
        );
        let generator = SteamGenerator::new(exchanger, steam);
        let expected = generator.secondary_side.get_temperature();

        let visual = SteamGeneratorVisual::new(
            generator,
            Pos2::new(0.0, 0.0),
            Vec2::new(120.0, 240.0),
            degc(100.0),
            degc(400.0),
        );
        assert_eq!(visual.scalars().steam_temp, expected);
        println!(
            "6.0 MPa / 2780 kJ/kg -> {:.3} degC",
            expected.get::<degree_celsius>()
        );

        // A different state must give a different drawn temperature.
        let hotter = HemSteamCv::new_from_ph(
            Pressure::new::<megapascal>(3.0),
            AvailableEnergy::new::<joule_per_kilogram>(2_900_000.0),
            volume,
        );
        let generator = SteamGenerator::new(exchanger, hotter);
        let expected_hot = generator.secondary_side.get_temperature();
        let visual = SteamGeneratorVisual::new(
            generator,
            Pos2::new(0.0, 0.0),
            Vec2::new(120.0, 240.0),
            degc(100.0),
            degc(400.0),
        );
        assert_eq!(visual.scalars().steam_temp, expected_hot);
        println!(
            "3.0 MPa / 2900 kJ/kg -> {:.3} degC",
            expected_hot.get::<degree_celsius>()
        );
        assert!(expected_hot != expected);
    }

    /// The physics constructor must not fabricate a primary side it cannot
    /// see: it starts isothermal at the secondary temperature, and the
    /// builders replace those values.
    #[test]
    fn the_physics_constructor_starts_isothermal_until_told_otherwise() {
        let steam = HemSteamCv::new_from_ph(
            Pressure::new::<megapascal>(6.0),
            AvailableEnergy::new::<joule_per_kilogram>(2_780_000.0),
            Volume::new::<cubic_meter>(1.0),
        );
        let exchanger = HeatExchanger::new(
            FlowArrangement::CounterCurrent,
            Area::new::<square_meter>(800.0),
            HeatTransfer::new::<watt_per_square_meter_kelvin>(1500.0),
        );
        let generator = SteamGenerator::new(exchanger, steam);
        let secondary = generator.secondary_side.get_temperature();

        let visual = SteamGeneratorVisual::new(
            generator,
            Pos2::ZERO,
            Vec2::new(120.0, 240.0),
            degc(100.0),
            degc(400.0),
        );
        let drawn = visual.scalars();
        assert_eq!(drawn.primary_inlet_temp, secondary);
        assert_eq!(drawn.primary_outlet_temp, secondary);
        assert_eq!(drawn.feedwater_temp, secondary);
        assert_eq!(visual.kind(), SteamGeneratorKind::VerticalUTube);

        let visual = visual
            .with_primary_temperatures(degc(320.0), degc(288.0))
            .with_feedwater_temperature(degc(226.0))
            .with_kind(SteamGeneratorKind::HelicalCoil);
        let drawn = visual.scalars();
        assert_eq!(drawn.primary_inlet_temp, degc(320.0));
        assert_eq!(drawn.primary_outlet_temp, degc(288.0));
        assert_eq!(drawn.feedwater_temp, degc(226.0));
        assert_eq!(
            drawn.steam_temp, secondary,
            "the live steam path must survive"
        );
        assert_eq!(visual.kind(), SteamGeneratorKind::HelicalCoil);
    }

    /// The scalar path hands the caller's state to the art, and both paths
    /// draw.
    #[test]
    fn the_scalar_path_reaches_the_art_and_draws() {
        let s = SteamGeneratorScalars {
            primary_inlet_temp: degc(320.0),
            primary_outlet_temp: degc(288.0),
            feedwater_temp: degc(226.0),
            steam_temp: degc(285.0),
            water_level_frac: 0.62,
        };
        let v = SteamGeneratorVisual::from_scalars(
            SteamGeneratorKind::VerticalUTube,
            Pos2::new(120.0, 220.0),
            Vec2::new(200.0, 400.0),
            degc(100.0),
            degc(400.0),
            s,
        )
        .with_water_level(0.5)
        .without_labels();
        assert_eq!(v.art().scalars(), SteamGeneratorScalars { water_level_frac: 0.5, ..s });
        assert_eq!(v.size(), Vec2::new(200.0, 400.0));
        assert!(bugis::components::shapes_drawn_by(v) > 0);
    }
}
