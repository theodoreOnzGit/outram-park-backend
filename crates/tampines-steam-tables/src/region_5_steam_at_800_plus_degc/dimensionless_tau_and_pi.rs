use uom::si::f64::*;
use uom::si::pressure::megapascal;
use uom::si::thermodynamic_temperature::kelvin;
/// Returns the region-5 tau (dimensionless reduced temperature, 1000/T)
/// Temperature is assumed to be in K
pub fn tau_5(t: ThermodynamicTemperature) -> f64 {
    // Temperature is assumed to be in K
    let t_kelvin = t.get::<kelvin>();
    1000.0 / t_kelvin
}

/// Returns the region-5 pi (dimensionless reduced pressure, p/1 MPa)
/// Pressure is assumed to be in Pa
pub fn pi_5(p: Pressure) -> f64 {
    let p_megapascals = p.get::<megapascal>();
    // Pressure is assumed to be in Pa
    p_megapascals / (1.0)
}
