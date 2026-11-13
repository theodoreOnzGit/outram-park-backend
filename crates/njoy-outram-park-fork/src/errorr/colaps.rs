// SPDX-License-Identifier: AGPL-3.0-only
// Translated from NJOY2016.80 `src/errorr.f90` (git commit ac5adf5f33d893e42f2eed7fb286b0d51c7580da):
//   - `subroutine colaps`, l.9097-9533 (GENDF to the union grid);
//   - `subroutine rdgout` / `rdsig` / `rdchi`, l.2755-2937 and l.6195-6225 (reading it back).
// NJOY2016 is under a modified BSD 3-Clause (LANL/DOE) licence, GPL-compatible;
// this derivative file is distributed under AGPL-3.0-only. This is a modified,
// non-LANL version, not endorsed by LANL/DOE. See crate root LICENSE.njoy + NOTICE.

//! **`colaps`**: ERRORR's group cross sections from an input GENDF (gh:#1369).
//!
//! With `ngout /= 0` ERRORR does not average the PENDF (`grpav`); it collapses (or
//! expands) the GENDF's groups onto the union grid `un`, assuming the cross section
//! and the flux are constant inside an input group (`errorr.f90:9097-9533`). The
//! result is upstream's scratch tape `ntp = -10`, binary, so every word here keeps
//! full precision.
//!
//! Upstream idioms kept as written:
//! - a union group with `sigb = 0` writes no record (`:9258`), so reading it back
//!   gives 0 (`rdgout` zeroes `sig` first);
//! - the chi translation tests `er <= ea3` (`:9461`), where `er` and `ea3` are the
//!   values the **last MF=3 section** left behind. [`ColapsState`] carries them.

use crate::NjoyError;

use super::gendf::{sigfig_bounds, CondensedGendf, CondensedSection};

/// The `ntp` tape `colaps` writes, as ERRORR reads it back.
#[derive(Debug, Clone, PartialEq)]
pub struct UnionGendf {
    /// The temperature `colaps` takes from the GENDF (`tempin = c1h`, `:9171`).
    pub tempin: f64,
    /// `ela`: the GENDF bounds, `sigfig`'d to 6 figures.
    pub ela: Vec<f64>,
    /// MF=3 sections: `(mt, records)` with `records[ib-1] = (flux, nu, sig)` for each
    /// union group a record was written for.
    pub mf3: Vec<(i32, Vec<Option<[f64; 3]>>)>,
    /// MF=5/MT=18 on the union grid: `(flux, chi)` per union group (`:9500-9520`).
    pub chi: Option<Vec<(f64, f64)>>,
}

impl UnionGendf {
    /// `rdgout(mfd = 3, mti = -1)`: the flux on the union grid from MT=1's records
    /// (`:2859-2930`, `ib = if`), 0 where no record was written.
    ///
    /// # Errors
    /// [`NjoyError::NotPorted`] when MT=1 is absent (upstream's `mti = -2` union of
    /// the partial fluxes is not translated).
    pub fn flux(&self, nunion: usize) -> Result<Vec<f64>, NjoyError> {
        let (_, recs) = self
            .mf3
            .iter()
            .find(|(mt, _)| *mt == 1)
            .ok_or(NjoyError::NotPorted("errorr colaps: GENDF without MT=1 (rdgout mti = -2)"))?;
        Ok((0..nunion).map(|i| recs.get(i).copied().flatten().map_or(0.0, |r| r[0])).collect())
    }

    /// `rdchi` (`:6195-6225`): chi per union group from the MF=5/MT=18 records.
    ///
    /// # Errors
    /// [`NjoyError::EndfParse`] when `colaps` wrote no chi.
    pub fn chi_vector(&self) -> Result<Vec<f64>, NjoyError> {
        self.chi
            .as_ref()
            .map(|c| c.iter().map(|&(_, x)| x).collect())
            .ok_or_else(|| NjoyError::EndfParse("errorr rdchi: no MF=5/MT=18 on the collapsed tape".into()))
    }
}

/// The variables `colaps` keeps across sections (`er`, `ea3`).
#[derive(Debug, Clone, Copy, Default)]
struct ColapsState {
    er: f64,
    ea3: f64,
}

/// One MF=3 section onto the union grid (`:9213-9277`).
fn collapse_mf3(sec: &CondensedSection, ela: &[f64], un: &[f64], st: &mut ColapsState) -> Result<Vec<Option<[f64; 3]>>, NjoyError> {
    let nunion = un.len() - 1;
    let inuf = usize::from(matches!(sec.mt, 452 | 455 | 456));
    let mt = sec.mt;
    let short = move || NjoyError::EndfParse(format!("errorr colaps: MF=3/MT={mt} ends before the union grid"));
    let mut next = 0usize;
    // `listio`: the next group record; `(flxa, xnua, siga, ea2, ea3)` as upstream sets them.
    let read = |next: &mut usize| -> Result<(f64, f64, f64, f64, f64), NjoyError> {
        let r = sec.groups.get(*next).ok_or_else(short)?;
        *next += 1;
        let w = |i: usize| r.words.get(i).copied().unwrap_or(0.0);
        let jg = r.ig.max(1) as usize;
        let lo = *ela.get(jg - 1).ok_or_else(short)?;
        let hi = *ela.get(jg).ok_or_else(short)?;
        Ok((w(0), w(1), w(1 + inuf), lo, hi))
    };
    let (mut flxa, mut xnua, mut siga, mut ea2) = (0.0, 0.0, 0.0, 0.0);
    st.ea3 = 0.0;
    while st.ea3 <= un[0] {
        let (f, x, s, lo, hi) = read(&mut next)?;
        flxa = f;
        xnua = x;
        siga = s;
        ea2 = lo;
        st.ea3 = hi;
    }
    let mut out = vec![None; nunion];
    let mut ib = 0usize;
    loop {
        // label 230
        ib += 1;
        let (mut xnub, mut sigb, mut flxb) = (0.0f64, 0.0f64, 0.0f64);
        let mut el = un[ib - 1];
        // label 250
        loop {
            if ea2 > un[ib] {
                break;
            }
            if ea2 > el {
                el = ea2;
            }
            st.er = un[ib];
            if st.ea3 < st.er {
                st.er = st.ea3;
            }
            let flux = flxa * (st.er - el) / (st.ea3 - ea2);
            xnub += siga * xnua * flux;
            sigb += siga * flux;
            flxb += flux;
            if st.ea3 > un[ib] {
                break;
            }
            if st.ea3 == un[ib] && ib >= nunion {
                break;
            }
            let (f, x, s, lo, hi) = read(&mut next)?;
            flxa = f;
            xnua = x;
            siga = s;
            let ea1 = st.ea3;
            ea2 = lo;
            st.ea3 = hi;
            el = ea1;
            if ea1 == un[ib] {
                break;
            }
        }
        // label 280
        if sigb == 0.0 && ib < nunion {
            continue;
        }
        let mut rec = [flxb, 0.0, 0.0];
        if sigb != 0.0 && flxb != 0.0 {
            rec[1] = xnub / sigb;
            rec[1 + inuf] = sigb / flxb;
        }
        out[ib - 1] = Some(rec);
        if ib >= nunion {
            break;
        }
    }
    Ok(out)
}

/// The MF=5/MT=18 chi of a `mffis = 5` GENDF onto the union grid (`:9278-9290`,
/// `:9443-9520`): `scr18` is MF=3/MT=18's flux per GENDF group, `chi` the MF=5
/// vector.
fn collapse_chi(cg: &CondensedGendf, ela: &[f64], un: &[f64], st: ColapsState, mffis: i32, win: &FissionWindow) -> Result<Vec<(f64, f64)>, NjoyError> {
    let chi = if mffis == 6 { mf6_chi(cg, ela, win)? } else { Vec::new() };
    collapse_chi_with(cg, ela, un, st, (mffis == 6).then_some(chi))
}

/// The spectrum `colaps` builds from a GROUPR MF=6/MT=18 matrix (`:9300-9440`):
/// the `ig = 0` isotropic record times `nu sigf` for the groups it covers, plus
/// each matrix row, each weighted by its flux inside the subsection's
/// `[eltst, ehtst]` window that contains `efmean`, then normalised.
fn mf6_chi(cg: &CondensedGendf, ela: &[f64], win: &FissionWindow) -> Result<Vec<f64>, NjoyError> {
    let ng = ela.len() - 1;
    let s6 = cg.section(6, 18).ok_or_else(|| NjoyError::EndfParse("errorr colaps: no MF=6/MT=18".into()))?;
    let mut recs = s6.groups.iter();
    let first = recs.next().ok_or_else(|| NjoyError::EndfParse("errorr colaps: empty MF=6/MT=18".into()))?;
    if first.ig != 0 {
        return Err(NjoyError::NotPorted("errorr colaps: GROUPR MF=6 fission matrix without an ig = 0 spectrum (not ready for file6)"));
    }
    let mut iso = vec![0.0f64; ng];
    for (i, &v) in first.words.iter().enumerate() {
        let idx = first.ig2lo as i64 - 1 + i as i64; // 0-based iso(ig2lo-1+i)
        if idx >= 0 && (idx as usize) < ng {
            iso[idx as usize] = v;
        }
    }
    let mut acc = vec![0.0f64; ng];
    let (mut eltst, mut ehtst) = (win.eclo, win.echi);
    let mut jg = 0i64;
    while jg < ng as i64 {
        let r = recs.next().ok_or_else(|| NjoyError::EndfParse("errorr colaps: MF=6/MT=18 ends early".into()))?;
        jg = i64::from(r.ig);
        let w = |i: usize| r.words.get(i - 1).copied().unwrap_or(0.0);
        let mut flxa = w(1);
        let j = jg.max(1) as usize;
        let mut ea1 = *ela.get(j - 1).ok_or_else(|| NjoyError::EndfParse("errorr colaps: MF=6 group off the grid".into()))?;
        let mut ea2 = *ela.get(j).ok_or_else(|| NjoyError::EndfParse("errorr colaps: MF=6 group off the grid".into()))?;
        if win.igflag != 0 {
            eltst = ea1;
            ehtst = ea2;
        }
        if win.efmean >= ehtst || win.efmean <= eltst {
            flxa = 0.0;
        } else {
            let ea12 = ea2 - ea1;
            if ea1 < eltst {
                ea1 = eltst;
            }
            if ea2 > ehtst {
                ea2 = ehtst;
            }
            flxa = flxa * (ea2 - ea1) / ea12;
        }
        if r.ig2lo == 0 {
            let sfnuf = w(2);
            for (a, &s) in acc.iter_mut().zip(&iso) {
                *a += s * flxa * sfnuf;
            }
        } else {
            for i in 2..=r.words.len() {
                let idx = r.ig2lo as i64 + i as i64 - 3; // 0-based acc(ig2lo+i-2)
                if idx >= 0 && (idx as usize) < ng {
                    acc[idx as usize] += w(i) * flxa;
                }
            }
        }
        if ea1 > win.efmean {
            jg = ng as i64 + 1;
        }
    }
    let sfnuf: f64 = acc.iter().fold(0.0, |s, &x| s + x);
    Ok(acc.iter().map(|&x| x / sfnuf).collect())
}

/// The window `colaps` weights an MF=6 fission matrix in (`eclo`, `echi`,
/// `efmean`, `igflag` from the MF=35 subsection scan, `errorr.f90:571-640`).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct FissionWindow {
    /// `eclo` (eV).
    pub eclo: f64,
    /// `echi` (eV).
    pub echi: f64,
    /// `efmean` (eV).
    pub efmean: f64,
    /// `igflag`.
    pub igflag: i32,
}

fn collapse_chi_with(cg: &CondensedGendf, ela: &[f64], un: &[f64], st: ColapsState, mf6: Option<Vec<f64>>) -> Result<Vec<(f64, f64)>, NjoyError> {
    let ng = ela.len() - 1;
    let mut scr18 = vec![0.0; ng + 1];
    let s3 = cg.section(3, 18).ok_or_else(|| NjoyError::EndfParse("errorr colaps: no MF=3/MT=18 for chi".into()))?;
    for r in &s3.groups {
        let jg = r.ig.max(1) as usize;
        if let Some(v) = scr18.get_mut(jg - 1) {
            *v = r.words.first().copied().unwrap_or(0.0);
        }
    }
    let chi: Vec<f64> = match mf6 {
        Some(c) => c,
        None => {
            let s5 = cg.section(5, 18).ok_or_else(|| NjoyError::EndfParse("errorr colaps: no MF=5/MT=18".into()))?;
            // one LIST: `scr(jg+6)` is group jg's chi whatever `ig2lo` says
            s5.groups.first().map(|r| r.words.clone()).unwrap_or_default()
        }
    };
    let chi_at = |jg: usize| chi.get(jg - 1).copied().unwrap_or(0.0);
    let nunion = un.len() - 1;
    let mut out = Vec::with_capacity(nunion);
    let mut jg = 0usize;
    let mut egu = 0.0f64;
    let bound = |j: usize| -> Result<f64, NjoyError> {
        ela.get(j - 1).copied().ok_or_else(|| NjoyError::EndfParse("errorr colaps: chi past the GENDF groups".into()))
    };
    for ib in 1..=nunion {
        let mut enl = un[ib - 1];
        let enu = un[ib];
        while egu <= enl {
            jg += 1;
            egu = bound(jg + 1)?;
        }
        let mut egl = bound(jg)?;
        let mut dea = egu - egl;
        let mut flxg = scr18[jg - 1];
        let mut chig = chi_at(jg);
        let (chiu, flxu);
        if st.er <= st.ea3 {
            let flux = flxg * (enu - enl) / dea;
            chiu = chig * flux;
            flxu = flux;
        } else {
            let mut enuu = egu;
            let flux = flxg * (enuu - enl) / dea;
            let (mut c, mut f) = (chig * flux, flux);
            while egu <= enu {
                jg += 1;
                enl = enuu;
                egl = egu;
                egu = bound(jg + 1)?;
                dea = egu - egl;
                flxg = scr18[jg - 1];
                chig = chi_at(jg);
                enuu = if egu <= enu { egu } else { enu };
                let flux = flxg * (enuu - enl) / dea;
                c += chig * flux;
                f += flux;
            }
            chiu = c;
            flxu = f;
        }
        let _ = egl;
        out.push((flxu, if flxu != 0.0 { chiu / flxu } else { 0.0 }));
    }
    Ok(out)
}

/// `colaps` (`errorr.f90:9097-9533`) of the condensed GENDF onto `un`, for a
/// fission spectrum file `mffis` (0, 5, or 6 with the MF=35 window `win`).
///
/// # Errors
/// [`NjoyError::EndfParse`] when the GENDF does not span the union grid or a
/// section ends early; [`NjoyError::NotPorted`] for an MF=6 matrix without its
/// `ig = 0` spectrum record (upstream: "not ready for file6").
pub fn colaps(cg: &CondensedGendf, un: &[f64], mffis: i32, win: &FissionWindow) -> Result<UnionGendf, NjoyError> {
    let ela = sigfig_bounds(&cg.egn);
    if ela.first().copied().unwrap_or(f64::MAX) > un[0] {
        return Err(NjoyError::EndfParse("errorr colaps: ngout group structure does not span union grid".into()));
    }
    let mut st = ColapsState::default();
    let mut mf3 = Vec::new();
    let mut chi = None;
    for sec in &cg.sections {
        if mffis > 0 && sec.mf > mffis {
            break;
        }
        if sec.mf == mffis {
            if sec.mt == 18 {
                chi = Some(collapse_chi(cg, &ela, un, st, mffis, win)?);
            }
            continue;
        }
        if sec.mf > 3 {
            continue;
        }
        if sec.head.n[0] != 1 || sec.head.n[1] != 1 {
            return Err(NjoyError::EndfParse("errorr colaps: not coded for multiple sigma zeroes or legendre orders".into()));
        }
        mf3.push((sec.mt, collapse_mf3(sec, &ela, un, &mut st)?));
    }
    Ok(UnionGendf { tempin: cg.temp, ela, mf3, chi })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::errorr::gendf::{GendfKind, GendfRecord, GroupRecord};

    fn sec(mf: i32, mt: i32, groups: Vec<GroupRecord>) -> CondensedSection {
        let mut head = GendfRecord::marker(GendfKind::Cont, 1, mf);
        head.mt = mt;
        head.n = [1, 1, 0, 2];
        CondensedSection { mf, mt, head, groups }
    }

    /// Two GENDF groups split onto a three-group union grid: the flux divides by
    /// width, the cross section is carried, and chi follows its group.
    #[test]
    fn union_grid_splits_flux_by_width() {
        let g = |ig: i32, w: Vec<f64>| GroupRecord { ng2: 2, ig2lo: 1, ig, words: w };
        let cg = CondensedGendf {
            tpid: String::new(),
            za_awr: [1.0, 1.0],
            temp: 300.0,
            egn: vec![1.0, 2.0, 4.0],
            sections: vec![
                sec(3, 1, vec![g(1, vec![10.0, 3.0]), g(2, vec![20.0, 5.0])]),
                sec(3, 18, vec![g(1, vec![10.0, 1.0]), g(2, vec![20.0, 1.0])]),
                sec(5, 18, vec![GroupRecord { ng2: 2, ig2lo: 1, ig: 2, words: vec![0.25, 0.75] }]),
            ],
        };
        let win = FissionWindow { eclo: 1.0, echi: 4.0, efmean: 2.5, igflag: 0 };
        let un = sigfig_bounds(&[1.0, 2.0, 3.0, 4.0]);
        let u = colaps(&cg, &un, 5, &win).unwrap();
        let f = u.flux(3).unwrap();
        let close = |a: &[f64], b: &[f64]| a.iter().zip(b).all(|(x, y)| (x - y).abs() <= 1e-12 * y.abs());
        assert!(close(&f, &[10.0, 10.0, 10.0]), "{f:?}");
        let mt1 = &u.mf3[0].1;
        assert!(close(&[mt1[0].unwrap()[1], mt1[2].unwrap()[1]], &[3.0, 5.0]));
        assert!(close(&u.chi_vector().unwrap(), &[0.25, 0.75, 0.75]));
        // mffis = 6 with no MF=6 section: MF=5 is skipped and no chi is written
        assert!(colaps(&cg, &un, 6, &win).unwrap().chi.is_none());
    }
}
