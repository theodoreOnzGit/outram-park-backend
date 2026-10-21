//! The ACE way into [`super::NuclearData`] (gh #936): tables read as they
//! are. See [`super::NuclearData::from_ace_files`] for the rules.

use std::sync::Arc;

use njoy_outram_park_fork::acer::ace_set::{AceLookupError, AceSet, AceSetTable};
use njoy_outram_park_fork::acer::read::AceClass;
use njoy_outram_park_fork::leapr::deck::LeaprDeck;
use njoy_outram_park_fork::leapr::decks::{embedded_deck_text, SabMaterial};

use super::super::isotope::Isotope;
use super::super::masses::AtomicMasses;
use super::DataError;
use crate::material::nuclide::Nuclide;
use crate::material::thermal::ThermalScattering;

/// An indexed ACE set, and the thermal tables bound to laws by hand.
#[derive(Debug, Clone)]
pub(super) struct AceData {
    name: String,
    set: Arc<AceSet>,
    bound: Vec<(SabMaterial, String)>,
}

impl AceData {
    pub(super) fn new(name: &str, files: Vec<(String, Vec<u8>)>) -> Result<Self, DataError> {
        let set = AceSet::from_files(files);
        let neutron = set
            .tables()
            .iter()
            .any(|t| t.class() == AceClass::ContinuousNeutron);
        if !neutron {
            return Err(DataError::NoTapes(format!(
                "ACE files {name}: no continuous-energy neutron table ({} files skipped: {:?})",
                set.skipped().len(),
                set.skipped()
            )));
        }
        Ok(Self {
            name: name.to_string(),
            set: Arc::new(set),
            bound: Vec::new(),
        })
    }

    pub(super) fn bind(&mut self, law: SabMaterial, stem: &str) {
        self.bound.retain(|(l, _)| *l != law);
        self.bound.push((law, stem.to_string()));
    }

    /// Every table, with its file and temperature, and what was bound: the
    /// run's record of exactly which data it used.
    pub(super) fn label(&self) -> String {
        let tables: Vec<String> = self
            .set
            .tables()
            .iter()
            .map(AceSetTable::describe)
            .collect();
        let mut s = format!(
            "ACE files {} (read as they are, nothing processed): {}",
            self.name,
            tables.join("; ")
        );
        for (law, stem) in &self.bound {
            s.push_str(&format!("; {law:?} bound to '{stem}'"));
        }
        if !self.set.skipped().is_empty() {
            s.push_str(&format!(
                "; {} files not ACE, skipped",
                self.set.skipped().len()
            ));
        }
        s
    }

    fn err(&self, what: String, reason: impl std::fmt::Display) -> DataError {
        DataError::Ace {
            library: self.name.clone(),
            what,
            reason: reason.to_string(),
        }
    }

    fn za(&self, isotope: Isotope) -> Result<u32, DataError> {
        if isotope.metastable_state() > 0 {
            return Err(self.err(
                isotope.name(),
                "metastable targets are not matched on the ACE way in: ACE ZAIDs mark them by \
                 conventions the header does not state",
            ));
        }
        Ok(1000 * u32::from(isotope.z()) + u32::from(isotope.a()))
    }

    fn neutron_tables(&self, za: u32) -> impl Iterator<Item = &AceSetTable> {
        self.set
            .tables()
            .iter()
            .filter(move |t| t.class() == AceClass::ContinuousNeutron && t.za() == Some(za))
    }

    pub(super) fn file_of(&self, isotope: Isotope) -> Result<&str, DataError> {
        let za = self.za(isotope)?;
        let mut tables: Vec<&AceSetTable> = self.neutron_tables(za).collect();
        tables.sort_by_key(|t| t.is_zero_kelvin());
        tables
            .first()
            .map(|t| t.file.as_str())
            .ok_or_else(|| self.err(isotope.name(), AceLookupError::Missing))
    }

    /// Masses from each neutron table's AWR (the same at every temperature).
    pub(super) fn atomic_masses(&self) -> AtomicMasses {
        let mut pairs: Vec<(Isotope, f64)> = Vec::new();
        for t in self.set.tables() {
            if t.class() != AceClass::ContinuousNeutron {
                continue;
            }
            let Some(za) = t.za() else { continue };
            let (Ok(z), Ok(a)) = (u8::try_from(za / 1000), u16::try_from(za % 1000)) else {
                continue;
            };
            let Ok(i) = Isotope::metastable(z, a, 0) else {
                continue;
            };
            if !pairs.iter().any(|(p, _)| *p == i) {
                pairs.push((i, t.table.header.awr));
            }
        }
        AtomicMasses::from_awr(pairs, &self.label())
    }

    /// The thermal table's ZAID name for `law`: bound by hand, else the name
    /// njoy's generator writes (`tsl` + the LEAPR deck's MAT).
    fn stem_for(&self, law: SabMaterial) -> Option<String> {
        if let Some((_, s)) = self.bound.iter().find(|(l, _)| *l == law) {
            return Some(s.clone());
        }
        let deck = LeaprDeck::parse(embedded_deck_text(law)?).ok()?;
        Some(format!("tsl{}", deck.mat))
    }

    pub(super) fn load_nuclide(
        &self,
        isotope: Isotope,
        temperature_k: f64,
        law: Option<SabMaterial>,
    ) -> Result<Nuclide, DataError> {
        let za = self.za(isotope)?;
        let what = format!("{isotope} at {temperature_k} K");
        let table = self
            .set
            .neutron(za, temperature_k)
            .map_err(|e| self.err(what.clone(), e))?;
        let njoy = |source| DataError::Njoy {
            what: format!("{isotope} from {}", table.file),
            source,
        };
        let mut nuc = Nuclide::from_ace(&table.table, &isotope.name()).map_err(njoy)?;
        if !nuc.has_dbrc() {
            match self.set.zero_kelvin(za) {
                Ok(cold) => nuc = nuc.with_elastic_0k_from_ace(&cold.table).map_err(njoy)?,
                // No companion: DBRC stays off, and the nuclide's
                // `dbrc_unavailable_reason` puts that in the run's provenance.
                Err(AceLookupError::Missing | AceLookupError::WrongTemperature(_)) => {}
                Err(e) => return Err(self.err(format!("{isotope}'s 0 K companion"), e)),
            }
        }
        let Some(law) = law else {
            return Ok(nuc);
        };
        let stem = self.stem_for(law).ok_or_else(|| {
            self.err(
                format!("{law:?}"),
                "no compiled-in LEAPR deck to name its table",
            )
        })?;
        let t = self.set.thermal(&stem, temperature_k).map_err(|e| {
            self.err(
                format!("thermal law {law:?} (table '{stem}') at {temperature_k} K"),
                e,
            )
        })?;
        let sab = ThermalScattering::from_ace(&t.table, &format!("{law:?}")).map_err(|source| {
            DataError::Njoy {
                what: format!("thermal law {law:?} from {}", t.file),
                source,
            }
        })?;
        Ok(nuc.with_thermal_scattering(sab))
    }
}
