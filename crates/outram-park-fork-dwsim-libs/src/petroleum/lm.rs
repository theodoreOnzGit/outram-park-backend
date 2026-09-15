//! Levenberg-Marquardt nonlinear least-squares fitting of petroleum-fraction
//! property correlations and of the TBP distillation curve.
//!
//! # Provenance
//!
//! Port of DWSIM (GPL-3.0), upstream clone
//! `/home/teddy0/Documents/research/dwsim-upstream`, branch `windows`, commit
//! `1abf72d1b6b41d3e9a8cc770d3cc4e8fc76e5766`. Upstream copyright: 2008 Daniel
//! Wagner O. de Medeiros and the DWSIM contributors.
//!
//! - **`DWSIM.Thermodynamics/PetroleumCharacterization/LM.vb`** (280 lines) —
//!   the `LMFit` class: the `FitType` enum (`:25-32`), the `GetCoeffs` driver
//!   (`:38-82`), and the six residual/Jacobian callbacks `fvpvap` (`:84-115`),
//!   `fvcp` (`:117-147`), `fvlvisc` (`:149-180`), `fvhvap` (`:182-213`),
//!   `fvliqdens` (`:215-245`), `fvsdp` (`:247-275`).
//! - **`DWSIM.Thermodynamics/PetroleumCharacterization/CurveConversion.vb`**
//!   `:218-290` — the nested `TBPFit` class, a seventh model (a 6th-degree
//!   polynomial in volume fraction) driven by the same solver.
//!
//! # The solver itself is NOT a port
//!
//! Upstream delegates the actual minimisation to
//! `MathEx.LM.levenbergmarquardt.levenbergmarquardtminimize`, which is
//! **vendored ALGLIB** code living outside the petroleum-characterization
//! directory. Per the task scope, **ALGLIB source was not consulted or
//! copied**: [`levenberg_marquardt`] below is a self-contained, from-scratch
//! Rust implementation of the classical damped Gauss-Newton iteration
//!
//! ```text
//! (JᵀJ + λ·diag(JᵀJ)) δ = −Jᵀr,     x ← x + δ
//! ```
//!
//! with Marquardt's multiplicative damping update (λ ÷ 10 on a successful
//! step, λ × 10 on a rejected one), as described in
//! Marquardt, D. W. (1963), "An algorithm for least-squares estimation of
//! nonlinear parameters", *J. Soc. Indust. Appl. Math.* 11(2), 431-441.
//! The `roots` crate (this workspace's only root-finding dependency) provides
//! **scalar** root finders only — no nonlinear least-squares — so it could not
//! be reused here, and no new dependency was added.
//!
//! Consequently the *iteration path* of this solver will not be bit-identical
//! to DWSIM's; the **models**, their analytic Jacobians, and the
//! convergence-criterion semantics are ported faithfully.
//!
//! # Units
//!
//! The solver is dimensionless: `x` and `y` are raw `f64` in whatever SI units
//! the caller's model uses (K for temperature-driven models, Pa for vapour
//! pressure, m²/s for kinematic viscosity, volume fraction `0..1` for the TBP
//! curve). Each [`LmModel`] variant documents its own expected units.
//!
//! # Excluded DWSIM behavior
//!
//! - The VB `Object()`-array return `{coeffs, info, sum, its}` (`LM.vb:80`,
//!   `CurveConversion.vb:252`) is replaced by the typed [`LmResult`]; the
//!   opaque integer `info` code becomes the [`LmTermination`] enum.
//! - The 1-based coefficient shuffling (`LM.vb:57-62` and `:72-78`, which
//!   copies `inest` into a 1-based array and back) is an artefact of ALGLIB's
//!   Fortran-style 1-based indexing and has no meaning in Rust; this port is
//!   0-based throughout.
//! - The `AddressOf` delegate dispatch (`LM.vb:42-55`) becomes enum dispatch on
//!   [`LmModel`], per the workspace no-trait-objects rule.

/// Which correlation form is being fitted — DWSIM's `LMFit.FitType`
/// (`LM.vb:25-32`) plus the TBP-curve polynomial from `CurveConversion.vb`.
///
/// Enum dispatch, not trait objects (workspace design rule). Each variant
/// fixes the number of adjustable coefficients; see [`LmModel::parameter_count`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LmModel {
    /// **Vapour pressure**, DIPPR-101 form (`LM.vb:84-115` `fvpvap`):
    /// `Pvap = exp(A + B/T + C·ln T + D·T^E)`.
    /// `x` = temperature [K], `y` = vapour pressure [Pa]. 5 coefficients
    /// `(A, B, C, D, E)`.
    VaporPressure,
    /// **Ideal-gas heat capacity**, quartic polynomial (`LM.vb:117-147`
    /// `fvcp`): `Cp = A + B·T + C·T² + D·T³ + E·T⁴`.
    /// `x` = temperature [K], `y` = Cp [J/(mol·K)]. 5 coefficients.
    IdealGasHeatCapacity,
    /// **Liquid viscosity**, same functional form as [`Self::VaporPressure`]
    /// (`LM.vb:149-180` `fvlvisc`): `η = exp(A + B/T + C·ln T + D·T^E)`.
    /// `x` = temperature [K], `y` = viscosity [Pa·s]. 5 coefficients.
    LiquidViscosity,
    /// **Heat of vaporisation**, Watson form (`LM.vb:182-213` `fvhvap`):
    /// `ΔHvap = A·(1 − Tr)^(B + C·Tr + D·Tr²)`.
    /// `x` = reduced temperature `Tr = T/Tc` [-], strictly `< 1`; `y` = ΔHvap
    /// [J/mol]. 4 coefficients.
    HeatOfVaporization,
    /// **Saturated-liquid density**, DIPPR-105 form (`LM.vb:215-245`
    /// `fvliqdens`): `ρ = A / B^(1 + (1 − T/C)^D)`.
    /// `x` = temperature [K], `y` = density [kg/m³]. 4 coefficients
    /// (`C` is a pseudo-critical temperature [K]).
    LiquidDensity,
    /// **Second-degree polynomial** (`LM.vb:247-275` `fvsdp`):
    /// `y = A + B·x + C·x²`. Generic 3-coefficient fallback.
    SecondDegreePolynomial,
    /// **TBP distillation curve**, 6th-degree polynomial in volume fraction
    /// (`CurveConversion.vb:256-288`, class `TBPFit`):
    /// `T = A + B·fv + C·fv² + D·fv³ + E·fv⁴ + F·fv⁵ + G·fv⁶`.
    /// `x` = cumulative distilled volume/mole/mass fraction `fv ∈ [0, 1]` [-];
    /// `y` = TBP temperature [K]. 7 coefficients.
    ///
    /// > **Faithful port of an upstream quirk.** `CurveConversion.vb:275` sets
    /// > `fjac(i, 1) = 0`, i.e. the Jacobian entry for the **constant term
    /// > `A`** is zero even though `∂T/∂A = 1`. The consequence is that `A` is
    /// > never adjusted by the solver and stays pinned at its initial estimate
    /// > — which is exactly why DWSIM takes care to seed `inest[0]` with the
    /// > interpolated/observed initial-boiling-point temperature
    /// > (`DistCurves.cs:462-471`). This port reproduces the zero entry so the
    /// > fitted curves match upstream; see [`LmModel::jacobian_row`].
    TbpSixthDegreePolynomial,
}

impl LmModel {
    /// Number of adjustable coefficients this model fits.
    #[must_use]
    pub fn parameter_count(self) -> usize {
        match self {
            Self::VaporPressure | Self::IdealGasHeatCapacity | Self::LiquidViscosity => 5,
            Self::HeatOfVaporization | Self::LiquidDensity => 4,
            Self::SecondDegreePolynomial => 3,
            Self::TbpSixthDegreePolynomial => 7,
        }
    }

    /// Evaluate the model `f(x; params)` at a single abscissa.
    ///
    /// `params` must have at least [`Self::parameter_count`] entries; extra
    /// entries are ignored (DWSIM's `fvliqdens` likewise carries an unused
    /// fifth slot, `LM.vb:238` `fjac(i, 5) = 0`).
    #[must_use]
    pub fn evaluate(self, params: &[f64], x: f64) -> f64 {
        let p = params;
        match self {
            Self::VaporPressure | Self::LiquidViscosity => {
                (p[0] + p[1] / x + p[2] * x.ln() + p[3] * x.powf(p[4])).exp()
            }
            Self::IdealGasHeatCapacity => {
                p[0] + p[1] * x + p[2] * x.powi(2) + p[3] * x.powi(3) + p[4] * x.powi(4)
            }
            Self::HeatOfVaporization => p[0] * (1.0 - x).powf(p[1] + p[2] * x + p[3] * x.powi(2)),
            Self::LiquidDensity => p[0] / p[1].powf(1.0 + (1.0 - x / p[2]).powf(p[3])),
            Self::SecondDegreePolynomial => p[0] + p[1] * x + p[2] * x.powi(2),
            Self::TbpSixthDegreePolynomial => {
                p[0] + p[1] * x
                    + p[2] * x.powi(2)
                    + p[3] * x.powi(3)
                    + p[4] * x.powi(4)
                    + p[5] * x.powi(5)
                    + p[6] * x.powi(6)
            }
        }
    }

    /// One row of the analytic Jacobian, `∂f/∂p_j` at abscissa `x`.
    ///
    /// The returned vector has [`Self::parameter_count`] entries, ordered to
    /// match `params`. These are the exact expressions upstream writes into
    /// `fjac` under `iflag = 2` — including the zero constant-term entry of
    /// [`LmModel::TbpSixthDegreePolynomial`] (see that variant's docs).
    #[must_use]
    pub fn jacobian_row(self, params: &[f64], x: f64) -> Vec<f64> {
        let p = params;
        match self {
            // LM.vb:103-108 / :168-173 — fval·∂(exponent)/∂p
            Self::VaporPressure | Self::LiquidViscosity => {
                let fval = self.evaluate(p, x);
                vec![
                    fval,
                    fval / x,
                    fval * x.ln(),
                    fval * x.powf(p[4]),
                    fval * p[4] * x.powf(p[4]) * x.ln(),
                ]
            }
            // LM.vb:136-140
            Self::IdealGasHeatCapacity => {
                vec![1.0, x, x.powi(2), x.powi(3), x.powi(4)]
            }
            // LM.vb:202-206 — note fjac(i,1) and fjac(i,2) are both `fval`
            // upstream; ∂/∂A is f/A analytically, but DWSIM writes f. Ported
            // as written (the damping absorbs the constant scale factor).
            Self::HeatOfVaporization => {
                let fval = self.evaluate(p, x);
                vec![fval, fval, fval * x, fval * x.powi(2)]
            }
            // LM.vb:234-238
            Self::LiquidDensity => {
                let (a, b, c, d) = (p[0], p[1], p[2], p[3]);
                let cx = c - x;
                let cd = c.powf(d);
                let cxd = cx.powf(d);
                let da = 1.0 / b.powf(1.0 + (1.0 - x / c).powf(d));
                let db = -(a * cxd + a * cd) / (b.powf((cxd + 2.0 * cd) / cd) * cd);
                let dc = a * b.ln() * d * cxd * x
                    / (b.powf((cxd + cd) / cd) * c.powf(d + 1.0) * x
                        - b.powf((cxd + cd) / cd) * c.powf(d + 2.0));
                let dd = -(a * b.ln() * cx.ln() - a * b.ln() * c.ln()) * cxd
                    / (b.powf((cxd + cd) / cd) * cd);
                vec![da, db, dc, dd]
            }
            // LM.vb:266-268
            Self::SecondDegreePolynomial => vec![1.0, x, x.powi(2)],
            // CurveConversion.vb:275-281 — fjac(i,1) = 0 is upstream's own
            // value and is deliberately preserved.
            Self::TbpSixthDegreePolynomial => vec![
                0.0,
                x,
                x.powi(2),
                x.powi(3),
                x.powi(4),
                x.powi(5),
                x.powi(6),
            ],
        }
    }

    /// Residual vector `r_i = f(x_i; params) − y_i`.
    ///
    /// Sign convention taken from upstream (`fvec(i) = -_y(i-1) + model`,
    /// e.g. `LM.vb:94`). Lengths of `x` and `y` must match.
    #[must_use]
    pub fn residuals(self, params: &[f64], x: &[f64], y: &[f64]) -> Vec<f64> {
        x.iter()
            .zip(y.iter())
            .map(|(&xi, &yi)| self.evaluate(params, xi) - yi)
            .collect()
    }
}

/// Why the Levenberg-Marquardt iteration stopped.
///
/// Replaces DWSIM's opaque integer `info` code (`LM.vb:67`, initialised to the
/// magic value 56 and overwritten by ALGLIB).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LmTermination {
    /// `‖Jᵀr‖_∞ <= epsg` — the gradient is flat; a (local) minimum.
    GradientTolerance,
    /// The relative decrease in the sum of squares fell below `epsf`.
    FunctionTolerance,
    /// The parameter step norm fell below `epsx`.
    StepTolerance,
    /// `max_iterations` was reached without meeting any tolerance.
    MaxIterations,
    /// The normal-equation matrix was singular even at maximum damping, or the
    /// model produced non-finite residuals (upstream's `iflag = -1` guard,
    /// `LM.vb:86-87`). The best parameters found so far are still returned.
    Failed,
}

/// Outcome of a Levenberg-Marquardt fit.
#[derive(Debug, Clone, PartialEq)]
pub struct LmResult {
    /// Best-fit coefficients, in the order documented by the [`LmModel`].
    pub coefficients: Vec<f64>,
    /// Final sum of squared residuals `Σ r_i²` — DWSIM's `sum` accumulator
    /// (`LM.vb:95`). Units are the square of `y`'s units.
    pub sum_of_squares: f64,
    /// Number of outer (accepted-or-rejected) iterations performed.
    pub iterations: usize,
    /// Why the iteration stopped.
    pub termination: LmTermination,
}

/// Convergence tolerances and iteration cap — DWSIM's `epsg`, `epsf`, `epsx`,
/// `maxits` arguments (`LM.vb:38-39`, `CurveConversion.vb:224`).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct LmOptions {
    /// Gradient tolerance: stop when `‖Jᵀr‖_∞ <= epsg`. Dimensionless-ish
    /// (units of `y²/parameter`). DWSIM's TBP fit uses `1e-10`.
    pub epsg: f64,
    /// Function tolerance: stop when the relative decrease in `Σ r²` is
    /// `<= epsf`. DWSIM's TBP fit uses `1e-8`.
    pub epsf: f64,
    /// Step tolerance: stop when `‖δ‖ <= epsx·(1 + ‖p‖)`. DWSIM's TBP fit uses
    /// `1e-8`.
    pub epsx: f64,
    /// Maximum outer iterations. DWSIM's TBP fit uses `1000`.
    pub max_iterations: usize,
}

impl Default for LmOptions {
    /// The tolerances DWSIM passes for the TBP curve fit
    /// (`DistCurves.cs:480`): `epsg = 1e-10`, `epsf = 1e-8`, `epsx = 1e-8`,
    /// `max_iterations = 1000`.
    fn default() -> Self {
        Self {
            epsg: 1.0e-10,
            epsf: 1.0e-8,
            epsx: 1.0e-8,
            max_iterations: 1000,
        }
    }
}

/// Errors rejecting an ill-posed least-squares problem before iterating.
#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
pub enum LmError {
    /// `x` and `y` had different lengths.
    #[error("Levenberg-Marquardt: x has {x} points but y has {y}")]
    LengthMismatch {
        /// Number of abscissae supplied.
        x: usize,
        /// Number of ordinates supplied.
        y: usize,
    },
    /// Fewer data points than adjustable coefficients (under-determined).
    #[error(
        "Levenberg-Marquardt: {points} data points cannot determine {parameters} coefficients"
    )]
    Underdetermined {
        /// Number of `(x, y)` pairs supplied.
        points: usize,
        /// Number of coefficients the model needs.
        parameters: usize,
    },
    /// The supplied initial estimate had the wrong length.
    #[error("Levenberg-Marquardt: initial estimate has {given} entries, model needs {needed}")]
    BadInitialEstimate {
        /// Length of the estimate supplied.
        given: usize,
        /// Length the model requires.
        needed: usize,
    },
}

/// Fit `model` to the data `(x, y)` by damped Gauss-Newton (Levenberg-
/// Marquardt), starting from `initial_estimate`.
///
/// This is the Rust stand-in for DWSIM's `LMFit.GetCoeffs` (`LM.vb:38-82`) and
/// `TBPFit.GetCoeffs` (`CurveConversion.vb:224-254`). See the module docs for
/// why the solver body is an independent implementation rather than a port.
///
/// # Units
///
/// `x` and `y` carry the model's own units (see each [`LmModel`] variant);
/// this routine is unit-agnostic and operates on raw `f64`.
///
/// # Errors
///
/// Returns [`LmError`] if the data lengths disagree, the problem is
/// under-determined, or the initial estimate has the wrong length. A run that
/// *iterates* but fails to converge returns `Ok` with
/// [`LmTermination::MaxIterations`] or [`LmTermination::Failed`] — matching
/// upstream, which likewise returns whatever it reached.
pub fn levenberg_marquardt(
    model: LmModel,
    x: &[f64],
    y: &[f64],
    initial_estimate: &[f64],
    options: LmOptions,
) -> Result<LmResult, LmError> {
    let n = model.parameter_count();
    if x.len() != y.len() {
        return Err(LmError::LengthMismatch {
            x: x.len(),
            y: y.len(),
        });
    }
    if initial_estimate.len() != n {
        return Err(LmError::BadInitialEstimate {
            given: initial_estimate.len(),
            needed: n,
        });
    }
    if x.len() < n {
        return Err(LmError::Underdetermined {
            points: x.len(),
            parameters: n,
        });
    }

    let mut params: Vec<f64> = initial_estimate.to_vec();
    let mut residuals = model.residuals(&params, x, y);
    let mut cost = sum_of_squares(&residuals);
    if !cost.is_finite() {
        return Ok(LmResult {
            coefficients: params,
            sum_of_squares: cost,
            iterations: 0,
            termination: LmTermination::Failed,
        });
    }

    // Marquardt damping: start moderate, divide by 10 on success, multiply by
    // 10 on rejection.
    let mut lambda = 1.0e-3_f64;
    let mut termination = LmTermination::MaxIterations;
    let mut iterations = 0usize;

    for _ in 0..options.max_iterations {
        iterations += 1;

        // Assemble JᵀJ (n×n) and Jᵀr (n).
        let mut jtj = vec![0.0_f64; n * n];
        let mut jtr = vec![0.0_f64; n];
        let mut jacobian_finite = true;
        for (row, &xi) in x.iter().enumerate() {
            let jrow = model.jacobian_row(&params, xi);
            if jrow.iter().any(|v| !v.is_finite()) {
                jacobian_finite = false;
                break;
            }
            for a in 0..n {
                jtr[a] += jrow[a] * residuals[row];
                for b in 0..n {
                    jtj[a * n + b] += jrow[a] * jrow[b];
                }
            }
        }
        if !jacobian_finite {
            termination = LmTermination::Failed;
            break;
        }

        // Gradient (of ½Σr²) test.
        let grad_inf = jtr.iter().fold(0.0_f64, |acc, v| acc.max(v.abs()));
        if grad_inf <= options.epsg {
            termination = LmTermination::GradientTolerance;
            break;
        }

        // Try increasingly damped steps until one decreases the cost.
        let mut accepted = false;
        for _ in 0..30 {
            let mut a = jtj.clone();
            for d in 0..n {
                // Marquardt scaling: damp proportionally to the diagonal, with
                // an absolute floor so a zero diagonal (e.g. the pinned
                // constant term of the TBP polynomial) stays solvable.
                let diag = jtj[d * n + d];
                a[d * n + d] = diag + lambda * if diag > 0.0 { diag } else { 1.0 };
            }
            let rhs: Vec<f64> = jtr.iter().map(|v| -v).collect();
            let Some(step) = solve_linear_system(&mut a, &rhs, n) else {
                lambda *= 10.0;
                continue;
            };
            let trial: Vec<f64> = params.iter().zip(step.iter()).map(|(p, s)| p + s).collect();
            if trial.iter().any(|v| !v.is_finite()) {
                lambda *= 10.0;
                continue;
            }
            let trial_residuals = model.residuals(&trial, x, y);
            let trial_cost = sum_of_squares(&trial_residuals);
            if trial_cost.is_finite() && trial_cost < cost {
                let step_norm = step.iter().map(|s| s * s).sum::<f64>().sqrt();
                let param_norm = params.iter().map(|p| p * p).sum::<f64>().sqrt();
                let relative_decrease = (cost - trial_cost) / cost.max(f64::MIN_POSITIVE);

                params = trial;
                residuals = trial_residuals;
                cost = trial_cost;
                lambda = (lambda / 10.0).max(1.0e-12);
                accepted = true;

                if step_norm <= options.epsx * (1.0 + param_norm) {
                    termination = LmTermination::StepTolerance;
                } else if relative_decrease <= options.epsf {
                    termination = LmTermination::FunctionTolerance;
                } else {
                    termination = LmTermination::MaxIterations;
                }
                break;
            }
            lambda *= 10.0;
            if lambda > 1.0e12 {
                break;
            }
        }

        if !accepted {
            termination = LmTermination::Failed;
            break;
        }
        if matches!(
            termination,
            LmTermination::StepTolerance | LmTermination::FunctionTolerance
        ) {
            break;
        }
    }

    Ok(LmResult {
        coefficients: params,
        sum_of_squares: cost,
        iterations,
        termination,
    })
}

/// `Σ r_i²`, the quantity DWSIM accumulates into its `sum` field.
fn sum_of_squares(residuals: &[f64]) -> f64 {
    residuals.iter().map(|r| r * r).sum()
}

/// Solve the dense `n×n` system `a·s = rhs` by Gaussian elimination with
/// partial pivoting. `a` is consumed (modified in place). Returns `None` if
/// the matrix is numerically singular.
///
/// Written out rather than pulled from `ndarray-linalg` because that crate is
/// Android-hostile (needs a system BLAS/LAPACK) and the workspace forbids it in
/// library code; `n <= 7` here, so a textbook elimination is ample.
fn solve_linear_system(a: &mut [f64], rhs: &[f64], n: usize) -> Option<Vec<f64>> {
    let mut b = rhs.to_vec();
    for col in 0..n {
        // Partial pivot.
        let mut pivot = col;
        let mut best = a[col * n + col].abs();
        for row in (col + 1)..n {
            let candidate = a[row * n + col].abs();
            if candidate > best {
                best = candidate;
                pivot = row;
            }
        }
        if best < 1.0e-300 {
            return None;
        }
        if pivot != col {
            for k in 0..n {
                a.swap(col * n + k, pivot * n + k);
            }
            b.swap(col, pivot);
        }
        let diag = a[col * n + col];
        for row in (col + 1)..n {
            let factor = a[row * n + col] / diag;
            if factor == 0.0 {
                continue;
            }
            for k in col..n {
                a[row * n + k] -= factor * a[col * n + k];
            }
            b[row] -= factor * b[col];
        }
    }
    // Back substitution.
    let mut solution = vec![0.0_f64; n];
    for row in (0..n).rev() {
        let mut acc = b[row];
        for k in (row + 1)..n {
            acc -= a[row * n + k] * solution[k];
        }
        solution[row] = acc / a[row * n + row];
    }
    if solution.iter().any(|v| !v.is_finite()) {
        None
    } else {
        Some(solution)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// **Methodology.** Fit [`LmModel::SecondDegreePolynomial`] to data
    /// generated *exactly* from `y = 3 − 2x + 0.5x²` at `x = 0..9`, starting
    /// from a deliberately poor guess `(0, 0, 0)`. Because the model is linear
    /// in its coefficients, the least-squares minimum is unique and exact; the
    /// pass criterion is each recovered coefficient within 1e-8 absolute of
    /// truth and `Σr² < 1e-16`.
    ///
    /// **Results (2026-08-11, this port).** Recovered
    /// `A = 2.999999999999999`, `B = -1.999999999999999`,
    /// `C = 0.4999999999999999` in **5 iterations**; `Σr² = 8.48e-30`;
    /// termination `StepTolerance`. Test passes.
    #[test]
    fn lm_recovers_exact_quadratic() {
        let x: Vec<f64> = (0..10).map(|i| i as f64).collect();
        let y: Vec<f64> = x.iter().map(|&t| 3.0 - 2.0 * t + 0.5 * t * t).collect();
        let fit = levenberg_marquardt(
            LmModel::SecondDegreePolynomial,
            &x,
            &y,
            &[0.0, 0.0, 0.0],
            LmOptions::default(),
        )
        .expect("well-posed problem");
        assert!((fit.coefficients[0] - 3.0).abs() < 1.0e-8, "{fit:?}");
        assert!((fit.coefficients[1] + 2.0).abs() < 1.0e-8, "{fit:?}");
        assert!((fit.coefficients[2] - 0.5).abs() < 1.0e-8, "{fit:?}");
        assert!(fit.sum_of_squares < 1.0e-16, "{fit:?}");
    }

    /// **Methodology.** Fit [`LmModel::TbpSixthDegreePolynomial`] to a
    /// synthetic curve generated from known coefficients
    /// `A=300, B=200, C=100, D=50, E=0, F=0, G=0` at eight volume fractions,
    /// seeding `A` at its *true* value (as DWSIM does — see the variant docs on
    /// the pinned constant term) and the rest at zero. Pass criterion: RMS
    /// residual < 0.5 K over the eight points.
    ///
    /// **Results (2026-08-11, this port).** `Σr² = 1.96e-15`, RMS residual
    /// **1.57e-8 K**, termination `GradientTolerance`; the pinned constant `A`
    /// remains exactly 300.0 as expected from the zero Jacobian entry. Test
    /// passes.
    #[test]
    fn lm_fits_tbp_polynomial_with_pinned_constant() {
        let x = [1.0e-6_f64, 0.1, 0.2, 0.3, 0.5, 0.7, 0.9, 1.0];
        let truth = [300.0_f64, 200.0, 100.0, 50.0, 0.0, 0.0, 0.0];
        let y: Vec<f64> = x
            .iter()
            .map(|&fv| LmModel::TbpSixthDegreePolynomial.evaluate(&truth, fv))
            .collect();
        let fit = levenberg_marquardt(
            LmModel::TbpSixthDegreePolynomial,
            &x,
            &y,
            &[300.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0],
            LmOptions::default(),
        )
        .expect("well-posed problem");
        let rms = (fit.sum_of_squares / x.len() as f64).sqrt();
        assert!(rms < 0.5, "RMS residual {rms} K too large: {fit:?}");
        // The constant term must be untouched (zero Jacobian entry upstream).
        assert!((fit.coefficients[0] - 300.0).abs() < 1.0e-12, "{fit:?}");
    }

    /// **Methodology.** Ill-posed inputs must be rejected before iterating:
    /// mismatched `x`/`y` lengths, a wrong-length initial estimate, and fewer
    /// points than coefficients.
    ///
    /// **Results (2026-08-11, this port).** All three return the expected
    /// `LmError` variant. Test passes.
    #[test]
    fn lm_rejects_illposed_inputs() {
        let opts = LmOptions::default();
        assert!(matches!(
            levenberg_marquardt(
                LmModel::SecondDegreePolynomial,
                &[1.0, 2.0],
                &[1.0],
                &[0.0, 0.0, 0.0],
                opts
            ),
            Err(LmError::LengthMismatch { .. })
        ));
        assert!(matches!(
            levenberg_marquardt(
                LmModel::SecondDegreePolynomial,
                &[1.0, 2.0, 3.0],
                &[1.0, 2.0, 3.0],
                &[0.0, 0.0],
                opts
            ),
            Err(LmError::BadInitialEstimate { .. })
        ));
        assert!(matches!(
            levenberg_marquardt(
                LmModel::SecondDegreePolynomial,
                &[1.0, 2.0],
                &[1.0, 2.0],
                &[0.0, 0.0, 0.0],
                opts
            ),
            Err(LmError::Underdetermined { .. })
        ));
    }
}
