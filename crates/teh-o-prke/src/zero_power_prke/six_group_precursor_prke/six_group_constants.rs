use std::f64::consts::LN_2;

use uom::ConstZero;
use uom::si::f64::*;
use uom::si::time::second;
use uom::si::ratio::ratio;

use crate::zero_power_prke::six_group_precursor_prke::DecayConstant;

/// different nuclides or fuels have different delayed groups
#[derive(Debug,Clone,Copy)]
pub enum FissioningNuclideType {
    /// chooses the U233 group of delayed constants
    U233,
    /// chooses the U235 group of delayed constants
    U235,
    /// chooses the Pu239 group of delayed constants
    Pu239
}

impl FissioningNuclideType {
    /// returns a new decay constant array based on nuclide
    /// (for this version, all decay constant arrays are the same)
    pub fn get_decay_constant_array(&self) -> [DecayConstant;6] {
        match self {
            FissioningNuclideType::U233 => {
                return new_decay_constant_array();
            },
            FissioningNuclideType::U235 => {
                return new_decay_constant_array();
            },
            FissioningNuclideType::Pu239 => {
                return new_decay_constant_array();
            },
        }
    }

    /// returns a delayed fraction array based on nuclide 
    pub fn get_delayed_fraction_array(&self) -> [Ratio;6] {
        match self {
            FissioningNuclideType::U233 => {
                return new_u233_delayed_neutron_fraction_array();
            },
            FissioningNuclideType::U235 => {
                return new_u235_delayed_neutron_fraction_array();
            },
            FissioningNuclideType::Pu239 => {
                return new_pu239_delayed_neutron_fraction_array();
            },
        }
    }
}


/// produces a new decay constant array
pub fn new_decay_constant_array() -> [DecayConstant;6] {

    // from table 5.1 (not sure where from, duderstadt?)
    // need to double check
    let mut decay_constant_array: [DecayConstant;6] = 
        [
        Frequency::ZERO,
        Frequency::ZERO,
        Frequency::ZERO,
        Frequency::ZERO,
        Frequency::ZERO,
        Frequency::ZERO,
        ];

    let half_life_array: [Time;6] = 
        [
        Time::new::<second>(56.0),
        Time::new::<second>(23.0),
        Time::new::<second>(6.2),
        Time::new::<second>(2.3),
        Time::new::<second>(0.61),
        Time::new::<second>(0.23),
        ];

    // crude, but gets the job done
    //
    // lambda (decay constant) = ln(2)/(half life)
    decay_constant_array[0] = LN_2/half_life_array[0];
    decay_constant_array[1] = LN_2/half_life_array[1];
    decay_constant_array[2] = LN_2/half_life_array[2];
    decay_constant_array[3] = LN_2/half_life_array[3];
    decay_constant_array[4] = LN_2/half_life_array[4];
    decay_constant_array[5] = LN_2/half_life_array[5];

    // done!
    decay_constant_array
        

}

/// produces a new delayed fraction for u233 
pub fn new_u233_delayed_neutron_fraction_array() -> [Ratio;6] {

    let delayed_neutron_array: [Ratio;6] = 
        [
        Ratio::new::<ratio>(0.00023),
        Ratio::new::<ratio>(0.00078),
        Ratio::new::<ratio>(0.00064),
        Ratio::new::<ratio>(0.00074),
        Ratio::new::<ratio>(0.00014),
        Ratio::new::<ratio>(0.00008),
        ];

    delayed_neutron_array
}

/// produces a new delayed fraction for u235 
pub fn new_u235_delayed_neutron_fraction_array() -> [Ratio;6] {

    let delayed_neutron_array: [Ratio;6] = 
        [
        Ratio::new::<ratio>(0.00021),
        Ratio::new::<ratio>(0.00142),
        Ratio::new::<ratio>(0.00128),
        Ratio::new::<ratio>(0.00257),
        Ratio::new::<ratio>(0.00075),
        Ratio::new::<ratio>(0.00027),
        ];

    delayed_neutron_array
}


/// produces a new delayed fraction for pu239 
pub fn new_pu239_delayed_neutron_fraction_array() -> [Ratio;6] {

    let delayed_neutron_array: [Ratio;6] = 
        [
        Ratio::new::<ratio>(0.00007),
        Ratio::new::<ratio>(0.00063),
        Ratio::new::<ratio>(0.00044),
        Ratio::new::<ratio>(0.00069),
        Ratio::new::<ratio>(0.00018),
        Ratio::new::<ratio>(0.00009),
        ];

    delayed_neutron_array
}

