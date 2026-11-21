//! **Waste classification and disposal rating from an activated inventory**
//! (gh:#1391). A port of OpenMC's `openmc/waste.py` (286 lines, MIT,
//! vendored at `upstream_source/OpenMC/openmc/waste.py`, commit
//! `8f79d9d02483c90e4134f03a9df4b75ce6b062bf`), whose two functions are
//! `Material.waste_classification` and `Material.waste_disposal_rating`
//! (`openmc/material.py:1838`, `:1862`).
//!
//! | OpenMC | here |
//! |---|---|
//! | `_waste_disposal_rating(mat, limits, metal, by_nuclide)` (`waste.py:79`) | [`waste_disposal_rating`], [`waste_disposal_rating_by_nuclide`] |
//! | `_waste_classification(mat, metal)` (`waste.py:7`) | [`waste_classification`] |
//! | the limit dictionaries inside `_waste_disposal_rating` | [`WasteLimits`], [`FETTER_LIMITS_CI_PER_M3`] and the NRC tables below |
//! | `Material.get_activity(units='Ci/m3', by_nuclide=True)` | [`WasteInventory::from_inventory`], from the workspace's one inventory type |
//! | `openmc.data.half_life` (ENDF/B-VIII.0 `half_life.json`) | [`HalfLifeTable`] |
//!
//! # What the rating is
//!
//! The rating is `sum_i (A_i / limit_i)` over the nuclides that have a limit,
//! with `A_i` the specific activity in Ci/m^3 and `limit_i` in the same unit.
//! Below 1 the material "meets" the limits. [`waste_classification`] applies
//! the NRC's sum-of-fractions rules of 10 CFR 61.55(a)(3)-(7) to the Table 1
//! (long-lived) and Table 2 (short-lived) ratings.
//!
//! # Data and citations
//!
//! The tables are upstream's, copied by a script that reads `waste.py`'s own
//! dictionaries (`ast`), not retyped, and checked against upstream by
//! `verification_and_validation/decay_photon_and_waste_1391/`:
//!
//! - **Fetter** ([`FETTER_LIMITS_CI_PER_M3`], 81 nuclides): "Specific activity
//!   limits for radionuclides with half-lives between 5 years and 1e12 years
//!   from Table 2 in Fetter" (`waste.py:130`); the docstring cites "Fetter et
//!   al. (1990)" (`waste.py:104`). Upstream gives no fuller citation. This
//!   crate takes it to be S. Fetter, E. T. Cheng and F. M. Mann, "Long term
//!   radioactive waste from fusion reactors: Part II", Fusion Engineering and
//!   Design 13 (1990) 239-246. **Not re-checked** against the paper.
//! - **NRC Table 1** (long-lived, `NRC_long`, `waste.py:215`): 10 CFR 61.55,
//!   Table 1. C-14, Tc-99 and I-129 are in Ci/m^3 (C-14 and, for metal, Ni-59
//!   and Nb-94 take the "activated metal" column); the transuranic limits are
//!   in nCi/g and converted with the material's mass density
//!   (`factor = 1e6 * rho / 1e9`).
//! - **NRC Table 2** (short-lived, `NRC_short_A/B/C`, `waste.py:253-279`):
//!   10 CFR 61.55, Table 2, columns 1-3 (Class A, B, C). Ni-63's limits depend
//!   on the metal column. Class A also gets **every nuclide whose half-life is
//!   below five years at 700 Ci/m^3** (the "radionuclides with half-lives
//!   less than 5 years" row), which needs a half-life table ([`HalfLifeTable`]).
//!
//! # Differences from upstream's inputs
//!
//! - Upstream reads a `Material`; this takes a [`WasteInventory`]: per-nuclide
//!   activity in Ci/m^3 and the mass density in g/cm^3. From an
//!   [`RadionuclideInventory`] the activity is `A_Bq / V * 1e6 / 3.7e10`
//!   (upstream: `lambda * 1e24 * N * (1e6 / 3.7e10)`; the two are the same
//!   number to round-off, because the inventory stores `lambda * atoms`).
//!   Mass density is supplied, not computed from atomic masses.
//! - Upstream's half-lives come from the ENDF/B-VIII.0 table
//!   `openmc/data/half_life.json` (73 KB). It is **not** embedded here:
//!   [`HalfLifeTable::from_openmc_json`] reads that file, and
//!   [`HalfLifeTable::from_chain`] reads a depletion chain's half-lives (the
//!   `chain_file` route of `openmc.data.half_life`). The table is a required
//!   argument of the NRC functions: a missing half-life would silently drop a
//!   short-lived nuclide from the Class A sum, as upstream's `None` does.
//! - [`waste_classification`]'s `metal` defaults differ upstream between
//!   `Material.waste_classification` (`False`) and `_waste_classification`
//!   (`True`); both are an explicit argument here.
//!
//! # Not a licensing tool
//!
//! This is education and V&V code (`RESPONSIBLE_USE.md`). It reproduces
//! OpenMC's reading of 10 CFR 61.55; it is not an authority for any
//! classification, licensing or disposal decision.

#[cfg(not(any(feature = "std", test)))]
#[allow(unused_imports)]
use crate::nostd_prelude::*;

use uom::si::f64::{MassDensity, Volume};
use uom::si::mass_density::gram_per_cubic_centimeter;
use uom::si::radioactivity::becquerel;
use uom::si::volume::cubic_centimeter;

use njoy_outram_park_fork::hdf5::depletion_chain_xml::DepletionChainXml;

use crate::hash::HashMap;
use crate::inventory::RadionuclideInventory;

/// Becquerel per curie (`openmc/material.py:44`, `_BECQUEREL_PER_CURIE`).
pub const BECQUEREL_PER_CURIE: f64 = 3.7e10;

/// Five years in seconds, as `waste.py:265` writes it:
/// `60.0 * 60.0 * 24.0 * 365.25 * 5.0`.
pub const FIVE_YEARS_SECONDS: f64 = 60.0 * 60.0 * 24.0 * 365.25 * 5.0;

/// Why a waste evaluation could not be made.
#[derive(Debug, Clone, PartialEq, thiserror::Error)]
pub enum WasteError {
    /// `region` is past the last region of the inventory.
    #[error("the inventory has no region {0}")]
    NoSuchRegion(usize),
    /// A non-positive or non-finite volume.
    #[error("the volume must be positive and finite")]
    BadVolume,
    /// A negative or non-finite mass density.
    #[error("the mass density must be non-negative and finite")]
    BadMassDensity,
    /// A half-life file that is not a flat JSON object of numbers.
    #[error("half-life JSON is malformed near byte {0}")]
    BadHalfLifeJson(usize),
}

/// A named set of specific-activity limits (`limits` of `_waste_disposal_rating`).
#[derive(Debug, Clone, PartialEq)]
pub enum WasteLimits {
    /// Fetter et al. (1990), Table 2.
    Fetter,
    /// 10 CFR 61.55 Table 1 (long-lived).
    NrcLong,
    /// 10 CFR 61.55 Table 2, Class A column.
    NrcShortA,
    /// 10 CFR 61.55 Table 2, Class B column.
    NrcShortB,
    /// 10 CFR 61.55 Table 2, Class C column.
    NrcShortC,
    /// Caller's limits: nuclide name and Ci/m^3.
    Custom(Vec<(String, f64)>),
}

/// The classification of [`waste_classification`] (the strings upstream returns).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WasteClass {
    /// "Class A".
    ClassA,
    /// "Class B".
    ClassB,
    /// "Class C".
    ClassC,
    /// "GTCC": greater than Class C.
    Gtcc,
}

impl WasteClass {
    /// The string `_waste_classification` returns.
    #[must_use]
    pub fn as_str(self) -> &'static str {
        match self {
            WasteClass::ClassA => "Class A",
            WasteClass::ClassB => "Class B",
            WasteClass::ClassC => "Class C",
            WasteClass::Gtcc => "GTCC",
        }
    }
}

/// Fetter et al. (1990) Table 2 limits, Ci/m^3, in upstream's order
/// (`waste.py:128-213`).
pub const FETTER_LIMITS_CI_PER_M3: [(&str, f64); 81] = [
    ("Be10", 5000.0),
    ("C14", 600.0),
    ("Al26", 0.09),
    ("Si32", 600.0),
    ("Cl36", 10.0),
    ("Ar39", 20000.0),
    ("Ar42", 20000.0),
    ("K40", 2.0),
    ("Ca41", 10000.0),
    ("Ti44", 200.0),
    ("Fe60", 0.1),
    ("Co60", 300000000.0),
    ("Ni59", 900.0),
    ("Ni63", 700000.0),
    ("Se79", 50.0),
    ("Kr81", 30.0),
    ("Sr90", 800000.0),
    ("Nb91", 200.0),
    ("Nb92", 0.2),
    ("Nb94", 0.2),
    ("Mo93", 4000.0),
    ("Tc97", 0.4),
    ("Tc98", 0.01),
    ("Tc99", 0.06),
    ("Pd107", 900.0),
    ("Ag108_m1", 3.0),
    ("Sn121_m1", 700000.0),
    ("Sn126", 0.1),
    ("I129", 2.0),
    ("Cs137", 50000.0),
    ("Ba133", 200000000.0),
    ("La137", 200.0),
    ("Sm151", 50000000.0),
    ("Eu150_m1", 3000.0),
    ("Eu152", 300000.0),
    ("Eu154", 5000000.0),
    ("Gd148", 200000.0),
    ("Gd150", 2000.0),
    ("Tb157", 5000.0),
    ("Tb158", 4.0),
    ("Dy154", 1000.0),
    ("Ho166_m1", 0.2),
    ("Hf178_m1", 9000.0),
    ("Hf182", 0.2),
    ("Re186_m1", 20.0),
    ("Ir192_m1", 1.0),
    ("Pt193", 200000000.0),
    ("Hg194", 0.5),
    ("Pb202", 0.6),
    ("Pb210", 30000000.0),
    ("Bi207", 9000.0),
    ("Bi208", 0.08),
    ("Bi210_m1", 1.0),
    ("Po209", 3000.0),
    ("Ra226", 0.1),
    ("Ra228", 30000000.0),
    ("Ac227", 500000.0),
    ("Th229", 2.0),
    ("Th230", 0.3),
    ("Th232", 0.1),
    ("Pa231", 0.7),
    ("U232", 30.0),
    ("U233", 20.0),
    ("U234", 90.0),
    ("U235", 2.0),
    ("Np236", 1.0),
    ("Np237", 1.0),
    ("Pu238", 70000.0),
    ("Pu239", 1000.0),
    ("Pu240", 1000.0),
    ("Pu241", 2000.0),
    ("Pu242", 1000.0),
    ("Pu244", 0.9),
    ("Am241", 50.0),
    ("Am242_m1", 300.0),
    ("Am243", 2.0),
    ("Cm243", 600.0),
    ("Cm244", 500000.0),
    ("Cm245", 5.0),
    ("Cm246", 800.0),
    ("Cm248", 800.0),
];

/// 10 CFR 61.55 Table 1 transuranics limited in nCi/g (`waste.py:231-249`),
/// each at 100 nCi/g.
const NRC_LONG_100_NCI_PER_G: [&str; 18] = [
    "Np237", "Pu238", "Pu239", "Pu240", "Pu242", "Pu244", "Am241", "Am243", "Cm243", "Cm244",
    "Cm245", "Cm246", "Cm247", "Cm248", "Bk247", "Cf249", "Cf250", "Cf251",
];

/// Half-lives in seconds, by lower-case nuclide name.
///
/// `None` is a stable nuclide: `openmc.data.half_life` returns `None` for it.
/// A name that is absent is also `None`, as upstream's `dict.get` is.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct HalfLifeTable {
    by_name: HashMap<String, Option<f64>>,
}

impl HalfLifeTable {
    /// An empty table: every nuclide reads as stable or unknown.
    #[must_use]
    pub fn empty() -> Self {
        Self::default()
    }

    /// `openmc/data/half_life.json` (ENDF/B-VIII.0): a flat JSON object of
    /// lower-case names and half-lives in seconds.
    ///
    /// # Errors
    /// [`WasteError::BadHalfLifeJson`] for anything else.
    pub fn from_openmc_json(text: &str) -> Result<Self, WasteError> {
        let bytes = text.as_bytes();
        let mut i = 0;
        let skip_ws = |i: &mut usize| {
            while *i < bytes.len() && bytes[*i].is_ascii_whitespace() {
                *i += 1;
            }
        };
        skip_ws(&mut i);
        if bytes.get(i) != Some(&b'{') {
            return Err(WasteError::BadHalfLifeJson(i));
        }
        i += 1;
        let mut table = Self::empty();
        loop {
            skip_ws(&mut i);
            match bytes.get(i) {
                Some(b'}') => return Ok(table),
                Some(b',') => {
                    i += 1;
                    continue;
                }
                Some(b'"') => {}
                _ => return Err(WasteError::BadHalfLifeJson(i)),
            }
            let start = i + 1;
            let end = text[start..].find('"').map(|k| start + k).ok_or(WasteError::BadHalfLifeJson(i))?;
            let key = &text[start..end];
            i = end + 1;
            skip_ws(&mut i);
            if bytes.get(i) != Some(&b':') {
                return Err(WasteError::BadHalfLifeJson(i));
            }
            i += 1;
            skip_ws(&mut i);
            let vstart = i;
            while i < bytes.len() && !matches!(bytes[i], b',' | b'}') && !bytes[i].is_ascii_whitespace() {
                i += 1;
            }
            let value = &text[vstart..i];
            let seconds = if value == "null" {
                None
            } else {
                Some(value.parse::<f64>().map_err(|_| WasteError::BadHalfLifeJson(vstart))?)
            };
            table.by_name.insert(key.to_ascii_lowercase(), seconds);
        }
    }

    /// The half-lives of a depletion chain's nuclides (`half_life` of
    /// `chain[isotope]`); a nuclide with none is stable.
    #[must_use]
    pub fn from_chain(chain: &DepletionChainXml) -> Self {
        let mut table = Self::empty();
        for n in &chain.nuclides {
            table.by_name.insert(n.name.to_ascii_lowercase(), n.half_life_seconds);
        }
        table
    }

    /// Put `other`'s entries over this table's, as a chain takes precedence
    /// over the ENDF/B-VIII.0 table in `openmc.data.half_life`.
    #[must_use]
    pub fn overlaid_with(mut self, other: &Self) -> Self {
        for (k, v) in &other.by_name {
            self.by_name.insert(k.clone(), *v);
        }
        self
    }

    /// `openmc.data.half_life(nuclide)`: seconds, or `None` for a stable or
    /// unknown nuclide.
    #[must_use]
    pub fn half_life(&self, nuclide: &str) -> Option<f64> {
        self.by_name.get(&nuclide.to_ascii_lowercase()).copied().flatten()
    }

    /// Number of entries.
    #[must_use]
    pub fn len(&self) -> usize {
        self.by_name.len()
    }

    /// True when there are no entries.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.by_name.is_empty()
    }
}

/// What the waste functions read from a material: specific activity of each
/// nuclide in Ci/m^3, in the order of the material's nuclides, and the mass
/// density in g/cm^3.
#[derive(Debug, Clone, PartialEq)]
pub struct WasteInventory {
    activities_ci_per_m3: Vec<(String, f64)>,
    mass_density_g_cm3: f64,
}

impl WasteInventory {
    /// From activities already in Ci/m^3.
    ///
    /// # Errors
    /// [`WasteError::BadMassDensity`].
    pub fn new(activities_ci_per_m3: Vec<(String, f64)>, mass_density_g_cm3: f64) -> Result<Self, WasteError> {
        if !(mass_density_g_cm3 >= 0.0) || !mass_density_g_cm3.is_finite() {
            return Err(WasteError::BadMassDensity);
        }
        Ok(Self { activities_ci_per_m3, mass_density_g_cm3 })
    }

    /// From the workspace's [`RadionuclideInventory`], one region of it, for
    /// the volume that region occupies: `A_Bq / V * 1e6 / 3.7e10` per nuclide
    /// (`Material.get_activity(units='Ci/m3')`, `material.py:1687`), nuclides
    /// named as GNDS names, in the inventory's order.
    ///
    /// # Errors
    /// [`WasteError::NoSuchRegion`], [`WasteError::BadVolume`],
    /// [`WasteError::BadMassDensity`].
    pub fn from_inventory(
        inventory: &RadionuclideInventory,
        region: usize,
        volume: Volume,
        mass_density: MassDensity,
    ) -> Result<Self, WasteError> {
        let v_cm3 = volume.get::<cubic_centimeter>();
        if !(v_cm3 > 0.0) || !v_cm3.is_finite() {
            return Err(WasteError::BadVolume);
        }
        let multiplier = 1e6 / BECQUEREL_PER_CURIE;
        let mut out = Vec::with_capacity(inventory.nuclides().len());
        for entry in inventory.nuclides() {
            let bq = entry.activity_in(region).ok_or(WasteError::NoSuchRegion(region))?.get::<becquerel>();
            out.push((entry.isotope().name(), bq / v_cm3 * multiplier));
        }
        Self::new(out, mass_density.get::<gram_per_cubic_centimeter>())
    }

    /// The activities, Ci/m^3.
    #[must_use]
    pub fn activities_ci_per_m3(&self) -> &[(String, f64)] {
        &self.activities_ci_per_m3
    }

    /// The mass density, g/cm^3.
    #[must_use]
    pub fn mass_density_g_cm3(&self) -> f64 {
        self.mass_density_g_cm3
    }
}

fn set_limit(limits: &mut Vec<(String, f64)>, name: &str, value: f64) {
    match limits.iter_mut().find(|(n, _)| n == name) {
        Some(slot) => slot.1 = value,
        None => limits.push((name.to_string(), value)),
    }
}

/// The limit dictionary `_waste_disposal_rating` builds for `limits`
/// (`waste.py:128-279`), in Ci/m^3.
fn limit_table(
    inventory: &WasteInventory,
    limits: &WasteLimits,
    metal: bool,
    half_lives: &HalfLifeTable,
) -> Vec<(String, f64)> {
    match limits {
        WasteLimits::Fetter => FETTER_LIMITS_CI_PER_M3.iter().map(|(n, v)| ((*n).to_string(), *v)).collect(),
        WasteLimits::NrcLong => {
            let mut t: Vec<(String, f64)> = Vec::new();
            set_limit(&mut t, "C14", 8.0);
            set_limit(&mut t, "Tc99", 3.0);
            set_limit(&mut t, "I129", 0.08);
            if metal {
                set_limit(&mut t, "C14", 80.0);
                set_limit(&mut t, "Ni59", 220.0);
                set_limit(&mut t, "Nb94", 0.2);
            }
            // nCi/g to Ci/m3
            let factor = (1e6 * inventory.mass_density_g_cm3) / 1e9;
            set_limit(&mut t, "Pu241", 3500.0 * factor);
            set_limit(&mut t, "Cm242", 20000.0 * factor);
            for name in NRC_LONG_100_NCI_PER_G {
                set_limit(&mut t, name, 100.0 * factor);
            }
            t
        }
        WasteLimits::NrcShortA => {
            let mut t: Vec<(String, f64)> = Vec::new();
            set_limit(&mut t, "H3", 40.0);
            set_limit(&mut t, "Co60", 700.0);
            set_limit(&mut t, "Ni63", if metal { 35.0 } else { 3.5 });
            set_limit(&mut t, "Sr90", 0.04);
            set_limit(&mut t, "Cs137", 1.0);
            // radionuclides with half-lives < 5 years
            for (nuc, _) in &inventory.activities_ci_per_m3 {
                if half_lives.half_life(nuc).is_some_and(|h| h < FIVE_YEARS_SECONDS) {
                    set_limit(&mut t, nuc, 700.0);
                }
            }
            t
        }
        WasteLimits::NrcShortB => {
            let mut t: Vec<(String, f64)> = Vec::new();
            set_limit(&mut t, "Ni63", if metal { 700.0 } else { 70.0 });
            set_limit(&mut t, "Sr90", 150.0);
            set_limit(&mut t, "Cs137", 44.0);
            t
        }
        WasteLimits::NrcShortC => {
            let mut t: Vec<(String, f64)> = Vec::new();
            set_limit(&mut t, "Ni63", if metal { 7000.0 } else { 700.0 });
            set_limit(&mut t, "Sr90", 7000.0);
            set_limit(&mut t, "Cs137", 4600.0);
            t
        }
        WasteLimits::Custom(v) => v.clone(),
    }
}

/// `_waste_disposal_rating(mat, limits, metal, by_nuclide=True)`
/// (`waste.py:79`): the ratio `activity / limit` of every nuclide that has a
/// limit, in the inventory's order.
#[must_use]
pub fn waste_disposal_rating_by_nuclide(
    inventory: &WasteInventory,
    limits: &WasteLimits,
    metal: bool,
    half_lives: &HalfLifeTable,
) -> Vec<(String, f64)> {
    let table = limit_table(inventory, limits, metal, half_lives);
    let mut ratio = Vec::new();
    for (nuc, ci_m3) in &inventory.activities_ci_per_m3 {
        if let Some((_, limit)) = table.iter().find(|(n, _)| n == nuc) {
            ratio.push((nuc.clone(), ci_m3 / limit));
        }
    }
    ratio
}

/// `_waste_disposal_rating(mat, limits, metal)` (`waste.py:79`): the sum of
/// the ratios. Below 1.0 the material meets the limits.
#[must_use]
pub fn waste_disposal_rating(
    inventory: &WasteInventory,
    limits: &WasteLimits,
    metal: bool,
    half_lives: &HalfLifeTable,
) -> f64 {
    let mut sum = 0.0;
    for (_, r) in waste_disposal_rating_by_nuclide(inventory, limits, metal, half_lives) {
        sum += r;
    }
    sum
}

/// `_waste_classification(mat, metal)` (`waste.py:7`): the NRC class from the
/// sum-of-fractions rules of 10 CFR 61.55(a)(3)-(7).
#[must_use]
pub fn waste_classification(inventory: &WasteInventory, metal: bool, half_lives: &HalfLifeTable) -> WasteClass {
    let ratio1 = waste_disposal_rating(inventory, &WasteLimits::NrcLong, metal, half_lives);
    let ratio2 = [
        waste_disposal_rating(inventory, &WasteLimits::NrcShortA, metal, half_lives),
        waste_disposal_rating(inventory, &WasteLimits::NrcShortB, metal, half_lives),
        waste_disposal_rating(inventory, &WasteLimits::NrcShortC, metal, half_lives),
    ];
    let table1_present = ratio1 > 0.0;
    let table2_present = ratio2.iter().any(|&x| x > 0.0);

    let classify_table2 = |col1: f64, col2: f64, col3: f64| {
        if col1 < 1.0 {
            WasteClass::ClassA
        } else if col2 < 1.0 {
            WasteClass::ClassB
        } else if col3 < 1.0 {
            WasteClass::ClassC
        } else {
            WasteClass::Gtcc
        }
    };

    if table1_present && table2_present {
        // 61.55(a)(5)
        if ratio1 < 0.1 {
            classify_table2(ratio2[0], ratio2[1], ratio2[2])
        } else if ratio1 < 1.0 {
            if ratio2[2] < 1.0 { WasteClass::ClassC } else { WasteClass::Gtcc }
        } else {
            WasteClass::Gtcc
        }
    } else if table1_present {
        // 61.55(a)(3)
        if ratio1 < 0.1 {
            WasteClass::ClassA
        } else if ratio1 < 1.0 {
            WasteClass::ClassC
        } else {
            WasteClass::Gtcc
        }
    } else if table2_present {
        // 61.55(a)(4)
        classify_table2(ratio2[0], ratio2[1], ratio2[2])
    } else {
        // 61.55(a)(6)
        WasteClass::ClassA
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::inventory::{InventoryPoint, InventorySource, Regions};
    use crate::model::Isotope;
    use uom::si::f64::Radioactivity;
    use uom::si::mass_density::gram_per_cubic_centimeter as gpcc;
    use uom::si::radioactivity::curie;
    use uom::si::volume::cubic_meter;

    fn inv(a: &[(&str, f64)], rho: f64) -> WasteInventory {
        WasteInventory::new(a.iter().map(|(n, v)| ((*n).to_string(), *v)).collect(), rho).unwrap()
    }

    fn hl() -> HalfLifeTable {
        HalfLifeTable::from_openmc_json(
            r#"{ "h3": 388789600.0, "co60": 166340000.0, "mn54": 26971000.0, "ni63": 3.1e9, "c14": 1.8e11, "xe135": 32904.0, "na24": 53856.0, "fe56": null }"#,
        )
        .unwrap()
    }

    #[test]
    fn half_life_json_and_lookup() {
        let t = hl();
        assert_eq!(t.len(), 8);
        assert!(!t.is_empty());
        assert!(HalfLifeTable::empty().is_empty());
        assert_eq!(t.half_life("H3"), Some(388789600.0));
        assert_eq!(t.half_life("Fe56"), None);
        assert_eq!(t.half_life("nothing"), None);
        for bad in ["[1]", "{\"a\" 1}", "{\"a\": x}", "{a: 1}", "{\"a\": 1"] {
            assert!(HalfLifeTable::from_openmc_json(bad).is_err(), "{bad}");
        }
        assert!(HalfLifeTable::from_openmc_json("{}").unwrap().is_empty());
    }

    #[test]
    fn chain_half_lives_overlay_the_table() {
        let chain = DepletionChainXml::parse(
            r#"<depletion_chain><nuclide name="H3" half_life="100.0" decay_modes="0" reactions="0"/><nuclide name="Co60" decay_modes="0" reactions="0"/></depletion_chain>"#,
        )
        .unwrap();
        let c = HalfLifeTable::from_chain(&chain);
        assert_eq!(c.half_life("h3"), Some(100.0));
        let both = hl().overlaid_with(&c);
        assert_eq!(both.half_life("H3"), Some(100.0));
        // a chain nuclide with no half-life is stable, even where the table has one
        assert_eq!(both.half_life("Co60"), None);
        assert_eq!(both.half_life("C14"), Some(1.8e11));
    }

    #[test]
    fn fetter_table_is_upstreams_and_rates_by_ratio() {
        assert_eq!(FETTER_LIMITS_CI_PER_M3.len(), 81);
        assert_eq!(FETTER_LIMITS_CI_PER_M3[0], ("Be10", 5000.0));
        assert_eq!(FETTER_LIMITS_CI_PER_M3[80], ("Cm248", 800.0));
        let m = inv(&[("Nb94", 0.1), ("Fe55", 5.0), ("C14", 300.0)], 7.8);
        let by = waste_disposal_rating_by_nuclide(&m, &WasteLimits::Fetter, false, &HalfLifeTable::empty());
        assert_eq!(by, vec![("Nb94".to_string(), 0.1 / 0.2), ("C14".to_string(), 300.0 / 600.0)]);
        let r = waste_disposal_rating(&m, &WasteLimits::Fetter, false, &HalfLifeTable::empty());
        assert_eq!(r, 0.5 + 0.5);
        let custom = WasteLimits::Custom(vec![("Fe55".to_string(), 10.0)]);
        assert_eq!(waste_disposal_rating(&m, &custom, false, &HalfLifeTable::empty()), 0.5);
    }

    #[test]
    fn nrc_long_limits_depend_on_metal_and_density() {
        let m = inv(&[("C14", 8.0), ("Ni59", 22.0), ("Pu239", 7.8e-3 * 100.0)], 7.8);
        let t = HalfLifeTable::empty();
        // not metal: C14 limit 8 -> ratio 1; Ni59 has no limit; Pu239 limit 100 nCi/g * 7.8 g/cm3 = 0.78 Ci/m3
        let by = waste_disposal_rating_by_nuclide(&m, &WasteLimits::NrcLong, false, &t);
        assert_eq!(by.len(), 2);
        assert_eq!(by[0].1, 1.0);
        assert!((by[1].1 - 0.78 / (100.0 * (1e6 * 7.8 / 1e9))).abs() < 1e-15);
        // metal: C14 limit 80, Ni59 220
        let by = waste_disposal_rating_by_nuclide(&m, &WasteLimits::NrcLong, true, &t);
        assert_eq!(by.len(), 3);
        assert_eq!(by[0].1, 0.1);
        assert_eq!(by[1].1, 0.1);
    }

    #[test]
    fn nrc_short_class_a_adds_short_lived_nuclides_at_700() {
        let m = inv(&[("Mn54", 700.0), ("Ni63", 3.5), ("Fe56", 1.0), ("H3", 40.0)], 7.8);
        let by = waste_disposal_rating_by_nuclide(&m, &WasteLimits::NrcShortA, false, &hl());
        // Mn54 (0.85 y) is added; Ni63 limit 3.5; Fe56 stable; H3 limit 40 (12.3 y, not < 5 y)
        let names: Vec<&str> = by.iter().map(|(n, _)| n.as_str()).collect();
        assert_eq!(names, vec!["Mn54", "Ni63", "H3"]);
        assert_eq!(by[0].1, 1.0);
        assert_eq!(by[1].1, 1.0);
        let metal = waste_disposal_rating_by_nuclide(&m, &WasteLimits::NrcShortA, true, &hl());
        assert_eq!(metal[1].1, 0.1);
        // an empty half-life table cannot see Mn54
        assert_eq!(waste_disposal_rating_by_nuclide(&m, &WasteLimits::NrcShortA, false, &HalfLifeTable::empty()).len(), 2);
        // Class B and C columns, metal and not
        let m = inv(&[("Ni63", 70.0), ("Sr90", 150.0), ("Cs137", 44.0)], 1.0);
        let t = HalfLifeTable::empty();
        assert_eq!(waste_disposal_rating(&m, &WasteLimits::NrcShortB, false, &t), 3.0);
        assert_eq!(waste_disposal_rating(&m, &WasteLimits::NrcShortB, true, &t), 0.1 + 1.0 + 1.0);
        let c = waste_disposal_rating_by_nuclide(&m, &WasteLimits::NrcShortC, false, &t);
        assert_eq!(c[0].1, 0.1);
        let c = waste_disposal_rating_by_nuclide(&m, &WasteLimits::NrcShortC, true, &t);
        assert_eq!(c[0].1, 0.01);
    }

    #[test]
    fn classification_walks_every_branch() {
        let t = hl();
        let cls = |a: &[(&str, f64)]| waste_classification(&inv(a, 1.0), false, &t);
        // (a)(6): nothing listed
        assert_eq!(cls(&[("Fe56", 1.0)]), WasteClass::ClassA);
        // (a)(3): table 1 only; I129 limit 0.08
        assert_eq!(cls(&[("I129", 0.001)]), WasteClass::ClassA);
        assert_eq!(cls(&[("I129", 0.04)]), WasteClass::ClassC);
        assert_eq!(cls(&[("I129", 0.08)]), WasteClass::Gtcc);
        // (a)(4): table 2 only. Cs137 limits 1, 44, 4600
        assert_eq!(cls(&[("Cs137", 0.5)]), WasteClass::ClassA);
        assert_eq!(cls(&[("Cs137", 10.0)]), WasteClass::ClassB);
        assert_eq!(cls(&[("Cs137", 1000.0)]), WasteClass::ClassC);
        assert_eq!(cls(&[("Cs137", 5000.0)]), WasteClass::Gtcc);
        // (a)(5): both tables
        assert_eq!(cls(&[("I129", 0.001), ("Cs137", 10.0)]), WasteClass::ClassB);
        assert_eq!(cls(&[("I129", 0.04), ("Cs137", 0.5)]), WasteClass::ClassC);
        assert_eq!(cls(&[("I129", 0.04), ("Cs137", 5000.0)]), WasteClass::Gtcc);
        assert_eq!(cls(&[("I129", 0.5), ("Cs137", 0.5)]), WasteClass::Gtcc);
        // metal form changes the C14 limit from 8 to 80
        let m = inv(&[("C14", 20.0)], 1.0);
        assert_eq!(waste_classification(&m, false, &t), WasteClass::Gtcc);
        assert_eq!(waste_classification(&m, true, &t), WasteClass::ClassC);
        assert_eq!(WasteClass::ClassA.as_str(), "Class A");
        assert_eq!(WasteClass::ClassB.as_str(), "Class B");
        assert_eq!(WasteClass::ClassC.as_str(), "Class C");
        assert_eq!(WasteClass::Gtcc.as_str(), "GTCC");
    }

    #[test]
    fn inventory_adapter_converts_bq_to_ci_per_m3() {
        let co60 = Isotope::from_name("Co60").unwrap();
        let am = Isotope::from_name("Am242_m1").unwrap();
        let inventory = RadionuclideInventory::from_activities(
            Regions::Named(vec!["a".into(), "b".into()]),
            vec![
                (co60, vec![Radioactivity::new::<curie>(2.0), Radioactivity::new::<curie>(4.0)]),
                (am, vec![Radioactivity::new::<becquerel>(0.0), Radioactivity::new::<becquerel>(3.7e10)]),
            ],
            InventoryPoint::unspecified(),
            InventorySource::Prescribed { reference: "test".into() },
        )
        .unwrap();
        let v = Volume::new::<cubic_meter>(2.0);
        let rho = MassDensity::new::<gpcc>(7.8);
        let w = WasteInventory::from_inventory(&inventory, 1, v, rho).unwrap();
        assert_eq!(w.activities_ci_per_m3()[0].0, "Co60");
        assert_eq!(w.activities_ci_per_m3()[1].0, "Am242_m1");
        // 4 Ci in 2 m3 = 2 Ci/m3 (curie is defined as 3.7e10 Bq)
        assert!((w.activities_ci_per_m3()[0].1 - 2.0).abs() < 1e-12);
        assert!((w.activities_ci_per_m3()[1].1 - 0.5).abs() < 1e-12);
        assert!((w.mass_density_g_cm3() - 7.8).abs() < 1e-12);
        assert_eq!(WasteInventory::from_inventory(&inventory, 2, v, rho), Err(WasteError::NoSuchRegion(2)));
        assert_eq!(
            WasteInventory::from_inventory(&inventory, 0, Volume::new::<cubic_meter>(0.0), rho),
            Err(WasteError::BadVolume)
        );
        assert_eq!(
            WasteInventory::from_inventory(&inventory, 0, v, MassDensity::new::<gpcc>(-1.0)),
            Err(WasteError::BadMassDensity)
        );
        // Am242m (0.5 Ci/m3 against 300) and Co60 (2 against 3e8) are Fetter nuclides
        let r = waste_disposal_rating(&w, &WasteLimits::Fetter, false, &HalfLifeTable::empty());
        let expect = 2.0 / 3e8 + 0.5 / 300.0;
        assert!((r - expect).abs() < 1e-12 * expect);
    }
}
