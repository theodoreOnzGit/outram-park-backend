// SPDX-License-Identifier: AGPL-3.0-only
// Translated from NJOY2016.80 `src/covr.f90` (git commit ac5adf5f33d893e42f2eed7fb286b0d51c7580da):
//   - `subroutine smilab`, l.1649-1708; `subroutine matmes`, l.1710-1757;
//   - `subroutine elem`, l.1759-1789; `subroutine mtno`, l.1791-1890.
// NJOY2016 is under a modified BSD 3-Clause (LANL/DOE) licence, GPL-compatible;
// this derivative file is distributed under AGPL-3.0-only. This is a modified,
// non-LANL version, not endorsed by LANL/DOE. See crate root LICENSE.njoy + NOTICE.

//! The labels of COVR's plots (gh:#1369): `smilab`, `matmes`, `elem` and `mtno`.
//!
//! The strings are VIEWR's text markup (`<x>` upper case, `#EH<...#HXEX`
//! superscripts, `]` Greek), built exactly as upstream builds them, including
//! its `character(n)` truncation and blank padding.

use crate::acer::fortran_fmt::fortran_e;

/// What `smilab` needs besides its arguments: the module variables it reads.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct LabelContext {
    /// `mf35`: 3 (cross sections) or 5 (spectra).
    pub mf35: i32,
    /// `mfflg` of the input tape (`-11`, `-12`, `-14`).
    pub mfflg: i32,
    /// `einc`: `C2` of the last MF=5 LIST read (eV).
    pub einc: f64,
    /// `iverf`: `L1` of the material's MF=1 head.
    pub iverf: i32,
}

/// The element symbols of `elem` (`covr.f90:1770-1782`).
const ENAME: [&str; 103] = [
    "<h> ", "<h>e", "<l>i", "<b>e", "<b> ", "<c> ", "<n> ", "<o> ", "<f> ", "<n>e", "<n>a", "<m>g", "<a>l", "<s>i",
    "<p> ", "<s> ", "<c>l", "<a>r", "<k> ", "<c>a", "<s>c", "<t>i", "<v> ", "<c>r", "<m>n", "<f>e", "<c>o", "<n>i",
    "<c>u", "<z>n", "<g>a", "<g>e", "<a>s", "<s>e", "<b>r", "<k>r", "<r>b", "<s>r", "<y> ", "<z>r", "<n>b", "<m>o",
    "<t>c", "<r>u", "<r>h", "<p>d", "<a>g", "<c>d", "<i>n", "<s>n", "<s>b", "<t>e", "<i> ", "<x>e", "<c>s", "<b>a",
    "<l>a", "<c>e", "<p>r", "<n>d", "<p>m", "<s>m", "<e>u", "<g>d", "<t>b", "<d>y", "<h>o", "<e>r", "<t>m", "<y>b",
    "<l>u", "<h>f", "<t>a", "<w> ", "<r>e", "<o>s", "<i>r", "<p>t", "<a>u", "<h>g", "<t>l", "<p>b", "<b>i", "<p>o",
    "<a>t", "<r>n", "<f>r", "<r>a", "<a>c", "<t>h", "<p>a", "<u> ", "<n>p", "<p>u", "<a>m", "<c>m", "<b>k", "<c>f",
    "<e>s", "<f>m", "<m>d", "<n>o", "<l>w",
];

/// `mtno`'s reaction table (`covr.f90:1806-1818`): MT, name length, name.
const IRA1: [i32; 33] = [
    1, 2, 3, 4, 16, 17, 18, 22, 28, 37, 51, 91, 102, 103, 104, 105, 106, 107, 111, 207, 780, 781, 452, 455, 456, 25, 24,
    32, 33, 41, 112, 115, 116,
];
const IRA2: [usize; 33] = [5, 4, 7, 6, 3, 3, 2, 5, 3, 3, 1, 6, 4, 2, 2, 2, 6, 4, 3, 5, 5, 5, 5, 5, 5, 6, 6, 3, 3, 4, 5, 3, 3];
const HIRA: [&str; 33] = [
    "tot.)   ", "el.)    ", "nonel.) ", "inel.)  ", "2n)     ", "3n)     ", "f)      ", "n]a<)   ", "np)     ",
    "4n)     ", ")       ", "cont.)  ", "]g<)    ", "p)      ", "d)      ", "t)      ", "<h>e3)  ", "]a<)    ",
    "2p      ", "<h>e)   ", "]a<0)   ", "]a<1)   ", " ]n<)   ", " ]n<)   ", " ]n<)   ", "3n]a<)  ", "2n]a<)  ",
    "nd)     ", "nt)     ", "2np)    ", "p]a<)   ", "pd)     ", "pt)     ",
];

/// The first `n` characters of `s` blank-padded to `n` (Fortran `s(1:n)` of a
/// `character` variable at least `n` long).
fn head(s: &str, n: usize) -> String {
    format!("{s:<n$.n$}")
}

/// `elem` (`covr.f90:1759-1789`): the symbol of element `iz` and its length
/// (3 when its fourth character is blank).
pub fn elem(iz: i32) -> (String, usize) {
    let name = ENAME.get((iz - 1).max(0) as usize).copied().unwrap_or("    ");
    let len = if name.ends_with(' ') { 3 } else { 4 };
    (name.to_string(), len)
}

/// `mtno` (`covr.f90:1791-1890`): `(lnamel, inamel, ivl, lnamer, inamer)`, the left
/// name, the discrete-level number and the right name of reaction `mt`.
pub fn mtno(mt: i32, iverf: i32) -> (String, usize, i32, String, usize) {
    let in_range = |lo: i32, hi: i32| mt >= lo && mt < hi && iverf > 5;
    let (mut lnamel, mut inamel) = if mt > 50 && mt < 92 {
        ("(n,n    ".to_string(), 4)
    } else if in_range(600, 650) {
        ("(n,p    ".to_string(), 4)
    } else if in_range(650, 700) {
        ("(n,d    ".to_string(), 4)
    } else if in_range(700, 750) {
        ("(n,t    ".to_string(), 4)
    } else if in_range(750, 800) {
        ("(n,h    ".to_string(), 4)
    } else if in_range(800, 850) {
        ("(n,a    ".to_string(), 4)
    } else if mt == 452 {
        ("(total  ".to_string(), 6)
    } else if mt == 455 {
        ("(delayed".to_string(), 8)
    } else if mt == 456 {
        ("(prompt ".to_string(), 7)
    } else if mt == 251 {
        ("(mt     ".to_string(), 3)
    } else if mt == 261 {
        ("(spectr.".to_string(), 8)
    } else {
        ("(n,     ".to_string(), 3)
    };
    let mut ivl = -1;
    if mt > 50 && mt < 91 {
        ivl = mt - 50;
    }
    for (lo, hi) in [(600, 650), (650, 700), (700, 750), (750, 800), (800, 850)] {
        if in_range(lo, hi) {
            ivl = mt - lo;
        }
    }
    let mut jloc = 11;
    let mut idone = false;
    let mut j = 0;
    while j < 33 && !idone {
        j += 1;
        jloc = j;
        if IRA1[j - 1] == mt || ((0..=99).contains(&ivl) && IRA1[j - 1] == 51) {
            idone = true;
        }
    }
    if !idone {
        // `write(nmeh1(4:6),'(i3)') mt`
        lnamel = format!("(mt{mt:>3}  ");
        inamel = 6;
        jloc = 11;
    }
    (lnamel, inamel, ivl, HIRA[jloc - 1].to_string(), IRA2[jloc - 1])
}

/// `matmes` (`covr.f90:1710-1757`): the reaction name in nuclear-physics notation
/// and its length.
pub fn matmes(iza: i32, mt: i32, iverf: i32) -> (String, usize) {
    let iz = iza / 1000;
    let ia = iza - iz * 1000;
    let (lname, iname) = elem(iz);
    let (liso, niso) = if ia > 0 && ia < 10 {
        (format!("#EH<{ia:1}#HXEX"), 10)
    } else if (10..100).contains(&ia) {
        (format!("#EH<{ia:2}#HXEX"), 11)
    } else if ia >= 100 {
        (format!("#EH<{ia:3}#HXEX"), 12)
    } else {
        (String::new(), 0)
    };
    let (lmat, nmat) = if ia == 0 {
        (head(&lname, iname), iname)
    } else {
        (format!("{}{}", head(&liso, niso), head(&lname, iname)), niso + iname)
    };
    let (lnamel, inamel, ivl, lnamer, inamer) = mtno(mt, iverf);
    let (lnam, nnam) = if ivl <= 0 {
        (lnamer, inamer)
    } else if ivl < 10 {
        (format!("#L.25H.75<{ivl:1}#HXLX<)"), 18)
    } else {
        (format!("#L.25H.75<{ivl:2}#HXLX<)"), 19)
    };
    let s = format!("{}{}{}", head(&lmat, nmat), head(&lnamel, inamel), head(&lnam, nnam));
    (s, nmat + inamel + nnam)
}

/// `smilab` (`covr.f90:1649-1708`): the label of a semi-log plot, as the
/// `character(80)` string `plotit` writes (blank-padded or cut to 80).
pub fn smilab(iza: i32, mt: i32, izap: i32, mtflg: i32, ctx: &LabelContext) -> String {
    let nu = (452..=456).contains(&mt);
    let (str1, l1) = if nu && mtflg == 0 {
        ("[d]n/n", 6)
    } else if nu {
        ("    ]n", 6)
    } else if mt == 251 && mtflg != 0 {
        ("    ]m", 6)
    } else if mtflg != 0 && ctx.mf35 == 5 {
        ("Grp-average ]f", 14)
    } else if mtflg == 0 && ctx.mf35 == 5 {
        ("[d]f/f", 6)
    } else if mtflg == 0 && mt == 251 {
        ("[d]m/m", 6)
    } else if mtflg != 0 {
        ("    ]s", 6)
    } else {
        ("[d]s/s", 6)
    };
    let (str2, l2) = if mtflg != 0 && ctx.mf35 == 5 && ctx.einc > 9.999e4 {
        (format!(">(<e>#LH>in#HXLX>={} <m>e<v>), ", crate::viewr::fmt::f(ctx.einc / 1.0e6, 5, 2)), 34)
    } else if mtflg != 0 && ctx.mf35 == 5 {
        (format!(">(<e>#LH>in#HXLX>= {} e<v>), ", fortran_e(ctx.einc, 2, 9)), 35)
    } else {
        ("> vs. <e> for ".to_string(), 14)
    };
    let (str3, lstr3) = matmes(iza, mt, ctx.iverf);
    let mut s = format!("{}{}{}", head(str1, l1), head(&str2, l2), head(&str3, lstr3));
    if ctx.mfflg == -14 && izap == 0 {
        s.push_str(", MF40");
    } else if ctx.mfflg == -14 {
        s.push_str(&format!(", izap = {izap:>7}"));
    }
    head(&s, 80)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The labels of NJOY2016's deck 34 plot file (`tests/34/referenceTape33`, lines 3 and 59).
    #[test]
    fn deck_34_labels() {
        let ctx = LabelContext { mf35: 5, mfflg: -12, einc: 2.5e5, iverf: 6 };
        assert_eq!(
            smilab(94240, 18, 0, 1, &ctx).trim_end(),
            "Grp-average ]f>(<e>#LH>in#HXLX>= 0.25 <m>e<v>), #EH<240#HXEX<p>u(n,f)"
        );
        assert_eq!(smilab(94240, 18, 0, 0, &ctx).trim_end(), "[d]f/f> vs. <e> for #EH<240#HXEX<p>u(n,f)");
        assert_eq!(smilab(94240, 18, 0, 0, &ctx).len(), 80);
    }

    /// Discrete levels, an MT outside the table, and a low `einc`.
    #[test]
    fn levels_and_unknown_mts() {
        assert_eq!(matmes(26056, 52, 6).0, "#EH<56#HXEX<f>e(n,n#L.25H.75<2#HXLX<)");
        assert_eq!(matmes(1001, 102, 6).0, "#EH<1#HXEX<h>(n,]g<)");
        assert_eq!(matmes(92000, 300, 6).0, "<u>(mt300)");
        let ctx = LabelContext { mf35: 5, mfflg: -12, einc: 2.53e-2, iverf: 6 };
        let s = smilab(98252, 18, 0, 1, &ctx);
        // `l2 = 35` of a 36-character `str2` drops its trailing blank, as upstream does
        assert!(s.starts_with("Grp-average ]f>(<e>#LH>in#HXLX>=  2.53E-02 e<v>),#EH<252#HXEX<c>f(n,f)"), "{s}");
        let ctx40 = LabelContext { mf35: 3, mfflg: -14, einc: 0.0, iverf: 6 };
        assert!(smilab(92235, 102, 0, 0, &ctx40).trim_end().ends_with(", MF40"));
    }
}
