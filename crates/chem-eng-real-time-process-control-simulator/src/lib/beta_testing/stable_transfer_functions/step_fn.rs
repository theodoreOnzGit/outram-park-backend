
use uom::{si::{f64::*, time::second, ratio::ratio}, ConstZero};

use crate::beta_testing::errors::ChemEngProcessControlSimulatorError;
/// Step Function struct, 
/// will help to caluclate
/// u1(t - t1) * Kp * a_0
/// where Kp is process gain,
/// a_0 is the user input
/// u(t-t1) is the heaviside function
///
#[derive(Debug,PartialEq, PartialOrd, Clone, Copy)]
pub struct StepFunction {
    process_gain: Ratio,
    start_time: Time,
    user_input: Ratio,
    current_time: Time,
}

impl Default for StepFunction {
    fn default() -> Self {
        StepFunction { 
            process_gain: Ratio::new::<ratio>(1.0), 
            start_time: Time::new::<second>(0.0), 
            user_input: Ratio::new::<ratio>(1.0), 
            current_time: Time::new::<second>(0.0),
        }
    }
}


impl StepFunction {

    /// constructor 
    pub fn new(
        process_gain: Ratio,
        start_time: Time,
        user_input: Ratio,
        current_time: Time,) -> Result<Self, ChemEngProcessControlSimulatorError> {
        Ok(StepFunction { 
            process_gain, 
            start_time, 
            user_input, 
            current_time,
        })
    }

    /// checks if the step function has reached
    /// steady state,
    pub fn is_steady_state(&self) -> bool {
        let time_elapsed = self.current_time - self.start_time;


        if time_elapsed.value >= 0.0 {
            return true;
        }

        return false;
    }


    /// calculates the response of the first order system
    /// at a given time
    /// u1(t - t1) * Kp * [1-exp(- [t-t1] / tau])
    pub fn calculate_response(&mut self, simulation_time: Time) -> Ratio {

        // get the current time (t - t0)
        self.current_time = simulation_time;
        let time_elapsed = self.current_time - self.start_time;

        // first let's deal with the heaviside function

        let heaviside_on: bool = time_elapsed.value >= 0.0;

        // if the current time is before start time, no response 
        // from this transfer function
        if !heaviside_on {
            return Ratio::ZERO;
        }


        // otherwise, calculate as per normal

        // u1(t - t1) * Kp * [1-exp(- [t-t1] / tau])
        let response: Ratio = self.steady_state_value();

        return response;
    }

    /// steady state value 
    /// u1(t - t1) * Kp 
    pub fn steady_state_value(&self) -> Ratio {
        let response: Ratio = self.user_input * self.process_gain;
        response
    }
}

