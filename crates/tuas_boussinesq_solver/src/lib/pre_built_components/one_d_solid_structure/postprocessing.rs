use uom::si::f64::*;
use crate::tuas_lib_error::TuasLibError;

use super::SolidStructure;

impl SolidStructure {
    /// returns the nodal temperature vector (in kelvin, K) of the solid
    /// structure's control-volume array, ordered from the back node to the
    /// front node.
    pub fn array_temperature(&mut self) -> Result<Vec<ThermodynamicTemperature>, TuasLibError> {
        self.solid_array.get_temperature_vector()
    }
}
