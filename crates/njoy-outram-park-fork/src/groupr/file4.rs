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
}

/// A File-4 angular distribution ready to answer `getfle` queries.
#[derive(Debug, Clone, PartialEq)]
pub struct File4Angular {
    /// `LCT` — frame of the data (1 lab, 2 CM).
    pub lct: i32,
    /// `LI` — 1 when the section declares isotropy (no records follow).
    pub isotropic: bool,
    points: Vec<EnergyPoint>,
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
        let p = legndr(x, l);
        let yw = y * w;
        for il in 0..nl_max {
            fl[il] += yw * p[il];
        }
    }
    let mut nlz = 1;
    for il in 2..=nl_max {
        let j = (fl[il - 1] / TOLER).round();
        fl[il - 1] = j * TOLER;
        if j != 0.0 {
            nlz = il;
        }
    }
    fl.truncate(nlz);
    Ok(fl)
}

/// Read one TAB2's worth of angular records (`ne` of them, each a LIST for
/// `ltt = 1` or a TAB1 for `ltt = 2`) into [`EnergyPoint`]s.
fn read_records(
    cur: &mut SectionCursor<'_>,
    ltt: i32,
    nl_max: usize,
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
                    fl: getco_legendre(&list.data, nl_max),
                    law,
                }
            }
            2 => {
                let t = cur.read_tab1()?;
                EnergyPoint {
                    energy: t.head.c2,
                    fl: getco_tabulated(&t.interp, &t.pairs, nl_max)?,
                    law,
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
        if li != 1 {
            if lct == 1 {
                // getco:10685 — "lab to cm conversion not coded".
                return Err(NjoyError::EndfParse(format!(
                    "getfle: MF=4/MT={mt} of mat {mat} is a lab-frame distribution (LCT=1); \
                     upstream cannot convert it to CM either"
                )));
            }
            match ltt {
                1 | 2 => read_records(&mut cur, ltt, nl_max, &mut points)?,
                3 => {
                    read_records(&mut cur, 1, nl_max, &mut points)?;
                    read_records(&mut cur, 2, nl_max, &mut points)?;
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
        Ok(Self {
            lct,
            isotropic,
            points,
        })
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
        // Label 300.
        if e >= elo * (1.0 - SMALL) {
            let (flo, fhi) = (&pts[hi - 1].fl, &pts[hi].fl);
            let nlmax = flo.len().max(fhi.len());
            let law = IntLaw::from_code(pts[hi].law);
            let mut fle = vec![0.0f64; nle];
            for (i, slot) in fle.iter_mut().enumerate().take(nlmax.min(nle)) {
                let ylo = flo.get(i).copied().unwrap_or(0.0);
                let yhi = fhi.get(i).copied().unwrap_or(0.0);
                *slot = terp1(elo, ylo, ehi, yhi, e, law)?;
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
        let fl = getco_tabulated(&[(2, 2)], &[(-1.0, 0.0), (1.0, 1.0)], NLD).unwrap();
        assert!((fl[0] - 1.0).abs() < 2e-8, "fl(1) = {}", fl[0]);
        assert_eq!(fl.len(), 2, "nlz: {fl:?}");
        assert!((fl[1] - 1.0 / 3.0).abs() < 2e-8, "fl(2) = {}", fl[1]);
    }

    /// Methodology: NJOY's 64-point table must integrate `1` and `mu^2`
    /// (`2` and `2/3`) — a sanity check on the transcription of `qp`/`qw`.
    /// Result (2026-09-10): both within 1e-8.
    #[test]
    fn gauss64_table_integrates_polynomials() {
        let s0: f64 = gauss64().map(|(_, w)| w).sum();
        let s2: f64 = gauss64().map(|(x, w)| w * x * x).sum();
        assert!((s0 - 2.0).abs() < 1e-8, "{s0}");
        assert!((s2 - 2.0 / 3.0).abs() < 1e-8, "{s2}");
    }
}
