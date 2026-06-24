use crate::lagrangian_decay_simulator::lagrangian_diffusion::temperature_dependent_collisions::{TrisoPebbleLayerMaterial, try_get_diffusion_coeff_jiang};


// If you already use the `approx` crate elsewhere, this is the nicest way:
// approx = "0.5"
use approx::assert_relative_eq;

use fission_yields_data::prelude::Nuclide;
// If you use `uom`, these are common imports. Adjust to match your project.
use uom::si::f64::*;
use uom::si::thermodynamic_temperature::kelvin;

#[test]
fn test_diffusion_coeff_jiang_matches_tabulated_ag110m_in_sic() {
    // GIVEN
    let triso_layer = TrisoPebbleLayerMaterial::SiC;
    let nuclide = Nuclide::Ag110m; // adjust if your enum uses a different naming

    // neutron fluence: 5.5e25 n/m^2
    let fluence =
        ArealNumberDensity::new::<uom::si::areal_number_density::per_square_meter>(5.5e25);
    let gamma_neutron_fluence = Some(fluence);

    // (T [K], log10(D [m^2/s])) for Ag-110m in SiC
    let data: &[(f64, f64)] = &[
        (623.2402, -26.4665),
        (665.5721, -25.2610),
        (713.5482, -24.1057),
        (758.7022, -23.2518),
        (815.1447, -22.2471),
        (882.8757, -21.2425),
        (925.2076, -20.4891),
        (992.9386, -19.7356),
        (1071.9582, -18.8314),
        (1150.9777, -18.1784),
        (1244.1078, -17.4250),
        (1320.3052, -16.9227),
        (1399.3247, -16.5208),
        (1472.7000, -16.0688),
        (1531.9646, -15.8176),
        (1605.3399, -15.4660),
        (1678.7151, -15.2148),
        (1732.3355, -14.9134),
        (1794.4223, -14.6623),
        (1881.9082, -14.4111),
        (1918.5958, -14.3107),
        (1975.0383, -14.1098),
        (2017.3702, -13.9088),
        (2062.5242, -13.8084),
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
        if t_k > 1975.0 {

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
            max_relative = rtol,
        );
    }
}

#[test]
fn test_diffusion_coeff_jiang_matches_tabulated_ag110m_in_pyc() {
    // GIVEN
    let triso_layer = TrisoPebbleLayerMaterial::PyC;
    let nuclide = Nuclide::Ag110m; // adjust if your enum uses a different naming

    // neutron fluence: 5.5e25 n/m^2
    let fluence =
        ArealNumberDensity::new::<uom::si::areal_number_density::per_square_meter>(5.5e25);
    let gamma_neutron_fluence = Some(fluence);

    // (T [K], log10(D [m^2/s])) for Ag-110m in PyC
    let data: &[(f64, f64)] = &[
        (623.2402, -21.0918),
        (668.3942, -20.2379),
        (702.2597, -19.6351),
        (744.5916, -19.0324),
        (792.5677, -18.2789),
        (849.0102, -17.6259),
        (928.0298, -16.8724),
        (992.9386, -16.3199),
        (1080.4245, -15.7171),
        (1150.9777, -15.2148),
        (1213.0644, -14.7628),
        (1300.5503, -14.3609),
        (1382.3920, -14.0093),
        (1458.5893, -13.7079),
        (1517.8540, -13.4065),
        (1596.8735, -13.2056),
        (1639.2054, -13.0549),
        (1695.6479, -12.9042),
        (1754.9125, -12.8038),
        (1797.2444, -12.6531),
        (1862.1533, -12.5526),
        (1915.7737, -12.4019),
        (1966.5719, -12.2512),
        (2023.0144, -12.1005),
        (2070.9906, -12.0503),
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
        if t_k > 1200.0 {

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
fn test_diffusion_coeff_jiang_matches_tabulated_ag110m_in_kernel() {
    // GIVEN
    let triso_layer = TrisoPebbleLayerMaterial::KernelUO2;
    let nuclide = Nuclide::Ag110m; // adjust if your enum uses a different naming

    // neutron fluence: 5.5e25 n/m^2
    let fluence =
        ArealNumberDensity::new::<uom::si::areal_number_density::per_square_meter>(5.5e25);
    let gamma_neutron_fluence = Some(fluence);

    // (T [K], log10(D [m^2/s])) for Ag-110m in Kernel
    let data: &[(f64, f64)] = &[
        (617.5960, -22.1969),
        (654.2836, -21.3430),
        (702.2597, -20.3886),
        (750.2358, -19.5347),
        (809.5005, -18.7812),
        (871.5872, -18.1282),
        (933.6740, -17.4250),
        (987.2944, -16.9227),
        (1046.5590, -16.4204),
        (1117.1122, -15.8678),
        (1198.9538, -15.4660),
        (1263.8627, -14.9637),
        (1351.3486, -14.5116),
        (1433.1902, -14.1600),
        (1529.1425, -13.7581),
        (1605.3399, -13.5070),
        (1692.8258, -13.2056),
        (1749.2683, -13.0549),
        (1825.4657, -12.8540),
        (1884.7303, -12.7033),
        (1938.3507, -12.6028),
        (1969.3940, -12.5024),
        (2017.3702, -12.4521),
        (2065.3463, -12.3517),
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
        if t_k > 1350.0 {

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
            max_relative = rtol,
        );
    }
}

#[test]
fn test_diffusion_coeff_jiang_matches_tabulated_ag110m_in_buffer() {
    // GIVEN
    let triso_layer = TrisoPebbleLayerMaterial::Buffer;
    let nuclide = Nuclide::Ag110m; // adjust if your enum uses a different naming

    // neutron fluence: 5.5e25 n/m^2
    let fluence =
        ArealNumberDensity::new::<uom::si::areal_number_density::per_square_meter>(5.5e25);
    let gamma_neutron_fluence = Some(fluence);

    // (T [K], log10(D [m^2/s])) for Ag-110m in Buffer
    let data: &[(f64, f64)] = &[
        (648.6393, -8.0318),
        (809.5005, -8.0318),
        (995.7608, -7.9314),
        (1198.9538, -8.0318),
        (1399.3247, -8.0318),
        (1554.5416, -7.9816),
        (1678.7151, -7.9816),
        (1836.7542, -8.0318),
        (1943.9949, -8.0821),
        (2039.9472, -8.0821),
    ];

    // THEN
    // 40% okay for log plots...
    let rtol = 0.40;

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
        assert_relative_eq!(
            got_d_m2_s,
            expected_d_m2_s,
            max_relative = rtol,
        );
    }
}
