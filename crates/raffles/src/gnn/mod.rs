//! Graph neural networks for physics: message passing, and the physics-guided
//! bound on how much of it is enough.
//!
//! # Why this is in RAFFLES
//!
//! A message-passing graph network trained on simulation output is a
//! **surrogate model** — the thing [`crate::surrogate`] is for, and the thing a
//! UQ campaign needs when the real model is too expensive to sample thousands
//! of times. The intended consumers are the workspace's particle and transport
//! codes: granular DEM (`outram-park-fork-liggghts`), Monte Carlo transport
//! (`outram-mc-libs`) and the TRISO/Lagrangian work in `boon-lay`, all of which
//! already have a natural graph structure — contacts, cells, particles.
//!
//! # The part worth having first
//!
//! [`bound`] — the physics-guided lower bound on message-passing iterations —
//! needs **no neural network and no `burn`**, and is useful on its own. It
//! answers a question that is otherwise settled by hyperparameter search: how
//! many message-passing steps does this PDE on this mesh actually require? Get
//! it wrong downwards and the network **under-reaches**: its prediction cannot
//! physically depend on the data that determine the answer, so it fails in
//! rollout no matter how well it trains.
//!
//! [`graph`] holds the topology that bound is computed from — radius and
//! contact graphs, hop distances, diameters, receptive fields, disjoint-union
//! batching — and is likewise `burn`-free. ~~`mc_geometry` builds that
//! topology from an `outram-mc-libs` CSG geometry~~ **CORRECTED 2026-10-02**:
//! the CSG adapter (`cell_adjacency_graph`) moved to
//! `outram_blender::gnn_graph`, behind that crate's `gnn-graph` feature, by
//! maintainer decision (GitHub issue #486: `outram-blender` owns geometry
//! description). RAFFLES no longer depends on `outram-mc-libs` at all. The
//! matching bridge for granular DEM lives in `outram-park-fork-liggghts`
//! behind its `gnn` feature. Both adapters live with the geometry because
//! the dependency only runs one way: geometry crates depend on RAFFLES, never
//! the reverse.
//!
//! [`mpnn`] is the network itself and [`training`] is its training loop and
//! autoregressive rollout; both are behind the crate's `burn` feature.
//! [`dataset`] reads the upstream's own `.pt` trajectory files and needs no
//! `burn` at all, so a caller can measure a published dataset's mesh and its
//! reach requirement without building a tensor library.
//!
//! # Provenance
//!
//! Unlike the rest of this crate, [`mpnn`] **is a port**: the upstream
//! Physics-guided-MPNN repository is GPL-3.0, the same licence as this
//! workspace, so its model code could be translated directly. That file carries
//! the attribution header the crate's `CLAUDE.md` requires.
//!
//! [`bound`]'s formulas are *not* transcribed — they are not in the upstream
//! code, which fixes its iteration count by configuration — and are derived
//! there from the principles the paper states, with the derivation written out.
//! See that module for exactly how far that claim goes.
//!
//! # Reference
//!
//! - L. Tesan and M. M. Iparraguirre et al. (2025). On the under-reaching
//!   phenomenon in message passing neural PDE solvers: revisiting the CFL
//!   condition. arXiv:2507.08861. <https://arxiv.org/abs/2507.08861>
//! - T. Pfaff, M. Fortunato, A. Sanchez-Gonzalez and P. W. Battaglia (2021).
//!   Learning mesh-based simulation with graph networks. *ICLR 2021*.
//!   arXiv:2010.03409 — MeshGraphNet, the encoder–processor–decoder
//!   architecture the upstream builds on.
//!
//! # Status
//!
//! No human V&V. The reach property of the network is tested directly (see
//! [`mpnn`]), the bound is tested against hand-computed lattice diameters and
//! CFL numbers, and [`training`] runs an end-to-end Poisson experiment against
//! exact solutions. That experiment demonstrates the penalty for under-reaching
//! badly; it does NOT resolve whether reaching the bound exactly matters, and
//! the test says so with its measurements. Nothing here has been trained on or
//! validated against a published PDE dataset — though [`dataset`] now reads
//! those datasets, and the measurements it takes from all five of the
//! upstream's are recorded in its tests, including one place where the bound
//! computed here differs from the number the upstream's README states.

pub mod bound;
pub mod dataset;
pub mod graph;

#[cfg(feature = "burn")]
pub mod mpnn;

#[cfg(feature = "burn")]
pub mod training;

pub use bound::{physics_guided_lower_bound, IterationBound, PdeClass};
pub use dataset::TorchArchive;
pub use graph::Graph;

#[cfg(feature = "burn")]
pub use mpnn::{MessagePassingNet, Mlp, Processor};

#[cfg(feature = "burn")]
pub use training::{rollout, train, MpnnTrainingConfig, MpnnTrainingReport};
