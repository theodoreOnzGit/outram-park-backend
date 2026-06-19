use uom::{si::{f64::*, time::second, frequency::hertz, ratio::ratio}, ConstZero};

use crate::beta_testing::errors::ChemEngProcessControlSimulatorError;

/// step responses for transfer function of type 
///
/// G(s) = (a1 s^2 + bs)/ (a2 s^2 + b2 s + c)
///
///
#[derive(Debug,PartialEq, PartialOrd, Clone)]
pub struct DecayingSinusoid {
    pub(crate) magnitude: Ratio,
    /// decay frequency or 1/decay time
    pub(crate) a: Frequency,
    pub(crate) previous_timestep_input: Ratio,
    /// oscillation frequency
    pub(crate) omega: Frequency,
    /// previous timestep output
    pub(crate) offset: Ratio,
    /// delay
    pub(crate) delay: Time,
    /// vector of first order responses 
    pub(crate) response_vec: Vec<DecaySinusoidResponse>,
    /// choose whether it's a sine or cosine,
    pub(crate) sinusoid_type: TransferFnSinusoidType,
}

#[derive(Debug,PartialEq, PartialOrd, Clone, Copy)]
pub enum TransferFnSinusoidType {
    Sine,
    Cosine
}

impl Default for DecayingSinusoid {
    /// default is: 
    ///
    /// 1 / ( (s+1)^2 + 1)
    /// time in seconds, 
    /// frequency in hertz
    fn default() -> Self {
        DecayingSinusoid 
            { magnitude: Ratio::new::<ratio>(1.0), 
            a: Frequency::new::<hertz>(1.0), 
            previous_timestep_input: Ratio::ZERO, 
            offset: Ratio::ZERO, 
            delay: Time::new::<second>(0.0), 
            response_vec: vec![],
            omega: Frequency::new::<hertz>(1.0),
            sinusoid_type: TransferFnSinusoidType::Sine,
        }
    }
}

impl DecayingSinusoid {

    /// constructors 
    pub fn new_sine(magnitude: Ratio,
        decay_frequency: Frequency,
        initial_input: Ratio,
        initial_value: Ratio,
        delay: Time, omega:Frequency) -> Result<Self, ChemEngProcessControlSimulatorError> {

        // if damping factor is less than or equal 
        // 0, should throw an error 
        // or panic (i will use errors maybe later?)

        if decay_frequency.value <= 0.0 {
            return Err(ChemEngProcessControlSimulatorError::
                UnstableDampingFactorForStableTransferFunction);
        }

        if omega.value <= 0.0 {
            return Err(ChemEngProcessControlSimulatorError::
                UnstableDampingFactorForStableTransferFunction);
        }

        Ok(DecayingSinusoid { 
            magnitude, 
            a: decay_frequency, 
            previous_timestep_input: initial_input, 
            offset: initial_value, 
            delay, 
            response_vec: vec![],
            omega,
            sinusoid_type: TransferFnSinusoidType::Sine,
        })

    }


    pub fn new_cosine(magnitude: Ratio,
        decay_frequency: Frequency,
        initial_input: Ratio,
        initial_value: Ratio,
        delay: Time, omega:Frequency) -> Result<Self, ChemEngProcessControlSimulatorError> {

        if decay_frequency.value <= 0.0 {
            return Err(ChemEngProcessControlSimulatorError::
                UnstableDampingFactorForStableTransferFunction);
        }

        if omega.value <= 0.0 {
            return Err(ChemEngProcessControlSimulatorError::
                UnstableDampingFactorForStableTransferFunction);
        }

        Ok(DecayingSinusoid { 
            magnitude, 
            a: decay_frequency, 
            previous_timestep_input: initial_input, 
            offset: initial_value, 
            delay, 
            response_vec: vec![],
            omega,
            sinusoid_type: TransferFnSinusoidType::Cosine,
        })

    }

    /// sets the user input to some value
    pub fn set_user_input_and_calc_output(&mut self, 
        current_time: Time,
        current_input: Ratio) 
    -> Result<Ratio,ChemEngProcessControlSimulatorError> {
        // check if input is equal to current input 

        // case where input is not the same to 9 decimal places

        let input_changed: bool = 
        (current_input.get::<ratio>() * 1e9).round() 
        - (self.previous_timestep_input.clone().get::<ratio>()*1e9).round() 
        != 0.0 ;

        if input_changed {
            // need to add a response to the vector

            let magnitude = self.magnitude;
            let process_time = self.a;
            let user_input = current_input - self.previous_timestep_input;
            // the time where the first order response kicks in
            let start_time = current_time + self.delay;
            let sinusoid_frequency = self.omega;
            let sinusoid_type = self.sinusoid_type;

            // make a new response
            let new_response = DecaySinusoidResponse::new(
                magnitude,
                process_time,
                start_time,
                user_input,
                current_time,
                sinusoid_frequency,
                sinusoid_type
            )?;

            // add response to the vector
            self.response_vec.push(new_response);

            // then we need to change the previous_timestep_input 
            // to the current input value 
            self.previous_timestep_input = current_input;

            // then we are done!

        }

        // clean up the vector first
        self.clear_sinusoid_response_vector();

        // need to calculate using the list of 
        // first order response vectors as per normal
        //
        // So we are summing this up
        // O(t) = summing:: u2(t - t2) * b [1-exp(-a * [t-t2])] 
        // + offset
        // first we add the offset

        let summation_of_responses: Ratio = self.response_vec.
            iter_mut().map(
                |second_order_response|{
                    second_order_response.calculate_response(current_time)}
            ).sum();
        //dbg!(summation_of_responses);
        //dbg!(&self.response_vec);

        let output = self.offset + summation_of_responses;

        return Ok(output);

    }

    /// clears the item if they have reached steady state
    fn clear_sinusoid_response_vector(&mut self){

        let index_of_steady_state_result = self.response_vec.iter().position(
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
                let first_order_response = self.response_vec[index].clone();
                let steady_state_value_of_response = 
                first_order_response.steady_state_value();
                self.offset += steady_state_value_of_response;

                // then i remove the first order response from the 
                // index
                self.response_vec.remove(index);
            },

            // if no vectors reach steady state, exit
            // with no issue
            None => return,
        }

        // now, we have cleared the vector once, if there are other 
        // times we need to clear the vector, then we enter a loop

        let index_of_steady_state_result = self.response_vec.iter().position(
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
            let index_of_steady_state_result = self.response_vec.iter().position(
                |first_order_response| {
                    first_order_response.is_steady_state()
                }
            );

            steady_state_responses_present = match index_of_steady_state_result {
                Some(index) => {
                    // first get the steady state value and add it to the 
                    // offset
                    let first_order_response = self.response_vec[index].clone();
                    let steady_state_value_of_response = 
                    first_order_response.steady_state_value();
                    self.offset += steady_state_value_of_response;

                    // then i remove the first order response from the 
                    // index
                    self.response_vec.remove(index);

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




/// second order response struct, 
/// will help to caluclate
/// step responses for underdamped, crtically damped and 
/// overdamped stable systems
#[derive(Debug,PartialEq, PartialOrd, Clone, Copy)]
pub struct DecaySinusoidResponse {
    magnitude: Ratio,
    a: Frequency,
    start_time: Time,
    user_input: Ratio,
    current_time: Time,
    omega: Frequency,
    sinusoid_type: TransferFnSinusoidType,
}

impl Default for DecaySinusoidResponse {
    /// default is a critically damped system with 
    /// 1 / ( (s+1)^2 + 1)
    /// time in seconds, 
    /// frequency in hertz
    fn default() -> Self {
        DecaySinusoidResponse { 
            magnitude: Ratio::new::<ratio>(1.0), 
            a: Frequency::new::<hertz>(1.0), 
            start_time: Time::new::<second>(0.0), 
            user_input: Ratio::new::<ratio>(1.0), 
            current_time: Time::new::<second>(0.0),
            omega: Frequency::new::<hertz>(1.0),
            sinusoid_type: TransferFnSinusoidType::Sine,
        }
    }
}


impl DecaySinusoidResponse {

    /// constructor 
    pub fn new(
        magnitude: Ratio,
        a: Frequency,
        start_time: Time,
        user_input: Ratio,
        current_time: Time,
        omega: Frequency,
        sinusoid_type: TransferFnSinusoidType) -> Result<Self,ChemEngProcessControlSimulatorError> {

        // if damping factor is less than or equal 
        // 0, should throw an error 
        // or panic (i will use errors maybe later?)

        if a.value <= 0.0 {
            return Err(ChemEngProcessControlSimulatorError::
                UnstableDampingFactorForStableTransferFunction);
        }
        Ok(DecaySinusoidResponse { 
            magnitude, 
            a, 
            start_time, 
            user_input, 
            current_time,
            omega,
            sinusoid_type,
        })
    }

    /// checks if the transfer function has more or less reached 
    /// steady state,
    ///
    /// this is determined by exp(-at) 
    /// if at is 20 or more, then we have reached steady state
    pub fn is_steady_state(&self) -> bool {
        let time_elapsed = self.current_time - self.start_time;

        //  (at) in exp(-at)
        let at: Ratio = time_elapsed *  self.a;

        if at > Ratio::new::<ratio>(20.0){
            return true;
        }

        // 

        return false;
    }


    /// calculates the response of the second order system
    /// at a given time
    pub fn calculate_response(&mut self, simulation_time: Time) -> Ratio {

        // get the current time (t - t0)
        self.current_time = simulation_time;
        let time_elapsed = self.current_time - self.start_time;

        // first let's deal with the heaviside function

        let heaviside_on: bool = self.current_time > self.start_time;

        // if the current time is before start time, no response 
        // from this transfer function
        if !heaviside_on {
            return Ratio::ZERO;
        }



        let response: Ratio;
        let at: Ratio = time_elapsed *  self.a;
        let at: f64 = at.get::<ratio>();
        let omega_t: Ratio = time_elapsed * self.omega;
        let omega_t: f64 = omega_t.get::<ratio>();

        response = match self.sinusoid_type {
            TransferFnSinusoidType::Sine => {
                self.user_input
                * self.magnitude 
                * (-at).exp()
                * (omega_t).sin()
            },
            TransferFnSinusoidType::Cosine => {
                self.user_input
                * self.magnitude 
                * (-at).exp()
                * (omega_t).cos()
            },
        };

        return response;

    }

    /// steady state value 
    /// of a decaying sinusoid is zero
    pub fn steady_state_value(&self) -> Ratio {
        let response: Ratio = Ratio::ZERO;
        response
    }
}


