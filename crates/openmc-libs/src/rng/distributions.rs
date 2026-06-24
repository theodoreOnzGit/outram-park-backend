/// Probability distribution samplers — port of `src/random_dist.cpp`.
///
/// C++ source: `src/random_dist.cpp`, `include/openmc/random_dist.h`.
/// Also covers energy/angle distributions from
/// `src/distribution_energy.cpp`, `src/distribution_angle.cpp`.

use std::f64::consts::PI;

use super::lcg::prn;

/// Sample a uniform deviate on `[low, high)`.
#[inline]
pub fn uniform(seed: &mut u64, low: f64, high: f64) -> f64 {
    low + (high - low) * prn(seed)
}

/// Sample a standard normal deviate N(0,1) via Box-Muller transform.
///
/// Uses two uniform draws: `u1 = prn(seed)`, `u2 = prn(seed)`.
/// Returns `√(−2 ln u1) · cos(2π u2)`.
///
/// Drop-in replacement for `rand_distr::StandardNormal` in boon-lay diffusion
/// modules.  For N(μ, σ²): `μ + σ * sample_normal(seed)`.
#[inline]
pub fn sample_normal(seed: &mut u64) -> f64 {
    // Guard against ln(0): with a 52-bit mantissa prn() == 0 is astronomically
    // rare but possible; clamp to the smallest positive f64.
    let u1 = prn(seed).max(f64::MIN_POSITIVE);
    let u2 = prn(seed);
    (-2.0 * u1.ln()).sqrt() * (2.0 * PI * u2).cos()
}

/// Sample a 3-D displacement from N(0, σ²) in each axis independently.
///
/// Returns `(dx, dy, dz)` with each component drawn from N(0, σ²).
/// Used by boon-lay Lagrangian diffusion to advance a particle one step.
#[inline]
pub fn sample_normal_3d(seed: &mut u64, sigma: f64) -> (f64, f64, f64) {
    (sigma * sample_normal(seed),
     sigma * sample_normal(seed),
     sigma * sample_normal(seed))
}

/// Sample from an exponential distribution with the given `rate` λ.
///
/// Uses inverse-CDF: `x = −ln(u) / λ`.  Mean of the distribution is `1/λ`.
///
/// Drop-in replacement for `rand_distr::Exp::new(rate).unwrap().sample(&mut rng)`
/// in boon-lay collision/scattering modules.
#[inline]
pub fn sample_exp(seed: &mut u64, rate: f64) -> f64 {
    -prn(seed).ln() / rate
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sample_normal_mean_and_variance() {
        let mut seed = 0xc0ffee_u64;
        let n = 100_000;
        let samples: Vec<f64> = (0..n).map(|_| sample_normal(&mut seed)).collect();
        let mean = samples.iter().sum::<f64>() / n as f64;
        let var = samples.iter().map(|x| (x - mean).powi(2)).sum::<f64>() / n as f64;
        assert!(mean.abs() < 0.02, "mean = {mean:.4}, expected ~0");
        assert!((var - 1.0).abs() < 0.02, "variance = {var:.4}, expected ~1");
    }

    #[test]
    fn sample_exp_mean() {
        let mut seed = 0xdeadbeef_u64;
        let rate = 2.5;
        let n = 100_000;
        let mean = (0..n).map(|_| sample_exp(&mut seed, rate)).sum::<f64>() / n as f64;
        let expected = 1.0 / rate;
        assert!((mean - expected).abs() / expected < 0.01,
            "mean = {mean:.4}, expected {expected:.4}");
    }

    #[test]
    fn uniform_stays_in_range() {
        let mut seed = 42_u64;
        for _ in 0..10_000 {
            let x = uniform(&mut seed, -5.0, 3.0);
            assert!(x >= -5.0 && x < 3.0);
        }
    }
}
