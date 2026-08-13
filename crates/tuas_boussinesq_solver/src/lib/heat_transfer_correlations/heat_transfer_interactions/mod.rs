//! Heat-transfer interactions between two control volumes.
//!
//! A heat-transfer interaction pairs a geometry (slab, cylinder, or sphere)
//! with a physical mechanism (conduction, convection, advection, or
//! radiation) and produces either a thermal conductance (watts per kelvin) or
//! a heat flow (watts) between the two control volumes.
//!
//! Submodules:
//!
//! - [`conductance`] — solid-solid and solid-fluid conduction/convection
//!   conductances (watts per kelvin), plus the radiation conductance helper.
//! - [`advection`] — fluid-fluid enthalpy transport (watts) driven by mass
//!   flow between control volumes.
//! - [`heat_transfer_geometry`] — enums/structs describing the geometry of an
//!   interaction (curved-surface fluid/solid arrangement, Cartesian layers).
//! - [`heat_transfer_interaction_enums`] — the [`HeatTransferInteractionType`]
//!   enum enumerating every supported interaction and its `(p, T)` → thermal
//!   conductance dispatch.
//!
//! [`HeatTransferInteractionType`]: heat_transfer_interaction_enums::HeatTransferInteractionType

/// for solid-solid and solid-fluid interaction, heat transfer can
/// be expressed in terms of thermal conductance or thermal
/// resistance
///
/// this module contains functions for these
pub mod conductance;
pub use conductance::*;

/// for fluid-fluid interaction, where fluid flows and carries
/// heat from one control volume to another, we call this advection
/// functions to calculate advection and placed in this module
pub mod advection;
pub use advection::*;

/// for heat transfer interactions between two control volumes,
/// there will be a certain geometry, this is usually a cylinder,
/// sphere or slab (straight line)
///
/// The enums and structs responsible for handling this information are
/// stored here
pub mod heat_transfer_geometry;

/// for heat transfer interactions, there are various types,
/// for example, it could be advection, conduction or a simple fixed
/// heat addition
///
/// these are represented in heat transfer enums which are stored here
pub mod heat_transfer_interaction_enums;

/// unit tests for heat transfer interactions
#[cfg(test)]
pub mod tests;
