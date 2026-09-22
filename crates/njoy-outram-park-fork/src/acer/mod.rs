//! ACER — assemble and write a continuous-energy **ACE** library (Type-1 ASCII).
//!
//! Ported from the cross-section core of NJOY2016 `acefc.f90`:
//! - `acelod` — load ENDF data into the ACE arrays (the ESZ block and the
//!   MTR/LQR/TYR/LSIG/SIG cross-section blocks) — see [`build`].
//! - `aceout` + `change` — the Type-1 ASCII file format (header, NXS/JXS arrays,
//!   then the XSS data block written four 20-character fields per line).
//!   **One serialiser**: [`read::RawAceTable::to_type1_string`] and
//!   [`read::RawAceTable::to_type2_bytes`], on the shared edit descriptors in
//!   [`fortran_fmt`]. [`write`] converts an [`AceTable`] into that raw form
//!   and nothing else.
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
//! | LAND / AND (**elastic** angular distribution, MT=2) | 8–9 | **built** (see [`angular`]) |
//! | NU (fission ν̄) | 2 (`nu`) | **built** (see [`nu`]) |
//! | LAND / AND (non-elastic angular distributions) | 8–9 | **built** (see [`angular`]) |
//! | LDLW / DLW (energy distributions) | 10–11 | **built** (see [`energy`]) |
//! | GPD / MTRP / LSIGP / SIGP / LANDP / ANDP / LDLWP / DLWP (photon production) | 12–19 | **built** (see [`photon_blocks`]) |
//! | heating (KERMA) — ESZ column 5 | — | **built** (HEATR H1–H5, via `from_reconr_full`'s `heating` arg) |
//!
//! > ~~The file this writes carries cross sections plus the **elastic**
//! > angular distribution. Secondary energy distributions (DLW) and
//! > non-elastic angular data are still absent, so a transport code cannot yet
//! > follow an inelastic or fission collision.~~ **CORRECTED 2026-09-21** —
//! > that claim was left behind by the increments that added them. `build.rs`
//! > populates JXS slots 2 (NU), 8–9 (LAND/AND for every reaction, not just
//! > elastic), 10–11 (LDLW/DLW) and 12–19 (the photon-production blocks);
//! > `grep -n 'jxs\[' src/acer/build.rs` lists them. The V&V records in
//! > `verification_and_validation/acer_ce_vs_njoy2016_multi_nuclide.md` compare
//! > all of them against NJOY over 55 evaluations.
//!
//! ## Other ACE classes
//!
//! `acer` writes five kinds of table and this crate now builds three of them:
//!
//! | `iopt` | class | upstream | here |
//! |---|---|---|---|
//! | 1 | fast / continuous-energy (`c`) | `acefc.f90` | **built** — this module |
//! | 2 | thermal S(α,β) (`t`) | `aceth.f90` | **built** — [`thermal`], all three `IFENG` forms |
//! | 3 | dosimetry (`y`) | `acedo.f90` | **built** — [`dosimetry`] |
//! | 4 | photo-atomic (`p`) | `acepa.f90` | **built** — [`photoatomic`] |
//! | 5 | photonuclear (`u`) | `acepn.f90` | not ported |
//! | 7, 8 | read / edit a Type-1 / Type-2 file | `acer.f90` | **read, edited and rewritten** — [`read`]; the `print` half is not implemented |
//!
//! ## Entry point
//!
//! ```no_run
//! use njoy_outram_park_fork::acer::AceTable;
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

pub mod acesix;
pub mod angular;
pub mod build;
pub mod dosimetry;
pub mod energy;
pub mod fortran_fmt;
pub mod nu;
pub mod photoatomic;
pub mod photon_blocks;

/// True when the evaluation supplies **MF=4/5/6 secondary distributions for
/// MT=19** (first-chance fission) — upstream's `mt19` flag, set from the tape
/// dictionary at `acefc.f90:388`.
///
/// This decides which fission representation an ACE table stores: with it set,
/// the partial chances MT=19/20/21/38 are stored and MT=18 is dropped; without
/// it, MT=18 is stored and the partials are dropped. Exactly one set may be
/// stored, because MT=18 is their sum.
///
/// Measured on ENDF/B-VIII.0, 2026-09-20: true for U-234 (MF=4/MT=19), false
/// for U-235 and U-238 — matching which reactions NJOY2016 puts in each of
/// those tables.
/// Exact integral of a **lin-lin** tabulation over `[a, b]`.
///
/// Both ACE tables are lin-lin by construction, so a trapezoid over each
/// panel — with the end panels clipped to `a` and `b` and the integrand
/// interpolated there — is not an approximation, it is the integral of the
/// function the table *defines*.
///
/// This is the instrument that makes a broadened-region comparison
/// possible at all: it depends only on the function each table represents,
/// not on where either code chose to put its grid points. A shared-point
/// comparison cannot reach the broadened region, because below `thnmax`
/// the two adaptive grids essentially never coincide.
pub fn integrate_linlin(e: &[f64], x: &[f64], a: f64, b: f64) -> f64 {
    if b <= a || e.len() < 2 {
        return 0.0;
    }
    let at = |i: usize, t: f64| -> f64 {
        // Linear interpolation inside panel i for energy t.
        let (e0, e1) = (e[i], e[i + 1]);
        if e1 <= e0 {
            return x[i];
        }
        x[i] + (x[i + 1] - x[i]) * (t - e0) / (e1 - e0)
    };
    let mut acc = 0.0;
    // First panel whose upper edge exceeds `a`.
    let mut i = match e.binary_search_by(|v| v.partial_cmp(&a).unwrap()) {
        Ok(k) => k,
        Err(k) => k.saturating_sub(1),
    };
    while i + 1 < e.len() && e[i + 1] <= a {
        i += 1;
    }
    while i + 1 < e.len() && e[i] < b {
        let lo = e[i].max(a);
        let hi = e[i + 1].min(b);
        if hi > lo {
            let (xlo, xhi) = (at(i, lo), at(i, hi));
            acc += 0.5 * (xlo + xhi) * (hi - lo);
        }
        i += 1;
    }
    acc
}

pub fn has_mt19_distributions(tape: &crate::endf::tape::Tape, mat: i32) -> bool {
    (4..=6).any(|mf| tape.section(mat, mf, 19).is_some())
}
/// Thermal scattering **S(α,β)** ACE table writer.
///
/// ~~scaffold only (Phase 4, scheduled after the continuous-energy ACE
/// library)~~ **CORRECTED 2026-09-17** — implemented: `thermal.rs` (387
/// lines) writes the ITIE/ITIX/ITXE inelastic blocks and the ITCE/ITCX
/// coherent-elastic Bragg blocks from [`crate::thermr`]'s
/// `IncoherentInelastic`/`CoherentElastic`, and is exercised by `op-1y4y`'s
/// closing evidence (`thermal_from_mf7` writing ITCE/ITCX from
/// `s_of_e_at(temp_k)`). See [`thermal`] for the table layout.
pub mod read;
pub mod thermal;
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
    /// JXS(2): location of the fission ν̄ (NU) block.
    ///
    /// ~~deferred; `0`~~ **CORRECTED 2026-09-20** — written by [`super::nu`],
    /// and `0` now means only what it should: the material is not fissile.
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
    /// JXS(13): location of the photon-production MT list (`MTRP`).
    ///
    /// `0` when the table carries no photon production — a legal ACE table,
    /// not a malformed one.
    pub const MTRP: usize = 12;
    /// JXS(14): locators into [`SIGP`], one per photon-production entry.
    pub const LSIGP: usize = 13;
    /// JXS(15): photon-production yields (MFTYPE=12) or cross sections
    /// (MFTYPE=13).
    pub const SIGP: usize = 14;
    /// JXS(16): locators into [`ANDP`]; `0` means isotropic.
    pub const LANDP: usize = 15;
    /// JXS(17): photon angular distributions. Empty when every photon is
    /// isotropic, in which case it equals [`LDLWP`].
    pub const ANDP: usize = 16;
    /// JXS(18): locators into [`DLWP`], one per photon-production entry.
    pub const LDLWP: usize = 17;
    /// JXS(19): photon energy distributions (ACE Law 2 discrete lines or
    /// Law 4 continua).
    pub const DLWP: usize = 18;

    pub const END: usize = 21;
}

/// Run the ACER card-input driver (NJOY module entry point).
///
/// **Status:** this module's processing physics is ported (see its `README.md`
/// and the typed API above); the NJOY *card-input driver* itself is not yet
/// ported, so this returns [`crate::NjoyError::NotPorted`]. Use the module's
/// typed API directly rather than this driver.
pub fn run() -> Result<(), crate::NjoyError> {
    Err(crate::NjoyError::NotPorted(
        "acer driver (physics ported — use the module API)",
    ))
}
