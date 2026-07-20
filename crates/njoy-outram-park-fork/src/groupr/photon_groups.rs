// Ported from NJOY2016 `src/groupr.f90` (git commit ac5adf5f33d893e42f2eed7fb286b0d51c7580da).
// NJOY2016 is under a modified BSD 3-Clause (LANL/DOE) licence, GPL-compatible;
// this derivative file is distributed under GPL-3.0-only. This is a modified,
// non-LANL version, not endorsed by LANL/DOE. See crate root LICENSE.njoy + NOTICE.

//! Built-in **photon (gamma) group structures** for the GROUPR module.
//!
//! # What this module computes
//!
//! GROUPR's `gengpg(igg,ngg,egg)` (`groupr.f90:4651-4863`) selects one of NJOY's
//! built-in multigroup **gamma-ray** energy-boundary structures by the integer
//! index `igg`, filling the group-boundary array `egg` (length `ngg+1`, ascending
//! in energy) and returning the group count `ngg`. This module ports every
//! built-in table faithfully. It is the photon counterpart of the *neutron*
//! group structures, which live in [`crate::errorr::groups`] (`gengpn`) — the
//! two are independent tables and are **not** duplicated here.
//!
//! # Energy unit and ordering
//!
//! All boundaries are in **electron-volts (eV)** and returned ascending (lowest
//! boundary first). The upstream `parameter` tables are written in **MeV** for
//! `igg = 2..6, 8, 9` (GROUPR multiplies them by `emev = 1.0e6`) and already in
//! **eV** for `igg = 7` (LANL 24-group) and `igg = 10` (VITAMIN-J 42-group);
//! this port applies the same scaling so every returned value is in eV.
//!
//! # `igg` index map (from `groupr.f90:4655-4670`)
//!
//! | `igg` | structure | groups |
//! |------:|-----------|-------:|
//! | 0  | none (no photon groups) | 0 |
//! | 1  | arbitrary structure, **read from input** (not a table) | — |
//! | 2  | CSEWG 94-group | 94 |
//! | 3  | LANL 12-group | 12 |
//! | 4  | Steiner 21-group (ORNL-TM-2564) | 21 |
//! | 5  | Straker 22-group | 22 |
//! | 6  | LANL 48-group | 48 |
//! | 7  | LANL 24-group | 24 |
//! | 8  | VITAMIN-C 36-group | 36 |
//! | 9  | VITAMIN-E 38-group | 38 |
//! | 10 | VITAMIN-J 42-group | 42 |
//!
//! # Not ported: read-from-input structure
//!
//! `igg == 1` is the *arbitrary* structure: NJOY reads `ngg` and then the
//! `ngg+1` boundaries from the system input file as a free-format list
//! (`groupr.f90:4745-4752`). There is no hardcoded table, so
//! [`PhotonGroupStructure::boundaries`] returns [`NjoyError::NotPorted`] for it
//! rather than fabricating boundaries. Any unrecognised `igg` (NJOY's "illegal
//! group structure" abort at `groupr.f90:4838-4839`) is reported as an error.
//!
//! # VITAMIN-C vs VITAMIN-E (the shared `eg8` table)
//!
//! Upstream stores a single 39-value `eg8` table (`groupr.f90:4721-4728`) and
//! derives *both* the VITAMIN-E 38-group (`igg = 9`, all 39 boundaries) and the
//! VITAMIN-C 36-group (`igg = 8`, 37 boundaries) from it. For VITAMIN-C the
//! Fortran removes `eg8(7) = 0.075 MeV` and drops the final `eg8(39) = 20 MeV`
//! (`groupr.f90:4786-4798`). This port reproduces that splice exactly.

use crate::NjoyError;

/// A conversion factor spelled out for human readers: 1 MeV = 1.0e6 eV.
///
/// Fortran calls this `emev` (`groupr.f90:4730`). The MeV-valued built-in
/// tables are multiplied by it to yield eV.
const MEV_IN_EV: f64 = 1.0e6;

/// Named handle for a built-in photon (gamma) group structure.
///
/// Each variant maps one-to-one to NJOY's integer `igg` index. Use
/// [`PhotonGroupStructure::boundaries`] to obtain the group-boundary energies in
/// eV (ascending, length = groups + 1).
///
/// The [`PhotonGroupStructure::None`] variant (`igg = 0`) means "no photon
/// groups" (a neutron-only GROUPR run); its boundary list is empty. The
/// [`PhotonGroupStructure::Arbitrary`] variant (`igg = 1`) is read from the
/// input deck and has no built-in table.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PhotonGroupStructure {
    /// `igg = 0`: no photon group structure (neutron-only run). Zero groups.
    None,
    /// `igg = 1`: arbitrary structure, read from the input deck. No built-in
    /// table — [`PhotonGroupStructure::boundaries`] returns `NotPorted`.
    Arbitrary,
    /// `igg = 2`: CSEWG 94-group gamma structure.
    Csewg94,
    /// `igg = 3`: LANL 12-group gamma structure.
    Lanl12,
    /// `igg = 4`: Steiner 21-group gamma structure (ORNL-TM-2564).
    Steiner21,
    /// `igg = 5`: Straker 22-group gamma structure.
    Straker22,
    /// `igg = 6`: LANL 48-group gamma structure.
    Lanl48,
    /// `igg = 7`: LANL 24-group gamma structure (table already in eV).
    Lanl24,
    /// `igg = 8`: VITAMIN-C 36-group gamma structure (derived from `eg8`).
    VitaminC36,
    /// `igg = 9`: VITAMIN-E 38-group gamma structure (all of `eg8`).
    VitaminE38,
    /// `igg = 10`: VITAMIN-J 42-group gamma structure (table already in eV).
    VitaminJ42,
}

impl PhotonGroupStructure {
    /// The integer `igg` index NJOY uses for this structure
    /// (`groupr.f90:4655-4670`).
    pub fn igg(self) -> i32 {
        match self {
            Self::None => 0,
            Self::Arbitrary => 1,
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

    /// Construct from the integer `igg` index. Returns `None` for any value
    /// outside `0..=10` (NJOY aborts with "illegal group structure").
    pub fn from_igg(igg: i32) -> Option<Self> {
        Some(match igg {
            0 => Self::None,
            1 => Self::Arbitrary,
            2 => Self::Csewg94,
            3 => Self::Lanl12,
            4 => Self::Steiner21,
            5 => Self::Straker22,
            6 => Self::Lanl48,
            7 => Self::Lanl24,
            8 => Self::VitaminC36,
            9 => Self::VitaminE38,
            10 => Self::VitaminJ42,
            _ => return None,
        })
    }

    /// The number of gamma groups this structure defines, if known.
    ///
    /// `None`/`Arbitrary` return [`Option::None`] (zero groups, or a count that
    /// is only known once the input deck is read, respectively).
    pub fn group_count(self) -> Option<usize> {
        Some(match self {
            Self::None => 0,
            Self::Arbitrary => return Option::None,
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

    /// Whether this selector reads its boundaries from the input deck
    /// (`igg == 1`). Mirrors the `else if (igg.eq.1)` branch at
    /// `groupr.f90:4744`.
    pub fn reads_boundaries(self) -> bool {
        matches!(self, Self::Arbitrary)
    }

    /// Group-boundary energies in **eV**, ascending, length = groups + 1.
    ///
    /// - [`PhotonGroupStructure::None`] returns an empty vector (no photon
    ///   groups; upstream sets `ngg = 0`, `egg(1) = 0`).
    /// - [`PhotonGroupStructure::Arbitrary`] returns [`NjoyError::NotPorted`]
    ///   (`"groupr::gengpg::arbitrary"`) because the boundaries come from input.
    /// - Every other variant returns the built-in table.
    pub fn boundaries(self) -> Result<Vec<f64>, NjoyError> {
        photon_group_structure(self.igg())
    }
}

/// Return the built-in photon group boundaries in **eV** for NJOY index `igg`.
///
/// This is the free-function form of [`PhotonGroupStructure::boundaries`] and is
/// the single source of truth for the tables. It reproduces `gengpg`
/// (`groupr.f90:4651-4863`) exactly, including the MeV-to-eV scaling and the
/// VITAMIN-C `eg8` splice.
///
/// # Parameters
///
/// - `igg`: the NJOY gamma-group-structure index (`0..=10`).
///
/// # Returns
///
/// The `ngg + 1` boundary energies in eV, ascending. `igg = 0` yields an empty
/// vector; `igg = 1` (arbitrary/read-in) yields [`NjoyError::NotPorted`]; any
/// other value yields [`NjoyError::EndfParse`] ("illegal gamma group
/// structure").
pub fn photon_group_structure(igg: i32) -> Result<Vec<f64>, NjoyError> {
    match igg {
        0 => Ok(Vec::new()),
        1 => Err(NjoyError::NotPorted("groupr::gengpg::arbitrary")),
        2 => Ok(scale_mev(&EG2)),
        3 => Ok(scale_mev(&EG3)),
        4 => Ok(scale_mev(&EG4)),
        5 => Ok(scale_mev(&EG5)),
        6 => Ok(scale_mev(&EG6)),
        7 => Ok(EG7.to_vec()), // already in eV
        8 => Ok(vitamin_c_36()),
        9 => Ok(scale_mev(&EG8)),
        10 => Ok(EG10.to_vec()), // already in eV
        _ => Err(NjoyError::EndfParse(format!(
            "illegal gamma group structure igg={igg} (gengpg, groupr.f90:4838)"
        ))),
    }
}

/// Multiply an MeV-valued table by 1.0e6 to yield eV (the `egg(ig)*emev` loop).
fn scale_mev(table: &[f64]) -> Vec<f64> {
    table.iter().map(|e| e * MEV_IN_EV).collect()
}

/// Build the VITAMIN-C 36-group boundaries (`igg = 8`) from the shared `eg8`
/// table, in eV.
///
/// Reproduces `groupr.f90:4786-4799`: fill `egg(1..=37)` from `eg8(1..=37)`,
/// then for `ig = 7..=37` overwrite `egg(ig) = eg8(ig+1)`. The net effect is
/// `eg8` with element 7 (`0.075 MeV`) removed and the final element 39
/// (`20 MeV`) dropped, giving 37 boundaries (36 groups).
fn vitamin_c_36() -> Vec<f64> {
    // ngp = 37 boundaries.
    let mut egg = vec![0.0_f64; 37];
    // First loop: egg(1..=37) = eg8(1..=37).
    for (i, slot) in egg.iter_mut().enumerate() {
        *slot = EG8[i] * MEV_IN_EV;
    }
    // Second loop: for ig = 7..=37 (1-based), egg(ig) = eg8(ig+1).
    // 1-based ig 7..=37 -> 0-based index 6..=36; source eg8(ig+1) -> index ig.
    for ig0 in 6..37 {
        egg[ig0] = EG8[ig0 + 1] * MEV_IN_EV;
    }
    egg
}

// ---------------------------------------------------------------------------
// Built-in boundary tables, transcribed mechanically from the Fortran
// `parameter` arrays in gengpg (groupr.f90:4677-4737). Each Rust `const` keeps
// the Fortran name (upper-cased) and length. `EG2..EG6, EG8` are in MeV;
// `EG7, EG10` are in eV. Extraction verified by matching every array length to
// its Fortran `dimension(N)`.
// ---------------------------------------------------------------------------

/// CSEWG 94-group (`igg = 2`), MeV. Fortran `eg2`, `dimension(95)`
/// (`groupr.f90:4677-4691`).
const EG2: [f64; 95] = [
    0.005, 0.01, 0.015, 0.02, 0.03, 0.035, 0.04, 0.045, 0.055, 0.06, 0.065, 0.075, 0.08, 0.09,
    0.1, 0.12, 0.14, 0.15, 0.16, 0.19, 0.22, 0.26, 0.3, 0.325, 0.35, 0.375, 0.4, 0.425, 0.45,
    0.5, 0.525, 0.55, 0.575, 0.6, 0.625, 0.65, 0.675, 0.7, 0.75, 0.8, 0.825, 0.865, 0.9, 1.0,
    1.125, 1.2, 1.25, 1.33, 1.42, 1.5, 1.6, 1.66, 1.75, 1.875, 2.0, 2.166, 2.333, 2.5, 2.666,
    2.833, 3.0, 3.166, 3.333, 3.5, 3.65, 3.8, 3.9, 4.0, 4.2, 4.4, 4.5, 4.7, 5.0, 5.2, 5.4, 5.5,
    5.75, 6.0, 6.25, 6.5, 6.75, 7.0, 7.25, 7.5, 7.75, 8.0, 8.5, 9.0, 9.5, 10.0, 10.6, 11.0,
    12.0, 14.0, 20.0,
];

/// LANL 12-group (`igg = 3`), MeV. Fortran `eg3`, `dimension(13)`
/// (`groupr.f90:4692-4694`).
const EG3: [f64; 13] = [
    0.01, 0.10, 0.50, 1.0, 2.0, 3.0, 4.0, 5.0, 6.0, 7.0, 8.0, 9.0, 20.0,
];

/// Steiner 21-group (`igg = 4`), MeV. Fortran `eg4`, `dimension(22)`
/// (`groupr.f90:4695-4699`).
const EG4: [f64; 22] = [
    0.01, 0.1, 0.2, 0.4, 1.0, 1.5, 2.0, 2.5, 3.0, 3.5, 4.0, 4.5, 5.0, 5.5, 6.0, 6.5, 7.0, 7.5,
    8.0, 10.0, 12.0, 14.0,
];

/// Straker 22-group (`igg = 5`), MeV. Fortran `eg5`, `dimension(23)`
/// (`groupr.f90:4700-4705`).
const EG5: [f64; 23] = [
    0.01, 0.03, 0.06, 0.10, 0.15, 0.30, 0.45, 0.60, 0.80, 1.0, 1.33, 1.66, 2.0, 2.5, 3.0, 3.5,
    4.0, 5.0, 6.0, 7.0, 8.0, 10.0, 14.0,
];

/// LANL 48-group (`igg = 6`), MeV. Fortran `eg6`, `dimension(49)`
/// (`groupr.f90:4706-4715`).
const EG6: [f64; 49] = [
    0.001, 0.01, 0.02, 0.03, 0.045, 0.06, 0.08, 0.1, 0.15, 0.2, 0.3, 0.4, 0.45, 0.5, 0.525, 0.6,
    0.7, 0.8, 0.9, 1.0, 1.125, 1.2, 1.33, 1.5, 1.66, 1.875, 2.0, 2.333, 2.5, 2.666, 3.0, 3.5,
    4.0, 4.5, 5.0, 5.5, 6.0, 6.5, 7.0, 7.5, 8.0, 9.0, 10.0, 12.0, 14.0, 17.0, 20.0, 30.0, 50.0,
];

/// LANL 24-group (`igg = 7`), **eV** (not scaled). Fortran `eg7`,
/// `dimension(25)` (`groupr.f90:4716-4720`).
const EG7: [f64; 25] = [
    1.0e4, 3.0e4, 6.0e4, 1.0e5, 2.0e5, 3.0e5, 5.0e5, 5.25e5, 7.5e5, 1.0e6, 1.33e6, 1.66e6, 2.0e6,
    2.5e6, 3.0e6, 4.0e6, 5.0e6, 6.0e6, 7.0e6, 8.0e6, 9.0e6, 1.0e7, 1.2e7, 1.7e7, 3.0e7,
];

/// Shared VITAMIN-C/-E `eg8` table (`igg = 8, 9`), MeV. Fortran `eg8`,
/// `dimension(39)` (`groupr.f90:4721-4728`). VITAMIN-E 38-group uses all 39
/// boundaries; VITAMIN-C 36-group splices it (see [`vitamin_c_36`]).
const EG8: [f64; 39] = [
    0.01, 0.02, 0.03, 0.045, 0.06, 0.07, 0.075, 0.10, 0.15, 0.20, 0.30, 0.40, 0.45, 0.51, 0.512,
    0.60, 0.70, 0.80, 1.0, 1.33, 1.5, 1.66, 2.0, 2.5, 3.0, 3.5, 4.0, 4.5, 5.0, 5.5, 6.0, 6.5,
    7.0, 7.5, 8.0, 10.0, 12.0, 14.0, 20.0,
];

/// VITAMIN-J 42-group (`igg = 10`), **eV** (not scaled). Fortran `eg10`,
/// `dimension(43)` (`groupr.f90:4729-4737`).
const EG10: [f64; 43] = [
    1.0e3, 1.0e4, 2.0e4, 3.0e4, 4.5e4, 6.0e4, 7.0e4, 7.5e4, 1.0e5, 1.50e5, 2.00e5, 3.00e5,
    4.00e5, 4.50e5, 5.10e5, 5.12e5, 6.00e5, 7.00e5, 8.00e5, 1.00e6, 1.33e6, 1.34e6, 1.50e6,
    1.66e6, 2.00e6, 2.50e6, 3.00e6, 3.50e6, 4.00e6, 4.50e6, 5.00e6, 5.50e6, 6.00e6, 6.50e6,
    7.00e6, 7.50e6, 8.00e6, 1.00e7, 1.20e7, 1.40e7, 2.00e7, 3.00e7, 5.00e7,
];

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    /// The `igg` <-> enum mapping round-trips over the full legal range and
    /// rejects out-of-range indices.
    ///
    /// **Methodology.** For `igg = 0..=10`, assert `from_igg(x).igg() == x`;
    /// assert `from_igg` returns `None` for `-1` and `11`. Mirrors the `igg`
    /// dispatch block at `groupr.f90:4746-4837`.
    ///
    /// **Result (2026-07-15, commit ac5adf5).** All 11 indices round-trip;
    /// `-1` and `11` reject.
    #[test]
    fn igg_round_trips() {
        for igg in 0..=10 {
            let s = PhotonGroupStructure::from_igg(igg).expect("valid igg");
            assert_eq!(s.igg(), igg);
        }
        assert!(PhotonGroupStructure::from_igg(-1).is_none());
        assert!(PhotonGroupStructure::from_igg(11).is_none());
    }

    /// Every built-in structure returns the documented group count, strictly
    /// ascending boundaries, and the correct endpoint energies.
    ///
    /// **Methodology.** For each `igg` in `2..=10`, fetch the boundaries and
    /// check: (a) length == `group_count() + 1`; (b) strictly increasing in eV;
    /// (c) first and last boundary equal the Fortran table endpoints scaled to
    /// eV. Endpoints are read directly from the `eg*` `parameter` arrays at
    /// `groupr.f90:4677-4737`.
    ///
    /// **Result (2026-07-15, commit ac5adf5).** All eight built-in structures
    /// pass. Group counts {94,12,21,22,48,24,36,38,42} match the header table;
    /// endpoints match (e.g. CSEWG94 spans 5.0e3-2.0e7 eV; LANL24 spans
    /// 1.0e4-3.0e7 eV; VITAMIN-J42 spans 1.0e3-5.0e7 eV).
    #[test]
    fn builtin_structures_counts_ordering_endpoints() {
        // (igg, groups, first_ev, last_ev)
        let cases = [
            (2, 94, 0.005 * MEV_IN_EV, 20.0 * MEV_IN_EV),
            (3, 12, 0.01 * MEV_IN_EV, 20.0 * MEV_IN_EV),
            (4, 21, 0.01 * MEV_IN_EV, 14.0 * MEV_IN_EV),
            (5, 22, 0.01 * MEV_IN_EV, 14.0 * MEV_IN_EV),
            (6, 48, 0.001 * MEV_IN_EV, 50.0 * MEV_IN_EV),
            (7, 24, 1.0e4, 3.0e7),
            (8, 36, 0.01 * MEV_IN_EV, 14.0 * MEV_IN_EV),
            (9, 38, 0.01 * MEV_IN_EV, 20.0 * MEV_IN_EV),
            (10, 42, 1.0e3, 5.0e7),
        ];
        for (igg, groups, first, last) in cases {
            let s = PhotonGroupStructure::from_igg(igg).unwrap();
            let b = s.boundaries().expect("built-in table");
            assert_eq!(b.len(), groups + 1, "igg={igg} boundary count");
            assert_eq!(s.group_count(), Some(groups), "igg={igg} group_count");
            for w in b.windows(2) {
                assert!(w[1] > w[0], "igg={igg} not strictly ascending: {w:?}");
            }
            assert!((b[0] - first).abs() < 1e-3, "igg={igg} first boundary");
            assert!(
                (b[b.len() - 1] - last).abs() < 1.0,
                "igg={igg} last boundary"
            );
        }
    }

    /// `igg = 0` yields zero groups; `igg = 1` is not ported (read from input).
    ///
    /// **Methodology.** Assert `None` -> empty vector (upstream `ngg = 0`);
    /// assert `Arbitrary` -> `NotPorted("groupr::gengpg::arbitrary")`, matching
    /// the read-in branch at `groupr.f90:4744-4752`.
    ///
    /// **Result (2026-07-15).** `None` -> `[]`; `Arbitrary` -> `NotPorted`.
    #[test]
    fn none_and_arbitrary_behaviour() {
        assert_eq!(
            PhotonGroupStructure::None.boundaries().unwrap(),
            Vec::<f64>::new()
        );
        match PhotonGroupStructure::Arbitrary.boundaries() {
            Err(NjoyError::NotPorted(tag)) => {
                assert_eq!(tag, "groupr::gengpg::arbitrary");
            }
            other => panic!("expected NotPorted, got {other:?}"),
        }
    }

    /// The VITAMIN-C 36-group splice removes `eg8(7)` and drops `eg8(39)`.
    ///
    /// **Methodology.** VITAMIN-C (`igg = 8`) must equal `eg8` (scaled to eV)
    /// with element index 7 (`0.075 MeV`) removed and the last element
    /// (`20 MeV`) dropped, per `groupr.f90:4786-4798`. Assert the exact 37-value
    /// sequence, and that `0.075 MeV = 75_000 eV` is absent while it *is*
    /// present in VITAMIN-E (`igg = 9`).
    ///
    /// **Result (2026-07-15, commit ac5adf5).** VITAMIN-C has 37 boundaries
    /// spanning 1.0e4-1.4e7 eV (last = `eg8(38) = 14 MeV`); `75_000 eV` absent.
    /// VITAMIN-E has 39 boundaries spanning 1.0e4-2.0e7 eV; `75_000 eV` present.
    /// Splice reproduced exactly.
    #[test]
    fn vitamin_c_splice_matches_fortran() {
        let vc = PhotonGroupStructure::VitaminC36.boundaries().unwrap();
        let ve = PhotonGroupStructure::VitaminE38.boundaries().unwrap();

        // Expected VITAMIN-C = eg8 without index 6 (0-based; 0.075) and without
        // the final 20.0, in eV.
        let mut expected: Vec<f64> = EG8.to_vec();
        expected.remove(6); // 0.075 MeV
        expected.pop(); // 20.0 MeV
        let expected: Vec<f64> = expected.iter().map(|e| e * MEV_IN_EV).collect();

        assert_eq!(vc.len(), 37);
        assert_eq!(vc, expected);

        let seventy_five_k = 0.075 * MEV_IN_EV;
        assert!(!vc.iter().any(|&e| (e - seventy_five_k).abs() < 1e-6));
        assert!(ve.iter().any(|&e| (e - seventy_five_k).abs() < 1e-6));
        assert_eq!(vc[vc.len() - 1], 14.0 * MEV_IN_EV);
        assert_eq!(ve[ve.len() - 1], 20.0 * MEV_IN_EV);
    }
}
