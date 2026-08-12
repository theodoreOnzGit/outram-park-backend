/// Material composition and macroscopic cross-section lookup.
///
/// C++ source: `src/material.cpp` (1603 LOC), `include/openmc/material.h`.
///
/// A `Material` is a mixture of nuclides at specified atom/weight densities.
/// During transport, the material provides:
///   - Macroscopic total cross section Σ_t (sum of nuclide contributions)
///   - Nuclide sampling (select which nuclide the neutron collides with)
///   - Temperature for Doppler-broadened cross-section lookup

/// A nuclide component within a material.
#[derive(Debug, Clone)]
pub struct NuclideComponent {
    /// Index into the global nuclide array.
    pub nuclide_idx: usize,
    /// Atom density in atoms/barn·cm.
    pub atom_density: f64,
}

use crate::material::nuclide::Nuclide;
use crate::rng::lcg::prn;

/// Macroscopic cross sections of a material at one energy \[cm⁻¹\].
///
/// Each channel is Σ_x(E) = Σ_i N_i · σ_x,i(E), with N_i the atom density
/// \[atoms/barn·cm\] and σ in barn, so the product is in cm⁻¹.
#[derive(Debug, Clone, Copy, Default)]
pub struct MacroXs {
    /// Total Σ_t \[cm⁻¹\] — governs the distance-to-collision sample.
    pub total: f64,
    /// Elastic scattering Σ_s \[cm⁻¹\].
    pub elastic: f64,
    /// Fission Σ_f \[cm⁻¹\].
    pub fission: f64,
    /// Fission production ν̄·Σ_f \[cm⁻¹\] — the k-eigenvalue source term.
    pub nu_fission: f64,
}

/// A material — mixture of nuclides.  Maps to `openmc::Material`.
pub struct Material {
    pub id: i32,
    pub name: String,
    pub components: Vec<NuclideComponent>,
    /// Temperature in Kelvin (passed straight to the WMP Doppler evaluator).
    pub temperature: f64,
}

impl Material {
    /// Macroscopic cross sections at energy `e` \[eV\], summed over all nuclides.
    ///
    /// `nuclides` is the global nuclide array; each component indexes into it.
    /// Uses the material temperature for Doppler-broadened lookups.
    pub fn macro_xs(&self, e: f64, nuclides: &[Nuclide]) -> MacroXs {
        let mut m = MacroXs::default();
        for c in &self.components {
            let x = nuclides[c.nuclide_idx].xs_at_energy(e, self.temperature);
            m.total += c.atom_density * x.total;
            m.elastic += c.atom_density * x.elastic;
            m.fission += c.atom_density * x.fission;
            m.nu_fission += c.atom_density * x.nu_fission;
        }
        m
    }

    /// Macroscopic total cross section Σ_t(E) \[cm⁻¹\] = Σ_i N_i·σ_t,i(E).
    pub fn macro_xs_total(&self, e: f64, nuclides: &[Nuclide]) -> f64 {
        self.components
            .iter()
            .map(|c| {
                c.atom_density
                    * nuclides[c.nuclide_idx]
                        .xs_at_energy(e, self.temperature)
                        .total
            })
            .sum()
    }

    /// Sample which nuclide the neutron collides with, weighted by each
    /// nuclide's contribution N_i·σ_t,i(E) to the macroscopic total.
    ///
    /// Returns the **component index** (into `self.components`). Assumes at least
    /// one component with positive total XS; the RNG draw is clamped to the last
    /// component to stay in bounds against floating-point round-off.
    pub fn sample_nuclide(&self, e: f64, seed: &mut u64, nuclides: &[Nuclide]) -> usize {
        let sigma_t: f64 = self
            .components
            .iter()
            .map(|c| {
                c.atom_density
                    * nuclides[c.nuclide_idx]
                        .xs_at_energy(e, self.temperature)
                        .total
            })
            .sum();
        let xi = prn(seed) * sigma_t;
        let mut acc = 0.0;
        for (i, c) in self.components.iter().enumerate() {
            acc += c.atom_density
                * nuclides[c.nuclide_idx]
                    .xs_at_energy(e, self.temperature)
                    .total;
            if xi < acc {
                return i;
            }
        }
        self.components.len() - 1
    }
}
