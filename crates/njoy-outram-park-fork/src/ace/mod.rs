//! ACER — assemble and write a continuous-energy **ACE** library (Type-1 ASCII).
//!
//! Ported from the cross-section core of NJOY2016 `acefc.f90`:
//! - `acelod` — load ENDF data into the ACE arrays (the ESZ block and the
//!   MTR/LQR/TYR/LSIG/SIG cross-section blocks) — see [`build`].
//! - `aceout` + `change` — the Type-1 ASCII file format (header, NXS/JXS arrays,
//!   then the XSS data block written four 20-character fields per line) — see
//!   [`write`].
//!
//! ## What an ACE table is
//!
//! An ACE ("A Compact ENDF") table is the form a Monte-Carlo transport code
//! (MCNP, OpenMC) reads. It is three pieces of data:
//!
//! 1. a short **header** (ZAID string, atomic weight ratio, temperature, comment);
//! 2. two small integer arrays, **NXS** (16 dimensions) and **JXS** (32 locators
//!    into the data block, 1-based; `0` means "block absent"); and
//! 3. one big real array, **XSS**, holding every table laid end-to-end. JXS says
//!    where each block starts.
//!
//! ## Scope of this port (first ACER increment)
//!
//! This builds the **cross-section** portion of a neutron continuous-energy ACE
//! table, which is what RECONR + BROADR already give us:
//!
//! | ACE block | JXS slot | Status |
//! |-----------|----------|--------|
//! | ESZ (energy grid, total, disappearance, elastic, heating) | 1 (`esz`) | **built** |
//! | MTR / LQR / TYR / LSIG / SIG (reaction cross sections) | 3–7 | **built** |
//! | NU (fission ν̄) | 2 (`nu`) | deferred (needs MF=1/MT=452) |
//! | LAND / AND (angular distributions) | 8–9 | deferred (needs MF=4) |
//! | LDLW / DLW (energy distributions) | 10–11 | deferred (needs MF=5/MF=6) |
//! | heating (KERMA) — ESZ column 5 | — | zero (needs HEATR) |
//!
//! The file this writes is therefore a **valid cross-section ACE table** whose
//! secondary-particle blocks are empty. A transport code needs the AND/DLW
//! blocks to track scattering, so this is not yet a complete transport library —
//! it is the foundation the remaining ACER increments build on.
//!
//! ## Entry point
//!
//! ```no_run
//! use njoy_outram_park_fork::ace::AceTable;
//! use njoy_outram_park_fork::reconr::{reconr, ReconrConfig};
//! use njoy_outram_park_fork::endf::tape::Tape;
//! use std::fs::File;
//!
//! let tape = Tape::read(File::open("n-092_U_235-ENDF8.0.endf").unwrap()).unwrap();
//! let cfg  = ReconrConfig { mat: 9228, tolerance: 0.001, temperature: 0.0 };
//! let res  = reconr(&tape, &cfg).unwrap();
//!
//! // kT = 0 (0 K table); suffix .00 → ZAID "92235.00c"
//! let ace = AceTable::from_reconr(&res, 0.0, 0);
//! ace.write_type1("92235.00c.ace").unwrap();
//! ```

pub mod build;
pub mod write;

/// A continuous-energy ACE table held in memory.
///
/// Produced by [`AceTable::from_reconr`] and serialised by
/// [`write_type1`][AceTable::write_type1]. The three data pieces (header, the
/// integer arrays, and the [`xss`][AceTable::xss] data block) mirror the on-disk
/// layout exactly; see the [module docs](self) for the meaning of each array.
#[derive(Debug, Clone)]
pub struct AceTable {
    /// ZAID string, e.g. `"92235.00c"` — right-justified ZA + suffix + the
    /// incident-particle class letter (`c` = continuous-energy neutron). 10 chars.
    pub zaid: String,
    /// Atomic weight ratio AW0 = nuclide mass / neutron mass (`aw0` in NJOY).
    pub awr: f64,
    /// Temperature expressed as kT \[MeV\] (`tz` in NJOY): `k_B · T` converted to
    /// MeV. `0.0` for a 0 K table.
    pub kt_mev: f64,
    /// Processing-date string (`hd`), ≤ 10 chars. Cosmetic; a placeholder here.
    pub date: String,
    /// Free-text comment (`hk`), ≤ 70 chars.
    pub comment: String,
    /// Material-id string (`hm`), e.g. `"   mat9228"`. 10 chars.
    pub mat_id: String,
    /// The 16-entry **NXS** array of table dimensions (1-based in ACE docs;
    /// stored 0-based here). See [`nxs`](self::nxs) for the index names.
    pub nxs: [i32; 16],
    /// The 32-entry **JXS** array of 1-based locators into [`xss`](Self::xss).
    /// A zero entry means the block is absent. See [`jxs`](self::jxs) for names.
    pub jxs: [i32; 32],
    /// The **XSS** data block: every ACE block laid end-to-end. JXS locators are
    /// 1-based indices into this vector.
    pub xss: Vec<f64>,
    /// Per-element flag: `true` where the corresponding [`xss`](Self::xss) value
    /// is semantically an **integer** (e.g. an MT number, a point count, a
    /// locator) and must be written with the `i20` integer style rather than the
    /// `1pE20.11` real style. Length equals `xss.len()`.
    pub xss_is_int: Vec<bool>,
}

/// Named **NXS** indices (0-based into [`AceTable::nxs`]).
///
/// NXS holds the integer dimensions of the table. The ACE specification numbers
/// these 1..16; subtract one for the Rust array.
pub mod nxs {
    /// NXS(1): total length of the XSS data block.
    pub const LEN_XSS: usize = 0;
    /// NXS(2): ZA = 1000·Z + A.
    pub const ZA: usize = 1;
    /// NXS(3): NES — number of energies on the union grid.
    pub const NES: usize = 2;
    /// NXS(4): NTR — number of reactions stored in the MTR block (excludes
    /// elastic MT=2 and the redundant total MT=1).
    pub const NTR: usize = 3;
    /// NXS(5): NR — number of reactions that have angular distributions
    /// (deferred; `0`).
    pub const NR: usize = 4;
    /// NXS(6): NTRP — number of photon-production reactions (deferred; `0`).
    pub const NTRP: usize = 5;
    /// NXS(9): S — excited-state number of the target (`0` = ground state).
    pub const S: usize = 8;
    /// NXS(10): Z — atomic number.
    pub const Z: usize = 9;
    /// NXS(11): A — mass number.
    pub const A: usize = 10;
}

/// Named **JXS** indices (0-based into [`AceTable::jxs`]).
///
/// Each entry is a 1-based locator into [`AceTable::xss`] marking where a block
/// begins; `0` means the block is absent. The ACE specification numbers these
/// 1..32; subtract one for the Rust array.
pub mod jxs {
    /// JXS(1): location of the ESZ block (always `1`).
    pub const ESZ: usize = 0;
    /// JXS(2): location of the fission ν̄ (NU) block (deferred; `0`).
    pub const NU: usize = 1;
    /// JXS(3): location of the MTR block (reaction MT numbers).
    pub const MTR: usize = 2;
    /// JXS(4): location of the LQR block (reaction Q-values \[MeV\]).
    pub const LQR: usize = 3;
    /// JXS(5): location of the TYR block (neutron yields / frame flags).
    pub const TYR: usize = 4;
    /// JXS(6): location of the LSIG block (per-reaction SIG locators).
    pub const LSIG: usize = 5;
    /// JXS(7): location of the SIG block (the reaction cross sections).
    pub const SIG: usize = 6;
    /// JXS(8): location of the LAND block (angular-distribution locators;
    /// deferred; `0`).
    pub const LAND: usize = 7;
    /// JXS(9): location of the AND block (angular distributions; deferred; `0`).
    pub const AND: usize = 8;
    /// JXS(10): location of the LDLW block (energy-distribution locators;
    /// deferred; `0`).
    pub const LDLW: usize = 9;
    /// JXS(11): location of the DLW block (energy distributions; deferred; `0`).
    pub const DLW: usize = 10;
    /// JXS(22): location of the last word of the table (`END` = XSS length).
    pub const END: usize = 21;
}
