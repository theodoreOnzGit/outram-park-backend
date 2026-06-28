//! RECONR — Reconstruct pointwise cross sections from resonance parameters.
//!
//! Ported from `reconr.f90` in NJOY2016 (~5 700 lines of Fortran 90).
//!
//! RECONR converts an ENDF evaluation into a PENDF tape: all cross sections in
//! MF=3 become fully pointwise (lin-lin TAB1), with resonance contributions
//! evaluated from MF=2 and added to the background. The result is a fine-grid
//! representation that supports accurate Doppler broadening in BROADR.
//!
//! ## Porting phases
//!
//! | Phase | Content | Status |
//! |-------|---------|--------|
//! | 2a | MF=1/MF=2 headers + linearisation + LRU=0 path (H-2) | **this version** |
//! | 2b | SLBW/MLBW resonance evaluation (U-235 LRU=1) | planned |
//! | 2c | Reich-Moore (LRF=3, SAMMY) + unresolved (LRU=2) | future |
//!
//! ## Entry point
//!
//! Call [`reconr`] with a parsed [`Tape`] and a [`ReconrConfig`]:
//!
//! ```no_run
//! use std::fs::File;
//! use njoy_outram_park_fork::{endf::tape::Tape, reconr::{reconr, ReconrConfig}};
//!
//! let tape = Tape::read(File::open("n-001_H_002.endf").unwrap()).unwrap();
//! let config = ReconrConfig { mat: 128, tolerance: 0.001, temperature: 0.0 };
//! let result = reconr(&tape, &config).unwrap();
//! println!("Material ZA = {}", result.material.za);
//! ```

pub mod linearize;
pub mod mf1;
pub mod mf2;
pub mod slbw;

pub use mf1::MaterialInfo;
pub use mf2::{EnergyRange, ResonanceFormalism, ResonanceInfo};

use crate::{
    endf::{records::SectionCursor, tape::Tape},
    NjoyError,
};

/// Configuration for one RECONR run on a single material.
#[derive(Debug, Clone)]
pub struct ReconrConfig {
    /// Material number (MAT) to process.
    pub mat: i32,
    /// Fractional reconstruction tolerance.
    ///
    /// Cross sections are linearised until the linear interpolation error is
    /// below this fraction of the local cross-section value.
    /// NJOY default: `0.001` (0.1 %).
    pub tolerance: f64,
    /// Reconstruction temperature [K].  `0.0` means 0 K (no Doppler).
    pub temperature: f64,
}

/// One reconstructed MF=3 cross-section section.
#[derive(Debug, Clone)]
pub struct ReconrSection {
    /// Section number (MT).
    pub mt: i32,
    /// Fully lin-lin (E [eV], σ [b]) grid.
    pub pairs: Vec<(f64, f64)>,
}

/// Result of running RECONR on one material.
#[derive(Debug, Clone)]
pub struct ReconrResult {
    /// Material header extracted from MF=1/MT=451.
    pub material: MaterialInfo,
    /// Linearised MF=3 cross sections, sorted by MT.
    pub sections: Vec<ReconrSection>,
}

/// Reconstruct pointwise cross sections for one material.
///
/// Reads the material identified by `config.mat` from `tape`, linearises every
/// MF=3 cross section to lin-lin within `config.tolerance`, and returns the
/// result. Resonance contributions are added in Phase 2b; for LRU=0 materials
/// (e.g. H-2) the MF=3 background is already the complete answer.
///
/// # Errors
///
/// - [`NjoyError::SectionNotFound`] if MF=1/MT=451 or MF=3 sections are absent.
/// - [`NjoyError::NotPorted`] if MF=2 contains LRU=1 or LRU=2 resonances
///   (these are Phase 2b/c work).
pub fn reconr(tape: &Tape, config: &ReconrConfig) -> Result<ReconrResult, NjoyError> {
    let mat = config.mat;
    let eps = config.tolerance;

    // MF=1/MT=451 — material header (ZA, AWR, LRP, LFI, EMAX, …)
    let mf1_sec = tape
        .section(mat, 1, 451)
        .ok_or(NjoyError::SectionNotFound { mat, mf: 1, mt: 451 })?;
    let material = mf1::parse_material_info(mf1_sec)?;

    // MF=2/MT=151 — resonance info (may be absent for charged-particle data)
    let _res_info = if let Some(mf2_sec) = tape.section(mat, 2, 151) {
        mf2::parse_resonance_info(mf2_sec)?
    } else {
        ResonanceInfo::default()
    };
    // Phase 2b: use _res_info to add resonance cross sections.

    // Process all MF=3 sections for this material.
    let mut sections: Vec<ReconrSection> = tape
        .sections()
        .iter()
        .filter(|s| s.key.mat == mat && s.key.mf == 3)
        .map(|sec| {
            let mut cur = SectionCursor::new(&sec.rows);
            // CONT record: ZA, AWR, QM, QI, 0, LR (reaction Q-values & type flag)
            let _cont = cur.read_cont()?;
            // TAB1: interpolation regions + (E, σ) pairs
            let tab1 = cur.read_tab1()?;

            // Linearise to lin-lin within tolerance.
            let pairs = linearize::linearize_tab1(&tab1.interp, &tab1.pairs, eps);

            Ok(ReconrSection { mt: sec.key.mt, pairs })
        })
        .collect::<Result<Vec<_>, NjoyError>>()?;

    sections.sort_by_key(|s| s.mt);

    Ok(ReconrResult { material, sections })
}
