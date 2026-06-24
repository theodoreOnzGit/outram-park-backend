/// Probability distribution samplers — port of `src/random_dist.cpp`.
///
/// C++ source: `src/random_dist.cpp`, `include/openmc/random_dist.h`.
/// Also covers energy/angle distributions from
/// `src/distribution_energy.cpp`, `src/distribution_angle.cpp`.

use super::lcg::prn;

/// Sample a uniform deviate on `[low, high)`.
#[inline]
pub fn uniform(seed: &mut u64, low: f64, high: f64) -> f64 {
    low + (high - low) * prn(seed)
}

/// Sample a Maxwellian energy distribution: f(E) ∝ √E · exp(−E / kT).
/// `theta` is the temperature parameter in eV.
///
/// Maps to `double maxwell_spectrum(double theta, uint64_t* seed)`.
/// TODO: port Box-Muller / Ahrens-Dieter algorithm from OpenMC.
pub fn maxwell(seed: &mut u64, theta: f64) -> f64 {
    let _ = (seed, theta);
    todo!("maxwell_spectrum: port from src/random_dist.cpp")
}

/// Sample a Watt fission spectrum: f(E) ∝ sinh(√(a·E)) · exp(−E/b).
/// Parameters `a` and `b` in eV.
///
/// Maps to `double watt_spectrum(double a, double b, uint64_t* seed)`.
/// TODO: port from OpenMC.
pub fn watt(seed: &mut u64, a: f64, b: f64) -> f64 {
    let _ = (seed, a, b);
    todo!("watt_spectrum: port from src/random_dist.cpp")
}

/// Sample an isotropic direction on the unit sphere.
///
/// Returns (u, v, w) direction cosines. Polar angle sampled uniformly in
/// cos θ ∈ [−1, 1]; azimuthal angle φ ∈ [0, 2π).
/// TODO: port from `distribution_angle.cpp`.
pub fn isotropic_direction(seed: &mut u64) -> (f64, f64, f64) {
    let _ = seed;
    todo!("isotropic direction: port from src/distribution_angle.cpp")
}
