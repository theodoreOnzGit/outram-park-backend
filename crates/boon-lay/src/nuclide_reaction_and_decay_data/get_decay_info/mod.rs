use crate::prelude::{DecayType, NuclideReactionAndDecayData};
use fission_yields_data::prelude::Nuclide;
use openmc_libs::rng::lcg::Lcg64 as Rand64;
use uom::si::{f64::*, ratio::ratio};

impl NuclideReactionAndDecayData {
    /// this obtains half life of the nuclide 
    ///
    /// if stable, this returns none
    pub fn try_get_half_life(&self) -> Option<Time> {

        match self.half_life_information {
            super::HalfLifeAndDecayEnergyInfo::Stable => None,
            super::HalfLifeAndDecayEnergyInfo::Unstable(
                half_life, _decay_energy
            ) => Some(half_life),
        }
    }
    /// checks whether the nuclide is stable 
    #[inline]
    pub fn is_stable(&self) -> bool {

        match self.half_life_information {
            super::HalfLifeAndDecayEnergyInfo::Stable => true,
            super::HalfLifeAndDecayEnergyInfo::Unstable(
                _half_life, _decay_energy
            ) => false,
        }
    }

    /// checks whether nuclide is unstable (just for readability sake)
    #[inline]
    pub fn is_unstable(&self) -> bool {

        // use the previous code lol, opposite of is_stable()
        return !self.is_stable();
    }



    /// this obtains decay energy of the nuclide 
    ///
    /// if stable, this returns none
    pub fn get_decay_energy(&self) -> Option<Energy> {

        match self.half_life_information {
            super::HalfLifeAndDecayEnergyInfo::Stable => None,
            super::HalfLifeAndDecayEnergyInfo::Unstable(
                _half_life, decay_energy
            ) => Some(decay_energy),
        }
    }


    /// get decay branch, branching ratio, decay type and target 
    ///
    /// Question is how to represent branch data most effectively 
    /// so that it is easy to access and construct decay chains
    ///
    /// Is a struct good? or enum?
    pub fn get_decay_branch_info(&self) -> Vec<(Ratio, Nuclide,DecayType)> {


        todo!()

    }


    // get next target using oorandom pseudorandom number generator
    // this is done along with decay type
    // from oorandom
    pub fn get_next_target_nuclide_with_rng(&self, rng: &mut Rand64)-> Option<(Nuclide, DecayType)> {


        // first let's use the rng to get a number between 0 and 1 
        let mut random_num_between_0_and_1 = rng.rand_float();

        // let's obtain the branching ratios 
        let decay_branch_data = self.decay_information.clone();

        // let's do a case for 0 and 1 

        if decay_branch_data.len() == 0 {
            // this is no more nuclide, this is, the nuclide is stable 
            return None;

        }

        if decay_branch_data.len() == 1 {
            // this means radionuclide is unstable, but only one 
            // decay path
            let target_nuclide = decay_branch_data[0].target.unwrap();
            let decay_type = decay_branch_data[0].decay_type;

            return Some((target_nuclide,decay_type));
        }

        // then if we have more than one branch, we do the main code
        //
        // basically if we have a branching ratio of 0.6, 0.2 and 0.2 
        //
        // for decay branch 1, 2 and 3
        //
        // and our RNG produces 0.75, 
        //
        // we should be going for branch 2 
        //
        // |--------------|----|----|
        // 0.0          0.6   0.8   1.0
        //     branch 1     br 2  br3
        //
        // we can see 0.75 is in branch 2
        //
        // 
        // For this, the algorithm can be,
        //
        // suppose the branching ratio is 0.6 first 
        //
        // and 0.75 > 0.6, 
        //
        // we subtract 0.6 from 0.75 to get 0.15 
        //
        // the next branching ratio is 0.2 
        //
        // now, 0.15 < 0.2 
        //
        // so we select branch 2, and the target within branch 2
        //
        //


        for decay_data in decay_branch_data.iter() {
            // now, we check if the random number is greater than the 
            // branching ratio

            let branching_ratio_float = decay_data.branching_ratio.get::<ratio>();

            if random_num_between_0_and_1 > branching_ratio_float {
                // if greater than the branching ratio float, then 
                // don't select this path, move on.
                //
                // BUT subtract the branching_ratio_float from the random_num_between_0_and_1
                random_num_between_0_and_1 -= branching_ratio_float;
            } else {
                // in this case, we want to select this branch 

                let target_nuclide = decay_data.target.unwrap();
                let decay_type = decay_data.decay_type;

                return Some((target_nuclide,decay_type));

            };

        }



        todo!("next rng code is buggy!");


    }


    // get next target using oorandom pseudorandom number generator
    // this is done along with decay type
    // from oorandom
    pub fn get_next_target_nuclide_with_float(&self, 
        mut random_num_between_0_and_1: f64)-> Option<(Nuclide, DecayType)> {

        // guard clause 

        if random_num_between_0_and_1 >= 1.0 || random_num_between_0_and_1 < 0.0 {

            panic!("random number is not between 0 and 1");
        }


        // let's obtain the branching ratios 
        let decay_branch_data = self.decay_information.clone();

        // let's do a case for 0 and 1 

        if decay_branch_data.len() == 0 {
            // this is no more nuclide, this is, the nuclide is stable 
            return None;

        }

        if decay_branch_data.len() == 1 {
            // this means radionuclide is unstable, but only one 
            // decay path
            let target_nuclide = decay_branch_data[0].target.unwrap();
            let decay_type = decay_branch_data[0].decay_type;

            return Some((target_nuclide,decay_type));
        }

        // then if we have more than one branch, we do the main code
        //
        // basically if we have a branching ratio of 0.6, 0.2 and 0.2 
        //
        // for decay branch 1, 2 and 3
        //
        // and our RNG produces 0.75, 
        //
        // we should be going for branch 2 
        //
        // |--------------|----|----|
        // 0.0          0.6   0.8   1.0
        //     branch 1     br 2  br3
        //
        // we can see 0.75 is in branch 2
        //
        // 
        // For this, the algorithm can be,
        //
        // suppose the branching ratio is 0.6 first 
        //
        // and 0.75 > 0.6, 
        //
        // we subtract 0.6 from 0.75 to get 0.15 
        //
        // the next branching ratio is 0.2 
        //
        // now, 0.15 < 0.2 
        //
        // so we select branch 2, and the target within branch 2
        //
        //


        for decay_data in decay_branch_data.iter() {
            // now, we check if the random number is greater than the 
            // branching ratio

            let branching_ratio_float = decay_data.branching_ratio.get::<ratio>();

            if random_num_between_0_and_1 > branching_ratio_float {
                // if greater than the branching ratio float, then 
                // don't select this path, move on.
                //
                // BUT subtract the branching_ratio_float from the random_num_between_0_and_1
                random_num_between_0_and_1 -= branching_ratio_float;
            } else {
                // in this case, we want to select this branch 

                let target_nuclide = decay_data.target.unwrap();
                let decay_type = decay_data.decay_type;

                return Some((target_nuclide,decay_type));

            };

        }



        todo!("code is buggy!");


    }


}



#[cfg(test)]
pub mod tests;
