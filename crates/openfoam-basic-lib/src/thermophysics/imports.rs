/// Common uom re-exports for thermophysics implementation files.
///
/// Every EOS / thermo / transport source file starts with
/// `use crate::thermophysics::imports::*;` instead of repeating the full
/// type/unit import block.  From outside the crate the same symbols are
/// reachable as:
///
/// ```rust
/// use openfoam_basic_lib::thermophysics::imports::*;
/// let p = Pressure::new::<pascal>(101325.0);
/// assert!(p.get::<pascal>() > 0.0);
/// ```

// ── quantity types ────────────────────────────────────────────────────────────
pub use uom::si::f64::{
    AvailableEnergy, DynamicViscosity, MassDensity, MolarMass,
    Pressure, Ratio, SpecificHeatCapacity, ThermalConductivity,
    ThermodynamicTemperature,
};

// ── unit markers (used in ::new::<unit>() and .get::<unit>()) ────────────────
pub use uom::si::available_energy::joule_per_kilogram;
pub use uom::si::dynamic_viscosity::pascal_second;
pub use uom::si::mass_density::kilogram_per_cubic_meter;
pub use uom::si::molar_mass::{gram_per_mole, kilogram_per_mole};
pub use uom::si::pressure::pascal;
pub use uom::si::ratio::ratio;
pub use uom::si::specific_heat_capacity::joule_per_kilogram_kelvin;
pub use uom::si::thermal_conductivity::watt_per_meter_kelvin;
pub use uom::si::thermodynamic_temperature::kelvin;

// ── crate-local ───────────────────────────────────────────────────────────────
pub use crate::thermophysics::quantities::Compressibility;
