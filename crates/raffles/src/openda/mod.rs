// SPDX-License-Identifier: AGPL-3.0-only
// Copyright (C) 2026 OUTRAM PARK contributors
//
// Module root. NOT a port; every ported file carries its own OpenDA (LGPL-3.0-
// or-later) or JAMA (public domain) header. Added 2026-11-04 for GitHub issue
// #1192 (epic #1177) at the workspace maintainer's direction.

//! **OpenDA's data-assimilation and calibration algorithms** (gh:#1192).
//!
//! Ported from OpenDA (OpenDA Association, Java,
//! <https://github.com/OpenDA-Association/OpenDA>, commit `e51565c2c8de`,
//! LGPL-3.0-or-later; `LICENSE-LGPL-3.0-OPENDA`, crate `NOTICE`), the
//! algorithms and not the Java framework (no XML configuration, no model
//! wrappers):
//!
//! | Item | Upstream |
//! |---|---|
//! | [`run_sequential`], [`SequentialFilter`] | `AbstractSequentialAlgorithm`, `AbstractSequentialEnsembleAlgorithm` |
//! | [`enkf_update`], [`enkf_analysis`], [`Localization`], [`zhang_weights`] | `EnKF`, `EnsembleVectors`, `AutoLocalizationZhang2011` |
//! | [`ensr_update`], [`ensr_analysis`] | `EnSR` |
//! | [`particle_filter_update`], [`residual_resampling`] | `ParticleFilter` (residual resampling only, as upstream) |
//! | [`steady_state_analysis`], [`SteadyStateGain`] | `SteadyStateFilter` |
//! | [`Dud`] | `BaseDudCoreOptimizer`, `DudCoreOptimizer` |
//! | [`Simplex`] | `SimplexCoreOptimizer` |
//! | [`Powell`] | `PowellCoreOptimizer` |
//! | [`WeightedLeastSquares`], [`SimulationPredictor`] | `SimulationKwadraticCostFunction` (OpenDA's weighted cost) |
//! | [`SimpleCost`], [`RosenbrockCost`], [`simple_least_squares`] | OpenDA's test cost functions |
//! | [`StochVector`] | `StochVector`, `SqrtCovariance` (noise and uncertainty) |
//! | [`SimpleStochModel`], [`Dynamics`], [`Observations`] | `SimpleStochModelInstance` with the oscillator and Lorenz test models; `CsvStochObserver` |
//! | [`linalg`] | `org.openda.utils.Matrix`, `VectorDouble` |
//! | [`jama`] | JAMA 1.0.2's SVD (public domain), which OpenDA's `inverse`/`sqrt`/`rightSolve` use |
//!
//! Written here, not ported: [`DrawSource`] (replays OpenDA's recorded draws
//! for differential tests, or draws from the workspace LCG); the
//! [`OdaStochModel`], [`CostFunction`] and [`Predictor`] seams (traits
//! consumed generically, after OpenDA's interfaces). The typed route for a
//! caller's [`crate::fidelity::StateSpaceModel`] is
//! [`crate::fidelity::Assimilator::OpenDaEnsemble`].
//!
//! The kernels round as Java does (sequential IEEE, no FMA), so on the same
//! inputs and draws the port is **bit-identical** with OpenDA; upstream
//! quirks are ported faithfully and listed in each file's header and in
//! `verification_and_validation/openda_gh1192.md` §3.
//!
//! **Synthetic data only** (`RESPONSIBLE_USE.md`): twin experiments,
//! teaching and V&V, never the state of a real plant.
//!
//! V&V: `crates/raffles/verification_and_validation/openda_gh1192.md`,
//! tests `tests/openda_gh1192.rs`.

pub mod cost;
pub mod dud;
pub mod enkf;
pub mod ensr;
pub mod jama;
pub mod linalg;
pub mod models;
pub mod noise;
pub mod powell;
pub mod sequential;
pub mod simplex;

pub use cost::{simple_least_squares, CostFunction, CostHistory, IdentityPredictor, LeastSquaresView, Predictor, RosenbrockCost, SimpleCost, SimulationPredictor, WeightedLeastSquares};
pub use dud::Dud;
pub use enkf::{compute_gain, enkf_analysis, enkf_update, zhang_weights, EnkfUpdate, EnsembleVectors, Localization, LocalizationInput};
pub use ensr::{ensr_analysis, ensr_update, particle_filter_analysis, particle_filter_update, residual_resampling, steady_state_analysis, Resampling, SteadyStateGain};
pub use models::{Dynamics, Observations, OdaStochModel, SimpleModelConfig, SimpleStochModel};
pub use noise::{DrawSource, StochVector};
pub use powell::Powell;
pub use sequential::{run_sequential, EnsembleOptions, SequentialFilter, SequentialRun, SequentialStep};
pub use simplex::Simplex;
