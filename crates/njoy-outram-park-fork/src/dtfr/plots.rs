// Translated from NJOY2016.80 (commit ac5adf5) `src/dtfr.f90`: `ploted` (`:948-1121`),
// `histod` (`:1123-1209`), `dpend` (`:1211-1292`), `plotnn` (`:1294-1415`) and
// `plotnp` (`:1417-1507`).
// NJOY2016 is under a modified BSD 3-Clause (LANL/DOE) licence, GPL-compatible;
// this derivative file is distributed under AGPL-3.0-only. This is a modified,
// non-LANL version, not endorsed by LANL/DOE. See crate root LICENSE.njoy + NOTICE.

//! **DTFR's plots** (gh:#1369, deck 03): the VIEWR plot commands for the edit
//! histograms with the PENDF overlaid, and the isometric scattering tables.
//!
//! ~~Plotting is permanently out of scope for this port~~ (`README.md`):
//! **CORRECTED 2026-11-13 (gh:#1369):** translated here, because deck 03's
//! unit 36 and VIEWR's PostScript of it (unit 37) are compared.
//!
//! Upstream kept: `plotnn` destroys the table it plots and sets the module's
//! `xmin`/`xmax` (`plotnp` has local ones); `ig2l` keeps its last value when
//! no secondary group of a row is in range; a real power `ten**x` is `pow`
//! and an integer one `__powidf2` (PETIR's `r_powf` and `r_powi`).

use petir::mathf::RealMath;

use crate::dtfr::deck::{e1p, Dtfr};
use crate::endf::gety1::Gety1;
use crate::endf::records::SectionCursor;
use crate::NjoyError;

const TEN: f64 = 10.0;

/// `Ew.d` with no scale factor (`0.dd...E+ee`), as gfortran writes it.
fn e0(x: f64, d: usize, w: usize) -> String {
    let body = if x == 0.0 {
        format!("0.{}E+00", "0".repeat(d))
    } else {
        let s = format!("{:.*e}", d.saturating_sub(1), x.abs());
        let (m, e) = s.split_once('e').unwrap_or((s.as_str(), "0"));
        let digits: String = m.chars().filter(|c| c.is_ascii_digit()).collect();
        let e: i32 = e.parse::<i32>().unwrap_or(0) + 1;
        let sign = if x < 0.0 { "-" } else { "" };
        if e.abs() >= 100 {
            format!("{sign}0.{digits}{}{}", if e < 0 { '-' } else { '+' }, e.abs())
        } else {
            format!("{sign}0.{digits}E{}{:02}", if e < 0 { '-' } else { '+' }, e.abs())
        }
    };
    if body.len() > w {
        return "*".repeat(w);
    }
    format!("{body:>w$}")
}

fn f(x: f64, w: usize, d: usize) -> String {
    crate::viewr::fmt::f(x, w, d)
}

fn line(d: &mut Dtfr, s: &str) {
    d.plot.push_str(s);
    d.plot.push('\n');
}

/// The window line of a plot (`ifilm = 2`, four per frame).
fn window(d: &mut Dtfr, tail: &str) {
    const WW: f64 = 5.0;
    const HH: f64 = 3.75;
    let xll = WW * f64::from(d.ixo);
    let yll = HH - HH * f64::from(d.iyo);
    let lead = if d.ixo == 0 && d.iyo == 0 { " 1 0 1. 1." } else { " -1 0 1. 1." };
    let s = format!("{lead}{}{}{}{}{}/{tail}", f(xll, 6, 2), f(yll, 6, 2), f(WW, 6, 2), f(HH, 6, 2), f(0.0, 6, 2));
    line(d, &s);
}

fn advance(d: &mut Dtfr) {
    d.ixo += 1;
    if d.ixo > 1 {
        d.ixo = 0;
    }
    if d.ixo == 0 {
        d.iyo += 1;
    }
    if d.iyo > 1 {
        d.iyo = 0;
    }
}

/// `ploted` (`:948-1121`).
///
/// # Errors
/// A PENDF section `findf` would not find.
pub(crate) fn ploted(d: &mut Dtfr, sig: &[f64]) -> Result<(), NjoyError> {
    let nchar = [' ', 'x', '+', '*', '0'];
    let nplt = d.iptotl;
    for i in 1..=nplt {
        let nk = (d.ned + 3) as usize;
        let Some(ksave) = (1..=nk).find(|&k| d.jped[k] == i && d.ids[k] > 0) else { continue };
        let jpos = i;
        let mut mt = d.mted[ksave];
        if mt == 300 {
            continue;
        }
        if mt == 1 && i != d.iptotl {
            continue;
        }
        let hedn = d.hednam[jpos as usize].clone();
        let labelz = format!("{:<6.6}    {:<6.6}", d.hisnam, hedn);
        let ngn = d.ngn as usize;
        let mut y = vec![0.0f64; ngn + 1];
        for (k, yk) in y.iter_mut().enumerate().skip(1) {
            let locs = jpos + d.itabl * (d.ngn - k as i32);
            *yk = sig[locs as usize];
        }
        let (x, z) = histod(d, &y, ngn);
        if d.ifilm == 1 {
            line(d, " 1/");
        } else {
            window(d, "");
        }
        let xtag;
        if (500..=522).contains(&mt) {
            xtag = if d.ifilm == 1 { TEN.r_powf(d.xmax) / 10.0 } else { 3.0 * TEN.r_powf(d.xmax) / 100.0 };
        } else {
            let t = d.xmin + (d.xmax - d.xmin) / 50.0;
            xtag = TEN.r_powf(t);
        }
        let ytag = TEN.r_powf(d.ymax - (d.ymax - d.ymin) / 50.0);
        line(d, &format!(" '<{labelz:>20}'/"));
        line(d, " /");
        line(d, &format!(" 4 0 3 1{}{}/", e0(xtag, 2, 10), e0(ytag, 2, 10)));
        line(d, &format!("{}{}{}/", e1p(TEN.r_powf(d.xmin), 2, 10), e1p(TEN.r_powf(d.xmax), 2, 10), e1p(1.0, 2, 10)));
        line(d, " '<e>nergy (e<v>)'/");
        line(d, &format!("{}{}{}/", e1p(TEN.r_powf(d.ymin), 2, 10), e1p(TEN.r_powf(d.ymax), 2, 10), e1p(1.0, 2, 10)));
        line(d, " '<c>ross <s>ection (barns)'/");
        line(d, "/");
        line(d, " 0 0 0 0 1/");
        line(d, " '<m.g.>'/");
        line(d, " 0");
        for ih in 1..x.len() {
            line(d, &format!("{}{}/", e1p(x[ih], 4, 13), e1p(z[ih], 4, 13)));
        }
        line(d, " /");
        if d.ifilm != 1 {
            advance(d);
        }
        let skip = if d.iphph == 1 {
            if d.iverf >= 6 { !(501..=522).contains(&mt) } else { !(501..=602).contains(&mt) }
        } else if d.iverf >= 6 {
            (mt > 150 && mt < 301) || (mt > 450 && mt < 600) || mt > 849
        } else {
            (mt > 150 && mt < 301) || (mt > 450 && mt < 700) || mt > 799
        };
        if skip {
            continue;
        }
        // Draw the smooth plot(s) (`:1069-1118`).
        let mut j = 0usize;
        let mut k = ksave;
        loop {
            j += 1;
            let l1 = format!("mt{mt:3}");
            let _nsym0 = nchar.get(j - 1).copied().unwrap_or(' ');
            let (px, py) = dpend(d, mt)?;
            let npts = px.len() - 1;
            let (lin, nsym, ndash) = if j < 5 {
                (0usize, 0usize, j)
            } else if j < 9 {
                (1 + npts / 5, 4, j - 4)
            } else {
                (1 + npts / 5, 3, j - 8)
            };
            line(d, &format!("{:3}/ pendf plot for mt={:4}", j + 1, mt));
            line(d, "/");
            line(d, &format!("{lin:5}{nsym:3}{ndash:3} 0 1/"));
            line(d, &format!(" '<{l1:<8.8}'/"));
            line(d, " 0");
            for ipt in 1..=npts {
                line(d, &format!("{}{}/", e1p(px[ipt], 4, 13), e1p(py[ipt], 4, 13)));
            }
            line(d, " /");
            // label 150.
            let mut next = None;
            k += 1;
            while k <= d.ned as usize {
                if d.ids[k] != 0 && d.jped[k] == i {
                    next = Some(k);
                    break;
                }
                k += 1;
            }
            match next {
                Some(kk) => mt = d.mted[kk],
                None => break,
            }
        }
    }
    Ok(())
}

/// `histod` (`:1123-1209`): the histogram `(x, z)` (1-based) and the scales.
fn histod(d: &mut Dtfr, y: &[f64], ng: usize) -> (Vec<f64>, Vec<f64>) {
    const SMALL: f64 = 1.0e-10;
    const BIG: f64 = 1.0e10;
    const FACT: f64 = 1.7;
    let eg = &d.egn;
    let mut x = vec![0.0f64; 2 * ng + 4];
    let mut z = vec![0.0f64; 2 * ng + 4];
    let mut yh = SMALL;
    let mut yl = BIG;
    let mut n = 1usize;
    z[n] = 0.0;
    for i in 1..=ng {
        if y[i] != 0.0 || n != 1 {
            n += 2;
            z[n - 1] = y[i];
            z[n] = y[i];
            x[n - 2] = eg[i];
            x[n - 1] = eg[i];
            if y[i] > yh {
                yh = y[i];
            }
            if y[i] > 0.0 && y[i] < yl {
                yl = y[i];
            }
        }
    }
    x[n] = eg[ng + 1];
    n += 1;
    x[n] = eg[ng + 1];
    z[n] = 0.0;
    yh *= FACT;
    let ayh = yh.r_log10();
    let mut iph = ayh as i32;
    if iph >= 0 {
        iph += 1;
    }
    let ayl = yl.r_log10();
    let mut ipl = (ayl - 1.0) as i32;
    if iph - ipl > 6 {
        ipl = iph - 6;
    }
    if iph == ipl {
        iph += 1;
    }
    d.ymin = f64::from(ipl);
    d.ymax = f64::from(iph);
    if iph - ipl <= 3 {
        let ynow = d.ymin;
        if yl > 2.0 * TEN.r_powf(ynow) {
            d.ymin = ynow + 2.0f64.r_log10();
        }
        if yl > 5.0 * TEN.r_powf(ynow) {
            d.ymin = ynow + 5.0f64.r_log10();
        }
        let ynow = d.ymax;
        if yh < 0.5 * TEN.r_powf(ynow) {
            d.ymax = ynow + 0.5f64.r_log10();
        }
        if yh < 0.2 * TEN.r_powf(ynow) {
            d.ymax = ynow + 0.2f64.r_log10();
        }
    }
    d.bot = TEN.r_powf(d.ymin);
    d.top = TEN.r_powf(d.ymax);
    let axh = x[n].r_log10();
    let mut iph = axh as i32;
    if axh - f64::from(iph) != 0.0 {
        iph += 1;
    }
    d.xmax = f64::from(iph);
    let ipl = (x[1].r_log10() + SMALL) as i32;
    let mut ipx = -4;
    if ipl > ipx {
        ipx = -2;
    }
    if ipl > ipx {
        ipx = 4;
    }
    if ipl > ipx {
        ipx = 5;
    }
    if ipl > ipx {
        ipx = 6;
    }
    d.xmin = f64::from(ipx);
    if iph - ipx <= 2 {
        let xnow = d.xmax;
        if x[n] < 0.5 * TEN.r_powf(xnow) {
            d.xmax = xnow + 0.5f64.r_log10();
        }
        if x[n] < 0.2 * TEN.r_powf(xnow) {
            d.xmax = xnow + 0.2f64.r_log10();
        }
        let xnow = d.xmin;
        if x[1] > 2.0 * TEN.r_powf(xnow) {
            d.xmin = xnow + 2.0f64.r_log10();
        }
        if x[1] > 5.0 * TEN.r_powf(xnow) {
            d.xmin = xnow + 5.0f64.r_log10();
        }
    }
    d.axl = d.xmin;
    d.axd = d.xmax - d.xmin;
    for zi in z.iter_mut().take(n + 1).skip(1) {
        if *zi <= d.bot {
            *zi = d.bot;
        }
    }
    x.truncate(n + 1);
    z.truncate(n + 1);
    (x, z)
}

/// `dpend` (`:1211-1292`): the PENDF reaction `mt`, thinned for plotting.
/// Returns 1-based `(x, y)`.
fn dpend(d: &Dtfr, mt: i32) -> Result<(Vec<f64>, Vec<f64>), NjoyError> {
    const EHIGH: f64 = 5.0e7;
    const NDIM: usize = 7000;
    let mft = if d.iphph == 1 { 23 } else { 3 };
    let p = d.pendf.as_ref().ok_or_else(|| NjoyError::InvalidInput("dpend: no pendf unit".into()))?;
    let sec = p.section(d.matd, mft, mt).ok_or(NjoyError::SectionNotFound { mat: d.matd, mf: mft, mt })?;
    let mut cur = SectionCursor::new(&sec.rows);
    cur.read_cont()?;
    let tab = cur.read_tab1()?;
    let mut g = Gety1::new(&tab);
    let mut enext = g.get(0.0).xnext;
    let fact = 2000.0 / d.axd;
    let step = TEN.r_powf(1.0 / 50.0);
    let elow = TEN.r_powf(d.axl);
    let mut x = vec![0.0f64; 1];
    let mut y = vec![0.0f64; 1];
    let mut npts = 0usize;
    let mut ixlast = -100i32;
    let mut ns = 0i32;
    let mut elast = enext;
    loop {
        // label 210.
        let mut e = enext;
        if e > step * elast {
            e = step * elast;
        }
        elast = e;
        let v = g.get(e);
        let mut s = v.y;
        enext = v.xnext;
        if enext < elow {
            continue;
        }
        if s < 0.0 {
            s = -s;
        }
        if s > d.top {
            s = d.top;
        }
        if s < d.bot {
            s = d.bot;
        }
        if e < elow {
            e = elow;
        }
        let ix = ((e.r_log10() - d.axl) * fact) as i32;
        // 230 = add a point, 240, 250, 260 = store, 270.
        let mut lbl;
        if ix != ixlast {
            ixlast = ix;
            ns = 0;
            lbl = 230;
        } else if s == y[npts] {
            lbl = 260;
        } else {
            lbl = match ns {
                1 => 230,
                2 | 3 => 240,
                4 => 250,
                5 => 270,
                _ => 240,
            };
        }
        if lbl == 240 {
            let ym1 = if npts >= 1 { y[npts - 1] } else { 0.0 };
            lbl = if (s - y[npts]) * (y[npts] - ym1) > 0.0 { 260 } else { 230 };
        }
        if lbl == 250 {
            ns = 5;
            let s2 = y[npts - 2];
            let s1 = y[npts - 1];
            if s2 <= s1 {
                y[npts - 2] = s1;
                y[npts - 1] = s2;
            }
            lbl = 270;
        }
        if lbl == 270 {
            if s > y[npts - 2] {
                y[npts - 2] = s;
            } else if s < y[npts - 1] {
                y[npts - 1] = s;
            }
            lbl = 260;
        }
        if lbl == 230 {
            npts += 1;
            if npts > NDIM {
                return Err(NjoyError::InvalidInput("dpend: npts exceeds ndim.".into()));
            }
            x.push(0.0);
            y.push(0.0);
            ns += 1;
        }
        // label 260.
        x[npts] = e;
        y[npts] = s;
        if enext > EHIGH {
            break;
        }
    }
    Ok((x, y))
}

/// `plotnn` (`:1294-1415`); destroys `sig`.
pub(crate) fn plotnn(d: &mut Dtfr, sig: &mut [f64], il: i32) {
    const EMIN: f64 = 1.0e3;
    const EMAX: f64 = 1.0e8;
    const BIG: f64 = 1.0e10;
    let ngn = d.ng;
    let ltabn = d.itabl - d.iptotl;
    let mut k = 0usize;
    let mut top = -BIG;
    for ig in 1..=ngn {
        let mut j = (d.itabl * (ig - 1) + d.iptotl) as usize;
        let ig2 = (ngn - ig + 1) as usize;
        let dl = d.egn[ig2 + 1].r_ln() - d.egn[ig2].r_ln();
        for _ in 1..=ltabn {
            k += 1;
            j += 1;
            let mut sigj = sig[j];
            if d.egn[ig2] < EMIN {
                sigj = 0.0;
            }
            if d.egn[ig2 + 1] > EMAX {
                sigj = 0.0;
            }
            sig[k] = sigj / dl;
            if sig[k] > top {
                top = sig[k];
            }
        }
    }
    let mut i = top.r_log10() as i32;
    if i >= 0 {
        i += 1;
    }
    let bot = TEN.r_powi(i - 5);
    let top = TEN.r_powi(i);
    let lim = (ngn * ltabn) as usize;
    for v in sig.iter_mut().take(lim + 1).skip(1) {
        if *v < bot {
            *v = bot;
        }
    }
    let l = il - 1;
    let ititle = if d.iphph == 1 {
        format!("{:<6.6} l={l} phot-phot table", d.hisnam)
    } else {
        format!("{:<6.6} l={l} neut-neut table", d.hisnam)
    };
    let ngnu = ngn as usize;
    let mut xmin = d.egn[1];
    if xmin < EMIN {
        xmin = EMIN;
    }
    let mut i = xmin.r_log10() as i32;
    if i < 0 {
        i -= 1;
    }
    d.xmin = TEN.r_powi(i);
    let mut xmax = d.egn[ngnu + 1];
    if xmax > EMAX {
        xmax = EMAX;
    }
    let mut i = xmax.r_log10() as i32;
    if i >= 0 {
        i += 1;
    }
    d.xmax = TEN.r_powi(i);
    let (xmin, xmax) = (d.xmin, d.xmax);
    if d.ifilm == 1 {
        line(d, " 1/ 3d data");
    } else {
        window(d, " 3d data");
    }
    line(d, &format!(" '<{ititle:<26.26}'/"));
    line(d, " /");
    line(d, " -4 2/");
    line(d, &format!("{}{} 1./", e1p(xmin, 4, 12), e1p(xmax, 4, 12)));
    line(d, " '<s>ec. <e>nergy'/");
    line(d, &format!("{}{} 1./", e1p(xmin, 4, 12), e1p(xmax, 4, 12)));
    line(d, " '<e>nergy (e<v>)'/");
    line(d, &format!("{}{} 1./", e1p(bot, 4, 12), e1p(top, 4, 12)));
    line(d, " '<x>sec/leth'/");
    line(d, "/");
    if d.ifilm == 1 {
        line(d, " 15. -15. 15. -2.5 6.5 2.5/");
    } else {
        line(d, " 12. -12. 12. -2. 5.25 2./");
    }
    line(d, " 1/ 3d data");
    let mut ig2l = 0usize;
    for ig1 in 1..=ngnu {
        if d.egn[ig1] >= xmin && d.egn[ig1 + 1] <= xmax {
            let i = (ngn - ig1 as i32 + 1) as i32;
            line(d, &format!(" {}/", e1p(d.egn[ig1], 4, 12)));
            let mut zlast = bot;
            for ig2 in 1..=ngnu {
                if d.egn[ig2] >= xmin && d.egn[ig2 + 1] <= xmax {
                    let j = ngn - ig2 as i32 + 1;
                    let k = j - i + d.ipingp - d.iptotl;
                    if k >= 1 {
                        let mut ss = sig.get((k + ltabn * (j - 1)) as usize).copied().unwrap_or(0.0);
                        if k > ltabn {
                            ss = bot;
                        }
                        if ss < bot {
                            ss = bot;
                        }
                        let e2 = d.egn[ig2];
                        line(d, &format!(" {}{}/", e1p(e2, 4, 12), e1p(zlast, 4, 12)));
                        line(d, &format!(" {}{}/", e1p(e2, 4, 12), e1p(ss, 4, 12)));
                        zlast = ss;
                        ig2l = ig2;
                    }
                }
            }
            let e2 = d.egn[ig2l + 1];
            line(d, &format!(" {}{}/", e1p(e2, 4, 12), e1p(zlast, 4, 12)));
            line(d, &format!(" {}{}/", e1p(e2, 4, 12), e1p(bot, 4, 12)));
            line(d, " /");
        }
    }
    line(d, " /");
    if d.ifilm == 1 {
        return;
    }
    advance(d);
}

/// `plotnp` (`:1417-1507`).
pub(crate) fn plotnp(d: &mut Dtfr, sig: &[f64], ip: i32) {
    const EMIN: f64 = 1.0e3;
    const EMAX: f64 = 1.0e8;
    let ititle = format!("{:<6.6} l={} neut-phot table", d.hisnam, ip - 1);
    if d.ifilm == 1 {
        line(d, " 1/ 3d data");
    } else {
        window(d, " 3d data");
    }
    let (ngn, ngp) = (d.ngn as usize, d.ngp as usize);
    let mut top = 0.0f64;
    for ign in 1..=ngn {
        let i = ngn - ign + 1;
        for igp in 1..=ngp {
            let j = ngp - igp + 1;
            let ss = sig[j + ngp * (i - 1)] / (d.egg[igp + 1] - d.egg[igp]);
            if d.egn[ign] >= EMIN && d.egn[ign + 1] <= EMAX && d.egg[igp] >= EMIN && d.egg[igp + 1] <= EMAX && ss > top {
                top = ss;
            }
        }
    }
    let mut i = top.r_log10() as i32;
    if i >= 0 {
        i += 1;
    }
    let bot = TEN.r_powi(i - 5);
    let top = TEN.r_powi(i);
    let mut xmin = d.egn[1];
    if xmin < EMIN {
        xmin = EMIN;
    }
    let mut i = xmin.r_log10() as i32;
    if i < 0 {
        i -= 1;
    }
    let xmin = TEN.r_powi(i);
    let mut xmax = d.egn[ngn + 1];
    if xmax > EMAX {
        xmax = EMAX;
    }
    let mut i = xmax.r_log10() as i32;
    if i >= 0 {
        i += 1;
    }
    let xmax = TEN.r_powi(i);
    line(d, &format!(" '<{ititle:<26.26}'/"));
    line(d, " /");
    line(d, " -4 2/");
    line(d, &format!("{}{} 1./", e1p(xmin, 4, 12), e1p(xmax, 4, 12)));
    line(d, " '<g>amma <e>nergy'/");
    line(d, &format!("{}{} 1./", e1p(xmin, 4, 12), e1p(xmax, 4, 12)));
    line(d, " '<e>nergy (e<v>)'/");
    line(d, &format!("{}{} 1./", e1p(bot, 4, 12), e1p(top, 4, 12)));
    line(d, " '<x>sec/e<v>'/");
    line(d, "/");
    if d.ifilm == 1 {
        line(d, " 15. -15. 15. -2.5 6.5 2.5/");
    } else {
        line(d, " 12. -12. 12. -2. 5.25 2./");
    }
    line(d, " 1/ 3d data");
    for ign in 1..=ngn {
        if d.egn[ign] >= xmin && d.egn[ign + 1] <= xmax {
            let i = ngn - ign + 1;
            line(d, &format!(" {}/", e1p(d.egn[ign], 4, 12)));
            let mut zlast = bot;
            let mut ss = 0.0;
            let mut igpl = 0usize;
            for igp in 1..=ngp {
                let j = ngp - igp + 1;
                ss = sig[j + ngp * (i - 1)] / (d.egg[igp + 1] - d.egg[igp]);
                if ss < bot {
                    ss = bot;
                }
                let eg = d.egg[igp];
                line(d, &format!(" {}{}/", e1p(eg, 4, 12), e1p(zlast, 4, 12)));
                line(d, &format!(" {}{}/", e1p(eg, 4, 12), e1p(ss, 4, 12)));
                zlast = ss;
                igpl = igp;
            }
            let eg = d.egg[igpl + 1];
            line(d, &format!(" {}{}/", e1p(eg, 4, 12), e1p(zlast, 4, 12)));
            line(d, &format!(" {}{}/", e1p(eg, 4, 12), e1p(ss, 4, 12)));
            line(d, " /");
        }
    }
    line(d, " /");
    if d.ifilm == 1 {
        return;
    }
    advance(d);
}

#[cfg(test)]
mod tests {
    use super::e0;

    /// `Ew.d` without a scale factor, as gfortran 13 writes `2e10.2`.
    /// Result (2026-11-13): `0.10E+05`, `-0.25E-02`, `0.00E+00`.
    #[test]
    fn e_without_scale_factor() {
        assert_eq!(e0(1.0e4, 2, 10), "  0.10E+05");
        assert_eq!(e0(-0.0025, 2, 10), " -0.25E-02");
        assert_eq!(e0(0.0, 2, 10), "  0.00E+00");
    }
}
