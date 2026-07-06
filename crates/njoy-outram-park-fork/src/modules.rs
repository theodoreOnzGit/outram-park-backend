//! NJOY processing-module dispatch.
//!
//! Each upstream NJOY module (`RECONR`, `BROADR`, `ACER`, …) is an essentially
//! independent program that reads one or more ENDF tapes and writes another. In
//! this crate each module has **one home** at `src/<name>/`, holding its Rust
//! source (physics or a `NotPorted` stub) and a co-located `README.md`
//! documenting theory, implementation, testing status, and caveats.
//!
//! This file holds only the **dispatch** layer: the [`NjoyModule`] enum and its
//! `run` method. Per the workspace design rules, module selection is dispatched
//! through an enum, not a trait object — the set of modules is closed and known
//! at compile time, so a missing `match` arm is a compile error.
//!
//! **Status:** every module's card-input `run()` driver currently returns
//! [`crate::NjoyError::NotPorted`]; ported *physics* is reached directly through
//! the module's typed API (e.g. [`crate::reconr`], [`crate::broadr`],
//! [`crate::acer`], [`crate::heatr`], [`crate::gaspr`], [`crate::thermr`]). See
//! each module's `README.md` for the real per-piece status, and
//! `docs/porting-plan.md` for phasing.

/// The NJOY processing modules that can be selected on the card-input stream,
/// grouped by the order a typical processing run uses them.
///
/// Dispatch is by `match`, not `dyn` — adding a variant forces every call site
/// to handle it. Marked `#[non_exhaustive]` because variants are still being
/// added as modules are ported. (`samm`, the internal R-matrix kernel called by
/// RECONR/UNRESR, is not a user-selectable module and has no variant.)
#[non_exhaustive]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NjoyModule {
    /// Convert tapes between ASCII and NJOY binary; select/copy materials.
    Moder,
    /// Reconstruct pointwise cross sections from resonance parameters.
    Reconr,
    /// Doppler-broaden and thin pointwise cross sections.
    Broadr,
    /// Unresolved-range effective self-shielded cross sections (Bondarenko).
    Unresr,
    /// Pointwise heating (KERMA) and damage-energy cross sections.
    Heatr,
    /// Thermal-range scattering cross sections from S(α,β).
    Thermr,
    /// Gas-production cross sections (MT=203–207).
    Gaspr,
    /// Unresolved-resonance probability tables for Monte Carlo self-shielding.
    Purr,
    /// Prepare an ACE library for continuous-energy Monte Carlo (e.g. OpenMC).
    Acer,
    /// Multigroup neutron/photon cross sections and matrices.
    Groupr,
    /// Multigroup photoatomic (photon-interaction) data.
    Gaminr,
    /// Multigroup cross-section and distribution covariance matrices.
    Errorr,
    /// Post-process and plot ERRORR covariances.
    Covr,
    /// Generate the thermal scattering law S(α,β) (upstream of THERMR).
    Leapr,
    /// DTF-IV format libraries for discrete-ordinates (Sₙ) codes.
    Dtfr,
    /// Standard CCCC interface files (ISOTXS, BRKOXS, DLAYXS).
    Ccccr,
    /// MATXS generalized material cross-section files (for TRANSX).
    Matxsr,
    /// Pointwise resonance cross-section files for near-epithermal codes.
    Resxsr,
    /// Libraries for the EPRI-CELL / EPRI-CPM reactor codes.
    Powr,
    /// Libraries for the WIMS reactor-physics code.
    Wimsr,
    /// Linear combinations of cross sections onto a new PENDF tape.
    Mixr,
    /// Generate VIEWR plot input from ENDF/PENDF/GENDF data.
    Plotr,
    /// Render 2-D/3-D plots to PostScript.
    Viewr,
}

impl NjoyModule {
    /// Run this module's card-input driver.
    ///
    /// **Status:** every driver currently returns
    /// [`crate::NjoyError::NotPorted`]; ported *physics* is reached through the
    /// module's typed API (`crate::reconr`, `crate::broadr`, `crate::acer`,
    /// `crate::heatr`, `crate::gaspr`, `crate::thermr`). See each module's
    /// `README.md` for the real per-piece status.
    pub fn run(self) -> Result<(), crate::NjoyError> {
        match self {
            Self::Moder => crate::moder::run(),
            Self::Reconr => crate::reconr::run(),
            Self::Broadr => crate::broadr::run(),
            Self::Unresr => crate::unresr::run(),
            Self::Heatr => crate::heatr::run(),
            Self::Thermr => crate::thermr::run(),
            Self::Gaspr => crate::gaspr::run(),
            Self::Purr => crate::purr::run(),
            Self::Acer => crate::acer::run(),
            Self::Groupr => crate::groupr::run(),
            Self::Gaminr => crate::gaminr::run(),
            Self::Errorr => crate::errorr::run(),
            Self::Covr => crate::covr::run(),
            Self::Leapr => crate::leapr::run(),
            Self::Dtfr => crate::dtfr::run(),
            Self::Ccccr => crate::ccccr::run(),
            Self::Matxsr => crate::matxsr::run(),
            Self::Resxsr => crate::resxsr::run(),
            Self::Powr => crate::powr::run(),
            Self::Wimsr => crate::wimsr::run(),
            Self::Mixr => crate::mixr::run(),
            Self::Plotr => crate::plotr::run(),
            Self::Viewr => crate::viewr::run(),
        }
    }
}
