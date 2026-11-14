// SPDX-License-Identifier: AGPL-3.0-only
// Translated from NJOY2016.80 `src/powr.f90` (git commit ac5adf5f33d893e42f2eed7fb286b0d51c7580da):
//   - `therm` (l.1544-1912) and `packa` (l.1955-1990). `tprnt` (l.1914-1953)
//     prints to the listing only and is not translated.
// NJOY2016 is under a modified BSD 3-Clause (LANL/DOE) licence, GPL-compatible;
// this derivative file is distributed under AGPL-3.0-only. This is a modified,
// non-LANL version, not endorsed by LANL/DOE. See crate root LICENSE.njoy + NOTICE.

//! `lib = 2`: the EPRI-CELL thermal library (input for LIBRAR), one block
//! per card-3 material and temperature, written straight to `nout`.
//!
//! Upstream quirks kept: the GENDF's label is read with `contio`, so a label
//! that does not read as six `e11.0` numbers (GROUPR's title, such as
//! `'94-pu-238'`) stops the run, as gfortran does; a card 3 with `matd < 0` does not end the run (only
//! `matd = 0` does) and then stops on "not found"; the group bounds `grp`
//! are taken from the first material only (`igprnt`); a packed row with no
//! element above `1e-6` still reports `ngi = 2` (`packa`).

use super::fmt::{chars, f, i, ints, six_e12_5};
use super::reader::{Reader, NWSCR};
use super::ThermMaterial;
use crate::NjoyError;

fn err(msg: String) -> NjoyError {
    NjoyError::InvalidInput(format!("therm: {msg}"))
}

#[derive(Clone, Copy)]
enum T {
    L115,
    L120,
    L130,
    L152,
    L220,
}

/// `therm(iprint,igprnt)` (`powr.f90:1544-1912`) on the materials of cards
/// 3-5. Returns the text written on `nout`.
///
/// # Errors
/// Upstream's `error` stops ("p0 flux not found.", "ngn is not equal to
/// ngnd.", "mat=... temp .. not found", "storage exceeded.").
pub fn therm(r: &mut Reader, mats: &[ThermMaterial]) -> Result<String, NjoyError> {
    let watt = 1.0e13f64;
    r.ngnd = 35;
    let ngnd = 35usize;
    let ngndsq = ngnd * ngnd;
    let mut s0 = vec![0.0f64; ngnd + 1];
    let mut s1 = vec![0.0f64; ngnd + 1];
    let mut xa = vec![0.0f64; ngnd + 1];
    let mut xf = vec![0.0f64; ngnd + 1];
    let mut b = vec![0.0f64; ngndsq + 1];
    let mut p = vec![0.0f64; ngndsq + 1];
    let ngnd1 = ngnd + 1;
    let mut grp = vec![0.0f64; ngnd1 + 1];
    let il = 1usize;
    let lz = 6usize;
    let mut valu = [0.0f64; 21];
    let mut igi = [0i32; 36];
    let mut ngi = [0i32; 36];
    let mut igprnt = 0;
    let mut out: Vec<String> = Vec::new();
    for m in mats {
        if m.matd == 0 {
            break;
        }
        let matd = m.matd;
        let idtemp = m.idtemp;
        let aname = chars(&m.idname, 12);
        let (itrc, mti, mtc) = (m.itrc, m.mti, m.mtc);
        let z = m.z;
        valu[2] = z[0];
        valu[3] = z[1];
        valu[4] = z[2];
        valu[5] = z[3];
        valu[6] = z[4] * watt;
        valu[7] = z[5] * watt;
        valu[8] = 0.0;
        valu[9] = z[6];
        valu[10] = z[7];

        // read data for desired nuclide
        let mut i318 = 0;
        let mut mf3mt2 = 0;
        for k in 1..=ngndsq {
            b[k] = 0.0;
            p[k] = 0.0;
        }
        let mut iflx0 = 0;
        let mut iflx1 = 0;
        r.repoz();
        // upstream reads the tape label with contio, not tpidio
        if !r.label_reads_as_cont() {
            return Err(err("the GENDF label is read by contio (6e11.0): Bad value during floating point read".into()));
        }
        r.contio(1)?;
        let mut ifound = 0;
        let (mut nl, mut nz) = (0usize, 0usize);
        let (mut idm, mut idma, mut idb, mut ida, mut idf, mut id1, mut id2, mut id3, mut id4, mut id5) =
            (0, 0, 0, 0, 0, 0, 0, 0, 0, 0);
        let mut nlm = 0usize;
        let mut lab = T::L115;
        loop {
            match lab {
                T::L115 => {
                    r.contio(1)?;
                    lab = if r.h.math < 0 {
                        T::L220
                    } else if r.h.math == 0 || r.h.mfh == 0 || r.h.mth == 0 {
                        T::L115
                    } else if r.h.math == matd {
                        T::L120
                    } else if ifound > 0 {
                        T::L220
                    } else {
                        r.tomend()?;
                        T::L115
                    };
                }
                T::L120 => {
                    nl = r.h.l1h.max(0) as usize;
                    nz = r.h.l2h.max(0) as usize;
                    let (mfh, mth) = (r.h.mfh, r.h.mth);
                    lab = if mfh == 3 || (mfh == 6 && mth == mti) || (mfh == 6 && mth == mtc && mtc != 0) {
                        T::L152
                    } else if mfh == 1 {
                        T::L130
                    } else {
                        r.tosend()?;
                        T::L115
                    };
                }
                T::L130 => {
                    let iza = r.h.c1h.round() as i32;
                    let nuclid = iza * 10;
                    let awr = r.h.c2h;
                    let nsigz = r.h.l2h.max(0) as usize;
                    let ntw = r.h.n2h.max(0) as usize;
                    let (mut nb, mut nw) = r.listio(1)?;
                    let itemp = r.h.c1h.round() as i32;
                    if idtemp != itemp {
                        if ifound > 0 {
                            lab = T::L220;
                            continue;
                        }
                        r.tomend()?;
                        lab = T::L115;
                        continue;
                    }
                    // 132
                    ifound = 1;
                    idm = nuclid;
                    idma = idtemp;
                    idb = 0;
                    ida = 0;
                    idf = 0;
                    id1 = 0;
                    id2 = 0;
                    id3 = 0;
                    id4 = 0;
                    id5 = 0;
                    nlm = 0;
                    let mut ia = 1usize;
                    while nb != 0 {
                        ia += nw;
                        (nb, nw) = r.moreio(ia, nb)?;
                        if ia + nw - 1 > NWSCR {
                            return Err(err("storage exceeded.".into()));
                        }
                    }
                    valu[1] = awr;
                    r.ngn = r.h.l1h;
                    if r.ngn != r.ngnd && r.ngn != r.ngnf {
                        return Err(err("ngn is not equal to ngnd.".into()));
                    }
                    if igprnt <= 0 {
                        let ia = 1 + lz + ntw + nsigz;
                        let lim = (r.ngn + 1) as usize;
                        if r.ngn == r.ngnd {
                            for k in 1..=lim {
                                grp[k] = r.sc(k - 1 + ia);
                            }
                        } else {
                            for k in 1..=lim {
                                let ig = r.icgrp(k as i32);
                                if ig != 0 {
                                    grp[ig as usize] = r.sc(ia + k - 1);
                                }
                            }
                        }
                        igprnt = 1;
                    }
                    for k in 1..=ngnd {
                        r.cflux[k] = 0.0;
                        r.cflux[k + ngnd] = 0.0;
                        s0[k] = 0.0;
                        s1[k] = 0.0;
                        xa[k] = 0.0;
                        xf[k] = 0.0;
                    }
                    lab = T::L115;
                }
                T::L152 => {
                    let (mut nb, mut nw) = r.listio(1)?;
                    let mut ia = 1usize;
                    while nb != 0 {
                        ia += nw;
                        (nb, nw) = r.moreio(ia, nb)?;
                        if ia + nw - 1 > NWSCR {
                            return Err(err("storage exceeded.".into()));
                        }
                    }
                    let ig = r.h.n2h;
                    let igc = r.icgrp(ig);
                    if !(mf3mt2 > 0 || iflx0 == 0 || (r.h.mfh == 3 && r.h.mth == 1)) {
                        mf3mt2 = 1;
                        for k in 1..=ngnd {
                            if r.cflux[k] != 0.0 {
                                r.cflux[k] = 1.0 / r.cflux[k];
                            }
                            if r.cflux[ngnd + k] != 0.0 {
                                r.cflux[ngnd + k] = 1.0 / r.cflux[ngnd + k];
                            }
                        }
                    }
                    // 160
                    let mth = r.h.mth;
                    if r.h.mfh != 6 {
                        if !(igc == 0 || igc > ngnd as i32) {
                            let g = igc as usize;
                            if mth == 1 {
                                // 164: coarse group flux
                                r.cflux[g] += r.sc(1 + lz);
                                iflx0 = 1;
                                if nl > 1 {
                                    r.cflux[ngnd + g] += r.sc(lz + 2);
                                    iflx1 = 1;
                                }
                            } else {
                                if mth > 1 && iflx0 == 0 {
                                    return Err(err("p0 flux not found.".into()));
                                }
                                if mth == 18 || (mth == 19 && i318 == 0) {
                                    // 170: fission
                                    idf = 1;
                                    if mth == 18 {
                                        i318 = 1;
                                    }
                                    xf[g] += r.sc(1 + lz + nl * nz) * r.sc(1 + lz) * r.cflux[g];
                                } else if (102..=150).contains(&mth) {
                                    // 180: capture
                                    xa[g] += r.sc(1 + lz + nl * nz) * r.sc(1 + lz) * r.cflux[g];
                                } else {
                                    r.tosend()?;
                                    lab = T::L115;
                                    continue;
                                }
                            }
                        }
                        // 166
                        lab = if ig < r.ngn { T::L152 } else { T::L115 };
                        continue;
                    }
                    // 185: scattering matrix
                    if ida <= 0 {
                        ida = 1;
                    }
                    let ig2lo = r.h.l2h;
                    let ng2 = r.h.l1h.max(0) as usize;
                    if nl > nlm {
                        nlm = nl;
                    }
                    if igc != 0 && igc <= ngnd as i32 {
                        let g = igc as usize;
                        for k in 2..=ng2 {
                            let loca = 1 + (il - 1) + lz + nl * nz * (k - 1);
                            let ig2 = ig2lo + k as i32 - 2;
                            let ig2c = r.icgrp(ig2);
                            if ig2c != 0 && ig2c <= ngnd as i32 {
                                let loc = ig2c as usize + ngnd * (g - 1);
                                p[loc] += r.sc(loca) * r.sc(1 + lz) * r.cflux[g];
                                s0[g] += r.sc(loca) * r.sc(1 + lz) * r.cflux[g];
                                if nl != 1 {
                                    let ndexc = if iflx1 == 0 { g } else { ngnd + g };
                                    let ndexf = if iflx1 == 0 { 1 + lz } else { 1 + lz + 1 };
                                    s1[g] += r.sc(loca + 1) * r.sc(ndexf) * r.cflux[ndexc];
                                }
                            }
                        }
                    }
                    lab = if ig < r.ngn { T::L152 } else { T::L115 };
                }
                T::L220 => break,
            }
        }
        // 220: prepare final matrix
        let _ = nlm;
        if ifound == 0 {
            return Err(err(format!("mat={matd:4} temp {} not found", i(idtemp, 2))));
        }
        let mut nbm = 0usize;
        if ida != 0 {
            // convert to xsec per unit energy
            for ig in 1..=ngnd {
                for ig2 in 1..=ngnd {
                    let k = ig2 + ngnd * (ig - 1);
                    p[k] = p[k] * (grp[1 + ig] - grp[ig]) / (grp[1 + ig2] - grp[ig2]);
                }
            }
            nbm = packa(&p, ngnd, &mut b, &mut igi, &mut ngi);
            id3 = nbm as i32;
        }
        // absorption and transport cross sections
        for k in 1..=ngnd {
            if idf != 0 {
                xa[k] += xf[k];
            }
            if ida != 0 {
                s1[k] = xa[k] + s0[k] - s1[k];
            }
        }
        if valu[10] == 0.0 {
            valu[10] = s0[ngnd];
        }
        // write out data for this nuclide
        if itrc == 0 {
            valu[4] = 1.0 - valu[4];
        }
        if itrc == 1 {
            valu[4] = -1.0;
        }
        let head: String = [idm, idma].iter().map(|&n| i(n, 10)).collect::<String>()
            + &[idb, ida, idf, id1, id2, id3, id4, id5].iter().map(|&n| i(n, 5)).collect::<String>();
        out.push(head);
        out.push(format!(
            "{}{}{}{}",
            &aname[0..4],
            &aname[4..8],
            &aname[8..10],
            (1..=6).map(|k| f(valu[k], 10, 4)).collect::<String>()
        ));
        out.push((7..=13).map(|k| f(valu[k], 10, 4)).collect());
        out.push((14..=20).map(|k| f(valu[k], 10, 4)).collect());
        six_e12_5(&mut out, &xa[1..=ngnd]);
        if idf == 1 {
            six_e12_5(&mut out, &xf[1..=ngnd]);
        }
        if itrc == 1 {
            six_e12_5(&mut out, &s1[1..=ngnd]);
        }
        if ida == 1 {
            ints(&mut out, &igi[1..=ngnd], 12, 6);
            ints(&mut out, &ngi[1..=ngnd], 12, 6);
            six_e12_5(&mut out, &b[1..=nbm]);
        }
    }
    let mut text = String::new();
    for l in out {
        text.push_str(&l);
        text.push('\n');
    }
    Ok(text)
}

/// `packa(tpa,igroup,a,k,igi,ng)` (`powr.f90:1955-1990`): pack the
/// `igroup x igroup` matrix `tpa` (column-major, 1-based) by rows into `a`,
/// dropping elements below `1e-6` before a row's first and after its last.
/// Returns `k`, the words packed.
fn packa(tpa: &[f64], igroup: usize, a: &mut [f64], igi: &mut [i32], ng: &mut [i32]) -> usize {
    let small = 1.0e-6f64;
    let t = |i: usize, j: usize| tpa[i + igroup * (j - 1)];
    let mut k = 0usize;
    for i in 1..=igroup {
        igi[i] = 0;
        ng[i] = 0;
        let mut idone = 0;
        let mut j = 0usize;
        let mut jj = 0usize;
        while j < igroup && idone == 0 {
            j += 1;
            jj = igroup - j + 1;
            if t(i, jj).abs() >= small {
                idone = 1;
            }
        }
        let jhi = jj;
        for j in 1..=jhi {
            if t(i, j).abs() >= small || igi[i] != 0 {
                if t(i, j).abs() >= small && igi[i] == 0 {
                    igi[i] = j as i32;
                }
                k += 1;
                a[k] = t(i, j);
            }
        }
        ng[i] = jhi as i32 - igi[i] + 1;
    }
    k
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn packa_keeps_upstreams_band_and_empty_rows() {
        // 3x3, column-major, 1-based: tpa(i,j) = p[i + 3*(j-1)]
        let mut p = vec![0.0; 10];
        p[1 + 3 * 1] = 2.0; // tpa(1,2)
        p[1 + 3 * 2] = 3.0; // tpa(1,3)
        p[3] = 1.0e-7; // tpa(3,1): below small
        p[3 + 3 * 2] = 4.0; // tpa(3,3)
        let mut a = vec![0.0; 10];
        let (mut igi, mut ng) = ([0; 4], [0; 4]);
        let k = packa(&p, 3, &mut a, &mut igi, &mut ng);
        assert_eq!(k, 3);
        assert_eq!(&a[1..=3], &[2.0, 3.0, 4.0]);
        assert_eq!((igi[1], ng[1]), (2, 2));
        // an empty row: jhi = 1, igi = 0, ng = 2
        assert_eq!((igi[2], ng[2]), (0, 2));
        assert_eq!((igi[3], ng[3]), (3, 1));
    }
}
