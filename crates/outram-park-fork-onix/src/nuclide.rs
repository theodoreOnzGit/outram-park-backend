//! Nuclide identity — atomic number, mass number, and metastable state.
//!
//! Physical quantity: a *nuclide* is one nuclear species, identified by its
//! proton number `Z` (dimensionless count), mass number `A = Z + N`
//! (dimensionless count of nucleons), and an integer *metastable state* index
//! `m` (0 = ground state, 1 = first isomer, …).
//!
//! ## Provenance (GPLv3 relicensing of MIT upstream)
//!
//! Ported from the ONIX depletion code (open-source, MIT licensed):
//!   * upstream project: ONIX — <https://github.com/jlanversin/ONIX>
//!   * upstream commit:  `7328dc6`
//!   * source files:     `onix/utils/functions.py` (`zamid_to_name`,
//!     `name_to_zamid`, lines 272–325 — the `zzaaam = 10000*Z + 10*A + m`
//!     packing convention) and `onix/passport.py` (the `zamid`/`state`
//!     accessors).
//!
//! This file is an independent Rust re-implementation of that convention. The
//! OUTRAM PARK fork relicenses the derived work under **GPL-3.0-only** (MIT is
//! GPL-3.0-compatible; the upstream MIT notice is preserved above).

/// A nuclide: proton number, mass number, and metastable-state index.
///
/// * `z` — proton number (atomic number). Valid range in practice `1..=118`.
/// * `a` — mass number (nucleons, protons + neutrons). Must satisfy `a >= z`.
/// * `m` — metastable-state index: `0` = ground state, `1` = first metastable
///   isomer, and so on. Physically small (`0..=2` for essentially all data).
///
/// Units: all three fields are **dimensionless integer counts**.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct Nuclide {
    /// Proton number `Z` (atomic number), dimensionless count.
    pub z: u32,
    /// Mass number `A` (nucleon count), dimensionless. Physically `a >= z`.
    pub a: u32,
    /// Metastable-state index (0 = ground, 1 = first isomer, …), dimensionless.
    pub m: u8,
}

/// ONIX-style packed nuclide id: `zamid = 10000*Z + 10*A + m`.
///
/// This is the exact packing used by ONIX (`name_to_zamid`,
/// `onix/utils/functions.py:322`) but held as an integer instead of a string.
/// Units: dimensionless. Example: U-235 ground state → `922350`; Am-242m1 →
/// `952421`.
pub type ZamId = u64;

impl Nuclide {
    /// Construct a nuclide from `Z`, `A`, and metastable index `m`.
    ///
    /// No physical validation is performed here (callers building chains from
    /// trusted data libraries would only ever pass valid `a >= z`); use
    /// [`Nuclide::is_physical`] if you need a sanity gate.
    pub const fn new(z: u32, a: u32, m: u8) -> Self {
        Self { z, a, m }
    }

    /// The ONIX packed id `10000*Z + 10*A + m` (dimensionless).
    ///
    /// Ported from `onix/utils/functions.py:322` (`name_to_zamid`). This is the
    /// hash key used to look nuclides up in a depletion system's index map.
    pub const fn zamid(&self) -> ZamId {
        10_000 * self.z as u64 + 10 * self.a as u64 + self.m as u64
    }

    /// Reconstruct a nuclide from its ONIX packed id.
    ///
    /// Inverse of [`Nuclide::zamid`]; mirrors the digit-slicing in ONIX's
    /// `zamid_to_name` (`onix/utils/functions.py:280`). The last digit is `m`,
    /// the preceding three are `A`, the remainder is `Z`.
    pub const fn from_zamid(zamid: ZamId) -> Self {
        let m = (zamid % 10) as u8;
        let a = ((zamid / 10) % 1000) as u32;
        let z = (zamid / 10_000) as u32;
        Self { z, a, m }
    }

    /// Neutron number `N = A - Z` (dimensionless count).
    ///
    /// Returns `None` if `a < z` (an unphysical nuclide).
    pub const fn neutron_number(&self) -> Option<u32> {
        self.a.checked_sub(self.z)
    }

    /// Basic physicality gate: `z >= 1` and `a >= z`.
    ///
    /// This is a coarse screen, not a check against a nuclide chart.
    pub const fn is_physical(&self) -> bool {
        self.z >= 1 && self.a >= self.z
    }

    /// Apply signed `(dZ, dA, dm)` deltas, returning the product nuclide.
    ///
    /// This is the primitive underlying every decay/reaction daughter lookup:
    /// ONIX encodes each channel as a `[dZ, dA, dm]` triple applied to the
    /// parent's packed id (`onix/data/list_and_dict.py:244,250`). Returns
    /// `None` if the result would be unphysical (`Z < 1` or `A < 1` or
    /// `A < Z`), which is how a channel with no tracked daughter drops out.
    pub fn apply_delta(&self, dz: i32, da: i32, dm: i32) -> Option<Nuclide> {
        let z = (self.z as i64 + dz as i64).try_into().ok()?;
        let a = (self.a as i64 + da as i64).try_into().ok()?;
        let m_i = self.m as i64 + dm as i64;
        if !(0..=255).contains(&m_i) {
            return None;
        }
        let candidate = Nuclide { z, a, m: m_i as u8 };
        if candidate.is_physical() {
            Some(candidate)
        } else {
            None
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn zamid_round_trip_u235() {
        // U-235 ground state: Z=92, A=235, m=0 -> 922350 (ONIX convention).
        let u235 = Nuclide::new(92, 235, 0);
        assert_eq!(u235.zamid(), 922_350);
        assert_eq!(Nuclide::from_zamid(922_350), u235);
    }

    #[test]
    fn zamid_round_trip_am242m() {
        // Am-242m1: Z=95, A=242, m=1 -> 952421 (ONIX convention).
        let am = Nuclide::new(95, 242, 1);
        assert_eq!(am.zamid(), 952_421);
        assert_eq!(Nuclide::from_zamid(952_421), am);
    }

    #[test]
    fn neutron_number_and_physicality() {
        let u235 = Nuclide::new(92, 235, 0);
        assert_eq!(u235.neutron_number(), Some(143));
        assert!(u235.is_physical());
        assert!(!Nuclide::new(92, 90, 0).is_physical());
    }
}
