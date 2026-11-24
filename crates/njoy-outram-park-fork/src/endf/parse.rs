//! Low-level ENDF line and float parsing.
//!
//! Ported from the read path of `contio`/`lineio`/`a11` in NJOY2016 `endf.f90`.

#[cfg(not(any(feature = "std", test)))]
#[allow(unused_imports)]
use crate::nostd_prelude::*;
use petir::mathf::RealMath;
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
    // Hollerith / non-numeric content: silently return 0.0. Right for text
    // records; a numeric record uses [`parse_endf_float_checked`] (gh:#660).
    Ok(parse_endf_float_checked(s).unwrap_or(0.0))
}

/// [`parse_endf_float`] without the text fallback: `None` when the field is
/// not a number a Fortran `e11.0` read accepts (gh:#660).
///
/// NJOY2016 reads numeric records with `(6e11.0,i4,i2,i3,i5)` and no
/// `iostat` (`endf.f90:55`, `:852`), so gfortran stops the run on such a
/// field ("Bad value during floating point read"); measured 2026-11-24 on a
/// C-12 tape with `4.7x81670` in MF=3/MT=2. A blank field is `0.0`, and blanks
/// inside a field are ignored (`BN`), as in [`parse_endf_float`].
///
/// ```
/// use njoy_outram_park_fork::endf::parse::parse_endf_float_checked;
/// assert_eq!(parse_endf_float_checked("           "), Some(0.0));
/// assert_eq!(parse_endf_float_checked(" 4.74 8167 "), Some(4.748167));
/// assert_eq!(parse_endf_float_checked(" 1.5D3"), Some(1500.0));
/// assert_eq!(parse_endf_float_checked(" 4.7x81670 "), None);
/// assert_eq!(parse_endf_float_checked("  2-He-  4 "), None);
/// ```
#[must_use]
pub fn parse_endf_float_checked(s: &str) -> Option<f64> {
    let s = s.trim();
    // **CHANGED 2026-11-11 (gh:#1369, test 01 / ENDF/B-V tape 511):** a blank
    // inside a numeric field is ignored, as a Fortran formatted `read` does
    // (BN mode). ENDF/B-V writes `1.00000- 5` for 1e-5; the exponent `- 5`
    // used to fail to parse and the field silently became 0.0, which made
    // HEATR's photon-yield scan on C-nat loop forever.
    let compact;
    let s = if s.contains(' ') {
        compact = s.replace(' ', "");
        compact.as_str()
    } else {
        s
    };
    // gfortran reads a field holding only a sign or a point as 0 (measured
    // 2026-11-24 with gfortran 13.3: `'          -'`, `'+'`, `'.'` give 0.0, iostat 0).
    if s.is_empty() || s == "+" || s == "-" || s == "." {
        return Some(0.0);
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
            // No exponent separator — plain integer-like field (e.g. "0") or text.
            // Fortran also reads an unsigned `D` exponent (`1.5D3`), which
            // Rust's parser does not (gh:#660).
            s.parse::<f64>()
                .ok()
                .or_else(|| s.replace(['D', 'd'], "e").parse::<f64>().ok())
        }
        Some(sep) => {
            // Fortran also writes an explicit exponent letter (`1.00000E-5`,
            // `1.5D+3`); ENDF/B-VIII.0 C-12/C-13/O-16/Li-7 MF=3 use that form.
            // Without stripping it the mantissa `1.00000E` fails to parse and
            // the field silently became 0.0 (bead op-sti5).
            let mantissa = s[..sep].trim_end_matches(['E', 'e', 'D', 'd']);
            let exponent = &s[sep..]; // includes the `+`/`-`
            // ANCHOR: lesson_float_once
            // ONE correctly-rounded conversion of the whole decimal, as a
            // Fortran formatted read does.
            //
            // ~~`m * 10^e`~~ **CORRECTED 2026-09-26**: that rounds twice (the
            // mantissa, then the product), so `1.090000+7` could come out an
            // ulp off 10 900 000. It mattered: `acelf5`'s MT=18 tail
            // supplement tests `dele > 2e5` strictly, and on U-235 the
            // 10.9-11.1 MeV panels are exactly 200 keV wide in the evaluation
            // -- NJOY left them alone and this crate split them, 16 extra
            // points per incident energy and 2 568 extra DLW words.
            let mantissa = if mantissa.is_empty() || mantissa == "+" || mantissa == "-" {
                if mantissa.starts_with('-') {
                    "-1"
                } else {
                    "1"
                }
            } else {
                mantissa
            };
            match exponent.parse::<i32>() {
                Ok(e) => {
                    // The same 40 bytes, written through `core::fmt` rather
                    // than `std::io::Cursor` so it builds without `std`
                    // (gh:#1592); the text written, and so the parse, is
                    // unchanged.
                    let mut w = FixedBuf { buf: [0u8; 40], len: 0 };
                    use core::fmt::Write;
                    if write!(w, "{mantissa}e{e}").is_ok() {
                        let n = w.len;
                        core::str::from_utf8(&w.buf[..n])
                            .ok()
                            .and_then(|t| t.parse::<f64>().ok())
                    } else {
                        None
                    }
                }
                Err(_) => None,
            }
            // ANCHOR_END: lesson_float_once
        }
    };

    result
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
    let n0 = xx.abs().r_log10() as i32;

    let (f, s, n);
    if xx.abs() < ONE {
        // endf.f90:933-944 (label 110)
        let mut nn = 1 - n0;
        let mut ff = xx * TEN.r_powi(nn);
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
        let mut ff = xx / TEN.r_powi(nn);
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
    let n0 = x.abs().r_log10() as i32;

    let (f, s, mut n);
    if x.abs() < ONEM {
        // endf.f90:965-967 (label 140)
        f = x;
        s = '-';
        n = -1;
    } else {
        // endf.f90:958-964
        let mut ff = x / TEN.r_powi(n0);
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
    let displayed = f * TEN.r_powi(scale);
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
///
/// Columns are **characters**, not bytes (since 2026-11-10, gh:#660): a
/// non-ASCII character (an accented author name in MF=1/MT=451 text) fills
/// one column and is read as `?`, which no numeric field accepts, so a text
/// field still reads as `0.0` exactly as it does for ASCII text. Before, the
/// 80-byte copy and byte slicing panicked when a multibyte character
/// straddled a field boundary. ASCII lines parse exactly as before.
pub fn parse_line(line: &str) -> Result<RawLine, NjoyError> {
    parse_line_checked(line, false)
}

/// [`parse_line`], refusing a field that is not a number when `numeric` is
/// true: an [`NjoyError::EndfParse`] naming the field, where NJOY2016's
/// formatted read stops with "Bad value during floating point read"
/// (gh:#660). With `numeric` false a non-numeric field reads `0.0`, which is
/// right only for text (Hollerith) records. With `numeric` true the MAT, MF,
/// MT and sequence-number columns must be Fortran integers too (`i4,i2,i3,i5`,
/// blanks ignored); with it false they read as before, 0 when unparsable.
///
/// # Errors
///
/// [`NjoyError::EndfParse`] on a non-numeric field or identifier column when
/// `numeric` is true.
pub fn parse_line_checked(line: &str, numeric: bool) -> Result<RawLine, NjoyError> {
    let buf = columns(line);
    let s = core::str::from_utf8(&buf).map_err(|e| NjoyError::EndfParse(e.to_string()))?;

    let mut fields = [0.0f64; 6];
    for i in 0..6 {
        let field = &s[i * 11..(i + 1) * 11];
        fields[i] = match parse_endf_float_checked(field) {
            Some(v) => v,
            None if numeric => {
                return Err(NjoyError::EndfParse(format!(
                    "field {} ({field:?}) of MAT {} MF {} MT {} is not a number; \
                     NJOY2016 stops here (Fortran e11.0: bad value during floating point read)",
                    i + 1,
                    s[66..70].trim(),
                    s[70..72].trim(),
                    s[72..75].trim()
                )))
            }
            None => 0.0,
        };
    }

    let (mat, mf, mt) = if numeric {
        record_ids(s)?
    } else {
        (
            s[66..70].trim().parse::<i32>().unwrap_or(0),
            s[70..72].trim().parse::<i32>().unwrap_or(0),
            s[72..75].trim().parse::<i32>().unwrap_or(0),
        )
    };

    Ok(RawLine {
        fields,
        mat,
        mf,
        mt,
    })
}

/// A line as 80 one-byte columns: characters, not bytes (gh:#660), a
/// non-ASCII character read as `?`, padded with blanks. Every byte is ASCII,
/// so byte slices of it are on char boundaries.
fn columns(line: &str) -> [u8; 80] {
    let mut buf = [b' '; 80];
    for (slot, c) in buf.iter_mut().zip(line.chars()) {
        *slot = if c.is_ascii() { c as u8 } else { b'?' };
    }
    buf
}

/// A Fortran `iN` read of `s` (gh:#660): blanks are ignored (`BN`), an
/// all-blank field is 0, an optional sign then digits; anything else is
/// `None`, where gfortran stops ("Bad value during integer read"; measured
/// 2026-11-24: a lone sign or a letter is refused, `'1 2'` reads 12).
///
/// ```
/// use njoy_outram_park_fork::endf::parse::parse_fortran_int;
/// assert_eq!(parse_fortran_int("   "), Some(0));
/// assert_eq!(parse_fortran_int("1 2"), Some(12));
/// assert_eq!(parse_fortran_int("-59"), Some(-59));
/// assert_eq!(parse_fortran_int("  -"), None);
/// assert_eq!(parse_fortran_int("a  "), None);
/// ```
#[must_use]
pub fn parse_fortran_int(s: &str) -> Option<i64> {
    let t: String = s.chars().filter(|&c| c != ' ').collect();
    if t.is_empty() {
        return Some(0);
    }
    t.parse::<i64>().ok()
}

/// MAT, MF, MT of an 80-column line, read as NJOY2016 reads every card,
/// `(...,i4,i2,i3,i5)` (`contio`, `hollio`, `tosend`): each must be a Fortran
/// integer, and so must the sequence number in columns 76-80 (gh:#660).
fn record_ids(s: &str) -> Result<(i32, i32, i32), NjoyError> {
    let mut out = [0i32; 4];
    for (k, (name, a, b)) in [("MAT", 66, 70), ("MF", 70, 72), ("MT", 72, 75), ("sequence number", 75, 80)]
        .into_iter()
        .enumerate()
    {
        out[k] = parse_fortran_int(&s[a..b])
            .and_then(|v| i32::try_from(v).ok())
            .ok_or_else(|| {
                NjoyError::EndfParse(format!(
                    "{name} field ({:?}, columns {}-{b}) is not an integer; NJOY2016 stops here \
                     (Fortran i-format: bad value during integer read)",
                    &s[a..b],
                    a + 1
                ))
            })?;
    }
    Ok((out[0], out[1], out[2]))
}

/// Check a line's MAT, MF, MT and sequence-number columns as NJOY2016 reads
/// them on every card, a text card included (`hollio`, `tosend`:
/// `(16a4,a2,i4,i2,i3,i5)`), without reading its six data fields (gh:#660).
///
/// # Errors
///
/// [`NjoyError::EndfParse`] when one of them is not a Fortran integer.
pub fn check_record_ids(line: &str) -> Result<(), NjoyError> {
    let buf = columns(line);
    let s = core::str::from_utf8(&buf).map_err(|e| NjoyError::EndfParse(e.to_string()))?;
    record_ids(s).map(|_| ())
}

/// Check every card of a coded GENDF tape after its label (the TPID) as
/// NJOY2016's `contio`/`listio` read it, `(6e11.0,i4,i2,i3,i5)`: a GENDF has
/// no text record (its title is the TPID), so a non-numeric field or a
/// non-integer MAT/MF/MT/sequence column is refused (gh:#660). Cards of a
/// material other than `only_mat` (when given) have their identifier columns
/// checked only, as NJOY's `tosend` reads them while skipping.
///
/// # Errors
///
/// [`NjoyError::EndfParse`] naming the line.
pub fn check_gendf_text(text: &str, only_mat: Option<i32>) -> Result<(), NjoyError> {
    for (k, line) in text.lines().enumerate().skip(1) {
        let with_line = |e: NjoyError| match e {
            NjoyError::EndfParse(why) => NjoyError::EndfParse(format!("GENDF line {}: {why}", k + 1)),
            other => other,
        };
        check_record_ids(line).map_err(with_line)?;
        let rl = parse_line(line)?;
        if only_mat.is_none_or(|m| m == rl.mat) {
            parse_line_checked(line, true).map_err(with_line)?;
        }
    }
    Ok(())
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

    /// Fortran E/D-exponent forms as they appear packed in the ENDF/B-VIII.0
    /// C-12 tape (`n-006_C_012-ENDF8.0.endf` MF=3/MT=1 line 4:
    /// ` 1.00000E-5 4.94234700 1.090516E-5 …`) — these read as 0.0 before the
    /// fix (op-sti5).
    #[test]
    fn endf_float_fortran_exponent_letter() {
        assert!((parse_endf_float(" 1.00000E-5").unwrap() - 1.0e-5).abs() < 1e-17);
        assert!((parse_endf_float("1.090516E-5").unwrap() - 1.090516e-5).abs() < 1e-17);
        assert!((parse_endf_float(" .001000000").unwrap() - 1.0e-3).abs() < 1e-15);
        assert!((parse_endf_float("   1.5D+3  ").unwrap() - 1.5e3).abs() < 1e-9);
        assert!((parse_endf_float("  -2.5e-01 ").unwrap() + 0.25).abs() < 1e-15);
        assert!((parse_endf_float("  1.5E5    ").unwrap() - 1.5e5).abs() < 1e-9);
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

/// A fixed 40-byte text buffer for `parse_endf_float`'s re-assembled number:
/// `core::fmt::Write` into a stack array, so no allocation and no `std`
/// (gh:#1592). A write past the end fails, as the `Cursor` it replaces did.
struct FixedBuf {
    buf: [u8; 40],
    len: usize,
}

impl core::fmt::Write for FixedBuf {
    fn write_str(&mut self, s: &str) -> core::fmt::Result {
        let end = self.len + s.len();
        if end > self.buf.len() {
            return Err(core::fmt::Error);
        }
        self.buf[self.len..end].copy_from_slice(s.as_bytes());
        self.len = end;
        Ok(())
    }
}
