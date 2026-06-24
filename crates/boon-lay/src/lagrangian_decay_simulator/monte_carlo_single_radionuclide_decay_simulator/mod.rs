use std::time::SystemTime;

use fission_yields_data::prelude::Nuclide;
use openmc_libs::rng::lcg::Lcg64 as Rand64;
use uom::{ConstZero, si::f64::*};
use uom::si::time::second;
use uom::si::time::millisecond;
use uom::si::radioactivity::becquerel;

use crate::prelude::decay_library::DecayLibrary;
use crate::prelude::NuclideReactionAndDecayData;
use crate::lagrangian_decay_simulator::StochasticDecayChain;
use crate::prelude::HalfLifeAndDecayEnergyInfo;

#[derive(Debug,Clone,PartialEq)]
pub struct SingleNuclideSimulatorMC {
    /// the current nuclide the simulator is simulating
    /// it can change over time
    current_nuclide: Nuclide,
    /// current half life information for current nuclide 
    current_half_life_info: HalfLifeAndDecayEnergyInfo,
    /// current time to next decay 
    current_time_to_next_decay: Time,
    /// time passed in the simulation
    simulated_time: Time,
    /// time passed in real-life
    calculation_time: Time,

    /// the decay chain in equation 
    stochastic_decay_chain: StochasticDecayChain,

    /// the time to live vector showing how long the subsequent nuclides 
    /// live
    ///
    /// basically, time remaining to next decay
    time_to_live_vec: Vec<Time>,

    /// a position vector representing the position of the nuclide 
    /// this is based on cartesian coordinates
    pub position: (Length, Length, Length)
}

// basically the idea of this simulator is to take a current nuclide,
// then simulate the decay of it over time 
//
// so it will take the current nuclide and before simulation:
// 1. generate the decay chain of subsequent nuclides 
// 2. generate a vector of time remaining to the next nuclide 
// (this is stochastically determined based on half life)
//
// Now during simulation, the simulator will take in a timestep, and 
// based on that timestep given, proceed to the next nuclide
//
// Note: if concerns exist about computational expense, 
// we can always speed up simulation using flamegraph later.
//
//

impl SingleNuclideSimulatorMC {

    /// this obtains a time to live stochastically for the decay chain using 
    /// half life 
    /// From:
    /// N = N\_0 exp(-lambda * t)
    ///
    /// we get:
    /// t = Ln (N/N\_0) / (-lambda)
    /// t = Ln (N/N\_0) / (ln 2) * half life.
    ///
    /// N/N\_0 is a random number between 0 and 1
    pub fn get_time_to_decay_stochastic(rng: &mut Rand64, half_life: Time) 
        -> Time {

            let n_by_n0 = rng.rand_float();

            let half_life_coeff: f64 = n_by_n0.ln().abs() / (2.0_f64.ln());

            return half_life_coeff * half_life;

    }

    /// generate a new decay chain simulation 
    pub fn new_decay_chain_simulation(current_nuclide: Nuclide,
        decay_library: &mut DecayLibrary,
    ) -> Self {

        // first let's get a decay chain stochastically (ie pick one decay 
        // branch)

        let decay_chain_for_new_nuclide = 
            StochasticDecayChain::new_single_stochastic_chain_from_nuclide(
                current_nuclide, 
                decay_library
            );

        // now we can generate a time to live vector 

        let mut time_to_live_vec: Vec<Time> = vec![];

        for (_nuclide, half_life_info) in decay_chain_for_new_nuclide.iter() {

            match half_life_info {
                HalfLifeAndDecayEnergyInfo::Stable => {

                    // for stable nuclides, the time to live is 
                    // not a number (basically infinite)
                    time_to_live_vec.push(
                        Time::new::<second>(f64::INFINITY)
                    );
                },
                HalfLifeAndDecayEnergyInfo::Unstable(
                    half_life, _decay_energy
                ) => {
                    let time_to_live = 
                        Self::get_time_to_decay_stochastic(
                            &mut decay_library.random_number_generator, 
                            *half_life
                        );

                    time_to_live_vec.push(time_to_live);
                },
            }
            

        }

        // this panics for ultra heavy nuclides
        let nuclide_decay_struct: NuclideReactionAndDecayData 
            = decay_library.try_match_nuclides_to_decay_data(current_nuclide)
            .unwrap();

        let current_half_life_info = nuclide_decay_struct.half_life_information;

        let current_time_to_next_decay: Time;

        match current_half_life_info {
            HalfLifeAndDecayEnergyInfo::Stable => {

                // for stable nuclides, the time to live is 
                // not a number (basically infinite)
                current_time_to_next_decay = Time::new::<second>(f64::INFINITY);
            },
            HalfLifeAndDecayEnergyInfo::Unstable(
                half_life, _decay_energy
            ) => {
                let time_to_live = 
                    Self::get_time_to_decay_stochastic(
                        &mut decay_library.random_number_generator, 
                        half_life
                    );

                current_time_to_next_decay = time_to_live
            },
        }


        let position = (Length::ZERO, Length::ZERO, Length::ZERO);

        return Self {
            current_nuclide,
            current_half_life_info,
            simulated_time: Time::ZERO,
            calculation_time: Time::ZERO,
            stochastic_decay_chain: decay_chain_for_new_nuclide,
            time_to_live_vec,
            current_time_to_next_decay,
            position,
        };

    }
    /// generate a new decay chain simulation 
    /// based on a new nuclide, usually due to transmutation, 
    /// but keep the elapsed time and simulated time
    pub fn transmute_nuclide(&mut self, 
        nuclide: Nuclide,
        decay_library: &mut DecayLibrary,){

        // i'm going to return a blank decay simulation first 

        let mut fresh_simulation = Self::new_decay_chain_simulation(
            nuclide, decay_library);

        fresh_simulation.simulated_time = self.simulated_time;
        fresh_simulation.calculation_time = self.calculation_time;

        *self = fresh_simulation;
    }

    // move the simulation forward by some time supplied by the user
    // also provides the nuclide of interest currently
    #[inline]
    pub fn advance_timestep(&mut self, timestep: Time) -> 
        (Nuclide, HalfLifeAndDecayEnergyInfo){
        // we do loop timing
        let loop_time = SystemTime::now();
        let loop_time_start = loop_time.elapsed().unwrap();

        // main calculation loop
        //
        // given a timestep, the job is to then find the next nuclide
        {
            // first, add the timestep 
            self.simulated_time += timestep;
            // this is for multiple nuclides remaining
            // if timestep overshoots several nuclide decays, this is 
            // helpful
            let mut timestep_remaining = timestep;


            for (i, (nuclide,half_life_info)) in 
                self.stochastic_decay_chain.iter().enumerate() {

                    let time_to_next_nuclide = self.current_time_to_next_decay;

                    // in the case timestep is less than the decay to next 
                    // nuclide, deduct the time to live for the time 
                    // to next nuclide
                    //
                    // the current nuclide has not decayed yet
                    if timestep_remaining < time_to_next_nuclide {
                        self.current_time_to_next_decay -= timestep_remaining;
                        break;
                    };

                    // in the case timestep is equal to decay of next nuclide 
                    // we have a single decay
                    if timestep_remaining == time_to_next_nuclide {

                        // firstly, current time to next decay is zero
                        // (ie we have moved on to the next nuclide)
                        // when moving onto the next nuclide, the current time 
                        // to next decay is the first element of the 
                        // time to live vector
                        self.current_time_to_next_decay = self.time_to_live_vec[i];
                        self.current_nuclide = *nuclide;
                        self.current_half_life_info = half_life_info.clone();
                        // time to live for next nuclide is zero
                        self.time_to_live_vec[i] = Time::ZERO;
                        // break out of the loop
                        break;

                    }

                    // in case timestep is more than time to next nuclide,
                    if timestep_remaining > time_to_next_nuclide {

                        // subtract the current time to next decay
                        // from the remaining
                        timestep_remaining -= self.current_time_to_next_decay;
                        // move onto the next nuclide in the decay chain
                        self.current_time_to_next_decay = self.time_to_live_vec[i];
                        self.current_nuclide = *nuclide;
                        self.current_half_life_info = half_life_info.clone();

                        // mark time to live as zero, indicating that this 
                        // nuclide has decayed
                        self.time_to_live_vec[i] = Time::ZERO;

                        // once done, continue to the next timestep

                    }



            }

        }

        // now let's tidy up
        // first the stochastic decay chain vector 

        for time_to_live in self.time_to_live_vec.iter(){

            if *time_to_live == Time::ZERO {

                // basically we keep removing if we have time to live = 0 
                // then we remove the first element of 
                // the vector in the decay chain

                self.stochastic_decay_chain.nuclides_and_decay_data_vec.remove(0);


            } else if *time_to_live > Time::ZERO {

                // basically this is this is the case if the time to live is 
                // greater than 0, then we stop doing this 
                // this is important because if the rng gives 
                //
                // time to live
                // [
                // 10s,
                // 0s,
                // 30s 
                // ] 
                //
                // I don't want to remove the 0s time to live just yet, 
                // that will mess up the logic 
                //
                // save that for the next time
                // 
                
                break;

            }

        }

        // next i will clean up the time to live 
        // this will clear up the leading zeroes

        // this was advised by ChatGPT5
        //


        let first_non_zero = 
            self.time_to_live_vec.iter()
            .position(|&x| x != Time::ZERO)
            .unwrap_or(self.time_to_live_vec.len());
        self.time_to_live_vec.drain(0..first_non_zero);

        // now that we've cleared up the vectors, we can time the simulation




        let loop_time_end = loop_time.elapsed().unwrap();
        let time_taken_for_calculation_loop_milliseconds: f64 = 
            (loop_time_end - loop_time_start)
            .as_millis() as f64;

        self.calculation_time += Time::new::<millisecond>(
            time_taken_for_calculation_loop_milliseconds
        );

        return (self.current_nuclide, self.current_half_life_info.clone());

    }

    /// as function name implies, get time to next decay
    /// unless the radionuclide is already stable
    #[inline]
    pub fn get_time_to_next_decay(&self) -> Time {

        self.current_time_to_next_decay

    }
    /// as name implies, gets current nuclide
    #[inline]
    pub fn get_current_nuclide(&self) -> Nuclide {

        self.current_nuclide

    }
    /// as function name implies, get nuclide in next decay
    /// unless the radionuclide is already stable
    /// then returns None
    #[inline]
    pub fn get_next_decay_nuclide(&self) -> Option<Nuclide> {

        match self.current_half_life_info {
            HalfLifeAndDecayEnergyInfo::Stable => return None,
            HalfLifeAndDecayEnergyInfo::Unstable(_, _) => {

            },
        }

        // if we have decays, the next nuclide is the first 
        // in the vector
        let (next_nuclide, _half_life_info) = self
            .stochastic_decay_chain
            .nuclides_and_decay_data_vec.first().unwrap();

        return Some(*next_nuclide);


    }
    /// as function name implies, get the time to live vector
    #[inline]
    pub fn get_time_to_live_vec(&self) -> Vec<Time> {

        return self.time_to_live_vec.clone();
    }
    #[inline]
    pub fn get_decay_chain_vec(&self) -> Vec<Nuclide> {
        let mut decay_chain_vec: Vec<Nuclide> = vec![];

        for (nuclide,_half_life_info) in &self.stochastic_decay_chain {
            decay_chain_vec.push(*nuclide);
        }


        return decay_chain_vec;
    }
    /// gets current simulated time
    #[inline]
    pub fn get_current_simulated_time(&self) -> Time {
        return self.simulated_time;
    }
    /// gets current elapsed time
    #[inline]
    pub fn get_current_elapsed_time(&self) -> Time {
        return self.calculation_time;
    }

    #[inline]
    pub fn get_current_half_life_info(&self) -> HalfLifeAndDecayEnergyInfo {
        self.current_half_life_info.clone()
    }
    // returns half life
    #[inline]
    pub fn get_current_half_life(&self) -> Time {
        match self.current_half_life_info {
            HalfLifeAndDecayEnergyInfo::Stable => {
                Time::new::<second>(f64::INFINITY)
            },
            HalfLifeAndDecayEnergyInfo::Unstable(half_life, _decay_energy) => {
                half_life
            },
        }
    }

    // returns decay constant 
    #[inline]
    pub fn get_decay_constant(&self) -> Radioactivity {
        match self.current_half_life_info {
            HalfLifeAndDecayEnergyInfo::Stable => {
                Radioactivity::new::<becquerel>(0.0)
            },
            HalfLifeAndDecayEnergyInfo::Unstable(half_life, _decay_energy) => {
                (2_f64.ln()/half_life).into()
            },
        }
    }


    #[inline]
    pub fn force_decay_to_next_nuclide(&mut self) -> (Nuclide, HalfLifeAndDecayEnergyInfo) 
    {
        let timestep = self.get_time_to_next_decay();

        self.advance_timestep(timestep)

    }

    #[inline]
    pub fn check_if_current_nuclide_matches(&self, nuclide_to_check: Nuclide) -> bool {
        if self.get_current_nuclide() == nuclide_to_check {
            return true;
        } 

        return false;

    }

}

pub mod postprocessing;

#[cfg(test)]
pub mod tests;
