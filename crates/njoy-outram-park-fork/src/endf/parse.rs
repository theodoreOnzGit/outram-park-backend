//! Low-level ENDF line and float parsing.
//!
//! Ported from the read path of `contio`/`lineio`/`a11` in NJOY2016 `endf.f90`.

use crate::NjoyError;

/// Parse one ENDF 11-column float field.
///
/// ENDF floats look like `+1.234567+6` or ` 0.000000+0`.  The exponent sign
/// (`+` / `-`) immediately follows the mantissa — there is no `E`.  Blank
/// fields (or all-space strings) are interpreted as `0.0` (Fortran `e11.0`).
///
/// Hollerith (text) fields in MF=1/MT=451 descriptive records are also silently
/// mapped to `0.0`, matching Fortran's `e11.0` edit descriptor behaviour when
/// character data is read into a real variable.
///
/// # Examples
///
/// ```
/// use njoy_outram_park_fork::endf::parse::parse_endf_float;
/// assert!((parse_endf_float("+1.234567+6").unwrap() - 1.234567e6).abs() < 1e-3);
/// assert_eq!(parse_endf_float("           ").unwrap(), 0.0);
/// assert_eq!(parse_endf_float(" 0.000000+0").unwrap(), 0.0);
/// // Hollerith text in MF=1 MT=451 lines → 0.0 (not an error)
/// assert_eq!(parse_endf_float("  2-He-  4 ").unwrap(), 0.0);
/// ```
pub fn parse_endf_float(s: &str) -> Result<f64, NjoyError> {
    let s = s.trim();
    if s.is_empty() {
        return Ok(0.0);
    }

    // Find the exponent separator: a `+` or `-` that is not the leading sign
    // of the mantissa (i.e. occurs after position 0).
    let bytes = s.as_bytes();
    let exp_start = bytes[1..]
        .iter()
        .position(|&b| b == b'+' || b == b'-')
        .map(|p| p + 1); // offset back to position in full string

    let result = match exp_start {
        None => {
            // No exponent separator — plain integer-like field (e.g. "0") or text
            s.parse::<f64>().ok()
        }
        Some(sep) => {
            let mantissa = &s[..sep];
            let exponent = &s[sep..]; // includes the `+`/`-`
            let mant: Option<f64> = if mantissa.is_empty() || mantissa == "+" || mantissa == "-" {
                Some(1.0_f64.copysign(if mantissa.starts_with('-') { -1.0 } else { 1.0 }))
            } else {
                mantissa.parse::<f64>().ok()
            };
            let exp: Option<i32> = exponent.parse::<i32>().ok();
            match (mant, exp) {
                (Some(m), Some(e)) => Some(m * 10_f64.powi(e)),
                _ => None,
            }
        }
    };

    // Hollerith / non-numeric content: silently return 0.0, matching Fortran e11.0
    Ok(result.unwrap_or(0.0))
}

/// Format one ENDF 11-column float field — the write-side inverse of
/// [`parse_endf_float`].
///
/// Ported from `subroutine a11` in NJOY2016 `endf.f90:882-981`. Produces the
/// familiar `+1.234567+6`-style form (7 significant figures, explicit signed
/// exponent) for most values. For **positive** values with `0.1 < x <
/// 0.999999999e7`, NJOY instead tries an extended 9-significant-figure plain
/// decimal form (`f11.8` down to `f11.2` with an implicit Fortran `kP` scale
/// factor, e.g. `123456.789` or `0.99916730`), then falls back to the compact
/// exponential form if that plain write leaves trailing zeros suggesting a
/// more compact encoding was available (`endf.f90:976-979`, ported verbatim
/// including the character-position checks).
///
/// Note this only reproduces `a11`'s mantissa/exponent field; it does not
/// implement the separate plain-integer path NJOY uses for CONT-record
/// control fields (L1/L2/N1/N2) — see `Tape::write` for that caveat.
///
/// # Examples
///
/// ```
/// use njoy_outram_park_fork::endf::parse::format_endf_float;
/// assert_eq!(format_endf_float(0.0), " 0.000000+0");
/// assert_eq!(format_endf_float(2004.0), " 2.004000+3");
/// assert_eq!(format_endf_float(0.9991673), " 9.991673-1");
/// ```
pub fn format_endf_float(x: f64) -> String {
    // endf.f90:911-916 — zero is a special case.
    if x == 0.0 {
        return format_mantissa_exp(0.0, '+', 0);
    }

    // endf.f90:917 — `if (x.gt.tenth.and.x.lt.tmil) go to 130`
    const TENTH: f64 = 0.1;
    const TMIL: f64 = 0.999999999e7;
    if x > TENTH && x < TMIL {
        format_a11_extended(x)
    } else {
        format_a11_normal(x)
    }
}

/// The "normal" seven/six/five-significant-figure branch, `endf.f90:917-953`
/// (labels 100–160, skipping the extended-form entry at 130).
fn format_a11_normal(x: f64) -> String {
    const ONE: f64 = 1.0;
    const TEN: f64 = 10.0;
    const UP: f64 = 0.000000001;
    const TOP7: f64 = 9.9999995;
    const TOP6: f64 = 9.999995;
    const TOP5: f64 = 9.99995;

    // endf.f90:921 — `n=int(log10(abs(xx)))`. Fortran `int()` truncates
    // toward zero; `as i32` on f64 does the same, so this matches exactly.
    let xx = x;
    let n0 = xx.abs().log10() as i32;

    let (f, s, n);
    if xx.abs() < ONE {
        // endf.f90:933-944 (label 110)
        let mut nn = 1 - n0;
        let mut ff = xx * TEN.powi(nn);
        if within_a11_bound(nn, ff, TOP7, TOP6, TOP5) {
            s = '-';
        } else {
            ff = ff / TEN + UP;
            nn -= 1;
            s = if nn > 0 { '-' } else { '+' };
        }
        f = ff;
        n = nn;
    } else {
        // endf.f90:922-932 (fallthrough from label 100)
        let mut nn = n0;
        let mut ff = xx / TEN.powi(nn);
        if !within_a11_bound(nn, ff, TOP7, TOP6, TOP5) {
            ff = ff / TEN + UP;
            nn += 1;
        }
        f = ff;
        n = nn;
        s = '+';
    }
    format_mantissa_exp(f, s, n)
}

/// `endf.f90:925-928` / `936-939` — the three range-keyed bound checks
/// (mutually exclusive on `|n|`, so equivalent to a single threshold lookup).
fn within_a11_bound(n: i32, ff: f64, top7: f64, top6: f64, top5: f64) -> bool {
    let an = n.abs();
    if an < 10 {
        ff.abs() < top7
    } else if an < 100 {
        ff.abs() < top6
    } else {
        ff.abs() < top5
    }
}

/// `endf.f90:946-952` (label 160) — assemble `f9.6`/`f8.5`/`f7.4` mantissa +
/// signed 1/2/3-digit exponent, chosen by `|n|` magnitude.
fn format_mantissa_exp(f: f64, s: char, n: i32) -> String {
    let an = n.abs();
    let mantissa = if an < 10 {
        format!("{:9.6}", f)
    } else if an < 100 {
        format!("{:8.5}", f)
    } else {
        format!("{:7.4}", f)
    };
    format!("{mantissa}{s}{an}")
}

/// The extended nine-significant-figure branch for `0.1 < x <
/// 0.999999999e7`, `endf.f90:955-980` (labels 130–160 plus the post-hoc
/// fallback rewrites). Only reached for positive `x` (guaranteed by the
/// `x.gt.tenth` guard in [`format_endf_float`]).
fn format_a11_extended(x: f64) -> String {
    const ONEM: f64 = 0.999999999;
    const TOP9: f64 = 9.999999995;
    const BOT9: f64 = 9.99999995;
    const TEN: f64 = 10.0;
    const TENTH: f64 = 0.1;

    // endf.f90:956 — `n=int(log10(abs(x)))`
    let n0 = x.abs().log10() as i32;

    let (f, s, mut n);
    if x.abs() < ONEM {
        // endf.f90:965-967 (label 140)
        f = x;
        s = '-';
        n = -1;
    } else {
        // endf.f90:958-964
        let mut ff = x / TEN.powi(n0);
        let mut nn = n0;
        let bound = if nn.abs() < 10 { TOP9 } else { BOT9 };
        if ff.abs() >= bound {
            ff /= TEN;
            nn += 1;
        }
        f = ff;
        s = '+';
        n = nn;
    }

    // endf.f90:968-975 (label 150) — plain decimal write with an implicit
    // Fortran `kP` scale factor (k=n for n=1..6): displayed = f * 10^max(n,0),
    // decimal digits = 8 - max(n,0), field width always 11.
    let scale = n.max(0);
    let decimals = (8 - scale) as usize;
    let displayed = f * TEN.powi(scale);
    let mut hx = format!("{:11.decimals$}", displayed);

    // endf.f90:976 — relabel the exponent before the fallback checks below.
    if n == -1 {
        n = 1;
    }

    // endf.f90:977 — `if (f.gt.onem.and.hx(10:11).eq.'00') ...`
    // 1-indexed columns 10-11 == 0-indexed hx[9..11].
    if f > ONEM && hx.len() >= 11 && &hx[9..11] == "00" {
        hx = format_mantissa_exp(f, s, n);
    }
    // endf.f90:978-979 — `if (f.gt.tenth.and.f.lt.onem.and.hx(11:11).eq.'0')
    // write(hx,'(1p,f9.6,a,i1)') f,s,n` — `1p` scale factor: display f*10.
    if f > TENTH && f < ONEM && hx.len() >= 11 && &hx[10..11] == "0" {
        hx = format_mantissa_exp(f * TEN, s, n);
    }

    hx
}

/// Format one ENDF data line (six fields + MAT/MF/MT + sequence number) —
/// the write-side counterpart of [`parse_line`].
///
/// All six fields are written via [`format_endf_float`]. This is a faithful
/// port of the *data value* encoding NJOY's `lineio` uses (`endf.f90:838-880`,
/// which formats all six `ah(1..6)` fields through `a11` unconditionally).
///
/// **Known simplification (documented, not hidden):** upstream NJOY's
/// `contio` (`endf.f90:37-111`) writes **CONT**-record lines differently: only
/// the first two fields (C1, C2) go through `a11`; the last four (L1, L2, N1,
/// N2) are integer control fields written as plain right-justified integers
/// (`i11`), not the exponential `a11` form. This crate's in-memory
/// [`crate::endf::tape::Section`] stores every row as a uniform `[f64; 6]`
/// with no per-row CONT-vs-LIST tag, so `format_line` cannot reproduce that
/// distinction — every row (CONT or LIST) is written with all six fields in
/// `a11` form. The **value** written is numerically identical (e.g. an
/// integer control field `5` becomes `5.000000+0`, which parses back to
/// exactly `5.0`), so round-tripping through this crate's own reader is exact;
/// the column layout of CONT rows simply does not byte-match genuine upstream
/// NJOY output for those four integer fields.
pub fn format_line(fields: &[f64; 6], mat: i32, mf: i32, mt: i32, seq: i32) -> String {
    let mut line = String::with_capacity(80);
    for f in fields {
        line.push_str(&format_endf_float(*f));
    }
    // Columns 67-70 MAT (i4), 71-72 MF (i2), 73-75 MT (i3), 76-80 seq (i5) —
    // mirrors the column layout documented for parse_line's inverse.
    line.push_str(&format!("{mat:4}{mf:2}{mt:3}{seq:5}"));
    line
}

/// One parsed 80-character ENDF line.
#[derive(Debug, Clone)]
pub struct RawLine {
    /// The six 11-column data fields. Integer-valued fields are stored as
    /// `f64` (Fortran stores them as `real` then `nint`-converts at call sites).
    pub fields: [f64; 6],
    /// Material number.
    pub mat: i32,
    /// File number.
    pub mf: i32,
    /// Section number.
    pub mt: i32,
}

/// Parse one 80-character ENDF ASCII line.
///
/// Columns 1–66 are six 11-char float fields.
/// Columns 67–70 are `MAT` (right-justified integer).
/// Columns 71–72 are `MF`.
/// Columns 73–75 are `MT`.
/// Columns 76–80 are the sequence number (ignored).
pub fn parse_line(line: &str) -> Result<RawLine, NjoyError> {
    // Pad to 80 chars so slicing is always in-bounds
    let mut buf = [b' '; 80];
    let bytes = line.as_bytes();
    let copy_len = bytes.len().min(80);
    buf[..copy_len].copy_from_slice(&bytes[..copy_len]);
    let s = std::str::from_utf8(&buf).unwrap();

    let mut fields = [0.0f64; 6];
    for i in 0..6 {
        fields[i] = parse_endf_float(&s[i * 11..(i + 1) * 11])?;
    }

    let mat = s[66..70].trim().parse::<i32>().unwrap_or(0);
    let mf = s[70..72].trim().parse::<i32>().unwrap_or(0);
    let mt = s[72..75].trim().parse::<i32>().unwrap_or(0);

    Ok(RawLine {
        fields,
        mat,
        mf,
        mt,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn endf_float_zero() {
        assert_eq!(parse_endf_float(" 0.000000+0").unwrap(), 0.0);
        assert_eq!(parse_endf_float("           ").unwrap(), 0.0);
    }

    #[test]
    fn endf_float_positive() {
        let v = parse_endf_float(" 2.004000+3").unwrap();
        assert!((v - 2004.0).abs() < 1e-6, "got {}", v);
    }

    #[test]
    fn endf_float_negative_exponent() {
        let v = parse_endf_float(" 9.991673-1").unwrap();
        assert!((v - 0.9991673).abs() < 1e-9, "got {}", v);
    }

    #[test]
    fn endf_float_large() {
        let v = parse_endf_float(" 2.000000+7").unwrap();
        assert!((v - 2.0e7).abs() < 1.0, "got {}", v);
    }

    #[test]
    fn parse_cont_line() {
        // From He-4: " 2.004000+3 3.968219+0         -1          0          0          0 228 1451"
        let line =
            " 2.004000+3 3.968219+0         -1          0          0          0 228 1451    1";
        let rl = parse_line(line).unwrap();
        assert_eq!(rl.mat, 228);
        assert_eq!(rl.mf, 1);
        assert_eq!(rl.mt, 451);
        assert!((rl.fields[0] - 2004.0).abs() < 0.001, "{}", rl.fields[0]);
        assert!((rl.fields[1] - 3.968219).abs() < 1e-5, "{}", rl.fields[1]);
        assert_eq!(rl.fields[2] as i32, -1); // L1 = -1
        assert_eq!(rl.fields[3] as i32, 0);
    }
}
