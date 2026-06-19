use ndarray::*;
use ndarray_linalg::Solve;
use uom::si::f64::*;
use uom::si::volumetric_number_density::per_cubic_meter;
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
        background_source_rate: VolumetricNumberRate)
        -> Result<Array1<VolumetricNumberDensity>,TehOPrkeError> {

            // first, construct coefficient matrix

            let coefficient_matrix = self.construct_coefficient_matrix(
                timestep, 
                reactivity, 
                neutron_generation_time);
            // next, unit test to check if the units match 
            {
                let _test_quantity: VolumetricNumberDensity = 
                    (coefficient_matrix[[6,0]]
                    * self.precursor_and_neutron_pop_and_source_array[0]).into();
            }

            // map this to a f64
            // all SI units

            let coefficient_matrix_float: Array2<f64> = coefficient_matrix.map(
                |&coefficient_dimensioned_quantity|{
                    let coefficient_float: f64 = 
                        coefficient_dimensioned_quantity.get::<ratio>();
                    coefficient_float
                }
            );

            let neutron_population_number_density = self.precursor_and_neutron_pop_and_source_array[0];
            let delayed_neutron_precursor_group_1_concentration = self.precursor_and_neutron_pop_and_source_array[1];
            let delayed_neutron_precursor_group_2_concentration = self.precursor_and_neutron_pop_and_source_array[2];
            let delayed_neutron_precursor_group_3_concentration = self.precursor_and_neutron_pop_and_source_array[3];
            let delayed_neutron_precursor_group_4_concentration = self.precursor_and_neutron_pop_and_source_array[4];
            let delayed_neutron_precursor_group_5_concentration = self.precursor_and_neutron_pop_and_source_array[5];
            let delayed_neutron_precursor_group_6_concentration = self.precursor_and_neutron_pop_and_source_array[6];

            let precursor_and_neutron_pop_and_source_array_with_background_source = 
                Self::construct_present_timestep_concentration_and_neutron_pop_vector(
                    delayed_neutron_precursor_group_1_concentration, 
                    delayed_neutron_precursor_group_2_concentration, 
                    delayed_neutron_precursor_group_3_concentration, 
                    delayed_neutron_precursor_group_4_concentration, 
                    delayed_neutron_precursor_group_5_concentration, 
                    delayed_neutron_precursor_group_6_concentration, 
                    neutron_population_number_density, 
                    background_source_rate, timestep);

            // neutron and precursor_and_neutron_pop_and_source_vector 
            // also must be mapped to f64 array

            let precursor_and_neutron_pop_and_source_vector: Array1<f64> 
                = precursor_and_neutron_pop_and_source_array_with_background_source.iter().
                map(|precursor_or_neutron_number_density|{
                    // all SI units
                    let number_density_float: f64 = 
                        precursor_or_neutron_number_density.get::<per_cubic_meter>();
                    number_density_float
                }).collect();

            let precursor_and_neutron_pop_and_source_vector_next_timestep_float: Array1<f64>
                = coefficient_matrix_float.solve(&precursor_and_neutron_pop_and_source_vector)?;

            let precursor_and_neutron_pop_and_source_vector_next_timestep: Array1<VolumetricNumberDensity> 
                = precursor_and_neutron_pop_and_source_vector_next_timestep_float.iter()
                .map(
                    |&precursor_or_neutron_number_density_float|{
                        VolumetricNumberDensity::new::<per_cubic_meter>(
                            precursor_or_neutron_number_density_float
                        )
                }).collect();

            // edit the neutron source vector, 
            // However, need to pivot around the source vector

            self.precursor_and_neutron_pop_and_source_array[0] = 
                precursor_and_neutron_pop_and_source_vector_next_timestep[0];
            self.precursor_and_neutron_pop_and_source_array[1] = 
                precursor_and_neutron_pop_and_source_vector_next_timestep[1];
            self.precursor_and_neutron_pop_and_source_array[2] = 
                precursor_and_neutron_pop_and_source_vector_next_timestep[2];
            self.precursor_and_neutron_pop_and_source_array[3] = 
                precursor_and_neutron_pop_and_source_vector_next_timestep[3];
            self.precursor_and_neutron_pop_and_source_array[4] = 
                precursor_and_neutron_pop_and_source_vector_next_timestep[4];
            self.precursor_and_neutron_pop_and_source_array[5] = 
                precursor_and_neutron_pop_and_source_vector_next_timestep[5];
            self.precursor_and_neutron_pop_and_source_array[6] = 
                precursor_and_neutron_pop_and_source_vector_next_timestep[6];

            // return to environment
            Ok(precursor_and_neutron_pop_and_source_vector_next_timestep)

    }

    /// constructs the vector for delayed neutron precursor concentration 
    /// and neutron population concentration
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
        timestep: Time
        ) -> 
        Array1<VolumetricNumberDensity>{

            let array_width_and_height = 7;
            let mut precursor_and_neutron_pop_and_source_vector: Array1<VolumetricNumberDensity> = 
                Array::zeros(array_width_and_height);

            precursor_and_neutron_pop_and_source_vector[1] = delayed_neutron_precursor_group_1_concentration;
            precursor_and_neutron_pop_and_source_vector[2] = delayed_neutron_precursor_group_2_concentration;
            precursor_and_neutron_pop_and_source_vector[3] = delayed_neutron_precursor_group_3_concentration;
            precursor_and_neutron_pop_and_source_vector[4] = delayed_neutron_precursor_group_4_concentration;
            precursor_and_neutron_pop_and_source_vector[5] = delayed_neutron_precursor_group_5_concentration;
            precursor_and_neutron_pop_and_source_vector[6] = delayed_neutron_precursor_group_6_concentration;


            let background_source_term: VolumetricNumberDensity = 
                (background_source_rate * timestep).into();

            precursor_and_neutron_pop_and_source_vector[0] = 
                background_source_term
                + neutron_population_number_density;


            precursor_and_neutron_pop_and_source_vector
    }

    /// constructs the matrix required for 
    /// solution of implicit six group PRKE
    pub fn construct_coefficient_matrix(&self,
        timestep: Time,
        reactivity: Ratio,
        neutron_generation_time: Time) -> Array2<Ratio> {

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

        let timestep_to_neutron_generation_time_ratio: Ratio = 
            timestep/neutron_generation_time;

        let total_delayed_fraction: Ratio = 
            self.delayed_fraction_array.clone().into_iter().sum();

        // top row coefficient
        let top_left_coefficient = Ratio::new::<ratio>(1.0) - 
            timestep_to_neutron_generation_time_ratio*(
                reactivity - total_delayed_fraction);
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
            coefficient_matrix[[0,0]] = top_left_coefficient;
            coefficient_matrix[[0,1]] = -delta_t_lambda_1;
            coefficient_matrix[[0,2]] = -delta_t_lambda_2;
            coefficient_matrix[[0,3]] = -delta_t_lambda_3;
            coefficient_matrix[[0,4]] = -delta_t_lambda_4;
            coefficient_matrix[[0,5]] = -delta_t_lambda_5;
            coefficient_matrix[[0,6]] = -delta_t_lambda_6;
        }
        // then the left column
        {
            coefficient_matrix[[1,0]] = -timestep_to_neutron_generation_time_ratio*beta_1;
            coefficient_matrix[[2,0]] = -timestep_to_neutron_generation_time_ratio*beta_2;
            coefficient_matrix[[3,0]] = -timestep_to_neutron_generation_time_ratio*beta_3;
            coefficient_matrix[[4,0]] = -timestep_to_neutron_generation_time_ratio*beta_4;
            coefficient_matrix[[5,0]] = -timestep_to_neutron_generation_time_ratio*beta_5;
            coefficient_matrix[[6,0]] = -timestep_to_neutron_generation_time_ratio*beta_6;
        }
        // Lastly, the diagonal
        {
            coefficient_matrix[[1,1]] = Ratio::new::<ratio>(1.0)+delta_t_lambda_1;
            coefficient_matrix[[2,2]] = Ratio::new::<ratio>(1.0)+delta_t_lambda_2;
            coefficient_matrix[[3,3]] = Ratio::new::<ratio>(1.0)+delta_t_lambda_3;
            coefficient_matrix[[4,4]] = Ratio::new::<ratio>(1.0)+delta_t_lambda_4;
            coefficient_matrix[[5,5]] = Ratio::new::<ratio>(1.0)+delta_t_lambda_5;
            coefficient_matrix[[6,6]] = Ratio::new::<ratio>(1.0)+delta_t_lambda_6;
        }

        // return the coefficient_matrix
        coefficient_matrix
    }

}
