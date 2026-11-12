// SPDX-License-Identifier: AGPL-3.0-only
// Translated from the Fortran edit descriptors used by NJOY2016.80 `graph.f90`
// and `viewr.f90` (`Fw.d`, `Iw`, `O3.3`).
// NJOY2016 is under a modified BSD 3-Clause (LANL/DOE) licence, GPL-compatible;
// this derivative file is distributed under AGPL-3.0-only. This is a modified,
// non-LANL version, not endorsed by LANL/DOE. See crate root LICENSE.njoy + NOTICE.

//! The three Fortran edit descriptors the PostScript writer needs, as gfortran
//! 13 produces them.
//!
//! - `Fw.d`: right-justified in `w` columns, `d` decimals, correctly rounded
//!   (gfortran and Rust both print the exact decimal expansion, ties to even).
//!   A negative value that rounds to zero keeps its sign (`-0.00`, which the
//!   reference tapes show). When the field is too narrow the leading `0` of
//!   `0.xx` is dropped, and if it is still too narrow the field is filled with
//!   `*`.
//! - `Iw`: right-justified integer, `*` fill on overflow.
//! - `O3.3`: octal with at least three digits.

/// `Fw.d` of `x`.
pub fn f(x: f64, w: usize, d: usize) -> String {
    let mut s = format!("{x:.d$}");
    if s.len() > w {
        if let Some(rest) = s.strip_prefix("0.") {
            s = format!(".{rest}");
        } else if let Some(rest) = s.strip_prefix("-0.") {
            s = format!("-.{rest}");
        }
    }
    if s.len() > w {
        return "*".repeat(w);
    }
    format!("{s:>w$}")
}

/// `Iw` of `n`.
pub fn i(n: i64, w: usize) -> String {
    let s = n.to_string();
    if s.len() > w {
        return "*".repeat(w);
    }
    format!("{s:>w$}")
}

/// `O3.3` of `n` (non-negative).
pub fn o3(n: u32) -> String {
    format!("{n:03o}")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fixed_point_fields() {
        assert_eq!(f(576.0, 9, 2), "   576.00");
        assert_eq!(f(0.0, 6, 3), " 0.000");
        assert_eq!(f(-0.001, 7, 2), "  -0.00");
        assert_eq!(f(1.0, 6, 3), " 1.000");
        assert_eq!(f(0.5, 4, 3), ".500");
        assert_eq!(f(-0.5, 5, 3), "-.500");
        assert_eq!(f(12345.678, 5, 1), "*****");
        assert_eq!(f(0.25, 5, 1), "  0.2");
    }

    #[test]
    fn integer_and_octal_fields() {
        assert_eq!(i(7, 4), "   7");
        assert_eq!(i(123456, 5), "*****");
        assert_eq!(i(-12, 5), "  -12");
        assert_eq!(o3(0o101), "101");
        assert_eq!(o3(5), "005");
    }
}
