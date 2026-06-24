use crate::lagrangian_decay_simulator::lagrangian_diffusion::single_particle_simulator::constructive_solid_geometry::TrisoRegion;
use crate::lagrangian_decay_simulator::lagrangian_diffusion::single_particle_simulator::{constructive_solid_geometry::TrisoCell, SingleParticleDiffusionSimulatorMC};
use crate::prelude::SingleNuclideSimulatorMC;
use fission_yields_data::prelude::Nuclide;
use uom::si::f64::*;
use uom::si::diffusion_coefficient::square_meter_per_second;
use uom::si::length::{angstrom, meter};
use uom::si::ratio::ratio;
use uom::ConstZero;

impl SingleParticleDiffusionSimulatorMC {

    // this deals with movement within triso particles
    #[inline]
    pub fn scatter_within_triso_particle_gaussian(
        &mut self,
        triso_cell: TrisoCell,
        nuclide: Nuclide,
        timestep: Time,
    ) {
        // --- tolerances for (2) and (3) ---

        // nudge length: choose something tiny relative to geometry
        // (meter here; tune based on your smallest layer thickness)
        const R_EPS_M: f64 = 1e-12;
        let r_eps = Length::new::<meter>(R_EPS_M);


        #[inline]
        fn unit_radial(p: [Length; 3]) -> [f64; 3] {
            // returns dimensionless unit vector
            let x = p[0].get::<meter>();
            let y = p[1].get::<meter>();
            let z = p[2].get::<meter>();
            let r = (x * x + y * y + z * z).sqrt().max(1e-30);
            [x / r, y / r, z / r]
        }

        let mut remaining_timestep = timestep;

        while remaining_timestep > Time::ZERO {
                    // diffusion coeff at current position
        let (x, y, z) = self.position;
        let pos: [Length; 3] = [x, y, z];

        let diffusion_coeff = triso_cell
            .try_get_diffusion_coefficient(pos, nuclide)
            .unwrap_or_else(|| DiffusionCoefficient::new::<square_meter_per_second>(1e-6));

        let jump_distance = Length::new::<angstrom>(2.0);
        let collision_frequency: Frequency =
            diffusion_coeff * 6.0 / (jump_distance * jump_distance);

        // sample ONE velocity for this iteration
        let velocity = self.get_gaussian_velocity_vector(jump_distance, collision_frequency);

        let time_opt_to_sphere_boundary: Option<Time> =
            triso_cell.get_time_to_sphere_boundary(pos, velocity);

        // now after velocity sampled, I also want to sample a timestep 

        let calc_diffusion_timestep = false; 

        if calc_diffusion_timestep {

            // next get lengthscale
            let region = triso_cell.get_triso_region(pos);

            let triso_layer_lengthscale = match region {
                TrisoRegion::Fuel => triso_cell.get_fuel_radius(),
                TrisoRegion::Buffer => triso_cell.get_buffer_radius() - triso_cell.get_fuel_radius(),
                TrisoRegion::IPyC => triso_cell.get_ipyc_radius() - triso_cell.get_buffer_radius(),
                TrisoRegion::SiC => triso_cell.get_sic_radius() - triso_cell.get_ipyc_radius(),
                TrisoRegion::OPyC => triso_cell.get_opyc_radius() - triso_cell.get_sic_radius(),

                // For the 'Outside' region, there is no containing shell. A reasonable
                // default is the radius of the entire particle, representing the boundary
                // that was just crossed. Another option could be Length::ZERO if this
                // state should be handled specially, but using the particle radius is safer
                // to avoid potential division-by-zero errors later.
                TrisoRegion::Outside => triso_cell.get_opyc_radius(),
            };

            // we get a diffusion scaled timestep 

            let mut diffusion_scaled_timestep: Time = 
                triso_layer_lengthscale * 
                triso_layer_lengthscale / 
                diffusion_coeff;

            // this is an arbitrary timesteps scale factor to 
            // enable the 
            let timestep_scale_factor = 1e-5;

            diffusion_scaled_timestep *= timestep_scale_factor;
            //dbg!(&diffusion_scaled_timestep);
        };



        let Some(time_to_next_boundary) = time_opt_to_sphere_boundary else {
            // no boundary ahead: finish remaining time with THIS velocity
            let length_array: [Length; 3] = [
                velocity[0] * remaining_timestep,
                velocity[1] * remaining_timestep,
                velocity[2] * remaining_timestep,
            ];
            self.move_particle_using_array(length_array);
            return;
        };

        // IMPORTANT: if boundary is after the remaining time, move remaining time using SAME velocity and exit
        if time_to_next_boundary >= remaining_timestep {
            let length_array: [Length; 3] = [
                velocity[0] * remaining_timestep,
                velocity[1] * remaining_timestep,
                velocity[2] * remaining_timestep,
            ];
            self.move_particle_using_array(length_array);
            return;
        }

        // otherwise move exactly to boundary
        let length_array: [Length; 3] = [
            velocity[0] * time_to_next_boundary,
            velocity[1] * time_to_next_boundary,
            velocity[2] * time_to_next_boundary,
        ];
        self.move_particle_using_array(length_array);

        remaining_timestep -= time_to_next_boundary;

            // (3) Nudge across boundary so next iteration doesn't re-hit at t≈0
            let p_now = {
                let (x, y, z) = self.position;
                [x, y, z]
            };
            let n = unit_radial(p_now);
            let vdotn = velocity[0].get::<uom::si::velocity::meter_per_second>() * n[0]
                + velocity[1].get::<uom::si::velocity::meter_per_second>() * n[1]
                + velocity[2].get::<uom::si::velocity::meter_per_second>() * n[2];

            let s = if vdotn >= 0.0 { 1.0 } else { -1.0 };
            let nudge: [Length; 3] = [
                Length::new::<meter>(s * r_eps.get::<meter>() * n[0]),
                Length::new::<meter>(s * r_eps.get::<meter>() * n[1]),
                Length::new::<meter>(s * r_eps.get::<meter>() * n[2]),
            ];
            self.move_particle_using_array(nudge);
        }

        // Final move for whatever time remains
        if remaining_timestep > Time::ZERO {
            let (x, y, z) = self.position;
            let pos: [Length; 3] = [x, y, z];

            let diffusion_coeff = triso_cell
                .try_get_diffusion_coefficient(pos, nuclide)
                .unwrap_or_else(|| DiffusionCoefficient::new::<uom::si::diffusion_coefficient::square_meter_per_second>(1e-6));

            let jump_distance = Length::new::<uom::si::length::angstrom>(2.0);
            let collision_frequency: Frequency =
                diffusion_coeff * 6.0 / (jump_distance * jump_distance);

            let velocity = self.get_gaussian_velocity_vector(jump_distance, collision_frequency);

            let length_array: [Length; 3] = [
                velocity[0] * remaining_timestep,
                velocity[1] * remaining_timestep,
                velocity[2] * remaining_timestep,
            ];
            self.move_particle_using_array(length_array);
        }
    }


    /// this deals with movement within triso particles
    ///
    /// assuming there is no change in diffusion coefficient along the 
    /// travel path of the particle
    #[inline]
    pub fn scatter_within_triso_particle_gaussian_simple(
        &mut self, 
        triso_cell: TrisoCell,
        nuclide: Nuclide,
        timestep: Time,
        ){

        // first find the diffusion coeff 
        let (x,y,z) = self.position;
        let pos_array: [Length;3] = [x,y,z];

        let diffusion_coeff_option = 
            triso_cell.try_get_diffusion_coefficient(pos_array, nuclide);

        let diffusion_coeff: DiffusionCoefficient = match diffusion_coeff_option {
            Some(coeff) => coeff,
            // the default diffusion coefficient is same as a cracked layer 
            // unless otherwise stated
            None => DiffusionCoefficient::new::<square_meter_per_second>(1e-6),
        };

        // now when having diffusion coeff, I need a mean free path and number 
        // of collisions
        // I'm going to use a jump distance of 2 angstroms 
        let jump_distance = Length::new::<angstrom>(2.0);

        // using D = 1/6 lambda^2 * nu 

        let collision_frequency: Frequency 
            = diffusion_coeff * 6.0 / (jump_distance * jump_distance);

        let no_of_collisions_f64: f64 
            = (collision_frequency * timestep).get::<ratio>();



        self.move_particle_gaussian_sampling_f64(jump_distance, 
            no_of_collisions_f64);


    }

    /// moves the particle in the SingleNuclideSimulatorMC 
    /// in a Gaussian direction
    /// within a triso particle
    pub fn move_single_decaying_particle_gaussian_triso_particle(
        &mut self,
        single_particle_sim: &mut SingleNuclideSimulatorMC,
        triso_cell: TrisoCell,
        timestep: Time,
    ){

        self.position = single_particle_sim.position;
        let nuclide = single_particle_sim.get_current_nuclide();

        self.scatter_within_triso_particle_gaussian(triso_cell, nuclide, timestep);

        single_particle_sim.position = self.position;
    }



    // this deals with movement within triso particles
    //
    // This is for scattering within triso particles which is by brute 
    // force, that is one scatter at a time (I reckon this will be quite lengthy)
    #[inline]
    pub fn scatter_within_triso_particle_brute_force(
        &mut self, 
        triso_cell: TrisoCell,
        nuclide: Nuclide,
        timestep: Time,
        ){

        // first find the diffusion coeff 
        let (x,y,z) = self.position;
        let pos_array: [Length;3] = [x,y,z];

        let diffusion_coeff_option = 
            triso_cell.try_get_diffusion_coefficient(pos_array, nuclide);

        let diffusion_coeff: DiffusionCoefficient = match diffusion_coeff_option {
            Some(coeff) => coeff,
            // the default diffusion coefficient is same as a cracked layer 
            // unless otherwise stated
            None => DiffusionCoefficient::new::<square_meter_per_second>(1e-6),
        };

        // now when having diffusion coeff, I need a mean free path and number 
        // of collisions
        // I'm going to use a jump distance of 2 angstroms 
        let jump_distance = Length::new::<angstrom>(2.0);

        // using D = 1/6 lambda^2 * nu 

        let collision_frequency: Frequency 
            = diffusion_coeff * 6.0 / (jump_distance * jump_distance);

        let no_of_collisions_f64: f64 
            = (collision_frequency * timestep).get::<ratio>();



        self.move_particle_gaussian_sampling_f64(jump_distance, 
            no_of_collisions_f64);


    }
}
