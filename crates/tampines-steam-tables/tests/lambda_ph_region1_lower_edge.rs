// SPDX-License-Identifier: AGPL-3.0-only
//
// gh:#904: `lambda_ph_eqm` panicked inside the crate's own (p,h) validity
// range, at the low-enthalpy edge of low isobars.

//! # `lambda_ph_eqm` at the 273.15 K edge of Region 1 (gh:#904)
//!
//! **What failed.** At `(p, h) = (611.657 Pa, 0)` and `(1000 Pa, 0)` the
//! (p,h) flash puts the state in IF97 Region 1, and the Region 1 backward
//! equation `T(p,h)` gives `T = 273.1384 K` and `273.1383 K`: about 21 mK
//! below the forward temperature (~273.16 K), inside the 25 mK consistency
//! IAPWS-IF97 allows the backward equation. `lambda_ph_eqm` then re-derived
//! the region from `(T, p)` for the critical-enhancement term, found
//! `T < 273.15 K`, and panicked ("t,p flashing at eqm out of bounds!").
//!
//! **Methodology.** The fix routes the critical-enhancement term by the
//! `(p,h)` region, as this crate's own `cp_ph_eqm`, `cv_ph_eqm` and
//! `kappa_t_ph_eqm` already do (Region 1 evaluates the Region 1 basic
//! equation at the backward `T`). The checks: no panic at both points; the
//! result is finite and within 0.1 % of `lambda_ph_eqm` at the nearest
//! Region 1 point at 1000 Pa where `T >= 273.15 K` and the old route worked
//! (the property is smooth here; 0.1 % is far above the ~1e-5 change of
//! lambda over 21 mK or over 400 Pa); and the
//! value at a Region 1 point well inside the range is unchanged.
//!
//! **Results (2026-11-10, x86_64-linux, release):** pass. Before the fix
//! (develop `a836e2f07`) both edge points panicked.

use tampines_steam_tables::interfaces::functional_programming::ph_flash_eqm::{
    lambda_ph_eqm, ph_flash_region, t_ph_eqm,
};
use tampines_steam_tables::interfaces::functional_programming::pt_flash_eqm::FwdEqnRegion;
use uom::si::available_energy::joule_per_kilogram;
use uom::si::f64::{AvailableEnergy, Pressure};
use uom::si::pressure::pascal;
use uom::si::thermal_conductivity::watt_per_meter_kelvin;
use uom::si::thermodynamic_temperature::kelvin;

fn ph(p_pa: f64, h_jkg: f64) -> (Pressure, AvailableEnergy) {
    (
        Pressure::new::<pascal>(p_pa),
        AvailableEnergy::new::<joule_per_kilogram>(h_jkg),
    )
}

/// The anchor: the nearest Region 1 point at 1000 Pa whose backward T is at
/// or above 273.15 K, where the old (T, p) route works. At 611.657 Pa there
/// is no such point: Region 1 there spans only h ~ -0.04 to ~0.6 J/kg, all
/// with T < 273.15 K, and h = 10 J/kg is already in the two-phase dome. Liquid
/// conductivity changes by far less than 0.1 % between 611 and 1000 Pa.
fn anchor_lambda() -> f64 {
    let p = Pressure::new::<pascal>(1000.0);
    let mut h_n = 0.0;
    loop {
        let h = AvailableEnergy::new::<joule_per_kilogram>(h_n);
        assert_eq!(ph_flash_region(p, h), FwdEqnRegion::Region1);
        if t_ph_eqm(p, h).get::<kelvin>() >= 273.15 {
            return lambda_ph_eqm(p, h).get::<watt_per_meter_kelvin>();
        }
        h_n += 10.0;
    }
}

#[test]
fn lambda_ph_does_not_panic_where_backward_t_is_below_273_15_k() {
    let lambda_n = anchor_lambda();
    for p_pa in [611.657, 1000.0] {
        let (p, h) = ph(p_pa, 0.0);
        assert_eq!(ph_flash_region(p, h), FwdEqnRegion::Region1);
        let t = t_ph_eqm(p, h).get::<kelvin>();
        assert!(t < 273.15, "the reproducer needs T below 273.15 K, got {t}");

        let lambda = lambda_ph_eqm(p, h).get::<watt_per_meter_kelvin>();
        assert!(lambda.is_finite());
        println!("p = {p_pa} Pa: T = {t:.4} K, lambda = {lambda:.6}; anchor {lambda_n:.6}");
        assert!((lambda / lambda_n - 1.0).abs() < 1e-3);
    }
}

/// Liquid water at 1 MPa and h = 400 kJ/kg (about 95 C): IAPWS 2011
/// thermal conductivity there is about 0.677 W/(m K); the route change does
/// not touch it (the Region 1 route and the old (T, p) route agree off the
/// edge, so this pins that the value is still the physical one).
#[test]
fn lambda_ph_region1_interior_unchanged() {
    let (p, h) = ph(1.0e6, 400.0e3);
    assert_eq!(ph_flash_region(p, h), FwdEqnRegion::Region1);
    let lambda = lambda_ph_eqm(p, h).get::<watt_per_meter_kelvin>();
    assert!((lambda - 0.677).abs() < 0.01, "lambda = {lambda}");
    // Bit-identical to the pre-fix value (develop `a836e2f07`, measured
    // 2026-11-10): 6.758120943956557e-1.
    assert_eq!(lambda, 6.758120943956557e-1);
}
