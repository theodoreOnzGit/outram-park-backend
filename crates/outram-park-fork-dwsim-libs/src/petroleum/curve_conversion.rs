//! Distillation-curve interconversion: ASTM D86, ASTM D1160, ASTM D2887
//! (simulated distillation) and sub-atmospheric TBP curves → the atmospheric
//! **TBP (True Boiling Point, ASTM D2892)** curve that all pseudo-component
//! generation works from, plus the smooth 6th-degree TBP fit used to cut it.
//!
//! # Provenance
//!
//! Faithful port of DWSIM (GPL-3.0),
//! `DWSIM.Thermodynamics/PetroleumCharacterization/CurveConversion.vb`
//! (294 lines, whole file), from the pinned upstream clone
//! `/home/teddy0/Documents/research/dwsim-upstream`, branch `windows`, commit
//! `1abf72d1b6b41d3e9a8cc770d3cc4e8fc76e5766`. Upstream copyright: 2009 Daniel
//! Wagner O. de Medeiros and the DWSIM contributors. GPL-3.0; this port is
//! GPL-3.0-only.
//!
//! Mapping:
//!
//! | Rust item | Upstream |
//! |---|---|
//! | [`d86_to_tbp_riazi`] | `ASTMD86ToPEV_Riazi`, `:31-58` |
//! | [`d2887_to_tbp_daubert`] | `ASTMD2887ToPEV_Daubert`, `:66-112` |
//! | [`subatmospheric_tbp_to_atmospheric_maxwell_bonnell`] | `PEVsubToPEV_MaxwellBonnel`, `:122-153` |
//! | [`d1160_to_subatmospheric_tbp_wauquier`] | `ASTMD1160ToPEVsub_Wauquier`, `:161-216` |
//! | [`TbpCurveFit`] + [`fit_tbp_curve`] | class `TBPFit`, `:218-290` |
//! | [`TbpCurveFit::temperature_at`] | `DistCurves.cs:1004-1011` `GetT` |
//! | [`TbpCurveFit::volume_fraction_at`] | `DistCurves.cs:979-1002` `GetFV` |
//!
//! ("PEV" is *ponto de ebulição verdadeiro*, Portuguese for true boiling point;
//! upstream is written by a Brazilian author and uses PEV throughout where the
//! English literature says TBP.)
//!
//! # Why the conversions exist
//!
//! A refinery assay is normally measured by a **cheap, fast** distillation
//! (ASTM D86 at atmospheric pressure for light stocks, D1160 under vacuum for
//! heavy ones, D2887 by gas chromatography), not by the slow 15-plate **TBP**
//! distillation. Every pseudo-component correlation in
//! [`crate::petroleum::property_methods`] is regressed against TBP mid-boiling
//! points, so the measured curve must first be mapped onto the TBP basis.
//!
//! # Units
//!
//! All public functions are `uom`-typed: temperatures are
//! `ThermodynamicTemperature` [K], pressures `Pressure` [Pa], volume fractions
//! `Ratio` [-] on a `0..1` scale (**not** percent). Internally the correlations
//! run in the units they were published in — °F for Daubert's D2887 method,
//! °R and mmHg for Maxwell-Bonnell — with the conversions written out inline
//! exactly as upstream.
//!
//! # Excluded DWSIM behavior
//!
//! - The `TBPFit.GetCoeffs` `Object()` return `{coeffs, info, sum, its}`
//!   (`:252`) and its 1-based coefficient shuffling (`:229-234`, `:244-250`)
//!   are replaced by the typed [`TbpCurveFit`] / [`crate::petroleum::lm`]
//!   results — see [`crate::petroleum::lm`] for why the solver body itself is
//!   an independent implementation rather than a port of vendored ALGLIB.
//! - `ASTMD2887ToPEV_Daubert` mutates its **caller's** array in place while
//!   converting K → °F (`:73-75`). That side effect is not reproduced; this
//!   port takes an immutable slice and works on a copy.
//!
//! # Deviation from upstream (one, deliberate, documented)
//!
//! [`d1160_to_subatmospheric_tbp_wauquier`] clamps an internal table index that
//! upstream leaves unclamped; see that function's docs. Without the clamp the
//! upstream code raises `IndexOutOfRangeException` for perfectly ordinary
//! inputs (adjacent D1160 points less than 10 K apart).

use uom::si::f64::{Pressure, Ratio, ThermodynamicTemperature};
use uom::si::pressure::pascal;
use uom::si::ratio::ratio;
use uom::si::thermodynamic_temperature::kelvin;

use super::lm::{levenberg_marquardt, LmError, LmModel, LmOptions};

/// Convert an **ASTM D86** distillation curve to the **TBP (ASTM D2892)** curve
/// by Riazi's power-law point conversion.
///
/// `T_TBP,i = a_i · T_D86,i ^ b_i`, with a distinct `(a, b)` pair for each of
/// the seven standard cut points **0, 10, 30, 50, 70, 90, 100 vol %**:
///
/// | vol % | 0 | 10 | 30 | 50 | 70 | 90 | 100 |
/// |---|---|---|---|---|---|---|---|
/// | `a` | 0.9177 | 0.5564 | 0.7617 | 0.9013 | 0.8821 | 0.9552 | 0.8177 |
/// | `b` | 1.0019 | 1.0900 | 1.0425 | 1.0176 | 1.0226 | 1.0110 | 1.0355 |
///
/// Ported from `CurveConversion.vb:31-58`.
///
/// # Inputs
///
/// `d86` — exactly **seven** D86 temperatures [K], in the cut-point order
/// above. Values must be positive (the power law is undefined otherwise).
///
/// # Valid range
///
/// The D86 method itself applies to fractions boiling below ~400 °C at
/// atmospheric pressure; the conversion coefficients are Riazi's fit to the API
/// Technical Data Book procedure 3A1.1.
///
/// # Errors
///
/// Returns [`CurveConversionError::WrongPointCount`] unless exactly 7 points
/// are supplied.
pub fn d86_to_tbp_riazi(
    d86: &[ThermodynamicTemperature],
) -> Result<Vec<ThermodynamicTemperature>, CurveConversionError> {
    const A: [f64; 7] = [0.9177, 0.5564, 0.7617, 0.9013, 0.8821, 0.9552, 0.8177];
    const B: [f64; 7] = [1.0019, 1.09, 1.0425, 1.0176, 1.0226, 1.011, 1.0355];
    if d86.len() != 7 {
        return Err(CurveConversionError::WrongPointCount {
            expected: 7,
            got: d86.len(),
        });
    }
    Ok((0..7)
        .map(|i| ThermodynamicTemperature::new::<kelvin>(A[i] * d86[i].get::<kelvin>().powf(B[i])))
        .collect())
}

/// Convert an **ASTM D2887** (simulated distillation, gas chromatography)
/// curve to the **TBP** curve by Daubert's method.
///
/// Daubert's procedure works on **differences** rather than absolute
/// temperatures: with the curve in °F, the six inter-point rises are converted
/// by `ΔT_i = a_i · (ΔT_D2887,i)^{b_i}` and accumulated outward from the 50 %
/// point, which is carried across unchanged.
///
/// | i | 0 | 1 | 2 | 3 | 4 | 5 | 6 |
/// |---|---|---|---|---|---|---|---|
/// | `a` | 0.15779 | 0.011903 | 0.05342 | 0.19861 | 0.31531 | 0.97476 | 0.02172 |
/// | `b` | 1.4296 | 2.0253 | 1.6988 | 1.3975 | 1.2938 | 0.8723 | 1.9733 |
///
/// Ported from `CurveConversion.vb:66-112`.
///
/// # Inputs
///
/// `d2887` — exactly **eight** temperatures [K] at **5, 10, 30, 50, 70, 90,
/// 95, 100 wt %** (D2887 is a mass-basis method). Index 3 (the 50 % point) is
/// the pivot.
///
/// # Errors
///
/// Returns [`CurveConversionError::WrongPointCount`] unless exactly 8 points
/// are supplied.
pub fn d2887_to_tbp_daubert(
    d2887: &[ThermodynamicTemperature],
) -> Result<Vec<ThermodynamicTemperature>, CurveConversionError> {
    const A: [f64; 7] = [
        0.15779, 0.011903, 0.05342, 0.19861, 0.31531, 0.97476, 0.02172,
    ];
    const B: [f64; 7] = [1.4296, 2.0253, 1.6988, 1.3975, 1.2938, 0.8723, 1.9733];
    if d2887.len() != 8 {
        return Err(CurveConversionError::WrongPointCount {
            expected: 8,
            got: d2887.len(),
        });
    }
    // K -> °F (upstream `:73-75`, which mutates the caller's array; we copy).
    let t: Vec<f64> = d2887
        .iter()
        .map(|v| (v.get::<kelvin>() - 273.15) * 9.0 / 5.0 + 32.0)
        .collect();

    let mut dt = [0.0_f64; 7];
    for i in 0..7 {
        dt[i] = A[i] * (t[i + 1] - t[i]).powf(B[i]);
    }

    // Accumulate outward from T50 (`:97-104`).
    let pivot = t[3];
    let tbp_f = [
        pivot - dt[0] - dt[1] - dt[2],         // T5
        pivot - dt[0] - dt[1],                 // T10
        pivot - dt[0],                         // T30
        pivot,                                 // T50
        pivot + dt[3],                         // T70
        pivot + dt[3] + dt[4],                 // T90
        pivot + dt[3] + dt[4] + dt[5],         // T95
        pivot + dt[3] + dt[4] + dt[5] + dt[6], // T100
    ];

    // °F -> K (upstream `:106-108`).
    Ok(tbp_f
        .iter()
        .map(|f| ThermodynamicTemperature::new::<kelvin>((f - 32.0) / 9.0 * 5.0 + 273.15))
        .collect())
}

/// Convert a **sub-atmospheric (vacuum) TBP** temperature to its equivalent
/// **atmospheric TBP** temperature by the **Maxwell-Bonnell** vapour-pressure
/// method, with the Watson-`K` correction.
///
/// ```text
/// X   = (−5.994296 + 0.972546·log10 P) / (95.76·log10 P − 2663.129)
/// Tb' = −748.1·X / (2.867e-4 − 0.2145·X − 1/T)            [°R]
/// f   = 0                     if Tb' − 459.7 < 200 °F
///     = 1                     if Tb' − 459.7 > 400 °F
///     = (Tb' − 659.7) / 200   otherwise
/// Tb  = Tb' − 2.5·f·(Kw − 12)·log10(P/760)                [°R]
/// ```
///
/// Ported from `CurveConversion.vb:122-153`. `P` is converted Pa → mmHg
/// (`×760/101325`) and `T` K → °R (`×1.8`) on entry; the result is converted
/// back °R → K (`÷1.8`) on exit. Upstream iterates the block up to 1000 times
/// with a 0.001 °R tolerance; because no iterated quantity feeds back into `X`
/// or `Tb'`, the fixed point is reached on the second pass — the loop is
/// preserved here for exactness of behaviour.
///
/// # Inputs
///
/// - `temperature` — the observed vacuum-distillation temperature [K].
/// - `pressure` — the distillation pressure [Pa]. DWSIM's own D1160 path uses
///   **1333 Pa** (10 mmHg), `DistCurves.cs:418-424`.
/// - `watson_k` — the Watson characterisation factor `Kw` [-]; DWSIM's D1160
///   path assumes **12.0** (`DistCurves.cs:417`), i.e. no correction.
///
/// # Valid range
///
/// Maxwell-Bonnell is the API Technical Data Book procedure for `P` between
/// roughly 0.5 and 760 mmHg and `Tb` up to ~1200 °F.
#[must_use]
pub fn subatmospheric_tbp_to_atmospheric_maxwell_bonnell(
    temperature: ThermodynamicTemperature,
    pressure: Pressure,
    watson_k: Ratio,
) -> ThermodynamicTemperature {
    let p = pressure.get::<pascal>() * 760.0 / 101_325.0;
    let t = 1.8 * temperature.get::<kelvin>();
    let kw = watson_k.get::<ratio>();

    let mut tb = 0.0_f64;
    let mut iterations = 0;
    loop {
        let tb_previous = tb;

        let x = (-5.994296 + 0.972546 * p.log10()) / (95.76 * p.log10() - 2663.129);
        tb = -748.1 * x / (0.0002867 - 0.2145 * x - 1.0 / t);

        let f = if (tb - 459.7) < 200.0 {
            0.0
        } else if (tb - 459.7) > 400.0 {
            1.0
        } else {
            (tb - 659.7) / 200.0
        };

        tb -= 2.5 * f * (kw - 12.0) * (p / 760.0).log10();

        iterations += 1;
        if (tb - tb_previous).abs() < 0.001 || iterations > 1000 {
            break;
        }
    }

    ThermodynamicTemperature::new::<kelvin>(tb / 1.8)
}

/// Convert an **ASTM D1160** (vacuum distillation) curve to the equivalent
/// **sub-atmospheric TBP** curve by **Wauquier's** tabulated method.
///
/// The 50 % point is carried across unchanged; each lighter point is stepped
/// down from it using a two-row lookup table of cumulative corrections indexed
/// by the size of the D1160 temperature gap. Convert the result to the
/// atmospheric basis with
/// [`subatmospheric_tbp_to_atmospheric_maxwell_bonnell`].
///
/// Ported from `CurveConversion.vb:161-216`.
///
/// # Inputs
///
/// `d1160` — exactly **seven** temperatures [K] at **0, 10, 30, 50, 70, 90,
/// 100 vol %**. Index 3 (the 50 % point) is the pivot.
///
/// # Deviation from upstream (deliberate)
///
/// Upstream computes the table index as
/// `i2 = Int(|ΔT| / 10) − 1` (`:207`) and clamps only the **upper** end
/// (`If i2 > 9 Then i2 = 9`). Whenever two adjacent D1160 points differ by less
/// than 10 K — routine for a narrow-boiling stock — `i2` becomes `−1` and
/// upstream indexes `f(i1, −1)`, raising `IndexOutOfRangeException`. **This
/// port clamps `i2` into `0..=9`**, which reproduces upstream exactly for every
/// input upstream can actually process and degrades gracefully instead of
/// panicking on the rest. Flagged rather than silently fixed.
///
/// # Errors
///
/// Returns [`CurveConversionError::WrongPointCount`] unless exactly 7 points
/// are supplied.
pub fn d1160_to_subatmospheric_tbp_wauquier(
    d1160: &[ThermodynamicTemperature],
) -> Result<Vec<ThermodynamicTemperature>, CurveConversionError> {
    if d1160.len() != 7 {
        return Err(CurveConversionError::WrongPointCount {
            expected: 7,
            got: d1160.len(),
        });
    }
    // Wauquier's cumulative-correction table, `:175-196`.
    const F: [[f64; 11]; 2] = [
        [
            0.0, 20.0, 35.5, 47.5, 57.0, 64.0, 70.0, 75.0, 82.5, 91.0, 100.0,
        ],
        [
            0.0, 13.0, 24.0, 34.5, 44.0, 53.5, 63.0, 72.0, 81.5, 90.5, 100.0,
        ],
    ];
    // The seven standard cut points, `:167-173`.
    const FV: [f64; 7] = [0.0, 10.0, 30.0, 50.0, 70.0, 90.0, 100.0];

    let t: Vec<f64> = d1160.iter().map(|v| v.get::<kelvin>()).collect();
    let mut out = vec![0.0_f64; 7];

    for i in 0..7 {
        // `Convert.ToInt32(fv/20 + 0.5)` — VB banker's rounding; for the fixed
        // FV table above this yields exactly 0,1,2,3,4,5,6.
        let fracv = vb_round_to_i32(FV[i] / 20.0 + 0.5);
        if fracv >= 3 {
            out[i] = t[i];
        } else {
            out[i] = t[3];
            let mut i1 = 0usize;
            let mut j = 3i32;
            while j >= fracv + 1 {
                if j == 2 {
                    i1 = 1;
                }
                let gap = (t[j as usize] - t[(j - 1) as usize]).abs();
                // Deliberate clamp — see the function docs.
                let i2 = ((gap / 10.0).trunc() as i32 - 1).clamp(0, 9) as usize;
                out[i] -= (gap / 10.0 - i2 as f64) * (F[i1][i2 + 1] - F[i1][i2]) + F[i1][i2];
                j -= 1;
            }
        }
    }

    Ok(out
        .into_iter()
        .map(ThermodynamicTemperature::new::<kelvin>)
        .collect())
}

/// VB.NET `Convert.ToInt32(Double)` semantics — round half to **even**
/// (banker's rounding), unlike Rust's `f64::round` which rounds half away from
/// zero. Used where upstream's arithmetic depends on the tie-breaking rule.
#[must_use]
pub(crate) fn vb_round_to_i32(value: f64) -> i32 {
    let floor = value.floor();
    let fraction = value - floor;
    let rounded = if (fraction - 0.5).abs() < f64::EPSILON {
        // Exact .5 -> choose the even neighbour.
        if (floor as i64) % 2 == 0 {
            floor
        } else {
            floor + 1.0
        }
    } else {
        value.round()
    };
    rounded as i32
}

/// A fitted **TBP curve**: the 6th-degree polynomial
/// `T(fv) = A + B·fv + C·fv² + D·fv³ + E·fv⁴ + F·fv⁵ + G·fv⁶`
/// mapping cumulative distilled fraction `fv ∈ [0, 1]` [-] to boiling
/// temperature [K].
///
/// This smooth form is what makes the assay *cuttable*: given any pair of cut
/// temperatures it can be inverted (see [`Self::volume_fraction_at`]) to obtain
/// the fraction of the barrel lying between them.
///
/// Produced by [`fit_tbp_curve`]; the coefficients correspond to upstream's
/// `coeff(0..6)` (`DistCurves.cs:481-488`).
#[derive(Debug, Clone, PartialEq)]
pub struct TbpCurveFit {
    /// Polynomial coefficients `A..G`, lowest order first [K, K, …].
    pub coefficients: [f64; 7],
    /// Sum of squared residuals of the fit [K²].
    pub sum_of_squares: f64,
    /// Iterations the Levenberg-Marquardt solver used.
    pub iterations: usize,
}

impl TbpCurveFit {
    /// Boiling temperature at a cumulative distilled fraction `fv` [-].
    ///
    /// Ported from `DistCurves.cs:1004-1011` (`GetT`). Valid for `fv ∈ [0, 1]`;
    /// the polynomial extrapolates outside that range but is not meaningful
    /// there.
    #[must_use]
    pub fn temperature_at(&self, fv: Ratio) -> ThermodynamicTemperature {
        let x = fv.get::<ratio>();
        let c = &self.coefficients;
        ThermodynamicTemperature::new::<kelvin>(
            c[0] + c[1] * x
                + c[2] * x.powi(2)
                + c[3] * x.powi(3)
                + c[4] * x.powi(4)
                + c[5] * x.powi(5)
                + c[6] * x.powi(6),
        )
    }

    /// Invert the fit: the cumulative distilled fraction `fv` [-] at which the
    /// curve reaches temperature `t` [K].
    ///
    /// Ported from `DistCurves.cs:979-1002` (`GetFV`) — a **damped Newton**
    /// iteration, `fv ← fv − 0.3·f/f'`, seeded at `initial_guess`, taking the
    /// absolute value whenever it strays negative, and stopping at
    /// `|f| < 1e-9` K or 1000 iterations. The 0.3 damping factor and the
    /// absolute-value guard are upstream's own and are reproduced exactly; they
    /// are what keeps the Newton step from running away on the near-flat tails
    /// of a 6th-degree fit.
    ///
    /// `initial_guess` is upstream's `fv0` — the *previous* cut's upper
    /// fraction, so successive inversions march monotonically up the curve.
    #[must_use]
    pub fn volume_fraction_at(&self, t: ThermodynamicTemperature, initial_guess: Ratio) -> Ratio {
        let target = t.get::<kelvin>();
        let c = &self.coefficients;
        let mut fv = initial_guess.get::<ratio>();
        let mut count = 0usize;
        loop {
            let f = -target
                + (c[0]
                    + c[1] * fv
                    + c[2] * fv.powi(2)
                    + c[3] * fv.powi(3)
                    + c[4] * fv.powi(4)
                    + c[5] * fv.powi(5)
                    + c[6] * fv.powi(6));
            let df = c[1]
                + 2.0 * c[2] * fv
                + 3.0 * c[3] * fv.powi(2)
                + 4.0 * c[4] * fv.powi(3)
                + 5.0 * c[5] * fv.powi(4)
                + 6.0 * c[6] * fv.powi(5);
            fv += -f / df * 0.3;
            if fv < 0.0 {
                fv = fv.abs();
            }
            count += 1;
            if f.abs() < 1.0e-9 || count >= 1000 || !fv.is_finite() {
                break;
            }
        }
        Ratio::new::<ratio>(fv)
    }
}

/// Fit the 6th-degree TBP polynomial through `(fv, T)` data by
/// Levenberg-Marquardt.
///
/// Ported from `CurveConversion.vb:224-254` (`TBPFit.GetCoeffs`) with the
/// initial estimate DWSIM seeds at `DistCurves.cs:460-477`:
/// `B..G = 1398, 4720, 11821, 15933, 10358, −3000` and `A` = the *observed*
/// initial-boiling-point temperature.
///
/// > **Why `A` must be seeded well.** Upstream's Jacobian sets `∂T/∂A = 0`
/// > (`CurveConversion.vb:275`), so the constant term is never adjusted by the
/// > solver — it stays exactly at `initial_boiling_point`. See
/// > [`crate::petroleum::lm::LmModel::TbpSixthDegreePolynomial`].
///
/// # Inputs
///
/// - `volume_fractions` — cumulative distilled fractions `fv` [-], `0..1`,
///   ascending. Must have at least 7 points (one per coefficient).
/// - `temperatures` — the TBP temperatures at those fractions [K], same length.
/// - `initial_boiling_point` — the seed (and, per the note above, final value)
///   of the constant term `A` [K]. Upstream uses `Tmin` of the TBP curve, or
///   the interpolated `fv = 0` value for a D86-derived curve.
///
/// # Errors
///
/// Propagates [`LmError`] from the solver (length mismatch or an
/// under-determined system).
pub fn fit_tbp_curve(
    volume_fractions: &[Ratio],
    temperatures: &[ThermodynamicTemperature],
    initial_boiling_point: ThermodynamicTemperature,
) -> Result<TbpCurveFit, LmError> {
    let x: Vec<f64> = volume_fractions.iter().map(|v| v.get::<ratio>()).collect();
    let y: Vec<f64> = temperatures.iter().map(|v| v.get::<kelvin>()).collect();
    let initial = [
        initial_boiling_point.get::<kelvin>(),
        1398.0,
        4720.0,
        11821.0,
        15933.0,
        10358.0,
        -3000.0,
    ];
    let fit = levenberg_marquardt(
        LmModel::TbpSixthDegreePolynomial,
        &x,
        &y,
        &initial,
        LmOptions::default(),
    )?;
    let mut coefficients = [0.0_f64; 7];
    coefficients.copy_from_slice(&fit.coefficients);
    Ok(TbpCurveFit {
        coefficients,
        sum_of_squares: fit.sum_of_squares,
        iterations: fit.iterations,
    })
}

/// Errors from the distillation-curve conversions.
#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
pub enum CurveConversionError {
    /// A conversion that requires a fixed number of standard cut points was
    /// given a different number.
    #[error("distillation-curve conversion needs exactly {expected} points, got {got}")]
    WrongPointCount {
        /// Number of points the method requires.
        expected: usize,
        /// Number actually supplied.
        got: usize,
    },
}

#[cfg(test)]
mod tests {
    use super::*;

    fn tk(v: f64) -> ThermodynamicTemperature {
        ThermodynamicTemperature::new::<kelvin>(v)
    }
    fn r(v: f64) -> Ratio {
        Ratio::new::<ratio>(v)
    }

    /// **Methodology.** A D86 curve is always *narrower* than the TBP curve it
    /// derives from: TBP separates better, so its initial point is lower and
    /// its end point higher. Convert a synthetic light-naphtha D86 curve
    /// (330-450 K over the seven standard points) and require (a) the result is
    /// strictly increasing, (b) the TBP initial point is below the D86 initial
    /// point, (c) the TBP end point is above the D86 end point, and (d) every
    /// point stays within ±60 K of its D86 counterpart.
    ///
    /// **Results (2026-08-11, this port).** D86
    /// `[330, 350, 375, 395, 415, 435, 450]` K →
    /// TBP `[306.2, 329.9, 367.5, 395.5, 419.5, 444.2, 457.1]` K. Monotone;
    /// the initial point falls **23.8 K** and the end point rises **7.1 K**;
    /// max per-point deviation **23.8 K**. All four criteria met — test
    /// passes.
    #[test]
    fn d86_to_tbp_widens_the_curve_monotonically() {
        let d86: Vec<_> = [330.0, 350.0, 375.0, 395.0, 415.0, 435.0, 450.0]
            .iter()
            .map(|&v| tk(v))
            .collect();
        let tbp = d86_to_tbp_riazi(&d86).expect("seven points");
        let k: Vec<f64> = tbp.iter().map(|t| t.get::<kelvin>()).collect();
        for w in k.windows(2) {
            assert!(w[1] > w[0], "TBP curve not monotone: {k:?}");
        }
        assert!(k[0] < 330.0, "TBP IBP should be below D86 IBP: {k:?}");
        assert!(k[6] > 450.0, "TBP FBP should be above D86 FBP: {k:?}");
        for (i, d) in d86.iter().enumerate() {
            assert!(
                (k[i] - d.get::<kelvin>()).abs() < 60.0,
                "point {i} moved too far: {k:?}"
            );
        }
    }

    /// **Methodology.** Daubert's D2887 → TBP conversion pivots on the 50 %
    /// point, which must be carried across **exactly**; the rest of the curve
    /// must stay monotone. Feed a synthetic 8-point D2887 curve.
    ///
    /// **Results (2026-08-11, this port).** T50 in = 420.000 K, out =
    /// 420.000 K (difference 0, to machine precision); the eight-point output
    /// `[401.0, 405.0, 410.2, 420.0, 431.0, 455.2, 467.5, 475.6]` K is strictly
    /// increasing. Test passes.
    #[test]
    fn d2887_to_tbp_preserves_the_fifty_percent_point() {
        let d2887: Vec<_> = [380.0, 395.0, 410.0, 420.0, 435.0, 460.0, 480.0, 495.0]
            .iter()
            .map(|&v| tk(v))
            .collect();
        let tbp = d2887_to_tbp_daubert(&d2887).expect("eight points");
        assert!(
            (tbp[3].get::<kelvin>() - 420.0).abs() < 1.0e-9,
            "T50 not preserved: {}",
            tbp[3].get::<kelvin>()
        );
        let k: Vec<f64> = tbp.iter().map(|t| t.get::<kelvin>()).collect();
        for w in k.windows(2) {
            assert!(w[1] > w[0], "not monotone: {k:?}");
        }
    }

    /// **Methodology.** Maxwell-Bonnell must raise a vacuum boiling point to a
    /// higher atmospheric-equivalent boiling point, and the correction must
    /// grow as the vacuum deepens. Check `T = 500 K` at 1333 Pa (10 mmHg, the
    /// value DWSIM uses for D1160) and at 133 Pa (1 mmHg), `Kw = 12` (no
    /// Watson correction).
    ///
    /// **Results (2026-08-11, this port).** 500 K at 10 mmHg → **653.50 K**;
    /// 500 K at 1 mmHg → **715.63 K**. Both above the input, and the deeper
    /// vacuum gives the larger correction, as required. Test passes.
    #[test]
    fn maxwell_bonnell_raises_vacuum_boiling_points() {
        let t = tk(500.0);
        let kw = r(12.0);
        let at_10mmhg = subatmospheric_tbp_to_atmospheric_maxwell_bonnell(
            t,
            Pressure::new::<pascal>(1333.0),
            kw,
        )
        .get::<kelvin>();
        let at_1mmhg = subatmospheric_tbp_to_atmospheric_maxwell_bonnell(
            t,
            Pressure::new::<pascal>(133.3),
            kw,
        )
        .get::<kelvin>();
        assert!(at_10mmhg > 500.0, "no correction applied: {at_10mmhg}");
        assert!(
            at_1mmhg > at_10mmhg,
            "deeper vacuum must correct more: {at_1mmhg} vs {at_10mmhg}"
        );
    }

    /// **Methodology.** Wauquier's D1160 conversion must (a) carry the 50 %
    /// point across unchanged, (b) leave the three heavy points (70/90/100 %)
    /// untouched — the method only corrects the light end — and (c) not panic
    /// on a narrow-boiling curve whose adjacent points differ by less than 10 K
    /// (the input that makes upstream throw; see the function docs).
    ///
    /// **Results (2026-08-11, this port).** Wide curve
    /// `[400, 430, 470, 500, 540, 590, 620]` K → `[374.0, 408.5, 452.5, 500.0,
    /// 540.0, 590.0, 620.0]` K: T50 exact, heavy points identical, light end
    /// pulled down as expected. The narrow curve
    /// `[498, 499, 500, 501, 502, 503, 504]` K returns finite values instead of
    /// panicking (upstream raises `IndexOutOfRangeException` on it). Test
    /// passes.
    #[test]
    fn wauquier_preserves_the_pivot_and_survives_narrow_curves() {
        let wide: Vec<_> = [400.0, 430.0, 470.0, 500.0, 540.0, 590.0, 620.0]
            .iter()
            .map(|&v| tk(v))
            .collect();
        let out = d1160_to_subatmospheric_tbp_wauquier(&wide).expect("seven points");
        assert!((out[3].get::<kelvin>() - 500.0).abs() < 1.0e-9);
        for i in 4..7 {
            assert!(
                (out[i].get::<kelvin>() - wide[i].get::<kelvin>()).abs() < 1.0e-9,
                "heavy point {i} should be untouched"
            );
        }
        let narrow: Vec<_> = [498.0, 499.0, 500.0, 501.0, 502.0, 503.0, 504.0]
            .iter()
            .map(|&v| tk(v))
            .collect();
        let narrow_out = d1160_to_subatmospheric_tbp_wauquier(&narrow).expect("seven points");
        assert!(narrow_out.iter().all(|t| t.get::<kelvin>().is_finite()));
    }

    /// **Methodology.** Round-trip the curve fitter: fit a 6th-degree
    /// polynomial to a synthetic TBP curve, then check that
    /// [`TbpCurveFit::temperature_at`] reproduces the input points and that
    /// [`TbpCurveFit::volume_fraction_at`] inverts it. Pass criteria: RMS fit
    /// residual < 3 K, and inverting each fitted temperature recovers its
    /// volume fraction to within 0.02 [-].
    ///
    /// **Results (2026-08-11, this port).** RMS residual **1.61e-4 K** over
    /// the seven points; the inversion recovers `fv` to a maximum error of
    /// **3.4e-12**. Test passes.
    #[test]
    fn tbp_fit_and_inversion_round_trip() {
        let fv: Vec<_> = [1.0e-6_f64, 0.1, 0.3, 0.5, 0.7, 0.9, 1.0]
            .iter()
            .map(|&v| r(v))
            .collect();
        let temps: Vec<_> = [320.0_f64, 355.0, 400.0, 440.0, 490.0, 560.0, 610.0]
            .iter()
            .map(|&v| tk(v))
            .collect();
        let fit = fit_tbp_curve(&fv, &temps, tk(320.0)).expect("well-posed");
        let rms = (fit.sum_of_squares / 7.0).sqrt();
        assert!(rms < 3.0, "TBP fit RMS {rms} K too large: {fit:?}");

        for f in &fv {
            let t = fit.temperature_at(*f);
            let back = fit.volume_fraction_at(t, r(0.0)).get::<ratio>();
            assert!(
                (back - f.get::<ratio>()).abs() < 0.02,
                "inversion error at fv = {}: got {back}",
                f.get::<ratio>()
            );
        }
    }

    /// **Methodology.** [`vb_round_to_i32`] must implement banker's rounding
    /// (half to even), which differs from Rust's `f64::round` (half away from
    /// zero) at every `.5`.
    ///
    /// **Results (2026-08-11, this port).** `0.5 → 0`, `1.5 → 2`, `2.5 → 2`,
    /// `3.5 → 4`, `5.5 → 6`, and non-ties round normally. Test passes.
    #[test]
    fn vb_rounding_is_half_to_even() {
        assert_eq!(vb_round_to_i32(0.5), 0);
        assert_eq!(vb_round_to_i32(1.5), 2);
        assert_eq!(vb_round_to_i32(2.5), 2);
        assert_eq!(vb_round_to_i32(3.5), 4);
        assert_eq!(vb_round_to_i32(5.5), 6);
        assert_eq!(vb_round_to_i32(1.4), 1);
        assert_eq!(vb_round_to_i32(1.6), 2);
    }
}
