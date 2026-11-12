// Ported from NJOY2016 `src/acefc.f90` (git commit ac5adf5f33d893e42f2eed7fb286b0d51c7580da; the 2016.80 line numbers
// after 7547 are 3 higher): `subroutine aplotr` (`acefc.f90:16835-17097`) and `subroutine aplopf`
// (`acefc.f90:17099-17206`).
// NJOY2016 is under a modified BSD 3-Clause (LANL/DOE) licence, GPL-compatible; this derivative file is
// distributed under AGPL-3.0-only. This is a modified, non-LANL version, not endorsed by LANL/DOE.
// See crate root LICENSE.njoy + NOTICE.

//! `aplotr` (inelastic levels, then threshold reactions, five curves per
//! page) and `aplopf` (higher-order fission reactions).

use super::fmt::{hk_it, nint, AceTable, Plot};
use super::scale::{ascle, mtname, plot_name};

const SMALL: f64 = 1.0e-12;
/// `xsmin=1.e-6_kr`: a double here (`aplots` compares with a single).
const XSMIN: f64 = 1.0e-6;
const NDEN: i64 = 4000;

fn reaction(a: &AceTable, i: i64) -> (i64, i64, i64, i64) {
    let k = nint(a.x(a.lsig + i - 1) + a.sig as f64 - 1.0);
    (a.xi(a.mtr + i - 1), k, a.xi(k + 1), a.xi(k))
}

/// `aplotr`.
pub fn aplotr(a: &AceTable, w: &mut Plot, hk: &str, iwcol: i64) {
    let it = hk_it(hk);
    let mut ilev = 0i64;
    let mut ilast = false;
    // The inelastic levels, five per page.
    while !ilast {
        let (mut xmin, mut xmax, mut ymin, mut ymax) = (1000.0f64, 0.0f64, 1000.0f64, 0.0f64);
        let mut nlev = 0;
        for i in 1..=a.ntr {
            let mt = a.xi(a.mtr + i - 1);
            if mt >= 51 + ilev && mt <= 90 && a.izai == 1 && nlev < 5 {
                nlev += 1;
                let (_, k, n, iaa) = reaction(a, i);
                let mut ylast = -1.0;
                for j in 1..=n {
                    let x = a.x(iaa + j - 1);
                    let y = a.x(k + 2 + j - 1);
                    if y != ylast {
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
                    ylast = y;
                }
            }
        }
        if nlev == 0 {
            ilast = true;
        } else {
            let (x1, x2, major, _) = ascle(4, xmin, xmax);
            xmin = x1;
            xmax = x2;
            let xstep = (xmax - xmin) / major as f64;
            let (y1, y2, major, _) = ascle(4, ymin, ymax);
            ymin = y1;
            ymax = y2;
            let ystep = (ymax - ymin) / major as f64;
            let thin = (xmax - xmin) / NDEN as f64;
            w.page(iwcol, hk, it);
            w.line("'<i>nelastic levels'/");
            let xtag = 3.0 * xmin / 10.0 + 7.0 * xmax / 10.0;
            let ytag = ymin / 10.0 + 9.0 * ymax / 10.0;
            w.tag("1 0 2 1", xtag, ytag);
            w.axis(xmin, xmax, xstep);
            w.line("'<e>nergy (<m>e<v>)'/");
            w.axis(ymin, ymax, ystep);
            w.line("'<c>ross section (barns)'/");
            w.line("/");
            nlev = 0;
            let mut idone = false;
            let mut i = 0;
            while i < a.ntr && !idone {
                i += 1;
                let mt = a.xi(a.mtr + i - 1);
                if mt >= 51 + ilev {
                    if mt > 90 || nlev >= 5 {
                        idone = true;
                    } else {
                        nlev += 1;
                        let (_, k, n, iaa) = reaction(a, i);
                        if nlev > 1 {
                            w.next_curve(nlev);
                        }
                        w.style(iwcol, (nlev - 1) % 5);
                        w.line(&format!("'{}'/", plot_name(a, mt)));
                        w.line("0/");
                        let mut xlast = SMALL;
                        for j in 1..=n {
                            let x = a.x(iaa + j - 1);
                            if n <= NDEN || x >= xlast + thin || j == n {
                                let y = a.x(k + 2 + j - 1);
                                w.pair(x, y);
                                xlast = x;
                            }
                        }
                        w.line("/");
                    }
                }
            }
        }
        ilev += nlev;
    }
    // The threshold reactions.
    let skip = |i: i64, nlev: i64, mtlast: i64| -> bool {
        let (mt, _k, _n, iaa) = reaction(a, i);
        let mut f = false;
        if nlev == 5 {
            f = true;
        }
        if mt < 5 {
            f = true;
        }
        if (18..=21).contains(&mt) {
            f = true;
        }
        if mt == 38 {
            f = true;
        }
        if (50..91).contains(&mt) && a.izai == 1 {
            f = true;
        }
        if mt > 207 && mt < 600 {
            f = true;
        }
        if mt <= mtlast {
            f = true;
        }
        if a.x(iaa) < XSMIN {
            f = true;
        }
        f
    };
    let special = |i: i64, mt: i64| -> bool {
        let nx = a.xi(a.mtr + i);
        mt < 203 && (203..=207).contains(&nx)
    };
    let mut idone = false;
    let mut mtlast = 0i64;
    let mut mtl = 0i64;
    while !idone {
        let (mut xmin, mut xmax, mut ymin, mut ymax) = (1000.0f64, 0.0f64, 1000.0f64, 0.0f64);
        let mut nlev = 0;
        for i in 1..=a.ntr {
            if !skip(i, nlev, mtlast) {
                nlev += 1;
                let (mt, k, n, iaa) = reaction(a, i);
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
                if special(i, mt) {
                    nlev = 5;
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
            let (y1, y2, major, _) = ascle(4, ymin, ymax);
            ymin = y1;
            ymax = y2;
            let ystep = (ymax - ymin) / major as f64;
            let thin = (xmax - xmin) / NDEN as f64;
            w.page(iwcol, hk, it);
            w.line("'<t>hreshold reactions'/");
            w.line("1 0 2 1/");
            w.axis(xmin, xmax, xstep);
            w.line("'<e>nergy (<m>e<v>)'/");
            w.axis(ymin, ymax, ystep);
            w.line("'<c>ross section (barns)'/");
            w.line("/");
            nlev = 0;
            for i in 1..=a.ntr {
                if !skip(i, nlev, mtlast) {
                    let (mt, k, n, iaa) = reaction(a, i);
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
                        if n <= NDEN || x >= xlast + thin || j == 1 || j == n {
                            let y = a.x(k + 2 + j - 1);
                            w.pair(x, y);
                            xlast = x;
                        }
                    }
                    w.line("/");
                    if special(i, mt) {
                        nlev = 5;
                    }
                }
            }
            mtlast = mtl;
        }
    }
}

/// `aplopf`: the higher-order fission reactions (MT = 20, 21, 38).
pub fn aplopf(a: &AceTable, w: &mut Plot, hk: &str, iwcol: i64) {
    let it = hk_it(hk);
    let wanted = |mt: i64| -> bool {
        let mut f = false;
        if mt < 20 {
            f = true;
        }
        if mt > 21 && mt < 38 {
            f = true;
        }
        if mt > 38 {
            f = true;
        }
        !f
    };
    let (mut xmin, mut xmax, mut ymin, mut ymax) = (1000.0f64, 0.0f64, 1000.0f64, 0.0f64);
    let mut nlev = 0;
    for i in 1..=a.ntr {
        let (mt, k, n, iaa) = reaction(a, i);
        if wanted(mt) {
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
    if nlev != 0 && ymax != 0.0 {
        let (x1, x2, major, _) = ascle(4, xmin, xmax);
        xmin = x1;
        xmax = x2;
        let xstep = (xmax - xmin) / major as f64;
        let (y1, y2, major, _) = ascle(4, ymin, ymax);
        ymin = y1;
        ymax = y2;
        let ystep = (ymax - ymin) / major as f64;
        let thin = (xmax - xmin) / NDEN as f64;
        w.page(iwcol, hk, it);
        w.line("'<h>igh-order fission reactions'/");
        w.line("1 0 2 1/");
        w.axis(xmin, xmax, xstep);
        // Upstream's label reads `(<m>e>v>)`.
        w.line("'<e>nergy (<m>e>v>)'/");
        w.axis(ymin, ymax, ystep);
        w.line("'<c>ross section (barns)'/");
        w.line("/");
        nlev = 0;
        for i in 1..=a.ntr {
            let (mt, k, n, iaa) = reaction(a, i);
            if wanted(mt) {
                nlev += 1;
                if nlev > 1 {
                    w.next_curve(nlev);
                }
                w.style(iwcol, (nlev - 1) % 5);
                w.line(&format!("'{}'/", mtname(mt, a.izai, a.iverf)));
                w.line("0/");
                let mut xlast = SMALL;
                for j in 1..=n {
                    let x = a.x(iaa + j - 1);
                    if n <= NDEN || x >= xlast + thin || j == 1 || j == n {
                        let y = a.x(k + 2 + j - 1);
                        w.pair(x, y);
                        xlast = x;
                    }
                }
                w.line("/");
            }
        }
    }
}
