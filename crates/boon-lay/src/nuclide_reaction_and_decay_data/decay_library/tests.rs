use fission_yields_data::prelude::Nuclide;

use crate::prelude::{decay_library::DecayLibrary, NuclideReactionAndDecayData};
use uom::si::{f64::*, time::year};

#[test]
fn index_tritium(){

    let tritium = Nuclide::H3;

    let nuclide_library = DecayLibrary::new();

    let tritium_decay_nuclide: NuclideReactionAndDecayData = 
        nuclide_library.try_match_nuclides_to_decay_data(tritium).unwrap();

    assert_eq!(tritium_decay_nuclide.nuclide,tritium);

    // obtain half life 

    let half_life: Time = match tritium_decay_nuclide.half_life_information {
        crate::prelude::HalfLifeAndDecayEnergyInfo::Stable => todo!(),
        crate::prelude::HalfLifeAndDecayEnergyInfo::Unstable(tritium_half_life, _decay_energy) => {
            tritium_half_life
        },
    };

    // assert if its equal to 12.3 y 

    let half_life_tritium_years = (half_life.get::<year>()*10.0).round()/10.0;

    assert_eq!(12.3, half_life_tritium_years);

    // assert that decay only has one path 

    assert_eq!(tritium_decay_nuclide.decay_information.len(),1);

    // assert if the target is helium 3 

    let decay_data = tritium_decay_nuclide.decay_information[0].clone();

    let decay_target = decay_data.target.unwrap();


    assert_eq!(decay_target, Nuclide::He3);


}
