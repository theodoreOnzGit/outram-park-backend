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
use uom::si::ratio::ratio;
use uom::si::time::second;

/// 
/// This is a simulation of:
///
///          5 - 2s
/// G(s) = -------- exp(-2s)
///         4s + 2
///
///
/// Input is in the form:
///
/// G(s) = 
///
/// a1 s + b1 
/// ---------
/// a2 s + b2 
///
///
pub(crate) fn stable_first_order_with_delay_simulation_with_zeroes(){

    // type alias called TimeSquared

    let a1: Time = -Time::new::<second>(2.0);
    let b1: Ratio = Ratio::new::<ratio>(5.0);

    let a2: Time = Time::new::<second>(4.0);
    let b2: Ratio = Ratio::new::<ratio>(2.0);
    let dead_time = Time::new::<second>(2.0);
    let mut current_simulation_time: Time = Time::new::<second>(0.0);
    let max_simulation_time: Time = Time::new::<second>(30.0 as f64);
    let timestep: Time = Time::new::<second>(0.1);

    let mut tf = TransferFnFirstOrder::new(a1, b1, a2, b2).unwrap();
    tf.set_dead_time(dead_time);
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

    let mut wtr = tf.spawn_writer("first_order_with_zeroes_with_delay".to_string()).unwrap();

    let stuff_to_do_in_simulation_loop = move ||{

        // let _output = tf.set_user_input_and_calc(user_input,time);
        // tf.csv_write_values();
        //
        // probably want to assert something as well
        // assert approx equal 

        // should be equal within x percent of a suitable scale
        // so lets say 1e-9 times of 1,
        
        // step up to 9 if t >= 0 
        if current_simulation_time >= Time::ZERO {
            user_input = Ratio::new::<ratio>(9.0);
        }

        let output = tf.set_user_input_and_calc(
            user_input,current_simulation_time).unwrap();
        //dbg!(output);
        // assert example
        assert_abs_diff_eq!(1.0,1.01, epsilon = 0.1);
        let writer_borrow = &mut wtr;
        tf.csv_write_values(writer_borrow, current_simulation_time, 
            user_input, output).unwrap();
        //let current_time_string = current_simulation_time.get::<second>().to_string();
        //let input_string = user_input.get::<ratio>().to_string();
        //let output_string = output.get::<ratio>().to_string();

        
        //wtr.write_record(&[current_time_string,
        //    input_string,
        //    output_string]).unwrap();
        //wtr.flush().unwrap();
        
        current_simulation_time += timestep;
    };

    // need to create a pointer for the stuff_to_do_in_simulation_loop
    // this is to enable parallelism
    let user_task_ptr = Arc::new(Mutex::new(stuff_to_do_in_simulation_loop));
    simulation_template(max_simulation_time, timestep, current_simulation_time,
        user_task_ptr);


}
/// 
/// This is a simulation of:
///
///          5 
/// G(s) = -------- exp(-2s)
///         4s + 2
///
///
/// Input is in the form:
///
/// G(s) = 
///
/// a1 s + b1 
/// ---------
/// a2 s + b2 
/// 
///
/// validated with scilab
///
pub(crate) fn stable_first_order_with_delay_simulation_no_zeroes(){

    // type alias called TimeSquared

    let a1: Time = Time::ZERO;
    let b1: Ratio = Ratio::new::<ratio>(5.0);

    let a2: Time = Time::new::<second>(4.0);
    let b2: Ratio = Ratio::new::<ratio>(2.0);
    let dead_time = Time::new::<second>(2.0);
    let mut current_simulation_time: Time = Time::new::<second>(0.0);
    let max_simulation_time: Time = Time::new::<second>(30.0 as f64);
    let timestep: Time = Time::new::<second>(0.1);

    let mut tf = TransferFnFirstOrder::new(a1, b1, a2, b2).unwrap();
    tf.set_dead_time(dead_time);
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

    let mut wtr = tf.spawn_writer("first_order_no_zeroes_with_delay".to_string()).unwrap();

    let stuff_to_do_in_simulation_loop = move ||{

        // let _output = tf.set_user_input_and_calc(user_input,time);
        // tf.csv_write_values();
        //
        // probably want to assert something as well
        // assert approx equal 

        // should be equal within x percent of a suitable scale
        // so lets say 1e-9 times of 1,
        
        // step up to 9 if t >= 0 
        if current_simulation_time >= Time::ZERO {
            user_input = Ratio::new::<ratio>(9.0);
        }

        let output = tf.set_user_input_and_calc(
            user_input,current_simulation_time).unwrap();
        //dbg!(output);
        // assert example
        assert_abs_diff_eq!(1.0,1.01, epsilon = 0.1);
        let writer_borrow = &mut wtr;
        tf.csv_write_values(writer_borrow, current_simulation_time, 
            user_input, output).unwrap();
        //let current_time_string = current_simulation_time.get::<second>().to_string();
        //let input_string = user_input.get::<ratio>().to_string();
        //let output_string = output.get::<ratio>().to_string();

        
        //wtr.write_record(&[current_time_string,
        //    input_string,
        //    output_string]).unwrap();
        //wtr.flush().unwrap();
        
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
