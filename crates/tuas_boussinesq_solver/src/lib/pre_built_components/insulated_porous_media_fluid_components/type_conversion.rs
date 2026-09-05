//! Type conversions for `InsulatedPorousMediaFluidComponent`.
//!
//! Converts the component into a `FluidComponent` (via its inner fluid array)
//! so it can be stored in fluid-component collections and hydraulic networks.
use crate::array_fluid_collections::fluid_array_lateral_coupling::FluidArray;
use crate::array_fluid_collections::fluid_component_collection::fluid_component::FluidComponent;

use super::InsulatedPorousMediaFluidComponent;
impl Into<FluidComponent> for InsulatedPorousMediaFluidComponent {
    fn into(self) -> FluidComponent {
        // get the fluid component
        let fluid_array_heat_transfer_entity = self.pipe_fluid_array;
        let fluid_array: FluidArray = fluid_array_heat_transfer_entity.try_into().unwrap();

        FluidComponent::FluidArray(fluid_array)
    }
}
