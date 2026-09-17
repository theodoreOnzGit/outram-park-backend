use subregion_2a::t_ps_2a;
use subregion_2b::t_ps_2b;
use subregion_2c::t_ps_2c;
use uom::si::{f64::*, pressure::megapascal, specific_heat_capacity::kilojoule_per_kilogram_kelvin};

/// Subregion 2a backward `(p,s)` correlation (temperature from pressure and entropy).
pub mod subregion_2a;
/// Subregion 2b backward `(p,s)` correlation (temperature from pressure and entropy).
pub mod subregion_2b;
/// Subregion 2c backward `(p,s)` correlation (temperature from pressure and entropy).
pub mod subregion_2c;

/// Region 2 backward `(p,s)` equation: temperature T (K) from pressure (Pa)
/// and specific entropy (J/(kg·K)). Dispatches to the 2a / 2b / 2c subregion
/// correlations by the 4 MPa (2a|2b) and 5.85 kJ/(kg·K) (2b|2c) boundaries.
#[inline]
pub fn t_ps_2(p: Pressure, s: SpecificHeatCapacity) -> ThermodynamicTemperature {
    let p_boundary_2a2b = Pressure::new::<megapascal>(4.0);
    let s_boundary_2b2c = SpecificHeatCapacity::new::<kilojoule_per_kilogram_kelvin>(5.85);

    if p <= p_boundary_2a2b {
        return t_ps_2a(p, s);
    };

    if s >= s_boundary_2b2c {
        return t_ps_2b(p, s);
    } else {
        return t_ps_2c(p, s);
    };
}
