// Ported from NJOY2016 `src/acefc.f90` (git commit ac5adf5f33d893e42f2eed7fb286b0d51c7580da; the 2016.80 line numbers
// after 7547 are 3 higher): `subroutine aplodd` (`acefc.f90:17541-18004`) and `subroutine getl7`
// (`:19465-19542`).
// NJOY2016 is under a modified BSD 3-Clause (LANL/DOE) licence, GPL-compatible; this derivative file is
// distributed under AGPL-3.0-only. This is a modified, non-LANL version, not endorsed by LANL/DOE.
// See crate root LICENSE.njoy + NOTICE.

//! Secondary-energy distribution pages (`aplodd`): laws 4, 44, 61 and 67,
//! one 3-D page per reaction and law. Upstream's quirks are kept: law 4's
//! second pass reads `xss(1+n+loci)` with the *reaction* index `n`, and its
//! `test` limit compounds by 0.1 % per energy; law 44/61 passes `zmax`
//! through `ascll` twice; `getl7`'s initialising call overwrites its `ep`.

use super::angular_pages::emission_title;
use super::fmt::{hk_it, nint, AceTable, Plot};
use super::scale::{ascle, ascll, plot_name};
use crate::endf::interp::{terp1, IntLaw};
use crate::NjoyError;

const EMAX: f64 = 1.0e10;
const SMALL: f64 = 1.0e-12;
const DP: f64 = 0.01;

/// `getl7(ep,epnext,loci,a7,f0)` (`acefc.f90:19465`), `a7(k) = xss(base + k)`.
/// Returns `(ep, epnext, f0)`: `ep` is overwritten by the initialising call
/// (`ep < 0`) and `f0` only by a normal call.
fn getl7(a: &AceTable, base: i64, ep_in: f64, loci: i64, f0_in: f64) -> Result<(f64, f64, f64), NjoyError> {
    let a7 = |k: i64| a.x(base + k);
    let mut ep = ep_in;
    let mut epnext = EMAX;
    if ep < 0.0 {
        let nmu = nint(a7(loci + 1));
        let locmu = loci + 2;
        for imu in 1..=nmu {
            let locimu = nint(a7(locmu + nmu + imu - 1));
            ep = a7(locimu + 2);
            if ep < epnext {
                epnext = ep;
            }
        }
        return Ok((ep, epnext, f0_in));
    }
    let intmu = nint(a7(loci));
    let nmu = nint(a7(loci + 1));
    let locmu = loci + 2;
    let mut f0 = 0.0f64;
    let mut ulast = 0.0f64;
    let mut flast = 0.0f64;
    let (mut ep1, mut ep2) = (0.0f64, 0.0f64);
    let mut inow = 0i64;
    for imu in 1..=nmu {
        let u = a7(locmu + imu - 1);
        let locimu = nint(a7(locmu + nmu + imu - 1));
        let intep = nint(a7(locimu));
        let npep = nint(a7(locimu + 1));
        let mut iep = 0;
        let mut idone = false;
        while iep < npep && !idone {
            iep += 1;
            inow = locimu + 1 + iep;
            ep1 = a7(inow);
            ep2 = a7(inow + 1);
            if ep >= ep1 && ep < ep2 {
                idone = true;
            }
        }
        let f;
        if ep >= ep2 {
            f = a7(inow + 1 + npep);
        } else {
            f = terp1(ep1, a7(inow + npep), ep2, a7(inow + 1 + npep), ep, IntLaw::from_code(intep as u32))?;
            let mut epn = ep2;
            let fact1 = 1.0 - 1.0 / 10000.0;
            let fact2 = 1.0 - 2.0 * 1.0 / 10000.0;
            if intep == 1 && ep < fact2 * ep2 {
                epn = fact1 * ep2;
            }
            if epn < epnext {
                epnext = epn;
            }
        }
        if intmu == 1 && imu > 1 {
            f0 += flast * (u - ulast);
        }
        if intmu == 2 && imu > 1 {
            f0 += (f + flast) * (u - ulast) / 2.0;
        }
        ulast = u;
        flast = f;
    }
    Ok((ep, epnext, f0))
}

/// The `j`, `l` walk shared by every law branch: `l` at the law-specific data.
fn law_start(a: &AceTable, loc: i64) -> (i64, i64) {
    let mut l = loc + 1;
    let law = a.xi(l);
    l += 2;
    let mut j = a.xi(l);
    if j != 0 {
        l += 2 * j;
    }
    l += 1;
    j = a.xi(l);
    l = l + 2 * j + 1;
    (law, l)
}

fn header(w: &mut Plot, a: &AceTable, hk: &str, it: usize, iwcol: i64, name: &str) {
    w.page(iwcol, hk, it);
    emission_title(w, a.izai, name);
    w.line("-1 2/");
}

fn law4(a: &AceTable, w: &mut Plot, hk: &str, it: usize, iwcol: i64, n: i64, name: &str, l0: i64) {
    let mut l = l0;
    let (mut zmin, mut zmax1, mut zmax2) = (1000.0f64, 0.0f64, 0.0f64);
    let j = a.xi(l);
    if j != 0 {
        l += 2 * j;
    }
    l += 1;
    let ne = a.xi(l);
    let mut ymin = a.x(l + 1);
    let mut ymax = a.x(l + ne);
    let mut test = 3.0f64;
    if ymax > test * a.x(l + ne - 1) {
        ymax = a.x(l + ne - 1);
    }
    for ie in 2..=ne {
        let e = a.x(ie + l);
        test = ymax + ymax / 1000.0;
        if e <= test {
            let mut loci = nint(a.x(ie + ne + l) + a.dlw as f64 - 1.0);
            let nn = a.xi(loci + 1);
            loci += 1;
            if nn > 2 {
                for j in 1..=nn {
                    let p = a.x(j + nn + loci);
                    if p < zmin {
                        zmin = p;
                    }
                    if p > zmax2 && p < zmax1 {
                        zmax2 = p;
                    }
                    if p > zmax1 {
                        zmax2 = zmax1;
                        zmax1 = p;
                    }
                }
            }
        }
    }
    if zmax1 > 0.0 {
        let mut zmax = if (zmax2 / zmax1 < 1.0e-4) && (zmax2 != 0.0) { zmax2 } else { zmax1 };
        zmin = zmax / 10000.0;
        (zmin, zmax) = ascll(zmin, zmax);
        let (mut xmin, mut xmax) = (1000.0f64, 0.0f64);
        for ie in 2..=ne {
            let e = a.x(ie + l);
            test += test / 1000.0;
            if e <= test {
                let mut loci = nint(a.x(ie + ne + l) + a.dlw as f64 - 1.0);
                let nn = a.xi(loci + 1);
                loci += 1;
                if nn > 2 {
                    let mut epl = a.x(1 + loci);
                    // `xss(1+n+loci)`: upstream's `n` is the reaction index here.
                    let mut ppl = a.x(1 + n + loci);
                    for j in 1..=nn {
                        let ep = a.x(j + loci);
                        let p = a.x(j + nn + loci);
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
        header(w, a, hk, it, iwcol, name);
        w.axis(xmin, xmax, xstep);
        w.line("'<s>ec. <e>nergy'/");
        w.axis(ymin, ymax, ystep);
        w.line("'<e>nergy (<m>e<v>)'/");
        w.axis(zmin, zmax, 1.0);
        w.line("'<p>rob/<m>e<v>'/");
        w.line("/");
        w.line("/");
        w.line("1/");
        for ie in 2..=ne {
            let e = a.x(ie + l);
            let mut loci = nint(a.x(ie + ne + l) + a.dlw as f64 - 1.0);
            let intt = a.xi(loci);
            let nn = a.xi(loci + 1);
            if nn > 2 {
                loci += 1;
                w.single(e);
                let mut ylast = zmin;
                for j in 1..=nn {
                    let ep = a.x(j + loci);
                    let mut p = a.x(j + nn + loci);
                    if p < zmin {
                        p = zmin;
                    }
                    if ep <= xmax {
                        if intt == 1 {
                            w.pair(ep, ylast);
                        }
                        w.pair(ep, p);
                    }
                    ylast = p;
                }
                w.line("/");
            }
        }
        w.line("/");
    }
}

fn law44(a: &AceTable, w: &mut Plot, hk: &str, it: usize, iwcol: i64, name: &str, l0: i64) {
    let mut l = l0;
    let j = a.xi(l);
    if j != 0 {
        l += 2 * j;
    }
    l += 1;
    let ne = a.xi(l);
    let mut ymin = a.x(l + 1);
    let mut ymax = a.x(l + ne);
    let test = 3.0f64;
    if ymax > test * a.x(l + ne - 1) {
        ymax = a.x(l + ne - 1);
    }
    let (mut zmin, mut zmax1, mut zmax2) = (1000.0f64, 0.0f64, 0.0f64);
    for ie in 2..=ne {
        let e = a.x(ie + l);
        let test = ymax + ymax / 1000.0;
        if e <= test {
            let mut loci = nint(a.x(ie + ne + l) + a.dlw as f64 - 1.0);
            let nd = nint(a.x(loci) / 10.0);
            let nn = a.xi(loci + 1);
            if nn > 2 {
                loci += 1;
                for j in 1..=nn {
                    let mut p = a.x(j + nn + loci);
                    if j <= nd {
                        p *= 100.0;
                    }
                    if p < zmin {
                        zmin = p;
                    }
                    if p > zmax2 && p < zmax1 {
                        zmax2 = p;
                    }
                    if p > zmax1 {
                        zmax2 = zmax1;
                        zmax1 = p;
                    }
                }
            }
        }
    }
    if zmax1 > 0.0 {
        let mut zmax = if (zmax2 / zmax1 < 1.0e-4) && (zmax2 != 0.0) { zmax2 } else { zmax1 };
        zmin = zmax / 10000.0;
        (zmin, zmax) = ascll(zmin, zmax);
        let (mut xmin, mut xmax) = (1000.0f64, 0.0f64);
        for ie in 2..=ne {
            let e = a.x(ie + l);
            let test = ymax + ymax / 1000.0;
            if e <= test {
                let mut loci = nint(a.x(ie + ne + l) + a.dlw as f64 - 1.0);
                let nd = nint(a.x(loci) / 10.0);
                let nn = a.xi(loci + 1);
                if nn > 2 {
                    loci += 1;
                    let mut epl = a.x(1 + loci);
                    let mut ppl = a.x(1 + nn + loci);
                    for j in 1..=nn {
                        let ep = a.x(j + loci);
                        let mut p = a.x(j + nn + loci);
                        if j <= nd {
                            p *= 100.0;
                        }
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
        zmin = zmax / 10000.0;
        (zmin, zmax) = ascll(zmin, zmax);
        let thin = (xmax - xmin) / 500.0;
        header(w, a, hk, it, iwcol, name);
        w.axis(xmin, xmax, xstep);
        w.line("'<s>ec. <e>nergy'/");
        w.axis(ymin, ymax, ystep);
        w.line("'<e>nergy (<m>e<v>)'/");
        w.axis(zmin, zmax, 1.0);
        w.line("'<p>rob/<m>e<v>'/");
        w.line("/");
        w.line("/");
        w.line("1/");
        for ie in 2..=ne {
            let e = a.x(ie + l);
            let test = ymax + ymax / 1000.0;
            if e <= test {
                let mut loci = nint(a.x(ie + ne + l) + a.dlw as f64 - 1.0);
                let intt = nint(a.x(loci)) % 10;
                let nd = nint(a.x(loci) / 10.0);
                let nn = a.xi(loci + 1);
                loci += 1;
                w.single(e);
                let mut xlast = SMALL;
                let mut ylast = zmin;
                let mut k = 0;
                for j in 1..=nn {
                    let ep = a.x(j + loci);
                    let mut flag = false;
                    if ep < xmin || ep > xmax {
                        flag = true;
                    }
                    if nn > 500 && ep < xlast + thin && j != 1 && j != nn {
                        flag = true;
                    }
                    if !flag {
                        let mut p = a.x(j + nn + loci);
                        if j <= nd {
                            p *= 100.0;
                        }
                        if p < zmin {
                            p = zmin;
                        }
                        if p > zmax {
                            p = zmax;
                        }
                        if j <= nd {
                            w.pair(ep - DP, zmin);
                        }
                        if intt == 1 && j > nd {
                            w.pair(ep, ylast);
                        }
                        w.pair(ep, p);
                        if j <= nd {
                            w.pair(ep + DP, zmin);
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
}

fn law67(a: &AceTable, w: &mut Plot, hk: &str, it: usize, iwcol: i64, name: &str, l0: i64) -> Result<(), NjoyError> {
    let mut l = l0;
    let base = a.dlw - 1;
    let (mut xmin, mut xmax, mut ymin, mut ymax) = (1000.0f64, 0.0f64, 1000.0f64, 0.0f64);
    let (mut zmax1, mut zmax2) = (0.0f64, 0.0f64);
    let j = a.xi(l);
    if j != 0 {
        l += 2 * j;
    }
    l += 1;
    let ne = a.xi(l);
    let mut f0 = 0.0f64;
    for ie in 2..=ne {
        let e = a.x(ie + l);
        if e < ymin {
            ymin = e;
        }
        if e > ymax {
            ymax = e;
        }
        let loci = a.xi(ie + ne + l);
        let (_ep, mut epnext, f) = getl7(a, base, -1.0, loci, f0)?;
        f0 = f;
        while epnext != EMAX {
            let ep = epnext;
            let (_e, en, f) = getl7(a, base, ep, loci, f0)?;
            epnext = en;
            f0 = f;
            if f0 > zmax2 && f0 < zmax1 {
                zmax2 = f0;
            }
            if f0 > zmax1 {
                zmax2 = zmax1;
                zmax1 = f0;
            }
        }
    }
    if zmax1 > 0.0 {
        let mut zmax = if (zmax2 / zmax1 < 1.0e-4) && (zmax2 != 0.0) { zmax2 } else { zmax1 };
        let mut zmin = zmax / 10000.0;
        (zmin, zmax) = ascll(zmin, zmax);
        for ie in 2..=ne {
            let loci = a.xi(ie + ne + l);
            let (mut ep, mut epnext, f) = getl7(a, base, -1.0, loci, f0)?;
            f0 = f;
            let mut epl = ep;
            let mut ppl = 0.0f64;
            while epnext != EMAX {
                ep = epnext;
                let (_e, en, f) = getl7(a, base, ep, loci, f0)?;
                epnext = en;
                f0 = f;
                if f0 >= zmin && epl < xmin {
                    xmin = epl;
                }
                if ppl >= zmin && ep > xmax {
                    xmax = ep;
                }
                epl = ep;
                ppl = f0;
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
        header(w, a, hk, it, iwcol, name);
        w.axis(xmin, xmax, xstep);
        w.line("'<s>ec. <e>nergy'/");
        w.axis(ymin, ymax, ystep);
        w.line("'<e>nergy (<m>e<v>)'/");
        w.axis(zmin, zmax, 1.0);
        w.line("'<p>rob/<m>e<v>'/");
        w.line("/");
        w.line("/");
        w.line("1/");
        for ie in 2..=ne {
            let e = a.x(ie + l);
            let loci = a.xi(ie + ne + l);
            w.single(e);
            let (_ep, mut epnext, f) = getl7(a, base, -1.0, loci, f0)?;
            f0 = f;
            while epnext != EMAX {
                let ep = epnext;
                let (_e, en, f) = getl7(a, base, ep, loci, f0)?;
                epnext = en;
                f0 = f;
                if f0 < zmin {
                    f0 = zmin;
                }
                w.pair(ep, f0);
            }
            w.line("/");
        }
        w.line("/");
    }
    Ok(())
}

/// `aplodd`: 3-D secondary-energy pages for every reaction with a law 4, 44,
/// 61 or 67 block.
pub fn aplodd(a: &AceTable, w: &mut Plot, hk: &str, iwcol: i64) -> Result<(), NjoyError> {
    let it = hk_it(hk);
    for n in 1..=a.nr {
        let mt = a.xi(a.mtr + n - 1).abs();
        let name = plot_name(a, mt);
        let mut locl = vec![nint(a.x(a.ldlw + n - 1) + a.dlw as f64 - 1.0)];
        let mut l = locl[0];
        while a.xi(l) != 0 {
            l = a.xi(l);
            l = l + a.dlw - 1;
            locl.push(l);
        }
        for &loc in &locl {
            let (law, l) = law_start(a, loc);
            if law == 4 {
                law4(a, w, hk, it, iwcol, n, &name, l);
            } else if law == 44 || law == 61 {
                law44(a, w, hk, it, iwcol, &name, l);
            } else if law == 67 {
                law67(a, w, hk, it, iwcol, &name, l)?;
            }
        }
    }
    Ok(())
}
