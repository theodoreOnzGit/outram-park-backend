// ---------------------------------------------------------------------------
// Ported from SEPIA (Simulation-Enabled Prediction, Inference, and Analysis).
//
//   Upstream project: SEPIA — Los Alamos National Laboratory
//   Upstream repo:    https://github.com/lanl/SEPIA
//   Upstream files:   sepia/{SepiaData,SepiaDistCov,SepiaLogLik,SepiaPrior,
//                     SepiaModel,SepiaPredict}.py (see each submodule)
//   Upstream commit:  309890423a121919dc09a678c65a21748bbefb4b  (master)
//   Accessed:         2026-11-04
//
//   (c) 2020. Triad National Security, LLC. All rights reserved.
//   Licensed under the BSD 3-Clause License; the full text is preserved
//   verbatim in crates/raffles/LICENSE-BSD-SEPIA.
//
// This Rust translation is part of RAFFLES / Outram Park and is distributed
// under AGPL-3.0-only. BSD-3 -> AGPLv3 is ONE-WAY: this file may not be
// contributed back to SEPIA or redistributed under BSD terms.
//
// Translation notes: module layout only; see data.rs and model.rs.
// sample.rs is NOT a port.
// ---------------------------------------------------------------------------

//! **Kennedy–O'Hagan calibration with model discrepancy** (gh:#1180),
//! translated from SEPIA's scalar-output model (the GPMSA formulation of
//! Higdon et al. 2004, 2008):
//!
//! ```text
//! y_obs(x) = η(x, θ) + δ(x) + ε,   η ~ GP over (x, t) fitted to simulator runs,
//!                                  δ ~ GP over x,  ε ~ N(0, 1/λ_Os)
//! ```
//!
//! (Kennedy & O'Hagan 2001, *J. R. Stat. Soc. B* 63:425.)
//!
//! - [`KohData`]: SEPIA's transforms (unit cube, standardised y).
//! - [`KohModel`]: [`KohForm`] × [`KohPriors`]; [`KohModel::log_lik`],
//!   [`KohModel::log_prior`], [`KohModel::log_post`], [`KohModel::predict`]
//!   (the conditional Gaussian of (δ, η) at new x), [`EmulatorFactor`] for a
//!   fixed emulator.
//! - [`KohModel::sample`]: raffles' TMCMC on SEPIA's posterior (not a port).
//!
//! **Upstream findings that shape this module** (V&V record): SEPIA's own
//! univariate setup cannot carry δ (only its likelihood and its dense
//! predictor still have the GPMSA branch), and that dense predictor's
//! scalar + δ branch places one block wrongly; the port follows upstream's
//! own comment there.
//!
//! The typed route from a `ParametricModel` and a `Dataset` is
//! [`crate::fidelity::koh`]. V&V:
//! `crates/raffles/verification_and_validation/koh_calibration_gh1180.md`,
//! differential tests `tests/koh_sepia_gh1180.rs`.

mod data;
mod model;
mod sample;

pub use data::{pairwise_sum, KohData};
pub use model::{cov_cross, cov_self, EmulatorFactor, KohForm, KohModel, KohParams, KohPredictive, KohPriors, ParamPrior, PriorDist};
pub use sample::{KohFree, KohPosterior, KohSampleConfig};
