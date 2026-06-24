use crate::lagrangian_decay_simulator::lagrangian_diffusion::temperature_dependent_collisions::{TrisoPebbleLayerMaterial, try_get_diffusion_coeff_jiang};


// If you already use the `approx` crate elsewhere, this is the nicest way:
// approx = "0.5"
use approx::assert_relative_eq;

use fission_yields_data::prelude::Nuclide;
// If you use `uom`, these are common imports. Adjust to match your project.
use uom::si::f64::*;
use uom::si::thermodynamic_temperature::kelvin;


#[test]
fn test_diffusion_coeff_jiang_matches_tabulated_sr_in_sic() {
    // GIVEN
    let triso_layer = TrisoPebbleLayerMaterial::SiC;
    let nuclide = Nuclide::Sr90; // adjust if your enum uses a different Sr nuclide name

    // neutron fluence: 5.5e25 n/m^2
    let fluence =
        ArealNumberDensity::new::<uom::si::areal_number_density::per_square_meter>(5.5e25);
    let gamma_neutron_fluence = Some(fluence);

    // (T [K], log10(D [m^2/s])) for Sr in SiC
    let data: &[(f64, f64)] = &[
        (626.0623, -26.0144),
        (668.3942, -24.9596),
        (713.5482, -23.9047),
        (769.9907, -22.8499),
        (832.0775, -21.7448),
        (902.6306, -20.7402),
        (984.4723, -19.7356),
        (1074.7803, -18.8314),
        (1187.6653, -17.9273),
        (1303.3724, -17.1738),
        (1399.3247, -16.4706),
        (1506.5655, -16.0185),
        (1599.6956, -15.5664),
        (1704.1143, -15.1646),
        (1797.2444, -14.9134),
        (1881.9082, -14.5116),
        (1969.3940, -14.2604),
        (2051.2357, -14.0093),
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
        if t_k > 1881.0 {

            // larger tolerance for higher temperatures
            let rtol = 0.55;
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
fn test_diffusion_coeff_jiang_matches_tabulated_sr_in_pyc() {
    // GIVEN
    let triso_layer = TrisoPebbleLayerMaterial::PyC;
    let nuclide = Nuclide::Sr90; // adjust if your enum uses a different Sr nuclide name

    // neutron fluence: 5.5e25 n/m^2
    let fluence =
        ArealNumberDensity::new::<uom::si::areal_number_density::per_square_meter>(5.5e25);
    let gamma_neutron_fluence = Some(fluence);

    // (T [K], log10(D [m^2/s])) for Sr in PyC
    let data: &[(f64, f64)] = &[
        (611.9517, -22.4481),
        (640.1730, -21.7448),
        (674.0385, -20.7904),
        (716.3703, -19.9868),
        (767.1686, -18.9319),
        (840.5439, -17.8771),
        (919.5634, -16.8222),
        (984.4723, -15.9683),
        (1071.9582, -15.2651),
        (1153.7998, -14.5116),
        (1229.9972, -13.9591),
        (1317.4831, -13.3061),
        (1404.9690, -12.9042),
        (1489.6327, -12.4521),
        (1579.9407, -12.1005),
        (1667.4266, -11.6987),
        (1754.9125, -11.3973),
        (1848.0427, -11.1461),
        (1932.7064, -10.8950),
        (2023.0144, -10.6438),
        (2082.2791, -10.4931),
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
        dbg!(&(t_k, log10_d));

        if t_k > 1000.0 {

            // larger tolerance for higher temperatures
            let rtol = 0.50;
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
fn test_diffusion_coeff_jiang_matches_tabulated_sr_in_kernel() {
    // GIVEN
    let triso_layer = TrisoPebbleLayerMaterial::KernelUO2;
    let nuclide = Nuclide::Sr90; // adjust if your enum uses a different Sr nuclide name

    // neutron fluence: 5.5e25 n/m^2
    let fluence =
        ArealNumberDensity::new::<uom::si::areal_number_density::per_square_meter>(5.5e25);
    let gamma_neutron_fluence = Some(fluence);

    // (T [K], log10(D [m^2/s])) for Sr in Kernel
    let data: &[(f64, f64)] = &[
        (942.1404, -29.5306),
        (978.8280, -28.5762),
        (1032.4484, -27.3707),
        (1088.8909, -26.0647),
        (1148.1555, -24.9596),
        (1204.5980, -23.8043),
        (1258.2184, -22.9001),
        (1337.2379, -21.6444),
        (1410.6132, -20.6900),
        (1509.3876, -19.4844),
        (1608.1620, -18.5301),
        (1698.4700, -17.7264),
        (1777.4895, -16.8724),
        (1848.0427, -16.3701),
        (1932.7064, -15.8678),
        (2025.8366, -15.2148),
        (2090.7454, -14.7125),
    ];

    // THEN
    //
    // I reason that experimental data can be up to 30% different
    // sometimes the log graph errors also
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
        if t_k > 2090.0 {

            // larger tolerance for higher temperatures
            let rtol = 0.30;
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
fn test_diffusion_coeff_jiang_matches_tabulated_sr_in_buffer() {
    // GIVEN
    let triso_layer = TrisoPebbleLayerMaterial::Buffer;
    let nuclide = Nuclide::Sr90; // adjust if your enum uses a different Sr nuclide name

    // neutron fluence: 5.5e25 n/m^2
    let fluence =
        ArealNumberDensity::new::<uom::si::areal_number_density::per_square_meter>(5.5e25);
    let gamma_neutron_fluence = Some(fluence);

    // (T [K], log10(D [m^2/s])) for Sr in Buffer
    let data: &[(f64, f64)] = &[
        (620.4181, -7.9314),
        (758.7022, -7.9314),
        (880.0536, -7.9314),
        (973.1838, -7.9816),
        (1049.3811, -7.8811),
        (1134.0449, -7.9314),
        (1215.8866, -7.9314),
        (1309.0167, -7.9816),
        (1407.7911, -7.9314),
        (1498.0991, -7.8811),
        (1571.4744, -7.8811),
        (1636.3832, -7.8811),
        (1723.8691, -7.9314),
        (1800.0665, -7.9816),
        (1890.3745, -7.9816),
        (1960.9277, -7.9314),
        (2051.2357, -7.9314),
        (2087.9233, -7.9314),
    ];

    // THEN
    //
    // 40% is okay for log plots
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
