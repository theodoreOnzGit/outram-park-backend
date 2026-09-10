// Ported from NJOY2016 `src/groupr.f90` (git commit ac5adf5f33d893e42f2eed7fb286b0d51c7580da).
// NJOY2016 is under a modified BSD 3-Clause (LANL/DOE) licence, GPL-compatible;
// this derivative file is distributed under GPL-3.0-only. This is a modified,
// non-LANL version, not endorsed by LANL/DOE. See crate root LICENSE.njoy + NOTICE.

//! The `panel` quadrature for a **transfer matrix** — `panel`
//! (`groupr.f90:5858-6091`), its caller loop (`:510-540`), and the
//! `displa` normalisation/trimming (`:6093-6437`) — driven by a two-body
//! feed ([`crate::groupr::two_body`]).
//!
//! Unlike the vector reduction in [`crate::groupr::panel`], the matrix
//! path needs everything `panel` does: the sub-panel bounded by the next
//! cross-section, flux *and feed-function* point; the `rndoff`/`delta`
//! shading of the lower and upper sample energies (so a discontinuity of
//! any factor is sampled on both sides); the Lobatto rule of order
//! `nq + 2` (2, 6 or 10 points) over which the feed function is re-evaluated
//! at each abscissa while `sigma*phi` is taken linear (`:6009-6056`); the
//! `ig1`/`iglo`/`igt` secondary-group bookkeeping; and `displa`'s
//! seven-figure `ans(il,iz,ig2)/ans(il,iz,1)` ratio with its `ig2lo`/`ng2`
//! trimming. All of it is ported statement for statement here, including
//! the `save`d state that carries the last feed function, the last
//! cross-section/flux samples and the next break across sub-panels.
//!
//! # Scope
//! - `nz >= 1` dilutions with one [`GroupFlux`] per dilution — the `il = 1`
//!   flux component of `genflx`. Upstream weights higher Legendre orders
//!   with `fout(l-1)*fac` when `nz > 1` (`:5605-5608`); that is **not**
//!   provided here, so `nz > 1` together with `nl > 1` is refused.
//! - The cross section must be a tabulated [`PointwiseXs::LinLin`] so the
//!   `gety1` conventions apply (next point, duplicate-energy discontinuity,
//!   zero past the last point → `emax`).

// The Fortran index loops are kept as written so each statement can be
// checked against its `groupr.f90` line.
#![allow(clippy::needless_range_loop)]

use crate::groupr::gendf::{GendfGroupRecord, GendfSection};
use crate::groupr::panel::{GroupFlux, PointwiseXs};
use crate::groupr::two_body::TwoBodyFeed;
use crate::mixr::mix::sigfig;
use crate::NjoyError;

/// `rndoff` (`:5895`).
const RNDOFF: f64 = 1.000_002;
/// `delta` (`:5896`).
const DELTA: f64 = 0.999_995;
/// `emax` (`:5897`).
const EMAX: f64 = 1.0e10;
/// `small` (`:5898`).
const SMALL: f64 = 1.0e-10;
/// `smin` in `displa` (`:6109`).
const SMIN: f64 = 1.0e-9;
/// `big` in `displa` (`:6110`).
const BIG: f64 = 1.0e10;

/// Lobatto abscissae/weights (`qp2/qw2`, `qp6/qw6`, `qp10/qw10`, `:5877-5894`).
const QP2: [f64; 2] = [-1.0, 1.0];
const QW2: [f64; 2] = [1.0, 1.0];
const QP6: [f64; 6] = [
    -1.0,
    -0.765_055_32,
    -0.285_231_52,
    0.285_231_52,
    0.765_055_32,
    1.0,
];
const QW6: [f64; 6] = [
    0.066_666_67,
    0.378_474_96,
    0.554_858_38,
    0.554_858_38,
    0.378_474_96,
    0.066_666_67,
];
const QP10: [f64; 10] = [
    -1.0,
    -0.919_533_908_2,
    -0.738_773_865_1,
    -0.477_924_949_8,
    -0.165_278_957_7,
    0.165_278_957_7,
    0.477_924_949_8,
    0.738_773_865_1,
    0.919_533_908_2,
    1.0,
];
const QW10: [f64; 10] = [
    0.022_222_222_2,
    0.133_305_990_8,
    0.224_889_342_0,
    0.292_042_683_6,
    0.327_539_761_2,
    0.327_539_761_2,
    0.292_042_683_6,
    0.224_889_342_0,
    0.133_305_990_8,
    0.022_222_222_2,
];

/// `getsig` for the matrix path: value, next point, and `gety1`'s
/// duplicate-energy discontinuity flag (`endf.f90:2032-2037`).
fn getsig(pairs: &[(f64, f64)], e: f64) -> (f64, f64, bool) {
    // Value: lin-lin inside, zero outside (gety1 conventions).
    let n = pairs.len();
    let value = if n == 0 || e < pairs[0].0 || e > pairs[n - 1].0 {
        0.0
    } else {
        let pos = pairs.partition_point(|&(x, _)| x <= e);
        let i = pos.saturating_sub(1).min(n - 2);
        let (x1, y1) = pairs[i];
        let (x2, y2) = pairs[i + 1];
        if x2 == x1 {
            y1
        } else {
            y1 + (y2 - y1) * (e - x1) / (x2 - x1)
        }
    };
    let idx = pairs.partition_point(|&(x, _)| x <= e);
    if idx < n {
        let enext = pairs[idx].0;
        let idis = idx + 1 < n && pairs[idx + 1].0 == enext;
        (value, enext, idis)
    } else {
        (value, EMAX, false)
    }
}

/// The `save`d state of `panel` plus its `stmp/slst/ftmp/flst` arrays
/// (`:5906-5907`, `:5919-5935`).
struct PanelState {
    elast: f64,
    idisc: bool,
    enext: f64,
    nq: usize,
    nqp: usize,
    ig1: usize,
    ng1: usize,
    /// `ff(il, ig)` from the last `getff`.
    ff: Vec<Vec<f64>>,
    /// `[iz][il]`.
    slst: Vec<Vec<f64>>,
    flst: Vec<Vec<f64>>,
    stmp: Vec<Vec<f64>>,
    ftmp: Vec<Vec<f64>>,
}

impl PanelState {
    fn new(nl: usize, nz: usize) -> Self {
        PanelState {
            elast: 0.0,
            idisc: false,
            enext: 0.0,
            nq: 0,
            nqp: 0,
            ig1: 0,
            ng1: 0,
            ff: vec![Vec::new(); nl],
            slst: vec![vec![0.0; nl]; nz],
            flst: vec![vec![0.0; nl]; nz],
            stmp: vec![vec![0.0; nl]; nz],
            ftmp: vec![vec![0.0; nl]; nz],
        }
    }
}

/// `ans(il, iz, it)` stored as `ans[it][iz][il]`.
type Ans = Vec<Vec<Vec<f64>>>;

/// Fold a source's `(en, idiscf)` into `(enext, idisc)` (`:5947-5949` etc.).
fn merge_next(en: f64, idiscf: bool, enext: &mut f64, idisc: &mut bool) {
    if (en - *enext).abs() < *enext * SMALL && idiscf && !*idisc {
        *idisc = idiscf;
    }
    if en < *enext * (1.0 - SMALL) {
        *idisc = idiscf;
    }
    if en < *enext * (1.0 - SMALL) {
        *enext = en;
    }
}

/// One `panel` call (`:5858-6091`). `ehi` is the caller's `enext`, reduced
/// in place when a break falls inside; `ng`/`iglo` are the caller's
/// `ng2`/`ig2lo`.
#[allow(clippy::too_many_arguments)]
fn panel(
    st: &mut PanelState,
    pairs: &[(f64, f64)],
    fluxes: &[GroupFlux],
    feed: &mut TwoBodyFeed,
    nl: usize,
    nz: usize,
    elo_in: f64,
    ehi: &mut f64,
    ans: &mut Ans,
    ng: &mut usize,
    iglo: &mut usize,
) -> Result<(), NjoyError> {
    let mut elo = elo_in;
    let elow = elo;
    if elo > *ehi * (1.0 + SMALL) {
        return Err(NjoyError::EndfParse(format!(
            "panel: elo ({elo}) > ehi ({})",
            *ehi
        )));
    }
    let getflx = |e: f64, out: &mut Vec<Vec<f64>>| -> (f64, bool) {
        let mut en = EMAX;
        for (iz, f) in fluxes.iter().enumerate() {
            let v = f.value(e);
            for il in 0..nl {
                out[iz][il] = v;
            }
            en = en.min(f.next_break(e));
        }
        (en, false)
    };
    let getsig_into = |e: f64, out: &mut Vec<Vec<f64>>| -> (f64, bool) {
        let (s, en, idis) = getsig(pairs, e);
        for row in out.iter_mut() {
            for v in row.iter_mut() {
                *v = s;
            }
        }
        (en, idis)
    };

    // Retrieve factors in integrands at lower boundary (`:5937-5960`).
    let mut zero_panel = false;
    let mut en = EMAX;
    let mut idiscf = false;
    if (elo - st.elast).abs() >= st.elast * SMALL {
        if elo * RNDOFF < *ehi {
            elo *= RNDOFF;
        }
        st.elast = elo;
        let (enf, idf) = getflx(elo, &mut st.flst);
        let (ens, ids) = getsig_into(elo, &mut st.slst);
        st.enext = ens;
        st.idisc = ids;
        if st.enext >= EMAX * (1.0 - SMALL) {
            zero_panel = true;
        } else {
            merge_next(enf, idf, &mut st.enext, &mut st.idisc);
            let at = feed.feed(elo, nl)?;
            st.ff = at.ff;
            st.ng1 = at.ng;
            st.ig1 = at.iglo;
            st.nq = at.nq;
            merge_next(at.enext, at.idisc, &mut st.enext, &mut st.idisc);
            st.nq += 2;
            if st.nq > 10 {
                st.nq = 10;
            }
        }
    }

    let mut ehigh = 0.0;
    if !zero_panel {
        // Retrieve cross section and flux at upper boundary (`:5963-5977`).
        if st.enext < DELTA * *ehi {
            *ehi = st.enext;
            ehigh = *ehi;
            if st.idisc && *ehi * DELTA > elo {
                ehigh = *ehi * DELTA;
            }
        } else {
            ehigh = DELTA * *ehi;
        }
        let (enf, idf) = getflx(ehigh, &mut st.ftmp);
        en = enf;
        idiscf = idf;
        let (ens, ids) = getsig_into(ehigh, &mut st.stmp);
        st.enext = ens;
        st.idisc = ids;
        if st.enext > EMAX * (1.0 - SMALL) {
            // Label 110: zero cross section over entire panel?
            if st.stmp[0][0] == 0.0 {
                zero_panel = true;
            }
        } else {
            merge_next(en, idiscf, &mut st.enext, &mut st.idisc);
        }
    }

    if zero_panel {
        // Label 115.
        for row in ans[0].iter_mut() {
            row.iter_mut().for_each(|v| *v = 1.0);
        }
        for row in ans[1].iter_mut() {
            row.iter_mut().for_each(|v| *v = 0.0);
        }
        *ng = 2;
        *iglo = 1;
        en = EMAX;
        st.nqp = 0;
    } else {
        // Label 125: group fluxes, flux linear over the panel (`:5991-5999`).
        let aq = (*ehi + elow) / 2.0;
        let bq = (*ehi - elow) / 2.0;
        for iz in 0..nz {
            for il in 0..nl {
                ans[0][iz][il] += (st.ftmp[iz][il] + st.flst[iz][il]) * bq;
            }
        }
        // Lobatto quadrature over the panel (`:6002-6056`).
        let (qp, qw): (&[f64], &[f64]) = match st.nq {
            2 => (&QP2, &QW2),
            6 => (&QP6, &QW6),
            10 => (&QP10, &QW10),
            other => {
                return Err(NjoyError::EndfParse(format!("panel: bad nq {other}")));
            }
        };
        for iq in 0..st.nq {
            let mut eq = aq + bq * qp[iq];
            let wq = bq * qw[iq];
            eq = sigfig(eq, 9, 0);
            let t1 = (eq - elow) / (*ehi - elow);
            if iq > 0 {
                if eq > ehigh {
                    eq = ehigh;
                }
                let at = feed.feed(eq, nl)?;
                st.ff = at.ff;
                st.ng1 = at.ng;
                st.ig1 = at.iglo;
                st.nqp = at.nq;
                en = at.enext;
                idiscf = at.idisc;
            }
            if *iglo == 0 {
                *iglo = st.ig1;
            }
            for iz in 0..nz {
                for il in 0..nl {
                    let a = st.stmp[iz][il] * st.ftmp[iz][il];
                    let b = st.slst[iz][il] * st.flst[iz][il];
                    let rr = (b + (a - b) * t1) * wq;
                    for ig in 1..=st.ng1 {
                        let f = st.ff[il][ig - 1];
                        if f != 0.0 {
                            let igt = st.ig1 as i64 + ig as i64 - *iglo as i64 + 1;
                            // `igt <= 1` is the "thermal range problem" message
                            // path (`:6034-6045`): nothing is deposited.
                            if igt > 1 {
                                let igt = igt as usize;
                                if igt > ans.len() {
                                    return Err(NjoyError::EndfParse(format!(
                                        "panel: secondary slot {igt} exceeds ng2g {}",
                                        ans.len()
                                    )));
                                }
                                ans[igt - 1][iz][il] += rr * f;
                                if igt > *ng {
                                    *ng = igt;
                                }
                            }
                        }
                    }
                }
            }
        }
    }

    // Label 220: save for the next panel (`:6073-6089`).
    st.elast = ehigh;
    for iz in 0..nz {
        for il in 0..nl {
            st.slst[iz][il] = st.stmp[iz][il];
            st.flst[iz][il] = st.ftmp[iz][il];
        }
    }
    merge_next(en, idiscf, &mut st.enext, &mut st.idisc);
    if st.enext <= *ehi {
        st.enext = RNDOFF * *ehi;
    }
    st.nq = st.nqp + 2;
    if st.nq > 10 {
        st.nq = 10;
    }
    Ok(())
}

/// `displa` for a transfer matrix (`:6210-6335`): divide by the group
/// flux with seven-figure rounding, trim leading/trailing zero secondary
/// groups. Returns `(igzero, ig2lo, ng2)` and leaves the packed result in
/// `ans[..ng2]`.
fn displa(ans: &mut Ans, nl: usize, nz: usize, ng2: usize, ig2lo: usize) -> (bool, usize, usize) {
    let mut igzero = false;
    let mut ilo = 0usize;
    let mut ihi = ng2;
    if nz > 1 {
        // Label 310: self-shielded matrices.
        let mut result = vec![0.0f64; nz];
        for ig2 in 2..=ng2 {
            let mut izero = false;
            for il in 0..nl {
                for iz in 0..nz {
                    result[iz] = 0.0;
                    if ig2 == 2 {
                        ans[0][iz][il] = sigfig(ans[0][iz][il], 7, 0);
                    }
                    if ans[ig2 - 1][iz][il] != 0.0 {
                        if ans[0][iz][il] == 0.0 {
                            ans[0][iz][il] = BIG;
                        }
                        result[iz] = sigfig(ans[ig2 - 1][iz][il] / ans[0][iz][il], 7, 0);
                        izero = true;
                    }
                }
                if ilo == 0 && (izero || ig2 == ng2) {
                    ilo = ig2;
                }
                if ilo != 0 {
                    for iz in 0..nz {
                        ans[ig2 - ilo + 1][iz][il] = result[iz];
                    }
                    if izero {
                        ihi = ig2;
                        igzero = true;
                    }
                }
            }
        }
    } else if nl > 1 {
        // Label 210 path: infinite-dilution matrices (iz = 1).
        let mut result = vec![0.0f64; nl];
        for ig2 in 2..=ng2 {
            let mut izero = false;
            for il in 0..nl {
                result[il] = 0.0;
                if ans[ig2 - 1][0][il] != 0.0 {
                    if ans[0][0][il] == 0.0 {
                        ans[0][0][il] = BIG;
                    }
                    ans[0][0][il] = sigfig(ans[0][0][il], 7, 0);
                    result[il] = sigfig(ans[ig2 - 1][0][il] / ans[0][0][il], 7, 0);
                    if result[il].abs() >= SMIN {
                        izero = true;
                    }
                }
            }
            if ilo == 0 && (izero || ig2 == ng2) {
                ilo = ig2;
            }
            if ilo != 0 {
                ans[ig2 - ilo + 1][0][..nl].copy_from_slice(&result[..nl]);
                if izero {
                    ihi = ig2;
                    igzero = true;
                }
            }
        }
    } else {
        // Label 250: isotropic infinite-dilution matrix (il = 1, iz = 1).
        let mut j = 2usize;
        for ig2 in 2..=ng2 {
            if ilo > 0 {
                j += 1;
            }
            if ans[0][0][0] == 0.0 {
                ans[0][0][0] = BIG;
            }
            ans[0][0][0] = sigfig(ans[0][0][0], 7, 0);
            ans[j - 1][0][0] = sigfig(ans[ig2 - 1][0][0] / ans[0][0][0], 7, 0);
            if ans[j - 1][0][0].abs() < SMIN / 1000.0 {
                ans[j - 1][0][0] = 0.0;
            }
            if ans[j - 1][0][0] != 0.0 {
                if ilo == 0 {
                    ilo = ig2;
                }
                ihi = ig2;
                igzero = true;
            }
        }
        if !igzero {
            ilo = ng2;
        }
    }
    let ig2lo_out = (ig2lo as i64 + ilo as i64 - 2).max(0) as usize;
    let ng2_out = (ihi as i64 - ilo as i64 + 2).max(0) as usize;
    (igzero, ig2lo_out, ng2_out)
}

/// Identity of the GENDF section a matrix run writes.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct MatrixHeader {
    /// `MF` (6 for a neutron transfer matrix).
    pub mf: i32,
    /// `MT`.
    pub mt: i32,
    /// `ZA` (HEAD `C1`).
    pub za: f64,
    /// `ZAM` (HEAD `C2`).
    pub zam: f64,
    /// `LRFLAG` (HEAD `N1`).
    pub lrflag: i32,
    /// Temperature \[K\] (each LIST `C1`).
    pub temperature_k: f64,
    /// `emaxx` (`:425-431`): groups starting at or above it are skipped,
    /// except the last one. `2e7` unless MF=1/451 says otherwise.
    pub emaxx: f64,
}

/// The initial-group loop (`:490-580`) for a two-body transfer matrix:
/// `panel` sub-panels per group, `displa`, and the GENDF record layout
/// (`:891-929`, `it` outermost, then `iz`, then `il`).
///
/// - `sigma` — the PENDF cross section (must be [`PointwiseXs::LinLin`]);
/// - `fluxes` — one weighting flux per dilution (`nz = fluxes.len()`);
/// - `feed` — the reaction's [`TwoBodyFeed`] (its `egn` is the group
///   structure); it is reset before the walk;
/// - `nl` — Legendre orders (`lord + 1`).
///
/// # Errors
/// [`NjoyError::NotPorted`] for `nz > 1 && nl > 1`; [`NjoyError::EndfParse`]
/// for an empty flux list or a non-tabulated cross section; feed errors.
pub fn two_body_matrix(
    sigma: &PointwiseXs,
    fluxes: &[GroupFlux],
    feed: &mut TwoBodyFeed,
    nl: usize,
    header: &MatrixHeader,
) -> Result<GendfSection, NjoyError> {
    let nz = fluxes.len();
    if nz == 0 || nl == 0 {
        return Err(NjoyError::EndfParse(
            "two_body_matrix: need >= 1 flux and >= 1 Legendre order".into(),
        ));
    }
    if nz > 1 && nl > 1 {
        return Err(NjoyError::NotPorted(
            "groupr::matrix_panel P_l flux components for nz > 1 (genflx fout(l-1)*fac, groupr.f90:5605-5608)",
        ));
    }
    let PointwiseXs::LinLin(pairs) = sigma else {
        return Err(NjoyError::EndfParse(
            "two_body_matrix: cross section must be tabulated (gety1 semantics)".into(),
        ));
    };
    let pairs: &[(f64, f64)] = pairs;
    if pairs.is_empty() {
        return Err(NjoyError::EndfParse(
            "two_body_matrix: empty cross section".into(),
        ));
    }
    let first = pairs[0].0;
    let egn = feed.egn().to_vec();
    let ngn = egn.len() - 1;
    let ng2g = ngn + 1;
    feed.reset();
    let mut st = PanelState::new(nl, nz);
    let mut records = Vec::new();
    for ig in 1..=ngn {
        let mut elo = egn[ig - 1];
        let ehi = egn[ig];
        let mut ig2lo = 0usize;
        let mut ng2 = 2usize;
        if ehi <= first {
            continue;
        }
        if elo >= header.emaxx && ig != ngn {
            continue;
        }
        let mut enext = ehi;
        let mut ans: Ans = vec![vec![vec![0.0; nl]; nz]; ng2g + 1];
        loop {
            panel(
                &mut st, pairs, fluxes, feed, nl, nz, elo, &mut enext, &mut ans, &mut ng2,
                &mut ig2lo,
            )?;
            if enext == ehi {
                break;
            }
            elo = enext;
            enext = ehi;
        }
        let (igzero, ig2lo_out, ng2_out) = displa(&mut ans, nl, nz, ng2, ig2lo);
        if igzero || ig == ngn {
            let mut data = Vec::with_capacity(nl * nz * ng2_out);
            for slot in ans.iter().take(ng2_out) {
                for row in slot.iter() {
                    data.extend_from_slice(row);
                }
            }
            records.push(GendfGroupRecord {
                ig: ig as i32,
                ig2lo: ig2lo_out as i32,
                ng2: ng2_out as i32,
                data,
            });
        }
    }
    Ok(GendfSection {
        mf: header.mf,
        mt: header.mt,
        za: header.za,
        zam: header.zam,
        nl: nl as i32,
        nz: nz as i32,
        lrflag: header.lrflag,
        num_groups: ngn as i32,
        temperature_k: header.temperature_k,
        records,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::endf::tape::{Section, Tape};
    use crate::endf::EndfKey;
    use crate::groupr::file4::{File4Angular, NLD};
    use std::sync::Arc;

    fn isotropic_angular() -> File4Angular {
        let rows = vec![
            [1001.0, 0.9992, 0.0, 1.0, 0.0, 0.0],
            [0.0, 0.9992, 0.0, 2.0, 0.0, 0.0],
            [0.0, 0.0, 0.0, 0.0, 1.0, 2.0],
            [2.0, 2.0, 0.0, 0.0, 0.0, 0.0],
            [0.0, 1.0e-5, 0.0, 0.0, 1.0, 0.0],
            [0.0, 0.0, 0.0, 0.0, 0.0, 0.0],
            [0.0, 2.0e7, 0.0, 0.0, 1.0, 0.0],
            [0.0, 0.0, 0.0, 0.0, 0.0, 0.0],
        ];
        let tape = Tape::from_sections(
            String::new(),
            vec![Section {
                key: EndfKey {
                    mat: 125,
                    mf: 4,
                    mt: 2,
                },
                rows,
            }],
        );
        File4Angular::from_tape(&tape, 125, 2, NLD).unwrap()
    }

    /// Methodology: a constant 4-barn elastic cross section, a flat flux and
    /// a heavy target (`awr = 100`, `alpha = 0.960788`) on decade-wide
    /// groups. Only incident energies within `[egn, egn/alpha]` of a group's
    /// lower edge scatter below it; with a flat flux the lost fraction is
    /// `(ln(1/alpha)/(1-alpha) - 1)/9 = 0.00223711` of the group (scale
    /// invariant), so every group has one down-scatter element
    /// `4 * 0.00223711 = 0.00894845` barn and a diagonal of `4` minus that.
    /// Prediction: `ng2 = 3`, `ig2lo = ig - 1` for `ig >= 2`; for the bottom
    /// group `ng2 = 2`, `ig2lo = 1` and the full 4 barn stay on the diagonal,
    /// because `getdis` starts its first band at `w = -1` (`:9455`) so
    /// nothing scatters out below the lowest boundary;
    /// elements within 1e-5 relative of the analytic values (the 10-point
    /// Lobatto rule on the `1/E`-shaped feed and the seven-figure rounding),
    /// group flux `ehi - elo` within 1e-5. Result (2026-09-10): down-scatter
    /// 0.008948458 vs 0.008948447 analytic (1.3e-6).
    #[test]
    fn constant_xs_heavy_target_is_diagonal() {
        let egn = [1.0, 10.0, 100.0, 1000.0];
        let awr = 100.0f64;
        let alpha = ((awr - 1.0) / (awr + 1.0)).powi(2);
        let lost = 4.0 * ((1.0 / alpha).ln() / (1.0 - alpha) - 1.0) / 9.0;
        let mut feed = TwoBodyFeed::new(isotropic_angular(), &egn, awr, 0.0, 0).unwrap();
        let sigma = PointwiseXs::LinLin(Arc::new(vec![(1.0e-5, 4.0), (2.0e7, 4.0)]));
        let flux = [GroupFlux::Flat];
        let header = MatrixHeader {
            mf: 6,
            mt: 2,
            za: 1001.0,
            zam: 0.0,
            lrflag: 0,
            temperature_k: 0.0,
            emaxx: 2.0e7,
        };
        let sec = two_body_matrix(&sigma, &flux, &mut feed, 1, &header).unwrap();
        assert_eq!(sec.records.len(), 3, "{sec:?}");
        for (k, rec) in sec.records.iter().enumerate() {
            assert_eq!(rec.ig, k as i32 + 1);
            let width = egn[k + 1] - egn[k];
            assert!(((rec.data[0] - width) / width).abs() < 1e-5, "{rec:?}");
            let diag = 4.0 - lost;
            if k == 0 {
                assert_eq!((rec.ig2lo, rec.ng2), (1, 2), "{rec:?}");
                assert!((rec.data[1] - 4.0).abs() < 1e-5, "{rec:?}");
            } else {
                assert_eq!((rec.ig2lo, rec.ng2), (k as i32, 3), "{rec:?}");
                assert!(
                    ((rec.data[1] - lost) / lost).abs() < 1e-5,
                    "{rec:?} vs {lost}"
                );
                assert!(((rec.data[2] - diag) / diag).abs() < 1e-5, "{rec:?}");
            }
        }
    }

    /// Methodology: hydrogen (`alpha = 0`) with an isotropic CM distribution
    /// scatters uniformly in `E' in [0, E]`, so with a flat flux and a
    /// constant cross section the P0 row of the top group sums to the cross
    /// section and every lower group receives a positive share. Prediction:
    /// row sum 4 within 1e-5 relative, `ig2lo = 1`, all elements > 0.
    /// Result (2026-09-10): as predicted.
    #[test]
    fn hydrogen_row_sums_to_cross_section() {
        let egn = [1.0e-5, 1.0, 10.0, 100.0];
        let mut feed = TwoBodyFeed::new(isotropic_angular(), &egn, 0.9992, 0.0, 0).unwrap();
        let sigma = PointwiseXs::LinLin(Arc::new(vec![(1.0e-5, 4.0), (2.0e7, 4.0)]));
        let flux = [GroupFlux::Flat];
        let header = MatrixHeader {
            mf: 6,
            mt: 2,
            za: 1001.0,
            zam: 0.0,
            lrflag: 0,
            temperature_k: 0.0,
            emaxx: 2.0e7,
        };
        let sec = two_body_matrix(&sigma, &flux, &mut feed, 1, &header).unwrap();
        let top = sec.records.last().unwrap();
        assert_eq!(top.ig, 3);
        assert_eq!(top.ig2lo, 1, "{top:?}");
        assert_eq!(top.ng2, 4, "{top:?}");
        let sum: f64 = top.data[1..].iter().sum();
        assert!(((sum - 4.0) / 4.0).abs() < 1e-5, "{top:?}");
        assert!(top.data[1..].iter().all(|&v| v > 0.0), "{top:?}");
    }
}
