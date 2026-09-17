//! Convenience re-exports — the intended entry point for this crate.
//!
//! ```
//! use teh_o_prke::prelude::*;
//! ```
//!
//! This brings every type needed to set up and run a point-kinetics
//! calculation into scope, so a caller never has to know which module an item
//! lives in. **A public item reachable only by its full module path is not
//! considered exposed** — if you add one, add it here in the same change.
//!
//! Advancing the PRKE state is [`SixGroupPRKE::step_implicit`] (and
//! `step_explicit`). Those are short aliases for the far longer
//! `solve_next_timestep_precursor_concentration_and_neutron_pop_vector_*`
//! names, which still work.
//!
//! The completeness test is executable: this crate's own examples and doc
//! examples must compile with `use teh_o_prke::prelude::*;` as their only
//! import from this crate.
//!
//! ```
//! use teh_o_prke::prelude::*;
//! use uom::si::f64::*;
//! use uom::si::time::second;
//! use uom::si::ratio::ratio;
//! use uom::si::volumetric_number_rate::per_cubic_meter_second;
//!
//! let mut prke = SixGroupPRKE::default();
//!
//! let dt: Time = Time::new::<second>(0.1);
//! let reactivity: Ratio = Ratio::new::<ratio>(0.0);
//! let generation_time: Time = Time::new::<second>(1.0e-4);
//! let source = VolumetricNumberRate::new::<per_cubic_meter_second>(1.0);
//!
//! // Zero reactivity: the population should hold steady against the source.
//! prke.solve_next_timestep_precursor_concentration_and_neutron_pop_vector_implicit(
//!     dt, reactivity, generation_time, source,
//! )?;
//!
//! let _n = prke.get_current_neutron_population_density();
//! let _beta = prke.get_total_delayed_fraction();
//! # Ok::<(), teh_o_prke::prelude::TehOPrkeError>(())
//! ```

// Point reactor kinetics: the six-group precursor solver and its nuclide data.
pub use crate::zero_power_prke::six_group_precursor_prke::six_group_constants::FissioningNuclideType;
pub use crate::zero_power_prke::six_group_precursor_prke::{DecayConstant, SixGroupPRKE};

// Delayed neutrons as a standalone layer, for coupling into an external solver.
pub use crate::delayed_neutron_layer::{DelayedNeutronLayer, NUM_DELAYED_GROUPS};

// Reactivity feedback: fuel temperature, control rods, the six-factor formula,
// and fission-product poisoning.
pub use crate::control_rod_feedback::obtain_rod_worth_cylinder;
pub use crate::feedback_mechanisms::fission_product_poisons::Xenon135Poisoning;
pub use crate::feedback_mechanisms::SixFactorFormulaFeedback;
pub use crate::fuel_temperature_feedback::{
    obtain_fuel_temperature_feedback_coeff_thermal_spectrum,
    obtain_fuel_temperature_reactivity_feedback_thermal_spectrum, SimpleFuelTemperatureFeedback,
};

// Decay heat after shutdown.
pub use crate::decay_heat::{
    DecayHeat, FissioningNuclide, DECAY_HEAT_GROUPS, NOMINAL_ENERGY_PER_FISSION_MEV,
};

// Prompt-critical excursions (Nordheim-Fuchs).
pub use crate::nordheim_fuchs::NordheimFuchsExactTimestepper;

// Time integration and the dense linear solver underneath it.
pub use crate::matrix::{MatrixError, SquareMatrix};
pub use crate::time_stepping::openfoam_ode_system::ODESystem;
pub use crate::time_stepping::openfoam_rfk45::RKF45;

// The crate's error type, so a caller can write a signature that propagates it
// without hunting for the module it is defined in.
pub use crate::teh_o_prke_error::TehOPrkeError;
