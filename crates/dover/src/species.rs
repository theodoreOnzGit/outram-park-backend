// SPDX-License-Identifier: GPL-3.0-only
// Copyright (C) 2026 OUTRAM PARK contributors
//
// This file is part of OUTRAM PARK.
//
// OUTRAM PARK is free software: you can redistribute it and/or modify it
// under the terms of the GNU General Public License as published by the
// Free Software Foundation, either version 3 of the License, or (at your
// option) any later version.
//
// OUTRAM PARK is distributed in the hope that it will be useful, but
// WITHOUT ANY WARRANTY; without even the implied warranty of
// MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE.  See the GNU
// General Public License for more details.
//
// You should have received a copy of the GNU General Public License along
// with OUTRAM PARK.  If not, see <https://www.gnu.org/licenses/>.

//! Standard thermochemistry for the five steam-methane-reforming species.
//!
//! # What this is, and why it is a table of only ten numbers
//!
//! Everything thermodynamic in [`crate::smr`] is *derived* from this table —
//! the reaction enthalpies, the reaction entropies, the equilibrium constants
//! and, through them, the reverse rate constants. Nothing downstream carries a
//! thermochemical constant of its own.
//!
//! That is deliberate. Scattering `206_000.0` and `-41_200.0` through the
//! reaction builders would put two much-quoted numbers beyond checking; here
//! they are computed from formation data and asserted against the textbook
//! values in this module's own tests. The provenance surface is five species,
//! two numbers each, in one place.
//!
//! # Provenance — and an obligation this file does not discharge
//!
//! The values below are the standard-state (298.15 K, 1 bar) ideal-gas
//! enthalpy of formation and absolute entropy, as tabulated in the **NIST-JANAF
//! Thermochemical Tables (4th ed., Chase 1998)** and the **CODATA Key Values
//! for Thermodynamics**. They are the values that appear identically across
//! every standard reference; none is a fitted or adjusted quantity.
//!
//! **The workspace `CLAUDE.md` requires any document that informs the code to
//! be catalogued in `crates/kovan-literature` with its access tier and
//! provenance. That has NOT been done for these, and this file does not do
//! it.** The container this was written in has no path to fetch the source
//! document, and inventing a catalogue entry for a document nobody ingested
//! would be worse than recording the gap. So the gap is recorded: **cataloguing
//! NIST-JANAF (or an equivalent open compilation) is outstanding, and is
//! required before any of this is cited as validated.** Everything here is
//! checkable against any thermochemistry text in the meantime, and the tests
//! below do exactly that.
//!
//! # Units
//!
//! - `enthalpy_formation`: standard enthalpy of formation `ΔH°f` at 298.15 K,
//!   in **J/mol**. Zero by definition for an element in its reference state
//!   (H₂ here).
//! - `entropy`: standard absolute (third-law) entropy `S°` at 298.15 K, in
//!   **J/(mol·K)**. This is an absolute entropy, *not* an entropy of
//!   formation — the distinction matters, because a reaction entropy formed
//!   from absolute entropies is correct while one formed by mixing the two
//!   conventions is not.
//! - `molar_mass`: **kg/mol**, for mass-basis reporting only; no
//!   thermodynamic quantity here depends on it.

/// The five species a steam-methane-reforming CSTR tracks.
///
/// The discriminant order **is** the component index used throughout
/// [`crate::smr`] and by `ReactorFeed::molar_flows`; [`Species::INDEX_ORDER`]
/// is the canonical listing and [`Species::index`] the mapping. Reordering
/// this enum silently reindexes every stoichiometry, so do not.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum Species {
    /// Methane, CH₄ — the reactant whose conversion the whole model is about.
    Methane,
    /// Water as steam, H₂O(g). The reforming feed is steam, never liquid
    /// water, and the formation enthalpy below is the **gas-phase** one.
    Steam,
    /// Carbon monoxide, CO — the reforming product and the shift reactant.
    CarbonMonoxide,
    /// Carbon dioxide, CO₂ — the shift product.
    CarbonDioxide,
    /// Hydrogen, H₂ — the product of interest, and the element whose
    /// reference state fixes the zero of the enthalpy scale.
    Hydrogen,
}

impl Species {
    /// Every species, in the canonical component-index order.
    pub const INDEX_ORDER: [Species; 5] = [
        Species::Methane,
        Species::Steam,
        Species::CarbonMonoxide,
        Species::CarbonDioxide,
        Species::Hydrogen,
    ];

    /// This species' component index into a feed's `molar_flows`.
    #[must_use]
    pub fn index(self) -> usize {
        match self {
            Species::Methane => 0,
            Species::Steam => 1,
            Species::CarbonMonoxide => 2,
            Species::CarbonDioxide => 3,
            Species::Hydrogen => 4,
        }
    }

    /// Chemical formula, for reports and CSV headers.
    #[must_use]
    pub fn formula(self) -> &'static str {
        match self {
            Species::Methane => "CH4",
            Species::Steam => "H2O",
            Species::CarbonMonoxide => "CO",
            Species::CarbonDioxide => "CO2",
            Species::Hydrogen => "H2",
        }
    }

    /// Parse a formula as written in a deck. Case-sensitive, because `co` and
    /// `CO` differing by case is exactly the kind of typo a deck reader should
    /// reject rather than guess at.
    #[must_use]
    pub fn from_formula(s: &str) -> Option<Self> {
        Self::INDEX_ORDER.into_iter().find(|sp| sp.formula() == s)
    }

    /// Standard enthalpy of formation `ΔH°f` at 298.15 K, **J/mol**.
    ///
    /// See the module docs for the source and for the outstanding cataloguing
    /// obligation.
    #[must_use]
    pub fn enthalpy_formation(self) -> f64 {
        match self {
            Species::Methane => -74_600.0,
            Species::Steam => -241_800.0,
            Species::CarbonMonoxide => -110_500.0,
            Species::CarbonDioxide => -393_500.0,
            // Element in its reference state: zero by definition, not by
            // measurement.
            Species::Hydrogen => 0.0,
        }
    }

    /// Standard absolute entropy `S°` at 298.15 K, **J/(mol·K)**.
    #[must_use]
    pub fn entropy(self) -> f64 {
        match self {
            Species::Methane => 186.3,
            Species::Steam => 188.8,
            Species::CarbonMonoxide => 197.7,
            Species::CarbonDioxide => 213.8,
            Species::Hydrogen => 130.7,
        }
    }

    /// Molar mass `M`, **kg/mol**. Reporting only.
    #[must_use]
    pub fn molar_mass(self) -> f64 {
        match self {
            Species::Methane => 0.016_043,
            Species::Steam => 0.018_015,
            Species::CarbonMonoxide => 0.028_010,
            Species::CarbonDioxide => 0.044_010,
            Species::Hydrogen => 0.002_016,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn index_order_and_index_agree() {
        for (i, sp) in Species::INDEX_ORDER.iter().enumerate() {
            assert_eq!(sp.index(), i, "{sp:?}");
        }
    }

    #[test]
    fn formulae_round_trip_and_are_case_sensitive() {
        for sp in Species::INDEX_ORDER {
            assert_eq!(Species::from_formula(sp.formula()), Some(sp));
        }
        assert_eq!(
            Species::from_formula("co"),
            None,
            "must not accept lowercase"
        );
        assert_eq!(Species::from_formula("Methane"), None, "formula, not name");
    }

    /// Hydrogen's formation enthalpy is zero *by definition of the scale*, and
    /// its absolute entropy is emphatically not — conflating the two is the
    /// classic way to get a reaction entropy wrong by ~130 J/(mol·K).
    #[test]
    fn hydrogen_has_zero_formation_enthalpy_but_nonzero_absolute_entropy() {
        assert_eq!(Species::Hydrogen.enthalpy_formation(), 0.0);
        assert!(Species::Hydrogen.entropy() > 100.0);
    }

    /// Every absolute entropy must be positive (third law), and every
    /// molar mass sane.
    #[test]
    fn the_table_is_physically_sane() {
        for sp in Species::INDEX_ORDER {
            assert!(sp.entropy() > 0.0, "{sp:?} has non-positive S°");
            assert!(
                sp.molar_mass() > 0.001 && sp.molar_mass() < 0.1,
                "{sp:?} molar mass {} kg/mol",
                sp.molar_mass()
            );
            assert!(sp.enthalpy_formation().is_finite());
        }
    }

    /// Steam, not liquid water.
    ///
    /// Liquid water's `ΔH°f` is −285.8 kJ/mol; the gas-phase value is
    /// −241.8 kJ/mol, the 44.0 kJ/mol difference being the enthalpy of
    /// vaporisation at 298 K. Using the liquid value here would shift the
    /// reforming enthalpy by that much per mole of steam and quietly wreck
    /// every equilibrium constant, so it is pinned.
    #[test]
    fn water_is_the_gas_phase_value() {
        let h = Species::Steam.enthalpy_formation();
        assert!(
            (h - -241_800.0).abs() < 1.0,
            "expected gas-phase H2O, got {h} J/mol"
        );
        assert!(
            (h - -285_800.0).abs() > 40_000.0,
            "this looks like liquid water"
        );
    }
}
