// SPDX-License-Identifier: GPL-3.0
//
// TRISO-ATOPS fork — provenance
// -----------------------------
// Upstream project : TRISO-ATOPS (INL) — https://github.com/IdahoLabResearch/TRISO-ATOPS
// Upstream commit  : de374c8
// Upstream source  : trisoatops/ (calculation_functions.py, run_functions.py, trisoatops.py)
// Original license : MIT — Copyright (c) 2026 Battelle Energy Alliance, LLC
//                    (DOE contract DE-AC07-05ID14517; authors B. D. Stoyer,
//                     D. A. Petti, A. C. Raichart; User Manual also credits K. E. Egan)
// This port is distributed under GPL-3.0 as part of the combined boon-lay work;
// the MIT notice above is retained per the MIT license (see LICENSE.triso-atops
// and NOTICE.triso-atops at the crate root). MIT is GPLv3-compatible.

//! # `triso_atops_fork` — Eulerian / continuum TRISO fission-product release
//!
//! This module is a Rust **fork of Idaho National Laboratory's TRISO-ATOPS**
//! (TRISO Analysis TOol for Predictive Source terms). It is the
//! **Eulerian / continuum-diffusion** complement to the rest of `boon-lay`,
//! which models the same physics from a **Lagrangian** (single-atom
//! Monte-Carlo tracking) perspective.
//!
//! Where the Lagrangian side walks individual atoms through the TRISO layers,
//! TRISO-ATOPS uses **closed-form analytical solutions to the Fickian
//! diffusion equation** (the *Booth* equivalent-sphere model, a *breakthrough*
//! model, and a graphite *attenuation* model) to predict the fraction of each
//! fission-product nuclide released from the fuel kernel and matrix graphite.
//! The equations originate from the NP-MHTGR New Production Reactor Program
//! (Anderson et al., "Generic Reactor Plant Description and Source Terms
//! Volume 1", EG&G Idaho, 1989); half-lives are from the IAEA Live Chart of
//! Nuclides.
//!
//! ## What lives where
//!
//! | Submodule | Physical content |
//! |---|---|
//! | [`nuclide_model`](crate::triso_atops_fork::nuclide_model) | The TRISO-ATOPS nuclide record (Z, A, half-life, decay constant, parent), the five transport [`ElementGroup`](crate::triso_atops_fork::nuclide_model::ElementGroup)s, and the supported-nuclide database. |
//! | [`diffusion`](crate::triso_atops_fork::diffusion) | Arrhenius diffusion coefficients `D(T)` in m^2/s in the kernel, matrix graphite, and (for Ag) the SiC layer, plus the time-integrated `∫D dt` used by transient/accident release. |
//! | [`release_models`](crate::triso_atops_fork::release_models) | The dimensionless release-fraction / release-to-birth models: Booth (long-lived, short-lived), breakthrough, graphite attenuation, and their transient (accident) variants, plus the group dispatchers. |
//! | [`activities`](crate::triso_atops_fork::activities) | Circulating / plate-out / clean-up activity bookkeeping and the release-rate / graphite source terms, plus the Ci↔Bq and `A = λN` conversions (bead op-b4a.2.2, done). |
//! | [`normal_operation`](crate::triso_atops_fork::normal_operation) | Per-node normal-operation orchestration ([`normal_operation_node`](crate::triso_atops_fork::normal_operation::normal_operation_node)) composing the whole chain to curies (bead op-b4a.2.2, done). The JSON run-file driver + accident case remain scaffolded (bead op-b4a.2.3). |
//!
//! ## Units
//!
//! Every public function takes and returns `uom` dimensioned quantities. The
//! named aliases below spell out what each dimensionless-or-rate quantity means
//! for a reader hovering in their editor:
//!
//! - [`DecayConstant`](crate::triso_atops_fork::DecayConstant) — the radioactive
//!   decay constant `λ = ln 2 / t½`, SI unit `s^-1` (dimensionally a
//!   [`uom::si::f64::Frequency`]).
//! - [`ReleaseFraction`](crate::triso_atops_fork::ReleaseFraction) — a
//!   dimensionless release fraction or release-to-birth ratio in `[0, 1]` (a
//!   [`uom::si::f64::Ratio`]).
//!
//! Temperatures are [`uom::si::f64::ThermodynamicTemperature`]; the TRISO-ATOPS
//! correlations are written in °C internally, so the functions read the input as
//! both °C (for the valid-range thresholds) and K (for the Arrhenius exponent).
//!
//! ## Scope of this fork
//!
//! The **GUI** (`trisoatops_gui.py`) is intentionally **not** ported —
//! `boon-lay` is a headless library and the workspace requires non-GUI library
//! code to build for Android. See `docs/triso-atops-fork.md` for the full
//! Python→Rust module map and the port/verification status.

/// The radioactive decay constant `λ = ln 2 / t½`.
///
/// SI unit `s^-1`; dimensionally a frequency. Construct with
/// `Frequency::new::<hertz>(..)` and read with `.get::<hertz>()`
/// (`hertz` == `s^-1` here — the name is only a dimension label).
pub type DecayConstant = uom::si::f64::Frequency;

/// A dimensionless release fraction or release-to-birth (`<R/B>`) ratio.
///
/// Physically in `[0, 1]`. The release models clamp to this range where the
/// upstream code does. Construct with `Ratio::new::<ratio>(..)` and read with
/// `.get::<ratio>()`.
pub type ReleaseFraction = uom::si::f64::Ratio;

pub mod nuclide_model;

pub mod diffusion;

pub mod release_models;

pub mod activities;

pub mod normal_operation;

pub use activities::Activity;
pub use nuclide_model::{ElementGroup, TrisoAtopsNuclide};
