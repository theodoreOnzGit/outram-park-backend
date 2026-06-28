//! Named type aliases for all dimensioned quantities used in this crate.
//!
//! `uom` raw generic types (`Quantity<ISQ<...>, SI<f64>, f64>`) are unreadable in
//! editor hover-tips. These aliases are the first thing a user sees when they call
//! a public function — they replace the raw generics with names that make physical
//! sense.
//!
//! All quantities use 64-bit floats and SI base units. To create a value or extract
//! a numeric reading:
//!
//! ```
//! use njoy_outram_park_fork::units::{NeutronEnergy, CrossSection, Temperature};
//! use uom::si::energy::electronvolt;
//! use uom::si::area::barn;
//! use uom::si::thermodynamic_temperature::kelvin;
//!
//! // Create
//! let e  = NeutronEnergy::new::<electronvolt>(0.0253);   // thermal, 0.0253 eV
//! let xs = CrossSection::new::<barn>(583.0);             // U-235 fission at thermal
//! let t  = Temperature::new::<kelvin>(293.6);            // room temperature
//!
//! // Read back
//! let e_ev    = e.get::<electronvolt>();
//! let xs_barn = xs.get::<barn>();
//! let t_k     = t.get::<kelvin>();
//! ```

/// Neutron (or incident particle) kinetic energy.
///
/// ENDF evaluations store energies in eV; use `uom::si::energy::electronvolt`
/// to create and extract values. Common conversion factors:
/// - 1 eV = 1.602 176 634 × 10⁻¹⁹ J
/// - 1 MeV = 10⁶ eV
pub type NeutronEnergy = uom::si::f64::Energy;

/// Microscopic nuclear cross section [m²].
///
/// ENDF evaluations tabulate cross sections in barns; use `uom::si::area::barn`
/// to create and extract values. 1 barn = 10⁻²⁸ m².
pub type CrossSection = uom::si::f64::Area;

/// Thermodynamic temperature [K].
///
/// Used for Doppler broadening (`broaden_xs`). Use
/// `uom::si::thermodynamic_temperature::kelvin` to create and extract values.
pub type Temperature = uom::si::f64::ThermodynamicTemperature;
