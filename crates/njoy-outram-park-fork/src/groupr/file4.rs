// Ported from NJOY2016 `src/groupr.f90` (git commit ac5adf5f33d893e42f2eed7fb286b0d51c7580da).
// NJOY2016 is under a modified BSD 3-Clause (LANL/DOE) licence, GPL-compatible;
// this derivative file is distributed under GPL-3.0-only. This is a modified,
// non-LANL version, not endorsed by LANL/DOE. See crate root LICENSE.njoy + NOTICE.

//! File-4 angular distributions as Legendre coefficients at any incident
//! energy — `getfle` (`groupr.f90:9679-9873`) and `getco` (`:10462-10705`).
//!
//! GROUPR's two-body feed (`getdis`) needs, at each incident energy `E`, the
//! centre-of-mass Legendre coefficients `fle(1..nle)` of the scattering
//! cosine distribution, with `fle(1) = 1` for a normalised distribution.
//! `getfle` brackets `E` between two tabulated incident energies and
//! interpolates coefficient by coefficient with the TAB2 law of the upper
//! point; `getco` turns each raw record into coefficients:
//!
//! * `LTT = 1` (Legendre, `getco:10547-10560`): `fl(1) = 1`,
//!   `fl(il) = a_{il-1}`, trimmed to the last non-zero coefficient;
//! * `LTT = 2` (tabulated, `getco:10578-10700`): a 64-point Gauss-Legendre
//!   quadrature of `f(mu) P_l(mu)` with NJOY's own node/weight tables, each
//!   coefficient above the first rounded to `1e-8` (`toler`); `fl(1)` is the
//!   quadrature of `f` itself and is **not** renormalised, exactly as
//!   upstream leaves it;
//! * `LTT = 3`: the Legendre TAB2 followed by the tabulated one; `getfle`
//!   hands over at the end of the first (`:9780-9800`).
//!
//! **The slide at label 210 (`:9789-9793`) copies only `nhi` entries**:
//! `do i=1,nhi: flo(i)=fhi(i)` moves the upper point's first `nhi = nlz`
//! coefficients down, so every `flo(k)` with `k > nlz` keeps whatever the
//! last point that *had* a `k`-th coefficient left there — `getco` cleans
//! `fhi(1..nle)` on every read but nothing ever cleans `flo`. In a bracket
//! whose lower point has fewer coefficients than its upper one (`nlmax =
//! max(nlo, nhi)`), the missing low-side coefficients are therefore the
//! stale ones, not zero. This port reproduces that deliberately
//! ([`File4Angular::from_tape`] precomputes the carried `flo` per bracket):
//! measured 2026-09-10 on U-238 MF=4/MT=52, where `a3` first appears at
//! 1.06 MeV after being absent since 640 keV, the GROUPR P3 transfer from
//! group 26 to group 25 is `-8.760e-5` in NJOY, `-9.469e-5` with clean
//! zeros (the converged value is `-9.48e-5`), and `-8.7605e-5` with the
//! carried `flo` (`tests/groupr_u238_inelastic_matrix_golden.rs`).
//!
//! Below the first tabulated energy the distribution is isotropic
//! (`:9840-9847`); above the last, upstream extrapolates up to `1.01 E_last`
//! (`over`, `:9757`) and aborts beyond. Only the frame the data is given in
//! is supported (`lcd == lct`; `getdis` asks for the CM frame and File 4
//! two-body data is CM), as upstream's lab-to-CM path is "not coded"
//! (`:10685-10686`).

use crate::endf::interp::{eval_tab1, terp1, IntLaw};
use crate::endf::records::SectionCursor;
use crate::endf::tape::Tape;
use crate::groupr::kinematics::legndr;
use crate::NjoyError;

/// `nld` in `getdis` (`groupr.f90:9412`): the coefficient count requested
/// from `getfle`.
pub const NLD: usize = 21;

/// `toler` (`getco:10530`): coefficients from the tabulated quadrature are
/// rounded to this.
const TOLER: f64 = 1.0e-8;
/// `small` (`getfle:9700`).
const SMALL: f64 = 1.0e-10;
/// `over` (`getfle:9701`): extrapolation allowed to this factor above the
/// last tabulated energy.
const OVER: f64 = 1.01;

/// NJOY's 64-point Gauss-Legendre nodes (`getco:10475-10497`).
const QP64: [f64; 32] = [
    9.99305042e-01,
    9.96340117e-01,
    9.91013371e-01,
    9.83336254e-01,
    9.73326828e-01,
    9.61008800e-01,
    9.46411375e-01,
    9.29569172e-01,
    9.10522137e-01,
    8.89315446e-01,
    8.65999398e-01,
    8.40629296e-01,
    8.13265315e-01,
    7.83972359e-01,
    7.52819907e-01,
    7.19881850e-01,
    6.85236313e-01,
    6.48965471e-01,
    6.11155355e-01,
    5.71895646e-01,
    5.31279464e-01,
    4.89403146e-01,
    4.46366017e-01,
    4.02270158e-01,
    3.57220158e-01,
    3.11322872e-01,
    2.64687162e-01,
    2.17423644e-01,
    1.69644420e-01,
    1.21462819e-01,
    7.29931218e-02,
    2.43502927e-02,
];
/// NJOY's 64-point Gauss-Legendre weights (`getco:10498-10520`).
const QW64: [f64; 32] = [
    1.78328072e-03,
    4.14703326e-03,
    6.50445797e-03,
    8.84675983e-03,
    1.11681395e-02,
    1.34630479e-02,
    1.57260305e-02,
    1.79517158e-02,
    2.01348232e-02,
    2.22701738e-02,
    2.43527026e-02,
    2.63774697e-02,
    2.83396726e-02,
    3.02346571e-02,
    3.20579284e-02,
    3.38051618e-02,
    3.54722133e-02,
    3.70551285e-02,
    3.85501532e-02,
    3.99537411e-02,
    4.12625632e-02,
    4.24735151e-02,
    4.35837245e-02,
    4.45905582e-02,
    4.54916279e-02,
    4.62847966e-02,
    4.69681828e-02,
    4.75401657e-02,
    4.79993886e-02,
    4.83447622e-02,
    4.85754674e-02,
    4.86909570e-02,
];

/// The 64 (node, weight) pairs in upstream order (`-qp(32) .. -qp(1), qp(1)
/// .. qp(32)` mirrored with the same weights).
fn gauss64() -> impl Iterator<Item = (f64, f64)> {
    (0..64).map(|i| {
        if i < 32 {
            (-QP64[i], QW64[i])
        } else {
            (QP64[63 - i], QW64[63 - i])
        }
    })
}

/// One tabulated incident energy of a File-4 section: its CM Legendre
/// coefficients (`fl(1) = 1` for Legendre data) and the TAB2 interpolation
/// law that applies to the panel ending at this point.
#[derive(Debug, Clone, PartialEq)]
struct EnergyPoint {
    energy: f64,
    /// `fl(1..nl)` trimmed to the last non-zero coefficient (`nlz`).
    fl: Vec<f64>,
    /// Interpolation law for `[previous energy, this energy]`
    /// (`int = fls(6+2*ir)` after `nne` advances, `getfle:9807-9808`).
    law: u32,
    /// The record's L1: `LT` of an MF=4 LIST, `LANG` of a File-6 `LAW = 2` LIST
    /// (checked by [`File4Angular::from_mf6_law2`]); 0 for a TAB1.
    lang: i32,
}

/// A File-4 angular distribution ready to answer `getfle` queries.
#[derive(Debug, Clone, PartialEq)]
pub struct File4Angular {
    /// `LCT` — frame of the data (1 lab, 2 CM).
    pub lct: i32,
    /// `LI` — 1 when the section declares isotropy (no records follow).
    pub isotropic: bool,
    points: Vec<EnergyPoint>,
    /// `flo` as `getfle` holds it while `e` lies in the bracket whose lower
    /// point is `points[j]` (`lo_fl[j]`, `nl_max` entries): the clean
    /// `getco` array for `j = 0`, then the label-210 slide's partial copy
    /// (`:9789-9793`) — see the module doc.
    lo_fl: Vec<Vec<f64>>,
    /// HEATR's `hgtfle` rule (`heatr.f90:4364-4367`): a log law (4 or 5)
    /// falls back to its linear partner when the two coefficients differ in
    /// sign. GROUPR's `getfle` has no such rule.
    log_sign_fallback: bool,
}

/// How [`File4Angular::from_tape_with`] reads a section: GROUPR's `getfle` /
/// `getco` ([`File4Options::GROUPR`]) or HEATR's `hgtfle` / `hgetco`
/// ([`File4Options::HEATR`]), which are the same routine with three
/// differences.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct File4Options {
    /// Rounding of tabulated-record coefficients: `1e-8` in `getco`
    /// (`groupr.f90:10530`), `1e-6` in `hgetco` (`heatr.f90:4474`).
    pub toler: f64,
    /// Accept a lab-frame (`LCT = 1`) section as if it were CM. `hgtfle`
    /// does this for MT=51-90 when `AWR >= 10` (`heatr.f90:4252-4260`);
    /// both routines otherwise refuse lab data.
    pub lab_as_cm: bool,
    /// `hgtfle`'s log-law sign fallback (see the field of the same name).
    pub log_sign_fallback: bool,
    /// `getco` with `lcd = 1` (lab wanted) on CM data (`LCT = 2`): every record
    /// goes through the 64-point quadrature with `xn = (1+awr x)/sqrt(1+awr^2+2 awr x)`
    /// (`groupr.f90:10550`, `:10686`). `getff` label 200 asks for this.
    pub cm_to_lab: bool,
}

impl File4Options {
    /// GROUPR's `getfle`.
    pub const GROUPR: File4Options = File4Options { toler: TOLER, lab_as_cm: false, log_sign_fallback: false, cm_to_lab: false };
    /// HEATR's `hgtfle`, with lab data refused (set `lab_as_cm` per reaction).
    pub const HEATR: File4Options = File4Options { toler: 1.0e-6, lab_as_cm: false, log_sign_fallback: true, cm_to_lab: false };
}

/// `getfle`'s answer at one energy.
#[derive(Debug, Clone, PartialEq)]
pub struct LegendreAt {
    /// `fle(1..nle)`; `fle[0]` is the zeroth coefficient (1 for Legendre
    /// data, the quadrature of `f` for tabulated data).
    pub fle: Vec<f64>,
    /// `nle` — number of meaningful coefficients (1 when isotropic).
    pub nle: usize,
    /// `enext` — the next energy at which the answer may change.
    pub enext: f64,
    /// `idis` — 1 when `enext` is a discontinuity (histogram law, or the
    /// isotropic region below the first point).
    pub idis: bool,
}

/// `getco` for `LTT = 1` (`groupr.f90:10547-10560`): Legendre coefficients
/// `a_1..a_NL` straight from the LIST, `fl(1) = 1`, trimmed to `nlz`.
fn getco_legendre(a: &[f64], nl_max: usize) -> Vec<f64> {
    let np = a.len() + 1;
    let mut fl = vec![0.0f64; nl_max];
    fl[0] = 1.0;
    let mut nlz = 1;
    for il in 2..=nl_max {
        if il <= np {
            fl[il - 1] = a[il - 2];
            if fl[il - 1] != 0.0 {
                nlz = il;
            }
        }
    }
    fl.truncate(nlz);
    fl
}

/// `getco` for `LTT = 2` (`groupr.f90:10578-10700`): 64-point quadrature of
/// the tabulated `f(mu)` against `P_l`, `toler` rounding, trimmed to `nlz`.
fn getco_tabulated(
    interp: &[(u32, u32)],
    pairs: &[(f64, f64)],
    nl_max: usize,
    toler: f64,
    awr_lab: Option<f64>,
) -> Result<Vec<f64>, NjoyError> {
    let l = nl_max - 1;
    let mut fl = vec![0.0f64; nl_max];
    for (x, w) in gauss64() {
        // terpa(y, x, ...): the TAB1's own laws; 0 outside its range.
        let y = if x < pairs[0].0 || x > pairs[pairs.len() - 1].0 {
            0.0
        } else {
            eval_tab1(x, interp, pairs)?
        };
        let p = legndr(cm_to_lab_cosine(x, awr_lab), l);
        let yw = y * w;
        for il in 0..nl_max {
            fl[il] += yw * p[il];
        }
    }
    let mut nlz = 1;
    for il in 2..=nl_max {
        let j = (fl[il - 1] / toler).round();
        fl[il - 1] = j * toler;
        if j != 0.0 {
            nlz = il;
        }
    }
    fl.truncate(nlz);
    Ok(fl)
}

/// `getco` label 370 (`groupr.f90:10684-10686`): the cosine the Legendre
/// polynomials are taken at, `x` itself, or for CM data wanted in the lab
/// (`awr_lab = Some(awr)`) `(1 + awr x) / sqrt(1 + awr^2 + 2 awr x)`.
fn cm_to_lab_cosine(x: f64, awr_lab: Option<f64>) -> f64 {
    match awr_lab {
        Some(awr) => (1.0 + awr * x) / (1.0 + awr * awr + 2.0 * awr * x).sqrt(),
        None => x,
    }
}

/// `getco` label 300 for Legendre data (`LTT = 1`) wanted in another frame
/// (`groupr.f90:10603-10612`, `:10684-10700`): `y = 1/2 + sum (2l+1)/2 a_l P_l(x)`
/// at the 64 Gauss points, projected on `P_l(xn)`, `toler` rounding, trimmed.
fn getco_legendre_converted(a: &[f64], nl_max: usize, toler: f64, awr: f64) -> Vec<f64> {
    let l = nl_max - 1;
    let np = a.len();
    let mut fl = vec![0.0f64; nl_max];
    for (x, w) in gauss64() {
        let pr = legndr(x, np);
        let mut y = 1.0;
        y /= 2.0;
        for ip in 1..=np {
            y += (2 * ip + 1) as f64 * pr[ip] * a[ip - 1] / 2.0;
        }
        let p = legndr(cm_to_lab_cosine(x, Some(awr)), l);
        let yw = y * w;
        for il in 0..nl_max {
            fl[il] += yw * p[il];
        }
    }
    let mut nlz = 1;
    for il in 2..=nl_max {
        let j = (fl[il - 1] / toler).round();
        fl[il - 1] = j * toler;
        if j != 0.0 {
            nlz = il;
        }
    }
    fl.truncate(nlz);
    fl
}

/// `getco` with `lang = 2` and `lcd = lct` (`groupr.f90:10563-10573`): `np = c(5)`
/// (the LIST's `NW`), `fl(il) = c(il+6)` for `il <= np`, else 0, for `il = 1..nl`;
/// `nl = nlz`, the last non-zero (at least 1). `a_0` is not renormalised.
fn getco_law5_legendre(c: &[f64], np: usize, nl_max: usize) -> Vec<f64> {
    let mut fl = vec![0.0f64; nl_max];
    let mut nlz = 1;
    for il in 1..=nl_max {
        if il <= np {
            fl[il - 1] = c.get(il - 1).copied().unwrap_or(0.0);
            if fl[il - 1] != 0.0 {
                nlz = il;
            }
        }
    }
    fl.truncate(nlz);
    fl
}

/// Read one TAB2's worth of angular records (`ne` of them, each a LIST for
/// `ltt = 1` or a TAB1 for `ltt = 2`) into [`EnergyPoint`]s. `awr_lab` is
/// `Some(awr)` when CM data is wanted in the lab ([`File4Options::cm_to_lab`]).
fn read_records(
    cur: &mut SectionCursor<'_>,
    ltt: i32,
    nl_max: usize,
    toler: f64,
    awr_lab: Option<f64>,
    out: &mut Vec<EnergyPoint>,
) -> Result<(), NjoyError> {
    let tab2 = cur.read_tab2()?;
    let ne = tab2.head.n2 as usize;
    let regions = &tab2.interp;
    let mut ir = 0usize;
    for nne in 1..=ne {
        // `if (nne.gt.fls(5+2*ir)) ir=ir+1` — the region whose NBT covers nne.
        while ir + 1 < regions.len() && nne as u32 > regions[ir].0 {
            ir += 1;
        }
        let law = regions.get(ir).map(|r| r.1).unwrap_or(2);
        let point = match ltt {
            1 => {
                let list = cur.read_list()?;
                EnergyPoint {
                    energy: list.head.c2,
                    fl: match awr_lab {
                        Some(awr) => getco_legendre_converted(&list.data, nl_max, toler, awr),
                        None => getco_legendre(&list.data, nl_max),
                    },
                    law,
                    lang: list.head.l1,
                }
            }
            2 => {
                let t = cur.read_tab1()?;
                EnergyPoint {
                    energy: t.head.c2,
                    fl: getco_tabulated(&t.interp, &t.pairs, nl_max, toler, awr_lab)?,
                    law,
                    lang: 0,
                }
            }
            other => {
                return Err(NjoyError::EndfParse(format!(
                    "getfle: unsupported LTT={other} record"
                )))
            }
        };
        out.push(point);
    }
    Ok(())
}

/// The `flo` array `getfle` holds for each bracket, replayed in order:
/// `getco(points[0])` padded with zeros to `nl_max` (`:9722-9727`), then for
/// every later point the label-210 slide `do i=1,nhi: flo(i)=fhi(i)` with
/// `nhi = nlz` of that point (`:9789-9793`) — entries above `nlz` are left
/// as they were.
fn carried_lo(points: &[EnergyPoint], nl_max: usize) -> Vec<Vec<f64>> {
    let mut out = Vec::with_capacity(points.len());
    let mut flo = vec![0.0f64; nl_max];
    for (j, p) in points.iter().enumerate() {
        if j == 0 {
            flo[..p.fl.len()].copy_from_slice(&p.fl);
            for v in flo.iter_mut().skip(p.fl.len()) {
                *v = 0.0;
            }
        } else {
            flo[..p.fl.len()].copy_from_slice(&p.fl);
        }
        out.push(flo.clone());
    }
    out
}

impl File4Angular {
    /// Read `MF=4/MT=mt` of `mat` and convert every record with `getco`
    /// (`getfle` initialisation, `groupr.f90:9705-9760`, for all energies
    /// rather than the first two). `nl_max` is the coefficient count asked
    /// for (`nld = 21` in `getdis`).
    ///
    /// # Errors
    /// [`NjoyError::SectionNotFound`] when the section is absent;
    /// [`NjoyError::EndfParse`] for `LTT` outside `0..=3`, or a lab-frame
    /// distribution (`LCT = 1`), which upstream cannot convert to CM either.
    pub fn from_tape(tape: &Tape, mat: i32, mt: i32, nl_max: usize) -> Result<Self, NjoyError> {
        Self::from_tape_with(tape, mat, mt, nl_max, &File4Options::GROUPR)
    }

    /// `getfle` with `lcd = 1` (the lab frame), as `getff` label 200 asks for it
    /// (`groupr.f90:7152-7153`): lab data (`LCT = 1`) is read as it is, CM data
    /// (`LCT = 2`, unless `LI = 1`) goes through `getco`'s 64-point quadrature
    /// with the CM-to-lab cosine (`:10550`, `:10686`), so a CM-isotropic table
    /// gains lab moments.
    ///
    /// # Errors
    /// As [`File4Angular::from_tape_with`].
    pub fn from_tape_lab(tape: &Tape, mat: i32, mt: i32, nl_max: usize) -> Result<Self, NjoyError> {
        let opts = File4Options { lab_as_cm: true, cm_to_lab: true, ..File4Options::GROUPR };
        Self::from_tape_with(tape, mat, mt, nl_max, &opts)
    }

    /// [`File4Angular::from_tape`] with the reading rules of `opts`, so
    /// HEATR's `hgtfle` reuses this port instead of a second one.
    ///
    /// # Errors
    /// As [`File4Angular::from_tape`]; a lab-frame section is refused unless
    /// `opts.lab_as_cm`.
    pub fn from_tape_with(
        tape: &Tape,
        mat: i32,
        mt: i32,
        nl_max: usize,
        opts: &File4Options,
    ) -> Result<Self, NjoyError> {
        let toler = opts.toler;
        let sec = tape
            .section(mat, 4, mt)
            .ok_or(NjoyError::SectionNotFound { mat, mf: 4, mt })?;
        let mut cur = SectionCursor::new(&sec.rows);
        let head = cur.read_cont()?;
        let lvt = head.l1;
        let ltt = head.l2;
        let second = if lvt == 0 {
            cur.read_cont()?
        } else {
            cur.read_list()?.head
        };
        let li = second.l1;
        let lct = second.l2;
        let mut points = Vec::new();
        // `getco` with `lcd = 1` on CM data: `xn` from `awr = fls(2)`, the HEAD's C2 (`:9713`).
        let awr_lab = (opts.cm_to_lab && lct == 2).then_some(head.c2);
        if li != 1 {
            if lct == 1 && !opts.lab_as_cm {
                // getco:10685 — "lab to cm conversion not coded".
                return Err(NjoyError::EndfParse(format!(
                    "getfle: MF=4/MT={mt} of mat {mat} is a lab-frame distribution (LCT=1); \
                     upstream cannot convert it to CM either"
                )));
            }
            match ltt {
                1 | 2 => read_records(&mut cur, ltt, nl_max, toler, awr_lab, &mut points)?,
                3 => {
                    read_records(&mut cur, 1, nl_max, toler, awr_lab, &mut points)?;
                    read_records(&mut cur, 2, nl_max, toler, awr_lab, &mut points)?;
                }
                other => {
                    return Err(NjoyError::EndfParse(format!(
                        "getfle: MF=4/MT={mt} of mat {mat}: LTT={other} not supported"
                    )))
                }
            }
        }
        // getfle:9758 — two points that are both isotropic mean isotropy.
        let isotropic = li == 1 || (points.len() == 2 && points.iter().all(|p| p.fl.len() == 1));
        let lo_fl = carried_lo(&points, nl_max);
        Ok(Self {
            lct,
            isotropic,
            points,
            lo_fl,
            log_sign_fallback: opts.log_sign_fallback,
        })
    }

    /// `getfle` with `mfd = 8` on a File-6 `LAW = 2` subsection (gh:#1370): the
    /// subsection of `MF=6/MT=mt` whose `ZAP` is `zap` (the first, as `getmf6`'s
    /// search, `groupr.f90:7588-7615`), its TAB2 and one LIST per incident energy
    /// `[0, E, LANG, 0, NW, NL]` read with `ltt = 1` (`:9733-9760`), each through
    /// `getco`'s Legendre branch. The frame is the MF=6 HEAD's `LCT` (`getmf6`,
    /// `:7581`).
    ///
    /// # Errors
    /// [`NjoyError::SectionNotFound`] without the section; [`NjoyError::NotPorted`],
    /// by name, for `LANG /= 0` (`getco` would read a tabulated record as Legendre
    /// coefficients), for `LCT /= 2` (upstream rewrites `LCT = 1` to CM for
    /// MT=51-90 with `AWR >= 10`, and reads `LCT = 3` per particle; neither is
    /// translated), and for a subsection that is not `LAW = 2`;
    /// [`NjoyError::EndfParse`] when no subsection has that `ZAP`.
    pub fn from_mf6_law2(tape: &Tape, mat: i32, mt: i32, zap: i32, nl_max: usize) -> Result<Self, NjoyError> {
        let sec = tape.section(mat, 6, mt).ok_or(NjoyError::SectionNotFound { mat, mf: 6, mt })?;
        let mut cur = SectionCursor::new(&sec.rows);
        let head = cur.read_cont()?;
        let lct = head.l2;
        if lct != 2 {
            return Err(NjoyError::NotPorted("groupr getfle mfd = 8: File-6 LAW = 2 with LCT /= 2 (getmf6's LCT = 1/3 handling)"));
        }
        for _ in 0..head.n1.max(0) {
            let t = cur.read_tab1()?;
            let law = t.head.l2;
            if t.head.c1.round() as i32 != zap {
                crate::acer::energy::mf6::skip_mf6_subsection(&mut cur, law)?;
                continue;
            }
            if law != 2 {
                return Err(NjoyError::NotPorted("groupr getfle mfd = 8: File-6 subsection is not LAW = 2"));
            }
            let mut points = Vec::new();
            read_records(&mut cur, 1, nl_max, TOLER, None, &mut points)?;
            if points.iter().any(|p| p.lang != 0) {
                return Err(NjoyError::NotPorted("groupr getfle mfd = 8: File-6 LAW = 2 with LANG /= 0 (getco reads the record as Legendre coefficients)"));
            }
            let isotropic = points.len() == 2 && points.iter().all(|p| p.fl.len() == 1);
            let lo_fl = carried_lo(&points, nl_max);
            return Ok(Self { lct, isotropic, points, lo_fl, log_sign_fallback: false });
        }
        Err(NjoyError::EndfParse(format!("getfle: MF=6/MT={mt} of mat {mat} has no subsection with ZAP = {zap}")))
    }

    /// `getmf6`'s `LAW = 4` (discrete two-body recoil) for the subsection whose
    /// `ZAP` is `zap` (gh:#1370, `groupr.f90:7636-7653`): upstream backs up to the
    /// **first** subsection of `MF=6/MT=mt` ("assumed to be the first subsection"),
    /// re-reads its TAB1 and lets `getfle` (`mfd = 8`) read that subsection's angular
    /// records. When the first subsection is `LAW = 3`, `law = -4` and `getfle` makes
    /// the distribution CM-isotropic (`:9733-9734`); when it is `LAW = 2`, its
    /// Legendre records are read as [`File4Angular::from_mf6_law2`] reads them. The
    /// caller must also flip the CM cosine in `getdis` (`if (law.eq.4) wqp=-wqp`,
    /// `:9519`): [`crate::groupr::two_body::TwoBodyFeed::with_law4_cosine_flip`].
    ///
    /// # Errors
    /// [`NjoyError::SectionNotFound`] without the section; [`NjoyError::NotPorted`],
    /// by name, when the `zap` subsection is not `LAW = 4`; when the first subsection
    /// is neither `LAW = 2` nor `LAW = 3` (upstream's `getfle` would read its records
    /// as `LAW = 2` Legendre lists, which they are not: undefined); when the `zap`
    /// subsection's `AWP` is not 1 (the port's `getdis` holds `aprime = 1`); for
    /// `LANG /= 0` and `LCT /= 2` as in `from_mf6_law2`.
    pub fn from_mf6_law4(tape: &Tape, mat: i32, mt: i32, zap: i32, nl_max: usize) -> Result<Self, NjoyError> {
        let sec = tape.section(mat, 6, mt).ok_or(NjoyError::SectionNotFound { mat, mf: 6, mt })?;
        let mut cur = SectionCursor::new(&sec.rows);
        let head = cur.read_cont()?;
        let mut first_law = None;
        let mut found = None;
        for _ in 0..head.n1.max(0) {
            let t = cur.read_tab1()?;
            let law = t.head.l2;
            first_law.get_or_insert(law);
            if t.head.c1.round() as i32 == zap {
                found = Some((law, t.head.c2));
                break;
            }
            crate::acer::energy::mf6::skip_mf6_subsection(&mut cur, law)?;
        }
        let Some((law, awp)) = found else {
            return Err(NjoyError::EndfParse(format!("getmf6: MF=6/MT={mt} of mat {mat} has no subsection with ZAP = {zap}")));
        };
        if law != 4 {
            return Err(NjoyError::NotPorted("groupr getmf6 law 4: the requested File-6 subsection is not LAW = 4"));
        }
        if awp != 1.0 {
            return Err(NjoyError::NotPorted("groupr getmf6 law 4: AWP /= 1 for the outgoing neutron (getdis aprime = awp/awrp)"));
        }
        if head.l2 != 2 {
            return Err(NjoyError::NotPorted("groupr getfle mfd = 8: File-6 LAW = 4 with LCT /= 2 (getmf6's LCT = 1/3 handling)"));
        }
        match first_law {
            Some(2) => {
                // `tmp(l)=zap`, `tmp(l+1)=awp` only relabel the re-read TAB1; `getfle`
                // then reads the first subsection's TAB2 and LISTs.
                let mut cur = SectionCursor::new(&sec.rows);
                let _ = cur.read_cont()?;
                let _ = cur.read_tab1()?;
                let mut points = Vec::new();
                read_records(&mut cur, 1, nl_max, TOLER, None, &mut points)?;
                if points.iter().any(|p| p.lang != 0) {
                    return Err(NjoyError::NotPorted("groupr getfle mfd = 8: File-6 LAW = 4 partner with LANG /= 0 (getco reads the record as Legendre coefficients)"));
                }
                let isotropic = points.len() == 2 && points.iter().all(|p| p.fl.len() == 1);
                let lo_fl = carried_lo(&points, nl_max);
                Ok(Self { lct: 2, isotropic, points, lo_fl, log_sign_fallback: false })
            }
            // `if (lf.eq.3) law=-4`, then `getfle` sets `iso = 1` (`:9734`).
            Some(3) => Ok(Self::isotropic_cm()),
            _ => Err(NjoyError::NotPorted("groupr getmf6 law 4: first File-6 subsection is not LAW = 2 or 3 (getfle would misread it)")),
        }
    }

    /// `getfle` with `mfd = 8` on a File-6 `LAW = 5` subsection for a **neutron**
    /// (gh:#1370, `groupr.f90:9733-9760` with `getco`, `:10462-10704`). The TAB2 is
    /// `[SPI, 0, LIDP, 0, NR, NE]`; upstream stores `LIDP` into each LIST's L2
    /// (`fls(iraw+3)=lidp`), which only `getco`'s Coulomb branch reads. Each LIST
    /// `[0, E, LTP, 0, NW, NL]` with `LTP = 2` (nuclear Legendre coefficients
    /// `a_0..a_NL`) goes through `getco` with `lang = 2` (`l1h`) and `lcd = lct = 2`:
    /// `fl(il) = c(il+6)` for `il <= NW` (`:10563-10573`); `a_0` is kept as given (no
    /// `fl(1) = 1`), trimmed to the last non-zero, unrounded. GROUPR's `conver`
    /// copies such a record to its scratch tape unchanged (`:11590-11596`).
    ///
    /// For a neutron (`izap = 1`) `getdis` adds no Coulomb term (`:9529`, `mtd = 2`
    /// and `izap > 1` only) and keeps `wmin = -1`, `wmax = 1` (`:9449-9452`), so
    /// `SPI` and `LIDP` have no effect.
    ///
    /// # Errors
    /// [`NjoyError::SectionNotFound`] without the section; [`NjoyError::NotPorted`]
    /// by name for: `LCT /= 2`; a subsection that is not `LAW = 5`; `LTP = 1`
    /// (`getco` label 360 jumps to 370 for `izap <= 1` with `y` never set, so
    /// upstream uses an uninitialised value: undefined); and `LTP > 2`. For
    /// `LTP > 2` upstream's `conver` (`:11597-11621`) rewrites the record on its
    /// scratch tape as `LTP = 2` after multiplying `p(mu)` by `sig(E)*(1-mu)`, with
    /// `sig` from a `getsig` call that reads the PENDF and leaves the ENDF header
    /// globals (`l1h`, `n2h`) at that tape's last record; `nm = n2h` and `getco`'s
    /// `lang = l1h`, `nmu = n2h` are then taken from it. Measured 2026-11-14 with an
    /// instrumented NJOY2016.80 on a synthetic Si-30 tape (gh:#1370, MT 54 `LTP = 12`
    /// and MT 55 `LTP = 14`, three records each): the first two converted records of
    /// each section read back as `NW = 1`, `a_0 = 0`, and `getco`'s tabulated branch
    /// ran only for the first record of each section. That is not a defined computation, so it
    /// is refused rather than reproduced.
    pub fn from_mf6_law5(tape: &Tape, mat: i32, mt: i32, zap: i32, nl_max: usize) -> Result<Self, NjoyError> {
        let sec = tape.section(mat, 6, mt).ok_or(NjoyError::SectionNotFound { mat, mf: 6, mt })?;
        let mut cur = SectionCursor::new(&sec.rows);
        let head = cur.read_cont()?;
        if head.l2 != 2 {
            return Err(NjoyError::NotPorted("groupr getfle mfd = 8: File-6 LAW = 5 with LCT /= 2 (getmf6's LCT = 1/3 handling)"));
        }
        for _ in 0..head.n1.max(0) {
            let t = cur.read_tab1()?;
            let law = t.head.l2;
            if t.head.c1.round() as i32 != zap {
                crate::acer::energy::mf6::skip_mf6_subsection(&mut cur, law)?;
                continue;
            }
            if law != 5 {
                return Err(NjoyError::NotPorted("groupr getfle mfd = 8: File-6 subsection is not LAW = 5"));
            }
            let tab2 = cur.read_tab2()?;
            let ne = tab2.head.n2.max(0) as usize;
            let regions = &tab2.interp;
            let mut ir = 0usize;
            let mut points = Vec::with_capacity(ne);
            for nne in 1..=ne {
                while ir + 1 < regions.len() && nne as u32 > regions[ir].0 {
                    ir += 1;
                }
                let elaw = regions.get(ir).map(|r| r.1).unwrap_or(2);
                let list = cur.read_list()?;
                let ltp = list.head.l1;
                let fl = match ltp {
                    1 => return Err(NjoyError::NotPorted("groupr getco: File-6 LAW = 5 LTP = 1 for a neutron (label 360 goes to 370 with y unset: undefined upstream)")),
                    2 => getco_law5_legendre(&list.data, list.head.n1.max(0) as usize, nl_max),
                    _ => return Err(NjoyError::NotPorted("groupr conver: File-6 LAW = 5 LTP > 2 for a neutron (conver's getsig leaves l1h/n2h stale: undefined upstream)")),
                };
                points.push(EnergyPoint { energy: list.head.c2, fl, law: elaw, lang: ltp });
            }
            let isotropic = points.len() == 2 && points.iter().all(|p| p.fl.len() == 1);
            let lo_fl = carried_lo(&points, nl_max);
            return Ok(Self { lct: 2, isotropic, points, lo_fl, log_sign_fallback: false });
        }
        Err(NjoyError::EndfParse(format!("getfle: MF=6/MT={mt} of mat {mat} has no subsection with ZAP = {zap}")))
    }

    /// An isotropic centre-of-mass distribution with no records: what `getfle`
    /// gives for File-6 two-body data with `|law| = 3` or `law = -4` (`mfd = 8`,
    /// `iso = 1`, `groupr.f90:9733-9737`, then label 440 on every call).
    pub fn isotropic_cm() -> Self {
        Self { lct: 2, isotropic: true, points: Vec::new(), lo_fl: Vec::new(), log_sign_fallback: false }
    }

    /// Number of tabulated incident energies.
    pub fn len(&self) -> usize {
        self.points.len()
    }

    /// True when no records were read.
    pub fn is_empty(&self) -> bool {
        self.points.is_empty()
    }

    /// The tabulated incident energies \[eV\].
    pub fn energies(&self) -> Vec<f64> {
        self.points.iter().map(|p| p.energy).collect()
    }

    /// The first tabulated energy — `getfle`'s initial `enext`
    /// (`groupr.f90:9762`); `1e10` when isotropic.
    pub fn first_energy(&self) -> f64 {
        if self.isotropic || self.points.is_empty() {
            1.0e10
        } else {
            self.points[0].energy
        }
    }

    /// `getfle` at `e > 0` (`groupr.f90:9767-9873`): `nle` coefficients.
    ///
    /// # Errors
    /// [`NjoyError::EndfParse`] above `1.01 x` the last tabulated energy
    /// ("desired energy above highest given").
    pub fn coefficients_at(&self, e: f64, nle: usize) -> Result<LegendreAt, NjoyError> {
        let iso = |enext: f64, idis: bool| {
            let mut fle = vec![0.0f64; nle.max(1)];
            fle[0] = 1.0;
            LegendreAt {
                fle,
                nle: 1,
                enext,
                idis,
            }
        };
        // Label 440: isotropic distribution or request.
        if nle == 1 || self.isotropic || self.points.is_empty() {
            return Ok(iso(1.0e10, false));
        }
        let pts = &self.points;
        // Bracket: the upper point is the first energy with e < ehi*(1-small)
        // (`:9770`); at the end of the table, up to `over*ehi` is allowed.
        let mut hi = 1usize;
        while hi + 1 < pts.len() && e >= pts[hi].energy * (1.0 - SMALL) {
            hi += 1;
        }
        let (elo, ehi) = (pts[hi - 1].energy, pts[hi].energy);
        if e >= ehi * (1.0 - SMALL) && e >= OVER * ehi {
            return Err(NjoyError::EndfParse(format!(
                "getfle: desired energy {e} above highest given ({ehi})"
            )));
        }
        // Label 300. `flo` is the carried array (`:9789-9793`), `nlo` the
        // lower point's own count (`nlo = nhi` at the slide).
        if e >= elo * (1.0 - SMALL) {
            let (flo, fhi) = (&self.lo_fl[hi - 1], &pts[hi].fl);
            let nlmax = pts[hi - 1].fl.len().max(fhi.len());
            let mut fle = vec![0.0f64; nle];
            for (i, slot) in fle.iter_mut().enumerate().take(nlmax.min(nle)) {
                let ylo = flo.get(i).copied().unwrap_or(0.0);
                let yhi = fhi.get(i).copied().unwrap_or(0.0);
                let mut code = pts[hi].law;
                if self.log_sign_fallback && (code == 4 || code == 5) && ylo * yhi <= 0.0 {
                    code -= 2;
                }
                *slot = terp1(elo, ylo, ehi, yhi, e, IntLaw::from_code(code))?;
            }
            Ok(LegendreAt {
                fle,
                nle: nlmax.min(nle),
                enext: ehi,
                idis: pts[hi].law == 1,
            })
        } else {
            // Below the first point: isotropic, enext = ehi, idis = 1.
            Ok(iso(ehi, true))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::endf::tape::{Section, Tape};
    use crate::endf::EndfKey;

    fn pack(values: &[f64]) -> Vec<[f64; 6]> {
        values
            .chunks(6)
            .map(|c| {
                let mut r = [0.0f64; 6];
                r[..c.len()].copy_from_slice(c);
                r
            })
            .collect()
    }

    /// An MF=4/MT=2 section: LTT=1 Legendre at 1 eV (a1 = 0.2) and 1 MeV
    /// (a1 = 0.4, a2 = 0.1), CM frame, lin-lin in energy.
    fn legendre_tape() -> Tape {
        let mut rows = vec![
            [92238.0, 236.0, 0.0, 1.0, 0.0, 0.0],
            [0.0, 236.0, 0.0, 2.0, 0.0, 0.0],
            [0.0, 0.0, 0.0, 0.0, 1.0, 2.0],
            [2.0, 2.0, 0.0, 0.0, 0.0, 0.0],
            [0.0, 1.0, 0.0, 0.0, 1.0, 0.0],
            [0.2, 0.0, 0.0, 0.0, 0.0, 0.0],
            [0.0, 1.0e6, 0.0, 0.0, 2.0, 0.0],
        ];
        rows.extend(pack(&[0.4, 0.1]));
        Tape::from_sections(
            String::new(),
            vec![Section {
                key: EndfKey {
                    mat: 9237,
                    mf: 4,
                    mt: 2,
                },
                rows,
            }],
        )
    }

    /// Methodology: `getco` for `LTT=1` keeps `fl(1)=1` and copies `a_l`
    /// (`:10547-10560`); `getfle` interpolates lin-lin between the two
    /// energies and is isotropic below the first. At 1 eV: [1, 0.2, 0]; at
    /// 500 keV (halfway): a1 = 0.3, a2 = 0.05; at 0.5 eV: isotropic with
    /// `enext = 1e6`, `idis = 1`. Result (2026-09-10): exact.
    #[test]
    fn legendre_records_interpolate_and_go_isotropic_below() {
        let f = File4Angular::from_tape(&legendre_tape(), 9237, 2, NLD).unwrap();
        assert_eq!(f.lct, 2);
        assert!(!f.isotropic);
        assert_eq!(f.energies(), vec![1.0, 1.0e6]);
        // At the first point `nle = max(nlo, nhi)` (`:9822-9826`): 3, with
        // the third coefficient interpolating to 0 at `elo`.
        let at1 = f.coefficients_at(1.0, NLD).unwrap();
        assert_eq!(at1.nle, 3);
        assert!((at1.fle[0] - 1.0).abs() < 1e-15 && (at1.fle[1] - 0.2).abs() < 1e-15);
        assert_eq!(at1.fle[2], 0.0);
        assert_eq!(at1.enext, 1.0e6);
        let mid = f.coefficients_at(500_000.5, NLD).unwrap();
        assert_eq!(mid.nle, 3);
        assert!((mid.fle[1] - 0.3).abs() < 1e-6 && (mid.fle[2] - 0.05).abs() < 1e-6);
        let below = f.coefficients_at(0.5, NLD).unwrap();
        assert_eq!(below.nle, 1);
        assert_eq!(below.fle[0], 1.0);
        assert_eq!(below.enext, 1.0e6);
        assert!(below.idis);
        assert!(f.coefficients_at(1.02e6, NLD).is_err());
    }

    /// Methodology: `getco` for `LTT=2` — the tabulated `f(mu) = (1 + mu)/2`
    /// (lin-lin between (-1, 0) and (1, 1)) has exact Legendre coefficients
    /// `fl(1) = 1`, `fl(2) = 1/3`, `fl(l>2) = 0`. NJOY's 64-point rule with
    /// its 9-digit nodes integrates a linear function to ~1e-8, and the
    /// `toler` rounding then clears the higher orders. Result (2026-09-10):
    /// `fl(2)` within 2e-8 of 1/3, `nlz = 2`.
    #[test]
    fn tabulated_linear_distribution_gives_one_third() {
        let fl = getco_tabulated(&[(2, 2)], &[(-1.0, 0.0), (1.0, 1.0)], NLD, TOLER, None).unwrap();
        assert!((fl[0] - 1.0).abs() < 2e-8, "fl(1) = {}", fl[0]);
        assert_eq!(fl.len(), 2, "nlz: {fl:?}");
        assert!((fl[1] - 1.0 / 3.0).abs() < 2e-8, "fl(2) = {}", fl[1]);
    }

    /// Methodology: the label-210 slide copies only `nhi` entries
    /// (`:9789-9793`), so a bracket whose lower point lacks a coefficient
    /// its upper point has interpolates from the last point that had it.
    /// Three Legendre points: 1 eV `(a1, a2) = (0.2, 0.1)`, 10 eV `a1 =
    /// 0.3` only (`nlz = 2`), 100 eV `(0.4, 0.3)`. At 55 eV (halfway in the
    /// second bracket) upstream gives `fle(3) = terp1(10, 0.1, 100, 0.3)
    /// = 0.2`, not `terp1(10, 0, 100, 0.3) = 0.15`; in the first bracket
    /// nothing is stale. Result (2026-09-10): exact.
    #[test]
    fn slide_keeps_stale_high_order_coefficients() {
        let mut rows = vec![
            [92238.0, 236.0, 0.0, 1.0, 0.0, 0.0],
            [0.0, 236.0, 0.0, 2.0, 0.0, 0.0],
            [0.0, 0.0, 0.0, 0.0, 1.0, 3.0],
            [3.0, 2.0, 0.0, 0.0, 0.0, 0.0],
            [0.0, 1.0, 0.0, 0.0, 2.0, 0.0],
            [0.2, 0.1, 0.0, 0.0, 0.0, 0.0],
            [0.0, 10.0, 0.0, 0.0, 1.0, 0.0],
            [0.3, 0.0, 0.0, 0.0, 0.0, 0.0],
            [0.0, 100.0, 0.0, 0.0, 2.0, 0.0],
        ];
        rows.extend(pack(&[0.4, 0.3]));
        let tape = Tape::from_sections(
            String::new(),
            vec![Section {
                key: EndfKey {
                    mat: 9237,
                    mf: 4,
                    mt: 51,
                },
                rows,
            }],
        );
        let f = File4Angular::from_tape(&tape, 9237, 51, NLD).unwrap();
        let first = f.coefficients_at(5.5, NLD).unwrap();
        assert_eq!(first.nle, 3);
        assert!((first.fle[1] - 0.25).abs() < 1e-12 && (first.fle[2] - 0.05).abs() < 1e-12);
        let second = f.coefficients_at(55.0, NLD).unwrap();
        assert_eq!(second.nle, 3);
        assert!((second.fle[1] - 0.35).abs() < 1e-12, "{:?}", second.fle);
        assert!(
            (second.fle[2] - 0.2).abs() < 1e-12,
            "stale a2 expected: {:?}",
            second.fle
        );
    }

    /// Methodology: NJOY's 64-point table must integrate `1` and `mu^2`
    /// (`2` and `2/3`) — a sanity check on the transcription of `qp`/`qw`.
    /// Result (2026-09-10): both within 1e-8.
    /// `getco` label 300 to the lab: CM-isotropic data given as Legendre
    /// (no coefficients) and as a flat table give the same lab moments, and the
    /// first one is positive (forward peaking, `awr = 2`).
    #[test]
    fn cm_isotropic_converts_to_forward_lab_moments() {
        let a = getco_legendre_converted(&[], NLD, TOLER, 2.0);
        let t = getco_tabulated(&[(2, 2)], &[(-1.0, 0.5), (1.0, 0.5)], NLD, TOLER, Some(2.0)).unwrap();
        assert!(a.len() > 1 && a[1] > 0.0, "{a:?}");
        assert_eq!(a.len(), t.len());
        for (x, y) in a.iter().zip(&t) {
            assert!((x - y).abs() <= 1e-12, "{a:?} {t:?}");
        }
    }

    #[test]
    fn gauss64_table_integrates_polynomials() {
        let s0: f64 = gauss64().map(|(_, w)| w).sum();
        let s2: f64 = gauss64().map(|(x, w)| w * x * x).sum();
        assert!((s0 - 2.0).abs() < 1e-8, "{s0}");
        assert!((s2 - 2.0 / 3.0).abs() < 1e-8, "{s2}");
    }
}
