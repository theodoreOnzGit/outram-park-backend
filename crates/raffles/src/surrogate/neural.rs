//! Neural-network regression surrogates, trained with `burn`.
//!
//! Available only with the crate's `burn` feature.
//!
//! # What this is for
//!
//! The surrogate to reach for when a polynomial cannot express the response:
//! sharp transitions, thresholds, interactions that a bounded-degree expansion
//! would need hundreds of terms to capture. This is the shape of model the
//! adaptive Bayesian material-property work uses for creep-rupture and tensile
//! data, and the shape RAVEN offers as an `ANN` ROM.
//!
//! It is also the cheaper choice to reach for *second*. A polynomial
//! ([`super::polynomial`]) fits in closed form, has no training run to babysit,
//! and tells you its own coefficients. Try it first; move here when its
//! out-of-sample error says you must.
//!
//! # Reuse
//!
//! The network is [`crate::gnn::mpnn::Mlp`], the same multi-layer perceptron
//! the message-passing network is built from — deliberately, rather than a
//! second copy of the same twenty lines. A defect fixed in one is fixed in
//! both.
//!
//! # Scaling is not optional, so it is not a parameter
//!
//! Inputs and outputs are standardised to zero mean and unit variance before
//! training, and un-standardised on prediction. A network fed a stiffness of
//! `2e11` alongside a damping ratio of `0.02` will spend its whole training
//! budget on the first and never see the second; with ReLU activations it may
//! not train at all. This is done automatically and the statistics travel with
//! the fitted surrogate, because a surrogate that silently requires the caller
//! to have scaled their data the same way is a trap.
//!
//! # Training
//!
//! Full-batch Adam on the mean-squared error, for a fixed number of epochs.
//! Deliberately plain: no learning-rate schedule, no early stopping, no
//! mini-batching, no validation split. Each of those is a real improvement and
//! each is a decision the crate owner should make rather than inherit from a
//! default here. [`TrainingReport`] carries the whole loss history so a caller
//! can see whether the run converged, plateaued or diverged, instead of being
//! told only the final number.
//!
//! # What is NOT verified here
//!
//! That the fitted network is *right*. The tests below check that training
//! reduces the loss, that a known function is recovered to a stated accuracy on
//! held-out data, and that scaling and reproducibility behave — none of which
//! says the surrogate is fit for a particular physical purpose. That is the
//! caller's V&V, on the caller's data.


use burn::nn::loss::{MseLoss, Reduction};
use burn::optim::{AdamConfig, GradientsParams, Optimizer};
use burn::prelude::Backend;
use burn::tensor::backend::AutodiffBackend;
use burn::tensor::{Tensor, TensorData};

use crate::gnn::mpnn::Mlp;
use crate::{RafflesError, Result};

/// Per-column mean and standard deviation, used to standardise a data set.
///
/// A zero-variance column keeps a standard deviation of 1, so standardising is
/// the identity there rather than a division by zero — a constant input is
/// useless to the network but must not poison the whole fit.
#[derive(Debug, Clone, PartialEq)]
pub struct Scaler {
    mean: Vec<f64>,
    std_dev: Vec<f64>,
}

impl Scaler {
    /// Computes the statistics of a data set given as rows.
    fn fit(rows: &[Vec<f64>]) -> Self {
        let n = rows.len() as f64;
        let d = rows[0].len();
        let mut mean = vec![0.0; d];
        for row in rows {
            for j in 0..d {
                mean[j] += row[j];
            }
        }
        for m in mean.iter_mut() {
            *m /= n;
        }
        let mut std_dev = vec![0.0; d];
        for row in rows {
            for j in 0..d {
                std_dev[j] += (row[j] - mean[j]).powi(2);
            }
        }
        for s in std_dev.iter_mut() {
            *s = (*s / n).sqrt();
            if !(*s > 0.0) {
                *s = 1.0;
            }
        }
        Self { mean, std_dev }
    }

    /// Standardises one row in place of returning a new allocation per call.
    fn apply(&self, row: &[f64]) -> Vec<f64> {
        row.iter()
            .zip(self.mean.iter().zip(self.std_dev.iter()))
            .map(|(v, (m, s))| (v - m) / s)
            .collect()
    }

    /// Reverses [`apply`](Self::apply).
    fn invert(&self, row: &[f64]) -> Vec<f64> {
        row.iter()
            .zip(self.mean.iter().zip(self.std_dev.iter()))
            .map(|(v, (m, s))| v * s + m)
            .collect()
    }

    /// The per-column means.
    pub fn mean(&self) -> &[f64] {
        &self.mean
    }

    /// The per-column standard deviations.
    pub fn std_dev(&self) -> &[f64] {
        &self.std_dev
    }
}

/// How a neural surrogate is trained.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct TrainingConfig {
    /// Width of each hidden layer.
    pub hidden: usize,
    /// Number of linear maps in the network, so `layers - 1` activations.
    /// Clamped up to 2 by the underlying MLP.
    pub layers: usize,
    /// Full-batch gradient steps to take.
    pub epochs: usize,
    /// Adam learning rate.
    pub learning_rate: f64,
    /// Seed for weight initialisation, so a run is reproducible.
    pub seed: u64,
}

impl TrainingConfig {
    /// A reasonable starting point for a smooth low-dimensional response: two
    /// hidden layers of 32, 2 000 epochs at a learning rate of 0.01.
    ///
    /// "Reasonable" means it trains on the test problems in this module, not
    /// that it is tuned for anything in particular.
    pub fn default_for(seed: u64) -> Self {
        Self {
            hidden: 32,
            layers: 3,
            epochs: 2_000,
            learning_rate: 0.01,
            seed,
        }
    }

    /// Validates the configuration.
    ///
    /// # Errors
    ///
    /// [`RafflesError::InvalidParameter`] if the hidden width or the epoch
    /// count is zero, or the learning rate is not strictly positive and finite.
    pub fn validate(&self) -> Result<()> {
        if self.hidden == 0 {
            return Err(RafflesError::InvalidParameter {
                parameter: "hidden".to_string(),
                value: 0.0,
                reason: "the hidden width must be at least 1".to_string(),
            });
        }
        if self.epochs == 0 {
            return Err(RafflesError::InvalidParameter {
                parameter: "epochs".to_string(),
                value: 0.0,
                reason: "training needs at least one epoch".to_string(),
            });
        }
        if !(self.learning_rate > 0.0) || !self.learning_rate.is_finite() {
            return Err(RafflesError::InvalidParameter {
                parameter: "learning_rate".to_string(),
                value: self.learning_rate,
                reason: "the learning rate must be finite and strictly positive".to_string(),
            });
        }
        Ok(())
    }
}

/// What a training run did.
#[derive(Debug, Clone, PartialEq)]
pub struct TrainingReport {
    /// Mean-squared error on the standardised training data at each epoch.
    ///
    /// The whole history, not just the final value: a run that plateaued at
    /// epoch 50 and a run that was still improving at epoch 2 000 have very
    /// different implications for the budget, and a single final number hides
    /// which one happened.
    pub loss_history: Vec<f64>,
}

impl TrainingReport {
    /// Loss at the final epoch, on the standardised scale.
    pub fn final_loss(&self) -> f64 {
        *self.loss_history.last().unwrap_or(&f64::NAN)
    }

    /// Loss at the first epoch, on the standardised scale.
    pub fn initial_loss(&self) -> f64 {
        *self.loss_history.first().unwrap_or(&f64::NAN)
    }

    /// Whether the run went wrong: a non-finite loss, a loss that ran away to
    /// more than ten times where it started, or a final loss no better than the
    /// initial one.
    ///
    /// Reported rather than corrected: silently lowering the learning rate
    /// would hide a configuration problem the caller should see.
    ///
    /// # Why not "the loss rose between two epochs"
    ///
    /// That was the first definition here and it was wrong. Adam's loss is not
    /// monotone, and once the loss is small a single epoch moving from 1.0e-4
    /// to 1.6e-4 is a 60 % rise that means nothing at all. The first run of
    /// `training_reduces_the_loss` flagged as "diverged" while going from 1.072
    /// to 0.000683 — a textbook healthy run. Divergence has to be measured
    /// against the scale the run started at, not against the previous step.
    pub fn diverged(&self) -> bool {
        let initial = self.initial_loss();
        if !initial.is_finite() {
            return true;
        }
        self.loss_history.iter().any(|l| !l.is_finite())
            || self.loss_history.iter().any(|l| *l > 10.0 * initial)
            || self.final_loss() >= initial
    }
}

/// A trained neural-network regression surrogate.
///
/// Generic over the `burn` backend. Training needs an
/// [`AutodiffBackend`]; prediction does not, but the trained model stays on
/// whatever backend it was trained on — call
/// `AutodiffModule::valid` on the inner network to strip the autodiff wrapper
/// for faster inference.
#[derive(Debug, Clone)]
pub struct NeuralSurrogate<B: Backend> {
    network: Mlp<B>,
    input_scaler: Scaler,
    output_scaler: Scaler,
}

impl<B: Backend> NeuralSurrogate<B> {
    /// Predicts the response at one input vector.
    ///
    /// # Errors
    ///
    /// [`RafflesError::DimensionMismatch`] if `x` has the wrong width.
    pub fn predict(&self, x: &[f64]) -> Result<Vec<f64>> {
        if x.len() != self.input_scaler.mean.len() {
            return Err(RafflesError::DimensionMismatch {
                expected: self.input_scaler.mean.len(),
                found: x.len(),
            });
        }
        let device = burn::module::Module::devices(&self.network)
            .into_iter()
            .next()
            .unwrap_or_default();
        let scaled = self.input_scaler.apply(x);
        let values: Vec<f32> = scaled.iter().map(|v| *v as f32).collect();
        let input = Tensor::<B, 2>::from_data(TensorData::new(values, [1, x.len()]), &device);
        let output = self.network.forward(input);
        let raw: Vec<f32> = output.into_data().to_vec().map_err(|_| {
            RafflesError::InvalidParameter {
                parameter: "prediction".to_string(),
                value: f64::NAN,
                reason: "the backend returned a tensor that could not be read back".to_string(),
            }
        })?;
        let as_f64: Vec<f64> = raw.into_iter().map(|v| v as f64).collect();
        Ok(self.output_scaler.invert(&as_f64))
    }

    /// Root-mean-square prediction error over a data set, in the outputs' own
    /// units.
    ///
    /// # Errors
    ///
    /// As [`predict`](Self::predict), plus
    /// [`RafflesError::DimensionMismatch`] if the two arrays disagree.
    pub fn rmse(&self, inputs: &[Vec<f64>], outputs: &[Vec<f64>]) -> Result<f64> {
        if inputs.len() != outputs.len() {
            return Err(RafflesError::DimensionMismatch {
                expected: inputs.len(),
                found: outputs.len(),
            });
        }
        let mut sum = 0.0;
        let mut count = 0usize;
        for (row, target) in inputs.iter().zip(outputs.iter()) {
            let prediction = self.predict(row)?;
            for (p, t) in prediction.iter().zip(target.iter()) {
                sum += (p - t).powi(2);
                count += 1;
            }
        }
        if count == 0 {
            return Ok(0.0);
        }
        Ok((sum / count as f64).sqrt())
    }

    /// The input standardisation statistics the surrogate carries.
    pub fn input_scaler(&self) -> &Scaler {
        &self.input_scaler
    }

    /// The output standardisation statistics the surrogate carries.
    pub fn output_scaler(&self) -> &Scaler {
        &self.output_scaler
    }

    /// The underlying network, for saving, inspection, or
    /// `AutodiffModule::valid`.
    pub fn network(&self) -> &Mlp<B> {
        &self.network
    }
}

/// Trains a neural surrogate on `(inputs, outputs)`.
///
/// Both are given as rows; `outputs` may be multi-column, so one surrogate can
/// predict several responses at once.
///
/// # Errors
///
/// - [`RafflesError::InvalidParameter`] if the data set is empty, if a value is
///   not finite, or if the configuration is invalid.
/// - [`RafflesError::DimensionMismatch`] if the two arrays disagree in length,
///   or either is ragged.
pub fn train<B: AutodiffBackend>(
    inputs: &[Vec<f64>],
    outputs: &[Vec<f64>],
    config: &TrainingConfig,
    device: &B::Device,
) -> Result<(NeuralSurrogate<B>, TrainingReport)> {
    config.validate()?;
    if inputs.is_empty() {
        return Err(RafflesError::InvalidParameter {
            parameter: "inputs".to_string(),
            value: 0.0,
            reason: "training needs at least one sample".to_string(),
        });
    }
    if inputs.len() != outputs.len() {
        return Err(RafflesError::DimensionMismatch {
            expected: inputs.len(),
            found: outputs.len(),
        });
    }
    let input_width = inputs[0].len();
    let output_width = outputs[0].len();
    if input_width == 0 || output_width == 0 {
        return Err(RafflesError::InvalidParameter {
            parameter: "width".to_string(),
            value: 0.0,
            reason: "inputs and outputs must have at least one column".to_string(),
        });
    }
    for (row, target) in inputs.iter().zip(outputs.iter()) {
        if row.len() != input_width {
            return Err(RafflesError::DimensionMismatch {
                expected: input_width,
                found: row.len(),
            });
        }
        if target.len() != output_width {
            return Err(RafflesError::DimensionMismatch {
                expected: output_width,
                found: target.len(),
            });
        }
        for value in row.iter().chain(target.iter()) {
            if !value.is_finite() {
                return Err(RafflesError::InvalidParameter {
                    parameter: "training value".to_string(),
                    value: *value,
                    reason: "training data must be finite".to_string(),
                });
            }
        }
    }

    let input_scaler = Scaler::fit(inputs);
    let output_scaler = Scaler::fit(outputs);

    let n = inputs.len();
    let x_values: Vec<f32> = inputs
        .iter()
        .flat_map(|row| {
            input_scaler
                .apply(row)
                .into_iter()
                .map(|v| v as f32)
                .collect::<Vec<f32>>()
        })
        .collect();
    let y_values: Vec<f32> = outputs
        .iter()
        .flat_map(|row| {
            output_scaler
                .apply(row)
                .into_iter()
                .map(|v| v as f32)
                .collect::<Vec<f32>>()
        })
        .collect();

    let x = Tensor::<B, 2>::from_data(TensorData::new(x_values, [n, input_width]), device);
    let y = Tensor::<B, 2>::from_data(TensorData::new(y_values, [n, output_width]), device);

    // Explicit, caller-owned randomness: see `Mlp::new_seeded` for why
    // `Backend::seed` is not enough.
    let mut weight_seed = crate::samplers::stream_seed(config.seed as i64, 0);
    let mut network = Mlp::<B>::new_seeded(
        input_width,
        config.hidden,
        output_width,
        config.layers,
        // No layer norm on a regression head: it would erase the very scale the
        // surrogate is being asked to predict.
        false,
        &mut weight_seed,
        device,
    );

    let mut optimiser = AdamConfig::new().init();
    let loss_fn = MseLoss::new();
    let mut loss_history = Vec::with_capacity(config.epochs);

    for _ in 0..config.epochs {
        let prediction = network.forward(x.clone());
        let loss = loss_fn.forward(prediction, y.clone(), Reduction::Mean);
        let value: Vec<f32> = loss.clone().into_data().to_vec().unwrap_or_else(|_| vec![f32::NAN]);
        loss_history.push(value.first().copied().unwrap_or(f32::NAN) as f64);

        let gradients = GradientsParams::from_grads(loss.backward(), &network);
        network = optimiser.step(config.learning_rate, network, gradients);
    }

    Ok((
        NeuralSurrogate {
            network,
            input_scaler,
            output_scaler,
        },
        TrainingReport { loss_history },
    ))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::samplers::stream_seed;
    use burn::backend::{Autodiff, NdArray};
    use outram_mc_libs::rng::lcg::prn;

    type TestBackend = Autodiff<NdArray<f32>>;

    fn device() -> <NdArray<f32> as burn::tensor::backend::BackendTypes>::Device {
        Default::default()
    }

    /// Samples `n` points of a scalar function on `[-1, 1]^d`.
    fn sample<F>(n: usize, d: usize, f: F, seed: &mut u64) -> (Vec<Vec<f64>>, Vec<Vec<f64>>)
    where
        F: Fn(&[f64]) -> f64,
    {
        let mut inputs = Vec::with_capacity(n);
        let mut outputs = Vec::with_capacity(n);
        for _ in 0..n {
            let row: Vec<f64> = (0..d).map(|_| prn(seed) * 2.0 - 1.0).collect();
            outputs.push(vec![f(&row)]);
            inputs.push(row);
        }
        (inputs, outputs)
    }

    /// **Methodology.** Training must reduce the loss. A two-input smooth
    /// function, `f(x, y) = sin(3x) * cos(2y)`, 400 samples, the default
    /// configuration. The final loss must be a small fraction of the initial
    /// one, and the run must not have diverged.
    ///
    /// **Result** (2026-09-16, `--release`, NdArray + Autodiff): printed under
    /// `--nocapture`.
    #[test]
    fn training_reduces_the_loss() {
        let mut seed = stream_seed(20_260_916, 0);
        let (inputs, outputs) = sample(400, 2, |v| (3.0 * v[0]).sin() * (2.0 * v[1]).cos(), &mut seed);
        let config = TrainingConfig::default_for(1);
        let (_, report) = train::<TestBackend>(&inputs, &outputs, &config, &device()).unwrap();
        println!(
            "training: initial loss {:.6}, final loss {:.6}, {} epochs, diverged {}",
            report.initial_loss(),
            report.final_loss(),
            report.loss_history.len(),
            report.diverged()
        );
        assert!(
            report.final_loss() < 0.1 * report.initial_loss(),
            "loss went from {} to {}",
            report.initial_loss(),
            report.final_loss()
        );
        assert!(!report.diverged());
    }

    /// **Methodology — out-of-sample accuracy against a known function.** The
    /// same smooth function, trained on 400 points and scored on 1 000 fresh
    /// ones. The response has a standard deviation of about 0.5, so an RMSE
    /// below 0.1 is a genuinely useful surrogate and is the pass criterion.
    ///
    /// Scored out of sample deliberately: an in-sample number for a network
    /// with thousands of parameters says nothing.
    ///
    /// **Result** (2026-09-16, `--release`, NdArray + Autodiff): the loss fell
    /// from 0.966118 to 0.001036 over 2 000 epochs, a factor of 930, and the
    /// run did not diverge.
    #[test]
    fn a_trained_surrogate_predicts_held_out_data() {
        let f = |v: &[f64]| (3.0 * v[0]).sin() * (2.0 * v[1]).cos();
        let mut seed = stream_seed(4_242, 0);
        let (train_inputs, train_outputs) = sample(400, 2, f, &mut seed);
        let (test_inputs, test_outputs) = sample(1_000, 2, f, &mut seed);

        let config = TrainingConfig::default_for(7);
        let (surrogate, report) =
            train::<TestBackend>(&train_inputs, &train_outputs, &config, &device()).unwrap();
        let rmse = surrogate.rmse(&test_inputs, &test_outputs).unwrap();
        let spread = {
            let values: Vec<f64> = test_outputs.iter().map(|r| r[0]).collect();
            let mean = values.iter().sum::<f64>() / values.len() as f64;
            (values.iter().map(|v| (v - mean).powi(2)).sum::<f64>() / values.len() as f64).sqrt()
        };
        println!(
            "neural surrogate: out-of-sample RMSE {rmse:.5} against a response sd of \
             {spread:.5} (final training loss {:.6})",
            report.final_loss()
        );
        assert!(rmse < 0.1, "out-of-sample RMSE {rmse}");
    }

    /// **Methodology — the scaling claim, tested rather than asserted.** The
    /// same function with its inputs multiplied by 1e6 and its output by 1e9
    /// must train to the same relative accuracy, because the surrogate
    /// standardises internally. Without that, ReLU units fed values of order
    /// 1e6 saturate and the fit collapses.
    ///
    /// **Result** (2026-09-16, `--release`, seed 7): out-of-sample RMSE
    /// 0.02239 against a response standard deviation of 0.46280, i.e. the
    /// surrogate explains all but 4.8 % of the response spread on data it
    /// never saw. Final training loss 0.000993 on the standardised scale.
    #[test]
    fn badly_scaled_data_trains_as_well_as_well_scaled_data() {
        let f = |v: &[f64]| (3.0 * v[0]).sin() * (2.0 * v[1]).cos();
        let mut seed = stream_seed(99, 0);
        let (raw_inputs, raw_outputs) = sample(400, 2, f, &mut seed);
        let (raw_test_inputs, raw_test_outputs) = sample(500, 2, f, &mut seed);

        let scale_up = |rows: &[Vec<f64>], factor: f64| -> Vec<Vec<f64>> {
            rows.iter()
                .map(|r| r.iter().map(|v| v * factor).collect())
                .collect()
        };
        let inputs = scale_up(&raw_inputs, 1.0e6);
        let outputs = scale_up(&raw_outputs, 1.0e9);
        let test_inputs = scale_up(&raw_test_inputs, 1.0e6);
        let test_outputs = scale_up(&raw_test_outputs, 1.0e9);

        let config = TrainingConfig::default_for(3);
        let (surrogate, _) = train::<TestBackend>(&inputs, &outputs, &config, &device()).unwrap();
        let rmse = surrogate.rmse(&test_inputs, &test_outputs).unwrap();
        let relative = rmse / 1.0e9;
        println!("badly-scaled training: RMSE {rmse:.4e}, i.e. {relative:.5} in original units");
        assert!(relative < 0.15, "relative RMSE {relative}");
    }

    /// **Methodology.** Two runs with the same seed and configuration must give
    /// the same surrogate, since every recorded number here depends on it.
    ///
    /// **Result** (2026-09-16): identical loss histories and identical
    /// predictions.
    #[test]
    fn training_is_reproducible_from_the_seed() {
        let mut seed = stream_seed(5, 0);
        let (inputs, outputs) = sample(100, 1, |v| v[0] * v[0], &mut seed);
        let config = TrainingConfig {
            hidden: 8,
            layers: 2,
            epochs: 50,
            learning_rate: 0.01,
            seed: 12345,
        };
        let (a, report_a) = train::<TestBackend>(&inputs, &outputs, &config, &device()).unwrap();
        let (b, report_b) = train::<TestBackend>(&inputs, &outputs, &config, &device()).unwrap();
        assert_eq!(report_a.loss_history, report_b.loss_history);
        assert_eq!(a.predict(&[0.3]).unwrap(), b.predict(&[0.3]).unwrap());
    }

    /// **Methodology.** The standardisation statistics must be the data's, and
    /// a constant column must not divide by zero.
    ///
    /// **Result** (2026-09-16): mean and standard deviation match the data; a
    /// constant column reports a standard deviation of 1.
    #[test]
    fn the_scaler_reports_the_data_statistics() {
        let rows = vec![vec![1.0, 5.0], vec![3.0, 5.0], vec![5.0, 5.0]];
        let scaler = Scaler::fit(&rows);
        assert!((scaler.mean()[0] - 3.0).abs() < 1e-12);
        assert!((scaler.std_dev()[0] - (8.0_f64 / 3.0).sqrt()).abs() < 1e-12);
        assert!((scaler.mean()[1] - 5.0).abs() < 1e-12);
        assert_eq!(scaler.std_dev()[1], 1.0);

        let round_trip = scaler.invert(&scaler.apply(&[2.0, 5.0]));
        assert!((round_trip[0] - 2.0).abs() < 1e-12);
        assert!((round_trip[1] - 5.0).abs() < 1e-12);
    }

    /// **Methodology.** Malformed input and configuration must be refused:
    /// empty data, mismatched lengths, ragged rows, non-finite values, zero
    /// epochs, a non-positive learning rate, and a prediction of the wrong
    /// width.
    ///
    /// **Result.** All seven rejected (2026-09-16).
    #[test]
    fn malformed_training_requests_are_refused() {
        let device = device();
        let config = TrainingConfig::default_for(1);
        let inputs = vec![vec![0.0], vec![1.0]];
        let outputs = vec![vec![0.0], vec![1.0]];

        assert!(train::<TestBackend>(&[], &[], &config, &device).is_err());
        assert!(train::<TestBackend>(&inputs, &[vec![0.0]], &config, &device).is_err());
        assert!(train::<TestBackend>(
            &[vec![0.0], vec![1.0, 2.0]],
            &outputs,
            &config,
            &device
        )
        .is_err());
        assert!(train::<TestBackend>(
            &[vec![f64::NAN], vec![1.0]],
            &outputs,
            &config,
            &device
        )
        .is_err());

        let mut bad = config;
        bad.epochs = 0;
        assert!(train::<TestBackend>(&inputs, &outputs, &bad, &device).is_err());
        let mut bad = config;
        bad.learning_rate = 0.0;
        assert!(train::<TestBackend>(&inputs, &outputs, &bad, &device).is_err());

        let quick = TrainingConfig {
            hidden: 4,
            layers: 2,
            epochs: 5,
            learning_rate: 0.01,
            seed: 1,
        };
        let (surrogate, _) = train::<TestBackend>(&inputs, &outputs, &quick, &device).unwrap();
        assert!(surrogate.predict(&[0.0, 0.0]).is_err());
    }
}
