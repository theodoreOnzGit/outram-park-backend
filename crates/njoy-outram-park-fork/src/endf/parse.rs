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
    let mf  = s[70..72].trim().parse::<i32>().unwrap_or(0);
    let mt  = s[72..75].trim().parse::<i32>().unwrap_or(0);

    Ok(RawLine { fields, mat, mf, mt })
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
        let line = " 2.004000+3 3.968219+0         -1          0          0          0 228 1451    1";
        let rl = parse_line(line).unwrap();
        assert_eq!(rl.mat, 228);
        assert_eq!(rl.mf, 1);
        assert_eq!(rl.mt, 451);
        assert!((rl.fields[0] - 2004.0).abs() < 0.001, "{}", rl.fields[0]);
        assert!((rl.fields[1] - 3.968219).abs() < 1e-5, "{}", rl.fields[1]);
        assert_eq!(rl.fields[2] as i32, -1);  // L1 = -1
        assert_eq!(rl.fields[3] as i32,  0);
    }
}
