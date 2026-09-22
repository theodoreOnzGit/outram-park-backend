//! The v1.1 plant view: the whole HTR-10 plant, driven by this simulator's
//! running model.
//!
//! It replaced the v1 schematic on screen on 2026-09-22 (maintainer
//! direction). The drawing itself is the engine's
//! `components::htr10_plant::draw_htr10_plant`, the same one the widget
//! studio's HTR-10 page uses; this module only fills it from the
//! [`HtgrSnapshot`] each frame. So everything that moves or colours here is
//! the model's state:
//!
//! | on screen | from the snapshot |
//! |---|---|
//! | pebble bed | fuel temperature |
//! | helium in / out, vessel wall | core inlet / outlet temperature |
//! | reflector | bed (graphite) temperature |
//! | control rods | operator command, slewed like v1, with the scram floor |
//! | helium tracers | helium mass flow and residence time |
//! | main steam / feedwater / condensate | SG steam outlet; (p, h) flashes as in v1 |
//! | turbine rotor | shaft speed |
//! | condenser | exhaust quality, condensate temperature, cooling water |
//! | secondary tracers | secondary mass flow and residence time |
//!
//! Two things are not model state, and are stated rather than invented: the
//! feed pump draws stationary, because this plant has no pump shaft-speed
//! model (the same choice v1 made), and the bed height is the equilibrium
//! loading, since the model has no bed-height variable.
//!
//! **Offline demonstration only**, per the workspace `RESPONSIBLE_USE.md`.

use super::schematic::{feed_and_condensate_temps, SchematicTracers, DISPLAY_MAX_K, DISPLAY_MIN_K};
use super::state::HtgrSnapshot;
use crate::physics::secondary_loop::COOLING_WATER_INLET_K;
use egui::Ui;
use outram_park_digital_twin_engine::animation::control_rod_drive::ControlRodDrive;
use outram_park_digital_twin_engine::components::control_rod_drive::slewed_control_rod_insertion;
use outram_park_digital_twin_engine::components::htr10_plant::{
    draw_htr10_plant, SecondaryLoopView, SecondaryTracers,
};
use outram_park_digital_twin_engine::components::htr10_reactor_schematic::EQUILIBRIUM_BED_HEIGHT_CM;
use outram_park_digital_twin_engine::components::{Htr10ReactorSchematic, Htr10SteamGeneratorVisual};
use uom::si::angular_velocity::radian_per_second;
use uom::si::f64::{AngularVelocity, MassRate, ThermodynamicTemperature, Time};
use uom::si::mass_rate::kilogram_per_second;
use uom::si::thermodynamic_temperature::kelvin;
use uom::si::time::second;

/// Drawn vessel width, points. A drawing choice, sized so the whole plant fits
/// the simulator's default window.
const VESSEL_WIDTH: f32 = 200.0;

fn k(value_k: f64) -> ThermodynamicTemperature {
    ThermodynamicTemperature::new::<kelvin>(value_k)
}

/// Draw the v1.1 plant from this frame's `snapshot`, with the application's
/// tracer trains.
pub fn draw_plant_v1_1(ui: &mut Ui, snapshot: &HtgrSnapshot, tracers: &SchematicTracers) {
    let (min_t, max_t) = (k(DISPLAY_MIN_K), k(DISPLAY_MAX_K));
    let primary = tracers.primary;
    let secondary_train = tracers.secondary;

    // The rods: the operator's command slewed at the drive speed, with the
    // protection system's scram insertion as a floor, exactly as v1 draws them
    // (see that module's section 4 for why the scram is not slewed twice).
    let commanded = snapshot.control_rod_insertion_fraction;
    let slewed = slewed_control_rod_insertion(
        ui.ctx(),
        ui.id().with("htr10_control_rod_bank"),
        commanded,
        ControlRodDrive::htr10(commanded),
    );
    let rods = slewed.max(snapshot.scram_insertion_fraction as f32);

    // Every helium pass carries the primary train, as every v1 helium run
    // does: one loop, one flow, one residence time.
    let reactor = Htr10ReactorSchematic::new(
        Htr10ReactorSchematic::native_size(VESSEL_WIDTH),
        min_t,
        max_t,
        k(snapshot.fuel_temperature_k),
        k(snapshot.core_inlet_temp_k),
        k(snapshot.core_outlet_temp_k),
        k(snapshot.bed_temperature_k),
        k(snapshot.core_inlet_temp_k),
    )
    .with_control_rod_frac(rods)
    .with_bed_height_cm(EQUILIBRIUM_BED_HEIGHT_CM)
    .with_downcomer_tracer(primary)
    .with_riser_tracer(primary)
    .with_plenum_tracer(primary)
    .with_cold_plenum_tracer(primary)
    .with_hot_duct_tracer(primary)
    .with_cold_duct_tracer(primary);

    let (feedwater_temp, condensate_temp) = feed_and_condensate_temps(snapshot);
    let steam_temp = k(snapshot.sg_steam_outlet_temp_k);
    let make_sg = |size| {
        Htr10SteamGeneratorVisual::new(
            size,
            min_t,
            max_t,
            k(snapshot.core_outlet_temp_k),
            k(snapshot.ihx_outlet_temp_k),
            feedwater_temp,
            steam_temp,
        )
        .with_riser_tracer(primary)
        .with_shell_gas_tracer(primary)
        .with_coil_water_tracer(secondary_train)
        .with_feedwater_tracer(secondary_train)
        .with_steam_tracer(secondary_train)
        .with_duct_inlet_tracers(primary, primary)
    };

    let secondary = SecondaryLoopView {
        steam_temp,
        feedwater_temp,
        condensing_temp: condensate_temp,
        exhaust_quality: snapshot.steam_quality_after_turbine,
        cooling_water_inlet_temp: k(COOLING_WATER_INLET_K),
        cooling_water_outlet_temp: k(snapshot.cooling_water_outlet_temp_k),
        mass_flow: MassRate::new::<kilogram_per_second>(snapshot.secondary_mass_flow_kg_per_s),
        pipe_residence_time: Time::new::<second>(snapshot.secondary_residence_time_s),
        turbine_speed: AngularVelocity::new::<radian_per_second>(snapshot.shaft_speed_rad_per_s),
        // No pump shaft-speed model in this plant: drawn stationary, as in v1.
        pump_speed: AngularVelocity::new::<radian_per_second>(0.0),
        simulation_time: Time::new::<second>(snapshot.sim_time_s),
        tracers: SecondaryTracers {
            main_steam: secondary_train,
            exhaust: secondary_train,
            condensate: secondary_train,
            feed: secondary_train,
        },
        min_temp: min_t,
        max_temp: max_t,
    };

    draw_htr10_plant(ui, reactor, make_sg, &secondary);
}
