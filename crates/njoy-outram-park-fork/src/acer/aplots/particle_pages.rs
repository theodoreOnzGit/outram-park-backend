// Ported from NJOY2016 `src/acefc.f90` (git commit ac5adf5f33d893e42f2eed7fb286b0d51c7580da; the 2016.80 line numbers
// after 7547 are 3 higher): `subroutine aploxp` (`acefc.f90:18790-19463`).
// NJOY2016 is under a modified BSD 3-Clause (LANL/DOE) licence, GPL-compatible; this derivative file is
// distributed under AGPL-3.0-only. This is a modified, non-LANL version, not endorsed by LANL/DOE.
// See crate root LICENSE.njoy + NOTICE.

//! Particle-production pages (`aploxp`, tables with `ntype > 0`): particle
//! heating, recoil heating, production cross sections, then for each
//! production type the 3-D emission page of every law 4/44/61 reaction and
//! the angular-distribution pages.
//!
//! Kept as upstream has them: the `it` of `hk(1:it)` is reset by the pages
//! that recompute it and *not* by the angular pages; a particle with no label
//! text (`ipt` not 1, 9, 31, 32, 33, 34; and 2 on the emission pages) gets no
//! label record.

use super::angular_pages::angular_perspective;
use super::fmt::{hk_it, AceTable, Plot};
use super::scale::{ascle, ascll, plot_name};

const NDEN: i64 = 4000;
const DELTA: f64 = 0.01;
const UP: f64 = 1.01;
const BIG: f64 = 1.0e10;
const SMALL: f64 = 1.0e-12;
const FIFTH: f64 = 1.0 / 5.0;

/// The curve label of the heating and production pages by `ipt`.
fn curve_label(ipt: i64, photons: &str) -> Option<String> {
    let t = match ipt {
        1 => "neutrons",
        2 => photons,
        9 => "protons",
        31 => "deuterons",
        32 => "tritons",
        33 => "he-3",
        34 => "alphas",
        _ => return None,
    };
    Some(format!("'{t}'/"))
}

/// `(ipt, hpd, iaa, naa)` of production type `i` (1-based).
fn ptype(a: &AceTable, i: i64) -> (i64, i64, i64, i64) {
    let ipt = a.xi(a.ptype + i - 1);
    let hpd = a.xi(a.ploct + 10 * (i - 1));
    (ipt, hpd, a.xi(hpd), a.xi(hpd + 1))
}

/// One curve of a production page: `col` selects the heating (`hpd+1+naa+i`)
/// or the cross section (`hpd+1+i`) column.
fn production_curve(a: &AceTable, w: &mut Plot, ii: i64, heating: bool, thin: f64) {
    let (ipt, hpd, iaa, naa) = ptype(a, ii);
    if ii > 1 {
        w.line("2/");
        w.line("/");
    }
    w.line(&format!("0 0 0{}/", super::fmt::fi(ii - 1, 2)));
    if let Some(l) = curve_label(ipt, if heating { "photons" } else { "photons/5" }) {
        w.line(&l);
    }
    w.line("0/");
    let mut xlast = SMALL;
    for i in 1..=naa {
        let e = a.x(a.esz + iaa - 2 + i);
        if e > FIFTH {
            let mut xs = if heating { a.x(hpd + 1 + naa + i) } else { a.x(hpd + 1 + i) };
            if !heating && ipt == 2 {
                xs /= 5.0;
            }
            if naa <= NDEN || e >= xlast + thin || i == naa {
                w.pair(e, xs);
                xlast = e;
            }
        }
    }
    w.line("/");
}

fn heating_page(a: &AceTable, w: &mut Plot, hk: &str, iwcol: i64) {
    let it = hk_it(hk);
    let (mut xmin, mut xmax, mut ymin, mut ymax) = (BIG, 0.0f64, BIG, -BIG);
    for i in 1..=a.ntype {
        let (_ipt, hpd, iaa, naa) = ptype(a, i);
        for ie in 1..=naa {
            let e = a.x(a.esz + iaa + ie - 2);
            if e > FIFTH {
                let xs = a.x(hpd + 1 + naa + ie);
                if e < xmin {
                    xmin = e;
                }
                if e > xmax {
                    xmax = e;
                }
                if xs < ymin {
                    ymin = xs;
                }
                if xs > ymax {
                    ymax = xs;
                }
            }
        }
    }
    if ymax == 0.0 {
        return;
    }
    let (x1, x2, major, _) = ascle(4, xmin, xmax);
    xmin = x1;
    xmax = x2;
    let xstep = (xmax - xmin) / major as f64;
    let (y1, y2, major, _) = ascle(4, ymin, ymax);
    ymin = y1;
    ymax = y2;
    let ystep = (ymax - ymin) / major as f64;
    w.page(iwcol, hk, it);
    w.line("'<p>article heating contributions'/");
    let xtag = 95.0 * xmin / 100.0 + 5.0 * xmax / 100.0;
    let ytag = ymin / 10.0 + 9.0 * ymax / 10.0;
    w.tag("1 0 2 1", xtag, ytag);
    w.axis(xmin, xmax, xstep);
    w.line("'<e>nergy (<m>e<v>)'/");
    w.axis(ymin, ymax, ystep);
    w.line("'<m>e<v>/collision'/");
    w.line("/");
    let thin = (xmax - xmin) / NDEN as f64;
    for ii in 1..=a.ntype {
        production_curve(a, w, ii, true, thin);
    }
}

fn recoil_heat(a: &AceTable, i: i64) -> f64 {
    let nes = a.nes;
    let mut heat = a.x(a.esz + 4 * nes - 1 + i);
    for j in 1..=a.ntype {
        let (_ipt, hpd, iaa, naa) = ptype(a, j);
        let ie = i - iaa - 1;
        if ie >= 1 && ie <= naa {
            heat -= a.x(hpd + 1 + naa + ie);
        }
    }
    heat
}

fn recoil_page(a: &AceTable, w: &mut Plot, hk: &str, iwcol: i64) {
    let it = hk_it(hk);
    let nes = a.nes;
    let (mut xmin, mut xmax, mut ymin, mut ymax) = (BIG, 0.0f64, BIG, -BIG);
    for i in 1..=nes {
        let e = a.x(a.esz - 1 + i);
        let heat = recoil_heat(a, i);
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
    if ymin == 0.0 && ymax == 0.0 {
        return;
    }
    if ymin < 0.0 && ymax < -ymin / 2.0 {
        ymax = -ymin / 2.0;
    }
    let (x1, x2, major, _) = ascle(4, xmin, xmax);
    xmin = x1;
    xmax = x2;
    let xstep = (xmax - xmin) / major as f64;
    let (y1, y2, major, _) = ascle(4, ymin, ymax);
    ymin = y1;
    ymax = y2;
    let ystep = (ymax - ymin) / major as f64;
    w.page(iwcol, hk, it);
    w.line("'<r>ecoil <h>eating'/");
    w.line("1 0 2 1/");
    w.axis(xmin, xmax, xstep);
    w.line("'<e>nergy (<m>e<v>)'/");
    w.axis(ymin, ymax, ystep);
    w.line("'<h>eating (<m>e<v>/reaction)'/");
    w.line("/");
    w.line("/");
    w.line("'recoil heating'/");
    w.line("0/");
    let thin = (xmax - xmin) / NDEN as f64;
    let mut xlast = SMALL;
    for i in 1..=nes {
        let e = a.x(a.esz - 1 + i);
        if e >= FIFTH && (nes <= NDEN || e >= xlast + thin || i == nes) {
            w.pair(e, recoil_heat(a, i));
            xlast = e;
        }
    }
    w.line("/");
}

fn production_page(a: &AceTable, w: &mut Plot, hk: &str, iwcol: i64) {
    let it = hk_it(hk);
    let (mut xmin, mut xmax, mut ymin, mut ymax) = (BIG, 0.0f64, BIG, -BIG);
    for i in 1..=a.ntype {
        let (ipt, hpd, iaa, naa) = ptype(a, i);
        for ie in 1..=naa {
            let e = a.x(a.esz + iaa + ie - 2);
            if e > FIFTH {
                let mut xs = a.x(hpd + 1 + ie);
                if ipt == 2 {
                    xs /= 5.0;
                }
                if e < xmin {
                    xmin = e;
                }
                if e > xmax {
                    xmax = e;
                }
                if xs < ymin {
                    ymin = xs;
                }
                if xs > ymax {
                    ymax = xs;
                }
            }
        }
    }
    let (x1, x2, major, _) = ascle(4, xmin, xmax);
    xmin = x1;
    xmax = x2;
    let xstep = (xmax - xmin) / major as f64;
    let (y1, y2, major, _) = ascle(4, ymin, ymax);
    ymin = y1;
    ymax = y2;
    let ystep = (ymax - ymin) / major as f64;
    w.page(iwcol, hk, it);
    w.line("'<p>article production cross sections'/");
    let mut xtag = 95.0 * xmin / 100.0 + 5.0 * xmax / 100.0;
    if xmax > 30.0 {
        xtag = 35.0 * xmin / 100.0 + 65.0 * xmax / 100.0;
    }
    let ytag = ymin / 10.0 + 9.0 * ymax / 10.0;
    w.tag("1 0 2 1", xtag, ytag);
    w.axis(xmin, xmax, xstep);
    w.line("'<e>nergy (<m>e<v>)'/");
    w.axis(ymin, ymax, ystep);
    w.line("'<c>ross section (barns)'/");
    w.line("/");
    let thin = (xmax - xmin) / NDEN as f64;
    for ii in 1..=a.ntype {
        production_curve(a, w, ii, false, thin);
    }
}

/// `'<particle>s from <name>'`: the 3-D emission page subtitle.
fn emitted_label(ipt: i64, name: &str) -> Option<String> {
    let lead = match ipt {
        1 => "neutrons",
        9 => "protons",
        31 => "deuterons",
        32 => "tritons",
        33 => "he3s",
        34 => "alphas",
        _ => return None,
    };
    Some(format!("'{lead} from {name}'/"))
}

/// The ` neutron` ... ` alpha` tail of the angular page subtitle.
fn angular_tail(ipt: i64) -> Option<&'static str> {
    Some(match ipt {
        1 => " neutron",
        9 => " proton",
        31 => " deuteron",
        32 => " triton",
        33 => " 3he",
        34 => " alpha",
        _ => return None,
    })
}

/// `aploxp`.
pub fn aploxp(a: &AceTable, w: &mut Plot, hk: &str, iwcol: i64) {
    heating_page(a, w, hk, iwcol);
    recoil_page(a, w, hk, iwcol);
    production_page(a, w, hk, iwcol);
    let mut it = hk_it(hk);

    for i in 1..=a.ntype {
        let ipt = a.xi(a.ptype + i - 1);
        let base = a.ploct + 10 * (i - 1);
        let mtrh = a.xi(base + 1);
        let nmtr = a.xi(a.ntro + i - 1);
        let lsigh = a.xi(base + 3);
        let landh = a.xi(base + 5);
        let andh = a.xi(base + 6);
        let ldlwh = a.xi(base + 7);
        let dlwh = a.xi(base + 8);
        for imt in 1..=nmtr {
            let mt = a.xi(mtrh + imt - 1);
            let l2 = dlwh + a.xi(ldlwh + imt - 1) - 1;
            let law = a.xi(l2 + 1);
            let _ = lsigh;
            if law == 4 || law == 44 || law == 61 {
                let name = plot_name(a, mt);
                let l3 = dlwh + a.xi(l2 + 2) - 1;
                let ne = a.xi(l3 + 1);
                let (mut zmin, mut zmax1, mut zmax2) = (1000.0f64, 0.0f64, 0.0f64);
                let mut ymin = a.x(l3 + 2);
                let mut ymax = a.x(l3 + 2 + ne - 1);
                let test = 3.0;
                let mut i1 = 1;
                if ne > 2 {
                    i1 = 2;
                    if ymax > test * a.x(l3 + 2 + ne - 2) {
                        ymax = a.x(l3 + 2 + ne - 2);
                    }
                }
                for ie in i1..=ne {
                    let e = a.x(l3 + 2 + ie - 1);
                    if e <= UP * ymax {
                        let mut loci = a.xi(l3 + 2 + ne + ie - 1) + dlwh - 1;
                        let nn = a.xi(loci + 1);
                        if nn >= 2 {
                            loci += 1;
                            for j in 1..=nn {
                                let pd = a.x(loci + nn + j);
                                if pd < zmin {
                                    zmin = pd;
                                }
                                if pd > zmax2 && pd < zmax1 {
                                    zmax2 = pd;
                                }
                                if pd > zmax1 {
                                    zmax2 = zmax1;
                                    zmax1 = pd;
                                }
                            }
                        } else if nn == 1 {
                            loci += 1;
                            let pd = a.x(loci + 2);
                            zmin = pd;
                            zmax1 = pd;
                            zmax2 = zmax1;
                        }
                    }
                }
                let mut zmax = if zmax2 / zmax1 < 1.0e-4 && zmax2 != 0.0 { zmax2 } else { zmax1 };
                zmin = zmax / 10000.0;
                (zmin, zmax) = ascll(zmin, zmax);
                let (mut xmin, mut xmax) = (1000.0f64, 0.0f64);
                for ie in 1..=ne {
                    let e = a.x(l3 + 2 + ie - 1);
                    if e <= UP * ymax {
                        let mut loci = a.xi(l3 + 2 + ne + ie - 1) + dlwh - 1;
                        let nn = a.xi(loci + 1);
                        loci += 1;
                        let mut epl = a.x(loci + 1);
                        let mut pdl = a.x(loci + nn + 1);
                        for j in 1..=nn {
                            let ep = a.x(loci + j);
                            let pd = a.x(loci + nn + j);
                            if pd >= zmin && epl < xmin {
                                xmin = epl;
                            }
                            if pdl >= zmin && ep > xmax {
                                xmax = ep;
                            }
                            epl = ep;
                            pdl = pd;
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
                it = hk_it(hk);
                w.page(iwcol, hk, it);
                if let Some(l) = emitted_label(ipt, &name) {
                    w.line(&l);
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
                    let e = a.x(l3 + 2 + ie - 1);
                    if e <= UP * ymax {
                        let mut loci = a.xi(l3 + 2 + ne + ie - 1) + dlwh - 1;
                        let intt = a.xi(loci);
                        let nd = a.xi(loci) / 10;
                        let nn = a.xi(loci + 1);
                        w.single(e);
                        loci += 1;
                        let mut ylast = zmin;
                        let mut k = 0;
                        for j in 1..=nn {
                            let ep = a.x(loci + j);
                            if ep >= xmin && ep <= xmax {
                                let mut pd = a.x(loci + nn + j);
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
                w.line("/");
            }

            // The angular distribution, if any.
            let na = a.xi(landh + imt - 1);
            if na > 0 {
                let mt = a.xi(mtrh + imt - 1).abs();
                let name = plot_name(a, mt);
                let trimmed = name.trim_end();
                let label = angular_tail(ipt).map(|t| format!("{trimmed}{t}"));
                angular_perspective(a, w, hk, &mut it, iwcol, na + andh - 1, andh, label.as_deref());
            }
        }
    }
}
