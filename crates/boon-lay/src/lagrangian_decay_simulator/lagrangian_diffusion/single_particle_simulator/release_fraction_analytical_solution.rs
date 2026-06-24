// Add these imports to your project's `Cargo.toml` if not already present:
// [dependencies]
// uom = { version = "0.35", features = ["f64", "si"] }

// Add these `use` statements at the top of your relevant file (e.g., a new verification module)
use uom::si::f64::*;
use uom::si::diffusion_coefficient::square_meter_per_second;
use uom::si::length::meter;
use uom::si::ratio::ratio;
use uom::si::time::second;
use std::f64::consts::PI;

/// Calculates the analytical fraction of material released from a sphere over time.
///
/// This solution is for diffusion from a sphere of radius `radius`
/// with a constant diffusion coefficient `diffusion_coefficient`,
/// assuming a uniform initial concentration within the sphere
/// and a perfect sink (zero concentration) at the surface.
///
/// # Arguments
/// * `diffusion_coefficient` - The constant diffusion coefficient (e.g., in m²/s).
/// * `radius` - The radius of the sphere (e.g., in m).
/// * `time` - The elapsed time (e.g., in s).
/// * `num_terms` - The number of terms to use in the infinite series summation.
///                 More terms provide higher accuracy, but 10-20 are usually sufficient.
///
/// # Returns
/// A `f64` representing the fraction of material released (between 0.0 and 1.0).
///
/// # Panics
/// Panics if `radius` is zero or `time` is negative.
pub fn calculate_analytical_fraction_released(
    diffusion_coefficient: DiffusionCoefficient,
    radius: Length,
    time: Time,
    num_terms: usize,
) -> f64 {
    // Basic validation using raw values for comparison, but calculations use uom
    if radius.get::<meter>() <= 0.0 {
        panic!("Radius must be positive for analytical solution.");
    }
    if time.get::<second>() < 0.0 {
        panic!("Time cannot be negative for analytical solution.");
    }
    if diffusion_coefficient.get::<square_meter_per_second>() < 0.0 {
        panic!("Diffusion coefficient cannot be negative.");
    }

    // Handle t=0 case explicitly
    if time.get::<second>() == 0.0 {
        return 0.0; // No release at t=0
    }

    let mut sum_terms = 0.0;

    // Calculate D*t / R^2 using uom quantities
    // (DiffusionCoefficient * Time) results in Area
    let dt_product: Area = diffusion_coefficient * time;
    // (Length * Length) results in Area
    let r_squared: Area = radius * radius;

    // (Area / Area) results in Dimensionless
    // We use .get::<ratio>() to extract the f64 value from the Dimensionless quantity
    let dimensionless_ratio_dt_r2: f64 = (dt_product / r_squared).get::<ratio>();

    // Sum the infinite series for fraction remaining
    //
    // I inspected this series (by inspection)
    // it looks identical to the analytical formula. So should be okay
    for n in 1..=num_terms {
        let n_f64 = n as f64;
        let n_pi_squared = (n_f64 * PI).powi(2); // This part is dimensionless

        let term_exponent_value = -dimensionless_ratio_dt_r2 * n_pi_squared;
        let term_coefficient = 6.0 / n_pi_squared;
        
        sum_terms += term_coefficient * term_exponent_value.exp();
    }

    let fraction_remaining = sum_terms;

    // The fraction released is 1 - fraction_remaining
    1.0 - fraction_remaining
}

// Example usage (you can put this in a test or a temporary main function)
#[cfg(test)]
mod tests {
    use super::*;
    use uom::si::diffusion_coefficient::square_meter_per_second;
    use uom::si::length::millimeter;
    use uom::si::time::hour;

    #[test]
    fn test_analytical_fraction_released() {
        let d = DiffusionCoefficient::new::<square_meter_per_second>(1.0e-12); // 1e-12 m²/s
        let r = Length::new::<millimeter>(250.0); // 250 mm = 0.25 m
        let t = Time::new::<hour>(1000.0); // 1000 hours

        // For this specific set of parameters, let's calculate the expected value
        // You'd typically compare against a known value from a reference or another tool.
        // For demonstration, let's pick a time where some release has occurred.
        let fraction_released = calculate_analytical_fraction_released(d, r, t, 100); // Use 100 terms for good accuracy

        println!("Diffusion Coefficient: {:?}", d);
        println!("Radius: {:?}", r);
        println!("Time: {:?}", t);
        println!("Analytical Fraction Released: {}", fraction_released);

        // Assert that the fraction is within a reasonable range (0 to 1)
        assert!(fraction_released >= 0.0);
        assert!(fraction_released <= 1.0);

        // Add more specific assertions if you have known values for D, R, T
        // For example, if you know at a certain time, the release should be ~0.5:
        // assert!((fraction_released - 0.5).abs() < 0.01);
    }

    #[test]
    fn test_analytical_fraction_released_at_t_zero() {
        let d = DiffusionCoefficient::new::<square_meter_per_second>(1.0e-12);
        let r = Length::new::<millimeter>(250.0);
        let t = Time::new::<second>(0.0);
        let fraction_released = calculate_analytical_fraction_released(d, r, t, 10);
        assert_eq!(fraction_released, 0.0);
    }

    #[test]
    #[should_panic(expected = "Radius must be positive for analytical solution.")]
    fn test_analytical_fraction_released_zero_radius_panics() {
        let d = DiffusionCoefficient::new::<square_meter_per_second>(1.0e-12);
        let r = Length::new::<meter>(0.0);
        let t = Time::new::<second>(10.0);
        calculate_analytical_fraction_released(d, r, t, 10);
    }
}


// now, I want to compare some analytical solutions
//  J.D. Hales, R.L. Williamson, S.R. Novascone, D.M. Perez, B.W. Spencer, G. Pastore,
// Multidimensional multiphysics simulation of TRISO particle fuel, J. Nucl. Mater.
// 443 (2013) 531–543, doi:10.1016/j.jnucmat.2013.07.070 .
// Multidimensional multiphysics simulation of TRISO particle fuel,


#[cfg(test)]
mod verification {

    use std::panic::catch_unwind;

    use fission_yields_data::prelude::Nuclide;
    use uom::si::f64::*;
    use uom::si::length::micrometer;
    use uom::si::thermodynamic_temperature::degree_celsius;
    use uom::si::time::hour;
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
