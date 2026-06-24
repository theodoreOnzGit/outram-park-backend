use std::sync::{Arc, Mutex};

use boon_lay::{prelude::{decay_library::DecayLibrary, SingleNuclideSimulatorMC}, Nuclide};
use openmc_libs::rng::lcg::Lcg64;

use crate::decay_simulator_v1::DecaySimApp;

impl DecaySimApp {

    /// this basically constructs a simulation for a single thread to run
    pub fn construct_new_single_thread_multi_particle_simulation(num_of_nuclides: u64,
        nuclide: Nuclide,
        rng_seed: u64)->
        Arc<Mutex<(Vec<SingleNuclideSimulatorMC>, DecayLibrary)>>{

            let mut decay_library = DecayLibrary::new();
            // i want some random seed

            decay_library.random_number_generator =
                Lcg64::new(rng_seed as u128);


            let new_simulation
                = SingleNuclideSimulatorMC::new_decay_chain_simulation(
                    nuclide, &mut decay_library
                );
            let mut v: Vec<SingleNuclideSimulatorMC> = vec![
                new_simulation; num_of_nuclides.try_into().unwrap()
            ];

            // basically I should not be repeating the same nuclide simulation,
            // I need to be individually constructing them

            // i tried rayon here, can't really do this because the
            // decay library
            for simulation in v.iter_mut() {
                let new_simulation
                    = SingleNuclideSimulatorMC::new_decay_chain_simulation(
                        nuclide, &mut decay_library
                    );


                *simulation = new_simulation;


            }


            return Arc::new(Mutex::new(
                    (v,decay_library)
            ));
    }

}


/// contains information for the user to communicate between the UI and the
/// threads doing the computation work
pub mod simulator_state;

pub mod run;
