//! Axial (end-to-end) connections for a `FluidArray`.
//!
//! These submodules attach heat-transfer entities to the front (higher
//! coordinate) or back (lower coordinate) faces of the array along its flow
//! axis: other single control volumes, boundary conditions (constant heat flux
//! in W/m^2, constant heat rate in W, or constant temperature in K), and other
//! array CVs (fluid arrays or solid columns). Axial links carry both advective
//! enthalpy transport (when mass flows across the face) and conduction; lateral
//! (radial) coupling lives in the sibling `lateral_connection` module instead.

// for axial connections, we can connect heat transfer entities to it
// or away from it,
//
// these could be boundary conditions or other control volumes
//
// in my original code, these are quite well abstracted as all you see 
// is connection to new heat transfer entities


/// the baseline for all interactions with other array cvs 
/// is the interaction with single cvs and bcs 
/// this module takes care of the interactions with single cvs
pub mod interaction_with_single_cv;

/// the baseline for all interactions with other array cvs 
/// is the interaction with single cvs and bcs 
/// this module takes care of the interactions with single cvs
pub mod interaction_with_bc;

/// this module takes care of the interactions with other array cvs 
/// both solid arrays and fluid arrays
pub mod interaction_with_array_cv;


