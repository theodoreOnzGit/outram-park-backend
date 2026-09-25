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
    /// **Delayed** fission production ν̄_d·Σ_f \[cm⁻¹\] (GitHub #262).
    ///
    /// Zero for a material whose nuclides carry no ENDF MF=1/455. That zero is
    /// a statement about the *data*, not a measurement of the delayed
    /// fraction: `Material::has_delayed_data` distinguishes the two, and a
    /// kinetics calculation that ignores the distinction would report
    /// β_eff = 0 for a system whose evaluations simply do not carry MT=455.
    pub nu_fission_delayed: f64,
    /// Precursor-decay-weighted delayed production `Σ_k λ_k ν̄_d,k Σ_f`
    /// \[cm⁻¹ s⁻¹\] — upstream's `SCORE_DECAY_RATE`
    /// (`src/tallies/tally_scoring.cpp:773`), summed over groups.
    pub decay_rate: f64,
    /// Absorption Σ_a \[cm⁻¹\] — **radiative capture + fission** (i.e. every
    /// reaction with no neutron in the exit channel, plus fission), aggregated
    /// from each nuclide's [`crate::material::nuclide::MicroXS::absorption`].
    ///
    /// This is the real absorption, **not** `Σ_t − Σ_elastic` — it excludes
    /// inelastic scatter, (n,2n) and (n,3n), which keep or multiply the neutron.
    /// Mirrors OpenMC's `Nuclide::create_derived` (`src/nuclide.cpp:409-417`):
    /// the energy-dependent sum of the non-redundant *disappearance* reactions
    /// (MT 101–117, 600–849, …) and fission.
    pub absorption: f64,
}

#[derive(Debug, Clone)]
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
            m.absorption += c.atom_density * x.absorption;
            // Delayed split (GitHub #262). `x.fission` is the microscopic
            // fission cross section, so the delayed production is
            // `sigma_f * nu_d(E)` and the decay-rate score weights each
            // group's share by its own lambda.
            let nuc = &nuclides[c.nuclide_idx];
            if x.fission > 0.0 {
                if let Some(d) = nuc.delayed() {
                    let nu_d = d.nu_delayed_at(e);
                    m.nu_fission_delayed += c.atom_density * x.fission * nu_d;
                    for k in 0..d.n_groups() {
                        m.decay_rate += c.atom_density
                            * x.fission
                            * nu_d
                            * d.group_fraction(k, e)
                            * d.lambda[k];
                    }
                }
            }
        }
        m
    }

    /// **Prompt** fission production ν̄_p·Σ_f \[cm⁻¹\] = `nu_fission −
    /// nu_fission_delayed`, floored at zero.
    ///
    /// See [`crate::material::nuclide::Nuclide::nu_prompt`] for why the floor
    /// is there and how large it has ever needed to be.
    pub fn nu_fission_prompt(xs: &MacroXs) -> f64 {
        (xs.nu_fission - xs.nu_fission_delayed).max(0.0)
    }

    /// Whether **every** fissionable nuclide in this material carries
    /// delayed-neutron data.
    ///
    /// A kinetics result from a material where this is `false` is missing part
    /// of its delayed production and the β it produces is a lower bound, not a
    /// measurement. Exposed so a caller can refuse rather than quietly report
    /// the smaller number.
    pub fn has_delayed_data(&self, nuclides: &[Nuclide]) -> bool {
        self.components.iter().all(|c| {
            let n = &nuclides[c.nuclide_idx];
            n.delayed().is_some() || !n.is_fissionable()
        })
    }

    /// Macroscopic total cross section Σ_t(E) \[cm⁻¹\] = Σ_i N_i·σ_t,i(E).
    pub fn macro_xs_total(&self, e: f64, nuclides: &[Nuclide]) -> f64 {
        self.components
            .iter()
            .map(|c| {
                // `total_at_energy`, not `xs_at_energy(..).total`: identical
                // value, but it skips ~45 grid evaluations per nuclide that
                // this sum discards. See `Nuclide::total_at_energy`.
                c.atom_density * nuclides[c.nuclide_idx].total_at_energy(e, self.temperature)
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
