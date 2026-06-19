use ndarray::*;
use uom::si::f64::*;
use uom::si::time::second;
use uom::si::volumetric_number_density::per_cubic_meter;
use uom::si::volumetric_number_rate::per_cubic_meter_second;

use crate::teh_o_prke_error::TehOPrkeError;
use crate::time_stepping::openfoam_rfk45::RKF45;
use super::SixGroupPRKE;
impl SixGroupPRKE {

    /// solves for the neutron population and precursor concentration 
    /// using an explicit time marching scheme
    pub fn solve_next_timestep_precursor_concentration_and_neutron_pop_vector_explicit(
        &mut self,
        timestep: Time,
        reactivity: Ratio,
        neutron_generation_time: Time,
        background_source_rate: VolumetricNumberRate)
        -> Result<Array1<VolumetricNumberDensity>,TehOPrkeError> {


            // first, get neutron population and precursor density
            let neutron_population_number_density = self.precursor_and_neutron_pop_and_source_array[0];
            let delayed_neutron_precursor_group_1_concentration = self.precursor_and_neutron_pop_and_source_array[1];
            let delayed_neutron_precursor_group_2_concentration = self.precursor_and_neutron_pop_and_source_array[2];
            let delayed_neutron_precursor_group_3_concentration = self.precursor_and_neutron_pop_and_source_array[3];
            let delayed_neutron_precursor_group_4_concentration = self.precursor_and_neutron_pop_and_source_array[4];
            let delayed_neutron_precursor_group_5_concentration = self.precursor_and_neutron_pop_and_source_array[5];
            let delayed_neutron_precursor_group_6_concentration = self.precursor_and_neutron_pop_and_source_array[6];


            // the ode system here takes a time t 
            // and a vector of neutron population and precursor densities 
            // and then returns the vector of derivatives
            // that is 
            //
            // dn/dt = (rho - beta)/Lambda n(t) + sum_i^n lambda_i C_i + S
            // d C_i/dt = beta_i/Lambda n(t) - lambda_i C_i
            //
            // of course, be mindful that there is a background source rate 
            // for dn/dt

            // lets obtain the delayed fraction precursor and so on
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


            let total_delayed_fraction: Ratio = 
                self.delayed_fraction_array.clone().into_iter().sum();

            // let's have rho-beta/lambda
            // (rho - beta)/Lambda
            // this will give a frequency of sorts

            let rho_minus_beta_over_big_lambda: Frequency = 
                (reactivity - total_delayed_fraction)/neutron_generation_time;

            // then let's compute the beta by Lambda ratios 

            let beta_by_big_lambda_1: Frequency = beta_1/neutron_generation_time;
            let beta_by_big_lambda_2: Frequency = beta_2/neutron_generation_time;
            let beta_by_big_lambda_3: Frequency = beta_3/neutron_generation_time;
            let beta_by_big_lambda_4: Frequency = beta_4/neutron_generation_time;
            let beta_by_big_lambda_5: Frequency = beta_5/neutron_generation_time;
            let beta_by_big_lambda_6: Frequency = beta_6/neutron_generation_time;


            // now create a closure in SI units
            // that returns the derivative vector based on t and y
            // use si units
            let prke_ode_system = 
                |_t: f64, y: &Vec<f64>| -> Vec<f64> {

                    let neutron_population_number_density 
                        = VolumetricNumberDensity::new::<per_cubic_meter>(y[0]);
                    let delayed_neutron_precursor_group_1_concentration 
                        = VolumetricNumberDensity::new::<per_cubic_meter>(y[1]);
                    let delayed_neutron_precursor_group_2_concentration 
                        = VolumetricNumberDensity::new::<per_cubic_meter>(y[2]);
                    let delayed_neutron_precursor_group_3_concentration 
                        = VolumetricNumberDensity::new::<per_cubic_meter>(y[3]);
                    let delayed_neutron_precursor_group_4_concentration 
                        = VolumetricNumberDensity::new::<per_cubic_meter>(y[4]);
                    let delayed_neutron_precursor_group_5_concentration 
                        = VolumetricNumberDensity::new::<per_cubic_meter>(y[5]);
                    let delayed_neutron_precursor_group_6_concentration 
                        = VolumetricNumberDensity::new::<per_cubic_meter>(y[6]);

                    // dn/dt = (rho - beta)/Lambda n(t) + sum_i^n lambda_i C_i + S
                    // d C_i/dt = beta_i/Lambda n(t) - lambda_i C_i

                    let decay_rate_1: VolumetricNumberRate 
                        = (lambda_1 * delayed_neutron_precursor_group_1_concentration).into();
                    let decay_rate_2: VolumetricNumberRate 
                        = (lambda_2 * delayed_neutron_precursor_group_2_concentration).into();
                    let decay_rate_3: VolumetricNumberRate 
                        = (lambda_3 * delayed_neutron_precursor_group_3_concentration).into();
                    let decay_rate_4: VolumetricNumberRate 
                        = (lambda_4 * delayed_neutron_precursor_group_4_concentration).into();
                    let decay_rate_5: VolumetricNumberRate 
                        = (lambda_5 * delayed_neutron_precursor_group_5_concentration).into();
                    let decay_rate_6: VolumetricNumberRate 
                        = (lambda_6 * delayed_neutron_precursor_group_6_concentration).into();

                    let total_decay_rate: VolumetricNumberRate 
                        = decay_rate_1 +
                        decay_rate_2 +
                        decay_rate_3 +
                        decay_rate_4 +
                        decay_rate_5 +
                        decay_rate_6;

                    // neutron pop derivative
                    let mut dndt: VolumetricNumberRate = 
                        (rho_minus_beta_over_big_lambda * neutron_population_number_density).into();

                    dndt += total_decay_rate;
                    dndt += background_source_rate;
                    // then delayed precursor derivative

                    let mut dc1dt: VolumetricNumberRate = 
                        (beta_by_big_lambda_1 * neutron_population_number_density).into();
                    dc1dt -= decay_rate_1;
                    let mut dc2dt: VolumetricNumberRate = 
                        (beta_by_big_lambda_2 * neutron_population_number_density).into();
                    dc2dt -= decay_rate_2;
                    let mut dc3dt: VolumetricNumberRate = 
                        (beta_by_big_lambda_3 * neutron_population_number_density).into();
                    dc3dt -= decay_rate_3;
                    let mut dc4dt: VolumetricNumberRate = 
                        (beta_by_big_lambda_4 * neutron_population_number_density).into();
                    dc4dt -= decay_rate_4;
                    let mut dc5dt: VolumetricNumberRate = 
                        (beta_by_big_lambda_5 * neutron_population_number_density).into();
                    dc5dt -= decay_rate_5;
                    let mut dc6dt: VolumetricNumberRate = 
                        (beta_by_big_lambda_6 * neutron_population_number_density).into();
                    dc6dt -= decay_rate_6;


                    // start with empty vector
                    let mut dydt: Vec<f64> = vec![0.0; 7];

                    dydt[0] = dndt.get::<per_cubic_meter_second>();
                    dydt[1] = dc1dt.get::<per_cubic_meter_second>();
                    dydt[2] = dc2dt.get::<per_cubic_meter_second>();
                    dydt[3] = dc3dt.get::<per_cubic_meter_second>();
                    dydt[4] = dc4dt.get::<per_cubic_meter_second>();
                    dydt[5] = dc5dt.get::<per_cubic_meter_second>();
                    dydt[6] = dc6dt.get::<per_cubic_meter_second>();

                    dydt
                };


            // now the ode system needs to be solved because 
            // we need a stepsize dt 
            // as well as dydt = f(t,y)
            //
            // now, for prke, the dydt function is not 
            // explicitly based on time here 
            // it is based on a time changing reactivity based 
            // on user input and feedback mechanism.
            //
            // for the sake of this programming,
            // dydt = f(y)
            // so i need to make a dummy time to put this into the
            // function 
            //

            let dummy_time = timestep;
            let current_neutron_pop_and_precursor_vector_si_units: Vec<f64>
                = vec![
                neutron_population_number_density.get::<per_cubic_meter>(),
                delayed_neutron_precursor_group_1_concentration.get::<per_cubic_meter>(),
                delayed_neutron_precursor_group_2_concentration.get::<per_cubic_meter>(),
                delayed_neutron_precursor_group_3_concentration.get::<per_cubic_meter>(),
                delayed_neutron_precursor_group_4_concentration.get::<per_cubic_meter>(),
                delayed_neutron_precursor_group_5_concentration.get::<per_cubic_meter>(),
                delayed_neutron_precursor_group_6_concentration.get::<per_cubic_meter>(),
                ];

            let next_timestep_neutron_pop_and_precursor_vector_si_units = 
                RKF45::solve_functional_prog_single_stepsize_no_stepsize_adjust(
                    dummy_time.get::<second>(), 
                    current_neutron_pop_and_precursor_vector_si_units, 
                    timestep.get::<second>(), 
                    prke_ode_system
                    );

            let precursor_and_neutron_pop_and_source_vector_next_timestep: Vec<VolumetricNumberDensity>
                = 
                next_timestep_neutron_pop_and_precursor_vector_si_units
                .into_iter()
                .map(|num_density_f64|{
                    VolumetricNumberDensity::new::<per_cubic_meter>(num_density_f64)
                })
                .collect();

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
            Ok(precursor_and_neutron_pop_and_source_vector_next_timestep.into())
    }

}
