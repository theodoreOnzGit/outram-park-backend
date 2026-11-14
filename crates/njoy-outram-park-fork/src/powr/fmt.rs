// SPDX-License-Identifier: AGPL-3.0-only
// The gfortran edit descriptors of NJOY2016.80 `src/powr.f90` (git commit
// ac5adf5f33d893e42f2eed7fb286b0d51c7580da) `lib = 1` and `lib = 2` writes.
// NJOY2016 is under a modified BSD 3-Clause (LANL/DOE) licence, GPL-compatible;
// this derivative file is distributed under AGPL-3.0-only. This is a modified,
// non-LANL version, not endorsed by LANL/DOE. See crate root LICENSE.njoy + NOTICE.

//! The edit descriptors POWR writes its libraries with, on top of the crate's
//! one copy of `Ew.d` and `Fw.d` ([`crate::acer::fortran_fmt`]).

use crate::acer::fortran_fmt::{fixed, fortran_e};

/// `1pE12.5`. A negative zero keeps its sign, as gfortran writes it.
pub fn e12_5(x: f64) -> String {
    if x == 0.0 && x.is_sign_negative() {
        return "-0.00000E+00".to_string();
    }
    fortran_e(x, 5, 12)
}

/// `(1p,6e12.5)` of `v`: six to a line; an empty list is one empty record.
pub fn six_e12_5(out: &mut Vec<String>, v: &[f64]) {
    if v.is_empty() {
        out.push(String::new());
        return;
    }
    for c in v.chunks(6) {
        out.push(c.iter().map(|&x| e12_5(x)).collect());
    }
}

/// `Fw.d`, with gfortran's `w` asterisks when the number does not fit.
pub fn f(x: f64, w: usize, d: usize) -> String {
    let s = format!("{x:>w$.d$}");
    if s.len() > w {
        "*".repeat(w)
    } else {
        s
    }
}

/// `Iw`, with gfortran's `w` asterisks when the number does not fit.
pub fn i(n: i32, w: usize) -> String {
    let s = format!("{n:>w$}");
    if s.len() > w {
        "*".repeat(w)
    } else {
        s
    }
}

/// `(<k>i<w>)` of `v`: `k` to a line; an empty list is one empty record.
pub fn ints(out: &mut Vec<String>, v: &[i32], k: usize, w: usize) {
    if v.is_empty() {
        out.push(String::new());
        return;
    }
    for c in v.chunks(k) {
        out.push(c.iter().map(|&n| i(n, w)).collect());
    }
}

/// A character variable of `n` characters, as `read(*,*)` left it: the text
/// cut or blank-padded to `n`.
pub fn chars(s: &str, n: usize) -> String {
    fixed(s, n)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn descriptors_write_as_gfortran() {
        assert_eq!(e12_5(0.0), " 0.00000E+00");
        assert_eq!(e12_5(-0.0), "-0.00000E+00");
        assert_eq!(e12_5(6.059271e-4), " 6.05927E-04");
        assert_eq!(f(236.0, 10, 4), "  236.0000");
        assert_eq!(f(1.0e7, 10, 4), "**********");
        assert_eq!(i(942380, 6), "942380");
        assert_eq!(i(1_000_000, 6), "******");
        let mut v = Vec::new();
        six_e12_5(&mut v, &[]);
        ints(&mut v, &[1, 2, 3], 2, 6);
        assert_eq!(v, vec!["".to_string(), "     1     2".into(), "     3".into()]);
        assert_eq!(chars("ab", 4), "ab  ");
    }
}
