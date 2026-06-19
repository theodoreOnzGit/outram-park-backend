use uom::si::ratio::ratio;
use uom::si::time::second;
use uom::si::f64::*;

use crate::alpha_nightly::errors::ChemEngProcessControlSimulatorError;
use super::first_order_transfer_fn::FirstOrderResponse;
use crate::alpha_nightly::stable_transfer_functions::step_fn::StepFunction;

/// Transfer function in the form:
///
/// G(s) = K_p [1 - 1/(tau_p s + 1)]
///
/// This comes from:
/// G(s) =  K_p s / (tau_p s + 1)
///
#[derive(Debug,PartialEq, PartialOrd, Clone)]
pub struct FirstOrderStableTransferFnForZeroes {
    pub(crate) process_gain: Ratio,
    pub(crate) process_time: Time,
    pub(crate) previous_timestep_input: Ratio,
    /// previous timestep output
    pub(crate) offset: Ratio,
    /// delay
    pub(crate) delay: Time,

    /// vector of first order responses 
    pub(crate) first_order_response_vec: Vec<FirstOrderResponse>,
    /// vector of step functions
    pub(crate) step_fn_response_vec: Vec<StepFunction>,
}

impl Default for FirstOrderStableTransferFnForZeroes {
    /// default is: 
    ///
    /// s / (s + 1)
    ///
    /// with initial user input of 0.0 
    /// and initial user value of 0.0
    fn default() -> Self {
        FirstOrderStableTransferFnForZeroes { 
            process_gain: Ratio::new::<ratio>(1.0), 
            process_time: Time::new::<second>(1.0), 
            previous_timestep_input: Ratio::new::<ratio>(0.0), 
            offset: Ratio::new::<ratio>(0.0), 
            delay: Time::new::<second>(0.0), 
            first_order_response_vec: vec![],
            step_fn_response_vec: vec![],
        }
    }
}


impl FirstOrderStableTransferFnForZeroes {

    /// constructors 
    /// G(s) =  K_p s / (tau_p s + 1)
    pub fn new(process_gain: Ratio,
        process_time: Time,
        initial_input: Ratio,
        initial_value: Ratio,
        delay: Time,) -> Result<Self, ChemEngProcessControlSimulatorError> {

        if process_time.get::<second>() <= 0.0 {
            return Err(ChemEngProcessControlSimulatorError::
                UnstableDampingFactorForStableTransferFunction);
        }
        Ok(FirstOrderStableTransferFnForZeroes { 
            process_gain, 
            process_time, 
            previous_timestep_input: initial_input, 
            offset: initial_value, 
            delay, 
            first_order_response_vec: vec![],
            step_fn_response_vec: vec![],
        })
    }


    /// sets the user input to some value
    /// The transfer function is: 
    ///
    /// K_p [1 - 1/(tau_p s + 1)]
    pub fn set_user_input_and_calc_output(&mut self, 
        current_time: Time,
        current_input: Ratio) 
    -> Result<Ratio, ChemEngProcessControlSimulatorError> {
        // check if input is equal to current input 

        // case where input is not the same to 9 decimal places

        let input_changed: bool = 
            (current_input.get::<ratio>() * 1e9).round() 
            - (self.previous_timestep_input.clone()
                .get::<ratio>()*1e9).round() != 0.0 ;

        if input_changed {
            // when input changed, change the offset
            // to the current user input 
            // need to add a response to the vector
            //
            // moreover, one must change the offset because there is 
            // an immediate response, 
            //
            // However, one must also account for time delays
            //
            // Therefore, get a vector of offsets, once the time 
            // delay reaches, then add the offset vector to 
            // the existing offset, and delete the vector

            let process_gain = self.process_gain;
            let process_time = self.process_time;
            let user_input = current_input - self.previous_timestep_input;
            // the time where the first order response kicks in
            let start_time = current_time + self.delay;

            // make a new first order response
            let new_first_order_response = FirstOrderResponse::new(
                -process_gain,
                process_time,
                start_time,
                user_input,
                current_time
            )?;

            // add first order response to the vector
            self.first_order_response_vec.push(new_first_order_response);

            // make a new step fn response 
            //
            //
            let new_step_fn_response = StepFunction::new(
                process_gain,
                start_time,
                user_input,
                current_time
            )?;

            self.step_fn_response_vec.push(new_step_fn_response);

            // then we need to change the previous_timestep_input 
            // to the current input value 
            self.previous_timestep_input = current_input;

            // then we are done!
            
        }

        // clean up the vector first
        self.clear_first_order_response_vector();
        self.clear_step_fn_response_vector();

        // need to calculate using the list of 
        // first order response vectors as per normal
        //
        // So we are summing this up
        // O(t) = summing:: u2(t - t2) * b [1-exp(-a * [t-t2])] 
        // + offset
        // first we add the offset

        let summation_of_first_order_responses: Ratio = self.first_order_response_vec.
            iter_mut().map(
                |first_order_response|{
                    first_order_response.calculate_response(current_time)}
            ).sum();

        let summation_of_step_fn_responses: Ratio = self.step_fn_response_vec.
            iter_mut().map(
                |step_fn_response|{
                    step_fn_response.calculate_response(current_time)}
            ).sum();

        let output = self.offset 
            + summation_of_first_order_responses
            + summation_of_step_fn_responses;

        return Ok(output);

    }

    /// clears the item if they have reached steady state
    fn clear_first_order_response_vector(&mut self){

        let index_of_steady_state_result = self.first_order_response_vec.iter().position(
            |first_order_response| {
                first_order_response.is_steady_state()
            }
        );

        match index_of_steady_state_result {

            // if I found something at the index, remove it, 
            // repeatedly test it until nothing is left
            Some(index) => {
                // first get the steady state value and add it to the 
                // offset
                let first_order_response = self.first_order_response_vec[index].clone();
                let steady_state_value_of_response = 
                    first_order_response.steady_state_value();
                self.offset += steady_state_value_of_response;

                // then i remove the first order response from the 
                // index
                self.first_order_response_vec.remove(index);
            },

            // if no vectors reach steady state, exit
            // with no issue
            None => return,
        }

        // now, we have cleared the vector once, if there are other 
        // times we need to clear the vector, then we enter a loop

        let index_of_steady_state_result = self.first_order_response_vec.iter().position(
            |first_order_response| {
                first_order_response.is_steady_state()
            }
        );
        // check if steady state responses are present
        let mut steady_state_responses_present = 
            match index_of_steady_state_result {
                Some(_) => true,
                None => false,
            };
        
        if !steady_state_responses_present {
            return;
        } 

        // repeatedly clear the vector until no steady state responses 
        // are left
        while steady_state_responses_present {

            // check for index
            let index_of_steady_state_result = self.first_order_response_vec.iter().position(
                |first_order_response| {
                    first_order_response.is_steady_state()
                }
            );

            steady_state_responses_present = match index_of_steady_state_result {
                Some(index) => {
                    // first get the steady state value and add it to the 
                    // offset
                    let first_order_response = self.first_order_response_vec[index].clone();
                    let steady_state_value_of_response = 
                        first_order_response.steady_state_value();
                    self.offset += steady_state_value_of_response;

                    // then i remove the first order response from the 
                    // index
                    self.first_order_response_vec.remove(index);

                    // return true value to while loop
                    true
                },
                // return false value to while loop
                None => false,
            };

        }
        return;

    }
    
    /// clears the item if they have reached steady state
    fn clear_step_fn_response_vector(&mut self){

        let index_of_steady_state_result = self.step_fn_response_vec.iter().position(
            |step_fn_response| {
                step_fn_response.is_steady_state()
            }
        );

        match index_of_steady_state_result {

            // if I found something at the index, remove it, 
            // repeatedly test it until nothing is left
            Some(index) => {
                // first get the steady state value and add it to the 
                // offset
                let step_fn_response = self.step_fn_response_vec[index].clone();
                let steady_state_value_of_response = 
                    step_fn_response.steady_state_value();
                self.offset += steady_state_value_of_response;

                // then i remove the first order response from the 
                // index
                self.step_fn_response_vec.remove(index);
            },

            // if no vectors reach steady state, exit
            // with no issue
            None => return,
        }

        // now, we have cleared the vector once, if there are other 
        // times we need to clear the vector, then we enter a loop

        let index_of_steady_state_result = self.step_fn_response_vec.iter().position(
            |step_fn_response| {
                step_fn_response.is_steady_state()
            }
        );
        // check if steady state responses are present
        let mut steady_state_responses_present = 
            match index_of_steady_state_result {
                Some(_) => true,
                None => false,
            };
        
        if !steady_state_responses_present {
            return;
        } 

        // repeatedly clear the vector until no steady state responses 
        // are left
        while steady_state_responses_present {

            // check for index
            let index_of_steady_state_result = self.step_fn_response_vec.iter().position(
                |step_fn_response| {
                    step_fn_response.is_steady_state()
                }
            );

            steady_state_responses_present = match index_of_steady_state_result {
                Some(index) => {
                    // first get the steady state value and add it to the 
                    // offset
                    let step_fn_response = self.step_fn_response_vec[index].clone();
                    let steady_state_value_of_response = 
                        step_fn_response.steady_state_value();
                    self.offset += steady_state_value_of_response;

                    // then i remove the first order response from the 
                    // index
                    self.step_fn_response_vec.remove(index);

                    // return true value to while loop
                    true
                },
                // return false value to while loop
                None => false,
            };

        }
        return;

    }
}

