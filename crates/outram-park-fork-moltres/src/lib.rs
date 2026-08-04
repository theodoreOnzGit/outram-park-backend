// SPDX-License-Identifier: GPL-3.0-only
// Copyright (C) 2026 OUTRAM PARK contributors
//
// Physics formulation derived from Moltres (MSR multiphysics on MOOSE)
//   Upstream: https://github.com/arfc/moltres (UIUC ARFC group)
//   Upstream commit: 3dd2ce7 (cloned under upstream_source/moltres)
//   Upstream license: LGPL-2.1, incorporated into this GPL-3.0 crate under
//   the LGPL-2.1 section 3 GPL-conversion option. Per-file provenance
//   headers list the exact kernels each module's formulation comes from.
//   This is an independent finite-volume reimplementation on
//   outram-foam-basic-lib — no MOOSE, no PETSc, no finite elements, no
//   upstream code copied.
//
// This file is part of OUTRAM PARK.
//
// OUTRAM PARK is free software: you can redistribute it and/or modify it
// under the terms of the GNU General Public License as published by the
// Free Software Foundation, either version 3 of the License, or (at your
// option) any later version.
//
// OUTRAM PARK is distributed in the hope that it will be useful, but
// WITHOUT ANY WARRANTY; without even the implied warranty of
// MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE.  See the GNU
// General Public License for more details.
//
// You should have received a copy of the GNU General Public License along
// with OUTRAM PARK.  If not, see <https://www.gnu.org/licenses/>.

//! # outram-park-fork-moltres
//!
//! Circulating-fuel molten-salt-reactor (MSR) multiphysics on the
//! outram-foam **finite-volume** layer — the physics formulation of the
//! LGPL-2.1 [Moltres](https://github.com/arfc/moltres) code (multigroup
//! neutron diffusion + delayed-neutron precursor drift + salt heat
//! transfer), deliberately reimplemented on
//! [`outram_foam_basic_lib`]'s `FvMesh`/`fvm` operators instead of
//! MOOSE/PETSc finite elements. Not affiliated with the Moltres/ARFC
//! project.
//!
//! > **⚠️ Untrusted AI-assisted draft — pending human V&V.** All physics
//! > here is verified only against analytic/limiting cases by automated
//! > tests (each test documents its methodology and measured results);
//! > no human review, no validation against MSRE benchmark data yet. Not
//! > for nuclear facility operation, reactor control, safety-critical, or
//! > licensing decisions (see the workspace `RESPONSIBLE_USE.md`).
//!
//! ## What it models (first pass)
//!
//! - [`diffusion`] — static-fuel multigroup neutron-diffusion k-eigenvalue
//!   (power iteration over `fvm::laplacian + fvm::sp` group systems).
//! - [`precursors`] — delayed-neutron precursor **advection–decay drift**
//!   `dC_i/dt + div(u C_i) - div(D_C grad C_i) = beta_i/k S_f - lambda_i
//!   C_i`: the defining circulating-fuel physics.
//! - [`circulating`] — the coupled flux + drifting-precursor eigenvalue on
//!   a closed loop: reactivity falls with loop speed as precursors decay
//!   outside the core (the classic MSRE circulation loss).
//! - [`thermal`] — reduced slug-flow salt temperature + heat exchanger +
//!   linear cross-section temperature feedback, Picard-coupled.
//! - [`ring_mesh`] — the closed 1-D loop mesh (periodic topology via a
//!   ring of internal faces; no cyclic boundary machinery needed).
//! - [`materials`] — SI multigroup cross-section records and their
//!   materialisation to per-cell fields.
//!
//! **Prescribed flow only:** the salt velocity is an input (rigid loop
//! circulation), not solved — full CFD coupling is the appbuilder/GeN-Foam
//! path. **Steady eigenvalue only** for the coupled system (precursor
//! transients exist as [`precursors::PrecursorDrift::step`], but there is
//! no coupled flux transient yet). Units are **SI (metres)** throughout;
//! see [`materials`] for cm → m conversion of standard reactor-physics
//! tables.
//!
//! ## Verification summary (measured 2026-08-04, release build)
//!
//! | Check | Reference | Measured result |
//! |---|---|---|
//! | 1-group bare-slab k | analytic `nuSigma_f/(Sigma_a + D B^2)` | rel. err `6.3e-6` |
//! | 2-group bare-slab k | analytic two-group formula | rel. err `9.5e-7` |
//! | Zero-flow precursors | algebraic equilibrium `beta S_f/(k lambda)` | rel. err `2.3e-16` |
//! | Loop precursor balance | production = decay on closed loop | imbalance `<= 8.6e-11` |
//! | u = 0 circulating solver | equals static solver | `dk = 2.2e-16` |
//! | Circulation reactivity loss | monotone, `< beta`, MSRE-order | 151–388 pcm over 0.15–2.4 m/s (287 pcm at the MSRE-like nominal 0.6 m/s) |
//! | Loop energy balance | HX removal = deposited power; slug-flow `dT` | imbalance `1.1e-8`; `dT` matches analytic to 0.03 % |
//! | Feedback sign | k falls / T rises with power | monotone at 0.5/4/8 MW, ~170 pcm/MW |
//!
//! (Each check records full methodology and the measured numbers in the
//! corresponding test's doc comment.)

#![forbid(unsafe_code)]

pub mod circulating;
pub mod diffusion;
pub mod error;
pub mod materials;
pub mod precursors;
pub mod ring_mesh;
pub mod thermal;

/// Convenience re-exports of the crate's main types.
pub mod prelude {
    pub use crate::circulating::CirculatingFuelSolver;
    pub use crate::diffusion::{reactivity, EigenReport, EigenSettings, StaticDiffusion};
    pub use crate::error::MoltresError;
    pub use crate::materials::{
        DelayedFamily, FaceFluxField, MsrMaterial, NeutronFluxField, PrecursorField,
        TemperatureField, XsFields,
    };
    pub use crate::precursors::PrecursorDrift;
    pub use crate::ring_mesh::RingMesh;
    pub use crate::thermal::{
        CoupledMsrSolver, CoupledReport, SaltThermalConfig, SaltThermalModel,
    };
}
