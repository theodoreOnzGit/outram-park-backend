//! Serialise an [`AceTable`] to a **Type-1 ASCII** ACE file.
//!
//! Ports `aceout` (the header) and `change` (the XSS data block) from NJOY2016
//! `acefc.f90` for the `itype == 1`, `mcnpx == 0` case. The on-disk layout is:
//!
//! ```text
//! line 1:  ZAID(a10) AWR(f12.6) ' ' kT(1pe11.4) ' ' date(a10)
//! line 2:  comment(a70) mat-id(a10)
//! 4 lines: 16 × (IZ(i7) AW(f11.0))           — 4 pairs per line
//! 6 lines: NXS(1..16) then JXS(1..32)        — 8 integers (i9) per line
//! N lines: XSS data                          — 4 values per line, 20-char fields
//! ```
//!
//! Each XSS word is written as an integer (`i20`) or a real (`1pE20.11`) per its
//! [`AceTable::xss_is_int`] flag, four to a line — the line breaks are positional
//! (every fourth value), exactly as NJOY's `typen` buffers them.

use std::io::{self, Write};
use std::path::Path;

use super::AceTable;

impl AceTable {
    /// Write this table to `path` as a Type-1 ASCII ACE file.
    ///
    /// # Errors
    /// Propagates any [`std::io::Error`] from creating or writing the file.
    pub fn write_type1<P: AsRef<Path>>(&self, path: P) -> io::Result<()> {
        let mut f = io::BufWriter::new(std::fs::File::create(path)?);
        self.write_to(&mut f)
    }

    /// Write this table in Type-1 ASCII to any [`Write`] sink.
    ///
    /// Exposed (rather than only [`write_type1`][Self::write_type1]) so callers
    /// can serialise to an in-memory buffer — used by the round-trip tests.
    pub fn write_to<W: Write>(&self, w: &mut W) -> io::Result<()> {
        // ── Header ──────────────────────────────────────────────────────────
        writeln!(
            w,
            "{:<10}{} {} {:<10}",
            fixed10(&self.zaid),
            fortran_f(self.awr, 12, 6),
            fortran_e(self.kt_mev, 4, 11),
            fixed10(&self.date),
        )?;
        writeln!(w, "{:<70}{:<10}", fixed(&self.comment, 70), fixed10(&self.mat_id))?;

        // ── 16 (IZ, AW) pairs, 4 per line (all zero for a plain neutron table) ─
        for row in 0..4 {
            let mut line = String::new();
            for col in 0..4 {
                let _ = row * 4 + col;
                line.push_str(&format!("{:7}{}", 0, fortran_f0(0.0)));
            }
            writeln!(w, "{line}")?;
        }

        // ── NXS (16) then JXS (32): 48 integers, 8 per line (i9) ────────────
        let ints: Vec<i32> = self.nxs.iter().chain(self.jxs.iter()).copied().collect();
        for chunk in ints.chunks(8) {
            let mut line = String::new();
            for &v in chunk {
                line.push_str(&format!("{v:9}"));
            }
            writeln!(w, "{line}")?;
        }

        // ── XSS data: 4 values per line, 20-char fields ─────────────────────
        let mut col = 0;
        let mut line = String::new();
        for (i, &v) in self.xss.iter().enumerate() {
            if self.xss_is_int[i] {
                line.push_str(&format!("{:20}", v.round() as i64));
            } else {
                line.push_str(&fortran_e(v, 11, 20));
            }
            col += 1;
            if col == 4 {
                writeln!(w, "{line}")?;
                line.clear();
                col = 0;
            }
        }
        if col > 0 {
            writeln!(w, "{line}")?; // flush the partial final line
        }
        Ok(())
    }
}

// ── Fortran-style formatting helpers ───────────────────────────────────────────

/// Truncate or pad `s` to exactly `n` characters (left-justified).
fn fixed(s: &str, n: usize) -> String {
    let mut t: String = s.chars().take(n).collect();
    while t.chars().count() < n {
        t.push(' ');
    }
    t
}

/// Exactly 10 characters, left-justified (header string fields).
fn fixed10(s: &str) -> String {
    fixed(s, 10)
}

/// Fortran `Fw.d` fixed-point format: width `w`, `d` decimals, right-justified.
fn fortran_f(x: f64, w: usize, d: usize) -> String {
    format!("{x:>w$.d$}")
}

/// Fortran `F11.0` format: integer-valued with a trailing decimal point, e.g.
/// `0.0` → `"         0."`. Rust's `{:.0}` drops the point, so add it back.
fn fortran_f0(x: f64) -> String {
    let body = format!("{}.", x.round() as i64);
    format!("{body:>11}")
}

/// Fortran `1pEw.d` scientific format: one digit before the point, `decimals`
/// after, a signed two-digit exponent, right-justified to `width`.
///
/// Example: `fortran_e(92235.0, 11, 20)` → `"  9.22350000000E+04"`.
fn fortran_e(x: f64, decimals: usize, width: usize) -> String {
    if x == 0.0 || !x.is_finite() {
        let mant = if decimals == 0 {
            "0".to_string()
        } else {
            format!("0.{}", "0".repeat(decimals))
        };
        return format!("{:>width$}", format!(" {mant}E+00"));
    }
    let sign = if x < 0.0 { '-' } else { ' ' };
    let s = format!("{:.*E}", decimals, x.abs());
    let (mant, exp) = s.split_once('E').unwrap();
    let exp: i32 = exp.parse().unwrap();
    let exp_sign = if exp < 0 { '-' } else { '+' };
    let body = format!("{sign}{mant}E{exp_sign}{:02}", exp.abs());
    format!("{body:>width$}")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn e_format_width_and_roundtrip() {
        // 1pE20.11 of a positive number: width 20, 11 mantissa digits, signed exp.
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

    #[test]
    fn e11_4_temperature_field() {
        // kT field is 1pE11.4 (width 11).
        let s = fortran_e(2.53e-8, 4, 11);
        assert_eq!(s.len(), 11, "{s:?}");
        assert!((s.trim().parse::<f64>().unwrap() - 2.53e-8).abs() < 1e-11);
    }

    #[test]
    fn f0_has_trailing_point() {
        assert_eq!(fortran_f0(0.0), "         0.");
        assert_eq!(fortran_f0(0.0).len(), 11);
    }
}
