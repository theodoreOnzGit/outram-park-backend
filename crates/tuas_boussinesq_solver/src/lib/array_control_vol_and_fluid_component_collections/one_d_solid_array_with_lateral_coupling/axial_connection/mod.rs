//! Axial (end-to-end) connections for a [`super::SolidColumn`].
//!
//! These modules attach heat-transfer entities to the back or front boundary
//! node of the solid array: another single control volume, a boundary
//! condition (constant heat flux, constant heat rate, or constant
//! temperature), or another array control volume (solid column or fluid
//! array). Advection interactions are rejected — a solid array only conducts.
//! Heat flows through the shared end node, so the linking helpers reduce to
//! the single-CV pairwise interaction routines.

/// the baseline for all interactions with other array cvs
/// is the interaction with single cvs and bcs
/// this module takes care of the interactions with single cvs
pub mod interaction_with_single_cv;

/// the baseline for all interactions with other array cvs
/// is the interaction with single cvs and bcs
/// this module takes care of the interactions with boundary conditions
/// (constant heat flux, constant heat rate, constant temperature)
pub mod interaction_with_bc;

/// this module takes care of the interactions with other array cvs 
/// both solid arrays and fluid arrays
pub mod interaction_with_array_cv;


