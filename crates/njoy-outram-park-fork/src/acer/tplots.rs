// Ported from NJOY2016 `src/aceth.f90` (git commit ac5adf5f33d893e42f2eed7fb286b0d51c7580da):
// `subroutine tplots` (l.1073-2106) and its private copy of `ascll` (l.2251-2286).
// NJOY2016 is under a modified BSD 3-Clause (LANL/DOE) licence, GPL-compatible;
// this derivative file is distributed under AGPL-3.0-only. This is a modified,
// non-LANL version, not endorsed by LANL/DOE. See crate root LICENSE.njoy + NOTICE.

//! `tplots`: the viewr input (plot file) of a thermal ACE table (gh:#1369).
//!
//! Pages, in order: the log-log cross sections (inelastic, elastic, total), the
//! mean cosine `mubar`, the mean outgoing energy `ebar`, and, for the continuous
//! form (`IFENG = 2`), the four 3-D secondary-energy pages and the
//! angle-energy pages. The page text is written with the same record shapes as
//! [`super::aplots`]; the `xss` array reads zero outside the table, as
//! Fortran's zero-filled 50 000 000-word array does.
//!
//! **Variables that outlive their loop are function-scope here too.** `xn`,
//! `xielas` and `e` keep what the last pass left in them, because `tplots` reads
//! them after the loop that set them (the `total` curve).
//!
//! **`aceth`'s `ascll` is not `acefc`'s**: it lacks the "make sure amin and amax
//! differ" step, so [`ascll`] here is its own copy. `ascle` is the same routine
//! in both modules and is reused from [`super::aplots`].

use super::aplots::scale::ascle;
use super::aplots::{fmt, Plot};
use super::read::RawAceTable;
use crate::NjoyError;
use petir::mathf::RealMath;

/// `aceth.f90:2251`: adjust the limits of a log axis. Also `acepn.f90:3718`'s
/// copy, which is the same routine (reused by [`super::phnplo`]).
pub(crate) fn ascll(amin: f64, amax: f64) -> (f64, f64) {
    let (b1, b2, b3, b4, ohoh1, ten) = (0.0f64, 0.301f64, 0.699f64, 1.0f64, 0.001f64, 10.0f64);
    let top = amax.r_log10();
    let mut itop = top as i64;
    if top < 0.0 {
        itop -= 1;
    }
    let fi = itop as f64;
    let mut a2 = fi + 1.0;
    if top <= fi + b3 + ohoh1 {
        a2 = fi + b3;
    }
    if top <= fi + b2 + ohoh1 {
        a2 = fi + b2;
    }
    if top <= fi + b1 + ohoh1 {
        a2 = fi + b1;
    }
    let amax_new = ten.r_powf(a2);
    let bot = amin.r_log10();
    let mut ibot = bot as i64;
    if bot < 0.0 {
        ibot -= 1;
    }
    let fb = ibot as f64;
    let mut a1 = fb;
    if bot >= fb + b2 - ohoh1 {
        a1 = fb + b2;
    }
    if bot >= fb + b3 - ohoh1 {
        a1 = fb + b3;
    }
    if bot >= fb + b4 - ohoh1 {
        a1 = fb + b4;
    }
    (ten.r_powf(a1), amax_new)
}

/// The table's `NXS`/`JXS` entries `tplots` reads (module variables of `aceth`).
struct Th<'a> {
    xss: &'a [f64],
    nil: i64,
    nieb: i64,
    idpnc: i64,
    ncl: i64,
    ifeng: i64,
    ncli: i64,
    itie: i64,
    itxe: i64,
    itce: i64,
    itca: i64,
    itcei: i64,
    itcai: i64,
}

impl Th<'_> {
    /// `xss(i)`, 1-based; zero outside the table.
    fn x(&self, i: i64) -> f64 {
        if i >= 1 && (i as usize) <= self.xss.len() {
            self.xss[(i - 1) as usize]
        } else {
            0.0
        }
    }
    fn xi(&self, i: i64) -> i64 {
        fmt::nint(self.x(i))
    }
}

/// Linear interpolation of the table `(e1,x1)-(e2,x2)` over the pair of blocks at
/// `ebase` (energies) and `xbase` (values) with `n` points, as `tplots` loops
/// `do while (j.lt.n)`: the last `j` whose interval holds `e` sets the value.
fn interp_blocks(t: &Th, e: f64, ebase: i64, xbase: i64, n: i64, cur: f64) -> f64 {
    let mut v = cur;
    let mut j = 0;
    while j < n {
        j += 1;
        let (e1, x1) = (t.x(ebase + j), t.x(xbase + j));
        let (e2, x2) = (t.x(ebase + j + 1), t.x(xbase + j + 1));
        if e >= e1 && e <= e2 {
            v = x1 + (e - e1) * (x2 - x1) / (e2 - e1);
        }
    }
    v
}

/// The `wt` of the bin pattern (`ifeng = 0` flat, `1` the `1 4 10 .. 10 4 1` one).
fn bin_weight(ifeng: i64, j: i64, nbini: i64) -> f64 {
    if ifeng == 0 {
        1.0
    } else {
        let mut wt = 10.0;
        if j == 1 || j == nbini {
            wt = 1.0;
        }
        if j == 2 || j == nbini - 1 {
            wt = 4.0;
        }
        wt
    }
}

/// `ubar` of every incident energy (`tplots`' mubar loop), from `loc` as `tplots` walks it.
fn mubar_of(t: &Th, nie: i64, i: i64, loc: &mut i64, nang: i64, nbini: i64) -> f64 {
    let _ = (nie, i);
    let mut ubar = 0.0;
    let mut sum = 0.0;
    if t.ifeng <= 1 {
        for j in 1..=nbini {
            let wt = bin_weight(t.ifeng, j, nbini);
            for k in 2..=nang + 1 {
                ubar += wt * t.x(*loc + k);
                sum += wt;
            }
            *loc += nang + 1;
        }
    } else {
        let mut cdl = 0.0;
        *loc += nang + 3;
        for _ in 2..=nbini {
            let p = t.x(*loc + 3) - cdl;
            for k in 1..=nang {
                ubar += t.x(*loc + 3 + k) * p / 2.0;
                ubar += t.x(*loc + 3 + k - nang - 3) * p / 2.0;
                sum += p;
            }
            cdl = t.x(*loc + 3);
            *loc += nang + 3;
        }
    }
    ubar / sum
}

/// `ebar` of one incident energy (`tplots`' ebar loop).
fn ebar_of(t: &Th, loc: &mut i64, nang: i64, nbini: i64) -> f64 {
    let mut ebar = 0.0;
    let mut sum = 0.0;
    if t.ifeng <= 1 {
        for j in 1..=nbini {
            let eprime = t.x(*loc + 1);
            let wt = bin_weight(t.ifeng, j, nbini);
            ebar += wt * eprime;
            sum += wt;
            *loc += nang + 1;
        }
    } else {
        let (mut cdl, mut xl, mut pl) = (0.0f64, 0.0f64, 0.0f64);
        *loc += nang + 3;
        for _ in 2..=nbini {
            let x = t.x(*loc + 1);
            let p = t.x(*loc + 2);
            let mut u = (pl - (p - pl) * xl / (x - xl)) * (x * x - xl * xl) / 2.0 + ((p - pl) / (x - xl)) * (x * x * x - xl * xl * xl) / 3.0;
            let ul = (x - xl) * (p + pl) / 2.0;
            u /= ul;
            let un = t.x(*loc + 3) - cdl;
            ebar += u * un;
            sum += un;
            xl = x;
            pl = p;
            cdl = t.x(*loc + 3);
            *loc += nang + 3;
        }
    }
    ebar / sum
}

/// One of the four 3-D secondary-energy pages (`zdiv` is 1000 for the first, 500 after).
#[allow(clippy::too_many_arguments)]
fn pages_3d(t: &Th, w: &mut Plot, hk: &str, it: usize, iwcol: i64, nie: i64, (xmin, xmax, ymin, ymax): (f64, f64, f64, f64), zdiv: f64) {
    let nang = t.nil - 1;
    let mut loc = t.itxe - 1 + 2 * nie;
    let (mut zmin, mut zmax) = (1000.0f64, 0.0f64);
    for ie in 1..=nie {
        let e = t.x(t.itie + ie);
        let nbini = t.xi(t.itxe - 1 + nie + ie);
        for _ in 1..=nbini {
            let ep = t.x(loc + 1);
            if e >= ymin && e <= ymax && ep > xmin && ep < xmax {
                let p = t.x(loc + 2);
                if p < zmin {
                    zmin = p;
                }
                if p > zmax {
                    zmax = p;
                }
            }
            loc += nang + 3;
        }
    }
    if zmax > 0.0 {
        zmin = zmax / zdiv;
        (zmin, zmax) = ascll(zmin, zmax);
        w.page(iwcol, hk, it);
        w.line("'thermal inelastic'/");
        w.line("-4 2/");
        w.axis(xmin, xmax, 1.0);
        w.line("'<s>ec. <e>nergy'/");
        w.axis(ymin, ymax, 1.0);
        w.line("'<e>nergy (<m>e<v>)'/");
        w.axis(zmin, zmax, 1.0);
        w.line("'<p>rob/<m>e<v>'/");
        w.line("/");
        w.line("10 -15 15 3.5 5.5 2.5/");
        w.line("1/");
        let mut loc = t.itxe - 1 + 2 * nie;
        for ie in 1..=nie {
            let e = t.x(t.itie + ie);
            let inside = e >= ymin && e <= ymax;
            if inside {
                w.single(e);
            }
            let nbini = t.xi(t.itxe - 1 + nie + ie);
            for _ in 1..=nbini {
                let ep = t.x(loc + 1);
                let mut p = t.x(loc + 2);
                if p < zmin {
                    p = zmin;
                }
                if inside && ep > xmin && ep <= xmax {
                    w.pair(ep, p);
                }
                loc += nang + 3;
            }
            if inside {
                w.line("/");
            }
        }
        w.line("/");
    }
}

/// `tplots(nout, hk)`: the viewr input text of the thermal table `raw`, whose
/// comment is `hk`. Colour pages (`ipcol = 2`, `iwcol = 3`).
///
/// # Errors
/// [`NjoyError::InvalidInput`] where upstream would loop forever (a
/// non-positive energy step, or `iskip = 0`).
pub fn tplots(raw: &RawAceTable, hk: &str) -> Result<String, NjoyError> {
    let hk = fmt::pad70(hk);
    let it = fmt::hk_it(&hk);
    let (ipcol, iwcol) = (2i64, 3i64);
    let n = |k: usize| i64::from(raw.nxs[k]);
    let j = |k: usize| i64::from(raw.jxs[k]);
    let t = Th {
        xss: &raw.xss,
        nil: n(2),
        nieb: n(3),
        idpnc: n(4),
        ncl: n(5),
        ifeng: n(6),
        ncli: n(7),
        itie: j(0),
        itxe: j(2),
        itce: j(3),
        itca: j(5),
        itcei: j(6),
        itcai: j(8),
    };
    let (big, scale) = (1.0e10f64, 1.0e6f64);
    let mut w = Plot::default();
    w.line(&format!("1 2 .30{}/", fmt::fi(ipcol, 3)));
    let nie = t.xi(t.itie);
    let nee = if t.itce > 0 { t.xi(t.itce) } else { 0 };
    let neei = if t.itcei > 0 { t.xi(t.itcei) } else { 0 };
    let idpnc = t.idpnc;
    let coh = idpnc == 4 || idpnc == 5;

    // ---- log-log total, inelastic and elastic ----
    let (mut xmin, mut xmax, mut ymin, mut ymax) = (big, 0.0f64, big, -big);
    let (mut xelas, mut xielas, mut xn) = (0.0f64, 0.0f64, 0.0f64);
    let mut e;
    for i in 1..=nie {
        e = t.x(t.itie + i);
        let xnelas = t.x(t.itie + nie + i);
        if e < xmin {
            xmin = e;
        }
        if e > xmax {
            xmax = e;
        }
        if xnelas < ymin {
            ymin = xnelas;
        }
        if xnelas > ymax {
            ymax = xnelas;
        }
        if idpnc != 0 {
            let mut jj = 0;
            xelas = 0.0;
            xielas = 0.0;
            while jj < nee {
                jj += 1;
                let e1 = t.x(t.itce + jj);
                let mut x1 = t.x(t.itce + nee + jj);
                if coh {
                    x1 /= e1;
                }
                let e2 = t.x(t.itce + jj + 1);
                let mut x2 = t.x(t.itce + nee + jj + 1);
                if coh {
                    x2 /= e2;
                }
                if e >= e1 && e <= e2 {
                    xelas = x1 + (e - e1) * (x2 - x1) / (e2 - e1);
                    if xelas < ymin {
                        ymin = xelas;
                    }
                    if xelas > ymax {
                        ymax = xelas;
                    }
                }
            }
            if idpnc == 5 {
                let mut jj = 0;
                while jj < neei {
                    jj += 1;
                    let e1 = t.x(t.itcei + jj);
                    let x1 = t.x(t.itcei + neei + jj);
                    let e2 = t.x(t.itcei + jj + 1);
                    let x2 = t.x(t.itcei + neei + jj + 1);
                    if e >= e1 && e <= e2 {
                        xielas = x1 + (e - e1) * (x2 - x1) / (e2 - e1);
                        if xielas < ymin {
                            ymin = xielas;
                        }
                        if xielas > ymax {
                            ymax = xielas;
                        }
                    }
                }
            }
            if xelas > 0.0 {
                let tot = xnelas + xelas + xielas;
                if tot < ymin {
                    ymin = tot;
                }
                if tot > ymax {
                    ymax = tot;
                }
            }
        }
    }
    (xmin, xmax) = ascll(xmin, xmax);
    if ymin < ymax / scale {
        ymin = ymax / scale;
    }
    (ymin, ymax) = ascll(ymin, ymax);
    w.page(iwcol, &hk, it);
    w.line("'<t>hermal cross sections'/");
    let xtag = 2.0 * xmin;
    let mut ytag = 7.0 * ymin.r_log10() / 10.0 + 3.0 * ymax.r_log10() / 10.0;
    ytag = 10.0f64.r_powf(ytag);
    w.tag("4 0 2 1", xtag, ytag);
    w.axis(xmin, xmax, 1.0);
    w.line("'<e>nergy (<m>e<v>)'/");
    w.axis(ymin, ymax, 1.0);
    w.line("'<c>ross section (barns)'/");
    w.line("/");
    w.style(iwcol, 1);
    w.line("'inelastic'/");
    w.line("0/");
    for i in 1..=nie {
        w.pair(t.x(t.itie + i), t.x(t.itie + nie + i));
    }
    w.line("/");
    if idpnc != 0 {
        w.line("2/");
        w.line("/");
        w.style(iwcol, 2);
        w.line(if idpnc == 3 { "'incoherent elastic'/" } else { "'coherent elastic'/" });
        w.line("0/");
        e = 0.0;
        if coh {
            e = t.x(t.itce + 1);
            let xs = t.x(t.itce + nee + 1) / e / 100.0;
            w.pair(e, xs);
        }
        for i in 1..=nee {
            e = t.x(t.itce + i);
            let mut xs = t.x(t.itce + nee + i);
            if coh {
                xs /= e;
            }
            w.pair(e, xs);
            if coh && i < nee {
                e = t.x(t.itce + i + 1);
                xs = t.x(t.itce + nee + i) / e;
                w.pair(e, xs);
            }
        }
        if coh {
            for _ in 1..=25 {
                e += e / 10.0;
                if e > xmax {
                    break;
                }
                let xs = t.x(t.itce + 2 * nee) / e;
                w.pair(e, xs);
            }
        }
        w.line("/");
        if idpnc == 5 {
            w.line("3/");
            w.line("/");
            w.style(iwcol, 3);
            w.line("'incoherent elastic'/");
            w.line("0/");
            for i in 1..=neei {
                w.pair(t.x(t.itcei + i), t.x(t.itcei + neei + i));
            }
            w.line("/");
            w.line("4/");
        } else {
            w.line("3/");
        }
        w.line("/");
        w.line("/");
        w.line("'total'/");
        w.line("0/");
        let mut i = 0;
        let mut idone = false;
        e = 0.0;
        while !idone && i < nee {
            i += 1;
            e = t.x(t.itie + i);
            if e >= t.x(t.itce + 1) {
                idone = true;
            } else {
                xn = t.x(t.itie + nie + i);
                if idpnc == 5 {
                    xielas = interp_blocks(&t, e, t.itcei, t.itcei + neei, neei, xielas);
                }
                w.pair(e, xn + xielas);
            }
        }
        if coh {
            e = t.x(t.itce + 1);
            xn = interp_blocks(&t, e, t.itie, t.itie + nie, nie, xn);
            if idpnc == 5 {
                xielas = interp_blocks(&t, e, t.itcei, t.itcei + neei, neei, xielas);
            }
            w.pair(e, xn + xielas);
        }
        for i in 1..=nee {
            e = t.x(t.itce + i);
            let mut xs = t.x(t.itce + nee + i);
            xn = 0.0;
            xielas = 0.0;
            if coh {
                xs /= e;
            }
            xn = interp_blocks(&t, e, t.itie, t.itie + nie, nie, xn);
            if idpnc == 5 {
                xielas = interp_blocks(&t, e, t.itcei, t.itcei + neei, neei, xielas);
            }
            w.pair(e, xs + xn + xielas);
            if i < nee && coh {
                e = t.x(t.itce + i + 1);
                xs = t.x(t.itce + nee + i) / e;
                xn = interp_blocks(&t, e, t.itie, t.itie + nie, nie, xn);
                if idpnc == 5 {
                    xielas = interp_blocks(&t, e, t.itcei, t.itcei + neei, neei, xielas);
                }
                w.pair(e, xs + xn + xielas);
            }
        }
        if coh {
            for _ in 1..=25 {
                e += e / 10.0;
                if e > xmax {
                    break;
                }
                let xs = t.x(t.itce + 2 * nee) / e;
                xn = interp_blocks(&t, e, t.itie, t.itie + nie, nie, xn);
                if idpnc == 5 {
                    xielas = interp_blocks(&t, e, t.itcei, t.itcei + neei, neei, xielas);
                }
                w.pair(e, xs + xn + xielas);
            }
        }
        if idpnc == 5 {
            for i in 1..=neei {
                if e < t.x(t.itcei + i) {
                    e = t.x(t.itcei + i);
                    xielas = t.x(t.itcei + neei + i);
                    xn = interp_blocks(&t, e, t.itie, t.itie + nie, nie, xn);
                    w.pair(e, xn + xielas);
                }
            }
        }
        w.line("/");
    }
    let _ = (xelas, xn, xielas);

    // ---- mubar ----
    let mut nang = t.nil + 1;
    if t.ifeng > 1 {
        nang = t.nil - 1;
    }
    let mut nbini = t.nieb;
    (xmin, xmax, ymin, ymax) = (big, 0.0, big, -big);
    let mut loc = t.itxe - 1;
    if t.ifeng > 1 {
        loc += 2 * nie;
    }
    for i in 1..=nie {
        e = t.x(t.itie + i);
        if t.ifeng > 1 {
            nbini = t.xi(t.itxe - 1 + nie + i);
        }
        let ubar = mubar_of(&t, nie, i, &mut loc, nang, nbini);
        if e < xmin {
            xmin = e;
        }
        if e > xmax {
            xmax = e;
        }
        if ubar < ymin {
            ymin = ubar;
        }
        if ubar > ymax {
            ymax = ubar;
        }
    }
    if nee > 0 {
        ymin = -1.0;
        ymax = 1.0;
    }
    (xmin, xmax) = ascll(xmin, xmax);
    let (y1, y2, major, _minor) = ascle(4, ymin, ymax);
    (ymin, ymax) = (y1, y2);
    let ystep = (ymax - ymin) / major as f64;
    w.page(iwcol, &hk, it);
    w.line("'<t>hermal mubar'/");
    let xtag = 2.0 * xmin;
    let ytag = 7.0 * ymin / 10.0 + 3.0 * ymax / 10.0;
    w.tag("3 0 2 1", xtag, ytag);
    w.axis(xmin, xmax, 1.0);
    w.line("'<e>nergy (<m>e<v>)'/");
    w.axis(ymin, ymax, ystep);
    w.line("'<m>ubar'/");
    w.line("/");
    w.style(iwcol, 1);
    w.line("'inelastic'/");
    w.line("0/");
    loc = t.itxe - 1;
    if t.ifeng > 1 {
        loc += 2 * nie;
    }
    for i in 1..=nie {
        e = t.x(t.itie + i);
        if t.ifeng > 1 {
            nbini = t.xi(t.itxe - 1 + nie + i);
        }
        let ubar = mubar_of(&t, nie, i, &mut loc, nang, nbini);
        w.pair(e, ubar);
    }
    w.line("/");
    if nee != 0 {
        if coh {
            w.line("2/");
            w.line("/");
            w.style(iwcol, 2);
            w.line("'coherent elastic'/");
            w.line("0/");
            e = t.x(t.itce + 1);
            let ell = t.x(t.itce + nee);
            let mut guard = 0;
            while e <= ell {
                guard += 1;
                if guard > 1_000_000 || !(e > 0.0) {
                    return Err(NjoyError::InvalidInput("tplots: the coherent mubar energy loop does not advance".into()));
                }
                let mut idone = false;
                let mut i = 0;
                let (mut ubar, mut sum, mut bl) = (0.0f64, 0.0f64, 0.0f64);
                while !idone && i < nee {
                    i += 1;
                    let ei = t.x(t.itce + i);
                    if ei > e {
                        idone = true;
                    } else {
                        let ui = 1.0 - 2.0 * ei / e;
                        let bi = t.x(t.itce + nee + i) - bl;
                        ubar += bi * ui / e;
                        sum += bi / e;
                    }
                    bl = t.x(t.itce + nee + i);
                }
                ubar /= sum;
                w.pair(e, ubar);
                e += e / 50.0;
            }
            w.line("/");
        }
        if idpnc == 3 {
            w.line("2/");
            w.line("/");
            w.style(iwcol, 2);
            w.line("'incoherent elastic'/");
            w.line("0/");
            let mut loc = t.itca - 1;
            for i in 1..=nee {
                e = t.x(t.itce + i);
                let mut ubar = 0.0;
                for jj in 1..=t.ncl + 1 {
                    ubar += t.x(loc + jj) / (t.ncl + 1) as f64;
                }
                w.pair(e, ubar);
                loc += t.ncl + 1;
            }
            w.line("/");
        }
        if idpnc == 5 {
            w.line("3/");
            w.line("/");
            w.style(iwcol, 3);
            w.line("'incoherent elastic'/");
            w.line("0/");
            let mut loc = t.itcai - 1;
            for i in 1..=neei {
                e = t.x(t.itcei + i);
                let mut ubar = 0.0;
                for jj in 1..=t.ncli + 1 {
                    ubar += t.x(loc + jj) / (t.ncli + 1) as f64;
                }
                w.pair(e, ubar);
                loc += t.ncli + 1;
            }
            w.line("/");
        }
    }

    // ---- ebar ----
    nang = t.nil + 1;
    if t.ifeng > 1 {
        nang = t.nil - 1;
    }
    nbini = t.nieb;
    (xmin, xmax, ymin, ymax) = (big, 0.0, big, -big);
    loc = t.itxe - 1;
    if t.ifeng > 1 {
        loc += 2 * nie;
    }
    for i in 1..=nie {
        e = t.x(t.itie + i);
        if t.ifeng > 1 {
            nbini = t.xi(t.itxe - 1 + nie + i);
        }
        let ebar = ebar_of(&t, &mut loc, nang, nbini);
        if e < xmin {
            xmin = e;
        }
        if e > xmax {
            xmax = e;
        }
        if ebar < ymin {
            ymin = ebar;
        }
        if ebar > ymax {
            ymax = ebar;
        }
    }
    (xmin, xmax) = ascll(xmin, xmax);
    (ymin, ymax) = ascll(ymin, ymax);
    if ymin < ymax / scale {
        ymin = ymax / scale;
    }
    w.page(iwcol, &hk, it);
    w.line("'<t>hermal ebar'/");
    let xtag = 2.0 * xmin;
    let mut ytag = 2.0 * ymin.r_log10() / 10.0 + 8.0 * ymax.r_log10() / 10.0;
    ytag = 10.0f64.r_powf(ytag);
    w.tag("4 0 2 1", xtag, ytag);
    w.axis(xmin, xmax, 1.0);
    w.line("'<e>nergy (<m>e<v>)'/");
    w.axis(ymin, ymax, 1.0);
    w.line("'<e>bar (<m>e<v>)'/");
    w.line("/");
    w.style(iwcol, 1);
    w.line("'inelastic'/");
    w.line("0/");
    loc = t.itxe - 1;
    if t.ifeng > 1 {
        loc += 2 * nie;
    }
    for i in 1..=nie {
        e = t.x(t.itie + i);
        if t.ifeng > 1 {
            nbini = t.xi(t.itxe - 1 + nie + i);
        }
        let ebar = ebar_of(&t, &mut loc, nang, nbini);
        w.pair(e, ebar);
    }
    w.line("/");
    if t.ifeng <= 1 {
        w.line("99/");
        return Ok(w.text);
    }

    // ---- 3-d plots of the continuous secondary-energy distributions ----
    pages_3d(&t, &mut w, &hk, it, iwcol, nie, (5.0 / scale / 100_000.0, 0.5 / scale, 1.0 / scale / 100_000.0, 2.0 / scale / 100.0), 1000.0);
    pages_3d(&t, &mut w, &hk, it, iwcol, nie, (2.0 / scale / 1000.0, 5.0 / scale / 10.0, 2.0 / scale / 100.0, 2.0 / scale / 10.0), 500.0);
    pages_3d(&t, &mut w, &hk, it, iwcol, nie, (2.0 / scale / 100.0, 2.0 / scale, 2.0 / scale / 10.0, 2.0 / scale), 500.0);
    pages_3d(&t, &mut w, &hk, it, iwcol, nie, (1.0 / scale / 100.0, 10.0 / scale, 2.0 / scale, 10.0 / scale), 500.0);

    // ---- angle-energy distributions at several incident energies ----
    let mut ie = 19i64;
    let iskip = (nie - ie) / 4;
    while ie < nie {
        let nang = t.nil - 1;
        let mut loc = t.xi(t.itxe - 1 + ie);
        let nbini = t.xi(t.itxe - 1 + nie + ie);
        let zmin = 1.0 / 100.0;
        let zmax = 10.0f64;
        let (mut ymin, mut ymax) = (10.0f64, 1.0 / scale / 10_000_000.0);
        let (xmin, xmax) = (-1.0f64, 1.0f64);
        let mut epl = 0.0f64;
        loc += nang + 3;
        for _ in 2..nbini {
            let ep = (t.x(loc + 1) + epl) / 2.0;
            if t.x(loc + 1) > epl {
                if ep < ymin {
                    ymin = ep;
                }
                if ep > ymax {
                    ymax = ep;
                }
                epl = t.x(loc + 1);
            }
            loc += nang + 3;
        }
        (ymin, ymax) = ascll(ymin, ymax);
        w.page(iwcol, &hk, it);
        w.line(&format!("'thermal inelastic for e={} MeV'/", fmt::fe(t.x(t.itie + ie), 10, 3, true)));
        w.line("-2 2/");
        w.axis(xmin, xmax, 0.5);
        w.line("'<c>osine'/");
        w.axis(ymin, ymax, 1.0);
        w.line("'<s>ec. <e>nergy'/");
        w.axis(zmin, zmax, 1.0);
        w.line("'<p>rob/cosine'/");
        w.line("/");
        w.line("10 -15 15 3.5 5.5 2.5/");
        w.line("1/");
        let mut loc = t.xi(t.itxe - 1 + ie);
        let skip = 1.0 + (ymax / ymin).r_log10() / 60.0;
        let mut epl = 0.0f64;
        loc += nang + 3;
        for jj in 2..nbini {
            let ep = (t.x(loc + 1) + epl) / 2.0;
            if t.x(loc + 1) > epl && (jj == 2 || ep > skip * epl) {
                w.single(ep);
                let mut ul = -1.0f64;
                w.pair(-1.0, zmin);
                for k in 1..=nang {
                    let u = t.x(loc + 3 + k);
                    let mut un = if k < nang { (u + t.x(loc + 3 + k + 1)) / 2.0 } else { 1.0 };
                    if k == 1 && u - ul > 5.0 * (un - u) {
                        ul = u - 3.0 * (un - u);
                    }
                    if k == nang && un - u > 5.0 * (u - ul) {
                        un = u + 3.0 * (u - ul);
                    }
                    let mut p = 1.0f64;
                    p /= nang as f64;
                    if un != ul {
                        p /= un - ul;
                    }
                    if k == 1 {
                        w.pair(ul, zmin);
                    }
                    w.pair(u, p);
                    if k == nang {
                        w.pair(un, zmin);
                    }
                    ul = un;
                }
                w.pair(1.0, zmin);
                w.line("/");
                epl = t.x(loc + 1);
            }
            loc += nang + 3;
        }
        w.line("/");
        if iskip <= 0 {
            return Err(NjoyError::InvalidInput("tplots: iskip <= 0 (upstream loops forever)".into()));
        }
        ie += iskip;
    }
    w.line("99/");
    Ok(w.text)
}
