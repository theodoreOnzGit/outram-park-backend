//! Postprocessing for the non-insulated porous-media fluid component:
//! extracts the nodal temperature vectors (in kelvin) of each of its three
//! control-volume arrays — the outer pipe shell, the fluid, and the interior
//! porous-media solid (e.g. the twisted tape in CIET's heated section).

use uom::si::f64::*;

use super::NonInsulatedPorousMediaFluidComponent;

impl NonInsulatedPorousMediaFluidComponent {
    /// provides the nodal temperature vector (in kelvin) of the steel pipe
    /// shell along the heated section
    pub fn pipe_shell_temperature(&mut self) -> Vec<ThermodynamicTemperature>{
        self.pipe_shell.get_temperature_vector().unwrap()
    }

    /// provides the nodal temperature vector (in kelvin) of the fluid
    /// (e.g. Therminol VP1) along the heated section
    pub fn pipe_fluid_array_temperature(&mut self) -> Vec<ThermodynamicTemperature>{
        self.pipe_fluid_array.get_temperature_vector().unwrap()
    }

    /// provides the nodal temperature vector (in kelvin) of the interior
    /// porous-media solid (e.g. the twisted tape) along the heated section
    pub fn interior_solid_array_temperature(&mut self) -> Vec<ThermodynamicTemperature>{
        self.interior_solid_array_for_porous_media.get_temperature_vector().unwrap()
    }
}
