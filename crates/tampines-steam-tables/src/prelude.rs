pub use crate::interfaces::functional_programming;
pub use crate::interfaces::object_oriented_programming::*;

/// Bounds-checked, `Result`-returning facade over the panicking flash
/// internals — see [`crate::interfaces::checked`]. Import as
/// `tampines_steam_tables::prelude::checked::try_h_tp_eqm_single_phase`
/// (etc.) when out-of-range input must not kill the calling thread.
pub use crate::interfaces::checked;

pub use crate::steam_turbine_equations::
converging_diverging_nozzles::isentropic_converging_nozzle::
get_choked_flow_massrate_and_state_from_stagnation_properties_and_area;
