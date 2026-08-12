use crate::{FHRSimulatorApp};
use crate::app::thermal_hydraulics_backend::fhr_thermal_hydraulics_state::FHRThermalHydraulicsState;
use crate::app::thermal_hydraulics_backend::secondary_loop::steam_generator_duty::{
    feedwater_inlet_state, FeedwaterInletState,
};
use tampines_steam_tables::interfaces::functional_programming::ph_flash_eqm::x_ph_flash;
use tampines_steam_tables::interfaces::functional_programming::ps_flash_eqm::x_ps_flash;
use tampines_steam_tables::interfaces::functional_programming::{ph_flash_eqm, ps_flash_eqm, pt_flash_eqm};
use tampines_steam_tables::prelude::functional_programming::ph_flash_eqm::v_ph_eqm;
use tampines_steam_tables::region_4_vap_liq_equilibrium::sat_temp_4;
use tampines_steam_tables::steam_turbine_equations::ThreePhaseElectricGeneratorTurbine;
use tampines_steam_tables::TampinesSteamArray;
use uom::si::area::square_meter;
use uom::si::f64::*;
use uom::si::length::{inch, meter};
use uom::si::mass_rate::kilogram_per_second;
use uom::si::power::watt;
use uom::si::pressure::{bar, pascal};
use uom::si::ratio::ratio;
use uom::si::thermodynamic_temperature::degree_celsius;
use uom::si::time::second;
use uom::si::velocity::meter_per_second;

/// Number of cells in the spatially-resolved steam-generator tube
/// ([`TampinesSteamArray`]).
const SG_TUBE_CELLS: i64 = 15;
/// Steam-generator tube length \[m\].
const SG_TUBE_LENGTH_M: f64 = 2.0;
/// **Aggregate** flow area of the steam-generator tube bundle \[m²\], chosen
/// so the full secondary mass flow (tens of kg/s) gives a *gentle* feedwater
/// velocity of a few tenths of a m/s -- the 1-D array models the whole
/// bundle as one representative tube, and a near-incompressible liquid must
/// be driven gently to stay inside the IAPWS-IF97 envelope (see the
/// stability guide).
const SG_TUBE_AREA_M2: f64 = 0.2;
/// Nominal feedwater velocity the tube is pre-initialised at \[m/s\], matched
/// to the design flow (~50 kg/s over the aggregate area) so the liquid
/// column does not accelerate abruptly on the first driven step.
const SG_TUBE_NOMINAL_VELOCITY_M_PER_S: f64 = 0.25;
/// [`TampinesSteamArray`] internal timestep \[s\], set by the *liquid*
/// acoustic CFL (`dx/c ≈ (2/15)/1450 ≈ 9e-5 s`), not the flow.
const SG_TUBE_DT_S: f64 = 5.0e-5;
/// How many array sub-steps to advance per outer TH timestep. The array is a
/// **persistent, quasi-steady** sub-model: it is not re-converged each call
/// (that would need thousands of acoustic-CFL steps); instead it is nudged a
/// bounded amount each TH step and relaxes toward the current boundary
/// conditions over real time — a steam generator physically takes seconds to
/// heat up.
const SG_TUBE_SUBSTEPS_PER_TH_STEP: usize = 25;

/// Builds the persistent steam-generator-tube [`TampinesSteamArray`],
/// pre-initialised as subcooled liquid feedwater already flowing at ~1 m/s
/// (so the near-incompressible liquid does not water-hammer on start-up) with
/// default pressure bounding and PIMPLE under-relaxation. Created once by
/// [`FHRSimulatorApp::calculate_thermal_hydraulics_loop`] and driven each TH
/// step by [`FHRSimulatorApp::secondary_loop_single_timestep`].
pub(crate) fn build_steam_generator_tube() -> TampinesSteamArray {
    let mut tube = TampinesSteamArray::new(
        Length::new::<meter>(SG_TUBE_LENGTH_M),
        Area::new::<square_meter>(SG_TUBE_AREA_M2),
        SG_TUBE_CELLS,
        Time::new::<second>(SG_TUBE_DT_S),
    )
    .expect("valid steam-generator tube geometry");

    // PIMPLE with heavy under-relaxation + default pressure bounding: robust
    // for a near-incompressible liquid heated through the saturation dome.
    tube.set_pimple_algorithm(1, 2, Ratio::new::<ratio>(0.2), Ratio::new::<ratio>(0.2));

    // Pre-initialise as ~90 degC subcooled feedwater at the default pump
    // outlet pressure (1.2 bar), already moving at the nominal velocity.
    // Matching the operating pressure avoids a depressurisation rarefaction
    // (which would cool a cell below the 273.15 K validity floor) on the
    // first driven step.
    let p0 = Pressure::new::<bar>(1.2);
    let t0 = ThermodynamicTemperature::new::<degree_celsius>(90.0);
    let n = SG_TUBE_CELLS as usize;
    for c in 0..n {
        tube.p.internal[c] = p0.get::<pascal>();
    }
    tube.set_temperature_vector(vec![t0; n]).unwrap();
    tube.set_uniform_velocity_field(
        Velocity::new::<meter_per_second>(SG_TUBE_NOMINAL_VELOCITY_M_PER_S),
    );
    tube
}

impl FHRSimulatorApp {
    /// contains info for calculating secondary loop for one 
    /// timestep 
    ///
    /// the key thing is the tube side temperature
    ///
    /// note that in this simplified steam generator calculation,
    /// everything instantly goes to steady state
    pub(crate) fn secondary_loop_single_timestep(
        fhr_th_state: &mut FHRThermalHydraulicsState,
        timestep: Time,
        user_specified_secondary_loop_mass_flowrate: &mut MassRate,
        // pump settings
        user_specified_pump_outlet_pressure: Pressure,
        current_simulation_time: Time,
        turbine_omega: AngularVelocity,
        load_resistance: ElectricalResistance,
        // persistent spatially-resolved steam-generator tube (see
        // `build_steam_generator_tube`); driven each TH step and read back
        // for the tube outlet enthalpy, replacing the old lumped
        // `h_out = h_in + Q/mdot` energy balance.
        steam_generator_tube: &mut TampinesSteamArray,
    ) -> SecondaryLoopState {
        let heat_rate_to_steam_generator_tube =
            -fhr_th_state.heat_added_to_steam_generator_shell_side
            /timestep;
        // first start at the boundary condition
        
        // this is condenser outlet
        // we start at 0.1 bar, 35 degrees C
        let condenser_outlet_pressure = Pressure::new::<bar>(0.1);
        let condenser_outlet_temperature = 
            ThermodynamicTemperature::new::<degree_celsius>(35.0);


        // then calculate the entropy...
        //
        let condenser_outlet_entropy: SpecificHeatCapacity
            = pt_flash_eqm::s_tp_eqm_single_phase(
                condenser_outlet_temperature,
                condenser_outlet_pressure);
        let steam_quality_after_condenser =
            x_ps_flash(condenser_outlet_pressure,
                condenser_outlet_entropy);

        // then condenser goes into pump... with new pressure
        // assume pump is isentropic... for simplicity
        //
        // it pumps it up to a minimum of 1 bar.
        //
        // This is delegated to `feedwater_inlet_state` so that the
        // steam-generator *pinch cap* (which needs the cold-side inlet to
        // compute `Q_max`, see `steam_generator_duty`) and this cycle
        // calculation can never disagree about where the cold stream starts.
        // A cap computed against a different feedwater state than the cycle
        // actually uses would not be a cap at all.
        let feedwater_inlet: FeedwaterInletState =
            feedwater_inlet_state(user_specified_pump_outlet_pressure);

        let pump_outlet_temperature: ThermodynamicTemperature =
            feedwater_inlet.temperature;
        let sg_inlet_pressure: Pressure = feedwater_inlet.pressure;

        // now the pump outlet temperature will form the
        // inlet of the steam generator

        let steam_generator_tube_inlet_enthalpy: AvailableEnergy =
            feedwater_inlet.enthalpy;

        let pressure_after_pump =
            sg_inlet_pressure;
        let enthalpy_after_pump = steam_generator_tube_inlet_enthalpy;

        let steam_quality_after_pump = 
            x_ph_flash(pressure_after_pump, enthalpy_after_pump);


        // now, if mass flowrate is less than some value, crash the
        // system because the steam generator is too hot
        if *user_specified_secondary_loop_mass_flowrate <
            MassRate::new::<kilogram_per_second>(0.1) {
                println!("steam generator feedwater flowrate too low!");
                panic!("mass flowrate <0.1 kg/s, unsafe operation");
        }

        let sg_outlet_pressure = sg_inlet_pressure;

        // ── Spatially-resolved steam-generator tube (TampinesSteamArray) ──
        // Replaces the old lumped energy balance
        //   h_out = h_in + Q_dot / mdot
        // with the real IAPWS-IF97 compressible-flow steam-table physics.
        // The feedwater enters as subcooled liquid, a distributed shell-side
        // heat source (the primary-side heat rate) is applied along the tube,
        // and the downstream pressure is fixed; the array is stepped a bounded
        // number of times (it is a persistent, quasi-steady sub-model — see
        // `build_steam_generator_tube`) and its outlet enthalpy is read back.
        {
            // Feedwater inlet velocity from the mass flow and the aggregate
            // bundle area: v = mdot / (rho_feed · A). rho_feed from the pump-
            // outlet (T, p).
            let feed_rho = pt_flash_eqm::v_tp_eqm_single_phase(
                pump_outlet_temperature, sg_inlet_pressure,
            ).recip();
            let inlet_velocity: Velocity =
                *user_specified_secondary_loop_mass_flowrate
                    / feed_rho
                    / Area::new::<square_meter>(SG_TUBE_AREA_M2);

            steam_generator_tube.set_inlet_velocity(inlet_velocity);
            steam_generator_tube.set_inlet_enthalpy(steam_generator_tube_inlet_enthalpy);
            steam_generator_tube.set_outlet_pressure(sg_outlet_pressure);

            // Distributed shell-side heat (clamp to >= 0: a steam generator
            // receives heat; a spuriously negative rate would just be no heat).
            let heat_w = heat_rate_to_steam_generator_tube.get::<watt>().max(0.0);
            let total_heat = Power::new::<watt>(heat_w);
            let n_cells = SG_TUBE_CELLS as usize;
            let q_fraction = vec![1.0 / n_cells as f64; n_cells];

            for _ in 0..SG_TUBE_SUBSTEPS_PER_TH_STEP {
                steam_generator_tube
                    .lateral_link_new_power_vector(total_heat, q_fraction.clone())
                    .unwrap();
                steam_generator_tube.step();
            }
        }

        let steam_generator_tube_outlet_enthalpy =
            steam_generator_tube.get_outlet_enthalpy();
        // this is to get saturation temperature in steam generator tubes
        let sat_temperature_in_sg_tube_degc 
            = sat_temp_4(sg_outlet_pressure).get::<degree_celsius>();

        let steam_gen_tube_outlet_temperature = 
            ph_flash_eqm::t_ph_eqm(
                sg_outlet_pressure, 
                steam_generator_tube_outlet_enthalpy
            );

        // then with this enthalpy and pressure, have an isentropic 
        // enthalpy drop in the turbine 
        // to specified pressure (0.2 bar) 

        let turbine_inlet_enthalpy = 
            steam_generator_tube_outlet_enthalpy;
        let turbine_inlet_pressure = sg_outlet_pressure;

        let pressure_after_steam_generator_tube = 
            turbine_inlet_pressure;
        let enthalpy_after_steam_generator_tube = 
            turbine_inlet_enthalpy;
        let steam_quality_after_steam_generator_tube_side = 
            x_ph_flash(pressure_after_steam_generator_tube, 
                enthalpy_after_steam_generator_tube);

        let turbine_inlet_entropy = ph_flash_eqm::s_ph_eqm(
            pressure_after_steam_generator_tube, 
            steam_generator_tube_outlet_enthalpy
        );

        // this is fixed... can modify in future to make this user changeable
        let turbine_outlet_pressure = 
            Pressure::new::<bar>(0.2);
        let turbine_outlet_entropy = turbine_inlet_entropy;
        

        let turbine_outlet_enthalpy = 
            ps_flash_eqm::h_ps_eqm(
                turbine_outlet_pressure, 
                turbine_outlet_entropy
            );

        let steam_quality_after_turbine = 
            x_ph_flash(turbine_outlet_pressure, 
                turbine_outlet_enthalpy);



        // now it goes into the condenser
        let condenser_inlet_enthalpy = turbine_outlet_enthalpy;
        let condenser_outlet_enthalpy =
            pt_flash_eqm::h_tp_eqm_single_phase(
                condenser_outlet_temperature, condenser_outlet_pressure
            );

        // this is the required cooling rate for condenser at steady state
        let condenser_duty: Power = 
            *user_specified_secondary_loop_mass_flowrate * 
            (condenser_inlet_enthalpy - condenser_outlet_enthalpy);

        // now the steam turbine, 
        let mut steam_turbine = 
            ThreePhaseElectricGeneratorTurbine::new_250_megawatt_generator();

        steam_turbine.set_omega(turbine_omega);


        let torque_source: Torque;
        // these are some arbitrary Parameters
        let turbine_enthalpy_loss: AvailableEnergy = 
            turbine_inlet_enthalpy - turbine_outlet_enthalpy;

        let turbine_power_input: Power = 
            *user_specified_secondary_loop_mass_flowrate * 
            turbine_enthalpy_loss;

        let turbine_inlet_area = Length::new::<inch>(20.0) * Length::new::<inch>(20.0);


        let turbine_radius = Length::new::<meter>(5.0);
        let turbine_blade_speed: Velocity = 
            turbine_radius * turbine_omega;

        let turbine_steam_speed: Velocity;
        let turbine_inlet_rho: MassDensity = 
            v_ph_eqm(turbine_inlet_pressure, turbine_inlet_enthalpy).recip();

        turbine_steam_speed = *user_specified_secondary_loop_mass_flowrate
            /turbine_inlet_rho/ 
            turbine_inlet_area;

        // note: using relative velocity is kind of unstable
        // numerically
        let _turbine_steam_relative_velocity: Velocity = 
            turbine_steam_speed - turbine_blade_speed;

        let turbine_force: Force = 
            turbine_power_input/turbine_blade_speed;

        // this is just an arbitrary efficiency factor
        // I don't have actual figures, so the efficiency ratios may 
        // be incorrect
        let efficiency_factor = 0.75;



        torque_source = 
            (turbine_force * turbine_radius * efficiency_factor).into();


        steam_turbine.advance_timestep(
            torque_source, 
            load_resistance, 
            current_simulation_time, 
            timestep,
        );
        let turbine_power: Power = 
            steam_turbine.get_power(load_resistance, current_simulation_time);



        let secondary_loop_state = 
            SecondaryLoopState {
                steam_gen_tube_outlet_temperature,
                turbine_power,
                condenser_duty,
                steam_quality_after_condenser,
                steam_quality_after_pump,
                steam_quality_after_steam_generator_tube_side,
                steam_quality_after_turbine,
                sat_temperature_in_sg_tube_degc,
                steam_turbine,
            };


        return secondary_loop_state;

    }
}

/// contains important info for various temperatures and pressures 
/// in the secondary loop
#[derive(Debug,Clone)]
pub struct SecondaryLoopState {
    /// steam generator tube outlet temperature (boiler temperature)
    pub steam_gen_tube_outlet_temperature: ThermodynamicTemperature,
    /// turbine power output 
    pub turbine_power: Power,
    /// condenser cooling duty at steady state
    pub condenser_duty: Power,

    /// steam quality (void fraction) after condenser 
    pub steam_quality_after_condenser: f64,
    /// steam quality (void fraction) after pump
    pub steam_quality_after_pump: f64,
    /// steam quality (void fraction) after steam generator tube 
    /// side
    pub steam_quality_after_steam_generator_tube_side: f64,
    /// steam quality (void fraction) after steam turbine
    pub steam_quality_after_turbine: f64,

    /// sat temperature in sg tube 
    pub sat_temperature_in_sg_tube_degc: f64,

    /// steam turbine 
    pub steam_turbine: ThreePhaseElectricGeneratorTurbine,

}

/// some code for departure from nucleate boiling.
/// https://www.nuclear-power.com/nuclear-engineering/heat-transfer/boiling-and-condensation/boiling-crisis-critical-heat-flux/
///
/// This is to prevent excessively high temperatures at the heat exchanger
/// thus causing simulation crash
///
/// I use pool boiling curves here because Departure from Nucleate 
/// Boiling or DNB should occur earlier than 
/// for flow boiling. So perhaps this is a conservative estimate.
///
///
/// Now, even for pool boiling, lots of correlations exist for said DNB 
///
/// I'm doing only a very simplified version.
pub mod pool_boiling;

/// Second-law-safe steam-generator duty: the effectiveness-NTU cap that makes
/// a tube-side/shell-side temperature cross impossible by construction, plus
/// the shared definition of the feedwater inlet state both this module and the
/// shell-side heat balance read.
pub mod steam_generator_duty;

#[cfg(test)]
pub mod vibe_code_mass_balance;
