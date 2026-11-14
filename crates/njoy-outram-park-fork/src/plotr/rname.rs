// SPDX-License-Identifier: AGPL-3.0-only
// Translated from NJOY2016.80 `src/plotr.f90` (git commit ac5adf5f33d893e42f2eed7fb286b0d51c7580da):
//   `subroutine rname`, l.2970-3146.
// NJOY2016 is under a modified BSD 3-Clause (LANL/DOE) licence, GPL-compatible;
// this derivative file is distributed under AGPL-3.0-only. This is a modified,
// non-LANL version, not endorsed by LANL/DOE. See crate root LICENSE.njoy + NOTICE.

//! `rname`: the reaction name PLOTR puts in the second title of an automatic
//! (`mtd = 0`) plot. The table and both branches are upstream's, quirks
//! included: the `hndf9` names are declared and never used, so for
//! `iverf >= 6` MT=201-207 index the thermal names; MT=221-236 start at entry
//! 408, so MT=229-236 fall past the table's end (refused); for `iverf < 6`,
//! MT=443 is `damage` and MT=444 `kerma` (`:3131-3134`).
//! Inside a double-quoted Fortran string `''` is two apostrophes, so
//! `"(n,n''f)  "` is ten characters as written.

/// `hndf` (`plotr.f90:2980-3064`), 415 names of 10 characters.
#[rustfmt::skip]
const HNDF: [&str; 415] = [
    "total     ", "elastic   ", "nonelastic", "inelastic ", "(n,x)     ",
    "(n,2n_1f) ", "(n,2n_2f) ", "(n,2n_3f) ", "(n,2n_4f) ", "(n,x)     ",
    "(n,2nd)   ", "(n,x)     ", "(n,x)     ", "(n,x)     ", "(n,x)     ",
    "(n,2n)    ", "(n,3n)    ", "fission   ", "(n,f)     ", "(n,n''f)  ",
    "(n,2nf)   ", "(n,n''a)  ", "(n,n'')3a ", "(n,2n)a   ", "(n,3n)a   ",
    "(n,2n)iso ", "(n,abs)   ", "(n,n''p)  ", "(n,n''a)  ", "(n,2n)2a  ",
    "(n,x)     ", "(n,n''d)  ", "(n,n''t)  ", "(n,n''he3)", "(n,n'')d2a",
    "(n,n'')t2a", "(n,4n)    ", "(n,3nf)   ", "(n,x)     ", "(n,x)     ",
    "(n,2np)   ", "(n,3np)   ", "(n,n2p)   ", "(n,npa)   ", "(n,x)     ",
    "(n,2n_1s) ", "(n,2n_2s) ", "(n,2n_3s) ", "(n,2n_4s) ", "(n,x)     ",
    "(n,n_1)   ", "(n,n_2)   ", "(n,n_3)   ", "(n,n_4)   ", "(n,n_5)   ",
    "(n,n_6)   ", "(n,n_7)   ", "(n,n_8)   ", "(n,n_9)   ", "(n,n_10)  ",
    "(n,n_11)  ", "(n,n_12)  ", "(n,n_13)  ", "(n,n_14)  ", "(n,n_15)  ",
    "(n,n_16)  ", "(n,n_17)  ", "(n,n_18)  ", "(n,n_19)  ", "(n,n_20)  ",
    "(n,n_21)  ", "(n,n_22)  ", "(n,n_23)  ", "(n,n_24)  ", "(n,n_25)  ",
    "(n,n_26)  ", "(n,n_27)  ", "(n,n_28)  ", "(n,n_29)  ", "(n,n_30)  ",
    "(n,n_31)  ", "(n,n_32)  ", "(n,n_33)  ", "(n,n_34)  ", "(n,n_35)  ",
    "(n,n_36)  ", "(n,n_37)  ", "(n,n_38)  ", "(n,n_39)  ", "(n,n_40)  ",
    "(n,n_c)   ", "(n,x)     ", "(n,x)     ", "(n,x)     ", "(n,x)     ",
    "(n,x)     ", "(n,x)     ", "(n,x)     ", "(n,n'')g  ", "(n,x)     ",
    "(n,parab) ", "(n,g)     ", "(n,p)     ", "(n,d)     ", "(n,t)     ",
    "(n,he3)   ", "(n,a)     ", "(n,2a)    ", "(n,3a)    ", "(n,x)     ",
    "(n,2p)    ", "(n,pa)    ", "(n,t2a)   ", "(n,d2a)   ", "(n,pd)    ",
    "(n,pt)    ", "(n,da)    ", "(n,x)     ", "(n,x)     ", "(n,dest)  ",
    "(n,x)     ", "(n,x)     ", "(n,x)     ", "(n,x)     ", "(n,x)     ",
    "(n,x)     ", "(n,x)     ", "(n,x)     ", "(n,x)     ", "(n,x)     ",
    "(n,x)     ", "(n,x)     ", "(n,x)     ", "(n,x)     ", "(n,x)     ",
    "(n,x)     ", "(n,x)     ", "(n,x)     ", "(n,x)     ", "(n,x)     ",
    "(n,x)     ", "(n,x)     ", "(n,x)     ", "(n,x)     ", "(n,x)     ",
    "(n,x)     ", "(n,x)     ", "(n,x)     ", "(n,x)     ", "(n,p_0)   ",
    "(n,p_1)   ", "(n,p_2)   ", "(n,p_3)   ", "(n,p_4)   ", "(n,p_5)   ",
    "(n,p_6)   ", "(n,p_7)   ", "(n,p_8)   ", "(n,p_9)   ", "(n,p_10)  ",
    "(n,p_11)  ", "(n,p_12)  ", "(n,p_13)  ", "(n,p_14)  ", "(n,p_15)  ",
    "(n,p_16)  ", "(n,p_17)  ", "(n,p_18)  ", "(n,p_19)  ", "(n,p_20)  ",
    "(n,p_21)  ", "(n,p_22)  ", "(n,p_23)  ", "(n,p_24)  ", "(n,p_25)  ",
    "(n,p_26)  ", "(n,p_27)  ", "(n,p_28)  ", "(n,p_29)  ", "(n,p_30)  ",
    "(n,p_31)  ", "(n,p_32)  ", "(n,p_33)  ", "(n,p_34)  ", "(n,p_35)  ",
    "(n,p_36)  ", "(n,p_37)  ", "(n,p_38)  ", "(n,p_39)  ", "(n,p_40)  ",
    "(n,p_41)  ", "(n,p_42)  ", "(n,p_43)  ", "(n,p_44)  ", "(n,p_45)  ",
    "(n,p_46)  ", "(n,p_47)  ", "(n,p_48)  ", "(n,p_c)   ", "(n,d_0)   ",
    "(n,d_1)   ", "(n,d_2)   ", "(n,d_3)   ", "(n,d_4)   ", "(n,d_5)   ",
    "(n,d_6)   ", "(n,d_7)   ", "(n,d_8)   ", "(n,d_9)   ", "(n,d_10)  ",
    "(n,d_11)  ", "(n,d_12)  ", "(n,d_13)  ", "(n,d_14)  ", "(n,d_15)  ",
    "(n,d_16)  ", "(n,d_17)  ", "(n,d_18)  ", "(n,d_19)  ", "(n,d_20)  ",
    "(n,d_21)  ", "(n,d_22)  ", "(n,d_23)  ", "(n,d_24)  ", "(n,d_25)  ",
    "(n,d_26)  ", "(n,d_27)  ", "(n,d_28)  ", "(n,d_29)  ", "(n,d_30)  ",
    "(n,d_31)  ", "(n,d_32)  ", "(n,d_33)  ", "(n,d_34)  ", "(n,d_35)  ",
    "(n,d_36)  ", "(n,d_37)  ", "(n,d_38)  ", "(n,d_39)  ", "(n,d_40)  ",
    "(n,d_41)  ", "(n,d_42)  ", "(n,d_43)  ", "(n,d_44)  ", "(n,d_45)  ",
    "(n,d_46)  ", "(n,d_47)  ", "(n,d_48)  ", "(n,d_c)   ", "(n,t_0)   ",
    "(n,t_1)   ", "(n,t_2)   ", "(n,t_3)   ", "(n,t_4)   ", "(n,t_5)   ",
    "(n,t_6)   ", "(n,t_7)   ", "(n,t_8)   ", "(n,t_9)   ", "(n,t_10)  ",
    "(n,t_11)  ", "(n,t_12)  ", "(n,t_13)  ", "(n,t_14)  ", "(n,t_15)  ",
    "(n,t_16)  ", "(n,t_17)  ", "(n,t_18)  ", "(n,t_19)  ", "(n,t_20)  ",
    "(n,t_21)  ", "(n,t_22)  ", "(n,t_23)  ", "(n,t_24)  ", "(n,t_25)  ",
    "(n,t_26)  ", "(n,t_27)  ", "(n,t_28)  ", "(n,t_29)  ", "(n,t_30)  ",
    "(n,t_31)  ", "(n,t_32)  ", "(n,t_33)  ", "(n,t_34)  ", "(n,t_35)  ",
    "(n,t_36)  ", "(n,t_37)  ", "(n,t_38)  ", "(n,t_39)  ", "(n,t_40)  ",
    "(n,t_41)  ", "(n,t_42)  ", "(n,t_43)  ", "(n,t_44)  ", "(n,t_45)  ",
    "(n,t_46)  ", "(n,t_47)  ", "(n,t_48)  ", "(n,t_c)   ", "(n,he3_0) ",
    "(n,he3_1) ", "(n,he3_2) ", "(n,he3_3) ", "(n,he3_4) ", "(n,he3_5) ",
    "(n,he3_6) ", "(n,he3_7) ", "(n,he3_8) ", "(n,he3_9) ", "(n,he3_10)",
    "(n,he3_11)", "(n,he3_12)", "(n,he3_13)", "(n,he3_14)", "(n,he3_15)",
    "(n,he3_16)", "(n,he3_17)", "(n,he3_18)", "(n,he3_19)", "(n,he3_20)",
    "(n,he3_21)", "(n,he3_22)", "(n,he3_23)", "(n,he3_24)", "(n,he3_25)",
    "(n,he3_26)", "(n,he3_27)", "(n,he3_28)", "(n,he3_29)", "(n,he3_30)",
    "(n,he3_31)", "(n,he3_32)", "(n,he3_33)", "(n,he3_34)", "(n,he3_35)",
    "(n,he3_36)", "(n,he3_37)", "(n,he3_38)", "(n,he3_39)", "(n,he3_40)",
    "(n,he3_41)", "(n,he3_42)", "(n,he3_43)", "(n,he3_44)", "(n,he3_45)",
    "(n,he3_46)", "(n,he3_47)", "(n,he3_48)", "(n,he3_c) ", "(n,a_0)   ",
    "(n,a_1)   ", "(n,a_2)   ", "(n,a_3)   ", "(n,a_4)   ", "(n,a_5)   ",
    "(n,a_6)   ", "(n,a_7)   ", "(n,a_8)   ", "(n,a_9)   ", "(n,a_10)  ",
    "(n,a_11)  ", "(n,a_12)  ", "(n,a_13)  ", "(n,a_14)  ", "(n,a_15)  ",
    "(n,a_16)  ", "(n,a_17)  ", "(n,a_18)  ", "(n,a_19)  ", "(n,a_20)  ",
    "(n,a_21)  ", "(n,a_22)  ", "(n,a_23)  ", "(n,a_24)  ", "(n,a_25)  ",
    "(n,a_26)  ", "(n,a_27)  ", "(n,a_28)  ", "(n,a_29)  ", "(n,a_30)  ",
    "(n,a_31)  ", "(n,a_32)  ", "(n,a_33)  ", "(n,a_34)  ", "(n,a_35)  ",
    "(n,a_36)  ", "(n,a_37)  ", "(n,a_38)  ", "(n,a_39)  ", "(n,a_40)  ",
    "(n,a_41)  ", "(n,a_42)  ", "(n,a_43)  ", "(n,a_44)  ", "(n,a_45)  ",
    "(n,a_46)  ", "(n,a_47)  ", "(n,a_48)  ", "(n,a_c)   ", "free gas  ",
    "H(H2O)    ", "poly      ", "          ", "H(ZrH) inc", "H(ZrH) coh",
    "benzine   ", "D(D2O)    ", "graph inc ", "graph coh ", "Be inc    ",
    "Be coh    ", "BeO inc   ", "BeO coh   ", "Zr(ZrH)inc", "Zr(ZrH)coh",
];

/// `rname(mt,name)` under the endf module's `iverf`.
///
/// # Errors
/// [`crate::NjoyError::NotPorted`] for an MT whose table index falls outside
/// `hndf` (upstream reads outside the array: undefined).
pub fn rname(mt: i32, iverf: i32) -> Result<String, crate::NjoyError> {
    let at = |i: i32| -> Result<String, crate::NjoyError> {
        usize::try_from(i - 1)
            .ok()
            .and_then(|k| HNDF.get(k))
            .map(|s| (*s).to_string())
            .ok_or(crate::NjoyError::NotPorted(
                "plotr rname: an MT outside the name table (undefined upstream)",
            ))
    };
    if iverf >= 6 {
        let mut i = mt;
        if (201..=207).contains(&i) {
            i += 200;
        }
        if i >= 600 {
            i -= 450;
        }
        // `name=hndf(i)` comes first upstream and is then overwritten for these
        // MTs, so its value (out of the table for 251-444) never matters.
        let name = match mt {
            221..=236 => at(408 + mt - 221)?,
            251 => "mubar     ".into(),
            252 => "xi        ".into(),
            253 => "gamma     ".into(),
            301 => "heating   ".into(),
            443 => "kerma     ".into(),
            444 => "damage    ".into(),
            _ => at(i)?,
        };
        return Ok(name);
    }
    let name = if mt < 150 {
        at(mt)?
    } else if (201..=207).contains(&mt) {
        at(mt + 200)?
    } else if (221..=236).contains(&mt) {
        at(408 + mt - 221)?
    } else if (700..718).contains(&mt) {
        at(mt - 550)?
    } else if mt == 718 {
        at(199)?
    } else if mt == 719 {
        "(n,p_c)x  ".into()
    } else if (720..738).contains(&mt) {
        at(mt - 520)?
    } else if mt == 738 {
        at(249)?
    } else if mt == 739 {
        "(n,d_c)x  ".into()
    } else if (740..758).contains(&mt) {
        at(mt - 490)?
    } else if mt == 758 {
        at(299)?
    } else if mt == 759 {
        "(n,t_c)x  ".into()
    } else if (760..779).contains(&mt) {
        at(mt - 460)?
    } else if mt == 778 {
        at(349)?
    } else if mt == 779 {
        "(n,he3_c)x".into()
    } else if (780..798).contains(&mt) {
        at(mt - 430)?
    } else if mt == 798 {
        at(399)?
    } else if mt == 799 {
        "(n,a_c)x  ".into()
    } else if mt == 301 {
        "heating   ".into()
    } else if mt == 443 {
        "damage    ".into()
    } else if mt == 444 {
        "kerma     ".into()
    } else if mt == 251 {
        "mubar     ".into()
    } else if mt == 252 {
        "xi        ".into()
    } else if mt == 253 {
        "gamma     ".into()
    } else {
        "unknown   ".into()
    };
    Ok(name)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn names_follow_both_branches() {
        assert_eq!(rname(2, 6).unwrap(), "elastic   ");
        assert_eq!(rname(102, 6).unwrap(), "(n,g)     ");
        // `i = 203 + 200` lands on a blank entry of the thermal names: upstream's.
        assert_eq!(rname(203, 6).unwrap(), "          ");
        assert_eq!(rname(600, 6).unwrap(), "(n,p_0)   ");
        assert_eq!(rname(221, 6).unwrap(), "graph inc ");
        assert_eq!(rname(443, 6).unwrap(), "kerma     ");
        assert_eq!(rname(443, 5).unwrap(), "damage    ");
        assert_eq!(rname(718, 5).unwrap(), "(n,p_c)   ");
        assert_eq!(rname(160, 5).unwrap(), "unknown   ");
        assert_eq!(rname(22, 5).unwrap(), "(n,n''a)  ");
        assert!(rname(0, 6).is_err());
        // MT=229-236 index past the 415 names (`408 + mt - 221`): undefined upstream.
        assert!(rname(230, 6).is_err());
        assert!(rname(500, 6).is_err());
    }
}
