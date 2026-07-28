//! Plant state and plot history for the CIET Educational Simulator v2.
//!
//! ## Provenance
//!
//! Ported from the CIET Educational Simulator **v1**
//! (`crates/tuas_boussinesq_solver/examples/ciet_educational_simulator/ciet_simulator_v1/app/panels_and_pages/ciet_data.rs`),
//! GPL-3.0, same licence. [`PagePlotData`] below is v1's code, unchanged.
//!
//! **What v2 changed here:** v1 defined its own `CIETState` struct in this file.
//! v2 does not: the plant state now lives in the crate library at
//! [`outram_park_digital_twin_engine::ciet_opcua::state`], so that the physics
//! thread, the egui GUI *and* the OPC-UA server all share one definition. This
//! file re-exports it under both names so the ported pages keep compiling.
//! `Arc<Mutex<_>>` also became `Arc<RwLock<_>>` at every call site.
//!
//! `PagePlotData` deliberately stayed here: it is plotting/CSV history for the
//! GUI, not plant state, and nothing outside this binary needs it.

/// The CIET plant state, shared between the physics, OPC-UA and GUI threads.
///
/// Defined in the crate library so the OPC-UA layer and the GUI agree on it;
/// see [`outram_park_digital_twin_engine::ciet_opcua::state::CietState`] for
/// the field-by-field documentation (units, valid ranges, which fields are
/// controls and which are outputs).
pub use outram_park_digital_twin_engine::ciet_opcua::state::CietState;

/// v1-compatible alias: the ported v1 tree refers to this type as `CIETState`.
///
/// Kept so the port stays a faithful translation of v1 rather than a rename
/// sweep. New code should prefer [`CietState`].
pub use outram_park_digital_twin_engine::ciet_opcua::state::CietState as CIETState;

use uom::si::f64::*;
use uom::si::heat_transfer::watt_per_square_meter_kelvin;
use uom::si::mass_rate::kilogram_per_second;
use uom::si::power::kilowatt;
use uom::si::pressure::pascal;
use uom::si::thermodynamic_temperature::{degree_celsius, kelvin};
use uom::si::time::second;
use uom::ConstZero;

/// this is the struct used to store data for graph plotting and
/// csv extraction
/// have to lock this in an Arc Mutex pointer for parallelism
#[derive(Debug, Clone)]
pub struct PagePlotData {
    /// the heater data here is a tuple,
    ///
    /// simulation time, heater power, inlet temp and outlet temp
    pub heater_plot_data: Vec<(
        Time,
        Power,
        ThermodynamicTemperature,
        ThermodynamicTemperature,
    )>,

    /// the CTAH data in a tuple, I want it to have the
    /// Time
    /// heat transfer coeff,
    /// Inlet Temperature
    /// Outlet Temperature
    /// Outlet Temperature Set pt
    ///
    pub ctah_plot_data: Vec<(
        Time,
        HeatTransfer,
        ThermodynamicTemperature,
        ThermodynamicTemperature,
        ThermodynamicTemperature,
    )>,

    /// the TCHX data in a tuple
    /// Time
    /// heat transfer coeff,
    /// Inlet Temperature
    /// Outlet Temperature
    /// Outlet Temperature Set pt
    pub tchx_plot_data: Vec<(
        Time,
        HeatTransfer,
        ThermodynamicTemperature,
        ThermodynamicTemperature,
        ThermodynamicTemperature,
    )>,
    // time,
    // pump pressure
    // tube mass flowrate,
    // ctah pump temperature
    pub ctah_pump_plot_data: Vec<(Time, Pressure, MassRate, ThermodynamicTemperature)>,

    // time,
    // shell mass flowrate ,
    // tube mass flowrate,
    // dhx shell inlet temp
    // dhx shell outlet temp
    // dhx tube inlet temp
    // dhx tube outlet temp
    pub dhx_plot_data: Vec<(
        Time,
        MassRate,
        MassRate,
        ThermodynamicTemperature,
        ThermodynamicTemperature,
        ThermodynamicTemperature,
        ThermodynamicTemperature,
    )>,

    // recording interval for graphs
    pub graph_data_record_interval_seconds: f64,

    // recording interval for csv
    pub csv_display_interval_seconds: f64,
}

pub const NUM_DATA_PTS_IN_PLOTS: usize = 4000;

impl PagePlotData {
    /// inserts a data point, most recent being on top
    pub fn insert_heater_data(
        &mut self,
        simulation_time: Time,
        heater_power: Power,
        inlet_temp_bt11: ThermodynamicTemperature,
        outlet_temp_bt12: ThermodynamicTemperature,
    ) {
        // first convert into a tuple,

        let data_tuple = (
            simulation_time,
            heater_power,
            inlet_temp_bt11,
            outlet_temp_bt12,
        );

        // now insert this into the heater
        // how?
        // map the vectors out first
        let mut current_heater_data_vec: Vec<(
            Time,
            Power,
            ThermodynamicTemperature,
            ThermodynamicTemperature,
        )>;

        current_heater_data_vec = self.heater_plot_data.iter().map(|&values| values).collect();

        // now, insert the latest data at the top
        current_heater_data_vec.insert(0, data_tuple);

        // take the first NUM_DATA_PTS_IN_PLOTS pieces as a fixed size array
        // which is basically the array size

        let mut new_array_to_be_put_back: Vec<(
            Time,
            Power,
            ThermodynamicTemperature,
            ThermodynamicTemperature,
        )> = vec![
            (
                Time::ZERO,
                Power::ZERO,
                ThermodynamicTemperature::ZERO,
                ThermodynamicTemperature::ZERO
            );
            NUM_DATA_PTS_IN_PLOTS
        ];

        // map the first NUM_DATA_PTS_IN_PLOTS values of the current heater data vec

        for n in 0..NUM_DATA_PTS_IN_PLOTS {
            new_array_to_be_put_back[n] = current_heater_data_vec[n];
        }

        self.heater_plot_data = new_array_to_be_put_back;
    }

    pub fn insert_ctah_data(
        &mut self,
        simulation_time: Time,
        ctah_heat_transfer_coeff: HeatTransfer,
        inlet_temp_bt43: ThermodynamicTemperature,
        outlet_temp_bt41: ThermodynamicTemperature,
        outlet_temp_set_pt: ThermodynamicTemperature,
    ) {
        let data_tuple = (
            simulation_time,
            ctah_heat_transfer_coeff,
            inlet_temp_bt43,
            outlet_temp_bt41,
            outlet_temp_set_pt,
        );

        // now insert this into the heater
        // how?
        // map the vectors out first
        let mut current_ctah_data_vec: Vec<(
            Time,
            HeatTransfer,
            ThermodynamicTemperature,
            ThermodynamicTemperature,
            ThermodynamicTemperature,
        )>;

        current_ctah_data_vec = self.ctah_plot_data.iter().map(|&values| values).collect();

        // now, insert the latest data at the top
        current_ctah_data_vec.insert(0, data_tuple);

        // take the first NUM_DATA_PTS_IN_PLOTS pieces as a fixed size array
        // which is basically the array size

        let mut new_array_to_be_put_back: Vec<(
            Time,
            HeatTransfer,
            ThermodynamicTemperature,
            ThermodynamicTemperature,
            ThermodynamicTemperature,
        )> = vec![
            (
                Time::ZERO,
                HeatTransfer::ZERO,
                ThermodynamicTemperature::ZERO,
                ThermodynamicTemperature::ZERO,
                ThermodynamicTemperature::ZERO
            );
            NUM_DATA_PTS_IN_PLOTS
        ];

        // map the first NUM_DATA_PTS_IN_PLOTS values of the current heater data vec

        for n in 0..NUM_DATA_PTS_IN_PLOTS {
            new_array_to_be_put_back[n] = current_ctah_data_vec[n];
        }

        self.ctah_plot_data = new_array_to_be_put_back;
    }

    pub fn insert_tchx_data(
        &mut self,
        simulation_time: Time,
        tchx_heat_transfer_coeff: HeatTransfer,
        inlet_temp_bt65: ThermodynamicTemperature,
        outlet_temp_bt66: ThermodynamicTemperature,
        outlet_temp_set_pt: ThermodynamicTemperature,
    ) {
        let data_tuple = (
            simulation_time,
            tchx_heat_transfer_coeff,
            inlet_temp_bt65,
            outlet_temp_bt66,
            outlet_temp_set_pt,
        );

        // now insert this into the heater
        // how?
        // map the vectors out first
        let mut current_tchx_data_vec: Vec<(
            Time,
            HeatTransfer,
            ThermodynamicTemperature,
            ThermodynamicTemperature,
            ThermodynamicTemperature,
        )>;

        current_tchx_data_vec = self.tchx_plot_data.iter().map(|&values| values).collect();

        // now, insert the latest data at the top
        current_tchx_data_vec.insert(0, data_tuple);

        // take the first NUM_DATA_PTS_IN_PLOTS pieces as a fixed size array
        // which is basically the array size

        let mut new_array_to_be_put_back: Vec<(
            Time,
            HeatTransfer,
            ThermodynamicTemperature,
            ThermodynamicTemperature,
            ThermodynamicTemperature,
        )> = vec![
            (
                Time::ZERO,
                HeatTransfer::ZERO,
                ThermodynamicTemperature::ZERO,
                ThermodynamicTemperature::ZERO,
                ThermodynamicTemperature::ZERO
            );
            NUM_DATA_PTS_IN_PLOTS
        ];

        // map the first NUM_DATA_PTS_IN_PLOTS values of the current heater data vec

        for n in 0..NUM_DATA_PTS_IN_PLOTS {
            new_array_to_be_put_back[n] = current_tchx_data_vec[n];
        }

        self.tchx_plot_data = new_array_to_be_put_back;
    }
    /// gets bt 65 data over time
    /// time in second, temp in degc
    pub fn get_bt_65_degc_vs_time_secs_vec(&self) -> Vec<[f64; 2]> {
        let time_bt65_vec: Vec<[f64; 2]> = self
            .tchx_plot_data
            .iter()
            .map(|tuple| {
                let (time, _tchx_htc, bt65, _bt66, _bt66_setpt) = *tuple;

                if bt65.get::<kelvin>() > 0.0 {
                    [time.get::<second>(), bt65.get::<degree_celsius>()]
                } else {
                    // don't return anything, a default 20.0 will do
                    // this is the initial condition
                    [0.0, 20.0]
                }
            })
            .collect();

        return time_bt65_vec;
    }
    /// gets bt 66 data over time
    /// time in second, temp in degc
    pub fn get_bt_66_degc_vs_time_secs_vec(&self) -> Vec<[f64; 2]> {
        let time_bt66_vec: Vec<[f64; 2]> = self
            .tchx_plot_data
            .iter()
            .map(|tuple| {
                let (time, _tchx_htc, _bt65, bt66, _bt66_setpt) = *tuple;

                if bt66.get::<kelvin>() > 0.0 {
                    [time.get::<second>(), bt66.get::<degree_celsius>()]
                } else {
                    // don't return anything, a default 20.0 will do
                    // this is the initial condition
                    [0.0, 20.0]
                }
            })
            .collect();

        return time_bt66_vec;
    }
    /// gets bt 66 set point data over time
    /// time in second, temp in degc
    pub fn get_bt_66_setpt_degc_vs_time_secs_vec(&self) -> Vec<[f64; 2]> {
        let time_bt66_vec: Vec<[f64; 2]> = self
            .tchx_plot_data
            .iter()
            .map(|tuple| {
                let (time, _tchx_htc, _bt65, bt66, bt66_setpt) = *tuple;

                if bt66.get::<kelvin>() > 0.0 {
                    [time.get::<second>(), bt66_setpt.get::<degree_celsius>()]
                } else {
                    // don't return anything, a default 20.0 will do
                    // this is the initial condition
                    [0.0, 20.0]
                }
            })
            .collect();

        return time_bt66_vec;
    }

    /// get tchx htc data vs time
    pub fn get_tchx_htc_watts_per_m2_kelvin_vs_time_secs_vec(&self) -> Vec<[f64; 2]> {
        let time_tchx_htc_vec: Vec<[f64; 2]> = self
            .tchx_plot_data
            .iter()
            .map(|tuple| {
                let (time, tchx_htc, _bt65, bt66, _bt66_setpt) = *tuple;

                if bt66.get::<kelvin>() > 0.0 {
                    [
                        time.get::<second>(),
                        tchx_htc.get::<watt_per_square_meter_kelvin>(),
                    ]
                } else {
                    // don't return anything, a default 20.0 will do
                    // this is the initial condition
                    [0.0, 20.0]
                }
            })
            .collect();

        return time_tchx_htc_vec;
    }

    /// gets bt 43 data over time
    /// time in second, temp in degc
    pub fn get_bt_43_degc_vs_time_secs_vec(&self) -> Vec<[f64; 2]> {
        let time_bt43_vec: Vec<[f64; 2]> = self
            .ctah_plot_data
            .iter()
            .map(|tuple| {
                let (time, _ctah_htc, bt43, _bt41, _bt41_setpt) = *tuple;

                if bt43.get::<kelvin>() > 0.0 {
                    [time.get::<second>(), bt43.get::<degree_celsius>()]
                } else {
                    // don't return anything, a default 20.0 will do
                    // this is the initial condition
                    [0.0, 20.0]
                }
            })
            .collect();

        return time_bt43_vec;
    }
    /// gets bt 41 data over time
    /// time in second, temp in degc
    pub fn get_bt_41_degc_vs_time_secs_vec(&self) -> Vec<[f64; 2]> {
        let time_bt41_vec: Vec<[f64; 2]> = self
            .ctah_plot_data
            .iter()
            .map(|tuple| {
                let (time, _ctah_htc, _bt43, bt41, _bt41_setpt) = *tuple;

                if bt41.get::<kelvin>() > 0.0 {
                    [time.get::<second>(), bt41.get::<degree_celsius>()]
                } else {
                    // don't return anything, a default 20.0 will do
                    // this is the initial condition
                    [0.0, 20.0]
                }
            })
            .collect();

        return time_bt41_vec;
    }
    /// gets bt 41 set point data over time
    /// time in second, temp in degc
    pub fn get_bt_41_setpt_degc_vs_time_secs_vec(&self) -> Vec<[f64; 2]> {
        let time_bt41_vec: Vec<[f64; 2]> = self
            .ctah_plot_data
            .iter()
            .map(|tuple| {
                let (time, _ctah_htc, _bt43, bt41, bt41_setpt) = *tuple;

                if bt41.get::<kelvin>() > 0.0 {
                    [time.get::<second>(), bt41_setpt.get::<degree_celsius>()]
                } else {
                    // don't return anything, a default 20.0 will do
                    // this is the initial condition
                    [0.0, 20.0]
                }
            })
            .collect();

        return time_bt41_vec;
    }

    /// get ctah htc data vs time
    pub fn get_ctah_htc_watts_per_m2_kelvin_vs_time_secs_vec(&self) -> Vec<[f64; 2]> {
        let time_ctah_htc_vec: Vec<[f64; 2]> = self
            .ctah_plot_data
            .iter()
            .map(|tuple| {
                let (time, ctah_htc, _bt43, bt41, _bt41_setpt) = *tuple;

                if bt41.get::<kelvin>() > 0.0 {
                    [
                        time.get::<second>(),
                        ctah_htc.get::<watt_per_square_meter_kelvin>(),
                    ]
                } else {
                    // don't return anything, a default 20.0 will do
                    // this is the initial condition
                    [0.0, 20.0]
                }
            })
            .collect();

        return time_ctah_htc_vec;
    }

    /// gets bt 11 data over time
    /// time in second, temp in degc
    pub fn get_bt_11_degc_vs_time_secs_vec(&self) -> Vec<[f64; 2]> {
        let time_bt11_vec: Vec<[f64; 2]> = self
            .heater_plot_data
            .iter()
            .map(|tuple| {
                let (time, _power, bt11, _bt12) = *tuple;

                if bt11.get::<kelvin>() > 0.0 {
                    [time.get::<second>(), bt11.get::<degree_celsius>()]
                } else {
                    // don't return anything, a default 20.0 will do
                    // this is the initial condition
                    [0.0, 20.0]
                }
            })
            .collect();

        return time_bt11_vec;
    }

    /// time in second, temp in degc
    pub fn get_bt_12_degc_vs_time_secs_vec(&self) -> Vec<[f64; 2]> {
        let time_bt12_vec: Vec<[f64; 2]> = self
            .heater_plot_data
            .iter()
            .map(|tuple| {
                let (time, _power, _bt11, bt12) = *tuple;

                if bt12.get::<kelvin>() > 0.0 {
                    [time.get::<second>(), bt12.get::<degree_celsius>()]
                } else {
                    // don't return anything, a 20.0 will do
                    // this is the initial condition
                    [0.0, 20.0]
                }
            })
            .collect();

        return time_bt12_vec;
    }

    /// heater power in kw, time in seconds
    pub fn get_heater_power_kw_vs_time_secs_vec(&self) -> Vec<[f64; 2]> {
        let time_heater_power_vec: Vec<[f64; 2]> = self
            .heater_plot_data
            .iter()
            .map(|tuple| {
                let (time, power, bt11, _bt12) = *tuple;

                if bt11.get::<kelvin>() > 0.0 {
                    [time.get::<second>(), power.get::<kilowatt>()]
                } else {
                    // don't return anything, a default 0.0 will do
                    // this is the initial condition
                    [0.0, 0.0]
                }
            })
            .collect();

        return time_heater_power_vec;
    }

    // now for the ctah pump

    pub fn insert_ctah_pump_data(
        &mut self,
        simulation_time: Time,
        ctah_pump_pressure_or_loop_pressure_drop: Pressure,
        ctah_branch_mass_flowrate: MassRate,
        ctah_pump_temperature: ThermodynamicTemperature,
    ) {
        let data_tuple = (
            simulation_time,
            ctah_pump_pressure_or_loop_pressure_drop,
            ctah_branch_mass_flowrate,
            ctah_pump_temperature,
        );

        // now insert this into the heater
        // how?
        // map the vectors out first
        let mut current_ctah_pump_vec: Vec<(Time, Pressure, MassRate, ThermodynamicTemperature)>;

        current_ctah_pump_vec = self
            .ctah_pump_plot_data
            .iter()
            .map(|&values| values)
            .collect();

        // now, insert the latest data at the top
        current_ctah_pump_vec.insert(0, data_tuple);

        // take the first NUM_DATA_PTS_IN_PLOTS pieces as a fixed size array
        // which is basically the array size

        let mut new_array_to_be_put_back: Vec<(
            Time,
            Pressure,
            MassRate,
            ThermodynamicTemperature,
        )> = vec![
            (
                Time::ZERO,
                Pressure::ZERO,
                MassRate::ZERO,
                ThermodynamicTemperature::ZERO
            );
            NUM_DATA_PTS_IN_PLOTS
        ];

        // map the first NUM_DATA_PTS_IN_PLOTS values of the current heater data vec

        for n in 0..NUM_DATA_PTS_IN_PLOTS {
            new_array_to_be_put_back[n] = current_ctah_pump_vec[n];
        }

        self.ctah_pump_plot_data = new_array_to_be_put_back;
    }

    // ctah pump

    /// get ctah pump pressure data vs time
    pub fn get_ctah_pump_pressure_pascals_vs_time_secs_vec(&self) -> Vec<[f64; 2]> {
        let time_ctah_pump_pressure_vec: Vec<[f64; 2]> = self
            .ctah_pump_plot_data
            .iter()
            .map(|tuple| {
                let (time, ctah_pump_pressure, _ctah_br_flowrate, ctah_pump_temp) = *tuple;

                if ctah_pump_temp.get::<kelvin>() > 0.0 {
                    [time.get::<second>(), ctah_pump_pressure.get::<pascal>()]
                } else {
                    // don't return anything, a default 0.0 will do
                    // this is the initial condition
                    [0.0, 0.0]
                }
            })
            .collect();

        return time_ctah_pump_pressure_vec;
    }

    pub fn get_ctah_br_mass_kg_per_s_vs_time_secs_vec(&self) -> Vec<[f64; 2]> {
        let time_ctah_pump_massrate_vec: Vec<[f64; 2]> = self
            .ctah_pump_plot_data
            .iter()
            .map(|tuple| {
                let (time, _ctah_pump_pressure, ctah_br_flowrate, ctah_pump_temp) = *tuple;

                if ctah_pump_temp.get::<kelvin>() > 0.0 {
                    [
                        time.get::<second>(),
                        ctah_br_flowrate.get::<kilogram_per_second>(),
                    ]
                } else {
                    // don't return anything, a default 0.0 will do
                    // this is the initial condition
                    [0.0, 0.0]
                }
            })
            .collect();

        return time_ctah_pump_massrate_vec;
    }

    pub fn get_ctah_pump_temp_degc_vs_time_secs_vec(&self) -> Vec<[f64; 2]> {
        let time_ctah_pump_massrate_vec: Vec<[f64; 2]> = self
            .ctah_pump_plot_data
            .iter()
            .map(|tuple| {
                let (time, _ctah_pump_pressure, _ctah_br_flowrate, ctah_pump_temp) = *tuple;

                if ctah_pump_temp.get::<kelvin>() > 0.0 {
                    [time.get::<second>(), ctah_pump_temp.get::<degree_celsius>()]
                } else {
                    // don't return anything, a default 20.0 will do
                    // this is the initial condition
                    [0.0, 20.0]
                }
            })
            .collect();

        return time_ctah_pump_massrate_vec;
    }

    pub fn insert_dhx_data(
        &mut self,
        simulation_time: Time,
        shell_side_mass_rate_dhx_br: MassRate,
        tube_side_mass_rate_dracs_loop: MassRate,
        dhx_shell_side_inlet_temp: ThermodynamicTemperature,
        dhx_shell_side_outlet_temp: ThermodynamicTemperature,
        dhx_tube_side_inlet_temp: ThermodynamicTemperature,
        dhx_tube_side_outlet_temp: ThermodynamicTemperature,
    ) {
        let data_tuple = (
            simulation_time,
            shell_side_mass_rate_dhx_br,
            tube_side_mass_rate_dracs_loop,
            dhx_shell_side_inlet_temp,
            dhx_shell_side_outlet_temp,
            dhx_tube_side_inlet_temp,
            dhx_tube_side_outlet_temp,
        );

        // now insert this into the heater
        // how?
        // map the vectors out first
        let mut current_tchx_data_vec: Vec<(
            Time,
            MassRate,
            MassRate,
            ThermodynamicTemperature,
            ThermodynamicTemperature,
            ThermodynamicTemperature,
            ThermodynamicTemperature,
        )>;

        current_tchx_data_vec = self.dhx_plot_data.iter().map(|&values| values).collect();

        // now, insert the latest data at the top
        current_tchx_data_vec.insert(0, data_tuple);

        // take the first NUM_DATA_PTS_IN_PLOTS pieces as a fixed size array
        // which is basically the array size

        let mut new_array_to_be_put_back: Vec<(
            Time,
            MassRate,
            MassRate,
            ThermodynamicTemperature,
            ThermodynamicTemperature,
            ThermodynamicTemperature,
            ThermodynamicTemperature,
        )> = vec![
            (
                Time::ZERO,
                MassRate::ZERO,
                MassRate::ZERO,
                ThermodynamicTemperature::ZERO,
                ThermodynamicTemperature::ZERO,
                ThermodynamicTemperature::ZERO,
                ThermodynamicTemperature::ZERO
            );
            NUM_DATA_PTS_IN_PLOTS
        ];

        // map the first NUM_DATA_PTS_IN_PLOTS values of the current heater data vec

        for n in 0..NUM_DATA_PTS_IN_PLOTS {
            new_array_to_be_put_back[n] = current_tchx_data_vec[n];
        }

        self.dhx_plot_data = new_array_to_be_put_back;
    }

    pub fn get_dhx_tube_inlet_temp_degc_vs_time_secs_vec(&self) -> Vec<[f64; 2]> {
        let time_dhx_tube_inlet_vec: Vec<[f64; 2]> = self
            .dhx_plot_data
            .iter()
            .map(|tuple| {
                let (
                    time,
                    _dhx_shell_dhx_br_mass_flowrate,
                    _dhx_tube_dracs_loop_mass_flowrate,
                    dhx_shell_inlet_temp,
                    _dhx_shell_outlet_temp,
                    dhx_tube_inlet_temp,
                    _dhx_tube_outlet_temp,
                ) = *tuple;

                if dhx_shell_inlet_temp.get::<kelvin>() > 0.0 {
                    [
                        time.get::<second>(),
                        dhx_tube_inlet_temp.get::<degree_celsius>(),
                    ]
                } else {
                    // don't return anything, a default 20.0 will do
                    // this is the initial condition
                    [0.0, 20.0]
                }
            })
            .collect();

        return time_dhx_tube_inlet_vec;
    }

    pub fn get_dhx_tube_outlet_temp_degc_vs_time_secs_vec(&self) -> Vec<[f64; 2]> {
        let time_dhx_tube_outlet_vec: Vec<[f64; 2]> = self
            .dhx_plot_data
            .iter()
            .map(|tuple| {
                let (
                    time,
                    _dhx_shell_dhx_br_mass_flowrate,
                    _dhx_tube_dracs_loop_mass_flowrate,
                    dhx_shell_inlet_temp,
                    _dhx_shell_outlet_temp,
                    _dhx_tube_inlet_temp,
                    dhx_tube_outlet_temp,
                ) = *tuple;

                if dhx_shell_inlet_temp.get::<kelvin>() > 0.0 {
                    [
                        time.get::<second>(),
                        dhx_tube_outlet_temp.get::<degree_celsius>(),
                    ]
                } else {
                    // don't return anything, a default 20.0 will do
                    // this is the initial condition
                    [0.0, 20.0]
                }
            })
            .collect();

        return time_dhx_tube_outlet_vec;
    }
    pub fn get_dhx_shell_inlet_temp_degc_vs_time_secs_vec(&self) -> Vec<[f64; 2]> {
        let time_dhx_shell_inlet_vec: Vec<[f64; 2]> = self
            .dhx_plot_data
            .iter()
            .map(|tuple| {
                let (
                    time,
                    _dhx_shell_dhx_br_mass_flowrate,
                    _dhx_tube_dracs_loop_mass_flowrate,
                    dhx_shell_inlet_temp,
                    _dhx_shell_outlet_temp,
                    _dhx_tube_inlet_temp,
                    _dhx_tube_outlet_temp,
                ) = *tuple;

                if dhx_shell_inlet_temp.get::<kelvin>() > 0.0 {
                    [
                        time.get::<second>(),
                        dhx_shell_inlet_temp.get::<degree_celsius>(),
                    ]
                } else {
                    // don't return anything, a default 20.0 will do
                    // this is the initial condition
                    [0.0, 20.0]
                }
            })
            .collect();

        return time_dhx_shell_inlet_vec;
    }
    pub fn get_dhx_shell_outlet_temp_degc_vs_time_secs_vec(&self) -> Vec<[f64; 2]> {
        let time_dhx_shell_outlet_vec: Vec<[f64; 2]> = self
            .dhx_plot_data
            .iter()
            .map(|tuple| {
                let (
                    time,
                    _dhx_shell_dhx_br_mass_flowrate,
                    _dhx_tube_dracs_loop_mass_flowrate,
                    dhx_shell_inlet_temp,
                    dhx_shell_outlet_temp,
                    _dhx_tube_inlet_temp,
                    _dhx_tube_outlet_temp,
                ) = *tuple;

                if dhx_shell_inlet_temp.get::<kelvin>() > 0.0 {
                    [
                        time.get::<second>(),
                        dhx_shell_outlet_temp.get::<degree_celsius>(),
                    ]
                } else {
                    // don't return anything, a default 20.0 will do
                    // this is the initial condition
                    [0.0, 20.0]
                }
            })
            .collect();

        return time_dhx_shell_outlet_vec;
    }
    pub fn get_dhx_shell_mass_rate_kg_per_s_vs_time_secs_vec(&self) -> Vec<[f64; 2]> {
        let time_dhx_br_massrate_vec: Vec<[f64; 2]> = self
            .dhx_plot_data
            .iter()
            .map(|tuple| {
                let (
                    time,
                    dhx_shell_dhx_br_mass_flowrate,
                    _dhx_tube_dracs_loop_mass_flowrate,
                    dhx_shell_inlet_temp,
                    _dhx_shell_outlet_temp,
                    _dhx_tube_inlet_temp,
                    _dhx_tube_outlet_temp,
                ) = *tuple;

                if dhx_shell_inlet_temp.get::<kelvin>() > 0.0 {
                    [
                        time.get::<second>(),
                        dhx_shell_dhx_br_mass_flowrate.get::<kilogram_per_second>(),
                    ]
                } else {
                    // don't return anything, a default 0.0 will do
                    // this is the initial condition
                    [0.0, 0.0]
                }
            })
            .collect();

        return time_dhx_br_massrate_vec;
    }
    pub fn get_dhx_tube_mass_rate_kg_per_s_vs_time_secs_vec(&self) -> Vec<[f64; 2]> {
        let time_dracs_loop_massrate_vec: Vec<[f64; 2]> = self
            .dhx_plot_data
            .iter()
            .map(|tuple| {
                let (
                    time,
                    _dhx_shell_dhx_br_mass_flowrate,
                    dhx_tube_dracs_loop_mass_flowrate,
                    dhx_shell_inlet_temp,
                    _dhx_shell_outlet_temp,
                    _dhx_tube_inlet_temp,
                    _dhx_tube_outlet_temp,
                ) = *tuple;

                if dhx_shell_inlet_temp.get::<kelvin>() > 0.0 {
                    [
                        time.get::<second>(),
                        dhx_tube_dracs_loop_mass_flowrate.get::<kilogram_per_second>(),
                    ]
                } else {
                    // don't return anything, a default 0.0 will do
                    // this is the initial condition
                    [0.0, 0.0]
                }
            })
            .collect();

        return time_dracs_loop_massrate_vec;
    }
}

impl Default for PagePlotData {
    fn default() -> Self {
        // basically a whole array of dimensioned zeroes
        let heater_data_default = vec![
            (
                Time::ZERO,
                Power::ZERO,
                ThermodynamicTemperature::ZERO,
                ThermodynamicTemperature::ZERO
            );
            NUM_DATA_PTS_IN_PLOTS
        ];

        let ctah_data_default = vec![
            (
                Time::ZERO,
                HeatTransfer::ZERO,
                ThermodynamicTemperature::ZERO,
                ThermodynamicTemperature::ZERO,
                ThermodynamicTemperature::ZERO
            );
            NUM_DATA_PTS_IN_PLOTS
        ];

        // tchx data default

        let tchx_data_default = vec![
            (
                Time::ZERO,
                HeatTransfer::ZERO,
                ThermodynamicTemperature::ZERO,
                ThermodynamicTemperature::ZERO,
                ThermodynamicTemperature::ZERO
            );
            NUM_DATA_PTS_IN_PLOTS
        ];

        // ctah pump data default

        let ctah_pump_data_default = vec![
            (
                Time::ZERO,
                Pressure::ZERO,
                MassRate::ZERO,
                ThermodynamicTemperature::ZERO
            );
            NUM_DATA_PTS_IN_PLOTS
        ];
        // dhx data default
        //
        // time,
        // shell mass flowrate ,
        // tube mass flowrate,
        // dhx shell inlet temp
        // dhx shell outlet temp
        // dhx tube inlet temp
        // dhx tube outlet temp
        let dhx_data_default = vec![
            (
                Time::ZERO,
                MassRate::ZERO,
                MassRate::ZERO,
                ThermodynamicTemperature::ZERO,
                ThermodynamicTemperature::ZERO,
                ThermodynamicTemperature::ZERO,
                ThermodynamicTemperature::ZERO,
            );
            NUM_DATA_PTS_IN_PLOTS
        ];

        // by default, record every 0.1s
        let graph_data_record_interval_seconds = 0.1;
        let csv_display_interval_seconds = 0.1;

        Self {
            // first, a blank dataset
            heater_plot_data: heater_data_default,
            ctah_plot_data: ctah_data_default,
            tchx_plot_data: tchx_data_default,
            ctah_pump_plot_data: ctah_pump_data_default,
            dhx_plot_data: dhx_data_default,
            graph_data_record_interval_seconds,
            csv_display_interval_seconds,
        }
    }
}
