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
//! [`CondenserVisual`], [`PumpVisual`], [`ValveVisual`], and
//! [`InstrumentationVisual`]. Connectors between components are drawn as plain
//! `painter` lines: the engine's [`PipeVisual`](outram_park_digital_twin_engine::components::PipeVisual)
//! needs a heavy `tampines` fluid-array backend to construct, deferred to v2
//! (bead `op-wqk.9.4`).

use egui::{pos2, Color32, Pos2, Stroke, Ui, Vec2};

use outram_park_digital_twin_engine::components::{
    CondenserVisual, HeatExchangerVisual, InstrumentationVisual, PumpVisual, ReactorVesselVisual,
    SteamGeneratorVisual, TurbineVisual, ValveVisual,
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
    Area, AvailableEnergy, HeatTransfer, Power, Pressure, Ratio, ThermodynamicTemperature, Volume,
};
use uom::si::heat_transfer::watt_per_square_meter_kelvin;
use uom::si::power::watt;
use uom::si::pressure::megapascal;
use uom::si::ratio::{percent, ratio};
use uom::si::thermodynamic_temperature::kelvin;
use uom::si::volume::cubic_meter;

use crate::app::state::HtgrSnapshot;

/// Draw the whole schematic from `snapshot` into `ui`.
pub fn draw_schematic(ui: &mut Ui, snapshot: &HtgrSnapshot) {
    // Reserve a fixed canvas so the absolute widget positions have room; the
    // engine widgets paint at their own `screen_position`, independent of the
    // egui layout cursor.
    let canvas = Vec2::new(1040.0, 460.0);
    let (canvas_rect, _response) = ui.allocate_exact_size(canvas, egui::Sense::hover());
    let origin = canvas_rect.min.to_vec2();

    // Helper to shift a schematic-local point into canvas coordinates.
    let at = |x: f32, y: f32| -> Pos2 { pos2(x, y) + origin };

    // --- Connector lines (painter; PipeVisual deferred, bead op-wqk.9.4) ---
    let hot_he = Stroke::new(4.0, Color32::from_rgb(220, 90, 40)); // hot helium
    let cold_he = Stroke::new(4.0, Color32::from_rgb(80, 130, 220)); // return helium
    let steam = Stroke::new(4.0, Color32::from_rgb(210, 210, 210));
    let heat = Stroke::new(3.0, Color32::from_rgb(240, 180, 60));
    let painter = ui.painter();
    // primary loop
    painter.line_segment([at(150.0, 120.0), at(300.0, 120.0)], hot_he); // reactor -> IHX (hot)
    painter.line_segment([at(300.0, 180.0), at(150.0, 180.0)], cold_he); // IHX -> reactor (cold)
    painter.line_segment([at(240.0, 180.0), at(240.0, 260.0)], cold_he); // down to circulator
                                                                         // IHX -> steam generator (heat transfer)
    painter.line_segment([at(360.0, 150.0), at(500.0, 150.0)], heat);
    // secondary loop
    painter.line_segment([at(560.0, 120.0), at(700.0, 120.0)], steam); // SG -> turbine
    painter.line_segment([at(820.0, 130.0), at(900.0, 130.0)], steam); // turbine -> condenser
    painter.line_segment([at(900.0, 180.0), at(720.0, 300.0)], cold_he); // condenser -> feed pump
    painter.line_segment([at(700.0, 300.0), at(540.0, 180.0)], cold_he); // feed pump -> SG

    // --- Reactor vessel (real nee_soon prompt-excursion physics) ---
    let mut reactor_physics = NordheimFuchsExactTimestepper::default();
    reactor_physics.power = Power::new::<watt>(snapshot.reactor_power_mw * 1.0e6);
    reactor_physics.fuel_temperature =
        ThermodynamicTemperature::new::<kelvin>(snapshot.fuel_temperature_k);
    ui.add(ReactorVesselVisual::new(
        reactor_physics,
        at(100.0, 150.0),
        Vec2::new(90.0, 120.0),
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
    ui.add(TurbineVisual::new(
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
