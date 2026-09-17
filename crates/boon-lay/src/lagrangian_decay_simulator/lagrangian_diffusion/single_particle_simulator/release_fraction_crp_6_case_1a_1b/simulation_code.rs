//! Monte-Carlo fractional release from a bare fuel kernel (IAEA CRP-6 Case 1).
//!
//! CRP-6 Case 1 is single-layer diffusion: a spherical fuel kernel with a
//! **uniform initial concentration** of a fission product and a **perfect-sink**
//! surface. The fraction released by time `t` has the closed-form Crank series
//! solution (see [`calculate_analytical_fraction_released`]); this module
//! reproduces it with the Walk-on-Spheres first-passage engine, which is the
//! Lagrangian counterpart of that continuum (Eulerian) solution.
//!
//! [`calculate_analytical_fraction_released`]:
//! crate::lagrangian_decay_simulator::lagrangian_diffusion::single_particle_simulator::release_fraction_analytical_solution::calculate_analytical_fraction_released

use fission_yields_data::prelude::Nuclide;
use uom::si::f64::*;
use uom::ConstZero;

use crate::lagrangian_decay_simulator::lagrangian_diffusion::central_limit_theorem::oorandom_rng::OoRng64;
use crate::lagrangian_decay_simulator::lagrangian_diffusion::first_passage::walk_on_spheres::{
    sample_uniform_in_ball, WoSWalker,
};
use crate::lagrangian_decay_simulator::lagrangian_diffusion::temperature_dependent_collisions::{
    try_get_diffusion_coeff_jiang, TrisoPebbleLayerMaterial,
};

/// Monte-Carlo fractional release of `nuclide` from a bare spherical UO2 kernel.
///
/// Places `n_histories` atoms uniformly through the kernel volume, walks each to
/// the perfect-sink surface with the Walk-on-Spheres engine, and returns the
/// fraction whose release time is at or before `time`. The kernel diffusion
/// coefficient is the temperature-dependent Jiang correlation (no neutron
/// fluence), matching the analytical verification cases in the sibling
/// `release_fraction_analytical_solution` module.
///
/// # Arguments
///
/// - `nuclide` — the diffusing fission product (e.g. `Cs137`).
/// - `kernel_radius` — the UO2 kernel radius (`Length`; 212.5 µm for CRP-6).
/// - `temperature` — kernel temperature (`ThermodynamicTemperature`).
/// - `time` — elapsed time at which the release fraction is evaluated.
/// - `n_histories` — number of independent atoms simulated (statistical error
///   scales as `1/sqrt(n_histories)`).
/// - `seed` — RNG seed for reproducibility.
///
/// # Returns
///
/// The released fraction in `[0, 1]`.
pub fn mc_kernel_release_fraction(
    nuclide: Nuclide,
    kernel_radius: Length,
    temperature: ThermodynamicTemperature,
    time: Time,
    n_histories: usize,
    seed: u64,
) -> f64 {
    let diffusion_coefficient = try_get_diffusion_coeff_jiang(
        TrisoPebbleLayerMaterial::KernelUO2,
        nuclide,
        temperature,
        Some(ArealNumberDensity::ZERO),
    )
    .expect("kernel diffusion coefficient should be available for this nuclide/temperature");

    let capture_eps = kernel_radius * 1e-3;
    let mut master = OoRng64::from_u64(seed);
    let mut released = 0usize;

    for _ in 0..n_histories {
        let start = sample_uniform_in_ball(&mut master.0, kernel_radius);
        let child = OoRng64::from_u64(master.next_u64());
        let mut walker = WoSWalker::new(start, nuclide, child);
        let release_time =
            walker.walk_to_absorbing_sphere(kernel_radius, diffusion_coefficient, capture_eps);
        if release_time <= time {
            released += 1;
        }
    }

    released as f64 / n_histories as f64
}

/// The kernel diffusion coefficient used by [`mc_kernel_release_fraction`], for
/// reporting alongside a release fraction (e.g. in a V&V record).
pub fn kernel_diffusion_coefficient(
    nuclide: Nuclide,
    temperature: ThermodynamicTemperature,
) -> DiffusionCoefficient {
    try_get_diffusion_coeff_jiang(
        TrisoPebbleLayerMaterial::KernelUO2,
        nuclide,
        temperature,
        Some(ArealNumberDensity::ZERO),
    )
    .expect("kernel diffusion coefficient should be available for this nuclide/temperature")
}
