use openmc_libs::rng::lcg::Lcg64 as Rand64;

use crate::prelude::decay_library::DecayLibrary;

impl DecayLibrary {

    /// allows user to obtain a random number and a clone of 
    /// the rng
    pub fn get_random_number_and_rng(&mut self,) -> (f64, Rand64) {
        let random_num = self.random_number_generator.rand_float();

        return (random_num, self.random_number_generator.clone());
    }
}
