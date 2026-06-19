use uom::si::volumetric_number_density::per_cubic_meter;
use uom::si::{f64::*, ratio::ratio};
use uom::ConstZero;

use crate::zero_power_prke::six_group_precursor_prke::SixGroupPRKE;
use crate::zero_power_prke::six_group_precursor_prke::six_group_constants::new_u235_delayed_neutron_fraction_array;
use crate::zero_power_prke::six_group_precursor_prke::six_group_constants::new_u233_delayed_neutron_fraction_array;
use crate::zero_power_prke::six_group_precursor_prke::six_group_constants::new_pu239_delayed_neutron_fraction_array;
use crate::zero_power_prke::six_group_precursor_prke::six_group_constants::FissioningNuclideType;
#[test]
pub fn test_pu239_total_delayed_frac(){

    let total_delayed_fraction_reference = Ratio::new::<ratio>(0.00210);
    
    let pu239_delayed_frac_array = new_pu239_delayed_neutron_fraction_array();

    let test_delayed_frac: Ratio = pu239_delayed_frac_array.into_iter().sum();

    assert_eq!(test_delayed_frac,total_delayed_fraction_reference)


}

#[test]
pub fn test_u233_total_delayed_frac(){

    let total_delayed_fraction_reference = Ratio::new::<ratio>(0.00261);
    
    let u233_delayed_frac_array = new_u233_delayed_neutron_fraction_array();

    let test_delayed_frac: Ratio = u233_delayed_frac_array.into_iter().sum();

    assert_eq!(test_delayed_frac,total_delayed_fraction_reference)


}

#[test]
pub fn test_u235_total_delayed_frac(){

    use approx::*;
    let total_delayed_fraction_reference = Ratio::new::<ratio>(0.00650);
    
    let u235_delayed_frac_array = new_u235_delayed_neutron_fraction_array();

    let test_delayed_frac: Ratio = u235_delayed_frac_array.into_iter().sum();

    assert_abs_diff_eq!(
        test_delayed_frac.get::<ratio>(), 
        total_delayed_fraction_reference.get::<ratio>(), 
        epsilon = f64::EPSILON);


}

#[test]
pub fn prke_test_zero_reactivity(){
    // let's make a new six group PRKE struct,
    // with zero reactivity and zero new source 
    // we should get a constant neutron pop 

    use uom::si::time::{millisecond, nanosecond};
    use approx::assert_abs_diff_eq;
    let delayed_group_mode = FissioningNuclideType::U235;


    let precursor_and_neutron_pop_and_source_array: [VolumetricNumberDensity;7] = 
        [
        VolumetricNumberDensity::new::<per_cubic_meter>(1.0),
        VolumetricNumberDensity::ZERO,
        VolumetricNumberDensity::ZERO,
        VolumetricNumberDensity::ZERO,
        VolumetricNumberDensity::ZERO,
        VolumetricNumberDensity::ZERO,
        VolumetricNumberDensity::ZERO,
        ];
    let decay_constant_array = delayed_group_mode.get_decay_constant_array();
    let delayed_fraction_array = delayed_group_mode.get_delayed_fraction_array();

    let mut prke_test = SixGroupPRKE {
        decay_constant_array,
        delayed_fraction_array,
        delayed_group_mode,
        precursor_and_neutron_pop_and_source_array,
    };

    let timestep = Time::new::<millisecond>(50.0);
    let neutron_generation_time = Time::new::<nanosecond>(10.0);
    let zero_reactivity = Ratio::ZERO;
    let background_source_rate = VolumetricNumberRate::ZERO;

    // now before running a timestep, we should get a neutron pop of 1 per m3
    let initial_neutron_pop_float = prke_test.get_current_neutron_population_density().
        get::<per_cubic_meter>();

    assert_eq!(initial_neutron_pop_float,1.0);

    // now let's run a timestep with zero reactivity

    let number_of_timesteps = 10000;

    for _ in 0..number_of_timesteps {

        prke_test.solve_next_timestep_precursor_concentration_and_neutron_pop_vector_implicit(
            timestep, 
            zero_reactivity, 
            neutron_generation_time, 
            background_source_rate).unwrap();
    }


    // makes sense that we dont add to one because of the delayed precursors 
    //
    // however, the sum of all precursor concentrations and neutron 
    // populations should be 1.0

    let precursor_sum_with_neutron_pop_array = 
        prke_test.solve_next_timestep_precursor_concentration_and_neutron_pop_vector_implicit(
            timestep, 
            zero_reactivity, 
            neutron_generation_time, 
            background_source_rate).unwrap();

    let precursor_and_neutron_pop_sum: VolumetricNumberDensity
        = precursor_sum_with_neutron_pop_array.into_iter().sum();


    assert_abs_diff_eq!(
        precursor_and_neutron_pop_sum.get::<per_cubic_meter>(),
        1.0,
        epsilon = 1e-9);

    // now the neutron population after 50 ms * 10000 timesteps is:
    assert_abs_diff_eq!(
        prke_test.get_current_neutron_population_density().value,
        1.17917639e-7,
        epsilon = 1e-11);

    // if critical, it should not change after next 500 s

    for _ in 0..number_of_timesteps {

        prke_test.solve_next_timestep_precursor_concentration_and_neutron_pop_vector_implicit(
            timestep, 
            zero_reactivity, 
            neutron_generation_time, 
            background_source_rate).unwrap();
    }

    // now the neutron population after another 50 ms * 10000 timesteps is:
    // or else 500s should be the same
    assert_abs_diff_eq!(
        prke_test.get_current_neutron_population_density().value,
        1.17917639e-7,
        epsilon = 1e-11);
}
