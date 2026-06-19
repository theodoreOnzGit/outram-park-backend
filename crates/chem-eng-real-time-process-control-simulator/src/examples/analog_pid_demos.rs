/// Copyright [2023] [Theodore Kay Chen Ong, Professor Per F. Peterson,
/// University of California, Berkeley
/// Thermal Hydraulics Lab, Repository Contributors and 
/// Singapore Nuclear Research and Safety Initiative (SNRSI)]
/// 
/// Licensed under the Apache License, Version 2.0 (the "License");
/// you may not use this file except in compliance with the License.
/// You may obtain a copy of the License at
///
///     http://www.apache.org/licenses/LICENSE-2.0
///
/// Unless required by applicable law or agreed to in writing, software
/// distributed under the License is distributed on an "AS IS" BASIS,
/// WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
/// See the License for the specific language governing permissions and
/// limitations under the License.
use std::sync::{Arc, Mutex};
use std::thread;

use chem_eng_real_time_process_control_simulator::alpha_nightly::prelude::*;
use uom::ConstZero;
use uom::si::f64::*;
use uom::si::frequency::hertz;
use uom::si::ratio::ratio;
use uom::si::time::{second, millisecond};
/// 
/// This is a simulation of a feedback PI controller 
/// The process gain is:
///
///         2.5s^2 - 0.5 s + 1
/// G(s) = ---------------------------
///         3 s^2 + 4 s + 4
///
/// we use a feedback controller with 
/// K_c = 0.5 
/// K_c/tau_i = 0.3 Hertz
/// tau_d = 1.0 s 
/// alpha = 1.0
///
///
/// approximately validated... can use some improvement still
///
/// The derivative function is: 
///
/// G(s) = 0.5s / (1 + s)
/// G(s) = 0.5  * s / (1 + s)
///
/// So derivative gain is 0.5 (same as Kc)
/// derivative time is 1 second 
/// alpha is 1
///
/// usually we use: 
///
/// G(s) = (tau_d s)/(alpha* tau_d s + 1)
///
///
pub(crate) fn fine_timesteps_proportional_integral_derivative_test(){


    // controller settings
    let controller_gain = Ratio::new::<ratio>(0.5);
    let integral_time: Time = controller_gain / Frequency::new::<hertz>(0.3);
    let derivative_time: Time = Time::new::<second>(1.0);
    // derivative time ratio
    let alpha: Ratio = Ratio::new::<ratio>(1.0);

    // simulation settings
    let max_simulation_time: Time = Time::new::<second>(90 as f64);
    let timestep: Time = Time::new::<second>(0.02);
    let mut current_simulation_time: Time = Time::new::<second>(0.0);

    

    let mut pid_controller: AnalogController = 
    AnalogController::new_filtered_pid_controller(controller_gain,
        integral_time,
        derivative_time,
        alpha).unwrap();

    // we also have a measurement delay of 0.0001 s 
    // or 0.1 ms
    let measurement_delay = Time::new::<millisecond>(0.1);

    let mut measurement_delay_block: AnalogController = 
    ProportionalController::new(Ratio::new::<ratio>(1.0)).unwrap().into();

    measurement_delay_block.set_dead_time(measurement_delay);
    // now for the transfer function 

    use uom::si::{Quantity, ISQ, SI};
    use uom::typenum::*;
    // type alias called TimeSquared
    type TimeSquared = 
    Quantity<ISQ<Z0, Z0, P2, Z0, Z0, Z0, Z0>, SI<f64>, f64>;

    let one_second = Time::new::<second>(1.0);

    let a1: TimeSquared = one_second * Time::new::<second>(2.5);
    let b1: Time = -Time::new::<second>(0.5);
    let c1: Ratio = Ratio::new::<ratio>(1.0);

    let a2: TimeSquared =one_second * one_second* 3.0;
    let b2: Time = Time::new::<second>(4.0);
    let c2: Ratio = Ratio::new::<ratio>(4.0);

    let mut tf = TransferFnSecondOrder::new(a1, b1, c1, a2, b2, c2).unwrap();

    //
    // if you need to set initial values
    // because the transfer function only measures deviations from 
    // these inputs and outputs
    //
    // // do this before starting up
    //
    // // actually transfer functions work with deviation variables,
    // // the initial input and output is always zero
    // tf.set_dead_time(initial_value);

    let mut user_set_point = Ratio::ZERO;
    let mut measured_output = Ratio::ZERO;

    // writer creation

    let mut wtr = pid_controller.spawn_writer("fine_timestep_pid_controller_test".to_string()).unwrap();

    let stuff_to_do_in_simulation_loop = move ||{
        // for this case, I have three step functions 
        // the step functions are:
        //
        // 0 to 5s, input is zero 
        // 5s onwards, input (set point) is 5 (dimensionless)

        if current_simulation_time <= Time::ZERO {
            // do nothing, leave it at zero
        } else if current_simulation_time > Time::new::<second>(5.0) {
            user_set_point = Ratio::new::<ratio>(5.0);
        } 

        // error = y_sp(t) - y(t)
        let set_point_error = user_set_point - measured_output;

        // true output

        let transfer_fn_input = pid_controller.set_user_input_and_calc(
            set_point_error, current_simulation_time).unwrap();

        let tf_output = tf.set_user_input_and_calc(transfer_fn_input, 
            current_simulation_time).unwrap();

        // measured output set for next timestep

        measured_output = measurement_delay_block.set_user_input_and_calc(
            tf_output, current_simulation_time).unwrap();


        // write 
        let writer_borrow = &mut wtr;
        pid_controller.csv_write_values(
            writer_borrow, current_simulation_time, 
            user_set_point, tf_output).unwrap();

        current_simulation_time += timestep;
    };

    // need to create a pointer for the stuff_to_do_in_simulation_loop
    // this is to enable parallelism
    let user_task_ptr = Arc::new(Mutex::new(stuff_to_do_in_simulation_loop));
    simulation_template(max_simulation_time, timestep, current_simulation_time,
        user_task_ptr);


}
/// 
/// This is a simulation of a feedback PI controller 
/// The process gain is:
///
///         2.5s^2 - 0.5 s + 1
/// G(s) = ---------------------------
///         3 s^2 + 4 s + 4
///
/// we use a feedback controller with 
/// K_c = 0.5 
/// K_c/tau_i = 0.3 Hertz
/// tau_d = 1.0 s 
/// alpha = 1.0
///
///
/// approximately validated... can use some improvement still
///
/// The derivative function is: 
///
/// G(s) = 0.5s / (1 + s)
/// G(s) = 0.5  * s / (1 + s)
///
/// So derivative gain is 0.5 (same as Kc)
/// derivative time is 1 second 
/// alpha is 1
///
/// usually we use: 
///
/// G(s) = (tau_d s)/(alpha* tau_d s + 1)
///
///
pub(crate) fn proportional_integral_derivative_test(){


    // controller settings
    let controller_gain = Ratio::new::<ratio>(0.5);
    let integral_time: Time = controller_gain / Frequency::new::<hertz>(0.3);
    let derivative_time: Time = Time::new::<second>(1.0);
    // derivative time ratio
    let alpha: Ratio = Ratio::new::<ratio>(1.0);

    // simulation settings
    let max_simulation_time: Time = Time::new::<second>(90 as f64);
    let timestep: Time = Time::new::<second>(0.2);
    let mut current_simulation_time: Time = Time::new::<second>(0.0);

    

    let mut pid_controller: AnalogController = 
    AnalogController::new_filtered_pid_controller(controller_gain,
        integral_time,
        derivative_time,
        alpha).unwrap();

    // we also have a measurement delay of 0.0001 s 
    // or 0.1 ms
    let measurement_delay = Time::new::<millisecond>(0.1);

    let mut measurement_delay_block: AnalogController = 
    ProportionalController::new(Ratio::new::<ratio>(1.0)).unwrap().into();

    measurement_delay_block.set_dead_time(measurement_delay);
    // now for the transfer function 

    use uom::si::{Quantity, ISQ, SI};
    use uom::typenum::*;
    // type alias called TimeSquared
    type TimeSquared = 
    Quantity<ISQ<Z0, Z0, P2, Z0, Z0, Z0, Z0>, SI<f64>, f64>;

    let one_second = Time::new::<second>(1.0);

    let a1: TimeSquared = one_second * Time::new::<second>(2.5);
    let b1: Time = -Time::new::<second>(0.5);
    let c1: Ratio = Ratio::new::<ratio>(1.0);

    let a2: TimeSquared =one_second * one_second* 3.0;
    let b2: Time = Time::new::<second>(4.0);
    let c2: Ratio = Ratio::new::<ratio>(4.0);

    let mut tf = TransferFnSecondOrder::new(a1, b1, c1, a2, b2, c2).unwrap();

    //
    // if you need to set initial values
    // because the transfer function only measures deviations from 
    // these inputs and outputs
    //
    // // do this before starting up
    //
    // // actually transfer functions work with deviation variables,
    // // the initial input and output is always zero
    // tf.set_dead_time(initial_value);

    let mut user_set_point = Ratio::ZERO;
    let mut measured_output = Ratio::ZERO;

    // writer creation

    let mut wtr = pid_controller.spawn_writer("demo_pid_controller_test".to_string()).unwrap();

    let stuff_to_do_in_simulation_loop = move ||{
        // for this case, I have three step functions 
        // the step functions are:
        //
        // 0 to 5s, input is zero 
        // 5s onwards, input (set point) is 5 (dimensionless)

        if current_simulation_time <= Time::ZERO {
            // do nothing, leave it at zero
        } else if current_simulation_time > Time::new::<second>(5.0) {
            user_set_point = Ratio::new::<ratio>(5.0);
        } 

        // error = y_sp(t) - y(t)
        let set_point_error = user_set_point - measured_output;

        // true output

        let transfer_fn_input = pid_controller.set_user_input_and_calc(
            set_point_error, current_simulation_time).unwrap();

        let tf_output = tf.set_user_input_and_calc(transfer_fn_input, 
            current_simulation_time).unwrap();

        // measured output set for next timestep

        measured_output = measurement_delay_block.set_user_input_and_calc(
            tf_output, current_simulation_time).unwrap();


        // write 
        let writer_borrow = &mut wtr;
        pid_controller.csv_write_values(
            writer_borrow, current_simulation_time, 
            user_set_point, tf_output).unwrap();

        current_simulation_time += timestep;
    };

    // need to create a pointer for the stuff_to_do_in_simulation_loop
    // this is to enable parallelism
    let user_task_ptr = Arc::new(Mutex::new(stuff_to_do_in_simulation_loop));
    simulation_template(max_simulation_time, timestep, current_simulation_time,
        user_task_ptr);


}

/// 
/// This is a simulation of a feedback PI controller 
/// The process gain is:
///
///         2.5s^2 - 0.5 s + 1
/// G(s) = ---------------------------
///         3 s^2 + 4 s + 4
///
/// we use a feedback controller with 
/// K_c = 0.5 
/// and K_c/tau_i = 0.3 Hertz
///
/// approximately validated... can use some improvement still
/// this is probably due to the fact I'm using analog controllers when 
/// I should be using digital (discretised) controllers
pub(crate) fn proportional_integral_test(){


    let controller_gain = Ratio::new::<ratio>(0.5);
    let integral_time: Time = controller_gain / Frequency::new::<hertz>(0.3);

    let mut current_simulation_time: Time = Time::new::<second>(0.0);
    // simulation settings
    let max_simulation_time: Time = Time::new::<second>(90 as f64);
    let timestep: Time = Time::new::<second>(0.2);

    let mut pi_controller: AnalogController = 
        AnalogController::new_pi_controller(controller_gain,
            integral_time).unwrap();

    // we also have a measurement delay of 0.0001 s 
    // or 0.1 ms
    let measurement_delay = Time::new::<millisecond>(0.1);

    let mut measurement_delay_block: AnalogController = 
    ProportionalController::new(Ratio::new::<ratio>(1.0)).unwrap().into();

    measurement_delay_block.set_dead_time(measurement_delay);
    // now for the transfer function 

    use uom::si::{Quantity, ISQ, SI};
    use uom::typenum::*;
    // type alias called TimeSquared
    type TimeSquared = 
    Quantity<ISQ<Z0, Z0, P2, Z0, Z0, Z0, Z0>, SI<f64>, f64>;

    let one_second = Time::new::<second>(1.0);

    let a1: TimeSquared = one_second * Time::new::<second>(2.5);
    let b1: Time = -Time::new::<second>(0.5);
    let c1: Ratio = Ratio::new::<ratio>(1.0);

    let a2: TimeSquared =one_second * one_second* 3.0;
    let b2: Time = Time::new::<second>(4.0);
    let c2: Ratio = Ratio::new::<ratio>(4.0);

    let mut tf = TransferFnSecondOrder::new(a1, b1, c1, a2, b2, c2).unwrap();

    //
    // if you need to set initial values
    // because the transfer function only measures deviations from 
    // these inputs and outputs
    //
    // // do this before starting up
    //
    // // actually transfer functions work with deviation variables,
    // // the initial input and output is always zero
    // tf.set_dead_time(initial_value);

    let mut user_set_point = Ratio::ZERO;
    let mut measured_output = Ratio::ZERO;

    // writer creation

    let mut wtr = pi_controller.spawn_writer("demo_pi_controller_test".to_string()).unwrap();

    let stuff_to_do_in_simulation_loop = move ||{
        // for this case, I have three step functions 
        // the step functions are:
        //
        // 0 to 5s, input is zero 
        // 5s onwards, input (set point) is 5 (dimensionless)

        if current_simulation_time <= Time::ZERO {
            // do nothing, leave it at zero
        } else if current_simulation_time > Time::new::<second>(5.0) {
            user_set_point = Ratio::new::<ratio>(5.0);
        } 

        // error = y_sp(t) - y(t)
        let set_point_error = user_set_point - measured_output;

        // true output

        let transfer_fn_input = pi_controller.set_user_input_and_calc(
            set_point_error, current_simulation_time).unwrap();

        let tf_output = tf.set_user_input_and_calc(transfer_fn_input, 
            current_simulation_time).unwrap();

        // measured output set for next timestep

        measured_output = measurement_delay_block.set_user_input_and_calc(
            tf_output, current_simulation_time).unwrap();


        // write 
        let writer_borrow = &mut wtr;
        pi_controller.csv_write_values(
            writer_borrow, current_simulation_time, 
            user_set_point, tf_output).unwrap();

        current_simulation_time += timestep;
    };

    // need to create a pointer for the stuff_to_do_in_simulation_loop
    // this is to enable parallelism
    let user_task_ptr = Arc::new(Mutex::new(stuff_to_do_in_simulation_loop));
    simulation_template(max_simulation_time, timestep, current_simulation_time,
        user_task_ptr);


}

/// 
/// This is a simulation of a feedback P controller 
/// The process gain is:
///
///         2.5s^2 - 0.5 s + 1
/// G(s) = ---------------------------
///         3 s^2 + 4 s + 4
///
/// we use a feedback controller with 
/// K_c = 0.5 
///
/// approximately validated... can use some improvement still
pub(crate) fn proportional_standalone_feedback_test(){


    let controller_gain = Ratio::new::<ratio>(0.5);

    let mut current_simulation_time: Time = Time::new::<second>(0.0);
    // simulation settings
    let max_simulation_time: Time = Time::new::<second>(90 as f64);
    let timestep: Time = Time::new::<second>(0.2);

    let mut p_controller: AnalogController = 
        ProportionalController::new(controller_gain).unwrap().into();

    // we also have a measurement delay of 0.0001 s 
    // or 0.1 ms
    let measurement_delay = Time::new::<millisecond>(0.1);

    let mut measurement_delay_block: AnalogController = 
    ProportionalController::new(Ratio::new::<ratio>(1.0)).unwrap().into();

    measurement_delay_block.set_dead_time(measurement_delay);
    // now for the transfer function 

    use uom::si::{Quantity, ISQ, SI};
    use uom::typenum::*;
    // type alias called TimeSquared
    type TimeSquared = 
    Quantity<ISQ<Z0, Z0, P2, Z0, Z0, Z0, Z0>, SI<f64>, f64>;

    let one_second = Time::new::<second>(1.0);

    let a1: TimeSquared = one_second * Time::new::<second>(2.5);
    let b1: Time = -Time::new::<second>(0.5);
    let c1: Ratio = Ratio::new::<ratio>(1.0);

    let a2: TimeSquared =one_second * one_second* 3.0;
    let b2: Time = Time::new::<second>(4.0);
    let c2: Ratio = Ratio::new::<ratio>(4.0);

    let mut tf = TransferFnSecondOrder::new(a1, b1, c1, a2, b2, c2).unwrap();

    //
    // if you need to set initial values
    // because the transfer function only measures deviations from 
    // these inputs and outputs
    //
    // // do this before starting up
    //
    // // actually transfer functions work with deviation variables,
    // // the initial input and output is always zero
    // tf.set_dead_time(initial_value);

    let mut user_set_point = Ratio::ZERO;
    let mut measured_output = Ratio::ZERO;

    // writer creation

    let mut wtr = p_controller.spawn_writer("demo_p_controller_test".to_string()).unwrap();

    let stuff_to_do_in_simulation_loop = move ||{
        // for this case, I have three step functions 
        // the step functions are:
        //
        // 0 to 5s, input is zero 
        // 5s onwards, input (set point) is 5 (dimensionless)

        if current_simulation_time <= Time::ZERO {
            // do nothing, leave it at zero
        } else if current_simulation_time > Time::new::<second>(5.0) {
            user_set_point = Ratio::new::<ratio>(5.0);
        } 

        // error = y_sp(t) - y(t)
        let set_point_error = user_set_point - measured_output;

        // true output

        let transfer_fn_input = p_controller.set_user_input_and_calc(
            set_point_error, current_simulation_time).unwrap();

        let tf_output = tf.set_user_input_and_calc(transfer_fn_input, 
            current_simulation_time).unwrap();

        // measured output set for next timestep

        measured_output = measurement_delay_block.set_user_input_and_calc(
            tf_output, current_simulation_time).unwrap();


        // write 
        let writer_borrow = &mut wtr;
        p_controller.csv_write_values(
            writer_borrow, current_simulation_time, 
            user_set_point, tf_output).unwrap();

        current_simulation_time += timestep;
    };

    // need to create a pointer for the stuff_to_do_in_simulation_loop
    // this is to enable parallelism
    let user_task_ptr = Arc::new(Mutex::new(stuff_to_do_in_simulation_loop));
    simulation_template(max_simulation_time, timestep, current_simulation_time,
        user_task_ptr);


}


/// 
/// This is a simulation of three step changes for an integral 
/// controller 1/s
pub(crate) fn integral_controller_ramp_test(){

    let one_second = Time::new::<second>(1.0);

    let integral_time: Time = one_second;
    let controller_gain = Ratio::new::<ratio>(1.0);
    let mut current_simulation_time: Time = Time::new::<second>(0.0);
    let max_simulation_time: Time = Time::new::<second>(60 as f64);
    let timestep: Time = Time::new::<second>(0.2);

    let mut integral_controller: AnalogController = 
        IntegralController::new(controller_gain,
            integral_time).unwrap().into();

    //
    // if you need to set initial values
    // because the transfer function only measures deviations from 
    // these inputs and outputs
    //
    // // do this before starting up
    //
    // // actually transfer functions work with deviation variables,
    // // the initial input and output is always zero
    // tf.set_dead_time(initial_value);

    let mut user_input = Ratio::ZERO;

    // writer creation

    let mut wtr = integral_controller.spawn_writer("demo_ramp_fn".to_string()).unwrap();

    let stuff_to_do_in_simulation_loop = move ||{
        // for this case, I have three step functions 
        // the step functions are:
        //
        // at 5s to 10s, input is 1 
        // at at 10s to 15s, input is 2.5
        // at 15s onwards, input is -1

        if current_simulation_time <= Time::ZERO {
            // do nothing, leave it at zero
        } else if current_simulation_time <= Time::new::<second>(5.0) {
            user_input = Ratio::ZERO;
        } else if current_simulation_time <= Time::new::<second>(10.0) {
            user_input = Ratio::new::<ratio>(1.0);
        } else if current_simulation_time <= Time::new::<second>(15.0) {
            user_input = Ratio::new::<ratio>(2.5);
        } else {
            user_input = Ratio::new::<ratio>(-1.0);
        }

        let output = integral_controller.set_user_input_and_calc(
            user_input, current_simulation_time).unwrap();

        // write 
        let writer_borrow = &mut wtr;
        integral_controller.csv_write_values(
            writer_borrow, current_simulation_time, 
            user_input, output).unwrap();

        current_simulation_time += timestep;
    };

    // need to create a pointer for the stuff_to_do_in_simulation_loop
    // this is to enable parallelism
    let user_task_ptr = Arc::new(Mutex::new(stuff_to_do_in_simulation_loop));
    simulation_template(max_simulation_time, timestep, current_simulation_time,
        user_task_ptr);


}

/// 
/// This is a simulation of three step changes for a filtered derivative 
/// controller 1/(0.1 s + 1)
///
/// So derivative time is 1 second, 
/// and alpha is 0.1
pub(crate) fn derivative_controller_step_test(){

    // controller settings
    let one_second = Time::new::<second>(1.0);
    let derivative_time: Time = one_second;
    let controller_gain = Ratio::new::<ratio>(1.0);
    let alpha = Ratio::new::<ratio>(0.1);

    // simulation settings
    let mut current_simulation_time: Time = Time::new::<second>(0.0);
    let max_simulation_time: Time = Time::new::<second>(60 as f64);
    let timestep: Time = Time::new::<second>(0.2);

    let mut filtered_derivative_controller: AnalogController = 
        FilteredDerivativeController::new(controller_gain,
            derivative_time,
            alpha).unwrap().into();

    //
    // if you need to set initial values
    // because the transfer function only measures deviations from 
    // these inputs and outputs
    //
    // // do this before starting up
    //
    // // actually transfer functions work with deviation variables,
    // // the initial input and output is always zero
    // tf.set_dead_time(initial_value);

    let mut user_input = Ratio::ZERO;

    // writer creation

    let mut wtr = filtered_derivative_controller.spawn_writer(
        "demo_filtered_derivative_standalone".to_string()).unwrap();

    let stuff_to_do_in_simulation_loop = move ||{
        // for this case, I have three step functions 
        // the step functions are:
        //
        // at 5s to 10s, input is 1 
        // at at 10s to 15s, input is 2.5
        // at 15s onwards, input is -1

        if current_simulation_time <= Time::ZERO {
            // do nothing, leave it at zero
        } else if current_simulation_time <= Time::new::<second>(5.0) {
            user_input = Ratio::ZERO;
        } else if current_simulation_time <= Time::new::<second>(10.0) {
            user_input = Ratio::new::<ratio>(1.0);
        } else if current_simulation_time <= Time::new::<second>(15.0) {
            user_input = Ratio::new::<ratio>(2.5);
        } else {
            user_input = Ratio::new::<ratio>(-1.0);
        }

        let output = filtered_derivative_controller.set_user_input_and_calc(
            user_input, current_simulation_time).unwrap();

        // write 
        let writer_borrow = &mut wtr;
        filtered_derivative_controller.csv_write_values(
            writer_borrow, current_simulation_time, 
            user_input, output).unwrap();

        current_simulation_time += timestep;
    };

    // need to create a pointer for the stuff_to_do_in_simulation_loop
    // this is to enable parallelism
    let user_task_ptr = Arc::new(Mutex::new(stuff_to_do_in_simulation_loop));
    simulation_template(max_simulation_time, timestep, current_simulation_time,
        user_task_ptr);


}

/// 
/// This is a simulation of three step changes for 
/// proportional controller 
///
/// the transfer function is mean to be 0.5, 
/// so input and output must be half
pub(crate) fn proportional_controller_step_test(){

    // controller settings
    let controller_gain = Ratio::new::<ratio>(0.5);

    // simulation settings
    let mut current_simulation_time: Time = Time::new::<second>(0.0);
    let max_simulation_time: Time = Time::new::<second>(60 as f64);
    let timestep: Time = Time::new::<second>(0.2);

    let mut proportional_controller: AnalogController = 
        ProportionalController::new(controller_gain,
            ).unwrap().into();

    //
    // if you need to set initial values
    // because the transfer function only measures deviations from 
    // these inputs and outputs
    //
    // // do this before starting up
    //
    // // actually transfer functions work with deviation variables,
    // // the initial input and output is always zero
    // tf.set_dead_time(initial_value);

    let mut user_input = Ratio::ZERO;

    // writer creation

    let mut wtr = proportional_controller.spawn_writer(
        "openloop_proportional_controller_standalone".to_string()).unwrap();

    let stuff_to_do_in_simulation_loop = move ||{
        // for this case, I have three step functions 
        // the step functions are:
        //
        // at 5s to 10s, input is 1 
        // at at 10s to 15s, input is 2.5
        // at 15s onwards, input is -1

        if current_simulation_time <= Time::ZERO {
            // do nothing, leave it at zero
        } else if current_simulation_time <= Time::new::<second>(5.0) {
            user_input = Ratio::ZERO;
        } else if current_simulation_time <= Time::new::<second>(10.0) {
            user_input = Ratio::new::<ratio>(1.0);
        } else if current_simulation_time <= Time::new::<second>(15.0) {
            user_input = Ratio::new::<ratio>(2.5);
        } else {
            user_input = Ratio::new::<ratio>(-1.0);
        }

        let output = proportional_controller.set_user_input_and_calc(
            user_input, current_simulation_time).unwrap();

        // write 
        let writer_borrow = &mut wtr;
        proportional_controller.csv_write_values(
            writer_borrow, current_simulation_time, 
            user_input, output).unwrap();

        current_simulation_time += timestep;
    };

    // need to create a pointer for the stuff_to_do_in_simulation_loop
    // this is to enable parallelism
    let user_task_ptr = Arc::new(Mutex::new(stuff_to_do_in_simulation_loop));
    simulation_template(max_simulation_time, timestep, current_simulation_time,
        user_task_ptr);


}

fn simulation_template(
    max_simulation_time: Time,
    timestep: Time,
    mut current_simulation_time: Time,
    user_task_ptr: Arc<Mutex<impl FnMut() -> ()
    + std::marker::Send + 'static>>){

    let user_task_ptr_clone = user_task_ptr.clone();

    let task = move || {
        while current_simulation_time.le(&max_simulation_time) {

            let mut user_task_ref = user_task_ptr_clone.lock().unwrap();
            user_task_ref();

            current_simulation_time += timestep;
        }};

    let handle = thread::spawn(task);
    handle.join().unwrap();
}

