#[derive(Debug,Clone,Copy)]
pub struct CIETState {
    pub heater_power_kilowatts: f64,
    pub ctah_pump_massrate_set_point: f64,
}

impl Default for CIETState {
    fn default() -> Self {
        CIETState {
            heater_power_kilowatts: 0.0,
            ctah_pump_massrate_set_point: 0.0,
        }
    }
}
