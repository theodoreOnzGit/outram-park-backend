// Ported from NJOY2016 `src/acefc.f90` (git commit ac5adf5f33d893e42f2eed7fb286b0d51c7580da; the 2016.80 line numbers
// after 7547 are 3 higher): `subroutine aplof4` (`acefc.f90:17208-17431`), `subroutine aplonu`
// (`:17433-17539`) and `subroutine aplodn` (`:18006-18191`).
// NJOY2016 is under a modified BSD 3-Clause (LANL/DOE) licence, GPL-compatible; this derivative file is
// distributed under AGPL-3.0-only. This is a modified, non-LANL version, not endorsed by LANL/DOE.
// See crate root LICENSE.njoy + NOTICE.

//! Angular-distribution pages (`aplof4`), the total-fission nubar page
//! (`aplonu`) and the delayed-neutron pages (`aplodn`).

use super::fmt::{fe, fi, hk_it, nint, AceTable, Plot};
use super::scale::{ascle, ascll, plot_name};
use crate::NjoyError;

const BIG: f64 = 1.0e10;
const EPS: f64 = 1.0e-5;
const DN: f64 = 0.99;
const UP: f64 = 1.01;
const MAXE: i64 = 1200;

/// `it` of `aplof4` and `aploxp`'s angular branch: it is *not* reset to 1, so
/// a blank comment leaves its previous value.
fn update_it(hk: &str, it: &mut usize) {
    for (i, c) in hk.chars().take(70).enumerate() {
        if c != ' ' {
            *it = i + 1;
        }
    }
}

/// The perspective angular-distribution page(s) for one reaction: the
/// `else` branch of `aplof4` and `aploxp`'s "check for an angular
/// distribution" (same code, with `andh` for `and`). `label` is the text after
/// `angular distribution for` (name, and the particle for `aploxp`); `None`
/// writes no subtitle record, as `aploxp` does for a particle it has no text for.
pub fn angular_perspective(a: &AceTable, w: &mut Plot, hk: &str, it: &mut usize, iwcol: i64, na: i64, andd: i64, label: Option<&str>) {
    let ne = a.xi(na);
    let nb = na + ne;
    let (xmin, xmax) = (-1.0f64, 1.0f64);
    let xstep = 1.0f64 / 2.0;
    let mut ymin = a.x(na + 1);
    let mut ymax = a.x(na + ne);
    let mut itwo = 0;
    let mut brk = 0.0f64;
    if ne > 4 {
        let test = 3.0;
        if ymax > test * a.x(na + ne - 1) {
            ymax = a.x(na + ne - 1);
        }
        let test = 50.0;
        if ymax >= test {
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
            ymax = a.x(na + ne);
        }
        let (y1, y2, major, _) = ascle(4, ymin, ymax);
        ymin = y1;
        ymax = y2;
        let ystep = (ymax - ymin) / major as f64;
        let mut zmin = BIG;
        let mut zmax = 0.0f64;
        for i in 1..=ne {
            let e = a.x(na + i);
            if e <= (1.0 + EPS) * ymax && e >= ymin {
                let mut k = nint(a.x(nb + i).abs()) + andd - 1;
                let np = a.xi(k + 1);
                k += 1;
                for j in 1..=np {
                    let pp = a.x(k + np + j);
                    if pp < zmin {
                        zmin = pp;
                    }
                    if pp > zmax {
                        zmax = pp;
                    }
                }
            }
        }
        let rat = 100000.0;
        if zmin <= 0.0 {
            zmin = zmax / rat;
        }
        if zmax / zmin > rat {
            zmin = zmax / rat;
        }
        (zmin, zmax) = ascll(zmin, zmax);
        if zmin == zmax {
            zmax *= 2.0;
            zmin = zmax / 10.0;
        }
        update_it(hk, it);
        w.page(iwcol, hk, *it);
        if let Some(label) = label {
            w.line(&format!("'angular distribution for {label}'/"));
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
        for i in 1..=ne {
            let e = a.x(na + i);
            let mut flag = false;
            if e > (1.0 + EPS) * ymax {
                flag = true;
            }
            if e < ymin {
                flag = true;
            }
            if i > 1 && i < ne && ne > 150 && e - elast < stepm {
                flag = true;
            }
            if !flag {
                w.single(e);
                let mut k = nint(a.x(nb + i).abs()) + andd - 1;
                let intt = a.xi(k);
                let np = a.xi(k + 1);
                k += 1;
                let mut ylast = zmin;
                for j in 1..=np {
                    let cc = a.x(k + j);
                    let mut pp = a.x(k + np + j);
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
        if itwo == 0 {
            itwo = 3;
        }
        if itwo == 2 {
            itwo = 3;
        }
        if itwo == 1 {
            itwo = 2;
        }
    }
}

/// `aplof4`: scattering-contour and angular-distribution pages.
pub fn aplof4(a: &AceTable, w: &mut Plot, hk: &str, iwcol: i64) -> Result<(), NjoyError> {
    let mut it = 0usize;
    let nr1 = a.nr + 1;
    for nn in 1..=nr1 {
        let n = nn - 1;
        let mut na = a.xi(a.land + n);
        if na > 0 {
            let name = if n == 0 {
                "elastic   ".to_string()
            } else {
                let mt = a.xi(a.mtr + n - 1).abs();
                plot_name(a, mt)
            };
            na = na + a.and - 1;
            let ne = a.xi(na);
            let nb = na + ne;
            let k = a.xi(nb + 1);
            if k >= 0 {
                // equiprobable contours
                if ne > MAXE {
                    return Err(NjoyError::InvalidInput("aplof4: too many e values in angular distribution".into()));
                }
                let mut xmin = 1000.0f64;
                let mut imin = ne;
                let mut loce = vec![0i64; (ne + 2) as usize];
                for ie in 1..=ne {
                    let kk = a.xi(ie + nb);
                    if kk > 0 {
                        let x = a.x(na + ie);
                        if x < xmin {
                            xmin = x;
                            imin = ie;
                        }
                    }
                    loce[ie as usize] = kk + a.and - 1;
                }
                let nk = loce[ne as usize] - loce[(ne - 1).max(0) as usize];
                if imin != ne && nk >= 1 {
                    update_it(hk, &mut it);
                    let xmax = a.x(na + ne);
                    let (x1, x2, major, _) = ascle(4, xmin, xmax);
                    let xmin = x1;
                    let xmax = x2;
                    let xstep = (xmax - xmin) / major as f64;
                    w.page(iwcol, hk, it);
                    w.line(&format!("'<s>cattering contours for {name}'/"));
                    w.line("1/");
                    w.axis(xmin, xmax, xstep);
                    w.line("'<e>nergy (<m>e<v>)'/");
                    w.axis(-1.0, 1.0, 0.5);
                    w.line("'<c>osine>'/");
                    w.line("/");
                    for j in 1..=nk {
                        if j > 1 {
                            w.next_curve(j);
                        }
                        w.line("/");
                        w.line("0");
                        for ie in imin..=ne {
                            let kk = loce[ie as usize] + j - 1;
                            w.pair(a.x(na + ie), a.x(kk));
                        }
                        w.line("/");
                    }
                }
            } else {
                angular_perspective(a, w, hk, &mut it, iwcol, na, a.and, Some(&name));
            }
        }
    }
    Ok(())
}

/// The `l`, `j` walk to the nubar block: `(l, j)` after the optional
/// prompt/total header (`aplonu`).
fn nubar_start(a: &AceTable) -> (i64, i64) {
    let mut l = a.nu;
    let mut j = a.xi(l);
    let kf = j;
    if kf < 0 {
        l = l + kf.abs() + 1;
        j = a.xi(l);
    }
    (l, j)
}

/// `aplonu`: the total fission nubar page.
pub fn aplonu(a: &AceTable, w: &mut Plot, hk: &str, iwcol: i64) {
    const STEP: f64 = 1.2;
    let (mut xmin, mut xmax, mut ymin, mut ymax) = (BIG, 0.0f64, BIG, -BIG);
    let (mut l, j) = nubar_start(a);
    if j != 2 {
        let e = a.x(a.esz);
        let emax = a.x(a.esz + a.nes - 1);
        l += 1;
        let n = a.xi(l);
        xmin = e;
        xmax = emax;
        ymin = a.x(l + 1);
        ymax = ymin;
        for i in 2..=n {
            ymax += a.x(l + i) * petir::real::powi(emax, (i - 1) as i32);
        }
    } else {
        l += 1;
        let nr1 = a.xi(l);
        if nr1 > 0 {
            l += 2 * nr1;
        }
        l += 1;
        let ne = a.xi(l);
        for i in 1..=ne {
            let x = a.x(l + i);
            let y = a.x(l + i + ne);
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
    let (y1, y2, major, _) = ascle(4, ymin, ymax);
    ymin = y1;
    ymax = y2;
    let ystep = (ymax - ymin) / major as f64;
    let xstep = (xmax - xmin) / 4.0;
    let it = hk_it(hk);
    w.page(iwcol, hk, it);
    w.line("'<t>otal fission nubar'/");
    w.line("1 0 2/");
    w.axis(xmin, xmax, xstep);
    w.line("'<e>nergy (<m>e<v>)'/");
    w.axis(ymin, ymax, ystep);
    w.line("'<f>ission nubar'/");
    w.line("/");
    w.line("/");
    w.line("0/");
    let (mut l, j) = nubar_start(a);
    if j != 2 {
        let mut e = a.x(a.esz);
        let emax = a.x(a.esz + a.nes - 1);
        l += 1;
        let n = a.xi(l);
        while e < emax {
            let mut sum = a.x(l + 1);
            for i in 2..=n {
                sum += a.x(l + i) * petir::real::powi(e, (i - 1) as i32);
            }
            w.pair(e, sum);
            e += STEP;
        }
    } else {
        l += 1;
        let nr1 = a.xi(l);
        if nr1 > 0 {
            l += 2 * nr1;
        }
        l += 1;
        let ne = a.xi(l);
        for i in 1..=ne {
            w.pair(a.x(l + i), a.x(l + i + ne));
        }
    }
    w.line("/");
}

/// The page subtitle `aplodd` writes by incident particle (`izai`): nothing
/// for a particle it has no text for.
pub fn emission_title(w: &mut Plot, izai: i64, name: &str) {
    let lead = match izai {
        1 => "<n>eutron",
        1001 => "<p>roton",
        1002 => "<d>euteron",
        1003 => "<t>riton",
        2003 => "<H>e-3",
        2004 => "<a>lpha",
        _ => return,
    };
    w.line(&format!("'{lead} emission for {name}'/"));
}

/// The locator walk to the spectrum of delayed group `i` (`aplodn`):
/// returns `(loci, intt, n)` with `loci` pointing at the first energy.
fn delayed_spectrum(a: &AceTable, i: i64) -> (i64, i64, i64) {
    let mut loct = nint(a.x(i - 1 + a.ldnd) + a.dnd as f64 - 1.0);
    let m = a.xi(loct + 3);
    loct = loct + 3 + 2 * m;
    loct += 1;
    let n = a.xi(loct);
    loct = loct + 1 + 2 * n;
    let m = a.xi(loct);
    loct += 2 * m;
    loct += 1;
    let ne = a.xi(loct);
    let loci = nint(a.x(1 + ne + loct)) + a.dnd - 1;
    let intt = a.xi(loci);
    let n = a.xi(loci + 1);
    (loci + 1, intt, n)
}

/// `(decay, frac)` of group `i` from the `dndat` block.
fn delayed_constants(a: &AceTable, i: i64) -> (f64, f64) {
    let mut l = a.dndat;
    let (mut decay, mut frac) = (0.0, 0.0);
    for j in 1..=a.ndnf {
        if j == i {
            decay = a.x(l);
        }
        l += 2;
        let nn = a.xi(l);
        l += nn;
        if j == i {
            frac = a.x(l + 1);
        }
        l += nn + 1;
    }
    (decay, frac)
}

/// `aplodn`: the delayed nubar page and the delayed-spectra page.
pub fn aplodn(a: &AceTable, w: &mut Plot, hk: &str, iwcol: i64) {
    const STEP: f64 = 1.2;
    const SCALE: f64 = 1.0e2;
    let it = hk_it(hk);
    let (mut xmin, mut xmax, mut ymin, mut ymax) = (BIG, 0.0f64, BIG, -BIG);
    let mut l = a.nud;
    l += 1;
    // `nr=nint(xss(l))` assigns the module variable; nothing after it reads it.
    let nr = a.xi(l);
    if nr > 0 {
        l += 2 * nr;
    }
    l += 1;
    let ne = a.xi(l);
    for i in 1..=ne {
        let x = a.x(l + i);
        let y = a.x(l + i + ne);
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
    ymin /= STEP;
    ymax *= STEP;
    let (y1, y2, major, _) = ascle(4, ymin, ymax);
    ymin = y1;
    ymax = y2;
    let ystep = (ymax - ymin) / major as f64;
    let xstep = (xmax - xmin) / 4.0;
    w.page(iwcol, hk, it);
    w.line("'<d>elayed nubar'/");
    w.line("1 0 2/");
    w.axis(xmin, xmax, xstep);
    w.line("'<e>nergy (<m>e<v>)'/");
    w.axis(ymin, ymax, ystep);
    w.line("'<d>elayed nubar'/");
    w.line("/");
    w.line("/");
    w.line("0/");
    for i in 1..=ne {
        w.pair(a.x(l + i), a.x(l + i + ne));
    }
    w.line("/");

    // The delayed spectra.
    let (mut xmin, mut xmax, mut ymin, mut ymax) = (BIG, 0.0f64, BIG, -BIG);
    for i in 1..=a.ndnf {
        let (loci, _intt, n) = delayed_spectrum(a, i);
        let (_decay, frac) = delayed_constants(a, i);
        for j in 1..=n {
            let mut x = a.x(j + loci);
            if x == 0.0 {
                x = a.x(j + 1 + loci) / 10.0;
            }
            let y = frac * a.x(j + loci + n);
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
    (xmin, xmax) = ascll(xmin, xmax);
    if ymin < ymax / SCALE {
        ymin = ymax / SCALE;
    }
    (ymin, ymax) = ascll(ymin, ymax);
    w.page(iwcol, hk, it);
    w.line("'<d>elayed neutron spectra'/");
    let xtag = STEP * xmin;
    let ytag = ymax / 30.0;
    w.tag("4 0 2 1", xtag, ytag);
    w.axis(xmin, xmax, 1.0);
    w.line("'<e>nergy (<m>e<v>)'/");
    w.axis(ymin, ymax, 1.0);
    w.line("'<p>robability'/");
    w.line("/");
    for i in 1..=a.ndnf {
        if i > 1 {
            w.next_curve(i);
        }
        w.style(iwcol, i);
        let (decay, frac) = delayed_constants(a, i);
        w.line(&format!("'group{} frac{:7.4} decay/shake{}'/", fi(i, 2), frac, fe(decay, 10, 3, true)));
        w.line("0/");
        let (loci, intt, n) = delayed_spectrum(a, i);
        let mut m = n;
        if intt == 1 {
            m -= 1;
        }
        for j in 1..=m {
            let mut x = a.x(j + loci);
            if x == 0.0 {
                x = a.x(j + 1 + loci) / 10.0;
            }
            let mut y = frac * a.x(j + loci + n);
            if y < ymin {
                y = ymin;
            }
            w.pair(x, y);
            if intt == 1 {
                let x = a.x(j + 1 + loci);
                let mut y = frac * a.x(j + loci + n);
                if y < ymin {
                    y = ymin;
                }
                w.pair(x, y);
            }
        }
        w.line("/");
    }
}
