// SPDX-License-Identifier: GPL-3.0
//
// TRISO-ATOPS fork — provenance
// -----------------------------
// Upstream project : TRISO-ATOPS (INL) — https://github.com/IdahoLabResearch/TRISO-ATOPS
// Upstream commit  : de374c8
// Upstream source  : trisoatops/utility_functions/calculation_functions.py
//                    (class `Nuclide`; the `nuclides` dict; the module-level
//                     `noble_gases`, `halogens`, `special_metals` lists)
// Original license : MIT — Copyright (c) 2026 Battelle Energy Alliance, LLC
//                    (DOE contract DE-AC07-05ID14517)
// This port is distributed under GPL-3.0 as part of the combined boon-lay work;
// the MIT notice above is retained (see LICENSE.triso-atops / NOTICE.triso-atops).

//! # Nuclide model — species records and transport groups
//!
//! This module ports TRISO-ATOPS's notion of a *nuclide*: the identity
//! (name, atomic number `Z`, mass number `A`), the decay data (half-life and
//! the derived decay constant `λ`), and the parent nuclide(s) whose decay feeds
//! it. It also ports the **five transport groups** the code sorts every nuclide
//! into, because a nuclide's group decides which release model is applied to it.
//!
//! ## What belongs here
//!
//! - [`TrisoAtopsNuclide`] — one immutable species record (ports the Python
//!   `Nuclide` class in `calculation_functions.py`).
//! - [`ElementGroup`] — the noble-gas / halogen / special-metal / silver / other
//!   partition used to dispatch release models (ports the `noble_gases`,
//!   `halogens`, `special_metals` lists and the `z == 47 or z == 46` silver test).
//! - [`nuclide_database`] — the ~80-nuclide supported table (ports the `nuclides`
//!   dict), see User Manual §2.1.3 / Table 4.
//!
//! ## What does *not* belong here
//!
//! The runtime `sl` (short-lived) and `parent_decay` flags. In TRISO-ATOPS those
//! are **not intrinsic** to a nuclide — they are recomputed for each run from the
//! reactor's irradiation time (`nuclide_import`) and the accident duration
//! (`nuclide_import_accident`). They therefore live in the nodal-orchestration
//! layer ([`crate::triso_atops_fork::normal_operation`], scaffolded).

pub mod nuclide_database;

pub use nuclide_database::{supported_nuclides, TRISO_ATOPS_NUCLIDE_COUNT};

use uom::si::f64::{Frequency, Time};
use uom::si::frequency::hertz;
use uom::si::time::second;

use super::DecayConstant;

/// A single fission-product nuclide as modelled by TRISO-ATOPS.
///
/// Ports the Python `Nuclide` class in `calculation_functions.py`. All fields are
/// intrinsic nuclide data (the run-dependent `sl` / `parent_decay` flags are
/// **not** stored here — see the module note).
///
/// # Fields
/// - `name` — the canonical TRISO-ATOPS name, e.g. `"Kr-85m"`, `"Ag-110m"`,
///   `"Cs-137"` (element symbol, hyphen, mass number, optional metastable `m`).
/// - `z` — atomic number (number of protons), dimensionless count.
/// - `a` — mass number (protons + neutrons), dimensionless count.
/// - `half_life` — the nuclide half-life `t½` (a [`Time`], SI seconds). Values
///   are from the IAEA Live Chart of Nuclides (User Manual §7 ref. 2).
/// - `parents` — canonical names of the nuclide's decay parent(s) within the
///   TRISO-ATOPS model; empty if the nuclide is treated purely as a direct
///   fission product. Used to chain parent → daughter activity in a run.
#[derive(Debug, Clone, PartialEq)]
pub struct TrisoAtopsNuclide {
    /// Canonical TRISO-ATOPS nuclide name, e.g. `"Cs-137"`.
    pub name: &'static str,
    /// Atomic number `Z` (proton count).
    pub z: u32,
    /// Mass number `A` (nucleon count).
    pub a: u32,
    /// Half-life `t½` (SI seconds).
    pub half_life: Time,
    /// Decay parent name(s) inside the TRISO-ATOPS model; empty ⇒ direct fission product.
    pub parents: &'static [&'static str],
}

impl TrisoAtopsNuclide {
    /// Build a nuclide record from a half-life given in **seconds**.
    ///
    /// Convenience constructor used by the [`nuclide_database`]; the half-life
    /// argument is a bare `f64` in seconds only because it is table data — the
    /// stored [`half_life`](Self::half_life) is a `uom` [`Time`].
    ///
    /// # Arguments
    /// - `name` — canonical name (`"Kr-85m"`).
    /// - `z`, `a` — atomic and mass numbers.
    /// - `half_life_seconds` — `t½` in seconds (IAEA value).
    /// - `parents` — decay-parent names (may be empty).
    #[must_use]
    pub fn from_seconds(
        name: &'static str,
        z: u32,
        a: u32,
        half_life_seconds: f64,
        parents: &'static [&'static str],
    ) -> Self {
        Self {
            name,
            z,
            a,
            half_life: Time::new::<second>(half_life_seconds),
            parents,
        }
    }

    /// The radioactive decay constant `λ = ln 2 / t½`.
    ///
    /// Returns a [`DecayConstant`] (SI `s^-1`). Ports the Python
    /// `self.lam = np.log(2) / hl`.
    #[must_use]
    pub fn decay_constant(&self) -> DecayConstant {
        let t_half_s = self.half_life.get::<second>();
        Frequency::new::<hertz>(std::f64::consts::LN_2 / t_half_s)
    }

    /// The transport [`ElementGroup`] this nuclide belongs to, from its `Z`.
    ///
    /// See [`ElementGroup::from_atomic_number`].
    #[must_use]
    pub fn element_group(&self) -> ElementGroup {
        ElementGroup::from_atomic_number(self.z)
    }
}

/// The five transport groups TRISO-ATOPS sorts nuclides into.
///
/// A nuclide's group selects which release model is applied to it (Booth vs.
/// breakthrough vs. the empirical noble-gas/halogen `<R/B>` correlation) and how
/// its plate-out / graphite activity is handled. Ports the module-level
/// `noble_gases`, `halogens`, and `special_metals` lists plus the `z == 47 or
/// z == 46` silver test in `calculation_functions.py`. See User Manual §3.1
/// ("Defining Nuclides").
///
/// The variants are dispatched with a `match` (no trait objects), so adding a
/// group forces every release-model dispatcher to handle it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ElementGroup {
    /// Noble gases: He, Ne, Ar, Kr, Xe, Rn (`Z ∈ {2, 10, 18, 36, 54, 86}`).
    /// Released via the empirical `<R/B>_fail` correlation; do not plate out or
    /// build graphite activity.
    NobleGas,
    /// Halogens **as defined by TRISO-ATOPS**: F, Cl, Br, I, At **plus** the
    /// chalcogens Se and Te grouped here for transport
    /// (`Z ∈ {9, 17, 35, 53, 85, 34, 52}`). Released via the empirical
    /// `<R/B>_fail` correlation like noble gases, but they plate out.
    Halogen,
    /// Special metals: Rb, Sr, Cs, Ba, Eu (`Z ∈ {37, 38, 55, 56, 63}`).
    /// Released via the Booth equivalent-sphere model.
    SpecialMetal,
    /// Silver group: Ag and Pd (`Z ∈ {47, 46}`). Released via the breakthrough
    /// model through the SiC layer (the limiting barrier for Ag).
    Silver,
    /// Any other fission metal not in the groups above. Assigned a fixed nominal
    /// `<R/B>_fail = 1e-5` and a fixed graphite attenuation in the upstream code.
    Other,
}

impl ElementGroup {
    /// Classify an atomic number `Z` into its TRISO-ATOPS transport group.
    ///
    /// The tests are applied in the same precedence as the upstream code:
    /// noble gas, then halogen (including Se/Te), then special metal, then
    /// silver/palladium, else other.
    ///
    /// # Arguments
    /// - `z` — atomic number (proton count), a dimensionless integer.
    #[must_use]
    pub fn from_atomic_number(z: u32) -> Self {
        const NOBLE_GASES: [u32; 6] = [2, 10, 18, 36, 54, 86];
        // TRISO-ATOPS counts Te (52) and Se (34) as halogens for transport.
        const HALOGENS: [u32; 7] = [9, 17, 34, 35, 52, 53, 85];
        const SPECIAL_METALS: [u32; 5] = [37, 38, 55, 56, 63];

        if NOBLE_GASES.contains(&z) {
            Self::NobleGas
        } else if HALOGENS.contains(&z) {
            Self::Halogen
        } else if SPECIAL_METALS.contains(&z) {
            Self::SpecialMetal
        } else if z == 47 || z == 46 {
            Self::Silver
        } else {
            Self::Other
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use approx::assert_relative_eq;

    #[test]
    fn element_group_classification_matches_upstream_lists() {
        assert_eq!(ElementGroup::from_atomic_number(36), ElementGroup::NobleGas); // Kr
        assert_eq!(ElementGroup::from_atomic_number(54), ElementGroup::NobleGas); // Xe
        assert_eq!(ElementGroup::from_atomic_number(53), ElementGroup::Halogen); // I
        assert_eq!(ElementGroup::from_atomic_number(52), ElementGroup::Halogen); // Te
        assert_eq!(ElementGroup::from_atomic_number(34), ElementGroup::Halogen); // Se
        assert_eq!(
            ElementGroup::from_atomic_number(55),
            ElementGroup::SpecialMetal
        ); // Cs
        assert_eq!(
            ElementGroup::from_atomic_number(38),
            ElementGroup::SpecialMetal
        ); // Sr
        assert_eq!(ElementGroup::from_atomic_number(47), ElementGroup::Silver); // Ag
        assert_eq!(ElementGroup::from_atomic_number(46), ElementGroup::Silver); // Pd
        assert_eq!(ElementGroup::from_atomic_number(40), ElementGroup::Other); // Zr
    }

    #[test]
    fn decay_constant_is_ln2_over_half_life() {
        // Cs-137: t½ ≈ 949232333 s ⇒ λ ≈ 7.30e-10 s^-1
        let cs137 = TrisoAtopsNuclide::from_seconds("Cs-137", 55, 137, 949_232_333.0, &[]);
        let lam = cs137.decay_constant().get::<hertz>();
        assert_relative_eq!(
            lam,
            std::f64::consts::LN_2 / 949_232_333.0,
            max_relative = 1e-12
        );
        assert_relative_eq!(lam, 7.302e-10, max_relative = 1e-3);
    }
}
