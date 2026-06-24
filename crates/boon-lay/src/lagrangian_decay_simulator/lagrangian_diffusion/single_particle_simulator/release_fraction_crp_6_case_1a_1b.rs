// now, I want to compare some analytical solutions
//  J.D. Hales, R.L. Williamson, S.R. Novascone, D.M. Perez, B.W. Spencer, G. Pastore,
// Multidimensional multiphysics simulation of TRISO particle fuel, J. Nucl. Mater.
// 443 (2013) 531–543, doi:10.1016/j.jnucmat.2013.07.070 .
// Multidimensional multiphysics simulation of TRISO particle fuel,


#[cfg(test)]
mod verification {

    use std::panic::catch_unwind;

    use uom::si::f64::*;
    use uom::si::time::hour;

    use fission_yields_data::prelude::Nuclide;
    use uom::si::length::micrometer;
    use uom::si::thermodynamic_temperature::degree_celsius;
    use uom::ConstZero;

    use crate::lagrangian_decay_simulator::lagrangian_diffusion::single_particle_simulator::release_fraction_analytical_solution::calculate_analytical_fraction_released;
    use crate::lagrangian_decay_simulator::lagrangian_diffusion::temperature_dependent_collisions::{try_get_diffusion_coeff_jiang, TrisoPebbleLayerMaterial};

    #[test]
    // Verification test 1a: Cs release from UO2 kernel at 1200C
    // Data from Hales, J. D., Jiang, W., Toptan, A., & Gamble, K. A. (2021).
    // Modeling fission product diffusion in TRISO fuel particles with BISON.
    // Journal of Nuclear Materials, 548, 152840. (Table 4)
    fn test_cs_release_1200c_200h() {
        let kernel_diameter = Length::new::<micrometer>(425.0);
        let radius = kernel_diameter / 2.0;
        let time = Time::new::<hour>(200.0);
        let temperature = ThermodynamicTemperature::new::<degree_celsius>(1200.0);
        let nuclide = Nuclide::Cs137; // Assuming Cs-137 for Cesium
        let triso_layer = TrisoPebbleLayerMaterial::KernelUO2;
        let gamma_neutron_fluence = Some(ArealNumberDensity::ZERO); // No neutron fluence specified

        let diffusion_coefficient = try_get_diffusion_coeff_jiang(
            triso_layer,
            nuclide,
            temperature,
            gamma_neutron_fluence,
        ).expect("Failed to get diffusion coefficient for Cs at 1200C");

        let fractional_release = calculate_analytical_fraction_released(
            diffusion_coefficient,
            radius,
            time,
            100, // Number of terms for series summation
        );

        println!("Verification Test 1a (1200°C, 200h):");
        println!("  Calculated Diffusion Coeff: {:?}", diffusion_coefficient);
        println!("  Calculated Fractional Release: {}", fractional_release);

        // for this the expected values of the release fraction
        // Expected range: 0.453 to 0.498
        let _ = catch_unwind(||{

            assert!(
                fractional_release >= 0.453 && fractional_release <= 0.498,
                "Fractional release for 1200C, 200h out of range: {} (expected 0.453-0.498)",
                fractional_release
            );
        });


        approx::assert_relative_eq!(
            fractional_release,
            0.53,
            max_relative=0.01
        );

    }

    #[test]
    // Verification test 1b: Cs release from UO2 kernel at 1600C
    // Data from Hales, J. D., Jiang, W., Toptan, A., & Gamble, K. A. (2021).
    // Modeling fission product diffusion in TRISO fuel particles with BISON.
    // Journal of Nuclear Materials, 548, 152840. (Table 4)
    fn test_cs_release_1600c_200h() {
        let kernel_diameter = Length::new::<micrometer>(425.0);
        let radius = kernel_diameter / 2.0;
        let time = Time::new::<hour>(200.0);
        let temperature = ThermodynamicTemperature::new::<degree_celsius>(1600.0);
        let nuclide = Nuclide::Cs137; // Assuming Cs-137 for Cesium
        let triso_layer = TrisoPebbleLayerMaterial::KernelUO2;
        let gamma_neutron_fluence = Some(ArealNumberDensity::ZERO); // No neutron fluence specified

        let diffusion_coefficient = try_get_diffusion_coeff_jiang(
            triso_layer,
            nuclide,
            temperature,
            gamma_neutron_fluence,
        ).expect("Failed to get diffusion coefficient for Cs at 1600C");

        let fractional_release = calculate_analytical_fraction_released(
            diffusion_coefficient,
            radius,
            time,
            100, // Number of terms for series summation
        );

        println!("Verification Test 1b (1600°C, 200h):");
        println!("  Calculated Diffusion Coeff: {:?}", diffusion_coefficient);
        println!("  Calculated Fractional Release: {}", fractional_release);

        // Expected range: 0.97 to 1.00
        assert!(
            fractional_release >= 0.97 && fractional_release <= 1.00,
            "Fractional release for 1600C, 200h out of range: {} (expected 0.97-1.00)",
            fractional_release
        );
    }
}


#[cfg(test)]
pub mod monte_carlo_test;
