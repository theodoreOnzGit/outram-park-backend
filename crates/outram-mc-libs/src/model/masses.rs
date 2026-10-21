//! **Atomic masses for weight ↔ atom conversions**, and the two constants
//! every number-density conversion needs.
//!
//! # Where the masses come from (PROVISIONAL: open on gh #937)
//!
//! Whether weight-percent inputs convert with the evaluation's own **ENDF
//! AWR** or with a **cited atomic-mass table** (e.g. AME2020) is still an
//! open maintainer decision on #937. Until it is made, this module has
//! exactly one source, [`MassSource::EndfAwr`], and every resolved material
//! reports it, so a number density computed from a weight basis always says
//! where its masses came from.
//!
//! `EndfAwr` is the mass transport already uses for kinematics: the AWR in
//! each tape's MF1/MT451 HEAD record, the ratio of the nuclide's mass to the
//! neutron's (ENDF-102, section 1.1), times the neutron mass in unified
//! atomic mass units ([`NEUTRON_MASS_U`]). One library, one mass per
//! nuclide, the same everywhere in a run.
//!
//! Only the availability and masses of the nuclides **in the library** are
//! known, which is also what natural-element expansion needs (OpenMC's
//! `Element.expand` partitions an isotope the library lacks into one it has,
//! `openmc/element.py:150-195`).

use std::collections::BTreeMap;

use super::isotope::Isotope;

/// Neutron mass in unified atomic mass units: CODATA 2018 recommended value,
/// `1.00866491595(49) u` (Tiesinga et al., Rev. Mod. Phys. 93, 025010
/// (2021)). Converts an ENDF AWR (a ratio to the neutron mass) to `u`.
pub const NEUTRON_MASS_U: f64 = 1.008_664_915_95;

/// The Avogadro constant \[mol⁻¹\], exact by the 2019 SI definition.
pub const AVOGADRO_PER_MOL: f64 = 6.022_140_76e23;

/// Atoms per barn·cm in one atom per cm³ (`1 barn = 1e-24 cm²`).
pub const BARN_CM_PER_CUBIC_CM: f64 = 1.0e-24;

/// Where a set of [`AtomicMasses`] came from. One variant today; see the
/// module docs for why the choice is still open.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum MassSource {
    /// `AWR × m_n` from each tape's MF1/MT451 HEAD record, read from the
    /// library named in the string.
    EndfAwr(String),
    /// Masses given directly by the caller, in `u` (tests, or a cited table
    /// the caller supplies; the citation is the string).
    Given(String),
}

impl std::fmt::Display for MassSource {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            MassSource::EndfAwr(lib) => {
                write!(f, "ENDF AWR x neutron mass (CODATA 2018), from {lib}")
            }
            MassSource::Given(why) => write!(f, "given by the caller: {why}"),
        }
    }
}

/// The nuclides a library holds, with their atomic masses in `u`.
///
/// A nuclide absent from this table is absent from the library: a material
/// that needs it fails to resolve with an error naming it, never silently.
#[derive(Debug, Clone, PartialEq)]
pub struct AtomicMasses {
    masses: BTreeMap<Isotope, f64>,
    source: MassSource,
}

impl AtomicMasses {
    /// From `(isotope, mass in u)` pairs, with a statement of where they came
    /// from. Later duplicates replace earlier ones.
    pub fn new(pairs: impl IntoIterator<Item = (Isotope, f64)>, source: MassSource) -> Self {
        Self {
            masses: pairs.into_iter().collect(),
            source,
        }
    }

    /// From ENDF AWRs (ratio to the neutron mass), converted with
    /// [`NEUTRON_MASS_U`].
    pub fn from_awr(pairs: impl IntoIterator<Item = (Isotope, f64)>, library: &str) -> Self {
        Self::new(
            pairs.into_iter().map(|(i, awr)| (i, awr * NEUTRON_MASS_U)),
            MassSource::EndfAwr(library.to_string()),
        )
    }

    /// Mass of `isotope` in `u`, if the library holds it.
    pub fn mass_u(&self, isotope: Isotope) -> Option<f64> {
        self.masses.get(&isotope).copied()
    }

    /// Whether the library holds `isotope`.
    pub fn contains(&self, isotope: Isotope) -> bool {
        self.masses.contains_key(&isotope)
    }

    /// Where the masses came from.
    pub fn source(&self) -> &MassSource {
        &self.source
    }

    /// Every nuclide the library holds, in (Z, A, m) order.
    pub fn isotopes(&self) -> impl Iterator<Item = Isotope> + '_ {
        self.masses.keys().copied()
    }
}

#[cfg(test)]
mod tests {
    use super::super::isotopes::*;
    use super::*;

    /// AWR converts with the neutron mass, an absent nuclide is reported
    /// absent, and the source travels with the table.
    #[test]
    fn awr_converts_with_the_neutron_mass() {
        let m = AtomicMasses::from_awr([(U235, 233.0248)], "test");
        assert!((m.mass_u(U235).unwrap() - 233.0248 * NEUTRON_MASS_U).abs() < 1e-12);
        assert!(m.contains(U235) && !m.contains(U238));
        assert_eq!(m.mass_u(U238), None);
        assert_eq!(m.isotopes().collect::<Vec<_>>(), vec![U235]);
        assert!(m.source().to_string().contains("ENDF AWR"));
        let g = AtomicMasses::new([(H1, 1.0)], MassSource::Given("unit".into()));
        assert!(g.source().to_string().contains("unit"));
    }
}
