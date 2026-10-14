//! (c) and (d): a LEAPR deck and an incident tape to a thermal scattering law
//! and its thermal ACE table.

use uom::si::thermodynamic_temperature::kelvin;

use super::{cross_sections, ThermalSettings};
use crate::acer::thermal::ace_incident_grid;
use crate::acer::AceTable;
use crate::endf::tape::Tape;
use crate::leapr::deck::LeaprDeck;
use crate::leapr::run::{run_deck_at_temperature, Mf1Header};
use crate::thermr::mf7::{parse_mf7_at_temperature, Mf7};
use crate::thermr::on_pendf::{thermr_tables, ThermrTables};
use crate::units::Temperature;
use crate::NjoyError;

/// A bound scatterer's thermal scattering law at one temperature, generated
/// by LEAPR and processed by THERMR on the incident nuclide's PENDF: what
/// NJOY's `leapr → reconr → broadr → thermr` deck leaves for ACER. Made by
/// [`thermal_scattering`]; [`Self::to_ace`] writes the thermal ACE table.
///
/// Two consumers read it:
/// - **ACER** reads [`Self::thermr`] ([`Self::to_ace`]);
/// - a **transport code** reads the law itself: `outram-mc-libs`'
///   `ThermalScattering::from_tape(&law.leapr_tape, law.mat, T, name)` takes
///   [`Self::leapr_tape`], or `ThermalScattering::from_ace` the table.
///
/// Cross sections are per principal atom (per H for H in H₂O); energies in
/// eV.
#[derive(Debug, Clone)]
pub struct ThermalScatteringLaw {
    /// The LEAPR deck's title (card 2).
    pub title: String,
    /// The thermal evaluation's MAT (LEAPR card 4), e.g. 1 for H in H₂O.
    pub mat: i32,
    /// The temperature LEAPR and THERMR ran at.
    pub temperature: Temperature,
    /// LEAPR's ENDF tape at [`Self::temperature`] (MF=1/MT=451 and MF=7),
    /// read back from its text as THERMR reads it.
    pub leapr_tape: Tape,
    /// LEAPR's MF=1/MT=451 header (its comment cards and dictionary).
    pub leapr_header: Mf1Header,
    /// The MF=7 law parsed from [`Self::leapr_tape`]: `S(α,β)` (MT=4) and the
    /// elastic channel (MT=2), if the deck has one.
    pub mf7: Mf7,
    /// What THERMR wrote: `calcem`'s inelastic table, the MF=3 on the
    /// incident PENDF's grid, the elastic tables, and the `natom`, `emax`,
    /// cosines and tolerance used.
    pub thermr: ThermrTables,
    /// The settings this was made with.
    pub settings: ThermalSettings,
}

impl ThermalScatteringLaw {
    /// ACER on this law: the thermal (class `t`) ACE table, on the incident
    /// energies ACER keeps up to THERMR's `emax`, with
    /// [`ThermalSettings::n_outgoing`] outgoing energies in
    /// [`ThermalSettings::form`], named [`ThermalSettings::table_name`]
    /// (`tsl<MAT>` when unset). The second half of [`thermal_ace`].
    ///
    /// # Errors
    /// [`NjoyError::InvalidInput`] for a table name that is not 2 to 6
    /// letters or digits, or no outgoing energies; those of ACER.
    pub fn to_ace(&self) -> Result<AceTable, NjoyError> {
        let name = match &self.settings.table_name {
            Some(n) => n.clone(),
            None => format!("tsl{}", self.mat),
        };
        // The ZAID is `(a6,'.',i2,'t')`; a one-character name puts a `.` in
        // column 2, which OpenMC's reader (and this crate's) take for an ACE
        // 2.0 header (`openmc/data/ace.py:350`).
        if !(2..=6).contains(&name.len()) || !name.chars().all(|c| c.is_ascii_alphanumeric()) {
            return Err(NjoyError::InvalidInput(format!(
                "thermal table name {name:?} must be 2 to 6 letters or digits (ACER's tname)"
            )));
        }
        if self.settings.n_outgoing == 0 {
            return Err(NjoyError::InvalidInput("ACER needs at least one outgoing energy".into()));
        }
        let grid = ace_incident_grid(self.thermr.emax_ev);
        AceTable::thermal_from_thermr(
            &self.thermr,
            &name,
            self.settings.ace_suffix,
            &grid,
            self.settings.n_outgoing,
            self.settings.form,
        )
    }
}

/// **(c)** A bound scatterer's thermal scattering law at `temperature`, from
/// its LEAPR deck and the incident nuclide's ENDF tape (`endf`, e.g. H-1's
/// for H in H₂O): LEAPR, then RECONR and BROADR of `endf` to `temperature`
/// ([`cross_sections`] with PURR and HEATR off: THERMR reads only the
/// elastic cross section, MAT, AWR and ZA, so they could not change the
/// result), then THERMR.
///
/// `leapr_deck` is the deck's text: a `.leapr` file, or NJOY input starting
/// with `leapr` (the decks of ENDF/B-VIII.0's thermal sublibrary are in
/// [`crate::leapr::decks`]).
///
/// **The temperature.** LEAPR runs *at* `temperature`, on the deck's block
/// for it ([`crate::leapr::run::run_deck_at_temperature`]): a deck with one
/// phonon spectrum (graphite, ZrH, …) accepts any temperature; a deck with a
/// spectrum per temperature (H₂O, D₂O) accepts one within `T/1000 + 5` K of
/// a declared temperature and refuses the rest. Because the law is tabulated
/// at the request, THERMR's `T/500` window always matches and nothing is
/// interpolated in temperature. NJOY2016 on the same cut deck makes the same
/// table; NJOY2016 on the published `tsl-*.endf` tape is a different oracle,
/// and agrees only to the tape's 7-figure print precision.
///
/// # Errors
/// [`NjoyError::InvalidInput`] for a temperature that is not positive and
/// finite, settings out of range, or a law without `B(4)`/`B(6)` when `emax`
/// or `natom` is not set; [`NjoyError::NotPorted`] when the deck has no block
/// for the temperature or uses a LEAPR feature the port lacks; those of
/// [`cross_sections`] and THERMR.
pub fn thermal_scattering(
    endf: &Tape,
    leapr_deck: &str,
    temperature: Temperature,
    settings: &ThermalSettings,
) -> Result<ThermalScatteringLaw, NjoyError> {
    let t_k = temperature.get::<kelvin>();
    if !t_k.is_finite() || t_k <= 0.0 {
        return Err(NjoyError::InvalidInput(format!("a thermal law needs a temperature above 0 K, got {t_k} K")));
    }
    if settings.n_cosines < 2 || !(settings.thermr_tolerance > 0.0) {
        return Err(NjoyError::InvalidInput(format!(
            "THERMR needs at least 2 cosines and a positive tolerance, got {} and {}",
            settings.n_cosines, settings.thermr_tolerance
        )));
    }
    let mut deck = LeaprDeck::parse(leapr_deck)?;
    if let Some(c) = settings.leapr_constants {
        deck = deck.with_constants(c);
    }
    let run = run_deck_at_temperature(&deck, t_k)?;
    // THERMR reads LEAPR's tape, so read it back from its text.
    let leapr_tape = Tape::read(std::io::Cursor::new(run.write_text()?))?;
    let mf7 = parse_mf7_at_temperature(&leapr_tape, deck.mat, Some(t_k))?;
    let b = mf7
        .incoherent_inelastic
        .as_ref()
        .ok_or(NjoyError::NotPorted("a thermal law without incoherent inelastic (MT=4)"))?
        .b
        .clone();
    let natom = match (settings.natom, b.get(5)) {
        (Some(n), _) => n,
        (None, Some(&n)) if n >= 1.0 => n,
        _ => return Err(NjoyError::InvalidInput("natom is not set and the law has no B(6) of 1 or more".into())),
    };
    let emax_ev = match (settings.emax_ev(), b.get(3)) {
        (Some(e), _) => e,
        (None, Some(&e)) if e > 0.0 => e,
        _ => return Err(NjoyError::InvalidInput("emax is not set and the law has no positive B(4)".into())),
    };
    if !(natom > 0.0 && emax_ev > 0.0) {
        return Err(NjoyError::InvalidInput(format!("natom and emax must be positive, got {natom} and {emax_ev} eV")));
    }
    let incident = cross_sections(endf, temperature, &settings.incident())?.incident_pendf()?;
    let thermr = thermr_tables(&mf7, t_k, natom, settings.n_cosines, emax_ev, settings.thermr_tolerance, Some(&incident))?;
    Ok(ThermalScatteringLaw {
        title: run.title,
        mat: deck.mat,
        temperature,
        leapr_tape,
        leapr_header: run.header,
        mf7,
        thermr,
        settings: settings.clone(),
    })
}

/// **(d)** The thermal (class `t`) ACE table of a bound scatterer at
/// `temperature`, from its LEAPR deck and the incident nuclide's ENDF tape:
/// [`thermal_scattering`] followed by [`ThermalScatteringLaw::to_ace`]. The
/// file's bytes are [`AceTable::to_type1_text`] or
/// [`AceTable::to_type2_bytes`].
///
/// # Errors
/// Those of [`thermal_scattering`] and [`ThermalScatteringLaw::to_ace`].
pub fn thermal_ace(
    endf: &Tape,
    leapr_deck: &str,
    temperature: Temperature,
    settings: &ThermalSettings,
) -> Result<AceTable, NjoyError> {
    thermal_scattering(endf, leapr_deck, temperature, settings)?.to_ace()
}
