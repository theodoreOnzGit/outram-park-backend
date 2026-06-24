use std::{f64::consts::PI, sync::{Arc, Mutex}};

use boon_lay::{lagrangian_decay_simulator::lagrangian_diffusion::central_limit_theorem::oorandom_rng::OoRng64, prelude::{decay_library::DecayLibrary, SingleNuclideSimulatorMC}, Nuclide};
use openmc_libs::rng::lcg::{Lcg64, prn};
use uom::{si::f64::Length, ConstZero};

use crate::triso_simulator_v1::{front_end::triso_particle::TrisoParticleUi, TRISOSimApp};

impl TRISOSimApp {

    pub fn construct_new_single_thread_multi_particle_simulation(num_of_nuclides: u64,
        nuclide: Nuclide,
        rng_seed: u64)->
        Arc<Mutex<(Vec<SingleNuclideSimulatorMC>, DecayLibrary)>>{

            let mut decay_library = DecayLibrary::new();

            decay_library.random_number_generator =
                Lcg64::new(rng_seed as u128);

            let mut rng_for_position = OoRng64::from_u64(rng_seed);

            let new_simulation
                = SingleNuclideSimulatorMC::new_decay_chain_simulation(
                    nuclide, &mut decay_library
                );
            let mut v: Vec<SingleNuclideSimulatorMC> = vec![
                new_simulation; num_of_nuclides.try_into().unwrap()
            ];

            let new_triso_particle = TrisoParticleUi::default();

            let _buffer_radius = new_triso_particle.get_diameter_after_buffer() * 0.5;
            let _ipyc_radius = new_triso_particle.get_diameter_after_ipyc() * 0.5;
            let opyc_radius = new_triso_particle.get_diameter_after_opyc() * 0.5;
            let fuel_radius = new_triso_particle.get_diameter_after_fuel() * 0.5;

            for simulation in v.iter_mut() {
                let mut new_simulation
                    = SingleNuclideSimulatorMC::new_decay_chain_simulation(
                        nuclide, &mut decay_library
                    );

                let coordinate = Self::random_point_in_triso(
                    fuel_radius,
                    opyc_radius,
                    &mut decay_library.random_number_generator,
                    &mut rng_for_position,
                );

                new_simulation.position = coordinate;

                *simulation = new_simulation;
            }

            return Arc::new(Mutex::new(
                    (v,decay_library)
            ));
    }


    pub fn random_point_in_spherical_shell(
        r_in: Length,
        r_out: Length,
        seed: &mut u64,
    ) -> (Length, Length, Length) {
        assert!(r_in >= Length::ZERO && r_in < r_out, "Require 0 <= r_in < r_out");

        let u: f64 = prn(seed);
        let r_in3 = r_in * r_in * r_in;
        let r_out3 = r_out * r_out * r_out;
        let rho = (u * (r_out3 - r_in3) + r_in3).cbrt();

        let z: f64 = prn(seed) * 2.0 - 1.0;
        let phi: f64 = prn(seed) * 2.0 * PI;
        let t = (1.0 - z * z).max(0.0).sqrt();
        let x = t * phi.cos();
        let y = t * phi.sin();

        (rho * x, rho * y, rho * z)
    }

    pub fn random_point_in_triso(
        r_fuel: Length,
        r_opyc: Length,
        rng_for_layer: &mut Lcg64,
        rng_for_position: &mut OoRng64) -> (Length, Length, Length) {

        let layer_random_number: f64 = rng_for_layer.rand_float();

        if layer_random_number < 1.0 - 1e-5 {
            let coordinate = Self::random_point_in_spherical_shell(
                Length::ZERO,
                r_fuel,
                &mut rng_for_position.0
            );
            return coordinate;
        }

        let coordinate = Self::random_point_in_spherical_shell(
            r_fuel,
            r_opyc,
            &mut rng_for_position.0
        );

        return coordinate;
    }
}

pub mod simulator_state;
pub mod run;
