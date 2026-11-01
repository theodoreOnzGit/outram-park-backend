use ndarray::*;
use uom::si::f64::*;

use crate::teh_o_prke_error::TehOPrkeError;
use super::SixGroupPRKE;
impl SixGroupPRKE {
    /// solves for the neutron population and precursor concentration
    /// using an explicit time marching scheme
    pub fn solve_next_timestep_precursor_concentration_and_neutron_pop_vector_explicit(
        &mut self,
        timestep: Time,
        reactivity: Ratio,
        neutron_generation_time: Time,
        background_source_rate: VolumetricNumberRate,
    ) -> Result<Array1<VolumetricNumberDensity>, TehOPrkeError> {
        // gh:#1082: one PRKE core; see n_group_prke::rkf45_step. Pinned
        // bit-identical to the pre-refactor hand-unrolled RKF45 closure.
        crate::zero_power_prke::n_group_prke::rkf45_step(
            &self.decay_constant_array,
            &self.delayed_fraction_array,
            &mut self.precursor_and_neutron_pop_and_source_array,
            timestep,
            reactivity,
            neutron_generation_time,
            background_source_rate,
        )?;
        Ok(Array1::from(
            self.precursor_and_neutron_pop_and_source_array.to_vec(),
        ))
    }
}
