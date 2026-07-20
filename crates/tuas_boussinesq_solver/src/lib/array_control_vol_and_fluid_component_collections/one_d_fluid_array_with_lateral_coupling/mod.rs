//! A 1D fluid control-volume array (`FluidArray`) with lateral (radial)
//! conduction coupling and axial advection.
//!
//! This module owns the `FluidArray` type: a chain of fluid nodes discretising
//! a pipe or channel along its flow axis. The array carries axial advection
//! (mass flow in kg/s) and axial conduction between neighbouring nodes, and can
//! be coupled laterally (radially) to adjacent solid or fluid arrays through
//! shared thermal conductances (W/K) with no radial advection. It also acts as
//! a `FluidComponent`, providing pressure-loss / mass-flowrate relations via a
//! Darcy friction-factor correlation.
//!
//! The node temperatures (K) are advanced with an implicit (backward) Euler
//! scheme; see the `calculation` submodule for the energy balance. Submodules
//! group the behaviour: `constructors`/`default` build arrays, `preprocessing`
//! computes timestep/Reynolds/Nusselt quantities, `postprocessing` reads out
//! temperature profiles, `lateral_connection` wires radial conductances and
//! power sources, `axial_connection` links neighbouring CVs/BCs at the front and
//! back, `fluid_component_calculation` holds the friction-loss correlations, and
//! `type_conversion` converts to/from `FluidComponent`.

use crate::heat_transfer_correlations::nusselt_number_correlations::enums::NusseltCorrelation;
use crate::single_control_vol::SingleCVNode;
use crate::boussinesq_thermophysical_properties::Material;
use crate::boussinesq_thermophysical_properties::specific_enthalpy::try_get_h;
use uom::si::f64::*;
use ndarray::*;

use crate::tuas_lib_error::TuasLibError;

use self::fluid_component_calculation::DimensionlessDarcyLossCorrelations;


/// A 1D array of fluid control volumes discretising a pipe or channel along
/// its flow (axial) direction.
///
/// It contains a back single CV (lowest axial coordinate) and a front single
/// CV (highest axial coordinate), plus `inner_nodes` interior nodes between
/// them, and carries axial advection (mass flow in kg/s), axial conduction
/// between neighbouring nodes, and lateral (radial) conduction coupling to
/// adjacent solid or fluid arrays (thermal conductances in W/K, no radial
/// advection).
///
/// It can represent a cylindrical pipe, an annular channel, or an arbitrary
/// odd-shaped flow passage (see the constructors). Usually these are nested
/// inside a larger heat-transfer component and then used.
///
/// Node temperatures (K) are advanced with the implicit (backward) Euler
/// scheme.
///
/// You must supply the number of nodes for the fluid array. Note that the
/// front and back CVs each count as one node, so the total node count is
/// `inner_nodes + 2`.
#[derive(Debug,Clone,PartialEq)]
pub struct FluidArray {

    /// represents the control volume at the back 
    /// imagine a car or train cruising along in a positive x direction
    ///
    /// //----------------------------------------------> x 
    ///
    /// //            (back --- train/car --- front)
    /// //            lower x                 higher x
    ///
    pub back_single_cv: SingleCVNode,

    /// represents the control volume at the front
    ///
    /// to think of which is front and back, we think of coordinates 
    /// imagine a car or train cruising along in a positive x direction
    ///
    /// //----------------------------------------------> x 
    ///
    /// //            (back --- train/car --- front)
    /// //            lower x                 higher x
    ///
    pub front_single_cv: SingleCVNode,

    /// number of inner nodes ,
    /// besides the back and front node 
    ///
    /// total number of nodes is inner_nodes + 2
    inner_nodes: usize,


    // total length for the array
    total_length: Length,

    // cross sectional area for the 1D array, assumed to be uniform 
    pub (crate) xs_area: Area,

    /// temperature array current timestep 
    /// only accessible via get and set methods
    pub (crate) temperature_array_current_timestep: Array1<ThermodynamicTemperature>,

    /// control volume material 
    pub material_control_volume: Material,

    /// control volume pressure 
    pub pressure_control_volume: Pressure,

    // volume fraction array 
    pub (crate) volume_fraction_array: Array1<f64>,

    /// mass flowrate through the fluid array, 
    mass_flowrate: MassRate,

    /// pressure loss term 
    pressure_loss: Pressure,

    /// wetted perimeter (for hydraulic diameter) 
    wetted_perimeter: Length,

    /// incline angle 
    incline_angle: Angle,

    /// internal pressure source 
    internal_pressure_source: Pressure,

    /// fluid component loss properties 
    /// be it for pipe or something else
    pub fluid_component_loss_properties: DimensionlessDarcyLossCorrelations,

    /// nusselt correlation 
    pub nusselt_correlation: NusseltCorrelation,

    /// now fluid arrays can be connected to solid arrays 
    /// or other fluid arrays adjacent to it radially
    ///
    /// There will be no advection in the radial direction,
    /// but there can be thermal conductance shared between the nodes 
    ///
    /// hence, I only want to have a copy of the temperature 
    /// arrays radially adjacent to it
    ///
    /// plus their thermal resistances
    /// N is the array size, which is known at compile time

    pub lateral_adjacent_array_temperature_vector: 
    Vec<Array1<ThermodynamicTemperature>>,

    /// now fluid arrays can be connected to solid arrays 
    /// or other fluid arrays adjacent to it radially
    ///
    /// There will be no advection in the radial direction,
    /// but there can be thermal conductance shared between the nodes 
    ///
    /// hence, I only want to have a copy of the temperature 
    /// arrays radially adjacent to it
    ///
    /// plus their thermal resistances 
    /// N is the array size, which is known at compile time
    pub lateral_adjacent_array_conductance_vector:
    Vec<Array1<ThermalConductance>>,

    /// fluid arrays can also be connected to heat sources 
    /// or have specified volumetric heat sources 
    pub q_vector: Vec<Power>,

    /// fluid arrays should have their power distributed according 
    /// to their nodes 
    pub q_fraction_vector: Vec<Array1<f64>>,

}

impl FluidArray {



    /// obtains a clone of the temperature array in Array1 ndarray 
    /// form 
    pub fn get_temperature_array(&self) -> Result< 
    Array1<ThermodynamicTemperature>, TuasLibError> {

        // converts the fixed sized temperature array (at compile time) 
        // into a dynamically sized ndarray type so we can use solve
        // methods
        let mut temperature_arr: Array1<ThermodynamicTemperature> = 
        Array1::default(self.len());

        for (idx,temperature) in 
            self.temperature_array_current_timestep.iter().enumerate() {
                temperature_arr[idx] = *temperature;
        }

        Ok(temperature_arr)


    }


    /// sets the node temperature array (K) from a temperature vector.
    ///
    /// The vector length must equal the number of nodes (`inner_nodes + 2`),
    /// otherwise a `ShapeMismatch` error is returned. The back and front single
    /// CV specific enthalpies are re-synchronised from the first and last
    /// temperatures.
    pub fn set_temperature_vector(&mut self,
    temperature_vec: Vec<ThermodynamicTemperature>) -> Result<(), TuasLibError>{

        let number_of_temperature_nodes = self.len();

        // check if temperature_vec has the correct number_of_temperature_nodes

        if temperature_vec.len() !=  number_of_temperature_nodes {
            let shape_error = ShapeError::from_kind(
                ErrorKind::IncompatibleShape
            );

            return Err(TuasLibError::ShapeMismatch(shape_error.to_string()));

        }

        for (index,temperature) in 
            self.temperature_array_current_timestep.iter_mut().enumerate() {
            *temperature = temperature_vec[index];
        }

        // we also need to ensure that the front and end nodes are 
        // properly synchronised in terms of temperature
        //

        let back_cv_temperature: ThermodynamicTemperature 
        = temperature_vec[0];

        let front_cv_temperature: ThermodynamicTemperature 
        = *temperature_vec.last().unwrap();


        // update enthalpies of control volumes withing

        let material = self.material_control_volume;
        let pressure = self.pressure_control_volume;

        let back_cv_enthalpy = try_get_h(
            material,
            back_cv_temperature,
            pressure
        )?;

        let front_cv_enthalpy = try_get_h(
            material,
            front_cv_temperature,
            pressure
        )?;

        self.back_single_cv.current_timestep_control_volume_specific_enthalpy
            = back_cv_enthalpy;

        self.front_single_cv.current_timestep_control_volume_specific_enthalpy
            = front_cv_enthalpy;


        
        Ok(())
    }

    /// sets the node temperature array (K) from an `Array1` ndarray.
    ///
    /// Delegates to `set_temperature_vector`, so the same length check
    /// (`inner_nodes + 2`) and front/back enthalpy re-synchronisation apply.
    pub fn set_temperature_array(&mut self,
    temperature_arr: Array1<ThermodynamicTemperature>) -> Result<(),
    TuasLibError> {

        // we'll convert the temperature array into vector form 
        // and use the existing method 
        let mut temperature_vec: Vec<ThermodynamicTemperature> 
        = vec![];

        for temperature in temperature_arr.iter() {
            temperature_vec.push(*temperature)
        }

        self.set_temperature_vector(temperature_vec)


    }

    /// length of the fluid array 
    pub fn len(&self) -> usize {
        self.inner_nodes + 2
    }
}
/// Functions or methods to retrieve temperature and other such 
/// data from the array_cv
pub mod postprocessing;


/// Functions or methods to get timestep and other such quantiies 
/// for calculations 
///
/// helps to set up quantities used in calculation step
pub mod preprocessing;
 

/// Contains functions which advance the timestep
/// it's the bulk of calculation
pub mod calculation;


/// contains code to connect control volumes laterally,
/// in a cylindrical situation, it means radially 
pub mod lateral_connection;


/// contains code to connect to other array cvs, other boundary conditions 
/// or other single cvs
pub mod axial_connection;

/// defaults 
pub mod default;


/// constructors  
pub mod constructors;

/// fluid component calculations 
/// with the DimensionlessDarcyLossCorrelations
pub mod fluid_component_calculation;

/// type conversion 
pub mod type_conversion;


/// unit tests, especially for connection with single control volumes 
/// among other verification tests
#[cfg(test)]
pub mod tests;
