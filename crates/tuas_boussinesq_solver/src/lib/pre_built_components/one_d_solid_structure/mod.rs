//! One-dimensional solid conduction structure.
//!
//! This module provides [`SolidStructure`], a standalone 1D solid heat
//! structure (for example a hollow cylinder) discretised into an axial column
//! of solid control volumes. Unlike the fluid-component builders, it carries no
//! internal fluid array — it is a pure conduction body that can be laterally
//! coupled to an ambient temperature boundary and/or fed a heat source, and
//! optionally used as an additional wall / structural mass in a loop model.
//!
//! Units: lengths and diameters in metres (m), cross-sectional area in square
//! metres (m^2), temperatures in kelvin (K), pressure in pascals (Pa), power in
//! watts (W) and thermal conductance in watts per kelvin (W/K). All public
//! signatures use `uom` dimensioned quantities.
//!
//! Submodules:
//! - [`preprocessing`] — build the lateral conductances and boundary-condition
//!   links (ambient-temperature and mixed power/ambient couplings).
//! - [`calculation`] — advance the structure one timestep.
//! - [`postprocessing`] — read back the nodal temperature vector.
use crate::array_control_vol_and_fluid_component_collections::one_d_solid_array_with_lateral_coupling::SolidColumn;
use crate::boussinesq_thermophysical_properties::SolidMaterial;

use super::heat_transfer_entities::cv_types::CVType;
use super::heat_transfer_entities::HeatTransferEntity;
use uom::si::f64::*;

/// A one-dimensional solid conduction structure.
///
/// Represents a solid body (typically a hollow cylinder) as a single radial
/// layer of solid control volumes discretised axially. There is no internal
/// fluid; heat enters through a lateral coupling to an ambient boundary and/or
/// a user-supplied power source, and conducts along the axial nodes.
///
/// The standard assumption is that each axial end boundary has no conduction
/// heat transfer in the axial direction (zero-power boundary condition) unless
/// the user links something else to it.
#[derive(Clone,Debug,PartialEq)]
pub struct SolidStructure {

    inner_nodes: usize,

    /// this HeatTransferEntity represents the solid body itself
    /// (a single radial layer of solid control volumes)
    ///
    /// it is laterally coupled to an ambient temperature boundary
    /// and/or a heat source; it has no coupled fluid array
    pub solid_array: HeatTransferEntity,

    /// axial length of the structure, in metres (m)
    pub strucutre_length: Length,

    /// cross-sectional area of the solid, in square metres (m^2)
    pub cross_sectional_area: Area,

}

impl SolidStructure {

    /// constructs a solid structure as a hollow cylinder.
    ///
    /// `initial_temperature` (K) sets every node's starting temperature,
    /// `solid_pressure` (Pa) the (nominal) solid pressure, `cross_sectional_area`
    /// (m^2) the stored flow/solid area, `shell_id`/`shell_od` (m) the inner and
    /// outer diameters of the cylindrical shell, and `cylinder_length` (m) its
    /// axial length. `user_specified_inner_nodes` sets the number of interior
    /// axial nodes (total nodes = inner nodes + 2).
    pub fn new_hollow_cylinder(initial_temperature: ThermodynamicTemperature,
        solid_pressure: Pressure,
        cross_sectional_area: Area,
        shell_id: Length,
        shell_od: Length,
        cylinder_length: Length,
        pipe_shell_material: SolidMaterial,
        user_specified_inner_nodes: usize,) -> SolidStructure {

        // now the outer pipe array
        let pipe_shell = 
        SolidColumn::new_cylindrical_shell(
            cylinder_length,
            shell_id,
            shell_od,
            initial_temperature,
            solid_pressure,
            pipe_shell_material,
            user_specified_inner_nodes 
        );


        return Self { inner_nodes: user_specified_inner_nodes,
            solid_array: CVType::SolidArrayCV(pipe_shell).into(),
            strucutre_length: cylinder_length,
            cross_sectional_area,
        };
    }
}


/// stuff such as conductances are calculated here
pub mod preprocessing;



/// stuff for calculation is done here, ie, advancing timestep
pub mod calculation;

/// postprocessing stuff, ie, get the temperature vectors 
/// of both arrays of control volumes 
pub mod postprocessing;
