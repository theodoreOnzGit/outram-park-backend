// Ported from NJOY2016.80 `src/acepn.f90` (commit ab27c6414a0f):
//   - `subroutine phnplo`, l.2830-3573: the plot file of ACER `iopt = 7` on a
//     photo-nuclear (`u`) table;
//   - its private `ascle` (l.3575-3716, identical to `acefc.f90`'s) and `ascll`
//     (l.3718-3753, identical to `aceth.f90`'s), reused from `aplots` and `tplots`.
// NJOY2016 is under a modified BSD 3-Clause (LANL/DOE) licence, GPL-compatible;
// this derivative file is distributed under AGPL-3.0-only. This is a modified,
// non-LANL version, not endorsed by LANL/DOE. See crate root LICENSE.njoy + NOTICE.

//! `phnplo`: the viewr input text (plot file) of a photo-nuclear ACE table
//! (gh:#1369).
//!
//! Pages, in upstream's order: lin-lin total/nonelastic/elastic, the partial
//! cross sections (`NTR > 1`), the heating, the particle heating contributions
//! (only when one is non-zero), the particle production cross sections
//! (photons divided by 5), then for each production type and each reaction an
//! angular-distribution page (`LANDP > 0`, split at 20 MeV when the grid spans
//! it) and a 3-D emission page (laws 4, 44 and 61).
//!
//! The page records are [`super::aplots`]'s (`fmt::Plot`); `ascle` is
//! [`super::aplots::scale::ascle`] (the three upstream copies are identical) and
//! `ascll` is [`super::tplots`]'s: `acepn.f90`'s copy, like `aceth.f90`'s, lacks
//! `acefc.f90`'s "make sure amin and amax differ" step (checked by `diff`,
//! 2026-11-13).
//!
//! Upstream quirks kept, each on purpose:
//! - names are `mtname(mt, name, 0)` with `name(2:2) = 'g'`; the partial-page
//!   curve label writes all 10 characters of `name`, the 3-D titles `len_trim`;
//! - the partial page reads its energies as `xss(iaa+j-1)`, without the `ESZ`
//!   offset (`ESZ = 1` in every table `acephn` writes, so it lands on the grid);
//! - the angular page's `do ic=1,70` does not reset `it` first;
//! - the angular page writes no title line for photons (`ipt = 2`);
//! - the 3-D page's `zmin = zmax/10000`, `go to 100` when `zmax = 0`, discrete
//!   lines (`j <= nd`) scaled by 100 and bracketed by `ep ± 0.01` at `zmin`, and
//!   `intt` compared with 1 *before* the `10*nd` is taken off it.

use super::aplots::scale::{ascle, mtname};
use super::aplots::{fmt, Plot};
use super::read::RawAceTable;
use super::tplots::ascll;

const BIG: f64 = 1.0e10;
const DELTA: f64 = 0.01;
const UP: f64 = 1.01;
const DN: f64 = 0.99;
const EPS: f64 = 1.0e-5;

/// `xss(i)`, 1-based; zero outside the table.
fn x(xss: &[f64], i: i64) -> f64 {
    if i >= 1 && (i as usize) <= xss.len() {
        xss[(i - 1) as usize]
    } else {
        0.0
    }
}

/// `nint(xss(i))`.
fn xi(xss: &[f64], i: i64) -> i64 {
    fmt::nint(x(xss, i))
}

/// The `NXS`/`JXS` words `phnplo` reads (module variables of `acepn`).
struct Pn {
    nes: i64,
    ntr: i64,
    ntype: i64,
    esz: i64,
    non: i64,
    els: i64,
    thn: i64,
    mtr: i64,
    lsig: i64,
    sig: i64,
    ixsa: i64,
}

/// `mtname(mt,name,0)` then `if (name(1:1).eq.'(') name(2:2)='g'`.
fn g_name(mt: i64) -> String {
    // `iverf` is the `endf` module's: 6 after the ENDF-6 `iopt = 5` run, -1 when
    // unset; `mtname` treats both alike.
    let mut n: Vec<char> = mtname(mt, 0, -1).chars().collect();
    if n.first() == Some(&'(') && n.len() > 1 {
        n[1] = 'g';
    }
    n.into_iter().collect()
}

/// `name(1:len_trim(name))`.
fn trimmed(name: &str) -> &str {
    name.trim_end_matches(' ')
}

/// `xtag=95*xmin/100+5*xmax/100`, with the `xmax > 30` variant when `wide`.
fn xtag_of(xmin: f64, xmax: f64, wide: bool) -> f64 {
    if wide && xmax > 30.0 {
        35.0 * xmin / 100.0 + 65.0 * xmax / 100.0
    } else {
        95.0 * xmin / 100.0 + 5.0 * xmax / 100.0
    }
}

/// The 2-D page header after `('1',i3,'/')` and the title: subtitle, tag,
/// axes and their labels (`ylabel`), then the blank legend record.
#[allow(clippy::too_many_arguments)]
fn page_2d(w: &mut Plot, hk: &str, it: usize, iwcol: i64, sub: &str, tag: Option<(f64, f64)>, ax: (f64, f64, f64, f64, f64, f64), ylabel: &str) {
    w.page(iwcol, hk, it);
    w.line(&format!("'{sub}'/"));
    match tag {
        Some((xt, yt)) => w.tag("1 0 2 1", xt, yt),
        None => w.line("1 0 2 1/"),
    }
    w.axis(ax.0, ax.1, ax.2);
    w.line("'<e>nergy (<m>e<v>)'/");
    w.axis(ax.3, ax.4, ax.5);
    w.line(&format!("'{ylabel}'/"));
    w.line("/");
}

/// Linear `ascle(4, ...)` of both axes: `(xmin, xmax, xstep, ymin, ymax, ystep)`.
fn scale_both(xmin: f64, xmax: f64, ymin: f64, ymax: f64) -> (f64, f64, f64, f64, f64, f64) {
    let (xmin, xmax, major, _) = ascle(4, xmin, xmax);
    let xstep = (xmax - xmin) / major as f64;
    let (ymin, ymax, major, _) = ascle(4, ymin, ymax);
    let ystep = (ymax - ymin) / major as f64;
    (xmin, xmax, xstep, ymin, ymax, ystep)
}

/// The particle label of the heating and production pages (`photons` given).
fn particle_label(ipt: i64, photons: &str) -> Option<String> {
    let s = match ipt {
        1 => "neutrons",
        2 => photons,
        9 => "protons",
        31 => "deuterons",
        32 => "tritons",
        33 => "he-3",
        34 => "alphas",
        _ => return None,
    };
    Some(format!("'{s}'/"))
}

/// Lin-lin total, nonelastic and elastic (`acepn.f90:2866-2955`).
fn principal(xss: &[f64], p: &Pn, w: &mut Plot, hk: &str, it: &mut usize, iwcol: i64) {
    let (mut xmin, mut xmax, mut ymin, mut ymax) = (BIG, 0.0f64, BIG, -BIG);
    for i in 1..=p.nes {
        let e = x(xss, p.esz - 1 + i);
        let stot = x(xss, p.esz + p.nes - 1 + i);
        xmin = if e < xmin { e } else { xmin };
        xmax = if e > xmax { e } else { xmax };
        ymin = if stot < ymin { stot } else { ymin };
        ymax = if stot > ymax { stot } else { ymax };
        if p.els != 0 {
            let snon = x(xss, p.non - 1 + i);
            ymin = if snon < ymin { snon } else { ymin };
            ymax = if snon > ymax { snon } else { ymax };
            let selas = x(xss, p.els - 1 + i);
            ymin = if selas < ymin { selas } else { ymin };
            ymax = if selas > ymax { selas } else { ymax };
        }
    }
    let ax = scale_both(xmin, xmax, ymin, ymax);
    *it = fmt::hk_it(hk);
    let tag = (xtag_of(ax.0, ax.1, true), ax.3 / 10.0 + 9.0 * ax.4 / 10.0);
    page_2d(w, hk, *it, iwcol, "<p>rincipal cross sections", Some(tag), ax, "<c>ross section (barns)");
    w.line("/");
    w.line("'total'/");
    w.line("0/");
    for i in 1..=p.nes {
        w.pair(x(xss, p.esz - 1 + i), x(xss, p.esz + p.nes - 1 + i));
    }
    w.line("/");
    if p.els > 0 {
        w.line("2/");
        w.line("/");
        w.line("0 0 1/");
        w.line("'nonelastic'/");
        w.line("0/");
        for i in 1..=p.nes {
            w.pair(x(xss, p.esz - 1 + i), x(xss, p.non - 1 + i));
        }
        w.line("/");
        w.line("3/");
        w.line("/");
        w.line("0 0 2/");
        w.line("'elastic'/");
        w.line("0/");
        for i in 1..=p.nes {
            w.pair(x(xss, p.esz - 1 + i), x(xss, p.els - 1 + i));
        }
        w.line("/");
    }
}

/// Lin-lin partial cross sections, `ntr > 1` (`acepn.f90:2957-3052`).
fn partials(xss: &[f64], p: &Pn, w: &mut Plot, hk: &str, it: &mut usize, iwcol: i64) {
    if p.ntr <= 1 {
        return;
    }
    let (mut xmin, mut xmax, mut ymin, mut ymax) = (BIG, 0.0f64, BIG, -BIG);
    for i in 1..=p.ntr {
        let k = fmt::nint(x(xss, p.lsig + i - 1) + p.sig as f64 - 1.0);
        let n = xi(xss, k + 1);
        let iaa = xi(xss, k);
        for j in 1..=n {
            let xx = x(xss, iaa + j - 1);
            let y = x(xss, k + 2 + j - 1);
            if y != 0.0 || j <= 1 {
                xmin = if xx < xmin { xx } else { xmin };
                xmax = if xx > xmax { xx } else { xmax };
                ymin = if y < ymin { y } else { ymin };
                ymax = if y > ymax { y } else { ymax };
            }
        }
    }
    let ax = scale_both(xmin, xmax, ymin, ymax);
    *it = fmt::hk_it(hk);
    let tag = (xtag_of(ax.0, ax.1, true), ax.3 / 10.0 + 9.0 * ax.4 / 10.0);
    page_2d(w, hk, *it, iwcol, "<p>artial cross sections", Some(tag), ax, "<c>ross section (barns)");
    for ii in 1..=p.ntr {
        let mt = xi(xss, p.mtr + ii - 1);
        let name = g_name(mt);
        let k = fmt::nint(x(xss, p.lsig + ii - 1) + p.sig as f64 - 1.0);
        let n = xi(xss, k + 1);
        let iaa = xi(xss, k);
        if ii == 1 {
            w.line(&format!("0 0 0{}/", fmt::fi(ii - 1, 2)));
        } else {
            w.line("2/");
            w.line("/");
            let (mut idash, mut icolor) = (0i64, ii - 1);
            while icolor >= 8 {
                idash += 1;
                icolor -= 8;
            }
            w.line(&format!("0 0{}{}/", fmt::fi(idash, 2), fmt::fi(icolor, 2)));
        }
        w.line(&format!("'{name}'/"));
        w.line("0/");
        for j in 1..=n {
            w.pair(x(xss, iaa + j - 1), x(xss, k + 2 + j - 1));
        }
        w.line("/");
    }
}

/// Lin-lin heating per reaction (`acepn.f90:3054-3088`).
fn heating(xss: &[f64], p: &Pn, w: &mut Plot, hk: &str, it: &mut usize, iwcol: i64) {
    let (mut xmin, mut xmax, mut ymin, mut ymax) = (BIG, 0.0f64, BIG, -BIG);
    for i in 1..=p.nes {
        let e = x(xss, p.esz - 1 + i);
        let heat = x(xss, p.thn - 1 + i);
        xmin = if e < xmin { e } else { xmin };
        xmax = if e > xmax { e } else { xmax };
        ymin = if heat < ymin { heat } else { ymin };
        ymax = if heat > ymax { heat } else { ymax };
    }
    let ax = scale_both(xmin, xmax, ymin, ymax);
    *it = fmt::hk_it(hk);
    page_2d(w, hk, *it, iwcol, "<h>eating", None, ax, "<h>eating (<m>e<v>/reaction)");
    w.line("/");
    w.line("'heating'/");
    w.line("0/");
    for i in 1..=p.nes {
        w.pair(x(xss, p.esz - 1 + i), x(xss, p.thn - 1 + i));
    }
    w.line("/");
}

/// The `(pxs, phn)` words of production type `i` (1-based) of the IXSA block.
fn ixsa_word(xss: &[f64], p: &Pn, i: i64, off: i64) -> i64 {
    xi(xss, p.ixsa + 12 * (i - 1) + off)
}

/// The particle heating (`prod = false`, `acepn.f90:3090-3197`) or production
/// (`prod = true`, `:3199-3279`) page.
fn particle_page(xss: &[f64], p: &Pn, w: &mut Plot, hk: &str, it: &mut usize, iwcol: i64, prod: bool) {
    // heating reads `xss(phn+1+ie)`, production `xss(pxs+1+ie)` (photons /5).
    let value = |i: i64, ie: i64| -> f64 {
        let ipt = ixsa_word(xss, p, i, 0);
        if prod {
            let v = x(xss, ixsa_word(xss, p, i, 2) + 1 + ie);
            if ipt == 2 {
                v / 5.0
            } else {
                v
            }
        } else {
            x(xss, ixsa_word(xss, p, i, 3) + 1 + ie)
        }
    };
    let (mut xmin, mut xmax, mut ymin, mut ymax) = (BIG, 0.0f64, BIG, -BIG);
    for i in 1..=p.ntype {
        let pxs = ixsa_word(xss, p, i, 2);
        let iaa = xi(xss, pxs);
        let naa = xi(xss, pxs + 1);
        for ie in 1..=naa {
            let e = x(xss, p.esz + iaa + ie - 2);
            let v = value(i, ie);
            xmin = if e < xmin { e } else { xmin };
            xmax = if e > xmax { e } else { xmax };
            ymin = if v < ymin { v } else { ymin };
            ymax = if v > ymax { v } else { ymax };
        }
    }
    // `if (ymax.ne.zero)` guards the heating page only.
    if !prod && ymax == 0.0 {
        return;
    }
    let ax = scale_both(xmin, xmax, ymin, ymax);
    *it = fmt::hk_it(hk);
    let tag = (xtag_of(ax.0, ax.1, prod), ax.3 / 10.0 + 9.0 * ax.4 / 10.0);
    let (sub, ylabel) = if prod {
        ("<p>article production cross sections", "<c>ross section (barns)")
    } else {
        ("<p>article heating contributions", "<m>e<v>/collision")
    };
    page_2d(w, hk, *it, iwcol, sub, Some(tag), ax, ylabel);
    for ii in 1..=p.ntype {
        let ipt = ixsa_word(xss, p, ii, 0);
        let pxs = ixsa_word(xss, p, ii, 2);
        let iaa = xi(xss, pxs);
        let naa = xi(xss, pxs + 1);
        if ii > 1 {
            w.line("2/");
            w.line("/");
        }
        w.line(&format!("0 0 0{}/", fmt::fi(ii - 1, 2)));
        if let Some(l) = particle_label(ipt, if prod { "photons/5" } else { "photons" }) {
            w.line(&l);
        }
        w.line("0/");
        for i in 1..=naa {
            w.pair(x(xss, p.esz + iaa - 2 + i), value(ii, i));
        }
        w.line("/");
    }
}

/// The angular-distribution page(s) of one reaction (`acepn.f90:3316-3446`).
/// `na` is `LANDP(imt) > 0`; `it` is upstream's, not reset here.
#[allow(clippy::too_many_arguments)]
fn angular_pages(xss: &[f64], w: &mut Plot, hk: &str, it: &mut usize, iwcol: i64, ipt: i64, name: &str, na: i64, andp: i64) {
    let na = na + andp - 1;
    let ne = xi(xss, na);
    let nb = na + ne;
    let (xmin, xmax, xstep) = (-1.0f64, 1.0f64, 1.0f64 / 2.0);
    let mut ymin = x(xss, na + 1);
    let mut ymax = x(xss, na + ne);
    let mut itwo = 0i64;
    let mut brk = 0.0f64;
    if ne > 4 {
        if ymax > 3.0 * x(xss, na + ne - 1) {
            ymax = x(xss, na + ne - 1);
        }
        if ymax >= 50.0 {
            brk = 20.0;
            if ymin < DN * brk && ymax > UP * brk {
                itwo = 1;
            }
        }
    }
    while (0..=2).contains(&itwo) {
        if itwo == 1 {
            ymax = brk;
        } else if itwo == 2 {
            ymin = brk;
            ymax = x(xss, na + ne);
        }
        let (y1, y2, major, _) = ascle(4, ymin, ymax);
        ymin = y1;
        ymax = y2;
        let ystep = (ymax - ymin) / major as f64;
        let (mut zmin, mut zmax) = (BIG, 0.0f64);
        for ie in 1..=ne {
            let e = x(xss, na + ie);
            if e <= (1.0 + EPS) * ymax && e >= ymin {
                let k = fmt::nint(x(xss, nb + ie).abs()) + andp - 1;
                let np = xi(xss, k + 1);
                let k = k + 1;
                for j in 1..=np {
                    let pp = x(xss, k + np + j);
                    zmin = if pp < zmin { pp } else { zmin };
                    zmax = if pp > zmax { pp } else { zmax };
                }
            }
        }
        let rat = 100_000.0f64;
        if zmin <= 0.0 {
            zmin = zmax / rat;
        }
        if zmax / zmin > rat {
            zmin = zmax / rat;
        }
        let (z1, z2) = ascll(zmin, zmax);
        zmin = z1;
        zmax = z2;
        if zmin == zmax {
            zmax *= 2.0;
            zmin = zmax / 10.0;
        }
        for (ic, c) in hk.chars().take(70).enumerate() {
            if c != ' ' {
                *it = ic + 1;
            }
        }
        w.page(iwcol, hk, *it);
        let who = match ipt {
            1 => Some("neutron"),
            9 => Some("proton"),
            31 => Some("deuteron"),
            32 => Some("triton"),
            33 => Some("3he"),
            34 => Some("alpha"),
            _ => None,
        };
        if let Some(who) = who {
            w.line(&format!("'angular distribution for {} {who}'/", trimmed(name)));
        }
        w.line("-1 2/");
        w.axis(xmin, xmax, xstep);
        w.line("'<c>osine'/");
        w.axis(ymin, ymax, ystep);
        w.line("'<e>nergy (<m>e<v>)'/");
        w.axis(zmin, zmax, 1.0);
        w.line("'<p>rob/<C>os'/");
        w.line("/");
        w.line(" 15. -15. 15. -2.5 6.5 2.5/");
        w.line("1/");
        let stepm = (ymax - ymin) / 150.0;
        let mut elast = 0.0f64;
        for ie in 1..=ne {
            let e = x(xss, na + ie);
            let skip = e > (1.0 + EPS) * ymax || e < ymin || (ie > 1 && ie < ne && ne > 150 && e - elast < stepm);
            if !skip {
                w.single(e);
                let k = fmt::nint(x(xss, nb + ie).abs()) + andp - 1;
                let intt = xi(xss, k);
                let np = xi(xss, k + 1);
                let k = k + 1;
                let mut ylast = zmin;
                for j in 1..=np {
                    let cc = x(xss, k + j);
                    let mut pp = x(xss, k + np + j);
                    if pp < zmin {
                        pp = zmin;
                    }
                    if intt == 1 {
                        w.pair(cc, ylast);
                    }
                    w.pair(cc, pp);
                    ylast = pp;
                }
                w.line("/");
                elast = e;
            }
        }
        w.line("/");
        itwo = match itwo {
            0 | 2 => 3,
            _ => 2,
        };
    }
}

/// The 3-D emission page of one reaction, laws 4, 44 and 61
/// (`acepn.f90:3448-3561`). `l2` is `dlwp + LDLWP(imt) - 1`.
#[allow(clippy::too_many_arguments)]
fn emission_page(xss: &[f64], w: &mut Plot, hk: &str, it: &mut usize, iwcol: i64, ipt: i64, name: &str, l2: i64, dlwp: i64) {
    let l3 = dlwp + xi(xss, l2 + 2) - 1;
    let ne = xi(xss, l3 + 1);
    let (mut zmin, mut zmax) = (1000.0f64, 0.0f64);
    let mut ymin = x(xss, l3 + 2);
    let mut ymax = x(xss, l3 + 2 + ne - 1);
    if ymax > 3.0 * x(xss, l3 + 2 + ne - 2) {
        ymax = x(xss, l3 + 2 + ne - 2);
    }
    // `loci` of the incident energy `ie`, and `nn` there.
    let loc = |ie: i64| -> i64 { xi(xss, l3 + 2 + ne + ie - 1) + dlwp - 1 };
    for ie in 2..=ne {
        let e = x(xss, l3 + 2 + ie - 1);
        if e <= UP * ymax {
            let loci = loc(ie);
            let nn = xi(xss, loci + 1);
            if nn > 2 {
                let loci = loci + 1;
                // "skip first two point that may be pseudo-threshold"
                for j in 3..=nn {
                    let pd = x(xss, loci + nn + j);
                    zmin = if pd < zmin { pd } else { zmin };
                    zmax = if pd > zmax { pd } else { zmax };
                }
            }
        }
    }
    if zmax == 0.0 {
        // `go to 100`
        return;
    }
    zmin = zmax / 10_000.0;
    let (z1, z2) = ascll(zmin, zmax);
    zmin = z1;
    zmax = z2;
    let (mut xmin, mut xmax) = (1000.0f64, 0.0f64);
    for ie in 1..=ne {
        let e = x(xss, l3 + 2 + ie - 1);
        if e <= UP * ymax {
            let loci = loc(ie);
            let nn = xi(xss, loci + 1);
            let loci = loci + 1;
            for j in 1..=nn {
                let ep = x(xss, loci + j);
                let pd = x(xss, loci + nn + j);
                if pd >= zmin && pd <= zmax {
                    xmin = if ep < xmin { ep } else { xmin };
                    xmax = if ep > xmax { ep } else { xmax };
                }
            }
        }
    }
    let (x1, x2, major, _) = ascle(2, xmin, xmax);
    xmin = x1;
    xmax = x2;
    let xstep = (xmax - xmin) / major as f64;
    let (y1, y2, major, _) = ascle(4, ymin, ymax);
    ymin = y1;
    ymax = y2;
    let ystep = (ymax - ymin) / major as f64;
    *it = fmt::hk_it(hk);
    w.page(iwcol, hk, *it);
    let who = match ipt {
        1 => Some("neutrons"),
        2 => Some("photons"),
        9 => Some("protons"),
        31 => Some("deuterons"),
        32 => Some("tritons"),
        33 => Some("he3s"),
        34 => Some("alphas"),
        _ => None,
    };
    if let Some(who) = who {
        w.line(&format!("'{who} from {}'/", trimmed(name)));
    }
    w.line("-1 2/");
    w.axis(xmin, xmax, xstep);
    w.line("'<s>ec. <e>nergy'/");
    w.axis(ymin, ymax, ystep);
    w.line("'<e>nergy (<m>e<v>)'/");
    w.axis(zmin, zmax, 1.0);
    w.line("'<p>rob/<m>e<v>'/");
    w.line("/");
    w.line("/");
    w.line("1/");
    for ie in 1..=ne {
        let e = x(xss, l3 + 2 + ie - 1);
        if e <= UP * ymax {
            let loci = loc(ie);
            let intt = xi(xss, loci);
            let nd = xi(xss, loci) / 10;
            let nn = xi(xss, loci + 1);
            if nn > 2 {
                w.single(e);
                let loci = loci + 1;
                let mut ylast = zmin;
                let mut k = 0i64;
                for j in 1..=nn {
                    let ep = x(xss, loci + j);
                    if ep >= xmin && ep <= xmax {
                        let mut pd = x(xss, loci + nn + j);
                        if j <= nd {
                            pd *= 100.0;
                        }
                        if pd < zmin {
                            pd = zmin;
                        }
                        if pd > zmax {
                            pd = zmax;
                        }
                        if j <= nd {
                            w.pair(ep - DELTA, zmin);
                        }
                        if intt == 1 && j > nd {
                            w.pair(ep, ylast);
                        }
                        w.pair(ep, pd);
                        if j <= nd {
                            w.pair(ep + DELTA, zmin);
                        }
                        k += 1;
                        ylast = pd;
                    }
                }
                if k < 2 {
                    w.pair(xmin, zmin);
                    w.pair(xmax, zmin);
                }
                w.line("/");
            }
        }
    }
    w.line("/");
}

/// `phnplo(nout, hk)`: the viewr input text for the photo-nuclear table `raw`,
/// whose comment is `hk` (page titles). Colour pages (`ipcol = 2`,
/// `iwcol = 3`), as upstream always writes them.
pub fn phnplo(raw: &RawAceTable, hk: &str) -> String {
    let hk = fmt::pad70(hk);
    let n = |k: usize| i64::from(raw.nxs[k]);
    let j = |k: usize| i64::from(raw.jxs[k]);
    // `lxs,za,nes,ntr,ntype,...` and `esz,tot,non,els,thn,mtr,lqr,lsig,sig,ixsa,...`
    // (`acepn.f90:1883-1884`).
    let p = Pn { nes: n(2), ntr: n(3), ntype: n(4), esz: j(0), non: j(2), els: j(3), thn: j(4), mtr: j(5), lsig: j(7), sig: j(8), ixsa: j(9) };
    let xss = raw.xss.as_slice();
    let (ipcol, iwcol) = (2i64, 3i64);
    let mut it = 1usize;
    let mut w = Plot::default();
    w.line(&format!("1 2 .30{}/", fmt::fi(ipcol, 3)));
    principal(xss, &p, &mut w, &hk, &mut it, iwcol);
    partials(xss, &p, &mut w, &hk, &mut it, iwcol);
    heating(xss, &p, &mut w, &hk, &mut it, iwcol);
    particle_page(xss, &p, &mut w, &hk, &mut it, iwcol, false);
    particle_page(xss, &p, &mut w, &hk, &mut it, iwcol, true);
    // 3-D plots of particle emission, per production type and reaction.
    for i in 1..=p.ntype {
        let ipt = ixsa_word(xss, &p, i, 0);
        let ntrp = ixsa_word(xss, &p, i, 1);
        let mtrp = ixsa_word(xss, &p, i, 4);
        let landp = ixsa_word(xss, &p, i, 8);
        let andp = ixsa_word(xss, &p, i, 9);
        let ldlwp = ixsa_word(xss, &p, i, 10);
        let dlwp = ixsa_word(xss, &p, i, 11);
        for imt in 1..=ntrp {
            let mt = xi(xss, mtrp + imt - 1);
            let na = xi(xss, landp + imt - 1);
            let l2 = dlwp + xi(xss, ldlwp + imt - 1) - 1;
            let law = xi(xss, l2 + 1);
            let name = g_name(mt);
            if na > 0 {
                angular_pages(xss, &mut w, &hk, &mut it, iwcol, ipt, &name, na, andp);
            }
            if law == 4 || law == 44 || law == 61 {
                emission_page(xss, &mut w, &hk, &mut it, iwcol, ipt, &name, l2, dlwp);
            }
        }
    }
    w.line("99/");
    w.line("stop");
    w.text
}

#[cfg(test)]
mod tests {
    use super::*;

    /// `mtname(mt,name,0)` with the `'g'` rewrite: a `(` name gets the photon
    /// letter and keeps its 10 columns; a plain name is unchanged.
    #[test]
    fn photon_names_keep_ten_columns() {
        assert_eq!(g_name(103), "(g,p)     ");
        assert_eq!(g_name(5), "(g,x)     ");
        assert_eq!(g_name(18), "fission   ");
        assert_eq!(trimmed(&g_name(103)), "(g,p)");
    }

    /// The label position: `xmax > 30` moves `xtag` only where upstream tests it.
    #[test]
    fn xtag_moves_only_on_the_wide_pages() {
        assert_eq!(xtag_of(0.0, 100.0, true), 65.0);
        assert_eq!(xtag_of(0.0, 100.0, false), 5.0);
        assert_eq!(xtag_of(0.0, 20.0, true), 1.0);
    }
}
