/// S(α,β) thermal scattering tables.
///
/// C++ source: `src/thermal.cpp`, `include/openmc/thermal.h`.
///
/// At low energies (E < ~4 eV), free-gas treatment of scattering from bound
/// atoms is inaccurate. OpenMC supports tabulated S(α,β) data (ENDF/B-VII+
/// thermal scattering law files) for materials like H in H₂O, graphite, etc.
///
/// TODO: port after the core XS lookup and reaction sampling framework is in place.

/// S(α,β) thermal scattering table.  Maps to `openmc::ThermalScattering`.
pub struct ThermalScattering {
    pub name: String,
    // TODO: energy grid, inelastic/elastic coherent & incoherent tables
}
