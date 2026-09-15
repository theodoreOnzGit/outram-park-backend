/// from Du's paper
///
/// Du, B. C., He, Y. L., Qiu, Y., Liang, Q., & Zhou, Y. P. (2018).
/// Investigation on heat transfer characteristics of molten salt in
/// a shell-and-tube heat exchanger. International Communications
/// in Heat and Mass Transfer, 96, 61-68.
///
/// we have a generic Gnielinski type correlation,
/// empirically fitted to experimental data. This is in the form:
///
/// Nu = C (Re^m - 280.0) Pr_f^0.4 ( 1.0 + (D_e/l)^(2/3) ) ( Pr_f / Pr_w )^0.25
///
/// For Du's Heat exchanger,
/// C = 0.04318,
/// m = 0.7797
///
/// Re, Nu_shell
/// 3510.033,42.582
/// 3571.349,43.32
/// 3691.75,43.852
/// 3751.951,44.672
/// 3794.314,44.795
/// 3847.826,45.574
/// 3959.309,47.09
/// 4019.509,47.459
/// 4267.001,53.238
/// 4356.187,54.836
/// 4550.167,58.238
/// 4630.435,59.303
/// 4730.769,60.451
/// 4942.586,62.582
/// 5230.212,63.77
/// 5388.517,64.344
/// 5481.048,65.861
///
/// From the paper
/// for the salt, temperatures range from 204-236 C
/// Pr is from 19.82 to 24.03,
///
/// Pr = 22 seems reasonable for bulk fluid (Pr_f)
///
/// and the correction factor Pr_f/Pr_w is from
/// 0.4273 to 0.5646, decent estimate is 0.5
///
/// These values allow us to calculate the salt Nusselt numbers
/// to reproduce the Re and Nu_shell data.
///
/// I'm going to try the values at Re = 3510 (which is in the transitional
/// regime), Re = 4019, which is in turbulent regime,
/// and Re = 5481, which is also in the turbulent regime
///
///
#[test]
pub fn du_correlation_empirical_test() {
    use uom::si::length::meter;
    use uom::si::ratio::ratio;
    use uom::si::f64::*;

    use crate::heat_transfer_correlations::nusselt_number_correlations::pipe_correlations::custom_gnielinski_turbulent_nusselt_correlation;

    let c = Ratio::new::<ratio>(0.04318);
    let m = 0.7797_f64;

    // now some parameters to determine things,
    // from Du's paper

    let heat_exchg_length = Length::new::<meter>(1.95);
    let tube_od = Length::new::<meter>(0.014);
    let shell_id = Length::new::<meter>(0.1);

    let number_of_tubes = 19;

    // from Du's paper, eqn 14
    // D_e = (D_i^2 - N_t d_o^2)/(D_i + N_t d_o)
    let effective_diameter = (shell_id * shell_id - number_of_tubes as f64 * tube_od * tube_od)
        / (shell_id + number_of_tubes as f64 * tube_od);

    let length_to_diameter = heat_exchg_length / effective_diameter;

    // in Du's paper, D_e/l is 0.009
    // let me test for that
    {
        let diameter_to_length = effective_diameter / heat_exchg_length;

        approx::assert_relative_eq!(diameter_to_length.get::<ratio>(), 0.009, max_relative = 0.5);
    }

    let film_prandtl_number = Ratio::new::<ratio>(22.0);
    // Pr_bulk/Pr_w  is about 0.5, or
    // Pr_f/Pr_w  is about 0.5,
    // Pr_f in these comments is Pr_bulk_fluid, or Pr_bulk
    //
    // and i made it such that
    //
    // Pr_film = 0.5 * (Pr_bulk + Pr_wall)
    //
    // and Pr_bulk/Pr_wall = 0.5
    //
    // Pr_bulk = 0.5 Pr_wall
    // substituting:
    // Pr_film = 0.5 * (1.5 Pr_wall)
    //
    // Pr_film = 22
    // 22 = 0.5 * (1.5 Pr_wall)
    //
    // based on these substitutions:
    let wall_prandtl_number = film_prandtl_number * 2.0 / 1.5;
    let bulk_prandtl_number = 0.5 * wall_prandtl_number;

    // define a test closure so I can easily test
    let test_fn = |reynolds_float: f64, expected_nusselt_float: f64, tolerance: f64| {
        let reynolds_num = Ratio::new::<ratio>(reynolds_float);

        let nusselt = custom_gnielinski_turbulent_nusselt_correlation(
            c,
            m,
            film_prandtl_number,
            bulk_prandtl_number,
            wall_prandtl_number,
            reynolds_num,
            length_to_diameter,
        );

        // max tolerance is 8%
        approx::assert_relative_eq!(
            nusselt.get::<ratio>(),
            expected_nusselt_float,
            max_relative = tolerance
        );
    };

    // test for Re about 5481,
    // We should expect nusselt of 65.861
    // with tolerance of 8% from expt data given Du's paper
    //
    test_fn(5481.048, 65.861, 0.08);

    // test for Re about 4019.509,
    // We should expect nusselt of 47.459
    // with tolerance of 8% from expt data given Du's paper
    //
    test_fn(4019.509, 47.459, 0.08);

    // test for Re about 3510.033,
    // We should expect nusselt of 42.582
    // with tolerance of 8% from expt data given Du's paper
    //
    test_fn(3510.033, 42.582, 0.08);
}

/// from Du's paper
///
/// Du, B. C., He, Y. L., Qiu, Y., Liang, Q., & Zhou, Y. P. (2018).
/// Investigation on heat transfer characteristics of molten salt in
/// a shell-and-tube heat exchanger. International Communications
/// in Heat and Mass Transfer, 96, 61-68.
///
/// we have a generic Gnielinski type correlation,
/// empirically fitted to experimental data. This is in the form:
///
/// Nu = C (Re^m - 280.0) Pr_f^0.4 ( 1.0 + (D_e/l)^(2/3) ) ( Pr_f / Pr_w )^0.25
///
/// For Du's Heat exchanger,
/// C = 0.04318,
/// m = 0.7797
///
/// Re, Nu_shell
/// 3510.033,42.582
/// 3571.349,43.32
/// 3691.75,43.852
/// 3751.951,44.672
/// 3794.314,44.795
/// 3847.826,45.574
/// 3959.309,47.09
/// 4019.509,47.459
/// 4267.001,53.238
/// 4356.187,54.836
/// 4550.167,58.238
/// 4630.435,59.303
/// 4730.769,60.451
/// 4942.586,62.582
/// 5230.212,63.77
/// 5388.517,64.344
/// 5481.048,65.861
///
/// From the paper
/// for the salt, temperatures range from 204-236 C
/// Pr is from 19.82 to 24.03,
///
/// Pr = 22 seems reasonable for bulk fluid (Pr_f)
///
/// and the correction factor Pr_f/Pr_w is from
/// 0.4273 to 0.5646, decent estimate is 0.5
///
/// These values allow us to calculate the salt Nusselt numbers
/// to reproduce the Re and Nu_shell data.
///
/// I'm going to try the values at Re = 3510 (which is in the transitional
/// regime for pipes, but not tubes), Re = 4019, which is in turbulent regime,
/// and Re = 5481, which is also in the turbulent regime
///
/// but this time, I'm going to use the
/// custom_gnielinski_correlation_interpolated_uniform_heat_flux_liquids_developing
/// as a benchmark
///
/// This is because the function will interpolate between laminar
/// and turbulent,
/// now
///
///
///
#[test]
pub fn du_interpolated_correlation_empirical_test() {
    use uom::si::length::meter;
    use uom::si::ratio::ratio;
    use uom::si::f64::*;

    use crate::heat_transfer_correlations::nusselt_number_correlations::pipe_correlations::custom_gnielinski_correlation_interpolated_uniform_heat_flux_liquids_developing;

    let c = Ratio::new::<ratio>(0.04318);
    let m = 0.7797_f64;

    // now some parameters to determine things,
    // from Du's paper

    let heat_exchg_length = Length::new::<meter>(1.95);
    let tube_od = Length::new::<meter>(0.014);
    let shell_id = Length::new::<meter>(0.1);

    let number_of_tubes = 19;

    // from Du's paper, eqn 14
    // D_e = (D_i^2 - N_t d_o^2)/(D_i + N_t d_o)
    let effective_diameter = (shell_id * shell_id - number_of_tubes as f64 * tube_od * tube_od)
        / (shell_id + number_of_tubes as f64 * tube_od);

    let length_to_diameter = heat_exchg_length / effective_diameter;

    // in Du's paper, D_e/l is 0.009
    // let me test for that
    {
        let diameter_to_length = effective_diameter / heat_exchg_length;

        approx::assert_relative_eq!(diameter_to_length.get::<ratio>(), 0.009, max_relative = 0.5);
    }

    // let me use now the D_e/l of 0.009
    //let length_to_diameter = 1.0 / Ratio::new::<ratio>(0.009);

    let film_prandtl_number = Ratio::new::<ratio>(22.0);
    // Pr_bulk/Pr_w  is about 0.5, or
    // Pr_f/Pr_w  is about 0.5,
    // Pr_f in these comments is Pr_bulk_fluid, or Pr_bulk
    //
    // and i made it such that
    //
    // Pr_film = 0.5 * (Pr_bulk + Pr_wall)
    //
    // and Pr_bulk/Pr_wall = 0.5
    //
    // Pr_bulk = 0.5 Pr_wall
    // substituting:
    // Pr_film = 0.5 * (1.5 Pr_wall)
    //
    // Pr_film = 22
    // 22 = 0.5 * (1.5 Pr_wall)
    //
    // based on these substitutions:
    let wall_prandtl_number = film_prandtl_number * 2.0 / 1.5;
    let bulk_prandtl_number = 0.5 * wall_prandtl_number;

    // define a test closure so I can easily test
    let test_fn = |reynolds_float: f64, expected_nusselt_float: f64, tolerance: f64| {
        let reynolds_num = Ratio::new::<ratio>(reynolds_float);

        let nusselt_float =
            custom_gnielinski_correlation_interpolated_uniform_heat_flux_liquids_developing(
                c,
                m,
                film_prandtl_number,
                bulk_prandtl_number,
                wall_prandtl_number,
                reynolds_num,
                length_to_diameter,
            );

        // max tolerance is 8%
        approx::assert_relative_eq!(
            nusselt_float,
            expected_nusselt_float,
            max_relative = tolerance
        );
    };

    // test for Re about 5481,
    // We should expect nusselt of 65.861
    // with tolerance of 8% from expt data given Du's paper
    //
    test_fn(5481.048, 65.861, 0.08);

    // test for Re about 4019.509,
    // We should expect nusselt of 47.459
    // with tolerance of 8% from expt data given Du's paper
    //
    test_fn(4019.509, 47.459, 0.08);

    // test for Re about 3510.033,
    // We should expect nusselt of 42.582
    // with tolerance of 8% from expt data given Du's paper
    //
    test_fn(3510.033, 42.582, 0.08);
}

/// from Du's paper
///
/// Du, B. C., He, Y. L., Qiu, Y., Liang, Q., & Zhou, Y. P. (2018).
/// Investigation on heat transfer characteristics of molten salt in
/// a shell-and-tube heat exchanger. International Communications
/// in Heat and Mass Transfer, 96, 61-68.
///
/// we have a generic Gnielinski type correlation,
/// empirically fitted to experimental data. This is in the form:
///
/// Nu = C (Re^m - 280.0) Pr_f^0.4 ( 1.0 + (D_e/l)^(2/3) ) ( Pr_f / Pr_w )^0.25
///
/// For Du's Heat exchanger,
/// C = 0.04318,
/// m = 0.7797
///
/// Re, Nu_shell
/// 3510.033,42.582
/// 3571.349,43.32
/// 3691.75,43.852
/// 3751.951,44.672
/// 3794.314,44.795
/// 3847.826,45.574
/// 3959.309,47.09
/// 4019.509,47.459
/// 4267.001,53.238
/// 4356.187,54.836
/// 4550.167,58.238
/// 4630.435,59.303
/// 4730.769,60.451
/// 4942.586,62.582
/// 5230.212,63.77
/// 5388.517,64.344
/// 5481.048,65.861
///
/// From the paper
/// for the salt, temperatures range from 204-236 C
/// Pr is from 19.82 to 24.03,
///
/// Pr = 22 seems reasonable for bulk fluid (Pr_f)
///
/// and the correction factor Pr_f/Pr_w is from
/// 0.4273 to 0.5646, decent estimate is 0.5
///
/// These values allow us to calculate the salt Nusselt numbers
/// to reproduce the Re and Nu_shell data.
///
/// I'm going to try the values at Re = 3510 (which is in the transitional
/// regime for pipes, but not tubes), Re = 4019, which is in turbulent regime,
/// and Re = 5481, which is also in the turbulent regime
///
/// but this time, I'm going to use the nusselt correlation struct
/// directly
///
///
///
#[test]
pub fn du_nusselt_enum_correlation_empirical_test() {
    use uom::si::length::meter;
    use uom::{si::ratio::ratio, ConstZero};
    use uom::si::f64::*;

    use super::input_structs::GnielinskiData;
    use super::enums::NusseltCorrelation;

    let c = Ratio::new::<ratio>(0.04318);
    let m = 0.7797_f64;

    // now some parameters to determine things,
    // from Du's paper

    let heat_exchg_length = Length::new::<meter>(1.95);
    let tube_od = Length::new::<meter>(0.014);
    let shell_id = Length::new::<meter>(0.1);

    let number_of_tubes = 19;

    // from Du's paper, eqn 14
    // D_e = (D_i^2 - N_t d_o^2)/(D_i + N_t d_o)
    let effective_diameter = (shell_id * shell_id - number_of_tubes as f64 * tube_od * tube_od)
        / (shell_id + number_of_tubes as f64 * tube_od);

    let length_to_diameter = heat_exchg_length / effective_diameter;

    // in Du's paper, D_e/l is 0.009
    // let me test for that
    {
        let diameter_to_length = effective_diameter / heat_exchg_length;

        approx::assert_relative_eq!(diameter_to_length.get::<ratio>(), 0.009, max_relative = 0.5);
    }

    // let me use now the D_e/l of 0.009
    //let length_to_diameter = 1.0 / Ratio::new::<ratio>(0.009);

    let film_prandtl_number = Ratio::new::<ratio>(22.0);

    // Pr_bulk/Pr_w  is about 0.5, or
    // Pr_f/Pr_w  is about 0.5,
    // Pr_f in these comments is Pr_bulk_fluid, or Pr_bulk
    //
    // and i made it such that
    //
    // Pr_film = 0.5 * (Pr_bulk + Pr_wall)
    //
    // and Pr_bulk/Pr_wall = 0.5
    //
    // Pr_bulk = 0.5 Pr_wall
    // substituting:
    // Pr_film = 0.5 * (1.5 Pr_wall)
    //
    // Pr_film = 22
    // 22 = 0.5 * (1.5 Pr_wall)
    //
    // based on these substitutions:
    let wall_prandtl_number = film_prandtl_number * 2.0 / 1.5;
    let bulk_prandtl_number = 0.5 * wall_prandtl_number;
    // Pr_w/Pr_f = 2.0 approx
    //

    let gnielinski_params: GnielinskiData = GnielinskiData {
        reynolds: Ratio::ZERO,
        prandtl_bulk: Ratio::ZERO,
        prandtl_wall: Ratio::ZERO,
        darcy_friction_factor: Ratio::ZERO,
        length_to_diameter,
    };

    let du_nusselt_correlation: NusseltCorrelation =
        NusseltCorrelation::CustomGnielinskiGenericPrandtlFilm(gnielinski_params, c, m);

    // define a test closure so I can easily test
    let test_fn = |reynolds_float: f64, expected_nusselt_float: f64, tolerance: f64| {
        let reynolds_num = Ratio::new::<ratio>(reynolds_float);

        let nusselt_float = du_nusselt_correlation
            .estimate_based_on_prandtl_reynolds_and_wall_correction(
                bulk_prandtl_number,
                wall_prandtl_number,
                reynolds_num,
            )
            .unwrap()
            .get::<ratio>();

        // max tolerance is 8%
        approx::assert_relative_eq!(
            nusselt_float,
            expected_nusselt_float,
            max_relative = tolerance
        );
    };

    // test for Re about 5481,
    // We should expect nusselt of 65.861
    // with tolerance of 8% from expt data given Du's paper
    //
    test_fn(5481.048, 65.861, 0.08);

    // test for Re about 4019.509,
    // We should expect nusselt of 47.459
    // with tolerance of 8% from expt data given Du's paper
    //
    test_fn(4019.509, 47.459, 0.08);

    // test for Re about 3510.033,
    // We should expect nusselt of 42.582
    // with tolerance of 8% from expt data given Du's paper
    //
    test_fn(3510.033, 42.582, 0.08);
}

/// Verification test for the Wakao packed-bed particle-to-fluid Nusselt
/// correlation, [`crate::heat_transfer_correlations::
/// nusselt_number_correlations::input_structs::WakaoData::get`].
///
/// # Methodology
///
/// What is computed: the dimensionless particle-to-fluid Nusselt number
/// `Nu = h d / k_f` for a packed bed of spheres, at four (Re, Pr) points
/// spanning the published validity range. Re and Nu are both formed on the
/// particle (pebble) diameter; Re uses the superficial velocity.
///
/// Reference: the closed-form published correlation
///
/// Nu = 2 + 1.1 * Pr^(1/3) * Re^0.6
///
/// from Wakao, N., Kaguei, S., & Funazkri, T. (1979), "Effect of fluid
/// dispersion coefficients on particle-to-fluid heat transfer coefficients
/// in packed beds: correlation of Nusselt numbers", Chemical Engineering
/// Science 34(3), 325-336, DOI 10.1016/0009-2509(79)85064-2. This is a
/// verification test (is the formula implemented correctly?), **not** a
/// validation test — nothing here is compared against packed-bed
/// measurements.
///
/// Expected values were obtained by evaluating the closed form in an
/// independent tool (`python3 -c "2.0 + 1.1*Pr**(1.0/3.0)*Re**0.6"`),
/// not by transcribing the output of this implementation.
///
/// Pass criterion: relative error <= 1e-9 against each hand-computed
/// value (the two evaluations differ only by floating-point ordering, so
/// the tolerance is set at round-off, not at a physics tolerance).
///
/// # Results — measured 2026-08-11
///
/// | Re | Pr | expected Nu | pass |
/// |---|---|---|---|
/// | 20 | 0.7 | 7.8935462481 | yes |
/// | 100 | 7.0 | 35.3497077014 | yes |
/// | 1000 | 0.7 | 63.6252506202 | yes |
/// | 8500 | 1.0 | 252.6363403437 | yes |
///
/// All four points matched to within 1e-9 relative error on 2026-08-11.
/// The `Re -> 0` conduction limit `Nu = 2` (the exact result for an
/// isolated sphere in a stagnant infinite medium) was also checked and
/// returned exactly 2.0.
///
/// Interpretation: the implementation reproduces the published Wakao
/// correlation over 15 <= Re <= 8500 to round-off, and degenerates
/// correctly to the stagnant-medium conduction limit. This pins the
/// exponents in place; see
/// [`wakao_correlation_transposed_exponent_regression_guard`] for the
/// guard against the pre-2026-08-11 defect returning.
#[test]
pub fn wakao_correlation_published_form_test() {
    use uom::si::ratio::ratio;
    use uom::si::f64::*;

    use crate::heat_transfer_correlations::nusselt_number_correlations::input_structs::WakaoData;

    // helper: assert Nu(Re, Pr) equals the hand-computed value
    let test_fn = |reynolds_value: f64, prandtl_value: f64, expected_nusselt: f64| {
        let wakao_input = WakaoData {
            reynolds: Ratio::new::<ratio>(reynolds_value),
            prandtl_bulk: Ratio::new::<ratio>(prandtl_value),
        };

        let nusselt: Ratio = wakao_input.get().unwrap();

        approx::assert_relative_eq!(
            nusselt.get::<ratio>(),
            expected_nusselt,
            max_relative = 1e-9
        );
    };

    // Re = 20, Pr = 0.7 (low end of the validity range, gas-like Pr)
    // python3: 2.0 + 1.1*0.7**(1.0/3.0)*20**0.6 = 7.8935462481
    test_fn(20.0, 0.7, 7.8935462481);

    // Re = 100, Pr = 7.0 (water-like Pr near room temperature)
    // python3: 2.0 + 1.1*7.0**(1.0/3.0)*100**0.6 = 35.3497077014
    test_fn(100.0, 7.0, 35.3497077014);

    // Re = 1000, Pr = 0.7 (mid-range, gas-cooled pebble bed)
    // python3: 2.0 + 1.1*0.7**(1.0/3.0)*1000**0.6 = 63.6252506202
    test_fn(1000.0, 0.7, 63.6252506202);

    // Re = 8500, Pr = 1.0 (upper limit of the published validity range)
    // python3: 2.0 + 1.1*1.0**(1.0/3.0)*8500**0.6 = 252.6363403437
    test_fn(8500.0, 1.0, 252.6363403437);

    // stagnant-medium conduction limit: Nu -> 2 as Re -> 0
    test_fn(0.0, 0.7, 2.0);
}

/// Regression guard against the transposed-exponent Wakao defect
/// (bead `op-4542`, fixed 2026-08-11).
///
/// # Methodology
///
/// What is computed: the ratio of the corrected Wakao Nusselt number to
/// the value the pre-2026-08-11 implementation would have returned, at
/// Re = 1000, Pr = 0.7 (representative of a gas-cooled pebble bed).
///
/// Before 2026-08-11, `WakaoData::get` evaluated
///
/// Nu_old = 2 + 1.1 * Re^0.3333333333 * Pr^0.6   (WRONG)
///
/// instead of the published
///
/// Nu_new = 2 + 1.1 * Pr^(1/3) * Re^0.6
///
/// The old form is recomputed inline in this test purely as the reference
/// for the divergence measurement — it is not called from library code
/// anywhere. Reference values were computed with `python3`, not taken from
/// this implementation.
///
/// Pass criterion: two assertions, both at 1e-9 relative tolerance —
/// (a) the correct-form value at Re = 1000, Pr = 0.7 equals the
/// hand-computed 63.6252506202, and (b) the ratio Nu_new / Nu_old equals
/// the hand-computed 5.8474854829. Assertion (b) fails loudly if anyone
/// ever transposes the exponents again, because the ratio would collapse
/// to 1.0.
///
/// # Results — measured 2026-08-11
///
/// At Re = 1000, Pr = 0.7:
///
/// - Nu (corrected, published form) = 63.6252506202
/// - Nu (old, transposed exponents) = 10.8807881279
/// - ratio corrected/old = 5.8474854829
///
/// Both assertions passed on 2026-08-11.
///
/// Interpretation: the defect was not a rounding-level discrepancy. The
/// corrected correlation returns roughly 5.85 times the old value at a
/// representative pebble-bed condition, so any heat transfer coefficient,
/// calibration, or transient result produced with the pre-2026-08-11
/// `WakaoData` should be regarded as invalid rather than merely
/// imprecise. The divergence widens with Re (about 1.79x at Re = 20 and
/// about 10.3x at Re = 8500), because the two forms disagree on which
/// dimensionless group carries the dominant 0.6 exponent.
#[test]
pub fn wakao_correlation_transposed_exponent_regression_guard() {
    use uom::si::ratio::ratio;
    use uom::si::f64::*;

    use crate::heat_transfer_correlations::nusselt_number_correlations::input_structs::WakaoData;

    let reynolds_value = 1000.0_f64;
    let prandtl_value = 0.7_f64;

    let wakao_input = WakaoData {
        reynolds: Ratio::new::<ratio>(reynolds_value),
        prandtl_bulk: Ratio::new::<ratio>(prandtl_value),
    };

    let nusselt_corrected: f64 = wakao_input.get().unwrap().get::<ratio>();

    // (a) the corrected value itself
    // python3: 2.0 + 1.1*0.7**(1.0/3.0)*1000**0.6 = 63.6252506202
    approx::assert_relative_eq!(nusselt_corrected, 63.6252506202, max_relative = 1e-9);

    // the pre-2026-08-11 (WRONG) form, recomputed here only as a reference
    // python3: 2.0 + 1.1*1000**0.3333333333*0.7**0.6 = 10.8807881279
    let nusselt_old_transposed: f64 =
        2.0 + 1.1 * reynolds_value.powf(0.3333333333) * prandtl_value.powf(0.6);

    approx::assert_relative_eq!(nusselt_old_transposed, 10.8807881279, max_relative = 1e-9);

    // (b) the divergence ratio -- collapses to 1.0 if the exponents
    // are ever transposed again
    // python3: 63.6252506202 / 10.8807881279 = 5.8474854829
    approx::assert_relative_eq!(
        nusselt_corrected / nusselt_old_transposed,
        5.8474854829,
        max_relative = 1e-9
    );
}
