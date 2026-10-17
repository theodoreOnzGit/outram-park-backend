//! The object-style way in: [`NuclearDataLibrary`], a struct that holds an
//! evaluation and what has been made from it, with chained methods.
//!
//! **Functional core, OOP shell** (maintainer decision, 2026-10-17, GitHub
//! #925 / #933). All processing lives in the free functions of
//! [`crate::process`]. Every method here that processes is a thin
//! `#[inline]` call into one of them, and stores the result so the next call
//! can use it. Nothing is computed twice in two styles, so the two cannot
//! drift: `tests/nuclear_data_library_is_process.rs` pins that the object
//! gives the free functions' result **bit for bit**, with the same physics
//! defaults (PURR, HEATR and the 0 K elastic DBRC needs, all on).
//!
//! ## Quick start
//!
//! ```no_run
//! use njoy_outram_park_fork::interface::NuclearDataLibrary;
//! use njoy_outram_park_fork::units::{NeutronEnergy, Temperature};
//! use uom::si::{area::barn, energy::electronvolt, thermodynamic_temperature::kelvin};
//!
//! # fn demo() -> Result<(), njoy_outram_park_fork::NjoyError> {
//! // MAT, ZA and AWR are read from the tape.
//! let lib = NuclearDataLibrary::from_file("n-092_U_238.endf")?
//!     .reconstruct(0.001)?                                  // RECONR, 0 K
//!     .broaden(Temperature::new::<kelvin>(293.6))?;         // BROADR, PURR, HEATR
//!
//! let e = NeutronEnergy::new::<electronvolt>(6.67);
//! println!("σ_γ(6.67 eV) = {:.1} b", lib.capture_xs(e).get::<barn>());
//! let ace_text = lib.to_ace()?.to_type1_text()?;             // ACER
//! # let _ = ace_text; Ok(())
//! # }
//! ```
//!
//! The same three steps in the functional style are
//! `process::reconstruct`, `process::cross_sections_at` and
//! `NuclearCrossSections::to_ace` (or `process::ace` in one call).
//!
//! ## What the object adds
//!
//! - **State between calls.** The [`Reconstruction`] (RECONR at 0 K) is kept,
//!   so each [`NuclearDataLibrary::broaden`] starts again **from the 0 K
//!   reconstruction**, never from the last broadened table (#617): broadening
//!   to 600 K and then to 293.6 K gives the 293.6 K data, not a twice-broadened
//!   table. Several temperatures from one RECONR:
//!   [`NuclearDataLibrary::cross_sections_at_temperatures`].
//! - **Bytes in.** [`NuclearDataLibrary::from_bytes`] and
//!   [`NuclearDataLibrary::from_tape`] read no file, and no method that
//!   processes reads a file, starts a thread or reads a clock, so the object
//!   runs on `wasm32-unknown-unknown` as `process` does. The path methods
//!   ([`NuclearDataLibrary::from_file`], `write_*`) are native conveniences
//!   that wrap them.

use std::path::Path;
use std::sync::Arc;

use uom::si::{area::barn, energy::electronvolt, thermodynamic_temperature::kelvin};

use crate::{
    acer::AceTable,
    endf::{tape::Tape, MtReaction},
    process::{self, CrossSectionSettings, NuclearCrossSections, Reconstruction},
    purr::UrrProbabilityTables,
    reconr::{eval_lin_lin, ReconrResult},
    units::{CrossSection, NeutronEnergy, Temperature},
    NjoyError,
};

// ── Main struct ───────────────────────────────────────────────────────────────

/// One ENDF evaluation and the products [`crate::process`] has made from it.
///
/// ```text
/// NuclearDataLibrary::from_file(path)?     // or from_bytes / from_tape; MAT from the tape
///     .reconstruct(tolerance)?             // process::reconstruct      (RECONR, 0 K)
///     .broaden(temperature)?               // process::cross_sections_at (BROADR, PURR, HEATR)
/// ```
///
/// Query with [`elastic_xs`][Self::elastic_xs], [`capture_xs`][Self::capture_xs]
/// and the rest, read the products with
/// [`cross_sections`][Self::cross_sections], or export with
/// [`to_ace`][Self::to_ace] and [`to_pendf_tape`][Self::to_pendf_tape].
///
/// **Physics on by default**, the same defaults as
/// [`CrossSectionSettings::default`]; the `without_*` methods ablate, as the
/// settings builders do.
#[derive(Debug, Clone)]
pub struct NuclearDataLibrary {
    tape: Arc<Tape>,
    mat: i32,
    za: f64,
    awr: f64,
    settings: CrossSectionSettings,
    temperature: Temperature,
    reconstruction: Option<Reconstruction>,
    processed: Option<NuclearCrossSections>,
}

impl NuclearDataLibrary {
    // ── Constructors ──────────────────────────────────────────────────────────

    /// Load an ENDF-6 evaluation from `path` (plain or gzipped).
    ///
    /// **Native convenience:** reads the file, then [`Self::from_tape`]. In a
    /// browser use [`Self::from_bytes`].
    ///
    /// # Errors
    ///
    /// [`NjoyError::Io`] if the file cannot be read, [`NjoyError::EndfParse`]
    /// if the tape is malformed, and those of [`Self::from_tape`].
    pub fn from_file<P: AsRef<Path>>(path: P) -> Result<Self, NjoyError> {
        Self::from_tape(Tape::read_file(path.as_ref())?)
    }

    /// An evaluation from the bytes of an ENDF-6 text tape (an upload, an
    /// `include_bytes!`). Reads no file.
    ///
    /// # Errors
    ///
    /// [`NjoyError::EndfParse`] for a malformed tape, and those of
    /// [`Self::from_tape`].
    #[inline]
    pub fn from_bytes(bytes: &[u8]) -> Result<Self, NjoyError> {
        Self::from_tape(Tape::read(bytes)?)
    }

    /// An evaluation from a parsed [`Tape`]. The tape must hold one material;
    /// its MAT, and the ZA and AWR of its MF=1/MT=451 HEAD record, are read
    /// from it.
    ///
    /// # Errors
    ///
    /// [`NjoyError::InvalidInput`] for a tape with no material or several,
    /// and [`NjoyError::SectionNotFound`] without an MF=1/MT=451 section.
    pub fn from_tape(tape: Tape) -> Result<Self, NjoyError> {
        let mat = process::single_material(&tape)?;
        let head = tape
            .section(mat, 1, 451)
            .and_then(|s| s.rows.first().copied())
            .ok_or(NjoyError::SectionNotFound {
                mat,
                mf: 1,
                mt: 451,
            })?;
        Ok(NuclearDataLibrary {
            tape: Arc::new(tape),
            mat,
            za: head[0],
            awr: head[1],
            settings: CrossSectionSettings::default(),
            temperature: Temperature::new::<kelvin>(0.0),
            reconstruction: None,
            processed: None,
        })
    }

    // ── Settings ──────────────────────────────────────────────────────────────

    /// Process with `settings` from now on. Products made under other
    /// settings are dropped (the reconstruction too, if the RECONR tolerance
    /// changed), so nothing held can disagree with [`Self::settings`]; call
    /// [`Self::broaden`] again.
    pub fn with_settings(mut self, settings: CrossSectionSettings) -> Self {
        if settings.reconr_tolerance.to_bits() != self.settings.reconr_tolerance.to_bits() {
            self.reconstruction = None;
        }
        if settings != self.settings {
            self.processed = None;
        }
        self.settings = settings;
        self
    }

    /// Ablate PURR ([`CrossSectionSettings::without_purr`]).
    #[inline]
    pub fn without_purr(self) -> Self {
        let s = self.settings.clone().without_purr();
        self.with_settings(s)
    }

    /// Ablate HEATR ([`CrossSectionSettings::without_heatr`]).
    #[inline]
    pub fn without_heatr(self) -> Self {
        let s = self.settings.clone().without_heatr();
        self.with_settings(s)
    }

    /// Ablate the 0 K elastic ([`CrossSectionSettings::without_elastic_0k`]).
    #[inline]
    pub fn without_elastic_0k(self) -> Self {
        let s = self.settings.clone().without_elastic_0k();
        self.with_settings(s)
    }

    /// The settings the next processing step uses.
    #[inline]
    pub fn settings(&self) -> &CrossSectionSettings {
        &self.settings
    }

    // ── Pipeline steps ────────────────────────────────────────────────────────

    /// RECONR at 0 K with fractional `tolerance` (NJOY's default is `0.001`):
    /// [`process::reconstruct`]. Sets [`CrossSectionSettings::reconr_tolerance`],
    /// and drops any broadened product, so the temperature is 0 K until
    /// [`Self::broaden`].
    ///
    /// # Errors
    ///
    /// Those of [`process::reconstruct`] (a non-positive tolerance, RECONR's
    /// own refusals).
    #[inline]
    pub fn reconstruct(mut self, tolerance: f64) -> Result<Self, NjoyError> {
        self.settings.reconr_tolerance = tolerance;
        let recon = process::reconstruct(&self.tape, &self.settings)?;
        // Share the reconstruction's copy of the tape, so only one is held.
        self.tape = Arc::clone(&recon.tape);
        self.reconstruction = Some(recon);
        self.processed = None;
        self.temperature = Temperature::new::<kelvin>(0.0);
        Ok(self)
    }

    /// The cross sections at temperature `t`: [`process::cross_sections_at`]
    /// on the held 0 K reconstruction (BROADR above 0 K, PURR, HEATR, the 0 K
    /// elastic). Each call starts from the 0 K reconstruction, so a second
    /// `broaden` replaces the first rather than broadening it again (#617).
    ///
    /// # Errors
    ///
    /// [`NjoyError::InvalidInput`] before [`Self::reconstruct`], and those of
    /// [`process::cross_sections_at`].
    #[inline]
    pub fn broaden(mut self, t: Temperature) -> Result<Self, NjoyError> {
        let recon = self
            .reconstruction
            .as_ref()
            .ok_or_else(Self::not_reconstructed)?;
        self.processed = Some(process::cross_sections_at(recon, t, &self.settings)?);
        self.temperature = t;
        Ok(self)
    }

    /// The cross sections at each of `temperatures` from one RECONR, without
    /// changing what this object holds: [`process::cross_sections_at`] on the
    /// held reconstruction, or [`process::cross_sections_at_temperatures`]
    /// (which runs RECONR once) when there is none yet.
    ///
    /// # Errors
    ///
    /// Those of the `process` function called.
    #[inline]
    pub fn cross_sections_at_temperatures(
        &self,
        temperatures: &[Temperature],
    ) -> Result<Vec<NuclearCrossSections>, NjoyError> {
        match &self.reconstruction {
            Some(recon) => temperatures
                .iter()
                .map(|&t| process::cross_sections_at(recon, t, &self.settings))
                .collect(),
            None => {
                process::cross_sections_at_temperatures(&self.tape, temperatures, &self.settings)
            }
        }
    }

    // ── Products ──────────────────────────────────────────────────────────────

    /// RECONR's 0 K result, once [`Self::reconstruct`] has run.
    #[inline]
    pub fn reconstruction(&self) -> Option<&Reconstruction> {
        self.reconstruction.as_ref()
    }

    /// The processed cross sections, once [`Self::broaden`] has run.
    #[inline]
    pub fn cross_sections(&self) -> Option<&NuclearCrossSections> {
        self.processed.as_ref()
    }

    /// The processed cross sections, by value.
    #[inline]
    pub fn into_cross_sections(self) -> Option<NuclearCrossSections> {
        self.processed
    }

    /// The 0 K elastic DBRC needs ([`NuclearCrossSections::elastic_0k`]),
    /// once [`Self::broaden`] has run.
    #[inline]
    pub fn elastic_0k(&self) -> Option<&[(f64, f64)]> {
        self.processed.as_ref()?.elastic_0k.as_deref()
    }

    /// PURR's probability tables, once [`Self::broaden`] has run above 0 K
    /// for an evaluation with an unresolved range.
    #[inline]
    pub fn urr(&self) -> Option<&UrrProbabilityTables> {
        self.processed.as_ref()?.urr.as_ref()
    }

    /// The evaluation.
    #[inline]
    pub fn tape(&self) -> &Tape {
        &self.tape
    }

    // ── Cross-section queries ──────────────────────────────────────────────────

    /// Total cross section \[barns\] at incident neutron energy `e` (MT=1).
    ///
    /// Read from the processed (PENDF) sections after [`Self::broaden`], from
    /// RECONR's 0 K result after [`Self::reconstruct`] alone; zero before
    /// either, or outside the tabulated range.
    #[inline]
    pub fn total_xs(&self, e: NeutronEnergy) -> CrossSection {
        self.xs_for_reaction(MtReaction::Mt1Total, e)
    }

    /// Elastic scattering cross section \[barns\] at energy `e` (MT=2).
    #[inline]
    pub fn elastic_xs(&self, e: NeutronEnergy) -> CrossSection {
        self.xs_for_reaction(MtReaction::Mt2Elastic, e)
    }

    /// Fission cross section \[barns\] at energy `e` (MT=18); zero for a
    /// non-fissile material.
    #[inline]
    pub fn fission_xs(&self, e: NeutronEnergy) -> CrossSection {
        self.xs_for_reaction(MtReaction::Mt18Fission, e)
    }

    /// Radiative capture cross section \[barns\] at energy `e` (MT=102).
    #[inline]
    pub fn capture_xs(&self, e: NeutronEnergy) -> CrossSection {
        self.xs_for_reaction(MtReaction::Mt102Capture, e)
    }

    /// Cross section \[barns\] for a named ENDF reaction at energy `e`, by
    /// lin-lin interpolation.
    ///
    /// ```
    /// # use njoy_outram_park_fork::{interface::NuclearDataLibrary, MtReaction};
    /// # use uom::si::energy::electronvolt;
    /// # let lib = NuclearDataLibrary::from_file(
    /// #     "../../reference-data/endf/n-018_Ar_37-tendl2023.endf"
    /// # ).unwrap().reconstruct(0.001).unwrap();
    /// let e = uom::si::f64::Energy::new::<electronvolt>(1540.0);
    /// let xs = lib.xs_for_reaction(MtReaction::Mt2Elastic, e);
    /// ```
    pub fn xs_for_reaction(&self, mt: MtReaction, e: NeutronEnergy) -> CrossSection {
        let e_ev = e.get::<electronvolt>();
        let sigma = self
            .sections()
            .and_then(|r| r.sections.iter().find(|s| s.mt == mt))
            .map_or(0.0, |s| eval_lin_lin(&s.pairs, e_ev));
        CrossSection::new::<barn>(sigma)
    }

    /// The temperature of the held products: the last [`Self::broaden`], or
    /// 0 K.
    #[inline]
    pub fn temperature(&self) -> Temperature {
        self.temperature
    }

    /// Material number (MAT), read from the tape.
    #[inline]
    pub fn mat(&self) -> i32 {
        self.mat
    }

    /// ZA (Z×1000 + A), from the tape's MF=1/MT=451 HEAD record.
    #[inline]
    pub fn za(&self) -> f64 {
        self.za
    }

    /// AWR (target mass / neutron mass), from the tape's MF=1/MT=451 HEAD
    /// record.
    #[inline]
    pub fn awr(&self) -> f64 {
        self.awr
    }

    // ── Export ────────────────────────────────────────────────────────────────

    /// The held sections as `uom`-typed tables ([`ContinuousEnergyData`]):
    /// the processed ones after [`Self::broaden`], RECONR's 0 K ones after
    /// [`Self::reconstruct`] alone.
    ///
    /// # Errors
    ///
    /// [`NjoyError::InvalidInput`] before [`Self::reconstruct`].
    pub fn to_continuous_energy_data(&self) -> Result<ContinuousEnergyData, NjoyError> {
        let r = self.sections().ok_or_else(Self::not_reconstructed)?;
        let reactions = r
            .sections
            .iter()
            .map(|sec| ReactionCrossSection {
                mt: sec.mt,
                data: sec
                    .pairs
                    .iter()
                    .map(|&(e, sigma)| {
                        (
                            NeutronEnergy::new::<electronvolt>(e),
                            CrossSection::new::<barn>(sigma),
                        )
                    })
                    .collect(),
            })
            .collect();
        Ok(ContinuousEnergyData {
            za: r.material.za,
            awr: r.material.awr,
            temperature: self.temperature,
            reactions,
        })
    }

    /// The continuous-energy ACE table of the processed cross sections:
    /// [`NuclearCrossSections::to_ace`], the second half of [`process::ace`].
    ///
    /// ~~Builds the cross-section core of the ACE table~~ **CHANGED
    /// 2026-10-17 (GitHub #933):** until then this object assembled its own
    /// table through `AceTable::from_reconr_full` with no PURR, delayed,
    /// GPD or charged-particle blocks and no PENDF rounding. It is now
    /// `process`'s table, NJOY2016's word for word where `process::ace` is
    /// (`tests/process_api.rs`).
    ///
    /// # Errors
    ///
    /// [`NjoyError::InvalidInput`] before [`Self::broaden`], and ACER's.
    #[inline]
    pub fn to_ace(&self) -> Result<AceTable, NjoyError> {
        self.processed()?.to_ace()
    }

    /// Write [`Self::to_ace`] as a Type-1 (text) ACE file. Native
    /// convenience; in a browser use [`AceTable::to_type1_text`].
    ///
    /// # Errors
    ///
    /// Those of [`Self::to_ace`], or [`NjoyError::Io`].
    pub fn write_ace<P: AsRef<Path>>(&self, path: P) -> Result<(), NjoyError> {
        self.to_ace()?.write_type1(path).map_err(NjoyError::Io)
    }

    /// Write [`Self::to_ace`] as a Type-2 (Fortran unformatted, `itype = 2`)
    /// ACE file. Native convenience; in a browser use
    /// [`AceTable::to_type2_bytes`].
    ///
    /// # Errors
    ///
    /// As [`Self::write_ace`].
    pub fn write_ace_type2<P: AsRef<Path>>(&self, path: P) -> Result<(), NjoyError> {
        self.to_ace()?.write_type2(path)
    }

    /// The processed cross sections as a **PENDF tape**: MF=1/MT=451 (ZA,
    /// AWR, and the temperature in the record NJOY's GROUPR and ERRORR read
    /// it from) and one MF=3 section per reaction, built by
    /// [`Tape::pendf_from_pointwise`]. The values are already PENDF-rounded,
    /// so they survive [`Tape::write`] and [`Tape::read`] unchanged.
    ///
    /// **Scope, stated:** this is the pointwise content of NJOY's PENDF, not
    /// a byte copy of it. NJOY's RECONR/BROADR tape also carries the MF=1
    /// description text, MF=2 with `LRU = 0`, each MF=3 section's `QM` and
    /// `LR`, and the MF=12/13 photon sections; those are not written. HEATR's
    /// MT=301 and PURR's MT=152/153 are not written either; they are on
    /// [`NuclearCrossSections::heating`] and [`NuclearCrossSections::urr`].
    ///
    /// # Errors
    ///
    /// [`NjoyError::InvalidInput`] before [`Self::broaden`].
    pub fn to_pendf_tape(&self) -> Result<Tape, NjoyError> {
        let xs = self.processed()?;
        let iverf = self
            .tape
            .section(self.mat, 1, 451)
            .map_or(6, |s| crate::moder::layout::iverf_from_mf1(&s.rows));
        Ok(Tape::pendf_from_pointwise(
            self.mat,
            iverf,
            xs.za(),
            xs.awr(),
            xs.temperature.get::<kelvin>(),
            xs.pendf
                .sections
                .iter()
                .map(|s| (s.mt.number(), s.pairs.as_slice())),
        ))
    }

    /// Write [`Self::to_pendf_tape`] as ENDF text with [`Tape::write`]
    /// (MODER's formatting). Native convenience.
    ///
    /// ~~Returns [`NjoyError::NotPorted`] until the PENDF writer is
    /// implemented.~~ **CHANGED 2026-10-17 (GitHub #933):** writes the
    /// pointwise PENDF content described on [`Self::to_pendf_tape`].
    ///
    /// # Errors
    ///
    /// Those of [`Self::to_pendf_tape`], or [`NjoyError::Io`].
    pub fn write_pendf<P: AsRef<Path>>(&self, path: P) -> Result<(), NjoyError> {
        let mut text = Vec::new();
        self.to_pendf_tape()?.write(&mut text)?;
        std::fs::write(path, text).map_err(NjoyError::Io)
    }

    // ── Private helpers ───────────────────────────────────────────────────────

    /// The sections queries read: processed if there are any, else RECONR's.
    fn sections(&self) -> Option<&ReconrResult> {
        match (&self.processed, &self.reconstruction) {
            (Some(p), _) => Some(&p.pendf),
            (None, Some(r)) => Some(&r.reconr),
            (None, None) => None,
        }
    }

    fn processed(&self) -> Result<&NuclearCrossSections, NjoyError> {
        self.processed.as_ref().ok_or_else(|| {
            NjoyError::InvalidInput("call .reconstruct() and .broaden() first".into())
        })
    }

    fn not_reconstructed() -> NjoyError {
        NjoyError::InvalidInput("call .reconstruct() first".into())
    }
}

// ── ContinuousEnergyData ──────────────────────────────────────────────────────

/// In-memory continuous-energy nuclear data for one material.
///
/// Returned by [`NuclearDataLibrary::to_continuous_energy_data`]. Contains
/// the reconstructed (and, after `broaden`, Doppler-broadened) cross sections
/// as uom-typed (energy, σ) tables — the same data that would live in an ACE
/// file, but accessible without parsing text formats.
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
        let pairs: Vec<(f64, f64)> = self
            .data
            .iter()
            .map(|(en, xs)| (en.get::<electronvolt>(), xs.get::<barn>()))
            .collect();
        CrossSection::new::<barn>(eval_lin_lin(&pairs, e_ev))
    }
}
