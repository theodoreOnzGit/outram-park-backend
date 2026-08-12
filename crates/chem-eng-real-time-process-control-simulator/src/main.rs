// Copyright [2023] [Theodore Kay Chen Ong, Professor Per F. Peterson,
// University of California, Berkeley
// Thermal Hydraulics Lab, Repository Contributors and
// Singapore Nuclear Research and Safety Initiative (SNRSI)]
//
// SPDX-License-Identifier: GPL-3.0-only
//
// Relicensed from Apache-2.0 to GPL-3.0-only on 2026-08-11 by the sole
// copyright holder (maintainer-directed) — see the crate NOTICE file.
// Versions of this crate published to crates.io before the relicense
// remain available under Apache-2.0.
//
// This program is free software: you can redistribute it and/or modify it
// under the terms of the GNU General Public License as published by the
// Free Software Foundation, version 3 of the License.
//
// This program is distributed in the hope that it will be useful, but
// WITHOUT ANY WARRANTY; without even the implied warranty of
// MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE.  See the GNU
// General Public License for more details.
//
// You should have received a copy of the GNU General Public License along
// with this program.  If not, see <https://www.gnu.org/licenses/>.
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
fn _debug() {
    examples::second_order_demos::_debug_stable_critdamped_second_order_simulation();
    examples::second_order_demos::_debug2_stable_critdamped_second_order_simulation();
    examples::second_order_demos::_debug_stable_overdamped_second_order_simulation();
    examples::second_order_demos::_debug2_stable_overdamped_second_order_simulation();
}
