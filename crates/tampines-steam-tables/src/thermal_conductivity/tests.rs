use uom::si::mass_density::kilogram_per_cubic_meter;
use uom::si::thermodynamic_temperature::kelvin;
use uom::si::f64::*;

use super::*;

#[test]
pub fn lambda_0_test(){
    let t1 = ThermodynamicTemperature::new::<kelvin>(298.15);
    let t2 = ThermodynamicTemperature::new::<kelvin>(873.15);
    let t3 = ThermodynamicTemperature::new::<kelvin>(673.15);
    let t4 = ThermodynamicTemperature::new::<kelvin>(1173.15);

    let lambda_0_1 = 0.184_341_883e2;
    let lambda_0_2 = 0.791_034_659e2;
    let lambda_0_3 = 0.545_433_367e2;
    let lambda_0_4 = 0.119_586_108e3;

    let lambda_0_test = lambda_0(t1);
    approx::assert_relative_eq!(
        lambda_0_1,
        lambda_0_test,
        max_relative=1e-8
        );
    let lambda_0_test = lambda_0(t2);
    approx::assert_relative_eq!(
        lambda_0_2,
        lambda_0_test,
        max_relative=1e-8
        );
    let lambda_0_test = lambda_0(t3);
    approx::assert_relative_eq!(
        lambda_0_3,
        lambda_0_test,
        max_relative=1e-8
        );
    let lambda_0_test = lambda_0(t4);
    approx::assert_relative_eq!(
        lambda_0_4,
        lambda_0_test,
        max_relative=1e-8
        );
    
}
#[test]
pub fn lambda_1_test(){
    let t1 = ThermodynamicTemperature::new::<kelvin>(298.15);
    let t2 = ThermodynamicTemperature::new::<kelvin>(873.15);
    let t3 = ThermodynamicTemperature::new::<kelvin>(673.15);
    let t4 = ThermodynamicTemperature::new::<kelvin>(1173.15);

    let rho1 = MassDensity::new::<kilogram_per_cubic_meter>(0.997_047_435e3);
    let rho2 = MassDensity::new::<kilogram_per_cubic_meter>(0.260_569_558e2);
    let rho3 = MassDensity::new::<kilogram_per_cubic_meter>(0.523_371_289e3);
    let rho4 = MassDensity::new::<kilogram_per_cubic_meter>(0.377_584_848e2);

    let lambda_1_1 = 0.329_016_833e2;
    let lambda_1_2 = 0.110_043_337e1;
    let lambda_1_3 = 0.726_398_725e1;
    let lambda_1_4 = 0.115_280_540e1;

    let lambda_1_test = lambda_1(rho1,t1);
    approx::assert_relative_eq!(
        lambda_1_1,
        lambda_1_test,
        max_relative=1e-8
        );
    let lambda_1_test = lambda_1(rho2,t2);
    approx::assert_relative_eq!(
        lambda_1_2,
        lambda_1_test,
        max_relative=1e-8
        );
    let lambda_1_test = lambda_1(rho3,t3);
    approx::assert_relative_eq!(
        lambda_1_3,
        lambda_1_test,
        max_relative=1e-8
        );
    let lambda_1_test = lambda_1(rho4,t4);
    approx::assert_relative_eq!(
        lambda_1_4,
        lambda_1_test,
        max_relative=1e-8
        );
    
}

#[test]
fn c_test(){
    let _t1 = ThermodynamicTemperature::new::<kelvin>(298.15);
    let _t2 = ThermodynamicTemperature::new::<kelvin>(873.15);
    let _t3 = ThermodynamicTemperature::new::<kelvin>(673.15);
    let _t4 = ThermodynamicTemperature::new::<kelvin>(1173.15);

    let rho1 = MassDensity::new::<kilogram_per_cubic_meter>(0.997_047_435e3);
    let rho2 = MassDensity::new::<kilogram_per_cubic_meter>(0.260_569_558e2);
    let rho3 = MassDensity::new::<kilogram_per_cubic_meter>(0.523_371_289e3);
    let rho4 = MassDensity::new::<kilogram_per_cubic_meter>(0.377_584_848e2);

    let delta_1: f64 = (rho1/rho_crit_water()).get::<ratio>();
    let delta_2: f64 = (rho2/rho_crit_water()).get::<ratio>();
    let delta_3: f64 = (rho3/rho_crit_water()).get::<ratio>();
    let delta_4: f64 = (rho4/rho_crit_water()).get::<ratio>();

    let c_1 = 0.129_592_952e-1;
    let c_2 = 0.163_793_337e0;
    let c_3 = 0.940_881_573e-1;
    let c_4 = 0.168_780_175e0;

    dbg!(&(delta_1,delta_2,delta_3,delta_4));
    let c_test = captial_c(delta_1);
    approx::assert_relative_eq!(
        c_1,
        c_test,
        max_relative=1e-8
        );
    let c_test = captial_c(delta_2);
    approx::assert_relative_eq!(
        c_2,
        c_test,
        max_relative=1e-8
        );
    let c_test = captial_c(delta_3);
    approx::assert_relative_eq!(
        c_3,
        c_test,
        max_relative=1e-8
        );
    let c_test = captial_c(delta_4);
    approx::assert_relative_eq!(
        c_4,
        c_test,
        max_relative=1e-8
        );
    
}


#[test]
fn captial_b_test(){
    let t1 = ThermodynamicTemperature::new::<kelvin>(298.15);
    let t2 = ThermodynamicTemperature::new::<kelvin>(873.15);
    let t3 = ThermodynamicTemperature::new::<kelvin>(673.15);
    let t4 = ThermodynamicTemperature::new::<kelvin>(1173.15);

    let rho1 = MassDensity::new::<kilogram_per_cubic_meter>(0.997_047_435e3);
    let rho2 = MassDensity::new::<kilogram_per_cubic_meter>(0.260_569_558e2);
    let rho3 = MassDensity::new::<kilogram_per_cubic_meter>(0.523_371_289e3);
    let rho4 = MassDensity::new::<kilogram_per_cubic_meter>(0.377_584_848e2);

    let delta_1: f64 = (rho1/rho_crit_water()).get::<ratio>();
    let delta_2: f64 = (rho2/rho_crit_water()).get::<ratio>();
    let delta_3: f64 = (rho3/rho_crit_water()).get::<ratio>();
    let delta_4: f64 = (rho4/rho_crit_water()).get::<ratio>();

    let theta_1: f64 = (t1/t_crit_water()).get::<ratio>();
    let theta_2: f64 = (t2/t_crit_water()).get::<ratio>();
    let theta_3: f64 = (t3/t_crit_water()).get::<ratio>();
    let theta_4: f64 = (t4/t_crit_water()).get::<ratio>();

    let kappa_t_1 = 0.451_570_597e-3 * Pressure::new::<megapascal>(1.0).recip();
    let kappa_t_2 = 0.105_138_803e0 * Pressure::new::<megapascal>(1.0).recip();
    let kappa_t_3 = 0.141_857_631e-1 * Pressure::new::<megapascal>(1.0).recip();
    let kappa_t_4 = 0.510_625_539e-1 * Pressure::new::<megapascal>(1.0).recip();

    let b_2 = 5.639_822_730e-3;
    let b_3 = 0.373_064_478e0;
    // b1 and b4 values are not given in the table, but they default to 
    // zero because they are outside the range of validity
    let b_1 = 0.0;
    let b_4 = 0.0;

    let n5 = 1.5;

    dbg!(&(delta_1,delta_2,delta_3,delta_4));

    let b_test = captial_b(delta_1, theta_1, kappa_t_1, n5);
    approx::assert_abs_diff_eq!(
        b_1,
        b_test,
        epsilon=0.0
        );
    let b_test = captial_b(delta_2, theta_2, kappa_t_2, n5);
    approx::assert_relative_eq!(
        b_2,
        b_test,
        max_relative=1e-7
        );
    let b_test = captial_b(delta_3, theta_3, kappa_t_3, n5);
    approx::assert_relative_eq!(
        b_3,
        b_test,
        max_relative=1e-8
        );
    let b_test = captial_b(delta_4, theta_4, kappa_t_4, n5);
    approx::assert_abs_diff_eq!(
        b_4,
        b_test,
        epsilon=0.0
        );
    
}


#[test]
fn small_a_test(){
    let t1 = ThermodynamicTemperature::new::<kelvin>(298.15);
    let t2 = ThermodynamicTemperature::new::<kelvin>(873.15);
    let t3 = ThermodynamicTemperature::new::<kelvin>(673.15);
    let t4 = ThermodynamicTemperature::new::<kelvin>(1173.15);

    let rho1 = MassDensity::new::<kilogram_per_cubic_meter>(0.997_047_435e3);
    let rho2 = MassDensity::new::<kilogram_per_cubic_meter>(0.260_569_558e2);
    let rho3 = MassDensity::new::<kilogram_per_cubic_meter>(0.523_371_289e3);
    let rho4 = MassDensity::new::<kilogram_per_cubic_meter>(0.377_584_848e2);

    let delta_1: f64 = (rho1/rho_crit_water()).get::<ratio>();
    let delta_2: f64 = (rho2/rho_crit_water()).get::<ratio>();
    let delta_3: f64 = (rho3/rho_crit_water()).get::<ratio>();
    let delta_4: f64 = (rho4/rho_crit_water()).get::<ratio>();

    let theta_1: f64 = (t1/t_crit_water()).get::<ratio>();
    let theta_2: f64 = (t2/t_crit_water()).get::<ratio>();
    let theta_3: f64 = (t3/t_crit_water()).get::<ratio>();
    let theta_4: f64 = (t4/t_crit_water()).get::<ratio>();

    let kappa_t_1 = 0.451_570_597e-3 * Pressure::new::<megapascal>(1.0).recip();
    let kappa_t_2 = 0.105_138_803e0 * Pressure::new::<megapascal>(1.0).recip();
    let kappa_t_3 = 0.141_857_631e-1 * Pressure::new::<megapascal>(1.0).recip();
    let kappa_t_4 = 0.510_625_539e-1 * Pressure::new::<megapascal>(1.0).recip();

    // a1 and a4 values are not given in the table, 
    let a_1 = 0.0;
    let a_2 = 0.271_968_296e-1;
    let a_3 = 0.105_363_489e1;
    let a_4 = 0.0;

    let n3 = 0.135_882_142_589_674e1;
    let n4 = 0.508_474_576_271;
    let n5 = 1.5;

    dbg!(&(delta_1,delta_2,delta_3,delta_4));

    let a_test = small_a(n3, delta_1, theta_1, kappa_t_1, n4, n5);
    approx::assert_abs_diff_eq!(
        a_1,
        a_test,
        epsilon=0.0
        );
    let a_test = small_a(n3, delta_2, theta_2, kappa_t_2, n4, n5);
    approx::assert_relative_eq!(
        a_2,
        a_test,
        max_relative=1e-7
        );
    let a_test = small_a(n3, delta_3, theta_3, kappa_t_3, n4, n5);
    approx::assert_relative_eq!(
        a_3,
        a_test,
        max_relative=1e-8
        );
    
    let a_test = small_a(n3, delta_4, theta_4, kappa_t_4, n4, n5);
    approx::assert_abs_diff_eq!(
        a_4,
        a_test,
        epsilon=0.0
        );
}


#[test]
fn capital_a_test(){
    let t1 = ThermodynamicTemperature::new::<kelvin>(298.15);
    let t2 = ThermodynamicTemperature::new::<kelvin>(873.15);
    let t3 = ThermodynamicTemperature::new::<kelvin>(673.15);
    let t4 = ThermodynamicTemperature::new::<kelvin>(1173.15);

    let rho1 = MassDensity::new::<kilogram_per_cubic_meter>(0.997_047_435e3);
    let rho2 = MassDensity::new::<kilogram_per_cubic_meter>(0.260_569_558e2);
    let rho3 = MassDensity::new::<kilogram_per_cubic_meter>(0.523_371_289e3);
    let rho4 = MassDensity::new::<kilogram_per_cubic_meter>(0.377_584_848e2);

    let delta_1: f64 = (rho1/rho_crit_water()).get::<ratio>();
    let delta_2: f64 = (rho2/rho_crit_water()).get::<ratio>();
    let delta_3: f64 = (rho3/rho_crit_water()).get::<ratio>();
    let delta_4: f64 = (rho4/rho_crit_water()).get::<ratio>();

    let theta_1: f64 = (t1/t_crit_water()).get::<ratio>();
    let theta_2: f64 = (t2/t_crit_water()).get::<ratio>();
    let theta_3: f64 = (t3/t_crit_water()).get::<ratio>();
    let theta_4: f64 = (t4/t_crit_water()).get::<ratio>();

    let kappa_t_1 = 0.451_570_597e-3 * Pressure::new::<megapascal>(1.0).recip();
    let kappa_t_2 = 0.105_138_803e0 * Pressure::new::<megapascal>(1.0).recip();
    let kappa_t_3 = 0.141_857_631e-1 * Pressure::new::<megapascal>(1.0).recip();
    let kappa_t_4 = 0.510_625_539e-1 * Pressure::new::<megapascal>(1.0).recip();

    // a1 and a4 values are not given in the table, 
    let a_1 = 0.0;
    let a_2 = 0.917_330_648e-2;
    let a_3 = 0.176_976_803e0;
    let a_4 = 0.0;

    let n2 = 0.636_619_772_367_581;
    let n3 = 0.135_882_142_589_674e1;
    let n4 = 0.508_474_576_271;
    let n5 = 1.5;

    let b1 = 0.101_056_194e1;
    let b2 = 0.133_679_365e1;
    let b3 = 0.294_802_310e1;
    let b4 = 0.128_536_509e1;


    dbg!(&(delta_1,delta_2,delta_3,delta_4));

    let a_test = captial_a(n2, n3, 
        delta_1, theta_1, kappa_t_1, 
        n4, n5, b1);
    approx::assert_abs_diff_eq!(
        a_1,
        a_test,
        epsilon=0.0
        );
    let a_test = captial_a(n2, n3, 
        delta_2, theta_2, kappa_t_2, 
        n4, n5, b2);
    approx::assert_relative_eq!(
        a_2,
        a_test,
        max_relative=1e-7
        );
    let a_test = captial_a(n2, n3, 
        delta_3, theta_3, kappa_t_3, 
        n4, n5, b3);
    approx::assert_relative_eq!(
        a_3,
        a_test,
        max_relative=1e-8
        );
    
    let a_test = captial_a(n2, n3, 
        delta_4, theta_4, kappa_t_4, 
        n4, n5, b4);
    approx::assert_abs_diff_eq!(
        a_4,
        a_test,
        epsilon=0.0
        );
}


#[test]
fn lambda_2_test(){
    let t1 = ThermodynamicTemperature::new::<kelvin>(298.15);
    let t2 = ThermodynamicTemperature::new::<kelvin>(873.15);
    let t3 = ThermodynamicTemperature::new::<kelvin>(673.15);
    let t4 = ThermodynamicTemperature::new::<kelvin>(1173.15);

    let p1 = Pressure::new::<megapascal>(0.1);
    let p2 = Pressure::new::<megapascal>(10.0);
    let p3 = Pressure::new::<megapascal>(40.0);
    let p4 = Pressure::new::<megapascal>(20.0);



    // lambda_2_1 and lambda_2_4 values are not given in the table, 
    let lambda_2_1 = 0.0;
    let lambda_2_2 = 0.286_724_816e-1;
    let lambda_2_3 = 0.163_158_335e2;
    let lambda_2_4 = 0.0;




    let lambda_2_test = lambda_2_crit_enhancement_term_tp_single_phase(t1, p1);
    approx::assert_abs_diff_eq!(
        lambda_2_1,
        lambda_2_test,
        epsilon=0.0
        );
    let lambda_2_test = 
        lambda_2_crit_enhancement_term_tp_single_phase(t2, p2);
        approx::assert_relative_eq!(
        lambda_2_2,
        lambda_2_test,
        max_relative=1e-7
        );
    let lambda_2_test = 
        lambda_2_crit_enhancement_term_tp_single_phase(t3, p3);
    approx::assert_relative_eq!(
        lambda_2_3,
        lambda_2_test,
        max_relative=1e-5
        );
    
    let lambda_2_test = 
        lambda_2_crit_enhancement_term_tp_single_phase(t4, p4);
    approx::assert_abs_diff_eq!(
        lambda_2_4,
        lambda_2_test,
        epsilon=0.0
        );
}


#[test]
fn lambda_test_tp_flash(){
    let t1 = ThermodynamicTemperature::new::<kelvin>(298.15);
    let t2 = ThermodynamicTemperature::new::<kelvin>(873.15);
    let t3 = ThermodynamicTemperature::new::<kelvin>(673.15);
    let t4 = ThermodynamicTemperature::new::<kelvin>(1173.15);

    let p1 = Pressure::new::<megapascal>(0.1);
    let p2 = Pressure::new::<megapascal>(10.0);
    let p3 = Pressure::new::<megapascal>(40.0);
    let p4 = Pressure::new::<megapascal>(20.0);



    // lambda_2_1 and lambda_2_4 values are not given in the table, 
    let lambda_1_watt_per_m_k = 0.606_515_827e0;
    let lambda_2_watt_per_m_k = 0.870_767_659e-1;
    let lambda_3_watt_per_m_k = 0.412_517_936e0;
    let lambda_4_watt_per_m_k = 0.137_859_512e0;




    let lambda_2_test = lambda_tp_eqm_single_phase(t1, p1).get::<watt_per_meter_kelvin>();
    approx::assert_relative_eq!(
        lambda_1_watt_per_m_k,
        lambda_2_test,
        max_relative=1e-8
        );
    let lambda_2_test = lambda_tp_eqm_single_phase(t2, p2).get::<watt_per_meter_kelvin>();
        approx::assert_relative_eq!(
        lambda_2_watt_per_m_k,
        lambda_2_test,
        max_relative=1e-8
        );
    let lambda_2_test = lambda_tp_eqm_single_phase(t3, p3).get::<watt_per_meter_kelvin>();
    approx::assert_relative_eq!(
        lambda_3_watt_per_m_k,
        lambda_2_test,
        max_relative=1e-6
        );
    
    let lambda_2_test = lambda_tp_eqm_single_phase(t4, p4).get::<watt_per_meter_kelvin>();
    approx::assert_relative_eq!(
        lambda_4_watt_per_m_k,
        lambda_2_test,
        max_relative=1e-8
        );
}

/// ## Methodology
/// Two-phase (region 4) thermal conductivity via the `(p, h)` path,
/// `lambda_ph_eqm`. Before the 2026-07-14 fix, this panicked in the
/// two-phase dome: the critical-enhancement term
/// `lambda_2_crit_enhancement_term_tp_two_phase_estimate` delegated cp/cv/
/// kappa_t to the single-phase `(T, p)` routines, which `todo!()` in region
/// 4 (T and p are not independent on the saturation line). The fix
/// quality-weights the saturated region-1 (liquid) and region-2 (vapour)
/// values instead. This blocked any boiling steam-generator model, since
/// feedwater passes straight through the dome.
///
/// The check is a **physical-plausibility / regression** guard, not a
/// comparison to a tabulated IAPWS two-phase number (IAPWS R15-11 is a
/// single-phase formulation; there is no standard two-phase-mixture λ to
/// cite). At p = 1 bar (T_sat ≈ 372.76 K) it asserts that across the dome
/// λ is finite, strictly positive, and brackets between the saturated
/// vapour and saturated liquid conductivities (λ_g ≈ 0.025 W/m/K, λ_f ≈
/// 0.68 W/m/K for water at ~100 °C), and that increasing quality moves λ
/// monotonically toward the vapour value.
///
/// ## Results (2026-07-14)
/// Passes. At p = 1 bar the region-4 λ decreases monotonically from the
/// near-liquid value at low quality to the near-vapour value at high
/// quality; every sampled quality is finite and in (0.02, 0.75) W/m/K. No
/// panic anywhere in the dome.
#[test]
fn lambda_ph_two_phase_is_finite_and_bracketed() {
    use uom::si::available_energy::joule_per_kilogram;
    use uom::si::pressure::bar;
    use uom::si::thermal_conductivity::watt_per_meter_kelvin;
    use crate::interfaces::functional_programming::ph_flash_eqm::{
        lambda_ph_eqm, ph_flash_region, x_ph_flash,
    };
    use crate::interfaces::functional_programming::pt_flash_eqm::FwdEqnRegion;

    let p = Pressure::new::<bar>(1.0);

    // h_f ≈ 4.17e5, h_g ≈ 2.675e6 J/kg at 1 bar; sample interior of the dome.
    let enthalpies_j_per_kg = [6.0e5, 1.0e6, 1.5e6, 2.0e6, 2.5e6];

    let mut last_lambda = f64::INFINITY;
    let mut last_x = -1.0;
    for &h_val in enthalpies_j_per_kg.iter() {
        let h = AvailableEnergy::new::<joule_per_kilogram>(h_val);

        // Confirm the sample is genuinely two-phase (region 4).
        assert_eq!(
            ph_flash_region(p, h),
            FwdEqnRegion::Region4,
            "h = {h_val} J/kg at 1 bar should be two-phase"
        );

        let lambda = lambda_ph_eqm(p, h).get::<watt_per_meter_kelvin>();
        let x = x_ph_flash(p, h);

        assert!(
            lambda.is_finite() && lambda > 0.0,
            "two-phase lambda must be finite and positive; got {lambda} at h = {h_val}"
        );
        assert!(
            (0.02..0.75).contains(&lambda),
            "two-phase lambda {lambda} W/m/K at x = {x:.3} outside the \
             saturated vapour..liquid band (~0.025..0.68) at 1 bar"
        );
        // Higher quality (more vapour) ⇒ lower conductivity.
        assert!(
            x > last_x,
            "test enthalpies should sample increasing quality"
        );
        assert!(
            lambda <= last_lambda + 1e-9,
            "lambda should decrease monotonically with quality; \
             got {lambda} at x = {x:.3} after {last_lambda} at x = {last_x:.3}"
        );
        last_lambda = lambda;
        last_x = x;
    }
}
