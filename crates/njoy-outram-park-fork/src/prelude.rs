//! Convenience re-exports for downstream users.
//!
//! `use njoy_outram_park_fork::prelude::*;` brings the common public types into
//! scope. Extend the `pub use` list whenever a new public item is added, per the
//! workspace porting workflow.

pub use crate::endf::EndfKey;
pub use crate::endf::{Cont, List, Tab1, Tab2, Tape};
pub use crate::error::NjoyError;
pub use crate::broadr::{broaden_result, broadening_limit, doppler_broaden, doppler_broaden_below};
pub use crate::gaspr::{gas_channel, gas_yield_for, GasChannel, GasProduction, GasSpecies, GasYield};
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

// ── The high-level entry points (GitHub #871) ────────────────────────────────
pub use crate::process::{
    ace, cross_sections, thermal_ace, thermal_scattering, CrossSectionSettings, NuclearCrossSections,
    PurrSettings, ThermalScatteringLaw, ThermalSettings,
};

// ── A whole library from a folder, zip or upload; ACE files by header; zip
// archives in memory (GitHub #935, #936) ───────────────────────────────────────
pub use crate::acer::ace_set::{AceLookupError, AceSet, AceSetTable};
pub use crate::archive::{write_zip, ZipArchive, ZipEntry, ZipMethod};
pub use crate::endf_library::{
    fuzzy_score, generate_ace, generate_thermal_ace, suggest_thermal_partner, AceRequest,
    EndfHeader, EndfLibraryIndex, GeneratedAce, IndexedTape, LibraryLayout, LibrarySource,
    LookupError, SubLibrary,
};
