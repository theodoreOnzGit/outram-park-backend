// SPDX-License-Identifier: AGPL-3.0-only
//
// A translation of OpenMC's windowed-multipole generator,
// `openmc/data/multipole.py` (`WindowedMultipole.from_endf`,
// `vectfit_nuclide`, `_vectfit_xs`, `_windowing`, `from_multipole`) and
// `openmc/data/vectfit.py`, at commit 8f79d9d02483c90e4134f03a9df4b75ce6b062bf
// (2026-11-10). OpenMC is MIT-licensed, Copyright (c) 2011-2025
// Massachusetts Institute of Technology, UChicago Argonne LLC, and OpenMC
// contributors; the permission notice is in `vectfit.rs` and in NOTICE.

//! **Generating** windowed-multipole data from an ENDF evaluation (gh:#1390).
//!
//! The rest of [`crate::wmp`] reads and evaluates WMP libraries; this module
//! makes one, the way OpenMC's `WindowedMultipole.from_endf` does:
//!
//! 1. [`pointwise_from_endf`]: this crate's RECONR -> GASPR -> ACER at 0 K
//!    (the deck `openmc.data.njoy.make_ace` writes), then OpenMC's ACE
//!    extraction: elastic, absorption (MT=27, by the `SUM_RULES`), fission,
//!    cut at the lower of the resolved-range top and the first threshold.
//! 2. [`vectfit_pointwise`]: Fast Relaxed Vector Fitting
//!    ([`vectfit::vectfit`]) of `sigma(E) E` as a rational function of
//!    `sqrt(E)`, searching the pole count per piece ([`vectfit_xs`]).
//! 3. [`search_windowing`] / [`windowing`]: equal-`sqrt(E)` windows, each the
//!    fewest consecutive poles plus a curve-fit background meeting
//!    `rtol = 1e-3`, `atol = 1e-5` b.
//!
//! [`from_endf`] chains the three with upstream's defaults.
//! [`WmpGenerated::to_windowed_multipole`] hands the result to the crate's
//! evaluator.
//!
//! # Agreement with upstream
//!
//! The pointwise front end and every control decision are ported line for
//! line. The numerics upstream delegates to LAPACK/OpenBLAS (blocked QR,
//! `DGELSD`, `DGEEV`, `ZGEMM`) and numpy's SIMD `pow` are re-expressed here
//! (an unblocked Householder QR, PETIR's Golub-Reinsch SVD and Francis QR),
//! so fitted poles agree to round-off amplified by the fit, not to the bit.
//! Measured: `docs/wmp-generation.md` and
//! `verification_and_validation/wmp_generate/`.
//!
//! Not ported: logging, the diagnostic figures (`path_out`), and pickle I/O.

mod fit;
mod front;
mod npy;
pub mod vectfit;
mod windowing;

#[cfg(test)]
mod tests;

use std::path::Path;

use crate::wmp::Cf64;
use crate::NjoyError;

pub use fit::{vectfit_pointwise, vectfit_xs, PieceFit, VectfitOptions};
pub use front::{pointwise_from_endf, zero_kelvin_ace};
pub use windowing::{search_windowing, windowing, SearchOptions, WindowingOptions, WmpGenerated};

/// Boltzmann constant \[eV/K\], OpenMC's `K_BOLTZMANN` (and this crate's).
pub(crate) const K_BOLTZMANN: f64 = super::K_BOLTZMANN;

/// Upper temperature the windows are padded for \[K\] (`TEMPERATURE_LIMIT`).
pub const TEMPERATURE_LIMIT: f64 = 3000.0;

/// The 0 K pointwise cross sections `vectfit_nuclide` fits (barn on an eV
/// grid), as its lines 448-472 assemble them.
#[derive(Debug, Clone)]
pub struct PointwiseXs {
    /// GNDS name.
    pub name: String,
    /// Atomic weight ratio (ACE header).
    pub awr: f64,
    /// Energy grid \[eV\].
    pub energy: Vec<f64>,
    /// MT=1 \[b\].
    pub total: Vec<f64>,
    /// MT=2 \[b\].
    pub elastic: Vec<f64>,
    /// MT=27 \[b\] (zeros if no absorption).
    pub absorption: Vec<f64>,
    /// MT=18 \[b\], for a fissionable nuclide.
    pub fission: Option<Vec<f64>>,
}

/// Multipole data (`mp_data`): poles and residues per energy piece.
#[derive(Debug, Clone)]
pub struct MultipoleData {
    /// GNDS name.
    pub name: String,
    /// Atomic weight ratio.
    pub awr: f64,
    /// Lowest energy \[eV\].
    pub e_min: f64,
    /// Highest energy \[eV\].
    pub e_max: f64,
    /// Fitted reactions, `[2, 27]` or `[2, 27, 18]`.
    pub mts: Vec<i32>,
    /// Poles per piece.
    pub poles: Vec<Vec<Cf64>>,
    /// Residues per piece, `[piece][reaction][pole]`.
    pub residues: Vec<Vec<Vec<Cf64>>>,
}

/// `vectfit_nuclide` (`multipole.py:387-538`) with upstream defaults
/// (`njoy_error = 5e-4`, automatic pieces).
pub fn vectfit_nuclide(endf_path: &Path, njoy_error: f64, opts: &VectfitOptions) -> Result<MultipoleData, NjoyError> {
    let xs = pointwise_from_endf(endf_path, njoy_error)?;
    vectfit_pointwise(&xs, None, opts)
}

/// `WindowedMultipole.from_endf` (`multipole.py:1005-1046`) with upstream's
/// defaults: `vectfit_nuclide(njoy_error = 5e-4)` then
/// `from_multipole(search = True)`.
///
/// # Errors
/// A deck that cannot run, an unsupported MF=2 layout, or a fit or
/// windowing that upstream would also abort on.
pub fn from_endf(endf_path: &Path) -> Result<WmpGenerated, NjoyError> {
    let mp = vectfit_nuclide(endf_path, 5e-4, &VectfitOptions::default())?;
    search_windowing(&mp, &SearchOptions::default())
}
