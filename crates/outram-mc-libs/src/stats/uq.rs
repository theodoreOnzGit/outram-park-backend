//! **Uncertainty quantification and sensitivity by sampling inputs** —
//! GitHub #498.
//!
//! # What this is, and what it is not
//!
//! **Quantification only.** Input uncertainties (material densities, packing
//! fraction, impurity content, dimensions — nuclear-data covariances later,
//! from the njoy ERRORR port) are sampled, the caller's Monte Carlo model is
//! run at each sample, and the spread of the outputs is reported with each
//! input's share of it. **Nothing is fitted to a reference here.** A later
//! Bayesian calibration (`raffles::bayesian`, `raffles::abc`) must follow the
//! root `CLAUDE.md` model-hierarchy rule: uncalibrated first, then ablation,
//! never tuning until a comparison passes.
//!
//! # The pieces
//!
//! - [`UncertainInput`]: a named input with a `raffles` [`Distribution`].
//! - [`UqDesign::sampled`]: a design from any `raffles` [`Sampler`] (Monte
//!   Carlo, Latin hypercube, grid), mapped through each input's inverse CDF.
//! - [`UqDesign::sobol`]: a Saltelli design for first-order and total Sobol
//!   indices (`raffles::sensitivity::SobolSampleLayout`).
//! - [`UqDesign::evaluate`]: runs the caller's closure at every point on the
//!   [`super::ensemble`] runner (in order, worker-count independent).
//! - [`UqReport::analyse`]: output mean, total sd, the part of the variance
//!   that is the runs' own Monte Carlo noise, empirical quantiles, and per
//!   input Pearson / Spearman correlations and a marginal least-squares slope
//!   (sampled designs), or Sobol indices (Sobol designs).
//! - [`DirectPerturbation`]: the central-difference derivative the sampled
//!   slope is checked against.
//!
//! # Monte Carlo noise is separated, not ignored
//!
//! Each output carries its own statistical `σ_i`. The observed output
//! variance is input variance **plus** that noise, so
//! `var_inputs ≈ var_total − mean(σ_i²)` is reported beside the total, and
//! the noise fraction is stated. Sensitivity indices from noise-dominated
//! outputs are not interpretable, and the report says when that is the case
//! rather than quoting them bare.
//!
//! # V&V gate (from #498, fixed in advance)
//!
//! On a case with a known analytic sensitivity (a bare sphere's `k` against
//! density), the sampled sensitivity matches the analytic or
//! direct-perturbation value within its uncertainty: here, the marginal
//! least-squares slope agrees with [`DirectPerturbation`] at
//! `|z| ≤ 2` ([`DirectPerturbation::agreement`]).
//!
//! **Result: NOT YET MEASURED (testing deferred by maintainer, 2026-10-03).**
//! Protocol: `verification_and_validation/stats_epic_493/498_uq_sensitivity.md`.

use raffles::distributions::{ContinuousDistribution1D, Distribution};
use raffles::samplers::{MonteCarlo, Sampler, SamplingDesign};
use raffles::sensitivity::{
    input_output_correlations, sobol_indices, CorrelationKind, SobolIndices, SobolSampleLayout,
};

/// `|z|` band for the direct-perturbation agreement gate, fixed in advance.
pub const AGREEMENT_Z_BAND: f64 = 2.0;

/// One uncertain input.
#[derive(Debug, Clone, PartialEq)]
pub struct UncertainInput {
    /// How the input is named in reports.
    pub name: String,
    /// Its distribution, in the input's own unit.
    pub distribution: Distribution,
}

/// How a design was built, which decides what can be estimated from it.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum DesignKind {
    /// Independent or stratified samples: correlations and slopes.
    Sampled,
    /// A Saltelli stack for Sobol indices.
    Sobol(SobolSampleLayout),
}

/// A set of input points to run the model at.
#[derive(Debug, Clone, PartialEq)]
pub struct UqDesign {
    /// The inputs, in column order.
    pub inputs: Vec<UncertainInput>,
    /// `points[j][i]`: run `j`'s value of input `i`, in the input's unit.
    pub points: Vec<Vec<f64>>,
    /// How the points were generated.
    pub kind: DesignKind,
}

fn map_row(inputs: &[UncertainInput], u: &[f64]) -> Result<Vec<f64>, String> {
    inputs
        .iter()
        .zip(u)
        .map(|(inp, &ui)| {
            let v = inp
                .distribution
                .ppf(ui)
                .map_err(|e| format!("{}: {e}", inp.name))?;
            if v.is_finite() {
                Ok(v)
            } else {
                Err(format!(
                    "{}: probability {ui} maps to a non-finite value; the sampler hit the \
                     edge of an unbounded support",
                    inp.name
                ))
            }
        })
        .collect()
}

impl UqDesign {
    /// Map `sampler`'s unit-hypercube design through each input's inverse CDF.
    ///
    /// # Errors
    ///
    /// Sampler dimension differs from the input count, or a point maps to a
    /// non-finite value.
    pub fn sampled(
        inputs: Vec<UncertainInput>,
        sampler: &Sampler,
        master_seed: i64,
    ) -> Result<Self, String> {
        if sampler.dimensions() != inputs.len() {
            return Err(format!(
                "sampler has {} dimensions but there are {} inputs",
                sampler.dimensions(),
                inputs.len()
            ));
        }
        let points = sampler
            .generate(master_seed)
            .iter()
            .map(|u| map_row(&inputs, u))
            .collect::<Result<Vec<_>, _>>()?;
        Ok(Self {
            inputs,
            points,
            kind: DesignKind::Sampled,
        })
    }

    /// A Saltelli design for Sobol indices: `base_samples · (k + 2)` runs.
    /// `A` and `B` are independent Monte Carlo draws (one `2k`-dimensional
    /// design, split by columns, so each column has its own stream).
    ///
    /// # Errors
    ///
    /// Zero inputs or samples, or a non-finite mapped value.
    pub fn sobol(
        inputs: Vec<UncertainInput>,
        base_samples: usize,
        master_seed: i64,
    ) -> Result<Self, String> {
        let k = inputs.len();
        let layout = SobolSampleLayout::new(k, base_samples).map_err(|e| e.to_string())?;
        let mc = MonteCarlo::new(base_samples, 2 * k).map_err(|e| e.to_string())?;
        let u = mc.generate(master_seed);
        let mut a = Vec::with_capacity(base_samples * k);
        let mut b = Vec::with_capacity(base_samples * k);
        for row in &u {
            a.extend(map_row(&inputs, &row[..k])?);
            b.extend(map_row(&inputs, &row[k..])?);
        }
        let flat = layout.build_design(&a, &b).map_err(|e| e.to_string())?;
        let points = flat.chunks(k).map(|c| c.to_vec()).collect();
        Ok(Self {
            inputs,
            points,
            kind: DesignKind::Sobol(layout),
        })
    }

    /// Run `model(point, index) -> (value, sigma)` at every point on `workers`
    /// threads, in order. `index` is the point's row, for deriving a seed.
    pub fn evaluate<F>(&self, workers: usize, model: F) -> Vec<(f64, f64)>
    where
        F: Fn(&[f64], usize) -> (f64, f64) + Sync,
    {
        let idx: Vec<u64> = (0..self.points.len() as u64).collect();
        super::ensemble::run_seeds(&idx, workers, |i| {
            model(&self.points[i as usize], i as usize)
        })
        .into_iter()
        .map(|r| (r.value, r.sigma))
        .collect()
    }
}

/// One input's sensitivity measures from a sampled design.
#[derive(Debug, Clone, PartialEq)]
pub struct InputSensitivity {
    /// The input's name.
    pub name: String,
    /// Pearson correlation of output with input.
    pub pearson: f64,
    /// Spearman rank correlation.
    pub spearman: f64,
    /// Marginal least-squares slope `d(output)/d(input)` with its 1σ — the
    /// quantity the #498 gate compares with a direct perturbation. Marginal:
    /// other inputs' variation is in the residual, which is correct for
    /// independent inputs and inflates the slope's error otherwise.
    pub slope: (f64, f64),
}

/// What [`UqReport::analyse`] measured.
#[derive(Debug, Clone, PartialEq)]
pub struct UqReport {
    /// Number of runs.
    pub n: usize,
    /// Mean output.
    pub mean: f64,
    /// Total sample sd of the outputs.
    pub sd_total: f64,
    /// `mean(σ_i²)`: the runs' own Monte Carlo noise variance.
    pub mc_noise_variance: f64,
    /// `sqrt(max(0, sd_total² − mc_noise_variance))`: the spread attributable
    /// to the inputs.
    pub sd_inputs: f64,
    /// `mc_noise_variance / sd_total²` (≥ 0.5 means noise-dominated: the
    /// sensitivity measures are not interpretable).
    pub noise_fraction: f64,
    /// Empirical 2.5 %, 50 %, 97.5 % quantiles of the outputs.
    pub quantiles: [f64; 3],
    /// Per-input measures (sampled designs only).
    pub sensitivities: Vec<InputSensitivity>,
    /// Sobol indices (Sobol designs only).
    pub sobol: Option<SobolIndices>,
}

impl UqReport {
    /// Analyse `outputs[j] = (value, sigma)` for `design.points[j]`.
    ///
    /// # Errors
    ///
    /// Length mismatch, fewer than three runs, or a `raffles` estimator error
    /// (e.g. a constant input column).
    pub fn analyse(design: &UqDesign, outputs: &[(f64, f64)]) -> Result<Self, String> {
        let n = outputs.len();
        if n != design.points.len() {
            return Err(format!("{} outputs for {} design points", n, design.points.len()));
        }
        if n < 3 {
            return Err("UQ needs at least three runs".into());
        }
        let y: Vec<f64> = outputs.iter().map(|o| o.0).collect();
        let (mean, sd_total, _) = raffles::estimators::pooled(&y);
        let mc_noise_variance = outputs.iter().map(|o| o.1 * o.1).sum::<f64>() / n as f64;
        let var_total = sd_total * sd_total;
        let sd_inputs = (var_total - mc_noise_variance).max(0.0).sqrt();
        let noise_fraction = if var_total > 0.0 {
            mc_noise_variance / var_total
        } else {
            f64::INFINITY
        };
        let mut sorted = y.clone();
        sorted.sort_by(f64::total_cmp);
        let quantiles = [
            quantile(&sorted, 0.025),
            quantile(&sorted, 0.5),
            quantile(&sorted, 0.975),
        ];
        let k = design.inputs.len();
        let mut sensitivities = Vec::new();
        let mut sobol = None;
        match design.kind {
            DesignKind::Sampled => {
                let flat: Vec<f64> = design.points.iter().flatten().copied().collect();
                let p = input_output_correlations(&flat, k, &y, CorrelationKind::Pearson)
                    .map_err(|e| e.to_string())?;
                let s = input_output_correlations(&flat, k, &y, CorrelationKind::Spearman)
                    .map_err(|e| e.to_string())?;
                for i in 0..k {
                    let x: Vec<f64> = design.points.iter().map(|r| r[i]).collect();
                    sensitivities.push(InputSensitivity {
                        name: design.inputs[i].name.clone(),
                        pearson: p[i],
                        spearman: s[i],
                        slope: ols_slope(&x, &y).ok_or("degenerate input column")?,
                    });
                }
            }
            DesignKind::Sobol(layout) => {
                sobol = Some(sobol_indices(layout, &y).map_err(|e| e.to_string())?);
            }
        }
        Ok(Self {
            n,
            mean,
            sd_total,
            mc_noise_variance,
            sd_inputs,
            noise_fraction,
            quantiles,
            sensitivities,
            sobol,
        })
    }
}

/// Linear-interpolated empirical quantile of an ascending slice.
fn quantile(sorted: &[f64], p: f64) -> f64 {
    let h = p * (sorted.len() - 1) as f64;
    let lo = h.floor() as usize;
    let hi = h.ceil() as usize;
    sorted[lo] + (h - lo as f64) * (sorted[hi] - sorted[lo])
}

/// Simple least-squares slope of `y` on `x` with its textbook 1σ,
/// `sqrt(s²_res / Σ(x − x̄)²)`, `s²_res` with `n − 2` dof. `None` for fewer
/// than three points or a constant `x`.
///
/// Generic statistics that would belong in RAFFLES; kept here, as #493's
/// ownership comment allows, until the crate owner has agreed to the
/// estimator modules.
fn ols_slope(x: &[f64], y: &[f64]) -> Option<(f64, f64)> {
    let n = x.len();
    if n < 3 || y.len() != n {
        return None;
    }
    let nf = n as f64;
    let mx = x.iter().sum::<f64>() / nf;
    let my = y.iter().sum::<f64>() / nf;
    let sxx: f64 = x.iter().map(|v| (v - mx).powi(2)).sum();
    if !(sxx > 0.0) {
        return None;
    }
    let sxy: f64 = x.iter().zip(y).map(|(a, b)| (a - mx) * (b - my)).sum();
    let slope = sxy / sxx;
    let ss_res: f64 = x
        .iter()
        .zip(y)
        .map(|(a, b)| (b - my - slope * (a - mx)).powi(2))
        .sum();
    Some((slope, (ss_res / (nf - 2.0) / sxx).sqrt()))
}

/// A central-difference derivative from two independent runs at `x0 ± dx`.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct DirectPerturbation {
    /// `(v₊ − v₋) / (2 dx)`.
    pub derivative: f64,
    /// `sqrt(σ₊² + σ₋²) / (2 dx)` — statistical only; the truncation error of
    /// the central difference is `O(dx²)` and is the caller's to keep small.
    pub sigma: f64,
}

impl DirectPerturbation {
    /// From the runs at `x0 + dx` (`plus`) and `x0 − dx` (`minus`), each
    /// `(value, sigma)`.
    pub fn new(dx: f64, plus: (f64, f64), minus: (f64, f64)) -> Self {
        Self {
            derivative: (plus.0 - minus.0) / (2.0 * dx),
            sigma: (plus.1 * plus.1 + minus.1 * minus.1).sqrt() / (2.0 * dx),
        }
    }

    /// `(z, agrees)` of a sampled slope `(value, sigma)` against this
    /// derivative: `z = Δ / sqrt(σ₁² + σ₂²)`, agreeing when
    /// `|z| ≤ AGREEMENT_Z_BAND`.
    pub fn agreement(&self, slope: (f64, f64)) -> (f64, bool) {
        let z = (slope.0 - self.derivative) / (slope.1 * slope.1 + self.sigma * self.sigma).sqrt();
        (z, z.abs() <= AGREEMENT_Z_BAND)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use raffles::distributions::{Normal, Uniform};
    use raffles::samplers::LatinHypercube;

    fn inputs() -> Vec<UncertainInput> {
        vec![
            UncertainInput {
                name: "density".into(),
                distribution: Distribution::Normal(Normal::new(18.7, 0.1).unwrap()),
            },
            UncertainInput {
                name: "radius".into(),
                distribution: Distribution::Uniform(Uniform::new(8.6, 8.9).unwrap()),
            },
        ]
    }

    /// **Methodology.** A linear "model" `y = 2·density − 3·radius` with a
    /// fixed per-run σ of 0.01 and no noise added. The marginal slopes must
    /// recover 2 and −3 within **4 of their own reported σ** (LHS, 200
    /// points; the other input's variation sits in the residual, so each
    /// marginal slope scatters by about 0.17 here — the gate is sized from
    /// the estimator's own error, fixed in advance). The noise variance must
    /// be reported as `1e-4`. A direct perturbation of the model must give
    /// exactly 2, and `agreement` must accept a slope 1σ away and reject one
    /// 3σ away (hand numbers, so the gate's arithmetic is tested without
    /// sampling luck).
    ///
    /// **Result:** NOT YET MEASURED (testing deferred by maintainer,
    /// 2026-10-03).
    #[test]
    fn linear_model_slopes_and_perturbation_agree() {
        let sampler = Sampler::LatinHypercube(LatinHypercube::new(200, 2).unwrap());
        let d = UqDesign::sampled(inputs(), &sampler, 498).unwrap();
        let out = d.evaluate(3, |p, _| (2.0 * p[0] - 3.0 * p[1], 0.01));
        let r = UqReport::analyse(&d, &out).unwrap();
        assert!((r.mc_noise_variance - 1e-4).abs() < 1e-15);
        let s0 = r.sensitivities[0].slope;
        let s1 = r.sensitivities[1].slope;
        assert!((s0.0 - 2.0).abs() < 4.0 * s0.1, "density slope {s0:?}");
        assert!((s1.0 + 3.0).abs() < 4.0 * s1.1, "radius slope {s1:?}");
        let dp = DirectPerturbation::new(
            0.05,
            (2.0 * 18.75 - 3.0 * 8.75, 0.01),
            (2.0 * 18.65 - 3.0 * 8.75, 0.01),
        );
        assert!((dp.derivative - 2.0).abs() < 1e-9);
        let combined = |s: f64| (s * s + dp.sigma * dp.sigma).sqrt();
        assert!(dp.agreement((2.0 + combined(0.3), 0.3)).1);
        assert!(!dp.agreement((2.0 + 3.0 * combined(0.3), 0.3)).1);
    }

    /// **Methodology.** A Saltelli design on the additive model above must
    /// have `base·(k + 2)` rows and give first-order indices that sum to
    /// about 1 (gate `0.8..1.2` at 512 base samples).
    ///
    /// ~~**Result:** NOT YET MEASURED (testing deferred by maintainer,
    /// 2026-10-03).~~ **Result (2026-10-07, gh:#584):** failed on first run
    /// with a first-order sum of `-1.437`. The cause was the estimator, not
    /// this gate: `raffles::sensitivity::sobol_indices` multiplied the
    /// *uncentred* `y_B` (here `ybar/sd ~ 34`), whose sampling error swamps
    /// the index at `n = 512`. A 2000-seed replica gave the sum an sd of
    /// `2.17` uncentred and `0.058` centred. With `y_B` centred in `raffles`
    /// the test passes, gate unchanged. The sum itself is not printed.
    #[test]
    fn sobol_design_on_an_additive_model() {
        let d = UqDesign::sobol(inputs(), 512, 7).unwrap();
        assert_eq!(d.points.len(), 512 * 4);
        let out: Vec<(f64, f64)> = d.points.iter().map(|p| (2.0 * p[0] - 3.0 * p[1], 0.0)).collect();
        let r = UqReport::analyse(&d, &out).unwrap();
        let s = r.sobol.unwrap();
        let sum: f64 = s.first_order.iter().sum();
        assert!(sum > 0.8 && sum < 1.2, "sum of first-order {sum}");
        assert!(r.sensitivities.is_empty());
    }

    /// Mismatched lengths and too-few runs are refused.
    #[test]
    fn bad_inputs_are_refused() {
        let sampler = Sampler::LatinHypercube(LatinHypercube::new(4, 3).unwrap());
        assert!(UqDesign::sampled(inputs(), &sampler, 1).is_err());
        let sampler = Sampler::LatinHypercube(LatinHypercube::new(4, 2).unwrap());
        let d = UqDesign::sampled(inputs(), &sampler, 1).unwrap();
        assert!(UqReport::analyse(&d, &[(1.0, 0.1); 3]).is_err());
        assert!(ols_slope(&[1.0, 1.0, 1.0], &[1.0, 2.0, 3.0]).is_none());
    }
}
