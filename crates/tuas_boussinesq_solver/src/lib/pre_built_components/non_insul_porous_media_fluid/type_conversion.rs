use crate::array_fluid_collections::fluid_array_lateral_coupling::FluidArray;
use crate::array_fluid_collections::fluid_component_collection::fluid_component::FluidComponent;

use super::NonInsulatedPorousMediaFluidComponent;

impl Into<FluidComponent> for NonInsulatedPorousMediaFluidComponent {
    fn into(self) -> FluidComponent {
        // get the fluid component
        let fluid_array_heat_transfer_entity = self.pipe_fluid_array;
        let fluid_array: FluidArray = fluid_array_heat_transfer_entity.try_into().unwrap();

        FluidComponent::FluidArray(fluid_array)
    }
}
