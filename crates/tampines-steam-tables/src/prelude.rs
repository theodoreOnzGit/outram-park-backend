pub use crate::interfaces::functional_programming;
pub use crate::interfaces::object_oriented_programming::*;

/// Bounds-checked, `Result`-returning facade over the panicking flash
/// internals — see [`crate::interfaces::checked`]. Import as
/// `tampines_steam_tables::prelude::checked::try_h_tp_eqm_single_phase`
/// (etc.) when out-of-range input must not kill the calling thread.
pub use crate::interfaces::checked;

pub use crate::steam_turbine_equations::converging_diverging_nozzles::isentropic_converging_nozzle::get_choked_flow_massrate_and_state_from_stagnation_properties_and_area;

// ── Functional entry points, exported as SYMBOLS not just module paths ───────
//
// Added after an API dogfood run (gh #58). The lines above re-export
// `functional_programming` as a *module name*: `use prelude::*` therefore gives
// you a path to walk, not functions you can call or discover. An agent asked to
// build a Rankine cycle reported it could not tell which flash functions
// existed and had to read the source to find them -- the prelude offered no
// hint that `ps_flash_eqm` or `sat_temp_4` were there at all.
//
// Glob-importing each flash module puts the actual function names in scope, so
// `h_ps_eqm`, `t_ph_eqm` and friends are reachable and, more importantly,
// discoverable by completion.
pub use crate::interfaces::functional_programming::hs_flash_eqm::*;
pub use crate::interfaces::functional_programming::ph_flash_eqm::*;
pub use crate::interfaces::functional_programming::ps_flash_eqm::*;
pub use crate::interfaces::functional_programming::pt_flash_eqm::*;

// Saturation line. These were in no prelude at all, despite being the first
// thing anyone touching steam needs: "what is the boiling point at this
// pressure?"
pub use crate::region_4_vap_liq_equilibrium::sat_pressure::sat_pressure_4;
pub use crate::region_4_vap_liq_equilibrium::sat_temp::sat_temp_4;

// ── A units gotcha worth stating once ────────────────────────────────────────
//
// Specific entropy is carried as `uom::si::f64::SpecificHeatCapacity`, and the
// dogfood agent lost time wondering why entropy was "a heat capacity".
//
// It is not a modelling mistake. Specific entropy and specific heat capacity
// are both J/(kg K) and are therefore the SAME DIMENSION, so `uom` -- which
// keys types on dimension -- cannot tell them apart and offers no separate
// `SpecificEntropy`. Read the type as "the J/(kg K) quantity", and reach for
// `uom::si::specific_heat_capacity::joule_per_kilogram_kelvin` when you need
// to get a number out of one.
//
// The same coincidence bites elsewhere: specific enthalpy and specific internal
// energy are both J/kg, which `uom` calls `AvailableEnergy` -- not
// `SpecificEnergy`, which does not exist. Guessing that name is a documented
// friction point.

/// Worked check that this prelude is sufficient on its own: saturation
/// properties and a backward flash, with no deeper import from this crate.
///
/// ```
/// use tampines_steam_tables::prelude::*;
/// use uom::si::f64::*;
/// use uom::si::pressure::kilopascal;
/// use uom::si::thermodynamic_temperature::degree_celsius;
///
/// // Boiling point of water at 101.325 kPa is 99.97 C by IF97.
/// let p = Pressure::new::<kilopascal>(101.325);
/// let t_sat = sat_temp_4(p);
/// assert!((t_sat.get::<degree_celsius>() - 99.97).abs() < 0.02);
/// ```
pub mod _prelude_is_sufficient {}
