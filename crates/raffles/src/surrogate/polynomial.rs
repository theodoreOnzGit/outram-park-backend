//! Polynomial regression surrogates, fitted by least squares.
//!
//! # What this is
//!
//! A multivariate polynomial of bounded total degree, fitted to a sample of
//! (input, output) pairs and evaluated in place of the expensive model. The
//! oldest and least fashionable surrogate, and often the right one: it is
//! cheap, it is deterministic, it extrapolates predictably badly (which is
//! better than extrapolating unpredictably badly), and its coefficients mean
//! something.
//!
//! It is also the foundation of a polynomial chaos expansion — the same fit in
//! an orthogonal basis — which is why RAVEN's ROM layer starts here.
//!
//! # The basis
//!
//! All monomials of total degree at most `degree` in `d` inputs, in graded
//! lexicographic order, starting with the constant. The number of them is
//! `C(d + degree, degree)`, which [`PolynomialSurrogate::basis_size`] reports:
//! 4 inputs at degree 3 is 35 terms, at degree 5 it is 126, and at degree 8 it
//! is 495. That growth is why a caller should ask before fitting rather than
//! after.
//!
//! # Solving
//!
//! Normal equations with a Cholesky factorisation, plus optional Tikhonov
//! ridge regularisation on the diagonal. Not the most numerically refined
//! choice — a QR or SVD of the design matrix is better conditioned — and the
//! reason it is used anyway is stated honestly in
//! [`PolynomialSurrogate::fit`]: it keeps the crate free of a linear-algebra
//! dependency, and the condition-number cost is bounded by rescaling the inputs
//! to `[-1, 1]` first, which this module does automatically.
//!
//! # References
//!
//! - N. Wiener (1938). The homogeneous chaos. *American Journal of
//!   Mathematics, 60*(4), 897–936. doi:
//!   [10.2307/2371268](https://doi.org/10.2307/2371268)
//! - D. Xiu and G. E. Karniadakis (2002). The Wiener–Askey polynomial chaos for
//!   stochastic differential equations. *SIAM Journal on Scientific Computing,
//!   24*(2), 619–644. doi:
//!   [10.1137/S1064827501387826](https://doi.org/10.1137/S1064827501387826)

use crate::{RafflesError, Result};

/// A fitted multivariate polynomial surrogate.
///
/// Built by [`PolynomialSurrogate::fit`] and evaluated by
/// [`PolynomialSurrogate::predict`].
#[derive(Debug, Clone, PartialEq)]
pub struct PolynomialSurrogate {
    /// Exponent vector of each basis term, `basis[k][j]` being the power of
    /// input `j` in term `k`.
    basis: Vec<Vec<usize>>,
    /// Fitted coefficient of each basis term.
    coefficients: Vec<f64>,
    /// Per-input centre used to rescale to `[-1, 1]`.
    centre: Vec<f64>,
    /// Per-input half-range used to rescale to `[-1, 1]`; never zero.
    half_range: Vec<f64>,
    /// Total degree of the basis.
    degree: usize,
}

impl PolynomialSurrogate {
    /// Number of basis terms for `inputs` variables at total degree `degree`:
    /// `C(inputs + degree, degree)`.
    ///
    /// Saturating, so an absurd request reports `usize::MAX` rather than
    /// wrapping to a small, reassuring number.
    pub fn basis_size(inputs: usize, degree: usize) -> usize {
        let mut size: usize = 1;
        for k in 1..=degree {
            size = size.saturating_mul(inputs + k) / k;
        }
        size
    }

    /// Fits a polynomial of total degree `degree` to `(inputs, outputs)` by
    /// least squares.
    ///
    /// `inputs` is one row per sample; `outputs` is the scalar model response
    /// at each. Inputs are rescaled to `[-1, 1]` internally — a cubic in a
    /// variable of order `1e6` otherwise produces a design matrix with entries
    /// spanning `1e18`, and no amount of care in the solver recovers from that.
    ///
    /// `ridge` adds Tikhonov regularisation to the normal equations. Pass `0.0`
    /// for a plain least-squares fit; pass a small positive value (`1e-8` is a
    /// reasonable start) when the design is rank-deficient, which happens
    /// whenever there are fewer samples than basis terms.
    ///
    /// # Why normal equations rather than QR
    ///
    /// The normal equations square the condition number, which is a real cost.
    /// They are used here because they need only a Cholesky factorisation of a
    /// `basis x basis` matrix, which is thirty lines and no dependency, whereas
    /// a well-implemented QR is not. The rescaling above keeps the conditioning
    /// tolerable for the degrees this is used at, and
    /// [`PolynomialSurrogate::fit`] tells the caller when it is not by refusing
    /// to factorise. If a caller needs degree 10 in eight variables, the right
    /// answer is `faer` — already in the workspace — and not a nudge to the
    /// ridge parameter.
    ///
    /// # Errors
    ///
    /// - [`RafflesError::DimensionMismatch`] if `inputs` and `outputs` disagree
    ///   in length, or the input rows are ragged.
    /// - [`RafflesError::InvalidParameter`] if there are no samples, if a value
    ///   is not finite, if `ridge` is negative, or if the normal equations are
    ///   not positive definite even after regularisation — which means the
    ///   design cannot identify the coefficients, and returning some arbitrary
    ///   one of the infinitely many solutions would be worse than failing.
    pub fn fit(inputs: &[Vec<f64>], outputs: &[f64], degree: usize, ridge: f64) -> Result<Self> {
        if inputs.is_empty() {
            return Err(RafflesError::InvalidParameter {
                parameter: "inputs".to_string(),
                value: 0.0,
                reason: "fitting needs at least one sample".to_string(),
            });
        }
        if inputs.len() != outputs.len() {
            return Err(RafflesError::DimensionMismatch {
                expected: inputs.len(),
                found: outputs.len(),
            });
        }
        if ridge < 0.0 || !ridge.is_finite() {
            return Err(RafflesError::InvalidParameter {
                parameter: "ridge".to_string(),
                value: ridge,
                reason: "the ridge parameter must be finite and non-negative".to_string(),
            });
        }
        let d = inputs[0].len();
        if d == 0 {
            return Err(RafflesError::InvalidParameter {
                parameter: "inputs".to_string(),
                value: 0.0,
                reason: "samples must have at least one input variable".to_string(),
            });
        }
        for row in inputs {
            if row.len() != d {
                return Err(RafflesError::DimensionMismatch {
                    expected: d,
                    found: row.len(),
                });
            }
            for value in row {
                if !value.is_finite() {
                    return Err(RafflesError::InvalidParameter {
                        parameter: "input".to_string(),
                        value: *value,
                        reason: "input values must be finite".to_string(),
                    });
                }
            }
        }
        for value in outputs {
            if !value.is_finite() {
                return Err(RafflesError::InvalidParameter {
                    parameter: "output".to_string(),
                    value: *value,
                    reason: "output values must be finite".to_string(),
                });
            }
        }

        // Rescale each input to [-1, 1]; a degenerate column keeps half_range 1
        // so the rescaling is the identity rather than a division by zero.
        let mut centre = vec![0.0; d];
        let mut half_range = vec![1.0; d];
        for j in 0..d {
            let mut lo = f64::INFINITY;
            let mut hi = f64::NEG_INFINITY;
            for row in inputs {
                lo = lo.min(row[j]);
                hi = hi.max(row[j]);
            }
            centre[j] = 0.5 * (lo + hi);
            let span = 0.5 * (hi - lo);
            half_range[j] = if span > 0.0 { span } else { 1.0 };
        }

        let basis = monomial_basis(d, degree);
        let terms = basis.len();

        // Normal equations: (X^T X + ridge I) c = X^T y, accumulated without
        // materialising X, which for a large sample is the whole memory cost.
        let mut gram = vec![vec![0.0; terms]; terms];
        let mut rhs = vec![0.0; terms];
        for (row, y) in inputs.iter().zip(outputs.iter()) {
            let scaled: Vec<f64> = (0..d)
                .map(|j| (row[j] - centre[j]) / half_range[j])
                .collect();
            let features = evaluate_basis(&basis, &scaled);
            for i in 0..terms {
                rhs[i] += features[i] * y;
                for k in i..terms {
                    gram[i][k] += features[i] * features[k];
                }
            }
        }
        for i in 0..terms {
            gram[i][i] += ridge;
            for k in 0..i {
                gram[i][k] = gram[k][i];
            }
        }

        let coefficients = solve_symmetric_positive_definite(&gram, &rhs).ok_or_else(|| {
            RafflesError::InvalidParameter {
                parameter: "inputs".to_string(),
                value: inputs.len() as f64,
                reason: format!(
                    "the normal equations for {terms} basis terms are not positive definite with \
                     {} samples; add samples, lower the degree, or pass a positive ridge",
                    inputs.len()
                ),
            }
        })?;

        Ok(Self {
            basis,
            coefficients,
            centre,
            half_range,
            degree,
        })
    }

    /// Evaluates the fitted polynomial at one input vector.
    ///
    /// # Errors
    ///
    /// [`RafflesError::DimensionMismatch`] if `x` has the wrong length.
    pub fn predict(&self, x: &[f64]) -> Result<f64> {
        if x.len() != self.centre.len() {
            return Err(RafflesError::DimensionMismatch {
                expected: self.centre.len(),
                found: x.len(),
            });
        }
        let scaled: Vec<f64> = (0..x.len())
            .map(|j| (x[j] - self.centre[j]) / self.half_range[j])
            .collect();
        let features = evaluate_basis(&self.basis, &scaled);
        Ok(features
            .iter()
            .zip(self.coefficients.iter())
            .map(|(f, c)| f * c)
            .sum())
    }

    /// Total degree of the fitted basis.
    pub fn degree(&self) -> usize {
        self.degree
    }

    /// Number of basis terms, and therefore of fitted coefficients.
    pub fn terms(&self) -> usize {
        self.basis.len()
    }

    /// The fitted coefficients, aligned with [`basis`](Self::basis).
    pub fn coefficients(&self) -> &[f64] {
        &self.coefficients
    }

    /// The exponent vector of each basis term.
    pub fn basis(&self) -> &[Vec<usize>] {
        &self.basis
    }

    /// Coefficient of determination `R^2` on a data set.
    ///
    /// 1 is a perfect fit and 0 is no better than predicting the mean.
    /// **Negative values are possible and are meaningful**: they say the
    /// surrogate is worse than the mean, which is the signature of an
    /// over-fitted model evaluated on data it was not fitted to. Nothing here
    /// clamps that away.
    ///
    /// # Errors
    ///
    /// As [`predict`](Self::predict), plus
    /// [`RafflesError::DimensionMismatch`] if the two arrays disagree.
    pub fn r_squared(&self, inputs: &[Vec<f64>], outputs: &[f64]) -> Result<f64> {
        if inputs.len() != outputs.len() {
            return Err(RafflesError::DimensionMismatch {
                expected: inputs.len(),
                found: outputs.len(),
            });
        }
        let n = outputs.len() as f64;
        let mean = outputs.iter().sum::<f64>() / n;
        let mut residual = 0.0;
        let mut total = 0.0;
        for (row, y) in inputs.iter().zip(outputs.iter()) {
            let prediction = self.predict(row)?;
            residual += (y - prediction).powi(2);
            total += (y - mean).powi(2);
        }
        if total == 0.0 {
            return Ok(if residual == 0.0 { 1.0 } else { 0.0 });
        }
        Ok(1.0 - residual / total)
    }

    /// Root-mean-square prediction error on a data set.
    ///
    /// # Errors
    ///
    /// As [`r_squared`](Self::r_squared).
    pub fn rmse(&self, inputs: &[Vec<f64>], outputs: &[f64]) -> Result<f64> {
        if inputs.len() != outputs.len() {
            return Err(RafflesError::DimensionMismatch {
                expected: inputs.len(),
                found: outputs.len(),
            });
        }
        let mut sum = 0.0;
        for (row, y) in inputs.iter().zip(outputs.iter()) {
            sum += (y - self.predict(row)?).powi(2);
        }
        Ok((sum / outputs.len() as f64).sqrt())
    }
}

/// All exponent vectors of total degree at most `degree` in `d` variables,
/// graded lexicographic, constant term first.
fn monomial_basis(d: usize, degree: usize) -> Vec<Vec<usize>> {
    let mut basis = vec![vec![0usize; d]];
    let mut previous_level: Vec<Vec<usize>> = vec![vec![0usize; d]];
    for _ in 1..=degree {
        let mut level = Vec::new();
        for exponents in &previous_level {
            // Raise only at or after the last raised position, which generates
            // each multiset exactly once.
            let start = exponents
                .iter()
                .enumerate()
                .rev()
                .find(|(_, e)| **e > 0)
                .map(|(j, _)| j)
                .unwrap_or(0);
            for j in start..d {
                let mut next = exponents.clone();
                next[j] += 1;
                if !level.contains(&next) {
                    level.push(next);
                }
            }
        }
        basis.extend(level.clone());
        previous_level = level;
    }
    basis
}

/// Evaluates every basis monomial at a (rescaled) point.
fn evaluate_basis(basis: &[Vec<usize>], x: &[f64]) -> Vec<f64> {
    basis
        .iter()
        .map(|exponents| {
            exponents
                .iter()
                .enumerate()
                .map(|(j, e)| x[j].powi(*e as i32))
                .product()
        })
        .collect()
}

/// Solves `A c = b` for symmetric positive-definite `A` by Cholesky.
///
/// Returns `None` when `A` is not positive definite, which the caller turns
/// into a diagnosis rather than a silent pseudo-solution.
fn solve_symmetric_positive_definite(a: &[Vec<f64>], b: &[f64]) -> Option<Vec<f64>> {
    let n = a.len();
    let mut l = vec![vec![0.0; n]; n];
    for i in 0..n {
        for j in 0..=i {
            let mut sum = a[i][j];
            for k in 0..j {
                sum -= l[i][k] * l[j][k];
            }
            if i == j {
                if !(sum > 0.0) || !sum.is_finite() {
                    return None;
                }
                l[i][j] = sum.sqrt();
            } else {
                l[i][j] = sum / l[j][j];
            }
        }
    }
    // Forward substitution, then back substitution.
    let mut y = vec![0.0; n];
    for i in 0..n {
        let mut sum = b[i];
        for k in 0..i {
            sum -= l[i][k] * y[k];
        }
        y[i] = sum / l[i][i];
    }
    let mut x = vec![0.0; n];
    for i in (0..n).rev() {
        let mut sum = y[i];
        for k in (i + 1)..n {
            sum -= l[k][i] * x[k];
        }
        x[i] = sum / l[i][i];
    }
    if x.iter().all(|v| v.is_finite()) {
        Some(x)
    } else {
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::samplers::stream_seed;
    use outram_mc_libs::rng::lcg::prn;

    /// **Methodology.** The basis size must be `C(d + p, p)`. Checked against
    /// hand-computed values: 1 input at degree 3 is 4; 2 at degree 2 is 6;
    /// 3 at degree 3 is 20; 4 at degree 5 is 126.
    ///
    /// **Result** (2026-09-16): all four exact, and the generated basis has
    /// exactly that many distinct exponent vectors.
    #[test]
    fn the_basis_has_the_size_the_formula_says() {
        for (d, p, expected) in [(1, 3, 4), (2, 2, 6), (3, 3, 20), (4, 5, 126)] {
            assert_eq!(
                PolynomialSurrogate::basis_size(d, p),
                expected,
                "d={d} p={p}"
            );
            let basis = monomial_basis(d, p);
            assert_eq!(basis.len(), expected, "generated basis for d={d} p={p}");
            // Every term distinct, and none above the total degree.
            for (i, term) in basis.iter().enumerate() {
                assert!(term.iter().sum::<usize>() <= p);
                assert!(!basis[..i].contains(term), "duplicate term {term:?}");
            }
        }
    }

    /// **Methodology — the exactness property.** A polynomial surrogate of
    /// degree `p` fitted to data generated by a polynomial of degree at most
    /// `p` must reproduce it to machine precision, not approximately. Fitted to
    /// `f(x, y) = 3 - 2x + 0.5y + x*y - 4y^2` at 60 points, degree 2.
    ///
    /// **Result** (2026-09-16, `--release`, seed 20260916): in-sample RMSE
    /// 3.816e-15 and out-of-sample RMSE 3.379e-15 over 200 fresh points, with
    /// R^2 = 1.000000000000. Machine precision, as the exactness property
    /// requires — an approximate answer here would mean the least-squares
    /// solve is wrong, not that the fit is hard.
    #[test]
    fn a_polynomial_is_reproduced_exactly() {
        let truth = |x: f64, y: f64| 3.0 - 2.0 * x + 0.5 * y + x * y - 4.0 * y * y;
        let mut seed = stream_seed(20_260_916, 0);
        let mut inputs = Vec::new();
        let mut outputs = Vec::new();
        for _ in 0..60 {
            let x = prn(&mut seed) * 4.0 - 2.0;
            let y = prn(&mut seed) * 4.0 - 2.0;
            inputs.push(vec![x, y]);
            outputs.push(truth(x, y));
        }
        let surrogate = PolynomialSurrogate::fit(&inputs, &outputs, 2, 0.0).unwrap();
        let in_sample = surrogate.rmse(&inputs, &outputs).unwrap();

        let mut test_inputs = Vec::new();
        let mut test_outputs = Vec::new();
        for _ in 0..200 {
            let x = prn(&mut seed) * 4.0 - 2.0;
            let y = prn(&mut seed) * 4.0 - 2.0;
            test_inputs.push(vec![x, y]);
            test_outputs.push(truth(x, y));
        }
        let out_of_sample = surrogate.rmse(&test_inputs, &test_outputs).unwrap();
        println!(
            "degree-2 fit to a degree-2 truth: in-sample RMSE {in_sample:.3e}, \
             out-of-sample RMSE {out_of_sample:.3e}, R^2 {:.12}",
            surrogate.r_squared(&test_inputs, &test_outputs).unwrap()
        );

        assert!(in_sample < 1e-10, "in-sample RMSE {in_sample}");
        assert!(out_of_sample < 1e-10, "out-of-sample RMSE {out_of_sample}");
    }

    /// **Methodology — against a published test function.** The Ishigami
    /// function, `sin(x1) + 7 sin^2(x2) + 0.1 x3^4 sin(x1)` on `[-pi, pi]^3`,
    /// is the standard sensitivity-analysis benchmark and is emphatically not a
    /// polynomial. A degree-6 fit to 600 samples is scored on 2 000 fresh ones,
    /// so the number reported is genuine predictive accuracy and not an
    /// in-sample flatter.
    ///
    /// The point of this test is not that the fit is good — it is that the
    /// reported accuracy is honest, and that raising the degree improves it in
    /// the expected way. A polynomial surrogate for a function with a
    /// `sin^2` term is a compromise, and the number should say so.
    ///
    /// **Result** (2026-09-16, `--release`, seed 4242): out-of-sample R^2
    /// 0.2144 at degree 2 (10 terms), 0.7250 at degree 4 (35 terms) and
    /// 0.9739 at degree 6 (84 terms), with RMSE falling 3.3331 -> 1.9718 ->
    /// 0.6079. Monotone improvement, and degree 6 is a usable surrogate for a
    /// function that is not a polynomial.
    #[test]
    fn the_ishigami_fit_reports_honest_out_of_sample_accuracy() {
        let ishigami =
            |x: &[f64]| x[0].sin() + 7.0 * x[1].sin().powi(2) + 0.1 * x[2].powi(4) * x[0].sin();
        let mut seed = stream_seed(4_242, 0);
        let draw = |n: usize, seed: &mut u64| {
            let mut inputs = Vec::new();
            let mut outputs = Vec::new();
            for _ in 0..n {
                let row: Vec<f64> = (0..3)
                    .map(|_| (prn(seed) * 2.0 - 1.0) * core::f64::consts::PI)
                    .collect();
                outputs.push(ishigami(&row));
                inputs.push(row);
            }
            (inputs, outputs)
        };
        let (train_inputs, train_outputs) = draw(600, &mut seed);
        let (test_inputs, test_outputs) = draw(2_000, &mut seed);

        let mut row = Vec::new();
        for degree in [2usize, 4, 6] {
            let surrogate =
                PolynomialSurrogate::fit(&train_inputs, &train_outputs, degree, 1e-10).unwrap();
            let r2 = surrogate.r_squared(&test_inputs, &test_outputs).unwrap();
            let rmse = surrogate.rmse(&test_inputs, &test_outputs).unwrap();
            row.push((degree, surrogate.terms(), (r2 * 10000.0).round() / 10000.0));
            println!(
                "Ishigami degree {degree} ({} terms): out-of-sample R^2 {r2:.4}, RMSE {rmse:.4}",
                surrogate.terms()
            );
        }
        // Accuracy must improve with degree, and degree 6 must be a usable fit.
        assert!(row[0].2 < row[1].2 && row[1].2 < row[2].2, "{row:?}");
        assert!(row[2].2 > 0.9, "degree-6 out-of-sample R^2 {}", row[2].2);
    }

    /// **Methodology — over-fitting must be visible, not hidden.** Fitting a
    /// degree-8 polynomial (45 terms in two variables) to 50 noisy samples of a
    /// simple quadratic should fit the training data better than a degree-2
    /// model while predicting fresh data worse. If `r_squared` clamped negative
    /// values away, that would be invisible.
    ///
    /// **Result** (2026-09-16, `--release`, seed 7): degree 2 gave in-sample
    /// RMSE 0.1076 and out-of-sample 0.0511; degree 8 (45 terms on 50 noisy
    /// samples) gave in-sample 0.0427 — better — and out-of-sample 34.7891,
    /// which is 680 times worse than the degree-2 model. Over-fitting is not
    /// subtle when you measure it out of sample, and invisible when you do
    /// not.
    #[test]
    fn over_fitting_shows_up_out_of_sample() {
        let truth = |x: f64, y: f64| 1.0 + x - 0.5 * y * y;
        let mut seed = stream_seed(7, 0);
        let mut inputs = Vec::new();
        let mut outputs = Vec::new();
        for _ in 0..50 {
            let x = prn(&mut seed) * 2.0 - 1.0;
            let y = prn(&mut seed) * 2.0 - 1.0;
            let noise = (prn(&mut seed) - 0.5) * 0.4;
            inputs.push(vec![x, y]);
            outputs.push(truth(x, y) + noise);
        }
        let mut test_inputs = Vec::new();
        let mut test_outputs = Vec::new();
        for _ in 0..500 {
            let x = prn(&mut seed) * 2.0 - 1.0;
            let y = prn(&mut seed) * 2.0 - 1.0;
            test_inputs.push(vec![x, y]);
            test_outputs.push(truth(x, y));
        }

        let simple = PolynomialSurrogate::fit(&inputs, &outputs, 2, 1e-12).unwrap();
        let complex = PolynomialSurrogate::fit(&inputs, &outputs, 8, 1e-12).unwrap();

        let simple_in = simple.rmse(&inputs, &outputs).unwrap();
        let complex_in = complex.rmse(&inputs, &outputs).unwrap();
        let simple_out = simple.rmse(&test_inputs, &test_outputs).unwrap();
        let complex_out = complex.rmse(&test_inputs, &test_outputs).unwrap();
        println!(
            "over-fitting: degree 2 in-sample RMSE {simple_in:.4} / out {simple_out:.4}; \
             degree 8 ({} terms) in-sample {complex_in:.4} / out {complex_out:.4}",
            complex.terms()
        );

        assert!(
            complex_in < simple_in,
            "the larger basis should fit the training data better"
        );
        assert!(
            complex_out > simple_out,
            "the larger basis should predict fresh data worse; it did not, so this test is no \
             longer demonstrating over-fitting"
        );
    }

    /// **Methodology.** A rank-deficient design — fewer samples than basis
    /// terms, with no ridge — must be refused with a message that says what to
    /// do, rather than returning one arbitrary solution out of infinitely many.
    /// Adding a ridge must then make it succeed.
    ///
    /// **Result** (2026-09-16): refused without a ridge, fitted with one.
    #[test]
    fn a_rank_deficient_design_is_refused_not_guessed() {
        let inputs = vec![vec![0.0, 0.0], vec![1.0, 0.0], vec![0.0, 1.0]];
        let outputs = vec![1.0, 2.0, 3.0];
        // Degree 3 in 2 variables is 10 terms against 3 samples.
        assert!(PolynomialSurrogate::fit(&inputs, &outputs, 3, 0.0).is_err());
        assert!(PolynomialSurrogate::fit(&inputs, &outputs, 3, 1e-6).is_ok());
    }

    /// **Methodology.** Rescaling must make a badly-scaled fit work. The same
    /// quadratic in a variable of order 1e6, where an unscaled design matrix
    /// would span 1e18 and the normal equations would lose every digit.
    ///
    /// **Result** (2026-09-16, `--release`, seed 11): RMSE 3.397e-16 against
    /// an output spread of 5.647 — machine precision on inputs spanning 2e6,
    /// which is what the internal rescaling buys.
    #[test]
    fn badly_scaled_inputs_still_fit() {
        let truth = |x: f64| 2.0 + 3.0e-6 * x + 1.0e-13 * x * x;
        let mut seed = stream_seed(11, 0);
        let mut inputs = Vec::new();
        let mut outputs = Vec::new();
        for _ in 0..40 {
            let x = prn(&mut seed) * 2.0e6 - 1.0e6;
            inputs.push(vec![x]);
            outputs.push(truth(x));
        }
        let surrogate = PolynomialSurrogate::fit(&inputs, &outputs, 2, 0.0).unwrap();
        let rmse = surrogate.rmse(&inputs, &outputs).unwrap();
        let spread = outputs.iter().cloned().fold(f64::NEG_INFINITY, f64::max)
            - outputs.iter().cloned().fold(f64::INFINITY, f64::min);
        println!("badly-scaled fit: RMSE {rmse:.3e} against an output spread of {spread:.3e}");
        assert!(rmse < 1e-9 * spread.max(1.0), "RMSE {rmse}");
    }

    /// **Methodology.** Malformed input must be refused: no samples, mismatched
    /// lengths, ragged rows, a non-finite value, a negative ridge, and a
    /// prediction of the wrong width.
    ///
    /// **Result.** All six rejected (2026-09-16).
    #[test]
    fn malformed_fits_are_refused() {
        let inputs = vec![vec![0.0], vec![1.0], vec![2.0]];
        let outputs = vec![0.0, 1.0, 4.0];
        assert!(PolynomialSurrogate::fit(&[], &[], 1, 0.0).is_err());
        assert!(PolynomialSurrogate::fit(&inputs, &[0.0], 1, 0.0).is_err());
        assert!(
            PolynomialSurrogate::fit(&[vec![0.0], vec![1.0, 2.0]], &[0.0, 1.0], 1, 0.0).is_err()
        );
        assert!(
            PolynomialSurrogate::fit(&[vec![f64::NAN], vec![1.0]], &[0.0, 1.0], 1, 0.0).is_err()
        );
        assert!(PolynomialSurrogate::fit(&inputs, &outputs, 1, -1.0).is_err());

        let surrogate = PolynomialSurrogate::fit(&inputs, &outputs, 2, 0.0).unwrap();
        assert!(surrogate.predict(&[1.0, 2.0]).is_err());
    }
}
