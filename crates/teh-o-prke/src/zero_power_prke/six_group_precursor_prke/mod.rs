use uom::ConstZero;
use uom::si::f64::*;
use uom::si::volumetric_number_density::per_cubic_meter;
use uom::si::ratio::ratio;

/// contains six group delayed precursor decay constants and 
/// delayed fraction
pub mod six_group_constants;
use six_group_constants::FissioningNuclideType;

/// Decay Constant is essentially the same units as frequency
pub type DecayConstant = Frequency;
/// SixGroupPRKE
#[derive(Debug,Clone,Copy)]
pub struct SixGroupPRKE {
    /// contains an array for the various half lives 
    /// of the delayed precursors
    pub decay_constant_array: [DecayConstant;6],
    /// contains delayed fraction arrays for the delayed precursors 
    /// this is different for u235, u233 and Pu239
    pub delayed_fraction_array: [Ratio;6],
    /// determines the set of delayed group constants based on your choice 
    /// of fissile isotope
    pub delayed_group_mode: FissioningNuclideType,

    /// precursor_and_neutron_pop_and_source_array 
    pub precursor_and_neutron_pop_and_source_array: [VolumetricNumberDensity;7],
}




/// default is to use u235 decay constants and delayed fraction, with 
/// starting neutron population of 1 per m3
impl Default for SixGroupPRKE {
    fn default() -> Self {
        let delayed_group_mode = FissioningNuclideType::U235;

        // the arrangement is 
        // [precursor grp 1, 
        // precursor grp 2, 
        // precursor grp 3, 
        // precursor grp 4, 
        // precursor grp 5, 
        // precursor grp 6,
        // neutron population]


        let precursor_and_neutron_pop_and_source_array: [VolumetricNumberDensity;7] = 
            [
            VolumetricNumberDensity::new::<per_cubic_meter>(1.0) ,
            VolumetricNumberDensity::ZERO,
            VolumetricNumberDensity::ZERO,
            VolumetricNumberDensity::ZERO,
            VolumetricNumberDensity::ZERO,
            VolumetricNumberDensity::ZERO,
            VolumetricNumberDensity::ZERO,
            ];
        let decay_constant_array = delayed_group_mode.get_decay_constant_array();
        let delayed_fraction_array = delayed_group_mode.get_delayed_fraction_array();

        Self {
            decay_constant_array,
            delayed_fraction_array,
            delayed_group_mode,
            precursor_and_neutron_pop_and_source_array,
        }

    }
}

/// contains time stepping implicit solvers for SixGroupPRKE
pub mod implicit_solver;

/// contains time stepping explicit solvers for SixGroupPRKE 
pub mod explicit_solver;

impl SixGroupPRKE {

    /// obtains current neutron population 
    pub fn get_current_neutron_population_density(&self) -> VolumetricNumberDensity {
        self.precursor_and_neutron_pop_and_source_array[0]
    }

    /// total delayed fraction 
    pub fn get_total_delayed_fraction(&self) -> Ratio {

        let total_delayed_fraction: Ratio = 
            self.delayed_fraction_array.clone().into_iter().sum();

        total_delayed_fraction
    }


    /// enables you to convert reactivity into keff, useful for calculating 
    /// the neutron generation time
    pub fn get_keff_from_reactivity(reactivity: Ratio) -> Ratio {

        // reactivity is rho 
        //
        // rho = (k-1)/k
        //
        // k * rho = k - 1
        // k * rho - k = - 1
        // k - k * rho = 1
        // k * (1 - rho) = 1
        // k = 1/(1 - rho) 
        //

        let ratio_one = Ratio::new::<ratio>(1.0);

        let keff = ratio_one/(ratio_one - reactivity);

        keff

    }
    
}


