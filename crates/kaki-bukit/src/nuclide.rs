// PROVENANCE
//   Upstream project: CYCLUS <https://github.com/cyclus/cyclus>, which vendors
//                     PyNE's nucname module as src/pyne.{h,cc}
//   Upstream file:    src/pyne.cc  (namespace pyne::nucname)
//   Upstream commit:  d4faab7ce0566ccb8febfcf50915cdeedee59db0
//   Upstream licence: BSD-3-Clause (Cyclus); PyNE itself is BSD-2-Clause.
//                     Both are GPLv3-compatible.
//   Copyright (c) 2010-2016, University of Wisconsin Computational Nuclear
//   Engineering Research Group; PyNE developers.
//
//   Independent Rust translation, (C) 2026 Theodore Ong and the outram-park
//   contributors, GPL-3.0-only. Relicensing is ONE-WAY.

//! Nuclide identifiers in PyNE `zzzaaammmm` form.
//!
//! # The identifier format
//!
//! A nuclide is one `i32` packing three fields:
//!
//! ```text
//!     Z Z Z A A A M M M M
//!     \_____/ \___/ \_____/
//!      proton   mass  metastable
//!      number  number   state
//! ```
//!
//! so U-235 in its ground state is `92_235_0000` = `922350000`, and the first
//! metastable state of Am-242 is `95_242_0001` = `952420001`. Uranium as a
//! natural element, with no mass number, is `920000000`.
//!
//! # Scope of this translation
//!
//! PyNE's `nucname` also parses and prints roughly a dozen other conventions
//! (`zzaaam`, MCNP, Serpent, NIST, CINDER, ALARA, SZA). **None of those are
//! ported**, because Cyclus itself only ever calls `id`, `isnuclide`, `znum`,
//! `anum`, `snum` and `name`, and the remaining converters exist to talk to
//! specific external codes that this crate does not talk to. Porting them
//! speculatively would add several hundred lines of lookup tables with no
//! caller to exercise them.
//!
//! What *is* ported is exact: [`Nuc::is_nuclide`] reproduces
//! `pyne::nucname::isnuclide(int)` condition for condition, including the
//! `aaa <= zzz * 7` physicality bound that upstream applies in `id()`.

use core::fmt;

use crate::error::{CyclusError, Result};

/// A nuclide identifier in PyNE `zzzaaammmm` form.
///
/// # Validity
///
/// The wrapper does **not** enforce validity on construction, and that is
/// deliberate rather than an oversight: upstream's `CompMap` is keyed on a
/// bare `int`, and compositions read from a file routinely contain ids that
/// have to be *checked and reported*, not rejected at the point of parsing.
/// [`Nuc::new`] is therefore infallible and [`Nuc::is_nuclide`] is the check,
/// mirroring `compmath::ValidNucs`. Use [`Nuc::validated`] where an invalid id
/// should be an error at construction.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Nuc(pub i32);

impl Nuc {
    /// Wraps a raw `zzzaaammmm` identifier without checking it.
    #[inline]
    #[must_use]
    pub const fn new(id: i32) -> Self {
        Self(id)
    }

    /// Wraps a raw identifier, returning an error if it is not a nuclide.
    ///
    /// # Errors
    ///
    /// [`CyclusError::InvalidNuclide`] if [`Nuc::is_nuclide`] is `false`.
    pub fn validated(id: i32) -> Result<Self> {
        let n = Self(id);
        if n.is_nuclide() {
            Ok(n)
        } else {
            Err(CyclusError::InvalidNuclide(id))
        }
    }

    /// Builds an identifier from proton number, mass number and metastable
    /// state.
    ///
    /// # Parameters
    ///
    /// - `z` — proton number, 1 to 118.
    /// - `a` — mass number (nucleon count), 0 for a natural element.
    /// - `m` — metastable state, 0 for the ground state.
    ///
    /// # Errors
    ///
    /// [`CyclusError::Value`] if any field is outside the range the packing
    /// can represent (`z` in `1..=999`, `a` in `0..=999`, `m` in `0..=9999`).
    ///
    /// # Examples
    ///
    /// ```
    /// use kaki_bukit::nuclide::Nuc;
    ///
    /// assert_eq!(Nuc::from_zam(92, 235, 0)?.raw(), 922350000);
    /// assert_eq!(Nuc::from_zam(95, 242, 1)?.raw(), 952420001);
    /// # Ok::<(), kaki_bukit::error::CyclusError>(())
    /// ```
    pub fn from_zam(z: i32, a: i32, m: i32) -> Result<Self> {
        if !(1..=999).contains(&z) {
            return Err(CyclusError::Value("proton number out of range 1..=999"));
        }
        if !(0..=999).contains(&a) {
            return Err(CyclusError::Value("mass number out of range 0..=999"));
        }
        if !(0..=9999).contains(&m) {
            return Err(CyclusError::Value("metastable state out of range 0..=9999"));
        }
        Ok(Self(z * 10_000_000 + a * 10_000 + m))
    }

    /// The raw `zzzaaammmm` identifier.
    #[inline]
    #[must_use]
    pub const fn raw(self) -> i32 {
        self.0
    }

    /// Proton number `Z`. Upstream `pyne::nucname::znum`.
    #[inline]
    #[must_use]
    pub const fn z(self) -> i32 {
        self.0 / 10_000_000
    }

    /// Mass number `A`. Upstream `pyne::nucname::anum`.
    ///
    /// Note the `% 1000`: the mass field is three digits, so this is not
    /// simply `(id / 10_000)`.
    #[inline]
    #[must_use]
    pub const fn a(self) -> i32 {
        (self.0 / 10_000) % 1000
    }

    /// Metastable state. Upstream `pyne::nucname::snum`. `0` is the ground
    /// state.
    #[inline]
    #[must_use]
    pub const fn m(self) -> i32 {
        self.0 % 10_000
    }

    /// Neutron number `N = A - Z`.
    ///
    /// Not an upstream function; added because the decay and separations code
    /// reads more clearly with it than with the subtraction inline. Meaningless
    /// (and negative or zero) for a natural-element id, where `A` is 0.
    #[inline]
    #[must_use]
    pub const fn n(self) -> i32 {
        self.a() - self.z()
    }

    /// `true` if this is a specific nuclide. Upstream
    /// `pyne::nucname::isnuclide(int)`.
    ///
    /// Reproduces upstream's three conditions in order:
    ///
    ///  1. the id must exceed `10_000_000` (i.e. `Z >= 1`);
    ///  2. `A` must be non-zero — an id with `A == 0` names an *element*, not
    ///     a nuclide, and upstream returns `false` for it;
    ///  3. `A >= Z`, since a nuclide cannot have fewer nucleons than protons.
    ///
    /// It additionally applies the `A <= 7Z` bound that upstream enforces
    /// inside `id()` — `isnuclide` calls `id()` first and returns `false` when
    /// it throws `NotANuclide`, so the bound is part of the observable
    /// behaviour of `isnuclide` even though it is not written in its body.
    ///
    /// # Examples
    ///
    /// ```
    /// use kaki_bukit::nuclide::Nuc;
    ///
    /// assert!(Nuc::new(922350000).is_nuclide());   // U-235
    /// assert!(!Nuc::new(920000000).is_nuclide());  // natural uranium: an element
    /// assert!(!Nuc::new(10920000).is_nuclide());   // A < Z
    /// ```
    #[must_use]
    pub const fn is_nuclide(self) -> bool {
        if self.0 <= 10_000_000 {
            return false;
        }
        let zzz = self.0 / 10_000_000;
        let aaa = (self.0 % 10_000_000) / 10_000;
        if aaa == 0 {
            return false; // an element, not a nuclide
        }
        if aaa < zzz {
            return false;
        }
        // Upstream's physicality bound, applied inside pyne::nucname::id().
        if aaa > zzz * 7 {
            return false;
        }
        true
    }

    /// `true` if this id names a natural element rather than a nuclide
    /// (`A == 0`, e.g. `920000000` for natural uranium).
    #[inline]
    #[must_use]
    pub const fn is_element(self) -> bool {
        self.0 > 10_000_000 && (self.0 % 10_000_000) / 10_000 == 0
    }

    /// The IUPAC element symbol for this nuclide's proton number, e.g. `"U"`.
    ///
    /// Returns `None` for a proton number outside 1..=118.
    #[must_use]
    pub fn symbol(self) -> Option<&'static str> {
        element_symbol(self.z())
    }
}

/// Formats as the PyNE `name` form: symbol, mass number, and `M`-suffixed
/// metastable state — `U235`, `Am242M`, `U` for natural uranium.
///
/// Falls back to the raw integer when the proton number is not a known
/// element, so a malformed id still prints something a reader can act on
/// rather than panicking or printing nothing.
impl fmt::Display for Nuc {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self.symbol() {
            Some(sym) => {
                write!(f, "{sym}")?;
                if self.a() != 0 {
                    write!(f, "{}", self.a())?;
                }
                if self.m() != 0 {
                    write!(f, "M{}", self.m())?;
                }
                Ok(())
            }
            None => write!(f, "{}", self.0),
        }
    }
}

/// IUPAC element symbols indexed by `Z - 1`, hydrogen through oganesson.
const ELEMENT_SYMBOLS: [&str; 118] = [
    "H", "He", "Li", "Be", "B", "C", "N", "O", "F", "Ne", "Na", "Mg", "Al", "Si", "P", "S", "Cl",
    "Ar", "K", "Ca", "Sc", "Ti", "V", "Cr", "Mn", "Fe", "Co", "Ni", "Cu", "Zn", "Ga", "Ge", "As",
    "Se", "Br", "Kr", "Rb", "Sr", "Y", "Zr", "Nb", "Mo", "Tc", "Ru", "Rh", "Pd", "Ag", "Cd", "In",
    "Sn", "Sb", "Te", "I", "Xe", "Cs", "Ba", "La", "Ce", "Pr", "Nd", "Pm", "Sm", "Eu", "Gd", "Tb",
    "Dy", "Ho", "Er", "Tm", "Yb", "Lu", "Hf", "Ta", "W", "Re", "Os", "Ir", "Pt", "Au", "Hg", "Tl",
    "Pb", "Bi", "Po", "At", "Rn", "Fr", "Ra", "Ac", "Th", "Pa", "U", "Np", "Pu", "Am", "Cm", "Bk",
    "Cf", "Es", "Fm", "Md", "No", "Lr", "Rf", "Db", "Sg", "Bh", "Hs", "Mt", "Ds", "Rg", "Cn", "Nh",
    "Fl", "Mc", "Lv", "Ts", "Og",
];

/// The IUPAC symbol for proton number `z`, or `None` outside 1..=118.
#[must_use]
pub fn element_symbol(z: i32) -> Option<&'static str> {
    if (1..=118).contains(&z) {
        Some(ELEMENT_SYMBOLS[(z - 1) as usize])
    } else {
        None
    }
}

/// Nuclides this crate names directly, for readability at call sites.
///
/// Upstream writes these as bare integer literals throughout (`922350000`
/// appears eleven times in the enrichment toolkit alone). Naming them is the
/// "human interface layer" rule applied to a magic number: a reader hovering
/// over `nuc::U235` learns more than one hovering over `922350000`.
pub mod nuc {
    use super::Nuc;

    /// U-235, ground state.
    pub const U235: Nuc = Nuc::new(922350000);
    /// U-238, ground state.
    pub const U238: Nuc = Nuc::new(922380000);
    /// U-234, ground state — the minor feed isotope tracked by enrichment.
    pub const U234: Nuc = Nuc::new(922340000);
    /// Pu-239, ground state.
    pub const PU239: Nuc = Nuc::new(942390000);
    /// Pu-241, ground state.
    pub const PU241: Nuc = Nuc::new(942410000);
    /// O-16, ground state — the oxygen in an oxide fuel.
    pub const O16: Nuc = Nuc::new(80160000);
    /// Natural uranium as an element (no mass number).
    pub const U_NATURAL: Nuc = Nuc::new(920000000);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn decomposes_u235() {
        let u = nuc::U235;
        assert_eq!(u.z(), 92);
        assert_eq!(u.a(), 235);
        assert_eq!(u.m(), 0);
        assert_eq!(u.n(), 143);
        assert_eq!(u.symbol(), Some("U"));
    }

    #[test]
    fn decomposes_a_metastable_state() {
        let am242m = Nuc::new(952420001);
        assert_eq!(am242m.z(), 95);
        assert_eq!(am242m.a(), 242);
        assert_eq!(am242m.m(), 1);
    }

    #[test]
    fn from_zam_round_trips() {
        for (z, a, m) in [(92, 235, 0), (95, 242, 1), (8, 16, 0), (1, 1, 0)] {
            let n = Nuc::from_zam(z, a, m).unwrap();
            assert_eq!((n.z(), n.a(), n.m()), (z, a, m));
        }
    }

    #[test]
    fn from_zam_rejects_out_of_range_fields() {
        assert!(Nuc::from_zam(0, 1, 0).is_err());
        assert!(Nuc::from_zam(92, 1000, 0).is_err());
        assert!(Nuc::from_zam(92, 235, 10_000).is_err());
    }

    #[test]
    fn is_nuclide_matches_upstream_conditions() {
        // Real nuclides.
        assert!(nuc::U235.is_nuclide());
        assert!(nuc::O16.is_nuclide());
        assert!(Nuc::new(10010000).is_nuclide()); // H-1

        // An element id is explicitly not a nuclide upstream.
        assert!(!nuc::U_NATURAL.is_nuclide());
        assert!(nuc::U_NATURAL.is_element());

        // A < Z is unphysical.
        assert!(!Nuc::from_zam(92, 10, 0).unwrap().is_nuclide());

        // A > 7Z is upstream's physicality bound.
        assert!(!Nuc::from_zam(1, 8, 0).unwrap().is_nuclide());
        assert!(Nuc::from_zam(1, 7, 0).unwrap().is_nuclide());

        // Below the Z>=1 floor.
        assert!(!Nuc::new(0).is_nuclide());
        assert!(!Nuc::new(10_000_000).is_nuclide());
    }

    #[test]
    fn validated_reports_the_offending_id() {
        assert_eq!(
            Nuc::validated(920000000),
            Err(CyclusError::InvalidNuclide(920000000))
        );
        assert_eq!(Nuc::validated(922350000).unwrap(), nuc::U235);
    }

    #[test]
    fn display_uses_pyne_name_form() {
        use alloc::format;
        assert_eq!(format!("{}", nuc::U235), "U235");
        assert_eq!(format!("{}", Nuc::new(952420001)), "Am242M1");
        assert_eq!(format!("{}", nuc::U_NATURAL), "U");
        // Unknown element falls back to the raw id rather than panicking.
        // Z = 200 is beyond the periodic table, so there is no symbol.
        let unknown = Nuc::new(200 * 10_000_000);
        assert!(unknown.symbol().is_none());
        assert_eq!(format!("{}", unknown), "2000000000");
    }

    #[test]
    fn ordering_is_by_raw_id_so_compmaps_iterate_deterministically() {
        let mut v = alloc::vec![nuc::U238, nuc::O16, nuc::U235];
        v.sort();
        assert_eq!(v, alloc::vec![nuc::O16, nuc::U235, nuc::U238]);
    }
}
