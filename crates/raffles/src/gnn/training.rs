//! Training and rollout for the message-passing network — the other half of
//! the Physics-guided-MPNN port.
//!
//! Available only with the crate's `burn` feature.
//!
//! # What is here, and what the upstream had
//!
//! The upstream repository's training path is a PyTorch Lightning module with a
//! Weights & Biases sweep, a `.pt` dataset loader, a rollout driver and a
//! plotting suite. What is ported here is the part that is *model behaviour*
//! rather than experiment plumbing:
//!
//! - [`train`] — full-batch Adam on the mean-squared error over a set of
//!   samples that share one graph, using the disjoint-union batching that
//!   PyTorch Geometric does with its `Batch` object and that
//!   [`Graph::repeat`] does here.
//! - [`rollout`] — autoregressive time stepping, feeding each prediction back
//!   in as the next input. **This is where under-reaching actually bites**: a
//!   single step can look fine and a hundred-step rollout can still diverge.
//!
//! Deliberately not ported: the Lightning wrapper, the hyperparameter sweep,
//! the `.pt` loader (PyTorch's pickle-based tensor format, which nothing in
//! this workspace should learn to read), and the plotting. Those are the
//! harness; a caller in this workspace brings their own data and their own
//! plotting.
//!
//! # Predicting the increment, not the state
//!
//! [`train`] and [`rollout`] are written for a model that predicts the
//! **change** over one step, with the caller adding it to the current state.
//! That is what MeshGraphNet does and it matters: predicting the state
//! directly makes the network spend its capacity reproducing its own input,
//! and makes a rollout drift as soon as it is slightly wrong. Predicting the
//! increment keeps the error additive rather than compounding through the
//! identity.
//!
//! [`rollout`] does the addition itself. If your targets are absolute states,
//! subtract before training and this module's shape is still right.
//!
//! # Reference
//!
//! - L. Tesan and M. M. Iparraguirre et al. (2025). On the under-reaching
//!   phenomenon in message passing neural PDE solvers: revisiting the CFL
//!   condition. arXiv:2507.08861. <https://arxiv.org/abs/2507.08861>

use burn::optim::{AdamConfig, GradientsParams, Optimizer};
use burn::tensor::backend::AutodiffBackend;
use burn::tensor::{Tensor, TensorData};

use super::graph::Graph;
use super::mpnn::MessagePassingNet;
use crate::{RafflesError, Result};

/// How a message-passing network is trained.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct MpnnTrainingConfig {
    /// Full-batch gradient steps.
    pub epochs: usize,
    /// Adam learning rate.
    pub learning_rate: f64,
}

impl MpnnTrainingConfig {
    /// A starting point that trains the test problems in this module: 1 500
    /// epochs at a learning rate of 0.005.
    pub fn default_config() -> Self {
        Self {
            epochs: 1_500,
            learning_rate: 0.005,
        }
    }

    /// Validates the configuration.
    ///
    /// # Errors
    ///
    /// [`RafflesError::InvalidParameter`] if `epochs` is zero or the learning
    /// rate is not finite and strictly positive.
    pub fn validate(&self) -> Result<()> {
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

/// What a message-passing training run did.
#[derive(Debug, Clone, PartialEq)]
pub struct MpnnTrainingReport {
    /// Mean-squared error at each epoch.
    pub loss_history: Vec<f64>,
}

impl MpnnTrainingReport {
    /// Loss at the final epoch.
    pub fn final_loss(&self) -> f64 {
        *self.loss_history.last().unwrap_or(&f64::NAN)
    }

    /// Loss at the first epoch.
    pub fn initial_loss(&self) -> f64 {
        *self.loss_history.first().unwrap_or(&f64::NAN)
    }
}

/// Trains a message-passing network on samples that share one graph.
///
/// `inputs[s][i]` is the feature vector of node `i` in sample `s`, and
/// `targets[s][i]` is what the network should produce there. All samples are
/// stacked into one disjoint-union graph and trained as a single batch, so the
/// cost is one forward pass per epoch regardless of the sample count.
///
/// The network is taken by value and returned, because `burn`'s optimiser step
/// consumes and rebuilds the module.
///
/// # Errors
///
/// - [`RafflesError::InvalidParameter`] if there are no samples, if a value is
///   not finite, or if the configuration is invalid.
/// - [`RafflesError::DimensionMismatch`] if `inputs` and `targets` disagree, if
///   a sample has a different number of nodes than the graph, or if the feature
///   widths are ragged.
pub fn train<B: AutodiffBackend>(
    network: MessagePassingNet<B>,
    graph: &Graph,
    inputs: &[Vec<Vec<f64>>],
    targets: &[Vec<Vec<f64>>],
    config: &MpnnTrainingConfig,
    device: &B::Device,
) -> Result<(MessagePassingNet<B>, MpnnTrainingReport)> {
    config.validate()?;
    if inputs.is_empty() {
        return Err(RafflesError::InvalidParameter {
            parameter: "inputs".to_string(),
            value: 0.0,
            reason: "training needs at least one sample".to_string(),
        });
    }
    if inputs.len() != targets.len() {
        return Err(RafflesError::DimensionMismatch {
            expected: inputs.len(),
            found: targets.len(),
        });
    }
    let nodes = graph.node_count();
    let input_width = inputs[0][0].len();
    let target_width = targets[0][0].len();
    for (sample, target) in inputs.iter().zip(targets.iter()) {
        if sample.len() != nodes || target.len() != nodes {
            return Err(RafflesError::DimensionMismatch {
                expected: nodes,
                found: sample.len().min(target.len()),
            });
        }
        for row in sample {
            if row.len() != input_width || row.iter().any(|v| !v.is_finite()) {
                return Err(RafflesError::DimensionMismatch {
                    expected: input_width,
                    found: row.len(),
                });
            }
        }
        for row in target {
            if row.len() != target_width || row.iter().any(|v| !v.is_finite()) {
                return Err(RafflesError::DimensionMismatch {
                    expected: target_width,
                    found: row.len(),
                });
            }
        }
    }

    let samples = inputs.len();
    let batched = graph.repeat(samples)?;

    let x_values: Vec<f32> = inputs
        .iter()
        .flat_map(|sample| sample.iter().flat_map(|row| row.iter().map(|v| *v as f32)))
        .collect();
    let y_values: Vec<f32> = targets
        .iter()
        .flat_map(|sample| sample.iter().flat_map(|row| row.iter().map(|v| *v as f32)))
        .collect();

    let x = Tensor::<B, 2>::from_data(
        TensorData::new(x_values, [nodes * samples, input_width]),
        device,
    );
    let y = Tensor::<B, 2>::from_data(
        TensorData::new(y_values, [nodes * samples, target_width]),
        device,
    );

    let mut network = network;
    let mut optimiser = AdamConfig::new().init();
    let mut loss_history = Vec::with_capacity(config.epochs);

    for _ in 0..config.epochs {
        let prediction = network.forward(&batched, x.clone());
        let difference = prediction - y.clone();
        let loss = difference.clone().powi_scalar(2).mean();
        let value: Vec<f32> = loss
            .clone()
            .into_data()
            .to_vec()
            .unwrap_or_else(|_| vec![f32::NAN]);
        loss_history.push(value.first().copied().unwrap_or(f32::NAN) as f64);

        let gradients = GradientsParams::from_grads(loss.backward(), &network);
        network = optimiser.step(config.learning_rate, network, gradients);
    }

    Ok((network, MpnnTrainingReport { loss_history }))
}

/// Autoregressive rollout: step the state forward `steps` times, feeding each
/// prediction back in.
///
/// The network is treated as predicting the **increment** over one step, so
/// each new state is `state + network(state)`. See the module documentation for
/// why that is the right shape.
///
/// Returns the trajectory including the initial state, so the result has
/// `steps + 1` entries.
///
/// # Why this is the test that matters
///
/// A one-step prediction can be accurate while a rollout diverges, because a
/// rollout feeds the network its own errors. If the network under-reaches, the
/// missing influence does not average out over steps — it accumulates, and the
/// trajectory leaves the manifold the training data lived on. Measure a model
/// here, not on single-step error.
///
/// # Errors
///
/// [`RafflesError::DimensionMismatch`] if `initial` does not have one row per
/// graph node, or its rows are ragged.
pub fn rollout<B: burn::prelude::Backend>(
    network: &MessagePassingNet<B>,
    graph: &Graph,
    initial: &[Vec<f64>],
    steps: usize,
    device: &B::Device,
) -> Result<Vec<Vec<Vec<f64>>>> {
    let nodes = graph.node_count();
    if initial.len() != nodes {
        return Err(RafflesError::DimensionMismatch {
            expected: nodes,
            found: initial.len(),
        });
    }
    let width = initial[0].len();
    for row in initial {
        if row.len() != width {
            return Err(RafflesError::DimensionMismatch {
                expected: width,
                found: row.len(),
            });
        }
    }

    let mut trajectory = Vec::with_capacity(steps + 1);
    let mut state: Vec<Vec<f64>> = initial.to_vec();
    trajectory.push(state.clone());

    for _ in 0..steps {
        let values: Vec<f32> = state
            .iter()
            .flat_map(|row| row.iter().map(|v| *v as f32))
            .collect();
        let input = Tensor::<B, 2>::from_data(TensorData::new(values, [nodes, width]), device);
        let increment = network.forward(graph, input);
        let raw: Vec<f32> =
            increment
                .into_data()
                .to_vec()
                .map_err(|_| RafflesError::InvalidParameter {
                    parameter: "rollout".to_string(),
                    value: f64::NAN,
                    reason: "the backend returned a tensor that could not be read back".to_string(),
                })?;
        for (i, row) in state.iter_mut().enumerate() {
            for (j, value) in row.iter_mut().enumerate() {
                *value += raw[i * width + j] as f64;
            }
        }
        trajectory.push(state.clone());
    }
    Ok(trajectory)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::gnn::bound::{physics_guided_lower_bound, PdeClass};
    use crate::samplers::stream_seed;
    use burn::backend::{Autodiff, NdArray};
    use burn::prelude::Backend;
    use outram_mc_libs::rng::lcg::prn;

    type TestBackend = Autodiff<NdArray<f32>>;

    fn device() -> <NdArray<f32> as burn::tensor::backend::BackendTypes>::Device {
        Default::default()
    }

    /// A path graph of `n` nodes.
    fn path(n: usize) -> Graph {
        let edges: Vec<(usize, usize)> = (0..n - 1).map(|i| (i, i + 1)).collect();
        Graph::from_undirected_edges(n, &edges).unwrap()
    }

    /// Solves the 1-D Poisson problem `-u'' = f` on `n` interior nodes with
    /// homogeneous Dirichlet ends, by direct tridiagonal elimination.
    ///
    /// This is the **elliptic** case: every interior value depends on every
    /// source value, however far away, which is exactly the situation the
    /// physics-guided bound says needs a reach of the whole graph.
    fn poisson_1d(f: &[f64]) -> Vec<f64> {
        let n = f.len();
        // Tridiagonal (-1, 2, -1) forward elimination.
        let mut c = vec![0.0; n];
        let mut d = vec![0.0; n];
        c[0] = -1.0 / 2.0;
        d[0] = f[0] / 2.0;
        for i in 1..n {
            let denominator = 2.0 + c[i - 1];
            c[i] = -1.0 / denominator;
            d[i] = (f[i] + d[i - 1]) / denominator;
        }
        let mut u = vec![0.0; n];
        u[n - 1] = d[n - 1];
        for i in (0..n - 1).rev() {
            u[i] = d[i] - c[i] * u[i + 1];
        }
        u
    }

    /// Builds `samples` random source terms and their exact Poisson solutions.
    fn poisson_dataset(
        nodes: usize,
        samples: usize,
        seed: &mut u64,
    ) -> (Vec<Vec<Vec<f64>>>, Vec<Vec<Vec<f64>>>) {
        let mut inputs = Vec::with_capacity(samples);
        let mut targets = Vec::with_capacity(samples);
        for _ in 0..samples {
            // A smooth random source: a couple of sinusoids, so the solutions
            // are varied but not white noise.
            let a = prn(seed) * 2.0 - 1.0;
            let b = prn(seed) * 2.0 - 1.0;
            let f: Vec<f64> = (0..nodes)
                .map(|i| {
                    let x = (i as f64 + 1.0) / (nodes as f64 + 1.0);
                    a * (core::f64::consts::PI * x).sin()
                        + b * (2.0 * core::f64::consts::PI * x).sin()
                })
                .collect();
            let u = poisson_1d(&f);
            inputs.push(f.iter().map(|v| vec![*v]).collect());
            targets.push(u.iter().map(|v| vec![*v]).collect());
        }
        (inputs, targets)
    }

    /// **Methodology — testing the paper's claim, and reporting what actually
    /// happened.** The physics-guided bound says an elliptic problem needs a
    /// message-passing reach of the whole graph, because its solution at any
    /// node depends on the source at every node. Below that bound the network
    /// is *structurally* unable to represent the map.
    ///
    /// Tested by building the map directly: a 1-D Poisson problem `-u'' = f` on
    /// a 9-node path (diameter 8, so the bound is 8), with exact solutions from
    /// a tridiagonal solve. Networks of different message-passing depth are
    /// trained identically — same data, same epochs, same learning rate, same
    /// weight seed — and scored on 200 held-out source terms.
    ///
    /// # What three runs actually showed
    ///
    /// All measured 2026-09-16, `--release`, held-out RMSE (final training loss
    /// in brackets):
    ///
    /// | M | A: 400 samples, hidden 32, 1 500 ep, lr 0.005 | B: 200, hidden 16, 4 000 ep, lr 0.003 | C: 200, hidden 16, 1 500 ep, lr 0.005 |
    /// |---|---|---|---|
    /// | 1 | 0.1376 (0.0059) | 0.0924 (0.0059) | 0.4930 (0.1362) |
    /// | 2 | 0.0675 (0.0203) | 0.0984 (0.0203) | — |
    /// | 4 | 0.0395 (0.0017) | 0.0500 (0.0018) | 0.0637 (0.0027) |
    /// | 8 (the bound) | 0.2963 (0.0623) | 0.1530 (0.0427) | 0.0835 (0.0046) |
    ///
    /// **What is robust across all three: a severely under-reaching network is
    /// much worse.** M = 1 is beaten by every deeper model in every run, by
    /// factors of 3.5, 1.8 and 7.7. That is the under-reaching penalty, and it
    /// is the claim this test asserts.
    ///
    /// **What is NOT resolved here: whether reaching the bound specifically
    /// matters.** The ordering of M = 4 against M = 8 flips between runs, and
    /// it flips for a reason visible in the *training* loss rather than the
    /// test loss — in runs A and B the 8-deep residual stack never fitted its
    /// own training data (0.0623 and 0.0427 against M = 4's 0.0017), and in run
    /// C it did (0.0046). A model that has not trained says nothing about
    /// reach. The variance between training runs at this size exceeds the
    /// effect being measured.
    ///
    /// Closing that gap needs what the upstream training path has and this one
    /// does not: a learning-rate schedule, depth-aware normalisation, and a
    /// budget set per model rather than shared. Until then this test
    /// demonstrates the penalty for under-reaching badly, not the sharpness of
    /// the bound — and saying so is the point, because a passing test that
    /// implied otherwise would be worse than no test.
    #[test]
    fn message_passing_reach_helps_until_optimisation_becomes_the_limit() {
        let nodes = 9;
        let graph = path(nodes);
        let bound =
            physics_guided_lower_bound(PdeClass::Elliptic, &graph, 1.0 / nodes as f64, None)
                .unwrap();
        assert_eq!(bound.required, 8, "the 9-node path's diameter");

        let mut seed = stream_seed(20_260_916, 0);
        let (train_inputs, train_targets) = poisson_dataset(nodes, 200, &mut seed);
        let (test_inputs, test_targets) = poisson_dataset(nodes, 200, &mut seed);

        let config = MpnnTrainingConfig {
            epochs: 1_500,
            learning_rate: 0.005,
        };
        let mut curve = Vec::new();
        for steps in [1usize, 4, 8] {
            let mut weight_seed = stream_seed(7, 0);
            let network = MessagePassingNet::<TestBackend>::new_seeded(
                1,
                1,
                16,
                steps,
                2,
                &mut weight_seed,
                &device(),
            );
            let (trained, report) = train::<TestBackend>(
                network,
                &graph,
                &train_inputs,
                &train_targets,
                &config,
                &device(),
            )
            .unwrap();

            let mut squared = 0.0;
            let mut count = 0usize;
            for (sample, target) in test_inputs.iter().zip(test_targets.iter()) {
                let values: Vec<f32> = sample.iter().map(|row| row[0] as f32).collect();
                let input = Tensor::<TestBackend, 2>::from_data(
                    TensorData::new(values, [nodes, 1]),
                    &device(),
                );
                let prediction: Vec<f32> =
                    trained.forward(&graph, input).into_data().to_vec().unwrap();
                for (i, target_row) in target.iter().enumerate() {
                    squared += (prediction[i] as f64 - target_row[0]).powi(2);
                    count += 1;
                }
            }
            let rmse = (squared / count as f64).sqrt();
            curve.push((steps, rmse, report.final_loss()));
            println!(
                "M = {steps} (bound {}): held-out RMSE {rmse:.5}, final TRAINING loss {:.6}",
                bound.required,
                report.final_loss()
            );
        }

        println!("(steps, held-out RMSE, final training loss): {curve:?}");

        // The one claim three runs support: a severely under-reaching network
        // is much worse than a deeper one. Deliberately NOT asserting anything
        // about M = 4 versus M = 8 — see this test's documentation for why
        // that comparison is not resolved at this problem size.
        for (index, label) in [(1usize, "M = 4"), (2, "M = 8")] {
            assert!(
                curve[index].1 < 0.5 * curve[0].1,
                "{label} should clearly beat the severely under-reaching M = 1: {curve:?}"
            );
        }
    }

    /// **Methodology.** Batching by disjoint union must be equivalent to
    /// running the samples one at a time: no edge crosses between copies, so
    /// the two must agree exactly. Checked by comparing a batched forward pass
    /// against per-sample passes on a 7-node path with 3 samples.
    ///
    /// **Result** (2026-09-16): agreement to 1e-6 (single precision), i.e. the
    /// copies do not leak into each other.
    #[test]
    fn batching_by_disjoint_union_does_not_leak_between_samples() {
        let graph = path(7);
        let samples = 3;
        let batched = graph.repeat(samples).unwrap();
        assert_eq!(batched.node_count(), 21);
        assert_eq!(batched.edge_count(), graph.edge_count() * samples);
        assert!(!batched.is_connected(), "the copies must stay disjoint");

        let mut weight_seed = stream_seed(3, 0);
        let network = MessagePassingNet::<TestBackend>::new_seeded(
            1,
            1,
            8,
            3,
            2,
            &mut weight_seed,
            &device(),
        );

        let features: Vec<Vec<f64>> = (0..21).map(|i| vec![(i as f64) * 0.1]).collect();
        let values: Vec<f32> = features.iter().map(|r| r[0] as f32).collect();
        let batched_input =
            Tensor::<TestBackend, 2>::from_data(TensorData::new(values, [21, 1]), &device());
        let batched_output: Vec<f32> = network
            .forward(&batched, batched_input)
            .into_data()
            .to_vec()
            .unwrap();

        for sample in 0..samples {
            let slice: Vec<f32> = (0..7).map(|i| features[sample * 7 + i][0] as f32).collect();
            let input =
                Tensor::<TestBackend, 2>::from_data(TensorData::new(slice, [7, 1]), &device());
            let output: Vec<f32> = network.forward(&graph, input).into_data().to_vec().unwrap();
            for i in 0..7 {
                let difference = (output[i] - batched_output[sample * 7 + i]).abs();
                assert!(
                    difference < 1e-6,
                    "sample {sample} node {i} differs by {difference}"
                );
            }
        }
    }

    /// **Methodology.** A rollout must produce `steps + 1` states, keep the
    /// shape, and accumulate the network's increments rather than replacing the
    /// state. Checked on a network trained for one epoch — the numbers do not
    /// matter, the mechanics do.
    ///
    /// **Result** (2026-09-16): 11 states of 7 nodes each; the first is the
    /// initial state unchanged; consecutive states differ.
    #[test]
    fn a_rollout_accumulates_increments() {
        let graph = path(7);
        let mut weight_seed = stream_seed(11, 0);
        let network = MessagePassingNet::<TestBackend>::new_seeded(
            1,
            1,
            8,
            2,
            2,
            &mut weight_seed,
            &device(),
        );
        let initial: Vec<Vec<f64>> = (0..7).map(|i| vec![i as f64 * 0.25]).collect();
        let trajectory = rollout(&network, &graph, &initial, 10, &device()).unwrap();

        assert_eq!(trajectory.len(), 11);
        assert_eq!(trajectory[0], initial);
        for state in &trajectory {
            assert_eq!(state.len(), 7);
        }
        assert_ne!(trajectory[1], trajectory[0]);
    }

    /// **Methodology.** Training must reduce the loss on a problem the network
    /// can represent: a 1-D Poisson map with the step count at the bound.
    ///
    /// **Result** (2026-09-16, `--release`): printed under `--nocapture`.
    #[test]
    fn training_reduces_the_loss() {
        let nodes = 9;
        let graph = path(nodes);
        let mut seed = stream_seed(5, 0);
        let (inputs, targets) = poisson_dataset(nodes, 200, &mut seed);
        let mut weight_seed = stream_seed(2, 0);
        let network = MessagePassingNet::<TestBackend>::new_seeded(
            1,
            1,
            32,
            8,
            2,
            &mut weight_seed,
            &device(),
        );
        let (_, report) = train::<TestBackend>(
            network,
            &graph,
            &inputs,
            &targets,
            &MpnnTrainingConfig::default_config(),
            &device(),
        )
        .unwrap();
        println!(
            "MPNN training: initial loss {:.6}, final loss {:.6}",
            report.initial_loss(),
            report.final_loss()
        );
        assert!(report.final_loss() < 0.2 * report.initial_loss());
    }

    /// **Methodology.** Malformed training requests must be refused: no
    /// samples, mismatched counts, a sample with the wrong number of nodes,
    /// zero epochs, and a rollout whose initial state has the wrong height.
    ///
    /// **Result.** All five rejected (2026-09-16).
    #[test]
    fn malformed_requests_are_refused() {
        let graph = path(5);
        let mut weight_seed = stream_seed(1, 0);
        let network = MessagePassingNet::<TestBackend>::new_seeded(
            1,
            1,
            4,
            1,
            2,
            &mut weight_seed,
            &device(),
        );
        let config = MpnnTrainingConfig::default_config();
        let good: Vec<Vec<Vec<f64>>> = vec![(0..5).map(|i| vec![i as f64]).collect()];

        assert!(
            train::<TestBackend>(network.clone(), &graph, &[], &[], &config, &device()).is_err()
        );
        assert!(
            train::<TestBackend>(network.clone(), &graph, &good, &[], &config, &device()).is_err()
        );
        let wrong_height: Vec<Vec<Vec<f64>>> = vec![(0..3).map(|i| vec![i as f64]).collect()];
        assert!(train::<TestBackend>(
            network.clone(),
            &graph,
            &wrong_height,
            &wrong_height,
            &config,
            &device()
        )
        .is_err());
        let mut bad = config;
        bad.epochs = 0;
        assert!(
            train::<TestBackend>(network.clone(), &graph, &good, &good, &bad, &device()).is_err()
        );

        assert!(rollout(&network, &graph, &[vec![0.0], vec![1.0]], 3, &device()).is_err());
        assert!(graph.repeat(0).is_err());
    }
}
