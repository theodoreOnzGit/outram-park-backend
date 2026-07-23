//! Parallel ensembles of independent Lagrangian histories.
//!
//! Each atom's Walk-on-Spheres history is completely independent of every other,
//! so an ensemble is embarrassingly parallel. This module runs the histories
//! across cores with `rayon`, giving a real, verifiable speedup on the CPU (the
//! wgpu compute path in [`super::super`]'s GPU module extends the same idea to
//! the GPU for very large, real-time ensembles).
//!
//! Reproducibility: each history `i` gets an independent RNG stream seeded
//! deterministically from a base seed and `i` (a SplitMix64 mix), so a run is
//! bit-for-bit reproducible and independent of how `rayon` schedules the work.

use fission_yields_data::prelude::Nuclide;
use rayon::prelude::*;
use uom::si::f64::*;

use crate::lagrangian_decay_simulator::lagrangian_diffusion::central_limit_theorem::oorandom_rng::OoRng64;
use crate::lagrangian_decay_simulator::lagrangian_diffusion::first_passage::depletion::{
    DepletionOutcome, Transmutation,
};
use crate::lagrangian_decay_simulator::lagrangian_diffusion::first_passage::walk_on_spheres::{
    sample_uniform_in_ball, WalkParams, WoSWalker,
};
use crate::lagrangian_decay_simulator::lagrangian_diffusion::single_particle_simulator::constructive_solid_geometry::TrisoCell;
use crate::lagrangian_decay_simulator::lagrangian_diffusion::single_particle_simulator::release_fraction_crp_6_case_1a_1b::simulation_code::kernel_diffusion_coefficient;
use crate::nuclide_reaction_and_decay_data::decay_library::DecayLibrary;

/// Independent, well-separated RNG seed for history `index` from `base_seed`.
///
/// A SplitMix64 finaliser so that consecutive indices produce far-apart streams
/// (a plain `base + index` would give highly correlated LCG sequences).
#[inline]
pub fn history_seed(base_seed: u64, index: usize) -> u64 {
    let mut z = base_seed ^ (index as u64).wrapping_mul(0x9E37_79B9_7F4A_7C15);
    z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
    z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
    z ^ (z >> 31)
}

/// Size and seeding of a parallel ensemble.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct EnsembleConfig {
    /// Number of independent atom histories to simulate.
    pub n_histories: usize,
    /// Base RNG seed; combined with the history index for per-atom streams.
    pub base_seed: u64,
}

/// Parallel Monte-Carlo fractional release from a bare fuel kernel (CRP-6 Case
/// 1), evaluated at `time`.
///
/// The parallel counterpart of
/// [`super::super::single_particle_simulator::release_fraction_crp_6_case_1a_1b::simulation_code::mc_kernel_release_fraction`]:
/// same physics, histories spread across cores. Returns the released fraction in
/// `[0, 1]`.
pub fn parallel_kernel_release_fraction(
    nuclide: Nuclide,
    kernel_radius: Length,
    temperature: ThermodynamicTemperature,
    time: Time,
    config: &EnsembleConfig,
) -> f64 {
    let diffusion_coefficient = kernel_diffusion_coefficient(nuclide, temperature);
    let capture_eps = kernel_radius * 1e-3;

    let released: usize = (0..config.n_histories)
        .into_par_iter()
        .map(|i| {
            let mut master = OoRng64::from_u64(history_seed(config.base_seed, i));
            let start = sample_uniform_in_ball(&mut master.0, kernel_radius);
            let child = OoRng64::from_u64(master.next_u64());
            let mut walker = WoSWalker::new(start, nuclide, child);
            let release_time =
                walker.walk_to_absorbing_sphere(kernel_radius, diffusion_coefficient, capture_eps);
            usize::from(release_time <= time)
        })
        .sum();

    released as f64 / config.n_histories as f64
}

/// Parallel multilayer + depletion ensemble.
///
/// Births `config.n_histories` atoms of `initial_nuclide` at the particle centre
/// and advances each — diffusing while decaying and transmuting — until it is
/// released or its simulated time reaches `until`. Returns one
/// [`DepletionOutcome`] per history. The decay library is shared read-only across
/// threads.
pub fn parallel_advance_until(
    initial_nuclide: Nuclide,
    triso_cell: &TrisoCell,
    params: &WalkParams,
    decay_library: &DecayLibrary,
    transmutation: Transmutation,
    until: Time,
    config: &EnsembleConfig,
) -> Vec<DepletionOutcome> {
    (0..config.n_histories)
        .into_par_iter()
        .map(|i| {
            let mut master = OoRng64::from_u64(history_seed(config.base_seed, i));
            let child = OoRng64::from_u64(master.next_u64());
            let mut walker = WoSWalker::new_at_center(initial_nuclide, child);
            walker.advance_until(triso_cell, params, decay_library, transmutation, until)
        })
        .collect()
}

/// Fraction of an outcome set that was released (an inventory-release summary).
pub fn released_fraction(outcomes: &[DepletionOutcome]) -> f64 {
    if outcomes.is_empty() {
        return 0.0;
    }
    let released = outcomes
        .iter()
        .filter(|o| matches!(o, DepletionOutcome::Released { .. }))
        .count();
    released as f64 / outcomes.len() as f64
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::lagrangian_decay_simulator::lagrangian_diffusion::single_particle_simulator::release_fraction_analytical_solution::calculate_analytical_fraction_released;
    use uom::si::length::micrometer;
    use uom::si::thermodynamic_temperature::degree_celsius;
    use uom::si::time::hour;

    #[test]
    fn history_seeds_are_distinct() {
        let a = history_seed(42, 0);
        let b = history_seed(42, 1);
        let c = history_seed(42, 2);
        assert_ne!(a, b);
        assert_ne!(b, c);
        assert_ne!(a, c);
    }

    #[test]
    fn parallel_run_is_reproducible() {
        let nuclide = Nuclide::Cs137;
        let radius = Length::new::<micrometer>(212.5);
        let temp = ThermodynamicTemperature::new::<degree_celsius>(1200.0);
        let time = Time::new::<hour>(200.0);
        let config = EnsembleConfig {
            n_histories: 5_000,
            base_seed: 0xABCD,
        };
        let a = parallel_kernel_release_fraction(nuclide, radius, temp, time, &config);
        let b = parallel_kernel_release_fraction(nuclide, radius, temp, time, &config);
        assert_eq!(a, b, "same config must give identical results");
    }

    #[test]
    fn parallel_kernel_release_matches_crank() {
        let nuclide = Nuclide::Cs137;
        let radius = Length::new::<micrometer>(212.5);
        let temp = ThermodynamicTemperature::new::<degree_celsius>(1200.0);
        let time = Time::new::<hour>(200.0);
        let config = EnsembleConfig {
            n_histories: 40_000,
            base_seed: 0x0C1A_5EED,
        };
        let mc = parallel_kernel_release_fraction(nuclide, radius, temp, time, &config);
        let d = kernel_diffusion_coefficient(nuclide, temp);
        let crank = calculate_analytical_fraction_released(d, radius, time, 200);
        assert!(
            (mc - crank).abs() < 0.02,
            "parallel MC {mc:.4} vs Crank {crank:.4}"
        );
    }
}
