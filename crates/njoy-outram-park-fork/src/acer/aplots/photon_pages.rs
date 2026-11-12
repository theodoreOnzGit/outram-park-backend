// Ported from NJOY2016 `src/acefc.f90` (git commit ac5adf5f33d893e42f2eed7fb286b0d51c7580da; the 2016.80 line numbers
// after 7547 are 3 higher): `subroutine aplopp` (`acefc.f90:18193-18788`).
// NJOY2016 is under a modified BSD 3-Clause (LANL/DOE) licence, GPL-compatible; this derivative file is
// distributed under AGPL-3.0-only. This is a modified, non-LANL version, not endorsed by LANL/DOE.
// See crate root LICENSE.njoy + NOTICE.

//! Detailed photon pages (`aplopp`): one 3-D page per photon reaction with a
//! law-4 distribution, then the thermal-capture and 14 MeV photon spectra
//! (neutron tables only).
//!
//! Kept as upstream has them: `nd` is `nint(xss(loci))/10` (an *integer*
//! division, unlike `aplodd`'s `nint(xss(loci)/10)`); the page loop scales
//! discrete lines when `j < nd` while the limits loop uses `j <= nd`; the
//! 14 MeV block skips no `2*m` interpolation words; the second spectrum
//! reuses the first's `iimax` when no bin exceeds `ymin`.

use super::fmt::{hk_it, nint, AceTable, Plot};
use super::scale::{ascle, ascll, mtname};
use crate::mixr::mix::sigfig;

const SMALL: f64 = 1.0e-12;
const UP: f64 = 1.001;
const DELT: f64 = 0.01;
const ETH: f64 = 2.53e-8;
const E14: f64 = 14.0;

/// `loct` at the energy list of a law-4 block starting at `loc1` (the
/// `loc1+3` ... `ne` walk of `aplopp`), returning `(loct, ne)`.
fn law4_energies(a: &AceTable, loc: i64) -> (i64, i64) {
    let mut loct = loc + 3;
    let m = a.xi(loct);
    if m != 0 {
        loct += 2 * m;
    }
    loct += 1;
    let n = a.xi(loct);
    loct = loct + 1 + 2 * n;
    let m = a.xi(loct);
    if m != 0 {
        loct += 2 * m;
    }
    loct += 1;
    (loct, a.xi(loct))
}

/// `(intt, nd, n, loci)` of the distribution at energy `ie` of the law-4
/// block at `loct` with `ne` energies; `loci` is advanced past the header.
fn dist_at(a: &AceTable, loct: i64, ne: i64, ie: i64) -> (i64, i64, i64, i64) {
    let loci = nint(a.x(ie + ne + loct) + a.dlwp as f64 - 1.0);
    let intt = a.xi(loci) % 10;
    let nd = a.xi(loci) / 10;
    let n = a.xi(loci + 1);
    (intt, nd, n, loci + 1)
}

fn page_for_reaction(a: &AceTable, w: &mut Plot, hk: &str, it: usize, iwcol: i64, i: i64, egmth: &mut f64, egm14: &mut f64) {
    let mti = nint(a.x(i - 1 + a.mtrp) / 1000.0);
    let name = mtname(mti, a.izai, a.iverf);
    let loc = nint(a.x(i - 1 + a.ldlwp) + a.dlwp as f64 - 1.0);
    let law = a.xi(loc + 1);
    if law != 4 {
        return;
    }
    let (loct, ne) = law4_energies(a, loc);
    let mut ymin = a.x(loct + 1);
    let mut ymax = a.x(loct + ne);
    let test = 3.0;
    if ne > 3 && ymax > test * a.x(loct + ne - 1) {
        ymax = a.x(loct + ne - 1);
    }
    let (mut zmin, mut zmax) = (1000.0f64, 0.0f64);
    let ie1 = if ne > 2 && mti != 102 && mti != 18 { 2 } else { 1 };
    for ie in ie1..=ne {
        let e = a.x(ie + loct);
        if e <= UP * ymax {
            let (intt, nd, n, loci) = dist_at(a, loct, ne, ie);
            if nd > 0 {
                for j in 1..=nd {
                    let p = a.x(j + n + loci) * 100.0;
                    if p < zmin {
                        zmin = p;
                    }
                    if p > zmax {
                        zmax = p;
                    }
                }
            }
            if n > nd && n > 2 {
                for j in (nd + 1)..=n {
                    let p = a.x(j + n + loci);
                    if intt != 1 || j != n {
                        if p < zmin {
                            zmin = p;
                        }
                        if p > zmax {
                            zmax = p;
                        }
                    }
                }
            }
        }
    }
    if zmax <= 0.0 {
        return;
    }
    zmin = zmax / 10000.0;
    (zmin, zmax) = ascll(zmin, zmax);
    let (mut xmin, mut xmax) = (1000.0f64, 0.0f64);
    for ie in ie1..=ne {
        let e = a.x(ie + loct);
        if e <= UP * ymax {
            let (_intt, nd, n, loci) = dist_at(a, loct, ne, ie);
            if nd > 0 {
                for j in 1..=nd {
                    let mut ep = a.x(j + loci);
                    if ep < 0.0 {
                        ep = -ep + e * a.aw0 / (a.aw0 + a.awi);
                    }
                    if ep - DELT < xmin {
                        xmin = ep - DELT;
                    }
                    if ep + DELT > xmax {
                        xmax = ep + DELT;
                    }
                }
            }
            if n > nd {
                let mut epl = a.x(1 + nd + loci);
                let mut ppl = a.x(1 + nd + n + loci);
                for j in (nd + 1)..=n {
                    let ep = a.x(j + loci);
                    let p = a.x(j + n + loci);
                    if p >= zmin && epl < xmin {
                        xmin = epl;
                    }
                    if ppl >= zmin && ep > xmax {
                        xmax = ep;
                    }
                    epl = ep;
                    ppl = p;
                }
            }
            if e <= 100.0 * ETH && xmax > *egmth {
                *egmth = xmax;
            }
            if e <= E14 + E14 / 10.0 && xmax > *egm14 {
                *egm14 = xmax;
            }
        }
    }
    if xmax - xmin < xmax / 10.0 {
        xmin -= xmin / 10.0;
        xmax += xmax / 10.0;
    }
    let (x1, x2, major, _) = ascle(1, xmin, xmax);
    xmin = x1;
    xmax = x2;
    let xstep = (xmax - xmin) / major as f64;
    let (y1, y2, major, _) = ascle(4, ymin, ymax);
    ymin = y1;
    ymax = y2;
    let ystep = (ymax - ymin) / major as f64;
    let thin = (xmax - xmin) / 500.0;
    w.page(iwcol, hk, it);
    w.line(&format!("'<p>hoton emission for {name}'/"));
    w.line("-1 2/");
    w.axis(xmin, xmax, xstep);
    w.line("'<e>#L.3H.8]g#HXLX> (<m>e<v>)'/");
    w.axis(ymin, ymax, ystep);
    w.line("'<e>#L.3H.8>n#HXLX> (<m>e<v>)'/");
    w.axis(zmin, zmax, 1.0);
    w.line("'<p>rob/<m>e<v>'/");
    w.line("/");
    w.line("/");
    w.line("1/");
    for ie in ie1..=ne {
        let e = a.x(ie + loct);
        if e <= UP * ymax {
            let (intt, nd, n, loci) = dist_at(a, loct, ne, ie);
            w.single(e);
            let mut xlast = SMALL;
            let mut ylast = zmin;
            let mut k = 0;
            for j in 1..=n {
                let mut ep = a.x(j + loci);
                if j <= nd && ep < 0.0 {
                    ep = -ep + e * a.aw0 / (a.aw0 + a.awi);
                }
                let mut flag = false;
                if ep < xmin || ep > xmax {
                    flag = true;
                }
                if n > 500 && ep < xlast + thin && j != 1 && j != n {
                    flag = true;
                }
                if !flag {
                    let mut p = a.x(j + n + loci);
                    if j < nd {
                        p *= 100.0;
                    }
                    if p < zmin {
                        p = zmin;
                    }
                    if p > zmax {
                        p = zmax;
                    }
                    if j <= nd {
                        w.pair(ep - DELT, zmin);
                    }
                    if intt == 1 && j > nd {
                        w.pair(ep, ylast);
                    }
                    w.pair(ep, p);
                    if j <= nd {
                        w.pair(ep + DELT, zmin);
                    }
                    k += 1;
                    xlast = ep;
                    ylast = p;
                }
            }
            if k < 2 {
                w.pair(xmin, zmin);
                w.pair(xmax, zmin);
            }
            w.line("/");
        }
    }
    w.line("/");
}

/// The shared `iflag`/`g` of the spectrum accumulation: adds the
/// contribution of the distribution at `loci` (with `nd` discrete lines and
/// `n` points) to every one of the 2000 bins.
fn accumulate_bins(a: &AceTable, spect: &mut [f64], ymax: &mut f64, delta: f64, intt: i64, nd: i64, n: i64, loci: i64, ys: f64, f: f64) {
    for ii in 1..=2000usize {
        let eii = delta * ii as f64;
        for j in (nd + 1)..=n {
            let xj = a.x(j + loci);
            let xj1 = a.x(j + loci + 1);
            let xjm = a.x(j - 1 + loci);
            let mut iflag = false;
            if intt == 1 && j < n && xj <= eii && xj1 > eii {
                iflag = true;
            }
            if intt != 1 && j < n && xj <= eii && xj1 > eii {
                iflag = true;
            }
            if intt != 1 && j > 1 && xjm <= eii && xj > eii {
                iflag = true;
            }
            if iflag {
                let g = if intt == 1 {
                    1.0
                } else if xj <= eii {
                    (xj1 - eii) / (xj1 - xj)
                } else {
                    (eii - xjm) / (xj - xjm)
                };
                let p = a.x(j + n + loci);
                spect[ii] += ys * p * f * g;
                if spect[ii] > *ymax {
                    *ymax = spect[ii];
                }
            }
        }
    }
}

fn add_line(spect: &mut [f64], ymax: &mut f64, ep: f64, delta: f64, v: f64) {
    let iii = nint(ep / delta).max(1);
    if iii <= 2000 {
        spect[iii as usize] += v;
        if spect[iii as usize] > *ymax {
            *ymax = spect[iii as usize];
        }
    }
}

/// `f` of the bracketing incident-energy pair `(ie-1, ie)` or `(ie, ie+1)`
/// around `ee`, or `None` when neither brackets it (`iflag = 0`).
fn bracket(a: &AceTable, loc1: i64, ne: i64, ie: i64, ee: f64) -> Option<f64> {
    let mut flag = false;
    if ie < ne && a.x(ie + loc1) <= ee && a.x(ie + 1 + loc1) > ee {
        flag = true;
    }
    if ie > 1 && a.x(ie - 1 + loc1) <= ee && a.x(ie + loc1) > ee {
        flag = true;
    }
    if !flag {
        return None;
    }
    Some(if a.x(ie + loc1) <= ee {
        (a.x(ie + 1 + loc1) - ee) / (a.x(ie + 1 + loc1) - a.x(ie + loc1))
    } else {
        (ee - a.x(ie - 1 + loc1)) / (a.x(ie + loc1) - a.x(ie - 1 + loc1))
    })
}

#[allow(clippy::too_many_arguments)]
fn spectrum_page(w: &mut Plot, hk: &str, it: usize, iwcol: i64, title: &str, delta: f64, spect: &mut [f64], ymax_in: f64, iimax: &mut usize, thermal: bool) {
    let mut ymax = ymax_in;
    let mut ymin;
    if thermal {
        ymin = ymax / 1000.0;
    } else {
        ymin = ymax;
        for ii in 1..=2000usize {
            if spect[ii] > 0.0 && spect[ii] < ymin {
                ymin = spect[ii];
            }
        }
        ymin /= 2.0;
    }
    (ymin, ymax) = ascll(ymin, ymax);
    for ii in 1..=2000usize {
        if spect[ii] < ymin {
            spect[ii] = ymin;
        }
        if spect[ii] > ymin {
            *iimax = ii;
        }
    }
    let xmin = delta;
    let xmax = delta * (*iimax as f64 + 1.0);
    let (xmin, xmax, major, _) = ascle(2, xmin, xmax);
    let xstep = (xmax - xmin) / major as f64;
    w.page(iwcol, hk, it);
    w.line(title);
    w.line("2/");
    w.axis(xmin, xmax, xstep);
    w.line("'<g>amma <e>nergy (<m>e<v>)'/");
    w.axis(ymin, ymax, 1.0);
    w.line("'<g>amma <p>rod (barns/<m>e<v>)'/");
    w.line("/");
    w.line("/");
    w.line("0/");
    for ie in 1..=*iimax {
        let e = delta * ie as f64;
        let mut p = spect[ie];
        if p < ymin {
            p = ymin;
        }
        w.pair(e, p);
    }
    w.line("/");
}

/// `aplopp`.
pub fn aplopp(a: &AceTable, w: &mut Plot, hk: &str, iwcol: i64) {
    let it = hk_it(hk);
    let (mut egmth, mut egm14) = (0.0f64, 0.0f64);
    let mut s = 0.0f64;
    let mut y;
    let mut iimax = 0usize;
    if a.ntrp != 0 {
        for i in 1..=a.ntrp {
            page_for_reaction(a, w, hk, it, iwcol, i, &mut egmth, &mut egm14);
        }
    }
    egmth = sigfig(egmth + egmth / 20.0, 2, 0);
    egm14 = sigfig(egm14 + egm14 / 20.0, 2, 0);
    if egmth == 0.0 {
        egmth = 10.0;
    }
    if egm14 == 0.0 {
        egm14 = 25.0;
    }
    // The spectra are for neutrons only.
    if a.izai != 1 {
        return;
    }
    let nes = a.nes;

    // The thermal capture photon spectrum.
    let delta = egmth / 2000.0;
    let mut spect = vec![0.0f64; 2001];
    if a.ntrp != 0 {
        let mut idone = false;
        let mut iie = 0i64;
        while iie < nes && !idone {
            iie += 1;
            if a.x(a.esz + iie - 1) > ETH {
                idone = true;
            }
        }
        let mut ymax = 0.0f64;
        for i in 1..=a.ntrp {
            let mti = nint(a.x(i - 1 + a.mtrp) / 1000.0);
            if mti != 102 {
                continue;
            }
            let mut loct = nint(a.x(i - 1 + a.lsigp) + a.sigp as f64 - 1.0);
            let mftype = a.xi(loct);
            loct += 1;
            if mftype != 13 {
                let mtmult = a.xi(loct);
                loct += 1;
                let m = a.xi(loct);
                let mut intt = 2;
                if m > 0 {
                    intt = a.xi(loct + 2);
                }
                loct = loct + 1 + 2 * m;
                let n = a.xi(loct);
                let mut idone = false;
                let mut ii = 0;
                while ii < n - 1 && !idone {
                    ii += 1;
                    if a.x(loct + ii + 1) > ETH {
                        idone = true;
                    }
                }
                if intt == 1 {
                    y = a.x(loct + ii + n);
                } else {
                    y = (ETH - a.x(loct + ii)) * a.x(loct + ii + 1 + n) / (a.x(loct + ii + 1) - a.x(loct + ii))
                        + (a.x(loct + ii + 1) - ETH) * a.x(loct + ii + n) / (a.x(loct + ii + 1) - a.x(loct + ii));
                }
                for ir in 1..=a.ntr {
                    let mt = a.xi(a.mtr + ir - 1);
                    if mt == mtmult {
                        let k = nint(a.x(a.lsig + ir - 1) + a.sig as f64 - 1.0);
                        let iaa = a.xi(k);
                        s = a.x(k + 2 + iie - iaa);
                    }
                }
            } else {
                let je = a.xi(loct);
                s = 0.0;
                if iie >= je {
                    s = a.x(loct + 2 + iie - je);
                }
                y = 1.0;
            }
            let mut loc1 = nint(a.x(i - 1 + a.ldlwp) + a.dlwp as f64 - 1.0);
            let law = a.xi(loc1 + 1);
            if law == 2 {
                let eg = a.x(loc1 + 10);
                add_line(&mut spect, &mut ymax, eg, delta, y * s / delta);
            }
            if law == 4 {
                let (l1, ne) = law4_energies(a, loc1);
                loc1 = l1;
                for ie in 1..=ne {
                    if let Some(f) = bracket(a, loc1, ne, ie, ETH) {
                        let (intt, nd, n, loci) = dist_at(a, loc1, ne, ie);
                        if nd != 0 {
                            for j in 1..=nd {
                                let ep = a.x(j + loci);
                                let p = a.x(j + n + loci);
                                add_line(&mut spect, &mut ymax, ep, delta, f * p / delta);
                            }
                        }
                        accumulate_bins(a, &mut spect, &mut ymax, delta, intt, nd, n, loci, y * s, f);
                    }
                }
            }
        }
        if ymax > 0.0 {
            spectrum_page(w, hk, it, iwcol, "'thermal capture photon spectrum'/", delta, &mut spect, ymax, &mut iimax, true);
        }
    }

    // The net photon spectrum for 14 MeV neutrons.
    let delta = egm14 / 2000.0;
    if a.ntrp != 0 {
        let mut idone = false;
        let mut i = 0i64;
        let mut iie = 0i64;
        while i < nes && !idone {
            i += 1;
            iie = nes - i;
            if a.x(a.esz + iie - 1) <= E14 {
                idone = true;
            }
        }
        let mut spect = vec![0.0f64; 2001];
        let mut ymax = 0.0f64;
        for i in 1..=a.ntrp {
            let mut loct = nint(a.x(i - 1 + a.lsigp) + a.sigp as f64 - 1.0);
            let mftype = a.xi(loct);
            loct += 1;
            if mftype != 13 {
                let mtmult = a.xi(loct);
                loct += 1;
                loct += 1;
                let n = a.xi(loct);
                let mut idone = false;
                let mut ii = 0;
                while !idone && ii < n {
                    ii += 1;
                    if a.x(loct + ii + 1) > E14 {
                        idone = true;
                    }
                }
                y = (E14 - a.x(loct + ii)) * a.x(loct + ii + 1 + n) / (a.x(loct + ii + 1) - a.x(loct + ii))
                    + (a.x(loct + ii + 1) - E14) * a.x(loct + ii + n) / (a.x(loct + ii + 1) - a.x(loct + ii));
                for ir in 1..=a.ntr {
                    let mt = a.xi(a.mtr + ir - 1);
                    if mt == mtmult {
                        let k = nint(a.x(a.lsig + ir - 1) + a.sig as f64 - 1.0);
                        let iaa = a.xi(k);
                        s = a.x(k + 2 + iie - iaa);
                    }
                }
            } else {
                let je = a.xi(loct);
                s = 0.0;
                if iie >= je {
                    s = a.x(loct + 2 + iie - je);
                }
                y = 1.0;
            }
            let mut loc1 = nint(a.x(i - 1 + a.ldlwp) + a.dlwp as f64 - 1.0);
            let law = a.xi(loc1 + 1);
            if law == 2 {
                let lp = a.xi(loc1 + 9);
                let mut eg = a.x(loc1 + 10);
                if lp == 2 {
                    eg += a.aw0 * E14 / (a.aw0 + 1.0);
                }
                add_line(&mut spect, &mut ymax, eg, delta, y * s / delta);
            }
            if law == 4 {
                let (l1, ne) = law4_energies(a, loc1);
                loc1 = l1;
                for ie in 1..=ne {
                    if let Some(f) = bracket(a, loc1, ne, ie, E14) {
                        let (intt, nd, n, loci) = dist_at(a, loc1, ne, ie);
                        if nd != 0 {
                            for j in 1..=nd {
                                let mut ep = a.x(j + loci);
                                if ep < 0.0 {
                                    ep = -ep + E14 * a.aw0 / (a.aw0 + a.awi);
                                }
                                let p = a.x(j + n + loci);
                                add_line(&mut spect, &mut ymax, ep, delta, f * p / delta);
                            }
                        }
                        if n > nd {
                            accumulate_bins(a, &mut spect, &mut ymax, delta, intt, nd, n, loci, y * s, f);
                        }
                    }
                }
            }
        }
        if ymax > 0.0 {
            spectrum_page(w, hk, it, iwcol, "'14 <m>e<v> photon spectrum'/", delta, &mut spect, ymax, &mut iimax, false);
        }
    }
}
