/// this struct holds all the data required for CIET 
/// for the ui to display it
///  
///
/// This is much easier compared to having arc mutex locks for each 
/// piece of data. 
#[derive(Debug,Clone,Copy)]
pub struct CIETState {
    pub heater_power_kilowatts: f64,
    pub ctah_pump_massrate_set_point: f64,
    pub bt_11_heater_inlet_deg_c: f64,
    pub bt_12_heater_outlet_deg_c: f64,
    pub bt_43_ctah_inlet_deg_c: f64,
    pub bt_41_ctah_outlet_deg_c: f64,
    pub bt_41_ctah_outlet_set_pt_deg_c: f64,
    pub bt_21_dhx_shell_inlet_deg_c: f64,
    pub bt_25_dhx_shell_outlet_deg_c: f64,
    pub bt_60_dhx_tube_inlet_deg_c: f64,
    pub bt_21_dhx_tube_outlet_deg_c: f64,
    pub bt_65_tchx_inlet_deg_c: f64,
    pub bt_66_tchx_outlet_deg_c: f64,
    pub fm_60_dracs_kg_per_s: f64,
    pub fm_20_dhx_branch_kg_per_s: f64,
    pub fm_40_ctah_branch_kg_per_s: f64,
}

impl Default for CIETState {
    fn default() -> Self {
        CIETState {
            heater_power_kilowatts: 0.0,
            ctah_pump_massrate_set_point: 0.0,
            bt_11_heater_inlet_deg_c: 21.0,
            bt_12_heater_outlet_deg_c: 21.0,
            bt_43_ctah_inlet_deg_c: 21.0,
            bt_41_ctah_outlet_deg_c: 21.0,
            bt_41_ctah_outlet_set_pt_deg_c: 21.0,
            bt_21_dhx_shell_inlet_deg_c: 21.0,
            bt_25_dhx_shell_outlet_deg_c: 21.0,
            bt_60_dhx_tube_inlet_deg_c: 21.0,
            bt_21_dhx_tube_outlet_deg_c: 21.0,
            bt_65_tchx_inlet_deg_c: 21.0,
            bt_66_tchx_outlet_deg_c: 21.0,
            fm_60_dracs_kg_per_s: 0.0,
            fm_20_dhx_branch_kg_per_s: 0.0,
            fm_40_ctah_branch_kg_per_s: 0.0,
        }
    }
}

impl CIETState {
}

