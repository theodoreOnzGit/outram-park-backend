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
//! ## Derivation, step by step
//!
//! The whole model is built up from two first-principles laws. This is a
//! condensed narrative; the full derivation (with limits, term-by-term code
//! correspondence, and references) is in the crate-root
//! `TRISO_ATOPS_DERIVATION.md` (Python-model view) and `docs/triso-atops-derivation.md`
//! (Rust-port view). Each step names the function that implements it.
//!
//! 1. **First principles.** Fickian diffusion `∂C/∂t = D∇²C` and radioactive
//!    decay `dN/dt = −λN`, with `λ = ln2 / t½`
//!    ([`TrisoAtopsNuclide::decay_constant`](crate::triso_atops_fork::nuclide_model::TrisoAtopsNuclide::decay_constant)).
//!    A fission product in the fuel obeys both at once:
//!    `∂C/∂t = D∇²C − λC + B` (birth rate `B`).
//! 2. **Equivalent sphere.** The Booth idealisation (Booth 1957) replaces the
//!    real multi-shell TRISO particle with one uniform sphere of radius `a` per
//!    chemical group. The group partition is
//!    [`ElementGroup`](crate::triso_atops_fork::nuclide_model::ElementGroup); the
//!    special-metal sphere radius `a_booth = √(2·a_grain·r)` is formed in
//!    [`rb_fail`](crate::triso_atops_fork::release_models::rb_fail).
//! 3. **Effective coefficient.** Everything depends on `D` and `a` only through
//!    `D' = D/a²` (units s⁻¹). `D` follows an Arrhenius law
//!    `D(T) = D0·exp(−Q/RT)`, implemented in
//!    [`diffusion_coefficient`](crate::triso_atops_fork::diffusion::diffusion_coefficient)
//!    and [`diffusion_coefficient_sic_ag`](crate::triso_atops_fork::diffusion::diffusion_coefficient_sic_ag).
//! 4. **Stable-species release.** Diffusion out of the sphere gives the
//!    fractional release `f = 1 − (6/π²)·Σ n⁻²·exp(−n²π²·D't)` (short-time limit
//!    `6√(D't/π) − 3D't`), in
//!    [`booth_longlived`](crate::triso_atops_fork::release_models::steady_state::booth_longlived).
//! 5. **Add decay.** Short-lived species reach a steady release-to-birth ratio
//!    `⟨R/B⟩ = (3/μ)(coth μ − 1/μ)`, `μ = √(λa²/D)`
//!    ([`booth_shortlived_fast_diffuse`](crate::triso_atops_fork::release_models::steady_state::booth_shortlived_fast_diffuse)).
//!    Silver permeates the SiC barrier by the Daynes–Barrer membrane time-lag
//!    solution ([`breakthrough_model`](crate::triso_atops_fork::release_models::steady_state::breakthrough_model));
//!    volatiles use an empirical fit
//!    ([`rb_fail_noble_gases`](crate::triso_atops_fork::release_models::steady_state::rb_fail_noble_gases));
//!    graphite hold-up is the attenuation factor
//!    ([`attenuation_factor`](crate::triso_atops_fork::release_models::steady_state::attenuation_factor)).
//! 6. **Assemble.** Per nuclide per node: `D` → `⟨R/B⟩_fail`
//!    ([`rb_fail`](crate::triso_atops_fork::release_models::rb_fail)) → release
//!    rate `R` ([`release_rate`](crate::triso_atops_fork::activities::release_rate))
//!    → source `S` + graphite `G`
//!    ([`base_activities`](crate::triso_atops_fork::activities::base_activities))
//!    → loop pools `C`/`P`/`HPS`
//!    ([`activities::coolant_activity`](crate::triso_atops_fork::activities::coolant_activity))
//!    → curies. The whole chain is
//!    [`normal_operation_node`](crate::triso_atops_fork::normal_operation::normal_operation_node).
//! 7. **Transient.** For an accident the products `Dt`, `D't` become time
//!    integrals `∫D dt`, `∫D' dt`
//!    ([`integrate_diffusion_over_time`](crate::triso_atops_fork::diffusion::integrate_diffusion_over_time)),
//!    and the Step 4/5 series are reused in
//!    [`release_models::transient`](crate::triso_atops_fork::release_models::transient).
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
