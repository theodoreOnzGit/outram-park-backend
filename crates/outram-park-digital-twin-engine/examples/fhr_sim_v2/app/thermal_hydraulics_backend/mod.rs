use std::sync::{Arc, Mutex};

use std::thread;
use std::time::{Duration, SystemTime};

use fhr_thermal_hydraulics_state::FHRThermalHydraulicsState;
use ndarray::{Array, Array1};
use tampines_steam_tables::steam_turbine_equations::ThreePhaseElectricGeneratorTurbine;
use tuas_boussinesq_solver::boussinesq_thermophysical_properties::LiquidMaterial;
use tuas_boussinesq_solver::pre_built_components::shell_and_tube_heat_exchanger::SimpleShellAndTubeHeatExchanger;
use tuas_boussinesq_solver::prelude::beta_testing::{FluidArray, HeatTransferEntity, HeatTransferInteractionType};
use uom::si::angular_velocity::revolution_per_minute;
use uom::si::energy::kilojoule;
use uom::si::mass_rate::kilogram_per_second;
use uom::si::power::megawatt;
use uom::si::pressure::{bar, kilopascal};
use uom::si::thermal_conductance::watt_per_kelvin;
//use teh_o_prke::decay_heat::DecayHeat;
//use teh_o_prke::feedback_mechanisms::fission_product_poisons::Xenon135Poisoning;
//use teh_o_prke::zero_power_prke::six_group::FissioningNuclideType;
//use teh_o_prke::{feedback_mechanisms::SixFactorFormulaFeedback, zero_power_prke::six_group::SixGroupPRKE};
//use uom::si::area::square_meter;
//use uom::si::energy::{kilojoule, megaelectronvolt};
//use uom::si::heat_transfer::watt_per_square_meter_kelvin;
//use uom::si::linear_number_density::per_meter;
//use uom::si::mass::kilogram;
//use uom::si::power::megawatt;
use uom::si::time::{microsecond, second};
//use uom::si::velocity::meter_per_second;
//use uom::si::volume::cubic_meter;
//use uom::si::volumetric_number_rate::per_cubic_meter_second;
use uom::si::f64::*;
//use uom::si::ratio::ratio;
use uom::si::thermodynamic_temperature::degree_celsius;
use uom::ConstZero;
use uom::si::electrical_resistance::ohm;


use components::*;
use pri_loop_fluid_mechanics_calc_fns::four_branch_pri_and_intermediate_loop_fluid_mechanics_only;
use tuas_boussinesq_solver::pre_built_components::insulated_pipes_and_fluid_components::InsulatedFluidComponent;
use tuas_boussinesq_solver::pre_built_components::non_insulated_fluid_components::NonInsulatedFluidComponent;
use crate::app::thermal_hydraulics_backend::secondary_loop::pool_boiling::pool_boiling_improvised_correlation_as_fraction_of_maximum;
use crate::app::thermal_hydraulics_backend::secondary_loop::steam_generator_duty::{
    feedwater_inlet_state, pinch_limited_steam_generator_duty, FeedwaterInletState,
    SteamGeneratorDuty, SteamGeneratorDutyLimit,
};
use crate::app::thermal_hydraulics_backend::salt_freeze_guard::SaltFreezeMonitor;
use crate::app::thermal_hydraulics_backend::secondary_loop::SecondaryLoopState;
use crate::{FHRSimulatorApp, FHRState};



impl FHRSimulatorApp {

    /// for the gFHR primary loop, and intermediate loop 
    /// there are four branches that need to be solved for flowrate 
    ///
    /// this code handles the solution procedure
    /// using the tuas_boussinesq_solver library code
    ///
    /// and handles fluid mechanics and heat transfer for one time step
    pub(crate) fn four_branch_pri_and_intermediate_loop_single_time_step(
        pri_loop_pump_pressure: Pressure,
        intrmd_loop_pump_pressure: Pressure,
        reactor_power: Power,
        timestep: Time,
        // diagnostics 
        simulation_time: Time,
        // reactor branch
        reactor_pipe_1: &mut InsulatedFluidComponent,
        // downcomer branch 1
        downcomer_pipe_2: &mut InsulatedFluidComponent,
        // downcomer branch 2
        downcomer_pipe_3: &mut InsulatedFluidComponent,
        // mixing nodes for pri loop
        bottom_mixing_node_pri_loop: &mut HeatTransferEntity,
        top_mixing_node_pri_loop: &mut HeatTransferEntity,
        // Intermediate heat exchanger branch in pri loop
        fhr_pipe_11: &mut InsulatedFluidComponent,
        fhr_pipe_10: &mut InsulatedFluidComponent,
        fhr_pri_loop_pump_9: &mut NonInsulatedFluidComponent,
        fhr_pipe_8: &mut InsulatedFluidComponent,
        fhr_pipe_7: &mut InsulatedFluidComponent,
        ihx_sthe_6: &mut SimpleShellAndTubeHeatExchanger,
        fhr_pipe_5: &mut InsulatedFluidComponent,
        fhr_pipe_4: &mut InsulatedFluidComponent,
        // intermediate loop ihx side
        fhr_pipe_17: &mut InsulatedFluidComponent,
        fhr_pipe_12: &mut InsulatedFluidComponent,
        // intermediate loop steam generator side
        fhr_intrmd_loop_pump_16: &mut NonInsulatedFluidComponent,
        fhr_pipe_15: &mut InsulatedFluidComponent,
        fhr_steam_generator_shell_side_14: &mut NonInsulatedFluidComponent,
        fhr_pipe_13: &mut InsulatedFluidComponent,
        // mixing nodes for intermediate loop
        bottom_mixing_node_intrmd_loop: &mut HeatTransferEntity,
        top_mixing_node_intrmd_loop: &mut HeatTransferEntity,
        // steam generator settings
        steam_generator_tube_side_temperature: ThermodynamicTemperature,
        steam_generator_overall_ua: ThermalConductance,
        // Secondary-side inlet conditions, needed so the steam-generator duty
        // can be clamped to the thermodynamic maximum
        // `Q_max = C_min (T_salt_in - T_feed_in)` rather than run open-loop off
        // the `UA` slider. See
        // `secondary_loop::steam_generator_duty` for why this is a physics
        // requirement and not a cosmetic one.
        feedwater_inlet: FeedwaterInletState,
        feedwater_mass_flowrate: MassRate,

        ) -> FHRThermalHydraulicsState {

            // fluid mechnaics portion for both loops


            let (reactor_branch_flow, downcomer_branch_1_flow,
                downcomer_branch_2_flow, pri_loop_intermediate_heat_exchanger_branch_flow,
                intrmd_loop_ihx_br_flow, intrmd_loop_steam_gen_br_flow)
                = four_branch_pri_and_intermediate_loop_fluid_mechanics_only(
                    pri_loop_pump_pressure, 
                    intrmd_loop_pump_pressure, 
                    reactor_pipe_1, 
                    downcomer_pipe_2, 
                    downcomer_pipe_3, 
                    fhr_pipe_11, 
                    fhr_pipe_10, 
                    fhr_pri_loop_pump_9, 
                    fhr_pipe_8, 
                    fhr_pipe_7, 
                    ihx_sthe_6, 
                    fhr_pipe_5, 
                    fhr_pipe_4, 
                    fhr_pipe_17, 
                    fhr_pipe_12, 
                    fhr_intrmd_loop_pump_16, 
                    fhr_pipe_15, 
                    fhr_steam_generator_shell_side_14, 
                    fhr_pipe_13);

            // thermal hydraulics part
            //
            // first, we are going to make heat transfer interactions
            // need rough temperature for density calcs, not super 
            // important as we assume boussineq approximation 
            // ie density differences only important for buoyancy calcs
            let average_temperature_for_density_calcs_pri_loop = 
                ThermodynamicTemperature::new::<degree_celsius>(600.0);


            let average_flibe_density = 
                LiquidMaterial::FLiBe.try_get_density(
                    average_temperature_for_density_calcs_pri_loop).unwrap();

            let downcomer_branch_1_advection_heat_transfer_interaction = 
                HeatTransferInteractionType::
                new_advection_interaction(downcomer_branch_1_flow, 
                    average_flibe_density, 
                    average_flibe_density);

            let downcomer_branch_2_advection_heat_transfer_interaction = 
                HeatTransferInteractionType::
                new_advection_interaction(downcomer_branch_2_flow, 
                    average_flibe_density, 
                    average_flibe_density);

            let reactor_branch_advection_heat_transfer_interaction = 
                HeatTransferInteractionType::
                new_advection_interaction(reactor_branch_flow, 
                    average_flibe_density, 
                    average_flibe_density);

            let ihx_advection_heat_transfer_interaction = 
                HeatTransferInteractionType::
                new_advection_interaction(pri_loop_intermediate_heat_exchanger_branch_flow, 
                    average_flibe_density, 
                    average_flibe_density);
            // for intermediate loop, we use lower temp, 
            // about 450 C
            //
            // as it is a HITEC salt (nitrate salt)
            let average_temperature_for_density_calcs_intrmd_loop = 
                ThermodynamicTemperature::new::<degree_celsius>(450.0);

            let average_hitec_density = 
                LiquidMaterial::HITEC.try_get_density(
                    average_temperature_for_density_calcs_intrmd_loop).unwrap();

            let intrmd_loop_ihx_br_heat_transfer_interaction = 
                HeatTransferInteractionType::
                new_advection_interaction(intrmd_loop_ihx_br_flow, 
                    average_hitec_density, 
                    average_hitec_density);
            let intrmd_loop_steam_gen_br_heat_transfer_interaction = 
                HeatTransferInteractionType::
                new_advection_interaction(intrmd_loop_steam_gen_br_flow, 
                    average_hitec_density, 
                    average_hitec_density);

            // note that reactor branch flow, 
            // downcomer_branch_1_flow, 
            // downcomer_branch_1_flow and 
            // intermediate_heat_exchanger_branch_flow in the pri loop 
            // all go from bottom mixing node to top mixing node
            //
            // with this in mind, we now link up the components 

            // downcomer 1 branch
            {
                bottom_mixing_node_pri_loop.link_to_front(
                    &mut downcomer_pipe_2.pipe_fluid_array, 
                    downcomer_branch_1_advection_heat_transfer_interaction)
                    .unwrap();

                downcomer_pipe_2.pipe_fluid_array.link_to_front(
                    top_mixing_node_pri_loop, 
                    downcomer_branch_1_advection_heat_transfer_interaction)
                    .unwrap();

                }
            // downcomer 2 branch
            {
                bottom_mixing_node_pri_loop.link_to_front(
                    &mut downcomer_pipe_3.pipe_fluid_array, 
                    downcomer_branch_2_advection_heat_transfer_interaction)
                    .unwrap();

                downcomer_pipe_3.pipe_fluid_array.link_to_front(
                    top_mixing_node_pri_loop, 
                    downcomer_branch_2_advection_heat_transfer_interaction)
                    .unwrap();
                }
            // pri loop 
            // ihx branch 
            {

                bottom_mixing_node_pri_loop.link_to_front(
                    &mut fhr_pipe_11.pipe_fluid_array, 
                    ihx_advection_heat_transfer_interaction)
                    .unwrap();

                fhr_pipe_11.pipe_fluid_array.link_to_front(
                    &mut fhr_pipe_10.pipe_fluid_array, 
                    ihx_advection_heat_transfer_interaction)
                    .unwrap();

                fhr_pipe_10.pipe_fluid_array.link_to_front(
                    &mut fhr_pri_loop_pump_9.pipe_fluid_array, 
                    ihx_advection_heat_transfer_interaction)
                    .unwrap();

                fhr_pri_loop_pump_9.pipe_fluid_array.link_to_front(
                    &mut fhr_pipe_8.pipe_fluid_array, 
                    ihx_advection_heat_transfer_interaction)
                    .unwrap();

                fhr_pipe_8.pipe_fluid_array.link_to_front(
                    &mut fhr_pipe_7.pipe_fluid_array, 
                    ihx_advection_heat_transfer_interaction)
                    .unwrap();

                fhr_pipe_7.pipe_fluid_array.link_to_front(
                    &mut ihx_sthe_6.shell_side_fluid_array, 
                    ihx_advection_heat_transfer_interaction)
                    .unwrap();

                ihx_sthe_6.shell_side_fluid_array.link_to_front(
                    &mut fhr_pipe_5.pipe_fluid_array,
                    ihx_advection_heat_transfer_interaction)
                    .unwrap();

                fhr_pipe_5.pipe_fluid_array.link_to_front(
                    &mut fhr_pipe_4.pipe_fluid_array,
                    ihx_advection_heat_transfer_interaction)
                    .unwrap();

                fhr_pipe_4.pipe_fluid_array.link_to_front(
                    top_mixing_node_pri_loop, 
                    ihx_advection_heat_transfer_interaction)
                    .unwrap();
                }

            // intermediate loop ihx branch 
            {

                bottom_mixing_node_intrmd_loop.link_to_front(
                    &mut fhr_pipe_17.pipe_fluid_array, 
                    intrmd_loop_ihx_br_heat_transfer_interaction)
                    .unwrap();

                ihx_sthe_6.tube_side_fluid_array_for_single_tube.link_to_back(
                    &mut fhr_pipe_17.pipe_fluid_array, 
                    intrmd_loop_ihx_br_heat_transfer_interaction)
                    .unwrap();

                ihx_sthe_6.tube_side_fluid_array_for_single_tube.link_to_front(
                    &mut fhr_pipe_12.pipe_fluid_array, 
                    intrmd_loop_ihx_br_heat_transfer_interaction)
                    .unwrap();

                fhr_pipe_12.pipe_fluid_array.link_to_front(
                    top_mixing_node_intrmd_loop, 
                    intrmd_loop_ihx_br_heat_transfer_interaction)
                    .unwrap();

                }

            // intermediate loop steam generator branch
            {

                bottom_mixing_node_intrmd_loop.link_to_front(
                    &mut fhr_intrmd_loop_pump_16.pipe_fluid_array, 
                    intrmd_loop_steam_gen_br_heat_transfer_interaction)
                    .unwrap();

                fhr_intrmd_loop_pump_16.pipe_fluid_array.link_to_front(
                    &mut fhr_pipe_15.pipe_fluid_array, 
                    intrmd_loop_steam_gen_br_heat_transfer_interaction)
                    .unwrap();

                fhr_pipe_15.pipe_fluid_array.link_to_front(
                    &mut fhr_steam_generator_shell_side_14.pipe_fluid_array, 
                    intrmd_loop_steam_gen_br_heat_transfer_interaction)
                    .unwrap();

                fhr_steam_generator_shell_side_14.pipe_fluid_array.link_to_front(
                    &mut fhr_pipe_13.pipe_fluid_array, 
                    intrmd_loop_steam_gen_br_heat_transfer_interaction)
                    .unwrap();

                fhr_pipe_13.pipe_fluid_array.link_to_front(
                    top_mixing_node_intrmd_loop, 
                    intrmd_loop_steam_gen_br_heat_transfer_interaction)
                    .unwrap();
                }
            let heat_added_to_steam_generator: Energy;
            let steam_generator_duty: SteamGeneratorDuty;
            {

                // ── Steam-generator duty ────────────────────────────────
                // Heat leaves the HITEC shell and enters the water/steam
                // tube. Before 2026-08-12 this was the bare conductance
                // product `Q = UA*(T_shell_bulk - T_tube_out)` with no upper
                // bound, so a large enough slider `UA` transferred more heat
                // than the pinch permits -- a second-law violation that
                // surfaced as a tube-outlet temperature above the shell
                // temperature. It is now clamped to the thermodynamic
                // maximum by `pinch_limited_steam_generator_duty`; see that
                // module for the derivation and the V&V sweep.
                //
                // The *same* clamped number is applied to both sides
                // (`-Q` to the shell, `+Q` to the tube), which also closes an
                // energy-conservation hole: the old code pushed a negative
                // duty into the shell while the tube side clamped it at zero,
                // creating energy out of nothing in exactly the state a cross
                // produces.

                let number_of_temperature_nodes_for_sg = 2;
                let mut q_frac_arr: Array1<f64> = Array::default(number_of_temperature_nodes_for_sg);
                // we want the middle node to contain all the power
                q_frac_arr[0] = 0.5;
                q_frac_arr[1] = 0.5;
                let mut sg_fluid_array_clone: FluidArray =
                    fhr_steam_generator_shell_side_14
                    .pipe_fluid_array
                    .clone()
                    .try_into()
                    .unwrap();

                let shell_mean_temperature = sg_fluid_array_clone
                    .try_get_bulk_temperature()
                    .unwrap();

                // `T_hot_in` for the pinch: the hottest shell node, which is
                // the salt inlet end of the counter-flow exchanger.
                let shell_inlet_temperature: ThermodynamicTemperature =
                    sg_fluid_array_clone
                    .get_temperature_vector()
                    .unwrap()
                    .into_iter()
                    .fold(shell_mean_temperature, |hottest, node_temperature| {
                        if node_temperature.get::<degree_celsius>()
                            > hottest.get::<degree_celsius>() {
                                node_temperature
                            } else {
                                hottest
                            }
                    });

                // HITEC `cp` at the shell bulk temperature; the shell-side
                // fluid is declared as `LiquidMaterial::HITEC` in
                // `new_fhr_intermediate_loop_steam_generator_shell_side_14`.
                let salt_specific_heat_capacity: SpecificHeatCapacity =
                    LiquidMaterial::HITEC
                    .try_get_cp(shell_mean_temperature)
                    .unwrap();

                steam_generator_duty = pinch_limited_steam_generator_duty(
                    steam_generator_overall_ua,
                    shell_mean_temperature,
                    shell_inlet_temperature,
                    intrmd_loop_steam_gen_br_flow,
                    salt_specific_heat_capacity,
                    feedwater_inlet,
                    feedwater_mass_flowrate,
                    steam_generator_tube_side_temperature,
                );

                // heat leaves the shell at exactly the rate it enters the tube
                let steam_gen_heat_change: Power = -steam_generator_duty.duty;
                heat_added_to_steam_generator =
                    steam_gen_heat_change * timestep;

                sg_fluid_array_clone
                    .lateral_link_new_power_vector(
                        steam_gen_heat_change,
                        q_frac_arr)
                    .unwrap();

                fhr_steam_generator_shell_side_14.pipe_fluid_array
                    = sg_fluid_array_clone.into();
            }
            // now for the reactor branch, we must have some kind of 
            // power input here 
            {

                // i'll use the lateral link new power vector code 
                //
                // this sets the reactor power in the middle part of the 
                // pipe
                let number_of_temperature_nodes_for_reactor = 5;
                let mut q_frac_arr: Array1<f64> = Array::default(number_of_temperature_nodes_for_reactor);
                // we want the middle node to contain all the power
                q_frac_arr[0] = 0.0;
                q_frac_arr[1] = 0.0;
                q_frac_arr[2] = 1.0;
                q_frac_arr[3] = 0.0;
                q_frac_arr[4] = 0.0;

                // now i need to get the fluid array out first 

                let mut reactor_fluid_array_clone: FluidArray = 
                    reactor_pipe_1
                    .pipe_fluid_array
                    .clone()
                    .try_into()
                    .unwrap();

                reactor_fluid_array_clone
                    .lateral_link_new_power_vector(
                        reactor_power, 
                        q_frac_arr)
                    .unwrap();

                reactor_pipe_1.pipe_fluid_array = 
                    reactor_fluid_array_clone.into();

                // now, add the connections

                reactor_pipe_1.pipe_fluid_array.link_to_front(
                    top_mixing_node_pri_loop, 
                    reactor_branch_advection_heat_transfer_interaction)
                    .unwrap();
                reactor_pipe_1.pipe_fluid_array.link_to_back(
                    bottom_mixing_node_pri_loop, 
                    reactor_branch_advection_heat_transfer_interaction)
                    .unwrap();
                }

            // now we are ready to advance timesteps for all components 
            // and mixing nodes 

            let zero_power = Power::ZERO;
            // for pri loop 
            // I'm not going to add another round of power 
            // because I already added it to the top
            // so i'll just add zero power
            //
            // this is reactor and downcomer branches
            {
                reactor_pipe_1
                    .lateral_and_miscellaneous_connections_no_wall_correction(
                        reactor_branch_flow, 
                        zero_power)
                    .unwrap();

                downcomer_pipe_2
                    .lateral_and_miscellaneous_connections_no_wall_correction(
                        downcomer_branch_1_flow, 
                        zero_power)
                    .unwrap();

                downcomer_pipe_3
                    .lateral_and_miscellaneous_connections_no_wall_correction(
                        downcomer_branch_2_flow, 
                        zero_power)
                    .unwrap();
                }

            // this is the pri loop ihx branch
            // except for the ihx itself
            {

                fhr_pipe_11
                    .lateral_and_miscellaneous_connections_no_wall_correction(
                        pri_loop_intermediate_heat_exchanger_branch_flow, 
                        zero_power)
                    .unwrap();
                fhr_pipe_10
                    .lateral_and_miscellaneous_connections_no_wall_correction(
                        pri_loop_intermediate_heat_exchanger_branch_flow, 
                        zero_power)
                    .unwrap();
                fhr_pri_loop_pump_9
                    .lateral_and_miscellaneous_connections_no_wall_correction(
                        pri_loop_intermediate_heat_exchanger_branch_flow, 
                        zero_power)
                    .unwrap();
                fhr_pipe_8
                    .lateral_and_miscellaneous_connections_no_wall_correction(
                        pri_loop_intermediate_heat_exchanger_branch_flow, 
                        zero_power)
                    .unwrap();
                fhr_pipe_7
                    .lateral_and_miscellaneous_connections_no_wall_correction(
                        pri_loop_intermediate_heat_exchanger_branch_flow, 
                        zero_power)
                    .unwrap();
                fhr_pipe_5
                    .lateral_and_miscellaneous_connections_no_wall_correction(
                        pri_loop_intermediate_heat_exchanger_branch_flow, 
                        zero_power)
                    .unwrap();
                fhr_pipe_4
                    .lateral_and_miscellaneous_connections_no_wall_correction(
                        pri_loop_intermediate_heat_exchanger_branch_flow, 
                        zero_power)
                    .unwrap();
                }

            // ihx 
            {

                let prandtl_wall_correction_setting = true; 
                let tube_side_total_mass_flowrate = intrmd_loop_ihx_br_flow;
                let shell_side_total_mass_flowrate = pri_loop_intermediate_heat_exchanger_branch_flow;

                ihx_sthe_6.lateral_and_miscellaneous_connections(
                    prandtl_wall_correction_setting, 
                    tube_side_total_mass_flowrate, 
                    shell_side_total_mass_flowrate).unwrap();

            }
            // hitec intrmd loop 
            //
            // except for ihx itself
            {
                // ihx branch
                fhr_pipe_17
                    .lateral_and_miscellaneous_connections_no_wall_correction(
                        intrmd_loop_ihx_br_flow, 
                        zero_power)
                    .unwrap();
                fhr_pipe_12
                    .lateral_and_miscellaneous_connections_no_wall_correction(
                        intrmd_loop_ihx_br_flow, 
                        zero_power)
                    .unwrap();

                // steam gen branch
                fhr_intrmd_loop_pump_16
                    .lateral_and_miscellaneous_connections_no_wall_correction(
                        intrmd_loop_steam_gen_br_flow, 
                        zero_power)
                    .unwrap();
                fhr_pipe_15
                    .lateral_and_miscellaneous_connections_no_wall_correction(
                        intrmd_loop_steam_gen_br_flow, 
                        zero_power)
                    .unwrap();
                fhr_steam_generator_shell_side_14
                    .lateral_and_miscellaneous_connections_no_wall_correction(
                        intrmd_loop_steam_gen_br_flow, 
                        zero_power)
                    .unwrap();
                fhr_pipe_13
                    .lateral_and_miscellaneous_connections_no_wall_correction(
                        intrmd_loop_steam_gen_br_flow, 
                        zero_power)
                    .unwrap();
                }

            // timestep advance for all heat transfer entities
            {
                // pri loop (with ihx)
                reactor_pipe_1
                    .advance_timestep(timestep)
                    .unwrap();
                downcomer_pipe_2
                    .advance_timestep(timestep)
                    .unwrap();
                downcomer_pipe_3
                    .advance_timestep(timestep)
                    .unwrap();


                fhr_pipe_4
                    .advance_timestep(timestep)
                    .unwrap();
                fhr_pipe_5
                    .advance_timestep(timestep)
                    .unwrap();
                fhr_pipe_7
                    .advance_timestep(timestep)
                    .unwrap();
                fhr_pipe_8
                    .advance_timestep(timestep)
                    .unwrap();
                fhr_pri_loop_pump_9
                    .advance_timestep(timestep)
                    .unwrap();
                fhr_pipe_10
                    .advance_timestep(timestep)
                    .unwrap();
                fhr_pipe_11
                    .advance_timestep(timestep)
                    .unwrap();

                // intermediate branch (less ihx)
                fhr_pipe_12
                    .advance_timestep(timestep)
                    .unwrap();
                fhr_pipe_17
                    .advance_timestep(timestep)
                    .unwrap();
                fhr_pipe_13
                    .advance_timestep(timestep)
                    .unwrap();
                fhr_steam_generator_shell_side_14
                    .advance_timestep(timestep)
                    .unwrap();
                fhr_pipe_15
                    .advance_timestep(timestep)
                    .unwrap();
                fhr_intrmd_loop_pump_16
                    .advance_timestep(timestep)
                    .unwrap();

                // all mixing nodes
                top_mixing_node_pri_loop
                    .advance_timestep_mut_self(timestep)
                    .unwrap();
                bottom_mixing_node_pri_loop
                    .advance_timestep_mut_self(timestep)
                    .unwrap();
                top_mixing_node_intrmd_loop
                    .advance_timestep_mut_self(timestep)
                    .unwrap();
                bottom_mixing_node_intrmd_loop
                    .advance_timestep_mut_self(timestep)
                    .unwrap();

                ihx_sthe_6
                    .advance_timestep(timestep)
                    .unwrap();
                }

            // now I want reactor temperature profile 
            let reactor_temp_profile: Vec<ThermodynamicTemperature> = 
                reactor_pipe_1
                .pipe_fluid_array_temperature()
                .unwrap();
            let reactor_temp_profile_degc: Vec<f64> = 
                reactor_temp_profile
                .into_iter()
                .map(|temperature|{
                    (temperature.get::<degree_celsius>()*100.0).round()/100.0
                })
            .collect();

            // sthe temperature profile
            let ihx_shell_side_temp_profile: Vec<ThermodynamicTemperature> = 
                ihx_sthe_6 
                .shell_side_fluid_array_temperature()
                .unwrap();

            let ihx_shell_side_temp_profile_degc: Vec<f64> = 
                ihx_shell_side_temp_profile
                .into_iter()
                .map(|temperature|{
                    (temperature.get::<degree_celsius>()*100.0).round()/100.0
                })
            .collect();

            let ihx_tube_side_temp_profile: Vec<ThermodynamicTemperature> = 
                ihx_sthe_6 
                .inner_tube_fluid_array_temperature()
                .unwrap();

            let ihx_tube_side_temp_profile_degc: Vec<f64> = 
                ihx_tube_side_temp_profile
                .into_iter()
                .map(|temperature|{
                    (temperature.get::<degree_celsius>()*100.0).round()/100.0
                })
            .collect();

            // steam generator tube side temp profile
            let sg_shell_side_temp_profile: Vec<ThermodynamicTemperature> = 
                fhr_steam_generator_shell_side_14 
                .pipe_fluid_array_temperature()
                .unwrap();

            let sg_shell_side_temp_profile_degc: Vec<f64> = 
                sg_shell_side_temp_profile
                .into_iter()
                .map(|temperature|{
                    (temperature.get::<degree_celsius>()*100.0).round()/100.0
                })
            .collect();

            // pipe 4, after reactor outlet 
            let pipe_4_temp_profile: Vec<ThermodynamicTemperature> = 
                fhr_pipe_4 
                .pipe_fluid_array_temperature()
                .unwrap();

            let pipe_4_temp_profile_degc: Vec<f64> = 
                pipe_4_temp_profile
                .into_iter()
                .map(|temperature|{
                    (temperature.get::<degree_celsius>()*100.0).round()/100.0
                })
            .collect();
            // pipe 5, just before STHE
            let pipe_5_temp_profile: Vec<ThermodynamicTemperature> = 
                fhr_pipe_5 
                .pipe_fluid_array_temperature()
                .unwrap();

            let pipe_5_temp_profile_degc: Vec<f64> = 
                pipe_5_temp_profile
                .into_iter()
                .map(|temperature|{
                    (temperature.get::<degree_celsius>()*100.0).round()/100.0
                })
            .collect();
            // pipe 7, just after STHE
            let pipe_7_temp_profile: Vec<ThermodynamicTemperature> = 
                fhr_pipe_7 
                .pipe_fluid_array_temperature()
                .unwrap();

            let pipe_7_temp_profile_degc: Vec<f64> = 
                pipe_7_temp_profile
                .into_iter()
                .map(|temperature|{
                    (temperature.get::<degree_celsius>()*100.0).round()/100.0
                })
            .collect();
            // pipe 8, just before pump
            let pipe_8_temp_profile: Vec<ThermodynamicTemperature> = 
                fhr_pipe_8 
                .pipe_fluid_array_temperature()
                .unwrap();

            let pipe_8_temp_profile_degc: Vec<f64> = 
                pipe_8_temp_profile
                .into_iter()
                .map(|temperature|{
                    (temperature.get::<degree_celsius>()*100.0).round()/100.0
                })
            .collect();

            // pipe 10, just after pump
            let pipe_10_temp_profile: Vec<ThermodynamicTemperature> = 
                fhr_pipe_10 
                .pipe_fluid_array_temperature()
                .unwrap();

            let pipe_10_temp_profile_degc: Vec<f64> = 
                pipe_10_temp_profile
                .into_iter()
                .map(|temperature|{
                    (temperature.get::<degree_celsius>()*100.0).round()/100.0
                })
            .collect();

            // pipe 11, just before reactor inlet
            let pipe_11_temp_profile: Vec<ThermodynamicTemperature> = 
                fhr_pipe_11 
                .pipe_fluid_array_temperature()
                .unwrap();

            let pipe_11_temp_profile_degc: Vec<f64> = 
                pipe_11_temp_profile
                .into_iter()
                .map(|temperature|{
                    (temperature.get::<degree_celsius>()*100.0).round()/100.0
                })
            .collect();


            // pipe 12, just before STHE tube side
            let pipe_12_temp_profile: Vec<ThermodynamicTemperature> = 
                fhr_pipe_12 
                .pipe_fluid_array_temperature()
                .unwrap();

            let pipe_12_temp_profile_degc: Vec<f64> = 
                pipe_12_temp_profile
                .into_iter()
                .map(|temperature|{
                    (temperature.get::<degree_celsius>()*100.0).round()/100.0
                })
            .collect();

            // pipe 13, just before steam generator shell side
            let pipe_13_temp_profile: Vec<ThermodynamicTemperature> = 
                fhr_pipe_13 
                .pipe_fluid_array_temperature()
                .unwrap();

            let pipe_13_temp_profile_degc: Vec<f64> = 
                pipe_13_temp_profile
                .into_iter()
                .map(|temperature|{
                    (temperature.get::<degree_celsius>()*100.0).round()/100.0
                })
            .collect();

            // pipe 15, just after steam generator shell side
            let pipe_15_temp_profile: Vec<ThermodynamicTemperature> = 
                fhr_pipe_15 
                .pipe_fluid_array_temperature()
                .unwrap();

            let pipe_15_temp_profile_degc: Vec<f64> = 
                pipe_15_temp_profile
                .into_iter()
                .map(|temperature|{
                    (temperature.get::<degree_celsius>()*100.0).round()/100.0
                })
            .collect();


            // pipe 17, just after steam generator shell side
            let pipe_17_temp_profile: Vec<ThermodynamicTemperature> = 
                fhr_pipe_17 
                .pipe_fluid_array_temperature()
                .unwrap();

            let pipe_17_temp_profile_degc: Vec<f64> = 
                pipe_17_temp_profile
                .into_iter()
                .map(|temperature|{
                    (temperature.get::<degree_celsius>()*100.0).round()/100.0
                })
            .collect();

            // pri pump
            let pump_9_temp_profile: Vec<ThermodynamicTemperature> = 
                fhr_pri_loop_pump_9 
                .pipe_fluid_array_temperature()
                .unwrap();

            let pump_9_temp_profile_degc: Vec<f64> = 
                pump_9_temp_profile
                .into_iter()
                .map(|temperature|{
                    (temperature.get::<degree_celsius>()*100.0).round()/100.0
                })
            .collect();

            // intrmd pump
            let pump_16_temp_profile: Vec<ThermodynamicTemperature> = 
                fhr_intrmd_loop_pump_16 
                .pipe_fluid_array_temperature()
                .unwrap();

            let pump_16_temp_profile_degc: Vec<f64> = 
                pump_16_temp_profile
                .into_iter()
                .map(|temperature|{
                    (temperature.get::<degree_celsius>()*100.0).round()/100.0
                })
            .collect();

            // downcomer_temp profile
            let downcomer_2_temp_profile: Vec<ThermodynamicTemperature> = 
                downcomer_pipe_2 
                .pipe_fluid_array_temperature()
                .unwrap();

            let downcomer_2_temp_profile_degc: Vec<f64> = 
                downcomer_2_temp_profile
                .into_iter()
                .map(|temperature|{
                    (temperature.get::<degree_celsius>()*100.0).round()/100.0
                })
            .collect();

            let downcomer_3_temp_profile: Vec<ThermodynamicTemperature> = 
                downcomer_pipe_3 
                .pipe_fluid_array_temperature()
                .unwrap();

            let downcomer_3_temp_profile_degc: Vec<f64> = 
                downcomer_3_temp_profile
                .into_iter()
                .map(|temperature|{
                    (temperature.get::<degree_celsius>()*100.0).round()/100.0
                })
            .collect();


            let fhr_state = FHRThermalHydraulicsState {
                reactor_branch_flow,
                downcomer_branch_1_flow,
                downcomer_branch_2_flow,
                intermediate_heat_exchanger_branch_flow: pri_loop_intermediate_heat_exchanger_branch_flow,
                intrmd_loop_ihx_br_flow,
                intrmd_loop_steam_gen_br_flow,
                simulation_time,
                reactor_temp_profile_degc,
                ihx_shell_side_temp_profile_degc,
                ihx_tube_side_temp_profile_degc,
                sg_shell_side_temp_profile_degc,
                pipe_4_temp_profile_degc,
                pipe_5_temp_profile_degc,
                pipe_7_temp_profile_degc,
                pipe_8_temp_profile_degc,
                pump_9_temp_profile_degc,
                pipe_10_temp_profile_degc,
                pipe_11_temp_profile_degc,
                pipe_12_temp_profile_degc,
                pipe_13_temp_profile_degc,
                pipe_15_temp_profile_degc,
                pump_16_temp_profile_degc,
                pipe_17_temp_profile_degc,
                downcomer_2_temp_profile_degc,
                downcomer_3_temp_profile_degc,
                heat_added_to_steam_generator_shell_side: heat_added_to_steam_generator,
                steam_generator_effectiveness: steam_generator_duty.effectiveness,
                steam_generator_maximum_duty: steam_generator_duty.thermodynamic_maximum,
                steam_generator_duty_limit: steam_generator_duty.limited_by,
            };

            // if one wants to monitor flow through the loop
            let debugging = false;
            if debugging {
                dbg!(&fhr_state);
            }
            return fhr_state;
        }



    /// Drives the FHR primary, intermediate and secondary loops forever, one
    /// 0.1 s thermal-hydraulics timestep at a time, publishing each step's
    /// state into the shared [`FHRState`] the GUI reads.
    ///
    /// `salt_freeze_monitor` is the graceful-stop handle: at the top of every
    /// iteration this loop checks the *previous* step's temperature profiles
    /// for a salt node that has gone below its melting point, and if it finds
    /// one it records the freeze and parks **before** entering a step whose
    /// range-checked property lookups would panic the thread. It stays parked,
    /// holding the plant exactly as it froze, until the operator presses the
    /// melt button in the modal, at which point the frozen loop's components
    /// are rebuilt at
    /// [`MELT_RESTORE_TEMPERATURE_DEGC`](salt_freeze_guard::MELT_RESTORE_TEMPERATURE_DEGC)
    /// and stepping resumes. See [`salt_freeze_guard`] for why the melt is a
    /// deliberate, labelled teaching shortcut rather than physics.
    pub fn calculate_thermal_hydraulics_loop(
        fhr_state: Arc<Mutex<FHRState>>,
        salt_freeze_monitor: SaltFreezeMonitor){

        let thermal_hydraulics_timestep = Time::new::<second>(0.1);

        let fhr_state_clone = fhr_state.clone();
        // now, time controls
        let loop_time = SystemTime::now();
        let mut current_simulation_time = Time::ZERO;

        // create components first
        let initial_temperature = ThermodynamicTemperature::new::<degree_celsius>(
            fhr_state_clone.lock().unwrap().core_outlet_temp_degc
        );
        let mut reactor_pipe_1 = new_reactor_vessel_pipe_1(initial_temperature);
        let mut downcomer_pipe_2 = new_downcomer_pipe_2(initial_temperature);
        let mut downcomer_pipe_3 = new_downcomer_pipe_3(initial_temperature);

        // pri loop branch (positive is in this order of flow)
        let mut fhr_pipe_11 = new_fhr_pipe_11(initial_temperature);
        let mut fhr_pipe_10 = new_fhr_pipe_10(initial_temperature);
        let mut fhr_pri_loop_pump_9 = new_fhr_pri_loop_pump_9(initial_temperature);
        let mut fhr_pipe_8 = new_fhr_pipe_8(initial_temperature);
        let mut fhr_pipe_7 = new_fhr_pipe_7(initial_temperature);
        // note that for HITEC, the temperature range is from
        // 440-800K
        // this is 167-527C
        // so intial temperature of 500C is ok
        let mut ihx_sthe_6 = new_ihx_sthe_6_version_1(initial_temperature);
        let mut fhr_pipe_5 = new_fhr_pipe_5(initial_temperature);
        let mut fhr_pipe_4 = new_fhr_pipe_4_ver_2(initial_temperature);


        let initial_temperature_intrmd_loop =
            initial_temperature;
        // intermediate loop ihx side
        // (excluding sthe)
        let mut fhr_pipe_17 = new_fhr_pipe_17(initial_temperature_intrmd_loop);
        let mut fhr_pipe_12 = new_fhr_pipe_12(initial_temperature_intrmd_loop);

        // intermediate loop steam generator side
        let mut fhr_intrmd_loop_pump_16 = new_fhr_intermediate_loop_pump_16(
            initial_temperature_intrmd_loop);
        let mut fhr_pipe_15 = new_fhr_pipe_15(initial_temperature_intrmd_loop);
        let mut fhr_steam_generator_shell_side_14
            = new_fhr_intermediate_loop_steam_generator_shell_side_14(
                initial_temperature_intrmd_loop);
        let mut fhr_pipe_13 = new_fhr_pipe_13(initial_temperature_intrmd_loop);


        // probably want to use fhr state
        let mut pri_loop_pump_pressure = Pressure::new::<kilopascal>(-10.0);
        let mut intrmd_loop_pump_pressure = Pressure::new::<kilopascal>(-10.0);

        // mixing nodes for pri loop 
        let mut bottom_mixing_node_pri_loop = 
            gfhr_bottom_mixing_node_pri_loop(initial_temperature);
        let mut top_mixing_node_pri_loop = 
            gfhr_top_mixing_node_pri_loop(initial_temperature);
        // mixing nodes for intermediate loop 
        let mut bottom_mixing_node_intrmd_loop = 
            gfhr_bottom_mixing_node_intrmd_loop(initial_temperature_intrmd_loop);
        let mut top_mixing_node_intrmd_loop = 
            gfhr_top_mixing_node_intrmd_loop(initial_temperature_intrmd_loop);

        // create initial mass flowrates 

        // start with some initial flow rates
        let (reactor_branch_flow, downcomer_branch_1_flow, 
            downcomer_branch_2_flow, intermediate_heat_exchanger_branch_flow,
            intrmd_loop_ihx_br_flow,
            intrmd_loop_steam_gen_br_flow)
            = four_branch_pri_and_intermediate_loop_fluid_mechanics_only(
                pri_loop_pump_pressure, 
                intrmd_loop_pump_pressure, 
                &reactor_pipe_1, 
                &downcomer_pipe_2, 
                &downcomer_pipe_3, 
                &fhr_pipe_11, 
                &fhr_pipe_10, 
                &fhr_pri_loop_pump_9, 
                &fhr_pipe_8, 
                &fhr_pipe_7, 
                &ihx_sthe_6, 
                &fhr_pipe_5, 
                &fhr_pipe_4, 
                &fhr_pipe_17, 
                &fhr_pipe_12, 
                &fhr_intrmd_loop_pump_16, 
                &fhr_pipe_15, 
                &fhr_steam_generator_shell_side_14, 
                &fhr_pipe_13,
            );


        let mut current_fhr_thermal_hydraulics_state = FHRThermalHydraulicsState {
            downcomer_branch_1_flow,
            downcomer_branch_2_flow,
            intermediate_heat_exchanger_branch_flow,
            intrmd_loop_ihx_br_flow,
            intrmd_loop_steam_gen_br_flow,
            reactor_branch_flow,
            simulation_time: current_simulation_time,
            reactor_temp_profile_degc: vec![],
            ihx_shell_side_temp_profile_degc: vec![],
            ihx_tube_side_temp_profile_degc: vec![],
            sg_shell_side_temp_profile_degc: vec![],
            pipe_4_temp_profile_degc: vec![],
            pipe_5_temp_profile_degc: vec![],
            pipe_7_temp_profile_degc: vec![],
            pipe_8_temp_profile_degc: vec![],
            pump_9_temp_profile_degc: vec![],
            pipe_10_temp_profile_degc: vec![],
            pipe_11_temp_profile_degc: vec![],
            pipe_12_temp_profile_degc: vec![],
            pipe_13_temp_profile_degc: vec![],
            pipe_15_temp_profile_degc: vec![],
            pump_16_temp_profile_degc: vec![],
            pipe_17_temp_profile_degc: vec![],
            downcomer_2_temp_profile_degc: vec![],
            downcomer_3_temp_profile_degc: vec![],
            heat_added_to_steam_generator_shell_side: Energy::ZERO,
            steam_generator_effectiveness: 0.0,
            steam_generator_maximum_duty: Power::ZERO,
            steam_generator_duty_limit: SteamGeneratorDutyLimit::NoDrivingTemperatureDifference,
        };

        let mut current_fhr_steam_gen_state: SecondaryLoopState
        = SecondaryLoopState {
            steam_gen_tube_outlet_temperature: ThermodynamicTemperature::new::<degree_celsius>(35.0),
            turbine_power: Power::ZERO,
            condenser_duty: Power::ZERO,
            steam_quality_after_condenser: 0.0,
            steam_quality_after_pump: 0.0,
            steam_quality_after_steam_generator_tube_side: 1.0,
            steam_quality_after_turbine: 0.2,
            sat_temperature_in_sg_tube_degc: 120.0,
            steam_turbine: ThreePhaseElectricGeneratorTurbine::new_250_megawatt_generator(),
        };

        // Persistent spatially-resolved steam-generator tube
        // (TampinesSteamArray). Created once; driven and advanced a bounded
        // amount each TH step by secondary_loop_single_timestep, so it relaxes
        // toward the current boundary conditions over real time.
        let mut steam_generator_tube =
            crate::app::thermal_hydraulics_backend::secondary_loop::build_steam_generator_tube();

        // calculation loop (indefinite)
        //
        // to be done once every timestep
        loop {

            // ── salt-freeze guard ───────────────────────────────────────
            // Checked FIRST, on the temperature profiles the *previous* step
            // produced, so that a loop which has gone below its salt's
            // melting point never enters the step whose range-checked
            // property lookups (`get_flibe_density`, `get_hitec_viscosity`,
            // ...) would return out-of-range and be unwrapped into a panic.
            // Detecting afterwards, or catching the unwind, would leave the
            // plant half-advanced and its stored energy indeterminate --
            // stopping cleanly at a known state is the whole point.
            if let Some(freeze_event) =
                salt_freeze_guard::detect_salt_freeze(&current_fhr_thermal_hydraulics_state) {
                    salt_freeze_monitor.record(freeze_event);
                }

            // `event()` is `None` unless a freeze is currently being
            // reported, so this both tests "are we paused?" and gives us the
            // loop to restore. It must be read *before* `take_melt_request`,
            // which clears the event as it consumes the request.
            if let Some(active_freeze) = salt_freeze_monitor.event() {
                if salt_freeze_monitor.take_melt_request() {
                    // The operator asked for a melt. Rebuild every component
                    // of the frozen loop at the simulator's own cold-start
                    // temperature. This is NOT a thaw model -- see
                    // `salt_freeze_guard`'s module docs; the modal says so to
                    // the user in as many words.
                    let melt_temperature =
                        ThermodynamicTemperature::new::<degree_celsius>(
                            salt_freeze_guard::MELT_RESTORE_TEMPERATURE_DEGC
                        );
                    let frozen_loop = active_freeze.frozen_loop;

                    match frozen_loop {
                        salt_freeze_guard::FrozenLoop::PrimaryFlibe => {
                            reactor_pipe_1 = new_reactor_vessel_pipe_1(melt_temperature);
                            downcomer_pipe_2 = new_downcomer_pipe_2(melt_temperature);
                            downcomer_pipe_3 = new_downcomer_pipe_3(melt_temperature);
                            fhr_pipe_11 = new_fhr_pipe_11(melt_temperature);
                            fhr_pipe_10 = new_fhr_pipe_10(melt_temperature);
                            fhr_pri_loop_pump_9 = new_fhr_pri_loop_pump_9(melt_temperature);
                            fhr_pipe_8 = new_fhr_pipe_8(melt_temperature);
                            fhr_pipe_7 = new_fhr_pipe_7(melt_temperature);
                            fhr_pipe_5 = new_fhr_pipe_5(melt_temperature);
                            fhr_pipe_4 = new_fhr_pipe_4_ver_2(melt_temperature);
                            bottom_mixing_node_pri_loop =
                                gfhr_bottom_mixing_node_pri_loop(melt_temperature);
                            top_mixing_node_pri_loop =
                                gfhr_top_mixing_node_pri_loop(melt_temperature);
                            // The IHX is a single object holding the FLiBe
                            // shell and the HITEC tube, so melting the
                            // primary loop necessarily resets both of its
                            // sides. `reset_profiles_after_melt` reports the
                            // same coupling.
                            ihx_sthe_6 = new_ihx_sthe_6_version_1(melt_temperature);
                        },
                        salt_freeze_guard::FrozenLoop::IntermediateHitec => {
                            fhr_pipe_17 = new_fhr_pipe_17(melt_temperature);
                            fhr_pipe_12 = new_fhr_pipe_12(melt_temperature);
                            fhr_intrmd_loop_pump_16 =
                                new_fhr_intermediate_loop_pump_16(melt_temperature);
                            fhr_pipe_15 = new_fhr_pipe_15(melt_temperature);
                            fhr_steam_generator_shell_side_14 =
                                new_fhr_intermediate_loop_steam_generator_shell_side_14(
                                    melt_temperature);
                            fhr_pipe_13 = new_fhr_pipe_13(melt_temperature);
                            bottom_mixing_node_intrmd_loop =
                                gfhr_bottom_mixing_node_intrmd_loop(melt_temperature);
                            top_mixing_node_intrmd_loop =
                                gfhr_top_mixing_node_intrmd_loop(melt_temperature);
                            ihx_sthe_6 = new_ihx_sthe_6_version_1(melt_temperature);
                        },
                    }

                    // Refresh the stale frozen profiles so the guard does not
                    // immediately re-trip on numbers the melt has just made
                    // untrue. The next real step overwrites them anyway.
                    salt_freeze_guard::reset_profiles_after_melt(
                        &mut current_fhr_thermal_hydraulics_state,
                        frozen_loop,
                    );
                } else {
                    // Parked. Hold the plant exactly as it froze; do not
                    // advance time, do not touch a salt property.
                    thread::sleep(Duration::from_millis(
                        salt_freeze_guard::FREEZE_PAUSE_POLL_INTERVAL_MS
                    ));
                    continue;
                }
            }

            // so now, let's do the necessary things
            // first, timestep and loop time 
            //
            // second, read and update the local_ciet_state

            let loop_time_start = loop_time.elapsed().unwrap();

            //
            let accumulated_energy_from_prke = 
                Energy::new::<kilojoule>(
                    fhr_state_clone.lock().unwrap().prke_loop_accumulated_heat_removal_kilojoules
                );

            // once energy is "taken out" from the store, 
            // it is meant to be zero
            fhr_state_clone.lock().unwrap().prke_loop_accumulated_heat_removal_kilojoules 
                = 0.0;
            let reactor_power = 
                accumulated_energy_from_prke/thermal_hydraulics_timestep;

            // then primary loop pressure 
            
            pri_loop_pump_pressure = 
                Pressure::new::<kilopascal>(
                    -fhr_state_clone.lock().unwrap().fhr_pri_loop_pump_pressure_kilopascals
                );
            intrmd_loop_pump_pressure = 
                Pressure::new::<kilopascal>(
                    -fhr_state_clone.lock().unwrap().fhr_intermediate_loop_pump_pressure_kilopascals
                );
            // steam generator settings 
            //

            // now for the overall heat transfer coeff, this 
            // is based on some degree of chf,
            // i based it on pool boiling, which may be the most conservative 
            // in predicting chf as there is no local flow.
            // first we get the steam generator shell side fluid temp 

            let num_of_nodes_in_sg_shell: f64 = 
                fhr_state_clone.lock().unwrap() 
                .sg_shell_14_temperature_vector_degc
                .len() as f64;
            let sg_shell_14_sum_temp_degc: f64 = 
                fhr_state_clone.lock().unwrap() 
                .sg_shell_14_temperature_vector_degc
                .iter().sum();

            let sg_shell_14_avg_temp: f64 = 
                sg_shell_14_sum_temp_degc/num_of_nodes_in_sg_shell;
            // now we need a degree of superheat

            let sat_temperature_in_sg_tube_degc: f64 = 
                current_fhr_steam_gen_state.sat_temperature_in_sg_tube_degc;

            let degree_of_superheat: TemperatureInterval = 
                TemperatureInterval::new::<uom::si::temperature_interval::kelvin>(
                    sat_temperature_in_sg_tube_degc - 
                    sg_shell_14_avg_temp
                );

            let mut critical_heat_flux_ua_modifier: f64 
                = pool_boiling_improvised_correlation_as_fraction_of_maximum(
                    degree_of_superheat);
            // this only works if degree of superheat is more than 1 K 

            if degree_of_superheat.get::<uom::si::temperature_interval::kelvin>() <= 1.0 {
                critical_heat_flux_ua_modifier = 0.025;
            };

            // now if quality is too low or high, just adjust to 
            // 0.025 also 
            //
            

            let sg_tube_steam_quality = current_fhr_steam_gen_state
                .steam_quality_after_steam_generator_tube_side;

            if sg_tube_steam_quality > 0.99 {
                critical_heat_flux_ua_modifier = 0.025;
            };

            if sg_tube_steam_quality < 0.01 {
                critical_heat_flux_ua_modifier = 0.025;
            };

            // probably want to have some way to program this overall 
            // ua so that simulation is more stable...
            //
            // either something to do with control volumes or something 
            // else. 
            // That we don't get heat added to steam to be too much.
            // Probably a multi-node heat exchanger where small amounts 
            // of heat are added to each node.
            //
            // so that we don't get any kind of numerical instability
            //
            // will prbably need to do a fresh derivation.
            //
            // Also need more calculations to determine a suitable 
            // surface temperature, rather than just the salt 
            // temperature itself, which is likely too hot.
            let chf_on = false; 
            let mut steam_generator_overall_ua: ThermalConductance 
                = ThermalConductance::new::<watt_per_kelvin>(
                    fhr_state_clone.lock().unwrap().user_specified_secondary_loop_ua_watt_per_kelvin
                    * critical_heat_flux_ua_modifier
                );
            if !chf_on {

                steam_generator_overall_ua
                    = ThermalConductance::new::<watt_per_kelvin>(
                        fhr_state_clone.lock().unwrap().user_specified_secondary_loop_ua_watt_per_kelvin
                    );

            }


            let steam_generator_tube_side_temperature =
                ThermodynamicTemperature::new::<degree_celsius>(
                    fhr_state_clone.lock().unwrap().steam_generator_tube_outlet_temperature_degc
                );

            // Secondary-side inlet conditions, read here so the
            // steam-generator duty inside the intermediate-loop step can be
            // clamped to `Q_max = C_min (T_salt_in - T_feed_in)`. These are
            // the *same* sliders the secondary loop reads below, and the
            // feedwater state comes from the one shared
            // `feedwater_inlet_state` so the cap and the steam cycle cannot
            // disagree about where the cold stream starts.
            let secondary_loop_mass_flowrate_for_pinch: MassRate =
                MassRate::new::<kilogram_per_second>(
                    fhr_state_clone.lock().unwrap()
                    .user_specified_secondary_loop_mass_flowrate_kg_per_s
                );
            let pump_outlet_pressure_for_pinch: Pressure =
                Pressure::new::<bar>(
                    fhr_state_clone.lock().unwrap()
                    .user_specified_secondary_loop_pump_outlet_pressure_bar
                );
            let feedwater_inlet: FeedwaterInletState =
                feedwater_inlet_state(pump_outlet_pressure_for_pinch);

            // now calculate the fhr primary and intermediate loops
            current_fhr_thermal_hydraulics_state = 
                Self::four_branch_pri_and_intermediate_loop_single_time_step(
                    pri_loop_pump_pressure, 
                    intrmd_loop_pump_pressure, 
                    reactor_power, 
                    thermal_hydraulics_timestep, current_simulation_time, 
                    &mut reactor_pipe_1, &mut downcomer_pipe_2, 
                    &mut downcomer_pipe_3, &mut bottom_mixing_node_pri_loop, 
                    &mut top_mixing_node_pri_loop, &mut fhr_pipe_11, 
                    &mut fhr_pipe_10, &mut fhr_pri_loop_pump_9, 
                    &mut fhr_pipe_8, &mut fhr_pipe_7, &mut ihx_sthe_6, 
                    &mut fhr_pipe_5, &mut fhr_pipe_4, &mut fhr_pipe_17, 
                    &mut fhr_pipe_12, &mut fhr_intrmd_loop_pump_16, 
                    &mut fhr_pipe_15, &mut fhr_steam_generator_shell_side_14, 
                    &mut fhr_pipe_13, &mut bottom_mixing_node_intrmd_loop, 
                    &mut top_mixing_node_intrmd_loop, 
                    steam_generator_tube_side_temperature,
                    steam_generator_overall_ua,
                    feedwater_inlet,
                    secondary_loop_mass_flowrate_for_pinch);

            let debug = false;
            if debug {
                dbg!(&current_fhr_thermal_hydraulics_state); 
            }


            // now calculate the secondary loop 
            let mut user_specified_secondary_loop_mass_flowrate = 
                MassRate::new::<kilogram_per_second>(
                    fhr_state_clone.lock().unwrap() 
                    .user_specified_secondary_loop_mass_flowrate_kg_per_s
                );
            let user_specified_pump_outlet_pressure = 
                Pressure::new::<bar>(
                    fhr_state_clone.lock().unwrap() 
                    .user_specified_secondary_loop_pump_outlet_pressure_bar
                );

            let turbine_omega: AngularVelocity 
                = AngularVelocity::new::<revolution_per_minute>(
                    fhr_state_clone.lock().unwrap().turbine_rpm
                );

            // note: estimate was AI generated,
            // need to check
            let load_resistance = ElectricalResistance::new::<ohm>(1.3);
            current_fhr_steam_gen_state =
                Self::secondary_loop_single_timestep(
                    &mut current_fhr_thermal_hydraulics_state,
                    thermal_hydraulics_timestep,
                    &mut user_specified_secondary_loop_mass_flowrate,
                    user_specified_pump_outlet_pressure,
                    current_simulation_time,
                    turbine_omega,
                    load_resistance,
                    &mut steam_generator_tube,
                );

            // now let's get the turbine current rpm 

            
            if debug {
                dbg!(&current_fhr_steam_gen_state);
            }


            current_simulation_time += thermal_hydraulics_timestep;

            let simulation_time_seconds = current_simulation_time.get::<second>();

            let elapsed_time_seconds = 
                (loop_time.elapsed().unwrap().as_secs_f64() * 100.0).round()/100.0;

            *&mut fhr_state_clone.lock().unwrap().thermal_hydraulics_simulation_time_seconds 
                = elapsed_time_seconds;

            let overall_simulation_in_realtime_or_faster: bool = 
                simulation_time_seconds > elapsed_time_seconds;

            // now update the ciet state 
            let loop_time_end = loop_time.elapsed().unwrap();
            let time_taken_for_calculation_loop_microseconds: f64 = 
                (loop_time_end - loop_time_start)
                .as_micros() as f64;

            *&mut fhr_state_clone.lock().unwrap().thermal_hydraulics_timestep_microseconds
                = thermal_hydraulics_timestep.get::<microsecond>().round();
            *&mut fhr_state_clone.lock().unwrap().thermal_hydraulics_calc_time_microseconds
                = time_taken_for_calculation_loop_microseconds;

            // update temperatures
            {
                let mut fhr_state_lock = fhr_state_clone.lock().unwrap();

                // the reactor branch itself has five elements
                // from 0,1,2,3,4 in order of going from bottom 
                // of the core to the top
                fhr_state_lock.core_inlet_temp_degc = 
                    current_fhr_thermal_hydraulics_state
                    .reactor_temp_profile_degc[0];
                fhr_state_lock.core_bottom_temp_degc = 
                    current_fhr_thermal_hydraulics_state
                    .reactor_temp_profile_degc[1];
                fhr_state_lock.pebble_bed_coolant_temp_degc = 
                    current_fhr_thermal_hydraulics_state
                    .reactor_temp_profile_degc[2];
                fhr_state_lock.core_top_temp_degc = 
                    current_fhr_thermal_hydraulics_state
                    .reactor_temp_profile_degc[3];
                fhr_state_lock.core_outlet_temp_degc = 
                    current_fhr_thermal_hydraulics_state
                    .reactor_temp_profile_degc[4];

                // the downcomers 1 and 2 also have branches
                // with 5 nodes each 
                // however, the fhr itself only has three distinct regions 
                // for display
                // for now I will just use node 0, 2 and 4
                //

                fhr_state_lock.left_downcomer_lower_temp_degc = 
                    current_fhr_thermal_hydraulics_state 
                    .downcomer_2_temp_profile_degc[0];
                fhr_state_lock.left_downcomer_mid_temp_degc = 
                    current_fhr_thermal_hydraulics_state 
                    .downcomer_2_temp_profile_degc[2];
                fhr_state_lock.left_downcomer_upper_temp_degc = 
                    current_fhr_thermal_hydraulics_state 
                    .downcomer_2_temp_profile_degc[4];

                fhr_state_lock.right_downcomer_lower_temp_degc = 
                    current_fhr_thermal_hydraulics_state 
                    .downcomer_3_temp_profile_degc[0];
                fhr_state_lock.right_downcomer_mid_temp_degc = 
                    current_fhr_thermal_hydraulics_state 
                    .downcomer_3_temp_profile_degc[2];
                fhr_state_lock.right_downcomer_upper_temp_degc = 
                    current_fhr_thermal_hydraulics_state 
                    .downcomer_3_temp_profile_degc[4];

                // flowrate diagnostics 
                fhr_state_lock.intermediate_loop_clockwise_flow_kg_per_s = 
                    (current_fhr_thermal_hydraulics_state 
                    .intrmd_loop_ihx_br_flow 
                    .get::<kilogram_per_second>()*1000.0)/1000.0;

                fhr_state_lock.reactor_branch_flowrate_kg_per_s = 
                    (current_fhr_thermal_hydraulics_state 
                    .reactor_branch_flow 
                    .get::<kilogram_per_second>()*1000.0)/1000.0;
                fhr_state_lock.downcomer1_branch_flowrate_kg_per_s = 
                    (current_fhr_thermal_hydraulics_state 
                    .downcomer_branch_1_flow 
                    .get::<kilogram_per_second>()*1000.0)/1000.0;

                fhr_state_lock.downcomer2_branch_flowrate_kg_per_s = 
                    (current_fhr_thermal_hydraulics_state 
                    .downcomer_branch_2_flow
                    .get::<kilogram_per_second>()*1000.0)/1000.0;
                fhr_state_lock.ihx_branch_flowrate_kg_per_s = 
                    (current_fhr_thermal_hydraulics_state 
                    .intermediate_heat_exchanger_branch_flow
                    .get::<kilogram_per_second>()*1000.0)/1000.0;

                // pri loop state 
                fhr_state_lock.pipe_4_temperature_vector_degc = 
                    current_fhr_thermal_hydraulics_state
                    .pipe_4_temp_profile_degc.clone();
                fhr_state_lock.pipe_5_temperature_vector_degc = 
                    current_fhr_thermal_hydraulics_state
                    .pipe_5_temp_profile_degc.clone();
                fhr_state_lock.ihx_shell_6_temperature_vector_degc = 
                    current_fhr_thermal_hydraulics_state
                    .ihx_shell_side_temp_profile_degc.clone();
                fhr_state_lock.pipe_7_temperature_vector_degc = 
                    current_fhr_thermal_hydraulics_state
                    .pipe_7_temp_profile_degc.clone();
                fhr_state_lock.pipe_8_temperature_vector_degc = 
                    current_fhr_thermal_hydraulics_state
                    .pipe_8_temp_profile_degc.clone();
                fhr_state_lock.pri_pump_9_temperature_vector_degc = 
                    current_fhr_thermal_hydraulics_state
                    .pump_9_temp_profile_degc.clone();
                fhr_state_lock.pipe_10_temperature_vector_degc = 
                    current_fhr_thermal_hydraulics_state
                    .pipe_10_temp_profile_degc.clone();
                fhr_state_lock.pipe_11_temperature_vector_degc = 
                    current_fhr_thermal_hydraulics_state
                    .pipe_11_temp_profile_degc.clone();
                // intermediate loop state
                fhr_state_lock.ihx_tube_6_temperature_vector_degc = 
                    current_fhr_thermal_hydraulics_state
                    .ihx_tube_side_temp_profile_degc.clone();
                fhr_state_lock.pipe_12_temperature_vector_degc = 
                    current_fhr_thermal_hydraulics_state
                    .pipe_12_temp_profile_degc.clone();
                fhr_state_lock.pipe_13_temperature_vector_degc = 
                    current_fhr_thermal_hydraulics_state
                    .pipe_13_temp_profile_degc.clone();
                // sg = steam generator
                fhr_state_lock.sg_shell_14_temperature_vector_degc = 
                    current_fhr_thermal_hydraulics_state
                    .sg_shell_side_temp_profile_degc.clone();
                fhr_state_lock.pipe_15_temperature_vector_degc = 
                    current_fhr_thermal_hydraulics_state
                    .pipe_15_temp_profile_degc.clone();
                fhr_state_lock.intrmd_pump_16_temperature_vector_degc = 
                    current_fhr_thermal_hydraulics_state
                    .pump_16_temp_profile_degc.clone();
                fhr_state_lock.pipe_17_temperature_vector_degc = 
                    current_fhr_thermal_hydraulics_state
                    .pipe_17_temp_profile_degc.clone();

                // secondary loop state
                fhr_state_lock
                    .user_specified_secondary_loop_mass_flowrate_kg_per_s = 
                    (
                        user_specified_secondary_loop_mass_flowrate
                        .get::<kilogram_per_second>()*1000.0
                    )/1000.0;

                fhr_state_lock 
                    .steam_generator_tube_outlet_temperature_degc = 
                    (
                        current_fhr_steam_gen_state 
                        .steam_gen_tube_outlet_temperature
                        .get::<degree_celsius>()*1000.0
                    )/1000.0;
                fhr_state_lock 
                    .steam_quality_after_condenser = 
                    (
                        current_fhr_steam_gen_state 
                        .steam_quality_after_condenser*1000.0
                    )/1000.0;
                fhr_state_lock 
                    .steam_quality_after_pump = 
                    (current_fhr_steam_gen_state 
                     .steam_quality_after_pump*1000.0
                    )/1000.0;
                fhr_state_lock 
                    .steam_quality_after_steam_generator_tube_side = 
                    (
                        current_fhr_steam_gen_state 
                        .steam_quality_after_steam_generator_tube_side*1000.0
                    )/1000.0;
                fhr_state_lock 
                    .steam_quality_after_turbine = 
                    (
                        current_fhr_steam_gen_state 
                        .steam_quality_after_turbine*1000.0
                    )/1000.0;
                fhr_state_lock
                    .turbine_rpm = 
                    (
                        current_fhr_steam_gen_state 
                        .steam_turbine.get_omega()
                        .get::<revolution_per_minute>()*1000.0
                    )/1000.0;
                fhr_state_lock 
                    .turbine_power_megawatts = 
                    (
                        current_fhr_steam_gen_state 
                        .turbine_power
                        .get::<megawatt>()*1000.0
                    )/1000.0;
                fhr_state_lock 
                    .condenser_duty_megawatts = 
                    (
                        current_fhr_steam_gen_state 
                        .condenser_duty
                        .get::<megawatt>()*1000.0
                    )/1000.0;

                

            }


            let time_to_sleep_microseconds: u64 = 
                (thermal_hydraulics_timestep.get::<microsecond>() - 
                 time_taken_for_calculation_loop_microseconds)
                .round().abs() as u64;

            let time_to_sleep: Duration = 
                Duration::from_micros(time_to_sleep_microseconds - 1);


            // last condition for sleeping
            let real_time_in_current_timestep: bool = 
                time_to_sleep_microseconds > 1;

            //
            let fast_forward_botton_on = false;

            if overall_simulation_in_realtime_or_faster && 
                real_time_in_current_timestep && 
                    !fast_forward_botton_on 
            {
                thread::sleep(time_to_sleep);
            } else if overall_simulation_in_realtime_or_faster 
                && real_time_in_current_timestep 
                    && fast_forward_botton_on 
            {
                // sleep 5 microseconds if fast fwd
                let short_time_to_sleep: Duration = Duration::from_micros(5);
                thread::sleep(short_time_to_sleep);
            } else {
                // don't sleep

            }




        }

    }
}

/// contains simple components for the fhr simulator
///
/// these are components for primary loop and secondary loop 
/// turbine components not included (will be in tampines-steam-tables)
pub mod components;

/// contains functions for calculating pri loop 
/// fluid mechanics
pub mod pri_loop_fluid_mechanics_calc_fns;


/// code for fhr thermal hydraulics state 
pub mod fhr_thermal_hydraulics_state;

/// code responsible for rankine cycle
pub mod secondary_loop;

/// graceful salt-freeze handling: detect a loop dropping below its salt's
/// melting point *before* the property call that would panic, pause the
/// physics thread, and offer the operator a (deliberately unphysical, and
/// labelled as such) melt to resume
pub mod salt_freeze_guard;

#[cfg(test)]
mod steam_generator_pinch_at_the_nominal_operating_point {
    use super::components::*;
    use super::pri_loop_fluid_mechanics_calc_fns::four_branch_pri_and_intermediate_loop_fluid_mechanics_only;
    use super::secondary_loop::steam_generator_duty::{
        feedwater_inlet_state, pinch_limited_steam_generator_duty, SteamGeneratorDutyLimit,
    };
    use tampines_steam_tables::interfaces::functional_programming::ph_flash_eqm;
    use tuas_boussinesq_solver::boussinesq_thermophysical_properties::LiquidMaterial;
    use uom::si::f64::*;
    use uom::si::mass_rate::kilogram_per_second;
    use uom::si::power::watt;
    use uom::si::pressure::{bar, kilopascal};
    use uom::si::thermal_conductance::watt_per_kelvin;
    use uom::si::thermodynamic_temperature::degree_celsius;

    /// The intermediate-loop steam-generator-branch salt flow \[kg/s\] this
    /// plant actually runs at, measured rather than assumed.
    ///
    /// Solves the four-branch fluid-mechanics problem at the shipped default
    /// intermediate-pump pressure with every component at the 500 degC
    /// cold-start temperature. The magnitude is what matters for the capacity
    /// rate; the sign only records which way round the branch the salt goes.
    fn nominal_steam_generator_branch_salt_flow() -> MassRate {
        let cold_start = ThermodynamicTemperature::new::<degree_celsius>(500.0);
        // `FHRState::default()` sets both loop pumps to 100 kPa, which the
        // driver applies as -100 kPa (see `calculate_thermal_hydraulics_loop`).
        let pump_pressure = Pressure::new::<kilopascal>(-100.0);

        let (_reactor, _downcomer_1, _downcomer_2, _pri_ihx, _intrmd_ihx, sg_branch) =
            four_branch_pri_and_intermediate_loop_fluid_mechanics_only(
                pump_pressure,
                pump_pressure,
                &new_reactor_vessel_pipe_1(cold_start),
                &new_downcomer_pipe_2(cold_start),
                &new_downcomer_pipe_3(cold_start),
                &new_fhr_pipe_11(cold_start),
                &new_fhr_pipe_10(cold_start),
                &new_fhr_pri_loop_pump_9(cold_start),
                &new_fhr_pipe_8(cold_start),
                &new_fhr_pipe_7(cold_start),
                &new_ihx_sthe_6_version_1(cold_start),
                &new_fhr_pipe_5(cold_start),
                &new_fhr_pipe_4_ver_2(cold_start),
                &new_fhr_pipe_17(cold_start),
                &new_fhr_pipe_12(cold_start),
                &new_fhr_intermediate_loop_pump_16(cold_start),
                &new_fhr_pipe_15(cold_start),
                &new_fhr_intermediate_loop_steam_generator_shell_side_14(cold_start),
                &new_fhr_pipe_13(cold_start),
            );

        sg_branch
    }

    /// **V&V — where on the `UA` slider the pre-fix model started crossing,
    /// at the plant's real nominal operating point.**
    ///
    /// ## Methodology
    ///
    /// The synthetic sweep in
    /// [`secondary_loop::steam_generator_duty`](super::secondary_loop::steam_generator_duty)
    /// proves the cap holds over the whole slider space, but it varies the
    /// salt flow freely. This test instead pins the *actual* operating point
    /// the simulator ships with, so the answer to "how far do I have to push
    /// the slider before the physics breaks?" is a real number and not a
    /// hypothetical:
    ///
    /// - salt flow **measured** by solving the four-branch fluid-mechanics
    ///   problem at the default pump pressure
    ///   ([`nominal_steam_generator_branch_salt_flow`]);
    /// - shell at 500 degC (the simulator's cold-start / nominal salt
    ///   temperature), HITEC `cp` from `tuas_boussinesq_solver`;
    /// - feedwater 50 kg/s at 1.2 bar (both `FHRState::default()`), giving the
    ///   feedwater inlet from the shared [`feedwater_inlet_state`];
    /// - lagged tube outlet 100 degC.
    ///
    /// The `UA` slider (0 to 7.0e5 W/K) is then walked in 1e3 W/K steps to
    /// find the lowest setting at which the **pre-fix** duty `UA * dT` exceeds
    /// the thermodynamic maximum, and the pinch-limited duty is checked at the
    /// slider maximum.
    ///
    /// ## Results — measured 2026-08-12
    ///
    /// Measured operating point: SG-branch salt flow **642.9 kg/s**, HITEC
    /// `cp` **1560.0 J/(kg K)**, feedwater inlet **35.0 degC** at 1.2 bar.
    /// Thermodynamic maximum **1.6709e8 W = 167.1 MW**, set by the
    /// **feedwater enthalpy pinch** (the cold side runs out of temperature
    /// difference before the 642.9 kg/s salt stream does).
    ///
    /// | `UA` \[W/K\] | Pre-fix duty | What the pre-fix model does |
    /// |---|---|---|
    /// | 1.5e5 (shipped default) | 60.0 MW | fine — below the 167.1 MW maximum, cap inactive |
    /// | **4.18e5** | 167.1 MW | **first crossing**, 60 % of the way up the slider |
    /// | 4.98e5 | 199.2 MW | steam outlet **787.4 degC** against a **500.0 degC** salt inlet — a **287.4 K temperature cross** |
    /// | 5.08e5 | 203.2 MW | steam state leaves the IAPWS-IF97 envelope entirely; the `(p, h)` flash has no solution and **panics the physics thread** |
    /// | 7.0e5 (slider maximum) | 280.0 MW | **1.68x** the thermodynamic maximum; implies cooling the 500 degC salt by 279 K to 221 degC |
    ///
    /// Post-fix at the slider maximum: duty capped at 167.1 MW, effectiveness
    /// exactly 1.0000, `limited_by = FeedwaterEnthalpyPinch`. Post-fix at the
    /// shipped default: 6.0000e7 W = 60.0 MW, `limited_by = Conductance`,
    /// bit-identical to `UA * dT`.
    ///
    /// ## Interpretation
    ///
    /// This is the number that answers the maintainer's report directly.
    /// Dragging the `UA` slider past **60 %** of its range put the simulator
    /// into a second-law violation, and by 73 % it was crashing the physics
    /// thread outright on an out-of-envelope steam state. That matches the
    /// simulator's own long-standing note in `main.rs` — *"the steam
    /// temperature is sometimes too high. This is especially when the UA
    /// value exceeds some number"* — and gives that number: 4.18e5 W/K at
    /// this operating point.
    ///
    /// This test is also the *regression* guard for the fix's most important
    /// non-goal: at the **shipped default** `UA` the cap must be inactive, so
    /// the fix must not have quietly changed the simulator's nominal duty. The
    /// test asserts that too, and it holds — the default duty is unchanged.
    #[test]
    fn the_ua_slider_crossed_partway_up_at_the_nominal_operating_point() {
        let salt_flow = nominal_steam_generator_branch_salt_flow();
        let salt_flow_kg_per_s = salt_flow.get::<kilogram_per_second>().abs();
        let shell = ThermodynamicTemperature::new::<degree_celsius>(500.0);
        let salt_cp = LiquidMaterial::HITEC
            .try_get_cp(shell)
            .expect("HITEC cp at 500 degC");
        let feedwater = feedwater_inlet_state(Pressure::new::<bar>(1.2));
        let feed_flow = MassRate::new::<kilogram_per_second>(50.0);
        let tube_outlet = ThermodynamicTemperature::new::<degree_celsius>(100.0);
        let driving_span_k = 500.0 - 100.0;

        println!(
            "nominal SG-branch salt flow {salt_flow_kg_per_s:.1} kg/s, \
             HITEC cp {:.1} J/(kg K), feedwater inlet {:.1} degC",
            salt_cp.value,
            feedwater.temperature.get::<degree_celsius>()
        );

        let duty_at = |ua_w_per_k: f64| {
            pinch_limited_steam_generator_duty(
                ThermalConductance::new::<watt_per_kelvin>(ua_w_per_k),
                shell,
                shell,
                salt_flow,
                salt_cp,
                feedwater,
                feed_flow,
                tube_outlet,
            )
        };

        let maximum_duty_w = duty_at(7.0e5).thermodynamic_maximum.get::<watt>();
        println!(
            "thermodynamic maximum {:.4e} W = {:.1} MW",
            maximum_duty_w,
            maximum_duty_w / 1.0e6
        );

        // lowest slider setting at which the pre-fix `UA * dT` breaks the cap
        let mut first_crossing_ua: Option<f64> = None;
        let mut ua = 0.0;
        while ua <= 7.0e5 {
            if ua * driving_span_k > maximum_duty_w {
                first_crossing_ua = Some(ua);
                break;
            }
            ua += 1.0e3;
        }
        let first_crossing_ua =
            first_crossing_ua.expect("the slider must be able to reach a crossing");
        println!(
            "pre-fix formula first exceeds the thermodynamic maximum at \
             UA = {first_crossing_ua:.3e} W/K ({:.0}% of the way up the 0-7.0e5 W/K slider)",
            100.0 * first_crossing_ua / 7.0e5
        );

        let at_slider_max = duty_at(7.0e5);
        let legacy_at_slider_max_w = 7.0e5 * driving_span_k;
        println!(
            "at the slider maximum: pre-fix {:.4e} W = {:.1} MW, pinch-limited {:.4e} W \
             = {:.1} MW (eps {:.4}, limited by {:?}); the pre-fix duty is {:.2}x the \
             maximum and would cool the salt by {:.0} K, to {:.0} degC",
            legacy_at_slider_max_w,
            legacy_at_slider_max_w / 1.0e6,
            at_slider_max.duty.get::<watt>(),
            at_slider_max.duty.get::<watt>() / 1.0e6,
            at_slider_max.effectiveness,
            at_slider_max.limited_by,
            legacy_at_slider_max_w / maximum_duty_w,
            legacy_at_slider_max_w / (salt_flow_kg_per_s * salt_cp.value),
            500.0 - legacy_at_slider_max_w / (salt_flow_kg_per_s * salt_cp.value),
        );

        // How large a temperature cross the pre-fix model actually produced,
        // walked up the slider. Above some setting the steam state leaves the
        // IAPWS-IF97 envelope entirely and the `(p, h)` flash would panic --
        // which is itself one of the crashes this simulator suffers -- so the
        // walk stops at the last flashable point and says so.
        let ceiling_enthalpy = feedwater.enthalpy + Power::new::<watt>(maximum_duty_w) / feed_flow;
        let mut last_flashable: Option<(f64, f64, f64)> = None;
        let mut ua = first_crossing_ua;
        while ua <= 7.0e5 {
            let legacy_w = ua * driving_span_k;
            let outlet_enthalpy =
                feedwater.enthalpy + Power::new::<watt>(legacy_w) / feed_flow;
            if outlet_enthalpy > ceiling_enthalpy * 3.0 {
                // far outside the envelope; do not attempt the flash
                break;
            }
            // The flash panics once the state leaves the envelope; silence
            // the default hook around it so the expected out-of-range does
            // not print a scary backtrace in an otherwise passing test.
            let previous_hook = std::panic::take_hook();
            std::panic::set_hook(Box::new(|_| {}));
            let flashed = std::panic::catch_unwind(|| {
                ph_flash_eqm::t_ph_eqm(feedwater.pressure, outlet_enthalpy)
                    .get::<degree_celsius>()
            });
            std::panic::set_hook(previous_hook);
            match flashed {
                Ok(steam_outlet_degc) => {
                    last_flashable = Some((ua, legacy_w, steam_outlet_degc));
                }
                Err(_) => {
                    println!(
                        "at UA = {ua:.3e} W/K the pre-fix duty of {:.1} MW throws the                          steam state clean out of the IAPWS-IF97 envelope -- the (p, h)                          flash has no solution and panics the physics thread. This is                          one of the simulator's crashes, caused directly by the                          unbounded duty.",
                        legacy_w / 1.0e6
                    );
                    break;
                }
            }
            ua += 1.0e4;
        }
        if let Some((ua, legacy_w, steam_outlet_degc)) = last_flashable {
            println!(
                "worst pre-fix temperature cross still inside IF97: at UA = {ua:.3e} W/K \
                 the {:.1} MW duty puts the steam outlet at {steam_outlet_degc:.1} degC \
                 against a 500.0 degC salt inlet -- a {:.1} K cross",
                legacy_w / 1.0e6,
                steam_outlet_degc - 500.0
            );
            assert!(
                steam_outlet_degc > 500.0,
                "the pre-fix formula must produce a genuine cross here"
            );
        }

        let at_default = duty_at(1.5e5);
        println!(
            "at the shipped default UA = 1.5e5 W/K: duty {:.4e} W = {:.1} MW, \
             limited by {:?} (cap inactive => nominal behaviour preserved)",
            at_default.duty.get::<watt>(),
            at_default.duty.get::<watt>() / 1.0e6,
            at_default.limited_by
        );

        // The defect was reachable by dragging the slider, not only at its end
        assert!(
            first_crossing_ua > 0.0 && first_crossing_ua < 7.0e5,
            "the crossing threshold {first_crossing_ua:.3e} W/K must lie strictly \
             inside the 0-7.0e5 W/K slider range"
        );
        // The cap binds at the top of the slider ...
        assert!(
            (at_slider_max.effectiveness - 1.0).abs() < 1.0e-9,
            "at the slider maximum the exchanger must be pinch-limited, \
             effectiveness was {}",
            at_slider_max.effectiveness
        );
        // ... and is inactive at the shipped default, so the fix does not
        // change how the simulator behaves out of the box.
        assert_eq!(
            at_default.limited_by,
            SteamGeneratorDutyLimit::Conductance,
            "the shipped default UA must not be throttled by the cap"
        );
        assert!(
            (at_default.duty.get::<watt>() - 1.5e5 * driving_span_k).abs()
                / (1.5e5 * driving_span_k)
                < 1.0e-9,
            "at the default UA the duty must still be exactly UA * dT"
        );
    }
}
