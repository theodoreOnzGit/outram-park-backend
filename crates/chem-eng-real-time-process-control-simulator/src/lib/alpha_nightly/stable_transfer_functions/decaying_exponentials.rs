use uom::{si::{f64::*, time::second, frequency::hertz, ratio::ratio}, ConstZero};

use crate::alpha_nightly::errors::ChemEngProcessControlSimulatorError;

/// step responses for transfer function of type 
///
/// G(s) = (a1 s^2 + bs)/ (a2 s^2 + b2 s + c)
///
///
#[derive(Debug,PartialEq, PartialOrd, Clone)]
pub struct DecayingSecondOrderExponential {
    pub(crate) magnitude_alpha: Ratio,
    pub(crate) magnitude_beta: Ratio,
    /// decay frequency of first root, 
    pub(crate) alpha: Frequency,
    /// decay frequency of second root (equal 
    pub(crate) beta: Frequency,
    /// to first root if there is only one root, or both equal)
    pub(crate) previous_timestep_input: Ratio,
    /// previous timestep output
    pub(crate) offset: Ratio,
    /// delay
    pub(crate) delay: Time,
    /// vector of first order responses 
    pub(crate) response_vec: Vec<DecaySecondOrderExponentialResponse>,
    /// choose whether it's a critically damped or 
    /// overdamped system
    pub(crate) exponent_type: DecayingExponentialType,
}

#[derive(Debug,PartialEq, PartialOrd, Clone, Copy)]
pub enum DecayingExponentialType {
    // two distinct roots
    Overdamped, 
    // two equal real roots
    CriticallyDamped,

}

impl DecayingSecondOrderExponential {


    /// constructor for new over damped system
    /// with two real roots
    pub fn new_overdamped(
        magnitude_alpha: Ratio,
        magnitude_beta: Ratio,
        alpha: Frequency,
        beta: Frequency,
        initial_input: Ratio,
        initial_value: Ratio,
        delay: Time) -> Result<Self,ChemEngProcessControlSimulatorError> {

        // if damping factor is less than or equal 
        // 0, should throw an error 
        // or panic (i will use errors maybe later?)

        let exponent_type = DecayingExponentialType::Overdamped;

        if alpha.value <= 0.0 {
            return Err(ChemEngProcessControlSimulatorError::
                UnstableDampingFactorForStableTransferFunction);
        }
        Ok(DecayingSecondOrderExponential { 
            magnitude_alpha,
            magnitude_beta, 
            alpha, 
            beta, 
            previous_timestep_input: initial_input, 
            offset: initial_value, 
            delay, 
            response_vec: vec![], 
            exponent_type,
        })
    }
    /// constructor for new critically damped system
    /// with two equal roots
    ///
    /// it will be in the form 
    ///
    /// magnitude_alpha * t * exp (-alpha t) 
    /// + magnitude_beta * exp (- beta t)
    ///
    /// magnitude_alpha is necessarily in a frequency unit
    /// and it will be converted into hertz before storage
    ///
    ///
    pub fn new_critical(
        magnitude_alpha: Frequency,
        magnitude_beta: Ratio,
        lambda: Frequency,
        initial_input: Ratio,
        initial_value: Ratio,
        delay: Time) -> Result<Self,ChemEngProcessControlSimulatorError> {

        // if damping factor is less than or equal 
        // 0, should throw an error 
        // or panic (i will use errors maybe later?)

        let exponent_type = DecayingExponentialType::CriticallyDamped;
        let magnitude_alpha = Ratio::new::<ratio>(
            magnitude_alpha.get::<hertz>()
        );

        // for critically damped systems, there is only one characteristic 
        // damping frequency, which is lambda
        // therefore, the real part of the 
        // two roots, alpha and beta are the same
        let alpha = lambda;
        let beta =  lambda;

        if alpha.value <= 0.0 {
            return Err(ChemEngProcessControlSimulatorError::
                UnstableDampingFactorForStableTransferFunction);
        }
        Ok(DecayingSecondOrderExponential { 
            magnitude_alpha,
            magnitude_beta, 
            alpha, 
            beta, 
            previous_timestep_input: initial_input, 
            offset: initial_value, 
            delay, 
            response_vec: vec![], 
            exponent_type,
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

            let user_input = current_input - self.previous_timestep_input;
            let magnitude_alpha_times_user_input = self.magnitude_alpha
                *user_input;
            let magnitude_beta_times_user_input = self.magnitude_beta
                *user_input;
            // the time where the first order response kicks in
            let start_time = current_time + self.delay;
            let exponent_type = self.exponent_type;
            let alpha = self.alpha;
            let beta = self.beta;

            // make a new response
            let new_response;

            match exponent_type {
                DecayingExponentialType::Overdamped => {
                    new_response = 
                        DecaySecondOrderExponentialResponse::new_overdamped(
                            magnitude_alpha_times_user_input, 
                            magnitude_beta_times_user_input, 
                            alpha, 
                            beta, 
                            start_time, 
                            user_input, 
                            current_time)?
                },
                DecayingExponentialType::CriticallyDamped => {
                    new_response = 
                        DecaySecondOrderExponentialResponse::new_critical(
                            Frequency::new::<hertz>(
                                magnitude_alpha_times_user_input.get::<ratio>()
                            ), 
                            magnitude_beta_times_user_input, 
                            alpha, 
                            beta, 
                            start_time, 
                            user_input, 
                            current_time)?
                },
            }


            // add response to the vector
            self.response_vec.push(new_response);

            // then we need to change the previous_timestep_input 
            // to the current input value 
            self.previous_timestep_input = current_input;

            // then we are done!

        }

        // clean up the vector first
        self.clear_exponent_response_vector();

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
    fn clear_exponent_response_vector(&mut self){

        let index_of_steady_state_result = self.response_vec.iter().position(
            |second_order_response| {
                second_order_response.is_steady_state()
            }
        );

        match index_of_steady_state_result {

            // if I found something at the index, remove it, 
            // repeatedly test it until nothing is left
            Some(index) => {
                // first get the steady state value and add it to the 
                // offset
                let second_order_response = self.response_vec[index].clone();
                let steady_state_value_of_response = 
                second_order_response.steady_state_value();
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
            |second_order_response| {
                second_order_response.is_steady_state()
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
                |second_order_response| {
                    second_order_response.is_steady_state()
                }
            );

            steady_state_responses_present = match index_of_steady_state_result {
                Some(index) => {
                    // first get the steady state value and add it to the 
                    // offset
                    let second_order_response = self.response_vec[index].clone();
                    let steady_state_value_of_response = 
                    second_order_response.steady_state_value();
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
///
///
/// for decaying exponential responses, we have two main cases 
/// the first is where there are two equal real roots. This is 
/// critical damping
///
/// The second is where we have two real unequal roots. This is 
/// overdamping. 
#[derive(Debug,PartialEq, PartialOrd, Clone, Copy)]
pub(crate) struct DecaySecondOrderExponentialResponse {
    magnitude_alpha_times_user_input: Ratio,
    magnitude_beta_times_user_input: Ratio,
    alpha: Frequency,
    beta: Frequency,
    start_time: Time,
    user_input: Ratio,
    current_time: Time,
    exponential_type: DecayingExponentialType,
}

impl Default for DecaySecondOrderExponentialResponse {
    /// default is a critically damped system with 
    /// 1 / ( (s+1)^2 + 1)
    /// time in seconds, 
    /// frequency in hertz
    fn default() -> Self {
        DecaySecondOrderExponentialResponse { 
            magnitude_alpha_times_user_input: Ratio::new::<ratio>(1.0), 
            magnitude_beta_times_user_input: Ratio::new::<ratio>(1.0), 
            alpha: Frequency::new::<hertz>(1.0), 
            beta: Frequency::new::<hertz>(1.0),
            start_time: Time::new::<second>(0.0), 
            user_input: Ratio::new::<ratio>(1.0), 
            current_time: Time::new::<second>(0.0),
            exponential_type: DecayingExponentialType::CriticallyDamped,
        }
    }
}


impl DecaySecondOrderExponentialResponse {


    /// constructor for new over damped system
    /// with two real roots
    pub fn new_overdamped(
        magnitude_alpha_times_user_input: Ratio,
        magnitude_beta_times_user_input: Ratio,
        alpha: Frequency,
        beta: Frequency,
        start_time: Time,
        user_input: Ratio,
        current_time: Time) -> Result<Self,ChemEngProcessControlSimulatorError> {

        // if damping factor is less than or equal 
        // 0, should throw an error 
        // or panic (i will use errors maybe later?)

        let exponential_type = DecayingExponentialType::Overdamped;

        if alpha.value <= 0.0 {
            return Err(ChemEngProcessControlSimulatorError::
                UnstableDampingFactorForStableTransferFunction);
        }
        Ok(DecaySecondOrderExponentialResponse { 
            magnitude_alpha_times_user_input, 
            magnitude_beta_times_user_input, 
            alpha, 
            beta,
            start_time, 
            user_input, 
            current_time,
            exponential_type,
        })
    }
    /// constructor for new over damped system
    /// with two equal roots
    ///
    /// it will be in the form 
    ///
    /// magnitude_alpha * t * exp (-alpha t) 
    /// + magnitude_beta * exp (- beta t)
    ///
    /// magnitude_alpha is necessarily in a frequency unit
    /// and it will be converted into hertz before storage
    ///
    ///
    pub fn new_critical(
        magnitude_alpha_times_user_input: Frequency,
        magnitude_beta_times_user_input: Ratio,
        alpha: Frequency,
        beta: Frequency,
        start_time: Time,
        user_input: Ratio,
        current_time: Time) -> Result<Self,ChemEngProcessControlSimulatorError> {

        // if damping factor is less than or equal 
        // 0, should throw an error 
        // or panic (i will use errors maybe later?)

        let exponential_type = DecayingExponentialType::CriticallyDamped;
        let magnitude_alpha_times_user_input = Ratio::new::<ratio>(
            magnitude_alpha_times_user_input.get::<hertz>()
        );

        if alpha.value <= 0.0 {
            return Err(ChemEngProcessControlSimulatorError::
                UnstableDampingFactorForStableTransferFunction);
        }
        Ok(DecaySecondOrderExponentialResponse { 
            magnitude_alpha_times_user_input, 
            magnitude_beta_times_user_input, 
            alpha, 
            beta,
            start_time, 
            user_input, 
            current_time,
            exponential_type,
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
        let at: Ratio = time_elapsed * self.alpha;
        let bt: Ratio = time_elapsed * self.beta;

        match self.exponential_type {
            DecayingExponentialType::Overdamped => {
                // need both alpha and beta to be more than 20
                if at > Ratio::new::<ratio>(20.0) && 
                bt > Ratio::new::<ratio>(20.0) {
                    return true;
                }
            },
            DecayingExponentialType::CriticallyDamped => {
                // for critically damped systems we can represent it 
                // using x exp (-x) 
                //
                // if x > 23, generally we can ignore things 
                //
                // However, this usually comes in the form: 
                //
                // t exp (-lambda t)
                //
                // so lambda t = 23 doesn't quite cut it all the time
                // we must impose an additional constraint.
                // Consider rewriting x exp (-x)
                // 1/ lambda * (lambda t) exp (-lambda t)
                //
                // my tolerance initially was for 
                // (lambda t) exp (- lambda t) \approx 1e-9
                //
                // Now I must also consider 1/lambda
                //
                // so just take the product, 

                let inverse_lambda: Time = 1.0 / self.alpha;
                let lambda_t = at;

                // i'm not going to go into specifics... but this 
                // will have to do 

                let exponent_ratio: Ratio = 
                    lambda_t * (-lambda_t.get::<ratio>()).exp() 
                    * inverse_lambda.get::<second>();

                let exponent_decayed: bool =  at > Ratio::new::<ratio>(23.0) && 
                    bt > Ratio::new::<ratio>(23.0);                

                if exponent_decayed && exponent_ratio < Ratio::new::<ratio>(1e-10) {
                    return true;
                }



            },
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

        let heaviside_on: bool = self.current_time >= self.start_time;

        // if the current time is before start time, no response 
        // from this transfer function
        if !heaviside_on {
            return Ratio::ZERO;
        }



        let response: Ratio;

        // for convenience, we calculate alpha t and beta t 
        let alpha_t: Ratio = time_elapsed * self.alpha;
        let beta_t: Ratio = time_elapsed * self.beta;

        response = match self.exponential_type {
            DecayingExponentialType::CriticallyDamped => {
                // for two equal roots, also quite straightforward
                // magnitude_alpha * t * exp(-alpha t)
                // magnitude_beta * exp(-beta t)

                //
                let t_exponential_response = self.magnitude_alpha_times_user_input * 
                    time_elapsed.get::<second>() *
                    (-alpha_t.get::<ratio>()).exp();

                let exponential_response = self.magnitude_beta_times_user_input * 
                    (-beta_t.get::<ratio>()).exp();

                t_exponential_response + exponential_response



            },
            DecayingExponentialType::Overdamped => {
                // for two unequal roots, also quite straightforward
                // magnitude_alpha * exp(-alpha t)
                // magnitude_beta * exp(-beta t)
                // user input part checks out
                
                self.magnitude_alpha_times_user_input * 
                    (-alpha_t.get::<ratio>()).exp() +
                self.magnitude_beta_times_user_input * 
                    (-beta_t.get::<ratio>()).exp()
            },

        };

        return response;

    }

    /// steady state value 
    /// of a decaying exponential is zero
    /// eventually
    pub fn steady_state_value(&self) -> Ratio {
        let response: Ratio = Ratio::ZERO;
        response
    }
}



