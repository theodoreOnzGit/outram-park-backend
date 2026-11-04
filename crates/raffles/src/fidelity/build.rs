// SPDX-License-Identifier: AGPL-3.0-only
// Copyright (C) 2026 OUTRAM PARK contributors
//
// NOT ported from RAVEN. Added 2026-11-04 for GitHub issue #1194 (epic #1177)
// at the WORKSPACE MAINTAINER's direction; the crate owner's (Adolphus Lye's)
// review is outstanding — see this crate's CLAUDE.md, "Scope boundaries".

//! [`build`]: sample a [`HighFidelityModel`] over a design, split the runs,
//! fit a surrogate to the training rows and validate it on the held-out ones.
//!
//! V&V of the whole `fidelity` layer is in this file's tests (Ishigami,
//! poisoning) and recorded in
//! `crates/raffles/verification_and_validation/fidelity_interface_gh1194.md`.

use super::dataset::{Dataset, Fidelity, Split};
use super::models::HighFidelityModel;
use super::surrogate::{Surrogate, SurrogateSpec, ValidationReport};
use crate::samplers::Sampler;
use crate::{RafflesError, Result};

/// Which design rows are held out. Fixed in the plan, before any run.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HeldOut {
    /// The last `n` rows of the design (for a Monte Carlo design these are
    /// an independent random sample).
    Last(usize),
    /// Every `k`-th row, `k − 1, 2k − 1, …` (a spread-out subset of a
    /// space-filling design).
    EveryKth(usize),
}

impl HeldOut {
    /// The split of an `n`-row design.
    ///
    /// # Errors
    ///
    /// [`RafflesError::InvalidParameter`] for `EveryKth(0)`, or a rule that
    /// leaves no training row.
    pub fn split(self, n: usize) -> Result<Vec<Split>> {
        let split: Vec<Split> = match self {
            Self::Last(h) => (0..n)
                .map(|i| {
                    if i + h >= n {
                        Split::HeldOut
                    } else {
                        Split::Training
                    }
                })
                .collect(),
            Self::EveryKth(0) => {
                return Err(RafflesError::InvalidParameter {
                    parameter: "EveryKth".into(),
                    value: 0.0,
                    reason: "k must be at least 1".into(),
                })
            }
            Self::EveryKth(k) => (0..n)
                .map(|i| {
                    if i % k == k - 1 {
                        Split::HeldOut
                    } else {
                        Split::Training
                    }
                })
                .collect(),
        };
        if !split.contains(&Split::Training) {
            return Err(RafflesError::InvalidParameter {
                parameter: "held_out".into(),
                value: n as f64,
                reason: "the rule holds out every row".into(),
            });
        }
        Ok(split)
    }
}

/// How [`build`] runs: the design's seed, the held-out rule, whether to run
/// leave-one-out, and what to record about the data.
#[derive(Debug, Clone, PartialEq)]
pub struct BuildPlan {
    /// Seed of the design ([`Sampler::generate`]).
    pub master_seed: i64,
    /// Which rows are held out.
    pub held_out: HeldOut,
    /// Whether the report carries a leave-one-out RMSE on the training rows.
    pub leave_one_out: bool,
    /// The fidelity tag of the model's runs.
    pub fidelity: Fidelity,
    /// Provenance recorded on the dataset.
    pub provenance: String,
}

/// Runs `hf` at every row of `design` (mapped through
/// [`HighFidelityModel::input_space`]) and splits the runs by
/// `plan.held_out`.
///
/// # Errors
///
/// [`RafflesError::DimensionMismatch`] if the design's width is not the
/// input's; the model's own failures; the held-out rule's.
pub fn sample<M: HighFidelityModel>(
    hf: &M,
    design: &Sampler,
    plan: &BuildPlan,
) -> Result<Dataset<M::Input, M::Output>> {
    let space = hf.input_space();
    let rows = design.generate(plan.master_seed);
    let mut xs = Vec::with_capacity(rows.len());
    let mut ys = Vec::with_capacity(rows.len());
    for row in &rows {
        let x = space.map_unit(row)?;
        ys.push(hf.evaluate(&x)?);
        xs.push(x);
    }
    let split = plan.held_out.split(rows.len())?;
    Dataset::new(&xs, &ys, &split, plan.fidelity.clone(), &plan.provenance)
}

/// Samples `hf` over `design` ([`sample`]), fits `spec` to the training rows
/// and validates on the held-out rows ([`ValidationReport::of`]).
///
/// # Errors
///
/// [`sample`]'s, the fit's and the validation's.
pub fn build<M: HighFidelityModel>(
    hf: &M,
    design: &Sampler,
    plan: &BuildPlan,
    spec: &SurrogateSpec,
) -> Result<(Surrogate<M::Input, M::Output>, ValidationReport<M::Output>)> {
    let data = sample(hf, design, plan)?;
    let surrogate = Surrogate::fit(spec, &data)?;
    let report = ValidationReport::of(&surrogate, spec, &data, plan.leave_one_out)?;
    Ok((surrogate, report))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::fidelity::ParameterSpace;
    use crate::samplers::{stream_seed, LatinHypercube, MonteCarlo};
    use crate::surrogate::PolynomialSurrogate;
    use petir::mathf::RealMath;
    use petir::rng::lcg::prn;
    use uom::si::angle::radian;
    use uom::si::f64::{Angle, Ratio};
    use uom::si::ratio::ratio;

    crate::quantities! {
        /// The Ishigami function's inputs, angles in `[-π, π]`.
        #[derive(Debug, Clone, Copy, PartialEq)]
        pub struct IshigamiInput {
            /// x₁.
            pub x1: Angle,
            /// x₂.
            pub x2: Angle,
            /// x₃.
            pub x3: Angle,
        }
    }

    crate::quantities! {
        /// The Ishigami response (dimensionless).
        #[derive(Debug, Clone, Copy, PartialEq)]
        pub struct IshigamiOutput {
            /// `sin x₁ + 7 sin² x₂ + 0.1 x₃⁴ sin x₁`.
            pub y: Ratio,
        }
    }

    /// The toy high-fidelity model: Ishigami at `a = 7`, `b = 0.1`, the
    /// same expression as `surrogate::polynomial`'s Ishigami test.
    struct Ishigami;

    fn ishigami(x: &[f64]) -> f64 {
        x[0].r_sin() + 7.0 * x[1].r_sin().r_powi(2) + 0.1 * x[2].r_powi(4) * x[0].r_sin()
    }

    impl HighFidelityModel for Ishigami {
        type Input = IshigamiInput;
        type Output = IshigamiOutput;
        fn input_space(&self) -> ParameterSpace<IshigamiInput> {
            let a = |v: f64| Angle::new::<radian>(v);
            let pi = core::f64::consts::PI;
            match ParameterSpace::bounded(
                &IshigamiInput {
                    x1: a(-pi),
                    x2: a(-pi),
                    x3: a(-pi),
                },
                &IshigamiInput {
                    x1: a(pi),
                    x2: a(pi),
                    x3: a(pi),
                },
            ) {
                Ok(s) => s,
                Err(e) => panic!("{e}"),
            }
        }
        fn evaluate(&self, x: &IshigamiInput) -> Result<IshigamiOutput> {
            let v = [
                x.x1.get::<radian>(),
                x.x2.get::<radian>(),
                x.x3.get::<radian>(),
            ];
            Ok(IshigamiOutput {
                y: Ratio::new::<ratio>(ishigami(&v)),
            })
        }
    }

    fn plan(held_out: HeldOut, leave_one_out: bool) -> BuildPlan {
        BuildPlan {
            master_seed: 4_242,
            held_out,
            leave_one_out,
            fidelity: Fidelity::new(1, "Ishigami"),
            provenance: "analytic Ishigami, a = 7, b = 0.1".into(),
        }
    }

    /// `(train x, train y, test x, test y)` exactly as
    /// `surrogate::polynomial::tests::the_ishigami_fit_reports_honest_out_of_sample_accuracy`
    /// draws them (seed 4242, stream 0, 600 then 2000 rows).
    fn legacy_draws() -> (Vec<Vec<f64>>, Vec<f64>, Vec<Vec<f64>>, Vec<f64>) {
        let mut seed = stream_seed(4_242, 0);
        let mut draw = |n: usize| {
            let mut inputs = Vec::new();
            let mut outputs = Vec::new();
            for _ in 0..n {
                let row: Vec<f64> = (0..3)
                    .map(|_| (prn(&mut seed) * 2.0 - 1.0) * core::f64::consts::PI)
                    .collect();
                outputs.push(ishigami(&row));
                inputs.push(row);
            }
            (inputs, outputs)
        };
        let (a, b) = draw(600);
        let (c, d) = draw(2_000);
        (a, b, c, d)
    }

    /// **Methodology — the typed path reproduces the recorded polynomial
    /// results bit for bit.** The 600 training and 2000 test draws of the
    /// existing `PolynomialSurrogate` Ishigami test are put into a typed
    /// `Dataset<IshigamiInput, IshigamiOutput>` (inputs as `Angle`, split
    /// training / held-out), fitted through `Surrogate::fit` with
    /// `SurrogateSpec::Polynomial` at degrees 2, 4, 6 (ridge 1e-10, as
    /// there), and validated by `ValidationReport::of`. Pass: the fitted
    /// coefficients equal the direct `PolynomialSurrogate::fit`'s (`==`), the
    /// held-out RMSE equals `PolynomialSurrogate::rmse` on the test draws bit
    /// for bit, and both round to the RMSE recorded on 2026-09-16 (3.3331,
    /// 1.9718, 0.6079).
    ///
    /// **Prediction** (written before the run): identical bits, since the
    /// typed layer only copies `value` and the same `fit` runs on the same
    /// rows in the same order. **Result (2026-11-04):** coefficients and held-out RMSE
    /// identical at all three degrees (`==`, `to_bits`); RMSE 3.3331, 1.9718,
    /// 0.6079, as recorded; max |error| 12.4143, 8.2290, 4.5333 (new).
    #[test]
    fn the_typed_path_reproduces_the_recorded_ishigami_fit_bit_for_bit() {
        let (tx, ty, hx, hy) = legacy_draws();
        let to_in = |r: &Vec<f64>| IshigamiInput {
            x1: Angle::new::<radian>(r[0]),
            x2: Angle::new::<radian>(r[1]),
            x3: Angle::new::<radian>(r[2]),
        };
        let to_out = |y: &f64| IshigamiOutput {
            y: Ratio::new::<ratio>(*y),
        };
        let xs: Vec<IshigamiInput> = tx.iter().chain(hx.iter()).map(to_in).collect();
        let ys: Vec<IshigamiOutput> = ty.iter().chain(hy.iter()).map(to_out).collect();
        let split: Vec<Split> = (0..2_600)
            .map(|i| {
                if i < 600 {
                    Split::Training
                } else {
                    Split::HeldOut
                }
            })
            .collect();
        let data = Dataset::new(
            &xs,
            &ys,
            &split,
            Fidelity::new(1, "Ishigami"),
            "legacy draws",
        )
        .unwrap();
        for (degree, recorded) in [(2usize, 3.3331), (4, 1.9718), (6, 0.6079)] {
            let spec = SurrogateSpec::Polynomial {
                degree,
                ridge: 1e-10,
            };
            let s = Surrogate::fit(&spec, &data).unwrap();
            let direct = PolynomialSurrogate::fit(&tx, &ty, degree, 1e-10).unwrap();
            let multi = s.as_polynomial().unwrap();
            assert_eq!(multi.outputs()[0], direct);
            let report = ValidationReport::of(&s, &spec, &data, false).unwrap();
            let rmse = report.held_out_rmse.unwrap().y.value;
            assert_eq!(
                rmse.to_bits(),
                direct.rmse(&hx, &hy).unwrap().to_bits(),
                "degree {degree}"
            );
            assert!(
                ((rmse * 1e4).round() / 1e4 - recorded).abs() < 1e-12,
                "degree {degree}: {rmse}"
            );
            println!(
                "Ishigami typed path, degree {degree}: held-out RMSE {rmse:.4}, max |error| {:.4}",
                report.held_out_max_error.unwrap().y.value
            );
        }
    }

    /// **Methodology — `build` end to end, against the direct fit.** `build`
    /// runs the typed Ishigami model on a 2600-row Monte Carlo design (seed
    /// 4242) mapped through `U(−π, π)` on each angle, holds out the last
    /// 2000 rows, fits degree 6 (ridge 1e-10) and reports held-out RMSE,
    /// max error and LOO RMSE. Pass: the surrogate and every number equal a
    /// direct `PolynomialSurrogate::fit` / `rmse` / `LeaveOneOut` on the
    /// same rows bit for bit; and, as a sanity bound on the design (not a
    /// tuned gate), held-out RMSE within 0.6079 ± 0.15 of the legacy draw's,
    /// since both are 600-point random designs on the same function.
    ///
    /// **Prediction** (written before the run): identical bits; RMSE near
    /// 0.6; LOO RMSE a little above the held-out RMSE (LOO models see 599
    /// points). **Result (2026-11-04):** identical bits for the surrogate,
    /// held-out RMSE, max error and LOO RMSE; held-out RMSE 0.6218, max
    /// |error| 3.6508, LOO RMSE 0.5526. **The LOO prediction missed**: LOO
    /// came out *below* the held-out RMSE, not above. Both estimate the same
    /// error from different finite samples (600 LOO residuals against 2000
    /// fresh points); a 12 % difference is within what that allows, and
    /// nothing was changed in response.
    #[test]
    fn build_matches_the_direct_fit_bit_for_bit() {
        let design = Sampler::MonteCarlo(MonteCarlo::new(2_600, 3).unwrap());
        let plan = plan(HeldOut::Last(2_000), true);
        let spec = SurrogateSpec::Polynomial {
            degree: 6,
            ridge: 1e-10,
        };
        let (s, report) = build(&Ishigami, &design, &plan, &spec).unwrap();
        let data = sample(&Ishigami, &design, &plan).unwrap();
        assert_eq!(data.counts(), (600, 2_000));
        assert_eq!(report.counts, (600, 2_000));
        let (tx, ty) = data.training_si();
        let (hx, hy) = data.held_out_si();
        let ty: Vec<f64> = ty.iter().map(|r| r[0]).collect();
        let hy: Vec<f64> = hy.iter().map(|r| r[0]).collect();
        let direct = PolynomialSurrogate::fit(&tx, &ty, 6, 1e-10).unwrap();
        let multi = s.as_polynomial().unwrap();
        assert_eq!(multi.outputs()[0], direct);
        let rmse = report.held_out_rmse.unwrap().y.value;
        assert_eq!(rmse.to_bits(), direct.rmse(&hx, &hy).unwrap().to_bits());
        let max = hx
            .iter()
            .zip(hy.iter())
            .map(|(x, y)| (y - direct.predict(x).unwrap()).abs())
            .fold(0.0, f64::max);
        assert_eq!(
            report.held_out_max_error.unwrap().y.value.to_bits(),
            max.to_bits()
        );
        let loo = crate::surrogate::validation::LeaveOneOut::fit(&tx, &ty, 6, 1e-10)
            .unwrap()
            .rmse();
        assert_eq!(report.loo_rmse.unwrap().y.value.to_bits(), loo.to_bits());
        println!("build, Ishigami degree 6: held-out RMSE {rmse:.4}, max |error| {max:.4}, LOO RMSE {loo:.4}");
        assert!((rmse - 0.6079).abs() < 0.15, "{rmse}");
        assert!(report.coverage_2sigma.is_none());
    }

    /// **Methodology — the held-out split is never read by a fit.** A
    /// dataset's held-out outputs are replaced by NaN, and separately by
    /// ±1e300; `Surrogate::fit` on each must give a surrogate equal (`==` on
    /// every coefficient) to the clean one, while the validation report on
    /// the poisoned data reports the poison (NaN, or an RMSE of order 1e300)
    /// rather than hiding it.
    ///
    /// **Prediction:** identical fits. **Result (2026-11-04):** identical fits for NaN, +1e300 and
    /// −1e300 (`==` on the multi-output polynomial); the poisoned reports
    /// read NaN, and above 1e299, as asserted.
    #[test]
    fn poisoned_held_out_outputs_do_not_change_the_fit() {
        let design = Sampler::LatinHypercube(LatinHypercube::new(200, 3).unwrap());
        let plan = plan(HeldOut::EveryKth(4), false);
        let clean = sample(&Ishigami, &design, &plan).unwrap();
        assert_eq!(clean.counts(), (150, 50));
        let spec = SurrogateSpec::Polynomial {
            degree: 4,
            ridge: 1e-10,
        };
        let reference = Surrogate::fit(&spec, &clean).unwrap();
        let reference = reference.as_polynomial().unwrap().clone();
        for (k, poison) in [f64::NAN, 1e300, -1e300].into_iter().enumerate() {
            let mut xs = Vec::new();
            let mut ys = Vec::new();
            for i in 0..clean.len() {
                let (x, mut y, s) = clean.row(i).unwrap();
                if s == Split::HeldOut {
                    y.y = Ratio::new::<ratio>(poison);
                }
                xs.push(x);
                ys.push(y);
            }
            let poisoned = Dataset::new(
                &xs,
                &ys,
                clean.split(),
                clean.fidelity().clone(),
                "poisoned",
            )
            .unwrap();
            let s = Surrogate::fit(&spec, &poisoned).unwrap();
            let fit = s.as_polynomial().unwrap();
            assert_eq!(fit, &reference, "poison {k}");
            let r = ValidationReport::of(&s, &spec, &poisoned, false)
                .unwrap()
                .held_out_rmse
                .unwrap()
                .y
                .value;
            assert!(
                if poison.is_nan() {
                    r.is_nan()
                } else {
                    r > 1e299
                },
                "poison {k}: report {r}"
            );
        }
    }

    /// The held-out rules, and a model failure reaching the caller.
    #[test]
    fn held_out_rules_and_failures() {
        use Split::{HeldOut as H, Training as T};
        assert_eq!(HeldOut::Last(2).split(4).unwrap(), vec![T, T, H, H]);
        assert_eq!(HeldOut::EveryKth(2).split(4).unwrap(), vec![T, H, T, H]);
        assert!(HeldOut::EveryKth(0).split(4).is_err());
        assert!(HeldOut::Last(4).split(4).is_err());
        assert!(HeldOut::EveryKth(1).split(4).is_err());

        struct Failing;
        impl HighFidelityModel for Failing {
            type Input = IshigamiInput;
            type Output = IshigamiOutput;
            fn input_space(&self) -> ParameterSpace<IshigamiInput> {
                Ishigami.input_space()
            }
            fn evaluate(&self, _: &IshigamiInput) -> Result<IshigamiOutput> {
                Err(RafflesError::ModelEvaluation {
                    model: "Failing".into(),
                    reason: "always".into(),
                })
            }
        }
        let design = Sampler::MonteCarlo(MonteCarlo::new(5, 3).unwrap());
        let spec = SurrogateSpec::Polynomial {
            degree: 1,
            ridge: 0.0,
        };
        let e = build(&Failing, &design, &plan(HeldOut::Last(1), false), &spec).unwrap_err();
        assert_eq!(e.to_string(), "model `Failing` failed: always");
        let wrong_width = Sampler::MonteCarlo(MonteCarlo::new(5, 2).unwrap());
        assert!(build(
            &Ishigami,
            &wrong_width,
            &plan(HeldOut::Last(1), false),
            &spec
        )
        .is_err());
    }

    /// The neural route through the same interface: trains on Ishigami's
    /// training rows and is validated on held-out rows (no LOO, no σ).
    /// A plumbing check, not a V&V gate: the gate is only that the
    /// network beats predicting the training mean on held-out data.
    ///
    /// **Result (2026-11-04):** held-out RMSE 0.6367 against the mean
    /// predictor's 3.5808.
    #[cfg(feature = "burn")]
    #[test]
    fn the_neural_route_runs_through_the_same_interface() {
        let design = Sampler::LatinHypercube(LatinHypercube::new(300, 3).unwrap());
        let plan = plan(HeldOut::EveryKth(5), true);
        let mut config = crate::surrogate::neural::TrainingConfig::default_for(7);
        config.epochs = 600;
        let spec = SurrogateSpec::Neural { config };
        let (s, report) = build(&Ishigami, &design, &plan, &spec).unwrap();
        assert!(report.loo_rmse.is_none());
        let data = sample(&Ishigami, &design, &plan).unwrap();
        let (_, hy) = data.held_out_si();
        let (_, ty) = data.training_si();
        let mean = ty.iter().map(|r| r[0]).sum::<f64>() / ty.len() as f64;
        let sd = (hy.iter().map(|r| (r[0] - mean).r_powi(2)).sum::<f64>() / hy.len() as f64).sqrt();
        let rmse = report.held_out_rmse.unwrap().y.value;
        println!(
            "neural route, Ishigami: held-out RMSE {rmse:.4} against the mean predictor's {sd:.4}"
        );
        assert!(rmse < sd, "{rmse} vs {sd}");
        assert!(s
            .predict(&data.row(0).unwrap().0)
            .unwrap()
            .std_dev
            .is_none());
    }
}
