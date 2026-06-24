
use uom::{si::{f64::*, length::meter, linear_number_density::per_meter, ratio::ratio, time::second}, ConstZero};

use crate::lagrangian_decay_simulator::lagrangian_diffusion::isotropic_scattering::sample_isotropic_direction_into_array;
use crate::lagrangian_decay_simulator::lagrangian_diffusion::central_limit_theorem::sample_dimensioned_gaussian_vector;
use crate::lagrangian_decay_simulator::lagrangian_diffusion::central_limit_theorem::per_component_variance_exponential_for_3d_vector;
use crate::lagrangian_decay_simulator::lagrangian_diffusion::isotropic_scattering::sample_free_path;
use crate::lagrangian_decay_simulator::lagrangian_diffusion::central_limit_theorem::per_component_variance_exponential_for_3d_vector_u64;
use crate::lagrangian_decay_simulator::lagrangian_diffusion::central_limit_theorem::oorandom_rng::OoRng64;

#[derive(Debug,Clone,Copy,PartialEq)]
pub struct SingleParticleDiffusionSimulatorMC {
    pub position: (Length, Length, Length),
    /// random number generator
    pub rng: OoRng64,
}

impl SingleParticleDiffusionSimulatorMC {

    /// constructor for new diffusion simulator
    pub fn new_from_rng(outside_rng: &mut OoRng64) -> Self {

        let zero_length = Length::ZERO;
        let coordinates = (zero_length, zero_length, zero_length);

        // seed a child RNG from the parent
        let rng = OoRng64::from_u64(outside_rng.next_u64());

        return Self {
            position: coordinates,
            rng,
        }

    }


    /// this moves the particle by an array
    pub fn move_particle_using_array(&mut self, length_array: [Length; 3]){
        let dx = length_array[0];
        let dy = length_array[1];
        let dz = length_array[2];

        let (x, y, z) = self.position;

        self.position = (x+dx, y+dy, z+dz);

    }

    /// this moves the particle by an tuple
    pub fn move_particle_using_tuple(&mut self, length_tuple: (Length, Length, Length)){
        let (dx, dy, dz) = length_tuple;

        let (x, y, z) = self.position;

        self.position = (x+dx, y+dy, z+dz);

    }


    /// move particle assuming normal distribution
    /// given mean free path and number of collisions
    pub fn move_particle_gaussian_sampling_u64(&mut self,
        mean_free_path: Length,
        no_of_collisions: u64){

        let per_component_variance =
            per_component_variance_exponential_for_3d_vector_u64(
                no_of_collisions, mean_free_path);

        let gaussian_length_array =
            sample_dimensioned_gaussian_vector(
                &mut self.rng.0,
                per_component_variance,
            );

        self.move_particle_using_array(gaussian_length_array);

    }
    /// move particle assuming normal distribution
    /// given mean free path and number of collisions
    pub fn move_particle_gaussian_sampling_f64(&mut self,
        mean_free_path: Length,
        no_of_collisions: f64){

        let per_component_variance =
            per_component_variance_exponential_for_3d_vector(
                no_of_collisions, mean_free_path);

        let gaussian_length_array =
            sample_dimensioned_gaussian_vector(
                &mut self.rng.0,
                per_component_variance,
            );



        self.move_particle_using_array(gaussian_length_array);

    }

    #[inline]
    pub fn get_gaussian_velocity_vector(
        &mut self,
        mean_free_path: Length,
        collision_rate: Frequency) -> [Velocity;3] {

        let unit_timestep = Time::new::<second>(1.0);
        let no_of_collisions_per_second: f64 =
            (collision_rate * unit_timestep).get::<ratio>();

        let per_component_variance =
            per_component_variance_exponential_for_3d_vector(
                no_of_collisions_per_second, mean_free_path);

        // this is how far you travel in 1s
        let gaussian_length_array =
            sample_dimensioned_gaussian_vector(
                &mut self.rng.0,
                per_component_variance,
            );


        // I get back the velocity
        let gaussian_velocity_array =
            [
            gaussian_length_array[0]/unit_timestep,
            gaussian_length_array[1]/unit_timestep,
            gaussian_length_array[2]/unit_timestep,
            ];

        return gaussian_velocity_array;


    }

    /// samples isotropic direction
    pub fn sample_isotropic_direction(&mut self) -> [Ratio;3] {
        sample_isotropic_direction_into_array(&mut self.rng.0)
    }

    /// samples distance travelled given a mean free path
    /// given a macroscopic scattering cross section
    pub fn sample_mean_free_path_given_sigma_s(&mut self, sigma_s: LinearNumberDensity)
        -> Length {

            let sigma_s_per_meter: f64 = sigma_s.get::<per_meter>();
            let distance_travelled_randomised_meters =
                sample_free_path(&mut self.rng.0, sigma_s_per_meter);

            return Length::new::<meter>(distance_travelled_randomised_meters);

    }

    // if we have a scattering marcoscopic cross section,
    // we can scatter the particle isotropically
    #[inline]
    pub fn scatter_isotropically_using_macro_xs(
        &mut self,
        sigma_s: LinearNumberDensity,
    ){
        let [dx_unit, dy_unit, dz_unit] = self.sample_isotropic_direction();
        let randomly_sampled_length = self.sample_mean_free_path_given_sigma_s(sigma_s);

        let dx = dx_unit * randomly_sampled_length;
        let dy = dy_unit * randomly_sampled_length;
        let dz = dz_unit * randomly_sampled_length;

        let length_array = [dx,dy,dz];
        self.move_particle_using_array(length_array);

    }



    // now, for challenge with scattering is that we want to
    // is that we want to have them precalculated.
    //
    // That isn't easy, but I'll probably do this another day
}

/// implements conversion and interaction with the
/// SingleNuclideSimulatorMC
pub mod interaction_with_decaying_nuclide_simulator;


/// implements movement within triso particle regime
pub mod movement_within_triso_particle;




/// next challenge is how do we include geometry?
/// There is simple constructive solid geometry,
/// then there are more complex things like STL files
///
///
/// I mean there can be more complex ways to do things,
/// but the simplest is with constructive solid geometry
///
/// simplest thing is a sphere.
///
/// where the norm can be used to determine if a a coordinate is within
/// the sphere or not
pub mod constructive_solid_geometry;

/// from
/// https://www-eng.lbl.gov/~shuman/NEXT/MATERIALS&COMPONENTS/Xe_damage/Crank-The-Mathematics-of-Diffusion.pdf
/// page 91
/// the total amount of diffusing substance entering or leaving a sphere is
/// Mt/M_infty = 1 - 6/(pi^2) \sum_(i=1)^infty 1/n^2 exp (- D n^2 pi^2 t/a^2)
///
/// Crank, J. (1975). The mathematics of diffusion (2nd ed.). Clarendon Press.
///
/// This is for a sphere
///
pub mod release_fraction_analytical_solution;

/// this is for caching of standard normals so that simulations are sped up
pub mod cached_normals;

/// for CRP 6 case 1a and 1b we can compare the Monte Carlo simulation
/// to the analytical solution
///
/// TO BE DONE
pub mod release_fraction_crp_6_case_1a_1b;
