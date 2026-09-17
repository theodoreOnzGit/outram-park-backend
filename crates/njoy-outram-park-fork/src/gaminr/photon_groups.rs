// Ported from NJOY2016 `src/gaminr.f90` (git commit ac5adf5f33d893e42f2eed7fb286b0d51c7580da).
// NJOY2016 is under a modified BSD 3-Clause (LANL/DOE) licence, GPL-compatible;
// this derivative file is distributed under GPL-3.0-only. This is a modified,
// non-LANL version, not endorsed by LANL/DOE. See crate root LICENSE.njoy + NOTICE.

//! Built-in photon (gamma) group structures for the GAMINR module.
//!
//! # What this module computes
//!
//! GAMINR's `genggp` (`gaminr.f90:585-776`) selects one of NJOY's built-in
//! multigroup *photon* energy-boundary structures by the integer index `igg`,
//! filling the group-boundary array `egg` (length `ngg+1`) and returning the
//! group count `ngg`. This is the photon analogue of GROUPR/ERRORR's neutron
//! `gengpn`/`egngpn`.
//!
//! ## Local-copy note (for the crate maintainer)
//!
//! GAMINR carries its **own** copy of the photon group-boundary tables inside
//! `genggp`. A sibling GROUPR port may add its own photon group structures under
//! `src/groupr/`. To keep this port faithful to `gaminr.f90` and avoid reaching
//! into a concurrently-edited sibling, the tables below are ported **locally**
//! from `gaminr.f90`. The lead may later dedupe GAMINR's and GROUPR's photon
//! tables into one shared source if they are proven identical.
//!
//! # Energy unit and ordering
//!
//! All boundaries are returned in **electron-volts (eV)**. In `gaminr.f90` the
//! `eg2`–`eg6` and `eg8` tables are written in **MeV** and multiplied by
//! `mev = 1.0e6` when copied into `egg` (`gaminr.f90:667,685,692,...`); the
//! `eg7` and `eg10` tables are already in eV and copied verbatim
//! (`gaminr.f90:720,742`). Every built-in structure here is **ascending in
//! energy** (lowest boundary first), matching the Fortran source tables. The
//! returned `Vec<f64>` has length `ngg + 1`.
//!
//! # `igg` index map (from `gaminr.f90:73-86, 590-602`)
//!
//! | `igg` | structure | groups |
//! |------:|-----------|-------:|
//! | 0  | none (empty) | 0 |
//! | 1  | arbitrary structure, **read from input** (not a table) | — |
//! | 2  | CSEWG | 94 |
//! | 3  | LANL | 12 |
//! | 4  | Steiner (ORNL-TM-2564) | 21 |
//! | 5  | Straker | 22 |
//! | 6  | LANL | 48 |
//! | 7  | LANL | 24 |
//! | 8  | VITAMIN-C | 36 |
//! | 9  | VITAMIN-E | 38 |
//! | 10 | VITAMIN-J | 42 |

use crate::NjoyError;

/// One MeV expressed in eV — the `mev` conversion factor (`gaminr.f90:667`).
const MEV: f64 = 1.0e6;

/// Built-in photon group-structure selector — the `igg` option of GAMINR
/// (`gaminr.f90:73-86, 585-602`).
///
/// Each variant names a predefined multigroup photon energy-boundary set, or
/// (for [`PhotonGroupStructure::None`] / [`PhotonGroupStructure::ArbitraryRead`])
/// a non-tabulated case. Use [`PhotonGroupStructure::from_igg`] to build one
/// from the raw card-2 integer and [`PhotonGroupStructure::boundaries`] to get
/// the eV boundary vector.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PhotonGroupStructure {
    /// `igg = 0`: no photon group structure (empty). GAMINR returns `ngg = 0`
    /// and does no group averaging (`gaminr.f90:670-672`).
    None,
    /// `igg = 1`: arbitrary structure, **read from the input deck** as `ngg`
    /// followed by `ngg+1` boundaries in eV (`gaminr.f90:675-679`). No built-in
    /// table — see [`crate::gaminr::input::GroupGrid`].
    ArbitraryRead,
    /// `igg = 2`: CSEWG 94-group structure (`gaminr.f90:682-686`).
    Csewg94,
    /// `igg = 3`: LANL 12-group structure (`gaminr.f90:689-693`).
    Lanl12,
    /// `igg = 4`: Steiner 21-group gamma-ray structure, ORNL-TM-2564
    /// (`gaminr.f90:696-700`).
    Steiner21,
    /// `igg = 5`: Straker 22-group structure (`gaminr.f90:703-707`).
    Straker22,
    /// `igg = 6`: LANL 48-group structure (`gaminr.f90:710-714`).
    Lanl48,
    /// `igg = 7`: LANL 24-group structure (`gaminr.f90:717-721`).
    Lanl24,
    /// `igg = 8`: VITAMIN-C 36-group structure (`gaminr.f90:724-736`). Derived
    /// from the 39-value VITAMIN-E table with boundaries `eg8(7)` (0.075 MeV)
    /// and `eg8(39)` (20 MeV) removed.
    VitaminC36,
    /// `igg = 9`: VITAMIN-E 38-group structure (`gaminr.f90:724-730`).
    VitaminE38,
    /// `igg = 10`: VITAMIN-J 42-group structure (`gaminr.f90:739-743`).
    VitaminJ42,
}

impl PhotonGroupStructure {
    /// Build a [`PhotonGroupStructure`] from the raw `igg` card-2 integer.
    ///
    /// Mirrors the `igg` dispatch in `genggp` (`gaminr.f90:672-748`). An `igg`
    /// outside `0..=10` is GAMINR's "illegal group structure" abort
    /// (`gaminr.f90:746-747`); here it returns [`NjoyError::EndfParse`] instead
    /// of aborting the process.
    pub fn from_igg(igg: i32) -> Result<Self, NjoyError> {
        Ok(match igg {
            0 => Self::None,
            1 => Self::ArbitraryRead,
            2 => Self::Csewg94,
            3 => Self::Lanl12,
            4 => Self::Steiner21,
            5 => Self::Straker22,
            6 => Self::Lanl48,
            7 => Self::Lanl24,
            8 => Self::VitaminC36,
            9 => Self::VitaminE38,
            10 => Self::VitaminJ42,
            other => {
                return Err(NjoyError::EndfParse(format!(
                    "gaminr genggp: illegal photon group structure igg={other} (valid 0..=10)"
                )));
            }
        })
    }

    /// The raw `igg` integer for this structure (inverse of
    /// [`PhotonGroupStructure::from_igg`]).
    pub fn igg(self) -> i32 {
        match self {
            Self::None => 0,
            Self::ArbitraryRead => 1,
            Self::Csewg94 => 2,
            Self::Lanl12 => 3,
            Self::Steiner21 => 4,
            Self::Straker22 => 5,
            Self::Lanl48 => 6,
            Self::Lanl24 => 7,
            Self::VitaminC36 => 8,
            Self::VitaminE38 => 9,
            Self::VitaminJ42 => 10,
        }
    }

    /// Number of energy groups `ngg` for this built-in structure, or `None` for
    /// the [`PhotonGroupStructure::None`] / [`PhotonGroupStructure::ArbitraryRead`]
    /// cases whose group count is either zero or read from the deck.
    ///
    /// The boundary vector has `ngg + 1` entries.
    pub fn group_count(self) -> Option<usize> {
        Some(match self {
            Self::None => return None,
            Self::ArbitraryRead => return None,
            Self::Csewg94 => 94,
            Self::Lanl12 => 12,
            Self::Steiner21 => 21,
            Self::Straker22 => 22,
            Self::Lanl48 => 48,
            Self::Lanl24 => 24,
            Self::VitaminC36 => 36,
            Self::VitaminE38 => 38,
            Self::VitaminJ42 => 42,
        })
    }

    /// Return the group-boundary vector in **eV**, ascending, length `ngg + 1`.
    ///
    /// Faithful port of the boundary construction in `genggp`
    /// (`gaminr.f90:670-748`).
    ///
    /// # Errors
    ///
    /// - [`PhotonGroupStructure::ArbitraryRead`] (`igg = 1`) has no built-in
    ///   table — its boundaries come from the input deck — so this returns
    ///   [`NjoyError::NotPorted`] (`gaminr.f90:675-679`).
    ///
    /// [`PhotonGroupStructure::None`] returns an empty structure (`ngg = 0`,
    /// `gaminr.f90:670-672`), represented here as the single zero boundary
    /// `egg(1)=0` that `genggp` leaves in place.
    pub fn boundaries(self) -> Result<Vec<f64>, NjoyError> {
        Ok(match self {
            // igg=0: none. ngg=0, egg(1)=0 (gaminr.f90:670-672).
            Self::None => vec![0.0],
            // igg=1: read from input deck (gaminr.f90:675-679).
            Self::ArbitraryRead => {
                return Err(NjoyError::NotPorted(
                    "gaminr genggp igg=1 arbitrary photon structure (read from input deck)",
                ));
            }
            // igg=2: csewg 94-group; eg2 in MeV (gaminr.f90:682-686).
            Self::Csewg94 => EG2.iter().map(|&e| e * MEV).collect(),
            // igg=3: lanl 12-group; eg3 in MeV (gaminr.f90:689-693).
            Self::Lanl12 => EG3.iter().map(|&e| e * MEV).collect(),
            // igg=4: steiner 21-group; eg4 in MeV (gaminr.f90:696-700).
            Self::Steiner21 => EG4.iter().map(|&e| e * MEV).collect(),
            // igg=5: straker 22-group; eg5 in MeV (gaminr.f90:703-707).
            Self::Straker22 => EG5.iter().map(|&e| e * MEV).collect(),
            // igg=6: lanl 48-group; eg6 in MeV (gaminr.f90:710-714).
            Self::Lanl48 => EG6.iter().map(|&e| e * MEV).collect(),
            // igg=7: lanl 24-group; eg7 already in eV (gaminr.f90:717-721).
            Self::Lanl24 => EG7.to_vec(),
            // igg=8: vitamin-c 36-group; eg8 in MeV with eg8(7) and eg8(39)
            // removed (gaminr.f90:724-736).
            Self::VitaminC36 => vitamin_c_36(),
            // igg=9: vitamin-e 38-group; eg8 in MeV, all 39 boundaries
            // (gaminr.f90:724-730).
            Self::VitaminE38 => EG8.iter().map(|&e| e * MEV).collect(),
            // igg=10: vitamin-j 42-group; eg10 already in eV (gaminr.f90:739-743).
            Self::VitaminJ42 => EG10.to_vec(),
        })
    }
}

/// Convenience free function mirroring the `genggp(igg) -> egg` call site.
///
/// Equivalent to `PhotonGroupStructure::from_igg(igg)?.boundaries()`. Returns
/// the eV boundary vector (length `ngg + 1`, ascending). See
/// [`PhotonGroupStructure::boundaries`] for the error cases.
///
/// # Source
///
/// `gaminr.f90:585-776` (`genggp`).
pub fn photon_group_structure(igg: i32) -> Result<Vec<f64>, NjoyError> {
    PhotonGroupStructure::from_igg(igg)?.boundaries()
}

/// Build the VITAMIN-C 36-group boundaries (`igg = 8`) from the VITAMIN-E table.
///
/// Faithful port of `gaminr.f90:724-736`: start from `eg8(1..=37)*mev`, then for
/// `ig` in `7..=37` overwrite `egg(ig) = eg8(ig+1)*mev`. The net effect is to
/// drop `eg8(7)` (0.075 MeV) and `eg8(39)` (20 MeV), leaving 37 boundaries for
/// 36 groups. Indices below are 0-based translations of the 1-based Fortran.
fn vitamin_c_36() -> Vec<f64> {
    // ngg=36, ngm=ngg+1=37 boundaries (gaminr.f90:725-727).
    let ngm = 37usize;
    let mut egg = vec![0.0f64; ngm];
    // First fill egg(1..=37) = eg8(1..=37)*mev (gaminr.f90:728-730).
    for ig in 1..=ngm {
        egg[ig - 1] = EG8[ig - 1] * MEV;
    }
    // Then remove eg8(7)/eg8(39): egg(ig)=eg8(ig+1)*mev for ig=7..=37
    // (gaminr.f90:731-736). eg8 has 39 entries so eg8(38) (ig=37) is valid.
    for ig in 7..=ngm {
        egg[ig - 1] = EG8[ig] * MEV; // eg8(ig+1) 1-based == EG8[ig] 0-based
    }
    egg
}

// ---------------------------------------------------------------------------
// Built-in boundary tables, verbatim from gaminr.f90 (energies as written:
// eg2-eg6 and eg8 in MeV, eg7 and eg10 in eV).
// ---------------------------------------------------------------------------

/// CSEWG 94-group boundaries in **MeV** — `eg2(95)` (`gaminr.f90:609-624`).
const EG2: [f64; 95] = [
    0.005, 0.01, 0.015, 0.02, 0.03, 0.035, 0.04, 0.045, 0.055, 0.06, 0.065, 0.075, 0.08, 0.09, 0.1,
    0.12, 0.14, 0.15, 0.16, 0.19, 0.22, 0.26, 0.3, 0.325, 0.35, 0.375, 0.4, 0.425, 0.45, 0.5,
    0.525, 0.55, 0.575, 0.6, 0.625, 0.65, 0.675, 0.7, 0.75, 0.8, 0.825, 0.865, 0.9, 1.0, 1.125,
    1.2, 1.25, 1.33, 1.42, 1.5, 1.6, 1.66, 1.75, 1.875, 2.0, 2.166, 2.333, 2.5, 2.666, 2.833, 3.0,
    3.166, 3.333, 3.5, 3.65, 3.8, 3.9, 4.0, 4.2, 4.4, 4.5, 4.7, 5.0, 5.2, 5.4, 5.5, 5.75, 6.0,
    6.25, 6.5, 6.75, 7.0, 7.25, 7.5, 7.75, 8.0, 8.5, 9.0, 9.5, 10.0, 10.6, 11.0, 12.0, 14.0, 20.0,
];

/// LANL 12-group boundaries in **MeV** — `eg3(13)` (`gaminr.f90:625-627`).
const EG3: [f64; 13] = [
    0.01, 0.10, 0.50, 1.0, 2.0, 3.0, 4.0, 5.0, 6.0, 7.0, 8.0, 9.0, 20.0,
];

/// Steiner 21-group boundaries in **MeV** — `eg4(22)` (`gaminr.f90:628-632`).
const EG4: [f64; 22] = [
    0.01, 0.1, 0.2, 0.4, 1.0, 1.5, 2.0, 2.5, 3.0, 3.5, 4.0, 4.5, 5.0, 5.5, 6.0, 6.5, 7.0, 7.5, 8.0,
    10.0, 12.0, 14.0,
];

/// Straker 22-group boundaries in **MeV** — `eg5(23)` (`gaminr.f90:633-637`).
const EG5: [f64; 23] = [
    0.01, 0.03, 0.06, 0.10, 0.15, 0.30, 0.45, 0.60, 0.80, 1.0, 1.33, 1.66, 2.0, 2.5, 3.0, 3.5, 4.0,
    5.0, 6.0, 7.0, 8.0, 10.0, 14.0,
];

/// LANL 48-group boundaries in **MeV** — `eg6(49)` (`gaminr.f90:638-646`).
const EG6: [f64; 49] = [
    0.001, 0.01, 0.02, 0.03, 0.045, 0.06, 0.08, 0.1, 0.15, 0.2, 0.3, 0.4, 0.45, 0.5, 0.525, 0.6,
    0.7, 0.8, 0.9, 1.0, 1.125, 1.2, 1.33, 1.5, 1.66, 1.875, 2.0, 2.333, 2.5, 2.666, 3.0, 3.5, 4.0,
    4.5, 5.0, 5.5, 6.0, 6.5, 7.0, 7.5, 8.0, 9.0, 10.0, 12.0, 14.0, 17.0, 20.0, 30.0, 50.0,
];

/// LANL 24-group boundaries in **eV** — `eg7(25)` (`gaminr.f90:647-651`).
const EG7: [f64; 25] = [
    1.0e4, 3.0e4, 6.0e4, 1.0e5, 2.0e5, 3.0e5, 5.0e5, 5.25e5, 7.5e5, 1.0e6, 1.33e6, 1.66e6, 2.0e6,
    2.5e6, 3.0e6, 4.0e6, 5.0e6, 6.0e6, 7.0e6, 8.0e6, 9.0e6, 1.0e7, 1.2e7, 1.7e7, 3.0e7,
];

/// VITAMIN-E 38-group boundaries in **MeV** — `eg8(39)` (`gaminr.f90:652-658`).
///
/// Also the source table for the VITAMIN-C 36-group structure (`igg = 8`),
/// which removes `eg8(7)` and `eg8(39)`; see [`vitamin_c_36`].
const EG8: [f64; 39] = [
    0.01, 0.02, 0.03, 0.045, 0.06, 0.07, 0.075, 0.10, 0.15, 0.20, 0.30, 0.40, 0.45, 0.510, 0.512,
    0.60, 0.70, 0.80, 1.0, 1.33, 1.50, 1.66, 2.0, 2.5, 3.0, 3.5, 4.0, 4.5, 5.0, 5.5, 6.0, 6.5, 7.0,
    7.5, 8.0, 10.0, 12.0, 14.0, 20.0,
];

/// VITAMIN-J 42-group boundaries in **eV** — `eg10(43)` (`gaminr.f90:659-666`).
const EG10: [f64; 43] = [
    1.0e3, 1.0e4, 2.0e4, 3.0e4, 4.5e4, 6.0e4, 7.0e4, 7.5e4, 1.0e5, 1.50e5, 2.00e5, 3.00e5, 4.00e5,
    4.50e5, 5.10e5, 5.12e5, 6.00e5, 7.00e5, 8.00e5, 1.00e6, 1.33e6, 1.34e6, 1.50e6, 1.66e6, 2.00e6,
    2.50e6, 3.00e6, 3.50e6, 4.00e6, 4.50e6, 5.00e6, 5.50e6, 6.00e6, 6.50e6, 7.00e6, 7.50e6, 8.00e6,
    1.00e7, 1.20e7, 1.40e7, 2.00e7, 3.00e7, 5.00e7,
];

#[cfg(test)]
mod tests {
    use super::*;

    /// Methodology: for every built-in `igg`, the returned boundary vector must
    /// have exactly `ngg + 1` entries (the GAMINR invariant, `gaminr.f90:677`)
    /// with `ngg` taken from `genggp`'s `ngg=` assignments
    /// (`gaminr.f90:683,690,697,704,711,718,725-726,740`).
    ///
    /// Results (verified 2026-07-15 against gaminr.f90 DATA statements):
    /// igg 2->95, 3->13, 4->22, 5->23, 6->49, 7->25, 8->37, 9->39, 10->43 bounds.
    #[test]
    fn group_counts_match_fortran() {
        let cases = [
            (2, 94usize),
            (3, 12),
            (4, 21),
            (5, 22),
            (6, 48),
            (7, 24),
            (8, 36),
            (9, 38),
            (10, 42),
        ];
        for (igg, ngg) in cases {
            let s = PhotonGroupStructure::from_igg(igg).unwrap();
            assert_eq!(s.group_count(), Some(ngg), "igg={igg} ngg");
            let egg = s.boundaries().unwrap();
            assert_eq!(egg.len(), ngg + 1, "igg={igg} boundary count");
        }
    }

    /// Methodology: every built-in structure is defined ascending in `gaminr.f90`
    /// and the port preserves that order; assert strict monotone increase.
    /// Result (2026-07-15): all 9 built-in structures strictly ascending.
    #[test]
    fn boundaries_strictly_ascending() {
        for igg in 2..=10 {
            let egg = PhotonGroupStructure::from_igg(igg)
                .unwrap()
                .boundaries()
                .unwrap();
            for w in egg.windows(2) {
                assert!(w[1] > w[0], "igg={igg}: {} !> {}", w[1], w[0]);
            }
        }
    }

    /// Methodology: check exact endpoint values against the Fortran DATA tables,
    /// applying the eV conversion (MeV tables scaled by 1e6; eV tables verbatim).
    ///
    /// Results (2026-07-15, from gaminr.f90:609-666):
    /// - CSEWG (igg=2): 0.005 MeV -> 5.0e3 eV .. 20 MeV -> 2.0e7 eV.
    /// - LANL-24 (igg=7): 1.0e4 eV .. 3.0e7 eV (verbatim eV).
    /// - VITAMIN-J (igg=10): 1.0e3 eV .. 5.0e7 eV (verbatim eV).
    #[test]
    fn endpoints_match_fortran() {
        let csewg = PhotonGroupStructure::Csewg94.boundaries().unwrap();
        assert_eq!(csewg[0], 5.0e3);
        assert_eq!(*csewg.last().unwrap(), 2.0e7);

        let lanl24 = PhotonGroupStructure::Lanl24.boundaries().unwrap();
        assert_eq!(lanl24[0], 1.0e4);
        assert_eq!(*lanl24.last().unwrap(), 3.0e7);

        let vitj = PhotonGroupStructure::VitaminJ42.boundaries().unwrap();
        assert_eq!(vitj[0], 1.0e3);
        assert_eq!(*vitj.last().unwrap(), 5.0e7);
    }

    /// Methodology: VITAMIN-C (igg=8) is VITAMIN-E (igg=9) with `eg8(7)`
    /// (0.075 MeV) and `eg8(39)` (20 MeV) removed (`gaminr.f90:731-736`).
    /// Verify the two dropped values are absent from igg=8 but present in igg=9,
    /// and that igg=8 shares igg=9's first 6 boundaries.
    ///
    /// Result (2026-07-15): igg=8 has 37 bounds; 7.5e4 eV and 2.0e7 eV present
    /// in igg=9, absent in igg=8; first 6 boundaries identical.
    #[test]
    fn vitamin_c_drops_two_bounds() {
        let vc = PhotonGroupStructure::VitaminC36.boundaries().unwrap();
        let ve = PhotonGroupStructure::VitaminE38.boundaries().unwrap();
        assert_eq!(vc.len(), 37);
        assert_eq!(ve.len(), 39);
        // eg8(7)=0.075 MeV = 7.5e4 eV; eg8(39)=20 MeV = 2.0e7 eV.
        assert!(ve.contains(&7.5e4) && ve.contains(&2.0e7));
        assert!(!vc.contains(&7.5e4), "0.075 MeV should be removed in igg=8");
        assert!(!vc.contains(&2.0e7), "20 MeV should be removed in igg=8");
        // First six boundaries shared (eg8(1..=6)).
        assert_eq!(&vc[..6], &ve[..6]);
    }

    /// Methodology: igg=0 gives an empty structure (ngg=0, single zero bound);
    /// igg=1 is read-from-input (NotPorted); out-of-range igg is an error.
    /// Result (2026-07-15): matches `genggp` control flow (gaminr.f90:670-747).
    #[test]
    fn none_arbitrary_and_illegal() {
        assert_eq!(PhotonGroupStructure::None.boundaries().unwrap(), vec![0.0]);
        assert!(matches!(
            PhotonGroupStructure::ArbitraryRead.boundaries(),
            Err(NjoyError::NotPorted(_))
        ));
        assert!(PhotonGroupStructure::from_igg(11).is_err());
        assert!(PhotonGroupStructure::from_igg(-1).is_err());
    }
}
