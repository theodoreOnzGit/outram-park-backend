//! Non-insulated fluid components — bare pipes and fluid components that
//! exchange heat directly with an ambient boundary.
//!
//! This module provides [`NonInsulatedFluidComponent`], a fluid component with
//! no insulation layer: a fluid array (the flowing coolant) coupled to a solid
//! pipe shell, which in turn loses (or gains) heat to an ambient-temperature
//! boundary through a user-supplied heat-transfer coefficient. Because there is
//! no insulation, these are the right choice when heat exchange with the
//! surroundings is intended — e.g. coolers, heaters, or piping whose heat loss
//! is being tracked.
//!
//! The fluid-to-shell coupling uses a Nusselt-number correlation (Gnielinski by
//! default; the CIET heater correlation for the heater builder). Pressure drop
//! is set by a Darcy friction / form-loss correlation.
//!
//! Units: temperatures in kelvin (K) or degrees Celsius (degC), mass flow rate
//! in kilograms per second (kg/s), heat input / power in watts (W), pressure
//! and pressure drop in pascals (Pa), thermal conductance in watts per kelvin
//! (W/K), heat-transfer coefficient in watts per square metre kelvin
//! (W/(m^2 K)), and lengths / diameters in metres (m). All public signatures
//! carry `uom` dimensioned quantities.
//!
//! Submodules:
//! - [`preprocessing`] — build the ambient and fluid-to-shell conductances and
//!   wire the lateral / axial connections; Reynolds-number helper.
//! - [`fluid_component`] — `FluidComponentTrait` impl (mass flow ↔ pressure loss).
//! - [`calculation`] — advance the component one timestep.
//! - [`postprocessing`] — read back the shell and fluid nodal temperature vectors.
//! - [`type_conversion`] — convert into a `FluidComponent`.
//! - [`calibration`] — override the fluid Nusselt correlation.
use crate::array_control_vol_and_fluid_component_collections::one_d_fluid_array_with_lateral_coupling::fluid_component_calculation::DimensionlessDarcyLossCorrelations;
use crate::array_control_vol_and_fluid_component_collections::one_d_fluid_array_with_lateral_coupling::FluidArray;
use crate::array_control_vol_and_fluid_component_collections::one_d_solid_array_with_lateral_coupling::SolidColumn;
use crate::boussinesq_thermophysical_properties::SolidMaterial;
use crate::boussinesq_thermophysical_properties::LiquidMaterial;
use crate::heat_transfer_correlations::nusselt_number_correlations::enums::NusseltCorrelation;
use crate::heat_transfer_correlations::nusselt_number_correlations::input_structs::NusseltPrandtlReynoldsData;

use super::heat_transfer_entities::cv_types::CVType;
use super::heat_transfer_entities::HeatTransferEntity;
use uom::si::area::square_meter;
use uom::si::f64::*;
use uom::si::heat_transfer::watt_per_square_meter_kelvin;
use uom::si::length::meter;
use uom::si::pressure::atmosphere;
use uom::si::ratio::ratio;
use uom::ConstZero;

/// The simplest component is a non insulated pipe
///
/// This is a simple pipe with a set hydraulic diameter and length
///
/// the standard assumption is that at each boundary of this pipe,
/// there is no conduction heat transfer in the axial direction
/// TODO: the nusselt number correlations for the shell and tube side 
/// are not yet capable/tested of handling nusselt number correlations other 
/// than Gnielinski type correlations
///
#[derive(Clone,Debug,PartialEq)]
pub struct NonInsulatedFluidComponent {

    inner_nodes: usize,

    /// this HeatTransferEntity represents the pipe shell which is 
    /// exposed to an ambient constant temperature boundary condition
    /// This is because constant heat flux BCs are not common for pipes
    ///
    /// only one radial layer of control volumes is used to simulate 
    /// the pipe shell
    pub pipe_shell: HeatTransferEntity,


    /// this HeatTransferEntity represents the pipe fluid
    /// which is coupled to the pipe shell via a Nusselt Number based
    /// thermal resistance (usually Gnielinski correlation)
    pub pipe_fluid_array: HeatTransferEntity,

    /// pipe ambient temperature
    pub ambient_temperature: ThermodynamicTemperature,

    /// pipe heat transfer coefficient to ambient
    pub heat_transfer_to_ambient: HeatTransfer,

    /// pipe  outer diameter 
    pub od: Length,

    /// pipe inner diameter 
    pub id: Length,

    /// flow area 
    pub flow_area: Area,

    /// loss correlation 
    pub custom_component_loss_correlation: DimensionlessDarcyLossCorrelations

}

impl NonInsulatedFluidComponent {

    /// constructs a new pipe
    ///
    /// you need to supply the initial temperature, ambient temperature
    /// as well as all the pipe parameters 
    ///
    /// such as:
    ///
    /// 1. flow area 
    /// 2. hydraulic diameter 
    /// 3. incline angle
    /// 4. any form losses beyond the Gnielinski correlation
    /// 5. inner diameter (id)
    /// 6. outer diameter (od)
    /// 7. pipe shell material 
    /// 8. pipe fluid 
    /// 9. fluid pressure (if in doubt, 1 atmosphere will do)
    /// 10. solid pressure (if in doubt, 1 atmosphere will do)
    /// 11. heat transfer coeffficient to ambient
    /// 12. how many inner axial nodes for both solid and fluid arrays
    ///
    /// The number of total axial nodes is the number of inner nodes plus 2
    ///
    /// this is because there are two nodes at the periphery of the pipe 
    /// and there
    ///
    /// at each timestep, you are allowed to set a heater power, where 
    /// heat is dumped into the heated tube surrounding the pipe
    ///
    /// so the pipe shell becomes the heating element so to speak
    pub fn new_bare_pipe(initial_temperature: ThermodynamicTemperature,
        ambient_temperature: ThermodynamicTemperature,
        fluid_pressure: Pressure,
        solid_pressure: Pressure,
        flow_area: Area,
        incline_angle: Angle,
        form_loss: Ratio,
        id: Length,
        od: Length,
        pipe_length: Length,
        hydraulic_diameter: Length,
        surface_roughness: Length,
        pipe_shell_material: SolidMaterial,
        pipe_fluid: LiquidMaterial,
        htc_to_ambient: HeatTransfer,
        user_specified_inner_nodes: usize) -> NonInsulatedFluidComponent {

        // inner fluid_array
        let mut fluid_array: FluidArray = 
        FluidArray::new_odd_shaped_pipe(
            pipe_length,
            hydraulic_diameter,
            flow_area,
            initial_temperature,
            fluid_pressure,
            pipe_shell_material,
            pipe_fluid,
            form_loss,
            user_specified_inner_nodes,
            incline_angle
        );
        let custom_component_loss_correlation = DimensionlessDarcyLossCorrelations::
                new_pipe(pipe_length, 
                    surface_roughness, 
                    hydraulic_diameter, 
                    form_loss);

        fluid_array.fluid_component_loss_properties = custom_component_loss_correlation;

        // now the outer steel array
        let pipe_shell = 
        SolidColumn::new_cylindrical_shell(
            pipe_length,
            id,
            od,
            initial_temperature,
            solid_pressure,
            pipe_shell_material,
            user_specified_inner_nodes 
        );

        return Self { inner_nodes: user_specified_inner_nodes,
            pipe_shell: CVType::SolidArrayCV(pipe_shell).into(),
            pipe_fluid_array: CVType::FluidArrayCV(fluid_array).into(),
            ambient_temperature,
            heat_transfer_to_ambient: htc_to_ambient,
            od,
            id,
            flow_area,
            custom_component_loss_correlation,
        };
    }


    /// constructs a new heater v2 based on de wet's model,
    /// but without the inner twisted tape 
    pub fn new_dewet_model_heater_v2_no_twisted_tape(
        initial_temperature: ThermodynamicTemperature,
        ambient_temperature: ThermodynamicTemperature,
        user_specified_inner_nodes: usize) -> Self {

        let flow_area = Area::new::<square_meter>(0.00105);
        let heated_length = Length::new::<meter>(1.6383);
        let atmospheric_pressure = Pressure::new::<atmosphere>(1.0);
        let hydraulic_diameter = Length::new::<meter>(0.01467);

        // heater is inclined 90 degrees upwards, not that this is 
        // particularly important for this scenario

        let pipe_incline_angle = Angle::new::<uom::si::angle::degree>(90.0);

        // default is a 20 W/(m^2 K) callibrated heat transfer coeff 
        // theoretically it's 6 W/(m^2 K) but then we'll have to manually 
        // input wall structures for additional heat loss
        //
        let h_to_air: HeatTransfer = 
        HeatTransfer::new::<watt_per_square_meter_kelvin>(20.0);
        let steel_shell_id = Length::new::<meter>(0.0381);
        let steel_shell_od = Length::new::<meter>(0.04);


        // inner therminol array 
        //
        // the darcy loss correlation is f = 17.9 *Re^{-0.34}
        // accurate to within 4% (Lukas et al)
        // Improved Heat Transfer and Volume Scaling through 
        // Novel Heater Design
        // 

        let a = Ratio::ZERO;
        let b = Ratio::new::<ratio>(17.9);
        let c: f64  = -0.34;
        let mut therminol_array: FluidArray = 
        FluidArray::new_custom_component(
            heated_length,
            hydraulic_diameter,
            flow_area,
            initial_temperature,
            atmospheric_pressure,
            LiquidMaterial::TherminolVP1,
            a,
            b,
            c,
            user_specified_inner_nodes,
            pipe_incline_angle
        );

        // the therminol array nusselt correlation should be that of the 
        // heater 

        let heater_prandtl_reynolds_data: NusseltPrandtlReynoldsData 
        = NusseltPrandtlReynoldsData::default();
        therminol_array.nusselt_correlation = 
            NusseltCorrelation::CIETHeaterVersion2(
                heater_prandtl_reynolds_data
                );

        let darcy_loss_correlation = 
            therminol_array.fluid_component_loss_properties.clone();

        // now the outer steel array
        let steel_shell_array = 
        SolidColumn::new_cylindrical_shell(
            heated_length,
            steel_shell_id,
            steel_shell_od,
            initial_temperature,
            atmospheric_pressure,
            SolidMaterial::SteelSS304L,
            user_specified_inner_nodes 
        );





        return Self { inner_nodes: user_specified_inner_nodes, 
            pipe_shell: steel_shell_array.into(), 
            pipe_fluid_array: therminol_array.into(), 
            ambient_temperature, 
            heat_transfer_to_ambient: h_to_air, 
            od: steel_shell_od, 
            id: steel_shell_id, 
            flow_area, 
            custom_component_loss_correlation: darcy_loss_correlation 
        };

    }

    /// constructs a new non-insulated custom fluid component
    ///
    /// like [`Self::new_bare_pipe`], but with a user-defined Darcy friction
    /// correlation instead of the built-in pipe correlation.
    ///
    /// you need to supply the initial temperature, ambient temperature
    /// as well as all the component parameters
    ///
    /// The Darcy friction factor is calculated as:
    ///
    /// f_darcy = form_loss + b Re^(c)
    ///
    /// b is the reynolds_coefficient
    /// c is reynolds power
    pub fn new_custom_component(initial_temperature: ThermodynamicTemperature,
        ambient_temperature: ThermodynamicTemperature,
        fluid_pressure: Pressure,
        solid_pressure: Pressure,
        flow_area: Area,
        incline_angle: Angle,
        form_loss: Ratio,
        reynolds_coefficient: Ratio,
        reynolds_power: f64,
        shell_id: Length,
        shell_od: Length,
        component_length: Length,
        hydraulic_diameter: Length,
        pipe_shell_material: SolidMaterial,
        pipe_fluid: LiquidMaterial,
        htc_to_ambient: HeatTransfer,
        user_specified_inner_nodes: usize,) -> NonInsulatedFluidComponent {

        // inner fluid_array

        let a = form_loss;
        let b = reynolds_coefficient;
        let c = reynolds_power;

        let fluid_array: FluidArray = 
            FluidArray::new_custom_component(
                component_length, 
                hydraulic_diameter, 
                flow_area, 
                initial_temperature, 
                fluid_pressure, 
                pipe_fluid, 
                form_loss, 
                b, 
                c, 
                user_specified_inner_nodes, 
                incline_angle);

        // now the outer pipe array
        let pipe_shell = 
        SolidColumn::new_cylindrical_shell(
            component_length,
            shell_id,
            shell_od,
            initial_temperature,
            solid_pressure,
            pipe_shell_material,
            user_specified_inner_nodes 
        );


        // custom component loss correlation
        //


        let custom_component_loss_correlation = DimensionlessDarcyLossCorrelations::
            new_simple_reynolds_power_component(a, b, c);

        return Self { inner_nodes: user_specified_inner_nodes,
            pipe_shell: CVType::SolidArrayCV(pipe_shell).into(),
            pipe_fluid_array: CVType::FluidArrayCV(fluid_array).into(),
            ambient_temperature,
            heat_transfer_to_ambient: htc_to_ambient,
            od: shell_od,
            id: shell_id,
            flow_area,
            custom_component_loss_correlation,
        };
    }
}


/// stuff such as conductances are calculated here
pub mod preprocessing;

/// implementations for the FluidComponent trait
/// are done here
pub mod fluid_component;


/// stuff for calculation is done here, ie, advancing timestep
pub mod calculation;

/// postprocessing stuff, ie, get the temperature vectors 
/// of both arrays of control volumes 
pub mod postprocessing;

/// type conversion, such as into fluid component and such
pub mod type_conversion;


/// calibration, for calibrating thickness or nusselt correlation 
/// (incomplete)
pub mod calibration;

/// validation and verification tests 
#[cfg(test)]
pub mod tests;
