use fission_yields_data::prelude::Nuclide;

use crate::prelude::{HalfLifeAndDecayEnergyInfo, NuclideReactionAndDecayData, decay_library::DecayLibrary};
/// StochasticDecayChain classes give a single path of the decay chain 
/// based on random number generator
#[derive(Debug, PartialEq,Clone)]
pub struct StochasticDecayChain {
    pub nuclides_and_decay_data_vec: Vec<(Nuclide,HalfLifeAndDecayEnergyInfo)>
}

/// implements iterator for decay chain
pub mod iterator_for_decay_chain;


// and then I want to implement a method that starts from a single nuclide 
// stable or unstable 

impl StochasticDecayChain {

    /// this function returns a single decay chain 
    /// randomly picked from the various branching ratios inside 
    pub fn new_single_stochastic_chain_from_nuclide(starting_nuclide: Nuclide,
        decay_library: &mut DecayLibrary)-> StochasticDecayChain {

        let mut nuclides_and_decay_data: Vec<(Nuclide,HalfLifeAndDecayEnergyInfo)> 
            = vec![];

        // let's first check if the nuclide is stable by obtaining the decay 
        // data 

        let starting_nuclide_data: NuclideReactionAndDecayData 
            = decay_library.try_match_nuclides_to_decay_data(starting_nuclide).unwrap();

        match starting_nuclide_data.half_life_information {
            HalfLifeAndDecayEnergyInfo::Stable => {
                // in case its stable, don't add anything to the decay chain
                // there's nothign to add
                return StochasticDecayChain {
                    nuclides_and_decay_data_vec: nuclides_and_decay_data
                };
            },
            HalfLifeAndDecayEnergyInfo::Unstable(_half_life, _decay_energy) => {
                // in case it's unstable, I need to get the subsequent 
                // nuclide
                // using the random number generator
                // let me get the random number, 

                let (random_num_between_0_and_1,_rng) 
                    = decay_library.get_random_number_and_rng();

                // then get next target nuclide with float 
                let (next_nuclide, _decay_type) 
                    = starting_nuclide_data.get_next_target_nuclide_with_float(
                        random_num_between_0_and_1)
                    .unwrap();

                let next_nuclide_data: NuclideReactionAndDecayData 
                    = decay_library
                    .try_match_nuclides_to_decay_data(next_nuclide)
                    .unwrap();

                // if unstable, we push the next decay information up
                nuclides_and_decay_data.push(
                    (next_nuclide,next_nuclide_data.half_life_information)
                );

            },
        }

        // now here, we have to enter a while loop and keep adding
        // till the decays finish

        let (latest_nuclide,
            latest_nuclide_decay_info) 
            = nuclides_and_decay_data.last().unwrap();
        
        let mut subsequent_nuclide: Nuclide = latest_nuclide.clone();
        let mut subsequent_nuclide_decay_info: HalfLifeAndDecayEnergyInfo 
            = latest_nuclide_decay_info.clone();

        let mut subsequent_nuclide_data: NuclideReactionAndDecayData 
            = decay_library.try_match_nuclides_to_decay_data(
                subsequent_nuclide
            ).unwrap();

        while subsequent_nuclide_data.is_unstable() {

            match subsequent_nuclide_decay_info {
                HalfLifeAndDecayEnergyInfo::Stable => {
                    // in case its stable, don't add anything to the decay chain
                    // there's nothign to add
                    return StochasticDecayChain {
                        nuclides_and_decay_data_vec: nuclides_and_decay_data
                    };
                },
                HalfLifeAndDecayEnergyInfo::Unstable(_half_life, _decay_energy) => {
                    // in case it's unstable, I need to get the subsequent 
                    // nuclide
                    // using the random number generator
                    // let me get the random number, 

                    let (random_num_between_0_and_1,_rng) 
                        = decay_library.get_random_number_and_rng();

                    // then get next target nuclide with float 
                    let (next_nuclide, _decay_type) 
                        = subsequent_nuclide_data.get_next_target_nuclide_with_float(
                            random_num_between_0_and_1)
                        .unwrap();

                    let next_nuclide_data: NuclideReactionAndDecayData 
                        = decay_library
                        .try_match_nuclides_to_decay_data(next_nuclide)
                        .unwrap();

                    subsequent_nuclide_decay_info = 
                        next_nuclide_data.half_life_information.clone();
                    subsequent_nuclide = next_nuclide;
                    subsequent_nuclide_data = next_nuclide_data;

                    // if unstable, we push the next decay information up
                    nuclides_and_decay_data.push(
                        (subsequent_nuclide,subsequent_nuclide_decay_info.clone())
                    );



                },
            }

        }






        return StochasticDecayChain {
            nuclides_and_decay_data_vec: nuclides_and_decay_data
        };


    }
}



