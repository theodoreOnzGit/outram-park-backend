use fission_yields_data::prelude::{parse_nuclide_allow_underscore_isomer, Nuclide};
use uom::si::{energy::electronvolt, f64::*, ratio::ratio, time::second};

use crate::decay_xml_info_serde::SerdeNuclideData;


#[derive(Debug, PartialEq,Clone)]
pub struct NuclideReactionAndDecayData {
    // contains the nuclide of interest
    pub nuclide: Nuclide,
    // half life info 
    pub half_life_information: HalfLifeAndDecayEnergyInfo,
    // decay information 
    pub decay_information: Vec<DecayData>,
    

}
/// contains code to access decay information in an easier manner 
pub mod get_decay_info;

// contains information on whether or not there is half life, and how long 
// is it in seconds
#[derive(Debug, PartialEq,Clone)]
pub enum HalfLifeAndDecayEnergyInfo {
    // for stable nuclides
    Stable,
    // for unstable nuclides
    // obtain half life and decay Q value
    Unstable(Time, Energy),

}
// contains information on reaction data
#[derive(Debug, PartialEq,Clone)]
pub struct DecayData {
    pub decay_type: DecayType,
    pub target: Option<Nuclide>,
    pub branching_ratio: Ratio,

}
// contains information on reaction data
#[derive(Debug, PartialEq,Clone, Copy)]
pub enum DecayType {
    Alpha,
    ElectronCaptureBetaPlus,
    ElectronCaptureBetaPlusAndAlpha,
    BetaMinus,
    BetaMinusAndNeutron,
    BetaMinusAndTwoNeutron,
    BetaMinusAndThreeNeutron,
    BetaMinusAndFourNeutron,
    BetaMinusAndAlpha,
    DoubleBetaMinus,
    IsomericTransition,
    Proton,
    DoubleProton,
    ElectronCaptureBetaPlusAndProton,
    ElectronCaptureBetaPlusDoubleProton,
    SpontaneousFission,
    ElectronCaptureBetaPlusAndSpontaneousFission,
    Neutron,
    DoubleNeutron,

}

impl DecayType {

    pub fn parse_from_string(string: &str) -> Self {

        let decay_type: Self = match string {
            "alpha" => DecayType::Alpha,
            "betaminus" => DecayType::BetaMinus,
            "beta-" => DecayType::BetaMinus,
            "beta-,n" => DecayType::BetaMinusAndNeutron,
            "beta-,n,n" => DecayType::BetaMinusAndTwoNeutron,
            "beta-,n,n,n" => DecayType::BetaMinusAndThreeNeutron,
            "beta-,n,n,n,n" => DecayType::BetaMinusAndFourNeutron,
            "beta-,beta-" => DecayType::DoubleBetaMinus,
            "ec/beta+" => DecayType::ElectronCaptureBetaPlus,
            "ec/beta+,alpha" => DecayType::ElectronCaptureBetaPlusAndAlpha,
            "beta-,alpha" => DecayType::BetaMinusAndAlpha,
            "IT" => DecayType::ElectronCaptureBetaPlus,
            "p" => DecayType::Proton,
            "p,p" => DecayType::DoubleProton,
            "ec/beta+,p" => DecayType::ElectronCaptureBetaPlusAndProton,
            "ec/beta+,p,p" => DecayType::ElectronCaptureBetaPlusDoubleProton,
            "sf" => DecayType::SpontaneousFission,
            "ec/beta+,sf" => DecayType::ElectronCaptureBetaPlusAndSpontaneousFission,
            "n" => DecayType::Neutron,
            "n,n" => DecayType::DoubleNeutron,
            _ => {
                dbg!(&string);
                todo!("does not match any decay type")
            },
        };


        return decay_type;

    }
}

impl From<SerdeNuclideData> for NuclideReactionAndDecayData {
    fn from(raw_data_serde: SerdeNuclideData) -> Self {


        // first convert nuclide name to string 

        let mut nuclide_string: String = raw_data_serde.name;

        fn modify_isomer_string (nuclide_string: &mut String) {
            // this part modifies the isomer nuclides 
            // in the string from OpenMC style to my Nuclide style 
            //
            // I denote the first isomer as m 
            // OpenMC denotes it as m1 
            //
            // likewise, 
            //
            // OpenMC calls the second isomer m2 
            //
            // OpenMC => fission-yields-data library
            // m1 => m 
            // m2 => m1 
            // m3 => m2
            //
            //

            // first, if i detect m1, change it to m 

            if let Some(stripped) = nuclide_string.strip_suffix("m1") {
                *nuclide_string = format!("{stripped}m");
            }

            // second, if i detect m2, change it to m1
            if let Some(stripped) = nuclide_string.strip_suffix("m2") {
                *nuclide_string = format!("{stripped}m1");
            }

            // third, if i detect m3, change it to m2
            if let Some(stripped) = nuclide_string.strip_suffix("m3") {
                *nuclide_string = format!("{stripped}m2");
            }

            // third, if i detect m4, change it to m3
            if let Some(stripped) = nuclide_string.strip_suffix("m4") {
                *nuclide_string = format!("{stripped}m3");
            }

        }

        modify_isomer_string(&mut nuclide_string);



        let nuclide_enum: Nuclide = 
            parse_nuclide_allow_underscore_isomer(&nuclide_string)
            .unwrap();
        // next, parse half life info 
        // if there is no half life info, it will be a None enum,
        let half_life_seconds: Option<f64> = 
            raw_data_serde.half_life_seconds;

        // then decay types
        let mut decay_information: Vec<DecayData> = vec![];

        let half_life_information: HalfLifeAndDecayEnergyInfo = match half_life_seconds {
            None => HalfLifeAndDecayEnergyInfo::Stable,
            Some(half_life_seconds) => {


                let half_life = Time::new::<second>(half_life_seconds);
                // if there is a half life, there will surely be a decay energy
                let decay_energy = Energy::new::<electronvolt>(
                    raw_data_serde.decay_energy_electronvolt.unwrap()
                );

                // now lets do the decay information 
                // inclusive of decay types and branching ratio 
                // 
                // lets open the vector

                for decay_data in raw_data_serde.raw_decay_data {

                    // first deal with branching ratio
                    let branching_ratio = Ratio::new::<ratio>(
                        decay_data.branching_ratio
                    );
                    // then deal with the nuclide
                    let nuclide_string_option = decay_data.target;

                    let target_nuclide_option: Option<Nuclide> = match nuclide_string_option {
                        Some(mut nuclide_string) => {
                            modify_isomer_string(&mut nuclide_string);
                            let nuclide_enum: Nuclide = 
                                parse_nuclide_allow_underscore_isomer(&nuclide_string)
                                .unwrap();
                            Some(nuclide_enum)

                        },
                        None => None,
                    };

                    let decay_type: DecayType = 
                        DecayType::parse_from_string(&decay_data.decay_type);

                    let processed_decay_data: DecayData = 
                        DecayData { 
                            decay_type, 
                            target: target_nuclide_option, 
                            branching_ratio,
                        };

                    decay_information.push(processed_decay_data);


                };


                HalfLifeAndDecayEnergyInfo::Unstable(
                    half_life,decay_energy
                )
            },

        };
        // now for decay information, we need to check many decay modes we 
        // have, and match against the size of the vector 

        let decay_modes_option = raw_data_serde.decay_modes;
        // the number of decay modes is an option, 
        // so if the number is zero, then likely there is a none there 

        let num_of_decay_modes_reference: u32 = match decay_modes_option {
            Some(number_of_decay_modes) => number_of_decay_modes,
            None => 0,
        };

        // now lets get the size of the decay mode vector
        let num_of_decay_modes_test: u32 = decay_information
            .len().try_into().unwrap();

        assert_eq!(num_of_decay_modes_test,num_of_decay_modes_reference);
        

        // final step, finish the data
        let data = NuclideReactionAndDecayData {
            nuclide: nuclide_enum,
            half_life_information,
            decay_information,
        };

        return data;
    }
}



/// this contains tests for alkali metals and hydrogen
#[cfg(test)]
pub mod alkali_metals_and_hydrogen;

/// this contains tests for alkaline_earth metals 
#[cfg(test)]
pub mod alkaline_earth_metals;

/// this contains tests for transition metals
#[cfg(test)]
pub mod transition_metals_test;


/// this contains tests for noble gases
#[cfg(test)]
pub mod noble_gases_test;


/// this contains tests for halogens
#[cfg(test)]
pub mod halogens_test;


/// this contains tests for chalcogens
#[cfg(test)]
pub mod chalcogens_test;


/// this contains tests for pnictogens
#[cfg(test)]
pub mod pnictogens_test;


/// this contains tests for lanthanides
#[cfg(test)]
pub mod lanthanides_test;


/// this contains tests for actinides
#[cfg(test)]
pub mod actinides_test;

/// this contains tests for the carbon group 
#[cfg(test)]
pub mod carbon_group_test;

/// this contains tests for the boron group
#[cfg(test)]
pub mod boron_group_test;

/// this contains tests for heavier than actinides 
#[cfg(test)]
pub mod heavier_than_actinides;

/// contains modules to parse nuclides and obtain their respective xml data 
pub mod parse_nuclides_to_decay_data;

/// contains a module for a full decay library, which is meant to make it 
/// easy to obtain information based on the nuclide enum 
pub mod decay_library;


