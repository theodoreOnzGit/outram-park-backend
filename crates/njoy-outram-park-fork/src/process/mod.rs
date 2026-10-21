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
//! - **Physics on by default** (root `CLAUDE.md`): PURR's probability tables,
//!   HEATR's heating and the 0 K elastic that DBRC needs
//!   ([`NuclearCrossSections::elastic_0k`], GitHub #928) are in the default
//!   [`CrossSectionSettings`]; turning any off is an explicit `without_*`
//!   call.
//! - **One RECONR, many temperatures** (GitHub #934): [`cross_sections`] is
//!   [`reconstruct`] (RECONR at 0 K, independent of temperature) followed by
//!   [`cross_sections_at`] (BROADR, PURR, HEATR). Keep the [`Reconstruction`]
//!   and call the second as often as needed, or use
//!   [`cross_sections_at_temperatures`]; the results are bit for bit those of
//!   [`cross_sections`].
//! - **The object form.** [`crate::interface::NuclearDataLibrary`] holds the
//!   tape, the settings, the [`Reconstruction`] and the latest
//!   [`NuclearCrossSections`] between calls; each of its methods is a thin
//!   `#[inline]` call into a function here (GitHub #933), so the two styles
//!   cannot drift.
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
    /// The **0 K (unbroadened) elastic** cross section, `(E [eV], σ [b])`,
    /// that DBRC (Doppler Broadening Rejection Correction) needs to accept or
    /// reject a sampled target velocity (GitHub #928). It is RECONR's MT=2 at
    /// 0 K as written to the 0 K PENDF tape: every value through ENDF's
    /// 11-column field, the same precision as [`Self::pendf`]. At 0 K it is
    /// [`Self::pendf`]'s MT=2, bit for bit. The whole energy range is kept;
    /// a transport code uses the part below its DBRC limit.
    ///
    /// **On by default** (root `CLAUDE.md`: physics the evaluation supplies is
    /// default-on). `None` only after
    /// [`CrossSectionSettings::without_elastic_0k`], or for an evaluation with
    /// no MT=2.
    pub elastic_0k: Option<Vec<(f64, f64)>>,
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
/// Exactly [`reconstruct`] followed by [`cross_sections_at`]. For several
/// temperatures of one nuclide call those two, or
/// [`cross_sections_at_temperatures`], so that RECONR runs once.
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
    check_temperature(temperature)?;
    check_tolerances(settings)?;
    cross_sections_at(&reconstruct(endf, settings)?, temperature, settings)
}

/// RECONR's 0 K pointwise reconstruction of one evaluation: the
/// temperature-independent first stage of [`cross_sections`]. Made by
/// [`reconstruct`]; [`cross_sections_at`] turns it into cross sections at
/// any temperature. Holding one is how a single RECONR serves several
/// temperatures (GitHub #934).
#[derive(Debug, Clone)]
pub struct Reconstruction {
    /// The evaluation, shared with every [`NuclearCrossSections`] made from
    /// this reconstruction.
    pub tape: Arc<Tape>,
    /// The ENDF material number (MAT), read from the tape.
    pub mat: i32,
    /// RECONR's result at 0 K at full `f64` precision, as on RECONR's binary
    /// tape. [`cross_sections_at`] applies the PENDF text rounding.
    pub reconr: ReconrResult,
    /// The RECONR tolerance it was made with (fractional).
    pub reconr_tolerance: f64,
}

impl Reconstruction {
    /// The atomic weight ratio (target mass / neutron mass) from MF=1/MT=451.
    pub fn awr(&self) -> f64 {
        self.reconr.material.awr
    }

    /// `1000 Z + A` of the target.
    pub fn za(&self) -> f64 {
        self.reconr.material.za
    }
}

/// The first stage of [`cross_sections`]: RECONR at 0 K on the tape's one
/// material, at `settings.reconr_tolerance`. No other setting is read.
///
/// # Errors
/// [`NjoyError::InvalidInput`] for a tape with no material or several, or a
/// tolerance that is not positive; and RECONR's own.
pub fn reconstruct(endf: &Tape, settings: &CrossSectionSettings) -> Result<Reconstruction, NjoyError> {
    let tolerance = settings.reconr_tolerance;
    if !(tolerance > 0.0) {
        return Err(NjoyError::InvalidInput(format!("the RECONR tolerance must be positive, got {tolerance}")));
    }
    let mat = single_material(endf)?;
    let reconr = reconr(endf, &ReconrConfig { mat, tolerance, temperature: 0.0 })?;
    Ok(Reconstruction { tape: Arc::new(endf.clone()), mat, reconr, reconr_tolerance: tolerance })
}

/// The rest of [`cross_sections`] after its first stage: BROADR (above 0 K),
/// PURR and HEATR at `temperature`, and the 0 K elastic. Bit for bit what
/// [`cross_sections`] returns for the same tape, temperature and settings.
///
/// # Errors
/// [`NjoyError::InvalidInput`] for a negative or non-finite temperature, a
/// tolerance that is not positive, or a `settings.reconr_tolerance` other
/// than the one `recon` was made with (the result would record settings it
/// was not made with); and those of PURR and HEATR.
pub fn cross_sections_at(
    recon: &Reconstruction,
    temperature: Temperature,
    settings: &CrossSectionSettings,
) -> Result<NuclearCrossSections, NjoyError> {
    check_temperature(temperature)?;
    check_tolerances(settings)?;
    if settings.reconr_tolerance.to_bits() != recon.reconr_tolerance.to_bits() {
        return Err(NjoyError::InvalidInput(format!(
            "the reconstruction was made at RECONR tolerance {}, the settings ask for {}",
            recon.reconr_tolerance, settings.reconr_tolerance
        )));
    }
    let t_k = temperature.get::<kelvin>();
    let (endf, mat, r0) = (recon.tape.as_ref(), recon.mat, &recon.reconr);
    let pendf = if t_k > 0.0 {
        crate::broadr::broaden_result_with_tolerance(r0, t_k, settings.broadr_tolerance).through_pendf_text()
    } else {
        r0.through_pendf_text()
    };
    let elastic_0k = if settings.elastic_0k { elastic_0k_on_pendf(r0) } else { None };
    let mut deck = AceDeck::default().with_temperature(t_k);
    if !settings.heatr {
        deck = deck.without_heatr();
    }
    if let (Some(p), true) = (settings.purr, t_k > 0.0) {
        deck = deck.with_purr(p.bands, p.ladders, p.samples);
    }
    let modules = crate::acer::deck_modules(endf, mat, &pendf, K_B_MEV * t_k, &deck)?;
    Ok(NuclearCrossSections {
        tape: Arc::clone(&recon.tape),
        mat,
        temperature,
        pendf,
        urr: modules.urr,
        heating: modules.kerma,
        elastic_0k,
        settings: settings.clone(),
    })
}

/// [`cross_sections`] at each of `temperatures` from **one** RECONR
/// (GitHub #934): [`reconstruct`] once, then [`cross_sections_at`] per
/// temperature, in the order given. Each element is bit for bit what
/// [`cross_sections`] returns at that temperature.
///
/// # Errors
/// Those of [`reconstruct`] and [`cross_sections_at`]. Every temperature is
/// checked before RECONR runs.
pub fn cross_sections_at_temperatures(
    endf: &Tape,
    temperatures: &[Temperature],
    settings: &CrossSectionSettings,
) -> Result<Vec<NuclearCrossSections>, NjoyError> {
    for &t in temperatures {
        check_temperature(t)?;
    }
    check_tolerances(settings)?;
    let recon = reconstruct(endf, settings)?;
    temperatures.iter().map(|&t| cross_sections_at(&recon, t, settings)).collect()
}

/// RECONR's 0 K MT=2 as written to the 0 K PENDF tape, or `None` without an
/// MT=2. The rounding is [`ReconrResult::through_pendf_text`]'s, applied to
/// that one section.
fn elastic_0k_on_pendf(r0: &ReconrResult) -> Option<Vec<(f64, f64)>> {
    use crate::endf::parse::{format_endf_float, parse_endf_float};
    let rt = |x: f64| parse_endf_float(&format_endf_float(x)).unwrap_or(x);
    let el = r0.sections.iter().find(|s| s.mt == MtReaction::Mt2Elastic)?;
    Some(el.pairs.iter().map(|&(e, s)| (rt(e), rt(s))).collect())
}

/// A temperature [`cross_sections`] accepts: finite and at least 0 K.
fn check_temperature(temperature: Temperature) -> Result<(), NjoyError> {
    let t_k = temperature.get::<kelvin>();
    if !t_k.is_finite() || t_k < 0.0 {
        return Err(NjoyError::InvalidInput(format!("temperature must be 0 K or above, got {t_k} K")));
    }
    Ok(())
}

/// Both RECONR's and BROADR's tolerances positive.
fn check_tolerances(settings: &CrossSectionSettings) -> Result<(), NjoyError> {
    let (r, b) = (settings.reconr_tolerance, settings.broadr_tolerance);
    if !(r > 0.0 && b > 0.0) {
        return Err(NjoyError::InvalidInput(format!(
            "RECONR and BROADR tolerances must be positive, got {r} and {b}"
        )));
    }
    Ok(())
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
///
/// # Errors
/// [`NjoyError::InvalidInput`] for a tape with no material or several.
pub fn single_material(endf: &Tape) -> Result<i32, NjoyError> {
    match endf.materials().as_slice() {
        [mat] => Ok(*mat),
        [] => Err(NjoyError::InvalidInput("the ENDF tape holds no material".into())),
        many => Err(NjoyError::InvalidInput(format!(
            "the ENDF tape holds {} materials ({many:?}); pass a tape with one",
            many.len()
        ))),
    }
}
