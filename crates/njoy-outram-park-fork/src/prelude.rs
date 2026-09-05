//! Convenience re-exports for downstream users.
//!
//! `use njoy_outram_park_fork::prelude::*;` brings the common public types into
//! scope. Extend the `pub use` list whenever a new public item is added, per the
//! workspace porting workflow.

pub use crate::endf::EndfKey;
pub use crate::endf::{Cont, List, Tab1, Tab2, Tape};
pub use crate::error::NjoyError;
pub use crate::broadr::doppler_broaden;
pub use crate::gaspr::{GasProduction, GasSpecies};
pub use crate::heatr::Kerma;
pub use crate::reconr::{reconr, ReconrConfig, ReconrResult};

pub use crate::modules::NjoyModule;

// ── Multigroup collapse ───────────────────────────────────────────────────────
//
// Added after an API dogfood run (gh #58): an agent asked to take an ENDF file
// through RECONR -> BROADR -> multigroup collapse using only this prelude got
// stuck dead after broadening. It searched the prelude for "group" and
// "collapse", found nothing, and reasonably concluded the crate could not do it
// -- when in fact `bake_mgxs.rs` does exactly that, by importing from
// `nuclear_data` and `errorr::groups` directly.
//
// A pipeline whose last stage is invisible from the prelude may as well not
// exist for anyone learning the crate, so the last stage is exported here.
pub use crate::nuclear_data::{Mgxs, MgxsLibrary, WeightingSpectrum, ENDF_MAX_ENERGY_EV};
pub use crate::errorr::groups::{neutron_group_structure, NeutronGroupStructure};
pub use crate::groupr::panel::group_integral;
pub use crate::reconr::reconr_background;
