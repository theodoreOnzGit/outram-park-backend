//! CIET heater top and bottom head (bare, no insulation) components.
//!
//! This module models the two unheated end sections of the CIET heater
//! (version 2) — the top head and the bottom head — as the
//! [`HeaterTopBottomHead`] control-volume component. Each head is a lumped
//! set of three coupled thermal masses (Therminol VP-1 fluid, SS304L steel
//! shell, and twisted tape) exchanging heat with the ambient air.
//!
//! Units throughout follow `uom` SI conventions: temperatures in kelvin (K),
//! heat-transfer coefficients in W/(m^2 K), surface areas in m^2, lengths in
//! metres (m), and mass flow in kg/s. Submodules split the component into
//! preprocessing (construction / connections), calculation (timestep
//! advance), and postprocessing (temperature readout).

use crate::array_control_vol_and_fluid_component_collections::one_d_fluid_array_with_lateral_coupling::FluidArray;
use crate::array_control_vol_and_fluid_component_collections::one_d_solid_array_with_lateral_coupling::SolidColumn;
use crate::boussinesq_thermophysical_properties::SolidMaterial;
use crate::boussinesq_thermophysical_properties::LiquidMaterial;

use super::heat_transfer_entities::HeatTransferEntity;
use uom::si::f64::*;
use uom::si::area::square_meter;
use uom::si::heat_transfer::watt_per_square_meter_kelvin;
use uom::si::length::inch;
use uom::si::length::meter;
use uom::si::ratio::ratio;
use uom::si::pressure::atmosphere;
/// Models one of the CIET heater (version 2, bare/no insulation)
/// unheated end sections — either the top head or the bottom head.
///
/// The heater insulation was burnt off around 2018, after which many
/// frequency-response tests were run with the insulation removed; this
/// component therefore assumes no insulation but retains the twisted-tape
/// interior of heater version 2.
///
/// Unlike the heated-section component, this struct represents the top OR
/// bottom head (the unheated end regions of the heater), constructed via
/// [`HeaterTopBottomHead::new_top_head`] or
/// [`HeaterTopBottomHead::new_bottom_head`]. Each head is modelled as three
/// coupled thermal masses — the therminol (Therminol VP-1) fluid, the steel
/// (SS304L) shell, and the twisted tape — together with the ambient air state
/// used for heat loss.
#[derive(Debug, Clone, PartialEq)]
pub struct HeaterTopBottomHead {
    inner_nodes: usize,

    /// heat transfer entity representing control volumes
    /// for the twisted tape in the heater top and bottom heads
    pub twisted_tape_interior: HeatTransferEntity,

    /// heat transfer entity representing control volumes
    /// for the steel piping in the heater top and bottom heads
    pub steel_shell: HeatTransferEntity,

    /// heat transfer entity representing control volumes
    /// for the therminol fluid in the heater top and bottom heads
    pub therminol_array: HeatTransferEntity,

    /// ambient temperature of the surrounding air used to calculate heat loss
    /// (a thermodynamic temperature, SI unit kelvin)
    pub ambient_temperature: ThermodynamicTemperature,

    /// heat transfer coefficient used to calculate heat loss to air
    /// (SI unit watt per square metre per kelvin, W/(m^2 K))
    pub heat_transfer_to_air: HeatTransfer,
}

impl HeaterTopBottomHead {
    /// synthesiszes a new heater top head
    ///
    /// K = 3.75 (this is in addition to pipe form losses)
    /// l = 0.0889m
    /// d_h = 6.60e-3 m (hydraulic diameter)
    ///
    /// no insulation is assumed for this because
    /// currently the ciet model has no insulation
    ///
    /// now, we can attach heat structures to either one of the control volumes
    ///
    /// However, it should be attached to a heat structure
    /// (perhaps a SteelSS304L single CV with appropriate conductance
    /// 0.25 in^2 area, and callibrate length)
    ///
    /// It's only a rough guess
    ///
    /// We can callibrate until the outlet temperature at MX-10 is 102.45C
    /// That was the steady state temperature not considering other
    /// parasitic losses
    ///
    pub fn new_top_head(
        initial_temperature: ThermodynamicTemperature,
        ambient_temperature: ThermodynamicTemperature,
    ) -> Self {
        let user_specified_inner_nodes: usize = 0;
        let flow_area = Area::new::<square_meter>(0.00105);
        let head_length = Length::new::<meter>(0.0889);
        let atmospheric_pressure = Pressure::new::<atmosphere>(1.0);
        let pipe_form_loss = Ratio::new::<ratio>(3.75);
        let hydraulic_diameter = Length::new::<meter>(0.01467);

        // heater is inclined 90 degrees upwards, not that this is
        // particularly important for this scenario

        let pipe_incline_angle = Angle::new::<uom::si::angle::degree>(90.0);

        // theoretically it's 6 W/(m^2 K) but then we'll have to manually
        // input wall structures for additional heat loss
        //
        let h_to_air: HeatTransfer = HeatTransfer::new::<watt_per_square_meter_kelvin>(6.0);
        let id = Length::new::<meter>(0.0381);
        let od = Length::new::<meter>(0.04);

        // inner therminol array
        let therminol_array: FluidArray = FluidArray::new_odd_shaped_pipe(
            head_length,
            hydraulic_diameter,
            flow_area,
            initial_temperature,
            atmospheric_pressure,
            SolidMaterial::SteelSS304L,
            LiquidMaterial::TherminolVP1,
            pipe_form_loss,
            user_specified_inner_nodes,
            pipe_incline_angle,
        );
        // now the outer steel array
        let steel_shell_array = SolidColumn::new_cylindrical_shell(
            head_length,
            id,
            od,
            initial_temperature,
            atmospheric_pressure,
            SolidMaterial::SteelSS304L,
            user_specified_inner_nodes,
        );
        // now twisted_tape

        // the twisted tape width is assumed to be the twisted
        // tape diameter in De Wet's dissertation
        let twisted_tape_width: Length = Length::new::<inch>(1.0);
        let twisted_tape_thickness = Length::new::<inch>(0.048);
        let twisted_tape_height = head_length;

        let twisted_tape = SolidColumn::new_block(
            twisted_tape_height,
            twisted_tape_thickness,
            twisted_tape_width,
            initial_temperature,
            atmospheric_pressure,
            SolidMaterial::SteelSS304L,
            user_specified_inner_nodes,
        );

        return Self {
            inner_nodes: user_specified_inner_nodes,
            twisted_tape_interior: twisted_tape.into(),
            steel_shell: steel_shell_array.into(),
            therminol_array: therminol_array.into(),
            ambient_temperature,
            heat_transfer_to_air: h_to_air,
        };
    }
    /// synthesiszes a new user callibrated heater top head
    ///
    /// K = 3.75 (this is in addition to pipe form losses)
    /// l = 0.0889m
    /// d_h = 6.60e-3 m (hydraulic diameter)
    ///
    /// no insulation is assumed for this because
    /// currently the ciet model has no insulation
    ///
    /// now, we can attach heat structures to either one of the control volumes
    ///
    /// However, it should be attached to a heat structure
    /// (perhaps a SteelSS304L single CV with appropriate conductance
    /// 0.25 in^2 area, and callibrate length)
    ///
    /// It's only a rough guess
    ///
    /// We can callibrate until the outlet temperature at MX-10 is 102.45C
    /// That was the steady state temperature not considering other
    /// parasitic losses
    ///
    pub fn _new_user_callibrated_top_head(
        initial_temperature: ThermodynamicTemperature,
        ambient_temperature: ThermodynamicTemperature,
        h_to_air: HeatTransfer,
    ) -> Self {
        let user_specified_inner_nodes: usize = 0;
        let flow_area = Area::new::<square_meter>(0.00105);
        let head_length = Length::new::<meter>(0.0889);
        let atmospheric_pressure = Pressure::new::<atmosphere>(1.0);
        let pipe_form_loss = Ratio::new::<ratio>(3.75);
        let hydraulic_diameter = Length::new::<meter>(0.01467);

        // heater is inclined 90 degrees upwards, not that this is
        // particularly important for this scenario

        let pipe_incline_angle = Angle::new::<uom::si::angle::degree>(90.0);

        // theoretically it's 6 W/(m^2 K) but then we'll have to manually
        // input wall structures for additional heat loss
        //
        let id = Length::new::<meter>(0.0381);
        let od = Length::new::<meter>(0.04);

        // inner therminol array
        let therminol_array: FluidArray = FluidArray::new_odd_shaped_pipe(
            head_length,
            hydraulic_diameter,
            flow_area,
            initial_temperature,
            atmospheric_pressure,
            SolidMaterial::SteelSS304L,
            LiquidMaterial::TherminolVP1,
            pipe_form_loss,
            user_specified_inner_nodes,
            pipe_incline_angle,
        );
        // now the outer steel array
        let steel_shell_array = SolidColumn::new_cylindrical_shell(
            head_length,
            id,
            od,
            initial_temperature,
            atmospheric_pressure,
            SolidMaterial::SteelSS304L,
            user_specified_inner_nodes,
        );
        // now twisted_tape

        // the twisted tape width is assumed to be the twisted
        // tape diameter in De Wet's dissertation
        let twisted_tape_width: Length = Length::new::<inch>(1.0);
        let twisted_tape_thickness = Length::new::<inch>(0.048);
        let twisted_tape_height = head_length;

        let twisted_tape = SolidColumn::new_block(
            twisted_tape_height,
            twisted_tape_thickness,
            twisted_tape_width,
            initial_temperature,
            atmospheric_pressure,
            SolidMaterial::SteelSS304L,
            user_specified_inner_nodes,
        );

        return Self {
            inner_nodes: user_specified_inner_nodes,
            twisted_tape_interior: twisted_tape.into(),
            steel_shell: steel_shell_array.into(),
            therminol_array: therminol_array.into(),
            ambient_temperature,
            heat_transfer_to_air: h_to_air,
        };
    }

    /// synthesiszes a new heater bottom head
    ///
    /// K = 3.95 (this is in addition to pipe form losses)
    /// l = 0.19685m
    /// d_h = 6.60e-3 m (hydraulic diameter)
    ///
    /// no insulation is assumed for this because
    /// currently the ciet model has no insulation
    ///
    /// now, we can attach heat structures to either one of the control volumes
    ///
    /// However, it should be attached to a heat structure
    /// (perhaps a SteelSS304L single CV with appropriate conductance
    /// 0.25 in^2 area, and callibrate length)
    ///
    /// It's only a rough guess
    ///
    pub fn new_bottom_head(
        initial_temperature: ThermodynamicTemperature,
        ambient_temperature: ThermodynamicTemperature,
    ) -> Self {
        let user_specified_inner_nodes: usize = 0;
        let flow_area = Area::new::<square_meter>(0.00105);
        let head_length = Length::new::<meter>(0.19685);
        let atmospheric_pressure = Pressure::new::<atmosphere>(1.0);
        let pipe_form_loss = Ratio::new::<ratio>(3.95);
        let hydraulic_diameter = Length::new::<meter>(0.01467);

        // heater is inclined 90 degrees upwards, not that this is
        // particularly important for this scenario

        let pipe_incline_angle = Angle::new::<uom::si::angle::degree>(90.0);

        // theoretically it's 6 W/(m^2 K) but then we'll have to manually
        // input wall structures for additional heat loss
        //
        let h_to_air: HeatTransfer = HeatTransfer::new::<watt_per_square_meter_kelvin>(6.0);
        let id = Length::new::<meter>(0.0381);
        let od = Length::new::<meter>(0.04);

        // inner therminol array
        let therminol_array: FluidArray = FluidArray::new_odd_shaped_pipe(
            head_length,
            hydraulic_diameter,
            flow_area,
            initial_temperature,
            atmospheric_pressure,
            SolidMaterial::SteelSS304L,
            LiquidMaterial::TherminolVP1,
            pipe_form_loss,
            user_specified_inner_nodes,
            pipe_incline_angle,
        );
        // now the outer steel array
        let steel_shell_array = SolidColumn::new_cylindrical_shell(
            head_length,
            id,
            od,
            initial_temperature,
            atmospheric_pressure,
            SolidMaterial::SteelSS304L,
            user_specified_inner_nodes,
        );
        // now twisted_tape (TBC)
        let twisted_tape_width: Length = Length::new::<inch>(1.0);
        let twisted_tape_thickness = Length::new::<inch>(0.048);
        let twisted_tape_height = head_length;
        let twisted_tape = SolidColumn::new_block(
            twisted_tape_height,
            twisted_tape_thickness,
            twisted_tape_width,
            initial_temperature,
            atmospheric_pressure,
            SolidMaterial::SteelSS304L,
            user_specified_inner_nodes,
        );

        return Self {
            inner_nodes: user_specified_inner_nodes,
            twisted_tape_interior: twisted_tape.into(),
            steel_shell: steel_shell_array.into(),
            therminol_array: therminol_array.into(),
            ambient_temperature,
            heat_transfer_to_air: h_to_air,
        };
    }
    /// synthesiszes a new heater bottom head allowing the
    /// user to callibrate the heat transfer coefficient to air
    ///
    /// K = 3.95 (this is in addition to pipe form losses)
    /// l = 0.19685m
    /// d_h = 6.60e-3 m (hydraulic diameter)
    ///
    /// no insulation is assumed for this because
    /// currently the ciet model has no insulation
    ///
    /// now, we can attach heat structures to either one of the control volumes
    ///
    /// However, it should be attached to a heat structure
    /// (perhaps a SteelSS304L single CV with appropriate conductance
    /// 0.25 in^2 area, and callibrate length)
    ///
    /// It's only a rough guess
    ///
    pub fn _new_user_callibrated_bottom_head(
        initial_temperature: ThermodynamicTemperature,
        ambient_temperature: ThermodynamicTemperature,
        h_to_air: HeatTransfer,
    ) -> Self {
        let user_specified_inner_nodes: usize = 0;
        let flow_area = Area::new::<square_meter>(0.00105);
        let head_length = Length::new::<meter>(0.19685);
        let atmospheric_pressure = Pressure::new::<atmosphere>(1.0);
        let pipe_form_loss = Ratio::new::<ratio>(3.95);
        let hydraulic_diameter = Length::new::<meter>(0.01467);

        // heater is inclined 90 degrees upwards, not that this is
        // particularly important for this scenario

        let pipe_incline_angle = Angle::new::<uom::si::angle::degree>(90.0);

        // theoretically it's 6 W/(m^2 K) but then we'll have to manually
        // input wall structures for additional heat loss
        //
        let id = Length::new::<meter>(0.0381);
        let od = Length::new::<meter>(0.04);

        // inner therminol array
        let therminol_array: FluidArray = FluidArray::new_odd_shaped_pipe(
            head_length,
            hydraulic_diameter,
            flow_area,
            initial_temperature,
            atmospheric_pressure,
            SolidMaterial::SteelSS304L,
            LiquidMaterial::TherminolVP1,
            pipe_form_loss,
            user_specified_inner_nodes,
            pipe_incline_angle,
        );
        // now the outer steel array
        let steel_shell_array = SolidColumn::new_cylindrical_shell(
            head_length,
            id,
            od,
            initial_temperature,
            atmospheric_pressure,
            SolidMaterial::SteelSS304L,
            user_specified_inner_nodes,
        );
        // now twisted_tape (TBC)
        let twisted_tape_width: Length = Length::new::<inch>(1.0);
        let twisted_tape_thickness = Length::new::<inch>(0.048);
        let twisted_tape_height = head_length;
        let twisted_tape = SolidColumn::new_block(
            twisted_tape_height,
            twisted_tape_thickness,
            twisted_tape_width,
            initial_temperature,
            atmospheric_pressure,
            SolidMaterial::SteelSS304L,
            user_specified_inner_nodes,
        );

        return Self {
            inner_nodes: user_specified_inner_nodes,
            twisted_tape_interior: twisted_tape.into(),
            steel_shell: steel_shell_array.into(),
            therminol_array: therminol_array.into(),
            ambient_temperature,
            heat_transfer_to_air: h_to_air,
        };
    }
}

/// contains method implementations for obtaining conductances
/// between the different arrays, and also laterally coupling
/// the arrays to one another using a radial thermal resistance
pub mod preprocessing;

/// contains method implementations for FluidComponentTrait
/// This means all the stuff about getting mass flowrate from pressure
/// and vice versa
pub mod fluid_entity;

/// contains methods to help advance timesteps (ie update the
/// state of the control volumes after each timestep)
pub mod calculation;

/// for postprocessing, one can obtain temperature profiles
/// of the component using the postprocessing modules
pub mod postprocessing;
