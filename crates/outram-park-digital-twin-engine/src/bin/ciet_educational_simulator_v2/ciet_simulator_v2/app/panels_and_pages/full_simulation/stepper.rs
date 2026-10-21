//! One timestep of the CIET loop, as a value (gh:#924).
//!
//! [`CietLoop`] owns every component, controller and lagged flow rate that
//! [`super::educational_ciet_loop_version_4`] used to keep as locals of one
//! never-returning function; [`CietLoop::step`] is that function's loop body
//! minus the real-time pacing, moved here unchanged. The desktop physics
//! thread calls it once per timestep and paces around it; the browser demo
//! (`examples/ciet_web`) calls the same code from a Web Worker. One copy of
//! the physics, two drivers.
//!
//! ## Parallelism
//!
//! Each step solves the DRACS loop flow and the primary loop flows from the
//! components as they stand at the START of the step, while this thread links
//! up and advances the heat transfer; the flows found are used on the NEXT
//! step. Those solves go through [`super::fork::fork`]: a thread natively, as
//! v1 had, and run inline on `wasm32`, which has no threads. Because the
//! solves read start-of-step copies either way, the two orders give the same
//! numbers (checked bit for bit by `the_stepper_is_deterministic_and_inline_equals_threaded`).
//!
//! ## The move was checked against the loop it came from
//!
//! On 2026-10-17 the loop as it stood before this module existed (commit
//! `c02bbf5108`) was copied, run for a fixed number of steps without its
//! sleeps, and compared with [`CietLoop::step`] from the same state: equal in
//! every field of the plant state after 400 steps at 8 kW with the pump off,
//! 400 steps at 8 kW and 4 kPa, and 200 steps at 0 kW and -3 kPa. The copy
//! (1900 lines) was not committed; the comparison is recorded in gh:#924.
//!
//! **The desktop's timestep is an `f32`.** Equality needed the step to take
//! `get_timestep_seconds() as f64`, i.e. 0.10000000149 s, as the desktop loop
//! does while it keeps up with real time; it switches to exactly
//! [`max_timestep`] (0.1 s) only when it falls behind. Stepping exactly
//! 0.1 s from the start differs in the 10th figure after 400 steps.
//!
//! ## Provenance
//!
//! The physics is v1's (see [`super`]), unchanged by the move. Two
//! `AnalogController` dead-time blocks v1 built and never used are not
//! carried over.

// The snapshot half of this module (`flow_snapshot`, `FlowSource::Later`,
// `finish_step`, ...) is the browser demo's (`examples/ciet_web`, which
// includes this file by path); the desktop binary steps with
// `FlowSource::Solve` only.
#![allow(dead_code)]

use std::sync::Arc;

use super::diagnostics::pri_loop_heater_temperature_diagnostics_ver_4;
use super::fluid_solvers::three_branch_pri_loop_flowrates_parallel_ver_4;
use super::fork::fork;
use super::heat_transfer_solvers::{
    ciet_pri_loop_three_branch_link_up_components_ver_4,
    pri_loop_three_branch_advance_timestep_except_dhx_ver_4,
};
use tuas_boussinesq_solver::{
    boussinesq_thermophysical_properties::LiquidMaterial,
    pre_built_components::ciet_three_branch_plus_dracs::components::{
        new_active_ctah_horizontal, new_active_ctah_vertical,
    },
    prelude::beta_testing::{HeatTransferEntity, InsulatedPorousMediaFluidComponent},
    single_control_vol::SingleCVNode,
};
use uom::si::{
    length::{inch, meter},
    mass_rate::kilogram_per_second,
    power::kilowatt,
    pressure::{atmosphere, pascal},
};

use outram_park_digital_twin_engine::ciet_opcua::state::{CietState as CIETState, HeaterType};

use uom::si::f64::*;
use uom::si::length::centimeter;
use uom::si::{frequency::hertz, ratio::ratio};
use tuas_boussinesq_solver::prelude::beta_testing::FluidArray;
use uom::ConstZero;
use tuas_boussinesq_solver::heat_transfer_correlations::nusselt_number_correlations::enums::NusseltCorrelation;
use tuas_boussinesq_solver::pre_built_components::ciet_isothermal_test_components::*;
use tuas_boussinesq_solver::pre_built_components::ciet_nat_circ_tests::coupled_dracs_loop_tests::dhx_constructor::new_dhx_sthe_version_1;
use tuas_boussinesq_solver::pre_built_components::ciet_nat_circ_tests::coupled_dracs_loop_tests::dracs_loop_calc_functions_no_tchx_calibration::dracs_loop_dhx_tube_temperature_diagnostics;
use tuas_boussinesq_solver::pre_built_components::ciet_nat_circ_tests::coupled_dracs_loop_tests::dracs_loop_calc_functions_sam_tchx_calibration::{coupled_dracs_fluid_mechanics_calc_abs_mass_rate_sam_tchx_calibration, coupled_dracs_loop_link_up_components_sam_tchx_calibration, dracs_loop_advance_timestep_except_dhx_sam_tchx_calibration};
use tuas_boussinesq_solver::pre_built_components::ciet_nat_circ_tests::coupled_dracs_loop_tests::pri_loop_calc_functions::pri_loop_dhx_shell_temperature_diagnostics;
use tuas_boussinesq_solver::pre_built_components::ciet_nat_circ_tests::dracs_loop_components::*;
use uom::si::thermodynamic_temperature::{degree_celsius, kelvin};
use uom::si::heat_transfer::watt_per_square_meter_kelvin;
use uom::si::time::second;
use tuas_boussinesq_solver::pre_built_components::insulated_pipes_and_fluid_components::InsulatedFluidComponent;
use tuas_boussinesq_solver::pre_built_components::non_insulated_fluid_components::NonInsulatedFluidComponent;
use tuas_boussinesq_solver::pre_built_components::shell_and_tube_heat_exchanger::SimpleShellAndTubeHeatExchanger;
use chem_eng_real_time_process_control_simulator::alpha_nightly::transfer_fn_wrapper_and_enums::TransferFnTraits;
use chem_eng_real_time_process_control_simulator::alpha_nightly::controllers::AnalogController;

/// The DHX's calibrated heat loss to ambient, W/(m^2 K) (v1's regression;
/// set at construction and again after every link-up, which resets it).
const DHX_HEAT_LOSS_TO_AMBIENT_WATTS_PER_M2_KELVIN: f64 = 33.9;

/// The largest timestep the loop takes, for Courant-number stability (v1).
pub fn max_timestep() -> Time {
    Time::new::<second>(0.1)
}

/// Where a step's flow rates come from (gh:#924).
#[derive(Clone, Copy, Debug)]
pub enum FlowSource {
    /// Solved in the step, on threads natively and inline on `wasm32` (v1).
    Solve,
    /// Solved elsewhere from this step's [`CietLoop::flow_snapshot`] (the
    /// browser's flow workers) and handed over by [`CietLoop::finish_step`],
    /// which must be called before the next step.
    Later,
}

/// Flow rates solved elsewhere, for [`CietLoop::finish_step`].
#[derive(Clone, Copy, Debug)]
pub struct GivenFlows {
    /// DRACS loop flow magnitude.
    pub dracs: MassRate,
    /// DHX branch flow (TUAS's sign).
    pub dhx_branch: MassRate,
    /// Heater branch flow (TUAS's sign).
    pub heater_branch: MassRate,
    /// CTAH branch flow (TUAS's sign).
    pub ctah_branch: MassRate,
}

impl GivenFlows {
    /// From kg/s: the DRACS magnitude and the (DHX, heater, CTAH) branches.
    pub fn from_kg_per_s(dracs: f64, primary: [f64; 3]) -> Self {
        let m = MassRate::new::<kilogram_per_second>;
        Self { dracs: m(dracs), dhx_branch: m(primary[0]), heater_branch: m(primary[1]), ctah_branch: m(primary[2]) }
    }
}

/// What the two flow solves read, as plain numbers (gh:#924): the DRACS
/// fluid temperatures, K; and the pump pressure rise (Pa), CTAH and DHX
/// valves shut (1/0), coarse heater (1/0), then the primary fluid
/// temperatures, K. See [`CietLoop::flow_snapshot`].
#[derive(Clone, Debug, PartialEq)]
pub struct FlowSnapshot {
    /// For [`CietLoop::solve_dracs_snapshot`].
    pub dracs: Vec<f64>,
    /// For [`CietLoop::solve_primary_snapshot`].
    pub primary: Vec<f64>,
}

/// Append a fluid array's temperatures, K.
fn push_temperatures(hte: &HeatTransferEntity, out: &mut Vec<f64>) {
    let fluid_array: FluidArray = hte.clone().try_into().expect("a CIET fluid component holds a fluid array");
    out.extend(
        fluid_array
            .get_temperature_vector()
            .expect("a fluid array has temperatures")
            .iter()
            .map(|t| t.get::<kelvin>()),
    );
}

/// Set a fluid array's temperatures from the front of `temps_k`; return the
/// rest.
fn set_temperatures<'a>(hte: &mut HeatTransferEntity, temps_k: &'a [f64]) -> Result<&'a [f64], String> {
    let mut fluid_array: FluidArray = hte.clone().try_into().map_err(|e| format!("{e:?}"))?;
    let (mine, rest) = temps_k.split_at_checked(fluid_array.len()).ok_or("flow snapshot too short")?;
    fluid_array
        .set_temperature_vector(mine.iter().map(|&k| ThermodynamicTemperature::new::<kelvin>(k)).collect())
        .map_err(|e| format!("{e:?}"))?;
    *hte = fluid_array.into();
    Ok(rest)
}

/// The whole CIET loop between timesteps: components, controllers, the
/// simulation clock and the flow rates solved on the previous step.
#[derive(Clone)]
pub struct CietLoop {
    tchx_pid_controller: AnalogController,
    fresh_tchx_pid_controller: AnalogController,
    ctah_pid_controller: AnalogController,
    fresh_ctah_pid_controller: AnalogController,
    current_simulation_time: Time,
    pipe_34: InsulatedFluidComponent,
    pipe_33: InsulatedFluidComponent,
    pipe_32: InsulatedFluidComponent,
    pipe_31a: InsulatedFluidComponent,
    static_mixer_61_label_31: InsulatedFluidComponent,
    dhx_tube_side_30b: NonInsulatedFluidComponent,
    dhx_sthe: SimpleShellAndTubeHeatExchanger,
    dhx_tube_side_30a: NonInsulatedFluidComponent,
    tchx_35a: NonInsulatedFluidComponent,
    tchx_35b_1: NonInsulatedFluidComponent,
    tchx_35b_2: NonInsulatedFluidComponent,
    static_mixer_60_label_36: InsulatedFluidComponent,
    pipe_36a: InsulatedFluidComponent,
    pipe_37: InsulatedFluidComponent,
    flowmeter_60_37a: NonInsulatedFluidComponent,
    pipe_38: InsulatedFluidComponent,
    pipe_39: InsulatedFluidComponent,
    pipe_5a: InsulatedFluidComponent,
    pipe_26: InsulatedFluidComponent,
    pipe_25a: InsulatedFluidComponent,
    static_mixer_21_label_25: InsulatedFluidComponent,
    pipe_23a: InsulatedFluidComponent,
    static_mixer_20_label_23: InsulatedFluidComponent,
    pipe_22: InsulatedFluidComponent,
    flowmeter_20_21a: NonInsulatedFluidComponent,
    pipe_21: InsulatedFluidComponent,
    pipe_20: InsulatedFluidComponent,
    pipe_19: InsulatedFluidComponent,
    pipe_17b: InsulatedFluidComponent,
    pipe_4: InsulatedFluidComponent,
    pipe_3: InsulatedFluidComponent,
    pipe_2a: InsulatedFluidComponent,
    static_mixer_10_label_2: InsulatedFluidComponent,
    heater_top_head_1a: InsulatedPorousMediaFluidComponent,
    heater_ver_1_15_nodes: InsulatedPorousMediaFluidComponent,
    heater_ver_1_8_nodes: InsulatedPorousMediaFluidComponent,
    heater_bottom_head_1b: InsulatedPorousMediaFluidComponent,
    pipe_18: InsulatedFluidComponent,
    pipe_5b: InsulatedFluidComponent,
    static_mixer_41_label_6: InsulatedFluidComponent,
    pipe_6a: InsulatedFluidComponent,
    ctah_vertical_label_7a: NonInsulatedFluidComponent,
    ctah_horizontal_label_7b: NonInsulatedFluidComponent,
    pipe_8a: InsulatedFluidComponent,
    static_mixer_40_label_8: InsulatedFluidComponent,
    pipe_9: InsulatedFluidComponent,
    pipe_10: InsulatedFluidComponent,
    pipe_11: InsulatedFluidComponent,
    pipe_12: InsulatedFluidComponent,
    ctah_pump: NonInsulatedFluidComponent,
    pipe_13: InsulatedFluidComponent,
    pipe_14: InsulatedFluidComponent,
    flowmeter_40_14a: NonInsulatedFluidComponent,
    pipe_15: InsulatedFluidComponent,
    pipe_16: InsulatedFluidComponent,
    pipe_17a: InsulatedFluidComponent,
    top_mixing_node_5a_5b_4: HeatTransferEntity,
    bottom_mixing_node_17a_17b_18: HeatTransferEntity,
    mass_flowrate_dhx_br: MassRate,
    mass_flowrate_dracs_loop_abs: MassRate,
    mass_flowrate_ctah_br: MassRate,
    mass_flowrate_heater_br: MassRate,
}

impl CietLoop {
    /// The loop as v1 builds it: every component at the TCHX outlet set
    /// point of `local_ciet_state_initial`, calibrated, flows zero.
    ///
    /// # Panics
    ///
    /// If a TUAS constructor fails (a bug, as in v1).
    pub fn new(local_ciet_state_initial: &CIETState) -> Self {
    let tchx_outlet_temperature_set_point_degc =
        local_ciet_state_initial.bt_66_tchx_outlet_set_pt_deg_c;
    let tchx_outlet_temperature_set_point =
        ThermodynamicTemperature::new::<degree_celsius>(tchx_outlet_temperature_set_point_degc);


    // controller for tchx

    let tchx_controller_gain = Ratio::new::<ratio>(70.75);
    let tchx_integral_time: Time = tchx_controller_gain / Frequency::new::<hertz>(50.0);
    let tchx_derivative_time: Time = Time::new::<second>(1.0);
    // derivative time ratio
    let tchx_alpha: Ratio = Ratio::new::<ratio>(1.0);

    let tchx_pid_controller: AnalogController = AnalogController::new_filtered_pid_controller(
        tchx_controller_gain,
        tchx_integral_time,
        tchx_derivative_time,
        tchx_alpha,
    )
    .unwrap();


    let fresh_tchx_pid_controller = tchx_pid_controller.clone();


    // controller for ctah
    let ctah_controller_gain = Ratio::new::<ratio>(70.75);
    let ctah_integral_time: Time = ctah_controller_gain / Frequency::new::<hertz>(50.0);
    let ctah_derivative_time: Time = Time::new::<second>(1e-2);
    // derivative time ratio
    let ctah_alpha: Ratio = Ratio::new::<ratio>(1.0);

    let ctah_pid_controller: AnalogController = AnalogController::new_filtered_pid_controller(
        ctah_controller_gain,
        ctah_integral_time,
        ctah_derivative_time,
        ctah_alpha,
    )
    .unwrap();


    let fresh_ctah_pid_controller = ctah_pid_controller.clone();


    let current_simulation_time = Time::ZERO;
    let initial_temperature = tchx_outlet_temperature_set_point;

    // create components
    // DRACS hot branch or (mostly) hot leg
    let mut pipe_34 = new_pipe_34(initial_temperature);
    let mut pipe_33 = new_pipe_33(initial_temperature);
    let mut pipe_32 = new_pipe_32(initial_temperature);
    let mut pipe_31a = new_pipe_31a(initial_temperature);
    let mut static_mixer_61_label_31 = new_static_mixer_61_label_31(initial_temperature);
    let dhx_tube_side_30b = new_dhx_tube_side_30b(initial_temperature);
    let mut dhx_sthe = new_dhx_sthe_version_1(initial_temperature);
    let dhx_tube_side_30a = new_dhx_tube_side_30a(initial_temperature);

    // DRACS cold branch or (mostly) cold leg
    let tchx_35a = new_ndhx_tchx_horizontal_35a(initial_temperature);
    let tchx_35b_1 = new_ndhx_tchx_vertical_35b_1(initial_temperature);
    let mut tchx_35b_2 = new_ndhx_tchx_vertical_35b_2(initial_temperature);
    let mut static_mixer_60_label_36 = new_static_mixer_60_label_36(initial_temperature);
    let mut pipe_36a = new_pipe_36a(initial_temperature);
    let mut pipe_37 = new_pipe_37(initial_temperature);
    let flowmeter_60_37a = new_flowmeter_60_37a(initial_temperature);
    let mut pipe_38 = new_pipe_38_sam_model(initial_temperature);
    let mut pipe_39 = new_pipe_39(initial_temperature);

    // pri loop dhx branch top to bottom 5a to 17b
    let ambient_temperature = ThermodynamicTemperature::new::<degree_celsius>(20.0);

    let mut pipe_5a = new_branch_5a(initial_temperature);
    let mut pipe_26 = new_pipe_26(initial_temperature);
    let mut pipe_25a = new_pipe_25a(initial_temperature);
    let mut static_mixer_21_label_25 = new_static_mixer_21_label_25(initial_temperature);
    // here is where the dhx shell side should be (component 24)
    let mut pipe_23a = new_pipe_23a(initial_temperature);
    let mut static_mixer_20_label_23 = new_static_mixer_20_label_23(initial_temperature);
    let mut pipe_22 = new_pipe_22_sam_model(initial_temperature);
    let flowmeter_20_21a = new_flowmeter_20_label_21a(initial_temperature);
    let mut pipe_21 = new_pipe_21(initial_temperature);
    let mut pipe_20 = new_pipe_20(initial_temperature);
    let mut pipe_19 = new_pipe_19(initial_temperature);
    let mut pipe_17b = new_branch_17b(initial_temperature);

    // heater branch top to bottom 4 to 18
    let mut pipe_4 = new_pipe_4(initial_temperature);
    let mut pipe_3 = new_pipe_3_sam_model(initial_temperature);
    let mut pipe_2a = new_pipe_2a(initial_temperature);
    let mut static_mixer_10_label_2 = new_static_mixer_10_label_2(initial_temperature);
    let mut heater_top_head_1a = InsulatedPorousMediaFluidComponent::new_ciet_heater_v1_top_head(
        initial_temperature,
        ambient_temperature,
        2 - 2,
    );
    let mut heater_ver_1_15_nodes =
        InsulatedPorousMediaFluidComponent::new_ciet_heater_v1_with_annular_pipe(
            initial_temperature,
            ambient_temperature,
            15 - 2,
        );

    let heater_ver_1_8_nodes =
        InsulatedPorousMediaFluidComponent::new_ciet_heater_v1_with_annular_pipe(
            initial_temperature,
            ambient_temperature,
            8 - 2,
        );
    let mut heater_bottom_head_1b =
        InsulatedPorousMediaFluidComponent::new_ciet_heater_v1_bottom_head(
            initial_temperature,
            ambient_temperature,
            2 - 2,
        );
    let mut pipe_18 = new_pipe_18(initial_temperature);

    // ctah branch
    let mut pipe_5b = new_branch_5b(initial_temperature);
    let mut static_mixer_41_label_6 = new_static_mixer_41_label_6(initial_temperature);
    let mut pipe_6a = new_pipe_6a(initial_temperature);
    let mut ctah_vertical_label_7a = new_active_ctah_vertical(initial_temperature);
    let mut ctah_horizontal_label_7b = new_active_ctah_horizontal(initial_temperature);
    let mut pipe_8a = new_pipe_8a(initial_temperature);
    let mut static_mixer_40_label_8 = new_static_mixer_40_label_8(initial_temperature);
    let mut pipe_9 = new_pipe_9(initial_temperature);
    let mut pipe_10 = new_pipe_10(initial_temperature);
    let mut pipe_11 = new_pipe_11(initial_temperature);
    let mut pipe_12 = new_pipe_12(initial_temperature);
    let ctah_pump = new_ctah_pump(initial_temperature);
    let mut pipe_13 = new_pipe_13(initial_temperature);
    let mut pipe_14 = new_pipe_14(initial_temperature);
    let flowmeter_40_14a = new_flowmeter_40_14a(initial_temperature);
    let mut pipe_15 = new_pipe_15(initial_temperature);
    let mut pipe_16 = new_pipe_16(initial_temperature);
    let mut pipe_17a = new_branch_17a(initial_temperature);

    // mixing nodes between the pipes, should make for more elegant
    // way of linking parallel pipes.

    let top_mixing_node_5a_5b_4: HeatTransferEntity;
    let bottom_mixing_node_17a_17b_18: HeatTransferEntity;

    // mixing node is a sphere about diameter of ping pong ball
    // (1 in)

    let mixing_node_diameter = Length::new::<centimeter>(3.84);
    let mixing_node_material = LiquidMaterial::TherminolVP1;
    let mixing_node_pressure = Pressure::new::<atmosphere>(1.0);
    let mixing_node = SingleCVNode::new_sphere(
        mixing_node_diameter,
        mixing_node_material.into(),
        initial_temperature,
        mixing_node_pressure,
    )
    .unwrap();

    top_mixing_node_5a_5b_4 = mixing_node.clone().into();
    bottom_mixing_node_17a_17b_18 = mixing_node.into();

    let (
        shell_side_to_tubes_nusselt_number_correction_factor,
        dhx_insulation_thickness_regression_cm,
        shell_side_to_ambient_nusselt_correction_factor,
        dhx_heat_loss_to_ambient_watts_per_m2_kelvin,
    ) = (4.7, 0.161, 10.3, 33.9);

    let (
        pri_loop_cold_leg_insulation_thickness_cm,
        pri_loop_hot_leg_insulation_thickness_cm,
        dracs_loop_cold_leg_insulation_thickness_cm,
        dracs_loop_hot_leg_insulation_thickness_cm,
    ) = (0.15, 0.24, 3.00, 0.75);

    let (
        heater_calibrated_nusselt_factor_float,
        _expt_heater_surf_temp_avg_degc,
        _simulated_expected_heater_surf_temp_degc,
        _heater_surface_temp_tolerance_degc,
    ) = (1.6, 109.47, 105.76, 5.0);

    let ctah_branch_post_ctah_insulation_thickness_cm = 1.00;
    let ctah_branch_pre_ctah_insulation_thickness_cm = 0.5;

    // calibration steps **************
    // calibrate DHX STHE
    // calibrated thickness settings
    // did not calibrate ctah branch in this iteration

    let dhx_calibrated_insulation_thickness =
        Length::new::<centimeter>(dhx_insulation_thickness_regression_cm);

    let pri_loop_cold_leg_insulation_thickness =
        Length::new::<centimeter>(pri_loop_cold_leg_insulation_thickness_cm);
    let pri_loop_hot_leg_insulation_thickness =
        Length::new::<centimeter>(pri_loop_hot_leg_insulation_thickness_cm);
    let dracs_loop_cold_leg_insulation_thickness =
        Length::new::<centimeter>(dracs_loop_cold_leg_insulation_thickness_cm);
    let dracs_loop_hot_leg_insulation_thickness =
        Length::new::<centimeter>(dracs_loop_hot_leg_insulation_thickness_cm);
    let ctah_branch_insulation_thickness_pre_ctah =
        Length::new::<centimeter>(ctah_branch_pre_ctah_insulation_thickness_cm);
    let ctah_branch_insulation_thickness_post_ctah =
        Length::new::<centimeter>(ctah_branch_post_ctah_insulation_thickness_cm);

    // calibrated nusselt correlation settings (using Gnielinksi correlation)

    let calibrated_nusselt_factor =
        Ratio::new::<ratio>(shell_side_to_tubes_nusselt_number_correction_factor);

    let calibrated_parasitic_heat_loss_nusselt_factor =
        Ratio::new::<ratio>(shell_side_to_ambient_nusselt_correction_factor);
    // calibrate heat trf coeff to environment
    // (will need to be redone in the loop
    dhx_sthe.heat_transfer_to_ambient = HeatTransfer::new::<watt_per_square_meter_kelvin>(
        dhx_heat_loss_to_ambient_watts_per_m2_kelvin,
    );
    // calibrate shell side fluid array to tubes nusselt number correlation

    fn calibrate_nusselt_correlation_of_heat_transfer_entity(
        nusselt_correlation: &mut NusseltCorrelation,
        calibration_ratio: Ratio,
    ) {
        // it's a little bit troublesome, but we have to open
        // up the enums and change the nusselt correlation like
        // so

        let calibrated_nusselt_correlation = match nusselt_correlation {
            NusseltCorrelation::PipeGnielinskiGeneric(gnielinski_data) => {
                NusseltCorrelation::PipeGnielinskiCalibrated(
                    gnielinski_data.clone(),
                    calibration_ratio,
                )
            }
            NusseltCorrelation::PipeGnielinskiCalibrated(gnielinski_data, _) => {
                NusseltCorrelation::PipeGnielinskiCalibrated(
                    gnielinski_data.clone(),
                    calibration_ratio,
                )
            }
            _ => todo!(),
        };
        *nusselt_correlation = calibrated_nusselt_correlation;
    }

    calibrate_nusselt_correlation_of_heat_transfer_entity(
        &mut dhx_sthe.shell_side_nusselt_correlation_to_tubes,
        calibrated_nusselt_factor,
    );

    calibrate_nusselt_correlation_of_heat_transfer_entity(
        &mut dhx_sthe.shell_side_nusselt_correlation_parasitic,
        calibrated_parasitic_heat_loss_nusselt_factor,
    );
    // calibrate nusselt correlation within TCHX, CTAH to high values
    // eg 10k.

    let ideal_nusselt_number_for_tchx: NusseltCorrelation =
        NusseltCorrelation::IdealNusseltOneBillion;

    let ideal_nusselt_number_for_ctah: NusseltCorrelation =
        NusseltCorrelation::IdealNusseltOneBillion;

    tchx_35b_2.calibrate_nusselt_correlation_for_fluid_within_pipe(ideal_nusselt_number_for_tchx);

    ctah_vertical_label_7a
        .calibrate_nusselt_correlation_for_fluid_within_pipe(ideal_nusselt_number_for_ctah);
    ctah_horizontal_label_7b
        .calibrate_nusselt_correlation_for_fluid_within_pipe(ideal_nusselt_number_for_ctah);

    // for the heater, i also calibrate the Nusselt correlation by 5 times,
    // to prevent the steel from overheating due to high power
    //
    // nusselt number change and calibration should be easier though,
    // may want some quality of life improvements for user interface in future
    let heater_calibrated_nusselt_factor =
        Ratio::new::<ratio>(heater_calibrated_nusselt_factor_float);
    let mut heater_fluid_array_clone: FluidArray = heater_ver_1_15_nodes
        .pipe_fluid_array
        .clone()
        .try_into()
        .unwrap();

    calibrate_nusselt_correlation_of_heat_transfer_entity(
        &mut heater_fluid_array_clone.nusselt_correlation,
        heater_calibrated_nusselt_factor,
    );

    heater_ver_1_15_nodes.pipe_fluid_array = heater_fluid_array_clone.into();

    // now calibrate the insulation thickness for all

    dhx_sthe.calibrate_insulation_thickness(dhx_calibrated_insulation_thickness);
    // pri loop cold leg
    static_mixer_20_label_23.calibrate_insulation_thickness(pri_loop_cold_leg_insulation_thickness);
    pipe_23a.calibrate_insulation_thickness(pri_loop_cold_leg_insulation_thickness);
    pipe_22.calibrate_insulation_thickness(pri_loop_cold_leg_insulation_thickness);
    pipe_21.calibrate_insulation_thickness(pri_loop_cold_leg_insulation_thickness);
    // note that flowmeter is considered not insulated
    pipe_20.calibrate_insulation_thickness(pri_loop_cold_leg_insulation_thickness);
    pipe_19.calibrate_insulation_thickness(pri_loop_cold_leg_insulation_thickness);
    pipe_17b.calibrate_insulation_thickness(pri_loop_cold_leg_insulation_thickness);
    pipe_18.calibrate_insulation_thickness(pri_loop_cold_leg_insulation_thickness);

    let bottom_head_pipe_length = Length::new::<meter>(0.19685);
    let bottom_head_insulation_id =
        Length::new::<inch>(1.51) + 2.0 * Length::new::<meter>(0.001905);
    heater_bottom_head_1b.calibrate_insulation_thickness(
        bottom_head_pipe_length,
        bottom_head_insulation_id,
        pri_loop_cold_leg_insulation_thickness,
    );

    // pri loop hot leg

    let top_head_pipe_length = Length::new::<meter>(0.0889);
    let top_head_insulation_id = Length::new::<inch>(1.51) + 2.0 * Length::new::<meter>(0.001905);

    heater_top_head_1a.calibrate_insulation_thickness(
        top_head_pipe_length,
        top_head_insulation_id,
        pri_loop_hot_leg_insulation_thickness,
    );
    static_mixer_10_label_2.calibrate_insulation_thickness(pri_loop_hot_leg_insulation_thickness);
    pipe_2a.calibrate_insulation_thickness(pri_loop_hot_leg_insulation_thickness);
    pipe_3.calibrate_insulation_thickness(pri_loop_hot_leg_insulation_thickness);
    pipe_4.calibrate_insulation_thickness(pri_loop_hot_leg_insulation_thickness);
    pipe_5a.calibrate_insulation_thickness(pri_loop_hot_leg_insulation_thickness);
    pipe_26.calibrate_insulation_thickness(pri_loop_hot_leg_insulation_thickness);
    pipe_25a.calibrate_insulation_thickness(pri_loop_hot_leg_insulation_thickness);
    static_mixer_21_label_25.calibrate_insulation_thickness(pri_loop_hot_leg_insulation_thickness);

    // dracs loop cold leg

    static_mixer_60_label_36
        .calibrate_insulation_thickness(dracs_loop_cold_leg_insulation_thickness);
    pipe_36a.calibrate_insulation_thickness(dracs_loop_cold_leg_insulation_thickness);
    pipe_37.calibrate_insulation_thickness(dracs_loop_cold_leg_insulation_thickness);
    pipe_38.calibrate_insulation_thickness(dracs_loop_cold_leg_insulation_thickness);
    pipe_39.calibrate_insulation_thickness(dracs_loop_cold_leg_insulation_thickness);

    // dracs loop hot leg

    pipe_31a.calibrate_insulation_thickness(dracs_loop_hot_leg_insulation_thickness);
    static_mixer_61_label_31
        .calibrate_insulation_thickness(dracs_loop_hot_leg_insulation_thickness);
    pipe_32.calibrate_insulation_thickness(dracs_loop_hot_leg_insulation_thickness);
    pipe_33.calibrate_insulation_thickness(dracs_loop_hot_leg_insulation_thickness);
    pipe_34.calibrate_insulation_thickness(dracs_loop_hot_leg_insulation_thickness);

    // ctah branch
    pipe_5b.calibrate_insulation_thickness(ctah_branch_insulation_thickness_pre_ctah);
    static_mixer_41_label_6
        .calibrate_insulation_thickness(ctah_branch_insulation_thickness_pre_ctah);
    pipe_6a.calibrate_insulation_thickness(ctah_branch_insulation_thickness_pre_ctah);
    pipe_8a.calibrate_insulation_thickness(ctah_branch_insulation_thickness_post_ctah);
    static_mixer_40_label_8
        .calibrate_insulation_thickness(ctah_branch_insulation_thickness_post_ctah);
    pipe_9.calibrate_insulation_thickness(ctah_branch_insulation_thickness_post_ctah);
    pipe_10.calibrate_insulation_thickness(ctah_branch_insulation_thickness_post_ctah);
    pipe_11.calibrate_insulation_thickness(ctah_branch_insulation_thickness_post_ctah);
    pipe_12.calibrate_insulation_thickness(ctah_branch_insulation_thickness_post_ctah);
    pipe_13.calibrate_insulation_thickness(ctah_branch_insulation_thickness_post_ctah);
    pipe_14.calibrate_insulation_thickness(ctah_branch_insulation_thickness_post_ctah);
    pipe_15.calibrate_insulation_thickness(ctah_branch_insulation_thickness_post_ctah);
    pipe_16.calibrate_insulation_thickness(ctah_branch_insulation_thickness_post_ctah);
    pipe_17a.calibrate_insulation_thickness(ctah_branch_insulation_thickness_post_ctah);

    let mass_flowrate_dhx_br: MassRate = MassRate::ZERO;
    let mass_flowrate_dracs_loop_abs: MassRate = MassRate::ZERO;
    let mass_flowrate_ctah_br: MassRate = MassRate::ZERO;
    let mass_flowrate_heater_br: MassRate = MassRate::ZERO;

        Self {
            tchx_pid_controller,
            fresh_tchx_pid_controller,
            ctah_pid_controller,
            fresh_ctah_pid_controller,
            current_simulation_time,
            pipe_34,
            pipe_33,
            pipe_32,
            pipe_31a,
            static_mixer_61_label_31,
            dhx_tube_side_30b,
            dhx_sthe,
            dhx_tube_side_30a,
            tchx_35a,
            tchx_35b_1,
            tchx_35b_2,
            static_mixer_60_label_36,
            pipe_36a,
            pipe_37,
            flowmeter_60_37a,
            pipe_38,
            pipe_39,
            pipe_5a,
            pipe_26,
            pipe_25a,
            static_mixer_21_label_25,
            pipe_23a,
            static_mixer_20_label_23,
            pipe_22,
            flowmeter_20_21a,
            pipe_21,
            pipe_20,
            pipe_19,
            pipe_17b,
            pipe_4,
            pipe_3,
            pipe_2a,
            static_mixer_10_label_2,
            heater_top_head_1a,
            heater_ver_1_15_nodes,
            heater_ver_1_8_nodes,
            heater_bottom_head_1b,
            pipe_18,
            pipe_5b,
            static_mixer_41_label_6,
            pipe_6a,
            ctah_vertical_label_7a,
            ctah_horizontal_label_7b,
            pipe_8a,
            static_mixer_40_label_8,
            pipe_9,
            pipe_10,
            pipe_11,
            pipe_12,
            ctah_pump,
            pipe_13,
            pipe_14,
            flowmeter_40_14a,
            pipe_15,
            pipe_16,
            pipe_17a,
            top_mixing_node_5a_5b_4,
            bottom_mixing_node_17a_17b_18,
            mass_flowrate_dhx_br,
            mass_flowrate_dracs_loop_abs,
            mass_flowrate_ctah_br,
            mass_flowrate_heater_br,
        }
    }

    /// The DRACS loop's flow rate magnitude from the loop as it stands
    /// (v1's DRACS flow thread, moved here unchanged, gh:#924).
    pub fn dracs_flow(&self) -> MassRate {
            let dhx_tube_side_heat_exchanger_30 =
                self.dhx_sthe.get_clone_of_tube_side_parallel_tube_fluid_component();
            let absolute_mass_flowrate_dracs =
                coupled_dracs_fluid_mechanics_calc_abs_mass_rate_sam_tchx_calibration(
                    &self.pipe_34,
                    &self.pipe_33,
                    &self.pipe_32,
                    &self.pipe_31a,
                    &self.static_mixer_61_label_31,
                    &self.dhx_tube_side_30b,
                    &dhx_tube_side_heat_exchanger_30,
                    &self.dhx_tube_side_30a,
                    &self.tchx_35a,
                    &self.tchx_35b_1,
                    &self.tchx_35b_2,
                    &self.static_mixer_60_label_36,
                    &self.pipe_36a,
                    &self.pipe_37,
                    &self.flowmeter_60_37a,
                    &self.pipe_38,
                    &self.pipe_39,
                );

            absolute_mass_flowrate_dracs
    }

    /// The primary loop's (DHX, heater, CTAH) branch flow rates from the
    /// loop as it stands, at this pump pressure rise, these valves and this
    /// heater mesh (v1's primary flow thread, moved here unchanged,
    /// gh:#924).
    pub fn primary_flows(
        &self,
        pump_pressure: Pressure,
        ctah_branch_blocked: bool,
        dhx_branch_blocked: bool,
        current_heater_type: HeaterType,
    ) -> (MassRate, MassRate, MassRate) {
            //
            // first, absolute mass flowrate across two branches
            let dhx_shell_side_pipe_24 =
                self.dhx_sthe.get_clone_of_shell_side_fluid_component();
            // flow should go from up to down
            // this was tested ok

            match current_heater_type {
                HeaterType::InsulatedHeaterV1Fine15Mesh => {
                    let (dhx_flow, heater_flow, ctah_flow) =
                        three_branch_pri_loop_flowrates_parallel_ver_4(
                            pump_pressure,
                            ctah_branch_blocked,
                            dhx_branch_blocked,
                            &self.pipe_4,
                            &self.pipe_3,
                            &self.pipe_2a,
                            &self.static_mixer_10_label_2,
                            &self.heater_top_head_1a,
                            &self.heater_ver_1_15_nodes,
                            &self.heater_bottom_head_1b,
                            &self.pipe_18,
                            &self.pipe_5a,
                            &self.pipe_26,
                            &self.pipe_25a,
                            &self.static_mixer_21_label_25,
                            &dhx_shell_side_pipe_24,
                            &self.static_mixer_20_label_23,
                            &self.pipe_23a,
                            &self.pipe_22,
                            &self.flowmeter_20_21a,
                            &self.pipe_21,
                            &self.pipe_20,
                            &self.pipe_19,
                            &self.pipe_17b,
                            &self.pipe_5b,
                            &self.static_mixer_41_label_6,
                            &self.pipe_6a,
                            &self.ctah_vertical_label_7a,
                            &self.ctah_horizontal_label_7b,
                            &self.pipe_8a,
                            &self.static_mixer_40_label_8,
                            &self.pipe_9,
                            &self.pipe_10,
                            &self.pipe_11,
                            &self.pipe_12,
                            &self.ctah_pump,
                            &self.pipe_13,
                            &self.pipe_14,
                            &self.flowmeter_40_14a,
                            &self.pipe_15,
                            &self.pipe_16,
                            &self.pipe_17a,
                        );

                    (dhx_flow, heater_flow, ctah_flow)
                }
                HeaterType::InsulatedHeaterV1Coarse8Mesh => {
                    let (dhx_flow, heater_flow, ctah_flow) =
                        three_branch_pri_loop_flowrates_parallel_ver_4(
                            pump_pressure,
                            ctah_branch_blocked,
                            dhx_branch_blocked,
                            &self.pipe_4,
                            &self.pipe_3,
                            &self.pipe_2a,
                            &self.static_mixer_10_label_2,
                            &self.heater_top_head_1a,
                            &self.heater_ver_1_8_nodes,
                            &self.heater_bottom_head_1b,
                            &self.pipe_18,
                            &self.pipe_5a,
                            &self.pipe_26,
                            &self.pipe_25a,
                            &self.static_mixer_21_label_25,
                            &dhx_shell_side_pipe_24,
                            &self.static_mixer_20_label_23,
                            &self.pipe_23a,
                            &self.pipe_22,
                            &self.flowmeter_20_21a,
                            &self.pipe_21,
                            &self.pipe_20,
                            &self.pipe_19,
                            &self.pipe_17b,
                            &self.pipe_5b,
                            &self.static_mixer_41_label_6,
                            &self.pipe_6a,
                            &self.ctah_vertical_label_7a,
                            &self.ctah_horizontal_label_7b,
                            &self.pipe_8a,
                            &self.static_mixer_40_label_8,
                            &self.pipe_9,
                            &self.pipe_10,
                            &self.pipe_11,
                            &self.pipe_12,
                            &self.ctah_pump,
                            &self.pipe_13,
                            &self.pipe_14,
                            &self.flowmeter_40_14a,
                            &self.pipe_15,
                            &self.pipe_16,
                            &self.pipe_17a,
                        );

                    (dhx_flow, heater_flow, ctah_flow)
                }
            }
    }

    /// The fluid arrays the DRACS flow solve reads, in a fixed order.
    fn dracs_fluid_arrays(&self) -> [&HeatTransferEntity; 17] {
        [
            &self.pipe_34.pipe_fluid_array,
            &self.pipe_33.pipe_fluid_array,
            &self.pipe_32.pipe_fluid_array,
            &self.pipe_31a.pipe_fluid_array,
            &self.static_mixer_61_label_31.pipe_fluid_array,
            &self.dhx_tube_side_30b.pipe_fluid_array,
            &self.dhx_sthe.tube_side_fluid_array_for_single_tube,
            &self.dhx_tube_side_30a.pipe_fluid_array,
            &self.tchx_35a.pipe_fluid_array,
            &self.tchx_35b_1.pipe_fluid_array,
            &self.tchx_35b_2.pipe_fluid_array,
            &self.static_mixer_60_label_36.pipe_fluid_array,
            &self.pipe_36a.pipe_fluid_array,
            &self.pipe_37.pipe_fluid_array,
            &self.flowmeter_60_37a.pipe_fluid_array,
            &self.pipe_38.pipe_fluid_array,
            &self.pipe_39.pipe_fluid_array,
        ]
    }

    fn dracs_fluid_arrays_mut(&mut self) -> [&mut HeatTransferEntity; 17] {
        [
            &mut self.pipe_34.pipe_fluid_array,
            &mut self.pipe_33.pipe_fluid_array,
            &mut self.pipe_32.pipe_fluid_array,
            &mut self.pipe_31a.pipe_fluid_array,
            &mut self.static_mixer_61_label_31.pipe_fluid_array,
            &mut self.dhx_tube_side_30b.pipe_fluid_array,
            &mut self.dhx_sthe.tube_side_fluid_array_for_single_tube,
            &mut self.dhx_tube_side_30a.pipe_fluid_array,
            &mut self.tchx_35a.pipe_fluid_array,
            &mut self.tchx_35b_1.pipe_fluid_array,
            &mut self.tchx_35b_2.pipe_fluid_array,
            &mut self.static_mixer_60_label_36.pipe_fluid_array,
            &mut self.pipe_36a.pipe_fluid_array,
            &mut self.pipe_37.pipe_fluid_array,
            &mut self.flowmeter_60_37a.pipe_fluid_array,
            &mut self.pipe_38.pipe_fluid_array,
            &mut self.pipe_39.pipe_fluid_array,
        ]
    }

    /// The fluid arrays the primary flow solve reads (both heater meshes),
    /// in a fixed order.
    fn primary_fluid_arrays(&self) -> [&HeatTransferEntity; 40] {
        [
            &self.pipe_4.pipe_fluid_array,
            &self.pipe_3.pipe_fluid_array,
            &self.pipe_2a.pipe_fluid_array,
            &self.static_mixer_10_label_2.pipe_fluid_array,
            &self.heater_top_head_1a.pipe_fluid_array,
            &self.heater_ver_1_15_nodes.pipe_fluid_array,
            &self.heater_ver_1_8_nodes.pipe_fluid_array,
            &self.heater_bottom_head_1b.pipe_fluid_array,
            &self.pipe_18.pipe_fluid_array,
            &self.pipe_5a.pipe_fluid_array,
            &self.pipe_26.pipe_fluid_array,
            &self.pipe_25a.pipe_fluid_array,
            &self.static_mixer_21_label_25.pipe_fluid_array,
            &self.dhx_sthe.shell_side_fluid_array,
            &self.static_mixer_20_label_23.pipe_fluid_array,
            &self.pipe_23a.pipe_fluid_array,
            &self.pipe_22.pipe_fluid_array,
            &self.flowmeter_20_21a.pipe_fluid_array,
            &self.pipe_21.pipe_fluid_array,
            &self.pipe_20.pipe_fluid_array,
            &self.pipe_19.pipe_fluid_array,
            &self.pipe_17b.pipe_fluid_array,
            &self.pipe_5b.pipe_fluid_array,
            &self.static_mixer_41_label_6.pipe_fluid_array,
            &self.pipe_6a.pipe_fluid_array,
            &self.ctah_vertical_label_7a.pipe_fluid_array,
            &self.ctah_horizontal_label_7b.pipe_fluid_array,
            &self.pipe_8a.pipe_fluid_array,
            &self.static_mixer_40_label_8.pipe_fluid_array,
            &self.pipe_9.pipe_fluid_array,
            &self.pipe_10.pipe_fluid_array,
            &self.pipe_11.pipe_fluid_array,
            &self.pipe_12.pipe_fluid_array,
            &self.ctah_pump.pipe_fluid_array,
            &self.pipe_13.pipe_fluid_array,
            &self.pipe_14.pipe_fluid_array,
            &self.flowmeter_40_14a.pipe_fluid_array,
            &self.pipe_15.pipe_fluid_array,
            &self.pipe_16.pipe_fluid_array,
            &self.pipe_17a.pipe_fluid_array,
        ]
    }

    fn primary_fluid_arrays_mut(&mut self) -> [&mut HeatTransferEntity; 40] {
        [
            &mut self.pipe_4.pipe_fluid_array,
            &mut self.pipe_3.pipe_fluid_array,
            &mut self.pipe_2a.pipe_fluid_array,
            &mut self.static_mixer_10_label_2.pipe_fluid_array,
            &mut self.heater_top_head_1a.pipe_fluid_array,
            &mut self.heater_ver_1_15_nodes.pipe_fluid_array,
            &mut self.heater_ver_1_8_nodes.pipe_fluid_array,
            &mut self.heater_bottom_head_1b.pipe_fluid_array,
            &mut self.pipe_18.pipe_fluid_array,
            &mut self.pipe_5a.pipe_fluid_array,
            &mut self.pipe_26.pipe_fluid_array,
            &mut self.pipe_25a.pipe_fluid_array,
            &mut self.static_mixer_21_label_25.pipe_fluid_array,
            &mut self.dhx_sthe.shell_side_fluid_array,
            &mut self.static_mixer_20_label_23.pipe_fluid_array,
            &mut self.pipe_23a.pipe_fluid_array,
            &mut self.pipe_22.pipe_fluid_array,
            &mut self.flowmeter_20_21a.pipe_fluid_array,
            &mut self.pipe_21.pipe_fluid_array,
            &mut self.pipe_20.pipe_fluid_array,
            &mut self.pipe_19.pipe_fluid_array,
            &mut self.pipe_17b.pipe_fluid_array,
            &mut self.pipe_5b.pipe_fluid_array,
            &mut self.static_mixer_41_label_6.pipe_fluid_array,
            &mut self.pipe_6a.pipe_fluid_array,
            &mut self.ctah_vertical_label_7a.pipe_fluid_array,
            &mut self.ctah_horizontal_label_7b.pipe_fluid_array,
            &mut self.pipe_8a.pipe_fluid_array,
            &mut self.static_mixer_40_label_8.pipe_fluid_array,
            &mut self.pipe_9.pipe_fluid_array,
            &mut self.pipe_10.pipe_fluid_array,
            &mut self.pipe_11.pipe_fluid_array,
            &mut self.pipe_12.pipe_fluid_array,
            &mut self.ctah_pump.pipe_fluid_array,
            &mut self.pipe_13.pipe_fluid_array,
            &mut self.pipe_14.pipe_fluid_array,
            &mut self.flowmeter_40_14a.pipe_fluid_array,
            &mut self.pipe_15.pipe_fluid_array,
            &mut self.pipe_16.pipe_fluid_array,
            &mut self.pipe_17a.pipe_fluid_array,
        ]
    }

    /// What the flow solves read from this loop now, as plain numbers a
    /// Web Worker can be sent (gh:#924): every fluid temperature, K, of the
    /// components each solve reads, and the pump and valve settings of
    /// `local_ciet_state`. Taken at the start of a step and solved by
    /// another loop's [`CietLoop::solve_dracs_snapshot`] and
    /// [`CietLoop::solve_primary_snapshot`], it gives that step's flows.
    pub fn flow_snapshot(&self, local_ciet_state: &CIETState) -> FlowSnapshot {
        let mut dracs = Vec::new();
        for hte in self.dracs_fluid_arrays() {
            push_temperatures(hte, &mut dracs);
        }
        let mut primary = vec![
            local_ciet_state.get_ctah_pump_pressure_f64(),
            f64::from(u8::from(local_ciet_state.is_ctah_branch_blocked)),
            f64::from(u8::from(local_ciet_state.is_dhx_branch_blocked)),
            f64::from(u8::from(matches!(
                local_ciet_state.current_heater_type,
                HeaterType::InsulatedHeaterV1Coarse8Mesh
            ))),
        ];
        for hte in self.primary_fluid_arrays() {
            push_temperatures(hte, &mut primary);
        }
        FlowSnapshot { dracs, primary }
    }

    /// Set this loop's DRACS fluid temperatures from a
    /// [`FlowSnapshot::dracs`] and solve the DRACS flow, kg/s.
    ///
    /// # Errors
    ///
    /// The snapshot does not fit this loop's arrays.
    pub fn solve_dracs_snapshot(&mut self, dracs: &[f64]) -> Result<f64, String> {
        let mut rest = dracs;
        for hte in self.dracs_fluid_arrays_mut() {
            rest = set_temperatures(hte, rest)?;
        }
        if !rest.is_empty() {
            return Err(format!("{} DRACS temperatures left over", rest.len()));
        }
        Ok(self.dracs_flow().get::<kilogram_per_second>())
    }

    /// Set this loop's primary fluid temperatures from a
    /// [`FlowSnapshot::primary`] and solve the (DHX, heater, CTAH) branch
    /// flows, kg/s, at the pump and valve settings it carries.
    ///
    /// # Errors
    ///
    /// The snapshot does not fit this loop's arrays.
    pub fn solve_primary_snapshot(&mut self, primary: &[f64]) -> Result<[f64; 3], String> {
        let (controls, mut rest) = primary.split_at_checked(4).ok_or("primary snapshot too short")?;
        for hte in self.primary_fluid_arrays_mut() {
            rest = set_temperatures(hte, rest)?;
        }
        if !rest.is_empty() {
            return Err(format!("{} primary temperatures left over", rest.len()));
        }
        let heater = if controls[3] != 0.0 {
            HeaterType::InsulatedHeaterV1Coarse8Mesh
        } else {
            HeaterType::InsulatedHeaterV1Fine15Mesh
        };
        let (dhx, heater_flow, ctah) = self.primary_flows(
            Pressure::new::<pascal>(controls[0]),
            controls[1] != 0.0,
            controls[2] != 0.0,
            heater,
        );
        Ok([dhx, heater_flow, ctah].map(|m| m.get::<kilogram_per_second>()))
    }

    /// Simulation time reached so far.
    pub fn simulation_time(&self) -> Time {
        self.current_simulation_time
    }

    /// The flow rates the last step found, kg/s, unrounded: DHX branch,
    /// heater branch, CTAH branch (TUAS's signs), and the DRACS loop's
    /// magnitude (the DRACS solve returns an absolute value). The plant state
    /// rounds the first, third and fourth and does not carry the second; the
    /// browser demo draws flow directions from these.
    pub fn branch_flows_kg_per_s(&self) -> [f64; 4] {
        [
            self.mass_flowrate_dhx_br.get::<kilogram_per_second>(),
            self.mass_flowrate_heater_br.get::<kilogram_per_second>(),
            self.mass_flowrate_ctah_br.get::<kilogram_per_second>(),
            self.mass_flowrate_dracs_loop_abs.get::<kilogram_per_second>(),
        ]
    }

    /// Advance the loop by `timestep`: read the controls from
    /// `local_ciet_state`, integrate, and write the measurements back into it
    /// (v1's loop body, minus pacing). `verbose_temperatures` `dbg!`s the
    /// loop temperatures, as `--verbose-temperatures`.
    ///
    /// # Panics
    ///
    /// If a TUAS solver call fails (a bug, as in v1).
    pub fn step(&mut self, local_ciet_state: &mut CIETState, timestep: Time, verbose_temperatures: bool) {
        self.step_with(local_ciet_state, timestep, verbose_temperatures, FlowSource::Solve);
    }

    /// [`CietLoop::step`], with the flow rates it records for the next step
    /// either solved here (`FlowSource::Solve`, the desktop) or solved
    /// elsewhere from [`CietLoop::flow_snapshot`] taken at the start of this
    /// step and handed over by [`CietLoop::finish_step`] (`FlowSource::Later`,
    /// the browser's flow workers, which run while this step's heat
    /// transfer does).
    ///
    /// # Panics
    ///
    /// If a TUAS solver call fails (a bug, as in v1).
    pub fn step_with(
        &mut self,
        local_ciet_state: &mut CIETState,
        timestep: Time,
        verbose_temperatures: bool,
        flow_source: FlowSource,
    ) {
        // a copy of the loop before this step changes any component, for
        // the flow solves (gh:#924)
        let start_of_step = match flow_source {
            FlowSource::Solve => Some(Arc::new(self.clone())),
            FlowSource::Later => None,
        };
        let average_temperature_for_density_calcs =
            ThermodynamicTemperature::new::<degree_celsius>(80.0);
        let ambient_htc = HeatTransfer::new::<watt_per_square_meter_kelvin>(20.0);
        let dhx_heat_loss_to_ambient_watts_per_m2_kelvin = DHX_HEAT_LOSS_TO_AMBIENT_WATTS_PER_M2_KELVIN;
        let Self {
            tchx_pid_controller,
            fresh_tchx_pid_controller,
            ctah_pid_controller,
            fresh_ctah_pid_controller,
            current_simulation_time,
            pipe_34,
            pipe_33,
            pipe_32,
            pipe_31a,
            static_mixer_61_label_31,
            dhx_tube_side_30b,
            dhx_sthe,
            dhx_tube_side_30a,
            tchx_35a,
            tchx_35b_1,
            tchx_35b_2,
            static_mixer_60_label_36,
            pipe_36a,
            pipe_37,
            flowmeter_60_37a,
            pipe_38,
            pipe_39,
            pipe_5a,
            pipe_26,
            pipe_25a,
            static_mixer_21_label_25,
            pipe_23a,
            static_mixer_20_label_23,
            pipe_22,
            flowmeter_20_21a,
            pipe_21,
            pipe_20,
            pipe_19,
            pipe_17b,
            pipe_4,
            pipe_3,
            pipe_2a,
            static_mixer_10_label_2,
            heater_top_head_1a,
            heater_ver_1_15_nodes,
            heater_ver_1_8_nodes,
            heater_bottom_head_1b,
            pipe_18,
            pipe_5b,
            static_mixer_41_label_6,
            pipe_6a,
            ctah_vertical_label_7a,
            ctah_horizontal_label_7b,
            pipe_8a,
            static_mixer_40_label_8,
            pipe_9,
            pipe_10,
            pipe_11,
            pipe_12,
            ctah_pump,
            pipe_13,
            pipe_14,
            flowmeter_40_14a,
            pipe_15,
            pipe_16,
            pipe_17a,
            top_mixing_node_5a_5b_4,
            bottom_mixing_node_17a_17b_18,
            mass_flowrate_dhx_br,
            mass_flowrate_dracs_loop_abs,
            mass_flowrate_ctah_br,
            mass_flowrate_heater_br,
        } = self;
        if local_ciet_state
            .heater_control
            .advanced_heater_control_switched_on
        {
            let demanded = local_ciet_state
                .heater_control
                .demanded_power(*current_simulation_time);
            local_ciet_state.set_heater_power_kilowatts(demanded.get::<kilowatt>());
        }

        let input_power_kilowatts = local_ciet_state.get_heater_power_kilowatts();
        let input_power = Power::new::<kilowatt>(input_power_kilowatts);



        // this is a safety killswitch

        let heater_outlet_temp_degc = local_ciet_state.get_heater_outlet_temp_degc();

        let heater_inlet_temp_degc = local_ciet_state.get_heater_inlet_temp_degc();

        // from legacy killswitch
        //let heater_bulk_temp_degc =
        //    heater_ver_1.pipe_fluid_array.try_get_bulk_temperature()
        //    .unwrap()
        //    .get::<degree_celsius>();

        let tchx_heat_transfer_coeff: HeatTransfer;
        let mut heat_rate_through_heater;

        if heater_outlet_temp_degc > 150.0 {
            heat_rate_through_heater = Power::ZERO;
            local_ciet_state.set_heater_power_kilowatts(0.0);
        } else if heater_inlet_temp_degc > 150.0 {
            heat_rate_through_heater = Power::ZERO;
            local_ciet_state.set_heater_power_kilowatts(0.0);
        } else {
            heat_rate_through_heater = input_power;
        }

        // I'm going to check all arrays in the heater as well

        match local_ciet_state.current_heater_type {
            HeaterType::InsulatedHeaterV1Fine15Mesh => {
                let heater_temp_vec_degc: Vec<f64> = heater_ver_1_15_nodes
                    .pipe_fluid_array
                    .get_temperature_vector()
                    .unwrap()
                    .iter()
                    .map(|temp| temp.get::<degree_celsius>())
                    .collect();
                // killswitch if any heater cv exceeds 160.0 C
                for heater_cv_temperature_degc in heater_temp_vec_degc.iter() {
                    if *heater_cv_temperature_degc > 160.0 {
                        heat_rate_through_heater = Power::ZERO;
                        local_ciet_state.set_heater_power_kilowatts(0.0);
                    }
                }
                // killswitch if any heated pipe cv exceeds 350C
                //
                // this tends to happen when you reverse the pump in the
                // the heater fluid gets too hot and simulation crashes
                // forced circ loop such that you have opposing mixed convection

                let heater_surf_temp_vec_degc: Vec<f64> = heater_ver_1_15_nodes
                    .pipe_shell
                    .get_temperature_vector()
                    .unwrap()
                    .iter()
                    .map(|temp| temp.get::<degree_celsius>())
                    .collect();

                for heater_cv_temperature_degc in heater_surf_temp_vec_degc.iter() {
                    if *heater_cv_temperature_degc > 350.0 {
                        heat_rate_through_heater = Power::ZERO;
                        local_ciet_state.set_heater_power_kilowatts(0.0);
                    }
                }
            }
            HeaterType::InsulatedHeaterV1Coarse8Mesh => {
                let corase_mesh_heater_temp_vec_degc: Vec<f64> = heater_ver_1_8_nodes
                    .pipe_fluid_array
                    .get_temperature_vector()
                    .unwrap()
                    .iter()
                    .map(|temp| temp.get::<degree_celsius>())
                    .collect();

                for heater_cv_temperature_degc in corase_mesh_heater_temp_vec_degc.iter() {
                    if *heater_cv_temperature_degc > 160.0 {
                        heat_rate_through_heater = Power::ZERO;
                        local_ciet_state.set_heater_power_kilowatts(0.0);
                    }
                }

                let corase_mesh_heater_surf_temp_vec_degc: Vec<f64> = heater_ver_1_8_nodes
                    .pipe_shell
                    .get_temperature_vector()
                    .unwrap()
                    .iter()
                    .map(|temp| temp.get::<degree_celsius>())
                    .collect();
                for heater_cv_temperature_degc in corase_mesh_heater_surf_temp_vec_degc.iter() {
                    if *heater_cv_temperature_degc > 350.0 {
                        heat_rate_through_heater = Power::ZERO;
                        local_ciet_state.set_heater_power_kilowatts(0.0);
                    }
                }
            }
        }

        let tchx_outlet_temperature_set_point_degc =
            local_ciet_state.bt_66_tchx_outlet_set_pt_deg_c;

        let tchx_outlet_temperature_set_point =
            ThermodynamicTemperature::new::<degree_celsius>(tchx_outlet_temperature_set_point_degc);

        let ctah_outlet_temperature_set_point_degc =
            local_ciet_state.bt_41_ctah_outlet_set_pt_deg_c;


        let reference_tchx_and_ctah_htc = HeatTransfer::new::<watt_per_square_meter_kelvin>(40.0);

        let tchx_outlet_temperature: ThermodynamicTemperature = {
            // the front of the tchx is connected to static mixer
            // 60 label 36
            let tchx_35_b2_pipe_fluid_array_clone: FluidArray =
                tchx_35b_2.pipe_fluid_array.clone().try_into().unwrap();

            // take the front single cv temperature
            //
            // front single cv temperature is defunct
            // probably need to debug this

            let tchx_35_b2_front_single_cv_temperature: ThermodynamicTemperature =
                tchx_35_b2_pipe_fluid_array_clone
                    .front_single_cv
                    .temperature;

            let _tchx_35b_2_array_temperature: Vec<ThermodynamicTemperature> =
                tchx_35b_2.pipe_fluid_array_temperature().unwrap();

            //dbg!(&tchx_35b_array_temperature);

            tchx_35_b2_front_single_cv_temperature
        };
        // we will need to change the tchx heat transfer coefficient
        // using the PID controller
        //
        // record tchx outlet temperature if it is last 5s of time

        tchx_heat_transfer_coeff = {
            // first, calculate the set point error

            let reference_temperature_interval_deg_celsius = 80.0;
            // tchx outlet temperature
            let tchx_outlet_temperature_degc = local_ciet_state.get_tchx_outlet_temp_degc();

            // error = y_sp - y_measured
            let set_point_abs_error_deg_celsius = -tchx_outlet_temperature_set_point
                .get::<degree_celsius>()
                + tchx_outlet_temperature_degc;

            let nondimensional_error: Ratio = (set_point_abs_error_deg_celsius
                / reference_temperature_interval_deg_celsius)
                .into();

            // let's get the output

            let dimensionless_heat_trf_input: Ratio = tchx_pid_controller
                .set_user_input_and_calc(nondimensional_error, *current_simulation_time)
                .unwrap();

            // the dimensionless output is:
            //
            // (desired output - ref_val)/ref_val = dimensionless_input
            //
            //
            // the reference value is decided by the user
            // in this case 40 W/(m^2 K)

            let mut tchx_heat_trf_output = dimensionless_heat_trf_input
                * reference_tchx_and_ctah_htc
                + reference_tchx_and_ctah_htc;

            // make sure it cannot be less than a certain amount
            let tchx_minimum_heat_transfer = HeatTransfer::new::<watt_per_square_meter_kelvin>(5.0);

            let tchx_max_heat_transfer = HeatTransfer::new::<watt_per_square_meter_kelvin>(1e6f64);

            // this makes it physically realistic
            if tchx_heat_trf_output < tchx_minimum_heat_transfer {
                tchx_heat_trf_output = tchx_minimum_heat_transfer;

                // if the controller goes too long below the min heat
                // transfer, the integral reset time tends to respond very slowly
                // hence, i'd rather reset the controller if it goes below
                // the min heat transfer
                // so in this regard, it behaves like a ProportionalController
                *tchx_pid_controller = fresh_tchx_pid_controller.clone();
            } else if tchx_heat_trf_output > tchx_max_heat_transfer {
                // also do same reset behaviour if it gets too high
                tchx_heat_trf_output = tchx_max_heat_transfer;
                *tchx_pid_controller = fresh_tchx_pid_controller.clone();
            }

            tchx_heat_trf_output
        };

        // now let's caluculate the ctah heat trf coeff
        // first get the ctah outlet temperature at around
        // pipe 8a

        // this is buggy, got to find out why
        let ctah_heat_transfer_coeff: HeatTransfer = {
            // this assumes forward flow
            let mut ctah_outlet_temp_degc = pipe_8a
                .pipe_fluid_array
                .try_get_bulk_temperature()
                .unwrap()
                .get::<degree_celsius>();

            // if backflow, then we take pipe 6a to be outlet temp

            if local_ciet_state.fm40_ctah_branch_kg_per_s < 0.0 {
                ctah_outlet_temp_degc = pipe_6a
                    .pipe_fluid_array
                    .try_get_bulk_temperature()
                    .unwrap()
                    .get::<degree_celsius>();
            }

            let reference_temperature_interval_deg_celsius = 80.0;

            // error = y_sp - y_measured
            let set_point_abs_error_deg_celsius =
                -ctah_outlet_temperature_set_point_degc + ctah_outlet_temp_degc;

            let nondimensional_error: Ratio = (set_point_abs_error_deg_celsius
                / reference_temperature_interval_deg_celsius)
                .into();

            // let's get the output

            let dimensionless_heat_trf_input: Ratio = ctah_pid_controller
                .set_user_input_and_calc(nondimensional_error, *current_simulation_time)
                .unwrap();
            // the dimensionless output is:
            //
            // (desired output - ref_val)/ref_val = dimensionless_input
            //
            //
            // the reference value is decided by the user
            // in this case 40 W/(m^2 K)

            let mut ctah_heat_trf_output = dimensionless_heat_trf_input
                * reference_tchx_and_ctah_htc
                + reference_tchx_and_ctah_htc;

            // make sure it cannot be less than a certain amount
            let ctah_minimum_heat_transfer = HeatTransfer::new::<watt_per_square_meter_kelvin>(5.0);
            let ctah_max_heat_transfer = HeatTransfer::new::<watt_per_square_meter_kelvin>(20000.0);

            // this makes it physically realistic
            if ctah_heat_trf_output < ctah_minimum_heat_transfer {
                ctah_heat_trf_output = ctah_minimum_heat_transfer;
                // if the controller goes too long below the min heat
                // transfer, the integral reset time tends to respond very slowly
                // hence, i'd rather reset the controller if it goes below
                // the min heat transfer
                // so in this regard, it behaves like a ProportionalController
                *ctah_pid_controller = fresh_ctah_pid_controller.clone();
            } else if ctah_heat_trf_output > ctah_max_heat_transfer {
                ctah_heat_trf_output = ctah_max_heat_transfer;
                *ctah_pid_controller = fresh_ctah_pid_controller.clone();
            }

            ctah_heat_trf_output
        };
        // fluid calculation loop
        //
        // gh:#924: the two flow solves are `dracs_flow` and `primary_flows`,
        // run on `start_of_step`, a copy of the loop taken before anything
        // in this step changed a component (v1 cloned every component here
        // instead; the controllers above change no component, so the copies
        // hold the same values). Their flows are used on the NEXT step. With
        // `FlowSource::Given` the solves run elsewhere (the browser's flow
        // workers) on a `FlowSnapshot` of the same start-of-step state.

        // pump pressure and valve settings, read from ciet state
        let pump_pressure = Pressure::new::<pascal>(local_ciet_state.get_ctah_pump_pressure_f64());
        let ctah_branch_blocked = local_ciet_state.is_ctah_branch_blocked;
        let dhx_branch_blocked = local_ciet_state.is_dhx_branch_blocked;
        let current_heater_type = local_ciet_state.current_heater_type;

        let flow_jobs = match flow_source {
            FlowSource::Solve => {
                let start_of_step = start_of_step.expect("Solve copies the loop");
                let for_dracs = start_of_step.clone();
                let for_primary = start_of_step;
                Some((
                    fork(move || for_dracs.dracs_flow()),
                    fork(move || {
                        for_primary.primary_flows(
                            pump_pressure,
                            ctah_branch_blocked,
                            dhx_branch_blocked,
                            current_heater_type,
                        )
                    }),
                ))
            }
            FlowSource::Later => None,
        };

        // likely the natural circulation is counter clockwise
        // now, set flowrate using the global flowrate first
        let counter_clockwise_dracs_flowrate = *mass_flowrate_dracs_loop_abs;
        let dhx_flow = *mass_flowrate_dhx_br;
        let heater_flow = *mass_flowrate_heater_br;
        let ctah_flow = *mass_flowrate_ctah_br;
        // next,
        // link up the heat transfer entities
        // all lateral linking is done except for DHX
        //
        // note, the ambient heat transfer coefficient is not set for
        // the DHX sthe
        coupled_dracs_loop_link_up_components_sam_tchx_calibration(
            counter_clockwise_dracs_flowrate,
            tchx_heat_transfer_coeff,
            average_temperature_for_density_calcs,
            ambient_htc,
            pipe_34,
            pipe_33,
            pipe_32,
            pipe_31a,
            static_mixer_61_label_31,
            dhx_tube_side_30b,
            dhx_sthe,
            dhx_tube_side_30a,
            tchx_35a,
            tchx_35b_1,
            tchx_35b_2,
            static_mixer_60_label_36,
            pipe_36a,
            pipe_37,
            flowmeter_60_37a,
            pipe_38,
            pipe_39,
        );

        //dbg!(&(dhx_flow,heater_flow,ctah_flow));

        match local_ciet_state.current_heater_type {
            HeaterType::InsulatedHeaterV1Fine15Mesh => {
                ciet_pri_loop_three_branch_link_up_components_ver_4(
                    dhx_flow,
                    heater_flow,
                    ctah_flow,
                    heat_rate_through_heater,
                    average_temperature_for_density_calcs,
                    ambient_htc,
                    ctah_heat_transfer_coeff,
                    pipe_4,
                    pipe_3,
                    pipe_2a,
                    static_mixer_10_label_2,
                    heater_top_head_1a,
                    heater_ver_1_15_nodes,
                    heater_bottom_head_1b,
                    pipe_18,
                    pipe_5a,
                    pipe_26,
                    pipe_25a,
                    static_mixer_21_label_25,
                    dhx_sthe,
                    static_mixer_20_label_23,
                    pipe_23a,
                    pipe_22,
                    flowmeter_20_21a,
                    pipe_21,
                    pipe_20,
                    pipe_19,
                    pipe_17b,
                    pipe_5b,
                    static_mixer_41_label_6,
                    pipe_6a,
                    ctah_vertical_label_7a,
                    ctah_horizontal_label_7b,
                    pipe_8a,
                    static_mixer_40_label_8,
                    pipe_9,
                    pipe_10,
                    pipe_11,
                    pipe_12,
                    ctah_pump,
                    pipe_13,
                    pipe_14,
                    flowmeter_40_14a,
                    pipe_15,
                    pipe_16,
                    pipe_17a,
                    top_mixing_node_5a_5b_4,
                    bottom_mixing_node_17a_17b_18,
                );
            }
            HeaterType::InsulatedHeaterV1Coarse8Mesh => {
                ciet_pri_loop_three_branch_link_up_components_ver_4(
                    dhx_flow,
                    heater_flow,
                    ctah_flow,
                    heat_rate_through_heater,
                    average_temperature_for_density_calcs,
                    ambient_htc,
                    ctah_heat_transfer_coeff,
                    pipe_4,
                    pipe_3,
                    pipe_2a,
                    static_mixer_10_label_2,
                    heater_top_head_1a,
                    heater_ver_1_8_nodes,
                    heater_bottom_head_1b,
                    pipe_18,
                    pipe_5a,
                    pipe_26,
                    pipe_25a,
                    static_mixer_21_label_25,
                    dhx_sthe,
                    static_mixer_20_label_23,
                    pipe_23a,
                    pipe_22,
                    flowmeter_20_21a,
                    pipe_21,
                    pipe_20,
                    pipe_19,
                    pipe_17b,
                    pipe_5b,
                    static_mixer_41_label_6,
                    pipe_6a,
                    ctah_vertical_label_7a,
                    ctah_horizontal_label_7b,
                    pipe_8a,
                    static_mixer_40_label_8,
                    pipe_9,
                    pipe_10,
                    pipe_11,
                    pipe_12,
                    ctah_pump,
                    pipe_13,
                    pipe_14,
                    flowmeter_40_14a,
                    pipe_15,
                    pipe_16,
                    pipe_17a,
                    top_mixing_node_5a_5b_4,
                    bottom_mixing_node_17a_17b_18,
                );
            }
        }

        // need to calibrate dhx sthe ambient htc
        // because the coupled_dracs_pri_loop_dhx_heater_link_up_components
        // function sets the heat transfer to ambient
        dhx_sthe.heat_transfer_to_ambient = HeatTransfer::new::<watt_per_square_meter_kelvin>(
            dhx_heat_loss_to_ambient_watts_per_m2_kelvin,
        );

        // calibrate heater to ambient htc
        heater_ver_1_15_nodes.heat_transfer_to_ambient =
            HeatTransfer::new::<watt_per_square_meter_kelvin>(6.0);
        heater_ver_1_8_nodes.heat_transfer_to_ambient =
            HeatTransfer::new::<watt_per_square_meter_kelvin>(6.0);

        // todo: need to advance timestep here

        // advance timestep
        dracs_loop_advance_timestep_except_dhx_sam_tchx_calibration(
            timestep,
            pipe_34,
            pipe_33,
            pipe_32,
            pipe_31a,
            static_mixer_61_label_31,
            dhx_tube_side_30b,
            dhx_tube_side_30a,
            tchx_35a,
            tchx_35b_1,
            tchx_35b_2,
            static_mixer_60_label_36,
            pipe_36a,
            pipe_37,
            flowmeter_60_37a,
            pipe_38,
            pipe_39,
        );

        // pri_loop_advance_timestep_dhx_br_and_heater_br_except_dhx(
        //     timestep, pipe_4, pipe_3, pipe_2a,
        //     static_mixer_10_label_2, heater_top_head_1a,
        //     &mut heater_ver_1, heater_bottom_head_1b,
        //     pipe_18, pipe_5a, pipe_26, pipe_25a,
        //     static_mixer_21_label_25, static_mixer_20_label_23,
        //     pipe_23a, pipe_22, flowmeter_20_21a,
        //     pipe_21, pipe_20, pipe_19, pipe_17b);

        match local_ciet_state.current_heater_type {
            HeaterType::InsulatedHeaterV1Fine15Mesh => {
                pri_loop_three_branch_advance_timestep_except_dhx_ver_4(
                    timestep,
                    pipe_4,
                    pipe_3,
                    pipe_2a,
                    static_mixer_10_label_2,
                    heater_top_head_1a,
                    heater_ver_1_15_nodes,
                    heater_bottom_head_1b,
                    pipe_18,
                    pipe_5a,
                    pipe_26,
                    pipe_25a,
                    static_mixer_21_label_25,
                    static_mixer_20_label_23,
                    pipe_23a,
                    pipe_22,
                    flowmeter_20_21a,
                    pipe_21,
                    pipe_20,
                    pipe_19,
                    pipe_17b,
                    pipe_5b,
                    static_mixer_41_label_6,
                    pipe_6a,
                    ctah_vertical_label_7a,
                    ctah_horizontal_label_7b,
                    pipe_8a,
                    static_mixer_40_label_8,
                    pipe_9,
                    pipe_10,
                    pipe_11,
                    pipe_12,
                    ctah_pump,
                    pipe_13,
                    pipe_14,
                    flowmeter_40_14a,
                    pipe_15,
                    pipe_16,
                    pipe_17a,
                    top_mixing_node_5a_5b_4,
                    bottom_mixing_node_17a_17b_18,
                );
            }
            HeaterType::InsulatedHeaterV1Coarse8Mesh => {
                pri_loop_three_branch_advance_timestep_except_dhx_ver_4(
                    timestep,
                    pipe_4,
                    pipe_3,
                    pipe_2a,
                    static_mixer_10_label_2,
                    heater_top_head_1a,
                    heater_ver_1_8_nodes,
                    heater_bottom_head_1b,
                    pipe_18,
                    pipe_5a,
                    pipe_26,
                    pipe_25a,
                    static_mixer_21_label_25,
                    static_mixer_20_label_23,
                    pipe_23a,
                    pipe_22,
                    flowmeter_20_21a,
                    pipe_21,
                    pipe_20,
                    pipe_19,
                    pipe_17b,
                    pipe_5b,
                    static_mixer_41_label_6,
                    pipe_6a,
                    ctah_vertical_label_7a,
                    ctah_horizontal_label_7b,
                    pipe_8a,
                    static_mixer_40_label_8,
                    pipe_9,
                    pipe_10,
                    pipe_11,
                    pipe_12,
                    ctah_pump,
                    pipe_13,
                    pipe_14,
                    flowmeter_40_14a,
                    pipe_15,
                    pipe_16,
                    pipe_17a,
                    top_mixing_node_5a_5b_4,
                    bottom_mixing_node_17a_17b_18,
                );
            }
        }

        // for dhx, a little more care is needed to do the
        // lateral and misc connections and advance timestep
        // advance timestep
        //
        // by default, dhx flowrate is downwards in this setup

        let prandtl_wall_correction_setting = true;
        let tube_side_total_mass_flowrate = -counter_clockwise_dracs_flowrate;
        let shell_side_total_mass_flowrate = dhx_flow;

        dhx_sthe
            .lateral_and_miscellaneous_connections(
                prandtl_wall_correction_setting,
                tube_side_total_mass_flowrate,
                shell_side_total_mass_flowrate,
            )
            .unwrap();

        dhx_sthe.advance_timestep(timestep).unwrap();

        // join the mass flow calculation handles, and record the flows
        // (gh:#924: or take the flows solved elsewhere)
        // (with `FlowSource::Later` they stay as they are until `finish_step`)
        let solved_flows = flow_jobs.map(|(dracs_job, primary_job)| {
            let (dhx, heater, ctah) = primary_job.join();
            GivenFlows { dracs: dracs_job.join(), dhx_branch: dhx, heater_branch: heater, ctah_branch: ctah }
        });

        // display temperatures and stuff
        //
        // v2: v1 hardcoded this `true`, which `dbg!`s the whole loop's
        // temperatures on every timestep. Off unless the user asks
        // (`--verbose-temperatures`) -- in headless mode it would bury the
        // status line.
        let display_temperatures = verbose_temperatures;
        // temperatures before and after heater
        let ((bt_11, _wt_10), (bt_12, _wt_13)) = pri_loop_heater_temperature_diagnostics_ver_4(
            heater_bottom_head_1b,
            static_mixer_10_label_2,
            display_temperatures,
        );
        // temperatures before and after dhx shell
        let ((bt_21, _wt_20), (bt_27, _wt_26)) = pri_loop_dhx_shell_temperature_diagnostics(
            pipe_25a,
            static_mixer_20_label_23,
            display_temperatures,
        );
        // temperatures before and after dhx tube
        let ((_bt_21, _wt_20), (_bt_27, _wt_26)) = dracs_loop_dhx_tube_temperature_diagnostics(
            dhx_tube_side_30a,
            dhx_tube_side_30b,
            display_temperatures,
        );
        // heater average surface temp

        match local_ciet_state.current_heater_type {
            HeaterType::InsulatedHeaterV1Fine15Mesh => {
                let heater_avg_surf_temp: ThermodynamicTemperature = heater_ver_1_15_nodes
                    .pipe_shell
                    .try_get_bulk_temperature()
                    .unwrap();

                let _simulated_heater_avg_surf_temp_degc: f64 =
                    heater_avg_surf_temp.get::<degree_celsius>();
            }
            HeaterType::InsulatedHeaterV1Coarse8Mesh => {
                let heater_avg_surf_temp: ThermodynamicTemperature = heater_ver_1_8_nodes
                    .pipe_shell
                    .try_get_bulk_temperature()
                    .unwrap();

                let _simulated_heater_avg_surf_temp_degc: f64 =
                    heater_avg_surf_temp.get::<degree_celsius>();
            }
        }

        // update the local ciet state
        // update to 2dp

        local_ciet_state.bt_66_tchx_outlet_deg_c =
            (tchx_outlet_temperature.get::<degree_celsius>() * 100.0).round() / 100.0;

        local_ciet_state.bt_11_heater_inlet_deg_c =
            (bt_11.get::<degree_celsius>() * 100.0).round() / 100.0;

        local_ciet_state.bt_12_heater_outlet_deg_c =
            (bt_12.get::<degree_celsius>() * 100.0).round() / 100.0;

        // primary loop DHX plus heater branch update
        {
            let pipe_1a_temp = *heater_top_head_1a
                .pipe_fluid_array_temperature()
                .first()
                .unwrap();

            local_ciet_state.pipe_1a_temp_degc = pipe_1a_temp.get::<degree_celsius>() as f32;

            let pipe_1b_temp = *heater_bottom_head_1b
                .pipe_fluid_array_temperature()
                .first()
                .unwrap();

            local_ciet_state.pipe_1b_temp_degc = pipe_1b_temp.get::<degree_celsius>() as f32;

            let pipe_2a_temp = *pipe_2a
                .pipe_fluid_array_temperature()
                .unwrap()
                .first()
                .unwrap();

            local_ciet_state.pipe_2a_temp_degc = pipe_2a_temp.get::<degree_celsius>() as f32;

            let pipe_2_temp = *static_mixer_10_label_2
                .pipe_fluid_array_temperature()
                .unwrap()
                .first()
                .unwrap();

            local_ciet_state.pipe_2_temp_degc = pipe_2_temp.get::<degree_celsius>() as f32;

            let pipe_3_temp = *pipe_3
                .pipe_fluid_array_temperature()
                .unwrap()
                .first()
                .unwrap();

            local_ciet_state.pipe_3_temp_degc = pipe_3_temp.get::<degree_celsius>() as f32;

            let pipe_4_temp = *pipe_4
                .pipe_fluid_array_temperature()
                .unwrap()
                .first()
                .unwrap();

            local_ciet_state.pipe_4_temp_degc = pipe_4_temp.get::<degree_celsius>() as f32;

            let pipe_18_temp = *pipe_18
                .pipe_fluid_array_temperature()
                .unwrap()
                .first()
                .unwrap();

            local_ciet_state.pipe_18_temp_degc = pipe_18_temp.get::<degree_celsius>() as f32;

            // dhx branch

            // note: need f32 for colour, because floats used to calculate
            // rgb are all in f32

            let pipe_17b_temp = *pipe_17b
                .pipe_fluid_array_temperature()
                .unwrap()
                .first()
                .unwrap();

            local_ciet_state.pipe_17b_temp_degc = pipe_17b_temp.get::<degree_celsius>() as f32;

            let pipe_19_temp = *pipe_19
                .pipe_fluid_array_temperature()
                .unwrap()
                .first()
                .unwrap();

            local_ciet_state.pipe_19_temp_degc = pipe_19_temp.get::<degree_celsius>() as f32;

            let pipe_20_temp = *pipe_20
                .pipe_fluid_array_temperature()
                .unwrap()
                .first()
                .unwrap();

            local_ciet_state.pipe_20_temp_degc = pipe_20_temp.get::<degree_celsius>() as f32;

            let pipe_21_temp = *pipe_21
                .pipe_fluid_array_temperature()
                .unwrap()
                .first()
                .unwrap();

            local_ciet_state.pipe_21_temp_degc = pipe_21_temp.get::<degree_celsius>() as f32;

            // fm20 (21a)
            // mass flowrate at 3 dp

            let pipe_21a_temp = *flowmeter_20_21a
                .pipe_fluid_array_temperature()
                .unwrap()
                .first()
                .unwrap();

            local_ciet_state.fm20_label_21a_temp_degc =
                pipe_21a_temp.get::<degree_celsius>() as f32;

            // (gh:#924: FM-20, FM-60 and FM-40 are written by
            // `finish_step`, so the browser's split step writes them the same way)

            let pipe_22_temp = *pipe_22
                .pipe_fluid_array_temperature()
                .unwrap()
                .first()
                .unwrap();

            local_ciet_state.pipe_22_temp_degc = pipe_22_temp.get::<degree_celsius>() as f32;

            let pipe_23a_temp = *pipe_23a
                .pipe_fluid_array_temperature()
                .unwrap()
                .first()
                .unwrap();

            local_ciet_state.pipe_23a_temp_degc = pipe_23a_temp.get::<degree_celsius>() as f32;

            local_ciet_state.bt_21_dhx_shell_inlet_deg_c = bt_21.get::<degree_celsius>();
            local_ciet_state.pipe_25a_temp_degc =
                local_ciet_state.bt_21_dhx_shell_inlet_deg_c as f32;

            local_ciet_state.bt_27_dhx_shell_outlet_deg_c = bt_27.get::<degree_celsius>();
            local_ciet_state.pipe_23_temp_degc =
                local_ciet_state.bt_27_dhx_shell_outlet_deg_c as f32;

            let pipe_25_temp = *static_mixer_21_label_25
                .pipe_fluid_array_temperature()
                .unwrap()
                .first()
                .unwrap();

            local_ciet_state.pipe_25_temp_degc = pipe_25_temp.get::<degree_celsius>() as f32;

            let pipe_26_temp = *pipe_26
                .pipe_fluid_array_temperature()
                .unwrap()
                .first()
                .unwrap();

            local_ciet_state.pipe_26_temp_degc = pipe_26_temp.get::<degree_celsius>() as f32;

            let pipe_5a_temp = *pipe_5a
                .pipe_fluid_array_temperature()
                .unwrap()
                .first()
                .unwrap();

            local_ciet_state.pipe_5a_temp_degc = pipe_5a_temp.get::<degree_celsius>() as f32;
        }
        // DRACS loop update
        {
            let pipe_30a_temp = *dhx_tube_side_30a
                .pipe_fluid_array_temperature()
                .unwrap()
                .first()
                .unwrap();

            local_ciet_state.pipe_30a_temp_degc = pipe_30a_temp.get::<degree_celsius>() as f32;

            local_ciet_state.bt_60_dhx_tube_inlet_deg_c = pipe_30a_temp.get::<degree_celsius>();

            let pipe_30b_temp = *dhx_tube_side_30b
                .pipe_fluid_array_temperature()
                .unwrap()
                .first()
                .unwrap();

            local_ciet_state.pipe_30b_temp_degc = pipe_30b_temp.get::<degree_celsius>() as f32;

            local_ciet_state.bt_21_dhx_tube_outlet_deg_c = pipe_30b_temp.get::<degree_celsius>();

            let pipe_31_temp = *static_mixer_61_label_31
                .pipe_fluid_array_temperature()
                .unwrap()
                .first()
                .unwrap();

            local_ciet_state.pipe_31_temp_degc = pipe_31_temp.get::<degree_celsius>() as f32;

            let pipe_31a_temp = *pipe_31a
                .pipe_fluid_array_temperature()
                .unwrap()
                .first()
                .unwrap();

            local_ciet_state.pipe_31a_temp_degc = pipe_31a_temp.get::<degree_celsius>() as f32;

            let pipe_32_temp = *pipe_32
                .pipe_fluid_array_temperature()
                .unwrap()
                .first()
                .unwrap();

            local_ciet_state.pipe_32_temp_degc = pipe_32_temp.get::<degree_celsius>() as f32;

            let pipe_33_temp = *pipe_33
                .pipe_fluid_array_temperature()
                .unwrap()
                .first()
                .unwrap();

            local_ciet_state.pipe_33_temp_degc = pipe_33_temp.get::<degree_celsius>() as f32;

            let pipe_34_temp = *pipe_34
                .pipe_fluid_array_temperature()
                .unwrap()
                .first()
                .unwrap();

            local_ciet_state.pipe_34_temp_degc = pipe_34_temp.get::<degree_celsius>() as f32;

            local_ciet_state.bt_65_tchx_inlet_deg_c =
                ((local_ciet_state.pipe_34_temp_degc * 100.0).round() as f64) / 100.0_f64;
            // tchx htc update
            local_ciet_state.tchx_htc_watt_per_m2_kelvin =
                (tchx_heat_transfer_coeff.get::<watt_per_square_meter_kelvin>() * 100.0).round()
                    / 100.0;

            let pipe_36a_temp = *pipe_36a
                .pipe_fluid_array_temperature()
                .unwrap()
                .first()
                .unwrap();

            local_ciet_state.pipe_36a_temp_degc = pipe_36a_temp.get::<degree_celsius>() as f32;

            let pipe_36_temp = *static_mixer_60_label_36
                .pipe_fluid_array_temperature()
                .unwrap()
                .first()
                .unwrap();

            local_ciet_state.pipe_36_temp_degc = pipe_36_temp.get::<degree_celsius>() as f32;

            let pipe_37_temp = *pipe_37
                .pipe_fluid_array_temperature()
                .unwrap()
                .first()
                .unwrap();

            local_ciet_state.pipe_37_temp_degc = pipe_37_temp.get::<degree_celsius>() as f32;

            // fm60 (37a)
            // 3dp

            let pipe_21a_temp = *flowmeter_60_37a
                .pipe_fluid_array_temperature()
                .unwrap()
                .first()
                .unwrap();

            local_ciet_state.fm60_label_37a_temp_degc =
                pipe_21a_temp.get::<degree_celsius>() as f32;

            // 4dp preferred

            let pipe_38_temp = *pipe_38
                .pipe_fluid_array_temperature()
                .unwrap()
                .first()
                .unwrap();

            local_ciet_state.pipe_38_temp_degc = pipe_38_temp.get::<degree_celsius>() as f32;

            let pipe_39_temp = *pipe_39
                .pipe_fluid_array_temperature()
                .unwrap()
                .first()
                .unwrap();

            local_ciet_state.pipe_39_temp_degc = pipe_39_temp.get::<degree_celsius>() as f32;
        }

        // still need to do ctah branch update
        {
            let pipe_5b_temp = *pipe_5b
                .pipe_fluid_array_temperature()
                .unwrap()
                .first()
                .unwrap();

            local_ciet_state.pipe_5b_temp_degc = pipe_5b_temp.get::<degree_celsius>() as f32;

            let pipe_6a_temp = *pipe_6a
                .pipe_fluid_array_temperature()
                .unwrap()
                .first()
                .unwrap();

            // 6a and inlet bt-43 are about the same

            local_ciet_state.pipe_6a_temp_degc = pipe_6a_temp.get::<degree_celsius>() as f32;

            local_ciet_state.bt_43_ctah_inlet_deg_c =
                (pipe_6a_temp.get::<degree_celsius>() * 100.0).round() / 100.0;

            let pipe_6_temp = *static_mixer_41_label_6
                .pipe_fluid_array_temperature()
                .unwrap()
                .first()
                .unwrap();
            // update ctah htc

            local_ciet_state.ctah_htc_watt_per_m2_kelvin =
                (ctah_heat_transfer_coeff.get::<watt_per_square_meter_kelvin>() * 100.0).round()
                    / 100.0;

            local_ciet_state.pipe_6_temp_degc = pipe_6_temp.get::<degree_celsius>() as f32;

            let pipe_8a_temp = *pipe_8a
                .pipe_fluid_array_temperature()
                .unwrap()
                .first()
                .unwrap();

            // pipe 8a and bt41 are about the same
            local_ciet_state.pipe_8a_temp_degc = pipe_8a_temp.get::<degree_celsius>() as f32;

            local_ciet_state.bt_41_ctah_outlet_deg_c =
                (pipe_8a_temp.get::<degree_celsius>() * 100.0).round() / 100.0;

            let pipe_8_temp = *static_mixer_40_label_8
                .pipe_fluid_array_temperature()
                .unwrap()
                .first()
                .unwrap();

            local_ciet_state.pipe_8_temp_degc = pipe_8_temp.get::<degree_celsius>() as f32;

            let pipe_9_temp = *pipe_9
                .pipe_fluid_array_temperature()
                .unwrap()
                .first()
                .unwrap();

            local_ciet_state.pipe_9_temp_degc = pipe_9_temp.get::<degree_celsius>() as f32;

            let pipe_10_temp = *pipe_10
                .pipe_fluid_array_temperature()
                .unwrap()
                .first()
                .unwrap();

            local_ciet_state.pipe_10_temp_degc = pipe_10_temp.get::<degree_celsius>() as f32;

            let pipe_11_temp = *pipe_11
                .pipe_fluid_array_temperature()
                .unwrap()
                .first()
                .unwrap();

            local_ciet_state.pipe_11_temp_degc = pipe_11_temp.get::<degree_celsius>() as f32;

            let pipe_12_temp = *pipe_12
                .pipe_fluid_array_temperature()
                .unwrap()
                .first()
                .unwrap();

            local_ciet_state.pipe_12_temp_degc = pipe_12_temp.get::<degree_celsius>() as f32;

            // ctah pump temp

            let ctah_pump_temp = *ctah_pump
                .pipe_fluid_array_temperature()
                .unwrap()
                .first()
                .unwrap();

            local_ciet_state.ctah_pump_temp_degc = ctah_pump_temp.get::<degree_celsius>();

            let pipe_13_temp = *pipe_13
                .pipe_fluid_array_temperature()
                .unwrap()
                .first()
                .unwrap();

            local_ciet_state.pipe_13_temp_degc = pipe_13_temp.get::<degree_celsius>() as f32;

            let pipe_14_temp = *pipe_14
                .pipe_fluid_array_temperature()
                .unwrap()
                .first()
                .unwrap();

            local_ciet_state.pipe_14_temp_degc = pipe_14_temp.get::<degree_celsius>() as f32;

            let pipe_14a_temp = *flowmeter_40_14a
                .pipe_fluid_array_temperature()
                .unwrap()
                .first()
                .unwrap();

            local_ciet_state.fm40_label_14a_temp_degc =
                pipe_14a_temp.get::<degree_celsius>() as f32;


            let pipe_15_temp = *pipe_15
                .pipe_fluid_array_temperature()
                .unwrap()
                .first()
                .unwrap();

            local_ciet_state.pipe_15_temp_degc = pipe_15_temp.get::<degree_celsius>() as f32;

            let pipe_16_temp = *pipe_16
                .pipe_fluid_array_temperature()
                .unwrap()
                .first()
                .unwrap();

            local_ciet_state.pipe_16_temp_degc = pipe_16_temp.get::<degree_celsius>() as f32;

            let pipe_17a_temp = *pipe_17a
                .pipe_fluid_array_temperature()
                .unwrap()
                .first()
                .unwrap();

            local_ciet_state.pipe_17a_temp_degc = pipe_17a_temp.get::<degree_celsius>() as f32;
        }

        // lastly, mixing node update

        {
            let top_mixing_node_5a_5b_4_temp =
                top_mixing_node_5a_5b_4.try_get_bulk_temperature().unwrap();

            local_ciet_state.top_mixing_node_5a_5b_4_temp_degc =
                top_mixing_node_5a_5b_4_temp.get::<degree_celsius>() as f32;

            let bottom_mixing_node_17a_17b_18_temp = bottom_mixing_node_17a_17b_18
                .try_get_bulk_temperature()
                .unwrap();

            local_ciet_state.bottom_mixing_node_17a_17b_18_temp_degc =
                bottom_mixing_node_17a_17b_18_temp.get::<degree_celsius>() as f32;
        }

        *current_simulation_time += timestep;
        let simulation_time_seconds = current_simulation_time.get::<second>();
        local_ciet_state.simulation_time_seconds = (simulation_time_seconds * 10.0).round() / 10.0;

        if let Some(flows) = solved_flows {
            self.finish_step(local_ciet_state, flows);
        }
    }

    /// Record the flow rates this step solved, for the next step, and write
    /// FM-20, FM-60 and FM-40 into `local_ciet_state` (v1's rounding). Called
    /// by the step itself with `FlowSource::Solve`; with `FlowSource::Later`
    /// the caller calls it with the flows solved from this step's
    /// [`CietLoop::flow_snapshot`], before the next step (gh:#924).
    pub fn finish_step(&mut self, local_ciet_state: &mut CIETState, flows: GivenFlows) {
        self.mass_flowrate_dracs_loop_abs = flows.dracs;
        self.mass_flowrate_dhx_br = flows.dhx_branch;
        self.mass_flowrate_ctah_br = flows.ctah_branch;
        self.mass_flowrate_heater_br = flows.heater_branch;

        local_ciet_state.fm20_dhx_branch_kg_per_s =
            ((self.mass_flowrate_dhx_br.get::<kilogram_per_second>() * 10000.0).round() / 10000.0)
                as f32;
        local_ciet_state.fm_60_dracs_kg_per_s =
            (self.mass_flowrate_dracs_loop_abs.get::<kilogram_per_second>() * 10000.0).round()
                / 10000.0;
        local_ciet_state.fm40_ctah_branch_kg_per_s =
            (self.mass_flowrate_ctah_br.get::<kilogram_per_second>() * 10000.0).round() / 10000.0;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The plant the tests run: 8 kW in the heater, the pump off (so the
    /// primary loop and the DRACS run on natural circulation), every branch
    /// open, the fine heater mesh.
    fn heated_plant() -> CIETState {
        let mut state = CIETState::default();
        state.set_heater_power_kilowatts(8.0);
        state
    }

    /// Run `steps` timesteps of 0.1 s; return the state after each and the
    /// wall time per step.
    fn run(steps: usize) -> (Vec<CIETState>, f64) {
        let mut state = heated_plant();
        let mut ciet = CietLoop::new(&state);
        let start = std::time::Instant::now();
        let mut trace = Vec::with_capacity(steps);
        for _ in 0..steps {
            ciet.step(&mut state, max_timestep(), false);
            trace.push(state.clone());
        }
        (trace, start.elapsed().as_secs_f64() / steps as f64)
    }

    /// **The stepper is deterministic, and running the flow solves inline
    /// (as on `wasm32`) gives the same plant as running them on threads**
    /// (gh:#924).
    ///
    /// Methodology: the heated plant above, 300 steps of 0.1 s (30 s of
    /// simulation) three times: twice with the flow solves on threads, once
    /// inline (`fork::set_inline`). Pass: every field of the plant state is
    /// equal after every step, in all three runs. A sanity check that the
    /// physics ran: the heater outlet is above its inlet and the primary loop
    /// circulates naturally through the DHX branch at the end. (The DRACS is
    /// still at rest at 30 s: no heat has reached the DHX from a 21 degC
    /// start. I first asserted a DRACS flow, and that was wrong.) Also prints the wall time per step, the
    /// real-time budget being 100 ms.
    ///
    /// Result, 2026-10-17 (release, this machine): see the gh:#924 record.
    #[test]
    fn the_stepper_is_deterministic_and_inline_equals_threaded() {
        let steps = 300;
        let (threaded, t_threaded) = run(steps);
        let (again, _) = run(steps);
        super::super::fork::set_inline(true);
        let (inline, t_inline) = run(steps);
        super::super::fork::set_inline(false);
        assert_eq!(threaded, again, "two threaded runs differ");
        for (i, (a, b)) in threaded.iter().zip(&inline).enumerate() {
            assert_eq!(a, b, "inline and threaded differ at step {}", i + 1);
        }
        let last = threaded.last().expect("steps");
        assert!(last.bt_12_heater_outlet_deg_c > last.bt_11_heater_inlet_deg_c, "{last:?}");
        assert!(last.fm20_dhx_branch_kg_per_s > 0.0, "{last:?}");
        eprintln!(
            "CIET stepper, {steps} steps of 0.1 s: threaded {:.2} ms/step, inline {:.2} ms/step; \
             at the end heater {:.2} -> {:.2} degC, DRACS {:.4} kg/s, DHX branch {:.4}, CTAH branch {:.4} kg/s",
            t_threaded * 1e3,
            t_inline * 1e3,
            last.bt_11_heater_inlet_deg_c,
            last.bt_12_heater_outlet_deg_c,
            last.fm_60_dracs_kg_per_s,
            last.fm20_dhx_branch_kg_per_s,
            last.fm40_ctah_branch_kg_per_s,
        );
    }

    /// The heated plant with the pump switched to 2 kPa at 15 s, so the run
    /// has natural and forced circulation; `split` solves the flows on two
    /// other loops from snapshots (as the browser's flow workers do).
    fn run_with_pump(steps: usize, split: bool) -> (Vec<CIETState>, f64) {
        let mut state = heated_plant();
        let mut ciet = CietLoop::new(&state);
        let mut dracs_worker = CietLoop::new(&state);
        let mut primary_worker = CietLoop::new(&state);
        let start = std::time::Instant::now();
        let mut trace = Vec::with_capacity(steps);
        for i in 0..steps {
            if i == 150 {
                state.set_ctah_pump_pressure_pascals(2000.0);
            }
            if split {
                let snap = ciet.flow_snapshot(&state);
                let dracs = dracs_worker.solve_dracs_snapshot(&snap.dracs).expect("dracs snapshot");
                let primary = primary_worker.solve_primary_snapshot(&snap.primary).expect("primary snapshot");
                ciet.step_with(&mut state, max_timestep(), false, FlowSource::Later);
                ciet.finish_step(&mut state, GivenFlows::from_kg_per_s(dracs, primary));
            } else {
                ciet.step(&mut state, max_timestep(), false);
            }
            trace.push(state.clone());
        }
        (trace, start.elapsed().as_secs_f64() / steps as f64)
    }

    /// **Flows solved on other loops from a snapshot are the flows solved
    /// in the step** (gh:#924): what makes the browser's separate flow
    /// workers the same simulator as the desktop's flow threads.
    ///
    /// Methodology: 300 steps of 0.1 s, natural circulation then the pump
    /// at 2 kPa from 15 s, once with the flows solved in the step and once
    /// with each step's [`CietLoop::flow_snapshot`] solved by two other
    /// `CietLoop`s (one for the DRACS, one for the primary loop) and handed
    /// back after the step's heat transfer ([`FlowSource::Later`], then
    /// [`CietLoop::finish_step`]). Pass: every field of the plant state
    /// equal after every step. Also prints the cost per step of the second
    /// way done serially, and the snapshot sizes.
    ///
    /// Result, 2026-10-17 (release): see the gh:#924 record.
    #[test]
    fn flows_solved_from_snapshots_are_the_flows_solved_in_the_step() {
        let steps = 300;
        let (inside, t_inside) = run_with_pump(steps, false);
        let (split, t_split) = run_with_pump(steps, true);
        for (i, (a, b)) in inside.iter().zip(&split).enumerate() {
            assert_eq!(a, b, "snapshot flows differ at step {}", i + 1);
        }
        let last = inside.last().expect("steps");
        assert!(last.fm40_ctah_branch_kg_per_s > 0.05, "the pump drives the CTAH branch: {last:?}");
        let state = heated_plant();
        let snap = CietLoop::new(&state).flow_snapshot(&state);
        eprintln!(
            "CIET flows from snapshots: identical over {steps} steps; in-step {:.2} ms/step, \
             snapshot route serially {:.2} ms/step; snapshot {} + {} numbers; \
             at the end CTAH branch {:.4} kg/s, DHX branch {:.4} kg/s",
            t_inside * 1e3,
            t_split * 1e3,
            snap.dracs.len(),
            snap.primary.len(),
            last.fm40_ctah_branch_kg_per_s,
            last.fm20_dhx_branch_kg_per_s,
        );
    }
}
