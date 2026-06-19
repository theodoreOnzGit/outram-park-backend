use crate::alpha_nightly::transfer_fn_wrapper_and_enums::{TransferFnFirstOrder, TransferFnTraits};
use csv::Writer;
use uom::si::f64::*;
use uom::ConstZero;
use uom::si::frequency::hertz;
use uom::si::ratio::ratio;
use uom::si::time::second;
use crate::alpha_nightly::errors::ChemEngProcessControlSimulatorError;

use super::AnalogController;

/// Integral controller with transfer function
///
/// G(s) = K_c / (tau_I s) exp(-cs)
///
/// the controller has two main parts, 
/// firstly, a delay function 
///
/// and the integral ramp response function
#[derive(Debug,PartialEq, PartialOrd, Clone)]
pub struct IntegralController{
    pub(crate) ramp_function: RampResponseRealTime,
    pub(crate) delay_function: TransferFnFirstOrder,
}

impl Default for IntegralController {

    /// returns 1/s
    fn default() -> Self {
        Self::new(Ratio::new::<ratio>(1.0), 
            Time::new::<second>(1.0)).unwrap()
    }
}

impl TransferFnTraits for IntegralController {
    fn set_dead_time(&mut self, dead_time: Time) {
        self.delay_function.set_dead_time(dead_time)
    }

    fn set_user_input_and_calc(&mut self, 
        user_input: Ratio,
        time_of_input: Time) -> Result<Ratio, 
    ChemEngProcessControlSimulatorError> {
        // feed the input into the delay_function first 

        let delay_fn_output: Ratio = 
        self.delay_function.set_user_input_and_calc(user_input, time_of_input)?;

        // now pipe the delay fn output into the ramp function 
        let integral_controller_output = 
        self.ramp_function.set_user_input_and_calc(delay_fn_output, time_of_input)?;

        return Ok(integral_controller_output);

    }


    fn spawn_writer(&mut self, name: String) -> Result<csv::Writer<std::fs::File>,
    ChemEngProcessControlSimulatorError> {
        let mut title_string: String = name;
        title_string += "_integral_controller.csv";
        let wtr = Writer::from_path(title_string)?;
        Ok(wtr)
    }

    fn csv_write_values(&mut self, 
        wtr: &mut csv::Writer<std::fs::File>,
        time: Time,
        input: Ratio,
        output: Ratio) -> Result<(), 
    ChemEngProcessControlSimulatorError> {
        let current_time_string = time.get::<second>().to_string();
        let input_string = input.get::<ratio>().to_string();
        let output_string = output.get::<ratio>().to_string();

        wtr.write_record(&[current_time_string,
            input_string,
            output_string])?;

        wtr.flush().unwrap();

        Ok(())
    }
}

impl IntegralController {

    /// integral controller in the form: 
    ///
    /// G(s) = Kc / (tau_I s)
    pub fn new(controller_gain: Ratio,
        integral_time: Time) -> Result<Self,ChemEngProcessControlSimulatorError> {

        let ramp_function = RampResponseRealTime::new(
            integral_time, controller_gain)?;

        let a1 = Time::new::<second>(1.0);
        let b1 = Ratio::new::<ratio>(1.0);
        let a2 = Time::new::<second>(1.0);
        let b2 = Ratio::new::<ratio>(1.0);

        let delay_function: TransferFnFirstOrder = 
        TransferFnFirstOrder::new(
            a1, b1, a2, b2)?;

        Ok(Self {
            ramp_function,
            delay_function,
        })
    }

}

/// Ramp response for integral controller 
/// able to take in a time varying input
///
/// Transfer function is G(s) = 1/s
#[derive(Debug,PartialEq, PartialOrd, Clone, Copy)]
pub struct RampResponseRealTime {
    pub(crate) start_time: Time,
    pub(crate) current_time: Time,
    /// previous timestep input
    pub(crate) previous_timestep_input: Ratio,

    /// offset for ramp function calculations
    pub(crate) offset: Ratio,

    /// gradient gain, which one will add and change the offsets
    pub(crate) gradient_gain: Frequency,

    /// integral_time or reset time 
    pub(crate) integral_time: Time,
    
    /// controller_gain 
    pub(crate) controller_gain: Ratio,
    
}

impl Default for RampResponseRealTime {
    /// returns G(s) = 1/s
    fn default() -> Self {
        Self { start_time: Time::ZERO, 
            current_time: Time::ZERO, 
            previous_timestep_input: Ratio::ZERO, 
            offset: Ratio::ZERO,
            gradient_gain: Frequency::ZERO,
            integral_time: Time::new::<second>(1.0),
            controller_gain: Ratio::new::<ratio>(1.0),
        }
    }
}

impl RampResponseRealTime {

    pub fn new(integral_time: Time,
    controller_gain: Ratio) -> Result<Self,ChemEngProcessControlSimulatorError> {
        // we start with unit ramp
        let mut ramp_response = Self::default();
        ramp_response.integral_time = integral_time;
        ramp_response.controller_gain = controller_gain;

        return Ok(ramp_response);
    }

    pub fn set_user_input_and_calc(&mut self, 
        user_input: Ratio,
        time_of_input: Time) -> Result<Ratio, 
    ChemEngProcessControlSimulatorError> {

        // check if input is equal to current input 

        // case where input is not the same to 9 decimal places

        let input_changed: bool = 
        (user_input.get::<ratio>() * 1e9).round() 
        - (self.previous_timestep_input.clone()
            .get::<ratio>()*1e9).round() != 0.0 ;

        // time update 
        self.current_time = time_of_input;

        // if input changed, then we must change the gradient and 
        // the offset
        if input_changed {
            let k_c = self.controller_gain;
            let tau_i = self.integral_time;
            let a_i = user_input - self.previous_timestep_input;

            let gradient_change: Frequency = a_i * k_c/tau_i;
            let offset_change: Ratio = -a_i*time_of_input *k_c/tau_i;

            self.offset += offset_change;
            self.gradient_gain += gradient_change;

            // finally change the input at last timestep
            self.previous_timestep_input = user_input;
        }

        // now calc based on the linear input: 

        let output = self.current_time * self.gradient_gain + self.offset;

        return Ok(output);
    }
}


/// Ramp response for integral controller, 
/// This is because the integral of a step function is 
/// a ramp response
///
/// Allows for a user defined start time where the ramp 
/// response switches on
///
///
/// The response is:
///
/// y(t) = u (t - t_start) * a1 * K * (t - t_start)
///
#[derive(Debug,PartialEq, PartialOrd, Clone, Copy)]
pub struct RampResponse {
    pub(crate) gradient_gain: Frequency,
    pub(crate) start_time: Time,
    pub(crate) user_input: Ratio,
    pub(crate) current_time: Time,
}

impl Default for RampResponse {
    fn default() -> Self {
        RampResponse { 
            gradient_gain: Frequency::new::<hertz>(1.0), 
            start_time: Time::ZERO, 
            user_input: Ratio::new::<ratio>(1.0), 
            current_time: Time::ZERO
        }
    }
}

impl RampResponse {

    /// constructor
    pub fn new(
        gradient_gain: Frequency,
        start_time: Time,
        user_input: Ratio,
        current_time:Time) -> Result<Self, ChemEngProcessControlSimulatorError>{
        
        Ok(Self { 
            gradient_gain, 
            start_time, 
            user_input, 
            current_time 
        })
    }

    /// calculates the current value of the ramp response 
    pub fn calculate_response(&mut self, 
        simulation_time: Time)
        -> Ratio {

            // get the current time (t - t0)
            self.current_time = simulation_time;
            let time_elapsed = self.current_time - self.start_time;

            let heaviside_on: bool = self.current_time >= self.start_time;


            // if the current time is before start time, no response 
            // from this transfer function
            if !heaviside_on {
                return Ratio::ZERO;
            }

            // or else just calculate the response 

            let response: Ratio = self.user_input 
                * self.gradient_gain 
                * time_elapsed;
            return response;

        }

    /// checks if ramp function is past its dead time
    /// for 1s or more
    pub fn is_started_for_1s(&self) -> bool {
            let time_elapsed = self.current_time - self.start_time;

            let turned_on_for_1s: bool = time_elapsed >= 
                Time::new::<second>(1.0);

            

            // if the current time is before start time, no response 
            // from this transfer function
            if !turned_on_for_1s {
                return false;
            }

            return true;
    }

}

impl Into<AnalogController> for IntegralController {
    fn into(self) -> AnalogController {
        AnalogController::IntegralStandalone(self)
    }
}

