//! Nuclear-data **provider** surface — what transport codes pull from this crate.
//!
//! Per the OUTRAM PARK architecture (`docs/architecture.md`), *all* nuclear-data
//! representation lives in `njoy-outram-park-fork`. Downstream transport crates
//! (`openmc-libs` for Monte Carlo, later a deterministic solver) do **not** own
//! cross sections — they call into this module for the microscopic cross sections
//! and secondary distributions a history needs.
//!
//! This module is the *thin, human-readable* boundary over the heavier machinery:
//! - [`crate::wmp`] — windowed-multipole cross sections + analytic Doppler.
//! - [`crate::ace`] — the ACE writer / (future) lean-ACE pointwise tables.
//! - [`secondary`] — ν̄(E) and χ(E), which WMP does not carry.
//!
//! # Dispatch is an enum, not a trait object
//!
//! Following the workspace rule (no `dyn`), a nuclide's cross-section *source* is
//! the [`XsProvider`] enum. A consumer holds one and calls [`XsProvider::micro`];
//! adding a new representation is a new variant that every `match` must handle.

pub mod secondary;

use crate::wmp::WindowedMultipole;
use secondary::{FissionSpectrum, NuBar};

/// Microscopic neutron cross sections at one energy/temperature \[barn\].
///
/// The common currency between the data crate and any transport kernel. All
/// channels are microscopic (per target atom); the transport code multiplies by
/// atom density to get macroscopic Σ \[cm⁻¹\].
#[derive(Debug, Clone, Copy, Default)]
pub struct MicroXs {
    /// Total σ_t \[barn\].
    pub total: f64,
    /// Elastic scattering σ_s \[barn\].
    pub elastic: f64,
    /// Fission σ_f \[barn\].
    pub fission: f64,
    /// Radiative capture σ_γ \[barn\].
    pub capture: f64,
    /// Fission production ν·σ_f \[barn\] (ν̄ folded in for the fission source).
    pub nu_fission: f64,
}

/// The cross-section *representation* backing one nuclide.
///
/// - `Multipole` — WMP σ + ν̄ + χ; the compact, in-crate, analytically-broadenable
///   path (the default OUTRAM PARK ships).
/// - `LeanAce` — a trimmed pointwise ACE table (single/few temperatures), for
///   nuclides or energy ranges WMP does not cover (e.g. a high-energy tail).
pub enum XsProvider {
    Multipole {
        wmp: WindowedMultipole,
        nu: NuBar,
        chi: FissionSpectrum,
    },
    LeanAce(LeanAce),
}

impl XsProvider {
    /// Microscopic cross sections at incident energy `e` \[eV\] and temperature
    /// `temp_k` \[K\]. Dispatches over the representation.
    pub fn micro(&self, e: f64, temp_k: f64) -> MicroXs {
        match self {
            XsProvider::Multipole { wmp, nu, .. } => {
                let x = wmp.evaluate(e, temp_k);
                MicroXs {
                    total: x.total(),
                    elastic: x.scatter,
                    fission: x.fission,
                    capture: x.capture(),
                    nu_fission: x.fission * nu.at(e),
                }
            }
            XsProvider::LeanAce(ace) => ace.micro(e, temp_k),
        }
    }
}

/// A trimmed pointwise ACE table (one or a few temperatures), embedded in-crate.
///
/// Scaffold. The lean-ACE path complements WMP where the multipole form does not
/// apply (above `e_max`, or nuclides without a WMP evaluation). Baked offline
/// from a full ACE library down to the curated nuclide/energy set — see
/// `docs/architecture.md`.
#[derive(Debug, Clone, Default)]
pub struct LeanAce {
    /// Nuclide name.
    pub name: String,
    /// Temperature of this pointwise set \[K\].
    pub temperature_k: f64,
    // TODO: energy grid + aligned σ columns (total/elastic/fission/capture) + ν̄.
}

impl LeanAce {
    /// Microscopic cross sections by log-log interpolation — **not yet ported**.
    pub fn micro(&self, _e: f64, _temp_k: f64) -> MicroXs {
        todo!("LeanAce::micro: log-log interpolation of the embedded pointwise table")
    }
}
