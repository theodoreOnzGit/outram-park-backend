/// Energy source distributions.
///
/// C++ source: `src/distribution_energy.cpp`, `include/openmc/distribution_energy.h`.

/// Trait for energy distributions (returns energy in eV).
pub trait EnergyDist: Send + Sync {
    fn sample(&self, seed: &mut u64) -> f64;
}

/// Monoenergetic source (all particles at the same energy).
pub struct Monoenergetic {
    pub e: f64,
}
impl EnergyDist for Monoenergetic {
    fn sample(&self, _seed: &mut u64) -> f64 {
        self.e
    }
}

/// Maxwellian fission spectrum `f(E) ∝ √E · exp(−E / θ)`, `theta` in eV.
///
/// Sampling is [`crate::rng::distributions::maxwell`], the port of
/// `random_dist.cpp`'s `maxwell_spectrum`.
pub struct MaxwellSpectrum {
    pub theta: f64,
}
impl EnergyDist for MaxwellSpectrum {
    fn sample(&self, seed: &mut u64) -> f64 {
        crate::rng::distributions::maxwell(seed, self.theta)
    }
}

/// Watt fission spectrum `f(E) ∝ exp(−E/a) · sinh(√(b·E))`, `a` in eV and `b`
/// in eV⁻¹.
///
/// Sampling is [`crate::rng::distributions::watt`], the port of
/// `random_dist.cpp`'s `watt_spectrum`.
pub struct WattSpectrum {
    pub a: f64,
    pub b: f64,
}
impl EnergyDist for WattSpectrum {
    fn sample(&self, seed: &mut u64) -> f64 {
        crate::rng::distributions::watt(seed, self.a, self.b)
    }
}

/// Tabulated source energy distribution, sampled by inverting a piecewise-linear
/// CDF.
///
/// `energies` \[eV\] is ascending; `cdf` is aligned with it, non-decreasing,
/// starting at 0 and ending at 1. A value is drawn by finding the bin holding a
/// uniform `xi` and interpolating linearly across it — the `Tabular` /
/// `histogram`-free arm of OpenMC's `distribution_energy.cpp`.
///
/// # This used to be a live panic
///
/// Until 2026-09-16 `sample` was `todo!()`: a constructible source distribution
/// that aborted the run if anything ever drew from it. It was found by the
/// physics-coverage survey rather than by a test, because nothing in the crate
/// constructed one — which is exactly how a latent panic survives.
///
/// # Degenerate inputs
///
/// Returns the first energy for an empty or single-point table, and clamps
/// `xi` into `[0, 1]`, so no input shape can panic. A table whose CDF does not
/// reach 1 simply saturates at its last energy.
pub struct TabulatedEnergy {
    /// Ascending outgoing-energy grid \[eV\].
    pub energies: Vec<f64>,
    /// Cumulative probability aligned with [`Self::energies`].
    pub cdf: Vec<f64>,
}

impl EnergyDist for TabulatedEnergy {
    fn sample(&self, seed: &mut u64) -> f64 {
        let n = self.energies.len().min(self.cdf.len());
        if n == 0 {
            return 0.0;
        }
        if n == 1 {
            return self.energies[0];
        }
        let xi = crate::rng::lcg::prn(seed).clamp(0.0, 1.0);
        // First index whose cdf is >= xi; the sampled value lies in the bin
        // ending there.
        let hi = match self.cdf[..n].iter().position(|&c| c >= xi) {
            None => return self.energies[n - 1],
            Some(0) => 1,
            Some(i) => i,
        };
        let (c0, c1) = (self.cdf[hi - 1], self.cdf[hi]);
        let (e0, e1) = (self.energies[hi - 1], self.energies[hi]);
        if c1 > c0 {
            e0 + (e1 - e0) * (xi - c0) / (c1 - c0)
        } else {
            e0
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A uniform CDF over `[1, 3]` MeV must reproduce a uniform distribution:
    /// the sampled mean lands on the midpoint and nothing escapes the support.
    ///
    /// The regression guard for the `todo!()` this replaced — the previous
    /// implementation could not be tested at all, because calling it aborted.
    #[test]
    fn tabulated_energy_inverts_a_linear_cdf() {
        let d = TabulatedEnergy {
            energies: vec![1.0e6, 2.0e6, 3.0e6],
            cdf: vec![0.0, 0.5, 1.0],
        };
        let mut seed = 0x7AB1_E000u64;
        let n = 200_000;
        let mut sum = 0.0;
        for _ in 0..n {
            let e = d.sample(&mut seed);
            assert!(
                (1.0e6..=3.0e6).contains(&e),
                "sampled {e} outside the tabulated support [1e6, 3e6]"
            );
            sum += e;
        }
        let mean = sum / n as f64;
        // Uniform on [1, 3] MeV has mean 2 MeV; 0.5 % covers the MC error.
        assert!(
            (mean / 2.0e6 - 1.0).abs() < 5.0e-3,
            "mean sampled energy {mean:.6e} eV, expected 2.0e6 for a uniform CDF"
        );
    }

    /// Degenerate tables return a value rather than panicking — the property
    /// the `todo!()` version lacked.
    #[test]
    fn tabulated_energy_handles_degenerate_tables() {
        let mut seed = 1u64;
        assert_eq!(
            TabulatedEnergy { energies: vec![], cdf: vec![] }.sample(&mut seed),
            0.0
        );
        assert_eq!(
            TabulatedEnergy { energies: vec![5.0e5], cdf: vec![1.0] }.sample(&mut seed),
            5.0e5
        );
        // A CDF that never reaches 1 saturates at the last energy.
        let d = TabulatedEnergy {
            energies: vec![1.0e6, 2.0e6],
            cdf: vec![0.0, 0.25],
        };
        for _ in 0..1000 {
            let e = d.sample(&mut seed);
            assert!((1.0e6..=2.0e6).contains(&e), "{e}");
        }
    }
}
