/// Probability distribution samplers — port of `src/random_dist.cpp`.
///
/// C++ source: `src/random_dist.cpp`, `include/openmc/random_dist.h`.
/// Also covers energy/angle distributions from
/// `src/distribution_energy.cpp`, `src/distribution_angle.cpp`.
use std::f64::consts::PI;

use super::lcg::prn;
use crate::mathf::RealMath;

/// The generic samplers — `uniform`, `sample_normal` (Box-Muller),
/// `sample_normal_3d` and `sample_exp` — **moved 2026-10-02** to
/// [`raffles::distributions::seeded`] (GitHub #500), byte for byte, and are
/// re-exported here so `outram_mc_libs::rng::distributions::*` call sites (this
/// crate, `boon-lay`, `nee_soon`) are unchanged. Their `cos` keeps this crate's
/// routing: this crate's `deterministic-math` feature forwards to RAFFLES'.
/// The physics samplers below stay here.
pub use raffles::distributions::seeded::{sample_exp, sample_normal, sample_normal_3d, uniform};

/// Sample a Maxwellian energy distribution: f(E) ∝ √E · exp(−E / θ).
/// `theta` is the temperature parameter in eV; the returned energy is in eV.
///
/// Maps to `double maxwell_spectrum(double T, uint64_t* seed)` in
/// `src/random_dist.cpp`. Uses the standard three-uniform algorithm: with
/// r₁,r₂,r₃ ∈ [0,1), `E = −θ·(ln r₁ + ln r₂ · cos²(½π r₃))`, which is exact for
/// the √E·exp(−E/θ) density (Everett & Cashwell).
pub fn maxwell(seed: &mut u64, theta: f64) -> f64 {
    // Clamp the logs' arguments away from 0 to avoid −∞ on the astronomically
    // rare prn() == 0.
    let r1 = prn(seed).max(f64::MIN_POSITIVE);
    let r2 = prn(seed).max(f64::MIN_POSITIVE);
    let r3 = prn(seed);
    let c = (0.5 * PI * r3).r_cos();
    -theta * (r1.r_ln() + r2.r_ln() * c * c)
}

/// Sample a Watt fission spectrum: f(E) ∝ exp(−E/a) · sinh(√(b·E)).
/// `a` in eV, `b` in eV⁻¹; the returned energy is in eV.
///
/// Maps to `double watt_spectrum(double a, double b, uint64_t* seed)`. Draws a
/// Maxwellian `w` with temperature `a`, then shifts it:
/// `E = w + ¼a²b + (2ξ−1)·√(a²b·w)` (Everett & Cashwell). This is the sampler
/// OpenMC uses for the prompt-fission source.
pub fn watt(seed: &mut u64, a: f64, b: f64) -> f64 {
    let w = maxwell(seed, a);
    w + 0.25 * a * a * b + (2.0 * prn(seed) - 1.0) * (a * a * b * w).sqrt()
}

/// Sample an isotropic direction on the unit sphere.
///
/// Returns direction cosines `(u, v, w)`. The polar cosine μ is uniform on
/// [−1, 1] and the azimuth φ uniform on [0, 2π): `(μ, √(1−μ²)·cos φ,
/// √(1−μ²)·sin φ)`. Maps to `Direction::sample_isotropic` / `isotropic()`.
pub fn isotropic_direction(seed: &mut u64) -> (f64, f64, f64) {
    let mu = 2.0 * prn(seed) - 1.0;
    let phi = 2.0 * PI * prn(seed);
    let a = (1.0 - mu * mu).max(0.0).sqrt();
    (mu, a * phi.r_cos(), a * phi.r_sin())
}

// The statistical tests of `uniform`, `sample_normal` and `sample_exp` moved
// with them to `raffles::distributions::seeded` (2026-10-02).
