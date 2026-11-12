// Ported from NJOY2016 `src/acefc.f90` (git commit ac5adf5f33d893e42f2eed7fb286b0d51c7580da; the 2016.80 line numbers
// after 7547 are 3 higher): `subroutine aplots`, the lin-lin and lin-log pages (`acefc.f90:16390-16750`).
// NJOY2016 is under a modified BSD 3-Clause (LANL/DOE) licence, GPL-compatible; this derivative file is
// distributed under AGPL-3.0-only. This is a modified, non-LANL version, not endorsed by LANL/DOE.
// See crate root LICENSE.njoy + NOTICE.

//! The linear pages of `aplots`: principal cross sections above 2 MeV,
//! heating, damage, and the lin-log non-threshold reaction pages.

use super::fmt::{hk_it, nint, AceTable, Plot};
use super::log_pages::SINGLE_1E6;
use super::scale::{ascle, ascll, plot_name};

const BIG: f64 = 1.0e10;
const SMALL: f64 = 1.0e-12;
const HMIN: f64 = 1.0e-10;
const NDEN: i64 = 4000;

/// `test=1; test=test/5` and `test/10`.
const FIFTH: f64 = 1.0 / 5.0;
const TENTH: f64 = 1.0 / 10.0;

/// One lin-lin curve over the energy grid: points with `e >= test`, thinned to
/// `thin`, always keeping the last point when `keep_last`.
fn lin_curve(w: &mut Plot, a: &AceTable, test: f64, thin: f64, keep_last: bool, val: impl Fn(i64) -> f64) {
    let nes = a.nes;
    let mut xlast = SMALL;
    for i in 1..=nes {
        let e = a.x(a.esz - 1 + i);
        if e >= test && (nes <= NDEN || e >= xlast + thin || (keep_last && i == nes)) {
            w.pair(e, val(i));
            xlast = e;
        }
    }
    w.line("/");
}

/// The lin-lin principal cross-section page (energies above 2 MeV).
pub fn principal_lin(a: &AceTable, w: &mut Plot, hk: &str, iwcol: i64) {
    let it = hk_it(hk);
    let nes = a.nes;
    let (mut xmin, mut xmax, mut ymin, mut ymax) = (BIG, 0.0f64, BIG, -BIG);
    for i in 1..=nes {
        let e = a.x(a.esz - 1 + i);
        if e > 2.0 {
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
    }
    ymin = 0.0;
    let (x1, x2, major, _) = ascle(4, xmin, xmax);
    xmin = x1;
    xmax = x2;
    let xstep = (xmax - xmin) / major as f64;
    let (y1, y2, major, _) = ascle(4, ymin, ymax);
    ymin = y1;
    ymax = y2;
    let ystep = (ymax - ymin) / major as f64;
    w.page(iwcol, hk, it);
    w.line("'<p>rincipal cross sections'/");
    let xtag = 35.0 * xmin / 100.0 + 65.0 * xmax / 100.0;
    let ytag = ymin / 10.0 + 9.0 * ymax / 10.0;
    w.tag("1 0 2 1", xtag, ytag);
    w.axis(xmin, xmax, xstep);
    w.line("'<e>nergy (<m>e<v>)'/");
    w.axis(ymin, ymax, ystep);
    w.line("'<c>ross section (barns)'/");
    w.line("/");
    w.line("/");
    w.line("'total'/");
    w.line("0/");
    let thin = (xmax - xmin) / NDEN as f64;
    lin_curve(w, a, FIFTH, thin, true, |i| a.x(a.esz + nes - 1 + i));
    w.line("2/");
    w.line("/");
    w.line(if iwcol == 0 { "0 0 1/" } else { "0 0 0 3/" });
    w.line("'absorption'/");
    w.line("0/");
    lin_curve(w, a, TENTH, thin, true, |i| a.x(a.esz + 2 * nes - 1 + i));
    w.line("3/");
    w.line("/");
    w.line(if iwcol == 0 { "0 0 2/" } else { "0 0 0 2/" });
    w.line("'elastic'/");
    w.line("0/");
    lin_curve(w, a, TENTH, thin, true, |i| a.x(a.esz + 3 * nes - 1 + i));
    if a.gpd != 0 {
        w.line("4/");
        w.line("/");
        w.line(if iwcol == 0 { "0 0 3/" } else { "0 0 0 1/" });
        w.line("'gamma production'/");
        w.line("0/");
        lin_curve(w, a, TENTH, thin, false, |i| a.x(a.gpd - 1 + i));
    }
}

/// The lin-lin heating page and the lin-lin damage page.
pub fn heating_damage_lin(a: &AceTable, w: &mut Plot, hk: &str, iwcol: i64) {
    let it = hk_it(hk);
    let nes = a.nes;
    let (mut xmin, mut xmax, mut ymin, mut ymax) = (BIG, 0.0f64, BIG, -BIG);
    for i in 1..=nes {
        let e = a.x(a.esz - 1 + i);
        let heat = a.x(a.esz + 4 * nes - 1 + i);
        if e < xmin {
            xmin = e;
        }
        if e > xmax {
            xmax = e;
        }
        if heat < ymin {
            ymin = heat;
        }
        if heat > ymax && e > 1.0 {
            ymax = heat;
        }
    }
    if ymin != 0.0 || ymax != 0.0 {
        let (x1, x2, major, _) = ascle(4, xmin, xmax);
        xmin = x1;
        xmax = x2;
        let xstep = (xmax - xmin) / major as f64;
        let (y1, y2, major, _) = ascle(4, ymin, ymax);
        ymin = y1;
        ymax = y2;
        let ystep = (ymax - ymin) / major as f64;
        w.page(iwcol, hk, it);
        w.line("'<h>eating'/");
        w.line("1 0 2 1/");
        w.axis(xmin, xmax, xstep);
        w.line("'<e>nergy (<m>e<v>)'/");
        w.axis(ymin, ymax, ystep);
        w.line("'<h>eating (<m>e<v>/reaction)'/");
        w.line("/");
        w.line("/");
        w.line("'heating'/");
        w.line("0/");
        let thin = (xmax - xmin) / NDEN as f64;
        lin_curve(w, a, FIFTH, thin, true, |i| a.x(a.esz + 4 * nes - 1 + i));
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
            if e >= FIFTH {
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
                if dam > ymax && e > 1.0 {
                    ymax = dam;
                }
            }
        }
        if ymin != 0.0 || ymax != 0.0 {
            let (x1, x2, major, _) = ascle(4, xmin, xmax);
            xmin = x1;
            xmax = x2;
            let xstep = (xmax - xmin) / major as f64;
            let (y1, y2, major, _) = ascle(4, ymin, ymax);
            ymin = y1;
            ymax = y2;
            let ystep = (ymax - ymin) / major as f64;
            w.page(iwcol, hk, it);
            w.line("'<d>amage'/");
            w.line("1 0 2 1/");
            w.axis(xmin, xmax, xstep);
            w.line("'<e>nergy (<m>e<v>)'/");
            w.axis(ymin, ymax, ystep);
            w.line("'<d>amage (<m>e<v>-barns)'/");
            w.line("/");
            w.line("/");
            w.line("'damage'/");
            w.line("0/");
            let thin = (xmax - xmin) / NDEN as f64;
            let mut xlast = SMALL;
            for i in 1..=n {
                let e = a.x(iaa + i - 1);
                if e >= FIFTH && (nes <= NDEN || e >= xlast + thin || i == nes) {
                    let mut dam = a.x(k + 2 + i - 1);
                    if dam < HMIN {
                        dam = HMIN;
                    }
                    w.pair(e, dam);
                    xlast = e;
                }
            }
            w.line("/");
        }
    }
}

/// `iflag` of the lin-log non-threshold selection.
fn skip_reaction(a: &AceTable, i: i64, nlev: i64, mtlast: i64) -> (bool, i64, i64, i64, i64) {
    let mt = a.xi(a.mtr + i - 1);
    let k = nint(a.x(a.lsig + i - 1) + a.sig as f64 - 1.0);
    let n = a.xi(k + 1);
    let iaa = a.xi(k);
    let skip = nlev == 5 || mt <= mtlast || mt > 207 || a.x(iaa) > SINGLE_1E6;
    (skip, mt, k, n, iaa)
}

/// The lin-log non-threshold reaction pages, five curves per page.
pub fn nonthreshold_linlog(a: &AceTable, w: &mut Plot, hk: &str, iwcol: i64) {
    let it = hk_it(hk);
    let mut mtlast = 0i64;
    let mut mtl = 0i64;
    let mut idone = false;
    while !idone {
        let (mut xmin, mut xmax, mut ymin, mut ymax) = (1000.0f64, 0.0f64, 1000.0f64, 0.0f64);
        let mut nlev = 0;
        for i in 1..=a.ntr {
            let (skip, _mt, k, n, iaa) = skip_reaction(a, i, nlev, mtlast);
            if !skip {
                nlev += 1;
                for j in 1..=n {
                    let x = a.x(iaa + j - 1);
                    let y = a.x(k + 2 + j - 1);
                    let mut flag = false;
                    if x < FIFTH {
                        flag = true;
                    }
                    if y == 0.0 && j > 1 {
                        flag = true;
                    }
                    if !flag {
                        if x < xmin {
                            xmin = x;
                        }
                        if x > xmax {
                            xmax = x;
                        }
                        if y < ymin {
                            ymin = y;
                        }
                        if y > ymax && x > 1.0 {
                            ymax = y;
                        }
                    }
                }
            }
        }
        if nlev == 0 || ymax == 0.0 {
            idone = true;
        } else {
            let (x1, x2, major, _) = ascle(4, xmin, xmax);
            xmin = x1;
            xmax = x2;
            let xstep = (xmax - xmin) / major as f64;
            (ymin, ymax) = ascll(ymin, ymax);
            if ymax == ymin {
                idone = true;
            } else {
                let thin = (xmax - xmin) / NDEN as f64;
                w.page(iwcol, hk, it);
                w.line("'<n>on-threshold reactions'/");
                w.line("2 0 2 1/");
                w.axis(xmin, xmax, xstep);
                w.line("'<e>nergy (<m>e<v>)'/");
                w.axis(ymin, ymax, 1.0);
                w.line("'<c>ross section (barns)'/");
                w.line("/");
                nlev = 0;
                for i in 1..=a.ntr {
                    let (skip, mt, k, n, iaa) = skip_reaction(a, i, nlev, mtlast);
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
                            if x >= FIFTH && (n <= NDEN || x >= xlast + thin || j == n) {
                                let mut y = a.x(k + 2 + j - 1);
                                if y < ymin {
                                    y = ymin;
                                }
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
}
