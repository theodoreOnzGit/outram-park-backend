// Ported from NJOY2016 `src/acefc.f90` (git commit ac5adf5f33d893e42f2eed7fb286b0d51c7580da; the 2016.80 line numbers
// after 7547 are 3 higher): `subroutine aplots`, the log-log principal, resonance and heating/damage pages
// (`acefc.f90:15215-15700`, `16190-16380`).
// NJOY2016 is under a modified BSD 3-Clause (LANL/DOE) licence, GPL-compatible; this derivative file is
// distributed under AGPL-3.0-only. This is a modified, non-LANL version, not endorsed by LANL/DOE.
// See crate root LICENSE.njoy + NOTICE.

//! The log-log pages of `aplots`: principal cross sections, the expanded
//! resonance pages (total, then fission and capture), log heating, log
//! damage, and the log non-threshold reaction pages.

use super::fmt::{hk_it, nint, AceTable, Plot};
use super::scale::{ascll, plot_name};
use petir::mathf::RealMath;

const BIG: f64 = 1.0e10;
const SMALL: f64 = 1.0e-12;
const SCALE: f64 = 1.0e6;
const HMIN: f64 = 1.0e-10;
const NDEN: i64 = 4000;
/// `1.e-6` of `aplots` is a default-real (single-precision) literal.
pub const SINGLE_1E6: f64 = 1.0e-6f32 as f64;

/// `thin=ten**(log10(xmax/xmin)/nden)`.
fn thin_log(xmax: f64, xmin: f64) -> f64 {
    10.0f64.r_powf((xmax / xmin).r_log10() / NDEN as f64)
}

/// One curve of a log page: every point of `val(i)` over `lo..=hi` thinned to
/// `nden` per decade, written as `e, max(v, floor)`.
fn log_curve(w: &mut Plot, a: &AceTable, lo: i64, hi: i64, thin: f64, floor: f64, val: impl Fn(i64) -> f64) {
    let n = hi - lo + 1;
    let mut xlast = SMALL;
    for i in lo..=hi {
        let e = a.x(a.esz - 1 + i);
        let mut v = val(i);
        if v < floor {
            v = floor;
        }
        if n <= NDEN || e >= thin * xlast {
            w.pair(e, v);
            xlast = e;
        }
    }
    w.line("/");
}

/// The first page and the pages that follow it: log-log total, absorption,
/// elastic and gamma production.
pub fn principal_log(a: &AceTable, w: &mut Plot, hk: &str, iwcol: i64) {
    let it = hk_it(hk);
    let nes = a.nes;
    let (mut xmin, mut xmax, mut ymin, mut ymax) = (BIG, 0.0f64, BIG, -BIG);
    for i in 1..=nes {
        let e = a.x(a.esz - 1 + i);
        let tot = a.x(a.esz + nes - 1 + i);
        let abso = a.x(a.esz + 2 * nes - 1 + i);
        let elas = a.x(a.esz + 3 * nes - 1 + i);
        if e < xmin {
            xmin = e;
        }
        if e > xmax {
            xmax = e;
        }
        for v in [tot, abso, elas] {
            if v < ymin {
                ymin = v;
            }
            if v > ymax {
                ymax = v;
            }
        }
        if a.gpd != 0 {
            let g = a.x(a.gpd - 1 + i);
            if g < ymin {
                ymin = g;
            }
            if g > ymax {
                ymax = g;
            }
        }
    }
    (xmin, xmax) = ascll(xmin, xmax);
    if ymin < ymax / SCALE {
        ymin = ymax / SCALE;
    }
    (ymin, ymax) = ascll(ymin, ymax);
    w.page(iwcol, hk, it);
    w.line("'<p>rincipal cross sections'/");
    let xtag = 5.0 * xmin;
    let mut ytag = 7.0 * ymin.r_log10() / 10.0 + 3.0 * ymax.r_log10() / 10.0;
    ytag = 10.0f64.r_powf(ytag);
    w.tag("4 0 2 1", xtag, ytag);
    w.axis(xmin, xmax, 1.0);
    w.line("'<e>nergy (<m>e<v>)'/");
    w.axis(ymin, ymax, 1.0);
    w.line("'<c>ross section (barns)'/");
    w.line("/");
    w.line("/");
    w.line("'total'/");
    w.line("0/");
    let thin = thin_log(xmax, xmin);
    log_curve(w, a, 1, nes, thin, ymin, |i| a.x(a.esz + nes - 1 + i));
    w.line("2/");
    w.line("/");
    w.line(if iwcol == 0 { "0 0 1/" } else { "0 0 0 3/" });
    w.line("'absorption'/");
    w.line("0/");
    log_curve(w, a, 1, nes, thin, ymin, |i| a.x(a.esz + 2 * nes - 1 + i));
    w.line("3/");
    w.line("/");
    w.line(if iwcol == 0 { "0 0 2/" } else { "0 0 0 2/" });
    w.line("'elastic'/");
    w.line("0/");
    log_curve(w, a, 1, nes, thin, ymin, |i| a.x(a.esz + 3 * nes - 1 + i));
    if a.gpd != 0 {
        w.line("4/");
        w.line("/");
        w.line(if iwcol == 0 { "0 0 3/" } else { "0 0 0 1/" });
        w.line("'gamma production'/");
        w.line("0/");
        log_curve(w, a, 1, nes, thin, ymin, |i| a.x(a.gpd - 1 + i));
    }
}

/// The window walk shared by both expanded-resonance passes. Returns the
/// `(idone, ii1, ii2, e1, e2)` state after the inner loop.
fn window(a: &AceTable, maxii: i64, ii1_in: i64, e1_in: f64) -> (i64, i64, i64, f64, f64) {
    let nes = a.nes;
    let mut idone = 0;
    let mut ii1 = ii1_in;
    let mut e1 = e1_in;
    let mut ii2;
    let mut e2;
    loop {
        ii2 = ii1;
        e2 = 10.0 * e1;
        let iil = nint(e2.r_log10());
        e2 = petir::real::powi(10.0, iil as i32);
        while ii2 < nes && a.x(a.esz + ii2 - 1) < e2 + SMALL * e2 {
            ii2 += 1;
        }
        if ii2 - ii1 > maxii {
            idone = 1;
        }
        if ii2 >= nes {
            idone = 2;
        }
        ii2 -= 1;
        if idone == 0 {
            e1 = e2;
            ii1 = ii2;
        } else {
            break;
        }
    }
    (idone, ii1, ii2, e1, e2)
}

/// The expanded resonance pages for the total cross section.
pub fn resonance_total(a: &AceTable, w: &mut Plot, hk: &str, iwcol: i64) {
    let nes = a.nes;
    if nes < 1500 {
        return;
    }
    let it = hk_it(hk);
    let mut idone = 0;
    let mut e1 = a.x(a.esz);
    let mut ii1 = 1;
    while idone == 0 {
        let (d, i1, ii2, e1n, _e2w) = window(a, 120, ii1, e1);
        idone = d;
        ii1 = i1;
        e1 = e1n;
        if idone == 1 {
            let e2 = a.x(a.esz - 1 + ii2);
            let nn = ii2 - ii1 + 1;
            let (mut xmin, mut xmax, mut ymin, mut ymax) = (BIG, SMALL, BIG, SMALL);
            for i in ii1..=ii2 {
                let e = a.x(a.esz - 1 + i);
                let tot = a.x(a.esz + nes - 1 + i);
                if e < xmin {
                    xmin = e;
                }
                if e > xmax {
                    xmax = e;
                }
                if tot > 0.0 && tot < ymin {
                    ymin = tot;
                }
                if tot > 0.0 && tot > ymax {
                    ymax = tot;
                }
            }
            (xmin, xmax) = ascll(xmin, xmax);
            if ymin < ymax / SCALE {
                ymin = ymax / SCALE;
            }
            (ymin, ymax) = ascll(ymin, ymax);
            w.page(iwcol, hk, it);
            w.line("'resonance total cross section'/");
            w.line("4 0 2 1/");
            w.axis(xmin, xmax, 1.0);
            w.line("'<e>nergy (<m>e<v>)'/");
            w.axis(ymin, ymax, 1.0);
            w.line("'<c>ross section (barns)'/");
            w.line("/");
            w.line("/");
            w.line("'total'/");
            w.line("0/");
            let thin = thin_log(xmax, xmin);
            let mut xlast = SMALL;
            for i in ii1..=ii2 {
                let e = a.x(a.esz - 1 + i);
                if nn <= NDEN || e >= thin * xlast {
                    let mut tot = a.x(a.esz + nes - 1 + i);
                    if tot < ymin {
                        tot = ymin;
                    }
                    w.pair(e, tot);
                    xlast = e;
                }
            }
            w.line("/");
            if ii2 < nes - 200 && e2 < 10.0 {
                e1 = e2;
                ii1 = ii2;
                idone = 0;
            }
        }
    }
}

/// The expanded resonance pages for fission and capture.
pub fn resonance_absorption(a: &AceTable, w: &mut Plot, hk: &str, iwcol: i64) {
    let nes = a.nes;
    if nes < 1500 {
        return;
    }
    let it = hk_it(hk);
    let (mut kf, mut iif, mut kc, mut iic) = (0i64, 0i64, 0i64, 0i64);
    let mut idone = 0;
    let mut e1 = a.x(a.esz);
    let mut ii1 = 1;
    while idone == 0 {
        let (d, i1, ii2, e1n, _e2w) = window(a, 100, ii1, e1);
        idone = d;
        ii1 = i1;
        e1 = e1n;
        if idone == 1 {
            let e2 = a.x(a.esz - 1 + ii2);
            let nn = ii2 - ii1 + 1;
            let mut nnf = 0;
            for i in 1..=a.ntr {
                let mt = a.xi(a.mtr + i - 1);
                if mt == 18 || mt == 19 {
                    kf = nint(a.x(a.lsig + i - 1) + a.sig as f64 - 1.0);
                    nnf = a.xi(kf + 1);
                    iif = a.xi(kf);
                } else if mt == 102 {
                    kc = nint(a.x(a.lsig + i - 1) + a.sig as f64 - 1.0);
                    iic = a.xi(kc);
                }
            }
            let (mut xmin, mut xmax, mut ymin, mut ymax) = (BIG, SMALL, BIG, SMALL);
            let mut nofiss = 1;
            for i in ii1..=ii2 {
                let e = a.x(a.esz - 1 + i);
                let mut fiss = 0.0;
                if nnf > 0 && i >= iif {
                    fiss = a.x(kf + 2 + i - iif);
                }
                if fiss > 0.0 {
                    nofiss = 0;
                }
                let cap = a.x(kc + 2 + i - iic);
                if e < xmin {
                    xmin = e;
                }
                if e > xmax {
                    xmax = e;
                }
                if fiss > 0.0 && fiss < ymin {
                    ymin = fiss;
                }
                if fiss > 0.0 && fiss > ymax {
                    ymax = fiss;
                }
                if cap > 0.0 && cap < ymin {
                    ymin = cap;
                }
                if cap > 0.0 && cap > ymax {
                    ymax = cap;
                }
            }
            (xmin, xmax) = ascll(xmin, xmax);
            if ymin < ymax / SCALE {
                ymin = ymax / SCALE;
            }
            (ymin, ymax) = ascll(ymin, ymax);
            w.page(iwcol, hk, it);
            w.line("'resonance absorption cross sections'/");
            w.line("4 0 2 1/");
            w.axis(xmin, xmax, 1.0);
            w.line("'<e>nergy (<m>e<v>)'/");
            w.axis(ymin, ymax, 1.0);
            w.line("'<c>ross section (barns)'/");
            w.line("/");
            w.line("/");
            w.line("'capture'/");
            w.line("0/");
            let thin = thin_log(xmax, xmin);
            let mut xlast = SMALL;
            for i in ii1..=ii2 {
                let e = a.x(a.esz - 1 + i);
                if nn <= NDEN || e >= thin * xlast {
                    let mut cap = a.x(kc + 2 + i - iic);
                    if cap < ymin {
                        cap = ymin;
                    }
                    w.pair(e, cap);
                    xlast = e;
                }
            }
            w.line("/");
            if nofiss != 1 {
                w.line("2/");
                w.line("/");
                w.line("0 0 1/");
                w.line("'fission'/");
                w.line("0/");
                let mut xlast = SMALL;
                for i in ii1..=ii2 {
                    let e = a.x(a.esz - 1 + i);
                    if nn <= NDEN || e >= thin * xlast {
                        let mut fiss = 0.0;
                        if i >= iif {
                            fiss = a.x(kf + 2 + i - iif);
                        }
                        if fiss < ymin {
                            fiss = ymin;
                        }
                        w.pair(e, fiss);
                        xlast = e;
                    }
                }
                w.line("/");
            }
            if ii2 < nes - 200 && e2 < 10.0 {
                e1 = e2;
                ii1 = ii2;
                idone = 0;
            }
        }
    }
}

/// The log heating page and the log damage page.
pub fn heating_damage_log(a: &AceTable, w: &mut Plot, hk: &str, iwcol: i64) {
    let it = hk_it(hk);
    let nes = a.nes;
    // heating per reaction
    let (mut xmin, mut xmax, mut ymin, mut ymax) = (BIG, 0.0f64, BIG, -BIG);
    for i in 1..=nes {
        let e = a.x(a.esz - 1 + i);
        let mut heat = a.x(a.esz + 4 * nes - 1 + i);
        if heat < HMIN {
            heat = HMIN;
        }
        if e < xmin {
            xmin = e;
        }
        if e > xmax {
            xmax = e;
        }
        if heat < ymin {
            ymin = heat;
        }
        if heat > ymax {
            ymax = heat;
        }
    }
    if ymax > ymin {
        (xmin, xmax) = ascll(xmin, xmax);
        if ymin < ymax / SCALE {
            ymin = ymax / SCALE;
        }
        (ymin, ymax) = ascll(ymin, ymax);
        w.page(iwcol, hk, it);
        w.line("'<h>eating'/");
        w.line("4 0 2 1/");
        w.axis(xmin, xmax, 1.0);
        w.line("'<e>nergy (<m>e<v>)'/");
        w.axis(ymin, ymax, 1.0);
        w.line("'<h>eating (<m>e<v>/reaction)'/");
        w.line("/");
        w.line("/");
        w.line("'heating'/");
        w.line("0/");
        let thin = thin_log(xmax, xmin);
        let mut xlast = SMALL;
        for i in 1..=nes {
            let e = a.x(a.esz - 1 + i);
            if nes <= NDEN || e >= thin * xlast {
                let mut heat = a.x(a.esz + 4 * nes - 1 + i);
                if heat < HMIN {
                    heat = HMIN;
                }
                if heat < ymin {
                    heat = ymin;
                }
                w.pair(e, heat);
                xlast = e;
            }
        }
        w.line("/");
    }
    // damage
    let (mut xmin, mut xmax, mut ymin, mut ymax) = (BIG, 0.0f64, BIG, -BIG);
    let (mut n, mut k, mut iaa) = (0i64, 0i64, 0i64);
    for i in 1..=a.ntr {
        let mt = a.xi(a.mtr + i - 1);
        if mt == 444 {
            k = nint(a.x(a.lsig + i - 1) + a.sig as f64 - 1.0);
            n = a.xi(k + 1);
            iaa = a.xi(k);
        }
    }
    if n != 0 {
        for i in 1..=n {
            let e = a.x(iaa + i - 1);
            let mut dam = a.x(k + 2 + i - 1);
            if dam < HMIN {
                dam = HMIN;
            }
            if e < xmin {
                xmin = e;
            }
            if e > xmax {
                xmax = e;
            }
            if dam < ymin {
                ymin = dam;
            }
            if dam > ymax {
                ymax = dam;
            }
        }
        if ymax > ymin {
            (xmin, xmax) = ascll(xmin, xmax);
            if ymin < ymax / SCALE {
                ymin = ymax / SCALE;
            }
            (ymin, ymax) = ascll(ymin, ymax);
            w.page(iwcol, hk, it);
            w.line("'<d>amage'/");
            w.line("4 0 2 1/");
            w.axis(xmin, xmax, 1.0);
            w.line("'<e>nergy (<m>e<v>)'/");
            w.axis(ymin, ymax, 1.0);
            w.line("'<d>amage (<m>e<v>-barns)'/");
            w.line("/");
            w.line("/");
            w.line("'damage'/");
            w.line("0/");
            let thin = thin_log(xmax, xmin);
            let mut xlast = SMALL;
            for i in 1..=n {
                let e = a.x(iaa + i - 1);
                if nes <= NDEN || e >= thin * xlast {
                    let mut dam = a.x(k + 2 + i - 1);
                    if dam < HMIN {
                        dam = HMIN;
                    }
                    if dam < ymin {
                        dam = ymin;
                    }
                    w.pair(e, dam);
                    xlast = e;
                }
            }
            w.line("/");
        }
    }
}

/// `iflag` of the non-threshold selection (`mt.le.mtlast`, `mt.gt.207`,
/// `xss(iaa).gt.1.e-6`, five curves per page).
fn nonthr_skip(a: &AceTable, i: i64, nlev: i64, mtlast: i64) -> (bool, i64, i64, i64, i64) {
    let mt = a.xi(a.mtr + i - 1);
    let k = nint(a.x(a.lsig + i - 1) + a.sig as f64 - 1.0);
    let n = a.xi(k + 1);
    let iaa = a.xi(k);
    let mut skip = false;
    if nlev == 5 {
        skip = true;
    }
    if mt <= mtlast {
        skip = true;
    }
    if mt > 207 {
        skip = true;
    }
    if a.x(iaa) > SINGLE_1E6 {
        skip = true;
    }
    (skip, mt, k, n, iaa)
}

/// The log-log non-threshold reaction pages, five curves per page.
pub fn nonthreshold_log(a: &AceTable, w: &mut Plot, hk: &str, iwcol: i64) {
    let it = hk_it(hk);
    let mut mtlast = 0i64;
    let mut mtl = 0i64;
    let mut nlev = 1;
    while nlev > 0 {
        let (mut xmin, mut xmax, mut ymin, mut ymax) = (1000.0f64, 0.0f64, 1000.0f64, 0.0f64);
        nlev = 0;
        for i in 1..=a.ntr {
            let (skip, _mt, k, n, iaa) = nonthr_skip(a, i, nlev, mtlast);
            if !skip {
                nlev += 1;
                for j in 1..=n {
                    let x = a.x(iaa + j - 1);
                    let y = a.x(k + 2 + j - 1);
                    if y != 0.0 || j <= 1 {
                        if x < xmin {
                            xmin = x;
                        }
                        if x > xmax {
                            xmax = x;
                        }
                        if y < ymin {
                            ymin = y;
                        }
                        if y > ymax {
                            ymax = y;
                        }
                    }
                }
            }
        }
        if nlev != 0 {
            (xmin, xmax) = ascll(xmin, xmax);
            if ymin < ymax / SCALE {
                ymin = ymax / SCALE;
            }
            (ymin, ymax) = ascll(ymin, ymax);
            let thin = thin_log(xmax, xmin);
            w.page(iwcol, hk, it);
            w.line("'<n>on-threshold reactions'/");
            w.line("4 0 2 1/");
            w.axis(xmin, xmax, 1.0);
            w.line("'<e>nergy (<m>e<v>)'/");
            w.axis(ymin, ymax, 1.0);
            w.line("'<c>ross section (barns)'/");
            w.line("/");
            nlev = 0;
            for i in 1..=a.ntr {
                let (skip, mt, k, n, iaa) = nonthr_skip(a, i, nlev, mtlast);
                if !skip {
                    mtl = mt;
                    nlev += 1;
                    if nlev > 1 {
                        w.next_curve(nlev);
                    }
                    w.style(iwcol, (nlev - 1) % 5);
                    w.line(&format!("'{}'/", plot_name(a, mt)));
                    w.line("0/");
                    let mut xlast = SMALL;
                    for j in 1..=n {
                        let x = a.x(iaa + j - 1);
                        let mut y = a.x(k + 2 + j - 1);
                        if y < ymin {
                            y = ymin;
                        }
                        if n <= NDEN || x >= thin * xlast {
                            w.pair(x, y);
                            xlast = x;
                        }
                    }
                    w.line("/");
                }
            }
            mtlast = mtl;
        }
    }
}
