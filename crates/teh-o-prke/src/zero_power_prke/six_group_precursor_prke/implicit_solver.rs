use ndarray::*;
use uom::si::f64::*;
use uom::si::ratio::ratio;

use crate::teh_o_prke_error::TehOPrkeError;
use super::SixGroupPRKE;

impl SixGroupPRKE {
    /// returns the next timestep neutron source vector
    ///
    /// also updates the current precursor and concentration vector
    ///
    /// this timestepping is implicit rather than explicit
    pub fn solve_next_timestep_precursor_concentration_and_neutron_pop_vector_implicit(
        &mut self,
        timestep: Time,
        reactivity: Ratio,
        neutron_generation_time: Time,
        background_source_rate: VolumetricNumberRate,
    ) -> Result<Array1<VolumetricNumberDensity>, TehOPrkeError> {
        // gh:#1082: one PRKE core. The arithmetic lives in the N-group module
        // and is pinned bit-identical to the pre-refactor hand-unrolled 7x7
        // solve (zero_power_prke::six_group_bit_identity_pin).
        crate::zero_power_prke::n_group_prke::backward_euler_step(
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

    /// constructs the vector for delayed neutron precursor concentration
    /// and neutron population concentration
    ///
    /// Since gh:#1082 kept for inspection only; the implicit step builds the
    /// same right-hand side in `n_group_prke::backward_euler_step`.
    ///
    /// note that the first precursor concentration group is that
    /// with the longest half life
    ///
    pub fn construct_present_timestep_concentration_and_neutron_pop_vector(
        delayed_neutron_precursor_group_1_concentration: VolumetricNumberDensity,
        delayed_neutron_precursor_group_2_concentration: VolumetricNumberDensity,
        delayed_neutron_precursor_group_3_concentration: VolumetricNumberDensity,
        delayed_neutron_precursor_group_4_concentration: VolumetricNumberDensity,
        delayed_neutron_precursor_group_5_concentration: VolumetricNumberDensity,
        delayed_neutron_precursor_group_6_concentration: VolumetricNumberDensity,
        neutron_population_number_density: VolumetricNumberDensity,
        background_source_rate: VolumetricNumberRate,
        timestep: Time,
    ) -> Array1<VolumetricNumberDensity> {
        let array_width_and_height = 7;
        let mut precursor_and_neutron_pop_and_source_vector: Array1<VolumetricNumberDensity> =
            Array::zeros(array_width_and_height);

        precursor_and_neutron_pop_and_source_vector[1] =
            delayed_neutron_precursor_group_1_concentration;
        precursor_and_neutron_pop_and_source_vector[2] =
            delayed_neutron_precursor_group_2_concentration;
        precursor_and_neutron_pop_and_source_vector[3] =
            delayed_neutron_precursor_group_3_concentration;
        precursor_and_neutron_pop_and_source_vector[4] =
            delayed_neutron_precursor_group_4_concentration;
        precursor_and_neutron_pop_and_source_vector[5] =
            delayed_neutron_precursor_group_5_concentration;
        precursor_and_neutron_pop_and_source_vector[6] =
            delayed_neutron_precursor_group_6_concentration;

        let background_source_term: VolumetricNumberDensity =
            (background_source_rate * timestep).into();

        precursor_and_neutron_pop_and_source_vector[0] =
            background_source_term + neutron_population_number_density;

        precursor_and_neutron_pop_and_source_vector
    }

    /// constructs the matrix required for
    /// solution of implicit six group PRKE
    ///
    /// Since gh:#1082 the implicit step assembles this same matrix, entry for
    /// entry, inside `n_group_prke::backward_euler_step` (for any number of
    /// groups); this function is kept for inspection and is no longer on the
    /// stepping path.
    pub fn construct_coefficient_matrix(
        &self,
        timestep: Time,
        reactivity: Ratio,
        neutron_generation_time: Time,
    ) -> Array2<Ratio> {
        // preliminaries
        let lambda_1 = self.decay_constant_array[0];
        let lambda_2 = self.decay_constant_array[1];
        let lambda_3 = self.decay_constant_array[2];
        let lambda_4 = self.decay_constant_array[3];
        let lambda_5 = self.decay_constant_array[4];
        let lambda_6 = self.decay_constant_array[5];

        let beta_1 = self.delayed_fraction_array[0];
        let beta_2 = self.delayed_fraction_array[1];
        let beta_3 = self.delayed_fraction_array[2];
        let beta_4 = self.delayed_fraction_array[3];
        let beta_5 = self.delayed_fraction_array[4];
        let beta_6 = self.delayed_fraction_array[5];

        let timestep_to_neutron_generation_time_ratio: Ratio = timestep / neutron_generation_time;

        let total_delayed_fraction: Ratio = self.delayed_fraction_array.clone().into_iter().sum();

        // top row coefficient
        let top_left_coefficient = Ratio::new::<ratio>(1.0)
            - timestep_to_neutron_generation_time_ratio * (reactivity - total_delayed_fraction);
        let delta_t_lambda_1: Ratio = timestep * lambda_1;
        let delta_t_lambda_2: Ratio = timestep * lambda_2;
        let delta_t_lambda_3: Ratio = timestep * lambda_3;
        let delta_t_lambda_4: Ratio = timestep * lambda_4;
        let delta_t_lambda_5: Ratio = timestep * lambda_5;
        let delta_t_lambda_6: Ratio = timestep * lambda_6;

        let array_width_and_height = 7;

        let mut coefficient_matrix: Array2<Ratio> =
            Array::zeros((array_width_and_height, array_width_and_height));

        // okay i won't pivot this time to avoid confusion

        // start changing the top row
        {
            coefficient_matrix[[0, 0]] = top_left_coefficient;
            coefficient_matrix[[0, 1]] = -delta_t_lambda_1;
            coefficient_matrix[[0, 2]] = -delta_t_lambda_2;
            coefficient_matrix[[0, 3]] = -delta_t_lambda_3;
            coefficient_matrix[[0, 4]] = -delta_t_lambda_4;
            coefficient_matrix[[0, 5]] = -delta_t_lambda_5;
            coefficient_matrix[[0, 6]] = -delta_t_lambda_6;
        }
        // then the left column
        {
            coefficient_matrix[[1, 0]] = -timestep_to_neutron_generation_time_ratio * beta_1;
            coefficient_matrix[[2, 0]] = -timestep_to_neutron_generation_time_ratio * beta_2;
            coefficient_matrix[[3, 0]] = -timestep_to_neutron_generation_time_ratio * beta_3;
            coefficient_matrix[[4, 0]] = -timestep_to_neutron_generation_time_ratio * beta_4;
            coefficient_matrix[[5, 0]] = -timestep_to_neutron_generation_time_ratio * beta_5;
            coefficient_matrix[[6, 0]] = -timestep_to_neutron_generation_time_ratio * beta_6;
        }
        // Lastly, the diagonal
        {
            coefficient_matrix[[1, 1]] = Ratio::new::<ratio>(1.0) + delta_t_lambda_1;
            coefficient_matrix[[2, 2]] = Ratio::new::<ratio>(1.0) + delta_t_lambda_2;
            coefficient_matrix[[3, 3]] = Ratio::new::<ratio>(1.0) + delta_t_lambda_3;
            coefficient_matrix[[4, 4]] = Ratio::new::<ratio>(1.0) + delta_t_lambda_4;
            coefficient_matrix[[5, 5]] = Ratio::new::<ratio>(1.0) + delta_t_lambda_5;
            coefficient_matrix[[6, 6]] = Ratio::new::<ratio>(1.0) + delta_t_lambda_6;
        }

        // return the coefficient_matrix
        coefficient_matrix
    }
}
