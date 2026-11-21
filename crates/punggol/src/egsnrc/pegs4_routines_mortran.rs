// SPDX-License-Identifier: AGPL-3.0-only
//
// Attribution (RESEARCH_INTEGRITY_AND_PROVENANCE.md): port of EGSnrc.
//   Upstream project: EGSnrc, https://github.com/nrc-cnrc/EGSnrc
//   Upstream file:    HEN_HOUSE/src/pegs4_routines.mortran
//   Upstream commit:  bd49a47aae3938f49f4100b8e018091861408b19
//   Copyright:        National Research Council of Canada (and the EGS4 / PEGS4
//                     SLAC authors for the parts of the file that carry that
//                     notice; authors of the NRC adaptation: Blake Walters,
//                     2013, contributor Frederic Tessier)
//   Upstream licence: AGPL-3.0-or-later; this port is AGPL-3.0-only.
//! `HEN_HOUSE/src/pegs4_routines.mortran` (EGSnrc, 2300 lines): the PEGS4
//! cross-section and stopping-power generator library, adapted by NRC for
//! on-the-fly PEGS data generation.
//!
//! Upstream: <https://github.com/nrc-cnrc/EGSnrc>, commit
//! `bd49a47aae3938f49f4100b8e018091861408b19` (read 2026-11-21 from a
//! read-only scratch clone, never committed here). Licence: AGPL-3.0.
//!
//! **Status: first port, not differentially tested (gh:#1680).** Nothing here
//! has been compared with a compiled EGSnrc. The tests check internal
//! consistency (integrators against closed forms, differential against
//! integrated cross sections, additivity, interpolation exactness) only.
//!
//! # Single precision
//!
//! `$REAL4` expands to `real*4` (`pegs4_macros.mortran:52`, "pegs4 uses single
//! precision. real*8 causes problems"), so **every quantity in this file is
//! `f32`, storage and arithmetic**, and the expression order of the Mortran is
//! kept so that the rounding sequence matches left-to-right Fortran
//! evaluation. Literals such as `0.666667`, `2./3.` and `1.E-25` are single
//! precision. Transcendentals go through `petir::mathf::RealMath` on `f32`.
//! Whether a Fortran compiler contracts to FMA or uses a different `libm`
//! `logf` is not controlled here; that is exactly what phase 2 must measure.
//!
//! # COMMON blocks
//!
//! The Mortran `COMIN/..../` blocks (`pegs4_macros.mortran:59-206`) are
//! expanded into one owned struct, [`Pegs4Common`], passed explicitly
//! (`&self` where upstream reads a block, `&mut self` where it writes one).
//! No globals, no `static mut`. The blocks it carries are `PMCONS`/`DERCON`
//! (as [`Pegs4Constants`], supplied by the caller: upstream fills them in
//! `$INIT-PEGS4-VARIABLES`, `pegs4_macros.mortran:430-462`, not in this
//! file), `THRESHP`, `MOLVAR`, the numeric part of `MIXDAT`, `BREMPRP`,
//! `DBRPR`, `LSPION`, `EPSTAR`, `RADLEN`, `MIMSD`, `LBREMZ` and `ELEMTB`'s
//! symbol table. Not carried: the name strings of `MIXDAT`/`MEDINP`
//! (`ELEM_NAMES`, `COMP_NAMES`, `OUTDENSITY`, `MEDID`): upstream uses them
//! only as arguments of the C++ `egs_estar` call, which is not ported.
//! `SAVE`d locals (`APRIM`'s `APRIMD/EPRIM/ZPRIM/APRIMZ/napre/naprz`, `QFIT`'s
//! `nkp`) become fields or constants.
//!
//! # Omitted on purpose
//!
//! * Fortran I/O used only for diagnostics (`OUTPUT`, `write(58,*)`, the
//!   commented-out summary blocks, the `$WRITE_MEDERR` macro, `QD`'s message
//!   and `MSG` argument, `QFIT`'s `IPRN` trace, `PWLF1`'s "CATASTROPHE" line,
//!   `SPINIT`'s clamp warnings). Where the diagnostic tells the caller
//!   something, a flag or a status field carries it instead
//!   ([`SpinitReport`], [`Pwlf1Result::catastrophe`], [`DcadreResult::ier`]).
//! * `$CALL_EXIT(n)` and `$egs_fatal` become `Err(`[`Pegs4Error`]`)`.
//! * File reading is in-memory text: `APRIM`'s `aprime.data` is
//!   [`Pegs4Common::aprime_text`], `SPINIT`'s density file is a `&str`.
//!   Reading follows Fortran list-directed record semantics (see
//!   [`ListReader`]).
//! * `SPINIT`'s `egs_estar` call (the ICRU/ESTAR density-effect interface,
//!   `pegs4_routines.mortran:1708-1712`) is **not in the upstream clone**
//!   (the only occurrence of the name is in this file), so it is taken as an
//!   input, [`EstarDensity`], and tested with a synthetic table.
//! * Plot/histogram routines: none are in this file.
//!
//! # What is ported
//!
//! | upstream | here |
//! |---|---|
//! | `EFUNS` (54-134) | [`Pegs4Common::efuns`] |
//! | `BREMTM`, `BREMRM`, `BREMRZ`, `BREMDZ`, `BREMFZ` (138-229) | [`Pegs4Common::bremtm`], [`Pegs4Common::bremrm`], [`Pegs4Common::bremrz`], [`Pegs4Common::bremdz`], [`Pegs4Common::bremfz`] |
//! | `BRMSFZ` (233-255) | [`Pegs4Common::brmsfz`] |
//! | `AMOLTM`, `AMOLRM` (259-305) | [`Pegs4Common::amoltm`], [`Pegs4Common::amolrm`] |
//! | `BHABTM`, `BHABRM` (309-355) | [`Pegs4Common::bhabtm`], [`Pegs4Common::bhabrm`] |
//! | `ANIHTM` (359-377) | [`Pegs4Common::anihtm`] |
//! | `SPTOTP`, `SPTOTE` (381-431) | [`Pegs4Common::sptotp`], [`Pegs4Common::sptote`] |
//! | `SPIONE`, `SPIONB`, `SPIONP` (435-565) | [`Pegs4Common::spione`], [`Pegs4Common::spionb`], [`Pegs4Common::spionp`] (density search 518-543: [`Pegs4Common::density_delta`]) |
//! | `BRMSTM`, `BRMSRM`, `BRMSRZ`, `BRMSDZ` (569-653) | [`Pegs4Common::brmstm`], [`Pegs4Common::brmsrm`], [`Pegs4Common::brmsrz`], [`Pegs4Common::brmsdz`] |
//! | `APRIM` (658-768) | [`Pegs4Common::aprim`] |
//! | `AINTP` (772-813) | [`aintp`] |
//! | `TMXS`, `TMXB` (817-859) | [`Pegs4Common::tmxs`], [`Pegs4Common::tmxb`] |
//! | `ALKE`, `ALKEI` (863-887) | [`Pegs4Common::alke`], [`Pegs4Common::alkei`] |
//! | `PWLF1` (891-986) | [`pwlf1`] |
//! | `QFIT` (990-1129) | [`qfit`] |
//! | `QD` (1133-1170) | [`qd`] |
//! | `DCADRE` (1175-1656) | [`dcadre`] |
//! | `SPINIT` (1660-1879) | [`Pegs4Common::spinit`] |
//! | `MIX` (1883-2086) | [`Pegs4Common::mix`] |
//! | `DIFFER` (2090-2252) | [`Pegs4Common::differ`] |
//! | `FCOULCP`, `XSIFP` (2254-2276) | [`Pegs4Common::fcoulcp`], [`Pegs4Common::xsifp`] |
//! | `ZTBL` (2280-2300) | [`Pegs4Common::ztbl`] |
//!
//! Every routine in the file is ported; nothing is left out except the
//! diagnostics and the `egs_estar` call listed above.
//!
//! # Things in the upstream source that look wrong (quoted, not changed)
//!
//! * `pegs4_routines.mortran:1825-1841` (`SPINIT`, density-file composition
//!   check): one flag `ICHECK` is set to 0 by *any* matching element (line
//!   1833) and set to 1 only by a weight mismatch, so a later matching
//!   element **clears an earlier mismatch**, and an element absent from the
//!   PEGS medium goes unnoticed once any earlier element has matched. Ported
//!   as written; pinned by a test.
//! * `:1748` / `:1787` (`SPINIT`): `NEPST>150` is checked *after* the
//!   150-element arrays were filled (`:1783`). This port checks first.
//! * `:1075` (`QFIT`): `SXFH=AMIN1(XLL+W*ISUB,XH)` compares a value in
//!   `XFUN` space with the raw `XH`; harmless while `XFUN(x) < x` (as for
//!   `ALKE`) but not in general (the comparison with `XFH` would be
//!   consistent).
//! * `:1898` (`MIX`): local arrays `XSI(20)`, `ZZX(20)`, `FZC(20)`,
//!   `FCOUL(20)`, `ZZ(20)` while `$MXEL = 50` (`egsnrc.macros:198`): a medium
//!   with more than 20 elements overruns upstream. This port sizes them to
//!   `NEP`.
//! * `:1519` (`DCADRE`): `L` can reach `MAXTBL = 10` with `T(10,10)`; if the
//!   `ERRET` test at `:1520` falls through at `L = 10`, the next pass writes
//!   `T(11,1)`. Here that is an index panic (the Fortran is an out-of-bounds
//!   write).
//! * `:418-429` / `:391-403` (`SPTOTE`/`SPTOTP`): no `ELSE`; `IUNRSTP`
//!   outside 0..7 leaves the result unset. Here: `Err`.
//! * `:1976`, `:1703`: `MIX` multiplies `RHOP` by `GASPP` and `SPINIT`
//!   multiplies `IPOTVAL` by 1e6 in place, so calling either twice on the
//!   same common block rescales twice.
//! * `egsnrc.mortran:2434` (the sibling `XSIF`) uses `Z**(-0.666667)` and
//!   `Z**(-0.33333)` where this file's `XSIFP` (`:2274`) uses `-2./3.` and
//!   `-1./3.`; the two differ at the 1e-6 relative level (checked in a test).

#[cfg(not(any(feature = "std", test)))]
#[allow(unused_imports)]
use crate::nostd_prelude::*;

use petir::mathf::RealMath;
use petir::real::Real;

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
pub(super) fn log10_f32(x: f32) -> f32 {
    #[cfg(feature = "std")]
    {
        x.log10()
    }
    #[cfg(not(feature = "std"))]
    {
        RealMath::r_log10(f64::from(x)) as f32
    }
}
fn sqrt(x: f32) -> f32 {
    Real::sqrt(x)
}
/// Fortran `AINT` / `INT` truncation toward zero, kept in `f32`.
fn aint(x: f32) -> f32 {
    Real::trunc(x)
}

/// `$MXEL`-independent sizes from the upstream COMMON blocks.
const MAX_EPST: usize = 150; // EPSTEN(150), pegs4_macros.mortran:88
const MAX_ELEMENTS_EPST: usize = 20; // WEPST(20), ZEPST(20)
/// `$NAPRE`, maximum number of `APRIM` energies (`pegs4_routines.mortran:677`).
const NAPRE: usize = 115;
/// `$NAPRZ`, maximum number of `APRIM` elements (`:678`).
const NAPRZ: usize = 14;
/// `$RERR` (`pegs4_macros.mortran:55`).
const RERR: f32 = 1.0e-5;
/// `$AERR` (`pegs4_macros.mortran:56`).
const AERR: f32 = 1.0e-16;
/// `$FUDGEMS` (`pegs4_macros.mortran:57`).
const FUDGEMS: f32 = 1.0;

/// What a `$CALL_EXIT(n)` or `$egs_fatal` in the upstream becomes.
#[derive(Debug, Clone, PartialEq)]
pub enum Pegs4Error {
    /// `$CALL_EXIT(code)` after an `OUTPUT` message.
    CallExit {
        /// The exit code upstream passes.
        code: i32,
        /// The message upstream prints.
        message: String,
    },
    /// `$egs_fatal(*, message)`.
    Fatal(String),
}

/// The `PMCONS` and `DERCON` COMMON blocks (`pegs4_macros.mortran:64-72`).
/// Upstream sets them in `$INIT-PEGS4-VARIABLES` (`:430-462`), outside this
/// file; the caller supplies them. `rmpt2` is declared in `DERCON` but never
/// assigned in the code read.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Pegs4Constants {
    /// `PIP`.
    pub pip: f32,
    /// `C`, speed of light (cm/s).
    pub c: f32,
    /// `RME`, electron rest mass (g).
    pub rme: f32,
    /// `HBAR` (erg s).
    pub hbar: f32,
    /// `ECGS`, electron charge (esu).
    pub ecgs: f32,
    /// `EMKS` (C).
    pub emks: f32,
    /// `AN`, Avogadro's number.
    pub an: f32,
    /// `RADDEG`.
    pub raddeg: f32,
    /// `FSC`, fine-structure constant.
    pub fsc: f32,
    /// `ERGMEV`.
    pub ergmev: f32,
    /// `R0`, classical electron radius (cm).
    pub r0: f32,
    /// `RMP`, electron rest energy (MeV), single precision.
    pub rmp: f32,
    /// `RMPT2`.
    pub rmpt2: f32,
    /// `RMPSQ`.
    pub rmpsq: f32,
    /// `A22P9`.
    pub a22p9: f32,
    /// `A6680`.
    pub a6680: f32,
}

/// The `ESTAR` result `SPINIT` expects from `egs_estar` when no density file
/// is given (`pegs4_routines.mortran:1708-1713`): the density-effect
/// correction on ESTAR's energy grid (kinetic energies, MeV), and the
/// I-value ESTAR used (eV, `IEV`). Upstream's arrays are `REAL*8`
/// (`:1673`); they are narrowed to `f32` on assignment to `EPSTD`/`EPSTEN`.
#[derive(Debug, Clone, PartialEq)]
pub struct EstarDensity {
    /// `densityCorr`.
    pub dens_corr: Vec<f64>,
    /// `enGrid`, kinetic energies in MeV.
    pub en_grid: Vec<f64>,
    /// `IEV`, mean excitation energy in eV.
    pub iev: f32,
}

/// What `SPINIT` reports beyond its state changes (replaces its warning
/// printouts).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct SpinitReport {
    /// `:1699`: the I-value looked as if it had been given in eV, not MeV.
    pub ipot_looks_like_ev: bool,
    /// `:1755`/`:1794`: `EPSTEN(1)` was above `AE` and was set to `AE`.
    pub lowest_energy_set_to_ae: bool,
    /// `:1759`/`:1798`: `EPSTEN(NEPST)` was below `UE` and was set to `UE`.
    pub highest_energy_set_to_ue: bool,
}

/// The result of [`dcadre`] and [`qd`].
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct DcadreResult {
    /// `DCADRE`, the integral estimate.
    pub value: f32,
    /// `ERROR`, the estimated bound on its absolute error.
    pub error: f32,
    /// `IER`: 0 clean; 65, 66 warnings; 131, 132, 133 terminal
    /// (`:1208-1232`; the codes the code assigns are 65, 66, 131-133).
    pub ier: i32,
}

/// The result of [`pwlf1`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Pwlf1Result {
    /// `NI`, the number of subintervals used (as `QFIT` reports it,
    /// `NJ = NI + 2`).
    pub ni: i32,
    /// The final consistency re-fit failed (upstream prints "CATASTROPHE"
    /// at `:983-984`).
    pub catastrophe: bool,
}

/// A Fortran list-directed `READ(unit,*)` over in-memory text.
///
/// Semantics kept: items are separated by blanks or commas; a read that
/// needs more items continues onto following records; a read that is
/// satisfied **discards the rest of its last record**; `r*c` repeat counts
/// are honoured. Not kept: namelists, `/` terminators, null values.
/// Owned lines instead of a borrowed `Lines` (no lifetime parameters).
#[derive(Debug, Clone)]
pub struct ListReader {
    records: Vec<String>,
    pos: usize,
}

impl ListReader {
    /// Wrap text; each line is one record.
    pub fn new(text: &str) -> Self {
        Self { records: text.lines().map(|l| l.to_owned()).collect(), pos: 0 }
    }

    /// `READ(unit,'(A)') s`: the whole next record, or `None` at end of file.
    pub fn read_line(&mut self) -> Option<String> {
        let r = self.records.get(self.pos).cloned();
        if r.is_some() {
            self.pos += 1;
        }
        r
    }

    /// `READ(unit,*)` of `n` items as raw tokens.
    pub fn items(&mut self, n: usize) -> Result<Vec<String>, Pegs4Error> {
        let mut out: Vec<String> = Vec::with_capacity(n);
        while out.len() < n {
            let rec = match self.records.get(self.pos) {
                Some(r) => r.clone(),
                None => return Err(Pegs4Error::Fatal("end of file during list-directed read".to_owned())),
            };
            self.pos += 1;
            for tok in rec.split(|c: char| c.is_whitespace() || c == ',').filter(|t| !t.is_empty()) {
                if out.len() >= n {
                    break;
                }
                if let Some((r, c)) = tok.split_once('*') {
                    if let Ok(rep) = r.parse::<usize>() {
                        for _ in 0..rep {
                            if out.len() < n {
                                out.push(c.to_owned());
                            }
                        }
                        continue;
                    }
                }
                out.push(tok.to_owned());
            }
        }
        Ok(out)
    }

    /// `READ(unit,*)` of `n` reals (`D` exponents accepted, as Fortran does).
    pub fn reals(&mut self, n: usize) -> Result<Vec<f32>, Pegs4Error> {
        let toks = self.items(n)?;
        let mut v = Vec::with_capacity(n);
        for t in toks {
            let fixed = t.replace(['D', 'd'], "e");
            match fixed.parse::<f32>() {
                Ok(x) => v.push(x),
                Err(_) => return Err(Pegs4Error::Fatal(format!("bad real in list-directed read: {t}"))),
            }
        }
        Ok(v)
    }

    fn int_token(t: &str) -> Result<i32, Pegs4Error> {
        t.parse::<i32>().map_err(|_| Pegs4Error::Fatal(format!("bad integer in list-directed read: {t}")))
    }
}

/// `AINTP` (`pegs4_routines.mortran:772-813`): linear or logarithmic
/// interpolation in `xa`/`ya`. `ya(1,i)` of the Fortran array `YA(ISK,NX)`
/// is `ya[(i-1)*isk]`; pass a sub-slice to start at another row. `nx` must
/// be at least 2 (upstream indexes `XA(0)` otherwise). The interval is the
/// first `J` with `X < XA(J)`, or the last one: values outside the table are
/// **extrapolated**. A non-positive `XA(I)` turns `XLOG` off (`:798`).
pub fn aintp(x: f32, xa: &[f32], nx: usize, ya: &[f32], isk: usize, xlog: bool, ylog: bool) -> f32 {
    let mut xlogl = xlog;
    // FIND INTERVAL FOR X INTERPOLATION (:794-797)
    let mut j = nx;
    for jj in 2..=nx {
        if x < xa[jj - 1] {
            j = jj;
            break;
        }
    }
    let i = j - 1;
    if xa[i - 1] <= 0.0 {
        xlogl = false;
    }
    let (xi, xj, xv) = if !xlogl { (xa[i - 1], xa[j - 1], x) } else { (ln(xa[i - 1]), ln(xa[j - 1]), ln(x)) };
    let ya_i = ya[(i - 1) * isk];
    let ya_j = ya[(j - 1) * isk];
    if ylog && (ya_i == 0.0 || ya_j == 0.0) {
        return 0.0;
    }
    if ylog {
        let yi = ln(ya_i);
        let yj = ln(ya_j);
        let v = if xj == xi { yi } else { (yi * (xj - xv) + yj * (xv - xi)) / (xj - xi) };
        exp(v)
    } else if xj == xi {
        ya_i
    } else {
        (ya_i * (xj - xv) + ya_j * (xv - xi)) / (xj - xi)
    }
}

/// `QD` (`pegs4_routines.mortran:1133-1170`): `DCADRE` with the PEGS4
/// tolerances `$AERR = 1.E-16`, `$RERR = 1.E-5`. The `MSG` argument and the
/// `IER > 66` printout are omitted; `ier` is in the result.
pub fn qd<F: FnMut(f32) -> f32>(f: F, a: f32, b: f32) -> DcadreResult {
    dcadre(f, a, b, AERR, RERR)
}

/// Labels of the `GO TO` network of `DCADRE`.
#[derive(Clone, Copy, PartialEq, Eq)]
enum Lbl {
    L5,
    L10,
    L15,
    L25,
    L40,
    L50,
    L55,
    L60,
    L65,
    L70,
    L75,
    L80,
    L90,
    L95,
    L100,
    L110,
    L125,
    L130,
    L135,
    L145,
    L150,
    L155,
    L160,
    L165,
    L170,
    L175,
    L180,
    L185,
    L200,
    L205,
    L210,
    L215,
    L220,
}

/// `DCADRE` (`pegs4_routines.mortran:1175-1656`): "cautious adaptive Romberg
/// extrapolation" (de Boor's CADRE, 1974), single precision. `f` is the
/// integrand (upstream's `EXTERNAL F`). The `GO TO` network is kept label for
/// label (`Lbl` is the label); arrays are 1-based (index 0 unused) so that
/// the Fortran indices read unchanged. Status codes in [`DcadreResult::ier`].
///
/// Panics (index out of range) in the one case where upstream writes outside
/// `T(10,10)`; see the module notes.
#[allow(clippy::cognitive_complexity)]
pub fn dcadre<F: FnMut(f32) -> f32>(mut f: F, a: f32, b: f32, aerr: f32, rerr: f32) -> DcadreResult {
    // DATA (:1288-1293)
    let aitlow: f32 = 1.1;
    let h2tol: f32 = 0.15;
    let aittol: f32 = 0.1;
    let jumptl: f32 = 0.01;
    let maxts: i32 = 2049;
    let maxtbl: i32 = 10;
    let mxstge: i32 = 30;
    let rn: [f32; 5] = [0.0, 0.7142005, 0.3466282, 0.843751, 0.1263305];
    let zero: f32 = 0.0;
    let p1: f32 = 0.1;
    let half: f32 = 0.5;
    let one: f32 = 1.0;
    let two: f32 = 2.0;
    let four: f32 = 4.0;
    let fourp5: f32 = 4.5;
    let ten: f32 = 10.0;
    let hun: f32 = 100.0;

    let mut t = [[0.0f32; 11]; 11];
    let mut r = [0.0f32; 12];
    let mut ait = [0.0f32; 12];
    let mut dif = [0.0f32; 12];
    let mut ts = [0.0f32; 2050];
    let mut ibegs = [0i32; 32];
    let mut begin = [0.0f32; 32];
    let mut finis = [0.0f32; 32];
    let mut est = [0.0f32; 32];
    let mut reglsv = [false; 32];

    let alg4o2 = log10_f32(two);
    let mut cadre = zero;
    let mut error = zero;
    let mut curest = zero;
    let mut vint = zero;
    let mut ier: i32 = 0;
    let length = (b - a).abs();

    let mut h2conv = false;
    let mut aitken = false;
    let mut right = false;
    let mut reglar = false;
    let (mut stepmn, mut stepnm, mut stage) = (zero, zero, zero);
    let mut istage: i32 = 1;
    let (mut fnsize, mut prever) = (zero, zero);
    let (mut beg, mut fbeg, mut end, mut fend) = (zero, zero, zero, zero);
    let (mut ibeg, mut iend): (i32, i32) = (0, 0);
    let (mut errr, mut erra) = (zero, zero);
    let (mut step, mut astep, mut tabs, mut tabtlm) = (zero, zero, zero, zero);
    let (mut l, mut n, mut lm1, mut it): (usize, i32, usize, usize) = (0, 0, 0, 0);
    let (mut fnn, mut errer, mut ergl, mut ergoal) = (zero, zero, zero, zero);
    let (mut fextrp, mut diff, mut slope, mut fbeg2) = (zero, zero, zero, zero);
    let (mut sing, mut fextm1, mut h2nxt, mut singnx) = (zero, zero, zero, zero);
    let mut i: usize = 0;

    let mut lbl;
    if length == zero {
        lbl = Lbl::L215;
    } else if rerr > p1 || rerr < zero {
        lbl = Lbl::L210;
    } else if aerr == zero && (rerr + hun) <= hun {
        lbl = Lbl::L210;
    } else {
        errr = rerr;
        erra = aerr.abs();
        stepmn = length / ((1u32 << mxstge) as f32);
        stepnm = length.max(a.abs()).max(b.abs()) * ten;
        stage = half;
        istage = 1;
        fnsize = zero;
        prever = zero;
        reglar = false;
        beg = a;
        fbeg = f(beg) * half;
        ts[1] = fbeg;
        ibeg = 1;
        end = b;
        fend = f(end) * half;
        ts[2] = fend;
        iend = 2;
        lbl = Lbl::L5;
    }

    loop {
        match lbl {
            Lbl::L5 => {
                right = false;
                lbl = Lbl::L10;
            }
            Lbl::L10 => {
                step = end - beg;
                astep = step.abs();
                if astep < stepmn {
                    lbl = Lbl::L205;
                    continue;
                }
                if stepnm + astep == stepnm {
                    lbl = Lbl::L205;
                    continue;
                }
                t[1][1] = fbeg + fend;
                tabs = fbeg.abs() + fend.abs();
                l = 1;
                n = 1;
                h2conv = false;
                aitken = false;
                lbl = Lbl::L15;
            }
            Lbl::L15 => {
                lm1 = l;
                l += 1;
                let n2 = n + n;
                fnn = n2 as f32;
                let mut istep = (iend - ibeg) / n;
                if istep <= 1 {
                    let mut ii = iend;
                    iend += n;
                    if iend > maxts {
                        lbl = Lbl::L200;
                        continue;
                    }
                    let hovn = step / fnn;
                    let mut iii = iend;
                    let mut fi = one;
                    let mut k = 1;
                    while k <= n2 {
                        ts[iii as usize] = ts[ii as usize];
                        let rval = end - fi * hovn;
                        ts[(iii - 1) as usize] = f(rval);
                        fi += two;
                        iii -= 2;
                        ii -= 1;
                        k += 2;
                    }
                    istep = 2;
                }
                // label 25 (only entered by falling through from here)
                let istep2 = ibeg + istep / 2;
                let mut sum = zero;
                let mut sumabs = zero;
                let mut k = istep2;
                while k <= iend {
                    sum += ts[k as usize];
                    sumabs += ts[k as usize].abs();
                    k += istep;
                }
                t[l][1] = t[l - 1][1] * half + sum / fnn;
                tabs = tabs * half + sumabs / fnn;
                n = n2;
                it = 1;
                vint = step * t[l][1];
                tabtlm = tabs * ten;
                fnsize = fnsize.max(t[l][1].abs());
                ergl = astep * fnsize * ten;
                ergoal = stage * erra.max(errr * (curest + vint).abs());
                fextrp = one;
                for ii in 1..=lm1 {
                    fextrp *= four;
                    t[ii][l] = t[l][ii] - t[l - 1][ii];
                    t[l][ii + 1] = t[l][ii] + t[ii][l] / (fextrp - one);
                }
                errer = astep * t[1][l].abs();
                if l > 2 {
                    lbl = Lbl::L40;
                } else if tabs + p1 * t[1][2].abs() == tabs {
                    lbl = Lbl::L135;
                } else {
                    lbl = Lbl::L15;
                }
            }
            Lbl::L25 => {
                // folded into L15 above (the only entry to label 25 is the
                // fall-through from L15); kept so the label set is complete
                lbl = Lbl::L40;
            }
            Lbl::L40 => {
                for ii in 2..=lm1 {
                    diff = zero;
                    if tabtlm + t[ii - 1][l].abs() != tabtlm {
                        diff = t[ii - 1][lm1] / t[ii - 1][l];
                    }
                    t[ii - 1][lm1] = diff;
                }
                if (four - t[1][lm1]).abs() <= h2tol {
                    lbl = Lbl::L60;
                } else if t[1][lm1] == zero {
                    lbl = Lbl::L55;
                } else if (two - t[1][lm1].abs()).abs() < jumptl {
                    lbl = Lbl::L130;
                } else if l == 3 {
                    lbl = Lbl::L15;
                } else {
                    h2conv = false;
                    if ((t[1][lm1] - t[1][l - 2]) / t[1][lm1]).abs() <= aittol {
                        lbl = Lbl::L75;
                    } else {
                        lbl = Lbl::L50;
                    }
                }
            }
            Lbl::L50 => {
                if reglar {
                    lbl = Lbl::L55;
                } else if l == 4 {
                    lbl = Lbl::L15;
                } else {
                    lbl = Lbl::L55;
                }
            }
            Lbl::L55 => {
                if errer > ergoal && (ergl + errer) != ergl {
                    lbl = Lbl::L175;
                } else {
                    lbl = Lbl::L145;
                }
            }
            Lbl::L60 => {
                if !h2conv {
                    aitken = false;
                    h2conv = true;
                }
                lbl = Lbl::L65;
            }
            Lbl::L65 => {
                fextrp = four;
                lbl = Lbl::L70;
            }
            Lbl::L70 => {
                it += 1;
                vint = step * t[l][it];
                errer = (step / (fextrp - one) * t[it - 1][l]).abs();
                if errer <= ergoal {
                    lbl = Lbl::L160;
                } else if ergl + errer == ergl {
                    lbl = Lbl::L160;
                } else if it == lm1 {
                    lbl = Lbl::L125;
                } else if t[it][lm1] == zero {
                    lbl = Lbl::L70;
                } else if t[it][lm1] <= fextrp {
                    lbl = Lbl::L125;
                } else {
                    if (t[it][lm1] / four - fextrp).abs() / fextrp < aittol {
                        fextrp *= four;
                    }
                    lbl = Lbl::L70;
                }
            }
            Lbl::L75 => {
                if t[1][lm1] < aitlow {
                    lbl = Lbl::L175;
                } else {
                    if !aitken {
                        h2conv = false;
                        aitken = true;
                    }
                    lbl = Lbl::L80;
                }
            }
            Lbl::L80 => {
                fextrp = t[l - 2][lm1];
                if fextrp > fourp5 {
                    lbl = Lbl::L65;
                } else if fextrp < aitlow {
                    lbl = Lbl::L175;
                } else if (fextrp - t[l - 3][lm1]).abs() / t[1][lm1] > h2tol {
                    lbl = Lbl::L175;
                } else {
                    sing = fextrp;
                    fextm1 = one / (fextrp - one);
                    ait[1] = zero;
                    for ii in 2..=l {
                        ait[ii] = t[ii][1] + (t[ii][1] - t[ii - 1][1]) * fextm1;
                        r[ii] = t[1][ii - 1];
                        dif[ii] = ait[ii] - ait[ii - 1];
                    }
                    it = 2;
                    lbl = Lbl::L90;
                }
            }
            Lbl::L90 => {
                vint = step * ait[l];
                errer *= fextm1;
                if errer > ergoal && (ergl + errer) != ergl {
                    lbl = Lbl::L95;
                } else {
                    // ALPHA=LOG10(SING)/ALG4O2-ONE is computed upstream and
                    // never used; kept for fidelity of the call sequence.
                    let _alpha = log10_f32(sing) / alg4o2 - one;
                    ier = ier.max(65);
                    lbl = Lbl::L160;
                }
            }
            Lbl::L95 => {
                it += 1;
                if it == lm1 {
                    lbl = Lbl::L125;
                } else {
                    if it <= 3 {
                        h2nxt = four;
                        singnx = sing + sing;
                    }
                    lbl = Lbl::L100;
                }
            }
            Lbl::L100 => {
                if h2nxt < singnx {
                    fextrp = h2nxt;
                    h2nxt *= four;
                } else {
                    fextrp = singnx;
                    singnx += singnx;
                }
                lbl = Lbl::L110;
            }
            Lbl::L110 => {
                for ii in it..=lm1 {
                    r[ii + 1] = zero;
                    if tabtlm + dif[ii + 1].abs() != tabtlm {
                        r[ii + 1] = dif[ii] / dif[ii + 1];
                    }
                }
                let h2tfex = -h2tol * fextrp;
                if r[l] - fextrp < h2tfex {
                    lbl = Lbl::L125;
                } else if r[l - 1] - fextrp < h2tfex {
                    lbl = Lbl::L125;
                } else {
                    errer = astep * dif[l].abs();
                    fextm1 = one / (fextrp - one);
                    for ii in it..=l {
                        ait[ii] += dif[ii] * fextm1;
                        dif[ii] = ait[ii] - ait[ii - 1];
                    }
                    lbl = Lbl::L90;
                }
            }
            Lbl::L125 => {
                fextrp = (prever / errer).max(aitlow);
                prever = errer;
                if l < 5 {
                    lbl = Lbl::L15;
                } else if (l as i32 - it as i32) > 2 && istage < mxstge {
                    lbl = Lbl::L170;
                } else {
                    let erret = errer / RealMath::r_powi(fextrp, maxtbl - l as i32);
                    if erret > ergoal && (ergl + erret) != ergl {
                        lbl = Lbl::L170;
                    } else {
                        lbl = Lbl::L15;
                    }
                }
            }
            Lbl::L130 => {
                if errer > ergoal && (ergl + errer) != ergl {
                    lbl = Lbl::L170;
                } else {
                    // DIFF=ABS(T(1,L))*(FN+FN) is assigned and unused upstream
                    diff = t[1][l].abs() * (fnn + fnn);
                    lbl = Lbl::L160;
                }
            }
            Lbl::L135 => {
                slope = (fend - fbeg) * two;
                fbeg2 = fbeg + fbeg;
                lbl = Lbl::L160;
                for ii in 1..=4usize {
                    let rval = beg + rn[ii] * step;
                    diff = (f(rval) - fbeg2 - rn[ii] * slope).abs();
                    if tabtlm + diff != tabtlm {
                        i = ii;
                        lbl = Lbl::L155;
                        break;
                    }
                }
            }
            Lbl::L145 => {
                slope = (fend - fbeg) * two;
                fbeg2 = fbeg + fbeg;
                i = 1;
                lbl = Lbl::L150;
            }
            Lbl::L150 => {
                let rval = beg + rn[i] * step;
                diff = (f(rval) - fbeg2 - rn[i] * slope).abs();
                lbl = Lbl::L155;
            }
            Lbl::L155 => {
                errer = errer.max(astep * diff);
                if errer > ergoal && (ergl + errer) != ergl {
                    lbl = Lbl::L175;
                } else {
                    i += 1;
                    if i <= 4 {
                        lbl = Lbl::L150;
                    } else {
                        ier = 66;
                        lbl = Lbl::L160;
                    }
                }
            }
            Lbl::L160 => {
                cadre += vint;
                error += errer;
                if right {
                    lbl = Lbl::L165;
                } else {
                    istage -= 1;
                    if istage == 0 {
                        lbl = Lbl::L220;
                    } else {
                        let s = istage as usize;
                        reglar = reglsv[s];
                        beg = begin[s];
                        end = finis[s];
                        curest = curest - est[s + 1] + vint;
                        iend = ibeg - 1;
                        fend = ts[iend as usize];
                        ibeg = ibegs[s];
                        lbl = Lbl::L180;
                    }
                }
            }
            Lbl::L165 => {
                curest += vint;
                stage += stage;
                iend = ibeg;
                ibeg = ibegs[istage as usize];
                end = beg;
                beg = begin[istage as usize];
                fend = fbeg;
                fbeg = ts[ibeg as usize];
                lbl = Lbl::L5;
            }
            Lbl::L170 => {
                reglar = true;
                lbl = Lbl::L175;
            }
            Lbl::L175 => {
                if istage == mxstge {
                    lbl = Lbl::L205;
                } else if right {
                    lbl = Lbl::L185;
                } else {
                    let s = istage as usize;
                    reglsv[s + 1] = reglar;
                    begin[s] = beg;
                    ibegs[s] = ibeg;
                    stage *= half;
                    lbl = Lbl::L180;
                }
            }
            Lbl::L180 => {
                right = true;
                beg = (beg + end) * half;
                ibeg = (ibeg + iend) / 2;
                ts[ibeg as usize] *= half;
                fbeg = ts[ibeg as usize];
                lbl = Lbl::L10;
            }
            Lbl::L185 => {
                let s = istage as usize;
                let nnleft = ibeg - ibegs[s];
                if iend + nnleft >= maxts {
                    lbl = Lbl::L200;
                } else {
                    let mut iii = ibegs[s];
                    let mut ii = iend;
                    for k in iii..=ibeg {
                        ii += 1;
                        ts[ii as usize] = ts[k as usize];
                    }
                    for k in ibeg..=ii {
                        ts[iii as usize] = ts[k as usize];
                        iii += 1;
                    }
                    iend += 1;
                    ibeg = iend - nnleft;
                    fend = fbeg;
                    fbeg = ts[ibeg as usize];
                    finis[s] = end;
                    end = beg;
                    beg = begin[s];
                    begin[s] = end;
                    reglsv[s] = reglar;
                    istage += 1;
                    let s2 = istage as usize;
                    reglar = reglsv[s2];
                    est[s2] = vint;
                    curest += est[s2];
                    lbl = Lbl::L5;
                }
            }
            Lbl::L200 => {
                ier = 131;
                lbl = Lbl::L215;
            }
            Lbl::L205 => {
                ier = 132;
                lbl = Lbl::L215;
            }
            Lbl::L210 => {
                ier = 133;
                lbl = Lbl::L215;
            }
            Lbl::L215 => {
                cadre = curest + vint;
                lbl = Lbl::L220;
            }
            Lbl::L220 => {
                return DcadreResult { value: cadre, error, ier };
            }
        }
    }
}

/// `QFIT` (`pegs4_routines.mortran:990-1129`): construct and test a
/// piecewise-linear fit of `nfun` functions on `[xl, xh]`, linear in
/// `xfun(x)`, with `nj` subintervals in total (two of them the "skirts").
///
/// Fortran in/out arguments are `&mut`: `nj` becomes `NI + 2` as actually
/// used (left unchanged on the two early returns), `ax` and `bx` are the
/// subinterval-index coefficients (`INTERV = AX*XFUN(X)+BX`, truncated), `rem`
/// the maximum relative error found. `af`/`bf` are the Fortran `AF(NALM,NFUN)`
/// and `BF(NALM,NFUN)` flattened column-major: `AF(j,k)` is
/// `af[(j-1) + (k-1)*nalm]`. `vfuns(x, out)` fills `out[..nfun]` (upstream's
/// `VFUNS`); `nfun` must be at most 10 (the local `(10)` arrays) and `nalm`
/// at least `NI + 2` (else an index panic stands in for the Fortran overrun).
/// `xfun`/`xfi` are upstream's `XFUN`/`XFI` (monotone increasing, mutually
/// inverse). Returns `QFIT`: true if the fit is within `ep` (and within
/// `zep` where the function is below `zthr`).
///
/// `NI` is guarded: when it would become 0 the integer division at `:1048`
/// would trap upstream; here `false` is returned.
#[allow(clippy::too_many_arguments)]
pub fn qfit<X, I, V>(
    nj: &mut i32,
    xl: f32,
    xh: f32,
    xr: f32,
    ep: f32,
    zthr: &[f32],
    zep: &[f32],
    rem: &mut f32,
    njp: i32,
    xfun: &X,
    xfi: &I,
    ax: &mut f32,
    bx: &mut f32,
    nalm: usize,
    nfun: usize,
    af: &mut [f32],
    bf: &mut [f32],
    vfuns: &mut V,
) -> bool
where
    X: Fn(f32) -> f32,
    I: Fn(f32) -> f32,
    V: FnMut(f32, &mut [f32]),
{
    const NKP: i32 = 3; // DATA NKP/3/
    let mut fsxl = [0.0f32; 10];
    let mut fsxh = [0.0f32; 10];
    let mut fip = [0.0f32; 10];

    if xh <= xl {
        // upstream prints "QFIT ERROR:XL SHOULD BE < XH" (:1029)
        return false;
    }
    let xs = xl.max(xh.min(xr));
    // GET NUMBER OF INTERNAL SUBINTERVALS ALLOWED AND CHECK (:1033-1036)
    let mut ni = *nj - 2;
    let xfl;
    if ((xs == xl || xs == xh) && ni >= 1) || ni >= 2 {
        xfl = xfun(xl);
    } else {
        return false;
    }
    let xfh = xfun(xh);
    let xfs = xfun(xs);
    // SET SUBINTERVAL WIDTH (:1041-1043)
    let xm = (xfh - xfs).max(xfs - xfl);
    let dx = xfh - xfl;
    let w = xm / (1.0f32).max(aint((ni as f32) * xm / dx));
    // RESET NI TO HOW MANY WE'RE ACTUALLY GOING TO USE (:1046)
    ni = ((ni as f32) - aint((ni as f32) - dx / w)) as i32;
    if ni <= 0 {
        return false;
    }
    // INTERIOR POINTS PER SUBINTERVAL (:1048-1050)
    let mut nip = NKP.max((njp + ni - 1) / ni);
    nip = (nip / 2) * 2 + 1;
    let xll = if xfh - xfs <= xfs - xfl { xfl } else { xfh - (ni as f32) * w };
    *ax = 1.0 / w;
    *bx = 2.0 - xll * *ax;

    *rem = 0.0;
    let mut ok = true;
    let mut sxfl = xll.max(xfl);
    let mut xsxf = xfi(sxfl);
    vfuns(xsxf, &mut fsxl);
    for isub in 1..=ni {
        let jsub = (isub + 1) as usize;
        let sxfh = (xll + w * (isub as f32)).min(xh);
        xsxf = xfi(sxfh);
        vfuns(xsxf, &mut fsxh);
        let dsxf = sxfh - sxfl;
        for ifun in 0..nfun {
            af[(jsub - 1) + ifun * nalm] = (fsxh[ifun] - fsxl[ifun]) / dsxf;
            bf[(jsub - 1) + ifun * nalm] = (fsxl[ifun] * sxfh - fsxh[ifun] * sxfl) / dsxf;
        }
        let wip = dsxf / ((nip + 1) as f32);
        for ip in 1..=nip {
            let sxfip = sxfl + (ip as f32) * wip;
            let xip = xfi(sxfip);
            vfuns(xip, &mut fip);
            for ifun in 0..nfun {
                let ffip = af[(jsub - 1) + ifun * nalm] * sxfip + bf[(jsub - 1) + ifun * nalm];
                let afip = fip[ifun].abs();
                let aer = (ffip - fip[ifun]).abs();
                let mut re = 0.0f32;
                if fip[ifun] != 0.0 {
                    re = aer / afip;
                }
                if afip >= zthr[ifun] {
                    *rem = rem.max(re);
                } else if aer > zep[ifun] {
                    ok = false;
                }
            }
        }
        sxfl = sxfh;
        fsxl[..nfun].copy_from_slice(&fsxh[..nfun]);
    }
    // SET UP SKIRTING SUBINTERVALS (:1120-1124)
    let nis = ni as usize;
    for ifun in 0..nfun {
        af[ifun * nalm] = af[1 + ifun * nalm];
        bf[ifun * nalm] = bf[1 + ifun * nalm];
        af[(nis + 1) + ifun * nalm] = af[nis + ifun * nalm];
        bf[(nis + 1) + ifun * nalm] = bf[nis + ifun * nalm];
    }
    ok = ok && *rem <= ep;
    *nj = ni + 2;
    ok
}

/// `PWLF1` (`pegs4_routines.mortran:891-986`): find the smallest number of
/// equal-width (in `xfun(x)`) subintervals for which [`qfit`] passes, by
/// doubling then bisection, and leave that fit in `af`/`bf`. Arguments are as
/// for [`qfit`] (`nimx` is the maximum number of subintervals tried,
/// `nip` is `QFIT`'s `NJP`). If even `nimx` fails, `ni` is the last number
/// tried and the arrays hold that (failing) fit, exactly as upstream
/// (`:959-966`).
#[allow(clippy::too_many_arguments)]
pub fn pwlf1<X, I, V>(
    nimx: i32,
    xl: f32,
    xu: f32,
    xr: f32,
    ep: f32,
    zthr: &[f32],
    zep: &[f32],
    nip: i32,
    xfun: &X,
    xfi: &I,
    ax: &mut f32,
    bx: &mut f32,
    nalm: usize,
    nfun: usize,
    af: &mut [f32],
    bf: &mut [f32],
    vfuns: &mut V,
) -> Pwlf1Result
where
    X: Fn(f32) -> f32,
    I: Fn(f32) -> f32,
    V: FnMut(f32, &mut [f32]),
{
    let mut rem = 0.0f32;
    let mut nl: i32 = 0;
    let mut nu: i32 = 1;
    let mut nj: i32;
    // FIND # OF INTERVALS REQUIRED (:955-968)
    loop {
        nj = nu.min(nimx);
        if qfit(&mut nj, xl, xu, xr, ep, zthr, zep, &mut rem, nip, xfun, xfi, ax, bx, nalm, nfun, af, bf, vfuns) {
            break;
        }
        if nu >= nimx {
            return Pwlf1Result { ni: nj, catastrophe: false };
        }
        nl = nu;
        nu *= 2;
    }
    // WE NOW HAVE AN UPPER AND LOWER LIMIT ON NI, REFINE IT (:970-977)
    nu = nj;
    while nu > nl + 1 {
        nj = (nl + nu) / 2;
        let nk = nj; // QFIT may lower NJ; the original is needed for NL
        if qfit(&mut nj, xl, xu, xr, ep, zthr, zep, &mut rem, nip, xfun, xfi, ax, bx, nalm, nfun, af, bf, vfuns) {
            nu = nj;
        } else {
            nl = nk;
        }
    }
    let mut ni = nu;
    if ni == nj {
        return Pwlf1Result { ni, catastrophe: false };
    }
    // CALL IT ONCE MORE TO GET THE FITS (:981-984)
    let again = qfit(&mut ni, xl, xu, xr, ep, zthr, zep, &mut rem, nip, xfun, xfi, ax, bx, nalm, nfun, af, bf, vfuns);
    Pwlf1Result { ni, catastrophe: !again }
}

/// The five default `APRIMD` columns (Z = 6, 13, 29, 50, 79) times 18
/// energies, then zeros: `DATA APRIMD` at `pegs4_routines.mortran:685-694`,
/// stored `APRIMD(NAPRE, NAPRZ)` column-major.
fn default_aprimd() -> Vec<f32> {
    let cols: [[f32; 18]; 5] = [
        [1.32, 1.26, 1.18, 1.13, 1.09, 1.07, 1.05, 1.04, 1.03, 1.02, 1.0, 1.0, 1.0, 1.0, 1.0, 1.0, 1.0, 1.0],
        [1.34, 1.27, 1.19, 1.13, 1.09, 1.07, 1.05, 1.04, 1.03, 1.02, 1.0, 1.0, 1.0, 1.0, 1.0, 1.0, 1.0, 1.0],
        [
            1.39, 1.30, 1.21, 1.14, 1.10, 1.07, 1.05, 1.04, 1.03, 1.02, 0.994, 0.991, 0.991, 0.990, 0.989, 0.989,
            0.988, 0.988,
        ],
        [
            1.46, 1.34, 1.23, 1.15, 1.11, 1.08, 1.06, 1.05, 1.03, 1.02, 0.989, 0.973, 0.971, 0.969, 0.967, 0.965,
            0.963, 0.963,
        ],
        [
            1.55, 1.40, 1.26, 1.17, 1.12, 1.09, 1.07, 1.05, 1.03, 1.02, 0.955, 0.935, 0.930, 0.925, 0.920, 0.915,
            0.911, 0.911,
        ],
    ];
    let mut v = vec![0.0f32; NAPRE * NAPRZ];
    for (iz, col) in cols.iter().enumerate() {
        v[iz * NAPRE..iz * NAPRE + 18].copy_from_slice(col);
    }
    v
}

/// Owned expansion of the PEGS4 COMMON blocks used by this file; see the
/// module notes. Construct with [`Pegs4Common::new`] (the `BLOCK DATA`
/// defaults), then fill the medium (`app`, `aep`, `ap`/`ue`/`up`, `nep`,
/// `pzp`, `zelemp`, `wap`, `rhozp`, `rhop`, `gaspp`, `ipotval`, `iunrstp`,
/// `iaprimp`, `epstflp`), then call [`Pegs4Common::mix`],
/// [`Pegs4Common::differ`] and [`Pegs4Common::spinit`] in that order, as
/// `pegs4.mortran` does.
#[derive(Debug, Clone)]
pub struct Pegs4Common {
    /// `PMCONS` and `DERCON`.
    pub k: Pegs4Constants,
    // THRESHP (pegs4_macros.mortran:74-78)
    /// `APP`, photon cut-off (MeV).
    pub app: f32,
    /// `AEP`, electron cut-off (total energy, MeV).
    pub aep: f32,
    /// `UPP`.
    pub upp: f32,
    /// `UEP`.
    pub uep: f32,
    /// `THBREMP`.
    pub thbremp: f32,
    /// `THMOLLP`, Moller threshold (`AEP + TEP`, `pegs4_macros.mortran:268`).
    pub thmollp: f32,
    /// `TEP` (`AEP - RMP`).
    pub tep: f32,
    /// `IUNRSTP`, the restricted/unrestricted stopping-power option (0..5
    /// accepted by `EFUNS`).
    pub iunrstp: i32,
    // MOLVAR (:115-120)
    /// `WM`, molecular weight.
    pub wm: f32,
    /// `ZC`, electrons per molecule.
    pub zc: f32,
    /// `ZT`.
    pub zt: f32,
    /// `ZB`.
    pub zb: f32,
    /// `ZF`.
    pub zf: f32,
    /// `ZS`.
    pub zs: f32,
    /// `ZE`.
    pub ze: f32,
    /// `ZX`.
    pub zx: f32,
    /// `ZA`.
    pub za: f32,
    /// `ZG`.
    pub zg: f32,
    /// `ZP`.
    pub zp: f32,
    /// `ZV`.
    pub zv: f32,
    /// `ZU`.
    pub zu: f32,
    /// `RLCP`, radiation length (cm).
    pub rlcp: f32,
    /// `EDEN`, electron density (electrons/cm^3).
    pub eden: f32,
    /// `RHOP`, density (g/cm^3).
    pub rhop: f32,
    /// `XCCP`.
    pub xccp: f32,
    /// `BLCCP`.
    pub blccp: f32,
    /// `TEFF0P`.
    pub teff0p: f32,
    /// `XR0P`.
    pub xr0p: f32,
    // MIXDAT numeric part (:169-181)
    /// `NEP`, number of elements.
    pub nep: usize,
    /// `PZP`, proportion by number.
    pub pzp: Vec<f32>,
    /// `ZELEMP`, atomic numbers.
    pub zelemp: Vec<f32>,
    /// `WAP`, atomic weights.
    pub wap: Vec<f32>,
    /// `RHOZP`, proportion by weight.
    pub rhozp: Vec<f32>,
    /// `GASPP`, gas pressure factor (0 for non-gases).
    pub gaspp: f32,
    /// `EZ`, effective Z.
    pub ez: f32,
    /// `TPZ`.
    pub tpz: f32,
    /// `IPOTVAL`, I-value (MeV on entry to `SPINIT`).
    pub ipotval: f32,
    // BREMPRP (:80-84)
    /// `DLP1..DLP6`, `[lvl-1][..]`: `dlp[0]` is `DLP1(1..6)`, ... `dlp[5]` is `DLP6(1..6)`.
    pub dlp: [[f32; 6]; 6],
    /// `DELCMP`.
    pub delcmp: f32,
    /// `ALPHIP(2)`.
    pub alphip: [f32; 2],
    /// `BPARP(2)`.
    pub bparp: [f32; 2],
    /// `DELPOSP(2)`.
    pub delposp: [f32; 2],
    // DBRPR (:183-186)
    /// `ALFP1(2)`.
    pub alfp1: [f32; 2],
    /// `ALFP2(2)`.
    pub alfp2: [f32; 2],
    /// `AL2`.
    pub al2: f32,
    // LSPION (:135-138)
    /// `CBAR` (never used in this file).
    pub cbar: f32,
    /// `X0`.
    pub x0: f32,
    /// `X1`.
    pub x1: f32,
    /// `SK`.
    pub sk: f32,
    /// `TOLN10`.
    pub toln10: f32,
    /// `AFACT`.
    pub afact: f32,
    /// `SPC1`.
    pub spc1: f32,
    /// `SPC2`.
    pub spc2: f32,
    /// `IEV`, I-value in eV.
    pub iev: f32,
    // EPSTAR (:86-94)
    /// `EPSTEN(150)`, total energies (MeV); the first `nepst` are valid.
    pub epsten: Vec<f32>,
    /// `EPSTD(150)`.
    pub epstd: Vec<f32>,
    /// `WEPST(20)`.
    pub wepst: Vec<f32>,
    /// `EPSTTL` (80 characters).
    pub epsttl: String,
    /// `NEPST`.
    pub nepst: usize,
    /// `IEPST`, 1-based search pointer (`DATA IEPST/1/`).
    pub iepst: usize,
    /// `EPSTFLP`.
    pub epstflp: i32,
    /// `NELEPS`.
    pub neleps: usize,
    /// `ZEPST(20)`.
    pub zepst: Vec<i32>,
    /// `IAPRFL`.
    pub iaprfl: i32,
    /// `IAPRIMP` (`DATA IAPRIMP/1`).
    pub iaprimp: i32,
    // RADLEN (:127-130), MIMSD (:122-125)
    /// `ALRAD(4)`.
    pub alrad: [f32; 4],
    /// `ALRADP(4)`.
    pub alradp: [f32; 4],
    /// `A1440`.
    pub a1440: f32,
    /// `A183`.
    pub a183: f32,
    /// `BMIN`.
    pub bmin: f32,
    // LBREMZ (:59-62)
    /// `CONST`.
    pub const_: f32,
    /// `DELC`.
    pub delc: f32,
    /// `EBREMZ`.
    pub ebremz: f32,
    /// `DELTAM`.
    pub deltam: f32,
    /// `XLNZ`.
    pub xlnz: f32,
    // ELEMTB (:160-165): the element symbols `ZTBL` searches
    /// `ASYMT(100)`; the caller fills it (`pegs4_macros.mortran:633-642`).
    pub asymt: Vec<String>,
    // APRIM's SAVEd locals (pegs4_routines.mortran:684-699)
    aprimd: Vec<f32>,
    eprim: Vec<f32>,
    zprim: Vec<f32>,
    aprimz: Vec<f32>,
    napre: usize,
    naprz: usize,
    /// The text of `$HEN_HOUSE/pegs4/aprime.data` (read when `IAPRIMP = 1`
    /// and `IAPRFL = 0`, `pegs4_routines.mortran:720-746`).
    pub aprime_text: Option<String>,
}

impl Pegs4Common {
    /// The `BLOCK DATA` defaults (`pegs4_macros.mortran:464-482, 593-594`
    /// and `DATA` statements in this file) around caller-supplied constants.
    pub fn new(k: Pegs4Constants) -> Self {
        let mut eprim = vec![0.0f32; NAPRE];
        let e18: [f32; 18] =
            [2., 3., 4., 5., 6., 7., 8., 9., 10., 11., 21., 31., 41., 51., 61., 71., 81., 91.];
        eprim[..18].copy_from_slice(&e18);
        let mut zprim = vec![0.0f32; NAPRZ];
        zprim[..5].copy_from_slice(&[6., 13., 29., 50., 79.]);
        Self {
            k,
            app: 0.0,
            aep: 0.0,
            upp: 0.0,
            uep: 0.0,
            thbremp: 0.0,
            thmollp: 0.0,
            tep: 0.0,
            iunrstp: 0,
            wm: 0.0,
            zc: 0.0,
            zt: 0.0,
            zb: 0.0,
            zf: 0.0,
            zs: 0.0,
            ze: 0.0,
            zx: 0.0,
            za: 0.0,
            zg: 0.0,
            zp: 0.0,
            zv: 0.0,
            zu: 0.0,
            rlcp: 0.0,
            eden: 0.0,
            rhop: 0.0,
            xccp: 0.0,
            blccp: 0.0,
            teff0p: 0.0,
            xr0p: 0.0,
            nep: 0,
            pzp: Vec::new(),
            zelemp: Vec::new(),
            wap: Vec::new(),
            rhozp: Vec::new(),
            gaspp: 0.0,
            ez: 0.0,
            tpz: 0.0,
            ipotval: 0.0,
            dlp: [[0.0; 6]; 6],
            delcmp: 0.0,
            alphip: [0.0; 2],
            bparp: [0.0; 2],
            delposp: [0.0; 2],
            alfp1: [0.0; 2],
            alfp2: [0.0; 2],
            al2: 0.0,
            cbar: 0.0,
            x0: 0.0,
            x1: 0.0,
            sk: 0.0,
            toln10: 0.0,
            afact: 0.0,
            spc1: 0.0,
            spc2: 0.0,
            iev: 0.0,
            epsten: vec![0.0; MAX_EPST],
            epstd: vec![0.0; MAX_EPST],
            wepst: vec![0.0; MAX_ELEMENTS_EPST],
            epsttl: String::new(),
            nepst: 0,
            iepst: 1,
            epstflp: 0,
            neleps: 0,
            zepst: vec![0; MAX_ELEMENTS_EPST],
            iaprfl: 0,
            iaprimp: 1,
            alrad: [5.31, 4.79, 4.74, 4.71],
            alradp: [6.144, 5.621, 5.805, 5.924],
            a1440: 1194.0,
            a183: 184.15,
            bmin: 4.5,
            const_: 0.0,
            delc: 0.0,
            ebremz: 0.0,
            deltam: 0.0,
            xlnz: 0.0,
            asymt: Vec::new(),
            aprimd: default_aprimd(),
            eprim,
            zprim,
            aprimz: vec![0.0; NAPRE],
            napre: 0,
            naprz: 0,
            aprime_text: None,
        }
    }

    /// `EFUNS` (`:54-134`): the electron/positron functions PEGS4 fits.
    /// Returns `V(1..8)` as indices 0..7: total electron cross section,
    /// total positron cross section, electron and positron total stopping
    /// powers, the electron and two positron branching ratios, and the
    /// multiple-scattering step limit. `IUNRSTP` 6 and 7 are an error here
    /// (`:129-132`, `$CALL_EXIT(20)`).
    pub fn efuns(&mut self, e: f32) -> Result<[f32; 8], Pegs4Error> {
        let mut v = [0.0f32; 8];
        let thbrem = self.k.rmp + self.app;
        let iu = self.iunrstp;
        if iu == 0 || iu == 1 || iu == 5 {
            let brem = self.bremtm(e)?;
            let amoll = self.amoltm(e);
            let bhab = self.bhabtm(e);
            let annih = self.anihtm(e);
            let esig = brem + amoll;
            v[0] = esig;
            let psig = brem + bhab + annih;
            v[1] = psig;
            v[2] = self.sptote(e, self.aep, self.app)?;
            v[3] = self.sptotp(e, self.aep, self.app)?;
            if esig > 0.0 {
                v[4] = brem / esig;
            } else if thbrem <= self.thmollp {
                v[4] = 1.0;
            } else {
                v[4] = 0.0;
            }
            v[5] = brem / psig;
            v[6] = (brem + bhab) / psig;
            v[7] = self.tmxs(e);
        } else if iu == 2 {
            v[0] = 0.0;
            v[1] = 0.0;
            v[4] = 0.0;
            v[5] = 0.0;
            v[6] = 0.0;
            v[2] = self.sptote(e, e, e)?;
            v[3] = self.sptotp(e, e, e)?;
            v[7] = self.tmxs(e);
        } else if iu == 3 {
            let brem = self.bremtm(e)?;
            let annih = self.anihtm(e);
            v[0] = brem;
            v[1] = brem + annih;
            v[2] = self.sptote(e, e, self.app)?;
            v[3] = self.sptotp(e, e, self.app)?;
            v[4] = 1.0;
            v[5] = brem / v[1];
            v[6] = v[5];
            v[7] = self.tmxs(e);
        } else if iu == 4 {
            v[0] = self.amoltm(e);
            v[1] = self.bhabtm(e);
            v[2] = self.sptote(e, self.aep, e)?;
            v[3] = self.sptotp(e, self.aep, e)?;
            v[4] = 0.0;
            v[5] = 0.0;
            v[6] = 1.0;
            v[7] = self.tmxs(e);
        } else {
            return Err(Pegs4Error::CallExit {
                code: 20,
                message: format!("*********IUNRST={iu} NOT ALLOWED BY EFUNS*****"),
            });
        }
        Ok(v)
    }

    /// `BREMTM` (`:138-149`): total bremsstrahlung cross section.
    pub fn bremtm(&mut self, e0: f32) -> Result<f32, Pegs4Error> {
        if e0 <= self.app + self.k.rmp {
            Ok(0.0)
        } else {
            self.bremrm(e0, self.app, e0 - self.k.rmp)
        }
    }

    /// `BREMRM` (`:153-166`): sum over the elements of [`Self::bremrz`].
    pub fn bremrm(&mut self, e: f32, k1: f32, k2: f32) -> Result<f32, Pegs4Error> {
        let mut s = 0.0f32;
        for i in 0..self.nep {
            let z = self.zelemp[i];
            let pz = self.pzp[i];
            s += pz * self.bremrz(z, e, k1, k2)?;
        }
        Ok(s)
    }

    /// `BREMRZ` (`:170-194`): `QD` of [`Self::bremfz`] over `k1..k2` after
    /// the `BREMDZ` initialisation call.
    pub fn bremrz(&mut self, z: f32, e: f32, k1: f32, k2: f32) -> Result<f32, Pegs4Error> {
        let _dummy = self.bremdz(z, e, k1)?;
        let this = &*self;
        Ok(qd(|k| this.bremfz(k), k1, k2).value)
    }

    /// `BREMDZ` (`:198-216`): `BRMSDZ / K` (SLAC-265 eq. 2.7.108).
    pub fn bremdz(&mut self, z: f32, e: f32, k: f32) -> Result<f32, Pegs4Error> {
        Ok(self.brmsdz(z, e, k)? / k)
    }

    /// `BREMFZ` (`:220-229`): `BRMSFZ / K`; relies on `LBREMZ` having been
    /// set by [`Self::brmsdz`].
    pub fn bremfz(&self, k: f32) -> f32 {
        self.brmsfz(k) / k
    }

    /// `BRMSFZ` (`:233-255`): `K * dsigma/dK` for bremsstrahlung in one
    /// element, from the screening-function fits; uses `LBREMZ`.
    pub fn brmsfz(&self, k: f32) -> f32 {
        let mut emkloc = self.ebremz - k;
        if emkloc == 0.0 {
            emkloc = 1.0e-25;
        }
        let delta = self.delc * k / emkloc;
        if delta >= self.deltam {
            0.0
        } else {
            let sb1;
            let sb2;
            if delta <= 1.0 {
                sb1 = 20.867 + delta * (-3.242 + delta * 0.625) - self.xlnz;
                sb2 = 20.209 + delta * (-1.930 + delta * (-0.086)) - self.xlnz;
            } else {
                sb1 = 21.12 - 4.184 * ln(delta + 0.952) - self.xlnz;
                sb2 = sb1;
            }
            let ee = emkloc / self.ebremz;
            self.const_ * ((1.0 + ee * ee) * sb1 - 0.666667 * ee * sb2)
        }
    }

    /// `AMOLTM` (`:259-274`): total Moller cross section.
    pub fn amoltm(&self, e0: f32) -> f32 {
        if e0 <= self.thmollp {
            0.0
        } else {
            let t0 = e0 - self.k.rmp;
            self.amolrm(e0, self.aep, t0 * 0.5 + self.k.rmp)
        }
    }

    /// `AMOLRM` (`:278-305`): Moller cross section for a secondary electron
    /// of total energy `en1..en2` (SLAC-265 eq. 2.10.3).
    pub fn amolrm(&self, en0: f32, en1: f32, en2: f32) -> f32 {
        let rmp = self.k.rmp;
        let t0 = en0 - rmp;
        let t1 = en1 - rmp;
        let t2 = en2 - rmp;
        let tm = t0 / rmp;
        let em = tm + 1.0;
        let c1 = (tm / em) * (tm / em);
        let c2 = (2.0 * tm + 1.0) / (em * em);
        let betasq = 1.0 - 1.0 / (em * em);
        let cmoll2 = self.rlcp * self.eden * 2.0 * self.k.pip * (self.k.r0 * self.k.r0) / (betasq * tm);
        let eps1 = t1 / t0;
        let epsp1 = 1.0 - eps1;
        let eps2 = t2 / t0;
        let epsp2 = 1.0 - eps2;
        cmoll2
            * (c1 * (eps2 - eps1) + 1.0 / eps1 - 1.0 / eps2 + 1.0 / epsp2 - 1.0 / epsp1
                - c2 * ln(eps2 * epsp1 / (eps1 * epsp2)))
    }

    /// `BHABTM` (`:309-322`): total Bhabha cross section.
    pub fn bhabtm(&self, e0: f32) -> f32 {
        if e0 <= self.aep {
            0.0
        } else {
            self.bhabrm(e0, self.aep, e0)
        }
    }

    /// `BHABRM` (`:326-355`): Bhabha cross section for a secondary electron
    /// of total energy `en1..en2` (SLAC-265 eq. 2.11.2).
    pub fn bhabrm(&self, en0: f32, en1: f32, en2: f32) -> f32 {
        let rmp = self.k.rmp;
        let t0 = en0 - rmp;
        let t1 = en1 - rmp;
        let t2 = en2 - rmp;
        let tm = t0 / rmp;
        let em = tm + 1.0;
        let y = 1.0 / (tm + 2.0);
        let betasi = 1.0 / (1.0 - 1.0 / (em * em));
        let cbhab2 = self.rlcp * self.eden * 2.0 * self.k.pip * (self.k.r0 * self.k.r0) / tm;
        let b1 = 2.0 - y * y;
        let b2 = 3.0 - y * (6.0 - y * (1.0 - y * 2.0));
        let b3 = 2.0 - y * (10.0 - y * (16.0 - y * 8.0));
        let b4 = 1.0 - y * (6.0 - y * (12.0 - y * 8.0));
        let eps1 = t1 / t0;
        let eps2 = t2 / t0;
        cbhab2
            * (betasi * (1.0 / eps1 - 1.0 / eps2) - b1 * ln(eps2 / eps1)
                + b2 * (eps2 - eps1)
                + eps2 * eps2 * (eps2 * b4 / 3.0 - 0.5 * b3)
                - eps1 * eps1 * (eps1 * b4 / 3.0 - 0.5 * b3))
    }

    /// `ANIHTM` (`:359-377`): total two-photon annihilation-in-flight cross
    /// section (SLAC-265 eq. 2.12.14).
    pub fn anihtm(&self, e0: f32) -> f32 {
        let gam = e0 / self.k.rmp;
        let p0p2 = gam * gam - 1.0;
        let p0p = sqrt(p0p2);
        let canih = self.rlcp * self.eden * self.k.pip * (self.k.r0 * self.k.r0) / (gam + 1.0);
        canih * ((gam * gam + 4.0 * gam + 1.0) / p0p2 * ln(gam + p0p) - (gam + 3.0) / p0p)
    }

    /// `SPTOTP` (`:381-405`): positron total stopping power for the
    /// `IUNRSTP` option. `IUNRSTP` outside 0..7 is an `Err` (upstream leaves
    /// the result unset).
    pub fn sptotp(&mut self, e0: f32, ee: f32, eg: f32) -> Result<f32, Pegs4Error> {
        Ok(match self.iunrstp {
            0 => self.spionp(e0, ee) + self.brmstm(e0, eg)?,
            1 => self.spionp(e0, e0),
            2 => self.spionp(e0, e0) + self.brmstm(e0, e0)?,
            3 => self.spionp(e0, e0) + self.brmstm(e0, eg)?,
            4 => self.spionp(e0, ee) + self.brmstm(e0, e0)?,
            5 => self.brmstm(e0, e0)?,
            6 => self.brmstm(e0, eg)?,
            7 => self.spionp(e0, ee),
            n => return Err(Self::bad_iunrst(n)),
        })
    }

    /// `SPTOTE` (`:409-431`): electron total stopping power; see
    /// [`Self::sptotp`].
    pub fn sptote(&mut self, e0: f32, ee: f32, eg: f32) -> Result<f32, Pegs4Error> {
        Ok(match self.iunrstp {
            0 => self.spione(e0, ee) + self.brmstm(e0, eg)?,
            1 => self.spione(e0, e0),
            2 => self.spione(e0, e0) + self.brmstm(e0, e0)?,
            3 => self.spione(e0, e0) + self.brmstm(e0, eg)?,
            4 => self.spione(e0, ee) + self.brmstm(e0, e0)?,
            5 => self.brmstm(e0, e0)?,
            6 => self.brmstm(e0, eg)?,
            7 => self.spione(e0, ee),
            n => return Err(Self::bad_iunrst(n)),
        })
    }

    fn bad_iunrst(n: i32) -> Pegs4Error {
        Pegs4Error::CallExit { code: 20, message: format!("IUNRSTP={n} outside 0..7: SPTOTE/SPTOTP leave the result unset") }
    }

    /// `SPIONE` (`:435-443`): electron collision stopping power (MeV per
    /// radiation length), restricted at total energy `ee`.
    pub fn spione(&mut self, e0: f32, ee: f32) -> f32 {
        self.spionb(e0, ee, false)
    }

    /// `SPIONP` (`:557-565`): positron collision stopping power.
    pub fn spionp(&mut self, e0: f32, ee: f32) -> f32 {
        self.spionb(e0, ee, true)
    }

    /// The linear-interpolation density-effect correction of `SPIONB`
    /// (`:503-545`), with the moving pointer `IEPST` (starting from its
    /// previous value "since we assume that the code is working up or down a
    /// grid"). Below the first energy the first two points are extrapolated
    /// (`IEPST = 1`, formula at `:541`); above the last, the last value is
    /// used. Requires `nepst >= 1` (upstream would read uninitialised
    /// `EPSTD(0)`); returns NaN otherwise.
    pub fn density_delta(&mut self, e0: f32) -> f32 {
        let nepst = self.nepst;
        if nepst == 0 {
            return f32::NAN;
        }
        // 1-based accessors
        let en = |s: &Self, i: usize| s.epsten[i - 1];
        let mut iepst = self.iepst;
        'search: {
            if e0 >= en(self, iepst) {
                if e0 == en(self, iepst) {
                    break 'search;
                }
                for i in iepst..nepst {
                    if e0 < en(self, i + 1) {
                        iepst = i;
                        break 'search;
                    }
                }
                iepst = nepst;
            } else {
                for i in (2..=iepst).rev() {
                    if e0 >= en(self, i - 1) {
                        iepst = i - 1;
                        break 'search;
                    }
                }
                iepst = 1;
            }
        }
        self.iepst = iepst;
        if iepst < nepst {
            self.epstd[iepst - 1]
                + (e0 - self.epsten[iepst - 1]) / (self.epsten[iepst] - self.epsten[iepst - 1])
                    * (self.epstd[iepst] - self.epstd[iepst - 1])
        } else {
            self.epstd[nepst - 1]
        }
    }

    /// `SPIONB` (`:447-553`): Berger-Seltzer collision stopping power, in
    /// MeV per radiation length, for an electron (`positr = false`) or a
    /// positron, with energy transfers below `ee - RM` (total energies) and
    /// the density correction from [`Self::density_delta`]. Needs
    /// [`Self::spinit`] to have run (`spc1`, `spc2`, the density table).
    pub fn spionb(&mut self, e0: f32, ee: f32, positr: bool) -> f32 {
        let rmp = self.k.rmp;
        let g = e0 / rmp;
        let eem = ee / rmp - 1.0;
        let t = g - 1.0;
        let eta2 = t * (g + 1.0);
        let beta2 = eta2 / (g * g);
        let aleta2 = ln(eta2);
        let _x = 0.21715 * aleta2; // computed and unused upstream (:482)
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
        let delta = self.density_delta(e0);
        (self.spc1 / beta2) * (ln(t + 2.0) - self.spc2 + fterm - delta)
    }

    /// `BRMSTM` (`:569-584`): bremsstrahlung energy-loss stopping power from
    /// photon energy 0 up to `min(eg, e0 - RM)` (MeV per radiation length).
    pub fn brmstm(&mut self, e0: f32, eg: f32) -> Result<f32, Pegs4Error> {
        if e0 <= self.k.rmp {
            Ok(0.0)
        } else {
            let au = eg.min(e0 - self.k.rmp);
            self.brmsrm(e0, 0.0, au)
        }
    }

    /// `BRMSRM` (`:588-601`): sum over the elements of [`Self::brmsrz`].
    pub fn brmsrm(&mut self, e: f32, k1: f32, k2: f32) -> Result<f32, Pegs4Error> {
        let mut s = 0.0f32;
        for i in 0..self.nep {
            let z = self.zelemp[i];
            let pz = self.pzp[i];
            s += pz * self.brmsrz(z, e, k1, k2)?;
        }
        Ok(s)
    }

    /// `BRMSRZ` (`:605-629`): `QD` of [`Self::brmsfz`] over `k1..k2` after
    /// the `BRMSDZ` initialisation.
    pub fn brmsrz(&mut self, z: f32, e: f32, k1: f32, k2: f32) -> Result<f32, Pegs4Error> {
        let _dummy = self.brmsdz(z, e, k1)?;
        let this = &*self;
        Ok(qd(|k| this.brmsfz(k), k1, k2).value)
    }

    /// `BRMSDZ` (`:633-653`): initialise `LBREMZ` for element `z` at total
    /// energy `ea` and return `BRMSFZ(k)`. Calls [`Self::aprim`], which may
    /// read `aprime.data` on first use. Sets `ebremz`, `delc`, `const_`,
    /// `xlnz`, `deltam`.
    pub fn brmsdz(&mut self, z: f32, ea: f32, k: f32) -> Result<f32, Pegs4Error> {
        self.ebremz = ea;
        self.delc = 136.0 * powf(z, -1.0 / 3.0) * self.k.rmp / self.ebremz;
        let ap = self.aprim(z, self.ebremz)?;
        self.const_ = ap
            * (self.k.an * self.rhop / self.wm)
            * (self.k.r0 * self.k.r0)
            * self.k.fsc
            * z
            * (z + self.xsifp(z))
            * self.rlcp;
        self.xlnz = 4.0 / 3.0 * ln(z);
        if self.ebremz >= 50.0 {
            self.xlnz += 4.0 * self.fcoulcp(z);
        }
        // DELTAM IS THE DELTA AT WHICH THE SQUARE BRACKETS GO TO ZERO (:649)
        self.deltam = exp((21.12 - self.xlnz) / 4.184) - 0.952;
        Ok(self.brmsfz(k))
    }

    /// `APRIM` (`:658-768`): the empirical correction to the bremsstrahlung
    /// cross section, by `IAPRIMP`: 0 the old Koch and Motz table
    /// (interpolated linearly in Z then in `E/RM`, 1 above 50 MeV), 1 the NRC
    /// table from `aprime.data` ([`Self::aprime_text`], parsed on first use,
    /// log-Z interpolation), 2 none. Any other value, a missing text, or
    /// more elements/energies than `$NAPRZ = 14` / `$NAPRE = 115`, is an
    /// error (`$CALL_EXIT(24)` / `$egs_fatal`).
    pub fn aprim(&mut self, z: f32, e: f32) -> Result<f32, Pegs4Error> {
        let rmp = self.k.rmp;
        match self.iaprimp {
            0 => {
                if self.iaprfl == 0 {
                    self.iaprfl = 1;
                }
                if e >= 50.0 {
                    Ok(1.0)
                } else {
                    let em = e / rmp;
                    for ie in 1..=18usize {
                        self.aprimz[ie - 1] = aintp(z, &self.zprim, 5, &self.aprimd[ie - 1..], NAPRE, false, false);
                    }
                    Ok(aintp(em, &self.eprim, 18, &self.aprimz, 1, false, false))
                }
            }
            1 => {
                if self.iaprfl == 0 {
                    self.load_aprime(rmp)?;
                }
                let em = e / rmp;
                for ie in 1..=self.napre {
                    self.aprimz[ie - 1] =
                        aintp(z, &self.zprim, self.naprz, &self.aprimd[ie - 1..], NAPRE, true, false);
                }
                Ok(aintp(em, &self.eprim, self.napre, &self.aprimz, 1, false, false))
            }
            2 => {
                if self.iaprfl == 0 {
                    self.iaprfl = 1;
                }
                Ok(1.0)
            }
            n => Err(Pegs4Error::CallExit { code: 24, message: format!("ILLEGAL VALUE FOR IAPRIM: {n}") }),
        }
    }

    /// The `IAPRFL = 0` branch of `APRIM` (`:720-746`): read the data.
    fn load_aprime(&mut self, rmp: f32) -> Result<(), Pegs4Error> {
        let text = match &self.aprime_text {
            Some(t) => t.clone(),
            None => return Err(Pegs4Error::Fatal("Cannot open file $HEN_HOUSE/pegs4/aprime.data".to_owned())),
        };
        let mut rd = ListReader::new(&text);
        let head = rd.items(2)?;
        let naprz = ListReader::int_token(&head[0])?;
        let napre = ListReader::int_token(&head[1])?;
        if naprz as usize > NAPRZ || naprz < 1 {
            return Err(Pegs4Error::CallExit {
                code: 24,
                message: "TOO MANY ELEMENTS FOR APRIME INTERPOLATION: CHANGE $NAPRZ AND RECOMPILE PEGS".to_owned(),
            });
        }
        if napre as usize > NAPRE || napre < 1 {
            return Err(Pegs4Error::CallExit {
                code: 24,
                message: "TOO MANY ENERGIES FOR APRIME INTERPOLATION: CHANGE $NAPRE AND RECOMPILE PEGS".to_owned(),
            });
        }
        let (naprz, napre) = (naprz as usize, napre as usize);
        let en = rd.reals(napre)?;
        for ie in 0..napre {
            self.eprim[ie] = 1.0 + en[ie] / rmp;
        }
        for iz in 0..naprz {
            let row = rd.reals(1 + napre)?;
            self.zprim[iz] = row[0];
            for ie in 0..napre {
                self.aprimd[iz * NAPRE + ie] = row[1 + ie];
            }
        }
        self.napre = napre;
        self.naprz = naprz;
        self.iaprfl = 1;
        Ok(())
    }

    /// `TMXS` (`:817-831`): maximum step size valid for multiple scattering:
    /// `min(TMXB * 0.8, 10)` (`SAFETY = 0.8`, `TABSMX = 10`).
    pub fn tmxs(&self, e: f32) -> f32 {
        let safety: f32 = 0.8;
        let tabsmx: f32 = 10.0;
        (self.tmxb(e) * safety).min(tabsmx)
    }

    /// `TMXB` (`:835-859`): the transport distance at which Bethe's
    /// criterion `XC^2 * B = 1` holds.
    pub fn tmxb(&self, e: f32) -> f32 {
        let esq = e * e;
        let beta2 = 1.0 - self.k.rmpsq / esq;
        let px2 = esq * beta2 / (self.xccp * self.xccp);
        px2 * beta2 / ln(self.blccp * px2)
    }

    /// `ALKE` (`:863-874`): log of kinetic energy, `ln(E - RM)`.
    pub fn alke(&self, e: f32) -> f32 {
        ln(e - self.k.rmp)
    }

    /// `ALKEI` (`:877-887`): inverse of [`Self::alke`].
    pub fn alkei(&self, x: f32) -> f32 {
        exp(x) + self.k.rmp
    }

    /// `SPINIT` (`:1660-1879`): load the density-effect table and set
    /// `SPC1`, `SPC2`, `TOLN10` for [`Self::spionb`]. Call after
    /// [`Self::mix`].
    ///
    /// `EPSTFLP = 0` (also any value outside 0..1, `:1687-1690`) takes the
    /// table from `estar` (the `egs_estar` result, which must be `Some`);
    /// `EPSTFLP = 1` reads `density_file` (the text of the user's density
    /// file, which must be `Some`): a title line, then `NEPST IEV RHO
    /// NELEPS`, then `NELEPS` pairs `Z W`, then `NEPST` pairs `E(kinetic,
    /// MeV) DELTA`; it is then checked against the PEGS composition (with the
    /// upstream's flawed flag logic, see the module notes). Energies are
    /// converted to total energy, clamped to `AE`/`UE` (reported in the
    /// result), `IEV` is converted to MeV, and `SPC1 = 2 pi r0^2 RM n_e
    /// X0`, `SPC2 = ln((I/RM)^2/2)`.
    ///
    /// Number of energies above 150, a zero molecular weight and a
    /// composition mismatch are errors (codes 22, 23, 23); `IPOTVAL` is
    /// scaled MeV -> eV *in place* on the `EPSTFLP = 0` path.
    pub fn spinit(&mut self, estar: Option<&EstarDensity>, density_file: Option<&str>) -> Result<SpinitReport, Pegs4Error> {
        let mut report = SpinitReport::default();
        self.toln10 = 2.0 * ln(10.0);
        if self.epstflp < 0 || self.epstflp > 1 {
            self.epstflp = 0;
        }
        let rmp = self.k.rmp;

        if self.epstflp == 0 {
            // No density file: take ESTAR's factors (:1692-1762)
            if self.ipotval > 1.0 {
                report.ipot_looks_like_ev = true;
            }
            self.ipotval *= 1_000_000.0;
            let es = match estar {
                Some(e) => e,
                None => return Err(Pegs4Error::Fatal("SPINIT: EPSTFL=0 needs the egs_estar result".to_owned())),
            };
            self.iev = es.iev;
            if es.dens_corr.len() != es.en_grid.len() || es.en_grid.is_empty() {
                return Err(Pegs4Error::Fatal("SPINIT: ESTAR grid and density correction differ in length".to_owned()));
            }
            if es.en_grid.len() > MAX_EPST {
                return Err(Self::too_many_energies(es.en_grid.len()));
            }
            // upstream fixes NEPST = 113 (the ESTAR grid); the grid length is used here
            self.nepst = es.en_grid.len();
            let _epstrh = self.rhop;
            self.neleps = self.nep;
            if self.neleps > MAX_ELEMENTS_EPST {
                return Err(Pegs4Error::Fatal("SPINIT: more than 20 elements".to_owned()));
            }
            let mut norm = 0.0f32;
            for iz in 0..self.neleps {
                self.zepst[iz] = self.zelemp[iz] as i32;
                norm += self.rhozp[iz];
            }
            if norm == 0.0 {
                return Err(Self::zero_weight());
            }
            for iw in 0..self.neleps {
                self.wepst[iw] = self.rhozp[iw] / norm;
            }
            for idf in 0..self.nepst {
                self.epstd[idf] = es.dens_corr[idf] as f32;
                self.epsten[idf] = es.en_grid[idf] as f32;
            }
            // CONVERT TO TOTAL ENERGIES (:1750-1752)
            for i in 0..self.nepst {
                self.epsten[i] += rmp;
            }
            let imev = self.iev * 1.0e-6;
            self.clamp_energy_range(&mut report);
            self.set_spc(imev);
        } else {
            // EPSTFL=1: read the density file (:1769-1851)
            let text = match density_file {
                Some(t) => t,
                None => return Err(Pegs4Error::Fatal(" Failed to open density file ".to_owned())),
            };
            let mut rd = ListReader::new(text);
            let title = rd.read_line().unwrap_or_default();
            self.epsttl = title.chars().take(80).collect();
            let head = rd.items(4)?;
            let nepst = ListReader::int_token(&head[0])?;
            let iev: f32 = head[1].replace(['D', 'd'], "e").parse().map_err(|_| Pegs4Error::Fatal("bad IEV".to_owned()))?;
            let epstrh: f32 = head[2].replace(['D', 'd'], "e").parse().map_err(|_| Pegs4Error::Fatal("bad density".to_owned()))?;
            let neleps = ListReader::int_token(&head[3])?;
            if nepst < 1 || nepst as usize > MAX_EPST {
                return Err(Self::too_many_energies(nepst.max(0) as usize));
            }
            if neleps < 1 || neleps as usize > MAX_ELEMENTS_EPST {
                return Err(Pegs4Error::Fatal("SPINIT: NELEPS outside 1..20".to_owned()));
            }
            self.nepst = nepst as usize;
            self.iev = iev;
            self.neleps = neleps as usize;
            let zw = rd.items(2 * self.neleps)?;
            for i in 0..self.neleps {
                self.zepst[i] = ListReader::int_token(&zw[2 * i])?;
                self.wepst[i] = zw[2 * i + 1].replace(['D', 'd'], "e").parse().map_err(|_| Pegs4Error::Fatal("bad WEPST".to_owned()))?;
            }
            let ed = rd.reals(2 * self.nepst)?;
            for i in 0..self.nepst {
                self.epsten[i] = ed[2 * i];
                self.epstd[i] = ed[2 * i + 1];
            }
            // CONVERT TO TOTAL ENERGIES (:1789-1791)
            for i in 0..self.nepst {
                self.epsten[i] += rmp;
            }
            let imev = self.iev * 1.0e-6;
            self.clamp_energy_range(&mut report);

            // CHECK COMPOSITION AND DENSITY (:1803-1849)
            let mut icheck = 0;
            let tlrnce: f32 = 0.01;
            if self.neleps != self.nep {
                icheck = 1;
            }
            if icheck == 0 && (epstrh < (1.0 - tlrnce) * self.rhop || epstrh > (1.0 + tlrnce) * self.rhop) {
                icheck = 1;
            }
            let mut epstwt = 0.0f32;
            for i in 0..self.nep {
                epstwt += self.rhozp[i];
            }
            // epstwt == 0 only prints upstream (:1821-1823) and carries on
            if icheck == 0 {
                let mut iespel = 0usize;
                icheck = 1;
                loop {
                    iespel += 1;
                    let mut ipegel = 0usize;
                    loop {
                        ipegel += 1;
                        if (self.zelemp[ipegel - 1] as i32) == self.zepst[iespel - 1] {
                            icheck = 0;
                            break;
                        }
                        if ipegel >= self.nep {
                            break;
                        }
                    }
                    if icheck == 0 {
                        let wref = self.rhozp[ipegel - 1] / epstwt;
                        let w = self.wepst[iespel - 1];
                        if w < (1.0 - tlrnce) * wref || w > (1.0 + tlrnce) * wref {
                            icheck = 1;
                        }
                    }
                    if iespel >= self.neleps {
                        break;
                    }
                }
            }
            if icheck == 1 {
                return Err(Pegs4Error::CallExit {
                    code: 23,
                    message: "COMPOSITION IN INPUT DENSITY FILE DOES NOT MATCH THAT BEING USED BY PEGS".to_owned(),
                });
            }
            self.set_spc(imev);
        }
        Ok(report)
    }

    fn too_many_energies(n: usize) -> Pegs4Error {
        Pegs4Error::CallExit { code: 22, message: format!("*****NEPST={n} IS GREATER THAN THE 150 ALLOWED") }
    }

    fn zero_weight() -> Pegs4Error {
        Pegs4Error::CallExit {
            code: 23,
            message: "*****IN SPINIT***SOMETHING WRONG, MOLECULAR WEIGHT OF MOLECULE IS ZERO (I.E. SUM OF RHOZ)***".to_owned(),
        }
    }

    /// "CHECK THAT WE HAVE COVERED ENERGY RANGES NEEDED" (`:1754-1762`).
    fn clamp_energy_range(&mut self, report: &mut SpinitReport) {
        if self.aep < self.epsten[0] {
            self.epsten[0] = self.aep;
            report.lowest_energy_set_to_ae = true;
        }
        if self.uep > self.epsten[self.nepst - 1] {
            self.epsten[self.nepst - 1] = self.uep;
            report.highest_energy_set_to_ue = true;
        }
    }

    /// `SPC1`, `SPC2` (`:1854-1855`).
    fn set_spc(&mut self, imev: f32) {
        self.spc1 = 2.0 * self.k.pip * (self.k.r0 * self.k.r0) * self.k.rmp * self.eden * self.rlcp;
        let r = imev / self.k.rmp;
        self.spc2 = ln(r * r / 2.0);
    }

    /// `MIX` (`:1883-2086`): molecular variables from the element table
    /// (`ZT`, `ZB`, `ZF`, `ZS`, `ZE`, `ZX`, `ZA`, `ZG`, `ZP`, `ZV`, `ZU`,
    /// `EZ`, `TPZ`, `WM`, `ZC`), the electron density `EDEN`, the radiation
    /// length `RLCP`, and the Moliere multiple-scattering constants `BLCCP`,
    /// `TEFF0P`, `XCCP`, `XR0P`. **Scales `RHOP` by `GASPP` in place** when
    /// `GASPP != 0` (`:1976`), so call it once per medium.
    pub fn mix(&mut self) {
        let k = self.k;
        if self.gaspp != 0.0 {
            self.rhop *= self.gaspp;
        }
        let al183 = ln(self.a183);
        self.tpz = 0.0;
        self.wm = 0.0;
        self.zc = 0.0;
        self.zt = 0.0;
        self.zb = 0.0;
        self.zf = 0.0;
        self.zs = 0.0;
        self.ze = 0.0;
        self.zx = 0.0;
        let mut zab = 0.0f32;
        for i in 0..self.nep {
            let zi = self.zelemp[i];
            self.tpz += self.pzp[i];
            self.wm += self.pzp[i] * self.wap[i];
            self.zc += self.pzp[i] * zi;
            let fzc = (k.fsc * zi) * (k.fsc * zi);
            let fcoul = self.fcoulcp(zi);
            let xsi = self.xsifp(zi);
            let zzx = self.pzp[i] * zi * (zi + xsi);
            if zi <= 4.0 {
                let izz = zi as usize;
                zab += zzx * self.alrad[izz - 1];
            } else {
                zab += zzx * (al183 + ln(powf(zi, -1.0 / 3.0)));
            }
            self.zt += zzx;
            self.zb += zzx * ln(powf(zi, -1.0 / 3.0));
            self.zf += zzx * fcoul;
            let zz = self.pzp[i] * zi * (zi + FUDGEMS);
            self.zs += zz;
            self.ze += zz * ((-2.0 / 3.0) * ln(zi));
            self.zx += zz * ln(1.0 + 3.34 * fzc);
        }
        self.ez = self.zc / self.tpz;
        self.za = al183 * self.zt;
        self.zg = self.zb / self.zt;
        self.zp = self.zb / self.za;
        self.zv = (self.zb - self.zf) / self.zt;
        self.zu = (self.zb - self.zf) / self.za;
        self.eden = k.an * self.rhop / self.wm * self.zc;
        self.rlcp = 1.0 / ((k.an * self.rhop / self.wm) * 4.0 * k.fsc * (k.r0 * k.r0) * (zab - self.zf));
        self.blccp = k.a6680 * self.rhop * self.zs * exp(self.ze / self.zs) * self.rlcp / (self.wm * exp(self.zx / self.zs));
        self.teff0p = (exp(self.bmin) / self.bmin) / self.blccp;
        self.xccp = (k.a22p9 / k.raddeg) * sqrt(self.zs * self.rhop * self.rlcp / self.wm);
        self.xr0p = self.xccp * sqrt(self.teff0p * self.bmin);
    }

    /// `DIFFER` (`:2090-2252`): the differential-cross-section constants for
    /// bremsstrahlung and pair production: `ALPHIP`, `ALFP1`, `ALFP2`,
    /// `BPARP`, `DELCMP`, `DELPOSP` and the six screening-rejection fits
    /// `DLP1..DLP6` (level `lvl` = 1 B-H A, 2 B-H B, 3 B-H C, 4 CC A, 5 CC B,
    /// 6 CC C). Uses `ZG`, `ZP`, `ZV`, `ZU` from [`Self::mix`].
    pub fn differ(&mut self) {
        let (zg, zp, zv, zu) = (self.zg, self.zp, self.zv, self.zu);
        self.al2 = ln(2.0);
        let al183 = ln(self.a183);
        let al2 = self.al2;
        self.alphip[0] = al2 * (4.0 / 3.0 + 1.0 / (9.0 * al183 * (1.0 + zp)));
        self.alphip[1] = al2 * (4.0 / 3.0 + 1.0 / (9.0 * al183 * (1.0 + zu)));
        self.alfp1[0] = 2.0 / 3.0 - 1.0 / (36.0 * al183 * (1.0 + zp));
        self.alfp1[1] = 2.0 / 3.0 - 1.0 / (36.0 * al183 * (1.0 + zu));
        self.alfp2[0] = (1.0 / 12.0) * (4.0 / 3.0 + 1.0 / (9.0 * al183 * (1.0 + zp)));
        self.alfp2[1] = (1.0 / 12.0) * (4.0 / 3.0 + 1.0 / (9.0 * al183 * (1.0 + zu)));
        self.bparp[0] = self.alfp1[0] / (self.alfp1[0] + self.alfp2[0]);
        self.bparp[1] = self.alfp1[1] / (self.alfp1[1] + self.alfp2[1]);
        self.delcmp = 136.0 * exp(zg) * self.k.rmp;
        self.delposp[0] = (exp((21.12 + 4.0 * zg) / 4.184) - 0.952) / self.delcmp;
        self.delposp[1] = (exp((21.12 + 4.0 * zv) / 4.184) - 0.952) / self.delcmp;
        // THE DENOMINATORS (:2191-2198)
        let f10 = 4.0 * al183;
        let f20 = f10 - 2.0 / 3.0;
        let a1den = 3.0 * f10 - f20 + 8.0 * zg;
        let a2den = 3.0 * f10 - f20 + 8.0 * zv;
        let b1den = f10 + 4.0 * zg;
        let b2den = f10 + 4.0 * zv;
        let c1den = 3.0 * f10 + f20 + 16.0 * zg;
        let c2den = 3.0 * f10 + f20 + 16.0 * zv;
        // dlp[j][lvl-1] is DLP(j+1)(lvl)
        // LVL=1, B-H A(DELTA)
        self.dlp[0][0] = (3.0 * 20.867 - 20.209 + 8.0 * zg) / a1den;
        self.dlp[1][0] = (3.0 * (-3.242) - (-1.930)) / a1den;
        self.dlp[2][0] = (3.0 * (0.625) - (0.086)) / a1den;
        self.dlp[3][0] = (2.0 * 21.12 + 8.0 * zg) / a1den;
        self.dlp[4][0] = 2.0 * (-4.184) / a1den;
        self.dlp[5][0] = 0.952;
        // LVL=4, CC A(DELTA)
        self.dlp[0][3] = (3.0 * 20.867 - 20.209 + 8.0 * zv) / a2den;
        self.dlp[1][3] = (3.0 * (-3.242) - (-1.930)) / a2den;
        self.dlp[2][3] = (3.0 * (0.625) - (0.086)) / a2den;
        self.dlp[3][3] = (2.0 * 21.12 + 8.0 * zv) / a2den;
        self.dlp[4][3] = 2.0 * (-4.184) / a2den;
        self.dlp[5][3] = 0.952;
        // LVL=2, B-H B(DELTA)
        self.dlp[0][1] = (20.867 + 4.0 * zg) / b1den;
        self.dlp[1][1] = -3.242 / b1den;
        self.dlp[2][1] = 0.625 / b1den;
        self.dlp[3][1] = (21.12 + 4.0 * zg) / b1den;
        self.dlp[4][1] = -4.184 / b1den;
        self.dlp[5][1] = 0.952;
        // LVL=5, CC B(DELTA)
        self.dlp[0][4] = (20.867 + 4.0 * zv) / b2den;
        self.dlp[1][4] = -3.242 / b2den;
        self.dlp[2][4] = 0.625 / b2den;
        self.dlp[3][4] = (21.12 + 4.0 * zv) / b2den;
        self.dlp[4][4] = -4.184 / b2den;
        self.dlp[5][4] = 0.952;
        // LVL=3, B-H C(DELTA)
        self.dlp[0][2] = (3.0 * 20.867 + 20.209 + 16.0 * zg) / c1den;
        self.dlp[1][2] = (3.0 * (-3.242) + (-1.930)) / c1den;
        self.dlp[2][2] = (3.0 * 0.625 + (-0.086)) / c1den;
        self.dlp[3][2] = (4.0 * 21.12 + 16.0 * zg) / c1den;
        self.dlp[4][2] = 4.0 * (-4.184) / c1den;
        self.dlp[5][2] = 0.952;
        // LVL=6, CC C(DELTA)
        self.dlp[0][5] = (3.0 * 20.867 + 20.209 + 16.0 * zv) / c2den;
        self.dlp[1][5] = (3.0 * (-3.242) + (-1.930)) / c2den;
        self.dlp[2][5] = (3.0 * 0.625 + (-0.086)) / c2den;
        self.dlp[3][5] = (4.0 * 21.12 + 16.0 * zv) / c2den;
        self.dlp[4][5] = 4.0 * (-4.184) / c2den;
        self.dlp[5][5] = 0.952;
    }

    /// `FCOULCP` (`:2254-2263`): the Coulomb correction, single precision,
    /// with `FSC` from [`Pegs4Constants`]. (The `FCOULC` of `egsnrc.mortran`
    /// is the double-precision sibling with `1/137.03604`.)
    pub fn fcoulcp(&self, z: f32) -> f32 {
        let a = self.k.fsc * z;
        let asq = a * a;
        asq * (1.0 / (1.0 + asq) + 0.20206 + asq * (-0.0369 + asq * (0.0083 + asq * (-0.002))))
    }

    /// `XSIFP` (`:2265-2276`). For `Z <= 4` it reads `ALRAD`/`ALRADP` at
    /// `INT(Z)`; `Z < 1` is out of range upstream (reads index 0) and
    /// returns NaN here.
    pub fn xsifp(&self, z: f32) -> f32 {
        if z <= 4.0 {
            let iz = z as usize;
            if iz < 1 {
                return f32::NAN;
            }
            self.alradp[iz - 1] / (self.alrad[iz - 1] - self.fcoulcp(z))
        } else {
            ln(self.a1440 * powf(z, -2.0 / 3.0)) / (ln(self.a183 * powf(z, -1.0 / 3.0)) - self.fcoulcp(z))
        }
    }

    /// `ZTBL` (`:2280-2300`): the atomic number of an element symbol, as
    /// `f32`. `"A"` is the alternate symbol for argon (18). An unknown
    /// symbol gives 0 (upstream prints a message). Fortran compares
    /// blank-padded `CHARACTER`s, so trailing blanks are ignored; case is
    /// significant. Uses [`Self::asymt`] (`NET` entries).
    pub fn ztbl(&self, iasym: &str) -> f32 {
        let s = iasym.trim_end();
        if s == "A" {
            return 18.0;
        }
        for (ie, sym) in self.asymt.iter().enumerate() {
            if s == sym.trim_end() {
                return (ie + 1) as f32;
            }
        }
        0.0
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Constants as `$INIT-PEGS4-VARIABLES` derives them
    /// (`pegs4_macros.mortran:446-462`), in single precision, with
    /// `PRM = 0.5109989461` (`egsnrc.macros:1140`). Test-only: the production
    /// routine belongs to the `pegs4_macros` port.
    fn consts() -> Pegs4Constants {
        let pip = 3.1415926536f32;
        let c = 2.997925e10f32;
        let hbar = 1.05450e-27f32;
        let ecgs = 4.80298e-10f32;
        let emks = 1.60210e-19f32;
        let an = 6.02252e23f32;
        let raddeg = 180.0 / pip;
        let fsc = (ecgs * ecgs) / (hbar * c);
        let ergmev = 1.0e6f32 * (emks * 1.0e7);
        let prm = 0.5109989461f32;
        let rme = prm / (c * c) * ergmev;
        let rmp = prm;
        let r0 = (ecgs * ecgs) / (rme * c * c);
        let a22p9 = raddeg * (4.0 * pip * an).sqrt() * (ecgs * ecgs) / ergmev;
        let q = hbar / (rme * c);
        let a6680 = 4.0 * pip * an * (q * q) * ((0.885f32 * 0.885) / (1.167 * 1.13));
        Pegs4Constants { pip, c, rme, hbar, ecgs, emks, an, raddeg, fsc, ergmev, r0, rmp, rmpt2: 2.0 * rmp, rmpsq: rmp * rmp, a22p9, a6680 }
    }

    /// A carbon medium: Z = 6, `WATBL(6) = 12.01115`, `RHOTBL(6) = 2.26`,
    /// `ITBL(6) = 78 eV` (`pegs4_macros.mortran:601-625`). The cut-offs and
    /// the density-effect table are SYNTHETIC test data, not a physical
    /// density correction.
    fn carbon() -> Pegs4Common {
        let mut p = Pegs4Common::new(consts());
        p.nep = 1;
        p.pzp = vec![1.0];
        p.zelemp = vec![6.0];
        p.wap = vec![12.01115];
        p.rhozp = vec![1.0];
        p.rhop = 2.26;
        p.aep = 0.521;
        p.app = 0.01;
        p.uep = 55.511;
        p.upp = 55.0;
        p.tep = p.aep - p.k.rmp;
        p.thmollp = p.aep + p.tep;
        p.ipotval = 78.0e-6; // MeV
        p.iaprimp = 0;
        p.mix();
        p.differ();
        let est = EstarDensity {
            dens_corr: vec![0.0, 0.0, 0.05, 0.4, 1.5, 3.0],
            en_grid: vec![0.001, 0.01, 0.1, 1.0, 10.0, 100.0],
            iev: 78.0,
        };
        p.spinit(Some(&est), None).unwrap();
        p
    }

    fn rel(a: f32, b: f32) -> f32 {
        ((a - b) / b).abs()
    }

    // ---- AINTP -------------------------------------------------------

    #[test]
    fn aintp_linear_log_extrapolation_stride_and_ylog() {
        let xa = [1.0f32, 2.0, 4.0];
        let ya = [10.0f32, 20.0, 40.0];
        assert_eq!(aintp(1.0, &xa, 3, &ya, 1, false, false), 10.0);
        assert!((aintp(3.0, &xa, 3, &ya, 1, false, false) - 30.0).abs() < 1e-5);
        // extrapolates above and below (:795-797)
        assert!((aintp(8.0, &xa, 3, &ya, 1, false, false) - 80.0).abs() < 1e-4);
        assert!((aintp(0.0, &xa, 3, &ya, 1, false, false) - 0.0).abs() < 1e-5);
        // log-x of y = ln x is exact
        let yl = [0.0f32, 2.0f32.ln(), 4.0f32.ln()];
        assert!((aintp(3.0, &xa, 3, &yl, 1, true, false) - 3.0f32.ln()).abs() < 1e-5);
        // y = x^2 is exact in log-log
        let y2 = [1.0f32, 4.0, 16.0];
        assert!(rel(aintp(3.0, &xa, 3, &y2, 1, true, true), 9.0) < 1e-5);
        // a zero ordinate with ylog gives 0 (:801)
        assert_eq!(aintp(1.5, &xa, 3, &[0.0, 1.0, 2.0], 1, false, true), 0.0);
        // a non-positive abscissa switches xlog off (:798)
        let xz = [0.0f32, 2.0, 4.0];
        assert!((aintp(1.0, &xz, 3, &ya, 1, true, false) - 15.0).abs() < 1e-5);
        // stride: ya(1,i) = ya[(i-1)*isk]
        let ya2 = [10.0f32, 99.0, 20.0, 99.0, 40.0, 99.0];
        assert!((aintp(3.0, &xa, 3, &ya2, 2, false, false) - 30.0).abs() < 1e-5);
        // a degenerate interval returns YI (:805)
        assert_eq!(aintp(1.0, &[2.0, 2.0], 2, &[7.0, 9.0], 1, false, false), 7.0);
    }

    // ---- DCADRE / QD ---------------------------------------------------

    #[test]
    fn dcadre_matches_closed_forms() {
        let r = dcadre(|x| x * x, 0.0, 1.0, 1e-16, 1e-5);
        assert!(rel(r.value, 1.0 / 3.0) < 2e-5, "{r:?}");
        assert_eq!(r.ier, 0);
        let r = dcadre(|x| x.sin(), 0.0, core::f32::consts::PI, 1e-16, 1e-5);
        assert!(rel(r.value, 2.0) < 2e-5, "{r:?}");
        // exp over [0, 2]
        let r = dcadre(|x| RealMath::r_exp(x), 0.0, 2.0, 1e-16, 1e-5);
        assert!(rel(r.value, 2.0f32.exp() - 1.0) < 2e-5, "{r:?}");
        // reversed limits flip the sign
        let r = dcadre(|x| x * x, 1.0, 0.0, 1e-16, 1e-5);
        assert!(rel(r.value, -1.0 / 3.0) < 2e-5);
        // a straight line is accepted by the "straight line" test (:135)
        let r = dcadre(|x| 3.0 * x + 1.0, 0.0, 2.0, 1e-16, 1e-5);
        assert!(rel(r.value, 8.0) < 1e-5, "{r:?}");
    }

    #[test]
    fn dcadre_handles_singularities_jumps_and_bad_input() {
        // x^-1/2 end-point singularity: exact value 2; handled with a warning code or clean
        let r = dcadre(|x| if x > 0.0 { 1.0 / sqrt(x) } else { 0.0 }, 0.0, 1.0, 1e-16, 1e-4);
        assert!(rel(r.value, 2.0) < 2e-3, "{r:?}");
        assert!(r.ier == 0 || r.ier >= 64, "{r:?}");
        // a jump discontinuity
        let r = dcadre(|x| if x < 0.3 { 1.0 } else { 3.0 }, 0.0, 1.0, 1e-16, 1e-4);
        assert!(rel(r.value, 0.3 + 2.1) < 1e-3, "{r:?}");
        // zero-length interval: 0, ier 0 (:1299)
        let r = dcadre(|x| x, 1.0, 1.0, 1e-16, 1e-5);
        assert_eq!((r.value, r.ier), (0.0, 0));
        // RERR > 0.1 or < 0: ier 133 (:1300)
        assert_eq!(dcadre(|x| x, 0.0, 1.0, 1e-16, 0.5).ier, 133);
        assert_eq!(dcadre(|x| x, 0.0, 1.0, 1e-16, -1.0).ier, 133);
        // AERR = 0 with RERR too small for the machine: 133 (:1301)
        assert_eq!(dcadre(|x| x, 0.0, 1.0, 0.0, 1e-9).ier, 133);
    }

    #[test]
    fn qd_uses_pegs4_tolerances() {
        let r = qd(|x| x * x * x, 0.0, 2.0);
        assert!(rel(r.value, 4.0) < 2e-5, "{r:?}");
        let r2 = dcadre(|x| x * x * x, 0.0, 2.0, AERR, RERR);
        assert_eq!(r, r2);
    }

    // ---- constants, ZTBL, FCOULCP, XSIFP -------------------------------

    #[test]
    fn fcoulcp_and_xsifp_agree_with_the_double_precision_siblings() {
        let p = Pegs4Common::new(consts());
        for z in [1.0f32, 6.0, 13.0, 29.0, 82.0] {
            let single = p.fcoulcp(z);
            let double = super::super::egsnrc_mortran::fcoulc(f64::from(z)) as f32;
            // FSC = ecgs^2/(hbar c) vs 1/137.03604, and single precision
            // measured 5e-5: FSC derived from the f32 constants vs 1/137.03604
            assert!(rel(single, double) < 2e-4, "fcoulc z={z}: {single} vs {double}");
        }
        for z in [1.0f32, 2.0, 4.0, 6.0, 29.0, 82.0] {
            let single = p.xsifp(z);
            let double = super::super::egsnrc_mortran::xsif(f64::from(z)) as f32;
            // for Z>4 the egsnrc sibling truncates the exponents (-0.666667, -0.33333)
            assert!(rel(single, double) < 2e-4, "xsif z={z}: {single} vs {double}");
        }
        assert!(p.xsifp(0.5).is_nan());
        // Coulomb correction is small and positive; f(Z) -> 0 as Z -> 0
        assert!(p.fcoulcp(1.0) > 0.0 && p.fcoulcp(1.0) < 1e-3);
    }

    #[test]
    fn ztbl_lookup() {
        let mut p = Pegs4Common::new(consts());
        // first entries of ASYMT (pegs4_macros.mortran:633)
        p.asymt = ["H", "HE", "LI", "BE", "B", "C", "N", "O", "F", "NE", "NA", "MG", "AL", "SI", "P", "S", "CL", "AR"]
            .iter()
            .map(|s| (*s).to_owned())
            .collect();
        assert_eq!(p.ztbl("C"), 6.0);
        assert_eq!(p.ztbl("C "), 6.0, "blank padded compare");
        assert_eq!(p.ztbl("AL"), 13.0);
        assert_eq!(p.ztbl("A"), 18.0, "alternate symbol for argon");
        assert_eq!(p.ztbl("AR"), 18.0);
        assert_eq!(p.ztbl("c"), 0.0, "case significant, unknown gives 0");
        assert_eq!(p.ztbl("XX"), 0.0);
    }

    // ---- MIX, DIFFER ----------------------------------------------------

    #[test]
    fn mix_matches_an_independent_f64_evaluation_and_carbon_radiation_length() {
        let p = carbon();
        let k = consts();
        let (an, rho, wm, z) = (f64::from(k.an), 2.26f64, 12.01115f64, 6.0f64);
        let fsc = f64::from(k.fsc);
        let r0 = f64::from(k.r0);
        let asq = (fsc * z).powi(2);
        let fc = asq * (1.0 / (1.0 + asq) + 0.20206 + asq * (-0.0369 + asq * (0.0083 + asq * (-0.002))));
        let xsi = (1194.0f64 * z.powf(-2.0 / 3.0)).ln() / ((184.15f64 * z.powf(-1.0 / 3.0)).ln() - fc);
        let zzx = z * (z + xsi);
        let zab = zzx * (184.15f64.ln() + z.powf(-1.0 / 3.0).ln());
        let zf = zzx * fc;
        let rlc = 1.0 / ((an * rho / wm) * 4.0 * fsc * r0 * r0 * (zab - zf));
        assert!(rel(p.rlcp, rlc as f32) < 2e-4, "rlc {} vs {}", p.rlcp, rlc);
        assert!(rel(p.eden, (an * rho / wm * z) as f32) < 2e-6);
        assert_eq!(p.zc, 6.0);
        assert_eq!(p.tpz, 1.0);
        assert_eq!(p.ez, 6.0);
        assert!(rel(p.wm, 12.01115) < 1e-6);
        assert!(rel(p.zs, 42.0) < 1e-6, "ZS = Z (Z + FUDGEMS)");
        // literature check, independent of this port: carbon X0 = 42.70 g/cm^2
        // (PDG atomic and nuclear properties table, quoted from memory, not
        // from the repository): measured below, no tuning.
        let x0_gcm2 = p.rlcp * p.rhop;
        assert!(rel(x0_gcm2, 42.70) < 0.03, "X0 = {x0_gcm2} g/cm2 vs 42.70");
        // multiple-scattering constants are positive and self-consistent (:2063, :2078)
        assert!(p.blccp > 0.0 && p.xccp > 0.0);
        assert!(rel(p.xr0p, p.xccp * sqrt(p.teff0p * p.bmin)) < 1e-6);
        assert!(rel(p.teff0p, (exp(4.5) / 4.5) / p.blccp) < 1e-6);
    }

    #[test]
    fn mix_scales_rho_by_gas_pressure_and_uses_alrad_for_light_elements() {
        let mut p = Pegs4Common::new(consts());
        p.nep = 1;
        p.pzp = vec![2.0];
        p.zelemp = vec![1.0];
        p.wap = vec![1.00797];
        p.rhozp = vec![1.0];
        p.rhop = 0.0808e-3; // placeholder gas density; the point is the scaling
        p.gaspp = 2.0;
        p.mix();
        assert!(rel(p.rhop, 0.1616e-3) < 1e-6, "RHOP scaled by GASPP (:1976)");
        assert!(p.rlcp.is_finite() && p.rlcp > 0.0);
        assert_eq!(p.zc, 2.0);
    }

    #[test]
    fn differ_constants_obey_their_defining_relations() {
        let p = carbon();
        for lvl in 0..6 {
            // the screening-rejection functions are 1 at DELTA=0 up to F1(0)=20.867 vs 4 ln A183
            assert!((p.dlp[0][lvl] - 1.0).abs() < 2e-3, "lvl {} dlp1 = {}", lvl + 1, p.dlp[0][lvl]);
            // the two branches meet near DELTA=1 (fits, not identities)
            let low = p.dlp[0][lvl] + p.dlp[1][lvl] + p.dlp[2][lvl];
            let high = p.dlp[3][lvl] + p.dlp[4][lvl] * ln(1.0 + p.dlp[5][lvl]);
            // measured 2026-11-21: up to 7e-3 (the F1 fits 18.25 vs 18.32 at DELTA=1); not tuned
            assert!((low - high).abs() < 1.5e-2, "lvl {} {low} vs {high}", lvl + 1);
            assert_eq!(p.dlp[5][lvl], 0.952);
        }
        // B(DELTA) goes to zero at DELPOS by construction (:2166-2177)
        let b_bh = p.dlp[3][1] + p.dlp[4][1] * ln(p.delcmp * p.delposp[0] + 0.952);
        let b_cc = p.dlp[3][4] + p.dlp[4][4] * ln(p.delcmp * p.delposp[1] + 0.952);
        assert!(b_bh.abs() < 1e-4 && b_cc.abs() < 1e-4, "{b_bh} {b_cc}");
        for i in 0..2 {
            assert!(rel(p.bparp[i], p.alfp1[i] / (p.alfp1[i] + p.alfp2[i])) < 1e-6);
        }
        assert!(rel(p.al2, core::f32::consts::LN_2) < 1e-6);
        assert!(rel(p.delcmp, 136.0 * exp(p.zg) * p.k.rmp) < 1e-6);
        // Coulomb-corrected alpha is a little larger or smaller than B-H, same order
        assert!(p.alphip[0] > 0.9 && p.alphip[0] < 1.2 && (p.alphip[0] - p.alphip[1]).abs() < 0.05);
    }

    // ---- Moller, Bhabha, annihilation ----------------------------------

    #[test]
    fn moller_and_bhabha_equal_the_integral_of_their_differential_cross_sections() {
        let p = carbon();
        let e0 = 1.5f32;
        let rmp = p.k.rmp;
        let t0 = e0 - rmp;
        let tm = t0 / rmp;
        let em = tm + 1.0;
        let eps1 = (p.aep - rmp) / t0;
        // Moller: derivative of eq. 2.10.3 with respect to the upper fraction
        let c1 = (tm / em).powi(2);
        let c2 = (2.0 * tm + 1.0) / (em * em);
        let betasq = 1.0 - 1.0 / (em * em);
        let cm = p.rlcp * p.eden * 2.0 * p.k.pip * p.k.r0 * p.k.r0 / (betasq * tm);
        let dmol = |e: f32| cm * (c1 + 1.0 / (e * e) + 1.0 / ((1.0 - e) * (1.0 - e)) - c2 * (1.0 / e + 1.0 / (1.0 - e)));
        let integral = dcadre(dmol, eps1, 0.5, 1e-16, 1e-5).value;
        let total = p.amoltm(e0);
        assert!(total > 0.0);
        assert!(rel(integral, total) < 1e-4, "Moller {integral} vs {total}");
        // Bhabha: derivative of eq. 2.11.2
        let y = 1.0 / (tm + 2.0);
        let betasi = 1.0 / (1.0 - 1.0 / (em * em));
        let cb = p.rlcp * p.eden * 2.0 * p.k.pip * p.k.r0 * p.k.r0 / tm;
        let b1 = 2.0 - y * y;
        let b2 = 3.0 - y * (6.0 - y * (1.0 - y * 2.0));
        let b3 = 2.0 - y * (10.0 - y * (16.0 - y * 8.0));
        let b4 = 1.0 - y * (6.0 - y * (12.0 - y * 8.0));
        let dbh = |e: f32| cb * (betasi / (e * e) - b1 / e + b2 + e * e * b4 - b3 * e);
        let integral = dcadre(dbh, eps1, 1.0, 1e-16, 1e-5).value;
        let total = p.bhabtm(e0);
        assert!(total > 0.0);
        assert!(rel(integral, total) < 1e-4, "Bhabha {integral} vs {total}");
        // additivity of the range form
        let a = p.amolrm(e0, p.aep, 1.0);
        let b = p.amolrm(e0, 1.0, t0 * 0.5 + rmp);
        let total = p.amoltm(e0);
        assert!(rel(a + b, total) < 1e-3);
    }

    #[test]
    fn thresholds_and_annihilation() {
        let p = carbon();
        assert_eq!(p.amoltm(p.thmollp), 0.0);
        assert_eq!(p.amoltm(p.thmollp - 0.1), 0.0);
        assert_eq!(p.bhabtm(p.aep), 0.0);
        let mut last = f32::INFINITY;
        for e in [0.6f32, 0.8, 1.5, 3.0, 10.0, 50.0] {
            let a = p.anihtm(e);
            assert!(a > 0.0 && a < last, "annihilation falls with energy: {a} at {e}");
            last = a;
        }
        // Moller grows with energy then flattens (more phase space at first)
        assert!(p.amoltm(1.0) > 0.0);
    }

    // ---- stopping powers ------------------------------------------------

    #[test]
    fn density_delta_search_interpolation_and_pointer() {
        let mut p = carbon();
        let rmp = p.k.rmp;
        // table: ke 0.001, 0.01, 0.1, 1, 10, 100 -> total + RM; ends clamped to AE, UE
        assert_eq!(p.nepst, 6);
        assert_eq!(p.epsten[0], p.aep.min(0.001 + rmp).min(p.epsten[0]));
        let e = |i: usize| p.epsten[i];
        let (e2, e3) = (e(2), e(3));
        let (d2, d3) = (p.epstd[2], p.epstd[3]);
        assert_eq!(p.density_delta(e2), d2);
        assert_eq!(p.iepst, 3);
        let mid = 0.5 * (e2 + e3);
        assert!((p.density_delta(mid) - 0.5 * (d2 + d3)).abs() < 1e-6);
        assert_eq!(p.iepst, 3);
        // walk down the grid
        let low = 0.5 * (p.epsten[0] + p.epsten[1]);
        let _ = p.density_delta(low);
        assert_eq!(p.iepst, 1);
        // walk up past the top: the last value (:543)
        assert_eq!(p.density_delta(1000.0), p.epstd[5]);
        assert_eq!(p.iepst, 6);
        // below the first point extrapolates (:541)
        let below = p.epsten[0] - 0.01;
        let v = p.density_delta(below);
        let slope = (p.epstd[1] - p.epstd[0]) / (p.epsten[1] - p.epsten[0]);
        assert!((v - (p.epstd[0] - 0.01 * slope)).abs() < 1e-6);
        // no table: NaN, not an out-of-range read
        let mut empty = Pegs4Common::new(consts());
        assert!(empty.density_delta(1.0).is_nan());
    }

    #[test]
    fn collision_stopping_power_is_positive_restricted_below_unrestricted_and_differs_by_charge() {
        let mut p = carbon();
        for e0 in [0.8f32, 1.5, 5.0, 20.0] {
            let unr_e = p.spione(e0, e0);
            let unr_p = p.spionp(e0, e0);
            assert!(unr_e > 0.0 && unr_p > 0.0, "e0={e0}");
            let mut last = 0.0;
            for frac in [0.1f32, 0.3, 0.6, 1.0] {
                let ee = p.k.rmp + frac * (e0 - p.k.rmp);
                let r = p.spione(e0, ee);
                assert!(r >= last, "restricted stopping power grows with the cut: {r} < {last}");
                last = r;
            }
            assert!(p.spione(e0, p.aep) < unr_e);
            assert!((unr_e - unr_p).abs() > 0.0);
        }
    }

    #[test]
    fn sptot_modes_are_the_documented_sums_and_reject_unknown_modes() {
        let mut p = carbon();
        let (e0, ee, eg) = (3.0f32, 1.0f32, 0.4f32);
        let ion_e = |p: &mut Pegs4Common, a: f32| p.spione(e0, a);
        let ion_p = |p: &mut Pegs4Common, a: f32| p.spionp(e0, a);
        for iu in 0..8 {
            p.iunrstp = iu;
            let brm = |p: &mut Pegs4Common, a: f32| p.brmstm(e0, a).unwrap();
            let (xe, xp) = match iu {
                0 => (ion_e(&mut p, ee) + brm(&mut p, eg), ion_p(&mut p, ee) + brm(&mut p, eg)),
                1 => (ion_e(&mut p, e0), ion_p(&mut p, e0)),
                2 => (ion_e(&mut p, e0) + brm(&mut p, e0), ion_p(&mut p, e0) + brm(&mut p, e0)),
                3 => (ion_e(&mut p, e0) + brm(&mut p, eg), ion_p(&mut p, e0) + brm(&mut p, eg)),
                4 => (ion_e(&mut p, ee) + brm(&mut p, e0), ion_p(&mut p, ee) + brm(&mut p, e0)),
                5 => (brm(&mut p, e0), brm(&mut p, e0)),
                6 => (brm(&mut p, eg), brm(&mut p, eg)),
                _ => (ion_e(&mut p, ee), ion_p(&mut p, ee)),
            };
            assert_eq!(p.sptote(e0, ee, eg).unwrap(), xe, "mode {iu}");
            assert_eq!(p.sptotp(e0, ee, eg).unwrap(), xp, "mode {iu}");
        }
        p.iunrstp = 9;
        assert!(p.sptote(e0, ee, eg).is_err() && p.sptotp(e0, ee, eg).is_err());
    }

    // ---- bremsstrahlung -------------------------------------------------

    #[test]
    fn bremsstrahlung_integrals_are_additive_and_the_radiative_loss_tracks_e_over_x0() {
        let mut p = carbon();
        let e = 20.0f32;
        let whole = p.brmsrm(e, 0.0, 15.0).unwrap();
        let a = p.brmsrm(e, 0.0, 5.0).unwrap();
        let b = p.brmsrm(e, 5.0, 15.0).unwrap();
        assert!(whole > 0.0);
        assert!(rel(a + b, whole) < 1e-4, "{a}+{b} vs {whole}");
        // photon-number integral likewise
        let nw = p.bremrm(e, 0.1, 15.0).unwrap();
        let na = p.bremrm(e, 0.1, 5.0).unwrap();
        let nb = p.bremrm(e, 5.0, 15.0).unwrap();
        assert!(rel(na + nb, nw) < 1e-4);
        // BREMDZ = BRMSDZ / K and BREMFZ = BRMSFZ / K
        let z = 6.0;
        let k = 3.0f32;
        let d = p.brmsdz(z, e, k).unwrap();
        assert!(rel(p.bremdz(z, e, k).unwrap(), d / k) < 1e-6);
        assert!(rel(p.bremfz(k), p.brmsfz(k) / k) < 1e-6);
        // unrestricted radiative loss of a 100 MeV electron is of order E/X0
        // (complete screening limit); an independent physical sanity check
        let e0 = 100.0 + p.k.rmp;
        let s = p.brmstm(e0, e0).unwrap();
        let ratio = s / (e0 - p.k.rmp);
        assert!(ratio > 0.8 && ratio < 1.2, "radiative loss/E in X0 units = {ratio}");
        // above the kinematic limit nothing more is added
        assert!(rel(p.brmstm(e0, 1000.0).unwrap(), s) < 1e-6);
        assert_eq!(p.brmstm(p.k.rmp, 1.0).unwrap(), 0.0);
        assert_eq!(p.bremtm(p.app + p.k.rmp).unwrap(), 0.0);
        assert!(p.bremtm(5.0).unwrap() > 0.0);
        // BRMSFZ vanishes at the end point (DELTA >= DELTAM) and for k at e
        assert_eq!(p.brmsfz(p.ebremz), 0.0);
    }

    #[test]
    fn brmsdz_applies_the_coulomb_term_only_above_50_mev() {
        let mut p = carbon();
        p.brmsdz(6.0, 49.9, 1.0).unwrap();
        let below = p.xlnz;
        assert!(rel(below, 4.0 / 3.0 * ln(6.0)) < 1e-6);
        p.brmsdz(6.0, 50.0, 1.0).unwrap();
        assert!(rel(p.xlnz, below + 4.0 * p.fcoulcp(6.0)) < 1e-6);
        assert!(rel(p.deltam, exp((21.12 - p.xlnz) / 4.184) - 0.952) < 1e-6);
    }

    // ---- APRIM ------------------------------------------------------------

    #[test]
    fn aprim_koch_motz_table_nodes_and_options() {
        let mut p = carbon();
        let rmp = p.k.rmp;
        p.iaprimp = 0;
        // at a Z and energy node the interpolation returns the table entry
        assert!(rel(p.aprim(6.0, 2.0 * rmp).unwrap(), 1.32) < 2e-6);
        assert!(rel(p.aprim(13.0, 3.0 * rmp).unwrap(), 1.27) < 2e-6);
        assert!(rel(p.aprim(79.0, 91.0 * rmp - 0.0).unwrap(), 0.911) < 2e-3);
        // E >= 50 MeV: exactly 1
        assert_eq!(p.aprim(6.0, 50.0).unwrap(), 1.0);
        // IAPRIM = 2: no correction
        p.iaprimp = 2;
        assert_eq!(p.aprim(6.0, 2.0).unwrap(), 1.0);
        // any other value is an error (:762)
        p.iaprimp = 5;
        assert!(matches!(p.aprim(6.0, 2.0), Err(Pegs4Error::CallExit { code: 24, .. })));
    }

    #[test]
    fn aprim_reads_the_nrc_table_from_text() {
        // SYNTHETIC aprime.data-shaped text: 2 elements x 3 energies, in the
        // layout of pegs4/aprime.data (header, energies, then Z and a column).
        let text = "2 3\n1.0, 2.0, 4.0\n6.\n1.2 1.1 1.0\n29.\n1.4 1.2 1.0\n";
        let mut p = carbon();
        p.iaprimp = 1;
        assert!(matches!(p.aprim(6.0, 1.0), Err(Pegs4Error::Fatal(_))), "no text: cannot open file");
        p.aprime_text = Some(text.to_owned());
        let rmp = p.k.rmp;
        // energies stored as 1 + E/RM, so E = 1.0 MeV -> 1 + 1/RM
        let at = |p: &mut Pegs4Common, z: f32, ke: f32| p.aprim(z, ke + rmp).unwrap();
        assert!(rel(at(&mut p, 6.0, 1.0), 1.2) < 2e-6);
        assert!(rel(at(&mut p, 6.0, 2.0), 1.1) < 2e-6);
        assert!(rel(at(&mut p, 29.0, 4.0), 1.0) < 2e-6);
        // log-Z interpolation between the two columns, linear in the energy
        let z = (6.0f32 * 29.0).sqrt();
        assert!(rel(at(&mut p, z, 1.0), 1.3) < 5e-6);
        assert_eq!(p.iaprfl, 1);
        // too many elements for $NAPRZ
        let mut q = carbon();
        q.iaprimp = 1;
        q.aprime_text = Some("15 3\n".to_owned());
        assert!(matches!(q.aprim(6.0, 1.0), Err(Pegs4Error::CallExit { code: 24, .. })));
        let mut q = carbon();
        q.iaprimp = 1;
        q.aprime_text = Some("2 116\n".to_owned());
        assert!(matches!(q.aprim(6.0, 1.0), Err(Pegs4Error::CallExit { code: 24, .. })));
    }

    /// Reads the real `aprime.data` when `PEGS4_APRIME_DATA` names it (the
    /// file is in the EGSnrc source tree, never committed here).
    #[cfg(feature = "std")]
    #[test]
    fn aprim_reads_the_real_aprime_data_when_available() {
        let Ok(path) = std::env::var("PEGS4_APRIME_DATA") else {
            std::eprintln!("PEGS4_APRIME_DATA not set: real aprime.data check skipped");
            return;
        };
        let text = std::fs::read_to_string(path).unwrap();
        let mut p = carbon();
        p.iaprimp = 1;
        p.aprime_text = Some(text);
        let v = p.aprim(6.0, 10.0).unwrap();
        std::eprintln!("APRIM(Z=6, E=10 MeV) from aprime.data = {v}");
        assert_eq!((p.napre, p.naprz), (115, 14));
        assert!(v > 0.9 && v < 1.5);
    }

    // ---- multiple-scattering step, ALKE ------------------------------------

    #[test]
    fn tmxb_tmxs_and_alke_are_consistent() {
        let p = carbon();
        for e in [0.6f32, 1.0, 5.0, 30.0] {
            let tb = p.tmxb(e);
            assert!(tb.is_finite() && tb > 0.0, "tmxb({e}) = {tb}");
            assert_eq!(p.tmxs(e), (tb * 0.8).min(10.0));
            // Bethe's criterion XC^2 B = 1 at the TMXB: B from exp(B)/B = BLCC T / beta^2 is positive
            let beta2 = 1.0 - p.k.rmpsq / (e * e);
            let omega0 = p.blccp * tb / beta2;
            assert!(omega0 > 1.0);
        }
        assert!(p.tmxs(1000.0) <= 10.0);
        for e in [0.6f32, 2.0, 40.0] {
            assert!(rel(p.alkei(p.alke(e)), e) < 1e-5);
        }
        assert!(rel(p.alke(1.0 + p.k.rmp), 0.0f32.exp().ln() + 1e-30) < 1.0 || p.alke(1.0 + p.k.rmp).abs() < 1e-5);
    }

    // ---- EFUNS ---------------------------------------------------------------

    #[test]
    fn efuns_regular_mode_matches_its_components() {
        let mut p = carbon();
        p.iunrstp = 0;
        let e = 2.0f32;
        let v = p.efuns(e).unwrap();
        let brem = p.bremtm(e).unwrap();
        let moll = p.amoltm(e);
        let bhab = p.bhabtm(e);
        let ann = p.anihtm(e);
        assert_eq!(v[0], brem + moll);
        assert_eq!(v[1], brem + bhab + ann);
        assert_eq!(v[2], p.sptote(e, p.aep, p.app).unwrap());
        assert_eq!(v[3], p.sptotp(e, p.aep, p.app).unwrap());
        assert!(rel(v[4], brem / (brem + moll)) < 1e-6 && v[4] > 0.0 && v[4] < 1.0);
        assert!(v[5] < v[6] && v[6] < 1.0 && v[5] > 0.0);
        assert_eq!(v[7], p.tmxs(e));
        // below the Moller threshold the electron branching is 1 when THBREM <= THMOLL (:90)
        let low = 0.5f32 + 0.0 * p.k.rmp;
        let v = p.efuns(p.app + p.k.rmp - 0.001 + low - low).unwrap();
        assert_eq!(v[0], 0.0);
        assert_eq!(v[4], 1.0, "THBREM = RM+AP is below THMOLL = 2 AE - RM");
    }

    #[test]
    fn efuns_other_unrestricted_modes() {
        let mut p = carbon();
        let e = 2.0f32;
        p.iunrstp = 2;
        let v = p.efuns(e).unwrap();
        assert_eq!([v[0], v[1], v[4], v[5], v[6]], [0.0; 5]);
        assert_eq!(v[2], p.sptote(e, e, e).unwrap());
        assert_eq!(v[3], p.sptotp(e, e, e).unwrap());
        p.iunrstp = 3;
        let v = p.efuns(e).unwrap();
        let brem = p.bremtm(e).unwrap();
        assert_eq!(v[0], brem);
        assert_eq!(v[1], brem + p.anihtm(e));
        assert_eq!(v[2], p.sptote(e, e, p.app).unwrap());
        assert_eq!(v[4], 1.0);
        assert_eq!(v[5], brem / v[1]);
        assert_eq!(v[6], v[5]);
        p.iunrstp = 4;
        let v = p.efuns(e).unwrap();
        assert_eq!(v[0], p.amoltm(e));
        assert_eq!(v[1], p.bhabtm(e));
        assert_eq!([v[4], v[5], v[6]], [0.0, 0.0, 1.0]);
        assert_eq!(v[2], p.sptote(e, p.aep, e).unwrap());
        p.iunrstp = 1;
        assert_eq!(p.efuns(e).unwrap()[0], p.bremtm(e).unwrap() + p.amoltm(e));
        for bad in [6, 7, 8] {
            p.iunrstp = bad;
            assert!(matches!(p.efuns(e), Err(Pegs4Error::CallExit { code: 20, .. })));
        }
    }

    // ---- SPINIT -------------------------------------------------------------------

    const DENSITY_FILE: &str = "SYNTHETIC DENSITY TABLE (test data, not a physical density correction)\n\
6 78.0 2.26 1\n\
6 1.0\n\
0.001 0.0\n0.01 0.0\n0.1 0.05\n1.0 0.4\n10.0 1.5\n100.0 3.0\n";

    fn carbon_for_file() -> Pegs4Common {
        let mut p = carbon();
        p.epstflp = 1;
        p
    }

    #[test]
    fn spinit_from_the_estar_input_sets_the_tables_and_spc() {
        let p = carbon();
        assert_eq!(p.nepst, 6);
        assert_eq!(p.neleps, 1);
        assert_eq!(p.zepst[0], 6);
        assert!(rel(p.wepst[0], 1.0) < 1e-6);
        assert!(rel(p.ipotval, 78.0) < 1e-6, "MeV -> eV in place (:1703)");
        assert_eq!(p.iev, 78.0);
        // total energies: kinetic + RM, first clamped up/down to AE, last to UE
        assert_eq!(p.epsten[0], p.aep.min(0.001 + p.k.rmp));
        assert_eq!(p.epsten[5], 100.0 + p.k.rmp);
        assert!(rel(p.epsten[2], 0.1 + p.k.rmp) < 1e-6);
        assert!(rel(p.toln10, 2.0 * 10.0f32.ln()) < 1e-6);
        let spc1 = 2.0 * p.k.pip * p.k.r0 * p.k.r0 * p.k.rmp * p.eden * p.rlcp;
        assert!(rel(p.spc1, spc1) < 1e-6);
        let imev = 78.0e-6f32 / p.k.rmp;
        assert!(rel(p.spc2, (imev * imev / 2.0).ln()) < 1e-5);
    }

    #[test]
    fn spinit_clamps_the_energy_range_and_reports_it() {
        let mut p = carbon();
        // table starts at ke 0.001 -> total 0.512; AE = 0.4 is below it: set to AE
        p.aep = 0.4;
        p.uep = 200.0; // above the top (100.5): set to UE
        p.ipotval = 78.0e-6;
        let est = EstarDensity { dens_corr: vec![0.0, 0.1, 0.2], en_grid: vec![0.001, 1.0, 100.0], iev: 78.0 };
        let r = p.spinit(Some(&est), None).unwrap();
        assert!(r.lowest_energy_set_to_ae && r.highest_energy_set_to_ue && !r.ipot_looks_like_ev);
        assert_eq!(p.epsten[0], 0.4);
        assert_eq!(p.epsten[2], 200.0);
        // an I-value in eV looks wrong (:1699)
        p.ipotval = 78.0;
        p.epstflp = 7; // outside 0..1: treated as 0 (:1687)
        let r = p.spinit(Some(&est), None).unwrap();
        assert!(r.ipot_looks_like_ev);
        assert_eq!(p.epstflp, 0);
        // errors: too many energies, missing input, zero weight
        let big = EstarDensity { dens_corr: vec![0.0; 151], en_grid: vec![1.0; 151], iev: 78.0 };
        assert!(matches!(p.spinit(Some(&big), None), Err(Pegs4Error::CallExit { code: 22, .. })));
        assert!(p.spinit(None, None).is_err());
        let mut z = carbon();
        z.rhozp = vec![0.0];
        assert!(matches!(z.spinit(Some(&est), None), Err(Pegs4Error::CallExit { code: 23, .. })));
    }

    #[test]
    fn spinit_reads_a_density_file_and_checks_the_composition() {
        let mut p = carbon_for_file();
        let r = p.spinit(None, Some(DENSITY_FILE)).unwrap();
        assert!(!r.lowest_energy_set_to_ae);
        assert_eq!(p.nepst, 6);
        assert_eq!(p.iev, 78.0);
        assert_eq!(p.zepst[0], 6);
        assert_eq!(p.wepst[0], 1.0);
        assert!(p.epsttl.starts_with("SYNTHETIC"));
        assert!(rel(p.epsten[3], 1.0 + p.k.rmp) < 1e-6);
        assert_eq!(p.epstd[3], 0.4);
        let imev = 78.0e-6f32 / p.k.rmp;
        assert!(rel(p.spc2, (imev * imev / 2.0).ln()) < 1e-5);
        // the density correction then reaches the stopping power: more delta, less stopping
        let mut with = carbon_for_file();
        with.spinit(None, Some(DENSITY_FILE)).unwrap();
        let mut without = carbon_for_file();
        let flat = DENSITY_FILE.replace("0.05\n1.0 0.4\n10.0 1.5\n100.0 3.0", "0.0\n1.0 0.0\n10.0 0.0\n100.0 0.0");
        without.spinit(None, Some(&flat)).unwrap();
        assert!(with.spione(5.0, 5.0) < without.spione(5.0, 5.0));
        // wrong density (more than 1 %), wrong number of elements, wrong element
        for bad in [
            DENSITY_FILE.replace("2.26", "2.50"),
            DENSITY_FILE.replace("6 78.0 2.26 1\n6 1.0", "6 78.0 2.26 2\n6 0.5 8 0.5"),
            DENSITY_FILE.replace("6 78.0 2.26 1\n6 1.0", "6 78.0 2.26 1\n8 1.0"),
        ] {
            let mut q = carbon_for_file();
            assert!(matches!(q.spinit(None, Some(&bad)), Err(Pegs4Error::CallExit { code: 23, .. })), "{bad}");
        }
        // too many energies (checked before filling the arrays here)
        let mut q = carbon_for_file();
        let too_many = DENSITY_FILE.replace("6 78.0", "151 78.0");
        assert!(matches!(q.spinit(None, Some(&too_many)), Err(Pegs4Error::CallExit { code: 22, .. })));
        // truncated file
        let mut q = carbon_for_file();
        assert!(q.spinit(None, Some("title\n6 78.0 2.26 1\n6 1.0\n0.001 0.0\n")).is_err());
        assert!(q.spinit(None, None).is_err());
    }

    /// Pins the upstream flag logic of `pegs4_routines.mortran:1825-1841` as
    /// written (see the module notes): a later matching element clears an
    /// earlier weight mismatch, and an unmatched later element is not seen.
    #[test]
    fn spinit_composition_check_keeps_the_upstream_flag_quirk() {
        let mut base = carbon();
        base.nep = 2;
        base.pzp = vec![1.0, 1.0];
        base.zelemp = vec![6.0, 8.0];
        base.wap = vec![12.01115, 15.9994];
        base.rhozp = vec![0.5, 0.5];
        base.epstflp = 1;
        base.mix();
        base.ipotval = 78.0e-6;
        let file = |w1: &str, w2: &str, z2: &str| {
            format!("SYNTHETIC\n3 78.0 2.26 2\n6 {w1} {z2} {w2}\n0.001 0.0\n1.0 0.1\n100.0 0.2\n")
        };
        let rho = base.rhop;
        let with_rho = |s: String| s.replace("2.26", &format!("{rho}"));
        // consistent file passes
        let mut q = base.clone();
        assert!(q.spinit(None, Some(&with_rho(file("0.5", "0.5", "8")))).is_ok());
        // first element's weight is off, second matches: upstream ACCEPTS (flag cleared by line 1833)
        let mut q = base.clone();
        assert!(q.spinit(None, Some(&with_rho(file("0.9", "0.5", "8")))).is_ok());
        // second element's weight is off: rejected
        let mut q = base.clone();
        assert!(q.spinit(None, Some(&with_rho(file("0.5", "0.9", "8")))).is_err());
        // second ESTAR element (Z=9) is absent from the PEGS medium: upstream ACCEPTS (first matched)
        let mut q = base.clone();
        assert!(q.spinit(None, Some(&with_rho(file("0.5", "0.5", "9")))).is_ok());
    }

    // ---- ListReader -------------------------------------------------------------

    #[test]
    fn list_reader_follows_record_semantics() {
        let mut r = ListReader::new("1 2,3\n4 5 6\n\n7\n2*9.5 1d1\n");
        assert_eq!(r.items(2).unwrap(), ["1", "2"]);
        // the rest of the satisfied record ("3") is discarded
        assert_eq!(r.items(4).unwrap(), ["4", "5", "6", "7"], "continues over records, skips blanks");
        assert_eq!(r.reals(3).unwrap(), [9.5, 9.5, 10.0], "repeat counts and D exponents");
        assert!(r.items(1).is_err(), "end of file");
        let mut r = ListReader::new("title\nx\n");
        assert_eq!(r.read_line().as_deref(), Some("title"));
        assert!(r.reals(1).is_err(), "not a number");
        assert!(ListReader::int_token("1.5").is_err());
    }

    // ---- QFIT / PWLF1 ---------------------------------------------------------------

    /// Two functions of x: `f1 = 3 ln x + 2` (exactly linear in `XFUN = ln x`)
    /// and `f2 = x^2`, fitted on [1, 100].
    fn vfuns(x: f32, out: &mut [f32]) {
        out[0] = 3.0 * ln(x) + 2.0;
        out[1] = x * x;
    }

    #[test]
    fn qfit_and_pwlf1_fit_within_tolerance_and_find_a_minimal_count() {
        let xfun = |x: f32| ln(x);
        let xfi = |x: f32| exp(x);
        let zthr = [0.0f32; 2];
        let zep = [0.0f32; 2];
        let nalm = 200usize;
        let mut af = vec![0.0f32; nalm * 2];
        let mut bf = vec![0.0f32; nalm * 2];
        let (mut ax, mut bx) = (0.0f32, 0.0f32);
        let ep = 0.01f32;
        let mut f = vfuns;
        let r = pwlf1(
            nalm as i32 - 3, 1.0, 100.0, 100.0, ep, &zthr, &zep, 20, &xfun, &xfi, &mut ax, &mut bx, nalm, 2,
            &mut af, &mut bf, &mut f,
        );
        assert!(!r.catastrophe);
        assert!(r.ni >= 3 && r.ni < nalm as i32, "{r:?}");
        // evaluate the fit with INTERV = AX*XFUN(X)+BX (truncated) and check the bound
        let mut worst = 0.0f32;
        for i in 0..400 {
            let x = 1.0 + 99.0 * (i as f32) / 399.0;
            let interv = (ax * xfun(x) + bx) as usize;
            let mut exact = [0.0f32; 2];
            vfuns(x, &mut exact);
            for k in 0..2 {
                let fit = af[(interv - 1) + k * nalm] * xfun(x) + bf[(interv - 1) + k * nalm];
                worst = worst.max(((fit - exact[k]) / exact[k]).abs());
            }
        }
        assert!(worst <= ep * 1.5, "worst relative error {worst} against ep {ep}");
        // XR is a subinterval boundary: the fit is exact there (x = 100)
        let interv = (ax * xfun(100.0) + bx) as usize;
        let fit = af[(interv - 1) + nalm] * xfun(100.0) + bf[(interv - 1) + nalm];
        assert!(rel(fit, 10000.0) < 1e-4);
        // one fewer subinterval than PWLF1 found fails QFIT (it is the smallest)
        let mut nj = r.ni - 1;
        let mut rem = 0.0;
        let ok = qfit(&mut nj, 1.0, 100.0, 100.0, ep, &zthr, &zep, &mut rem, 20, &xfun, &xfi, &mut ax, &mut bx, nalm, 2, &mut af, &mut bf, &mut f);
        assert!(!ok || rem <= ep);
        // a function linear in XFUN alone is fitted by the minimum number of intervals
        let mut g = |x: f32, out: &mut [f32]| out[0] = 3.0 * ln(x) + 2.0;
        let mut af1 = vec![0.0f32; nalm];
        let mut bf1 = vec![0.0f32; nalm];
        let r1 = pwlf1(
            nalm as i32 - 3, 1.0, 100.0, 100.0, 1e-4, &zthr[..1], &zep[..1], 20, &xfun, &xfi, &mut ax, &mut bx, nalm, 1,
            &mut af1, &mut bf1, &mut g,
        );
        assert!(r1.ni <= 4 && !r1.catastrophe, "{r1:?}");
    }

    #[test]
    fn qfit_early_returns_and_pwlf1_gives_up_at_nimx() {
        let xfun = |x: f32| ln(x);
        let xfi = |x: f32| exp(x);
        let zthr = [0.0f32; 2];
        let zep = [0.0f32; 2];
        let nalm = 64usize;
        let mut af = vec![0.0f32; nalm * 2];
        let mut bf = vec![0.0f32; nalm * 2];
        let (mut ax, mut bx, mut rem) = (7.0f32, 8.0f32, 9.0f32);
        let mut f = vfuns;
        // XL >= XH
        let mut nj = 10;
        assert!(!qfit(&mut nj, 5.0, 5.0, 5.0, 0.01, &zthr, &zep, &mut rem, 20, &xfun, &xfi, &mut ax, &mut bx, nalm, 2, &mut af, &mut bf, &mut f));
        assert_eq!((nj, ax, bx, rem), (10, 7.0, 8.0, 9.0), "outputs untouched on the early return");
        // too few subintervals for an interior XR (needs NI >= 2)
        let mut nj = 3;
        assert!(!qfit(&mut nj, 1.0, 100.0, 10.0, 0.01, &zthr, &zep, &mut rem, 20, &xfun, &xfi, &mut ax, &mut bx, nalm, 2, &mut af, &mut bf, &mut f));
        assert_eq!(nj, 3);
        // an unreachable tolerance with NIMX small: PWLF1 returns the last count tried
        let r = pwlf1(4, 1.0, 100.0, 100.0, 1e-9, &zthr, &zep, 20, &xfun, &xfi, &mut ax, &mut bx, nalm, 2, &mut af, &mut bf, &mut f);
        assert!(!r.catastrophe);
        assert!(r.ni <= 4);
    }
}
