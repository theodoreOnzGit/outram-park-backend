// Ported from NJOY2016 `src/resxsr.f90` (git commit ac5adf5f33d893e42f2eed7fb286b0d51c7580da).
// NJOY2016 is under a modified BSD 3-Clause (LANL/DOE) licence, GPL-compatible;
// this derivative file is distributed under GPL-3.0-only. This is a modified,
// non-LANL version, not endorsed by LANL/DOE. See crate root LICENSE.njoy + NOTICE.

//! RESXS file record layout — headers, field values, and word-count index math.
//!
//! The RESXS format specification is embedded verbatim in the Fortran comment
//! block `resxsr.f90:43-189`. This module models each record as a documented
//! Rust struct and reproduces the **word counts** (`w` in the spec) and the
//! **header/field values** the writer emits (`resxsr.f90:435-501`). It is pure
//! layout arithmetic — no tape I/O — so every field can be checked against the
//! Fortran formulas.
//!
//! ## RESXS record sequence (`resxsr.f90:62-78`)
//!
//! 1. [`FileIdentification`] — always
//! 2. [`FileControl`] — always
//! 3. set Hollerith identification — always (blank words, `resxsr.f90:457-463`)
//! 4. [`FileData`] — always
//! 5. per material: [`MaterialControl`] + one or more cross-section blocks
//!
//! All energies are in **electron-volts (eV)**, cross sections in **barns**,
//! temperatures in **kelvin**, and `amass` is the dimensionless atomic-weight
//! ratio to the neutron.

/// Hollerith file-name constant written to the file-identification record.
///
/// Upstream stores `hfile='resxsr'` (`resxsr.f90:214`) and writes it via
/// `read(hfile,'(a6)')` (`resxsr.f90:439`). The format-spec comment names the
/// file `resxs` (`resxsr.f90:46, 92`); the six-character code constant is the
/// authoritative on-tape value and is what this constant reflects.
pub const FILE_NAME: &str = "resxsr";

/// `mult` — double-precision word multiplier (`resxsr.f90:216`). A Hollerith
/// `a8` word occupies `mult` storage words; `mult == 2` on the double-precision
/// builds this port targets (`resxsr.f90:95-97`).
pub const MULT: i32 = 2;

/// `nbuf` — loada/finda scratch-buffer length in words (`resxsr.f90:217`).
pub const NBUF: i32 = 5000;

/// `nx` — default per-energy record width: `1` energy word plus up to 30
/// reaction values (`resxsr.f90:218`). For ENDF/B-IV tapes it is reset to the
/// tape's `n2h` (`resxsr.f90:285`).
pub const NX: i32 = 31;

/// `nblok` — energy blocking factor: the maximum number of storage words in a
/// single cross-section block record (`resxsr.f90:219`).
pub const NBLOK: i32 = 5000;

/// Number of reactions stored per material (`resxsr.f90:166-167`).
///
/// A fissionable material stores three reactions (elastic, fission, capture); a
/// non-fissionable material stores two (elastic, capture).
pub fn nreac_for(fissionable: bool) -> i32 {
    if fissionable {
        3
    } else {
        2
    }
}

// ---------------------------------------------------------------------------
// Record 1 — file identification (resxsr.f90:83-99, written at :437-445)
// ---------------------------------------------------------------------------

/// RESXS **file identification** record (`resxsr.f90:83-99`).
///
/// Layout `hname,(huse(i),i=1,2),ivers`; word count `w = 1 + 3*mult`
/// (`resxsr.f90:88`, computed as `nwds = 3*mult+1` at `:438`).
/// Fortran write format `(4h ov ,a8,1h*,2a8,1h*,i6)` (`resxsr.f90:90`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FileIdentification {
    /// `hname` — the file name Hollerith (`resxs`/`resxsr`); see [`FILE_NAME`].
    pub hname: String,
    /// `huse` word 1 — first 6 chars of the user id (`resxsr.f90:93`).
    pub huse1: String,
    /// `huse` word 2 — next 6 chars of the user id.
    pub huse2: String,
    /// `ivers` — file version number (`resxsr.f90:94`).
    pub ivers: i32,
}

impl FileIdentification {
    /// Word count `w = 1 + 3*mult` (`resxsr.f90:88, 438`).
    pub fn word_count(mult: i32) -> i32 {
        3 * mult + 1
    }
}

// ---------------------------------------------------------------------------
// Record 2 — file control (resxsr.f90:102-118, written at :447-455)
// ---------------------------------------------------------------------------

/// RESXS **file control** record (`resxsr.f90:102-118`).
///
/// Layout `efirst,elast,nholl,nmat,nblok`; word count `w = 5`
/// (`resxsr.f90:107`, `nwds = 5` at `:448`). Fortran write format
/// `(4h 1d ,2i6)` (`resxsr.f90:109`).
#[derive(Debug, Clone, PartialEq)]
pub struct FileControl {
    /// `efirst` — lowest energy on the file, **eV** (`resxsr.f90:111`).
    pub efirst: f64,
    /// `elast` — highest energy on the file, **eV** (`resxsr.f90:112`).
    pub elast: f64,
    /// `nholl` — number of words in the set-Hollerith record (`resxsr.f90:113`).
    pub nholl: i32,
    /// `nmat` — number of materials on the file (`resxsr.f90:115`).
    pub nmat: i32,
    /// `nblok` — energy blocking factor (`resxsr.f90:116`).
    pub nblok: i32,
}

impl FileControl {
    /// Word count `w = 5` (`resxsr.f90:107, 448`).
    pub fn word_count() -> i32 {
        5
    }
}

// ---------------------------------------------------------------------------
// Record 3 — set Hollerith identification (resxsr.f90:121-133, :457-463)
// ---------------------------------------------------------------------------

/// Word count of the **set Hollerith identification** record:
/// `w = nholl * mult` (`resxsr.f90:126`, `nwds = mult*nholl` at `:458`).
///
/// The writer emits blank (`hblank`) words here (`resxsr.f90:460-463`); the
/// descriptive text lives in the file-data / material records. Fortran write
/// format `(4h 2d ,8a8/(9a8))` (`resxsr.f90:128`).
pub fn set_hollerith_words(nholl: i32, mult: i32) -> i32 {
    nholl * mult
}

// ---------------------------------------------------------------------------
// Record 4 — file data (resxsr.f90:136-150, written at :465-473)
// ---------------------------------------------------------------------------

/// RESXS **file data** record (`resxsr.f90:136-150`).
///
/// Layout `(hmatn(i)),(ntemp(i)),(locm(i))`; word count
/// `w = (mult+2)*nmat` (`resxsr.f90:141`, `nwds = (mult+2)*nmat` at `:466`).
/// Fortran write formats `(4h 3d ,8a8/(9a8))` for `hmatn` and `(12i6)` for
/// `ntemp`/`locm` (`resxsr.f90:143-144`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FileData {
    /// `hmatn(i)` — Hollerith identifier for each material (`resxsr.f90:146`).
    pub hmatn: Vec<String>,
    /// `ntemp(i)` — number of temperatures for each material (`resxsr.f90:147`).
    pub ntemp: Vec<i32>,
    /// `locm(i)` — record location of each material on the file
    /// (`resxsr.f90:148`).
    pub locm: Vec<i32>,
}

impl FileData {
    /// Word count `w = (mult+2)*nmat` (`resxsr.f90:141, 466`), where
    /// `nmat = hmatn.len()`.
    pub fn word_count(&self, mult: i32) -> i32 {
        (mult + 2) * self.hmatn.len() as i32
    }
}

// ---------------------------------------------------------------------------
// Record 5a — material control (resxsr.f90:153-170, written at :399-410)
// ---------------------------------------------------------------------------

/// RESXS **material control** record (`resxsr.f90:153-170`).
///
/// Layout `hmat,amass,(temp(i),i=1,ntemp),nreac,nener`; word count
/// `w = mult + 3 + ntemp` (`resxsr.f90:158`, `nwds = mult+n+3` at `:401`).
/// Fortran write formats `(4h 6d ,a8,1h*,1p1e12.5/(6e12.5))` for `hmat,temp`
/// and `(2i6)` for `nreac,nener` (`resxsr.f90:160-161`).
#[derive(Debug, Clone, PartialEq)]
pub struct MaterialControl {
    /// `hmat` — Hollerith material identifier (`resxsr.f90:163`).
    pub hmat: String,
    /// `amass` — atomic-weight ratio to the neutron, dimensionless
    /// (`resxsr.f90:164`).
    pub amass: f64,
    /// `temp(i)` — temperature values for this material, **kelvin**
    /// (`resxsr.f90:165`). `ntemp = temps.len()`.
    pub temps: Vec<f64>,
    /// `nreac` — number of reactions (3 fissionable, 2 otherwise;
    /// `resxsr.f90:166-167`). Computed upstream as `jx/n` (`resxsr.f90:408`).
    pub nreac: i32,
    /// `nener` — number of energies for this material (`resxsr.f90:168`), the
    /// thinned point count `je` (`resxsr.f90:409`).
    pub nener: i32,
}

impl MaterialControl {
    /// Word count `w = mult + 3 + ntemp` (`resxsr.f90:158, 401`).
    pub fn word_count(&self, mult: i32) -> i32 {
        mult + 3 + self.temps.len() as i32
    }
}

// ---------------------------------------------------------------------------
// Record 5b — cross-section block sizing (resxsr.f90:173-189, :412-430/:486-500)
// ---------------------------------------------------------------------------

/// Number of storage words per energy point in a cross-section block:
/// `nn = 1 + nreac*ntemp` (`resxsr.f90:413` material loop, `:488` output copy).
///
/// The `1` is the energy word; the remaining `nreac*ntemp` are the reaction
/// values ordered `(nelas, nfis, ng)` at `temp(1)`, then the same at `temp(2)`,
/// and so on (`resxsr.f90:184-188`).
pub fn xs_point_words(nreac: i32, ntemp: i32) -> i32 {
    1 + nreac * ntemp
}

/// Capacity of one cross-section block record, in words: the largest whole
/// number of energy points that fits within `nblok`,
/// `nwds = (nblok/nn)*nn` (`resxsr.f90:490`).
///
/// Blocks never split an energy point across records.
pub fn xs_block_capacity(nblok: i32, nn: i32) -> i32 {
    (nblok / nn) * nn
}

/// Number of block records needed for `nw` total words at `nwds` words/block:
/// `nb = ceil(nw/nwds)` (`resxsr.f90:491-492`).
pub fn num_blocks(nw: i32, nwds: i32) -> i32 {
    let mut nb = nw / nwds;
    if nw > nb * nwds {
        nb += 1;
    }
    nb
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Constants and per-record word counts match the Fortran formulas.
    ///
    /// **Methodology.** The RESXS spec gives each record's word count `w`
    /// symbolically (`resxsr.f90:88, 107, 126, 141, 158`) and the writer
    /// computes the same as `nwds` (`resxsr.f90:438, 448, 458, 466, 401`). Plug
    /// in `mult=2` (the double-precision build) and concrete counts and assert
    /// each helper reproduces the Fortran value.
    ///
    /// **Result (2026-07-15, commit ac5adf5).** With `mult=2`:
    /// file-id `w = 3*2+1 = 7`; file-control `w = 5`;
    /// set-Hollerith with `nholl=4` → `4*2 = 8`;
    /// file-data with `nmat=3` → `(2+2)*3 = 12`;
    /// material-control with `ntemp=2` → `2+3+2 = 7`. Constants
    /// `MULT=2, NBUF=5000, NX=31, NBLOK=5000`, `FILE_NAME="resxsr"`.
    #[test]
    fn record_word_counts_match_fortran() {
        assert_eq!(MULT, 2);
        assert_eq!(NBUF, 5000);
        assert_eq!(NX, 31);
        assert_eq!(NBLOK, 5000);
        assert_eq!(FILE_NAME, "resxsr");

        assert_eq!(FileIdentification::word_count(MULT), 7);
        assert_eq!(FileControl::word_count(), 5);
        assert_eq!(set_hollerith_words(4, MULT), 8);

        let fd = FileData {
            hmatn: vec!["u235".into(), "u238".into(), "pu239".into()],
            ntemp: vec![1, 1, 1],
            locm: vec![0, 5, 9],
        };
        assert_eq!(fd.word_count(MULT), 12);

        let mc = MaterialControl {
            hmat: "u238".into(),
            amass: 236.006,
            temps: vec![293.6, 600.0],
            nreac: 3,
            nener: 128,
        };
        assert_eq!(mc.word_count(MULT), 7);
    }

    /// `nreac` reaction counts follow the fissionable flag.
    ///
    /// **Methodology.** `resxsr.f90:166-167` — 3 reactions when fissionable
    /// (elastic, fission, capture), 2 otherwise (elastic, capture).
    ///
    /// **Result (2026-07-15).** `nreac_for(true)==3`, `nreac_for(false)==2`.
    #[test]
    fn reaction_count_by_fissionability() {
        assert_eq!(nreac_for(true), 3);
        assert_eq!(nreac_for(false), 2);
    }

    /// Cross-section block sizing reproduces the Fortran blocking arithmetic.
    ///
    /// **Methodology.** For a fissionable material with 2 temperatures the
    /// per-point width is `nn = 1 + 3*2 = 7` (`resxsr.f90:413/488`). A block
    /// holds `(nblok/nn)*nn` words with `nblok=5000` → `(5000/7)*7 = 714*7 =
    /// 4998` (`resxsr.f90:490`). For `nener=1000` points the total is
    /// `nw = 1000*7 = 7000` words → `nb = ceil(7000/4998) = 2` blocks
    /// (`resxsr.f90:491-492`).
    ///
    /// **Result (2026-07-15).** `nn=7`, block capacity `4998`, `nb=2`; and a
    /// non-fissionable material (`nreac=2`, `ntemp=1`) gives `nn=3`, capacity
    /// `(5000/3)*3 = 4998`.
    #[test]
    fn block_sizing_matches_fortran() {
        let nn = xs_point_words(3, 2);
        assert_eq!(nn, 7);
        let cap = xs_block_capacity(NBLOK, nn);
        assert_eq!(cap, 4998);
        let nw = 1000 * nn;
        assert_eq!(nw, 7000);
        assert_eq!(num_blocks(nw, cap), 2);

        let nn2 = xs_point_words(2, 1);
        assert_eq!(nn2, 3);
        assert_eq!(xs_block_capacity(NBLOK, nn2), 4998);

        // Exact-fit case: no rounding up.
        assert_eq!(num_blocks(4998, 4998), 1);
        assert_eq!(num_blocks(4999, 4998), 2);
    }
}
