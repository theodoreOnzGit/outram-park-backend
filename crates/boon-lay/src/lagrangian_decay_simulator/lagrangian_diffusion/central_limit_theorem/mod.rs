use openmc_libs::rng::distributions::sample_normal;
use uom::si::f64::*;


/// Compute per-component variance sigma2 for the Gaussian displacement after n isotropic steps.
/// General case: sigma2 = n * E[S^2] / 3.
/// For exponential step lengths with mean lambda, E[S^2] = 2 lambda^2 ⇒ sigma2 = n * 2 lambda^2 / 3.
///
#[inline]
pub fn per_component_variance_from_second_moment_u64(
    no_of_collisions: u64, e_s2: Area
) -> Area {
    (no_of_collisions as f64) * e_s2 / 3.0
}

/// this obtains the variance given n random collisions
/// and a mean free path length
///
/// denoted as lambda
///
/// this is meant for 3d vector
#[inline]
pub fn per_component_variance_exponential_for_3d_vector_u64(
    no_of_collisions: u64, mean_free_path: Length) -> Area {
    let e_s2: Area = 2.0 * mean_free_path * mean_free_path;
    per_component_variance_from_second_moment_u64(no_of_collisions, e_s2)
}

/// Compute per-component variance sigma2 for the Gaussian displacement after n isotropic steps.
/// General case: sigma2 = n * E[S^2] / 3.
/// For exponential step lengths with mean lambda, E[S^2] = 2 lambda^2 ⇒ sigma2 = n * 2 lambda^2 / 3.
///
#[inline]
pub fn per_component_variance_from_second_moment(
    no_of_collisions: f64, e_s2: Area
) -> Area {
    (no_of_collisions) * e_s2 / 3.0
}

/// this obtains the variance given n random collisions
/// and a mean free path length
///
/// denoted as lambda
///
/// this is meant for 3d vector
#[inline]
pub fn per_component_variance_exponential_for_3d_vector(
    no_of_collisions: f64, mean_free_path: Length) -> Area {
    let e_s2: Area = 2.0 * mean_free_path * mean_free_path;
    per_component_variance_from_second_moment(no_of_collisions, e_s2)
}



#[inline]
/// Sample a 3D Gaussian displacement vector X ~ N(0, sigma2 * I3).
pub fn sample_dimensioned_gaussian_vector(seed: &mut u64, per_component_variance: Area) -> [Length; 3] {
    let std_deviation = per_component_variance.sqrt();
    let x = sample_normal(seed);
    let y = sample_normal(seed);
    let z = sample_normal(seed);
    [std_deviation * x, std_deviation * y, std_deviation * z]
}


/// OoRng64 adapter (now wraps the OpenMC LCG instead of oorandom)
pub mod oorandom_rng;
