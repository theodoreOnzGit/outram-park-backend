//! Enum layer that unifies thermal control volumes (CVs) and boundary
//! conditions (BCs) behind a single [`HeatTransferEntity`] type so a solver
//! can hold, link, advance, and interrogate either kind through one API.
//!
//! Module map:
//! - [`cv_types`] — the [`cv_types::CVType`] enum wrapping the control-volume
//!   variants (single node, fluid array, solid array) and their `From`/
//!   `TryFrom` conversions.
//! - [`bc_types`] — convenience constructors that build boundary-condition
//!   [`HeatTransferEntity`] values (constant temperature in K, heat flux in
//!   W/m^2, heat addition in W, adiabatic).
//! - [`preprocessing`] — sets up a heat-transfer problem: linking entities via
//!   heat-transfer interactions (single CV–BC and CV–CV thermal connections /
//!   conductance links in W/K), setting mass flowrates in kg/s, and computing
//!   mesh-stability timesteps in seconds.
//! - [`calculation`] — advances a control volume by one timestep (in seconds),
//!   converting accumulated enthalpy-change rates into the next-timestep state.
//! - [`postprocessing`] — extracts temperatures (K) and densities (kg/m^3)
//!   from an entity.
//! - [`type_conversion`] — `Into`/`TryFrom`/`TryInto` between the concrete CV
//!   and BC types and [`HeatTransferEntity`].
//! - [`conversion_to_data_advection`] — builds a `DataAdvection` interaction
//!   from two heat transfer entities.
//! - [`tests`] — mixing-joint and CIET-heater verification tests.

use self::cv_types::CVType;
use crate::tuas_lib_error::TuasLibError;
use crate::boundary_conditions::BCType;
/// Contains entities which transfer heat and interact with each
/// other
///
/// for example, control volumes and boundary conditions
#[derive(Debug, Clone, PartialEq)]
pub enum HeatTransferEntity {
    /// Contains a list of ControlVolumeTypes
    ControlVolume(CVType),
    /// Contains a list of Boundary conditions
    BoundaryConditions(BCType),
}

impl HeatTransferEntity {
    /// allows the user to override the heat transfer entity
    pub fn set(&mut self, user_input_hte: HeatTransferEntity) -> Result<(), TuasLibError> {
        *self = user_input_hte;

        Ok(())
    }
}

/// all the types of Control volumes are represented in an enum
/// to abstract away the complications of connecting different types
/// of control volumes.
pub mod cv_types;

/// converts to and from boundary conditions
pub mod bc_types;

/// preprocessing
///
/// this module contains abstraction pertaining
/// to how to set up a heat transfer problem
///
/// This means setting up the timestep, mass flowrates and how
/// heat transfer entities are linked to each other via heat
/// transfer interactions
pub mod preprocessing;

/// postprocessing contains functions to obtain temperature profiles
/// of the HeatTransferEntity
pub mod postprocessing;

/// calculation modules deal mainly with advancing timestep
pub mod calculation;

/// type conversion
/// converts underlying nested enums into HeatTransferEntity objects
pub mod type_conversion;

/// convert to data_advection
/// that is to say, you can construct a DataAdvection struct from
/// a HeatTransferEntity
pub mod conversion_to_data_advection;

/// tests
#[cfg(test)]
pub mod tests;
