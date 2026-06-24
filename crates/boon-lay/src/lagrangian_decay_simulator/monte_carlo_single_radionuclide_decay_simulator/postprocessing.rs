use fission_yields_data::prelude::Nuclide;

use crate::prelude::SingleNuclideSimulatorMC;

impl SingleNuclideSimulatorMC {
    // this is vibe coded 
    // obtains a unique list of nuclides  
    // within the decay chain for the simulator

    // Unique nuclides from a single simulator (sorted by Z,A; order not preserved)
    pub fn chain_nuclides_unique_sorted(&self) -> Vec<Nuclide> {
        let mut v: Vec<(Nuclide, (u32, u32))> = self
            .stochastic_decay_chain
            .nuclides_and_decay_data_vec
            .iter()
            .map(|(n, _)| (*n, n.get_z_a()))
            .collect();

        v.sort_by_key(|&(_, key)| key);
        v.dedup_by_key(|&mut (_, key)| key);

        // I had to make an edition because it was not including the  
        // current nuclide
        // then add the existing nuclide 
        let mut unique_nuclide_vector: Vec<Nuclide> = 
            v.into_iter().map(|(n, _)| n).collect();
        unique_nuclide_vector.push(self.current_nuclide);

        unique_nuclide_vector

    }

    // Unique nuclides across multiple simulators (sorted by Z,A; order not preserved)
    pub fn all_chain_nuclides_unique_sorted(sims: &[SingleNuclideSimulatorMC]) -> Vec<Nuclide> {
        let mut v: Vec<(Nuclide, (u32, u32))> = Vec::new();
        for sim in sims {
            for &(n, _) in &sim.stochastic_decay_chain.nuclides_and_decay_data_vec {
                v.push((n, n.get_z_a()));
                // need to get the current nuclide also otherwise 
                // only plotted products
                //
                // this is rather inefficient, but okay lah
                v.push((sim.current_nuclide, sim.current_nuclide.get_z_a()));
            }
        }

        // we also need to get the existing nuclide (missed by ChatGPT5)
        

        v.sort_by_key(|&(_, key)| key);
        v.dedup_by_key(|&mut (_, key)| key);
        v.into_iter().map(|(n, _)| n).collect()
    }

    // this wasn't vibe coded
    // chatgpt didn't give the correct one
    //
    // however, it did give a useful initial template
    pub fn count_nuclides_in_sims_linear(
        sims: &[SingleNuclideSimulatorMC],
        unique: &[Nuclide],
    ) -> Vec<(Nuclide, u64)> {
        // Initialize counts aligned to `unique`
        let mut nuclide_counter_vec: Vec<(Nuclide, u64)> = unique.iter().cloned().map(|n| (n, 0)).collect();
        // this counts starts everything at zero

        // so we have to iterate over every simulator 

        for sim in sims {

            let current_nuclide: Nuclide = sim.current_nuclide;

            // then let's compare it to the nuclide 
            for (nuclide_to_check, current_count) in nuclide_counter_vec.iter_mut() {

                if current_nuclide == *nuclide_to_check {
                    *current_count += 1;
                }

            }



        }


        nuclide_counter_vec
    }
}
