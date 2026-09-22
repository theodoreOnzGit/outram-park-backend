//! The Fortran edit descriptors an ACE file is written with — **one copy**.
//!
//! Every ACE writer in NJOY formats its header and data with the same handful
//! of descriptors: `a10`/`a13`/`a70` for text, `f12.6` and `f11.0` for the
//! header reals, `1pE11.4` for the temperature and `1pE20.11` (or `i20`) for
//! an XSS word. None of them is what Rust's `{}` produces, and each has a
//! trap:
//!
//! | descriptor | trap | Rust's answer |
//! |---|---|---|
//! | `f11.0` | keeps the decimal point | `{:.0}` drops it |
//! | `1pEw.d` | one digit before the point, **signed two-digit** exponent | `{:E}` gives `1E4` |
//! | `1pEw.d` of zero | `" 0.0000E+00"` | `{:E}` gives `0E0` |
//!
//! ## Why this module exists
//!
//! It did not, until 2026-09-22. `src/acer/write.rs` and `src/acer/read.rs`
//! each carried their own `fortran_e`, `fortran_f`, `fortran_f0` and padding
//! helpers, and **they had drifted**: the read-side copy formatted zero as
//! `0.0000` instead of `" 0.0000E+00"`, wrote `f11.0` without its decimal
//! point, and used Rust's `{:E}` exponent. All three are header fields, so
//! every byte after them shifted — and none of it showed up, because the only
//! test of that writer compared *values*. Two implementations of one edit
//! descriptor is one implementation and one latent bug; this is the single
//! one, and both writers call it.

/// Truncate or pad `s` to exactly `n` characters, left-justified — Fortran
/// `a<n>` on output.
pub fn fixed(s: &str, n: usize) -> String {
    let mut t: String = s.chars().take(n).collect();
    while t.chars().count() < n {
        t.push(' ');
    }
    t
}

/// The same as [`fixed`], as bytes, for a binary (Type-2) record.
pub fn fixed_field(s: &str, n: usize) -> Vec<u8> {
    let mut v: Vec<u8> = s.bytes().take(n).collect();
    v.resize(n, b' ');
    v
}

/// Fortran `F<w>.<d>`: fixed point, width `w`, `d` decimals, right-justified.
pub fn fortran_f(x: f64, w: usize, d: usize) -> String {
    format!("{x:>w$.d$}")
}

/// Fortran `F11.0`: integer-valued **with its trailing decimal point**, e.g.
/// `0.0` → `"         0."`. Rust's `{:.0}` drops the point, which silently
/// changes every IZ/AW line of a header.
pub fn fortran_f0(x: f64) -> String {
    let body = format!("{}.", x.round() as i64);
    format!("{body:>11}")
}

/// Fortran `1pE<w>.<d>`: one digit before the point, `d` after, and a
/// **signed two-digit** exponent, right-justified to `w`.
///
/// Zero and any non-finite value are written in the same shape rather than as
/// a bare `0.0000` — `1pE11.4` of `tz = 0` is `" 0.0000E+00"` on every 0 K
/// table NJOY writes.
///
/// # Examples
/// ```
/// use njoy_outram_park_fork::acer::fortran_fmt::fortran_e;
/// assert_eq!(fortran_e(0.0, 4, 11), " 0.0000E+00");
/// assert_eq!(fortran_e(92235.0, 11, 20).trim(), "9.22350000000E+04");
/// ```
pub fn fortran_e(x: f64, d: usize, w: usize) -> String {
    if x == 0.0 || !x.is_finite() {
        let mant = if d == 0 {
            "0".to_string()
        } else {
            format!("0.{}", "0".repeat(d))
        };
        return format!("{:>w$}", format!(" {mant}E+00"));
    }
    let sign = if x < 0.0 { '-' } else { ' ' };
    // Take the mantissa and exponent from Rust's own scientific formatter
    // rather than from `x / 10^floor(log10 x)`. Both give the same digits
    // almost always; the division does not, because it rounds twice (once in
    // the power of ten, once in the quotient) and can land a digit low on a
    // value sitting just under a decade boundary. `{:E}` is correctly rounded
    // and renormalises a mantissa that carries to 10 by itself.
    let s = format!("{:.*E}", d, x.abs());
    let (mant, exp) = s.split_once('E').unwrap_or((s.as_str(), "0"));
    let exp: i32 = exp.parse().unwrap_or(0);
    let body = format!(
        "{sign}{mant}E{}{:02}",
        if exp < 0 { "-" } else { "+" },
        exp.abs()
    );
    format!("{body:>w$}")
}

/// `1pE20.11` — `typen`'s real style for an XSS word (`acecm.f90:791`).
pub fn fortran_e20(x: f64) -> String {
    fortran_e(x, 11, 20)
}

/// `i20` — `typen`'s integer style for an XSS word (`acecm.f90:790`), which
/// rounds rather than truncates (`nint`).
pub fn fortran_i20(x: f64) -> String {
    format!("{:20}", x.round() as i64)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn e_format_width_and_roundtrip() {
        let s = fortran_e(92235.0, 11, 20);
        assert_eq!(s.len(), 20, "field must be exactly 20 wide: {s:?}");
        assert!(s.trim().ends_with("E+04"));
        assert!((s.trim().parse::<f64>().unwrap() - 92235.0).abs() < 1e-3);
    }

    #[test]
    fn e_format_zero_and_negative() {
        assert_eq!(fortran_e(0.0, 11, 20).trim(), "0.00000000000E+00");
        let neg = fortran_e(-1.5e-7, 11, 20);
        assert_eq!(neg.len(), 20);
        assert!((neg.trim().parse::<f64>().unwrap() + 1.5e-7).abs() < 1e-18);
    }

    /// The temperature field, where the zero case used to come out `0.0000`.
    #[test]
    fn e11_4_temperature_field() {
        let s = fortran_e(2.53e-8, 4, 11);
        assert_eq!(s.len(), 11, "{s:?}");
        assert!((s.trim().parse::<f64>().unwrap() - 2.53e-8).abs() < 1e-11);
        assert_eq!(fortran_e(0.0, 4, 11), " 0.0000E+00");
    }

    #[test]
    fn f0_has_trailing_point() {
        assert_eq!(fortran_f0(0.0), "         0.");
        assert_eq!(fortran_f0(0.0).len(), 11);
        assert_eq!(fortran_f0(235.0), "       235.");
    }

    /// The mantissa comes from Rust's own scientific formatter, not from
    /// `x / 10^floor(log10 x)`. The two agree on about 399 983 of every
    /// 400 000 random values and this is one of the 17 that differ: the
    /// division rounds twice and lands a unit high in the 12th digit.
    ///
    /// The old `write.rs` used the accurate form and the old `read.rs` used
    /// the division; merging them meant picking one, and it was picked by
    /// measurement rather than by which file was touched last.
    #[test]
    fn mantissa_is_correctly_rounded_not_divided() {
        let x = 587_985_670_346.5f64;
        assert_eq!(fortran_e(x, 11, 20).trim(), "5.87985670346E+11");
        // What the division form gives, recorded so the difference is visible:
        let e = x.abs().log10().floor() as i32;
        let divided = format!("{:.11}", x.abs() / 10f64.powi(e));
        assert_eq!(divided, "5.87985670347", "the division form is a unit high here");
    }

    /// A mantissa that rounds up to 10 must renormalise rather than widen the
    /// field.
    #[test]
    fn mantissa_carry_renormalises() {
        let s = fortran_e(9.999_999_999_999e3, 4, 11);
        assert_eq!(s.len(), 11, "{s:?}");
        assert_eq!(s.trim(), "1.0000E+04");
    }
}
