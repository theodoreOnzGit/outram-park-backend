// SPDX-License-Identifier: AGPL-3.0-only
// Copyright (C) 2026 OUTRAM PARK contributors
//
// NOT ported from RAVEN. Added 2026-11-04 for GitHub issue #1194 (epic #1177)
// at the WORKSPACE MAINTAINER's direction; the crate owner's (Adolphus Lye's)
// review is outstanding — see this crate's CLAUDE.md, "Scope boundaries".

//! **One common interface from a high-fidelity model to a surrogate**
//! (gh:#1194, epic gh:#1177): traits for the caller's models, enums for the
//! routes RAFFLES provides, and `uom` quantities at the edge.
//!
//! # The pieces
//!
//! | Item | Job |
//! |---|---|
//! | [`Quantities`], [`quantities!`](crate::quantities), [`Channel`], [`SiQuantity`] | a record of named `uom` quantities (an input `x`, an output `y`, a parameter vector `θ`) mapped one to one onto SI base values |
//! | [`ParameterSpace<Q>`] | a bound or a prior per channel, as a RAFFLES [`Distribution`](crate::distributions::Distribution), stated in uom |
//! | [`HighFidelityModel`] | `y = f(x)`, implemented by the caller (CFD, Monte Carlo, a toy function) |
//! | [`ParametricModel`] | `y = f_LF(x, θ)`, the object of calibration (Kennedy–O'Hagan gh:#1180, DUD gh:#1192) |
//! | [`Dataset<I, O>`], [`MultiFidelityDataset`] | runs with provenance, a [`Fidelity`] tag and a [`Split`] fixed at construction |
//! | [`SurrogateSpec`], [`Surrogate<I, O>`] | what to fit, and the fitted model: `Polynomial` (one fit per output), `Neural` behind the `burn` feature, since gh:#1193 RAVEN's `PolynomialChaos` and `Kriging { route: RavenSklearn }` (with a σ), since gh:#1190 SMT's `Kriging { route: Smt }` and `CoKriging` (MFK, fitted by `Surrogate::fit_multi` on a `MultiFidelityDataset`) |
//! | [`Prediction<O>`] | `mean: O`, `std_dev: Option<O>` (the uncertainty keeps the output's units) |
//! | [`ValidationReport<O>`] | held-out RMSE and max error per output in output units, LOO RMSE on the training rows, 2σ coverage when a σ exists |
//! | [`CorrectedModel<L>`], [`ThetaEstimate`], [`discrepancy_dataset`] | `y_LF(x, θ) + δ(x)`; since gh:#1180 `ThetaEstimate::Posterior` |
//! | [`koh::KohCalibration`], [`koh::KohSpec`] | Kennedy–O'Hagan calibration of a [`ParametricModel`] against a [`Dataset`]'s training rows, with a GP discrepancy (SEPIA's formulation, gh:#1180) |
//! | [`build`], [`sample`], [`BuildPlan`], [`HeldOut`] | sample `hf` over a [`Sampler`](crate::samplers::Sampler) design, split, fit, validate |
//! | [`StateSpaceModel`], [`Assimilator`], [`Gaussian`], [`AssimilationStep`] | `x_{k+1} = f(x_k)`, `z_k = h(x_k)`, implemented by the caller; FilterPy's EKF / UKF / EnKF ([`crate::filtering`]) run on it, typed (gh:#1191) |
//!
//! # Design rules it follows
//!
//! - **Traits between crates, enums within.** [`HighFidelityModel`] and
//!   [`ParametricModel`] are implemented in the physics crate and consumed
//!   here generically (`M: HighFidelityModel`), never as `dyn` (root
//!   `AGENTS.md`, "Crate-level interfaces may follow SOLID principles"). The
//!   routes RAFFLES provides are the closed enum [`Surrogate`]. No `dyn`, no
//!   `Box`, no lifetimes; `PhantomData` carries `I` and `O`.
//! - **uom at the edge, SI `f64` inside.** Every public signature that carries
//!   a physical quantity is typed; the numerics (polynomial, network, the
//!   ports still to come) run on SI base values, which is what keeps the
//!   differential tests of the ports against their upstream unchanged. The
//!   round trip is exact (`quantities` module tests).
//! - **The held-out split is fixed before fitting**, in the [`Dataset`] or
//!   the [`BuildPlan`], and no fit reads a held-out row: the test
//!   `poisoned_held_out_outputs_do_not_change_the_fit` pins it.
//! - **Routes are added with their ports, not as placeholders.** ~~Kriging and
//!   co-kriging (SMT, gh:#1190) become [`SurrogateSpec`] / [`Surrogate`]
//!   variants when they land~~ (landed 2026-11-04: `KrigingRoute::Smt`,
//!   `SurrogateSpec::CoKriging`; ~~polynomial chaos (RAVEN, gh:#1193)~~ landed
//!   2026-11-04 with RAVEN's kriging route); ~~a sibling
//!   `StateSpaceModel` trait comes with the Kalman ports (gh:#1191)~~
//!   landed 2026-11-04 with them: [`StateSpaceModel`] and [`Assimilator`].
//!
//! # Scope note
//!
//! [`build`] runs the caller's model, which the crate's earlier scope ("the
//! caller runs their own model and hands RAFFLES arrays of numbers") did not
//! do. It does so only through the caller's own trait implementation, with
//! no physics here; the maintainer directed the interface (gh:#1194).
//!
//! # V&V
//!
//! Record: `crates/raffles/verification_and_validation/fidelity_interface_gh1194.md`
//! (predictions written before the runs). The first real implementer is the
//! digital twin engine's double-pipe pair
//! (`outram_park_digital_twin_engine::multifidelity::double_pipe::fidelity`).

mod build;
mod dataset;
pub mod koh;
mod models;
mod quantities;
mod space;
mod state_space;
mod surrogate;

pub use build::{build, sample, BuildPlan, HeldOut};
pub use dataset::{Dataset, Fidelity, MultiFidelityDataset, SiRows, Split};
pub use models::{
    discrepancy_dataset, CorrectedModel, HighFidelityModel, ParametricModel, ThetaEstimate,
};
pub use quantities::{check_width, Channel, Quantities, SiQuantity};
pub use space::ParameterSpace;
pub use state_space::{AssimilationStep, Assimilator, Gaussian, StateSpaceModel};
#[cfg(feature = "burn")]
pub use surrogate::NeuralBackend;
pub use surrogate::{KrigingRoute, Prediction, Surrogate, SurrogateSpec, ValidationReport};
