/// Per-nuclide cross-section grids and interpolation.
///
/// C++ source: `src/nuclide.cpp` (1229 LOC), `include/openmc/nuclide.h`.
///
/// A `Nuclide` holds the energy grid and tabulated cross-section data loaded
/// from an HDF5 ACE-format library.  During transport, `xs_at_energy(E)` is
/// called for every collision to look up the microscopic cross sections
/// (elastic, fission, capture, total) at the current neutron energy.
///
/// Key data layout (matches OpenMC's internal representation):
///   - `energy_grid`: sorted energy points (eV)
///   - `xs_total`, `xs_elastic`, `xs_fission`, `xs_absorption`: aligned vectors

use ndarray::Array1;

/// Microscopic cross sections at a given energy (barn = 1e-24 cm²).
#[derive(Debug, Clone, Copy)]
pub struct MicroXS {
    pub total:      f64,
    pub elastic:    f64,
    pub fission:    f64,
    pub absorption: f64,
    /// Fission production multiplier ν (average neutrons per fission).
    pub nu_fission: f64,
}

/// A single nuclide's tabulated XS data.  Maps to `openmc::Nuclide`.
pub struct Nuclide {
    pub name: String,
    pub awr: f64,          // atomic weight ratio (mass / neutron mass)
    pub energy_grid: Array1<f64>,
    pub xs_total: Array1<f64>,
    pub xs_elastic: Array1<f64>,
    pub xs_fission: Array1<f64>,
    pub xs_absorption: Array1<f64>,
    pub nu_fission: Array1<f64>,
}

impl Nuclide {
    /// Interpolate all cross sections at energy `e` (eV).
    ///
    /// Uses log-log interpolation on the energy grid (OpenMC default).
    /// TODO: port from `nuclide.cpp::calculate_xs()`.
    pub fn xs_at_energy(&self, _e: f64) -> MicroXS {
        todo!("Nuclide::xs_at_energy: port log-log interpolation from nuclide.cpp")
    }
}
