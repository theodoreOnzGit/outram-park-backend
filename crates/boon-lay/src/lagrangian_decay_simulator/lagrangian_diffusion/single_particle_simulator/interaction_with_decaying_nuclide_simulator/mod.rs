use uom::si::diffusion_coefficient::square_meter_per_second;
use uom::si::f64::*;
use uom::si::ratio::ratio;
use uom::ConstZero;

use crate::lagrangian_decay_simulator::lagrangian_diffusion::single_particle_simulator::constructive_solid_geometry::{TrisoCell, TrisoRegion};
use crate::prelude::SingleNuclideSimulatorMC;
use crate::lagrangian_decay_simulator::lagrangian_diffusion::single_particle_simulator::SingleParticleDiffusionSimulatorMC;

impl SingleParticleDiffusionSimulatorMC {

    /// moves the particle in the SingleNuclideSimulatorMC 
    /// in a random walk direction 
    /// you'll need to define a linear number density (ie 
    /// macroscopic cross section)
    pub fn move_single_decaying_particle_isotropically(
        &mut self,
        single_particle_sim: &mut SingleNuclideSimulatorMC,
        sigma_s: LinearNumberDensity){

        self.position = single_particle_sim.position;

        self.scatter_isotropically_using_macro_xs(sigma_s);

        single_particle_sim.position = self.position;

    }

    /// moves the particle in the SingleNuclideSimulatorMC 
    /// in a Gaussian direction
    /// providing the mean free path and number of collisions 
    pub fn move_single_decaying_particle_gaussian_mfp_and_no_of_collisions(
        &mut self,
        single_particle_sim: &mut SingleNuclideSimulatorMC,
        mean_free_path: Length,
        no_of_collisions: u64,
    ){

        self.position = single_particle_sim.position;

        self.move_particle_gaussian_sampling_u64(mean_free_path, no_of_collisions);

        single_particle_sim.position = self.position;
    }

    /// moves the particle in the SingleNuclideSimulatorMC 
    /// in a Gaussian direction
    /// providing the mean free path and number of collisions 
    ///
    /// I want to have it done through 100 collisions rather than
    /// one single collision in every timestep
    ///
    pub fn move_single_decaying_particle_within_triso(
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

    /// this helps to auto_timestep based on the fourier number
    pub fn move_single_decaying_particle_within_triso_based_on_fourier_no(
        &mut self,
        single_particle_sim: &mut SingleNuclideSimulatorMC,
        triso_cell: TrisoCell,
        timestep: Time,
    ){

        self.position = single_particle_sim.position;
        let nuclide = single_particle_sim.get_current_nuclide();
        // I would like to scale timestep appropriately, based on diffusion 
        // coeff and lengthscales 
        //
        // it would seem that diff_coeff = m^2/s
        // then timestep is  delta_t = diff_coeff/(length_scale of layer)
        // let's try that

        // first get diffusion coeff
        let (x, y, z) = self.position;
        let pos: [Length; 3] = [x, y, z];
        let diffusion_coeff = triso_cell
            .try_get_diffusion_coefficient(pos, nuclide)
            .unwrap_or_else(|| DiffusionCoefficient::new::<square_meter_per_second>(1e-6));

        // 1e-4 is quite slow!
        // but it is the necessary accurate number
        let threshold_fourier_number: Ratio = Ratio::new::<ratio>(1e-2);

        let fourier_number_lengthscale: Length = 
            triso_cell.get_lengthscale_for_fourier_number(pos);

        // Fo = Dt/x^2 
        // t = Fo * x^2/D
        let auto_timestep: Time = threshold_fourier_number 
            * fourier_number_lengthscale * fourier_number_lengthscale 
            / diffusion_coeff;
        
        let mut timestep_remaining = timestep;

        if timestep > auto_timestep {

            // this is sub-timestepping step
            while timestep_remaining > Time::ZERO {

                // this method is brute force
                self.scatter_within_triso_particle_gaussian(triso_cell, nuclide, auto_timestep);

                timestep_remaining -= auto_timestep;
            }

            // once done, use remaining timestep
            self.scatter_within_triso_particle_gaussian(triso_cell, nuclide, timestep_remaining);

            // now, this algorithm is slow
            // Very slow! when too many particles end up in the buffer
            //
            // I'm thinking to use a pre-cached response, a library 
            // with:
            //
            // 1000 samples of 
            // - 1 normal distribution scatter
            // - 10 normal distribution scatters
            // - 100 normal distribution scatters 
            // - 1000 normal distribution scatters 
            // - 10000 normal distribution scatters
            //
            // Perhaps each normal distribution scatter can be based on the last 
            // one
            //
            // These will be standard normals, can be hard coded in
            // to save on calculation time.


        } else {

            self.scatter_within_triso_particle_gaussian(triso_cell, nuclide, timestep);
        }

        single_particle_sim.position = self.position;
    }

}

/// now, for particle movement, I may want to consider sub-timestepping the 
/// thing for the buffer layer or for any layer where diffusion is too small 
/// It seems that 10 seconds is too high for the 
/// fuel kernel, IPyC, and buffer layer 
///
/// 1 second is okay for fuel kernel and IPyC but not buffer layer 
///
/// I wonder what is the proper criteria though, for diffusion
///
/// if 1s is okay and 10s is not okay, I want to look at the 
/// diffusion coeff based timestep within those values
/// That is, the D = x^2/t 
///
/// so Dt/x^2 = some constant 
///
/// this is like a Courant number analogue
///
/// This module is strictly mean to test for this
#[cfg(test)]
pub mod tests_for_auto_timestepping;
