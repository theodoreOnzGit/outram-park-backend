// boon-lay: BOmbardment of neutrons On Nuclides with Lagrangian transport and
// transmutation Yields.
//
// Lagrangian radionuclide transport, decay, fission, and diffusion for
// TRISO/HTGR/FHR fuel particles.
//
// See CLAUDE.md for migration notes from the standalone crate (uom 0.37 → 0.38,
// egui 0.29 → 0.34) and the source copy checklist.

/// prelude is here for easy imports
pub mod prelude;

/// import the nuclide enum
pub use fission_yields_data::prelude::Nuclide;
/// import all nuclides into this crate
pub use fission_yields_data::prelude::Nuclide::*;

/// Raw serde structs for the ENDF-8 depletion XML data.
/// Copy from `../boon-lay/src/decay_xml_info_serde/mod.rs`.
pub mod decay_xml_info_serde;

/// Nuclide struct with half-life, decay types, and branching ratios.
/// Parsed from the ENDF-8 XML via `decay_xml_info_serde`.
/// Copy from `../boon-lay/src/nuclide_reaction_and_decay_data/`.
pub mod nuclide_reaction_and_decay_data;

/// Lagrangian decay simulator: stochastic decay chains, single-nuclide MC,
/// Lagrangian diffusion in TRISO layers, CSG geometry.
/// Copy from `../boon-lay/src/lagrangian_decay_simulator/`.
pub mod lagrangian_decay_simulator;

/// Lagrangian transmutation and fission simulator (stub in original).
/// Copy from `../boon-lay/src/lagrangian_transmutation_and_fission_simulator/`.
pub mod lagrangian_transmutation_and_fission_simulator;
