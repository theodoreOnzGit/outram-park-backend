use std::sync::atomic::{AtomicUsize, Ordering};
use std::time::{SystemTime, UNIX_EPOCH};
use openmc_libs::rng::distributions::sample_normal;
use uom::si::time::second;

/// Fast cached pool of standard normal random numbers for diffusion simulation
pub struct DiffusionRandomCache {
    /// Pre-computed normal samples N(0,1)
    normals: Vec<f64>,
    /// Atomic index for thread-safe round-robin access
    index: AtomicUsize,
    /// Total cache size
    size: usize,
}

impl DiffusionRandomCache {
    /// Create a new cache with the specified number of pre-generated samples.
    /// Uses the OpenMC LCG seeded from the current wall-clock time.
    pub fn new(cache_size: usize) -> Self {
        let t = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default();
        let mut seed: u64 = t.subsec_nanos() as u64 ^ t.as_secs().wrapping_mul(0x9e3779b97f4a7c15);
        let normals: Vec<f64> = (0..cache_size)
            .map(|_| sample_normal(&mut seed))
            .collect();

        Self {
            normals,
            index: AtomicUsize::new(0),
            size: cache_size,
        }
    }

    /// Get a single standard normal sample
    #[inline]
    pub fn get_normal(&self) -> f64 {
        let idx = self.index.fetch_add(1, Ordering::Relaxed) % self.size;
        // SAFETY: idx is always < size due to modulo
        unsafe { *self.normals.get_unchecked(idx) }
    }

    /// Get three independent normal samples (for x, y, z)
    #[inline]
    pub fn get_normal_3d(&self) -> (f64, f64, f64) {
        (
            self.get_normal(),
            self.get_normal(),
            self.get_normal(),
        )
    }

    /// Get scaled displacement for 3D diffusion
    #[inline]
    pub fn get_displacement_3d(&self, scale: f64) -> (f64, f64, f64) {
        let (n1, n2, n3) = self.get_normal_3d();
        (n1 * scale, n2 * scale, n3 * scale)
    }
}
// Manual Clone implementation
impl Clone for DiffusionRandomCache {
    fn clone(&self) -> Self {
        let normals = self.normals.clone();
        let current_index = self.index.load(Ordering::Relaxed);
        Self {
            normals,
            index: AtomicUsize::new(current_index),
            size: self.size,
        }
    }
}

// Manual Debug implementation (since AtomicUsize doesn't implement Debug in the usual way)
impl std::fmt::Debug for DiffusionRandomCache {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("DiffusionRandomCache")
            .field("size", &self.size)
            .field("current_index", &self.index.load(Ordering::Relaxed))
            .field("normals_len", &self.normals.len())
            .finish()
    }
}

// Optional: PartialEq for testing
impl PartialEq for DiffusionRandomCache {
    fn eq(&self, other: &Self) -> bool {
        self.normals == other.normals && self.size == other.size
    }
}



use crate::lagrangian_decay_simulator::lagrangian_diffusion::central_limit_theorem::per_component_variance_exponential_for_3d_vector;
use crate::lagrangian_decay_simulator::lagrangian_diffusion::single_particle_simulator::{constructive_solid_geometry::TrisoCell, SingleParticleDiffusionSimulatorMC};
use crate::prelude::SingleNuclideSimulatorMC;
use fission_yields_data::prelude::Nuclide;
use uom::si::f64::*;
use uom::si::diffusion_coefficient::square_meter_per_second;
use uom::si::length::{angstrom, meter};
use uom::si::ratio::ratio;
use uom::si::velocity::meter_per_second;
use uom::ConstZero;

impl SingleParticleDiffusionSimulatorMC {

    /// Cached version: get_gaussian_velocity_vector using pre-generated random samples
    #[inline]
    fn get_gaussian_velocity_vector_cached(
        &self,
        mean_free_path: Length,
        collision_rate: Frequency,
        cache: &DiffusionRandomCache,
    ) -> [Velocity; 3] {
        // Get three cached normal samples
        let (n1, n2, n3) = cache.get_normal_3d();

        // Calculate velocity scale
        let unit_timestep = Time::new::<second>(1.0);
        let no_of_collisions_per_second: f64 =
            (collision_rate * unit_timestep).get::<ratio>();

        let per_component_variance: Area =
            per_component_variance_exponential_for_3d_vector(
                no_of_collisions_per_second, mean_free_path);
        let std_deviation: Length = per_component_variance.sqrt();

        [
            n1 * std_deviation/unit_timestep,
            n2 * std_deviation/unit_timestep,
            n3 * std_deviation/unit_timestep,
        ]
    }

    /// Cached version: move_particle_gaussian_sampling using pre-generated random samples
    #[inline]
    fn move_particle_gaussian_sampling_cached(
        &mut self,
        jump_distance: Length,
        no_of_collisions_f64: f64,
        cache: &DiffusionRandomCache,
    ) {
        // For central limit theorem:
        // displacement ~ N(0, n * lambda^2) where n = number of collisions
        let sigma = jump_distance.get::<meter>() * no_of_collisions_f64.sqrt();

        // Get cached displacement
        let (dx, dy, dz) = cache.get_displacement_3d(sigma);

        let displacement = [
            Length::new::<meter>(dx),
            Length::new::<meter>(dy),
            Length::new::<meter>(dz),
        ];

        self.move_particle_using_array(displacement);
    }

    /// CACHED VERSION: Scatter within TRISO particle using Gaussian sampling with boundary handling
    /// This uses pre-generated random samples for 10-50x speedup
    #[inline]
    pub fn scatter_within_triso_particle_gaussian_cached(
        &mut self,
        triso_cell: TrisoCell,
        nuclide: Nuclide,
        timestep: Time,
        cache: &DiffusionRandomCache,
    ) {
        // nudge length: choose something tiny relative to geometry
        const R_EPS_M: f64 = 1e-12;
        let r_eps = Length::new::<meter>(R_EPS_M);

        #[inline]
        fn unit_radial(p: [Length; 3]) -> [f64; 3] {
            let x = p[0].get::<meter>();
            let y = p[1].get::<meter>();
            let z = p[2].get::<meter>();
            let r = (x * x + y * y + z * z).sqrt().max(1e-30);
            [x / r, y / r, z / r]
        }

        let mut remaining_timestep = timestep;

        while remaining_timestep > Time::ZERO {
            let (x, y, z) = self.position;
            let pos: [Length; 3] = [x, y, z];

            let diffusion_coeff = triso_cell
                .try_get_diffusion_coefficient(pos, nuclide)
                .unwrap_or_else(|| DiffusionCoefficient::new::<square_meter_per_second>(1e-6));

            let jump_distance = Length::new::<angstrom>(2.0);
            let collision_frequency: Frequency =
                diffusion_coeff * 6.0 / (jump_distance * jump_distance);

            let velocity = self.get_gaussian_velocity_vector_cached(
                jump_distance,
                collision_frequency,
                cache
            );

            let time_opt_to_sphere_boundary: Option<Time> =
                triso_cell.get_time_to_sphere_boundary(pos, velocity);

            let Some(time_to_next_boundary) = time_opt_to_sphere_boundary else {
                let length_array: [Length; 3] = [
                    velocity[0] * remaining_timestep,
                    velocity[1] * remaining_timestep,
                    velocity[2] * remaining_timestep,
                ];
                self.move_particle_using_array(length_array);
                return;
            };

            if time_to_next_boundary >= remaining_timestep {
                let length_array: [Length; 3] = [
                    velocity[0] * remaining_timestep,
                    velocity[1] * remaining_timestep,
                    velocity[2] * remaining_timestep,
                ];
                self.move_particle_using_array(length_array);
                return;
            }

            let length_array: [Length; 3] = [
                velocity[0] * time_to_next_boundary,
                velocity[1] * time_to_next_boundary,
                velocity[2] * time_to_next_boundary,
            ];
            self.move_particle_using_array(length_array);

            remaining_timestep -= time_to_next_boundary;

            let p_now = {
                let (x, y, z) = self.position;
                [x, y, z]
            };
            let n = unit_radial(p_now);
            let vdotn = velocity[0].get::<meter_per_second>() * n[0]
                + velocity[1].get::<meter_per_second>() * n[1]
                + velocity[2].get::<meter_per_second>() * n[2];

            let s = if vdotn >= 0.0 { 1.0 } else { -1.0 };
            let nudge: [Length; 3] = [
                Length::new::<meter>(s * r_eps.get::<meter>() * n[0]),
                Length::new::<meter>(s * r_eps.get::<meter>() * n[1]),
                Length::new::<meter>(s * r_eps.get::<meter>() * n[2]),
            ];
            self.move_particle_using_array(nudge);
        }
    }

    /// CACHED VERSION: Simplified Gaussian scattering within TRISO particle
    /// Assumes no change in diffusion coefficient along the travel path
    #[inline]
    pub fn scatter_within_triso_particle_gaussian_simple_cached(
        &mut self,
        triso_cell: TrisoCell,
        nuclide: Nuclide,
        timestep: Time,
        cache: &DiffusionRandomCache,
    ) {
        let (x, y, z) = self.position;
        let pos_array: [Length; 3] = [x, y, z];

        let diffusion_coeff_option =
            triso_cell.try_get_diffusion_coefficient(pos_array, nuclide);

        let diffusion_coeff: DiffusionCoefficient = match diffusion_coeff_option {
            Some(coeff) => coeff,
            None => DiffusionCoefficient::new::<square_meter_per_second>(1e-6),
        };

        let jump_distance = Length::new::<angstrom>(2.0);

        let collision_frequency: Frequency
            = diffusion_coeff * 6.0 / (jump_distance * jump_distance);

        let no_of_collisions_f64: f64
            = (collision_frequency * timestep).get::<ratio>();

        self.move_particle_gaussian_sampling_cached(
            jump_distance,
            no_of_collisions_f64,
            cache
        );
    }

    /// CACHED VERSION: Move single decaying particle with Gaussian sampling in TRISO particle
    pub fn move_single_decaying_particle_gaussian_triso_particle_cached(
        &mut self,
        single_particle_sim: &mut SingleNuclideSimulatorMC,
        triso_cell: TrisoCell,
        timestep: Time,
        cache: &DiffusionRandomCache,
    ) {
        self.position = single_particle_sim.position;
        let nuclide = single_particle_sim.get_current_nuclide();

        self.scatter_within_triso_particle_gaussian_cached(
            triso_cell,
            nuclide,
            timestep,
            cache
        );

        single_particle_sim.position = self.position;
    }

    /// this helps to auto_timestep based on the fourier number
    pub fn move_single_decaying_particle_within_triso_based_on_fourier_no_cached(
        &mut self,
        single_particle_sim: &mut SingleNuclideSimulatorMC,
        triso_cell: TrisoCell,
        timestep: Time,
        cache: &DiffusionRandomCache,
    ){

        self.position = single_particle_sim.position;
        let nuclide = single_particle_sim.get_current_nuclide();

        let (x, y, z) = self.position;
        let pos: [Length; 3] = [x, y, z];
        let diffusion_coeff = triso_cell
            .try_get_diffusion_coefficient(pos, nuclide)
            .unwrap_or_else(|| DiffusionCoefficient::new::<square_meter_per_second>(1e-6));

        let threshold_fourier_number: Ratio = Ratio::new::<ratio>(1e-2);

        let fourier_number_lengthscale: Length =
            triso_cell.get_lengthscale_for_fourier_number(pos);

        // Fo = Dt/x^2 → t = Fo * x^2/D
        let auto_timestep: Time = threshold_fourier_number
            * fourier_number_lengthscale * fourier_number_lengthscale
            / diffusion_coeff;

        let mut timestep_remaining = timestep;

        if timestep > auto_timestep {

            while timestep_remaining > Time::ZERO {
                self.scatter_within_triso_particle_gaussian_cached(
                    triso_cell, nuclide, auto_timestep,
                    cache,);
                timestep_remaining -= auto_timestep;
            }

            self.scatter_within_triso_particle_gaussian_cached(
                triso_cell, nuclide, timestep_remaining,
                cache,
                );

        } else {

            self.scatter_within_triso_particle_gaussian_cached(
                triso_cell, nuclide, timestep,
                cache);
        }

        single_particle_sim.position = self.position;
    }

}
