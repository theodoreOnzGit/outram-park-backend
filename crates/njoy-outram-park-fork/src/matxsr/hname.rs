// Translated from NJOY2016.80 (commit ac5adf5) `src/matxsr.f90`, `subroutine hname`
// (`:1328-1536`).
// NJOY2016 is under a modified BSD 3-Clause (LANL/DOE) licence, GPL-compatible;
// this derivative file is distributed under AGPL-3.0-only. This is a modified,
// non-LANL version, not endorsed by LANL/DOE. See crate root LICENSE.njoy + NOTICE.

//! `hname`: the six-character MATXS name of a reaction (returned as the
//! `character(8)` it is stored in, two trailing blanks).
//!
//! Upstream sets `iverf = 6` on entry (`:1404`), whatever the data, so an
//! ENDF-4 photo-electric MT=602 is named as an ENDF-6 proton level, `p02`, and
//! MT=621 as `p21` (deck 03).

const I1: [i32; 33] = [2, 3, 4, 5, 6, 7, 8, 9, 16, 17, 18, 19, 20, 21, 22, 23, 24, 25, 26, 28, 29, 30, 32, 33, 34, 35, 36, 37, 38, 41, 42, 44, 45];
const H1: [&str; 33] = [
    "elas ", "nonel", "inel ", "x    ", "2n1  ", "2n2  ", "2n3  ", "2n4  ", "2n   ", "3n   ", "ftot ", "f    ", "nf   ", "2nf  ", "na   ",
    "n3a  ", "2na  ", "3na  ", "2ni  ", "np   ", "n2a  ", "2n2a ", "nd   ", "nt   ", "nh   ", "nd2a ", "nt2a ", "4n   ", "3nf  ", "2np  ",
    "3np  ", "n2p  ", "npa  ",
];
const I2: [i32; 16] = [101, 102, 103, 104, 105, 106, 107, 108, 109, 111, 112, 113, 114, 115, 116, 117];
const H2: [&str; 16] = ["abs  ", "g    ", "p    ", "d    ", "t    ", "h    ", "a    ", "2a   ", "h3a  ", "2p   ", "pa   ", "t2a  ", "d2a  ", "pd   ", "pt   ", "da   "];
const H3: [&str; 7] = [".neut", ".gam ", ".h1  ", ".h2  ", ".h3  ", ".he3 ", ".he4 "];
const H4: [&str; 26] = [
    "free  ", "hh2o  ", "poly  ", "poly$ ", "hzrh  ", "hzrh$ ", "benz  ", "dd2o  ", "graph ", "graph$", "be    ", "be$   ", "bebeo ",
    "bebeo$", "zrzrh ", "zrzrh$", "obeo  ", "obeo$ ", "ouo2  ", "ouo2$ ", "uuo2  ", "uuo2$ ", "al    ", "al$   ", "fe    ", "fe$   ",
];
const I5: [i32; 10] = [251, 252, 253, 259, 301, 443, 444, 452, 455, 456];
const H5: [&str; 10] = ["mubar ", "xi    ", "gamma ", "invel ", "heat  ", "kerma ", "dame  ", "nutot ", "nudel ", "nupmt "];
const I6: [i32; 10] = [500, 502, 504, 515, 516, 517, 522, 525, 602, 621];
const H6: [&str; 10] = ["stop  ", "gcoh  ", "ginch ", "gpaire", "gpair ", "gpairn", "gabs  ", "gheat ", "gabs  ", "gheat "];
const ILR: [i32; 19] = [31, 16, 17, 22, 23, 24, 25, 26, 28, 29, 30, 32, 33, 34, 35, 36, 37, 39, 40];
const HLR: [&str; 19] = ["g  ", "n  ", "nn ", "a  ", "aaa", "na ", "nna", "ni ", "p  ", "aa ", "naa", "d  ", "t  ", "h  ", "daa", "taa", "nnn", "ic ", "ee "];

/// Fortran `i<w>` (`*` fill on overflow).
fn iw(n: i32, w: usize) -> String {
    crate::viewr::fmt::i(i64::from(n), w)
}

/// `character(6)` assignment: pad or cut to six.
fn six(s: &str) -> String {
    format!("{:<6.6}", s)
}

/// `hname(hreact, hp, mt, lr, izam)`: `hp` is the one-letter particle.
pub fn hname(hp: char, mt: i32, lr: i32, izam: i32) -> String {
    const IVERF: i32 = 6;
    let strng: String;
    let mut level = false;
    if izam > 0 {
        // label 340.
        let ii = izam / 10;
        let mm = izam - 10 * ii;
        let jj = ii + 100 * mm;
        strng = if mt == 102 && jj >= 10000 {
            format!("c{}", iw(jj, 5))
        } else if mt == 102 {
            format!("c0{}", iw(jj, 4))
        } else if jj >= 10000 {
            format!("r{}", iw(jj, 5))
        } else {
            format!("r0{}", iw(jj, 4))
        };
        return format!("{}  ", six(&strng));
    }
    let mtname = || six(&format!("mt{}", iw(mt, 3)));
    let s: String = if mt <= 49 {
        match I1.iter().position(|&m| m == mt) {
            Some(j) => six(&format!("{hp}{}", H1[j])),
            None => mtname(),
        }
    } else if mt <= 91 {
        level = true;
        if mt == 91 {
            six("ncn")
        } else if mt < 60 {
            six(&format!("n0{}", iw(mt - 50, 1)))
        } else {
            six(&format!("n{}", iw(mt - 50, 2)))
        }
    } else if mt <= 150 {
        match I2.iter().position(|&m| m == mt) {
            Some(j) => six(&format!("{hp}{}", H2[j])),
            None => mtname(),
        }
    } else if mt <= 207 {
        if mt < 201 {
            mtname()
        } else {
            six(H3[(mt - 201) as usize])
        }
    } else if mt <= 246 {
        if mt < 221 {
            mtname()
        } else {
            six(H4[(mt - 221) as usize])
        }
    } else if mt <= 499 {
        match I5.iter().position(|&m| m == mt) {
            Some(j) => six(H5[j]),
            None => mtname(),
        }
    } else if IVERF >= 6 && mt > 599 {
        // label 290, ENDF-6 particle levels.
        if mt > 891 {
            mtname()
        } else {
            level = true;
            let w = |p: &str, k: i32| six(&format!("{p}0{}", iw(k, 1)));
            let w2 = |p: &str, k: i32| six(&format!("{p}{}", iw(k, 2)));
            let mut st = String::from("      ");
            if (600..610).contains(&mt) {
                st = w("p", mt - 600);
            }
            if (610..649).contains(&mt) {
                st = w2("p", mt - 600);
            }
            if mt == 649 {
                st = six("pcn");
            }
            if (650..660).contains(&mt) {
                st = w("d", mt - 650);
            }
            if (660..699).contains(&mt) {
                st = w2("d", mt - 650);
            }
            if mt == 699 {
                st = six("dcn");
            }
            if (700..710).contains(&mt) {
                st = w("t", mt - 700);
            }
            if (710..749).contains(&mt) {
                st = w2("t", mt - 700);
            }
            if mt == 749 {
                st = six("tcd");
            }
            if (750..760).contains(&mt) {
                st = w("h", mt - 750);
            }
            if (760..799).contains(&mt) {
                st = w2("h", mt - 750);
            }
            if mt == 799 {
                st = six("hcn");
            }
            if (800..810).contains(&mt) {
                st = w("a", mt - 800);
            }
            if (810..849).contains(&mt) {
                st = w2("a", mt - 800);
            }
            if mt == 849 {
                st = six("acn");
            }
            if (875..885).contains(&mt) {
                st = six(&format!("2n0{}", iw(mt - 875, 1)));
            }
            if (885..891).contains(&mt) {
                st = six(&format!("2n{}", iw(mt - 875, 2)));
            }
            if mt == 891 {
                st = six("2ncn");
            }
            st
        }
    } else {
        match I6.iter().position(|&m| m == mt) {
            Some(j) => six(H6[j]),
            None => mtname(),
        }
    };
    // label 310: the `lr` flag.
    let mut out = s;
    if level && lr != 0 {
        if let Some(j) = ILR.iter().position(|&l| l == lr) {
            let temp: String = out.chars().take(3).collect();
            out = six(&format!("{temp}{}", HLR[j]));
        }
    }
    format!("{out}  ")
}

#[cfg(test)]
mod tests {
    use super::hname;

    /// Names NJOY2016.80 writes on decks 03 and 30 (units 35 and 44).
    #[test]
    fn names_as_upstream() {
        assert_eq!(hname('g', 602, 0, 0), "p02     ");
        assert_eq!(hname('g', 621, 0, 0), "p21     ");
        assert_eq!(hname('g', 502, 0, 0), "gcoh    ");
        assert_eq!(hname('n', 2, 0, 0), "nelas   ");
        assert_eq!(hname('n', 251, 0, 0), "mubar   ");
        assert_eq!(hname('n', 102, 0, 0), "ng      ");
        assert_eq!(hname('n', 52, 0, 0), "n02     ");
        assert_eq!(hname('n', 1000, 0, 0), "mt***   ");
    }
}
