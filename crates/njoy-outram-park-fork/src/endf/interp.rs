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
    /// Linear–log: y = exp(lin interp of ln y).
    LinLog = 3,
    /// Log–linear: x treated logarithmically, y linearly.
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
/// Mirrors `terp1(x1,y1,x2,y2,x,y,i)` in NJOY2016 `endf.f90`.
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
    let r = (x - x1) / (x2 - x1); // always linear in x for laws 1-2-3

    match law {
        IntLaw::Histogram => Ok(y1),
        IntLaw::LinLin => Ok(y1 + r * (y2 - y1)),
        IntLaw::LinLog => {
            if y1 <= 0.0 || y2 <= 0.0 {
                return Err(NjoyError::EndfParse(
                    "log interpolation with non-positive y".into(),
                ));
            }
            Ok(y1 * (y2 / y1).powf(r))
        }
        IntLaw::LogLin => {
            if x1 <= 0.0 || x2 <= 0.0 {
                return Err(NjoyError::EndfParse(
                    "log interpolation with non-positive x".into(),
                ));
            }
            let r_log = (x / x1).ln() / (x2 / x1).ln();
            Ok(y1 + r_log * (y2 - y1))
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
}
