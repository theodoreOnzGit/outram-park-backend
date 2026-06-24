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

/// A material — mixture of nuclides.  Maps to `openmc::Material`.
pub struct Material {
    pub id: i32,
    pub name: String,
    pub components: Vec<NuclideComponent>,
    /// Temperature in eV.
    pub temperature: f64,
}

impl Material {
    /// Macroscopic total cross section at energy `e` (eV) in cm⁻¹.
    ///
    /// Σ_t(E) = Σ_i  N_i · σ_t,i(E)
    ///
    /// TODO: implement once nuclide XS lookup is ported.
    pub fn macro_xs_total(&self, _e: f64, _nuclides: &[crate::material::nuclide::Nuclide]) -> f64 {
        todo!("Material::macro_xs_total: requires nuclide XS lookup")
    }

    /// Sample which nuclide the neutron interacts with, weighted by Σ_i.
    ///
    /// Returns the component index.
    /// TODO: implement with nuclide XS.
    pub fn sample_nuclide(&self, _e: f64, _rng: &mut u64, _nuclides: &[crate::material::nuclide::Nuclide]) -> usize {
        todo!("Material::sample_nuclide")
    }
}
