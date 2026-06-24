
/// prelude is here for easy imports
pub mod prelude;

/// import the nuclide enum
pub use fission_yields_data::prelude::Nuclide;
/// import all nuclides into this crate
pub use fission_yields_data::prelude::Nuclide::*;

/// this contains the raw information 
/// based on pwr neutron spectrum
pub mod decay_xml_info_serde;

/// this is the struct that converts the SerdeNuclideData to 
/// NuclideReactionAndDecayData 
pub mod nuclide_reaction_and_decay_data;


/// this is the part that deals with decay simulation in lagrangian 
/// or monte carlo bit 
/// this part deals only with the terminal user interface
pub mod lagrangian_decay_simulator;

/// this is the part that deals with transmutation and fission 
/// simulation in lagrangian 
pub mod lagrangian_transmutation_and_fission_simulator;
