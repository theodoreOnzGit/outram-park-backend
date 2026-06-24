use crate::lagrangian_decay_simulator::lagrangian_diffusion::temperature_dependent_collisions::{TrisoPebbleLayerMaterial, try_get_diffusion_coeff_jiang};


// If you already use the `approx` crate elsewhere, this is the nicest way:
// approx = "0.5"
use approx::assert_relative_eq;

use fission_yields_data::prelude::Nuclide;
// If you use `uom`, these are common imports. Adjust to match your project.
use uom::si::f64::*;
use uom::si::thermodynamic_temperature::kelvin;

// Adjust these to your actual constructors / enums.
// The test assumes:
// - you want Cs in SiC
// - neutron fluence is supplied as ArealNumberDensity in n/m^2
#[test]
fn test_diffusion_coeff_jiang_matches_tabulated_cs_in_sic() {
    // GIVEN
    let triso_layer = TrisoPebbleLayerMaterial::SiC;
    let nuclide = Nuclide::Cs137;

    // neutron fluence: 5.5e25 n/m^2
    // Adjust constructor/units to your ArealNumberDensity type.
    let fluence = ArealNumberDensity::new::<uom::si::areal_number_density::per_square_meter>(5.5e25);
    let gamma_neutron_fluence = Some(fluence);

    // (T [K], log10(D [m^2/s]))
    let data: &[(f64, f64)] = &[
        (608.9744, -23.4219),
        (649.3590, -22.7841),
        (696.4744, -22.1860),
        (743.5897, -21.5880),
        (795.1923, -20.8704),
        (869.2308, -20.1927),
        (952.2436, -19.5150),
        (1048.7179, -18.9967),
        (1147.4359, -18.3987),
        (1259.6154, -17.8804),
        (1324.6795, -17.6013),
        (1403.2051, -17.4419),
        (1470.5128, -17.1229),
        (1535.5769, -17.0033),
        (1584.9359, -16.8837),
        (1688.1410, -16.6445),
        (1744.2308, -16.3654),
        (1811.5385, -16.1262),
        (1883.3333, -15.8472),
        (1950.6410, -15.4086),
        // note that for these values, a larger error 
        // bound is required as the logscale gets bigger
        (2017.9487, -14.9302),
        (2087.5000, -14.6910),
        ];

    // THEN
    // Tolerance: choose something appropriate for your implementation
    // (e.g., regression fit, interpolation, or piecewise model).
    // Here we allow 2% relative error.
    let rtol = 0.02;

    for &(t_k, log10_d) in data {
        let temperature = ThermodynamicTemperature::new::<kelvin>(t_k);

        let got = try_get_diffusion_coeff_jiang(
            triso_layer,
            nuclide,
            temperature,
            gamma_neutron_fluence,
        )
            .unwrap_or_else(|| panic!("Expected Some(D) at T={t_k} K, got None"));

        dbg!(&temperature);
        // expected D in m^2/s
        let expected_d_m2_s = 10f64.powf(log10_d);

        // Convert `got` to f64 in m^2/s. Adjust accessor to match your type.
        // Common patterns:
        // - got.get::<diffusion_coefficient::square_meter_per_second>()
        // - got.value
        // - f64::from(got)
        let got_d_m2_s = got.get::<uom::si::diffusion_coefficient::square_meter_per_second>();

        if t_k > 2000.0 {

            // larger tolerance for higher temperatures
            let rtol = 0.40;
            assert_relative_eq!(
                got_d_m2_s,
                expected_d_m2_s,
                max_relative=rtol,
            );

            continue;
        }


        assert_relative_eq!(
            got_d_m2_s,
            expected_d_m2_s,
            max_relative=rtol,
        );
    }
}

#[test]
fn test_diffusion_coeff_jiang_matches_tabulated_cs_in_pyc() {
    // GIVEN
    let triso_layer = TrisoPebbleLayerMaterial::PyC; // <- change from SiC to PyC
    let nuclide = Nuclide::Cs137;

    // neutron fluence: 5.5e25 n/m^2
    let fluence = ArealNumberDensity::new::<uom::si::areal_number_density::per_square_meter>(5.5e25);
    let gamma_neutron_fluence = Some(fluence);

    // (T [K], log10(D [m^2/s])) for Cs in PyC
    let data: &[(f64, f64)] = &[
        (608.9744, -26.1329),
        (624.6795, -25.7741),
        (642.6282, -25.2957),
        (671.7949, -24.4585),
        (698.7179, -23.8605),
        (725.6410, -23.1429),
        (768.2692, -22.1860),
        (801.9231, -21.7076),
        (833.3333, -21.1894),
        (884.9359, -20.1927),
        (932.0513, -19.5947),
        (1006.0897, -18.5980),
        (1071.1538, -17.9601),
        (1140.7051, -17.3223),
        (1230.4487, -16.6445),
        (1317.9487, -16.0465),
        (1391.9872, -15.4485),
        (1479.4872, -15.0498),
        (1551.2821, -14.6512),
        (1629.8077, -14.3721),
        (1697.1154, -13.9734),
        (1764.4231, -13.7342),
        (1840.7051, -13.5349),
        (1901.2821, -13.2558),
        (1968.5897, -13.0565),
        (2024.6795, -12.8173),
        (2076.2821, -12.7375),
        ];

    // THEN
    // Tolerance: adjust as needed for your PyC implementation
    let rtol = 0.02;

    for &(t_k, log10_d) in data {
        let temperature = ThermodynamicTemperature::new::<kelvin>(t_k);

        let got = try_get_diffusion_coeff_jiang(
            triso_layer,
            nuclide,
            temperature,
            gamma_neutron_fluence,
        )
            .unwrap_or_else(|| panic!("Expected Some(D) at T={t_k} K, got None"));

        // expected D in m^2/s
        let expected_d_m2_s = 10f64.powf(log10_d);

        let got_d_m2_s = got.get::<uom::si::diffusion_coefficient::square_meter_per_second>();

        dbg!(&temperature);
        if t_k > 1629.0 {

            // larger tolerance for higher temperatures
            let rtol = 0.25;
            assert_relative_eq!(
                got_d_m2_s,
                expected_d_m2_s,
                max_relative=rtol,
            );

            continue;
        }
        assert_relative_eq!(
            got_d_m2_s,
            expected_d_m2_s,
            max_relative = rtol,
        );
    }
}

#[test]
fn test_diffusion_coeff_jiang_matches_tabulated_cs_in_kernel() {
    // GIVEN
    let triso_layer = TrisoPebbleLayerMaterial::KernelUO2; // <- kernel material/layer
    let nuclide = Nuclide::Cs137;

    // neutron fluence: 5.5e25 n/m^2
    let fluence =
        ArealNumberDensity::new::<uom::si::areal_number_density::per_square_meter>(5.5e25);
    let gamma_neutron_fluence = Some(fluence);

    // (T [K], log10(D [m^2/s])) for Cs in TRISO kernel
    let data: &[(f64, f64)] = &[
        (617.9487, -24.7774),
        (642.6282, -24.2591),
        (667.3077, -23.4618),
        (703.2051, -22.7442),
        (736.8590, -22.0266),
        (777.2436, -21.2691),
        (822.1154, -20.4718),
        (855.7692, -19.9934),
        (911.8590, -19.1163),
        (974.6795, -18.5183),
        (1012.8205, -17.8804),
        (1091.3462, -17.1628),
        (1145.1923, -16.6445),
        (1221.4744, -16.0465),
        (1295.5128, -15.5282),
        (1380.7692, -15.1296),
        (1466.0256, -14.6512),
        (1551.2821, -14.1728),
        (1629.8077, -13.7741),
        (1703.8462, -13.4551),
        (1766.6667, -13.1761),
        (1831.7308, -12.9767),
        (1901.2821, -12.6578),
        (1977.5641, -12.3787),
        (2053.8462, -12.1794),
    ];

    // THEN
    let rtol = 0.02;

    for &(t_k, log10_d) in data {
        let temperature = ThermodynamicTemperature::new::<kelvin>(t_k);

        let got = try_get_diffusion_coeff_jiang(
            triso_layer,
            nuclide,
            temperature,
            gamma_neutron_fluence,
        )
        .unwrap_or_else(|| panic!("Expected Some(D) at T={t_k} K, got None"));

        let expected_d_m2_s = 10f64.powf(log10_d);
        let got_d_m2_s = got.get::<uom::si::diffusion_coefficient::square_meter_per_second>();

        dbg!(&temperature);
        if t_k > 1551.0 {

            // larger tolerance for higher temperatures
            let rtol = 0.35;
            assert_relative_eq!(
                got_d_m2_s,
                expected_d_m2_s,
                max_relative=rtol,
            );

            continue;
        }
        assert_relative_eq!(
            got_d_m2_s,
            expected_d_m2_s,
            max_relative = rtol,
        );
    }
}

#[test]
fn test_diffusion_coeff_jiang_matches_tabulated_cs_in_buffer() {
    // GIVEN
    let triso_layer = TrisoPebbleLayerMaterial::Buffer;
    let nuclide = Nuclide::Cs137;

    // neutron fluence: 5.5e25 n/m^2
    let fluence =
        ArealNumberDensity::new::<uom::si::areal_number_density::per_square_meter>(5.5e25);
    let gamma_neutron_fluence = Some(fluence);

    // (T [K], log10(D [m^2/s])) for Cs in Buffer
    let data: &[(f64, f64)] = &[
        (642.6282, -7.9535),
        (718.9103, -7.9136),
        (781.7308, -7.9136),
        (862.5000, -7.9136),
        (963.4615, -7.9136),
        (1100.3205, -7.9136),
        (1228.2051, -7.9136),
        (1398.7179, -7.9535),
        (1515.3846, -7.9934),
        (1634.2949, -7.9535),
        (1746.4744, -7.9535),
        (1831.7308, -7.9136),
        (1930.4487, -7.9535),
        (2026.9231, -7.9535),
    ];

    // THEN
    let rtol = 0.20;

    for &(t_k, log10_d) in data {
        let temperature = ThermodynamicTemperature::new::<kelvin>(t_k);

        let got = try_get_diffusion_coeff_jiang(
            triso_layer,
            nuclide,
            temperature,
            gamma_neutron_fluence,
        )
        .unwrap_or_else(|| panic!("Expected Some(D) at T={t_k} K, got None"));

        let expected_d_m2_s = 10f64.powf(log10_d);
        let got_d_m2_s = got.get::<uom::si::diffusion_coefficient::square_meter_per_second>();

        assert_relative_eq!(
            got_d_m2_s,
            expected_d_m2_s,
            max_relative = rtol,
        );
    }
}
