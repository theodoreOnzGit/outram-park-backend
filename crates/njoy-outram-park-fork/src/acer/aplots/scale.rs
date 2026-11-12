// Ported from NJOY2016 `src/acefc.f90` (git commit ac5adf5f33d893e42f2eed7fb286b0d51c7580da; the 2016.80 line numbers
// after 7547 are 3 higher): `subroutine ascle` and `subroutine ascll`, and `src/acecm.f90`: `subroutine mtname`.
// NJOY2016 is under a modified BSD 3-Clause (LANL/DOE) licence, GPL-compatible; this derivative file is
// distributed under AGPL-3.0-only. This is a modified, non-LANL version, not endorsed by LANL/DOE.
// See crate root LICENSE.njoy + NOTICE.

//! Axis scaling (`ascle`, `ascll`) and the reaction names (`mtname`).
//!
//! Single-precision literals matter here: `ascll`'s `0.1*amin` is the
//! `real(4)` 0.1 widened to double (0.10000000149...), and the `1000.`/`10.`
//! constants of `ascle` are exact. They are written as `f32` casts below.

use super::fmt::AceTable;
use super::mtname_table::HNDF;
use petir::mathf::RealMath;

/// `ascle(m,z1,z2,major,minor)` (`acefc.f90:19544`): automatic scaling of a
/// linear axis. Returns `(z1, z2, major, minor)`.
pub fn ascle(m: i32, z1: f64, z2: f64) -> (f64, f64, i64, i64) {
    let mut zmin = z1;
    let mut zmax = z2;
    if zmax <= zmin || m <= 0 || m > 20 {
        return (0.0, 2.0 * z2, 1, 0);
    }
    let fm = f64::from(m);
    if zmax == 0.0 || zmin == 0.0 {
        zmax -= zmax.abs() / 1_000_000.0;
        zmin += zmin.abs() / 1_000_000.0;
    } else {
        let mut zbar = zmax / zmin;
        if zbar.abs() / 1000.0 >= 1.0 {
            zmin = 0.0;
        }
        if zbar.abs() * 1000.0 <= 1.0 {
            zmax = 0.0;
            zmax -= zmax.abs() / 1_000_000.0;
            zmin += zmin.abs() / 1_000_000.0;
        } else if ((zbar - 1.0).abs() - 5.0 * fm / 100_000.0) > 0.0 {
            zmax -= zmax.abs() / 1_000_000.0;
            zmin += zmin.abs() / 1_000_000.0;
        } else {
            zbar = (zmax + zmin) / 2.0;
            let z = 26.0 * fm * zbar.abs() / 1_000_000.0;
            zmax = zbar + z;
            zmin = zbar - z;
        }
    }
    let mut p = (zmax - zmin) / fm;
    let mut iflag = 0;
    let mut tenk = 1.0f64;
    if p < 1.0 {
        iflag = 1;
        p = 1.0 / p;
    }
    while p >= 10000.0 {
        p /= 10000.0;
        tenk *= 10000.0;
    }
    while p >= 10.0 {
        p /= 10.0;
        tenk *= 10.0;
    }
    if iflag != 0 {
        p = 10.0 / p;
        tenk = 1.0 / 10.0 / tenk;
    }
    let nm;
    let mut test = 2.0 + 1.0 / 100.0;
    if p <= test {
        test = 2.0 - 1.0 / 100.0;
        if p <= test {
            p = 1.0;
            nm = 5;
        } else {
            p = 2.0;
            nm = 4;
        }
    } else {
        test = 5.0 - 1.0 / 100.0;
        if p < test {
            p = 2.0;
            nm = 4;
        } else {
            p = 5.0;
            nm = 5;
        }
    }
    let dz = p * tenk;
    let mut n1 = (zmin / dz) as i64;
    let mut z = (n1 as f64) * dz;
    if z > zmin {
        z -= dz;
        n1 -= 1;
    }
    let z1o = z;
    let mut n2 = (zmax / dz) as i64;
    z = (n2 as f64) * dz;
    if z < zmax {
        n2 += 1;
        z += dz;
    }
    let major = n2 - n1;
    (z1o, z, major, nm * major)
}

/// `ascll(amin,amax)` (`acefc.f90:19687`): adjust the limits of a log axis.
/// Returns `(amin, amax)`.
pub fn ascll(amin: f64, amax: f64) -> (f64, f64) {
    let b1 = 0.0f64;
    let b2 = 0.301f64;
    let b3 = 0.699f64;
    let b4 = 1.0f64;
    let ohoh1 = 0.001f64;
    let ten = 10.0f64;
    let top = amax.r_log10();
    let mut itop = top as i64;
    if top < 0.0 {
        itop -= 1;
    }
    let fi = itop as f64;
    let mut amax2 = fi + 1.0;
    if top <= fi + b3 + ohoh1 {
        amax2 = fi + b3;
    }
    if top <= fi + b2 + ohoh1 {
        amax2 = fi + b2;
    }
    if top <= fi + b1 + ohoh1 {
        amax2 = fi + b1;
    }
    let amax2 = ten.r_powf(amax2);
    let bot = amin.r_log10();
    let mut ibot = bot as i64;
    if bot < 0.0 {
        ibot -= 1;
    }
    let fb = ibot as f64;
    let mut amin2 = fb;
    if bot >= fb + b2 - ohoh1 {
        amin2 = fb + b2;
    }
    if bot >= fb + b3 - ohoh1 {
        amin2 = fb + b3;
    }
    if bot >= fb + b4 - ohoh1 {
        amin2 = fb + b4;
    }
    let mut amin2 = ten.r_powf(amin2);
    let mut amax2 = amax2;
    if amin2 == amax2 {
        // `0.1*amin`: the default-real literal 0.1.
        amin2 *= f64::from(0.1f32);
        amax2 *= 10.0;
    }
    (amin2, amax2)
}

/// `mtname(mt,name,izai)` (`acecm.f90:21`): the 10-character reaction name.
pub fn mtname(mt: i64, izai: i64, iverf: i32) -> String {
    let hn = |i: i64| -> String {
        if (1..=500).contains(&i) {
            HNDF[(i - 1) as usize].to_string()
        } else {
            " ".repeat(10)
        }
    };
    let mut name = " ".repeat(10);
    if iverf >= 6 || iverf == -1 {
        if (201..=207).contains(&mt) {
            const H9: [&str; 7] =
                ["(n,xn)    ", "(n,xgma)  ", "(n,xp)    ", "(n,xd)    ", "(n,xt)    ", "(n,xhe3)  ", "(n,xa)    "];
            name = H9[(mt - 201) as usize].to_string();
        } else if mt == 444 {
            name = "damage    ".to_string();
        } else {
            let mut i = mt;
            if i > 999 {
                i -= 1000 * (i / 1000);
            }
            if i >= 600 {
                i -= 399;
            }
            name = hn(i);
        }
    } else if mt <= 200 {
        name = hn(mt);
    } else if (201..=207).contains(&mt) {
        const H9: [&str; 7] =
            ["(n,xn)    ", "(n,xgma)  ", "(n,xp)    ", "(n,xd)    ", "(n,xt)    ", "(n,xhe3)  ", "(n,xa)    "];
        name = H9[(mt - 201) as usize].to_string();
    } else if mt == 444 {
        name = "damage    ".to_string();
    } else if (700..718).contains(&mt) {
        name = hn(mt - 499);
    } else if mt == 718 {
        name = hn(250);
    } else if mt == 719 {
        name = "(n,p*c)x  ".to_string();
    } else if (720..738).contains(&mt) {
        name = hn(mt - 469);
    } else if mt == 738 {
        name = hn(300);
    } else if mt == 739 {
        name = "(n,d*c)x  ".to_string();
    } else if (740..758).contains(&mt) {
        name = hn(mt - 439);
    } else if mt == 758 {
        name = hn(350);
    } else if mt == 759 {
        name = "(n,t*c)x  ".to_string();
    } else if (760..779).contains(&mt) {
        name = hn(mt - 409);
    } else if mt == 778 {
        name = hn(400);
    } else if mt == 779 {
        name = "(n,he3*c)x".to_string();
    } else if (780..798).contains(&mt) {
        name = hn(mt - 379);
    } else if mt == 798 {
        name = hn(450);
    } else if mt == 799 {
        name = "(n,a*c)x  ".to_string();
    }
    if izai > 1 {
        if mt == 4 {
            name = "(z,n)    ".to_string();
            name.push(' ');
        } else if mt == 5 {
            name = "(z,x)    ".to_string();
            name.push(' ');
        } else if (izai == 1001 && mt == 103)
            || (izai == 1002 && mt == 104)
            || (izai == 1003 && mt == 105)
            || (izai == 2003 && mt == 106)
            || (izai == 2004 && mt == 107)
        {
            name = hn(4);
        }
    }
    name
}

/// The `name(2:2)` rewrite that follows every `mtname` call in the plot
/// routines: a name starting `(` gets the incident particle's letter.
pub fn charged_name(name: &str, izai: i64) -> String {
    let mut n: Vec<char> = name.chars().collect();
    if n.first() == Some(&'(') {
        let c = match izai {
            1 => Some('n'),
            1001 => Some('p'),
            1002 => Some('d'),
            1003 => Some('t'),
            2003 => Some('s'),
            2004 => Some('a'),
            _ => None,
        };
        if let Some(c) = c {
            n[1] = c;
        }
    }
    n.into_iter().collect()
}

/// `mtname` then the `name(2:2)` rewrite, as the plot routines call them.
pub fn plot_name(a: &AceTable, mt: i64) -> String {
    charged_name(&mtname(mt, a.izai, a.iverf), a.izai)
}
