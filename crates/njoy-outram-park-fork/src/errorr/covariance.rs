// Ported from NJOY2016 `src/errorr.f90` (git commit ac5adf5f33d893e42f2eed7fb286b0d51c7580da):
//   - `subroutine errorr` ENDF-version detection, l.456-466 (`detect_endf_version`).
//   - `subroutine covcal` ENDF I/O staging phase, l.1868-2060 (`read_covariance_section`),
//     the part of `covcal` that reads MF=31/33 records into the `scr(...)` flat
//     buffer *before* the group-average loop (l.2086-2417) runs.
// NJOY2016 is under a modified BSD 3-Clause (LANL/DOE) licence, GPL-compatible;
// this derivative file is distributed under GPL-3.0-only. This is a modified,
// non-LANL version, not endorsed by LANL/DOE. See crate root LICENSE.njoy + NOTICE.

//! ENDF **MF=31/33 covariance-section reader** — the structural ENDF I/O half
//! of `covcal` (`errorr.f90:1770-2417`), *without* the numeric group-average
//! interpretation.
//!
//! # Scope: reader, not solver
//!
//! `covcal` interleaves two very different concerns in one Fortran routine:
//!
//! 1. **ENDF record I/O** (`errorr.f90:1868-2060`): walk the MF=31/33 file for
//!    the requested material, and for each reaction's section read its `NL`
//!    subsections, each of which carries `NC` "combination" (NC-type) and `NI`
//!    "direct matrix" (NI-type) sub-subsections. This stage only does
//!    `contio`/`listio`/`moreio` calls — no arithmetic beyond staging raw
//!    values into a flat buffer (`scr`).
//! 2. **Group-average interpretation** (`errorr.f90:2086-2417`): for every pair
//!    of union-grid groups `(jg, jh)`, decode the `LB`-tagged flat data
//!    according to one of ten mutually-exclusive encodings (`LB` 0..=9, see the
//!    ENDF-6 Formats Manual §File 33) and accumulate `cov(jh)`. This step needs
//!    the union energy grid (`subroutine gridd`, `errorr.f90:1091-1483`, not
//!    yet ported in this crate) and is fused directly into the group loop
//!    rather than being a separable decode step.
//!
//! This module ports **only stage 1** — a faithful structural reader that
//! turns a tape's `(mat, mf, mt)` MF=31/33 section into
//! [`CovarianceSection`]/[`CovarianceSubsection`]/[`NcSubsection`]/
//! [`NiSubsection`], preserving every raw ENDF field. It does **not** decode
//! `LB`-specific matrix layouts, apply energy windows, or compute a single
//! covariance number — that is stage 2 and stays
//! [`NjoyError::NotPorted`] pending the union-grid and group-average kernels.
//! Never treat the raw `list.data` arrays here as already-decoded covariances.
//!
//! # Validated against the real tape
//!
//! The field mapping below was cross-checked line-for-line against
//! `crates/njoy-outram-park-fork/tests/resources/n-092_U_235-ENDF8.0.endf`
//! (MAT=9228): MF=33/MT=1 (7 lines: HEAD + one NC-type subsection with
//! `LTY=0`, a 9-pair "this reaction is the sum of its components" list) and
//! MF=33/MT=2 (HEAD declares `NL=9`; first subsection is `NI`-type with
//! `LB=5`, `LS=1`, `NT=113050 = 475*476/2` matching `NP=475` exactly for a
//! symmetric upper-triangular pack). See the `#[cfg(test)]` block for the
//! synthetic round-trip and the real-tape structural smoke test.

use crate::endf::records::{Cont, List, SectionCursor};
use crate::endf::tape::Tape;
use crate::NjoyError;

// ---------------------------------------------------------------------------
// ENDF format-era detection (errorr.f90:456-466)
// ---------------------------------------------------------------------------

/// Which ENDF format era a tape's covariance sections were written in.
///
/// NJOY's `covcal` only reads the extra CONT record that carries `LTY` ahead
/// of each NC-type sub-subsection when `iverf > 4` (`errorr.f90:2017-2018`);
/// ENDF/B-IV tapes (`iverf == 4`) never carry that record and use a different
/// top-level `NL` field position. `iverf` is determined once per material from
/// its MF=1/MT=451 header (`errorr.f90:456-466`), not from the MF=31/33
/// section itself — see [`detect_endf_version`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SubsectionFormat {
    /// `iverf == 4`: legacy ENDF/B-IV. **Not ported** —
    /// [`read_covariance_section`] returns [`NjoyError::NotPorted`] for this
    /// variant; no ENDF/B-IV covariance tapes are exercised by this crate's
    /// test resources.
    EndfFormatIv,
    /// `iverf == 5` or `iverf == 6`: every ENDF tape since ~1990, including
    /// this crate's `n-092_U_235-ENDF8.0.endf` test resource. NC-type
    /// sub-subsections carry a leading CONT record whose `L2` field is `LTY`.
    EndfFormatVOrVi,
}

/// Detect the ENDF format era of a material from its MF=1/MT=451 header
/// (`subroutine errorr`, `errorr.f90:456-466`).
///
/// Fortran: after `tpidio` (the tape-ID line, already stripped by
/// [`Tape::read`](crate::endf::tape::Tape::read)), ERRORR reads the MT=451
/// section's first two CONT records — the section HEAD
/// `(ZA,AWR,LRP,LFI,NLIB,NMOD)` (`errorr.f90:456`, this crate's `rows[0]`) and
/// the descriptive-data CONT `(ELIS,STA,LIS,LISO,0,NFOR)` (`errorr.f90:457`,
/// `rows[1]`) — and inspects that second record's `N1`/`N2` fields
/// (`errorr.f90:459-466`):
/// - `N1 != 0 && N2 == 0` -> ENDF/B-IV.
/// - `N2 == 0` (and `N1 == 0`) -> ENDF-5.
/// - otherwise (`N2 != 0`, i.e. `NFOR` present) -> ENDF-6 (a third CONT record
///   follows on real tapes, `errorr.f90:465`, not needed for this format
///   determination and not read here).
///
/// Returns [`SubsectionFormat::EndfFormatIv`] for ENDF/B-IV and
/// [`SubsectionFormat::EndfFormatVOrVi`] for ENDF-5/ENDF-6, matching what
/// [`read_covariance_section`] needs to know.
///
/// # Errors
/// [`NjoyError::SectionNotFound`] if `(mat, 1, 451)` is absent, or
/// [`NjoyError::EndfParse`] if the section has fewer than two header rows.
pub fn detect_endf_version(tape: &Tape, mat: i32) -> Result<SubsectionFormat, NjoyError> {
    let section = tape
        .section(mat, 1, 451)
        .ok_or(NjoyError::SectionNotFound { mat, mf: 1, mt: 451 })?;
    let mut cur = SectionCursor::new(&section.rows);
    let _head = cur.read_cont()?; // errorr.f90:456 — (ZA,AWR,LRP,LFI,NLIB,NMOD), unused here.
    let descriptive = cur.read_cont()?; // errorr.f90:457 — (ELIS,STA,LIS,LISO,0,NFOR).
    if descriptive.n1 != 0 && descriptive.n2 == 0 {
        Ok(SubsectionFormat::EndfFormatIv)
    } else {
        // Covers both the n2==0 (ENDF-5) and n2!=0 (ENDF-6) upstream branches
        // (errorr.f90:461-466); both read MF=31/33 covariance sections the
        // same way, so `read_covariance_section` does not need to tell them
        // apart any further.
        Ok(SubsectionFormat::EndfFormatVOrVi)
    }
}

// ---------------------------------------------------------------------------
// Covariance section structure (errorr.f90:1868-2060)
// ---------------------------------------------------------------------------

/// One MF=31 (nu-bar) or MF=33 (cross-section) covariance section: everything
/// following a reaction's HEAD record
/// `[MAT,MF,MT/ ZA, AWR, 0, MTL, 0, NL]HEAD` (`errorr.f90:1872`, the `contio`
/// call that opens each reaction while `covcal` walks the file).
///
/// Physical quantities: `za` is `1000*Z + A` (dimensionless nuclide id);
/// `awr` is the atomic weight ratio to the neutron mass (dimensionless).
#[derive(Debug, Clone)]
pub struct CovarianceSection {
    /// `ZA` — `1000*Z + A` identifying the target nuclide.
    pub za: f64,
    /// `AWR` — atomic weight ratio (mass of the target / mass of the neutron).
    pub awr: f64,
    /// `MTL` — the ENDF-6 Formats Manual File-33 HEAD's `L2` field: nonzero
    /// when this MT's covariance is folded into a lumped reaction `MTL`
    /// instead of being listed directly. **Not read back by NJOY's own
    /// `covcal`** under a named variable (its lumped-reaction skip test is
    /// `NL == 0`, `errorr.f90:1911`, which is equivalent for a
    /// standards-conforming tape); exposed here for completeness of the raw
    /// HEAD record.
    pub mtl: i32,
    /// The `NL` subsections (`errorr.f90:1872-1883` reads `NL` itself; each
    /// subsection is read by the loop at `errorr.f90:1962-2060`). Empty
    /// exactly when `NL == 0` — the lumped-component skip case
    /// (`errorr.f90:1911`, `"ignore components of a lumped reaction"`).
    pub subsections: Vec<CovarianceSubsection>,
}

/// One `NL` subsection of a [`CovarianceSection`]: the covariance of this
/// section's reaction against a second reaction `(mat1, mt1)`.
///
/// Fortran: the CONT record `[MAT,MF,MT/ XMF1, XLFS1, MAT1, MT1, NC, NI]CONT`
/// (`errorr.f90:1969`, fields assigned at `:1979-1983`).
#[derive(Debug, Clone)]
pub struct CovarianceSubsection {
    /// `XMF1` — MF of the second reaction (`0.0` conventionally defaults to
    /// this section's own MF).
    pub xmf1: f64,
    /// `XLFS1` — final-state index of the second reaction (`0.0` = ground
    /// state / not applicable).
    pub xlfs1: f64,
    /// `MAT1` — MAT of the second reaction (`0` = same material as this
    /// section, `errorr.f90:1979`, Fortran `l1h`).
    pub mat1: i32,
    /// `MT1` — MT of the second reaction (`errorr.f90:1980`, Fortran `l2h`).
    /// `mat1 == 0 && mt1 == <this section's MT>` is the reaction's
    /// self-covariance subsection.
    pub mt1: i32,
    /// `NC`-type sub-subsections (`errorr.f90:1981`, Fortran `n1h`): the
    /// reaction expressed as a combination involving other reactions/a
    /// standard. Read in file order (`errorr.f90:2014-2030`).
    pub nc: Vec<NcSubsection>,
    /// `NI`-type sub-subsections (`errorr.f90:1982`, Fortran `n2h`): direct
    /// covariance-matrix data, one `LB`-tagged LIST record each. Read in file
    /// order (`errorr.f90:2031-2060`).
    pub ni: Vec<NiSubsection>,
}

/// An `NC`-type sub-subsection (`errorr.f90:2014-2030`): expresses this
/// subsection's covariance via a combination involving other reactions,
/// selected by `LTY`.
///
/// `LTY` values (ENDF-6 Formats Manual §File 33; `covcal`'s own dispatch is at
/// `errorr.f90:2106-2124`, routed through `subroutine stand`,
/// `errorr.f90:2939-3009`, for `LTY` 1..=3):
/// - `0`: the reaction is the explicit sum of other reactions, each with a
///   coefficient — the common "this MT is the sum of its components" case
///   (confirmed against the real U-235 tape's MF=33/MT=1 section: `LTY=0`,
///   9 `(coefficient=1.0, MT)` pairs summing MT=1 from its partials).
/// - `1`: derived via a linear combination with a third reaction.
/// - `2`: derived as the ratio to a standard reaction.
/// - `3`: derived by removing the average value of a standard reaction.
///
/// **This reader does not interpret `lty` or decode `list.data`'s pair
/// layout** — that is `covcal`'s group-average stage
/// (`errorr.f90:2106-2124`), which needs the union energy grid and is not yet
/// ported. `list.data` is the raw flat array exactly as `listio`/`moreio`
/// read it (`errorr.f90:2019-2022`); `list.head.c1`/`c2` are the `E1`/`E2`
/// energy-window bounds in **eV**.
#[derive(Debug, Clone)]
pub struct NcSubsection {
    /// `LTY` — from the leading CONT record's `L2` field, read only for
    /// [`SubsectionFormat::EndfFormatVOrVi`] (`errorr.f90:2017-2018`).
    pub lty: i32,
    /// The following LIST record, raw: header + the flat `NCI`-length data
    /// array (`errorr.f90:2019-2022`). Field semantics beyond `E1`/`E2` are
    /// not decoded here.
    pub list: List,
}

/// An `NI`-type sub-subsection (`errorr.f90:2038-2051`): the covariance-matrix
/// data itself, a single raw ENDF LIST record
/// `[MAT,MF,MT/ 0.0, 0.0, LS, LB, NT, NP]LIST`.
///
/// `LB` (`errorr.f90:2095`, read back as Fortran `lb`) selects one of ten
/// mutually-exclusive matrix encodings `0..=9` (ENDF-6 Formats Manual §File
/// 33); `LS` (`errorr.f90:2094`, Fortran local `lt`) is a symmetry flag
/// (`0`=asymmetric, `1`=symmetric) meaningful only for `LB` in `{5, 6}`.
/// Decoding the flat `list.data` (length `NT = list.head.n1`) array per `LB`
/// needs the union energy grid (`subroutine gridd`, `errorr.f90:1091-1483`,
/// not yet ported) and is fused into `covcal`'s per-group-pair loop
/// (`errorr.f90:2086-2417`, dispatched by `LB` at `:2138,2154,2179,2206,2238`)
/// rather than being a separable step, so this reader stops at the raw
/// record — exactly where NJOY's own `scr(...)` staging buffer sits before
/// the group loop runs.
///
/// Confirmed against the real U-235 tape (MF=33/MT=2, first subsection):
/// `LS=1`, `LB=5`, `NT=113050`, `NP=475`, and `113050 == 475*476/2` — the
/// expected symmetric upper-triangular pack size for `LB=5`.
#[derive(Debug, Clone)]
pub struct NiSubsection {
    /// `LS` — symmetry flag (0 = asymmetric, 1 = symmetric), the LIST head's
    /// `L1` field (`errorr.f90:2094`, local var `lt`). Only meaningful for
    /// `lb` in `{5, 6}`.
    pub ls: i32,
    /// `LB` — matrix-encoding law (`0..=9`), the LIST head's `L2` field
    /// (`errorr.f90:2095`).
    pub lb: i32,
    /// The raw LIST record: `list.head.n1` is `NT` (the flat data length,
    /// always equal to the ENDF LIST convention's `NPL`); `list.head.n2` is
    /// `NP`. `list.data` holds the `NT` raw words, undecoded.
    pub list: List,
}

/// Read one MF=31 or MF=33 covariance section for `(mat, mt)` off `tape`
/// (`covcal`'s ENDF I/O staging phase, `errorr.f90:1868-2060`).
///
/// This is a **pure structural reader**: it walks the section's `NL`
/// subsections and their `NC`/`NI` sub-subsections and returns every raw ENDF
/// field, but performs no arithmetic and applies no energy windows — see the
/// module docs for the reader/solver split. Only [`SubsectionFormat::EndfFormatVOrVi`]
/// is ported (every tape this crate ships or tests against); the legacy
/// ENDF/B-IV layout is a documented gap.
///
/// # Errors
/// - [`NjoyError::SectionNotFound`] if `(mat, mf, mt)` is absent from `tape`.
/// - [`NjoyError::NotPorted`] if `mf` is not 31 or 33, or `format` is
///   [`SubsectionFormat::EndfFormatIv`].
/// - [`NjoyError::EndfParse`] if the section's records are shorter or
///   differently shaped than the ENDF File-33 layout requires (propagated
///   from [`SectionCursor`]).
pub fn read_covariance_section(
    tape: &Tape,
    mat: i32,
    mf: i32,
    mt: i32,
    format: SubsectionFormat,
) -> Result<CovarianceSection, NjoyError> {
    if mf != 31 && mf != 33 {
        return Err(NjoyError::NotPorted(
            "errorr::covariance::read_covariance_section (only mf=31/33 are ported)",
        ));
    }
    if format == SubsectionFormat::EndfFormatIv {
        return Err(NjoyError::NotPorted(
            "errorr::covariance::read_covariance_section (endf/b-iv legacy layout, errorr.f90:iverf==4)",
        ));
    }

    let section = tape
        .section(mat, mf, mt)
        .ok_or(NjoyError::SectionNotFound { mat, mf, mt })?;
    let mut cur = SectionCursor::new(&section.rows);

    // HEAD: [MAT,MF,MT/ ZA, AWR, 0, MTL, 0, NL]HEAD (errorr.f90:1872, nl=n2h
    // read at :1881-1883 for iverf>4).
    let head = cur.read_cont()?;
    let za = head.c1;
    let awr = head.c2;
    let mtl = head.l2;
    let nl = head.n2;

    let mut subsections = Vec::with_capacity(nl.max(0) as usize);
    for _ in 0..nl {
        subsections.push(read_subsection(&mut cur)?);
    }

    Ok(CovarianceSection { za, awr, mtl, subsections })
}

/// Read one `NL` subsection: its CONT header plus its `NC` and `NI`
/// sub-subsections (`errorr.f90:1962-2060`, one iteration of the `do 650
/// il=1,nl` loop's read-only portion).
fn read_subsection(cur: &mut SectionCursor<'_>) -> Result<CovarianceSubsection, NjoyError> {
    // [MAT,MF,MT/ XMF1, XLFS1, MAT1, MT1, NC, NI]CONT (errorr.f90:1969, 1979-1983).
    let sub_head: Cont = cur.read_cont()?;
    let xmf1 = sub_head.c1;
    let xlfs1 = sub_head.c2;
    let mat1 = sub_head.l1;
    let mt1 = sub_head.l2;
    let nc_count = sub_head.n1;
    let ni_count = sub_head.n2;

    // NC-type sub-subsections (errorr.f90:2014-2030): each is a leading CONT
    // (whose L2 is LTY, iverf>4 only) followed by a LIST.
    let mut nc = Vec::with_capacity(nc_count.max(0) as usize);
    for _ in 0..nc_count {
        let lty_cont = cur.read_cont()?; // errorr.f90:2017
        let lty = lty_cont.l2; // errorr.f90:2018
        let list = cur.read_list()?; // errorr.f90:2019-2022
        nc.push(NcSubsection { lty, list });
    }

    // NI-type sub-subsections (errorr.f90:2031-2060): a LIST record only, no
    // leading CONT. `LS`/`LB` come straight off the LIST head's L1/L2.
    let mut ni = Vec::with_capacity(ni_count.max(0) as usize);
    for _ in 0..ni_count {
        let list = cur.read_list()?; // errorr.f90:2041 (+ moreio continuation, :2046-2051)
        let ls = list.head.l1;
        let lb = list.head.l2;
        ni.push(NiSubsection { ls, lb, list });
    }

    Ok(CovarianceSubsection { xmf1, xlfs1, mat1, mt1, nc, ni })
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use crate::endf::tape::{Section, Tape};
    use crate::endf::EndfKey;

    /// Build a two-row-per-record synthetic MF=33 section mirroring the real
    /// U-235 MF=33/MT=1 section (7 lines: HEAD + one NC-type subsection with
    /// `LTY=0`), to check the reader against hand-computed row counts without
    /// depending on the large real-tape fixture.
    ///
    /// **Methodology.** Hand-construct the raw `[f64; 6]` rows for
    /// `HEAD, subsection-CONT, LTY-CONT, LIST-head+data` (9 data words -> 2
    /// data rows packed 6-per-row), matching the field layout documented on
    /// [`read_covariance_section`], and assert every field the reader
    /// extracts equals the value placed in the synthetic rows. This is a
    /// pure structural round-trip, not a physics check.
    #[test]
    fn synthetic_mf33_round_trip_matches_hand_built_rows() {
        let rows: Vec<[f64; 6]> = vec![
            // HEAD: za, awr, 0, mtl=0, 0, nl=1
            [9.223501e4, 2.330248e2, 0.0, 0.0, 0.0, 1.0],
            // subsection CONT: xmf1=0, xlfs1=0, mat1=0, mt1=1, nc=1, ni=0
            [0.0, 0.0, 0.0, 1.0, 1.0, 0.0],
            // LTY CONT: 0,0,0, lty=0, 0,0
            [0.0, 0.0, 0.0, 0.0, 0.0, 0.0],
            // LIST head: e1=1e-5, e2=3e7, 0,0, n1=9 (data count), n2=4 (np)
            [1.0e-5, 3.0e7, 0.0, 0.0, 9.0, 4.0],
            // 9 data words, packed 6 then 3
            [1.0, 2.0, 1.0, 5.0, 1.0, 16.0],
            [1.0, 17.0, 1.0, 18.0, 1.0, 0.0],
        ];
        let key = EndfKey { mat: 9228, mf: 33, mt: 1 };
        let tape = Tape::from_sections("synthetic".into(), vec![Section { key, rows }]);

        let sec = read_covariance_section(&tape, 9228, 33, 1, SubsectionFormat::EndfFormatVOrVi)
            .expect("synthetic section parses");

        assert_eq!(sec.za, 9.223501e4);
        assert_eq!(sec.awr, 2.330248e2);
        assert_eq!(sec.mtl, 0);
        assert_eq!(sec.subsections.len(), 1);

        let sub = &sec.subsections[0];
        assert_eq!(sub.mat1, 0);
        assert_eq!(sub.mt1, 1);
        assert_eq!(sub.nc.len(), 1);
        assert!(sub.ni.is_empty());

        let nc = &sub.nc[0];
        assert_eq!(nc.lty, 0);
        assert_eq!(nc.list.head.c1, 1.0e-5);
        assert_eq!(nc.list.head.c2, 3.0e7);
        assert_eq!(nc.list.data.len(), 9);
        assert_eq!(nc.list.data[1], 2.0); // first (coefficient, MT) pair's MT=2
    }

    /// Structural smoke test against the real U-235 ENDF8.0 tape
    /// (`tests/resources/n-092_U_235-ENDF8.0.endf`, MAT=9228): parse the
    /// MF=33/MT=1 (total) and MF=33/MT=2 (elastic) sections and assert the
    /// invariants hand-verified against the raw file (see module docs).
    /// Structural only — no covariance values are computed or asserted.
    #[test]
    fn real_tape_mf33_mt1_and_mt2_structural_invariants() {
        let mut p = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"));
        p.push("tests/resources/n-092_U_235-ENDF8.0.endf");
        let tape = Tape::read(std::fs::File::open(p).unwrap()).unwrap();

        assert_eq!(
            detect_endf_version(&tape, 9228).unwrap(),
            SubsectionFormat::EndfFormatVOrVi,
            "ENDF8.0 is a modern (ENDF-6) tape"
        );

        // MT=1 (total): NL=1, one NC-type subsection, LTY=0, 9 (coeff, MT) pairs.
        let mt1 = read_covariance_section(&tape, 9228, 33, 1, SubsectionFormat::EndfFormatVOrVi)
            .expect("U-235 MF=33/MT=1");
        assert_eq!(mt1.mtl, 0);
        assert_eq!(mt1.subsections.len(), 1);
        assert_eq!(mt1.subsections[0].mt1, 1);
        assert_eq!(mt1.subsections[0].nc.len(), 1);
        assert!(mt1.subsections[0].ni.is_empty());
        assert_eq!(mt1.subsections[0].nc[0].lty, 0);
        assert_eq!(mt1.subsections[0].nc[0].list.data.len(), 18); // 9 pairs -> 18 words

        // MT=2 (elastic): NL=9, first subsection is self-covariance (mt1=2)
        // with two NI-type records, the first LB=5/LS=1/NT=113050/NP=475.
        let mt2 = read_covariance_section(&tape, 9228, 33, 2, SubsectionFormat::EndfFormatVOrVi)
            .expect("U-235 MF=33/MT=2");
        assert_eq!(mt2.subsections.len(), 9);
        let self_cov = &mt2.subsections[0];
        assert_eq!(self_cov.mat1, 0);
        assert_eq!(self_cov.mt1, 2);
        assert!(self_cov.nc.is_empty());
        assert_eq!(self_cov.ni.len(), 2);
        let first_ni = &self_cov.ni[0];
        assert_eq!(first_ni.ls, 1);
        assert_eq!(first_ni.lb, 5);
        assert_eq!(first_ni.list.head.n1, 113_050);
        assert_eq!(first_ni.list.head.n2, 475);
        assert_eq!(first_ni.list.data.len(), 113_050);
        assert_eq!(113_050, 475 * 476 / 2, "LB=5 symmetric upper-triangular pack size");
    }
}
