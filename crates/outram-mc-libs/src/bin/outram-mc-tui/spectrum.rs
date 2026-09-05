//! Neutron spectrum tally + cross-section overlay data (op-iom).
//!
//! Builds a log-energy [`outram_mc_libs::tally::filter::EnergyFilter`] flux
//! tally (mirroring `crates/outram-mc-libs/tests/openmc_notebooks/flux_spectrum.rs`),
//! and samples the primary material's macroscopic total cross section
//! [`Material::macro_xs_total`] at the same bin midpoints, so the two series
//! share one log-E x-axis and can be drawn on the same
//! [`crate::ui::spectrum_screen`] chart. See [`crate::presets`] for which
//! presets can attach a tally at all (only the CSG-backed ones).

use outram_mc_libs::material::material::Material;
use outram_mc_libs::material::nuclide::Nuclide;
use outram_mc_libs::tally::filter::EnergyFilter;
use outram_mc_libs::tally::tally::{ScoreType, Tally, TallyBin};

/// Number of log-spaced energy bins the spectrum/XS overlay tallies and plots.
/// Matches the 50-bin grid `flux_spectrum.rs` uses for the same 1e-3 eV .. 20 MeV
/// span (thermal through the fast fission-source region).
pub const N_SPECTRUM_BINS: usize = 50;
const E_LO: f64 = 1.0e-3;
const E_HI: f64 = 2.0e7;

/// `n`-bin log-spaced energy grid from `e_lo` to `e_hi` \[eV\] (`n+1` ascending
/// edges) — the same construction `flux_spectrum.rs` uses.
fn log_energy_grid(e_lo: f64, e_hi: f64, n: usize) -> Vec<f64> {
    let (l0, l1) = (e_lo.ln(), e_hi.ln());
    (0..=n)
        .map(|i| (l0 + (l1 - l0) * i as f64 / n as f64).exp())
        .collect()
}

/// A fresh, empty spectrum tally over the standard log-energy grid, ready to
/// pass to `run_keff_csg`'s `tally: Option<&mut Tally>` parameter.
pub fn new_spectrum_tally() -> (Tally, Vec<f64>) {
    let edges = log_energy_grid(E_LO, E_HI, N_SPECTRUM_BINS);
    let filter = EnergyFilter {
        bins: edges.clone(),
    };
    let tally = Tally {
        id: 1,
        name: "spectrum overlay".into(),
        filters: vec![Box::new(filter)],
        scores: vec![ScoreType::Flux],
        bins: vec![TallyBin::default(); N_SPECTRUM_BINS],
    };
    (tally, edges)
}

/// The finished overlay: per-bin track-length flux mean alongside the primary
/// material's macroscopic total cross section at the same bin midpoint, both
/// on the shared `energy_edges` log-E grid.
#[derive(Debug, Clone)]
pub struct SpectrumOverlay {
    /// `N_SPECTRUM_BINS + 1` ascending energy bin edges \[eV\].
    pub energy_edges: Vec<f64>,
    /// Track-length flux mean per bin (arbitrary units — no volume/power
    /// normalization; see [`outram_mc_libs::tally::tally::TallyBin::mean`]).
    pub flux_mean: Vec<f64>,
    /// Macroscopic total cross section Sigma_t(E) \[cm^-1\] of the primary
    /// material, sampled at each bin's midpoint energy.
    pub xs_mid: Vec<f64>,
    /// Which material's Sigma_t is plotted (for the chart legend/caption).
    pub xs_material_name: String,
}

impl SpectrumOverlay {
    /// Finish an overlay from a filled-in tally plus the case's primary
    /// material/nuclide arrays. `n_active` is the run's active-generation
    /// count (the number of Monte Carlo realizations the tally accumulated
    /// over — see [`outram_mc_libs::tally::tally::TallyBin::mean`]).
    pub fn finish(
        tally: &Tally,
        edges: &[f64],
        n_active: u64,
        material: &Material,
        nuclides: &[Nuclide],
    ) -> Self {
        let flux_mean: Vec<f64> = tally.bins.iter().map(|b| b.mean(n_active)).collect();
        let xs_mid: Vec<f64> = edges
            .windows(2)
            .map(|w| {
                let e_mid = (w[0] * w[1]).sqrt(); // geometric mean — the natural midpoint on a log axis.
                material.macro_xs_total(e_mid, nuclides)
            })
            .collect();
        SpectrumOverlay {
            energy_edges: edges.to_vec(),
            flux_mean,
            xs_mid,
            xs_material_name: material.name.clone(),
        }
    }

    /// Bin-midpoint energies \[eV\] (geometric mean of each bin's edges) — the
    /// x-coordinate both series share.
    pub fn mid_energies(&self) -> Vec<f64> {
        self.energy_edges
            .windows(2)
            .map(|w| (w[0] * w[1]).sqrt())
            .collect()
    }
}
