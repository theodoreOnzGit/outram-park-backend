//! **The high-level way in** (GitHub #871): four functions that take an ENDF
//! tape and a temperature to processed nuclear data, the way an NJOY deck does.
//!
//! | function | inputs | output | NJOY modules |
//! |---|---|---|---|
//! | [`cross_sections`] | incident ENDF tape, `T` | [`NuclearCrossSections`] | RECONR → BROADR → PURR → HEATR |
//! | [`ace`] | incident ENDF tape, `T` | [`AceTable`] (class `c`) | the above, then ACER |
//! | [`thermal_scattering`] | incident ENDF tape, LEAPR deck, `T` | [`ThermalScatteringLaw`] | LEAPR; RECONR → BROADR; THERMR |
//! | [`thermal_ace`] | incident ENDF tape, LEAPR deck, `T` | [`AceTable`] (class `t`) | the above, then ACER |
//!
//! ```no_run
//! use njoy_outram_park_fork::endf::tape::Tape;
//! use njoy_outram_park_fork::process::{self, CrossSectionSettings, ThermalSettings};
//! use njoy_outram_park_fork::units::Temperature;
//! use uom::si::thermodynamic_temperature::kelvin;
//!
//! # fn demo() -> Result<(), njoy_outram_park_fork::NjoyError> {
//! let h1 = Tape::read(std::fs::File::open("n-001_H_001.endf")?)?;
//! let t = Temperature::new::<kelvin>(293.6);
//!
//! // (a) and (b): H-1's cross sections, and its continuous-energy ACE file.
//! let xs = process::cross_sections(&h1, t, &CrossSectionSettings::default())?;
//! let ace = xs.to_ace()?;
//! let ace_text: String = ace.to_type1_text()?;
//!
//! // (c) and (d): H in H2O from its LEAPR deck, and its thermal ACE file.
//! let deck = std::fs::read_to_string("tsl-HinH2O.leapr")?;
//! let settings = ThermalSettings::default().with_table_name("hh2o");
//! let law = process::thermal_scattering(&h1, &deck, t, &settings)?;
//! let thermal_text: String = law.to_ace()?.to_type1_text()?;
//! # Ok(())
//! # }
//! ```
//!
//! # What is in and what is not
//!
//! - **Bytes in, values out.** Nothing here reads a file, starts a thread or
//!   reads a clock, so all four run on `wasm32-unknown-unknown` (the browser
//!   demo calls them). Read the tape with [`Tape::read`] on any reader.
//! - **Physics on by default** (root `CLAUDE.md`): PURR's probability tables
//!   and HEATR's heating are in the default [`CrossSectionSettings`]; turning
//!   either off is an explicit `without_*` call.
//! - **The thermal scattering law always comes from LEAPR** (GitHub #870):
//!   [`thermal_scattering`] takes a LEAPR deck, never an evaluated
//!   `tsl-*.endf` tape. Those tapes are oracles for the tests only.
//! - **The settings are NJOY's defaults**: RECONR and BROADR at 0.001, PURR
//!   with 20 bands, 64 ladders and 10 000 samples (NJOY2016's reference decks
//!   and this crate's word-for-word gate), THERMR at 0.001 with 16 cosines,
//!   `natom` and `emax` from the law's own `B(6)` and `B(4)` (as OpenMC's
//!   `make_ace_thermal` takes them), ACER with 16 equally probable outgoing
//!   energies (`iwt = 1`).
//!
//! # Verification
//!
//! Each result is the input of a stage that is already gated word for word
//! against NJOY2016, and the four functions are gated again as wholes
//! (`tests/process_api.rs`):
//!
//! - [`ace`] reproduces NJOY2016's U-234 table at 293.6 K
//!   (`RECONR+BROADR+PURR+ACER`, no HEATR, as the reference was made) in
//!   every word.
//! - [`thermal_ace`] on the ENDF/B-VIII.0 H-in-H₂O LEAPR deck cut to 293.6 K
//!   and H-1 reproduces NJOY2016 run on the same deck
//!   (`leapr → reconr → broadr → thermr → acer`) in every word.
//!
//! The results, with dates, are in that file's module doc.

mod settings;
mod thermal;

pub use settings::{CrossSectionSettings, PurrSettings, ThermalSettings};
pub use thermal::{thermal_ace, thermal_scattering, ThermalScatteringLaw};

use std::sync::Arc;

use uom::si::thermodynamic_temperature::kelvin;

use crate::acer::{AceDeck, AceTable};
use crate::endf::tape::Tape;
use crate::endf::MtReaction;
use crate::heatr::Kerma;
use crate::purr::UrrProbabilityTables;
use crate::reconr::{reconr, ReconrConfig, ReconrResult, ReconrSection};
use crate::units::Temperature;
use crate::NjoyError;

/// Boltzmann's constant \[MeV/K\], NJOY2016's (`phys.f90`, CODATA 2018).
const K_B_MEV: f64 = 8.617_333_262e-11;

/// An incident nuclide's processed cross sections at one temperature: what
/// NJOY's `reconr → broadr → purr → heatr` leaves on its PENDF, and what
/// ACER reads. Made by [`cross_sections`].
///
/// Every energy is in eV and every cross section in barns, as on the PENDF.
#[derive(Debug, Clone)]
pub struct NuclearCrossSections {
    /// The evaluation the cross sections were made from. ACER reads its
    /// angular, energy, ν̄, delayed-neutron and photon data directly.
    pub tape: Arc<Tape>,
    /// The ENDF material number (MAT) of the evaluation.
    pub mat: i32,
    /// The temperature the cross sections are Doppler-broadened to.
    pub temperature: Temperature,
    /// The pointwise cross sections, one section per reaction (MT), on
    /// RECONR's grid and broadened by BROADR, as written to the PENDF tape:
    /// every energy and cross section through ENDF's 11-column text field.
    pub pendf: ReconrResult,
    /// PURR's unresolved-resonance probability tables. `None` when PURR was
    /// switched off, at 0 K, or when the evaluation has no unresolved range.
    pub urr: Option<UrrProbabilityTables>,
    /// HEATR's heating (KERMA, MT=301, with photons transported). `None` only
    /// when HEATR was switched off.
    pub heating: Option<Kerma>,
    /// The settings these were made with.
    pub settings: CrossSectionSettings,
}

impl NuclearCrossSections {
    /// The atomic weight ratio (target mass / neutron mass) from MF=1/MT=451.
    pub fn awr(&self) -> f64 {
        self.pendf.material.awr
    }

    /// `1000 Z + A` of the target.
    pub fn za(&self) -> f64 {
        self.pendf.material.za
    }

    /// The pointwise `(E [eV], σ [b])` table of one reaction, or `None` when
    /// the evaluation does not have it.
    pub fn reaction(&self, mt: MtReaction) -> Option<&ReconrSection> {
        self.pendf.sections.iter().find(|s| s.mt == mt)
    }

    /// ACER on these cross sections: the continuous-energy (class `c`) ACE
    /// table, with the ZAID suffix [`CrossSectionSettings::ace_suffix`]. The
    /// second half of [`ace`].
    ///
    /// # Errors
    /// Those of ACER's block builders (a malformed MF=4/5/6 section).
    pub fn to_ace(&self) -> Result<AceTable, NjoyError> {
        let t_k = self.temperature.get::<kelvin>();
        let modules = crate::acer::DeckModules { urr: self.urr.clone(), kerma: self.heating.clone() };
        crate::acer::assemble(&self.tape, self.mat, &self.pendf, K_B_MEV * t_k, self.settings.ace_suffix, &modules)
    }

    /// What THERMR reads from this PENDF: the MAT, AWR, ZA and the elastic
    /// (MT=2) cross section.
    ///
    /// # Errors
    /// [`NjoyError::SectionNotFound`] without an elastic section.
    pub fn incident_pendf(&self) -> Result<crate::acer::thermal::IncidentPendf, NjoyError> {
        let elastic = self
            .reaction(MtReaction::Mt2Elastic)
            .ok_or(NjoyError::SectionNotFound { mat: self.mat, mf: 3, mt: 2 })?
            .pairs
            .clone();
        Ok(crate::acer::thermal::IncidentPendf {
            mat: self.mat,
            awr: self.awr(),
            moderator_za: vec![self.za() as i32],
            elastic,
        })
    }
}

/// **(a)** Process an incident-neutron ENDF tape to its cross sections at
/// `temperature`: RECONR, BROADR (above 0 K), PURR and HEATR, as an NJOY
/// deck runs them, with `settings` ([`CrossSectionSettings::default`] is
/// NJOY's).
///
/// `endf` must hold one material (the incident nuclide's evaluation, e.g.
/// `n-001_H_001.endf`). `temperature` is at least 0 K; at 0 K BROADR and
/// PURR are not run, as in NJOY's `reconr → acer` deck.
///
/// **Cost.** Seconds for a light nuclide. PURR is a Monte Carlo over
/// resonance ladders: about 45 s for U-238 on one core, the price of the
/// unresolved-resonance self-shielding (switch it off with
/// [`CrossSectionSettings::without_purr`] only to ablate it).
///
/// # Errors
/// [`NjoyError::InvalidInput`] for a tape with no material or several, or a
/// negative or non-finite temperature; and those of RECONR, PURR and HEATR.
pub fn cross_sections(
    endf: &Tape,
    temperature: Temperature,
    settings: &CrossSectionSettings,
) -> Result<NuclearCrossSections, NjoyError> {
    let t_k = temperature.get::<kelvin>();
    if !t_k.is_finite() || t_k < 0.0 {
        return Err(NjoyError::InvalidInput(format!("temperature must be 0 K or above, got {t_k} K")));
    }
    let mat = single_material(endf)?;
    let pendf = reconr_broadr(endf, mat, t_k, settings.reconr_tolerance, settings.broadr_tolerance)?;
    let mut deck = AceDeck::default().with_temperature(t_k);
    if !settings.heatr {
        deck = deck.without_heatr();
    }
    if let (Some(p), true) = (settings.purr, t_k > 0.0) {
        deck = deck.with_purr(p.bands, p.ladders, p.samples);
    }
    let modules = crate::acer::deck_modules(endf, mat, &pendf, K_B_MEV * t_k, &deck)?;
    Ok(NuclearCrossSections {
        tape: Arc::new(endf.clone()),
        mat,
        temperature,
        pendf,
        urr: modules.urr,
        heating: modules.kerma,
        settings: settings.clone(),
    })
}

/// **(b)** Process an incident-neutron ENDF tape to its continuous-energy ACE
/// table at `temperature`: [`cross_sections`] followed by
/// [`NuclearCrossSections::to_ace`]. The file's bytes are
/// [`AceTable::to_type1_text`] (text, Type 1) or
/// [`AceTable::to_type2_bytes`] (binary, Type 2).
///
/// # Errors
/// Those of [`cross_sections`] and of ACER.
pub fn ace(endf: &Tape, temperature: Temperature, settings: &CrossSectionSettings) -> Result<AceTable, NjoyError> {
    cross_sections(endf, temperature, settings)?.to_ace()
}

/// The tape's one material, or an error naming what is there.
fn single_material(endf: &Tape) -> Result<i32, NjoyError> {
    match endf.materials().as_slice() {
        [mat] => Ok(*mat),
        [] => Err(NjoyError::InvalidInput("the ENDF tape holds no material".into())),
        many => Err(NjoyError::InvalidInput(format!(
            "the ENDF tape holds {} materials ({many:?}); pass a tape with one",
            many.len()
        ))),
    }
}

/// RECONR at `reconr_tol`, then BROADR to `t_k` at `broadr_tol` when
/// `t_k > 0`; the result as written to the PENDF tape.
fn reconr_broadr(endf: &Tape, mat: i32, t_k: f64, reconr_tol: f64, broadr_tol: f64) -> Result<ReconrResult, NjoyError> {
    if !(reconr_tol > 0.0 && broadr_tol > 0.0) {
        return Err(NjoyError::InvalidInput(format!(
            "RECONR and BROADR tolerances must be positive, got {reconr_tol} and {broadr_tol}"
        )));
    }
    let r0 = reconr(endf, &ReconrConfig { mat, tolerance: reconr_tol, temperature: 0.0 })?;
    let r = if t_k > 0.0 { crate::broadr::broaden_result_with_tolerance(&r0, t_k, broadr_tol) } else { r0 };
    Ok(r.through_pendf_text())
}
