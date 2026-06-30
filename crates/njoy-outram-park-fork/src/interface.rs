//! User-friendly OOP API for the NJOY nuclear-data processing pipeline.
//!
//! The entry point is [`NuclearDataLibrary`], a builder-pattern struct that reads
//! an ENDF file, runs RECONR to reconstruct pointwise cross sections, and can
//! query or export the result.
//!
//! ## Quick start
//!
//! ```no_run
//! use njoy_outram_park_fork::interface::NuclearDataLibrary;
//! use uom::si::energy::electronvolt;
//! use uom::si::area::barn;
//!
//! // Load Ar-37 and reconstruct cross sections
//! let lib = NuclearDataLibrary::from_file("n-018_Ar_37-tendl2023.endf", 1828)
//!     .expect("load failed")
//!     .reconstruct(0.001)
//!     .expect("RECONR failed");
//!
//! // Query at 1540 eV (near the first Ar-37 resonance peak)
//! let e = uom::si::f64::Energy::new::<electronvolt>(1540.0);
//! let xs = lib.elastic_xs(e);
//! println!("σ_el(1540 eV) ≈ {:.1} b", xs.get::<barn>());
//! ```

use std::{fs::File, path::Path};

use uom::si::{
    area::barn,
    energy::electronvolt,
    thermodynamic_temperature::kelvin,
};

use crate::{
    broadr::doppler_broaden,
    endf::{tape::Tape, MtReaction},
    reconr::{eval_lin_lin, reconr, ReconrConfig, ReconrResult},
    units::{CrossSection, NeutronEnergy, Temperature},
    NjoyError,
};

// ── Main struct ───────────────────────────────────────────────────────────────

/// An ENDF nuclear-data evaluation processed through the NJOY pipeline.
///
/// Created with [`from_file`][Self::from_file] (or [`from_tape`][Self::from_tape]
/// for testing), then stepped through the pipeline with consuming methods:
///
/// ```text
/// NuclearDataLibrary::from_file(path, mat)?
///     .reconstruct(tolerance)?         // RECONR (pointwise XS)
///     .broaden(temperature)?           // BROADR (Doppler, Phase 3)
/// ```
///
/// Query the processed data with [`elastic_xs`][Self::elastic_xs],
/// [`fission_xs`][Self::fission_xs], etc., or export it with
/// [`to_continuous_energy_data`][Self::to_continuous_energy_data].
#[derive(Debug)]
pub struct NuclearDataLibrary {
    tape: Tape,
    mat:  i32,
    temperature: Temperature,
    reconr: Option<ReconrResult>,
}

impl NuclearDataLibrary {
    // ── Constructors ──────────────────────────────────────────────────────────

    /// Load an ENDF-6 file from `path` for material `mat`.
    ///
    /// `mat` is the ENDF material number (e.g. 1828 for Ar-37, 9228 for U-235,
    /// 128 for H-2). The file must contain an MF=1/MT=451 header for `mat`.
    ///
    /// # Errors
    ///
    /// Returns [`NjoyError::Io`] if the file cannot be opened, or
    /// [`NjoyError::EndfParse`] if the ENDF tape format is invalid.
    pub fn from_file<P: AsRef<Path>>(path: P, mat: i32) -> Result<Self, NjoyError> {
        let f = File::open(path).map_err(NjoyError::Io)?;
        let tape = Tape::read(f)?;
        Ok(Self::from_tape(tape, mat))
    }

    /// Build a [`NuclearDataLibrary`] from a pre-parsed [`Tape`].
    ///
    /// Used in tests to avoid re-parsing the file for every step.
    pub fn from_tape(tape: Tape, mat: i32) -> Self {
        NuclearDataLibrary {
            tape,
            mat,
            temperature: Temperature::new::<kelvin>(0.0),
            reconr: None,
        }
    }

    // ── Pipeline steps ────────────────────────────────────────────────────────

    /// Run RECONR to reconstruct fully pointwise (lin-lin) cross sections.
    ///
    /// `tolerance` is the fractional reconstruction accuracy; NJOY default is
    /// `0.001` (0.1%). Resonance contributions from MF=2 (SLBW/MLBW) are added
    /// to the MF=3 background for materials with LRU=1 evaluated resonances.
    ///
    /// Consumes and returns `self` so pipeline steps can be chained.
    ///
    /// # Errors
    ///
    /// Returns [`NjoyError::NotPorted`] for resonance formalisms not yet ported
    /// (LRF ≥ 3: Reich-Moore, R-Matrix Limited).
    pub fn reconstruct(mut self, tolerance: f64) -> Result<Self, NjoyError> {
        let config = ReconrConfig { mat: self.mat, tolerance, temperature: 0.0 };
        self.reconr = Some(reconr(&self.tape, &config)?);
        Ok(self)
    }

    /// Apply Doppler broadening at temperature `t` using the SIGMA1 free-gas method.
    ///
    /// Requires [`reconstruct`][Self::reconstruct] to have been called first.
    /// Broadens all reconstructed cross sections using the effective target
    /// temperature `t` \[K\].
    ///
    /// # Errors
    ///
    /// Returns [`NjoyError::NotPorted`] if RECONR has not been run first.
    pub fn broaden(mut self, t: Temperature) -> Result<Self, NjoyError> {
        let awr = self.awr().ok_or(NjoyError::NotPorted(
            "call .reconstruct() before .broaden()",
        ))?;
        let temp_k = t.get::<kelvin>();
        self.temperature = t;

        if temp_k > 0.0 {
            if let Some(r) = &mut self.reconr {
                r.sections = doppler_broaden(&r.sections, awr, temp_k);
            }
        }
        Ok(self)
    }

    // ── Cross-section queries ──────────────────────────────────────────────────

    /// Total cross section \[barns\] at incident neutron energy `e` (MT=1).
    ///
    /// Returns zero if RECONR has not been run or the energy is out of range.
    pub fn total_xs(&self, e: NeutronEnergy) -> CrossSection {
        CrossSection::new::<barn>(self.eval(MtReaction::Mt1Total, e.get::<electronvolt>()))
    }

    /// Elastic scattering cross section \[barns\] at energy `e` (MT=2).
    pub fn elastic_xs(&self, e: NeutronEnergy) -> CrossSection {
        CrossSection::new::<barn>(self.eval(MtReaction::Mt2Elastic, e.get::<electronvolt>()))
    }

    /// Fission cross section \[barns\] at energy `e` (MT=18).
    ///
    /// Returns zero for non-fissile materials.
    pub fn fission_xs(&self, e: NeutronEnergy) -> CrossSection {
        CrossSection::new::<barn>(self.eval(MtReaction::Mt18Fission, e.get::<electronvolt>()))
    }

    /// Radiative capture cross section \[barns\] at energy `e` (MT=102).
    pub fn capture_xs(&self, e: NeutronEnergy) -> CrossSection {
        CrossSection::new::<barn>(self.eval(MtReaction::Mt102Capture, e.get::<electronvolt>()))
    }

    /// Cross section \[barns\] for a named ENDF reaction at energy `e`.
    ///
    /// ```
    /// # use njoy_outram_park_fork::{interface::NuclearDataLibrary, MtReaction};
    /// # use uom::si::energy::electronvolt;
    /// # let lib = NuclearDataLibrary::from_file(
    /// #     "tests/resources/n-018_Ar_37-tendl2023.endf", 1828
    /// # ).unwrap().reconstruct(0.001).unwrap();
    /// let e = uom::si::f64::Energy::new::<electronvolt>(1540.0);
    /// let xs = lib.xs_for_reaction(MtReaction::Mt2Elastic, e);
    /// ```
    pub fn xs_for_reaction(&self, mt: MtReaction, e: NeutronEnergy) -> CrossSection {
        CrossSection::new::<barn>(self.eval(mt, e.get::<electronvolt>()))
    }

    /// Processing temperature \[K\] set by [`broaden`][Self::broaden].
    pub fn temperature(&self) -> Temperature {
        self.temperature
    }

    /// Material number (MAT).
    pub fn mat(&self) -> i32 {
        self.mat
    }

    /// Return ZA (Z×1000 + A) of this material, or `None` if RECONR has not run.
    pub fn za(&self) -> Option<f64> {
        self.reconr.as_ref().map(|r| r.material.za)
    }

    /// Return AWR (atomic weight ratio) of this material, or `None` if RECONR has not run.
    pub fn awr(&self) -> Option<f64> {
        self.reconr.as_ref().map(|r| r.material.awr)
    }

    // ── Export ────────────────────────────────────────────────────────────────

    /// Convert to an in-memory [`ContinuousEnergyData`] struct for use with OpenMC.
    ///
    /// The returned struct owns the (energy, σ) tables as uom-typed values and can
    /// be passed directly to a future OpenMC Rust binding without parsing ACE files.
    ///
    /// # Errors
    ///
    /// Returns [`NjoyError::NotPorted`] if RECONR has not been run first.
    pub fn to_continuous_energy_data(&self) -> Result<ContinuousEnergyData, NjoyError> {
        let r = self.reconr.as_ref().ok_or(NjoyError::NotPorted(
            "call .reconstruct() before .to_continuous_energy_data()",
        ))?;

        let reactions = r.sections.iter().map(|sec| {
            let data = sec.pairs.iter().map(|&(e, sigma)| {
                (
                    NeutronEnergy::new::<electronvolt>(e),
                    CrossSection::new::<barn>(sigma),
                )
            }).collect();
            ReactionCrossSection { mt: sec.mt, data }
        }).collect();

        Ok(ContinuousEnergyData {
            za:          r.material.za,
            awr:         r.material.awr,
            temperature: self.temperature,
            reactions,
        })
    }

    /// Write a PENDF-style text file (stub — Phase 2b milestone output).
    ///
    /// Returns [`NjoyError::NotPorted`] until the PENDF writer is implemented.
    pub fn write_pendf<P: AsRef<Path>>(&self, _path: P) -> Result<(), NjoyError> {
        Err(NjoyError::NotPorted("PENDF writer (future work)"))
    }

    /// Write a continuous-energy **ACE** file (Type-1 ASCII) for this material.
    ///
    /// Builds the cross-section core of the ACE table — the ESZ block (energy
    /// grid, total, disappearance, elastic, heating) and the MTR/LQR/TYR/LSIG/SIG
    /// reaction blocks — from the reconstructed (and optionally broadened) cross
    /// sections, then serialises it. The ZAID suffix and class letter follow the
    /// MCNP convention (e.g. `92235.00c`).
    ///
    /// When MF=4/MT=2 is present, the **elastic** angular distribution is written
    /// to the LAND/AND blocks. The remaining secondary-particle blocks (fission
    /// ν̄, non-elastic angular and all energy distributions) and the heating
    /// (KERMA) column are not yet written — see [`crate::ace`] for the scope. The
    /// file is a valid cross-section + elastic-scattering ACE table but not yet a
    /// complete transport library.
    ///
    /// # Errors
    ///
    /// Returns [`NjoyError::NotPorted`] if RECONR has not been run first,
    /// [`NjoyError::EndfParse`] if the MF=4 section is malformed, or
    /// [`NjoyError::Io`] if the file cannot be written.
    pub fn write_ace<P: AsRef<Path>>(&self, path: P) -> Result<(), NjoyError> {
        let r = self.reconr.as_ref().ok_or(NjoyError::NotPorted(
            "call .reconstruct() before .write_ace()",
        ))?;
        // tz = k_B·T expressed in MeV (NJOY's `tz`).
        let kt_mev = crate::common::phys::BK_EV_PER_K * self.temperature.get::<kelvin>() / 1.0e6;
        let ace = match self.tape.section(self.mat, 4, 2) {
            Some(sec) => {
                let ang = crate::ace::angular::parse_elastic_angular(sec)?;
                crate::ace::AceTable::from_reconr_with_angular(r, kt_mev, 0, &ang)
            }
            None => crate::ace::AceTable::from_reconr(r, kt_mev, 0),
        };
        ace.write_type1(path).map_err(NjoyError::Io)
    }

    // ── Private helpers ───────────────────────────────────────────────────────

    fn eval(&self, mt: MtReaction, e_ev: f64) -> f64 {
        match &self.reconr {
            None => 0.0,
            Some(r) => {
                let sec = r.sections.iter().find(|s| s.mt == mt);
                sec.map_or(0.0, |s| eval_lin_lin(&s.pairs, e_ev))
            }
        }
    }
}

// ── ContinuousEnergyData ──────────────────────────────────────────────────────

/// In-memory continuous-energy nuclear data for one material.
///
/// Returned by [`NuclearDataLibrary::to_continuous_energy_data`]. Contains
/// the fully reconstructed, Doppler-broadened cross sections as uom-typed
/// (energy, σ) tables — the same data that would live in an ACE file, but
/// accessible without parsing text formats.
///
/// Designed for direct consumption by a future OpenMC Rust binding in this
/// workspace (`openmc-libs`), where the material data is loaded once and queried
/// many times during transport.
#[derive(Debug, Clone)]
pub struct ContinuousEnergyData {
    /// Z×1000 + A (e.g. 1828 for Ar-37, 9228 for U-235).
    pub za: f64,
    /// Atomic weight ratio: target mass / neutron mass.
    pub awr: f64,
    /// Temperature at which broadening was applied \[K\].
    pub temperature: Temperature,
    /// Cross sections for each available reaction, ordered by MT number.
    pub reactions: Vec<ReactionCrossSection>,
}

impl ContinuousEnergyData {
    /// Find the cross section table for ENDF reaction `mt`.
    ///
    /// Returns `None` if the reaction is absent (e.g. MT=18 for a non-fissile nuclide).
    pub fn reaction(&self, mt: MtReaction) -> Option<&ReactionCrossSection> {
        self.reactions.iter().find(|r| r.mt == mt)
    }
}

/// Cross sections for one ENDF reaction, as a fully lin-lin (energy, σ) table.
#[derive(Debug, Clone)]
pub struct ReactionCrossSection {
    /// Reaction type. Use [`MtReaction::try_from`] or [`MtReaction::from_any`]
    /// to convert from a raw integer if you are working with the [`i32`] level.
    pub mt: MtReaction,
    /// Pointwise (energy \[J\], σ \[m²\]) data, sorted by energy.
    ///
    /// Use `energy.get::<electronvolt>()` and `sigma.get::<barn>()` to recover
    /// the eV / barn values used in nuclear data processing.
    pub data: Vec<(NeutronEnergy, CrossSection)>,
}

impl ReactionCrossSection {
    /// Evaluate σ at energy `e` by linear interpolation \[barns\].
    ///
    /// Returns the cross section at the boundary if `e` is outside the
    /// tabulated range.
    pub fn at(&self, e: NeutronEnergy) -> CrossSection {
        let e_ev = e.get::<electronvolt>();
        let pairs: Vec<(f64, f64)> = self.data.iter()
            .map(|(en, xs)| (en.get::<electronvolt>(), xs.get::<barn>()))
            .collect();
        CrossSection::new::<barn>(eval_lin_lin(&pairs, e_ev))
    }
}
