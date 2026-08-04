//! Plant schematic panel, built **entirely on the engine's reusable visual
//! widgets** ([`outram_park_digital_twin_engine::components`]) -- not on
//! locally re-derived widget boilerplate (the mistake `fhr_sim_v2` made).
//!
//! Each frame this rebuilds the engine widgets from the current
//! [`HtgrSnapshot`] scalars and lays them out left-to-right as a
//! helium-primary / steam-secondary HTGR:
//!
//! ```text
//!   [Reactor] --hot He--> [IHX] ==heat==> [Steam Gen] --steam--> [Turbine] --> [Condenser]
//!        ^                   |                  ^                                    |
//!        +---- circulator ---+                  +------- feed pump / valve ----------+
//! ```
//!
//! Widgets used (all from the engine crate): [`ReactorVesselVisual`],
//! [`HeatExchangerVisual`], [`SteamGeneratorVisual`], [`TurbineVisual`],
//! [`CondenserVisual`], [`PumpVisual`], [`ValveVisual`],
//! [`InstrumentationVisual`], and [`PipeVisual`] for every connector run.
//!
//! ## Connectors are real pipe widgets, with flow tracers
//!
//! The connector runs are [`PipeVisual`]s built through
//! [`PipeVisual::from_scalars`], not raw `painter` lines. A full
//! `tampines::components::Pipe` would need a `SinglePhaseFluidArray` or
//! `CompressibleFluidArray` per connector, which is far more machinery than a
//! schematic line needs; the scalar path takes this plant's own real
//! temperature, mass flow, and residence time instead.
//!
//! Each run carries a [`TracerTrain`] whose marks travel at `1/residence_time`
//! of the run per second, so the animation is a direct readout of the physical
//! transport time: raise the helium flow and the primary tracers visibly speed
//! up. The trains live in [`SchematicTracers`], owned by the app and advanced
//! once per frame -- widgets are rebuilt every repaint, so a train owned by a
//! widget would reset its phase each frame.

use egui::{pos2, Color32, Pos2, Stroke, Ui, Vec2};

use outram_park_digital_twin_engine::animation::TracerTrain;
use outram_park_digital_twin_engine::components::{
    CondenserVisual, HeatExchangerVisual, InstrumentationVisual, PipeScalars, PipeVisual,
    PumpVisual, ReactorVesselVisual, SteamGeneratorVisual, TurbineVisual, ValveVisual,
};

use nee_soon::NordheimFuchsExactTimestepper;
use outram_park_fork_dwsim_libs::heat_exchanger::lmtd::FlowArrangement;
use outram_park_fork_dwsim_libs::pump::modes::PumpSpecification;
use outram_park_fork_dwsim_libs::valve::iec_60534::{OpeningCharacteristic, ValveFlowCoefficient};
use tampines::components::{Condenser, HeatExchanger, Pump, SteamGenerator, Turbine, Valve};
use tampines::hem::HemSteamCv;
use uom::si::area::square_meter;
use uom::si::available_energy::joule_per_kilogram;
use uom::si::f64::{
    Area, AvailableEnergy, HeatTransfer, MassRate, Power, Pressure, Ratio,
    ThermodynamicTemperature, Time, Volume,
};
use uom::si::heat_transfer::watt_per_square_meter_kelvin;
use uom::si::mass_rate::kilogram_per_second;
use uom::si::power::watt;
use uom::si::pressure::megapascal;
use uom::si::ratio::{percent, ratio};
use uom::si::thermodynamic_temperature::kelvin;
use uom::si::time::second;
use uom::si::volume::cubic_meter;

use crate::app::state::HtgrSnapshot;

/// Colour-map floor for the helium runs \[K\] -- below the coldest helium the
/// loop reaches, so the return leg reads as genuinely cold rather than clipped.
const HELIUM_COLOUR_MIN_K: f64 = 500.0;
/// Colour-map ceiling for the helium runs \[K\].
const HELIUM_COLOUR_MAX_K: f64 = 1200.0;
/// Colour-map floor for the water/steam runs \[K\].
const STEAM_COLOUR_MIN_K: f64 = 300.0;
/// Colour-map ceiling for the water/steam runs \[K\].
const STEAM_COLOUR_MAX_K: f64 = 900.0;

/// Number of tracer marks drawn on each connector run.
const TRACER_MARKS: usize = 5;

/// Flow-tracer state for the schematic's connector runs, owned by the app and
/// advanced once per frame.
///
/// Two trains, one per loop: every primary run shares the helium train and
/// every secondary run shares the steam train, so marks stay in step around
/// each loop. See [`crate::app::schematic`]'s module docs for why these are
/// app-owned rather than widget-owned.
#[derive(Debug, Clone, Copy)]
pub struct SchematicTracers {
    /// Marks on the helium primary runs.
    pub primary: TracerTrain,
    /// Marks on the water/steam secondary runs.
    pub secondary: TracerTrain,
}

impl SchematicTracers {
    /// Fresh, stagnant trains.
    pub fn new() -> Self {
        Self {
            primary: TracerTrain::new(TRACER_MARKS),
            secondary: TracerTrain::new(TRACER_MARKS),
        }
    }

    /// Advance both trains by one animation frame of `dt`, using the loop
    /// residence times and mass flows the physics thread published.
    ///
    /// Because the marks move at `1/residence_time` of a run per second, the
    /// on-screen speed is the real transport speed: at zero flow the residence
    /// time is unbounded and the trains freeze.
    pub fn advance(&mut self, dt: Time, snapshot: &HtgrSnapshot) {
        self.primary.advance(
            dt,
            Time::new::<second>(snapshot.helium_residence_time_s),
            MassRate::new::<kilogram_per_second>(snapshot.helium_mass_flow_kg_per_s),
        );
        self.secondary.advance(
            dt,
            Time::new::<second>(snapshot.secondary_residence_time_s),
            MassRate::new::<kilogram_per_second>(snapshot.secondary_mass_flow_kg_per_s),
        );
    }
}

impl Default for SchematicTracers {
    fn default() -> Self {
        Self::new()
    }
}

/// Build one connector run as a scalar-backed [`PipeVisual`] carrying `tracer`.
fn run(
    from: Pos2,
    to: Pos2,
    temperature_k: f64,
    mass_flow_kg_per_s: f64,
    residence_time_s: f64,
    colour_min_k: f64,
    colour_max_k: f64,
    tracer: TracerTrain,
) -> PipeVisual {
    PipeVisual::from_scalars(
        PipeScalars {
            temperature: ThermodynamicTemperature::new::<kelvin>(temperature_k),
            mass_flow: MassRate::new::<kilogram_per_second>(mass_flow_kg_per_s),
            residence_time: Time::new::<second>(residence_time_s),
        },
        from,
        to - from,
        ThermodynamicTemperature::new::<kelvin>(colour_min_k),
        ThermodynamicTemperature::new::<kelvin>(colour_max_k),
    )
    .with_tracer(tracer)
}

/// Draw the whole schematic from `snapshot` into `ui`, animating the connector
/// runs with the app-owned `tracers`.
pub fn draw_schematic(ui: &mut Ui, snapshot: &HtgrSnapshot, tracers: &SchematicTracers) {
    // Reserve a fixed canvas so the absolute widget positions have room; the
    // engine widgets paint at their own `screen_position`, independent of the
    // egui layout cursor.
    let canvas = Vec2::new(1040.0, 460.0);
    let (canvas_rect, _response) = ui.allocate_exact_size(canvas, egui::Sense::hover());
    let origin = canvas_rect.min.to_vec2();

    // Helper to shift a schematic-local point into canvas coordinates.
    let at = |x: f32, y: f32| -> Pos2 { pos2(x, y) + origin };

    // --- Connector runs: real PipeVisual widgets, coloured by the fluid
    //     temperature they actually carry and animated by the real flow ---
    let he_flow = snapshot.helium_mass_flow_kg_per_s;
    let he_tau = snapshot.helium_residence_time_s;
    let steam_flow = snapshot.secondary_mass_flow_kg_per_s;
    let steam_tau = snapshot.secondary_residence_time_s;

    // Primary loop: reactor -> IHX carries core-outlet helium; the return legs
    // carry the (cooler) IHX outlet.
    ui.add(run(
        at(150.0, 120.0),
        at(300.0, 120.0),
        snapshot.core_outlet_temp_k,
        he_flow,
        he_tau,
        HELIUM_COLOUR_MIN_K,
        HELIUM_COLOUR_MAX_K,
        tracers.primary,
    ));
    ui.add(run(
        at(300.0, 180.0),
        at(150.0, 180.0),
        snapshot.ihx_outlet_temp_k,
        he_flow,
        he_tau,
        HELIUM_COLOUR_MIN_K,
        HELIUM_COLOUR_MAX_K,
        tracers.primary,
    ));
    ui.add(run(
        at(240.0, 180.0),
        at(240.0, 260.0),
        snapshot.core_inlet_temp_k,
        he_flow,
        he_tau,
        HELIUM_COLOUR_MIN_K,
        HELIUM_COLOUR_MAX_K,
        tracers.primary,
    ));

    // Secondary loop: SG -> turbine and turbine -> condenser carry steam; the
    // condensate/feedwater legs carry the cold end of the cycle.
    ui.add(run(
        at(560.0, 120.0),
        at(700.0, 120.0),
        snapshot.sg_steam_outlet_temp_k,
        steam_flow,
        steam_tau,
        STEAM_COLOUR_MIN_K,
        STEAM_COLOUR_MAX_K,
        tracers.secondary,
    ));
    ui.add(run(
        at(820.0, 130.0),
        at(900.0, 130.0),
        snapshot.turbine_inlet_temp_k,
        steam_flow,
        steam_tau,
        STEAM_COLOUR_MIN_K,
        STEAM_COLOUR_MAX_K,
        tracers.secondary,
    ));
    ui.add(run(
        at(900.0, 180.0),
        at(720.0, 300.0),
        snapshot.cooling_water_outlet_temp_k,
        steam_flow,
        steam_tau,
        STEAM_COLOUR_MIN_K,
        STEAM_COLOUR_MAX_K,
        tracers.secondary,
    ));
    ui.add(run(
        at(700.0, 300.0),
        at(540.0, 180.0),
        snapshot.cooling_water_outlet_temp_k,
        steam_flow,
        steam_tau,
        STEAM_COLOUR_MIN_K,
        STEAM_COLOUR_MAX_K,
        tracers.secondary,
    ));

    // IHX -> steam generator is a *heat* transfer, not a fluid run, so it stays
    // a painter line: there is no stream, temperature, or residence time for a
    // PipeVisual to represent.
    ui.painter().line_segment(
        [at(360.0, 150.0), at(500.0, 150.0)],
        Stroke::new(3.0, Color32::from_rgb(240, 180, 60)),
    );

    // --- Reactor vessel (real nee_soon prompt-excursion physics) ---
    let mut reactor_physics = NordheimFuchsExactTimestepper::default();
    reactor_physics.power = Power::new::<watt>(snapshot.reactor_power_mw * 1.0e6);
    reactor_physics.fuel_temperature =
        ThermodynamicTemperature::new::<kelvin>(snapshot.fuel_temperature_k);
    ui.add(ReactorVesselVisual::new(
        reactor_physics,
        at(100.0, 150.0),
        Vec2::new(90.0, 120.0),
        // Spans the fuel temperatures this HTGR model reaches, so a nominal
        // operating point sits mid-scale rather than pinned at either end.
        ThermodynamicTemperature::new::<kelvin>(600.0),
        ThermodynamicTemperature::new::<kelvin>(1500.0),
    ));

    // --- IHX (helium/steam boundary heat exchanger) ---
    let ihx = HeatExchanger::new(
        FlowArrangement::CounterCurrent,
        Area::new::<square_meter>(500.0),
        HeatTransfer::new::<watt_per_square_meter_kelvin>(500.0),
    );
    ui.add(HeatExchangerVisual::new(
        ihx,
        at(330.0, 150.0),
        Vec2::new(60.0, 90.0),
    ));

    // --- Helium circulator pump ---
    let circulator = Pump::new(
        PumpSpecification::DeltaP(Pressure::new::<megapascal>(0.3)),
        Ratio::new::<ratio>(0.8),
    );
    ui.add(PumpVisual::new(
        circulator,
        at(240.0, 290.0),
        Vec2::new(44.0, 44.0),
    ));

    // --- Steam generator (colours by secondary steam temperature) ---
    let reference_volume = Volume::new::<cubic_meter>(1.0);
    let steam_state: HemSteamCv = HemSteamCv::new_from_ph(
        Pressure::new::<megapascal>(snapshot.steam_pressure_mpa),
        AvailableEnergy::new::<joule_per_kilogram>(snapshot.steam_enthalpy_j_per_kg),
        reference_volume,
    );
    let sg_heat_exchanger = HeatExchanger::new(
        FlowArrangement::CounterCurrent,
        Area::new::<square_meter>(800.0),
        HeatTransfer::new::<watt_per_square_meter_kelvin>(1500.0),
    );
    let steam_generator = SteamGenerator::new(sg_heat_exchanger, steam_state);
    ui.add(SteamGeneratorVisual::new(
        steam_generator,
        at(530.0, 150.0),
        Vec2::new(60.0, 110.0),
        ThermodynamicTemperature::new::<kelvin>(500.0),
        ThermodynamicTemperature::new::<kelvin>(900.0),
    ));

    // --- Turbine (colours by inlet steam temperature) ---
    let turbine = Turbine::new(steam_state, Ratio::new::<ratio>(0.85));
    ui.add(TurbineVisual::new_thermo(
        turbine,
        at(760.0, 120.0),
        Vec2::new(120.0, 70.0),
        ThermodynamicTemperature::new::<kelvin>(500.0),
        ThermodynamicTemperature::new::<kelvin>(900.0),
    ));

    // --- Condenser ---
    let condenser = Condenser::new(
        Pressure::new::<megapascal>(snapshot.condenser_pressure_kpa / 1000.0),
        snapshot.steam_quality_after_turbine,
    );
    ui.add(CondenserVisual::new(
        condenser,
        at(940.0, 130.0),
        Vec2::new(70.0, 70.0),
    ));

    // --- Feedwater control valve (colours by opening) ---
    let feed_valve = Valve::new(
        ValveFlowCoefficient(80.0),
        OpeningCharacteristic::Linear,
        Ratio::new::<percent>(75.0),
    );
    ui.add(ValveVisual::new(
        feed_valve,
        at(620.0, 260.0),
        Vec2::new(40.0, 30.0),
    ));

    // --- Feedwater pump ---
    let feed_pump = Pump::new(
        PumpSpecification::DeltaP(Pressure::new::<megapascal>(snapshot.steam_pressure_mpa)),
        Ratio::new::<ratio>(0.75),
    );
    ui.add(PumpVisual::new(
        feed_pump,
        at(710.0, 300.0),
        Vec2::new(44.0, 44.0),
    ));

    // --- Instrumentation readouts (engine InstrumentationVisual) ---
    let readouts: [(f32, f32, &str, String); 6] = [
        (
            60.0,
            235.0,
            "Power",
            format!("{:.1} MWth", snapshot.reactor_power_mw),
        ),
        (
            60.0,
            255.0,
            "T_fuel",
            format!("{:.0} K", snapshot.fuel_temperature_k),
        ),
        (
            330.0,
            260.0,
            "T_He,out",
            format!("{:.0} K", snapshot.core_outlet_temp_k),
        ),
        (
            330.0,
            280.0,
            "IHX duty",
            format!("{:.1} MW", snapshot.ihx_duty_mw),
        ),
        (
            760.0,
            210.0,
            "P_turb",
            format!("{:.1} MW", snapshot.turbine_power_mw),
        ),
        (
            760.0,
            230.0,
            "x_exhaust",
            format!("{:.3}", snapshot.steam_quality_after_turbine),
        ),
    ];
    for (x, y, label, value) in readouts {
        ui.add(InstrumentationVisual::new(at(x, y), label, value));
    }
}
