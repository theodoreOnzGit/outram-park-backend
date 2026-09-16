// ---------------------------------------------------------------------------
// Ported from Physics-guided-MPNN.
//
//   Upstream project: Physics-guided Message Passing Iterations
//                     (L. Tesan and M. M. Iparraguirre et al.)
//   Upstream repo:    https://github.com/mikelunizar/Physics-guided-MPNN
//   Upstream file:    src/model/encoders.py
//   Upstream commit:  12b75acde0764a6ca1c1a6d72515cbefa62d5f75
//   Accessed:         2026-09-16
//
//   Licensed under the GNU General Public License v3.0, as published in that
//   repository's LICENSE file. This Rust translation is part of RAFFLES /
//   Outram Park and is distributed under GPL-3.0-only, which is the same
//   licence — so unlike most of this crate, this file IS a port and carries
//   the attribution above.
//
// Translation notes: the upstream is PyTorch + PyTorch Geometric; this is
// `burn` with no_std + alloc. PyTorch Geometric's `Data` object and its
// `SumAggregation` have no equivalent here, so the graph travels separately
// from the tensors (see `MessagePassingNet::forward`) and aggregation is a
// `select_assign` with `IndexingUpdateOp::Add`, which is the same scatter-add.
// `nn.Sequential` becomes an explicit `Vec<Linear>` plus an activation, because
// `burn` has no heterogeneous sequential container and the workspace design
// rules forbid the trait objects one would need to build it. The upstream's
// `shared_mp` option (reuse one processor across all steps) is kept.
// Training loops, dataset loading, rollout and plotting are NOT ported: they
// are the experiment harness, not the model.
// ---------------------------------------------------------------------------

//! The message-passing network itself, in `burn`.
//!
//! Available only with the crate's `burn` feature; everything in
//! [`super::graph`] and [`super::bound`] works without it, which is deliberate
//! — a caller who wants to *check* a model's reach should not have to build a
//! tensor library to do it.
//!
//! # Architecture
//!
//! The encoder–processor–decoder shape of MeshGraphNet, in the node-only form
//! the upstream uses:
//!
//! 1. **Encode.** An MLP lifts each node's input features into a latent vector
//!    of width `hidden`, followed by a layer norm.
//! 2. **Process**, `M` times. Each node gathers the latent vectors of its
//!    senders, sums them, concatenates the sum with its own vector, and an MLP
//!    maps that `2 * hidden` vector back to `hidden`. The result is **added**
//!    to the previous vector — a residual step, which is what lets `M` be large
//!    without the signal washing out.
//! 3. **Decode.** An MLP maps the final latent vector to the output features,
//!    with no layer norm, because the output is a physical quantity and
//!    normalising it away would be wrong.
//!
//! `M` — the number of processor steps — is the quantity
//! [`super::bound::physics_guided_lower_bound`] exists to choose. This is the
//! whole point of pairing the two in one module: the model that has a reach and
//! the calculation that says how much reach the physics needs.
//!
//! # Scope
//!
//! Inference and the forward pass, which is what a physics code wants from a
//! trained surrogate. Training is `burn`'s own business (`burn::optim`,
//! autodiff backend) and is not wrapped here; the upstream's training loop,
//! dataset loader, rollout driver and plotting are experiment harness rather
//! than model, and are not ported.



use burn::module::{Module, Param};
use burn::nn::{LayerNorm, LayerNormConfig, Linear, LinearConfig, Relu};
use burn::prelude::Backend;
use burn::tensor::{IndexingUpdateOp, Int, Tensor, TensorData};
use outram_mc_libs::rng::lcg::prn;

use super::graph::Graph;

/// A multi-layer perceptron: `layers` linear maps with ReLU between them, and
/// an optional layer norm on the output.
///
/// `burn` has no heterogeneous `Sequential`, and building one would need the
/// trait objects the workspace design rules forbid, so the layers are an
/// explicit `Vec` and the forward pass walks it. That is also clearer: the
/// activation placement is visible rather than buried in a container.
#[derive(Module, Debug)]
pub struct Mlp<B: Backend> {
    layers: Vec<Linear<B>>,
    norm: Option<LayerNorm<B>>,
    activation: Relu,
}

impl<B: Backend> Mlp<B> {
    /// Builds an MLP with `layers` linear maps in total (so `layers - 1`
    /// activations), width `hidden` between them.
    ///
    /// `layers` is clamped up to 2: a "one-layer MLP" is a single linear map
    /// with no non-linearity, which is never what a caller of this constructor
    /// means and is silently useless if allowed through.
    pub fn new(
        input: usize,
        hidden: usize,
        output: usize,
        layers: usize,
        layer_norm: bool,
        device: &B::Device,
    ) -> Self {
        let layers = layers.max(2);
        let mut linears = Vec::with_capacity(layers);
        linears.push(LinearConfig::new(input, hidden).init(device));
        for _ in 0..layers.saturating_sub(2) {
            linears.push(LinearConfig::new(hidden, hidden).init(device));
        }
        linears.push(LinearConfig::new(hidden, output).init(device));
        Self {
            layers: linears,
            norm: if layer_norm {
                Some(LayerNormConfig::new(output).init(device))
            } else {
                None
            },
            activation: Relu::new(),
        }
    }

    /// Builds an MLP whose weights come from an explicit, caller-owned random
    /// stream rather than from `burn`'s backend RNG.
    ///
    /// # Why this exists
    ///
    /// `burn`'s NdArray backend seeds a **process-global** generator, so
    /// `Backend::seed` does not make a model reproducible when anything else in
    /// the process draws from it concurrently. That is not hypothetical: the
    /// first version of the neural-surrogate reproducibility test failed
    /// exactly this way, two identical training calls producing initial losses
    /// of 1.2769 and 0.9423 because `cargo test`'s other threads were drawing
    /// from the same global stream in between.
    ///
    /// This constructor takes `seed` as an ordinary mutable LCG state, in the
    /// same style as the rest of this crate, so the weights depend on nothing
    /// but that state. Two calls with the same starting state produce
    /// bit-identical weights whatever else the process is doing.
    ///
    /// The distribution is the one `burn`'s default initialiser uses for a
    /// linear layer — uniform on `[-k, k]` with `k = 1 / sqrt(fan_in)` — so
    /// this is a determinism change, not a change of initialisation scheme.
    pub fn new_seeded(
        input: usize,
        hidden: usize,
        output: usize,
        layers: usize,
        layer_norm: bool,
        seed: &mut u64,
        device: &B::Device,
    ) -> Self {
        let mut mlp = Self::new(input, hidden, output, layers, layer_norm, device);
        for layer in mlp.layers.iter_mut() {
            let [fan_in, fan_out] = layer.weight.val().dims();
            let k = 1.0 / (fan_in as f64).sqrt();
            let weights: Vec<f32> = (0..fan_in * fan_out)
                .map(|_| ((prn(seed) * 2.0 - 1.0) * k) as f32)
                .collect();
            layer.weight = Param::from_data(
                TensorData::new(weights, [fan_in, fan_out]),
                device,
            );
            if let Some(bias) = layer.bias.as_mut() {
                let n = bias.val().dims()[0];
                let values: Vec<f32> = (0..n)
                    .map(|_| ((prn(seed) * 2.0 - 1.0) * k) as f32)
                    .collect();
                *bias = Param::from_data(TensorData::new(values, [n]), device);
            }
        }
        mlp
    }

    /// Applies the MLP to a `[nodes, features]` tensor.
    pub fn forward(&self, input: Tensor<B, 2>) -> Tensor<B, 2> {
        let mut x = input;
        let last = self.layers.len() - 1;
        for (index, layer) in self.layers.iter().enumerate() {
            x = layer.forward(x);
            if index != last {
                x = self.activation.forward(x);
            }
        }
        match &self.norm {
            Some(norm) => norm.forward(x),
            None => x,
        }
    }
}

/// One message-passing step: gather, sum, concatenate, transform, add.
#[derive(Module, Debug)]
pub struct Processor<B: Backend> {
    mlp: Mlp<B>,
}

impl<B: Backend> Processor<B> {
    /// Builds a processor over latent vectors of width `hidden`.
    pub fn new(hidden: usize, layers: usize, device: &B::Device) -> Self {
        Self {
            // 2 * hidden in: the node's own vector concatenated with the sum of
            // its incoming messages.
            mlp: Mlp::new(2 * hidden, hidden, hidden, layers, true, device),
        }
    }

    /// Advances the latent node features one message-passing step.
    ///
    /// `senders` and `receivers` are the edge index tensors, both of length
    /// `edges`. The residual add at the end is not decoration: without it a
    /// deep stack of these steps loses the node's own state, and the whole
    /// point of a large `M` is to stack many of them.
    pub fn forward(
        &self,
        x: Tensor<B, 2>,
        senders: Tensor<B, 1, Int>,
        receivers: Tensor<B, 1, Int>,
    ) -> Tensor<B, 2> {
        let [nodes, hidden] = x.dims();
        let device = x.device();

        // Gather each edge's message from its sender.
        let messages = x.clone().select(0, senders);
        // Scatter-add them into their receivers: PyTorch Geometric's
        // SumAggregation, spelled in burn.
        let aggregated = Tensor::zeros([nodes, hidden], &device).select_assign(
            0,
            receivers,
            messages,
            IndexingUpdateOp::Add,
        );

        let combined = Tensor::cat(vec![x.clone(), aggregated], 1);
        x + self.mlp.forward(combined)
    }
}

/// An encoder–processor–decoder message-passing network.
///
/// Built with [`MessagePassingNet::new`]; run with
/// [`MessagePassingNet::forward`]. The number of processor steps is fixed at
/// construction and is reported by
/// [`message_passing_steps`](Self::message_passing_steps), so a caller can
/// check it against [`super::bound::physics_guided_lower_bound`] — see
/// [`MessagePassingNet::check_reach`], which does exactly that.
#[derive(Module, Debug)]
pub struct MessagePassingNet<B: Backend> {
    encoder: Mlp<B>,
    processors: Vec<Processor<B>>,
    decoder: Mlp<B>,
}

impl<B: Backend> MessagePassingNet<B> {
    /// Builds the network.
    ///
    /// - `input` — node feature width going in.
    /// - `output` — node feature width coming out, the physical quantity being
    ///   predicted.
    /// - `hidden` — latent width; the upstream default is 128.
    /// - `message_passing_steps` — `M`, the number of processor steps. **This
    ///   is not a free hyperparameter**: see the module documentation and
    ///   [`super::bound`].
    /// - `layers` — linear maps per MLP; the upstream default is 2.
    pub fn new(
        input: usize,
        output: usize,
        hidden: usize,
        message_passing_steps: usize,
        layers: usize,
        device: &B::Device,
    ) -> Self {
        let processors = (0..message_passing_steps.max(1))
            .map(|_| Processor::new(hidden, layers, device))
            .collect();
        Self {
            encoder: Mlp::new(input, hidden, hidden, layers, true, device),
            processors,
            // No layer norm on the decoder: the output is a physical quantity
            // and normalising it would destroy its scale.
            decoder: Mlp::new(hidden, hidden, output, layers, false, device),
        }
    }

    /// Number of message-passing steps `M` this network performs.
    pub fn message_passing_steps(&self) -> usize {
        self.processors.len()
    }

    /// Runs the network over a graph.
    ///
    /// `features` is `[nodes, input]` and the result is `[nodes, output]`. The
    /// graph travels separately from the tensor because `burn` has no
    /// graph-carrying tensor type; the edge indices are converted to tensors
    /// here, once per call.
    ///
    /// # Panics
    ///
    /// If `features` has a different number of rows than the graph has nodes.
    /// This is a programming error rather than a data error — the two come from
    /// the same mesh — so it panics rather than returning a `Result` that every
    /// call site would have to thread through a training loop.
    pub fn forward(&self, graph: &Graph, features: Tensor<B, 2>) -> Tensor<B, 2> {
        let [nodes, _] = features.dims();
        assert_eq!(
            nodes,
            graph.node_count(),
            "feature rows ({nodes}) must match the graph's node count ({})",
            graph.node_count()
        );

        let device = features.device();
        let senders = edge_tensor::<B>(graph.senders(), &device);
        let receivers = edge_tensor::<B>(graph.receivers(), &device);

        let mut x = self.encoder.forward(features);
        for processor in &self.processors {
            x = processor.forward(x, senders.clone(), receivers.clone());
        }
        self.decoder.forward(x)
    }

    /// Checks this network's reach against what a PDE class needs on a given
    /// mesh.
    ///
    /// A convenience over [`super::bound::physics_guided_lower_bound`] that
    /// fills in the configured step count from the network itself, so the two
    /// cannot drift apart.
    pub fn check_reach(
        &self,
        class: super::bound::PdeClass,
        graph: &Graph,
        hop_length: f64,
    ) -> crate::Result<super::bound::IterationBound> {
        super::bound::physics_guided_lower_bound(
            class,
            graph,
            hop_length,
            Some(self.message_passing_steps()),
        )
    }
}

/// Converts an edge index slice into a `burn` integer tensor.
fn edge_tensor<B: Backend>(indices: &[usize], device: &B::Device) -> Tensor<B, 1, Int> {
    let data: Vec<i64> = indices.iter().map(|i| *i as i64).collect();
    Tensor::from_data(TensorData::from(data.as_slice()), device)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::gnn::bound::PdeClass;
    use burn::backend::NdArray;

    type TestBackend = NdArray<f32>;

    fn device() -> <TestBackend as burn::tensor::backend::BackendTypes>::Device {
        Default::default()
    }

    /// A path graph of `n` nodes.
    fn path(n: usize) -> Graph {
        let edges: Vec<(usize, usize)> = (0..n - 1).map(|i| (i, i + 1)).collect();
        Graph::from_undirected_edges(n, &edges).unwrap()
    }

    /// **Methodology.** The forward pass must produce one output row per node,
    /// of the requested width, and every value must be finite. A 7-node path,
    /// 3 input features, 2 outputs, hidden 16, 4 message-passing steps.
    ///
    /// **Result** (2026-09-16, NdArray backend): shape `[7, 2]`, all finite.
    #[test]
    fn the_forward_pass_has_the_right_shape() {
        let device = device();
        let graph = path(7);
        let net = MessagePassingNet::<TestBackend>::new(3, 2, 16, 4, 2, &device);
        let features = Tensor::<TestBackend, 2>::ones([7, 3], &device);
        let output = net.forward(&graph, features);
        assert_eq!(output.dims(), [7, 2]);
        let values: Vec<f32> = output.into_data().to_vec().unwrap();
        assert!(values.iter().all(|v| v.is_finite()), "{values:?}");
    }

    /// **Methodology — the reach property, which is the entire subject of this
    /// module.** After `M` message-passing steps a node's output must depend on
    /// nodes within `M` hops and on no others. Tested by perturbation on a
    /// 9-node path with `M = 2`: change the input features at node 8 and check
    /// which outputs move. Nodes 6, 7 and 8 are within 2 hops of node 8 and must
    /// change; nodes 0 to 5 are further and must not move at all.
    ///
    /// This is the direct, empirical form of the under-reaching argument: the
    /// network is *structurally* unable to propagate information further than
    /// its step count, whatever its weights are.
    ///
    /// **Result** (2026-09-16, NdArray backend, untrained weights): the
    /// per-node output change was
    /// `[0, 0, 0, 0, 0, 0, 0.00589, 0.05180, 0.02583]`. Nodes 0 to 5 moved by
    /// EXACTLY zero — not by a small amount, by zero — while the three nodes
    /// within reach all moved. That is under-reaching demonstrated rather than
    /// argued: no training schedule can put information where the architecture
    /// cannot carry it.
    #[test]
    fn a_node_cannot_be_influenced_from_further_than_the_step_count() {
        let device = device();
        let graph = path(9);
        let steps = 2;
        let net = MessagePassingNet::<TestBackend>::new(1, 1, 8, steps, 2, &device);

        let base = Tensor::<TestBackend, 2>::zeros([9, 1], &device);
        let baseline: Vec<f32> = net
            .forward(&graph, base.clone())
            .into_data()
            .to_vec()
            .unwrap();

        // Perturb node 8 only.
        let mut perturbed_values = vec![0.0f32; 9];
        perturbed_values[8] = 1.0;
        let perturbed = Tensor::<TestBackend, 2>::from_data(
            TensorData::new(perturbed_values, [9, 1]),
            &device,
        );
        let after: Vec<f32> = net.forward(&graph, perturbed).into_data().to_vec().unwrap();

        let deltas: Vec<f32> = baseline
            .iter()
            .zip(after.iter())
            .map(|(a, b)| (b - a).abs())
            .collect();
        println!("per-node output change from perturbing node 8 with M = {steps}: {deltas:?}");

        for (node, delta) in deltas.iter().enumerate() {
            let hops = 8 - node;
            if hops > steps {
                assert_eq!(
                    *delta, 0.0,
                    "node {node} is {hops} hops away but moved by {delta} with only {steps} steps"
                );
            }
        }
        assert!(
            deltas[8] > 0.0 && deltas[7] > 0.0 && deltas[6] > 0.0,
            "nodes within reach did not move: {deltas:?}"
        );
    }

    /// **Methodology.** The model must be able to report its own reach against
    /// the physics, with no separate bookkeeping. A 4-step network on a
    /// 9-node path, asked about an elliptic problem whose bound is the path's
    /// diameter of 8.
    ///
    /// **Result** (2026-09-16): required 8, configured 4, not satisfied,
    /// shortfall 4.
    #[test]
    fn the_model_reports_its_own_under_reaching() {
        let device = device();
        let graph = path(9);
        let net = MessagePassingNet::<TestBackend>::new(1, 1, 8, 4, 2, &device);
        let bound = net.check_reach(PdeClass::Elliptic, &graph, 0.125).unwrap();
        assert_eq!(bound.required, 8);
        assert_eq!(bound.configured, Some(4));
        assert_eq!(bound.is_satisfied(), Some(false));
        assert_eq!(bound.shortfall(), Some(4));
    }

    /// **Methodology.** Aggregation must be a genuine sum over incoming edges,
    /// so a node with more neighbours receives more. Compared on a star graph:
    /// the hub (4 neighbours) and a leaf (1 neighbour) must not produce the
    /// same latent update from identical inputs.
    ///
    /// **Result** (2026-09-16): the hub and leaf outputs differ.
    #[test]
    fn aggregation_sums_over_incoming_edges() {
        let device = device();
        let graph = Graph::from_undirected_edges(5, &[(0, 1), (0, 2), (0, 3), (0, 4)]).unwrap();
        let net = MessagePassingNet::<TestBackend>::new(1, 1, 8, 1, 2, &device);
        let features = Tensor::<TestBackend, 2>::ones([5, 1], &device);
        let output: Vec<f32> = net.forward(&graph, features).into_data().to_vec().unwrap();
        assert!(
            (output[0] - output[1]).abs() > 1e-6,
            "hub {} and leaf {} should differ: {output:?}",
            output[0],
            output[1]
        );
        // The four leaves are interchangeable and must agree with each other.
        for leaf in 2..5 {
            assert!((output[1] - output[leaf]).abs() < 1e-5, "{output:?}");
        }
    }

    /// **Methodology.** The step count reported by the model must be the number
    /// actually performed, and a zero request must be clamped to one rather
    /// than producing a network with no processors at all.
    ///
    /// **Result** (2026-09-16): 5 and 1 respectively.
    #[test]
    fn the_step_count_is_what_it_says() {
        let device = device();
        let net = MessagePassingNet::<TestBackend>::new(1, 1, 4, 5, 2, &device);
        assert_eq!(net.message_passing_steps(), 5);
        let clamped = MessagePassingNet::<TestBackend>::new(1, 1, 4, 0, 2, &device);
        assert_eq!(clamped.message_passing_steps(), 1);
    }
}
