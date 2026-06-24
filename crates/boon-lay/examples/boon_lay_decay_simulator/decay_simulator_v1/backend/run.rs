use std::{sync::{Arc, Barrier, Mutex}, thread, time::{Duration, SystemTime}};

use boon_lay::{Nuclide, prelude::{SingleNuclideSimulatorMC, decay_library::DecayLibrary}};
use uom::si::{f64::Time, time::{millisecond, second}};

use crate::decay_simulator_v1::{backend::simulator_state::SimulatorState, DecaySimApp};


impl DecaySimApp {

    /// at each simulation, the simulator will run in the background
    ///
    /// now, challenge is, each simulator may run too fast, or slow
    /// depending on thread speed, relative to other simulations
    ///
    ///
    /// the way to do it, according to ChatGPT5, is to use Arc barrier
    ///
    pub fn run_decay_chain_simulation(
        thread_ptr: Arc<Mutex<(Vec<SingleNuclideSimulatorMC>,DecayLibrary)>>,
        simulator_state_ptr: Arc<Mutex<SimulatorState>>,
        thread_number: u8,
        barrier: Arc<Barrier>,
        ){

        let loop_time = SystemTime::now();


        // this is the main loop
        loop {


            let loop_time_start = loop_time.elapsed().unwrap();
            // firstly, we obtain the simulator state
            let simulator_state_clone: SimulatorState
                = simulator_state_ptr.lock().unwrap().clone();
            // now, for timekeeping, only thread 1 is responsible,
            // no other thread is important in this regard
            // other barriers will stay in sync because of the barrier
            // code


            // check if the pause button is on
            // if pause button on, skip all contents in current iteration
            if simulator_state_clone.is_paused() {

                continue;
            }



            // check if the restart button is pressed

            if simulator_state_clone.is_restart_button_pressed() {

                // if restart button is pressed, then reset the simulator
                // with the nuclide supplied by the user but all times set to zero

                let (mut simulation_vector, mut decay_library):
                    (Vec<SingleNuclideSimulatorMC>, DecayLibrary) =
                     thread_ptr.lock().unwrap().clone();

                // let me get the nuclide of interest first

                let user_set_nuclide: Nuclide =
                    simulator_state_ptr.lock().unwrap().get_user_selected_nuclide();

                // this pre-simulates all the decay trajectories
                for simulation in simulation_vector.iter_mut() {

                    let new_simulation
                        = SingleNuclideSimulatorMC::new_decay_chain_simulation(
                            user_set_nuclide, &mut decay_library
                        );


                    *simulation = new_simulation;

                }
                // for convenience, i want to get the half life of this
                // nuclide
                let nuclide_info: boon_lay::prelude::NuclideReactionAndDecayData =
                    decay_library.try_match_nuclides_to_decay_data(user_set_nuclide)
                    .unwrap();

                // get half life
                let nuclide_half_life: Time =
                    nuclide_info.try_get_half_life().unwrap();

                // set timestep to 0.1% half life
                let timestep_based_on_hl: Time = 1e-3 * nuclide_half_life;


                // make sure all threads in sync
                barrier.wait();

                // once done
                *thread_ptr.lock().unwrap() =
                    (simulation_vector, decay_library);
                if thread_number == 1 {

                    simulator_state_ptr.lock().unwrap().turn_off_restart_button();
                    simulator_state_ptr.lock().unwrap().turn_off_change_nuclide_button();
                    simulator_state_ptr.lock().unwrap().reset_simulated_time();
                    simulator_state_ptr.lock().unwrap().set_timestep(timestep_based_on_hl);

                    // upon restarting, we must toggle a flag to replot
                    // the nuclides
                    simulator_state_ptr.lock().unwrap().turn_on_change_nuclide_to_plot_button();

                }

                // make sure all threads in sync
                barrier.wait();

            }

            if simulator_state_clone.is_change_nuclide_button_pressed() {
                // check if change_nuclide button is pressed,
                // if so, change the nuclide button, but do not reset
                // all the time to zero

                // first if this is thread 1, then we change
                // all the restart and change nuclide off

                let (mut simulation_vector, mut decay_library):
                    (Vec<SingleNuclideSimulatorMC>, DecayLibrary) =
                     thread_ptr.lock().unwrap().clone();

                // let me get the nuclide of interest first

                let user_set_nuclide: Nuclide =
                    simulator_state_ptr.lock().unwrap().get_user_selected_nuclide();

                barrier.wait();
                // this pre-simulates all the decay trajectories
                for simulation in simulation_vector.iter_mut() {
                    simulation.transmute_nuclide(user_set_nuclide, &mut decay_library);

                }
                // once done
                *thread_ptr.lock().unwrap() =
                    (simulation_vector, decay_library.clone());


                // for convenience, i want to get the half life of this
                // nuclide
                let nuclide_info: boon_lay::prelude::NuclideReactionAndDecayData =
                    decay_library.try_match_nuclides_to_decay_data(user_set_nuclide)
                    .unwrap();

                // get half life
                let nuclide_half_life: Time =
                    nuclide_info.try_get_half_life().unwrap();

                // set timestep to 0.1% half life
                let timestep_based_on_hl: Time = 1e-3 * nuclide_half_life;
                // make sure all threads in sync
                barrier.wait();

                if thread_number == 1 {

                    simulator_state_ptr.lock().unwrap().turn_off_restart_button();
                    simulator_state_ptr.lock().unwrap().turn_off_change_nuclide_button();
                    simulator_state_ptr.lock().unwrap().set_timestep(timestep_based_on_hl);

                    // upon changing nuclide, we must toggle a flag to replot
                    // the nuclides
                    simulator_state_ptr.lock().unwrap().turn_on_change_nuclide_to_plot_button();

                }

                // make sure all threads in sync
                barrier.wait();
            }

            // so all the conditions for running are met
            // now we just run

            // firstly, we lock the pointer
            // making a clone of the simulation vector and library

            let (mut simulation_vector, decay_library):
                (Vec<SingleNuclideSimulatorMC>, DecayLibrary) =
                 thread_ptr.lock().unwrap().clone();

            // technically decay libraries are not needed here

            let timestep = simulator_state_clone.get_timestep();

            // all we are doing here is to advance_timestep
            for decay_simulation in simulation_vector.iter_mut() {
                decay_simulation.advance_timestep(timestep);
            };

            // once the decay simulation is complete, lock the thread ptr
            // and return the simulation vector
            *thread_ptr.lock().unwrap() = (simulation_vector, decay_library);

            // now let's keep things in time
            // this is for real-time simulation

            let realtime = false;

            if realtime {
                let loop_time_end = loop_time.elapsed().unwrap();
                let time_taken_for_calculation_loop_milliseconds: f64 =
                    (loop_time_end - loop_time_start)
                    .as_millis() as f64;

                let time_to_sleep_milliseconds: u64 =
                    (timestep.get::<millisecond>() -
                     time_taken_for_calculation_loop_milliseconds)
                    .round().abs() as u64;

                let time_to_sleep_realtime: Duration =
                    Duration::from_millis(time_to_sleep_milliseconds - 1);
                // time to sleep for real-time (default)
                thread::sleep(time_to_sleep_realtime);
            } else {
                let time_to_sleep_milliseconds: u64 =
                    50;
                let time_to_sleep_non_realtime: Duration =
                    Duration::from_millis(time_to_sleep_milliseconds);
                thread::sleep(time_to_sleep_non_realtime);
            }

            barrier.wait();

            // again, only thread 1 is responsible to timekeeping
            // other threads don't touch

            if thread_number == 1 {

                simulator_state_ptr.lock().unwrap().add_to_simulated_time(timestep);

                let elapsed_time_seconds =
                    (loop_time.elapsed().unwrap().as_secs_f64() * 100.0).round()/100.0;
                let elapsed_time = Time::new::<second>(elapsed_time_seconds);

                // count fraction remaining (approx)
                let (simulation_vector, _decay_library):
                    (Vec<SingleNuclideSimulatorMC>, DecayLibrary) =
                     thread_ptr.lock().unwrap().clone();

                let user_set_nuclide: Nuclide =
                    simulator_state_ptr.lock().unwrap().get_user_selected_nuclide();

                let mut surviving_nuclide_counter = 0;

                for nuclide_simulation in simulation_vector.iter() {
                    if nuclide_simulation.check_if_current_nuclide_matches(user_set_nuclide){
                        surviving_nuclide_counter += 1;
                    }
                }


                let nuclide_fraction_remaining =
                    surviving_nuclide_counter as f64 /
                    simulation_vector.clone().len() as f64 ;

                simulator_state_ptr.lock().unwrap().set_nuclide_fraction(
                    nuclide_fraction_remaining
                );

                simulator_state_ptr.lock().unwrap().set_elapsed_time(elapsed_time);

            }


            // thread 2 will be responsible for constructing the nuclide fraction
            // vector
            if thread_number == 2 {


                let (simulation_vector, _decay_library):
                    (Vec<SingleNuclideSimulatorMC>, DecayLibrary) =
                     thread_ptr.lock().unwrap().clone();


                let mut nuclide_vector: Vec<Nuclide> = vec![];

                for simulation in simulation_vector {
                    let nuclide = simulation.get_current_nuclide();
                    nuclide_vector.push(nuclide);
                }

                let nuclide_fraction_vector: Vec<(Nuclide, f64)> =
                    DecaySimApp::fractions_vec_map(&nuclide_vector);

                simulator_state_ptr.lock().unwrap().set_nuclide_fraction_vector(
                    nuclide_fraction_vector);



            }
            barrier.wait();



        };



    }



}
