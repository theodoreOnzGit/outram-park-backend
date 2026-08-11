//! ENDF interpolation laws.
//!
//! Ported from `terp1` and `terpa` in NJOY2016 `endf.f90`.
//!
//! ENDF defines six interpolation laws for tabulated data (INT codes):
//!
//! | Code | Name | Formula |
//! |------|------|---------|
//! | 1 | histogram | y = y₁ (constant) |
//! | 2 | lin-lin   | linear in x and y |
//! | 3 | lin-log   | linear in x, log in y |
//! | 4 | log-lin   | log in x, linear in y |
//! | 5 | log-log   | log in x and y |
//! | 6 | charged-particle special (not used here) |

use crate::NjoyError;

/// Interpolation law code as defined in ENDF-6.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u32)]
pub enum IntLaw {
    /// Histogram (constant): y = y₁ for x₁ ≤ x < x₂.
    Histogram = 1,
    /// Linear–linear: y interpolated linearly in both x and y.
    LinLin = 2,
    /// Linear–log (ENDF INT=3): y linear in ln(x).
    LinLog = 3,
    /// Log–linear (ENDF INT=4): ln(y) linear in x.
    LogLin = 4,
    /// Log–log: both x and y treated logarithmically.
    LogLog = 5,
}

impl IntLaw {
    /// Convert an ENDF INT code to the enum. Unknown codes default to `LinLin`
    /// with a warning (matches NJOY's `terp1` fallthrough behaviour).
    pub fn from_code(code: u32) -> Self {
        match code {
            1 => IntLaw::Histogram,
            2 => IntLaw::LinLin,
            3 => IntLaw::LinLog,
            4 => IntLaw::LogLin,
            5 => IntLaw::LogLog,
            _ => IntLaw::LinLin,
        }
    }
}

/// Interpolate y at x given two bounding points (x₁, y₁) and (x₂, y₂) under
/// the given ENDF interpolation law.
///
/// Mirrors `terp1(x1,y1,x2,y2,x,y,i)` in NJOY2016 `endf.f90`, including its
/// degenerate-interval handling (`x₂ = x₁` or `y₂ = y₁` returns `y₁`). The law
/// semantics follow ENDF-102 §0.5.2 exactly: INT=3 is *y linear in ln x*
/// ([`IntLaw::LinLog`]) and INT=4 is *ln y linear in x* ([`IntLaw::LogLin`]).
///
/// > **History.** Until 2026-08-11 this function had the formulas for laws 3
/// > and 4 swapped (code 3 computed ln-y-linear-in-x and vice versa); the swap
/// > was caught while wiring S(α,β) temperature interpolation, whose LI=4
/// > records must interpolate ln S linearly in T.
///
/// Returns `Err(NjoyError::EndfParse)` if a log argument is non-positive.
///
/// # Examples
///
/// ```
/// use njoy_outram_park_fork::endf::interp::{terp1, IntLaw};
/// // Linear interpolation at midpoint
/// let y = terp1(0.0, 0.0, 2.0, 4.0, 1.0, IntLaw::LinLin).unwrap();
/// assert!((y - 2.0).abs() < 1e-12);
/// ```
pub fn terp1(x1: f64, y1: f64, x2: f64, y2: f64, x: f64, law: IntLaw) -> Result<f64, NjoyError> {
    if (x2 - x1).abs() < f64::EPSILON {
        return Ok(y1); // degenerate interval
    }
    if y1 == y2 {
        return Ok(y1); // flat segment — every law reduces to y₁ (upstream parity)
    }
    let r = (x - x1) / (x2 - x1); // linear-in-x fraction (laws 2 and 4)

    match law {
        IntLaw::Histogram => Ok(y1),
        IntLaw::LinLin => Ok(y1 + r * (y2 - y1)),
        IntLaw::LinLog => {
            // ENDF INT=3: y linear in ln(x).
            if x1 <= 0.0 || x2 <= 0.0 || x <= 0.0 {
                return Err(NjoyError::EndfParse(
                    "log interpolation with non-positive x".into(),
                ));
            }
            let r_log = (x / x1).ln() / (x2 / x1).ln();
            Ok(y1 + r_log * (y2 - y1))
        }
        IntLaw::LogLin => {
            // ENDF INT=4: ln(y) linear in x.
            if y1 <= 0.0 || y2 <= 0.0 {
                return Err(NjoyError::EndfParse(
                    "log interpolation with non-positive y".into(),
                ));
            }
            Ok(y1 * (y2 / y1).powf(r))
        }
        IntLaw::LogLog => {
            if x1 <= 0.0 || x2 <= 0.0 || y1 <= 0.0 || y2 <= 0.0 {
                return Err(NjoyError::EndfParse(
                    "log-log interpolation with non-positive value".into(),
                ));
            }
            let r_log = (x / x1).ln() / (x2 / x1).ln();
            Ok(y1 * (y2 / y1).powf(r_log))
        }
    }
}

/// Evaluate a TAB1 at point `x` using the region interpolation table.
///
/// `interp` is the region table: `(nbt, int_code)` pairs where `nbt` is the
/// index (1-based) of the last point in that region. `xy` is the `(x, y)` pair
/// slice in order.
///
/// Returns `0.0` for `x` outside the table range (matching NJOY's `gety1`
/// behaviour: function is zero outside the defined range).
pub fn eval_tab1(x: f64, interp: &[(u32, u32)], xy: &[(f64, f64)]) -> Result<f64, NjoyError> {
    if xy.is_empty() {
        return Ok(0.0);
    }
    let x_min = xy.first().unwrap().0;
    let x_max = xy.last().unwrap().0;
    if x < x_min || x > x_max {
        return Ok(0.0);
    }

    // Binary search for the interval [xy[i], xy[i+1]] containing x
    let pos = xy.partition_point(|&(xi, _)| xi <= x);
    let i = if pos == 0 { 0 } else { pos - 1 };
    let i = i.min(xy.len() - 2);

    let (x1, y1) = xy[i];
    let (x2, y2) = xy[i + 1];

    // Determine which interpolation region contains point i+1 (1-based ENDF index)
    let endf_idx = (i + 2) as u32; // 1-based index of the right endpoint
    let law_code = interp
        .iter()
        .find(|&&(nbt, _)| endf_idx <= nbt)
        .map(|&(_, int)| int)
        .unwrap_or(2); // default lin-lin

    terp1(x1, y1, x2, y2, x, IntLaw::from_code(law_code))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn linlin_midpoint() {
        let y = terp1(1.0, 2.0, 3.0, 6.0, 2.0, IntLaw::LinLin).unwrap();
        assert!((y - 4.0).abs() < 1e-12);
    }

    #[test]
    fn histogram_returns_left() {
        let y = terp1(0.0, 5.0, 10.0, 99.0, 7.0, IntLaw::Histogram).unwrap();
        assert_eq!(y, 5.0);
    }

    #[test]
    fn loglog() {
        // y = x^2 from (1,1) to (10,100), interp at x=3 → y=9
        let y = terp1(1.0, 1.0, 10.0, 100.0, 3.0, IntLaw::LogLog).unwrap();
        assert!((y - 9.0).abs() < 1e-10, "got {}", y);
    }

    /// ENDF INT=3 (lin-log): y linear in ln(x). At the geometric mean of x the
    /// result is the arithmetic mean of y — pins the law-3 semantics of
    /// ENDF-102 §0.5.2 / NJOY `terp1` (`y = y1 + log(x/x1)·(y2−y1)/log(x2/x1)`),
    /// guarding against the pre-2026-08-11 3↔4 formula swap.
    #[test]
    fn linlog_is_y_linear_in_ln_x() {
        let y = terp1(1.0, 2.0, 100.0, 6.0, 10.0, IntLaw::LinLog).unwrap();
        assert!((y - 4.0).abs() < 1e-12, "y linear in ln x: got {y}, want 4");
    }

    /// ENDF INT=4 (log-lin): ln(y) linear in x. At the arithmetic mean of x the
    /// result is the geometric mean of y — pins the law-4 semantics of
    /// ENDF-102 §0.5.2 / NJOY `terp1` (`y = y1·exp((x−x1)·log(y2/y1)/(x2−x1))`).
    /// This is the law the ENDF/B-VIII.0 graphite S(α,β) LI=4 records use for
    /// temperature interpolation.
    #[test]
    fn loglin_is_ln_y_linear_in_x() {
        let y = terp1(0.0, 1.0, 2.0, 9.0, 1.0, IntLaw::LogLin).unwrap();
        assert!((y - 3.0).abs() < 1e-12, "ln y linear in x: got {y}, want 3");
    }

    /// Flat segments short-circuit to y₁ under every law (upstream `terp1`
    /// parity: `y2.eq.y1` returns y1), including log-in-y laws where y = 0
    /// would otherwise be a domain error.
    #[test]
    fn flat_segment_returns_y1_for_all_laws() {
        for law in [IntLaw::Histogram, IntLaw::LinLin, IntLaw::LinLog, IntLaw::LogLin, IntLaw::LogLog] {
            let y = terp1(1.0, 0.0, 2.0, 0.0, 1.5, law).unwrap();
            assert_eq!(y, 0.0, "flat zero segment must interpolate to zero ({law:?})");
        }
    }
}
