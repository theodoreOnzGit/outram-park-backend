/// Energy source distributions.
///
/// C++ source: `src/distribution_energy.cpp`, `include/openmc/distribution_energy.h`.

/// Trait for energy distributions (returns energy in eV).
pub trait EnergyDist: Send + Sync {
    fn sample(&self, seed: &mut u64) -> f64;
}

/// Monoenergetic source (all particles at the same energy).
pub struct Monoenergetic { pub e: f64 }
impl EnergyDist for Monoenergetic {
    fn sample(&self, _seed: &mut u64) -> f64 { self.e }
}

/// Maxwellian fission spectrum: f(E) ∝ √E · exp(−E / θ). θ in eV.
/// TODO: port Maxwell sampler from `random_dist.cpp`.
pub struct MaxwellSpectrum { pub theta: f64 }
impl EnergyDist for MaxwellSpectrum {
    fn sample(&self, seed: &mut u64) -> f64 {
        crate::rng::distributions::maxwell(seed, self.theta)
    }
}

/// Watt fission spectrum: f(E) ∝ exp(−E/a) · sinh(√(b·E)). a, b in eV.
/// TODO: port from `random_dist.cpp`.
pub struct WattSpectrum { pub a: f64, pub b: f64 }
impl EnergyDist for WattSpectrum {
    fn sample(&self, seed: &mut u64) -> f64 {
        crate::rng::distributions::watt(seed, self.a, self.b)
    }
}

/// Tabulated energy distribution (piecewise linear CDF).
/// TODO: port interpolation from `distribution_energy.cpp`.
pub struct TabulatedEnergy {
    pub energies: Vec<f64>,
    pub cdf: Vec<f64>,
}
impl EnergyDist for TabulatedEnergy {
    fn sample(&self, _seed: &mut u64) -> f64 {
        todo!("TabulatedEnergy::sample: port from distribution_energy.cpp")
    }
}
