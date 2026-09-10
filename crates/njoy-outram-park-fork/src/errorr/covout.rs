// Ported from NJOY2016 `src/errorr.f90` (git commit ac5adf5f33d893e42f2eed7fb286b0d51c7580da):
//   - `subroutine sigc`, l.7789-8007 (`sigc`) — coarse-group cross sections + MF=1/MF=3 output.
//   - `subroutine covout`, l.7018-7787 (`covout`, `ErrorrResult::to_tape`) — mfcov=33, mf32=0 path.
// NJOY2016 is under a modified BSD 3-Clause (LANL/DOE) licence, GPL-compatible;
// this derivative file is distributed under GPL-3.0-only. This is a modified,
// non-LANL version, not endorsed by LANL/DOE. See crate root LICENSE.njoy + NOTICE.

//! Collapse to the user group structure and write the covariance tape —
//! `sigc` and `covout` (`errorr.f90`).
//!
//! `sigc` sums the union-group fluxes and reaction rates into the user
//! groups (`cflx`, `csig`). `covout` then, for every ordered reaction pair
//! `(ix, ixp)`, accumulates the union-group absolute covariances
//! ([`super::covcal::FineCovariance`]) into the coarse matrix `cova`,
//! weighting each contribution by the derivation coefficients
//! `akxy(iy, ix, k) * akxy(iyp, ixp, kp)` so that a *derived* reaction's
//! covariance is assembled from its components', and finally divides by
//! `cflx(ig) cflx(igp)` (absolute) and, for `irelco = 1`, by
//! `csig(ig,ix) csig(igp,ixp)` (relative). The "trivial derivation" fast
//! path (`isd = 1`, both reactions directly evaluated everywhere) and its
//! `iabort` fallback are reproduced so the port visits exactly the blocks
//! upstream does.
//!
//! [`ErrorrResult::to_tape`] lays the result out the way `covout`/`sigc`
//! write `nout`: MF=1/MT=451 (group bounds), MF=3 per reaction (a single
//! LIST of `csig`), MF=33 per reaction (HEAD, then per `MT1` a CONT and one
//! LIST per non-empty row), with the lumped-component placeholder sections
//! interleaved where upstream puts them. That tape is what COVR reads.
//!
//! **Scope:** `mfcov = 33`, no MF=32 (`resprx`/`rescon` and the
//! resonance-parameter diagonal listing are not ported), `nin = 0` (no
//! input covariance tape to copy), `nout != 0`.

use crate::endf::tape::{Section, Tape};
use crate::endf::EndfKey;
use crate::mixr::mix::sigfig;

use super::covcal::{lumped_sigma, FineCovariance};
use super::gridd::{CovarianceReactions, DerivedCoefficients, LumpedReaction, NDIG};
use super::grpav::UnionGroupXs;

/// `eps` — `covout`'s zero threshold on output elements (`errorr.f90:7071`).
const EPS: f64 = 1.0e-20;

/// Coarse-group cross sections and fluxes (`sigc`, `errorr.f90:7789-7898`).
#[derive(Debug, Clone)]
pub struct CoarseGroupXs {
    /// `cflx(1:ngn)` — user-group flux integrals.
    pub cflx: Vec<f64>,
    /// `csig(ig, ix)` as `[ix][ig]`.
    pub csig: Vec<Vec<f64>>,
}

/// Calculate the coarse-group cross sections (`subroutine sigc`,
/// `errorr.f90:7789-7898`, the numeric part; the tape output lives in
/// [`ErrorrResult::to_tape`]).
///
/// `egn` are the user bounds, `flux` the union-group flux vector, `groups`
/// the union-group cross sections. Both grids are `sigfig`'d to [`NDIG`]
/// before comparison, as upstream does (`errorr.f90:7845-7851`).
pub fn sigc(
    egn: &[f64],
    groups: &UnionGroupXs,
    flux: &[f64],
    reactions: &CovarianceReactions,
) -> CoarseGroupXs {
    let ngn = egn.len() - 1;
    let nunion = groups.nunion();
    let egt: Vec<f64> = groups.un.iter().map(|&e| sigfig(e, NDIG, 0)).collect();
    let egn: Vec<f64> = egn.iter().map(|&e| sigfig(e, NDIG, 0)).collect();
    let in_group = |ig: usize, jg: usize| egt[jg] >= egn[ig] && egt[jg] < egn[ig + 1];

    let mut cflx = vec![0.0; ngn];
    for (ig, c) in cflx.iter_mut().enumerate() {
        for jg in 0..nunion {
            if in_group(ig, jg) {
                *c += flux[jg];
            }
        }
    }
    let mut csig = Vec::with_capacity(reactions.mts.len());
    for (ix, &mt) in reactions.mts.iter().enumerate() {
        let sig = if (851..=870).contains(&mt) {
            lumped_sigma(groups, reactions, mt)
        } else {
            let _ = reactions.mats[ix];
            groups.sigma_for(mt)
        };
        let mut row = vec![0.0; ngn];
        for (ig, c) in row.iter_mut().enumerate() {
            let mut s = 0.0;
            for jg in 0..nunion {
                if in_group(ig, jg) {
                    s += sig[jg] * flux[jg];
                }
            }
            *c = if cflx[ig] > 0.0 { s / cflx[ig] } else { 0.0 };
        }
        csig.push(row);
    }
    CoarseGroupXs { cflx, csig }
}

/// One output covariance matrix `(MT, MAT1/MT1)` in the user group
/// structure (`covout`, `errorr.f90:7413-7526`).
#[derive(Debug, Clone)]
pub struct CoarseCovariance {
    /// Reaction.
    pub mt: i32,
    /// Companion material (`0` = this one).
    pub mat1: i32,
    /// Companion reaction.
    pub mt1: i32,
    /// `ngn`.
    pub ngn: usize,
    /// Row-major `[ig][igp]`: relative (`irelco = 1`) or absolute
    /// (`irelco = 0`) covariance; elements with `|v| <= 1e-20` are zero.
    pub values: Vec<f64>,
    /// `izero` — any element non-zero.
    pub nonzero: bool,
}

impl CoarseCovariance {
    /// Element `(ig, igp)`, 0-based.
    pub fn get(&self, ig: usize, igp: usize) -> f64 {
        self.values[ig * self.ngn + igp]
    }
}

/// The complete ERRORR output for one material (what `nout` carries plus
/// the coarse-group bookkeeping the listing prints).
#[derive(Debug, Clone)]
pub struct ErrorrResult {
    /// Material.
    pub matd: i32,
    /// `ZA`, `AWR` from the MF=33 HEAD.
    pub za: f64,
    /// `AWR`.
    pub awr: f64,
    /// ENDF format era (`iverf`), written to the MF=1 HEAD.
    pub iverf: i32,
    /// Temperature \[K\].
    pub tempin: f64,
    /// `irelco` — `1` relative, `0` absolute.
    pub irelco: i32,
    /// User group bounds `egn(1:ngn+1)` \[eV\].
    pub egn: Vec<f64>,
    /// Union grid `un(1:nunion+1)` \[eV\].
    pub un: Vec<f64>,
    /// Reactions (`mts`/`mats`/`mzap`, possibly negated lumped components).
    pub reactions: CovarianceReactions,
    /// Derived-cross-section coefficients (`ek`/`akxy`, the "coefficients
    /// for derived cross sections" table of the listing).
    pub derived: DerivedCoefficients,
    /// Coarse-group cross sections and flux.
    pub coarse: CoarseGroupXs,
    /// Covariance matrices for every `ix <= ixp`, in output order.
    pub blocks: Vec<CoarseCovariance>,
    /// Informational messages upstream would print (`grpav` thresholds,
    /// the MT=3 note).
    pub messages: Vec<String>,
}

impl ErrorrResult {
    /// `ngn`.
    pub fn ngn(&self) -> usize {
        self.egn.len() - 1
    }

    /// The matrix for `(mt, mt1)` of this material (either ordering).
    pub fn covariance(&self, mt: i32, mt1: i32) -> Option<&CoarseCovariance> {
        self.blocks
            .iter()
            .find(|b| b.mat1 == 0 && ((b.mt == mt && b.mt1 == mt1) || (b.mt == mt1 && b.mt1 == mt)))
    }

    /// Lay the result out as an ENDF-format covariance tape exactly as
    /// `sigc`/`covout` write `nout` (`errorr.f90:7812-7838, 7891-7898,
    /// 7280-7317, 7413-7431, 7488-7526`).
    pub fn to_tape(&self) -> Tape {
        let ngn = self.ngn();
        let mat = self.matd;
        let mut sections = Vec::new();
        let key = |mf: i32, mt: i32| EndfKey { mat, mf, mt };

        // MF=1/MT=451: HEAD (za, awr, iverf, 0, -11, 0) + LIST (tempin, 0, ngn, 0, ngn+1, 0) egn.
        let mut rows = vec![[self.za, self.awr, f64::from(self.iverf), 0.0, -11.0, 0.0]];
        rows.push([self.tempin, 0.0, ngn as f64, 0.0, (ngn + 1) as f64, 0.0]);
        pack_rows(&mut rows, &self.egn);
        sections.push(Section {
            key: key(1, 451),
            rows,
        });

        // MF=3: one LIST per reaction of this material (za, mzap, 0, 0, ngn, 0).
        for (ix, &mt) in self.reactions.mts.iter().enumerate() {
            if self.reactions.mats[ix] != 0 {
                continue;
            }
            let mut rows = vec![[
                self.za,
                f64::from(self.reactions.mzap[ix]),
                0.0,
                0.0,
                ngn as f64,
                0.0,
            ]];
            pack_rows(&mut rows, &self.coarse.csig[ix]);
            sections.push(Section {
                key: key(3, mt),
                rows,
            });
        }

        // MF=33 (errorr.f90:7280-7317): lumped-component placeholders precede
        // the first reaction whose MT exceeds them.
        let mut lump_components: Vec<(i32, i32)> = Vec::new(); // (component, mtl)
        for LumpedReaction { mtl, components } in &self.reactions.lumps {
            for &c in components {
                lump_components.push((c, *mtl));
            }
        }
        let mut next_lump = 0usize;
        let nmt = self.reactions.mts.len();
        let nmts = nmt;
        let mut block_iter = self.blocks.iter();
        for (ix, &mt) in self.reactions.mts.iter().enumerate() {
            while next_lump < lump_components.len() && lump_components[next_lump].0 < mt {
                let (c, mtl) = lump_components[next_lump];
                sections.push(Section {
                    key: key(33, c),
                    rows: vec![[self.za, self.awr, 0.0, f64::from(mtl), 0.0, 0.0]],
                });
                next_lump += 1;
            }
            let mut rows = vec![[self.za, self.awr, 0.0, 0.0, 0.0, (nmts - ix) as f64]];
            for ixp in ix..nmts {
                let b = block_iter
                    .next()
                    .expect("one CoarseCovariance per (ix, ixp) pair");
                rows.push([
                    0.0,
                    0.0,
                    f64::from(self.reactions.mats[ixp]),
                    f64::from(self.reactions.mts[ixp]),
                    0.0,
                    ngn as f64,
                ]);
                debug_assert_eq!((b.mt, b.mt1), (mt, self.reactions.mts[ixp]));
                for ig in 0..ngn {
                    let row = &b.values[ig * ngn..(ig + 1) * ngn];
                    let mut ig2lo = 0usize;
                    let mut ng2 = 0usize;
                    for (igp, &v) in row.iter().enumerate() {
                        if v.abs() > EPS {
                            if ig2lo == 0 {
                                ig2lo = igp + 1;
                            }
                            ng2 = igp + 1;
                        }
                    }
                    if ng2 != 0 || ig + 1 >= ngn {
                        if ng2 == 0 {
                            ig2lo = ig + 1;
                            ng2 = ig + 1;
                        }
                        let n = ng2 - ig2lo + 1;
                        rows.push([0.0, 0.0, n as f64, ig2lo as f64, n as f64, (ig + 1) as f64]);
                        pack_rows(&mut rows, &row[ig2lo - 1..ng2]);
                    }
                }
            }
            sections.push(Section {
                key: key(33, mt),
                rows,
            });
        }
        Tape::from_sections(String::new(), sections)
    }
}

/// Append `data` to `rows` six words per row, zero-padded (the LIST body
/// layout `listio` writes).
fn pack_rows(rows: &mut Vec<[f64; 6]>, data: &[f64]) {
    for chunk in data.chunks(6) {
        let mut row = [0.0f64; 6];
        row[..chunk.len()].copy_from_slice(chunk);
        rows.push(row);
    }
}

/// Coarse-group index (0-based) of union energy `e`
/// (`covout`, `errorr.f90:7357-7360`): the first `i` with
/// `egn(i) <= e < egn(i+1)`; `None` when `e >= egn(ngn+1)`.
fn coarse_index(egn: &[f64], e: f64) -> Option<usize> {
    let ngn = egn.len() - 1;
    if e >= egn[ngn] {
        return None;
    }
    (0..ngn).find(|&i| e >= egn[i] && e < egn[i + 1])
}

/// Accumulate one fine block's rows into `cova` (`covout`,
/// `errorr.f90:7343-7395`). `cova` is `[ig][igp]`.
#[allow(clippy::too_many_arguments)]
fn accumulate(
    rows: &[Vec<f64>],
    un: &[f64],
    egn: &[f64],
    derived: &DerivedCoefficients,
    iy: usize,
    iyp: usize,
    ix: usize,
    ixp: usize,
    cova: &mut [f64],
) -> bool {
    let ngn = egn.len() - 1;
    let nek = derived.nek();
    let ek = &derived.ek;
    let mut izero = false;
    for (jg, row) in rows.iter().enumerate() {
        if row.is_empty() || (row.len() == 1 && row[0] == 0.0) {
            continue;
        }
        let egtjg = un[jg];
        let Some(ig) = coarse_index(egn, egtjg) else {
            continue;
        };
        // derived energy range for jg (label 280)
        let Some(k) = (0..nek).find(|&k| egtjg >= ek[k] && egtjg < ek[k + 1]) else {
            continue;
        };
        let mut igp = 0usize;
        let mut kp = 0usize;
        for (jgp, &c) in row.iter().enumerate() {
            if c == 0.0 {
                continue;
            }
            let egtjgp = un[jgp];
            // derived energy range for jgp (label 320)
            while !(egtjgp >= ek[kp] && egtjgp < ek[kp + 1]) {
                if kp + 1 >= nek {
                    kp = usize::MAX;
                    break;
                }
                kp += 1;
            }
            if kp == usize::MAX {
                kp = 0;
                continue;
            }
            // second coarse group (label 330)
            while !(egtjgp >= egn[igp] && egtjgp < egn[igp + 1]) {
                if igp + 1 >= ngn {
                    igp = usize::MAX;
                    break;
                }
                igp += 1;
            }
            if igp == usize::MAX {
                igp = 0;
                continue;
            }
            // add this contribution (label 350)
            let a = derived.get(iy, ix, k) * derived.get(iyp, ixp, kp) * c;
            cova[ig * ngn + igp] += a;
            if cova[ig * ngn + igp] != 0.0 {
                izero = true;
            }
            if iyp != iy {
                let b = derived.get(iyp, ix, kp) * derived.get(iy, ixp, k) * c;
                cova[igp * ngn + ig] += b;
                if cova[igp * ngn + ig] != 0.0 {
                    izero = true;
                }
            }
        }
    }
    izero
}

/// Compute the output covariances for every reaction pair in the user
/// group structure (`subroutine covout`, `errorr.f90:7018-7787`,
/// `mfcov = 33`, `mf32 = 0`).
///
/// Returns the matrices in output order (`ix` outer, `ixp >= ix` inner).
pub fn covout(
    fine: &FineCovariance,
    un: &[f64],
    egn: &[f64],
    reactions: &CovarianceReactions,
    derived: &DerivedCoefficients,
    coarse: &CoarseGroupXs,
    irelco: i32,
) -> Vec<CoarseCovariance> {
    let ngn = egn.len() - 1;
    let nmt = reactions.mts.len();
    let nmts = nmt;
    let index_of = |mat: i32, mt: i32| {
        reactions
            .mts
            .iter()
            .zip(&reactions.mats)
            .position(|(&m, &a)| m == mt && a == mat)
    };
    let mut out = Vec::with_capacity(nmt * (nmt + 1) / 2);
    for ix in 0..nmt {
        let mut iabort = false;
        for ixp in ix..nmts {
            // trivial derivation case? (errorr.f90:7236-7244)
            let isd =
                !iabort && derived.is_directly_evaluated(ix) && derived.is_directly_evaluated(ixp);
            let mut cova = vec![0.0f64; ngn * ngn];
            let mut izero = false;
            if isd {
                match fine.block(reactions.mts[ix], reactions.mats[ixp], reactions.mts[ixp]) {
                    Some(b) => {
                        izero = accumulate(&b.rows, un, egn, derived, ix, ixp, ix, ixp, &mut cova);
                    }
                    None => {
                        // desired mt1 missing: empty matrix, abort speed-up (errorr.f90:7325-7331)
                        iabort = true;
                    }
                }
            } else {
                for b in &fine.blocks {
                    if b.rows
                        .iter()
                        .all(|r| r.is_empty() || (r.len() == 1 && r[0] == 0.0))
                    {
                        continue;
                    }
                    let (Some(iy), Some(iyp)) = (index_of(0, b.mt), index_of(b.mat1, b.mt1)) else {
                        continue; // upstream: fatal "unable to find iy or iyp"; unreachable for iread=0
                    };
                    if accumulate(&b.rows, un, egn, derived, iy, iyp, ix, ixp, &mut cova) {
                        izero = true;
                    }
                }
            }
            // relative / absolute output elements (errorr.f90:7458-7486)
            let mut values = vec![0.0f64; ngn * ngn];
            for ig in 0..ngn {
                for igp in 0..ngn {
                    let mut v = cova[ig * ngn + igp] / (coarse.cflx[ig] * coarse.cflx[igp]);
                    if v != 0.0 && irelco != 0 {
                        let denom = coarse.csig[ix][ig] * coarse.csig[ixp][igp];
                        if denom > 0.0 {
                            v /= denom.max(EPS);
                        } else {
                            v = 0.0;
                        }
                    }
                    if v.abs() <= EPS {
                        v = 0.0;
                    }
                    values[ig * ngn + igp] = v;
                }
            }
            out.push(CoarseCovariance {
                mt: reactions.mts[ix],
                mat1: reactions.mats[ixp],
                mt1: reactions.mts[ixp],
                ngn,
                values,
                nonzero: izero,
            });
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn coarse_index_and_pack_rows() {
        let egn = [1.0, 10.0, 100.0];
        assert_eq!(coarse_index(&egn, 1.0), Some(0));
        assert_eq!(coarse_index(&egn, 50.0), Some(1));
        assert_eq!(coarse_index(&egn, 100.0), None);
        let mut rows = Vec::new();
        pack_rows(&mut rows, &[1.0, 2.0, 3.0, 4.0, 5.0, 6.0, 7.0]);
        assert_eq!(rows.len(), 2);
        assert_eq!(rows[1], [7.0, 0.0, 0.0, 0.0, 0.0, 0.0]);
    }
}
