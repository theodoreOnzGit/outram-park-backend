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
#[macro_use]
extern crate approx;
mod examples;
fn main() {
    println!("library_demo");
    examples::second_order_demos::stable_second_order_simulation();
    examples::second_order_demos::no_zeroes_stable_underdamped_second_order_simulation();
    examples::second_order_demos::decaying_sine_stable_underdamped_second_order_simulation();
    examples::second_order_demos::demo_complex_stable_underdamped_second_order_simulation();
    examples::second_order_demos::demo_stable_critdamped_second_order_simulation();

    examples::second_order_demos::demo_stable_overdamped_second_order_simulation();

    examples::first_order_demos::stable_first_order_with_delay_simulation_no_zeroes();
    examples::first_order_demos::stable_first_order_with_delay_simulation_with_zeroes();
    examples::generic_transfer_fn_demos::stable_second_order_simulation_with_delay();
    examples::analog_pid_demos::integral_controller_ramp_test();
    examples::analog_pid_demos::proportional_integral_test();
    examples::analog_pid_demos::proportional_integral_derivative_test();
    examples::analog_pid_demos::fine_timesteps_proportional_integral_derivative_test();
    examples::analog_pid_demos::derivative_controller_step_test();
    examples::analog_pid_demos::proportional_standalone_feedback_test();
    examples::analog_pid_demos::proportional_controller_step_test();
    examples::feedback_control_examples::proportional_derivative_kick_eliminator_feedback_loop_example();

    // uncomment for debug 
    //_debug();
}

// some tests and examples are used for debugging, i leave them here
fn _debug(){
    examples::second_order_demos::_debug_stable_critdamped_second_order_simulation();
    examples::second_order_demos::_debug2_stable_critdamped_second_order_simulation();
    examples::second_order_demos::_debug_stable_overdamped_second_order_simulation();
    examples::second_order_demos::_debug2_stable_overdamped_second_order_simulation();

}

