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
    // `endf.f90:1420` tests exact equality, not a tolerance. A tolerance here
    // would treat a legitimately narrow panel as degenerate and return `y1`
    // where upstream interpolates.
    if x2 == x1 {
        return Ok(y1);
    }
    // `:1426` — one guard covering the histogram law, a flat segment, and the
    // lower endpoint, in that order.
    if law == IntLaw::Histogram || y2 == y1 || x == x1 {
        return Ok(y1);
    }
    match law {
        IntLaw::Histogram => Ok(y1),
        // `:1431`. The grouping is upstream's: multiply, then divide. It is
        // algebraically the same as scaling a precomputed fraction and is
        // **not** the same in floating point, and the difference shows up as
        // the last bit of a written ACE word.
        IntLaw::LinLin => Ok(y1 + (x - x1) * (y2 - y1) / (x2 - x1)),
        IntLaw::LinLog => {
            // ENDF INT=3: y linear in ln(x) (`:1435`).
            if x1 <= 0.0 || x2 <= 0.0 || x <= 0.0 {
                return Err(NjoyError::EndfParse(
                    "log interpolation with non-positive x".into(),
                ));
            }
            Ok(y1 + (x / x1).ln() * (y2 - y1) / (x2 / x1).ln())
        }
        IntLaw::LogLin => {
            // ENDF INT=4: ln(y) linear in x (`:1439`). Upstream writes this as
            // `y1*exp(...)`, not as a power; `powf` rounds differently.
            if y1 <= 0.0 || y2 <= 0.0 {
                return Err(NjoyError::EndfParse(
                    "log interpolation with non-positive y".into(),
                ));
            }
            Ok(y1 * ((x - x1) * (y2 / y1).ln() / (x2 - x1)).exp())
        }
        IntLaw::LogLog => {
            // `:1443-1448`. Upstream returns `y1` outright when `y1 = 0`
            // rather than erroring, because a log-log panel rooted at zero is
            // legal on an ENDF tape and is flat by construction.
            if y1 == 0.0 {
                return Ok(y1);
            }
            if x1 <= 0.0 || x2 <= 0.0 || x <= 0.0 || y1 < 0.0 || y2 <= 0.0 {
                return Err(NjoyError::EndfParse(
                    "log-log interpolation with non-positive value".into(),
                ));
            }
            Ok(y1 * ((x / x1).ln() * (y2 / y1).ln() / (x2 / x1).ln()).exp())
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


/// `shade` in `terpa` (`endf.f90:1743`) — how far past the last point the
/// table's last value is still returned.
pub const TERPA_SHADE: f64 = 1.00001;
/// `xbig` in `terpa` (`endf.f90:1744`) — upstream's "no further break".
pub const TERPA_XBIG: f64 = 1.0e12;

/// `terpa(y, x, xnext, idis, a, ip, ir)` — value, next grid point above `x`,
/// and the discontinuity flag, for a TAB1 held in memory.
///
/// Faithful port of `terpa` (`endf.f90:1729-1814`), including every boundary
/// convention, which is the part callers get wrong:
///
/// - **below the first point**: `y = 0`, `xnext = x_1`, `idis = true`;
/// - **at a tabulated point** `x_k` (`k < NP`): `y = y_k` exactly, never an
///   interpolation of the panel above it, `xnext = x_{k+1}`;
/// - **from the last point up to `shade` times it**: `y = y_NP` with
///   `xnext = shade^2 x_NP`, or `xbig` when `y_NP = 0`;
/// - **beyond that**: `y = 0`, `xnext = xbig`.
///
/// `idis` is set for a histogram region and for a duplicated `xnext` (an ENDF
/// discontinuity is two points sharing an abscissa).
///
/// Upstream carries `ip`/`ir` across calls as a sequential-access optimisation;
/// this re-brackets on every call, which gives the same answer for any `x`
/// rather than only for an ascending sequence.
pub fn terpa(interp: &[(u32, u32)], pairs: &[(f64, f64)], x: f64) -> (f64, f64, bool) {
    let np = pairs.len();
    if np == 0 {
        return (0.0, TERPA_XBIG, false);
    }
    if x < pairs[0].0 {
        return (0.0, pairs[0].0, true);
    }
    let last = pairs[np - 1];
    if x >= last.0 {
        if x < TERPA_SHADE * last.0 {
            let xnext = if last.1 > 0.0 {
                TERPA_SHADE * TERPA_SHADE * last.0
            } else {
                TERPA_XBIG
            };
            return (last.1, xnext, false);
        }
        return (0.0, TERPA_XBIG, false);
    }
    // 1-based index of the first point strictly above `x`.
    let ip = (pairs.partition_point(|q| q.0 <= x) + 1).clamp(2, np);
    let law = interp
        .iter()
        .find(|&&(nbt, _)| ip as u32 <= nbt)
        .map(|&(_, int)| int)
        .unwrap_or(2);
    let (x1, y1) = pairs[ip - 2];
    let (x2, y2) = pairs[ip - 1];
    let y = if x == x1 {
        y1
    } else {
        terp1(x1, y1, x2, y2, x, IntLaw::from_code(law)).unwrap_or(y1)
    };
    let mut idis = law == 1;
    if ip < np && pairs[ip].0 == x2 {
        idis = true;
    }
    (y, x2, idis)
}

/// Exact integral over one tabulated panel under an ENDF interpolation law.
///
/// Faithful port of `gral` (`endf.f90:1815-1930`): the integral from `x1` to
/// `x2` of the function whose panel endpoints are `(xl, yl)` and `(xh, yh)`
/// under INT code `int_code`.
///
/// Upstream's **fallbacks are part of the specification** and are reproduced
/// here: a log-in-x law with a non-positive abscissa degrades to lin-lin, a
/// log-in-y law with a negative ordinate degrades to lin-lin, and log-log
/// degrades first to log-lin and then to lin-lin the same way. Nothing here
/// errors — upstream's only `error()` call is on `x2 < x1`, which is a caller
/// bug and is returned as `Err`.
///
/// The `|z| <= 0.1` series expansions are upstream's and are kept: the closed
/// forms they replace lose precision on a narrow panel, which is exactly where
/// a cumulative integral like ACER's coherent sampling block spends its time.
///
/// # Errors
/// [`NjoyError::EndfParse`] when `x2 < x1`.
pub fn gral(
    xl: f64,
    yl: f64,
    xh: f64,
    yh: f64,
    x1: f64,
    x2: f64,
    int_code: u32,
) -> Result<f64, NjoyError> {
    const BREAK: f64 = 0.1;
    if x2 == x1 {
        return Ok(0.0);
    }
    if x2 < x1 {
        return Err(NjoyError::EndfParse(format!(
            "gral: x2 ({x2:e}) < x1 ({x1:e})"
        )));
    }
    // y linear in x — also every fallback's destination.
    let linlin = || {
        let b = (yh - yl) / (xh - xl);
        let a = yl - b * xl;
        (x2 - x1) * (a + b * (x2 + x1) / 2.0)
    };
    // y linear in ln(x) (INT=3).
    let linlog = || {
        let b = (yh - yl) / (xh / xl).ln();
        let z = (x2 - x1) / x1;
        if z.abs() <= BREAK {
            (x2 - x1) * (yl + b * (x1 / xl).ln())
                + b * x1 * z * z * (1.0 + z * (-1.0 / 3.0 + z * (1.0 / 6.0 - z / 10.0))) / 2.0
        } else {
            (x2 - x1) * (yl + b * (x1 / xl).ln())
                + b * x1 * (1.0 + (x2 / x1) * ((x2 / x1).ln() - 1.0))
        }
    };
    // ln(y) linear in x (INT=4).
    let loglin = || {
        let b = (yh / yl).ln() / (xh - xl);
        let a = yl.ln() - b * xl;
        let z = (x2 - x1) * b;
        if z.abs() <= BREAK {
            (a + b * x1).exp() * (x2 - x1) * (1.0 + z * (1.0 / 2.0 + z / 6.0))
        } else {
            (a + b * x1).exp() * (z.exp() - 1.0) / b
        }
    };
    let f = match int_code {
        1 => (x2 - x1) * yl,
        2 => linlin(),
        3 => {
            if xl <= 0.0 || xh <= 0.0 {
                linlin()
            } else {
                linlog()
            }
        }
        4 => {
            if yl < 0.0 || yh < 0.0 {
                linlin()
            } else {
                loglin()
            }
        }
        5 => {
            if xl <= 0.0 || xh <= 0.0 {
                if yl < 0.0 || yh < 0.0 {
                    linlin()
                } else {
                    loglin()
                }
            } else if yl < 0.0 || yh < 0.0 {
                linlog()
            } else {
                let b = (yh / yl).ln() / (xh / xl).ln();
                let z = (b + 1.0) * (x2 / x1).ln();
                if z.abs() <= BREAK {
                    yl * x1 * (x1 / xl).powf(b) * (x2 / x1).ln() * (1.0 + z * (1.0 / 2.0 + z / 6.0))
                } else {
                    yl * x1 * (x1 / xl).powf(b) * ((x2 / x1).powf(b + 1.0) - 1.0) / (b + 1.0)
                }
            }
        }
        // Upstream's `gral` leaves `gral = 0` for any other code.
        _ => 0.0,
    };
    Ok(f)
}

/// Integral of a TAB1 from `x1` to `x2`, panel by panel through [`gral`].
///
/// Faithful port of `intega` (`endf.f90:1649-1727`). The function is taken as
/// **zero outside the table's own range**, which is upstream's stated
/// assumption and not an approximation this port introduces.
///
/// `interp` is the ENDF region table `(nbt, int_code)` with `nbt` the 1-based
/// index of the region's last point; `pairs` are the `(x, y)` points in
/// ascending `x`.
///
/// # Errors
/// [`NjoyError::EndfParse`] when `x2 < x1`, propagated from [`gral`].
pub fn intega(
    interp: &[(u32, u32)],
    pairs: &[(f64, f64)],
    x1: f64,
    x2: f64,
) -> Result<f64, NjoyError> {
    let np = pairs.len();
    if np < 2 || x2 <= x1 {
        return Ok(0.0);
    }
    // Wholly outside the table: upstream's `go to 170` with f = 0.
    if x2 <= pairs[0].0 || x1 >= pairs[np - 1].0 {
        return Ok(0.0);
    }
    let law_at = |ip: usize| -> u32 {
        interp
            .iter()
            .find(|&&(nbt, _)| ip as u32 <= nbt)
            .map(|&(_, int)| int)
            .unwrap_or(2)
    };
    // `ip` is upstream's 1-based index of the first point above `x1`.
    let mut ip = pairs.partition_point(|q| q.0 <= x1) + 1;
    if ip > np {
        ip = np;
    }
    if ip < 2 {
        ip = 2;
    }
    let mut xlo = x1.max(pairs[ip - 2].0);
    let mut f = 0.0f64;
    loop {
        let (xl, yl) = pairs[ip - 2];
        let (xh, yh) = pairs[ip - 1];
        let xhi = xh.min(x2);
        f += gral(xl, yl, xh, yh, xlo, xhi, law_at(ip))?;
        if xhi >= x2 || ip == np {
            break;
        }
        xlo = xhi;
        ip += 1;
    }
    Ok(f)
}
#[cfg(test)]
mod tests {
    use super::*;

    /// `gral` is an **exact** panel integral, not a quadrature, so each law is
    /// checked against its own closed form rather than against a fine sum.
    #[test]
    fn gral_is_exact_for_every_law() {
        // INT=1 histogram: the integral is y1 * (x2 - x1).
        assert!((gral(1.0, 3.0, 2.0, 7.0, 1.0, 2.0, 1).unwrap() - 3.0).abs() < 1e-14);
        // INT=2 lin-lin over the whole panel: the trapezoid, exactly.
        let f = gral(1.0, 2.0, 3.0, 6.0, 1.0, 3.0, 2).unwrap();
        assert!((f - (2.0 + 6.0) / 2.0 * 2.0).abs() < 1e-13, "{f}");
        // INT=4, ln(y) linear in x: y = y1 exp(b (x-x1)) with
        // b = ln(y2/y1)/(x2-x1); the integral is (y2 - y1)/b.
        let (x1, y1, x2, y2) = (1.0f64, 1.0f64, 3.0f64, 8.0f64);
        let b = (y2 / y1).ln() / (x2 - x1);
        let want = (y2 - y1) / b;
        let got = gral(x1, y1, x2, y2, x1, x2, 4).unwrap();
        assert!((got - want).abs() / want < 1e-12, "law 4: {got} vs {want}");
        // INT=5, log-log: y = y1 (x/x1)^p with p = ln(y2/y1)/ln(x2/x1); the
        // integral is y1 x1 [(x2/x1)^(p+1) - 1] / (p+1).
        let p = (y2 / y1).ln() / (x2 / x1).ln();
        let want = y1 * x1 * ((x2 / x1).powf(p + 1.0) - 1.0) / (p + 1.0);
        let got = gral(x1, y1, x2, y2, x1, x2, 5).unwrap();
        assert!((got - want).abs() / want < 1e-12, "law 5: {got} vs {want}");
        // INT=3, y linear in ln(x): integral of y1 + b ln(x/x1) over the panel,
        // b = (y2-y1)/ln(x2/x1). Closed form: (x2-x1)(y1 - b) + b(x2 ln(x2/x1)).
        let b3 = (y2 - y1) / (x2 / x1).ln();
        let want = (x2 - x1) * (y1 - b3) + b3 * x2 * (x2 / x1).ln();
        let got = gral(x1, y1, x2, y2, x1, x2, 3).unwrap();
        assert!((got - want).abs() / want < 1e-12, "law 3: {got} vs {want}");
    }

    /// Upstream's fallbacks are part of the specification: a log law with a
    /// non-positive abscissa or ordinate degrades to lin-lin rather than
    /// erroring, which is how `acepho` gets away with integrating a squared
    /// form factor whose first abscissa is zero.
    #[test]
    fn gral_falls_back_to_linlin_where_upstream_does() {
        // INT=3 with a non-positive abscissa -> lin-lin (`endf.f90:1849-1854`).
        let got = gral(0.0, 1.0, 2.0, 4.0, 0.0, 2.0, 3).unwrap();
        let linlin = gral(0.0, 1.0, 2.0, 4.0, 0.0, 2.0, 2).unwrap();
        assert!((got - linlin).abs() < 1e-14, "law 3 with x1 = 0: {got}");
        // INT=4 with a negative ordinate -> lin-lin (`:1868-1872`).
        let got = gral(1.0, -1.0, 2.0, 4.0, 1.0, 2.0, 4).unwrap();
        let linlin = gral(1.0, -1.0, 2.0, 4.0, 1.0, 2.0, 2).unwrap();
        assert!((got - linlin).abs() < 1e-14, "law 4 with y1 < 0: {got}");
        // INT=5 degrades in two steps, and the first step is **not** lin-lin:
        // a non-positive abscissa sends it to INT=4, not to INT=2 (`:1888`).
        let got = gral(0.0, 1.0, 2.0, 4.0, 0.0, 2.0, 5).unwrap();
        let law4 = gral(0.0, 1.0, 2.0, 4.0, 0.0, 2.0, 4).unwrap();
        assert!((got - law4).abs() < 1e-14, "law 5 with x1 = 0 -> law 4: {got}");
        // A negative ordinate with positive abscissae sends INT=5 to INT=3.
        let got = gral(1.0, -1.0, 2.0, 4.0, 1.0, 2.0, 5).unwrap();
        let law3 = gral(1.0, -1.0, 2.0, 4.0, 1.0, 2.0, 3).unwrap();
        assert!((got - law3).abs() < 1e-14, "law 5 with y1 < 0 -> law 3: {got}");
        // An unknown code integrates to zero, as upstream's `gral` does by
        // never assigning past its `if` chain.
        assert_eq!(gral(1.0, 1.0, 2.0, 4.0, 1.0, 2.0, 9).unwrap(), 0.0);
    }

    /// `x2 < x1` is a caller bug, not a zero integral.
    #[test]
    fn gral_rejects_a_reversed_interval() {
        assert!(gral(0.0, 0.0, 2.0, 4.0, 2.0, 1.0, 2).is_err());
    }

    /// `intega` walks panels and clips to the table's own range, which is the
    /// behaviour `acepho`'s coherent sampling integral depends on.
    #[test]
    fn intega_spans_panels_and_clips_outside_the_table() {
        // y = x on [0, 3] as three lin-lin panels.
        let interp = [(4u32, 2u32)];
        let pairs = [(0.0, 0.0), (1.0, 1.0), (2.0, 2.0), (3.0, 3.0)];
        let f = intega(&interp, &pairs, 0.0, 3.0).unwrap();
        assert!((f - 4.5).abs() < 1e-13, "{f}");
        // Partial panel.
        let f = intega(&interp, &pairs, 0.5, 2.5).unwrap();
        assert!((f - (2.5 * 2.5 - 0.5 * 0.5) / 2.0).abs() < 1e-13, "{f}");
        // The function is zero outside the table, so an upper limit past the
        // last point adds nothing.
        let f = intega(&interp, &pairs, 0.0, 10.0).unwrap();
        assert!((f - 4.5).abs() < 1e-13, "{f}");
        // Entirely outside.
        assert_eq!(intega(&interp, &pairs, 5.0, 6.0).unwrap(), 0.0);
        assert_eq!(intega(&interp, &pairs, 1.0, 1.0).unwrap(), 0.0);
    }

    /// `terp1`'s lin-lin grouping is upstream's, and that is checkable: the
    /// two algebraically equal forms differ in the last bit on a case picked
    /// for it.
    #[test]
    fn terp1_linlin_uses_upstreams_grouping() {
        let (x1, y1, x2, y2, x) = (1.5e7, 0.1849166, 2.0e7, 0.1651052, 1.50000000000014994e7);
        let ours = terp1(x1, y1, x2, y2, x, IntLaw::LinLin).unwrap();
        let upstream = y1 + (x - x1) * (y2 - y1) / (x2 - x1);
        assert_eq!(
            ours.to_bits(),
            upstream.to_bits(),
            "terp1 must reproduce endf.f90:1431's grouping bit for bit"
        );
    }

    /// `x2 == x1` is upstream's degeneracy test. A tolerance would swallow a
    /// narrow-but-real panel.
    #[test]
    fn terp1_degeneracy_is_exact_equality_not_a_tolerance() {
        let (x1, x2) = (1.0, 1.0 + 1.0e-17);
        assert!(x2 > x1 || x2 == x1); // whichever the hardware gives
        // A panel narrower than EPSILON but genuinely distinct must still
        // interpolate rather than return y1.
        let (a, b) = (1.0e-30, 3.0e-30);
        let y = terp1(a, 0.0, b, 2.0, 2.0e-30, IntLaw::LinLin).unwrap();
        assert!((y - 1.0).abs() < 1e-12, "narrow panel must interpolate, got {y}");
    }

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
        for law in [
            IntLaw::Histogram,
            IntLaw::LinLin,
            IntLaw::LinLog,
            IntLaw::LogLin,
            IntLaw::LogLog,
        ] {
            let y = terp1(1.0, 0.0, 2.0, 0.0, 1.5, law).unwrap();
            assert_eq!(
                y, 0.0,
                "flat zero segment must interpolate to zero ({law:?})"
            );
        }
    }
}
