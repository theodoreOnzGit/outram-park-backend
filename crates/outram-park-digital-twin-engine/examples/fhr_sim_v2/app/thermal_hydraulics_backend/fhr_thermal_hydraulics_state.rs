/// this is the struct to contain the thermal hydraulics state of the fhr
use crate::app::thermal_hydraulics_backend::secondary_loop::steam_generator_duty::SteamGeneratorDutyLimit;
use uom::si::f64::*;
#[derive(Debug,Clone)]
pub struct FHRThermalHydraulicsState {
    /// reactor branch flow (upwards through the core)
    /// note that positive flow means from bottom mixing node to top
    pub reactor_branch_flow: MassRate,
    /// downcomer 1 branch flow (upwards through the core)
    /// note that positive flow means from bottom mixing node to top
    pub downcomer_branch_1_flow: MassRate,
    /// downcomer 2 branch flow (upwards through the core)
    /// note that positive flow means from bottom mixing node to top
    pub downcomer_branch_2_flow: MassRate,
    /// ihx branch flow 
    /// note that positive flow means from bottom mixing node to top
    pub intermediate_heat_exchanger_branch_flow: MassRate,
    /// ihx branch flow 
    /// note that positive flow means from bottom 
    /// (between pipe 17 and pump 16) 
    /// to top
    /// (between pipe 12 and pipe 13)
    pub intrmd_loop_ihx_br_flow: MassRate,
    /// steam generator branch
    /// note that positive flow means from bottom 
    /// (between pipe 17 and pump 16) 
    /// to top
    /// (between pipe 12 and pipe 13)
    pub intrmd_loop_steam_gen_br_flow: MassRate,

    // other diagnostics 
    /// shows the current simulation time
    pub simulation_time: Time,

    // temperature diagnostics 
    /// shows the current reactor temperature profile in degc (2dp)
    pub reactor_temp_profile_degc: Vec<f64>,
    /// shows the current ihx shell side temperature profile in degc (2dp)
    pub ihx_shell_side_temp_profile_degc: Vec<f64>,
    /// shows the current ihx tube side temperature profile in degc (2dp)
    pub ihx_tube_side_temp_profile_degc: Vec<f64>,
    /// shows the current steam generator side temperature profile in degc (2dp)
    pub sg_shell_side_temp_profile_degc: Vec<f64>,

    /// shows the temperature profile of pipe_4
    pub pipe_4_temp_profile_degc: Vec<f64>,
    /// shows the temperature profile of pipe_5
    pub pipe_5_temp_profile_degc: Vec<f64>,
    /// shows the temperature profile of pipe_7
    pub pipe_7_temp_profile_degc: Vec<f64>,
    /// shows the temperature profile of pipe_8
    pub pipe_8_temp_profile_degc: Vec<f64>,
    /// shows the temperature profile of pump_9 in the primary loop
    pub pump_9_temp_profile_degc: Vec<f64>,
    /// shows the temperature profile of pipe_10
    pub pipe_10_temp_profile_degc: Vec<f64>,
    /// shows the temperature profile of pipe_11
    pub pipe_11_temp_profile_degc: Vec<f64>,


    // intermediate loop

    /// shows the temperature profile of pipe_12
    pub pipe_12_temp_profile_degc: Vec<f64>,
    /// shows the temperature profile of pipe_13
    pub pipe_13_temp_profile_degc: Vec<f64>,
    /// shows the temperature profile of pipe_15
    pub pipe_15_temp_profile_degc: Vec<f64>,
    /// shows the temperature profile of pump_16 in the intermediate loop
    pub pump_16_temp_profile_degc: Vec<f64>,
    /// shows the temperature profile of pipe_17
    pub pipe_17_temp_profile_degc: Vec<f64>,

    // downcomers
    /// shows the temperature profile of pipe_12
    pub downcomer_2_temp_profile_degc: Vec<f64>,
    /// shows the temperature profile of pipe_13
    pub downcomer_3_temp_profile_degc: Vec<f64>,

    // for coupling to secondary loop
    /// heat added to steam generator
    pub heat_added_to_steam_generator_shell_side: Energy,

    /// Steam-generator effectiveness `Q / Q_max` for this timestep,
    /// dimensionless and always within `[0, 1]`.
    ///
    /// `Q_max = C_min (T_salt_in - T_feed_in)` is the counter-flow
    /// thermodynamic maximum; an effectiveness of exactly 1 means the
    /// exchanger is pinch-limited and raising the `UA` slider further will not
    /// (and physically cannot) transfer any more heat. See
    /// [`secondary_loop::steam_generator_duty`] for the derivation and the
    /// V&V sweep.
    ///
    /// [`secondary_loop::steam_generator_duty`]:
    ///     crate::app::thermal_hydraulics_backend::secondary_loop::steam_generator_duty
    pub steam_generator_effectiveness: f64,

    /// The counter-flow thermodynamic maximum duty `Q_max` \[W\] the transfer
    /// was clamped against this timestep. Reported alongside
    /// [`Self::steam_generator_effectiveness`] so a user who finds the `UA`
    /// slider unresponsive can see the number that is actually binding.
    pub steam_generator_maximum_duty: Power,

    /// Which physical constraint set the steam-generator duty this timestep:
    /// the `UA` conductance (the normal, well-posed regime), the salt capacity
    /// rate, the feedwater enthalpy pinch, or the absence of any driving
    /// temperature difference.
    pub steam_generator_duty_limit: SteamGeneratorDutyLimit,

}

