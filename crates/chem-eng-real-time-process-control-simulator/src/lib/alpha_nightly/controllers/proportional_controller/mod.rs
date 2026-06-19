use crate::alpha_nightly::transfer_fn_wrapper_and_enums::{TransferFnFirstOrder, TransferFnTraits};
use uom::si::f64::*;
use uom::si::ratio::ratio;
use uom::si::time::second;
use crate::alpha_nightly::errors::ChemEngProcessControlSimulatorError;

use super::AnalogController;

/// a filtered derivative controller 
///
/// G(s) = K_c
///
/// The form is identical to that of a first order transfer function 
/// with s = 0 as its only zero
///
/// Therefore, I'll just have this struct house a transfer function
///
///
#[derive(Debug,PartialEq, PartialOrd, Clone)]
pub struct ProportionalController{
    pub transfer_fn: TransferFnFirstOrder,
}

impl ProportionalController {

    /// a filtered derivative controller 
    /// in the form:
    /// G(s) = K_c 
    pub fn new(controller_gain: Ratio) -> Result<Self,ChemEngProcessControlSimulatorError> {

        // G(s) = (a1 s + b1)/(a2 s + b2)
        //
        // a1 = K_c * 1 second (time)
        // b1 = K_c * 1 (ratio)
        // a2 = 1 second (time)
        // b2 = 1 (ratio)
        let a1 = Time::new::<second>(1.0) * controller_gain;
        let b1 = controller_gain;
        let a2 = Time::new::<second>(1.0);
        let b2 = Ratio::new::<ratio>(1.0);
        let transfer_fn = TransferFnFirstOrder::new(a1, b1, a2, b2).unwrap();

        Ok(Self { transfer_fn })
    }
}
impl Default for ProportionalController {
    /// gives: 
    /// G(s) = 1
    ///
    /// Now, I'm using a very lame method to implement this,
    ///
    /// it is essentially a transfer function with: 
    ///
    /// (s + 1)/(s + 1) = 1
    ///
    /// I'm doing this so that I can re-use my transfer function code.
    ///
    /// This is probably overdoing it in terms of computation cost 
    /// but that's not an issue ... yet ...
    ///
    fn default() -> Self {

        // G(s) = (a1 s + b1)/(a2 s + b2)
        //
        // a1 = 1 second 
        // b1 = 1 (ratio)
        // a2 = 1 second 
        // b2 = 1 (ratio)
        let b1 = Ratio::new::<ratio>(1.0);
        let b2 = Ratio::new::<ratio>(1.0);
        let a1 = Time::new::<second>(1.0);
        let a2 = Time::new::<second>(1.0);
        let transfer_fn = TransferFnFirstOrder::new(a1, b1, a2, b2).unwrap();

        return Self { transfer_fn };

    }
}

impl TransferFnTraits for ProportionalController {
    fn set_dead_time(&mut self, dead_time: Time) {
        self.transfer_fn.set_dead_time(dead_time)
    }

    fn set_user_input_and_calc(&mut self, 
        user_input: Ratio,
        time_of_input: Time) -> Result<Ratio, 
    ChemEngProcessControlSimulatorError> {
        self.transfer_fn.set_user_input_and_calc(user_input, time_of_input)
    }

    fn spawn_writer(&mut self, name: String) -> Result<csv::Writer<std::fs::File>,
    ChemEngProcessControlSimulatorError> {

        self.transfer_fn.spawn_writer(name + "_proportional_controller_")
    }

    fn csv_write_values(&mut self, 
        wtr: &mut csv::Writer<std::fs::File>,
        time: Time,
        input: Ratio,
        output: Ratio) -> Result<(), 
    ChemEngProcessControlSimulatorError> {
        self.transfer_fn.csv_write_values(wtr, time, input, output)
    }
}


impl Into<AnalogController> for ProportionalController {
    fn into(self) -> AnalogController {
        AnalogController::P(self)
    }
}
