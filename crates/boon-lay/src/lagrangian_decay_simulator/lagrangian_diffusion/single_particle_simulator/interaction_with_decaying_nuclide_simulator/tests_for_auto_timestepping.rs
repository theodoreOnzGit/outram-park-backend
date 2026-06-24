use crate::lagrangian_decay_simulator::lagrangian_diffusion::temperature_dependent_collisions::{TrisoPebbleLayerMaterial, try_get_diffusion_coeff_jiang};


// If you already use the `approx` crate elsewhere, this is the nicest way:
// approx = "0.5"
use approx::assert_relative_eq;

use fission_yields_data::prelude::Nuclide;
// If you use `uom`, these are common imports. Adjust to match your project.
use uom::si::f64::*;
use uom::si::length::micrometer;
use uom::si::ratio::ratio;
use uom::si::thermodynamic_temperature::degree_celsius;
use uom::si::time::second;

/// the dimensionless number is pretty much the fourier number
#[test]
fn diffusion_fourier_number_calc(){

    // GIVEN
    let sic = TrisoPebbleLayerMaterial::SiC;
    let fuel = TrisoPebbleLayerMaterial::KernelUO2;
    let pyc = TrisoPebbleLayerMaterial::PyC;
    let buffer = TrisoPebbleLayerMaterial::Buffer;
    let nuclide = Nuclide::Cs137;

    // neutron fluence: 5.5e25 n/m^2
    // Adjust constructor/units to your ArealNumberDensity type.
    let fluence = ArealNumberDensity::new::<uom::si::areal_number_density::per_square_meter>(5.5e25);
    let gamma_neutron_fluence = Some(fluence);

    let temperature = ThermodynamicTemperature::new::<degree_celsius>(1600.0);

    // diffusion coeffs 

    let sic_diffusion_coeff = try_get_diffusion_coeff_jiang(
        sic, 
        nuclide, 
        temperature, 
        gamma_neutron_fluence).unwrap();

    let fuel_diffusion_coeff = try_get_diffusion_coeff_jiang(
        fuel, 
        nuclide, 
        temperature, 
        gamma_neutron_fluence).unwrap();

    let pyc_diffusion_coeff = try_get_diffusion_coeff_jiang(
        pyc, 
        nuclide, 
        temperature, 
        gamma_neutron_fluence).unwrap();

    let buffer_diffusion_coeff = try_get_diffusion_coeff_jiang(
        buffer, 
        nuclide, 
        temperature, 
        gamma_neutron_fluence).unwrap();


    // then lengthscales 
    // radius and layer thickness 

    let fuel_radius = 
        Length::new::<micrometer>(425.0 * 0.5);
    let buffer_thickness = 
        Length::new::<micrometer>(100.0);
    let ipyc_thickness = 
        Length::new::<micrometer>(40.0);
    let sic_thickness = 
        Length::new::<micrometer>(35.0);

    
    // timesteps 
    //
    let timestep_1s = Time::new::<second>(1.0);
    let timestep_10s = Time::new::<second>(10.0);

    // Fourier diffusion number = dimensionless number

    let fourier_number_fuel_1s: Ratio = 
        fuel_diffusion_coeff * timestep_1s / 
        fuel_radius / fuel_radius;

    let fourier_number_buffer_1s: Ratio = 
        buffer_diffusion_coeff * timestep_1s / 
        buffer_thickness / buffer_thickness;

    let fourier_number_sic_1s: Ratio = 
        sic_diffusion_coeff * timestep_1s / 
        sic_thickness / sic_thickness;

    let fourier_number_ipyc_1s: Ratio = 
        pyc_diffusion_coeff * timestep_1s / 
        ipyc_thickness / ipyc_thickness;

    // 10s
    let fourier_number_fuel_10s: Ratio = 
        fuel_diffusion_coeff * timestep_10s / 
        fuel_radius / fuel_radius;

    let fourier_number_buffer_10s: Ratio = 
        buffer_diffusion_coeff * timestep_10s / 
        buffer_thickness / buffer_thickness;

    let fourier_number_sic_10s: Ratio = 
        sic_diffusion_coeff * timestep_10s / 
        sic_thickness / sic_thickness;

    let fourier_number_ipyc_10s: Ratio = 
        pyc_diffusion_coeff * timestep_10s / 
        ipyc_thickness / ipyc_thickness;

    // these are the fourier numbers  for fuel and buffer
    //
    // at 1s

    assert_relative_eq!(
        fourier_number_fuel_1s.get::<ratio>(),
        2.7688e-6,
        max_relative=1e-3
    );

    assert_relative_eq!(
        fourier_number_buffer_1s.get::<ratio>(),
        1.00,
        max_relative=1e-3
    );
    assert_relative_eq!(
        fourier_number_sic_1s.get::<ratio>(),
        1.0986e-7,
        max_relative=1e-3
    );
    assert_relative_eq!(
        fourier_number_ipyc_1s.get::<ratio>(),
        2.5389e-5,
        max_relative=1e-3
    );

    // these are the fourier numbers at 10s

    assert_relative_eq!(
        fourier_number_fuel_10s.get::<ratio>(),
        2.7688e-5,
        max_relative=1e-3
    );

    assert_relative_eq!(
        fourier_number_buffer_10s.get::<ratio>(),
        10.00,
        max_relative=1e-3
    );
    assert_relative_eq!(
        fourier_number_sic_10s.get::<ratio>(),
        1.0986e-6,
        max_relative=1e-3
    );
    assert_relative_eq!(
        fourier_number_ipyc_10s.get::<ratio>(),
        2.5389e-4,
        max_relative=1e-3
    );

    // conjecture, if fourier number greater than 1e-5, it is best 
    // to subtimestep
    //
    // note: NOT AI generated

}
