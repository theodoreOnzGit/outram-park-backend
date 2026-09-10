// Ported from NJOY2016 `src/errorr.f90` (git commit ac5adf5f33d893e42f2eed7fb286b0d51c7580da):
//   - `subroutine covcal`, l.1770-2417 (`covcal`, `NiRecord`, `contribution`) — mfcov=33,
//     nstan=0 path: the group-average stage (l.2086-2417) that the structural reader in
//     `covariance.rs` deliberately left out.
//   - `subroutine lumpxs`, l.6969-7016 (`lumped_sigma`).
// NJOY2016 is under a modified BSD 3-Clause (LANL/DOE) licence, GPL-compatible;
// this derivative file is distributed under GPL-3.0-only. This is a modified,
// non-LANL version, not endorsed by LANL/DOE. See crate root LICENSE.njoy + NOTICE.

//! Absolute covariances on the union grid — `covcal` (`errorr.f90:1770-2417`).
//!
//! For every MF=33 section (reaction `MT`) and every subsection
//! (`MAT1`/`MT1`), `covcal` evaluates the NI-type records on each pair of
//! union groups `(jg, jh)` and writes one *row* per `jg` of the absolute,
//! flux-weighted covariance `cov(jh) * flx(jg) * flx(jh)` to a scratch tape
//! that `covout` later collapses. The encodings (`LB` 0–6 and 8) are decoded
//! with the same 1-based index arithmetic as the Fortran (`scr(loci+…)` →
//! `data(…)`), documented inline; see the ENDF-6 Formats Manual §33.2 for
//! what each `LB` means physically.
//!
//! Two upstream idioms are kept deliberately: every data word (energies
//! *and* matrix values) is rounded to `ndig = 6` figures on the way in
//! (`errorr.f90:2075-2077`), and the `LB = 4` "row" search mixes `eg` and
//! `ehr` (`errorr.f90:2364`) — that is what the reference code does, so it
//! is what the port does.
//!
//! **Scope:** `mfcov = 33`, `nstan = 0` (an NC-type record with `LTY` 1–3
//! is `NotPorted`), no `matb`/`matc` standards redefinition, ENDF-5/6. The
//! `LB = 7` law belongs to MF=35 and is rejected here as upstream does.

use crate::endf::records::SectionCursor;
use crate::endf::tape::Tape;
use crate::mixr::mix::sigfig;
use crate::NjoyError;

use super::gridd::{CovarianceReactions, NDIG};
use super::grpav::UnionGroupXs;

/// One NI-type sub-subsection as `covcal` stages it (`scr(loc(i)..)`,
/// `errorr.f90:2062-2077`).
#[derive(Debug, Clone)]
pub struct NiRecord {
    /// `LT` (`scr(loci+2)`); for `LB = 6` replaced by `NEC = (NT-1)/NER`.
    pub lt: usize,
    /// `LB`.
    pub lb: i32,
    /// `NP` — `scr(loci+5)`: pairs for `LB < 5`/`8`, `NE` for `LB = 5`,
    /// `NER` for `LB = 6`.
    pub np: usize,
    /// The `NT` data words, each `sigfig`'d to [`NDIG`].
    pub data: Vec<f64>,
}

impl NiRecord {
    /// Build from a LIST head `(c1,c2,l1,l2,n1,n2)` and its data
    /// (`errorr.f90:2062-2077`).
    fn from_list(l1: i32, l2: i32, n1: i32, n2: i32, data: &[f64]) -> Self {
        let lb = l2;
        let mut lt = l1.max(0) as usize;
        if lb == 6 && n2 > 0 {
            lt = ((n1 - 1) / n2).max(0) as usize;
        }
        NiRecord {
            lt,
            lb,
            np: n2.max(0) as usize,
            data: data.iter().map(|&v| sigfig(v, NDIG, 0)).collect(),
        }
    }

    /// `data(i)`, 1-based.
    #[inline]
    fn d(&self, i: usize) -> f64 {
        self.data[i - 1]
    }

    /// Upstream's law/flag consistency checks (`errorr.f90:2141-2149`).
    fn validate(&self, mt: i32, mt1: i32) -> Result<(), NjoyError> {
        if self.lb == 7 || self.lb > 8 {
            return Err(NjoyError::NotPorted(
                "errorr::covcal: lb=7 / lb>8 (MF=35 laws) are not coded for MF=33",
            ));
        }
        if self.lb < 3 && self.lt > 0 {
            return Err(NjoyError::EndfParse(format!(
                "errorr::covcal: lb={} when lt={} (MT={mt}, MT1={mt1})",
                self.lb, self.lt
            )));
        }
        Ok(())
    }
}

/// First 1-based interval `k` in `1..=nint` with `e(k) <= x < e(k+1)`,
/// scanning in order exactly as the Fortran `k = k+1 … go to` loops do.
#[inline]
fn find_bin(x: f64, nint: usize, e: impl Fn(usize) -> f64) -> Option<usize> {
    (1..=nint).find(|&k| x >= e(k) && x < e(k + 1))
}

/// The contribution of one record to `cov(jh)` for union groups `(jg, jh)`
/// with lower bounds `(eg, ehr)` (`errorr.f90:2150-2417`).
///
/// `sig`/`sig1` are the reaction / `MT1` union-group cross sections;
/// `dun` is `un(jg+1) - un(jg)` (used by `LB = 8`).
#[allow(clippy::too_many_arguments)]
fn contribution(
    r: &NiRecord,
    jg: usize,
    jh: usize,
    eg: f64,
    ehr: f64,
    dun: f64,
    sig_jg: f64,
    sig1_jh: f64,
) -> f64 {
    let np = r.np;
    let ss = sig_jg * sig1_jh;
    match r.lb {
        8 => {
            // errorr.f90:2154-2171: diagonal only, pairs (E_k,F_k) at
            // data(2k-1), data(2k); nk1 = np-1 intervals.
            if jh != jg || np < 2 {
                return 0.0;
            }
            match find_bin(eg, np - 1, |k| r.d(2 * k - 1)) {
                Some(k) => {
                    let xcv = (r.d(2 * k + 1) - r.d(2 * k - 1)) / dun;
                    r.d(2 * k) * xcv
                }
                None => 0.0,
            }
        }
        6 => {
            // errorr.f90:2196-2223: row grid data(1..np), column grid
            // data(np+1..np+lt), matrix data(np+lt+(k-1)(lt-1)+l).
            if np < 2 || r.lt < 2 {
                return 0.0;
            }
            let Some(k) = find_bin(eg, np - 1, |k| r.d(k)) else {
                return 0.0;
            };
            let nl1 = r.lt - 1;
            let Some(l) = find_bin(ehr, nl1, |l| r.d(np + l)) else {
                return 0.0;
            };
            r.d(np + r.lt + (k - 1) * nl1 + l) * ss
        }
        5 => {
            // errorr.f90:2225-2255: one grid data(1..np); nk1 = np-1.
            if np < 2 {
                return 0.0;
            }
            let nk1 = np - 1;
            let Some(k) = find_bin(eg, nk1, |k| r.d(k)) else {
                return 0.0;
            };
            let Some(l) = find_bin(ehr, nk1, |l| r.d(l)) else {
                return 0.0;
            };
            let idx = if r.lt == 1 {
                // symmetric upper-triangular pack (label 390)
                let (a, b) = if l >= k { (k, l) } else { (l, k) };
                nk1 * np / 2 - (np - a + 1) * (np - a) / 2 + (b - a)
            } else {
                (k - 1) * nk1 + (l - 1)
            };
            r.d(1 + idx + np) * ss
        }
        0..=4 => {
            // errorr.f90:2257-2417: (E,F) pairs; first table nk = np-lt pairs,
            // second table (lb=3/4) the last lt pairs.
            let nlt = r.lt;
            if np < nlt + 2 {
                return 0.0;
            }
            let nk = np - nlt;
            let nk1 = nk - 1;
            let nlt1 = nlt.saturating_sub(1);
            let e1 = |k: usize| r.d(2 * k - 1); // E_k of the first table
            let f1 = |k: usize| r.d(2 * k); // F_k of the first table
            let e2 = |l: usize| r.d(2 * nk + 2 * l - 1); // E'_l of the second table
            let f2 = |l: usize| r.d(2 * nk + 2 * l); // F'_l
            let Some(k) = find_bin(eg, nk1, e1) else {
                return 0.0;
            };
            if r.lb == 2 || r.lb == 3 {
                // label 440/450/460: column search on the same (lb=2) or
                // the second (lb=3) table.
                let (lend, ecol, fcol): (usize, &dyn Fn(usize) -> f64, &dyn Fn(usize) -> f64) =
                    if r.lb == 2 {
                        (nk1, &e1, &f1)
                    } else {
                        (nlt1, &e2, &f2)
                    };
                let Some(l) = find_bin(ehr, lend, ecol) else {
                    return 0.0;
                };
                return f1(k) * fcol(l) * ss;
            }
            // lb = 0, 1, 4: ehr must fall in the same first-table bin (label 490).
            if !(ehr >= e1(k) && ehr < e1(k + 1)) {
                return 0.0;
            }
            match r.lb {
                0 => f1(k),
                1 => f1(k) * ss,
                _ => {
                    // lb = 4 (labels 450/460/480/482/484): l for ehr on the
                    // second table, then m with the upstream mixed test.
                    let Some(l) = find_bin(ehr, nlt1, e2) else {
                        return 0.0;
                    };
                    let Some(m) = (1..=nlt1).find(|&m| eg >= e2(m) && ehr < e2(m + 1)) else {
                        return 0.0;
                    };
                    f1(k) * f2(m) * f2(l) * ss
                }
            }
        }
        _ => 0.0,
    }
}

/// One `(MT, MAT1, MT1)` covariance matrix on the union grid, as the rows
/// `covcal` writes to `nscr1` (`errorr.f90:2277-2311`).
#[derive(Debug, Clone)]
pub struct FineBlock {
    /// The section's reaction.
    pub mt: i32,
    /// `MAT1` of the subsection (`0` = same material).
    pub mat1: i32,
    /// `MT1` of the subsection.
    pub mt1: i32,
    /// Row `jg` (0-based) holds `cov(jh) * flx(jg) * flx(jh)` for
    /// `jh < jgend` (trailing zeros trimmed); an empty row was not written
    /// (all zero and not the last group). A null matrix (`iok = 0`) has
    /// every row empty.
    pub rows: Vec<Vec<f64>>,
}

/// Every fine-group covariance block, in tape order.
#[derive(Debug, Clone)]
pub struct FineCovariance {
    /// `nunion`.
    pub nunion: usize,
    /// `nmts` — sections processed.
    pub nmts: usize,
    /// The blocks.
    pub blocks: Vec<FineBlock>,
}

impl FineCovariance {
    /// Find the block for `(mt, mat1, mt1)`.
    pub fn block(&self, mt: i32, mat1: i32, mt1: i32) -> Option<&FineBlock> {
        self.blocks
            .iter()
            .find(|b| b.mt == mt && b.mat1 == mat1 && b.mt1 == mt1)
    }
}

/// Sum of the component cross sections of lumped reaction `mtl`
/// (`subroutine lumpxs`, `errorr.f90:6969-7016`).
pub fn lumped_sigma(groups: &UnionGroupXs, reactions: &CovarianceReactions, mtl: i32) -> Vec<f64> {
    let n = groups.nunion();
    let mut sig = vec![0.0; n];
    if let Some(l) = reactions.lumps.iter().find(|l| l.mtl == mtl) {
        for &mtd in &l.components {
            for (a, b) in sig.iter_mut().zip(groups.sigma_for(mtd)) {
                *a += b;
            }
        }
    }
    sig
}

/// Calculate absolute covariances in the union-group structure
/// (`subroutine covcal`, `errorr.f90:1770-2417`, `mfcov = 33`).
///
/// `flx` is the union-group flux (`UnionGroupXs::flux_vector`). Sections are
/// visited in MF=33 file order and must match `reactions.mts` (upstream's
/// "mfcov mt found not equal to input mt" check).
///
/// # Errors
/// - [`NjoyError::NotPorted`] for `LTY` 1–3 NC-type records or the MF=35
///   `LB = 7` law.
/// - [`NjoyError::EndfParse`] for `MT1 = 0`, an `mts` mismatch, or a
///   malformed record.
pub fn covcal(
    endf: &Tape,
    matd: i32,
    reactions: &CovarianceReactions,
    groups: &UnionGroupXs,
    flx: &[f64],
) -> Result<FineCovariance, NjoyError> {
    let nunion = groups.nunion();
    let un = &groups.un;
    let mut out = FineCovariance {
        nunion,
        nmts: 0,
        blocks: Vec::new(),
    };
    if !reactions.mf33_present {
        return Ok(out); // errorr.f90:1849-1852 "temporary patch for missing mf33"
    }

    for sec in endf
        .sections()
        .iter()
        .filter(|s| s.key.mat == matd && s.key.mf == 33)
    {
        let mth = sec.key.mt;
        let mut cur = SectionCursor::new(&sec.rows);
        let head = cur.read_cont()?;
        let nl = head.n2;
        if nl == 0 {
            continue; // component of a lumped reaction (errorr.f90:1922)
        }
        let nmts = out.nmts; // 0-based index of this section in mts
        out.nmts += 1;
        if reactions.mts.get(nmts).copied() != Some(mth) {
            return Err(NjoyError::EndfParse(format!(
                "errorr::covcal: mfcov mt found ({mth}) not equal to input mt ({:?})",
                reactions.mts.get(nmts)
            )));
        }
        // sig for this reaction (errorr.f90:1962-1970)
        let sig: Vec<f64> = if (851..=870).contains(&mth) {
            lumped_sigma(groups, reactions, mth)
        } else {
            groups.sigma_for(mth)
        };

        for _ in 0..nl {
            let sub = cur.read_cont()?;
            let mat1 = sub.l1;
            let mt1 = sub.l2;
            let nc = sub.n1;
            let ni = sub.n2;
            if mt1 == 0 {
                return Err(NjoyError::EndfParse(format!(
                    "errorr::covcal: illegal mt1=0 in MF=33/MT={mth}"
                )));
            }
            // errorr.f90:1997-2004: is (mat1, mt1) in the reaction list?
            let kmt1 = reactions
                .mts
                .iter()
                .zip(&reactions.mats)
                .position(|(&m, &a)| m == mt1 && a == mat1);
            let iok = kmt1.is_some();

            // NC-type sub-subsections (errorr.f90:2017-2030): read and skip
            // (lty=0 carries no matrix); lty 1..3 would need `stand`/nstan.
            for _ in 0..nc {
                let lty_cont = cur.read_cont()?;
                let lty = lty_cont.l2;
                let _list = cur.read_list()?;
                if iok && (1..=3).contains(&lty) {
                    return Err(NjoyError::NotPorted(
                        "errorr::covcal: NC-type sub-subsection with lty 1..3 (ratio-to-standard, needs nstan)",
                    ));
                }
            }
            // NI-type sub-subsections (errorr.f90:2058-2081)
            let mut recs: Vec<NiRecord> = Vec::with_capacity(ni.max(0) as usize);
            for _ in 0..ni {
                let list = cur.read_list()?;
                recs.push(NiRecord::from_list(
                    list.head.l1,
                    list.head.l2,
                    list.head.n1,
                    list.head.n2,
                    &list.data,
                ));
            }

            if !iok || recs.is_empty() {
                // label 600: null matrix
                out.blocks.push(FineBlock {
                    mt: mth,
                    mat1,
                    mt1,
                    rows: vec![Vec::new(); nunion],
                });
                continue;
            }
            for r in &recs {
                r.validate(mth, mt1)?;
            }
            // sigma for mt1 (errorr.f90:2103-2114)
            let sig1: Vec<f64> = if kmt1 != Some(nmts) {
                if (851..=870).contains(&mt1) {
                    lumped_sigma(groups, reactions, mt1)
                } else {
                    groups.sigma_for(mt1)
                }
            } else {
                sig.clone()
            };

            // generate covariance matrix using the specified laws (errorr.f90:2121-2311)
            let mut rows: Vec<Vec<f64>> = Vec::with_capacity(nunion);
            let mut cov = vec![0.0f64; nunion];
            for jg in 0..nunion {
                let eg = un[jg];
                let dun = un[jg + 1] - un[jg];
                for jh in 0..nunion {
                    let ehr = un[jh];
                    let mut c = 0.0;
                    for r in &recs {
                        c += contribution(r, jg, jh, eg, ehr, dun, sig[jg], sig1[jh]);
                    }
                    cov[jh] = c;
                }
                // write one row (errorr.f90:2277-2311)
                let mut jgend = 0usize;
                for (ih, &c) in cov.iter().enumerate() {
                    if c != 0.0 {
                        jgend = ih + 1;
                    }
                }
                if jgend == 0 {
                    if jg + 1 < nunion {
                        rows.push(Vec::new());
                        continue;
                    }
                    jgend = 1;
                }
                let row: Vec<f64> = (0..jgend).map(|ij| cov[ij] * flx[jg] * flx[ij]).collect();
                rows.push(row);
            }
            out.blocks.push(FineBlock {
                mt: mth,
                mat1,
                mt1,
                rows,
            });
        }
    }
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn rec(lt: i32, lb: i32, n2: i32, data: &[f64]) -> NiRecord {
        NiRecord::from_list(lt, lb, data.len() as i32, n2, data)
    }

    /// Union-grid energies reach the kernel `sigfig`'d exactly like the
    /// record data, so the hand values must be rounded the same way.
    fn sf(x: f64) -> f64 {
        sigfig(x, NDIG, 0)
    }

    /// `LB = 5`, `LS = 1`: the symmetric pack index reproduces the ENDF
    /// upper-triangular layout `F(k,l)` for a 3-point grid.
    #[test]
    fn lb5_symmetric_pack_indexing() {
        // E = 1,10,100 ; F upper-tri: (1,1)=11 (1,2)=12 (2,2)=22
        let r = rec(1, 5, 3, &[1.0, 10.0, 100.0, 11.0, 12.0, 22.0]);
        let c = |eg, ehr| contribution(&r, 0, 0, sf(eg), sf(ehr), 1.0, 2.0, 3.0);
        assert_eq!(c(1.0, 1.0), sf(11.0) * 6.0);
        assert_eq!(c(1.0, 10.0), sf(12.0) * 6.0);
        assert_eq!(c(10.0, 1.0), sf(12.0) * 6.0);
        assert_eq!(c(10.0, 10.0), sf(22.0) * 6.0);
        assert_eq!(c(100.0, 10.0), 0.0); // above the grid
                                         // LS = 0: full (NE-1)^2 matrix row-major
        let r0 = rec(0, 5, 3, &[1.0, 10.0, 100.0, 11.0, 12.0, 21.0, 22.0]);
        let c0 = |eg, ehr| contribution(&r0, 0, 0, sf(eg), sf(ehr), 1.0, 1.0, 1.0);
        assert_eq!(c0(1.0, 10.0), sf(12.0));
        assert_eq!(c0(10.0, 1.0), sf(21.0));
    }

    /// `LB = 1` is diagonal in the pair bins; `LB = 0` is absolute; `LB = 2`
    /// is the outer product `F_k F_l`; `LB = 8` scales by the bin/group width
    /// ratio on the diagonal only.
    #[test]
    fn lb0_1_2_8_laws() {
        let pairs = [1.0, 0.5, 10.0, 0.25, 100.0, 0.0];
        let r1 = rec(0, 1, 3, &pairs);
        assert_eq!(
            contribution(&r1, 0, 0, sf(1.0), sf(5.0), 1.0, 2.0, 2.0),
            sf(0.5) * 4.0
        );
        assert_eq!(
            contribution(&r1, 0, 1, sf(1.0), sf(10.0), 1.0, 2.0, 2.0),
            0.0
        );
        let r0 = rec(0, 0, 3, &pairs);
        assert_eq!(
            contribution(&r0, 0, 0, sf(10.0), sf(50.0), 1.0, 2.0, 2.0),
            sf(0.25)
        );
        let r2 = rec(0, 2, 3, &pairs);
        assert_eq!(
            contribution(&r2, 0, 1, sf(1.0), sf(10.0), 1.0, 2.0, 2.0),
            sf(0.5) * sf(0.25) * 4.0
        );
        let r8 = rec(0, 8, 3, &pairs);
        // group [1,4): xcv = (10-1)/3
        let dun = sf(4.0) - sf(1.0);
        let want = sf(0.5) * (sf(10.0) - sf(1.0)) / dun;
        assert!((contribution(&r8, 0, 0, sf(1.0), sf(1.0), dun, 1.0, 1.0) - want).abs() < 1e-12);
        assert_eq!(
            contribution(&r8, 0, 1, sf(1.0), sf(1.0), dun, 1.0, 1.0),
            0.0
        );
    }

    /// `LB = 6`: rectangular row/column grids.
    #[test]
    fn lb6_rectangular() {
        // NER=3 rows (1,10,100), NEC=2 cols (1,100): matrix 2x1 = [7, 9]
        let data = [1.0, 10.0, 100.0, 1.0, 100.0, 7.0, 9.0];
        let r = NiRecord::from_list(0, 6, data.len() as i32, 3, &data);
        assert_eq!(r.lt, 2);
        assert_eq!(
            contribution(&r, 0, 0, sf(1.0), sf(50.0), 1.0, 1.0, 1.0),
            sf(7.0)
        );
        assert_eq!(
            contribution(&r, 0, 0, sf(10.0), sf(1.0), 1.0, 1.0, 1.0),
            sf(9.0)
        );
    }

    #[test]
    fn validate_rejects_mf35_and_bad_lt() {
        assert!(rec(0, 7, 2, &[1.0, 2.0]).validate(1, 1).is_err());
        assert!(rec(1, 1, 3, &[1.0, 2.0, 3.0, 4.0, 5.0, 6.0])
            .validate(1, 1)
            .is_err());
        assert!(rec(0, 1, 2, &[1.0, 2.0, 3.0, 4.0]).validate(1, 1).is_ok());
    }
}
