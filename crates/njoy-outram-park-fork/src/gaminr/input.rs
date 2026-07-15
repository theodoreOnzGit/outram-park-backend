// Ported from NJOY2016 `src/gaminr.f90` (git commit ac5adf5f33d893e42f2eed7fb286b0d51c7580da).
// NJOY2016 is under a modified BSD 3-Clause (LANL/DOE) licence, GPL-compatible;
// this derivative file is distributed under GPL-3.0-only. This is a modified,
// non-LANL version, not endorsed by LANL/DOE. See crate root LICENSE.njoy + NOTICE.

//! GAMINR user-input card model.
//!
//! Faithful, line-traceable translation of GAMINR's free-format **input-card
//! deck** (`gaminr.f90:37-92` card summary; read by `ruing`/`genggp`/`gnwtf`,
//! `gaminr.f90:538-823`). It models the card sequence that configures a GAMINR
//! run — unit numbers, material, photon group-structure / weight selectors,
//! Legendre order, run title, an optional read-in group grid, and the reaction
//! list — as a documented Rust struct ([`GaminrInput`]) plus selector enums.
//!
//! Field names keep the upstream Fortran identifiers in their doc comments so a
//! discrepancy can be localised to a specific `read(nsysi,*)` statement. This
//! mirrors the ERRORR `src/errorr/driver.rs` card-deck pattern.
//!
//! # Card summary (from `gaminr.f90:37-92`)
//!
//! | Card | Fortran vars | Condition |
//! |------|--------------|-----------|
//! | 1 | `nendf npend ngam1 ngam2` | always (`gaminr.f90:551`) |
//! | 2 | `matb igg iwt lord iprint` | always (`gaminr.f90:557`) |
//! | 3 | `title` (up to 80 chars) | always (`gaminr.f90:558-559`) |
//! | 4 | `ngg` then `egg(ngg+1)` | `igg==1` only (`gaminr.f90:676-679`) |
//! | 5 | `wght` (TAB1) | `iwt==1` only (`gaminr.f90:805`) |
//! | 6 | `mfd mtd mtname` (repeat) | always; `mfd=0` ends material, `mfd=-1` = all |
//! | 7 | `matd` (next material; `0` ends run) | always (`gaminr.f90:516`) |

use crate::gaminr::photon_groups::PhotonGroupStructure;
use crate::gaminr::weights::WeightOption;
use crate::NjoyError;

// ---------------------------------------------------------------------------
// Card 1 — unit-number assignments (gaminr.f90:39-43, read at :551)
// ---------------------------------------------------------------------------

/// Card 1: logical unit numbers for the ENDF/PENDF tapes GAMINR reads and the
/// GAM-out tapes it reads/writes.
///
/// Fortran: `read(nsysi,*) nendf,npend,ngam1,ngam2` (`gaminr.f90:551`). A
/// negative unit number denotes a binary (unformatted) tape in NJOY; a positive
/// number denotes a formatted (ASCII) tape; zero means "absent". These are
/// transport-layer handles, not physical quantities — no units.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct UnitAssignments {
    /// `nendf` — unit for the input ENDF tape (photoatomic evaluation; the
    /// source of MF=27 form factors / incoherent scattering functions).
    pub nendf: i32,
    /// `npend` — unit for the input PENDF tape (pointwise photon cross sections,
    /// MF=23, produced upstream by RECONR).
    pub npend: i32,
    /// `ngam1` — unit for an input GAM-out tape (for adding a new material to an
    /// existing library). Default 0 (`gaminr.f90:549`).
    pub ngam1: i32,
    /// `ngam2` — unit for the output GAM-out (GENDF-like) tape. Default 0
    /// (`gaminr.f90:550`).
    pub ngam2: i32,
}

impl Default for UnitAssignments {
    /// NJOY defaults (`gaminr.f90:549-550`): `ngam1`/`ngam2` default to 0;
    /// `nendf`/`npend` must be supplied by the caller before a real run (here
    /// they default to 0 too).
    fn default() -> Self {
        Self {
            nendf: 0,
            npend: 0,
            ngam1: 0,
            ngam2: 0,
        }
    }
}

// ---------------------------------------------------------------------------
// Card 2 — print option (gaminr.f90:50, read at :557)
// ---------------------------------------------------------------------------

/// Print-verbosity option `iprint` (card 2, `gaminr.f90:50,557`). Default
/// [`PrintOption::Maximum`] (`iprint = 1`, `gaminr.f90:556`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PrintOption {
    /// `iprint = 0`: minimum print.
    Minimum,
    /// `iprint = 1`: maximum print (default).
    Maximum,
}

impl PrintOption {
    /// Build from the raw `iprint` integer (`gaminr.f90:50`). Any value other
    /// than 0 is treated as maximum, matching NJOY's `if (iprint.eq.1)` checks.
    pub fn from_iprint(iprint: i32) -> Self {
        if iprint == 0 {
            Self::Minimum
        } else {
            Self::Maximum
        }
    }

    /// The raw `iprint` integer (0 = minimum, 1 = maximum).
    pub fn iprint(self) -> i32 {
        match self {
            Self::Minimum => 0,
            Self::Maximum => 1,
        }
    }
}

// ---------------------------------------------------------------------------
// Card 4 — read-in group grid (gaminr.f90:54-56, 676-679)
// ---------------------------------------------------------------------------

/// Card 4: the read-in arbitrary photon group grid, present only when
/// `igg == 1` (`gaminr.f90:675-679`).
///
/// Fortran reads `ngg` then the `ngg+1` boundaries `egg` in **eV**. NJOY caps
/// `ngg+1` at `ngmax = 400` (`gaminr.f90:678`).
#[derive(Debug, Clone, PartialEq)]
pub struct GroupGrid {
    /// `ngg` — number of photon groups.
    pub ngg: usize,
    /// `egg(ngg+1)` — group boundaries in **eV**, ascending. Length `ngg + 1`.
    pub boundaries_ev: Vec<f64>,
}

impl GroupGrid {
    /// `ngmax` from `gaminr.f90:11` — the maximum `ngg+1` NJOY allows.
    pub const NGMAX: usize = 400;

    /// Validate a read-in grid against NJOY's constraints (`gaminr.f90:677-679`):
    /// the boundary count must equal `ngg + 1` and not exceed [`GroupGrid::NGMAX`].
    ///
    /// # Errors
    ///
    /// Returns [`NjoyError::EndfParse`] if the length is inconsistent or too large.
    pub fn validate(&self) -> Result<(), NjoyError> {
        if self.boundaries_ev.len() != self.ngg + 1 {
            return Err(NjoyError::EndfParse(format!(
                "gaminr card4: expected ngg+1={} boundaries, got {}",
                self.ngg + 1,
                self.boundaries_ev.len()
            )));
        }
        if self.ngg + 1 > Self::NGMAX {
            return Err(NjoyError::EndfParse(format!(
                "gaminr card4: too many groups (ngg+1={} > ngmax={})",
                self.ngg + 1,
                Self::NGMAX
            )));
        }
        Ok(())
    }
}

// ---------------------------------------------------------------------------
// Reaction list (card 6) and the built-in "process all" set
// ---------------------------------------------------------------------------

/// ENDF format version of the evaluation, determined by GAMINR from the tape
/// header (`gaminr.f90:158-164`). It selects the ENDF-VI reaction MT numbers
/// (`mtlst6`) over the ENDF-IV/V ones (`mtlst`) for the "process all" set.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EndfFormat {
    /// ENDF-IV (`iverf = 4`).
    Iv,
    /// ENDF-V (`iverf = 5`).
    V,
    /// ENDF-VI (`iverf = 6`) — photoelectric/heating MTs are 522/525.
    Vi,
}

impl EndfFormat {
    /// Whether the ENDF-VI reaction MT set (`mtlst6`) applies (`iverf >= 6`,
    /// `gaminr.f90:287`).
    pub fn uses_vi_mts(self) -> bool {
        matches!(self, Self::Vi)
    }
}

/// One reaction to be group-averaged: an ENDF file (`mfd`), section (`mtd`), and
/// a short descriptive name (`mtname`), matching card 6 (`gaminr.f90:59-66`).
///
/// `mfd = 23` selects a cross-section vector; `mfd = 26` selects a scattering
/// (group-to-group) matrix (`gaminr.f90:298-300`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct GaminrReaction {
    /// `mfd` — ENDF file to process: 23 (cross section) or 26 (matrix).
    pub mfd: i32,
    /// `mtd` — ENDF section (reaction) number.
    pub mtd: i32,
    /// `mtname` — 4-character reaction label (e.g. `"totl"`, `"coht"`).
    pub name: &'static str,
}

/// How the reaction list on card 6 is specified.
#[derive(Debug, Clone, PartialEq)]
pub enum ReactionSelection {
    /// `mfd = -1`: process **all** standard reactions present for the material;
    /// termination is automatic (`gaminr.f90:65-66, 280-288`). The concrete list
    /// is [`standard_reactions`] for the tape's [`EndfFormat`].
    ProcessAll,
    /// An explicit list of reactions read from repeated card 6 records, ended by
    /// `mfd = 0` (`gaminr.f90:271-279`).
    Explicit(Vec<GaminrReaction>),
}

/// The built-in "process all" reaction set (`gaminr.f90:111-118`).
///
/// Nine reactions in NJOY's fixed order: total, coherent (xsec + matrix),
/// incoherent (xsec + matrix), pair production (xsec + matrix), photoelectric
/// absorption, and total heating. The `mtd` values differ for ENDF-VI: the
/// photoelectric-absorption and heating sections become 522 and 525 instead of
/// 602 and 621 (`mtlst` vs `mtlst6`, `gaminr.f90:113-116, 287`).
///
/// | # | `mfd` | `mtd` (IV/V) | `mtd` (VI) | name |
/// |--:|------:|------------:|-----------:|------|
/// | 1 | 23 | 501 | 501 | totl |
/// | 2 | 23 | 502 | 502 | coht |
/// | 3 | 26 | 502 | 502 | coht |
/// | 4 | 23 | 504 | 504 | inch |
/// | 5 | 26 | 504 | 504 | inch |
/// | 6 | 23 | 516 | 516 | pair |
/// | 7 | 26 | 516 | 516 | pair |
/// | 8 | 23 | 602 | 522 | abst |
/// | 9 | 23 | 621 | 525 | heat |
pub fn standard_reactions(format: EndfFormat) -> Vec<GaminrReaction> {
    // mflst / mtlst / mtlst6 / nmlst (gaminr.f90:111-118).
    const MFLST: [i32; 9] = [23, 23, 26, 23, 26, 23, 26, 23, 23];
    const MTLST: [i32; 9] = [501, 502, 502, 504, 504, 516, 516, 602, 621];
    const MTLST6: [i32; 9] = [501, 502, 502, 504, 504, 516, 516, 522, 525];
    const NMLST: [&str; 9] = [
        "totl", "coht", "coht", "inch", "inch", "pair", "pair", "abst", "heat",
    ];
    let mts = if format.uses_vi_mts() { &MTLST6 } else { &MTLST };
    (0..9)
        .map(|i| GaminrReaction {
            mfd: MFLST[i],
            mtd: mts[i],
            name: NMLST[i],
        })
        .collect()
}

// ---------------------------------------------------------------------------
// The complete input deck
// ---------------------------------------------------------------------------

/// A complete, validated GAMINR input deck for one material
/// (`gaminr.f90:37-92`).
///
/// This is the human-facing configuration object: build it, then hand it to
/// [`crate::gaminr::run`] (once the numeric engine is ported). Every field cites
/// the Fortran card it comes from.
#[derive(Debug, Clone, PartialEq)]
pub struct GaminrInput {
    /// Card 1 — tape unit numbers.
    pub units: UnitAssignments,
    /// Card 2 `matb` — the material (MAT) to process (`gaminr.f90:45,557`).
    pub matb: i32,
    /// Card 2 `igg` — photon group-structure selector (`gaminr.f90:47`).
    pub group_structure: PhotonGroupStructure,
    /// Card 2 `iwt` — weight-function selector (`gaminr.f90:48`).
    pub weight: WeightOption,
    /// Card 2 `lord` — Legendre order for scattering matrices (`gaminr.f90:49`).
    pub lord: i32,
    /// Card 2 `iprint` — print verbosity (`gaminr.f90:50`).
    pub iprint: PrintOption,
    /// Card 3 `title` — run label, up to 80 characters (`gaminr.f90:52,558-559`).
    pub title: String,
    /// Card 4 — the read-in group grid, present iff `igg == 1`
    /// ([`PhotonGroupStructure::ArbitraryRead`]).
    pub group_grid: Option<GroupGrid>,
    /// Card 6 — the reaction list (explicit, or "process all").
    pub reactions: ReactionSelection,
    /// Card 7 `matd` — next material to process; `0` terminates the GAMINR run
    /// (`gaminr.f90:68-69,516-517`).
    pub next_matd: i32,
}

impl GaminrInput {
    /// Cross-field validation mirroring GAMINR's card-reading constraints.
    ///
    /// Checks that a read-in group grid is present exactly when
    /// `igg == 1` ([`PhotonGroupStructure::ArbitraryRead`]) and that the grid
    /// itself is well-formed (`gaminr.f90:675-679`). Does not touch tapes.
    ///
    /// # Errors
    ///
    /// [`NjoyError::EndfParse`] on an inconsistent deck; the grid's own
    /// [`GroupGrid::validate`] errors propagate.
    pub fn validate(&self) -> Result<(), NjoyError> {
        let is_read_in = matches!(self.group_structure, PhotonGroupStructure::ArbitraryRead);
        match (&self.group_grid, is_read_in) {
            (Some(grid), true) => grid.validate()?,
            (None, false) => {}
            (Some(_), false) => {
                return Err(NjoyError::EndfParse(
                    "gaminr card4: group_grid supplied but igg!=1".into(),
                ));
            }
            (None, true) => {
                return Err(NjoyError::EndfParse(
                    "gaminr card4: igg==1 requires a read-in group_grid".into(),
                ));
            }
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Methodology: the "process all" set is the 9 fixed reactions in
    /// `gaminr.f90:111-118`, with ENDF-VI switching MT 602/621 to 522/525.
    /// Result (2026-07-15): counts, ordering, mf/mt values, and names match the
    /// Fortran DATA arrays for both format families.
    #[test]
    fn standard_reaction_set() {
        let v5 = standard_reactions(EndfFormat::V);
        assert_eq!(v5.len(), 9);
        // mflst
        assert_eq!(
            v5.iter().map(|r| r.mfd).collect::<Vec<_>>(),
            vec![23, 23, 26, 23, 26, 23, 26, 23, 23]
        );
        // mtlst (IV/V)
        assert_eq!(
            v5.iter().map(|r| r.mtd).collect::<Vec<_>>(),
            vec![501, 502, 502, 504, 504, 516, 516, 602, 621]
        );
        assert_eq!(v5[0].name, "totl");
        assert_eq!(v5[8].name, "heat");

        let v6 = standard_reactions(EndfFormat::Vi);
        // mtlst6: last two switch to 522/525.
        assert_eq!(v6[7].mtd, 522);
        assert_eq!(v6[8].mtd, 525);
        // Everything else identical.
        assert_eq!(&v6[..7], &v5[..7]);
    }

    /// Methodology: `iprint` maps 0->Minimum else Maximum (`gaminr.f90:50,556`).
    /// Result (2026-07-15): round-trips for 0/1; nonzero -> Maximum.
    #[test]
    fn print_option_roundtrip() {
        assert_eq!(PrintOption::from_iprint(0), PrintOption::Minimum);
        assert_eq!(PrintOption::from_iprint(1), PrintOption::Maximum);
        assert_eq!(PrintOption::from_iprint(7), PrintOption::Maximum);
        assert_eq!(PrintOption::Minimum.iprint(), 0);
        assert_eq!(PrintOption::Maximum.iprint(), 1);
    }

    /// Methodology: a deck built with a built-in `igg` and no grid validates;
    /// an `igg==1` deck requires a well-formed grid; mismatches are errors
    /// (`gaminr.f90:675-679`).
    /// Result (2026-07-15): all four grid/igg combinations behave as specified.
    #[test]
    fn deck_validation() {
        let base = GaminrInput {
            units: UnitAssignments {
                nendf: 20,
                npend: 21,
                ngam1: 0,
                ngam2: 22,
            },
            matb: 600,
            group_structure: PhotonGroupStructure::VitaminJ42,
            weight: WeightOption::OneOverEWithRolloffs,
            lord: 3,
            iprint: PrintOption::Maximum,
            title: "carbon photoatomic test".into(),
            group_grid: None,
            reactions: ReactionSelection::ProcessAll,
            next_matd: 0,
        };
        assert!(base.validate().is_ok());

        // igg==1 without a grid is an error.
        let mut bad = base.clone();
        bad.group_structure = PhotonGroupStructure::ArbitraryRead;
        assert!(bad.validate().is_err());

        // igg==1 with a well-formed grid validates.
        let mut good = bad.clone();
        good.group_grid = Some(GroupGrid {
            ngg: 2,
            boundaries_ev: vec![1.0e3, 1.0e6, 2.0e7],
        });
        assert!(good.validate().is_ok());

        // Grid supplied for a built-in structure is an error.
        let mut mismatch = base.clone();
        mismatch.group_grid = Some(GroupGrid {
            ngg: 1,
            boundaries_ev: vec![1.0, 2.0],
        });
        assert!(mismatch.validate().is_err());
    }

    /// Methodology: [`GroupGrid::validate`] enforces length == ngg+1 and the
    /// ngmax cap (`gaminr.f90:677-679`).
    /// Result (2026-07-15): length mismatch and oversize both rejected.
    #[test]
    fn group_grid_bounds() {
        let ok = GroupGrid {
            ngg: 3,
            boundaries_ev: vec![1.0, 2.0, 3.0, 4.0],
        };
        assert!(ok.validate().is_ok());

        let wrong_len = GroupGrid {
            ngg: 3,
            boundaries_ev: vec![1.0, 2.0, 3.0],
        };
        assert!(wrong_len.validate().is_err());

        let too_many = GroupGrid {
            ngg: GroupGrid::NGMAX,
            boundaries_ev: vec![0.0; GroupGrid::NGMAX + 1],
        };
        assert!(too_many.validate().is_err());
    }
}
