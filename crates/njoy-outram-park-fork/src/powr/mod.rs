// SPDX-License-Identifier: AGPL-3.0-only
// Translated from NJOY2016.80 `src/powr.f90` (git commit ac5adf5f33d893e42f2eed7fb286b0d51c7580da):
//   - the module variables of `powm` (l.1-61) and the branch of `subroutine
//     powr` (l.63-296);
//   - `icgrp` (l.298-360).
// NJOY2016 is under a modified BSD 3-Clause (LANL/DOE) licence, GPL-compatible;
// this derivative file is distributed under AGPL-3.0-only. This is a modified,
// non-LANL version, not endorsed by LANL/DOE. See crate root LICENSE.njoy + NOTICE.

//! `POWR` — libraries for the EPRI-CELL / EPRI-CPM reactor codes.
//!
//! Prepares multigroup libraries in the format required by the Electric Power
//! Research Institute (EPRI) reactor-analysis codes EPRI-CELL and EPRI-CPM, from
//! GROUPR GENDF output. These lattice-physics codes compute core performance for
//! operating and reloading power reactors.
//!
//! **Upstream:** `powr.f90`. **Manual:** LA-UR-17-20093 §POWR.
//!
//! ~~**Status:** not yet ported — [`crate::NjoyError::NotPorted`] placeholder.~~
//! **Status (2026-11-14, gh:#1373):** `lib = 1` (fast, GAMTAP input:
//! `fast`, `gamll`, `gamxs`, `gamff`, `mergt`) and `lib = 2` (thermal,
//! LIBRAR input: `therm`, `packa`) are translated line for line and run from a
//! deck by [`run_powr`] (`crate::deck::powr` reads the cards). `lib = 3`
//! (EPRI-CPM, `cpm` and its `sfile*` routines, `powr.f90:1992-4902`) is
//! **not translated** and is refused by name. **2026-11-14 (gh:#1373):** run
//! first, upstream writes no `lib = 3` library at all. `cpm` reads card 5 into
//! local variables and then fills `ninat`, `ntmp`, `nszt`, `s0rf`, `irest`,
//! `sigpt`, `mtit`, `mtct`, `ip1tab`, `norft`, `ipost` and `iposrt` from
//! `scr(1..12*newmat)`, which nothing in `cpm` writes (an instrumented NJOY2016
//! printed all zeros for card 5 `0 3 7 1.e10 1 10.0 0 0 0 0 1 1`). On eight card-5
//! variants NJOY2016.80 stopped in `sfile2` ("reference sigma zero not found"),
//! failed to `DEALLOCATE` `glamt`, or segfaulted, and its library unit was empty
//! every time. With an earlier POWR in the same deck, `scr` would instead hold
//! that run's GENDF words. Nothing defined is left to reproduce, so the refusal
//! stays. Record: deck-driver README §39a. The listing (unit `nsyso`) is
//! not written; only the library on `nout` is. See `README.md` in this
//! directory for what was verified and how.
//!
//! - [`reader`]: the GENDF read state (`powm`'s module variables, the `scr`
//!   work array, `endf.f90`'s record routines through
//!   [`crate::plotr::stream::EndfStream`]).
//! - [`fast`], [`gamff`]: `lib = 1`.
//! - [`therm`]: `lib = 2`.
//! - [`fmt`]: the Fortran edit descriptors POWR writes with.

pub mod fast;
pub mod fmt;
pub mod gamff;
pub mod reader;
pub mod therm;

use crate::NjoyError;

/// One material of a `lib = 1` run (cards 3, 4 and 5).
#[derive(Debug, Clone, PartialEq)]
pub struct FastMaterial {
    /// Card 3 `matd`, the material (positive; a card with `matd <= 0` ends
    /// the run, see [`fast::fast`]).
    pub matd: i32,
    /// Card 3 `rtemp`, reference temperature in kelvin (preset 300).
    pub rtemp: f64,
    /// Card 3 `iff`, f-factor option (preset 1).
    pub iff: i32,
    /// Card 3 `nsgz`, sigma zeroes to process (preset 0: all on the tape).
    pub nsgz: i32,
    /// Card 3 `izref`, reference sigma zero of the elastic matrix (preset 1).
    pub izref: i32,
    /// Card 4 `hid`, the nuclide's description (`character(12)`).
    pub hid: String,
    /// Card 5 `fsp`, the fission spectrum's title (`character(40)`).
    pub fsp: String,
}

/// One material and temperature of a `lib = 2` run (cards 3, 4 and 5).
#[derive(Debug, Clone, PartialEq)]
pub struct ThermMaterial {
    /// Card 3 `matd` (`0` ends the run).
    pub matd: i32,
    /// Card 3 `idtemp`, the temperature id matched against `nint(T)` of the
    /// GENDF (preset 300).
    pub idtemp: i32,
    /// Card 3 `idname` (`character(12)`; the first ten characters are written).
    pub idname: String,
    /// Card 4 `itrc`, `mti`, `mtc`.
    pub itrc: i32,
    /// Card 4 `mti`, thermal inelastic MT.
    pub mti: i32,
    /// Card 4 `mtc`, thermal elastic MT.
    pub mtc: i32,
    /// Card 5: xi, alpha, mubar, nu, kappa fission, kappa capture, lambda,
    /// sigma s (preset 0).
    pub z: [f64; 8],
}

/// POWR's cards as `powr.f90` reads them.
#[derive(Debug, Clone, PartialEq)]
pub struct PowrCards {
    /// Card 1 `ngendf`, the GENDF unit.
    pub ngendf: i32,
    /// Card 1 `nout` (upstream takes its absolute value).
    pub nout: i32,
    /// Card 2 `lib` (1 fast, 2 thermal, 3 cpm).
    pub lib: i32,
    /// Card 2 `iprint` (listing only).
    pub iprint: i32,
    /// Card 2 `iclaps`.
    pub iclaps: i32,
    /// `lib = 1` materials, in input order.
    pub fast: Vec<FastMaterial>,
    /// `lib = 2` materials, in input order.
    pub therm: Vec<ThermMaterial>,
}

/// The coarse-group maps of `icgrp` (`powr.f90:303-334`), 186 words each.
const IGF: [i32; 186] = [
    0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 1, 1, 1, 2, 2, 3, 3, 4, 4, 4, 4, 4,
    4, 4, 4, 4, 5, 5, 5, 6, 6, 6, 6, 7, 7, 7, 7, 7, 7, 7, 8, 8, 9, 9, 10, 10, 11, 11, 12, 12, 13, 13,
    14, 14, 15, 15, 16, 16, 17, 17, 18, 18, 19, 19, 20, 20, 21, 21, 22, 22, 23, 23, 24, 24, 25, 25,
    26, 26, 27, 27, 28, 28, 29, 29, 30, 30, 31, 31, 32, 32, 33, 33, 34, 34, 35, 35, 36, 36, 37, 37,
    38, 38, 39, 39, 40, 40, 41, 41, 42, 42, 43, 43, 44, 44, 45, 45, 46, 46, 47, 47, 48, 48, 49, 49,
    50, 50, 51, 51, 52, 52, 53, 53, 54, 54, 55, 55, 56, 56, 57, 57, 58, 58, 59, 59, 60, 60, 61, 61,
    62, 62, 63, 63, 64, 64, 65, 65, 66, 66, 67, 67, 68, 68, 69, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
];
const IGT: [i32; 186] = {
    let head: [i32; 45] = [
        0, 1, 1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15, 16, 17, 18, 19, 19, 20, 21, 21, 22,
        22, 23, 23, 24, 24, 25, 26, 27, 28, 29, 30, 30, 31, 32, 32, 33, 34, 35, 36,
    ];
    let mut t = [0; 186];
    let mut i = 0;
    while i < 45 {
        t[i] = head[i];
        i += 1;
    }
    t
};
const IGC: [i32; 186] = [
    1, 2, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15, 16, 17, 18, 19, 20, 20, 21, 22, 22, 23,
    23, 24, 25, 26, 27, 28, 29, 29, 29, 30, 31, 32, 33, 34, 34, 34, 35, 35, 36, 36, 36, 37, 37, 37,
    37, 38, 39, 39, 40, 41, 41, 42, 42, 42, 42, 43, 43, 44, 44, 44, 45, 45, 45, 45, 46, 46, 46, 46,
    46, 47, 47, 47, 47, 48, 48, 48, 48, 48, 49, 49, 49, 49, 49, 49, 49, 50, 50, 50, 50, 50, 50, 50,
    51, 51, 51, 51, 51, 52, 52, 52, 53, 53, 53, 54, 54, 54, 54, 55, 55, 55, 55, 56, 56, 56, 56, 57,
    57, 57, 57, 58, 58, 58, 58, 59, 59, 59, 59, 60, 60, 60, 60, 61, 61, 61, 61, 62, 62, 62, 62, 63,
    63, 63, 63, 64, 64, 64, 64, 65, 65, 65, 65, 66, 66, 66, 66, 67, 67, 67, 67, 68, 68, 68, 68, 69,
    69, 69, 69, 70, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
];

/// `icgrp(ig)` (`powr.f90:298-360`): the coarse group of fine group `ig` of
/// the 185-group structure for library `lib`; `ig` itself when the tape is
/// already in the library's structure (`ngn == ngnd`).
///
/// An `ig` outside `1..=186` is outside upstream's tables (an out-of-bounds
/// read there); it maps to 0 here, which every caller treats as "no group".
pub fn icgrp(ig: i32, ngn: i32, ngnd: i32, lib: i32) -> i32 {
    if ngn == ngnd {
        return ig;
    }
    if ig == 0 {
        return 0;
    }
    let Some(k) = usize::try_from(ig - 1).ok().filter(|&k| k < 186) else {
        return 0;
    };
    match lib {
        1 => IGF[k],
        2 => IGT[k],
        3 => IGC[k],
        // upstream leaves icgrp unset; lib is checked before any call
        _ => 0,
    }
}

/// Run POWR (`powr.f90:63-296`) on the coded GENDF text `gendf`. Returns the
/// library text upstream writes on `nout`.
///
/// # Errors
/// `lib = 3` (refused: upstream writes no library, see the module doc) and any
/// other `lib` (upstream's "illegal
/// library requested."); the errors and refusals of [`fast::fast`] and
/// [`therm::therm`].
pub fn run_powr(cards: &PowrCards, gendf: &str) -> Result<String, NjoyError> {
    let mut r = reader::Reader::new(gendf, cards.lib)?;
    match cards.lib {
        1 => fast::fast(&mut r, &cards.fast),
        2 => therm::therm(&mut r, &cards.therm),
        // Measured 2026-11-14 (gh:#1373): upstream's `cpm` reads card 5 into locals
        // and fills the per-material arrays from `scr`, which nothing wrote, so
        // NJOY2016 writes no library: it stops in `sfile2` or dies at run time.
        3 => Err(NjoyError::NotPorted("POWR lib = 3 (EPRI-CPM, powr.f90:1992-4902): upstream cpm fills card 5 from an unwritten scr and writes no library")),
        _ => Err(NjoyError::InvalidInput("powr: illegal library requested.".into())),
    }
}

/// Run POWR as an [`crate::modules::NjoyModule`] with no tape units bound.
/// POWR runs from a deck ([`run_powr`] through `crate::deck`); this unit-less
/// entry point stays a refusal, as for the other modules.
pub fn run() -> Result<(), NjoyError> {
    Err(NjoyError::NotPorted("powr"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn icgrp_maps_as_upstream() {
        // no collapse: identity
        assert_eq!(icgrp(17, 68, 68, 1), 17);
        // 185 groups: the tables
        assert_eq!(icgrp(22, 185, 68, 1), 1);
        assert_eq!(icgrp(174, 185, 68, 1), 69);
        assert_eq!(icgrp(2, 185, 35, 2), 1);
        assert_eq!(icgrp(45, 185, 35, 2), 36);
        assert_eq!(icgrp(46, 185, 35, 2), 0);
        assert_eq!(icgrp(1, 185, 69, 3), 1);
        assert_eq!(icgrp(174, 185, 69, 3), 70);
        assert_eq!(icgrp(0, 185, 69, 3), 0);
        assert_eq!(icgrp(300, 185, 69, 3), 0);
        assert!(run().is_err());
    }
}
