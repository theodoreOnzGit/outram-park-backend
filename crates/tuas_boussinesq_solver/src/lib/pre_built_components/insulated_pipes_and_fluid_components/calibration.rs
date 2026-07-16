use super::InsulatedFluidComponent;
use uom::si::f64::*;

use crate::prelude::beta_testing::FluidArray;
use crate::tuas_lib_error::TuasLibError;
use crate::heat_transfer_correlations::nusselt_number_correlations::enums::NusseltCorrelation;

impl InsulatedFluidComponent {

    /// calibrates the insulation thickness of this pipe or component, 
    /// to increase or decrease parasitic heat loss
    /// however, will not change thermal inertia
    /// 
    pub fn calibrate_insulation_thickness(&mut self, 
        insulation_thickness: Length){

        let id = self.insulation_id;
        let od = id + 2.0*insulation_thickness.abs();
        self.insulation_od = od;

    }

    /// gets the insulation thickness based on 
    /// (insulation_od - insulation_id)/2
    pub fn get_insulation_thickness(&self) -> Length {
        let id = self.insulation_id;
        let od = self.insulation_od;

        let insulation_thickness = 0.5 * (od - id);

        return insulation_thickness;

    }

    /// calibrates the heat transfer coefficient to ambient 
    /// to some value 
    pub fn calibrate_heat_transfer_to_ambient(&mut self,
        ambient_htc: HeatTransfer){
        self.heat_transfer_to_ambient = ambient_htc;
    }

    /// tries to calibrate the Gnielinski Nusselt number correlation of the
    /// pipe fluid array by a multiplicative `calibration_ratio`
    /// (dimensionless).
    ///
    /// If the fluid array's Nusselt correlation is a Gnielinski variant
    /// (generic or already calibrated), it is replaced with a calibrated
    /// Gnielinski correlation carrying the new ratio. For any non-Gnielinski
    /// correlation this is not implemented and currently panics (`todo!`).
    /// Returns `Err(TuasLibError)` on failure paths.
    pub fn try_calibrate_gnielinski_nusselt(&mut self,
        calibration_ratio: Ratio) -> Result<(), TuasLibError>{

        let mut fluid_arr_clone: FluidArray = 
            self.pipe_fluid_array.clone().try_into().unwrap();



        let calibrated_nusselt_correlation = match fluid_arr_clone.nusselt_correlation {
            NusseltCorrelation::PipeGnielinskiGeneric(gnielinski_data) => {
                NusseltCorrelation::PipeGnielinskiCalibrated(
                    gnielinski_data.clone(), calibration_ratio)
            },
            NusseltCorrelation::PipeGnielinskiCalibrated(gnielinski_data, _) => {
                NusseltCorrelation::PipeGnielinskiCalibrated(
                    gnielinski_data.clone(), calibration_ratio)
            },
            _ => todo!("not implemented for non Gnielinksi nusselt numbers"),
        };
        fluid_arr_clone.nusselt_correlation = calibrated_nusselt_correlation;

        self.pipe_fluid_array = fluid_arr_clone.into();
        Ok(())
    }
}
