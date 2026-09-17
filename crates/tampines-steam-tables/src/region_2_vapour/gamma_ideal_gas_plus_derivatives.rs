use petir::mathf::RealMath;

use super::{pi_2, tau_2, REGION_2_COEFFS_IDEAL};
use uom::si::f64::*;

/// Returns the region-2 ideal gamma
/// Temperature is assumed to be in K
/// Pressure is assumed to be in Pa
pub fn gamma_2_ideal(t: ThermodynamicTemperature, p: Pressure) -> f64 {
    let pi = pi_2(p);
    let mut sum: f64 = 0.0;
    let tau = tau_2(t);
    for coefficient in REGION_2_COEFFS_IDEAL {
        let ji = coefficient[0];
        let ni = coefficient[1];
        sum += ni * tau.r_powf(ji);
    }
    pi.r_ln() + sum
}

/// Returns the region-2 ideal gamma_tau
/// Temperature is assumed to be in K
/// Pressure is assumed to be in Pa
pub fn gamma_tau_2_ideal(t: ThermodynamicTemperature, _p: Pressure) -> f64 {
    let mut sum: f64 = 0.0;
    let tau = tau_2(t);
    for coefficient in REGION_2_COEFFS_IDEAL {
        let ji = coefficient[0];
        let ni = coefficient[1];
        sum += ni * ji * tau.r_powf(ji - 1.0);
    }
    sum
}

/// Returns the region-2 ideal gamma_tau_tau
/// Temperature is assumed to be in K
/// Pressure is assumed to be in Pa
pub fn gamma_tau_tau_2_ideal(t: ThermodynamicTemperature, _p: Pressure) -> f64 {
    let mut sum: f64 = 0.0;
    let tau = tau_2(t);
    for coefficient in REGION_2_COEFFS_IDEAL {
        let ji = coefficient[0];
        let ni = coefficient[1];
        sum += ni * ji * (ji - 1.0) * tau.r_powf(ji - 2.0);
    }
    sum
}

/// Returns the region-2 ideal gamma_pi
/// Temperature is assumed to be in K
/// Pressure is assumed to be in Pa
pub fn gamma_pi_2_ideal(_t: ThermodynamicTemperature, p: Pressure) -> f64 {
    1.0 / pi_2(p)
}
