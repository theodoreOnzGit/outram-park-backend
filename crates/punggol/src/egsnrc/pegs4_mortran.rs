// SPDX-License-Identifier: AGPL-3.0-only
//
// Attribution (RESEARCH_INTEGRITY_AND_PROVENANCE.md): port of EGSnrc.
//   Upstream project: EGSnrc, https://github.com/nrc-cnrc/EGSnrc
//   Upstream file:    HEN_HOUSE/pegs4/pegs4.mortran
//   Upstream commit:  bd49a47aae3938f49f4100b8e018091861408b19
//   Copyright:        National Research Council of Canada, 2015; Stanford
//                     Linear Accelerator Center, 1986; Board of Trustees of the
//                     Leland Stanford Junior University, 1985 (authors of the
//                     original: Walter Nelson, Hideo Hirayama, Dave Rogers,
//                     1986; contributors named in the file header: Alex
//                     Bielajew, Iwan Kawrakow, Ernesto Mainegra-Hing, Reid
//                     Townson, Blake Walters).
//   Upstream licence: AGPL-3.0-or-later (SLAC and NRC jointly copyright
//                     PEGS4, distributed by NRC under the AGPL 3.0 in
//                     agreement with SLAC); this port is AGPL-3.0-only.
//! `HEN_HOUSE/pegs4/pegs4.mortran` (EGSnrc, 5893 lines): the stand-alone
//! PEGS4 program, which reads a deck of options (`ELEM`, `MIXT`, `COMP`,
//! `ENER`, `MIMS`, `PWLF`, `DECK`, `CALL`, ...), builds the cross sections
//! and stopping powers of one medium at a time, fits them piecewise-linearly
//! and writes a PEGS4 data set (the `.pegs4dat` file the transport reads, see
//! [`super::egsnrc_mortran::parse_pegs4_media`]).
//!
//! Upstream: <https://github.com/nrc-cnrc/EGSnrc>, commit
//! `bd49a47aae3938f49f4100b8e018091861408b19` (read 2026-11-21 from a
//! read-only scratch clone, never committed here). Licence: AGPL-3.0.
//!
//! **Status: first port, not differentially tested (gh:#1680).** Nothing here
//! has been compared with a compiled PEGS4. The tests check internal
//! consistency (totals against integrals of the differentials, round trips
//! through the DECK writer and [`super::egsnrc_mortran::parse_pegs4_media`],
//! the identities the density-effect fit must satisfy) and nothing more. No
//! number in this file is a claim of agreement with PEGS4.
//!
//! # Shape of the port
//!
//! The Fortran program is a main loop over option cards that mutates COMMON
//! blocks. Here:
//!
//! * the input deck is an in-memory list of [`Pegs4Block`] values, one enum
//!   variant per option, each carrying the `&INP` namelist assignments as
//!   [`Pegs4Inp`] (every field an `Option`: `Some` means "present in the
//!   namelist"; absent variables keep the value they had, as a Fortran
//!   NAMELIST read does). **There is no NAMELIST text parser.**
//! * the COMMON blocks and the `MEDSET`/`ENGSET` flags are the fields of
//!   [`Pegs4Program`], which owns a [`Pegs4Common`] (the previous port's
//!   state, reused for everything the two upstream files share) plus the
//!   blocks only this file has (`MIXDAT` names, `RSLTS`, `PWLFIN`, `RAYLEI`,
//!   the `L...` function-local commons, the `MIMSD` tables);
//! * units 8 and 9 (photo-electric/pair/coherent cross sections and atomic
//!   form factors) are [`Pegs4Data::parse`], free-format reads of in-memory
//!   text, as upstream's `READ(8,*)` / `READ(9,*)` (`:757-770`); unit 20 (the
//!   density-effect file) and unit 22 (`aprime.data`) are in-memory `&str`;
//! * the data set written to unit 7 is [`Pegs4Output::deck`], one `String` per
//!   record, in the Fortran formats (`1X,14I5`; `1X,1P,5E14.5`; ...), see
//!   [`fmt_e`], [`fmt_f`], [`fmt_i`];
//! * `$CALL_EXIT(n)` and `STOP` become `Err(String)`; [`run`] is the entry.
//!
//! # Precision
//!
//! `$REAL` is `real*4` (`pegs4.mortran:240`) and `DOUBLEPRECISION` is
//! replaced by `REAL` by a macro (`:644`, so `DCADRE` and `QD` are single
//! precision too, as in the previous port). **Storage and arithmetic are
//! `f32`** with the expression order of the Mortran kept; the intermediate
//! `REAL*8` of `COMPRM` (`:3890-3911`) is the one place `f64` is used, with
//! upstream's narrowing points. Whether the Fortran compiler contracts to
//! FMA or uses another `libm` `logf`/`expf` is not controlled here: that is
//! what phase 2 must measure. `**2`/`**3` are products, `**(-1./3.)` is
//! `powf`.
//!
//! # Reuse and what differs from `pegs4_routines.mortran`
//!
//! `pegs4.mortran` contains its own copies of most routines in
//! [`super::pegs4_routines_mortran`] (that file is the NRC library version
//! with `P`-suffixed names: `RMP`, `AEP`, `NEP`, ...). I diffed every shared
//! routine after normalising comments and the name suffixes: **`BREMTM`,
//! `BREMRM`, `BREMRZ`, `BREMDZ`, `BREMFZ`, `BRMS*`, `AMOLTM`, `AMOLRM`,
//! `BHABTM`, `BHABRM`, `ANIHTM`, `APRIM`, `AINTP`, `TMXS`, `TMXB`, `ALKE`,
//! `ALKEI`, `DCADRE`, `QD`, `QFIT`, `PWLF1`, `DIFFER`, `FCOULC`, `XSIF` and
//! `ZTBL` are the same computation** and are reused from `Pegs4Common`,
//! [`aintp`], [`qd`], [`pwlf1`] (called through [`Pegs4Common`]). The ones
//! that **differ**, ported here under `_pegs4` names:
//!
//! * `MIX` (`:1914-2109`): the radiation length is
//!   `RLC = 1/((ZAB-ZF)*(AN*RHO/WM)*4*FSC*R0**2)` (`:2034`), the library
//!   version `1/((AN*RHO/WM)*4*FSC*R0**2*(ZAB-ZF))`: a different `f32`
//!   rounding sequence ([`Pegs4Program::mix_pegs4`]).
//! * `SPINIT` (`:2672-2923`): the density effect comes from the
//!   Sternheimer-Seltzer-Berger table of 73 media, the Sternheimer-Peierls
//!   formula or user-supplied parameters (`EPSTFL = 0`) or from the density
//!   file (`EPSTFL = 1`); the library version always takes a table
//!   ([`Pegs4Program::spinit_pegs4`]).
//! * `SPIONB` (`:5184-5282`), and so `SPIONE`, `SPIONP`, `SPTOTE`, `SPTOTP`
//!   and `EFUNS`, which call it: with `EPSTFL = 0` the correction is the
//!   analytic `DELTA(X)` (`:5235-5240`); the library `SPIONB` always
//!   interpolates the table ([`Pegs4Program::spionb_pegs4`] and friends).
//!
//! Only here (never in the library file): `GFUNS`, `RFUNS`, `PMDCON`, `LAY`,
//! `MOLIER`, `IFUNT`, `FI`, `AFFACT`, the `ALIN`/`ALINI`/`AREC`/`ADFMOL`/
//! `ADIMOL`/`ADDMOL` distribution-function helpers, the differential,
//! f- and run-time functions `AMOLDM`, `AMOLFM`, `ANIHDM`, `ANIHFM`, `ANIHRM`,
//! `BHABDM`, `BHABFM`, `BREMDR`, `BREMFR`, `BREMRR`, `BREMTR`, `COMP*`,
//! `PAIR*`, `COHET*`, `PHOTT*`, and the derived functions `CRATIO`, `EBIND`,
//! `EBR1`, `EDEDX`, `ESIG`, `GBR1`, `GBR2`, `GMFP`, `PBR1`, `PBR2`, `PDEDX`,
//! `PSIG`, `TMXDE2`, and the main program's option logic.
//!
//! The standalone program's physical constants come from `PMDCON`
//! (`:2498-2551`), not from `$INIT-PEGS4-VARIABLES`: `RM = RME*C**2/ERGMEV`
//! with `RME = 9.1091E-28` is not the `PRM = 0.5109989461` the NRC library
//! uses ([`pmdcon`]).
//!
//! # Not ported
//!
//! * **Plotting and plot-driven options**: `PLOT`, `PLOT1`, `HPLT1`, `HPLT`
//!   and the `PLTN`, `PLTI`, `HPLT` and `TEST` options (`TEST` is a list of
//!   `PLOT` calls and nothing else). [`Pegs4Block::Plot`] and
//!   [`Pegs4Block::Test`] are accepted and do nothing but leave a line in
//!   [`Pegs4Output::listing`]. `DBUG` is empty upstream and is the same here.
//! * **Unit and file opening**: `open_units`, `egs_strip_path`,
//!   `show_pegs4_help`, `lnblnk1`, the command-line options (`:5478-5893`),
//!   and the `pegs4.log` side file `LAY` appends to (`:1862-1900`: it needs a
//!   file system and the date).
//! * **The `NAMELIST` parser** and the listing file (unit 6): the `OUTPUT`
//!   diagnostics are not reproduced except for a few lines in
//!   [`Pegs4Output::listing`]; `QD`'s unit-10 `DCADRE CODE` message
//!   (`:5164-5176`) is omitted, as in the previous port.
//! * `IFUN`, `IV`, `VLO`, `VHI`, `IDF`, `NPTS`, `EI`, `ISUB` of `&INP`: they
//!   feed only the plotting options.
//! * `ISTATB` (`DATA ISTATB`, `:1386-1390`): declared and never read.
//!
//! # Things in the upstream source that look wrong (quoted, not changed)
//!
//! * **A second medium in one run is refused** unless its `&INP` zeroes the
//!   density-effect variables. `SPINIT` stores its results in `AFACT`, `SK`,
//!   `X0`, `X1`, `IEV`, `CBAR` (`:2726-2729`, `:2744`, `:2828`), the same
//!   variables the `&INP` namelist assigns (`:720`), and the next `SPINIT`
//!   begins `IF(ISSB.EQ.0.AND.(AFACT.NE.0.0.OR.CBAR.NE.0.0.OR. ...))` ...
//!   `INCORRECT USER-OVERRIDE OF SSB-DATA`, `$CALL_EXIT(21)` (`:2711-2715`).
//!   Every input deck shipped in `pegs4/inputs` defines exactly one medium.
//!   Pinned by a test; the way round is `AFACT=0, CBAR=0, SK=0, X0=0, X1=0,
//!   IEV=0` in the next block.
//! * `:812-816`: a medium whose `RHO` is absent takes `RHOSAV`, the density
//!   of the *previous block* (already multiplied by `GASP` in `MIX`, `:2005`),
//!   for every option but `ELEM`. A `MIXT`/`COMP` after a gas without `RHO`
//!   silently inherits a scaled density.
//! * `:1004`: `XVAL(I)=XVAL(I)**2.` squares the form-factor abscissae **in
//!   place**, so a second `PWLF` with `IRAYL=1` (same program run) squares
//!   them again.
//! * `:980-984`, `:1015`: `PWLF` passes `NALE`/`NALG`/`NALR` as the maximum
//!   number of subintervals, but the arrays are dimensioned `$MXEKE = 150`,
//!   `$MXGE = 2000`, `$MXRL = 100` (`:160-166`): a larger `NALE` writes past
//!   `AFE`. Here it is an `Err`.
//! * `:3900` (`COMPRM`): `C1`, `C2`, `C3`, `EPS1`, `EPS2`, `K0P` are
//!   explicit `REAL*8` ("for very low energy photons") in a file whose
//!   macro turns every `DOUBLEPRECISION` into `REAL` (`:644`); the explicit
//!   `REAL*8` is not touched by the macro.
//! * `:2871-2874` (`SPINIT`, `EPSTFL=1`): a zero sum of `RHOZ` only prints
//!   "SOMETHING WRONG" and the run goes on, dividing by it at `:2888-2889`.
//! * `:2855-2893` (`SPINIT`, composition check): the flag `ICHECK` is cleared
//!   by *any* matching element and set only by a weight mismatch, so a later
//!   matching element clears an earlier mismatch (the library version has the
//!   same logic, `pegs4_routines.mortran:1825-1841`). Ported as written.
//! * `:2005`: `MIX` multiplies `RHO` by `GASP` in place and `LAY` writes
//!   that scaled `RHO` next to `GASP` (`:1830`); the reader (`HATCH`) takes
//!   the written `RHO` as the density.
//! * `:2133-2320` (`MOLIER`): `VERT2` is dimensioned `($MXVRT2 = 200,
//!   $MSSTEPS)` (`:158`) but filled to `MXV2 = 1000 - MXV1` entries, which can
//!   exceed 200. Here the rows hold 1000.
//! * **A defect found in this crate's existing code, not upstream**:
//!   `egsnrc_mortran::parse_pegs4_media` reads the gas header
//!   (`(5A1,5X,F11.0,4X,I2,26X,I1,9X,I1,9X,I1)`, `egsnrc.mortran:2086`) with
//!   `IUNRST` at column 52; the shipped gas record
//!   ` MIXT,RHO= 1.2048E-03,NE= 4,GASP= 1.0000E+00, IUNRST=0, ...`
//!   (`pegs4/data/521icru.pegs4dat`) and this file's [`Pegs4Program::lay`]
//!   put it at column 53, so the real layout fails to parse
//!   (`bad numeric field '='`). Its test fixture
//!   (`egsnrc_mortran.rs:9402`) omits the comma before ` IUNRST`, one
//!   character short of the real record. Not edited here (out of this
//!   file's scope).
//!
//! # What is ported
//!
//! | upstream (`pegs4.mortran`) | here |
//! |---|---|
//! | main program, option loop (`:668-1151`) | [`Pegs4Program::run`], [`run`] |
//! | `ELEM`/`MIXT`/`COMP` (`:855-940`) | `Pegs4Program::medium_option` |
//! | `ENER` (`:942-957`) | `Pegs4Program::ener_option` |
//! | `PWLF` (`:966-1022`) | `Pegs4Program::pwlf_option` |
//! | `DECK` (`:1024`) with `LAY` (`:1808-1909`) | [`Pegs4Program::lay`] |
//! | `CALL` (`:1056-1060`), `IFUNT`, `FI` (`:4516-4590`) | [`Pegs4Program::call_option`], [`ifunt`], [`Pegs4Program::fi`] |
//! | `MIMS` (`:959`) with `MOLIER` (`:2115-2324`) | [`Pegs4Program::molier`] |
//! | `BLOCK DATA` (`:1160-1428`) | [`Pegs4Program::new`], `SSB_*`, `ITBL`, `RHOTBL` |
//! | `PMDCON` (`:2498-2551`) | [`pmdcon`] |
//! | `GFUNS`, `RFUNS` (`:1680-1708`, `:2656-2667`) | [`Pegs4Program::gfuns`], [`Pegs4Program::rfuns`] |
//! | `MIX` (`:1914-2109`) | [`Pegs4Program::mix_pegs4`] |
//! | `SPINIT` (`:2672-2923`) | [`Pegs4Program::spinit_pegs4`] |
//! | `EFUNS` (`:1596-1675`) | [`Pegs4Program::efuns_pegs4`] |
//! | `SPIONB/E/P`, `SPTOTE/P` (`:5184-5365`) | `spionb_pegs4`, `spione_pegs4`, `spionp_pegs4`, `sptote_pegs4`, `sptotp_pegs4` |
//! | `ADDMOL`, `ADFMOL`, `ADIMOL`, `AFFACT`, `ALIN`, `ALINI`, `AREC` (`:2928-3058`, `:3375`) | methods of the same lower-case names |
//! | `AMOLDM`, `AMOLFM`, `ANIHDM`, `ANIHFM`, `ANIHRM` (`:3093-3254`) | methods |
//! | `BHABDM`, `BHABFM` (`:3389-3431`) | methods |
//! | `BREMDR`, `BREMFR`, `BREMRR`, `BREMTR` (`:3488-3689`) | methods |
//! | `COHETM`, `COHETZ`, `COMPDM`, `COMPFM`, `COMPRM`, `COMPTM`, `CRATIO` (`:3811-3942`) | methods |
//! | `EBIND`, `EBR1`, `EDEDX`, `ESIG` (`:4429-4492`) | methods |
//! | `GBR1`, `GBR2`, `GMFP` (`:4532-4571`) | methods |
//! | `PAIRDR`, `PAIRDZ`, `PAIRFR`, `PAIRFZ`, `PAIRRM`, `PAIRRR`, `PAIRRZ`, `PAIRTE`, `PAIRTM`, `PAIRTR`, `PAIRTU`, `PAIRTZ` (`:4728-5022`) | methods |
//! | `PBR1`, `PBR2`, `PDEDX`, `PHOTTE`, `PHOTTZ`, `PSIG` (`:5027-5125`) | methods (`PHOTTE` is [`Pegs4Program::photte`]) |
//! | `TMXDE2` (`:5397-5410`) | [`Pegs4Program::tmxde2`] |

#[cfg(not(any(feature = "std", test)))]
#[allow(unused_imports)]
use crate::nostd_prelude::*;

use petir::mathf::RealMath;
use petir::real::Real;

use super::get_media_inputs_mortran::{ASYMT, WATBL};
use super::pegs4_routines_mortran::{aintp, pwlf1, qd, ListReader, Pegs4Common, Pegs4Constants, Pegs4Error};

fn ln(x: f32) -> f32 {
    // glibc's `logf` (what gfortran calls) is not correctly rounded (0.35 % of arguments);
    // with `std` the platform's is used, which reproduces it bit for bit (gh:#1688)
    #[cfg(feature = "std")]
    {
        x.ln()
    }
    #[cfg(not(feature = "std"))]
    {
        RealMath::r_ln(f64::from(x)) as f32
    }
}
fn exp(x: f32) -> f32 {
    #[cfg(feature = "std")]
    {
        x.exp()
    }
    #[cfg(not(feature = "std"))]
    {
        RealMath::r_exp(f64::from(x)) as f32
    }
}
fn powf(x: f32, y: f32) -> f32 {
    #[cfg(feature = "std")]
    {
        x.powf(y)
    }
    #[cfg(not(feature = "std"))]
    {
        RealMath::r_powf(f64::from(x), f64::from(y)) as f32
    }
}
fn sqrt(x: f32) -> f32 {
    Real::sqrt(x)
}

/// `$MXEL` (`pegs4.mortran:153`): maximum number of elements in a medium.
pub const MXEL: usize = 20;
/// `$MXEKE` (`:162`): electron mapped energy intervals (`AFE`, `BFE` rows).
pub const MXEKE: usize = 150;
/// `$MXGE` (`:160`): photon mapped energy intervals (`AFG`, `BFG` rows).
pub const MXGE: usize = 2000;
/// `$MXRL` (`:166`): Rayleigh sampling intervals (`AFR`, `BFR`).
pub const MXRL: usize = 100;
/// `$MSSTEPS` (`:157`): multiple-scattering step sizes.
pub const MSSTEPS: usize = 16;
/// `$MXJREFF` (`:156`): size of the multiple-scattering `JREFF` map.
pub const MXJREFF: usize = 200;
/// `$MXVRT1` (`:155`): size of `VERT1`.
pub const MXVRT1: usize = 1000;
/// `NMED = $MXSTC` (`:154`, `:1194`): media with Sternheimer coefficients.
pub const NMED: usize = 73;
/// `LMED` (`:1194`): characters in a medium identifier.
pub const LMED: usize = 24;
/// `$F0(29)` (`:186`), the last `F0` value of `MOLIER`'s Bethe table.
const F0_29: f32 = 1.18e-38;

/// `RHOTBL` (`:1368-1376`): densities in g/cm^3 for `Z = 1..100`.
const RHOTBL: [f32; 100] = [
    0.0808, 0.19, 0.534, 1.85, 2.5, 2.26, 1.14, 1.568,
    1.5, 1.0, 0.9712, 1.74, 2.702, 2.4, 1.82, 2.07,
    2.2, 1.65, 0.86, 1.55, 3.02, 4.54, 5.87, 7.14,
    7.3, 7.86, 8.71, 8.90, 8.9333, 7.140, 5.91, 5.36,
    5.73, 4.80, 4.2, 3.4, 1.53, 2.6, 4.47, 6.4,
    8.57, 9.01, 11.50, 12.20, 12.50, 12., 10.5, 8.65,
    7.30, 7.31, 6.684, 6.24, 4.93, 2.7, 1.873, 3.5,
    6.15, 6.90, 6.769, 7.007, 1., 7.54, 5.17, 7.87,
    8.25, 8.56, 8.80, 9.06, 9.32, 6.96, 9.85, 11.40,
    16.60, 19.30, 20.53, 22.48, 22.42, 21.45, 19.30, 14.19,
    11.85, 11.34, 9.78, 9.30, 1., 4., 1., 5.,
    1., 11.0, 15.37, 18.90, 20.5, 19.737, 11.7, 7.,
    1., 1., 1., 1.,
];

/// `ITBL` (`:1378-1384`): mean excitation energies in eV for `Z = 1..100`.
const ITBL: [f32; 100] = [
    19.2, 41.8, 40., 63.7, 76.0, 78.0, 82.0, 95.0,
    115., 137., 149., 156., 166., 173., 173., 180.,
    174., 188., 190., 191., 216., 233., 245., 257.,
    272., 286., 297., 311., 322., 330., 334., 350.,
    347., 348., 357., 352., 363., 366., 379., 393.,
    417., 424., 428., 441., 449., 470., 470., 469.,
    488., 488., 487., 485., 491., 482., 488., 491.,
    501., 523., 535., 546., 560., 574., 580., 591.,
    614., 628., 650., 658., 674., 684., 694., 705.,
    718., 727., 736., 746., 757., 790., 790., 800.,
    810., 823., 823., 830., 825., 794., 827., 826.,
    841., 847., 878., 890., 902., 921., 934., 939.,
    952., 966., 980., 994.,
];

/// `MEDTB1..4` (`:1196-1235`): the 73 Sternheimer-Seltzer-Berger medium
/// identifiers, in table order, trimmed of the blank padding to 24 characters.
const SSB_NAMES: [&str; 73] = [
    "H2-GAS",
    "H2-LIQUID",
    "HE-GAS",
    "LI",
    "BE",
    "C-2.265 G/CM**3",
    "C-1.70 G/CM**3",
    "N2-GAS",
    "O2-GAS",
    "NE-GAS",
    "NA",
    "MG",
    "AL",
    "SI",
    "AR-GAS",
    "K",
    "CA",
    "TI",
    "V",
    "MN",
    "FE",
    "CO",
    "NI",
    "CU",
    "ZN",
    "GE",
    "SE",
    "KR-GAS",
    "RB",
    "MO",
    "AG",
    "CD",
    "IN",
    "SN",
    "XE-GAS",
    "CS",
    "GD",
    "TA",
    "W",
    "PT",
    "AU",
    "HG",
    "PB",
    "RN-GAS",
    "U",
    "AIR-GAS",
    "CO2-GAS",
    "POLYETHYLENE",
    "POLYPROPYLENE",
    "XYLENE",
    "TOLUENE",
    "NYLON",
    "VINYLTOLUENE",
    "A150-PLASTIC",
    "STILBENE",
    "POLYSTYRENE",
    "ANTHRACENE",
    "LEXAN",
    "LUCITE",
    "H2O",
    "MYLAR",
    "KAPTON",
    "LIF",
    "POLYVINYL-CL",
    "PYREX-GLASS",
    "SIO2",
    "CAF2",
    "PHOTOEMULSION",
    "AGCL",
    "NAI",
    "LII",
    "AGBR",
    "CSI",
];
/// `STDAT1..4` (`:1240-1322`): `AFACT, SK, X0, X1, IEV, CBAR` per medium
/// (Sternheimer, Seltzer and Berger, BNL 31435, as the file cites it).
const SSB_DATA: [[f32; 6]; 73] = [
    [0.03535, 6.790, 1.864, 3.5, 19.2, 9.584],
    [0.09179, 5.831, 0.476, 2.0, 21.8, 3.263],
    [0.0114, 7.625, 2.202, 4.0, 41.8, 11.139],
    [0.3492, 3.233, 0.0966, 2.0, 40.0, 3.122],
    [0.3518, 3.034, -0.0089, 2.0, 63.7, 2.785],
    [0.5848, 2.360, -0.0089, 2.0, 78.0, 2.868],
    [0.7154, 2.191, -0.0089, 2.0, 78.0, 3.155],
    [0.2120, 3.041, 1.738, 4.0, 82.0, 10.540],
    [0.2666, 2.825, 1.754, 4.0, 95.0, 10.700],
    [0.1202, 3.357, 2.073, 4.5, 137.0, 11.904],
    [0.2204, 3.103, 0.4515, 2.8, 149.0, 5.053],
    [0.1714, 3.223, 0.2386, 2.8, 156.0, 4.530],
    [0.3346, 2.795, 0.0966, 2.5, 166.0, 4.239],
    [0.3755, 2.720, 0.0966, 2.5, 173.0, 4.435],
    [0.1902, 2.982, 1.764, 4.5, 188.0, 11.948],
    [0.3041, 2.674, 0.2386, 3.0, 190.0, 5.642],
    [0.2177, 2.874, 0.1751, 3.0, 191.0, 5.040],
    [0.1782, 2.946, 0.0485, 3.0, 233.0, 4.445],
    [0.1737, 2.935, -0.0089, 3.0, 245.0, 4.266],
    [0.1996, 2.812, -0.0089, 3.0, 272.0, 4.270],
    [0.2101, 2.771, -0.0089, 3.0, 286.0, 4.291],
    [0.2229, 2.713, -0.0089, 3.0, 297.0, 4.260],
    [0.2504, 2.619, -0.0089, 3.0, 311.0, 4.312],
    [0.2557, 2.613, -0.0089, 3.0, 322.0, 4.419],
    [0.3163, 2.468, 0.0485, 3.0, 330.0, 4.691],
    [0.2809, 2.647, 0.2386, 3.0, 350.0, 5.141],
    [0.2979, 2.635, 0.2386, 3.0, 348.0, 5.321],
    [0.1519, 3.030, 1.716, 4.8, 352.0, 12.512],
    [0.1450, 3.078, 0.4515, 3.5, 363.0, 6.478],
    [0.2228, 2.824, 0.1751, 3.0, 424.0, 4.879],
    [0.3091, 2.563, -0.0089, 3.0, 470.0, 5.063],
    [0.1853, 2.819, 0.0485, 3.3, 469.0, 5.273],
    [0.2004, 2.790, 0.1751, 3.3, 487.0, 5.517],
    [0.1898, 2.839, 0.2386, 3.3, 488.0, 5.534],
    [0.1329, 3.020, 1.563, 5.0, 482.0, 12.728],
    [0.2214, 2.784, 0.4515, 3.5, 488.0, 6.914],
    [0.2068, 2.686, 0.0485, 3.5, 591.0, 5.874],
    [0.1663, 2.805, 0.1751, 3.5, 718.0, 5.526],
    [0.1499, 2.870, 0.1751, 3.5, 727.0, 5.406],
    [0.1465, 2.903, 0.0966, 3.5, 790.0, 5.473],
    [0.1533, 2.881, 0.0966, 3.5, 790.0, 5.575],
    [0.1824, 2.798, 0.2386, 3.5, 800.0, 5.961],
    [0.1861, 2.814, 0.2386, 3.5, 823.0, 6.202],
    [0.1130, 3.023, 1.537, 5.3, 794.0, 13.284],
    [0.1362, 3.034, 0.2386, 3.5, 890.0, 5.869],
    [0.2466, 2.879, 1.742, 4.0, 85.7, 10.595],
    [0.1999, 3.022, 1.648, 4.0, 88.7, 10.239],
    [0.4875, 2.544, 0.1379, 2.0, 57.4, 3.002],
    [0.2493, 2.975, 0.1537, 2.3, 59.2, 3.126],
    [0.2755, 2.911, 0.1695, 2.3, 61.8, 3.270],
    [0.2830, 2.890, 0.1722, 2.3, 62.5, 3.303],
    [0.5345, 2.439, 0.1336, 2.0, 63.9, 3.063],
    [0.3495, 2.749, 0.1467, 2.2, 64.7, 3.201],
    [0.5462, 2.435, 0.1329, 2.0, 65.1, 3.110],
    [0.2989, 2.851, 0.1731, 2.3, 67.7, 3.367],
    [0.3670, 2.724, 0.1647, 2.2, 68.7, 3.300],
    [0.5858, 2.364, 0.1146, 2.0, 69.5, 3.151],
    [0.3865, 2.664, 0.1608, 2.2, 73.1, 3.321],
    [0.3996, 2.606, 0.1824, 2.2, 74.0, 3.330],
    [0.2065, 3.007, 0.2400, 2.5, 75.0, 3.502],
    [0.3124, 2.782, 0.1561, 2.3, 78.7, 3.326],
    [0.4061, 2.614, 0.1492, 2.2, 79.3, 3.342],
    [0.1308, 3.476, 0.0171, 2.5, 94.0, 3.167],
    [0.1873, 2.962, 0.1558, 2.8, 108.2, 4.053],
    [0.2988, 2.805, 0.1479, 2.5, 134.0, 3.971],
    [0.1440, 3.220, 0.1385, 2.8, 139.2, 4.003],
    [0.3750, 2.592, 0.0676, 2.5, 166.0, 4.065],
    [0.3416, 2.496, 0.1009, 3.0, 331.0, 5.332],
    [0.1243, 3.002, -0.0138, 3.5, 398.4, 5.344],
    [0.1560, 2.926, 0.1203, 3.5, 452.0, 6.057],
    [0.1785, 2.845, 0.0892, 3.5, 485.1, 6.267],
    [0.1351, 2.976, 0.0358, 3.5, 487.2, 5.616],
    [0.1796, 2.840, 0.0395, 3.5, 553.1, 6.281],
];

// ---------------------------------------------------------------------
// Fortran output formats
// ---------------------------------------------------------------------

fn right_justify(s: String, w: usize) -> String {
    if s.len() > w {
        let mut stars = String::new();
        for _ in 0..w {
            stars.push('*');
        }
        return stars;
    }
    let mut out = String::new();
    for _ in 0..(w - s.len()) {
        out.push(' ');
    }
    out.push_str(&s);
    out
}

/// Fortran `1P,Ew.d` (gfortran): `d.dddddE+xx`, right-justified in `w`; a
/// three-digit exponent loses its `E` (`1.00000-100`). The `f32` is widened
/// to `f64` first, which is exact, so the digits are those of the exact
/// binary value correctly rounded, as `gfortran` prints a `REAL*4`.
/// Used by the `:FLT:` format `(1X,1P,5E14.5)` of `LAY` (`:1825`) and the
/// `E11.4`/`E12.5` of its headers.
pub fn fmt_e(x: f32, w: usize, d: usize) -> String {
    let v = f64::from(x);
    if v != v {
        return right_justify(String::from("NaN"), w);
    }
    if v.is_infinite() {
        return right_justify(String::from(if v < 0.0 { "-Infinity" } else { "Infinity" }), w);
    }
    let neg = v < 0.0 || (v == 0.0 && v.is_sign_negative());
    let body = format!("{:.*e}", d, v.abs());
    let (mant, exps) = match body.split_once('e') {
        Some(p) => p,
        None => (body.as_str(), "0"),
    };
    let e: i32 = exps.parse().unwrap_or(0);
    let sign = if e < 0 { '-' } else { '+' };
    let ea = e.abs();
    let tail = if ea < 100 { format!("E{sign}{ea:02}") } else { format!("{sign}{ea:03}") };
    let s = format!("{}{}{}", if neg { "-" } else { "" }, mant, tail);
    right_justify(s, w)
}

/// Fortran `Fw.d`: the leading zero is dropped only when the field is too
/// narrow without it (gfortran), `d = 0` prints the trailing point (`6.`).
pub fn fmt_f(x: f32, w: usize, d: usize) -> String {
    let v = f64::from(x);
    if v != v {
        return right_justify(String::from("NaN"), w);
    }
    if v.is_infinite() {
        return right_justify(String::from(if v < 0.0 { "-Infinity" } else { "Infinity" }), w);
    }
    let mut s = format!("{:.*}", d, v);
    if d == 0 {
        s.push('.');
    }
    if s.len() > w {
        if let Some(rest) = s.strip_prefix("0.") {
            s = format!(".{rest}");
        } else if let Some(rest) = s.strip_prefix("-0.") {
            s = format!("-.{rest}");
        }
    }
    right_justify(s, w)
}

/// Fortran `Iw`.
pub fn fmt_i(n: i64, w: usize) -> String {
    right_justify(format!("{n}"), w)
}

/// The `:FLT:` format `(1X,1P,5E14.5)` of `LAY` (`:1825`, and `MOLIER`'s
/// `:FLTNG:` is [`fnums_f52`]): a list of reals, five per record, each record
/// starting with one blank. An empty list writes one blank record.
fn flt_records(out: &mut Vec<String>, items: &[f32]) {
    if items.is_empty() {
        out.push(String::from(" "));
        return;
    }
    for chunk in items.chunks(5) {
        let mut line = String::from(" ");
        for &v in chunk {
            line.push_str(&fmt_e(v, 14, 5));
        }
        out.push(line);
    }
}

/// The `:INT:` format `(1X,14I5)` (`:1824`, `MOLIER`'s `:INTG:`).
fn int_records(out: &mut Vec<String>, items: &[i64]) {
    if items.is_empty() {
        out.push(String::from(" "));
        return;
    }
    for chunk in items.chunks(14) {
        let mut line = String::from(" ");
        for &v in chunk {
            line.push_str(&fmt_i(v, 5));
        }
        out.push(line);
    }
}

/// `MOLIER`'s `:FLTNG:` format `(1X,14F5.2)` (`:2309`).
fn fnums_f52(out: &mut Vec<String>, items: &[f32]) {
    if items.is_empty() {
        out.push(String::from(" "));
        return;
    }
    for chunk in items.chunks(14) {
        let mut line = String::from(" ");
        for &v in chunk {
            line.push_str(&fmt_f(v, 5, 2));
        }
        out.push(line);
    }
}

fn pad_to(s: &str, n: usize) -> String {
    let mut out: String = s.chars().take(n).collect();
    while out.chars().count() < n {
        out.push(' ');
    }
    out
}

// ---------------------------------------------------------------------
// PMDCON
// ---------------------------------------------------------------------

/// `PMDCON` (`pegs4.mortran:2498-2551`): the physical, mathematical and
/// derived constants of the stand-alone program, in single precision with
/// the Fortran expression order (`ECGS**2` is a product, `RME*C**2/ERGMEV`
/// is `(RME*(C*C))/ERGMEV`). `RM` is **not** the `PRM = 0.5109989461` of
/// the NRC library: it is derived from `RME = 9.1091E-28`, `C = 2.997925E+10`
/// and `ERGMEV` (`:2533-2535`, `:2548`).
pub fn pmdcon() -> Pegs4Constants {
    let pip: f32 = 3.141_592_653_6;
    let c: f32 = 2.997925e10;
    let rme: f32 = 9.1091e-28;
    let hbar: f32 = 1.05450e-27;
    let ecgs: f32 = 4.80298e-10;
    let emks: f32 = 1.60210e-19;
    let an: f32 = 6.02252e23;
    let raddeg: f32 = 180.0 / pip;
    let fsc: f32 = (ecgs * ecgs) / (hbar * c);
    let ergmev: f32 = 1.0e6 * (emks * 1.0e7);
    let r0: f32 = (ecgs * ecgs) / (rme * (c * c));
    let rm: f32 = rme * (c * c) / ergmev;
    let rmt2: f32 = rm * 2.0;
    let rmsq: f32 = rm * rm;
    let a22p9: f32 = raddeg * sqrt(4.0 * pip * an) * (ecgs * ecgs) / ergmev;
    let q: f32 = hbar / (rme * c);
    let a6680: f32 = 4.0 * pip * an * (q * q) * ((0.885f32 * 0.885f32) / (1.167f32 * 1.13f32));
    Pegs4Constants {
        pip,
        c,
        rme,
        hbar,
        ecgs,
        emks,
        an,
        raddeg,
        fsc,
        ergmev,
        r0,
        rmp: rm,
        rmpt2: rmt2,
        rmpsq: rmsq,
        a22p9,
        a6680,
    }
}

// ---------------------------------------------------------------------
// Units 8 and 9
// ---------------------------------------------------------------------

/// The data read from units 8 and 9 (`pegs4.mortran:756-770`): the
/// `PHPAIR` and `RAYLEI` COMMON blocks' tables. Index conventions: the
/// Fortran `PHE(in,iz)` is `phe[(iz-1)*61 + in-1]`, `PRD(in,iz)` is
/// `prd[(iz-1)*17 + in-1]`, `AFAC(i,iz)` is `afac[(iz-1)*100 + i-1]`.
#[derive(Debug, Clone, PartialEq)]
pub struct Pegs4Data {
    /// `NPHE(100)`: number of energies per element.
    pub nphe: Vec<i32>,
    /// `PHE($mxphote,100)`: photon energies (MeV).
    pub phe: Vec<f32>,
    /// `PHD($mxphote,100)`: photo-electric cross sections (barns/atom).
    pub phd: Vec<f32>,
    /// `EKEDGE(100)`: K-edge energies (keV).
    pub ekedge: Vec<f32>,
    /// `PRE($mxpaire)`: pair-production energies.
    pub pre: Vec<f32>,
    /// `PRD($mxpaire,100)`: pair cross sections.
    pub prd: Vec<f32>,
    /// `COHE($mxphote,100)`: coherent cross sections.
    pub cohe: Vec<f32>,
    /// `XVAL(100)`: form-factor abscissae.
    pub xval: Vec<f32>,
    /// `AFAC(100,100)`: atomic form factors.
    pub afac: Vec<f32>,
}

/// `$mxphote` (`:455`) and `$mxpaire` (`:454`).
pub const MXPHOTE: usize = 61;
/// See [`MXPHOTE`].
pub const MXPAIRE: usize = 17;

fn real_token(t: &str) -> Result<f32, String> {
    let fixed = t.replace(['D', 'd'], "e");
    fixed.parse::<f32>().map_err(|_| format!("bad real in list-directed read: {t}"))
}

fn err_string(e: Pegs4Error) -> String {
    match e {
        Pegs4Error::CallExit { code, message } => format!("CALL_EXIT({code}): {message}"),
        Pegs4Error::Fatal(m) => format!("egs_fatal: {m}"),
    }
}

impl Pegs4Data {
    /// `READ(8,*) NPHE, ((PHE..)), ((PHD..)), EKEDGE, PRE, ((PRD..)),
    /// ((COHE..))` and `READ(9,*) XVAL, ((AFAC..))` (`:757-770`) on in-memory
    /// text. Each is **one** list-directed read, so the order is the Fortran
    /// array order (first index fastest) and the rest of the last record is
    /// discarded ([`ListReader`]).
    ///
    /// # Errors
    ///
    /// A message when the text is short or a token is not a number.
    pub fn parse(xsection: &str, formfactor: &str) -> Result<Self, String> {
        let n8 = 100 + MXPHOTE * 100 * 2 + 100 + MXPAIRE + MXPAIRE * 100 + MXPHOTE * 100;
        let mut rd = ListReader::new(xsection);
        let toks = rd.items(n8).map_err(err_string)?;
        let mut it = toks.iter();
        let mut take = |n: usize| -> Result<Vec<f32>, String> {
            let mut v = Vec::with_capacity(n);
            for _ in 0..n {
                v.push(real_token(it.next().ok_or_else(|| String::from("unit 8 data ends early"))?)?);
            }
            Ok(v)
        };
        let nphe_f = take(100)?;
        let nphe: Vec<i32> = nphe_f.iter().map(|&v| v as i32).collect();
        let phe = take(MXPHOTE * 100)?;
        let phd = take(MXPHOTE * 100)?;
        let ekedge = take(100)?;
        let pre = take(MXPAIRE)?;
        let prd = take(MXPAIRE * 100)?;
        let cohe = take(MXPHOTE * 100)?;
        let n9 = 100 + 100 * 100;
        let mut rd9 = ListReader::new(formfactor);
        let toks9 = rd9.items(n9).map_err(err_string)?;
        let mut all9 = Vec::with_capacity(n9);
        for t in &toks9 {
            all9.push(real_token(t)?);
        }
        let xval = all9[..100].to_vec();
        let afac = all9[100..].to_vec();
        Ok(Self { nphe, phe, phd, ekedge, pre, prd, cohe, xval, afac })
    }

    /// An all-zero table set (no element has data).
    pub fn empty() -> Self {
        Self {
            nphe: vec![0; 100],
            phe: vec![0.0; MXPHOTE * 100],
            phd: vec![0.0; MXPHOTE * 100],
            ekedge: vec![0.0; 100],
            pre: vec![0.0; MXPAIRE],
            prd: vec![0.0; MXPAIRE * 100],
            cohe: vec![0.0; MXPHOTE * 100],
            xval: vec![0.0; 100],
            afac: vec![0.0; 100 * 100],
        }
    }
}

// ---------------------------------------------------------------------
// The input deck
// ---------------------------------------------------------------------

/// The `&INP` namelist (`pegs4.mortran:737-743`), restricted to the
/// variables the non-plotting options use. `Some` means the namelist
/// assigns it; arrays assign a prefix (`PZ=2,1` sets `PZ(1)` and `PZ(2)` and
/// leaves the rest), as a Fortran NAMELIST read does. `Pegs4Inp::default()`
/// is `&INP &END`.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Pegs4Inp {
    /// `NE`.
    pub ne: Option<usize>,
    /// `PZ(1..)`, proportions by number.
    pub pz: Option<Vec<f32>>,
    /// `RHO`, g/cm^3.
    pub rho: Option<f32>,
    /// `RHOZ(1..)`, proportions by weight.
    pub rhoz: Option<Vec<f32>>,
    /// `WA(1..)`, atomic-weight overrides.
    pub wa: Option<Vec<f32>>,
    /// `AE` (negative: `-AE*RM`, see `ENER`).
    pub ae: Option<f32>,
    /// `UE`.
    pub ue: Option<f32>,
    /// `AP`.
    pub ap: Option<f32>,
    /// `UP`.
    pub up: Option<f32>,
    /// `XP(1..4)`, arguments of `CALL`.
    pub xp: Option<Vec<f32>>,
    /// `EPE`.
    pub epe: Option<f32>,
    /// `ZTHRE(1..8)`.
    pub zthre: Option<Vec<f32>>,
    /// `ZEPE(1..8)`.
    pub zepe: Option<Vec<f32>>,
    /// `NIPE`.
    pub nipe: Option<i32>,
    /// `NALE`.
    pub nale: Option<i32>,
    /// `EPG`.
    pub epg: Option<f32>,
    /// `ZTHRG(1..4)`.
    pub zthrg: Option<Vec<f32>>,
    /// `ZEPG(1..4)`.
    pub zepg: Option<Vec<f32>>,
    /// `NIPG`.
    pub nipg: Option<i32>,
    /// `NALG`.
    pub nalg: Option<i32>,
    /// `EPR`.
    pub epr: Option<f32>,
    /// `ZTHRR(1)`.
    pub zthrr: Option<f32>,
    /// `ZEPR(1)`.
    pub zepr: Option<f32>,
    /// `NIPR`.
    pub nipr: Option<i32>,
    /// `NALR`.
    pub nalr: Option<i32>,
    /// `GASP`, gas pressure in atmospheres (0: not a gas).
    pub gasp: Option<f32>,
    /// `IUNRST`.
    pub iunrst: Option<i32>,
    /// `IRAYL`.
    pub irayl: Option<i32>,
    /// `AFACT`.
    pub afact: Option<f32>,
    /// `SK`.
    pub sk: Option<f32>,
    /// `X0`.
    pub x0: Option<f32>,
    /// `X1`.
    pub x1: Option<f32>,
    /// `IEV`.
    pub iev: Option<f32>,
    /// `CBAR`.
    pub cbar: Option<f32>,
    /// `ISSB`.
    pub issb: Option<f32>,
    /// `EPSTFL`.
    pub epstfl: Option<i32>,
    /// `IAPRIM`.
    pub iaprim: Option<i32>,
}

/// The cards of an `ELEM`, `MIXT` or `COMP` option after the option name
/// and the namelist (`:884-885`): `MEDIUM` and the Sternheimer id
/// (`(24A1,6X,24A1)`), then the element symbols (`(24(A2,1X))`).
#[derive(Debug, Clone, Default, PartialEq)]
pub struct MediumBlock {
    /// The `&INP` namelist of this block.
    pub inp: Pegs4Inp,
    /// `MEDIUM`, padded or cut to 24 characters.
    pub medium: String,
    /// `IDSTRN`, 24 characters; a blank first character means "copy
    /// `MEDIUM`" (`:886`).
    pub idstrn: String,
    /// `ASYM(1..)`: element symbols, two significant characters each.
    pub asym: Vec<String>,
}

impl MediumBlock {
    /// Read the two cards as `INPUT MEDIUM,IDSTRN; (24A1,6X,24A1)` and
    /// `INPUT (ASYM(I),I=1,NE); (24(A2,1X))` do: the medium name is columns
    /// 1-24, the Sternheimer id columns 31-54, and every three columns of
    /// the symbol card hold one two-character symbol (up to 24 of them;
    /// missing columns read as blanks).
    pub fn from_cards(inp: Pegs4Inp, medium_card: &str, symbol_card: &str) -> Self {
        let chars: Vec<char> = medium_card.chars().collect();
        let take = |from: usize, n: usize| -> String {
            let mut s = String::new();
            for i in from..from + n {
                s.push(chars.get(i).copied().unwrap_or(' '));
            }
            s
        };
        let sym: Vec<char> = symbol_card.chars().collect();
        let mut asym = Vec::new();
        for i in 0..24 {
            let mut s = String::new();
            for j in 0..2 {
                s.push(sym.get(3 * i + j).copied().unwrap_or(' '));
            }
            asym.push(s);
        }
        Self { inp, medium: take(0, 24), idstrn: take(30, 24), asym }
    }
}

/// `CALL` option card (`:1056-1060`): the function name (`(6A1)`).
#[derive(Debug, Clone, Default, PartialEq)]
pub struct CallBlock {
    /// The `&INP` namelist (`XP(1..4)` are the arguments).
    pub inp: Pegs4Inp,
    /// `NAME`, six characters.
    pub name: String,
}

/// One option block of the deck: the option name card and the cards that
/// follow it (`:804-1151`).
#[derive(Debug, Clone, PartialEq)]
pub enum Pegs4Block {
    /// `ELEM`: a one-element medium.
    Elem(MediumBlock),
    /// `MIXT`: a mixture, proportions by weight (`RHOZ`).
    Mixt(MediumBlock),
    /// `COMP`: a compound, proportions by number of atoms (`PZ`).
    Comp(MediumBlock),
    /// `ENER`: the energy limits `AE`, `UE`, `AP`, `UP` (negative values are
    /// multiples of `RM`).
    Ener(Pegs4Inp),
    /// `MIMS`: material-independent multiple-scattering data (`MOLIER`).
    Mims(Pegs4Inp),
    /// `PWLF`: the piecewise-linear fits.
    Pwlf(Pegs4Inp),
    /// `DECK`: write the data set.
    Deck(Pegs4Inp),
    /// `TEST`: only plots upstream; accepted and ignored (not ported).
    Test(Pegs4Inp),
    /// `DBUG`: empty upstream.
    Dbug(Pegs4Inp),
    /// `CALL`: evaluate one function.
    Call(CallBlock),
    /// `PLTN`, `PLTI` or `HPLT` (the option name): plotting, not ported.
    Plot(String, Pegs4Inp),
    /// `STOP` (or the end of the deck).
    Stop,
}

impl Pegs4Block {
    /// The four-character option name.
    pub fn option_name(&self) -> &'static str {
        match self {
            Pegs4Block::Elem(_) => "ELEM",
            Pegs4Block::Mixt(_) => "MIXT",
            Pegs4Block::Comp(_) => "COMP",
            Pegs4Block::Ener(_) => "ENER",
            Pegs4Block::Mims(_) => "MIMS",
            Pegs4Block::Pwlf(_) => "PWLF",
            Pegs4Block::Deck(_) => "DECK",
            Pegs4Block::Test(_) => "TEST",
            Pegs4Block::Dbug(_) => "DBUG",
            Pegs4Block::Call(_) => "CALL",
            Pegs4Block::Plot(_, _) => "PLOT",
            Pegs4Block::Stop => "STOP",
        }
    }

    fn inp(&self) -> Option<&Pegs4Inp> {
        match self {
            Pegs4Block::Elem(m) | Pegs4Block::Mixt(m) | Pegs4Block::Comp(m) => Some(&m.inp),
            Pegs4Block::Ener(i)
            | Pegs4Block::Mims(i)
            | Pegs4Block::Pwlf(i)
            | Pegs4Block::Deck(i)
            | Pegs4Block::Test(i)
            | Pegs4Block::Dbug(i) => Some(i),
            Pegs4Block::Call(c) => Some(&c.inp),
            Pegs4Block::Plot(_, i) => Some(i),
            Pegs4Block::Stop => None,
        }
    }
}

/// The result of one `CALL` option.
#[derive(Debug, Clone, PartialEq)]
pub struct Pegs4CallResult {
    /// The function name as given.
    pub name: String,
    /// `IFUN`, 1-based position in the function table.
    pub ifun: usize,
    /// `XP(1..4)` as passed.
    pub xp: [f32; 4],
    /// `VALUE=FI(IFUN,XP(1),...)`.
    pub value: f32,
}

/// What the program produced.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Pegs4Output {
    /// Unit 7: the data set, one record per entry (no newline), including
    /// the leading carriage-control blank each format writes.
    pub deck: Vec<String>,
    /// A few of the unit 6 messages; not the full listing.
    pub listing: Vec<String>,
    /// The `CALL` results, in order.
    pub calls: Vec<Pegs4CallResult>,
}

impl Pegs4Output {
    /// The unit 7 file text: every record followed by a newline.
    pub fn deck_text(&self) -> String {
        let mut s = String::new();
        for l in &self.deck {
            s.push_str(l);
            s.push('\n');
        }
        s
    }
}

// ---------------------------------------------------------------------
// The program state
// ---------------------------------------------------------------------

/// `COMMON/LAMOLM/C1,C2,CMOLL,T0` (`:334`).
#[derive(Debug, Clone, Copy, Default)]
struct Lamolm {
    c1: f32,
    c2: f32,
    cmoll: f32,
    t0: f32,
}
/// `COMMON/LANIHM/A,C1,C2` (`:342`).
#[derive(Debug, Clone, Copy, Default)]
struct Lanihm {
    a: f32,
    c1: f32,
    c2: f32,
}
/// `COMMON/LBHABM/B1,B2,B3,B4,CBHAB,BETASI,T0` (`:350`).
#[derive(Debug, Clone, Copy, Default)]
struct Lbhabm {
    b1: f32,
    b2: f32,
    b3: f32,
    b4: f32,
    cbhab: f32,
    betasi: f32,
    t0: f32,
}
/// `COMMON/LBREMR/E,LA,LB,LD` (`:358`); `LA`, `LB`, `LD` are 1-based.
#[derive(Debug, Clone, Copy, Default)]
struct Lbremr {
    e: f32,
    la: usize,
    lb: usize,
    ld: usize,
}
/// `COMMON/LCOMPM/K0,CCOMP,C1,C2,C3` (`:375`).
#[derive(Debug, Clone, Copy, Default)]
struct Lcompm {
    k0: f32,
    ccomp: f32,
    c1: f32,
    c2: f32,
    c3: f32,
}
/// `COMMON/LPAIRR/K,LA,LC,LE` (`:383`); the indices are 1-based.
#[derive(Debug, Clone, Copy, Default)]
struct Lpairr {
    k: f32,
    la: usize,
    lc: usize,
    le: usize,
}
/// `COMMON/LPAIRZ/K,CONST,DELC,XLNZ,DELTAM` (`:392`).
#[derive(Debug, Clone, Copy, Default)]
struct Lpairz {
    k: f32,
    const_: f32,
    delc: f32,
    xlnz: f32,
    deltam: f32,
}

/// The `PWLFIN` COMMON block (`:474`), with the `BLOCK DATA` defaults
/// (`:1337-1342`).
#[derive(Debug, Clone, PartialEq)]
pub struct PwlfIn {
    /// `EPE`: allowed relative error, electron fits.
    pub epe: f32,
    /// `ZTHRE(8)`.
    pub zthre: [f32; 8],
    /// `ZEPE(8)`.
    pub zepe: [f32; 8],
    /// `NIPE`.
    pub nipe: i32,
    /// `NALE`.
    pub nale: i32,
    /// `EPG`.
    pub epg: f32,
    /// `ZTHRG(4)`.
    pub zthrg: [f32; 4],
    /// `ZEPG(4)`.
    pub zepg: [f32; 4],
    /// `NIPG`.
    pub nipg: i32,
    /// `NALG`.
    pub nalg: i32,
    /// `EPR`.
    pub epr: f32,
    /// `ZTHRR(1)`.
    pub zthrr: [f32; 1],
    /// `ZEPR(1)`.
    pub zepr: [f32; 1],
    /// `NIPR`.
    pub nipr: i32,
    /// `NALR`.
    pub nalr: i32,
}

impl Default for PwlfIn {
    fn default() -> Self {
        Self {
            epe: 0.01,
            zthre: [0.0; 8],
            zepe: [0.0; 8],
            nipe: 20,
            nale: MXEKE as i32,
            epg: 0.01,
            zthrg: [0.0, 0.1, 0.0, 0.0],
            zepg: [0.0, 0.01, 0.0, 0.0],
            nipg: 20,
            nalg: MXGE as i32,
            epr: 0.01,
            zthrr: [0.0],
            zepr: [0.0],
            nipr: 20,
            nalr: MXRL as i32,
        }
    }
}

/// The `MIMSD` tables `MOLIER` fills (`:408`), with the `BLOCK DATA`
/// values (`:1422-1424`).
#[derive(Debug, Clone, PartialEq)]
pub struct Mimsd {
    /// `MSTEPS`.
    pub msteps: usize,
    /// `JRMAX`.
    pub jrmax: usize,
    /// `MXV1`.
    pub mxv1: usize,
    /// `MXV2`.
    pub mxv2: usize,
    /// `MSMAP($MXJREFF)`.
    pub msmap: Vec<i32>,
    /// `FSTEP($MSSTEPS)`.
    pub fstep: [f32; MSSTEPS],
    /// `FSQR($MSSTEPS)`.
    pub fsqr: [f32; MSSTEPS],
    /// `VERT1($MXVRT1)`.
    pub vert1: Vec<f32>,
    /// `VERT2`, per step, `$MXVRT1` entries each (upstream dimensions this
    /// `$MXVRT2 = 200` and `MXV2` can exceed it; see the module notes).
    pub vert2: Vec<Vec<f32>>,
}

impl Default for Mimsd {
    fn default() -> Self {
        Self {
            msteps: MSSTEPS,
            jrmax: MXJREFF,
            mxv1: 0,
            mxv2: 0,
            msmap: vec![0; MXJREFF],
            fstep: [1., 2., 3., 4., 6., 8., 10., 15., 20., 30., 40., 60., 80., 100., 150., 200.],
            fsqr: [0.0; MSSTEPS],
            vert1: vec![0.0; MXVRT1],
            vert2: vec![vec![0.0; MXVRT1]; MSSTEPS],
        }
    }
}

/// The PEGS4 program state: the COMMON blocks, the `MEDSET`/`ENGSET` flags
/// and the data of units 8, 9, 20 and 22. Build with [`Pegs4Program::new`]
/// and drive with [`Pegs4Program::run`].
#[derive(Debug, Clone)]
pub struct Pegs4Program {
    /// The state shared with `pegs4_routines.mortran` (thresholds, `MOLVAR`,
    /// `MIXDAT` numbers, `BREMPR`, `DBRPR`, `LSPION`, `EPSTAR`, `RADLEN`,
    /// the `LBREMZ` block of `BRMSDZ`/`BRMSFZ`, the constants of `PMDCON`).
    pub c: Pegs4Common,
    /// Units 8 and 9.
    pub data: Pegs4Data,
    /// Unit 20: the text of the density-effect file, read when
    /// `EPSTFL = 1`.
    pub density_text: Option<String>,
    /// `Z(1..$MXEL)` (`MIXDAT`).
    pub z: [f32; MXEL],
    /// `PZ`.
    pub pz: [f32; MXEL],
    /// `WA`.
    pub wa: [f32; MXEL],
    /// `RHOZ`.
    pub rhoz: [f32; MXEL],
    /// `NE`.
    pub ne: usize,
    /// `MTYP` (the option name of the last medium).
    pub mtyp: String,
    /// `MEDIUM`, 24 characters.
    pub medium: String,
    /// `IDSTRN`, 24 characters.
    pub idstrn: String,
    /// `ASYM(1..NE)`.
    pub asym: Vec<String>,
    /// `ISSB` (`LSPION`): the user supplied the Sternheimer parameters.
    pub issb: f32,
    /// `TET2` (`THRESH`).
    pub tet2: f32,
    /// `TEM` (`THRESH`).
    pub tem: f32,
    /// `MEDSET`: a medium has been defined (`:934`).
    pub medset: bool,
    /// `ENGSET`: `ENER` has been given.
    pub engset: bool,
    /// `IRAYL`.
    pub irayl: i32,
    /// `XP(1..4)` of `&INP`.
    pub xp: [f32; 4],
    /// `PWLFIN`.
    pub pwlf: PwlfIn,
    /// `EBINDA` (`RSLTS`).
    pub ebinda: f32,
    /// `NEL`.
    pub nel: usize,
    /// `AXE`.
    pub axe: f32,
    /// `BXE`.
    pub bxe: f32,
    /// `AFE($MXEKE,8)`, `AFE(i,k)` at `(i-1) + (k-1)*MXEKE`.
    pub afe: Vec<f32>,
    /// `BFE`.
    pub bfe: Vec<f32>,
    /// `NGL`.
    pub ngl: usize,
    /// `AXG`.
    pub axg: f32,
    /// `BXG`.
    pub bxg: f32,
    /// `AFG($MXGE,4)`.
    pub afg: Vec<f32>,
    /// `BFG`.
    pub bfg: Vec<f32>,
    /// `NGR`.
    pub ngr: usize,
    /// `AXR`.
    pub axr: f32,
    /// `BXR`.
    pub bxr: f32,
    /// `AFR($MXRL)`.
    pub afr: Vec<f32>,
    /// `BFR`.
    pub bfr: Vec<f32>,
    /// `XVAL(100)` of `RAYLEI` (squared in place by `PWLF`).
    pub xval: Vec<f32>,
    /// `AFAC2(100)`.
    pub afac2: Vec<f32>,
    /// `AFFI(100)`.
    pub affi: Vec<f32>,
    /// `MIMSD`.
    pub mims: Mimsd,
    lamolm: Lamolm,
    lanihm: Lanihm,
    lbhabm: Lbhabm,
    lbremr: Lbremr,
    lcompm: Lcompm,
    lpairr: Lpairr,
    lpairz: Lpairz,
}

impl Pegs4Program {
    /// The state after `BLOCK DATA` (`:1160-1428`) and `PMDCON`
    /// (`:751`): `IUNRST = 0`, `IRAYL = 0`, `GASP = 0`, `EPSTFL = 0`,
    /// `IAPRIM = 1`, the density-effect parameters 0, the `PWLF` defaults,
    /// the element-symbol table. `data` is units 8 and 9.
    pub fn new(data: Pegs4Data) -> Self {
        let mut c = Pegs4Common::new(pmdcon());
        c.asymt = ASYMT.iter().map(|s| String::from(*s)).collect();
        let xval = data.xval.clone();
        Self {
            c,
            data,
            density_text: None,
            z: [0.0; MXEL],
            pz: [0.0; MXEL],
            wa: [0.0; MXEL],
            rhoz: [0.0; MXEL],
            ne: 0,
            mtyp: String::new(),
            medium: pad_to("", LMED),
            idstrn: pad_to("", LMED),
            asym: Vec::new(),
            issb: 0.0,
            tet2: 0.0,
            tem: 0.0,
            medset: false,
            engset: false,
            irayl: 0,
            xp: [0.0; 4],
            pwlf: PwlfIn::default(),
            ebinda: 0.0,
            nel: 0,
            axe: 0.0,
            bxe: 0.0,
            afe: vec![0.0; MXEKE * 8],
            bfe: vec![0.0; MXEKE * 8],
            ngl: 0,
            axg: 0.0,
            bxg: 0.0,
            afg: vec![0.0; MXGE * 4],
            bfg: vec![0.0; MXGE * 4],
            ngr: 0,
            axr: 0.0,
            bxr: 0.0,
            afr: vec![0.0; MXRL],
            bfr: vec![0.0; MXRL],
            xval,
            afac2: vec![0.0; 100],
            affi: vec![0.0; 100],
            mims: Mimsd::default(),
            lamolm: Lamolm::default(),
            lanihm: Lanihm::default(),
            lbhabm: Lbhabm::default(),
            lbremr: Lbremr::default(),
            lcompm: Lcompm::default(),
            lpairr: Lpairr::default(),
            lpairz: Lpairz::default(),
        }
    }

    /// `RM`, the electron rest energy of `PMDCON`.
    fn rm(&self) -> f32 {
        self.c.k.rmp
    }
}

/// `DL1..DL6(L)` screening-fit value at `delta` (`BREMFR` `:3537-3557`,
/// `PAIRFR` `:4781-4801`): the quadratic fit up to `delta = 1`, the
/// logarithmic one above. `l` is the 1-based level.
fn screening_fit(c: &Pegs4Common, delta: f32, l: usize) -> f32 {
    let i = l - 1;
    if delta <= 1.0 {
        c.dlp[0][i] + delta * (c.dlp[1][i] + delta * c.dlp[2][i])
    } else {
        c.dlp[3][i] + c.dlp[4][i] * ln(delta + c.dlp[5][i])
    }
}

fn bad_iunrst(n: i32) -> Pegs4Error {
    Pegs4Error::CallExit { code: 20, message: format!("IUNRST={n} outside 0..7: SPTOTE/SPTOTP leave the result unset") }
}

impl Pegs4Program {
    // -----------------------------------------------------------------
    // Distribution-function helpers
    // -----------------------------------------------------------------

    /// `ALIN` (`pegs4.mortran:3037-3046`): the linear distribution function.
    pub fn alin(&self, x: f32) -> f32 {
        x
    }

    /// `ALINI` (`:3051-3058`): the inverse of [`Self::alin`].
    pub fn alini(&self, x: f32) -> f32 {
        x
    }

    /// `AREC` (`:3375-3384`): `1/X`, the derivative of the logarithm.
    pub fn arec(&self, x: f32) -> f32 {
        1.0 / x
    }

    /// `ADFMOL` (`:2944-2955`): approximate c.d.f. for Moller and Bhabha,
    /// `-1/(X-RM)`.
    pub fn adfmol(&self, x: f32) -> f32 {
        -1.0 / (x - self.rm())
    }

    /// `ADIMOL` (`:2960-2971`): the inverse of [`Self::adfmol`], `-1/X+RM`.
    pub fn adimol(&self, x: f32) -> f32 {
        -1.0 / x + self.rm()
    }

    /// `ADDMOL` (`:2928-2939`): the derivative of [`Self::adfmol`],
    /// `1/(X-RM)**2`.
    pub fn addmol(&self, x: f32) -> f32 {
        let d = x - self.rm();
        1.0 / (d * d)
    }

    /// `AFFACT` (`:2976-2986`): `AINTP` of the summed squared form factor
    /// `AFAC2` against `XVAL`, log-log.
    pub fn affact(&self, x: f32) -> f32 {
        aintp(x, &self.xval, 100, &self.afac2, 1, true, true)
    }

    // -----------------------------------------------------------------
    // Moller, Bhabha, annihilation
    // -----------------------------------------------------------------

    /// `AMOLDM` (`:3093-3113`): differential Moller cross section for an
    /// incident electron of total energy `en0` giving a scattered electron
    /// of total energy `en`; sets `LAMOLM` for [`Self::amolfm`].
    pub fn amoldm(&mut self, en0: f32, en: f32) -> f32 {
        let k = self.c.k;
        let rm = k.rmp;
        let t0 = en0 - rm;
        let tm = t0 / rm;
        let em = tm + 1.0;
        let c1 = (tm / em) * (tm / em);
        let c2 = (2.0 * tm + 1.0) / (em * em);
        let betasq = 1.0 - 1.0 / (em * em);
        let cmoll = self.c.rlcp * self.c.eden * 2.0 * k.pip * (k.r0 * k.r0) / (betasq * t0 * tm);
        self.lamolm = Lamolm { c1, c2, cmoll, t0 };
        self.amolfm(en)
    }

    /// `AMOLFM` (`:3118-3134`): the Moller function of the scattered total
    /// energy, relying on `LAMOLM` set by [`Self::amoldm`] (all zero before).
    pub fn amolfm(&self, en: f32) -> f32 {
        let l = self.lamolm;
        let t = en - self.rm();
        let eps = t / l.t0;
        let epsp = 1.0 - eps;
        let epsi = 1.0 / eps;
        let epspi = 1.0 / epsp;
        l.cmoll * (l.c1 + epsi * (epsi - l.c2) + epspi * (epspi - l.c2))
    }

    /// `ANIHDM` (`:3191-3209`): differential two-photon annihilation cross
    /// section for a positron of total energy `e0` producing a photon of
    /// energy `k`; sets `LANIHM`.
    pub fn anihdm(&mut self, e0: f32, k: f32) -> f32 {
        let kk = self.c.k;
        let gam = e0 / kk.rmp;
        let a = gam + 1.0;
        let t0p = gam - 1.0;
        let c1 = self.c.rlcp * self.c.eden * kk.pip * (kk.r0 * kk.r0) / (a * t0p * kk.rmp);
        let c2 = a + 2.0 * gam / a;
        self.lanihm = Lanihm { a, c1, c2 };
        self.anihfm(k)
    }

    /// `ANIHFM` (`:3214-3227`): the annihilation function of the photon
    /// energy (`S1(X) = C1*(-1+(C2-1/X)/X)`), relying on `LANIHM`.
    pub fn anihfm(&self, k: f32) -> f32 {
        let l = self.lanihm;
        let s1 = |x: f32| l.c1 * (-1.0 + (l.c2 - 1.0 / x) / x);
        let kp = k / self.rm();
        s1(kp) + s1(l.a - kp)
    }

    /// `ANIHRM` (`:3232-3254`): annihilation cross section for photons in
    /// `k1..k2` (`S2(X) = RM*C1*(-X+C2*ln X+1/X)`).
    pub fn anihrm(&self, e0: f32, k1: f32, k2: f32) -> f32 {
        let kk = self.c.k;
        let rm = kk.rmp;
        let gam = e0 / rm;
        let kp1 = k1 / rm;
        let kp2 = k2 / rm;
        let a = gam + 1.0;
        let t0p = gam - 1.0;
        let c1 = self.c.rlcp * self.c.eden * kk.pip * (kk.r0 * kk.r0) / (a * t0p * rm);
        let c2 = a + 2.0 * gam / a;
        let s2 = |x: f32| rm * c1 * (-x + c2 * ln(x) + 1.0 / x);
        s2(kp2) - s2(kp1) + s2(a - kp1) - s2(a - kp2)
    }

    /// `BHABDM` (`:3389-3412`): differential Bhabha cross section; sets
    /// `LBHABM`.
    pub fn bhabdm(&mut self, en0: f32, en: f32) -> f32 {
        let kk = self.c.k;
        let rm = kk.rmp;
        let t0 = en0 - rm;
        let tm = t0 / rm;
        let em = tm + 1.0;
        let y = 1.0 / (tm + 2.0);
        let betasi = 1.0 / (1.0 - 1.0 / (em * em));
        let cbhab = self.c.rlcp * self.c.eden * 2.0 * kk.pip * (kk.r0 * kk.r0) / (t0 * tm);
        let b1 = 2.0 - y * y;
        let b2 = 3.0 - y * (6.0 - y * (1.0 - y * 2.0));
        let b3 = 2.0 - y * (10.0 - y * (16.0 - y * 8.0));
        let b4 = 1.0 - y * (6.0 - y * (12.0 - y * 8.0));
        self.lbhabm = Lbhabm { b1, b2, b3, b4, cbhab, betasi, t0 };
        self.bhabfm(en)
    }

    /// `BHABFM` (`:3417-3431`): the Bhabha function of the scattered total
    /// energy, relying on `LBHABM`.
    pub fn bhabfm(&self, en: f32) -> f32 {
        let l = self.lbhabm;
        let t = en - self.rm();
        let eps = t / l.t0;
        let epsi = 1.0 / eps;
        l.cbhab * (epsi * (epsi * l.betasi - l.b1) + l.b2 + eps * (eps * l.b4 - l.b3))
    }

    // -----------------------------------------------------------------
    // Bremsstrahlung (run-time, per molecule)
    // -----------------------------------------------------------------

    /// `BREMDR` (`:3488-3509`): run-time bremsstrahlung `dsigma/dk`
    /// (SLAC-265 eq. 2.7.108); selects the `LBREMR` level (`LD = 2`, `LA = 4`
    /// at `E >= 50`) and returns [`Self::bremfr`].
    pub fn bremdr(&mut self, ea: f32, k: f32) -> f32 {
        let (ld, ls) = if ea >= 50.0 { (2usize, 3usize) } else { (1usize, 0usize) };
        self.lbremr = Lbremr { e: ea, la: ls + 1, lb: ls + 2, ld };
        self.bremfr(k)
    }

    /// `BREMFR` (`:3537-3557`): the function [`Self::bremrr`] integrates,
    /// relying on `LBREMR`. Zero beyond `DELPOS(LD)`.
    pub fn bremfr(&self, k: f32) -> f32 {
        let l = self.lbremr;
        let e = l.e;
        let eps = k / e;
        let del = eps / (e * (1.0 - eps));
        if del > self.c.delposp[l.ld - 1] {
            return 0.0;
        }
        let delta = self.c.delcmp * del;
        let a = screening_fit(&self.c, delta, l.la);
        let b = screening_fit(&self.c, delta, l.lb);
        (self.c.alphip[l.ld - 1] * (1.0 - eps) / eps / self.c.al2 * a + 0.5 * (2.0 * eps) * b) / e
    }

    /// `BREMRR` (`:3576-3605`): `QD` of [`Self::bremfr`] over `k1..k2` after
    /// the `BREMDR` initialisation call.
    pub fn bremrr(&mut self, e: f32, k1: f32, k2: f32) -> f32 {
        let _dummy = self.bremdr(e, k1);
        let this = &*self;
        qd(|k| this.bremfr(k), k1, k2).value
    }

    /// `BREMTR` (`:3678-3689`): total bremsstrahlung cross section from the
    /// run-time functions, zero at or below `AP + RM`.
    pub fn bremtr(&mut self, e0: f32) -> f32 {
        if e0 <= self.c.app + self.rm() {
            0.0
        } else {
            let (ap, rm) = (self.c.app, self.rm());
            self.bremrr(e0, ap, e0 - rm)
        }
    }

    // -----------------------------------------------------------------
    // Compton and coherent
    // -----------------------------------------------------------------

    /// `COMPDM` (`:3851-3869`): differential Compton cross section for
    /// photon energy `k0a` to scatter to `k`; sets `LCOMPM`.
    pub fn compdm(&mut self, k0a: f32, k: f32) -> f32 {
        let kk = self.c.k;
        let k0 = k0a;
        let k0p = k0 / kk.rmp;
        let ccomp = self.c.rlcp * self.c.eden * kk.pip * (kk.r0 * kk.r0) / (k0 * k0p);
        let c1 = 1.0 / (k0p * k0p);
        let c2 = 1.0 - (2.0 + 2.0 * k0p) / (k0p * k0p);
        let c3 = (1.0 + 2.0 * k0p) / (k0p * k0p);
        self.lcompm = Lcompm { k0, ccomp, c1, c2, c3 };
        self.compfm(k)
    }

    /// `COMPFM` (`:3873-3885`): the Compton function of the scattered
    /// energy, relying on `LCOMPM`.
    pub fn compfm(&self, k: f32) -> f32 {
        let l = self.lcompm;
        let eps = k / l.k0;
        let epsi = 1.0 / eps;
        l.ccomp * ((l.c1 * epsi + l.c2) * epsi + l.c3 + eps)
    }

    /// `COMPRM` (`:3890-3911`): Compton cross section into `k1..k2`. The
    /// constants and `EPS` are **`REAL*8`** upstream (`:3900`): `K0P = K0/RM`
    /// is a single-precision quotient widened, `CCOMP2` is the single-precision
    /// product `RLC*EDEN*PI*R0**2` divided by `K0P` in double and narrowed,
    /// and the bracket is evaluated in double and narrowed once at the end.
    pub fn comprm(&self, k0: f32, k1: f32, k2: f32) -> f32 {
        let kk = self.c.k;
        let k0p: f64 = f64::from(k0 / kk.rmp);
        let ccomp2: f32 = (f64::from(self.c.rlcp * self.c.eden * kk.pip * (kk.r0 * kk.r0)) / k0p) as f32;
        let c1: f64 = 1.0 / (k0p * k0p);
        let c2: f64 = 1.0 - (2.0 + 2.0 * k0p) / (k0p * k0p);
        let c3: f64 = (1.0 + 2.0 * k0p) / (k0p * k0p);
        let eps1: f64 = f64::from(k1 / k0);
        let eps2: f64 = f64::from(k2 / k0);
        let v: f64 = f64::from(ccomp2)
            * (c1 * (1.0 / eps1 - 1.0 / eps2) + c2 * RealMath::r_ln(eps2 / eps1) + eps2 * (c3 + 0.5 * eps2)
                - eps1 * (c3 + 0.5 * eps1));
        v as f32
    }

    /// `COMPTM` (`:3916-3929`): total Compton cross section,
    /// `K1 = K0*RM/(RM+2*K0)` (SLAC-265 eq. 2.9.7).
    pub fn comptm(&self, k0: f32) -> f32 {
        let rm = self.rm();
        let k1 = k0 * rm / (rm + 2.0 * k0);
        self.comprm(k0, k1, k0)
    }

    /// `COHETZ` (`:3829-3847`): coherent (Rayleigh) cross section of one
    /// element in `1/RLC`, from the unit 8 table by log-log interpolation.
    /// `NaN` for `Z` outside 1..100 or fewer than two table energies
    /// (upstream indexes outside the array).
    pub fn cohetz(&self, z: f32, k: f32) -> f32 {
        let kk = self.c.k;
        let pcon: f32 = 1.0e-24 * (kk.an * self.c.rhop / self.c.wm) * self.c.rlcp;
        let iz = z as usize;
        if iz < 1 || iz > 100 || self.data.nphe[iz - 1] < 2 {
            return f32::NAN;
        }
        let off = (iz - 1) * MXPHOTE;
        pcon * aintp(k, &self.data.phe[off..], self.data.nphe[iz - 1] as usize, &self.data.cohe[off..], 1, true, true)
    }

    /// `COHETM` (`:3811-3824`): the mix of [`Self::cohetz`] over the elements.
    pub fn cohetm(&self, k: f32) -> f32 {
        let mut s = 0.0f32;
        for i in 0..self.c.nep {
            s += self.c.pzp[i] * self.cohetz(self.c.zelemp[i], k);
        }
        s
    }

    /// `PHOTTZ` (`:5092-5112`): photo-electric cross section of one element
    /// (log-log interpolation of unit 8), see [`Self::cohetz`] for `NaN`.
    pub fn phottz(&self, z: f32, k: f32) -> f32 {
        let kk = self.c.k;
        let pcon: f32 = 1.0e-24 * (kk.an * self.c.rhop / self.c.wm) * self.c.rlcp;
        let iz = z as usize;
        if iz < 1 || iz > 100 || self.data.nphe[iz - 1] < 2 {
            return f32::NAN;
        }
        let off = (iz - 1) * MXPHOTE;
        pcon * aintp(k, &self.data.phe[off..], self.data.nphe[iz - 1] as usize, &self.data.phd[off..], 1, true, true)
    }

    /// `PHOTTE` (`:5073-5087`): the mix of [`Self::phottz`] over the elements.
    pub fn photte(&self, k: f32) -> f32 {
        let mut s = 0.0f32;
        for i in 0..self.c.nep {
            s += self.c.pzp[i] * self.phottz(self.c.zelemp[i], k);
        }
        s
    }

    /// `EBIND` (`:4429-4446`): the photo-electric-weighted average K-edge
    /// binding energy in MeV (`EKEDGE` is in keV: `*0.001`).
    pub fn ebind(&self, e: f32) -> f32 {
        let mut eb = 0.0f32;
        for i in 0..self.c.nep {
            let z = self.c.zelemp[i];
            let j = z as usize;
            let edge = if (1..=100).contains(&j) { self.data.ekedge[j - 1] } else { f32::NAN };
            eb = eb + self.c.pzp[i] * self.phottz(z, e) * edge * 0.001;
        }
        let stot = self.photte(e);
        if stot != 0.0 {
            eb /= stot;
        }
        eb
    }

    // -----------------------------------------------------------------
    // Pair production
    // -----------------------------------------------------------------

    /// `PAIRDR` (`:4728-4747`): run-time pair `dsigma/dE-`; sets `LPAIRR`
    /// (level `LE = 2`, `LA = 4`, `LC = 6` at `K >= 50`).
    pub fn pairdr(&mut self, ka: f32, e: f32) -> f32 {
        let (le, ls) = if ka < 50.0 { (1usize, 0usize) } else { (2usize, 3usize) };
        self.lpairr = Lpairr { k: ka, la: ls + 1, lc: ls + 3, le };
        self.pairfr(e)
    }

    /// `PAIRFR` (`:4781-4801`): the function [`Self::pairrr`] integrates,
    /// relying on `LPAIRR`.
    pub fn pairfr(&self, e: f32) -> f32 {
        let l = self.lpairr;
        let k = l.k;
        let eps = e / k;
        let del = 1.0 / (k * eps * (1.0 - eps));
        if del > self.c.delposp[l.le - 1] {
            return 0.0;
        }
        let delta = self.c.delcmp * del;
        let a = screening_fit(&self.c, delta, l.la);
        let cc = screening_fit(&self.c, delta, l.lc);
        let q = e / k - 0.5;
        (self.c.alfp1[l.le - 1] * cc + self.c.alfp2[l.le - 1] * 12.0 * (q * q) * a) / k
    }

    /// `PAIRRR` (`:4858-4889`): `QD` of [`Self::pairfr`] over `e1..e2` after
    /// the `PAIRDR` initialisation.
    pub fn pairrr(&mut self, k: f32, e1: f32, e2: f32) -> f32 {
        let _dummy = self.pairdr(k, e1);
        let this = &*self;
        qd(|e| this.pairfr(e), e1, e2).value
    }

    /// `PAIRDZ` (`:4752-4776`): initialises `LPAIRZ` for element `z` at
    /// photon energy `ka` (SLAC-265 eq. 2.7.8) and returns
    /// [`Self::pairfz`]`(e)`.
    pub fn pairdz(&mut self, z: f32, ka: f32, e: f32) -> f32 {
        let kk = self.c.k;
        let k = ka;
        let delc = 136.0 * powf(z, -1.0 / 3.0) * kk.rmp / k;
        let const_ = (kk.an * self.c.rhop / self.c.wm)
            * (kk.r0 * kk.r0)
            * kk.fsc
            * z
            * (z + self.c.xsifp(z))
            * self.c.rlcp
            / (k * k * k);
        let mut xlnz = 4.0 / 3.0 * ln(z);
        if k >= 50.0 {
            xlnz += 4.0 * self.c.fcoulcp(z);
        }
        // DELTAM IS THE DELTA AT WHICH THE SQUARE BRACKETS GO TO ZERO (:4752-4776)
        let deltam = exp((21.12 - xlnz) / 4.184) - 0.952;
        self.lpairz = Lpairz { k, const_, delc, xlnz, deltam };
        self.pairfz(e)
    }

    /// `PAIRFZ` (`:4806-4836`): the function [`Self::pairrz`] integrates,
    /// relying on `LPAIRZ`. `1 - EPS = 0` is replaced by `1.18E-38`.
    pub fn pairfz(&self, e: f32) -> f32 {
        let l = self.lpairz;
        let eps = e / l.k;
        let mut oneeps = 1.0 - eps;
        if oneeps == 0.0 {
            oneeps = 1.18e-38;
        }
        let delta = l.delc / (eps * oneeps);
        if delta >= l.deltam {
            return 0.0;
        }
        let sb1;
        let sb2;
        if delta <= 1.0 {
            sb1 = 20.867 + delta * (-3.242 + delta * 0.625) - l.xlnz;
            sb2 = 20.209 + delta * (-1.930 + delta * (-0.086)) - l.xlnz;
        } else {
            sb1 = 21.12 - 4.184 * ln(delta + 0.952) - l.xlnz;
            sb2 = sb1;
        }
        let eplus = l.k - e;
        l.const_ * ((e * e + eplus * eplus) * sb1 + 0.666667 * e * eplus * sb2)
    }

    /// `PAIRRZ` (`:4894-4926`): `QD` of [`Self::pairfz`] over `e1..e2` after
    /// the `PAIRDZ` initialisation.
    pub fn pairrz(&mut self, z: f32, k: f32, e1: f32, e2: f32) -> f32 {
        let _dummy = self.pairdz(z, k, e1);
        let this = &*self;
        qd(|e| this.pairfz(e), e1, e2).value
    }

    /// `PAIRRM` (`:4841-4853`): the mix of [`Self::pairrz`] over the elements.
    pub fn pairrm(&mut self, k: f32, e1: f32, e2: f32) -> f32 {
        let mut s = 0.0f32;
        for i in 0..self.c.nep {
            let (z, pz) = (self.c.zelemp[i], self.c.pzp[i]);
            s += pz * self.pairrz(z, k, e1, e2);
        }
        s
    }

    /// `PAIRTZ` (`:4999-5022`): tabulated pair cross section of one element
    /// (barns converted to `1/RLC`, log-x linear-y interpolation of `PRE`,
    /// `PRD`), zero at or below `2*RM`.
    pub fn pairtz(&self, z: f32, k: f32) -> f32 {
        if k <= self.c.k.rmpt2 {
            return 0.0;
        }
        let kk = self.c.k;
        let pcon: f32 = 1.0e-24 * (kk.an * self.c.rhop / self.c.wm) * self.c.rlcp;
        let iz = z as usize;
        if iz < 1 || iz > 100 {
            return f32::NAN;
        }
        pcon * aintp(k, &self.data.pre, MXPAIRE, &self.data.prd[(iz - 1) * MXPAIRE..], 1, true, false)
    }

    /// `PAIRTE` (`:4931-4946`): the empirical total pair cross section of
    /// the medium, zero at or below `2*RM`.
    pub fn pairte(&self, k: f32) -> f32 {
        if k <= 2.0 * self.rm() {
            return 0.0;
        }
        let mut s = 0.0f32;
        for i in 0..self.c.nep {
            s += self.c.pzp[i] * self.pairtz(self.c.zelemp[i], k);
        }
        s
    }

    /// `PAIRTM` (`:4951-4962`): the theoretical total pair cross section,
    /// `PAIRRM(K0,RM,K0-RM)`.
    pub fn pairtm(&mut self, k0: f32) -> f32 {
        let rm = self.rm();
        if k0 <= 2.0 * rm {
            0.0
        } else {
            self.pairrm(k0, rm, k0 - rm)
        }
    }

    /// `PAIRTR` (`:4967-4978`): as [`Self::pairtm`] from the run-time
    /// functions.
    pub fn pairtr(&mut self, k0: f32) -> f32 {
        let rm = self.rm();
        if k0 <= 2.0 * rm {
            0.0
        } else {
            self.pairrr(k0, rm, k0 - rm)
        }
    }

    /// `PAIRTU` (`:4983-4994`): the pair cross section actually used: the
    /// empirical one below 50 MeV, the theoretical above.
    pub fn pairtu(&mut self, k: f32) -> f32 {
        if k < 50.0 {
            self.pairte(k)
        } else {
            self.pairtm(k)
        }
    }

    // -----------------------------------------------------------------
    // Derived functions
    // -----------------------------------------------------------------

    /// `CRATIO` (`:3933-3942`): the Rayleigh correction (coherent ratio).
    pub fn cratio(&mut self, e: f32) -> f32 {
        let tot = self.pairtu(e) + self.comptm(e) + self.photte(e);
        tot / (tot + self.cohetm(e))
    }

    /// `GMFP` (`:4562-4571`): photon mean free path (radiation lengths).
    pub fn gmfp(&mut self, e: f32) -> f32 {
        1.0 / (self.pairtu(e) + self.comptm(e) + self.photte(e))
    }

    /// `GBR1` (`:4532-4542`): branching ratio pair/(pair+Compton+photo).
    pub fn gbr1(&mut self, e: f32) -> f32 {
        let pair = self.pairtu(e);
        pair / (pair + self.comptm(e) + self.photte(e))
    }

    /// `GBR2` (`:4547-4557`): branching ratio (pair+Compton)/(all three).
    pub fn gbr2(&mut self, e: f32) -> f32 {
        let prco = self.pairtu(e) + self.comptm(e);
        prco / (prco + self.photte(e))
    }

    /// `GFUNS` (`:1680-1708`): the four photon functions `PWLF1` fits:
    /// `[GMFP, GBR1, GBR2, CRATIO]` (the coherent ratio `TSANSC/(TSANSC +
    /// COHR)`).
    pub fn gfuns(&mut self, e: f32) -> [f32; 4] {
        let pair = self.pairtu(e);
        let comp = self.comptm(e);
        let phot = self.photte(e);
        let cohr = self.cohetm(e);
        let tsansc = pair + comp + phot;
        let gmfp = 1.0 / tsansc;
        [gmfp, pair * gmfp, (pair + comp) * gmfp, tsansc / (tsansc + cohr)]
    }

    /// `RFUNS` (`:2656-2667`): the Rayleigh function `PWLF1` fits,
    /// `AINTP(E,AFFI,87,XVAL,1,.TRUE.,.TRUE.)`.
    pub fn rfuns(&self, e: f32) -> f32 {
        aintp(e, &self.affi, 87, &self.xval, 1, true, true)
    }

    /// `TMXDE2` (`:5397-5410`): `TMXB(E)/(E**2*(1-RM**2/E**2)**2)`.
    pub fn tmxde2(&self, e: f32) -> f32 {
        let esq = e * e;
        let betasq = 1.0 - self.c.k.rmpsq / esq;
        self.c.tmxb(e) / (esq * (betasq * betasq))
    }

    /// `EBR1` (`:4451-4463`): branching ratio brems/(brems+Moller).
    pub fn ebr1(&mut self, e: f32) -> Result<f32, Pegs4Error> {
        let brem = self.c.bremtm(e)?;
        let tebr = brem + self.c.amoltm(e);
        Ok(if tebr > 0.0 { brem / tebr } else { 0.0 })
    }

    /// `ESIG` (`:4484-4492`): electron cross section (1/RL), brems + Moller.
    pub fn esig(&mut self, e: f32) -> Result<f32, Pegs4Error> {
        Ok(self.c.bremtm(e)? + self.c.amoltm(e))
    }

    /// `PSIG` (`:5117-5125`): positron cross section, brems + Bhabha +
    /// annihilation.
    pub fn psig(&mut self, e: f32) -> Result<f32, Pegs4Error> {
        Ok(self.c.bremtm(e)? + self.c.bhabtm(e) + self.c.anihtm(e))
    }

    /// `PBR1` (`:5027-5037`): branching ratio brems/(brems+Bhabha+annih).
    pub fn pbr1(&mut self, e: f32) -> Result<f32, Pegs4Error> {
        let brem = self.c.bremtm(e)?;
        Ok(brem / (brem + self.c.bhabtm(e) + self.c.anihtm(e)))
    }

    /// `PBR2` (`:5042-5052`): branching ratio (brems+Bhabha)/(all three).
    pub fn pbr2(&mut self, e: f32) -> Result<f32, Pegs4Error> {
        let brbh = self.c.bremtm(e)? + self.c.bhabtm(e);
        Ok(brbh / (brbh + self.c.anihtm(e)))
    }

    /// `EDEDX` (`:4468-4479`): electron stopping power, `SPTOTE(E,AE,AP)`
    /// with this file's [`Self::sptote_pegs4`].
    pub fn ededx(&mut self, e: f32) -> Result<f32, Pegs4Error> {
        let (ae, ap) = (self.c.aep, self.c.app);
        self.sptote_pegs4(e, ae, ap)
    }

    /// `PDEDX` (`:5057-5068`): positron stopping power.
    pub fn pdedx(&mut self, e: f32) -> Result<f32, Pegs4Error> {
        let (ae, ap) = (self.c.aep, self.c.app);
        self.sptotp_pegs4(e, ae, ap)
    }

    // -----------------------------------------------------------------
    // Stopping powers with this file's SPIONB
    // -----------------------------------------------------------------

    /// `SPIONB` (`:5184-5282`) as the stand-alone program has it. Same as
    /// [`Pegs4Common::spionb`] (Berger-Seltzer collision stopping power in MeV
    /// per radiation length) except for the density correction: with
    /// `EPSTFL = 0` it is the analytic Sternheimer fit
    /// `0` for `X <= X0`; `TOLN10*X-CBAR+AFACT*(X1-X)**SK` for `X < X1`;
    /// `TOLN10*X-CBAR` above (`:5235-5240`), `X = 0.21715*ln(ETA2)`; with
    /// `EPSTFL = 1` the table interpolation of [`Pegs4Common::density_delta`].
    /// The library version always interpolates a table (see the module
    /// notes). Needs [`Self::spinit_pegs4`] to have run.
    pub fn spionb_pegs4(&mut self, e0: f32, ee: f32, positr: bool) -> f32 {
        let rm = self.rm();
        let g = e0 / rm;
        let eem = ee / rm - 1.0;
        let t = g - 1.0;
        let eta2 = t * (g + 1.0);
        let beta2 = eta2 / (g * g);
        let aleta2 = ln(eta2);
        let x = 0.21715 * aleta2;
        let fterm;
        if !positr {
            let d = eem.min(0.5 * t);
            fterm = -1.0 - beta2 + ln((t - d) * d) + t / (t - d) + (d * d / 2.0 + (2.0 * t + 1.0) * ln(1.0 - d / t)) / (g * g);
        } else {
            let d = eem.min(t);
            let tp2 = t + 2.0;
            let d2 = d * d;
            let d3 = d * d2;
            let d4 = d * d3;
            fterm = ln(t * d)
                - (beta2 / t)
                    * (t + 2.0 * d - (3.0 * d2 / 2.0) / tp2 - (d - d3 / 3.0) / (tp2 * tp2)
                        - (d2 / 2.0 - t * d3 / 3.0 + d4 / 4.0) / (tp2 * tp2 * tp2));
        }
        let delta = if self.c.epstflp == 0 {
            if x <= self.c.x0 {
                0.0
            } else if x < self.c.x1 {
                self.c.toln10 * x - self.c.cbar + self.c.afact * powf(self.c.x1 - x, self.c.sk)
            } else {
                self.c.toln10 * x - self.c.cbar
            }
        } else {
            self.c.density_delta(e0)
        };
        (self.c.spc1 / beta2) * (ln(t + 2.0) - self.c.spc2 + fterm - delta)
    }

    /// `SPIONE` (`:5287-5296`): electron collision stopping power.
    pub fn spione_pegs4(&mut self, e0: f32, ee: f32) -> f32 {
        self.spionb_pegs4(e0, ee, false)
    }

    /// `SPIONP` (`:5300-5309`): positron collision stopping power.
    pub fn spionp_pegs4(&mut self, e0: f32, ee: f32) -> f32 {
        self.spionb_pegs4(e0, ee, true)
    }

    /// `SPTOTE` (`:5313-5336`): electron total stopping power by `IUNRST`
    /// (0 restricted collision + restricted radiative; 1 unrestricted
    /// collision; 2 both unrestricted; 3 unrestricted collision + restricted
    /// radiative; 4 restricted collision + unrestricted radiative; 5
    /// unrestricted radiative; 6 restricted radiative; 7 restricted
    /// collision). Anything else is an `Err` (upstream leaves the result
    /// unset).
    pub fn sptote_pegs4(&mut self, e0: f32, ee: f32, eg: f32) -> Result<f32, Pegs4Error> {
        Ok(match self.c.iunrstp {
            0 => self.spione_pegs4(e0, ee) + self.c.brmstm(e0, eg)?,
            1 => self.spione_pegs4(e0, e0),
            2 => self.spione_pegs4(e0, e0) + self.c.brmstm(e0, e0)?,
            3 => self.spione_pegs4(e0, e0) + self.c.brmstm(e0, eg)?,
            4 => self.spione_pegs4(e0, ee) + self.c.brmstm(e0, e0)?,
            5 => self.c.brmstm(e0, e0)?,
            6 => self.c.brmstm(e0, eg)?,
            7 => self.spione_pegs4(e0, ee),
            n => return Err(bad_iunrst(n)),
        })
    }

    /// `SPTOTP` (`:5340-5364`): positron total stopping power; see
    /// [`Self::sptote_pegs4`].
    pub fn sptotp_pegs4(&mut self, e0: f32, ee: f32, eg: f32) -> Result<f32, Pegs4Error> {
        Ok(match self.c.iunrstp {
            0 => self.spionp_pegs4(e0, ee) + self.c.brmstm(e0, eg)?,
            1 => self.spionp_pegs4(e0, e0),
            2 => self.spionp_pegs4(e0, e0) + self.c.brmstm(e0, e0)?,
            3 => self.spionp_pegs4(e0, e0) + self.c.brmstm(e0, eg)?,
            4 => self.spionp_pegs4(e0, ee) + self.c.brmstm(e0, e0)?,
            5 => self.c.brmstm(e0, e0)?,
            6 => self.c.brmstm(e0, eg)?,
            7 => self.spionp_pegs4(e0, ee),
            n => return Err(bad_iunrst(n)),
        })
    }

    /// `EFUNS` (`:1596-1675`): the eight electron/positron functions `PWLF1`
    /// fits (`V(1..8)` as indices 0..7). The same as
    /// [`Pegs4Common::efuns`] but calling this file's `SPTOTE`/`SPTOTP`.
    /// `IUNRST` 6 and 7 are `Err` (`$CALL_EXIT(20)`).
    pub fn efuns_pegs4(&mut self, e: f32) -> Result<[f32; 8], Pegs4Error> {
        let mut v = [0.0f32; 8];
        let (aep, app) = (self.c.aep, self.c.app);
        let thbrem = self.c.thbremp;
        let iu = self.c.iunrstp;
        if iu == 0 || iu == 1 || iu == 5 {
            let brem = self.c.bremtm(e)?;
            let amoll = self.c.amoltm(e);
            let bhab = self.c.bhabtm(e);
            let annih = self.c.anihtm(e);
            let esig = brem + amoll;
            v[0] = esig;
            let psig = brem + bhab + annih;
            v[1] = psig;
            v[2] = self.sptote_pegs4(e, aep, app)?;
            v[3] = self.sptotp_pegs4(e, aep, app)?;
            if esig > 0.0 {
                v[4] = brem / esig;
            } else if thbrem <= self.c.thmollp {
                v[4] = 1.0;
            } else {
                v[4] = 0.0;
            }
            v[5] = brem / psig;
            v[6] = (brem + bhab) / psig;
            v[7] = self.c.tmxs(e);
        } else if iu == 2 {
            v[0] = 0.0;
            v[1] = 0.0;
            v[4] = 0.0;
            v[5] = 0.0;
            v[6] = 0.0;
            v[2] = self.sptote_pegs4(e, e, e)?;
            v[3] = self.sptotp_pegs4(e, e, e)?;
            v[7] = self.c.tmxs(e);
        } else if iu == 3 {
            let brem = self.c.bremtm(e)?;
            let annih = self.c.anihtm(e);
            v[0] = brem;
            v[1] = brem + annih;
            v[2] = self.sptote_pegs4(e, e, app)?;
            v[3] = self.sptotp_pegs4(e, e, app)?;
            v[4] = 1.0;
            v[5] = brem / v[1];
            v[6] = v[5];
            v[7] = self.c.tmxs(e);
        } else if iu == 4 {
            v[0] = self.c.amoltm(e);
            v[1] = self.c.bhabtm(e);
            v[2] = self.sptote_pegs4(e, aep, e)?;
            v[3] = self.sptotp_pegs4(e, aep, e)?;
            v[4] = 0.0;
            v[5] = 0.0;
            v[6] = 1.0;
            v[7] = self.c.tmxs(e);
        } else {
            return Err(Pegs4Error::CallExit {
                code: 20,
                message: format!("*********IUNRST={iu} NOT ALLOWED BY EFUNS*****"),
            });
        }
        Ok(v)
    }
}

/// The function table of `$FUNCTIONS` (`pegs4.mortran:630-640`): name and
/// number of arguments, in the order `IFUNT`/`FI` use (1-based `IFUN` is
/// the position + 1).
pub const FUNCTIONS: [(&str, usize); 78] = [
    ("ALIN", 1),
    ("ALINI", 1),
    ("ADFMOL", 1),
    ("ADIMOL", 1),
    ("ADDMOL", 1),
    ("ALOG", 1),
    ("EXP", 1),
    ("AREC", 1),
    ("ALKE", 1),
    ("ALKEI", 1),
    ("AMOLDM", 2),
    ("AMOLFM", 1),
    ("AMOLRM", 3),
    ("AMOLTM", 1),
    ("ANIHDM", 2),
    ("ANIHFM", 1),
    ("ANIHRM", 3),
    ("ANIHTM", 1),
    ("APRIM", 2),
    ("BHABDM", 2),
    ("BHABFM", 1),
    ("BHABRM", 3),
    ("BHABTM", 1),
    ("BREMDR", 2),
    ("BREMFR", 1),
    ("BREMDZ", 3),
    ("BRMSDZ", 3),
    ("BREMFZ", 1),
    ("BRMSFZ", 1),
    ("BREMRR", 3),
    ("BREMRM", 3),
    ("BREMRZ", 4),
    ("BREMTM", 1),
    ("BREMTR", 1),
    ("BRMSRM", 3),
    ("BRMSRZ", 4),
    ("BRMSTM", 2),
    ("COHETM", 1),
    ("COHETZ", 2),
    ("COMPDM", 2),
    ("COMPFM", 1),
    ("COMPRM", 3),
    ("COMPTM", 1),
    ("CRATIO", 1),
    ("EBIND", 1),
    ("EBR1", 1),
    ("EDEDX", 1),
    ("ESIG", 1),
    ("FCOULC", 1),
    ("GBR1", 1),
    ("GBR2", 1),
    ("GMFP", 1),
    ("PAIRDR", 2),
    ("PAIRFR", 1),
    ("PAIRDZ", 3),
    ("PAIRFZ", 1),
    ("PAIRRM", 3),
    ("PAIRRR", 3),
    ("PAIRRZ", 4),
    ("PAIRTE", 1),
    ("PAIRTM", 1),
    ("PAIRTR", 1),
    ("PAIRTU", 1),
    ("PAIRTZ", 2),
    ("PBR1", 1),
    ("PBR2", 1),
    ("PDEDX", 1),
    ("PHOTTZ", 2),
    ("PHOTTE", 1),
    ("PSIG", 1),
    ("SPIONE", 2),
    ("SPIONP", 2),
    ("SPTOTE", 3),
    ("SPTOTP", 3),
    ("TMXB", 1),
    ("TMXS", 1),
    ("TMXDE2", 1),
    ("XSIF", 1),
];

/// `IFUNT` (`:4576-4590`): the 1-based index of a function name in
/// [`FUNCTIONS`] (the name is six characters, blank-padded, so trailing
/// blanks are ignored and a longer name is cut to six), or `None` where
/// upstream prints `FUNC=... NOT MATCHED` and returns -1.
pub fn ifunt(name: &str) -> Option<usize> {
    let n: String = name.chars().take(6).collect();
    let n = n.trim_end();
    FUNCTIONS.iter().position(|(f, _)| *f == n).map(|p| p + 1)
}

impl Pegs4Program {
    /// `FI` (`:4516-4527`): the function multiplexer behind `CALL`. `ifun`
    /// is the 1-based index of [`ifunt`]; `x` are `XP(1..4)` and only the
    /// first `FUNCTIONS[ifun-1].1` are used. Functions that rely on a COMMON
    /// block another one fills (`AMOLFM`, `ANIHFM`, `BHABFM`, `BREMFR`,
    /// `COMPFM`, `PAIRFR`, `PAIRFZ`, `BREMFZ`, `BRMSFZ`) see whatever the
    /// last call left there (zero at the start), exactly as upstream.
    ///
    /// # Errors
    ///
    /// A [`Pegs4Error`] from the underlying routine, or `Fatal` for an
    /// `ifun` outside the table.
    pub fn fi(&mut self, ifun: usize, x: [f32; 4]) -> Result<f32, Pegs4Error> {
        if ifun < 1 || ifun > FUNCTIONS.len() {
            return Err(Pegs4Error::Fatal(format!("FI: function number {ifun} outside 1..{}", FUNCTIONS.len())));
        }
        let (x1, x2, x3, x4) = (x[0], x[1], x[2], x[3]);
        let v = match FUNCTIONS[ifun - 1].0 {
            "ALIN" => self.alin(x1),
            "ALINI" => self.alini(x1),
            "ADFMOL" => self.adfmol(x1),
            "ADIMOL" => self.adimol(x1),
            "ADDMOL" => self.addmol(x1),
            "ALOG" => ln(x1),
            "EXP" => exp(x1),
            "AREC" => self.arec(x1),
            "ALKE" => self.c.alke(x1),
            "ALKEI" => self.c.alkei(x1),
            "AMOLDM" => self.amoldm(x1, x2),
            "AMOLFM" => self.amolfm(x1),
            "AMOLRM" => self.c.amolrm(x1, x2, x3),
            "AMOLTM" => self.c.amoltm(x1),
            "ANIHDM" => self.anihdm(x1, x2),
            "ANIHFM" => self.anihfm(x1),
            "ANIHRM" => self.anihrm(x1, x2, x3),
            "ANIHTM" => self.c.anihtm(x1),
            "APRIM" => self.c.aprim(x1, x2)?,
            "BHABDM" => self.bhabdm(x1, x2),
            "BHABFM" => self.bhabfm(x1),
            "BHABRM" => self.c.bhabrm(x1, x2, x3),
            "BHABTM" => self.c.bhabtm(x1),
            "BREMDR" => self.bremdr(x1, x2),
            "BREMFR" => self.bremfr(x1),
            "BREMDZ" => self.c.bremdz(x1, x2, x3)?,
            "BRMSDZ" => self.c.brmsdz(x1, x2, x3)?,
            "BREMFZ" => self.c.bremfz(x1),
            "BRMSFZ" => self.c.brmsfz(x1),
            "BREMRR" => self.bremrr(x1, x2, x3),
            "BREMRM" => self.c.bremrm(x1, x2, x3)?,
            "BREMRZ" => self.c.bremrz(x1, x2, x3, x4)?,
            "BREMTM" => self.c.bremtm(x1)?,
            "BREMTR" => self.bremtr(x1),
            "BRMSRM" => self.c.brmsrm(x1, x2, x3)?,
            "BRMSRZ" => self.c.brmsrz(x1, x2, x3, x4)?,
            "BRMSTM" => self.c.brmstm(x1, x2)?,
            "COHETM" => self.cohetm(x1),
            "COHETZ" => self.cohetz(x1, x2),
            "COMPDM" => self.compdm(x1, x2),
            "COMPFM" => self.compfm(x1),
            "COMPRM" => self.comprm(x1, x2, x3),
            "COMPTM" => self.comptm(x1),
            "CRATIO" => self.cratio(x1),
            "EBIND" => self.ebind(x1),
            "EBR1" => self.ebr1(x1)?,
            "EDEDX" => self.ededx(x1)?,
            "ESIG" => self.esig(x1)?,
            "FCOULC" => self.c.fcoulcp(x1),
            "GBR1" => self.gbr1(x1),
            "GBR2" => self.gbr2(x1),
            "GMFP" => self.gmfp(x1),
            "PAIRDR" => self.pairdr(x1, x2),
            "PAIRFR" => self.pairfr(x1),
            "PAIRDZ" => self.pairdz(x1, x2, x3),
            "PAIRFZ" => self.pairfz(x1),
            "PAIRRM" => self.pairrm(x1, x2, x3),
            "PAIRRR" => self.pairrr(x1, x2, x3),
            "PAIRRZ" => self.pairrz(x1, x2, x3, x4),
            "PAIRTE" => self.pairte(x1),
            "PAIRTM" => self.pairtm(x1),
            "PAIRTR" => self.pairtr(x1),
            "PAIRTU" => self.pairtu(x1),
            "PAIRTZ" => self.pairtz(x1, x2),
            "PBR1" => self.pbr1(x1)?,
            "PBR2" => self.pbr2(x1)?,
            "PDEDX" => self.pdedx(x1)?,
            "PHOTTZ" => self.phottz(x1, x2),
            "PHOTTE" => self.photte(x1),
            "PSIG" => self.psig(x1)?,
            "SPIONE" => self.spione_pegs4(x1, x2),
            "SPIONP" => self.spionp_pegs4(x1, x2),
            "SPTOTE" => self.sptote_pegs4(x1, x2, x3)?,
            "SPTOTP" => self.sptotp_pegs4(x1, x2, x3)?,
            "TMXB" => self.c.tmxb(x1),
            "TMXS" => self.c.tmxs(x1),
            "TMXDE2" => self.tmxde2(x1),
            "XSIF" => self.c.xsifp(x1),
            other => return Err(Pegs4Error::Fatal(format!("FI: no dispatch for {other}"))),
        };
        Ok(v)
    }

    /// `MIX` (`:1914-2109`) as the stand-alone program has it: the same as
    /// [`Pegs4Common::mix`] except the radiation length, which here is
    /// `RLC = 1/((ZAB-ZF)*(AN*RHO/WM)*4*FSC*R0**2)` (`:2034`) rather than
    /// `1/((AN*RHO/WM)*4*FSC*R0**2*(ZAB-ZF))`: a different `f32` rounding
    /// sequence. `BLCC`, `TEFF0`, `XCC` and `XR0` follow from it
    /// (`:2081-2107`). Works on `self.c` (call after `medium_option` has
    /// copied `Z`, `PZ`, `WA`, `RHOZ`, `NE` there). **Scales `RHO` by `GASP`
    /// in place** when `GASP != 0` (`:2005`), so call it once per medium.
    pub fn mix_pegs4(&mut self) {
        let k = self.c.k;
        if self.c.gaspp != 0.0 {
            self.c.rhop = self.c.gaspp * self.c.rhop;
        }
        let al183 = ln(self.c.a183);
        self.c.tpz = 0.0;
        self.c.wm = 0.0;
        self.c.zc = 0.0;
        self.c.zt = 0.0;
        self.c.zb = 0.0;
        self.c.zf = 0.0;
        self.c.zs = 0.0;
        self.c.ze = 0.0;
        self.c.zx = 0.0;
        let mut zab = 0.0f32;
        for i in 0..self.c.nep {
            let zi = self.c.zelemp[i];
            let pzi = self.c.pzp[i];
            self.c.tpz += pzi;
            self.c.wm += pzi * self.c.wap[i];
            self.c.zc += pzi * zi;
            let fzc = (k.fsc * zi) * (k.fsc * zi);
            let fcoul = self.c.fcoulcp(zi);
            let xsi = self.c.xsifp(zi);
            let zzx = pzi * zi * (zi + xsi);
            if zi <= 4.0 {
                let izz = zi as usize;
                zab += zzx * self.c.alrad[izz - 1];
            } else {
                zab += zzx * (al183 + ln(powf(zi, -1.0 / 3.0)));
            }
            self.c.zt += zzx;
            self.c.zb += zzx * ln(powf(zi, -1.0 / 3.0));
            self.c.zf += zzx * fcoul;
            let zz = pzi * zi * (zi + 1.0);
            self.c.zs += zz;
            self.c.ze += zz * ((-2.0 / 3.0) * ln(zi));
            self.c.zx += zz * ln(1.0 + 3.34 * fzc);
        }
        self.c.ez = self.c.zc / self.c.tpz;
        self.c.za = al183 * self.c.zt;
        self.c.zg = self.c.zb / self.c.zt;
        self.c.zp = self.c.zb / self.c.za;
        self.c.zv = (self.c.zb - self.c.zf) / self.c.zt;
        self.c.zu = (self.c.zb - self.c.zf) / self.c.za;
        self.c.eden = k.an * self.c.rhop / self.c.wm * self.c.zc;
        self.c.rlcp = 1.0 / ((zab - self.c.zf) * (k.an * self.c.rhop / self.c.wm) * 4.0 * k.fsc * (k.r0 * k.r0));
        self.c.blccp = k.a6680 * self.c.rhop * self.c.zs * exp(self.c.ze / self.c.zs) * self.c.rlcp
            / (self.c.wm * exp(self.c.zx / self.c.zs));
        self.c.teff0p = (exp(self.c.bmin) / self.c.bmin) / self.c.blccp;
        self.c.xccp = (k.a22p9 / k.raddeg) * sqrt(self.c.zs * self.c.rhop * self.c.rlcp / self.c.wm);
        self.c.xr0p = self.c.xccp * sqrt(self.c.teff0p * self.c.bmin);
    }

    /// `SPINIT` (`:2672-2923`) as the stand-alone program has it: set the
    /// density-effect parameters and `SPC1`, `SPC2`, `TOLN10` for
    /// [`Self::spionb_pegs4`]. Call after [`Self::mix_pegs4`].
    ///
    /// * `EPSTFL = 0` (also any value outside 0..1, `:2689`): if `ISSB != 0`
    ///   the user's `AFACT, CBAR, SK, X0, X1, IEV` are used (all six must be
    ///   non-zero, `CBAR > 0`, else exit 21); otherwise they must all be
    ///   zero, and the medium is looked up by `IDSTRN` in the 73-medium
    ///   Sternheimer-Seltzer-Berger table, else the Sternheimer-Peierls
    ///   general formula is used: `IEV` from `ITBL` (an element; H, N, O are
    ///   refused, exit 21) or the Bragg rule with Berger and Seltzer's
    ///   adjusted I-values, `CBAR = 1+2 ln(I/(hbar*2*pi*VPLASM/ERGMEV))`,
    ///   `X0`, `X1`, `SK` by the case tables (`:2744-2817`), `AFACT` from
    ///   continuity at `X0`. A gas-pressure correction (`ln GASP`) follows
    ///   for every path (`:2820-2827`).
    /// * `EPSTFL = 1`: unit 20 ([`Self::density_text`]): a title line, then
    ///   `NEPST IEV RHO NELEPS`, `NELEPS` pairs `Z W`, `NEPST` pairs
    ///   `E(kinetic, MeV) DELTA`. Energies become total, are clamped to
    ///   `AE`/`UE`, and the composition and density are checked against the
    ///   PEGS medium to 1 % with upstream's flag logic (the same flaw as the
    ///   library version: a later matching element clears an earlier
    ///   mismatch).
    ///
    /// A zero sum of `RHOZ` only prints a message upstream (`:2871-2874`)
    /// and the run continues; it does here too (a line in `listing`).
    ///
    /// # Errors
    ///
    /// Exit codes 21, 22, 23 of upstream as [`Pegs4Error::CallExit`].
    pub fn spinit_pegs4(&mut self, listing: &mut Vec<String>) -> Result<(), Pegs4Error> {
        let k = self.c.k;
        let rm = k.rmp;
        self.c.toln10 = 2.0 * ln(10.0);
        let mut im: i32 = -100;
        if self.c.epstflp < 0 || self.c.epstflp > 1 {
            self.c.epstflp = 0;
        }
        let imev: f32;
        if self.c.epstflp == 0 {
            if self.issb != 0.0 {
                if self.c.afact == 0.0
                    || self.c.cbar <= 0.0
                    || self.c.sk == 0.0
                    || self.c.x0 == 0.0
                    || self.c.x1 == 0.0
                    || self.c.iev == 0.0
                {
                    return Err(Pegs4Error::CallExit {
                        code: 21,
                        message: String::from("USER ERROR - NOT ALL DENSITY EFFECT PARAMETERS INPUT   CODE STOPPED IN SPINIT"),
                    });
                }
                imev = self.c.iev * 1.0e-6;
                im = -1;
            } else {
                if self.c.afact != 0.0
                    || self.c.cbar != 0.0
                    || self.c.sk != 0.0
                    || self.c.x0 != 0.0
                    || self.c.x1 != 0.0
                    || self.c.iev != 0.0
                {
                    return Err(Pegs4Error::CallExit {
                        code: 21,
                        message: String::from("STOPPED IN SPINIT: INCORRECT USER-OVERRIDE OF SSB-DATA"),
                    });
                }
                // THE SSB LOOKUP TABLE (:2723-2730)
                let id = pad_to(&self.idstrn, LMED);
                let mut found: Option<usize> = None;
                for (m, name) in SSB_NAMES.iter().enumerate() {
                    if pad_to(name, LMED) == id {
                        found = Some(m);
                        break;
                    }
                }
                if let Some(m) = found {
                    let row = SSB_DATA[m];
                    self.c.afact = row[0];
                    self.c.sk = row[1];
                    self.c.x0 = row[2];
                    self.c.x1 = row[3];
                    self.c.iev = row[4];
                    self.c.cbar = row[5];
                    imev = self.c.iev * 1.0e-6;
                    im = (m + 1) as i32;
                } else {
                    // THE STERNHEIMER-PEIERLS GENERAL FORMULA (:2732-2817)
                    im = 0;
                    if self.c.nep == 1 {
                        let iz = self.c.zelemp[0] as usize;
                        if iz == 1 || iz == 7 || iz == 8 {
                            return Err(Pegs4Error::CallExit {
                                code: 21,
                                message: String::from(
                                    "STOPPED IN SUBROUTINE SPINIT BECAUSE THIS ELEMENT (H, N, OR O) CAN ONLY EXIST AS A DIATOMIC MOLECULE. REMEDY: USE COMP OPTION FOR H2, N2, OR O2 WITH NE=2,PZ=1,1 AND, IN THE CASE OF A GAS, DEFINE STERNHEIMER ID (I.E., IDSTRN) LIKE H2-GAS",
                                ),
                            });
                        }
                        self.c.iev = ITBL[iz - 1];
                    } else {
                        let mut aliadg = 0.0f32;
                        for ie in 0..self.c.nep {
                            let iz = self.c.zelemp[ie] as usize;
                            let gas = self.c.gaspp == 0.0;
                            if iz == 1 {
                                self.c.iev = 19.2;
                            } else if iz == 6 {
                                self.c.iev = if gas { 81.0 } else { 70.0 };
                            } else if iz == 7 {
                                self.c.iev = 82.0;
                            } else if iz == 8 {
                                self.c.iev = if gas { 106.0 } else { 97.0 };
                            } else if iz == 9 {
                                self.c.iev = 112.0;
                            } else if iz == 17 {
                                self.c.iev = 180.0;
                            } else {
                                self.c.iev = 1.13 * ITBL[iz - 1];
                            }
                            // NRCC: the 7 lines above reflect table 6 of ref 59 of SLAC-265 (:2748-2763)
                            aliadg = aliadg + self.c.pzp[ie] * self.c.zelemp[ie] * ln(self.c.iev);
                        }
                        aliadg /= self.c.zc;
                        self.c.iev = exp(aliadg);
                    }
                    let imev_sp = self.c.iev * 1.0e-6;
                    imev = imev_sp;
                    let edenl = if self.c.gaspp == 0.0 { self.c.eden } else { self.c.eden / self.c.gaspp };
                    let vplasm = sqrt(edenl / k.pip * k.r0 * (k.c * k.c));
                    self.c.cbar = 1.0 + 2.0 * ln(imev_sp / (k.hbar * 2.0 * k.pip * vplasm / k.ergmev));
                    let z1 = if self.c.nep >= 1 { self.c.zelemp[0] as i32 } else { 0 };
                    let z2 = if self.c.nep >= 2 { self.c.zelemp[1] as i32 } else { 0 };
                    if self.c.nep == 1 && z1 == 2 && self.c.gaspp != 0.0 {
                        self.c.x0 = 2.191;
                        self.c.x1 = 3.0;
                        self.c.sk = 3.297;
                    } else if self.c.nep == 2 && z1 == 1 && z2 == 1 {
                        if self.c.gaspp == 0.0 {
                            self.c.x0 = 0.425;
                            self.c.x1 = 2.0;
                            self.c.sk = 5.949;
                        } else {
                            self.c.x0 = 1.837;
                            self.c.x1 = 3.0;
                            self.c.sk = 4.754;
                        }
                    } else {
                        self.c.sk = 3.0;
                        let cbar = self.c.cbar;
                        if self.c.gaspp == 0.0 {
                            if self.c.iev < 100.0 {
                                if cbar < 3.681 {
                                    self.c.x0 = 0.2;
                                    self.c.x1 = 2.0;
                                } else {
                                    self.c.x0 = 0.326 * cbar - 1.0;
                                    self.c.x1 = 2.0;
                                }
                            } else if cbar < 5.215 {
                                self.c.x0 = 0.2;
                                self.c.x1 = 3.0;
                            } else {
                                self.c.x0 = 0.326 * cbar - 1.5;
                                self.c.x1 = 3.0;
                            }
                            if self.c.x0 >= self.c.x1 {
                                return Err(Pegs4Error::CallExit {
                                    code: 21,
                                    message: format!(
                                        "STOPPED IN SPINIT DUE TO X0.GE.X1, X0,X1,CBAR={} {} {} IF THIS IS GAS, YOU MUST DEFINE GASP(ATM)",
                                        self.c.x0, self.c.x1, cbar
                                    ),
                                });
                            }
                        } else if cbar < 10.0 {
                            self.c.x0 = 1.6;
                            self.c.x1 = 4.0;
                        } else if cbar < 10.5 {
                            self.c.x0 = 1.7;
                            self.c.x1 = 4.0;
                        } else if cbar < 11.0 {
                            self.c.x0 = 1.8;
                            self.c.x1 = 4.0;
                        } else if cbar < 11.5 {
                            self.c.x0 = 1.9;
                            self.c.x1 = 4.0;
                        } else if cbar < 12.25 {
                            self.c.x0 = 2.0;
                            self.c.x1 = 4.0;
                        } else if cbar < 13.804 {
                            self.c.x0 = 2.0;
                            self.c.x1 = 5.0;
                        } else {
                            self.c.x0 = 0.326 * cbar - 2.5;
                            self.c.x1 = 5.0;
                        }
                    }
                }
            }
            // :SSB-PARAMETERS-DEFINED: GAS PRESSURE CORRECTION (:2818-2828)
            if self.c.gaspp != 0.0 {
                let algasp = ln(self.c.gaspp);
                self.c.cbar -= algasp;
                self.c.x0 -= algasp / self.c.toln10;
                self.c.x1 -= algasp / self.c.toln10;
            }
            if im == 0 {
                self.c.afact = (self.c.cbar - self.c.toln10 * self.c.x0) / powf(self.c.x1 - self.c.x0, self.c.sk);
            }
        } else {
            // EPSTFL = 1: READ THE DENSITY-EFFECT FILE (:2831-2902)
            let text = match &self.density_text {
                Some(t) => t.clone(),
                None => return Err(Pegs4Error::Fatal(String::from("EPSTFL=1 but no density-effect file (unit 20) was supplied"))),
            };
            let mut rd = ListReader::new(&text);
            let title = rd.read_line().unwrap_or_default();
            self.c.epsttl = pad_to(&title, 80);
            let head = rd.items(4)?;
            let nepst: usize = head[0].parse().map_err(|_| Pegs4Error::Fatal(format!("bad NEPST: {}", head[0])))?;
            let iev = real_token(&head[1]).map_err(Pegs4Error::Fatal)?;
            let epstrh = real_token(&head[2]).map_err(Pegs4Error::Fatal)?;
            let neleps: usize = head[3].parse().map_err(|_| Pegs4Error::Fatal(format!("bad NELEPS: {}", head[3])))?;
            if nepst > 150 {
                return Err(Pegs4Error::CallExit {
                    code: 22,
                    message: format!("*****NEPST={nepst} IS GREATER THAN THE 150 ALLOWED"),
                });
            }
            if neleps > 20 {
                return Err(Pegs4Error::Fatal(format!("NELEPS={neleps} is greater than the 20 the arrays hold")));
            }
            self.c.iev = iev;
            self.c.nepst = nepst;
            self.c.neleps = neleps;
            let zw = rd.items(2 * neleps)?;
            for i in 0..neleps {
                self.c.zepst[i] = zw[2 * i].parse().map_err(|_| Pegs4Error::Fatal(format!("bad ZEPST: {}", zw[2 * i])))?;
                self.c.wepst[i] = real_token(&zw[2 * i + 1]).map_err(Pegs4Error::Fatal)?;
            }
            let ed = rd.items(2 * nepst)?;
            for i in 0..nepst {
                self.c.epsten[i] = real_token(&ed[2 * i]).map_err(Pegs4Error::Fatal)?;
                self.c.epstd[i] = real_token(&ed[2 * i + 1]).map_err(Pegs4Error::Fatal)?;
            }
            // CONVERT TO TOTAL ENERGIES (:2840-2841)
            for i in 0..nepst {
                self.c.epsten[i] += rm;
            }
            imev = self.c.iev * 1.0e-6;
            // CHECK THAT WE HAVE COVERED ENERGY RANGES NEEDED (:2845-2853)
            if nepst > 0 && self.c.aep < self.c.epsten[0] {
                listing.push(format!("SPINIT: lowest density-effect energy {} is above AE={}, set to AE", self.c.epsten[0], self.c.aep));
                self.c.epsten[0] = self.c.aep;
            }
            if nepst > 0 && self.c.uep > self.c.epsten[nepst - 1] {
                listing.push(format!(
                    "SPINIT: highest density-effect energy {} is below UE={}, set to UE",
                    self.c.epsten[nepst - 1],
                    self.c.uep
                ));
                self.c.epsten[nepst - 1] = self.c.uep;
            }
            // COMPOSITION AND DENSITY CHECK (:2855-2902)
            let tlrnce: f32 = 0.01;
            let mut icheck = 0;
            if neleps != self.c.nep {
                icheck = 1;
            }
            if icheck == 0 && (epstrh < ((1.0 - tlrnce) * self.c.rhop) || epstrh > ((1.0 + tlrnce) * self.c.rhop)) {
                icheck = 1;
            }
            let mut epstwt = 0.0f32;
            for i in 0..self.c.nep {
                epstwt += self.c.rhozp[i];
            }
            if epstwt == 0.0 {
                listing.push(String::from("IN SPINIT: SOMETHING WRONG, MOLECULAR WEIGHT OF MOLECULE IS ZERO (I.E. SUM OF RHOZ)"));
            }
            if icheck == 0 {
                let mut iespel = 0usize;
                icheck = 1;
                loop {
                    iespel += 1;
                    let mut ipegel = 0usize;
                    loop {
                        ipegel += 1;
                        if (self.c.zelemp[ipegel - 1] as i32) == self.c.zepst[iespel - 1] {
                            icheck = 0;
                            break;
                        }
                        if ipegel >= self.c.nep {
                            break;
                        }
                    }
                    if icheck == 0 {
                        let rz = self.c.rhozp[ipegel - 1];
                        let w = self.c.wepst[iespel - 1];
                        if w < ((1.0 - tlrnce) * rz / epstwt) || w > ((1.0 + tlrnce) * rz / epstwt) {
                            icheck = 1;
                        }
                    }
                    if iespel >= neleps {
                        break;
                    }
                }
            }
            if icheck == 1 {
                return Err(Pegs4Error::CallExit {
                    code: 23,
                    message: String::from("COMPOSITION IN INPUT DENSITY FILE DOES NOT MATCH THAT BEING USED BY PEGS, QUITTING EARLY"),
                });
            }
        }
        self.c.spc1 = 2.0 * k.pip * (k.r0 * k.r0) * rm * self.c.eden * self.c.rlcp;
        let r = imev / rm;
        self.c.spc2 = ln(r * r / 2.0);
        listing.push(match im {
            0 => String::from("SPINIT: Sternheimer-Peierls general formula used for the density effect"),
            n if n > 0 => String::from("SPINIT: Sternheimer-Seltzer-Berger table used for the density effect"),
            -1 => String::from("SPINIT: Sternheimer-Seltzer-Berger density effect data supplied by user"),
            _ => String::from("SPINIT: density effect read in directly"),
        });
        Ok(())
    }
}

/// `TH(29)` of `MOLIER` (`:2133-2135`): Bethe's table of reduced angles.
const MOLIER_TH: [f32; 29] = [
    0.05, 0.2, 0.4, 0.6, 0.8, 1.0, 1.2, 1.4, 1.6, 1.8, 2.0, 2.2, 2.4, 2.6, 2.8, 3.0, 3.2, 3.4, 3.6, 3.8, 4.07, 4.5, 5.0, 5.5,
    6.13, 7.0, 8.0, 9.0, 9.75,
];
/// `DTH(29)` (`:2136`): `.1, 19*0.2, 0.35, 3*0.5, 0.75, 3*1.0, 0.5`.
const MOLIER_DTH: [f32; 29] = [
    0.1, 0.2, 0.2, 0.2, 0.2, 0.2, 0.2, 0.2, 0.2, 0.2, 0.2, 0.2, 0.2, 0.2, 0.2, 0.2, 0.2, 0.2, 0.2, 0.2, 0.35, 0.5, 0.5, 0.5,
    0.75, 1.0, 1.0, 1.0, 0.5,
];
/// `F0(29)` (`:2137-2141`); the last value is `$F0(29) = 1.18E-38`.
const MOLIER_F0: [f32; 29] = [
    2.0, 1.9216, 1.7214, 1.4094, 1.0546, 0.7338, 0.4738, 0.2817, 0.1546, 0.0783, 0.0366, 0.01581, 0.0063, 0.00232, 7.9e-4,
    2.5e-4, 7.3e-5, 1.9e-5, 4.7e-6, 1.1e-6, 2.3e-7, 3.0e-9, 2.0e-11, 2.0e-13, 5.0e-16, 1.0e-21, 3.0e-28, 1.0e-35, F0_29,
];
/// `F1(29)` (`:2142-2146`).
const MOLIER_F1: [f32; 29] = [
    0.8456, 0.7038, 0.3437, -0.0777, -0.3981, -0.5285, -0.4770, -0.3183, -0.1396, -0.0006, 0.0782, 0.1054, 0.1008, 0.08262,
    0.06247, 0.0455, 0.03288, 0.02402, 0.01791, 0.01366, 0.010638, 0.00614, 0.003831, 0.002527, 0.001739, 0.000908, 0.0005211,
    0.0003208, 0.0002084,
];
/// `F2(29)` (`:2147-2151`).
const MOLIER_F2: [f32; 29] = [
    2.4929, 2.0694, 1.0488, -0.0044, -0.6068, -0.6359, -0.3086, 0.0525, 0.2423, 0.2386, 0.1316, 0.0196, -0.0467, -0.0649,
    -0.0546, -0.03568, -0.01923, -0.00847, -0.00264, 5.0e-5, 0.0010741, 0.0012294, 0.0008326, 0.0005368, 0.0003495, 0.0001584,
    7.83e-5, 4.17e-5, 2.37e-5,
];

fn put_f32s(dst: &mut [f32], src: &Option<Vec<f32>>, name: &str) -> Result<(), String> {
    if let Some(v) = src {
        if v.len() > dst.len() {
            return Err(format!("{name}: {} values given, the array holds {}", v.len(), dst.len()));
        }
        dst[..v.len()].copy_from_slice(v);
    }
    Ok(())
}

impl Pegs4Program {
    /// Apply the `&INP` assignments of a block to the state, in the way the
    /// Fortran namelist read does (only the variables present change). Array
    /// assignments set a prefix; an array longer than the Fortran array is
    /// an `Err` (the namelist read would fail).
    fn apply_inp(&mut self, inp: &Pegs4Inp) -> Result<(), String> {
        if let Some(ne) = inp.ne {
            if ne > MXEL {
                return Err(format!("NE={ne} exceeds $MXEL={MXEL}"));
            }
            self.ne = ne;
        }
        put_f32s(&mut self.pz, &inp.pz, "PZ")?;
        put_f32s(&mut self.rhoz, &inp.rhoz, "RHOZ")?;
        put_f32s(&mut self.wa, &inp.wa, "WA")?;
        put_f32s(&mut self.xp, &inp.xp, "XP")?;
        put_f32s(&mut self.pwlf.zthre, &inp.zthre, "ZTHRE")?;
        put_f32s(&mut self.pwlf.zepe, &inp.zepe, "ZEPE")?;
        put_f32s(&mut self.pwlf.zthrg, &inp.zthrg, "ZTHRG")?;
        put_f32s(&mut self.pwlf.zepg, &inp.zepg, "ZEPG")?;
        if let Some(v) = inp.rho {
            self.c.rhop = v;
        }
        if let Some(v) = inp.ae {
            self.c.aep = v;
        }
        if let Some(v) = inp.ue {
            self.c.uep = v;
        }
        if let Some(v) = inp.ap {
            self.c.app = v;
        }
        if let Some(v) = inp.up {
            self.c.upp = v;
        }
        if let Some(v) = inp.epe {
            self.pwlf.epe = v;
        }
        if let Some(v) = inp.nipe {
            self.pwlf.nipe = v;
        }
        if let Some(v) = inp.nale {
            self.pwlf.nale = v;
        }
        if let Some(v) = inp.epg {
            self.pwlf.epg = v;
        }
        if let Some(v) = inp.nipg {
            self.pwlf.nipg = v;
        }
        if let Some(v) = inp.nalg {
            self.pwlf.nalg = v;
        }
        if let Some(v) = inp.epr {
            self.pwlf.epr = v;
        }
        if let Some(v) = inp.zthrr {
            self.pwlf.zthrr[0] = v;
        }
        if let Some(v) = inp.zepr {
            self.pwlf.zepr[0] = v;
        }
        if let Some(v) = inp.nipr {
            self.pwlf.nipr = v;
        }
        if let Some(v) = inp.nalr {
            self.pwlf.nalr = v;
        }
        if let Some(v) = inp.gasp {
            self.c.gaspp = v;
        }
        if let Some(v) = inp.iunrst {
            self.c.iunrstp = v;
        }
        if let Some(v) = inp.irayl {
            self.irayl = v;
        }
        if let Some(v) = inp.afact {
            self.c.afact = v;
        }
        if let Some(v) = inp.sk {
            self.c.sk = v;
        }
        if let Some(v) = inp.x0 {
            self.c.x0 = v;
        }
        if let Some(v) = inp.x1 {
            self.c.x1 = v;
        }
        if let Some(v) = inp.iev {
            self.c.iev = v;
        }
        if let Some(v) = inp.cbar {
            self.c.cbar = v;
        }
        if let Some(v) = inp.issb {
            self.issb = v;
        }
        if let Some(v) = inp.epstfl {
            self.c.epstflp = v;
        }
        if let Some(v) = inp.iaprim {
            self.c.iaprimp = v;
        }
        Ok(())
    }

    /// The `ELEM`, `MIXT` and `COMP` options (`:855-940`): define the
    /// medium (symbols to `Z` by `ZTBL`, atomic weights from `WATBL` unless
    /// `WA` overrides, `PZ` from `RHOZ` for a mixture or `RHOZ` from `PZ`
    /// otherwise, `RHO` from `RHOTBL` for an element without one), then
    /// `MIX`, `SPINIT`, `DIFFER` (in that order, `:919-934`) and set
    /// `MEDSET`. `kind` is the option name (`"ELEM"`, `"MIXT"`, `"COMP"`).
    ///
    /// # Errors
    ///
    /// `NE <= 1` for a mixture or compound (exit 17), an unknown symbol (exit
    /// 16), fewer symbols than `NE`, and any error of `SPINIT`.
    fn medium_option(&mut self, kind: &str, mb: &MediumBlock, listing: &mut Vec<String>) -> Result<(), String> {
        let imixt;
        if kind == "ELEM" {
            self.ne = 1;
            self.pz[0] = 1.0;
            imixt = false;
        } else if kind == "MIXT" {
            if self.ne <= 1 {
                return Err(format!("NE={} IS IMPROPERLY DEFINED FOR A MIXTURE", self.ne));
            }
            imixt = true;
        } else {
            if self.ne <= 1 {
                return Err(format!("CALL_EXIT(17): NE={} IS IMPROPERLY DEFINED FOR A COMPOUND", self.ne));
            }
            imixt = false;
        }
        self.mtyp = kind.chars().take(4).collect();
        self.medium = pad_to(&mb.medium, LMED);
        let first_blank = mb.idstrn.chars().next().map_or(true, |ch| ch == ' ');
        self.idstrn = if first_blank { self.medium.clone() } else { pad_to(&mb.idstrn, LMED) };
        if mb.asym.len() < self.ne {
            return Err(format!("{} element symbols given for NE={}", mb.asym.len(), self.ne));
        }
        self.asym = mb.asym[..self.ne].to_vec();
        for i in 0..self.ne {
            let zi = self.c.ztbl(self.asym[i].trim_end());
            self.z[i] = zi;
            if zi == 0.0 {
                return Err(format!("CALL_EXIT(16): BAD ATOMIC SYMBOL '{}'....JOB ABORTED.", self.asym[i]));
            }
            // ALLOW THE USER TO OVERRIDE ATOMIC WEIGHTS (:911-916)
            if self.wa[i] == 0.0 {
                self.wa[i] = WATBL[(zi as usize) - 1];
            }
            if imixt {
                self.pz[i] = self.rhoz[i] / self.wa[i];
            } else {
                self.rhoz[i] = self.pz[i] * self.wa[i];
            }
        }
        // IF RHO IS NOT GIVEN FOR AN ELEMENT, SUPPLY IT FROM OUR TABLE (:917)
        if self.ne == 1 && self.c.rhop == 0.0 {
            self.c.rhop = RHOTBL[(self.z[0] as usize) - 1];
        }
        let ne = self.ne;
        self.c.nep = ne;
        self.c.zelemp = self.z[..ne].to_vec();
        self.c.pzp = self.pz[..ne].to_vec();
        self.c.wap = self.wa[..ne].to_vec();
        self.c.rhozp = self.rhoz[..ne].to_vec();
        self.mix_pegs4();
        self.spinit_pegs4(listing).map_err(err_string)?;
        self.c.differ();
        self.medset = true;
        Ok(())
    }

    /// The `ENER` option (`:942-957`): negative `AE`, `UE`, `AP`, `UP` are
    /// multiples of `RM`; `TE = AE-RM`, `TET2`, `TEM`, `THBREM = RM+AP`
    /// (threshold for discrete bremsstrahlung), `THMOLL = AE+TE`; sets
    /// `ENGSET`. The values themselves arrive through `&INP`.
    fn ener_option(&mut self) {
        let rm = self.rm();
        if self.c.aep < 0.0 {
            self.c.aep = -self.c.aep * rm;
        }
        if self.c.uep < 0.0 {
            self.c.uep = -self.c.uep * rm;
        }
        if self.c.app < 0.0 {
            self.c.app = -self.c.app * rm;
        }
        if self.c.upp < 0.0 {
            self.c.upp = -self.c.upp * rm;
        }
        self.c.tep = self.c.aep - rm;
        self.tet2 = self.c.tep * 2.0;
        self.tem = self.c.tep / rm;
        self.c.thbremp = rm + self.c.app;
        self.c.thmollp = self.c.aep + self.c.tep;
        self.engset = true;
    }

    /// The `PWLF` option (`:966-1022`): `EBINDA = EBIND(AP)`, then the
    /// piecewise-linear fits by `PWLF1` ([`pwlf1`]): the 8 electron/positron
    /// functions of [`Self::efuns_pegs4`] over `AE..UE` (linear in
    /// `ln(E-RM)`, `XR = THMOLL`), the 4 photon functions of
    /// [`Self::gfuns`] over `AP..UP` (linear in `ln E`, `XR = 2*RM`) and, if
    /// `IRAYL = 1`, the Rayleigh distribution function ([`Self::rfuns`])
    /// over `0..1`, after forming `AFAC2`, squaring `XVAL` in place and
    /// integrating `AFFACT` into `AFFI` (`:987-1017`).
    ///
    /// # Errors
    ///
    /// `MEDSET`/`ENGSET` not both set (exit 16), a `NALE`/`NALG`/`NALR`
    /// beyond the array size, or an error from the functions fitted.
    fn pwlf_option(&mut self, out: &mut Pegs4Output) -> Result<(), String> {
        if !(self.medset && self.engset) {
            return Err(format!("CALL_EXIT(16): MEDSET,ENGSET={},{},PWLF REQ. IGNORED.", self.medset, self.engset));
        }
        let rm = self.rm();
        let (ae, ue, ap, up) = (self.c.aep, self.c.uep, self.c.app, self.c.upp);
        let thmoll = self.c.thmollp;
        let rmt2 = self.c.k.rmpt2;
        let pw = self.pwlf.clone();
        if pw.nale < 1 || pw.nale as usize > MXEKE {
            return Err(format!("NALE={} outside 1..{MXEKE}: PWLF would write past AFE", pw.nale));
        }
        if pw.nalg < 1 || pw.nalg as usize > MXGE {
            return Err(format!("NALG={} outside 1..{MXGE}: PWLF would write past AFG", pw.nalg));
        }
        if pw.nalr < 1 || pw.nalr as usize > MXRL {
            return Err(format!("NALR={} outside 1..{MXRL}: PWLF would write past AFR", pw.nalr));
        }
        self.ebinda = self.ebind(ap);
        out.listing.push(format!("AVERAGE K-IONIZATION ENERGY={} (MEV)", fmt_f(self.ebinda, 10, 6)));

        // FIRST FIT ELECTRON (AND POSITRON) (:974-984)
        let mut afe = core::mem::take(&mut self.afe);
        let mut bfe = core::mem::take(&mut self.bfe);
        let xfun_e = move |x: f32| ln(x - rm);
        let xfi_e = move |x: f32| exp(x) + rm;
        let (mut axe, mut bxe) = (0.0f32, 0.0f32);
        let mut err: Option<Pegs4Error> = None;
        let res_e = {
            let mut vf = |x: f32, o: &mut [f32]| match self.efuns_pegs4(x) {
                Ok(v) => o[..8].copy_from_slice(&v),
                Err(e) => {
                    if err.is_none() {
                        err = Some(e);
                    }
                    for s in o[..8].iter_mut() {
                        *s = f32::NAN;
                    }
                }
            };
            pwlf1(
                pw.nale, ae, ue, thmoll, pw.epe, &pw.zthre, &pw.zepe, pw.nipe, &xfun_e, &xfi_e, &mut axe, &mut bxe,
                MXEKE, 8, &mut afe, &mut bfe, &mut vf,
            )
        };
        self.afe = afe;
        self.bfe = bfe;
        if let Some(e) = err {
            return Err(err_string(e));
        }
        self.nel = res_e.ni as usize;
        self.axe = axe;
        self.bxe = bxe;
        out.listing.push(format!("PWLF electron: NEL={}", self.nel));

        // NEXT FIT GAMMA (:983-985)
        let mut afg = core::mem::take(&mut self.afg);
        let mut bfg = core::mem::take(&mut self.bfg);
        let (mut axg, mut bxg) = (0.0f32, 0.0f32);
        let res_g = {
            let mut vf = |x: f32, o: &mut [f32]| {
                let v = self.gfuns(x);
                o[..4].copy_from_slice(&v);
            };
            pwlf1(
                pw.nalg, ap, up, rmt2, pw.epg, &pw.zthrg, &pw.zepg, pw.nipg, &|x: f32| ln(x), &|x: f32| exp(x), &mut axg,
                &mut bxg, MXGE, 4, &mut afg, &mut bfg, &mut vf,
            )
        };
        self.afg = afg;
        self.bfg = bfg;
        self.ngl = res_g.ni as usize;
        self.axg = axg;
        self.bxg = bxg;
        out.listing.push(format!("PWLF photon: NGL={}", self.ngl));

        if self.irayl == 1 {
            // PWLF FOR THE DISTRIBUTION FUNCTION FOR RAYLEIGH (:987-1017)
            for i in 0..100 {
                self.afac2[i] = 0.0;
                for inn in 0..self.c.nep {
                    let izz = self.c.zelemp[inn] as usize;
                    let a = self.data.afac[(izz - 1) * 100 + i];
                    self.afac2[i] = self.afac2[i] + self.c.pzp[inn] * (a * a);
                }
            }
            for i in 0..100 {
                self.xval[i] = self.xval[i] * self.xval[i];
            }
            let mut vals = vec![0.0f32; 100];
            for i in 2..=87usize {
                let this = &*self;
                vals[i - 1] = qd(|x| this.affact(x), this.xval[i - 2], this.xval[i - 1]).value;
            }
            self.affi = vals;
            for i in 2..=87usize {
                self.affi[i - 1] = self.affi[i - 1] + self.affi[i - 2];
            }
            for i in 1..=87usize {
                self.affi[i - 1] = self.affi[i - 1] / self.affi[86];
            }
            let mut afr = core::mem::take(&mut self.afr);
            let mut bfr = core::mem::take(&mut self.bfr);
            let (mut axr, mut bxr) = (0.0f32, 0.0f32);
            let res_r = {
                let this = &*self;
                let mut vf = |x: f32, o: &mut [f32]| {
                    o[0] = this.rfuns(x);
                };
                pwlf1(
                    pw.nalr, 0.0, 1.0, 0.0, pw.epr, &pw.zthrr, &pw.zepr, pw.nipr, &|x: f32| x, &|x: f32| x, &mut axr, &mut bxr,
                    MXRL, 1, &mut afr, &mut bfr, &mut vf,
                )
            };
            self.afr = afr;
            self.bfr = bfr;
            self.ngr = res_r.ni as usize;
            self.axr = axr;
            self.bxr = bxr;
            out.listing.push(format!("PWLF Rayleigh: NGR={}", self.ngr));
        }
        Ok(())
    }

    /// `LAY` (`:1808-1909`), the `DECK` option: the data set, in unit 7's
    /// formats (see [`Pegs4Output::deck`]):
    ///
    /// 1. ` MEDIUM=<24>,STERNCID=<24>`;
    /// 2. ` <MTYP>,RHO=<E11.4>,NE=<I2>[,GASP=<E11.4>], IUNRST=<I1>, EPSTFL=<I1>, IAPRIM=<I1>`;
    /// 3. one ` ASYM=..,Z=..,A=..,PZ=..,RHOZ=..` line per element;
    /// 4. `RLC, AE, AP, UE, UP`; the sizes `NSGE, NGE, NSEKE, NEKE, NLEKE,
    ///    NCMFP, NRANGE, IRAYL, IUNRST` (`NSGE`, `NSEKE`, `NLEKE`, `NCMFP`,
    ///    `NRANGE` are faked as 0, `:1846`); the 36 screening constants;
    ///    `DELCM` and the `ALPHI, BPAR, DELPOS` pairs; `XR0, TEFF0, BLCC,
    ///    XCC`; `BXE, AXE`; the electron fits; `EBINDA, BXG, AXG`; the first
    ///    three photon fits per energy; and, when `IRAYL != 0`, `NGR`, `BXR,
    ///    AXR`, the Rayleigh fit and the fourth photon fit.
    ///
    /// Not written: the `pegs4.log` side file (`:1862-1900`), which needs a
    /// file system and the date.
    pub fn lay(&self, out: &mut Vec<String>) {
        let c = &self.c;
        out.push(format!(" MEDIUM={},STERNCID={}", self.medium, self.idstrn));
        let mtyp = pad_to(&self.mtyp, 4);
        if c.gaspp != 0.0 {
            out.push(format!(
                " {},RHO={},NE={},GASP={}, IUNRST={}, EPSTFL={}, IAPRIM={}",
                mtyp,
                fmt_e(c.rhop, 11, 4),
                fmt_i(self.ne as i64, 2),
                fmt_e(c.gaspp, 11, 4),
                fmt_i(i64::from(c.iunrstp), 1),
                fmt_i(i64::from(c.epstflp), 1),
                fmt_i(i64::from(c.iaprimp), 1)
            ));
        } else {
            out.push(format!(
                " {},RHO={},NE={}, IUNRST={}, EPSTFL={}, IAPRIM={}",
                mtyp,
                fmt_e(c.rhop, 11, 4),
                fmt_i(self.ne as i64, 2),
                fmt_i(i64::from(c.iunrstp), 1),
                fmt_i(i64::from(c.epstflp), 1),
                fmt_i(i64::from(c.iaprimp), 1)
            ));
        }
        for ie in 0..self.ne {
            out.push(format!(
                " ASYM={},Z={},A={},PZ={},RHOZ={}",
                pad_to(&self.asym[ie], 2),
                fmt_f(self.z[ie], 3, 0),
                fmt_f(self.wa[ie], 9, 3),
                fmt_e(self.pz[ie], 12, 5),
                fmt_e(self.rhoz[ie], 12, 5)
            ));
        }
        flt_records(out, &[c.rlcp, c.aep, c.app, c.uep, c.upp]);
        let (nge, neke) = (self.ngl as i64, self.nel as i64);
        int_records(out, &[0, nge, 0, neke, 0, 0, 0, i64::from(self.irayl), i64::from(c.iunrstp)]);
        let mut v = Vec::new();
        for i in 0..6 {
            for j in 0..6 {
                v.push(c.dlp[j][i]);
            }
        }
        flt_records(out, &v);
        let mut v = vec![c.delcmp];
        for i in 0..2 {
            v.push(c.alphip[i]);
            v.push(c.bparp[i]);
            v.push(c.delposp[i]);
        }
        flt_records(out, &v);
        flt_records(out, &[c.xr0p, c.teff0p, c.blccp, c.xccp]);
        flt_records(out, &[self.bxe, self.axe]);
        let mut v = Vec::new();
        for i in 0..self.nel {
            for f in 0..8 {
                v.push(self.bfe[i + f * MXEKE]);
                v.push(self.afe[i + f * MXEKE]);
            }
        }
        flt_records(out, &v);
        flt_records(out, &[self.ebinda, self.bxg, self.axg]);
        let mut v = Vec::new();
        for i in 0..self.ngl {
            for f in 0..3 {
                v.push(self.bfg[i + f * MXGE]);
                v.push(self.afg[i + f * MXGE]);
            }
        }
        flt_records(out, &v);
        if self.irayl != 0 {
            int_records(out, &[self.ngr as i64]);
            flt_records(out, &[self.bxr, self.axr]);
            let mut v = Vec::new();
            for i in 0..self.ngr {
                v.push(self.bfr[i]);
                v.push(self.afr[i]);
            }
            flt_records(out, &v);
            let mut v = Vec::new();
            for i in 0..self.ngl {
                v.push(self.bfg[i + 3 * MXGE]);
                v.push(self.afg[i + 3 * MXGE]);
            }
            flt_records(out, &v);
        }
    }

    /// The `CALL` option (`:1056-1060`): `IFUN = IFUNT(NAME)`; an unknown
    /// name only prints a message upstream and the job goes on (a line in
    /// `listing` here); otherwise `VALUE = FI(IFUN, XP(1..4))`.
    fn call_option(&mut self, cb: &CallBlock, out: &mut Pegs4Output) -> Result<(), String> {
        match ifunt(&cb.name) {
            None => {
                out.listing.push(format!("FUNC={} NOT MATCHED", pad_to(&cb.name, 6)));
                Ok(())
            }
            Some(ifun) => {
                let xp = self.xp;
                let value = self.fi(ifun, xp).map_err(err_string)?;
                out.calls.push(Pegs4CallResult { name: String::from(FUNCTIONS[ifun - 1].0), ifun, xp, value });
                Ok(())
            }
        }
    }

    /// `MOLIER` (`:2115-2324`), the `MIMS` option: the material-independent
    /// multiple-scattering data. For each of the 16 step sizes it solves
    /// Moliere's `B` by Newton's method, forms the probabilities of the 29
    /// tabulated reduced angles, lumps those below 0.001 into the next
    /// larger angle, rounds them to 1000 integer entries (adjusting the
    /// largest ones when rounding cannot make 1000) and fills `VERT1` (the
    /// minimum over steps) and `VERT2` (the rest). The data set records go to
    /// `out` (unit 7: `:2310-2320`); the listing tables (`:2290-2306`) are not
    /// reproduced.
    pub fn molier(&mut self, out: &mut Vec<String>) {
        let msteps = self.mims.msteps;
        let jrmax = self.mims.jrmax;
        let bmin = self.c.bmin;
        let fstep = self.mims.fstep;
        // NOW FILL UP MSMAP (:2204-2207)
        for is in 1..msteps {
            let lo = fstep[is - 1] as usize;
            let hi = (fstep[is] - 1.0) as usize;
            for j in lo..=hi {
                self.mims.msmap[j - 1] = is as i32;
            }
        }
        self.mims.msmap[jrmax - 1] = msteps as i32;
        let blcmin = bmin - ln(bmin);
        let mut p = vec![[0.0f32; MSSTEPS]; 29];
        let mut q = vec![[0.0f32; MSSTEPS]; 29];
        let mut ip1 = vec![[0i32; MSSTEPS]; 29];
        for is in 0..msteps {
            let blc = blcmin + ln(fstep[is]);
            // NOW SOLVE FOR B: FIRST GUESS, THEN NEWTON (:2213-2217)
            let mut b = blc + ln(blc);
            loop {
                let bold = b;
                b = bold - (bold - ln(bold) - blc) / (1.0 - 1.0 / bold);
                if ((b - bold) / bold).abs() < 1.0e-5 {
                    break;
                }
            }
            self.mims.fsqr[is] = sqrt(fstep[is] * b / bmin);
            // PROBABILITIES OF THE INPUT REDUCED ANGLES (:2221-2227)
            let b1 = 1.0 / b;
            let mut ptot = 0.0f32;
            for i in 0..29 {
                p[i][is] = MOLIER_TH[i] * MOLIER_DTH[i] * (MOLIER_F0[i] + b1 * (MOLIER_F1[i] + b1 * MOLIER_F2[i]));
                ptot += p[i][is];
            }
            for i in 0..29 {
                p[i][is] /= ptot;
            }
            for i in 0..29 {
                q[i][is] = p[i][is];
            }
            // ALL PROBABILITIES EITHER 0.0 OR GREATER THAN 0.001 (:2231-2237)
            let mut i: i32 = 29;
            loop {
                let mut l: i32 = 1;
                while q[(i - 1) as usize][is] < 0.001 && i > l {
                    let qi = q[(i - l - 1) as usize][is];
                    q[(i - 1) as usize][is] += qi;
                    q[(i - l - 1) as usize][is] = 0.0;
                    l += 1;
                }
                i -= l;
                if i <= 0 {
                    break;
                }
            }
            // MAKE UP 1000 INTEGER PROBABILITIES (:2240-2245)
            let mut ppp = 0.5f32;
            let mut pp = 0.5f32;
            let mut itot: i32 = 0;
            let mut idif: i32 = 0;
            let mut done = false;
            for _jlr in 1..=10 {
                itot = 0;
                for i in 0..29 {
                    ip1[i][is] = (q[i][is] * 1000.0 + pp) as i32;
                    itot += ip1[i][is];
                }
                idif = itot - 1000;
                if idif == 0 {
                    done = true;
                    break;
                }
                ppp *= 0.5;
                if idif < 0 {
                    pp += ppp;
                } else {
                    pp -= ppp;
                }
            }
            if done {
                continue;
            }
            // GET 1000 ENTRIES WHEN ROUNDING ADJUSTMENT FAILS (:2249-2263)
            let mut ip2: Vec<usize> = (1..=29).collect();
            let mut n: i32 = 29;
            loop {
                n -= 1;
                let mut iflg = false;
                for j in 1..=n.max(0) as usize {
                    let i01 = ip2[j - 1];
                    let i02 = ip2[j];
                    if ip1[i01 - 1][is] < ip1[i02 - 1][is] {
                        ip2.swap(j - 1, j);
                        iflg = true;
                    }
                }
                if !iflg {
                    break;
                }
            }
            let (ida, inc) = if idif < 0 { (-idif, 1) } else { (idif, -1) };
            for i in 1..=(ida as usize).min(29) {
                let i01 = ip2[i - 1];
                ip1[i01 - 1][is] += inc;
            }
            let _ = itot;
        }
        // NOW FIND MINIMUM FREQUENCY FOR EACH ANGLE OVER ALL STEPS (:2266-2272)
        let mut iall = [0i32; 29];
        let mut mxv1: i32 = 0;
        for i in 0..29 {
            iall[i] = ip1[i][0];
            for is in 1..msteps {
                iall[i] = iall[i].min(ip1[i][is]);
            }
            mxv1 += iall[i];
        }
        let mxv2 = 1000 - mxv1;
        self.mims.mxv1 = mxv1.max(0) as usize;
        self.mims.mxv2 = mxv2.max(0) as usize;
        // NOW FILL UP THE VERT ARRAYS (:2275-2284)
        let mut ii = 0usize;
        for i in 0..29 {
            for _j in 0..iall[i].max(0) {
                if ii < MXVRT1 {
                    self.mims.vert1[ii] = MOLIER_TH[i];
                }
                ii += 1;
            }
        }
        for is in 0..msteps {
            let mut ii = 0usize;
            for i in 0..29 {
                let ixtr = ip1[i][is] - iall[i];
                for _j in 0..ixtr.max(0) {
                    if ii < MXVRT1 {
                        self.mims.vert2[is][ii] = MOLIER_TH[i];
                    }
                    ii += 1;
                }
            }
        }
        // PUNCH OUT THE DATA NEEDED BY EGS (:2310-2320)
        out.push(String::from(" MATERIAL INDEPENDENT MULTIPLE SCATTERING DATA"));
        int_records(
            out,
            &[jrmax as i64, msteps as i64, self.mims.mxv1 as i64, self.mims.mxv2 as i64],
        );
        let mut pairs: Vec<(f32, f32)> = Vec::new();
        for i in 0..msteps {
            pairs.push((fstep[i], self.mims.fsqr[i]));
        }
        if pairs.is_empty() {
            out.push(String::from(" "));
        }
        for chunk in pairs.chunks(4) {
            let mut line = String::from(" ");
            for &(a, b) in chunk {
                line.push_str(&fmt_f(a, 5, 0));
                line.push_str(&fmt_f(b, 11, 6));
            }
            out.push(line);
        }
        let msmap: Vec<i64> = self.mims.msmap[..jrmax].iter().map(|&v| i64::from(v)).collect();
        int_records(out, &msmap);
        fnums_f52(out, &self.mims.vert1[..self.mims.mxv1.min(MXVRT1)]);
        for mst in 0..msteps {
            fnums_f52(out, &self.mims.vert2[mst][..self.mims.mxv2.min(MXVRT1)]);
        }
    }

    /// The main program (`:668-1151`): run the option blocks in order. At
    /// the top of each block `WA` and `RHO` are saved and zeroed (`:776-780`),
    /// the block's `&INP` is applied, a zero `RHO` is restored from the saved
    /// one for every option but `ELEM` (`:812-816`, see the module notes),
    /// and for the non-medium options `RHO` and `WA` are restored and a
    /// changed `EPSTFL`, `IUNRST` or `IAPRIM` is reset (`:826-846`). `STOP`
    /// (or the end of the list) ends the run.
    ///
    /// # Errors
    ///
    /// Any of the program's `$CALL_EXIT` or `STOP` conditions.
    pub fn run(&mut self, blocks: &[Pegs4Block]) -> Result<Pegs4Output, String> {
        let mut out = Pegs4Output::default();
        for blk in blocks {
            let inp = match blk.inp() {
                Some(i) => i,
                None => {
                    out.listing.push(String::from("END OF FILE READ - EXIT FROM PEGS4"));
                    break;
                }
            };
            let wasav = self.wa;
            let rhosav = self.c.rhop;
            self.wa = [0.0; MXEL];
            self.c.rhop = 0.0;
            let epstfl_old = self.c.epstflp;
            let iunrst_old = self.c.iunrstp;
            let iaprim_old = self.c.iaprimp;
            self.apply_inp(inp)?;
            let is_elem = matches!(blk, Pegs4Block::Elem(_));
            let is_medium = matches!(blk, Pegs4Block::Elem(_) | Pegs4Block::Mixt(_) | Pegs4Block::Comp(_));
            if self.c.rhop == 0.0 && !is_elem {
                self.c.rhop = rhosav;
            }
            if !is_medium {
                // RESTORE (:827-846)
                self.c.rhop = rhosav;
                self.wa = wasav;
                if self.c.epstflp != epstfl_old {
                    out.listing.push(format!("EPSTFL in wrong OPTION block! Resetting to EPSTFL= {epstfl_old}"));
                    self.c.epstflp = epstfl_old;
                }
                if self.c.iunrstp != iunrst_old {
                    out.listing.push(format!("IUNRST in wrong OPTION block! Resetting to IUNRST= {iunrst_old}"));
                    self.c.iunrstp = iunrst_old;
                }
                if self.c.iaprimp != iaprim_old {
                    out.listing.push(format!("IAPRIM in wrong OPTION block! Resetting to IAPRIM= {iaprim_old}"));
                    self.c.iaprimp = iaprim_old;
                }
            }
            match blk {
                Pegs4Block::Elem(mb) => self.medium_option("ELEM", mb, &mut out.listing)?,
                Pegs4Block::Mixt(mb) => self.medium_option("MIXT", mb, &mut out.listing)?,
                Pegs4Block::Comp(mb) => self.medium_option("COMP", mb, &mut out.listing)?,
                Pegs4Block::Ener(_) => self.ener_option(),
                Pegs4Block::Mims(_) => {
                    let mut lines = Vec::new();
                    self.molier(&mut lines);
                    out.deck.extend(lines);
                }
                Pegs4Block::Pwlf(_) => self.pwlf_option(&mut out)?,
                Pegs4Block::Deck(_) => {
                    let mut lines = Vec::new();
                    self.lay(&mut lines);
                    out.deck.extend(lines);
                }
                Pegs4Block::Test(_) => out.listing.push(String::from("TEST: plot calls only upstream, not ported")),
                Pegs4Block::Dbug(_) => {}
                Pegs4Block::Call(cb) => self.call_option(cb, &mut out)?,
                Pegs4Block::Plot(name, _) => out.listing.push(format!("{name}: plotting is not ported")),
                Pegs4Block::Stop => {}
            }
        }
        Ok(out)
    }
}

/// Build a [`Pegs4Program`] from the text of units 8, 9 and (optionally) 22
/// and 20, run `blocks` and return what the program wrote. `aprime_text` is
/// the content of `aprime.data` (needed when `IAPRIM = 1`, the default);
/// `density_text` the density-effect file for `EPSTFL = 1`.
///
/// # Errors
///
/// A bad data file, or any `$CALL_EXIT` of the program.
pub fn run(
    xsection: &str,
    formfactor: &str,
    aprime_text: Option<&str>,
    density_text: Option<&str>,
    blocks: &[Pegs4Block],
) -> Result<Pegs4Output, String> {
    let data = Pegs4Data::parse(xsection, formfactor)?;
    let mut prog = Pegs4Program::new(data);
    prog.c.aprime_text = aprime_text.map(String::from);
    prog.density_text = density_text.map(String::from);
    prog.run(blocks)
}

#[cfg(test)]
mod tests {
    use super::super::egsnrc_mortran::parse_pegs4_media;
    use super::*;

    fn rel(a: f32, b: f32) -> f32 {
        ((a - b) / b).abs()
    }

    // ---- synthetic data (NOT physical: shapes only, to exercise the code) ----

    const E_PH: [f32; 8] = [0.001, 0.003, 0.01, 0.03, 0.1, 1.0, 10.0, 100.0];

    /// Tables for Z = 1, 6, 8 with made-up smooth shapes. They are **not**
    /// cross sections: nothing here claims physical values.
    fn synthetic_data() -> Pegs4Data {
        let mut d = Pegs4Data::empty();
        for z in [1usize, 6, 8] {
            let zf = z as f32;
            d.nphe[z - 1] = 8;
            for (i, &e) in E_PH.iter().enumerate() {
                d.phe[(z - 1) * MXPHOTE + i] = e;
                d.phd[(z - 1) * MXPHOTE + i] = 1.0e3 * zf / (e * e * e * 1.0e3 + 1.0);
                d.cohe[(z - 1) * MXPHOTE + i] = 2.0 * zf / (1.0 + e);
            }
            d.ekedge[z - 1] = 0.01 * zf * zf;
            for j in 0..MXPAIRE {
                d.prd[(z - 1) * MXPAIRE + j] = zf * 0.01 * (1.0 + j as f32);
            }
            for i in 0..100 {
                d.afac[(z - 1) * 100 + i] = zf / (1.0 + 5.0 * (0.02 * i as f32) * (0.02 * i as f32));
            }
        }
        for j in 0..MXPAIRE {
            d.pre[j] = 1.1 * powf(1.35, j as f32);
        }
        for i in 0..100 {
            d.xval[i] = 0.02 * i as f32;
        }
        d
    }

    fn inp0() -> Pegs4Inp {
        Pegs4Inp { iaprim: Some(0), ..Default::default() }
    }

    /// `&INP` that zeroes the density-effect variables a previous medium's
    /// `SPINIT` left behind (see the module notes: without it a second
    /// medium in one run is refused).
    fn inp_reset() -> Pegs4Inp {
        Pegs4Inp {
            iaprim: Some(0),
            afact: Some(0.0),
            sk: Some(0.0),
            x0: Some(0.0),
            x1: Some(0.0),
            iev: Some(0.0),
            cbar: Some(0.0),
            ..Default::default()
        }
    }

    fn elem(sym: &str, medium: &str, id: &str, inp: Pegs4Inp) -> Pegs4Block {
        Pegs4Block::Elem(MediumBlock { inp, medium: pad_to(medium, LMED), idstrn: pad_to(id, LMED), asym: vec![String::from(sym)] })
    }

    fn comp(syms: &[&str], medium: &str, id: &str, inp: Pegs4Inp) -> Pegs4Block {
        Pegs4Block::Comp(MediumBlock {
            inp,
            medium: pad_to(medium, LMED),
            idstrn: pad_to(id, LMED),
            asym: syms.iter().map(|s| String::from(*s)).collect(),
        })
    }

    fn ener(ae: f32, ue: f32, ap: f32, up: f32) -> Pegs4Block {
        Pegs4Block::Ener(Pegs4Inp { ae: Some(ae), ue: Some(ue), ap: Some(ap), up: Some(up), ..Default::default() })
    }

    fn carbon_elem() -> Pegs4Block {
        elem("C", "C", "C-2.265 G/CM**3", inp0())
    }

    /// A carbon program with a medium and energy limits, no fit yet.
    fn carbon_program() -> Pegs4Program {
        let mut p = Pegs4Program::new(synthetic_data());
        p.run(&[carbon_elem(), ener(0.521, 10.511, 0.01, 10.0)]).unwrap();
        p
    }

    // ---- Fortran formats ----

    #[test]
    fn fortran_formats_follow_gfortran() {
        assert_eq!(fmt_e(1.0, 14, 5), "   1.00000E+00");
        assert_eq!(fmt_e(-0.000123456, 14, 5), "  -1.23456E-04");
        assert_eq!(fmt_e(0.0, 14, 5), "   0.00000E+00");
        assert_eq!(fmt_e(1.5e38, 12, 5), " 1.50000E+38");
        assert_eq!(fmt_e(11340.0, 11, 4), " 1.1340E+04");
        assert_eq!(fmt_e(1.0, 4, 5), "****");
        assert_eq!(fmt_e(f32::NAN, 6, 2).trim(), "NaN");
        assert_eq!(fmt_e(f32::INFINITY, 10, 2).trim(), "Infinity");
        assert_eq!(fmt_e(f32::NEG_INFINITY, 10, 2).trim(), "-Infinity");
        assert_eq!(fmt_f(6.0, 3, 0), " 6.");
        assert_eq!(fmt_f(12.011, 9, 3), "   12.011");
        assert_eq!(fmt_f(0.05, 5, 2), " 0.05");
        assert_eq!(fmt_f(0.5, 4, 3), ".500");
        assert_eq!(fmt_f(-0.5, 5, 3), "-.500");
        assert_eq!(fmt_f(f32::NAN, 5, 2).trim(), "NaN");
        assert_eq!(fmt_f(f32::INFINITY, 9, 2).trim(), "Infinity");
        assert_eq!(fmt_i(-3, 5), "   -3");
        assert_eq!(fmt_i(123456, 5), "*****");
        let mut out = Vec::new();
        flt_records(&mut out, &[1.0, 2.0, 3.0, 4.0, 5.0, 6.0]);
        assert_eq!(out.len(), 2);
        assert_eq!(out[0].len(), 1 + 5 * 14);
        assert_eq!(out[1], "    6.00000E+00");
        let mut out = Vec::new();
        flt_records(&mut out, &[]);
        int_records(&mut out, &[]);
        fnums_f52(&mut out, &[]);
        assert_eq!(out, vec![String::from(" "); 3]);
        let mut out = Vec::new();
        int_records(&mut out, &(1..=15).collect::<Vec<i64>>());
        assert_eq!(out.len(), 2);
        assert_eq!(out[0].len(), 1 + 14 * 5);
        assert_eq!(out[1], "    15");
        let mut out = Vec::new();
        fnums_f52(&mut out, &[0.05, 9.75, 4.07]);
        assert_eq!(out[0], "  0.05 9.75 4.07");
    }

    #[test]
    fn pmdcon_matches_an_f64_evaluation_of_the_same_formulas() {
        let k = pmdcon();
        let (pi, c, rme, hbar, ecgs, emks, an) =
            (3.1415926536f64, 2.997925e10f64, 9.1091e-28f64, 1.05450e-27f64, 4.80298e-10f64, 1.60210e-19f64, 6.02252e23f64);
        let ergmev = 1.0e6 * (emks * 1.0e7);
        let rm = rme * c * c / ergmev;
        let fsc = ecgs * ecgs / (hbar * c);
        let r0 = ecgs * ecgs / (rme * c * c);
        assert!(f64::from(k.rmp) / rm - 1.0 < 1e-6 && rm / f64::from(k.rmp) - 1.0 < 1e-6);
        assert!((f64::from(k.fsc) / fsc - 1.0).abs() < 1e-6);
        assert!((f64::from(k.r0) / r0 - 1.0).abs() < 1e-6);
        assert!((f64::from(k.ergmev) / ergmev - 1.0).abs() < 1e-6);
        assert_eq!(k.rmpt2, k.rmp * 2.0);
        assert_eq!(k.rmpsq, k.rmp * k.rmp);
        let raddeg = 180.0 / pi;
        let a22p9 = raddeg * (4.0 * pi * an).sqrt() * ecgs * ecgs / ergmev;
        assert!((f64::from(k.a22p9) / a22p9 - 1.0).abs() < 1e-5);
        let q = hbar / (rme * c);
        let a6680 = 4.0 * pi * an * q * q * (0.885 * 0.885 / (1.167 * 1.13));
        assert!((f64::from(k.a6680) / a6680 - 1.0).abs() < 1e-5);
        // the stand-alone RM is NOT the NRC library's PRM = 0.5109989461
        assert!((k.rmp - 0.510_998_95).abs() > 1.0e-7);
        assert!((k.rmp - 0.511).abs() < 1.0e-4);
    }

    // ---- units 8 and 9, the deck types ----

    fn list_text(vals: &[String]) -> String {
        let mut s = String::new();
        for chunk in vals.chunks(5) {
            s.push(' ');
            s.push_str(&chunk.join(" "));
            s.push('\n');
        }
        s
    }

    /// The text of units 8 and 9 for `d`, as tokens (`D` exponents for the
    /// coherent table to exercise the reader).
    fn data_tokens(d: &Pegs4Data) -> (Vec<String>, Vec<String>) {
        let mut t8: Vec<String> = Vec::new();
        t8.extend(d.nphe.iter().map(|v| format!("{v}")));
        for arr in [&d.phe, &d.phd] {
            t8.extend(arr.iter().map(|v| format!("{v:e}")));
        }
        t8.extend(d.ekedge.iter().map(|v| format!("{v:e}")));
        t8.extend(d.pre.iter().map(|v| format!("{v:e}")));
        t8.extend(d.prd.iter().map(|v| format!("{v:e}")));
        t8.extend(d.cohe.iter().map(|v| format!("{:e}", v).replace('e', "D")));
        let mut t9: Vec<String> = d.xval.iter().map(|v| format!("{v:e}")).collect();
        t9.extend(d.afac.iter().map(|v| format!("{v:e}")));
        (t8, t9)
    }

    #[test]
    fn data_parse_reads_the_two_free_format_files() {
        let d = synthetic_data();
        let (t8, t9) = data_tokens(&d);
        let parsed = Pegs4Data::parse(&list_text(&t8), &list_text(&t9)).unwrap();
        assert_eq!(parsed, d);
        assert!(Pegs4Data::parse("1 2 3", &list_text(&t9)).is_err());
        assert!(Pegs4Data::parse(&list_text(&t8), "1 2 3").is_err());
        let mut bad = t8.clone();
        bad[150] = String::from("x");
        assert!(Pegs4Data::parse(&list_text(&bad), &list_text(&t9)).is_err());
        assert_eq!(err_string(Pegs4Error::Fatal(String::from("m"))), "egs_fatal: m");
    }

    #[test]
    fn medium_cards_follow_the_fortran_formats() {
        let b = MediumBlock::from_cards(Pegs4Inp::default(), &format!("{:<30}H2O", "WATER"), "H  O");
        assert_eq!(b.medium, "WATER                   ");
        assert_eq!(b.idstrn.trim_end(), "H2O");
        assert_eq!(b.asym.len(), 24);
        assert_eq!(b.asym[0], "H ");
        assert_eq!(b.asym[1], "O ");
        assert_eq!(b.asym[2], "  ");
        let e = MediumBlock::from_cards(Pegs4Inp::default(), "PB", "PB");
        assert_eq!(e.medium.trim_end(), "PB");
        assert_eq!(e.idstrn.trim(), "");
        assert_eq!(e.asym[0], "PB");
        assert_eq!(Pegs4Block::Stop.option_name(), "STOP");
        assert_eq!(carbon_elem().option_name(), "ELEM");
        assert_eq!(Pegs4Block::Plot(String::from("PLTN"), Pegs4Inp::default()).option_name(), "PLOT");
        for (b, n) in [
            (Pegs4Block::Mixt(MediumBlock::default()), "MIXT"),
            (Pegs4Block::Comp(MediumBlock::default()), "COMP"),
            (Pegs4Block::Mims(Pegs4Inp::default()), "MIMS"),
            (Pegs4Block::Pwlf(Pegs4Inp::default()), "PWLF"),
            (Pegs4Block::Deck(Pegs4Inp::default()), "DECK"),
            (Pegs4Block::Test(Pegs4Inp::default()), "TEST"),
            (Pegs4Block::Dbug(Pegs4Inp::default()), "DBUG"),
            (Pegs4Block::Call(CallBlock::default()), "CALL"),
        ] {
            assert_eq!(b.option_name(), n);
            assert!(b.inp().is_some());
        }
    }

    // ---- the function table ----

    #[test]
    fn function_table_and_ifunt() {
        assert_eq!(FUNCTIONS.len(), 78);
        assert_eq!(ifunt("ALIN"), Some(1));
        assert_eq!(ifunt("XSIF"), Some(78));
        assert_eq!(ifunt("GMFP  "), ifunt("GMFP"));
        assert_eq!(ifunt("GMFP  XYZ"), ifunt("GMFP"), "names are six characters");
        assert_eq!(ifunt("GMFPXX"), None);
        assert_eq!(ifunt("NOPE"), None);
        let mut names: Vec<&str> = FUNCTIONS.iter().map(|f| f.0).collect();
        names.sort_unstable();
        names.dedup();
        assert_eq!(names.len(), 78, "no duplicates");
        for (i, (n, na)) in FUNCTIONS.iter().enumerate() {
            assert_eq!(ifunt(n), Some(i + 1));
            assert!((1..=4).contains(na));
        }
    }

    fn args_for(name: &str) -> [f32; 4] {
        match name {
            "ALIN" | "ALINI" | "AREC" | "ADFMOL" | "ADIMOL" | "ADDMOL" | "ALOG" | "EXP" => [2.0, 0.0, 0.0, 0.0],
            "ALKE" | "ALKEI" => [3.0, 0.0, 0.0, 0.0],
            "AMOLDM" | "ANIHDM" | "BHABDM" => [5.0, 1.0, 0.0, 0.0],
            "AMOLFM" | "ANIHFM" | "BHABFM" => [1.0, 0.0, 0.0, 0.0],
            "AMOLRM" => [5.0, 0.6, 2.8, 0.0],
            "ANIHRM" => [5.0, 0.1, 2.0, 0.0],
            "BHABRM" => [5.0, 0.6, 5.0, 0.0],
            "APRIM" => [6.0, 5.0, 0.0, 0.0],
            "BREMDR" => [5.0, 1.0, 0.0, 0.0],
            "BREMFR" | "BREMFZ" | "BRMSFZ" => [1.0, 0.0, 0.0, 0.0],
            "BREMDZ" | "BRMSDZ" => [6.0, 5.0, 1.0, 0.0],
            "BREMRR" | "BREMRM" | "BRMSRM" => [5.0, 0.1, 4.0, 0.0],
            "BREMRZ" | "BRMSRZ" => [6.0, 5.0, 0.1, 4.0],
            "BRMSTM" => [5.0, 0.1, 0.0, 0.0],
            "COHETM" | "PHOTTE" => [0.1, 0.0, 0.0, 0.0],
            "COHETZ" | "PHOTTZ" => [6.0, 0.1, 0.0, 0.0],
            "COMPDM" => [1.0, 0.5, 0.0, 0.0],
            "COMPFM" => [0.5, 0.0, 0.0, 0.0],
            "COMPRM" => [1.0, 0.3, 1.0, 0.0],
            "COMPTM" | "CRATIO" | "EBIND" | "GBR1" | "GBR2" | "GMFP" => [1.0, 0.0, 0.0, 0.0],
            "FCOULC" | "XSIF" => [6.0, 0.0, 0.0, 0.0],
            "PAIRDR" => [10.0, 3.0, 0.0, 0.0],
            "PAIRFR" | "PAIRFZ" => [3.0, 0.0, 0.0, 0.0],
            "PAIRDZ" => [6.0, 10.0, 3.0, 0.0],
            "PAIRRM" | "PAIRRR" => [10.0, 0.6, 9.0, 0.0],
            "PAIRRZ" => [6.0, 10.0, 0.6, 9.0],
            "PAIRTZ" => [6.0, 10.0, 0.0, 0.0],
            "PAIRTE" | "PAIRTM" | "PAIRTR" | "PAIRTU" => [10.0, 0.0, 0.0, 0.0],
            "SPIONE" | "SPIONP" => [5.0, 0.6, 0.0, 0.0],
            "SPTOTE" | "SPTOTP" => [5.0, 0.6, 0.1, 0.0],
            _ => [5.0, 0.0, 0.0, 0.0],
        }
    }

    #[test]
    fn every_function_in_the_table_dispatches_and_is_finite() {
        let mut p = carbon_program();
        for (i, (name, _na)) in FUNCTIONS.iter().enumerate() {
            let v = p.fi(i + 1, args_for(name)).unwrap_or_else(|e| panic!("{name}: {e:?}"));
            assert!(v.is_finite(), "{name} = {v}");
        }
        assert!(p.fi(0, [0.0; 4]).is_err());
        assert!(p.fi(79, [0.0; 4]).is_err());
        // functions of one variable used by the plotting options
        assert_eq!(p.fi(ifunt("ALIN").unwrap(), [2.5, 0.0, 0.0, 0.0]).unwrap(), 2.5);
        assert_eq!(p.fi(ifunt("ALOG").unwrap(), [1.0, 0.0, 0.0, 0.0]).unwrap(), 0.0);
        let x = p.fi(ifunt("ADFMOL").unwrap(), [2.0, 0.0, 0.0, 0.0]).unwrap();
        let back = p.fi(ifunt("ADIMOL").unwrap(), [x, 0.0, 0.0, 0.0]).unwrap();
        assert!(rel(back, 2.0) < 1e-5, "ADIMOL inverts ADFMOL: {back}");
        let h = 1.0e-2f32;
        let d = (p.adfmol(2.0 + h) - p.adfmol(2.0 - h)) / (2.0 * h);
        assert!(rel(d, p.addmol(2.0)) < 1e-3, "ADDMOL is the derivative of ADFMOL");
        let (alke, alkei) = (p.fi(9, [3.0, 0.0, 0.0, 0.0]).unwrap(), p.alini(1.25));
        assert!(rel(p.c.alkei(alke), 3.0) < 1e-5);
        assert_eq!(alkei, 1.25);
        assert_eq!(p.arec(4.0), 0.25);
        assert_eq!(p.alin(7.0), 7.0);
    }

    // ---- differential cross sections against their totals ----

    /// Composite Simpson integral in `f64` of a function that may mutate
    /// state (the `L...` commons).
    fn simpson<F: FnMut(f32) -> f32>(mut f: F, a: f32, b: f32, n: usize) -> f64 {
        let h = (f64::from(b) - f64::from(a)) / n as f64;
        let mut s = f64::from(f(a)) + f64::from(f(b));
        for i in 1..n {
            let x = (f64::from(a) + h * i as f64) as f32;
            s += f64::from(f(x)) * if i % 2 == 1 { 4.0 } else { 2.0 };
        }
        s * h / 3.0
    }

    #[test]
    fn differentials_integrate_to_their_totals() {
        let mut p = carbon_program();
        let rm = p.rm();
        // Compton (SLAC-265 2.9.1 against 2.9.2/2.9.7)
        let k0 = 1.0f32;
        let k1 = k0 * rm / (rm + 2.0 * k0);
        let total = f64::from(p.comptm(k0));
        let num = simpson(|k| p.compdm(k0, k), k1, k0, 2000);
        assert!((num / total - 1.0).abs() < 1e-3, "Compton: {num} vs {total}");
        let part = f64::from(p.comprm(k0, 0.3, 0.8));
        let num = simpson(|k| p.compdm(k0, k), 0.3, 0.8, 2000);
        assert!((num / part - 1.0).abs() < 1e-3, "Compton range: {num} vs {part}");
        // Moller (2.10.1 against 2.10.3), Bhabha (2.11.1 against 2.11.2)
        let (en0, ae) = (5.0f32, p.c.aep);
        let t0 = en0 - rm;
        let tot = f64::from(p.c.amolrm(en0, ae, t0 * 0.5 + rm));
        let num = simpson(|e| p.amoldm(en0, e), ae, t0 * 0.5 + rm, 2000);
        assert!((num / tot - 1.0).abs() < 1e-3, "Moller: {num} vs {tot}");
        let tot = f64::from(p.c.bhabrm(en0, ae, en0));
        let num = simpson(|e| p.bhabdm(en0, e), ae, en0, 2000);
        assert!((num / tot - 1.0).abs() < 1e-3, "Bhabha: {num} vs {tot}");
        // annihilation in flight (2.12.1 against 2.12.11)
        let (k1, k2) = (0.2f32, 2.5f32);
        let tot = f64::from(p.anihrm(en0, k1, k2));
        let num = simpson(|k| p.anihdm(en0, k), k1, k2, 2000);
        assert!((num / tot - 1.0).abs() < 1e-3, "annihilation: {num} vs {tot}");
        // AMOLFM, ANIHFM, BHABFM, COMPFM keep the state of the last D call
        let _ = p.bhabdm(en0, 1.0);
        let f = p.bhabfm(1.0);
        assert_eq!(f, p.bhabdm(en0, 1.0));
        let _ = p.compdm(k0, 0.5);
        assert_eq!(p.compfm(0.5), p.compdm(k0, 0.5));
        let _ = p.amoldm(en0, 1.0);
        assert_eq!(p.amolfm(1.0), p.amoldm(en0, 1.0));
        let _ = p.anihdm(en0, 1.0);
        assert_eq!(p.anihfm(1.0), p.anihdm(en0, 1.0));
        // COMPRM keeps its double-precision intermediates: compare with an
        // independent f64 evaluation of SLAC-265 2.9.2
        let kk = p.c.k;
        let k0p = f64::from(k0) / f64::from(kk.rmp);
        let cc = f64::from(p.c.rlcp) * f64::from(p.c.eden) * f64::from(kk.pip) * f64::from(kk.r0) * f64::from(kk.r0) / k0p;
        let (e1, e2) = (0.3f64, 0.8f64);
        let c1 = 1.0 / (k0p * k0p);
        let c2 = 1.0 - (2.0 + 2.0 * k0p) / (k0p * k0p);
        let c3 = (1.0 + 2.0 * k0p) / (k0p * k0p);
        let exact = cc * (c1 * (1.0 / e1 - 1.0 / e2) + c2 * (e2 / e1).ln() + e2 * (c3 + 0.5 * e2) - e1 * (c3 + 0.5 * e1));
        assert!((f64::from(p.comprm(k0, 0.3, 0.8)) / exact - 1.0).abs() < 1e-5);
    }

    #[test]
    fn run_time_bremsstrahlung_and_pair_integrals_are_consistent() {
        let mut p = carbon_program();
        // additivity of the QD integrals over adjacent ranges
        let (e, k1, k2, k3) = (5.0f32, 0.1f32, 1.5f32, 4.0f32);
        let whole = p.bremrr(e, k1, k3);
        let parts = p.bremrr(e, k1, k2) + p.bremrr(e, k2, k3);
        assert!(whole > 0.0 && rel(whole, parts) < 1e-3, "BREMRR {whole} vs {parts}");
        let (kk, e1, e2, e3) = (10.0f32, 0.6f32, 3.0f32, 9.0f32);
        let whole = p.pairrr(kk, e1, e3);
        let parts = p.pairrr(kk, e1, e2) + p.pairrr(kk, e2, e3);
        assert!(whole > 0.0 && rel(whole, parts) < 1e-3, "PAIRRR {whole} vs {parts}");
        let whole = p.pairrz(6.0, kk, e1, e3);
        let parts = p.pairrz(6.0, kk, e1, e2) + p.pairrz(6.0, kk, e2, e3);
        assert!(whole > 0.0 && rel(whole, parts) < 1e-3, "PAIRRZ {whole} vs {parts}");
        // total of the run-time forms below the 0.1 MeV threshold is zero
        assert_eq!(p.bremtr(0.05), 0.0);
        assert_eq!(p.pairtm(1.0), 0.0);
        assert_eq!(p.pairtr(1.0), 0.0);
        assert_eq!(p.pairte(1.0), 0.0);
        assert_eq!(p.pairtz(6.0, 1.0), 0.0);
        // BREMFR is zero beyond DELPOS
        p.bremdr(1.0, 0.5);
        assert_eq!(p.bremfr(0.9999999), 0.0);
        // the empirical and the theoretical pair totals are two
        // parametrisations of one quantity: a smoke check only, with a bound
        // chosen before measuring (30 %), and the synthetic table is shaped
        // like nothing physical, so only the theoretical ones are compared
        let tm = p.pairtm(100.0);
        let tr = p.pairtr(100.0);
        assert!(tm > 0.0 && tr > 0.0);
        assert!(rel(tr, tm) < 0.30, "PAIRTR {tr} vs PAIRTM {tm}");
        assert_eq!(p.pairtu(100.0), p.pairtm(100.0));
        assert_eq!(p.pairtu(10.0), p.pairte(10.0));
        // PAIRRM at fixed state is the mix over the single element
        let m = p.pairrm(100.0, 0.511, 99.0);
        let z = p.pairrz(6.0, 100.0, 0.511, 99.0);
        assert!(rel(m, z) < 1e-5, "one element, PZ = 1");
    }

    #[test]
    fn photon_functions_are_mutually_consistent() {
        let mut p = carbon_program();
        for e in [0.02f32, 0.5, 3.0, 20.0] {
            let v = p.gfuns(e);
            assert_eq!(v[0], p.gmfp(e));
            let close = |a: f32, b: f32| (a - b).abs() <= 1e-6 * b.abs().max(1e-30);
            assert!(close(v[1], p.gbr1(e)), "GBR1 at {e}: {} vs {}", v[1], p.gbr1(e));
            assert!(close(v[2], p.gbr2(e)));
            assert!(close(v[3], p.cratio(e)));
            assert!(v[3] > 0.0 && v[3] <= 1.0, "coherent ratio {}", v[3]);
            assert!(v[1] <= v[2] + 1e-7);
            let tot = p.pairtu(e) + p.comptm(e) + p.photte(e);
            assert!(rel(1.0 / tot, v[0]) < 1e-6);
        }
        // photo-electric and coherent totals follow the single element
        assert_eq!(p.photte(0.1), p.phottz(6.0, 0.1));
        assert_eq!(p.cohetm(0.1), p.cohetz(6.0, 0.1));
        // the K-edge average is the K edge of the only element, in MeV
        let eb = p.ebind(0.1);
        assert!(rel(eb, p.data.ekedge[5] * 0.001) < 1e-5, "EBIND {eb}");
        // data out of range is NaN, not a panic
        assert!(p.phottz(0.0, 0.1).is_nan());
        assert!(p.cohetz(101.0, 0.1).is_nan());
        assert!(p.pairtz(0.0, 10.0).is_nan());
        assert!(p.phottz(2.0, 0.1).is_nan(), "no table for Z = 2 in the synthetic data");
    }

    #[test]
    fn electron_branching_and_stopping_functions() {
        let mut p = carbon_program();
        let e = 5.0f32;
        let esig = p.esig(e).unwrap();
        assert_eq!(esig, p.c.bremtm(e).unwrap() + p.c.amoltm(e));
        assert_eq!(p.psig(e).unwrap(), p.c.bremtm(e).unwrap() + p.c.bhabtm(e) + p.c.anihtm(e));
        let b = p.ebr1(e).unwrap();
        assert!(b > 0.0 && b < 1.0);
        assert_eq!(p.ebr1(0.52).unwrap(), 0.0, "below the Moller and brems thresholds");
        let (b1, b2) = (p.pbr1(e).unwrap(), p.pbr2(e).unwrap());
        assert!(0.0 < b1 && b1 < b2 && b2 <= 1.0);
        // restricted collision stopping power is below the unrestricted one
        let (ae, ap) = (p.c.aep, p.c.app);
        let restricted = p.spione_pegs4(e, ae);
        let unrestricted = p.spione_pegs4(e, e);
        assert!(restricted > 0.0 && restricted < unrestricted);
        assert!(p.spionp_pegs4(e, e) > 0.0);
        assert_eq!(p.ededx(e).unwrap(), p.sptote_pegs4(e, ae, ap).unwrap());
        assert_eq!(p.pdedx(e).unwrap(), p.sptotp_pegs4(e, ae, ap).unwrap());
        assert_eq!(p.ededx(e).unwrap(), restricted + p.c.brmstm(e, ap).unwrap());
        // the IUNRST modes are the documented sums
        for (iu, expect) in [
            (1, unrestricted),
            (2, unrestricted + p.c.brmstm(e, e).unwrap()),
            (3, unrestricted + p.c.brmstm(e, ap).unwrap()),
            (4, restricted + p.c.brmstm(e, e).unwrap()),
            (5, p.c.brmstm(e, e).unwrap()),
            (6, p.c.brmstm(e, ap).unwrap()),
            (7, restricted),
        ] {
            p.c.iunrstp = iu;
            assert_eq!(p.sptote_pegs4(e, ae, ap).unwrap(), expect, "IUNRST = {iu}");
        }
        p.c.iunrstp = 9;
        assert!(p.sptote_pegs4(e, ae, ap).is_err());
        assert!(p.sptotp_pegs4(e, ae, ap).is_err());
        assert!(p.efuns_pegs4(e).is_err());
        p.c.iunrstp = 0;
        // multiple-scattering step: TMXDE2 is TMXB over E^2 beta^4
        let rmsq = p.c.k.rmpsq;
        let beta2 = 1.0 - rmsq / (e * e);
        assert!(rel(p.tmxde2(e), p.c.tmxb(e) / (e * e * beta2 * beta2)) < 1e-5);
    }

    #[test]
    fn efuns_pegs4_covers_every_iunrst_mode() {
        let mut p = carbon_program();
        let e = 3.0f32;
        let regular = p.efuns_pegs4(e).unwrap();
        assert_eq!(regular[0], p.esig(e).unwrap());
        assert_eq!(regular[1], p.psig(e).unwrap());
        assert_eq!(regular[2], p.ededx(e).unwrap());
        assert_eq!(regular[3], p.pdedx(e).unwrap());
        assert_eq!(regular[7], p.c.tmxs(e));
        // below the Moller threshold there is no cross section
        let low = p.efuns_pegs4(0.52).unwrap();
        assert_eq!(low[0], 0.0);
        assert_eq!(low[4], 1.0, "THBREM <= THMOLL: the whole branching ratio goes to brems");
        for iu in [1, 2, 3, 4] {
            p.c.iunrstp = iu;
            let v = p.efuns_pegs4(e).unwrap();
            assert!(v.iter().all(|x| x.is_finite()), "IUNRST = {iu}: {v:?}");
            assert_eq!(v[7], p.c.tmxs(e));
        }
        p.c.iunrstp = 5;
        let v5 = p.efuns_pegs4(e).unwrap();
        assert_eq!(v5[0], regular[0], "IUNRST = 5 shares the regular branch of EFUNS");
        p.c.iunrstp = 6;
        assert!(p.efuns_pegs4(e).is_err());
    }

    // ---- MIX, SPINIT ----

    #[test]
    fn mix_pegs4_differs_from_the_library_mix_only_in_the_rlc_rounding() {
        let p = carbon_program();
        let mut lib = Pegs4Common::new(pmdcon());
        lib.asymt = p.c.asymt.clone();
        lib.nep = 1;
        lib.pzp = vec![1.0];
        lib.zelemp = vec![6.0];
        lib.wap = vec![12.01115];
        lib.rhozp = vec![12.01115];
        lib.rhop = 2.26;
        lib.mix();
        // RLC: two different f32 evaluation orders of one expression
        assert!(rel(p.c.rlcp, lib.rlcp) < 1e-6, "{} vs {}", p.c.rlcp, lib.rlcp);
        assert_eq!(p.c.eden, lib.eden);
        assert_eq!(p.c.zt, lib.zt);
        assert_eq!(p.c.wm, lib.wm);
        assert!(p.c.xccp.is_finite());
        eprintln!("RLC (cm): stand-alone {:e}, library order {:e}", p.c.rlcp, lib.rlcp);
        // an independent f64 evaluation of the radiation length of carbon
        // with this file's constants and the Tsai table (Z <= 4 branch not
        // used: Z = 6)
        let k = p.c.k;
        let z = 6.0f64;
        let fsc = f64::from(k.fsc);
        let asq = (fsc * z) * (fsc * z);
        let fc = asq * (1.0 / (1.0 + asq) + 0.20206 + asq * (-0.0369 + asq * (0.0083 + asq * (-0.002))));
        let xsi = (1194.0f64 * z.powf(-2.0 / 3.0)).ln() / ((184.15f64 * z.powf(-1.0 / 3.0)).ln() - fc);
        let zab = z * (z + xsi) * (184.15f64.ln() + z.powf(-1.0 / 3.0).ln());
        let zf = z * (z + xsi) * fc;
        let n = f64::from(k.an) * 2.26 / f64::from(12.01115f32);
        let rlc = 1.0 / ((zab - zf) * n * 4.0 * fsc * f64::from(k.r0) * f64::from(k.r0));
        assert!((f64::from(p.c.rlcp) / rlc - 1.0).abs() < 1e-4, "RLC {} vs f64 {rlc}", p.c.rlcp);
        // MIX scales RHO by GASP in place
        let mut g = Pegs4Common::new(pmdcon());
        g.asymt = p.c.asymt.clone();
        g.nep = 1;
        g.pzp = vec![1.0];
        g.zelemp = vec![2.0];
        g.wap = vec![4.0026];
        g.rhozp = vec![4.0026];
        g.rhop = 0.19;
        g.gaspp = 2.0;
        let mut q = Pegs4Program::new(Pegs4Data::empty());
        q.c = g;
        q.mix_pegs4();
        assert_eq!(q.c.rhop, 0.38);
    }

    fn sp_program(zs: &[&str], pz: &[f32], rho: f32, id: &str, inp: Pegs4Inp) -> Result<Pegs4Program, String> {
        let mut p = Pegs4Program::new(synthetic_data());
        let mut inp = inp;
        inp.ne = Some(zs.len());
        inp.pz = Some(pz.to_vec());
        inp.rho = Some(rho);
        let blk = if zs.len() == 1 { elem(zs[0], "X", id, inp) } else { comp(zs, "X", id, inp) };
        p.run(&[ener(0.521, 10.511, 0.01, 10.0), blk])?;
        Ok(p)
    }

    #[test]
    fn spinit_ssb_table_lookup_and_continuity_of_the_fit() {
        // a medium found in the Sternheimer-Seltzer-Berger table
        let p = sp_program(&["C"], &[1.0], 2.26, "AL", inp0()).unwrap();
        let m = SSB_NAMES.iter().position(|n| *n == "AL").unwrap();
        assert_eq!(m, 12);
        assert_eq!(
            [p.c.afact, p.c.sk, p.c.x0, p.c.x1, p.c.iev, p.c.cbar],
            SSB_DATA[m],
            "the row of the matching table entry"
        );
        assert_eq!(SSB_NAMES.len(), 73);
        assert_eq!(SSB_NAMES[0], "H2-GAS");
        assert_eq!(SSB_DATA[0], [0.03535, 6.790, 1.864, 3.5, 19.2, 9.584]);
        assert_eq!(SSB_NAMES[72], "CSI");
        assert_eq!(SSB_DATA[72], [0.1796, 2.840, 0.0395, 3.5, 553.1, 6.281]);
        // not in the table: the Sternheimer-Peierls formula with IEV = ITBL(Z)
        let p = sp_program(&["C"], &[1.0], 2.26, "NOT-A-TABLE-MEDIUM", inp0()).unwrap();
        assert_eq!(p.c.iev, 78.0, "ITBL(6) in eV");
        assert_eq!(p.c.sk, 3.0);
        assert_eq!(p.c.x1, 2.0, "solid with I < 100 eV");
        assert!(p.c.x0 >= 0.2 && p.c.x0 < p.c.x1);
        // the fit is continuous at X0 (DELTA = 0) and at X1 by construction
        let (t, cb, x0, x1, a, s) = (p.c.toln10, p.c.cbar, p.c.x0, p.c.x1, p.c.afact, p.c.sk);
        let at_x0 = t * x0 - cb + a * powf(x1 - x0, s);
        assert!(at_x0.abs() < 1e-4 * cb.abs(), "DELTA(X0) = {at_x0}");
        // SPC1, SPC2 as the formulas give them
        let k = p.c.k;
        assert_eq!(p.c.spc1, 2.0 * k.pip * (k.r0 * k.r0) * k.rmp * p.c.eden * p.c.rlcp);
        let r = p.c.iev * 1.0e-6 / k.rmp;
        assert_eq!(p.c.spc2, ln(r * r / 2.0));
        // STOPPING POWER BETWEEN X0 AND X1 follows the fit, below X0 it is
        // delta-free: check DELTA through SPIONB at two energies
        let e_lo = 0.6f32; // X below X0
        let e_hi = 9.0f32;
        let rm = k.rmp;
        for e in [e_lo, e_hi] {
            let g = e / rm;
            let tt = g - 1.0;
            let eta2 = tt * (g + 1.0);
            let x = 0.21715 * ln(eta2);
            let beta2 = eta2 / (g * g);
            let d = (p.c.aep / rm - 1.0).min(0.5 * tt);
            let fterm = -1.0 - beta2 + ln((tt - d) * d) + tt / (tt - d) + (d * d / 2.0 + (2.0 * tt + 1.0) * ln(1.0 - d / tt)) / (g * g);
            let delta = if x <= x0 {
                0.0
            } else if x < x1 {
                t * x - cb + a * powf(x1 - x, s)
            } else {
                t * x - cb
            };
            let want = (p.c.spc1 / beta2) * (ln(tt + 2.0) - p.c.spc2 + fterm - delta);
            let mut q = p.clone();
            let got = q.spione_pegs4(e, p.c.aep);
            assert_eq!(got, want, "E = {e}");
        }
    }

    #[test]
    fn spinit_bragg_rule_gas_pressure_user_parameters_and_errors() {
        // water-like compound, not in the table: Bragg additivity of ln I
        let p = sp_program(&["H", "O"], &[2.0, 1.0], 1.0, "WATER-LIKE", inp0()).unwrap();
        let (zh, zo) = (1.0f64, 8.0f64);
        let (ph, po) = (2.0f64, 1.0f64);
        let zc = ph * zh + po * zo;
        let want = ((ph * zh * 19.2f64.ln() + po * zo * 106.0f64.ln()) / zc).exp();
        assert!((f64::from(p.c.iev) / want - 1.0).abs() < 1e-5, "IEV {} vs {want}", p.c.iev);
        // the same compound as a gas: different I values for O and a
        // pressure correction on CBAR, X0 and X1
        let gas_inp = Pegs4Inp { gasp: Some(2.0), iaprim: Some(0), ..Default::default() };
        let g = sp_program(&["H", "O"], &[2.0, 1.0], 1.0, "WATER-LIKE", gas_inp.clone()).unwrap();
        let want_g = ((ph * zh * 19.2f64.ln() + po * zo * 97.0f64.ln()) / zc).exp();
        assert!((f64::from(g.c.iev) / want_g - 1.0).abs() < 1e-5);
        // the pressure correction shifts X1 (a gas has X1 = 4 or 5 before it)
        let shift = ln(2.0) / g.c.toln10;
        let before = g.c.x1 + shift;
        assert!((before - 4.0).abs() < 1e-5 || (before - 5.0).abs() < 1e-5, "X1 + ln(GASP)/TOLN10 = {before}");
        // user-supplied parameters (ISSB = 1)
        let user = Pegs4Inp {
            issb: Some(1.0),
            afact: Some(0.1),
            sk: Some(3.0),
            x0: Some(0.2),
            x1: Some(2.0),
            iev: Some(78.0),
            cbar: Some(3.0),
            iaprim: Some(0),
            ..Default::default()
        };
        let u = sp_program(&["C"], &[1.0], 2.26, "USER", user.clone()).unwrap();
        assert_eq!((u.c.afact, u.c.cbar, u.c.x0, u.c.x1), (0.1, 3.0, 0.2, 2.0));
        let mut missing = user.clone();
        missing.x0 = Some(0.0);
        let e = sp_program(&["C"], &[1.0], 2.26, "USER", missing).unwrap_err();
        assert!(e.contains("21"), "{e}");
        // overriding SSB data without ISSB is refused
        let bad = Pegs4Inp { afact: Some(0.1), iaprim: Some(0), ..Default::default() };
        let e = sp_program(&["C"], &[1.0], 2.26, "USER", bad).unwrap_err();
        assert!(e.contains("OVERRIDE"), "{e}");
        // H, N, O alone cannot be a medium (they are diatomic)
        let e = sp_program(&["H"], &[1.0], 0.09, "NOT-IN-TABLE", inp0()).unwrap_err();
        assert!(e.contains("DIATOMIC"), "{e}");
        let p = sp_program(&["H"], &[1.0], 0.09, "H2-GAS", inp0()).unwrap();
        assert_eq!(p.c.iev, 19.2);
        // an EPSTFL outside 0..1 is reset to 0
        let weird = Pegs4Inp { epstfl: Some(5), iaprim: Some(0), ..Default::default() };
        let p = sp_program(&["C"], &[1.0], 2.26, "AL", weird).unwrap();
        assert_eq!(p.c.epstflp, 0);
        // the hydrogen liquid table entry
        let p = sp_program(&["H", "H"], &[1.0, 1.0], 0.07, "H2-LIQUID", inp0()).unwrap();
        assert_eq!(p.c.afact, SSB_DATA[1][0], "H2-LIQUID is in the table");
    }

    #[test]
    fn spinit_special_cases_of_the_general_formula() {
        // two hydrogens, not in the table: H2 liquid and gas formulas
        let liquid = sp_program(&["H", "H"], &[1.0, 1.0], 0.07, "X-LIQ", inp0()).unwrap();
        assert_eq!((liquid.c.x0, liquid.c.x1, liquid.c.sk), (0.425, 2.0, 5.949));
        let gas = Pegs4Inp { gasp: Some(1.0), iaprim: Some(0), ..Default::default() };
        let h2g = sp_program(&["H", "H"], &[1.0, 1.0], 0.07, "X-GAS", gas.clone()).unwrap();
        assert_eq!((h2g.c.x0, h2g.c.x1, h2g.c.sk), (1.837, 3.0, 4.754));
        // the gas branches by CBAR; helium gas special case needs Z = 2 data
        // (the synthetic tables have none, but SPINIT does not read them)
        let mut he = Pegs4Program::new(synthetic_data());
        let he_block = elem("HE", "HE", "NOT-IN-TABLE", Pegs4Inp { gasp: Some(1.0), iaprim: Some(0), ..Default::default() });
        he.run(&[ener(0.521, 10.511, 0.01, 10.0), he_block]).unwrap();
        assert_eq!((he.c.x0, he.c.x1, he.c.sk), (2.191, 3.0, 3.297));
        // regular gas branch (nitrogen compound of Z = 7 and 8, as a gas)
        let g = sp_program(&["N", "O"], &[1.0, 1.0], 0.001, "NO-GAS", gas.clone()).unwrap();
        assert!(g.c.x1 == 4.0 || g.c.x1 == 5.0, "gas X1 is 4 or 5: {}", g.c.x1);
        // solids with I >= 100 eV
        let sio = sp_program(&["O", "C"], &[2.0, 1.0], 2.0, "X-HI", inp0()).unwrap();
        assert!(sio.c.iev > 60.0);
        assert!(sio.c.x1 == 2.0 || sio.c.x1 == 3.0);
    }

    fn density_file(rho: f32, z: i32, w: f32) -> String {
        // SYNTHETIC test data, not a physical density correction
        let mut s = String::from("SYNTHETIC DENSITY TABLE (test data)\n");
        s.push_str(&format!("  6   78.0   {rho}   1\n"));
        s.push_str(&format!(" {z} {w}\n"));
        s.push_str("  1.0E-03,0.000E+00  1.0E-02,0.000E+00  1.0E-01,0.100E+00\n");
        s.push_str("  1.0E+00,0.800E+00  1.0E+01,2.500E+00  1.0E+02,4.000E+00\n");
        s
    }

    #[test]
    fn spinit_reads_a_density_file_and_checks_the_composition() {
        let run_with = |text: &str, rho: f32, id: &str| -> Result<Pegs4Program, String> {
            let mut p = Pegs4Program::new(synthetic_data());
            p.density_text = Some(String::from(text));
            let inp = Pegs4Inp { epstfl: Some(1), iaprim: Some(0), rho: Some(rho), ..Default::default() };
            p.run(&[ener(0.521, 10.511, 0.01, 10.0), elem("C", "C", id, inp)])?;
            Ok(p)
        };
        let ok = run_with(&density_file(2.26, 6, 1.0), 2.26, "C").unwrap();
        assert_eq!(ok.c.epstflp, 1);
        assert_eq!((ok.c.nepst, ok.c.neleps), (6, 1));
        assert_eq!(ok.c.iev, 78.0);
        // energies are made total (kinetic + RM); this table already spans AE..UE
        assert_eq!(ok.c.epsten[0], 0.001 + ok.c.k.rmp);
        assert_eq!(ok.c.epsten[1], 0.01 + ok.c.k.rmp);
        assert_eq!(ok.c.epsten[5], 100.0 + ok.c.k.rmp);
        // a table that starts above AE and ends below UE is clamped to them
        let narrow = density_file(2.26, 6, 1.0)
            .replace("1.0E-03,0.000E+00", "1.0E+00,0.000E+00")
            .replace("1.0E+02,4.000E+00", "5.0E+00,4.000E+00");
        let c = run_with(&narrow.replace("1.0E-02,0.000E+00", "2.0E+00,0.000E+00").replace("1.0E-01,0.100E+00", "3.0E+00,0.100E+00").replace("1.0E+00,0.800E+00", "4.0E+00,0.800E+00").replace("1.0E+01,2.500E+00", "4.5E+00,2.500E+00"), 2.26, "C").unwrap();
        assert_eq!(c.c.epsten[0], 0.521, "clamped to AE");
        assert_eq!(c.c.epsten[5], 10.511, "clamped to UE");
        assert!(ok.c.epsttl.starts_with("SYNTHETIC DENSITY TABLE"));
        // the table is used by SPIONB (EPSTFL = 1) and agrees with the library
        // routine, which always interpolates
        let mut a = ok.clone();
        let mut lib = ok.c.clone();
        for e in [0.6f32, 1.5, 4.0, 9.9] {
            assert_eq!(a.spionb_pegs4(e, e, false), lib.spionb(e, e, false), "electron {e}");
            assert_eq!(a.spionb_pegs4(e, 0.6, true), lib.spionb(e, 0.6, true), "positron {e}");
        }
        // EFUNS agrees too (same table)
        let mut b = ok.clone();
        let mut lib2 = ok.c.clone();
        assert_eq!(b.efuns_pegs4(3.0).unwrap(), lib2.efuns(3.0).unwrap());
        // mismatches are refused with exit 23
        let e = run_with(&density_file(2.26, 6, 1.0), 3.5, "C").unwrap_err();
        assert!(e.contains("23"), "density mismatch: {e}");
        let e = run_with(&density_file(2.26, 8, 1.0), 2.26, "C").unwrap_err();
        assert!(e.contains("23"), "wrong element: {e}");
        let e = run_with(&density_file(2.26, 6, 0.5), 2.26, "C").unwrap_err();
        assert!(e.contains("23"), "wrong weight fraction: {e}");
        let mut two = density_file(2.26, 6, 1.0);
        two = two.replace("  6   78.0   2.26   1", "  6   78.0   2.26   2");
        two = two.replace(" 6 1\n", " 6 0.5 8 0.5\n");
        assert!(run_with(&two, 2.26, "C").is_err(), "NELEPS differs from NE");
        // no file, too many energies, too many elements
        let mut p = Pegs4Program::new(synthetic_data());
        let inp = Pegs4Inp { epstfl: Some(1), iaprim: Some(0), ..Default::default() };
        assert!(p.run(&[ener(0.521, 10.0, 0.01, 10.0), elem("C", "C", "C", inp)]).is_err());
        let big = density_file(2.26, 6, 1.0).replace("  6   78.0", " 151   78.0");
        assert!(run_with(&big, 2.26, "C").unwrap_err().contains("22"));
        let many = density_file(2.26, 6, 1.0).replace("78.0   2.26   1", "78.0   2.26   21");
        assert!(run_with(&many, 2.26, "C").is_err());
    }

    // ---- the option logic ----

    #[test]
    fn options_set_state_and_enforce_their_preconditions() {
        // ENER: negative values are multiples of RM
        let mut p = Pegs4Program::new(synthetic_data());
        let blk = Pegs4Block::Ener(Pegs4Inp { ae: Some(-1.5), ue: Some(-100.0), ap: Some(-0.5), up: Some(50.0), ..Default::default() });
        p.run(&[blk]).unwrap();
        let rm = p.c.k.rmp;
        assert_eq!(p.c.aep, 1.5 * rm);
        assert_eq!(p.c.uep, 100.0 * rm);
        assert_eq!(p.c.app, 0.5 * rm);
        assert_eq!(p.c.upp, 50.0);
        assert_eq!(p.c.tep, p.c.aep - rm);
        assert_eq!(p.tet2, p.c.tep * 2.0);
        assert_eq!(p.tem, p.c.tep / rm);
        assert_eq!(p.c.thbremp, rm + p.c.app);
        assert_eq!(p.c.thmollp, p.c.aep + p.c.tep);
        assert!(p.engset && !p.medset);
        // PWLF needs both
        let e = p.run(&[Pegs4Block::Pwlf(Pegs4Inp::default())]).unwrap_err();
        assert!(e.contains("PWLF REQ. IGNORED"), "{e}");
        // MIXT and COMP need NE > 1
        let mb = MediumBlock { inp: Pegs4Inp::default(), medium: String::new(), idstrn: String::new(), asym: vec![String::from("C")] };
        assert!(Pegs4Program::new(synthetic_data()).run(&[Pegs4Block::Mixt(mb.clone())]).is_err());
        let e = Pegs4Program::new(synthetic_data()).run(&[Pegs4Block::Comp(mb.clone())]).unwrap_err();
        assert!(e.contains("17"), "{e}");
        // an unknown symbol aborts with exit 16, as do too few symbols
        let e = Pegs4Program::new(synthetic_data()).run(&[elem("XX", "X", "X", Pegs4Inp::default())]).unwrap_err();
        assert!(e.contains("BAD ATOMIC SYMBOL"), "{e}");
        let two = Pegs4Inp { ne: Some(2), pz: Some(vec![1.0, 1.0]), rho: Some(1.0), ..Default::default() };
        let e = Pegs4Program::new(synthetic_data()).run(&[comp(&["C"], "X", "X", two)]).unwrap_err();
        assert!(e.contains("element symbols"), "{e}");
        // NE and array sizes are checked
        let toomany = Pegs4Inp { ne: Some(21), ..Default::default() };
        assert!(Pegs4Program::new(synthetic_data()).run(&[Pegs4Block::Dbug(toomany)]).is_err());
        let long = Pegs4Inp { pz: Some(vec![1.0; 21]), ..Default::default() };
        assert!(Pegs4Program::new(synthetic_data()).run(&[Pegs4Block::Dbug(long)]).is_err());
        // NALE beyond the array is refused (upstream would overrun AFE)
        let mut q = carbon_program();
        let big = Pegs4Inp { nale: Some(151), ..Default::default() };
        assert!(q.run(&[Pegs4Block::Pwlf(big)]).unwrap_err().contains("NALE"));
        let big = Pegs4Inp { nale: Some(150), nalg: Some(2001), ..Default::default() };
        assert!(q.run(&[Pegs4Block::Pwlf(big)]).unwrap_err().contains("NALG"));
        let big = Pegs4Inp { nale: Some(150), nalg: Some(2000), nalr: Some(101), ..Default::default() };
        assert!(q.run(&[Pegs4Block::Pwlf(big)]).unwrap_err().contains("NALR"));
        let none = Pegs4Inp { nale: Some(0), nalg: Some(2000), nalr: Some(100), ..Default::default() };
        assert!(q.run(&[Pegs4Block::Pwlf(none)]).is_err());
    }

    #[test]
    fn block_semantics_rho_restore_wrong_block_guard_stop_and_call() {
        let mut p = Pegs4Program::new(synthetic_data());
        let out = p
            .run(&[
                carbon_elem(),
                // the density of a MIXT/COMP without RHO is the PREVIOUS one (:812-816)
                comp(&["C", "C"], "CC", "C", Pegs4Inp { ne: Some(2), pz: Some(vec![1.0, 1.0]), ..inp_reset() }),
            ])
            .unwrap();
        assert_eq!(p.c.rhop, 2.26, "ELEM took RHOTBL(6) and the COMP inherited it");
        assert!(out.listing.iter().any(|l| l.contains("Sternheimer")));
        // an ELEM without RHO takes the table value, not the previous one
        let mut p = Pegs4Program::new(synthetic_data());
        p.run(&[
            elem("C", "C", "C", Pegs4Inp { rho: Some(5.0), iaprim: Some(0), ..Default::default() }),
            elem("C", "C", "C", inp_reset()),
        ])
        .unwrap();
        assert_eq!(p.c.rhop, 2.26);
        // IUNRST, EPSTFL and IAPRIM set in a non-medium block are reset
        let wrong = Pegs4Inp { iunrst: Some(3), epstfl: Some(1), iaprim: Some(2), ..Default::default() };
        let out = p.run(&[Pegs4Block::Ener(wrong)]).unwrap();
        assert_eq!((p.c.iunrstp, p.c.epstflp, p.c.iaprimp), (0, 0, 0));
        assert_eq!(out.listing.iter().filter(|l| l.contains("wrong OPTION block")).count(), 3);
        // RHO and WA of a non-medium block are restored
        let rho = p.c.rhop;
        p.run(&[Pegs4Block::Dbug(Pegs4Inp { rho: Some(99.0), wa: Some(vec![1.0]), ..Default::default() })]).unwrap();
        assert_eq!(p.c.rhop, rho);
        assert_eq!(p.wa[0], WATBL[5], "WA(1) restored to the table value of the medium");
        // STOP ends the run; later blocks are not executed
        let mut q = Pegs4Program::new(synthetic_data());
        let out = q.run(&[Pegs4Block::Stop, carbon_elem()]).unwrap();
        assert!(!q.medset);
        assert!(out.listing[0].contains("EXIT FROM PEGS4"));
        // the unported options are accepted and say so
        let out = q
            .run(&[
                Pegs4Block::Test(Pegs4Inp::default()),
                Pegs4Block::Dbug(Pegs4Inp::default()),
                Pegs4Block::Plot(String::from("HPLT"), Pegs4Inp::default()),
            ])
            .unwrap();
        assert_eq!(out.listing.len(), 2);
        // CALL: GMFP at XP(1) after the medium and the limits are set
        let mut r = Pegs4Program::new(synthetic_data());
        let call = |name: &str, xp: Vec<f32>| {
            Pegs4Block::Call(CallBlock { inp: Pegs4Inp { xp: Some(xp), ..Default::default() }, name: String::from(name) })
        };
        let out = r
            .run(&[carbon_elem(), ener(0.521, 10.511, 0.01, 10.0), call("GMFP", vec![1.5]), call("NOSUCH", vec![1.5]), call("GMFP", vec![])])
            .unwrap();
        assert_eq!(out.calls.len(), 2, "the unknown name is only a message");
        assert_eq!(out.calls[0].name, "GMFP");
        assert_eq!(out.calls[0].ifun, ifunt("GMFP").unwrap());
        assert_eq!(out.calls[0].value, r.gmfp(1.5));
        assert_eq!(out.calls[1].xp[0], 1.5, "XP persists between blocks");
        assert!(out.listing.iter().any(|l| l.contains("NOSUCH")));
        // a failing function surfaces as an Err
        let bad = r.run(&[Pegs4Block::Call(CallBlock { inp: Pegs4Inp { iunrst: Some(9), ..Default::default() }, name: String::from("EDEDX") })]);
        assert!(bad.is_ok(), "IUNRST set in a CALL block is reset (wrong block), so EDEDX works");
        r.c.iunrstp = 9;
        assert!(r.run(&[call("EDEDX", vec![5.0])]).is_err());
    }

    // ---- end to end: ELEM, ENER, PWLF, DECK, and the round trip ----

    fn run_carbon_deck() -> (Pegs4Program, Pegs4Output) {
        let mut p = Pegs4Program::new(synthetic_data());
        let out = p
            .run(&[
                carbon_elem(),
                ener(0.521, 10.511, 0.01, 10.0),
                Pegs4Block::Pwlf(Pegs4Inp::default()),
                Pegs4Block::Deck(Pegs4Inp::default()),
            ])
            .unwrap();
        (p, out)
    }

    #[test]
    fn carbon_deck_has_the_upstream_layout_and_round_trips_through_the_reader() {
        let (p, out) = run_carbon_deck();
        let d = &out.deck;
        assert_eq!(d[0], format!(" MEDIUM={:<24},STERNCID={:<24}", "C", "C-2.265 G/CM**3"));
        assert!(d[1].starts_with(" ELEM,RHO= 2.2600E+00,NE= 1, IUNRST=0, EPSTFL=0, IAPRIM=0"), "{}", d[1]);
        assert!(d[2].starts_with(" ASYM=C ,Z= 6.,A=   12.011,PZ= 1.00000E+00,RHOZ= 1.20112E+01"), "{}", d[2]);
        // record counts of every block
        let (nel, ngl) = (p.nel, p.ngl);
        assert!(nel >= 3 && ngl >= 3, "NEL = {nel}, NGL = {ngl}");
        let recs = |n: usize, per: usize| (n + per - 1) / per;
        let expect = 3 + recs(5, 5) + recs(9, 14) + recs(36, 5) + recs(7, 5) + recs(4, 5) + recs(2, 5) + recs(16 * nel, 5)
            + recs(3, 5) + recs(6 * ngl, 5);
        assert_eq!(d.len(), expect);
        assert!(d.iter().all(|l| l.starts_with(' ')), "every record begins with the format's blank");
        // the sizes record: NSGE NGE NSEKE NEKE NLEKE NCMFP NRANGE IRAYL IUNRST (format 1X,14I5)
        assert_eq!(d[4], format!(" {:>5}{:>5}{:>5}{:>5}{:>5}{:>5}{:>5}{:>5}{:>5}", 0, ngl, 0, nel, 0, 0, 0, 0, 0));
        // the deck is what parse_pegs4_media reads back
        let meds = parse_pegs4_media(&out.deck_text(), &["C"]).unwrap();
        let m = &meds[0];
        assert_eq!(m.nne, 1);
        assert_eq!((m.iunrst, m.epstfl, m.iaprim), (0, 0, 0));
        assert!(rel(m.rho as f32, 2.26) < 1e-4);
        assert_eq!(m.elements[0].asym.trim(), "C");
        assert_eq!(m.elements[0].zelem, 6.0);
        assert!((m.elements[0].wa - 12.011).abs() < 1e-3);
        assert!(rel(m.rlc as f32, p.c.rlcp) < 1e-5);
        assert!(rel(m.ae as f32, 0.521) < 1e-5 && rel(m.up as f32, 10.0) < 1e-5);
        assert_eq!(m.sizes, [0, ngl as i32, 0, nel as i32, 0, 0, 0]);
        assert_eq!(m.irayl, 0);
        assert!(m.rayleigh.is_none());
        assert_eq!(m.elecin.len(), nel);
        assert_eq!(m.photin.len(), ngl);
        for i in 0..6 {
            for k in 0..6 {
                assert!(rel(m.dl[k][i] as f32, p.c.dlp[k][i]) < 1e-4 || (m.dl[k][i] as f32 - p.c.dlp[k][i]).abs() < 1e-5);
            }
        }
        assert!(rel(m.delcm as f32, p.c.delcmp) < 1e-5);
        assert!(rel(m.xr0 as f32, p.c.xr0p) < 1e-4 && rel(m.xcc as f32, p.c.xccp) < 1e-5);
        assert!(rel(m.eke[0] as f32, p.bxe) < 1e-5 && rel(m.eke[1] as f32, p.axe) < 1e-5);
        assert!(rel(m.ge[0] as f32, p.bxg) < 1e-5 && rel(m.ge[1] as f32, p.axg) < 1e-5);
        // the first electron row is BFE(1,k), AFE(1,k) of the eight functions
        for f in 0..8 {
            let (b, a) = (p.bfe[f * MXEKE], p.afe[f * MXEKE]);
            assert!((m.elecin[0][2 * f] as f32 - b).abs() <= 1e-5 * b.abs().max(1e-30), "BFE function {f}");
            assert!((m.elecin[0][2 * f + 1] as f32 - a).abs() <= 1e-5 * a.abs().max(1e-30), "AFE function {f}");
        }
        for f in 0..3 {
            let (b, a) = (p.bfg[f * MXGE], p.afg[f * MXGE]);
            assert!((m.photin[0][2 * f] as f32 - b).abs() <= 1e-5 * b.abs().max(1e-30), "BFG function {f}");
            assert!((m.photin[0][2 * f + 1] as f32 - a).abs() <= 1e-5 * a.abs().max(1e-30), "AFG function {f}");
        }
        assert!(rel(m.ebinda as f32, p.ebinda) < 1e-4);
    }

    #[test]
    fn the_fits_reproduce_the_functions_they_fit() {
        let (mut p, _) = run_carbon_deck();
        let rm = p.rm();
        let epe = p.pwlf.epe;
        // electron esig and stopping powers at a few energies above AE
        for e in [0.7f32, 1.3, 3.0, 7.0] {
            let v = p.efuns_pegs4(e).unwrap();
            let x = ln(e - rm);
            let interv = (p.axe * x + p.bxe) as usize;
            for f in [0usize, 2, 7] {
                let fit = p.afe[(interv - 1) + f * MXEKE] * x + p.bfe[(interv - 1) + f * MXEKE];
                assert!(rel(fit, v[f]) <= 2.0 * epe, "electron function {f} at {e}: fit {fit}, exact {}", v[f]);
            }
        }
        // photon: GMFP, GBR1, GBR2 and CRATIO
        for e in [0.02f32, 0.3, 2.0, 8.0] {
            let v = p.gfuns(e);
            let x = ln(e);
            let interv = (p.axg * x + p.bxg) as usize;
            let tol = 2.0 * p.pwlf.epg;
            for f in [0usize, 3] {
                let fit = p.afg[(interv - 1) + f * MXGE] * x + p.bfg[(interv - 1) + f * MXGE];
                assert!(rel(fit, v[f]) <= tol, "photon function {f} at {e}: fit {fit}, exact {}", v[f]);
            }
        }
        // a second PWLF repeats the fit identically (no Rayleigh, so no
        // state is changed by the first)
        let (nel, ngl, axe, bxg) = (p.nel, p.ngl, p.axe, p.bxg);
        let again = p.run(&[Pegs4Block::Pwlf(Pegs4Inp::default())]).unwrap();
        assert_eq!((p.nel, p.ngl, p.axe, p.bxg), (nel, ngl, axe, bxg));
        assert!(again.listing.iter().any(|l| l.contains("PWLF electron")));
    }

    #[test]
    fn rayleigh_fit_and_the_squared_in_place_xval() {
        let mut p = Pegs4Program::new(synthetic_data());
        let ray = Pegs4Inp { irayl: Some(1), ne: Some(2), pz: Some(vec![2.0, 1.0]), rho: Some(1.0), iaprim: Some(0), ..Default::default() };
        let x0: Vec<f32> = p.xval.clone();
        let out = p
            .run(&[
                comp(&["H", "O"], "H2O", "H2O", ray),
                ener(0.521, 10.511, 0.01, 10.0),
                Pegs4Block::Pwlf(Pegs4Inp::default()),
                Pegs4Block::Deck(Pegs4Inp::default()),
            ])
            .unwrap();
        assert_eq!(p.irayl, 1);
        assert!(p.ngr >= 1);
        assert_eq!(p.affi[0], 0.0);
        assert!((p.affi[86] - 1.0).abs() < 1e-6, "AFFI is normalised to one at 87");
        assert!(p.affi[..87].windows(2).all(|w| w[1] >= w[0]), "a c.d.f. is non-decreasing");
        for i in 0..100 {
            assert_eq!(p.xval[i], x0[i] * x0[i]);
        }
        // AFAC2 is the PZ-weighted sum of squared form factors
        let i = 17;
        let want = 2.0 * p.data.afac[i] * p.data.afac[i] + 1.0 * p.data.afac[7 * 100 + i] * p.data.afac[7 * 100 + i];
        assert!(rel(p.afac2[i], want) < 1e-6);
        // the Rayleigh block is in the deck and reads back
        let m = &parse_pegs4_media(&out.deck_text(), &["H2O"]).unwrap()[0];
        assert_eq!(m.irayl, 1);
        let r = m.rayleigh.as_ref().unwrap();
        assert_eq!(r.ngr as usize, p.ngr);
        assert_eq!(r.rsct.len(), p.ngr);
        assert_eq!(r.cohe.len(), p.ngl);
        assert!(rel(r.rco[0] as f32, p.bxr) < 1e-5 || (r.rco[0] as f32 - p.bxr).abs() < 1e-5);
        // upstream's oddity (:1004): a second PWLF squares XVAL again
        p.run(&[Pegs4Block::Pwlf(Pegs4Inp::default())]).unwrap();
        for i in 0..100 {
            let sq = x0[i] * x0[i];
            assert_eq!(p.xval[i], sq * sq);
        }
        // RFUNS is AINTP of XVAL against AFFI: exact at a node
        assert!(rel(p.rfuns(p.affi[40]), p.xval[40]) < 1e-5);
    }

    #[test]
    fn gas_header_carries_gasp_and_the_pressure_scaled_density() {
        let mut p = Pegs4Program::new(synthetic_data());
        let gas = Pegs4Inp { gasp: Some(2.0), rho: Some(0.5), iaprim: Some(0), iunrst: Some(0), ..Default::default() };
        let out = p.run(&[elem("C", "C", "NOT-IN-TABLE", gas), ener(0.521, 10.0, 0.01, 10.0), Pegs4Block::Deck(Pegs4Inp::default())]).unwrap();
        assert_eq!(p.c.rhop, 1.0, "MIX multiplies RHO by GASP in place");
        // the layout of the shipped gas record (data/521icru.pegs4dat):
        // " MIXT,RHO= 1.2048E-03,NE= 4,GASP= 1.0000E+00, IUNRST=0, EPSTFL=1, IAPRIM=1"
        assert_eq!(out.deck[1], " ELEM,RHO= 1.0000E+00,NE= 1,GASP= 2.0000E+00, IUNRST=0, EPSTFL=0, IAPRIM=0");
        let shipped = " MIXT,RHO= 1.2048E-03,NE= 4,GASP= 1.0000E+00, IUNRST=0, EPSTFL=1, IAPRIM=1";
        let ours = format!(
            " MIXT,RHO={},NE={},GASP={}, IUNRST={}, EPSTFL={}, IAPRIM={}",
            fmt_e(1.2048e-3, 11, 4),
            fmt_i(4, 2),
            fmt_e(1.0, 11, 4),
            fmt_i(0, 1),
            fmt_i(1, 1),
            fmt_i(1, 1)
        );
        assert_eq!(ours, shipped);
    }

    // ---- MOLIER ----

    #[test]
    fn molier_fills_the_multiple_scattering_tables() {
        let mut p = Pegs4Program::new(Pegs4Data::empty());
        let out = p.run(&[Pegs4Block::Mims(Pegs4Inp::default())]).unwrap();
        let m = &p.mims;
        assert_eq!(out.deck[0], " MATERIAL INDEPENDENT MULTIPLE SCATTERING DATA");
        assert_eq!(out.deck[1], format!(" {:>5}{:>5}{:>5}{:>5}", 200, 16, m.mxv1, m.mxv2));
        assert_eq!(m.mxv1 + m.mxv2, 1000);
        // at the first step R = 1, B = BMIN, so FSQR = sqrt(1*BMIN/BMIN) = 1
        assert!((m.fsqr[0] - 1.0).abs() < 1e-4, "FSQR(1) = {}", m.fsqr[0]);
        assert!(m.fsqr.windows(2).all(|w| w[1] > w[0]), "FSQR grows with the step");
        // MSMAP(J) is the largest step index with FSTEP <= J
        assert_eq!(m.msmap[0], 1);
        assert_eq!(m.msmap[1], 2);
        assert_eq!(m.msmap[4], 4, "J = 5: FSTEP 4 (index 4)");
        assert_eq!(m.msmap[198], 15);
        assert_eq!(m.msmap[199], 16);
        assert!(m.msmap.windows(2).all(|w| w[1] >= w[0]));
        // every step has 1000 integer probabilities: VERT1 plus its VERT2
        // entries; entries are table angles
        for is in 0..MSSTEPS {
            let n2 = m.vert2[is][..m.mxv2].iter().filter(|&&t| t > 0.0).count();
            assert_eq!(n2, m.mxv2, "step {is}");
            assert!(m.vert2[is][..m.mxv2].iter().all(|t| MOLIER_TH.contains(t)));
        }
        assert!(m.vert1[..m.mxv1].iter().all(|t| MOLIER_TH.contains(t)));
        assert!(m.vert1[..m.mxv1].windows(2).all(|w| w[1] >= w[0]), "angles in table order");
        // the data records: counts and formats
        let recs = |n: usize, per: usize| (n + per - 1) / per;
        let expect = 1 + 1 + recs(16, 4) + recs(200, 14) + recs(m.mxv1, 14) + 16 * recs(m.mxv2, 14);
        assert_eq!(out.deck.len(), expect);
        assert!(out.deck[2].starts_with("    1."), "{}", out.deck[2]);
        assert!(out.deck[2].len() == 1 + 4 * 16);
    }

    #[test]
    fn the_free_run_function_builds_the_program_from_text() {
        let d = synthetic_data();
        let (t8, t9) = data_tokens(&d);
        // a tiny aprime.data: NAPRZ NAPRE, the energies (MeV), then Z and the factors
        let aprime = "2 3\n1.0 2.0 3.0\n 6 1.0 1.1 1.2\n 13 1.0 1.0 1.0\n";
        let dens = density_file(2.26, 6, 1.0);
        let inp = Pegs4Inp { epstfl: Some(1), ..Default::default() };
        let blocks = [
            elem("C", "C", "C", inp),
            ener(0.521, 10.511, 0.01, 10.0),
            Pegs4Block::Pwlf(Pegs4Inp::default()),
            Pegs4Block::Deck(Pegs4Inp::default()),
        ];
        let out = run(&list_text(&t8), &list_text(&t9), Some(aprime), Some(&dens), &blocks).unwrap();
        assert!(out.deck[1].starts_with(" ELEM,RHO= 2.2600E+00,NE= 1, IUNRST=0, EPSTFL=1, IAPRIM=1"), "{}", out.deck[1]);
        // without the aprime text IAPRIM = 1 cannot be served; the error is reported
        let e = run(&list_text(&t8), &list_text(&t9), None, Some(&dens), &[
            elem("C", "C", "C", Pegs4Inp { epstfl: Some(1), ..Default::default() }),
            ener(0.521, 10.511, 0.01, 10.0),
            Pegs4Block::Pwlf(Pegs4Inp::default()),
        ])
        .unwrap_err();
        assert!(e.contains("aprime"), "{e}");
        assert!(run("1 2", "1 2", None, None, &[]).is_err());
    }

    #[test]
    fn a_second_medium_needs_the_density_variables_reset() {
        // pegs4.mortran:2711-2713 refuses SSB data given without ISSB, and
        // SPINIT itself leaves AFACT, SK, X0, X1, IEV, CBAR set (:2726-2729,
        // :2828), so the next ELEM/MIXT/COMP of the same run trips it (exit 21)
        // unless its &INP zeroes them. Pinned as upstream behaves.
        let mut p = Pegs4Program::new(synthetic_data());
        let e = p.run(&[carbon_elem(), elem("C", "C", "C", inp0())]).unwrap_err();
        assert!(e.contains("21") && e.contains("OVERRIDE"), "{e}");
        let mut p = Pegs4Program::new(synthetic_data());
        p.run(&[carbon_elem(), elem("C", "C", "C", inp_reset())]).unwrap();
        assert!(p.medset);
    }

    // ---- real data (env-gated) ----

    /// Runs the example inputs of `HEN_HOUSE/pegs4/inputs` on the real
    /// `pgs4pepr.dat`, `pgs4form.dat` and `aprime.data` when they exist
    /// (`PEGS4_DATA_DIR`, default `/tmp/claude-0/up/egsnrc/HEN_HOUSE/pegs4`)
    /// and prints the results; this is not a comparison with PEGS4 (no
    /// reference output exists for these inputs on this machine: the shipped
    /// `data/pb.pegs4dat` was made with other settings, `EPSTFL=1`, 200 photon
    /// intervals). The only checks are internal: the file round-trips
    /// through `parse_pegs4_media`, the first and third records agree with
    /// that shipped file's format, and the fit counts are within the array
    /// limits.
    ///
    /// Results (2026-11-21, release build, `f32`, the files at
    /// `PEGS4_DATA_DIR`): `pgs4pepr.dat` holds exactly the 20217 numbers the
    /// `READ(8,*)` of `:757` asks for (`100 + 2*6100 + 100 + 17 + 1700 + 6100`),
    /// so the array layout read here is the file's. **PB** (`p4ielem`
    /// settings, `IAPRIM = 1` from `aprime.data`): `RHO = 11.34`,
    /// `RLC = 0.561722 cm`, `NEL = 150` (the `$MXEKE` limit: the 1 % electron
    /// fit does not converge within 150 intervals over 1.5 MeV to 100 GeV),
    /// `NGL = 242`, `EBINDA = 0.088004 MeV` (the K edge of lead, 88.004 keV,
    /// `EKEDGE(82)`), 789 records. **H2O** (`COMP`, `IRAYL=1`, `AE=0.700`,
    /// `UE=50.511`, `AP=0.01`, `UP=50`): `RLC = 36.0863 cm`, `NEL = 150`,
    /// `NGL = 2000`, `NGR = 100` (all at their array limits), `EBINDA = 0`
    /// (the file's `EKEDGE` is 0 for Z = 1 and 8). Record 1 and the element
    /// record of the PB deck are character-for-character the shipped
    /// `data/pb.pegs4dat` records (a layout check; that file was made with
    /// other settings and is **not** a reference for the numbers).
    #[test]
    fn real_data_end_to_end_when_the_files_exist() {
        let dir = std::env::var("PEGS4_DATA_DIR").unwrap_or_else(|_| String::from("/tmp/claude-0/up/egsnrc/HEN_HOUSE/pegs4"));
        let read = |f: &str| std::fs::read_to_string(format!("{dir}/{f}")).ok();
        let (Some(x), Some(f), Some(a)) = (read("pgs4pepr.dat"), read("pgs4form.dat"), read("aprime.data")) else {
            eprintln!("PEGS4 data files not found in {dir}: real-data test skipped");
            return;
        };
        // p4ielem.pegs4inp: ELEM PB; ENER AE=1.5,UE=100000,AP=.1,UP=100000; PWLF; DECK
        let pb = Pegs4Block::Elem(MediumBlock::from_cards(Pegs4Inp::default(), &format!("{:<30}PB", "PB"), "PB"));
        let blocks = [pb, ener(1.5, 100000.0, 0.1, 100000.0), Pegs4Block::Pwlf(Pegs4Inp::default()), Pegs4Block::Deck(Pegs4Inp::default())];
        let out = run(&x, &f, Some(&a), None, &blocks).unwrap();
        let m = &parse_pegs4_media(&out.deck_text(), &["PB"]).unwrap()[0];
        eprintln!("PB: RHO={} RLC={:e} cm NEL={} NGL={} EBINDA={:e}", m.rho, m.rlc, m.sizes[3], m.sizes[1], m.ebinda);
        eprintln!("PB deck lines: {}", out.deck.len());
        assert_eq!(m.nne, 1);
        assert!(m.sizes[3] >= 3 && m.sizes[3] as usize <= MXEKE);
        assert!(m.sizes[1] >= 3 && m.sizes[1] as usize <= MXGE);
        if let Some(shipped) = read("data/pb.pegs4dat") {
            let sl: Vec<&str> = shipped.lines().collect();
            assert_eq!(out.deck[0].trim_end(), sl[0].trim_end(), "record 1 layout");
            assert_eq!(out.deck[2].trim_end(), sl[2].trim_end(), "record 3 layout and values");
        }
        // p4icomp.pegs4inp: COMP H2O with Rayleigh data (p4icall adds IRAYL=1)
        let water = Pegs4Block::Comp(MediumBlock::from_cards(
            Pegs4Inp { ne: Some(2), rho: Some(1.0), pz: Some(vec![2.0, 1.0]), irayl: Some(1), ..Default::default() },
            &format!("{:<30}H2O", "H2O"),
            "H  O",
        ));
        let blocks = [water, ener(0.700, 50.511, 0.01, 50.0), Pegs4Block::Pwlf(Pegs4Inp::default()), Pegs4Block::Deck(Pegs4Inp::default())];
        let out = run(&x, &f, Some(&a), None, &blocks).unwrap();
        let m = &parse_pegs4_media(&out.deck_text(), &["H2O"]).unwrap()[0];
        eprintln!(
            "H2O: RLC={:e} cm NEL={} NGL={} NGR={} EBINDA={:e} (IEV-derived SPC not printed)",
            m.rlc,
            m.sizes[3],
            m.sizes[1],
            m.rayleigh.as_ref().map_or(0, |r| r.ngr),
            m.ebinda
        );
        assert_eq!(m.irayl, 1);
        assert!(m.rayleigh.is_some());
    }
    /// Differential test against a REAL compiled PEGS4 (phase 2, gh:#1680).
    /// `PEGS4_REFERENCE_DIR` holds the `pb700`, `h2o700` and `tis521`
    /// `.pegs4dat` files written by the upstream program built from
    /// `pegs4.mortran` at the pinned commit (Mortran3 + gfortran, inputs
    /// `HEN_HOUSE/pegs4/inputs/{pb700,h2o700,tis521}.pegs4inp`, one run each);
    /// the data files come from `PEGS4_DATA_DIR`. Compares every number of
    /// every line and prints the worst differences; only structure (line
    /// count, token count per line, header lines identical) is asserted. The
    /// numbers are in `docs/egsnrc_phase2_differential.md`.
    #[test]
    fn differential_against_compiled_pegs4() {
        let Ok(refdir) = std::env::var("PEGS4_REFERENCE_DIR") else {
            eprintln!("PEGS4_REFERENCE_DIR not set: differential test skipped");
            return;
        };
        let dir = std::env::var("PEGS4_DATA_DIR").unwrap_or_else(|_| String::from("/tmp/claude-0/up/egsnrc/HEN_HOUSE/pegs4"));
        let read = |f: &str| std::fs::read_to_string(format!("{dir}/{f}")).unwrap();
        let (x, f, a) = (read("pgs4pepr.dat"), read("pgs4form.dat"), read("aprime.data"));
        let water = Pegs4Block::Comp(MediumBlock::from_cards(
            Pegs4Inp { ne: Some(2), rho: Some(1.0), pz: Some(vec![2.0, 1.0]), irayl: Some(1), ..Default::default() },
            &format!("{:<30}H2O", "H2O"),
            "H  O",
        ));
        let tissue = Pegs4Block::Mixt(MediumBlock::from_cards(
            Pegs4Inp { ne: Some(4), rho: Some(1.0), rhoz: Some(vec![0.101, 0.111, 0.026, 0.762]), irayl: Some(1), ..Default::default() },
            &format!("{:<30}ICRUTISSUE", "ICRUTISSUE"),
            "H  C  N  O",
        ));
        let lead = Pegs4Block::Elem(MediumBlock::from_cards(
            Pegs4Inp { irayl: Some(1), ..Default::default() },
            &format!("{:<30}PB", "PB"),
            "PB",
        ));
        let cases = [
            ("pb700", lead, ener(0.700, 50.511, 0.010, 50.0)),
            ("h2o700", water, ener(0.700, 50.511, 0.01, 50.0)),
            ("tis521", tissue, ener(0.521, 20.511, 0.010, 20.0)),
        ];
        let num = |l: &str| -> Vec<f64> {
            l.split(|c: char| c.is_whitespace() || c == ',').filter_map(|t| t.replace('D', "E").parse::<f64>().ok()).collect()
        };
        for (name, medium, energies) in cases {
            let refs = std::fs::read_to_string(format!("{refdir}/{name}.pegs4dat")).unwrap();
            let blocks = [medium, energies, Pegs4Block::Pwlf(Pegs4Inp::default()), Pegs4Block::Deck(Pegs4Inp::default())];
            let out = run(&x, &f, Some(&a), None, &blocks).unwrap();
            if let Ok(path) = std::env::var("PEGS4_PORT_OUT_DIR") {
                std::fs::write(format!("{path}/{name}.port.pegs4dat"), out.deck_text()).unwrap();
            }
            let mine = &out.deck;
            let theirs: Vec<&str> = refs.lines().collect();
            let (mut worst, mut worst_line, mut worst_abs, mut n_exact, mut n_diff, mut n_tok) = (0.0_f64, 0usize, 0.0_f64, 0usize, 0usize, 0usize);
            for (i, (m, r)) in mine.iter().zip(theirs.iter()).enumerate() {
                if m.trim_end() == r.trim_end() {
                    n_exact += 1;
                    continue;
                }
                n_diff += 1;
                if std::env::var("PEGS4_SHOW_DIFFS").is_ok() {
                    eprintln!("{name} line {}:\n  port: {}\n  ref : {}", i + 1, m.trim_end(), r.trim_end());
                }
                let (nm, nr) = (num(m), num(r));
                if nm.len() != nr.len() {
                    n_tok += 1;
                    eprintln!("{name} line {}: token count {} vs {}\n  port: {}\n  ref : {}", i + 1, nm.len(), nr.len(), m.trim_end(), r.trim_end());
                    continue;
                }
                let scale = nr.iter().fold(0.0_f64, |s, v| s.max(v.abs())).max(1e-300);
                for (a, b) in nm.iter().zip(&nr) {
                    let rel = (a - b).abs() / b.abs().max(1e-300);
                    if rel > worst {
                        worst = rel;
                        worst_line = i + 1;
                    }
                    worst_abs = worst_abs.max((a - b).abs() / scale);
                }
            }
            eprintln!(
                "{name}: lines port {} ref {}; identical {n_exact}, differing {n_diff} (token-count mismatches {n_tok}); worst relative {worst:e} at line {worst_line}; worst |d|/max|line| {worst_abs:e}",
                mine.len(),
                theirs.len()
            );
            assert_eq!(mine.len(), theirs.len(), "{name}: line count");
            assert_eq!(n_tok, 0, "{name}: token counts");
        }
    }

}
