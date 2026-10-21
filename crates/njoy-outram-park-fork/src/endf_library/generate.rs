// SPDX-License-Identifier: GPL-3.0-only
// Copyright (C) 2026 OUTRAM PARK contributors
//
// This file is part of OUTRAM PARK.

//! **ACE files from a loaded library, at a chosen temperature** (gh #936):
//! what the browser's "generate ACE" page calls once the user has dropped an
//! ENDF folder, typed a temperature and picked tables. Every call is
//! [`crate::process`]; nothing here processes data of its own.
//!
//! - **Incident-neutron tables** ([`generate_ace`]): `process::ace` at the
//!   temperature, and by default **the 0 K companion** beside it, so DBRC has
//!   its 0 K elastic when the files are read back (#936 point 1, "the GUI
//!   generates the 0 K companion with each table"; correct physics is the
//!   default, [`AceRequest::without_zero_kelvin_companion`] ablates it).
//! - **Thermal tables** ([`generate_thermal_ace`]): `process::thermal_ace`
//!   from the **LEAPR deck compiled into this crate** for the chosen law,
//!   never from a `tsl-*.endf` tape (#870: the tapes are oracles). The
//!   incident nuclide's tape comes from the loaded library
//!   ([`suggest_thermal_partner`] finds it from the deck's ZA).
//!
//! The files are named the way `reference-data/ace` and
//! [`crate::acer`]'s readers expect: `<T>K/<name>.ace` for the table,
//! `0K/<name>.ace` for its companion, `<T>K/<law>.ace` for a thermal table;
//! [`crate::archive::write_zip`] packs a batch into one download.

use super::header::SubLibrary;
use super::index::{EndfLibraryIndex, IndexedTape};
use crate::leapr::deck::LeaprDeck;
use crate::leapr::decks::{embedded_deck_text, SabMaterial};
use crate::process::{self, CrossSectionSettings, ThermalSettings};
use crate::units::Temperature;
use crate::NjoyError;
use uom::si::thermodynamic_temperature::kelvin;

/// One generated ACE file.
#[derive(Debug, Clone)]
pub struct GeneratedAce {
    /// Where it goes in a library folder (`293.6K/U-235.ace`, `0K/U-235.ace`).
    pub path: String,
    /// Its ZAID as written (`92235.00c`, `tsl1.00t`).
    pub zaid: String,
    /// The Type-1 (ASCII) file.
    pub bytes: Vec<u8>,
}

/// **What to generate for an incident-neutron table**: the temperature and
/// the processing settings (NJOY's defaults, physics on), plus the 0 K
/// companion unless ablated.
#[derive(Debug, Clone)]
pub struct AceRequest {
    temperature: Temperature,
    settings: CrossSectionSettings,
    zero_kelvin_companion: bool,
}

impl AceRequest {
    /// At `temperature`, with [`CrossSectionSettings::default`] and the 0 K
    /// companion.
    #[must_use]
    pub fn new(temperature: Temperature) -> Self {
        Self {
            temperature,
            settings: CrossSectionSettings::default(),
            zero_kelvin_companion: true,
        }
    }

    /// **Ablation**: no 0 K companion. Tables read back without one carry
    /// DBRC off, and say so (`Nuclide::dbrc_unavailable_reason` in
    /// outram-mc). A 0 K companion is as large as the whole 0 K table
    /// (U-235: 253 MB as text, `reference-data/ace/MANIFEST.tsv`), which is
    /// the reason to want this in a browser.
    #[must_use]
    pub fn without_zero_kelvin_companion(mut self) -> Self {
        self.zero_kelvin_companion = false;
        self
    }

    /// Other processing settings (tolerances, PURR, HEATR).
    #[must_use]
    pub fn with_settings(mut self, settings: CrossSectionSettings) -> Self {
        self.settings = settings;
        self
    }
}

/// `293.6` → `"293.6K"`, `0` → `"0K"`: the temperature folder name.
#[must_use]
pub fn temperature_folder(t: Temperature) -> String {
    let k = t.get::<kelvin>();
    let mut s = format!("{k:.3}");
    while s.ends_with('0') {
        s.pop();
    }
    if s.ends_with('.') {
        s.pop();
    }
    format!("{s}K")
}

/// **The ACE table of `tape` at the request's temperature**, and its 0 K
/// companion unless ablated, as Type-1 files.
///
/// # Errors
/// Not an incident-neutron tape; a read, parse or processing failure.
pub fn generate_ace(
    index: &EndfLibraryIndex,
    tape: &IndexedTape,
    request: &AceRequest,
) -> Result<Vec<GeneratedAce>, NjoyError> {
    if tape.header.sub_library() != SubLibrary::IncidentNeutron {
        return Err(NjoyError::InvalidInput(format!(
            "{} is not an incident-neutron tape",
            tape.path
        )));
    }
    let endf = index.read_tape(tape)?;
    let name = tape.display_name();
    let mut temps = vec![request.temperature];
    if request.zero_kelvin_companion && request.temperature.get::<kelvin>() > 0.0 {
        temps.push(Temperature::new::<kelvin>(0.0));
    }
    temps
        .into_iter()
        .map(|t| {
            let table = process::ace(&endf, t, &request.settings)?;
            Ok(GeneratedAce {
                path: format!("{}/{name}.ace", temperature_folder(t)),
                zaid: table.zaid.trim().to_string(),
                bytes: table.to_type1_text()?.into_bytes(),
            })
        })
        .collect()
}

/// **The incident tape a thermal law is built on**, from the ZA in its
/// compiled-in LEAPR deck (H-1 for H in H₂O): the one tape of `library` with
/// that ZA. `None` when the deck names a nuclide the library lacks (graphite's
/// deck names natural carbon, ZA 6000, which ENDF/B-VIII.0 splits into C-12
/// and C-13), so the page must ask the user.
#[must_use]
pub fn suggest_thermal_partner<'a>(
    index: &'a EndfLibraryIndex,
    library: &str,
    law: SabMaterial,
) -> Option<&'a IndexedTape> {
    let deck = LeaprDeck::parse(embedded_deck_text(law)?).ok()?;
    let za = u32::try_from(deck.za.round() as i64).ok()?;
    index
        .incident_neutron(library, za / 1000, za % 1000, 0)
        .ok()
}

/// **The thermal ACE table of `law` at `temperature`**, from its compiled-in
/// LEAPR deck and the incident nuclide `partner` from the loaded library.
///
/// # Errors
/// No deck for the law; the partner is not an incident-neutron tape; those of
/// [`process::thermal_ace`] (a temperature the deck has no block for is
/// refused there, never interpolated).
pub fn generate_thermal_ace(
    index: &EndfLibraryIndex,
    law: SabMaterial,
    partner: &IndexedTape,
    temperature: Temperature,
    settings: &ThermalSettings,
) -> Result<GeneratedAce, NjoyError> {
    if partner.header.sub_library() != SubLibrary::IncidentNeutron {
        return Err(NjoyError::InvalidInput(format!(
            "{} is not an incident-neutron tape",
            partner.path
        )));
    }
    let deck = embedded_deck_text(law)
        .ok_or_else(|| NjoyError::InvalidInput(format!("no compiled-in LEAPR deck for {law:?}")))?;
    let endf = index.read_tape(partner)?;
    let table = process::thermal_ace(&endf, deck, temperature, settings)?;
    Ok(GeneratedAce {
        path: format!("{}/{}.ace", temperature_folder(temperature), law.label()),
        zaid: table.zaid.trim().to_string(),
        bytes: table.to_type1_text()?.into_bytes(),
    })
}
